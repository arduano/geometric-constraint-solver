// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::{
    ImportedBaselineEncoding, LineageActionDefinition, LineageActionKind,
    LineageAllocatorHighWater, LineageAuxiliaryHighWater, LineageDeveloperKey, LineageDigest,
    LineageDocument, LineageDocumentError, LineageDocumentIdentity, LineageEvaluationPolicy,
    LineageOpaqueId, LineageOutputId, LineageOutputIdentityFlow, LineageOutputKind, LineagePatch,
    LineagePatchOutcome, LineageReservationId, LineageReservationKind, LineageRevision,
    LineageSemanticKey, LineageStepId,
};

/// Maximum retained checkpoints in each Undo or Redo direction.
pub const MAX_LINEAGE_HISTORY_ENTRIES: usize = 1_024;
/// Current canonical lineage-session wire version.
pub const LINEAGE_SESSION_VERSION: u32 = 1;
/// Maximum canonical lineage-session JSON accepted or emitted.
pub const MAX_LINEAGE_SESSION_JSON_BYTES: usize = 64 * 1024 * 1024;
/// Maximum independent host/domain cursors retained beside lineage history.
pub const MAX_LINEAGE_AUXILIARY_HIGH_WATERS: usize = 64;

/// Revision and allocator cursors observed across current and abandoned history.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LineageLifecycleHighWater {
    pub revision: LineageRevision,
    pub allocator: LineageAllocatorHighWater,
}

/// Outcome of one complete lineage evaluation attempt.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum LineageEvaluationDisposition {
    /// A structurally accepted lineage revision has not been evaluated yet.
    Pending,
    /// Owning-domain materialization and independent validation both passed.
    Accepted,
    /// Evaluation completed but one or more owning-domain checks rejected it.
    Failed,
    /// Cooperative cancellation stopped work before a publishable result.
    Cancelled,
    /// The deterministic work budget was exhausted before completion.
    Exhausted,
    /// The attempt lost exact lineage or external-input compare-and-swap.
    Stale,
}

/// Structured evidence for the latest lineage evaluation attempt.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LineageEvaluationAttempt {
    pub target: LineageDocumentIdentity,
    pub policy: LineageEvaluationPolicy,
    pub disposition: LineageEvaluationDisposition,
    pub external_inputs: Option<LineageOpaqueId>,
    pub materialization_digest: Option<LineageDigest>,
    pub failed_steps: Vec<LineageStepId>,
    pub diagnostic: Option<LineageSemanticKey>,
}

/// Last complete independently accepted lineage/materialization authority.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LineageAcceptedAuthority {
    pub lineage: LineageDocumentIdentity,
    pub external_inputs: Option<LineageOpaqueId>,
    pub materialization_digest: LineageDigest,
}

#[derive(Clone, Debug)]
struct LineageSessionCheckpoint {
    document: LineageDocument,
    latest_attempt: Option<LineageEvaluationAttempt>,
    last_accepted_document: Option<LineageDocument>,
    last_accepted: Option<LineageAcceptedAuthority>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct LineageSessionCheckpointWire {
    document: String,
    latest_attempt: Option<LineageEvaluationAttempt>,
    last_accepted_document: Option<String>,
    last_accepted: Option<LineageAcceptedAuthority>,
}

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
struct LineageSessionCanonicalPayload<'a> {
    version: u32,
    document: &'a str,
    latest_attempt: &'a Option<LineageEvaluationAttempt>,
    last_accepted_document: &'a Option<String>,
    last_accepted: &'a Option<LineageAcceptedAuthority>,
    undo: &'a [LineageSessionCheckpointWire],
    redo: &'a [LineageSessionCheckpointWire],
    lifecycle: LineageLifecycleHighWater,
    auxiliary_high_waters: &'a BTreeMap<LineageSemanticKey, LineageAuxiliaryHighWater>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct LineageSessionWireV1 {
    version: u32,
    document: String,
    latest_attempt: Option<LineageEvaluationAttempt>,
    last_accepted_document: Option<String>,
    last_accepted: Option<LineageAcceptedAuthority>,
    undo: Vec<LineageSessionCheckpointWire>,
    redo: Vec<LineageSessionCheckpointWire>,
    lifecycle: LineageLifecycleHighWater,
    auxiliary_high_waters: BTreeMap<LineageSemanticKey, LineageAuxiliaryHighWater>,
    digest: LineageDigest,
}

/// Single user-visible exact-CAS lineage document and Undo/Redo history.
///
/// Checkpoints retain declarative program versions. Restoring a checkpoint
/// rebases its revision and allocator cursors above all observed forward
/// history, so abandoned step/output/reservation IDs are never reused.
#[derive(Clone, Debug)]
pub struct LineageSession {
    document: LineageDocument,
    latest_attempt: Option<LineageEvaluationAttempt>,
    last_accepted_document: Option<LineageDocument>,
    last_accepted: Option<LineageAcceptedAuthority>,
    undo: Vec<LineageSessionCheckpoint>,
    redo: Vec<LineageSessionCheckpoint>,
    lifecycle: LineageLifecycleHighWater,
    auxiliary_high_waters: BTreeMap<LineageSemanticKey, LineageAuxiliaryHighWater>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct HistoricalStepIdentityBinding {
    key: LineageDeveloperKey,
    outputs: Vec<LineageOutputId>,
    reservations: Vec<LineageReservationId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum HistoricalActionSchemaBinding {
    CanonicalBaseline(LineageSemanticKey),
    OpaqueBaseline(LineageSemanticKey),
    Versioned(LineageSemanticKey),
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct HistoricalActionIdentityBinding {
    kind: LineageActionKind,
    schema: HistoricalActionSchemaBinding,
    version: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct HistoricalOutputIdentityBinding {
    step: LineageStepId,
    key: LineageSemanticKey,
    kind: LineageOutputKind,
    reservation: Option<LineageReservationId>,
    flow: LineageOutputIdentityFlow,
    writable_leaves: Vec<LineageSemanticKey>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct HistoricalReservationIdentityBinding {
    step: LineageStepId,
    key: LineageSemanticKey,
    kind: LineageReservationKind,
}

#[derive(Default)]
struct HistoricalIdentityBindings {
    steps: BTreeMap<LineageStepId, HistoricalStepIdentityBinding>,
    actions: BTreeMap<LineageStepId, HistoricalActionIdentityBinding>,
    developer_steps: BTreeMap<LineageDeveloperKey, LineageStepId>,
    outputs: BTreeMap<LineageOutputId, HistoricalOutputIdentityBinding>,
    reservations: BTreeMap<LineageReservationId, HistoricalReservationIdentityBinding>,
    reservation_persistent_ids:
        BTreeMap<LineageReservationId, (LineageReservationKind, LineageOpaqueId)>,
    persistent_reservations:
        BTreeMap<(LineageReservationKind, LineageOpaqueId), LineageReservationId>,
}

impl HistoricalIdentityBindings {
    fn observe_document(&mut self, document: &LineageDocument) -> Result<(), LineageDocumentError> {
        document.validate()?;
        for step in document.steps() {
            insert_historical_binding(
                &mut self.steps,
                step.id,
                HistoricalStepIdentityBinding {
                    key: step.key.clone(),
                    outputs: step
                        .outputs
                        .iter()
                        .map(|output| output.id)
                        .collect::<BTreeSet<_>>()
                        .into_iter()
                        .collect(),
                    reservations: step
                        .reservations
                        .iter()
                        .map(|reservation| reservation.id)
                        .collect::<BTreeSet<_>>()
                        .into_iter()
                        .collect(),
                },
                "step",
                step.id.to_string(),
            )?;
            insert_historical_binding(
                &mut self.actions,
                step.id,
                historical_action_identity(&step.action),
                "step action",
                step.id.to_string(),
            )?;
            insert_historical_binding(
                &mut self.developer_steps,
                step.key.clone(),
                step.id,
                "step",
                step.key.to_string(),
            )?;

            for output in &step.outputs {
                let flow = step
                    .output_identity(output.id)
                    .ok_or(LineageDocumentError::MissingOutputIdentity {
                        step: step.id,
                        output: output.id,
                    })?
                    .flow;
                let writable_leaves = step
                    .writable_leaves
                    .iter()
                    .filter(|leaf| leaf.output == output.id)
                    .map(|leaf| leaf.key.clone())
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect::<Vec<_>>();
                insert_historical_binding(
                    &mut self.outputs,
                    output.id,
                    HistoricalOutputIdentityBinding {
                        step: step.id,
                        key: output.key.clone(),
                        kind: output.kind,
                        reservation: output.reservation,
                        flow,
                        writable_leaves,
                    },
                    "output",
                    output.id.to_string(),
                )?;
            }

            for reservation in &step.reservations {
                insert_historical_binding(
                    &mut self.reservations,
                    reservation.id,
                    HistoricalReservationIdentityBinding {
                        step: step.id,
                        key: reservation.key.clone(),
                        kind: reservation.kind,
                    },
                    "reservation",
                    reservation.id.to_string(),
                )?;
                insert_historical_binding(
                    &mut self.reservation_persistent_ids,
                    reservation.id,
                    (reservation.kind, reservation.persistent_id.clone()),
                    "persistent identity",
                    reservation.id.to_string(),
                )?;
                insert_historical_binding(
                    &mut self.persistent_reservations,
                    (reservation.kind, reservation.persistent_id.clone()),
                    reservation.id,
                    "persistent identity",
                    reservation.persistent_id.to_string(),
                )?;
            }
        }
        Ok(())
    }

    fn reject_reintroduced_consumed_numeric_identities(
        &self,
        document: &LineageDocument,
        lifecycle: LineageLifecycleHighWater,
    ) -> Result<(), LineageDocumentError> {
        for step in document.steps() {
            if step.id < lifecycle.allocator.next_step_id && !self.steps.contains_key(&step.id) {
                return Err(LineageDocumentError::CrossHistoryIdentityRebinding {
                    identity: "step",
                    value: step.id.to_string(),
                });
            }
            for output in &step.outputs {
                if output.id < lifecycle.allocator.next_output_id
                    && !self.outputs.contains_key(&output.id)
                {
                    return Err(LineageDocumentError::CrossHistoryIdentityRebinding {
                        identity: "output",
                        value: output.id.to_string(),
                    });
                }
            }
            for reservation in &step.reservations {
                if reservation.id < lifecycle.allocator.next_reservation_id
                    && !self.reservations.contains_key(&reservation.id)
                {
                    return Err(LineageDocumentError::CrossHistoryIdentityRebinding {
                        identity: "reservation",
                        value: reservation.id.to_string(),
                    });
                }
            }
        }
        Ok(())
    }
}

fn historical_action_identity(action: &LineageActionDefinition) -> HistoricalActionIdentityBinding {
    match action {
        LineageActionDefinition::ImportedBaseline { baseline } => match &baseline.encoding {
            ImportedBaselineEncoding::Canonical { schema, version } => {
                HistoricalActionIdentityBinding {
                    kind: action.kind(),
                    schema: HistoricalActionSchemaBinding::CanonicalBaseline(schema.clone()),
                    version: *version,
                }
            }
            ImportedBaselineEncoding::Opaque {
                media_type,
                version,
            } => HistoricalActionIdentityBinding {
                kind: action.kind(),
                schema: HistoricalActionSchemaBinding::OpaqueBaseline(media_type.clone()),
                version: *version,
            },
        },
        LineageActionDefinition::GeometryRecipe { action: payload }
        | LineageActionDefinition::Constraint { action: payload }
        | LineageActionDefinition::Dimension { action: payload }
        | LineageActionDefinition::Trim { action: payload }
        | LineageActionDefinition::Parameter { action: payload }
        | LineageActionDefinition::Binding { action: payload }
        | LineageActionDefinition::External { action: payload }
        | LineageActionDefinition::Operation { action: payload }
        | LineageActionDefinition::ComputedFeature { action: payload }
        | LineageActionDefinition::Annotation { action: payload } => {
            HistoricalActionIdentityBinding {
                kind: action.kind(),
                schema: HistoricalActionSchemaBinding::Versioned(payload.schema.clone()),
                version: payload.version,
            }
        }
    }
}

#[allow(
    clippy::missing_errors_doc,
    reason = "all session failures are represented by LineageDocumentError"
)]
impl LineageSession {
    /// Starts a session from a validated lineage document with empty history.
    pub fn new(document: LineageDocument) -> Result<Self, LineageDocumentError> {
        document.validate()?;
        let lifecycle = lifecycle_of(&document);
        Ok(Self {
            document,
            latest_attempt: None,
            last_accepted_document: None,
            last_accepted: None,
            undo: Vec::new(),
            redo: Vec::new(),
            lifecycle,
            auxiliary_high_waters: BTreeMap::new(),
        })
    }

    /// Restores canonical lineage JSON with intentionally empty history.
    pub fn from_json(json: &str) -> Result<Self, LineageDocumentError> {
        Self::new(LineageDocument::from_json(json)?)
    }

    /// Restores complete canonical retained/accepted authority and bounded
    /// Undo/Redo history from the versioned lineage-session wire format.
    pub fn from_session_json(json: &str) -> Result<Self, LineageDocumentError> {
        if json.len() > MAX_LINEAGE_SESSION_JSON_BYTES {
            return Err(LineageDocumentError::SessionJsonResourceLimit {
                limit: MAX_LINEAGE_SESSION_JSON_BYTES,
            });
        }
        let wire: LineageSessionWireV1 = serde_json::from_str(json)?;
        if wire.version != LINEAGE_SESSION_VERSION {
            return Err(LineageDocumentError::UnsupportedSessionVersion {
                actual: wire.version,
                expected: LINEAGE_SESSION_VERSION,
            });
        }
        if wire.undo.len() > MAX_LINEAGE_HISTORY_ENTRIES
            || wire.redo.len() > MAX_LINEAGE_HISTORY_ENTRIES
        {
            return Err(LineageDocumentError::ResourceLimit {
                resource: "lineage session history",
                actual: wire.undo.len().max(wire.redo.len()),
                limit: MAX_LINEAGE_HISTORY_ENTRIES,
            });
        }
        if wire.auxiliary_high_waters.len() > MAX_LINEAGE_AUXILIARY_HIGH_WATERS {
            return Err(LineageDocumentError::ResourceLimit {
                resource: "lineage session auxiliary high-waters",
                actual: wire.auxiliary_high_waters.len(),
                limit: MAX_LINEAGE_AUXILIARY_HIGH_WATERS,
            });
        }
        let expected_digest = session_wire_digest(&wire)?;
        if expected_digest != wire.digest {
            return Err(LineageDocumentError::DigestMismatch);
        }

        let document = LineageDocument::from_json(&wire.document)?;
        let last_accepted_document = wire
            .last_accepted_document
            .as_deref()
            .map(LineageDocument::from_json)
            .transpose()?;
        let undo = wire
            .undo
            .into_iter()
            .map(decode_checkpoint)
            .collect::<Result<Vec<_>, _>>()?;
        let redo = wire
            .redo
            .into_iter()
            .map(decode_checkpoint)
            .collect::<Result<Vec<_>, _>>()?;
        let session = Self {
            document,
            latest_attempt: wire.latest_attempt,
            last_accepted_document,
            last_accepted: wire.last_accepted,
            undo,
            redo,
            lifecycle: wire.lifecycle,
            auxiliary_high_waters: wire.auxiliary_high_waters,
        };
        session.validate_authority()?;
        Ok(session)
    }

    #[must_use]
    pub const fn document(&self) -> &LineageDocument {
        &self.document
    }

    /// Latest complete attempt evidence, including a pending retained edit.
    #[must_use]
    pub const fn latest_attempt(&self) -> Option<&LineageEvaluationAttempt> {
        self.latest_attempt.as_ref()
    }

    /// Exact last independently accepted lineage/materialization stamp.
    #[must_use]
    pub const fn last_accepted(&self) -> Option<&LineageAcceptedAuthority> {
        self.last_accepted.as_ref()
    }

    /// Last accepted declarative program. It may intentionally differ from
    /// the current retained document after a failed edit.
    #[must_use]
    pub const fn last_accepted_document(&self) -> Option<&LineageDocument> {
        self.last_accepted_document.as_ref()
    }

    #[must_use]
    pub fn identity(&self) -> LineageDocumentIdentity {
        self.document.identity()
    }

    #[must_use]
    pub const fn lifecycle_high_water(&self) -> LineageLifecycleHighWater {
        self.lifecycle
    }

    /// Returns one session-global never-reuse cursor. Auxiliary cursors are
    /// deliberately outside user-visible history and therefore never regress
    /// through Undo or Redo.
    #[must_use]
    pub fn auxiliary_high_water(
        &self,
        key: &LineageSemanticKey,
    ) -> Option<LineageAuxiliaryHighWater> {
        self.auxiliary_high_waters.get(key).copied()
    }

    /// Retains the maximum observed value of one session-global never-reuse
    /// cursor without changing lineage revision, Undo, Redo, or evaluation
    /// evidence.
    pub fn retain_auxiliary_high_water(
        &mut self,
        key: LineageSemanticKey,
        value: LineageAuxiliaryHighWater,
    ) -> Result<(), LineageDocumentError> {
        if !self.auxiliary_high_waters.contains_key(&key)
            && self.auxiliary_high_waters.len() == MAX_LINEAGE_AUXILIARY_HIGH_WATERS
        {
            return Err(LineageDocumentError::ResourceLimit {
                resource: "lineage session auxiliary high-waters",
                actual: self.auxiliary_high_waters.len().saturating_add(1),
                limit: MAX_LINEAGE_AUXILIARY_HIGH_WATERS,
            });
        }
        self.auxiliary_high_waters
            .entry(key)
            .and_modify(|retained| *retained = (*retained).max(value))
            .or_insert(value);
        Ok(())
    }

    #[must_use]
    pub const fn undo_len(&self) -> usize {
        self.undo.len()
    }

    #[must_use]
    pub const fn redo_len(&self) -> usize {
        self.redo.len()
    }

    #[must_use]
    pub const fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    #[must_use]
    pub const fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Removes evaluation evidence that crossed an untrusted serialization
    /// boundary while preserving the complete declarative program, lifecycle
    /// high-waters, and Undo/Redo topology.
    ///
    /// A canonical session digest proves only that its fields are internally
    /// self-consistent; it does not prove that a caller-supplied
    /// materialization digest came from the owning-domain evaluator. RPC and
    /// similar adapters must call this after strict decoding and before
    /// publishing a loaded session. Ordinary evaluation can then establish
    /// fresh accepted authority for the current revision.
    pub fn discard_unverified_evaluation_authority(&mut self) {
        Self::discard_checkpoint_evaluation_authority(
            &self.document,
            &mut self.latest_attempt,
            &mut self.last_accepted_document,
            &mut self.last_accepted,
        );
        for checkpoint in self.undo.iter_mut().chain(&mut self.redo) {
            Self::discard_checkpoint_evaluation_authority(
                &checkpoint.document,
                &mut checkpoint.latest_attempt,
                &mut checkpoint.last_accepted_document,
                &mut checkpoint.last_accepted,
            );
        }
    }

    /// Applies one atomic patch and records exactly one history position when
    /// its retained lineage content changes. Rejected and no-op patches leave
    /// both history directions byte-for-byte untouched.
    pub fn apply_patch(
        &mut self,
        patch: LineagePatch,
    ) -> Result<LineagePatchOutcome, LineageDocumentError> {
        self.validate_authority()?;
        let before = self.checkpoint();
        let mut candidate_document = self.document.clone();
        let outcome = candidate_document.apply_patch(patch)?;
        if outcome.changed {
            let mut historical_identities = self.historical_identity_bindings()?;
            historical_identities.observe_document(&candidate_document)?;
            self.document = candidate_document;
            push_bounded(&mut self.undo, before);
            self.redo.clear();
            self.observe_current();
            self.mark_pending();
        }
        Ok(outcome)
    }

    /// Replaces the complete program in one user-visible history edit.
    ///
    /// The replacement must belong to the same document, carry exactly the
    /// next revision, validate independently, and retain every allocator
    /// cursor observed in current or abandoned history. This is the core seam
    /// used by complete-program TypeScript reconciliation.
    pub fn reconcile(
        &mut self,
        expected: LineageDocumentIdentity,
        replacement: LineageDocument,
    ) -> Result<LineageDocumentIdentity, LineageDocumentError> {
        self.validate_authority()?;
        let actual = self.document.identity();
        if expected.document != actual.document {
            return Err(LineageDocumentError::WrongPatchDocument {
                expected: actual.document,
                actual: expected.document,
            });
        }
        if expected.revision != actual.revision || expected.digest != actual.digest {
            return Err(LineageDocumentError::StalePatch {
                expected_revision: expected.revision,
                expected_digest: expected.digest,
                actual_revision: actual.revision,
                actual_digest: actual.digest,
            });
        }
        replacement.validate()?;
        if replacement.id() != actual.document {
            return Err(LineageDocumentError::WrongPatchDocument {
                expected: actual.document,
                actual: replacement.id(),
            });
        }
        let expected_revision = actual
            .revision
            .raw()
            .checked_add(1)
            .map(LineageRevision::from_raw)
            .ok_or(LineageDocumentError::RevisionExhausted)?;
        if replacement.revision() != expected_revision {
            return Err(LineageDocumentError::NonSequentialReplacementRevision {
                expected: expected_revision,
                actual: replacement.revision(),
            });
        }
        validate_allocator_non_regression(
            replacement.allocator_high_water(),
            self.lifecycle.allocator,
        )?;
        let mut historical_identities = self.historical_identity_bindings()?;
        historical_identities
            .reject_reintroduced_consumed_numeric_identities(&replacement, self.lifecycle)?;
        historical_identities.observe_document(&replacement)?;

        let mut candidate = self.clone();
        let before = candidate.checkpoint();
        candidate.document = replacement;
        push_bounded(&mut candidate.undo, before);
        candidate.redo.clear();
        candidate.observe_current();
        candidate.mark_pending();
        candidate.validate_authority()?;
        let identity = candidate.document.identity();
        *self = candidate;
        Ok(identity)
    }

    /// Publishes exact evidence that the current retained program reproduced
    /// one complete independently validated materialization. This changes no
    /// lineage revision and creates no extra user-visible history position.
    pub fn accept_current(
        &mut self,
        expected: LineageDocumentIdentity,
        external_inputs: Option<LineageOpaqueId>,
        materialization_digest: LineageDigest,
    ) -> Result<LineageAcceptedAuthority, LineageDocumentError> {
        self.ensure_current(expected)?;
        let authority = LineageAcceptedAuthority {
            lineage: expected,
            external_inputs: external_inputs.clone(),
            materialization_digest,
        };
        self.last_accepted_document = Some(self.document.clone());
        self.last_accepted = Some(authority.clone());
        self.latest_attempt = Some(LineageEvaluationAttempt {
            target: expected,
            policy: self.document.evaluation_policy(),
            disposition: LineageEvaluationDisposition::Accepted,
            external_inputs,
            materialization_digest: Some(materialization_digest),
            failed_steps: Vec::new(),
            diagnostic: None,
        });
        Ok(authority)
    }

    /// Records one completed owning-domain rejection for the current retained
    /// program while preserving the previous accepted authority unchanged.
    /// The structurally accepted edit already owns its single history entry.
    pub fn reject_current(
        &mut self,
        expected: LineageDocumentIdentity,
        external_inputs: Option<LineageOpaqueId>,
        diagnostic: LineageSemanticKey,
        mut failed_steps: Vec<LineageStepId>,
    ) -> Result<(), LineageDocumentError> {
        self.ensure_current(expected)?;
        failed_steps.sort_unstable();
        failed_steps.dedup();
        if failed_steps.is_empty() {
            return Err(LineageDocumentError::InvalidEvaluationAttempt {
                message: "failed evaluation must name at least one live failed step",
            });
        }
        for step in &failed_steps {
            if !self
                .document
                .step(*step)
                .is_some_and(|value| value.state == crate::LineageStepState::Live)
            {
                return Err(LineageDocumentError::InvalidFailedStep { step: *step });
            }
        }
        self.latest_attempt = Some(LineageEvaluationAttempt {
            target: expected,
            policy: self.document.evaluation_policy(),
            disposition: LineageEvaluationDisposition::Failed,
            external_inputs,
            materialization_digest: None,
            failed_steps,
            diagnostic: Some(diagnostic),
        });
        Ok(())
    }

    /// Records cancellation, work exhaustion, or stale-publication evidence
    /// without changing retained lineage, accepted authority, or history.
    pub fn record_nonpublishing_attempt(
        &mut self,
        target: LineageDocumentIdentity,
        external_inputs: Option<LineageOpaqueId>,
        disposition: LineageEvaluationDisposition,
        diagnostic: LineageSemanticKey,
    ) -> Result<(), LineageDocumentError> {
        if target.document != self.document.id() {
            return Err(LineageDocumentError::WrongPatchDocument {
                expected: self.document.id(),
                actual: target.document,
            });
        }
        if !matches!(
            disposition,
            LineageEvaluationDisposition::Cancelled
                | LineageEvaluationDisposition::Exhausted
                | LineageEvaluationDisposition::Stale
        ) {
            return Err(LineageDocumentError::InvalidEvaluationAttempt {
                message: "nonpublishing evidence must be cancelled, exhausted, or stale",
            });
        }
        self.latest_attempt = Some(LineageEvaluationAttempt {
            target,
            policy: self.document.evaluation_policy(),
            disposition,
            external_inputs,
            materialization_digest: None,
            failed_steps: Vec::new(),
            diagnostic: Some(diagnostic),
        });
        Ok(())
    }

    /// Restores the preceding declarative program version with a fresh exact
    /// revision. Returns `None` when no Undo checkpoint exists.
    pub fn undo(&mut self) -> Result<Option<LineageDocumentIdentity>, LineageDocumentError> {
        let Some(mut restored) = self.undo.last().cloned() else {
            return Ok(None);
        };
        restored
            .document
            .rebase_after_restore(self.lifecycle.revision, self.lifecycle.allocator)?;
        let current = self.checkpoint();
        self.undo.pop();
        push_bounded(&mut self.redo, current);
        self.restore_checkpoint(restored);
        self.observe_current();
        self.mark_pending();
        Ok(Some(self.document.identity()))
    }

    /// Restores the next declarative program version with a fresh exact
    /// revision. Returns `None` when no Redo checkpoint exists.
    pub fn redo(&mut self) -> Result<Option<LineageDocumentIdentity>, LineageDocumentError> {
        let Some(mut restored) = self.redo.last().cloned() else {
            return Ok(None);
        };
        restored
            .document
            .rebase_after_restore(self.lifecycle.revision, self.lifecycle.allocator)?;
        let current = self.checkpoint();
        self.redo.pop();
        push_bounded(&mut self.undo, current);
        self.restore_checkpoint(restored);
        self.observe_current();
        self.mark_pending();
        Ok(Some(self.document.identity()))
    }

    /// Exports only authoritative lineage intent. Undo/Redo checkpoints are
    /// omitted; use [`Self::to_canonical_session_json`] for workspace-v7
    /// authority and exact history persistence.
    pub fn to_canonical_json(&self) -> Result<String, LineageDocumentError> {
        self.document.to_canonical_json()
    }

    /// Exports retained lineage, latest attempt, last accepted authority,
    /// bounded Undo/Redo checkpoints, and allocator lifecycle high-waters as
    /// strict deterministic canonical session JSON.
    pub fn to_canonical_session_json(&self) -> Result<String, LineageDocumentError> {
        self.validate_authority()?;
        let document = self.document.to_canonical_json()?;
        let last_accepted_document = self
            .last_accepted_document
            .as_ref()
            .map(LineageDocument::to_canonical_json)
            .transpose()?;
        let undo = self
            .undo
            .iter()
            .map(encode_checkpoint)
            .collect::<Result<Vec<_>, _>>()?;
        let redo = self
            .redo
            .iter()
            .map(encode_checkpoint)
            .collect::<Result<Vec<_>, _>>()?;
        let mut wire = LineageSessionWireV1 {
            version: LINEAGE_SESSION_VERSION,
            document,
            latest_attempt: self.latest_attempt.clone(),
            last_accepted_document,
            last_accepted: self.last_accepted.clone(),
            undo,
            redo,
            lifecycle: self.lifecycle,
            auxiliary_high_waters: self.auxiliary_high_waters.clone(),
            digest: LineageDigest::from_bytes([0; 32]),
        };
        wire.digest = session_wire_digest(&wire)?;
        let json = serde_json::to_string(&wire)?;
        if json.len() > MAX_LINEAGE_SESSION_JSON_BYTES {
            return Err(LineageDocumentError::SessionJsonResourceLimit {
                limit: MAX_LINEAGE_SESSION_JSON_BYTES,
            });
        }
        Ok(json)
    }

    fn observe_current(&mut self) {
        let current = lifecycle_of(&self.document);
        self.lifecycle.revision = self.lifecycle.revision.max(current.revision);
        self.lifecycle.allocator.next_step_id = self
            .lifecycle
            .allocator
            .next_step_id
            .max(current.allocator.next_step_id);
        self.lifecycle.allocator.next_output_id = self
            .lifecycle
            .allocator
            .next_output_id
            .max(current.allocator.next_output_id);
        self.lifecycle.allocator.next_reservation_id = self
            .lifecycle
            .allocator
            .next_reservation_id
            .max(current.allocator.next_reservation_id);
    }

    fn ensure_current(
        &self,
        expected: LineageDocumentIdentity,
    ) -> Result<(), LineageDocumentError> {
        let actual = self.document.identity();
        if expected.document != actual.document {
            return Err(LineageDocumentError::WrongPatchDocument {
                expected: actual.document,
                actual: expected.document,
            });
        }
        if expected.revision != actual.revision || expected.digest != actual.digest {
            return Err(LineageDocumentError::StalePatch {
                expected_revision: expected.revision,
                expected_digest: expected.digest,
                actual_revision: actual.revision,
                actual_digest: actual.digest,
            });
        }
        Ok(())
    }

    fn checkpoint(&self) -> LineageSessionCheckpoint {
        LineageSessionCheckpoint {
            document: self.document.clone(),
            latest_attempt: self.latest_attempt.clone(),
            last_accepted_document: self.last_accepted_document.clone(),
            last_accepted: self.last_accepted.clone(),
        }
    }

    fn restore_checkpoint(&mut self, checkpoint: LineageSessionCheckpoint) {
        self.document = checkpoint.document;
        self.latest_attempt = checkpoint.latest_attempt;
        self.last_accepted_document = checkpoint.last_accepted_document;
        self.last_accepted = checkpoint.last_accepted;
    }

    fn mark_pending(&mut self) {
        self.latest_attempt = Some(LineageEvaluationAttempt {
            target: self.document.identity(),
            policy: self.document.evaluation_policy(),
            disposition: LineageEvaluationDisposition::Pending,
            external_inputs: None,
            materialization_digest: None,
            failed_steps: Vec::new(),
            diagnostic: None,
        });
    }

    fn discard_checkpoint_evaluation_authority(
        document: &LineageDocument,
        latest_attempt: &mut Option<LineageEvaluationAttempt>,
        last_accepted_document: &mut Option<LineageDocument>,
        last_accepted: &mut Option<LineageAcceptedAuthority>,
    ) {
        *latest_attempt = Some(LineageEvaluationAttempt {
            target: document.identity(),
            policy: document.evaluation_policy(),
            disposition: LineageEvaluationDisposition::Pending,
            external_inputs: None,
            materialization_digest: None,
            failed_steps: Vec::new(),
            diagnostic: None,
        });
        *last_accepted_document = None;
        *last_accepted = None;
    }

    fn validate_authority(&self) -> Result<(), LineageDocumentError> {
        self.document.validate()?;
        if self.undo.len() > MAX_LINEAGE_HISTORY_ENTRIES
            || self.redo.len() > MAX_LINEAGE_HISTORY_ENTRIES
        {
            return Err(LineageDocumentError::ResourceLimit {
                resource: "lineage session history",
                actual: self.undo.len().max(self.redo.len()),
                limit: MAX_LINEAGE_HISTORY_ENTRIES,
            });
        }
        if self.auxiliary_high_waters.len() > MAX_LINEAGE_AUXILIARY_HIGH_WATERS {
            return Err(LineageDocumentError::ResourceLimit {
                resource: "lineage session auxiliary high-waters",
                actual: self.auxiliary_high_waters.len(),
                limit: MAX_LINEAGE_AUXILIARY_HIGH_WATERS,
            });
        }
        let document_id = self.document.id();
        validate_authority_pair(
            &self.document,
            self.latest_attempt.as_ref(),
            self.last_accepted_document.as_ref(),
            self.last_accepted.as_ref(),
            document_id,
        )?;
        validate_lifecycle_document(&self.document, self.lifecycle)?;
        if let Some(document) = &self.last_accepted_document {
            validate_lifecycle_document(document, self.lifecycle)?;
        }
        for checkpoint in self.undo.iter().chain(&self.redo) {
            validate_authority_pair(
                &checkpoint.document,
                checkpoint.latest_attempt.as_ref(),
                checkpoint.last_accepted_document.as_ref(),
                checkpoint.last_accepted.as_ref(),
                document_id,
            )?;
            validate_lifecycle_document(&checkpoint.document, self.lifecycle)?;
            if let Some(document) = &checkpoint.last_accepted_document {
                validate_lifecycle_document(document, self.lifecycle)?;
            }
        }
        self.historical_identity_bindings()?;
        Ok(())
    }

    fn historical_identity_bindings(
        &self,
    ) -> Result<HistoricalIdentityBindings, LineageDocumentError> {
        let mut bindings = HistoricalIdentityBindings::default();
        bindings.observe_document(&self.document)?;
        if let Some(document) = &self.last_accepted_document {
            bindings.observe_document(document)?;
        }
        for checkpoint in self.undo.iter().chain(&self.redo) {
            bindings.observe_document(&checkpoint.document)?;
            if let Some(document) = &checkpoint.last_accepted_document {
                bindings.observe_document(document)?;
            }
        }
        Ok(bindings)
    }
}

fn encode_checkpoint(
    checkpoint: &LineageSessionCheckpoint,
) -> Result<LineageSessionCheckpointWire, LineageDocumentError> {
    Ok(LineageSessionCheckpointWire {
        document: checkpoint.document.to_canonical_json()?,
        latest_attempt: checkpoint.latest_attempt.clone(),
        last_accepted_document: checkpoint
            .last_accepted_document
            .as_ref()
            .map(LineageDocument::to_canonical_json)
            .transpose()?,
        last_accepted: checkpoint.last_accepted.clone(),
    })
}

fn decode_checkpoint(
    checkpoint: LineageSessionCheckpointWire,
) -> Result<LineageSessionCheckpoint, LineageDocumentError> {
    Ok(LineageSessionCheckpoint {
        document: LineageDocument::from_json(&checkpoint.document)?,
        latest_attempt: checkpoint.latest_attempt,
        last_accepted_document: checkpoint
            .last_accepted_document
            .as_deref()
            .map(LineageDocument::from_json)
            .transpose()?,
        last_accepted: checkpoint.last_accepted,
    })
}

fn session_wire_digest(wire: &LineageSessionWireV1) -> Result<LineageDigest, LineageDocumentError> {
    let payload = LineageSessionCanonicalPayload {
        version: wire.version,
        document: &wire.document,
        latest_attempt: &wire.latest_attempt,
        last_accepted_document: &wire.last_accepted_document,
        last_accepted: &wire.last_accepted,
        undo: &wire.undo,
        redo: &wire.redo,
        lifecycle: wire.lifecycle,
        auxiliary_high_waters: &wire.auxiliary_high_waters,
    };
    Ok(crate::document::digest_bytes(&serde_json::to_vec(
        &payload,
    )?))
}

fn validate_authority_pair(
    document: &LineageDocument,
    latest_attempt: Option<&LineageEvaluationAttempt>,
    last_accepted_document: Option<&LineageDocument>,
    last_accepted: Option<&LineageAcceptedAuthority>,
    expected_document: crate::LineageDocumentId,
) -> Result<(), LineageDocumentError> {
    document.validate()?;
    if document.id() != expected_document {
        return Err(LineageDocumentError::InvalidSessionAuthority {
            message: "all retained history must belong to one lineage document",
        });
    }
    match (last_accepted_document, last_accepted) {
        (None, None) => {}
        (Some(accepted_document), Some(accepted)) => {
            accepted_document.validate()?;
            if accepted_document.id() != expected_document
                || accepted_document.identity() != accepted.lineage
            {
                return Err(LineageDocumentError::InvalidSessionAuthority {
                    message: "accepted authority must exactly identify its accepted program",
                });
            }
        }
        _ => {
            return Err(LineageDocumentError::InvalidSessionAuthority {
                message: "accepted program and materialization stamp must be present together",
            });
        }
    }
    if let Some(attempt) = latest_attempt {
        if attempt.target.document != expected_document {
            return Err(LineageDocumentError::InvalidEvaluationAttempt {
                message: "attempt belongs to a different lineage document",
            });
        }
        match attempt.disposition {
            LineageEvaluationDisposition::Pending => {
                if attempt.target != document.identity()
                    || attempt.materialization_digest.is_some()
                    || !attempt.failed_steps.is_empty()
                    || attempt.diagnostic.is_some()
                {
                    return Err(LineageDocumentError::InvalidEvaluationAttempt {
                        message: "pending evidence must identify current retained lineage only",
                    });
                }
            }
            LineageEvaluationDisposition::Accepted => {
                let Some(accepted) = last_accepted else {
                    return Err(LineageDocumentError::InvalidEvaluationAttempt {
                        message: "accepted attempt requires accepted authority",
                    });
                };
                if attempt.target != document.identity()
                    || attempt.target != accepted.lineage
                    || attempt.materialization_digest != Some(accepted.materialization_digest)
                    || attempt.external_inputs != accepted.external_inputs
                    || !attempt.failed_steps.is_empty()
                    || attempt.diagnostic.is_some()
                {
                    return Err(LineageDocumentError::InvalidEvaluationAttempt {
                        message: "accepted attempt must exactly match accepted authority",
                    });
                }
            }
            LineageEvaluationDisposition::Failed => {
                if attempt.target != document.identity()
                    || attempt.materialization_digest.is_some()
                    || attempt.failed_steps.is_empty()
                    || attempt.diagnostic.is_none()
                {
                    return Err(LineageDocumentError::InvalidEvaluationAttempt {
                        message: "failed attempt must identify current lineage and failed steps",
                    });
                }
                for step in &attempt.failed_steps {
                    if !document
                        .step(*step)
                        .is_some_and(|value| value.state == crate::LineageStepState::Live)
                    {
                        return Err(LineageDocumentError::InvalidFailedStep { step: *step });
                    }
                }
            }
            LineageEvaluationDisposition::Cancelled
            | LineageEvaluationDisposition::Exhausted
            | LineageEvaluationDisposition::Stale => {
                if attempt.materialization_digest.is_some()
                    || !attempt.failed_steps.is_empty()
                    || attempt.diagnostic.is_none()
                {
                    return Err(LineageDocumentError::InvalidEvaluationAttempt {
                        message: "nonpublishing attempt carries invalid result evidence",
                    });
                }
            }
        }
    }
    Ok(())
}

fn validate_lifecycle_document(
    document: &LineageDocument,
    lifecycle: LineageLifecycleHighWater,
) -> Result<(), LineageDocumentError> {
    if document.revision() > lifecycle.revision {
        return Err(LineageDocumentError::InvalidSessionAuthority {
            message: "session revision high-water regresses retained history",
        });
    }
    validate_allocator_non_regression(lifecycle.allocator, document.allocator_high_water())
}

fn lifecycle_of(document: &LineageDocument) -> LineageLifecycleHighWater {
    LineageLifecycleHighWater {
        revision: document.revision(),
        allocator: document.allocator_high_water(),
    }
}

fn push_bounded(history: &mut Vec<LineageSessionCheckpoint>, checkpoint: LineageSessionCheckpoint) {
    if history.len() == MAX_LINEAGE_HISTORY_ENTRIES {
        history.remove(0);
    }
    history.push(checkpoint);
}

fn insert_historical_binding<K, V>(
    bindings: &mut BTreeMap<K, V>,
    key: K,
    value: V,
    identity: &'static str,
    display: String,
) -> Result<(), LineageDocumentError>
where
    K: Ord,
    V: Eq,
{
    if let Some(existing) = bindings.get(&key) {
        if existing != &value {
            return Err(LineageDocumentError::CrossHistoryIdentityRebinding {
                identity,
                value: display,
            });
        }
    } else {
        bindings.insert(key, value);
    }
    Ok(())
}

fn validate_allocator_non_regression(
    actual: LineageAllocatorHighWater,
    minimum: LineageAllocatorHighWater,
) -> Result<(), LineageDocumentError> {
    if actual.next_step_id < minimum.next_step_id {
        return Err(LineageDocumentError::AllocatorRegression {
            allocator: "step ID",
            minimum: minimum.next_step_id.to_string(),
            actual: actual.next_step_id.to_string(),
        });
    }
    if actual.next_output_id < minimum.next_output_id {
        return Err(LineageDocumentError::AllocatorRegression {
            allocator: "output ID",
            minimum: minimum.next_output_id.to_string(),
            actual: actual.next_output_id.to_string(),
        });
    }
    if actual.next_reservation_id < minimum.next_reservation_id {
        return Err(LineageDocumentError::AllocatorRegression {
            allocator: "reservation ID",
            minimum: minimum.next_reservation_id.to_string(),
            actual: actual.next_reservation_id.to_string(),
        });
    }
    Ok(())
}
