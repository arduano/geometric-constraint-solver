// SPDX-License-Identifier: GPL-3.0-or-later
//! Trusted semantic target/history authority staged across external durable writes.
//! The compiler owns canonical inventory and the engine independently validates edits.

use crate::{error, json, parse};
use geosolve_collaboration::{
    history::{
        ContributionHistory, DependencyChange, InversePlan, ObjectDescription, PropertyAddress,
        PropertyChange, ReorderChange, StatementPosition, TransactionChanges,
    },
    protocol::{MAX_REVISION, OperationId},
    targets::{DeletionPlan, SemanticTarget, TargetLedger, TargetLimits},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    sync::atomic::{AtomicU32, Ordering},
};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

const MAX_HANDLES: usize = 32;
const MAX_HANDLE_BYTES: usize = 64 * 1024 * 1024;
const MAX_CHECKPOINT_BYTES: usize = 384 * 1024 * 1024;
const MAX_ACTIONS: usize = 100_000;
static NEXT_SEMANTIC_HOST: AtomicU32 = AtomicU32::new(0);

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Configuration {
    document_epoch: String,
    server_epoch: String,
    #[serde(default)]
    objects: Vec<InitialObject>,
    #[serde(default)]
    limits: Limits,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct InitialObject {
    object: String,
    #[serde(default)]
    dependencies: Vec<String>,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields, default)]
#[allow(clippy::struct_field_names)] // Matches the core TargetLimits contract.
struct Limits {
    max_objects_including_tombstones: usize,
    max_dependencies_per_object: usize,
    max_total_dependencies: usize,
}
impl Default for Limits {
    fn default() -> Self {
        let limits = TargetLimits::default();
        Self {
            max_objects_including_tombstones: limits.max_objects_including_tombstones,
            max_dependencies_per_object: limits.max_dependencies_per_object,
            max_total_dependencies: limits.max_total_dependencies,
        }
    }
}
impl TryFrom<Limits> for TargetLimits {
    type Error = String;
    fn try_from(limits: Limits) -> Result<Self, Self::Error> {
        let maximum = Self::default();
        if limits.max_objects_including_tombstones > maximum.max_objects_including_tombstones
            || limits.max_dependencies_per_object > maximum.max_dependencies_per_object
            || limits.max_total_dependencies > maximum.max_total_dependencies
        {
            return Err("semantic target limit exceeds adapter maximum".into());
        }
        Ok(Self {
            max_objects_including_tombstones: limits.max_objects_including_tombstones,
            max_dependencies_per_object: limits.max_dependencies_per_object,
            max_total_dependencies: limits.max_total_dependencies,
        })
    }
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Checkpoint {
    document_epoch: String,
    revision: u64,
    targets_json: String,
    history_json: String,
}
#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum TargetReference {
    Existing { target: SemanticTarget },
    Created { object: String },
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Creation {
    object: String,
    #[serde(default)]
    dependencies: Vec<TargetReference>,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DependencyUpdate {
    target: TargetReference,
    dependencies: Vec<TargetReference>,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    operation: OperationId,
    changes: Vec<PropertyChange>,
    structural: Option<StructuralRecord>,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct StructuralRecord {
    #[serde(default)]
    created: Vec<CreatedDescription>,
    #[serde(default)]
    deleted: Vec<DeletedDescription>,
    #[serde(default)]
    reorders: Vec<ReorderChange>,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CreatedDescription {
    object: String,
    payload: serde_json::Value,
    position: CreationPosition,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CreationPosition {
    previous: Option<PositionReference>,
    next: Option<PositionReference>,
}
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum PositionReference {
    Target(SemanticTarget),
    Reference(TargetReference),
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DeletedDescription {
    target: SemanticTarget,
    payload: serde_json::Value,
    position: StatementPosition,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RecordRequest {
    basis_revision: u64,
    revision: u64,
    operation: OperationId,
    changes: Vec<PropertyChange>,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Transaction {
    basis_revision: u64,
    revision: u64,
    #[serde(default)]
    create: Vec<Creation>,
    #[serde(default)]
    deletions: Vec<DeletionPlan>,
    #[serde(default)]
    dependencies: Vec<DependencyUpdate>,
    record: Option<Record>,
}
#[derive(Debug)]
struct Prepared {
    basis_revision: u64,
    plan: InversePlan,
    bytes: usize,
}
#[derive(Debug)]
struct Pending {
    id: String,
    basis_revision: u64,
    revision: u64,
    targets: TargetLedger,
    history: ContributionHistory,
}

/// Native authority for a trusted server; never dispatch browser packets directly
/// into these validated-edit methods. Snapshots omit speculative pending state.
#[derive(Debug)]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub struct TrustedSemanticHost {
    targets: TargetLedger,
    history: ContributionHistory,
    document_epoch: String,
    server_epoch: String,
    instance: u32,
    revision: u64,
    next_handle: u64,
    prepared: BTreeMap<String, Prepared>,
    pending: Option<Pending>,
    poisoned: bool,
}
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
impl TrustedSemanticHost {
    /// Allocates the trusted initial inventory before installing forward dependencies.
    ///
    /// # Errors
    /// Rejects invalid identities, dependency references and bounded resources.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(constructor))]
    pub fn new(config_json: &str) -> Result<Self, String> {
        let config: Configuration = parse(config_json)?;
        let mut targets = TargetLedger::new(config.limits.try_into()?).map_err(error)?;
        if config.objects.len() > MAX_ACTIONS {
            return Err("semantic inventory count limit".into());
        }
        for object in &config.objects {
            targets.create(&object.object).map_err(error)?;
        }
        for object in &config.objects {
            let target = targets
                .current(&object.object)
                .ok_or("missing initial object")?;
            let dependencies = object
                .dependencies
                .iter()
                .map(|name| {
                    targets
                        .current(name)
                        .ok_or_else(|| "missing initial dependency".to_owned())
                })
                .collect::<Result<Vec<_>, _>>()?;
            targets
                .set_dependencies(&target, &dependencies)
                .map_err(error)?;
        }
        Self::from_state(
            targets,
            ContributionHistory::default(),
            config.document_epoch,
            config.server_epoch,
            0,
        )
    }
    /// Restores the exact journal-authenticated native checkpoint pair. The caller
    /// independently reconstructs its accepted model and supplies its exact revision.
    ///
    /// # Errors
    /// Rejects foreign/oversized checkpoints, corrupt core state and revision mismatch.
    pub fn restore(config_json: &str, checkpoint_json: &str) -> Result<Self, String> {
        let config: Configuration = parse(config_json)?;
        if checkpoint_json.len() > MAX_CHECKPOINT_BYTES {
            return Err("semantic checkpoint byte limit".into());
        }
        let checkpoint: Checkpoint = serde_json::from_str(checkpoint_json).map_err(error)?;
        if checkpoint.document_epoch != config.document_epoch || checkpoint.revision > MAX_REVISION
        {
            return Err("invalid semantic checkpoint identity or revision".into());
        }
        let targets = TargetLedger::from_json(&checkpoint.targets_json, config.limits.try_into()?)
            .map_err(error)?;
        let history = ContributionHistory::from_json(&checkpoint.history_json).map_err(error)?;
        if history.revision() > checkpoint.revision {
            return Err("semantic history is ahead of accepted model revision".into());
        }
        Self::from_state(
            targets,
            history,
            config.document_epoch,
            config.server_epoch,
            checkpoint.revision,
        )
    }
    /// # Errors
    /// Reports JSON encoding failure.
    pub fn snapshot(&self) -> Result<String, String> {
        json(
            &serde_json::json!({"revision":self.revision,"historyRevision":self.history.revision(),"highWater":self.targets.high_water(),"dependencyCount":self.targets.dependency_count(),"hasPendingStage":self.pending.is_some(),"needsRecovery":self.poisoned}),
        )
    }
    /// Exact committed core JSON pair; persist a stage's candidate pair when writing.
    ///
    /// # Errors
    /// Reports bounded checkpoint or JSON encoding failure.
    pub fn checkpoint(&self) -> Result<String, String> {
        json(&self.checkpoint_of(&self.targets, &self.history, self.revision)?)
    }
    /// # Errors
    /// Reports JSON encoding failure.
    pub fn current(&self, object: &str) -> Result<String, String> {
        json(&self.targets.current(object))
    }
    /// # Errors
    /// Rejects stale, malformed or forged target generations.
    pub fn authenticate(&self, target_json: &str) -> Result<(), String> {
        self.targets
            .authenticate(&parse(target_json)?)
            .map_err(error)
    }
    /// # Errors
    /// Rejects stale/duplicate roots and bounded resources.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = planDelete))]
    pub fn plan_delete(&self, roots_json: &str) -> Result<String, String> {
        json(
            &self
                .targets
                .plan_delete(&parse::<Vec<SemanticTarget>>(roots_json)?)
                .map_err(error)?,
        )
    }
    /// # Errors
    /// Rejects changed exact dependent closure or target generations.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = authenticateDelete))]
    pub fn authenticate_delete(&self, plan_json: &str) -> Result<(), String> {
        self.targets
            .authenticate_delete(&parse(plan_json)?)
            .map_err(error)
    }
    /// # Errors
    /// Rejects malformed property addresses or JSON encoding failure.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = propertyOwner))]
    pub fn property_owner(&self, address_json: &str) -> Result<String, String> {
        json(
            &self
                .history
                .property_owner(&parse::<PropertyAddress>(address_json)?),
        )
    }
    /// Read-only committed personal history availability. Pending durability and
    /// recovery disable commands without allocating native preparation tickets.
    ///
    /// # Errors
    /// Reports encoding failure.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = userHistory))]
    pub fn user_history(&self, user_id: &str) -> Result<String, String> {
        let mut history = self.history.user_history(user_id, &self.targets);
        let unavailable = if self.poisoned {
            Some("semantic host requires durable recovery")
        } else if self.pending.is_some() {
            Some("semantic persistence is pending")
        } else {
            None
        };
        if let Some(reason) = unavailable {
            history.can_undo = false;
            history.can_redo = false;
            history.undo_unavailable = Some(reason.into());
            history.redo_unavailable = Some(reason.into());
        }
        let mut value = serde_json::to_value(history).map_err(error)?;
        value["revision"] = self.revision.into();
        value["hasPendingStage"] = self.pending.is_some().into();
        value["needsRecovery"] = self.poisoned.into();
        json(&value)
    }

    /// Retains a native checked inverse while independent domain validation runs.
    ///
    /// # Errors
    /// Rejects overwritten/deleted/absent contributions and bounded ticket capacity.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = prepareUndo))]
    pub fn prepare_undo(&mut self, user_id: &str) -> Result<String, String> {
        self.require_mutable()?;
        let plan = self
            .history
            .prepare_undo(user_id, &self.targets)
            .map_err(error)?;
        self.prepare(plan)
    }
    /// # Errors
    /// Rejects unavailable/overwritten Redo and bounded ticket capacity.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = prepareRedo))]
    pub fn prepare_redo(&mut self, user_id: &str) -> Result<String, String> {
        self.require_mutable()?;
        let plan = self
            .history
            .prepare_redo(user_id, &self.targets)
            .map_err(error)?;
        self.prepare(plan)
    }
    /// # Errors
    /// Rejects foreign/released handles or pending persistence.
    pub fn release(&mut self, ticket: &str) -> Result<(), String> {
        self.require_mutable()?;
        self.prepared
            .remove(ticket)
            .ok_or("unknown or stale semantic ticket")?;
        Ok(())
    }
    /// Records independently validated exact before/after observations on a clone.
    ///
    /// # Errors
    /// Rejects stale basis, invalid ownership/lifetimes and pending persistence.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = stageValidatedRecord))]
    pub fn stage_validated_record(&mut self, request_json: &str) -> Result<String, String> {
        let request: RecordRequest = parse(request_json)?;
        self.check_revision(request.basis_revision, request.revision)?;
        let mut history = self.history.clone();
        history
            .record(
                request.operation,
                request.revision,
                request.changes,
                &self.targets,
            )
            .map_err(error)?;
        self.install_stage(self.targets.clone(), history, request.revision, &[])
    }
    /// Applies trusted compiler-derived lifecycle/dependency updates and optional
    /// validated contribution observations in one staged transaction. Opt-in
    /// structural observations share the same personal property/lifecycle timeline.
    ///
    /// # Errors
    /// Rejects stale deletion closures, references, revisions and bounded resources.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = stageValidatedTransaction))]
    pub fn stage_validated_transaction(&mut self, request_json: &str) -> Result<String, String> {
        let request: Transaction = parse(request_json)?;
        self.check_revision(request.basis_revision, request.revision)?;
        if request
            .create
            .len()
            .saturating_add(request.deletions.len())
            .saturating_add(request.dependencies.len())
            > MAX_ACTIONS
        {
            return Err("semantic transaction count limit".into());
        }
        // Authenticate every client-reviewed closure against committed state before
        // candidate dependency changes can shrink or expand it.
        for plan in &request.deletions {
            self.targets.authenticate_delete(plan).map_err(error)?;
        }
        let mut targets = self.targets.clone();
        let mut history = self.history.clone();
        for plan in &request.deletions {
            targets.delete(plan).map_err(error)?;
        }
        let mut created = BTreeMap::new();
        let mut allocated = Vec::new();
        for creation in &request.create {
            let target = targets.create(&creation.object).map_err(error)?;
            created.insert(creation.object.clone(), target.clone());
            allocated.push(target);
        }
        for creation in &request.create {
            let dependencies = references(&creation.dependencies, &targets, &created)?;
            targets
                .set_dependencies(&created[&creation.object], &dependencies)
                .map_err(error)?;
        }
        let mut dependency_changes = Vec::new();
        for update in &request.dependencies {
            let target = reference(&update.target, &targets, &created)?;
            let dependencies = references(&update.dependencies, &targets, &created)?;
            if self.targets.authenticate(&target).is_ok() {
                dependency_changes.push(DependencyChange {
                    target: target.clone(),
                    before: self.targets.dependencies(&target).map_err(error)?,
                    after: {
                        let mut canonical = dependencies.clone();
                        canonical.sort();
                        canonical
                    },
                });
            }
            targets
                .set_dependencies(&target, &dependencies)
                .map_err(error)?;
        }
        record_transaction(
            &mut history,
            request.record,
            request.revision,
            dependency_changes,
            &self.targets,
            &targets,
            &created,
        )?;
        self.install_stage(targets, history, request.revision, &allocated)
    }
    /// Commits a native prepared inverse only after independent model validation.
    ///
    /// # Errors
    /// Rejects foreign/stale tickets, revision drift and changed core ownership.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = stageValidatedInverse))]
    pub fn stage_validated_inverse(
        &mut self,
        ticket: &str,
        operation_json: &str,
        revision_json: &str,
    ) -> Result<String, String> {
        self.require_mutable()?;
        let prepared = self
            .prepared
            .get(ticket)
            .ok_or("unknown or stale semantic ticket")?;
        let revision: u64 = parse(revision_json)?;
        self.check_revision(prepared.basis_revision, revision)?;
        let mut history = self.history.clone();
        let mut targets = self.targets.clone();
        let created = prepared
            .plan
            .structural()
            .create
            .iter()
            .map(|entry| entry.target.clone())
            .collect::<Vec<_>>();
        history
            .commit_inverse_transaction(
                &prepared.plan,
                parse(operation_json)?,
                revision,
                &mut targets,
            )
            .map_err(error)?;
        self.install_stage(targets, history, revision, &created)
    }
    /// Install only after exact candidate pair/model/source transaction is fsynced.
    ///
    /// # Errors
    /// Rejects foreign/reused stages and committed-state drift.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = commitStage))]
    pub fn commit_stage(&mut self, stage_id: &str) -> Result<String, String> {
        let stage = self.stage(stage_id)?;
        if stage.basis_revision != self.revision {
            return Err("semantic stage basis changed".into());
        }
        let stage = self.pending.take().ok_or("no pending semantic stage")?;
        self.targets = stage.targets;
        self.history = stage.history;
        self.revision = stage.revision;
        self.prepared.clear();
        self.snapshot()
    }
    /// Discards a candidate only when the trusted host knows persistence has not
    /// begun. This is not safe for an uncertain append; use `failStage` then.
    ///
    /// # Errors
    /// Rejects foreign/reused stages; discarded IDs are never reused.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = discardUnpersistedStage))]
    pub fn discard_unpersisted_stage(&mut self, stage_id: &str) -> Result<(), String> {
        self.stage(stage_id)?;
        self.pending = None;
        Ok(())
    }

    /// Uncertain persistence invalidates this runtime; reconstruct from durable state.
    ///
    /// # Errors
    /// Rejects foreign/reused stages without discarding the real pending stage.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = failStage))]
    pub fn fail_stage(&mut self, stage_id: &str) -> Result<(), String> {
        self.stage(stage_id)?;
        self.poisoned = true;
        self.pending = None;
        self.prepared.clear();
        Ok(())
    }
}
impl TrustedSemanticHost {
    fn from_state(
        targets: TargetLedger,
        history: ContributionHistory,
        document_epoch: String,
        server_epoch: String,
        revision: u64,
    ) -> Result<Self, String> {
        for epoch in [&document_epoch, &server_epoch] {
            if epoch.is_empty() || epoch.len() > 256 || epoch.chars().any(char::is_control) {
                return Err("invalid semantic host epoch".into());
            }
        }
        let instance = NEXT_SEMANTIC_HOST
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                value.checked_add(1)
            })
            .map_err(|_| "semantic host instance capacity")?;
        Ok(Self {
            targets,
            history,
            document_epoch,
            server_epoch,
            instance,
            revision,
            next_handle: 0,
            prepared: BTreeMap::new(),
            pending: None,
            poisoned: false,
        })
    }
    fn require_mutable(&self) -> Result<(), String> {
        if self.poisoned {
            return Err("semantic host requires durable recovery".into());
        }
        if self.pending.is_some() {
            return Err("semantic persistence is pending".into());
        }
        Ok(())
    }
    fn check_revision(&self, basis: u64, revision: u64) -> Result<(), String> {
        self.require_mutable()?;
        if basis != self.revision
            || revision != basis.checked_add(1).ok_or("semantic revision exhausted")?
            || revision > MAX_REVISION
        {
            return Err("semantic revision must advance exact committed basis by one".into());
        }
        Ok(())
    }
    fn next_id(&mut self) -> Result<String, String> {
        self.next_handle = self
            .next_handle
            .checked_add(1)
            .filter(|value| *value <= MAX_REVISION)
            .ok_or("semantic handle capacity")?;
        Ok(format!(
            "{}:{}:{}",
            self.server_epoch, self.instance, self.next_handle
        ))
    }
    fn prepare(&mut self, plan: InversePlan) -> Result<String, String> {
        let bytes = json(&(plan.changes(), plan.structural()))?.len();
        if self.prepared.len() >= MAX_HANDLES
            || self
                .prepared
                .values()
                .map(|entry| entry.bytes)
                .sum::<usize>()
                .saturating_add(bytes)
                > MAX_HANDLE_BYTES
        {
            return Err("semantic inverse capacity".into());
        }
        let ticket = self.next_id()?;
        let response = json(
            &serde_json::json!({"ticket":ticket,"basisRevision":self.revision,"contribution":plan.contribution(),"direction":plan.direction(),"changes":plan.changes(),"structural":plan.structural()}),
        )?;
        self.prepared.insert(
            ticket,
            Prepared {
                basis_revision: self.revision,
                plan,
                bytes,
            },
        );
        Ok(response)
    }
    fn checkpoint_of(
        &self,
        targets: &TargetLedger,
        history: &ContributionHistory,
        revision: u64,
    ) -> Result<Checkpoint, String> {
        Ok(Checkpoint {
            document_epoch: self.document_epoch.clone(),
            revision,
            targets_json: targets.to_json().map_err(error)?,
            history_json: history.to_json().map_err(error)?,
        })
    }
    fn install_stage(
        &mut self,
        targets: TargetLedger,
        history: ContributionHistory,
        revision: u64,
        created: &[SemanticTarget],
    ) -> Result<String, String> {
        let checkpoint = self.checkpoint_of(&targets, &history, revision)?;
        let id = self.next_id()?;
        let response = json(
            &serde_json::json!({"status":"staged","stageId":id,"basisRevision":self.revision,"revision":revision,"targetsJson":checkpoint.targets_json,"historyJson":checkpoint.history_json,"created":created}),
        )?;
        self.pending = Some(Pending {
            id,
            basis_revision: self.revision,
            revision,
            targets,
            history,
        });
        Ok(response)
    }
    fn stage(&self, id: &str) -> Result<&Pending, String> {
        self.pending
            .as_ref()
            .filter(|stage| stage.id == id)
            .ok_or_else(|| "unknown or stale semantic stage".into())
    }
}
fn reference(
    value: &TargetReference,
    targets: &TargetLedger,
    created: &BTreeMap<String, SemanticTarget>,
) -> Result<SemanticTarget, String> {
    let target = match value {
        TargetReference::Existing { target } => target,
        TargetReference::Created { object } => created
            .get(object)
            .ok_or("semantic reference was not created in this transaction")?,
    };
    targets.authenticate(target).map_err(error)?;
    Ok(target.clone())
}
fn references(
    values: &[TargetReference],
    targets: &TargetLedger,
    created: &BTreeMap<String, SemanticTarget>,
) -> Result<Vec<SemanticTarget>, String> {
    values
        .iter()
        .map(|value| reference(value, targets, created))
        .collect()
}

fn creation_position(
    value: &CreationPosition,
    targets: &TargetLedger,
    created: &BTreeMap<String, SemanticTarget>,
) -> Result<StatementPosition, String> {
    let resolve = |value: &PositionReference| match value {
        PositionReference::Target(target) => {
            targets.authenticate(target).map_err(error)?;
            Ok(target.clone())
        }
        PositionReference::Reference(value) => reference(value, targets, created),
    };
    Ok(StatementPosition {
        previous: value.previous.as_ref().map(resolve).transpose()?,
        next: value.next.as_ref().map(resolve).transpose()?,
    })
}

fn record_transaction(
    history: &mut ContributionHistory,
    record: Option<Record>,
    revision: u64,
    dependency_changes: Vec<DependencyChange>,
    before: &TargetLedger,
    targets: &TargetLedger,
    created: &BTreeMap<String, SemanticTarget>,
) -> Result<(), String> {
    if let Some(record) = record {
        if let Some(structural) = record.structural {
            let created = structural
                .created
                .into_iter()
                .map(|object| {
                    let target = created
                        .get(&object.object)
                        .ok_or("structural object not created in transaction")?
                        .clone();
                    Ok(ObjectDescription {
                        dependencies: targets.dependencies(&target).map_err(error)?,
                        target,
                        payload: object.payload,
                        position: creation_position(&object.position, targets, created)?,
                    })
                })
                .collect::<Result<Vec<_>, String>>()?;
            let deleted = structural
                .deleted
                .into_iter()
                .map(|object| {
                    Ok(ObjectDescription {
                        dependencies: before.dependencies(&object.target).map_err(error)?,
                        target: object.target,
                        payload: object.payload,
                        position: object.position,
                    })
                })
                .collect::<Result<Vec<_>, String>>()?;
            history
                .record_transaction(
                    record.operation,
                    revision,
                    TransactionChanges {
                        properties: record.changes,
                        created,
                        deleted,
                        dependencies: dependency_changes,
                        reorders: structural.reorders,
                    },
                    before,
                    targets,
                )
                .map_err(error)?;
        } else {
            history
                .record(record.operation, revision, record.changes, targets)
                .map_err(error)?;
        }
    }
    Ok(())
}
