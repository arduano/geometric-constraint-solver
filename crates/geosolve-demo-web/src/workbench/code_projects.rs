// SPDX-License-Identifier: GPL-3.0-or-later
//! Optional managed code-project composition for the demonstration workbench.
//!
//! This module deliberately owns presentation and caller-facing project
//! composition only. Executed managed artifacts, semantic identity and unified
//! code history remain public `geosolve-sketch-code` responsibilities. This
//! Rust adapter neither parses TypeScript nor duplicates solver equations.

#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};

use geosolve_constraint_editor::{
    ComputedEdgeGeometry, ComputedEdgeProvenance, ComputedFeatureDefinition,
    ComputedFeatureDocument, ComputedFeatureEvaluation, ComputedFeatureEvaluationState,
    ComputedFeatureId, ComputedFeatureSnapshot, DelegatedComputedFilletRadiusProposal,
    DelegatedPointDragProposal, IntentInspectorEditTarget, IntentInspectorField,
    IntentInspectorProjection, IntentNativeBinding, IntentWorkbenchProjection,
    NativeCurveSpanSource, ProjectionalEditorSession, SelectionItem,
};
use geosolve_sketch::{
    CurveSpan, DocumentId, DocumentObjectId, DocumentObjectRelabel, OperationControl,
    OperationOutcome, PersistentId, RetainedSketchDocumentSession, SketchHardValidity,
};
use geosolve_sketch_code::{
    BundledCodeProject, CodeGeneratedChildAddress, CodeInteractionOverlay, CodeOwnerAddress,
    CodePointEdit, CodeProject, CodeRectangleCorner, CodeSessionIdentity, CodeSessionReceipt,
    CodeWritableAddress, CompiledManagedSource, EditorBootstrapDeclaration, ExpandedCodeProject,
    ExpandedPort, ExpandedSemanticTarget, ExpandedWritablePoint, GeneratedMemberAddress,
    GeneratedMemberIdentity, KeyedReconcileState, ManagedControl, ManagedControlAccess,
    ManagedControlConsumerTarget, ManagedControlEdit, ManagedControlEditBatch, ManagedControlId,
    ManagedControlManifest, ManagedControlReadOnlyReason, ManagedControlSchema,
    ManagedControlToken, ManagedDeclarationDraft, ManagedDiagnostic, ManagedDiagnosticCode,
    ManagedMutationAuthority, ManagedPathSegment, ManagedSketchMutation, ManagedSpan, ManagedValue,
    MaterializedCodeProject, PatchModuleArtifact, PreparedManagedMutationReceipt,
    PreparedManagedMutationRequest, PreparedManagedSourceRequest, ProjectKey, SemanticOutputPath,
    SemanticSymbol, SketchCodeSession, UnitLiteral, bundled_code_project_demos,
    bundled_code_projects, direct_declaration_intent_symbol,
    expand_code_project_for_structural_edit, managed_control_authority, managed_control_manifest,
    materialize_code_project_cold, materialize_code_project_incremental_for_structural_edit,
    materialize_code_project_incremental_with_overlay,
    materialize_code_project_incremental_with_overlay_and_accepted_continuation_audited,
    prepare_editor_declaration_insertions, prepare_managed_mutation, prepare_managed_source,
    rehydrate_materialized_code_project, required_generated_members,
    validate_prepared_managed_mutation, validate_prepared_managed_source,
};
use geosolve_sketch_features::{
    ComputedEvaluationAllocator, ComputedFeatureEvaluationPolicy, ComputedFeatureEvaluationSnapshot,
};
use geosolve_sketch_intent::{
    GeometryRecipeKind, IntentKey, IntentNode, IntentNodeKind, IntentPortKind, IntentSessionId,
    NodeId,
};
use serde::{Deserialize, Serialize};

const MANAGED_FILE: &str = "sketch.ts";
const CODE_WORKBENCH_WIRE_VERSION: &str = "geosolve-code-workbench-v3";
const CODE_PROJECT_MODEL_SCALE: f64 = 1.0;
const MAX_MANAGED_DRAFT_DIAGNOSTIC_BYTES: usize = 64 * 1024;
static NEXT_CODE_MATERIALIZATION: AtomicU64 = AtomicU64::new(1);

/// One independently accepted replacement for the live projectional canvas.
/// The caller installs `editor` only after the code-session transaction has
/// published the matching opaque checkpoint.
pub(crate) struct AcceptedCodePublication {
    pub(crate) editor: Box<ProjectionalEditorSession>,
    pub(crate) receipt: CodeSessionReceipt,
}

/// One source-neutral compiler request prepared from an exact accepted canvas
/// candidate. The candidate checkpoint remains Rust-owned and is never sent to
/// the browser; only a receipt which cold-materializes to the same native
/// semantics may publish it.
pub(crate) struct PreparedManagedCanvasMutation {
    pub(crate) request: PreparedManagedMutationRequest,
    candidate_editor_checkpoint: Option<serde_json::Value>,
    declaration_label_projections: Vec<PreparedDeclarationLabelProjection>,
    selected_alias: Option<geosolve_sketch_intent::IntentKey>,
}

/// Exact Rust-prepared whole-source compilation request. Unlike a canvas
/// candidate, this carries no GUI-native checkpoint: the compiler result must
/// independently cold-materialize before one accepted project may publish.
pub(crate) struct PreparedManagedSourceApply {
    pub(crate) request: PreparedManagedSourceRequest,
}

pub(crate) enum ResolvedManagedSourceApply {
    Accepted {
        candidate: Box<CodeProjectWorkbench>,
        publication: AcceptedCodePublication,
    },
    RetainedFailure {
        source: String,
        diagnostic: String,
    },
}

/// Rust-only declaration identity retained across source insertion. It admits
/// only the exact GUI-symbol to compiler-emitted-alias label transition for
/// native objects owned by this persistent node.
#[derive(Clone, Debug, Eq, PartialEq)]
struct PreparedDeclarationLabelProjection {
    node: NodeId,
    terminal_symbol: IntentKey,
    declaration: SemanticSymbol,
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
    native_intent: geosolve_sketch_intent::IntentSessionIdentity,
    native_point: geosolve_sketch::DesignPointId,
    point: ExpandedWritablePoint,
    /// Exact declaration selected when a shared semantic lens was chosen.
    /// Source detachment may replace that node, so accepted publication
    /// restores the presentation selection through this stable alias.
    selected_alias: Option<geosolve_sketch_intent::IntentKey>,
    /// Exact native authority from which the authenticated point gesture
    /// began when a referenced consumer first needed local detachment.
    /// Producer gestures use the accepted code checkpoint directly.
    detached_origin_checkpoint: Option<serde_json::Value>,
}

/// Read-only terminal view over an authenticated editor route and its exact
/// independently accepted native preview. The editor contributes stable
/// semantic/ownership bindings; `session` is the sole geometry witness.
#[derive(Clone, Copy)]
struct TerminalPointPreview<'a> {
    editor: &'a ProjectionalEditorSession,
    session: &'a RetainedSketchDocumentSession,
}

impl<'a> TerminalPointPreview<'a> {
    fn from_accepted(editor: &'a ProjectionalEditorSession) -> Result<Self, String> {
        let session = &editor
            .coordinator()
            .accepted_materialization()
            .ok_or_else(|| "terminal code drag has no accepted native authority".to_owned())?
            .session;
        validate_terminal_preview_session(session)?;
        Ok(Self { editor, session })
    }

    fn point(self, handle: &ExpandedPort) -> Option<geosolve_sketch::DesignPointId> {
        expanded_port_point(self.editor, handle)
    }

    fn position(self, handle: &ExpandedPort) -> Option<[f64; 2]> {
        let point = self.point(handle)?;
        let position = self
            .session
            .accepted_state_for_current_input()?
            .document()
            .point(point)?
            .position;
        position.into_iter().all(f64::is_finite).then_some(position)
    }
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

/// The two canonical files shared verbatim with `geosolve-headless`.
///
/// This deliberately contains no editor checkpoint, solved coordinates,
/// interaction overlay, browser layout, or other second authority format.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CanonicalCodeProjectFiles {
    pub(crate) project_json: String,
    pub(crate) managed_source: String,
}

/// One explicitly non-canonical rescue copy of the live editor bytes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RawManagedDraftFile {
    pub(crate) source: String,
}

/// Typed refusal from the browser's canonical project export boundary.
///
/// Dirty and invalid drafts remain available through the explicitly separate
/// raw-source route; they can never be mistaken for accepted project bytes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum CanonicalCodeProjectExportError {
    NoProject,
    DirtyDraft,
    InvalidDraft { diagnostic: ManagedDiagnostic },
    RetainedFailedAuthority { stage: String, diagnostic: String },
    InvalidAcceptedAuthority(String),
}

impl std::fmt::Display for CanonicalCodeProjectExportError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoProject => formatter.write_str("no code project is open"),
            Self::DirtyDraft => formatter.write_str(
                "canonical export is unavailable until the managed-source draft is applied or reverted",
            ),
            Self::InvalidDraft { diagnostic } => write!(
                formatter,
                "canonical export is unavailable while the managed-source draft is invalid at line {}, column {}: {}",
                diagnostic.line, diagnostic.column, diagnostic.message,
            ),
            Self::RetainedFailedAuthority { stage, diagnostic } => write!(
                formatter,
                "canonical export is unavailable while failed {stage} authority is retained: {diagnostic}",
            ),
            Self::InvalidAcceptedAuthority(reason) => {
                write!(formatter, "canonical export authority is invalid: {reason}")
            }
        }
    }
}

/// Typed failure while preparing an imported headless/browser project.
///
/// A caller receives neither half of the replacement pair on error, so the
/// existing workbench/editor can remain byte-identical until a complete
/// candidate has crossed decode, expansion, native materialization, and
/// independent accepted-scene validation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum CanonicalCodeProjectImportError {
    InvalidProject(String),
    Materialization(String),
}

impl std::fmt::Display for CanonicalCodeProjectImportError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidProject(reason) => {
                write!(formatter, "canonical code project is invalid: {reason}")
            }
            Self::Materialization(reason) => write!(
                formatter,
                "canonical code project could not acquire accepted scene authority: {reason}",
            ),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
enum ManagedInspectorPropertyResolution<'a> {
    ModifiableSource(&'a ManagedControl),
    ModifiableInstance,
    Encoded { reason: String },
    Blocked { reason: String },
}

#[derive(Clone, Debug)]
enum ManagedInspectorOwner {
    Generated(CodeGeneratedChildAddress),
    Declaration(SemanticSymbol),
}

#[derive(Clone, Debug)]
struct ManagedInspectorContext<'a> {
    owner: ManagedInspectorOwner,
    controls: Vec<&'a ManagedControl>,
    routes: BTreeMap<SemanticOutputPath, Vec<usize>>,
    families: BTreeSet<String>,
    blocked_reason: Option<String>,
}

struct ManagedInspectorScope {
    owner: ManagedInspectorOwner,
    is_dimension: bool,
}

fn managed_consumer_matches_inspector_owner(
    consumer: &ManagedControlConsumerTarget,
    owner: &ManagedInspectorOwner,
) -> bool {
    match (consumer, owner) {
        (
            ManagedControlConsumerTarget::Generated {
                address, identity, ..
            },
            ManagedInspectorOwner::Generated(child),
        ) => {
            matches!(
                &child.owner.address,
                CodeOwnerAddress::GeneratedMember { address: child_address }
                    if child_address == address
            ) && child.owner.allocation == identity.allocation
                && child.owner.generation == identity.generation
        }
        (
            ManagedControlConsumerTarget::Declaration {
                declaration: candidate,
                ..
            },
            ManagedInspectorOwner::Declaration(declaration),
        ) => candidate == declaration,
        _ => false,
    }
}

fn indexed_managed_inspector_context(
    owner: ManagedInspectorOwner,
    is_dimension: bool,
    manifest: Option<&ManagedControlManifest>,
    blocked_reason: Option<String>,
) -> ManagedInspectorContext<'_> {
    let mut controls = Vec::new();
    let mut routes = BTreeMap::<SemanticOutputPath, Vec<usize>>::new();
    let mut families = BTreeSet::new();
    for control in manifest.into_iter().flat_map(|manifest| &manifest.controls) {
        let mut properties = BTreeSet::new();
        for consumer in &control.consumers {
            if !managed_consumer_matches_inspector_owner(&consumer.target, &owner) {
                continue;
            }
            let family = match &consumer.target {
                ManagedControlConsumerTarget::Declaration { family, .. }
                | ManagedControlConsumerTarget::Generated { family, .. } => family,
            };
            families.insert(family.clone());
            properties.insert(consumer.property.clone());

            // Direct dimension source spells the driving literal `target`,
            // while the central Inspector schema presents it as `value`.
            if is_dimension
                && matches!(
                    &consumer.target,
                    ManagedControlConsumerTarget::Declaration { .. }
                )
                && family.starts_with("dimension.")
                && matches!(
                    control.source.path.0.as_slice(),
                    [ManagedPathSegment::Field(source)] if source == "target"
                )
            {
                properties.insert(SemanticOutputPath(vec![ManagedPathSegment::Field(
                    "value".into(),
                )]));
            }
        }
        if properties.is_empty() {
            continue;
        }
        let index = controls.len();
        controls.push(control);
        for property in properties {
            routes.entry(property).or_default().push(index);
        }
    }
    ManagedInspectorContext {
        owner,
        controls,
        routes,
        families,
        blocked_reason,
    }
}

/// Browser-decoded value for one Code-panel managed control. The stable
/// control ID is the only source coordinate carried by DOM; this value is
/// reinterpreted against a freshly derived manifest before mutation.
pub(crate) enum ManagedControlSubmission {
    Number(f64),
    Boolean(bool),
    String(String),
}

struct ManagedFilletRadiusRoute {
    token: ManagedControlToken,
    value_kind: ManagedFilletRadiusValueKind,
    initiating_alias: geosolve_sketch_intent::IntentKey,
    initiating_feature: ComputedFeatureId,
    features: Vec<ComputedFeatureId>,
    origin_radius: f64,
}

enum ManagedFilletRadiusValueKind {
    Number,
    Unit(String),
}

impl ManagedFilletRadiusValueKind {
    fn replacement(&self, value: f64) -> ManagedValue {
        match self {
            Self::Number => ManagedValue::Number(value),
            Self::Unit(unit) => ManagedValue::Unit(UnitLiteral {
                unit: unit.clone(),
                value,
            }),
        }
    }
}

struct ManagedFilletAliasIndex<'a> {
    declarations: BTreeMap<SemanticSymbol, Vec<&'a geosolve_sketch_intent::IntentKey>>,
    generated: BTreeMap<
        GeneratedMemberAddress,
        BTreeMap<GeneratedMemberIdentity, Vec<&'a geosolve_sketch_intent::IntentKey>>,
    >,
}

impl<'a> ManagedFilletAliasIndex<'a> {
    fn new(expansion: &'a ExpandedCodeProject) -> Self {
        let mut declarations = BTreeMap::<SemanticSymbol, Vec<_>>::new();
        for (alias, declaration) in &expansion.declaration_provenance {
            declarations
                .entry(declaration.clone())
                .or_default()
                .push(alias);
        }

        let mut generated =
            BTreeMap::<GeneratedMemberAddress, BTreeMap<GeneratedMemberIdentity, Vec<_>>>::new();
        for child in &expansion.generated_children {
            let CodeOwnerAddress::GeneratedMember { address } = &child.address.owner.address else {
                continue;
            };
            let identity = GeneratedMemberIdentity {
                allocation: child.address.owner.allocation,
                generation: child.address.owner.generation,
            };
            generated
                .entry(address.clone())
                .or_default()
                .entry(identity)
                .or_default()
                .push(&child.alias);
        }
        Self {
            declarations,
            generated,
        }
    }

    fn aliases(
        &self,
        target: &ManagedControlConsumerTarget,
    ) -> Option<&[&'a geosolve_sketch_intent::IntentKey]> {
        match target {
            ManagedControlConsumerTarget::Declaration { declaration, .. } => {
                self.declarations.get(declaration).map(Vec::as_slice)
            }
            ManagedControlConsumerTarget::Generated {
                address, identity, ..
            } => self
                .generated
                .get(address)
                .and_then(|identities| identities.get(identity))
                .map(Vec::as_slice),
        }
    }
}

/// Read-only source/expansion projection consumed by the DOM-free bridge.
/// Stable managed symbols and generated addresses are presentation identity;
/// opaque Intent aliases remain action tokens owned by this adapter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ManagedDeclarationPanelProjection {
    pub(crate) source_digest: String,
    pub(crate) dirty: bool,
    pub(crate) blocked_reason: Option<String>,
    pub(crate) declarations: Vec<ManagedDeclarationPanelRow>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ManagedDeclarationPanelRow {
    pub(crate) id: String,
    pub(crate) symbol: SemanticSymbol,
    pub(crate) label: String,
    pub(crate) kind: String,
    pub(crate) group: Option<String>,
    pub(crate) source_start: usize,
    pub(crate) source_end: usize,
    pub(crate) selected: bool,
    pub(crate) selection_node: Option<NodeId>,
    pub(crate) suppressed: Option<bool>,
    pub(crate) suppression_control_id: Option<String>,
    pub(crate) closure_role: ManagedDeclarationClosureRole,
    pub(crate) closure_helpers: Vec<Self>,
    pub(crate) generated: Vec<ManagedGeneratedPanelRow>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ManagedDeclarationClosureRole {
    Independent,
    Root,
    Helper { root: SemanticSymbol },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ManagedGeneratedPanelRow {
    pub(crate) id: String,
    pub(crate) address: GeneratedMemberAddress,
    pub(crate) label: String,
    pub(crate) kind: String,
    pub(crate) source_start: usize,
    pub(crate) source_end: usize,
    pub(crate) selected: bool,
    pub(crate) selection_node: Option<NodeId>,
    pub(crate) suppressed: bool,
    pub(crate) suppression_token: Option<String>,
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

// This is an arithmetic-depth budget applied to a semantic coordinate scale,
// not a fixed ULP-distance gate. Rectangle aliases are independently solved
// projections of two canonical seeds, so their last-bit drift is relative to
// the authenticated rectangle's own seeds and aliases rather than unrelated
// document geometry or one (possibly near-zero) coordinate.
const TERMINAL_SEED_ROUNDOFF_ULPS: u64 = 8;
const TERMINAL_SEED_ROUNDOFF_FACTOR: f64 = 8.0;
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
    RectangleLensIndex::new(expansion)?
        .groups
        .get(key)
        .cloned()
        .ok_or_else(|| "rectangle semantic seed group has no complete corner codec".into())
}

struct RectangleLensIndex<'a> {
    groups: BTreeMap<
        (CodeWritableAddress, CodeWritableAddress),
        BTreeMap<CodeRectangleCorner, &'a ExpandedWritablePoint>,
    >,
}

impl<'a> RectangleLensIndex<'a> {
    fn new(expansion: &'a ExpandedCodeProject) -> Result<Self, String> {
        let mut groups = BTreeMap::<_, BTreeMap<_, _>>::new();
        let mut effective = BTreeMap::new();
        let mut ordinary = BTreeSet::new();
        let mut seed_owners = BTreeMap::<CodeWritableAddress, _>::new();

        for point in &expansion.writable_points {
            match &point.edit {
                CodePointEdit::Point { address } => {
                    ordinary.insert(address.clone());
                }
                CodePointEdit::RectangleCorner {
                    lower_left,
                    upper_right,
                    corner,
                    effective_lower_left,
                    effective_upper_right,
                } => {
                    if lower_left == upper_right {
                        return Err(
                            "rectangle semantic seed codec aliases both seed addresses".into()
                        );
                    }
                    let key = (lower_left.clone(), upper_right.clone());
                    for address in [lower_left, upper_right] {
                        if seed_owners
                            .insert(address.clone(), key.clone())
                            .is_some_and(|owner| owner != key)
                        {
                            return Err("rectangle semantic seed groups partially overlap".into());
                        }
                    }
                    let seed_bits = (
                        pair_bits(*effective_lower_left),
                        pair_bits(*effective_upper_right),
                    );
                    if effective
                        .insert(key.clone(), seed_bits)
                        .is_some_and(|expected| expected != seed_bits)
                    {
                        return Err(
                            "rectangle semantic corner codecs disagree on effective seeds".into(),
                        );
                    }
                    if groups
                        .entry(key)
                        .or_default()
                        .insert(*corner, point)
                        .is_some()
                    {
                        return Err(
                            "rectangle semantic seed group has a duplicate corner codec".into()
                        );
                    }
                }
            }
        }

        if ordinary
            .iter()
            .any(|address| seed_owners.contains_key(address))
        {
            return Err(
                "rectangle semantic seed address collides with an ordinary point codec".into(),
            );
        }
        if groups.values().any(|lenses| lenses.len() != 4) {
            return Err("rectangle semantic seed group has no complete corner codec".into());
        }
        Ok(Self { groups })
    }

    fn get(
        &self,
        key: &(CodeWritableAddress, CodeWritableAddress),
    ) -> Result<&BTreeMap<CodeRectangleCorner, &'a ExpandedWritablePoint>, String> {
        self.groups
            .get(key)
            .ok_or_else(|| "rectangle semantic seed group has no complete corner codec".into())
    }
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

fn point_seed_roundoff_compatible(left: [f64; 2], right: [f64; 2], model_scale: f64) -> bool {
    left.into_iter()
        .zip(right)
        .all(|(left, right)| scalar_seed_roundoff_compatible(left, right, model_scale))
}

fn semantic_roundoff_tolerance(left: f64, right: f64, model_scale: f64) -> Option<f64> {
    if !left.is_finite() || !right.is_finite() || !model_scale.is_finite() || model_scale <= 0.0 {
        return None;
    }
    Some(
        left.abs().max(right.abs()).max(model_scale) * f64::EPSILON * TERMINAL_SEED_ROUNDOFF_FACTOR,
    )
}

fn semantic_local_coordinate_scale(
    model_scales: impl IntoIterator<Item = f64>,
    positions: impl IntoIterator<Item = [f64; 2]>,
) -> Option<f64> {
    let mut scale = 1.0_f64;
    for model_scale in model_scales {
        if !model_scale.is_finite() || model_scale <= 0.0 {
            return None;
        }
        scale = scale.max(model_scale);
    }
    for coordinate in positions.into_iter().flatten() {
        if !coordinate.is_finite() {
            return None;
        }
        scale = scale.max(coordinate.abs());
    }
    Some(scale)
}

fn semantic_point_coordinate_scale(
    documents: [&geosolve_sketch::SketchDocument; 2],
    points: &BTreeSet<geosolve_sketch::DesignPointId>,
) -> Option<f64> {
    if points.is_empty() {
        return None;
    }
    let model_scales = documents.map(geosolve_sketch::SketchDocument::model_scale);
    let positions = documents.into_iter().flat_map(|document| {
        points
            .iter()
            .map(|point| document.point(*point).map(|point| point.position))
    });
    let positions = positions.collect::<Option<Vec<_>>>()?;
    semantic_local_coordinate_scale(model_scales, positions)
}

fn accepted_validation_is_publishable(
    validation: &geosolve_constraint_editor::IntentValidationEvidence,
) -> bool {
    validation.hard_residuals_validated
        && validation.all_active_features_current
        && validation
            .maximum_normalized_hard_residual
            .is_none_or(|residual| residual.is_finite() && residual <= 1.0e-9)
}

fn validate_terminal_preview_session(
    session: &RetainedSketchDocumentSession,
) -> Result<(), String> {
    let accepted = session
        .accepted_state_for_current_input()
        .ok_or_else(|| "terminal point preview has no current accepted native state".to_owned())?;
    let solve = accepted
        .diagnostics()
        .solve
        .ok_or_else(|| "terminal point preview has no independent solve evidence".to_owned())?;
    if !solve.accepted
        || solve.hard_validity != SketchHardValidity::Valid
        || !solve.hard_residuals_validated
        || solve
            .maximum_normalized_hard_residual
            .is_some_and(|residual| !residual.is_finite() || residual > 1.0e-9)
    {
        return Err(
            "terminal point preview failed independent native hard-residual validation".into(),
        );
    }
    Ok(())
}

fn scalar_seed_roundoff_compatible(left: f64, right: f64, model_scale: f64) -> bool {
    let Some(tolerance) = semantic_roundoff_tolerance(left, right, model_scale) else {
        return false;
    };
    let left = left.to_bits();
    let right = right.to_bits();
    if left == right {
        return true;
    }
    if left & !F64_SIGN_MASK == 0 && right & !F64_SIGN_MASK == 0 {
        return false;
    }
    (left & F64_SIGN_MASK == right & F64_SIGN_MASK
        && (f64::from_bits(left) - f64::from_bits(right)).abs() <= tolerance)
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
    // Exact-session immutable presentation authority. Dirty text is excluded;
    // clean and retained-failure sessions receive distinct keyed entries.
    // Mutations still derive a fresh borrow-scoped `ManagedControlAuthority`
    // for exact-CAS application.
    managed_control_manifest_cache: RefCell<Option<ManagedControlManifestCache>>,
    // A referenced consumer may need to be detached before native pointer
    // continuation starts. This is disposable gesture state and is never
    // serialized or entered into history unless the terminal sample commits.
    pending_semantic_point_drag: Option<PendingSemanticPointDrag>,
    // Last authenticated semantic lens retained solely for the memory-only
    // interaction trace. Publication consumes the pending token before a
    // later parity rejection can restore accepted authority, so diagnostics
    // need this non-authoritative handle to report the post-restore point.
    trace_semantic_point: Option<(u64, ExpandedWritablePoint)>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ManagedControlManifestCacheKey {
    session: CodeSessionIdentity,
    project: ProjectKey,
    source_digest: String,
    expansion_digest: String,
}

struct ManagedControlManifestCache {
    key: ManagedControlManifestCacheKey,
    manifest: Rc<ManagedControlManifest>,
}

/// Presentation provenance for one genuine code project. Bundled projects
/// retain their curated sample identity; standalone projects do not invent a
/// sample key.
#[derive(Clone, Debug)]
enum CodeProjectOrigin {
    Bundled(BundledCodeProject),
    Authored,
}

impl CodeProjectOrigin {
    fn title(&self) -> &'static str {
        match self {
            Self::Bundled(project) => project.title(),
            Self::Authored => "Untitled code sketch",
        }
    }

    fn demo_key(&self) -> Option<&'static str> {
        match self {
            Self::Bundled(project) => Some(project.key()),
            Self::Authored => None,
        }
    }

    fn to_wire(&self) -> CodeProjectOriginWire {
        match self {
            Self::Bundled(project) => CodeProjectOriginWire::Bundled {
                demo: project.key().into(),
            },
            Self::Authored => CodeProjectOriginWire::Authored,
        }
    }
}

#[derive(Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum CodeProjectOriginWire {
    Bundled { demo: String },
    Authored,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct CodeProjectWorkbenchWire {
    version: String,
    origin: CodeProjectOriginWire,
    project: String,
    session: String,
    selected_file: String,
    managed_draft: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    draft_diagnostic: Option<ManagedDiagnostic>,
}

fn restore_code_project_origin(origin: CodeProjectOriginWire) -> Result<CodeProjectOrigin, String> {
    let bundled = |key: &str| {
        bundled_code_projects()
            .into_iter()
            .find(|project| project.key() == key)
            .map(CodeProjectOrigin::Bundled)
            .ok_or_else(|| format!("unknown code project `{key}`"))
    };
    match origin {
        CodeProjectOriginWire::Bundled { demo } => bundled(&demo),
        CodeProjectOriginWire::Authored => Ok(CodeProjectOrigin::Authored),
    }
}

fn trace_pair(position: [f64; 2]) -> String {
    format!(
        "[{:.17e}/0x{:016x},{:.17e}/0x{:016x}]",
        position[0],
        position[0].to_bits(),
        position[1],
        position[1].to_bits(),
    )
}

/// Prepares the exact accepted project/source pair for browser download.
///
/// Taking an `Option` at this adapter boundary makes an ordinary non-code
/// workspace a typed refusal rather than an empty or stale download. Draft
/// validity is rederived from the current bytes; browser diagnostic markup is
/// never trusted as export authority.
pub(crate) fn canonical_code_project_files(
    code_project: Option<&CodeProjectWorkbench>,
) -> Result<CanonicalCodeProjectFiles, CanonicalCodeProjectExportError> {
    let code_project = code_project.ok_or(CanonicalCodeProjectExportError::NoProject)?;
    if let Some(diagnostic) = code_project.draft_diagnostic.clone() {
        return Err(CanonicalCodeProjectExportError::InvalidDraft { diagnostic });
    }
    if code_project.is_dirty() {
        // A draft is categorically non-canonical until a compiler host has
        // returned a complete envelope and Rust has accepted it.
        return Err(CanonicalCodeProjectExportError::DirtyDraft);
    }
    let compiled = code_project
        .project
        .managed
        .compiled
        .as_deref()
        .ok_or_else(|| {
            CanonicalCodeProjectExportError::InvalidAcceptedAuthority(
                "managed project lacks compiled authority".into(),
            )
        })?;
    compiled.validate().map_err(|error| {
        CanonicalCodeProjectExportError::InvalidAcceptedAuthority(error.to_string())
    })?;
    if compiled.normalized_source != code_project.managed_draft {
        return Err(CanonicalCodeProjectExportError::InvalidAcceptedAuthority(
            "managed draft differs from its accepted normalized source".into(),
        ));
    }

    let snapshot = code_project.session.snapshot();
    if let Some(failure) = &snapshot.failure {
        return Err(CanonicalCodeProjectExportError::RetainedFailedAuthority {
            stage: failure.stage.clone(),
            diagnostic: failure.diagnostic.clone(),
        });
    }
    let accepted = snapshot.accepted_code_project.as_ref().ok_or_else(|| {
        CanonicalCodeProjectExportError::InvalidAcceptedAuthority(
            "the code session has no accepted project".into(),
        )
    })?;
    if snapshot.code_project.as_ref() != Some(&code_project.project)
        || accepted != &code_project.project
        || snapshot.managed != accepted.managed
        || snapshot.accepted_source_digest != accepted.managed.source_digest
        || snapshot.accepted_expansion.is_none()
        || snapshot.accepted_generated.is_none()
    {
        return Err(CanonicalCodeProjectExportError::InvalidAcceptedAuthority(
            "current project, source, expansion, generated ledger, and accepted authority disagree"
                .into(),
        ));
    }
    let project_json = accepted.to_canonical_json().map_err(|error| {
        CanonicalCodeProjectExportError::InvalidAcceptedAuthority(error.to_string())
    })?;
    Ok(CanonicalCodeProjectFiles {
        project_json,
        managed_source: accepted.managed.source.clone(),
    })
}

impl CodeProjectWorkbench {
    /// Returns only the pinned data-only patch plans needed by the managed
    /// recorder. This is deliberately an on-demand handoff: ordinary pointer,
    /// camera, and snapshot traffic must not retransmit potentially large
    /// compiler context.
    pub(crate) fn managed_compiler_patches(
        &self,
    ) -> Result<BTreeMap<String, serde_json::Value>, String> {
        self.project.validate().map_err(|error| error.to_string())?;
        let mut patches = BTreeMap::new();
        for import in &self.project.managed.imports {
            for binding in &import.bindings {
                let matches = self
                    .project
                    .artifacts
                    .values()
                    .filter_map(|value| {
                        let artifact =
                            serde_json::from_value::<PatchModuleArtifact>(value.clone()).ok()?;
                        (artifact.module_specifier == import.module
                            && artifact.export_name == *binding)
                            .then_some(value.clone())
                    })
                    .collect::<Vec<_>>();
                match matches.as_slice() {
                    [] => {}
                    [artifact] => {
                        if patches.insert(binding.clone(), artifact.clone()).is_some() {
                            return Err(format!(
                                "managed compiler patch binding `{binding}` is ambiguous"
                            ));
                        }
                    }
                    _ => {
                        return Err(format!(
                            "managed compiler patch binding `{binding}` resolves more than once"
                        ));
                    }
                }
            }
        }
        Ok(patches)
    }

    fn managed_mutation_authority(
        &self,
    ) -> Result<(ManagedMutationAuthority, &CompiledManagedSource), String> {
        if self.is_dirty() {
            return Err(
                "Apply or Revert the managed-source draft before a structured source edit".into(),
            );
        }
        self.accepted_managed_mutation_authority()
    }

    fn accepted_managed_mutation_authority(
        &self,
    ) -> Result<(ManagedMutationAuthority, &CompiledManagedSource), String> {
        if self.session.snapshot().failure.is_some() {
            return Err(
                "resolve or Undo the retained code failure before a structured source edit".into(),
            );
        }
        let compiled = self
            .project
            .managed
            .compiled
            .as_deref()
            .ok_or_else(|| "this project has no compiled managed authority".to_owned())?;
        let expansion = self
            .session
            .snapshot()
            .accepted_expansion
            .as_ref()
            .ok_or_else(|| "code project has no accepted expansion authority".to_owned())?;
        let authority = ManagedMutationAuthority::new(
            self.project.project.clone(),
            self.session.identity().clone(),
            expansion.digest.clone(),
            self.project.managed.declaration_name_high_water,
            compiled,
        )
        .map_err(|error| error.to_string())?;
        Ok((authority, compiled))
    }

    /// Converts all and only the declarations added by one already accepted
    /// canvas gesture into a digest-bound managed mutation request.
    ///
    /// This method does not change the code session, source, outer history or
    /// accepted editor. The caller must immediately restore the accepted code
    /// checkpoint while the compiler host resolves the returned request.
    pub(crate) fn prepare_canvas_managed_mutation(
        &self,
        candidate_editor: &ProjectionalEditorSession,
    ) -> Result<PreparedManagedCanvasMutation, String> {
        let (authority, compiled) = self.managed_mutation_authority()?;
        let accepted_editor = self.restore_accepted_editor()?;
        let expansion = self
            .session
            .snapshot()
            .accepted_expansion
            .as_ref()
            .ok_or_else(|| "code project has no accepted expansion authority".to_owned())?;
        let accepted_nodes = accepted_editor.coordinator().intent().graph().nodes();
        let candidate_nodes = candidate_editor.coordinator().intent().graph().nodes();
        let added = candidate_nodes
            .values()
            .filter(|node| !accepted_nodes.contains_key(&node.id))
            .collect::<Vec<_>>();
        if added.is_empty() {
            return Err("the completed canvas gesture added no source declaration".into());
        }
        if added.len() > geosolve_sketch_code::MANAGED_MUTATION_BATCH_LIMIT {
            return Err(format!(
                "the completed canvas gesture added {} declarations; the managed batch limit is {}",
                added.len(),
                geosolve_sketch_code::MANAGED_MUTATION_BATCH_LIMIT,
            ));
        }

        let current_high_water = self.project.managed.declaration_name_high_water;
        let (declarations, candidate_high_water) =
            allocate_canvas_declaration_names(&self.project, &added, current_high_water)?;
        let declaration_label_projections =
            canvas_declaration_label_projections(candidate_editor, &declarations)?;
        let insertion = prepare_editor_declaration_insertions(
            &self.project,
            expansion,
            &accepted_editor,
            candidate_editor,
            &declarations,
        )
        .map_err(|error| error.to_string())?;
        if insertion.project != self.project.project
            || insertion.source_digest != authority.accepted_source_digest
            || insertion.expansion_digest != authority.accepted_expansion_digest
        {
            return Err(
                "canvas reverse projection returned stale project, source or expansion authority"
                    .into(),
            );
        }
        let mutation = ManagedSketchMutation::InsertDeclarations {
            declarations: insertion
                .declarations
                .into_iter()
                .map(ManagedDeclarationDraft::from)
                .collect(),
        };
        let request =
            prepare_managed_mutation(&authority, compiled, mutation, candidate_high_water)
                .map_err(|error| error.to_string())?;
        let candidate_editor_checkpoint = encode_editor_checkpoint(candidate_editor)?;
        Ok(PreparedManagedCanvasMutation {
            request,
            candidate_editor_checkpoint: Some(candidate_editor_checkpoint),
            declaration_label_projections,
            selected_alias: None,
        })
    }

    /// Prepares a panel-owned reorder, suppression or deletion against the
    /// same accepted source/IR/artifact identity used by canvas insertions.
    /// No native or source authority changes until its compiler receipt has
    /// passed [`Self::resolve_canvas_managed_mutation`].
    pub(crate) fn prepare_structured_managed_mutation(
        &self,
        mutation: ManagedSketchMutation,
    ) -> Result<PreparedManagedCanvasMutation, String> {
        let (authority, compiled) = self.managed_mutation_authority()?;
        let request = prepare_managed_mutation(
            &authority,
            compiled,
            mutation,
            authority.declaration_name_high_water,
        )
        .map_err(|error| error.to_string())?;
        Ok(PreparedManagedCanvasMutation {
            request,
            candidate_editor_checkpoint: None,
            declaration_label_projections: Vec::new(),
            selected_alias: None,
        })
    }

    /// Prepares one exact raw-source replacement against the complete current
    /// accepted compiler/session/expansion authority. The draft remains
    /// presentation-only until the browser or Deno compiler returns a receipt
    /// and Rust validates and materializes it.
    pub(crate) fn prepare_managed_source_apply(
        &self,
    ) -> Result<PreparedManagedSourceApply, String> {
        if !self.is_dirty() {
            return Err("the managed-source draft is unchanged".into());
        }
        let (authority, compiled) = self.accepted_managed_mutation_authority()?;
        let request = prepare_managed_source(&authority, compiled, self.managed_draft.clone())
            .map_err(|error| error.to_string())?;
        Ok(PreparedManagedSourceApply { request })
    }

    /// Prepares a structured source edit while retaining a currently selected
    /// managed declaration through cold rematerialization. The alias is read
    /// only after the accepted expansion has authenticated its source owner;
    /// ordinary GUI-owned selection therefore remains intentionally unclaimed.
    pub(crate) fn prepare_selected_structured_managed_mutation(
        &self,
        editor: &ProjectionalEditorSession,
        mutation: ManagedSketchMutation,
    ) -> Result<PreparedManagedCanvasMutation, String> {
        let selected_alias = match self.selected_managed_declaration(editor)? {
            Some(_) => {
                let node = editor
                    .selected_declaration()
                    .and_then(|node| editor.coordinator().intent().graph().node(node))
                    .ok_or_else(|| {
                        "the selected managed declaration disappeared before mutation preparation"
                            .to_owned()
                    })?;
                Some(node.symbol.clone())
            }
            None => None,
        };
        let mut prepared = self.prepare_structured_managed_mutation(mutation)?;
        prepared.selected_alias = selected_alias;
        Ok(prepared)
    }

    /// Publishes one already accepted delegated point release as persistent,
    /// equation-free instance placement. This route never edits or compiles
    /// managed source: the accepted native continuation authenticates the
    /// complete solver-coupled terminal before one outer history row commits.
    pub(crate) fn publish_delegated_point_terminal(
        &mut self,
        pointer_id: u64,
        origin_editor: &ProjectionalEditorSession,
        proposal: &DelegatedPointDragProposal,
        label: &str,
    ) -> Result<Option<AcceptedCodePublication>, String> {
        if !self.has_managed_authority() {
            return Err("delegated source point preparation requires managed authority".into());
        }
        let pending = self.pending_semantic_point_drag.as_ref().ok_or_else(|| {
            "delegated point terminal has no pending authenticated route".to_owned()
        })?;
        if pending.pointer_id != pointer_id || proposal.pointer_id != pointer_id {
            return Err("terminal pointer does not own the pending semantic point gesture".into());
        }
        if &pending.session != self.session.identity() {
            self.pending_semantic_point_drag = None;
            return Err(
                "semantic point gesture was invalidated by a newer code-session revision".into(),
            );
        }
        if pending.native_intent != proposal.intent
            || origin_editor.coordinator().intent().identity() != proposal.intent
            || pending.native_point != proposal.point
        {
            return Err(
                "delegated point terminal does not match its authenticated native route".into(),
            );
        }
        let preview = TerminalPointPreview {
            editor: origin_editor,
            session: proposal.terminal_session(),
        };
        validate_terminal_preview_session(preview.session)?;
        let point = pending.point.clone();
        let selected_alias = pending.selected_alias.clone();
        let target = preview.position(&point.handle).ok_or_else(|| {
            "delegated point terminal has no accepted semantic lens position".to_owned()
        })?;
        if pair_bits(target) != pair_bits(proposal.accepted_position) {
            return Err(
                "delegated point terminal disagrees with its accepted native position".into(),
            );
        }
        let bundle =
            self.canonical_terminal_point_bundle(&point, vec![(point.clone(), target)], preview)?;
        let publication = self.publish_semantic_point_preview_overlay(
            &bundle.placements,
            preview,
            &bundle.rectangle_projections,
            selected_alias.as_ref(),
            label,
        )?;
        if publication.is_some() {
            self.pending_semantic_point_drag
                .take()
                .expect("the authenticated delegated point route was present");
        }
        Ok(publication)
    }

    /// Authenticates one delegated computed-Fillet terminal against the fresh
    /// executed control manifest, then prepares the same source mutation used
    /// by Typed Panel edits.
    pub(crate) fn prepare_delegated_fillet_managed_mutation(
        &self,
        editor: &ProjectionalEditorSession,
        proposal: &DelegatedComputedFilletRadiusProposal,
    ) -> Result<PreparedManagedCanvasMutation, String> {
        if !self.has_managed_authority() {
            return Err("delegated Fillet source preparation requires managed authority".into());
        }
        let expansion = self
            .session
            .snapshot()
            .accepted_expansion
            .as_ref()
            .ok_or_else(|| "code project has no accepted expansion authority".to_owned())?;
        let authority = managed_control_authority(&self.project, expansion)
            .map_err(|error| error.to_string())?;
        let manifest = authority.manifest();
        let route = self
            .managed_fillet_radius_route_with_manifest(editor, manifest)?
            .ok_or_else(|| {
                "delegated Fillet radius no longer has managed-source authority".to_owned()
            })?;
        let accepted = editor
            .coordinator()
            .accepted_materialization()
            .ok_or_else(|| "delegated Fillet radius has no accepted native authority".to_owned())?;
        if proposal.intent != editor.coordinator().intent().identity()
            || proposal.expected != accepted.computed.input()
            || proposal.initiating_feature != route.initiating_feature
            || proposal.features != route.features
            || proposal.origin_radius.to_bits() != route.origin_radius.to_bits()
        {
            return Err(
                "delegated Fillet radius proposal no longer matches its complete accepted source group"
                    .into(),
            );
        }
        if !proposal.proposed_radius.is_finite() {
            return Err("delegated Fillet radius proposal must be finite".into());
        }
        let replacement = route.value_kind.replacement(proposal.proposed_radius);
        let control = manifest
            .control(&route.token.id)
            .filter(|control| control.token() == Some(&route.token))
            .ok_or_else(|| "delegated Fillet control token is stale".to_owned())?;
        let mutation = authority
            .prepare_mutation(&ManagedControlEditBatch::new([ManagedControlEdit {
                token: control
                    .token()
                    .expect("authenticated delegated control is editable")
                    .clone(),
                value: replacement,
            }]))
            .map_err(|error| error.to_string())?;
        let mut prepared = self.prepare_structured_managed_mutation(mutation)?;
        prepared.selected_alias = Some(route.initiating_alias);
        Ok(prepared)
    }

    /// Validates a browser/Deno receipt, cold-materializes it on an isolated
    /// workbench fork, compares it with the Rust-owned canvas candidate and
    /// returns a complete replacement. The receiver is untouched on error.
    pub(crate) fn resolve_canvas_managed_mutation(
        &self,
        prepared: &PreparedManagedCanvasMutation,
        receipt: PreparedManagedMutationReceipt,
    ) -> Result<(Self, AcceptedCodePublication), String> {
        let (live, _) = self.managed_mutation_authority()?;
        let validated = validate_prepared_managed_mutation(&live, &prepared.request, receipt)
            .map_err(|error| error.to_string())?;
        let candidate_high_water = validated.declaration_name_high_water();
        let compiled = validated.into_compiled();

        // Reconstructing through the bounded persistence wire gives this
        // candidate an independent session/history/cache. No mutation below
        // can leak into the live workbench before every native gate passes.
        let mut candidate = Self::from_persistence_json(&self.to_persistence_json()?)?;
        candidate
            .managed_draft
            .clone_from(&compiled.normalized_source);
        let outcome =
            candidate.apply_managed_compilation_with_high_water(compiled, candidate_high_water)?;
        let mut publication = match outcome {
            CodeApplyOutcome::Accepted(publication) => publication,
            CodeApplyOutcome::RetainedFailure { diagnostic, .. } => {
                return Err(format!(
                    "managed canvas candidate failed cold native materialization: {diagnostic}"
                ));
            }
        };
        if let Some(checkpoint) = &prepared.candidate_editor_checkpoint {
            let terminal = restore_editor_checkpoint(checkpoint)?;
            let expansion = candidate
                .session
                .snapshot()
                .accepted_expansion
                .as_ref()
                .ok_or_else(|| "resolved canvas mutation has no accepted expansion".to_owned())?;
            validate_terminal_native_parity_with_trace(
                &terminal,
                &publication.editor,
                expansion,
                &[],
                &prepared.declaration_label_projections,
                None,
            )?;
        }
        if let Some(alias) = &prepared.selected_alias {
            let node = publication
                .editor
                .coordinator()
                .intent()
                .graph()
                .node_by_symbol(alias)
                .ok_or_else(|| {
                    "resolved managed mutation lost its selected declaration".to_owned()
                })?
                .id;
            if !publication.editor.set_selected_declaration(Some(node)) {
                return Err("resolved managed mutation could not restore selection".into());
            }
        }
        Ok((candidate, publication))
    }

    /// Authenticates and independently materializes a compiler receipt for
    /// one exact Rust-prepared raw-source replacement. The receiver is never
    /// mutated. A native rejection returns only the candidate draft and
    /// diagnostic, leaving accepted source/IR/artifact/scene/history with the
    /// caller.
    pub(crate) fn resolve_managed_source_apply(
        &self,
        prepared: &PreparedManagedSourceApply,
        receipt: PreparedManagedMutationReceipt,
    ) -> Result<ResolvedManagedSourceApply, String> {
        let (live, _) = self.accepted_managed_mutation_authority()?;
        let validated = validate_prepared_managed_source(&live, &prepared.request, receipt)
            .map_err(|error| error.to_string())?;
        let candidate_high_water = validated.declaration_name_high_water();
        let compiled = validated.into_compiled();

        // Work on a persistence-round-tripped fork so rejected compilation or
        // native validation cannot change live history, caches or accepted
        // authority. `apply_managed_compilation_with_high_water` performs the
        // ordinary structural solve and independent validation gates.
        let mut candidate = Self::from_persistence_json(&self.to_persistence_json()?)?;
        candidate
            .managed_draft
            .clone_from(&prepared.request.candidate_source);
        match candidate.apply_managed_compilation_with_high_water(compiled, candidate_high_water)? {
            CodeApplyOutcome::Accepted(publication) => Ok(ResolvedManagedSourceApply::Accepted {
                candidate: Box::new(candidate),
                publication,
            }),
            CodeApplyOutcome::RetainedFailure { diagnostic, .. } => {
                Ok(ResolvedManagedSourceApply::RetainedFailure {
                    source: prepared.request.candidate_source.clone(),
                    diagnostic,
                })
            }
        }
    }

    /// Clears the prior gesture's diagnostic-only semantic lens before one
    /// new browser pointer-down starts a fresh interaction trace.
    pub(crate) fn reset_interaction_trace_gesture(&mut self) {
        self.trace_semantic_point = None;
    }

    /// Compact current authority identity for the memory-only interaction
    /// trace. This deliberately excludes managed source and editor snapshots.
    pub(crate) fn interaction_trace_context(&self) -> String {
        let identity = self.session.identity();
        let pending = self.pending_semantic_point_drag.as_ref();
        format!(
            "project={:?} origin={} session={} revision={} digest={} pending_pointer={} pending_alias={} pending_selector={:?}",
            self.session.snapshot().project,
            self.origin.demo_key().unwrap_or("non-bundled"),
            identity.session,
            identity.revision,
            identity.digest,
            pending.map_or_else(|| "none".into(), |pending| pending.pointer_id.to_string(),),
            pending.map_or("none", |pending| pending.point.handle.alias.as_str()),
            pending.map(|pending| &pending.point.handle.selector),
        )
    }

    /// Exact current native point owned by the pending semantic route, for
    /// before/preview/terminal/publication trace checkpoints.
    pub(crate) fn interaction_trace_pending_point(
        &self,
        editor: &ProjectionalEditorSession,
    ) -> String {
        let semantic_point = self
            .pending_semantic_point_drag
            .as_ref()
            .map(|pending| (pending.pointer_id, &pending.point, "pending"))
            .or_else(|| {
                self.trace_semantic_point
                    .as_ref()
                    .map(|(pointer, point)| (*pointer, point, "consumed"))
            });
        let Some((pointer_id, point, token)) = semantic_point else {
            return "pending=none".into();
        };
        let native = expanded_port_point(editor, &point.handle);
        let position = native.and_then(|point| {
            editor
                .coordinator()
                .presentation_session()
                .and_then(|session| session.accepted_state_for_current_input())
                .and_then(|accepted| accepted.document().point(point))
                .map(|point| point.position)
        });
        format!(
            "token={token} pointer={pointer_id} alias={} selector={:?} native={native:?} position={}",
            point.handle.alias,
            point.handle.selector,
            position.map_or_else(|| "none".into(), trace_pair),
        )
    }

    /// Starts one standalone code-authored sketch from the checked-in empty
    /// executed envelope, without manufacturing an ordinary GUI scene.
    pub(crate) fn new_authored() -> Result<(Self, Box<ProjectionalEditorSession>), String> {
        let project = CodeProject::empty(ProjectKey("code-authored-sketch".into()))
            .map_err(|error| error.to_string())?;
        Self::open_project(CodeProjectOrigin::Authored, project)
    }

    pub(crate) fn open_key(key: &str) -> Result<(Self, Box<ProjectionalEditorSession>), String> {
        let bundled = bundled_code_projects()
            .into_iter()
            .find(|project| project.key() == key)
            .ok_or_else(|| format!("unknown code project `{key}`"))?;
        let project = bundled.project();
        Self::open_project(CodeProjectOrigin::Bundled(bundled), project)
    }

    #[cfg(test)]
    pub(crate) fn open_managed_test_compiled(
        project_key: &str,
        compiled: CompiledManagedSource,
    ) -> Result<(Self, Box<ProjectionalEditorSession>), String> {
        let project = CodeProject::managed(ProjectKey(project_key.into()), compiled)
            .map_err(|error| error.to_string())?;
        Self::open_project(CodeProjectOrigin::Authored, project)
    }

    #[cfg(test)]
    pub(crate) fn open_managed_test_compiled_with_high_water(
        project_key: &str,
        compiled: CompiledManagedSource,
        declaration_name_high_water: u64,
    ) -> Result<(Self, Box<ProjectionalEditorSession>), String> {
        let mut project = CodeProject::managed(ProjectKey(project_key.into()), compiled)
            .map_err(|error| error.to_string())?;
        project.managed.declaration_name_high_water = declaration_name_high_water;
        project.validate().map_err(|error| error.to_string())?;
        Self::open_project(CodeProjectOrigin::Authored, project)
    }

    #[cfg(test)]
    pub(crate) fn open_managed_test_compiled_with_demo_pins(
        project_key: &str,
        demo_key: &str,
        compiled: CompiledManagedSource,
    ) -> Result<(Self, Box<ProjectionalEditorSession>), String> {
        let mut project = bundled_code_project_demos()
            .into_iter()
            .find(|demo| demo.id.key() == demo_key)
            .ok_or_else(|| format!("unknown code project `{demo_key}`"))?
            .project();
        project.project = ProjectKey(project_key.into());
        project.managed = compiled
            .into_managed_document()
            .map_err(|error| error.to_string())?;
        Self::open_project(CodeProjectOrigin::Authored, project)
    }

    #[cfg(test)]
    pub(crate) fn managed_test_overlay_is_empty(&self) -> bool {
        let overlay = &self.session.snapshot().interaction_overlay;
        overlay.drafts().is_empty() && overlay.suppressed_children().is_empty()
    }

    /// Builds a complete imported replacement without touching a live
    /// workbench. Only the returned pair may be swapped into browser state.
    pub(crate) fn import_canonical_project_json(
        json: &str,
    ) -> Result<(Self, Box<ProjectionalEditorSession>), CanonicalCodeProjectImportError> {
        let project = CodeProject::from_json(json)
            .map_err(|error| CanonicalCodeProjectImportError::InvalidProject(error.to_string()))?;
        Self::open_project(CodeProjectOrigin::Authored, project)
            .map_err(CanonicalCodeProjectImportError::Materialization)
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
        let materialized = rehydrate_restored_editor(&delegated_editor, expansion.clone())?;
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
                // The live editor and reconstructible cache must share the
                // restored checkpoint's authenticated Intent identity. Fork
                // that already-restored authority without a second decode.
                materialized: Some(materialized),
                managed_control_manifest_cache: RefCell::new(None),
                pending_semantic_point_drag: None,
                trace_semantic_point: None,
            },
            delegated_editor,
        ))
    }

    pub(crate) fn to_persistence_json(&self) -> Result<String, String> {
        validate_managed_draft_bound(&self.managed_draft)?;
        let wire = CodeProjectWorkbenchWire {
            version: CODE_WORKBENCH_WIRE_VERSION.into(),
            origin: self.origin.to_wire(),
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
            draft_diagnostic: self.draft_diagnostic.clone(),
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
        let origin = restore_code_project_origin(wire.origin)?;
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
        let draft_diagnostic = restore_managed_draft_diagnostic(
            &wire.managed_draft,
            &project.managed.source,
            wire.draft_diagnostic,
        )?;
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
            managed_control_manifest_cache: RefCell::new(None),
            pending_semantic_point_drag: None,
            trace_semantic_point: None,
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
        preview: TerminalPointPreview<'_>,
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
        let candidate_document = preview
            .session
            .accepted_state_for_current_input()
            .ok_or_else(|| "terminal rectangle has no current accepted document".to_owned())?
            .document();
        let rectangle_lenses = RectangleLensIndex::new(expansion)?;
        for (key, group) in rectangle_groups {
            let lenses = rectangle_lenses.get(&key)?;
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
            let first_target = preview.position(&first.handle).ok_or_else(|| {
                "canonical rectangle lens has no accepted native position".to_owned()
            })?;
            let second_target = preview.position(&second.handle).ok_or_else(|| {
                "canonical rectangle lens has no accepted native position".to_owned()
            })?;
            let (lower_left, upper_right) = canonical_rectangle_seeds(
                first_corner,
                first_target,
                second_corner,
                second_target,
            )?;
            let lens_positions = lenses
                .values()
                .map(|point| {
                    preview.position(&point.handle).ok_or_else(|| {
                        "rectangle parity lens has no accepted native position".to_owned()
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
            let coordinate_scale = semantic_local_coordinate_scale(
                [candidate_document.model_scale()],
                [lower_left, upper_right].into_iter().chain(lens_positions),
            )
            .ok_or_else(|| {
                "terminal rectangle has no finite local semantic coordinate scale".to_owned()
            })?;

            // Authenticate every one of the exact four GUI lenses against the
            // candidate's independently accepted native document. The two
            // diagonal anchors above are the only source seeds; adjacent
            // corners are solver-derived projections and may differ by
            // scale-relative machine roundoff, but not materially.
            for (corner, point) in lenses {
                let instance_position = preview.position(&point.handle).ok_or_else(|| {
                    "rectangle parity lens has no accepted native position".to_owned()
                })?;
                let native_point = preview.point(&point.handle).ok_or_else(|| {
                    "rectangle parity lens has no accepted native point binding".to_owned()
                })?;
                let native_position = candidate_document
                    .point(native_point)
                    .ok_or_else(|| "rectangle parity native point disappeared".to_owned())?
                    .position;
                if pair_bits(instance_position) != pair_bits(native_position) {
                    return Err(format!(
                        "rectangle `{}` terminal lens is not authenticated by accepted native authority",
                        key.0.display_path(),
                    ));
                }
                let expected = rectangle_corner_position(lower_left, upper_right, *corner);
                if !point_seed_roundoff_compatible(native_position, expected, coordinate_scale) {
                    return Err(format!(
                        "conflicting rectangle `{}` aliases disagree beyond semantic roundoff",
                        key.0.display_path(),
                    ));
                }
            }
            for (point, target) in &group {
                let corner = rectangle_corner(&point.edit)
                    .ok_or_else(|| "rectangle group contains a non-rectangle lens".to_owned())?;
                if lenses.get(&corner).copied() != Some(point) {
                    return Err(format!(
                        "rectangle `{}` terminal contains an unauthenticated corner lens",
                        key.0.display_path(),
                    ));
                }
                let accepted = preview
                    .position(&point.handle)
                    .ok_or_else(|| "rectangle terminal lens has no accepted position".to_owned())?;
                if pair_bits(*target) != pair_bits(accepted) {
                    return Err(format!(
                        "rectangle `{}` placement is not authenticated by its accepted lens",
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

    #[allow(
        clippy::too_many_lines,
        reason = "terminal publication keeps semantic staging, independent native parity, selection, history, and receipt authority adjacent"
    )]
    fn publish_semantic_point_preview_overlay(
        &mut self,
        placements: &[(ExpandedWritablePoint, [f64; 2])],
        preview: TerminalPointPreview<'_>,
        rectangle_projections: &[RectangleTerminalProjection],
        selected_alias: Option<&IntentKey>,
        label: &str,
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
        let accepted_continuation = preview
            .session
            .accepted_state_for_current_input()
            .ok_or_else(|| {
                "terminal code point has no current accepted numerical continuation".to_owned()
            })?
            .document();
        let audited =
            materialize_code_project_incremental_with_overlay_and_accepted_continuation_audited(
                self.materialized
                    .as_deref()
                    .ok_or_else(|| "code project has no warm native authority".to_owned())?,
                &self.project,
                &self.session.snapshot().generated,
                &overlay,
                accepted_continuation,
            );
        let mut materialized = audited.outcome.map_err(|error| error.to_string())?;
        let staged_preview = TerminalPointPreview::from_accepted(&materialized.editor)?;
        for (point, position) in placements {
            let staged_position = staged_preview
                .position(&point.handle)
                .ok_or_else(|| "staged code point has no Cartesian instance seed".to_owned())?;
            let terminal_position = preview
                .position(&point.handle)
                .ok_or_else(|| "terminal code point has no accepted native position".to_owned())?;
            if pair_bits(staged_position) != pair_bits(terminal_position)
                || pair_bits(terminal_position) != pair_bits(*position)
            {
                return Err("semantic point draft failed exact terminal/native seed parity".into());
            }
        }
        validate_terminal_preview_native_parity_with_trace(
            preview,
            &materialized.editor,
            &materialized.expansion,
            placements,
            rectangle_projections,
            &[],
            None,
        )?;
        if let Some(alias) = selected_alias {
            let node = materialized
                .editor
                .coordinator()
                .intent()
                .graph()
                .node_by_symbol(alias)
                .ok_or_else(|| "published point overlay lost its selected declaration".to_owned())?
                .id;
            if !materialized.editor.set_selected_declaration(Some(node)) {
                return Err("published point overlay could not restore selection".into());
            }
        }
        let expansion = materialized.expansion.clone();
        let checkpoint = encode_editor_checkpoint(&materialized.editor)?;
        let mut delegated_editor = Box::new(
            materialized
                .editor
                .fork_accepted_authority()
                .map_err(|error| error.to_string())?,
        );
        if let Some(alias) = selected_alias {
            let node = delegated_editor
                .coordinator()
                .intent()
                .graph()
                .node_by_symbol(alias)
                .ok_or_else(|| "delegated point overlay lost its selected declaration".to_owned())?
                .id;
            if !delegated_editor.set_selected_declaration(Some(node)) {
                return Err("delegated point overlay could not restore selection".into());
            }
        }
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
        let receipt = self
            .session
            .apply_prepared(prepared)
            .map_err(|error| error.to_string())?;
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
        // Pointer-down may transiently select the shared producer point before
        // semantic disambiguation runs. The preferred declaration was
        // authenticated from the pre-press selection, and the chosen writable
        // lens owns its exact stable alias.
        let selected_alias = preferred_declaration.map(|_| point.handle.alias.clone());
        self.trace_semantic_point = Some((pointer_id, point.clone()));
        if candidates.len() == 1 || !point.source.is_reference() {
            self.pending_semantic_point_drag = Some(PendingSemanticPointDrag {
                pointer_id,
                session: self.session.identity().clone(),
                native_intent: editor.coordinator().intent().identity(),
                native_point,
                point,
                selected_alias,
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
            native_intent: materialized.editor.coordinator().intent().identity(),
            native_point: detached_point,
            point,
            selected_alias,
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
            let mut editor = self.restore_accepted_editor()?;
            if let Some(alias) = pending.selected_alias {
                let node = editor
                    .coordinator()
                    .intent()
                    .graph()
                    .node_by_symbol(&alias)
                    .ok_or_else(|| {
                        "restored point-drag origin lost its selected declaration".to_owned()
                    })?
                    .id;
                if !editor.set_selected_declaration(Some(node)) {
                    return Err(
                        "restored point-drag origin could not reselect its declaration".into(),
                    );
                }
            }
            Ok(Some(editor))
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

    /// Human-facing project title for presentation adapters.
    ///
    /// The title is origin metadata only; it carries no code-session or
    /// accepted-scene authority.
    pub(crate) fn title(&self) -> &'static str {
        self.origin.title()
    }

    /// Currently selected source path in the code workspace.
    pub(crate) fn selected_file_path(&self) -> &str {
        self.selected_file.path()
    }

    /// Exact live managed editor bytes, including an unapplied draft.
    pub(crate) fn managed_draft(&self) -> &str {
        &self.managed_draft
    }

    /// Current compiler-host diagnostic for the exact live draft bytes.
    pub(crate) const fn draft_diagnostic(&self) -> Option<&ManagedDiagnostic> {
        self.draft_diagnostic.as_ref()
    }

    /// Retained code/materialization failure projected for one durable
    /// Problems surface. The accepted editor checkpoint remains authoritative.
    pub(crate) fn retained_failure(&self) -> Option<(String, String)> {
        self.session
            .snapshot()
            .failure
            .as_ref()
            .map(|failure| (failure.stage.clone(), failure.diagnostic.clone()))
    }

    /// Read-only custom project files exposed to a presentation host.
    pub(crate) fn custom_files(&self) -> impl Iterator<Item = (&str, &str)> {
        self.project
            .custom_files
            .iter()
            .map(|(path, file)| (path.as_str(), file.contents.as_str()))
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

    /// Retains one compiler-host-rejected candidate as presentation-only
    /// source while the exact prior project/session/editor authority remains
    /// accepted. The UTF-8 byte span is independently checked before it can
    /// become a navigable diagnostic.
    pub(crate) fn retain_managed_compiler_draft(
        &mut self,
        candidate_source: String,
        diagnostic: String,
        span: ManagedSpan,
    ) -> Result<(), String> {
        validate_managed_draft_bound(&candidate_source)?;
        validate_managed_draft_diagnostic(&candidate_source, &diagnostic, span)?;
        let (line, column) = managed_diagnostic_line_column(&candidate_source, span.start);
        self.pending_semantic_point_drag = None;
        self.selected_file = SelectedCodeFile::Managed;
        self.managed_draft = candidate_source;
        self.draft_diagnostic = Some(ManagedDiagnostic {
            code: ManagedDiagnosticCode::UnsupportedSyntax,
            message: diagnostic,
            span,
            line,
            column,
        });
        Ok(())
    }

    /// Exact editor bytes for the explicitly non-canonical rescue download.
    ///
    /// No parse, project validation, or accepted-scene claim is implied. The
    /// generic download adapter applies the caller's presentation size bound.
    pub(crate) fn raw_managed_draft_file(&self) -> RawManagedDraftFile {
        RawManagedDraftFile {
            source: self.managed_draft.clone(),
        }
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
        let snapshot = self.session.snapshot();
        let expansion = snapshot
            .accepted_expansion
            .as_ref()
            .ok_or_else(|| "code project has no accepted expansion authority".to_owned())?;
        if let Some(child) = expansion.generated_child_for_alias(&node.symbol)
            && let CodeOwnerAddress::GeneratedMember { address } = &child.address.owner.address
        {
            let declaration = SemanticSymbol(address.invocation.clone());
            if snapshot
                .managed
                .program
                .declarations
                .iter()
                .any(|candidate| candidate.symbol == declaration)
            {
                return Ok(Some(declaration));
            }
            return Err(
                "selected generated declaration has no authenticated managed-source invocation"
                    .into(),
            );
        }
        match expansion.declaration_for_alias(&node.symbol) {
            Some(declaration) => Ok(Some(declaration.clone())),
            None if node.symbol.as_str().starts_with("code.") => Err(
                "selected code-owned declaration has no authenticated managed-source provenance"
                    .into(),
            ),
            None => Ok(None),
        }
    }

    /// Exact accepted `sketch.ts` statement owned by one managed declaration.
    ///
    /// This is intentionally separate from individual managed-control spans:
    /// a source-backed declaration such as a direct Fillet owns several
    /// independently editable leaves, while selection-level navigation needs
    /// one honest common source region.
    pub(crate) fn managed_declaration_source_span(
        &self,
        declaration: &SemanticSymbol,
    ) -> Result<ManagedSpan, String> {
        let snapshot = self.session.snapshot();
        let managed = snapshot
            .accepted_code_project
            .as_ref()
            .map_or(&snapshot.managed, |project| &project.managed);
        managed
            .program
            .declarations
            .iter()
            .find(|candidate| &candidate.symbol == declaration)
            .map(|candidate| candidate.statement_span)
            .ok_or_else(|| {
                format!(
                    "accepted managed declaration `{}` has no authenticated source statement",
                    declaration.0,
                )
            })
    }

    /// Derives presentation metadata for every parameter of the selected
    /// code-owned Inspector from one fresh manifest. Ordinary GUI-owned fields
    /// are omitted and retain their existing editor behavior.
    pub(crate) fn inspector_parameter_presentations(
        &self,
        editor: &ProjectionalEditorSession,
        inspector: &IntentInspectorProjection,
    ) -> Result<Vec<super::design_projection::InspectorParameterPresentation>, String> {
        let projection = editor.workbench_projection();
        let manifest = self.managed_controls_cached();
        let descriptors = super::design_projection::InspectorDescriptorIndex::new(inspector);
        self.inspector_parameter_presentations_with_manifest(
            editor,
            &projection,
            inspector,
            &descriptors,
            manifest.as_ref().map(Rc::as_ref).map_err(String::as_str),
        )
    }

    /// Uses the caller's one durable-render projection and managed manifest.
    /// The Code panel and Inspector therefore share the same transient control
    /// authority without either rebuilding it or rescanning complete fan-out.
    pub(crate) fn inspector_parameter_presentations_with_manifest(
        &self,
        editor: &ProjectionalEditorSession,
        projection: &IntentWorkbenchProjection,
        inspector: &IntentInspectorProjection,
        descriptors: &super::design_projection::InspectorDescriptorIndex<'_>,
        manifest: Result<&ManagedControlManifest, &str>,
    ) -> Result<Vec<super::design_projection::InspectorParameterPresentation>, String> {
        use super::design_projection::{
            InspectorParameterAuthority, InspectorParameterPresentation,
        };

        let Some(context) =
            self.managed_inspector_context(editor, projection, inspector, manifest)?
        else {
            return Ok(Vec::new());
        };
        let targets = std::iter::once(IntentInspectorEditTarget::Suppressed).chain(
            inspector.fields.iter().map(|field| match field {
                IntentInspectorField::Definition { definition, .. } => {
                    IntentInspectorEditTarget::Definition {
                        field: definition.clone(),
                    }
                }
                IntentInspectorField::Instance { leaf, .. } => {
                    IntentInspectorEditTarget::Instance { leaf: *leaf }
                }
            }),
        );
        targets
            .map(|target| {
                let authority =
                    match Self::resolve_managed_inspector_property(&context, descriptors, &target)?
                    {
                        ManagedInspectorPropertyResolution::ModifiableSource(control) => {
                            let path = managed_source_path(control);
                            let generated_consumer_count = control
                                .consumers
                                .iter()
                                .filter(|consumer| {
                                    matches!(
                                        consumer.target,
                                        ManagedControlConsumerTarget::Generated { .. }
                                    )
                                })
                                .count();
                            InspectorParameterAuthority::ModifiableSource {
                                control_id: control.id.0.clone(),
                                source_start: control.source.span.start,
                                source_end: control.source.span.end,
                                source_path: path,
                                source_text: control.source.source_text.clone(),
                                consumer_count: control.consumers.len(),
                                generated_consumer_count,
                            }
                        }
                        ManagedInspectorPropertyResolution::ModifiableInstance => {
                            InspectorParameterAuthority::ModifiableInstance
                        }
                        ManagedInspectorPropertyResolution::Encoded { reason } => {
                            InspectorParameterAuthority::Encoded { reason }
                        }
                        ManagedInspectorPropertyResolution::Blocked { reason } => {
                            InspectorParameterAuthority::Blocked { reason }
                        }
                    };
                Ok(InspectorParameterPresentation { target, authority })
            })
            .collect()
    }

    fn managed_inspector_scope(
        &self,
        editor: &ProjectionalEditorSession,
        projection: &IntentWorkbenchProjection,
        inspector: &IntentInspectorProjection,
    ) -> Result<Option<ManagedInspectorScope>, String> {
        if projection.identity != inspector.identity
            || editor.coordinator().intent().identity() != inspector.identity
            || editor.selected_declaration() != Some(inspector.node)
        {
            return Err("the Inspector belongs to a stale code-project projection".into());
        }
        if self
            .materialized
            .as_deref()
            .map(|materialized| materialized.editor.coordinator().intent().identity())
            != Some(editor.coordinator().intent().identity())
        {
            return Err("the Inspector does not match accepted code-project authority".into());
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
        let owner = {
            let expansion = self
                .session
                .snapshot()
                .accepted_expansion
                .as_ref()
                .ok_or_else(|| "code project has no accepted expansion authority".to_owned())?;
            if let Some(child) = expansion.generated_child_for_alias(&node.symbol) {
                Some(ManagedInspectorOwner::Generated(child.address.clone()))
            } else {
                expansion
                    .declaration_for_alias(&node.symbol)
                    .cloned()
                    .map(ManagedInspectorOwner::Declaration)
            }
        };
        let Some(owner) = owner else {
            if node.symbol.as_str().starts_with("code.") {
                return Err(
                    "selected code-owned declaration has no authenticated managed-source provenance"
                        .into(),
                );
            }
            return Ok(None);
        };
        Ok(Some(ManagedInspectorScope {
            owner,
            is_dimension: matches!(node.kind, IntentNodeKind::Dimension { .. }),
        }))
    }

    fn managed_inspector_context<'a>(
        &self,
        editor: &ProjectionalEditorSession,
        projection: &IntentWorkbenchProjection,
        inspector: &IntentInspectorProjection,
        manifest: Result<&'a ManagedControlManifest, &str>,
    ) -> Result<Option<ManagedInspectorContext<'a>>, String> {
        let Some(scope) = self.managed_inspector_scope(editor, projection, inspector)? else {
            return Ok(None);
        };
        let blocked_reason = self.session.snapshot().failure.as_ref().map_or_else(
            || manifest.as_ref().err().map(|reason| (*reason).to_owned()),
            |_| {
                Some(
                    "Resolve or Undo the retained code failure before modifying this parameter"
                        .into(),
                )
            },
        );
        Ok(Some(indexed_managed_inspector_context(
            scope.owner,
            scope.is_dimension,
            manifest.ok(),
            blocked_reason,
        )))
    }

    fn managed_inspector_property_path(
        descriptors: &super::design_projection::InspectorDescriptorIndex<'_>,
        target: &IntentInspectorEditTarget,
    ) -> Result<(SemanticOutputPath, bool), String> {
        Ok(match target {
            IntentInspectorEditTarget::Suppressed => (
                SemanticOutputPath(vec![ManagedPathSegment::Field("suppressed".into())]),
                false,
            ),
            IntentInspectorEditTarget::Definition { field } => descriptors
                .definition(field)
                .map(|descriptor| (semantic_output_path(&descriptor.path), false))
                .ok_or_else(|| {
                    "the managed Inspector definition has no current schema path".to_owned()
                })?,
            IntentInspectorEditTarget::Instance { leaf } => {
                let (descriptor, path) = descriptors.instance(*leaf).ok_or_else(|| {
                    "the managed Inspector instance leaf has no current output descriptor"
                        .to_owned()
                })?;
                (
                    semantic_output_path(path),
                    descriptor.kind == IntentPortKind::Point,
                )
            }
        })
    }

    #[allow(
        clippy::too_many_lines,
        reason = "one closed resolver authenticates every Inspector target against semantic owner, manifest consumer, schema path, access token, and truthful fallback authority"
    )]
    fn resolve_managed_inspector_property<'a>(
        context: &'a ManagedInspectorContext<'a>,
        descriptors: &super::design_projection::InspectorDescriptorIndex<'_>,
        target: &IntentInspectorEditTarget,
    ) -> Result<ManagedInspectorPropertyResolution<'a>, String> {
        let (property, solver_instance_fallback) =
            Self::managed_inspector_property_path(descriptors, target)?;
        if let Some(reason) = &context.blocked_reason {
            return Ok(ManagedInspectorPropertyResolution::Blocked {
                reason: reason.clone(),
            });
        }
        if solver_instance_fallback {
            return Ok(ManagedInspectorPropertyResolution::ModifiableInstance);
        }
        if let Some(matching) = context.routes.get(&property) {
            let [index] = matching.as_slice() else {
                return Err(
                    "the managed Inspector property resolves to more than one source control"
                        .into(),
                );
            };
            let control = context.controls[*index];
            return Ok(match &control.access {
                ManagedControlAccess::Editable { .. } => {
                    ManagedInspectorPropertyResolution::ModifiableSource(control)
                }
                ManagedControlAccess::ReadOnly { reason, navigation } => {
                    ManagedInspectorPropertyResolution::Encoded {
                        reason: managed_read_only_reason(*reason, navigation.as_ref()),
                    }
                }
            });
        }
        if context.families.len() > 1 {
            return Err("one Inspector owner resolves to inconsistent generated families".into());
        }
        let family = context
            .families
            .first()
            .map(|family| managed_family_label(family));
        Ok(ManagedInspectorPropertyResolution::Encoded {
            reason: match (&context.owner, family) {
                (ManagedInspectorOwner::Generated(_), Some(family)) => {
                    format!("Generated {family} state · not declared in sketch.ts")
                }
                (ManagedInspectorOwner::Generated(_), None) => {
                    "Generated native state · not declared in sketch.ts".into()
                }
                (ManagedInspectorOwner::Declaration(_), Some(family)) => {
                    format!("Code-owned {family} state · not declared in sketch.ts")
                }
                (ManagedInspectorOwner::Declaration(_), None) => {
                    "Code-owned native state · not declared in sketch.ts".into()
                }
            },
        })
    }

    /// Resolves the active selected computed Fillet through accepted code
    /// ownership to one exact managed radius control, then returns every
    /// current computed-Fillet consumer of that shared source value.
    /// Ordinary GUI-owned or unmanaged Fillets deliberately return `None`.
    pub(crate) fn delegated_computed_fillet_radius_group(
        &self,
        editor: &ProjectionalEditorSession,
    ) -> Result<Option<Vec<ComputedFeatureId>>, String> {
        self.managed_fillet_radius_route(editor)
            .map(|route| route.map(|route| route.features))
    }

    #[allow(
        clippy::too_many_lines,
        reason = "one route keeps selected native ownership, exact expansion provenance, fresh control fan-out, current feature state, and source radius parity adjacent"
    )]
    fn managed_fillet_radius_route(
        &self,
        editor: &ProjectionalEditorSession,
    ) -> Result<Option<ManagedFilletRadiusRoute>, String> {
        let manifest = self.managed_controls_cached()?;
        self.managed_fillet_radius_route_with_manifest(editor, &manifest)
    }

    #[allow(
        clippy::too_many_lines,
        reason = "one route keeps selected native ownership, exact expansion provenance, supplied control fan-out, current feature state, and source radius parity adjacent"
    )]
    fn managed_fillet_radius_route_with_manifest(
        &self,
        editor: &ProjectionalEditorSession,
        manifest: &ManagedControlManifest,
    ) -> Result<Option<ManagedFilletRadiusRoute>, String> {
        if self
            .materialized
            .as_deref()
            .map(|materialized| materialized.editor.coordinator().intent().identity())
            != Some(editor.coordinator().intent().identity())
        {
            return Err("the Fillet gesture does not match accepted code-project authority".into());
        }
        let Some(selected) = editor.selected_declaration() else {
            return Ok(None);
        };
        let accepted = editor
            .coordinator()
            .accepted_materialization()
            .ok_or_else(|| "code project has no accepted native authority".to_owned())?;
        let selected_features = accepted
            .ownership
            .node(selected)
            .into_iter()
            .flat_map(|owner| owner.owned.iter())
            .filter_map(|binding| match binding {
                IntentNativeBinding::ComputedFeature(feature) => Some(*feature),
                _ => None,
            })
            .collect::<Vec<_>>();
        let initiating_feature = match selected_features.as_slice() {
            [] => return Ok(None),
            [feature] => *feature,
            _ => {
                return Err("the selected declaration owns more than one computed feature".into());
            }
        };
        let node = editor
            .coordinator()
            .intent()
            .graph()
            .node(selected)
            .ok_or_else(|| "the selected Fillet declaration disappeared".to_owned())?;
        if !matches!(
            node.kind,
            IntentNodeKind::ComputedFeature {
                feature: geosolve_sketch_intent::ComputedFeatureKind::FilletSet
            }
        ) {
            return Ok(None);
        }
        let snapshot = self.session.snapshot();
        let expansion = snapshot
            .accepted_expansion
            .as_ref()
            .ok_or_else(|| "code project has no accepted expansion authority".to_owned())?;
        let generated = expansion.generated_child_for_alias(&node.symbol);
        let declaration = generated
            .is_none()
            .then(|| expansion.declaration_for_alias(&node.symbol))
            .flatten();
        if generated.is_none() && declaration.is_none() {
            if node.symbol.as_str().starts_with("code.") {
                return Err(
                    "selected code-owned Fillet has no authenticated expansion provenance".into(),
                );
            }
            return Ok(None);
        }
        if self.session.snapshot().failure.is_some() {
            return Err(
                "resolve or Undo the retained code failure before dragging a managed Fillet".into(),
            );
        }
        if self.is_dirty() {
            return Err(
                "Apply or Revert the managed-source draft before dragging a managed Fillet".into(),
            );
        }
        let radius_path = SemanticOutputPath(vec![ManagedPathSegment::Field("radius".into())]);
        let owns_selected =
            |target: &ManagedControlConsumerTarget| match (target, generated, declaration) {
                (
                    ManagedControlConsumerTarget::Generated {
                        address, identity, ..
                    },
                    Some(child),
                    None,
                ) => {
                    matches!(
                        &child.address.owner.address,
                        CodeOwnerAddress::GeneratedMember { address: child_address }
                            if child_address == address
                    ) && child.address.owner.allocation == identity.allocation
                        && child.address.owner.generation == identity.generation
                }
                (
                    ManagedControlConsumerTarget::Declaration {
                        declaration: candidate,
                        ..
                    },
                    None,
                    Some(declaration),
                ) => candidate == declaration,
                _ => false,
            };
        let mut controls = manifest.editable().filter(|control| {
            control
                .consumers
                .iter()
                .any(|consumer| consumer.property == radius_path && owns_selected(&consumer.target))
        });
        let Some(control) = controls.next() else {
            return Ok(None);
        };
        if controls.next().is_some() {
            return Err("selected Fillet radius resolves to more than one managed control".into());
        }
        let origin_radius = managed_numeric_value(&control.value).ok_or_else(|| {
            "managed Fillet radius is not a finite numeric source value".to_owned()
        })?;
        let value_kind = managed_fillet_radius_value_kind(control)?;
        let token = control
            .token()
            .expect("manifest editable iterator yields an editable token")
            .clone();
        let features_by_id = accepted
            .features
            .features()
            .iter()
            .map(|feature| (feature.id, feature))
            .collect::<BTreeMap<_, _>>();
        if features_by_id.len() != accepted.features.features().len() {
            return Err("accepted managed Fillet authority repeats one feature ID".into());
        }
        let initiating = features_by_id
            .get(&initiating_feature)
            .copied()
            .ok_or_else(|| {
                "selected managed Fillet is absent from accepted authority".to_owned()
            })?;
        let ComputedFeatureDefinition::FilletSet(initiating) = &initiating.definition;
        if initiating.radius.to_bits() != origin_radius.to_bits() {
            return Err(
                "managed Fillet source radius and accepted feature radius no longer match".into(),
            );
        }

        let alias_index = ManagedFilletAliasIndex::new(expansion);
        let graph = editor.coordinator().intent().graph();
        let nodes_by_symbol = graph
            .nodes()
            .values()
            .map(|candidate| (candidate.symbol.clone(), candidate))
            .collect::<BTreeMap<_, _>>();
        if nodes_by_symbol.len() != graph.nodes().len() {
            return Err("accepted intent repeats one managed Fillet symbol".into());
        }
        let current_features = accepted
            .computed
            .feature_evaluations()
            .iter()
            .filter_map(|evaluation| {
                matches!(
                    &evaluation.state,
                    ComputedFeatureEvaluationState::Current { .. }
                )
                .then_some(evaluation.feature)
            })
            .collect::<BTreeSet<_>>();

        let mut features = BTreeSet::new();
        for consumer in control
            .consumers
            .iter()
            .filter(|consumer| consumer.property == radius_path)
        {
            let family = match &consumer.target {
                ManagedControlConsumerTarget::Declaration { family, .. }
                | ManagedControlConsumerTarget::Generated { family, .. } => family.as_str(),
            };
            if !matches!(family, "computed.fillet" | "computed.filletSet") {
                continue;
            }
            let aliases = alias_index.aliases(&consumer.target);
            let Some(aliases) = aliases.filter(|aliases| !aliases.is_empty()) else {
                return Err(
                    "managed Fillet control consumer has no accepted semantic alias".into(),
                );
            };
            for alias in aliases {
                let node = nodes_by_symbol.get(*alias).copied().ok_or_else(|| {
                    "managed Fillet control consumer disappeared from accepted intent".to_owned()
                })?;
                if !matches!(
                    node.kind,
                    IntentNodeKind::ComputedFeature {
                        feature: geosolve_sketch_intent::ComputedFeatureKind::FilletSet
                    }
                ) {
                    return Err(
                        "managed Fillet control consumer does not own a FilletSet declaration"
                            .into(),
                    );
                }
                let mut owned = accepted
                    .ownership
                    .node(node.id)
                    .into_iter()
                    .flat_map(|owner| owner.owned.iter())
                    .filter_map(|binding| match binding {
                        IntentNativeBinding::ComputedFeature(feature) => Some(*feature),
                        _ => None,
                    });
                let Some(feature) = owned.next() else {
                    return Err(
                        "managed Fillet control consumer does not own exactly one feature".into(),
                    );
                };
                if owned.next().is_some() {
                    return Err(
                        "managed Fillet control consumer does not own exactly one feature".into(),
                    );
                }
                if !features.insert(feature) {
                    return Err(
                        "managed Fillet control fan-out repeats one computed feature".into(),
                    );
                }
            }
        }
        if !features.contains(&initiating_feature) {
            return Err("managed Fillet control fan-out omits the initiating feature".into());
        }
        for feature in &features {
            let definition = features_by_id.get(feature).copied().ok_or_else(|| {
                "managed Fillet consumer is absent from accepted authority".to_owned()
            })?;
            let ComputedFeatureDefinition::FilletSet(fillet) = &definition.definition;
            if fillet.radius.to_bits() != origin_radius.to_bits()
                || !current_features.contains(feature)
            {
                return Err("managed Fillet consumer group is mixed or not Current".into());
            }
        }
        Ok(Some(ManagedFilletRadiusRoute {
            token,
            value_kind,
            initiating_alias: node.symbol.clone(),
            initiating_feature,
            features: features.into_iter().collect(),
            origin_radius,
        }))
    }
    fn apply_managed_compilation_with_high_water(
        &mut self,
        compiled: CompiledManagedSource,
        declaration_name_high_water: u64,
    ) -> Result<CodeApplyOutcome, String> {
        self.pending_semantic_point_drag = None;
        compiled
            .validate_input_source(&self.managed_draft)
            .map_err(|error| error.to_string())?;
        let mut candidate_project = self.project.clone();
        if declaration_name_high_water < candidate_project.managed.declaration_name_high_water
            || declaration_name_high_water > geosolve_sketch_code::MAX_CODE_SESSION_WIRE_INTEGER
        {
            return Err(
                "managed declaration-name high-water is non-monotonic or not wire-safe".into(),
            );
        }
        candidate_project.managed = compiled
            .into_managed_document()
            .map_err(|error| error.to_string())?;
        candidate_project.managed.declaration_name_high_water = declaration_name_high_water;
        candidate_project
            .validate()
            .map_err(|error| error.to_string())?;
        self.apply_candidate_project(candidate_project)
    }

    fn apply_candidate_project(
        &mut self,
        candidate_project: CodeProject,
    ) -> Result<CodeApplyOutcome, String> {
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
                let candidate_cache =
                    rehydrate_restored_editor(&delegated_editor, expansion.clone())?;
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
    pub(crate) fn step_history(
        &mut self,
        undo: bool,
    ) -> Result<Option<AcceptedCodePublication>, String> {
        if self.is_dirty() {
            return Err(
                "Apply or Revert the managed-source draft before moving code history".into(),
            );
        }
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
        let candidate_cache = rehydrate_restored_editor(&editor, expansion)?;
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

    /// Exact outer code-session identity consumed by the code-control RPC.
    pub(crate) fn code_session_identity(&self) -> &CodeSessionIdentity {
        self.session.identity()
    }

    /// Derives the transient managed-control manifest from current code
    /// authority. A dirty editor draft is deliberately excluded: accepting a
    /// token derived beneath uncommitted text would silently overwrite that
    /// text when the control is edited.
    pub(crate) fn managed_controls(&self) -> Result<ManagedControlManifest, String> {
        self.managed_controls_cached()
            .map(|manifest| manifest.as_ref().clone())
    }

    pub(crate) fn managed_controls_cached(&self) -> Result<Rc<ManagedControlManifest>, String> {
        if self.is_dirty() {
            return Err(
                "Apply or Revert the managed-source draft before inspecting managed controls"
                    .into(),
            );
        }
        let expansion = self.session.snapshot().expansion.as_ref().ok_or_else(|| {
            "current managed source has no authenticated expansion for controls".to_owned()
        })?;
        let key = ManagedControlManifestCacheKey {
            session: self.session.identity().clone(),
            project: self.project.project.clone(),
            source_digest: self.project.managed.source_digest.clone(),
            expansion_digest: expansion.digest.clone(),
        };
        if let Some(cached) = self.managed_control_manifest_cache.borrow().as_ref()
            && cached.key == key
        {
            return Ok(Rc::clone(&cached.manifest));
        }
        let manifest = Rc::new(
            managed_control_manifest(&self.project, expansion)
                .map_err(|error| error.to_string())?,
        );
        *self.managed_control_manifest_cache.borrow_mut() = Some(ManagedControlManifestCache {
            key,
            manifest: Rc::clone(&manifest),
        });
        Ok(manifest)
    }

    /// Re-authenticates one browser navigation request against current clean
    /// project/source authority. The DOM may carry only a stable control ID
    /// and claimed span; neither is trusted until this fresh manifest agrees.
    pub(crate) fn open_managed_control_source(
        &mut self,
        id: &str,
        claimed_start: usize,
        claimed_end: usize,
    ) -> Result<(usize, usize), String> {
        let manifest = self.managed_controls_cached()?;
        let control = manifest
            .control(&ManagedControlId(id.to_owned()))
            .ok_or_else(|| "managed source navigation target is unavailable".to_owned())?;
        if !matches!(control.access, ManagedControlAccess::Editable { .. }) {
            return Err("managed source navigation target is not editable".into());
        }
        let span = control.source.span;
        if span.start != claimed_start
            || span.end != claimed_end
            || span.end > self.managed_draft.len()
        {
            return Err("managed source navigation target belongs to stale authority".into());
        }
        drop(manifest);
        self.select_file(MANAGED_FILE)?;
        Ok((span.start, span.end))
    }

    /// Authenticates a panel/Inspector control against current executed
    /// provenance and returns the one exact managed value mutation it
    /// permits. This is source-neutral; the bridge still prepares and resolves
    /// the ordinary digest-bound compiler transaction.
    pub(crate) fn managed_control_source_mutation(
        &self,
        id: &str,
        submission: ManagedControlSubmission,
    ) -> Result<Option<ManagedSketchMutation>, String> {
        if self.is_dirty() {
            return Err(
                "Apply or Revert the managed-source draft before editing managed controls".into(),
            );
        }
        let expansion = self
            .session
            .snapshot()
            .accepted_expansion
            .as_ref()
            .ok_or_else(|| "code project has no accepted expansion authority".to_owned())?;
        let authority = managed_control_authority(&self.project, expansion)
            .map_err(|error| error.to_string())?;
        let control = authority
            .manifest()
            .control(&ManagedControlId(id.to_owned()))
            .filter(|control| control.token().is_some())
            .ok_or_else(|| "managed control is unavailable or read-only".to_owned())?;
        let replacement = managed_value_from_submission(control, submission)?;
        if managed_values_exactly_equal(&control.value, &replacement) {
            return Ok(None);
        }
        let batch = ManagedControlEditBatch::new([ManagedControlEdit {
            token: control
                .token()
                .expect("editable managed control has one exact token")
                .clone(),
            value: replacement,
        }]);
        authority
            .prepare_mutation(&batch)
            .map(Some)
            .map_err(|error| error.to_string())
    }

    pub(crate) fn is_dirty(&self) -> bool {
        self.managed_draft != self.session.snapshot().managed.source
            || self.draft_diagnostic.is_some()
    }

    #[must_use]
    pub(crate) fn has_managed_authority(&self) -> bool {
        self.project.managed.compiled.is_some()
    }

    /// Projects managed declarations in compiler-authenticated source order.
    /// Generated artifact outputs remain children of their source invocation;
    /// the bridge never reconstructs this ownership from labels or aliases.
    // Keep lexical declaration ordering, generated ownership, selection, and
    // source-owned suppression in one projection so callers cannot combine
    // rows from mismatched session snapshots.
    #[allow(clippy::too_many_lines)]
    pub(crate) fn declaration_panel_projection(
        &self,
        editor: &ProjectionalEditorSession,
    ) -> ManagedDeclarationPanelProjection {
        let snapshot = self.session.snapshot();
        let managed = &snapshot.managed;
        let expansion = snapshot
            .expansion
            .as_ref()
            .or(snapshot.accepted_expansion.as_ref());
        let generated = if snapshot.failure.is_some() {
            snapshot
                .accepted_generated
                .as_ref()
                .unwrap_or(&snapshot.generated)
        } else {
            &snapshot.generated
        };
        let dirty = self.is_dirty();
        let blocked_reason = if dirty {
            Some(
                "Apply or Revert the managed-source draft before structured declaration actions"
                    .into(),
            )
        } else if snapshot.failure.is_some() {
            Some(
                "Resolve or Undo the retained code failure before structured declaration actions"
                    .into(),
            )
        } else {
            None
        };
        let manifest = blocked_reason
            .is_none()
            .then(|| self.managed_controls_cached().ok())
            .flatten();
        let selected = editor.selected_declaration();
        let graph = editor.coordinator().intent().graph();
        let mut groups = BTreeMap::<SemanticSymbol, String>::new();
        for organization in &managed.program.organizations {
            for declaration in &organization.declarations {
                groups
                    .entry(declaration.clone())
                    .or_insert_with(|| organization.name.clone());
            }
        }

        let declaration_symbols = managed
            .program
            .declarations
            .iter()
            .map(|declaration| declaration.symbol.clone())
            .collect::<BTreeSet<_>>();
        let mut generated_by_declaration = BTreeMap::<SemanticSymbol, Vec<_>>::new();
        if let Some(expansion) = expansion {
            for member in generated.ordered_members() {
                let Some(provenance) = expansion.generated_provenance.get(&member.address) else {
                    continue;
                };
                let invocation = SemanticSymbol(member.address.invocation.clone());
                let source_owner = if declaration_symbols.contains(&invocation) {
                    invocation
                } else {
                    provenance.declaration.clone()
                };
                generated_by_declaration
                    .entry(source_owner)
                    .or_default()
                    .push((member, provenance));
            }
        }

        let mut declarations = managed
            .program
            .declarations
            .iter()
            .map(|declaration| {
                let representative = expansion.and_then(|expansion| {
                    representative_declaration_node(expansion, graph, &declaration.symbol)
                });
                let explicit_suppressed = managed.compiled.as_deref().map_or_else(
                    || match &declaration.arguments {
                        ManagedValue::Object(arguments) => {
                            arguments.get("suppressed").and_then(|value| match value {
                                ManagedValue::Bool(value) => Some(*value),
                                _ => None,
                            })
                        }
                        _ => None,
                    },
                    |compiled| {
                        Some(
                            compiled
                                .declaration_is_suppressed(&declaration.symbol)
                                .expect("accepted managed suppression projection is valid"),
                        )
                    },
                );
                let suppression_control_id = manifest.as_ref().and_then(|manifest| {
                    manifest.controls.iter().find_map(|control| {
                        let exact_field = matches!(
                            control.source.path.0.as_slice(),
                            [ManagedPathSegment::Field(field)] if field == "suppressed"
                        );
                        (control.source.declaration == declaration.symbol
                            && exact_field
                            && matches!(control.value, ManagedValue::Bool(_))
                            && matches!(control.access, ManagedControlAccess::Editable { .. }))
                        .then(|| control.id.0.clone())
                    })
                });
                let generated = generated_by_declaration
                    .remove(&declaration.symbol)
                    .unwrap_or_default()
                    .into_iter()
                    .map(|(member, _)| {
                        let generated_node = expansion.and_then(|expansion| {
                            generated_member_node(expansion, graph, &member.address)
                        });
                        let host_children = expansion.map_or_else(Vec::new, |expansion| {
                            expansion
                                .generated_children
                                .iter()
                                .filter(|child| {
                                    matches!(
                                        &child.address.owner.address,
                                        CodeOwnerAddress::GeneratedMember { address }
                                            if address == &member.address
                                    )
                                })
                                .collect::<Vec<_>>()
                        });
                        let suppressed = !host_children.is_empty()
                            && host_children.iter().all(|child| child.suppressed);
                        let suppression_token = match host_children.as_slice() {
                            [child] => serde_json::to_string(&child.address).ok(),
                            _ => None,
                        };
                        ManagedGeneratedPanelRow {
                            id: generated_panel_row_id(&member.address),
                            address: member.address.clone(),
                            label: generated_member_label(&member.address),
                            kind: "Generated output".into(),
                            source_start: declaration.statement_span.start,
                            source_end: declaration.statement_span.end,
                            selected: generated_node == selected,
                            selection_node: generated_node,
                            suppressed,
                            suppression_token,
                        }
                    })
                    .collect();
                ManagedDeclarationPanelRow {
                    id: managed_panel_row_id(&declaration.symbol),
                    symbol: declaration.symbol.clone(),
                    label: declaration.symbol.0.clone(),
                    kind: declaration.patch.as_ref().map_or_else(
                        || declaration.builder_path.join("."),
                        |_| "Patch invocation".into(),
                    ),
                    group: groups.get(&declaration.symbol).cloned(),
                    source_start: declaration.statement_span.start,
                    source_end: declaration.statement_span.end,
                    selected: representative == selected,
                    selection_node: representative,
                    suppressed: explicit_suppressed,
                    suppression_control_id,
                    closure_role: ManagedDeclarationClosureRole::Independent,
                    closure_helpers: Vec::new(),
                    generated,
                }
            })
            .collect::<Vec<_>>();
        if let Some(compiled) = managed.compiled.as_deref() {
            let closures = compiled
                .source_declaration_closures()
                .expect("accepted managed declaration closure projection is valid");
            let mut rows = declarations
                .into_iter()
                .map(|row| (row.symbol.clone(), row))
                .collect::<BTreeMap<_, _>>();
            let helper_symbols = closures
                .iter()
                .flat_map(|closure| closure.helpers.iter().cloned())
                .collect::<BTreeSet<_>>();
            for closure in closures {
                let helpers = closure
                    .helpers
                    .into_iter()
                    .filter_map(|helper| {
                        rows.get_mut(&helper).map(|row| {
                            row.closure_role = ManagedDeclarationClosureRole::Helper {
                                root: closure.root.clone(),
                            };
                            row.clone()
                        })
                    })
                    .collect::<Vec<_>>();
                if let Some(root) = rows.get_mut(&closure.root) {
                    root.closure_role = ManagedDeclarationClosureRole::Root;
                    root.closure_helpers = helpers;
                }
            }
            declarations = managed
                .program
                .declarations
                .iter()
                .filter(|declaration| !helper_symbols.contains(&declaration.symbol))
                .filter_map(|declaration| rows.remove(&declaration.symbol))
                .collect();
        }
        ManagedDeclarationPanelProjection {
            source_digest: managed.source_digest.clone(),
            dirty,
            blocked_reason,
            declarations,
        }
    }

    pub(crate) fn managed_source(&self) -> &str {
        &self.session.snapshot().managed.source
    }

    pub(crate) fn panel_markup(&self) -> String {
        let manifest = self.managed_controls_cached();
        self.panel_markup_with_managed_controls(
            manifest.as_ref().map(Rc::as_ref).map_err(String::as_str),
        )
    }

    pub(crate) fn panel_markup_with_managed_controls(
        &self,
        manifest: Result<&ManagedControlManifest, &str>,
    ) -> String {
        let mut markup = String::new();
        markup.push_str("<div class=\"wb-code-project\">");
        self.write_project_header(&mut markup);
        self.write_file_tabs(&mut markup);
        self.write_source_surface(&mut markup);
        self.write_artifact_status(&mut markup);
        Self::write_managed_controls(&mut markup, manifest);
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
                "<strong>{}</strong><small>managed · revision {}{}</small>",
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
                        "<span>Executed managed sketch</span></div><div>",
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
                            "<div class=\"wb-code-diagnostic\" role=\"alert\" ",
                            "data-line=\"{}\" data-column=\"{}\" ",
                            "data-source-start=\"{}\" data-source-end=\"{}\">",
                            "<strong>Line {}, column {}</strong><span>{}</span></div>"
                        ),
                        diagnostic.line,
                        diagnostic.column,
                        diagnostic.span.start,
                        diagnostic.span.end,
                        diagnostic.line,
                        diagnostic.column,
                        escape_html(&diagnostic.message),
                    );
                } else {
                    markup.push_str(
                        "<p class=\"wb-code-editor-note\">Apply compiles and validates the complete candidate before one publication. Comments and unowned formatting stay byte-identical.</p>",
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
                    "{} pinned module{}",
                    artifact_count,
                    if artifact_count == 1 { "" } else { "s" },
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

    fn write_managed_controls(
        markup: &mut String,
        manifest: Result<&ManagedControlManifest, &str>,
    ) {
        let manifest = match manifest {
            Ok(manifest) => manifest,
            Err(error) => {
                let _ = write!(
                    markup,
                    concat!(
                        "<section class=\"wb-code-lenses\"><header><strong>Managed controls</strong>",
                        "<span>Apply or Revert the source draft to refresh controls</span></header>",
                        "<p class=\"wb-code-editor-note\">{}</p></section>"
                    ),
                    escape_html(error),
                );
                return;
            }
        };
        let mut groups = BTreeMap::<&str, Vec<&ManagedControl>>::new();
        for control in manifest.editable() {
            groups
                .entry(control.source.declaration.0.as_str())
                .or_default()
                .push(control);
        }
        if groups.is_empty() {
            return;
        }
        markup.push_str(
            "<section class=\"wb-code-lenses\"><header><strong>Managed controls</strong><span>Authenticated source values and complete fan-out</span></header>",
        );
        for (declaration, controls) in groups {
            let _ = write!(
                markup,
                "<div class=\"wb-code-control-group\"><strong>{}</strong>",
                escape_html(declaration),
            );
            for control in controls {
                let path = managed_path_text(&control.source.path.0);
                let consumers = control.consumers.len();
                let _ = write!(
                    markup,
                    concat!(
                        "<div class=\"wb-code-lens wb-code-managed-control\"><div>",
                        "<strong>{}</strong><span class=\"wb-code-control-authority\">",
                        "Modifiable in sketch.ts</span>",
                        "<small><code>{}</code> · {} consumer{}</small></div>"
                    ),
                    escape_html(&path),
                    escape_html(&control.source.source_text),
                    consumers,
                    if consumers == 1 { "" } else { "s" },
                );
                write_managed_control_input(markup, control, &path);
                markup.push_str("</div>");
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
                markup.push_str("</div>");
            }
            markup.push_str("</div>");
        }
        if members.is_empty() {
            markup.push_str("<p class=\"wb-code-empty\">This project has no structurally generated members.</p>");
        }
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
}

fn managed_panel_row_id(symbol: &SemanticSymbol) -> String {
    format!("managed:{}", symbol.0)
}

fn generated_panel_row_id(address: &GeneratedMemberAddress) -> String {
    format!(
        "generated:{}",
        serde_json::to_string(address)
            .expect("validated generated-member addresses serialize infallibly"),
    )
}

fn generated_member_label(address: &GeneratedMemberAddress) -> String {
    for path in [&address.member_key, &address.output, &address.template] {
        if !path.is_empty() {
            return path.join(" / ");
        }
    }
    address.invocation.clone()
}

fn generated_target_alias(
    target: &ExpandedSemanticTarget,
) -> Option<&geosolve_sketch_intent::IntentKey> {
    match target {
        ExpandedSemanticTarget::Declaration { alias, .. } => Some(alias),
        ExpandedSemanticTarget::Port { port } => Some(&port.alias),
        ExpandedSemanticTarget::FeatureCorner { corner } => Some(&corner.point.alias),
        ExpandedSemanticTarget::Collection { members } => members
            .values()
            .find_map(|member| generated_target_alias(member)),
        ExpandedSemanticTarget::HostOutput { .. } => None,
    }
}

fn generated_member_node(
    expansion: &ExpandedCodeProject,
    graph: &geosolve_sketch_intent::IntentGraph,
    address: &GeneratedMemberAddress,
) -> Option<NodeId> {
    // A host-generated semantic child (notably a computed Fillet) is the
    // selectable output owned by this row. Its generated-provenance target
    // may instead be the parent corner operand, so prefer the exact child
    // address before considering ordinary generated declaration aliases.
    expansion
        .generated_children
        .iter()
        .find_map(|child| {
            let matches = matches!(
                &child.address.owner.address,
                CodeOwnerAddress::GeneratedMember { address: candidate }
                    if candidate == address
            );
            matches
                .then(|| graph.node_by_symbol(&child.alias).map(|node| node.id))
                .flatten()
        })
        .or_else(|| {
            expansion
                .generated_provenance
                .get(address)
                .and_then(|provenance| generated_target_alias(&provenance.target))
                .and_then(|alias| graph.node_by_symbol(alias))
                .map(|node| node.id)
        })
}

fn representative_declaration_node(
    expansion: &ExpandedCodeProject,
    graph: &geosolve_sketch_intent::IntentGraph,
    declaration: &SemanticSymbol,
) -> Option<NodeId> {
    let generated_aliases = expansion
        .generated_provenance
        .values()
        .filter_map(|provenance| generated_target_alias(&provenance.target))
        .chain(
            expansion
                .generated_children
                .iter()
                .map(|child| &child.alias),
        )
        .collect::<BTreeSet<_>>();
    let candidates = expansion
        .declaration_provenance
        .iter()
        .filter(|(_, owner)| *owner == declaration)
        .filter_map(|(alias, _)| graph.node_by_symbol(alias).map(|node| (alias, node.id)))
        .collect::<Vec<_>>();
    candidates
        .iter()
        .find(|(alias, _)| !generated_aliases.contains(alias))
        .or_else(|| candidates.first())
        .map(|(_, node)| *node)
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

fn validate_managed_draft_diagnostic(
    source: &str,
    diagnostic: &str,
    span: ManagedSpan,
) -> Result<(), String> {
    if diagnostic.is_empty() {
        return Err("managed compiler diagnostic is empty".into());
    }
    if diagnostic.len() > MAX_MANAGED_DRAFT_DIAGNOSTIC_BYTES {
        return Err(format!(
            "managed compiler diagnostic is {} bytes; the limit is {MAX_MANAGED_DRAFT_DIAGNOSTIC_BYTES}",
            diagnostic.len(),
        ));
    }
    if span.start > span.end || span.end > source.len() {
        return Err("managed compiler diagnostic span is outside its candidate source".into());
    }
    if !source.is_char_boundary(span.start) || !source.is_char_boundary(span.end) {
        return Err("managed compiler diagnostic span splits a UTF-8 code point".into());
    }
    Ok(())
}

fn managed_diagnostic_line_column(source: &str, offset: usize) -> (usize, usize) {
    let prefix = &source[..offset];
    let line = prefix.bytes().filter(|byte| *byte == b'\n').count() + 1;
    let column = prefix
        .rsplit_once('\n')
        .map_or(prefix.chars().count(), |(_, tail)| tail.chars().count())
        + 1;
    (line, column)
}

fn restore_managed_draft_diagnostic(
    draft: &str,
    accepted_source: &str,
    persisted: Option<ManagedDiagnostic>,
) -> Result<Option<ManagedDiagnostic>, String> {
    if let Some(diagnostic) = persisted {
        validate_managed_draft_diagnostic(draft, &diagnostic.message, diagnostic.span)?;
        let (line, column) = managed_diagnostic_line_column(draft, diagnostic.span.start);
        if diagnostic.line != line || diagnostic.column != column {
            return Err("persisted managed draft diagnostic has stale line or column".into());
        }
        return Ok(Some(diagnostic));
    }
    // A dirty draft is non-authoritative until the compiler host returns a V3
    // envelope. Without a persisted compiler diagnostic there is deliberately
    // no parser-only validity classification to restore.
    let _ = accepted_source;
    Ok(None)
}

const fn managed_canvas_name_base(kind: &IntentNodeKind) -> &'static str {
    match kind {
        IntentNodeKind::Geometry {
            recipe: GeometryRecipeKind::Segment,
        } => "segment",
        IntentNodeKind::Geometry { .. } => "geometry",
        IntentNodeKind::Constraint { .. } => "constraint",
        IntentNodeKind::Dimension { .. } => "dimension",
        IntentNodeKind::Operation { .. } => "operation",
        IntentNodeKind::ComputedFeature { .. } => "feature",
        IntentNodeKind::Aggregate { .. } => "aggregate",
        IntentNodeKind::Parameter { .. } => "parameter",
        IntentNodeKind::External { .. } => "external",
        IntentNodeKind::Bootstrap { .. } => "bootstrap",
        IntentNodeKind::Annotation => "annotation",
        IntentNodeKind::Identity { .. } => "identity",
    }
}

const fn managed_canvas_namespace(kind: &IntentNodeKind) -> Option<&'static str> {
    match kind {
        IntentNodeKind::Geometry { .. } => Some("geometry"),
        IntentNodeKind::Constraint { .. } => Some("constraint"),
        IntentNodeKind::Dimension { .. } => Some("dimension"),
        IntentNodeKind::Operation { .. } => Some("operation"),
        IntentNodeKind::ComputedFeature { .. } => Some("computed"),
        IntentNodeKind::Aggregate { .. } => Some("aggregate"),
        IntentNodeKind::Parameter { .. }
        | IntentNodeKind::External { .. }
        | IntentNodeKind::Bootstrap { .. }
        | IntentNodeKind::Annotation
        | IntentNodeKind::Identity { .. } => None,
    }
}

fn allocate_canvas_declaration_names(
    project: &CodeProject,
    nodes: &[&IntentNode],
    current_high_water: u64,
) -> Result<(Vec<EditorBootstrapDeclaration>, u64), String> {
    let mut occupied = project
        .managed
        .program
        .declarations
        .iter()
        .flat_map(|declaration| [declaration.variable.clone(), declaration.symbol.0.clone()])
        .chain(
            project
                .managed
                .program
                .scalar_bindings
                .iter()
                .map(|binding| binding.variable.clone()),
        )
        .collect::<BTreeSet<_>>();
    let mut high_water = current_high_water;
    let mut declarations = Vec::with_capacity(nodes.len());
    for node in nodes {
        let symbol = loop {
            high_water = high_water
                .checked_add(1)
                .filter(|value| *value <= geosolve_sketch_code::MAX_CODE_SESSION_WIRE_INTEGER)
                .ok_or_else(|| "the managed declaration-name allocator is exhausted".to_owned())?;
            let candidate = format!("{}{high_water}", managed_canvas_name_base(&node.kind));
            if occupied.insert(candidate.clone()) {
                break candidate;
            }
        };
        declarations.push(EditorBootstrapDeclaration::new(
            node.id,
            SemanticSymbol(symbol),
        ));
    }

    // One unordered Intent patch allocates simultaneously ready declarations
    // by their durable native symbol, not by source-array order. GUI symbols
    // and managed declaration symbols intentionally use different spellings.
    // For declarations from the same authoring namespace/name family, assign
    // the already reserved monotonic names in their eventual native-symbol
    // order so a multi-node gesture retains the exact candidate identities.
    let mut families = BTreeMap::<(&str, &str), Vec<usize>>::new();
    for (index, node) in nodes.iter().enumerate() {
        let Some(namespace) = managed_canvas_namespace(&node.kind) else {
            continue;
        };
        families
            .entry((namespace, managed_canvas_name_base(&node.kind)))
            .or_default()
            .push(index);
    }
    for ((namespace, _), indices) in families {
        if indices.len() < 2 {
            continue;
        }
        let mut symbols = indices
            .iter()
            .map(|index| {
                let symbol = declarations[*index].symbol.clone();
                direct_declaration_intent_symbol(&project.project, namespace, &symbol)
                    .map(|intent| (intent, symbol))
                    .map_err(|error| error.to_string())
            })
            .collect::<Result<Vec<_>, _>>()?;
        symbols.sort_by(|left, right| left.0.cmp(&right.0));
        for (index, (_, symbol)) in indices.into_iter().zip(symbols) {
            declarations[index].symbol = symbol;
        }
    }
    Ok((declarations, high_water))
}

fn canvas_declaration_label_projections(
    candidate_editor: &ProjectionalEditorSession,
    declarations: &[EditorBootstrapDeclaration],
) -> Result<Vec<PreparedDeclarationLabelProjection>, String> {
    let graph = candidate_editor.coordinator().intent().graph();
    let mut nodes = BTreeSet::new();
    let mut symbols = BTreeSet::new();
    declarations
        .iter()
        .map(|declaration| {
            if !nodes.insert(declaration.node) || !symbols.insert(declaration.symbol.clone()) {
                return Err("canvas declaration label witness repeats a node or symbol".into());
            }
            let node = graph.node(declaration.node).ok_or_else(|| {
                format!(
                    "canvas declaration label witness lost candidate node {}",
                    declaration.node
                )
            })?;
            Ok(PreparedDeclarationLabelProjection {
                node: declaration.node,
                terminal_symbol: node.symbol.clone(),
                declaration: declaration.symbol.clone(),
            })
        })
        .collect()
}

pub(crate) fn sample_group_markup(selected: Option<&str>) -> String {
    let demos = bundled_code_project_demos();
    let mut markup = String::new();
    for group in [
        "Patterns & generated geometry",
        "Structures",
        "Fabrication & products",
    ] {
        let _ = write!(
            markup,
            "<li class=\"wb-sample-branch\"><button type=\"button\" data-sample-group-trigger aria-haspopup=\"menu\" aria-expanded=\"false\">{}<span aria-hidden=\"true\">›</span></button><ul class=\"wb-sample-flyout\">",
            escape_html(group),
        );
        for demo in demos
            .iter()
            .filter(|demo| demo.id.semantic_group() == group)
        {
            let _ = write!(
                markup,
                "<li><button type=\"button\" data-code-sample-id=\"{}\"{}>{}</button></li>",
                demo.id.key(),
                if selected == Some(demo.id.key()) {
                    " aria-current=\"true\""
                } else {
                    ""
                },
                escape_html(demo.title),
            );
        }
        markup.push_str("</ul></li>");
    }
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

/// Builds the reconstructible warm cache from an editor which has already
/// crossed the checkpoint restore/validation boundary. The returned fork is
/// history-free and presentation-disposable; the supplied editor remains the
/// sole value published to the live workbench.
fn rehydrate_restored_editor(
    editor: &ProjectionalEditorSession,
    expansion: ExpandedCodeProject,
) -> Result<Box<MaterializedCodeProject>, String> {
    let cache_editor = editor
        .fork_accepted_authority()
        .map_err(|error| error.to_string())?;
    rehydrate_materialized_code_project(Box::new(cache_editor), expansion)
        .map_err(|error| error.to_string())
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
    policy: &TerminalComputedParityPolicy,
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
                let admits_reanchoring = !terminal.suppressed;
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
                                    let picked_parameter_matches = match policy {
                                        TerminalComputedParityPolicy::RectangleAliasRoundoff {
                                            source_scales,
                                        } if admits_reanchoring
                                            && source_scales.contains_key(&terminal.source)
                                            && source_scales.contains_key(&staged.source) =>
                                        {
                                            terminal_derived_scalar_matches(
                                                terminal.picked_parameter,
                                                staged.picked_parameter,
                                                1.0,
                                            )
                                        }
                                        TerminalComputedParityPolicy::Exact
                                        | TerminalComputedParityPolicy::RectangleAliasRoundoff {
                                            ..
                                        } => {
                                            terminal.picked_parameter.to_bits()
                                                == staged.picked_parameter.to_bits()
                                        }
                                    };
                                    // `picked_parameter` is a recomputable
                                    // scalar only in the authenticated causal
                                    // curve closure. Every durable owner,
                                    // winding, neighborhood, normal, endpoint
                                    // and periodic anchor stays exact.
                                    terminal.source == staged.source
                                        && terminal.winding == staged.winding
                                        && terminal.neighborhood == staged.neighborhood
                                        && terminal.normal_side == staged.normal_side
                                        && terminal.retained_endpoint == staged.retained_endpoint
                                        && terminal.periodic_anchor == staged.periodic_anchor
                                        && picked_parameter_matches
                                })
                        })
            })
}

#[allow(
    clippy::too_many_lines,
    reason = "one fail-closed diagnostic walker mirrors the exact persistent feature/corner/parent hierarchy"
)]
fn first_terminal_feature_document_mismatch(
    terminal: &ComputedFeatureDocument,
    staged: &ComputedFeatureDocument,
    policy: &TerminalComputedParityPolicy,
) -> String {
    let terminal_identity = terminal.identity();
    let staged_identity = staged.identity();
    if terminal_identity.document != staged_identity.document {
        return format!(
            "feature.document terminal={:?} staged={:?}",
            terminal_identity.document, staged_identity.document
        );
    }
    if terminal_identity.sketch_document != staged_identity.sketch_document {
        return format!(
            "feature.sketch_document terminal={:?} staged={:?}",
            terminal_identity.sketch_document, staged_identity.sketch_document
        );
    }
    if terminal.allocator_high_water() != staged.allocator_high_water() {
        return format!(
            "feature.allocator terminal={:?} staged={:?}",
            terminal.allocator_high_water(),
            staged.allocator_high_water()
        );
    }
    if terminal.features().len() != staged.features().len() {
        return format!(
            "feature.count terminal={} staged={}",
            terminal.features().len(),
            staged.features().len()
        );
    }
    for (index, (terminal_feature, staged_feature)) in terminal
        .features()
        .iter()
        .zip(staged.features())
        .enumerate()
    {
        if terminal_feature.id != staged_feature.id {
            return format!(
                "feature[{index}].id terminal={:?} staged={:?}",
                terminal_feature.id, staged_feature.id
            );
        }
        if terminal_feature.label != staged_feature.label {
            return format!(
                "feature[{index}].label terminal={:?} staged={:?}",
                terminal_feature.label, staged_feature.label
            );
        }
        if terminal_feature.suppressed != staged_feature.suppressed {
            return format!(
                "feature[{index}].suppressed terminal={} staged={}",
                terminal_feature.suppressed, staged_feature.suppressed
            );
        }
        let (
            ComputedFeatureDefinition::FilletSet(terminal_fillet),
            ComputedFeatureDefinition::FilletSet(staged_fillet),
        ) = (&terminal_feature.definition, &staged_feature.definition);
        if terminal_fillet.radius.to_bits() != staged_fillet.radius.to_bits() {
            return format!(
                "feature[{index}].radius terminal={:.17e} staged={:.17e}",
                terminal_fillet.radius, staged_fillet.radius
            );
        }
        if terminal_fillet.corners.len() != staged_fillet.corners.len() {
            return format!(
                "feature[{index}].corner_count terminal={} staged={}",
                terminal_fillet.corners.len(),
                staged_fillet.corners.len()
            );
        }
        for (corner_index, (terminal_corner, staged_corner)) in terminal_fillet
            .corners
            .iter()
            .zip(&staged_fillet.corners)
            .enumerate()
        {
            if terminal_corner.id != staged_corner.id {
                return format!(
                    "feature[{index}].corner[{corner_index}].id terminal={:?} staged={:?}",
                    terminal_corner.id, staged_corner.id
                );
            }
            if terminal_corner.endpoint_order != staged_corner.endpoint_order {
                return format!(
                    "feature[{index}].corner[{corner_index}].endpoint_order terminal={:?} staged={:?}",
                    terminal_corner.endpoint_order, staged_corner.endpoint_order
                );
            }
            if terminal_corner.sweep != staged_corner.sweep {
                return format!(
                    "feature[{index}].corner[{corner_index}].sweep terminal={:?} staged={:?}",
                    terminal_corner.sweep, staged_corner.sweep
                );
            }
            for (parent_name, terminal_parent, staged_parent) in [
                ("first", terminal_corner.first, staged_corner.first),
                ("second", terminal_corner.second, staged_corner.second),
            ] {
                if terminal_parent.source != staged_parent.source {
                    return format!(
                        "feature[{index}].corner[{corner_index}].{parent_name}.source terminal={:?} staged={:?}",
                        terminal_parent.source, staged_parent.source
                    );
                }
                let parameter_matches = match policy {
                    TerminalComputedParityPolicy::RectangleAliasRoundoff { source_scales }
                        if !terminal_feature.suppressed
                            && source_scales.contains_key(&terminal_parent.source)
                            && source_scales.contains_key(&staged_parent.source) =>
                    {
                        terminal_derived_scalar_matches(
                            terminal_parent.picked_parameter,
                            staged_parent.picked_parameter,
                            1.0,
                        )
                    }
                    TerminalComputedParityPolicy::Exact
                    | TerminalComputedParityPolicy::RectangleAliasRoundoff { .. } => {
                        terminal_parent.picked_parameter.to_bits()
                            == staged_parent.picked_parameter.to_bits()
                    }
                };
                if !parameter_matches {
                    return format!(
                        "feature[{index}].corner[{corner_index}].{parent_name}.parameter terminal={:.17e} staged={:.17e}",
                        terminal_parent.picked_parameter, staged_parent.picked_parameter
                    );
                }
                if terminal_parent.winding != staged_parent.winding
                    || terminal_parent.neighborhood != staged_parent.neighborhood
                    || terminal_parent.normal_side != staged_parent.normal_side
                    || terminal_parent.retained_endpoint != staged_parent.retained_endpoint
                    || terminal_parent.periodic_anchor != staged_parent.periodic_anchor
                {
                    return format!(
                        "feature[{index}].corner[{corner_index}].{parent_name}.branch terminal={terminal_parent:?} staged={staged_parent:?}"
                    );
                }
            }
        }
    }
    "unknown feature-document mismatch".into()
}

fn terminal_derived_scalar_matches(first: f64, second: f64, coordinate_scale: f64) -> bool {
    // The staged overlay and accepted pointer terminal start from the same
    // authenticated rectangle seeds, but redundant rectangle aliases can
    // differ within the explicitly admitted terminal seed cell. Re-evaluated
    // Fillet coordinates may therefore inherit only that bounded ULP/near-zero
    // noise. Keep every discrete owner, branch, winding and topology field
    // exact; this predicate applies only to recomputable finite scalars.
    if !first.is_finite() || !second.is_finite() {
        return false;
    }
    let first_bits = first.to_bits();
    let second_bits = second.to_bits();
    if first_bits == second_bits
        || (first_bits & !F64_SIGN_MASK == 0 && second_bits & !F64_SIGN_MASK == 0)
    {
        return true;
    }
    let Some(tolerance) = semantic_roundoff_tolerance(first, second, coordinate_scale) else {
        return false;
    };
    let zero_tolerance = TERMINAL_SEED_ZERO_ROUNDOFF * coordinate_scale.max(1.0);
    (first_bits & F64_SIGN_MASK == second_bits & F64_SIGN_MASK
        && (first - second).abs() <= tolerance)
        || (first.abs() <= zero_tolerance && second.abs() <= zero_tolerance)
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct TerminalPeriodicAngleUnwrap {
    turns: i8,
    staged: f64,
    residual: f64,
}

fn terminal_periodic_angle_unwrap(
    terminal: f64,
    staged: f64,
) -> Option<TerminalPeriodicAngleUnwrap> {
    if !terminal.is_finite() || !staged.is_finite() {
        return None;
    }
    let delta = terminal - staged;
    if !delta.is_finite() {
        return None;
    }
    let rounded_turns = (delta / std::f64::consts::TAU).round();
    if !rounded_turns.is_finite() || rounded_turns.abs().to_bits() != 1.0_f64.to_bits() {
        return None;
    }
    let turns = if rounded_turns.is_sign_negative() {
        -1
    } else {
        1
    };
    let staged = staged + f64::from(turns) * std::f64::consts::TAU;
    let residual = terminal - staged;
    (staged.is_finite() && residual.is_finite()).then_some(TerminalPeriodicAngleUnwrap {
        turns,
        staged,
        residual,
    })
}

fn terminal_derived_periodic_angle_matches(first: f64, second: f64) -> bool {
    if terminal_derived_scalar_matches(first, second, 1.0) {
        return true;
    }
    terminal_periodic_angle_unwrap(first, second)
        .is_some_and(|unwrapped| terminal_derived_scalar_matches(first, unwrapped.staged, 1.0))
}

fn terminal_derived_pair_matches(first: [f64; 2], second: [f64; 2], coordinate_scale: f64) -> bool {
    first
        .into_iter()
        .zip(second)
        .all(|(first, second)| terminal_derived_scalar_matches(first, second, coordinate_scale))
}

fn terminal_computed_edge_roundoff_scale(
    terminal: &geosolve_constraint_editor::ComputedEdge,
    staged: &geosolve_constraint_editor::ComputedEdge,
    source_scales: &TerminalRoundoffSourceScales,
) -> Option<f64> {
    let mut maximum = None::<u64>;
    let mut include_source = |source| {
        if let Some(scale) = source_scales.get(&source).copied()
            && maximum.is_none_or(|current| {
                f64::from_bits(scale)
                    .total_cmp(&f64::from_bits(current))
                    .is_gt()
            })
        {
            maximum = Some(scale);
        }
    };
    match (
        &terminal.geometry,
        &terminal.provenance,
        &staged.geometry,
        &staged.provenance,
    ) {
        (
            ComputedEdgeGeometry::NativeSourceFragment {
                source: terminal_geometry,
                ..
            },
            ComputedEdgeProvenance::SourceFragment {
                source: terminal_provenance,
                start_claim: terminal_start,
                end_claim: terminal_end,
                ..
            },
            ComputedEdgeGeometry::NativeSourceFragment {
                source: staged_geometry,
                ..
            },
            ComputedEdgeProvenance::SourceFragment {
                source: staged_provenance,
                start_claim: staged_start,
                end_claim: staged_end,
                ..
            },
        ) if terminal_geometry == terminal_provenance
            && staged_geometry == staged_provenance
            && terminal_geometry == staged_geometry
            && terminal_start == staged_start
            && terminal_end == staged_end =>
        {
            include_source(*terminal_geometry);
        }
        (
            ComputedEdgeGeometry::CircularArc(terminal_geometry),
            ComputedEdgeProvenance::FilletArc {
                owner: terminal_owner,
                sources: terminal_sources,
            },
            ComputedEdgeGeometry::CircularArc(staged_geometry),
            ComputedEdgeProvenance::FilletArc {
                owner: staged_owner,
                sources: staged_sources,
            },
        ) if terminal_owner == staged_owner
            && terminal_sources == staged_sources
            && terminal_geometry
                .contacts
                .iter()
                .map(|contact| contact.source)
                .eq(terminal_sources.iter().copied())
            && staged_geometry
                .contacts
                .iter()
                .map(|contact| contact.source)
                .eq(staged_sources.iter().copied()) =>
        {
            for source in terminal_sources {
                include_source(*source);
            }
        }
        _ => return None,
    }
    maximum
        .map(f64::from_bits)
        .filter(|scale| scale.is_finite() && *scale > 0.0)
}

fn terminal_computed_edge_matches(
    terminal: &geosolve_constraint_editor::ComputedEdge,
    staged: &geosolve_constraint_editor::ComputedEdge,
    source_scales: &TerminalRoundoffSourceScales,
) -> bool {
    if terminal.id.ordinal != staged.id.ordinal || terminal.role != staged.role {
        return false;
    }
    let Some(coordinate_scale) =
        terminal_computed_edge_roundoff_scale(terminal, staged, source_scales)
    else {
        return terminal.geometry == staged.geometry && terminal.provenance == staged.provenance;
    };
    if !terminal_computed_provenance_matches(&terminal.provenance, &staged.provenance) {
        return false;
    }
    match (&terminal.geometry, &staged.geometry) {
        (
            ComputedEdgeGeometry::NativeSourceFragment {
                source: terminal_source,
                interval: terminal_interval,
            },
            ComputedEdgeGeometry::NativeSourceFragment {
                source: staged_source,
                interval: staged_interval,
            },
        ) => {
            terminal_source == staged_source
                && terminal_derived_scalar_matches(
                    terminal_interval.start,
                    staged_interval.start,
                    1.0,
                )
                && terminal_derived_scalar_matches(terminal_interval.end, staged_interval.end, 1.0)
        }
        (
            ComputedEdgeGeometry::CircularArc(terminal),
            ComputedEdgeGeometry::CircularArc(staged),
        ) => {
            terminal_derived_pair_matches(terminal.center, staged.center, coordinate_scale)
                && terminal.radius.to_bits() == staged.radius.to_bits()
                && terminal_derived_periodic_angle_matches(terminal.start_angle, staged.start_angle)
                && terminal_derived_periodic_angle_matches(terminal.end_angle, staged.end_angle)
                && terminal.sweep == staged.sweep
                && terminal.tangent_orientations == staged.tangent_orientations
                && terminal.contacts.len() == staged.contacts.len()
                && terminal
                    .contacts
                    .iter()
                    .zip(staged.contacts)
                    .all(|(terminal, staged)| {
                        terminal.source == staged.source
                            && terminal.winding == staged.winding
                            && terminal_derived_scalar_matches(
                                terminal.parameter,
                                staged.parameter,
                                1.0,
                            )
                            && terminal_derived_scalar_matches(
                                terminal.total_parameter,
                                staged.total_parameter,
                                1.0,
                            )
                            && terminal_derived_pair_matches(
                                terminal.position,
                                staged.position,
                                coordinate_scale,
                            )
                    })
        }
        _ => false,
    }
}

fn terminal_computed_provenance_matches(
    terminal: &ComputedEdgeProvenance,
    staged: &ComputedEdgeProvenance,
) -> bool {
    match (terminal, staged) {
        (
            ComputedEdgeProvenance::SourceFragment {
                source: terminal_source,
                interval: terminal_interval,
                start_claim: terminal_start,
                end_claim: terminal_end,
            },
            ComputedEdgeProvenance::SourceFragment {
                source: staged_source,
                interval: staged_interval,
                start_claim: staged_start,
                end_claim: staged_end,
            },
        ) => {
            terminal_source == staged_source
                && terminal_start == staged_start
                && terminal_end == staged_end
                && terminal_derived_scalar_matches(
                    terminal_interval.start,
                    staged_interval.start,
                    1.0,
                )
                && terminal_derived_scalar_matches(terminal_interval.end, staged_interval.end, 1.0)
        }
        (
            ComputedEdgeProvenance::FilletArc {
                owner: terminal_owner,
                sources: terminal_sources,
            },
            ComputedEdgeProvenance::FilletArc {
                owner: staged_owner,
                sources: staged_sources,
            },
        ) => terminal_owner == staged_owner && terminal_sources == staged_sources,
        _ => false,
    }
}

fn terminal_computed_fragment_matches(
    terminal: &geosolve_constraint_editor::ComputedConstructionFragment,
    staged: &geosolve_constraint_editor::ComputedConstructionFragment,
    source_scales: &TerminalRoundoffSourceScales,
) -> bool {
    if !source_scales.contains_key(&terminal.source) || !source_scales.contains_key(&staged.source)
    {
        return terminal_computed_fragment_exact_matches(terminal, staged);
    }
    terminal.id.ordinal == staged.id.ordinal
        && terminal.source == staged.source
        && terminal.source_role == staged.source_role
        && terminal.provenance.owner == staged.provenance.owner
        && terminal.provenance.endpoint == staged.provenance.endpoint
        && terminal_derived_scalar_matches(terminal.interval.start, staged.interval.start, 1.0)
        && terminal_derived_scalar_matches(terminal.interval.end, staged.interval.end, 1.0)
        && terminal_derived_scalar_matches(
            terminal.provenance.base_interval.start,
            staged.provenance.base_interval.start,
            1.0,
        )
        && terminal_derived_scalar_matches(
            terminal.provenance.base_interval.end,
            staged.provenance.base_interval.end,
            1.0,
        )
}

fn terminal_computed_fragment_exact_matches(
    terminal: &geosolve_constraint_editor::ComputedConstructionFragment,
    staged: &geosolve_constraint_editor::ComputedConstructionFragment,
) -> bool {
    terminal.id.ordinal == staged.id.ordinal
        && terminal.source == staged.source
        && terminal.interval == staged.interval
        && terminal.source_role == staged.source_role
        && terminal.provenance == staged.provenance
}

fn terminal_feature_evaluation_matches(
    terminal: &ComputedFeatureEvaluation,
    staged: &ComputedFeatureEvaluation,
) -> bool {
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
                    |((terminal_corner, terminal_edge), (staged_corner, staged_edge))| {
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
}

#[derive(Clone, Debug, PartialEq)]
enum TerminalComputedParityPolicy {
    Exact,
    RectangleAliasRoundoff {
        source_scales: TerminalRoundoffSourceScales,
    },
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum TerminalScalarParityPolicy {
    Exact,
    Roundoff { coordinate_scale: f64 },
}

fn computed_snapshots_match_for_terminal_parity(
    terminal: &ComputedFeatureSnapshot,
    staged: &ComputedFeatureSnapshot,
    policy: &TerminalComputedParityPolicy,
) -> bool {
    terminal.edges().len() == staged.edges().len()
        && terminal
            .edges()
            .iter()
            .zip(staged.edges())
            .all(|(terminal, staged)| match policy {
                TerminalComputedParityPolicy::Exact => {
                    terminal.id.ordinal == staged.id.ordinal
                        && terminal.role == staged.role
                        && terminal.geometry == staged.geometry
                        && terminal.provenance == staged.provenance
                }
                TerminalComputedParityPolicy::RectangleAliasRoundoff { source_scales } => {
                    terminal_computed_edge_matches(terminal, staged, source_scales)
                }
            })
        && terminal.construction_fragments().len() == staged.construction_fragments().len()
        && terminal
            .construction_fragments()
            .iter()
            .zip(staged.construction_fragments())
            .all(|(terminal, staged)| match policy {
                TerminalComputedParityPolicy::Exact => {
                    terminal_computed_fragment_exact_matches(terminal, staged)
                }
                TerminalComputedParityPolicy::RectangleAliasRoundoff { source_scales } => {
                    terminal_computed_fragment_matches(terminal, staged, source_scales)
                }
            })
        && terminal.replaced_sources() == staged.replaced_sources()
        && terminal.feature_evaluations().len() == staged.feature_evaluations().len()
        && terminal
            .feature_evaluations()
            .iter()
            .zip(staged.feature_evaluations())
            .all(|(terminal, staged)| terminal_feature_evaluation_matches(terminal, staged))
}

fn terminal_computed_periodic_angle_evidence(
    terminal: &ComputedFeatureSnapshot,
    staged: &ComputedFeatureSnapshot,
    policy: &TerminalComputedParityPolicy,
) -> Vec<String> {
    let TerminalComputedParityPolicy::RectangleAliasRoundoff { source_scales } = policy else {
        return Vec::new();
    };
    terminal
        .edges()
        .iter()
        .zip(staged.edges())
        .enumerate()
        .flat_map(|(index, (terminal_edge, staged_edge))| {
            if terminal_computed_edge_roundoff_scale(terminal_edge, staged_edge, source_scales)
                .is_none()
                || !terminal_computed_edge_matches(terminal_edge, staged_edge, source_scales)
            {
                return Vec::new();
            }
            let (
                ComputedEdgeGeometry::CircularArc(terminal_arc),
                ComputedEdgeGeometry::CircularArc(staged_arc),
            ) = (&terminal_edge.geometry, &staged_edge.geometry)
            else {
                return Vec::new();
            };
            [
                (
                    "start_angle",
                    terminal_arc.start_angle,
                    staged_arc.start_angle,
                ),
                (
                    "end_angle",
                    terminal_arc.end_angle,
                    staged_arc.end_angle,
                ),
            ]
            .into_iter()
            .filter_map(|(field, terminal, staged)| {
                if terminal_derived_scalar_matches(terminal, staged, 1.0) {
                    return None;
                }
                let unwrapped = terminal_periodic_angle_unwrap(terminal, staged)?;
                terminal_derived_scalar_matches(terminal, unwrapped.staged, 1.0).then(|| {
                    format!(
                        "path=edge[{index}].arc.{field} periodic_turn={} unwrapped_staged={:.17e} unwrapped_residual={:.17e}",
                        unwrapped.turns, unwrapped.staged, unwrapped.residual,
                    )
                })
            })
            .collect::<Vec<_>>()
        })
        .collect()
}

fn terminal_scalar_mismatch_trace(
    path: &str,
    terminal: f64,
    staged: f64,
    policy: TerminalScalarParityPolicy,
    coordinate_domain: bool,
) -> String {
    let (coordinate_scale, tolerance) = match policy {
        TerminalScalarParityPolicy::Exact => (None, None),
        TerminalScalarParityPolicy::Roundoff { coordinate_scale } => {
            let scale = if coordinate_domain {
                coordinate_scale
            } else {
                1.0
            };
            (
                Some(scale),
                semantic_roundoff_tolerance(terminal, staged, scale),
            )
        }
    };
    format!(
        "path={path} terminal={:.17e}/0x{:016x} staged={:.17e}/0x{:016x} ulp_diff={} epsilon_budget={} coordinate_scale={coordinate_scale:?} tolerance={tolerance:?} zero_cell={} terminal_finite={} staged_finite={}",
        terminal,
        terminal.to_bits(),
        staged,
        staged.to_bits(),
        terminal.to_bits().abs_diff(staged.to_bits()),
        TERMINAL_SEED_ROUNDOFF_ULPS,
        TERMINAL_SEED_ZERO_ROUNDOFF,
        terminal.is_finite(),
        staged.is_finite(),
    )
}

fn terminal_periodic_angle_mismatch_trace(
    path: &str,
    terminal: f64,
    staged: f64,
    policy: TerminalScalarParityPolicy,
) -> String {
    let scalar = terminal_scalar_mismatch_trace(path, terminal, staged, policy, false);
    match policy {
        TerminalScalarParityPolicy::Exact => {
            format!("{scalar} periodic_policy=exact periodic_turn=None")
        }
        TerminalScalarParityPolicy::Roundoff { .. } => {
            let unwrapped = terminal_periodic_angle_unwrap(terminal, staged);
            let turn = unwrapped.map(|unwrapped| unwrapped.turns);
            let staged = unwrapped.map(|unwrapped| unwrapped.staged);
            let residual = unwrapped.map(|unwrapped| unwrapped.residual);
            let tolerance = unwrapped
                .and_then(|unwrapped| semantic_roundoff_tolerance(terminal, unwrapped.staged, 1.0));
            format!(
                "{scalar} periodic_policy=one-turn periodic_turn={turn:?} unwrapped_staged={staged:?} unwrapped_residual={residual:?} unwrapped_tolerance={tolerance:?}"
            )
        }
    }
}

fn terminal_pair_mismatch_trace(
    path: &str,
    terminal: [f64; 2],
    staged: [f64; 2],
    policy: TerminalScalarParityPolicy,
) -> Option<String> {
    ["x", "y"]
        .into_iter()
        .zip(terminal.into_iter().zip(staged))
        .find_map(|(axis, (terminal, staged))| {
            (!terminal_scalar_matches_trace_policy(terminal, staged, policy, true)).then(|| {
                terminal_scalar_mismatch_trace(
                    &format!("{path}.{axis}"),
                    terminal,
                    staged,
                    policy,
                    true,
                )
            })
        })
}

fn terminal_scalar_matches_trace_policy(
    terminal: f64,
    staged: f64,
    policy: TerminalScalarParityPolicy,
    coordinate_domain: bool,
) -> bool {
    match policy {
        TerminalScalarParityPolicy::Exact => terminal.to_bits() == staged.to_bits(),
        TerminalScalarParityPolicy::Roundoff { coordinate_scale } => {
            terminal_derived_scalar_matches(
                terminal,
                staged,
                if coordinate_domain {
                    coordinate_scale
                } else {
                    1.0
                },
            )
        }
    }
}

fn terminal_periodic_angle_matches_trace_policy(
    terminal: f64,
    staged: f64,
    policy: TerminalScalarParityPolicy,
) -> bool {
    match policy {
        TerminalScalarParityPolicy::Exact => terminal.to_bits() == staged.to_bits(),
        TerminalScalarParityPolicy::Roundoff { .. } => {
            terminal_derived_periodic_angle_matches(terminal, staged)
        }
    }
}

fn first_terminal_computed_provenance_mismatch(
    prefix: &str,
    terminal: &ComputedEdgeProvenance,
    staged: &ComputedEdgeProvenance,
    policy: TerminalScalarParityPolicy,
) -> String {
    match (terminal, staged) {
        (
            ComputedEdgeProvenance::SourceFragment {
                source: terminal_source,
                interval: terminal_interval,
                start_claim: terminal_start,
                end_claim: terminal_end,
            },
            ComputedEdgeProvenance::SourceFragment {
                source: staged_source,
                interval: staged_interval,
                start_claim: staged_start,
                end_claim: staged_end,
            },
        ) => {
            for (path, terminal, staged) in [
                (
                    "source",
                    format!("{terminal_source:?}"),
                    format!("{staged_source:?}"),
                ),
                (
                    "start_claim",
                    format!("{terminal_start:?}"),
                    format!("{staged_start:?}"),
                ),
                (
                    "end_claim",
                    format!("{terminal_end:?}"),
                    format!("{staged_end:?}"),
                ),
            ] {
                if terminal != staged {
                    return format!(
                        "path={prefix}.provenance.{path} terminal={terminal} staged={staged}"
                    );
                }
            }
            for (path, terminal, staged) in [
                (
                    "interval.start",
                    terminal_interval.start,
                    staged_interval.start,
                ),
                ("interval.end", terminal_interval.end, staged_interval.end),
            ] {
                if !terminal_scalar_matches_trace_policy(terminal, staged, policy, false) {
                    return terminal_scalar_mismatch_trace(
                        &format!("{prefix}.provenance.{path}"),
                        terminal,
                        staged,
                        policy,
                        false,
                    );
                }
            }
            format!("path={prefix}.provenance.source_fragment unknown_mismatch")
        }
        (
            ComputedEdgeProvenance::FilletArc {
                owner: terminal_owner,
                sources: terminal_sources,
            },
            ComputedEdgeProvenance::FilletArc {
                owner: staged_owner,
                sources: staged_sources,
            },
        ) => {
            if terminal_owner != staged_owner {
                return format!(
                    "path={prefix}.provenance.owner terminal={terminal_owner:?} staged={staged_owner:?}"
                );
            }
            if terminal_sources != staged_sources {
                return format!(
                    "path={prefix}.provenance.sources terminal={terminal_sources:?} staged={staged_sources:?}"
                );
            }
            format!("path={prefix}.provenance.fillet_arc unknown_mismatch")
        }
        _ => format!("path={prefix}.provenance.variant terminal={terminal:?} staged={staged:?}"),
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "the diagnostic enumerates every exact computed-edge field in causal comparison order"
)]
fn first_terminal_computed_edge_mismatch(
    index: usize,
    terminal: &geosolve_constraint_editor::ComputedEdge,
    staged: &geosolve_constraint_editor::ComputedEdge,
    policy: &TerminalComputedParityPolicy,
) -> String {
    let prefix = format!("edge[{index}]");
    if terminal.id.ordinal != staged.id.ordinal {
        return format!(
            "path={prefix}.id.ordinal terminal={} staged={}",
            terminal.id.ordinal, staged.id.ordinal
        );
    }
    if terminal.role != staged.role {
        return format!(
            "path={prefix}.role terminal={:?} staged={:?}",
            terminal.role, staged.role
        );
    }
    let scalar_policy = match policy {
        TerminalComputedParityPolicy::Exact => TerminalScalarParityPolicy::Exact,
        TerminalComputedParityPolicy::RectangleAliasRoundoff { source_scales } => {
            terminal_computed_edge_roundoff_scale(terminal, staged, source_scales)
                .map_or(TerminalScalarParityPolicy::Exact, |coordinate_scale| {
                    TerminalScalarParityPolicy::Roundoff { coordinate_scale }
                })
        }
    };
    let provenance_matches = match scalar_policy {
        TerminalScalarParityPolicy::Exact => terminal.provenance == staged.provenance,
        TerminalScalarParityPolicy::Roundoff { .. } => {
            terminal_computed_provenance_matches(&terminal.provenance, &staged.provenance)
        }
    };
    if !provenance_matches {
        return first_terminal_computed_provenance_mismatch(
            &prefix,
            &terminal.provenance,
            &staged.provenance,
            scalar_policy,
        );
    }
    match (&terminal.geometry, &staged.geometry) {
        (
            ComputedEdgeGeometry::NativeSourceFragment {
                source: terminal_source,
                interval: terminal_interval,
            },
            ComputedEdgeGeometry::NativeSourceFragment {
                source: staged_source,
                interval: staged_interval,
            },
        ) => {
            if terminal_source != staged_source {
                return format!(
                    "path={prefix}.native_source terminal={terminal_source:?} staged={staged_source:?}"
                );
            }
            for (field, terminal, staged) in [
                (
                    "interval.start",
                    terminal_interval.start,
                    staged_interval.start,
                ),
                ("interval.end", terminal_interval.end, staged_interval.end),
            ] {
                if !terminal_scalar_matches_trace_policy(terminal, staged, scalar_policy, false) {
                    return terminal_scalar_mismatch_trace(
                        &format!("{prefix}.{field}"),
                        terminal,
                        staged,
                        scalar_policy,
                        false,
                    );
                }
            }
            format!("path={prefix}.native_fragment unknown_mismatch")
        }
        (
            ComputedEdgeGeometry::CircularArc(terminal),
            ComputedEdgeGeometry::CircularArc(staged),
        ) => {
            if let Some(mismatch) = terminal_pair_mismatch_trace(
                &format!("{prefix}.arc.center"),
                terminal.center,
                staged.center,
                scalar_policy,
            ) {
                return mismatch;
            }
            if terminal.radius.to_bits() != staged.radius.to_bits() {
                return terminal_scalar_mismatch_trace(
                    &format!("{prefix}.arc.radius"),
                    terminal.radius,
                    staged.radius,
                    TerminalScalarParityPolicy::Exact,
                    true,
                );
            }
            for (field, terminal, staged) in [
                ("start_angle", terminal.start_angle, staged.start_angle),
                ("end_angle", terminal.end_angle, staged.end_angle),
            ] {
                if !terminal_periodic_angle_matches_trace_policy(terminal, staged, scalar_policy) {
                    return terminal_periodic_angle_mismatch_trace(
                        &format!("{prefix}.arc.{field}"),
                        terminal,
                        staged,
                        scalar_policy,
                    );
                }
            }
            if terminal.sweep != staged.sweep {
                return format!(
                    "path={prefix}.arc.sweep terminal={:?} staged={:?}",
                    terminal.sweep, staged.sweep
                );
            }
            if terminal.tangent_orientations != staged.tangent_orientations {
                return format!(
                    "path={prefix}.arc.tangent_orientations terminal={:?} staged={:?}",
                    terminal.tangent_orientations, staged.tangent_orientations
                );
            }
            if terminal.contacts.len() != staged.contacts.len() {
                return format!(
                    "path={prefix}.arc.contacts.length terminal={} staged={}",
                    terminal.contacts.len(),
                    staged.contacts.len()
                );
            }
            for (contact_index, (terminal, staged)) in
                terminal.contacts.iter().zip(&staged.contacts).enumerate()
            {
                let contact = format!("{prefix}.arc.contacts[{contact_index}]");
                if terminal.source != staged.source {
                    return format!(
                        "path={contact}.source terminal={:?} staged={:?}",
                        terminal.source, staged.source
                    );
                }
                if terminal.winding != staged.winding {
                    return format!(
                        "path={contact}.winding terminal={} staged={}",
                        terminal.winding, staged.winding
                    );
                }
                for (field, terminal, staged) in [
                    ("parameter", terminal.parameter, staged.parameter),
                    (
                        "total_parameter",
                        terminal.total_parameter,
                        staged.total_parameter,
                    ),
                ] {
                    if !terminal_scalar_matches_trace_policy(terminal, staged, scalar_policy, false)
                    {
                        return terminal_scalar_mismatch_trace(
                            &format!("{contact}.{field}"),
                            terminal,
                            staged,
                            scalar_policy,
                            false,
                        );
                    }
                }
                if let Some(mismatch) = terminal_pair_mismatch_trace(
                    &format!("{contact}.position"),
                    terminal.position,
                    staged.position,
                    scalar_policy,
                ) {
                    return mismatch;
                }
            }
            format!("path={prefix}.arc unknown_mismatch")
        }
        _ => format!(
            "path={prefix}.geometry.variant terminal={:?} staged={:?}",
            terminal.geometry, staged.geometry
        ),
    }
}

fn first_terminal_feature_evaluation_mismatch(
    index: usize,
    terminal: &ComputedFeatureEvaluation,
    staged: &ComputedFeatureEvaluation,
) -> String {
    let prefix = format!("feature_evaluations[{index}]");
    if terminal.feature != staged.feature {
        return format!(
            "path={prefix}.feature terminal={:?} staged={:?}",
            terminal.feature, staged.feature
        );
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
            if terminal.len() != staged.len() {
                return format!(
                    "path={prefix}.current.corner_edges.length terminal={} staged={}",
                    terminal.len(),
                    staged.len()
                );
            }
            for (edge_index, ((terminal_corner, terminal_edge), (staged_corner, staged_edge))) in
                terminal.iter().zip(staged).enumerate()
            {
                let edge = format!("{prefix}.current.corner_edges[{edge_index}]");
                if terminal_corner != staged_corner {
                    return format!(
                        "path={edge}.corner terminal={terminal_corner:?} staged={staged_corner:?}"
                    );
                }
                if terminal_edge.ordinal != staged_edge.ordinal {
                    return format!(
                        "path={edge}.edge.ordinal terminal={} staged={}",
                        terminal_edge.ordinal, staged_edge.ordinal
                    );
                }
            }
            format!("path={prefix}.current unknown_mismatch")
        }
        (
            ComputedFeatureEvaluationState::Failed { failure: terminal },
            ComputedFeatureEvaluationState::Failed { failure: staged },
        ) => format!("path={prefix}.failed terminal={terminal:?} staged={staged:?}"),
        (terminal, staged) => {
            format!("path={prefix}.state terminal={terminal:?} staged={staged:?}")
        }
    }
}

fn first_terminal_computed_snapshot_mismatch(
    terminal: &ComputedFeatureSnapshot,
    staged: &ComputedFeatureSnapshot,
    policy: &TerminalComputedParityPolicy,
) -> String {
    if terminal.edges().len() != staged.edges().len() {
        return format!(
            "path=edges.length terminal={} staged={}",
            terminal.edges().len(),
            staged.edges().len()
        );
    }
    for (index, (terminal, staged)) in terminal.edges().iter().zip(staged.edges()).enumerate() {
        let matches = match policy {
            TerminalComputedParityPolicy::Exact => {
                terminal.id.ordinal == staged.id.ordinal
                    && terminal.role == staged.role
                    && terminal.geometry == staged.geometry
                    && terminal.provenance == staged.provenance
            }
            TerminalComputedParityPolicy::RectangleAliasRoundoff { source_scales } => {
                terminal_computed_edge_matches(terminal, staged, source_scales)
            }
        };
        if !matches {
            return first_terminal_computed_edge_mismatch(index, terminal, staged, policy);
        }
    }
    if terminal.construction_fragments().len() != staged.construction_fragments().len() {
        return format!(
            "path=construction_fragments.length terminal={} staged={}",
            terminal.construction_fragments().len(),
            staged.construction_fragments().len()
        );
    }
    for (index, (terminal, staged)) in terminal
        .construction_fragments()
        .iter()
        .zip(staged.construction_fragments())
        .enumerate()
    {
        let matches = match policy {
            TerminalComputedParityPolicy::Exact => {
                terminal_computed_fragment_exact_matches(terminal, staged)
            }
            TerminalComputedParityPolicy::RectangleAliasRoundoff { source_scales } => {
                terminal_computed_fragment_matches(terminal, staged, source_scales)
            }
        };
        if !matches {
            return format!(
                "path=construction_fragments[{index}] terminal={terminal:?} staged={staged:?}"
            );
        }
    }
    if terminal.replaced_sources() != staged.replaced_sources() {
        return format!(
            "path=replaced_sources terminal={:?} staged={:?}",
            terminal.replaced_sources(),
            staged.replaced_sources()
        );
    }
    if terminal.feature_evaluations().len() != staged.feature_evaluations().len() {
        return format!(
            "path=feature_evaluations.length terminal={} staged={}",
            terminal.feature_evaluations().len(),
            staged.feature_evaluations().len()
        );
    }
    for (index, (terminal, staged)) in terminal
        .feature_evaluations()
        .iter()
        .zip(staged.feature_evaluations())
        .enumerate()
    {
        if !terminal_feature_evaluation_matches(terminal, staged) {
            return first_terminal_feature_evaluation_mismatch(index, terminal, staged);
        }
    }
    "path=computed_snapshot unknown_mismatch".into()
}

type TerminalRectangleAliasScales = BTreeMap<geosolve_sketch::DesignPointId, u64>;
type TerminalRoundoffSourceScales = BTreeMap<NativeCurveSpanSource, u64>;

#[derive(Clone, Debug, Eq, PartialEq)]
enum TerminalDocumentParity {
    Exact,
    NormalizedRedundantRectangleAliases(TerminalRectangleAliasScales),
    Mismatch,
}

impl TerminalDocumentParity {
    const fn matches(&self) -> bool {
        !matches!(self, Self::Mismatch)
    }

    fn normalized_redundant_rectangle_aliases(&self) -> Option<&TerminalRectangleAliasScales> {
        match self {
            Self::NormalizedRedundantRectangleAliases(points) => Some(points),
            Self::Exact | Self::Mismatch => None,
        }
    }
}

fn terminal_computed_roundoff_points<'a>(
    design: &'a TerminalDocumentParity,
    accepted: &'a TerminalDocumentParity,
) -> Option<&'a TerminalRectangleAliasScales> {
    match (
        design.normalized_redundant_rectangle_aliases(),
        accepted.normalized_redundant_rectangle_aliases(),
    ) {
        (Some(design), Some(accepted))
            if design.keys().eq(accepted.keys()) && !accepted.is_empty() =>
        {
            Some(accepted)
        }
        _ => None,
    }
}

fn terminal_document_normalizations_match(
    design: &TerminalDocumentParity,
    accepted: &TerminalDocumentParity,
) -> bool {
    match (design, accepted) {
        (TerminalDocumentParity::Exact, TerminalDocumentParity::Exact) => true,
        (
            TerminalDocumentParity::NormalizedRedundantRectangleAliases(design),
            TerminalDocumentParity::Exact,
        ) => !design.is_empty(),
        (
            TerminalDocumentParity::NormalizedRedundantRectangleAliases(design),
            TerminalDocumentParity::NormalizedRedundantRectangleAliases(accepted),
        ) => design.keys().eq(accepted.keys()) && !design.is_empty(),
        _ => false,
    }
}

fn maximum_point_roundoff_scale(
    point_scales: &TerminalRectangleAliasScales,
    points: impl IntoIterator<Item = geosolve_sketch::DesignPointId>,
) -> Option<u64> {
    let mut maximum = None::<u64>;
    for point in points {
        if let Some(scale) = point_scales.get(&point).copied()
            && maximum.is_none_or(|current| {
                f64::from_bits(scale)
                    .total_cmp(&f64::from_bits(current))
                    .is_gt()
            })
        {
            maximum = Some(scale);
        }
    }
    maximum
}

fn curve_definition_roundoff_scale(
    definition: &geosolve_sketch::CurveDefinition,
    point_scales: &TerminalRectangleAliasScales,
) -> Option<u64> {
    use geosolve_sketch::CurveDefinition;

    match definition {
        CurveDefinition::Line { start, end, .. }
        | CurveDefinition::RationalQuadraticConic { start, end, .. } => {
            maximum_point_roundoff_scale(point_scales, [*start, *end])
        }
        CurveDefinition::Polyline {
            points: controls, ..
        } => maximum_point_roundoff_scale(point_scales, controls.iter().copied()),
        CurveDefinition::BSpline { .. } | CurveDefinition::Nurbs { .. } => None,
        CurveDefinition::Circle { center, .. } | CurveDefinition::CircularArc { center, .. } => {
            maximum_point_roundoff_scale(point_scales, [*center])
        }
        CurveDefinition::QuadraticBezier { controls } => {
            maximum_point_roundoff_scale(point_scales, controls.iter().copied())
        }
        CurveDefinition::CubicBezier { controls } => {
            maximum_point_roundoff_scale(point_scales, controls.iter().copied())
        }
        CurveDefinition::Ellipse {
            center,
            major_axis_point,
            ..
        }
        | CurveDefinition::EllipticalArc {
            center,
            major_axis_point,
            ..
        } => maximum_point_roundoff_scale(point_scales, [*center, *major_axis_point]),
        CurveDefinition::ParabolaSegment { vertex, focus, .. } => {
            maximum_point_roundoff_scale(point_scales, [*vertex, *focus])
        }
        CurveDefinition::HyperbolaSegment {
            center,
            transverse_axis_point,
            ..
        } => maximum_point_roundoff_scale(point_scales, [*center, *transverse_axis_point]),
    }
}

fn insert_terminal_roundoff_source(
    sources: &mut TerminalRoundoffSourceScales,
    source: NativeCurveSpanSource,
    scale: u64,
) -> Result<(), String> {
    if sources.insert(source, scale).is_some() {
        return Err("terminal roundoff policy repeats one native source span".into());
    }
    Ok(())
}

fn insert_terminal_spline_roundoff_sources(
    sources: &mut TerminalRoundoffSourceScales,
    curve: geosolve_sketch::CurveId,
    definition: &geosolve_sketch::CurveDefinition,
    point_scales: &TerminalRectangleAliasScales,
) -> Result<(), String> {
    let (geosolve_sketch::CurveDefinition::BSpline {
        form,
        degree,
        controls,
        knots,
        span_ids,
        ..
    }
    | geosolve_sketch::CurveDefinition::Nurbs {
        form,
        degree,
        controls,
        knots,
        span_ids,
        ..
    }) = definition
    else {
        return Err("terminal roundoff spline helper received a non-spline curve".into());
    };
    if !controls
        .iter()
        .any(|control| point_scales.contains_key(control))
    {
        return Ok(());
    }
    let basis = match form {
        geosolve_sketch::DocumentBSplineForm::Clamped => {
            geosolve_sketch::BSplineBasis::try_clamped(*degree, controls.len(), knots.clone())
        }
        geosolve_sketch::DocumentBSplineForm::Periodic => {
            geosolve_sketch::BSplineBasis::try_periodic(*degree, controls.len(), knots.clone())
        }
    }
    .map_err(|error| {
        format!("terminal roundoff policy encountered an invalid spline basis: {error}")
    })?;
    if span_ids.len() != basis.spans().len() {
        return Err(
            "terminal roundoff policy encountered inconsistent spline span identity".into(),
        );
    }
    for (segment, span) in span_ids.iter().zip(basis.spans()) {
        let support = span
            .support()
            .iter()
            .map(|index| controls.get(*index).copied())
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| {
                "terminal roundoff policy encountered invalid spline support".to_owned()
            })?;
        let Some(scale) = maximum_point_roundoff_scale(point_scales, support) else {
            continue;
        };
        insert_terminal_roundoff_source(
            sources,
            NativeCurveSpanSource {
                span: CurveSpan {
                    curve,
                    segment: *segment,
                },
            },
            scale,
        )?;
    }
    Ok(())
}

fn terminal_roundoff_sources(
    document: &geosolve_sketch::SketchDocument,
    point_scales: &TerminalRectangleAliasScales,
) -> Result<TerminalRoundoffSourceScales, String> {
    use geosolve_sketch::CurveDefinition;

    let mut sources = TerminalRoundoffSourceScales::new();
    for curve in document.curves() {
        match &curve.definition {
            CurveDefinition::Polyline { points, closed, .. } => {
                let count = points.len().saturating_sub(1) + usize::from(*closed);
                for index in 0..count {
                    let end = if index + 1 == points.len() {
                        0
                    } else {
                        index + 1
                    };
                    let start_point = points.get(index).copied().ok_or_else(|| {
                        "terminal roundoff policy encountered an invalid polyline span".to_owned()
                    })?;
                    let end_point = points.get(end).copied().ok_or_else(|| {
                        "terminal roundoff policy encountered an invalid polyline span".to_owned()
                    })?;
                    let Some(scale) =
                        maximum_point_roundoff_scale(point_scales, [start_point, end_point])
                    else {
                        continue;
                    };
                    let segment = u32::try_from(index).map_err(|_| {
                        "terminal roundoff policy has an unrepresentable polyline span".to_owned()
                    })?;
                    insert_terminal_roundoff_source(
                        &mut sources,
                        NativeCurveSpanSource {
                            span: CurveSpan {
                                curve: curve.id,
                                segment,
                            },
                        },
                        scale,
                    )?;
                }
            }
            CurveDefinition::BSpline { .. } | CurveDefinition::Nurbs { .. } => {
                insert_terminal_spline_roundoff_sources(
                    &mut sources,
                    curve.id,
                    &curve.definition,
                    point_scales,
                )?;
            }
            _ => {
                let Some(scale) = curve_definition_roundoff_scale(&curve.definition, point_scales)
                else {
                    continue;
                };
                insert_terminal_roundoff_source(
                    &mut sources,
                    NativeCurveSpanSource {
                        span: CurveSpan {
                            curve: curve.id,
                            segment: 0,
                        },
                    },
                    scale,
                )?;
            }
        }
    }
    Ok(sources)
}

fn point_positions_match_bits(
    left: &geosolve_sketch::SketchDocument,
    right: &geosolve_sketch::SketchDocument,
) -> bool {
    left.points().len() == right.points().len()
        && left
            .points()
            .iter()
            .zip(right.points())
            .all(|(left, right)| {
                left.id == right.id && pair_bits(left.position) == pair_bits(right.position)
            })
}

fn scalar_values_match_bits(
    left: &geosolve_sketch::SketchDocument,
    right: &geosolve_sketch::SketchDocument,
) -> bool {
    left.scalars().len() == right.scalars().len()
        && left
            .scalars()
            .iter()
            .zip(right.scalars())
            .all(|(left, right)| {
                left.id == right.id && left.value.to_bits() == right.value.to_bits()
            })
}

#[allow(
    clippy::too_many_lines,
    reason = "one fail-closed parity normalizer keeps per-rectangle anchor, alias, local-scale, and exact-document checks adjacent"
)]
fn documents_match_for_terminal_parity(
    terminal_editor: &ProjectionalEditorSession,
    staged_editor: &ProjectionalEditorSession,
    terminal: &geosolve_sketch::SketchDocument,
    staged: &geosolve_sketch::SketchDocument,
    rectangle_projections: &[RectangleTerminalProjection],
    recomputable_line_branches: &BTreeSet<geosolve_sketch::CurveId>,
    object_relabels: &[DocumentObjectRelabel],
) -> Result<TerminalDocumentParity, String> {
    if terminal.exact_except_recomputable_line_branches_and_object_relabels(
        staged,
        recomputable_line_branches,
        object_relabels,
    ) && point_positions_match_bits(terminal, staged)
        && scalar_values_match_bits(terminal, staged)
    {
        return Ok(TerminalDocumentParity::Exact);
    }
    let resolve = |editor: &ProjectionalEditorSession, handle: &ExpandedPort| {
        expanded_port_point(editor, handle)
            .ok_or_else(|| "rectangle parity lens has no accepted native point binding".to_owned())
    };
    let mut anchors = BTreeSet::new();
    let mut redundant = BTreeSet::new();
    let mut redundant_scales = BTreeMap::new();
    for projection in rectangle_projections {
        let mut local_points = BTreeSet::new();
        for handle in &projection.anchors {
            let terminal_point = resolve(terminal_editor, handle)?;
            if terminal_point != resolve(staged_editor, handle)? {
                return Ok(TerminalDocumentParity::Mismatch);
            }
            if !anchors.insert(terminal_point) {
                return Err("rectangle parity repeats one anchor point".into());
            }
            local_points.insert(terminal_point);
        }
        for handle in &projection.redundant_aliases {
            let terminal_point = resolve(terminal_editor, handle)?;
            if terminal_point != resolve(staged_editor, handle)? {
                return Ok(TerminalDocumentParity::Mismatch);
            }
            if !redundant.insert(terminal_point) {
                return Err("rectangle parity repeats one redundant point".into());
            }
            local_points.insert(terminal_point);
        }
        let local_scale = semantic_point_coordinate_scale([terminal, staged], &local_points)
            .ok_or_else(|| {
                "rectangle parity has no finite local semantic coordinate scale".to_owned()
            })?;
        for handle in &projection.redundant_aliases {
            let point = resolve(terminal_editor, handle)?;
            if redundant_scales.insert(point, local_scale).is_some() {
                return Err("rectangle parity repeats one redundant point scale".into());
            }
        }
    }
    if !anchors.is_disjoint(&redundant) {
        return Err("rectangle parity anchor aliases a redundant point".into());
    }
    let mut normalized = terminal.clone();
    let mut normalized_redundant_rectangle_aliases = TerminalRectangleAliasScales::new();
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
            return Ok(TerminalDocumentParity::Mismatch);
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
        if !terminal_position.into_iter().all(f64::is_finite)
            || !staged_position.into_iter().all(f64::is_finite)
        {
            return Ok(TerminalDocumentParity::Mismatch);
        }
        let coordinate_scale = redundant_scales
            .get(&point)
            .copied()
            .ok_or_else(|| "rectangle parity redundant point has no local scale".to_owned())?;
        if !point_seed_roundoff_compatible(terminal_position, staged_position, coordinate_scale) {
            return Ok(TerminalDocumentParity::Mismatch);
        }
        normalized
            .set_point_position(point, staged_position)
            .map_err(|error| format!("rectangle parity normalization failed: {error}"))?;
        if normalized_redundant_rectangle_aliases
            .insert(point, coordinate_scale.to_bits())
            .is_some()
        {
            return Err("rectangle parity repeats one normalized alias scale".into());
        }
    }
    if !normalized.exact_except_recomputable_line_branches_and_object_relabels(
        staged,
        recomputable_line_branches,
        object_relabels,
    ) || !point_positions_match_bits(&normalized, staged)
        || !scalar_values_match_bits(&normalized, staged)
    {
        return Ok(TerminalDocumentParity::Mismatch);
    }
    Ok(if normalized_redundant_rectangle_aliases.is_empty() {
        TerminalDocumentParity::Exact
    } else {
        TerminalDocumentParity::NormalizedRedundantRectangleAliases(
            normalized_redundant_rectangle_aliases,
        )
    })
}

/// Reconstructs the exact source-seeded design witness beneath a delegated
/// native drag. Retained preview sessions keep their origin design inputs and
/// place the pointer target in the accepted solve; the semantic owner instead
/// persists only its authenticated source seeds. Rectangle aliases are added
/// as derived parity witnesses, never as extra drafts.
fn terminal_seeded_design_document(
    preview: TerminalPointPreview<'_>,
    placements: &[(ExpandedWritablePoint, [f64; 2])],
    rectangle_projections: &[RectangleTerminalProjection],
) -> Result<geosolve_sketch::SketchDocument, String> {
    let mut targets = BTreeMap::<geosolve_sketch::DesignPointId, [f64; 2]>::new();
    let mut insert = |handle: &ExpandedPort, target: [f64; 2]| -> Result<(), String> {
        let point = preview
            .point(handle)
            .ok_or_else(|| "terminal design seed has no native point binding".to_owned())?;
        if let Some(previous) = targets.insert(point, target)
            && pair_bits(previous) != pair_bits(target)
        {
            return Err("terminal design seed aliases conflicting native positions".into());
        }
        Ok(())
    };
    for (point, target) in placements {
        insert(&point.handle, *target)?;
    }
    for projection in rectangle_projections {
        for handle in projection
            .anchors
            .iter()
            .chain(&projection.redundant_aliases)
        {
            let target = preview.position(handle).ok_or_else(|| {
                "terminal rectangle design witness has no accepted position".to_owned()
            })?;
            insert(handle, target)?;
        }
    }
    let mut design = preview.session.design_document().clone();
    for (point, target) in targets {
        design
            .set_point_position(point, target)
            .map_err(|error| format!("terminal design seed normalization failed: {error}"))?;
    }
    Ok(design)
}

fn terminal_semantic_inputs_match(
    origin: &RetainedSketchDocumentSession,
    staged: &RetainedSketchDocumentSession,
) -> bool {
    let Some(origin_input) = origin
        .accepted_prepared_input()
        .map(geosolve_sketch::PreparedSketchInput::attempt_input)
    else {
        return false;
    };
    let Some(staged_input) = staged
        .accepted_prepared_input()
        .map(geosolve_sketch::PreparedSketchInput::attempt_input)
    else {
        return false;
    };
    origin_input.publication_request() == staged_input.publication_request()
        && origin_input.solver_config() == staged_input.solver_config()
        && origin_input.effective_activation_revision()
            == staged_input.effective_activation_revision()
        && origin_input.activation_digest() == staged_input.activation_digest()
        && origin_input.parameter_revision() == staged_input.parameter_revision()
        && origin_input.parameter_digest() == staged_input.parameter_digest()
        && origin_input.external_snapshot_set_revision()
            == staged_input.external_snapshot_set_revision()
        && origin_input.external_snapshot_set_digest()
            == staged_input.external_snapshot_set_digest()
        && origin.request() == staged.request()
        && origin.parameter_batch() == staged.parameter_batch()
        && origin.external_snapshot_set() == staged.external_snapshot_set()
        && origin.accepted_parameter_batch() == staged.accepted_parameter_batch()
        && origin.accepted_external_snapshot_set() == staged.accepted_external_snapshot_set()
}

fn document_object_for_native_binding(binding: IntentNativeBinding) -> Option<DocumentObjectId> {
    match binding {
        IntentNativeBinding::Point(id) => Some(DocumentObjectId::Point(id)),
        IntentNativeBinding::Scalar(id) => Some(DocumentObjectId::Scalar(id)),
        IntentNativeBinding::Curve(id) => Some(DocumentObjectId::Curve(id)),
        IntentNativeBinding::Contact(id) => Some(DocumentObjectId::Contact(id)),
        IntentNativeBinding::Constraint(id) => Some(DocumentObjectId::Constraint(id)),
        IntentNativeBinding::Dimension(id) => Some(DocumentObjectId::Dimension(id)),
        IntentNativeBinding::Parameter(id) => Some(DocumentObjectId::Parameter(id)),
        IntentNativeBinding::ExternalBinding(id) => Some(DocumentObjectId::ExternalBinding(id)),
        IntentNativeBinding::CurveSpan(_)
        | IntentNativeBinding::Source(_)
        | IntentNativeBinding::ComputedFeature(_)
        | IntentNativeBinding::ComputedFeatureCorner(_)
        | IntentNativeBinding::Logical(_) => None,
    }
}

fn document_object_label(
    document: &geosolve_sketch::SketchDocument,
    object: DocumentObjectId,
) -> Option<&str> {
    match object {
        DocumentObjectId::Point(id) => document.point(id).map(|value| value.label.as_str()),
        DocumentObjectId::Scalar(id) => document.scalar(id).map(|value| value.label.as_str()),
        DocumentObjectId::Curve(id) => document.curve(id).map(|value| value.label.as_str()),
        DocumentObjectId::Contact(id) => document.contact(id).map(|value| value.label.as_str()),
        DocumentObjectId::Constraint(id) => {
            document.constraint(id).map(|value| value.label.as_str())
        }
        DocumentObjectId::Dimension(id) => document.dimension(id).map(|value| value.label.as_str()),
        DocumentObjectId::Parameter(id) => document.parameter(id).map(|value| value.label.as_str()),
        DocumentObjectId::ExternalBinding(id) => document
            .external_binding(id)
            .map(|value| value.label.as_str()),
    }
}

fn authenticated_declaration_object_relabels(
    terminal: &ProjectionalEditorSession,
    staged: &ProjectionalEditorSession,
    expansion: &ExpandedCodeProject,
    projections: &[PreparedDeclarationLabelProjection],
    terminal_document: &geosolve_sketch::SketchDocument,
    staged_document: &geosolve_sketch::SketchDocument,
) -> Result<Vec<DocumentObjectRelabel>, String> {
    if projections.is_empty() {
        return Ok(Vec::new());
    }
    let terminal_authority = terminal
        .coordinator()
        .accepted_materialization()
        .ok_or_else(|| "declaration relabel witness has no terminal native authority".to_owned())?;
    let staged_authority = staged
        .coordinator()
        .accepted_materialization()
        .ok_or_else(|| "declaration relabel witness has no staged native authority".to_owned())?;
    let terminal_graph = terminal.coordinator().intent().graph();
    let staged_graph = staged.coordinator().intent().graph();
    let mut witnessed_terminal_nodes = BTreeSet::new();
    let mut witnessed_staged_nodes = BTreeSet::new();
    let mut witnessed_objects = BTreeSet::new();
    let mut relabels = Vec::new();
    for projection in projections {
        if !witnessed_terminal_nodes.insert(projection.node) {
            return Err("declaration relabel witness repeats a terminal persistent node".into());
        }
        let terminal_node = terminal_graph.node(projection.node).ok_or_else(|| {
            format!(
                "declaration relabel witness lost terminal node {}",
                projection.node
            )
        })?;
        if terminal_node.symbol != projection.terminal_symbol {
            return Err("declaration relabel witness terminal symbol is stale".into());
        }
        let mut staged_matches = staged_graph.nodes().values().filter(|node| {
            expansion.declaration_for_alias(&node.symbol) == Some(&projection.declaration)
        });
        let staged_node = staged_matches.next().ok_or_else(|| {
            format!(
                "declaration relabel witness lost staged declaration `{}`",
                projection.declaration.0
            )
        })?;
        if staged_matches.next().is_some() {
            return Err(format!(
                "declaration relabel witness staged declaration `{}` is not unique",
                projection.declaration.0
            ));
        }
        if !witnessed_staged_nodes.insert(staged_node.id) {
            return Err("declaration relabel witness repeats a staged persistent node".into());
        }
        let terminal_owner = terminal_authority
            .ownership
            .node(projection.node)
            .ok_or_else(|| "declaration relabel witness has no terminal native owner".to_owned())?;
        let staged_owner = staged_authority
            .ownership
            .node(staged_node.id)
            .ok_or_else(|| "declaration relabel witness has no staged native owner".to_owned())?;
        if terminal_owner.owned != staged_owner.owned {
            return Err(format!(
                "declaration relabel witness changed native ownership for `{}`: terminal={:?} staged={:?}",
                projection.declaration.0, terminal_owner.owned, staged_owner.owned
            ));
        }
        let terminal_prefix = projection.terminal_symbol.as_str();
        let staged_prefix = staged_node.symbol.as_str();
        for object in terminal_owner
            .owned
            .iter()
            .filter_map(|binding| document_object_for_native_binding(*binding))
        {
            if !witnessed_objects.insert(object) {
                return Err("declaration relabel witness repeats one native object".into());
            }
            let current = document_object_label(terminal_document, object).ok_or_else(|| {
                "declaration relabel witness terminal object disappeared".to_owned()
            })?;
            let replacement = document_object_label(staged_document, object).ok_or_else(|| {
                "declaration relabel witness staged object disappeared".to_owned()
            })?;
            let suffix = current.strip_prefix(terminal_prefix).ok_or_else(|| {
                "declaration-owned native label does not carry the exact terminal symbol prefix"
                    .to_owned()
            })?;
            if !suffix.is_empty() && !suffix.starts_with('.') {
                return Err(
                    "declaration-owned native label has a non-canonical symbol suffix".into(),
                );
            }
            let expected = format!("{staged_prefix}{suffix}");
            if replacement != expected {
                return Err(
                    "declaration-owned native label is not the exact staged alias projection"
                        .into(),
                );
            }
            if current != replacement {
                relabels.push(DocumentObjectRelabel::new(object, current, replacement));
            }
        }
    }
    Ok(relabels)
}

#[cfg(test)]
fn validate_terminal_native_parity(
    terminal: &ProjectionalEditorSession,
    staged: &ProjectionalEditorSession,
    expansion: &ExpandedCodeProject,
    rectangle_projections: &[RectangleTerminalProjection],
) -> Result<(), String> {
    validate_terminal_native_parity_with_trace(
        terminal,
        staged,
        expansion,
        rectangle_projections,
        &[],
        None,
    )
}

#[allow(
    clippy::too_many_lines,
    reason = "one parity gate records every accepted-authority domain before issuing a single rejection"
)]
fn validate_terminal_native_parity_with_trace(
    terminal: &ProjectionalEditorSession,
    staged: &ProjectionalEditorSession,
    expansion: &ExpandedCodeProject,
    rectangle_projections: &[RectangleTerminalProjection],
    declaration_label_projections: &[PreparedDeclarationLabelProjection],
    trace: Option<&mut super::interaction_trace::InteractionTrace>,
) -> Result<(), String> {
    validate_terminal_preview_native_parity_with_trace(
        TerminalPointPreview::from_accepted(terminal)?,
        staged,
        expansion,
        &[],
        rectangle_projections,
        declaration_label_projections,
        trace,
    )
}

#[allow(
    clippy::too_many_lines,
    reason = "one parity gate records every accepted-authority domain before issuing a single rejection"
)]
fn validate_terminal_preview_native_parity_with_trace(
    terminal: TerminalPointPreview<'_>,
    staged: &ProjectionalEditorSession,
    expansion: &ExpandedCodeProject,
    terminal_seed_placements: &[(ExpandedWritablePoint, [f64; 2])],
    rectangle_projections: &[RectangleTerminalProjection],
    declaration_label_projections: &[PreparedDeclarationLabelProjection],
    mut trace: Option<&mut super::interaction_trace::InteractionTrace>,
) -> Result<(), String> {
    let terminal_authority = terminal
        .editor
        .coordinator()
        .accepted_materialization()
        .ok_or_else(|| "terminal code drag has no accepted native authority".to_owned())?;
    let staged_authority = staged
        .coordinator()
        .accepted_materialization()
        .ok_or_else(|| "staged code drag has no accepted native authority".to_owned())?;
    if !accepted_validation_is_publishable(&terminal_authority.validation)
        || !accepted_validation_is_publishable(&staged_authority.validation)
    {
        return Err(
            "terminal parity requires two independently validated current native authorities"
                .into(),
        );
    }
    let terminal_input = terminal
        .session
        .accepted_prepared_input()
        .ok_or_else(|| "terminal code drag has no accepted prepared input".to_owned())?;
    let mut transient_computed = None;
    let terminal_computed = if terminal_authority.computed.input().sketch == terminal_input {
        &terminal_authority.computed
    } else {
        let mut allocator = ComputedEvaluationAllocator::from_high_water(
            terminal_authority.computed_evaluation_high_water,
        );
        let outcome = ComputedFeatureEvaluationSnapshot::capture(
            terminal.session,
            &terminal_authority.features,
            ComputedFeatureEvaluationPolicy::default(),
        )
        .map_err(|error| format!("terminal computed preview capture failed: {error}"))?
        .prepare(&mut allocator)
        .map_err(|error| format!("terminal computed preview preparation failed: {error}"))?
        .execute(OperationControl::unlimited())
        .map_err(|error| format!("terminal computed preview evaluation failed: {error}"))?;
        let OperationOutcome::Completed {
            value: computed, ..
        } = outcome
        else {
            return Err("terminal computed preview evaluation did not complete".into());
        };
        transient_computed.insert(computed)
    };
    if terminal_computed
        .feature_evaluations()
        .iter()
        .any(|evaluation| {
            !matches!(
                evaluation.state,
                ComputedFeatureEvaluationState::Current { .. }
            )
        })
    {
        return Err("terminal computed preview contains a non-current feature".into());
    }
    let mut recomputable =
        recomputable_code_line_branches(terminal.editor, expansion, declaration_label_projections)?;
    recomputable.extend(recomputable_code_line_branches(
        staged,
        expansion,
        declaration_label_projections,
    )?);
    let terminal_accepted = terminal
        .session
        .accepted_state_for_current_input()
        .ok_or_else(|| "terminal code drag has no current accepted document".to_owned())?
        .document();
    let staged_accepted = staged_authority
        .session
        .accepted_state_for_current_input()
        .ok_or_else(|| "staged code drag has no current accepted document".to_owned())?
        .document();
    let terminal_design =
        terminal_seeded_design_document(terminal, terminal_seed_placements, rectangle_projections)?;
    let declaration_object_relabels = authenticated_declaration_object_relabels(
        terminal.editor,
        staged,
        expansion,
        declaration_label_projections,
        &terminal_design,
        staged_authority.session.design_document(),
    )?;
    let design_document_parity = documents_match_for_terminal_parity(
        terminal.editor,
        staged,
        &terminal_design,
        staged_authority.session.design_document(),
        rectangle_projections,
        &recomputable,
        &declaration_object_relabels,
    )?;
    let accepted_document_parity = documents_match_for_terminal_parity(
        terminal.editor,
        staged,
        terminal_accepted,
        staged_accepted,
        rectangle_projections,
        &recomputable,
        &declaration_object_relabels,
    )?;
    let same_documents = design_document_parity.matches()
        && accepted_document_parity.matches()
        && terminal_document_normalizations_match(
            &design_document_parity,
            &accepted_document_parity,
        );
    let computed_policy =
        terminal_computed_roundoff_points(&design_document_parity, &accepted_document_parity)
            .map(|point_scales| {
                terminal_roundoff_sources(staged_accepted, point_scales).map(|source_scales| {
                    TerminalComputedParityPolicy::RectangleAliasRoundoff { source_scales }
                })
            })
            .transpose()?
            .unwrap_or(TerminalComputedParityPolicy::Exact);
    let same_feature_documents = feature_documents_match_for_terminal_parity(
        &terminal_authority.features,
        &staged_authority.features,
        &computed_policy,
    );
    let same_computed_snapshots = computed_snapshots_match_for_terminal_parity(
        terminal_computed,
        &staged_authority.computed,
        &computed_policy,
    );
    let same_features = same_feature_documents && same_computed_snapshots;
    let same_semantic_inputs =
        terminal_semantic_inputs_match(&terminal_authority.session, &staged_authority.session);
    if let Some(trace) = trace.as_deref_mut() {
        trace.record(
            "parity.documents",
            format!(
                "design={design_document_parity:?} accepted={accepted_document_parity:?} recomputable_line_branches={recomputable:?} authenticated_object_relabels={declaration_object_relabels:?}"
            ),
        );
        trace.record("parity.computed.policy", format!("{computed_policy:?}"));
        let periodic_angles = terminal_computed_periodic_angle_evidence(
            terminal_computed,
            &staged_authority.computed,
            &computed_policy,
        );
        if !periodic_angles.is_empty() {
            trace.record(
                "parity.computed.periodic-angle",
                periodic_angles.join(" | "),
            );
        }
        trace.record(
            "parity.features",
            format!(
                "feature_documents={same_feature_documents} computed_snapshot={same_computed_snapshots} terminal_features={:?} staged_features={:?}",
                terminal_authority.features.identity(),
                staged_authority.features.identity(),
            ),
        );
        trace.record(
            "parity.semantic-inputs",
            format!("durable_input_payloads={same_semantic_inputs}"),
        );
        if !same_computed_snapshots {
            trace.record(
                "parity.computed.first-mismatch",
                first_terminal_computed_snapshot_mismatch(
                    terminal_computed,
                    &staged_authority.computed,
                    &computed_policy,
                ),
            );
        }
    }
    // Revision/digest stamps and Fillet pick seeds can refresh when staged
    // source is canonically rematerialized. Discrete topology, durable branch
    // cells, ownership rows and persistent IDs stay exact. Recomputed finite
    // feature scalars receive the same bounded cell only after redundant
    // rectangle aliases demonstrably needed that normalization above.
    let same_ownership = terminal_authority.ownership.nodes == staged_authority.ownership.nodes
        && terminal_authority.ownership.ports == staged_authority.ownership.ports
        && terminal_authority.ownership.reservations == staged_authority.ownership.reservations
        && terminal_authority.ownership.writable_leaves
            == staged_authority.ownership.writable_leaves
        && terminal_authority.ownership.aggregates == staged_authority.ownership.aggregates;
    if let Some(trace) = trace.as_deref_mut() {
        trace.record(
            "parity.ownership",
            format!(
                "nodes={} ports={} reservations={} writable_leaves={} aggregates={}",
                terminal_authority.ownership.nodes == staged_authority.ownership.nodes,
                terminal_authority.ownership.ports == staged_authority.ownership.ports,
                terminal_authority.ownership.reservations
                    == staged_authority.ownership.reservations,
                terminal_authority.ownership.writable_leaves
                    == staged_authority.ownership.writable_leaves,
                terminal_authority.ownership.aggregates == staged_authority.ownership.aggregates,
            ),
        );
        trace.record(
            "parity.allocators",
            format!(
                "feature_terminal={:?} feature_staged={:?} sketch_terminal={:?} sketch_staged={:?}",
                terminal_authority.feature_lifecycle_high_water.allocator,
                staged_authority.feature_lifecycle_high_water.allocator,
                terminal.session.persistent_identity_high_water(),
                staged_authority.session.persistent_identity_high_water(),
            ),
        );
    }
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
    if !same_semantic_inputs {
        differences.push("native semantic inputs");
    }
    if terminal_authority.feature_lifecycle_high_water.allocator
        != staged_authority.feature_lifecycle_high_water.allocator
    {
        differences.push("feature allocator");
    }
    if terminal.session.persistent_identity_high_water()
        != staged_authority.session.persistent_identity_high_water()
    {
        differences.push("sketch allocator");
    }
    if !differences.is_empty() {
        let mut error = format!(
            "terminal code drag differs from its independently staged native authority in {}",
            differences.join(", ")
        );
        if !same_feature_documents {
            let detail = first_terminal_feature_document_mismatch(
                &terminal_authority.features,
                &staged_authority.features,
                &computed_policy,
            );
            let _ = write!(error, "; {detail}");
        } else if !same_computed_snapshots {
            let detail = first_terminal_computed_snapshot_mismatch(
                terminal_computed,
                &staged_authority.computed,
                &computed_policy,
            );
            let _ = write!(error, "; {detail}");
        }
        if let Some(trace) = trace.as_deref_mut() {
            trace.record("parity.reject", &error);
        }
        return Err(error);
    }
    if let Some(trace) = trace {
        trace.record("parity.accept", "all terminal authority domains match");
    }
    Ok(())
}

fn recomputable_code_line_branches(
    editor: &ProjectionalEditorSession,
    expansion: &ExpandedCodeProject,
    declaration_label_projections: &[PreparedDeclarationLabelProjection],
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
        // During canvas-to-source publication, the terminal checkpoint still
        // carries the GUI-authored symbol while the independently staged
        // source expansion carries `port.alias`. The Rust-only projection
        // binds both spellings to the same persistent node and declaration;
        // use it only for that authenticated transition. Ordinary code drags
        // have no projection and continue to require the exact expansion
        // alias.
        let node = intent.graph().node_by_symbol(&port.alias).or_else(|| {
            declaration_label_projections
                .iter()
                .find(|projection| projection.declaration == provenance.declaration)
                .and_then(|projection| {
                    let node = intent.graph().node(projection.node)?;
                    (node.symbol == projection.terminal_symbol
                        || expansion.declaration_for_alias(&node.symbol)
                            == Some(&projection.declaration))
                    .then_some(node)
                })
        });
        let node = node
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

fn write_managed_control_input(markup: &mut String, control: &ManagedControl, path: &str) {
    let Some(schema) = control.schema.as_ref() else {
        return;
    };
    let id = escape_attribute(&control.id.0);
    let label = escape_attribute(&format!("Edit managed {path}"));
    match (schema, &control.value) {
        (
            ManagedControlSchema::Number {
                number,
                minimum,
                maximum,
            },
            ManagedValue::Number(value),
        ) => {
            let step = if *number == geosolve_sketch_code::ManagedControlNumberKind::Real {
                "any"
            } else {
                "1"
            };
            let _ = write!(
                markup,
                "<label class=\"wb-code-lens-control\"><span class=\"wb-sr-only\">{label}</span><input type=\"number\" step=\"{step}\" value=\"{value}\" data-code-control-id=\"{id}\"",
            );
            write_managed_control_bounds(markup, *minimum, *maximum);
            markup.push_str(" /><small>value</small></label>");
        }
        (
            ManagedControlSchema::Unit {
                unit,
                number,
                minimum,
                maximum,
            },
            ManagedValue::Unit(value),
        ) if value.unit == *unit => {
            let step = if *number == geosolve_sketch_code::ManagedControlNumberKind::Real {
                "any"
            } else {
                "1"
            };
            let _ = write!(
                markup,
                "<label class=\"wb-code-lens-control\"><span class=\"wb-sr-only\">{label}</span><input type=\"number\" step=\"{step}\" value=\"{}\" data-code-control-id=\"{id}\"",
                value.value,
            );
            write_managed_control_bounds(markup, *minimum, *maximum);
            let _ = write!(markup, " /><small>{}</small></label>", escape_html(unit));
        }
        (ManagedControlSchema::Boolean, ManagedValue::Bool(value)) => {
            let checked = if *value { " checked" } else { "" };
            let _ = write!(
                markup,
                "<label class=\"wb-code-lens-control\"><span>{}</span><input type=\"checkbox\" data-code-control-id=\"{id}\"{checked} /></label>",
                escape_html(path),
            );
        }
        (ManagedControlSchema::Choice { choices }, ManagedValue::String(value)) => {
            let _ = write!(
                markup,
                "<label class=\"wb-code-lens-control\"><span class=\"wb-sr-only\">{label}</span><select data-code-control-id=\"{id}\">",
            );
            for choice in choices {
                let selected = if choice == value { " selected" } else { "" };
                let _ = write!(
                    markup,
                    "<option value=\"{}\"{selected}>{}</option>",
                    escape_attribute(choice),
                    escape_html(choice),
                );
            }
            markup.push_str("</select></label>");
        }
        (ManagedControlSchema::Text, ManagedValue::String(value)) => {
            let _ = write!(
                markup,
                "<label class=\"wb-code-lens-control\"><span class=\"wb-sr-only\">{label}</span><input type=\"text\" value=\"{}\" data-code-control-id=\"{id}\" /></label>",
                escape_attribute(value),
            );
        }
        _ => {
            markup
                .push_str("<span class=\"wb-code-readonly\">Control schema/source mismatch</span>");
        }
    }
}

fn write_managed_control_bounds(
    markup: &mut String,
    minimum: Option<geosolve_sketch_code::ManagedControlBound>,
    maximum: Option<geosolve_sketch_code::ManagedControlBound>,
) {
    if let Some(minimum) = minimum.filter(|bound| bound.inclusive) {
        let _ = write!(markup, " min=\"{}\"", minimum.value);
    }
    if let Some(maximum) = maximum.filter(|bound| bound.inclusive) {
        let _ = write!(markup, " max=\"{}\"", maximum.value);
    }
}

fn semantic_output_path(path: &geosolve_sketch_intent::IntentProjectionPath) -> SemanticOutputPath {
    SemanticOutputPath(
        path.segments()
            .iter()
            .map(|segment| match segment {
                geosolve_sketch_intent::IntentProjectionPathSegment::Field(field) => {
                    ManagedPathSegment::Field(field.as_str().to_owned())
                }
                geosolve_sketch_intent::IntentProjectionPathSegment::Index(index) => {
                    ManagedPathSegment::Index(usize::from(*index))
                }
            })
            .collect(),
    )
}

fn managed_value_from_submission(
    control: &ManagedControl,
    submission: ManagedControlSubmission,
) -> Result<ManagedValue, String> {
    let schema = control
        .schema
        .as_ref()
        .ok_or_else(|| "the selected managed control is read-only".to_owned())?;
    match (schema, &control.value, submission) {
        (
            ManagedControlSchema::Number { .. },
            ManagedValue::Number(_),
            ManagedControlSubmission::Number(value),
        ) if value.is_finite() => Ok(ManagedValue::Number(value)),
        (
            ManagedControlSchema::Unit { unit, .. },
            ManagedValue::Unit(current),
            ManagedControlSubmission::Number(value),
        ) if current.unit == *unit && value.is_finite() => Ok(ManagedValue::Unit(UnitLiteral {
            unit: current.unit.clone(),
            value,
        })),
        (
            ManagedControlSchema::Boolean,
            ManagedValue::Bool(_),
            ManagedControlSubmission::Boolean(value),
        ) => Ok(ManagedValue::Bool(value)),
        (
            ManagedControlSchema::Choice { .. } | ManagedControlSchema::Text,
            ManagedValue::String(_),
            ManagedControlSubmission::String(value),
        ) => Ok(ManagedValue::String(value)),
        (
            ManagedControlSchema::Number { .. } | ManagedControlSchema::Unit { .. },
            _,
            ManagedControlSubmission::Number(value),
        ) if !value.is_finite() => Err("managed-control number must be finite".into()),
        _ => Err(
            "managed-control submission does not match its fresh source representation and schema"
                .into(),
        ),
    }
}

fn managed_values_exactly_equal(left: &ManagedValue, right: &ManagedValue) -> bool {
    match (left, right) {
        (ManagedValue::Null, ManagedValue::Null) => true,
        (ManagedValue::Bool(left), ManagedValue::Bool(right)) => left == right,
        (ManagedValue::Number(left), ManagedValue::Number(right)) => {
            left.to_bits() == right.to_bits()
        }
        (ManagedValue::String(left), ManagedValue::String(right)) => left == right,
        (ManagedValue::Unit(left), ManagedValue::Unit(right)) => {
            left.unit == right.unit && left.value.to_bits() == right.value.to_bits()
        }
        (ManagedValue::Array(left), ManagedValue::Array(right)) => {
            left.len() == right.len()
                && left
                    .iter()
                    .zip(right)
                    .all(|(left, right)| managed_values_exactly_equal(left, right))
        }
        (ManagedValue::Object(left), ManagedValue::Object(right)) => {
            left.len() == right.len()
                && left.iter().all(|(key, left)| {
                    right
                        .get(key)
                        .is_some_and(|right| managed_values_exactly_equal(left, right))
                })
        }
        (
            ManagedValue::Reference {
                declaration: left_declaration,
                path: left_path,
            },
            ManagedValue::Reference {
                declaration: right_declaration,
                path: right_path,
            },
        ) => left_declaration == right_declaration && left_path == right_path,
        _ => false,
    }
}

fn managed_numeric_value(value: &ManagedValue) -> Option<f64> {
    match value {
        ManagedValue::Number(value) if value.is_finite() => Some(*value),
        ManagedValue::Unit(value) if value.value.is_finite() => Some(value.value),
        _ => None,
    }
}

fn managed_fillet_radius_value_kind(
    control: &ManagedControl,
) -> Result<ManagedFilletRadiusValueKind, String> {
    match (control.schema.as_ref(), &control.value) {
        (Some(ManagedControlSchema::Number { .. }), ManagedValue::Number(_)) => {
            Ok(ManagedFilletRadiusValueKind::Number)
        }
        (Some(ManagedControlSchema::Unit { unit, .. }), ManagedValue::Unit(value))
            if value.unit == *unit =>
        {
            Ok(ManagedFilletRadiusValueKind::Unit(value.unit.clone()))
        }
        _ => Err("managed Fillet radius has an incompatible source schema".into()),
    }
}

fn managed_path_text(path: &[ManagedPathSegment]) -> String {
    let mut text = String::new();
    for segment in path {
        match segment {
            ManagedPathSegment::Field(field) => {
                if !text.is_empty() {
                    text.push('.');
                }
                text.push_str(field);
            }
            ManagedPathSegment::Index(index) => {
                let _ = write!(text, "[{index}]");
            }
            ManagedPathSegment::Member { member } => {
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

fn managed_source_path(control: &ManagedControl) -> String {
    let path = managed_path_text(&control.source.path.0);
    if control.source.path.0.is_empty() {
        control.source.declaration.0.clone()
    } else {
        format!("{}.{}", control.source.declaration.0, path)
    }
}

fn managed_navigation_path(navigation: &geosolve_sketch_code::ManagedControlNavigation) -> String {
    if navigation.path.0.is_empty() {
        navigation.declaration.0.clone()
    } else {
        format!(
            "{}.{}",
            navigation.declaration.0,
            managed_path_text(&navigation.path.0),
        )
    }
}

fn managed_read_only_reason(
    reason: ManagedControlReadOnlyReason,
    navigation: Option<&geosolve_sketch_code::ManagedControlNavigation>,
) -> String {
    let reason = match reason {
        ManagedControlReadOnlyReason::Structure => "Structural source value",
        ManagedControlReadOnlyReason::Reference => "Source reference",
        ManagedControlReadOnlyReason::StructuralIdentity => "Structural identity",
        ManagedControlReadOnlyReason::SolverInstance => "Solver-owned instance value",
        ManagedControlReadOnlyReason::Null => "Null source value",
        ManagedControlReadOnlyReason::Absent => "Absent source value",
        ManagedControlReadOnlyReason::IncompatibleSchemas => "Incompatible consumer schemas",
        ManagedControlReadOnlyReason::UnprovenTransform => "Unproven source transform",
    };
    navigation.map_or_else(
        || format!("{reason} · no directly writable sketch.ts property"),
        |navigation| {
            format!(
                "{reason} · navigate to {} in sketch.ts",
                managed_navigation_path(navigation),
            )
        },
    )
}

fn managed_family_label(family: &str) -> String {
    let leaf = family.rsplit('.').next().unwrap_or(family);
    let mut characters = leaf.chars();
    characters.next().map_or_else(
        || "native".into(),
        |first| {
            let mut label = first.to_uppercase().collect::<String>();
            label.extend(characters);
            label
        },
    )
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
    fn canonical_export_is_exact_and_refuses_absent_dirty_or_invalid_draft_authority() {
        assert_eq!(
            canonical_code_project_files(None),
            Err(CanonicalCodeProjectExportError::NoProject),
        );

        let (mut workbench, _editor) = CodeProjectWorkbench::new_authored().unwrap();
        let clean = canonical_code_project_files(Some(&workbench)).unwrap();
        assert_eq!(
            clean.project_json,
            workbench.project.to_canonical_json().unwrap(),
        );
        assert_eq!(clean.managed_source, workbench.project.managed.source);
        assert_eq!(
            CodeProject::from_json(&clean.project_json).unwrap(),
            workbench.project,
        );

        let valid_dirty = format!("{}\n", workbench.managed_source());
        workbench.set_managed_draft(valid_dirty.clone());
        assert_eq!(workbench.raw_managed_draft_file().source, valid_dirty);
        assert_eq!(
            canonical_code_project_files(Some(&workbench)),
            Err(CanonicalCodeProjectExportError::DirtyDraft),
        );

        let invalid = "export default notASketch();".to_owned();
        workbench
            .retain_managed_compiler_draft(
                invalid.clone(),
                "compiler rejected the source".into(),
                ManagedSpan::new(0, invalid.len()),
            )
            .unwrap();
        assert_eq!(workbench.raw_managed_draft_file().source, invalid);
        let Err(CanonicalCodeProjectExportError::InvalidDraft { diagnostic }) =
            canonical_code_project_files(Some(&workbench))
        else {
            panic!("invalid draft must have one typed source-positioned refusal")
        };
        assert!(diagnostic.line >= 1);
        assert!(diagnostic.column >= 1);
        assert!(!diagnostic.message.is_empty());
    }

    #[test]
    fn compiler_rejected_draft_and_position_round_trip_while_tampering_rejects() {
        let (mut workbench, _editor) = CodeProjectWorkbench::new_authored().unwrap();
        let accepted_source = workbench.managed_source().to_owned();
        let candidate_source = format!("// 多字节\n{accepted_source}");
        let start = candidate_source.find('多').unwrap();
        let span = ManagedSpan::new(start, start + "多字节".len());
        workbench
            .retain_managed_compiler_draft(
                candidate_source.clone(),
                "compiler rejected the Unicode candidate".into(),
                span,
            )
            .unwrap();
        assert!(workbench.is_dirty());
        assert!(matches!(
            canonical_code_project_files(Some(&workbench)),
            Err(CanonicalCodeProjectExportError::InvalidDraft { .. }),
        ));
        let diagnostic = workbench.draft_diagnostic().unwrap();
        assert_eq!((diagnostic.line, diagnostic.column), (1, 4));

        let persisted = workbench.to_persistence_json().unwrap();
        let restored = CodeProjectWorkbench::from_persistence_json(&persisted).unwrap();
        assert_eq!(restored.managed_draft(), candidate_source);
        assert_eq!(restored.draft_diagnostic(), Some(diagnostic));
        assert_eq!(restored.to_persistence_json().unwrap(), persisted);
        assert!(matches!(
            canonical_code_project_files(Some(&restored)),
            Err(CanonicalCodeProjectExportError::InvalidDraft { .. }),
        ));

        let mut stale_line: serde_json::Value = serde_json::from_str(&persisted).unwrap();
        stale_line["draft_diagnostic"]["line"] = serde_json::json!(99);
        assert!(
            CodeProjectWorkbench::from_persistence_json(&stale_line.to_string())
                .err()
                .unwrap()
                .contains("stale line or column")
        );

        let mut split_utf8: serde_json::Value = serde_json::from_str(&persisted).unwrap();
        split_utf8["draft_diagnostic"]["span"]["end"] = serde_json::json!(start + 1);
        assert!(
            CodeProjectWorkbench::from_persistence_json(&split_utf8.to_string())
                .err()
                .unwrap()
                .contains("splits a UTF-8 code point")
        );

        let (mut fallback, _editor) = CodeProjectWorkbench::new_authored().unwrap();
        let unchanged_candidate = fallback.managed_source().to_owned();
        fallback
            .retain_managed_compiler_draft(
                unchanged_candidate,
                "compiler rejected before printing a candidate".into(),
                ManagedSpan::new(0, 0),
            )
            .unwrap();
        assert!(fallback.is_dirty(), "the correction UI must expose Revert");
        let fallback_persisted = fallback.to_persistence_json().unwrap();
        let mut fallback =
            CodeProjectWorkbench::from_persistence_json(&fallback_persisted).unwrap();
        assert!(fallback.draft_diagnostic().is_some());
        assert!(matches!(
            canonical_code_project_files(Some(&fallback)),
            Err(CanonicalCodeProjectExportError::InvalidDraft { .. }),
        ));
        assert!(fallback.revert_managed_draft());
        assert!(!fallback.is_dirty());
        canonical_code_project_files(Some(&fallback)).unwrap();
    }

    #[test]
    fn canonical_import_returns_only_a_fully_materialized_atomic_replacement_pair() {
        let (live, _live_editor) = CodeProjectWorkbench::new_authored().unwrap();
        let live_before = live.to_persistence_json().unwrap();
        let files = canonical_code_project_files(Some(&live)).unwrap();
        let controls_before = live.managed_controls().unwrap();

        let (imported, imported_editor) =
            CodeProjectWorkbench::import_canonical_project_json(&files.project_json).unwrap();
        assert_eq!(
            canonical_code_project_files(Some(&imported)).unwrap(),
            files,
        );
        let controls_after = imported.managed_controls().unwrap();
        assert_eq!(controls_after.project, controls_before.project);
        assert_eq!(
            controls_after.project_digest,
            controls_before.project_digest
        );
        assert_eq!(controls_after.source_digest, controls_before.source_digest);
        assert_eq!(
            controls_after
                .controls
                .iter()
                .map(|control| (
                    &control.id,
                    &control.source,
                    &control.value,
                    &control.schema
                ))
                .collect::<Vec<_>>(),
            controls_before
                .controls
                .iter()
                .map(|control| (
                    &control.id,
                    &control.source,
                    &control.value,
                    &control.schema
                ))
                .collect::<Vec<_>>(),
            "canonical import must preserve stable control IDs and exact source ownership",
        );
        let accepted = imported_editor
            .coordinator()
            .accepted_materialization()
            .expect("successful import has one accepted native scene");
        assert!(accepted_validation_is_publishable(&accepted.validation));
        assert_eq!(
            encode_editor_checkpoint(&imported_editor).unwrap(),
            *imported.accepted_editor_checkpoint(),
        );

        assert!(matches!(
            CodeProjectWorkbench::import_canonical_project_json("{not json"),
            Err(CanonicalCodeProjectImportError::InvalidProject(_)),
        ));
        let oversized = " ".repeat(geosolve_sketch_code::CODE_PROJECT_LIMIT + 1);
        assert!(matches!(
            CodeProjectWorkbench::import_canonical_project_json(&oversized),
            Err(CanonicalCodeProjectImportError::InvalidProject(_)),
        ));
        drop(oversized);
        let mut invalid_project = live.project.clone();
        invalid_project.managed.source_digest = "forged-source-digest".into();
        let invalid_project = serde_json::to_string(&invalid_project).unwrap();
        assert!(matches!(
            CodeProjectWorkbench::import_canonical_project_json(&invalid_project),
            Err(CanonicalCodeProjectImportError::InvalidProject(_)),
        ));
        assert_eq!(
            live.to_persistence_json().unwrap(),
            live_before,
            "failed candidate construction cannot mutate the live project/editor pair",
        );
    }

    #[test]
    fn rectangle_terminal_roundoff_contract_is_tight_and_signed_zero_exact() {
        let seed = 1.0_f64;
        let within = f64::from_bits(seed.to_bits() + TERMINAL_SEED_ROUNDOFF_ULPS);
        let outside = f64::from_bits(seed.to_bits() + TERMINAL_SEED_ROUNDOFF_ULPS + 1);
        assert!(scalar_seed_roundoff_compatible(seed, seed, 1.0));
        assert!(scalar_seed_roundoff_compatible(seed, within, 1.0));
        assert!(!scalar_seed_roundoff_compatible(seed, outside, 1.0));
        assert!(scalar_seed_roundoff_compatible(
            3.0 * f64::EPSILON,
            -2.0 * f64::EPSILON,
            1.0,
        ));
        assert!(!scalar_seed_roundoff_compatible(
            2.0 * TERMINAL_SEED_ZERO_ROUNDOFF,
            0.0,
            1.0,
        ));
        assert!(!scalar_seed_roundoff_compatible(0.0, -0.0, 1.0));
        assert!(!scalar_seed_roundoff_compatible(f64::NAN, f64::NAN, 1.0));
        assert!(!scalar_seed_roundoff_compatible(
            f64::INFINITY,
            f64::INFINITY,
            1.0,
        ));

        assert!(terminal_derived_scalar_matches(seed, within, 1.0));
        assert!(!terminal_derived_scalar_matches(seed, outside, 1.0));
        assert!(terminal_derived_scalar_matches(0.0, -0.0, 1.0));
        assert!(terminal_derived_scalar_matches(
            3.0 * f64::EPSILON,
            -2.0 * f64::EPSILON,
            1.0,
        ));
        assert!(!terminal_derived_scalar_matches(
            2.0 * TERMINAL_SEED_ZERO_ROUNDOFF,
            0.0,
            1.0,
        ));
        assert!(!terminal_derived_scalar_matches(f64::NAN, f64::NAN, 1.0));
        assert!(!terminal_derived_scalar_matches(
            f64::INFINITY,
            f64::INFINITY,
            1.0,
        ));

        let browser_terminal = 1.743_119_266_055_046_5_f64;
        let projected_alias = f64::from_bits(browser_terminal.to_bits() + 14);
        assert!(scalar_seed_roundoff_compatible(
            browser_terminal,
            projected_alias,
            80.0,
        ));
        assert!(terminal_derived_scalar_matches(
            browser_terminal,
            projected_alias,
            80.0,
        ));
        assert!(!scalar_seed_roundoff_compatible(
            browser_terminal,
            browser_terminal + 1.0e-10,
            80.0,
        ));
    }

    fn typed_panel_fillet_edge_and_sources() -> (
        geosolve_constraint_editor::ComputedEdge,
        TerminalRoundoffSourceScales,
    ) {
        let (_workbench, editor) = open_boxed("typed-panel");
        let edge = editor
            .coordinator()
            .accepted_materialization()
            .expect("Typed Panel accepted materialization")
            .computed
            .edges()
            .iter()
            .find(|edge| matches!(edge.geometry, ComputedEdgeGeometry::CircularArc(_)))
            .expect("Typed Panel computed Fillet arc")
            .clone();
        let ComputedEdgeGeometry::CircularArc(reference_arc) = &edge.geometry else {
            panic!("selected edge must remain a circular arc")
        };
        let source_scales = reference_arc
            .contacts
            .iter()
            .map(|contact| (contact.source, 1.0_f64.to_bits()))
            .collect();
        (edge, source_scales)
    }

    #[test]
    fn rectangle_terminal_periodic_arc_angle_roundoff_is_causal_and_bounded() {
        let (mut terminal, source_scales) = typed_panel_fillet_edge_and_sources();
        let mut staged = terminal.clone();
        let ComputedEdgeGeometry::CircularArc(terminal_arc) = &mut terminal.geometry else {
            panic!("selected edge must remain a circular arc")
        };
        let ComputedEdgeGeometry::CircularArc(staged_arc) = &mut staged.geometry else {
            panic!("selected edge must remain a circular arc")
        };
        terminal_arc.start_angle = -std::f64::consts::PI;
        staged_arc.start_angle = std::f64::consts::PI;

        assert!(
            terminal_computed_edge_matches(&terminal, &staged, &source_scales),
            "the authenticated computed arc must treat -PI/+PI as one periodic direction",
        );
        assert!(
            !terminal_computed_edge_matches(
                &terminal,
                &staged,
                &TerminalRoundoffSourceScales::new(),
            ),
            "an empty or unrelated policy must keep the same arc angles bit-exact",
        );
        assert!(!terminal_periodic_angle_matches_trace_policy(
            -std::f64::consts::PI,
            std::f64::consts::PI,
            TerminalScalarParityPolicy::Exact,
        ));
        assert!(terminal_periodic_angle_matches_trace_policy(
            -std::f64::consts::PI,
            std::f64::consts::PI,
            TerminalScalarParityPolicy::Roundoff {
                coordinate_scale: 1.0,
            },
        ));

        let ComputedEdgeGeometry::CircularArc(staged_arc) = &mut staged.geometry else {
            panic!("selected edge must remain a circular arc")
        };
        staged_arc.start_angle = std::f64::consts::PI - 1.0e-6;
        assert!(
            !terminal_computed_edge_matches(&terminal, &staged, &source_scales),
            "a genuinely different direction must not enter the periodic seam cell",
        );
        let policy = TerminalComputedParityPolicy::RectangleAliasRoundoff { source_scales };
        let edge_index = usize::try_from(terminal.id.ordinal).expect("bounded computed edge");
        let mismatch =
            first_terminal_computed_edge_mismatch(edge_index, &terminal, &staged, &policy);
        assert!(mismatch.starts_with(&format!("path=edge[{edge_index}].arc.start_angle ")));
        assert!(mismatch.contains("periodic_policy=one-turn"));
        assert!(mismatch.contains("periodic_turn=Some(-1)"));
        assert!(mismatch.contains("unwrapped_staged=Some("));
        assert!(mismatch.contains("unwrapped_residual=Some("));
        assert!(mismatch.contains("unwrapped_tolerance=Some("));
        assert!(!mismatch.contains("unknown_mismatch"));
    }

    #[test]
    fn rectangle_roundoff_keeps_the_encoded_fillet_radius_bit_exact() {
        let (edge, source_scales) = typed_panel_fillet_edge_and_sources();
        let mut changed = edge.clone();
        let ComputedEdgeGeometry::CircularArc(arc) = &mut changed.geometry else {
            panic!("selected edge must remain a circular arc")
        };
        arc.radius = f64::from_bits(arc.radius.to_bits() + 1);
        assert!(
            !terminal_computed_edge_matches(&edge, &changed, &source_scales),
            "the feature-owned radius is copied input, not causal rectangle roundoff",
        );
        let policy = TerminalComputedParityPolicy::RectangleAliasRoundoff { source_scales };
        let mismatch = first_terminal_computed_edge_mismatch(
            usize::try_from(edge.id.ordinal).expect("bounded computed edge"),
            &edge,
            &changed,
            &policy,
        );
        assert!(mismatch.contains(".arc.radius "));
        assert!(mismatch.contains("coordinate_scale=None"));
    }

    #[test]
    fn rectangle_roundoff_requires_internal_geometry_provenance_source_parity() {
        let (terminal, source_scales) = typed_panel_fillet_edge_and_sources();
        let mut inconsistent_terminal = terminal.clone();
        let mut inconsistent_staged = terminal.clone();
        for edge in [&mut inconsistent_terminal, &mut inconsistent_staged] {
            let ComputedEdgeProvenance::FilletArc { sources, .. } = &mut edge.provenance else {
                panic!("selected edge must retain Fillet provenance")
            };
            sources.swap(0, 1);
        }
        let ComputedEdgeGeometry::CircularArc(arc) = &mut inconsistent_staged.geometry else {
            panic!("selected edge must remain a circular arc")
        };
        arc.center[0] = f64::from_bits(arc.center[0].to_bits() + 1);
        assert_eq!(
            terminal_computed_edge_roundoff_scale(
                &inconsistent_terminal,
                &inconsistent_staged,
                &source_scales,
            ),
            None,
        );
        assert!(!terminal_computed_edge_matches(
            &inconsistent_terminal,
            &inconsistent_staged,
            &source_scales,
        ));
    }

    #[test]
    fn unrelated_large_rectangle_scale_does_not_inflate_a_computed_edge() {
        let (edge, mut source_scales) = typed_panel_fillet_edge_and_sources();
        let (_workbench, editor) = open_boxed("typed-panel");
        let unrelated_curve = editor
            .coordinator()
            .accepted_materialization()
            .expect("Typed Panel accepted materialization")
            .session
            .design_document()
            .curves()
            .iter()
            .map(|curve| NativeCurveSpanSource {
                span: CurveSpan {
                    curve: curve.id,
                    segment: 0,
                },
            })
            .find(|source| !source_scales.contains_key(source))
            .expect("Typed Panel has an unrelated rectangle source curve");
        source_scales.insert(unrelated_curve, 1.0e12_f64.to_bits());

        let mut material_difference = edge.clone();
        let ComputedEdgeGeometry::CircularArc(arc) = &mut material_difference.geometry else {
            panic!("selected edge must remain a circular arc")
        };
        arc.center[0] += 1.0e-8;
        assert_eq!(
            terminal_computed_edge_roundoff_scale(&edge, &material_difference, &source_scales),
            Some(1.0),
        );
        assert!(
            !terminal_computed_edge_matches(&edge, &material_difference, &source_scales),
            "an unrelated large rectangle must not loosen this edge's local roundoff cell",
        );
    }

    #[test]
    fn rectangle_roundoff_marks_only_incident_polyline_spans() {
        let mut document = geosolve_sketch::SketchDocument::new(1.0).unwrap();
        let points = [[0.0, 0.0], [1.0, 0.0], [2.0, 0.0], [3.0, 0.0]]
            .map(|position| document.add_point("polyline control", position).unwrap());
        let curve = document
            .add_curve(
                "polyline",
                geosolve_sketch::CurveDefinition::Polyline {
                    points: points.to_vec(),
                    closed: false,
                    branch_directions: vec![[1.0, 0.0]; 3],
                },
            )
            .unwrap();
        let sources =
            terminal_roundoff_sources(&document, &BTreeMap::from([(points[0], 1.0_f64.to_bits())]))
                .expect("valid polyline roundoff sources");
        let source = |segment| NativeCurveSpanSource {
            span: CurveSpan { curve, segment },
        };
        assert_eq!(
            sources.keys().copied().collect::<BTreeSet<_>>(),
            BTreeSet::from([source(0)]),
        );
        assert!(!sources.contains_key(&source(1)));
        assert!(!sources.contains_key(&source(2)));
    }

    #[test]
    fn rectangle_roundoff_marks_only_locally_supported_spline_spans() {
        let mut document = geosolve_sketch::SketchDocument::new(1.0).unwrap();
        let clamped_controls = (0..7)
            .map(|index| {
                document
                    .add_point("clamped control", [f64::from(index), 0.0])
                    .unwrap()
            })
            .collect::<Vec<_>>();
        let clamped_spans = [101, 103, 107, 109];
        let clamped = document
            .add_curve(
                "clamped cubic",
                geosolve_sketch::CurveDefinition::BSpline {
                    form: geosolve_sketch::DocumentBSplineForm::Clamped,
                    degree: 3,
                    controls: clamped_controls.clone(),
                    knots: vec![0.0, 0.0, 0.0, 0.0, 0.2, 0.55, 0.8, 1.0, 1.0, 1.0, 1.0],
                    span_ids: clamped_spans.to_vec(),
                    next_span_id: 110,
                },
            )
            .unwrap();

        let periodic_controls = (0..5)
            .map(|index| {
                document
                    .add_point("periodic control", [f64::from(index), 10.0])
                    .unwrap()
            })
            .collect::<Vec<_>>();
        let periodic_spans = [11, 17, 23, 29, 31];
        let periodic = document
            .add_curve(
                "periodic quadratic",
                geosolve_sketch::CurveDefinition::BSpline {
                    form: geosolve_sketch::DocumentBSplineForm::Periodic,
                    degree: 2,
                    controls: periodic_controls.clone(),
                    knots: vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0],
                    span_ids: periodic_spans.to_vec(),
                    next_span_id: 32,
                },
            )
            .unwrap();
        let sources = terminal_roundoff_sources(
            &document,
            &BTreeMap::from([
                (clamped_controls[0], 1.0_f64.to_bits()),
                (periodic_controls[0], 2.0_f64.to_bits()),
            ]),
        )
        .expect("valid local spline roundoff sources");
        let source = |curve, segment| NativeCurveSpanSource {
            span: CurveSpan { curve, segment },
        };

        assert!(sources.contains_key(&source(clamped, clamped_spans[0])));
        for segment in &clamped_spans[1..] {
            assert!(
                !sources.contains_key(&source(clamped, *segment)),
                "a distant clamped span must remain exact",
            );
        }
        for segment in [periodic_spans[0], periodic_spans[3], periodic_spans[4]] {
            assert!(
                sources.contains_key(&source(periodic, segment)),
                "periodic wraparound support must remain causal",
            );
        }
        for segment in [periodic_spans[1], periodic_spans[2]] {
            assert!(
                !sources.contains_key(&source(periodic, segment)),
                "a distant periodic span must remain exact",
            );
        }
    }

    #[test]
    fn terminal_evaluation_mismatch_trace_ignores_refreshable_edge_revisions() {
        let (_workbench, editor) = open_boxed("typed-panel");
        let evaluation = editor
            .coordinator()
            .accepted_materialization()
            .expect("Typed Panel accepted materialization")
            .computed
            .feature_evaluations()
            .first()
            .expect("Typed Panel computed feature evaluation")
            .clone();
        let mut refreshed = evaluation.clone();
        {
            let ComputedFeatureEvaluationState::Current { corner_edges } = &mut refreshed.state
            else {
                panic!("Typed Panel evaluation must remain Current")
            };
            let (_, edge) = corner_edges
                .first_mut()
                .expect("Typed Panel Current evaluation edge");
            edge.evaluation = geosolve_constraint_editor::ComputedEvaluationRevision::from_raw(
                edge.evaluation.raw().saturating_add(1),
            );
        }
        assert_ne!(evaluation, refreshed);
        assert!(
            terminal_feature_evaluation_matches(&evaluation, &refreshed),
            "evaluation-local edge revisions are deliberately outside terminal parity",
        );

        let ComputedFeatureEvaluationState::Current { corner_edges } = &mut refreshed.state else {
            panic!("Typed Panel evaluation must remain Current")
        };
        let (_, edge) = corner_edges
            .first_mut()
            .expect("Typed Panel Current evaluation edge");
        edge.ordinal = edge.ordinal.saturating_add(1);
        assert!(!terminal_feature_evaluation_matches(
            &evaluation,
            &refreshed,
        ));
        assert!(
            first_terminal_feature_evaluation_mismatch(0, &evaluation, &refreshed)
                .contains(".edge.ordinal "),
        );
    }

    #[test]
    fn validate_terminal_native_parity_trace_rejects_a_computed_scalar_mismatch() {
        let (workbench, mut terminal) = open_boxed("typed-panel");
        let expansion = workbench
            .session
            .snapshot()
            .accepted_expansion
            .clone()
            .expect("accepted Typed Panel expansion");
        let staged = terminal
            .fork_accepted_authority()
            .expect("independent staged Typed Panel authority");
        let (feature, radius) = {
            let accepted = terminal
                .coordinator()
                .accepted_materialization()
                .expect("Typed Panel accepted materialization");
            let feature = accepted
                .features
                .features()
                .first()
                .expect("Typed Panel computed Fillet feature");
            let ComputedFeatureDefinition::FilletSet(fillet) = &feature.definition;
            (feature.id, fillet.radius)
        };
        let changed_radius = f64::from_bits(radius.to_bits() + TERMINAL_SEED_ROUNDOFF_ULPS + 1);
        terminal
            .edit_computed_fillet_radius(feature, changed_radius)
            .expect("finite nearby radius remains an accepted native authority");

        let mut trace = super::super::interaction_trace::InteractionTrace::default();
        trace.begin_gesture(
            "test.pointerdown",
            "project=typed-panel scalar=fillet.radius",
        );
        let error = validate_terminal_native_parity_with_trace(
            &terminal,
            &staged,
            &expansion,
            &[],
            &[],
            Some(&mut trace),
        )
        .expect_err("changed computed radius must reject terminal/native parity");
        assert!(error.contains("computed features"));

        let exported = trace.export("project=typed-panel", "accepted authority retained");
        let mismatch_row = exported
            .lines()
            .find(|line| line.contains("\tparity.computed.first-mismatch\t"))
            .unwrap_or_else(|| panic!("trace must name its first computed mismatch: {exported}"));
        assert!(mismatch_row.contains("path=edge["));
        assert!(mismatch_row.contains(".arc."));
        assert!(!mismatch_row.contains("unknown_mismatch"));
        assert!(mismatch_row.contains("ulp_diff="));
        assert!(mismatch_row.contains(&format!("epsilon_budget={TERMINAL_SEED_ROUNDOFF_ULPS}")));
        assert!(
            exported
                .find("\tparity.computed.first-mismatch\t")
                .expect("first-mismatch stage")
                < exported
                    .find("\tparity.reject\t")
                    .expect("terminal parity rejection stage"),
        );
    }

    #[test]
    fn rectangle_terminal_derived_roundoff_keeps_public_fillet_branch_state_exact() {
        let (edge, source_scales) = typed_panel_fillet_edge_and_sources();
        let mut changed_branch = edge.clone();
        let ComputedEdgeGeometry::CircularArc(arc) = &mut changed_branch.geometry else {
            panic!("selected edge must remain a circular arc")
        };
        arc.sweep = match arc.sweep {
            geosolve_sketch::DocumentArcSweep::Clockwise => {
                geosolve_sketch::DocumentArcSweep::CounterClockwise
            }
            geosolve_sketch::DocumentArcSweep::CounterClockwise => {
                geosolve_sketch::DocumentArcSweep::Clockwise
            }
        };
        assert!(!terminal_computed_edge_matches(
            &edge,
            &changed_branch,
            &source_scales,
        ));

        let mut changed_tangent = edge.clone();
        let ComputedEdgeGeometry::CircularArc(arc) = &mut changed_tangent.geometry else {
            panic!("selected edge must remain a circular arc")
        };
        arc.tangent_orientations[0] = match arc.tangent_orientations[0] {
            geosolve_sketch::TangentOrientation::Aligned => {
                geosolve_sketch::TangentOrientation::Opposed
            }
            geosolve_sketch::TangentOrientation::Opposed => {
                geosolve_sketch::TangentOrientation::Aligned
            }
        };
        assert!(!terminal_computed_edge_matches(
            &edge,
            &changed_tangent,
            &source_scales,
        ));

        let mut changed_winding = edge.clone();
        let ComputedEdgeGeometry::CircularArc(arc) = &mut changed_winding.geometry else {
            panic!("selected edge must remain a circular arc")
        };
        arc.contacts[0].winding = arc.contacts[0]
            .winding
            .checked_add(1)
            .expect("bounded fixture winding");
        assert!(!terminal_computed_edge_matches(
            &edge,
            &changed_winding,
            &source_scales,
        ));

        let mut changed_provenance = edge.clone();
        let ComputedEdgeProvenance::FilletArc { sources, .. } = &mut changed_provenance.provenance
        else {
            panic!("selected edge must retain Fillet provenance")
        };
        sources.swap(0, 1);
        assert!(!terminal_computed_edge_matches(
            &edge,
            &changed_provenance,
            &source_scales,
        ));
    }

    #[test]
    fn computed_roundoff_requires_matching_design_and_accepted_alias_normalization() {
        let (_workbench, editor) = open_boxed("typed-panel");
        let points = editor
            .coordinator()
            .accepted_materialization()
            .expect("Typed Panel accepted materialization")
            .session
            .design_document()
            .points();
        let first = points.first().expect("Typed Panel point").id;
        let second = points.get(1).expect("second Typed Panel point").id;
        let normalized = TerminalDocumentParity::NormalizedRedundantRectangleAliases(
            BTreeMap::from([(first, 1.0_f64.to_bits())]),
        );
        let accepted_scale = TerminalDocumentParity::NormalizedRedundantRectangleAliases(
            BTreeMap::from([(first, 2.0_f64.to_bits())]),
        );
        let other =
            TerminalDocumentParity::NormalizedRedundantRectangleAliases(BTreeMap::from([(
                second,
                1.0_f64.to_bits(),
            )]));
        assert_eq!(
            terminal_computed_roundoff_points(&normalized, &normalized),
            Some(&BTreeMap::from([(first, 1.0_f64.to_bits())])),
        );
        assert!(terminal_document_normalizations_match(
            &normalized,
            &normalized,
        ));
        assert_eq!(
            terminal_computed_roundoff_points(&normalized, &accepted_scale),
            accepted_scale.normalized_redundant_rectangle_aliases(),
            "computed roundoff uses the accepted domain's scale metadata",
        );
        assert!(terminal_document_normalizations_match(
            &normalized,
            &accepted_scale,
        ));
        assert!(terminal_document_normalizations_match(
            &TerminalDocumentParity::Exact,
            &TerminalDocumentParity::Exact,
        ));
        assert!(
            terminal_computed_roundoff_points(&TerminalDocumentParity::Exact, &normalized)
                .is_none()
        );
        assert!(
            terminal_computed_roundoff_points(&normalized, &TerminalDocumentParity::Exact)
                .is_none()
        );
        assert!(terminal_computed_roundoff_points(&normalized, &other).is_none());
        assert!(!terminal_document_normalizations_match(
            &TerminalDocumentParity::Exact,
            &normalized,
        ));
        assert!(terminal_document_normalizations_match(
            &normalized,
            &TerminalDocumentParity::Exact,
        ));
        assert!(!terminal_document_normalizations_match(&normalized, &other,));
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

    const EXPECTED_SAMPLE_DOF: [(&str, usize, usize); 37] = [
        ("drafting-compass", 1, 1),
        ("bezier-continuity-bridge", 3, 1),
        ("twin-roller-cam", 2, 2),
        ("tangent-orbit", 1, 1),
        ("elliptic-trammel", 1, 1),
        ("scotch-yoke", 1, 1),
        ("rotating-constraint-square", 1, 1),
        ("scissor-jack", 1, 1),
        ("five-stage-scissor-tower", 1, 1),
        ("peaucellier-inversor", 1, 1),
        ("four-bar-coupler", 1, 1),
        ("pantograph-linkage", 2, 2),
        ("three-link-drawing-arm", 3, 3),
        ("constraint-dimension-sampler", 28, 28),
        ("auto-constraint-drafting", 12, 12),
        ("retained-drafting-relations", 16, 16),
        ("tangent-radial-normal", 6, 6),
        ("contact-branch-specimen", 0, 0),
        ("angle-dimension-annotations", 1, 1),
        ("contextual-constraint-annotations", 112, 112),
        ("dense-constraint-junction", 1, 1),
        ("construction-reference-geometry", 0, 0),
        ("curve-family-gallery", 172, 164),
        ("periodic-nurbs-specimen", 13, 12),
        ("fillet-workshop", 16, 16),
        ("rounded-polyline", 12, 12),
        ("typed-panel", 8, 8),
        ("braced-frame", 4, 4),
        ("mounting-plate", 16, 16),
        ("adaptive-lanterns", 21, 21),
        ("suspension-bridge", 7, 7),
        ("compass-rose", 10, 10),
        ("neon-manifold", 6, 6),
        ("pc-water-manifold", 0, 0),
        ("robotic-routing-board", 228, 228),
        ("cnc-joinery-fit-coupon", 16, 16),
        ("gridfinity-1x1x3-section", 0, 0),
    ];

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "one reviewed sample matrix keeps source, compiler, artifact, solve, DOF, and finite-scene parity adjacent"
    )]
    fn all_thirty_seven_samples_open_with_nonempty_independently_validated_native_canvases() {
        let demos = bundled_code_projects();
        assert_eq!(
            demos.len(),
            37,
            "every user-visible sample must have source and executed code authority"
        );
        for (demo, (expected_key, expected_raw_dof, expected_effective_dof)) in
            demos.into_iter().zip(EXPECTED_SAMPLE_DOF)
        {
            assert_eq!(
                demo.key(),
                expected_key,
                "the reviewed DOF ledger is ordered"
            );
            let project = demo.project();
            let source_before = project.managed.source.clone();
            let compiled_before = project
                .managed
                .compiled
                .clone()
                .expect("bundled sample owns compiler authority");
            let artifacts_before = project.artifacts.clone();
            let canonical = project
                .to_canonical_json()
                .unwrap_or_else(|error| panic!("{} canonical project: {error}", demo.key()));
            let restored = CodeProject::from_json(&canonical)
                .unwrap_or_else(|error| panic!("{} canonical round-trip: {error}", demo.key()));
            assert_eq!(
                restored.managed.source,
                source_before,
                "{} source",
                demo.key()
            );
            assert_eq!(
                restored.managed.compiled.as_deref(),
                Some(compiled_before.as_ref()),
                "{} executed IR and compiler artifact",
                demo.key(),
            );
            assert_eq!(
                restored.artifacts,
                artifacts_before,
                "{} patch artifacts",
                demo.key()
            );
            assert_eq!(
                restored.to_canonical_json().unwrap(),
                canonical,
                "{} canonical bytes",
                demo.key(),
            );

            let (workbench, editor) = open_with_editor(demo.key());
            let accepted = editor
                .coordinator()
                .accepted_materialization()
                .expect("code sample owns accepted native authority");
            let accepted_state = accepted
                .session
                .accepted_state_for_current_input()
                .expect("sample acceptance belongs to current input");
            let diagnostics = accepted_state.diagnostics();
            assert_eq!(
                diagnostics
                    .rank
                    .and_then(|rank| rank.numerical_right_nullity),
                Some(expected_raw_dof),
                "{} raw numerical right-nullity",
                demo.key(),
            );
            let mobility = diagnostics
                .mobility
                .unwrap_or_else(|| panic!("{} mobility diagnostic", demo.key()));
            assert_eq!(
                mobility.equality_degrees_of_freedom,
                Some(expected_raw_dof),
                "{} equality DOF agrees with its numerical right-nullity",
                demo.key(),
            );
            assert_eq!(
                mobility.bidirectional_bounded_degrees_of_freedom,
                Some(expected_effective_dof),
                "{} effective bidirectional mobility",
                demo.key(),
            );
            let design = accepted.session.design_document();
            assert!(
                !design.points().is_empty() && !design.curves().is_empty(),
                "{} opened an empty native canvas",
                demo.key(),
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
    fn reusable_projects_use_semantic_groups_without_implementation_badges() {
        let markup = sample_group_markup(None);
        assert!(!markup.contains("Code &amp; reusable patches"));
        assert!(!markup.contains("Code projects"));
        assert!(!markup.contains("wb-code-sample-mark"));
        for group in [
            "Patterns &amp; generated geometry",
            "Structures",
            "Fabrication &amp; products",
        ] {
            assert!(markup.contains(group), "missing semantic group {group}");
        }
        assert_eq!(markup.matches("data-sample-group-trigger").count(), 3);
        let demos = bundled_code_project_demos();
        assert_eq!(
            demos.len(),
            12,
            "the additive code catalog includes the routing and manufacturing dogfood demonstrations"
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
    fn managed_and_custom_files_have_truthful_distinct_ownership_surfaces() {
        let mut workbench = open("rounded-polyline");
        let managed = workbench.panel_markup();
        assert!(managed.contains("data-code-file-kind=\"managed\""));
        assert!(managed.contains("data-code-action=\"apply\""));
        assert!(managed.contains("Executed managed sketch"));
        assert!(managed.contains("Managed controls"));
        assert!(managed.contains("data-code-control-id="));
        assert!(managed.contains("Modifiable in sketch.ts"));
        assert!(managed.contains("<code>mm(4)</code>"));
        assert!(managed.contains("<strong>rounded</strong>"));
        assert!(managed.contains("<strong>radius</strong>"));
        assert!(!managed.contains("data-code-lens-declaration="));
        assert!(!managed.contains("data-code-lens-path="));
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
    fn every_enabled_manifest_control_is_reachable_once_in_the_code_panel() {
        for demo in bundled_code_project_demos() {
            let workbench = open(demo.id.key());
            let manifest = workbench
                .managed_controls()
                .unwrap_or_else(|error| panic!("{} manifest: {error}", demo.id.key()));
            let markup = workbench.panel_markup();
            let enabled = manifest.editable().collect::<Vec<_>>();
            assert_eq!(
                markup.matches("data-code-control-id=").count(),
                enabled.len(),
                "{} must render every enabled control exactly once",
                demo.id.key(),
            );
            for control in enabled {
                let attribute = format!(
                    "data-code-control-id=\"{}\"",
                    escape_attribute(&control.id.0),
                );
                assert_eq!(
                    markup.matches(&attribute).count(),
                    1,
                    "{} omitted or duplicated enabled control {}",
                    demo.id.key(),
                    control.id.0,
                );
            }
        }
    }

    fn assert_reloaded_manifests_are_transient(
        reproduction_workspace: &str,
        persistence_before: &str,
        manifest: &ManagedControlManifest,
    ) {
        let restored = CodeProjectWorkbench::from_persistence_json(reproduction_workspace).unwrap();
        assert_eq!(restored.to_persistence_json().unwrap(), persistence_before);
        let restored_clean_manifest = restored.managed_controls_cached().unwrap();
        assert_eq!(
            restored_clean_manifest.as_ref(),
            manifest,
            "reload must rederive the same deterministic transient manifest",
        );
    }

    #[test]
    fn managed_manifest_and_capabilities_are_transient_browser_authority() {
        let mut workbench = open("typed-panel");
        let persistence_before = workbench.to_persistence_json().unwrap();
        let project_before = workbench.project.to_canonical_json().unwrap();
        let expansion_before = serde_json::to_string(
            workbench
                .session
                .snapshot()
                .accepted_expansion
                .as_ref()
                .expect("accepted Typed Panel expansion"),
        )
        .unwrap();
        let cached_manifest = workbench.managed_controls_cached().unwrap();
        let cached_again = workbench.managed_controls_cached().unwrap();
        assert!(
            Rc::ptr_eq(&cached_manifest, &cached_again),
            "unchanged clean presentation must reuse one transient manifest authority",
        );
        let manifest = cached_manifest.as_ref().clone();
        let tokens = manifest
            .editable()
            .map(|control| {
                control
                    .token()
                    .expect("enabled control has an authenticated capability")
            })
            .collect::<Vec<_>>();
        assert!(!tokens.is_empty());

        assert_eq!(workbench.to_persistence_json().unwrap(), persistence_before);
        assert_eq!(
            workbench.project.to_canonical_json().unwrap(),
            project_before
        );
        assert_eq!(
            serde_json::to_string(
                workbench
                    .session
                    .snapshot()
                    .accepted_expansion
                    .as_ref()
                    .unwrap(),
            )
            .unwrap(),
            expansion_before,
        );
        let reproduction = crate::reproduction::encode_workspace(&persistence_before)
            .expect("encode exact code-workbench reproduction");
        let reproduction_workspace = crate::reproduction::decode_workspace(&reproduction)
            .expect("decode exact code-workbench reproduction");
        assert_eq!(
            reproduction_workspace.as_bytes(),
            persistence_before.as_bytes(),
            "reproduction transport must preserve the exact persistent code-workbench bytes",
        );
        assert!(!reproduction.contains("\"authentication\""));
        assert!(!reproduction_workspace.contains("\"authentication\""));
        for token in &tokens {
            let token_wire = serde_json::to_string(token).unwrap();
            assert!(!token.authentication.is_empty());
            assert!(
                !persistence_before.contains(&token.authentication),
                "managed capability authentication must not enter workspace persistence",
            );
            for (label, wire) in [
                ("reproduction payload", reproduction.as_str()),
                (
                    "decoded reproduction workspace",
                    reproduction_workspace.as_str(),
                ),
            ] {
                assert!(
                    !wire.contains(&token_wire),
                    "managed capability token must not enter {label}",
                );
                assert!(
                    !wire.contains(&token.authentication),
                    "managed capability authentication must not enter {label}",
                );
            }
        }
        let dirty_source = format!("{}// dirty cache isolation\n", workbench.managed_source());
        workbench.set_managed_draft(dirty_source);
        assert!(workbench.managed_controls_cached().is_err());
        assert!(workbench.revert_managed_draft());
        assert!(
            Rc::ptr_eq(
                &cached_manifest,
                &workbench.managed_controls_cached().unwrap(),
            ),
            "reverting to the same exact clean identity may reuse its immutable manifest",
        );
        assert_reloaded_manifests_are_transient(
            &reproduction_workspace,
            &persistence_before,
            &manifest,
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
    fn tampered_nested_code_session_rejects_before_restore() {
        let workbench = open("braced-frame");
        let json = workbench.to_persistence_json().unwrap();
        let mut wire: serde_json::Value = serde_json::from_str(&json).unwrap();
        let session = wire["session"].as_str().unwrap();
        wire["session"] = serde_json::Value::String(
            session.replace("geosolve-demo-braced-frame", "geosolve-demo-tampered-frame"),
        );
        assert!(CodeProjectWorkbench::from_persistence_json(&wire.to_string()).is_err());
    }

    #[test]
    fn managed_source_navigation_reauthenticates_exact_control_span() {
        let (mut workbench, _) = CodeProjectWorkbench::open_key("typed-panel").unwrap();
        let (id, span) = {
            let manifest = workbench.managed_controls().unwrap();
            let control = manifest
                .controls
                .iter()
                .find(|control| managed_source_path(control) == "cornerFillets.radius")
                .expect("Typed Panel shared radius control");
            (control.id.0.clone(), control.source.span)
        };
        assert_eq!(
            workbench
                .open_managed_control_source(&id, span.start, span.end)
                .unwrap(),
            (span.start, span.end),
        );
        assert_eq!(workbench.selected_file.path(), MANAGED_FILE);
        assert!(
            workbench
                .open_managed_control_source(&id, span.start, span.end.saturating_add(1))
                .unwrap_err()
                .contains("stale authority")
        );
    }
}
