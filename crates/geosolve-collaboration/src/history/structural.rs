// SPDX-License-Identifier: GPL-3.0-or-later
//! Contribution-owned lifecycle and typed structural properties. No model snapshots.

use super::{
    Contribution, ContributionHistory, HistoryDirection, HistoryError, InversePlan,
    MAX_CONTRIBUTIONS, MAX_WRITES, OperationId, OwnedChange, PropertyAddress, PropertyChange,
    PropertyState, SemanticTarget, TargetLedger, validate_address, validate_value,
};
use crate::targets::DeletionPlan;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

const DEPENDENCIES: &str = "$geosolve:dependencies";
const POSITION: &str = "$geosolve:position";
pub(super) fn is_reserved(property: &str) -> bool {
    property.starts_with("$geosolve:")
}

/// Stable neighboring declarations, never a global statement index/checkpoint.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StatementPosition {
    pub previous: Option<SemanticTarget>,
    pub next: Option<SemanticTarget>,
}

/// Server-observed per-object compiler inverse descriptor. The host independently
/// recompiles its payload and neighbors; this record confers no geometry authority.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ObjectDescription {
    pub target: SemanticTarget,
    pub dependencies: Vec<SemanticTarget>,
    pub payload: serde_json::Value,
    pub position: StatementPosition,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DependencyChange {
    pub target: SemanticTarget,
    pub before: Vec<SemanticTarget>,
    pub after: Vec<SemanticTarget>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReorderChange {
    pub target: SemanticTarget,
    pub before: StatementPosition,
    pub after: StatementPosition,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TransactionChanges {
    #[serde(default)]
    pub properties: Vec<PropertyChange>,
    #[serde(default)]
    pub created: Vec<ObjectDescription>,
    #[serde(default)]
    pub deleted: Vec<ObjectDescription>,
    #[serde(default)]
    pub dependencies: Vec<DependencyChange>,
    #[serde(default)]
    pub reorders: Vec<ReorderChange>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TargetRemapping {
    pub before: SemanticTarget,
    pub after: SemanticTarget,
}

/// Independently replayable compiler intent. Target allocation is speculative
/// until the exact target/history pair is accepted and durably published.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StructuralInverse {
    pub create: Vec<ObjectDescription>,
    pub delete: Option<DeletionPlan>,
    pub dependencies: Vec<DependencyChange>,
    pub reorders: Vec<ReorderChange>,
    pub restored: Vec<TargetRemapping>,
}
impl StructuralInverse {
    pub fn is_empty(&self) -> bool {
        self.create.is_empty()
            && self.delete.is_none()
            && self.dependencies.is_empty()
            && self.reorders.is_empty()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DeletedObject {
    description: ObjectDescription,
    properties: Vec<PropertyState>,
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct LifecycleChanges {
    created: Vec<ObjectDescription>,
    deleted: Vec<DeletedObject>,
}
impl LifecycleChanges {
    pub(super) fn is_empty(&self) -> bool {
        self.created.is_empty() && self.deleted.is_empty()
    }
    pub(super) fn validate_links(
        &self,
        previous: &BTreeMap<OperationId, &Contribution>,
        changes: &[OwnedChange],
    ) -> Result<(), HistoryError> {
        for created in &self.created {
            for (property, value) in [
                (DEPENDENCIES, serde_json::to_value(&created.dependencies)),
                (POSITION, serde_json::to_value(&created.position)),
            ] {
                let value = value.map_err(|_| HistoryError::Invalid)?;
                if !changes.iter().any(|entry| {
                    entry.change.address.target == created.target
                        && entry.change.address.property == property
                        && entry.change.before == value
                        && entry.change.after == value
                        && entry.prior_owner.is_none()
                }) {
                    return Err(HistoryError::Invalid);
                }
            }
        }
        for deleted in &self.deleted {
            for property in &deleted.properties {
                let valid = if let Some(owner) = &property.owner {
                    previous.get(owner).is_some_and(|contribution| {
                        contribution.changes.iter().any(|entry| {
                            entry.change.address == property.address
                                && entry.change.after == property.value
                        })
                    })
                } else {
                    previous.values().any(|contribution| {
                        contribution.changes.iter().any(|entry| {
                            entry.change.address == property.address
                                && entry.change.before == property.value
                        })
                    })
                };
                if !valid {
                    return Err(HistoryError::Invalid);
                }
            }
        }
        Ok(())
    }
    pub(super) fn validate(&self) -> Result<(), HistoryError> {
        if self.created.len().saturating_add(self.deleted.len()) > MAX_WRITES {
            return Err(HistoryError::Limit);
        }
        let mut names = BTreeSet::new();
        for object in self
            .created
            .iter()
            .chain(self.deleted.iter().map(|entry| &entry.description))
        {
            validate_object(object)?;
            if !names.insert(&object.target.object) {
                return Err(HistoryError::Invalid);
            }
        }
        for deleted in &self.deleted {
            let mut seen = BTreeSet::new();
            for property in &deleted.properties {
                validate_address(&property.address)?;
                validate_value(&property.value)?;
                if property.address.target != deleted.description.target
                    || !seen.insert(&property.address)
                {
                    return Err(HistoryError::Invalid);
                }
            }
        }
        Ok(())
    }
}

impl ContributionHistory {
    /// Records the exact independently accepted structural/model transaction in
    /// the same personal timeline as properties. Before/after ledger replay must
    /// match completely, including explicit same-value dependency contributions.
    ///
    /// # Errors
    /// Rejects unrecorded lifecycle/dependency changes, stale observations and limits.
    pub fn record_transaction(
        &mut self,
        operation: OperationId,
        revision: u64,
        transaction: TransactionChanges,
        before: &TargetLedger,
        after: &TargetLedger,
    ) -> Result<(), HistoryError> {
        self.check_operation(&operation, revision)?;
        if transaction
            .properties
            .iter()
            .any(|change| is_reserved(&change.address.property))
        {
            return Err(HistoryError::Invalid);
        }
        let structural = LifecycleChanges {
            created: transaction.created.clone(),
            deleted: transaction
                .deleted
                .iter()
                .map(|description| DeletedObject {
                    description: description.clone(),
                    properties: self
                        .properties
                        .values()
                        .filter(|entry| entry.address.target == description.target)
                        .cloned()
                        .collect(),
                })
                .collect(),
        };
        structural.validate()?;
        for object in &transaction.deleted {
            for (property, observed) in [
                (DEPENDENCIES, serde_json::to_value(&object.dependencies)),
                (POSITION, serde_json::to_value(&object.position)),
            ] {
                let observed = observed.map_err(|_| HistoryError::Invalid)?;
                if self
                    .properties
                    .get(&PropertyAddress {
                        target: object.target.clone(),
                        property: property.into(),
                    })
                    .is_some_and(|state| state.value != observed)
                {
                    return Err(HistoryError::Stale);
                }
            }
        }
        validate_transaction_targets(&transaction, before, after)?;
        let mut changes = transaction.properties;
        for created in &structural.created {
            changes.push(encoded_change(
                &created.target,
                DEPENDENCIES,
                &created.dependencies,
                &created.dependencies,
            )?);
            changes.push(encoded_change(
                &created.target,
                POSITION,
                &created.position,
                &created.position,
            )?);
        }
        for dependency in transaction.dependencies {
            changes.push(encoded_change(
                &dependency.target,
                DEPENDENCIES,
                &dependency.before,
                &dependency.after,
            )?);
        }
        for reorder in transaction.reorders {
            validate_position(&reorder.target, &reorder.before)?;
            validate_position(&reorder.target, &reorder.after)?;
            authenticate_position(&reorder.after, after, &BTreeSet::new())?;
            changes.push(encoded_change(
                &reorder.target,
                POSITION,
                &reorder.before,
                &reorder.after,
            )?);
        }
        self.record_owned(operation, revision, changes, structural, after)
    }

    pub(super) fn record_owned(
        &mut self,
        operation: OperationId,
        revision: u64,
        changes: Vec<PropertyChange>,
        structural: LifecycleChanges,
        targets: &TargetLedger,
    ) -> Result<(), HistoryError> {
        self.check_operation(&operation, revision)?;
        if changes.is_empty() && structural.is_empty() {
            return Err(HistoryError::Limit);
        }
        self.check_changes(&changes, targets)?;
        if self.contributions.len() >= MAX_CONTRIBUTIONS {
            return Err(HistoryError::Limit);
        }
        let owned = changes
            .into_iter()
            .map(|change| OwnedChange {
                prior_owner: self
                    .properties
                    .get(&change.address)
                    .and_then(|state| state.owner.clone()),
                change,
            })
            .collect::<Vec<_>>();
        let mut staged = self.clone();
        for entry in &owned {
            staged.properties.insert(
                entry.change.address.clone(),
                PropertyState {
                    address: entry.change.address.clone(),
                    value: entry.change.after.clone(),
                    owner: Some(operation.clone()),
                },
            );
        }
        staged.redo.remove(&operation.user_id);
        staged.contributions.push(Contribution {
            operation: operation.clone(),
            changes: owned,
            active: true,
            structural,
        });
        staged.operations.insert(operation);
        staged.revision = revision;
        staged.check_size()?;
        *self = staged;
        Ok(())
    }

    /// Atomically records a model-validated inverse and its fresh target lifetimes.
    /// Failed validation before this call consumes no history or allocation; stale
    /// or conflicting input here leaves both supplied ledgers unchanged.
    ///
    /// # Errors
    /// Rejects foreign users, changed ownership/dependencies/lifetimes and limits.
    pub fn commit_inverse_transaction(
        &mut self,
        plan: &InversePlan,
        operation: OperationId,
        revision: u64,
        targets: &mut TargetLedger,
    ) -> Result<(), HistoryError> {
        self.check_operation(&operation, revision)?;
        if operation.user_id != plan.contribution.user_id {
            return Err(HistoryError::Invalid);
        }
        let current = match plan.direction {
            HistoryDirection::Undo => self.prepare_undo(&operation.user_id, targets)?,
            HistoryDirection::Redo => self.prepare_redo(&operation.user_id, targets)?,
        };
        if current.contribution != plan.contribution
            || current.all_changes != plan.all_changes
            || current.ownership != plan.ownership
            || current.structural != plan.structural
        {
            return Err(HistoryError::Stale);
        }
        let mut staged_targets = targets.clone();
        apply_structural(&plan.structural, &mut staged_targets)?;
        let mut staged = self.clone();
        staged.remap(&plan.structural.restored)?;
        let index = staged
            .contributions
            .iter()
            .position(|entry| entry.operation == plan.contribution)
            .ok_or(HistoryError::Invalid)?;
        let contribution = &mut staged.contributions[index];
        for entry in &contribution.changes {
            let (value, owner) = match plan.direction {
                HistoryDirection::Undo => (entry.change.before.clone(), entry.prior_owner.clone()),
                HistoryDirection::Redo => (
                    entry.change.after.clone(),
                    Some(contribution.operation.clone()),
                ),
            };
            staged.properties.insert(
                entry.change.address.clone(),
                PropertyState {
                    address: entry.change.address.clone(),
                    value,
                    owner,
                },
            );
        }
        contribution.active = plan.direction == HistoryDirection::Redo;
        match plan.direction {
            HistoryDirection::Undo => staged
                .redo
                .entry(operation.user_id.clone())
                .or_default()
                .push(plan.contribution.clone()),
            HistoryDirection::Redo => {
                staged
                    .redo
                    .get_mut(&operation.user_id)
                    .ok_or(HistoryError::Invalid)?
                    .pop();
            }
        }
        staged.operations.insert(operation);
        staged.revision = revision;
        staged.check_size()?;
        *self = staged;
        *targets = staged_targets;
        Ok(())
    }

    pub(super) fn prepare_transaction(
        &self,
        contribution: &Contribution,
        direction: HistoryDirection,
        targets: &TargetLedger,
    ) -> Result<InversePlan, HistoryError> {
        let restore = match direction {
            HistoryDirection::Undo => contribution
                .structural
                .deleted
                .iter()
                .map(|entry| entry.description.clone())
                .collect::<Vec<_>>(),
            HistoryDirection::Redo => contribution.structural.created.clone(),
        };
        let mut structural = StructuralInverse::default();
        let mut staged_targets = targets.clone();
        for object in &restore {
            staged_targets
                .authenticate_tombstone(&object.target)
                .map_err(|_| HistoryError::Lifetime)?;
            let fresh = staged_targets
                .create(&object.target.object)
                .map_err(|_| HistoryError::Lifetime)?;
            structural.restored.push(TargetRemapping {
                before: object.target.clone(),
                after: fresh,
            });
        }
        let mut staged = self.clone();
        staged.remap(&structural.restored)?;
        let contribution = staged
            .contributions
            .iter()
            .find(|entry| entry.operation == contribution.operation)
            .ok_or(HistoryError::Invalid)?;
        structural.create = match direction {
            HistoryDirection::Undo => contribution
                .structural
                .deleted
                .iter()
                .map(|entry| entry.description.clone())
                .collect(),
            HistoryDirection::Redo => contribution.structural.created.clone(),
        };
        let restored_set = structural
            .create
            .iter()
            .map(|entry| entry.target.clone())
            .collect::<BTreeSet<_>>();
        for object in &structural.create {
            staged_targets
                .set_dependencies(&object.target, &object.dependencies)
                .map_err(|_| HistoryError::Lifetime)?;
            authenticate_position(&object.position, &staged_targets, &restored_set)?;
        }
        let (all_changes, ownership) =
            staged.inverse_changes(contribution, direction, &staged_targets)?;
        prepare_structural_properties(
            &all_changes,
            &mut staged_targets,
            &restored_set,
            &mut structural,
        )?;
        structural.delete = staged.prepare_removal(contribution, direction, &staged_targets)?;
        let changes = all_changes
            .iter()
            .filter(|entry| !is_reserved(&entry.address.property))
            .cloned()
            .collect();
        Ok(InversePlan {
            contribution: contribution.operation.clone(),
            direction,
            changes,
            all_changes,
            ownership,
            structural,
        })
    }

    fn inverse_changes(
        &self,
        contribution: &Contribution,
        direction: HistoryDirection,
        targets: &TargetLedger,
    ) -> Result<(Vec<PropertyChange>, Vec<Option<OperationId>>), HistoryError> {
        let mut all_changes = Vec::new();
        let mut ownership = Vec::new();
        for owned in &contribution.changes {
            targets
                .authenticate(&owned.change.address.target)
                .map_err(|_| HistoryError::Lifetime)?;
            let state = self
                .properties
                .get(&owned.change.address)
                .ok_or(HistoryError::Invalid)?;
            let (before, after, owner) = match direction {
                HistoryDirection::Undo => (
                    &owned.change.after,
                    &owned.change.before,
                    Some(contribution.operation.clone()),
                ),
                HistoryDirection::Redo => (
                    &owned.change.before,
                    &owned.change.after,
                    owned.prior_owner.clone(),
                ),
            };
            if &state.value != before || state.owner != owner {
                return Err(HistoryError::Overwritten);
            }
            ownership.push(owner);
            all_changes.push(PropertyChange {
                address: owned.change.address.clone(),
                before: before.clone(),
                after: after.clone(),
            });
        }
        Ok((all_changes, ownership))
    }

    fn prepare_removal(
        &self,
        contribution: &Contribution,
        direction: HistoryDirection,
        targets: &TargetLedger,
    ) -> Result<Option<DeletionPlan>, HistoryError> {
        let remove = match direction {
            HistoryDirection::Undo => contribution
                .structural
                .created
                .iter()
                .map(|entry| entry.target.clone())
                .collect::<Vec<_>>(),
            HistoryDirection::Redo => contribution
                .structural
                .deleted
                .iter()
                .map(|entry| entry.description.target.clone())
                .collect(),
        };
        if !remove.is_empty() {
            let plan = targets
                .plan_delete(&remove)
                .map_err(|_| HistoryError::Lifetime)?;
            if plan.closure.iter().collect::<BTreeSet<_>>()
                != remove.iter().collect::<BTreeSet<_>>()
            {
                return Err(HistoryError::Overwritten);
            }
            for target in &remove {
                let observed = self
                    .properties
                    .values()
                    .filter(|entry| &entry.address.target == target)
                    .cloned()
                    .collect::<Vec<_>>();
                match direction {
                    HistoryDirection::Undo => {
                        if observed.iter().any(|entry| {
                            entry.owner.as_ref().is_some_and(|owner| {
                                owner.user_id != contribution.operation.user_id
                            })
                        }) {
                            return Err(HistoryError::Overwritten);
                        }
                    }
                    HistoryDirection::Redo => {
                        let recorded = contribution
                            .structural
                            .deleted
                            .iter()
                            .find(|entry| &entry.description.target == target)
                            .ok_or(HistoryError::Invalid)?;
                        if !same_effective_properties(&recorded.properties, &observed) {
                            return Err(HistoryError::Overwritten);
                        }
                    }
                }
            }
            return Ok(Some(plan));
        }
        Ok(None)
    }

    fn remap(&mut self, mappings: &[TargetRemapping]) -> Result<(), HistoryError> {
        let map = mappings
            .iter()
            .map(|entry| (entry.before.clone(), entry.after.clone()))
            .collect::<BTreeMap<_, _>>();
        if map.len() != mappings.len() {
            return Err(HistoryError::Invalid);
        }
        for contribution in &mut self.contributions {
            for owned in &mut contribution.changes {
                remap_change(&mut owned.change, &map)?;
            }
            for object in &mut contribution.structural.created {
                remap_object(object, &map);
            }
            for deleted in &mut contribution.structural.deleted {
                remap_object(&mut deleted.description, &map);
                for property in &mut deleted.properties {
                    remap_property(property, &map)?;
                }
                deleted
                    .properties
                    .sort_by(|left, right| left.address.cmp(&right.address));
            }
        }
        let mut properties = BTreeMap::new();
        for mut property in self.properties.values().cloned() {
            remap_property(&mut property, &map)?;
            if properties
                .insert(property.address.clone(), property)
                .is_some()
            {
                return Err(HistoryError::Invalid);
            }
        }
        self.properties = properties;
        Ok(())
    }
}

fn validate_transaction_targets(
    transaction: &TransactionChanges,
    before: &TargetLedger,
    after: &TargetLedger,
) -> Result<(), HistoryError> {
    let mut replay = before.clone();
    if !transaction.deleted.is_empty() {
        let roots = transaction
            .deleted
            .iter()
            .map(|entry| entry.target.clone())
            .collect::<Vec<_>>();
        let plan = before
            .plan_delete(&roots)
            .map_err(|_| HistoryError::Lifetime)?;
        if roots.iter().collect::<BTreeSet<_>>() != plan.closure.iter().collect::<BTreeSet<_>>() {
            return Err(HistoryError::Stale);
        }
        for deleted in &transaction.deleted {
            if before
                .dependencies(&deleted.target)
                .map_err(|_| HistoryError::Lifetime)?
                != deleted.dependencies
            {
                return Err(HistoryError::Stale);
            }
            after
                .authenticate_tombstone(&deleted.target)
                .map_err(|_| HistoryError::Lifetime)?;
            authenticate_position(&deleted.position, before, &BTreeSet::new())?;
        }
        replay.delete(&plan).map_err(|_| HistoryError::Lifetime)?;
    }
    let mut creations = transaction.created.iter().collect::<Vec<_>>();
    creations.sort_by_key(|entry| entry.target.generation);
    for created in &creations {
        if replay
            .create(&created.target.object)
            .map_err(|_| HistoryError::Lifetime)?
            != created.target
        {
            return Err(HistoryError::Lifetime);
        }
    }
    for created in creations {
        replay
            .set_dependencies(&created.target, &created.dependencies)
            .map_err(|_| HistoryError::Lifetime)?;
        authenticate_position(&created.position, after, &BTreeSet::new())?;
    }
    let mut dependencies = BTreeSet::new();
    for change in &transaction.dependencies {
        if !dependencies.insert(&change.target)
            || transaction
                .created
                .iter()
                .any(|entry| entry.target == change.target)
            || before
                .dependencies(&change.target)
                .map_err(|_| HistoryError::Lifetime)?
                != change.before
        {
            return Err(HistoryError::Stale);
        }
        if !change.after.windows(2).all(|pair| pair[0] < pair[1]) {
            return Err(HistoryError::Invalid);
        }
        replay
            .set_dependencies(&change.target, &change.after)
            .map_err(|_| HistoryError::Lifetime)?;
    }
    if replay.to_json().map_err(|_| HistoryError::Invalid)?
        != after.to_json().map_err(|_| HistoryError::Invalid)?
    {
        return Err(HistoryError::Stale);
    }
    Ok(())
}

fn apply_structural(
    plan: &StructuralInverse,
    targets: &mut TargetLedger,
) -> Result<(), HistoryError> {
    for mapping in &plan.restored {
        targets
            .authenticate_tombstone(&mapping.before)
            .map_err(|_| HistoryError::Lifetime)?;
        if targets
            .create(&mapping.after.object)
            .map_err(|_| HistoryError::Lifetime)?
            != mapping.after
        {
            return Err(HistoryError::Stale);
        }
    }
    for object in &plan.create {
        targets
            .set_dependencies(&object.target, &object.dependencies)
            .map_err(|_| HistoryError::Lifetime)?;
    }
    for dependency in &plan.dependencies {
        targets
            .set_dependencies(&dependency.target, &dependency.after)
            .map_err(|_| HistoryError::Lifetime)?;
    }
    if let Some(deletion) = &plan.delete {
        targets
            .delete(deletion)
            .map_err(|_| HistoryError::Lifetime)?;
    }
    Ok(())
}
fn encoded_change<T: Serialize>(
    target: &SemanticTarget,
    property: &str,
    before: &T,
    after: &T,
) -> Result<PropertyChange, HistoryError> {
    Ok(PropertyChange {
        address: PropertyAddress {
            target: target.clone(),
            property: property.into(),
        },
        before: serde_json::to_value(before).map_err(|_| HistoryError::Invalid)?,
        after: serde_json::to_value(after).map_err(|_| HistoryError::Invalid)?,
    })
}
fn decode<T: serde::de::DeserializeOwned>(value: &serde_json::Value) -> Result<T, HistoryError> {
    serde_json::from_value(value.clone()).map_err(|_| HistoryError::Invalid)
}
fn validate_target(target: &SemanticTarget) -> Result<(), HistoryError> {
    validate_address(&PropertyAddress {
        target: target.clone(),
        property: "target".into(),
    })
}
fn validate_position(
    target: &SemanticTarget,
    position: &StatementPosition,
) -> Result<(), HistoryError> {
    for neighbor in position.previous.iter().chain(position.next.iter()) {
        validate_target(neighbor)?;
        if neighbor == target {
            return Err(HistoryError::Invalid);
        }
    }
    if position.previous.is_some() && position.previous == position.next {
        return Err(HistoryError::Invalid);
    }
    Ok(())
}
fn authenticate_position(
    position: &StatementPosition,
    targets: &TargetLedger,
    restored: &BTreeSet<SemanticTarget>,
) -> Result<(), HistoryError> {
    for neighbor in position.previous.iter().chain(position.next.iter()) {
        if !restored.contains(neighbor) {
            targets
                .authenticate(neighbor)
                .map_err(|_| HistoryError::Lifetime)?;
        }
    }
    Ok(())
}
fn validate_object(object: &ObjectDescription) -> Result<(), HistoryError> {
    validate_target(&object.target)?;
    validate_value(&object.payload)?;
    validate_position(&object.target, &object.position)?;
    let mut seen = BTreeSet::new();
    if !object.dependencies.windows(2).all(|pair| pair[0] < pair[1]) {
        return Err(HistoryError::Invalid);
    }
    for dependency in &object.dependencies {
        validate_target(dependency)?;
        if dependency == &object.target || !seen.insert(dependency) {
            return Err(HistoryError::Invalid);
        }
    }
    Ok(())
}
fn remap_target(target: &mut SemanticTarget, map: &BTreeMap<SemanticTarget, SemanticTarget>) {
    if let Some(fresh) = map.get(target) {
        *target = fresh.clone();
    }
}
fn remap_position(
    position: &mut StatementPosition,
    map: &BTreeMap<SemanticTarget, SemanticTarget>,
) {
    for target in position.previous.iter_mut().chain(position.next.iter_mut()) {
        remap_target(target, map);
    }
}
fn remap_object(object: &mut ObjectDescription, map: &BTreeMap<SemanticTarget, SemanticTarget>) {
    remap_target(&mut object.target, map);
    for target in &mut object.dependencies {
        remap_target(target, map);
    }
    object.dependencies.sort();
    remap_position(&mut object.position, map);
}
fn remap_value(
    property: &str,
    value: &mut serde_json::Value,
    map: &BTreeMap<SemanticTarget, SemanticTarget>,
) -> Result<(), HistoryError> {
    match property {
        DEPENDENCIES => {
            let mut targets: Vec<SemanticTarget> = decode(value)?;
            for target in &mut targets {
                remap_target(target, map);
            }
            targets.sort();
            *value = serde_json::to_value(targets).map_err(|_| HistoryError::Invalid)?;
        }
        POSITION => {
            let mut position: StatementPosition = decode(value)?;
            remap_position(&mut position, map);
            *value = serde_json::to_value(position).map_err(|_| HistoryError::Invalid)?;
        }
        _ => {}
    }
    Ok(())
}
fn remap_change(
    change: &mut PropertyChange,
    map: &BTreeMap<SemanticTarget, SemanticTarget>,
) -> Result<(), HistoryError> {
    remap_target(&mut change.address.target, map);
    remap_value(&change.address.property, &mut change.before, map)?;
    remap_value(&change.address.property, &mut change.after, map)
}
fn remap_property(
    property: &mut PropertyState,
    map: &BTreeMap<SemanticTarget, SemanticTarget>,
) -> Result<(), HistoryError> {
    remap_target(&mut property.address.target, map);
    remap_value(&property.address.property, &mut property.value, map)
}

fn prepare_structural_properties(
    changes: &[PropertyChange],
    targets: &mut TargetLedger,
    restored_set: &BTreeSet<SemanticTarget>,
    structural: &mut StructuralInverse,
) -> Result<(), HistoryError> {
    for change in changes {
        match change.address.property.as_str() {
            DEPENDENCIES => {
                let entry = DependencyChange {
                    target: change.address.target.clone(),
                    before: decode(&change.before)?,
                    after: decode(&change.after)?,
                };
                if targets
                    .dependencies(&entry.target)
                    .map_err(|_| HistoryError::Lifetime)?
                    != entry.before
                {
                    return Err(HistoryError::Overwritten);
                }
                targets
                    .set_dependencies(&entry.target, &entry.after)
                    .map_err(|_| HistoryError::Lifetime)?;
                structural.dependencies.push(entry);
            }
            POSITION => {
                let entry = ReorderChange {
                    target: change.address.target.clone(),
                    before: decode(&change.before)?,
                    after: decode(&change.after)?,
                };
                authenticate_position(&entry.after, targets, restored_set)?;
                structural.reorders.push(entry);
            }
            _ => {}
        }
    }
    Ok(())
}

fn same_effective_properties(recorded: &[PropertyState], observed: &[PropertyState]) -> bool {
    let recorded = recorded
        .iter()
        .map(|entry| (&entry.address, entry))
        .collect::<BTreeMap<_, _>>();
    let observed = observed
        .iter()
        .map(|entry| (&entry.address, entry))
        .collect::<BTreeMap<_, _>>();
    // First observation of a previously untracked property may add its baseline.
    // Once that contribution is undone, an ownerless baseline is not another
    // author's effective contribution and must not permanently block Redo.
    recorded
        .iter()
        .all(|(address, entry)| observed.get(address) == Some(entry))
        && observed
            .iter()
            .all(|(address, entry)| entry.owner.is_none() || recorded.get(address) == Some(entry))
}

pub(super) fn validate_reserved_change(change: &PropertyChange) -> Result<(), HistoryError> {
    for value in [&change.before, &change.after] {
        match change.address.property.as_str() {
            DEPENDENCIES => {
                let dependencies: Vec<SemanticTarget> = decode(value)?;
                if !dependencies.windows(2).all(|pair| pair[0] < pair[1]) {
                    return Err(HistoryError::Invalid);
                }
                for dependency in dependencies {
                    validate_target(&dependency)?;
                    if dependency == change.address.target {
                        return Err(HistoryError::Invalid);
                    }
                }
            }
            POSITION => validate_position(&change.address.target, &decode(value)?)?,
            property if is_reserved(property) => return Err(HistoryError::Invalid),
            _ => {}
        }
    }
    Ok(())
}
