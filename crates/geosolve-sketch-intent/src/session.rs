// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[cfg(test)]
thread_local! {
    static SESSION_IDENTITY_HASH_COUNT: std::cell::Cell<usize> = const {
        std::cell::Cell::new(0)
    };
}

use crate::graph::{
    IntentAllocatorHighWater, IntentGraph, IntentGraphError, IntentReservationLedger,
    allocate_draft, allocated_alias_ports, assign_identity_generations, finish_allocated_draft,
};
use crate::ids::{
    CellId, ContentDigest, IntentKey, IntentSessionId, NodeId, PlanToken, ReservationId, Revision,
    digest_bytes, legacy_digest_bytes,
};
use crate::model::{
    IntentExternalInputs, IntentExternalInputsIdentity, IntentIdentityFlow, IntentInstanceState,
    IntentOrganization, IntentReservationLedgerIdentity, IntentSemanticIdentity,
    MaterializationEvidence, OrganizationCell, PatchPortRef, literal_matches_leaf, next_revision,
};
use crate::patch::{
    CellTarget, DeletePolicy, IntentAliasMap, IntentPatch, IntentPatchOperation,
    IntentPatchOperationKind, IntentPatchPolicy, IntentSemanticDiff,
};

/// Strict canonical intent-session wire version.
pub const INTENT_SESSION_VERSION: u32 = 2;
const LEGACY_INTENT_SESSION_VERSION: u32 = 1;
/// User-visible Undo/Redo bound.
pub const MAX_INTENT_HISTORY_ENTRIES: usize = 1_024;
/// Maximum strict canonical session JSON, aligned with the public workspace
/// and RPC response envelope.
pub const MAX_INTENT_SESSION_JSON_BYTES: usize = 64 * 1024 * 1024;
/// Maximum operations in one atomic unordered patch.
pub const MAX_INTENT_PATCH_OPERATIONS: usize = 16_384;

/// Exact current compare-and-swap identity plus independently revisioned
/// component identities.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IntentSessionIdentity {
    pub session: IntentSessionId,
    pub revision: Revision,
    pub digest: ContentDigest,
    pub graph: crate::IntentGraphIdentity,
    pub instance: crate::IntentInstanceIdentity,
    pub reservations: IntentReservationLedgerIdentity,
    pub organization: crate::IntentOrganizationIdentity,
    pub external_inputs: IntentExternalInputsIdentity,
}

/// Accepted, retained-failed, or presentation-only attempt disposition.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum IntentAttemptDisposition {
    Accepted,
    RetainedFailed,
    OrganizationOnly,
}

/// Typed materialization failure. Cancellation, exhaustion, and stale work are
/// never eligible for retained failed intent.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum IntentEvaluationFailureKind {
    InvalidGeometry,
    SolverRejected,
    MaterializationRejected,
    Cancelled,
    Exhausted,
    Stale,
}

impl IntentEvaluationFailureKind {
    const fn nonpublishing(self) -> bool {
        matches!(self, Self::Cancelled | Self::Exhausted | Self::Stale)
    }
}

/// Structured host materializer failure for a structurally valid candidate.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IntentEvaluationFailure {
    pub kind: IntentEvaluationFailureKind,
    pub failed_nodes: BTreeSet<NodeId>,
    pub diagnostic: IntentKey,
}

/// Evaluation result returned by the existing native materialization/solver
/// stack. No callback is persisted or executed during solving.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "outcome", rename_all = "snake_case", deny_unknown_fields)]
pub enum IntentEvaluation {
    Accepted { evidence: MaterializationEvidence },
    Failed { failure: IntentEvaluationFailure },
}

/// Latest current-intent evaluation evidence.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IntentLatestAttempt {
    pub target: IntentSemanticIdentity,
    pub disposition: IntentAttemptDisposition,
    pub materialization_digest: Option<ContentDigest>,
    pub failed_nodes: BTreeSet<NodeId>,
    pub diagnostic: Option<IntentKey>,
}

/// Last host-accepted scene authority. Its exact host inputs and authenticated
/// artifacts remain available even while newer failed intent is retained.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IntentAcceptedAuthority {
    pub target: IntentSemanticIdentity,
    pub graph: IntentGraph,
    pub instance: IntentInstanceState,
    pub reservations: IntentReservationLedger,
    pub external_inputs: IntentExternalInputs,
    pub evidence: MaterializationEvidence,
}

/// Immutable structurally validated candidate given to the host materializer.
#[derive(Clone, Debug)]
pub struct IntentCandidate {
    graph: IntentGraph,
    instance: IntentInstanceState,
    reservations: IntentReservationLedger,
    organization: IntentOrganization,
    external_inputs: IntentExternalInputs,
    semantic_identity: IntentSemanticIdentity,
    diff: IntentSemanticDiff,
}

impl IntentCandidate {
    #[must_use]
    pub const fn graph(&self) -> &IntentGraph {
        &self.graph
    }

    #[must_use]
    pub const fn instance(&self) -> &IntentInstanceState {
        &self.instance
    }

    #[must_use]
    pub const fn reservations(&self) -> &IntentReservationLedger {
        &self.reservations
    }

    #[must_use]
    pub const fn organization(&self) -> &IntentOrganization {
        &self.organization
    }

    #[must_use]
    pub const fn external_inputs(&self) -> &IntentExternalInputs {
        &self.external_inputs
    }

    #[must_use]
    pub const fn semantic_identity(&self) -> IntentSemanticIdentity {
        self.semantic_identity
    }

    #[must_use]
    pub const fn diff(&self) -> &IntentSemanticDiff {
        &self.diff
    }
}

/// Commit disposition authenticated by one plan token.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum IntentPlanDisposition {
    Accepted,
    RetainedFailed,
    OrganizationOnly,
}

/// Deterministic, clock-free descriptor of one committed user transaction.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IntentTransactionDescriptor {
    pub target_revision: Revision,
    pub disposition: IntentPlanDisposition,
    pub operation_kinds: Vec<IntentPatchOperationKind>,
    pub affected_nodes: BTreeSet<NodeId>,
    pub diff: IntentSemanticDiff,
}

impl IntentTransactionDescriptor {
    fn new(
        target_revision: Revision,
        disposition: IntentPlanDisposition,
        mut operation_kinds: Vec<IntentPatchOperationKind>,
        diff: IntentSemanticDiff,
    ) -> Self {
        operation_kinds.sort_unstable();
        operation_kinds.dedup();
        let affected_nodes = diff.affected_nodes();
        Self {
            target_revision,
            disposition,
            operation_kinds,
            affected_nodes,
            diff,
        }
    }

    fn validate(&self) -> Result<(), IntentSessionError> {
        if self.operation_kinds.is_empty()
            || self.operation_kinds.len() > MAX_INTENT_PATCH_OPERATIONS
            || !self
                .operation_kinds
                .windows(2)
                .all(|pair| pair[0] < pair[1])
            || self.affected_nodes != self.diff.affected_nodes()
            || (self.disposition == IntentPlanDisposition::OrganizationOnly
                && self.diff.requires_materialization())
            || (self.disposition != IntentPlanDisposition::OrganizationOnly
                && !self.diff.requires_materialization())
        {
            return Err(IntentSessionError::InvalidHistoryDescriptor);
        }
        Ok(())
    }
}

/// Read-only deterministic projection of applied and next-redoable
/// transactions. `redoable` is ordered in the order Redo will apply it.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IntentHistoryProjection {
    pub applied: Vec<IntentTransactionDescriptor>,
    pub redoable: Vec<IntentTransactionDescriptor>,
}

/// Opaque staged transaction returned by [`IntentSession::plan_patch`].
#[derive(Clone, Debug)]
pub struct IntentPatchPlan {
    base: IntentSessionIdentity,
    target: IntentSessionIdentity,
    token: PlanToken,
    disposition: IntentPlanDisposition,
    aliases: IntentAliasMap,
    diff: IntentSemanticDiff,
    descriptor: IntentTransactionDescriptor,
    staged: SessionCheckpoint,
    reservations: IntentReservationLedger,
    undo: Vec<HistoryEntry>,
    redo: Vec<HistoryEntry>,
    allocator: IntentAllocatorHighWater,
}

impl IntentPatchPlan {
    #[must_use]
    pub const fn base(&self) -> IntentSessionIdentity {
        self.base
    }

    #[must_use]
    pub const fn target(&self) -> IntentSessionIdentity {
        self.target
    }

    #[must_use]
    pub const fn token(&self) -> PlanToken {
        self.token
    }

    #[must_use]
    pub const fn disposition(&self) -> IntentPlanDisposition {
        self.disposition
    }

    #[must_use]
    pub const fn aliases(&self) -> &IntentAliasMap {
        &self.aliases
    }

    #[must_use]
    pub const fn diff(&self) -> &IntentSemanticDiff {
        &self.diff
    }

    #[must_use]
    pub const fn descriptor(&self) -> &IntentTransactionDescriptor {
        &self.descriptor
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct SessionCheckpoint {
    graph: IntentGraph,
    instance: IntentInstanceState,
    reservation_identity: IntentReservationLedgerIdentity,
    /// Exact never-reused reservation prefix retained when this checkpoint
    /// was captured. Experimental wire-v1 omitted it and is migrated through
    /// its independently authenticated historical ledger.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    reservation_high_water: Option<ReservationId>,
    organization: IntentOrganization,
    external_inputs: IntentExternalInputs,
    latest_attempt: Option<IntentLatestAttempt>,
    accepted: Option<IntentAcceptedAuthority>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct HistoryEntry {
    checkpoint: SessionCheckpoint,
    descriptor: IntentTransactionDescriptor,
    #[serde(
        default = "ContentDigest::zero",
        skip_serializing_if = "content_digest_is_zero"
    )]
    before: ContentDigest,
    #[serde(
        default = "ContentDigest::zero",
        skip_serializing_if = "content_digest_is_zero"
    )]
    after: ContentDigest,
}

fn content_digest_is_zero(value: &ContentDigest) -> bool {
    *value == ContentDigest::zero()
}

/// Identifies the user-restorable checkpoint content while deliberately
/// excluding monotonic/recomputable authority which changes during Undo/Redo.
///
/// Graph, instance and organization component revisions are advanced when a
/// historical body is restored. The reservation ledger likewise retains
/// later tombstones, and accepted materialization artifacts can be refreshed
/// without creating a user transaction. Stable definitions, values,
/// organization, exact host inputs, attempt disposition and accepted logical
/// content are the causal history state and remain invariant across those
/// maintenance operations.
fn history_checkpoint_digest(checkpoint: &SessionCheckpoint) -> ContentDigest {
    let attempt = checkpoint.latest_attempt.as_ref().map(|value| {
        (
            value.disposition,
            &value.failed_nodes,
            value.diagnostic.as_ref(),
        )
    });
    let accepted = checkpoint.accepted.as_ref().map(|value| {
        (
            &value.graph.nodes,
            &value.instance.values,
            &value.external_inputs,
        )
    });
    digest_bytes(
        &serde_json::to_vec(&(
            &checkpoint.graph.nodes,
            &checkpoint.instance.values,
            checkpoint.organization.default_cell,
            &checkpoint.organization.cell_order,
            &checkpoint.organization.cells,
            &checkpoint.organization.node_names,
            &checkpoint.external_inputs,
            attempt,
            accepted,
        ))
        .expect("history checkpoint identity is infallibly serializable"),
    )
}

fn bind_history_edges(
    current: &SessionCheckpoint,
    undo: &mut [HistoryEntry],
    redo: &mut [HistoryEntry],
) {
    let current = history_checkpoint_digest(current);
    let undo_states = undo
        .iter()
        .map(|entry| history_checkpoint_digest(&entry.checkpoint))
        .collect::<Vec<_>>();
    let redo_states = redo
        .iter()
        .map(|entry| history_checkpoint_digest(&entry.checkpoint))
        .collect::<Vec<_>>();

    for (index, entry) in undo.iter_mut().enumerate() {
        entry.before = undo_states[index];
        entry.after = undo_states.get(index + 1).copied().unwrap_or(current);
    }
    for (index, entry) in redo.iter_mut().enumerate() {
        entry.before = redo_states.get(index + 1).copied().unwrap_or(current);
        entry.after = redo_states[index];
    }
}

fn validate_history_edges(
    current: &SessionCheckpoint,
    undo: &[HistoryEntry],
    redo: &[HistoryEntry],
) -> Result<(), IntentSessionError> {
    let current = history_checkpoint_digest(current);
    let undo_states = undo
        .iter()
        .map(|entry| history_checkpoint_digest(&entry.checkpoint))
        .collect::<Vec<_>>();
    let redo_states = redo
        .iter()
        .map(|entry| history_checkpoint_digest(&entry.checkpoint))
        .collect::<Vec<_>>();

    for (index, entry) in undo.iter().enumerate() {
        let after = undo_states.get(index + 1).copied().unwrap_or(current);
        if entry.before != undo_states[index] || entry.after != after {
            return Err(IntentSessionError::InvalidHistoryTransition);
        }
    }
    for (index, entry) in redo.iter().enumerate() {
        let before = redo_states.get(index + 1).copied().unwrap_or(current);
        if entry.before != before || entry.after != redo_states[index] {
            return Err(IntentSessionError::InvalidHistoryTransition);
        }
    }
    Ok(())
}

fn validate_history_descriptors(
    current: &SessionCheckpoint,
    undo: &[HistoryEntry],
    redo: &[HistoryEntry],
) -> Result<(), IntentSessionError> {
    for (index, entry) in undo.iter().enumerate() {
        let after = undo.get(index + 1).map_or(current, |next| &next.checkpoint);
        validate_history_descriptor_transition(&entry.checkpoint, after, &entry.descriptor)?;
    }
    for (index, entry) in redo.iter().enumerate() {
        let before = redo.get(index + 1).map_or(current, |next| &next.checkpoint);
        validate_history_descriptor_transition(before, &entry.checkpoint, &entry.descriptor)?;
    }
    Ok(())
}

fn validate_history_descriptor_transition(
    before: &SessionCheckpoint,
    after: &SessionCheckpoint,
    descriptor: &IntentTransactionDescriptor,
) -> Result<(), IntentSessionError> {
    let before_nodes = before.graph.nodes();
    let after_nodes = after.graph.nodes();
    let created_nodes = after_nodes
        .keys()
        .filter(|node| !before_nodes.contains_key(node))
        .copied()
        .collect::<BTreeSet<_>>();
    let deleted_nodes = before_nodes
        .keys()
        .filter(|node| !after_nodes.contains_key(node))
        .copied()
        .collect::<BTreeSet<_>>();
    let mut definition_nodes = created_nodes
        .iter()
        .chain(&deleted_nodes)
        .copied()
        .collect::<BTreeSet<_>>();
    definition_nodes.extend(before_nodes.iter().filter_map(|(node, before_node)| {
        after_nodes
            .get(node)
            .filter(|after_node| !history_node_definition_eq(before_node, after_node))
            .map(|_| *node)
    }));

    let instance_nodes = before
        .instance
        .values()
        .keys()
        .chain(after.instance.values().keys())
        .filter(|leaf| !deleted_nodes.contains(&leaf.node))
        .filter(|leaf| before.instance.values().get(leaf) != after.instance.values().get(leaf))
        .map(|leaf| leaf.node)
        .collect::<BTreeSet<_>>();
    let organization_changed = before.organization.default_cell != after.organization.default_cell
        || before.organization.cell_order != after.organization.cell_order
        || before.organization.cells != after.organization.cells
        || before.organization.node_names != after.organization.node_names;
    let external_inputs_changed = before.external_inputs != after.external_inputs;
    let disposition_matches = match descriptor.disposition {
        IntentPlanDisposition::Accepted => after
            .latest_attempt
            .as_ref()
            .is_some_and(|attempt| attempt.disposition == IntentAttemptDisposition::Accepted),
        IntentPlanDisposition::RetainedFailed => after
            .latest_attempt
            .as_ref()
            .is_some_and(|attempt| attempt.disposition == IntentAttemptDisposition::RetainedFailed),
        IntentPlanDisposition::OrganizationOnly => organization_changed,
    };
    let authority_transition_matches = match descriptor.disposition {
        IntentPlanDisposition::Accepted => true,
        IntentPlanDisposition::RetainedFailed => before.accepted == after.accepted,
        IntentPlanDisposition::OrganizationOnly => {
            history_attempt_content_eq(
                before.latest_attempt.as_ref(),
                after.latest_attempt.as_ref(),
            ) && history_accepted_content_eq(before.accepted.as_ref(), after.accepted.as_ref())
        }
    };
    let graph_changed = !definition_nodes.is_empty();
    let operation_kinds_match =
        history_operation_kinds_match(before, after, descriptor, &created_nodes, &deleted_nodes);
    let organization_nodes_match = history_organization_nodes_match(
        before,
        after,
        &created_nodes,
        &deleted_nodes,
        &descriptor.diff.organization_nodes,
    );

    if !authority_transition_matches {
        return Err(IntentSessionError::InvalidHistoryTransition);
    }
    if descriptor.diff.created_nodes != created_nodes
        || descriptor.diff.deleted_nodes != deleted_nodes
        || descriptor.diff.definition_nodes != definition_nodes
        || descriptor.diff.instance_nodes != instance_nodes
        || descriptor.diff.graph_changed != graph_changed
        || descriptor.diff.instance_changed
            != (!instance_nodes.is_empty() || !deleted_nodes.is_empty())
        || descriptor.diff.organization_changed != organization_changed
        || descriptor.diff.external_inputs_changed != external_inputs_changed
        || !organization_nodes_match
        || !disposition_matches
        || !operation_kinds_match
    {
        return Err(IntentSessionError::InvalidHistoryDescriptor);
    }
    Ok(())
}

fn history_attempt_content_eq(
    left: Option<&IntentLatestAttempt>,
    right: Option<&IntentLatestAttempt>,
) -> bool {
    left.map(|attempt| {
        (
            attempt.disposition,
            &attempt.failed_nodes,
            attempt.diagnostic.as_ref(),
        )
    }) == right.map(|attempt| {
        (
            attempt.disposition,
            &attempt.failed_nodes,
            attempt.diagnostic.as_ref(),
        )
    })
}

fn history_accepted_content_eq(
    left: Option<&IntentAcceptedAuthority>,
    right: Option<&IntentAcceptedAuthority>,
) -> bool {
    match (left, right) {
        (None, None) => true,
        (Some(left), Some(right)) => {
            left.graph.nodes == right.graph.nodes
                && left.instance.values == right.instance.values
                && left.external_inputs == right.external_inputs
        }
        _ => false,
    }
}

fn history_operation_kinds_match(
    before: &SessionCheckpoint,
    after: &SessionCheckpoint,
    descriptor: &IntentTransactionDescriptor,
    created_nodes: &BTreeSet<NodeId>,
    deleted_nodes: &BTreeSet<NodeId>,
) -> bool {
    history_operation_kinds(before, after, created_nodes, deleted_nodes)
        .is_some_and(|expected| descriptor.operation_kinds == expected)
}

/// Reconstructs the unique effective patch categories from two retained
/// checkpoint bodies. History descriptors are audit projections, not replay
/// logs: no-op requests and the number of same-category edits carry no durable
/// meaning, while every observable state transition must be reachable through
/// the closed patch vocabulary.
#[allow(
    clippy::too_many_lines,
    reason = "one closed transition audit reconstructs every patch category before accepting history"
)]
fn history_operation_kinds(
    before: &SessionCheckpoint,
    after: &SessionCheckpoint,
    created_nodes: &BTreeSet<NodeId>,
    deleted_nodes: &BTreeSet<NodeId>,
) -> Option<Vec<IntentPatchOperationKind>> {
    let mut expected = BTreeSet::new();
    if !created_nodes.is_empty() {
        expected.insert(IntentPatchOperationKind::CreateNode);
    }
    if !deleted_nodes.is_empty() {
        expected.insert(IntentPatchOperationKind::DeleteNode);
    }

    let mut reachable_graph = before.graph.clone();
    reachable_graph
        .nodes
        .retain(|node, _| !deleted_nodes.contains(node));
    for node in created_nodes {
        reachable_graph
            .nodes
            .insert(*node, after.graph.nodes().get(node)?.clone());
    }
    for (node_id, after_node) in after
        .graph
        .nodes()
        .iter()
        .filter(|(node, _)| !created_nodes.contains(node) && !deleted_nodes.contains(node))
    {
        let current = reachable_graph.node(*node_id)?;
        if current.kind != after_node.kind
            || current.bootstrap_origin != after_node.bootstrap_origin
        {
            reachable_graph.eject_bootstrap_point(*node_id).ok()?;
            expected.insert(IntentPatchOperationKind::EjectBootstrapPoint);
        }
        if reachable_graph.node(*node_id)?.suppressed != after_node.suppressed {
            reachable_graph
                .set_suppressed(*node_id, after_node.suppressed)
                .ok()?;
            expected.insert(IntentPatchOperationKind::SetSuppressed);
        }
        let current_fields = reachable_graph.node(*node_id)?.fields.clone();
        if current_fields != after_node.fields {
            for field in current_fields
                .keys()
                .filter(|field| !after_node.fields.contains_key(*field))
            {
                reachable_graph.unset_field(*node_id, field.clone()).ok()?;
                expected.insert(IntentPatchOperationKind::UnsetDefinitionField);
            }
            for (field, value) in &after_node.fields {
                if current_fields.get(field) != Some(value) {
                    reachable_graph
                        .set_field(*node_id, field.clone(), value.clone())
                        .ok()?;
                    expected.insert(IntentPatchOperationKind::SetDefinitionField);
                }
            }
        }
        let current_inputs = reachable_graph.node(*node_id)?.inputs.clone();
        if current_inputs != after_node.inputs {
            if current_inputs.keys().ne(after_node.inputs.keys()) {
                return None;
            }
            for (slot, source) in &after_node.inputs {
                if current_inputs.get(slot) != Some(source) {
                    reachable_graph
                        .rebind_input(*node_id, *slot, *source)
                        .ok()?;
                }
            }
            expected.insert(IntentPatchOperationKind::RebindInput);
        }
    }
    assign_identity_generations(&mut reachable_graph).ok()?;
    if reachable_graph.nodes().len() != after.graph.nodes().len()
        || reachable_graph.nodes().iter().any(|(node, expected)| {
            after
                .graph
                .nodes()
                .get(node)
                .is_none_or(|actual| !history_node_definition_eq(expected, actual))
        })
    {
        return None;
    }

    let retained_instance_changed = before
        .instance
        .values()
        .keys()
        .chain(after.instance.values().keys())
        .filter(|leaf| !created_nodes.contains(&leaf.node) && !deleted_nodes.contains(&leaf.node))
        .any(|leaf| before.instance.values().get(leaf) != after.instance.values().get(leaf));
    if before.instance.values().iter().any(|(leaf, _)| {
        !created_nodes.contains(&leaf.node)
            && !deleted_nodes.contains(&leaf.node)
            && !after.instance.values().contains_key(leaf)
    }) {
        return None;
    }
    if retained_instance_changed {
        expected.insert(IntentPatchOperationKind::SetInstanceLeaf);
    }

    if before.organization.default_cell != after.organization.default_cell {
        return None;
    }
    let before_cells = before
        .organization
        .cells
        .keys()
        .copied()
        .collect::<BTreeSet<_>>();
    let after_cells = after
        .organization
        .cells
        .keys()
        .copied()
        .collect::<BTreeSet<_>>();
    let created_cells = after_cells
        .difference(&before_cells)
        .copied()
        .collect::<BTreeSet<_>>();
    let deleted_cells = before_cells
        .difference(&after_cells)
        .copied()
        .collect::<BTreeSet<_>>();
    if !created_cells.is_empty() {
        expected.insert(IntentPatchOperationKind::CreateCell);
    }
    if !deleted_cells.is_empty() {
        expected.insert(IntentPatchOperationKind::DeleteCell);
    }
    for cell in before_cells.intersection(&after_cells) {
        if before.organization.cells[cell].name != after.organization.cells[cell].name {
            return None;
        }
    }

    if before.organization.node_names.iter().any(|(node, name)| {
        !deleted_nodes.contains(node)
            && after
                .organization
                .node_names
                .get(node)
                .is_some_and(|after| after != name)
    }) {
        expected.insert(IntentPatchOperationKind::RenameNode);
    }
    let common_cell_order_before = before
        .organization
        .cell_order
        .iter()
        .filter(|cell| after_cells.contains(cell))
        .copied()
        .collect::<Vec<_>>();
    let common_cell_order_after = after
        .organization
        .cell_order
        .iter()
        .filter(|cell| before_cells.contains(cell))
        .copied()
        .collect::<Vec<_>>();
    if common_cell_order_before != common_cell_order_after {
        expected.insert(IntentPatchOperationKind::ReorderCells);
    }
    if history_declaration_move_required(before, after, created_nodes, deleted_nodes) {
        expected.insert(IntentPatchOperationKind::MoveDeclaration);
    }
    if before.external_inputs != after.external_inputs {
        expected.insert(IntentPatchOperationKind::ReplaceExternalInputs);
    }

    Some(expected.into_iter().collect())
}

fn history_ports_eq(left: &crate::IntentNode, right: &crate::IntentNode) -> bool {
    left.ports.len() == right.ports.len()
        && left.ports.iter().all(|(port, left_port)| {
            right.ports.get(port).is_some_and(|right_port| {
                left_port.id == right_port.id
                    && left_port.selector == right_port.selector
                    && left_port.kind == right_port.kind
                    && left_port.writable == right_port.writable
                    && history_identity_flow_eq(left_port.flow, right_port.flow)
            })
        })
}

fn history_declaration_locations(
    organization: &IntentOrganization,
    retained: &BTreeSet<NodeId>,
) -> BTreeMap<NodeId, (crate::CellId, usize)> {
    let mut locations = BTreeMap::new();
    for (cell, value) in &organization.cells {
        for (index, node) in value
            .declarations
            .iter()
            .filter(|node| retained.contains(node))
            .enumerate()
        {
            locations.insert(*node, (*cell, index));
        }
    }
    locations
}

fn history_declaration_move_required(
    before: &SessionCheckpoint,
    after: &SessionCheckpoint,
    created_nodes: &BTreeSet<NodeId>,
    deleted_nodes: &BTreeSet<NodeId>,
) -> bool {
    let retained = before
        .graph
        .nodes()
        .keys()
        .filter(|node| !deleted_nodes.contains(node) && !created_nodes.contains(node))
        .copied()
        .collect::<BTreeSet<_>>();
    let mut expected = after
        .organization
        .cells
        .keys()
        .map(|cell| (*cell, Vec::new()))
        .collect::<BTreeMap<_, _>>();
    for (cell, value) in &before.organization.cells {
        if let Some(declarations) = expected.get_mut(cell) {
            declarations.extend(
                value
                    .declarations
                    .iter()
                    .filter(|node| retained.contains(node))
                    .copied(),
            );
        }
    }
    let mut deleted_cells = before
        .organization
        .cells
        .keys()
        .filter(|cell| !after.organization.cells.contains_key(cell))
        .copied()
        .collect::<Vec<_>>();
    deleted_cells
        .sort_by_key(|cell| IntentPatchOperation::DeleteCell { cell: *cell }.canonical_bytes());
    let default = expected
        .get_mut(&after.organization.default_cell)
        .expect("validated organization retains its default cell");
    for cell in deleted_cells {
        default.extend(
            before.organization.cells[&cell]
                .declarations
                .iter()
                .filter(|node| retained.contains(node))
                .copied(),
        );
    }
    let actual = after
        .organization
        .cells
        .iter()
        .map(|(cell, value)| {
            (
                *cell,
                value
                    .declarations
                    .iter()
                    .filter(|node| retained.contains(node))
                    .copied()
                    .collect::<Vec<_>>(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    expected != actual
}

fn history_organization_nodes_match(
    before: &SessionCheckpoint,
    after: &SessionCheckpoint,
    created_nodes: &BTreeSet<NodeId>,
    deleted_nodes: &BTreeSet<NodeId>,
    claimed: &BTreeSet<NodeId>,
) -> bool {
    let retained = before
        .graph
        .nodes()
        .keys()
        .filter(|node| !deleted_nodes.contains(node) && !created_nodes.contains(node))
        .copied()
        .collect::<BTreeSet<_>>();
    if claimed.iter().any(|node| !retained.contains(node)) {
        return false;
    }
    let before_locations = history_declaration_locations(&before.organization, &retained);
    let after_locations = history_declaration_locations(&after.organization, &retained);
    let mandatory = retained
        .iter()
        .filter(|node| {
            before.organization.node_names.get(node) != after.organization.node_names.get(node)
                || before_locations.get(node).map(|location| location.0)
                    != after_locations.get(node).map(|location| location.0)
        })
        .copied()
        .collect::<BTreeSet<_>>();
    if !mandatory.is_subset(claimed) {
        return false;
    }
    if claimed.iter().any(|node| {
        !mandatory.contains(node) && before_locations.get(node) == after_locations.get(node)
    }) {
        return false;
    }
    for cell in before
        .organization
        .cells
        .keys()
        .filter(|cell| after.organization.cells.contains_key(cell))
    {
        let before_order = before.organization.cells[cell]
            .declarations
            .iter()
            .filter(|node| retained.contains(node) && !claimed.contains(node))
            .copied()
            .collect::<Vec<_>>();
        let after_order = after.organization.cells[cell]
            .declarations
            .iter()
            .filter(|node| retained.contains(node) && !claimed.contains(node))
            .copied()
            .collect::<Vec<_>>();
        if before_order != after_order {
            return false;
        }
    }
    true
}

fn history_node_definition_eq(left: &crate::IntentNode, right: &crate::IntentNode) -> bool {
    left.id == right.id
        && left.symbol == right.symbol
        && left.kind == right.kind
        && left.bootstrap_origin == right.bootstrap_origin
        && left.suppressed == right.suppressed
        && left.inputs == right.inputs
        && left.fields == right.fields
        && left.operation_outputs == right.operation_outputs
        && left.reservations == right.reservations
        && left.child_order == right.child_order
        && left.children == right.children
        && history_ports_eq(left, right)
}

fn history_identity_flow_eq(left: IntentIdentityFlow, right: IntentIdentityFlow) -> bool {
    match (left, right) {
        (
            IntentIdentityFlow::Continued { source: left, .. },
            IntentIdentityFlow::Continued { source: right, .. },
        )
        | (
            IntentIdentityFlow::Retired { source: left, .. },
            IntentIdentityFlow::Retired { source: right, .. },
        ) => left == right,
        _ => left == right,
    }
}

/// Retained design-intent authority and bounded transaction history.
#[derive(Clone, Debug, PartialEq)]
pub struct IntentSession {
    id: IntentSessionId,
    revision: Revision,
    identity: IntentSessionIdentity,
    semantic_identity: IntentSemanticIdentity,
    graph: IntentGraph,
    instance: IntentInstanceState,
    reservations: IntentReservationLedger,
    organization: IntentOrganization,
    external_inputs: IntentExternalInputs,
    latest_attempt: Option<IntentLatestAttempt>,
    accepted: Option<IntentAcceptedAuthority>,
    undo: Vec<HistoryEntry>,
    redo: Vec<HistoryEntry>,
    allocator: IntentAllocatorHighWater,
}

impl IntentSession {
    /// Creates an empty session with stable caller-supplied namespace and the
    /// built-in non-deletable `Sketch` presentation cell.
    ///
    /// # Errors
    ///
    /// Returns an error only if the built-in organization key is invalid.
    pub fn with_id(id: IntentSessionId) -> Result<Self, IntentSessionError> {
        let sketch = IntentKey::new("Sketch")?;
        let revision = Revision::from_raw(0);
        let graph = IntentGraph::empty();
        let instance = IntentInstanceState::empty();
        let reservations = IntentReservationLedger::empty();
        let organization = IntentOrganization::new(CellId::from_raw(1), sketch);
        let external_inputs = IntentExternalInputs::default();
        let latest_attempt = None;
        let accepted = None;
        let undo = Vec::new();
        let redo = Vec::new();
        let allocator = IntentAllocatorHighWater::initial();
        let semantic_identity =
            semantic_identity(&graph, &instance, reservations.identity(), &external_inputs);
        let identity = session_identity(
            id,
            revision,
            &graph,
            &instance,
            &reservations,
            &organization,
            &external_inputs,
            latest_attempt.as_ref(),
            accepted.as_ref(),
            &undo,
            &redo,
            allocator,
        );
        Ok(Self {
            id,
            revision,
            identity,
            semantic_identity,
            graph,
            instance,
            reservations,
            organization,
            external_inputs,
            latest_attempt: None,
            accepted: None,
            undo,
            redo,
            allocator,
        })
    }

    #[must_use]
    pub const fn id(&self) -> IntentSessionId {
        self.id
    }

    #[must_use]
    pub const fn graph(&self) -> &IntentGraph {
        &self.graph
    }

    #[must_use]
    pub const fn instance(&self) -> &IntentInstanceState {
        &self.instance
    }

    #[must_use]
    pub const fn reservations(&self) -> &IntentReservationLedger {
        &self.reservations
    }

    #[must_use]
    pub const fn organization(&self) -> &IntentOrganization {
        &self.organization
    }

    #[must_use]
    pub const fn external_inputs(&self) -> &IntentExternalInputs {
        &self.external_inputs
    }

    #[must_use]
    pub const fn latest_attempt(&self) -> Option<&IntentLatestAttempt> {
        self.latest_attempt.as_ref()
    }

    #[must_use]
    pub const fn accepted(&self) -> Option<&IntentAcceptedAuthority> {
        self.accepted.as_ref()
    }

    #[must_use]
    pub const fn allocator_high_water(&self) -> IntentAllocatorHighWater {
        self.allocator
    }

    #[must_use]
    pub const fn semantic_identity(&self) -> IntentSemanticIdentity {
        self.semantic_identity
    }

    #[must_use]
    pub const fn identity(&self) -> IntentSessionIdentity {
        self.identity
    }

    /// Plans and fully validates one unordered patch without mutating the
    /// session. The materializer closure is called exactly once only when
    /// graph, instance, or external-input state changed.
    ///
    /// # Errors
    ///
    /// Returns a typed planning, structural, evaluation, resource, or stale-
    /// authority error. Every error leaves the session unchanged.
    #[allow(
        clippy::too_many_lines,
        reason = "one staging path authenticates the complete exact-CAS transaction before publication"
    )]
    pub fn plan_patch<F>(
        &self,
        patch: IntentPatch,
        materialize: F,
    ) -> Result<IntentPatchPlan, IntentPlanError>
    where
        F: FnOnce(&IntentCandidate) -> IntentEvaluation,
    {
        self.plan_patch_with_history(patch, materialize, true)
    }

    /// Plans and fully validates one unordered patch without constructing a
    /// nested user Undo/Redo snapshot.
    ///
    /// This is the narrow transaction seam for an adjacent composite owner
    /// which already owns the sole user-visible history row. The source must
    /// already be a history-free delegated checkpoint; the accepted graph,
    /// materialization evidence, exact-CAS revision, allocator high-water and
    /// plan-token checks otherwise remain identical to [`Self::plan_patch`].
    ///
    /// # Errors
    ///
    /// Returns the ordinary typed planning error, or rejects a source which
    /// still contains nested Undo/Redo. Every error leaves the session
    /// unchanged.
    pub fn plan_delegated_patch<F>(
        &self,
        patch: IntentPatch,
        materialize: F,
    ) -> Result<IntentPatchPlan, IntentPlanError>
    where
        F: FnOnce(&IntentCandidate) -> IntentEvaluation,
    {
        if !self.undo.is_empty() || !self.redo.is_empty() {
            return Err(IntentSessionError::DelegatedCheckpointContainsHistory.into());
        }
        self.plan_patch_with_history(patch, materialize, false)
    }

    #[allow(
        clippy::too_many_lines,
        reason = "one staging path authenticates the complete exact-CAS transaction before publication"
    )]
    fn plan_patch_with_history<F>(
        &self,
        patch: IntentPatch,
        materialize: F,
        publish_history: bool,
    ) -> Result<IntentPatchPlan, IntentPlanError>
    where
        F: FnOnce(&IntentCandidate) -> IntentEvaluation,
    {
        self.ensure_current(patch.expected)?;
        if patch.operations().len() > MAX_INTENT_PATCH_OPERATIONS {
            return Err(IntentPlanError::ResourceLimit {
                resource: "patch operations",
                actual: patch.operations().len(),
                limit: MAX_INTENT_PATCH_OPERATIONS,
            });
        }
        validate_patch_conflicts(patch.operations())?;
        let policy = patch.policy;
        let base_checkpoint = self.checkpoint();
        let mut staged = base_checkpoint.clone();
        let mut reservations = self.reservations.clone();
        let mut allocator = self.allocator;
        let mut aliases = IntentAliasMap::default();
        let mut diff = IntentSemanticDiff::default();
        apply_patch_operations(
            patch.into_operations(),
            &mut staged,
            &mut allocator,
            &mut aliases,
            &mut diff,
        )?;
        if aliases
            .nodes
            .values()
            .any(|node| !staged.graph.nodes().contains_key(node))
            || aliases
                .cells
                .values()
                .any(|cell| !staged.organization.cells().contains_key(cell))
        {
            return Err(IntentPlanError::ConflictingOperations {
                target: "newly allocated identity deleted in the same patch".into(),
            });
        }
        if !diff.graph_changed
            && !diff.instance_changed
            && !diff.organization_changed
            && !diff.external_inputs_changed
        {
            return Err(IntentPlanError::NoChanges);
        }

        if diff.graph_changed {
            staged.graph.revision =
                next_revision(staged.graph.revision).ok_or(IntentPlanError::RevisionExhausted)?;
        }
        if diff.instance_changed {
            staged.instance.revision = next_revision(staged.instance.revision)
                .ok_or(IntentPlanError::RevisionExhausted)?;
        }
        if diff.organization_changed {
            staged.organization.revision = next_revision(staged.organization.revision)
                .ok_or(IntentPlanError::RevisionExhausted)?;
        }
        assign_identity_generations(&mut staged.graph)?;
        staged.graph.validate()?;
        if reservations.reconcile(&staged.graph)? {
            reservations.revision =
                next_revision(reservations.revision).ok_or(IntentPlanError::RevisionExhausted)?;
        }
        staged.reservation_identity = reservations.identity();
        staged.reservation_high_water = Some(allocator.next_reservation);
        validate_organization(&staged.graph, &staged.organization)?;
        let semantic = semantic_identity(
            &staged.graph,
            &staged.instance,
            staged.reservation_identity,
            &staged.external_inputs,
        );
        let candidate = IntentCandidate {
            graph: staged.graph.clone(),
            instance: staged.instance.clone(),
            reservations: reservations.clone(),
            organization: staged.organization.clone(),
            external_inputs: staged.external_inputs.clone(),
            semantic_identity: semantic,
            diff: diff.clone(),
        };

        let disposition = if diff.requires_materialization() {
            match materialize(&candidate) {
                IntentEvaluation::Accepted { evidence } => {
                    evidence.validate()?;
                    if evidence.external_inputs != staged.external_inputs.identity() {
                        return Err(IntentPlanError::MaterializationInputMismatch);
                    }
                    staged.latest_attempt = Some(IntentLatestAttempt {
                        target: semantic,
                        disposition: IntentAttemptDisposition::Accepted,
                        materialization_digest: Some(evidence.digest),
                        failed_nodes: BTreeSet::new(),
                        diagnostic: None,
                    });
                    staged.accepted = Some(IntentAcceptedAuthority {
                        target: semantic,
                        graph: staged.graph.clone(),
                        instance: staged.instance.clone(),
                        reservations: reservations.clone(),
                        external_inputs: staged.external_inputs.clone(),
                        evidence,
                    });
                    IntentPlanDisposition::Accepted
                }
                IntentEvaluation::Failed { failure } => {
                    validate_failure(&staged.graph, &failure)?;
                    if failure.kind.nonpublishing() {
                        return Err(IntentPlanError::NonpublishingEvaluation { failure });
                    }
                    if policy == IntentPatchPolicy::RequireAccepted {
                        return Err(IntentPlanError::EvaluationRejected { failure });
                    }
                    staged.latest_attempt = Some(IntentLatestAttempt {
                        target: semantic,
                        disposition: IntentAttemptDisposition::RetainedFailed,
                        materialization_digest: None,
                        failed_nodes: failure.failed_nodes,
                        diagnostic: Some(failure.diagnostic),
                    });
                    // Deliberately preserve the complete prior accepted authority.
                    IntentPlanDisposition::RetainedFailed
                }
            }
        } else {
            // Organization is non-semantic and cannot invalidate accepted bytes.
            staged.latest_attempt.clone_from(&self.latest_attempt);
            staged.accepted.clone_from(&self.accepted);
            IntentPlanDisposition::OrganizationOnly
        };

        let revision = next_revision(self.revision).ok_or(IntentPlanError::RevisionExhausted)?;
        let created_nodes = staged
            .graph
            .nodes()
            .keys()
            .filter(|node| !base_checkpoint.graph.nodes().contains_key(node))
            .copied()
            .collect::<BTreeSet<_>>();
        let deleted_nodes = base_checkpoint
            .graph
            .nodes()
            .keys()
            .filter(|node| !staged.graph.nodes().contains_key(node))
            .copied()
            .collect::<BTreeSet<_>>();
        let operation_kinds =
            history_operation_kinds(&base_checkpoint, &staged, &created_nodes, &deleted_nodes)
                .ok_or(IntentSessionError::InvalidHistoryDescriptor)?;
        let descriptor =
            IntentTransactionDescriptor::new(revision, disposition, operation_kinds, diff.clone());
        descriptor.validate()?;
        let mut undo = if publish_history {
            self.undo.clone()
        } else {
            Vec::new()
        };
        if publish_history {
            push_bounded(
                &mut undo,
                HistoryEntry {
                    before: history_checkpoint_digest(&base_checkpoint),
                    after: history_checkpoint_digest(&staged),
                    checkpoint: base_checkpoint,
                    descriptor: descriptor.clone(),
                },
            );
        }
        let redo = Vec::new();
        let target = session_identity(
            self.id,
            revision,
            &staged.graph,
            &staged.instance,
            &reservations,
            &staged.organization,
            &staged.external_inputs,
            staged.latest_attempt.as_ref(),
            staged.accepted.as_ref(),
            &undo,
            &redo,
            allocator,
        );
        let base = self.identity();
        let token = plan_token(
            base,
            target,
            disposition,
            &aliases,
            &diff,
            &descriptor,
            &staged,
            &reservations,
            &undo,
            &redo,
            allocator,
        );
        Ok(IntentPatchPlan {
            base,
            target,
            token,
            disposition,
            aliases,
            diff,
            descriptor,
            staged,
            reservations,
            undo,
            redo,
            allocator,
        })
    }

    /// Atomically publishes one exact planned candidate. Any intervening
    /// mutation or altered token rejects without changing session state.
    ///
    /// # Errors
    ///
    /// Returns an error for stale authority, an altered token, or revision
    /// exhaustion.
    pub fn commit_plan(
        &mut self,
        plan: IntentPatchPlan,
    ) -> Result<IntentSessionIdentity, IntentSessionError> {
        self.ensure_current(plan.base)?;
        let expected_token = plan_token(
            plan.base,
            plan.target,
            plan.disposition,
            &plan.aliases,
            &plan.diff,
            &plan.descriptor,
            &plan.staged,
            &plan.reservations,
            &plan.undo,
            &plan.redo,
            plan.allocator,
        );
        if expected_token != plan.token {
            return Err(IntentSessionError::InvalidPlanToken);
        }
        let mut staged = self.clone();
        staged.restore_checkpoint(plan.staged);
        staged.reservations = plan.reservations;
        staged.undo = plan.undo;
        staged.redo = plan.redo;
        staged.allocator = plan.allocator;
        staged.revision = plan.target.revision;
        staged.refresh_cached_identities();
        let actual = staged.identity();
        if actual != plan.target {
            return Err(IntentSessionError::InvalidPlanToken);
        }
        *self = staged;
        Ok(actual)
    }

    /// Commits the sole accepted initialization of a new session without
    /// presenting migration/bootstrap construction as a user Undo step.
    ///
    /// The plan must start at revision zero on a completely empty semantic
    /// session and contain only declaration creation. Normal interactive and
    /// code-authored patches must use [`Self::commit_plan`] instead.
    ///
    /// # Errors
    ///
    /// Rejects a non-empty session, a non-accepted plan, any non-creation
    /// operation, or the ordinary stale/token/session validation failures.
    pub fn commit_initialization_plan(
        &mut self,
        plan: IntentPatchPlan,
    ) -> Result<IntentSessionIdentity, IntentSessionError> {
        let is_empty = self.revision.raw() == 0
            && self.graph.nodes().is_empty()
            && self.instance.values().is_empty()
            && self.accepted.is_none()
            && self.latest_attempt.is_none()
            && self.undo.is_empty()
            && self.redo.is_empty();
        let is_initialization = plan.disposition == IntentPlanDisposition::Accepted
            && !plan.descriptor.operation_kinds.is_empty()
            && plan
                .descriptor
                .operation_kinds
                .iter()
                .all(|kind| *kind == IntentPatchOperationKind::CreateNode);
        if !is_empty || !is_initialization {
            return Err(IntentSessionError::InvalidInitializationPlan);
        }
        self.commit_plan(plan)?;
        self.undo.clear();
        self.redo.clear();
        self.refresh_cached_identities();
        self.validate()?;
        Ok(self.identity())
    }

    /// Installs independently validated host evidence for the pristine empty
    /// graph without creating a declaration or user-visible Undo entry.
    ///
    /// This is the sole initialization path for a fresh projectional editor:
    /// it establishes an accepted empty native canvas before the first authored
    /// patch while keeping revision zero and the built-in empty organization.
    ///
    /// # Errors
    ///
    /// Rejects a session with any retained semantic state, allocation, attempt,
    /// accepted authority or history, and rejects evidence stamped for different
    /// external inputs.
    pub fn install_pristine_empty_acceptance(
        &mut self,
        evidence: MaterializationEvidence,
    ) -> Result<IntentSessionIdentity, IntentSessionError> {
        evidence.validate()?;
        let pristine = self.revision.raw() == 0
            && self.graph.nodes().is_empty()
            && self.instance.values().is_empty()
            && self.reservations.entries().is_empty()
            && self.organization.node_names().is_empty()
            && self.external_inputs == IntentExternalInputs::default()
            && self.latest_attempt.is_none()
            && self.accepted.is_none()
            && self.undo.is_empty()
            && self.redo.is_empty()
            && self.allocator == IntentAllocatorHighWater::initial();
        if !pristine || evidence.external_inputs != self.external_inputs.identity() {
            return Err(IntentSessionError::InvalidEmptyInitialization);
        }
        let mut staged = self.clone();
        let target = semantic_identity(
            &staged.graph,
            &staged.instance,
            staged.reservations.identity(),
            &staged.external_inputs,
        );
        staged.latest_attempt = Some(IntentLatestAttempt {
            target,
            disposition: IntentAttemptDisposition::Accepted,
            materialization_digest: Some(evidence.digest),
            failed_nodes: BTreeSet::new(),
            diagnostic: None,
        });
        staged.accepted = Some(IntentAcceptedAuthority {
            target,
            graph: staged.graph.clone(),
            instance: staged.instance.clone(),
            reservations: staged.reservations.clone(),
            external_inputs: staged.external_inputs.clone(),
            evidence,
        });
        staged.refresh_cached_identities();
        staged.validate()?;
        *self = staged;
        Ok(self.identity())
    }

    /// Restores the preceding complete transaction checkpoint with a fresh
    /// overall exact-CAS revision. Stable IDs and allocator high-water remain
    /// exact and never regress.
    ///
    /// # Errors
    ///
    /// Returns an error if the revision exhausts or restored authority fails
    /// validation.
    pub fn undo(&mut self) -> Result<Option<IntentSessionIdentity>, IntentSessionError> {
        if self.undo.is_empty() {
            return Ok(None);
        }
        let mut staged = self.clone();
        let Some(entry) = staged.undo.pop() else {
            return Ok(None);
        };
        let HistoryEntry {
            checkpoint,
            descriptor,
            before,
            after,
        } = entry;
        let current = staged.checkpoint();
        push_bounded(
            &mut staged.redo,
            HistoryEntry {
                checkpoint: current,
                descriptor,
                before,
                after,
            },
        );
        staged.restore_history_checkpoint(checkpoint)?;
        staged.revision =
            next_revision(staged.revision).ok_or(IntentSessionError::RevisionExhausted)?;
        staged.refresh_cached_identities();
        staged.validate()?;
        *self = staged;
        Ok(Some(self.identity()))
    }

    /// Restores the next complete transaction checkpoint with a fresh overall
    /// exact-CAS revision.
    ///
    /// # Errors
    ///
    /// Returns an error if the revision exhausts or restored authority fails
    /// validation.
    pub fn redo(&mut self) -> Result<Option<IntentSessionIdentity>, IntentSessionError> {
        if self.redo.is_empty() {
            return Ok(None);
        }
        let mut staged = self.clone();
        let Some(entry) = staged.redo.pop() else {
            return Ok(None);
        };
        let HistoryEntry {
            checkpoint,
            descriptor,
            before,
            after,
        } = entry;
        let current = staged.checkpoint();
        push_bounded(
            &mut staged.undo,
            HistoryEntry {
                checkpoint: current,
                descriptor,
                before,
                after,
            },
        );
        staged.restore_history_checkpoint(checkpoint)?;
        staged.revision =
            next_revision(staged.revision).ok_or(IntentSessionError::RevisionExhausted)?;
        staged.refresh_cached_identities();
        staged.validate()?;
        *self = staged;
        Ok(Some(self.identity()))
    }

    #[must_use]
    pub fn undo_len(&self) -> usize {
        self.undo.len()
    }

    #[must_use]
    pub fn redo_len(&self) -> usize {
        self.redo.len()
    }

    /// Returns a deterministic, clock-free read-only transaction projection.
    #[must_use]
    pub fn history_projection(&self) -> IntentHistoryProjection {
        IntentHistoryProjection {
            applied: self
                .undo
                .iter()
                .map(|entry| entry.descriptor.clone())
                .collect(),
            redoable: self
                .redo
                .iter()
                .rev()
                .map(|entry| entry.descriptor.clone())
                .collect(),
        }
    }

    /// Re-evaluates the current accepted authority after an Undo/Redo restore
    /// retained newer reservation/tombstone high-water.
    ///
    /// History restoration deliberately advances component identities and
    /// never discards reservations observed later in the session. When the
    /// restored accepted scene belongs to that current semantic state, its
    /// native materialization evidence must therefore be regenerated by the
    /// authoritative host before publication. This method performs that one
    /// history-neutral refresh atomically. It neither creates a transaction nor
    /// changes graph, instance, organization, external inputs, allocator, or
    /// overall revision.
    ///
    /// Returns `Ok(false)` when the retained accepted authority is historical
    /// beneath current failed intent and needs no refresh.
    ///
    /// # Errors
    ///
    /// Returns a typed evaluation/input/session error and leaves the complete
    /// session unchanged unless the host returns accepted evidence for the
    /// exact current semantic identity.
    pub fn refresh_current_accepted_evidence<F>(
        &mut self,
        materialize: F,
    ) -> Result<bool, IntentPlanError>
    where
        F: FnOnce(&IntentCandidate) -> IntentEvaluation,
    {
        let semantic = self.semantic_identity();
        let Some(accepted) = self.accepted.as_ref() else {
            return Ok(false);
        };
        if accepted.target != semantic {
            return Ok(false);
        }
        let candidate = IntentCandidate {
            graph: self.graph.clone(),
            instance: self.instance.clone(),
            reservations: self.reservations.clone(),
            organization: self.organization.clone(),
            external_inputs: self.external_inputs.clone(),
            semantic_identity: semantic,
            diff: IntentSemanticDiff::default(),
        };
        let evidence = match materialize(&candidate) {
            IntentEvaluation::Accepted { evidence } => evidence,
            IntentEvaluation::Failed { failure } if failure.kind.nonpublishing() => {
                return Err(IntentPlanError::NonpublishingEvaluation { failure });
            }
            IntentEvaluation::Failed { failure } => {
                validate_failure(&self.graph, &failure)?;
                return Err(IntentPlanError::EvaluationRejected { failure });
            }
        };
        evidence.validate()?;
        if evidence.external_inputs != self.external_inputs.identity() {
            return Err(IntentPlanError::MaterializationInputMismatch);
        }
        let mut staged = self.clone();
        let authority = staged
            .accepted
            .as_mut()
            .ok_or(IntentSessionError::InvalidAuthority)?;
        authority.graph.clone_from(&staged.graph);
        authority.instance.clone_from(&staged.instance);
        authority.reservations.clone_from(&staged.reservations);
        authority
            .external_inputs
            .clone_from(&staged.external_inputs);
        authority.target = semantic;
        authority.evidence = evidence;
        let latest = staged
            .latest_attempt
            .as_mut()
            .ok_or(IntentSessionError::InvalidAuthority)?;
        if latest.target != semantic
            || latest.disposition != IntentAttemptDisposition::Accepted
            || !latest.failed_nodes.is_empty()
            || latest.diagnostic.is_some()
        {
            return Err(IntentSessionError::InvalidAuthority.into());
        }
        latest.materialization_digest = Some(authority.evidence.digest);
        staged.propagate_current_accepted_authority_into_redo()?;
        staged.refresh_cached_identities();
        staged.validate()?;
        *self = staged;
        Ok(true)
    }

    /// Exports current intent, exact accepted/attempt authority, monotonic
    /// allocators, and bounded history as strict deterministic JSON.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid authority, excessive output, or JSON
    /// serialization failure.
    pub fn to_canonical_json(&self) -> Result<String, IntentSessionError> {
        self.validate()?;
        let mut wire = IntentSessionWire {
            version: INTENT_SESSION_VERSION,
            id: self.id,
            revision: self.revision,
            current: self.checkpoint(),
            undo: self.undo.clone(),
            redo: self.redo.clone(),
            reservations: self.reservations.clone(),
            allocator: self.allocator,
            digest: ContentDigest::zero(),
        };
        wire.digest = session_wire_digest(&wire);
        let json = serde_json::to_string(&wire)?;
        if json.len() > MAX_INTENT_SESSION_JSON_BYTES {
            return Err(IntentSessionError::JsonResourceLimit {
                limit: MAX_INTENT_SESSION_JSON_BYTES,
            });
        }
        Ok(json)
    }

    /// Returns an authenticated clone with identical current/accepted intent
    /// and monotonic allocator authority but no nested user Undo/Redo.
    ///
    /// This is the neutral delegated-checkpoint seam for an adjacent composite
    /// session which owns the one user-visible history. It does not rewrite
    /// graph, instance, organization, reservations, accepted evidence or the
    /// overall revision, and it never mutates the source session.
    ///
    /// # Errors
    ///
    /// Returns an error if the source session or the resulting history-free
    /// authority fails validation.
    pub fn delegated_checkpoint(&self) -> Result<Self, IntentSessionError> {
        self.validate()?;
        let mut delegated = Self {
            id: self.id,
            revision: self.revision,
            identity: self.identity,
            semantic_identity: self.semantic_identity,
            graph: self.graph.clone(),
            instance: self.instance.clone(),
            reservations: self.reservations.clone(),
            organization: self.organization.clone(),
            external_inputs: self.external_inputs.clone(),
            latest_attempt: self.latest_attempt.clone(),
            accepted: self.accepted.clone(),
            undo: Vec::new(),
            redo: Vec::new(),
            allocator: self.allocator,
        };
        delegated.refresh_cached_identities();
        delegated.validate()?;
        Ok(delegated)
    }

    /// Consumes this session and removes nested user Undo/Redo in place.
    ///
    /// This is the allocation- and stack-bounded form for adjacent composite
    /// owners which already own the session value. Unlike
    /// [`Self::delegated_checkpoint`], it does not clone the current graph,
    /// accepted authority, or potentially deep bounded history before
    /// discarding that history.
    ///
    /// # Errors
    ///
    /// Returns an error if the resulting history-free authority fails strict
    /// validation. A constructed [`IntentSession`] already satisfies its
    /// private source invariants; history is discarded before validation so a
    /// deep but bounded outer-owned history is never needlessly traversed.
    pub fn into_delegated_checkpoint(mut self) -> Result<Self, IntentSessionError> {
        self.undo.clear();
        self.redo.clear();
        self.refresh_cached_identities();
        self.validate()?;
        Ok(self)
    }

    /// Encodes [`Self::delegated_checkpoint`] using the ordinary strict
    /// canonical session wire, with authenticated empty Undo and Redo arrays.
    ///
    /// # Errors
    ///
    /// Returns an error when the source session, delegated checkpoint, or
    /// canonical encoding fails validation or its resource bound.
    pub fn to_delegated_checkpoint_json(&self) -> Result<String, IntentSessionError> {
        self.delegated_checkpoint()?.to_canonical_json()
    }

    /// Decodes a strict canonical session only when it contains no nested
    /// user history. This prevents a composite history owner from
    /// accidentally accepting an ordinary self-owning session checkpoint.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed, non-canonical, unauthenticated, or
    /// history-bearing session JSON.
    pub fn from_delegated_checkpoint_json(json: &str) -> Result<Self, IntentSessionError> {
        let session = Self::from_json(json)?;
        if !session.undo.is_empty() || !session.redo.is_empty() {
            return Err(IntentSessionError::DelegatedCheckpointContainsHistory);
        }
        Ok(session)
    }

    /// Imports strict canonical intent-session JSON and authenticates all
    /// current/history structure and high-water state.
    ///
    /// # Errors
    ///
    /// Returns an error for excessive, malformed, unsupported, digest-
    /// mismatched, or structurally invalid input.
    pub fn from_json(json: &str) -> Result<Self, IntentSessionError> {
        Self::from_authenticated_wire(authenticated_session_wire(json)?)
    }

    /// Imports one already-authenticated pre-M91 session whose contact
    /// declarations carry only the redundant topology defaults removed by
    /// M91's intrinsic-contact schema.
    ///
    /// This is a deliberately narrow migration seam for historical
    /// reproduction capsules. Ordinary session import remains strict through
    /// [`Self::from_json`]. The source wire must still be canonical and
    /// digest-authenticated, every other declaration field remains subject to
    /// the current closed schema, and only exact legacy defaults are removed.
    /// Explicit supporting-line intent is translated to the current
    /// `*_support` field. Explicit line and Polyline branch directions are
    /// normalized exactly once to preserve the historical materializer's
    /// accepted native meaning now that current source retains their bits.
    /// Non-default bounds, periods, hybrid old/new fields, and unrelated
    /// unknown fields fail closed.
    ///
    /// # Errors
    ///
    /// Returns the ordinary canonical/session validation errors, or a typed
    /// historical-contact error when the authenticated wire is not within the
    /// exact migration subset.
    #[doc(hidden)]
    pub fn from_historical_contact_domain_json(json: &str) -> Result<Self, IntentSessionError> {
        let mut wire = authenticated_session_wire(json)?;
        validate_historical_contact_wire_authority(&wire)?;
        let mut migrated_fields = 0_usize;
        migrate_historical_contact_checkpoint(&mut wire.current, &mut migrated_fields)?;
        for entry in wire.undo.iter_mut().chain(&mut wire.redo) {
            migrate_historical_contact_checkpoint(&mut entry.checkpoint, &mut migrated_fields)?;
        }
        if migrated_fields == 0 {
            return Err(IntentSessionError::HistoricalContactDomainMigrationNotApplicable);
        }
        bind_history_edges(&wire.current, &mut wire.undo, &mut wire.redo);
        Self::from_authenticated_wire(wire)
    }

    fn from_authenticated_wire(wire: IntentSessionWire) -> Result<Self, IntentSessionError> {
        if wire.current.reservation_high_water != Some(wire.allocator.next_reservation) {
            return Err(IntentSessionError::InvalidAuthority);
        }
        let semantic_identity = semantic_identity(
            &wire.current.graph,
            &wire.current.instance,
            wire.reservations.identity(),
            &wire.current.external_inputs,
        );
        let identity = session_identity(
            wire.id,
            wire.revision,
            &wire.current.graph,
            &wire.current.instance,
            &wire.reservations,
            &wire.current.organization,
            &wire.current.external_inputs,
            wire.current.latest_attempt.as_ref(),
            wire.current.accepted.as_ref(),
            &wire.undo,
            &wire.redo,
            wire.allocator,
        );
        let session = Self {
            id: wire.id,
            revision: wire.revision,
            identity,
            semantic_identity,
            graph: wire.current.graph,
            instance: wire.current.instance,
            reservations: wire.reservations,
            organization: wire.current.organization,
            external_inputs: wire.current.external_inputs,
            latest_attempt: wire.current.latest_attempt,
            accepted: wire.current.accepted,
            undo: wire.undo,
            redo: wire.redo,
            allocator: wire.allocator,
        };
        session.validate()?;
        Ok(session)
    }

    fn checkpoint(&self) -> SessionCheckpoint {
        SessionCheckpoint {
            graph: self.graph.clone(),
            instance: self.instance.clone(),
            reservation_identity: self.reservations.identity(),
            reservation_high_water: Some(self.allocator.next_reservation),
            organization: self.organization.clone(),
            external_inputs: self.external_inputs.clone(),
            latest_attempt: self.latest_attempt.clone(),
            accepted: self.accepted.clone(),
        }
    }

    fn restore_checkpoint(&mut self, checkpoint: SessionCheckpoint) {
        self.graph = checkpoint.graph;
        self.instance = checkpoint.instance;
        self.organization = checkpoint.organization;
        self.external_inputs = checkpoint.external_inputs;
        self.latest_attempt = checkpoint.latest_attempt;
        self.accepted = checkpoint.accepted;
    }

    fn refresh_cached_identities(&mut self) {
        self.semantic_identity = semantic_identity(
            &self.graph,
            &self.instance,
            self.reservations.identity(),
            &self.external_inputs,
        );
        self.identity = session_identity(
            self.id,
            self.revision,
            &self.graph,
            &self.instance,
            &self.reservations,
            &self.organization,
            &self.external_inputs,
            self.latest_attempt.as_ref(),
            self.accepted.as_ref(),
            &self.undo,
            &self.redo,
            self.allocator,
        );
    }

    fn restore_history_checkpoint(
        &mut self,
        mut checkpoint: SessionCheckpoint,
    ) -> Result<(), IntentSessionError> {
        let graph_changed = self.graph.nodes != checkpoint.graph.nodes;
        let instance_changed = self.instance.values != checkpoint.instance.values;
        let organization_changed = self.organization.default_cell
            != checkpoint.organization.default_cell
            || self.organization.cell_order != checkpoint.organization.cell_order
            || self.organization.cells != checkpoint.organization.cells
            || self.organization.node_names != checkpoint.organization.node_names;
        let checkpoint_semantic = semantic_identity(
            &checkpoint.graph,
            &checkpoint.instance,
            checkpoint.reservation_identity,
            &checkpoint.external_inputs,
        );
        let accepted_was_current = checkpoint
            .accepted
            .as_ref()
            .is_some_and(|accepted| accepted.target == checkpoint_semantic);

        checkpoint.graph.revision = if graph_changed {
            next_revision(self.graph.revision).ok_or(IntentSessionError::RevisionExhausted)?
        } else {
            self.graph.revision
        };
        checkpoint.instance.revision = if instance_changed {
            next_revision(self.instance.revision).ok_or(IntentSessionError::RevisionExhausted)?
        } else {
            self.instance.revision
        };
        checkpoint.organization.revision = if organization_changed {
            next_revision(self.organization.revision)
                .ok_or(IntentSessionError::RevisionExhausted)?
        } else {
            self.organization.revision
        };
        if self.reservations.reconcile(&checkpoint.graph)? {
            self.reservations.revision = next_revision(self.reservations.revision)
                .ok_or(IntentSessionError::RevisionExhausted)?;
        }
        checkpoint.reservation_identity = self.reservations.identity();
        checkpoint.reservation_high_water = Some(self.allocator.next_reservation);
        let restored_semantic = semantic_identity(
            &checkpoint.graph,
            &checkpoint.instance,
            checkpoint.reservation_identity,
            &checkpoint.external_inputs,
        );
        if let Some(attempt) = &mut checkpoint.latest_attempt {
            attempt.target = restored_semantic;
        }
        if accepted_was_current && let Some(accepted) = &mut checkpoint.accepted {
            accepted.target = restored_semantic;
            accepted.graph.clone_from(&checkpoint.graph);
            accepted.instance.clone_from(&checkpoint.instance);
            accepted.reservations.clone_from(&self.reservations);
        }
        self.restore_checkpoint(checkpoint);
        self.propagate_current_accepted_authority_into_redo()?;
        Ok(())
    }

    /// Carries history-neutral accepted-authority maintenance across the
    /// chronological Redo prefix of retained-failed transactions.
    ///
    /// Undo may reconcile reservation/tombstone high-water for the restored
    /// accepted state, and an explicit host refresh may replace its exact
    /// materialization evidence. Neither operation changes the logical scene
    /// retained beneath a failed intent. Keeping the exact authority equal on
    /// both sides of every such edge lets canonical import reject a forged
    /// authority instead of treating maintenance metadata as user history.
    fn propagate_current_accepted_authority_into_redo(&mut self) -> Result<(), IntentSessionError> {
        let mut accepted = self.accepted.clone();
        for entry in self.redo.iter_mut().rev() {
            if entry.descriptor.disposition == IntentPlanDisposition::RetainedFailed {
                if !history_accepted_content_eq(
                    accepted.as_ref(),
                    entry.checkpoint.accepted.as_ref(),
                ) {
                    return Err(IntentSessionError::InvalidHistoryTransition);
                }
                entry.checkpoint.accepted.clone_from(&accepted);
            } else {
                accepted.clone_from(&entry.checkpoint.accepted);
            }
        }
        Ok(())
    }

    fn ensure_current(&self, expected: IntentSessionIdentity) -> Result<(), IntentSessionError> {
        let actual = self.identity();
        if expected.session != self.id {
            return Err(IntentSessionError::WrongSession {
                expected: self.id,
                actual: expected.session,
            });
        }
        if expected != actual {
            return Err(IntentSessionError::StaleCas {
                expected: Box::new(expected),
                actual: Box::new(actual),
            });
        }
        Ok(())
    }

    fn validate(&self) -> Result<(), IntentSessionError> {
        if self.undo.len() > MAX_INTENT_HISTORY_ENTRIES
            || self.redo.len() > MAX_INTENT_HISTORY_ENTRIES
        {
            return Err(IntentSessionError::HistoryLimit);
        }
        if self.undo.iter().chain(&self.redo).any(|entry| {
            entry.descriptor.target_revision.raw() == 0
                || entry.descriptor.target_revision > self.revision
        }) || !self
            .undo
            .windows(2)
            .all(|pair| pair[0].descriptor.target_revision < pair[1].descriptor.target_revision)
            || !self
                .redo
                .windows(2)
                .all(|pair| pair[0].descriptor.target_revision > pair[1].descriptor.target_revision)
            || self
                .undo
                .last()
                .zip(self.redo.last())
                .is_some_and(|(undo, redo)| {
                    undo.descriptor.target_revision >= redo.descriptor.target_revision
                })
        {
            return Err(IntentSessionError::InvalidHistoryDescriptor);
        }
        self.reservations.validate_against_graph(&self.graph)?;
        let current = self.checkpoint();
        validate_checkpoint(&current, &self.reservations, true)?;
        for entry in self.undo.iter().chain(&self.redo) {
            entry.descriptor.validate()?;
            validate_checkpoint(&entry.checkpoint, &self.reservations, false)?;
        }
        validate_history_edges(&current, &self.undo, &self.redo)?;
        validate_history_descriptors(&current, &self.undo, &self.redo)?;
        validate_allocator(
            self.allocator,
            std::iter::once(&current)
                .chain(self.undo.iter().map(|entry| &entry.checkpoint))
                .chain(self.redo.iter().map(|entry| &entry.checkpoint)),
            &self.reservations,
        )?;
        let semantic = semantic_identity(
            &self.graph,
            &self.instance,
            self.reservations.identity(),
            &self.external_inputs,
        );
        let identity = session_identity(
            self.id,
            self.revision,
            &self.graph,
            &self.instance,
            &self.reservations,
            &self.organization,
            &self.external_inputs,
            self.latest_attempt.as_ref(),
            self.accepted.as_ref(),
            &self.undo,
            &self.redo,
            self.allocator,
        );
        if semantic != self.semantic_identity || identity != self.identity {
            return Err(IntentSessionError::InvalidCachedIdentity);
        }
        Ok(())
    }
}

fn authenticated_session_wire(json: &str) -> Result<IntentSessionWire, IntentSessionError> {
    if json.len() > MAX_INTENT_SESSION_JSON_BYTES {
        return Err(IntentSessionError::JsonResourceLimit {
            limit: MAX_INTENT_SESSION_JSON_BYTES,
        });
    }
    let mut wire: IntentSessionWire = serde_json::from_str(json)?;
    if !matches!(
        wire.version,
        LEGACY_INTENT_SESSION_VERSION | INTENT_SESSION_VERSION
    ) {
        return Err(IntentSessionError::UnsupportedVersion {
            expected: INTENT_SESSION_VERSION,
            actual: wire.version,
        });
    }
    let expected_digest = if wire.version == LEGACY_INTENT_SESSION_VERSION {
        legacy_session_wire_digest(&wire)
    } else {
        session_wire_digest(&wire)
    };
    if wire.digest != expected_digest {
        return Err(IntentSessionError::DigestMismatch);
    }
    if serde_json::to_string(&wire)? != json {
        return Err(IntentSessionError::NonCanonicalJson);
    }
    let reservation_identity = if wire.version == LEGACY_INTENT_SESSION_VERSION {
        wire.reservations.legacy_identity()
    } else {
        wire.reservations.identity()
    };
    if wire.current.reservation_identity != reservation_identity {
        return Err(IntentSessionError::InvalidAuthority);
    }
    if wire.version == LEGACY_INTENT_SESSION_VERSION {
        validate_legacy_wire_authority(&wire)?;
        migrate_legacy_wire_identities(&mut wire)?;
    }
    if wire.current.reservation_high_water != Some(wire.allocator.next_reservation) {
        return Err(IntentSessionError::InvalidAuthority);
    }
    Ok(wire)
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct IntentSessionWire {
    version: u32,
    id: IntentSessionId,
    revision: Revision,
    current: SessionCheckpoint,
    undo: Vec<HistoryEntry>,
    redo: Vec<HistoryEntry>,
    reservations: IntentReservationLedger,
    allocator: IntentAllocatorHighWater,
    digest: ContentDigest,
}

fn session_wire_digest(wire: &IntentSessionWire) -> ContentDigest {
    digest_bytes(
        &serde_json::to_vec(&(
            wire.version,
            wire.id,
            wire.revision,
            &wire.current,
            &wire.undo,
            &wire.redo,
            &wire.reservations,
            wire.allocator,
        ))
        .expect("session wire payload is infallibly serializable"),
    )
}

fn legacy_session_wire_digest(wire: &IntentSessionWire) -> ContentDigest {
    legacy_digest_bytes(
        &serde_json::to_vec(&(
            wire.version,
            wire.id,
            wire.revision,
            &wire.current,
            &wire.undo,
            &wire.redo,
            &wire.reservations,
            wire.allocator,
        ))
        .expect("session wire payload is infallibly serializable"),
    )
}

fn migrate_historical_contact_checkpoint(
    checkpoint: &mut SessionCheckpoint,
    migrated_fields: &mut usize,
) -> Result<(), IntentSessionError> {
    migrate_historical_contact_graph(&mut checkpoint.graph, migrated_fields)?;
    normalize_historical_branch_directions(&mut checkpoint.graph);
    let accepted_materialization_digest = if let Some(accepted) = &mut checkpoint.accepted {
        migrate_historical_contact_graph(&mut accepted.graph, migrated_fields)?;
        normalize_historical_branch_directions(&mut accepted.graph);
        accepted.target = semantic_identity(
            &accepted.graph,
            &accepted.instance,
            accepted.reservations.identity(),
            &accepted.external_inputs,
        );
        Some(accepted.evidence.digest)
    } else {
        None
    };
    let target = semantic_identity(
        &checkpoint.graph,
        &checkpoint.instance,
        checkpoint.reservation_identity,
        &checkpoint.external_inputs,
    );
    if let Some(attempt) = &mut checkpoint.latest_attempt {
        attempt.target = target;
        attempt.materialization_digest = match attempt.disposition {
            IntentAttemptDisposition::Accepted => accepted_materialization_digest,
            IntentAttemptDisposition::RetainedFailed
            | IntentAttemptDisposition::OrganizationOnly => None,
        };
    }
    Ok(())
}

fn normalize_historical_branch_directions(graph: &mut IntentGraph) {
    for node in graph.nodes.values_mut() {
        let accepts_field = |name: &str| match node.kind {
            crate::IntentNodeKind::Geometry {
                recipe: crate::GeometryRecipeKind::Segment | crate::GeometryRecipeKind::MidpointLine,
            } => name == "branch_direction",
            crate::IntentNodeKind::Geometry {
                recipe: crate::GeometryRecipeKind::Polyline,
            } => name
                .strip_prefix("branch_direction_")
                .is_some_and(|ordinal| {
                    ordinal.len() == 4 && ordinal.bytes().all(|byte| byte.is_ascii_digit())
                }),
            _ => false,
        };
        for (field, value) in &mut node.fields {
            if !accepts_field(field.0.as_str()) {
                continue;
            }
            let crate::IntentLiteral::Point(direction) = value else {
                continue;
            };
            let length = direction[0].hypot(direction[1]);
            *direction = if length <= f64::EPSILON {
                [1.0, 0.0]
            } else {
                [direction[0] / length, direction[1] / length]
            };
        }
    }
}

fn validate_historical_contact_wire_authority(
    wire: &IntentSessionWire,
) -> Result<(), IntentSessionError> {
    if wire.undo.len() > MAX_INTENT_HISTORY_ENTRIES || wire.redo.len() > MAX_INTENT_HISTORY_ENTRIES
    {
        return Err(IntentSessionError::HistoryLimit);
    }
    if wire.undo.iter().chain(&wire.redo).any(|entry| {
        entry.descriptor.target_revision.raw() == 0
            || entry.descriptor.target_revision > wire.revision
    }) || !wire
        .undo
        .windows(2)
        .all(|pair| pair[0].descriptor.target_revision < pair[1].descriptor.target_revision)
        || !wire
            .redo
            .windows(2)
            .all(|pair| pair[0].descriptor.target_revision > pair[1].descriptor.target_revision)
        || wire
            .undo
            .last()
            .zip(wire.redo.last())
            .is_some_and(|(undo, redo)| {
                undo.descriptor.target_revision >= redo.descriptor.target_revision
            })
    {
        return Err(IntentSessionError::InvalidHistoryDescriptor);
    }
    wire.reservations
        .validate_against_graph(&wire.current.graph)?;
    validate_historical_contact_checkpoint_authority(&wire.current, &wire.reservations, true)?;
    for entry in wire.undo.iter().chain(&wire.redo) {
        entry.descriptor.validate()?;
        validate_historical_contact_checkpoint_authority(
            &entry.checkpoint,
            &wire.reservations,
            false,
        )?;
    }
    validate_history_edges(&wire.current, &wire.undo, &wire.redo)?;
    validate_history_descriptors(&wire.current, &wire.undo, &wire.redo)?;
    validate_allocator(
        wire.allocator,
        std::iter::once(&wire.current)
            .chain(wire.undo.iter().map(|entry| &entry.checkpoint))
            .chain(wire.redo.iter().map(|entry| &entry.checkpoint)),
        &wire.reservations,
    )
}

fn validate_historical_contact_checkpoint_authority(
    checkpoint: &SessionCheckpoint,
    reservations: &IntentReservationLedger,
    current: bool,
) -> Result<(), IntentSessionError> {
    checkpoint.external_inputs.validate()?;
    for (leaf, value) in checkpoint.instance.values() {
        checkpoint.graph.writable_leaf(*leaf)?;
        if !literal_matches_leaf(value, leaf.field) {
            return Err(IntentSessionError::InvalidInstance);
        }
        value.validate()?;
    }
    validate_checkpoint_reservation_authority(checkpoint, reservations, current)?;
    validate_organization(&checkpoint.graph, &checkpoint.organization)?;
    let semantic = semantic_identity(
        &checkpoint.graph,
        &checkpoint.instance,
        checkpoint.reservation_identity,
        &checkpoint.external_inputs,
    );
    if checkpoint.latest_attempt.is_none()
        && (checkpoint.accepted.is_some()
            || !checkpoint.graph.nodes().is_empty()
            || !checkpoint.instance.values().is_empty()
            || checkpoint.external_inputs != IntentExternalInputs::default())
    {
        return Err(IntentSessionError::InvalidAuthority);
    }
    if let Some(attempt) = &checkpoint.latest_attempt {
        if attempt.target != semantic {
            return Err(IntentSessionError::InvalidAuthority);
        }
        match attempt.disposition {
            IntentAttemptDisposition::Accepted => {
                let accepted = checkpoint
                    .accepted
                    .as_ref()
                    .ok_or(IntentSessionError::InvalidAuthority)?;
                if accepted.target != semantic
                    || attempt.materialization_digest != Some(accepted.evidence.digest)
                    || !attempt.failed_nodes.is_empty()
                    || attempt.diagnostic.is_some()
                {
                    return Err(IntentSessionError::InvalidAuthority);
                }
            }
            IntentAttemptDisposition::RetainedFailed => {
                if attempt.failed_nodes.is_empty()
                    || attempt.diagnostic.is_none()
                    || attempt.materialization_digest.is_some()
                    || checkpoint
                        .accepted
                        .as_ref()
                        .is_some_and(|accepted| accepted.target == semantic)
                    || attempt
                        .failed_nodes
                        .iter()
                        .any(|node| !checkpoint.graph.nodes().contains_key(node))
                {
                    return Err(IntentSessionError::InvalidAuthority);
                }
            }
            IntentAttemptDisposition::OrganizationOnly => {
                return Err(IntentSessionError::InvalidAuthority);
            }
        }
    }
    if let Some(accepted) = &checkpoint.accepted {
        accepted.external_inputs.validate()?;
        for (leaf, value) in accepted.instance.values() {
            accepted.graph.writable_leaf(*leaf)?;
            if !literal_matches_leaf(value, leaf.field) {
                return Err(IntentSessionError::InvalidInstance);
            }
            value.validate()?;
        }
        accepted
            .reservations
            .validate_against_graph(&accepted.graph)?;
        accepted.evidence.validate()?;
        if accepted.graph.identity() != accepted.target.graph
            || accepted.instance.identity() != accepted.target.instance
            || accepted.reservations.identity() != accepted.target.reservations
            || accepted.external_inputs.identity() != accepted.target.external_inputs
            || accepted.external_inputs.identity() != accepted.evidence.external_inputs
            || accepted.target.external_inputs != accepted.evidence.external_inputs
        {
            return Err(IntentSessionError::InvalidAuthority);
        }
        reservations.validate_superset_of(&accepted.reservations)?;
    }
    Ok(())
}

fn migrate_historical_contact_graph(
    graph: &mut IntentGraph,
    migrated_fields: &mut usize,
) -> Result<(), IntentSessionError> {
    for node in graph.nodes.values_mut() {
        for prefix in historical_contact_prefixes(&node.kind) {
            *migrated_fields += migrate_historical_contact_fields(node, prefix)?;
        }
    }
    Ok(())
}

fn historical_contact_prefixes(kind: &crate::IntentNodeKind) -> &'static [&'static str] {
    use crate::ConstraintKind as C;

    match kind {
        crate::IntentNodeKind::Geometry {
            recipe: crate::GeometryRecipeKind::TangentArc,
        } => &["source"],
        crate::IntentNodeKind::Constraint {
            constraint: C::PointOnCurve | C::LineCurveTangency | C::CurveDirection,
        } => &["contact"],
        crate::IntentNodeKind::Constraint {
            constraint:
                C::LineCircleTangency
                | C::CircleArcTangency
                | C::CurveCurveContact
                | C::CurveCurveTangency
                | C::EqualCurvature
                | C::EndpointContinuity
                | C::LineLineFillet
                | C::CurveCurveFillet,
        } => &["first_contact", "second_contact"],
        _ => &[],
    }
}

fn historical_field_key(name: &str) -> crate::IntentFieldKey {
    crate::IntentFieldKey(
        IntentKey::new(name).expect("internal historical contact field keys are valid"),
    )
}

fn migrate_historical_contact_fields(
    node: &mut crate::IntentNode,
    prefix: &str,
) -> Result<usize, IntentSessionError> {
    let domain_key = historical_field_key(&format!("{prefix}_domain"));
    let lower_key = historical_field_key(&format!("{prefix}_domain_lower"));
    let upper_key = historical_field_key(&format!("{prefix}_domain_upper"));
    let period_key = historical_field_key(&format!("{prefix}_domain_period"));
    let support_key = historical_field_key(&format!("{prefix}_support"));
    let range_lower_key = historical_field_key(&format!("{prefix}_range_lower"));
    let range_upper_key = historical_field_key(&format!("{prefix}_range_upper"));
    let carries_legacy = [&domain_key, &lower_key, &upper_key, &period_key]
        .into_iter()
        .any(|field| node.fields.contains_key(field));
    if !carries_legacy {
        return Ok(0);
    }
    if node.fields.contains_key(&support_key)
        || node.fields.contains_key(&range_lower_key)
        || node.fields.contains_key(&range_upper_key)
    {
        return Err(historical_contact_rejection(
            node.id,
            "legacy and current contact topology fields are mixed",
        ));
    }

    let domain = node.fields.remove(&domain_key).ok_or_else(|| {
        historical_contact_rejection(node.id, "legacy contact auxiliaries have no domain kind")
    })?;
    let lower = node.fields.remove(&lower_key);
    let upper = node.fields.remove(&upper_key);
    let period = node.fields.remove(&period_key);
    let removed = 1
        + usize::from(lower.is_some())
        + usize::from(upper.is_some())
        + usize::from(period.is_some());
    let crate::IntentLiteral::Enum(domain) = domain else {
        return Err(historical_contact_rejection(
            node.id,
            "legacy contact domain kind is not an enum",
        ));
    };
    let lower = historical_dimensionless_quantity(node.id, lower.as_ref(), "lower bound")?;
    let upper = historical_dimensionless_quantity(node.id, upper.as_ref(), "upper bound")?;
    let period = historical_dimensionless_quantity(node.id, period.as_ref(), "period")?;

    match domain.as_str() {
        "bounded"
            if lower.unwrap_or(0.0).to_bits() == 0.0_f64.to_bits()
                && upper.unwrap_or(1.0).to_bits() == 1.0_f64.to_bits()
                && period.is_none() => {}
        "periodic"
            if lower.is_none()
                && upper.is_none()
                && period.unwrap_or(std::f64::consts::TAU).to_bits()
                    == std::f64::consts::TAU.to_bits() => {}
        "supporting_line" if lower.is_none() && upper.is_none() && period.is_none() => {
            node.fields.insert(
                support_key,
                crate::IntentLiteral::Enum(
                    IntentKey::new("supporting_line")
                        .expect("internal supporting-line enum is valid"),
                ),
            );
        }
        "bounded" => {
            return Err(historical_contact_rejection(
                node.id,
                "only the exact legacy bounded [0, 1] topology default is migratable",
            ));
        }
        "periodic" => {
            return Err(historical_contact_rejection(
                node.id,
                "only the exact legacy tau-period topology default is migratable",
            ));
        }
        "supporting_line" => {
            return Err(historical_contact_rejection(
                node.id,
                "legacy supporting-line contact carries incompatible bounds or period",
            ));
        }
        _ => {
            return Err(historical_contact_rejection(
                node.id,
                "legacy contact domain kind is unknown",
            ));
        }
    }
    Ok(removed)
}

fn historical_dimensionless_quantity(
    node: NodeId,
    literal: Option<&crate::IntentLiteral>,
    component: &'static str,
) -> Result<Option<f64>, IntentSessionError> {
    match literal {
        None => Ok(None),
        Some(crate::IntentLiteral::Quantity {
            value,
            unit: crate::IntentUnit::Dimensionless,
        }) if value.is_finite() => Ok(Some(*value)),
        Some(_) => Err(historical_contact_rejection(
            node,
            match component {
                "lower bound" => {
                    "legacy contact lower bound is not a finite dimensionless quantity"
                }
                "upper bound" => {
                    "legacy contact upper bound is not a finite dimensionless quantity"
                }
                "period" => "legacy contact period is not a finite dimensionless quantity",
                _ => "legacy contact component is invalid",
            },
        )),
    }
}

const fn historical_contact_rejection(node: NodeId, reason: &'static str) -> IntentSessionError {
    IntentSessionError::HistoricalContactDomainMigrationRejected { node, reason }
}

fn legacy_graph_identity(graph: &IntentGraph) -> crate::IntentGraphIdentity {
    crate::IntentGraphIdentity(crate::ComponentIdentity {
        revision: graph.revision,
        digest: legacy_digest_bytes(
            &serde_json::to_vec(&(graph.revision, &graph.nodes))
                .expect("legacy graph identity is infallibly serializable"),
        ),
    })
}

fn legacy_instance_identity(instance: &IntentInstanceState) -> crate::IntentInstanceIdentity {
    crate::IntentInstanceIdentity(crate::ComponentIdentity {
        revision: instance.revision,
        digest: legacy_digest_bytes(
            &serde_json::to_vec(&(&instance.revision, &instance.values))
                .expect("legacy instance identity is infallibly serializable"),
        ),
    })
}

fn legacy_external_identity(inputs: &IntentExternalInputs) -> IntentExternalInputsIdentity {
    IntentExternalInputsIdentity {
        revision: inputs.revision,
        digest: legacy_digest_bytes(
            &serde_json::to_vec(&(
                inputs.revision,
                &inputs.parameter_batch,
                &inputs.external_snapshots,
            ))
            .expect("legacy external-input identity is infallibly serializable"),
        ),
    }
}

fn legacy_semantic_identity(
    graph: &IntentGraph,
    instance: &IntentInstanceState,
    reservations: IntentReservationLedgerIdentity,
    external_inputs: &IntentExternalInputs,
) -> IntentSemanticIdentity {
    IntentSemanticIdentity {
        graph: legacy_graph_identity(graph),
        instance: legacy_instance_identity(instance),
        reservations,
        external_inputs: legacy_external_identity(external_inputs),
    }
}

fn legacy_materialization_evidence_digest(evidence: &MaterializationEvidence) -> ContentDigest {
    legacy_digest_bytes(
        &serde_json::to_vec(&(
            evidence.external_inputs,
            &evidence.materialization,
            &evidence.ownership,
            &evidence.host_validation,
        ))
        .expect("legacy materialization evidence is infallibly serializable"),
    )
}

fn migrated_legacy_checkpoint_reservation_identity(
    checkpoint: &SessionCheckpoint,
    current_reservations: &IntentReservationLedger,
    current: bool,
) -> Option<(IntentReservationLedgerIdentity, ReservationId)> {
    if current {
        return (checkpoint.reservation_identity == current_reservations.legacy_identity()).then(
            || {
                (
                    current_reservations.identity(),
                    current_reservations
                        .next_reservation_high_water()
                        .expect("validated reservation ledger has finite high-water"),
                )
            },
        );
    }
    legacy_historical_checkpoint_reservation_ledger_candidates(checkpoint, current_reservations)
        .into_iter()
        .find(|candidate| checkpoint.reservation_identity == candidate.legacy_identity())
        .map(|candidate| {
            (
                candidate.identity(),
                candidate
                    .next_reservation_high_water()
                    .expect("validated reservation ledger has finite high-water"),
            )
        })
}

/// Reconstructs the sole ledger prefix authenticated by a wire-v2 checkpoint.
/// The high-water is an exact witness, not a hint: graph reconciliation may
/// change only declaration disposition and may not manufacture a reservation
/// outside the retained monotonic prefix.
fn checkpoint_reservation_ledger_from_high_water(
    checkpoint: &SessionCheckpoint,
    current_reservations: &IntentReservationLedger,
) -> Option<IntentReservationLedger> {
    let high_water = checkpoint.reservation_high_water?;
    let mut candidate = current_reservations.prefix_before(high_water);
    if candidate.next_reservation_high_water() != Some(high_water)
        || candidate
            .validate_graph_metadata(&checkpoint.graph)
            .is_err()
        || candidate.reconcile(&checkpoint.graph).is_err()
    {
        return None;
    }
    candidate.revision = checkpoint.reservation_identity.0.revision;
    (candidate.next_reservation_high_water() == Some(high_water)
        && candidate.validate_against_graph(&checkpoint.graph).is_ok())
    .then_some(candidate)
}

/// Experimental wire-v1 did not retain a reservation high-water. Its
/// independently authenticated checkpoints therefore use the original
/// bounded reconstruction candidates only during one-way legacy migration.
fn legacy_historical_checkpoint_reservation_ledger_candidates(
    checkpoint: &SessionCheckpoint,
    current_reservations: &IntentReservationLedger,
) -> Vec<IntentReservationLedger> {
    let mut bases = vec![
        IntentReservationLedger::empty(),
        current_reservations.clone(),
    ];
    if let Some(accepted) = &checkpoint.accepted {
        bases.push(accepted.reservations.clone());
    }
    let mut candidates = Vec::new();
    for mut candidate in bases {
        if candidate.validate_against_graph(&checkpoint.graph).is_ok() {
            candidates.push(candidate.clone());
        }
        if candidate.reconcile(&checkpoint.graph).is_ok() {
            candidate.revision = checkpoint.reservation_identity.0.revision;
            candidates.push(candidate);
        }
    }
    candidates
}

fn historical_checkpoint_reservation_identity_valid(
    checkpoint: &SessionCheckpoint,
    current_reservations: &IntentReservationLedger,
) -> bool {
    checkpoint_reservation_ledger_from_high_water(checkpoint, current_reservations)
        .is_some_and(|candidate| candidate.identity() == checkpoint.reservation_identity)
}

fn validate_legacy_wire_authority(wire: &IntentSessionWire) -> Result<(), IntentSessionError> {
    validate_legacy_checkpoint_authority(&wire.current, &wire.reservations, true)?;
    for entry in wire.undo.iter().chain(&wire.redo) {
        validate_legacy_checkpoint_authority(&entry.checkpoint, &wire.reservations, false)?;
    }
    Ok(())
}

fn validate_legacy_checkpoint_authority(
    checkpoint: &SessionCheckpoint,
    current_reservations: &IntentReservationLedger,
    current: bool,
) -> Result<(), IntentSessionError> {
    if checkpoint.reservation_high_water.is_some()
        || migrated_legacy_checkpoint_reservation_identity(
            checkpoint,
            current_reservations,
            current,
        )
        .is_none()
    {
        return Err(IntentSessionError::InvalidAuthority);
    }
    let semantic = legacy_semantic_identity(
        &checkpoint.graph,
        &checkpoint.instance,
        checkpoint.reservation_identity,
        &checkpoint.external_inputs,
    );
    if checkpoint.latest_attempt.is_none()
        && (checkpoint.accepted.is_some()
            || !checkpoint.graph.nodes().is_empty()
            || !checkpoint.instance.values().is_empty()
            || checkpoint.external_inputs != IntentExternalInputs::default())
    {
        return Err(IntentSessionError::InvalidAuthority);
    }
    if let Some(accepted) = &checkpoint.accepted {
        let accepted_semantic = legacy_semantic_identity(
            &accepted.graph,
            &accepted.instance,
            accepted.reservations.legacy_identity(),
            &accepted.external_inputs,
        );
        if accepted.target != accepted_semantic
            || accepted.evidence.external_inputs
                != legacy_external_identity(&accepted.external_inputs)
            || accepted.evidence.digest
                != legacy_materialization_evidence_digest(&accepted.evidence)
        {
            return Err(IntentSessionError::InvalidAuthority);
        }
    }
    if let Some(attempt) = &checkpoint.latest_attempt {
        if attempt.target != semantic {
            return Err(IntentSessionError::InvalidAuthority);
        }
        match attempt.disposition {
            IntentAttemptDisposition::Accepted => {
                let accepted = checkpoint
                    .accepted
                    .as_ref()
                    .ok_or(IntentSessionError::InvalidAuthority)?;
                if accepted.target != semantic
                    || attempt.materialization_digest != Some(accepted.evidence.digest)
                    || !attempt.failed_nodes.is_empty()
                    || attempt.diagnostic.is_some()
                {
                    return Err(IntentSessionError::InvalidAuthority);
                }
            }
            IntentAttemptDisposition::RetainedFailed => {
                if attempt.failed_nodes.is_empty()
                    || attempt.diagnostic.is_none()
                    || attempt.materialization_digest.is_some()
                    || checkpoint
                        .accepted
                        .as_ref()
                        .is_some_and(|accepted| accepted.target == semantic)
                    || attempt
                        .failed_nodes
                        .iter()
                        .any(|node| !checkpoint.graph.nodes().contains_key(node))
                {
                    return Err(IntentSessionError::InvalidAuthority);
                }
            }
            IntentAttemptDisposition::OrganizationOnly => {
                return Err(IntentSessionError::InvalidAuthority);
            }
        }
    }
    Ok(())
}

fn migrate_legacy_wire_identities(wire: &mut IntentSessionWire) -> Result<(), IntentSessionError> {
    let current_reservations = wire.reservations.clone();
    migrate_legacy_checkpoint(&mut wire.current, &current_reservations, true);
    for entry in wire.undo.iter_mut().chain(&mut wire.redo) {
        migrate_legacy_checkpoint(&mut entry.checkpoint, &current_reservations, false);
    }
    bind_history_edges(&wire.current, &mut wire.undo, &mut wire.redo);
    let undo_kinds = wire
        .undo
        .iter()
        .enumerate()
        .map(|(index, entry)| {
            let after = wire
                .undo
                .get(index + 1)
                .map_or(&wire.current, |next| &next.checkpoint);
            let created = after
                .graph
                .nodes()
                .keys()
                .filter(|node| !entry.checkpoint.graph.nodes().contains_key(node))
                .copied()
                .collect::<BTreeSet<_>>();
            let deleted = entry
                .checkpoint
                .graph
                .nodes()
                .keys()
                .filter(|node| !after.graph.nodes().contains_key(node))
                .copied()
                .collect::<BTreeSet<_>>();
            history_operation_kinds(&entry.checkpoint, after, &created, &deleted)
                .ok_or(IntentSessionError::InvalidHistoryDescriptor)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let redo_kinds = wire
        .redo
        .iter()
        .enumerate()
        .map(|(index, entry)| {
            let before = wire
                .redo
                .get(index + 1)
                .map_or(&wire.current, |next| &next.checkpoint);
            let created = entry
                .checkpoint
                .graph
                .nodes()
                .keys()
                .filter(|node| !before.graph.nodes().contains_key(node))
                .copied()
                .collect::<BTreeSet<_>>();
            let deleted = before
                .graph
                .nodes()
                .keys()
                .filter(|node| !entry.checkpoint.graph.nodes().contains_key(node))
                .copied()
                .collect::<BTreeSet<_>>();
            history_operation_kinds(before, &entry.checkpoint, &created, &deleted)
                .ok_or(IntentSessionError::InvalidHistoryDescriptor)
        })
        .collect::<Result<Vec<_>, _>>()?;
    for (entry, kinds) in wire.undo.iter_mut().zip(undo_kinds) {
        entry.descriptor.operation_kinds = kinds;
    }
    for (entry, kinds) in wire.redo.iter_mut().zip(redo_kinds) {
        entry.descriptor.operation_kinds = kinds;
    }
    wire.version = INTENT_SESSION_VERSION;
    Ok(())
}

fn migrate_legacy_checkpoint(
    checkpoint: &mut SessionCheckpoint,
    current_reservations: &IntentReservationLedger,
    current: bool,
) {
    let (reservation_identity, reservation_high_water) =
        migrated_legacy_checkpoint_reservation_identity(checkpoint, current_reservations, current)
            .expect("legacy checkpoint authority was validated before migration");
    checkpoint.reservation_identity = reservation_identity;
    checkpoint.reservation_high_water = Some(reservation_high_water);
    if let Some(accepted) = &mut checkpoint.accepted {
        accepted.evidence.external_inputs = accepted.external_inputs.identity();
        accepted.evidence.reauthenticate();
        accepted.target = semantic_identity(
            &accepted.graph,
            &accepted.instance,
            accepted.reservations.identity(),
            &accepted.external_inputs,
        );
        if accepted.graph == checkpoint.graph
            && accepted.instance == checkpoint.instance
            && accepted.external_inputs == checkpoint.external_inputs
        {
            checkpoint.reservation_identity = accepted.reservations.identity();
            checkpoint.reservation_high_water = Some(
                accepted
                    .reservations
                    .next_reservation_high_water()
                    .expect("validated accepted ledger has finite high-water"),
            );
        }
    }
    let current = semantic_identity(
        &checkpoint.graph,
        &checkpoint.instance,
        checkpoint.reservation_identity,
        &checkpoint.external_inputs,
    );
    if let Some(attempt) = &mut checkpoint.latest_attempt {
        attempt.target = current;
        attempt.materialization_digest = match attempt.disposition {
            IntentAttemptDisposition::Accepted => checkpoint
                .accepted
                .as_ref()
                .map(|accepted| accepted.evidence.digest),
            IntentAttemptDisposition::RetainedFailed
            | IntentAttemptDisposition::OrganizationOnly => None,
        };
    }
}

fn semantic_identity(
    graph: &IntentGraph,
    instance: &IntentInstanceState,
    reservations: IntentReservationLedgerIdentity,
    external_inputs: &IntentExternalInputs,
) -> IntentSemanticIdentity {
    IntentSemanticIdentity {
        graph: graph.identity(),
        instance: instance.identity(),
        reservations,
        external_inputs: external_inputs.identity(),
    }
}

#[allow(clippy::too_many_arguments)]
fn session_identity(
    session: IntentSessionId,
    revision: Revision,
    graph: &IntentGraph,
    instance: &IntentInstanceState,
    reservations: &IntentReservationLedger,
    organization: &IntentOrganization,
    external_inputs: &IntentExternalInputs,
    latest_attempt: Option<&IntentLatestAttempt>,
    accepted: Option<&IntentAcceptedAuthority>,
    undo: &[HistoryEntry],
    redo: &[HistoryEntry],
    allocator: IntentAllocatorHighWater,
) -> IntentSessionIdentity {
    #[cfg(test)]
    SESSION_IDENTITY_HASH_COUNT.with(|count| count.set(count.get() + 1));
    let graph_identity = graph.identity();
    let instance_identity = instance.identity();
    let reservation_identity = reservations.identity();
    let organization_identity = organization.identity();
    let external_identity = external_inputs.identity();
    let digest = digest_bytes(
        &serde_json::to_vec(&(
            session,
            revision,
            graph_identity,
            instance_identity,
            reservation_identity,
            organization_identity,
            external_identity,
            latest_attempt,
            accepted,
            undo,
            redo,
            allocator,
        ))
        .expect("session identity payload is infallibly serializable"),
    );
    IntentSessionIdentity {
        session,
        revision,
        digest,
        graph: graph_identity,
        instance: instance_identity,
        reservations: reservation_identity,
        organization: organization_identity,
        external_inputs: external_identity,
    }
}

#[allow(clippy::too_many_arguments)]
fn plan_token(
    base: IntentSessionIdentity,
    target: IntentSessionIdentity,
    disposition: IntentPlanDisposition,
    aliases: &IntentAliasMap,
    diff: &IntentSemanticDiff,
    descriptor: &IntentTransactionDescriptor,
    staged: &SessionCheckpoint,
    reservations: &IntentReservationLedger,
    undo: &[HistoryEntry],
    redo: &[HistoryEntry],
    allocator: IntentAllocatorHighWater,
) -> PlanToken {
    PlanToken::new(digest_bytes(
        &serde_json::to_vec(&(
            base,
            target,
            disposition,
            aliases,
            diff,
            descriptor,
            staged,
            reservations,
            undo,
            redo,
            allocator,
        ))
        .expect("plan token payload is infallibly serializable"),
    ))
}

fn validate_failure(
    graph: &IntentGraph,
    failure: &IntentEvaluationFailure,
) -> Result<(), IntentPlanError> {
    if failure.failed_nodes.is_empty() {
        return Err(IntentPlanError::InvalidEvaluationFailure);
    }
    if failure
        .failed_nodes
        .iter()
        .any(|node| !graph.nodes().contains_key(node))
    {
        return Err(IntentPlanError::InvalidEvaluationFailure);
    }
    Ok(())
}

fn resolve_patch_port(
    source: PatchPortRef,
    aliases: &IntentAliasMap,
) -> Result<crate::IntentPortRef, IntentPlanError> {
    match source {
        PatchPortRef::Stable { port } => Ok(port),
        PatchPortRef::Alias { node, selector } => aliases
            .port(&node, selector)
            .ok_or(IntentPlanError::UnknownAlias(node)),
    }
}

fn resolve_cell(target: CellTarget, aliases: &IntentAliasMap) -> Result<CellId, IntentPlanError> {
    match target {
        CellTarget::Stable { cell } => Ok(cell),
        CellTarget::Alias { alias } => aliases
            .cell(&alias)
            .ok_or(IntentPlanError::UnknownAlias(alias)),
    }
}

#[allow(clippy::too_many_lines)]
fn apply_patch_operations(
    operations: Vec<IntentPatchOperation>,
    staged: &mut SessionCheckpoint,
    allocator: &mut IntentAllocatorHighWater,
    aliases: &mut IntentAliasMap,
    diff: &mut IntentSemanticDiff,
) -> Result<(), IntentPlanError> {
    let mut create_cells = BTreeMap::<IntentKey, (IntentKey, Option<CellId>)>::new();
    let mut create_nodes = BTreeMap::<IntentKey, (IntentNodeDraftBox, Option<CellTarget>)>::new();
    for operation in &operations {
        match operation {
            IntentPatchOperation::CreateCell {
                alias,
                name,
                before,
            } => {
                create_cells.insert(alias.clone(), (name.clone(), *before));
            }
            IntentPatchOperation::CreateNode { alias, draft, cell } => {
                create_nodes.insert(
                    alias.clone(),
                    (IntentNodeDraftBox((**draft).clone()), cell.clone()),
                );
            }
            _ => {}
        }
    }

    // Alias-sorted allocation is independent of patch-array order.
    for (alias, (name, before)) in create_cells {
        let id = allocator.allocate_cell()?;
        if let Some(before) = before {
            let index = staged
                .organization
                .cell_order
                .iter()
                .position(|candidate| *candidate == before)
                .ok_or(IntentPlanError::UnknownCell(before))?;
            staged.organization.cell_order.insert(index, id);
        } else {
            staged.organization.cell_order.push(id);
        }
        staged.organization.cells.insert(
            id,
            OrganizationCell {
                id,
                name,
                declarations: Vec::new(),
            },
        );
        aliases.cells.insert(alias, id);
        diff.organization_changed = true;
    }

    let known_aliases = create_nodes.keys().cloned().collect::<BTreeSet<_>>();
    for (draft, _) in create_nodes.values() {
        for source in draft.0.inputs.values() {
            if let PatchPortRef::Alias { node, .. } = source
                && !known_aliases.contains(node)
            {
                return Err(IntentPlanError::UnknownAlias(node.clone()));
            }
        }
    }

    // Allocate and finish in dependency order so persistent native
    // reservations are available before a dependent declaration is lowered.
    // Symbol and alias are only deterministic ready-set tie-breaks; patch-array
    // and presentation order remain non-semantic.
    while !create_nodes.is_empty() {
        let alias = create_nodes
            .iter()
            .filter(|(_, (draft, _))| {
                draft.0.inputs.values().all(|source| match source {
                    PatchPortRef::Stable { .. } => true,
                    PatchPortRef::Alias { node, .. } => aliases.nodes.contains_key(node),
                })
            })
            .min_by(|(left_alias, (left, _)), (right_alias, (right, _))| {
                left.0
                    .symbol
                    .cmp(&right.0.symbol)
                    .then_with(|| left_alias.cmp(right_alias))
            })
            .map(|(alias, _)| alias.clone())
            .ok_or(IntentPlanError::Graph(IntentGraphError::DependencyCycle {
                node: allocator.next_node,
            }))?;
        let (draft, cell) = create_nodes
            .remove(&alias)
            .expect("ready alias remains pending");
        let value = allocate_draft(draft.0, allocator)?;
        aliases.nodes.insert(alias.clone(), value.id);
        for (selector, port) in allocated_alias_ports(&value) {
            aliases
                .ports
                .entry(alias.clone())
                .or_default()
                .insert(selector, port);
        }
        let inputs = value
            .draft
            .inputs
            .clone()
            .into_iter()
            .map(|(slot, source)| Ok((slot, resolve_patch_port(source, aliases)?)))
            .collect::<Result<BTreeMap<_, _>, IntentPlanError>>()?;
        let id = value.id;
        let name = value.draft.name.clone();
        let had_instance = !value.draft.initial_instance.is_empty();
        let node = finish_allocated_draft(value, inputs, &mut staged.instance)?;
        staged.graph.nodes.insert(id, node);
        let cell = cell
            .map(|target| resolve_cell(target, aliases))
            .transpose()?
            .unwrap_or(staged.organization.default_cell);
        let cell_value = staged
            .organization
            .cells
            .get_mut(&cell)
            .ok_or(IntentPlanError::UnknownCell(cell))?;
        cell_value.declarations.push(id);
        staged.organization.node_names.insert(id, name);
        diff.created_nodes.insert(id);
        diff.definition_nodes.insert(id);
        diff.graph_changed = true;
        diff.organization_changed = true;
        if had_instance {
            diff.instance_nodes.insert(id);
            diff.instance_changed = true;
        }
    }

    // Creation was resolved above in dependency order. Apply every retained
    // mutation before evaluating deletion closures so one unordered atomic
    // patch may rebind a surviving dependent away from a source that the same
    // patch then removes. Deletion policy still authenticates the exact
    // post-rebind closure; patch-array order remains non-semantic.
    let (deletions, mutations): (Vec<_>, Vec<_>) = operations
        .into_iter()
        .partition(|operation| matches!(operation, IntentPatchOperation::DeleteNode { .. }));
    for operation in mutations.into_iter().chain(deletions) {
        match operation {
            IntentPatchOperation::CreateNode { .. } | IntentPatchOperation::CreateCell { .. } => {}
            IntentPatchOperation::SetSuppressed { node, suppressed } => {
                if staged.graph.set_suppressed(node, suppressed)? {
                    diff.definition_nodes.insert(node);
                    diff.graph_changed = true;
                }
            }
            IntentPatchOperation::SetDefinitionField { node, field, value } => {
                if staged.graph.set_field(node, field, value)? {
                    diff.definition_nodes.insert(node);
                    diff.graph_changed = true;
                }
            }
            IntentPatchOperation::UnsetDefinitionField { node, field } => {
                if staged.graph.unset_field(node, field)? {
                    diff.definition_nodes.insert(node);
                    diff.graph_changed = true;
                }
            }
            IntentPatchOperation::SetInstanceLeaf { leaf, value } => {
                staged.graph.writable_leaf(leaf)?;
                if !literal_matches_leaf(&value, leaf.field) {
                    return Err(IntentPlanError::InvalidInstanceLiteral { leaf });
                }
                value.validate()?;
                if staged.instance.values.get(&leaf) != Some(&value) {
                    staged.instance.values.insert(leaf, value);
                    diff.instance_nodes.insert(leaf.node);
                    diff.instance_changed = true;
                }
            }
            IntentPatchOperation::RebindInput { node, slot, source } => {
                let source = resolve_patch_port(source, aliases)?;
                if staged.graph.rebind_input(node, slot, source)? {
                    diff.definition_nodes.insert(node);
                    diff.graph_changed = true;
                }
            }
            IntentPatchOperation::EjectBootstrapPoint { node } => {
                staged.graph.eject_bootstrap_point(node)?;
                diff.definition_nodes.insert(node);
                diff.graph_changed = true;
            }
            IntentPatchOperation::DeleteNode { node, policy } => {
                let roots = match &policy {
                    DeletePolicy::RejectDependents | DeletePolicy::Cascade { .. } => {
                        BTreeSet::from([node])
                    }
                    DeletePolicy::CascadeRoots { exact_roots, .. } => {
                        if !exact_roots.contains(&node) {
                            return Err(IntentPlanError::CascadeRootMissing { node });
                        }
                        exact_roots.clone()
                    }
                };
                let closure = staged.graph.dependent_closure(roots)?;
                match policy {
                    DeletePolicy::RejectDependents if closure.len() != 1 => {
                        return Err(IntentPlanError::DeleteHasDependents { node, closure });
                    }
                    DeletePolicy::Cascade { exact_nodes }
                    | DeletePolicy::CascadeRoots { exact_nodes, .. }
                        if exact_nodes != closure =>
                    {
                        return Err(IntentPlanError::CascadeMismatch {
                            expected: closure,
                            actual: exact_nodes,
                        });
                    }
                    DeletePolicy::RejectDependents
                    | DeletePolicy::Cascade { .. }
                    | DeletePolicy::CascadeRoots { .. } => {}
                }
                staged.graph.remove_nodes(&closure, &mut staged.instance);
                for cell in staged.organization.cells.values_mut() {
                    cell.declarations.retain(|value| !closure.contains(value));
                }
                staged
                    .organization
                    .node_names
                    .retain(|node, _| !closure.contains(node));
                diff.absorb_deleted(&closure);
            }
            IntentPatchOperation::RenameNode { node, name } => {
                let current = staged
                    .organization
                    .node_names
                    .get_mut(&node)
                    .ok_or(IntentPlanError::UnknownNode(node))?;
                if *current != name {
                    *current = name;
                    diff.organization_nodes.insert(node);
                    diff.organization_changed = true;
                }
            }
            IntentPatchOperation::MoveDeclaration { node, cell, before } => {
                if !staged.graph.nodes.contains_key(&node) {
                    return Err(IntentPlanError::UnknownNode(node));
                }
                let target = resolve_cell(cell, aliases)?;
                if !staged.organization.cells.contains_key(&target) {
                    return Err(IntentPlanError::UnknownCell(target));
                }
                let old = declaration_location(&staged.organization, node)
                    .ok_or(IntentPlanError::UnknownNode(node))?;
                for cell in staged.organization.cells.values_mut() {
                    cell.declarations.retain(|value| *value != node);
                }
                let declarations = &mut staged
                    .organization
                    .cells
                    .get_mut(&target)
                    .unwrap()
                    .declarations;
                let index = before
                    .map(|before| {
                        declarations
                            .iter()
                            .position(|value| *value == before)
                            .ok_or(IntentPlanError::InvalidDeclarationOrder)
                    })
                    .transpose()?
                    .unwrap_or(declarations.len());
                declarations.insert(index, node);
                let new = declaration_location(&staged.organization, node).unwrap();
                if old != new {
                    diff.organization_nodes.insert(node);
                    diff.organization_changed = true;
                }
            }
            IntentPatchOperation::DeleteCell { cell } => {
                if cell == staged.organization.default_cell {
                    return Err(IntentPlanError::CannotDeleteDefaultCell);
                }
                let removed = staged
                    .organization
                    .cells
                    .remove(&cell)
                    .ok_or(IntentPlanError::UnknownCell(cell))?;
                staged
                    .organization
                    .cell_order
                    .retain(|value| *value != cell);
                staged
                    .organization
                    .cells
                    .get_mut(&staged.organization.default_cell)
                    .expect("default cell exists")
                    .declarations
                    .extend(removed.declarations.iter().copied());
                diff.organization_nodes.extend(removed.declarations);
                diff.organization_changed = true;
            }
            IntentPatchOperation::ReorderCells { exact_order } => {
                if exact_order.iter().copied().collect::<BTreeSet<_>>()
                    != staged.organization.cells.keys().copied().collect()
                {
                    return Err(IntentPlanError::InvalidCellOrder);
                }
                if staged.organization.cell_order != exact_order {
                    staged.organization.cell_order = exact_order;
                    diff.organization_changed = true;
                }
            }
            IntentPatchOperation::ReplaceExternalInputs { inputs } => {
                // Constructor validation is repeated for hostile decoded patches.
                let checked = IntentExternalInputs::new(
                    inputs.revision,
                    inputs.parameter_batch,
                    inputs.external_snapshots,
                )?;
                if checked != staged.external_inputs
                    && checked.revision <= staged.external_inputs.revision
                {
                    return Err(IntentPlanError::ExternalInputRevisionNotAdvanced);
                }
                if staged.external_inputs != checked {
                    staged.external_inputs = checked;
                    diff.external_inputs_changed = true;
                }
            }
        }
    }
    Ok(())
}

struct IntentNodeDraftBox(crate::IntentNodeDraft);

fn declaration_location(
    organization: &IntentOrganization,
    node: NodeId,
) -> Option<(CellId, usize)> {
    organization.cells.iter().find_map(|(cell, value)| {
        value
            .declarations
            .iter()
            .position(|candidate| *candidate == node)
            .map(|index| (*cell, index))
    })
}

fn validate_patch_conflicts(operations: &[IntentPatchOperation]) -> Result<(), IntentPlanError> {
    let mut targets = BTreeSet::<String>::new();
    let mut deletes = BTreeSet::<NodeId>::new();
    let mut touched = BTreeSet::<NodeId>::new();
    for operation in operations {
        let (target, node) = match operation {
            IntentPatchOperation::CreateNode { alias, .. } => {
                (format!("create-node:{alias}"), None)
            }
            IntentPatchOperation::DeleteNode { node, .. } => {
                deletes.insert(*node);
                (format!("delete-node:{node}"), Some(*node))
            }
            IntentPatchOperation::SetSuppressed { node, .. } => {
                (format!("suppressed:{node}"), Some(*node))
            }
            IntentPatchOperation::SetDefinitionField { node, field, .. }
            | IntentPatchOperation::UnsetDefinitionField { node, field } => {
                (format!("field:{node}:{}", field.0), Some(*node))
            }
            IntentPatchOperation::SetInstanceLeaf { leaf, .. } => {
                (format!("leaf:{leaf:?}"), Some(leaf.node))
            }
            IntentPatchOperation::RebindInput { node, slot, .. } => {
                (format!("input:{node}:{slot:?}"), Some(*node))
            }
            IntentPatchOperation::EjectBootstrapPoint { node } => {
                (format!("eject-bootstrap-point:{node}"), Some(*node))
            }
            IntentPatchOperation::RenameNode { node, .. } => (format!("name:{node}"), Some(*node)),
            IntentPatchOperation::MoveDeclaration { node, .. } => {
                (format!("move:{node}"), Some(*node))
            }
            IntentPatchOperation::CreateCell { alias, .. } => {
                (format!("create-cell:{alias}"), None)
            }
            IntentPatchOperation::DeleteCell { cell } => (format!("delete-cell:{cell}"), None),
            IntentPatchOperation::ReorderCells { .. } => ("reorder-cells".into(), None),
            IntentPatchOperation::ReplaceExternalInputs { .. } => {
                ("replace-external-inputs".into(), None)
            }
        };
        if !targets.insert(target.clone()) {
            return Err(IntentPlanError::ConflictingOperations { target });
        }
        if let Some(node) = node {
            touched.insert(node);
        }
    }
    if let Some(node) = deletes.into_iter().find(|node| {
        operations
            .iter()
            .filter(|operation| operation_targets_node(operation, *node))
            .count()
            > 1
    }) {
        return Err(IntentPlanError::DeleteEditConflict { node });
    }
    Ok(())
}

fn operation_targets_node(operation: &IntentPatchOperation, expected: NodeId) -> bool {
    match operation {
        IntentPatchOperation::DeleteNode { node, .. }
        | IntentPatchOperation::SetSuppressed { node, .. }
        | IntentPatchOperation::SetDefinitionField { node, .. }
        | IntentPatchOperation::UnsetDefinitionField { node, .. }
        | IntentPatchOperation::RebindInput { node, .. }
        | IntentPatchOperation::EjectBootstrapPoint { node }
        | IntentPatchOperation::RenameNode { node, .. }
        | IntentPatchOperation::MoveDeclaration { node, .. } => *node == expected,
        IntentPatchOperation::SetInstanceLeaf { leaf, .. } => leaf.node == expected,
        _ => false,
    }
}

fn validate_organization(
    graph: &IntentGraph,
    organization: &IntentOrganization,
) -> Result<(), IntentSessionError> {
    if !organization.cells.contains_key(&organization.default_cell)
        || organization.cell_order.len() != organization.cells.len()
        || organization
            .cell_order
            .iter()
            .copied()
            .collect::<BTreeSet<_>>()
            != organization.cells.keys().copied().collect()
    {
        return Err(IntentSessionError::InvalidOrganization);
    }
    let declarations = organization
        .cells
        .values()
        .flat_map(|cell| cell.declarations.iter().copied())
        .collect::<Vec<_>>();
    if declarations.iter().copied().collect::<BTreeSet<_>>().len() != declarations.len()
        || declarations.iter().copied().collect::<BTreeSet<_>>()
            != graph.nodes().keys().copied().collect()
        || organization
            .node_names
            .keys()
            .copied()
            .collect::<BTreeSet<_>>()
            != graph.nodes().keys().copied().collect()
    {
        return Err(IntentSessionError::InvalidOrganization);
    }
    for (id, cell) in &organization.cells {
        if *id != cell.id {
            return Err(IntentSessionError::InvalidOrganization);
        }
    }
    Ok(())
}

fn validate_checkpoint(
    checkpoint: &SessionCheckpoint,
    reservations: &IntentReservationLedger,
    current: bool,
) -> Result<(), IntentSessionError> {
    validate_semantic_state(
        &checkpoint.graph,
        &checkpoint.instance,
        &checkpoint.external_inputs,
    )?;
    validate_checkpoint_reservation_authority(checkpoint, reservations, current)?;
    validate_organization(&checkpoint.graph, &checkpoint.organization)?;
    let semantic = semantic_identity(
        &checkpoint.graph,
        &checkpoint.instance,
        checkpoint.reservation_identity,
        &checkpoint.external_inputs,
    );
    if checkpoint.latest_attempt.is_none()
        && (checkpoint.accepted.is_some()
            || !checkpoint.graph.nodes().is_empty()
            || !checkpoint.instance.values().is_empty()
            || checkpoint.external_inputs != IntentExternalInputs::default())
    {
        return Err(IntentSessionError::InvalidAuthority);
    }
    if let Some(attempt) = &checkpoint.latest_attempt {
        if attempt.target != semantic {
            return Err(IntentSessionError::InvalidAuthority);
        }
        match attempt.disposition {
            IntentAttemptDisposition::Accepted => {
                let accepted = checkpoint
                    .accepted
                    .as_ref()
                    .ok_or(IntentSessionError::InvalidAuthority)?;
                if accepted.target != semantic
                    || attempt.materialization_digest != Some(accepted.evidence.digest)
                    || !attempt.failed_nodes.is_empty()
                    || attempt.diagnostic.is_some()
                {
                    return Err(IntentSessionError::InvalidAuthority);
                }
            }
            IntentAttemptDisposition::RetainedFailed => {
                if attempt.failed_nodes.is_empty()
                    || attempt.diagnostic.is_none()
                    || attempt.materialization_digest.is_some()
                    || checkpoint
                        .accepted
                        .as_ref()
                        .is_some_and(|accepted| accepted.target == semantic)
                    || attempt
                        .failed_nodes
                        .iter()
                        .any(|node| !checkpoint.graph.nodes().contains_key(node))
                {
                    return Err(IntentSessionError::InvalidAuthority);
                }
            }
            IntentAttemptDisposition::OrganizationOnly => {
                return Err(IntentSessionError::InvalidAuthority);
            }
        }
    }
    if let Some(accepted) = &checkpoint.accepted {
        validate_semantic_state(
            &accepted.graph,
            &accepted.instance,
            &accepted.external_inputs,
        )?;
        accepted
            .reservations
            .validate_against_graph(&accepted.graph)?;
        accepted.evidence.validate()?;
        if accepted.graph.identity() != accepted.target.graph
            || accepted.instance.identity() != accepted.target.instance
            || accepted.reservations.identity() != accepted.target.reservations
            || accepted.external_inputs.identity() != accepted.target.external_inputs
            || accepted.external_inputs.identity() != accepted.evidence.external_inputs
            || accepted.target.external_inputs != accepted.evidence.external_inputs
        {
            return Err(IntentSessionError::InvalidAuthority);
        }
    }
    if let Some(accepted) = &checkpoint.accepted {
        reservations.validate_superset_of(&accepted.reservations)?;
    }
    Ok(())
}

fn validate_checkpoint_reservation_authority(
    checkpoint: &SessionCheckpoint,
    reservations: &IntentReservationLedger,
    current: bool,
) -> Result<(), IntentSessionError> {
    reservations.validate_graph_metadata(&checkpoint.graph)?;
    let live_high_water = reservations
        .next_reservation_high_water()
        .ok_or(IntentSessionError::InvalidAuthority)?;
    let Some(checkpoint_high_water) = checkpoint.reservation_high_water else {
        return Err(IntentSessionError::InvalidAuthority);
    };
    if checkpoint_high_water.raw() == 0
        || (current && checkpoint_high_water != live_high_water)
        || (!current && checkpoint_high_water > live_high_water)
        || checkpoint.graph.nodes().values().any(|node| {
            node.reservations
                .keys()
                .any(|reservation| *reservation >= checkpoint_high_water)
        })
        || checkpoint.accepted.as_ref().is_some_and(|accepted| {
            !reservation_ledger_is_contiguous(&accepted.reservations)
                || accepted
                    .reservations
                    .entries()
                    .keys()
                    .any(|reservation| *reservation >= checkpoint_high_water)
        })
    {
        return Err(IntentSessionError::InvalidAuthority);
    }
    let reservation_identity_valid = if current {
        checkpoint.reservation_identity == reservations.identity()
    } else {
        historical_checkpoint_reservation_identity_valid(checkpoint, reservations)
    };
    if checkpoint.reservation_identity.0.revision > reservations.revision()
        || !reservation_identity_valid
    {
        return Err(IntentSessionError::InvalidAuthority);
    }
    Ok(())
}

fn validate_semantic_state(
    graph: &IntentGraph,
    instance: &IntentInstanceState,
    external_inputs: &IntentExternalInputs,
) -> Result<(), IntentSessionError> {
    graph.validate()?;
    external_inputs.validate()?;
    for (leaf, value) in instance.values() {
        graph.writable_leaf(*leaf)?;
        if !literal_matches_leaf(value, leaf.field) {
            return Err(IntentSessionError::InvalidInstance);
        }
        value.validate()?;
    }
    Ok(())
}

fn validate_allocator<'a>(
    allocator: IntentAllocatorHighWater,
    checkpoints: impl Iterator<Item = &'a SessionCheckpoint>,
    reservations: &IntentReservationLedger,
) -> Result<(), IntentSessionError> {
    if reservations.next_reservation_high_water() != Some(allocator.next_reservation)
        || !reservation_ledger_is_contiguous(reservations)
    {
        return Err(IntentSessionError::AllocatorRegression);
    }
    if reservations.entries().values().any(|record| {
        record.id.raw() >= allocator.next_reservation.raw()
            || record.owner_node.raw() >= allocator.next_node.raw()
            || record.owner_port.raw() >= allocator.next_port.raw()
    }) {
        return Err(IntentSessionError::AllocatorRegression);
    }
    for checkpoint in checkpoints {
        for node in checkpoint.graph.nodes().values() {
            if node.id.raw() >= allocator.next_node.raw()
                || node
                    .ports
                    .keys()
                    .any(|id| id.raw() >= allocator.next_port.raw())
                || node
                    .reservations
                    .keys()
                    .any(|id| id.raw() >= allocator.next_reservation.raw())
                || node
                    .children
                    .keys()
                    .any(|id| id.raw() >= allocator.next_child.raw())
            {
                return Err(IntentSessionError::AllocatorRegression);
            }
        }
        if checkpoint
            .organization
            .cells()
            .keys()
            .any(|id| id.raw() >= allocator.next_cell.raw() && *id != CellId::from_raw(1))
        {
            return Err(IntentSessionError::AllocatorRegression);
        }
    }
    Ok(())
}

fn reservation_ledger_is_contiguous(reservations: &IntentReservationLedger) -> bool {
    reservations
        .entries()
        .keys()
        .enumerate()
        .all(|(index, reservation)| {
            u64::try_from(index)
                .ok()
                .and_then(|value| value.checked_add(1))
                == Some(reservation.raw())
        })
}

fn push_bounded(history: &mut Vec<HistoryEntry>, entry: HistoryEntry) {
    if history.len() == MAX_INTENT_HISTORY_ENTRIES {
        history.remove(0);
    }
    history.push(entry);
}

/// Patch planning failure. Every variant leaves current intent, accepted scene,
/// allocator, and history unchanged.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum IntentPlanError {
    #[error(transparent)]
    Session(Box<IntentSessionError>),
    #[error(transparent)]
    Graph(#[from] IntentGraphError),
    #[error(transparent)]
    Model(#[from] crate::model::IntentModelError),
    #[error("unknown transaction alias `{0}`")]
    UnknownAlias(IntentKey),
    #[error("unknown intent node {0}")]
    UnknownNode(NodeId),
    #[error("unknown organization cell {0}")]
    UnknownCell(CellId),
    #[error("conflicting unordered patch operations target {target}")]
    ConflictingOperations { target: String },
    #[error("node {node} cannot be edited and deleted in one unordered patch")]
    DeleteEditConflict { node: NodeId },
    #[error("deleting node {node} has dependents {closure:?}")]
    DeleteHasDependents {
        node: NodeId,
        closure: BTreeSet<NodeId>,
    },
    #[error("delete cascade does not exactly match Rust-planned closure")]
    CascadeMismatch {
        expected: BTreeSet<NodeId>,
        actual: BTreeSet<NodeId>,
    },
    #[error("delete cascade roots do not include the addressed node {node}")]
    CascadeRootMissing { node: NodeId },
    #[error("the default Sketch cell cannot be deleted")]
    CannotDeleteDefaultCell,
    #[error("invalid exact cell ordering")]
    InvalidCellOrder,
    #[error("invalid declaration ordering target")]
    InvalidDeclarationOrder,
    #[error("invalid literal for writable leaf {leaf:?}")]
    InvalidInstanceLiteral { leaf: crate::LeafRef },
    #[error("materialization evidence identifies different external inputs")]
    MaterializationInputMismatch,
    #[error("replacement external inputs must advance their host-owned revision")]
    ExternalInputRevisionNotAdvanced,
    #[error("materializer returned invalid failed-node evidence")]
    InvalidEvaluationFailure,
    #[error("candidate evaluation rejected and policy requires accepted publication")]
    EvaluationRejected { failure: IntentEvaluationFailure },
    #[error("cancelled, exhausted, or stale evaluation cannot publish")]
    NonpublishingEvaluation { failure: IntentEvaluationFailure },
    #[error("patch makes no retained change")]
    NoChanges,
    #[error("intent revision exhausted")]
    RevisionExhausted,
    #[error("intent resource limit exceeded for {resource}: {actual} > {limit}")]
    ResourceLimit {
        resource: &'static str,
        actual: usize,
        limit: usize,
    },
}

impl From<IntentSessionError> for IntentPlanError {
    fn from(error: IntentSessionError) -> Self {
        Self::Session(Box::new(error))
    }
}

/// Session commit, Undo/Redo, or persistence failure.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum IntentSessionError {
    #[error("patch belongs to session {actual}; expected {expected}")]
    WrongSession {
        expected: IntentSessionId,
        actual: IntentSessionId,
    },
    #[error("stale intent CAS: expected {expected:?}, current is {actual:?}")]
    StaleCas {
        expected: Box<IntentSessionIdentity>,
        actual: Box<IntentSessionIdentity>,
    },
    #[error("invalid or altered intent plan token")]
    InvalidPlanToken,
    #[error("initial session publication requires one accepted declaration-only plan")]
    InvalidInitializationPlan,
    #[error(
        "empty accepted initialization requires one pristine revision-zero session and matching host inputs"
    )]
    InvalidEmptyInitialization,
    #[error("intent revision exhausted")]
    RevisionExhausted,
    #[error("invalid intent organization")]
    InvalidOrganization,
    #[error("invalid intent instance state")]
    InvalidInstance,
    #[error("invalid intent attempt/accepted authority")]
    InvalidAuthority,
    #[error("intent allocator high-water regresses retained history")]
    AllocatorRegression,
    #[error("intent history exceeds the fixed bound")]
    HistoryLimit,
    #[error("invalid deterministic intent-history descriptor")]
    InvalidHistoryDescriptor,
    #[error("intent history checkpoint does not match its causal transition")]
    InvalidHistoryTransition,
    #[error("the retained intent identity cache does not authenticate its state")]
    InvalidCachedIdentity,
    #[error("intent session JSON exceeds {limit} bytes")]
    JsonResourceLimit { limit: usize },
    #[error("unsupported intent session version {actual}; expected {expected}")]
    UnsupportedVersion { expected: u32, actual: u32 },
    #[error("intent session digest mismatch")]
    DigestMismatch,
    #[error("intent session JSON is not canonical")]
    NonCanonicalJson,
    #[error("authenticated session has no historical contact-domain fields to migrate")]
    HistoricalContactDomainMigrationNotApplicable,
    #[error("historical contact-domain migration rejected node {node}: {reason}")]
    HistoricalContactDomainMigrationRejected { node: NodeId, reason: &'static str },
    #[error("delegated intent checkpoint contains nested Undo/Redo history")]
    DelegatedCheckpointContainsHistory,
    #[error(transparent)]
    Graph(#[from] IntentGraphError),
    #[error(transparent)]
    Model(#[from] crate::model::IntentModelError),
    #[error(transparent)]
    Key(#[from] crate::IntentKeyError),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ConstraintKind, GeometryRecipeKind, InputRole, InputSlot, IntentFieldKey, IntentLiteral,
        IntentNodeDraft, IntentNodeKind, IntentPatchOperationKind, IntentPortRef,
        IntentReservationState, LeafField, LeafRef, ParameterIntentKind,
    };

    fn key(value: &str) -> IntentKey {
        IntentKey::new(value).unwrap()
    }

    fn accepted(candidate: &IntentCandidate) -> IntentEvaluation {
        IntentEvaluation::Accepted {
            evidence: MaterializationEvidence::new_host_artifacts(
                candidate.external_inputs().identity(),
                b"materialization".to_vec(),
                b"ownership".to_vec(),
                b"host-validation".to_vec(),
            )
            .unwrap(),
        }
    }

    fn session_with_historical_contact_domain() -> IntentSession {
        let mut session = IntentSession::with_id(IntentSessionId::from_raw(0x91_c0_1d)).unwrap();
        let primary = crate::IntentPortSelector::Node {
            role: crate::IntentPortRole::Primary,
            index: 0,
        };
        let point = IntentNodeDraft::new(
            IntentNodeKind::Geometry {
                recipe: GeometryRecipeKind::SketchPoint,
            },
            key("historical.contact.point"),
        )
        .with_instance_leaf(
            primary,
            LeafField::X,
            IntentLiteral::Quantity {
                value: 0.25,
                unit: crate::IntentUnit::Length,
            },
        )
        .with_instance_leaf(
            primary,
            LeafField::Y,
            IntentLiteral::Quantity {
                value: 0.0,
                unit: crate::IntentUnit::Length,
            },
        );
        let line = IntentNodeDraft::new(
            IntentNodeKind::Geometry {
                recipe: GeometryRecipeKind::Segment,
            },
            key("historical.contact.line"),
        );
        let contact = IntentNodeDraft::new(
            IntentNodeKind::Constraint {
                constraint: ConstraintKind::PointOnCurve,
            },
            key("historical.contact.constraint"),
        )
        .with_input(
            InputSlot::new(InputRole::Point, 0),
            PatchPortRef::Alias {
                node: key("point"),
                selector: primary,
            },
        )
        .with_input(
            InputSlot::new(InputRole::Span, 0),
            PatchPortRef::Alias {
                node: key("line"),
                selector: crate::IntentPortSelector::Node {
                    role: crate::IntentPortRole::Span,
                    index: 0,
                },
            },
        );
        let patch = IntentPatch::new(
            session.identity(),
            IntentPatchPolicy::RequireAccepted,
            vec![
                IntentPatchOperation::CreateNode {
                    alias: key("point"),
                    draft: Box::new(point),
                    cell: None,
                },
                IntentPatchOperation::CreateNode {
                    alias: key("line"),
                    draft: Box::new(line),
                    cell: None,
                },
                IntentPatchOperation::CreateNode {
                    alias: key("contact"),
                    draft: Box::new(contact),
                    cell: None,
                },
            ],
        );
        let plan = session.plan_patch(patch, accepted).unwrap();
        session.commit_plan(plan).unwrap();
        commit_point(
            &mut session,
            "post-contact",
            "historical.contact.post-contact",
        );
        session
            .undo()
            .unwrap()
            .expect("post-contact transaction is redoable");
        session
    }

    fn inject_historical_contact_field(
        checkpoint: &mut SessionCheckpoint,
        field: &str,
        value: IntentLiteral,
    ) {
        historical_contact_node_mut(checkpoint)
            .fields
            .insert(IntentFieldKey(key(field)), value);
    }

    fn historical_contact_node_mut(checkpoint: &mut SessionCheckpoint) -> &mut crate::IntentNode {
        checkpoint
            .graph
            .nodes
            .values_mut()
            .find(|node| {
                matches!(
                    node.kind,
                    IntentNodeKind::Constraint {
                        constraint: ConstraintKind::PointOnCurve
                    }
                )
            })
            .expect("historical fixture contact declaration")
    }

    fn historical_line_node_mut(
        checkpoint: &mut SessionCheckpoint,
    ) -> Option<&mut crate::IntentNode> {
        checkpoint.graph.nodes.values_mut().find(|node| {
            matches!(
                node.kind,
                IntentNodeKind::Geometry {
                    recipe: GeometryRecipeKind::Segment
                }
            )
        })
    }

    fn reauthenticate_historical_checkpoint(checkpoint: &mut SessionCheckpoint) {
        if let Some(accepted) = &mut checkpoint.accepted {
            accepted.graph.clone_from(&checkpoint.graph);
            accepted.target = semantic_identity(
                &accepted.graph,
                &accepted.instance,
                accepted.reservations.identity(),
                &accepted.external_inputs,
            );
        }
        let target = semantic_identity(
            &checkpoint.graph,
            &checkpoint.instance,
            checkpoint.reservation_identity,
            &checkpoint.external_inputs,
        );
        if let Some(attempt) = &mut checkpoint.latest_attempt {
            attempt.target = target;
            attempt.materialization_digest = checkpoint
                .accepted
                .as_ref()
                .map(|accepted| accepted.evidence.digest);
        }
    }

    fn authenticated_historical_contact_json() -> String {
        let session = session_with_historical_contact_domain();
        let mut wire: IntentSessionWire =
            serde_json::from_str(&session.to_canonical_json().unwrap()).unwrap();
        for checkpoint in std::iter::once(&mut wire.current)
            .chain(wire.undo.iter_mut().map(|entry| &mut entry.checkpoint))
            .chain(wire.redo.iter_mut().map(|entry| &mut entry.checkpoint))
        {
            if checkpoint.graph.nodes().values().any(|node| {
                matches!(
                    node.kind,
                    IntentNodeKind::Constraint {
                        constraint: ConstraintKind::PointOnCurve
                    }
                )
            }) {
                inject_historical_contact_field(
                    checkpoint,
                    "contact_domain",
                    IntentLiteral::Enum(key("bounded")),
                );
                inject_historical_contact_field(
                    checkpoint,
                    "contact_domain_lower",
                    IntentLiteral::Quantity {
                        value: 0.0,
                        unit: crate::IntentUnit::Dimensionless,
                    },
                );
                inject_historical_contact_field(
                    checkpoint,
                    "contact_domain_upper",
                    IntentLiteral::Quantity {
                        value: 1.0,
                        unit: crate::IntentUnit::Dimensionless,
                    },
                );
                reauthenticate_historical_checkpoint(checkpoint);
            }
        }
        bind_history_edges(&wire.current, &mut wire.undo, &mut wire.redo);
        authenticated_wire_json(wire)
    }

    #[test]
    fn historical_contact_domain_migration_is_explicit_strict_and_canonical() {
        let historical = authenticated_historical_contact_json();
        assert!(matches!(
            IntentSession::from_json(&historical),
            Err(IntentSessionError::Graph(
                IntentGraphError::UnknownDefinitionField { ref field, .. }
            )) if field.0.as_str() == "contact_domain"
        ));
        let migrated = IntentSession::from_historical_contact_domain_json(&historical).unwrap();
        let canonical = migrated.to_canonical_json().unwrap();
        assert!(!canonical.contains("contact_domain"));
        assert_eq!(
            IntentSession::from_json(&canonical)
                .unwrap()
                .to_canonical_json()
                .unwrap(),
            canonical
        );
        assert!(migrated.undo_len() > 0);
        assert!(migrated.redo_len() > 0);
        let mut replayed = migrated;
        replayed.redo().unwrap().expect("migrated Redo is valid");
        replayed.undo().unwrap().expect("migrated Undo is valid");
        replayed.validate().unwrap();

        let mut tampered = historical.clone().into_bytes();
        let index = tampered
            .windows(b"bounded".len())
            .position(|window| window == b"bounded")
            .unwrap();
        tampered[index] = b'B';
        assert!(matches!(
            IntentSession::from_historical_contact_domain_json(
                std::str::from_utf8(&tampered).unwrap()
            ),
            Err(IntentSessionError::DigestMismatch)
        ));

        let ordinary = session_with_historical_contact_domain()
            .to_canonical_json()
            .unwrap();
        assert!(matches!(
            IntentSession::from_historical_contact_domain_json(&ordinary),
            Err(IntentSessionError::HistoricalContactDomainMigrationNotApplicable)
        ));
    }

    #[test]
    fn historical_contact_domain_migration_preserves_legacy_branch_normalization_semantics() {
        let raw: [f64; 2] = [0.737_788_637_523_594_1, 0.675_031_796_540_784];
        let length = raw[0].hypot(raw[1]);
        let expected = [raw[0] / length, raw[1] / length];
        assert_ne!(
            raw, expected,
            "fixture must expose the one-ulp migration seam"
        );

        let mut wire: IntentSessionWire =
            serde_json::from_str(&authenticated_historical_contact_json()).unwrap();
        for checkpoint in std::iter::once(&mut wire.current)
            .chain(wire.undo.iter_mut().map(|entry| &mut entry.checkpoint))
            .chain(wire.redo.iter_mut().map(|entry| &mut entry.checkpoint))
        {
            if let Some(line) = historical_line_node_mut(checkpoint) {
                line.fields.insert(
                    IntentFieldKey(key("branch_direction")),
                    IntentLiteral::Point(raw),
                );
            }
            reauthenticate_historical_checkpoint(checkpoint);
        }
        bind_history_edges(&wire.current, &mut wire.undo, &mut wire.redo);

        let migrated =
            IntentSession::from_historical_contact_domain_json(&authenticated_wire_json(wire))
                .expect("historical materializer branch meaning migrates");
        let migrated_wire: IntentSessionWire =
            serde_json::from_str(&migrated.to_canonical_json().unwrap()).unwrap();
        for checkpoint in std::iter::once(&migrated_wire.current)
            .chain(migrated_wire.undo.iter().map(|entry| &entry.checkpoint))
            .chain(migrated_wire.redo.iter().map(|entry| &entry.checkpoint))
        {
            let require_expected = |graph: &IntentGraph| {
                let Some(line) = graph.nodes.values().find(|node| {
                    matches!(
                        node.kind,
                        IntentNodeKind::Geometry {
                            recipe: GeometryRecipeKind::Segment
                        }
                    )
                }) else {
                    return;
                };
                assert_eq!(
                    line.fields.get(&IntentFieldKey(key("branch_direction"))),
                    Some(&IntentLiteral::Point(expected)),
                );
            };
            require_expected(&checkpoint.graph);
            if let Some(accepted) = &checkpoint.accepted {
                require_expected(&accepted.graph);
            }
        }
    }

    #[test]
    fn historical_contact_domain_migration_rejects_nondefault_mixed_and_unrelated_fields() {
        let base = authenticated_historical_contact_json();
        for (label, mutate) in [
            (
                "non-default bound",
                (|checkpoint: &mut SessionCheckpoint| {
                    inject_historical_contact_field(
                        checkpoint,
                        "contact_domain_upper",
                        IntentLiteral::Quantity {
                            value: 0.5,
                            unit: crate::IntentUnit::Dimensionless,
                        },
                    );
                }) as fn(&mut SessionCheckpoint),
            ),
            (
                "mixed current field",
                |checkpoint: &mut SessionCheckpoint| {
                    inject_historical_contact_field(
                        checkpoint,
                        "contact_range_upper",
                        IntentLiteral::Quantity {
                            value: 0.5,
                            unit: crate::IntentUnit::Dimensionless,
                        },
                    );
                },
            ),
            (
                "non-enum domain literal",
                |checkpoint: &mut SessionCheckpoint| {
                    inject_historical_contact_field(
                        checkpoint,
                        "contact_domain",
                        IntentLiteral::Boolean(true),
                    );
                },
            ),
            (
                "auxiliary without domain",
                |checkpoint: &mut SessionCheckpoint| {
                    historical_contact_node_mut(checkpoint)
                        .fields
                        .remove(&IntentFieldKey(key("contact_domain")));
                },
            ),
            (
                "unrelated unknown field",
                |checkpoint: &mut SessionCheckpoint| {
                    inject_historical_contact_field(
                        checkpoint,
                        "unrelated_domain",
                        IntentLiteral::Enum(key("bounded")),
                    );
                },
            ),
        ] {
            let mut wire: IntentSessionWire = serde_json::from_str(&base).unwrap();
            wire.undo.clear();
            wire.redo.clear();
            mutate(&mut wire.current);
            reauthenticate_historical_checkpoint(&mut wire.current);
            bind_history_edges(&wire.current, &mut wire.undo, &mut wire.redo);
            let result =
                IntentSession::from_historical_contact_domain_json(&authenticated_wire_json(wire));
            assert!(result.is_err(), "{label} must reject");
        }

        let mut periodic: IntentSessionWire = serde_json::from_str(&base).unwrap();
        periodic.undo.clear();
        periodic.redo.clear();
        inject_historical_contact_field(
            &mut periodic.current,
            "contact_domain",
            IntentLiteral::Enum(key("periodic")),
        );
        inject_historical_contact_field(
            &mut periodic.current,
            "contact_domain_period",
            IntentLiteral::Quantity {
                value: 6.0,
                unit: crate::IntentUnit::Dimensionless,
            },
        );
        reauthenticate_historical_checkpoint(&mut periodic.current);
        bind_history_edges(&periodic.current, &mut periodic.undo, &mut periodic.redo);
        assert!(matches!(
            IntentSession::from_historical_contact_domain_json(&authenticated_wire_json(periodic)),
            Err(IntentSessionError::HistoricalContactDomainMigrationRejected { .. })
        ));
    }

    #[test]
    fn historical_contact_domain_migration_preserves_explicit_supporting_line_intent() {
        let mut wire: IntentSessionWire =
            serde_json::from_str(&authenticated_historical_contact_json()).unwrap();
        wire.undo.clear();
        wire.redo.clear();
        let node = historical_contact_node_mut(&mut wire.current);
        node.fields.insert(
            IntentFieldKey(key("contact_domain")),
            IntentLiteral::Enum(key("supporting_line")),
        );
        node.fields
            .remove(&IntentFieldKey(key("contact_domain_lower")));
        node.fields
            .remove(&IntentFieldKey(key("contact_domain_upper")));
        reauthenticate_historical_checkpoint(&mut wire.current);
        bind_history_edges(&wire.current, &mut wire.undo, &mut wire.redo);

        let migrated =
            IntentSession::from_historical_contact_domain_json(&authenticated_wire_json(wire))
                .expect("exact supporting-line topology maps to current contact support");
        let node = migrated
            .graph()
            .nodes()
            .values()
            .find(|node| {
                matches!(
                    node.kind,
                    IntentNodeKind::Constraint {
                        constraint: ConstraintKind::PointOnCurve
                    }
                )
            })
            .unwrap();
        assert_eq!(
            node.fields.get(&IntentFieldKey(key("contact_support"))),
            Some(&IntentLiteral::Enum(key("supporting_line")))
        );
        assert!(
            !node
                .fields
                .keys()
                .any(|field| field.0.as_str().starts_with("contact_domain"))
        );
        IntentSession::from_json(&migrated.to_canonical_json().unwrap())
            .expect("supporting-line migration emits an ordinary current-schema session");
    }

    fn commit_point(session: &mut IntentSession, alias: &str, symbol: &str) {
        let patch = IntentPatch::new(
            session.identity(),
            IntentPatchPolicy::RequireAccepted,
            vec![IntentPatchOperation::CreateNode {
                alias: key(alias),
                draft: Box::new(IntentNodeDraft::new(
                    IntentNodeKind::Geometry {
                        recipe: GeometryRecipeKind::SketchPoint,
                    },
                    key(symbol),
                )),
                cell: None,
            }],
        );
        let plan = session.plan_patch(patch, accepted).unwrap();
        session.commit_plan(plan).unwrap();
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "one owning transaction regression keeps optional removal, fail-closed schema checks, conflicts, history, and canonical reload together"
    )]
    fn unset_definition_field_removes_optional_state_and_rejects_required_or_unknown_fields() {
        let mut session = IntentSession::with_id(IntentSessionId::from_raw(0x91_0c02)).unwrap();
        let role = IntentFieldKey(key("role"));
        let kind = IntentFieldKey(key("kind"));
        let create = IntentPatch::new(
            session.identity(),
            IntentPatchPolicy::RequireAccepted,
            vec![
                IntentPatchOperation::CreateNode {
                    alias: key("point"),
                    draft: Box::new(
                        IntentNodeDraft::new(
                            IntentNodeKind::Geometry {
                                recipe: GeometryRecipeKind::SketchPoint,
                            },
                            key("unset.point"),
                        )
                        .with_field(role.clone(), IntentLiteral::Enum(key("construction"))),
                    ),
                    cell: None,
                },
                IntentPatchOperation::CreateNode {
                    alias: key("parameter"),
                    draft: Box::new(
                        IntentNodeDraft::new(
                            IntentNodeKind::Parameter {
                                parameter: ParameterIntentKind::Parameter,
                            },
                            key("unset.parameter"),
                        )
                        .with_field(kind.clone(), IntentLiteral::Enum(key("length"))),
                    ),
                    cell: None,
                },
            ],
        );
        let create_plan = session.plan_patch(create, accepted).unwrap();
        let point = create_plan.aliases().node(&key("point")).unwrap();
        let parameter = create_plan.aliases().node(&key("parameter")).unwrap();
        session.commit_plan(create_plan).unwrap();

        let unset_optional = IntentPatch::new(
            session.identity(),
            IntentPatchPolicy::RequireAccepted,
            vec![IntentPatchOperation::UnsetDefinitionField {
                node: point,
                field: role.clone(),
            }],
        );
        let unset_plan = session.plan_patch(unset_optional, accepted).unwrap();
        assert_eq!(
            unset_plan.descriptor().operation_kinds,
            vec![IntentPatchOperationKind::UnsetDefinitionField]
        );
        session.commit_plan(unset_plan).unwrap();
        assert!(
            !session
                .graph()
                .node(point)
                .unwrap()
                .fields
                .contains_key(&role)
        );
        session.validate().unwrap();

        let accepted_json = session.to_canonical_json().unwrap();
        let required = IntentPatch::new(
            session.identity(),
            IntentPatchPolicy::RequireAccepted,
            vec![IntentPatchOperation::UnsetDefinitionField {
                node: parameter,
                field: kind,
            }],
        );
        assert!(matches!(
            session.plan_patch(required, accepted),
            Err(IntentPlanError::Graph(
                IntentGraphError::MissingRequiredDefinitionField { node, .. }
            )) if node == parameter
        ));
        assert_eq!(session.to_canonical_json().unwrap(), accepted_json);

        let unknown = IntentPatch::new(
            session.identity(),
            IntentPatchPolicy::RequireAccepted,
            vec![IntentPatchOperation::UnsetDefinitionField {
                node: point,
                field: IntentFieldKey(key("not_a_schema_field")),
            }],
        );
        assert!(matches!(
            session.plan_patch(unknown, accepted),
            Err(IntentPlanError::Graph(
                IntentGraphError::UnknownDefinitionField { node, .. }
            )) if node == point
        ));
        assert_eq!(session.to_canonical_json().unwrap(), accepted_json);

        let repeated = IntentPatch::new(
            session.identity(),
            IntentPatchPolicy::RequireAccepted,
            vec![IntentPatchOperation::UnsetDefinitionField {
                node: point,
                field: role.clone(),
            }],
        );
        assert!(matches!(
            session.plan_patch(repeated, accepted),
            Err(IntentPlanError::NoChanges)
        ));

        let conflicting = IntentPatch::new(
            session.identity(),
            IntentPatchPolicy::RequireAccepted,
            vec![
                IntentPatchOperation::SetDefinitionField {
                    node: point,
                    field: role.clone(),
                    value: IntentLiteral::Enum(key("profile")),
                },
                IntentPatchOperation::UnsetDefinitionField {
                    node: point,
                    field: role.clone(),
                },
            ],
        );
        assert!(matches!(
            session.plan_patch(conflicting, accepted),
            Err(IntentPlanError::ConflictingOperations { target })
                if target == format!("field:{point}:role")
        ));
        assert_eq!(session.to_canonical_json().unwrap(), accepted_json);

        session
            .undo()
            .unwrap()
            .expect("optional unset enters history");
        assert_eq!(
            session.graph().node(point).unwrap().fields.get(&role),
            Some(&IntentLiteral::Enum(key("construction")))
        );
        session.redo().unwrap().expect("optional unset replays");
        assert!(
            !session
                .graph()
                .node(point)
                .unwrap()
                .fields
                .contains_key(&role)
        );
        IntentSession::from_json(&session.to_canonical_json().unwrap()).unwrap();
    }

    fn session_with_undo_and_redo() -> IntentSession {
        let mut session = IntentSession::with_id(IntentSessionId::from_raw(0x83_fd)).unwrap();
        commit_point(&mut session, "point-a", "wire.point.a");
        commit_point(&mut session, "point-b", "wire.point.b");
        commit_point(&mut session, "point-c", "wire.point.c");
        session.undo().unwrap().unwrap();
        assert_eq!(session.undo_len(), 2);
        assert_eq!(session.redo_len(), 1);
        session
    }

    fn session_with_retained_failure() -> IntentSession {
        let mut session = IntentSession::with_id(IntentSessionId::from_raw(0x83_fa)).unwrap();
        commit_point(&mut session, "accepted", "wire.accepted");
        commit_retained_failure(
            &mut session,
            "retained",
            "wire.retained",
            "retained-failure",
        );
        session
    }

    #[test]
    fn delegated_checkpoint_is_history_free_but_retains_current_and_allocator_authority() {
        let session = session_with_undo_and_redo();
        let delegated = session.delegated_checkpoint().unwrap();
        let consumed = session.clone().into_delegated_checkpoint().unwrap();
        assert_eq!(session.undo_len(), 2);
        assert_eq!(session.redo_len(), 1);
        assert_eq!(delegated.undo_len(), 0);
        assert_eq!(delegated.redo_len(), 0);
        assert_eq!(delegated.id(), session.id());
        assert_eq!(delegated.graph(), session.graph());
        assert_eq!(delegated.instance(), session.instance());
        assert_eq!(delegated.reservations(), session.reservations());
        assert_eq!(delegated.organization(), session.organization());
        assert_eq!(delegated.external_inputs(), session.external_inputs());
        assert_eq!(delegated.latest_attempt(), session.latest_attempt());
        assert_eq!(delegated.accepted(), session.accepted());
        assert_eq!(
            delegated.allocator_high_water(),
            session.allocator_high_water()
        );
        assert_eq!(delegated.identity().revision, session.identity().revision);
        assert_ne!(delegated.identity().digest, session.identity().digest);
        assert_eq!(consumed, delegated);

        let json = session.to_delegated_checkpoint_json().unwrap();
        let restored = IntentSession::from_delegated_checkpoint_json(&json).unwrap();
        assert_eq!(restored, delegated);
        assert_eq!(restored.to_canonical_json().unwrap(), json);
        assert!(matches!(
            IntentSession::from_delegated_checkpoint_json(&session.to_canonical_json().unwrap()),
            Err(IntentSessionError::DelegatedCheckpointContainsHistory)
        ));
    }

    #[test]
    fn delegated_patch_never_constructs_nested_history_and_rejects_non_delegated_sources() {
        let mut ordinary =
            IntentSession::with_id(IntentSessionId::from_raw(0x85_de1e_6a7e)).unwrap();
        commit_point(&mut ordinary, "point-a", "delegated.point.a");
        let patch_for = |session: &IntentSession| {
            IntentPatch::new(
                session.identity(),
                IntentPatchPolicy::RequireAccepted,
                vec![IntentPatchOperation::CreateNode {
                    alias: key("point-b"),
                    draft: Box::new(IntentNodeDraft::new(
                        IntentNodeKind::Geometry {
                            recipe: GeometryRecipeKind::SketchPoint,
                        },
                        key("delegated.point.b"),
                    )),
                    cell: None,
                }],
            )
        };

        assert!(matches!(
            ordinary.plan_delegated_patch(patch_for(&ordinary), accepted),
            Err(IntentPlanError::Session(error))
                if matches!(
                    *error,
                    IntentSessionError::DelegatedCheckpointContainsHistory
                )
        ));

        let mut delegated = ordinary.delegated_checkpoint().unwrap();
        let base = delegated.identity();
        let plan = delegated
            .plan_delegated_patch(patch_for(&delegated), accepted)
            .unwrap();
        assert_eq!(plan.base(), base);
        assert_eq!(plan.disposition(), IntentPlanDisposition::Accepted);
        let target = delegated.commit_plan(plan).unwrap();
        assert_eq!(target, delegated.identity());
        assert_eq!(target.revision.raw(), base.revision.raw() + 1);
        assert_eq!(delegated.graph().nodes().len(), 2);
        assert_eq!(delegated.undo_len(), 0);
        assert_eq!(delegated.redo_len(), 0);
        delegated.validate().unwrap();
    }

    #[test]
    fn unordered_patch_rebinds_survivor_before_validating_old_source_deletion() {
        let mut session = IntentSession::with_id(IntentSessionId::from_raw(0x84_0041)).unwrap();
        let point_ref = |alias: &str| PatchPortRef::Alias {
            node: key(alias),
            selector: crate::IntentPortSelector::Node {
                role: crate::IntentPortRole::Primary,
                index: 0,
            },
        };
        let point = |symbol: &str| {
            IntentNodeDraft::new(
                IntentNodeKind::Geometry {
                    recipe: GeometryRecipeKind::SketchPoint,
                },
                key(symbol),
            )
        };
        let line = IntentNodeDraft::new(
            IntentNodeKind::Geometry {
                recipe: GeometryRecipeKind::Segment,
            },
            key("warm.line"),
        )
        .with_input(InputSlot::new(InputRole::Point, 0), point_ref("old"))
        .with_input(InputSlot::new(InputRole::Point, 1), point_ref("end"));
        let create = IntentPatch::new(
            session.identity(),
            IntentPatchPolicy::RequireAccepted,
            vec![
                IntentPatchOperation::CreateNode {
                    alias: key("old"),
                    draft: Box::new(point("warm.old")),
                    cell: None,
                },
                IntentPatchOperation::CreateNode {
                    alias: key("replacement"),
                    draft: Box::new(point("warm.replacement")),
                    cell: None,
                },
                IntentPatchOperation::CreateNode {
                    alias: key("end"),
                    draft: Box::new(point("warm.end")),
                    cell: None,
                },
                IntentPatchOperation::CreateNode {
                    alias: key("line"),
                    draft: Box::new(line),
                    cell: None,
                },
            ],
        );
        let create_plan = session.plan_patch(create, accepted).unwrap();
        let old = create_plan.aliases().node(&key("old")).unwrap();
        let line = create_plan.aliases().node(&key("line")).unwrap();
        let replacement = create_plan
            .aliases()
            .port(
                &key("replacement"),
                crate::IntentPortSelector::Node {
                    role: crate::IntentPortRole::Primary,
                    index: 0,
                },
            )
            .unwrap();
        session.commit_plan(create_plan).unwrap();
        assert_eq!(
            session.graph().dependent_closure([old]).unwrap().len(),
            2,
            "the old point initially owns the retained line dependent",
        );

        // Delete deliberately precedes RebindInput. Patch-array order is not
        // semantic: the transaction must first detach the retained line, then
        // authenticate RejectDependents against the resulting graph.
        let edit = IntentPatch::new(
            session.identity(),
            IntentPatchPolicy::RequireAccepted,
            vec![
                IntentPatchOperation::DeleteNode {
                    node: old,
                    policy: DeletePolicy::RejectDependents,
                },
                IntentPatchOperation::RebindInput {
                    node: line,
                    slot: InputSlot::new(InputRole::Point, 0),
                    source: PatchPortRef::Stable { port: replacement },
                },
            ],
        );
        let edit_plan = session.plan_patch(edit, accepted).unwrap();
        session.commit_plan(edit_plan).unwrap();

        assert!(session.graph().node(old).is_none());
        assert_eq!(
            session.graph().node(line).unwrap().inputs[&InputSlot::new(InputRole::Point, 0)],
            replacement,
        );
        session.validate().unwrap();
    }

    fn commit_retained_failure(
        session: &mut IntentSession,
        alias: &str,
        symbol: &str,
        diagnostic: &str,
    ) {
        let patch = IntentPatch::new(
            session.identity(),
            IntentPatchPolicy::RetainFailedIntent,
            vec![IntentPatchOperation::CreateNode {
                alias: key(alias),
                draft: Box::new(IntentNodeDraft::new(
                    IntentNodeKind::Geometry {
                        recipe: GeometryRecipeKind::SketchPoint,
                    },
                    key(symbol),
                )),
                cell: None,
            }],
        );
        let plan = session
            .plan_patch(patch, |candidate| IntentEvaluation::Failed {
                failure: IntentEvaluationFailure {
                    kind: IntentEvaluationFailureKind::MaterializationRejected,
                    failed_nodes: BTreeSet::from([*candidate
                        .graph()
                        .nodes()
                        .keys()
                        .next_back()
                        .expect("retained declaration")]),
                    diagnostic: key(diagnostic),
                },
            })
            .unwrap();
        session.commit_plan(plan).unwrap();
        assert_eq!(
            session.latest_attempt().unwrap().disposition,
            IntentAttemptDisposition::RetainedFailed
        );
    }

    fn corrupt_accepted_graph(checkpoint: &mut SessionCheckpoint) {
        let accepted = checkpoint.accepted.as_mut().expect("accepted authority");
        let node_id = *accepted.graph.nodes.keys().next().expect("accepted node");
        let output = accepted.graph.nodes[&node_id]
            .ports
            .values()
            .next()
            .expect("accepted output");
        let source = IntentPortRef {
            node: node_id,
            port: output.id,
            kind: output.kind,
        };
        accepted
            .graph
            .nodes
            .get_mut(&node_id)
            .expect("accepted node")
            .inputs
            .insert(InputSlot::new(InputRole::Point, 0), source);
        accepted.target = semantic_identity(
            &accepted.graph,
            &accepted.instance,
            accepted.reservations.identity(),
            &accepted.external_inputs,
        );
    }

    fn corrupt_accepted_instance(checkpoint: &mut SessionCheckpoint) {
        let accepted = checkpoint.accepted.as_mut().expect("accepted authority");
        let node_id = *accepted.graph.nodes.keys().next().expect("accepted node");
        let port_id = accepted.graph.nodes[&node_id]
            .ports
            .keys()
            .next()
            .copied()
            .expect("accepted output");
        accepted.instance.values.insert(
            LeafRef {
                node: node_id,
                port: port_id,
                field: LeafField::Value,
            },
            IntentLiteral::Quantity {
                value: 83.0,
                unit: crate::IntentUnit::Dimensionless,
            },
        );
        accepted.target = semantic_identity(
            &accepted.graph,
            &accepted.instance,
            accepted.reservations.identity(),
            &accepted.external_inputs,
        );
    }

    fn rewrite_evidence_as_legacy(
        evidence: &mut MaterializationEvidence,
        external_inputs: &IntentExternalInputs,
    ) {
        evidence.external_inputs = legacy_external_identity(external_inputs);
        evidence.digest = legacy_materialization_evidence_digest(evidence);
    }

    fn rewrite_checkpoint_as_legacy(
        checkpoint: &mut SessionCheckpoint,
        current_reservations: &IntentReservationLedger,
    ) {
        let reservation_identity =
            checkpoint_reservation_ledger_from_high_water(checkpoint, current_reservations)
                .filter(|candidate| candidate.identity() == checkpoint.reservation_identity)
                .expect("current checkpoint reservation identity is reconstructible")
                .legacy_identity();
        checkpoint.reservation_identity = reservation_identity;
        checkpoint.reservation_high_water = None;

        let materialization_digest = checkpoint.accepted.as_mut().map(|accepted| {
            rewrite_evidence_as_legacy(&mut accepted.evidence, &accepted.external_inputs);
            accepted.target = legacy_semantic_identity(
                &accepted.graph,
                &accepted.instance,
                accepted.reservations.legacy_identity(),
                &accepted.external_inputs,
            );
            accepted.evidence.digest
        });
        let target = legacy_semantic_identity(
            &checkpoint.graph,
            &checkpoint.instance,
            reservation_identity,
            &checkpoint.external_inputs,
        );
        if let Some(attempt) = &mut checkpoint.latest_attempt {
            attempt.target = target;
            attempt.materialization_digest = match attempt.disposition {
                IntentAttemptDisposition::Accepted => materialization_digest,
                IntentAttemptDisposition::RetainedFailed
                | IntentAttemptDisposition::OrganizationOnly => None,
            };
        }
    }

    fn rewrite_wire_as_legacy(wire: &mut IntentSessionWire) {
        rewrite_checkpoint_as_legacy(&mut wire.current, &wire.reservations);
        for entry in wire.undo.iter_mut().chain(&mut wire.redo) {
            rewrite_checkpoint_as_legacy(&mut entry.checkpoint, &wire.reservations);
            entry.before = ContentDigest::zero();
            entry.after = ContentDigest::zero();
        }
        wire.version = LEGACY_INTENT_SESSION_VERSION;
        wire.digest = legacy_session_wire_digest(wire);
    }

    fn authenticated_wire_json(mut wire: IntentSessionWire) -> String {
        wire.digest = session_wire_digest(&wire);
        serde_json::to_string(&wire).unwrap()
    }

    fn assert_component_too_large<T>(
        result: &Result<T, IntentSessionError>,
        expected_component: &'static str,
    ) {
        assert!(matches!(
            result,
            Err(IntentSessionError::Model(
                crate::IntentModelError::ComponentTooLarge {
                    component,
                    limit: crate::model::MAX_INTENT_OPAQUE_COMPONENT_BYTES,
                }
            )) if *component == expected_component
        ));
    }

    #[test]
    fn tombstone_ledger_is_independent_of_bounded_history_eviction() {
        let mut session = IntentSession::with_id(IntentSessionId::from_raw(0x83ff)).unwrap();
        let create = IntentPatch::new(
            session.identity(),
            IntentPatchPolicy::RequireAccepted,
            vec![IntentPatchOperation::CreateNode {
                alias: key("doomed"),
                draft: Box::new(IntentNodeDraft::new(
                    IntentNodeKind::Geometry {
                        recipe: GeometryRecipeKind::SketchPoint,
                    },
                    key("doomed.point"),
                )),
                cell: None,
            }],
        );
        let plan = session.plan_patch(create, accepted).unwrap();
        let doomed = plan.aliases().node(&key("doomed")).unwrap();
        session.commit_plan(plan).unwrap();
        let reservation = *session
            .graph()
            .node(doomed)
            .unwrap()
            .reservations
            .keys()
            .next()
            .unwrap();
        let delete = IntentPatch::new(
            session.identity(),
            IntentPatchPolicy::RequireAccepted,
            vec![IntentPatchOperation::DeleteNode {
                node: doomed,
                policy: DeletePolicy::RejectDependents,
            }],
        );
        let plan = session.plan_patch(delete, accepted).unwrap();
        session.commit_plan(plan).unwrap();

        let create_survivor = IntentPatch::new(
            session.identity(),
            IntentPatchPolicy::RequireAccepted,
            vec![IntentPatchOperation::CreateNode {
                alias: key("survivor"),
                draft: Box::new(IntentNodeDraft::new(
                    IntentNodeKind::Geometry {
                        recipe: GeometryRecipeKind::SketchPoint,
                    },
                    key("survivor.point"),
                )),
                cell: None,
            }],
        );
        let plan = session.plan_patch(create_survivor, accepted).unwrap();
        let survivor = plan.aliases().node(&key("survivor")).unwrap();
        session.commit_plan(plan).unwrap();

        let mut before = session.checkpoint();

        for offset in 0..MAX_INTENT_HISTORY_ENTRIES {
            let mut after = before.clone();
            after.organization.revision = next_revision(after.organization.revision).unwrap();
            after.organization.node_names.insert(
                survivor,
                key(if offset % 2 == 0 {
                    "survivor.a"
                } else {
                    "survivor.b"
                }),
            );
            let mut diff = IntentSemanticDiff::default();
            diff.organization_nodes.insert(survivor);
            diff.organization_changed = true;
            push_bounded(
                &mut session.undo,
                HistoryEntry {
                    before: history_checkpoint_digest(&before),
                    after: history_checkpoint_digest(&after),
                    checkpoint: before,
                    descriptor: IntentTransactionDescriptor::new(
                        Revision::from_raw(4 + offset as u64),
                        IntentPlanDisposition::OrganizationOnly,
                        vec![IntentPatchOperationKind::RenameNode],
                        diff,
                    ),
                },
            );
            before = after;
        }
        session.restore_checkpoint(before);
        session.revision = Revision::from_raw(3 + MAX_INTENT_HISTORY_ENTRIES as u64);
        let current = session.checkpoint();
        bind_history_edges(&current, &mut session.undo, &mut session.redo);
        session.refresh_cached_identities();

        assert_eq!(session.undo.len(), MAX_INTENT_HISTORY_ENTRIES);
        assert!(session.undo.iter().all(|entry| {
            entry.descriptor.operation_kinds == vec![IntentPatchOperationKind::RenameNode]
        }));
        assert_eq!(
            session.reservations.entries()[&reservation].state,
            IntentReservationState::Tombstoned
        );
        assert!(reservation.raw() < session.allocator.next_reservation.raw());
        session.validate().unwrap();
    }

    #[test]
    fn canonical_import_rejects_inconsistent_accepted_attempt_provenance() {
        let mut session = IntentSession::with_id(IntentSessionId::from_raw(0x83fe)).unwrap();
        let create = IntentPatch::new(
            session.identity(),
            IntentPatchPolicy::RequireAccepted,
            vec![IntentPatchOperation::CreateNode {
                alias: key("accepted"),
                draft: Box::new(IntentNodeDraft::new(
                    IntentNodeKind::Geometry {
                        recipe: GeometryRecipeKind::SketchPoint,
                    },
                    key("accepted.point"),
                )),
                cell: None,
            }],
        );
        let plan = session.plan_patch(create, accepted).unwrap();
        session.commit_plan(plan).unwrap();

        let canonical = session.to_canonical_json().unwrap();
        let mut wire: IntentSessionWire = serde_json::from_str(&canonical).unwrap();
        wire.current.latest_attempt = None;
        wire.digest = session_wire_digest(&wire);
        let forged = serde_json::to_string(&wire).unwrap();
        assert!(matches!(
            IntentSession::from_json(&forged),
            Err(IntentSessionError::InvalidAuthority)
        ));

        let mut wire: IntentSessionWire = serde_json::from_str(&canonical).unwrap();
        wire.current.latest_attempt = None;
        wire.current.accepted = None;
        wire.digest = session_wire_digest(&wire);
        let forged = serde_json::to_string(&wire).unwrap();
        assert!(matches!(
            IntentSession::from_json(&forged),
            Err(IntentSessionError::InvalidAuthority)
        ));

        let mut wire: IntentSessionWire = serde_json::from_str(&canonical).unwrap();
        let target = wire.current.accepted.as_ref().unwrap().target;
        let failed_node = *wire.current.graph.nodes().keys().next().unwrap();
        wire.current.latest_attempt = Some(IntentLatestAttempt {
            target,
            disposition: IntentAttemptDisposition::RetainedFailed,
            materialization_digest: None,
            failed_nodes: BTreeSet::from([failed_node]),
            diagnostic: Some(key("forged-retained-failure")),
        });
        wire.digest = session_wire_digest(&wire);
        let forged = serde_json::to_string(&wire).unwrap();
        assert!(matches!(
            IntentSession::from_json(&forged),
            Err(IntentSessionError::InvalidAuthority)
        ));
    }

    #[test]
    fn canonical_legacy_v1_session_migrates_exactly_to_v2_sha_authority() {
        let session = session_with_undo_and_redo();
        let canonical_v2 = session.to_canonical_json().unwrap();
        let mut legacy: IntentSessionWire = serde_json::from_str(&canonical_v2).unwrap();
        rewrite_wire_as_legacy(&mut legacy);
        let canonical_v1 = serde_json::to_string(&legacy).unwrap();
        assert_eq!(
            serde_json::to_string(
                &serde_json::from_str::<IntentSessionWire>(&canonical_v1).unwrap()
            )
            .unwrap(),
            canonical_v1,
        );

        let migrated = IntentSession::from_json(&canonical_v1).unwrap();
        assert_eq!(migrated, session);
        assert_eq!(migrated.to_canonical_json().unwrap(), canonical_v2);

        let migrated_wire: IntentSessionWire =
            serde_json::from_str(&migrated.to_canonical_json().unwrap()).unwrap();
        assert_eq!(migrated_wire.version, INTENT_SESSION_VERSION);
        assert_eq!(migrated_wire.digest, session_wire_digest(&migrated_wire));
        assert_ne!(migrated_wire.digest, legacy.digest);
    }

    #[test]
    fn canonical_legacy_v1_retained_failed_authority_migrates_exactly() {
        let mut session = IntentSession::with_id(IntentSessionId::from_raw(0x8300_83f1)).unwrap();
        commit_point(&mut session, "accepted", "accepted.point");
        let patch = IntentPatch::new(
            session.identity(),
            IntentPatchPolicy::RetainFailedIntent,
            vec![IntentPatchOperation::CreateNode {
                alias: key("failed"),
                draft: Box::new(IntentNodeDraft::new(
                    IntentNodeKind::Geometry {
                        recipe: GeometryRecipeKind::SketchPoint,
                    },
                    key("failed.point"),
                )),
                cell: None,
            }],
        );
        let plan = session
            .plan_patch(patch, |candidate| IntentEvaluation::Failed {
                failure: IntentEvaluationFailure {
                    kind: IntentEvaluationFailureKind::MaterializationRejected,
                    failed_nodes: BTreeSet::from([*candidate
                        .graph()
                        .nodes()
                        .keys()
                        .next_back()
                        .unwrap()]),
                    diagnostic: key("retained-failure"),
                },
            })
            .unwrap();
        assert_eq!(plan.disposition(), IntentPlanDisposition::RetainedFailed);
        session.commit_plan(plan).unwrap();

        let canonical_v2 = session.to_canonical_json().unwrap();
        let mut legacy: IntentSessionWire = serde_json::from_str(&canonical_v2).unwrap();
        rewrite_wire_as_legacy(&mut legacy);
        let canonical_v1 = serde_json::to_string(&legacy).unwrap();
        let migrated = IntentSession::from_json(&canonical_v1).unwrap();

        assert_eq!(migrated, session);
        assert_eq!(migrated.to_canonical_json().unwrap(), canonical_v2);
        assert_eq!(
            migrated.latest_attempt().unwrap().disposition,
            IntentAttemptDisposition::RetainedFailed
        );
    }

    #[test]
    fn canonical_legacy_v1_migrates_retained_failed_intermediate_history_ledger() {
        let mut session = IntentSession::with_id(IntentSessionId::from_raw(0x8300_83f2)).unwrap();
        commit_point(&mut session, "accepted-a", "accepted.point.a");

        let failed_patch = IntentPatch::new(
            session.identity(),
            IntentPatchPolicy::RetainFailedIntent,
            vec![IntentPatchOperation::CreateNode {
                alias: key("retained-b"),
                draft: Box::new(IntentNodeDraft::new(
                    IntentNodeKind::Geometry {
                        recipe: GeometryRecipeKind::SketchPoint,
                    },
                    key("retained.point.b"),
                )),
                cell: None,
            }],
        );
        let failed = session
            .plan_patch(failed_patch, |candidate| IntentEvaluation::Failed {
                failure: IntentEvaluationFailure {
                    kind: IntentEvaluationFailureKind::MaterializationRejected,
                    failed_nodes: BTreeSet::from([*candidate
                        .graph()
                        .nodes()
                        .keys()
                        .next_back()
                        .expect("retained node")]),
                    diagnostic: key("retained-intermediate"),
                },
            })
            .unwrap();
        session.commit_plan(failed).unwrap();
        commit_point(&mut session, "accepted-c", "accepted.point.c");

        let canonical_v2 = session.to_canonical_json().unwrap();
        let mut legacy: IntentSessionWire = serde_json::from_str(&canonical_v2).unwrap();
        let retained_checkpoint = legacy
            .undo
            .iter()
            .find(|entry| {
                entry
                    .checkpoint
                    .latest_attempt
                    .as_ref()
                    .is_some_and(|attempt| {
                        attempt.disposition == IntentAttemptDisposition::RetainedFailed
                    })
            })
            .expect("retained-failed checkpoint enters Undo");
        assert_ne!(
            retained_checkpoint.checkpoint.reservation_identity,
            retained_checkpoint
                .checkpoint
                .accepted
                .as_ref()
                .expect("prior accepted authority")
                .reservations
                .identity(),
            "the regression must exercise an intermediate reservation revision",
        );
        assert_ne!(
            retained_checkpoint.checkpoint.reservation_identity,
            legacy.reservations.identity(),
            "the regression must not collapse to the current reservation ledger",
        );

        rewrite_wire_as_legacy(&mut legacy);
        let canonical_v1 = serde_json::to_string(&legacy).unwrap();
        let migrated = IntentSession::from_json(&canonical_v1).unwrap();

        assert_eq!(migrated, session);
        assert_eq!(migrated.to_canonical_json().unwrap(), canonical_v2);
        assert!(
            migrated
                .history_projection()
                .applied
                .iter()
                .any(|entry| { entry.disposition == IntentPlanDisposition::RetainedFailed })
        );
    }

    #[test]
    fn canonical_v2_rejects_reauthenticated_intermediate_reservation_identity_tampering() {
        let mut session = session_with_retained_failure();
        commit_point(
            &mut session,
            "accepted-after-failure",
            "wire.accepted.after.failure",
        );
        let mut wire: IntentSessionWire =
            serde_json::from_str(&session.to_canonical_json().unwrap()).unwrap();
        let checkpoint = wire
            .undo
            .iter_mut()
            .find(|entry| {
                entry
                    .checkpoint
                    .latest_attempt
                    .as_ref()
                    .is_some_and(|attempt| {
                        attempt.disposition == IntentAttemptDisposition::RetainedFailed
                    })
            })
            .map(|entry| &mut entry.checkpoint)
            .expect("retained-failed checkpoint enters Undo");
        checkpoint.reservation_identity.0.digest = ContentDigest::zero();
        checkpoint
            .latest_attempt
            .as_mut()
            .expect("retained attempt")
            .target
            .reservations = checkpoint.reservation_identity;

        assert!(matches!(
            IntentSession::from_json(&authenticated_wire_json(wire)),
            Err(IntentSessionError::InvalidAuthority)
        ));
    }

    #[test]
    fn canonical_v2_rejects_reauthenticated_checkpoint_reservation_prefix_tampering() {
        let mut session = IntentSession::with_id(IntentSessionId::from_raw(0x8300_83f3)).unwrap();
        commit_point(&mut session, "accepted", "accepted.point");
        session.undo().unwrap().expect("point creation is undoable");
        session.redo().unwrap().expect("point creation is redoable");
        commit_retained_failure(
            &mut session,
            "retained",
            "retained.point",
            "retained-failure",
        );
        let canonical = session.to_canonical_json().unwrap();

        for forged_high_water in [
            None,
            Some(ReservationId::from_raw(1)),
            Some(session.allocator.next_reservation),
        ] {
            let mut wire: IntentSessionWire = serde_json::from_str(&canonical).unwrap();
            let checkpoint = wire
                .undo
                .iter_mut()
                .find(|entry| entry.checkpoint.accepted.is_none())
                .map(|entry| &mut entry.checkpoint)
                .expect("Undo retains the accepted-less empty checkpoint");
            assert_ne!(checkpoint.reservation_high_water, forged_high_water);
            checkpoint.reservation_high_water = forged_high_water;
            assert!(matches!(
                IntentSession::from_json(&authenticated_wire_json(wire)),
                Err(IntentSessionError::InvalidAuthority)
            ));
        }

        let mut current: IntentSessionWire = serde_json::from_str(&canonical).unwrap();
        current.current.reservation_high_water = Some(ReservationId::from_raw(2));
        let current_result = IntentSession::from_json(&authenticated_wire_json(current));
        assert!(
            matches!(current_result, Err(IntentSessionError::InvalidAuthority)),
            "unexpected current-prefix result: {current_result:?}"
        );

        let accepted_session = session_with_undo_and_redo();
        let live_high_water = Some(accepted_session.allocator.next_reservation);
        let mut accepted: IntentSessionWire =
            serde_json::from_str(&accepted_session.to_canonical_json().unwrap()).unwrap();
        let checkpoint = accepted
            .undo
            .iter_mut()
            .find(|entry| {
                entry.checkpoint.accepted.is_some()
                    && entry.checkpoint.reservation_high_water != live_high_water
            })
            .map(|entry| &mut entry.checkpoint)
            .expect("Undo retains an accepted checkpoint below live high-water");
        checkpoint.reservation_high_water = live_high_water;
        let accepted_result = IntentSession::from_json(&authenticated_wire_json(accepted));
        assert!(
            matches!(accepted_result, Err(IntentSessionError::InvalidAuthority)),
            "unexpected accepted-prefix result: {accepted_result:?}"
        );
    }

    #[test]
    fn canonical_v2_rejects_checkpoint_high_water_which_splits_a_reservation_pair() {
        let mut session = IntentSession::with_id(IntentSessionId::from_raw(0x8300_83f4)).unwrap();
        commit_point(&mut session, "base", "accepted.base");
        let point_alias = |node| PatchPortRef::Alias {
            node: key(node),
            selector: crate::IntentPortSelector::Node {
                role: crate::IntentPortRole::Primary,
                index: 0,
            },
        };
        let coincident = IntentNodeDraft::new(
            IntentNodeKind::Constraint {
                constraint: ConstraintKind::Coincident,
            },
            key("coincident"),
        )
        .with_input(InputSlot::new(InputRole::Point, 0), point_alias("left"))
        .with_input(InputSlot::new(InputRole::Point, 1), point_alias("right"));
        let patch = IntentPatch::new(
            session.identity(),
            IntentPatchPolicy::RequireAccepted,
            vec![
                IntentPatchOperation::CreateNode {
                    alias: key("left"),
                    draft: Box::new(IntentNodeDraft::new(
                        IntentNodeKind::Geometry {
                            recipe: GeometryRecipeKind::SketchPoint,
                        },
                        key("left"),
                    )),
                    cell: None,
                },
                IntentPatchOperation::CreateNode {
                    alias: key("right"),
                    draft: Box::new(IntentNodeDraft::new(
                        IntentNodeKind::Geometry {
                            recipe: GeometryRecipeKind::SketchPoint,
                        },
                        key("right"),
                    )),
                    cell: None,
                },
                IntentPatchOperation::CreateNode {
                    alias: key("coincident"),
                    draft: Box::new(coincident),
                    cell: None,
                },
            ],
        );
        let plan = session.plan_patch(patch, accepted).unwrap();
        let coincident = plan.aliases().node(&key("coincident")).unwrap();
        session.commit_plan(plan).unwrap();

        let pair = session
            .graph()
            .node(coincident)
            .unwrap()
            .reservations
            .values()
            .find_map(|reservation| {
                reservation
                    .paired_with
                    .map(|paired| (reservation.id, paired))
            })
            .expect("Coincident retains its constraint/source pair");
        let (lower, split_high_water) = if pair.0 < pair.1 {
            pair
        } else {
            (pair.1, pair.0)
        };
        assert_eq!(split_high_water.raw(), lower.raw() + 1);

        let mut wire: IntentSessionWire =
            serde_json::from_str(&session.to_canonical_json().unwrap()).unwrap();
        let checkpoint = wire
            .undo
            .iter_mut()
            .find(|entry| {
                entry.checkpoint.accepted.is_some()
                    && entry.checkpoint.reservation_high_water
                        != Some(session.allocator.next_reservation)
            })
            .map(|entry| &mut entry.checkpoint)
            .expect("Undo retains the accepted base checkpoint");
        checkpoint.reservation_high_water = Some(split_high_water);
        let result = IntentSession::from_json(&authenticated_wire_json(wire));
        assert!(
            matches!(result, Err(IntentSessionError::InvalidAuthority)),
            "unexpected pair-splitting result: {result:?}"
        );
    }

    #[test]
    fn canonical_v2_rejects_reauthenticated_permuted_history_checkpoint_bodies() {
        let session = session_with_undo_and_redo();
        let canonical = session.to_canonical_json().unwrap();

        let mut undo: IntentSessionWire = serde_json::from_str(&canonical).unwrap();
        let (first, second) = undo.undo.split_at_mut(1);
        std::mem::swap(&mut first[0].checkpoint, &mut second[0].checkpoint);
        assert!(matches!(
            IntentSession::from_json(&authenticated_wire_json(undo)),
            Err(IntentSessionError::InvalidHistoryTransition)
        ));

        let mut with_two_redo = session;
        with_two_redo
            .undo()
            .unwrap()
            .expect("second transaction is undoable");
        assert_eq!(with_two_redo.redo_len(), 2);
        let mut redo: IntentSessionWire =
            serde_json::from_str(&with_two_redo.to_canonical_json().unwrap()).unwrap();
        let (first, second) = redo.redo.split_at_mut(1);
        std::mem::swap(&mut first[0].checkpoint, &mut second[0].checkpoint);
        assert!(matches!(
            IntentSession::from_json(&authenticated_wire_json(redo)),
            Err(IntentSessionError::InvalidHistoryTransition)
        ));
    }

    #[test]
    fn canonical_v2_rejects_reauthenticated_descriptors_for_different_checkpoint_bodies() {
        let session = session_with_undo_and_redo();
        let canonical = session.to_canonical_json().unwrap();
        let mut wire: IntentSessionWire = serde_json::from_str(&canonical).unwrap();

        let (first, second) = wire.undo.split_at_mut(1);
        std::mem::swap(&mut first[0].checkpoint, &mut second[0].checkpoint);
        bind_history_edges(&wire.current, &mut wire.undo, &mut wire.redo);

        assert!(matches!(
            IntentSession::from_json(&authenticated_wire_json(wire)),
            Err(IntentSessionError::InvalidHistoryDescriptor)
        ));
    }

    #[test]
    fn canonical_v2_rejects_reauthenticated_relabelled_suppression_effect() {
        let mut session = IntentSession::with_id(IntentSessionId::from_raw(0x83_fd_00)).unwrap();
        commit_point(&mut session, "suppression-kind", "wire.suppression.kind");
        let point = *session.graph().nodes().keys().next().unwrap();
        let suppress = IntentPatch::new(
            session.identity(),
            IntentPatchPolicy::RequireAccepted,
            vec![IntentPatchOperation::SetSuppressed {
                node: point,
                suppressed: true,
            }],
        );
        let plan = session.plan_patch(suppress, accepted).unwrap();
        session.commit_plan(plan).unwrap();

        let mut wire: IntentSessionWire =
            serde_json::from_str(&session.to_canonical_json().unwrap()).unwrap();
        wire.undo
            .last_mut()
            .expect("suppression enters Undo")
            .descriptor
            .operation_kinds = vec![IntentPatchOperationKind::SetDefinitionField];

        assert!(matches!(
            IntentSession::from_json(&authenticated_wire_json(wire)),
            Err(IntentSessionError::InvalidHistoryDescriptor)
        ));
    }

    #[test]
    fn canonical_v2_rejects_reauthenticated_body_inconsistent_history_descriptor() {
        let mut session = IntentSession::with_id(IntentSessionId::from_raw(0x83_fd_01)).unwrap();
        commit_point(&mut session, "operation-kind", "wire.operation.kind");
        let point = *session.graph().nodes().keys().next().unwrap();
        let rename = IntentPatch::new(
            session.identity(),
            IntentPatchPolicy::RequireAccepted,
            vec![IntentPatchOperation::RenameNode {
                node: point,
                name: key("forged operation kind"),
            }],
        );
        let plan = session
            .plan_patch(rename, |_| {
                panic!("organization-only patch must not materialize")
            })
            .unwrap();
        session.commit_plan(plan).unwrap();

        let canonical = session.to_canonical_json().unwrap();

        let mut substituted: IntentSessionWire = serde_json::from_str(&canonical).unwrap();
        substituted
            .undo
            .last_mut()
            .expect("rename enters Undo")
            .descriptor
            .operation_kinds = vec![IntentPatchOperationKind::MoveDeclaration];
        assert!(matches!(
            IntentSession::from_json(&authenticated_wire_json(substituted)),
            Err(IntentSessionError::InvalidHistoryDescriptor)
        ));

        let mut added: IntentSessionWire = serde_json::from_str(&canonical).unwrap();
        let operation_kinds = &mut added
            .undo
            .last_mut()
            .expect("rename enters Undo")
            .descriptor
            .operation_kinds;
        operation_kinds.push(IntentPatchOperationKind::SetDefinitionField);
        operation_kinds.sort_unstable();
        assert!(matches!(
            IntentSession::from_json(&authenticated_wire_json(added)),
            Err(IntentSessionError::InvalidHistoryDescriptor)
        ));

        let mut incomplete: IntentSessionWire = serde_json::from_str(&canonical).unwrap();
        let descriptor = &mut incomplete
            .undo
            .last_mut()
            .expect("rename enters Undo")
            .descriptor;
        descriptor.diff.organization_nodes.clear();
        descriptor.affected_nodes = descriptor.diff.affected_nodes();
        assert!(matches!(
            IntentSession::from_json(&authenticated_wire_json(incomplete)),
            Err(IntentSessionError::InvalidHistoryDescriptor)
        ));
    }

    #[test]
    fn canonical_v2_allows_history_neutral_organization_only_evidence_refresh() {
        let mut session = IntentSession::with_id(IntentSessionId::from_raw(0x83_fd_02)).unwrap();
        commit_point(&mut session, "organization", "wire.organization.authority");
        let point = *session.graph().nodes().keys().next().unwrap();
        let rename = IntentPatch::new(
            session.identity(),
            IntentPatchPolicy::RequireAccepted,
            vec![IntentPatchOperation::RenameNode {
                node: point,
                name: key("organization-only authority"),
            }],
        );
        let plan = session
            .plan_patch(rename, |_| {
                panic!("organization-only patch must not materialize")
            })
            .unwrap();
        session.commit_plan(plan).unwrap();

        let history = session.history_projection();
        assert!(
            session
                .refresh_current_accepted_evidence(|candidate| IntentEvaluation::Accepted {
                    evidence: MaterializationEvidence::new_host_artifacts(
                        candidate.external_inputs().identity(),
                        b"refreshed materialization".to_vec(),
                        b"refreshed ownership".to_vec(),
                        b"refreshed validation".to_vec(),
                    )
                    .unwrap(),
                })
                .unwrap()
        );
        assert_eq!(session.history_projection(), history);
        let canonical = session.to_canonical_json().unwrap();
        assert_eq!(
            IntentSession::from_json(&canonical)
                .unwrap()
                .to_canonical_json()
                .unwrap(),
            canonical,
        );
    }

    #[test]
    fn canonical_v2_rejects_reauthenticated_retained_failure_accepted_evidence_rewrite() {
        let session = session_with_retained_failure();
        let mut wire: IntentSessionWire =
            serde_json::from_str(&session.to_canonical_json().unwrap()).unwrap();
        let current = wire
            .current
            .accepted
            .as_mut()
            .expect("prior accepted authority");
        current.evidence = MaterializationEvidence::new_host_artifacts(
            current.external_inputs.identity(),
            b"different retained materialization".to_vec(),
            b"different retained ownership".to_vec(),
            b"different retained validation".to_vec(),
        )
        .unwrap();

        assert!(matches!(
            IntentSession::from_json(&authenticated_wire_json(wire)),
            Err(IntentSessionError::InvalidHistoryTransition)
        ));
    }

    #[test]
    fn canonical_v2_rejects_reauthenticated_retained_failure_accepted_authority_rewrite() {
        let session = session_with_retained_failure();
        let mut wire: IntentSessionWire =
            serde_json::from_str(&session.to_canonical_json().unwrap()).unwrap();
        let accepted = wire
            .current
            .accepted
            .as_mut()
            .expect("prior accepted authority");
        accepted.reservations.revision =
            next_revision(accepted.reservations.revision).expect("test revision advances");
        accepted.target = semantic_identity(
            &accepted.graph,
            &accepted.instance,
            accepted.reservations.identity(),
            &accepted.external_inputs,
        );
        bind_history_edges(&wire.current, &mut wire.undo, &mut wire.redo);

        assert!(matches!(
            IntentSession::from_json(&authenticated_wire_json(wire)),
            Err(IntentSessionError::InvalidHistoryTransition)
        ));
    }

    #[test]
    fn canonical_v2_rejects_reauthenticated_nonchronological_history_stacks() {
        let session = session_with_undo_and_redo();
        let canonical = session.to_canonical_json().unwrap();

        let mut crossed: IntentSessionWire = serde_json::from_str(&canonical).unwrap();
        let undo_revision = crossed
            .undo
            .last()
            .expect("two applied transactions")
            .descriptor
            .target_revision;
        let redo_revision = crossed
            .redo
            .last()
            .expect("one redoable transaction")
            .descriptor
            .target_revision;
        crossed
            .undo
            .last_mut()
            .expect("two applied transactions")
            .descriptor
            .target_revision = redo_revision;
        crossed
            .redo
            .last_mut()
            .expect("one redoable transaction")
            .descriptor
            .target_revision = undo_revision;
        assert!(matches!(
            IntentSession::from_json(&authenticated_wire_json(crossed)),
            Err(IntentSessionError::InvalidHistoryDescriptor)
        ));

        let mut duplicate: IntentSessionWire = serde_json::from_str(&canonical).unwrap();
        let duplicate_revision = duplicate
            .undo
            .last()
            .expect("two applied transactions")
            .descriptor
            .target_revision;
        duplicate
            .redo
            .last_mut()
            .expect("one redoable transaction")
            .descriptor
            .target_revision = duplicate_revision;
        assert!(matches!(
            IntentSession::from_json(&authenticated_wire_json(duplicate)),
            Err(IntentSessionError::InvalidHistoryDescriptor)
        ));
    }

    #[test]
    fn history_transition_identity_survives_undo_redo_and_canonical_reload() {
        let mut session = session_with_undo_and_redo();
        let edge = {
            let entry = session.undo.last().expect("second applied transaction");
            (entry.before, entry.after, entry.descriptor.clone())
        };

        session
            .undo()
            .unwrap()
            .expect("second transaction is undoable");
        let redo = session.redo.last().expect("second transaction is redoable");
        assert_eq!((redo.before, redo.after, redo.descriptor.clone()), edge);
        let undone_json = session.to_canonical_json().unwrap();
        assert_eq!(
            IntentSession::from_json(&undone_json)
                .unwrap()
                .to_canonical_json()
                .unwrap(),
            undone_json
        );

        session
            .redo()
            .unwrap()
            .expect("second transaction is redoable");
        let applied = session.undo.last().expect("second transaction is applied");
        assert_eq!(
            (applied.before, applied.after, applied.descriptor.clone()),
            edge
        );
        let redone_json = session.to_canonical_json().unwrap();
        assert_eq!(
            IntentSession::from_json(&redone_json)
                .unwrap()
                .to_canonical_json()
                .unwrap(),
            redone_json
        );
    }

    #[test]
    fn legacy_v1_import_rejects_reauthenticated_inner_authority_tampering() {
        let session = session_with_undo_and_redo();
        let canonical_v2 = session.to_canonical_json().unwrap();

        let legacy_wire = || {
            let mut wire: IntentSessionWire = serde_json::from_str(&canonical_v2).unwrap();
            rewrite_wire_as_legacy(&mut wire);
            wire
        };
        let authenticated_legacy_json = |mut wire: IntentSessionWire| {
            wire.digest = legacy_session_wire_digest(&wire);
            serde_json::to_string(&wire).unwrap()
        };

        let mut target = legacy_wire();
        target
            .current
            .latest_attempt
            .as_mut()
            .unwrap()
            .target
            .graph
            .0
            .digest = ContentDigest::zero();
        assert!(matches!(
            IntentSession::from_json(&authenticated_legacy_json(target)),
            Err(IntentSessionError::InvalidAuthority)
        ));

        let mut evidence = legacy_wire();
        evidence
            .current
            .accepted
            .as_mut()
            .unwrap()
            .evidence
            .ownership
            .push(0x83);
        assert!(matches!(
            IntentSession::from_json(&authenticated_legacy_json(evidence)),
            Err(IntentSessionError::InvalidAuthority)
        ));

        let mut reservation = legacy_wire();
        reservation.current.reservation_identity.0.digest = ContentDigest::zero();
        assert!(matches!(
            IntentSession::from_json(&authenticated_legacy_json(reservation)),
            Err(IntentSessionError::InvalidAuthority)
        ));

        let mut invented_high_water = legacy_wire();
        invented_high_water.current.reservation_high_water =
            Some(invented_high_water.allocator.next_reservation);
        assert!(matches!(
            IntentSession::from_json(&authenticated_legacy_json(invented_high_water)),
            Err(IntentSessionError::InvalidAuthority)
        ));

        let mut retained = session_with_retained_failure();
        commit_point(
            &mut retained,
            "accepted-after-retained",
            "wire.accepted.after.retained",
        );
        let mut retained_wire: IntentSessionWire =
            serde_json::from_str(&retained.to_canonical_json().unwrap()).unwrap();
        rewrite_wire_as_legacy(&mut retained_wire);
        let checkpoint = retained_wire
            .undo
            .iter_mut()
            .find(|entry| {
                entry
                    .checkpoint
                    .latest_attempt
                    .as_ref()
                    .is_some_and(|attempt| {
                        attempt.disposition == IntentAttemptDisposition::RetainedFailed
                    })
            })
            .map(|entry| &mut entry.checkpoint)
            .expect("retained-failed checkpoint enters legacy Undo");
        checkpoint.reservation_identity.0.digest = ContentDigest::zero();
        checkpoint
            .latest_attempt
            .as_mut()
            .expect("retained attempt")
            .target
            .reservations = checkpoint.reservation_identity;
        assert!(matches!(
            IntentSession::from_json(&authenticated_legacy_json(retained_wire)),
            Err(IntentSessionError::InvalidAuthority)
        ));
    }

    #[test]
    fn cached_identity_queries_never_rehash_retained_history() {
        let mut session = IntentSession::with_id(IntentSessionId::from_raw(0x8300_83ca)).unwrap();
        for index in 0..32 {
            let symbol = format!("cached_point_{index:02}");
            commit_point(&mut session, &symbol, &symbol);
        }
        assert_eq!(session.undo_len(), 32);
        let expected = session.identity();

        SESSION_IDENTITY_HASH_COUNT.with(|count| count.set(0));
        for _ in 0..10_000 {
            assert_eq!(session.identity(), expected);
            assert_eq!(session.semantic_identity().graph, expected.graph);
        }
        SESSION_IDENTITY_HASH_COUNT.with(|count| assert_eq!(count.get(), 0));

        let canonical = session.to_canonical_json().unwrap();
        SESSION_IDENTITY_HASH_COUNT.with(|count| assert!(count.get() > 0));
        let restored = IntentSession::from_json(&canonical).unwrap();
        assert_eq!(restored.identity(), expected);
    }

    #[test]
    fn malformed_materializer_evidence_rejects_before_plan_publication() {
        let session = IntentSession::with_id(IntentSessionId::from_raw(0x8300_83e0)).unwrap();
        let patch = || {
            IntentPatch::new(
                session.identity(),
                IntentPatchPolicy::RequireAccepted,
                vec![IntentPatchOperation::CreateNode {
                    alias: key("evidence-point"),
                    draft: Box::new(IntentNodeDraft::new(
                        IntentNodeKind::Geometry {
                            recipe: GeometryRecipeKind::SketchPoint,
                        },
                        key("evidence.point"),
                    )),
                    cell: None,
                }],
            )
        };
        let identity = session.identity();

        let forged_digest = session.plan_patch(patch(), |candidate| {
            let mut evidence = MaterializationEvidence::new_host_artifacts(
                candidate.external_inputs().identity(),
                b"materialization".to_vec(),
                b"ownership".to_vec(),
                b"validation".to_vec(),
            )
            .unwrap();
            evidence.digest = ContentDigest::zero();
            IntentEvaluation::Accepted { evidence }
        });
        assert!(matches!(
            forged_digest,
            Err(IntentPlanError::Model(
                crate::IntentModelError::EvidenceDigestMismatch
            ))
        ));
        assert_eq!(session.identity(), identity);
        assert!(session.graph().nodes().is_empty());

        let oversized = session.plan_patch(patch(), |candidate| IntentEvaluation::Accepted {
            evidence: MaterializationEvidence {
                external_inputs: candidate.external_inputs().identity(),
                materialization: vec![0; crate::model::MAX_INTENT_OPAQUE_COMPONENT_BYTES + 1],
                ownership: Vec::new(),
                host_validation: Vec::new(),
                digest: ContentDigest::zero(),
            },
        });
        assert!(matches!(
            oversized,
            Err(IntentPlanError::Model(
                crate::IntentModelError::ComponentTooLarge {
                    component: "materialization",
                    limit: crate::model::MAX_INTENT_OPAQUE_COMPONENT_BYTES,
                }
            ))
        ));
        assert_eq!(session.identity(), identity);
        assert!(session.graph().nodes().is_empty());
    }

    #[test]
    fn canonical_session_input_shares_the_public_sixty_four_mib_envelope() {
        let hostile = " ".repeat(MAX_INTENT_SESSION_JSON_BYTES + 1);
        assert!(matches!(
            IntentSession::from_json(&hostile[..MAX_INTENT_SESSION_JSON_BYTES]),
            Err(IntentSessionError::Json(_))
        ));
        assert!(matches!(
            IntentSession::from_json(&hostile),
            Err(IntentSessionError::JsonResourceLimit {
                limit: MAX_INTENT_SESSION_JSON_BYTES
            })
        ));
    }

    #[test]
    fn oversized_decoded_components_reject_current_accepted_undo_and_redo_atomically() {
        let session = session_with_undo_and_redo();
        let canonical = session.to_canonical_json().unwrap();
        let identity = session.identity();
        let oversized = || vec![0_u8; crate::model::MAX_INTENT_OPAQUE_COMPONENT_BYTES + 1];

        let mut current: IntentSessionWire = serde_json::from_str(&canonical).unwrap();
        current.current.external_inputs.parameter_batch = oversized();
        assert_component_too_large(
            &IntentSession::from_json(&authenticated_wire_json(current)),
            "parameter batch",
        );
        assert_eq!(session.identity(), identity);
        assert_eq!(session.to_canonical_json().unwrap(), canonical);

        let mut accepted: IntentSessionWire = serde_json::from_str(&canonical).unwrap();
        let accepted_authority = accepted.current.accepted.as_mut().unwrap();
        accepted_authority.evidence.materialization = oversized();
        accepted_authority.evidence.reauthenticate();
        accepted
            .current
            .latest_attempt
            .as_mut()
            .unwrap()
            .materialization_digest = Some(accepted_authority.evidence.digest);
        assert_component_too_large(
            &validate_checkpoint(&accepted.current, &accepted.reservations, true),
            "materialization",
        );
        assert_eq!(session.identity(), identity);
        assert_eq!(session.to_canonical_json().unwrap(), canonical);

        let mut undo: IntentSessionWire = serde_json::from_str(&canonical).unwrap();
        undo.undo
            .last_mut()
            .unwrap()
            .checkpoint
            .external_inputs
            .parameter_batch = oversized();
        assert_component_too_large(
            &validate_checkpoint(
                &undo.undo.last().unwrap().checkpoint,
                &undo.reservations,
                false,
            ),
            "parameter batch",
        );
        assert_eq!(session.identity(), identity);
        assert_eq!(session.to_canonical_json().unwrap(), canonical);

        let mut redo: IntentSessionWire = serde_json::from_str(&canonical).unwrap();
        redo.redo
            .last_mut()
            .unwrap()
            .checkpoint
            .external_inputs
            .external_snapshots = oversized();
        assert_component_too_large(
            &validate_checkpoint(
                &redo.redo.last().unwrap().checkpoint,
                &redo.reservations,
                false,
            ),
            "external snapshots",
        );
        assert_eq!(session.identity(), identity);
        assert_eq!(session.to_canonical_json().unwrap(), canonical);

        let mut forged_undo = session.clone();
        forged_undo
            .undo
            .last_mut()
            .unwrap()
            .checkpoint
            .external_inputs
            .parameter_batch = oversized();
        let before_undo = forged_undo.clone();
        assert!(matches!(
            forged_undo.undo(),
            Err(IntentSessionError::Model(
                crate::IntentModelError::ComponentTooLarge {
                    component: "parameter batch",
                    limit: crate::model::MAX_INTENT_OPAQUE_COMPONENT_BYTES,
                }
            ))
        ));
        assert_eq!(forged_undo, before_undo);

        let mut forged_redo = session.clone();
        forged_redo
            .redo
            .last_mut()
            .unwrap()
            .checkpoint
            .external_inputs
            .external_snapshots = oversized();
        let before_redo = forged_redo.clone();
        assert!(matches!(
            forged_redo.redo(),
            Err(IntentSessionError::Model(
                crate::IntentModelError::ComponentTooLarge {
                    component: "external snapshots",
                    limit: crate::model::MAX_INTENT_OPAQUE_COMPONENT_BYTES,
                }
            ))
        ));
        assert_eq!(forged_redo, before_redo);
    }

    #[test]
    fn retained_failure_authority_maintenance_crosses_the_complete_redo_prefix() {
        let mut session = IntentSession::with_id(IntentSessionId::from_raw(0x83_fd_04)).unwrap();
        commit_point(&mut session, "accepted", "wire.accepted");
        commit_retained_failure(
            &mut session,
            "retained-a",
            "wire.retained.a",
            "retained-failure-a",
        );
        commit_retained_failure(
            &mut session,
            "retained-b",
            "wire.retained.b",
            "retained-failure-b",
        );

        session.undo().unwrap().expect("second failure is undoable");
        session = IntentSession::from_json(&session.to_canonical_json().unwrap()).unwrap();
        session.undo().unwrap().expect("first failure is undoable");
        session = IntentSession::from_json(&session.to_canonical_json().unwrap()).unwrap();
        let maintained = session
            .accepted()
            .expect("accepted authority is restored")
            .clone();

        session.redo().unwrap().expect("first failure is redoable");
        session = IntentSession::from_json(&session.to_canonical_json().unwrap()).unwrap();
        assert_eq!(session.accepted(), Some(&maintained));
        session.redo().unwrap().expect("second failure is redoable");
        session = IntentSession::from_json(&session.to_canonical_json().unwrap()).unwrap();
        assert_eq!(session.accepted(), Some(&maintained));
        assert_eq!(
            session.latest_attempt().unwrap().disposition,
            IntentAttemptDisposition::RetainedFailed
        );
    }

    #[test]
    fn accepted_evidence_refresh_crosses_a_redoable_retained_failure() {
        let mut session = session_with_retained_failure();
        session
            .undo()
            .unwrap()
            .expect("retained failure is undoable");
        let refreshed = MaterializationEvidence::new_host_artifacts(
            session.external_inputs().identity(),
            b"refreshed materialization".to_vec(),
            b"refreshed ownership".to_vec(),
            b"refreshed validation".to_vec(),
        )
        .unwrap();
        assert!(
            session
                .refresh_current_accepted_evidence(|_| IntentEvaluation::Accepted {
                    evidence: refreshed.clone(),
                })
                .unwrap()
        );
        session = IntentSession::from_json(&session.to_canonical_json().unwrap()).unwrap();
        assert_eq!(session.accepted().unwrap().evidence, refreshed);

        session
            .redo()
            .unwrap()
            .expect("retained failure is redoable");
        session = IntentSession::from_json(&session.to_canonical_json().unwrap()).unwrap();
        assert_eq!(session.accepted().unwrap().evidence, refreshed);
        assert_eq!(
            session.latest_attempt().unwrap().disposition,
            IntentAttemptDisposition::RetainedFailed
        );
    }

    #[test]
    fn retained_failure_after_accepted_undo_redo_keeps_every_checkpoint_authoritative() {
        let mut session = IntentSession::with_id(IntentSessionId::from_raw(0x83_fd_05)).unwrap();
        commit_point(&mut session, "accepted", "wire.accepted");
        session.undo().unwrap().expect("point creation is undoable");
        session.redo().unwrap().expect("point creation is redoable");
        commit_retained_failure(
            &mut session,
            "retained",
            "wire.retained",
            "retained-failure",
        );

        let initial = &session.undo[0].checkpoint;
        assert!(initial.accepted.is_none());
        assert_eq!(
            initial.reservation_high_water,
            Some(ReservationId::from_raw(2))
        );
        assert_eq!(
            session.allocator.next_reservation,
            ReservationId::from_raw(3)
        );

        let current = session.checkpoint();
        validate_checkpoint(&current, &session.reservations, true)
            .unwrap_or_else(|error| panic!("Current checkpoint: {error:?}"));
        for (index, entry) in session.undo.iter().enumerate() {
            validate_checkpoint(&entry.checkpoint, &session.reservations, false)
                .unwrap_or_else(|error| panic!("Undo checkpoint {index}: {error:?}"));
        }
        assert!(session.redo.is_empty());
        session.validate().unwrap();
        let canonical = session.to_canonical_json().unwrap();
        assert_eq!(
            IntentSession::from_json(&canonical)
                .unwrap()
                .to_canonical_json()
                .unwrap(),
            canonical
        );
    }

    #[test]
    fn nested_accepted_graph_and_instance_are_validated_in_current_undo_and_redo() {
        let current = session_with_retained_failure();
        let current_json = current.to_canonical_json().unwrap();

        for corrupt in [
            corrupt_accepted_graph as fn(&mut SessionCheckpoint),
            corrupt_accepted_instance,
        ] {
            let mut wire: IntentSessionWire = serde_json::from_str(&current_json).unwrap();
            corrupt(&mut wire.current);
            assert!(
                IntentSession::from_json(&authenticated_wire_json(wire)).is_err(),
                "malformed nested Current accepted state must reject"
            );
        }

        let mut with_undo = current.clone();
        commit_point(
            &mut with_undo,
            "accepted-after-failure",
            "wire.accepted.after.failure",
        );
        let undo_json = with_undo.to_canonical_json().unwrap();
        for corrupt in [
            corrupt_accepted_graph as fn(&mut SessionCheckpoint),
            corrupt_accepted_instance,
        ] {
            let mut wire: IntentSessionWire = serde_json::from_str(&undo_json).unwrap();
            let retained = wire
                .undo
                .iter_mut()
                .find(|entry| {
                    entry
                        .checkpoint
                        .latest_attempt
                        .as_ref()
                        .is_some_and(|attempt| {
                            attempt.disposition == IntentAttemptDisposition::RetainedFailed
                        })
                })
                .expect("retained-failed Undo checkpoint");
            corrupt(&mut retained.checkpoint);
            assert!(
                IntentSession::from_json(&authenticated_wire_json(wire)).is_err(),
                "malformed nested Undo accepted state must reject"
            );
        }

        let mut with_redo = current;
        with_redo
            .undo()
            .unwrap()
            .expect("retained failure is undoable");
        let redo_json = with_redo.to_canonical_json().unwrap();
        for corrupt in [
            corrupt_accepted_graph as fn(&mut SessionCheckpoint),
            corrupt_accepted_instance,
        ] {
            let mut wire: IntentSessionWire = serde_json::from_str(&redo_json).unwrap();
            let retained = wire
                .redo
                .iter_mut()
                .find(|entry| {
                    entry
                        .checkpoint
                        .latest_attempt
                        .as_ref()
                        .is_some_and(|attempt| {
                            attempt.disposition == IntentAttemptDisposition::RetainedFailed
                        })
                })
                .expect("retained-failed Redo checkpoint");
            corrupt(&mut retained.checkpoint);
            assert!(
                IntentSession::from_json(&authenticated_wire_json(wire)).is_err(),
                "malformed nested Redo accepted state must reject"
            );
        }
    }
}
