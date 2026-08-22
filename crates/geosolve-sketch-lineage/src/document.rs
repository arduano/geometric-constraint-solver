// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

use crate::ids::{
    LineageDigest, LineageDocumentId, LineageDocumentIdentity, LineageIdParseError,
    LineageKeyError, LineageOutputId, LineageReservationId, LineageRevision, LineageStepId,
};
use crate::model::{
    ImportedBaselineEncoding, LineageActionDefinition, LineageActionKind,
    LineageOutputIdentityFlow, LineageOutputKind, LineageOutputRef, LineageStep, LineageStepState,
    reservation_matches_output,
};

/// Current canonical sketch-lineage wire version.
pub const LINEAGE_DOCUMENT_VERSION: u32 = 1;
/// Maximum accepted canonical lineage JSON size.
pub const MAX_LINEAGE_JSON_BYTES: usize = 16 * 1024 * 1024;
/// Maximum retained lineage steps, including tombstones.
pub const MAX_LINEAGE_STEPS: usize = 100_000;
/// Maximum logical output ports across all steps.
pub const MAX_LINEAGE_OUTPUTS: usize = 500_000;
/// Maximum materialized reservations across all steps.
pub const MAX_LINEAGE_RESERVATIONS: usize = 1_000_000;
/// Maximum action input bindings across all steps.
pub const MAX_LINEAGE_ACTION_INPUTS: usize = 1_000_000;
/// Maximum explicit writable-leaf declarations across all retained steps.
pub const MAX_LINEAGE_WRITABLE_LEAVES: usize = 2_000_000;
/// Maximum imported-baseline payload size.
pub const MAX_LINEAGE_BASELINE_BYTES: usize = 12 * 1024 * 1024;
/// Maximum serialized generic parameters for one action.
pub const MAX_LINEAGE_ACTION_PAYLOAD_BYTES: usize = 2 * 1024 * 1024;
/// Maximum nodes across all generic JSON action parameters.
pub const MAX_LINEAGE_PAYLOAD_NODES: usize = 1_000_000;
/// Maximum nesting depth in generic JSON action parameters.
pub const MAX_LINEAGE_PAYLOAD_DEPTH: usize = 64;
/// Maximum label size retained with one step.
pub const MAX_LINEAGE_LABEL_BYTES: usize = 1_024;

static LINEAGE_DOCUMENT_NONCE: AtomicU64 = AtomicU64::new(1);

/// Stable allocator cursors. IDs below these cursors are never reissued.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LineageAllocatorHighWater {
    pub next_step_id: LineageStepId,
    pub next_output_id: LineageOutputId,
    pub next_reservation_id: LineageReservationId,
}

/// How a materializer handles a failed or blocked chronological step.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LineageEvaluationPolicy {
    /// Stop all later live evaluation after the first failure/block.
    #[default]
    StrictChronological,
    /// Continue later live steps whose dependencies remain available.
    DependencyLocal,
}

/// Pre-materialization eligibility of one retained lineage step.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LineageStepEvaluationState {
    Ready,
    Suppressed,
    Tombstoned,
    Failed,
    Blocked { dependencies: Vec<LineageStepId> },
    Unevaluated,
}

/// One deterministic entry in a dependency/evaluation plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LineageDependencyPlanEntry {
    pub step: LineageStepId,
    pub state: LineageStepEvaluationState,
}

/// Replacement body for an existing step. Identity and ownership manifests do
/// not change during an ordinary rewrite.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LineageStepRewrite {
    pub label: String,
    pub action: LineageActionDefinition,
}

/// One mutation inside an exact-CAS atomic lineage patch.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum LineageMutation {
    Insert {
        before: Option<LineageStepId>,
        step: Box<LineageStep>,
    },
    Rewrite {
        step: LineageStepId,
        replacement: Box<LineageStepRewrite>,
    },
    Tombstone {
        step: LineageStepId,
    },
    DeleteSubtree {
        root: LineageStepId,
    },
    SetSuppressed {
        step: LineageStepId,
        suppressed: bool,
    },
    Reorder {
        step: LineageStepId,
        before: Option<LineageStepId>,
    },
    Rebind {
        step: LineageStepId,
        input: crate::LineageSemanticKey,
        target: LineageOutputRef,
    },
    SetEvaluationPolicy {
        policy: LineageEvaluationPolicy,
    },
}

/// Atomic exact-revision and exact-digest mutation request.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LineagePatch {
    pub expected: LineageDocumentIdentity,
    pub mutations: Vec<LineageMutation>,
}

impl LineagePatch {
    /// Creates an exact-CAS patch against a current document identity.
    #[must_use]
    pub fn new(
        expected: LineageDocumentIdentity,
        mutations: impl Into<Vec<LineageMutation>>,
    ) -> Self {
        Self {
            expected,
            mutations: mutations.into(),
        }
    }
}

/// Result of one structurally accepted atomic patch.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LineagePatchOutcome {
    pub identity: LineageDocumentIdentity,
    pub changed: bool,
    pub inserted_steps: Vec<LineageStepId>,
    pub tombstoned_steps: Vec<LineageStepId>,
}

/// Strict persistence, validation, or mutation failure.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum LineageDocumentError {
    #[error(transparent)]
    InvalidId(#[from] LineageIdParseError),
    #[error(transparent)]
    InvalidKey(#[from] LineageKeyError),
    #[error("unsupported lineage document version {actual}; expected {expected}")]
    UnsupportedVersion { actual: u32, expected: u32 },
    #[error("unsupported lineage session version {actual}; expected {expected}")]
    UnsupportedSessionVersion { actual: u32, expected: u32 },
    #[error("lineage JSON exceeds {limit} bytes")]
    JsonResourceLimit { limit: usize },
    #[error("lineage session JSON exceeds {limit} bytes")]
    SessionJsonResourceLimit { limit: usize },
    #[error("lineage resource limit exceeded for {resource}: {actual} > {limit}")]
    ResourceLimit {
        resource: &'static str,
        actual: usize,
        limit: usize,
    },
    #[error("invalid lineage field `{field}`: {message}")]
    InvalidField {
        field: &'static str,
        message: &'static str,
    },
    #[error("duplicate lineage step ID {0}")]
    DuplicateStep(LineageStepId),
    #[error("duplicate lineage output ID {0}")]
    DuplicateOutput(LineageOutputId),
    #[error("duplicate lineage reservation ID {0}")]
    DuplicateReservation(LineageReservationId),
    #[error("duplicate lineage developer key `{0}`")]
    DuplicateDeveloperKey(String),
    #[error("duplicate semantic key `{key}` in step {step}")]
    DuplicateSemanticKey { step: LineageStepId, key: String },
    #[error("duplicate materialized reservation `{persistent_id}`")]
    DuplicatePersistentReservation { persistent_id: String },
    #[error("unknown lineage step {0}")]
    UnknownStep(LineageStepId),
    #[error("unknown lineage output {output} on step {step}")]
    UnknownOutput {
        step: LineageStepId,
        output: LineageOutputId,
    },
    #[error("step {step} has no identity-flow declaration for output {output}")]
    MissingOutputIdentity {
        step: LineageStepId,
        output: LineageOutputId,
    },
    #[error("step {step} declares identity flow more than once for output {output}")]
    DuplicateOutputIdentity {
        step: LineageStepId,
        output: LineageOutputId,
    },
    #[error("step {step} declares writable leaf `{key}` more than once for output {output}")]
    DuplicateWritableLeaf {
        step: LineageStepId,
        output: LineageOutputId,
        key: String,
    },
    #[error("invalid writable leaf `{key}` for output {output} on step {step}: {message}")]
    InvalidWritableLeaf {
        step: LineageStepId,
        output: LineageOutputId,
        key: String,
        message: &'static str,
    },
    #[error("invalid identity flow for output {output} on step {step}: {message}")]
    InvalidOutputIdentity {
        step: LineageStepId,
        output: LineageOutputId,
        message: &'static str,
    },
    #[error("step {step} references retired lineage output {output} on step {provider}")]
    RetiredOutputReference {
        step: LineageStepId,
        provider: LineageStepId,
        output: LineageOutputId,
    },
    #[error(
        "step {step} references consumed lineage output {output} on step {provider}; current successor is {successor:?}"
    )]
    ConsumedOutputReference {
        step: LineageStepId,
        provider: LineageStepId,
        output: LineageOutputId,
        successor: LineageOutputRef,
    },
    #[error("live step {dependent} still depends on tombstoned step {deleted}")]
    LiveDependent {
        deleted: LineageStepId,
        dependent: LineageStepId,
    },
    #[error("unknown lineage reservation {reservation} on step {step}")]
    UnknownReservation {
        step: LineageStepId,
        reservation: LineageReservationId,
    },
    #[error("step {step} references lineage document {actual}, expected {expected}")]
    CrossDocumentReference {
        step: LineageStepId,
        expected: LineageDocumentId,
        actual: LineageDocumentId,
    },
    #[error("step {step} expects {expected:?}, but output is {actual:?}")]
    WrongOutputKind {
        step: LineageStepId,
        expected: LineageOutputKind,
        actual: LineageOutputKind,
    },
    #[error("step {step} has a forward reference to step {dependency}")]
    ForwardReference {
        step: LineageStepId,
        dependency: LineageStepId,
    },
    #[error("lineage dependency cycle includes step {step}")]
    DependencyCycle { step: LineageStepId },
    #[error("imported baseline step {step} must be the unique first step")]
    InvalidBaselinePosition { step: LineageStepId },
    #[error("reservation {reservation} on step {step} cannot back output kind {output:?}")]
    ReservationKindMismatch {
        step: LineageStepId,
        reservation: LineageReservationId,
        output: LineageOutputKind,
    },
    #[error("unexpected {allocator} allocation: expected {expected}, received {actual}")]
    UnexpectedAllocation {
        allocator: &'static str,
        expected: String,
        actual: String,
    },
    #[error("lineage identity allocator is exhausted")]
    IdExhausted,
    #[error("lineage revision is exhausted")]
    RevisionExhausted,
    #[error("replacement lineage revision must be exactly {expected}; received {actual}")]
    NonSequentialReplacementRevision {
        expected: LineageRevision,
        actual: LineageRevision,
    },
    #[error(
        "replacement {allocator} cursor regresses below retained high-water {minimum}: {actual}"
    )]
    AllocatorRegression {
        allocator: &'static str,
        minimum: String,
        actual: String,
    },
    #[error("lineage {identity} `{value}` is rebound across retained session history")]
    CrossHistoryIdentityRebinding {
        identity: &'static str,
        value: String,
    },
    #[error("lineage patch targets document {actual}, expected {expected}")]
    WrongPatchDocument {
        expected: LineageDocumentId,
        actual: LineageDocumentId,
    },
    #[error(
        "stale lineage patch: expected revision {expected_revision}/{expected_digest}, current is {actual_revision}/{actual_digest}"
    )]
    StalePatch {
        expected_revision: LineageRevision,
        expected_digest: LineageDigest,
        actual_revision: LineageRevision,
        actual_digest: LineageDigest,
    },
    #[error("lineage patch must contain at least one mutation")]
    EmptyPatch,
    #[error("step {step} is tombstoned and cannot be edited")]
    TombstonedStep { step: LineageStepId },
    #[error("step {step} does not have input `{input}`")]
    UnknownInput { step: LineageStepId, input: String },
    #[error("input `{input}` on step {step} expects {expected:?}, received {actual:?}")]
    RebindKindMismatch {
        step: LineageStepId,
        input: String,
        expected: LineageOutputKind,
        actual: LineageOutputKind,
    },
    #[error("failed-step evidence names unavailable step {step}")]
    InvalidFailedStep { step: LineageStepId },
    #[error("invalid lineage evaluation attempt: {message}")]
    InvalidEvaluationAttempt { message: &'static str },
    #[error("invalid lineage session authority: {message}")]
    InvalidSessionAuthority { message: &'static str },
    #[error("lineage payload digest does not match its canonical content")]
    DigestMismatch,
    #[error("invalid lineage JSON: {0}")]
    Json(#[from] serde_json::Error),
}

/// Separately versioned authoritative lineage intent.
#[derive(Clone, Debug, PartialEq)]
pub struct LineageDocument {
    id: LineageDocumentId,
    revision: LineageRevision,
    next_step_id: LineageStepId,
    next_output_id: LineageOutputId,
    next_reservation_id: LineageReservationId,
    evaluation_policy: LineageEvaluationPolicy,
    steps: Vec<LineageStep>,
}

#[derive(Clone, Copy, Debug)]
struct IdentityPortVersion {
    token: LineageOutputRef,
    generation: usize,
}

#[derive(Clone, Copy, Debug)]
enum IdentityTokenState {
    Current {
        generation: usize,
        successor: LineageOutputRef,
    },
    Retired,
}

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
struct LineageCanonicalPayload<'a> {
    version: u32,
    document_id: LineageDocumentId,
    revision: LineageRevision,
    next_step_id: LineageStepId,
    next_output_id: LineageOutputId,
    next_reservation_id: LineageReservationId,
    evaluation_policy: LineageEvaluationPolicy,
    steps: &'a [LineageStep],
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct LineageWireV1 {
    version: u32,
    document_id: LineageDocumentId,
    revision: LineageRevision,
    next_step_id: LineageStepId,
    next_output_id: LineageOutputId,
    next_reservation_id: LineageReservationId,
    evaluation_policy: LineageEvaluationPolicy,
    steps: Vec<LineageStep>,
    digest: LineageDigest,
}

#[allow(
    clippy::missing_errors_doc,
    reason = "all lineage failures are represented by LineageDocumentError"
)]
impl LineageDocument {
    /// Creates an empty lineage namespace with a process-unique identity.
    #[must_use]
    pub fn new() -> Self {
        let nonce = LINEAGE_DOCUMENT_NONCE.fetch_add(1, Ordering::Relaxed);
        let mixed = (u128::from(nonce) << 64) ^ 0x6765_6f73_6f6c_7665_6c69_6e65_6167_6501_u128;
        Self::with_id(LineageDocumentId::from_raw(if mixed == 0 {
            1
        } else {
            mixed
        }))
    }

    /// Creates an empty lineage namespace with a caller-supplied identity.
    #[must_use]
    pub const fn with_id(id: LineageDocumentId) -> Self {
        Self {
            id,
            revision: LineageRevision::from_raw(0),
            next_step_id: LineageStepId::from_raw(1),
            next_output_id: LineageOutputId::from_raw(1),
            next_reservation_id: LineageReservationId::from_raw(1),
            evaluation_policy: LineageEvaluationPolicy::StrictChronological,
            steps: Vec::new(),
        }
    }

    #[must_use]
    pub const fn id(&self) -> LineageDocumentId {
        self.id
    }

    #[must_use]
    pub const fn revision(&self) -> LineageRevision {
        self.revision
    }

    #[must_use]
    pub const fn evaluation_policy(&self) -> LineageEvaluationPolicy {
        self.evaluation_policy
    }

    #[must_use]
    pub const fn allocator_high_water(&self) -> LineageAllocatorHighWater {
        LineageAllocatorHighWater {
            next_step_id: self.next_step_id,
            next_output_id: self.next_output_id,
            next_reservation_id: self.next_reservation_id,
        }
    }

    #[must_use]
    pub fn steps(&self) -> &[LineageStep] {
        &self.steps
    }

    #[must_use]
    pub fn step(&self, id: LineageStepId) -> Option<&LineageStep> {
        self.steps.iter().find(|step| step.id == id)
    }

    #[must_use]
    pub fn step_by_key(&self, key: &str) -> Option<&LineageStep> {
        self.steps.iter().find(|step| step.key.as_str() == key)
    }

    #[must_use]
    pub fn output_ref(
        &self,
        step: LineageStepId,
        output: LineageOutputId,
    ) -> Option<LineageOutputRef> {
        self.step(step)?.output_ref(self.id, output)
    }

    #[must_use]
    pub fn digest(&self) -> LineageDigest {
        digest_bytes(&self.canonical_payload_bytes())
    }

    #[must_use]
    pub fn identity(&self) -> LineageDocumentIdentity {
        LineageDocumentIdentity {
            document: self.id,
            revision: self.revision,
            digest: self.digest(),
        }
    }

    /// Applies an ordered mutation batch atomically after exact identity CAS.
    pub fn apply_patch(
        &mut self,
        patch: LineagePatch,
    ) -> Result<LineagePatchOutcome, LineageDocumentError> {
        self.validate()?;
        let actual = self.identity();
        if patch.expected.document != self.id {
            return Err(LineageDocumentError::WrongPatchDocument {
                expected: self.id,
                actual: patch.expected.document,
            });
        }
        if patch.expected.revision != actual.revision || patch.expected.digest != actual.digest {
            return Err(LineageDocumentError::StalePatch {
                expected_revision: patch.expected.revision,
                expected_digest: patch.expected.digest,
                actual_revision: actual.revision,
                actual_digest: actual.digest,
            });
        }
        if patch.mutations.is_empty() {
            return Err(LineageDocumentError::EmptyPatch);
        }

        let mut candidate = self.clone();
        let mut inserted_steps = Vec::new();
        let mut tombstoned_steps = Vec::new();
        for mutation in patch.mutations {
            candidate.apply_mutation(mutation, &mut inserted_steps, &mut tombstoned_steps)?;
        }
        candidate.normalize();
        candidate.validate()?;
        let changed = candidate != *self;
        if changed {
            candidate.revision = next_revision(self.revision)?;
            candidate.validate()?;
            *self = candidate;
        }
        Ok(LineagePatchOutcome {
            identity: self.identity(),
            changed,
            inserted_steps,
            tombstoned_steps,
        })
    }

    /// Produces deterministic evaluation eligibility under the persisted policy.
    /// `failed_steps` is evaluator evidence from the same document identity.
    pub fn dependency_plan(
        &self,
        failed_steps: impl IntoIterator<Item = LineageStepId>,
    ) -> Result<Vec<LineageDependencyPlanEntry>, LineageDocumentError> {
        self.dependency_plan_for(self.evaluation_policy, failed_steps)
    }

    /// Produces a deterministic plan for an explicit evaluator policy without
    /// changing the retained document or its digest.
    ///
    /// This is the differential-oracle seam: an evaluator can compare a cold
    /// chronological attempt with a dependency-local attempt against exactly
    /// the same lineage identity.
    pub fn dependency_plan_for(
        &self,
        policy: LineageEvaluationPolicy,
        failed_steps: impl IntoIterator<Item = LineageStepId>,
    ) -> Result<Vec<LineageDependencyPlanEntry>, LineageDocumentError> {
        self.validate()?;
        let failed = failed_steps.into_iter().collect::<BTreeSet<_>>();
        for step in &failed {
            let Some(value) = self.step(*step) else {
                return Err(LineageDocumentError::InvalidFailedStep { step: *step });
            };
            if value.state != LineageStepState::Live {
                return Err(LineageDocumentError::InvalidFailedStep { step: *step });
            }
        }

        let mut states = BTreeMap::new();
        let mut plan = Vec::with_capacity(self.steps.len());
        let mut strict_stopped = false;
        for step in &self.steps {
            let state = match step.state {
                LineageStepState::Suppressed => LineageStepEvaluationState::Suppressed,
                LineageStepState::Tombstoned => LineageStepEvaluationState::Tombstoned,
                LineageStepState::Live if strict_stopped => LineageStepEvaluationState::Unevaluated,
                LineageStepState::Live => {
                    let blockers = step
                        .action
                        .inputs()
                        .iter()
                        .map(|input| input.source.step)
                        .chain(
                            step.output_identities
                                .iter()
                                .filter_map(|identity| identity.source().map(|source| source.step)),
                        )
                        .filter(|dependency| {
                            !matches!(
                                states.get(dependency),
                                Some(LineageStepEvaluationState::Ready)
                            )
                        })
                        .collect::<BTreeSet<_>>()
                        .into_iter()
                        .collect::<Vec<_>>();
                    if !blockers.is_empty() {
                        LineageStepEvaluationState::Blocked {
                            dependencies: blockers,
                        }
                    } else if failed.contains(&step.id) {
                        LineageStepEvaluationState::Failed
                    } else {
                        LineageStepEvaluationState::Ready
                    }
                }
            };
            if policy == LineageEvaluationPolicy::StrictChronological
                && matches!(
                    state,
                    LineageStepEvaluationState::Failed | LineageStepEvaluationState::Blocked { .. }
                )
            {
                strict_stopped = true;
            }
            states.insert(step.id, state.clone());
            plan.push(LineageDependencyPlanEntry {
                step: step.id,
                state,
            });
        }
        Ok(plan)
    }

    /// Computes the exact forward dependency closure of changed owner steps.
    ///
    /// The closure follows only typed input and identity-flow references. It
    /// deliberately ignores coordinates, materialized vector order, and
    /// incidental JSON strings.
    pub fn dirty_dependency_closure(
        &self,
        changed_steps: impl IntoIterator<Item = LineageStepId>,
    ) -> Result<Vec<LineageStepId>, LineageDocumentError> {
        self.validate()?;
        let mut dirty = changed_steps.into_iter().collect::<BTreeSet<_>>();
        for step in &dirty {
            if self.step(*step).is_none() {
                return Err(LineageDocumentError::UnknownStep(*step));
            }
        }
        for step in &self.steps {
            let depends_on_dirty = step
                .action
                .inputs()
                .iter()
                .map(|input| input.source.step)
                .chain(
                    step.output_identities
                        .iter()
                        .filter_map(|identity| identity.source().map(|source| source.step)),
                )
                .any(|dependency| dirty.contains(&dependency));
            if depends_on_dirty {
                dirty.insert(step.id);
            }
        }
        Ok(self
            .steps
            .iter()
            .filter_map(|step| dirty.contains(&step.id).then_some(step.id))
            .collect())
    }

    /// Serializes strict, deterministic canonical lineage JSON.
    pub fn to_canonical_json(&self) -> Result<String, LineageDocumentError> {
        self.validate()?;
        let wire = LineageWireV1 {
            version: LINEAGE_DOCUMENT_VERSION,
            document_id: self.id,
            revision: self.revision,
            next_step_id: self.next_step_id,
            next_output_id: self.next_output_id,
            next_reservation_id: self.next_reservation_id,
            evaluation_policy: self.evaluation_policy,
            steps: self.steps.clone(),
            digest: self.digest(),
        };
        Ok(serde_json::to_string(&wire)?)
    }

    /// Imports bounded strict canonical-v1 lineage JSON and verifies its digest.
    pub fn from_json(json: &str) -> Result<Self, LineageDocumentError> {
        if json.len() > MAX_LINEAGE_JSON_BYTES {
            return Err(LineageDocumentError::JsonResourceLimit {
                limit: MAX_LINEAGE_JSON_BYTES,
            });
        }
        let wire: LineageWireV1 = serde_json::from_str(json)?;
        if wire.version != LINEAGE_DOCUMENT_VERSION {
            return Err(LineageDocumentError::UnsupportedVersion {
                actual: wire.version,
                expected: LINEAGE_DOCUMENT_VERSION,
            });
        }
        let mut document = Self {
            id: wire.document_id,
            revision: wire.revision,
            next_step_id: wire.next_step_id,
            next_output_id: wire.next_output_id,
            next_reservation_id: wire.next_reservation_id,
            evaluation_policy: wire.evaluation_policy,
            steps: wire.steps,
        };
        document.normalize();
        document.validate()?;
        if document.digest() != wire.digest {
            return Err(LineageDocumentError::DigestMismatch);
        }
        Ok(document)
    }

    /// Validates complete semantic structure without evaluating geometry.
    #[allow(
        clippy::too_many_lines,
        reason = "whole-document validation keeps the ordered identity/dependency audit in one transaction"
    )]
    pub fn validate(&self) -> Result<(), LineageDocumentError> {
        if self.id.raw() == 0 {
            return Err(invalid_field("document_id", "must be nonzero"));
        }
        if self.next_step_id.raw() == 0
            || self.next_output_id.raw() == 0
            || self.next_reservation_id.raw() == 0
        {
            return Err(invalid_field(
                "allocator",
                "allocator cursors must be nonzero",
            ));
        }
        validate_resource("steps", self.steps.len(), MAX_LINEAGE_STEPS)?;
        let output_count = checked_sum(self.steps.iter().map(|step| step.outputs.len()))?;
        let reservation_count = checked_sum(self.steps.iter().map(|step| step.reservations.len()))?;
        let input_count = checked_sum(self.steps.iter().map(|step| step.action.inputs().len()))?;
        validate_resource("outputs", output_count, MAX_LINEAGE_OUTPUTS)?;
        validate_resource("reservations", reservation_count, MAX_LINEAGE_RESERVATIONS)?;
        validate_resource("action inputs", input_count, MAX_LINEAGE_ACTION_INPUTS)?;

        let mut step_ids = BTreeSet::new();
        let mut developer_keys = BTreeSet::new();
        let mut output_ids = BTreeSet::new();
        let mut reservation_ids = BTreeSet::new();
        let mut persistent_reservations = BTreeSet::new();
        let mut positions = BTreeMap::new();
        let mut payload_nodes = 0_usize;
        let mut writable_leaf_count = 0_usize;
        let mut imported_baseline = None;

        for (position, step) in self.steps.iter().enumerate() {
            if step.id.raw() == 0 || step.id >= self.next_step_id {
                return Err(invalid_field(
                    "step id",
                    "must be nonzero and below the allocator cursor",
                ));
            }
            if !step_ids.insert(step.id) {
                return Err(LineageDocumentError::DuplicateStep(step.id));
            }
            if !developer_keys.insert(step.key.clone()) {
                return Err(LineageDocumentError::DuplicateDeveloperKey(
                    step.key.to_string(),
                ));
            }
            positions.insert(step.id, position);
            validate_label(&step.label)?;
            validate_action(step, &mut payload_nodes)?;
            if step.action.kind() == LineageActionKind::ImportedBaseline
                && (position != 0 || imported_baseline.replace(step.id).is_some())
            {
                return Err(LineageDocumentError::InvalidBaselinePosition { step: step.id });
            }

            let mut local_output_keys = BTreeSet::new();
            let mut local_reservation_keys = BTreeSet::new();
            let mut local_output_identities = BTreeMap::new();
            let local_reservations = step
                .reservations
                .iter()
                .map(|reservation| (reservation.id, reservation))
                .collect::<BTreeMap<_, _>>();
            if local_reservations.len() != step.reservations.len() {
                let duplicate = duplicate_id(step.reservations.iter().map(|value| value.id));
                return Err(LineageDocumentError::DuplicateReservation(
                    duplicate.unwrap_or(LineageReservationId::from_raw(0)),
                ));
            }
            for identity in &step.output_identities {
                if local_output_identities
                    .insert(identity.output, identity)
                    .is_some()
                {
                    return Err(LineageDocumentError::DuplicateOutputIdentity {
                        step: step.id,
                        output: identity.output,
                    });
                }
            }
            for reservation in &step.reservations {
                if reservation.id.raw() == 0 || reservation.id >= self.next_reservation_id {
                    return Err(invalid_field(
                        "reservation id",
                        "must be nonzero and below the allocator cursor",
                    ));
                }
                if !reservation_ids.insert(reservation.id) {
                    return Err(LineageDocumentError::DuplicateReservation(reservation.id));
                }
                if !local_reservation_keys.insert(reservation.key.clone()) {
                    return Err(LineageDocumentError::DuplicateSemanticKey {
                        step: step.id,
                        key: reservation.key.to_string(),
                    });
                }
                if !persistent_reservations
                    .insert((reservation.kind, reservation.persistent_id.clone()))
                {
                    return Err(LineageDocumentError::DuplicatePersistentReservation {
                        persistent_id: reservation.persistent_id.to_string(),
                    });
                }
            }
            for output in &step.outputs {
                if output.id.raw() == 0 || output.id >= self.next_output_id {
                    return Err(invalid_field(
                        "output id",
                        "must be nonzero and below the allocator cursor",
                    ));
                }
                if !output_ids.insert(output.id) {
                    return Err(LineageDocumentError::DuplicateOutput(output.id));
                }
                if !local_output_keys.insert(output.key.clone()) {
                    return Err(LineageDocumentError::DuplicateSemanticKey {
                        step: step.id,
                        key: output.key.to_string(),
                    });
                }
                let identity = local_output_identities.get(&output.id).ok_or(
                    LineageDocumentError::MissingOutputIdentity {
                        step: step.id,
                        output: output.id,
                    },
                )?;
                if let Some(reservation_id) = output.reservation {
                    let reservation = local_reservations.get(&reservation_id).ok_or(
                        LineageDocumentError::UnknownReservation {
                            step: step.id,
                            reservation: reservation_id,
                        },
                    )?;
                    if !reservation_matches_output(reservation.kind, output.kind) {
                        return Err(LineageDocumentError::ReservationKindMismatch {
                            step: step.id,
                            reservation: reservation_id,
                            output: output.kind,
                        });
                    }
                }
                match identity.flow {
                    LineageOutputIdentityFlow::Created { reservation }
                        if output.reservation == Some(reservation) => {}
                    LineageOutputIdentityFlow::OwnedLogical
                    | LineageOutputIdentityFlow::Aliased { .. }
                    | LineageOutputIdentityFlow::Continued { .. }
                    | LineageOutputIdentityFlow::Retired { .. }
                        if output.reservation.is_none() => {}
                    LineageOutputIdentityFlow::Created { .. } => {
                        return Err(LineageDocumentError::InvalidOutputIdentity {
                            step: step.id,
                            output: output.id,
                            message: "created output must name its exact backing reservation",
                        });
                    }
                    LineageOutputIdentityFlow::OwnedLogical
                    | LineageOutputIdentityFlow::Aliased { .. }
                    | LineageOutputIdentityFlow::Continued { .. }
                    | LineageOutputIdentityFlow::Retired { .. } => {
                        return Err(LineageDocumentError::InvalidOutputIdentity {
                            step: step.id,
                            output: output.id,
                            message: "only created outputs may consume a fresh reservation",
                        });
                    }
                }
            }
            if local_output_identities.len() != step.outputs.len() {
                let output = step
                    .output_identities
                    .iter()
                    .find(|identity| !step.outputs.iter().any(|value| value.id == identity.output))
                    .map_or(LineageOutputId::from_raw(0), |identity| identity.output);
                return Err(LineageDocumentError::UnknownOutput {
                    step: step.id,
                    output,
                });
            }
            let mut local_writable_leaves = BTreeSet::new();
            for leaf in &step.writable_leaves {
                if !local_writable_leaves.insert((leaf.output, leaf.key.clone())) {
                    return Err(LineageDocumentError::DuplicateWritableLeaf {
                        step: step.id,
                        output: leaf.output,
                        key: leaf.key.to_string(),
                    });
                }
                let identity = local_output_identities.get(&leaf.output).ok_or(
                    LineageDocumentError::UnknownOutput {
                        step: step.id,
                        output: leaf.output,
                    },
                )?;
                if matches!(
                    identity.flow,
                    LineageOutputIdentityFlow::Aliased { .. }
                        | LineageOutputIdentityFlow::Retired { .. }
                ) {
                    return Err(LineageDocumentError::InvalidWritableLeaf {
                        step: step.id,
                        output: leaf.output,
                        key: leaf.key.to_string(),
                        message: "aliased and retired outputs cannot declare writable ownership",
                    });
                }
            }
            writable_leaf_count = writable_leaf_count
                .checked_add(step.writable_leaves.len())
                .ok_or(LineageDocumentError::ResourceLimit {
                    resource: "writable leaf declarations",
                    actual: usize::MAX,
                    limit: MAX_LINEAGE_WRITABLE_LEAVES,
                })?;
        }
        validate_resource(
            "writable leaf declarations",
            writable_leaf_count,
            MAX_LINEAGE_WRITABLE_LEAVES,
        )?;
        validate_resource(
            "generic payload nodes",
            payload_nodes,
            MAX_LINEAGE_PAYLOAD_NODES,
        )?;

        let dependencies = self.validate_references(&positions)?;
        for (dependent, providers) in &dependencies {
            let dependent_step = self
                .step(*dependent)
                .ok_or(LineageDocumentError::UnknownStep(*dependent))?;
            if dependent_step.state != LineageStepState::Live {
                continue;
            }
            if let Some(deleted) = providers.iter().find(|provider| {
                self.step(**provider)
                    .is_some_and(|step| step.state == LineageStepState::Tombstoned)
            }) {
                return Err(LineageDocumentError::LiveDependent {
                    deleted: *deleted,
                    dependent: *dependent,
                });
            }
        }
        validate_acyclic(&dependencies)?;
        for (step, values) in &dependencies {
            let position = positions[step];
            for dependency in values {
                if positions[dependency] >= position {
                    return Err(LineageDocumentError::ForwardReference {
                        step: *step,
                        dependency: *dependency,
                    });
                }
            }
        }
        let canonical_size = self.canonical_payload_bytes().len();
        validate_resource(
            "canonical JSON bytes",
            canonical_size,
            MAX_LINEAGE_JSON_BYTES,
        )?;
        Ok(())
    }

    pub(crate) fn rebase_after_restore(
        &mut self,
        retained_revision: LineageRevision,
        retained_allocator: LineageAllocatorHighWater,
    ) -> Result<(), LineageDocumentError> {
        self.next_step_id = self.next_step_id.max(retained_allocator.next_step_id);
        self.next_output_id = self.next_output_id.max(retained_allocator.next_output_id);
        self.next_reservation_id = self
            .next_reservation_id
            .max(retained_allocator.next_reservation_id);
        let high_revision = self.revision.max(retained_revision);
        self.revision = next_revision(high_revision)?;
        self.validate()
    }

    #[allow(
        clippy::too_many_lines,
        reason = "one chronological pass must validate dependencies and identity generations together"
    )]
    fn validate_references(
        &self,
        positions: &BTreeMap<LineageStepId, usize>,
    ) -> Result<BTreeMap<LineageStepId, Vec<LineageStepId>>, LineageDocumentError> {
        let mut dependencies = BTreeMap::new();
        let mut ports = BTreeMap::<LineageOutputRef, IdentityPortVersion>::new();
        let mut token_states = BTreeMap::<LineageOutputRef, IdentityTokenState>::new();
        for step in &self.steps {
            let mut input_keys = BTreeSet::new();
            let mut step_dependencies = Vec::new();
            for input in step.action.inputs() {
                if !input_keys.insert(input.key.clone()) {
                    return Err(LineageDocumentError::DuplicateSemanticKey {
                        step: step.id,
                        key: input.key.to_string(),
                    });
                }
                self.validate_output_reference(step.id, input.kind, input.source, positions)?;
                validate_current_identity_port(
                    step.id,
                    input.source,
                    positions,
                    &ports,
                    &token_states,
                )?;
                step_dependencies.push(input.source.step);
            }
            for identity in &step.output_identities {
                let output = step
                    .outputs
                    .iter()
                    .find(|candidate| candidate.id == identity.output)
                    .ok_or(LineageDocumentError::UnknownOutput {
                        step: step.id,
                        output: identity.output,
                    })?;
                let output_ref = LineageOutputRef {
                    document: self.id,
                    step: step.id,
                    output: output.id,
                    kind: output.kind,
                };
                if let Some(source) = identity.source() {
                    self.validate_output_reference(step.id, output.kind, source, positions)?;
                    validate_current_identity_port(
                        step.id,
                        source,
                        positions,
                        &ports,
                        &token_states,
                    )?;
                    step_dependencies.push(source.step);
                }

                match identity.flow {
                    LineageOutputIdentityFlow::OwnedLogical
                    | LineageOutputIdentityFlow::Created { .. } => {
                        let port = IdentityPortVersion {
                            token: output_ref,
                            generation: 0,
                        };
                        ports.insert(output_ref, port);
                        token_states.insert(
                            output_ref,
                            IdentityTokenState::Current {
                                generation: 0,
                                successor: output_ref,
                            },
                        );
                    }
                    LineageOutputIdentityFlow::Aliased { source } => {
                        if let Some(port) = ports.get(&source).copied() {
                            ports.insert(output_ref, port);
                        }
                    }
                    LineageOutputIdentityFlow::Continued { source } => {
                        if let Some(source_port) = ports.get(&source).copied() {
                            if step.state == LineageStepState::Live {
                                let generation = source_port.generation.checked_add(1).ok_or(
                                    LineageDocumentError::ResourceLimit {
                                        resource: "identity-flow generations",
                                        actual: usize::MAX,
                                        limit: MAX_LINEAGE_STEPS,
                                    },
                                )?;
                                let continued = IdentityPortVersion {
                                    token: source_port.token,
                                    generation,
                                };
                                ports.insert(output_ref, continued);
                                token_states.insert(
                                    source_port.token,
                                    IdentityTokenState::Current {
                                        generation,
                                        successor: output_ref,
                                    },
                                );
                            } else {
                                // An inactive continuation remains a typed
                                // dependency and a resolvable logical port, but
                                // does not consume the predecessor generation.
                                ports.insert(output_ref, source_port);
                            }
                        }
                    }
                    LineageOutputIdentityFlow::Retired { source } => {
                        if step.state == LineageStepState::Live
                            && let Some(source_port) = ports.get(&source).copied()
                        {
                            token_states.insert(source_port.token, IdentityTokenState::Retired);
                        }
                    }
                }
            }
            step_dependencies.sort_unstable();
            step_dependencies.dedup();
            dependencies.insert(step.id, step_dependencies);
        }
        Ok(dependencies)
    }

    fn validate_output_reference(
        &self,
        consumer: LineageStepId,
        expected_kind: LineageOutputKind,
        source: LineageOutputRef,
        positions: &BTreeMap<LineageStepId, usize>,
    ) -> Result<(), LineageDocumentError> {
        if source.document != self.id {
            return Err(LineageDocumentError::CrossDocumentReference {
                step: consumer,
                expected: self.id,
                actual: source.document,
            });
        }
        if expected_kind != source.kind {
            return Err(LineageDocumentError::WrongOutputKind {
                step: consumer,
                expected: expected_kind,
                actual: source.kind,
            });
        }
        if !positions.contains_key(&source.step) {
            return Err(LineageDocumentError::UnknownStep(source.step));
        }
        let provider = self
            .step(source.step)
            .ok_or(LineageDocumentError::UnknownStep(source.step))?;
        let output = provider
            .outputs
            .iter()
            .find(|output| output.id == source.output)
            .ok_or(LineageDocumentError::UnknownOutput {
                step: source.step,
                output: source.output,
            })?;
        if output.kind != expected_kind {
            return Err(LineageDocumentError::WrongOutputKind {
                step: consumer,
                expected: expected_kind,
                actual: output.kind,
            });
        }
        if matches!(
            provider
                .output_identity(source.output)
                .map(|value| &value.flow),
            Some(LineageOutputIdentityFlow::Retired { .. })
        ) {
            return Err(LineageDocumentError::RetiredOutputReference {
                step: consumer,
                provider: source.step,
                output: source.output,
            });
        }
        Ok(())
    }

    #[allow(
        clippy::too_many_lines,
        reason = "one exhaustive dispatcher owns atomic semantics for the closed mutation vocabulary"
    )]
    fn apply_mutation(
        &mut self,
        mutation: LineageMutation,
        inserted_steps: &mut Vec<LineageStepId>,
        tombstoned_steps: &mut Vec<LineageStepId>,
    ) -> Result<bool, LineageDocumentError> {
        match mutation {
            LineageMutation::Insert { before, step } => {
                let step = *step;
                if step.state == LineageStepState::Tombstoned {
                    return Err(invalid_field(
                        "insert state",
                        "new steps cannot begin tombstoned",
                    ));
                }
                self.consume_insert_allocations(&step)?;
                let index = match before {
                    Some(target) => self
                        .steps
                        .iter()
                        .position(|candidate| candidate.id == target)
                        .ok_or(LineageDocumentError::UnknownStep(target))?,
                    None => self.steps.len(),
                };
                inserted_steps.push(step.id);
                self.steps.insert(index, step);
                Ok(true)
            }
            LineageMutation::Rewrite { step, replacement } => {
                let value = self.step_mut_for_edit(step)?;
                let replacement = *replacement;
                let changed =
                    value.label != replacement.label || value.action != replacement.action;
                value.label = replacement.label;
                value.action = replacement.action;
                Ok(changed)
            }
            LineageMutation::Tombstone { step } => {
                let value = self
                    .steps
                    .iter_mut()
                    .find(|candidate| candidate.id == step)
                    .ok_or(LineageDocumentError::UnknownStep(step))?;
                if value.state == LineageStepState::Tombstoned {
                    return Ok(false);
                }
                value.state = LineageStepState::Tombstoned;
                tombstoned_steps.push(step);
                Ok(true)
            }
            LineageMutation::DeleteSubtree { root } => {
                if self.step(root).is_none() {
                    return Err(LineageDocumentError::UnknownStep(root));
                }
                let mut closure = BTreeSet::from([root]);
                loop {
                    let before = closure.len();
                    for step in &self.steps {
                        if step
                            .action
                            .inputs()
                            .iter()
                            .any(|input| closure.contains(&input.source.step))
                            || step.output_identities.iter().any(|identity| {
                                identity
                                    .source()
                                    .is_some_and(|source| closure.contains(&source.step))
                            })
                        {
                            closure.insert(step.id);
                        }
                    }
                    if closure.len() == before {
                        break;
                    }
                }
                let mut changed = false;
                for step in &mut self.steps {
                    if closure.contains(&step.id) && step.state != LineageStepState::Tombstoned {
                        step.state = LineageStepState::Tombstoned;
                        tombstoned_steps.push(step.id);
                        changed = true;
                    }
                }
                Ok(changed)
            }
            LineageMutation::SetSuppressed { step, suppressed } => {
                let value = self.step_mut_for_edit(step)?;
                let target = if suppressed {
                    LineageStepState::Suppressed
                } else {
                    LineageStepState::Live
                };
                let changed = value.state != target;
                value.state = target;
                Ok(changed)
            }
            LineageMutation::Reorder { step, before } => {
                if before == Some(step) {
                    return Err(invalid_field(
                        "reorder",
                        "a step cannot be ordered before itself",
                    ));
                }
                let old_index = self
                    .steps
                    .iter()
                    .position(|candidate| candidate.id == step)
                    .ok_or(LineageDocumentError::UnknownStep(step))?;
                if let Some(target) = before
                    && self.step(target).is_none()
                {
                    return Err(LineageDocumentError::UnknownStep(target));
                }
                let value = self.steps.remove(old_index);
                let new_index = match before {
                    Some(target) => self
                        .steps
                        .iter()
                        .position(|candidate| candidate.id == target)
                        .ok_or(LineageDocumentError::UnknownStep(target))?,
                    None => self.steps.len(),
                };
                let changed = old_index != new_index;
                self.steps.insert(new_index, value);
                Ok(changed)
            }
            LineageMutation::Rebind {
                step,
                input,
                target,
            } => {
                let value = self.step_mut_for_edit(step)?;
                let bindings =
                    value
                        .action
                        .inputs_mut()
                        .ok_or(LineageDocumentError::UnknownInput {
                            step,
                            input: input.to_string(),
                        })?;
                let binding = bindings
                    .iter_mut()
                    .find(|binding| binding.key == input)
                    .ok_or(LineageDocumentError::UnknownInput {
                        step,
                        input: input.to_string(),
                    })?;
                if target.kind != binding.kind {
                    return Err(LineageDocumentError::RebindKindMismatch {
                        step,
                        input: input.to_string(),
                        expected: binding.kind,
                        actual: target.kind,
                    });
                }
                let changed = binding.source != target;
                binding.source = target;
                Ok(changed)
            }
            LineageMutation::SetEvaluationPolicy { policy } => {
                let changed = self.evaluation_policy != policy;
                self.evaluation_policy = policy;
                Ok(changed)
            }
        }
    }

    fn step_mut_for_edit(
        &mut self,
        id: LineageStepId,
    ) -> Result<&mut LineageStep, LineageDocumentError> {
        let step = self
            .steps
            .iter_mut()
            .find(|candidate| candidate.id == id)
            .ok_or(LineageDocumentError::UnknownStep(id))?;
        if step.state == LineageStepState::Tombstoned {
            return Err(LineageDocumentError::TombstonedStep { step: id });
        }
        Ok(step)
    }

    fn consume_insert_allocations(
        &mut self,
        step: &LineageStep,
    ) -> Result<(), LineageDocumentError> {
        if step.id != self.next_step_id {
            return Err(LineageDocumentError::UnexpectedAllocation {
                allocator: "step ID",
                expected: self.next_step_id.to_string(),
                actual: step.id.to_string(),
            });
        }
        let output_ids = step
            .outputs
            .iter()
            .map(|output| output.id)
            .collect::<BTreeSet<_>>();
        validate_contiguous_ids(
            self.next_output_id.raw(),
            output_ids.iter().map(|id| id.raw()),
            "output ID",
        )?;
        let reservation_ids = step
            .reservations
            .iter()
            .map(|reservation| reservation.id)
            .collect::<BTreeSet<_>>();
        validate_contiguous_ids(
            self.next_reservation_id.raw(),
            reservation_ids.iter().map(|id| id.raw()),
            "reservation ID",
        )?;
        self.next_step_id = LineageStepId::from_raw(
            self.next_step_id
                .raw()
                .checked_add(1)
                .ok_or(LineageDocumentError::IdExhausted)?,
        );
        self.next_output_id = LineageOutputId::from_raw(
            self.next_output_id
                .raw()
                .checked_add(
                    u64::try_from(output_ids.len())
                        .map_err(|_| LineageDocumentError::IdExhausted)?,
                )
                .ok_or(LineageDocumentError::IdExhausted)?,
        );
        self.next_reservation_id = LineageReservationId::from_raw(
            self.next_reservation_id
                .raw()
                .checked_add(
                    u64::try_from(reservation_ids.len())
                        .map_err(|_| LineageDocumentError::IdExhausted)?,
                )
                .ok_or(LineageDocumentError::IdExhausted)?,
        );
        Ok(())
    }

    fn normalize(&mut self) {
        for step in &mut self.steps {
            step.outputs.sort_by_key(|output| output.id);
            step.output_identities
                .sort_by_key(|identity| identity.output);
            step.writable_leaves
                .sort_by(|left, right| (left.output, &left.key).cmp(&(right.output, &right.key)));
            step.reservations.sort_by_key(|reservation| reservation.id);
            step.action.normalize();
        }
    }

    fn canonical_payload_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(&LineageCanonicalPayload {
            version: LINEAGE_DOCUMENT_VERSION,
            document_id: self.id,
            revision: self.revision,
            next_step_id: self.next_step_id,
            next_output_id: self.next_output_id,
            next_reservation_id: self.next_reservation_id,
            evaluation_policy: self.evaluation_policy,
            steps: &self.steps,
        })
        .expect("lineage canonical payload contains only infallibly serializable values")
    }
}

impl Default for LineageDocument {
    fn default() -> Self {
        Self::new()
    }
}

fn validate_current_identity_port(
    consumer: LineageStepId,
    source: LineageOutputRef,
    positions: &BTreeMap<LineageStepId, usize>,
    ports: &BTreeMap<LineageOutputRef, IdentityPortVersion>,
    token_states: &BTreeMap<LineageOutputRef, IdentityTokenState>,
) -> Result<(), LineageDocumentError> {
    if positions
        .get(&source.step)
        .zip(positions.get(&consumer))
        .is_none_or(|(source_position, consumer_position)| source_position >= consumer_position)
    {
        return Ok(());
    }
    let Some(port) = ports.get(&source) else {
        return Ok(());
    };
    match token_states.get(&port.token) {
        Some(IdentityTokenState::Current {
            generation,
            successor,
        }) if *generation != port.generation => {
            Err(LineageDocumentError::ConsumedOutputReference {
                step: consumer,
                provider: source.step,
                output: source.output,
                successor: *successor,
            })
        }
        Some(IdentityTokenState::Retired) => Err(LineageDocumentError::RetiredOutputReference {
            step: consumer,
            provider: source.step,
            output: source.output,
        }),
        Some(IdentityTokenState::Current { .. }) | None => Ok(()),
    }
}

fn validate_action(
    step: &LineageStep,
    payload_nodes: &mut usize,
) -> Result<(), LineageDocumentError> {
    if let LineageActionDefinition::ImportedBaseline { baseline } = &step.action {
        if baseline.payload.is_empty() {
            return Err(invalid_field("baseline payload", "must not be empty"));
        }
        validate_resource(
            "baseline payload bytes",
            baseline.payload.len(),
            MAX_LINEAGE_BASELINE_BYTES,
        )?;
        let version = match &baseline.encoding {
            ImportedBaselineEncoding::Canonical { version, .. }
            | ImportedBaselineEncoding::Opaque { version, .. } => *version,
        };
        if version == 0 {
            return Err(invalid_field("baseline version", "must be nonzero"));
        }
    } else {
        let payload = step
            .action
            .payload()
            .expect("non-baseline definitions always contain a versioned payload");
        if payload.version == 0 {
            return Err(invalid_field("action version", "must be nonzero"));
        }
        let bytes = serde_json::to_vec(&payload.parameters)?;
        validate_resource(
            "action parameter bytes",
            bytes.len(),
            MAX_LINEAGE_ACTION_PAYLOAD_BYTES,
        )?;
        for (key, value) in &payload.parameters {
            crate::LineageSemanticKey::new(key.clone())?;
            validate_value(value, 1, payload_nodes)?;
        }
    }
    Ok(())
}

fn validate_value(
    value: &Value,
    depth: usize,
    nodes: &mut usize,
) -> Result<(), LineageDocumentError> {
    if depth > MAX_LINEAGE_PAYLOAD_DEPTH {
        return Err(LineageDocumentError::ResourceLimit {
            resource: "generic payload depth",
            actual: depth,
            limit: MAX_LINEAGE_PAYLOAD_DEPTH,
        });
    }
    *nodes = nodes
        .checked_add(1)
        .ok_or(LineageDocumentError::IdExhausted)?;
    match value {
        Value::Array(values) => {
            for value in values {
                validate_value(value, depth + 1, nodes)?;
            }
        }
        Value::Object(values) => {
            for (key, value) in values {
                crate::LineageSemanticKey::new(key.clone())?;
                validate_value(value, depth + 1, nodes)?;
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
    Ok(())
}

fn validate_label(label: &str) -> Result<(), LineageDocumentError> {
    validate_resource("label bytes", label.len(), MAX_LINEAGE_LABEL_BYTES)?;
    if label.chars().any(char::is_control) {
        return Err(invalid_field(
            "label",
            "must not contain control characters",
        ));
    }
    Ok(())
}

fn validate_acyclic(
    dependencies: &BTreeMap<LineageStepId, Vec<LineageStepId>>,
) -> Result<(), LineageDocumentError> {
    let mut remaining = dependencies
        .iter()
        .map(|(step, values)| (*step, values.len()))
        .collect::<BTreeMap<_, _>>();
    let mut dependents = BTreeMap::<LineageStepId, Vec<LineageStepId>>::new();
    for (step, values) in dependencies {
        for dependency in values {
            dependents.entry(*dependency).or_default().push(*step);
        }
    }
    let mut ready = remaining
        .iter()
        .filter_map(|(step, count)| (*count == 0).then_some(*step))
        .collect::<BTreeSet<_>>();
    let mut visited = 0_usize;
    while let Some(step) = ready.pop_first() {
        visited += 1;
        if let Some(values) = dependents.get(&step) {
            for dependent in values {
                let count = remaining
                    .get_mut(dependent)
                    .expect("every dependent is a retained step");
                *count -= 1;
                if *count == 0 {
                    ready.insert(*dependent);
                }
            }
        }
    }
    if visited != dependencies.len() {
        let step = remaining
            .into_iter()
            .find_map(|(step, count)| (count != 0).then_some(step))
            .expect("an unvisited dependency cycle contains at least one step");
        return Err(LineageDocumentError::DependencyCycle { step });
    }
    Ok(())
}

fn validate_contiguous_ids(
    first: u64,
    values: impl IntoIterator<Item = u64>,
    allocator: &'static str,
) -> Result<(), LineageDocumentError> {
    for (offset, actual) in values.into_iter().enumerate() {
        let expected = first
            .checked_add(u64::try_from(offset).map_err(|_| LineageDocumentError::IdExhausted)?)
            .ok_or(LineageDocumentError::IdExhausted)?;
        if actual != expected {
            return Err(LineageDocumentError::UnexpectedAllocation {
                allocator,
                expected: format!("{expected:016x}"),
                actual: format!("{actual:016x}"),
            });
        }
    }
    Ok(())
}

fn validate_resource(
    resource: &'static str,
    actual: usize,
    limit: usize,
) -> Result<(), LineageDocumentError> {
    if actual > limit {
        return Err(LineageDocumentError::ResourceLimit {
            resource,
            actual,
            limit,
        });
    }
    Ok(())
}

fn checked_sum(values: impl IntoIterator<Item = usize>) -> Result<usize, LineageDocumentError> {
    values.into_iter().try_fold(0_usize, |sum, value| {
        sum.checked_add(value)
            .ok_or(LineageDocumentError::IdExhausted)
    })
}

fn duplicate_id<T: Copy + Ord>(values: impl IntoIterator<Item = T>) -> Option<T> {
    let mut seen = BTreeSet::new();
    values.into_iter().find(|value| !seen.insert(*value))
}

const fn invalid_field(field: &'static str, message: &'static str) -> LineageDocumentError {
    LineageDocumentError::InvalidField { field, message }
}

fn next_revision(revision: LineageRevision) -> Result<LineageRevision, LineageDocumentError> {
    revision
        .raw()
        .checked_add(1)
        .map(LineageRevision::from_raw)
        .ok_or(LineageDocumentError::RevisionExhausted)
}

pub(crate) fn digest_bytes(bytes: &[u8]) -> LineageDigest {
    // Four independent stable FNV-1a lanes provide a deterministic content
    // fingerprint without platform hashing or a new cryptographic dependency.
    const OFFSETS: [u64; 4] = [
        0xcbf2_9ce4_8422_2325,
        0x8422_2325_cbf2_9ce4,
        0x9e37_79b9_7f4a_7c15,
        0x6a09_e667_f3bc_c909,
    ];
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut digest = [0_u8; 32];
    for (lane_index, offset) in OFFSETS.into_iter().enumerate() {
        let mut lane = offset ^ u64::try_from(bytes.len()).unwrap_or(u64::MAX);
        for byte in bytes {
            lane ^= u64::from(*byte).wrapping_add((lane_index as u64) << 8);
            lane = lane.wrapping_mul(PRIME);
            lane ^= lane.rotate_right(17);
        }
        digest[lane_index * 8..lane_index * 8 + 8].copy_from_slice(&lane.to_be_bytes());
    }
    LineageDigest::from_bytes(digest)
}
