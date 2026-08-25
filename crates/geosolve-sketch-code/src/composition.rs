// SPDX-License-Identifier: GPL-3.0-or-later

//! Cold, transactional composition of structural code expansion with the
//! ordinary projectional editor and native computed-feature authoring paths.

use std::collections::{BTreeMap, BTreeSet};

use geosolve_constraint_editor::{
    ComputedCornerRef, ComputedFeatureDefinition, FeatureAuthoringCandidate,
    FeatureAuthoringOptions, FeatureAuthoringOutcome, FeatureAuthoringState, FeatureAuthoringTool,
    IntentNativeBinding, ProjectionalEditorError, ProjectionalEditorSession,
    ProjectionalPatchOutcome, SelectionItem,
};
use geosolve_sketch::DocumentId;
use geosolve_sketch_intent::{
    DeletePolicy, IntentAliasMap, IntentKey, IntentKeyError, IntentNode, IntentNodeDraft,
    IntentPatch, IntentPatchOperation, IntentPatchPolicy, IntentPlanDisposition, IntentPortKind,
    IntentPortRef, IntentSession, IntentSessionId, IntentSessionIdentity, LeafRef, NodeId,
    PatchPortRef, intent_content_digest,
};
use thiserror::Error;

use crate::{
    CodeExpansionError, CodeHostRequest, CodeProject, ExpandedCodeProject, ExpandedFeatureCorner,
    GeneratedMemberAddress, GeneratedMemberIdentity, KeyedFilletHostRequest, KeyedReconcileState,
    expand_code_project,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum OutsideDependentPolicy {
    Reject,
    CascadeInOracle,
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
    #[error("restored generated host member `{member}` does not match the expanded code payload")]
    RehydratedHostMismatch { member: String },
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
    let intent = IntentSession::with_id(intent_session).map_err(|error| {
        ProjectionalEditorError::Coordinator(
            geosolve_constraint_editor::ProjectionalCoordinatorError::Intent(error),
        )
    })?;
    let mut editor = ProjectionalEditorSession::restore(intent, document, model_scale)?;
    let expansion = expand_code_project(
        project,
        reconciliation,
        editor.coordinator().intent().identity(),
    )?;
    let base_outcome = editor
        .apply_patch(expansion.patch.clone())
        .map_err(|error| CodeCompositionError::BaseEditor(error.to_string()))?;
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

    Ok(MaterializedCodeProject {
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

fn authenticate_expansion_envelope(
    editor: &ProjectionalEditorSession,
    expansion: &ExpandedCodeProject,
) -> Result<(), CodeCompositionError> {
    let provenance_rows = expansion.generated_provenance.iter().collect::<Vec<_>>();
    let bytes = serde_json::to_vec(&(
        &expansion.patch,
        &expansion.semantic_outputs,
        &provenance_rows,
        &expansion.host_requests,
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
    if feature.suppressed
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
/// The complete desired project is cold-materialized first as independent
/// branch/data evidence. Its ordinary declarations are then reconciled into
/// one unordered native patch over a transaction-local restoration of the
/// prior accepted authority. Compatible nodes, ports, reservations and native
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
    let accepted = previous
        .editor
        .coordinator()
        .accepted_materialization()
        .ok_or(CodeCompositionError::BaseNotAccepted)?;
    let document = accepted.session.design_document();
    let desired = materialize_code_project_cold(
        project,
        reconciliation,
        previous.editor.coordinator().intent().identity().session,
        document.id(),
        document.model_scale(),
    )?;
    let expansion = desired.expansion.clone();
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
        OutsideDependentPolicy::Reject,
    )?;
    let warm_oracle = materialize_warm_host_oracle(previous, expansion.clone(), &previous_symbols)?;
    let mut desired_drafts = complete_project_drafts(&warm_oracle)?;
    retain_unchanged_host_drafts(previous, &expansion, &previous_drafts, &mut desired_drafts)?;
    let (patch, retained_aliases) = incremental_project_patch(
        previous.editor.coordinator().intent(),
        &previous_symbols,
        &desired_drafts,
        OutsideDependentPolicy::Reject,
    )?;

    let mut editor = ProjectionalEditorSession::restore(
        previous.editor.coordinator().intent().clone(),
        document.id(),
        document.model_scale(),
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

    Ok(MaterializedCodeProject {
        editor,
        expansion,
        base_outcome,
        host_outputs,
    })
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

fn incremental_project_patch(
    intent: &IntentSession,
    previous_symbols: &BTreeSet<IntentKey>,
    desired_drafts: &[(IntentKey, IntentNodeDraft)],
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
    for (alias, draft) in desired_drafts {
        let Some(existing) = graph.node_by_symbol(&draft.symbol) else {
            continue;
        };
        if retained_schema_compatible(existing, draft)
            && retained_inputs_compatible(existing, draft, &retained, graph)
        {
            retained.insert(alias.clone(), existing.id);
        } else if replaceable_logical_aggregate(existing, draft) {
            replace.insert(existing.id);
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

    let obsolete = obsolete.union(&replace).copied().collect::<BTreeSet<_>>();
    if let Some(root) = obsolete.first().copied() {
        let pre_rebind_closure = graph
            .dependent_closure(obsolete.iter().copied())
            .map_err(|error| CodeCompositionError::BaseEditor(error.to_string()))?;
        if outside_policy == OutsideDependentPolicy::Reject
            && let Some(outside) = pre_rebind_closure.iter().find(|node| {
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
        let retained_nodes = retained.values().copied().collect::<BTreeSet<_>>();
        let exact_nodes = pre_rebind_closure
            .difference(&retained_nodes)
            .copied()
            .collect::<BTreeSet<_>>();
        operations.push(IntentPatchOperation::DeleteNode {
            node: root,
            policy: DeletePolicy::CascadeRoots {
                exact_roots: obsolete.clone(),
                // Every code-owned survivor is either retained/rebound above
                // or recreated under a new symbol. The intent planner applies
                // those mutations before authenticating this post-rebind
                // closure, which must therefore be exactly the obsolete set.
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

fn replaceable_logical_aggregate(node: &IntentNode, draft: &IntentNodeDraft) -> bool {
    matches!(
        (&node.kind, &draft.kind),
        (
            geosolve_sketch_intent::IntentNodeKind::Aggregate { aggregate: left },
            geosolve_sketch_intent::IntentNodeKind::Aggregate { aggregate: right },
        ) if left == right
    )
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
    let mut drafts = create_drafts(&materialized.expansion)?
        .into_iter()
        .map(|(alias, draft)| (alias, draft.clone()))
        .collect::<Vec<_>>();
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
    let mut host_outputs = BTreeMap::new();
    for request in requests {
        match request {
            CodeHostRequest::FilletAtCorner(request) => {
                let output = materialize_fillet_request(editor, aliases, request, None)?;
                insert_host_output(&mut host_outputs, &request.output, vec![output])?;
            }
            CodeHostRequest::RoundedRectangleProfile {
                output,
                identity,
                radius,
                corners,
                ..
            } => {
                let mut outputs = Vec::with_capacity(corners.len());
                for (key, corner) in corners {
                    let request = KeyedFilletHostRequest {
                        invocation: crate::SemanticSymbol(format!("{}.{}", output.invocation, key)),
                        member_key: vec![key.clone()],
                        output: output.clone(),
                        identity: *identity,
                        radius: radius.clone(),
                        corner: corner.clone(),
                        artifact_digest: String::new(),
                    };
                    outputs.push(materialize_fillet_request(
                        editor,
                        aliases,
                        &request,
                        Some(key),
                    )?);
                }
                insert_host_output(&mut host_outputs, output, outputs)?;
            }
        }
    }
    Ok(host_outputs)
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
        && node.inputs.keys().copied().collect::<BTreeSet<_>>()
            == draft.inputs.keys().copied().collect()
        && node.fields.keys().cloned().collect::<BTreeSet<_>>()
            == draft.fields.keys().cloned().collect()
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
    Ok(())
}

fn materialize_fillet_request(
    editor: &mut ProjectionalEditorSession,
    aliases: &IntentAliasMap,
    request: &KeyedFilletHostRequest,
    suffix: Option<&str>,
) -> Result<MaterializedFilletOutput, CodeCompositionError> {
    let symbol = host_symbol(&request.output, request.identity, suffix)?;
    let label = request.output.display_path();
    let (mut state, _) = prepare_fillet_candidate(editor, aliases, request, symbol.clone())?;
    let outcome = editor
        .apply_computed_fillet_preview(&mut state, symbol.clone())
        .map_err(|error| CodeCompositionError::HostEditor {
            member: label.clone(),
            diagnostic: error.to_string(),
        })?;
    if outcome.disposition != IntentPlanDisposition::Accepted {
        return Err(CodeCompositionError::HostOwnershipMismatch { member: label });
    }
    materialized_fillet_output(editor, &symbol, request)
}

fn prepare_fillet_candidate(
    editor: &mut ProjectionalEditorSession,
    aliases: &IntentAliasMap,
    request: &KeyedFilletHostRequest,
    preview_symbol: IntentKey,
) -> Result<(FeatureAuthoringState, FeatureAuthoringCandidate), CodeCompositionError> {
    let (_, incoming, outgoing) = resolve_corner(editor, aliases, &request.corner)?;
    let label = request.output.display_path();
    let mut state = FeatureAuthoringState::default();
    let outcome = editor
        .activate_feature_authoring(
            &mut state,
            FeatureAuthoringTool::Fillet,
            FeatureAuthoringOptions {
                fillet_radius: Some(request.radius.value),
                ..FeatureAuthoringOptions::default()
            },
            &[
                (SelectionItem::Curve(incoming), None),
                (SelectionItem::Curve(outgoing), None),
            ],
            preview_symbol,
        )
        .map_err(|error| CodeCompositionError::HostEditor {
            member: label.clone(),
            diagnostic: error.to_string(),
        })?;
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
    Ok((state, candidate))
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

fn insert_host_output(
    outputs: &mut BTreeMap<GeneratedMemberAddress, Vec<MaterializedFilletOutput>>,
    address: &GeneratedMemberAddress,
    value: Vec<MaterializedFilletOutput>,
) -> Result<(), CodeCompositionError> {
    if outputs.insert(address.clone(), value).is_some() {
        return Err(CodeCompositionError::DuplicateHostOutput(
            address.display_path(),
        ));
    }
    Ok(())
}

#[allow(
    dead_code,
    reason = "documents the exact cold-expansion identity boundary"
)]
const fn _intent_identity(session: &ProjectionalEditorSession) -> IntentSessionIdentity {
    session.coordinator().intent().identity()
}
