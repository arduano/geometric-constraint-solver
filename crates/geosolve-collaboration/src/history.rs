// SPDX-License-Identifier: GPL-3.0-or-later
//! Checked per-user semantic contribution history, independent from domain Undo.
//!
//! Values here are authenticated host observations after domain validation. This
//! module never solves, derives inverse geometry, or accepts client coordinates.
//! Stage on a clone and durably publish with the corresponding model transaction.

use crate::{
    protocol::{MAX_REVISION, OperationId},
    targets::{SemanticTarget, TargetLedger},
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

mod structural;
pub use structural::{
    DependencyChange, ObjectDescription, ReorderChange, StatementPosition, StructuralInverse,
    TargetRemapping, TransactionChanges,
};

const FORMAT: &str = "geosolve-contribution-history-v1";
const MAX_BYTES: usize = 64 * 1024 * 1024;
const MAX_VALUE_BYTES: usize = 1024 * 1024;
const MAX_WRITES: usize = 1024;
const MAX_CONTRIBUTIONS: usize = 16_384;

/// Opaque canonical property coordinate supplied by the shared authoring adapter.
/// It includes full semantic keyed path; never derive it from display labels.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PropertyAddress {
    pub target: SemanticTarget,
    pub property: String,
}

/// Exact before/after values captured by the server against its latest input.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PropertyChange {
    pub address: PropertyAddress,
    pub before: serde_json::Value,
    pub after: serde_json::Value,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PropertyState {
    address: PropertyAddress,
    value: serde_json::Value,
    owner: Option<OperationId>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct OwnedChange {
    change: PropertyChange,
    prior_owner: Option<OperationId>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Contribution {
    operation: OperationId,
    changes: Vec<OwnedChange>,
    active: bool,
    #[serde(
        default,
        skip_serializing_if = "structural::LifecycleChanges::is_empty"
    )]
    structural: structural::LifecycleChanges,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HistoryDirection {
    Undo,
    Redo,
}

/// Read-only personal history availability, without retained inverse tickets.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserSemanticHistory {
    pub undo_count: usize,
    pub redo_count: usize,
    pub can_undo: bool,
    pub can_redo: bool,
    pub undo_unavailable: Option<String>,
    pub redo_unavailable: Option<String>,
}

/// Opaque prepared inverse. Host validates the proposed changes independently
/// before committing this plan against the same live contribution history.
#[derive(Clone, Debug)]
pub struct InversePlan {
    contribution: OperationId,
    direction: HistoryDirection,
    changes: Vec<PropertyChange>,
    ownership: Vec<Option<OperationId>>,
    all_changes: Vec<PropertyChange>,
    structural: StructuralInverse,
}
impl InversePlan {
    pub fn contribution(&self) -> &OperationId {
        &self.contribution
    }
    pub fn direction(&self) -> HistoryDirection {
        self.direction
    }
    pub fn changes(&self) -> &[PropertyChange] {
        &self.changes
    }
    pub fn structural(&self) -> &StructuralInverse {
        &self.structural
    }
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum HistoryError {
    #[error("invalid semantic history input")]
    Invalid,
    #[error("semantic contribution history resource limit")]
    Limit,
    #[error("operation has already been recorded")]
    Duplicate,
    #[error("semantic history revision must advance in server order")]
    Revision,
    #[error("no contribution is available for this user's requested history action")]
    Empty,
    #[error("another contribution now owns this value; inverse unavailable")]
    Overwritten,
    #[error("target was deleted or recreated; inverse unavailable")]
    Lifetime,
    #[error("prepared inverse no longer matches the live contribution history")]
    Stale,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct HistorySnapshot {
    format: String,
    revision: u64,
    properties: Vec<PropertyState>,
    contributions: Vec<Contribution>,
    redo: BTreeMap<String, Vec<OperationId>>,
    operations: Vec<OperationId>,
}

/// Tracks effective ownership separately from monotonically increasing commits.
/// Undoing one's latest write restores the preceding effective owner, allowing
/// repeated personal Undo without mistaking the inverse for somebody else's edit.
#[derive(Clone, Debug, Default)]
pub struct ContributionHistory {
    revision: u64,
    properties: BTreeMap<PropertyAddress, PropertyState>,
    contributions: Vec<Contribution>,
    redo: BTreeMap<String, Vec<OperationId>>,
    operations: BTreeSet<OperationId>,
}

impl ContributionHistory {
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Records an independently accepted property batch. Same-value writes still
    /// acquire contribution ownership, even when the domain needs no solve.
    ///
    /// # Errors
    /// Rejects stale before values/lifetimes, duplicates, bad revisions and limits.
    pub fn record(
        &mut self,
        operation: OperationId,
        revision: u64,
        changes: Vec<PropertyChange>,
        targets: &TargetLedger,
    ) -> Result<(), HistoryError> {
        if changes
            .iter()
            .any(|change| structural::is_reserved(&change.address.property))
        {
            return Err(HistoryError::Invalid);
        }
        self.record_owned(
            operation,
            revision,
            changes,
            structural::LifecycleChanges::default(),
            targets,
        )
    }

    /// Picks this user's latest active contribution without skipping an unavailable
    /// newer inverse. Failure is truthful rather than silently undoing an older edit.
    ///
    /// # Errors
    /// Rejects absent, overwritten or deleted/recreated contribution targets.
    pub fn prepare_undo(
        &self,
        user_id: &str,
        targets: &TargetLedger,
    ) -> Result<InversePlan, HistoryError> {
        let contribution = self
            .contributions
            .iter()
            .rev()
            .find(|entry| entry.active && entry.operation.user_id == user_id)
            .ok_or(HistoryError::Empty)?;
        self.prepare(contribution, HistoryDirection::Undo, targets)
    }

    /// # Errors
    /// Rejects absent, overwritten or deleted/recreated Redo contributions.
    pub fn prepare_redo(
        &self,
        user_id: &str,
        targets: &TargetLedger,
    ) -> Result<InversePlan, HistoryError> {
        let operation = self
            .redo
            .get(user_id)
            .and_then(|stack| stack.last())
            .ok_or(HistoryError::Empty)?;
        let contribution = self
            .contributions
            .iter()
            .find(|entry| &entry.operation == operation && !entry.active)
            .ok_or(HistoryError::Invalid)?;
        self.prepare(contribution, HistoryDirection::Redo, targets)
    }

    /// Records a validated inverse as a new durable server commit without calling
    /// the domain's global checkpoint Undo. Unrelated intervening writes survive.
    ///
    /// # Errors
    /// Rejects foreign-user, stale/overwritten inverses and invalid commit order.
    pub fn commit_inverse(
        &mut self,
        plan: &InversePlan,
        operation: OperationId,
        revision: u64,
        targets: &TargetLedger,
    ) -> Result<(), HistoryError> {
        if !plan.structural.is_empty() {
            return Err(HistoryError::Invalid);
        }
        self.commit_inverse_transaction(plan, operation, revision, &mut targets.clone())
    }

    /// Observes current committed ownership without consuming history or handles.
    pub fn user_history(&self, user: &str, targets: &TargetLedger) -> UserSemanticHistory {
        let undo_unavailable = self
            .prepare_undo(user, targets)
            .err()
            .map(|error| error.to_string());
        let redo_unavailable = self
            .prepare_redo(user, targets)
            .err()
            .map(|error| error.to_string());
        UserSemanticHistory {
            undo_count: self
                .contributions
                .iter()
                .filter(|entry| entry.active && entry.operation.user_id == user)
                .count(),
            redo_count: self.redo.get(user).map_or(0, Vec::len),
            can_undo: undo_unavailable.is_none(),
            can_redo: redo_unavailable.is_none(),
            undo_unavailable,
            redo_unavailable,
        }
    }

    /// Authoritative observation useful for UI availability and checked host edits.
    pub fn property_owner(&self, address: &PropertyAddress) -> Option<&OperationId> {
        self.properties
            .get(address)
            .and_then(|state| state.owner.as_ref())
    }

    /// # Errors
    /// Rejects transport exceeding 64 MiB or unexpected serialization failure.
    pub fn to_json(&self) -> Result<String, HistoryError> {
        let snapshot = HistorySnapshot {
            format: FORMAT.into(),
            revision: self.revision,
            properties: self.properties.values().cloned().collect(),
            contributions: self.contributions.clone(),
            redo: self.redo.clone(),
            operations: self.operations.iter().cloned().collect(),
        };
        let json = serde_json::to_string(&snapshot).map_err(|_| HistoryError::Invalid)?;
        if json.len() > MAX_BYTES {
            return Err(HistoryError::Limit);
        }
        Ok(json)
    }

    /// Restores authored contribution state from authenticated durable storage.
    /// Structural checks do not replace the host's journal integrity or model replay.
    ///
    /// # Errors
    /// Rejects invalid owner links, duplicate operations/properties and resource bounds.
    pub fn from_json(json: &str) -> Result<Self, HistoryError> {
        if json.len() > MAX_BYTES {
            return Err(HistoryError::Limit);
        }
        let snapshot: HistorySnapshot =
            serde_json::from_str(json).map_err(|_| HistoryError::Invalid)?;
        if snapshot.format != FORMAT
            || snapshot.revision > MAX_REVISION
            || snapshot.contributions.len() > MAX_CONTRIBUTIONS
            || snapshot.operations.len() > MAX_CONTRIBUTIONS * 8
            || snapshot.operations.len() as u64 > snapshot.revision
            || snapshot.properties.len() > MAX_CONTRIBUTIONS * MAX_WRITES
        {
            return Err(HistoryError::Invalid);
        }
        let mut state = Self {
            revision: snapshot.revision,
            contributions: snapshot.contributions,
            redo: snapshot.redo,
            ..Self::default()
        };
        for operation in snapshot.operations {
            validate_operation(&operation)?;
            if !state.operations.insert(operation) {
                return Err(HistoryError::Invalid);
            }
        }
        let mut previous = BTreeMap::<OperationId, &Contribution>::new();
        for entry in &state.contributions {
            if !state.operations.contains(&entry.operation)
                || previous.contains_key(&entry.operation)
                || (entry.changes.is_empty() && entry.structural.is_empty())
                || entry.changes.len() > MAX_WRITES
            {
                return Err(HistoryError::Invalid);
            }
            entry.structural.validate()?;
            entry.structural.validate_links(&previous, &entry.changes)?;
            let mut seen = BTreeSet::new();
            for owned in &entry.changes {
                validate_change(&owned.change)?;
                if !seen.insert(owned.change.address.clone()) {
                    return Err(HistoryError::Invalid);
                }
                if let Some(owner) = &owned.prior_owner {
                    let prior = previous.get(owner).ok_or(HistoryError::Invalid)?;
                    if !prior.changes.iter().any(|item| {
                        item.change.address == owned.change.address
                            && item.change.after == owned.change.before
                    }) {
                        return Err(HistoryError::Invalid);
                    }
                }
            }
            previous.insert(entry.operation.clone(), entry);
        }
        for property in snapshot.properties {
            validate_address(&property.address)?;
            validate_value(&property.value)?;
            if let Some(owner) = &property.owner {
                let contribution = previous.get(owner).ok_or(HistoryError::Invalid)?;
                if !contribution.active
                    || !contribution.changes.iter().any(|owned| {
                        owned.change.address == property.address
                            && owned.change.after == property.value
                    })
                {
                    return Err(HistoryError::Invalid);
                }
            }
            let address = property.address.clone();
            if state.properties.insert(address, property).is_some() {
                return Err(HistoryError::Invalid);
            }
        }
        state.check_property_restoration()?;
        for (user, stack) in &state.redo {
            let mut seen = BTreeSet::new();
            for operation in stack {
                let contribution = previous.get(operation).ok_or(HistoryError::Invalid)?;
                if operation.user_id != *user || contribution.active || !seen.insert(operation) {
                    return Err(HistoryError::Invalid);
                }
            }
        }
        Ok(state)
    }

    fn check_property_restoration(&self) -> Result<(), HistoryError> {
        // Every touched property must retain its baseline even after all writes
        // have been undone. The last active contribution owns it; a same-value
        // write cannot disappear from restoration merely because values match.
        let mut expected_properties = BTreeMap::new();
        for contribution in &self.contributions {
            for owned in &contribution.changes {
                let address = &owned.change.address;
                let property = expected_properties
                    .entry(address.clone())
                    .or_insert_with(|| PropertyState {
                        address: address.clone(),
                        value: owned.change.before.clone(),
                        owner: None,
                    });
                if contribution.active {
                    property.value.clone_from(&owned.change.after);
                    property.owner = Some(contribution.operation.clone());
                }
            }
        }
        if self.properties != expected_properties {
            return Err(HistoryError::Invalid);
        }
        Ok(())
    }

    fn prepare(
        &self,
        contribution: &Contribution,
        direction: HistoryDirection,
        targets: &TargetLedger,
    ) -> Result<InversePlan, HistoryError> {
        self.prepare_transaction(contribution, direction, targets)
    }
    fn check_operation(&self, operation: &OperationId, revision: u64) -> Result<(), HistoryError> {
        validate_operation(operation)?;
        if self.operations.contains(operation) {
            return Err(HistoryError::Duplicate);
        }
        if revision <= self.revision || revision > MAX_REVISION {
            return Err(HistoryError::Revision);
        }
        if self.operations.len() >= MAX_CONTRIBUTIONS * 8 {
            return Err(HistoryError::Limit);
        }
        Ok(())
    }
    fn check_changes(
        &self,
        changes: &[PropertyChange],
        targets: &TargetLedger,
    ) -> Result<(), HistoryError> {
        if changes.len() > MAX_WRITES {
            return Err(HistoryError::Limit);
        }
        let mut seen = BTreeSet::new();
        for change in changes {
            validate_change(change)?;
            targets
                .authenticate(&change.address.target)
                .map_err(|_| HistoryError::Lifetime)?;
            if !seen.insert(&change.address) {
                return Err(HistoryError::Invalid);
            }
            if self
                .properties
                .get(&change.address)
                .is_some_and(|state| state.value != change.before)
            {
                return Err(HistoryError::Stale);
            }
        }
        Ok(())
    }
    fn check_size(&self) -> Result<(), HistoryError> {
        self.to_json().map(|_| ())
    }
}

fn validate_operation(operation: &OperationId) -> Result<(), HistoryError> {
    for value in [
        &operation.user_id,
        &operation.client_id,
        &operation.request_id,
    ] {
        if value.is_empty()
            || value.len() > 128
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"_-.:".contains(&byte))
        {
            return Err(HistoryError::Invalid);
        }
    }
    Ok(())
}
fn validate_address(address: &PropertyAddress) -> Result<(), HistoryError> {
    if address.target.object.is_empty()
        || address.target.object.len() > 512
        || address.target.object.chars().any(char::is_control)
        || address.target.generation == 0
        || address.target.generation > MAX_REVISION
        || address.property.is_empty()
        || address.property.len() > 2048
        || address.property.chars().any(char::is_control)
    {
        return Err(HistoryError::Invalid);
    }
    Ok(())
}
fn validate_value(value: &serde_json::Value) -> Result<(), HistoryError> {
    if serde_json::to_vec(value)
        .map_err(|_| HistoryError::Invalid)?
        .len()
        > MAX_VALUE_BYTES
    {
        return Err(HistoryError::Limit);
    }
    Ok(())
}
fn validate_change(change: &PropertyChange) -> Result<(), HistoryError> {
    structural::validate_reserved_change(change)?;
    validate_address(&change.address)?;
    validate_value(&change.before)?;
    validate_value(&change.after)
}
