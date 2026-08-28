// SPDX-License-Identifier: GPL-3.0-or-later
//! Optional M84 code-project composition for the demonstration workbench.
//!
//! This module deliberately owns presentation and caller-facing project
//! composition only. Managed parsing, artifact validation, semantic generated
//! identity, overrides and unified code history remain public
//! `geosolve-sketch-code` responsibilities. No source is evaluated in the
//! browser and this adapter contains no solver equations.

#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::sync::atomic::{AtomicU64, Ordering};

use geosolve_constraint_editor::{
    BOOTSTRAP_DOCUMENT_HEADER_CODEC_V1, ComputedFeatureDefinition, ComputedFeatureDocument,
    ComputedFeatureEvaluationState, ComputedFeatureSnapshot, IntentInspectorEditTarget,
    IntentInspectorEditValue, IntentInspectorProjection, IntentNativeBinding,
    ProjectionalEditorSession, SelectionItem,
};
use geosolve_sketch::{DocumentId, PersistentId};
use geosolve_sketch_code::{
    AuditedCodeWork, CodeGeneratedChildAddress, CodeInteractionOverlay, CodePointEdit, CodeProject,
    CodeProjectDemo, CodeProjectDemoId, CodeRectangleCorner, CodeSessionIdentity,
    CodeSessionReceipt, CodeWorkReceipt, CodeWritableAddress, EditorBootstrapDeclaration,
    ExpandedCodeProject, ExpandedPort, ExpandedSemanticTarget, ExpandedWritablePoint,
    KeyedReconcileState, ManagedDiagnostic, ManagedEdit, ManagedValue, MaterializedCodeProject,
    PatchModuleArtifact, ProjectKey, SemanticSymbol, SketchCodeSession, UnitLiteral,
    apply_managed_edit, bundled_code_project_demos, expand_code_project_for_structural_edit,
    initialize_code_project_from_editor, materialize_code_project_cold,
    materialize_code_project_incremental_for_structural_edit,
    materialize_code_project_incremental_with_overlay,
    materialize_code_project_incremental_with_overlay_audited, parse_managed_source,
    plan_managed_edit, rehydrate_materialized_code_project, required_generated_members,
};
use geosolve_sketch_intent::{
    BootstrapNativeKind, DimensionKind, GeometryRecipeKind, IntentLiteral, IntentNode,
    IntentNodeKind, IntentPortKind, IntentPortRole, IntentPortSelector, IntentSessionId,
    IntentUnit, LeafField, LeafRef, NodeId,
};
use serde::{Deserialize, Serialize};

#[cfg(test)]
use geosolve_sketch_code::{FeatureKind, GeneratedMemberAddress};
#[cfg(test)]
use geosolve_sketch_intent::IntentSession;

const MANAGED_FILE: &str = "sketch.ts";
const CODE_WORKBENCH_WIRE_VERSION: &str = "geosolve-code-workbench-v2";
const CODE_PROJECT_MODEL_SCALE: f64 = 1.0;
const AUTHORED_STARTER_SOURCE: &str = r#""use geosolve managed-v1";
import { sketch } from "@geosolve/sketch-code";

export default sketch(($) => {
  const frame = $.geometry.rectangle("frame", {
    lowerLeft: [0, 0],
    upperRight: [60, 35],
  });
  const diagonal = $.geometry.line("diagonal", {
    start: frame.corners.lowerLeft,
    end: frame.corners.upperRight,
  });
  $.organize("Code-authored frame", [frame, diagonal]);
  return $.outputs({ frame, diagonal });
});
"#;
static NEXT_CODE_MATERIALIZATION: AtomicU64 = AtomicU64::new(1);

/// One independently accepted replacement for the live projectional canvas.
/// The caller installs `editor` only after the code-session transaction has
/// published the matching opaque checkpoint.
pub(crate) struct AcceptedCodePublication {
    pub(crate) editor: Box<ProjectionalEditorSession>,
    pub(crate) receipt: CodeSessionReceipt,
}

/// Transient semantic disambiguation prepared before the first pointer frame.
/// It changes neither the code session nor outer history.
pub(crate) struct PreparedCodePointDrag {
    pub(crate) editor: Box<ProjectionalEditorSession>,
    pub(crate) point: geosolve_sketch::DesignPointId,
}

#[derive(Clone, Debug, PartialEq)]
struct PendingSemanticPointDrag {
    pointer_id: u64,
    session: CodeSessionIdentity,
    point: ExpandedWritablePoint,
    /// Exact native authority from which the authenticated point gesture
    /// began when a referenced consumer first needed local detachment.
    /// Producer gestures use the accepted code checkpoint directly.
    detached_origin_checkpoint: Option<serde_json::Value>,
}

/// A syntactically valid Apply either replaces native authority atomically or
/// retains its code diagnostic over the previous accepted editor checkpoint.
pub(crate) enum CodeApplyOutcome {
    Accepted(AcceptedCodePublication),
    RetainedFailure {
        receipt: CodeSessionReceipt,
        diagnostic: String,
    },
}

/// Closed ownership result for one already decoded Inspector edit. Only the
/// authenticated managed-dimension target route is claimed here; ordinary GUI
/// declarations and existing semantic point routes remain with their current
/// projectional/code-owned classifiers.
pub(crate) enum CodeInspectorEditRoute {
    NotClaimed,
    Claimed(CodeApplyOutcome),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum CodeDeleteTarget {
    ManagedDeclaration {
        declaration: SemanticSymbol,
        alias: geosolve_sketch_intent::IntentKey,
        session: CodeSessionIdentity,
    },
    GeneratedChild {
        address: CodeGeneratedChildAddress,
        alias: geosolve_sketch_intent::IntentKey,
        session: CodeSessionIdentity,
    },
}

#[derive(Clone, Debug, PartialEq)]
enum CodeOwnedEditorChange {
    SemanticPoints {
        placements: Vec<(ExpandedWritablePoint, [f64; 2])>,
    },
}

#[derive(Clone, Debug)]
struct RectangleTerminalProjection {
    anchors: [ExpandedPort; 2],
    redundant_aliases: [ExpandedPort; 2],
}

#[derive(Clone, Debug)]
struct CanonicalTerminalPointBundle {
    placements: Vec<(ExpandedWritablePoint, [f64; 2])>,
    rectangle_projections: Vec<RectangleTerminalProjection>,
}

const TERMINAL_SEED_ROUNDOFF_ULPS: u64 = 8;
const TERMINAL_SEED_ZERO_ROUNDOFF: f64 = 32.0 * f64::EPSILON;
const F64_SIGN_MASK: u64 = 0x8000_0000_0000_0000;

fn rectangle_corner(edit: &CodePointEdit) -> Option<CodeRectangleCorner> {
    match edit {
        CodePointEdit::RectangleCorner { corner, .. } => Some(*corner),
        CodePointEdit::Point { .. } => None,
    }
}

const fn opposite_rectangle_corner(corner: CodeRectangleCorner) -> CodeRectangleCorner {
    match corner {
        CodeRectangleCorner::LowerLeft => CodeRectangleCorner::UpperRight,
        CodeRectangleCorner::LowerRight => CodeRectangleCorner::UpperLeft,
        CodeRectangleCorner::UpperRight => CodeRectangleCorner::LowerLeft,
        CodeRectangleCorner::UpperLeft => CodeRectangleCorner::LowerRight,
    }
}

fn rectangle_lenses<'a>(
    expansion: &'a ExpandedCodeProject,
    key: &(CodeWritableAddress, CodeWritableAddress),
) -> Result<BTreeMap<CodeRectangleCorner, &'a ExpandedWritablePoint>, String> {
    if key.0 == key.1 {
        return Err("rectangle semantic seed codec aliases both seed addresses".into());
    }
    let mut lenses = BTreeMap::new();
    let mut effective = None;
    for point in &expansion.writable_points {
        match &point.edit {
            CodePointEdit::Point { address } if address == &key.0 || address == &key.1 => {
                return Err(
                    "rectangle semantic seed address collides with an ordinary point codec".into(),
                );
            }
            CodePointEdit::RectangleCorner {
                lower_left,
                upper_right,
                corner,
                effective_lower_left,
                effective_upper_right,
            } if lower_left == &key.0 && upper_right == &key.1 => {
                let seed_bits = (
                    pair_bits(*effective_lower_left),
                    pair_bits(*effective_upper_right),
                );
                if effective.is_some_and(|expected| expected != seed_bits) {
                    return Err(
                        "rectangle semantic corner codecs disagree on effective seeds".into(),
                    );
                }
                effective = Some(seed_bits);
                if lenses.insert(*corner, point).is_some() {
                    return Err("rectangle semantic seed group has a duplicate corner codec".into());
                }
            }
            CodePointEdit::RectangleCorner {
                lower_left,
                upper_right,
                ..
            } if [lower_left, upper_right]
                .into_iter()
                .any(|address| address == &key.0 || address == &key.1) =>
            {
                return Err("rectangle semantic seed groups partially overlap".into());
            }
            CodePointEdit::Point { .. } | CodePointEdit::RectangleCorner { .. } => {}
        }
    }
    if lenses.len() != 4 {
        return Err("rectangle semantic seed group has no complete corner codec".into());
    }
    Ok(lenses)
}

fn canonical_rectangle_seeds(
    first_corner: CodeRectangleCorner,
    first_target: [f64; 2],
    second_corner: CodeRectangleCorner,
    second_target: [f64; 2],
) -> Result<([f64; 2], [f64; 2]), String> {
    let mut lower = [None, None];
    let mut upper = [None, None];
    for (corner, target) in [(first_corner, first_target), (second_corner, second_target)] {
        match corner {
            CodeRectangleCorner::LowerLeft => lower = target.map(Some),
            CodeRectangleCorner::LowerRight => {
                upper[0] = Some(target[0]);
                lower[1] = Some(target[1]);
            }
            CodeRectangleCorner::UpperRight => upper = target.map(Some),
            CodeRectangleCorner::UpperLeft => {
                lower[0] = Some(target[0]);
                upper[1] = Some(target[1]);
            }
        }
    }
    let complete = |seed: [Option<f64>; 2]| {
        Some([seed[0]?, seed[1]?]).filter(|seed| seed.iter().all(|value| value.is_finite()))
    };
    let lower = complete(lower)
        .ok_or_else(|| "canonical rectangle lenses do not cover the lower seed".to_owned())?;
    let upper = complete(upper)
        .ok_or_else(|| "canonical rectangle lenses do not cover the upper seed".to_owned())?;
    Ok((lower, upper))
}

const fn rectangle_corner_position(
    lower: [f64; 2],
    upper: [f64; 2],
    corner: CodeRectangleCorner,
) -> [f64; 2] {
    match corner {
        CodeRectangleCorner::LowerLeft => lower,
        CodeRectangleCorner::LowerRight => [upper[0], lower[1]],
        CodeRectangleCorner::UpperRight => upper,
        CodeRectangleCorner::UpperLeft => [lower[0], upper[1]],
    }
}

fn point_seed_roundoff_compatible(left: [f64; 2], right: [f64; 2]) -> bool {
    left.into_iter()
        .zip(right)
        .all(|(left, right)| scalar_seed_roundoff_compatible(left, right))
}

fn scalar_seed_roundoff_compatible(left: f64, right: f64) -> bool {
    if !left.is_finite() || !right.is_finite() {
        return false;
    }
    let left = left.to_bits();
    let right = right.to_bits();
    if left == right {
        return true;
    }
    if left & !F64_SIGN_MASK == 0 && right & !F64_SIGN_MASK == 0 {
        return false;
    }
    (left & F64_SIGN_MASK == right & F64_SIGN_MASK
        && left.abs_diff(right) <= TERMINAL_SEED_ROUNDOFF_ULPS)
        || (f64::from_bits(left).abs() <= TERMINAL_SEED_ZERO_ROUNDOFF
            && f64::from_bits(right).abs() <= TERMINAL_SEED_ZERO_ROUNDOFF)
}

fn select_semantic_point_drag_lens(
    expansion: &ExpandedCodeProject,
    candidates: &[ExpandedWritablePoint],
    preferred_declaration: Option<&SemanticSymbol>,
) -> Result<Option<ExpandedWritablePoint>, String> {
    if candidates.is_empty() {
        return Ok(None);
    }
    let preferred = preferred_declaration.map_or_else(Vec::new, |preferred_declaration| {
        candidates
            .iter()
            .filter(|candidate| {
                expansion.declaration_for_alias(&candidate.handle.alias)
                    == Some(preferred_declaration)
            })
            .cloned()
            .collect::<Vec<_>>()
    });
    match preferred.as_slice() {
        [point] => Ok(Some(point.clone())),
        [] => {
            let producers = candidates
                .iter()
                .filter(|candidate| !candidate.source.is_reference())
                .cloned()
                .collect::<Vec<_>>();
            match producers.as_slice() {
                [point] => Ok(Some(point.clone())),
                [] if candidates.len() == 1 => Ok(Some(candidates[0].clone())),
                [] => {
                    Err("this shared code-owned point has no unique producer semantic lens".into())
                }
                _ => {
                    Err("this shared code-owned point has multiple producer semantic lenses".into())
                }
            }
        }
        _ => Err(
            "the selected declaration has multiple semantic point lenses at this shared point"
                .into(),
        ),
    }
}

#[cfg(test)]
#[derive(Clone, Debug, Eq, PartialEq)]
enum WritableCodeLeaf {
    GeneratedPoint(GeneratedMemberAddress),
    ManagedRectangle {
        declaration: SemanticSymbol,
        alias: geosolve_sketch_intent::IntentKey,
        argument: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum SelectedCodeFile {
    Managed,
    Custom(String),
}

impl SelectedCodeFile {
    fn path(&self) -> &str {
        match self {
            Self::Managed => MANAGED_FILE,
            Self::Custom(path) => path,
        }
    }
}

/// One genuine optional code-project session composed beside the ordinary
/// projectional editor. The project and code session are authoritative; the
/// selected file and invalid text draft are presentation state only.
pub(crate) struct CodeProjectWorkbench {
    origin: CodeProjectOrigin,
    project: CodeProject,
    session: SketchCodeSession,
    selected_file: SelectedCodeFile,
    managed_draft: String,
    draft_diagnostic: Option<ManagedDiagnostic>,
    last_receipt: Option<CodeSessionReceipt>,
    // Reconstructible warm authority. Persistence stores the delegated editor
    // checkpoint plus authenticated expansion, never this runtime cache.
    materialized: Option<Box<MaterializedCodeProject>>,
    // A referenced consumer may need to be detached before native pointer
    // continuation starts. This is disposable gesture state and is never
    // serialized or entered into history unless the terminal sample commits.
    pending_semantic_point_drag: Option<PendingSemanticPointDrag>,
}

/// Presentation provenance for one genuine code project. Bundled projects
/// retain their curated sample identity; GUI-promoted projects deliberately
/// have no fake sample key.
#[derive(Clone, Debug)]
enum CodeProjectOrigin {
    Bundled(CodeProjectDemo),
    Authored,
    Promoted,
}

impl CodeProjectOrigin {
    fn title(&self) -> &'static str {
        match self {
            Self::Bundled(demo) => demo.title,
            Self::Authored => "Untitled code sketch",
            Self::Promoted => "Promoted sketch",
        }
    }

    fn demo_key(&self) -> Option<&'static str> {
        match self {
            Self::Bundled(demo) => Some(demo.id.key()),
            Self::Authored | Self::Promoted => None,
        }
    }

    fn to_wire(&self) -> CodeProjectOriginWire {
        match self {
            Self::Bundled(demo) => CodeProjectOriginWire::Bundled {
                demo: demo.id.key().into(),
            },
            Self::Authored => CodeProjectOriginWire::Authored,
            Self::Promoted => CodeProjectOriginWire::Promoted,
        }
    }
}

#[derive(Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum CodeProjectOriginWire {
    Bundled { demo: String },
    Authored,
    Promoted,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct CodeProjectWorkbenchWire {
    version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    origin: Option<CodeProjectOriginWire>,
    /// Historical nominated-M84 wire accepted only on input. New persistence
    /// always writes the explicit bounded `origin` variant.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    demo: Option<String>,
    project: String,
    session: String,
    selected_file: String,
    managed_draft: String,
}

fn restore_code_project_origin(
    origin: Option<CodeProjectOriginWire>,
    legacy_demo: Option<&str>,
) -> Result<CodeProjectOrigin, String> {
    let bundled = |key: &str| {
        bundled_code_project_demos()
            .into_iter()
            .find(|demo| demo.id.key() == key)
            .map(CodeProjectOrigin::Bundled)
            .ok_or_else(|| format!("unknown code project `{key}`"))
    };
    match (origin, legacy_demo) {
        (Some(CodeProjectOriginWire::Bundled { demo }), None) => bundled(&demo),
        (Some(CodeProjectOriginWire::Authored), None) => Ok(CodeProjectOrigin::Authored),
        (Some(CodeProjectOriginWire::Promoted), None) => Ok(CodeProjectOrigin::Promoted),
        (None, Some(demo)) => bundled(demo),
        (None, None) => Err("code-project persistence has no origin".into()),
        (Some(CodeProjectOriginWire::Bundled { .. }), Some(_)) => {
            Err("bundled code-project origin is duplicated".into())
        }
        (Some(CodeProjectOriginWire::Authored), Some(_)) => {
            Err("authored code-project origin cannot name a bundled demo".into())
        }
        (Some(CodeProjectOriginWire::Promoted), Some(_)) => {
            Err("promoted code-project origin cannot name a bundled demo".into())
        }
    }
}

impl CodeProjectWorkbench {
    /// Starts one standalone code-authored sketch without first manufacturing
    /// an ordinary GUI scene. The starter is intentionally artifact-free and
    /// uses lexical typed feature references, then enters the same cold-
    /// validated project/session path as every bundled or promoted project.
    pub(crate) fn new_authored() -> Result<(Self, Box<ProjectionalEditorSession>), String> {
        let project = CodeProject::managed_only(
            ProjectKey("code-authored-sketch".into()),
            AUTHORED_STARTER_SOURCE,
        )
        .map_err(|error| error.to_string())?;
        Self::open_project(CodeProjectOrigin::Authored, project)
    }

    pub(crate) fn open_key(key: &str) -> Result<(Self, Box<ProjectionalEditorSession>), String> {
        let demo = bundled_code_project_demos()
            .into_iter()
            .find(|demo| demo.id.key() == key)
            .ok_or_else(|| format!("unknown code project `{key}`"))?;
        let project = demo.project();
        Self::open_project(CodeProjectOrigin::Bundled(demo), project)
    }

    /// Builds a genuine optional code project from every declaration in one
    /// accepted ordinary workspace. This is intentionally all-or-nothing: a
    /// successful promotion cannot silently discard unsupported ordinary
    /// declarations or retain a second hidden GUI authority.
    pub(crate) fn promote_from_editor(
        editor: &ProjectionalEditorSession,
    ) -> Result<(Self, Box<ProjectionalEditorSession>), String> {
        let project = projected_code_project(editor)?;
        Self::open_project(CodeProjectOrigin::Promoted, project)
    }

    fn open_project(
        origin: CodeProjectOrigin,
        project: CodeProject,
    ) -> Result<(Self, Box<ProjectionalEditorSession>), String> {
        project.validate().map_err(|error| error.to_string())?;
        let desired = required_generated_members(&project).map_err(|error| error.to_string())?;
        let plan = KeyedReconcileState::empty()
            .plan(desired, &BTreeSet::new())
            .map_err(|error| error.to_string())?;
        let materialized = materialize_candidate(&project, plan.staged())?;
        let expansion = materialized.expansion.clone();
        let checkpoint = encode_editor_checkpoint(&materialized.editor)?;
        let delegated_editor = restore_editor_checkpoint(&checkpoint)?;
        let materialized = rehydrate_materialized_code_project(
            restore_editor_checkpoint(&checkpoint)?,
            expansion.clone(),
        )
        .map_err(|error| error.to_string())?;
        let session = SketchCodeSession::new_project(
            project.clone(),
            plan.into_staged(),
            expansion,
            checkpoint,
        )
        .map_err(|error| error.to_string())?;
        let managed_draft = project.managed.source.clone();
        Ok((
            Self {
                origin,
                project,
                session,
                selected_file: SelectedCodeFile::Managed,
                managed_draft,
                draft_diagnostic: None,
                last_receipt: None,
                materialized: Some(materialized),
                pending_semantic_point_drag: None,
            },
            delegated_editor,
        ))
    }

    pub(crate) fn to_persistence_json(&self) -> Result<String, String> {
        validate_managed_draft_bound(&self.managed_draft)?;
        let wire = CodeProjectWorkbenchWire {
            version: CODE_WORKBENCH_WIRE_VERSION.into(),
            origin: Some(self.origin.to_wire()),
            demo: None,
            project: self
                .project
                .to_canonical_json()
                .map_err(|error| error.to_string())?,
            session: self
                .session
                .to_canonical_json()
                .map_err(|error| error.to_string())?,
            selected_file: self.selected_file.path().into(),
            managed_draft: self.managed_draft.clone(),
        };
        let json = serde_json::to_string(&wire).map_err(|error| error.to_string())?;
        if json.len() > geosolve_sketch_code::CODE_PROJECT_LIMIT {
            return Err(format!(
                "code workbench is {} bytes; the limit is {}",
                json.len(),
                geosolve_sketch_code::CODE_PROJECT_LIMIT,
            ));
        }
        Ok(json)
    }

    pub(crate) fn from_persistence_json(json: &str) -> Result<Self, String> {
        if json.len() > geosolve_sketch_code::CODE_PROJECT_LIMIT {
            return Err(format!(
                "code workbench is {} bytes; the limit is {}",
                json.len(),
                geosolve_sketch_code::CODE_PROJECT_LIMIT,
            ));
        }
        let wire: CodeProjectWorkbenchWire =
            serde_json::from_str(json).map_err(|error| error.to_string())?;
        if wire.version != CODE_WORKBENCH_WIRE_VERSION {
            return Err("unsupported code-workbench version".into());
        }
        validate_managed_draft_bound(&wire.managed_draft)?;
        let origin = restore_code_project_origin(wire.origin, wire.demo.as_deref())?;
        let project = CodeProject::from_json(&wire.project).map_err(|error| error.to_string())?;
        let session = SketchCodeSession::from_json_validating_checkpoints(
            &wire.session,
            validate_editor_checkpoint,
        )
        .map_err(|error| error.to_string())?;
        if session.snapshot().project != project.project
            || session.snapshot().managed != project.managed
            || session.snapshot().code_project.as_ref() != Some(&project)
            || session.snapshot().artifact_digests != artifact_digests(&project)?
        {
            return Err("code project and unified session checkpoints disagree".into());
        }
        let snapshot = session.snapshot();
        let accepted_project = snapshot
            .accepted_code_project
            .as_ref()
            .ok_or_else(|| "code session has no accepted project authority".to_owned())?;
        let accepted_generated = snapshot
            .accepted_generated
            .as_ref()
            .ok_or_else(|| "code session has no accepted generated authority".to_owned())?;
        validate_generated_members(accepted_project, accepted_generated, "accepted")?;
        match required_generated_members(&project) {
            Ok(desired) => {
                let actual = snapshot
                    .generated
                    .ordered_members()
                    .into_iter()
                    .map(|member| member.address)
                    .collect::<Vec<_>>();
                if actual != desired {
                    return Err(
                        "current code-session generated provenance does not match managed source"
                            .into(),
                    );
                }
            }
            Err(error) if snapshot.failure.is_some() && snapshot.expansion.is_none() => {
                if snapshot
                    .failure
                    .as_ref()
                    .is_none_or(|failure| failure.diagnostic != error.to_string())
                {
                    return Err(
                        "retained structural diagnostic does not authenticate managed source"
                            .into(),
                    );
                }
            }
            Err(error) => return Err(error.to_string()),
        }
        validate_editor_checkpoint(session.pointer_frame_checkpoint())?;
        let draft_diagnostic = if wire.managed_draft == project.managed.source {
            None
        } else {
            parse_managed_source(&wire.managed_draft)
                .err()
                .map(|error| error.diagnostic)
        };
        let accepted_expansion = snapshot
            .accepted_expansion
            .clone()
            .ok_or_else(|| "code session has no accepted expansion authority".to_owned())?;
        let materialized = rehydrate_materialized_code_project(
            restore_editor_checkpoint(session.pointer_frame_checkpoint())?,
            accepted_expansion,
        )
        .map_err(|error| error.to_string())?;
        let mut value = Self {
            origin,
            project,
            session,
            selected_file: SelectedCodeFile::Managed,
            managed_draft: wire.managed_draft,
            draft_diagnostic,
            last_receipt: None,
            materialized: Some(materialized),
            pending_semantic_point_drag: None,
        };
        value.select_file(&wire.selected_file)?;
        Ok(value)
    }

    pub(crate) fn accepted_editor_checkpoint(&self) -> &serde_json::Value {
        &self.session.snapshot().accepted_editor_checkpoint
    }

    fn ensure_materialized_cache(&mut self) -> Result<(), String> {
        if self.materialized.is_some() {
            return Ok(());
        }
        let expansion = self
            .session
            .snapshot()
            .accepted_expansion
            .clone()
            .ok_or_else(|| "code project has no accepted expansion authority".to_owned())?;
        self.materialized = Some(rehydrate_editor_checkpoint(
            self.session.pointer_frame_checkpoint(),
            expansion,
        )?);
        Ok(())
    }

    /// Captures one already accepted direct GUI mutation in the outer code
    /// history. Identical checkpoints are presentation-only and create no
    /// duplicate Undo entry.
    pub(crate) fn publish_delegated_editor_checkpoint(
        &mut self,
        editor_checkpoint: serde_json::Value,
        label: &str,
    ) -> Result<Option<AcceptedCodePublication>, String> {
        if self.pending_semantic_point_drag.is_some() {
            return Err(
                "an authenticated semantic point gesture is pending; only its terminal pointer may publish it"
                    .into(),
            );
        }
        self.publish_delegated_editor_checkpoint_unchecked(editor_checkpoint, label)
    }

    /// Publishes one pointer-up checkpoint through the exact semantic route
    /// authenticated at pointer-down. A generic save, another pointer, or a
    /// route prepared against an older code-session identity cannot consume
    /// the token or turn a transient preview into durable authority.
    pub(crate) fn publish_pointer_terminal_checkpoint(
        &mut self,
        pointer_id: u64,
        editor_checkpoint: &serde_json::Value,
        label: &str,
    ) -> Result<Option<AcceptedCodePublication>, String> {
        let candidate_editor = restore_editor_checkpoint(editor_checkpoint)?;
        self.publish_pointer_terminal_editor(pointer_id, &candidate_editor, label)
    }

    /// Publishes an authenticated code-owned point terminal directly from the
    /// already accepted live editor.
    ///
    /// The browser owns that editor and has just completed exact pointer-up
    /// validation, so serializing and cold-restoring it merely to classify the
    /// same semantic delta would duplicate authority work. The terminal still
    /// rematerializes the staged overlay through the ordinary code/Intent/
    /// solver path and persists one independently restorable checkpoint.
    pub(crate) fn publish_pointer_terminal_editor(
        &mut self,
        pointer_id: u64,
        candidate_editor: &ProjectionalEditorSession,
        label: &str,
    ) -> Result<Option<AcceptedCodePublication>, String> {
        self.publish_pointer_terminal_editor_audited(pointer_id, candidate_editor, label)
            .into_outcome()
    }

    pub(crate) fn publish_pointer_terminal_editor_audited(
        &mut self,
        pointer_id: u64,
        candidate_editor: &ProjectionalEditorSession,
        label: &str,
    ) -> AuditedCodeWork<Result<Option<AcceptedCodePublication>, String>> {
        let mut work = CodeWorkReceipt::default();
        let outcome = self.publish_pointer_terminal_editor_with_work(
            pointer_id,
            candidate_editor,
            label,
            &mut work,
        );
        AuditedCodeWork { outcome, work }
    }

    fn publish_pointer_terminal_editor_with_work(
        &mut self,
        pointer_id: u64,
        candidate_editor: &ProjectionalEditorSession,
        label: &str,
        work: &mut CodeWorkReceipt,
    ) -> Result<Option<AcceptedCodePublication>, String> {
        let pending = self.pending_semantic_point_drag.as_ref().ok_or_else(|| {
            "semantic point terminal has no pending authenticated route".to_owned()
        })?;
        if pending.pointer_id != pointer_id {
            return Err("terminal pointer does not own the pending semantic point gesture".into());
        }
        if &pending.session != self.session.identity() {
            self.pending_semantic_point_drag = None;
            return Err(
                "semantic point gesture was invalidated by a newer code-session revision".into(),
            );
        }
        self.ensure_materialized_cache()?;
        let pending = self
            .pending_semantic_point_drag
            .take()
            .expect("the authenticated pending semantic drag was present");
        let detached_origin = pending
            .detached_origin_checkpoint
            .as_ref()
            .map(restore_editor_checkpoint)
            .transpose()?;
        let origin_editor = detached_origin.as_deref().unwrap_or_else(|| {
            &self
                .materialized
                .as_deref()
                .expect("the materialized cache was ensured above")
                .editor
        });
        let change = classify_code_owned_editor_change(
            origin_editor,
            candidate_editor,
            self.session
                .snapshot()
                .accepted_expansion
                .as_ref()
                .ok_or_else(|| "code project has no accepted expansion authority".to_owned())?,
        )?;
        let Some(CodeOwnedEditorChange::SemanticPoints { placements }) = change else {
            return Err(
                "terminal semantic point gesture has no authenticated placement bundle".into(),
            );
        };
        let bundle =
            self.canonical_terminal_point_bundle(&pending.point, placements, candidate_editor)?;
        self.publish_semantic_point_overlay_with_work(
            &bundle.placements,
            candidate_editor,
            &bundle.rectangle_projections,
            label,
            work,
        )
    }

    /// Selects one deterministic semantic representation of the complete
    /// native point-drag closure. Rectangle corners are four GUI lenses over
    /// two code seeds, so one authenticated corner and its diagonal opposite
    /// form the canonical, component-disjoint pair. Solver-roundoff aliases
    /// must describe that same pair; material disagreements fail closed.
    /// Independent companion points remain in the atomic bundle.
    #[allow(
        clippy::too_many_lines,
        reason = "the authenticated terminal bundle keeps exact point and complete rectangle-codec conflict handling in one atomic classifier"
    )]
    fn canonical_terminal_point_bundle(
        &self,
        authenticated: &ExpandedWritablePoint,
        placements: Vec<(ExpandedWritablePoint, [f64; 2])>,
        candidate_editor: &ProjectionalEditorSession,
    ) -> Result<CanonicalTerminalPointBundle, String> {
        if !placements.iter().any(|(point, _)| point == authenticated) {
            return Err(
                "terminal semantic placement bundle does not contain its authenticated point lens"
                    .into(),
            );
        }
        let expansion = self
            .session
            .snapshot()
            .accepted_expansion
            .as_ref()
            .ok_or_else(|| "code project has no accepted expansion authority".to_owned())?;
        let mut rectangle_groups = BTreeMap::<
            (CodeWritableAddress, CodeWritableAddress),
            Vec<(ExpandedWritablePoint, [f64; 2])>,
        >::new();
        let mut point_groups =
            BTreeMap::<CodeWritableAddress, Vec<(ExpandedWritablePoint, [f64; 2])>>::new();
        for placement in placements {
            match &placement.0.edit {
                CodePointEdit::Point { address } => {
                    point_groups
                        .entry(address.clone())
                        .or_default()
                        .push(placement);
                }
                CodePointEdit::RectangleCorner {
                    lower_left,
                    upper_right,
                    ..
                } => {
                    rectangle_groups
                        .entry((lower_left.clone(), upper_right.clone()))
                        .or_default()
                        .push(placement);
                }
            }
        }

        let mut canonical = Vec::new();
        for (_address, group) in point_groups {
            let selected = group
                .iter()
                .find(|(point, _)| point == authenticated)
                .unwrap_or(&group[0]);
            if group
                .iter()
                .any(|(_, target)| pair_bits(*target) != pair_bits(selected.1))
            {
                return Err(
                    "conflicting semantic point aliases require distinct exact drafts".into(),
                );
            }
            canonical.push(selected.clone());
        }

        let mut rectangle_projections = Vec::new();
        for (key, group) in rectangle_groups {
            let lenses = rectangle_lenses(expansion, &key)?;
            let authenticated_corner = group.iter().find_map(|(point, _)| {
                (point == authenticated)
                    .then(|| rectangle_corner(&point.edit))
                    .flatten()
            });
            let (first_corner, second_corner) = authenticated_corner.map_or(
                (
                    CodeRectangleCorner::LowerLeft,
                    CodeRectangleCorner::UpperRight,
                ),
                |corner| (corner, opposite_rectangle_corner(corner)),
            );
            let first = if authenticated_corner == Some(first_corner) {
                authenticated.clone()
            } else {
                (*lenses
                    .get(&first_corner)
                    .ok_or_else(|| "canonical rectangle first lens disappeared".to_owned())?)
                .clone()
            };
            let second = (*lenses
                .get(&second_corner)
                .ok_or_else(|| "canonical rectangle second lens disappeared".to_owned())?)
            .clone();
            let first_target =
                expanded_port_position(candidate_editor, &first.handle).ok_or_else(|| {
                    "canonical rectangle lens has no Cartesian instance seed".to_owned()
                })?;
            let second_target = expanded_port_position(candidate_editor, &second.handle)
                .ok_or_else(|| {
                    "canonical rectangle lens has no Cartesian instance seed".to_owned()
                })?;
            let (lower_left, upper_right) = canonical_rectangle_seeds(
                first_corner,
                first_target,
                second_corner,
                second_target,
            )?;
            for (point, target) in &group {
                let corner = rectangle_corner(&point.edit)
                    .ok_or_else(|| "rectangle group contains a non-rectangle lens".to_owned())?;
                let expected = rectangle_corner_position(lower_left, upper_right, corner);
                if !point_seed_roundoff_compatible(*target, expected) {
                    return Err(format!(
                        "conflicting rectangle `{}` aliases disagree beyond solver roundoff",
                        key.0.display_path(),
                    ));
                }
            }
            let redundant_aliases = [
                CodeRectangleCorner::LowerLeft,
                CodeRectangleCorner::LowerRight,
                CodeRectangleCorner::UpperRight,
                CodeRectangleCorner::UpperLeft,
            ]
            .into_iter()
            .filter(|corner| *corner != first_corner && *corner != second_corner)
            .map(|corner| {
                lenses
                    .get(&corner)
                    .map(|point| point.handle.clone())
                    .ok_or_else(|| "redundant rectangle parity lens disappeared".to_owned())
            })
            .collect::<Result<Vec<_>, _>>()?
            .try_into()
            .map_err(|_| "rectangle parity requires exactly two redundant aliases".to_owned())?;
            rectangle_projections.push(RectangleTerminalProjection {
                anchors: [first.handle.clone(), second.handle.clone()],
                redundant_aliases,
            });
            canonical.push((first, first_target));
            canonical.push((second, second_target));
        }
        canonical.sort_by_key(|(point, _)| (point != authenticated, point.handle.clone()));
        self.session
            .stage_point_drags(canonical.iter().map(|(point, target)| (point, *target)))
            .map_err(|error| error.to_string())?;
        Ok(CanonicalTerminalPointBundle {
            placements: canonical,
            rectangle_projections,
        })
    }

    fn publish_delegated_editor_checkpoint_unchecked(
        &mut self,
        editor_checkpoint: serde_json::Value,
        label: &str,
    ) -> Result<Option<AcceptedCodePublication>, String> {
        if &editor_checkpoint == self.session.pointer_frame_checkpoint() {
            return Ok(None);
        }
        let candidate_editor = restore_editor_checkpoint(&editor_checkpoint)?;
        let accepted_editor = restore_editor_checkpoint(self.session.pointer_frame_checkpoint())?;
        let code_change = classify_code_owned_editor_change(
            &accepted_editor,
            &candidate_editor,
            self.session
                .snapshot()
                .accepted_expansion
                .as_ref()
                .ok_or_else(|| "code project has no accepted expansion authority".to_owned())?,
        )?;
        if let Some(change) = code_change {
            if self.session.snapshot().failure.is_some() {
                return Err(
                    "resolve or Undo the retained code failure before editing code-owned geometry"
                        .into(),
                );
            }
            return match change {
                CodeOwnedEditorChange::SemanticPoints { placements } => {
                    self.publish_semantic_point_overlay(&placements, &candidate_editor, &[], label)
                }
            };
        }
        let expansion = self
            .session
            .snapshot()
            .accepted_expansion
            .clone()
            .ok_or_else(|| "code project has no accepted expansion authority".to_owned())?;
        let candidate_cache = rehydrate_editor_checkpoint(&editor_checkpoint, expansion)?;
        let prepared = self
            .session
            .prepare_delegated_editor_publication(self.session.identity(), editor_checkpoint, label)
            .map_err(|error| error.to_string())?;
        let receipt = self
            .session
            .apply_prepared(prepared)
            .map_err(|error| error.to_string())?;
        self.materialized = Some(candidate_cache);
        self.last_receipt = Some(receipt.clone());
        Ok(Some(AcceptedCodePublication {
            editor: candidate_editor,
            receipt,
        }))
    }

    fn publish_semantic_point_overlay(
        &mut self,
        placements: &[(ExpandedWritablePoint, [f64; 2])],
        candidate_editor: &ProjectionalEditorSession,
        rectangle_projections: &[RectangleTerminalProjection],
        label: &str,
    ) -> Result<Option<AcceptedCodePublication>, String> {
        self.publish_semantic_point_overlay_with_work(
            placements,
            candidate_editor,
            rectangle_projections,
            label,
            &mut CodeWorkReceipt::default(),
        )
    }

    fn publish_semantic_point_overlay_with_work(
        &mut self,
        placements: &[(ExpandedWritablePoint, [f64; 2])],
        candidate_editor: &ProjectionalEditorSession,
        rectangle_projections: &[RectangleTerminalProjection],
        label: &str,
        work: &mut CodeWorkReceipt,
    ) -> Result<Option<AcceptedCodePublication>, String> {
        if self.is_dirty() {
            return Err(
                "Apply or Revert the managed-source draft before dragging code-owned geometry"
                    .into(),
            );
        }
        if self.session.snapshot().failure.is_some() {
            return Err(
                "resolve or Undo the retained code failure before dragging code-owned geometry"
                    .into(),
            );
        }
        if placements.is_empty() {
            return Err("semantic point publication has no placements".into());
        }
        if placements
            .iter()
            .flat_map(|(_, position)| *position)
            .any(|value| !value.is_finite())
        {
            return Err("code-owned point placement is not finite".into());
        }
        let overlay = self
            .session
            .stage_point_drags(
                placements
                    .iter()
                    .map(|(point, position)| (point, *position)),
            )
            .map_err(|error| error.to_string())?;
        self.ensure_materialized_cache()?;
        let audited = materialize_code_project_incremental_with_overlay_audited(
            self.materialized
                .as_deref()
                .ok_or_else(|| "code project has no warm native authority".to_owned())?,
            &self.project,
            &self.session.snapshot().generated,
            &overlay,
        );
        work.merge(audited.work);
        let materialized = audited.outcome.map_err(|error| error.to_string())?;
        for (point, position) in placements {
            let staged_position = expanded_port_position(&materialized.editor, &point.handle)
                .ok_or_else(|| "staged code point has no Cartesian instance seed".to_owned())?;
            let terminal_position = expanded_port_position(candidate_editor, &point.handle)
                .ok_or_else(|| "terminal code point has no Cartesian instance seed".to_owned())?;
            if pair_bits(staged_position) != pair_bits(terminal_position)
                || pair_bits(terminal_position) != pair_bits(*position)
            {
                return Err("semantic point draft failed exact terminal/native seed parity".into());
            }
        }
        validate_terminal_native_parity(
            candidate_editor,
            &materialized.editor,
            &materialized.expansion,
            rectangle_projections,
        )?;
        let expansion = materialized.expansion.clone();
        let checkpoint = encode_editor_checkpoint(&materialized.editor)?;
        let delegated_editor = Box::new(
            materialized
                .editor
                .fork_accepted_authority()
                .map_err(|error| error.to_string())?,
        );
        let prepared = self
            .session
            .prepare_project_overlay(
                self.session.identity(),
                overlay,
                expansion,
                checkpoint,
                label,
            )
            .map_err(|error| error.to_string())?;
        let audited = self.session.apply_prepared_audited(prepared);
        work.merge(audited.work);
        let receipt = audited.outcome.map_err(|error| error.to_string())?;
        self.materialized = Some(Box::new(materialized));
        self.last_receipt = Some(receipt.clone());
        Ok(Some(AcceptedCodePublication {
            editor: delegated_editor,
            receipt,
        }))
    }

    /// Authenticates a code-owned point against the accepted semantic draft
    /// overlay before pointer motion begins. Unsupported code-owned points
    /// fail closed instead of becoming opaque delegated checkpoints.
    pub(crate) fn point_drag_permission(
        &self,
        editor: &ProjectionalEditorSession,
        point: geosolve_sketch::DesignPointId,
    ) -> Result<(), String> {
        let accepted = editor
            .coordinator()
            .accepted_materialization()
            .ok_or_else(|| "code project has no accepted native authority".to_owned())?;
        let Some(owner) = accepted
            .ownership
            .exact_owner(IntentNativeBinding::Point(point))
        else {
            return Ok(());
        };
        let Some(node) = editor.coordinator().intent().graph().node(owner) else {
            return Err("point owner is absent from accepted intent".into());
        };
        if !node.symbol.as_str().starts_with("code.") {
            return Ok(());
        }
        let expansion = self
            .session
            .snapshot()
            .accepted_expansion
            .as_ref()
            .ok_or_else(|| "code project has no accepted expansion authority".to_owned())?;
        if expansion.declaration_for_alias(&node.symbol).is_none() {
            return Err(
                "code-owned point has no authenticated managed declaration provenance".into(),
            );
        }
        if !expansion
            .writable_points
            .iter()
            .any(|candidate| expanded_port_point(editor, &candidate.handle) == Some(point))
        {
            return Err("this code-owned point has no semantic GUI draft lens".into());
        }
        if self.is_dirty() {
            return Err(
                "Apply or Revert the managed-source draft before dragging code-owned geometry"
                    .into(),
            );
        }
        self.session
            .snapshot()
            .failure
            .as_ref()
            .map_or(Ok(()), |_| {
                Err(
                    "resolve or Undo the retained code failure before dragging code-owned geometry"
                        .into(),
                )
            })
    }

    /// Authenticates one exact semantic point lens for native continuation.
    ///
    /// A shared producer/consumer native point cannot express a local consumer
    /// drag until the reference is detached. The caller supplies the managed
    /// declaration selected before the point press; producer selection keeps
    /// ordinary shared-follow behavior, no selection deterministically chooses
    /// the unique producer, and a selected referenced consumer detaches
    /// locally. The chosen point lens is retained through every native preview
    /// frame so solver-derived movement of coupled points cannot masquerade as
    /// additional terminal seed writes. Ambiguous semantic lenses reject
    /// without changing session, source, accepted scene, or history.
    pub(crate) fn prepare_semantic_point_drag(
        &mut self,
        editor: &ProjectionalEditorSession,
        pointer_id: u64,
        native_point: geosolve_sketch::DesignPointId,
        preferred_declaration: Option<&SemanticSymbol>,
    ) -> Result<Option<PreparedCodePointDrag>, String> {
        if self.pending_semantic_point_drag.is_some() {
            return Err("another authenticated semantic point gesture is still pending".into());
        }
        self.point_drag_permission(editor, native_point)?;
        let expansion = self
            .session
            .snapshot()
            .accepted_expansion
            .as_ref()
            .ok_or_else(|| "code project has no accepted expansion authority".to_owned())?;
        let candidates = expansion
            .writable_points
            .iter()
            .filter(|candidate| {
                expanded_port_point(editor, &candidate.handle) == Some(native_point)
            })
            .cloned()
            .collect::<Vec<_>>();
        let Some(point) =
            select_semantic_point_drag_lens(expansion, &candidates, preferred_declaration)?
        else {
            return Ok(None);
        };
        if candidates.len() == 1 || !point.source.is_reference() {
            self.pending_semantic_point_drag = Some(PendingSemanticPointDrag {
                pointer_id,
                session: self.session.identity().clone(),
                point,
                detached_origin_checkpoint: None,
            });
            return Ok(None);
        }
        // Before detachment a referenced consumer intentionally owns no
        // Cartesian instance leaves of its own: it aliases the producer's
        // accepted native point. Seed the transient detachment from that exact
        // accepted native position, then authenticate the consumer-local
        // leaves after expansion below.
        let position = editor
            .coordinator()
            .accepted_materialization()
            .and_then(|accepted| accepted.session.design_document().point(native_point))
            .map(|point| point.position)
            .ok_or_else(|| "referenced consumer has no accepted native point seed".to_owned())?;
        let overlay = self
            .session
            .stage_point_drag(&point, position)
            .map_err(|error| error.to_string())?;
        self.ensure_materialized_cache()?;
        let materialized = materialize_code_project_incremental_with_overlay(
            self.materialized
                .as_deref()
                .ok_or_else(|| "code project has no warm native authority".to_owned())?,
            &self.project,
            &self.session.snapshot().generated,
            &overlay,
        )
        .map_err(|error| error.to_string())?;
        let detached_point = expanded_port_point(&materialized.editor, &point.handle)
            .ok_or_else(|| "detached consumer has no accepted native point".to_owned())?;
        let checkpoint = encode_editor_checkpoint(&materialized.editor)?;
        let detached_editor = restore_editor_checkpoint(&checkpoint)?;
        self.pending_semantic_point_drag = Some(PendingSemanticPointDrag {
            pointer_id,
            session: self.session.identity().clone(),
            point,
            detached_origin_checkpoint: Some(checkpoint),
        });
        Ok(Some(PreparedCodePointDrag {
            editor: detached_editor,
            point: detached_point,
        }))
    }

    #[must_use]
    pub(crate) fn has_pending_semantic_point_drag(&self, pointer_id: u64) -> bool {
        self.pending_semantic_point_drag
            .as_ref()
            .is_some_and(|pending| pending.pointer_id == pointer_id)
    }

    #[must_use]
    pub(crate) fn has_any_pending_semantic_point_drag(&self) -> bool {
        self.pending_semantic_point_drag.is_some()
    }

    /// Cancels a pre-frame semantic detachment and restores the exact accepted
    /// nested editor. The code session and history were never changed.
    pub(crate) fn cancel_semantic_point_drag(
        &mut self,
        pointer_id: Option<u64>,
    ) -> Result<Option<Box<ProjectionalEditorSession>>, String> {
        let Some(pending) = self.pending_semantic_point_drag.as_ref() else {
            return Ok(None);
        };
        if pointer_id.is_some_and(|pointer_id| pointer_id != pending.pointer_id) {
            return Ok(None);
        }
        let pending = self
            .pending_semantic_point_drag
            .take()
            .expect("the authenticated pending semantic drag was present");
        if pending.detached_origin_checkpoint.is_some() {
            self.restore_accepted_editor().map(Some)
        } else {
            Ok(None)
        }
    }

    /// Returns a truthful read-only diagnostic for unsupported direct
    /// manipulation of code-owned curves/features. Annotation placement and
    /// ordinary GUI-owned geometry are intentionally outside this guard.
    pub(crate) fn selected_code_geometry_mutation_permission(
        editor: &ProjectionalEditorSession,
    ) -> Result<(), String> {
        let accepted = editor
            .coordinator()
            .accepted_materialization()
            .ok_or_else(|| "code project has no accepted native authority".to_owned())?;
        for item in editor.editor().selection() {
            let bindings = match item {
                SelectionItem::Point(point) => vec![IntentNativeBinding::Point(*point)],
                SelectionItem::Curve(span) => vec![
                    IntentNativeBinding::CurveSpan(*span),
                    IntentNativeBinding::Curve(span.curve),
                ],
                SelectionItem::Constraint(constraint) => {
                    vec![IntentNativeBinding::Constraint(*constraint)]
                }
                SelectionItem::Dimension(dimension) => {
                    vec![IntentNativeBinding::Dimension(*dimension)]
                }
                SelectionItem::Feature(feature) => {
                    vec![IntentNativeBinding::ComputedFeature(*feature)]
                }
                SelectionItem::FeatureCorner(corner) => vec![
                    IntentNativeBinding::ComputedFeatureCorner(corner.corner),
                    IntentNativeBinding::ComputedFeature(corner.feature),
                ],
                SelectionItem::Datum(_) => Vec::new(),
            };
            if bindings.into_iter().any(|binding| {
                accepted
                    .ownership
                    .exact_owner(binding)
                    .and_then(|node| editor.coordinator().intent().graph().node(node))
                    .is_some_and(|node| node.symbol.as_str().starts_with("code."))
            }) {
                return Err(
                    "this code-owned property has no semantic GUI draft lens; edit managed source or an exposed lens"
                        .into(),
                );
            }
        }
        Ok(())
    }

    pub(crate) fn restore_accepted_editor(&self) -> Result<Box<ProjectionalEditorSession>, String> {
        restore_editor_checkpoint(self.session.pointer_frame_checkpoint())
    }

    pub(crate) fn demo_key(&self) -> Option<&'static str> {
        self.origin.demo_key()
    }

    pub(crate) fn select_file(&mut self, path: &str) -> Result<(), String> {
        if path == MANAGED_FILE {
            self.selected_file = SelectedCodeFile::Managed;
            return Ok(());
        }
        if self.project.custom_files.contains_key(path) {
            self.selected_file = SelectedCodeFile::Custom(path.to_owned());
            return Ok(());
        }
        Err(format!("code-project file `{path}` is unavailable"))
    }

    pub(crate) fn set_managed_draft(&mut self, draft: String) {
        self.pending_semantic_point_drag = None;
        self.managed_draft = draft;
        // A prior diagnostic authenticates different draft bytes and must not
        // continue to claim line ownership while the user edits.
        self.draft_diagnostic = None;
    }

    /// Resolves the currently selected intent declaration back to the exact
    /// managed semantic owner published by code expansion.
    ///
    /// The implementation deliberately does not decode the hashed `code.*`
    /// developer symbol. An ordinary GUI-owned declaration is represented by
    /// `None`; every expansion-owned declaration must be present in the
    /// authenticated provenance map.
    pub(crate) fn selected_managed_declaration(
        &self,
        editor: &ProjectionalEditorSession,
    ) -> Result<Option<SemanticSymbol>, String> {
        let Some(node_id) = editor.selected_declaration() else {
            return Ok(None);
        };
        let node = editor
            .coordinator()
            .intent()
            .graph()
            .node(node_id)
            .ok_or_else(|| "selected declaration is absent from current intent".to_owned())?;
        let expansion = self
            .session
            .snapshot()
            .accepted_expansion
            .as_ref()
            .ok_or_else(|| "code project has no accepted expansion authority".to_owned())?;
        match expansion.declaration_for_alias(&node.symbol) {
            Some(declaration) => Ok(Some(declaration.clone())),
            None if node.symbol.as_str().starts_with("code.") => Err(
                "selected code-owned declaration has no authenticated managed-source provenance"
                    .into(),
            ),
            None => Ok(None),
        }
    }

    /// Reverse-projects one authenticated direct-dimension Inspector target
    /// into its managed `target` scalar and rematerializes the complete code
    /// project atomically.
    ///
    /// This is deliberately a closed route: unrelated GUI/code-owned
    /// declarations return `NotClaimed`; once a supported direct managed
    /// dimension is recognized, only its exact target leaf can be claimed.
    #[allow(
        clippy::too_many_lines,
        reason = "one closed adapter route authenticates selection, provenance, exact leaf ownership, source parity, and atomic publication together"
    )]
    pub(crate) fn apply_managed_dimension_inspector_edit(
        &mut self,
        editor: &ProjectionalEditorSession,
        inspector: &IntentInspectorProjection,
        target: &IntentInspectorEditTarget,
        value: &IntentInspectorEditValue,
    ) -> Result<CodeInspectorEditRoute, String> {
        let projection = editor.workbench_projection();
        if editor.selected_inspector(&projection).as_ref() != Some(inspector) {
            return Err("the Inspector edit belongs to a stale code-project projection".into());
        }
        self.ensure_materialized_cache()?;
        if self
            .materialized
            .as_deref()
            .map(|materialized| materialized.editor.coordinator().intent().identity())
            != Some(editor.coordinator().intent().identity())
        {
            return Err("the Inspector edit does not match accepted code-project authority".into());
        }
        let node = editor
            .coordinator()
            .intent()
            .graph()
            .node(inspector.node)
            .ok_or_else(|| "the selected Inspector declaration disappeared".to_owned())?;
        if node.symbol != inspector.symbol {
            return Err("the selected Inspector symbol no longer matches its declaration".into());
        }
        let expansion = self
            .session
            .snapshot()
            .accepted_expansion
            .as_ref()
            .ok_or_else(|| "code project has no accepted expansion authority".to_owned())?;
        let Some(declaration) = expansion.declaration_for_alias(&node.symbol).cloned() else {
            if node.symbol.as_str().starts_with("code.") {
                return Err(
                    "selected code-owned declaration has no authenticated managed-source provenance"
                        .into(),
                );
            }
            return Ok(CodeInspectorEditRoute::NotClaimed);
        };
        let IntentNodeKind::Dimension { dimension } = node.kind else {
            return Ok(CodeInspectorEditRoute::NotClaimed);
        };
        if !matches!(
            dimension,
            DimensionKind::CurveLength | DimensionKind::Diameter
        ) {
            return Ok(CodeInspectorEditRoute::NotClaimed);
        }
        if self.session.snapshot().failure.is_some() {
            return Err(
                "resolve or Undo the retained code failure before editing a code-owned dimension"
                    .into(),
            );
        }

        let managed_declaration = self
            .project
            .managed
            .program
            .declarations
            .iter()
            .find(|candidate| candidate.symbol == declaration)
            .ok_or_else(|| {
                "managed dimension provenance no longer resolves to source".to_owned()
            })?;
        let expected_builder = match dimension {
            DimensionKind::CurveLength => ["dimension", "curveLength"],
            DimensionKind::Diameter => ["dimension", "diameter"],
            _ => unreachable!("direct managed dimensions are closed above"),
        };
        if managed_declaration.patch.is_some()
            || managed_declaration.builder_path.as_slice() != expected_builder
        {
            return Err(
                "managed dimension provenance does not authenticate a direct supported family"
                    .into(),
            );
        }

        let IntentInspectorEditTarget::Instance { leaf } = target else {
            return Err(
                "this code-owned dimension property has no managed Inspector lens; edit managed source"
                    .into(),
            );
        };
        let target_port = node
            .port_by_selector(IntentPortSelector::Node {
                role: IntentPortRole::Target,
                index: 0,
            })
            .ok_or_else(|| "managed dimension has no target scalar port".to_owned())?;
        let expected_leaf = LeafRef {
            node: node.id,
            port: target_port.id,
            field: LeafField::Value,
        };
        if *leaf != expected_leaf {
            return Err(
                "this code-owned dimension leaf has no managed Inspector lens; edit managed source"
                    .into(),
            );
        }
        let IntentInspectorEditValue::Literal {
            literal:
                IntentLiteral::Quantity {
                    value,
                    unit: IntentUnit::Length,
                },
        } = value
        else {
            return Err("managed dimension target requires a finite length literal".into());
        };
        if !value.is_finite() {
            return Err("managed dimension target must be finite".into());
        }
        let source_target =
            managed_object_path(&managed_declaration.arguments, &["target".to_owned()])
                .and_then(managed_length_value)
                .ok_or_else(|| {
                    "managed dimension target is not a direct scalar literal".to_owned()
                })?;
        let accepted_target = editor
            .coordinator()
            .intent()
            .instance()
            .values()
            .get(&expected_leaf);
        if accepted_target
            != Some(&IntentLiteral::Quantity {
                value: source_target,
                unit: IntentUnit::Length,
            })
        {
            return Err(
                "managed dimension source and accepted target leaf no longer authenticate each other"
                    .into(),
            );
        }
        if self.is_dirty() {
            return Err(
                "Apply or Revert the managed-source draft before editing a code-owned dimension"
                    .into(),
            );
        }
        if self.pending_semantic_point_drag.is_some() {
            return Err(
                "finish or cancel the pending semantic point gesture before editing a code-owned dimension"
                    .into(),
            );
        }
        let selected_alias = node.symbol.clone();
        let mut outcome = self.apply_scalar_lens(&declaration.0, "target", *value)?;
        if let CodeApplyOutcome::Accepted(publication) = &mut outcome {
            let selected = publication
                .editor
                .coordinator()
                .intent()
                .graph()
                .node_by_symbol(&selected_alias)
                .expect("accepted managed dimension expansion preserves its semantic alias")
                .id;
            assert!(
                publication.editor.set_selected_declaration(Some(selected)),
                "accepted managed dimension expansion preserves selectable semantic ownership",
            );
        }
        Ok(CodeInspectorEditRoute::Claimed(outcome))
    }

    /// Resolves Delete through accepted code expansion. A generated host
    /// child is a reversible suppression target; only its managed invocation
    /// declaration is a source-rewrite target.
    pub(crate) fn selected_code_delete_target(
        &self,
        editor: &ProjectionalEditorSession,
    ) -> Result<Option<CodeDeleteTarget>, String> {
        let Some(node_id) = editor.selected_declaration() else {
            return Ok(None);
        };
        let node = editor
            .coordinator()
            .intent()
            .graph()
            .node(node_id)
            .ok_or_else(|| "selected declaration is absent from current intent".to_owned())?;
        let expansion = self
            .session
            .snapshot()
            .accepted_expansion
            .as_ref()
            .ok_or_else(|| "code project has no accepted expansion authority".to_owned())?;
        if let Some(child) = expansion.generated_child_for_alias(&node.symbol) {
            return Ok(Some(CodeDeleteTarget::GeneratedChild {
                address: child.address.clone(),
                alias: node.symbol.clone(),
                session: self.session.identity().clone(),
            }));
        }
        match expansion.declaration_for_alias(&node.symbol) {
            Some(declaration) => Ok(Some(CodeDeleteTarget::ManagedDeclaration {
                declaration: declaration.clone(),
                alias: node.symbol.clone(),
                session: self.session.identity().clone(),
            })),
            None if node.symbol.as_str().starts_with("code.") => Err(
                "selected code-owned declaration has no authenticated semantic provenance".into(),
            ),
            None => Ok(None),
        }
    }

    fn authenticate_delete_target(&self, target: &CodeDeleteTarget) -> Result<(), String> {
        let (expected, alias) = match target {
            CodeDeleteTarget::ManagedDeclaration { session, alias, .. }
            | CodeDeleteTarget::GeneratedChild { session, alias, .. } => (session, alias),
        };
        if expected != self.session.identity() {
            return Err("stale semantic deletion target; select the geometry again".into());
        }
        let expansion = self
            .session
            .snapshot()
            .accepted_expansion
            .as_ref()
            .ok_or_else(|| "code project has no accepted expansion authority".to_owned())?;
        match target {
            CodeDeleteTarget::ManagedDeclaration { declaration, .. }
                if expansion.declaration_for_alias(alias) == Some(declaration) => {}
            CodeDeleteTarget::GeneratedChild { address, .. }
                if expansion
                    .generated_child_for_alias(alias)
                    .is_some_and(|child| child.address == *address) => {}
            _ => {
                return Err(
                    "semantic deletion target no longer matches accepted expansion provenance"
                        .into(),
                );
            }
        }
        Ok(())
    }

    /// Publishes a generated child Delete as one semantic suppression overlay
    /// transaction. Managed source and the owning invocation remain intact.
    pub(crate) fn suppress_generated_child(
        &mut self,
        target: &CodeDeleteTarget,
    ) -> Result<AcceptedCodePublication, String> {
        self.authenticate_delete_target(target)?;
        let CodeDeleteTarget::GeneratedChild { address, .. } = target else {
            return Err("semantic deletion target is not a generated child".into());
        };
        if self.is_dirty() {
            return Err(
                "Apply or Revert the managed-source draft before suppressing generated geometry"
                    .into(),
            );
        }
        if self.session.snapshot().failure.is_some() {
            return Err(
                "resolve or Undo the retained code failure before suppressing generated geometry"
                    .into(),
            );
        }
        self.pending_semantic_point_drag = None;
        let overlay = self
            .session
            .stage_generated_child_suppression(address, true)
            .map_err(|error| error.to_string())?;
        self.ensure_materialized_cache()?;
        let materialized = materialize_code_project_incremental_with_overlay(
            self.materialized
                .as_deref()
                .ok_or_else(|| "code project has no warm native authority".to_owned())?,
            &self.project,
            &self.session.snapshot().generated,
            &overlay,
        )
        .map_err(|error| error.to_string())?;
        let child = materialized
            .expansion
            .generated_children
            .iter()
            .find(|child| child.address == *address)
            .ok_or_else(|| "suppressed generated child lost semantic provenance".to_owned())?;
        if !child.suppressed {
            return Err("generated child suppression did not reach expanded authority".into());
        }
        let expansion = materialized.expansion.clone();
        let checkpoint = encode_editor_checkpoint(&materialized.editor)?;
        let delegated_editor = restore_editor_checkpoint(&checkpoint)?;
        let candidate_cache = rehydrate_editor_checkpoint(&checkpoint, expansion.clone())?;
        let prepared = self
            .session
            .prepare_project_overlay(
                self.session.identity(),
                overlay,
                expansion,
                checkpoint,
                "Suppress generated child",
            )
            .map_err(|error| error.to_string())?;
        let receipt = self
            .session
            .apply_prepared(prepared)
            .map_err(|error| error.to_string())?;
        self.materialized = Some(candidate_cache);
        self.last_receipt = Some(receipt.clone());
        Ok(AcceptedCodePublication {
            editor: delegated_editor,
            receipt,
        })
    }

    /// Reverse-projects a canvas deletion into the managed TypeScript file.
    pub(crate) fn delete_managed_declaration(
        &mut self,
        target: &CodeDeleteTarget,
    ) -> Result<CodeApplyOutcome, String> {
        self.authenticate_delete_target(target)?;
        let CodeDeleteTarget::ManagedDeclaration { declaration, .. } = target else {
            return Err("semantic deletion target is not a managed declaration".into());
        };
        if self.is_dirty() {
            return Err(
                "Apply or Revert the managed-source draft before deleting code-owned geometry"
                    .into(),
            );
        }
        if self.session.snapshot().failure.is_some() {
            return Err(
                "resolve or Undo the retained code failure before deleting code-owned geometry"
                    .into(),
            );
        }
        let plan = plan_managed_edit(
            &self.project.managed,
            ManagedEdit::DeleteDeclaration {
                declaration: declaration.clone(),
            },
        )
        .map_err(|error| error.to_string())?;
        let managed =
            apply_managed_edit(&self.project.managed, &plan).map_err(|error| error.to_string())?;
        self.managed_draft = managed.source;
        self.apply_managed_draft()
    }

    #[allow(
        clippy::too_many_lines,
        reason = "one workbench transaction keeps parse, structural pruning, cold validation, failure retention, and publication atomic"
    )]
    pub(crate) fn apply_managed_draft(&mut self) -> Result<CodeApplyOutcome, String> {
        self.pending_semantic_point_drag = None;
        let managed = match parse_managed_source(&self.managed_draft) {
            Ok(managed) => managed,
            Err(error) => {
                self.draft_diagnostic = Some(error.diagnostic.clone());
                return Err(error.to_string());
            }
        };
        let mut candidate_project = self.project.clone();
        candidate_project.managed = managed;
        candidate_project
            .validate()
            .map_err(|error| error.to_string())?;
        let desired_members = match required_generated_members(&candidate_project) {
            Ok(members) => members,
            Err(error) => {
                return self.retain_candidate_failure(
                    candidate_project,
                    None,
                    self.session.snapshot().interaction_overlay.clone(),
                    None,
                    "structural expansion",
                    error.to_string(),
                );
            }
        };
        let plan = match self.session.plan_structural_reconciliation(
            self.session.identity(),
            desired_members,
            &BTreeSet::new(),
        ) {
            Ok(plan) => plan,
            Err(error) => {
                return self.retain_candidate_failure(
                    candidate_project,
                    None,
                    self.session.snapshot().interaction_overlay.clone(),
                    None,
                    "keyed reconciliation",
                    error.to_string(),
                );
            }
        };
        self.ensure_materialized_cache()?;
        match materialize_code_project_incremental_for_structural_edit(
            self.materialized
                .as_deref()
                .ok_or_else(|| "code project has no warm native authority".to_owned())?,
            &candidate_project,
            plan.staged(),
            &self.session.snapshot().interaction_overlay,
        )
        .map_err(|error| error.to_string())
        {
            Ok((materialized, retained_overlay)) => {
                let expansion = materialized.expansion.clone();
                let checkpoint = encode_editor_checkpoint(&materialized.editor)?;
                let delegated_editor = restore_editor_checkpoint(&checkpoint)?;
                let candidate_cache = rehydrate_editor_checkpoint(&checkpoint, expansion.clone())?;
                let prepared = self
                    .session
                    .prepare_project_edit_from_plan_with_overlay(
                        self.session.identity(),
                        candidate_project.clone(),
                        plan,
                        retained_overlay,
                        expansion,
                        checkpoint,
                        "Apply managed source",
                    )
                    .map_err(|error| error.to_string())?;
                let receipt = self
                    .session
                    .apply_prepared(prepared)
                    .map_err(|error| error.to_string())?;
                self.project = candidate_project;
                self.managed_draft = self.session.snapshot().managed.source.clone();
                self.draft_diagnostic = None;
                self.materialized = Some(candidate_cache);
                self.last_receipt = Some(receipt.clone());
                Ok(CodeApplyOutcome::Accepted(AcceptedCodePublication {
                    editor: delegated_editor,
                    receipt,
                }))
            }
            Err(diagnostic) => {
                let (retained_expansion, retained_overlay) = expansion_for_retained_failure(
                    &candidate_project,
                    plan.staged(),
                    &self.session.snapshot().interaction_overlay,
                    self.materialized
                        .as_deref()
                        .ok_or_else(|| "code project has no warm native authority".to_owned())?,
                );
                self.retain_candidate_failure(
                    candidate_project,
                    Some(plan),
                    retained_overlay,
                    retained_expansion,
                    "native materialization",
                    diagnostic,
                )
            }
        }
    }

    /// Applies one artifact-declared scalar edit lens through the exact same
    /// managed-source/native transaction as an explicit source Apply.
    pub(crate) fn apply_scalar_lens(
        &mut self,
        declaration: &str,
        path: &str,
        value: f64,
    ) -> Result<CodeApplyOutcome, String> {
        if !value.is_finite() {
            return Err("edit-lens value must be finite".into());
        }
        let path = path
            .split('.')
            .filter(|segment| !segment.is_empty())
            .map(str::to_owned)
            .collect::<Vec<_>>();
        if path.is_empty() {
            return Err("edit-lens argument path is empty".into());
        }
        let declaration = SemanticSymbol(declaration.to_owned());
        let current = self
            .project
            .managed
            .program
            .declarations
            .iter()
            .find(|candidate| candidate.symbol == declaration)
            .and_then(|candidate| managed_object_path(&candidate.arguments, &path))
            .ok_or_else(|| "edit-lens target is unavailable in managed source".to_owned())?;
        let replacement = match current {
            ManagedValue::Unit(unit) => ManagedValue::Unit(UnitLiteral {
                unit: unit.unit.clone(),
                value,
            }),
            ManagedValue::Number(_) => ManagedValue::Number(value),
            _ => return Err("this edit lens does not target a scalar literal".into()),
        };
        let plan = plan_managed_edit(
            &self.project.managed,
            ManagedEdit::SetInvocationArgument {
                declaration,
                path,
                value: replacement,
            },
        )
        .map_err(|error| error.to_string())?;
        let managed =
            apply_managed_edit(&self.project.managed, &plan).map_err(|error| error.to_string())?;
        self.set_managed_draft(managed.source);
        self.apply_managed_draft()
    }

    fn retain_candidate_failure(
        &mut self,
        candidate_project: CodeProject,
        plan: Option<geosolve_sketch_code::KeyedReconcilePlan>,
        interaction_overlay: CodeInteractionOverlay,
        expansion: Option<geosolve_sketch_code::ExpandedCodeProject>,
        stage: &str,
        diagnostic: String,
    ) -> Result<CodeApplyOutcome, String> {
        let prepared = self
            .session
            .prepare_project_retained_failure(
                self.session.identity(),
                candidate_project.clone(),
                plan,
                interaction_overlay,
                expansion,
                self.session.snapshot().accepted_editor_checkpoint.clone(),
                stage,
                diagnostic.clone(),
                "Apply managed source (retained failure)",
            )
            .map_err(|error| error.to_string())?;
        let receipt = self
            .session
            .apply_prepared(prepared)
            .map_err(|error| error.to_string())?;
        self.project = candidate_project;
        self.managed_draft = self.session.snapshot().managed.source.clone();
        self.draft_diagnostic = None;
        self.last_receipt = Some(receipt.clone());
        Ok(CodeApplyOutcome::RetainedFailure {
            receipt,
            diagnostic,
        })
    }

    pub(crate) fn revert_managed_draft(&mut self) -> bool {
        self.pending_semantic_point_drag = None;
        let changed = self.managed_draft != self.session.snapshot().managed.source
            || self.draft_diagnostic.is_some();
        self.managed_draft = self.session.snapshot().managed.source.clone();
        self.draft_diagnostic = None;
        changed
    }

    #[cfg(test)]
    pub(crate) fn apply_override(
        &mut self,
        address: &GeneratedMemberAddress,
        value: ManagedValue,
    ) -> Result<AcceptedCodePublication, String> {
        self.pending_semantic_point_drag = None;
        let mut generated = self.session.snapshot().generated.clone();
        generated
            .set_override(address, value.clone())
            .map_err(|error| error.to_string())?;
        self.ensure_materialized_cache()?;
        let materialized = materialize_code_project_incremental_with_overlay(
            self.materialized
                .as_deref()
                .ok_or_else(|| "code project has no warm native authority".to_owned())?,
            &self.project,
            &generated,
            &self.session.snapshot().interaction_overlay,
        )
        .map_err(|error| error.to_string())?;
        let expansion = materialized.expansion.clone();
        let checkpoint = encode_editor_checkpoint(&materialized.editor)?;
        let delegated_editor = restore_editor_checkpoint(&checkpoint)?;
        let candidate_cache = rehydrate_editor_checkpoint(&checkpoint, expansion.clone())?;
        let prepared = self
            .session
            .prepare_project_override(
                self.session.identity(),
                address,
                value,
                expansion,
                checkpoint,
                "Place generated override",
            )
            .map_err(|error| error.to_string())?;
        let receipt = self
            .session
            .apply_prepared(prepared)
            .map_err(|error| error.to_string())?;
        self.materialized = Some(candidate_cache);
        self.last_receipt = Some(receipt.clone());
        Ok(AcceptedCodePublication {
            editor: delegated_editor,
            receipt,
        })
    }

    pub(crate) fn reset_override(
        &mut self,
        display_path: &str,
    ) -> Result<Option<AcceptedCodePublication>, String> {
        let address = self
            .session
            .snapshot()
            .generated
            .active()
            .keys()
            .find(|address| address.display_path() == display_path)
            .cloned()
            .ok_or_else(|| format!("generated member `{display_path}` is unavailable"))?;
        if self
            .session
            .snapshot()
            .generated
            .override_for(&address)
            .is_none()
        {
            return Ok(None);
        }
        self.pending_semantic_point_drag = None;
        let mut generated = self.session.snapshot().generated.clone();
        generated
            .reset_to_code(&address)
            .map_err(|error| error.to_string())?;
        self.ensure_materialized_cache()?;
        let materialized = materialize_code_project_incremental_with_overlay(
            self.materialized
                .as_deref()
                .ok_or_else(|| "code project has no warm native authority".to_owned())?,
            &self.project,
            &generated,
            &self.session.snapshot().interaction_overlay,
        )
        .map_err(|error| error.to_string())?;
        let expansion = materialized.expansion.clone();
        let checkpoint = encode_editor_checkpoint(&materialized.editor)?;
        let delegated_editor = restore_editor_checkpoint(&checkpoint)?;
        let candidate_cache = rehydrate_editor_checkpoint(&checkpoint, expansion.clone())?;
        let prepared = self
            .session
            .prepare_project_reset_to_code(
                self.session.identity(),
                &address,
                expansion,
                checkpoint,
                "Reset generated override",
            )
            .map_err(|error| error.to_string())?
            .ok_or_else(|| "generated override disappeared before publication".to_owned())?;
        let receipt = self
            .session
            .apply_prepared(prepared)
            .map_err(|error| error.to_string())?;
        self.materialized = Some(candidate_cache);
        self.last_receipt = Some(receipt.clone());
        Ok(Some(AcceptedCodePublication {
            editor: delegated_editor,
            receipt,
        }))
    }

    pub(crate) fn reset_semantic_draft(
        &mut self,
        address_token: &str,
    ) -> Result<Option<AcceptedCodePublication>, String> {
        let address: CodeWritableAddress = serde_json::from_str(address_token)
            .map_err(|_| "semantic draft target is malformed or stale".to_owned())?;
        if self
            .session
            .snapshot()
            .interaction_overlay
            .draft(&address)
            .is_none()
        {
            return Err(format!(
                "semantic draft `{}` is unavailable",
                address.display_path()
            ));
        }
        let edit = self
            .session
            .snapshot()
            .accepted_expansion
            .as_ref()
            .and_then(|expansion| {
                expansion.writable_points.iter().find_map(|point| {
                    point
                        .edit
                        .writable_addresses()
                        .contains(&address)
                        .then(|| point.edit.clone())
                })
            })
            .ok_or_else(|| {
                format!(
                    "semantic draft `{}` has no accepted edit lens",
                    address.display_path()
                )
            })?;
        let mut overlay = self.session.snapshot().interaction_overlay.clone();
        if overlay.reset_many(edit.writable_addresses()) == 0 {
            return Ok(None);
        }
        self.pending_semantic_point_drag = None;
        self.ensure_materialized_cache()?;
        let materialized = materialize_code_project_incremental_with_overlay(
            self.materialized
                .as_deref()
                .ok_or_else(|| "code project has no warm native authority".to_owned())?,
            &self.project,
            &self.session.snapshot().generated,
            &overlay,
        )
        .map_err(|error| error.to_string())?;
        let expansion = materialized.expansion.clone();
        let checkpoint = encode_editor_checkpoint(&materialized.editor)?;
        let delegated_editor = restore_editor_checkpoint(&checkpoint)?;
        let candidate_cache = rehydrate_editor_checkpoint(&checkpoint, expansion.clone())?;
        let prepared = self
            .session
            .prepare_project_reset_draft(
                self.session.identity(),
                &address,
                expansion,
                checkpoint,
                "Reset semantic GUI draft",
            )
            .map_err(|error| error.to_string())?;
        let Some(prepared) = prepared else {
            return Ok(None);
        };
        let receipt = self
            .session
            .apply_prepared(prepared)
            .map_err(|error| error.to_string())?;
        self.materialized = Some(candidate_cache);
        self.last_receipt = Some(receipt.clone());
        Ok(Some(AcceptedCodePublication {
            editor: delegated_editor,
            receipt,
        }))
    }

    pub(crate) fn reset_generated_child_suppression(
        &mut self,
        address_token: &str,
    ) -> Result<Option<AcceptedCodePublication>, String> {
        let address: CodeGeneratedChildAddress = serde_json::from_str(address_token)
            .map_err(|_| "generated-child suppression target is malformed or stale".to_owned())?;
        if self
            .session
            .snapshot()
            .interaction_overlay
            .generated_child_suppression(&address)
            != Some(true)
        {
            return Err(format!(
                "generated child `{}` is not suppressed",
                address.display_path()
            ));
        }
        let mut overlay = self.session.snapshot().interaction_overlay.clone();
        if !overlay.reset_generated_child_suppression(&address) {
            return Ok(None);
        }
        self.pending_semantic_point_drag = None;
        self.ensure_materialized_cache()?;
        let materialized = materialize_code_project_incremental_with_overlay(
            self.materialized
                .as_deref()
                .ok_or_else(|| "code project has no warm native authority".to_owned())?,
            &self.project,
            &self.session.snapshot().generated,
            &overlay,
        )
        .map_err(|error| error.to_string())?;
        let expansion = materialized.expansion.clone();
        let checkpoint = encode_editor_checkpoint(&materialized.editor)?;
        let delegated_editor = restore_editor_checkpoint(&checkpoint)?;
        let candidate_cache = rehydrate_editor_checkpoint(&checkpoint, expansion.clone())?;
        let prepared = self
            .session
            .prepare_project_reset_generated_child_suppression(
                self.session.identity(),
                &address,
                expansion,
                checkpoint,
                "Reset generated child suppression",
            )
            .map_err(|error| error.to_string())?;
        let Some(prepared) = prepared else {
            return Ok(None);
        };
        let receipt = self
            .session
            .apply_prepared(prepared)
            .map_err(|error| error.to_string())?;
        self.materialized = Some(candidate_cache);
        self.last_receipt = Some(receipt.clone());
        Ok(Some(AcceptedCodePublication {
            editor: delegated_editor,
            receipt,
        }))
    }

    pub(crate) fn step_history(
        &mut self,
        undo: bool,
    ) -> Result<Option<AcceptedCodePublication>, String> {
        if (undo && !self.session.can_undo()) || (!undo && !self.session.can_redo()) {
            return Ok(None);
        }
        self.pending_semantic_point_drag = None;
        // Restore and independently validate the nested authority before
        // replacing the live code session, so corrupt opaque checkpoint bytes
        // cannot leave history half-stepped.
        let mut candidate_session = self.session.clone();
        let receipt = if undo {
            candidate_session.undo()
        } else {
            candidate_session.redo()
        }
        .map_err(|error| error.to_string())?;
        let Some(receipt) = receipt else {
            return Ok(None);
        };
        let editor = restore_editor_checkpoint(candidate_session.pointer_frame_checkpoint())?;
        let expansion = candidate_session
            .snapshot()
            .accepted_expansion
            .clone()
            .ok_or_else(|| "code history restored no accepted expansion authority".to_owned())?;
        let candidate_cache = rehydrate_materialized_code_project(
            restore_editor_checkpoint(candidate_session.pointer_frame_checkpoint())?,
            expansion,
        )
        .map_err(|error| error.to_string())?;
        let project = candidate_session
            .snapshot()
            .code_project
            .clone()
            .ok_or_else(|| "code history restored incomplete project authority".to_owned())?;
        self.session = candidate_session;
        self.project = project;
        self.managed_draft = self.project.managed.source.clone();
        self.draft_diagnostic = None;
        self.materialized = Some(candidate_cache);
        self.last_receipt = Some(receipt.clone());
        Ok(Some(AcceptedCodePublication { editor, receipt }))
    }

    pub(crate) fn can_undo(&self) -> bool {
        self.session.can_undo()
    }

    pub(crate) fn can_redo(&self) -> bool {
        self.session.can_redo()
    }

    pub(crate) fn is_dirty(&self) -> bool {
        self.managed_draft != self.session.snapshot().managed.source
    }

    #[cfg(test)]
    pub(crate) fn managed_source(&self) -> &str {
        &self.session.snapshot().managed.source
    }

    pub(crate) fn panel_markup(&self) -> String {
        let mut markup = String::new();
        markup.push_str("<div class=\"wb-code-project\">");
        self.write_project_header(&mut markup);
        self.write_file_tabs(&mut markup);
        self.write_source_surface(&mut markup);
        self.write_artifact_status(&mut markup);
        self.write_edit_lenses(&mut markup);
        self.write_generated_members(&mut markup);
        markup.push_str("</div>");
        markup
    }

    fn write_project_header(&self, markup: &mut String) {
        let revision = self.session.identity().revision;
        let dirty = if self.is_dirty() {
            " · unsaved draft"
        } else {
            ""
        };
        let _ = write!(
            markup,
            concat!(
                "<header class=\"wb-code-project-header\"><div>",
                "<span class=\"wb-code-eyebrow\">Code project</span>",
                "<strong>{}</strong><small>managed-v1 · revision {}{}</small>",
                "</div><span class=\"wb-code-runtime-badge\">Rust runtime · data only</span></header>"
            ),
            escape_html(self.origin.title()),
            revision,
            dirty,
        );
    }

    fn write_file_tabs(&self, markup: &mut String) {
        markup.push_str(
            "<div class=\"wb-code-file-tabs\" role=\"tablist\" aria-label=\"Code project files\">",
        );
        write_file_tab(
            markup,
            MANAGED_FILE,
            "Managed",
            matches!(self.selected_file, SelectedCodeFile::Managed),
        );
        for path in self.project.custom_files.keys() {
            write_file_tab(
                markup,
                path,
                "Custom · read-only",
                self.selected_file.path() == path,
            );
        }
        markup.push_str("</div>");
    }

    fn write_source_surface(&self, markup: &mut String) {
        match &self.selected_file {
            SelectedCodeFile::Managed => {
                let _ = write!(
                    markup,
                    concat!(
                        "<section class=\"wb-code-editor\" data-code-file-kind=\"managed\">",
                        "<div class=\"wb-code-editor-toolbar\"><div><strong>sketch.ts</strong>",
                        "<span>GUI-managed subset</span></div><div>",
                        "<button type=\"button\" data-code-action=\"revert\"{}>Revert</button>",
                        "<button type=\"button\" data-code-action=\"apply\"{}>Apply</button>",
                        "</div></div>",
                        "<textarea id=\"wb-code-managed-source\" spellcheck=\"false\" ",
                        "autocomplete=\"off\" autocapitalize=\"off\" aria-label=\"Managed sketch TypeScript\">{}</textarea>"
                    ),
                    if self.is_dirty() || self.draft_diagnostic.is_some() {
                        ""
                    } else {
                        " disabled"
                    },
                    if self.is_dirty() { "" } else { " disabled" },
                    escape_html(&self.managed_draft),
                );
                if let Some(diagnostic) = &self.draft_diagnostic {
                    let _ = write!(
                        markup,
                        concat!(
                            "<div class=\"wb-code-diagnostic\" role=\"alert\" data-line=\"{}\">",
                            "<strong>Line {}, column {}</strong><span>{}</span></div>"
                        ),
                        diagnostic.line,
                        diagnostic.line,
                        diagnostic.column,
                        escape_html(&diagnostic.message),
                    );
                } else {
                    markup.push_str(
                        "<p class=\"wb-code-editor-note\">Apply reparses and validates the complete candidate before one publication. Comments and unowned formatting stay byte-identical.</p>",
                    );
                }
                markup.push_str("</section>");
            }
            SelectedCodeFile::Custom(path) => {
                let file = &self.project.custom_files[path];
                let _ = write!(
                    markup,
                    concat!(
                        "<section class=\"wb-code-editor\" data-code-file-kind=\"custom\">",
                        "<div class=\"wb-code-editor-toolbar\"><div><strong>{}</strong>",
                        "<span>User / AI owned</span></div><span class=\"wb-code-readonly\">Read-only in demo</span></div>",
                        "<pre tabindex=\"0\" aria-label=\"Read-only custom patch source\"><code>{}</code></pre>",
                        "<p class=\"wb-code-editor-note\">GeoSolve never rewrites or evaluates this file in Rust/WASM. The caller-owned Node build emits the pinned artifact below.</p>",
                        "</section>"
                    ),
                    escape_html(path),
                    escape_html(&file.contents),
                );
            }
        }
    }

    fn write_artifact_status(&self, markup: &mut String) {
        let artifact_count = self.project.artifacts.len();
        let lens_count = self
            .project
            .artifacts
            .values()
            .filter_map(|value| serde_json::from_value::<PatchModuleArtifact>(value.clone()).ok())
            .map(|artifact| artifact.edit_lenses.len())
            .sum::<usize>();
        let (heading, detail, state) = if artifact_count == 0 {
            (
                "Managed source ready",
                "No custom modules · direct declarations only".to_owned(),
                "Artifact-free",
            )
        } else {
            (
                "Artifacts ready",
                format!(
                    "{} pinned module{} · {} edit lens{}",
                    artifact_count,
                    if artifact_count == 1 { "" } else { "s" },
                    lens_count,
                    if lens_count == 1 { "" } else { "es" },
                ),
                "Offline · ABI v1",
            )
        };
        let _ = write!(
            markup,
            concat!(
                "<section class=\"wb-code-artifact-status\"><div>",
                "<span class=\"wb-code-status-dot\" aria-hidden=\"true\"></span>",
                "<div><strong>{}</strong><small>{}</small></div>",
                "</div><span>{}</span></section>"
            ),
            heading, detail, state,
        );
    }

    fn write_edit_lenses(&self, markup: &mut String) {
        let lenses = self
            .project
            .artifacts
            .values()
            .filter_map(|value| serde_json::from_value::<PatchModuleArtifact>(value.clone()).ok())
            .flat_map(|artifact| {
                let module = artifact.module_specifier;
                let export = artifact.export_name;
                artifact
                    .edit_lenses
                    .into_iter()
                    .map(move |lens| (module.clone(), export.clone(), lens))
            })
            .collect::<Vec<_>>();
        if lenses.is_empty() {
            return;
        }
        markup.push_str(
            "<section class=\"wb-code-lenses\"><header><strong>Edit lenses</strong><span>Generated values with managed-source controls</span></header>",
        );
        for (module, export, lens) in lenses {
            let output = managed_path_text(&lens.output.0);
            let argument = lens.invocation_argument.join(".");
            let scalar = self
                .project
                .managed
                .program
                .declarations
                .iter()
                .find(|declaration| {
                    declaration
                        .patch
                        .as_ref()
                        .is_some_and(|patch| patch.module_binding == export)
                })
                .and_then(|declaration| {
                    managed_object_path(&declaration.arguments, &lens.invocation_argument)
                        .and_then(|value| match value {
                            ManagedValue::Number(value) => Some((*value, None)),
                            ManagedValue::Unit(value) => {
                                Some((value.value, Some(value.unit.as_str())))
                            }
                            _ => None,
                        })
                        .map(|value| (declaration, value))
                });
            let _ = write!(
                markup,
                concat!(
                    "<div class=\"wb-code-lens\"><div><strong>{}</strong>",
                    "<small>{} → invocation.{}</small></div>"
                ),
                escape_html(&output),
                escape_html(&module),
                escape_html(&argument),
            );
            if let Some((declaration, (value, unit))) = scalar {
                let _ = write!(
                    markup,
                    concat!(
                        "<label class=\"wb-code-lens-control\"><span class=\"wb-sr-only\">Edit {}</span>",
                        "<input type=\"number\" step=\"any\" value=\"{}\" ",
                        "data-code-lens-declaration=\"{}\" data-code-lens-path=\"{}\" />",
                        "<small>{}</small></label>"
                    ),
                    escape_html(&output),
                    value,
                    escape_attribute(&declaration.symbol.0),
                    escape_attribute(&argument),
                    escape_html(unit.unwrap_or("value")),
                );
            } else {
                markup.push_str(
                    "<button type=\"button\" data-code-action=\"open-managed-lens\">Edit managed input</button>",
                );
            }
            markup.push_str("</div>");
        }
        markup.push_str("</section>");
    }

    fn write_generated_members(&self, markup: &mut String) {
        let members = self.session.snapshot().generated.ordered_members();
        let mut groups = BTreeMap::<&str, Vec<_>>::new();
        for member in &members {
            groups
                .entry(member.address.invocation.as_str())
                .or_default()
                .push(member);
        }
        markup.push_str("<section class=\"wb-code-generated\"><header><div><strong>Generated ownership</strong><span>Stable semantic keys, not wire IDs</span></div>");
        let _ = write!(
            markup,
            "<small>{} member{}</small></header>",
            members.len(),
            if members.len() == 1 { "" } else { "s" },
        );
        for (invocation, group) in groups {
            let _ = write!(
                markup,
                "<div class=\"wb-code-member-group\"><h3>{}</h3>",
                escape_html(invocation),
            );
            for member in group {
                let overridden = self
                    .session
                    .snapshot()
                    .generated
                    .override_for(&member.address)
                    .is_some();
                let path = member.address.display_path();
                let member_key = member.address.member_key.join(" / ");
                let _ = write!(
                    markup,
                    concat!(
                        "<div class=\"wb-code-member\" data-code-ownership=\"{}\">",
                        "<div><strong>{}</strong><small>{}</small></div>",
                        "<span class=\"wb-code-ownership-badge\">{}</span>"
                    ),
                    if overridden { "override" } else { "generated" },
                    escape_html(&member_key),
                    escape_html(&path),
                    if overridden { "Override" } else { "Code-owned" },
                );
                if overridden {
                    let _ = write!(
                        markup,
                        "<button type=\"button\" data-code-action=\"reset-override\" data-code-member=\"{}\">Reset to code</button>",
                        escape_attribute(&path),
                    );
                }
                markup.push_str("</div>");
            }
            markup.push_str("</div>");
        }
        if members.is_empty() {
            markup.push_str("<p class=\"wb-code-empty\">This project has no structurally generated members.</p>");
        }
        self.write_interaction_overlay_members(markup);
        if let Some(receipt) = &self.last_receipt {
            let _ = write!(
                markup,
                "<p class=\"wb-code-history-note\">Latest unified action: <strong>{}</strong> · revision {}</p>",
                escape_html(&receipt.label),
                receipt.after.revision,
            );
        }
        markup.push_str("</section>");
    }

    fn write_interaction_overlay_members(&self, markup: &mut String) {
        let overlay = &self.session.snapshot().interaction_overlay;
        for (address, draft) in overlay.drafts() {
            let path = address.display_path();
            let token = serde_json::to_string(address)
                .expect("validated semantic draft address is infallibly serializable");
            let _ = write!(
                markup,
                concat!(
                    "<div class=\"wb-code-member\" data-code-ownership=\"draft\">",
                    "<div><strong>GUI placement</strong><small>{}</small></div>",
                    "<span class=\"wb-code-ownership-badge\">{:?}</span>",
                    "<button type=\"button\" data-code-action=\"reset-semantic-draft\" ",
                    "data-code-draft=\"{}\">Reset to code</button></div>"
                ),
                escape_html(&path),
                draft.provenance,
                escape_attribute(&token),
            );
        }
        for (address, suppressed) in overlay.suppressed_children() {
            if !suppressed {
                continue;
            }
            let path = address.display_path();
            let token = serde_json::to_string(address)
                .expect("validated generated-child address is infallibly serializable");
            let _ = write!(
                markup,
                concat!(
                    "<div class=\"wb-code-member\" data-code-ownership=\"suppressed\">",
                    "<div><strong>Suppressed generated child</strong><small>{}</small></div>",
                    "<span class=\"wb-code-ownership-badge\">Suppressed</span>",
                    "<button type=\"button\" data-code-action=\"reset-child-suppression\" ",
                    "data-code-child=\"{}\">Restore from code</button></div>"
                ),
                escape_html(&path),
                escape_attribute(&token),
            );
        }
    }
}

fn validate_managed_draft_bound(draft: &str) -> Result<(), String> {
    if draft.len() > geosolve_sketch_code::MANAGED_SOURCE_LIMIT {
        Err(format!(
            "managed source draft is {} bytes; the limit is {}",
            draft.len(),
            geosolve_sketch_code::MANAGED_SOURCE_LIMIT,
        ))
    } else {
        Ok(())
    }
}

/// Read-only managed-v1 projection of one complete ordinary GUI workspace.
/// The project is deliberately rebuilt and revalidated when Promote is
/// clicked; these source bytes never become authority by being rendered.
pub(crate) struct OrdinaryCodePreview {
    source: String,
    declaration_count: usize,
}

/// Durable-panel presentation for the optional projection of an ordinary GUI
/// workspace. Conversion failure is a visible state, not absence of the Code
/// surface: hiding it would make an unsupported declaration indistinguishable
/// from the optional authoring layer not existing at all.
pub(crate) enum OrdinaryCodeSurface {
    Starter,
    Preview(OrdinaryCodePreview),
    Unavailable { reason: String },
}

impl OrdinaryCodeSurface {
    pub(crate) fn from_editor(editor: &ProjectionalEditorSession) -> Self {
        if ordinary_code_starter_available(editor) {
            return Self::Starter;
        }
        match OrdinaryCodePreview::from_editor(editor) {
            Ok(preview) => Self::Preview(preview),
            Err(reason) => Self::Unavailable { reason },
        }
    }

    pub(crate) fn panel_markup(&self) -> String {
        match self {
            Self::Starter => code_starter_markup(),
            Self::Preview(preview) => preview.panel_markup(),
            Self::Unavailable { reason } => format!(
                concat!(
                    "<div class=\"wb-code-project-empty wb-code-preview-unavailable\" ",
                    "data-code-preview-state=\"unavailable\" role=\"status\">",
                    "<span class=\"wb-code-eyebrow\">Managed code preview</span>",
                    "<strong>Code preview unavailable</strong>",
                    "<p>The complete ordinary sketch cannot yet be represented by the managed TypeScript projection.</p>",
                    "<p class=\"wb-code-diagnostic\">{}</p>",
                    "<p>Intent IR remains available, and no declaration has been omitted or made authoritative as partial code.</p>",
                    "</div>"
                ),
                escape_html(reason),
            ),
        }
    }
}

fn code_starter_markup() -> String {
    let mut markup = String::from(concat!(
        "<div class=\"wb-code-project-empty wb-code-starter\" ",
        "data-code-preview-state=\"starter\">",
        "<span class=\"wb-code-eyebrow\">Code-authored sketch</span>",
        "<strong>Start with editable sketch.ts</strong>",
        "<p>Create an artifact-free managed project directly from code. The starter demonstrates typed lexical feature references, and you can replace the complete source.</p>",
        "<button type=\"button\" class=\"wb-code-starter-action\" data-code-action=\"start-authored\">Start from code</button>",
        "<section class=\"wb-code-starter-samples\"><header><strong>Complete code examples</strong><span>Open, edit, Apply, drag and reload</span></header>",
        "<div class=\"wb-code-starter-grid\">",
    ));
    for demo in bundled_code_project_demos() {
        let card_title = demo
            .title
            .split_once(" · ")
            .map_or(demo.title, |(title, _)| title);
        let _ = write!(
            markup,
            concat!(
                "<button type=\"button\" class=\"wb-code-starter-card\" ",
                "data-code-sample-id=\"{}\" aria-label=\"{}\" title=\"{}\">",
                "<span class=\"wb-code-sample-mark\" ",
                "aria-hidden=\"true\">TS</span><strong>{}</strong><small>{}</small></button>"
            ),
            demo.id.key(),
            escape_html(demo.title),
            escape_html(demo.title),
            escape_html(card_title),
            escape_html(demo.summary()),
        );
    }
    markup.push_str("</div></section></div>");
    markup
}

fn ordinary_code_starter_available(editor: &ProjectionalEditorSession) -> bool {
    let coordinator = editor.coordinator();
    let intent = coordinator.intent();
    let semantic = intent.semantic_identity();
    let Some(authority) = intent.accepted() else {
        return false;
    };
    let Some(accepted) = coordinator.accepted_materialization() else {
        return false;
    };
    let nodes = intent.graph().nodes();
    let canonical_graph = nodes.len() == 1
        && nodes
            .values()
            .next()
            .is_some_and(is_canonical_document_foundation);
    let validation = &accepted.validation;
    canonical_graph
        && authority.target == semantic
        && validation.semantic == semantic
        && validation.hard_residuals_validated
        && validation.all_active_features_current
        && validation.point_count == 0
        && validation.curve_count == 0
        && validation.constraint_count == 0
        && validation.feature_count == 0
        && validation.computed_edge_count == 0
        && validation
            .maximum_normalized_hard_residual
            .is_none_or(|value| value.is_finite() && value <= 1.0e-9)
}

fn is_canonical_document_foundation(node: &IntentNode) -> bool {
    matches!(
        node.kind,
        IntentNodeKind::Bootstrap { ref object }
            if object.kind == BootstrapNativeKind::Document
                && object.codec.as_str() == BOOTSTRAP_DOCUMENT_HEADER_CODEC_V1
    ) && node.bootstrap_origin.is_none()
        && !node.suppressed
        && node.inputs.is_empty()
        && node.fields.is_empty()
        && node.operation_outputs.is_empty()
        && node.child_order.is_empty()
        && node.children.is_empty()
}

impl OrdinaryCodePreview {
    pub(crate) fn from_editor(editor: &ProjectionalEditorSession) -> Result<Self, String> {
        let project = projected_code_project(editor)?;
        Ok(Self {
            source: project.managed.source,
            declaration_count: project.managed.program.declarations.len(),
        })
    }

    pub(crate) fn panel_markup(&self) -> String {
        format!(
            concat!(
                "<div class=\"wb-code-project wb-code-preview\">",
                "<header class=\"wb-code-project-header\"><div>",
                "<span class=\"wb-code-eyebrow\">Managed code preview</span>",
                "<strong>Complete ordinary sketch</strong><small>{} declaration{}</small>",
                "</div><span class=\"wb-code-runtime-badge\">Read-only · not authority</span></header>",
                "<section class=\"wb-code-editor\" data-code-file-kind=\"managed-preview\">",
                "<div class=\"wb-code-editor-toolbar\"><div><strong>sketch.ts</strong>",
                "<span>Lexical, typed feature references</span></div>",
                "<button type=\"button\" data-code-action=\"promote-ordinary\">Promote to code project</button></div>",
                "<pre tabindex=\"0\" aria-label=\"Read-only managed sketch TypeScript preview\"><code>{}</code></pre>",
                "<p class=\"wb-code-editor-note\">Promotion reparses, cold-materializes, and independently validates the complete candidate before replacing the ordinary workspace. Unsupported declarations are never omitted.</p>",
                "</section></div>"
            ),
            self.declaration_count,
            if self.declaration_count == 1 { "" } else { "s" },
            escape_html(&self.source),
        )
    }
}

/// Produces one artifact-free project from every ordinary declaration in its
/// current presentation order. The optional layer owns the naming policy;
/// canonical node/port IDs never appear in managed source.
fn projected_code_project(editor: &ProjectionalEditorSession) -> Result<CodeProject, String> {
    let intent = editor.coordinator().intent();
    let projection = editor.workbench_projection();
    let ordered = projection
        .outline
        .iter()
        .flat_map(|cell| &cell.declarations)
        .collect::<Vec<_>>();
    if ordered.is_empty() {
        return Err("the ordinary sketch has no declarations to promote".into());
    }
    if ordered.len() != intent.graph().nodes().len() {
        return Err(
            "the complete ordinary declaration graph is not available for managed promotion".into(),
        );
    }

    let mut base_counts = BTreeMap::<String, usize>::new();
    let mut declarations = Vec::with_capacity(ordered.len());
    let mut document_foundations = 0_usize;
    for projected in ordered {
        let node = intent
            .graph()
            .node(projected.node)
            .ok_or_else(|| "an ordinary declaration disappeared during code preview".to_owned())?;
        if is_canonical_document_foundation(node) {
            // A fresh M83 workspace owns one logical native-document header
            // beneath every later recipe. It is materializer seed metadata,
            // not authored geometry, and the promoted CodeProject creates its
            // own independently validated document authority. No other
            // bootstrap object is omitted: legacy points/curves/constraints
            // continue through the unsupported-declaration failure below.
            document_foundations = document_foundations.saturating_add(1);
            continue;
        }
        let base = match node.kind {
            IntentNodeKind::Geometry {
                recipe: GeometryRecipeKind::TwoPointAlignedRectangle,
            } => "frame",
            IntentNodeKind::Geometry {
                recipe: GeometryRecipeKind::Segment,
            } if node.inputs.len() >= 2
                && node.inputs.values().all(|input| {
                    intent.graph().node(input.node).is_some_and(|owner| {
                        matches!(
                            owner.kind,
                            IntentNodeKind::Geometry {
                                recipe: GeometryRecipeKind::TwoPointAlignedRectangle
                            }
                        )
                    })
                }) =>
            {
                "diagonal"
            }
            IntentNodeKind::Geometry {
                recipe: GeometryRecipeKind::Segment,
            } => "line",
            IntentNodeKind::ComputedFeature {
                feature: geosolve_sketch_intent::ComputedFeatureKind::FilletSet,
            } => "fillet",
            IntentNodeKind::Constraint {
                constraint: geosolve_sketch_intent::ConstraintKind::Horizontal,
            } => "horizontal",
            IntentNodeKind::Constraint {
                constraint: geosolve_sketch_intent::ConstraintKind::Vertical,
            } => "vertical",
            _ => "declaration",
        };
        let ordinal = base_counts.entry(base.to_owned()).or_default();
        *ordinal = ordinal.saturating_add(1);
        let symbol = if *ordinal == 1 {
            base.to_owned()
        } else {
            format!("{base}{ordinal}")
        };
        declarations.push(EditorBootstrapDeclaration::new(
            node.id,
            SemanticSymbol(symbol),
        ));
    }
    if document_foundations > 1 {
        return Err("the ordinary sketch has more than one document foundation".into());
    }
    if declarations.is_empty() {
        return Err("the ordinary sketch has no authored declarations to promote".into());
    }
    initialize_code_project_from_editor(
        editor,
        ProjectKey("gui-promoted-sketch".into()),
        &declarations,
    )
    .map_err(|error| error.to_string())
}

pub(crate) fn sample_group_markup(selected: Option<CodeProjectDemoId>) -> String {
    let mut markup = String::from(
        "<li class=\"wb-sample-branch\"><button type=\"button\" data-sample-group-trigger aria-haspopup=\"menu\" aria-expanded=\"false\">Code &amp; reusable patches<span aria-hidden=\"true\">›</span></button><ul class=\"wb-sample-flyout\">",
    );
    for demo in bundled_code_project_demos() {
        let _ = write!(
            markup,
            "<li><button type=\"button\" data-code-sample-id=\"{}\"{}><span class=\"wb-code-sample-mark\" aria-hidden=\"true\">TS</span>{}</button></li>",
            demo.id.key(),
            if selected == Some(demo.id) {
                " aria-current=\"true\""
            } else {
                ""
            },
            escape_html(demo.title),
        );
    }
    markup.push_str("</ul></li>");
    markup
}

fn write_file_tab(markup: &mut String, path: &str, ownership: &str, selected: bool) {
    let _ = write!(
        markup,
        concat!(
            "<button type=\"button\" role=\"tab\" data-code-file=\"{}\" ",
            "aria-selected=\"{}\"><span>{}</span><small>{}</small></button>"
        ),
        escape_attribute(path),
        selected,
        escape_html(path.rsplit('/').next().unwrap_or(path)),
        escape_html(ownership),
    );
}

fn artifact_digests(project: &CodeProject) -> Result<BTreeMap<String, String>, String> {
    let modules = project
        .lock
        .get("modules")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| "code-project lock has no module pins".to_owned())?;
    modules
        .iter()
        .map(|(module, pin)| {
            pin.get("artifact")
                .and_then(serde_json::Value::as_str)
                .map(|digest| (module.clone(), digest.to_owned()))
                .ok_or_else(|| format!("code-project module `{module}` has no artifact digest"))
        })
        .collect()
}

fn validate_generated_members(
    project: &CodeProject,
    generated: &KeyedReconcileState,
    authority: &str,
) -> Result<(), String> {
    let desired = required_generated_members(project).map_err(|error| error.to_string())?;
    let actual = generated
        .ordered_members()
        .into_iter()
        .map(|member| member.address)
        .collect::<Vec<_>>();
    if actual == desired {
        Ok(())
    } else {
        Err(format!(
            "{authority} code-session generated provenance does not match managed source"
        ))
    }
}

fn materialize_candidate(
    project: &CodeProject,
    generated: &KeyedReconcileState,
) -> Result<MaterializedCodeProject, String> {
    let (intent, document) = next_materialization_ids()?;
    materialize_code_project_cold(
        project,
        generated,
        intent,
        document,
        CODE_PROJECT_MODEL_SCALE,
    )
    .map_err(|error| error.to_string())
}

fn expansion_for_retained_failure(
    project: &CodeProject,
    generated: &KeyedReconcileState,
    current_overlay: &CodeInteractionOverlay,
    previous: &MaterializedCodeProject,
) -> (
    Option<geosolve_sketch_code::ExpandedCodeProject>,
    CodeInteractionOverlay,
) {
    expand_code_project_for_structural_edit(
        project,
        generated,
        current_overlay,
        previous.editor.coordinator().intent().identity(),
    )
    .map_or_else(
        |_| (None, current_overlay.clone()),
        |(expansion, retained)| (Some(expansion), retained),
    )
}

fn next_materialization_ids() -> Result<(IntentSessionId, DocumentId), String> {
    let ordinal = NEXT_CODE_MATERIALIZATION.fetch_add(1, Ordering::Relaxed);
    if ordinal == u64::MAX {
        return Err("code-project materialization identity is exhausted".into());
    }
    let raw = 0x84_0000_0000_0000_u128 | u128::from(ordinal);
    Ok((
        IntentSessionId::from_raw(raw),
        DocumentId(PersistentId::from_u128(raw)),
    ))
}

fn encode_editor_checkpoint(
    editor: &ProjectionalEditorSession,
) -> Result<serde_json::Value, String> {
    let (computed_evaluation_high_water, revisions) =
        super::persistence::WorkspaceSnapshot::projectional_authority_metadata(editor)?;
    let delegated = super::persistence::WorkspaceSnapshot::from_delegated_projectional_editor(
        editor,
        computed_evaluation_high_water,
        revisions,
    )?;
    delegated.validate_delegated_intent_checkpoint()?;
    delegated.encode().map(serde_json::Value::String)
}

fn validate_editor_checkpoint(checkpoint: &serde_json::Value) -> Result<(), String> {
    restore_editor_checkpoint(checkpoint).map(|_| ())
}

fn restore_editor_checkpoint(
    checkpoint: &serde_json::Value,
) -> Result<Box<ProjectionalEditorSession>, String> {
    let encoded = checkpoint
        .as_str()
        .ok_or_else(|| "code-project editor checkpoint is not encoded text".to_owned())?;
    let snapshot = super::persistence::WorkspaceSnapshot::decode(encoded)?;
    snapshot.validate_delegated_intent_checkpoint()?;
    let editor = Box::new(super::persistence::projectional_editor_from_snapshot(
        &snapshot,
    )?);
    let accepted = editor
        .coordinator()
        .accepted_materialization()
        .ok_or_else(|| "code-project editor checkpoint has no accepted native scene".to_owned())?;
    if !accepted.validation.hard_residuals_validated
        || !accepted.validation.all_active_features_current
        || accepted
            .validation
            .maximum_normalized_hard_residual
            .is_some_and(|value| !value.is_finite() || value > 1.0e-9)
    {
        return Err("code-project editor checkpoint failed independent validation".into());
    }
    Ok(editor)
}

fn rehydrate_editor_checkpoint(
    checkpoint: &serde_json::Value,
    expansion: ExpandedCodeProject,
) -> Result<Box<MaterializedCodeProject>, String> {
    rehydrate_materialized_code_project(restore_editor_checkpoint(checkpoint)?, expansion)
        .map_err(|error| error.to_string())
}

#[allow(
    clippy::too_many_lines,
    reason = "the closed mutation classifier audits every permitted code-owned leaf and rejects all other canvas changes in one place"
)]
fn classify_code_owned_editor_change(
    accepted: &ProjectionalEditorSession,
    candidate: &ProjectionalEditorSession,
    expansion: &ExpandedCodeProject,
) -> Result<Option<CodeOwnedEditorChange>, String> {
    let accepted_intent = accepted.coordinator().intent();
    let candidate_intent = candidate.coordinator().intent();
    let mut accepted_code_nodes = BTreeMap::new();
    for alias in expansion.declaration_provenance.keys() {
        let node = accepted_intent
            .graph()
            .node_by_symbol(alias)
            .ok_or_else(|| format!("accepted code declaration `{alias}` disappeared"))?;
        accepted_code_nodes.insert(alias.clone(), node.id);
    }
    if let Some(node) = accepted_intent.graph().nodes().values().find(|node| {
        node.symbol.as_str().starts_with("code.")
            && !expansion.declaration_provenance.contains_key(&node.symbol)
    }) {
        return Err(format!(
            "code-owned declaration `{}` has no authenticated semantic provenance",
            node.symbol,
        ));
    }
    let candidate_code_symbols = candidate_intent
        .graph()
        .nodes()
        .values()
        .filter(|node| expansion.declaration_provenance.contains_key(&node.symbol))
        .map(|node| node.symbol.clone())
        .collect::<BTreeSet<_>>();
    if let Some(node) = candidate_intent.graph().nodes().values().find(|node| {
        node.symbol.as_str().starts_with("code.")
            && !expansion.declaration_provenance.contains_key(&node.symbol)
    }) {
        return Err(format!(
            "candidate code-owned declaration `{}` has no authenticated semantic provenance",
            node.symbol,
        ));
    }
    if accepted_code_nodes.keys().cloned().collect::<BTreeSet<_>>() != candidate_code_symbols {
        return Err(
            "code-owned declarations cannot be created or deleted in an opaque editor checkpoint"
                .into(),
        );
    }

    let mut changed_leaves = BTreeSet::new();
    for (symbol, accepted_node_id) in &accepted_code_nodes {
        let accepted_node = accepted_intent
            .graph()
            .node(*accepted_node_id)
            .ok_or_else(|| format!("accepted code declaration `{symbol}` disappeared"))?;
        let candidate_node = candidate_intent
            .graph()
            .node_by_symbol(symbol)
            .ok_or_else(|| format!("candidate code declaration `{symbol}` disappeared"))?;
        if accepted_node != candidate_node {
            return Err(
                "this code-owned definition is read-only on canvas; edit managed source or an exposed lens"
                    .into(),
            );
        }
        let accepted_name = accepted_intent
            .organization()
            .node_names()
            .get(accepted_node_id);
        let candidate_name = candidate_intent
            .organization()
            .node_names()
            .get(&candidate_node.id);
        if accepted_name != candidate_name {
            return Err(
                "code-owned declaration names must be changed through managed source".into(),
            );
        }
        let leaves = accepted_intent
            .instance()
            .values()
            .keys()
            .filter(|leaf| leaf.node == *accepted_node_id)
            .chain(
                candidate_intent
                    .instance()
                    .values()
                    .keys()
                    .filter(|leaf| leaf.node == candidate_node.id),
            )
            .copied()
            .collect::<BTreeSet<_>>();
        for leaf in leaves {
            let candidate_leaf = LeafRef {
                node: candidate_node.id,
                ..leaf
            };
            if accepted_intent.instance().values().get(&leaf)
                == candidate_intent.instance().values().get(&candidate_leaf)
            {
                continue;
            }
            changed_leaves.insert(leaf);
        }
    }
    validate_code_organization(accepted, candidate, &accepted_code_nodes)?;

    if changed_leaves.is_empty() {
        return Ok(None);
    }
    let mut covered_leaves = BTreeSet::new();
    let mut placements = Vec::new();
    for point in &expansion.writable_points {
        let leaves = expanded_port_leaves(accepted, &point.handle)?;
        let matched = leaves
            .intersection(&changed_leaves)
            .copied()
            .collect::<BTreeSet<_>>();
        if matched.is_empty() {
            continue;
        }
        covered_leaves.extend(matched);
        let position = expanded_port_position(candidate, &point.handle).ok_or_else(|| {
            format!(
                "semantic code point `{}.{}` has no exact Cartesian instance seed",
                point.handle.alias, point.handle.selector,
            )
        })?;
        if !position.into_iter().all(f64::is_finite) {
            return Err("semantic code point placement is not finite".into());
        }
        placements.push((point.clone(), position));
    }
    if covered_leaves != changed_leaves {
        return Err(
            "this code-owned placement has changed leaves without semantic GUI draft provenance"
                .into(),
        );
    }
    placements.sort_by(|(left, _), (right, _)| left.handle.cmp(&right.handle));
    placements.dedup_by(|(left, left_position), (right, right_position)| {
        left == right && pair_bits(*left_position) == pair_bits(*right_position)
    });
    Ok(Some(CodeOwnedEditorChange::SemanticPoints { placements }))
}

fn validate_code_organization(
    accepted: &ProjectionalEditorSession,
    candidate: &ProjectionalEditorSession,
    code_nodes: &BTreeMap<geosolve_sketch_intent::IntentKey, NodeId>,
) -> Result<(), String> {
    let code_ids = code_nodes.values().copied().collect::<BTreeSet<_>>();
    let signature = |editor: &ProjectionalEditorSession| {
        let organization = editor.coordinator().intent().organization();
        organization
            .cell_order()
            .iter()
            .filter_map(|cell_id| {
                let cell = organization.cells().get(cell_id)?;
                let declarations = cell
                    .declarations
                    .iter()
                    .filter(|node| code_ids.contains(node))
                    .copied()
                    .collect::<Vec<_>>();
                (!declarations.is_empty()).then_some((cell.name.clone(), declarations))
            })
            .collect::<Vec<_>>()
    };
    if signature(accepted) == signature(candidate) {
        Ok(())
    } else {
        Err("code-owned declaration organization must be changed through managed source".into())
    }
}

#[cfg(test)]
fn generated_point_leaves(
    editor: &ProjectionalEditorSession,
    expansion: &ExpandedCodeProject,
) -> Result<BTreeMap<LeafRef, GeneratedMemberAddress>, String> {
    let mut leaves = BTreeMap::new();
    for (address, provenance) in &expansion.generated_provenance {
        if !supported_point_override_address(address) {
            continue;
        }
        let ExpandedSemanticTarget::Port { port } = &provenance.target else {
            continue;
        };
        let node = editor
            .coordinator()
            .intent()
            .graph()
            .node_by_symbol(&port.alias)
            .ok_or_else(|| {
                format!(
                    "generated point `{}` has no intent owner",
                    address.display_path()
                )
            })?;
        let output = node.port_by_selector(port.selector).ok_or_else(|| {
            format!(
                "generated point `{}` has no stable output",
                address.display_path()
            )
        })?;
        for field in [LeafField::X, LeafField::Y] {
            let leaf = LeafRef {
                node: node.id,
                port: output.id,
                field,
            };
            if leaves.insert(leaf, address.clone()).is_some() {
                return Err("generated point leaves are not uniquely owned".into());
            }
        }
    }
    Ok(leaves)
}

#[cfg(test)]
fn managed_rectangle_alias(
    project: &CodeProject,
    expansion: &ExpandedCodeProject,
    declaration: &SemanticSymbol,
) -> Option<geosolve_sketch_intent::IntentKey> {
    let managed = project
        .managed
        .program
        .declarations
        .iter()
        .find(|candidate| {
            candidate.symbol == *declaration
                && candidate.patch.is_none()
                && candidate.builder_path == ["geometry", "rectangle"]
        })?;
    expansion.semantic_outputs.values().find_map(|output| {
        if output.reference.declaration != managed.symbol || !output.reference.output.0.is_empty() {
            return None;
        }
        match &output.target {
            ExpandedSemanticTarget::Declaration {
                alias,
                kind: FeatureKind::Feature,
            } => Some(alias.clone()),
            _ => None,
        }
    })
}

#[cfg(test)]
fn managed_rectangle_leaves(
    editor: &ProjectionalEditorSession,
    project: &CodeProject,
    expansion: &ExpandedCodeProject,
) -> Result<BTreeMap<LeafRef, WritableCodeLeaf>, String> {
    let mut leaves = BTreeMap::new();
    for declaration in &project.managed.program.declarations {
        let Some(alias) = managed_rectangle_alias(project, expansion, &declaration.symbol) else {
            continue;
        };
        let node = editor
            .coordinator()
            .intent()
            .graph()
            .node_by_symbol(&alias)
            .ok_or_else(|| {
                format!(
                    "managed rectangle `{}` has no intent declaration",
                    declaration.symbol.0
                )
            })?;
        if !matches!(
            node.kind,
            IntentNodeKind::Geometry {
                recipe: GeometryRecipeKind::TwoPointAlignedRectangle
            }
        ) {
            return Err(format!(
                "managed rectangle `{}` has the wrong intent recipe",
                declaration.symbol.0
            ));
        }
        for (index, argument) in [(0, "lowerLeft"), (2, "upperRight")] {
            let port = node
                .port_by_selector(IntentPortSelector::Node {
                    role: IntentPortRole::Corner,
                    index,
                })
                .ok_or_else(|| {
                    format!(
                        "managed rectangle `{}` has no writable `{argument}` port",
                        declaration.symbol.0
                    )
                })?;
            for field in [LeafField::X, LeafField::Y] {
                let leaf = LeafRef {
                    node: node.id,
                    port: port.id,
                    field,
                };
                let owner = WritableCodeLeaf::ManagedRectangle {
                    declaration: declaration.symbol.clone(),
                    alias: alias.clone(),
                    argument: argument.into(),
                };
                if leaves.insert(leaf, owner).is_some() {
                    return Err("managed rectangle leaves are not uniquely owned".into());
                }
            }
        }
    }
    Ok(leaves)
}

#[cfg(test)]
fn managed_rectangle_argument_position(
    editor: &ProjectionalEditorSession,
    alias: &geosolve_sketch_intent::IntentKey,
    argument: &str,
) -> Option<[f64; 2]> {
    let index = match argument {
        "lowerLeft" => 0,
        "upperRight" => 2,
        _ => return None,
    };
    let intent = editor.coordinator().intent();
    let node = intent.graph().node_by_symbol(alias)?;
    let port = node.port_by_selector(IntentPortSelector::Node {
        role: IntentPortRole::Corner,
        index,
    })?;
    let coordinate = |field| match intent.instance().values().get(&LeafRef {
        node: node.id,
        port: port.id,
        field,
    })? {
        IntentLiteral::Quantity {
            value,
            unit: IntentUnit::Length,
        } => Some(*value),
        _ => None,
    };
    Some([coordinate(LeafField::X)?, coordinate(LeafField::Y)?])
}

fn expanded_port_leaves(
    editor: &ProjectionalEditorSession,
    handle: &ExpandedPort,
) -> Result<BTreeSet<LeafRef>, String> {
    let node = editor
        .coordinator()
        .intent()
        .graph()
        .node_by_symbol(&handle.alias)
        .ok_or_else(|| format!("semantic point owner `{}` is absent", handle.alias))?;
    let port = node.port_by_selector(handle.selector).ok_or_else(|| {
        format!(
            "semantic point owner `{}` has no `{}` output",
            handle.alias, handle.selector,
        )
    })?;
    if handle.kind != IntentPortKind::Point || port.kind != handle.kind {
        return Err(format!(
            "semantic output `{}.{}` is not a Cartesian point",
            handle.alias, handle.selector,
        ));
    }
    Ok([LeafField::X, LeafField::Y]
        .into_iter()
        .map(|field| LeafRef {
            node: node.id,
            port: port.id,
            field,
        })
        .collect())
}

fn expanded_port_instance(
    editor: &ProjectionalEditorSession,
    handle: &ExpandedPort,
) -> Option<[f64; 2]> {
    let intent = editor.coordinator().intent();
    let node = intent.graph().node_by_symbol(&handle.alias)?;
    let port = node.port_by_selector(handle.selector)?;
    if handle.kind != IntentPortKind::Point || port.kind != handle.kind {
        return None;
    }
    let coordinate = |field| match intent.instance().values().get(&LeafRef {
        node: node.id,
        port: port.id,
        field,
    })? {
        IntentLiteral::Quantity {
            value,
            unit: IntentUnit::Length,
        } if value.is_finite() => Some(*value),
        _ => None,
    };
    Some([coordinate(LeafField::X)?, coordinate(LeafField::Y)?])
}

fn expanded_port_position(
    editor: &ProjectionalEditorSession,
    handle: &ExpandedPort,
) -> Option<[f64; 2]> {
    if let Some(position) = expanded_port_instance(editor, handle) {
        return Some(position);
    }
    let point = expanded_port_point(editor, handle)?;
    let position = editor
        .coordinator()
        .accepted_materialization()?
        .session
        .design_document()
        .point(point)?
        .position;
    position.into_iter().all(f64::is_finite).then_some(position)
}

fn expanded_port_point(
    editor: &ProjectionalEditorSession,
    handle: &ExpandedPort,
) -> Option<geosolve_sketch::DesignPointId> {
    if handle.kind != IntentPortKind::Point {
        return None;
    }
    let intent = editor.coordinator().intent();
    let node = intent.graph().node_by_symbol(&handle.alias)?;
    let port = node.port_by_selector(handle.selector)?;
    if port.kind != handle.kind {
        return None;
    }
    match editor
        .coordinator()
        .accepted_materialization()?
        .ownership
        .port(port.as_ref(node.id))?
    {
        IntentNativeBinding::Point(point) => Some(point),
        _ => None,
    }
}

fn feature_documents_match_for_terminal_parity(
    terminal: &ComputedFeatureDocument,
    staged: &ComputedFeatureDocument,
) -> bool {
    let terminal_identity = terminal.identity();
    let staged_identity = staged.identity();
    terminal_identity.document == staged_identity.document
        && terminal_identity.sketch_document == staged_identity.sketch_document
        && terminal.allocator_high_water() == staged.allocator_high_water()
        && terminal.features().len() == staged.features().len()
        && terminal
            .features()
            .iter()
            .zip(staged.features())
            .all(|(terminal, staged)| {
                if terminal.id != staged.id
                    || terminal.label != staged.label
                    || terminal.suppressed != staged.suppressed
                {
                    return false;
                }
                let (
                    ComputedFeatureDefinition::FilletSet(terminal),
                    ComputedFeatureDefinition::FilletSet(staged),
                ) = (&terminal.definition, &staged.definition);
                terminal.radius.to_bits() == staged.radius.to_bits()
                    && terminal.corners.len() == staged.corners.len()
                    && terminal
                        .corners
                        .iter()
                        .zip(&staged.corners)
                        .all(|(terminal, staged)| {
                            terminal.id == staged.id
                                && terminal.endpoint_order == staged.endpoint_order
                                && terminal.sweep == staged.sweep
                                && [
                                    (terminal.first, staged.first),
                                    (terminal.second, staged.second),
                                ]
                                .into_iter()
                                .all(|(terminal, staged)| {
                                    // `picked_parameter` is a recomputable
                                    // seed, not a branch cell. Every durable
                                    // owner, winding, neighborhood, normal,
                                    // endpoint and periodic anchor stays exact.
                                    terminal.source == staged.source
                                        && terminal.winding == staged.winding
                                        && terminal.neighborhood == staged.neighborhood
                                        && terminal.normal_side == staged.normal_side
                                        && terminal.retained_endpoint == staged.retained_endpoint
                                        && terminal.periodic_anchor == staged.periodic_anchor
                                })
                        })
            })
}

fn computed_snapshots_match_for_terminal_parity(
    terminal: &ComputedFeatureSnapshot,
    staged: &ComputedFeatureSnapshot,
) -> bool {
    terminal.edges().len() == staged.edges().len()
        && terminal
            .edges()
            .iter()
            .zip(staged.edges())
            .all(|(terminal, staged)| {
                terminal.id.ordinal == staged.id.ordinal
                    && terminal.role == staged.role
                    && terminal.geometry == staged.geometry
                    && terminal.provenance == staged.provenance
            })
        && terminal.construction_fragments().len() == staged.construction_fragments().len()
        && terminal
            .construction_fragments()
            .iter()
            .zip(staged.construction_fragments())
            .all(|(terminal, staged)| {
                terminal.id.ordinal == staged.id.ordinal
                    && terminal.source == staged.source
                    && terminal.interval == staged.interval
                    && terminal.source_role == staged.source_role
                    && terminal.provenance == staged.provenance
            })
        && terminal.replaced_sources() == staged.replaced_sources()
        && terminal.feature_evaluations().len() == staged.feature_evaluations().len()
        && terminal
            .feature_evaluations()
            .iter()
            .zip(staged.feature_evaluations())
            .all(|(terminal, staged)| {
                if terminal.feature != staged.feature {
                    return false;
                }
                match (&terminal.state, &staged.state) {
                    (
                        ComputedFeatureEvaluationState::Current {
                            corner_edges: terminal,
                        },
                        ComputedFeatureEvaluationState::Current {
                            corner_edges: staged,
                        },
                    ) => {
                        terminal.len() == staged.len()
                            && terminal.iter().zip(staged).all(
                                |(
                                    (terminal_corner, terminal_edge),
                                    (staged_corner, staged_edge),
                                )| {
                                    terminal_corner == staged_corner
                                        && terminal_edge.ordinal == staged_edge.ordinal
                                },
                            )
                    }
                    (
                        ComputedFeatureEvaluationState::Failed { failure: terminal },
                        ComputedFeatureEvaluationState::Failed { failure: staged },
                    ) => terminal == staged,
                    (
                        ComputedFeatureEvaluationState::Suppressed,
                        ComputedFeatureEvaluationState::Suppressed,
                    ) => true,
                    _ => false,
                }
            })
}

fn documents_match_for_terminal_parity(
    terminal_editor: &ProjectionalEditorSession,
    staged_editor: &ProjectionalEditorSession,
    terminal: &geosolve_sketch::SketchDocument,
    staged: &geosolve_sketch::SketchDocument,
    rectangle_projections: &[RectangleTerminalProjection],
    recomputable_line_branches: &BTreeSet<geosolve_sketch::CurveId>,
) -> Result<bool, String> {
    if terminal.exact_except_recomputable_line_branches(staged, recomputable_line_branches) {
        return Ok(true);
    }
    let resolve = |editor: &ProjectionalEditorSession, handle: &ExpandedPort| {
        expanded_port_point(editor, handle)
            .ok_or_else(|| "rectangle parity lens has no accepted native point binding".to_owned())
    };
    let mut anchors = BTreeSet::new();
    let mut redundant = BTreeSet::new();
    for projection in rectangle_projections {
        for handle in &projection.anchors {
            let terminal_point = resolve(terminal_editor, handle)?;
            if terminal_point != resolve(staged_editor, handle)? {
                return Ok(false);
            }
            if !anchors.insert(terminal_point) {
                return Err("rectangle parity repeats one anchor point".into());
            }
        }
        for handle in &projection.redundant_aliases {
            let terminal_point = resolve(terminal_editor, handle)?;
            if terminal_point != resolve(staged_editor, handle)? {
                return Ok(false);
            }
            if !redundant.insert(terminal_point) {
                return Err("rectangle parity repeats one redundant point".into());
            }
        }
    }
    if !anchors.is_disjoint(&redundant) {
        return Err("rectangle parity anchor aliases a redundant point".into());
    }
    let mut normalized = terminal.clone();
    for point in anchors {
        let terminal_position = terminal
            .point(point)
            .ok_or_else(|| "terminal rectangle anchor disappeared".to_owned())?
            .position;
        let staged_position = staged
            .point(point)
            .ok_or_else(|| "staged rectangle anchor disappeared".to_owned())?
            .position;
        if pair_bits(terminal_position) != pair_bits(staged_position) {
            return Ok(false);
        }
    }
    for point in redundant {
        let terminal_position = terminal
            .point(point)
            .ok_or_else(|| "terminal redundant rectangle point disappeared".to_owned())?
            .position;
        let staged_position = staged
            .point(point)
            .ok_or_else(|| "staged redundant rectangle point disappeared".to_owned())?
            .position;
        if pair_bits(terminal_position) == pair_bits(staged_position) {
            continue;
        }
        if !point_seed_roundoff_compatible(terminal_position, staged_position) {
            return Ok(false);
        }
        normalized
            .set_point_position(point, staged_position)
            .map_err(|error| format!("rectangle parity normalization failed: {error}"))?;
    }
    Ok(normalized.exact_except_recomputable_line_branches(staged, recomputable_line_branches))
}

fn validate_terminal_native_parity(
    terminal: &ProjectionalEditorSession,
    staged: &ProjectionalEditorSession,
    expansion: &ExpandedCodeProject,
    rectangle_projections: &[RectangleTerminalProjection],
) -> Result<(), String> {
    let terminal_authority = terminal
        .coordinator()
        .accepted_materialization()
        .ok_or_else(|| "terminal code drag has no accepted native authority".to_owned())?;
    let staged_authority = staged
        .coordinator()
        .accepted_materialization()
        .ok_or_else(|| "staged code drag has no accepted native authority".to_owned())?;
    let mut recomputable = recomputable_code_line_branches(terminal, expansion)?;
    recomputable.extend(recomputable_code_line_branches(staged, expansion)?);
    let terminal_accepted = terminal_authority
        .session
        .accepted_state_for_current_input()
        .ok_or_else(|| "terminal code drag has no current accepted document".to_owned())?
        .document();
    let staged_accepted = staged_authority
        .session
        .accepted_state_for_current_input()
        .ok_or_else(|| "staged code drag has no current accepted document".to_owned())?
        .document();
    let same_documents = documents_match_for_terminal_parity(
        terminal,
        staged,
        terminal_authority.session.design_document(),
        staged_authority.session.design_document(),
        rectangle_projections,
        &recomputable,
    )? && documents_match_for_terminal_parity(
        terminal,
        staged,
        terminal_accepted,
        staged_accepted,
        rectangle_projections,
        &recomputable,
    )?;
    let same_features = feature_documents_match_for_terminal_parity(
        &terminal_authority.features,
        &staged_authority.features,
    ) && computed_snapshots_match_for_terminal_parity(
        &terminal_authority.computed,
        &staged_authority.computed,
    );
    // Revision/digest stamps and Fillet pick seeds can refresh when staged
    // source is canonically rematerialized. Durable branch cells, complete
    // evaluated geometry, ownership rows and persistent IDs stay exact.
    let same_ownership = terminal_authority.ownership.nodes == staged_authority.ownership.nodes
        && terminal_authority.ownership.ports == staged_authority.ownership.ports
        && terminal_authority.ownership.reservations == staged_authority.ownership.reservations
        && terminal_authority.ownership.writable_leaves
            == staged_authority.ownership.writable_leaves
        && terminal_authority.ownership.aggregates == staged_authority.ownership.aggregates;
    let mut differences = Vec::new();
    if !same_documents {
        differences.push("sketch documents");
    }
    if !same_features {
        differences.push("computed features");
    }
    if !same_ownership {
        differences.push("native ownership");
    }
    if terminal_authority.feature_lifecycle_high_water.allocator
        != staged_authority.feature_lifecycle_high_water.allocator
    {
        differences.push("feature allocator");
    }
    if terminal_authority.session.persistent_identity_high_water()
        != staged_authority.session.persistent_identity_high_water()
    {
        differences.push("sketch allocator");
    }
    if !differences.is_empty() {
        return Err(format!(
            "terminal code drag differs from its independently staged native authority in {}",
            differences.join(", ")
        ));
    }
    Ok(())
}

fn recomputable_code_line_branches(
    editor: &ProjectionalEditorSession,
    expansion: &ExpandedCodeProject,
) -> Result<BTreeSet<geosolve_sketch::CurveId>, String> {
    let intent = editor.coordinator().intent();
    let accepted = editor
        .coordinator()
        .accepted_materialization()
        .ok_or_else(|| "code project has no accepted native authority".to_owned())?;
    let expansion_owned_segments = expansion
        .patch
        .operations()
        .iter()
        .filter_map(|operation| match operation {
            geosolve_sketch_intent::IntentPatchOperation::CreateNode { draft, .. }
                if matches!(
                    draft.kind,
                    IntentNodeKind::Geometry {
                        recipe: GeometryRecipeKind::Segment
                    }
                ) =>
            {
                Some(draft.symbol.clone())
            }
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    let mut curves = BTreeSet::new();
    for node in intent.graph().nodes().values() {
        let IntentNodeKind::Geometry { recipe } = node.kind else {
            continue;
        };
        // M83 Segment branches remain explicit. Only the exact current code
        // expansion proves that a Segment came from a managed/artifact
        // declaration whose branch is source-derived. An ordinary GUI Segment
        // living beside a code project must still compare bit-for-bit.
        let source_derived_segment = recipe == GeometryRecipeKind::Segment
            && expansion_owned_segments.contains(&node.symbol);
        if !source_derived_segment
            && !matches!(
                recipe,
                GeometryRecipeKind::Polyline
                    | GeometryRecipeKind::TwoPointAlignedRectangle
                    | GeometryRecipeKind::ThreePointCornerRectangle
                    | GeometryRecipeKind::CenterRectangle
                    | GeometryRecipeKind::ThreePointCenterRectangle
            )
        {
            continue;
        }
        if let Some(ownership) = accepted.ownership.node(node.id) {
            curves.extend(ownership.owned.iter().filter_map(|binding| match binding {
                IntentNativeBinding::Curve(curve) => Some(*curve),
                _ => None,
            }));
        }
    }
    for (address, provenance) in &expansion.generated_provenance {
        if address.template != ["polyline", "segment"] {
            continue;
        }
        let ExpandedSemanticTarget::Port { port } = &provenance.target else {
            continue;
        };
        let node = intent
            .graph()
            .node_by_symbol(&port.alias)
            .ok_or_else(|| format!("generated segment `{}` disappeared", address.display_path()))?;
        let output = node.port_by_selector(port.selector).ok_or_else(|| {
            format!(
                "generated segment `{}` lost its output",
                address.display_path()
            )
        })?;
        match accepted.ownership.port(output.as_ref(node.id)) {
            Some(IntentNativeBinding::CurveSpan(span)) => {
                curves.insert(span.curve);
            }
            Some(IntentNativeBinding::Curve(curve)) => {
                curves.insert(curve);
            }
            _ => {
                return Err(format!(
                    "generated segment `{}` has no native curve",
                    address.display_path()
                ));
            }
        }
    }
    Ok(curves)
}

#[cfg(test)]
fn supported_point_override_address(address: &GeneratedMemberAddress) -> bool {
    address.template == ["polyline", "vertex"] && address.output == ["point"]
}

fn pair_bits(value: [f64; 2]) -> [u64; 2] {
    [value[0].to_bits(), value[1].to_bits()]
}

fn managed_path_text(path: &[geosolve_sketch_code::ManagedPathSegment]) -> String {
    let mut text = String::new();
    for segment in path {
        match segment {
            geosolve_sketch_code::ManagedPathSegment::Field(field) => {
                if !text.is_empty() {
                    text.push('.');
                }
                text.push_str(field);
            }
            geosolve_sketch_code::ManagedPathSegment::Index(index) => {
                let _ = write!(text, "[{index}]");
            }
            geosolve_sketch_code::ManagedPathSegment::Member { member } => {
                let _ = write!(text, "[{member}]");
            }
        }
    }
    if text.is_empty() {
        "value".into()
    } else {
        text
    }
}

fn managed_object_path<'a>(value: &'a ManagedValue, path: &[String]) -> Option<&'a ManagedValue> {
    path.iter().try_fold(value, |value, field| {
        let ManagedValue::Object(object) = value else {
            return None;
        };
        object.get(field)
    })
}

fn managed_length_value(value: &ManagedValue) -> Option<f64> {
    match value {
        ManagedValue::Number(value) if value.is_finite() => Some(*value),
        ManagedValue::Unit(UnitLiteral { unit, value }) if unit == "mm" && value.is_finite() => {
            Some(*value)
        }
        _ => None,
    }
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn escape_attribute(value: &str) -> String {
    escape_html(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rectangle_terminal_roundoff_contract_is_tight_and_signed_zero_exact() {
        let seed = 1.0_f64;
        let within = f64::from_bits(seed.to_bits() + TERMINAL_SEED_ROUNDOFF_ULPS);
        let outside = f64::from_bits(seed.to_bits() + TERMINAL_SEED_ROUNDOFF_ULPS + 1);
        assert!(scalar_seed_roundoff_compatible(seed, seed));
        assert!(scalar_seed_roundoff_compatible(seed, within));
        assert!(!scalar_seed_roundoff_compatible(seed, outside));
        assert!(scalar_seed_roundoff_compatible(
            3.0 * f64::EPSILON,
            -2.0 * f64::EPSILON,
        ));
        assert!(!scalar_seed_roundoff_compatible(
            2.0 * TERMINAL_SEED_ZERO_ROUNDOFF,
            0.0,
        ));
        assert!(!scalar_seed_roundoff_compatible(0.0, -0.0));
        assert!(!scalar_seed_roundoff_compatible(f64::NAN, f64::NAN));
        assert!(!scalar_seed_roundoff_compatible(
            f64::INFINITY,
            f64::INFINITY,
        ));
    }

    fn ordinary_rectangle_diagonal() -> ProjectionalEditorSession {
        let intent = IntentSession::with_id(IntentSessionId::from_raw(0x84_f003)).unwrap();
        let mut editor = ProjectionalEditorSession::restore(
            intent,
            DocumentId(PersistentId::from_u128(0x84_f003)),
            1.0,
        )
        .unwrap();
        add_ordinary_rectangle_diagonal(&mut editor);
        editor
    }

    #[allow(
        clippy::too_many_lines,
        reason = "the exact mouse-shaped F004 fixture keeps both inferred axis constraints and its computed Fillet together"
    )]
    fn ordinary_line_fillet() -> ProjectionalEditorSession {
        use geosolve_constraint_editor::{
            FeatureAuthoringOptions, FeatureAuthoringOutcome, FeatureAuthoringState,
            FeatureAuthoringTool,
        };
        use geosolve_sketch_intent::{InputRole, InputSlot, PatchPortRef};

        let intent = IntentSession::with_id(IntentSessionId::from_raw(0x84_f004)).unwrap();
        let mut editor = ProjectionalEditorSession::restore(
            intent,
            DocumentId(PersistentId::from_u128(0x84_f004)),
            1.0,
        )
        .unwrap();
        let selector = |role| IntentPortSelector::Node { role, index: 0 };
        let length = |value| IntentLiteral::Quantity {
            value,
            unit: IntentUnit::Length,
        };
        let first_alias = geosolve_sketch_intent::IntentKey::new("gui-first-line").unwrap();
        let first = geosolve_sketch_intent::IntentNodeDraft::new(
            IntentNodeKind::Geometry {
                recipe: GeometryRecipeKind::Segment,
            },
            geosolve_sketch_intent::IntentKey::new("gui.firstLine").unwrap(),
        )
        .with_instance_leaf(selector(IntentPortRole::Start), LeafField::X, length(0.0))
        .with_instance_leaf(selector(IntentPortRole::Start), LeafField::Y, length(0.0))
        .with_instance_leaf(selector(IntentPortRole::End), LeafField::X, length(4.0))
        .with_instance_leaf(selector(IntentPortRole::End), LeafField::Y, length(0.0));
        let first = editor
            .apply_patch(geosolve_sketch_intent::IntentPatch::new(
                editor.coordinator().intent().identity(),
                geosolve_sketch_intent::IntentPatchPolicy::RequireAccepted,
                vec![geosolve_sketch_intent::IntentPatchOperation::CreateNode {
                    alias: first_alias.clone(),
                    draft: Box::new(first),
                    cell: None,
                }],
            ))
            .unwrap();
        let first_node = first.aliases.node(&first_alias).unwrap();
        let first_span = editor
            .coordinator()
            .intent()
            .graph()
            .node(first_node)
            .unwrap()
            .port_by_selector(selector(IntentPortRole::Span))
            .unwrap()
            .as_ref(first_node);
        let horizontal_alias =
            geosolve_sketch_intent::IntentKey::new("gui-first-horizontal").unwrap();
        let horizontal = geosolve_sketch_intent::IntentNodeDraft::new(
            IntentNodeKind::Constraint {
                constraint: geosolve_sketch_intent::ConstraintKind::Horizontal,
            },
            geosolve_sketch_intent::IntentKey::new("gui.firstHorizontal").unwrap(),
        )
        .with_input(
            InputSlot::new(InputRole::Span, 0),
            PatchPortRef::Stable { port: first_span },
        );
        editor
            .apply_patch(geosolve_sketch_intent::IntentPatch::new(
                editor.coordinator().intent().identity(),
                geosolve_sketch_intent::IntentPatchPolicy::RequireAccepted,
                vec![geosolve_sketch_intent::IntentPatchOperation::CreateNode {
                    alias: horizontal_alias,
                    draft: Box::new(horizontal),
                    cell: None,
                }],
            ))
            .unwrap();
        let shared = editor
            .coordinator()
            .intent()
            .graph()
            .node(first_node)
            .unwrap()
            .port_by_selector(selector(IntentPortRole::End))
            .unwrap()
            .as_ref(first_node);

        let second_alias = geosolve_sketch_intent::IntentKey::new("gui-second-line").unwrap();
        let second = geosolve_sketch_intent::IntentNodeDraft::new(
            IntentNodeKind::Geometry {
                recipe: GeometryRecipeKind::Segment,
            },
            geosolve_sketch_intent::IntentKey::new("gui.secondLine").unwrap(),
        )
        .with_input(
            InputSlot::new(InputRole::Point, 0),
            PatchPortRef::Stable { port: shared },
        )
        .with_instance_leaf(selector(IntentPortRole::End), LeafField::X, length(4.0))
        .with_instance_leaf(selector(IntentPortRole::End), LeafField::Y, length(4.0));
        let second = editor
            .apply_patch(geosolve_sketch_intent::IntentPatch::new(
                editor.coordinator().intent().identity(),
                geosolve_sketch_intent::IntentPatchPolicy::RequireAccepted,
                vec![geosolve_sketch_intent::IntentPatchOperation::CreateNode {
                    alias: second_alias.clone(),
                    draft: Box::new(second),
                    cell: None,
                }],
            ))
            .unwrap();
        let second_node = second.aliases.node(&second_alias).unwrap();
        let second_span = editor
            .coordinator()
            .intent()
            .graph()
            .node(second_node)
            .unwrap()
            .port_by_selector(selector(IntentPortRole::Span))
            .unwrap()
            .as_ref(second_node);
        let vertical_alias = geosolve_sketch_intent::IntentKey::new("gui-second-vertical").unwrap();
        let vertical = geosolve_sketch_intent::IntentNodeDraft::new(
            IntentNodeKind::Constraint {
                constraint: geosolve_sketch_intent::ConstraintKind::Vertical,
            },
            geosolve_sketch_intent::IntentKey::new("gui.secondVertical").unwrap(),
        )
        .with_input(
            InputSlot::new(InputRole::Span, 0),
            PatchPortRef::Stable { port: second_span },
        );
        editor
            .apply_patch(geosolve_sketch_intent::IntentPatch::new(
                editor.coordinator().intent().identity(),
                geosolve_sketch_intent::IntentPatchPolicy::RequireAccepted,
                vec![geosolve_sketch_intent::IntentPatchOperation::CreateNode {
                    alias: vertical_alias,
                    draft: Box::new(vertical),
                    cell: None,
                }],
            ))
            .unwrap();
        let accepted = editor.coordinator().accepted_materialization().unwrap();
        let IntentNativeBinding::Point(shared_point) = accepted.ownership.port(shared).unwrap()
        else {
            panic!("shared line endpoint must own one native point")
        };
        let mut authoring = FeatureAuthoringState::default();
        let symbol = geosolve_sketch_intent::IntentKey::new("gui.fillet").unwrap();
        let outcome = editor
            .activate_feature_authoring(
                &mut authoring,
                FeatureAuthoringTool::Fillet,
                FeatureAuthoringOptions {
                    fillet_radius: Some(1.0),
                    ..FeatureAuthoringOptions::default()
                },
                &[(SelectionItem::Point(shared_point), None)],
                symbol.clone(),
            )
            .unwrap();
        assert!(matches!(
            outcome,
            FeatureAuthoringOutcome::PreviewRequested { .. }
        ));
        editor
            .apply_computed_fillet_preview(&mut authoring, symbol)
            .unwrap();
        editor
    }

    fn add_ordinary_rectangle_diagonal(editor: &mut ProjectionalEditorSession) {
        let selector = |role, index| IntentPortSelector::Node { role, index };
        let length = |value| IntentLiteral::Quantity {
            value,
            unit: IntentUnit::Length,
        };
        let frame_alias = geosolve_sketch_intent::IntentKey::new("gui-frame").unwrap();
        let frame = geosolve_sketch_intent::IntentNodeDraft::new(
            IntentNodeKind::Geometry {
                recipe: GeometryRecipeKind::TwoPointAlignedRectangle,
            },
            geosolve_sketch_intent::IntentKey::new("gui.frame").unwrap(),
        )
        .with_instance_leaf(
            selector(IntentPortRole::Corner, 0),
            LeafField::X,
            length(-4.0),
        )
        .with_instance_leaf(
            selector(IntentPortRole::Corner, 0),
            LeafField::Y,
            length(-3.0),
        )
        .with_instance_leaf(
            selector(IntentPortRole::Corner, 2),
            LeafField::X,
            length(8.0),
        )
        .with_instance_leaf(
            selector(IntentPortRole::Corner, 2),
            LeafField::Y,
            length(5.0),
        );
        let frame = editor
            .apply_patch(geosolve_sketch_intent::IntentPatch::new(
                editor.coordinator().intent().identity(),
                geosolve_sketch_intent::IntentPatchPolicy::RequireAccepted,
                vec![geosolve_sketch_intent::IntentPatchOperation::CreateNode {
                    alias: frame_alias.clone(),
                    draft: Box::new(frame),
                    cell: None,
                }],
            ))
            .unwrap();
        let frame_node = frame.aliases.node(&frame_alias).unwrap();
        let lower_left = editor
            .coordinator()
            .intent()
            .graph()
            .node(frame_node)
            .unwrap()
            .port_by_selector(selector(IntentPortRole::Corner, 0))
            .unwrap()
            .as_ref(frame_node);
        let upper_right = editor
            .coordinator()
            .intent()
            .graph()
            .node(frame_node)
            .unwrap()
            .port_by_selector(selector(IntentPortRole::Corner, 2))
            .unwrap()
            .as_ref(frame_node);
        let diagonal_alias = geosolve_sketch_intent::IntentKey::new("gui-diagonal").unwrap();
        let diagonal = geosolve_sketch_intent::IntentNodeDraft::new(
            IntentNodeKind::Geometry {
                recipe: GeometryRecipeKind::Segment,
            },
            geosolve_sketch_intent::IntentKey::new("gui.diagonal").unwrap(),
        )
        .with_input(
            geosolve_sketch_intent::InputSlot::new(geosolve_sketch_intent::InputRole::Point, 0),
            geosolve_sketch_intent::PatchPortRef::Stable { port: lower_left },
        )
        .with_input(
            geosolve_sketch_intent::InputSlot::new(geosolve_sketch_intent::InputRole::Point, 1),
            geosolve_sketch_intent::PatchPortRef::Stable { port: upper_right },
        );
        editor
            .apply_patch(geosolve_sketch_intent::IntentPatch::new(
                editor.coordinator().intent().identity(),
                geosolve_sketch_intent::IntentPatchPolicy::RequireAccepted,
                vec![geosolve_sketch_intent::IntentPatchOperation::CreateNode {
                    alias: diagonal_alias,
                    draft: Box::new(diagonal),
                    cell: None,
                }],
            ))
            .unwrap();
    }

    fn open_with_editor(key: &str) -> (CodeProjectWorkbench, Box<ProjectionalEditorSession>) {
        CodeProjectWorkbench::open_key(key).expect("code project")
    }

    fn open(key: &str) -> CodeProjectWorkbench {
        open_with_editor(key).0
    }

    fn open_boxed(key: &str) -> (Box<CodeProjectWorkbench>, Box<ProjectionalEditorSession>) {
        let (workbench, editor) = open_with_editor(key);
        (Box::new(workbench), editor)
    }

    fn code_node_ids(
        editor: &ProjectionalEditorSession,
    ) -> BTreeMap<geosolve_sketch_intent::IntentKey, NodeId> {
        editor
            .coordinator()
            .intent()
            .graph()
            .nodes()
            .values()
            .filter(|node| node.symbol.as_str().starts_with("code."))
            .map(|node| (node.symbol.clone(), node.id))
            .collect()
    }

    fn generated_native_bindings(
        editor: &ProjectionalEditorSession,
        expansion: &ExpandedCodeProject,
    ) -> BTreeMap<GeneratedMemberAddress, (NodeId, IntentNativeBinding)> {
        let accepted = editor
            .coordinator()
            .accepted_materialization()
            .expect("generated native authority");
        expansion
            .generated_provenance
            .iter()
            .filter_map(|(address, provenance)| {
                let ExpandedSemanticTarget::Port { port } = &provenance.target else {
                    return None;
                };
                let node = editor
                    .coordinator()
                    .intent()
                    .graph()
                    .node_by_symbol(&port.alias)
                    .expect("generated declaration");
                let output = node
                    .port_by_selector(port.selector)
                    .expect("generated output");
                Some((
                    address.clone(),
                    (
                        node.id,
                        accepted
                            .ownership
                            .port(output.as_ref(node.id))
                            .expect("generated native binding"),
                    ),
                ))
            })
            .collect()
    }

    fn generated_fillet_owners(
        workbench: &CodeProjectWorkbench,
    ) -> BTreeMap<GeneratedMemberAddress, Vec<geosolve_constraint_editor::ComputedCornerRef>> {
        workbench
            .materialized
            .as_deref()
            .expect("warm code authority")
            .host_outputs
            .iter()
            .map(|(address, outputs)| {
                (
                    address.clone(),
                    outputs.iter().map(|output| output.owner).collect(),
                )
            })
            .collect()
    }

    fn native_binding_for_node(
        editor: &ProjectionalEditorSession,
        node: NodeId,
        predicate: impl Fn(IntentNativeBinding) -> bool,
    ) -> IntentNativeBinding {
        editor
            .coordinator()
            .accepted_materialization()
            .expect("accepted native authority")
            .ownership
            .node(node)
            .expect("native declaration owner")
            .owned
            .iter()
            .copied()
            .find(|binding| predicate(*binding))
            .expect("requested native binding")
    }

    fn curve_definition_for_binding(
        editor: &ProjectionalEditorSession,
        binding: IntentNativeBinding,
    ) -> geosolve_sketch::CurveDefinition {
        let curve = match binding {
            IntentNativeBinding::Curve(curve) => curve,
            IntentNativeBinding::CurveSpan(span) => span.curve,
            _ => panic!("binding is not native curve geometry"),
        };
        editor
            .coordinator()
            .accepted_materialization()
            .expect("accepted curve authority")
            .session
            .design_document()
            .curve(curve)
            .expect("owned native curve")
            .definition
            .clone()
    }

    fn assert_warm_cache_matches_session(workbench: &CodeProjectWorkbench) {
        let materialized = workbench.materialized.as_deref().expect("warm authority");
        assert_eq!(
            &materialized.expansion,
            workbench
                .session
                .snapshot()
                .accepted_expansion
                .as_ref()
                .expect("accepted expansion"),
        );
        assert_eq!(
            encode_editor_checkpoint(&materialized.editor).unwrap(),
            *workbench.accepted_editor_checkpoint(),
        );
    }

    fn add_gui_horizontal_on_generated_span(
        editor: &mut ProjectionalEditorSession,
        expansion: &ExpandedCodeProject,
        member_key: &str,
        symbol: &str,
        suppressed: bool,
    ) -> NodeId {
        let (_, provenance) = expansion
            .generated_provenance
            .iter()
            .find(|(address, _)| {
                address.template == ["polyline", "segment"] && address.member_key == [member_key]
            })
            .expect("generated Polyline span");
        let ExpandedSemanticTarget::Port { port } = &provenance.target else {
            panic!("generated Polyline segment must own one stable span port")
        };
        let owner = editor
            .coordinator()
            .intent()
            .graph()
            .node_by_symbol(&port.alias)
            .expect("generated span declaration");
        let span = owner
            .port_by_selector(port.selector)
            .expect("generated span output")
            .as_ref(owner.id);
        let mut draft = geosolve_sketch_intent::IntentNodeDraft::new(
            geosolve_sketch_intent::IntentNodeKind::Constraint {
                constraint: geosolve_sketch_intent::ConstraintKind::Horizontal,
            },
            geosolve_sketch_intent::IntentKey::new(symbol).unwrap(),
        )
        .with_input(
            geosolve_sketch_intent::InputSlot::new(geosolve_sketch_intent::InputRole::Span, 0),
            geosolve_sketch_intent::PatchPortRef::Stable { port: span },
        );
        draft.suppressed = suppressed;
        let alias = geosolve_sketch_intent::IntentKey::new("gui-dependent").unwrap();
        let outcome = editor
            .apply_patch(geosolve_sketch_intent::IntentPatch::new(
                editor.coordinator().intent().identity(),
                geosolve_sketch_intent::IntentPatchPolicy::RequireAccepted,
                vec![geosolve_sketch_intent::IntentPatchOperation::CreateNode {
                    alias: alias.clone(),
                    draft: Box::new(draft),
                    cell: None,
                }],
            ))
            .expect("ordinary GUI dependent");
        outcome.aliases.node(&alias).expect("ordinary GUI node")
    }

    #[test]
    fn all_nine_samples_open_with_nonempty_independently_validated_native_canvases() {
        let demos = bundled_code_project_demos();
        assert_eq!(
            demos.len(),
            9,
            "the curated M84 catalog includes the manifold dogfood demo"
        );
        for demo in demos {
            let (workbench, editor) = open_with_editor(demo.id.key());
            let accepted = editor
                .coordinator()
                .accepted_materialization()
                .expect("code sample owns accepted native authority");
            let design = accepted.session.design_document();
            assert!(
                !design.points().is_empty() && !design.curves().is_empty(),
                "{} opened an empty native canvas",
                demo.id.key(),
            );
            assert!(accepted.validation.hard_residuals_validated);
            assert!(accepted.validation.all_active_features_current);
            assert_eq!(editor.coordinator().intent().undo_len(), 0);
            assert_eq!(editor.coordinator().intent().redo_len(), 0);
            assert!(
                accepted
                    .validation
                    .maximum_normalized_hard_residual
                    .is_none_or(|value| value.is_finite() && value <= 1.0e-9)
            );
            assert!(workbench.session.snapshot().expansion.is_some());
            assert_eq!(
                workbench.session.snapshot().accepted_editor_checkpoint,
                encode_editor_checkpoint(&editor).unwrap(),
            );
        }
    }

    #[test]
    fn menu_owns_one_distinct_code_group_and_nine_genuine_project_leaves() {
        let markup = sample_group_markup(None);
        assert!(markup.contains("Code &amp; reusable patches"));
        let demos = bundled_code_project_demos();
        assert_eq!(
            demos.len(),
            9,
            "the curated M84 catalog includes the manifold dogfood demo"
        );
        assert_eq!(markup.matches("data-code-sample-id=").count(), demos.len());
        for demo in demos {
            assert_eq!(
                markup
                    .matches(&format!("data-code-sample-id=\"{}\"", demo.id.key()))
                    .count(),
                1,
            );
        }
        assert!(!markup.contains("data-sample-id="));
    }

    #[test]
    fn fresh_code_surface_offers_one_authored_entry_and_every_genuine_sample() {
        std::thread::Builder::new()
            .name("m84-code-authored-landing".into())
            .stack_size(32 * 1024 * 1024)
            .spawn(|| {
                let authority = super::super::fresh_projectional_authority().unwrap();
                let editor = authority.projectional_ref().unwrap();
                assert!(matches!(
                    OrdinaryCodeSurface::from_editor(editor),
                    OrdinaryCodeSurface::Starter
                ));
                let markup = OrdinaryCodeSurface::Starter.panel_markup();
                assert_eq!(
                    markup
                        .matches("data-code-action=\"start-authored\"")
                        .count(),
                    1
                );
                assert!(markup.contains("Start from code"));
                assert!(markup.contains("replace the complete source"));
                for demo in bundled_code_project_demos() {
                    assert!(!demo.summary().trim().is_empty());
                    assert_eq!(
                        markup
                            .matches(&format!("data-code-sample-id=\"{}\"", demo.id.key()))
                            .count(),
                        1,
                    );
                    assert!(markup.contains(demo.summary()));
                }
            })
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn authored_starter_applies_persists_and_retains_invalid_code_atomically() {
        std::thread::Builder::new()
            .name("m84-code-authored-project".into())
            .stack_size(32 * 1024 * 1024)
            .spawn(|| {
                let (mut workbench, editor) = CodeProjectWorkbench::new_authored().unwrap();
                assert_eq!(workbench.demo_key(), None);
                assert!(workbench.project.custom_files.is_empty());
                assert!(workbench.project.artifacts.is_empty());
                assert!(
                    workbench
                        .managed_source()
                        .contains("const frame = $.geometry.rectangle")
                );
                assert!(
                    workbench
                        .managed_source()
                        .contains("start: frame.corners.lowerLeft")
                );
                assert!(
                    workbench
                        .managed_source()
                        .contains("end: frame.corners.upperRight")
                );
                assert!(!workbench.managed_source().contains("{\"declaration\":"));

                let accepted = editor.coordinator().accepted_materialization().unwrap();
                assert!(accepted.validation.hard_residuals_validated);
                assert!(accepted.validation.all_active_features_current);
                assert!(
                    accepted
                        .validation
                        .maximum_normalized_hard_residual
                        .is_none_or(|value| value.is_finite() && value <= 1.0e-9)
                );
                assert_eq!(accepted.session.design_document().points().len(), 4);
                assert!(
                    accepted
                        .session
                        .design_document()
                        .points()
                        .iter()
                        .all(|point| point.position.into_iter().all(f64::is_finite))
                );

                let original_source = workbench.managed_source().to_owned();
                let original_checkpoint = workbench.accepted_editor_checkpoint().clone();
                workbench.set_managed_draft(original_source.replacen(
                    "upperRight: [60, 35]",
                    "upperRight: [72, 42]",
                    1,
                ));
                let CodeApplyOutcome::Accepted(publication) =
                    workbench.apply_managed_draft().unwrap()
                else {
                    panic!("valid authored source must publish")
                };
                assert!(workbench.managed_source().contains("upperRight: [72, 42]"));
                let moved = publication
                    .editor
                    .coordinator()
                    .accepted_materialization()
                    .unwrap();
                assert!(moved.validation.hard_residuals_validated);
                assert!(moved.validation.all_active_features_current);
                assert!(
                    moved
                        .validation
                        .maximum_normalized_hard_residual
                        .is_none_or(|value| value.is_finite() && value <= 1.0e-9)
                );
                let moved_document = moved.session.design_document();
                assert_eq!(moved_document.points().len(), 4);
                assert_eq!(moved_document.curves().len(), 5);
                let lower_left = moved_document
                    .points()
                    .iter()
                    .find(|point| {
                        point
                            .position
                            .into_iter()
                            .zip([0.0, 0.0])
                            .all(|(actual, expected)| (actual - expected).abs() <= 1.0e-12)
                    })
                    .unwrap()
                    .id;
                let upper_right = moved_document
                    .points()
                    .iter()
                    .find(|point| {
                        point
                            .position
                            .into_iter()
                            .zip([72.0, 42.0])
                            .all(|(actual, expected)| (actual - expected).abs() <= 1.0e-12)
                    })
                    .unwrap()
                    .id;
                let diagonal_count = moved_document
                    .curves()
                    .iter()
                    .filter(|curve| {
                        matches!(
                            curve.definition,
                            geosolve_sketch::CurveDefinition::Line { start, end, .. }
                                if start == lower_left && end == upper_right
                        )
                    })
                    .count();
                assert_eq!(
                    diagonal_count, 1,
                    "the lexical diagonal must alias the moved rectangle's exact native corner IDs"
                );

                let persisted = workbench.to_persistence_json().unwrap();
                let wire: serde_json::Value = serde_json::from_str(&persisted).unwrap();
                assert_eq!(wire["origin"]["kind"], "authored");
                let restored = CodeProjectWorkbench::from_persistence_json(&persisted).unwrap();
                assert_eq!(restored.origin.title(), "Untitled code sketch");
                assert_eq!(restored.demo_key(), None);
                assert_eq!(restored.managed_source(), workbench.managed_source());
                assert_eq!(restored.to_persistence_json().unwrap(), persisted);
                let restored_editor = restored.restore_accepted_editor().unwrap();
                let restored_accepted = restored_editor
                    .coordinator()
                    .accepted_materialization()
                    .unwrap();
                assert_eq!(
                    restored_accepted.session.design_document(),
                    moved_document,
                    "reload must retain exact lexical endpoint ownership and moved geometry"
                );

                let accepted_checkpoint = workbench.accepted_editor_checkpoint().clone();
                workbench.set_managed_draft(workbench.managed_source().replacen(
                    "upperRight: [72, 42]",
                    "upperRight: [0, 0]",
                    1,
                ));
                let CodeApplyOutcome::RetainedFailure { diagnostic, .. } =
                    workbench.apply_managed_draft().unwrap()
                else {
                    panic!("collapsed authored geometry must retain the prior accepted scene")
                };
                assert!(!diagnostic.is_empty());
                assert_eq!(workbench.accepted_editor_checkpoint(), &accepted_checkpoint);
                let retained_json = workbench.to_persistence_json().unwrap();
                let retained = CodeProjectWorkbench::from_persistence_json(&retained_json).unwrap();
                assert_eq!(retained.origin.title(), "Untitled code sketch");
                assert_eq!(retained.demo_key(), None);
                assert!(retained.managed_source().contains("upperRight: [0, 0]"));
                assert_eq!(
                    retained.accepted_editor_checkpoint(),
                    &accepted_checkpoint,
                    "retained-invalid reload must keep the last accepted native scene"
                );
                assert_eq!(retained.to_persistence_json().unwrap(), retained_json);

                let undone = workbench.step_history(true).unwrap().unwrap();
                assert!(workbench.managed_source().contains("upperRight: [72, 42]"));
                assert_eq!(
                    encode_editor_checkpoint(&undone.editor).unwrap(),
                    accepted_checkpoint
                );
                let undone = workbench.step_history(true).unwrap().unwrap();
                assert_eq!(workbench.managed_source(), original_source);
                assert_eq!(
                    encode_editor_checkpoint(&undone.editor).unwrap(),
                    original_checkpoint
                );
            })
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    fn authored_project_accepts_a_complete_source_replacement_with_exact_history() {
        const REPLACEMENT: &str = r#""use geosolve managed-v1";
import { sketch } from "@geosolve/sketch-code";

export default sketch(($) => {
  const baseline = $.geometry.line("baseline", {
    start: [-15, 8],
    end: [45, 8],
  });
  return $.outputs({ baseline });
});
"#;

        std::thread::Builder::new()
            .name("m84-code-authored-whole-source".into())
            .stack_size(32 * 1024 * 1024)
            .spawn(|| {
                let (mut workbench, _) = CodeProjectWorkbench::new_authored().unwrap();
                let starter = workbench.managed_source().to_owned();
                let starter_checkpoint = workbench.accepted_editor_checkpoint().clone();
                workbench.set_managed_draft(REPLACEMENT.to_owned());
                let CodeApplyOutcome::Accepted(publication) =
                    workbench.apply_managed_draft().unwrap()
                else {
                    panic!("complete valid replacement must publish atomically")
                };
                assert_eq!(workbench.managed_source(), REPLACEMENT);
                let accepted = publication
                    .editor
                    .coordinator()
                    .accepted_materialization()
                    .unwrap();
                assert!(accepted.validation.hard_residuals_validated);
                assert_eq!(accepted.validation.point_count, 2);
                assert_eq!(accepted.validation.curve_count, 1);
                for (point, expected) in accepted
                    .session
                    .design_document()
                    .points()
                    .iter()
                    .zip([[-15.0, 8.0], [45.0, 8.0]])
                {
                    assert!(
                        point
                            .position
                            .into_iter()
                            .zip(expected)
                            .all(|(actual, expected)| (actual - expected).abs() <= 1.0e-12)
                    );
                }

                let undone = workbench.step_history(true).unwrap().unwrap();
                assert_eq!(workbench.managed_source(), starter);
                assert_eq!(
                    encode_editor_checkpoint(&undone.editor).unwrap(),
                    starter_checkpoint
                );
                let redone = workbench.step_history(false).unwrap().unwrap();
                assert_eq!(workbench.managed_source(), REPLACEMENT);
                assert_eq!(
                    redone
                        .editor
                        .coordinator()
                        .accepted_materialization()
                        .unwrap()
                        .validation
                        .curve_count,
                    1
                );
            })
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    fn selected_code_node_resolves_through_semantic_provenance_not_hashed_symbol_text() {
        let (mut workbench, mut editor) = CodeProjectWorkbench::new_authored().unwrap();
        let expansion = workbench
            .session
            .snapshot()
            .accepted_expansion
            .clone()
            .unwrap();
        for expected in ["frame", "diagonal"] {
            let expected = SemanticSymbol(expected.into());
            let (alias, _) = expansion
                .declaration_provenance
                .iter()
                .find(|(_, declaration)| *declaration == &expected)
                .expect("managed declaration must publish alias provenance");
            assert!(
                alias.as_str().starts_with("code."),
                "the fixture must prove that opaque code aliases are not source symbols"
            );
            let node = editor
                .coordinator()
                .intent()
                .graph()
                .node_by_symbol(alias)
                .unwrap()
                .id;
            editor.set_selected_declaration(Some(node));
            assert_eq!(
                workbench.selected_managed_declaration(&editor).unwrap(),
                Some(expected),
            );
            let projection = editor.workbench_projection();
            let inspector = editor.selected_inspector(&projection).unwrap();
            assert!(matches!(
                workbench
                    .apply_managed_dimension_inspector_edit(
                        &editor,
                        &inspector,
                        &IntentInspectorEditTarget::Suppressed,
                        &IntentInspectorEditValue::Suppressed { suppressed: false },
                    )
                    .unwrap(),
                CodeInspectorEditRoute::NotClaimed,
            ));
        }
    }

    #[test]
    fn managed_canvas_deletion_publishes_validated_scene_and_exact_undo() {
        let (mut workbench, mut editor) = CodeProjectWorkbench::new_authored().unwrap();
        let source_before = workbench.managed_source().to_owned();
        let checkpoint_before = workbench.accepted_editor_checkpoint().clone();
        let revision_before = workbench.session.identity().revision;
        let expansion = workbench
            .session
            .snapshot()
            .accepted_expansion
            .as_ref()
            .unwrap();
        let alias = expansion
            .declaration_provenance
            .iter()
            .find_map(|(alias, declaration)| (declaration.0 == "diagonal").then_some(alias))
            .unwrap();
        let node = editor
            .coordinator()
            .intent()
            .graph()
            .node_by_symbol(alias)
            .unwrap()
            .id;
        assert!(editor.set_selected_declaration(Some(node)));
        let target = workbench
            .selected_code_delete_target(&editor)
            .unwrap()
            .unwrap();

        let CodeApplyOutcome::Accepted(publication) =
            workbench.delete_managed_declaration(&target).unwrap()
        else {
            panic!("deleting the independent managed line must publish")
        };
        assert_eq!(publication.receipt.before.revision, revision_before);
        assert_eq!(publication.receipt.after.revision, revision_before + 1);
        assert!(!workbench.managed_source().contains("const diagonal ="));
        assert!(workbench.managed_source().contains("const frame ="));
        assert!(!workbench.managed_source().contains("frame, diagonal"));
        let accepted = publication
            .editor
            .coordinator()
            .accepted_materialization()
            .unwrap();
        assert!(accepted.validation.hard_residuals_validated);
        assert!(accepted.validation.all_active_features_current);
        assert!(
            accepted
                .validation
                .maximum_normalized_hard_residual
                .is_none_or(|value| value.is_finite() && value <= 1.0e-9)
        );
        assert_eq!(accepted.validation.curve_count, 4);
        assert_eq!(
            encode_editor_checkpoint(&publication.editor).unwrap(),
            *workbench.accepted_editor_checkpoint(),
        );

        let undone = workbench.step_history(true).unwrap().unwrap();
        assert_eq!(workbench.managed_source(), source_before);
        assert_eq!(workbench.accepted_editor_checkpoint(), &checkpoint_before);
        assert_eq!(
            encode_editor_checkpoint(&undone.editor).unwrap(),
            checkpoint_before,
        );
    }

    #[test]
    fn managed_canvas_deletion_uses_exact_transitive_dependency_closure() {
        let (mut workbench, mut editor) = CodeProjectWorkbench::new_authored().unwrap();
        let expansion = workbench
            .session
            .snapshot()
            .accepted_expansion
            .as_ref()
            .unwrap();
        let alias = expansion
            .declaration_provenance
            .iter()
            .find_map(|(alias, declaration)| (declaration.0 == "frame").then_some(alias))
            .unwrap();
        let node = editor
            .coordinator()
            .intent()
            .graph()
            .node_by_symbol(alias)
            .unwrap()
            .id;
        assert!(editor.set_selected_declaration(Some(node)));
        let target = workbench
            .selected_code_delete_target(&editor)
            .unwrap()
            .unwrap();
        let outcome = workbench.delete_managed_declaration(&target).unwrap();
        let CodeApplyOutcome::Accepted(publication) = outcome else {
            let CodeApplyOutcome::RetainedFailure { diagnostic, .. } = outcome else {
                unreachable!()
            };
            panic!(
                "deleting the rectangle and dependent line must publish atomically: {diagnostic}"
            )
        };
        assert!(!workbench.managed_source().contains("const frame ="));
        assert!(!workbench.managed_source().contains("const diagonal ="));
        assert!(workbench.managed_source().contains("return $.outputs({});"));
        let accepted = publication
            .editor
            .coordinator()
            .accepted_materialization()
            .unwrap();
        assert!(accepted.validation.hard_residuals_validated);
        assert!(accepted.validation.all_active_features_current);
        assert_eq!(accepted.validation.point_count, 0);
        assert_eq!(accepted.validation.curve_count, 0);
        assert_eq!(
            encode_editor_checkpoint(&publication.editor).unwrap(),
            *workbench.accepted_editor_checkpoint(),
        );
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "one authority regression exercises stale, dirty, retained-failed, and GUI-owned semantic deletion targets"
    )]
    fn semantic_delete_target_rejects_stale_dirty_failed_and_gui_owned_authority() {
        let (mut workbench, mut editor) = CodeProjectWorkbench::new_authored().unwrap();
        let diagonal_alias = workbench
            .session
            .snapshot()
            .accepted_expansion
            .as_ref()
            .unwrap()
            .declaration_provenance
            .iter()
            .find_map(|(alias, declaration)| (declaration.0 == "diagonal").then(|| alias.clone()))
            .unwrap();
        let select_diagonal = |editor: &mut ProjectionalEditorSession| {
            let node = editor
                .coordinator()
                .intent()
                .graph()
                .node_by_symbol(&diagonal_alias)
                .unwrap()
                .id;
            assert!(editor.set_selected_declaration(Some(node)));
        };
        select_diagonal(&mut editor);
        let stale = workbench
            .selected_code_delete_target(&editor)
            .unwrap()
            .unwrap();

        workbench.set_managed_draft(workbench.managed_source().replacen(
            "upperRight: [60, 35]",
            "upperRight: [61, 35]",
            1,
        ));
        let CodeApplyOutcome::Accepted(publication) = workbench.apply_managed_draft().unwrap()
        else {
            panic!("valid source edit must advance semantic deletion authority")
        };
        editor = publication.editor;
        let source_after_edit = workbench.managed_source().to_owned();
        let checkpoint_after_edit = workbench.accepted_editor_checkpoint().clone();
        let revision_after_edit = workbench.session.identity().revision;
        let Err(error) = workbench.delete_managed_declaration(&stale) else {
            panic!("stale semantic deletion target unexpectedly published")
        };
        assert!(error.contains("stale semantic deletion target"));
        assert_eq!(workbench.managed_source(), source_after_edit);
        assert_eq!(
            workbench.accepted_editor_checkpoint(),
            &checkpoint_after_edit
        );
        assert_eq!(workbench.session.identity().revision, revision_after_edit);

        select_diagonal(&mut editor);
        let clean = workbench
            .selected_code_delete_target(&editor)
            .unwrap()
            .unwrap();
        workbench.set_managed_draft(format!("{}\n", workbench.managed_source()));
        let Err(error) = workbench.delete_managed_declaration(&clean) else {
            panic!("dirty semantic deletion target unexpectedly published")
        };
        assert!(error.contains("Apply or Revert"));
        assert_eq!(workbench.managed_source(), source_after_edit);
        assert_eq!(
            workbench.accepted_editor_checkpoint(),
            &checkpoint_after_edit
        );
        assert_eq!(workbench.session.identity().revision, revision_after_edit);
        assert!(workbench.revert_managed_draft());

        let gui_alias = geosolve_sketch_intent::IntentKey::new("gui-delete-audit").unwrap();
        let gui_point = geosolve_sketch_intent::IntentNodeDraft::new(
            IntentNodeKind::Geometry {
                recipe: GeometryRecipeKind::SketchPoint,
            },
            geosolve_sketch_intent::IntentKey::new("GUI delete audit").unwrap(),
        )
        .with_instance_leaf(
            IntentPortSelector::Node {
                role: IntentPortRole::Primary,
                index: 0,
            },
            LeafField::X,
            IntentLiteral::Quantity {
                value: 90.0,
                unit: IntentUnit::Length,
            },
        )
        .with_instance_leaf(
            IntentPortSelector::Node {
                role: IntentPortRole::Primary,
                index: 0,
            },
            LeafField::Y,
            IntentLiteral::Quantity {
                value: 45.0,
                unit: IntentUnit::Length,
            },
        );
        let outcome = editor
            .apply_patch(geosolve_sketch_intent::IntentPatch::new(
                editor.coordinator().intent().identity(),
                geosolve_sketch_intent::IntentPatchPolicy::RequireAccepted,
                vec![geosolve_sketch_intent::IntentPatchOperation::CreateNode {
                    alias: gui_alias.clone(),
                    draft: Box::new(gui_point),
                    cell: None,
                }],
            ))
            .unwrap();
        let gui_node = outcome.aliases.node(&gui_alias).unwrap();
        assert!(editor.set_selected_declaration(Some(gui_node)));
        assert_eq!(
            workbench.selected_code_delete_target(&editor).unwrap(),
            None,
            "ordinary GUI declarations must stay on the ordinary delete route",
        );

        select_diagonal(&mut editor);
        let before_failure_target = workbench
            .selected_code_delete_target(&editor)
            .unwrap()
            .unwrap();
        workbench.set_managed_draft(workbench.managed_source().replacen(
            "upperRight: [61, 35]",
            "upperRight: [0, 0]",
            1,
        ));
        assert!(matches!(
            workbench.apply_managed_draft().unwrap(),
            CodeApplyOutcome::RetainedFailure { .. }
        ));
        let retained_checkpoint = workbench.accepted_editor_checkpoint().clone();
        let retained_source = workbench.managed_source().to_owned();
        let retained_revision = workbench.session.identity().revision;
        select_diagonal(&mut editor);
        let retained_target = workbench
            .selected_code_delete_target(&editor)
            .unwrap()
            .unwrap();
        let Err(error) = workbench.delete_managed_declaration(&retained_target) else {
            panic!("retained-failure semantic deletion target unexpectedly published")
        };
        assert!(error.contains("resolve or Undo"));
        assert_eq!(workbench.managed_source(), retained_source);
        assert_eq!(workbench.accepted_editor_checkpoint(), &retained_checkpoint);
        assert_eq!(workbench.session.identity().revision, retained_revision);
        let Err(error) = workbench.delete_managed_declaration(&before_failure_target) else {
            panic!("pre-failure semantic deletion target unexpectedly published")
        };
        assert!(error.contains("stale semantic deletion target"));
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "one end-to-end regression audits suppression, reset and both history directions"
    )]
    fn generated_child_delete_is_reversible_suppression_not_source_deletion() {
        let (mut workbench, mut editor) = open_boxed("rounded-polyline");
        let source_before = workbench.managed_source().to_owned();
        let checkpoint_before = workbench.accepted_editor_checkpoint().clone();
        let child = workbench
            .session
            .snapshot()
            .accepted_expansion
            .as_ref()
            .unwrap()
            .generated_children
            .iter()
            .find(|child| !child.suppressed)
            .unwrap()
            .clone();
        let node = editor
            .coordinator()
            .intent()
            .graph()
            .node_by_symbol(&child.alias)
            .unwrap()
            .id;
        assert!(editor.set_selected_declaration(Some(node)));
        let target = workbench
            .selected_code_delete_target(&editor)
            .unwrap()
            .unwrap();
        assert!(matches!(
            &target,
            CodeDeleteTarget::GeneratedChild { address, alias, .. }
                if address == &child.address && alias == &child.alias
        ));

        let publication = workbench
            .suppress_generated_child(&target)
            .expect("generated child suppression must publish");
        assert_eq!(workbench.managed_source(), source_before);
        assert!(
            publication
                .editor
                .coordinator()
                .intent()
                .graph()
                .node_by_symbol(&child.alias)
                .unwrap()
                .suppressed,
        );
        assert_eq!(
            workbench
                .session
                .snapshot()
                .interaction_overlay
                .generated_child_suppression(&child.address),
            Some(true),
        );
        assert!(
            workbench
                .panel_markup()
                .contains("data-code-action=\"reset-child-suppression\"")
        );
        let restored = workbench
            .reset_generated_child_suppression(&serde_json::to_string(&child.address).unwrap())
            .unwrap()
            .expect("exact typed child address restores generated authority");
        assert!(
            !restored
                .editor
                .coordinator()
                .intent()
                .graph()
                .node_by_symbol(&child.alias)
                .unwrap()
                .suppressed
        );
        assert_eq!(
            workbench
                .session
                .snapshot()
                .interaction_overlay
                .generated_child_suppression(&child.address),
            None,
        );

        let undone = workbench.step_history(true).unwrap().unwrap();
        assert_eq!(workbench.managed_source(), source_before);
        assert!(
            undone
                .editor
                .coordinator()
                .intent()
                .graph()
                .node_by_symbol(&child.alias)
                .unwrap()
                .suppressed,
        );
        assert_eq!(
            workbench
                .session
                .snapshot()
                .interaction_overlay
                .generated_child_suppression(&child.address),
            Some(true),
        );
        let undone = workbench.step_history(true).unwrap().unwrap();
        assert_eq!(workbench.accepted_editor_checkpoint(), &checkpoint_before);
        assert!(
            !undone
                .editor
                .coordinator()
                .intent()
                .graph()
                .node_by_symbol(&child.alias)
                .unwrap()
                .suppressed
        );
    }

    #[test]
    fn managed_and_custom_files_have_truthful_distinct_ownership_surfaces() {
        let mut workbench = open("rounded-polyline");
        let managed = workbench.panel_markup();
        assert!(managed.contains("data-code-file-kind=\"managed\""));
        assert!(managed.contains("data-code-action=\"apply\""));
        assert!(managed.contains("GUI-managed subset"));
        assert!(managed.contains("Edit lenses"));
        assert!(managed.contains("data-code-lens-declaration=\"rounded\""));
        assert!(managed.contains("data-code-lens-path=\"radius\""));
        assert!(!managed.contains("Read-only in demo"));

        workbench
            .select_file("patches/round-every-corner.patch.ts")
            .unwrap();
        let custom = workbench.panel_markup();
        assert!(custom.contains("data-code-file-kind=\"custom\""));
        assert!(custom.contains("Read-only in demo"));
        assert!(custom.contains("p.each"));
        assert!(!custom.contains("id=\"wb-code-managed-source\""));
    }

    #[test]
    fn scalar_edit_lens_rewrites_one_leaf_and_publishes_one_outer_history_entry() {
        let mut workbench = open("rounded-polyline");
        let before = workbench.managed_source().to_owned();
        let before_revision = workbench.session.identity().revision;
        let CodeApplyOutcome::Accepted(publication) = workbench
            .apply_scalar_lens("rounded", "radius", 0.75)
            .unwrap()
        else {
            panic!("valid scalar lens must acquire native authority")
        };
        assert_eq!(publication.receipt.before.revision, before_revision);
        assert_eq!(publication.receipt.after.revision, before_revision + 1);
        assert!(workbench.managed_source().contains("radius: mm(0.75)"));
        assert_eq!(
            workbench
                .managed_source()
                .replace("radius: mm(0.75)", "radius: mm(4)"),
            before,
        );
        let undone = workbench.step_history(true).unwrap().unwrap();
        assert_eq!(workbench.managed_source(), before);
        assert_eq!(
            encode_editor_checkpoint(&undone.editor).unwrap(),
            *workbench.accepted_editor_checkpoint(),
        );
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "the exact reported manifold regression audits opaque provenance, one-leaf source rewrite, native validity, selection, outer history, Undo, and retained-invalid authority together"
    )]
    fn m86_f001_manifold_dimension_inspector_rewrites_managed_target_atomically() {
        const REPORTED_ALIAS: &str =
            "code.dimension.2cabcaba35f1866930e2549cbd95d899abeb2656e495bf047909f1d92176218b";
        const DECLARATION: &str = "topScrewRail3Length";
        const BEFORE_LINE: &str = "  const topScrewRail3Length = $.dimension.curveLength(\"topScrewRail3Length\", { curve: topScrewRail3.span, target: mm(16) });";
        const AFTER_LINE: &str = "  const topScrewRail3Length = $.dimension.curveLength(\"topScrewRail3Length\", { curve: topScrewRail3.span, target: mm(8) });";

        let (mut workbench, mut editor) = open_with_editor("pc-water-manifold");
        let source_before = workbench.managed_source().to_owned();
        assert!(source_before.contains(BEFORE_LINE));
        assert!(!workbench.can_undo());
        let revision_before = workbench.session.identity().revision;
        let checkpoint_before = workbench.accepted_editor_checkpoint().clone();
        let expansion = workbench
            .session
            .snapshot()
            .accepted_expansion
            .as_ref()
            .expect("manifold expansion authority");
        let (alias, resolved_declaration) = expansion
            .declaration_provenance
            .iter()
            .find(|(_, declaration)| declaration.0 == DECLARATION)
            .expect("reported dimension provenance");
        assert_eq!(alias.as_str(), REPORTED_ALIAS);
        assert_eq!(resolved_declaration.0, DECLARATION);

        let node = editor
            .coordinator()
            .intent()
            .graph()
            .node_by_symbol(alias)
            .expect("reported dimension declaration");
        assert_eq!(
            node.kind,
            IntentNodeKind::Dimension {
                dimension: DimensionKind::CurveLength,
            }
        );
        let node_id = node.id;
        let target_port = node
            .port_by_selector(IntentPortSelector::Node {
                role: IntentPortRole::Target,
                index: 0,
            })
            .expect("dimension target port")
            .as_ref(node_id);
        let target_leaf = LeafRef {
            node: node_id,
            port: target_port.port,
            field: LeafField::Value,
        };
        assert_eq!(
            editor
                .coordinator()
                .intent()
                .instance()
                .values()
                .get(&target_leaf),
            Some(&IntentLiteral::Quantity {
                value: 16.0,
                unit: IntentUnit::Length,
            })
        );
        let IntentNativeBinding::Scalar(target_scalar) = editor
            .coordinator()
            .accepted_materialization()
            .expect("accepted manifold authority")
            .ownership
            .port(target_port)
            .expect("native target ownership")
        else {
            panic!("managed dimension target must own one native scalar")
        };
        assert_eq!(
            editor
                .coordinator()
                .accepted_materialization()
                .unwrap()
                .session
                .design_document()
                .scalar(target_scalar)
                .unwrap()
                .value
                .to_bits(),
            16.0_f64.to_bits(),
        );

        assert!(editor.set_selected_declaration(Some(node_id)));
        let projection = editor.workbench_projection();
        let inspector = editor
            .selected_inspector(&projection)
            .expect("selected managed dimension Inspector");
        let route = workbench
            .apply_managed_dimension_inspector_edit(
                &editor,
                &inspector,
                &IntentInspectorEditTarget::Instance { leaf: target_leaf },
                &IntentInspectorEditValue::Literal {
                    literal: IntentLiteral::Quantity {
                        value: 8.0,
                        unit: IntentUnit::Length,
                    },
                },
            )
            .expect("valid managed dimension Inspector edit");
        let CodeInspectorEditRoute::Claimed(CodeApplyOutcome::Accepted(publication)) = route else {
            panic!("valid managed dimension Inspector edit must claim accepted authority")
        };
        assert_eq!(publication.receipt.before.revision, revision_before);
        assert_eq!(publication.receipt.after.revision, revision_before + 1);
        editor = publication.editor;
        let source_after = workbench.managed_source().to_owned();
        assert_eq!(
            source_after,
            source_before.replacen(BEFORE_LINE, AFTER_LINE, 1)
        );
        assert_eq!(source_after.matches(AFTER_LINE).count(), 1);
        assert_eq!(editor.selected_declaration(), Some(node_id));
        assert_eq!(
            editor
                .coordinator()
                .intent()
                .graph()
                .node(node_id)
                .unwrap()
                .symbol
                .as_str(),
            REPORTED_ALIAS,
        );
        assert_eq!(
            editor
                .coordinator()
                .intent()
                .instance()
                .values()
                .get(&target_leaf),
            Some(&IntentLiteral::Quantity {
                value: 8.0,
                unit: IntentUnit::Length,
            })
        );
        let accepted = editor
            .coordinator()
            .accepted_materialization()
            .expect("edited manifold authority");
        assert!(accepted.validation.hard_residuals_validated);
        assert!(accepted.validation.all_active_features_current);
        assert!(
            accepted
                .validation
                .maximum_normalized_hard_residual
                .is_none_or(|value| value.is_finite() && value <= 1.0e-9)
        );
        let document = accepted.session.design_document();
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
        assert_eq!(
            document.scalar(target_scalar).unwrap().value.to_bits(),
            8.0_f64.to_bits(),
        );
        let checkpoint_after = workbench.accepted_editor_checkpoint().clone();
        assert!(workbench.can_undo());
        assert!(!workbench.can_redo());

        let undone = workbench
            .step_history(true)
            .expect("managed dimension Undo")
            .expect("one outer code history entry");
        assert_eq!(workbench.managed_source(), source_before);
        assert_eq!(workbench.accepted_editor_checkpoint(), &checkpoint_before);
        assert!(!workbench.can_undo());
        assert!(workbench.can_redo());
        let undone_intent = undone.editor.coordinator().intent();
        assert_eq!(
            undone_intent.instance().values().get(&target_leaf),
            Some(&IntentLiteral::Quantity {
                value: 16.0,
                unit: IntentUnit::Length,
            })
        );
        let undone_accepted = undone
            .editor
            .coordinator()
            .accepted_materialization()
            .expect("undone manifold authority");
        assert!(undone_accepted.validation.hard_residuals_validated);
        assert_eq!(
            undone_accepted
                .session
                .design_document()
                .scalar(target_scalar)
                .unwrap()
                .value
                .to_bits(),
            16.0_f64.to_bits(),
        );

        let redone = workbench
            .step_history(false)
            .expect("managed dimension Redo")
            .expect("one outer code redo entry");
        assert_eq!(workbench.managed_source(), source_after);
        assert_eq!(workbench.accepted_editor_checkpoint(), &checkpoint_after);
        assert!(workbench.can_undo());
        assert!(!workbench.can_redo());
        let redone_node = redone
            .editor
            .coordinator()
            .intent()
            .graph()
            .node_by_symbol(&geosolve_sketch_intent::IntentKey::new(REPORTED_ALIAS).unwrap())
            .expect("managed dimension semantic alias after Redo");
        assert_eq!(redone_node.id, node_id);
        assert_eq!(
            redone
                .editor
                .coordinator()
                .intent()
                .instance()
                .values()
                .get(&target_leaf),
            Some(&IntentLiteral::Quantity {
                value: 8.0,
                unit: IntentUnit::Length,
            })
        );
        let redone_accepted = redone
            .editor
            .coordinator()
            .accepted_materialization()
            .expect("redone manifold authority");
        assert!(redone_accepted.validation.hard_residuals_validated);
        assert_eq!(
            redone_accepted
                .session
                .design_document()
                .scalar(target_scalar)
                .unwrap()
                .value
                .to_bits(),
            8.0_f64.to_bits(),
        );

        let mut undone = workbench
            .step_history(true)
            .expect("second managed dimension Undo")
            .expect("redone outer code entry remains undoable");
        assert_eq!(workbench.managed_source(), source_before);
        assert_eq!(workbench.accepted_editor_checkpoint(), &checkpoint_before);
        assert!(!workbench.can_undo());
        assert!(workbench.can_redo());

        let diameter_alias = workbench
            .session
            .snapshot()
            .accepted_expansion
            .as_ref()
            .unwrap()
            .declaration_provenance
            .iter()
            .find_map(|(alias, declaration)| {
                (declaration.0 == "screwNwOuterDiameter").then(|| alias.clone())
            })
            .expect("direct managed diameter provenance");
        let diameter_node = undone
            .editor
            .coordinator()
            .intent()
            .graph()
            .node_by_symbol(&diameter_alias)
            .expect("direct managed diameter declaration");
        assert_eq!(
            diameter_node.kind,
            IntentNodeKind::Dimension {
                dimension: DimensionKind::Diameter,
            }
        );
        let diameter_node_id = diameter_node.id;
        let diameter_target_port = diameter_node
            .port_by_selector(IntentPortSelector::Node {
                role: IntentPortRole::Target,
                index: 0,
            })
            .unwrap()
            .as_ref(diameter_node_id);
        let diameter_target_leaf = LeafRef {
            node: diameter_node_id,
            port: diameter_target_port.port,
            field: LeafField::Value,
        };
        let IntentNativeBinding::Scalar(diameter_target_scalar) = undone
            .editor
            .coordinator()
            .accepted_materialization()
            .unwrap()
            .ownership
            .port(diameter_target_port)
            .unwrap()
        else {
            panic!("managed diameter target must own one native scalar")
        };
        assert!(
            undone
                .editor
                .set_selected_declaration(Some(diameter_node_id))
        );
        let projection = undone.editor.workbench_projection();
        let inspector = undone
            .editor
            .selected_inspector(&projection)
            .expect("direct managed diameter Inspector");
        let retained_checkpoint = workbench.accepted_editor_checkpoint().clone();
        let retained_route = workbench
            .apply_managed_dimension_inspector_edit(
                &undone.editor,
                &inspector,
                &IntentInspectorEditTarget::Instance {
                    leaf: diameter_target_leaf,
                },
                &IntentInspectorEditValue::Literal {
                    literal: IntentLiteral::Quantity {
                        value: 0.0,
                        unit: IntentUnit::Length,
                    },
                },
            )
            .expect("invalid target remains a transactional code intent");
        let CodeInspectorEditRoute::Claimed(CodeApplyOutcome::RetainedFailure {
            receipt,
            diagnostic,
        }) = retained_route
        else {
            panic!("zero target must retain failure without replacing native authority")
        };
        assert_eq!(receipt.before.revision, revision_before + 4);
        assert_eq!(receipt.after.revision, revision_before + 5);
        assert!(!diagnostic.is_empty());
        assert!(workbench.managed_source().contains(
            "const screwNwOuterDiameter = $.dimension.diameter(\"screwNwOuterDiameter\", { curve: screwNwOuter.circle, target: mm(0) });"
        ));
        assert_eq!(workbench.accepted_editor_checkpoint(), &retained_checkpoint,);
        let retained = workbench.restore_accepted_editor().unwrap();
        let retained_accepted = retained
            .coordinator()
            .accepted_materialization()
            .expect("retained-invalid edit preserves accepted authority");
        assert!(retained_accepted.validation.hard_residuals_validated);
        assert_eq!(
            retained_accepted
                .session
                .design_document()
                .scalar(diameter_target_scalar)
                .unwrap()
                .value
                .to_bits(),
            5.0_f64.to_bits(),
        );
        let rejected_again = workbench.apply_managed_dimension_inspector_edit(
            &undone.editor,
            &inspector,
            &IntentInspectorEditTarget::Instance {
                leaf: diameter_target_leaf,
            },
            &IntentInspectorEditValue::Literal {
                literal: IntentLiteral::Quantity {
                    value: 12.0,
                    unit: IntentUnit::Length,
                },
            },
        );
        let Err(rejected_again) = rejected_again else {
            panic!("an active retained failure must reject another Inspector rewrite")
        };
        assert!(rejected_again.contains("resolve or Undo the retained code failure"));
        assert_eq!(workbench.accepted_editor_checkpoint(), &retained_checkpoint,);

        let restored = workbench
            .step_history(true)
            .expect("retained-invalid diameter Undo")
            .expect("retained-invalid diameter adds one outer history entry");
        assert_eq!(workbench.managed_source(), source_before);
        assert_eq!(workbench.accepted_editor_checkpoint(), &retained_checkpoint);
        assert!(!workbench.can_undo());
        assert!(workbench.can_redo());
        assert_eq!(
            restored
                .editor
                .coordinator()
                .accepted_materialization()
                .unwrap()
                .session
                .design_document()
                .scalar(diameter_target_scalar)
                .unwrap()
                .value
                .to_bits(),
            5.0_f64.to_bits(),
        );
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "one compact accepted-diameter fixture audits provenance, source, native target, history, and exact Undo together"
    )]
    fn m86_f001_direct_diameter_inspector_rewrites_managed_target_and_exact_undo() {
        const SOURCE: &str = r#""use geosolve managed-v1";
import { sketch, mm } from "@geosolve/sketch-code";

export default sketch(($) => {
  const screw = $.geometry.circle("screw", {
    center: [10, 12],
    radius: mm(2.5),
  });
  const screwDiameter = $.dimension.diameter("screwDiameter", {
    curve: screw.circle,
    target: mm(5),
  });
  return $.outputs({ screw, screwDiameter });
});
"#;
        let project = CodeProject::managed_only(ProjectKey("m86-direct-diameter".into()), SOURCE)
            .expect("direct diameter managed project");
        let (mut workbench, mut editor) =
            CodeProjectWorkbench::open_project(CodeProjectOrigin::Authored, project)
                .expect("accepted direct diameter project");
        let source_before = workbench.managed_source().to_owned();
        let checkpoint_before = workbench.accepted_editor_checkpoint().clone();
        let alias = workbench
            .session
            .snapshot()
            .accepted_expansion
            .as_ref()
            .unwrap()
            .declaration_provenance
            .iter()
            .find_map(|(alias, declaration)| {
                (declaration.0 == "screwDiameter").then(|| alias.clone())
            })
            .expect("direct diameter provenance");
        let node = editor
            .coordinator()
            .intent()
            .graph()
            .node_by_symbol(&alias)
            .expect("direct diameter declaration");
        assert_eq!(
            node.kind,
            IntentNodeKind::Dimension {
                dimension: DimensionKind::Diameter,
            }
        );
        let node_id = node.id;
        let target_port = node
            .port_by_selector(IntentPortSelector::Node {
                role: IntentPortRole::Target,
                index: 0,
            })
            .unwrap()
            .as_ref(node_id);
        let target_leaf = LeafRef {
            node: node_id,
            port: target_port.port,
            field: LeafField::Value,
        };
        let IntentNativeBinding::Scalar(target_scalar) = editor
            .coordinator()
            .accepted_materialization()
            .unwrap()
            .ownership
            .port(target_port)
            .unwrap()
        else {
            panic!("managed diameter target must own one native scalar")
        };
        assert!(editor.set_selected_declaration(Some(node_id)));
        let projection = editor.workbench_projection();
        let inspector = editor
            .selected_inspector(&projection)
            .expect("selected direct diameter Inspector");
        let route = workbench
            .apply_managed_dimension_inspector_edit(
                &editor,
                &inspector,
                &IntentInspectorEditTarget::Instance { leaf: target_leaf },
                &IntentInspectorEditValue::Literal {
                    literal: IntentLiteral::Quantity {
                        value: 8.0,
                        unit: IntentUnit::Length,
                    },
                },
            )
            .expect("valid direct diameter Inspector edit");
        let CodeInspectorEditRoute::Claimed(CodeApplyOutcome::Accepted(publication)) = route else {
            panic!("valid direct diameter Inspector edit must publish")
        };
        assert_eq!(
            workbench.managed_source(),
            source_before.replacen("target: mm(5)", "target: mm(8)", 1)
        );
        assert!(workbench.can_undo());
        assert!(!workbench.can_redo());
        assert_eq!(publication.editor.selected_declaration(), Some(node_id));
        let accepted = publication
            .editor
            .coordinator()
            .accepted_materialization()
            .expect("accepted direct diameter authority");
        assert!(accepted.validation.hard_residuals_validated);
        assert!(accepted.validation.all_active_features_current);
        assert_eq!(
            accepted
                .session
                .design_document()
                .scalar(target_scalar)
                .unwrap()
                .value
                .to_bits(),
            8.0_f64.to_bits(),
        );

        let undone = workbench
            .step_history(true)
            .expect("direct diameter Undo")
            .expect("one direct diameter history entry");
        assert_eq!(workbench.managed_source(), source_before);
        assert_eq!(workbench.accepted_editor_checkpoint(), &checkpoint_before);
        assert!(!workbench.can_undo());
        assert!(workbench.can_redo());
        assert_eq!(
            undone
                .editor
                .coordinator()
                .accepted_materialization()
                .unwrap()
                .session
                .design_document()
                .scalar(target_scalar)
                .unwrap()
                .value
                .to_bits(),
            5.0_f64.to_bits(),
        );
    }

    #[test]
    fn adaptive_managed_apply_preserves_unaffected_keys_and_adds_crest() {
        let (mut workbench, before_editor) = open_with_editor("rounded-polyline");
        let before = workbench.session.snapshot().generated.active().clone();
        let before_source = workbench.managed_source().to_owned();
        let before_checkpoint = workbench.accepted_editor_checkpoint().clone();
        let before_native = generated_native_bindings(
            &before_editor,
            workbench
                .session
                .snapshot()
                .accepted_expansion
                .as_ref()
                .unwrap(),
        );
        let before_fillets = generated_fillet_owners(&workbench);
        let before_points = before_editor
            .coordinator()
            .accepted_materialization()
            .unwrap()
            .session
            .design_document()
            .points()
            .len();
        let draft = workbench.managed_source().replace(
            "{ key: \"end\", position: [65, 10] },",
            "{ key: \"crest\", position: [60, 14] },\n      { key: \"end\", position: [65, 10] },",
        );
        workbench.set_managed_draft(draft);
        let publication = match workbench.apply_managed_draft().unwrap() {
            CodeApplyOutcome::Accepted(publication) => publication,
            CodeApplyOutcome::RetainedFailure { diagnostic, .. } => {
                panic!("valid crest insertion was retained as failed: {diagnostic}")
            }
        };
        let after = workbench.session.snapshot().generated.active();
        assert_eq!(before.len(), 15);
        assert_eq!(after.len(), 18);
        for (address, identity) in &before {
            assert_eq!(
                after.get(address),
                Some(identity),
                "{}",
                address.display_path()
            );
        }
        assert!(
            after.keys().any(|address| {
                address.member_key == ["crest"] && address.template == ["fillet"]
            })
        );
        assert_ne!(workbench.managed_source(), before_source);
        assert_ne!(workbench.accepted_editor_checkpoint(), &before_checkpoint);
        assert_eq!(
            publication.receipt.after,
            workbench.session.identity().clone()
        );
        let after_native = generated_native_bindings(
            &publication.editor,
            workbench
                .session
                .snapshot()
                .accepted_expansion
                .as_ref()
                .unwrap(),
        );
        for (address, identity) in before_native {
            assert_eq!(
                after_native.get(&address),
                Some(&identity),
                "native identity changed for {}",
                address.display_path(),
            );
        }
        let after_fillets = generated_fillet_owners(&workbench);
        for (address, owners) in before_fillets {
            assert_eq!(
                after_fillets.get(&address),
                Some(&owners),
                "Fillet owner changed for {}",
                address.display_path(),
            );
        }
        let after_points = publication
            .editor
            .coordinator()
            .accepted_materialization()
            .unwrap()
            .session
            .design_document()
            .points()
            .len();
        assert!(after_points > before_points);
        assert_warm_cache_matches_session(&workbench);
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "the integration regression keeps GUI cell, declaration, native identity, Apply, and Undo assertions in one scenario"
    )]
    fn gui_cell_and_declaration_survive_managed_apply_with_native_identity() {
        let (mut workbench, mut editor) = open_boxed("rounded-polyline");
        let cell_alias = geosolve_sketch_intent::IntentKey::new("gui-notes-cell").unwrap();
        let node_alias = geosolve_sketch_intent::IntentKey::new("gui-marker").unwrap();
        let marker = geosolve_sketch_intent::IntentNodeDraft::new(
            IntentNodeKind::Geometry {
                recipe: GeometryRecipeKind::SketchPoint,
            },
            geosolve_sketch_intent::IntentKey::new("gui.marker").unwrap(),
        )
        .with_instance_leaf(
            IntentPortSelector::Node {
                role: IntentPortRole::Primary,
                index: 0,
            },
            LeafField::X,
            IntentLiteral::Quantity {
                value: -5.0,
                unit: IntentUnit::Length,
            },
        )
        .with_instance_leaf(
            IntentPortSelector::Node {
                role: IntentPortRole::Primary,
                index: 0,
            },
            LeafField::Y,
            IntentLiteral::Quantity {
                value: -4.0,
                unit: IntentUnit::Length,
            },
        );
        let outcome = editor
            .apply_patch(geosolve_sketch_intent::IntentPatch::new(
                editor.coordinator().intent().identity(),
                geosolve_sketch_intent::IntentPatchPolicy::RequireAccepted,
                vec![
                    geosolve_sketch_intent::IntentPatchOperation::CreateCell {
                        alias: cell_alias.clone(),
                        name: geosolve_sketch_intent::IntentKey::new("GUI notes").unwrap(),
                        before: None,
                    },
                    geosolve_sketch_intent::IntentPatchOperation::CreateNode {
                        alias: node_alias.clone(),
                        draft: Box::new(marker),
                        cell: Some(geosolve_sketch_intent::CellTarget::Alias {
                            alias: cell_alias.clone(),
                        }),
                    },
                ],
            ))
            .unwrap();
        let cell = outcome.aliases.cell(&cell_alias).unwrap();
        let node = outcome.aliases.node(&node_alias).unwrap();
        let marker_before = editor
            .coordinator()
            .intent()
            .graph()
            .node(node)
            .unwrap()
            .clone();
        let point_before = native_binding_for_node(&editor, node, |binding| {
            matches!(binding, IntentNativeBinding::Point(_))
        });
        let published = workbench
            .publish_delegated_editor_checkpoint(
                encode_editor_checkpoint(&editor).unwrap(),
                "Add GUI notes marker",
            )
            .unwrap()
            .unwrap();
        assert_eq!(
            published
                .editor
                .coordinator()
                .intent()
                .organization()
                .cells()
                .get(&cell)
                .unwrap()
                .declarations,
            vec![node],
        );
        let IntentNativeBinding::Point(gui_point) = point_before else {
            panic!("ordinary GUI marker must own one point")
        };
        assert!(
            workbench
                .prepare_semantic_point_drag(&published.editor, 8407, gui_point, None)
                .unwrap()
                .is_none(),
            "an ordinary GUI point must remain on the delegated editor route",
        );
        assert!(!workbench.has_pending_semantic_point_drag(8407));

        workbench.set_managed_draft(
            workbench
                .managed_source()
                .replace("radius: mm(4)", "radius: mm(0.6)"),
        );
        let CodeApplyOutcome::Accepted(applied) = workbench.apply_managed_draft().unwrap() else {
            panic!("valid managed Apply unexpectedly retained a failure")
        };
        assert_eq!(
            applied.editor.coordinator().intent().graph().node(node),
            Some(&marker_before),
        );
        assert_eq!(
            applied
                .editor
                .coordinator()
                .intent()
                .organization()
                .cells()
                .get(&cell)
                .unwrap()
                .declarations,
            vec![node],
        );
        assert_eq!(
            native_binding_for_node(&applied.editor, node, |binding| {
                matches!(binding, IntentNativeBinding::Point(_))
            }),
            point_before,
        );
        assert_warm_cache_matches_session(&workbench);
    }

    #[test]
    fn gui_horizontal_on_unaffected_start_span_survives_crest_insertion_exactly() {
        let (mut workbench, mut editor) = open_boxed("rounded-polyline");
        let expansion = workbench
            .session
            .snapshot()
            .accepted_expansion
            .clone()
            .unwrap();
        let constraint = add_gui_horizontal_on_generated_span(
            &mut editor,
            &expansion,
            "start",
            "gui.constraint.start-horizontal",
            false,
        );
        let node_before = editor
            .coordinator()
            .intent()
            .graph()
            .node(constraint)
            .unwrap()
            .clone();
        let native_before = native_binding_for_node(&editor, constraint, |binding| {
            matches!(binding, IntentNativeBinding::Constraint(_))
        });
        workbench
            .publish_delegated_editor_checkpoint(
                encode_editor_checkpoint(&editor).unwrap(),
                "Constrain generated start span",
            )
            .unwrap()
            .unwrap();
        workbench.set_managed_draft(workbench.managed_source().replace(
            "{ key: \"end\", position: [65, 10] },",
            "{ key: \"crest\", position: [60, 14] },\n      { key: \"end\", position: [65, 10] },",
        ));
        let CodeApplyOutcome::Accepted(applied) = workbench.apply_managed_draft().unwrap() else {
            panic!("valid crest insertion unexpectedly retained a failure")
        };
        assert_eq!(
            applied
                .editor
                .coordinator()
                .intent()
                .graph()
                .node(constraint),
            Some(&node_before),
        );
        assert_eq!(
            native_binding_for_node(&applied.editor, constraint, |binding| {
                matches!(binding, IntentNativeBinding::Constraint(_))
            }),
            native_before,
        );
        assert_warm_cache_matches_session(&workbench);
    }

    #[test]
    fn persistence_rehydrate_then_crest_apply_preserves_all_unaffected_identities() {
        std::thread::Builder::new()
            .name("m84-persistence-rehydrate-crest".into())
            .stack_size(32 * 1024 * 1024)
            .spawn(|| {
                let workbench = open("rounded-polyline");
                let json = workbench.to_persistence_json().unwrap();
                let mut restored = CodeProjectWorkbench::from_persistence_json(&json).unwrap();
                let before_editor = restored.restore_accepted_editor().unwrap();
                let generated_before = restored.session.snapshot().generated.active().clone();
                let native_before = generated_native_bindings(
                    &before_editor,
                    restored
                        .session
                        .snapshot()
                        .accepted_expansion
                        .as_ref()
                        .unwrap(),
                );
                let fillets_before = generated_fillet_owners(&restored);
                restored.set_managed_draft(restored.managed_source().replace(
                    "{ key: \"end\", position: [65, 10] },",
                    "{ key: \"crest\", position: [60, 14] },\n      { key: \"end\", position: [65, 10] },",
                ));
                let CodeApplyOutcome::Accepted(applied) =
                    restored.apply_managed_draft().unwrap()
                else {
                    panic!("valid post-restore crest insertion unexpectedly retained a failure")
                };
                for (address, identity) in generated_before {
                    assert_eq!(
                        restored.session.snapshot().generated.active().get(&address),
                        Some(&identity),
                        "semantic identity changed after reload for {}",
                        address.display_path(),
                    );
                }
                let native_after = generated_native_bindings(
                    &applied.editor,
                    restored
                        .session
                        .snapshot()
                        .accepted_expansion
                        .as_ref()
                        .unwrap(),
                );
                for (address, binding) in native_before {
                    assert_eq!(native_after.get(&address), Some(&binding));
                }
                let fillets_after = generated_fillet_owners(&restored);
                for (address, owners) in fillets_before {
                    assert_eq!(fillets_after.get(&address), Some(&owners));
                }
                assert_warm_cache_matches_session(&restored);
            })
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    fn valid_but_impossible_apply_retains_source_over_the_prior_native_canvas() {
        let (mut workbench, editor) = open_with_editor("rounded-polyline");
        let accepted_checkpoint = workbench.accepted_editor_checkpoint().clone();
        let accepted_design = editor
            .coordinator()
            .accepted_materialization()
            .unwrap()
            .session
            .design_document()
            .clone();
        workbench.set_managed_draft(workbench.managed_source().replace("mm(4)", "mm(400)"));
        let CodeApplyOutcome::RetainedFailure {
            receipt,
            diagnostic,
        } = workbench.apply_managed_draft().unwrap()
        else {
            panic!("impossible Fillet radius unexpectedly acquired native authority")
        };
        assert!(receipt.retained_failure);
        assert!(!diagnostic.is_empty());
        assert_eq!(workbench.accepted_editor_checkpoint(), &accepted_checkpoint,);
        assert!(workbench.session.snapshot().failure.is_some());
        assert!(workbench.managed_source().contains("mm(400)"));
        assert_eq!(
            editor
                .coordinator()
                .accepted_materialization()
                .unwrap()
                .session
                .design_document(),
            &accepted_design,
        );
    }

    #[test]
    fn parsed_structural_failure_retains_code_intent_without_touching_native_authority() {
        let mut workbench = open("rounded-polyline");
        let before_source = workbench.managed_source().to_owned();
        let before_checkpoint = workbench.accepted_editor_checkpoint().clone();
        workbench.set_managed_draft(before_source.replace(
            "{ key: \"ridge\", position: [40, 18] },",
            "{ key: \"shoulder\", position: [40, 18] },",
        ));

        let CodeApplyOutcome::RetainedFailure {
            receipt,
            diagnostic,
        } = workbench.apply_managed_draft().unwrap()
        else {
            panic!("duplicate semantic key unexpectedly acquired native authority")
        };
        assert!(receipt.retained_failure);
        assert!(diagnostic.contains("duplicate Polyline key `shoulder`"));
        assert_eq!(
            workbench.session.snapshot().failure.as_ref().unwrap().stage,
            "structural expansion",
        );
        assert_eq!(workbench.accepted_editor_checkpoint(), &before_checkpoint);
        assert_ne!(workbench.managed_source(), before_source);

        workbench.step_history(true).unwrap().unwrap();
        assert_eq!(workbench.managed_source(), before_source);
        assert_eq!(workbench.accepted_editor_checkpoint(), &before_checkpoint);
    }

    #[test]
    fn retained_structural_failure_round_trips_with_accepted_canvas_authority() {
        let mut workbench = open("rounded-polyline");
        let accepted_checkpoint = workbench.accepted_editor_checkpoint().clone();
        workbench.set_managed_draft(workbench.managed_source().replace(
            "{ key: \"ridge\", position: [40, 18] },",
            "{ key: \"shoulder\", position: [40, 18] },",
        ));
        assert!(matches!(
            workbench.apply_managed_draft().unwrap(),
            CodeApplyOutcome::RetainedFailure { .. }
        ));

        let json = workbench.to_persistence_json().unwrap();
        let restored = CodeProjectWorkbench::from_persistence_json(&json).unwrap();
        assert!(restored.session.snapshot().failure.is_some());
        assert!(restored.managed_source().contains("key: \"shoulder\""));
        assert_eq!(restored.accepted_editor_checkpoint(), &accepted_checkpoint);
        assert_eq!(restored.to_persistence_json().unwrap(), json);
    }

    #[test]
    fn undo_redo_restore_source_expansion_and_native_scene_as_one_checkpoint() {
        let (mut workbench, _) = open_with_editor("rounded-polyline");
        let before_source = workbench.managed_source().to_owned();
        let before_expansion = workbench.session.snapshot().expansion.clone();
        let draft = before_source.replace("mm(4)", "mm(0.7)");
        workbench.set_managed_draft(draft.clone());
        let CodeApplyOutcome::Accepted(applied) = workbench.apply_managed_draft().unwrap() else {
            panic!("valid radius edit must acquire native authority")
        };
        let applied_checkpoint = workbench.accepted_editor_checkpoint().clone();
        assert_eq!(workbench.managed_source(), draft);
        assert_eq!(
            encode_editor_checkpoint(&applied.editor).unwrap(),
            applied_checkpoint,
        );
        assert_warm_cache_matches_session(&workbench);

        let undone = workbench.step_history(true).unwrap().unwrap();
        assert_eq!(workbench.managed_source(), before_source);
        assert_eq!(workbench.session.snapshot().expansion, before_expansion);
        assert_eq!(
            encode_editor_checkpoint(&undone.editor).unwrap(),
            *workbench.accepted_editor_checkpoint(),
        );
        assert_warm_cache_matches_session(&workbench);

        let redone = workbench.step_history(false).unwrap().unwrap();
        assert_eq!(workbench.managed_source(), draft);
        assert_eq!(workbench.accepted_editor_checkpoint(), &applied_checkpoint);
        assert_eq!(
            encode_editor_checkpoint(&redone.editor).unwrap(),
            applied_checkpoint,
        );
        assert_warm_cache_matches_session(&workbench);
    }

    #[test]
    fn direct_gui_edit_publishes_once_into_outer_history_without_nested_history() {
        let (mut workbench, mut editor) = open_boxed("typed-panel");
        let before_checkpoint = workbench.accepted_editor_checkpoint().clone();
        let before_revision = workbench.session.identity().revision;
        let patch = geosolve_sketch_intent::IntentPatch::new(
            editor.coordinator().intent().identity(),
            geosolve_sketch_intent::IntentPatchPolicy::RequireAccepted,
            vec![geosolve_sketch_intent::IntentPatchOperation::CreateCell {
                alias: geosolve_sketch_intent::IntentKey::new("gui-notes-cell").unwrap(),
                name: geosolve_sketch_intent::IntentKey::new("GUI notes").unwrap(),
                before: None,
            }],
        );
        editor.apply_patch(patch).unwrap();
        assert_eq!(editor.coordinator().intent().undo_len(), 1);

        let checkpoint = encode_editor_checkpoint(&editor).unwrap();
        let delegated = restore_editor_checkpoint(&checkpoint).unwrap();
        assert_eq!(delegated.coordinator().intent().undo_len(), 0);
        assert_eq!(delegated.coordinator().intent().redo_len(), 0);
        let publication = workbench
            .publish_delegated_editor_checkpoint(checkpoint.clone(), "Add GUI cell")
            .unwrap()
            .expect("one outer publication");
        let receipt = publication.receipt;
        assert_eq!(receipt.before.revision, before_revision);
        assert_eq!(receipt.after.revision, before_revision + 1);
        assert_eq!(workbench.accepted_editor_checkpoint(), &checkpoint);
        assert!(
            workbench
                .publish_delegated_editor_checkpoint(checkpoint, "Duplicate save")
                .unwrap()
                .is_none()
        );

        let undone = workbench.step_history(true).unwrap().unwrap();
        assert_eq!(workbench.accepted_editor_checkpoint(), &before_checkpoint);
        assert_eq!(
            encode_editor_checkpoint(&undone.editor).unwrap(),
            before_checkpoint,
        );
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "the regression proves overlay precedence, native parity, persistence, and Undo for one generated point"
    )]
    fn generated_polyline_point_edit_becomes_one_semantic_overlay_draft() {
        let (mut workbench, mut editor) = open_boxed("rounded-polyline");
        let address = workbench
            .session
            .snapshot()
            .generated
            .active()
            .keys()
            .find(|address| {
                address.template == ["polyline", "vertex"] && address.member_key == ["shoulder"]
            })
            .unwrap()
            .clone();
        let expansion = workbench
            .session
            .snapshot()
            .accepted_expansion
            .as_ref()
            .unwrap();
        let leaves = generated_point_leaves(&editor, expansion).unwrap();
        let mut operations = leaves
            .iter()
            .filter(|(_, owner)| *owner == &address)
            .map(|(leaf, _)| {
                let value = match leaf.field {
                    LeafField::X => 26.0,
                    LeafField::Y => 13.0,
                    _ => unreachable!("generated point owns only Cartesian leaves"),
                };
                geosolve_sketch_intent::IntentPatchOperation::SetInstanceLeaf {
                    leaf: *leaf,
                    value: IntentLiteral::Quantity {
                        value,
                        unit: IntentUnit::Length,
                    },
                }
            })
            .collect::<Vec<_>>();
        operations.sort_by_key(|operation| match operation {
            geosolve_sketch_intent::IntentPatchOperation::SetInstanceLeaf { leaf, .. } => {
                leaf.field
            }
            _ => unreachable!(),
        });
        editor
            .apply_patch(geosolve_sketch_intent::IntentPatch::new(
                editor.coordinator().intent().identity(),
                geosolve_sketch_intent::IntentPatchPolicy::RequireAccepted,
                operations,
            ))
            .unwrap();
        let checkpoint = encode_editor_checkpoint(&editor).unwrap();
        let revision = workbench.session.identity().revision;

        let publication = workbench
            .publish_delegated_editor_checkpoint(checkpoint.clone(), "Drag generated point")
            .unwrap()
            .expect("terminal point drag publishes once");
        let receipt = publication.receipt;
        assert_eq!(receipt.before.revision, revision);
        assert_eq!(receipt.after.revision, revision + 1);
        assert_eq!(
            workbench.accepted_editor_checkpoint(),
            &encode_editor_checkpoint(&publication.editor).unwrap()
        );
        assert!(
            workbench
                .session
                .snapshot()
                .generated
                .override_for(&address)
                .is_none(),
            "semantic GUI drafts must not mutate the legacy generated override tier",
        );
        let drafts = workbench.session.snapshot().interaction_overlay.drafts();
        let draft_address = drafts
            .iter()
            .find_map(|(draft_address, draft)| {
                (matches!(
                    &draft_address.owner.address,
                    geosolve_sketch_code::CodeOwnerAddress::GeneratedMember {
                        address: owner,
                    } if owner == &address
                ) && draft.value == geosolve_sketch_code::CodeDraftValue::Point([26.0, 13.0]))
                .then_some(draft_address.clone())
            })
            .expect("generated point drag must own one semantic draft");
        assert_eq!(
            restore_editor_checkpoint(&checkpoint)
                .unwrap()
                .coordinator()
                .intent()
                .undo_len(),
            0,
        );
        assert!(
            workbench
                .panel_markup()
                .contains("data-code-ownership=\"draft\"")
        );
        let reset = workbench
            .reset_semantic_draft(&serde_json::to_string(&draft_address).unwrap())
            .unwrap()
            .expect("Reset to code publishes one outer transaction");
        assert!(
            workbench
                .session
                .snapshot()
                .interaction_overlay
                .drafts()
                .is_empty()
        );
        assert_eq!(reset.receipt.after.revision, revision + 2);
        let undone_reset = workbench.step_history(true).unwrap().unwrap();
        assert_eq!(
            encode_editor_checkpoint(&undone_reset.editor).unwrap(),
            encode_editor_checkpoint(&publication.editor).unwrap(),
        );
        assert_eq!(
            workbench
                .session
                .snapshot()
                .interaction_overlay
                .draft(&draft_address)
                .map(|draft| draft.value),
            Some(geosolve_sketch_code::CodeDraftValue::Point([26.0, 13.0])),
        );
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "one adapter regression keeps semantic-draft retention, legacy override coexistence, owner pruning and exact Undo in the same accepted transaction path"
    )]
    fn semantic_draft_survives_source_and_override_edits_then_prunes_with_owner() {
        let (mut workbench, mut editor) = open_boxed("rounded-polyline");
        let shoulder = workbench
            .session
            .snapshot()
            .generated
            .active()
            .keys()
            .find(|address| {
                address.template == ["polyline", "vertex"] && address.member_key == ["shoulder"]
            })
            .unwrap()
            .clone();
        let expansion = workbench
            .session
            .snapshot()
            .accepted_expansion
            .as_ref()
            .unwrap();
        let leaves = generated_point_leaves(&editor, expansion).unwrap();
        let mut operations = leaves
            .iter()
            .filter(|(_, owner)| *owner == &shoulder)
            .map(
                |(leaf, _)| geosolve_sketch_intent::IntentPatchOperation::SetInstanceLeaf {
                    leaf: *leaf,
                    value: IntentLiteral::Quantity {
                        value: match leaf.field {
                            LeafField::X => 26.0,
                            LeafField::Y => 13.0,
                            _ => unreachable!("generated point owns only Cartesian leaves"),
                        },
                        unit: IntentUnit::Length,
                    },
                },
            )
            .collect::<Vec<_>>();
        operations.sort_by_key(|operation| match operation {
            geosolve_sketch_intent::IntentPatchOperation::SetInstanceLeaf { leaf, .. } => {
                leaf.field
            }
            _ => unreachable!(),
        });
        editor
            .apply_patch(geosolve_sketch_intent::IntentPatch::new(
                editor.coordinator().intent().identity(),
                geosolve_sketch_intent::IntentPatchPolicy::RequireAccepted,
                operations,
            ))
            .unwrap();
        let dragged = workbench
            .publish_delegated_editor_checkpoint(
                encode_editor_checkpoint(&editor).unwrap(),
                "Drag generated shoulder",
            )
            .unwrap()
            .unwrap();
        let draft_address = workbench
            .session
            .snapshot()
            .interaction_overlay
            .drafts()
            .iter()
            .find_map(|(address, draft)| {
                (matches!(
                    &address.owner.address,
                    geosolve_sketch_code::CodeOwnerAddress::GeneratedMember { address: owner }
                        if owner == &shoulder
                ) && draft.value == geosolve_sketch_code::CodeDraftValue::Point([26.0, 13.0]))
                .then_some(address.clone())
            })
            .expect("terminal drag publishes one shoulder draft");
        assert_eq!(
            encode_editor_checkpoint(&dragged.editor).unwrap(),
            *workbench.accepted_editor_checkpoint(),
        );

        workbench.set_managed_draft(
            workbench
                .managed_source()
                .replace("radius: mm(4)", "radius: mm(0.55)"),
        );
        let CodeApplyOutcome::Accepted(source_edit) = workbench.apply_managed_draft().unwrap()
        else {
            panic!("an unrelated managed-source edit must retain the semantic draft")
        };
        assert_eq!(
            workbench
                .session
                .snapshot()
                .interaction_overlay
                .draft(&draft_address)
                .map(|draft| draft.value),
            Some(geosolve_sketch_code::CodeDraftValue::Point([26.0, 13.0])),
        );
        let shoulder_point = workbench
            .session
            .snapshot()
            .accepted_expansion
            .as_ref()
            .unwrap()
            .writable_points
            .iter()
            .find(|point| {
                matches!(
                    &point.edit,
                    geosolve_sketch_code::CodePointEdit::Point { address }
                        if address == &draft_address
                )
            })
            .unwrap();
        assert_eq!(
            expanded_port_instance(&source_edit.editor, &shoulder_point.handle).map(pair_bits),
            Some(pair_bits([26.0, 13.0])),
        );

        let rise = workbench
            .session
            .snapshot()
            .generated
            .active()
            .keys()
            .find(|address| {
                address.template == ["polyline", "vertex"] && address.member_key == ["rise"]
            })
            .unwrap()
            .clone();
        workbench
            .apply_override(
                &rise,
                ManagedValue::Array(vec![ManagedValue::Number(22.0), ManagedValue::Number(3.0)]),
            )
            .expect("legacy override must coexist with an unrelated semantic draft");
        assert!(
            workbench
                .session
                .snapshot()
                .interaction_overlay
                .draft(&draft_address)
                .is_some(),
        );
        workbench
            .reset_override(&rise.display_path())
            .expect("legacy override reset must coexist with an unrelated semantic draft")
            .expect("legacy override reset publishes once");
        assert!(
            workbench
                .session
                .snapshot()
                .interaction_overlay
                .draft(&draft_address)
                .is_some(),
        );

        let checkpoint_before_removal = workbench.accepted_editor_checkpoint().clone();
        workbench.set_managed_draft(workbench.managed_source().replacen(
            "      { key: \"shoulder\", position: [24, 12] },\n",
            "",
            1,
        ));
        let CodeApplyOutcome::Accepted(removal) = workbench.apply_managed_draft().unwrap() else {
            panic!("removing a semantic draft owner must prune it in the same publication")
        };
        assert!(!workbench.managed_source().contains("key: \"shoulder\""));
        assert!(
            workbench
                .session
                .snapshot()
                .interaction_overlay
                .draft(&draft_address)
                .is_none(),
        );
        let accepted = removal
            .editor
            .coordinator()
            .accepted_materialization()
            .unwrap();
        assert!(accepted.validation.hard_residuals_validated);
        assert!(accepted.validation.all_active_features_current);
        assert!(
            accepted
                .validation
                .maximum_normalized_hard_residual
                .is_none_or(|value| value.is_finite() && value <= 1.0e-9),
        );

        let undone = workbench.step_history(true).unwrap().unwrap();
        assert_eq!(
            encode_editor_checkpoint(&undone.editor).unwrap(),
            checkpoint_before_removal,
        );
        assert!(workbench.managed_source().contains("key: \"shoulder\""));
        assert_eq!(
            workbench
                .session
                .snapshot()
                .interaction_overlay
                .draft(&draft_address)
                .map(|draft| draft.value),
            Some(geosolve_sketch_code::CodeDraftValue::Point([26.0, 13.0])),
        );
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "the regression compares retained attempted and accepted overlay/editor authorities through persistence and Undo"
    )]
    fn retained_native_failure_keeps_pruned_candidate_overlay_above_exact_accepted_draft() {
        let mut workbench = open("rounded-polyline");
        let point = workbench
            .session
            .snapshot()
            .accepted_expansion
            .as_ref()
            .unwrap()
            .writable_points
            .iter()
            .find(|point| {
                matches!(&point.edit, geosolve_sketch_code::CodePointEdit::Point { address }
                    if matches!(&address.owner.address,
                        geosolve_sketch_code::CodeOwnerAddress::GeneratedMember { address }
                            if address.template == ["polyline", "vertex"]
                                && address.member_key == ["shoulder"]))
            })
            .unwrap()
            .clone();
        let overlay = workbench
            .session
            .stage_point_drag(&point, [26.0, 13.0])
            .unwrap();
        let candidate = materialize_code_project_incremental_with_overlay(
            workbench.materialized.as_deref().unwrap(),
            &workbench.project,
            &workbench.session.snapshot().generated,
            &overlay,
        )
        .unwrap();
        let dragged = workbench
            .publish_semantic_point_overlay(
                &[(point, [26.0, 13.0])],
                &candidate.editor,
                &[],
                "Drag generated shoulder",
            )
            .unwrap()
            .unwrap();
        let accepted_overlay = workbench
            .session
            .snapshot()
            .accepted_interaction_overlay
            .clone();
        assert_eq!(accepted_overlay.drafts().len(), 1);
        let accepted_checkpoint = encode_editor_checkpoint(&dragged.editor).unwrap();
        let accepted_source = workbench.managed_source().to_owned();

        workbench.set_managed_draft(
            accepted_source
                .replacen("      { key: \"shoulder\", position: [24, 12] },\n", "", 1)
                .replace("radius: mm(4)", "radius: mm(400)"),
        );
        let CodeApplyOutcome::RetainedFailure { diagnostic, .. } =
            workbench.apply_managed_draft().unwrap()
        else {
            panic!("impossible structural edit unexpectedly acquired native authority")
        };
        assert!(!diagnostic.is_empty());
        assert!(workbench.session.snapshot().failure.is_some());
        assert!(
            workbench
                .session
                .snapshot()
                .interaction_overlay
                .drafts()
                .is_empty(),
            "the attempted structure must prune its removed shoulder owner",
        );
        assert_eq!(
            workbench.session.snapshot().accepted_interaction_overlay,
            accepted_overlay,
            "retained failure must not advance accepted semantic draft authority",
        );
        assert_eq!(
            workbench.accepted_editor_checkpoint(),
            &accepted_checkpoint,
            "retained failure must keep the exact accepted dragged canvas",
        );
        let persisted = workbench.to_persistence_json().unwrap();
        let restored = CodeProjectWorkbench::from_persistence_json(&persisted).unwrap();
        assert!(restored.session.snapshot().failure.is_some());
        assert!(
            restored
                .session
                .snapshot()
                .interaction_overlay
                .drafts()
                .is_empty()
        );
        assert_eq!(
            restored.session.snapshot().accepted_interaction_overlay,
            accepted_overlay,
        );

        let undone = workbench.step_history(true).unwrap().unwrap();
        assert_eq!(workbench.managed_source(), accepted_source);
        assert_eq!(
            workbench.session.snapshot().interaction_overlay,
            accepted_overlay,
        );
        assert_eq!(
            encode_editor_checkpoint(&undone.editor).unwrap(),
            accepted_checkpoint,
        );
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "the braced-frame acceptance scenario proves the complete GUI-to-code-to-GUI transaction and exact Undo/Redo authority"
    )]
    fn braced_frame_corner_drag_stages_overlay_and_preserves_managed_source() {
        let (mut workbench, mut editor) = open_boxed("braced-frame");
        let expansion = workbench
            .session
            .snapshot()
            .accepted_expansion
            .clone()
            .unwrap();
        let declaration = SemanticSymbol("frame".into());
        let alias = managed_rectangle_alias(&workbench.project, &expansion, &declaration).unwrap();
        let node = editor
            .coordinator()
            .intent()
            .graph()
            .node_by_symbol(&alias)
            .unwrap();
        let corner = node
            .port_by_selector(IntentPortSelector::Node {
                role: IntentPortRole::Corner,
                index: 2,
            })
            .unwrap();
        let rectangle_node = node.id;
        let rectangle_corner = corner.id;

        // An ordinary GUI-authored dependent shares the moved code-owned
        // corner. Warm source publication must retain its declaration/native
        // owner and let the existing solver update its endpoint.
        let dependent_alias =
            geosolve_sketch_intent::IntentKey::new("gui-frame-dependent").unwrap();
        let dependent = geosolve_sketch_intent::IntentNodeDraft::new(
            IntentNodeKind::Geometry {
                recipe: GeometryRecipeKind::Segment,
            },
            geosolve_sketch_intent::IntentKey::new("gui.frame-dependent").unwrap(),
        )
        .with_input(
            geosolve_sketch_intent::InputSlot::new(geosolve_sketch_intent::InputRole::Point, 0),
            geosolve_sketch_intent::PatchPortRef::Stable {
                port: corner.as_ref(rectangle_node),
            },
        )
        .with_instance_leaf(
            IntentPortSelector::Node {
                role: IntentPortRole::End,
                index: 0,
            },
            LeafField::X,
            IntentLiteral::Quantity {
                value: 78.0,
                unit: IntentUnit::Length,
            },
        )
        .with_instance_leaf(
            IntentPortSelector::Node {
                role: IntentPortRole::End,
                index: 0,
            },
            LeafField::Y,
            IntentLiteral::Quantity {
                value: 42.0,
                unit: IntentUnit::Length,
            },
        );
        let dependent_outcome = editor
            .apply_patch(geosolve_sketch_intent::IntentPatch::new(
                editor.coordinator().intent().identity(),
                geosolve_sketch_intent::IntentPatchPolicy::RequireAccepted,
                vec![geosolve_sketch_intent::IntentPatchOperation::CreateNode {
                    alias: dependent_alias.clone(),
                    draft: Box::new(dependent),
                    cell: None,
                }],
            ))
            .unwrap();
        let dependent_node = dependent_outcome.aliases.node(&dependent_alias).unwrap();
        let dependent_definition = editor
            .coordinator()
            .intent()
            .graph()
            .node(dependent_node)
            .unwrap()
            .clone();
        let dependent_curve = native_binding_for_node(&editor, dependent_node, |binding| {
            matches!(binding, IntentNativeBinding::Curve(_))
        });
        let gui_publication = workbench
            .publish_delegated_editor_checkpoint(
                encode_editor_checkpoint(&editor).unwrap(),
                "Add GUI frame dependent",
            )
            .unwrap()
            .unwrap();
        editor = gui_publication.editor;

        let source_before = workbench.managed_source().to_owned();
        let custom_before = workbench.project.custom_files.clone();
        let code_ids_before = code_node_ids(&editor);
        let before_checkpoint = workbench.accepted_editor_checkpoint().clone();
        let generated_before = generated_native_bindings(&editor, &expansion);
        let (brace_address, brace_identity) = generated_before
            .iter()
            .find(|(address, _)| address.template == ["diagonals", "rising"])
            .map(|(address, identity)| (address.clone(), *identity))
            .expect("generated rising brace");
        let brace_before = curve_definition_for_binding(&editor, brace_identity.1);
        let operations = [(LeafField::X, 64.0), (LeafField::Y, 38.0)]
            .into_iter()
            .map(
                |(field, value)| geosolve_sketch_intent::IntentPatchOperation::SetInstanceLeaf {
                    leaf: LeafRef {
                        node: rectangle_node,
                        port: rectangle_corner,
                        field,
                    },
                    value: IntentLiteral::Quantity {
                        value,
                        unit: IntentUnit::Length,
                    },
                },
            )
            .collect();
        editor
            .apply_patch(geosolve_sketch_intent::IntentPatch::new(
                editor.coordinator().intent().identity(),
                geosolve_sketch_intent::IntentPatchPolicy::RequireAccepted,
                operations,
            ))
            .unwrap();
        let revision = workbench.session.identity().revision;
        let publication = workbench
            .publish_delegated_editor_checkpoint(
                encode_editor_checkpoint(&editor).unwrap(),
                "Drag managed frame corner",
            )
            .unwrap()
            .expect("one managed-source placement publication");

        assert_eq!(publication.receipt.before.revision, revision);
        assert_eq!(publication.receipt.after.revision, revision + 1);
        assert_eq!(workbench.managed_source(), source_before);
        assert_eq!(
            workbench
                .session
                .snapshot()
                .interaction_overlay
                .drafts()
                .len(),
            2,
            "one rectangle-corner drag updates both canonical seeds atomically",
        );
        assert_eq!(workbench.project.custom_files, custom_before);
        assert_eq!(code_node_ids(&publication.editor), code_ids_before);
        assert_eq!(
            publication
                .editor
                .coordinator()
                .intent()
                .graph()
                .node(dependent_node),
            Some(&dependent_definition),
        );
        assert_eq!(
            native_binding_for_node(&publication.editor, dependent_node, |binding| {
                matches!(binding, IntentNativeBinding::Curve(_))
            }),
            dependent_curve,
        );
        let generated_after = generated_native_bindings(
            &publication.editor,
            workbench
                .session
                .snapshot()
                .accepted_expansion
                .as_ref()
                .unwrap(),
        );
        assert_eq!(generated_after.get(&brace_address), Some(&brace_identity));
        let brace_after = curve_definition_for_binding(&publication.editor, brace_identity.1);
        assert_ne!(
            brace_after, brace_before,
            "brace geometry did not follow frame"
        );
        assert_eq!(
            workbench.accepted_editor_checkpoint(),
            &encode_editor_checkpoint(&publication.editor).unwrap(),
        );
        let after_checkpoint = workbench.accepted_editor_checkpoint().clone();
        assert_eq!(publication.editor.coordinator().intent().undo_len(), 0,);

        let undone = workbench.step_history(true).unwrap().unwrap();
        assert_eq!(workbench.managed_source(), source_before);
        assert_eq!(workbench.project.custom_files, custom_before);
        assert_eq!(code_node_ids(&undone.editor), code_ids_before);
        assert_eq!(
            encode_editor_checkpoint(&undone.editor).unwrap(),
            before_checkpoint,
        );
        assert_eq!(
            curve_definition_for_binding(&undone.editor, brace_identity.1),
            brace_before,
        );
        assert_eq!(
            undone
                .editor
                .coordinator()
                .intent()
                .graph()
                .node(dependent_node),
            Some(&dependent_definition),
        );
        assert_warm_cache_matches_session(&workbench);
        let redone = workbench.step_history(false).unwrap().unwrap();
        assert_eq!(workbench.managed_source(), source_before);
        assert_eq!(workbench.project.custom_files, custom_before);
        assert_eq!(code_node_ids(&redone.editor), code_ids_before);
        assert_eq!(
            encode_editor_checkpoint(&redone.editor).unwrap(),
            after_checkpoint,
        );
        assert_eq!(
            curve_definition_for_binding(&redone.editor, brace_identity.1),
            brace_after,
        );
        assert_eq!(
            redone
                .editor
                .coordinator()
                .intent()
                .graph()
                .node(dependent_node),
            Some(&dependent_definition),
        );
        assert_warm_cache_matches_session(&workbench);
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "the focused generated-reference regression keeps source/native identity, measured value, managed rewrite, exact history, and continued GUI editing together"
    )]
    fn braced_frame_gui_reference_dimension_survives_overlay_drag_and_history() {
        let (mut workbench, mut editor) = open_boxed("braced-frame");
        let expansion = workbench
            .session
            .snapshot()
            .accepted_expansion
            .clone()
            .unwrap();
        let (_, brace_provenance) = expansion
            .generated_provenance
            .iter()
            .find(|(address, _)| address.template == ["diagonals", "rising"])
            .expect("generated rising brace provenance");
        let ExpandedSemanticTarget::Port { port: brace_port } = &brace_provenance.target else {
            panic!("generated rising brace must expose one stable span port")
        };
        let brace_node = editor
            .coordinator()
            .intent()
            .graph()
            .node_by_symbol(&brace_port.alias)
            .expect("generated rising brace declaration");
        let brace_node_id = brace_node.id;
        let brace_span_port = brace_node
            .port_by_selector(brace_port.selector)
            .expect("generated rising brace span")
            .as_ref(brace_node.id);
        let brace_binding = editor
            .coordinator()
            .accepted_materialization()
            .unwrap()
            .ownership
            .port(brace_span_port)
            .expect("generated rising brace native span");
        let IntentNativeBinding::CurveSpan(brace_span) = brace_binding else {
            panic!("generated rising brace port must own one native curve span")
        };

        let dimension_alias = geosolve_sketch_intent::IntentKey::new("gui-brace-length").unwrap();
        let dimension = geosolve_sketch_intent::IntentNodeDraft::new(
            IntentNodeKind::Dimension {
                dimension: geosolve_sketch_intent::DimensionKind::CurveLength,
            },
            geosolve_sketch_intent::IntentKey::new("Rising brace length").unwrap(),
        )
        .with_input(
            geosolve_sketch_intent::InputSlot::new(geosolve_sketch_intent::InputRole::Span, 0),
            geosolve_sketch_intent::PatchPortRef::Stable {
                port: brace_span_port,
            },
        )
        .with_field(
            geosolve_sketch_intent::IntentFieldKey(
                geosolve_sketch_intent::IntentKey::new("mode").unwrap(),
            ),
            IntentLiteral::Enum(geosolve_sketch_intent::IntentKey::new("reference").unwrap()),
        );
        let dimension_outcome = editor
            .apply_patch(geosolve_sketch_intent::IntentPatch::new(
                editor.coordinator().intent().identity(),
                geosolve_sketch_intent::IntentPatchPolicy::RequireAccepted,
                vec![geosolve_sketch_intent::IntentPatchOperation::CreateNode {
                    alias: dimension_alias,
                    draft: Box::new(dimension),
                    cell: None,
                }],
            ))
            .expect("ordinary GUI reference dimension");
        let dimension_node = dimension_outcome
            .aliases
            .node(&geosolve_sketch_intent::IntentKey::new("gui-brace-length").unwrap())
            .expect("GUI reference dimension node");
        let IntentNativeBinding::Dimension(dimension_id) =
            native_binding_for_node(&editor, dimension_node, |binding| {
                matches!(binding, IntentNativeBinding::Dimension(_))
            })
        else {
            unreachable!("dimension predicate returned a non-dimension binding")
        };
        let dimension_source = editor
            .coordinator()
            .accepted_materialization()
            .unwrap()
            .session
            .design_document()
            .dimension(dimension_id)
            .expect("native GUI reference dimension")
            .source_id;
        let published_dimension = workbench
            .publish_delegated_editor_checkpoint(
                encode_editor_checkpoint(&editor).unwrap(),
                "Add rising brace reference dimension",
            )
            .unwrap()
            .expect("GUI reference dimension publishes once");
        editor = published_dimension.editor;
        assert!(editor.set_selected_declaration(Some(dimension_node)));
        let projection = editor.workbench_projection();
        let inspector = editor
            .selected_inspector(&projection)
            .expect("ordinary GUI dimension Inspector");
        assert!(matches!(
            workbench
                .apply_managed_dimension_inspector_edit(
                    &editor,
                    &inspector,
                    &IntentInspectorEditTarget::Suppressed,
                    &IntentInspectorEditValue::Suppressed { suppressed: true },
                )
                .unwrap(),
            CodeInspectorEditRoute::NotClaimed,
        ));

        let reference_value = |editor: &ProjectionalEditorSession| {
            let authority = editor
                .coordinator()
                .accepted_materialization()
                .expect("accepted native authority");
            let accepted = authority
                .session
                .accepted_state_for_current_input()
                .expect("current accepted scene");
            let value = accepted
                .reference_value(dimension_id)
                .expect("finite reference measurement");
            assert!(value.is_finite());
            let dimension = accepted
                .document()
                .dimension(dimension_id)
                .expect("accepted native reference dimension");
            assert_eq!(dimension.source_id, dimension_source);
            assert_eq!(
                dimension.mode,
                geosolve_sketch::DocumentDimensionMode::Reference
            );
            let geosolve_sketch::DocumentDimensionDefinition::CurveLength { curve, .. } =
                dimension.definition
            else {
                panic!("GUI reference dimension changed kind")
            };
            assert_eq!(curve, brace_span);
            let geosolve_sketch::CurveDefinition::Line { start, end, .. } = &accepted
                .document()
                .curve(curve.curve)
                .expect("accepted brace curve")
                .definition
            else {
                panic!("rising brace changed geometry family")
            };
            let start = accepted.document().point(*start).unwrap().position;
            let end = accepted.document().point(*end).unwrap().position;
            let measured = (end[0] - start[0]).hypot(end[1] - start[1]);
            assert!((value - measured).abs() <= 1.0e-9);
            value
        };
        let value_before = reference_value(&editor);
        let source_before = workbench.managed_source().to_owned();
        let custom_before = workbench.project.custom_files.clone();
        let checkpoint_before = workbench.accepted_editor_checkpoint().clone();
        let brace_before = curve_definition_for_binding(&editor, brace_binding);

        let frame_alias = managed_rectangle_alias(
            &workbench.project,
            &expansion,
            &SemanticSymbol("frame".into()),
        )
        .expect("managed frame declaration");
        let frame = editor
            .coordinator()
            .intent()
            .graph()
            .node_by_symbol(&frame_alias)
            .expect("managed frame node");
        let upper_right = frame
            .port_by_selector(IntentPortSelector::Node {
                role: IntentPortRole::Corner,
                index: 2,
            })
            .expect("managed upper-right corner");
        let operations = [(LeafField::X, 64.0), (LeafField::Y, 38.0)]
            .into_iter()
            .map(
                |(field, value)| geosolve_sketch_intent::IntentPatchOperation::SetInstanceLeaf {
                    leaf: LeafRef {
                        node: frame.id,
                        port: upper_right.id,
                        field,
                    },
                    value: IntentLiteral::Quantity {
                        value,
                        unit: IntentUnit::Length,
                    },
                },
            )
            .collect();
        editor
            .apply_patch(geosolve_sketch_intent::IntentPatch::new(
                editor.coordinator().intent().identity(),
                geosolve_sketch_intent::IntentPatchPolicy::RequireAccepted,
                operations,
            ))
            .expect("managed frame corner edit");
        let rewritten = workbench
            .publish_delegated_editor_checkpoint(
                encode_editor_checkpoint(&editor).unwrap(),
                "Drag managed frame corner with GUI reference",
            )
            .unwrap()
            .expect("managed frame rewrite publishes once");
        let value_after = reference_value(&rewritten.editor);
        let brace_after = curve_definition_for_binding(&rewritten.editor, brace_binding);
        let source_after = workbench.managed_source().to_owned();
        let checkpoint_after = workbench.accepted_editor_checkpoint().clone();

        assert_eq!(source_after, source_before);
        assert_eq!(workbench.project.custom_files, custom_before);
        assert_ne!(brace_after, brace_before);
        assert_ne!(value_after.to_bits(), value_before.to_bits());
        assert_eq!(
            rewritten
                .editor
                .coordinator()
                .intent()
                .graph()
                .node_by_symbol(&brace_port.alias)
                .unwrap()
                .id,
            brace_node_id,
        );
        assert_eq!(
            rewritten
                .editor
                .coordinator()
                .accepted_materialization()
                .unwrap()
                .ownership
                .port(brace_span_port),
            Some(brace_binding),
        );
        assert_eq!(
            native_binding_for_node(&rewritten.editor, dimension_node, |binding| {
                matches!(binding, IntentNativeBinding::Dimension(_))
            }),
            IntentNativeBinding::Dimension(dimension_id),
        );
        let validation = &rewritten
            .editor
            .coordinator()
            .accepted_materialization()
            .unwrap()
            .validation;
        assert!(validation.hard_residuals_validated);
        assert!(validation.all_active_features_current);
        assert!(
            validation
                .maximum_normalized_hard_residual
                .is_none_or(|value| value.is_finite() && value <= 1.0e-9)
        );

        let undone = workbench.step_history(true).unwrap().unwrap();
        assert_eq!(workbench.managed_source(), source_before);
        assert_eq!(workbench.project.custom_files, custom_before);
        assert_eq!(
            encode_editor_checkpoint(&undone.editor).unwrap(),
            checkpoint_before,
        );
        assert_eq!(
            curve_definition_for_binding(&undone.editor, brace_binding),
            brace_before
        );
        assert_eq!(
            reference_value(&undone.editor).to_bits(),
            value_before.to_bits()
        );
        assert_eq!(
            native_binding_for_node(&undone.editor, dimension_node, |binding| {
                matches!(binding, IntentNativeBinding::Dimension(_))
            }),
            IntentNativeBinding::Dimension(dimension_id),
        );
        assert_warm_cache_matches_session(&workbench);

        let mut redone = workbench.step_history(false).unwrap().unwrap().editor;
        assert_eq!(workbench.managed_source(), source_after);
        assert_eq!(workbench.project.custom_files, custom_before);
        assert_eq!(encode_editor_checkpoint(&redone).unwrap(), checkpoint_after);
        assert_eq!(
            curve_definition_for_binding(&redone, brace_binding),
            brace_after
        );
        assert_eq!(reference_value(&redone).to_bits(), value_after.to_bits());
        assert_eq!(
            native_binding_for_node(&redone, dimension_node, |binding| {
                matches!(binding, IntentNativeBinding::Dimension(_))
            }),
            IntentNativeBinding::Dimension(dimension_id),
        );
        assert_warm_cache_matches_session(&workbench);

        redone
            .apply_patch(geosolve_sketch_intent::IntentPatch::new(
                redone.coordinator().intent().identity(),
                geosolve_sketch_intent::IntentPatchPolicy::RequireAccepted,
                vec![geosolve_sketch_intent::IntentPatchOperation::RenameNode {
                    node: dimension_node,
                    name: geosolve_sketch_intent::IntentKey::new("Verified rising length").unwrap(),
                }],
            ))
            .expect("ordinary GUI dimension remains editable");
        let renamed = workbench
            .publish_delegated_editor_checkpoint(
                encode_editor_checkpoint(&redone).unwrap(),
                "Rename rising brace reference dimension",
            )
            .unwrap()
            .expect("GUI dimension rename publishes once");
        assert_eq!(
            renamed
                .editor
                .coordinator()
                .intent()
                .organization()
                .node_names()
                .get(&dimension_node)
                .unwrap()
                .as_str(),
            "Verified rising length",
        );
        assert_eq!(
            native_binding_for_node(&renamed.editor, dimension_node, |binding| {
                matches!(binding, IntentNativeBinding::Dimension(_))
            }),
            IntentNativeBinding::Dimension(dimension_id),
        );
        assert_eq!(
            reference_value(&renamed.editor).to_bits(),
            value_after.to_bits()
        );
        assert_warm_cache_matches_session(&workbench);
    }

    #[test]
    fn gui_dependent_survives_warm_apply_reload_and_blocks_source_removal() {
        let (mut workbench, mut editor) = open_boxed("rounded-polyline");
        let expansion = workbench
            .session
            .snapshot()
            .accepted_expansion
            .clone()
            .unwrap();
        let dependent = add_gui_horizontal_on_generated_span(
            &mut editor,
            &expansion,
            "shoulder",
            "gui.constraint.shoulder",
            true,
        );
        let dependent_before = editor
            .coordinator()
            .intent()
            .graph()
            .node(dependent)
            .unwrap()
            .clone();
        let published = workbench
            .publish_delegated_editor_checkpoint(
                encode_editor_checkpoint(&editor).unwrap(),
                "Add GUI constraint",
            )
            .unwrap()
            .unwrap();
        assert_eq!(
            published
                .editor
                .coordinator()
                .intent()
                .graph()
                .node(dependent),
            Some(&dependent_before),
        );

        workbench.set_managed_draft(
            workbench
                .managed_source()
                .replace("radius: mm(4)", "radius: mm(0.55)"),
        );
        let CodeApplyOutcome::Accepted(applied) = workbench.apply_managed_draft().unwrap() else {
            panic!("valid warm radius edit unexpectedly retained a failure")
        };
        assert_eq!(
            applied
                .editor
                .coordinator()
                .intent()
                .graph()
                .node(dependent),
            Some(&dependent_before),
        );
        let persisted = workbench.to_persistence_json().unwrap();
        let restored = CodeProjectWorkbench::from_persistence_json(&persisted).unwrap();
        assert_eq!(
            restored
                .restore_accepted_editor()
                .unwrap()
                .coordinator()
                .intent()
                .graph()
                .node(dependent),
            Some(&dependent_before),
        );

        let accepted_checkpoint = workbench.accepted_editor_checkpoint().clone();
        workbench.set_managed_draft(workbench.managed_source().replacen(
            "      { key: \"shoulder\", position: [24, 12] },\n",
            "",
            1,
        ));
        let CodeApplyOutcome::RetainedFailure {
            receipt,
            diagnostic,
        } = workbench.apply_managed_draft().unwrap()
        else {
            panic!("removing a generated source with an outside dependent was accepted")
        };
        assert!(receipt.retained_failure);
        assert!(diagnostic.contains("outside dependent"));
        assert_eq!(workbench.accepted_editor_checkpoint(), &accepted_checkpoint);
        assert_eq!(
            workbench
                .restore_accepted_editor()
                .unwrap()
                .coordinator()
                .intent()
                .graph()
                .node(dependent),
            Some(&dependent_before),
        );
        assert!(workbench.step_history(true).unwrap().is_some());
        assert!(workbench.session.snapshot().failure.is_none());
        assert!(workbench.managed_source().contains("key: \"shoulder\""));
    }

    #[test]
    fn unmanaged_canvas_edit_of_code_declaration_is_rejected_without_divergence() {
        let (mut workbench, mut editor) = open_boxed("braced-frame");
        let before_checkpoint = workbench.accepted_editor_checkpoint().clone();
        let before_revision = workbench.session.identity().revision;
        let node = editor
            .coordinator()
            .intent()
            .graph()
            .nodes()
            .values()
            .find(|node| node.symbol.as_str().starts_with("code."))
            .unwrap()
            .id;
        editor
            .apply_patch(geosolve_sketch_intent::IntentPatch::new(
                editor.coordinator().intent().identity(),
                geosolve_sketch_intent::IntentPatchPolicy::RequireAccepted,
                vec![geosolve_sketch_intent::IntentPatchOperation::RenameNode {
                    node,
                    name: geosolve_sketch_intent::IntentKey::new("unmanaged GUI rename").unwrap(),
                }],
            ))
            .unwrap();
        let Err(diagnostic) = workbench.publish_delegated_editor_checkpoint(
            encode_editor_checkpoint(&editor).unwrap(),
            "Rename code declaration",
        ) else {
            panic!("unmanaged code declaration edit unexpectedly published")
        };
        assert!(diagnostic.contains("managed source"));
        assert_eq!(workbench.session.identity().revision, before_revision);
        assert_eq!(workbench.accepted_editor_checkpoint(), &before_checkpoint);
        assert_eq!(
            encode_editor_checkpoint(&workbench.restore_accepted_editor().unwrap()).unwrap(),
            before_checkpoint,
        );
    }

    #[test]
    fn invalid_managed_text_is_only_a_diagnostic_draft_and_revert_is_exact() {
        let mut workbench = open("typed-panel");
        let canonical = workbench.managed_source().to_owned();
        workbench
            .set_managed_draft(canonical.replace("export default", "while (true) export default"));
        assert!(workbench.apply_managed_draft().is_err());
        assert_eq!(workbench.managed_source(), canonical);
        let markup = workbench.panel_markup();
        assert!(markup.contains("wb-code-diagnostic"));
        assert!(markup.contains("Line "));
        assert!(workbench.revert_managed_draft());
        assert_eq!(workbench.managed_draft, canonical);
        assert!(workbench.draft_diagnostic.is_none());
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "the override/reset regression keeps the unrelated GUI geometry and constraint identity oracle together"
    )]
    fn override_then_reset_preserves_unrelated_gui_geometry_and_constraint_ids() {
        let (mut workbench, mut editor) = open_boxed("rounded-polyline");
        let segment_alias = geosolve_sketch_intent::IntentKey::new("gui-segment").unwrap();
        let horizontal_alias =
            geosolve_sketch_intent::IntentKey::new("gui-segment-horizontal").unwrap();
        let selector = |role| IntentPortSelector::Node { role, index: 0 };
        let coordinate = |value| IntentLiteral::Quantity {
            value,
            unit: IntentUnit::Length,
        };
        let segment = geosolve_sketch_intent::IntentNodeDraft::new(
            IntentNodeKind::Geometry {
                recipe: GeometryRecipeKind::Segment,
            },
            geosolve_sketch_intent::IntentKey::new("gui.fixture.segment").unwrap(),
        )
        .with_instance_leaf(
            selector(IntentPortRole::Start),
            LeafField::X,
            coordinate(-8.0),
        )
        .with_instance_leaf(
            selector(IntentPortRole::Start),
            LeafField::Y,
            coordinate(-8.0),
        )
        .with_instance_leaf(selector(IntentPortRole::End), LeafField::X, coordinate(8.0))
        .with_instance_leaf(
            selector(IntentPortRole::End),
            LeafField::Y,
            coordinate(-8.0),
        );
        let horizontal = geosolve_sketch_intent::IntentNodeDraft::new(
            IntentNodeKind::Constraint {
                constraint: geosolve_sketch_intent::ConstraintKind::Horizontal,
            },
            geosolve_sketch_intent::IntentKey::new("gui.fixture.horizontal").unwrap(),
        )
        .with_input(
            geosolve_sketch_intent::InputSlot::new(geosolve_sketch_intent::InputRole::Span, 0),
            geosolve_sketch_intent::PatchPortRef::Alias {
                node: segment_alias.clone(),
                selector: selector(IntentPortRole::Span),
            },
        );
        let outcome = editor
            .apply_patch(geosolve_sketch_intent::IntentPatch::new(
                editor.coordinator().intent().identity(),
                geosolve_sketch_intent::IntentPatchPolicy::RequireAccepted,
                vec![
                    geosolve_sketch_intent::IntentPatchOperation::CreateNode {
                        alias: segment_alias.clone(),
                        draft: Box::new(segment),
                        cell: None,
                    },
                    geosolve_sketch_intent::IntentPatchOperation::CreateNode {
                        alias: horizontal_alias.clone(),
                        draft: Box::new(horizontal),
                        cell: None,
                    },
                ],
            ))
            .unwrap();
        let segment = outcome.aliases.node(&segment_alias).unwrap();
        let horizontal = outcome.aliases.node(&horizontal_alias).unwrap();
        let segment_node = editor
            .coordinator()
            .intent()
            .graph()
            .node(segment)
            .unwrap()
            .clone();
        let horizontal_node = editor
            .coordinator()
            .intent()
            .graph()
            .node(horizontal)
            .unwrap()
            .clone();
        let curve = native_binding_for_node(&editor, segment, |binding| {
            matches!(binding, IntentNativeBinding::Curve(_))
        });
        let constraint = native_binding_for_node(&editor, horizontal, |binding| {
            matches!(binding, IntentNativeBinding::Constraint(_))
        });
        workbench
            .publish_delegated_editor_checkpoint(
                encode_editor_checkpoint(&editor).unwrap(),
                "Add unrelated GUI fixture",
            )
            .unwrap()
            .unwrap();
        let address = workbench
            .session
            .snapshot()
            .generated
            .active()
            .keys()
            .find(|address| {
                address.template == ["polyline", "vertex"] && address.member_key == ["rise"]
            })
            .unwrap()
            .clone();
        let overridden = workbench
            .apply_override(
                &address,
                ManagedValue::Array(vec![ManagedValue::Number(22.0), ManagedValue::Number(3.0)]),
            )
            .unwrap();
        assert_eq!(
            overridden
                .editor
                .coordinator()
                .intent()
                .graph()
                .node(segment),
            Some(&segment_node),
        );
        assert_eq!(
            overridden
                .editor
                .coordinator()
                .intent()
                .graph()
                .node(horizontal),
            Some(&horizontal_node),
        );
        assert_eq!(
            native_binding_for_node(&overridden.editor, segment, |binding| {
                matches!(binding, IntentNativeBinding::Curve(_))
            }),
            curve,
        );
        assert_eq!(
            native_binding_for_node(&overridden.editor, horizontal, |binding| {
                matches!(binding, IntentNativeBinding::Constraint(_))
            }),
            constraint,
        );
        let reset = workbench
            .reset_override(&address.display_path())
            .unwrap()
            .unwrap();
        assert_eq!(
            reset.editor.coordinator().intent().graph().node(segment),
            Some(&segment_node),
        );
        assert_eq!(
            reset.editor.coordinator().intent().graph().node(horizontal),
            Some(&horizontal_node),
        );
        assert_eq!(
            native_binding_for_node(&reset.editor, segment, |binding| {
                matches!(binding, IntentNativeBinding::Curve(_))
            }),
            curve,
        );
        assert_eq!(
            native_binding_for_node(&reset.editor, horizontal, |binding| {
                matches!(binding, IntentNativeBinding::Constraint(_))
            }),
            constraint,
        );
        assert_warm_cache_matches_session(&workbench);
    }

    #[test]
    fn override_badge_reset_and_unified_undo_restore_one_code_session() {
        let mut workbench = open("rounded-polyline");
        let address = workbench
            .session
            .snapshot()
            .generated
            .active()
            .keys()
            .find(|address| {
                address.template == ["polyline", "vertex"] && address.member_key == ["rise"]
            })
            .unwrap()
            .clone();
        workbench
            .apply_override(
                &address,
                ManagedValue::Array(vec![ManagedValue::Number(22.0), ManagedValue::Number(3.0)]),
            )
            .unwrap();
        let overridden = workbench.panel_markup();
        assert!(overridden.contains("data-code-ownership=\"override\""));
        assert!(overridden.contains("Reset to code"));
        assert!(
            workbench
                .reset_override(&address.display_path())
                .unwrap()
                .is_some()
        );
        assert!(
            workbench
                .session
                .snapshot()
                .generated
                .override_for(&address)
                .is_none()
        );
        assert!(workbench.step_history(true).unwrap().is_some());
        assert!(
            workbench
                .session
                .snapshot()
                .generated
                .override_for(&address)
                .is_some()
        );
    }

    #[test]
    fn unsupported_ordinary_projection_remains_visible_and_escaped() {
        let surface = OrdinaryCodeSurface::Unavailable {
            reason: "unsupported <future> & \"shape\"".into(),
        };
        let markup = surface.panel_markup();
        assert!(markup.contains("data-code-preview-state=\"unavailable\""));
        assert!(markup.contains("Code preview unavailable"));
        assert!(markup.contains("unsupported &lt;future&gt; &amp; &quot;shape&quot;"));
        assert!(markup.contains("Intent IR remains available"));
        assert!(!markup.contains("data-code-action=\"promote-ordinary\""));
        assert!(!markup.contains("unsupported <future>"));
    }

    #[test]
    fn retained_failed_rebind_keeps_code_visible_without_promoting_hybrid_authority() {
        let mut editor = ordinary_rectangle_diagonal();
        let diagonal = editor
            .coordinator()
            .intent()
            .graph()
            .node_by_symbol(&geosolve_sketch_intent::IntentKey::new("gui.diagonal").unwrap())
            .unwrap();
        let diagonal_id = diagonal.id;
        let end = diagonal.inputs
            [&geosolve_sketch_intent::InputSlot::new(geosolve_sketch_intent::InputRole::Point, 1)];
        let outcome = editor
            .apply_patch(geosolve_sketch_intent::IntentPatch::new(
                editor.coordinator().intent().identity(),
                geosolve_sketch_intent::IntentPatchPolicy::RetainFailedIntent,
                vec![geosolve_sketch_intent::IntentPatchOperation::RebindInput {
                    node: diagonal_id,
                    slot: geosolve_sketch_intent::InputSlot::new(
                        geosolve_sketch_intent::InputRole::Point,
                        0,
                    ),
                    source: geosolve_sketch_intent::PatchPortRef::Stable { port: end },
                }],
            ))
            .unwrap();
        assert_eq!(
            outcome.disposition,
            geosolve_sketch_intent::IntentPlanDisposition::RetainedFailed,
        );
        assert!(!ordinary_code_starter_available(&editor));

        let OrdinaryCodeSurface::Unavailable { reason } = OrdinaryCodeSurface::from_editor(&editor)
        else {
            panic!("retained-failed intent must not expose promotable hybrid code")
        };
        assert!(reason.contains("does not belong to the current retained intent"));
        let markup = OrdinaryCodeSurface::Unavailable { reason }.panel_markup();
        assert!(markup.contains("data-code-preview-state=\"unavailable\""));
        assert!(markup.contains("Code preview unavailable"));
        assert!(markup.contains("Intent IR remains available"));
        assert!(!markup.contains("data-code-action=\"promote-ordinary\""));
    }

    #[test]
    fn ordinary_rectangle_diagonal_preview_uses_lexical_feature_references() {
        let editor = ordinary_rectangle_diagonal();
        assert!(!ordinary_code_starter_available(&editor));
        let preview = OrdinaryCodePreview::from_editor(&editor).unwrap();
        assert!(
            preview
                .source
                .contains("const frame = $.geometry.rectangle")
        );
        assert!(preview.source.contains("const diagonal = $.geometry.line"));
        assert!(preview.source.contains("start: frame.corners.lowerLeft"));
        assert!(preview.source.contains("end: frame.corners.upperRight"));
        assert!(!preview.source.contains("{\"declaration\":"));

        let markup = preview.panel_markup();
        assert!(markup.contains("Managed code preview"));
        assert!(markup.contains("data-code-action=\"promote-ordinary\""));
        assert!(markup.contains("Read-only · not authority"));
        assert!(!markup.contains("<textarea"));
    }

    #[test]
    fn m84_f004_two_lines_and_fillet_project_and_promote_as_managed_code() {
        let editor = ordinary_line_fillet();
        let preview = OrdinaryCodePreview::from_editor(&editor).unwrap();
        assert_eq!(preview.declaration_count, 5);
        assert!(preview.source.contains("const line = $.geometry.line"));
        assert!(preview.source.contains("const line2 = $.geometry.line"));
        assert!(preview.source.contains("start: line.end"));
        assert!(
            preview
                .source
                .contains("const horizontal = $.constraint.horizontal")
        );
        assert!(preview.source.contains("curve: line.span"));
        assert!(
            preview
                .source
                .contains("const vertical = $.constraint.vertical")
        );
        assert!(preview.source.contains("curve: line2.span"));
        assert!(
            preview
                .source
                .contains("const fillet = $.computed.filletSet")
        );
        assert!(preview.source.contains("span: line.span"));
        assert!(preview.source.contains("span: line2.span"));
        assert!(preview.source.contains("parents: ["));
        assert!(!preview.source.contains("{\"declaration\":"));
        assert!(
            preview
                .panel_markup()
                .contains("data-code-action=\"promote-ordinary\"")
        );

        let (workbench, promoted) = CodeProjectWorkbench::promote_from_editor(&editor).unwrap();
        assert_eq!(workbench.demo_key(), None);
        assert_eq!(workbench.managed_source(), preview.source);
        let accepted = promoted.coordinator().accepted_materialization().unwrap();
        assert!(accepted.validation.hard_residuals_validated);
        assert!(accepted.validation.all_active_features_current);
        assert_eq!(accepted.validation.feature_count, 1);
        let constraints = accepted.session.design_document().constraints();
        assert_eq!(constraints.len(), 2);
        assert!(constraints.iter().any(|constraint| matches!(
            constraint.definition,
            geosolve_sketch::DocumentConstraintDefinition::Horizontal { .. }
        )));
        assert!(constraints.iter().any(|constraint| matches!(
            constraint.definition,
            geosolve_sketch::DocumentConstraintDefinition::Vertical { .. }
        )));
        assert!(
            accepted
                .validation
                .maximum_normalized_hard_residual
                .is_none_or(|value| value.is_finite() && value <= 1.0e-9)
        );
        let persistence = workbench.to_persistence_json().unwrap();
        let restored = CodeProjectWorkbench::from_persistence_json(&persistence).unwrap();
        assert_eq!(restored.demo_key(), None);
        assert_eq!(restored.managed_source(), preview.source);
        assert_eq!(restored.to_persistence_json().unwrap(), persistence);
    }

    #[test]
    fn fresh_workspace_foundation_does_not_hide_complete_lexical_code_preview() {
        std::thread::Builder::new()
            .name("m84-f003-fresh-workspace-code-preview".into())
            .stack_size(32 * 1024 * 1024)
            .spawn(|| {
                let mut authority = super::super::fresh_projectional_authority().unwrap();
                let editor = authority.projectional_mut().unwrap();
                let foundation = editor
                    .coordinator()
                    .intent()
                    .graph()
                    .nodes()
                    .values()
                    .collect::<Vec<_>>();
                assert_eq!(foundation.len(), 1);
                assert!(matches!(
                    foundation[0].kind,
                    IntentNodeKind::Bootstrap { ref object }
                        if object.kind == BootstrapNativeKind::Document
                            && object.codec.as_str() == BOOTSTRAP_DOCUMENT_HEADER_CODEC_V1
                ));
                let Err(empty_error) = OrdinaryCodePreview::from_editor(editor) else {
                    panic!("an infrastructure-only workspace must not expose an empty project")
                };
                assert_eq!(
                    empty_error,
                    "the ordinary sketch has no authored declarations to promote",
                );

                add_ordinary_rectangle_diagonal(editor);
                assert_eq!(
                    editor.coordinator().intent().graph().nodes().len(),
                    3,
                    "fresh authority must retain its foundation beside both authored declarations",
                );
                let preview = OrdinaryCodePreview::from_editor(editor).unwrap();
                assert_eq!(preview.declaration_count, 2);
                assert!(
                    preview
                        .source
                        .contains("const frame = $.geometry.rectangle")
                );
                assert!(preview.source.contains("const diagonal = $.geometry.line"));
                assert!(preview.source.contains("start: frame.corners.lowerLeft"));
                assert!(preview.source.contains("end: frame.corners.upperRight"));

                let (promoted, promoted_editor) =
                    CodeProjectWorkbench::promote_from_editor(editor).unwrap();
                assert_eq!(promoted.demo_key(), None);
                assert_eq!(promoted.project.managed.program.declarations.len(), 2);
                assert!(
                    promoted_editor
                        .coordinator()
                        .accepted_materialization()
                        .unwrap()
                        .validation
                        .hard_residuals_validated
                );
            })
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    fn ordinary_projection_never_omits_nonfoundation_bootstrap_objects() {
        let mut document = geosolve_sketch::SketchDocument::new(1.0).unwrap();
        document.add_point("legacy point", [2.0, 3.0]).unwrap();
        let session = geosolve_sketch::RetainedSketchDocumentSession::new(
            document,
            geosolve_sketch::DocumentSolveRequest::default(),
            geosolve_core::SolverConfig::default(),
        )
        .unwrap();
        let coordinator =
            geosolve_constraint_editor::RetainedEditorCoordinator::new(session).unwrap();
        let authority =
            super::super::WorkbenchDocumentAuthority::from_flat_coordinator(&coordinator).unwrap();
        let editor = authority.projectional_ref().unwrap();
        assert!(!ordinary_code_starter_available(editor));
        let bootstrap_kinds = editor
            .coordinator()
            .intent()
            .graph()
            .nodes()
            .values()
            .filter_map(|node| match node.kind {
                IntentNodeKind::Bootstrap { ref object } => Some(object.kind),
                _ => None,
            })
            .collect::<BTreeSet<_>>();
        assert_eq!(
            bootstrap_kinds,
            BTreeSet::from([BootstrapNativeKind::Document, BootstrapNativeKind::Point])
        );
        let Err(error) = OrdinaryCodePreview::from_editor(editor) else {
            panic!("a legacy point bootstrap must not disappear from managed promotion")
        };
        assert!(error.contains("depends on unselected declaration node"));
        let OrdinaryCodeSurface::Unavailable { reason } = OrdinaryCodeSurface::from_editor(editor)
        else {
            panic!("an unsupported complete projection must remain a visible diagnostic")
        };
        assert_eq!(reason, error);
        let markup = OrdinaryCodeSurface::Unavailable { reason }.panel_markup();
        assert!(markup.contains("Code preview unavailable"));
        assert!(!markup.contains("data-code-action=\"promote-ordinary\""));
    }

    #[test]
    fn ordinary_promotion_creates_one_real_project_and_round_trips_without_demo_identity() {
        let editor = ordinary_rectangle_diagonal();
        let (workbench, promoted_editor) =
            CodeProjectWorkbench::promote_from_editor(&editor).unwrap();
        assert_eq!(workbench.demo_key(), None);
        assert_eq!(
            workbench.session.snapshot().code_project.as_ref(),
            Some(&workbench.project)
        );
        assert_eq!(
            workbench.session.snapshot().accepted_code_project.as_ref(),
            Some(&workbench.project)
        );
        assert!(
            promoted_editor
                .coordinator()
                .accepted_materialization()
                .unwrap()
                .validation
                .hard_residuals_validated
        );

        let json = workbench.to_persistence_json().unwrap();
        let wire: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(wire["origin"]["kind"], "promoted");
        assert!(wire.get("demo").is_none());
        let restored = CodeProjectWorkbench::from_persistence_json(&json).unwrap();
        assert_eq!(restored.demo_key(), None);
        assert_eq!(restored.managed_source(), workbench.managed_source());
        assert_eq!(restored.to_persistence_json().unwrap(), json);
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "one end-to-end promotion regression audits overlay history, semantic dependency and native movement together"
    )]
    fn promoted_rectangle_overlay_edit_moves_lexically_dependent_line() {
        let editor = ordinary_rectangle_diagonal();
        let (mut workbench, mut promoted) =
            CodeProjectWorkbench::promote_from_editor(&editor).unwrap();
        let source_before = workbench.managed_source().to_owned();
        let expansion = workbench
            .session
            .snapshot()
            .accepted_expansion
            .clone()
            .unwrap();
        let frame = SemanticSymbol("frame".into());
        let frame_alias = managed_rectangle_alias(&workbench.project, &expansion, &frame).unwrap();
        let mut operations = managed_rectangle_leaves(&promoted, &workbench.project, &expansion)
            .unwrap()
            .into_iter()
            .filter_map(|(leaf, owner)| match owner {
                WritableCodeLeaf::ManagedRectangle { argument, .. } if argument == "upperRight" => {
                    Some(
                        geosolve_sketch_intent::IntentPatchOperation::SetInstanceLeaf {
                            leaf,
                            value: IntentLiteral::Quantity {
                                value: match leaf.field {
                                    LeafField::X => 10.0,
                                    LeafField::Y => 6.0,
                                    _ => unreachable!("rectangle placement owns Cartesian leaves"),
                                },
                                unit: IntentUnit::Length,
                            },
                        },
                    )
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        operations.sort_by_key(|operation| match operation {
            geosolve_sketch_intent::IntentPatchOperation::SetInstanceLeaf { leaf, .. } => {
                leaf.field
            }
            _ => unreachable!(),
        });
        promoted
            .apply_patch(geosolve_sketch_intent::IntentPatch::new(
                promoted.coordinator().intent().identity(),
                geosolve_sketch_intent::IntentPatchPolicy::RequireAccepted,
                operations,
            ))
            .unwrap();
        let checkpoint = encode_editor_checkpoint(&promoted).unwrap();
        let publication = workbench
            .publish_delegated_editor_checkpoint(checkpoint, "Drag promoted frame")
            .unwrap()
            .unwrap();
        assert_eq!(workbench.managed_source(), source_before);
        assert_eq!(
            workbench
                .session
                .snapshot()
                .interaction_overlay
                .drafts()
                .len(),
            2,
        );
        assert_eq!(
            managed_rectangle_argument_position(&publication.editor, &frame_alias, "upperRight")
                .unwrap()
                .map(f64::to_bits),
            [10.0, 6.0].map(f64::to_bits),
        );

        let accepted_expansion = workbench
            .session
            .snapshot()
            .accepted_expansion
            .as_ref()
            .unwrap();
        let line_alias = accepted_expansion
            .semantic_outputs
            .values()
            .find_map(|output| {
                (output.reference.declaration == SemanticSymbol("diagonal".into())
                    && output.reference.output.0.is_empty())
                .then(|| match &output.target {
                    ExpandedSemanticTarget::Declaration { alias, .. } => Some(alias.clone()),
                    _ => None,
                })
                .flatten()
            })
            .unwrap();
        let line = publication
            .editor
            .coordinator()
            .intent()
            .graph()
            .node_by_symbol(&line_alias)
            .unwrap();
        let end = line
            .port_by_selector(IntentPortSelector::Node {
                role: IntentPortRole::End,
                index: 0,
            })
            .unwrap();
        let accepted = publication
            .editor
            .coordinator()
            .accepted_materialization()
            .unwrap();
        let IntentNativeBinding::Point(end) = accepted.ownership.port(end.as_ref(line.id)).unwrap()
        else {
            panic!("managed line end must resolve to a native point")
        };
        assert_eq!(
            accepted
                .session
                .design_document()
                .point(end)
                .unwrap()
                .position
                .map(f64::to_bits),
            [10.0, 6.0].map(f64::to_bits),
        );
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "the regression proves both expansion-owned inclusion and ordinary explicit-branch exclusion with one shared native authority"
    )]
    fn recomputable_code_line_branches_exclude_ordinary_gui_segments() {
        let editor = ordinary_rectangle_diagonal();
        let (workbench, mut promoted) = CodeProjectWorkbench::promote_from_editor(&editor).unwrap();
        let expansion = workbench
            .session
            .snapshot()
            .accepted_expansion
            .clone()
            .unwrap();
        let managed_line_alias = expansion
            .semantic_outputs
            .values()
            .find_map(|output| {
                (output.reference.declaration == SemanticSymbol("diagonal".into()))
                    .then(|| match &output.target {
                        ExpandedSemanticTarget::Declaration { alias, .. } => Some(alias.clone()),
                        _ => None,
                    })
                    .flatten()
            })
            .expect("managed line declaration");
        let managed_line_node = promoted
            .coordinator()
            .intent()
            .graph()
            .node_by_symbol(&managed_line_alias)
            .expect("managed line node")
            .id;
        let IntentNativeBinding::Curve(managed_line_curve) =
            native_binding_for_node(&promoted, managed_line_node, |binding| {
                matches!(binding, IntentNativeBinding::Curve(_))
            })
        else {
            panic!("managed line must own one native curve")
        };

        let selector = |role| IntentPortSelector::Node { role, index: 0 };
        let length = |value| IntentLiteral::Quantity {
            value,
            unit: IntentUnit::Length,
        };
        let gui_alias = geosolve_sketch_intent::IntentKey::new("gui-parity-segment").unwrap();
        let gui_segment = geosolve_sketch_intent::IntentNodeDraft::new(
            IntentNodeKind::Geometry {
                recipe: GeometryRecipeKind::Segment,
            },
            geosolve_sketch_intent::IntentKey::new("gui.parity-segment").unwrap(),
        )
        .with_instance_leaf(selector(IntentPortRole::Start), LeafField::X, length(20.0))
        .with_instance_leaf(selector(IntentPortRole::Start), LeafField::Y, length(20.0))
        .with_instance_leaf(selector(IntentPortRole::End), LeafField::X, length(30.0))
        .with_instance_leaf(selector(IntentPortRole::End), LeafField::Y, length(20.0))
        .with_field(
            geosolve_sketch_intent::IntentFieldKey(
                geosolve_sketch_intent::IntentKey::new("branch_direction").unwrap(),
            ),
            IntentLiteral::Point([1.0, 0.0]),
        );
        let outcome = promoted
            .apply_patch(geosolve_sketch_intent::IntentPatch::new(
                promoted.coordinator().intent().identity(),
                geosolve_sketch_intent::IntentPatchPolicy::RequireAccepted,
                vec![geosolve_sketch_intent::IntentPatchOperation::CreateNode {
                    alias: gui_alias.clone(),
                    draft: Box::new(gui_segment),
                    cell: None,
                }],
            ))
            .expect("ordinary GUI Segment");
        let gui_node = outcome.aliases.node(&gui_alias).expect("ordinary GUI node");
        let IntentNativeBinding::Curve(gui_curve) =
            native_binding_for_node(&promoted, gui_node, |binding| {
                matches!(binding, IntentNativeBinding::Curve(_))
            })
        else {
            panic!("ordinary GUI Segment must own one native curve")
        };

        let recomputable = recomputable_code_line_branches(&promoted, &expansion).unwrap();
        assert!(
            recomputable.contains(&managed_line_curve),
            "the expansion-owned managed line remains source-derived"
        );
        assert!(
            !recomputable.contains(&gui_curve),
            "an ordinary GUI Segment must retain exact explicit branch authority"
        );

        let document = promoted
            .coordinator()
            .accepted_materialization()
            .unwrap()
            .session
            .design_document()
            .clone();
        let current = match &document.curve(gui_curve).unwrap().definition {
            geosolve_sketch::CurveDefinition::Line {
                branch_direction, ..
            } => *branch_direction,
            _ => panic!("ordinary GUI Segment must materialize as a line"),
        };
        let (sine, cosine) = 1.0e-10_f64.sin_cos();
        let replacement = [
            cosine * current[0] - sine * current[1],
            sine * current[0] + cosine * current[1],
        ];
        let mut same_cell_mismatch = document.clone();
        same_cell_mismatch
            .set_curve_branch(geosolve_sketch::CurveSpan::line(gui_curve), replacement)
            .expect("same-cell explicit branch mismatch remains a valid document");

        let mut intentionally_overbroad = recomputable.clone();
        intentionally_overbroad.insert(gui_curve);
        assert!(
            document.exact_except_recomputable_line_branches(
                &same_cell_mismatch,
                &intentionally_overbroad,
            ),
            "the fixture must exercise the normalization leak"
        );
        assert!(
            !document.exact_except_recomputable_line_branches(&same_cell_mismatch, &recomputable,),
            "the exact expansion-owned set must reject an ordinary Segment mismatch"
        );
    }

    #[test]
    fn static_workbench_has_one_discoverable_optional_code_projection_and_bounded_editor_styles() {
        let html = include_str!("../../index.html");
        let css = include_str!("../../styles.css");
        assert_eq!(html.matches("id=\"wb-design-tab-code\"").count(), 1);
        assert_eq!(html.matches("id=\"wb-design-code\"").count(), 1);
        assert!(html.contains("data-wb-design-tab=\"code\""));
        assert!(html.contains("data-wb-design-tab=\"code\" tabindex=\"-1\">Code</button>"));
        assert!(html.contains(">Intent IR</button>"));
        assert!(!html.contains(">Structured source</button>"));
        for selector in [
            ".wb-code-project",
            ".wb-code-file-tabs",
            ".wb-code-editor textarea",
            ".wb-code-artifact-status",
            ".wb-code-lenses",
            ".wb-code-member",
        ] {
            assert!(css.contains(selector), "missing `{selector}`");
        }
        assert!(css.contains("max-height: 27rem"));
    }

    #[test]
    fn wasm_adapter_routes_code_projects_only_at_durable_boundaries() {
        let source = include_str!("mod.rs");
        for route in [
            "data-code-sample-id",
            "open_projectional_code_project",
            "start_projectional_code_project",
            "install_projectional_code_project",
            "\"start-authored\" =>",
            "promote_projectional_code_project",
            "\"promote-ordinary\" =>",
            "selected_code_delete_target(wb.editor())",
            "CodeDeleteTarget::GeneratedChild { .. }",
            ".suppress_generated_child(&target)?",
            "apply_managed_draft()",
            "reset_override(&path)",
            "reset_semantic_draft(&path)",
            "reset_generated_child_suppression(&path)",
            "to_persistence_json()",
            "from_persistence_json(&decoded)",
            "focus_code_source && result.is_ok()",
        ] {
            assert!(source.contains(route), "missing durable route `{route}`");
        }
        let transient = source
            .split("fn render_projectional_canvas(")
            .nth(1)
            .and_then(|source| source.split("fn save_projectional(").next())
            .expect("transient projectional renderer");
        for forbidden in [
            "parse_managed_source",
            "apply_managed_draft",
            "panel_markup",
            "to_persistence_json",
        ] {
            assert!(
                !transient.contains(forbidden),
                "pointer-frame renderer admitted `{forbidden}`"
            );
        }
        let runtime_evaluation_call = ["ev", "al("].concat();
        assert!(!include_str!("code_projects.rs").contains(&runtime_evaluation_call));

        let install = source
            .split("fn install_projectional_code_project(")
            .nth(1)
            .and_then(|source| source.split("fn promote_projectional_code_project(").next())
            .expect("bounded code-project installation adapter");
        assert!(install.contains("fit_projectional_camera_to_authority"));
        assert!(
            !install.contains("camera.reset()"),
            "an accepted code scene must be fitted rather than left on the origin camera"
        );
    }

    #[test]
    fn complete_offline_project_session_draft_and_file_selection_round_trip() {
        let mut workbench = open("mounting-plate");
        workbench
            .select_file("patches/mounting-plate.patch.ts")
            .unwrap();
        let custom_before = workbench
            .project
            .custom_files
            .values()
            .map(|file| (file.path.clone(), file.contents.clone()))
            .collect::<BTreeMap<_, _>>();
        workbench.set_managed_draft(format!(
            "{}\n// unapplied presentation draft",
            workbench.managed_source()
        ));
        let json = workbench.to_persistence_json().unwrap();
        let restored = CodeProjectWorkbench::from_persistence_json(&json).unwrap();
        assert_eq!(
            restored.selected_file.path(),
            "patches/mounting-plate.patch.ts"
        );
        assert!(
            restored
                .managed_draft
                .ends_with("unapplied presentation draft")
        );
        assert_eq!(
            restored
                .project
                .custom_files
                .values()
                .map(|file| (file.path.clone(), file.contents.clone()))
                .collect::<BTreeMap<_, _>>(),
            custom_before,
        );
        assert_eq!(restored.to_persistence_json().unwrap(), json);
    }

    #[test]
    fn managed_presentation_draft_obeys_the_managed_source_bound_on_save_and_load() {
        let mut workbench = open("braced-frame");
        workbench.set_managed_draft(" ".repeat(geosolve_sketch_code::MANAGED_SOURCE_LIMIT + 1));
        let Err(error) = workbench.to_persistence_json() else {
            panic!("an oversized managed draft must not enter persistence")
        };
        assert!(error.contains("managed source draft"));

        let workbench = open("braced-frame");
        let mut wire: serde_json::Value =
            serde_json::from_str(&workbench.to_persistence_json().unwrap()).unwrap();
        wire["managed_draft"] =
            serde_json::Value::String(" ".repeat(geosolve_sketch_code::MANAGED_SOURCE_LIMIT + 1));
        let Err(error) = CodeProjectWorkbench::from_persistence_json(&wire.to_string()) else {
            panic!("an oversized persisted managed draft must reject atomically")
        };
        assert!(error.contains("managed source draft"));
    }

    #[test]
    fn historical_bundled_demo_wire_migrates_to_explicit_origin() {
        let workbench = open("braced-frame");
        let json = workbench.to_persistence_json().unwrap();
        let mut wire: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(wire["origin"]["kind"], "bundled");
        assert_eq!(wire["origin"]["demo"], "braced-frame");
        wire.as_object_mut().unwrap().remove("origin");
        wire["demo"] = serde_json::Value::String("braced-frame".into());

        let restored = CodeProjectWorkbench::from_persistence_json(&wire.to_string()).unwrap();
        assert_eq!(restored.demo_key(), Some("braced-frame"));
        let migrated: serde_json::Value =
            serde_json::from_str(&restored.to_persistence_json().unwrap()).unwrap();
        assert_eq!(migrated["origin"]["kind"], "bundled");
        assert!(migrated.get("demo").is_none());
    }

    #[test]
    fn authored_origin_rejects_a_conflicting_legacy_demo_identity() {
        let (workbench, _) = CodeProjectWorkbench::new_authored().unwrap();
        let mut wire: serde_json::Value =
            serde_json::from_str(&workbench.to_persistence_json().unwrap()).unwrap();
        assert_eq!(wire["origin"]["kind"], "authored");
        wire["demo"] = serde_json::Value::String("braced-frame".into());
        let Err(error) = CodeProjectWorkbench::from_persistence_json(&wire.to_string()) else {
            panic!("conflicting authored and bundled origins must reject")
        };
        assert!(error.contains("authored code-project origin cannot name a bundled demo"));
    }

    #[test]
    fn tampered_nested_code_session_rejects_before_restore() {
        let workbench = open("braced-frame");
        let json = workbench.to_persistence_json().unwrap();
        let mut wire: serde_json::Value = serde_json::from_str(&json).unwrap();
        let session = wire["session"].as_str().unwrap();
        wire["session"] = serde_json::Value::String(
            session.replace("m84-demo-braced-frame", "m84-demo-tampered-frame"),
        );
        assert!(CodeProjectWorkbench::from_persistence_json(&wire.to_string()).is_err());
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "one real pointer regression proves selected-consumer detachment, preview, terminal publication, and source/producer retention together"
    )]
    fn selected_referenced_endpoint_pointer_drag_detaches_only_the_consumer() {
        use geosolve_constraint_editor::{Modifiers, PointerInput, Viewport};

        let (workbench, mut editor) = CodeProjectWorkbench::new_authored().unwrap();
        let mut workbench = Box::new(workbench);
        let expansion = workbench
            .session
            .snapshot()
            .accepted_expansion
            .clone()
            .unwrap();
        let alias_for = |declaration: &str| {
            expansion
                .semantic_outputs
                .values()
                .find_map(|output| {
                    (output.reference.declaration == SemanticSymbol(declaration.into())
                        && output.reference.output.0.is_empty())
                    .then(|| match &output.target {
                        ExpandedSemanticTarget::Declaration { alias, .. } => Some(alias.clone()),
                        _ => None,
                    })
                    .flatten()
                })
                .unwrap()
        };
        let frame_alias = alias_for("frame");
        let diagonal_alias = alias_for("diagonal");
        let frame = editor
            .coordinator()
            .intent()
            .graph()
            .node_by_symbol(&frame_alias)
            .unwrap();
        let frame_node = frame.id;
        let frame_corner = frame
            .port_by_selector(IntentPortSelector::Node {
                role: IntentPortRole::Corner,
                index: 0,
            })
            .unwrap()
            .as_ref(frame_node);
        let diagonal = editor
            .coordinator()
            .intent()
            .graph()
            .node_by_symbol(&diagonal_alias)
            .unwrap();
        let diagonal_node = diagonal.id;
        let diagonal_start = diagonal
            .port_by_selector(IntentPortSelector::Node {
                role: IntentPortRole::Start,
                index: 0,
            })
            .unwrap()
            .as_ref(diagonal_node);
        let diagonal_span = diagonal
            .port_by_selector(IntentPortSelector::Node {
                role: IntentPortRole::Span,
                index: 0,
            })
            .unwrap()
            .as_ref(diagonal_node);
        let accepted = editor.coordinator().accepted_materialization().unwrap();
        let IntentNativeBinding::Point(shared_point) =
            accepted.ownership.port(frame_corner).unwrap()
        else {
            panic!("frame corner must own one native point")
        };
        assert_eq!(
            accepted.ownership.port(diagonal_start),
            Some(IntentNativeBinding::Point(shared_point)),
        );
        let producer_before = accepted
            .session
            .design_document()
            .point(shared_point)
            .unwrap()
            .position;

        let dimension_alias =
            geosolve_sketch_intent::IntentKey::new("gui-diagonal-reference-length").unwrap();
        let dimension = geosolve_sketch_intent::IntentNodeDraft::new(
            IntentNodeKind::Dimension {
                dimension: geosolve_sketch_intent::DimensionKind::CurveLength,
            },
            geosolve_sketch_intent::IntentKey::new("Diagonal reference length").unwrap(),
        )
        .with_input(
            geosolve_sketch_intent::InputSlot::new(geosolve_sketch_intent::InputRole::Span, 0),
            geosolve_sketch_intent::PatchPortRef::Stable {
                port: diagonal_span,
            },
        )
        .with_field(
            geosolve_sketch_intent::IntentFieldKey(
                geosolve_sketch_intent::IntentKey::new("mode").unwrap(),
            ),
            IntentLiteral::Enum(geosolve_sketch_intent::IntentKey::new("reference").unwrap()),
        );
        let dimension = editor
            .apply_patch(geosolve_sketch_intent::IntentPatch::new(
                editor.coordinator().intent().identity(),
                geosolve_sketch_intent::IntentPatchPolicy::RequireAccepted,
                vec![geosolve_sketch_intent::IntentPatchOperation::CreateNode {
                    alias: dimension_alias.clone(),
                    draft: Box::new(dimension),
                    cell: None,
                }],
            ))
            .unwrap();
        let dimension_node = dimension.aliases.node(&dimension_alias).unwrap();
        let IntentNativeBinding::Dimension(dimension_id) =
            native_binding_for_node(&editor, dimension_node, |binding| {
                matches!(binding, IntentNativeBinding::Dimension(_))
            })
        else {
            unreachable!("dimension predicate returned a non-dimension binding")
        };
        let published_dimension = workbench
            .publish_delegated_editor_checkpoint(
                encode_editor_checkpoint(&editor).unwrap(),
                "Add GUI diagonal reference dimension",
            )
            .unwrap()
            .unwrap();
        editor = published_dimension.editor;
        let source_before = workbench.managed_source().to_owned();
        let revision_before = workbench.session.identity().revision;

        assert!(editor.set_selected_declaration(Some(diagonal_node)));
        let preferred = workbench
            .selected_managed_declaration(&editor)
            .unwrap()
            .unwrap();
        assert_eq!(preferred, SemanticSymbol("diagonal".into()));
        let viewport = Viewport::new([900.0, 700.0], [30.0, 17.5], 10.0).unwrap();
        let pointer = |id, model| PointerInput {
            pointer_id: id,
            position: viewport.model_to_screen(model),
            modifiers: Modifiers::default(),
        };
        let scene = editor.scene(viewport, 0.5).unwrap();
        editor
            .pointer_down(&scene, pointer(8405, producer_before))
            .unwrap();
        let route = editor.editor().prepared_point_drag_route().unwrap();
        assert_eq!(route.point, shared_point);
        let prepared = workbench
            .prepare_semantic_point_drag(&editor, 8405, route.point, Some(&preferred))
            .unwrap()
            .expect("selected referenced consumer must prepare local detachment");
        editor.cancel_interaction();
        editor = prepared.editor;
        let scene = editor.scene(viewport, 0.5).unwrap();
        editor
            .pointer_down_exact_point(&scene, pointer(8405, producer_before), prepared.point)
            .unwrap();
        let target = [5.0, 6.0];
        let scene = editor.scene(viewport, 0.5).unwrap();
        editor.pointer_move(&scene, pointer(8405, target)).unwrap();
        let scene = editor.scene(viewport, 0.5).unwrap();
        let terminal = editor.pointer_up(&scene, pointer(8405, target)).unwrap();
        assert!(terminal.transaction.is_some());
        let publication = workbench
            .publish_pointer_terminal_checkpoint(
                8405,
                &encode_editor_checkpoint(&editor).unwrap(),
                "Detach selected diagonal endpoint",
            )
            .unwrap()
            .unwrap();

        assert_eq!(workbench.managed_source(), source_before);
        assert_eq!(workbench.session.identity().revision, revision_before + 1);
        let accepted = publication
            .editor
            .coordinator()
            .accepted_materialization()
            .unwrap();
        let IntentNativeBinding::Point(frame_point) =
            accepted.ownership.port(frame_corner).unwrap()
        else {
            panic!("frame corner must remain a native point")
        };
        let detached_diagonal = publication
            .editor
            .coordinator()
            .intent()
            .graph()
            .node_by_symbol(&diagonal_alias)
            .unwrap();
        let detached_diagonal_start = detached_diagonal
            .port_by_selector(IntentPortSelector::Node {
                role: IntentPortRole::Start,
                index: 0,
            })
            .unwrap()
            .as_ref(detached_diagonal.id);
        let detached_diagonal_span = detached_diagonal
            .port_by_selector(IntentPortSelector::Node {
                role: IntentPortRole::Span,
                index: 0,
            })
            .unwrap()
            .as_ref(detached_diagonal.id);
        let IntentNativeBinding::Point(diagonal_point) =
            accepted.ownership.port(detached_diagonal_start).unwrap()
        else {
            panic!("diagonal endpoint must remain a native point")
        };
        assert_ne!(frame_point, diagonal_point);
        let surviving_dimension = publication
            .editor
            .coordinator()
            .intent()
            .graph()
            .node(dimension_node)
            .expect("ordinary GUI dimension must survive consumer detachment");
        assert_eq!(
            surviving_dimension.inputs[&geosolve_sketch_intent::InputSlot::new(
                geosolve_sketch_intent::InputRole::Span,
                0,
            )],
            detached_diagonal_span,
        );
        assert_eq!(
            native_binding_for_node(&publication.editor, dimension_node, |binding| {
                matches!(binding, IntentNativeBinding::Dimension(_))
            }),
            IntentNativeBinding::Dimension(dimension_id),
        );
        assert_eq!(
            accepted
                .session
                .design_document()
                .point(frame_point)
                .unwrap()
                .position
                .map(f64::to_bits),
            producer_before.map(f64::to_bits),
        );
        assert_eq!(
            accepted
                .session
                .design_document()
                .point(diagonal_point)
                .unwrap()
                .position
                .map(f64::to_bits),
            target.map(f64::to_bits),
        );
        assert!(
            workbench
                .session
                .snapshot()
                .interaction_overlay
                .drafts()
                .values()
                .any(|draft| draft.provenance
                    == geosolve_sketch_code::CodeDraftProvenance::DetachedReference),
        );
    }

    #[allow(
        clippy::too_many_lines,
        reason = "one real pointer regression proves producer ownership, rectangle seed coupling, and attached-consumer continuation together"
    )]
    fn assert_producer_pointer_drag_keeps_the_referenced_consumer_attached(select_producer: bool) {
        use geosolve_constraint_editor::{Modifiers, PointerInput, Viewport};

        let (workbench, mut editor) = CodeProjectWorkbench::new_authored().unwrap();
        let mut workbench = Box::new(workbench);
        let expansion = workbench
            .session
            .snapshot()
            .accepted_expansion
            .clone()
            .unwrap();
        let alias_for = |declaration: &str| {
            expansion
                .semantic_outputs
                .values()
                .find_map(|output| {
                    (output.reference.declaration == SemanticSymbol(declaration.into())
                        && output.reference.output.0.is_empty())
                    .then(|| match &output.target {
                        ExpandedSemanticTarget::Declaration { alias, .. } => Some(alias.clone()),
                        _ => None,
                    })
                    .flatten()
                })
                .unwrap()
        };
        let frame_alias = alias_for("frame");
        let diagonal_alias = alias_for("diagonal");
        let frame_node = editor
            .coordinator()
            .intent()
            .graph()
            .node_by_symbol(&frame_alias)
            .unwrap()
            .id;
        let diagonal_node = editor
            .coordinator()
            .intent()
            .graph()
            .node_by_symbol(&diagonal_alias)
            .unwrap()
            .id;
        let frame_corner = editor
            .coordinator()
            .intent()
            .graph()
            .node(frame_node)
            .unwrap()
            .port_by_selector(IntentPortSelector::Node {
                role: IntentPortRole::Corner,
                index: 0,
            })
            .unwrap()
            .as_ref(frame_node);
        let diagonal_start = editor
            .coordinator()
            .intent()
            .graph()
            .node(diagonal_node)
            .unwrap()
            .port_by_selector(IntentPortSelector::Node {
                role: IntentPortRole::Start,
                index: 0,
            })
            .unwrap()
            .as_ref(diagonal_node);
        let accepted = editor.coordinator().accepted_materialization().unwrap();
        let IntentNativeBinding::Point(shared_point) =
            accepted.ownership.port(frame_corner).unwrap()
        else {
            panic!("frame corner must own one native point")
        };
        assert_eq!(
            accepted.ownership.port(diagonal_start),
            Some(IntentNativeBinding::Point(shared_point)),
        );
        let start = accepted
            .session
            .design_document()
            .point(shared_point)
            .unwrap()
            .position;
        let source_before = workbench.managed_source().to_owned();
        let revision_before = workbench.session.identity().revision;

        let preferred = if select_producer {
            assert!(editor.set_selected_declaration(Some(frame_node)));
            let preferred = workbench
                .selected_managed_declaration(&editor)
                .unwrap()
                .unwrap();
            assert_eq!(preferred, SemanticSymbol("frame".into()));
            Some(preferred)
        } else {
            assert!(editor.editor().selection().is_empty());
            None
        };
        let viewport = Viewport::new([900.0, 700.0], [30.0, 17.5], 10.0).unwrap();
        let pointer = |id, model| PointerInput {
            pointer_id: id,
            position: viewport.model_to_screen(model),
            modifiers: Modifiers::default(),
        };
        let scene = editor.scene(viewport, 0.5).unwrap();
        editor.pointer_down(&scene, pointer(8406, start)).unwrap();
        let route = editor.editor().prepared_point_drag_route().unwrap();
        assert_eq!(route.point, shared_point);
        assert!(
            workbench
                .prepare_semantic_point_drag(&editor, 8406, route.point, preferred.as_ref())
                .unwrap()
                .is_none(),
            "producer selection or no selection must keep ordinary shared-point authority",
        );
        assert!(workbench.has_pending_semantic_point_drag(8406));
        let checkpoint_before = workbench.accepted_editor_checkpoint().clone();
        let overlay_before = workbench.session.snapshot().interaction_overlay.clone();
        let persistence_before = workbench.to_persistence_json().unwrap();
        let identity_before = workbench.session.identity().clone();
        let Err(generic_error) = workbench.publish_delegated_editor_checkpoint(
            checkpoint_before.clone(),
            "Generic save during semantic drag",
        ) else {
            panic!("a generic save must not consume semantic pointer authority")
        };
        assert!(generic_error.contains("only its terminal pointer"));
        assert!(workbench.has_pending_semantic_point_drag(8406));
        let Err(pointer_error) = workbench.publish_pointer_terminal_checkpoint(
            9999,
            &checkpoint_before,
            "Foreign terminal during semantic drag",
        ) else {
            panic!("another pointer must not consume semantic pointer authority")
        };
        assert!(pointer_error.contains("does not own"));
        assert!(workbench.has_pending_semantic_point_drag(8406));
        let Err(reentrant_error) =
            workbench.prepare_semantic_point_drag(&editor, 9998, shared_point, preferred.as_ref())
        else {
            panic!("a reentrant preparation must not evict the first semantic route")
        };
        assert!(reentrant_error.contains("still pending"));
        assert!(workbench.has_pending_semantic_point_drag(8406));
        assert_eq!(workbench.session.identity(), &identity_before);
        assert_eq!(
            workbench.session.snapshot().interaction_overlay,
            overlay_before,
        );
        assert_eq!(workbench.accepted_editor_checkpoint(), &checkpoint_before);
        assert_eq!(workbench.to_persistence_json().unwrap(), persistence_before);
        let target = [1.0, 0.5];
        let scene = editor.scene(viewport, 0.5).unwrap();
        editor
            .pointer_move(&scene, pointer(8406, [0.45, 0.225]))
            .unwrap();
        let scene = editor.scene(viewport, 0.5).unwrap();
        editor.pointer_move(&scene, pointer(8406, target)).unwrap();
        let scene = editor.scene(viewport, 0.5).unwrap();
        assert!(
            editor
                .pointer_up(&scene, pointer(8406, target))
                .unwrap()
                .transaction
                .is_some()
        );
        let terminal_position = editor
            .coordinator()
            .accepted_materialization()
            .unwrap()
            .session
            .design_document()
            .point(shared_point)
            .unwrap()
            .position;
        let publication = workbench
            .publish_pointer_terminal_editor(8406, &editor, "Move selected frame corner")
            .unwrap()
            .unwrap();

        assert_eq!(workbench.managed_source(), source_before);
        assert_eq!(workbench.session.identity().revision, revision_before + 1);
        let accepted = publication
            .editor
            .coordinator()
            .accepted_materialization()
            .unwrap();
        let IntentNativeBinding::Point(frame_point) =
            accepted.ownership.port(frame_corner).unwrap()
        else {
            panic!("frame corner must remain a native point")
        };
        assert_eq!(
            accepted.ownership.port(diagonal_start),
            Some(IntentNativeBinding::Point(frame_point)),
            "the attached consumer must continue sharing the producer point",
        );
        assert_eq!(
            accepted
                .session
                .design_document()
                .point(frame_point)
                .unwrap()
                .position
                .map(f64::to_bits),
            terminal_position.map(f64::to_bits),
        );
        assert_eq!(
            workbench
                .session
                .snapshot()
                .interaction_overlay
                .drafts()
                .len(),
            2,
            "rectangle placement records its coupled canonical seed bundle",
        );
        assert!(
            workbench
                .session
                .snapshot()
                .interaction_overlay
                .drafts()
                .values()
                .all(|draft| draft.provenance
                    == geosolve_sketch_code::CodeDraftProvenance::CanvasDrag),
        );
        assert_eq!(publication.editor.coordinator().intent().undo_len(), 0);
        assert_eq!(publication.editor.coordinator().intent().redo_len(), 0);
        let warm = workbench
            .materialized
            .as_deref()
            .expect("pointer terminal must retain its staged native cache");
        assert_eq!(warm.editor.coordinator().intent().undo_len(), 0);
        assert_eq!(warm.editor.coordinator().intent().redo_len(), 0);
        assert_eq!(
            warm.base_outcome.identity,
            warm.editor.coordinator().intent().identity(),
        );
        assert_eq!(
            warm.expansion.digest,
            workbench
                .session
                .snapshot()
                .accepted_expansion
                .as_ref()
                .expect("accepted terminal expansion")
                .digest,
        );
        let restored = rehydrate_editor_checkpoint(
            workbench.accepted_editor_checkpoint(),
            warm.expansion.clone(),
        )
        .expect("persisted staged terminal must independently restore");
        validate_terminal_native_parity(
            &publication.editor,
            &restored.editor,
            &restored.expansion,
            &[],
        )
        .expect("persisted staged terminal and retained cache must remain exact");
    }

    #[test]
    fn no_selection_pointer_drag_keeps_the_referenced_consumer_attached() {
        assert_producer_pointer_drag_keeps_the_referenced_consumer_attached(false);
    }

    #[test]
    fn selected_producer_pointer_drag_keeps_the_referenced_consumer_attached() {
        assert_producer_pointer_drag_keeps_the_referenced_consumer_attached(true);
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "one real Compass Rose pointer regression keeps accepted release, complete coupled parity, persistence, identity, finiteness and residual validation together"
    )]
    fn compass_rose_shared_center_terminal_matches_the_last_native_preview() {
        use geosolve_constraint_editor::{Modifiers, PointerInput, Viewport};

        let (mut workbench, mut editor) = open_boxed("compass-rose");
        let expansion = workbench
            .session
            .snapshot()
            .accepted_expansion
            .as_ref()
            .expect("accepted Compass Rose expansion");
        let center_lens = expansion
            .writable_points
            .iter()
            .find(|point| {
                !point.source.is_reference()
                    && expansion.declaration_for_alias(&point.handle.alias)
                        == Some(&SemanticSymbol("north".into()))
                    && point.handle.selector
                        == IntentPortSelector::Node {
                            role: IntentPortRole::Start,
                            index: 0,
                        }
            })
            .expect("north.start is the unique shared-center producer")
            .clone();
        let center = expanded_port_point(&editor, &center_lens.handle)
            .expect("Compass Rose shared center native point");
        let position = |editor: &ProjectionalEditorSession, point| {
            editor
                .coordinator()
                .accepted_materialization()
                .and_then(|accepted| accepted.session.accepted_state_for_current_input())
                .and_then(|accepted| accepted.document().point(point))
                .map(|point| point.position)
                .expect("accepted native point")
        };
        let origin = position(&editor, center);
        let target = [origin[0] + 6.0, origin[1] - 4.0];
        let pointer_id = 8_421;
        let viewport = Viewport::new([1_440.0, 900.0], [0.0, 0.0], 10.0).unwrap();
        let pointer = |model| PointerInput {
            pointer_id,
            position: viewport.model_to_screen(model),
            modifiers: Modifiers::default(),
        };

        let scene = editor.scene(viewport, 0.5).expect("pointer-down scene");
        editor
            .pointer_down(&scene, pointer(origin))
            .expect("shared-center pointer-down");
        let route = editor
            .editor()
            .prepared_point_drag_route()
            .expect("shared-center point route");
        assert_eq!(route.point, center);
        assert!(
            workbench
                .prepare_semantic_point_drag(&editor, pointer_id, center, None)
                .expect("unique producer semantic route")
                .is_none(),
        );

        let scene = editor.scene(viewport, 0.5).expect("pointer-move scene");
        editor
            .pointer_move(&scene, pointer(target))
            .expect("valid shared-center preview");
        let scene = editor
            .scene(viewport, 0.5)
            .expect("replayed terminal pointer-move scene");
        editor
            .pointer_move(&scene, pointer(target))
            .expect("valid duplicate terminal preview");
        let preview_position = editor
            .coordinator()
            .presentation_session()
            .and_then(|session| session.accepted_state_for_current_input())
            .and_then(|state| state.document().point(center))
            .map(|point| point.position)
            .expect("accepted shared-center preview position");
        assert_eq!(preview_position.map(f64::to_bits), target.map(f64::to_bits));

        let preview_scene = editor.scene(viewport, 0.5).expect("pointer-up scene");
        assert!(
            editor
                .pointer_up(&preview_scene, pointer(target))
                .expect("valid shared-center terminal")
                .transaction
                .is_some(),
        );
        assert_eq!(
            position(&editor, center).map(f64::to_bits),
            preview_position.map(f64::to_bits),
            "native terminal publication must retain the exact preview",
        );
        let terminal_authority = editor
            .coordinator()
            .accepted_materialization()
            .expect("terminal accepted materialization");
        let terminal_design = terminal_authority.session.design_document().clone();
        let terminal_accepted = terminal_authority
            .session
            .accepted_state_for_current_input()
            .expect("terminal current accepted state")
            .document()
            .clone();
        let revision_before = workbench.session.identity().revision;

        let publication = workbench
            .publish_pointer_terminal_editor(pointer_id, &editor, "Move Compass Rose shared center")
            .expect("semantic terminal publication")
            .expect("one outer history row");
        let published_center = expanded_port_point(&publication.editor, &center_lens.handle)
            .expect("published shared-center point");
        assert_eq!(
            position(&publication.editor, published_center).map(f64::to_bits),
            preview_position.map(f64::to_bits),
            "outer code publication must not replace a valid drop with another solution",
        );
        let accepted = publication
            .editor
            .coordinator()
            .accepted_materialization()
            .expect("published accepted materialization");
        let expansion = workbench
            .session
            .snapshot()
            .accepted_expansion
            .as_ref()
            .expect("published accepted expansion");
        let mut recomputable = recomputable_code_line_branches(&editor, expansion)
            .expect("terminal source-derived line branches");
        recomputable.extend(
            recomputable_code_line_branches(&publication.editor, expansion)
                .expect("published source-derived line branches"),
        );
        assert!(
            terminal_design.exact_except_recomputable_line_branches(
                accepted.session.design_document(),
                &recomputable,
            ),
            "outer code publication must preserve the complete coupled terminal design",
        );
        assert!(
            terminal_accepted.exact_except_recomputable_line_branches(
                accepted
                    .session
                    .accepted_state_for_current_input()
                    .expect("published current accepted state")
                    .document(),
                &recomputable,
            ),
            "outer code publication must preserve the exact solved terminal scene",
        );
        assert_eq!(workbench.session.identity().revision, revision_before + 1);
        assert_eq!(publication.editor.coordinator().intent().undo_len(), 0);
        assert_eq!(publication.editor.coordinator().intent().redo_len(), 0);
        let warm = workbench
            .materialized
            .as_deref()
            .expect("published warm materialization");
        assert_eq!(warm.editor.coordinator().intent().undo_len(), 0);
        assert_eq!(warm.editor.coordinator().intent().redo_len(), 0);
        assert_eq!(
            warm.base_outcome.identity,
            warm.editor.coordinator().intent().identity(),
            "history-neutral publication identity must describe the retained editor",
        );
        for declaration in ["east", "south", "west"] {
            let referenced_start = expansion
                .writable_points
                .iter()
                .find(|point| {
                    expansion.declaration_for_alias(&point.handle.alias)
                        == Some(&SemanticSymbol(declaration.into()))
                        && point.handle.selector
                            == IntentPortSelector::Node {
                                role: IntentPortRole::Start,
                                index: 0,
                            }
                })
                .and_then(|point| expanded_port_point(&publication.editor, &point.handle))
                .expect("published referenced spoke start");
            assert_eq!(
                referenced_start, published_center,
                "{declaration}.start must remain attached to north.start",
            );
        }
        assert!(accepted.validation.hard_residuals_validated);
        assert!(accepted.validation.all_active_features_current);
        assert!(
            accepted
                .validation
                .maximum_normalized_hard_residual
                .is_none_or(|residual| residual.is_finite() && residual <= 1.0e-9),
        );
        assert!(
            accepted
                .session
                .accepted_state_for_current_input()
                .expect("published current accepted state")
                .document()
                .points()
                .iter()
                .flat_map(|point| point.position)
                .all(f64::is_finite),
        );
        let persisted = workbench
            .to_persistence_json()
            .expect("persisted code project");
        let restored =
            CodeProjectWorkbench::from_persistence_json(&persisted).expect("restored code project");
        assert_eq!(
            restored.accepted_editor_checkpoint(),
            workbench.accepted_editor_checkpoint(),
        );
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "one real generated-consumer gesture regression keeps detachment, repeat drag, semantic identity, native validation, and exact Undo/Redo adjacent"
    )]
    fn generated_referenced_endpoint_drag_is_repeatable_and_exactly_undoable() {
        std::thread::Builder::new()
            .name("m84-generated-consumer-pointer-drag".into())
            .stack_size(32 * 1024 * 1024)
            .spawn(|| {
                use geosolve_constraint_editor::{Modifiers, PointerInput, Viewport};

                let (mut workbench, mut editor) = open_boxed("braced-frame");
                let expansion = workbench
                    .session
                    .snapshot()
                    .accepted_expansion
                    .clone()
                    .unwrap();
                let rising_address = workbench
                    .session
                    .snapshot()
                    .generated
                    .active()
                    .keys()
                    .find(|address| {
                        address.invocation == "brace"
                            && address.template == ["diagonals", "rising"]
                            && address.output == ["span"]
                    })
                    .unwrap()
                    .clone();
                let generated_identity =
                    workbench.session.snapshot().generated.active()[&rising_address];
                let ExpandedSemanticTarget::Port { port: rising_span } =
                    &expansion.generated_provenance[&rising_address].target
                else {
                    panic!("generated rising brace must publish one span")
                };
                let rising_alias = rising_span.alias.clone();
                let rising_before = editor
                    .coordinator()
                    .intent()
                    .graph()
                    .node_by_symbol(&rising_alias)
                    .unwrap();
                let rising_before_id = rising_before.id;
                let rising_start = rising_before
                    .port_by_selector(IntentPortSelector::Node {
                        role: IntentPortRole::Start,
                        index: 0,
                    })
                    .unwrap()
                    .as_ref(rising_before_id);
                let accepted_before = editor.coordinator().accepted_materialization().unwrap();
                let IntentNativeBinding::Point(shared_point) =
                    accepted_before.ownership.port(rising_start).unwrap()
                else {
                    panic!("generated rising endpoint must alias a native point")
                };
                let start = accepted_before
                    .session
                    .design_document()
                    .point(shared_point)
                    .unwrap()
                    .position;
                let source_before = workbench.managed_source().to_owned();
                let checkpoint_before = workbench.accepted_editor_checkpoint().clone();
                let overlay_before = workbench.session.snapshot().interaction_overlay.clone();
                let revision_before = workbench.session.identity().revision;

                assert!(editor.set_selected_declaration(Some(rising_before_id)));
                let preferred = workbench
                    .selected_managed_declaration(&editor)
                    .unwrap()
                    .unwrap();
                assert_eq!(preferred, SemanticSymbol("brace".into()));
                let viewport = Viewport::new([900.0, 700.0], [30.0, 17.5], 10.0).unwrap();
                let pointer = |id, model| PointerInput {
                    pointer_id: id,
                    position: viewport.model_to_screen(model),
                    modifiers: Modifiers::default(),
                };
                let scene = editor.scene(viewport, 0.5).unwrap();
                editor.pointer_down(&scene, pointer(8410, start)).unwrap();
                let route = editor.editor().prepared_point_drag_route().unwrap();
                assert_eq!(route.point, shared_point);
                let prepared = workbench
                    .prepare_semantic_point_drag(&editor, 8410, shared_point, Some(&preferred))
                    .unwrap()
                    .expect("selected generated consumer must detach locally");
                editor.cancel_interaction();
                editor = prepared.editor;
                let scene = editor.scene(viewport, 0.5).unwrap();
                editor
                    .pointer_down_exact_point(&scene, pointer(8410, start), prepared.point)
                    .unwrap();
                let first_target = [5.0, 6.0];
                let scene = editor.scene(viewport, 0.5).unwrap();
                editor
                    .pointer_move(&scene, pointer(8410, first_target))
                    .unwrap();
                let scene = editor.scene(viewport, 0.5).unwrap();
                assert!(
                    editor
                        .pointer_up(&scene, pointer(8410, first_target))
                        .unwrap()
                        .transaction
                        .is_some()
                );
                let first = workbench
                    .publish_pointer_terminal_editor(
                        8410,
                        &editor,
                        "Detach generated rising brace endpoint",
                    )
                    .unwrap()
                    .unwrap();
                assert_eq!(workbench.managed_source(), source_before);
                assert_eq!(workbench.session.identity().revision, revision_before + 1);
                assert_eq!(
                    workbench.session.snapshot().generated.active()[&rising_address],
                    generated_identity,
                );
                assert!(
                    workbench
                        .session
                        .snapshot()
                        .interaction_overlay
                        .drafts()
                        .values()
                        .any(|draft| draft.provenance
                            == geosolve_sketch_code::CodeDraftProvenance::DetachedReference)
                );
                let first_checkpoint = workbench.accepted_editor_checkpoint().clone();
                let first_overlay = workbench.session.snapshot().interaction_overlay.clone();
                editor = first.editor;
                let rising_after = editor
                    .coordinator()
                    .intent()
                    .graph()
                    .node_by_symbol(&rising_alias)
                    .unwrap();
                assert_ne!(rising_after.id, rising_before_id);
                let detached_start = rising_after
                    .port_by_selector(IntentPortSelector::Node {
                        role: IntentPortRole::Start,
                        index: 0,
                    })
                    .unwrap()
                    .as_ref(rising_after.id);
                let accepted = editor.coordinator().accepted_materialization().unwrap();
                let IntentNativeBinding::Point(detached_point) =
                    accepted.ownership.port(detached_start).unwrap()
                else {
                    panic!("detached generated endpoint must remain a native point")
                };
                assert_ne!(detached_point, shared_point);
                assert_eq!(
                    accepted
                        .session
                        .design_document()
                        .point(detached_point)
                        .unwrap()
                        .position
                        .map(f64::to_bits),
                    first_target.map(f64::to_bits),
                );

                assert!(editor.set_selected_declaration(Some(rising_after.id)));
                let scene = editor.scene(viewport, 0.5).unwrap();
                editor
                    .pointer_down_exact_point(&scene, pointer(8411, first_target), detached_point)
                    .unwrap();
                assert!(
                    workbench
                        .prepare_semantic_point_drag(
                            &editor,
                            8411,
                            detached_point,
                            Some(&preferred),
                        )
                        .unwrap()
                        .is_none(),
                    "the detached endpoint must reuse its existing local lens",
                );
                let second_target = [7.0, 8.0];
                let scene = editor.scene(viewport, 0.5).unwrap();
                editor
                    .pointer_move(&scene, pointer(8411, second_target))
                    .unwrap();
                let scene = editor.scene(viewport, 0.5).unwrap();
                assert!(
                    editor
                        .pointer_up(&scene, pointer(8411, second_target))
                        .unwrap()
                        .transaction
                        .is_some()
                );
                let second = workbench
                    .publish_pointer_terminal_editor(
                        8411,
                        &editor,
                        "Move detached generated endpoint again",
                    )
                    .unwrap()
                    .unwrap();
                assert_eq!(workbench.session.identity().revision, revision_before + 2);
                assert_eq!(second.editor.coordinator().intent().undo_len(), 0);
                assert_eq!(second.editor.coordinator().intent().redo_len(), 0);
                let warm = workbench
                    .materialized
                    .as_deref()
                    .expect("repeated terminal warm materialization");
                assert_eq!(warm.editor.coordinator().intent().undo_len(), 0);
                assert_eq!(warm.editor.coordinator().intent().redo_len(), 0);
                assert_eq!(
                    warm.base_outcome.identity,
                    warm.editor.coordinator().intent().identity(),
                );
                let second_checkpoint = workbench.accepted_editor_checkpoint().clone();
                let accepted = second
                    .editor
                    .coordinator()
                    .accepted_materialization()
                    .unwrap();
                assert!(accepted.validation.hard_residuals_validated);
                assert!(accepted.validation.all_active_features_current);
                assert!(
                    accepted
                        .validation
                        .maximum_normalized_hard_residual
                        .is_none_or(|value| value.is_finite() && value <= 1.0e-9)
                );
                assert_eq!(
                    accepted
                        .session
                        .design_document()
                        .point(detached_point)
                        .unwrap()
                        .position
                        .map(f64::to_bits),
                    second_target.map(f64::to_bits),
                );

                let _ = workbench.step_history(true).unwrap().unwrap();
                assert_eq!(workbench.accepted_editor_checkpoint(), &first_checkpoint);
                assert_eq!(
                    workbench.session.snapshot().interaction_overlay,
                    first_overlay,
                );
                let _ = workbench.step_history(true).unwrap().unwrap();
                assert_eq!(workbench.accepted_editor_checkpoint(), &checkpoint_before);
                assert_eq!(
                    workbench.session.snapshot().interaction_overlay,
                    overlay_before,
                );
                let _ = workbench.step_history(false).unwrap().unwrap();
                assert_eq!(workbench.accepted_editor_checkpoint(), &first_checkpoint);
                let _ = workbench.step_history(false).unwrap().unwrap();
                assert_eq!(workbench.accepted_editor_checkpoint(), &second_checkpoint);
            })
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "the regression authenticates every transient detachment and cancellation authority before and after restoration"
    )]
    fn transient_reference_detachment_cancels_to_exact_accepted_authority() {
        let (workbench, mut editor) = CodeProjectWorkbench::new_authored().unwrap();
        let mut workbench = Box::new(workbench);
        let expansion = workbench
            .session
            .snapshot()
            .accepted_expansion
            .clone()
            .unwrap();
        let diagonal_alias = expansion
            .semantic_outputs
            .values()
            .find_map(|output| {
                (output.reference.declaration == SemanticSymbol("diagonal".into())
                    && output.reference.output.0.is_empty())
                .then(|| match &output.target {
                    ExpandedSemanticTarget::Declaration { alias, .. } => Some(alias.clone()),
                    _ => None,
                })
                .flatten()
            })
            .unwrap();
        let frame_alias = expansion
            .semantic_outputs
            .values()
            .find_map(|output| {
                (output.reference.declaration == SemanticSymbol("frame".into())
                    && output.reference.output.0.is_empty())
                .then(|| match &output.target {
                    ExpandedSemanticTarget::Declaration { alias, .. } => Some(alias.clone()),
                    _ => None,
                })
                .flatten()
            })
            .unwrap();
        let frame_node = editor
            .coordinator()
            .intent()
            .graph()
            .node_by_symbol(&frame_alias)
            .unwrap()
            .id;
        let diagonal_node = editor
            .coordinator()
            .intent()
            .graph()
            .node_by_symbol(&diagonal_alias)
            .unwrap()
            .id;
        let start = editor
            .coordinator()
            .intent()
            .graph()
            .node(diagonal_node)
            .unwrap()
            .port_by_selector(IntentPortSelector::Node {
                role: IntentPortRole::Start,
                index: 0,
            })
            .unwrap()
            .as_ref(diagonal_node);
        let IntentNativeBinding::Point(shared_point) = editor
            .coordinator()
            .accepted_materialization()
            .unwrap()
            .ownership
            .port(start)
            .unwrap()
        else {
            panic!("referenced line start must own one shared point")
        };
        assert!(editor.set_selected_declaration(Some(diagonal_node)));
        let preferred = workbench
            .selected_managed_declaration(&editor)
            .unwrap()
            .unwrap();
        let checkpoint_before = workbench.accepted_editor_checkpoint().clone();
        let source_before = workbench.managed_source().to_owned();
        let revision_before = workbench.session.identity().revision;
        let overlay_before = workbench.session.snapshot().interaction_overlay.clone();

        let prepared = workbench
            .prepare_semantic_point_drag(&editor, 8408, shared_point, Some(&preferred))
            .unwrap()
            .unwrap();
        assert!(workbench.has_pending_semantic_point_drag(8408));
        assert_ne!(
            encode_editor_checkpoint(&prepared.editor).unwrap(),
            checkpoint_before,
            "the transient editor must contain the local detached route",
        );
        assert!(
            workbench
                .cancel_semantic_point_drag(Some(9999))
                .unwrap()
                .is_none(),
            "another pointer cannot retire this semantic gesture",
        );
        assert!(workbench.has_pending_semantic_point_drag(8408));
        let restored = workbench
            .cancel_semantic_point_drag(Some(8408))
            .unwrap()
            .unwrap();
        assert_eq!(
            encode_editor_checkpoint(&restored).unwrap(),
            checkpoint_before,
        );
        assert!(!workbench.has_pending_semantic_point_drag(8408));
        assert_eq!(workbench.managed_source(), source_before);
        assert_eq!(workbench.session.identity().revision, revision_before);
        assert_eq!(
            workbench.session.snapshot().interaction_overlay,
            overlay_before,
        );

        assert!(editor.set_selected_declaration(Some(frame_node)));
        let producer = workbench
            .selected_managed_declaration(&editor)
            .unwrap()
            .unwrap();
        assert_eq!(producer, SemanticSymbol("frame".into()));
        assert!(
            workbench
                .prepare_semantic_point_drag(&editor, 8412, shared_point, Some(&producer))
                .unwrap()
                .is_none(),
            "the producer route needs no transient detachment",
        );
        assert!(workbench.has_pending_semantic_point_drag(8412));
        assert!(
            workbench
                .cancel_semantic_point_drag(Some(8412))
                .unwrap()
                .is_none(),
            "canceling an attached producer gesture needs no editor replacement",
        );
        assert!(!workbench.has_pending_semantic_point_drag(8412));
        assert_eq!(workbench.managed_source(), source_before);
        assert_eq!(workbench.session.identity().revision, revision_before);
        assert_eq!(workbench.accepted_editor_checkpoint(), &checkpoint_before);
        assert_eq!(
            workbench.session.snapshot().interaction_overlay,
            overlay_before,
        );

        workbench.set_managed_draft(format!("{}\n", workbench.managed_source()));
        let Err(error) =
            workbench.prepare_semantic_point_drag(&editor, 8409, shared_point, Some(&preferred))
        else {
            panic!("a dirty managed draft must refuse semantic preparation")
        };
        assert!(error.contains("Apply or Revert"));
        assert!(!workbench.has_pending_semantic_point_drag(8409));
        assert_eq!(workbench.session.identity().revision, revision_before);
        assert_eq!(workbench.accepted_editor_checkpoint(), &checkpoint_before);
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "the exact no-motion pointer lifecycle and every history-neutral authority invariant stay adjacent"
    )]
    fn no_motion_producer_pointer_release_retires_route_without_history() {
        use geosolve_constraint_editor::{Modifiers, PointerInput, Viewport};

        let (workbench, mut editor) = CodeProjectWorkbench::new_authored().unwrap();
        let mut workbench = Box::new(workbench);
        let expansion = workbench
            .session
            .snapshot()
            .accepted_expansion
            .as_ref()
            .unwrap();
        let producer = expansion
            .writable_points
            .iter()
            .find(|point| {
                !point.source.is_reference()
                    && expansion.declaration_for_alias(&point.handle.alias)
                        == Some(&SemanticSymbol("frame".into()))
            })
            .unwrap()
            .clone();
        let native_point = expanded_port_point(&editor, &producer.handle).unwrap();
        let position = editor
            .coordinator()
            .accepted_materialization()
            .unwrap()
            .session
            .design_document()
            .point(native_point)
            .unwrap()
            .position;
        let viewport = Viewport::new([900.0, 700.0], [30.0, 17.5], 10.0).unwrap();
        let input = PointerInput {
            pointer_id: 8_415,
            position: viewport.model_to_screen(position),
            modifiers: Modifiers::default(),
        };
        let identity_before = workbench.session.identity().clone();
        let checkpoint_before = workbench.accepted_editor_checkpoint().clone();
        let overlay_before = workbench.session.snapshot().interaction_overlay.clone();
        let source_before = workbench.managed_source().to_owned();
        let persistence_before = workbench.to_persistence_json().unwrap();
        let undo_before = workbench.can_undo();
        let redo_before = workbench.can_redo();

        let scene = editor.scene(viewport, 0.5).unwrap();
        editor.pointer_down(&scene, input).unwrap();
        let route = editor.editor().prepared_point_drag_route().unwrap();
        assert_eq!(route.pointer_id, input.pointer_id);
        assert_eq!(route.point, native_point);
        assert!(
            workbench
                .prepare_semantic_point_drag(
                    &editor,
                    input.pointer_id,
                    route.point,
                    Some(&SemanticSymbol("frame".into())),
                )
                .unwrap()
                .is_none(),
            "a producer route needs no transient detachment",
        );
        assert!(workbench.has_pending_semantic_point_drag(input.pointer_id));

        let scene = editor.scene(viewport, 0.5).unwrap();
        let outcome = editor.pointer_up(&scene, input).unwrap();
        assert!(outcome.transaction.is_none());
        assert!(outcome.effects.is_empty());
        assert!(editor.editor().active_pointer_gesture().is_none());
        assert!(
            workbench
                .cancel_semantic_point_drag(Some(input.pointer_id))
                .unwrap()
                .is_none(),
            "an attached producer route needs no editor replacement on no-motion release",
        );
        assert!(!workbench.has_pending_semantic_point_drag(input.pointer_id));
        assert_eq!(workbench.session.identity(), &identity_before);
        assert_eq!(workbench.accepted_editor_checkpoint(), &checkpoint_before);
        assert_eq!(
            workbench.session.snapshot().interaction_overlay,
            overlay_before,
        );
        assert_eq!(workbench.managed_source(), source_before);
        assert_eq!(workbench.can_undo(), undo_before);
        assert_eq!(workbench.can_redo(), redo_before);
        assert_eq!(workbench.to_persistence_json().unwrap(), persistence_before);
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "the test constructs one internally coherent newer delegated authority to hit the exact stale-session terminal branch"
    )]
    fn terminal_rejects_stored_session_identity_mismatch_and_preserves_newer_authority() {
        let (workbench, mut editor) = CodeProjectWorkbench::new_authored().unwrap();
        let mut workbench = Box::new(workbench);
        let expansion = workbench
            .session
            .snapshot()
            .accepted_expansion
            .as_ref()
            .unwrap();
        let producer = expansion
            .writable_points
            .iter()
            .find(|point| {
                !point.source.is_reference()
                    && expansion.declaration_for_alias(&point.handle.alias)
                        == Some(&SemanticSymbol("frame".into()))
            })
            .unwrap()
            .clone();
        let native_point = expanded_port_point(&editor, &producer.handle).unwrap();
        assert!(
            workbench
                .prepare_semantic_point_drag(
                    &editor,
                    8_416,
                    native_point,
                    Some(&SemanticSymbol("frame".into())),
                )
                .unwrap()
                .is_none(),
        );
        assert!(workbench.has_pending_semantic_point_drag(8_416));
        let stale_checkpoint = workbench.accepted_editor_checkpoint().clone();
        let stale_identity = workbench.session.identity().clone();

        let patch = geosolve_sketch_intent::IntentPatch::new(
            editor.coordinator().intent().identity(),
            geosolve_sketch_intent::IntentPatchPolicy::RequireAccepted,
            vec![geosolve_sketch_intent::IntentPatchOperation::CreateCell {
                alias: geosolve_sketch_intent::IntentKey::new("identity-mismatch-cell").unwrap(),
                name: geosolve_sketch_intent::IntentKey::new("Newer GUI authority").unwrap(),
                before: None,
            }],
        );
        editor.apply_patch(patch).unwrap();
        let newer_checkpoint = encode_editor_checkpoint(&editor).unwrap();
        assert_ne!(newer_checkpoint, stale_checkpoint);
        let newer_cache = rehydrate_editor_checkpoint(
            &newer_checkpoint,
            workbench
                .session
                .snapshot()
                .accepted_expansion
                .clone()
                .unwrap(),
        )
        .unwrap();
        let prepared = workbench
            .session
            .prepare_delegated_editor_publication(
                workbench.session.identity(),
                newer_checkpoint.clone(),
                "Install newer delegated authority under pending token",
            )
            .unwrap();
        let receipt = workbench.session.apply_prepared(prepared).unwrap();
        workbench.materialized = Some(newer_cache);
        workbench.last_receipt = Some(receipt);
        assert_ne!(workbench.session.identity(), &stale_identity);
        assert!(workbench.has_pending_semantic_point_drag(8_416));
        assert_warm_cache_matches_session(&workbench);

        let newer_identity = workbench.session.identity().clone();
        let newer_overlay = workbench.session.snapshot().interaction_overlay.clone();
        let newer_source = workbench.managed_source().to_owned();
        let newer_persistence = workbench.to_persistence_json().unwrap();
        let Err(error) = workbench.publish_pointer_terminal_checkpoint(
            8_416,
            &stale_checkpoint,
            "Reject terminal authenticated by stale code session",
        ) else {
            panic!("a terminal prepared under the older session must reject")
        };
        assert!(error.contains("invalidated by a newer code-session revision"));
        assert!(!workbench.has_pending_semantic_point_drag(8_416));
        assert_eq!(workbench.session.identity(), &newer_identity);
        assert_eq!(workbench.accepted_editor_checkpoint(), &newer_checkpoint);
        assert_eq!(
            workbench.session.snapshot().interaction_overlay,
            newer_overlay,
        );
        assert_eq!(workbench.managed_source(), newer_source);
        assert_eq!(workbench.to_persistence_json().unwrap(), newer_persistence);
        assert_warm_cache_matches_session(&workbench);
        let Err(second_error) = workbench.publish_pointer_terminal_checkpoint(
            8_416,
            &newer_checkpoint,
            "Cannot reuse consumed stale route",
        ) else {
            panic!("the stale terminal token must be consumed after identity rejection")
        };
        assert!(second_error.contains("no pending authenticated route"));
    }

    #[test]
    fn apply_and_undo_invalidate_an_old_semantic_terminal_without_reverting_newer_authority() {
        let (workbench, editor) = CodeProjectWorkbench::new_authored().unwrap();
        let mut workbench = Box::new(workbench);
        let producer_lens = |workbench: &CodeProjectWorkbench,
                             editor: &ProjectionalEditorSession| {
            let expansion = workbench
                .session
                .snapshot()
                .accepted_expansion
                .as_ref()
                .unwrap();
            let lens = expansion
                .writable_points
                .iter()
                .find(|point| {
                    !point.source.is_reference()
                        && expansion.declaration_for_alias(&point.handle.alias)
                            == Some(&SemanticSymbol("frame".into()))
                })
                .unwrap()
                .clone();
            let native = expanded_port_point(editor, &lens.handle).unwrap();
            (lens, native)
        };

        let (_, first_point) = producer_lens(&workbench, &editor);
        assert!(
            workbench
                .prepare_semantic_point_drag(
                    &editor,
                    8_413,
                    first_point,
                    Some(&SemanticSymbol("frame".into())),
                )
                .unwrap()
                .is_none(),
        );
        let stale_checkpoint = encode_editor_checkpoint(&editor).unwrap();
        workbench.set_managed_draft(workbench.managed_source().replacen(
            "upperRight: [60, 35]",
            "upperRight: [61, 35]",
            1,
        ));
        let CodeApplyOutcome::Accepted(applied) = workbench.apply_managed_draft().unwrap() else {
            panic!("valid managed-source edit must apply")
        };
        let applied_identity = workbench.session.identity().clone();
        let applied_checkpoint = workbench.accepted_editor_checkpoint().clone();
        let Err(error) = workbench.publish_pointer_terminal_checkpoint(
            8_413,
            &stale_checkpoint,
            "Stale terminal after Apply",
        ) else {
            panic!("Apply must invalidate the older semantic terminal")
        };
        assert!(error.contains("no pending authenticated route"));
        assert_eq!(workbench.session.identity(), &applied_identity);
        assert_eq!(workbench.accepted_editor_checkpoint(), &applied_checkpoint);

        let (_, second_point) = producer_lens(&workbench, &applied.editor);
        assert!(
            workbench
                .prepare_semantic_point_drag(
                    &applied.editor,
                    8_414,
                    second_point,
                    Some(&SemanticSymbol("frame".into())),
                )
                .unwrap()
                .is_none(),
        );
        let undone = workbench.step_history(true).unwrap().unwrap();
        let undone_identity = workbench.session.identity().clone();
        let undone_checkpoint = workbench.accepted_editor_checkpoint().clone();
        let Err(error) = workbench.publish_pointer_terminal_checkpoint(
            8_414,
            &encode_editor_checkpoint(&applied.editor).unwrap(),
            "Stale terminal after Undo",
        ) else {
            panic!("Undo must invalidate the older semantic terminal")
        };
        assert!(error.contains("no pending authenticated route"));
        assert_eq!(workbench.session.identity(), &undone_identity);
        assert_eq!(workbench.accepted_editor_checkpoint(), &undone_checkpoint);
        assert_eq!(
            encode_editor_checkpoint(&undone.editor).unwrap(),
            undone_checkpoint,
        );
    }
}
