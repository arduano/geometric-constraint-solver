// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::graph::{
    AllocatedDraft, IntentAllocatorHighWater, IntentGraph, IntentGraphError,
    IntentReservationLedger, allocate_draft, allocated_alias_ports, assign_identity_generations,
    finish_allocated_draft,
};
use crate::ids::{
    CellId, ContentDigest, IntentKey, IntentSessionId, NodeId, PlanToken, Revision, digest_bytes,
};
use crate::model::{
    IntentExternalInputs, IntentExternalInputsIdentity, IntentInstanceState, IntentOrganization,
    IntentReservationLedgerIdentity, IntentSemanticIdentity, MaterializationEvidence,
    OrganizationCell, PatchPortRef, literal_matches_leaf, next_revision,
};
use crate::patch::{
    CellTarget, DeletePolicy, IntentAliasMap, IntentPatch, IntentPatchOperation,
    IntentPatchOperationKind, IntentPatchPolicy, IntentSemanticDiff,
};

/// Strict canonical intent-session wire version.
pub const INTENT_SESSION_VERSION: u32 = 1;
/// User-visible Undo/Redo bound.
pub const MAX_INTENT_HISTORY_ENTRIES: usize = 1_024;
/// Maximum strict canonical session JSON, including bounded history.
pub const MAX_INTENT_SESSION_JSON_BYTES: usize = 256 * 1024 * 1024;
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
            || !self
                .operation_kinds
                .windows(2)
                .all(|pair| pair[0] <= pair[1])
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
}

/// Retained design-intent authority and bounded transaction history.
#[derive(Clone, Debug, PartialEq)]
pub struct IntentSession {
    id: IntentSessionId,
    revision: Revision,
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
        Ok(Self {
            id,
            revision: Revision::from_raw(0),
            graph: IntentGraph::empty(),
            instance: IntentInstanceState::empty(),
            reservations: IntentReservationLedger::empty(),
            organization: IntentOrganization::new(CellId::from_raw(1), sketch),
            external_inputs: IntentExternalInputs::default(),
            latest_attempt: None,
            accepted: None,
            undo: Vec::new(),
            redo: Vec::new(),
            allocator: IntentAllocatorHighWater::initial(),
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
    pub fn semantic_identity(&self) -> IntentSemanticIdentity {
        semantic_identity(
            &self.graph,
            &self.instance,
            self.reservations.identity(),
            &self.external_inputs,
        )
    }

    #[must_use]
    pub fn identity(&self) -> IntentSessionIdentity {
        session_identity(
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
        )
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
        let operation_kinds = patch
            .operations()
            .iter()
            .map(IntentPatchOperation::kind)
            .collect::<Vec<_>>();
        let mut staged = self.checkpoint();
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
        let descriptor =
            IntentTransactionDescriptor::new(revision, disposition, operation_kinds, diff.clone());
        descriptor.validate()?;
        let mut undo = self.undo.clone();
        push_bounded(
            &mut undo,
            HistoryEntry {
                checkpoint: self.checkpoint(),
                descriptor: descriptor.clone(),
            },
        );
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
        let actual = staged.identity();
        if actual != plan.target {
            return Err(IntentSessionError::InvalidPlanToken);
        }
        *self = staged;
        Ok(actual)
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
        let current = staged.checkpoint();
        push_bounded(
            &mut staged.redo,
            HistoryEntry {
                checkpoint: current,
                descriptor: entry.descriptor,
            },
        );
        staged.restore_history_checkpoint(entry.checkpoint)?;
        staged.revision =
            next_revision(staged.revision).ok_or(IntentSessionError::RevisionExhausted)?;
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
        let current = staged.checkpoint();
        push_bounded(
            &mut staged.undo,
            HistoryEntry {
                checkpoint: current,
                descriptor: entry.descriptor,
            },
        );
        staged.restore_history_checkpoint(entry.checkpoint)?;
        staged.revision =
            next_revision(staged.revision).ok_or(IntentSessionError::RevisionExhausted)?;
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

    /// Imports strict canonical intent-session JSON and authenticates all
    /// current/history structure and high-water state.
    ///
    /// # Errors
    ///
    /// Returns an error for excessive, malformed, unsupported, digest-
    /// mismatched, or structurally invalid input.
    pub fn from_json(json: &str) -> Result<Self, IntentSessionError> {
        if json.len() > MAX_INTENT_SESSION_JSON_BYTES {
            return Err(IntentSessionError::JsonResourceLimit {
                limit: MAX_INTENT_SESSION_JSON_BYTES,
            });
        }
        let wire: IntentSessionWire = serde_json::from_str(json)?;
        if wire.version != INTENT_SESSION_VERSION {
            return Err(IntentSessionError::UnsupportedVersion {
                expected: INTENT_SESSION_VERSION,
                actual: wire.version,
            });
        }
        if wire.digest != session_wire_digest(&wire) {
            return Err(IntentSessionError::DigestMismatch);
        }
        if serde_json::to_string(&wire)? != json {
            return Err(IntentSessionError::NonCanonicalJson);
        }
        if wire.current.reservation_identity != wire.reservations.identity() {
            return Err(IntentSessionError::InvalidAuthority);
        }
        let session = Self {
            id: wire.id,
            revision: wire.revision,
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
        {
            return Err(IntentSessionError::InvalidHistoryDescriptor);
        }
        self.reservations.validate_against_graph(&self.graph)?;
        validate_checkpoint(&self.checkpoint(), &self.reservations)?;
        for entry in self.undo.iter().chain(&self.redo) {
            entry.descriptor.validate()?;
            validate_checkpoint(&entry.checkpoint, &self.reservations)?;
        }
        validate_allocator(
            self.allocator,
            std::iter::once(&self.checkpoint())
                .chain(self.undo.iter().map(|entry| &entry.checkpoint))
                .chain(self.redo.iter().map(|entry| &entry.checkpoint)),
            &self.reservations,
        )?;
        Ok(())
    }
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

    let mut create_nodes = create_nodes.into_iter().collect::<Vec<_>>();
    create_nodes.sort_by(|(left_alias, (left, _)), (right_alias, (right, _))| {
        left.0
            .symbol
            .cmp(&right.0.symbol)
            .then_with(|| left_alias.cmp(right_alias))
    });
    let mut allocated = Vec::<(AllocatedDraft, Option<CellTarget>)>::new();
    for (alias, (draft, cell)) in create_nodes {
        let value = allocate_draft(draft.0, allocator)?;
        aliases.nodes.insert(alias.clone(), value.id);
        for (selector, port) in allocated_alias_ports(&value) {
            aliases
                .ports
                .entry(alias.clone())
                .or_default()
                .insert(selector, port);
        }
        allocated.push((value, cell));
    }
    for (value, cell) in allocated {
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

    for operation in operations {
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
            IntentPatchOperation::DeleteNode { node, policy } => {
                let closure = staged.graph.dependent_closure([node])?;
                match policy {
                    DeletePolicy::RejectDependents if closure.len() != 1 => {
                        return Err(IntentPlanError::DeleteHasDependents { node, closure });
                    }
                    DeletePolicy::Cascade { exact_nodes } if exact_nodes != closure => {
                        return Err(IntentPlanError::CascadeMismatch {
                            expected: closure,
                            actual: exact_nodes,
                        });
                    }
                    DeletePolicy::RejectDependents | DeletePolicy::Cascade { .. } => {}
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
            IntentPatchOperation::SetDefinitionField { node, field, .. } => {
                (format!("field:{node}:{}", field.0), Some(*node))
            }
            IntentPatchOperation::SetInstanceLeaf { leaf, .. } => {
                (format!("leaf:{leaf:?}"), Some(leaf.node))
            }
            IntentPatchOperation::RebindInput { node, slot, .. } => {
                (format!("input:{node}:{slot:?}"), Some(*node))
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
        | IntentPatchOperation::RebindInput { node, .. }
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
) -> Result<(), IntentSessionError> {
    checkpoint.graph.validate()?;
    reservations.validate_graph_metadata(&checkpoint.graph)?;
    if checkpoint.reservation_identity.0.revision > reservations.revision() {
        return Err(IntentSessionError::InvalidAuthority);
    }
    validate_organization(&checkpoint.graph, &checkpoint.organization)?;
    for (leaf, value) in checkpoint.instance.values() {
        checkpoint.graph.writable_leaf(*leaf)?;
        if !literal_matches_leaf(value, leaf.field) {
            return Err(IntentSessionError::InvalidInstance);
        }
        value.validate()?;
    }
    let semantic = semantic_identity(
        &checkpoint.graph,
        &checkpoint.instance,
        checkpoint.reservation_identity,
        &checkpoint.external_inputs,
    );
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
    if let Some(accepted) = &checkpoint.accepted
        && (accepted.graph.identity() != accepted.target.graph
            || accepted.instance.identity() != accepted.target.instance
            || accepted.reservations.identity() != accepted.target.reservations
            || accepted.external_inputs.identity() != accepted.target.external_inputs
            || accepted.external_inputs.identity() != accepted.evidence.external_inputs
            || accepted.target.external_inputs != accepted.evidence.external_inputs
            || accepted.evidence.digest
                != crate::intent_content_digest(
                    &serde_json::to_vec(&(
                        accepted.evidence.external_inputs,
                        &accepted.evidence.materialization,
                        &accepted.evidence.ownership,
                        &accepted.evidence.host_validation,
                    ))
                    .expect("accepted evidence is infallibly serializable"),
                ))
    {
        return Err(IntentSessionError::InvalidAuthority);
    }
    if let Some(accepted) = &checkpoint.accepted {
        accepted
            .reservations
            .validate_against_graph(&accepted.graph)?;
        reservations.validate_superset_of(&accepted.reservations)?;
    }
    Ok(())
}

fn validate_allocator<'a>(
    allocator: IntentAllocatorHighWater,
    checkpoints: impl Iterator<Item = &'a SessionCheckpoint>,
    reservations: &IntentReservationLedger,
) -> Result<(), IntentSessionError> {
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
    #[error("intent session JSON exceeds {limit} bytes")]
    JsonResourceLimit { limit: usize },
    #[error("unsupported intent session version {actual}; expected {expected}")]
    UnsupportedVersion { expected: u32, actual: u32 },
    #[error("intent session digest mismatch")]
    DigestMismatch,
    #[error("intent session JSON is not canonical")]
    NonCanonicalJson,
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
        GeometryRecipeKind, IntentNodeDraft, IntentNodeKind, IntentPatchOperationKind,
        IntentReservationState,
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
        let template = session.undo.last().unwrap().clone();

        for offset in 0..MAX_INTENT_HISTORY_ENTRIES {
            let mut entry = template.clone();
            entry.descriptor.target_revision = Revision::from_raw(3 + offset as u64);
            push_bounded(&mut session.undo, entry);
        }
        session.revision = Revision::from_raw(2 + MAX_INTENT_HISTORY_ENTRIES as u64);

        assert_eq!(session.undo.len(), MAX_INTENT_HISTORY_ENTRIES);
        assert!(session.undo.iter().all(|entry| {
            entry.descriptor.operation_kinds == vec![IntentPatchOperationKind::DeleteNode]
        }));
        assert_eq!(
            session.reservations.entries()[&reservation].state,
            IntentReservationState::Tombstoned
        );
        assert!(reservation.raw() < session.allocator.next_reservation.raw());
        session.validate().unwrap();
    }
}
