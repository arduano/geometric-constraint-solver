// SPDX-License-Identifier: GPL-3.0-or-later
//! Optional managed code-project composition for the demonstration workbench.
//!
//! This module deliberately owns presentation and caller-facing project
//! composition only. Executed managed artifacts, semantic identity and unified
//! code history remain public `geosolve-sketch-code` responsibilities. This
//! Rust adapter neither parses TypeScript nor duplicates solver equations.

#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

mod navigation;
#[cfg(test)]
mod workspace_tests;

use geosolve_sketch_code::editor_terminal::{
    TerminalPointPreview, expanded_port_point, validate_terminal_preview_session,
};
use geosolve_sketch_code::interaction::select_semantic_point_drag_lens;
use geosolve_sketch_code::managed_path_text;
#[cfg(test)]
use geosolve_sketch_code::managed_source_path;
#[cfg(test)]
use geosolve_sketch_code::{ExpandedSemanticTarget, generated_panel_row_id};
pub(crate) use geosolve_sketch_code::{
    ManagedControlSubmission, ManagedDeclarationClosureRole, ManagedDeclarationPanelProjection,
    ManagedDeclarationPanelRow, ManagedGeneratedPanelRow,
};
use geosolve_sketch_code::{encode_editor_checkpoint, restore_editor_checkpoint};

#[cfg(test)]
use geosolve_sketch_code::authoring_persistence::{
    CODE_WORKBENCH_WIRE_VERSION, LEGACY_CODE_WORKBENCH_WIRE_VERSION,
};
use geosolve_sketch_code::authoring_persistence::{
    SourceWorkspaceOrigin, managed_diagnostic_line_column, validate_managed_draft_bound,
    validate_managed_draft_diagnostic,
};

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};

use geosolve_constraint_editor::{
    ComputedFeatureDefinition, ComputedFeatureEvaluationState, ComputedFeatureId,
    DelegatedComputedFilletRadiusProposal, DelegatedPointDragProposal, IntentInspectorProjection,
    IntentNativeBinding, IntentWorkbenchProjection, ProjectionalEditorSession, SelectionItem,
};
use geosolve_sketch::{DocumentId, PersistentId};
#[cfg(test)]
use geosolve_sketch_code::editor_terminal::accepted_validation_is_publishable;
use geosolve_sketch_code::{
    BundledSampleSpec, CodeInteractionOverlay, CodeOwnerAddress, CodeProject, CodeSessionIdentity,
    CodeSessionReceipt, CompiledManagedSource, ExpandedCodeProject, ExpandedWritablePoint,
    GeneratedMemberAddress, GeneratedMemberIdentity, KeyedReconcileState, ManagedControl,
    ManagedControlAccess, ManagedControlConsumerTarget, ManagedControlEdit,
    ManagedControlEditBatch, ManagedControlId, ManagedControlManifest, ManagedControlSchema,
    ManagedControlToken, ManagedDiagnostic, ManagedDiagnosticCode, ManagedMutationAuthority,
    ManagedPathSegment, ManagedSketchMutation, ManagedSpan, ManagedValue, MaterializedCodeProject,
    PreparedManagedMutationReceipt, PreparedManagedMutationRequest, PreparedManagedSourceRequest,
    ProjectKey, SemanticOutputPath, SemanticSymbol, SketchCodeSession, UnitLiteral, bundled_sample,
    bundled_sample_catalog, managed_control_authority, managed_control_manifest,
    materialize_code_project_cold, materialize_code_project_incremental_with_overlay,
    prepare_managed_source, required_generated_members, validate_prepared_managed_source,
};
use geosolve_sketch_intent::{IntentNodeKind, IntentSessionId};
use serde::{Deserialize, Serialize};

const MANAGED_FILE: &str = "sketch.ts";
const CODE_PROJECT_MODEL_SCALE: f64 = 1.0;
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
    prepared: geosolve_sketch_engine::PreparedCanvasSourceMutation,
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
    /// Exact canvas selection before the originating press. A native press
    /// may replace it before semantic disambiguation, and code persistence
    /// deliberately carries no transient selection to restore on rejection.
    origin_selection: Vec<SelectionItem>,
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
    session: geosolve_sketch_engine::EditableSession,
    selected_file: SelectedCodeFile,
    managed_draft: String,
    draft_diagnostic: Option<ManagedDiagnostic>,
    last_receipt: Option<CodeSessionReceipt>,
    // Exact-session immutable presentation authority. Dirty text is excluded;
    // clean and retained-failure sessions receive distinct keyed entries.
    // Mutations still derive a fresh borrow-scoped `ManagedControlAuthority`
    // for exact-CAS application.
    managed_control_manifest_cache: RefCell<Option<ManagedControlManifestCache>>,
    authored_metadata_cache:
        RefCell<Option<(String, Rc<geosolve_sketch_code::ManagedAuthoredMetadata>)>>,
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
    Bundled(&'static BundledSampleSpec),
    Authored,
}

impl CodeProjectOrigin {
    fn sample_key(&self) -> Option<&'static str> {
        match self {
            Self::Bundled(sample) => Some(sample.key),
            Self::Authored => None,
        }
    }

    fn to_wire(&self) -> SourceWorkspaceOrigin {
        match self {
            Self::Bundled(sample) => SourceWorkspaceOrigin::Bundled {
                sample: sample.key.into(),
            },
            Self::Authored => SourceWorkspaceOrigin::Authored,
        }
    }
}

fn restore_code_project_origin(origin: SourceWorkspaceOrigin) -> Result<CodeProjectOrigin, String> {
    let bundled = |key: &str| {
        bundled_sample(key)
            .map(CodeProjectOrigin::Bundled)
            .ok_or_else(|| format!("unknown bundled sample `{key}`"))
    };
    match origin {
        SourceWorkspaceOrigin::Bundled { sample } => bundled(&sample),
        SourceWorkspaceOrigin::Authored => Ok(CodeProjectOrigin::Authored),
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

    let snapshot = code_project.session.source_session().snapshot();
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

/// Inspectable authored override state; geometry is always rebuilt from the source.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct WorkspaceDesign {
    pub format: String,
    pub project: ProjectKey,
    pub generated: KeyedReconcileState,
    pub overrides: CodeInteractionOverlay,
}

impl CodeProjectWorkbench {
    /// Returns only the pinned data-only patch plans needed by the managed
    /// recorder. This is deliberately an on-demand handoff: ordinary pointer,
    /// camera, and snapshot traffic must not retransmit potentially large
    /// compiler context.
    pub(crate) fn managed_compiler_patches(
        &self,
    ) -> Result<BTreeMap<String, serde_json::Value>, String> {
        geosolve_sketch_code::managed_compiler_patches(&self.project)
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
        if self.session.source_session().snapshot().failure.is_some() {
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
            .source_session()
            .snapshot()
            .accepted_expansion
            .as_ref()
            .ok_or_else(|| "code project has no accepted expansion authority".to_owned())?;
        let authority = ManagedMutationAuthority::new(
            self.project.project.clone(),
            self.session.source_session().identity().clone(),
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
        self.managed_mutation_authority()?;
        let prepared = self
            .session
            .prepare_canvas_source_mutation(candidate_editor)
            .map_err(|error| error.to_string())?;
        Ok(PreparedManagedCanvasMutation {
            request: prepared.request().clone(),
            prepared,
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
        self.managed_mutation_authority()?;
        let prepared = self
            .session
            .prepare_structured_source_mutation(mutation)
            .map_err(|error| error.to_string())?;
        Ok(PreparedManagedCanvasMutation {
            request: prepared.request().clone(),
            prepared,
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
        if self.is_dirty() {
            return Err(
                "Apply or Revert the managed-source draft before dragging code-owned geometry"
                    .into(),
            );
        }
        let pending = self.pending_semantic_point_drag.as_ref().ok_or_else(|| {
            "delegated point terminal has no pending authenticated route".to_owned()
        })?;
        if pending.pointer_id != pointer_id || proposal.pointer_id != pointer_id {
            return Err("terminal pointer does not own the pending semantic point gesture".into());
        }
        if &pending.session != self.session.source_session().identity() {
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
        let preview = TerminalPointPreview::new(origin_editor, proposal.terminal_session())?;
        validate_terminal_preview_session(preview.session())?;
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
        let mut candidate = self.session.fork_authoring();
        let receipt = candidate
            .commit_delegated_point_terminal(
                &point,
                origin_editor,
                proposal.terminal_session(),
                label,
            )
            .map_err(|error| error.to_string())?;
        let mut editor =
            restore_editor_checkpoint(candidate.source_session().pointer_frame_checkpoint())?;
        if let Some(alias) = selected_alias {
            let node = editor
                .coordinator()
                .intent()
                .graph()
                .node_by_symbol(&alias)
                .ok_or_else(|| "published point overlay lost its selected declaration".to_owned())?
                .id;
            if !editor.set_selected_declaration(Some(node)) {
                return Err("published point overlay could not restore selection".into());
            }
        }
        self.session = candidate;
        self.pending_semantic_point_drag = None;
        self.last_receipt = Some(receipt.clone());
        Ok(Some(AcceptedCodePublication { editor, receipt }))
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
            .source_session()
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
        self.managed_mutation_authority()?;
        let mut candidate = self.fork_authoring();
        let receipt = candidate
            .session
            .apply_canvas_source_mutation(&prepared.prepared, receipt)
            .map_err(|error| error.to_string())?;
        candidate.synchronize_source_state();
        let mut publication = AcceptedCodePublication {
            editor: candidate.restore_accepted_editor()?,
            receipt,
        };
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
        let mut candidate = self.fork_authoring();
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
        let identity = self.session.source_session().identity();
        let pending = self.pending_semantic_point_drag.as_ref();
        format!(
            "project={:?} origin={} session={} revision={} digest={} pending_pointer={} pending_alias={} pending_selector={:?}",
            self.session.source_session().snapshot().project,
            self.origin.sample_key().unwrap_or("non-bundled"),
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
        if let Some(sample) = bundled_sample(key) {
            return Self::open_project(CodeProjectOrigin::Bundled(sample), sample.project());
        }
        #[cfg(test)]
        if let Some(project) = super::test_code_projects::managed_regression_project(key) {
            return Self::open_project(CodeProjectOrigin::Authored, project);
        }
        Err(format!("unknown bundled sample `{key}`"))
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
    pub(crate) fn open_managed_test_compiled_with_sample_pins(
        project_key: &str,
        sample_key: &str,
        compiled: CompiledManagedSource,
    ) -> Result<(Self, Box<ProjectionalEditorSession>), String> {
        let mut project = if let Some(sample) = bundled_sample(sample_key) {
            sample.project()
        } else if let Some(project) =
            super::test_code_projects::managed_regression_project(sample_key)
        {
            project
        } else {
            return Err(format!("unknown bundled sample `{sample_key}`"));
        };
        project.project = ProjectKey(project_key.into());
        project.managed = compiled
            .into_managed_document()
            .map_err(|error| error.to_string())?;
        Self::open_project(CodeProjectOrigin::Authored, project)
    }

    #[cfg(test)]
    pub(crate) fn managed_test_overlay_is_empty(&self) -> bool {
        let overlay = &self.session.source_session().snapshot().interaction_overlay;
        overlay.drafts().is_empty() && overlay.suppressed_children().is_empty()
    }

    pub(crate) fn workspace_project_key(&self) -> ProjectKey {
        self.project.project.clone()
    }

    pub(crate) fn workspace_design(&self) -> WorkspaceDesign {
        WorkspaceDesign {
            format: "geosolve-design-v1".into(),
            project: self.project.project.clone(),
            generated: self.session.source_session().snapshot().generated.clone(),
            overrides: self
                .session
                .source_session()
                .snapshot()
                .interaction_overlay
                .clone(),
        }
    }

    pub(crate) fn open_workspace_design(
        project: CodeProject,
        design: WorkspaceDesign,
    ) -> Result<(Self, Box<ProjectionalEditorSession>), String> {
        if design.format != "geosolve-design-v1" || design.project != project.project {
            return Err("design sidecar format or project identity mismatch".into());
        }
        design
            .overrides
            .validate()
            .map_err(|error| error.to_string())?;
        Self::open_project_with_design(
            CodeProjectOrigin::Authored,
            project,
            &design.generated,
            design.overrides,
        )
    }

    /// Stages a complete local source/dependency update with the existing semantic
    /// history and overlays. Failed candidates cannot alter the live project.
    pub(crate) fn prepare_workspace_project(
        &self,
        project: CodeProject,
    ) -> Result<(Self, AcceptedCodePublication), String> {
        project.validate().map_err(|error| error.to_string())?;
        if project.project != self.project.project {
            return Err("workspace update belongs to a different project".into());
        }
        let mut candidate = self.fork_authoring();
        match candidate.apply_candidate_project(project)? {
            CodeApplyOutcome::Accepted(publication) => Ok((candidate, publication)),
            CodeApplyOutcome::RetainedFailure { diagnostic, .. } => Err(diagnostic),
        }
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
        Self::open_project_with_design(
            origin,
            project,
            &KeyedReconcileState::empty(),
            CodeInteractionOverlay::empty(),
        )
    }

    fn open_project_with_design(
        origin: CodeProjectOrigin,
        project: CodeProject,
        generated: &KeyedReconcileState,
        overlay: CodeInteractionOverlay,
    ) -> Result<(Self, Box<ProjectionalEditorSession>), String> {
        project.validate().map_err(|error| error.to_string())?;
        let desired = required_generated_members(&project).map_err(|error| error.to_string())?;
        let plan = generated
            .plan(desired, &BTreeSet::new())
            .map_err(|error| error.to_string())?;
        let (intent, document) = next_materialization_ids()?;
        let materialized = geosolve_sketch_code::materialize_code_project_cold_with_overlay(
            &project,
            plan.staged(),
            &overlay,
            intent,
            document,
            CODE_PROJECT_MODEL_SCALE,
        )
        .map_err(|error| error.to_string())?;
        let expansion = materialized.expansion.clone();
        let checkpoint = encode_editor_checkpoint(&materialized.editor)?;
        let delegated_editor = restore_editor_checkpoint(&checkpoint)?;
        let session = SketchCodeSession::new_project_with_overlay(
            project.clone(),
            plan.into_staged(),
            overlay,
            expansion,
            checkpoint,
        )
        .map_err(|error| error.to_string())?;
        let session = geosolve_sketch_engine::EditableSession::from_source_session(session)
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
                managed_control_manifest_cache: RefCell::new(None),
                authored_metadata_cache: RefCell::new(None),
                pending_semantic_point_drag: None,
                trace_semantic_point: None,
            },
            delegated_editor,
        ))
    }

    pub(crate) fn to_persistence_json(&self) -> Result<String, String> {
        geosolve_sketch_code::authoring_persistence::encode_source_workspace(
            &self.origin.to_wire(),
            &self.project,
            self.session.source_session(),
            self.selected_file.path(),
            &self.managed_draft,
            self.draft_diagnostic.as_ref(),
        )
    }

    pub(crate) fn from_persistence_json(json: &str) -> Result<Self, String> {
        let wire = geosolve_sketch_code::authoring_persistence::decode_source_workspace(json)?;
        let origin = restore_code_project_origin(wire.origin)?;
        let project = wire.project;
        let session = wire.session;
        let session =
            geosolve_sketch_engine::EditableSession::from_validated_source_history(session)
                .map_err(|error| error.to_string())?;
        let mut value = Self {
            origin,
            project,
            session,
            selected_file: SelectedCodeFile::Managed,
            managed_draft: wire.managed_draft,
            draft_diagnostic: wire.draft_diagnostic,
            last_receipt: None,
            managed_control_manifest_cache: RefCell::new(None),
            authored_metadata_cache: RefCell::new(None),
            pending_semantic_point_drag: None,
            trace_semantic_point: None,
        };
        value.select_file(&wire.selected_file)?;
        Ok(value)
    }

    pub(crate) fn accepted_editor_checkpoint(&self) -> &serde_json::Value {
        &self
            .session
            .source_session()
            .snapshot()
            .accepted_editor_checkpoint
    }

    fn synchronize_source_state(&mut self) {
        self.project = self
            .session
            .source_session()
            .snapshot()
            .code_project
            .clone()
            .expect("engine authoring retains source project authority");
        self.managed_draft = self.project.managed.source.clone();
        self.draft_diagnostic = None;
    }

    fn fork_authoring(&self) -> Self {
        Self {
            origin: self.origin.clone(),
            project: self.project.clone(),
            session: self.session.fork_authoring(),
            selected_file: self.selected_file.clone(),
            managed_draft: self.managed_draft.clone(),
            draft_diagnostic: self.draft_diagnostic.clone(),
            last_receipt: self.last_receipt.clone(),
            managed_control_manifest_cache: RefCell::new(None),
            authored_metadata_cache: RefCell::new(None),
            pending_semantic_point_drag: None,
            trace_semantic_point: None,
        }
    }

    pub(crate) fn local_point_targets(
        &self,
        editor: &ProjectionalEditorSession,
    ) -> BTreeMap<geosolve_sketch::DesignPointId, serde_json::Value> {
        self.session
            .source_session()
            .snapshot()
            .accepted_expansion
            .as_ref()
            .map(|expansion| geosolve_sketch_code::interaction::point_targets(expansion, editor))
            .unwrap_or_default()
    }

    pub(crate) fn local_presence_bindings(
        &self,
        editor: &ProjectionalEditorSession,
    ) -> BTreeMap<String, Vec<IntentNativeBinding>> {
        self.session
            .source_session()
            .snapshot()
            .accepted_expansion
            .as_ref()
            .map(|expansion| {
                geosolve_sketch_code::interaction::presence_bindings(expansion, editor)
            })
            .unwrap_or_default()
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
            .source_session()
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
            .source_session()
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
        origin_selection: &[SelectionItem],
    ) -> Result<Option<PreparedCodePointDrag>, String> {
        if self.pending_semantic_point_drag.is_some() {
            return Err("another authenticated semantic point gesture is still pending".into());
        }
        self.point_drag_permission(editor, native_point)?;
        let expansion = self
            .session
            .source_session()
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
                session: self.session.source_session().identity().clone(),
                native_intent: editor.coordinator().intent().identity(),
                native_point,
                point,
                selected_alias,
                origin_selection: origin_selection.to_vec(),
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
            .source_session()
            .stage_point_drag(&point, position)
            .map_err(|error| error.to_string())?;
        let materialized = materialize_code_project_incremental_with_overlay(
            self.session.accepted_materialization(),
            &self.project,
            &self.session.source_session().snapshot().generated,
            &overlay,
        )
        .map_err(|error| error.to_string())?;
        let detached_point = expanded_port_point(&materialized.editor, &point.handle)
            .ok_or_else(|| "detached consumer has no accepted native point".to_owned())?;
        let checkpoint = encode_editor_checkpoint(&materialized.editor)?;
        let detached_editor = restore_editor_checkpoint(&checkpoint)?;
        self.pending_semantic_point_drag = Some(PendingSemanticPointDrag {
            pointer_id,
            session: self.session.source_session().identity().clone(),
            native_intent: materialized.editor.coordinator().intent().identity(),
            native_point: detached_point,
            point,
            selected_alias,
            origin_selection: origin_selection.to_vec(),
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
            // Restore the authenticated consumer owner and its original
            // native selection independently. The declaration-only setter
            // clears native items; reprojecting a shared producer point could
            // instead replace the consumer's deliberately chosen source lens.
            editor.editor_mut().set_selection(pending.origin_selection);
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
        restore_editor_checkpoint(self.session.source_session().pointer_frame_checkpoint())
    }

    pub(crate) fn sample_key(&self) -> Option<&'static str> {
        self.origin.sample_key()
    }

    /// Human-facing project title for presentation adapters.
    ///
    /// The accepted source owns the title; dirty drafts never change it.
    pub(crate) fn title(&self) -> &str {
        self.managed_compilation()
            .and_then(|compiled| compiled.artifact.document.as_ref())
            .and_then(|document| document.title.as_deref())
            .filter(|title| !title.is_empty())
            .unwrap_or("Untitled code sketch")
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
            .source_session()
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

    /// Resolves the selected declaration through accepted source provenance.
    pub(crate) fn selected_managed_declaration(
        &self,
        editor: &ProjectionalEditorSession,
    ) -> Result<Option<SemanticSymbol>, String> {
        geosolve_sketch_code::ManagedSourceInspector::new(
            self.session.source_session().snapshot(),
            Some(self.session.accepted_materialization()),
        )
        .selected_managed_declaration(editor)
    }

    /// Exact accepted source statement owned by one managed declaration.
    pub(crate) fn managed_declaration_source_span(
        &self,
        declaration: &SemanticSymbol,
    ) -> Result<ManagedSpan, String> {
        geosolve_sketch_code::ManagedSourceInspector::new(
            self.session.source_session().snapshot(),
            Some(self.session.accepted_materialization()),
        )
        .managed_declaration_source_span(declaration)
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

    /// Shares the caller's retained projection, descriptors and control manifest.
    pub(crate) fn inspector_parameter_presentations_with_manifest(
        &self,
        editor: &ProjectionalEditorSession,
        projection: &IntentWorkbenchProjection,
        inspector: &IntentInspectorProjection,
        descriptors: &super::design_projection::InspectorDescriptorIndex<'_>,
        manifest: Result<&ManagedControlManifest, &str>,
    ) -> Result<Vec<super::design_projection::InspectorParameterPresentation>, String> {
        geosolve_sketch_code::ManagedSourceInspector::new(
            self.session.source_session().snapshot(),
            Some(self.session.accepted_materialization()),
        )
        .inspector_parameter_presentations(
            editor,
            projection,
            inspector,
            descriptors,
            manifest,
        )
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
        if Some(
            self.session
                .accepted_materialization()
                .editor
                .coordinator()
                .intent()
                .identity(),
        ) != Some(editor.coordinator().intent().identity())
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
        let snapshot = self.session.source_session().snapshot();
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
        if self.session.source_session().snapshot().failure.is_some() {
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
        let result = self
            .session
            .apply_project_retaining_failure(candidate_project)
            .map_err(|error| error.to_string())?;
        self.synchronize_source_state();
        match result {
            geosolve_sketch_engine::PersistentSourceApply::Accepted { receipt } => {
                self.last_receipt = Some(receipt.clone());
                Ok(CodeApplyOutcome::Accepted(AcceptedCodePublication {
                    editor: self.restore_accepted_editor()?,
                    receipt,
                }))
            }
            geosolve_sketch_engine::PersistentSourceApply::RetainedFailure {
                receipt,
                diagnostic,
            } => {
                self.last_receipt = Some(receipt.clone());
                Ok(CodeApplyOutcome::RetainedFailure {
                    receipt,
                    diagnostic,
                })
            }
        }
    }

    pub(crate) fn revert_managed_draft(&mut self) -> bool {
        self.pending_semantic_point_drag = None;
        let changed = self.managed_draft != self.session.source_session().snapshot().managed.source
            || self.draft_diagnostic.is_some();
        self.managed_draft = self
            .session
            .source_session()
            .snapshot()
            .managed
            .source
            .clone();
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
        if (undo && !self.session.source_session().can_undo())
            || (!undo && !self.session.source_session().can_redo())
        {
            return Ok(None);
        }
        self.pending_semantic_point_drag = None;
        let Some(receipt) = self
            .session
            .step_source_history(undo)
            .map_err(|error| error.to_string())?
        else {
            return Ok(None);
        };
        self.synchronize_source_state();
        self.last_receipt = Some(receipt.clone());
        Ok(Some(AcceptedCodePublication {
            editor: self.restore_accepted_editor()?,
            receipt,
        }))
    }

    pub(crate) fn can_undo(&self) -> bool {
        self.session.source_session().can_undo()
    }

    pub(crate) fn can_redo(&self) -> bool {
        self.session.source_session().can_redo()
    }

    /// Exact outer code-session identity consumed by the code-control RPC.
    pub(crate) fn chrome_source(&self) -> super::bridge::chrome_read::CodeChrome<'_> {
        super::bridge::chrome_read::CodeChrome::new(
            self.session.source_session().snapshot(),
            self.session.source_session().identity(),
            Some(self.session.accepted_materialization()),
            self.is_dirty(),
            self.managed_controls_cached(),
            self.authored_metadata_cached(),
        )
    }

    pub(crate) fn code_session_identity(&self) -> &CodeSessionIdentity {
        self.session.source_session().identity()
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
        let expansion = self
            .session
            .source_session()
            .snapshot()
            .expansion
            .as_ref()
            .ok_or_else(|| {
                "current managed source has no authenticated expansion for controls".to_owned()
            })?;
        let key = ManagedControlManifestCacheKey {
            session: self.session.source_session().identity().clone(),
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
            .source_session()
            .snapshot()
            .accepted_expansion
            .as_ref()
            .ok_or_else(|| "code project has no accepted expansion authority".to_owned())?;
        geosolve_sketch_code::managed_control_source_mutation(
            &self.project,
            expansion,
            id,
            submission,
        )
    }

    pub(crate) fn is_dirty(&self) -> bool {
        self.managed_draft != self.session.source_session().snapshot().managed.source
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
        geosolve_sketch_code::managed_declaration_panel_projection(
            self.session.source_session().snapshot(),
            editor,
            self.is_dirty(),
            self.managed_controls_cached().ok().as_deref(),
            self.authored_metadata_cached().ok().as_deref(),
        )
    }

    pub(crate) fn managed_source(&self) -> &str {
        &self.session.source_session().snapshot().managed.source
    }

    /// Accepted compiler authority; retained source drafts never enter this projection.
    pub(crate) fn managed_compilation(&self) -> Option<&CompiledManagedSource> {
        let snapshot = self.session.source_session().snapshot();
        snapshot
            .accepted_code_project
            .as_ref()
            .map_or(&snapshot.managed, |project| &project.managed)
            .compiled
            .as_deref()
    }

    pub(crate) fn metadata_edit_blocked_reason(&self) -> Option<String> {
        if self.is_dirty() {
            Some("Apply or Revert the source draft before editing properties".into())
        } else if self.session.source_session().snapshot().failure.is_some() {
            Some("Resolve or Undo the retained failure before editing properties".into())
        } else {
            None
        }
    }

    pub(crate) fn authored_metadata_cached(
        &self,
    ) -> Result<Rc<geosolve_sketch_code::ManagedAuthoredMetadata>, String> {
        let compiled = self
            .managed_compilation()
            .ok_or("This project has no compiled source")?;
        let digest = &compiled.artifact.artifact_digest;
        if let Some((cached_digest, metadata)) = self.authored_metadata_cache.borrow().as_ref()
            && cached_digest == digest
        {
            return Ok(Rc::clone(metadata));
        }
        let metadata = Rc::new(
            compiled
                .authored_metadata()
                .map_err(|error| error.to_string())?,
        );
        *self.authored_metadata_cache.borrow_mut() = Some((digest.clone(), Rc::clone(&metadata)));
        Ok(metadata)
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
        let revision = self.session.source_session().identity().revision;
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
            escape_html(self.title()),
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
        let members = self
            .session
            .source_session()
            .snapshot()
            .generated
            .ordered_members();
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
                    .source_session()
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

pub(crate) fn sample_group_markup(selected: Option<&str>) -> String {
    let mut markup = String::new();
    for category in [
        geosolve_sketch_code::SampleCategory::Mechanism,
        geosolve_sketch_code::SampleCategory::ProductFabrication,
        geosolve_sketch_code::SampleCategory::ReferenceLab,
        geosolve_sketch_code::SampleCategory::ScaleStudy,
    ] {
        let _ = write!(
            markup,
            "<li class=\"wb-sample-branch\"><button type=\"button\" data-sample-group-trigger aria-haspopup=\"menu\" aria-expanded=\"false\">{}<span aria-hidden=\"true\">›</span></button><ul class=\"wb-sample-flyout\">",
            escape_html(category.label()),
        );
        for sample in bundled_sample_catalog()
            .iter()
            .filter(|sample| sample.category == category)
        {
            let _ = write!(
                markup,
                "<li><button type=\"button\" data-sample-id=\"{}\"{}>{}</button></li>",
                sample.key,
                if selected == Some(sample.key) {
                    " aria-current=\"true\""
                } else {
                    ""
                },
                escape_html(sample.title),
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
    #[allow(
        clippy::too_many_lines,
        reason = "one adapter regression keeps the cold corner gesture, publication, locality and history witnesses together"
    )]
    fn m98_f041_cold_corner_publication_preserves_native_terminal_and_history() {
        use geosolve_constraint_editor::{EditorEffect, Modifiers, PointerInput, Viewport};

        let compiled = CompiledManagedSource::from_json(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../geosolve-sketch-engine/tests/fixtures/tool-operation-feature.json"
        )))
        .unwrap();
        let (mut workbench, mut editor) =
            CodeProjectWorkbench::open_managed_test_compiled("m98-f041-web", compiled).unwrap();
        let source_before = workbench.managed_source().to_owned();
        let project_before = workbench.project.to_canonical_json().unwrap();
        let accepted_document = |editor: &ProjectionalEditorSession| {
            let accepted = editor.coordinator().accepted_materialization().unwrap();
            assert!(accepted.validation.hard_residuals_validated);
            assert!(accepted.validation.all_active_features_current);
            assert!(
                accepted
                    .validation
                    .maximum_normalized_hard_residual
                    .is_none_or(|value| value.is_finite() && value <= 1.0e-9)
            );
            accepted
                .session
                .accepted_state_for_current_input()
                .unwrap()
                .document()
                .clone()
        };
        let document_before = accepted_document(&editor);
        let corner = document_before
            .points()
            .iter()
            .find(|point| pair_bits(point.position) == pair_bits([220.0, 0.0]))
            .unwrap()
            .id;
        let viewport = Viewport::new([800.0, 600.0], [220.0, 10.0], 10.0).unwrap();
        let input = |position| PointerInput {
            pointer_id: 98_041,
            position: viewport.model_to_screen(position),
            modifiers: Modifiers::default(),
        };
        assert!(
            workbench
                .prepare_semantic_point_drag(&editor, 98_041, corner, None, &[])
                .unwrap()
                .is_none()
        );
        let scene = editor.scene(viewport, 0.25).unwrap();
        editor
            .pointer_down_exact_point(&scene, input([220.0, 0.0]), corner)
            .unwrap();
        for step in 1..=4 {
            let ratio = f64::from(step) / 4.0;
            let scene = editor.scene(viewport, 0.25).unwrap();
            let effects = editor
                .pointer_move(&scene, input([220.0 - 5.0 * ratio, 21.0 * ratio]))
                .unwrap();
            assert!(effects.iter().any(|effect| matches!(
                effect,
                EditorEffect::PreviewPointMove { point, .. } if *point == corner
            )));
        }
        let scene = editor.scene(viewport, 0.25).unwrap();
        let proposal = editor
            .pointer_up_delegated_point(&scene, input([215.0, 21.0]))
            .unwrap()
            .proposal
            .unwrap();
        assert_eq!(
            pair_bits(proposal.accepted_position),
            pair_bits([215.0, 21.0])
        );
        assert!(
            !workbench.can_undo(),
            "preview cannot create source history"
        );
        let publication = workbench
            .publish_delegated_point_terminal(98_041, &editor, &proposal, "Move corner")
            .expect("cold corner release must retain its independently accepted preview")
            .unwrap();
        let document_after = accepted_document(&publication.editor);
        for point in document_after.points() {
            assert!(point.position.into_iter().all(f64::is_finite));
            let expected = if point.id == corner {
                [215.0, 21.0]
            } else {
                document_before.point(point.id).unwrap().position
            };
            assert_eq!(pair_bits(point.position), pair_bits(expected));
        }
        assert_eq!(workbench.managed_source(), source_before);
        assert_eq!(
            workbench.project.to_canonical_json().unwrap(),
            project_before
        );
        assert!(!workbench.has_any_pending_semantic_point_drag());

        let persisted = workbench.to_persistence_json().unwrap();
        let mut restored = CodeProjectWorkbench::from_persistence_json(&persisted).unwrap();
        assert_eq!(
            accepted_document(&restored.restore_accepted_editor().unwrap()),
            document_after
        );
        let undone = restored.step_history(true).unwrap().unwrap();
        assert_eq!(accepted_document(&undone.editor), document_before);
        assert!(
            !restored.can_undo(),
            "one release creates exactly one history entry"
        );
        let redone = restored.step_history(false).unwrap().unwrap();
        assert_eq!(accepted_document(&redone.editor), document_after);
        assert!(!restored.can_redo());
    }

    #[test]
    fn detached_point_cancel_restores_exact_selected_items_and_consumer_owner() {
        let compiled = CompiledManagedSource::from_json(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../packages/geosolve-sketch-code/test/fixtures/managed-compiler-envelope.json"
        )))
        .unwrap();
        let (mut workbench, mut editor) = CodeProjectWorkbench::open_managed_test_compiled(
            "m95-rejected-drag-selection",
            compiled,
        )
        .unwrap();
        let owner = SemanticSymbol("segment".into());
        let expansion = workbench
            .session
            .source_session()
            .snapshot()
            .accepted_expansion
            .as_ref()
            .unwrap();
        let consumer = expansion
            .declaration_provenance
            .iter()
            .find(|(_, declaration)| **declaration == owner)
            .and_then(|(alias, _)| editor.coordinator().intent().graph().node_by_symbol(alias))
            .unwrap()
            .id;
        let point = expansion
            .writable_points
            .iter()
            .filter(|point| point.source.is_reference())
            .find_map(|point| expanded_port_point(&editor, &point.handle))
            .unwrap();
        let owned_items = editor.navigation_selection_items([consumer]);
        assert_eq!(owned_items.len(), 2);
        for selection in [owned_items.clone(), vec![owned_items[1]], Vec::new()] {
            let persistence = workbench.to_persistence_json().unwrap();
            let identity = workbench.code_session_identity().clone();
            // Native pointer-down can replace the pre-press selection with
            // the shared producer; rollback must use the explicitly captured
            // origin rather than this current presentation.
            editor.set_selection([SelectionItem::Point(point)]);
            assert!(
                workbench
                    .prepare_semantic_point_drag(&editor, 95_003, point, Some(&owner), &selection,)
                    .unwrap()
                    .is_some()
            );
            assert!(
                workbench
                    .cancel_semantic_point_drag(Some(95_004))
                    .unwrap()
                    .is_none()
            );
            let restored = workbench
                .cancel_semantic_point_drag(Some(95_003))
                .unwrap()
                .unwrap();
            assert_eq!(restored.editor().selection(), selection);
            assert_eq!(
                workbench.selected_managed_declaration(&restored).unwrap(),
                Some(owner.clone())
            );
            assert_eq!(workbench.code_session_identity(), &identity);
            assert_eq!(workbench.to_persistence_json().unwrap(), persistence);
            assert!(!workbench.has_any_pending_semantic_point_drag());
            editor = restored;
        }
    }

    fn infeasible_contact_range_fixture(limited: bool) -> CompiledManagedSource {
        let fixture = if limited {
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../packages/geosolve-sketch-code/test/fixtures/managed-contact-range-infeasible-limited.json"
            ))
        } else {
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../packages/geosolve-sketch-code/test/fixtures/managed-contact-range-infeasible-base.json"
            ))
        };
        CompiledManagedSource::from_json(fixture)
            .expect("checked infeasible contact-range compiler fixture")
    }

    fn publish_managed_source_fixture(
        workbench: &mut CodeProjectWorkbench,
        source: &str,
        compiled: CompiledManagedSource,
    ) {
        workbench.set_managed_draft(source.to_owned());
        let prepared = workbench
            .prepare_managed_source_apply()
            .expect("fixture source differs from accepted source");
        let receipt = PreparedManagedMutationReceipt {
            ticket_digest: prepared.request.ticket.ticket_digest.clone(),
            base_source_digest: prepared.request.current.ir.source_digest.clone(),
            candidate_source_digest: compiled.ir.source_digest.clone(),
            compiled,
        };
        let ResolvedManagedSourceApply::Accepted { candidate, .. } = workbench
            .resolve_managed_source_apply(&prepared, receipt)
            .expect("fixture compiler receipt is valid")
        else {
            panic!("history fixture source must materialize successfully")
        };
        *workbench = *candidate;
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "one transaction regression freezes every accepted code/editor/materialization/history authority across a rejected numerical candidate"
    )]
    fn infeasible_contact_range_retains_authority_with_actionable_diagnostic() {
        let base = infeasible_contact_range_fixture(false);
        let candidate = infeasible_contact_range_fixture(true);
        let candidate_source = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../packages/geosolve-sketch-code/test/fixtures/managed-contact-range-infeasible-limited.sketch.ts"
        ))
        .to_owned();
        let (mut workbench, editor) =
            CodeProjectWorkbench::open_managed_test_compiled("m91-infeasible-contact-range", base)
                .expect("feasible contact-range base materializes");

        let project_before = workbench.project.to_canonical_json().unwrap();
        let session_before = workbench
            .session
            .source_session()
            .to_canonical_json()
            .unwrap();
        let identity_before = workbench.session.source_session().identity().clone();
        let checkpoint_before = workbench.accepted_editor_checkpoint().clone();
        let expansion_before = workbench
            .session
            .source_session()
            .snapshot()
            .accepted_expansion
            .clone()
            .expect("base expansion authority");
        let accepted_before = editor
            .coordinator()
            .accepted_materialization()
            .expect("base native authority");
        let document_before = accepted_before
            .session
            .accepted_state_for_current_input()
            .expect("base accepted document")
            .document()
            .clone();
        let ownership_before = accepted_before.ownership.clone();
        let validation_before = accepted_before.validation.clone();
        let evidence_before = accepted_before.evidence.clone();
        assert!(!workbench.session.source_session().can_undo());
        assert!(!workbench.session.source_session().can_redo());
        assert!(workbench.last_receipt.is_none());

        workbench.set_managed_draft(candidate_source.clone());
        let prepared = workbench
            .prepare_managed_source_apply()
            .expect("Rust prepares the structurally valid range edit");
        let receipt = PreparedManagedMutationReceipt {
            ticket_digest: prepared.request.ticket.ticket_digest.clone(),
            base_source_digest: prepared.request.current.ir.source_digest.clone(),
            candidate_source_digest: candidate.ir.source_digest.clone(),
            compiled: candidate,
        };
        let ResolvedManagedSourceApply::RetainedFailure { source, diagnostic } = workbench
            .resolve_managed_source_apply(&prepared, receipt)
            .expect("native rejection is a retained source outcome")
        else {
            panic!("numerically infeasible contact range must not publish")
        };

        assert_eq!(source, candidate_source);
        assert_eq!(workbench.managed_draft, candidate_source);
        assert_eq!(
            diagnostic,
            "base code patch rejected: managed contact `contact` authored interval [0, 0.5] was rejected: all hard constraints must admit a finite independently validated solution (native diagnostic `native-solver-rejected`)"
        );
        assert!(diagnostic.len() <= 256, "diagnostic must remain bounded");

        assert_eq!(
            workbench.project.to_canonical_json().unwrap(),
            project_before
        );
        assert_eq!(
            workbench
                .session
                .source_session()
                .to_canonical_json()
                .unwrap(),
            session_before
        );
        assert_eq!(
            workbench.session.source_session().identity(),
            &identity_before
        );
        assert_eq!(workbench.accepted_editor_checkpoint(), &checkpoint_before);
        assert_eq!(
            workbench
                .session
                .source_session()
                .snapshot()
                .accepted_expansion
                .as_ref(),
            Some(&expansion_before),
        );
        assert!(!workbench.session.source_session().can_undo());
        assert!(!workbench.session.source_session().can_redo());
        assert!(workbench.last_receipt.is_none());

        let restored = workbench
            .restore_accepted_editor()
            .expect("prior editor authority remains restorable");
        assert_eq!(
            encode_editor_checkpoint(&restored).unwrap(),
            checkpoint_before
        );
        let accepted_after = restored
            .coordinator()
            .accepted_materialization()
            .expect("prior materialization remains authoritative");
        assert_eq!(
            accepted_after
                .session
                .accepted_state_for_current_input()
                .expect("retained accepted document")
                .document(),
            &document_before,
        );
        assert_eq!(accepted_after.ownership, ownership_before);
        assert_eq!(accepted_after.validation, validation_before);
        assert_eq!(accepted_after.evidence, evidence_before);
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "one history regression keeps a rejected range edit, both history directions, accepted checkpoints, and replayability together"
    )]
    fn infeasible_contact_range_preserves_non_empty_undo_and_redo() {
        let base = CompiledManagedSource::from_json(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../packages/geosolve-sketch-code/test/fixtures/managed-contact-range-base.json"
        )))
        .expect("checked base compiler fixture");
        let supporting = CompiledManagedSource::from_json(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../packages/geosolve-sketch-code/test/fixtures/managed-contact-supporting-line.json"
        )))
        .expect("checked supporting-line compiler fixture");
        let normalized_supporting_source = supporting.normalized_source.clone();
        let limited = CompiledManagedSource::from_json(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../packages/geosolve-sketch-code/test/fixtures/managed-contact-range-limited.json"
        )))
        .expect("checked limited-range compiler fixture");
        let supporting_source = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../packages/geosolve-sketch-code/test/fixtures/managed-contact-supporting-line.sketch.ts"
        ));
        let limited_source = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../packages/geosolve-sketch-code/test/fixtures/managed-contact-range-limited.sketch.ts"
        ));
        let candidate = infeasible_contact_range_fixture(true);
        let candidate_source = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../packages/geosolve-sketch-code/test/fixtures/managed-contact-range-infeasible-limited.sketch.ts"
        ))
        .to_owned();
        let (mut workbench, _) =
            CodeProjectWorkbench::open_managed_test_compiled("m91-contact-history", base)
                .expect("base contact project materializes");

        publish_managed_source_fixture(&mut workbench, supporting_source, supporting);
        publish_managed_source_fixture(&mut workbench, limited_source, limited);
        workbench
            .step_history(true)
            .expect("history fixture Undo succeeds")
            .expect("second accepted source edit is undoable");
        assert!(workbench.can_undo());
        assert!(workbench.can_redo());
        assert_eq!(workbench.managed_source(), normalized_supporting_source);

        let accepted_before = workbench.to_persistence_json().unwrap();
        let session_before = workbench
            .session
            .source_session()
            .to_canonical_json()
            .unwrap();
        let project_before = workbench.project.to_canonical_json().unwrap();
        let checkpoint_before = workbench.accepted_editor_checkpoint().clone();

        workbench.set_managed_draft(candidate_source.clone());
        let prepared = workbench
            .prepare_managed_source_apply()
            .expect("Rust prepares the structurally valid range edit");
        let receipt = PreparedManagedMutationReceipt {
            ticket_digest: prepared.request.ticket.ticket_digest.clone(),
            base_source_digest: prepared.request.current.ir.source_digest.clone(),
            candidate_source_digest: candidate.ir.source_digest.clone(),
            compiled: candidate,
        };
        let ResolvedManagedSourceApply::RetainedFailure { source, diagnostic } = workbench
            .resolve_managed_source_apply(&prepared, receipt)
            .expect("native rejection is a retained source outcome")
        else {
            panic!("numerically infeasible contact range must not publish")
        };
        assert_eq!(source, candidate_source);
        assert!(diagnostic.contains("authored interval [0, 0.5] was rejected"));
        assert_eq!(
            workbench
                .session
                .source_session()
                .to_canonical_json()
                .unwrap(),
            session_before
        );
        assert_eq!(
            workbench.project.to_canonical_json().unwrap(),
            project_before
        );
        assert_eq!(workbench.accepted_editor_checkpoint(), &checkpoint_before);
        assert!(workbench.can_undo());
        assert!(workbench.can_redo());

        assert!(workbench.revert_managed_draft());
        assert_eq!(workbench.to_persistence_json().unwrap(), accepted_before);
        let mut control = CodeProjectWorkbench::from_persistence_json(&accepted_before).unwrap();
        for undo in [true, false] {
            workbench
                .step_history(undo)
                .expect("preserved history direction succeeds")
                .expect("preserved history direction has an entry");
            control
                .step_history(undo)
                .expect("control history direction succeeds")
                .expect("control history direction has an entry");
            assert_eq!(
                workbench.to_persistence_json().unwrap(),
                control.to_persistence_json().unwrap(),
            );
        }
    }

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

    fn open_with_editor(key: &str) -> (CodeProjectWorkbench, Box<ProjectionalEditorSession>) {
        CodeProjectWorkbench::open_key(key).expect("code project")
    }

    fn open(key: &str) -> CodeProjectWorkbench {
        open_with_editor(key).0
    }

    #[test]
    fn fabrication_operations_atlas_checkpoint_preserves_exact_authority() {
        let sample = bundled_sample("fabrication-operations-atlas").expect("registered atlas");
        let project = sample.project();
        let desired = required_generated_members(&project).expect("generated members");
        let plan = KeyedReconcileState::empty()
            .plan(desired, &BTreeSet::new())
            .expect("reconciliation plan");
        let materialized =
            materialize_candidate(&project, plan.staged()).expect("initial atlas materialization");
        let accepted = materialized
            .editor
            .coordinator()
            .accepted_materialization()
            .expect("atlas accepted materialization");
        assert!(accepted_validation_is_publishable(&accepted.validation));
        validate_terminal_preview_session(&accepted.session).expect("validated atlas session");
        let design_before = accepted.session.design_document().clone();
        let accepted_before = accepted
            .session
            .accepted_state_for_current_input()
            .expect("current accepted atlas")
            .document()
            .clone();
        for document in [&design_before, &accepted_before] {
            assert!(
                document
                    .points()
                    .iter()
                    .flat_map(|point| point.position)
                    .chain(document.scalars().iter().map(|scalar| scalar.value))
                    .all(f64::is_finite)
            );
        }
        let ownership_before = accepted.ownership.clone();
        let validation_before = accepted.validation.clone();
        let evidence_before = accepted.evidence.clone();

        let checkpoint = encode_editor_checkpoint(&materialized.editor).expect("atlas checkpoint");
        let restored = restore_editor_checkpoint(&checkpoint).expect("atlas checkpoint restore");
        let accepted_after = restored
            .coordinator()
            .accepted_materialization()
            .expect("restored atlas accepted materialization");
        assert!(accepted_validation_is_publishable(
            &accepted_after.validation
        ));
        assert_eq!(accepted_after.session.design_document(), &design_before);
        assert_eq!(
            accepted_after
                .session
                .accepted_state_for_current_input()
                .expect("restored current accepted atlas")
                .document(),
            &accepted_before,
        );
        assert_eq!(accepted_after.ownership, ownership_before);
        assert_eq!(accepted_after.validation, validation_before);
        assert_eq!(accepted_after.evidence, evidence_before);
    }

    fn open_boxed(key: &str) -> (Box<CodeProjectWorkbench>, Box<ProjectionalEditorSession>) {
        let (workbench, editor) = open_with_editor(key);
        (Box::new(workbench), editor)
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "one reviewed sample matrix keeps source, compiler, artifact, solve, DOF, and finite-scene parity adjacent"
    )]
    fn all_bundled_samples_open_with_nonempty_independently_validated_native_canvases() {
        let samples = bundled_sample_catalog();
        let reviewed: serde_json::Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../geosolve-sketch-code/assets/bundled-sample-catalog.json"
        )))
        .expect("reviewed sample catalog contract");
        assert_eq!(reviewed["schema"], 1);
        assert_eq!(
            serde_json::json!(
                samples
                    .iter()
                    .map(|sample| serde_json::json!({
                        "key": sample.key, "title": sample.title, "category": sample.category,
                    }))
                    .collect::<Vec<_>>()
            ),
            reviewed["samples"],
            "runtime catalog must match independent reviewed order and metadata"
        );
        for sample in samples {
            let expected_raw_dof = sample.expected.numerical_right_nullity();
            let expected_effective_dof = sample.expected.bidirectional_bounded_degrees_of_freedom();
            let project = sample.project();
            let source_before = project.managed.source.clone();
            let compiled_before = project
                .managed
                .compiled
                .clone()
                .expect("bundled sample owns compiler authority");
            let artifacts_before = project.artifacts.clone();
            let canonical = project
                .to_canonical_json()
                .unwrap_or_else(|error| panic!("{} canonical project: {error}", sample.key));
            let restored = CodeProject::from_json(&canonical)
                .unwrap_or_else(|error| panic!("{} canonical round-trip: {error}", sample.key));
            assert_eq!(
                restored.managed.source, source_before,
                "{} source",
                sample.key
            );
            assert_eq!(
                restored.managed.compiled.as_deref(),
                Some(compiled_before.as_ref()),
                "{} executed IR and compiler artifact",
                sample.key,
            );
            assert_eq!(
                restored.artifacts, artifacts_before,
                "{} patch artifacts",
                sample.key
            );
            assert_eq!(
                restored.to_canonical_json().unwrap(),
                canonical,
                "{} canonical bytes",
                sample.key,
            );

            let (workbench, editor) = open_with_editor(sample.key);
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
                sample.key,
            );
            let mobility = diagnostics
                .mobility
                .unwrap_or_else(|| panic!("{} mobility diagnostic", sample.key));
            assert_eq!(
                mobility.equality_degrees_of_freedom,
                Some(expected_raw_dof),
                "{} equality DOF agrees with its numerical right-nullity",
                sample.key,
            );
            assert_eq!(
                mobility.bidirectional_bounded_degrees_of_freedom,
                Some(expected_effective_dof),
                "{} effective bidirectional mobility",
                sample.key,
            );
            let design = accepted.session.design_document();
            assert!(
                !design.points().is_empty() && !design.curves().is_empty(),
                "{} opened an empty native canvas",
                sample.key,
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
            assert!(
                workbench
                    .session
                    .source_session()
                    .snapshot()
                    .expansion
                    .is_some()
            );
            assert_eq!(
                workbench
                    .session
                    .source_session()
                    .snapshot()
                    .accepted_editor_checkpoint,
                encode_editor_checkpoint(&editor).unwrap(),
            );
        }
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "one exhaustive sample matrix keeps edit, publication, finite-scene validation and exact Undo adjacent"
    )]
    fn all_bundled_samples_accept_one_semantic_seed_edit_and_exact_undo() {
        let samples = bundled_sample_catalog();
        let reviewed: serde_json::Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../geosolve-sketch-code/assets/bundled-sample-catalog.json"
        )))
        .expect("reviewed sample catalog contract");
        assert_eq!(reviewed["schema"], 1);
        assert_eq!(
            serde_json::json!(
                samples
                    .iter()
                    .map(|sample| serde_json::json!({
                        "key": sample.key, "title": sample.title, "category": sample.category,
                    }))
                    .collect::<Vec<_>>()
            ),
            reviewed["samples"],
            "runtime catalog must match independent reviewed order and metadata"
        );
        for (index, sample) in samples.iter().enumerate() {
            let (mut workbench, _editor) = open_with_editor(sample.key);
            let before = workbench.session.source_session().snapshot().clone();
            assert_eq!(
                before.interaction_overlay,
                CodeInteractionOverlay::empty(),
                "{} initial overlay",
                sample.key,
            );
            let materialized = workbench.session.accepted_materialization();
            let point = materialized
                .expansion
                .writable_points
                .iter()
                .find(|point| !point.source.is_reference())
                .or_else(|| materialized.expansion.writable_points.first())
                .cloned()
                .unwrap_or_else(|| panic!("{} writable semantic point", sample.key));
            let native_point = expanded_port_point(&materialized.editor, &point.handle)
                .unwrap_or_else(|| panic!("{} writable native point", sample.key));
            let position = materialized
                .editor
                .coordinator()
                .accepted_materialization()
                .and_then(|accepted| {
                    accepted
                        .session
                        .design_document()
                        .point(native_point)
                        .map(|point| point.position)
                })
                .unwrap_or_else(|| panic!("{} accepted writable position", sample.key));
            let scale = position[0].abs().max(position[1].abs()).max(1.0);
            let direction = if index % 2 == 0 { 1.0 } else { -1.0 };
            let target = [position[0] + direction * scale * 1.0e-8, position[1]];
            assert!(target.into_iter().all(f64::is_finite), "{}", sample.key);
            assert_ne!(pair_bits(target), pair_bits(position), "{}", sample.key);

            let overlay = workbench
                .session
                .source_session()
                .stage_point_drag(&point, target)
                .unwrap_or_else(|error| panic!("{} stage point seed: {error}", sample.key));
            assert_ne!(
                overlay, before.interaction_overlay,
                "{} edit must not qualify as a no-op",
                sample.key,
            );
            let next = materialize_code_project_incremental_with_overlay(
                materialized,
                &workbench.project,
                &workbench.session.source_session().snapshot().generated,
                &overlay,
            )
            .unwrap_or_else(|error| panic!("{} materialize point seed: {error}", sample.key));
            assert_ne!(
                next.expansion.patch,
                before
                    .accepted_expansion
                    .as_ref()
                    .expect("accepted expansion")
                    .patch,
                "{} staged instance seed must change the expanded design",
                sample.key,
            );
            let accepted = next
                .editor
                .coordinator()
                .accepted_materialization()
                .unwrap_or_else(|| panic!("{} edited accepted materialization", sample.key));
            assert!(
                accepted.validation.hard_residuals_validated,
                "{} independent hard validation",
                sample.key,
            );
            assert!(
                accepted.validation.all_active_features_current,
                "{} current computed features",
                sample.key,
            );
            assert!(
                accepted
                    .validation
                    .maximum_normalized_hard_residual
                    .is_none_or(|value| value.is_finite() && value <= 1.0e-9),
                "{} normalized hard residual",
                sample.key,
            );
            assert!(
                accepted
                    .session
                    .design_document()
                    .points()
                    .iter()
                    .flat_map(|point| point.position)
                    .chain(
                        accepted
                            .session
                            .design_document()
                            .scalars()
                            .iter()
                            .map(|scalar| scalar.value),
                    )
                    .all(f64::is_finite),
                "{} edited accepted geometry is finite",
                sample.key,
            );

            let expected = workbench.session.token().clone();
            workbench
                .session
                .apply_overlay(&expected, overlay.clone())
                .unwrap_or_else(|error| panic!("{} publish point seed: {error}", sample.key));
            assert_eq!(
                workbench
                    .session
                    .source_session()
                    .snapshot()
                    .interaction_overlay,
                overlay,
                "{} published overlay",
                sample.key,
            );
            assert_eq!(
                workbench.session.source_session().snapshot().managed,
                before.managed,
                "{} instance edit leaves source, IR, and executed artifact unchanged",
                sample.key,
            );
            assert_eq!(
                workbench.session.source_session().snapshot().code_project,
                before.code_project,
                "{} instance edit leaves pinned patch artifacts unchanged",
                sample.key,
            );
            workbench
                .session
                .step_source_history(true)
                .unwrap_or_else(|error| panic!("{} Undo: {error}", sample.key))
                .unwrap_or_else(|| panic!("{} Undo receipt", sample.key));
            assert_eq!(
                workbench.session.source_session().snapshot(),
                &before,
                "{} Undo restores the exact prior session snapshot",
                sample.key,
            );
            assert_eq!(
                workbench
                    .session
                    .source_session()
                    .snapshot()
                    .interaction_overlay,
                CodeInteractionOverlay::empty(),
                "{} Undo clears the representative edit",
                sample.key,
            );
        }
    }

    #[test]
    fn canonical_samples_use_four_semantic_groups_without_implementation_badges() {
        let markup = sample_group_markup(None);
        assert!(!markup.contains("Code &amp; reusable patches"));
        assert!(!markup.contains("Code projects"));
        assert!(!markup.contains("wb-code-sample-mark"));
        for group in [
            "Mechanisms",
            "Products &amp; fabrication",
            "Reference labs",
            "Scale studies",
        ] {
            assert!(markup.contains(group), "missing semantic group {group}");
        }
        assert_eq!(markup.matches("data-sample-group-trigger").count(), 4);
        let samples = geosolve_sketch_code::bundled_sample_catalog();
        assert_eq!(markup.matches("data-sample-id=").count(), samples.len());
        for sample in samples {
            assert_eq!(
                markup
                    .matches(&format!("data-sample-id=\"{}\"", sample.key))
                    .count(),
                1,
            );
        }
        assert!(!markup.contains("data-code-sample-id="));
    }

    #[test]
    fn managed_and_custom_files_have_truthful_distinct_ownership_surfaces() {
        let mut workbench = open("pc-water-manifold");
        let managed = workbench.panel_markup();
        assert!(managed.contains("data-code-file-kind=\"managed\""));
        assert!(managed.contains("data-code-action=\"apply\""));
        assert!(managed.contains("Executed managed sketch"));
        assert!(managed.contains("Managed controls"));
        assert!(managed.contains("data-code-control-id="));
        assert!(managed.contains("Modifiable in sketch.ts"));
        assert!(!managed.contains("data-code-lens-declaration="));
        assert!(!managed.contains("data-code-lens-path="));
        assert!(!managed.contains("Read-only in demo"));

        workbench
            .select_file("patches/water-channel.patch.ts")
            .unwrap();
        let custom = workbench.panel_markup();
        assert!(custom.contains("data-code-file-kind=\"custom\""));
        assert!(custom.contains("Read-only in demo"));
        assert!(custom.contains("waterChannel"));
        assert!(!custom.contains("id=\"wb-code-managed-source\""));
    }

    #[test]
    fn every_enabled_manifest_control_is_reachable_once_in_the_code_panel() {
        for sample in geosolve_sketch_code::bundled_sample_catalog() {
            let workbench = open(sample.key);
            let manifest = workbench
                .managed_controls()
                .unwrap_or_else(|error| panic!("{} manifest: {error}", sample.key));
            let markup = workbench.panel_markup();
            let enabled = manifest.editable().collect::<Vec<_>>();
            assert_eq!(
                markup.matches("data-code-control-id=").count(),
                enabled.len(),
                "{} must render every enabled control exactly once",
                sample.key,
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
                    sample.key,
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
        let mut workbench = open("mounting-plate");
        let persistence_before = workbench.to_persistence_json().unwrap();
        let project_before = workbench.project.to_canonical_json().unwrap();
        let expansion_before = serde_json::to_string(
            workbench
                .session
                .source_session()
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
                    .source_session()
                    .snapshot()
                    .accepted_expansion
                    .as_ref()
                    .unwrap(),
            )
            .unwrap(),
            expansion_before,
        );
        let reproduction =
            geosolve_constraint_editor::reproduction::encode_workspace(&persistence_before)
                .expect("encode exact code-workbench reproduction");
        let reproduction_workspace =
            geosolve_constraint_editor::reproduction::decode_workspace(&reproduction)
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
    fn m92_v5_session_transport_preserves_exact_session_and_readable_project() {
        let workbench = open("braced-frame");
        let json = workbench.to_persistence_json().unwrap();
        let wire: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(wire["version"], "geosolve-code-workbench-v5");
        assert_eq!(
            wire["project"].as_str().unwrap(),
            workbench.project.to_canonical_json().unwrap(),
        );
        assert_eq!(wire["managed_draft"], workbench.managed_draft);
        let encoded_session = wire["session"].as_str().unwrap();
        let session =
            geosolve_constraint_editor::reproduction::decode_workspace(encoded_session).unwrap();
        assert_eq!(
            session,
            workbench
                .session
                .source_session()
                .to_canonical_json()
                .unwrap()
        );
        assert!(encoded_session.len() < session.len());
        let restored = CodeProjectWorkbench::from_persistence_json(&json).unwrap();
        assert_eq!(restored.to_persistence_json().unwrap(), json);
    }

    fn persistence_history_workbench() -> CodeProjectWorkbench {
        let base = CompiledManagedSource::from_json(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../packages/geosolve-sketch-code/test/fixtures/managed-contact-range-base.json"
        )))
        .unwrap();
        let (mut workbench, _) =
            CodeProjectWorkbench::open_managed_test_compiled("m92-persistence-history", base)
                .unwrap();
        for (json, source) in [
            (
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../packages/geosolve-sketch-code/test/fixtures/managed-contact-supporting-line.json"
                )),
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../packages/geosolve-sketch-code/test/fixtures/managed-contact-supporting-line.sketch.ts"
                )),
            ),
            (
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../packages/geosolve-sketch-code/test/fixtures/managed-contact-range-limited.json"
                )),
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../packages/geosolve-sketch-code/test/fixtures/managed-contact-range-limited.sketch.ts"
                )),
            ),
        ] {
            let compiled = CompiledManagedSource::from_json(json).unwrap();
            publish_managed_source_fixture(&mut workbench, source, compiled);
        }
        workbench.step_history(true).unwrap().unwrap();
        assert!(workbench.can_undo() && workbench.can_redo());
        workbench
    }

    fn assert_persisted_native_authority(workbench: &CodeProjectWorkbench) {
        let editor = workbench.restore_accepted_editor().unwrap();
        assert_eq!(
            encode_editor_checkpoint(&editor).unwrap(),
            *workbench.accepted_editor_checkpoint(),
        );
        let accepted = editor.coordinator().accepted_materialization().unwrap();
        assert!(accepted.validation.hard_residuals_validated);
        assert!(accepted.validation.all_active_features_current);
        assert!(
            accepted
                .validation
                .maximum_normalized_hard_residual
                .is_none_or(|value| value.is_finite() && value <= 1.0e-9),
        );
        let document = accepted
            .session
            .accepted_state_for_current_input()
            .unwrap()
            .document();
        assert!(
            document
                .points()
                .iter()
                .flat_map(|point| point.position)
                .chain(document.scalars().iter().map(|scalar| scalar.value))
                .all(f64::is_finite),
        );
    }

    #[test]
    fn m92_v4_session_history_migrates_losslessly_to_v5() {
        let mut workbench = persistence_history_workbench();
        let persisted = workbench.to_persistence_json().unwrap();
        let session = workbench
            .session
            .source_session()
            .to_canonical_json()
            .unwrap();
        let project = workbench.project.to_canonical_json().unwrap();
        let mut legacy: serde_json::Value = serde_json::from_str(&persisted).unwrap();
        legacy["version"] = "geosolve-code-workbench-v4".into();
        legacy["session"] = session.clone().into();
        let mut migrated =
            CodeProjectWorkbench::from_persistence_json(&legacy.to_string()).unwrap();
        let mut restored = CodeProjectWorkbench::from_persistence_json(&persisted).unwrap();
        for candidate in [&migrated, &restored] {
            assert_eq!(candidate.to_persistence_json().unwrap(), persisted);
            assert_eq!(
                candidate
                    .session
                    .source_session()
                    .to_canonical_json()
                    .unwrap(),
                session
            );
            assert_eq!(candidate.project.to_canonical_json().unwrap(), project);
            assert_persisted_native_authority(candidate);
        }
        for undo in [false, true, true, false] {
            workbench.step_history(undo).unwrap().unwrap();
            for candidate in [&mut migrated, &mut restored] {
                candidate.step_history(undo).unwrap().unwrap();
                assert_eq!(
                    candidate.to_persistence_json().unwrap(),
                    workbench.to_persistence_json().unwrap(),
                );
                assert_persisted_native_authority(candidate);
            }
        }
    }

    #[test]
    fn m92_v5_session_transport_rejects_corruption_trailing_stream_and_oversize() {
        use base64::Engine as _;
        use base64::engine::general_purpose::URL_SAFE_NO_PAD;

        let workbench = open("braced-frame");
        let persisted = workbench.to_persistence_json().unwrap();
        let mut wire: serde_json::Value = serde_json::from_str(&persisted).unwrap();
        let payload = wire["session"].as_str().unwrap().to_owned();
        let fields = payload.split(':').map(str::to_owned).collect::<Vec<_>>();
        assert_eq!(fields.len(), 5);
        let mut cases = Vec::new();
        let mut corrupt = fields.clone();
        let checksum = u64::from_str_radix(&corrupt[3], 16).unwrap() ^ 1;
        corrupt[3] = format!("{checksum:016x}");
        cases.push(("checksum", corrupt.join(":")));
        let compressed = URL_SAFE_NO_PAD.decode(&fields[4]).unwrap();
        let mut truncated = fields.clone();
        truncated[4] = URL_SAFE_NO_PAD.encode(&compressed[..compressed.len() - 1]);
        cases.push(("truncated", truncated.join(":")));
        let mut trailing_bytes = compressed;
        trailing_bytes.push(0);
        let mut trailing = fields.clone();
        trailing[4] = URL_SAFE_NO_PAD.encode(trailing_bytes);
        cases.push(("trailing", trailing.join(":")));
        let mut oversized = fields.clone();
        oversized[2] = (geosolve_sketch_code::CODE_PROJECT_LIMIT + 1).to_string();
        cases.push(("decoded bound", oversized.join(":")));
        let mut over_expanding = fields;
        over_expanding[2] = "1".into();
        cases.push(("declared output bound", over_expanding.join(":")));
        for (label, payload) in cases {
            wire["session"] = payload.into();
            let error = CodeProjectWorkbench::from_persistence_json(&wire.to_string())
                .err()
                .unwrap_or_else(|| panic!("{label} must reject before restore"));
            assert!(
                error.contains("invalid compressed code session"),
                "{label}: {error}"
            );
        }
        assert_eq!(workbench.to_persistence_json().unwrap(), persisted);
    }

    #[test]
    fn m92_v5_session_transport_still_authenticates_current_undo_and_redo_checkpoints() {
        let workbench = persistence_history_workbench();
        let persisted = workbench.to_persistence_json().unwrap();
        let mut wire: serde_json::Value = serde_json::from_str(&persisted).unwrap();
        let canonical_session = workbench
            .session
            .source_session()
            .to_canonical_json()
            .unwrap();
        for path in ["/snapshot", "/undo/0/snapshot", "/redo/0/snapshot"] {
            let mut session: serde_json::Value = serde_json::from_str(&canonical_session).unwrap();
            let snapshot = session
                .pointer_mut(path)
                .expect("populated historical snapshot");
            let mut checkpoint: serde_json::Value =
                serde_json::from_str(snapshot["accepted_editor_checkpoint"].as_str().unwrap())
                    .unwrap();
            let digest = checkpoint["digest"].as_str().unwrap();
            checkpoint["digest"] = format!(
                "{}{}",
                if digest.starts_with('0') { '1' } else { '0' },
                &digest[1..]
            )
            .into();
            // Keep current/accepted checkpoint equality and a valid transport
            // checksum, so the delegated native authority validator must reject.
            snapshot["editor_checkpoint"] = checkpoint.to_string().into();
            snapshot["accepted_editor_checkpoint"] = snapshot["editor_checkpoint"].clone();
            let session = session.to_string();
            let payload =
                geosolve_constraint_editor::reproduction::encode_workspace(&session).unwrap();
            assert_eq!(
                geosolve_constraint_editor::reproduction::decode_workspace(&payload).unwrap(),
                session
            );
            wire["session"] = payload.into();
            let error = CodeProjectWorkbench::from_persistence_json(&wire.to_string())
                .err()
                .unwrap_or_else(|| panic!("tampered {path} native authority must reject"));
            assert!(error.contains("digest"), "{path}: {error}");
        }
        assert_eq!(workbench.to_persistence_json().unwrap(), persisted);
    }

    #[test]
    fn complete_offline_project_session_draft_and_file_selection_round_trip() {
        let mut workbench = open("pc-water-manifold");
        workbench
            .select_file("patches/water-channel.patch.ts")
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
            "patches/water-channel.patch.ts"
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
        let session = geosolve_constraint_editor::reproduction::decode_workspace(
            wire["session"].as_str().unwrap(),
        )
        .unwrap();
        let tampered =
            session.replace("geosolve-demo-braced-frame", "geosolve-demo-tampered-frame");
        assert_ne!(tampered, session);
        for version in [
            LEGACY_CODE_WORKBENCH_WIRE_VERSION,
            CODE_WORKBENCH_WIRE_VERSION,
        ] {
            wire["version"] = version.into();
            wire["session"] = if version == LEGACY_CODE_WORKBENCH_WIRE_VERSION {
                tampered.clone().into()
            } else {
                geosolve_constraint_editor::reproduction::encode_workspace(&tampered)
                    .unwrap()
                    .into()
            };
            assert!(CodeProjectWorkbench::from_persistence_json(&wire.to_string()).is_err());
        }
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
