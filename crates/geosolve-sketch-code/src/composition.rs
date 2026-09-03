// SPDX-License-Identifier: GPL-3.0-or-later

//! Cold, transactional composition of structural code expansion with the
//! ordinary projectional editor and native computed-feature authoring paths.

use std::collections::{BTreeMap, BTreeSet};

use geosolve_constraint_editor::{
    ComputedCornerRef, ComputedFeatureDefinition, FeatureAuthoringCandidate,
    FeatureAuthoringOptions, FeatureAuthoringOutcome, FeatureAuthoringState, FeatureAuthoringTool,
    IntentNativeBinding, PreparedIntentOperationPlan, ProjectionalEditorError,
    ProjectionalEditorSession, ProjectionalPatchOutcome, SelectionItem,
    prepare_intent_operation_output_plan, projectional_fillet_patch,
};
use geosolve_sketch::{DocumentId, SketchDocument};
use geosolve_sketch_intent::{
    DeletePolicy, InputRole, InputSlot, IntentAliasMap, IntentKey, IntentKeyError, IntentLiteral,
    IntentNode, IntentNodeDraft, IntentPatch, IntentPatchOperation, IntentPatchPolicy,
    IntentPlanDisposition, IntentPlanError, IntentPortKind, IntentPortRef, IntentPortRole,
    IntentPortSelector, IntentSession, IntentSessionId, IntentSessionIdentity, LeafRef, NodeId,
    PatchPortRef, intent_content_digest,
};
use thiserror::Error;

use crate::expansion::{CodeOperationPlanner, expand_code_project_with_overlay_and_planner};
use crate::{
    AuditedCodeWork, CodeExpansionError, CodeHostRequest, CodeInteractionOverlay, CodeProject,
    CodeWorkReceipt, ExpandedCodeProject, ExpandedFeatureCorner, GeneratedMemberAddress,
    GeneratedMemberIdentity, KeyedFilletHostRequest, KeyedReconcileState,
};

/// One exact native computed output produced for a generated member.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MaterializedFilletOutput {
    pub identity: GeneratedMemberIdentity,
    pub member_key: Vec<String>,
    pub owner: ComputedCornerRef,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct HostMemberKey {
    address: GeneratedMemberAddress,
    member_key: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
struct HostMemberRequest {
    key: HostMemberKey,
    request: KeyedFilletHostRequest,
    suffix: Option<String>,
}

struct PreparedHostFillet {
    member: HostMemberRequest,
    symbol: IntentKey,
    candidate: FeatureAuthoringCandidate,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum OutsideDependentPolicy {
    Reject,
    CascadeInOracle,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum IncrementalPatchMode {
    Native,
    Delegated,
}

/// A cold code project composed through the unchanged projectional editor.
///
/// The editor is returned as the sole native/intent authority. `expansion`
/// remains the equation-free source/provenance projection, while
/// `host_outputs` authenticates generated Fillet addresses against the native
/// persistent feature/corner IDs allocated by ordinary authoring.
#[derive(Debug)]
pub struct MaterializedCodeProject {
    pub editor: ProjectionalEditorSession,
    pub expansion: ExpandedCodeProject,
    pub base_outcome: ProjectionalPatchOutcome,
    pub host_outputs: BTreeMap<GeneratedMemberAddress, Vec<MaterializedFilletOutput>>,
}

/// Stack-safe cold-composition carrier used only while nested lowering and
/// independent validation frames are live.
struct ColdMaterializedCodeProject {
    editor: Box<ProjectionalEditorSession>,
    expansion: ExpandedCodeProject,
    base_outcome: ProjectionalPatchOutcome,
    host_outputs: BTreeMap<GeneratedMemberAddress, Vec<MaterializedFilletOutput>>,
}

impl ColdMaterializedCodeProject {
    fn into_public(self) -> MaterializedCodeProject {
        MaterializedCodeProject {
            editor: *self.editor,
            expansion: self.expansion,
            base_outcome: self.base_outcome,
            host_outputs: self.host_outputs,
        }
    }
}

/// Fail-closed structural/native composition diagnostic.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum CodeCompositionError {
    #[error(transparent)]
    Expansion(#[from] CodeExpansionError),
    #[error(transparent)]
    Editor(#[from] ProjectionalEditorError),
    #[error("base code patch rejected: {0}")]
    BaseEditor(String),
    #[error("generated host member `{member}` rejected: {diagnostic}")]
    HostEditor { member: String, diagnostic: String },
    #[error(transparent)]
    Key(#[from] IntentKeyError),
    #[error("code host identity could not be encoded: {0}")]
    Encoding(String),
    #[error("base code expansion did not publish accepted native authority")]
    BaseNotAccepted,
    #[error("expanded alias `{alias}` did not resolve to a stable intent port")]
    MissingAlias { alias: String },
    #[error("expanded semantic corner `{reference}` has no accepted native binding")]
    MissingNativeBinding { reference: String },
    #[error("expanded semantic corner `{reference}` resolved to {actual}, not {expected}")]
    NativeKindMismatch {
        reference: String,
        expected: &'static str,
        actual: &'static str,
    },
    #[error("native Fillet authoring for `{member}` did not produce a complete preview: {outcome}")]
    HostPreviewIncomplete { member: String, outcome: String },
    #[error("native Fillet preview for `{member}` did not retain its exact expanded parent spans")]
    HostParentMismatch { member: String },
    #[error("native Fillet declaration `{member}` has incomplete ownership evidence")]
    HostOwnershipMismatch { member: String },
    #[error("generated host output `{0}` was materialized more than once")]
    DuplicateHostOutput(String),
    #[error("incremental composition cannot remove prior declaration `{symbol}` yet")]
    IncrementalRemovalUnsupported { symbol: String },
    #[error("incremental declaration `{symbol}` has an unsupported retained schema change")]
    IncrementalSchemaChange { symbol: String },
    #[error("incremental replacement of `{symbol}` would also delete outside dependent nodes")]
    IncrementalOutsideDependent { symbol: String },
    #[error("incremental host member `{member}` changed in place and requires native re-authoring")]
    IncrementalHostChange { member: String },
    #[error("incremental expansion contained a non-create operation")]
    IncrementalUnexpectedOperation,
    #[error("expanded code payload does not match its authenticated digest")]
    ExpansionDigestMismatch,
    #[error(
        "expanded code patch belongs to intent session {actual}; restored native authority belongs to {expected}"
    )]
    ExpansionSessionMismatch { expected: String, actual: String },
    #[error("restored base declaration `{symbol}` does not match the expanded code payload")]
    RehydratedBaseMismatch { symbol: String },
    #[error("restored operation declaration `{symbol}` does not match its native output inventory")]
    RehydratedOperationPlanMismatch { symbol: String },
    #[error("restored generated host member `{member}` does not match the expanded code payload")]
    RehydratedHostMismatch { member: String },
}

struct NativeCodeOperationPlanner {
    expected: IntentSessionIdentity,
    document: DocumentId,
    model_scale: f64,
}

impl CodeOperationPlanner for NativeCodeOperationPlanner {
    fn prepare(
        &mut self,
        prefix: &[IntentPatchOperation],
        alias: &IntentKey,
        provisional: &IntentNodeDraft,
    ) -> Result<PreparedIntentOperationPlan, CodeExpansionError> {
        let mut operations = prefix.to_vec();
        operations.push(IntentPatchOperation::CreateNode {
            alias: alias.clone(),
            draft: Box::new(provisional.clone()),
            cell: None,
        });
        prepare_intent_operation_output_plan(
            self.expected,
            &operations,
            &provisional.symbol,
            self.document,
            self.model_scale,
        )
        .map_err(|error| CodeExpansionError::OperationPlanning {
            declaration: provisional.symbol.to_string(),
            message: error.to_string(),
        })
    }
}

/// Cold-expands and independently materializes one complete code project.
///
/// Custom TypeScript is not executed here. The function consumes only the
/// already validated managed CST and pinned data-only artifacts, applies the
/// resulting ordinary patch through [`ProjectionalEditorSession`], then routes
/// every requested Fillet through that editor's existing branch-explicit
/// native authoring API.
///
/// # Errors
///
/// Fails transaction-locally for invalid expansion/reconciliation, a rejected
/// native solve, stale/mismatched aliases, unsupported host geometry, or
/// incomplete native ownership evidence. Since the editor is constructed
/// inside this function, no caller-owned accepted authority can be partially
/// mutated on failure.
pub fn materialize_code_project_cold(
    project: &CodeProject,
    reconciliation: &KeyedReconcileState,
    intent_session: IntentSessionId,
    document: DocumentId,
    model_scale: f64,
) -> Result<MaterializedCodeProject, CodeCompositionError> {
    materialize_code_project_cold_with_overlay(
        project,
        reconciliation,
        &CodeInteractionOverlay::empty(),
        intent_session,
        document,
        model_scale,
    )
}

/// Cold materializes one code project after applying a semantic GUI seed
/// overlay. The overlay changes only intent instance leaves.
///
/// # Errors
///
/// Returns a typed expansion, reconciliation, editor, host-authoring, or
/// independent native-validation error without publishing partial authority.
pub fn materialize_code_project_cold_with_overlay(
    project: &CodeProject,
    reconciliation: &KeyedReconcileState,
    overlay: &CodeInteractionOverlay,
    intent_session: IntentSessionId,
    document: DocumentId,
    model_scale: f64,
) -> Result<MaterializedCodeProject, CodeCompositionError> {
    let intent = fresh_intent_session(intent_session)?;
    let expansion = expand_code_project_with_overlay_and_planner(
        project,
        reconciliation,
        overlay,
        intent.identity(),
        &mut NativeCodeOperationPlanner {
            expected: intent.identity(),
            document,
            model_scale,
        },
    )?;
    Ok(
        materialize_expanded_code_project_cold(expansion, intent, document, model_scale)?
            .into_public(),
    )
}

fn fresh_intent_session(
    intent_session: IntentSessionId,
) -> Result<IntentSession, CodeCompositionError> {
    IntentSession::with_id(intent_session).map_err(|error| {
        ProjectionalEditorError::Coordinator(
            geosolve_constraint_editor::ProjectionalCoordinatorError::Intent(error),
        )
        .into()
    })
}

fn restore_cold_editor(
    intent: IntentSession,
    document: DocumentId,
    model_scale: f64,
    pristine_empty: bool,
) -> Result<Box<ProjectionalEditorSession>, CodeCompositionError> {
    let editor = if pristine_empty {
        ProjectionalEditorSession::restore_pristine_empty(intent, document, model_scale)?
    } else {
        ProjectionalEditorSession::restore(intent, document, model_scale)?
    };
    Ok(Box::new(editor))
}

/// Materializes one already expanded project through an independent fresh
/// Intent/native authority. Keeping expansion outside this helper lets the
/// incremental composer select its warm unchanged-host path without parsing
/// or lowering the same project a second time.
fn materialize_expanded_code_project_cold(
    expansion: ExpandedCodeProject,
    intent: IntentSession,
    document: DocumentId,
    model_scale: f64,
) -> Result<ColdMaterializedCodeProject, CodeCompositionError> {
    // Cold lowering nests through intent planning, retained document solving,
    // and independent validation. Keep the large editor authority on the heap
    // while those shared frames are live; its public value shape is preserved
    // when the completed project moves into the caller's return place.
    let empty_patch = expansion.patch.operations().is_empty();
    let mut editor = restore_cold_editor(intent, document, model_scale, empty_patch)?;
    // A managed project may legitimately become empty after deleting its
    // final declaration. The generic intent API still rejects caller-issued
    // no-op patches; composition alone recognizes its authenticated empty
    // expansion as an already-accepted base over the restored empty document.
    let base_outcome = if empty_patch {
        ProjectionalPatchOutcome {
            identity: editor.coordinator().intent().identity(),
            disposition: IntentPlanDisposition::Accepted,
            aliases: IntentAliasMap::default(),
        }
    } else {
        editor
            .apply_patch(expansion.patch.clone())
            .map_err(|error| CodeCompositionError::BaseEditor(error.to_string()))?
    };
    if base_outcome.disposition != IntentPlanDisposition::Accepted {
        return Err(CodeCompositionError::BaseNotAccepted);
    }

    let host_outputs =
        materialize_host_requests(&mut editor, &base_outcome.aliases, &expansion.host_requests)?;

    let accepted = editor
        .coordinator()
        .accepted_materialization()
        .ok_or(CodeCompositionError::BaseNotAccepted)?;
    if !accepted.validation.hard_residuals_validated
        || !accepted.validation.all_active_features_current
        || accepted
            .validation
            .maximum_normalized_hard_residual
            .is_some_and(|value| !value.is_finite() || value > 1.0e-9)
    {
        return Err(CodeCompositionError::BaseNotAccepted);
    }

    Ok(ColdMaterializedCodeProject {
        editor,
        expansion,
        base_outcome,
        host_outputs,
    })
}

/// Reattaches one restored projectional editor to its already authenticated
/// equation-free expansion without allocating or publishing anything.
///
/// This is the warm-cache restoration boundary used after code-project
/// persistence, Undo, or Redo. It does not parse managed source, expand a
/// patch, execute TypeScript, invoke authoring, or mutate the supplied editor.
/// Instead it authenticates the stored expansion digest and intent-session
/// domain, reconstructs transaction aliases from durable declaration symbols,
/// and recovers generated Fillet ownership from the accepted native sidecar.
///
/// The boxed boundary keeps the large retained editor off test-thread stacks
/// while ownership moves from persistence into the code composition cache.
///
/// # Errors
///
/// Fails closed when the expansion digest/session, any base declaration,
/// stable input or writable seed, or any generated Fillet owner/radius/parent
/// span disagrees with the independently accepted restored authority.
pub fn rehydrate_materialized_code_project(
    editor: Box<ProjectionalEditorSession>,
    expansion: ExpandedCodeProject,
) -> Result<Box<MaterializedCodeProject>, CodeCompositionError> {
    validate_native_authority(&editor)?;
    authenticate_expansion_envelope(&editor, &expansion)?;
    authenticate_rehydrated_operation_plans(&editor, &expansion)?;
    let aliases = rehydrate_base_aliases(&editor, &expansion)?;
    let host_outputs = rehydrate_host_outputs(&editor, &aliases, &expansion.host_requests)?;
    let base_outcome = ProjectionalPatchOutcome {
        identity: editor.coordinator().intent().identity(),
        disposition: IntentPlanDisposition::Accepted,
        aliases,
    };
    Ok(Box::new(MaterializedCodeProject {
        editor: *editor,
        expansion,
        base_outcome,
        host_outputs,
    }))
}

#[allow(
    clippy::too_many_lines,
    reason = "coverage, source-order prefixes, native reprobes, and retained plan equality are authenticated together"
)]
fn authenticate_rehydrated_operation_plans(
    editor: &ProjectionalEditorSession,
    expansion: &ExpandedCodeProject,
) -> Result<(), CodeCompositionError> {
    let accepted = editor
        .coordinator()
        .accepted_materialization()
        .ok_or(CodeCompositionError::BaseNotAccepted)?;
    let document = accepted.session.design_document();
    let mut creates = BTreeMap::<IntentKey, IntentPatchOperation>::new();
    let mut native_operation_aliases = BTreeSet::<IntentKey>::new();
    for operation in expansion.patch.operations() {
        let IntentPatchOperation::CreateNode { alias, draft, .. } = operation else {
            return Err(CodeCompositionError::IncrementalUnexpectedOperation);
        };
        if creates.insert(alias.clone(), operation.clone()).is_some() {
            return Err(CodeCompositionError::RehydratedOperationPlanMismatch {
                symbol: draft.symbol.to_string(),
            });
        }
        if matches!(
            draft.kind,
            geosolve_sketch_intent::IntentNodeKind::Operation { .. }
        ) {
            native_operation_aliases.insert(alias.clone());
        }
    }

    let retained_operation_aliases = expansion
        .operation_plans
        .iter()
        .filter_map(|stored| stored.prefix_aliases.last().cloned())
        .collect::<BTreeSet<_>>();
    if retained_operation_aliases.len() != expansion.operation_plans.len()
        || retained_operation_aliases != native_operation_aliases
    {
        let symbol = native_operation_aliases
            .symmetric_difference(&retained_operation_aliases)
            .next()
            .or_else(|| retained_operation_aliases.iter().next())
            .map_or_else(|| "operation-plan-coverage".to_owned(), ToString::to_string);
        return Err(CodeCompositionError::RehydratedOperationPlanMismatch { symbol });
    }

    let mut prior_prefix = Vec::<IntentKey>::new();
    for stored in &expansion.operation_plans {
        if stored.prefix_aliases.len() <= prior_prefix.len()
            || stored.prefix_aliases[..prior_prefix.len()] != prior_prefix
        {
            return Err(CodeCompositionError::RehydratedOperationPlanMismatch {
                symbol: stored.symbol.to_string(),
            });
        }
        let mut unique = BTreeSet::new();
        let mut probe = Vec::with_capacity(stored.prefix_aliases.len());
        for alias in &stored.prefix_aliases {
            if !unique.insert(alias) {
                return Err(CodeCompositionError::RehydratedOperationPlanMismatch {
                    symbol: stored.symbol.to_string(),
                });
            }
            probe.push(creates.get(alias).cloned().ok_or_else(|| {
                CodeCompositionError::RehydratedOperationPlanMismatch {
                    symbol: stored.symbol.to_string(),
                }
            })?);
        }
        let IntentPatchOperation::CreateNode {
            draft: provisional, ..
        } = probe
            .last_mut()
            .expect("a retained operation prefix includes its operation")
        else {
            unreachable!("the canonical patch lookup contains only create-node operations")
        };
        let geosolve_sketch_intent::IntentNodeKind::Operation {
            operation: operation_kind,
        } = provisional.kind
        else {
            return Err(CodeCompositionError::RehydratedOperationPlanMismatch {
                symbol: stored.symbol.to_string(),
            });
        };
        if provisional.symbol != stored.symbol || stored.plan.operation != operation_kind {
            return Err(CodeCompositionError::RehydratedOperationPlanMismatch {
                symbol: stored.symbol.to_string(),
            });
        }

        provisional.operation_outputs.clear();
        let regenerated = prepare_intent_operation_output_plan(
            expansion.patch.expected,
            &probe,
            &stored.symbol,
            document.id(),
            document.model_scale(),
        )
        .map_err(|_| CodeCompositionError::RehydratedOperationPlanMismatch {
            symbol: stored.symbol.to_string(),
        })?;
        if regenerated != stored.plan {
            return Err(CodeCompositionError::RehydratedOperationPlanMismatch {
                symbol: stored.symbol.to_string(),
            });
        }
        prior_prefix.clone_from(&stored.prefix_aliases);
    }
    Ok(())
}

fn authenticate_expansion_envelope(
    editor: &ProjectionalEditorSession,
    expansion: &ExpandedCodeProject,
) -> Result<(), CodeCompositionError> {
    let provenance_rows = expansion.generated_provenance.iter().collect::<Vec<_>>();
    let declaration_rows = expansion.declaration_provenance.iter().collect::<Vec<_>>();
    let bytes = serde_json::to_vec(&(
        &expansion.patch,
        &expansion.semantic_outputs,
        &provenance_rows,
        &declaration_rows,
        &expansion.writable_points,
        &expansion.generated_children,
        &expansion.host_requests,
        &expansion.operation_plans,
    ))
    .map_err(|error| CodeCompositionError::Encoding(error.to_string()))?;
    if intent_content_digest(&bytes).to_string() != expansion.digest {
        return Err(CodeCompositionError::ExpansionDigestMismatch);
    }
    let expected = editor.coordinator().intent().identity().session;
    let actual = expansion.patch.expected.session;
    if actual != expected {
        return Err(CodeCompositionError::ExpansionSessionMismatch {
            expected: expected.to_string(),
            actual: actual.to_string(),
        });
    }
    Ok(())
}

fn rehydrate_base_aliases(
    editor: &ProjectionalEditorSession,
    expansion: &ExpandedCodeProject,
) -> Result<IntentAliasMap, CodeCompositionError> {
    let intent = editor.coordinator().intent();
    let graph = intent.graph();
    let drafts = create_drafts(expansion)?;
    let mut aliases = IntentAliasMap::default();
    for (alias, draft) in &drafts {
        let node = graph.node_by_symbol(&draft.symbol).ok_or_else(|| {
            CodeCompositionError::RehydratedBaseMismatch {
                symbol: draft.symbol.to_string(),
            }
        })?;
        if aliases.nodes.insert(alias.clone(), node.id).is_some() {
            return Err(CodeCompositionError::RehydratedBaseMismatch {
                symbol: draft.symbol.to_string(),
            });
        }
        let ports = aliases.ports.entry(alias.clone()).or_default();
        for port in node.ports.values() {
            if ports.insert(port.selector, port.as_ref(node.id)).is_some() {
                return Err(CodeCompositionError::RehydratedBaseMismatch {
                    symbol: draft.symbol.to_string(),
                });
            }
        }
    }
    for (_, draft) in drafts {
        authenticate_rehydrated_base_node(intent, &aliases, draft)?;
    }
    Ok(aliases)
}

fn authenticate_rehydrated_base_node(
    intent: &IntentSession,
    aliases: &IntentAliasMap,
    draft: &IntentNodeDraft,
) -> Result<(), CodeCompositionError> {
    let mismatch = || CodeCompositionError::RehydratedBaseMismatch {
        symbol: draft.symbol.to_string(),
    };
    let node = intent
        .graph()
        .node_by_symbol(&draft.symbol)
        .ok_or_else(mismatch)?;
    if !retained_schema_compatible(node, draft)
        || node.fields != draft.fields
        || node.suppressed != draft.suppressed
        || intent.organization().node_names().get(&node.id) != Some(&draft.name)
    {
        return Err(mismatch());
    }
    for (slot, source) in &draft.inputs {
        let expected = match source {
            PatchPortRef::Stable { port } => {
                intent.graph().port(*port).ok_or_else(mismatch)?;
                *port
            }
            PatchPortRef::Alias { node, selector } => {
                aliases.port(node, *selector).ok_or_else(mismatch)?
            }
        };
        if node.inputs.get(slot) != Some(&expected) {
            return Err(mismatch());
        }
    }
    for (selector, values) in &draft.initial_instance {
        let port = node.port_by_selector(*selector).ok_or_else(mismatch)?;
        for (field, value) in values {
            let leaf = LeafRef {
                node: node.id,
                port: port.id,
                field: *field,
            };
            if intent.instance().values().get(&leaf) != Some(value) {
                return Err(mismatch());
            }
        }
    }
    Ok(())
}

fn rehydrate_host_outputs(
    editor: &ProjectionalEditorSession,
    aliases: &IntentAliasMap,
    requests: &[CodeHostRequest],
) -> Result<BTreeMap<GeneratedMemberAddress, Vec<MaterializedFilletOutput>>, CodeCompositionError> {
    let members = expand_host_members(requests)?;
    let mut outputs = BTreeMap::<GeneratedMemberAddress, Vec<MaterializedFilletOutput>>::new();
    for member in members.values() {
        let symbol = host_symbol(
            &member.key.address,
            member.request.identity,
            member.suffix.as_deref(),
        )?;
        let output = materialized_fillet_output(editor, &symbol, &member.request)?;
        authenticate_rehydrated_fillet(editor, aliases, &member.request, output.owner)?;
        outputs
            .entry(member.key.address.clone())
            .or_default()
            .push(output);
    }
    for values in outputs.values_mut() {
        values.sort_by(|left, right| left.member_key.cmp(&right.member_key));
    }
    Ok(outputs)
}

fn authenticate_rehydrated_fillet(
    editor: &ProjectionalEditorSession,
    aliases: &IntentAliasMap,
    request: &KeyedFilletHostRequest,
    owner: ComputedCornerRef,
) -> Result<(), CodeCompositionError> {
    let mismatch = || CodeCompositionError::RehydratedHostMismatch {
        member: request.output.display_path(),
    };
    if !request.radius.value.is_finite() || request.radius.value <= 0.0 {
        return Err(mismatch());
    }
    let (_, incoming, outgoing) = resolve_corner(editor, aliases, &request.corner)?;
    let accepted = editor
        .coordinator()
        .accepted_materialization()
        .ok_or(CodeCompositionError::BaseNotAccepted)?;
    let feature = accepted
        .features
        .feature(owner.feature)
        .ok_or_else(mismatch)?;
    let ComputedFeatureDefinition::FilletSet(fillet) = &feature.definition;
    let corner = accepted
        .features
        .corner(owner.feature, owner.corner)
        .ok_or_else(mismatch)?;
    if feature.suppressed != request.suppressed
        || fillet.corners.len() != 1
        || fillet.radius.to_bits() != request.radius.value.to_bits()
        || BTreeSet::from([corner.first.source.span, corner.second.source.span])
            != BTreeSet::from([incoming, outgoing])
    {
        return Err(mismatch());
    }
    Ok(())
}

/// Incrementally expands one code project over an already accepted native
/// composition while retaining every structurally compatible declaration and
/// its stable ports, reservations, and native ownership.
///
/// Expansion runs exactly once. When native host requests are unchanged, their
/// authenticated drafts and outputs are retained while one unordered ordinary
/// patch is solved transactionally on a fork of prior accepted authority.
/// Host-structural edits retain the complete cold branch/data oracle before
/// warm reconciliation. Compatible nodes, ports, reservations and native
/// owners survive exactly; removals and changed Fillet corners are therefore
/// never exposed as invalid intermediate scenes.
///
/// # Errors
///
/// Returns the ordinary expansion/materialization diagnostics, or a typed
/// incremental-reconciliation rejection. The supplied accepted composition is
/// borrowed and remains unchanged on every failure.
#[allow(
    clippy::too_many_lines,
    reason = "one fail-closed warm transaction keeps retained base and native host reconciliation adjacent"
)]
pub fn materialize_code_project_incremental(
    previous: &MaterializedCodeProject,
    project: &CodeProject,
    reconciliation: &KeyedReconcileState,
) -> Result<MaterializedCodeProject, CodeCompositionError> {
    materialize_code_project_incremental_with_overlay(
        previous,
        project,
        reconciliation,
        &CodeInteractionOverlay::empty(),
    )
}

/// Incrementally materializes an explicit structural edit after projecting
/// the prior interaction overlay onto still-present, provenance-compatible
/// owners. The returned overlay must be published with the returned expansion
/// in the same outer code-session transaction.
///
/// # Errors
///
/// Returns a typed expansion, projection, incremental-composition, host, or
/// native-validation error without changing `previous`.
pub fn materialize_code_project_incremental_for_structural_edit(
    previous: &MaterializedCodeProject,
    project: &CodeProject,
    reconciliation: &KeyedReconcileState,
    current_overlay: &CodeInteractionOverlay,
) -> Result<(MaterializedCodeProject, CodeInteractionOverlay), CodeCompositionError> {
    let accepted = previous
        .editor
        .coordinator()
        .accepted_materialization()
        .ok_or(CodeCompositionError::BaseNotAccepted)?;
    let document = accepted.session.design_document();
    // Native operation result planning is a cold, non-publishing probe.  The
    // accepted retained session has necessarily advanced beyond revision
    // zero, so using its identity here makes every structural edit in a
    // project with an operation fail the planner's pristine-session guard.
    // Recreate only the session namespace for this overlay-free preflight;
    // the accepted editor remains the authority used by the actual
    // incremental materialization below.
    let cold_intent =
        fresh_intent_session(previous.editor.coordinator().intent().identity().session)?;
    let expected = cold_intent.identity();
    let overlay_free = expand_code_project_with_overlay_and_planner(
        project,
        reconciliation,
        &CodeInteractionOverlay::empty(),
        expected,
        &mut NativeCodeOperationPlanner {
            expected,
            document: document.id(),
            model_scale: document.model_scale(),
        },
    )?;
    let retained = overlay_free.retained_overlay(current_overlay);
    let materialized = materialize_code_project_incremental_with_overlay_and_work(
        previous,
        project,
        reconciliation,
        &retained,
        None,
        &mut CodeWorkReceipt::default(),
    )?;
    Ok((materialized, retained))
}

/// Incremental counterpart of [`materialize_code_project_cold_with_overlay`].
///
/// # Errors
///
/// Returns a typed expansion, projection, incremental-composition, host, or
/// native-validation error without changing `previous`.
pub fn materialize_code_project_incremental_with_overlay(
    previous: &MaterializedCodeProject,
    project: &CodeProject,
    reconciliation: &KeyedReconcileState,
    overlay: &CodeInteractionOverlay,
) -> Result<MaterializedCodeProject, CodeCompositionError> {
    // Call the common worker directly. Wrapping this large retained result in
    // `AuditedCodeWork` only to move it straight back out adds avoidable stack
    // pressure to structural code-edit transactions.
    materialize_code_project_incremental_with_overlay_and_work(
        previous,
        project,
        reconciliation,
        overlay,
        None,
        &mut CodeWorkReceipt::default(),
    )
}

/// Audited counterpart of
/// [`materialize_code_project_incremental_with_overlay`].
///
/// The receipt records entry into the deterministic expansion boundary even
/// when later native composition rejects. Merely holding a code project or
/// calling a stale outer API therefore cannot be mistaken for code work.
pub fn materialize_code_project_incremental_with_overlay_audited(
    previous: &MaterializedCodeProject,
    project: &CodeProject,
    reconciliation: &KeyedReconcileState,
    overlay: &CodeInteractionOverlay,
) -> AuditedCodeWork<Result<MaterializedCodeProject, CodeCompositionError>> {
    let mut work = CodeWorkReceipt::default();
    let outcome = materialize_code_project_incremental_with_overlay_and_work(
        previous,
        project,
        reconciliation,
        overlay,
        None,
        &mut work,
    );
    AuditedCodeWork::new(outcome, work)
}

/// Incrementally materializes one semantic overlay while certifying the exact
/// accepted numerical continuation produced by its authenticated native
/// preview. Only the overlay contributes source edits; the continuation is a
/// topology-checked, independently validated solve seed with no drag request.
///
/// # Errors
///
/// Returns the ordinary expansion/composition error, or rejects a foreign,
/// topologically incompatible, or invalid accepted continuation.
pub fn materialize_code_project_incremental_with_overlay_and_accepted_continuation_audited(
    previous: &MaterializedCodeProject,
    project: &CodeProject,
    reconciliation: &KeyedReconcileState,
    overlay: &CodeInteractionOverlay,
    accepted_continuation: &SketchDocument,
) -> AuditedCodeWork<Result<MaterializedCodeProject, CodeCompositionError>> {
    let mut work = CodeWorkReceipt::default();
    let outcome = materialize_code_project_incremental_with_overlay_and_work(
        previous,
        project,
        reconciliation,
        overlay,
        Some(accepted_continuation),
        &mut work,
    );
    AuditedCodeWork::new(outcome, work)
}

fn apply_incremental_project_patch(
    editor: &mut ProjectionalEditorSession,
    patch: IntentPatch,
    expansion: &ExpandedCodeProject,
    accepted_continuation: Option<&SketchDocument>,
    mode: IncrementalPatchMode,
) -> Result<ProjectionalPatchOutcome, CodeCompositionError> {
    if patch.operations().is_empty() {
        return Ok(ProjectionalPatchOutcome {
            identity: editor.coordinator().intent().identity(),
            disposition: IntentPlanDisposition::Accepted,
            aliases: IntentAliasMap::default(),
        });
    }
    let contact_range_edits = authored_contact_range_edits(editor, &patch, expansion);
    let outcome = match (accepted_continuation, mode) {
        (Some(continuation), _) => {
            editor.apply_delegated_patch_with_accepted_continuation(patch, continuation)
        }
        (None, IncrementalPatchMode::Delegated) => editor.apply_delegated_patch(patch),
        (None, IncrementalPatchMode::Native) => editor.apply_patch(patch),
    };
    outcome.map_err(|error| {
        let diagnostic = actionable_contact_range_rejection(&contact_range_edits, &error)
            .unwrap_or_else(|| error.to_string());
        CodeCompositionError::BaseEditor(diagnostic)
    })
}

#[derive(Clone, Debug, PartialEq)]
struct AuthoredContactRangeEdit {
    node: NodeId,
    contact: String,
    lower: f64,
    upper: f64,
}

fn authored_contact_range_edits(
    editor: &ProjectionalEditorSession,
    patch: &IntentPatch,
    expansion: &ExpandedCodeProject,
) -> Vec<AuthoredContactRangeEdit> {
    let graph = editor.coordinator().intent().graph();
    let mut changed = BTreeMap::<NodeId, BTreeSet<String>>::new();
    for operation in patch.operations() {
        let (node, field) = match operation {
            IntentPatchOperation::SetDefinitionField { node, field, .. }
            | IntentPatchOperation::UnsetDefinitionField { node, field } => (*node, field),
            _ => continue,
        };
        if let Some(prefix) = field.0.as_str().strip_suffix("_range_lower") {
            changed.entry(node).or_default().insert(prefix.to_owned());
        }
        if let Some(prefix) = field.0.as_str().strip_suffix("_range_upper") {
            changed.entry(node).or_default().insert(prefix.to_owned());
        }
    }

    let mut edits = Vec::new();
    for (node_id, prefixes) in changed {
        let Some(node) = graph.node(node_id) else {
            continue;
        };
        for prefix in prefixes {
            let lower_field = format!("{prefix}_range_lower");
            let upper_field = format!("{prefix}_range_upper");
            let lower = patched_definition_quantity(node, patch, &lower_field);
            let upper = patched_definition_quantity(node, patch, &upper_field);
            let (Some(lower), Some(upper)) = (lower, upper) else {
                continue;
            };
            let declaration = expansion
                .declaration_for_alias(&node.symbol)
                .map_or_else(|| node.symbol.to_string(), |symbol| symbol.0.clone());
            let contact = match prefix.as_str() {
                "contact" => declaration,
                "source" => format!("{declaration}.source"),
                "first_contact" => format!("{declaration}.contacts[0]"),
                "second_contact" => format!("{declaration}.contacts[1]"),
                _ => format!("{declaration}.{prefix}"),
            };
            edits.push(AuthoredContactRangeEdit {
                node: node_id,
                contact,
                lower,
                upper,
            });
        }
    }
    edits
}

fn patched_definition_quantity(node: &IntentNode, patch: &IntentPatch, name: &str) -> Option<f64> {
    let mut value = node
        .fields
        .iter()
        .find_map(|(field, value)| (field.0.as_str() == name).then_some(value));
    for operation in patch.operations() {
        match operation {
            IntentPatchOperation::SetDefinitionField {
                node: target,
                field,
                value: replacement,
            } if *target == node.id && field.0.as_str() == name => value = Some(replacement),
            IntentPatchOperation::UnsetDefinitionField {
                node: target,
                field,
            } if *target == node.id && field.0.as_str() == name => value = None,
            _ => {}
        }
    }
    match value {
        Some(IntentLiteral::Quantity { value, .. }) if value.is_finite() => Some(*value),
        _ => None,
    }
}

fn actionable_contact_range_rejection(
    edits: &[AuthoredContactRangeEdit],
    error: &ProjectionalEditorError,
) -> Option<String> {
    let ProjectionalEditorError::Coordinator(
        geosolve_constraint_editor::ProjectionalCoordinatorError::Plan(
            IntentPlanError::EvaluationRejected { failure },
        ),
    ) = error
    else {
        return None;
    };
    let edit = edits
        .iter()
        .find(|edit| failure.failed_nodes.contains(&edit.node))?;
    let invariant = match failure.diagnostic.as_str() {
        "native-solver-rejected" => {
            "all hard constraints must admit a finite independently validated solution"
        }
        "native-validation-rejected" => {
            "the independently validated normalized hard residual must remain at most 1e-9"
        }
        _ => "the candidate must pass native materialization and independent validation",
    };
    Some(format!(
        "managed contact `{}` authored interval [{}, {}] was rejected: {invariant} (native diagnostic `{}`)",
        edit.contact, edit.lower, edit.upper, failure.diagnostic,
    ))
}

#[allow(
    clippy::too_many_lines,
    reason = "one transactional path keeps cold planning, warm authority, host replay, and final validation adjacent"
)]
fn materialize_code_project_incremental_with_overlay_and_work(
    previous: &MaterializedCodeProject,
    project: &CodeProject,
    reconciliation: &KeyedReconcileState,
    overlay: &CodeInteractionOverlay,
    accepted_continuation: Option<&SketchDocument>,
    work: &mut CodeWorkReceipt,
) -> Result<MaterializedCodeProject, CodeCompositionError> {
    validate_native_authority(&previous.editor)?;
    authenticate_expansion_envelope(&previous.editor, &previous.expansion)?;
    let accepted = previous
        .editor
        .coordinator()
        .accepted_materialization()
        .ok_or(CodeCompositionError::BaseNotAccepted)?;
    let document = accepted.session.design_document();
    let cold_intent =
        fresh_intent_session(previous.editor.coordinator().intent().identity().session)?;
    work.record_expansion_attempt();
    let expansion = expand_code_project_with_overlay_and_planner(
        project,
        reconciliation,
        overlay,
        cold_intent.identity(),
        &mut NativeCodeOperationPlanner {
            expected: cold_intent.identity(),
            document: document.id(),
            model_scale: document.model_scale(),
        },
    )?;
    if expansion.host_requests == previous.expansion.host_requests {
        return materialize_unchanged_host_project_incremental(
            previous,
            expansion,
            accepted_continuation,
        );
    }

    let desired = materialize_expanded_code_project_cold(
        expansion,
        cold_intent,
        document.id(),
        document.model_scale(),
    )?
    .into_public();
    let expansion = desired.expansion.clone();
    let replacement_slots = detached_reference_replacement_slots(&previous.expansion, &expansion);
    let previous_symbols = materialized_project_symbols(previous)?;
    let previous_drafts = complete_project_drafts(previous)?;
    let cold_drafts = complete_project_drafts(&desired)?;
    // Reject a real project-external dependent before constructing the
    // history-free warm branch oracle. The returned patch is deliberately
    // discarded; the final patch below uses warm-native Fillet ordering.
    incremental_project_patch(
        previous.editor.coordinator().intent(),
        &previous_symbols,
        &cold_drafts,
        &replacement_slots,
        OutsideDependentPolicy::Reject,
    )?;
    let warm_oracle = materialize_warm_host_oracle(
        previous,
        expansion.clone(),
        &previous_symbols,
        &replacement_slots,
    )?;
    let mut desired_drafts = complete_project_drafts(&warm_oracle)?;
    retain_unchanged_host_drafts(previous, &expansion, &previous_drafts, &mut desired_drafts)?;
    let (patch, retained_aliases) = incremental_project_patch(
        previous.editor.coordinator().intent(),
        &previous_symbols,
        &desired_drafts,
        &replacement_slots,
        OutsideDependentPolicy::Reject,
    )?;

    let mut editor = ProjectionalEditorSession::restore(
        previous.editor.coordinator().intent().clone(),
        document.id(),
        document.model_scale(),
    )?;
    let mut base_outcome = apply_incremental_project_patch(
        &mut editor,
        patch,
        &expansion,
        accepted_continuation,
        IncrementalPatchMode::Native,
    )?;
    if base_outcome.disposition != IntentPlanDisposition::Accepted {
        return Err(CodeCompositionError::BaseNotAccepted);
    }
    merge_retained_aliases(
        &mut base_outcome.aliases,
        editor.coordinator().intent(),
        &retained_aliases,
    )?;
    let desired_hosts = expand_host_members(&expansion.host_requests)?;
    let mut host_outputs = BTreeMap::<GeneratedMemberAddress, Vec<MaterializedFilletOutput>>::new();
    for member in desired_hosts.values() {
        let symbol = host_symbol(
            &member.key.address,
            member.request.identity,
            member.suffix.as_deref(),
        )?;
        let output = materialized_fillet_output(&editor, &symbol, &member.request)?;
        host_outputs
            .entry(member.key.address.clone())
            .or_default()
            .push(output);
    }
    for outputs in host_outputs.values_mut() {
        outputs.sort_by(|left, right| left.member_key.cmp(&right.member_key));
    }
    validate_native_authority(&editor)?;
    let editor = editor.into_delegated_accepted_authority()?;
    base_outcome.identity = editor.coordinator().intent().identity();

    Ok(MaterializedCodeProject {
        editor,
        expansion,
        base_outcome,
        host_outputs,
    })
}

/// Applies one ordinary incremental patch over a transaction-local fork when
/// code expansion leaves every native host request unchanged.
///
/// Existing host declarations and outputs are authenticated and retained;
/// their source dependencies participate in the ordinary native solve, but no
/// cold project reconstruction or Fillet re-authoring is needed. The result is
/// consumed into a history-neutral delegated authority because the adjacent
/// code session owns the sole user-visible Undo/Redo row.
fn materialize_unchanged_host_project_incremental(
    previous: &MaterializedCodeProject,
    expansion: ExpandedCodeProject,
    accepted_continuation: Option<&SketchDocument>,
) -> Result<MaterializedCodeProject, CodeCompositionError> {
    let replacement_slots = detached_reference_replacement_slots(&previous.expansion, &expansion);
    let previous_symbols = materialized_project_symbols(previous)?;
    let mut desired_drafts = create_drafts(&expansion)?
        .into_iter()
        .map(|(alias, draft)| (alias, draft.clone()))
        .collect::<Vec<_>>();
    desired_drafts.extend(materialized_host_drafts(previous)?);
    let mut editor = previous.editor.fork_accepted_authority()?;
    let (patch, retained_aliases) = incremental_project_patch(
        editor.coordinator().intent(),
        &previous_symbols,
        &desired_drafts,
        &replacement_slots,
        OutsideDependentPolicy::Reject,
    )?;

    let mut base_outcome = apply_incremental_project_patch(
        &mut editor,
        patch,
        &expansion,
        accepted_continuation,
        IncrementalPatchMode::Delegated,
    )?;
    if base_outcome.disposition != IntentPlanDisposition::Accepted {
        return Err(CodeCompositionError::BaseNotAccepted);
    }
    merge_retained_aliases(
        &mut base_outcome.aliases,
        editor.coordinator().intent(),
        &retained_aliases,
    )?;
    validate_native_authority(&editor)?;
    authenticate_expansion_envelope(&editor, &expansion)?;
    let authenticated_aliases = rehydrate_base_aliases(&editor, &expansion)?;
    let observed_host_outputs =
        rehydrate_host_outputs(&editor, &authenticated_aliases, &expansion.host_requests)?;
    if observed_host_outputs != previous.host_outputs {
        let member = expansion
            .host_requests
            .first()
            .map_or_else(|| "retained host outputs".into(), host_request_display_path);
        return Err(CodeCompositionError::RehydratedHostMismatch { member });
    }
    base_outcome.aliases = authenticated_aliases;
    let host_outputs = previous.host_outputs.clone();
    base_outcome.identity = editor.coordinator().intent().identity();
    validate_native_authority(&editor)?;

    Ok(MaterializedCodeProject {
        editor,
        expansion,
        base_outcome,
        host_outputs,
    })
}

fn host_request_display_path(request: &CodeHostRequest) -> String {
    match request {
        CodeHostRequest::FilletAtCorner(request) => request.output.display_path(),
        CodeHostRequest::RoundedRectangleProfile { output, .. } => output.display_path(),
    }
}

/// Keeps an existing native host declaration as the continuation seed when
/// its semantic request is unchanged. Source geometry edits already flow
/// through the host declaration's ordinary dependencies; replacing its
/// freshly transported definition in the same patch would needlessly
/// re-author the computed feature and churn its persistent native owner.
///
/// Changed radius, parent aliases, member identity or branch-bearing request
/// still uses the warm host oracle above. New and removed members likewise
/// retain the ordinary create/delete path.
fn retain_unchanged_host_drafts(
    previous: &MaterializedCodeProject,
    expansion: &ExpandedCodeProject,
    previous_drafts: &[(IntentKey, IntentNodeDraft)],
    desired_drafts: &mut [(IntentKey, IntentNodeDraft)],
) -> Result<(), CodeCompositionError> {
    let previous_hosts = expand_host_members(&previous.expansion.host_requests)?;
    let desired_hosts = expand_host_members(&expansion.host_requests)?;
    let previous_drafts = previous_drafts
        .iter()
        .map(|(alias, draft)| (alias, draft))
        .collect::<BTreeMap<_, _>>();

    for (key, desired_host) in desired_hosts {
        if previous_hosts.get(&key) != Some(&desired_host) {
            continue;
        }
        let symbol = host_symbol(
            &desired_host.key.address,
            desired_host.request.identity,
            desired_host.suffix.as_deref(),
        )?;
        let previous_draft = previous_drafts.get(&symbol).ok_or_else(|| {
            CodeCompositionError::HostOwnershipMismatch {
                member: desired_host.key.address.display_path(),
            }
        })?;
        let desired_draft = desired_drafts
            .iter_mut()
            .find_map(|(alias, draft)| (alias == &symbol).then_some(draft))
            .ok_or_else(|| CodeCompositionError::HostOwnershipMismatch {
                member: desired_host.key.address.display_path(),
            })?;
        desired_draft.clone_from(previous_draft);
    }
    Ok(())
}

#[allow(
    clippy::too_many_lines,
    reason = "one incremental transaction keeps replacement, dependent rebinding, and exact delete authentication auditable together"
)]
fn incremental_project_patch(
    intent: &IntentSession,
    previous_symbols: &BTreeSet<IntentKey>,
    desired_drafts: &[(IntentKey, IntentNodeDraft)],
    replacement_slots: &BTreeMap<IntentKey, BTreeSet<InputSlot>>,
    outside_policy: OutsideDependentPolicy,
) -> Result<(IntentPatch, BTreeMap<IntentKey, NodeId>), CodeCompositionError> {
    let desired_symbols = desired_drafts
        .iter()
        .map(|(_, draft)| draft.symbol.clone())
        .collect::<BTreeSet<_>>();
    let graph = intent.graph();
    let obsolete = previous_symbols
        .difference(&desired_symbols)
        .filter_map(|symbol| graph.node_by_symbol(symbol).map(|node| node.id))
        .collect::<BTreeSet<_>>();
    let mut retained = BTreeMap::<IntentKey, NodeId>::new();
    let mut replace = BTreeSet::new();
    let mut detachable_replacements = BTreeMap::<NodeId, IntentKey>::new();
    for (alias, draft) in desired_drafts {
        let Some(existing) = graph.node_by_symbol(&draft.symbol) else {
            continue;
        };
        if retained_schema_compatible(existing, draft)
            && retained_inputs_compatible(existing, draft, &retained, graph)
        {
            retained.insert(alias.clone(), existing.id);
        } else if replaceable_code_owned_schema(existing, draft, replacement_slots.get(alias)) {
            replace.insert(existing.id);
            if replacement_slots.contains_key(alias) {
                detachable_replacements.insert(existing.id, alias.clone());
            }
        } else {
            return Err(CodeCompositionError::IncrementalSchemaChange {
                symbol: draft.symbol.to_string(),
            });
        }
    }

    let mut operations = Vec::new();
    for (alias, draft) in desired_drafts {
        if let Some(node) = retained.get(alias).copied() {
            append_retained_edits(intent, node, draft, &retained, &mut operations)?;
        } else {
            let mut draft = draft.clone();
            rewrite_draft_inputs(graph, &retained, &mut draft)?;
            operations.push(IntentPatchOperation::CreateNode {
                alias: alias.clone(),
                draft: Box::new(draft),
                cell: None,
            });
        }
    }

    // A detached code endpoint replaces its Segment declaration because the
    // neutral intent vocabulary has no unbind-input operation. Preserve any
    // ordinary GUI declaration that consumes one of that Segment's outputs by
    // explicitly rebinding the same typed selector to the replacement alias
    // before the old declaration is deleted. This does not grant code
    // authority over the GUI declaration; it is the same stable dependency
    // continuation used for code-owned consumers.
    for node in graph
        .nodes()
        .values()
        .filter(|node| !previous_symbols.contains(&node.symbol) && !replace.contains(&node.id))
    {
        for (slot, source) in &node.inputs {
            let Some(alias) = detachable_replacements.get(&source.node) else {
                continue;
            };
            let source_node = graph.node(source.node).ok_or_else(|| {
                CodeCompositionError::IncrementalSchemaChange {
                    symbol: alias.to_string(),
                }
            })?;
            let source_port = source_node.ports.get(&source.port).ok_or_else(|| {
                CodeCompositionError::IncrementalSchemaChange {
                    symbol: alias.to_string(),
                }
            })?;
            if source_port.kind != source.kind {
                return Err(CodeCompositionError::IncrementalSchemaChange {
                    symbol: alias.to_string(),
                });
            }
            operations.push(IntentPatchOperation::RebindInput {
                node: node.id,
                slot: *slot,
                source: PatchPortRef::Alias {
                    node: alias.clone(),
                    selector: source_port.selector,
                },
            });
        }
    }

    let obsolete = obsolete.union(&replace).copied().collect::<BTreeSet<_>>();
    if let Some(root) = obsolete.first().copied() {
        let exact_nodes =
            dependent_closure_after_planned_rebinds(graph, &obsolete, &operations, &retained)?;
        if outside_policy == OutsideDependentPolicy::Reject
            && let Some(outside) = exact_nodes.iter().find(|node| {
                !obsolete.contains(node)
                    && graph
                        .node(**node)
                        .is_none_or(|node| !previous_symbols.contains(&node.symbol))
            })
        {
            let symbol = graph
                .node(*outside)
                .map_or_else(|| outside.to_string(), |node| node.symbol.to_string());
            return Err(CodeCompositionError::IncrementalOutsideDependent { symbol });
        }
        operations.push(IntentPatchOperation::DeleteNode {
            node: root,
            policy: DeletePolicy::CascadeRoots {
                exact_roots: obsolete.clone(),
                // Retained dependents are rebound above. The intent planner
                // applies those mutations before independently authenticating
                // this exact reconstructed post-rebind closure.
                exact_nodes,
            },
        });
    }
    Ok((
        IntentPatch::new(
            intent.identity(),
            IntentPatchPolicy::RequireAccepted,
            operations,
        ),
        retained,
    ))
}

/// Computes the exact dependent closure over the existing graph after the
/// input rebinds already planned in this unordered patch.
///
/// Intent applies retained mutations before authenticating deletion. Looking
/// only at the old graph therefore overstates the cascade, while subtracting
/// every descendant of a retained node can hide a second, unrebound path into
/// that same descendant. Rebuilding the existing-node dependency edges after
/// each planned rebind preserves both sides of the contract. Newly created
/// nodes are admitted only when their dependency chain is disjoint from the
/// roots being deleted; otherwise their not-yet-allocated IDs could not form
/// an exact caller-stamped witness and the edit fails closed.
fn dependent_closure_after_planned_rebinds(
    graph: &geosolve_sketch_intent::IntentGraph,
    roots: &BTreeSet<NodeId>,
    operations: &[IntentPatchOperation],
    retained: &BTreeMap<IntentKey, NodeId>,
) -> Result<BTreeSet<NodeId>, CodeCompositionError> {
    let mut inputs = graph
        .nodes()
        .iter()
        .map(|(node_id, node)| {
            (
                *node_id,
                node.inputs
                    .iter()
                    .map(|(slot, source)| (*slot, source.node))
                    .collect::<BTreeMap<_, _>>(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let auxiliary_dependencies = graph
        .nodes()
        .iter()
        .map(|(node_id, node)| {
            let input_nodes = node
                .inputs
                .values()
                .map(|source| source.node)
                .collect::<BTreeSet<_>>();
            (
                *node_id,
                node.dependencies()
                    .difference(&input_nodes)
                    .copied()
                    .collect::<BTreeSet<_>>(),
            )
        })
        .collect::<BTreeMap<_, _>>();

    for operation in operations {
        let IntentPatchOperation::RebindInput { node, slot, source } = operation else {
            continue;
        };
        let node_inputs =
            inputs
                .get_mut(node)
                .ok_or_else(|| CodeCompositionError::IncrementalSchemaChange {
                    symbol: node.to_string(),
                })?;
        let source = match source {
            PatchPortRef::Stable { port } => Some(port.node),
            PatchPortRef::Alias { node, .. } => retained.get(node).copied(),
        };
        if let Some(source) = source {
            node_inputs.insert(*slot, source);
        } else {
            // An alias absent from `retained` names a declaration created by
            // this patch, so it contributes no edge between existing nodes.
            node_inputs.remove(slot);
        }
    }

    let mut reverse = BTreeMap::<NodeId, BTreeSet<NodeId>>::new();
    for node_id in graph.nodes().keys() {
        for dependency in inputs[node_id]
            .values()
            .copied()
            .chain(auxiliary_dependencies[node_id].iter().copied())
        {
            reverse.entry(dependency).or_default().insert(*node_id);
        }
    }

    let mut closure = roots.clone();
    let mut pending = roots.clone();
    while let Some(node) = pending.pop_first() {
        if let Some(dependents) = reverse.get(&node) {
            for dependent in dependents {
                if closure.insert(*dependent) {
                    pending.insert(*dependent);
                }
            }
        }
    }

    ensure_created_nodes_avoid_deleted_closure(operations, retained, &closure)?;
    Ok(closure)
}

fn ensure_created_nodes_avoid_deleted_closure(
    operations: &[IntentPatchOperation],
    retained: &BTreeMap<IntentKey, NodeId>,
    closure: &BTreeSet<NodeId>,
) -> Result<(), CodeCompositionError> {
    let mut tainted = BTreeSet::<IntentKey>::new();
    loop {
        let previous_len = tainted.len();
        for operation in operations {
            let IntentPatchOperation::CreateNode { alias, draft, .. } = operation else {
                continue;
            };
            let depends_on_closure = draft.inputs.values().any(|source| match source {
                PatchPortRef::Stable { port } => closure.contains(&port.node),
                PatchPortRef::Alias { node, .. } => {
                    retained
                        .get(node)
                        .is_some_and(|node| closure.contains(node))
                        || tainted.contains(node)
                }
            });
            if depends_on_closure {
                tainted.insert(alias.clone());
            }
        }
        if tainted.len() == previous_len {
            break;
        }
    }
    if let Some(alias) = tainted.first() {
        return Err(CodeCompositionError::IncrementalSchemaChange {
            symbol: alias.to_string(),
        });
    }
    Ok(())
}

fn replaceable_code_owned_schema(
    node: &IntentNode,
    draft: &IntentNodeDraft,
    authorized_slots: Option<&BTreeSet<InputSlot>>,
) -> bool {
    match (&node.kind, &draft.kind) {
        (
            geosolve_sketch_intent::IntentNodeKind::Aggregate { aggregate: left },
            geosolve_sketch_intent::IntentNodeKind::Aggregate { aggregate: right },
        ) => left == right,
        (
            geosolve_sketch_intent::IntentNodeKind::Geometry { recipe: left },
            geosolve_sketch_intent::IntentNodeKind::Geometry { recipe: right },
        ) if left == right
            && matches!(
                left,
                geosolve_sketch_intent::GeometryRecipeKind::Segment
                    | geosolve_sketch_intent::GeometryRecipeKind::CenterRadiusCircle
            ) =>
        {
            // A managed/generated point consumer may transition between a
            // lexical input reference and a detached local seed. Intent
            // patches do not have an "unbind input" mutation, so replace this
            // one code-owned logical declaration atomically and rebind its
            // retained dependents before deleting the prior node. This is a
            // structural seed-authority transition, not a solver equation or
            // a general native-schema migration seam.
            let current = node.inputs.keys().copied().collect::<BTreeSet<_>>();
            let desired = draft.inputs.keys().copied().collect::<BTreeSet<_>>();
            let changed = current
                .symmetric_difference(&desired)
                .copied()
                .collect::<BTreeSet<_>>();
            !changed.is_empty()
                && authorized_slots.is_some_and(|slots| changed.is_subset(slots))
                && node.operation_outputs == draft.operation_outputs
                && node.child_order.len() == usize::from(draft.dynamic_children)
                && node.fields.keys().collect::<BTreeSet<_>>()
                    == draft.fields.keys().collect::<BTreeSet<_>>()
        }
        _ => false,
    }
}

fn detached_reference_replacement_slots(
    previous: &ExpandedCodeProject,
    desired: &ExpandedCodeProject,
) -> BTreeMap<IntentKey, BTreeSet<InputSlot>> {
    let mut authorized = BTreeMap::<IntentKey, BTreeSet<InputSlot>>::new();
    for point in &desired.writable_points {
        let Some(previous_point) = previous
            .writable_points
            .iter()
            .find(|candidate| candidate.handle == point.handle && candidate.edit == point.edit)
        else {
            continue;
        };
        // Overlay detachment retains the lexical Reference marker in both
        // expansions, while a managed source transaction replaces that
        // selected consumer value with a literal. In either case the prior
        // exact consumer must be reference-backed: this is authorization to
        // detach one input, never a general local-to-reference schema seam.
        if !previous_point.source.is_reference() {
            continue;
        }
        let IntentPortSelector::Node { role, index: 0 } = point.handle.selector else {
            continue;
        };
        let index = match role {
            IntentPortRole::Start | IntentPortRole::Center => 0,
            IntentPortRole::End => 1,
            _ => continue,
        };
        authorized
            .entry(point.handle.alias.clone())
            .or_default()
            .insert(InputSlot::new(InputRole::Point, index));
    }
    authorized
}

fn retained_inputs_compatible(
    node: &IntentNode,
    draft: &IntentNodeDraft,
    retained: &BTreeMap<IntentKey, NodeId>,
    graph: &geosolve_sketch_intent::IntentGraph,
) -> bool {
    draft.inputs.iter().all(|(slot, source)| {
        let desired = resolve_patch_ref(graph, retained, source).ok();
        match desired.as_ref() {
            Some(PatchPortRef::Stable { port }) => node
                .inputs
                .get(slot)
                .is_none_or(|existing| existing.kind == port.kind),
            Some(PatchPortRef::Alias { .. }) => true,
            None => false,
        }
    })
}

fn create_drafts(
    expansion: &ExpandedCodeProject,
) -> Result<Vec<(IntentKey, &IntentNodeDraft)>, CodeCompositionError> {
    expansion
        .patch
        .operations()
        .iter()
        .map(|operation| match operation {
            IntentPatchOperation::CreateNode { alias, draft, .. } => {
                Ok((alias.clone(), draft.as_ref()))
            }
            _ => Err(CodeCompositionError::IncrementalUnexpectedOperation),
        })
        .collect()
}

fn materialized_project_symbols(
    materialized: &MaterializedCodeProject,
) -> Result<BTreeSet<IntentKey>, CodeCompositionError> {
    let mut symbols = create_drafts(&materialized.expansion)?
        .into_iter()
        .map(|(_, draft)| draft.symbol.clone())
        .collect::<BTreeSet<_>>();
    for member in expand_host_members(&materialized.expansion.host_requests)?.values() {
        symbols.insert(host_symbol(
            &member.key.address,
            member.request.identity,
            member.suffix.as_deref(),
        )?);
    }
    Ok(symbols)
}

fn complete_project_drafts(
    materialized: &MaterializedCodeProject,
) -> Result<Vec<(IntentKey, IntentNodeDraft)>, CodeCompositionError> {
    let mut drafts = create_drafts(&materialized.expansion)?
        .into_iter()
        .map(|(alias, draft)| (alias, draft.clone()))
        .collect::<Vec<_>>();
    drafts.extend(materialized_host_drafts(materialized)?);
    Ok(drafts)
}

fn materialized_host_drafts(
    materialized: &MaterializedCodeProject,
) -> Result<Vec<(IntentKey, IntentNodeDraft)>, CodeCompositionError> {
    let intent = materialized.editor.coordinator().intent();
    let reverse_aliases = materialized
        .base_outcome
        .aliases
        .ports
        .iter()
        .flat_map(|(alias, ports)| {
            ports
                .iter()
                .map(move |(selector, port)| (*port, (alias.clone(), *selector)))
        })
        .collect::<BTreeMap<_, _>>();
    let mut drafts = Vec::new();
    for member in expand_host_members(&materialized.expansion.host_requests)?.values() {
        let symbol = host_symbol(
            &member.key.address,
            member.request.identity,
            member.suffix.as_deref(),
        )?;
        let node = intent.graph().node_by_symbol(&symbol).ok_or_else(|| {
            CodeCompositionError::HostOwnershipMismatch {
                member: member.key.address.display_path(),
            }
        })?;
        let draft = draft_from_materialized_node(intent, node, &reverse_aliases)?;
        drafts.push((symbol, draft));
    }
    Ok(drafts)
}

fn materialize_warm_host_oracle(
    previous: &MaterializedCodeProject,
    expansion: ExpandedCodeProject,
    previous_symbols: &BTreeSet<IntentKey>,
    replacement_slots: &BTreeMap<IntentKey, BTreeSet<InputSlot>>,
) -> Result<MaterializedCodeProject, CodeCompositionError> {
    let accepted = previous
        .editor
        .coordinator()
        .accepted_materialization()
        .ok_or(CodeCompositionError::BaseNotAccepted)?;
    let document = accepted.session.design_document();
    let mut editor = ProjectionalEditorSession::restore(
        previous.editor.coordinator().intent().clone(),
        document.id(),
        document.model_scale(),
    )?;
    let base_drafts = create_drafts(&expansion)?
        .into_iter()
        .map(|(alias, draft)| (alias, draft.clone()))
        .collect::<Vec<_>>();
    let (patch, retained_aliases) = incremental_project_patch(
        editor.coordinator().intent(),
        previous_symbols,
        &base_drafts,
        replacement_slots,
        OutsideDependentPolicy::CascadeInOracle,
    )?;
    let mut base_outcome = if patch.operations().is_empty() {
        ProjectionalPatchOutcome {
            identity: editor.coordinator().intent().identity(),
            disposition: IntentPlanDisposition::Accepted,
            aliases: IntentAliasMap::default(),
        }
    } else {
        editor
            .apply_patch(patch)
            .map_err(|error| CodeCompositionError::BaseEditor(error.to_string()))?
    };
    if base_outcome.disposition != IntentPlanDisposition::Accepted {
        return Err(CodeCompositionError::BaseNotAccepted);
    }
    merge_retained_aliases(
        &mut base_outcome.aliases,
        editor.coordinator().intent(),
        &retained_aliases,
    )?;
    let host_outputs =
        materialize_host_requests(&mut editor, &base_outcome.aliases, &expansion.host_requests)?;
    validate_native_authority(&editor)?;
    Ok(MaterializedCodeProject {
        editor,
        expansion,
        base_outcome,
        host_outputs,
    })
}

fn materialize_host_requests(
    editor: &mut ProjectionalEditorSession,
    aliases: &IntentAliasMap,
    requests: &[CodeHostRequest],
) -> Result<BTreeMap<GeneratedMemberAddress, Vec<MaterializedFilletOutput>>, CodeCompositionError> {
    let members = expand_host_members(requests)?;
    if members.is_empty() {
        return Ok(BTreeMap::new());
    }

    let candidates = prepare_host_fillet_candidates(editor, aliases, &members)?;
    let publication = publish_host_fillet_batch(editor, &candidates)?;
    suppress_host_fillet_batch(editor, &candidates, &publication.aliases)?;
    collect_materialized_host_outputs(editor, &candidates)
}

fn prepare_host_fillet_candidates(
    editor: &ProjectionalEditorSession,
    aliases: &IntentAliasMap,
    members: &BTreeMap<HostMemberKey, HostMemberRequest>,
) -> Result<Vec<PreparedHostFillet>, CodeCompositionError> {
    let mut candidates = Vec::with_capacity(members.len());
    for member in members.values() {
        let symbol = host_symbol(
            &member.key.address,
            member.request.identity,
            member.suffix.as_deref(),
        )?;
        let candidate = prepare_fillet_candidate(editor, aliases, &member.request)?;
        candidates.push(PreparedHostFillet {
            member: member.clone(),
            symbol,
            candidate,
        });
    }
    Ok(candidates)
}

fn publish_host_fillet_batch(
    editor: &mut ProjectionalEditorSession,
    candidates: &[PreparedHostFillet],
) -> Result<ProjectionalPatchOutcome, CodeCompositionError> {
    // Candidate construction remains the ordinary exact native Fillet owner,
    // but a non-interactive code project has no reason to materialize and then
    // discard one history-free preview per corner. Every candidate is stamped
    // against this same accepted base before one unordered atomic patch asks
    // the native solver to validate the complete host set once.
    let expected = editor.coordinator().intent().identity();
    let accepted = editor
        .coordinator()
        .accepted_materialization()
        .ok_or(CodeCompositionError::BaseNotAccepted)?;
    let accepted_state = accepted
        .session
        .accepted_state_for_current_input()
        .ok_or(CodeCompositionError::BaseNotAccepted)?;
    let accepted_input = accepted
        .session
        .accepted_prepared_input()
        .ok_or(CodeCompositionError::BaseNotAccepted)?;
    let mut operations = Vec::with_capacity(candidates.len());
    for (ordinal, prepared) in candidates.iter().enumerate() {
        let translated = projectional_fillet_patch(
            expected,
            editor.coordinator().intent(),
            &accepted.ownership,
            accepted_input,
            accepted_state.identity(),
            prepared.symbol.clone(),
            &prepared.candidate,
        )
        .map_err(|error| CodeCompositionError::HostEditor {
            member: prepared.member.key.address.display_path(),
            diagnostic: error.to_string(),
        })?;
        let [operation] = translated.patch.operations() else {
            return Err(CodeCompositionError::HostOwnershipMismatch {
                member: prepared.member.key.address.display_path(),
            });
        };
        let mut operation = operation.clone();
        let IntentPatchOperation::CreateNode { alias, .. } = &mut operation else {
            return Err(CodeCompositionError::HostOwnershipMismatch {
                member: prepared.member.key.address.display_path(),
            });
        };
        *alias = host_fillet_batch_alias(ordinal)?;
        operations.push(operation);
    }
    let outcome = editor
        .apply_patch(IntentPatch::new(
            expected,
            IntentPatchPolicy::RequireAccepted,
            operations,
        ))
        .map_err(|error| CodeCompositionError::HostEditor {
            member: "batched generated Fillets".into(),
            diagnostic: error.to_string(),
        })?;
    if outcome.disposition != IntentPlanDisposition::Accepted {
        return Err(CodeCompositionError::HostOwnershipMismatch {
            member: "batched generated Fillets".into(),
        });
    }
    Ok(outcome)
}

fn suppress_host_fillet_batch(
    editor: &mut ProjectionalEditorSession,
    candidates: &[PreparedHostFillet],
    publication_aliases: &IntentAliasMap,
) -> Result<(), CodeCompositionError> {
    let operations = candidates
        .iter()
        .enumerate()
        .filter(|(_, prepared)| prepared.member.request.suppressed)
        .map(|(ordinal, _)| {
            let alias = host_fillet_batch_alias(ordinal)?;
            publication_aliases
                .node(&alias)
                .map(|node| IntentPatchOperation::SetSuppressed {
                    node,
                    suppressed: true,
                })
                .ok_or_else(|| CodeCompositionError::HostOwnershipMismatch {
                    member: alias.to_string(),
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    if operations.is_empty() {
        return Ok(());
    }
    let suppression = editor
        .apply_patch(IntentPatch::new(
            editor.coordinator().intent().identity(),
            IntentPatchPolicy::RequireAccepted,
            operations,
        ))
        .map_err(|error| CodeCompositionError::HostEditor {
            member: "batched generated Fillet suppression".into(),
            diagnostic: error.to_string(),
        })?;
    if suppression.disposition != IntentPlanDisposition::Accepted {
        return Err(CodeCompositionError::HostOwnershipMismatch {
            member: "batched generated Fillet suppression".into(),
        });
    }
    Ok(())
}

fn collect_materialized_host_outputs(
    editor: &ProjectionalEditorSession,
    candidates: &[PreparedHostFillet],
) -> Result<BTreeMap<GeneratedMemberAddress, Vec<MaterializedFilletOutput>>, CodeCompositionError> {
    let mut host_outputs = BTreeMap::<GeneratedMemberAddress, Vec<MaterializedFilletOutput>>::new();
    for prepared in candidates {
        host_outputs
            .entry(prepared.member.key.address.clone())
            .or_default()
            .push(materialized_fillet_output(
                editor,
                &prepared.symbol,
                &prepared.member.request,
            )?);
    }
    for outputs in host_outputs.values_mut() {
        outputs.sort_by(|left, right| left.member_key.cmp(&right.member_key));
    }
    Ok(host_outputs)
}

fn host_fillet_batch_alias(ordinal: usize) -> Result<IntentKey, IntentKeyError> {
    IntentKey::new(format!("code-host-fillet-batch-{ordinal:04x}"))
}

fn draft_from_materialized_node(
    intent: &IntentSession,
    node: &IntentNode,
    reverse_aliases: &BTreeMap<
        IntentPortRef,
        (IntentKey, geosolve_sketch_intent::IntentPortSelector),
    >,
) -> Result<IntentNodeDraft, CodeCompositionError> {
    let name = intent
        .organization()
        .node_names()
        .get(&node.id)
        .cloned()
        .unwrap_or_else(|| node.symbol.clone());
    let initial_instance = node
        .ports
        .values()
        .filter_map(|port| {
            let values = port
                .writable
                .iter()
                .filter_map(|field| {
                    intent
                        .instance()
                        .values()
                        .get(&LeafRef {
                            node: node.id,
                            port: port.id,
                            field: *field,
                        })
                        .cloned()
                        .map(|value| (*field, value))
                })
                .collect::<BTreeMap<_, _>>();
            (!values.is_empty()).then_some((port.selector, values))
        })
        .collect();
    Ok(IntentNodeDraft {
        kind: node.kind.clone(),
        symbol: node.symbol.clone(),
        name,
        inputs: node
            .inputs
            .iter()
            .map(|(slot, port)| {
                let (alias, selector) = reverse_aliases.get(port).ok_or_else(|| {
                    CodeCompositionError::MissingAlias {
                        alias: format!("native port {port:?}"),
                    }
                })?;
                Ok((
                    *slot,
                    PatchPortRef::Alias {
                        node: alias.clone(),
                        selector: *selector,
                    },
                ))
            })
            .collect::<Result<_, CodeCompositionError>>()?,
        fields: node.fields.clone(),
        initial_instance,
        operation_outputs: node.operation_outputs.clone(),
        dynamic_children: u16::try_from(node.child_order.len()).map_err(|_| {
            CodeCompositionError::IncrementalSchemaChange {
                symbol: node.symbol.to_string(),
            }
        })?,
        suppressed: node.suppressed,
    })
}

fn retained_schema_compatible(node: &IntentNode, draft: &IntentNodeDraft) -> bool {
    node.kind == draft.kind
        && node.operation_outputs == draft.operation_outputs
        && node.child_order.len() == usize::from(draft.dynamic_children)
        && draft.schema_generated_port_count() == Some(node.ports.len())
        && node.inputs.keys().copied().collect::<BTreeSet<_>>()
            == draft.inputs.keys().copied().collect()
}

fn append_retained_edits(
    intent: &IntentSession,
    node_id: NodeId,
    draft: &IntentNodeDraft,
    retained: &BTreeMap<IntentKey, NodeId>,
    operations: &mut Vec<IntentPatchOperation>,
) -> Result<(), CodeCompositionError> {
    let graph = intent.graph();
    let node =
        graph
            .node(node_id)
            .ok_or_else(|| CodeCompositionError::IncrementalSchemaChange {
                symbol: draft.symbol.to_string(),
            })?;
    for field in node
        .fields
        .keys()
        .filter(|field| !draft.fields.contains_key(*field))
    {
        operations.push(IntentPatchOperation::UnsetDefinitionField {
            node: node_id,
            field: field.clone(),
        });
    }
    for (field, value) in &draft.fields {
        if node.fields.get(field) != Some(value) {
            operations.push(IntentPatchOperation::SetDefinitionField {
                node: node_id,
                field: field.clone(),
                value: value.clone(),
            });
        }
    }
    for (slot, source) in &draft.inputs {
        let source = resolve_patch_ref(graph, retained, source)?;
        if node.inputs.get(slot).copied() != stable_port(&source) {
            operations.push(IntentPatchOperation::RebindInput {
                node: node_id,
                slot: *slot,
                source,
            });
        }
    }
    for (selector, values) in &draft.initial_instance {
        let port = node.port_by_selector(*selector).ok_or_else(|| {
            CodeCompositionError::IncrementalSchemaChange {
                symbol: draft.symbol.to_string(),
            }
        })?;
        for (field, value) in values {
            let leaf = LeafRef {
                node: node_id,
                port: port.id,
                field: *field,
            };
            if intent.instance().values().get(&leaf) != Some(value) {
                operations.push(IntentPatchOperation::SetInstanceLeaf {
                    leaf,
                    value: value.clone(),
                });
            }
        }
    }
    if node.suppressed != draft.suppressed {
        operations.push(IntentPatchOperation::SetSuppressed {
            node: node_id,
            suppressed: draft.suppressed,
        });
    }
    if intent.organization().node_names().get(&node_id) != Some(&draft.name) {
        operations.push(IntentPatchOperation::RenameNode {
            node: node_id,
            name: draft.name.clone(),
        });
    }
    Ok(())
}

fn rewrite_draft_inputs(
    graph: &geosolve_sketch_intent::IntentGraph,
    retained: &BTreeMap<IntentKey, NodeId>,
    draft: &mut IntentNodeDraft,
) -> Result<(), CodeCompositionError> {
    for source in draft.inputs.values_mut() {
        *source = resolve_patch_ref(graph, retained, source)?;
    }
    Ok(())
}

fn resolve_patch_ref(
    graph: &geosolve_sketch_intent::IntentGraph,
    retained: &BTreeMap<IntentKey, NodeId>,
    source: &PatchPortRef,
) -> Result<PatchPortRef, CodeCompositionError> {
    let PatchPortRef::Alias { node, selector } = source else {
        return Ok(source.clone());
    };
    let Some(node_id) = retained.get(node).copied() else {
        return Ok(source.clone());
    };
    let declaration =
        graph
            .node(node_id)
            .ok_or_else(|| CodeCompositionError::IncrementalSchemaChange {
                symbol: node.to_string(),
            })?;
    let port = declaration.port_by_selector(*selector).ok_or_else(|| {
        CodeCompositionError::IncrementalSchemaChange {
            symbol: declaration.symbol.to_string(),
        }
    })?;
    Ok(PatchPortRef::Stable {
        port: port.as_ref(node_id),
    })
}

const fn stable_port(source: &PatchPortRef) -> Option<IntentPortRef> {
    match source {
        PatchPortRef::Stable { port } => Some(*port),
        PatchPortRef::Alias { .. } => None,
    }
}

fn merge_retained_aliases(
    aliases: &mut IntentAliasMap,
    intent: &IntentSession,
    retained: &BTreeMap<IntentKey, NodeId>,
) -> Result<(), CodeCompositionError> {
    for (alias, node_id) in retained {
        let node = intent.graph().node(*node_id).ok_or_else(|| {
            CodeCompositionError::IncrementalSchemaChange {
                symbol: alias.to_string(),
            }
        })?;
        aliases.nodes.insert(alias.clone(), *node_id);
        let ports = aliases.ports.entry(alias.clone()).or_default();
        for port in node.ports.values() {
            ports.insert(port.selector, port.as_ref(*node_id));
        }
    }
    Ok(())
}

fn expand_host_members(
    requests: &[CodeHostRequest],
) -> Result<BTreeMap<HostMemberKey, HostMemberRequest>, CodeCompositionError> {
    let mut result = BTreeMap::new();
    for request in requests {
        let members = match request {
            CodeHostRequest::FilletAtCorner(request) => vec![HostMemberRequest {
                key: HostMemberKey {
                    address: request.output.clone(),
                    member_key: request.member_key.clone(),
                },
                request: request.clone(),
                suffix: None,
            }],
            CodeHostRequest::RoundedRectangleProfile {
                output,
                identity,
                radius,
                corners,
                suppressed_children,
                ..
            } => corners
                .iter()
                .map(|(key, corner)| HostMemberRequest {
                    key: HostMemberKey {
                        address: output.clone(),
                        member_key: vec![key.clone()],
                    },
                    request: KeyedFilletHostRequest {
                        invocation: crate::SemanticSymbol(format!("{}.{}", output.invocation, key)),
                        member_key: vec![key.clone()],
                        output: output.clone(),
                        identity: *identity,
                        radius: radius.clone(),
                        corner: corner.clone(),
                        artifact_digest: String::new(),
                        suppressed: suppressed_children.contains(key),
                    },
                    suffix: Some(key.clone()),
                })
                .collect(),
        };
        for member in members {
            if result.insert(member.key.clone(), member.clone()).is_some() {
                return Err(CodeCompositionError::DuplicateHostOutput(
                    member.key.address.display_path(),
                ));
            }
        }
    }
    Ok(result)
}

fn validate_native_authority(
    editor: &ProjectionalEditorSession,
) -> Result<(), CodeCompositionError> {
    let intent = editor.coordinator().intent();
    let accepted = editor
        .coordinator()
        .accepted_materialization()
        .ok_or(CodeCompositionError::BaseNotAccepted)?;
    let semantic = intent.semantic_identity();
    if intent.accepted().is_none_or(|authority| {
        authority.target != semantic || authority.evidence != accepted.evidence
    }) || accepted.validation.semantic != semantic
        || accepted.ownership.semantic != semantic
        || accepted
            .session
            .accepted_state_for_current_input()
            .is_none()
        || !accepted.validation.hard_residuals_validated
        || !accepted.validation.all_active_features_current
        || accepted
            .validation
            .maximum_normalized_hard_residual
            .is_some_and(|value| !value.is_finite() || value > 1.0e-9)
    {
        return Err(CodeCompositionError::BaseNotAccepted);
    }
    Ok(())
}

fn prepare_fillet_candidate(
    editor: &ProjectionalEditorSession,
    aliases: &IntentAliasMap,
    request: &KeyedFilletHostRequest,
) -> Result<FeatureAuthoringCandidate, CodeCompositionError> {
    let (_, incoming, outgoing) = resolve_corner(editor, aliases, &request.corner)?;
    let label = request.output.display_path();
    let snapshot =
        editor
            .feature_authoring_snapshot()
            .map_err(|error| CodeCompositionError::HostEditor {
                member: label.clone(),
                diagnostic: error.to_string(),
            })?;
    let document = snapshot.sketch_document().clone();
    let mut state = FeatureAuthoringState::default();
    let _ = state.activate(&snapshot, &document, FeatureAuthoringTool::Fillet, &[]);
    let options = state.set_options(
        &snapshot,
        FeatureAuthoringOptions {
            fillet_radius: Some(request.radius.value),
            ..FeatureAuthoringOptions::default()
        },
    );
    if matches!(options, FeatureAuthoringOutcome::Warning(_)) {
        return Err(CodeCompositionError::HostPreviewIncomplete {
            member: label,
            outcome: format!("{options:?}"),
        });
    }
    let outcome = state.pick_items(
        &snapshot,
        &document,
        &[
            (SelectionItem::Curve(incoming), None),
            (SelectionItem::Curve(outgoing), None),
        ],
    );
    let candidate = match outcome {
        FeatureAuthoringOutcome::PreviewRequested { candidate, .. }
        | FeatureAuthoringOutcome::Apply(candidate) => candidate,
        other => {
            return Err(CodeCompositionError::HostPreviewIncomplete {
                member: label,
                outcome: format!("{other:?}"),
            });
        }
    };
    let persistent_corners = candidate.persistent_corners();
    let [corner] = persistent_corners.as_slice() else {
        return Err(CodeCompositionError::HostParentMismatch { member: label });
    };
    let actual = BTreeSet::from([corner.first.source.span, corner.second.source.span]);
    if actual != BTreeSet::from([incoming, outgoing]) {
        return Err(CodeCompositionError::HostParentMismatch { member: label });
    }
    Ok(candidate)
}

fn materialized_fillet_output(
    editor: &ProjectionalEditorSession,
    symbol: &IntentKey,
    request: &KeyedFilletHostRequest,
) -> Result<MaterializedFilletOutput, CodeCompositionError> {
    let label = request.output.display_path();
    // Native authoring owns its transaction-local alias; the deterministic
    // code host key is the durable developer symbol on the accepted node.
    let node = editor
        .coordinator()
        .intent()
        .graph()
        .node_by_symbol(symbol)
        .map(|node| node.id)
        .ok_or_else(|| CodeCompositionError::HostOwnershipMismatch {
            member: label.clone(),
        })?;
    let accepted = editor
        .coordinator()
        .accepted_materialization()
        .ok_or(CodeCompositionError::BaseNotAccepted)?;
    let ownership = accepted.ownership.node(node).ok_or_else(|| {
        CodeCompositionError::HostOwnershipMismatch {
            member: label.clone(),
        }
    })?;
    let mut feature = None;
    for binding in &ownership.owned {
        if let IntentNativeBinding::ComputedFeature(value) = binding {
            feature = Some(*value);
        }
    }
    let declaration = editor
        .coordinator()
        .intent()
        .graph()
        .node(node)
        .ok_or_else(|| CodeCompositionError::HostOwnershipMismatch {
            member: label.clone(),
        })?;
    let feature_corners = declaration
        .ports
        .values()
        .filter(|port| port.kind == IntentPortKind::FeatureCorner)
        .filter_map(|port| match accepted.ownership.port(port.as_ref(node)) {
            Some(IntentNativeBinding::ComputedFeatureCorner(value)) => Some(value),
            _ => None,
        })
        .collect::<Vec<_>>();
    let [corner] = feature_corners.as_slice() else {
        return Err(CodeCompositionError::HostOwnershipMismatch { member: label });
    };
    let Some(feature) = feature else {
        return Err(CodeCompositionError::HostOwnershipMismatch { member: label });
    };
    Ok(MaterializedFilletOutput {
        identity: request.identity,
        member_key: request.member_key.clone(),
        owner: ComputedCornerRef {
            feature,
            corner: *corner,
        },
    })
}

fn resolve_corner(
    editor: &ProjectionalEditorSession,
    aliases: &IntentAliasMap,
    corner: &ExpandedFeatureCorner,
) -> Result<
    (
        geosolve_sketch::DesignPointId,
        geosolve_sketch::CurveSpan,
        geosolve_sketch::CurveSpan,
    ),
    CodeCompositionError,
> {
    let accepted = editor
        .coordinator()
        .accepted_materialization()
        .ok_or(CodeCompositionError::BaseNotAccepted)?;
    let resolve = |port: &crate::ExpandedPort| {
        let reference = aliases.port(&port.alias, port.selector).ok_or_else(|| {
            CodeCompositionError::MissingAlias {
                alias: port.alias.to_string(),
            }
        })?;
        accepted.ownership.port(reference).ok_or_else(|| {
            CodeCompositionError::MissingNativeBinding {
                reference: format!("{}:{:?}", port.alias, port.selector),
            }
        })
    };
    let point = expect_point(resolve(&corner.point)?, &corner.point.alias.to_string())?;
    let incoming = expect_span(
        resolve(&corner.incoming)?,
        &corner.incoming.alias.to_string(),
    )?;
    let outgoing = expect_span(
        resolve(&corner.outgoing)?,
        &corner.outgoing.alias.to_string(),
    )?;
    Ok((point, incoming, outgoing))
}

fn expect_point(
    binding: IntentNativeBinding,
    reference: &str,
) -> Result<geosolve_sketch::DesignPointId, CodeCompositionError> {
    match binding {
        IntentNativeBinding::Point(value) => Ok(value),
        other => Err(CodeCompositionError::NativeKindMismatch {
            reference: reference.to_owned(),
            expected: "point",
            actual: native_kind(other),
        }),
    }
}

fn expect_span(
    binding: IntentNativeBinding,
    reference: &str,
) -> Result<geosolve_sketch::CurveSpan, CodeCompositionError> {
    match binding {
        IntentNativeBinding::CurveSpan(value) => Ok(value),
        other => Err(CodeCompositionError::NativeKindMismatch {
            reference: reference.to_owned(),
            expected: "curve_span",
            actual: native_kind(other),
        }),
    }
}

const fn native_kind(binding: IntentNativeBinding) -> &'static str {
    match binding {
        IntentNativeBinding::Point(_) => "point",
        IntentNativeBinding::Scalar(_) => "scalar",
        IntentNativeBinding::Curve(_) => "curve",
        IntentNativeBinding::CurveSpan(_) => "curve_span",
        IntentNativeBinding::Contact(_) => "contact",
        IntentNativeBinding::Constraint(_) => "constraint",
        IntentNativeBinding::Dimension(_) => "dimension",
        IntentNativeBinding::Source(_) => "source",
        IntentNativeBinding::Parameter(_) => "parameter",
        IntentNativeBinding::ExternalBinding(_) => "external_binding",
        IntentNativeBinding::ComputedFeature(_) => "computed_feature",
        IntentNativeBinding::ComputedFeatureCorner(_) => "computed_feature_corner",
        IntentNativeBinding::Logical(_) => "logical",
    }
}

fn host_symbol(
    address: &GeneratedMemberAddress,
    identity: GeneratedMemberIdentity,
    suffix: Option<&str>,
) -> Result<IntentKey, CodeCompositionError> {
    let bytes = serde_json::to_vec(&(address, identity, suffix))
        .map_err(|error| CodeCompositionError::Encoding(error.to_string()))?;
    Ok(IntentKey::new(format!(
        "code.host.{}",
        intent_content_digest(&bytes)
    ))?)
}

#[allow(
    dead_code,
    reason = "documents the exact cold-expansion identity boundary"
)]
const fn _intent_identity(session: &ProjectionalEditorSession) -> IntentSessionIdentity {
    session.coordinator().intent().identity()
}
