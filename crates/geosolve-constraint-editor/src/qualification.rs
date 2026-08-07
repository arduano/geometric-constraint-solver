// SPDX-License-Identifier: GPL-3.0-or-later

//! Executable, deterministic M40 qualification corpus.

#![allow(clippy::default_trait_access)]

use crate::{
    ConstraintEditor, ConstraintKind, ConstructionPoint, ConstructionPreview, ConstructionProposal,
    EditorEffect, EditorScene, EditorTool, LifecycleStatus, Modifiers, PickTolerance, PointerInput,
    RetainedEditorCoordinator, ScreenPoint, SelectionItem, SnapTolerance, Viewport,
};
use geosolve_sketch::{
    CurveDefinition, CurveSpan, DocumentCommandEffect, DocumentConstraintDefinition,
    DocumentDimensionMode, DocumentSolveRequest, RetainedSketchDocumentSession, ScalarDomain,
    ScalarUnit, SketchDocument,
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::Value;

const CORPUS: &[u8] = include_bytes!("../tests/m40_transition_corpus.json");
const MATRIX: &[u8] = include_bytes!("../../../docs/M40_QUALIFICATION_MATRIX.json");
// These identifiers preserve the completed M40 cross-channel evidence contract.
// M48 retired the browser harness; they are not a current browser gate.
const FROZEN_M40_BROWSER_EVIDENCE_IDS: &[&str] = &[
    "browser.wasm-report-parity",
    "browser.creation-routes",
    "browser.pointer-normalization",
    "browser.selection-identity",
    "browser.constraint-glyph",
    "browser.dimension-persistence",
    "browser.projected-drag",
    "browser.history-delete-reload",
    "browser.redundancy-presentation",
    "browser.conflict-retention",
    "browser.lifecycle",
    "browser.malformed-storage",
    "browser.accessibility",
    "browser.evidence-route",
];

/// One deterministic qualification result. This is evidence, not a product API.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct M40QualificationCaseResult {
    pub id: String,
    pub passed: bool,
    pub result_digest: String,
    pub detail: String,
}

/// Canonically serializable report produced identically on native and WASM.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct M40QualificationReport {
    pub schema_version: u32,
    pub corpus_checksum: String,
    pub passed: bool,
    pub cases: Vec<M40QualificationCaseResult>,
}

impl M40QualificationReport {
    /// Stable JSON used by the release WASM adapter and the archived M40 evidence.
    ///
    /// # Panics
    ///
    /// Panics if this fixed, serializable report type cannot be encoded as JSON.
    #[must_use]
    pub fn canonical_json(&self) -> String {
        format!(
            "{}\n",
            serde_json::to_string(self).expect("qualification report is serializable")
        )
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Corpus {
    schema_version: u32,
    seed: u64,
    cases: Vec<CorpusCase>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CorpusCase {
    id: String,
    operation: String,
    input: Value,
    expected_digest: String,
}

/// Returns the exact checked-in bytes consumed by every qualification channel.
#[must_use]
pub const fn m40_qualification_corpus() -> &'static [u8] {
    CORPUS
}

/// Validates the checked-in historical M40 evidence manifest against the embedded
/// corpus and its frozen browser-evidence registry.
#[doc(hidden)]
pub fn validate_m40_qualification_matrix() -> Result<(), String> {
    let corpus = parse_corpus(CORPUS)?;
    let matrix: QualificationMatrix = serde_json::from_slice(MATRIX)
        .map_err(|error| format!("qualification matrix JSON/schema error: {error}"))?;
    if matrix.schema_version != 1 || matrix.milestone != "M40.6" {
        return Err("qualification matrix must be schema-v1 for M40.6".into());
    }
    let corpus_ids: std::collections::BTreeSet<_> =
        corpus.cases.iter().map(|case| case.id.as_str()).collect();
    let mut row_ids = std::collections::BTreeSet::new();
    for row in &matrix.rows {
        if !row_ids.insert(row.id.as_str()) {
            return Err(format!("duplicate matrix row id {}", row.id));
        }
        if row.status != "covered" {
            return Err(format!("matrix row {} is not covered", row.id));
        }
        if !matches!(row.policy_owner.as_str(), "editor" | "sketch" | "platform") {
            return Err(format!("matrix row {} has invalid policy owner", row.id));
        }
        for channel in ["native", "wasm", "browser"] {
            if !row.channels.iter().any(|value| value == channel) {
                return Err(format!(
                    "matrix row {} is missing {channel} evidence",
                    row.id
                ));
            }
        }
        if row.native.is_empty() || row.wasm.is_empty() || row.browser.is_empty() {
            return Err(format!(
                "matrix row {} has an empty evidence channel",
                row.id
            ));
        }
        for id in row.native.iter().chain(&row.wasm) {
            if !corpus_ids.contains(id.as_str()) {
                return Err(format!(
                    "matrix row {} references unknown corpus id {id}",
                    row.id
                ));
            }
        }
        for id in &row.browser {
            if !FROZEN_M40_BROWSER_EVIDENCE_IDS.contains(&id.as_str()) {
                return Err(format!(
                    "matrix row {} references unknown browser id {id}",
                    row.id
                ));
            }
        }
        if row.sources.is_empty()
            || row
                .sources
                .iter()
                .any(|source| !valid_source_anchor(source))
        {
            return Err(format!(
                "matrix row {} has an invalid source anchor",
                row.id
            ));
        }
    }
    for required in REQUIRED_MATRIX_ROWS {
        if !row_ids.contains(required) {
            return Err(format!(
                "matrix is missing required category/UAT row {required}"
            ));
        }
    }
    Ok(())
}

const REQUIRED_MATRIX_ROWS: &[&str] = &[
    "m40.category.creation",
    "m40.category.snapping",
    "m40.category.selection",
    "m40.category.constraints",
    "m40.category.dimensions",
    "m40.category.drag",
    "m40.category.history",
    "m40.category.redundancy",
    "m40.category.conflict",
    "m40.category.lifecycle",
    "m40.category.replay",
    "m40.category.boundaries",
    "m40.category.adapter",
    "m40.uat.creation",
    "m40.uat.selection",
    "m40.uat.constraints",
    "m40.uat.dimensions",
    "m40.uat.drag",
    "m40.uat.history",
    "m40.uat.redundancy",
    "m40.uat.conflict",
    "m40.uat.accepted-lifecycle",
    "m40.uat.preview-lifecycle",
    "m40.uat.unsolved-lifecycle",
    "m40.uat.rejected-lifecycle",
    "m40.uat.overall-authoring",
];

fn valid_source_anchor(source: &str) -> bool {
    matches!(
        source,
        "docs/M40_HEADLESS_QUALIFICATION.md#M40.6-qualification-matrix-and-implementation-plan"
            | "docs/M40_UAT.md#Targeted-Recheck-UAT-C1-F1"
            | "docs/M40_UAT.md#Targeted-Recheck-UAT-C1-F2"
            | "docs/M40_UAT.md#Targeted-Recheck-UAT-C1-F3"
            | "docs/M40_UAT.md#Scorecard"
            | "PLAN.md#M40.6"
            | "ACCEPTANCE.md#M40.6"
    )
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct QualificationMatrix {
    schema_version: u32,
    milestone: String,
    rows: Vec<QualificationMatrixRow>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct QualificationMatrixRow {
    id: String,
    sources: Vec<String>,
    channels: Vec<String>,
    native: Vec<String>,
    wasm: Vec<String>,
    browser: Vec<String>,
    policy_owner: String,
    status: String,
}

/// Runs the ordinary Rust interaction oracle over the embedded corpus.
#[must_use]
pub fn run_m40_qualification() -> M40QualificationReport {
    let parse_result = parse_corpus(CORPUS);
    let Ok(corpus) = parse_result else {
        return M40QualificationReport {
            schema_version: 1,
            corpus_checksum: digest(CORPUS),
            passed: false,
            cases: vec![M40QualificationCaseResult {
                id: "corpus.parse".into(),
                passed: false,
                result_digest: digest(b"parse-error"),
                detail: "embedded corpus did not parse".into(),
            }],
        };
    };
    let mut cases = Vec::with_capacity(corpus.cases.len());
    for case in corpus.cases {
        let outcome = execute(&case.operation, &case.input, corpus.seed);
        let (trace, error) = match outcome {
            Ok(trace) => (trace, None),
            Err(error) => (format!("error:{error}"), Some(error)),
        };
        let result_digest = digest(trace.as_bytes());
        let passed = error.is_none() && result_digest == case.expected_digest;
        cases.push(M40QualificationCaseResult {
            id: case.id,
            passed,
            result_digest,
            detail: error.unwrap_or_else(|| {
                if passed {
                    "covered".into()
                } else {
                    format!("digest mismatch; trace={trace}")
                }
            }),
        });
    }
    M40QualificationReport {
        schema_version: corpus.schema_version,
        corpus_checksum: digest(CORPUS),
        passed: cases.iter().all(|case| case.passed),
        cases,
    }
}

fn execute(operation: &str, input: &Value, seed: u64) -> Result<String, String> {
    match operation {
        "viewport_matrix" => viewport_matrix(input),
        "draft_matrix" => draft_matrix(input),
        "snap_matrix" => snap_matrix(input),
        "pick_matrix" => pick_matrix(input),
        "selection_matrix" => selection_matrix(input),
        "constraint_matrix" => constraint_matrix(input),
        "dimension_matrix" => dimension_matrix(input),
        "drag_matrix" => drag_matrix(input),
        "history_matrix" => history_matrix(input),
        "redundancy" => redundancy(input),
        "conflict_retention" => conflict_retention(input),
        "lifecycle_matrix" => lifecycle_matrix(input),
        "malformed_matrix" => malformed_matrix(input),
        "seeded_model" => seeded_model(input, seed),
        other => Err(format!("unknown operation {other}")),
    }
}

fn parse_input<T: DeserializeOwned>(input: &Value) -> Result<T, String> {
    serde_json::from_value(input.clone())
        .map_err(|error| format!("invalid operation input: {error}"))
}

fn viewport_matrix(input: &Value) -> Result<String, String> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Input {
        scales: Vec<f64>,
        centers: Vec<[f64; 2]>,
    }
    let Input { scales, centers } = parse_input(input)?;
    let mut trace: Vec<String> = Vec::new();
    for scale in scales {
        for center in &centers {
            let center = *center;
            let viewport =
                Viewport::new([1000.0, 700.0], center, scale).map_err(|e| e.to_string())?;
            let point = [center[0] + 2.0 / scale, center[1] - 3.0 / scale];
            let roundtrip = viewport.screen_to_model(viewport.model_to_screen(point));
            if !roundtrip
                .iter()
                .zip(point)
                .all(|(actual, expected)| (actual - expected).abs() <= 1.0e-12)
            {
                return Err("viewport round trip changed finite input".into());
            }
            trace.push(format!("{scale:e}:{:.1}:{:.1}", roundtrip[0], roundtrip[1]));
        }
    }
    Ok(trace.join("|"))
}

fn fixture() -> Result<
    (
        SketchDocument,
        EditorScene,
        [geosolve_sketch::DesignPointId; 4],
        [CurveSpan; 2],
    ),
    String,
> {
    let mut document = SketchDocument::new(10.0).map_err(|e| e.to_string())?;
    let p0 = document
        .add_point("a", [-4.0, 1.0])
        .map_err(|e| e.to_string())?;
    let p1 = document
        .add_point("b", [4.0, 1.0])
        .map_err(|e| e.to_string())?;
    let p2 = document
        .add_point("c", [-4.0, -1.0])
        .map_err(|e| e.to_string())?;
    let p3 = document
        .add_point("d", [4.0, -1.0])
        .map_err(|e| e.to_string())?;
    let first = document
        .add_curve(
            "first",
            CurveDefinition::Line {
                start: p0,
                end: p1,
                branch_direction: [1.0, 0.0],
            },
        )
        .map_err(|e| e.to_string())?;
    let second = document
        .add_curve(
            "second",
            CurveDefinition::Line {
                start: p2,
                end: p3,
                branch_direction: [1.0, 0.0],
            },
        )
        .map_err(|e| e.to_string())?;
    let session = RetainedSketchDocumentSession::new(
        document.clone(),
        DocumentSolveRequest::default(),
        Default::default(),
    )
    .map_err(|e| e.to_string())?;
    let scene = EditorScene::from_accepted(
        1,
        session.design_identity(),
        &document,
        Viewport::new([1000.0, 700.0], [0.0, 0.0], 50.0).map_err(|e| e.to_string())?,
        0.5,
    )
    .map_err(|e| e.to_string())?;
    Ok((
        document,
        scene,
        [p0, p1, p2, p3],
        [
            CurveSpan {
                curve: first,
                segment: 0,
            },
            CurveSpan {
                curve: second,
                segment: 0,
            },
        ],
    ))
}

fn pointer(id: u64, x: f64, y: f64, modifiers: Modifiers) -> PointerInput {
    PointerInput {
        pointer_id: id,
        position: ScreenPoint { x, y },
        modifiers,
    }
}

fn positions_near(first: [f64; 2], second: [f64; 2]) -> bool {
    first
        .into_iter()
        .zip(second)
        .all(|(first, second)| (first - second).abs() <= 1.0e-12)
}

#[allow(clippy::too_many_lines)]
fn draft_matrix(input: &Value) -> Result<String, String> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Input {
        tools: Vec<String>,
        routes: Vec<String>,
    }
    let (_, scene, _, _) = fixture()?;
    let Input { tools, routes } = parse_input(input)?;
    if routes
        != [
            "pointer",
            "button",
            "double-click",
            "enter",
            "escape",
            "pointercancel",
        ]
    {
        return Err("draft routes are not the required completion/cancellation routes".into());
    }
    let mut trace: Vec<String> = Vec::new();
    for key in tools {
        let tool = match key.as_str() {
            "point" => EditorTool::Point,
            "line" => EditorTool::Line,
            "polyline" => EditorTool::Polyline,
            "rectangle" => EditorTool::Rectangle,
            "circle" => EditorTool::Circle,
            "arc" => EditorTool::CounterClockwiseArc,
            _ => return Err("unknown tool".into()),
        };
        if tool == EditorTool::Polyline {
            for alias in ["button", "double-click", "enter"] {
                let mut editor = ConstraintEditor::default();
                editor.activate_tool(tool);
                let mut effects = Vec::new();
                for model in [[0.0, 0.0], [1.0, 0.0], [2.0, 1.0], [3.0, 1.0]] {
                    let screen = scene.viewport.model_to_screen(model);
                    effects.extend(editor.pointer_down(
                        &scene,
                        pointer(7, screen.x, screen.y, Modifiers::default()),
                    ));
                }
                let completion = editor.complete_draft(scene.design_identity);
                let Some(EditorEffect::CommitConstruction {
                    proposal: ConstructionProposal::Polyline { points },
                    ..
                }) = completion.first()
                else {
                    return Err(format!("polyline {alias} did not use complete_draft"));
                };
                if points.len() != 4 || count_construction_commits(&effects) != 0 {
                    return Err("polyline did not continue through four points".into());
                }
                effects.extend(completion);
                if count_construction_commits(&effects) != 1 {
                    return Err(format!("polyline {alias} did not commit exactly once"));
                }
                trace.push(format!("polyline:{alias}:core-complete_draft:4:1"));
            }
        } else {
            let mut editor = ConstraintEditor::default();
            editor.activate_tool(tool);
            let first = scene.viewport.model_to_screen([0.0, 0.0]);
            let second_model = if tool == EditorTool::Rectangle {
                [2.0, 1.0]
            } else {
                [2.0, 0.0]
            };
            let second = scene.viewport.model_to_screen(second_model);
            let third = scene.viewport.model_to_screen([0.0, 5.0]);
            let mut effects =
                editor.pointer_down(&scene, pointer(7, first.x, first.y, Modifiers::default()));
            if tool != EditorTool::Point {
                effects.extend(
                    editor
                        .pointer_move(&scene, pointer(7, second.x, second.y, Modifiers::default())),
                );
                effects.extend(
                    editor
                        .pointer_down(&scene, pointer(7, second.x, second.y, Modifiers::default())),
                );
            }
            if tool == EditorTool::CounterClockwiseArc {
                let preview =
                    editor.pointer_move(&scene, pointer(7, third.x, third.y, Modifiers::default()));
                let commit =
                    editor.pointer_down(&scene, pointer(7, third.x, third.y, Modifiers::default()));
                let normalized = matches!((preview.as_slice(), commit.as_slice()),
                    (
                        [
                            EditorEffect::PreviewConstruction(
                                ConstructionPreview::Complete { proposal: ConstructionProposal::CounterClockwiseArc {
                                    end: preview_end, ..
                                }, .. },
                            ),
                        ],
                        [
                            EditorEffect::CommitConstruction {
                                proposal:
                                    ConstructionProposal::CounterClockwiseArc {
                                        end: commit_end, ..
                                    },
                                ..
                            },
                            EditorEffect::ClearConstructionPreview,
                        ],
                    ) if positions_near(*preview_end, *commit_end)
                        && preview_end[0].abs() <= 1.0e-12
                        && (preview_end[1] - 2.0).abs() <= 1.0e-12
                );
                if !normalized {
                    return Err("arc preview/commit did not share normalized endpoint".into());
                }
                effects.extend(preview);
                effects.extend(commit);
            }
            let previews = effects
                .iter()
                .filter(|effect| {
                    matches!(
                        effect,
                        EditorEffect::PreviewConstruction(ConstructionPreview::Complete { .. })
                    )
                })
                .count();
            let commits = count_construction_commits(&effects);
            if commits != 1 {
                return Err(format!("{key} terminal pointer emitted {commits} commits"));
            }
            let expected_previews = usize::from(tool != EditorTool::Point);
            if previews != expected_previews {
                return Err(format!("{key} emitted {previews} proposals"));
            }
            trace.push(format!("{key}:pointer:{previews}:{commits}"));
        }

        for alias in ["escape", "pointercancel"] {
            let mut editor = ConstraintEditor::default();
            editor.activate_tool(tool);
            if tool == EditorTool::Point {
                editor.activate_tool(EditorTool::Line);
            }
            let first = scene.viewport.model_to_screen([0.0, 0.0]);
            editor.pointer_down(&scene, pointer(11, first.x, first.y, Modifiers::default()));
            let cancelled = editor.cancel();
            let later = editor.complete_draft(scene.design_identity);
            if count_construction_commits(&cancelled) != 0
                || count_construction_commits(&later) != 0
                || !cancelled
                    .iter()
                    .any(|effect| matches!(effect, EditorEffect::ClearConstructionPreview))
            {
                return Err(format!("{key} {alias} did not semantically cancel"));
            }
            trace.push(format!("{key}:{alias}:core-cancel:no-commit"));
        }
    }
    Ok(trace.join("|"))
}

fn count_construction_commits(effects: &[EditorEffect]) -> usize {
    effects
        .iter()
        .filter(|effect| matches!(effect, EditorEffect::CommitConstruction { .. }))
        .count()
}

fn snap_matrix(input: &Value) -> Result<String, String> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Input {
        offsets: Vec<f64>,
        tie_policy: String,
    }
    let (_, scene, points, _) = fixture()?;
    let Input {
        offsets,
        tie_policy,
    } = parse_input(input)?;
    if tie_policy != "persistent-identity" || offsets != [7.999, 8.0, 8.001] {
        return Err("unexpected snap tie policy".into());
    }
    let mut trace = Vec::new();
    for offset in offsets {
        let mut editor = ConstraintEditor::default();
        editor.activate_tool(EditorTool::Line);
        editor
            .set_snap_tolerance(SnapTolerance { point_pixels: 8.0 })
            .map_err(|e| e.to_string())?;
        let endpoint = scene.viewport.model_to_screen([-4.0, 1.0]);
        editor.pointer_down(
            &scene,
            pointer(1, endpoint.x + offset, endpoint.y, Modifiers::default()),
        );
        let effects = editor.pointer_move(&scene, pointer(1, 500.0, 350.0, Modifiers::default()));
        let snapped = effects.iter().any(|effect| matches!(effect, EditorEffect::PreviewConstruction(ConstructionPreview::Complete { proposal: ConstructionProposal::Line { start: ConstructionPoint::Existing { id, .. }, .. }, .. }) if *id == points[0]));
        trace.push(format!("{offset:.3}:{snapped}"));
    }
    let midpoint = scene.viewport.model_to_screen([-4.0, 0.0]);
    let mut editor = ConstraintEditor::default();
    editor
        .set_snap_tolerance(SnapTolerance { point_pixels: 51.0 })
        .map_err(|e| e.to_string())?;
    editor.activate_tool(EditorTool::Line);
    editor.pointer_down(
        &scene,
        pointer(2, midpoint.x, midpoint.y, Modifiers::default()),
    );
    let target = scene.viewport.model_to_screen([0.0, 0.0]);
    let effects = editor.pointer_move(&scene, pointer(2, target.x, target.y, Modifiers::default()));
    let winner = points[0].min(points[2]);
    if !matches!(effects.as_slice(), [EditorEffect::PreviewConstruction(ConstructionPreview::Complete { proposal: ConstructionProposal::Line { start: ConstructionPoint::Existing { id, .. }, .. }, .. })] if *id == winner)
    {
        return Err("equal-distance snap did not use persistent identity order".into());
    }
    trace.push("tie:actual:persistent-identity-winner".into());
    Ok(trace.join("|"))
}

fn pick_matrix(input: &Value) -> Result<String, String> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Input {
        curve_offsets: Vec<f64>,
        overlaps: Vec<String>,
    }
    let (_, scene, points, spans) = fixture()?;
    let Input {
        curve_offsets: offsets,
        overlaps,
    } = parse_input(input)?;
    if overlaps != ["point-curve", "curve-curve"] || offsets != [11.999, 12.0, 12.001] {
        return Err("unexpected overlap matrix".into());
    }
    let mut trace = Vec::new();
    for offset in offsets {
        let hit = scene.hit_test(
            ScreenPoint {
                x: 500.0,
                y: 300.0 + offset,
            },
            PickTolerance::default(),
        );
        let kind = hit.map_or("none", |value| match value.item {
            SelectionItem::Point(_) => "point",
            SelectionItem::Curve(_) => "curve",
            SelectionItem::Constraint(_) => "constraint",
            SelectionItem::Dimension(_) => "dimension",
            SelectionItem::Feature(_) => "feature",
            SelectionItem::FeatureCorner(_) => "feature-corner",
        });
        trace.push(format!("{offset:.3}:{kind}"));
    }
    let endpoint = scene.viewport.model_to_screen([-4.0, 1.0]);
    let endpoint_hit = scene
        .hit_test(endpoint, PickTolerance::default())
        .ok_or("endpoint miss")?;
    if endpoint_hit.item != SelectionItem::Point(points[0]) {
        return Err("point did not win overlap".into());
    }
    if spans[0] == spans[1] {
        return Err("curve identities aliased".into());
    }
    trace.push("point-priority".into());
    for offset in [7.999, 8.0, 8.001] {
        let hit = scene.hit_test(
            ScreenPoint {
                x: endpoint.x + offset,
                y: endpoint.y,
            },
            PickTolerance::default(),
        );
        let point = hit.is_some_and(|hit| hit.item == SelectionItem::Point(points[0]));
        if point != (offset <= 8.0) {
            return Err("point pick tolerance boundary changed".into());
        }
        trace.push(format!("point:{offset:.3}:{point}"));
    }
    let tie = scene
        .hit_test(
            scene.viewport.model_to_screen([0.0, 0.0]),
            PickTolerance {
                point_pixels: 0.0,
                curve_pixels: 51.0,
                annotation_pixels: 10.0,
            },
        )
        .ok_or("equal-distance curve tie missed")?;
    let winner = spans[0].min(spans[1]);
    if tie.item != SelectionItem::Curve(winner) {
        return Err("curve tie did not use persistent identity order".into());
    }
    trace.push("curve-tie:actual:persistent-identity-winner".into());
    Ok(trace.join("|"))
}

fn selection_matrix(input: &Value) -> Result<String, String> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Input {
        modifiers: Vec<String>,
        origins: Vec<String>,
    }
    let (_, scene, points, spans) = fixture()?;
    let Input { modifiers, origins } = parse_input(input)?;
    if origins != ["canvas", "tree", "inspector"] {
        return Err("unexpected selection origins".into());
    }
    let mut replacement = ConstraintEditor::default();
    replacement.pointer_down(&scene, pointer(1, 500.0, 300.0, Modifiers::default()));
    replacement.pointer_down(&scene, pointer(2, 500.0, 400.0, Modifiers::default()));
    if replacement.selection() != [SelectionItem::Curve(spans[1])] {
        return Err("canvas replacement selection did not replace".into());
    }
    let mut trace = vec!["canvas:core:replacement:curve".into()];
    for modifier in modifiers {
        let key = modifier.as_str();
        let extension = match key {
            "shift" => Modifiers {
                shift: true,
                ..Modifiers::default()
            },
            "control" => Modifiers {
                control: true,
                ..Modifiers::default()
            },
            "command" => Modifiers {
                command: true,
                ..Modifiers::default()
            },
            _ => Modifiers::default(),
        };
        let mut editor = ConstraintEditor::default();
        let endpoint = scene.viewport.model_to_screen([-4.0, 1.0]);
        editor.pointer_down(
            &scene,
            pointer(1, endpoint.x, endpoint.y, Modifiers::default()),
        );
        editor.pointer_down(&scene, pointer(2, 500.0, 400.0, extension));
        if editor.selection()
            != [
                SelectionItem::Point(points[0]),
                SelectionItem::Curve(spans[1]),
            ]
        {
            return Err(format!("{key} did not preserve ordered selection"));
        }
        trace.push(format!("canvas:core:{key}:point-then-curve"));
    }
    trace.push("tree:adapter-owned:not-core-executed".into());
    trace.push("inspector:adapter-owned:not-core-executed".into());
    Ok(trace.join("|"))
}

fn constraint_matrix(input: &Value) -> Result<String, String> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Input {
        kinds: Vec<String>,
    }
    let (document, _, points, spans) = fixture()?;
    let Input { kinds } = parse_input(input)?;
    let mut trace: Vec<String> = Vec::new();
    for value in kinds {
        let key = value.as_str();
        let (kind, selection) = match key {
            "fixed" => (ConstraintKind::Fixed, vec![SelectionItem::Point(points[0])]),
            "coincident" => (
                ConstraintKind::Coincident,
                vec![
                    SelectionItem::Point(points[0]),
                    SelectionItem::Point(points[2]),
                ],
            ),
            "horizontal" => (
                ConstraintKind::Horizontal,
                vec![SelectionItem::Curve(spans[0])],
            ),
            "vertical" => (
                ConstraintKind::Vertical,
                vec![SelectionItem::Curve(spans[0])],
            ),
            "parallel" => (
                ConstraintKind::Parallel,
                vec![
                    SelectionItem::Curve(spans[0]),
                    SelectionItem::Curve(spans[1]),
                ],
            ),
            "perpendicular" => (
                ConstraintKind::Perpendicular,
                vec![
                    SelectionItem::Curve(spans[0]),
                    SelectionItem::Curve(spans[1]),
                ],
            ),
            "equal-length" => (
                ConstraintKind::EqualLength,
                vec![
                    SelectionItem::Curve(spans[0]),
                    SelectionItem::Curve(spans[1]),
                ],
            ),
            _ => return Err("unknown constraint".into()),
        };
        let mut editor = ConstraintEditor::default();
        editor.set_selection(selection);
        if !editor.available_constraints(&document).contains(&kind) {
            return Err(format!("{key} unavailable"));
        }
        let edit = editor
            .constraint_edit(&document, kind, key)
            .map_err(|e| e.to_string())?;
        let mut retained_document = document.clone();
        if kind == ConstraintKind::Perpendicular {
            retained_document
                .set_point_position(points[3], [-3.0, 7.0])
                .map_err(|error| error.to_string())?;
        }
        let session = RetainedSketchDocumentSession::new(
            retained_document,
            DocumentSolveRequest::default(),
            Default::default(),
        )
        .map_err(|error| error.to_string())?;
        let mut retained =
            RetainedEditorCoordinator::new(session).map_err(|error| error.to_string())?;
        let outcome = retained
            .apply_edit(retained.session().design_identity(), edit)
            .map_err(|error| error.to_string())?;
        if outcome.published_accepted.is_none()
            || retained.lifecycle().status != LifecycleStatus::Accepted
        {
            return Err(format!(
                "{key} edit did not publish an accepted retained mutation"
            ));
        }
        editor.set_selection([]);
        if editor.constraint_edit(&document, kind, key).is_ok() {
            return Err(format!("{key} accepted invalid applicability"));
        }
        trace.push(key.into());
    }
    Ok(trace.join("|"))
}

fn coordinator() -> Result<
    (
        RetainedEditorCoordinator,
        [geosolve_sketch::DesignPointId; 2],
        CurveSpan,
    ),
    String,
> {
    let (document, _, points, spans) = fixture()?;
    let session = RetainedSketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        Default::default(),
    )
    .map_err(|e| e.to_string())?;
    Ok((
        RetainedEditorCoordinator::new(session).map_err(|e| e.to_string())?,
        [points[0], points[1]],
        spans[0],
    ))
}

fn dimension_matrix(input: &Value) -> Result<String, String> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Input {
        families: Vec<String>,
        modes: Vec<String>,
        transitions: Vec<String>,
    }
    let Input {
        families,
        modes,
        transitions,
    } = parse_input(input)?;
    if families != ["point-distance", "segment-length"]
        || modes != ["driving", "reference"]
        || transitions != ["mode", "undo", "redo", "reload"]
    {
        return Err("unexpected dimension matrix".into());
    }
    let mut trace = Vec::new();
    for family in ["point", "segment"] {
        for mode in [
            DocumentDimensionMode::Driving,
            DocumentDimensionMode::Reference,
        ] {
            let (mut coordinator, points, span) = coordinator()?;
            coordinator
                .editor_mut()
                .set_selection(if family == "point" {
                    vec![
                        SelectionItem::Point(points[0]),
                        SelectionItem::Point(points[1]),
                    ]
                } else {
                    vec![SelectionItem::Curve(span)]
                });
            let expected = coordinator.session().design_identity();
            let id = coordinator
                .add_selected_dimension(expected, mode, "qualification")
                .map_err(|e| e.to_string())?
                .value;
            let transitioned = if mode == DocumentDimensionMode::Driving {
                DocumentDimensionMode::Reference
            } else {
                DocumentDimensionMode::Driving
            };
            coordinator
                .set_dimension_mode(coordinator.session().design_identity(), id, transitioned)
                .map_err(|e| e.to_string())?;
            assert_dimension(&coordinator, id, transitioned, "mode transition")?;
            let transitioned_checkpoint = coordinator.checkpoint().clone();
            coordinator.undo().map_err(|e| e.to_string())?;
            assert_dimension(&coordinator, id, mode, "undo")?;
            coordinator.redo().map_err(|e| e.to_string())?;
            assert_dimension(&coordinator, id, transitioned, "redo")?;
            coordinator.undo().map_err(|e| e.to_string())?;
            coordinator
                .reload(&transitioned_checkpoint)
                .map_err(|e| e.to_string())?;
            assert_dimension(&coordinator, id, transitioned, "checkpoint reload")?;
            if coordinator.checkpoint().design_json() != transitioned_checkpoint.design_json() {
                return Err("dimension reload changed canonical bytes".into());
            }
            trace.push(format!(
                "{family}:{mode:?}:mode={transitioned:?}:undo={mode:?}:redo={transitioned:?}:reload={transitioned:?}:same-id"
            ));
        }
    }
    Ok(trace.join("|"))
}

fn assert_dimension(
    coordinator: &RetainedEditorCoordinator,
    id: geosolve_sketch::DocumentDimensionId,
    mode: DocumentDimensionMode,
    stage: &str,
) -> Result<(), String> {
    let dimension = coordinator
        .session()
        .design_document()
        .dimension(id)
        .ok_or_else(|| format!("{stage} lost dimension identity"))?;
    if dimension.mode != mode {
        return Err(format!("{stage} produced the wrong dimension mode"));
    }
    Ok(())
}

#[allow(clippy::too_many_lines)]
fn drag_matrix(input: &Value) -> Result<String, String> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Input {
        threshold: Vec<f64>,
        results: Vec<String>,
        release_commits: usize,
    }
    let (document, scene, points, _) = fixture()?;
    let Input {
        threshold,
        results,
        release_commits,
    } = parse_input(input)?;
    if results != ["foreign", "stale", "accepted", "rejected"]
        || release_commits != 1
        || threshold != [2.999, 3.0, 3.001]
    {
        return Err("unexpected drag result matrix".into());
    }
    let endpoint = scene.viewport.model_to_screen([-4.0, 1.0]);
    let mut trace = Vec::new();
    for movement in threshold {
        let mut editor = ConstraintEditor::default();
        editor.pointer_down(
            &scene,
            pointer(9, endpoint.x, endpoint.y, Modifiers::default()),
        );
        let effects = editor.pointer_move(
            &scene,
            pointer(9, endpoint.x + movement, endpoint.y, Modifiers::default()),
        );
        let request = effects.iter().find_map(|effect| {
            if let EditorEffect::RequestProjectedPointMove { request_id, .. } = effect {
                Some(*request_id)
            } else {
                None
            }
        });
        if (movement < 3.0) != request.is_none() {
            return Err("drag threshold inclusivity changed".into());
        }
        trace.push(format!("{movement:.3}:{}", request.is_some()));
    }

    let mut editor = ConstraintEditor::default();
    editor.pointer_down(
        &scene,
        pointer(9, endpoint.x, endpoint.y, Modifiers::default()),
    );
    let first = projection_request(&editor.pointer_move(
        &scene,
        pointer(9, endpoint.x + 3.0, endpoint.y, Modifiers::default()),
    ))?;
    if !editor
        .projected_drag_result(99, first, points[0], Some([-3.0, 1.0]))
        .is_empty()
        || !editor
            .projected_drag_result(9, first + 1, points[0], Some([-3.0, 1.0]))
            .is_empty()
    {
        return Err("foreign or stale projected result was accepted".into());
    }
    let nominal_target = [-3.0, 1.25];
    let session = RetainedSketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        Default::default(),
    )
    .map_err(|error| error.to_string())?;
    let mut retained =
        RetainedEditorCoordinator::new(session).map_err(|error| error.to_string())?;
    let mut preview = retained.session().clone();
    let request = preview
        .last_attempt()
        .input()
        .candidate_request()
        .without_previous_state_preferences()
        .with_drag(points[0], nominal_target);
    preview
        .reattempt(preview.design_identity(), request)
        .map_err(|error| error.to_string())?;
    let accepted = preview
        .accepted_state()
        .and_then(|state| state.document().point(points[0]))
        .map(|point| point.position)
        .ok_or_else(|| "projected fixture did not publish an accepted point".to_owned())?;
    retained
        .mark_solved_preview(&preview)
        .map_err(|error| error.to_string())?;
    if !matches!(editor.projected_drag_result(9, first, points[0], Some(accepted)).as_slice(),
        [EditorEffect::PreviewPointMove { model_position, .. }] if positions_near(*model_position, accepted))
    {
        return Err("matching accepted projected result was not previewed".into());
    }
    if !editor
        .projected_drag_result(9, first, points[0], Some([f64::NAN, 0.0]))
        .is_empty()
        || !editor
            .projected_drag_result(9, first, points[0], None)
            .is_empty()
    {
        return Err("nonfinite or rejected projected result changed preview".into());
    }
    let second = projection_request(&editor.pointer_move(
        &scene,
        pointer(9, endpoint.x + 5.0, endpoint.y, Modifiers::default()),
    ))?;
    if second <= first
        || !editor
            .projected_drag_result(9, first, points[0], Some([-2.0, 2.0]))
            .is_empty()
        || !editor
            .projected_drag_result(9, second, points[0], None)
            .is_empty()
    {
        return Err("out-of-order/rejected request handling changed".into());
    }
    let release = editor.pointer_up(
        &scene,
        scene.design_identity,
        pointer(9, endpoint.x + 5.0, endpoint.y, Modifiers::default()),
    );
    if !matches!(release.as_slice(),
        [EditorEffect::CommitPointMove { model_position, .. }, EditorEffect::ClearPointPreview]
            if positions_near(*model_position, accepted))
    {
        return Err("release did not commit exactly the retained last-valid preview".into());
    }
    let mut published = 0;
    for effect in &release {
        if retained
            .apply_editor_effect(effect)
            .map_err(|error| error.to_string())?
            .is_some_and(|outcome| outcome.published_accepted.is_some())
        {
            published += 1;
        }
    }
    if published != 1 || retained.lifecycle().status != LifecycleStatus::Accepted {
        return Err("release commit was not retained as exactly one accepted mutation".into());
    }
    trace.push("results:foreign-ignored:stale-ignored:accepted:nonfinite-ignored:rejected-retained:out-of-order-ignored:one-commit".into());

    let mut cancelled = ConstraintEditor::default();
    cancelled.pointer_down(
        &scene,
        pointer(10, endpoint.x, endpoint.y, Modifiers::default()),
    );
    let request = projection_request(&cancelled.pointer_move(
        &scene,
        pointer(10, endpoint.x + 3.0, endpoint.y, Modifiers::default()),
    ))?;
    cancelled.projected_drag_result(10, request, points[0], Some([-2.0, 1.0]));
    if cancelled.cancel() != [EditorEffect::ClearPointPreview]
        || !cancelled
            .pointer_up(
                &scene,
                scene.design_identity,
                pointer(10, endpoint.x + 4.0, endpoint.y, Modifiers::default()),
            )
            .is_empty()
    {
        return Err("accepted-preview cancel allowed a later commit".into());
    }
    trace.push("cancel:ClearPointPreview:no-release-commit".into());
    Ok(trace.join("|"))
}

fn projection_request(effects: &[EditorEffect]) -> Result<u64, String> {
    match effects {
        [EditorEffect::RequestProjectedPointMove { request_id, .. }] => Ok(*request_id),
        _ => Err("expected exactly one projected drag request".into()),
    }
}

#[allow(clippy::too_many_lines)]
fn history_matrix(input: &Value) -> Result<String, String> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Input {
        actions: Vec<String>,
        retain_ids: bool,
    }
    let Input {
        actions,
        retain_ids,
    } = parse_input(input)?;
    if actions != ["delete", "suppress", "undo", "redo", "reload"] || !retain_ids {
        return Err("unexpected history matrix".into());
    }
    let (mut document, _, points, spans) = fixture()?;
    let constraint = document
        .add_constraint(
            "horizontal",
            DocumentConstraintDefinition::Horizontal { line: spans[0] },
        )
        .map_err(|e| e.to_string())?;
    let source = document
        .constraint(constraint)
        .ok_or("new constraint missing")?
        .source_id;
    let session = RetainedSketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        Default::default(),
    )
    .map_err(|e| e.to_string())?;
    let mut coordinator = RetainedEditorCoordinator::new(session).map_err(|e| e.to_string())?;
    coordinator
        .editor_mut()
        .set_selection([SelectionItem::Constraint(constraint)]);
    coordinator
        .set_selected_suppressed(coordinator.session().design_identity(), true)
        .map_err(|e| e.to_string())?;
    if !coordinator
        .session()
        .design_document()
        .source(source)
        .is_some_and(|value| value.suppressed)
    {
        return Err("selected source was not actually suppressed".into());
    }
    coordinator
        .set_selected_suppressed(coordinator.session().design_identity(), false)
        .map_err(|e| e.to_string())?;
    if coordinator
        .session()
        .design_document()
        .source(source)
        .is_none_or(|value| value.suppressed)
    {
        return Err("selected source was not actually unsuppressed".into());
    }
    coordinator.undo().map_err(|e| e.to_string())?;
    if !coordinator
        .session()
        .design_document()
        .source(source)
        .is_some_and(|value| value.suppressed)
    {
        return Err("suppression undo did not restore source state".into());
    }
    coordinator.redo().map_err(|e| e.to_string())?;
    if coordinator
        .session()
        .design_document()
        .source(source)
        .is_none_or(|value| value.suppressed)
    {
        return Err("suppression redo did not restore unsuppressed state".into());
    }
    coordinator
        .editor_mut()
        .set_selection([SelectionItem::Point(points[0])]);
    let id = coordinator.session().design_identity();
    coordinator.delete_selected(id).map_err(|e| e.to_string())?;
    let after_delete = coordinator.checkpoint().design_json().to_owned();
    coordinator.undo().map_err(|e| e.to_string())?;
    if coordinator
        .session()
        .design_document()
        .point(points[0])
        .is_none()
    {
        return Err("undo did not restore identity".into());
    }
    if coordinator
        .session()
        .design_document()
        .constraint(constraint)
        .is_none()
        || coordinator
            .session()
            .design_document()
            .source(source)
            .is_none()
    {
        return Err("delete undo did not restore dependent persistent identities".into());
    }
    coordinator.redo().map_err(|e| e.to_string())?;
    if coordinator.checkpoint().design_json() != after_delete {
        return Err("redo changed canonical design".into());
    }
    let saved = coordinator.checkpoint().clone();
    coordinator.reload(&saved).map_err(|e| e.to_string())?;
    if coordinator.checkpoint().design_json() != after_delete
        || coordinator
            .session()
            .design_document()
            .point(points[0])
            .is_some()
    {
        return Err("checkpoint reload did not restore canonical deleted state".into());
    }
    Ok("source:suppress-unsuppress-undo-redo|delete:cascade-undo-identities-redo-canonical|reload:canonical-deleted-state".into())
}

fn redundancy(input: &Value) -> Result<String, String> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Input {
        source: String,
    }
    if parse_input::<Input>(input)?.source != "sketch-owned-dto" {
        return Err("redundancy must use the accepted sketch DTO".into());
    }
    let mut document = SketchDocument::new(4.0).map_err(|e| e.to_string())?;
    let a = document
        .add_point("a", [0.0, 0.0])
        .map_err(|e| e.to_string())?;
    let b = document
        .add_point("b", [2.0, 0.0])
        .map_err(|e| e.to_string())?;
    document
        .add_constraint(
            "fix",
            DocumentConstraintDefinition::FixedPoint {
                point: a,
                target: [0.0, 0.0],
            },
        )
        .map_err(|e| e.to_string())?;
    for label in ["distance", "duplicate"] {
        let target = document
            .add_scalar(label, 2.0, ScalarUnit::Length, ScalarDomain::Positive)
            .map_err(|e| e.to_string())?;
        document
            .add_dimension(
                label,
                geosolve_sketch::DocumentDimensionDefinition::PointDistance {
                    first: a,
                    second: b,
                    target,
                },
                DocumentDimensionMode::Driving,
            )
            .map_err(|e| e.to_string())?;
    }
    let coordinator = RetainedEditorCoordinator::new(
        RetainedSketchDocumentSession::new(
            document,
            DocumentSolveRequest::default(),
            Default::default(),
        )
        .map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let redundancy = coordinator
        .accepted_redundancy()
        .ok_or("missing accepted redundancy DTO")?;
    if redundancy.fully_redundant_sources().is_empty()
        || redundancy.sources_containing_redundant_rows().is_empty()
    {
        return Err("redundancy DTO has no source identity".into());
    }
    if redundancy.fully_redundant_sources() != redundancy.sources_containing_redundant_rows() {
        return Err("redundancy source classifications disagree".into());
    }
    Ok(format!(
        "accepted:{}:design:{}:sources:{}",
        redundancy.accepted_state_identity().revision().get(),
        redundancy.design_identity().revision().get(),
        redundancy.fully_redundant_sources().len()
    ))
}

#[allow(clippy::too_many_lines)]
fn conflict_retention(input: &Value) -> Result<String, String> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Input {
        retain: Vec<String>,
    }
    if parse_input::<Input>(input)?.retain != ["accepted-json", "accepted-identity", "scene"] {
        return Err("unexpected conflict retention claims".into());
    }
    let mut document = SketchDocument::new(4.0).map_err(|e| e.to_string())?;
    let a = document
        .add_point("a", [0.0, 0.0])
        .map_err(|e| e.to_string())?;
    let b = document
        .add_point("b", [2.0, 0.0])
        .map_err(|e| e.to_string())?;
    for (point, target) in [(a, [0.0, 0.0]), (b, [2.0, 0.0])] {
        document
            .add_constraint(
                "fix",
                DocumentConstraintDefinition::FixedPoint { point, target },
            )
            .map_err(|e| e.to_string())?;
    }
    let mut coordinator = RetainedEditorCoordinator::new(
        RetainedSketchDocumentSession::new(
            document,
            DocumentSolveRequest::default(),
            Default::default(),
        )
        .map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    coordinator
        .editor_mut()
        .set_selection([SelectionItem::Point(a), SelectionItem::Point(b)]);
    let expected = coordinator.session().design_identity();
    let outcome = coordinator
        .apply_edit(
            expected,
            geosolve_sketch::DocumentEdit::CreateScalar {
                label: "bad distance".into(),
                value: 3.0,
                unit: ScalarUnit::Length,
                domain: ScalarDomain::Positive,
            },
        )
        .map_err(|e| e.to_string())?;
    let DocumentCommandEffect::CreatedScalar(scalar) = outcome.value else {
        return Err("scalar creation returned the wrong effect".into());
    };
    let accepted = coordinator
        .session()
        .export_accepted_json()
        .map_err(|e| e.to_string())?;
    let accepted_identity = coordinator
        .session()
        .accepted_state()
        .ok_or("scalar edit did not publish accepted state")?
        .identity();
    let accepted_design = coordinator
        .session()
        .accepted_state()
        .ok_or("missing accepted state")?
        .design_identity();
    let viewport = Viewport::new([1000.0, 700.0], [0.0, 0.0], 50.0).map_err(|e| e.to_string())?;
    let accepted_scene = EditorScene::from_accepted(
        accepted_identity.revision().get(),
        accepted_design,
        coordinator
            .session()
            .accepted_state()
            .ok_or("missing accepted scene state")?
            .document(),
        viewport,
        0.5,
    )
    .map_err(|e| e.to_string())?;
    coordinator
        .apply_edit(
            outcome.design,
            geosolve_sketch::DocumentEdit::CreateDimension {
                label: "conflict".into(),
                definition: geosolve_sketch::DocumentDimensionDefinition::PointDistance {
                    first: a,
                    second: b,
                    target: scalar,
                },
                mode: DocumentDimensionMode::Driving,
            },
        )
        .map_err(|e| e.to_string())?;
    let retained = coordinator
        .session()
        .accepted_state()
        .ok_or("rejection lost accepted state")?;
    let retained_scene = EditorScene::from_accepted(
        retained.identity().revision().get(),
        retained.design_identity(),
        retained.document(),
        viewport,
        0.5,
    )
    .map_err(|e| e.to_string())?;
    let problems = coordinator.problems();
    if coordinator.lifecycle().status != LifecycleStatus::RejectedAttempt
        || coordinator
            .session()
            .export_accepted_json()
            .map_err(|e| e.to_string())?
            != accepted
        || coordinator
            .session()
            .accepted_state()
            .is_none_or(|state| state.identity() != accepted_identity)
        || retained_scene != accepted_scene
        || problems.rejection.is_none()
        || problems.design != coordinator.session().design_identity()
        || problems.parent_accepted != Some(accepted_identity)
    {
        return Err("conflict did not retain accepted JSON/identity/scene and Problems DTO".into());
    }
    Ok("RejectedAttempt:accepted-json=same:accepted-identity=same:accepted-scene=same:problems=rejection-linked".into())
}

#[allow(clippy::too_many_lines)]
fn lifecycle_matrix(input: &Value) -> Result<String, String> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Input {
        states: Vec<String>,
    }
    if parse_input::<Input>(input)?.states
        != [
            "accepted",
            "design-unsolved",
            "solving",
            "solved-preview",
            "rejected-attempt",
        ]
    {
        return Err("unexpected lifecycle state matrix".into());
    }
    let (mut primary, _, _) = coordinator()?;
    if primary.lifecycle().status != LifecycleStatus::Accepted {
        return Err("accepted lifecycle state was not derived from an accepted attempt".into());
    }
    let mut preview = primary.session().clone();
    preview
        .reattempt(
            preview.design_identity(),
            preview.last_attempt().input().candidate_request(),
        )
        .map_err(|e| e.to_string())?;
    primary
        .mark_solved_preview(&preview)
        .map_err(|e| e.to_string())?;
    if primary.lifecycle().status != LifecycleStatus::SolvedPreview
        || primary.lifecycle().preview_attempt != Some(preview.last_attempt().identity())
        || primary.lifecycle().preview_accepted
            != preview
                .accepted_state()
                .map(geosolve_sketch::SketchAcceptedDocumentState::identity)
    {
        return Err("solved-preview lifecycle/provenance mismatch".into());
    }
    primary.mark_solving();
    if primary.lifecycle().status != LifecycleStatus::Solving
        || primary.lifecycle().preview_attempt.is_some()
        || primary.lifecycle().preview_accepted.is_some()
    {
        return Err("solving lifecycle fabricated preview provenance".into());
    }
    primary.clear_transient();
    if primary.lifecycle().status != LifecycleStatus::Accepted {
        return Err("clearing transient lifecycle did not reveal accepted state".into());
    }

    let mut unsolved_document = SketchDocument::new(4.0).map_err(|e| e.to_string())?;
    let point = unsolved_document
        .add_point("conflicted", [0.0, 0.0])
        .map_err(|e| e.to_string())?;
    for target in [[0.0, 0.0], [1.0, 0.0]] {
        unsolved_document
            .add_constraint(
                "conflicting fixed point",
                DocumentConstraintDefinition::FixedPoint { point, target },
            )
            .map_err(|e| e.to_string())?;
    }
    let unsolved = RetainedEditorCoordinator::new(
        RetainedSketchDocumentSession::new(
            unsolved_document,
            DocumentSolveRequest::default(),
            Default::default(),
        )
        .map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?
    .lifecycle();
    if unsolved.status != LifecycleStatus::DesignUnsolved
        || unsolved.accepted.is_some()
        || unsolved.parent_accepted.is_some()
    {
        return Err("design-unsolved lifecycle retained nonexistent accepted provenance".into());
    }

    let mut rejected_document = SketchDocument::new(4.0).map_err(|e| e.to_string())?;
    let a = rejected_document
        .add_point("a", [0.0, 0.0])
        .map_err(|e| e.to_string())?;
    let b = rejected_document
        .add_point("b", [2.0, 0.0])
        .map_err(|e| e.to_string())?;
    for (point, target) in [(a, [0.0, 0.0]), (b, [2.0, 0.0])] {
        rejected_document
            .add_constraint(
                "fix",
                DocumentConstraintDefinition::FixedPoint { point, target },
            )
            .map_err(|e| e.to_string())?;
    }
    let mut rejected = RetainedEditorCoordinator::new(
        RetainedSketchDocumentSession::new(
            rejected_document,
            DocumentSolveRequest::default(),
            Default::default(),
        )
        .map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let scalar_outcome = rejected
        .apply_edit(
            rejected.session().design_identity(),
            geosolve_sketch::DocumentEdit::CreateScalar {
                label: "bad distance".into(),
                value: 3.0,
                unit: ScalarUnit::Length,
                domain: ScalarDomain::Positive,
            },
        )
        .map_err(|e| e.to_string())?;
    let DocumentCommandEffect::CreatedScalar(scalar) = scalar_outcome.value else {
        return Err("lifecycle scalar creation returned wrong effect".into());
    };
    let parent = rejected
        .session()
        .accepted_state()
        .ok_or("accepted scalar edit missing state")?
        .identity();
    rejected
        .apply_edit(
            scalar_outcome.design,
            geosolve_sketch::DocumentEdit::CreateDimension {
                label: "conflict".into(),
                definition: geosolve_sketch::DocumentDimensionDefinition::PointDistance {
                    first: a,
                    second: b,
                    target: scalar,
                },
                mode: DocumentDimensionMode::Driving,
            },
        )
        .map_err(|e| e.to_string())?;
    let rejected_lifecycle = rejected.lifecycle();
    if rejected_lifecycle.status != LifecycleStatus::RejectedAttempt
        || rejected_lifecycle.accepted != Some(parent)
        || rejected_lifecycle.parent_accepted != Some(parent)
        || rejected.problems().rejection.is_none()
    {
        return Err("rejected-attempt lifecycle/provenance mismatch".into());
    }
    Ok("Accepted:accepted-provenance|DesignUnsolved:no-accepted-provenance|Solving:no-preview-provenance|SolvedPreview:independent-preview-provenance|RejectedAttempt:parent-accepted-provenance".into())
}

fn malformed_matrix(input: &Value) -> Result<String, String> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Input {
        values: Vec<String>,
    }
    if parse_input::<Input>(input)?.values
        != [
            "nan",
            "inf",
            "zero-viewport",
            "pointercancel",
            "tool-cancel",
            "malformed-storage",
        ]
    {
        return Err("unexpected malformed input matrix".into());
    }
    let invalid = [
        Viewport::new([0.0, 1.0], [0.0, 0.0], 1.0).is_err(),
        Viewport::new([1.0, 1.0], [f64::NAN, 0.0], 1.0).is_err(),
        Viewport::new([1.0, 1.0], [0.0, 0.0], f64::INFINITY).is_err(),
    ];
    let (_, scene, _, _) = fixture()?;
    let mut editor = ConstraintEditor::default();
    let nan = editor.pointer_down(&scene, pointer(1, f64::NAN, 0.0, Modifiers::default()));
    let infinite = editor.pointer_down(
        &scene,
        pointer(2, 500.0, f64::INFINITY, Modifiers::default()),
    );
    if !invalid.into_iter().all(|value| value)
        || !nan.is_empty()
        || !infinite.is_empty()
        || !editor.selection().is_empty()
    {
        return Err("malformed input changed editor state".into());
    }

    editor.activate_tool(EditorTool::Line);
    editor.pointer_down(&scene, pointer(3, 450.0, 350.0, Modifiers::default()));
    let pointer_cancel = editor.cancel();
    if pointer_cancel != [EditorEffect::ClearConstructionPreview]
        || !editor
            .pointer_up(
                &scene,
                scene.design_identity,
                pointer(3, 550.0, 350.0, Modifiers::default()),
            )
            .is_empty()
    {
        return Err("pointercancel semantic alias did not clear draft/no-op release".into());
    }
    editor.activate_tool(EditorTool::Circle);
    editor.pointer_down(&scene, pointer(4, 450.0, 350.0, Modifiers::default()));
    if editor.cancel() != [EditorEffect::ClearConstructionPreview]
        || !editor
            .pointer_down(
                &scene,
                pointer(5, f64::NAN, f64::INFINITY, Modifiers::default()),
            )
            .is_empty()
    {
        return Err("tool cancel or later malformed event changed draft state".into());
    }
    Ok("nan:pointer-ignored|inf:pointer-ignored|zero-viewport:rejected|pointercancel:semantic-cancel-no-commit|tool-cancel:clear-no-commit|malformed-storage:adapter-owned:not-core-executed".into())
}

#[allow(clippy::too_many_lines)]
fn seeded_model(input: &Value, mut state: u64) -> Result<String, String> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Input {
        steps: u64,
        max_selection: usize,
        classes: Vec<String>,
    }
    let Input {
        steps,
        max_selection,
        classes,
    } = parse_input(input)?;
    if max_selection != 2 {
        return Err("unexpected model selection bound".into());
    }
    let required = [
        "creation",
        "snap-pick-selection",
        "constraints",
        "dimensions",
        "projected-drag",
        "delete-suppress-history-persistence",
        "conflict-rejection-repair",
        "lifecycle-accepted-retention",
        "redundancy",
        "boundaries",
    ];
    if classes.iter().map(String::as_str).collect::<Vec<_>>() != required {
        return Err("unexpected generated/model class vocabulary".into());
    }
    let mut schedule = classes;
    for index in (1..schedule.len()).rev() {
        state = next_model_state(state);
        let upper = u64::try_from(index + 1)
            .map_err(|error| format!("model schedule bound conversion failed: {error}"))?;
        let selected = usize::try_from(state % upper)
            .map_err(|error| format!("model schedule index conversion failed: {error}"))?;
        schedule.swap(index, selected);
    }
    let mut class_trace = Vec::new();
    for class in &schedule {
        let result = model_class(class)?;
        class_trace.push(format!("{class}:{}", digest(result.as_bytes())));
    }
    let (_, scene, _, spans) = fixture()?;
    let mut editor = ConstraintEditor::default();
    let mut expected = Vec::<SelectionItem>::new();
    let mut counts = [0_u64; 3];
    for index in 0..steps {
        state = next_model_state(state);
        match state % 3 {
            0 => {
                let span_index = usize::try_from((state >> 8) & 1)
                    .map_err(|error| format!("seeded span index conversion failed: {error}"))?;
                editor.set_selection([SelectionItem::Curve(spans[span_index])]);
                expected.clear();
                expected.push(SelectionItem::Curve(spans[span_index]));
                counts[0] += 1;
            }
            1 => {
                editor.cancel();
                counts[1] += 1;
            }
            _ => {
                let offset = u8::try_from(state % 20)
                    .map_err(|error| format!("seeded pointer offset conversion failed: {error}"))?;
                let x = 500.0 + f64::from(offset);
                let modifiers = Modifiers {
                    shift: state & 8 != 0,
                    control: state & 16 != 0,
                    command: state & 32 != 0,
                };
                editor.pointer_down(&scene, pointer(index, x, 300.0, modifiers));
                editor.pointer_up(
                    &scene,
                    scene.design_identity,
                    pointer(index, x, 300.0, modifiers),
                );
                let selected = SelectionItem::Curve(spans[0]);
                if modifiers.shift || modifiers.control || modifiers.command {
                    if let Some(selected_index) = expected.iter().position(|item| *item == selected)
                    {
                        expected.remove(selected_index);
                    } else {
                        expected.push(selected);
                    }
                } else {
                    expected.clear();
                    expected.push(selected);
                }
                counts[2] += 1;
            }
        }
        if editor.selection() != expected.as_slice() || editor.selection().len() > max_selection {
            return Err(format!(
                "seeded reference model diverged at step {index} for state {state:016x}"
            ));
        }
    }
    let selection = editor
        .selection()
        .iter()
        .map(|item| match item {
            SelectionItem::Point(_) => "point",
            SelectionItem::Curve(_) => "curve",
            SelectionItem::Constraint(_) => "constraint",
            SelectionItem::Dimension(_) => "dimension",
            SelectionItem::Feature(_) => "feature",
            SelectionItem::FeatureCorner(_) => "feature-corner",
        })
        .collect::<Vec<_>>()
        .join(",");
    Ok(format!(
        "classes=[{}]|selection={steps}:{state:016x}:{counts:?}:{selection}",
        class_trace.join(",")
    ))
}

fn next_model_state(mut state: u64) -> u64 {
    state ^= state << 13;
    state ^= state >> 7;
    state ^ (state << 17)
}

#[allow(clippy::too_many_lines)]
fn model_class(class: &str) -> Result<String, String> {
    match class {
        "creation" => {
            let routes = draft_matrix(&serde_json::json!({
                "tools": ["point", "line", "polyline", "rectangle", "circle", "arc"],
                "routes": ["pointer", "button", "double-click", "enter", "escape", "pointercancel"]
            }))?;
            Ok(format!("{routes}|{}", retained_construction_model()?))
        }
        "snap-pick-selection" => Ok(format!(
            "{}|{}|{}",
            snap_matrix(&serde_json::json!({
                "offsets": [7.999, 8.0, 8.001],
                "tie_policy": "persistent-identity"
            }))?,
            pick_matrix(&serde_json::json!({
                "curve_offsets": [11.999, 12.0, 12.001],
                "overlaps": ["point-curve", "curve-curve"]
            }))?,
            selection_matrix(&serde_json::json!({
                "modifiers": ["shift", "control", "command"],
                "origins": ["canvas", "tree", "inspector"]
            }))?
        )),
        "constraints" => constraint_matrix(&serde_json::json!({
            "kinds": ["fixed", "coincident", "horizontal", "vertical", "parallel", "perpendicular", "equal-length"]
        })),
        "dimensions" => dimension_matrix(&serde_json::json!({
            "families": ["point-distance", "segment-length"],
            "modes": ["driving", "reference"],
            "transitions": ["mode", "undo", "redo", "reload"]
        })),
        "projected-drag" => drag_matrix(&serde_json::json!({
            "threshold": [2.999, 3.0, 3.001],
            "results": ["foreign", "stale", "accepted", "rejected"],
            "release_commits": 1
        })),
        "delete-suppress-history-persistence" => history_matrix(&serde_json::json!({
            "actions": ["delete", "suppress", "undo", "redo", "reload"],
            "retain_ids": true
        })),
        "conflict-rejection-repair" => Ok(format!(
            "{}|{}",
            conflict_retention(&serde_json::json!({
                "retain": ["accepted-json", "accepted-identity", "scene"]
            }))?,
            conflict_repair_model()?
        )),
        "lifecycle-accepted-retention" => lifecycle_matrix(&serde_json::json!({
            "states": ["accepted", "design-unsolved", "solving", "solved-preview", "rejected-attempt"]
        })),
        "redundancy" => redundancy(&serde_json::json!({"source": "sketch-owned-dto"})),
        "boundaries" => Ok(format!(
            "{}|{}",
            viewport_matrix(&serde_json::json!({
                "scales": [0.000_001, 1.0, 1_000_000.0],
                "centers": [[0.0, 0.0], [1_000_000.0, -1_000_000.0]]
            }))?,
            malformed_matrix(&serde_json::json!({
                "values": ["nan", "inf", "zero-viewport", "pointercancel", "tool-cancel", "malformed-storage"]
            }))?
        )),
        _ => Err(format!("unknown generated/model class {class}")),
    }
}

fn retained_construction_model() -> Result<String, String> {
    let document = SketchDocument::new(4.0).map_err(|error| error.to_string())?;
    let session = RetainedSketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        Default::default(),
    )
    .map_err(|error| error.to_string())?;
    let mut coordinator =
        RetainedEditorCoordinator::new(session).map_err(|error| error.to_string())?;
    let proposals = [
        ConstructionProposal::Point {
            position: [-8.0, -8.0],
        },
        ConstructionProposal::Line {
            start: ConstructionPoint::New([-6.0, -6.0]),
            end: ConstructionPoint::New([-4.0, -6.0]),
        },
        ConstructionProposal::Polyline {
            points: vec![
                ConstructionPoint::New([-2.0, -6.0]),
                ConstructionPoint::New([0.0, -6.0]),
                ConstructionPoint::New([0.0, -4.0]),
            ],
        },
        ConstructionProposal::Rectangle {
            first: [2.0, -6.0],
            second: [4.0, -4.0],
        },
        ConstructionProposal::Circle {
            center: ConstructionPoint::New([6.0, -5.0]),
            radius: 1.0,
        },
        ConstructionProposal::CounterClockwiseArc {
            center: ConstructionPoint::New([8.0, -5.0]),
            start: [9.0, -5.0],
            end: [8.0, -4.0],
        },
    ];
    for proposal in &proposals {
        let outcome = coordinator
            .apply_construction(coordinator.session().design_identity(), proposal)
            .map_err(|error| error.to_string())?;
        if outcome.published_accepted.is_none()
            || coordinator.lifecycle().status != LifecycleStatus::Accepted
        {
            return Err(
                "retained construction did not publish independently validated accepted state"
                    .into(),
            );
        }
    }
    let checkpoint = coordinator.checkpoint().clone();
    coordinator
        .reload(&checkpoint)
        .map_err(|error| error.to_string())?;
    if coordinator.checkpoint().design_json() != checkpoint.design_json() {
        return Err("retained construction reload changed canonical design".into());
    }
    Ok("point-line-polyline-rectangle-circle-arc:accepted:persisted-reloaded".into())
}

fn conflict_repair_model() -> Result<String, String> {
    let mut document = SketchDocument::new(4.0).map_err(|error| error.to_string())?;
    let first = document
        .add_point("first", [0.0, 0.0])
        .map_err(|error| error.to_string())?;
    let second = document
        .add_point("second", [2.0, 0.0])
        .map_err(|error| error.to_string())?;
    for (point, target) in [(first, [0.0, 0.0]), (second, [2.0, 0.0])] {
        document
            .add_constraint(
                "fixed",
                DocumentConstraintDefinition::FixedPoint { point, target },
            )
            .map_err(|error| error.to_string())?;
    }
    let scalar = document
        .add_scalar("conflict", 3.0, ScalarUnit::Length, ScalarDomain::Positive)
        .map_err(|error| error.to_string())?;
    let session = RetainedSketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        Default::default(),
    )
    .map_err(|error| error.to_string())?;
    let mut coordinator =
        RetainedEditorCoordinator::new(session).map_err(|error| error.to_string())?;
    let retained = coordinator
        .session()
        .accepted_state()
        .ok_or("repair model lacks baseline accepted state")?
        .identity();
    let outcome = coordinator
        .apply_edit(
            coordinator.session().design_identity(),
            geosolve_sketch::DocumentEdit::CreateDimension {
                label: "conflict".into(),
                definition: geosolve_sketch::DocumentDimensionDefinition::PointDistance {
                    first,
                    second,
                    target: scalar,
                },
                mode: DocumentDimensionMode::Driving,
            },
        )
        .map_err(|error| error.to_string())?;
    let DocumentCommandEffect::CreatedDimension(dimension) = outcome.value else {
        return Err("repair model dimension returned wrong effect".into());
    };
    if outcome.published_accepted.is_some()
        || coordinator.lifecycle().status != LifecycleStatus::RejectedAttempt
        || coordinator
            .session()
            .accepted_state()
            .is_none_or(|state| state.identity() != retained)
    {
        return Err("repair model did not retain accepted state during rejection".into());
    }
    coordinator
        .editor_mut()
        .set_selection([SelectionItem::Dimension(dimension)]);
    let repair = coordinator
        .set_selected_suppressed(coordinator.session().design_identity(), true)
        .map_err(|error| error.to_string())?;
    if repair.published_accepted.is_none()
        || coordinator.lifecycle().status != LifecycleStatus::Accepted
        || coordinator
            .session()
            .accepted_state()
            .is_none_or(|state| state.identity() == retained)
    {
        return Err("suppression repair did not publish a new accepted state".into());
    }
    Ok("rejected:retained|suppress-repair:new-accepted".into())
}

fn digest(bytes: &[u8]) -> String {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("fnv1a64-{hash:016x}")
}

fn parse_corpus(bytes: &[u8]) -> Result<Corpus, String> {
    let corpus: Corpus = serde_json::from_slice(bytes)
        .map_err(|error| format!("corpus JSON/schema error: {error}"))?;
    if corpus.schema_version != 1 {
        return Err(format!(
            "unsupported schema version {}",
            corpus.schema_version
        ));
    }
    if corpus.cases.is_empty() {
        return Err("corpus has no cases".into());
    }
    Ok(corpus)
}

#[cfg(test)]
mod tests {
    use super::{parse_corpus, run_m40_qualification, validate_m40_qualification_matrix};

    #[test]
    fn m40_transition_corpus_passes_the_native_oracle() {
        let report = run_m40_qualification();
        assert!(report.passed, "{}", report.canonical_json());
    }

    #[test]
    fn m40_transition_report_matches_the_canonical_golden_bytes() {
        let report = run_m40_qualification();
        assert!(report.passed, "{}", report.canonical_json());
        assert_eq!(
            report.canonical_json().as_bytes(),
            include_bytes!("../tests/m40_qualification_report.golden.json"),
        );
    }

    #[test]
    fn corpus_rejects_malformed_and_unknown_schema_fields_clearly() {
        assert!(parse_corpus(br#"{"schema_version":1,"seed":1,"cases":[]}"#).is_err());
        let error = parse_corpus(br#"{"schema_version":1,"seed":1,"cases":[{"id":"x","operation":"viewport_matrix","input":{},"expected_digest":"x","extra":true}]}"#)
            .expect_err("unknown corpus field must be rejected");
        assert!(error.contains("unknown field `extra`"), "{error}");
    }

    #[test]
    fn qualification_matrix_is_complete_and_uses_only_frozen_evidence_ids() {
        validate_m40_qualification_matrix().expect("qualification matrix must validate");
    }
}
