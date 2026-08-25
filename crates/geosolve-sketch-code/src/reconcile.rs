// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::{BTreeMap, BTreeSet};

use geosolve_sketch_intent::intent_content_digest;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::ManagedValue;

const MAX_GENERATED_MEMBERS: usize = 65_536;
const MAX_ADDRESS_SEGMENTS: usize = 64;
const MAX_ADDRESS_KEY_BYTES: usize = 256;

/// Semantic identity coordinate of one generated output. Ordinal graph IDs
/// and presentation order are deliberately absent.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GeneratedMemberAddress {
    pub invocation: String,
    pub template: Vec<String>,
    pub member_key: Vec<String>,
    pub output: Vec<String>,
}

impl GeneratedMemberAddress {
    #[must_use]
    pub fn new(
        invocation: impl Into<String>,
        template: impl IntoIterator<Item = impl Into<String>>,
        member_key: impl IntoIterator<Item = impl Into<String>>,
        output: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        Self {
            invocation: invocation.into(),
            template: template.into_iter().map(Into::into).collect(),
            member_key: member_key.into_iter().map(Into::into).collect(),
            output: output.into_iter().map(Into::into).collect(),
        }
    }

    #[must_use]
    pub fn display_path(&self) -> String {
        format!(
            "{}/{}/{}/{}",
            self.invocation,
            self.template.join("/"),
            self.member_key.join("/"),
            self.output.join("/")
        )
    }
}

/// Never-reused allocation plus the visible generation of one semantic path.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GeneratedMemberIdentity {
    pub allocation: u64,
    pub generation: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReconciledMember {
    pub address: GeneratedMemberAddress,
    pub identity: GeneratedMemberIdentity,
}

/// Explicit user placement layered over one still-owned generated output.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GeneratedOverride {
    pub address: GeneratedMemberAddress,
    pub identity: GeneratedMemberIdentity,
    pub value: ManagedValue,
}

/// Durable keyed identity state. `order` is presentation-only; active member
/// identity is keyed solely by [`GeneratedMemberAddress`].
#[derive(Clone, Debug, PartialEq)]
pub struct KeyedReconcileState {
    revision: u64,
    high_water: u64,
    /// Greatest generation ever issued for each semantic address, including
    /// generations observed only on an Undo/Redo branch. Unlike a tombstone,
    /// this authority intentionally survives restoring an older active
    /// generation of the same address.
    generation_high_water: BTreeMap<GeneratedMemberAddress, u32>,
    active: BTreeMap<GeneratedMemberAddress, GeneratedMemberIdentity>,
    tombstones: BTreeMap<GeneratedMemberAddress, GeneratedMemberIdentity>,
    order: Vec<GeneratedMemberAddress>,
    overrides: BTreeMap<GeneratedMemberIdentity, GeneratedOverride>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct KeyedReconcileWire {
    revision: u64,
    high_water: u64,
    generation_high_water: Vec<(GeneratedMemberAddress, u32)>,
    active: Vec<(GeneratedMemberAddress, GeneratedMemberIdentity)>,
    tombstones: Vec<(GeneratedMemberAddress, GeneratedMemberIdentity)>,
    order: Vec<GeneratedMemberAddress>,
    overrides: Vec<(GeneratedMemberIdentity, GeneratedOverride)>,
}

impl Serialize for KeyedReconcileState {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        KeyedReconcileWire {
            revision: self.revision,
            high_water: self.high_water,
            generation_high_water: self
                .generation_high_water
                .iter()
                .map(|(address, generation)| (address.clone(), *generation))
                .collect(),
            active: self
                .active
                .iter()
                .map(|(address, identity)| (address.clone(), *identity))
                .collect(),
            tombstones: self
                .tombstones
                .iter()
                .map(|(address, identity)| (address.clone(), *identity))
                .collect(),
            order: self.order.clone(),
            overrides: self
                .overrides
                .iter()
                .map(|(identity, value)| (*identity, value.clone()))
                .collect(),
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for KeyedReconcileState {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let wire = KeyedReconcileWire::deserialize(deserializer)?;
        let generation_high_water_len = wire.generation_high_water.len();
        let active_len = wire.active.len();
        let tombstone_len = wire.tombstones.len();
        let override_len = wire.overrides.len();
        let state = Self {
            revision: wire.revision,
            high_water: wire.high_water,
            generation_high_water: wire.generation_high_water.into_iter().collect(),
            active: wire.active.into_iter().collect(),
            tombstones: wire.tombstones.into_iter().collect(),
            order: wire.order,
            overrides: wire.overrides.into_iter().collect(),
        };
        if state.generation_high_water.len() != generation_high_water_len
            || state.active.len() != active_len
            || state.tombstones.len() != tombstone_len
            || state.overrides.len() != override_len
        {
            return Err(serde::de::Error::custom(
                "duplicate keyed reconciliation entry",
            ));
        }
        state.validate().map_err(serde::de::Error::custom)?;
        Ok(state)
    }
}

impl Default for KeyedReconcileState {
    fn default() -> Self {
        Self::empty()
    }
}

impl KeyedReconcileState {
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            revision: 0,
            high_water: 0,
            generation_high_water: BTreeMap::new(),
            active: BTreeMap::new(),
            tombstones: BTreeMap::new(),
            order: Vec::new(),
            overrides: BTreeMap::new(),
        }
    }

    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.revision
    }

    #[must_use]
    pub const fn high_water(&self) -> u64 {
        self.high_water
    }

    /// Greatest generation ever allocated for one semantic address, including
    /// generations which currently exist only on an abandoned history branch.
    #[must_use]
    pub fn generation_high_water(&self, address: &GeneratedMemberAddress) -> Option<u32> {
        self.generation_high_water.get(address).copied()
    }

    #[must_use]
    pub const fn active(&self) -> &BTreeMap<GeneratedMemberAddress, GeneratedMemberIdentity> {
        &self.active
    }

    #[must_use]
    pub const fn tombstones(&self) -> &BTreeMap<GeneratedMemberAddress, GeneratedMemberIdentity> {
        &self.tombstones
    }

    #[must_use]
    pub fn ordered_members(&self) -> Vec<ReconciledMember> {
        self.order
            .iter()
            .filter_map(|address| {
                self.active
                    .get(address)
                    .copied()
                    .map(|identity| ReconciledMember {
                        address: address.clone(),
                        identity,
                    })
            })
            .collect()
    }

    #[must_use]
    /// Computes a deterministic digest of the validated keyed state.
    ///
    /// # Panics
    ///
    /// Panics only if serialization of this closed in-memory schema fails.
    pub fn identity(&self) -> String {
        let bytes = serde_json::to_vec(self)
            .expect("closed keyed reconciliation state is infallibly serializable");
        intent_content_digest(&bytes).to_string()
    }

    /// Stages a complete requested keyed expansion against exact current
    /// identity. Removed identities with outside dependents reject before any
    /// tombstone or allocation becomes authoritative.
    ///
    /// # Errors
    ///
    /// Returns a typed address, duplicate, dependency, resource, persistence
    /// or allocator-exhaustion error.
    pub fn plan(
        &self,
        requested: Vec<GeneratedMemberAddress>,
        outside_dependents: &BTreeSet<GeneratedMemberIdentity>,
    ) -> Result<KeyedReconcilePlan, KeyedReconcileError> {
        self.validate()?;
        if requested.len() > MAX_GENERATED_MEMBERS {
            return Err(KeyedReconcileError::ResourceLimit {
                actual: requested.len(),
                limit: MAX_GENERATED_MEMBERS,
            });
        }
        let mut unique = BTreeSet::new();
        for address in &requested {
            validate_address(address)?;
            if !unique.insert(address.clone()) {
                return Err(KeyedReconcileError::DuplicateAddress(
                    address.display_path(),
                ));
            }
        }

        let mut staged = self.clone();
        let mut created = Vec::new();
        let mut retained = Vec::new();
        let mut removed = Vec::new();
        for (address, identity) in &self.active {
            if !unique.contains(address) {
                if outside_dependents.contains(identity) {
                    return Err(KeyedReconcileError::DanglingExternalDependent {
                        address: address.display_path(),
                        identity: *identity,
                    });
                }
                staged.active.remove(address);
                staged.tombstones.insert(address.clone(), *identity);
                staged.overrides.remove(identity);
                removed.push(ReconciledMember {
                    address: address.clone(),
                    identity: *identity,
                });
            }
        }

        for address in &requested {
            if let Some(identity) = staged.active.get(address).copied() {
                retained.push(ReconciledMember {
                    address: address.clone(),
                    identity,
                });
                continue;
            }
            staged.high_water = staged
                .high_water
                .checked_add(1)
                .ok_or(KeyedReconcileError::HighWaterExhausted)?;
            let generation = match staged.generation_high_water.get(address) {
                Some(generation) => generation.checked_add(1).ok_or_else(|| {
                    KeyedReconcileError::GenerationExhausted(address.display_path())
                })?,
                None => 0,
            };
            let identity = GeneratedMemberIdentity {
                allocation: staged.high_water,
                generation,
            };
            staged
                .generation_high_water
                .insert(address.clone(), generation);
            staged.active.insert(address.clone(), identity);
            created.push(ReconciledMember {
                address: address.clone(),
                identity,
            });
        }
        let reordered = self.order != requested;
        staged.order = requested;
        let changed = !created.is_empty() || !removed.is_empty() || reordered;
        if changed {
            staged.revision = staged
                .revision
                .checked_add(1)
                .ok_or(KeyedReconcileError::RevisionExhausted)?;
        }
        staged.validate()?;
        Ok(KeyedReconcilePlan {
            expected: self.identity(),
            staged,
            created,
            retained,
            removed,
            reordered,
        })
    }

    /// Publishes an exact prepared reconciliation.
    ///
    /// # Errors
    ///
    /// Returns a stale-plan or invalid-persistence error without mutation.
    pub fn commit(&mut self, plan: KeyedReconcilePlan) -> Result<(), KeyedReconcileError> {
        if self.identity() != plan.expected {
            return Err(KeyedReconcileError::StalePlan);
        }
        plan.staged.validate()?;
        *self = plan.staged;
        Ok(())
    }

    /// Adds or replaces one explicit generated placement override.
    ///
    /// # Errors
    ///
    /// Returns an unknown-address, invalid-value or revision-exhaustion error.
    pub fn set_override(
        &mut self,
        address: &GeneratedMemberAddress,
        value: ManagedValue,
    ) -> Result<GeneratedOverride, KeyedReconcileError> {
        let Some(identity) = self.active.get(address).copied() else {
            return Err(KeyedReconcileError::UnknownActiveAddress(
                address.display_path(),
            ));
        };
        validate_override_value(&value)?;
        let value = GeneratedOverride {
            address: address.clone(),
            identity,
            value,
        };
        self.overrides.insert(identity, value.clone());
        self.revision = self
            .revision
            .checked_add(1)
            .ok_or(KeyedReconcileError::RevisionExhausted)?;
        Ok(value)
    }

    /// Removes an explicit placement so the generated value is owned by code
    /// again. Returns false when no override existed.
    ///
    /// # Errors
    ///
    /// Returns an unknown-address or revision-exhaustion error.
    pub fn reset_to_code(
        &mut self,
        address: &GeneratedMemberAddress,
    ) -> Result<bool, KeyedReconcileError> {
        let Some(identity) = self.active.get(address).copied() else {
            return Err(KeyedReconcileError::UnknownActiveAddress(
                address.display_path(),
            ));
        };
        let removed = self.overrides.remove(&identity).is_some();
        if removed {
            self.revision = self
                .revision
                .checked_add(1)
                .ok_or(KeyedReconcileError::RevisionExhausted)?;
        }
        Ok(removed)
    }

    #[must_use]
    pub fn override_for(&self, address: &GeneratedMemberAddress) -> Option<&GeneratedOverride> {
        self.active
            .get(address)
            .and_then(|identity| self.overrides.get(identity))
    }

    /// Validates all resource, allocation, generation, order and override
    /// invariants of a persisted state.
    ///
    /// # Errors
    ///
    /// Returns the first typed invalid-persistence or resource error.
    pub fn validate(&self) -> Result<(), KeyedReconcileError> {
        if self.active.len() > MAX_GENERATED_MEMBERS
            || self.order.len() > MAX_GENERATED_MEMBERS
            || self.tombstones.len() > MAX_GENERATED_MEMBERS
            || self.generation_high_water.len() > MAX_GENERATED_MEMBERS
        {
            return Err(KeyedReconcileError::ResourceLimit {
                actual: self
                    .active
                    .len()
                    .max(self.order.len())
                    .max(self.tombstones.len())
                    .max(self.generation_high_water.len()),
                limit: MAX_GENERATED_MEMBERS,
            });
        }
        for address in self.generation_high_water.keys() {
            validate_address(address)?;
        }
        let mut allocations = BTreeSet::new();
        for (address, identity) in self.active.iter().chain(&self.tombstones) {
            validate_address(address)?;
            if identity.allocation == 0 || identity.allocation > self.high_water {
                return Err(KeyedReconcileError::InvalidPersistence(
                    "member allocation lies outside high-water".into(),
                ));
            }
            if self.generation_high_water.get(address).copied() < Some(identity.generation) {
                return Err(KeyedReconcileError::InvalidPersistence(
                    "member generation lies outside generation high-water".into(),
                ));
            }
            if self.active.contains_key(address) && self.tombstones.contains_key(address) {
                let active = self.active[address];
                let retired = self.tombstones[address];
                if active.generation <= retired.generation
                    || active.allocation == retired.allocation
                {
                    return Err(KeyedReconcileError::InvalidPersistence(
                        "reused semantic key does not have a fresh generation".into(),
                    ));
                }
            }
            if !allocations.insert(identity.allocation) {
                return Err(KeyedReconcileError::InvalidPersistence(
                    "generated allocation is reused by multiple identities".into(),
                ));
            }
        }
        if self.order.len() != self.active.len()
            || self.order.iter().collect::<BTreeSet<_>>().len() != self.order.len()
            || self
                .order
                .iter()
                .any(|address| !self.active.contains_key(address))
        {
            return Err(KeyedReconcileError::InvalidPersistence(
                "presentation order is not an exact permutation of active members".into(),
            ));
        }
        for (identity, value) in &self.overrides {
            if value.identity != *identity || self.active.get(&value.address) != Some(identity) {
                return Err(KeyedReconcileError::InvalidPersistence(
                    "override does not name its exact active generation".into(),
                ));
            }
            validate_override_value(&value.value)?;
        }
        Ok(())
    }

    /// Restores an Undo/Redo checkpoint while preserving every allocation
    /// already observed after that checkpoint as a retired identity. This
    /// returns the checkpoint's exact active members and overrides, but a
    /// later fresh authoring action can never reuse a post-checkpoint
    /// allocation or generation.
    ///
    /// # Errors
    ///
    /// Returns a typed persistence error when either checkpoint is invalid or
    /// their never-reuse ledgers cannot be merged.
    pub fn restored_checkpoint(&self, checkpoint: &Self) -> Result<Self, KeyedReconcileError> {
        self.validate()?;
        checkpoint.validate()?;
        let mut restored = checkpoint.clone();
        restored.high_water = restored.high_water.max(self.high_water);
        for (address, generation) in &self.generation_high_water {
            restored
                .generation_high_water
                .entry(address.clone())
                .and_modify(|known| *known = (*known).max(*generation))
                .or_insert(*generation);
        }
        for (address, identity) in self.active.iter().chain(&self.tombstones) {
            if restored.active.contains_key(address) {
                continue;
            }
            restored
                .tombstones
                .entry(address.clone())
                .and_modify(|known| {
                    if identity.generation > known.generation {
                        *known = *identity;
                    }
                })
                .or_insert(*identity);
        }
        restored.validate()?;
        Ok(restored)
    }

    /// Whether this live ledger remembers every allocation and per-address
    /// generation which another checkpoint has ever observed. Active and
    /// tombstoned membership may legitimately differ.
    pub(crate) fn retains_allocator_authority_from(&self, observed: &Self) -> bool {
        self.high_water >= observed.high_water
            && observed
                .generation_high_water
                .iter()
                .all(|(address, generation)| {
                    self.generation_high_water.get(address).copied() >= Some(*generation)
                })
    }
}

/// Exact prepared keyed expansion.
#[derive(Clone, Debug, PartialEq)]
pub struct KeyedReconcilePlan {
    expected: String,
    staged: KeyedReconcileState,
    pub created: Vec<ReconciledMember>,
    pub retained: Vec<ReconciledMember>,
    pub removed: Vec<ReconciledMember>,
    pub reordered: bool,
}

impl KeyedReconcilePlan {
    /// Exact identity of the keyed state against which this plan was staged.
    #[must_use]
    pub fn expected_identity(&self) -> &str {
        &self.expected
    }

    #[must_use]
    pub const fn staged(&self) -> &KeyedReconcileState {
        &self.staged
    }

    #[must_use]
    pub fn into_staged(self) -> KeyedReconcileState {
        self.staged
    }
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[non_exhaustive]
pub enum KeyedReconcileError {
    #[error("generated expansion contains {actual} members; the limit is {limit}")]
    ResourceLimit { actual: usize, limit: usize },
    #[error("invalid generated member address `{0}`")]
    InvalidAddress(String),
    #[error("duplicate generated member address `{0}`")]
    DuplicateAddress(String),
    #[error(
        "cannot retire `{address}` ({identity:?}) because an ordinary declaration depends on it"
    )]
    DanglingExternalDependent {
        address: String,
        identity: GeneratedMemberIdentity,
    },
    #[error("generated identity high-water is exhausted")]
    HighWaterExhausted,
    #[error("generated identity generation is exhausted for `{0}`")]
    GenerationExhausted(String),
    #[error("generated reconciliation revision is exhausted")]
    RevisionExhausted,
    #[error("prepared generated reconciliation is stale")]
    StalePlan,
    #[error("generated address `{0}` is not active")]
    UnknownActiveAddress(String),
    #[error("generated override contains an invalid value")]
    InvalidOverride,
    #[error("invalid persisted generated state: {0}")]
    InvalidPersistence(String),
}

fn validate_address(address: &GeneratedMemberAddress) -> Result<(), KeyedReconcileError> {
    let total = 1_usize
        .saturating_add(address.template.len())
        .saturating_add(address.member_key.len())
        .saturating_add(address.output.len());
    if address.invocation.is_empty()
        || address.template.is_empty()
        || address.member_key.is_empty()
        || address.output.is_empty()
        || total > MAX_ADDRESS_SEGMENTS
    {
        return Err(KeyedReconcileError::InvalidAddress(address.display_path()));
    }
    for key in std::iter::once(&address.invocation)
        .chain(&address.template)
        .chain(&address.member_key)
        .chain(&address.output)
    {
        if key.is_empty() || key.len() > MAX_ADDRESS_KEY_BYTES || key.chars().any(char::is_control)
        {
            return Err(KeyedReconcileError::InvalidAddress(address.display_path()));
        }
    }
    Ok(())
}

fn validate_override_value(value: &ManagedValue) -> Result<(), KeyedReconcileError> {
    match value {
        ManagedValue::Number(value) if !value.is_finite() => {
            Err(KeyedReconcileError::InvalidOverride)
        }
        ManagedValue::Unit(value) if !value.value.is_finite() => {
            Err(KeyedReconcileError::InvalidOverride)
        }
        ManagedValue::Array(values) => {
            for value in values {
                validate_override_value(value)?;
            }
            Ok(())
        }
        ManagedValue::Object(values) => {
            for value in values.values() {
                validate_override_value(value)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn member(key: &str) -> GeneratedMemberAddress {
        GeneratedMemberAddress::new("rounded", ["fillet"], [key], ["arc"])
    }

    #[test]
    fn insertion_and_reorder_preserve_matching_semantic_identity() {
        let mut state = KeyedReconcileState::empty();
        state
            .commit(
                state
                    .plan(
                        vec![member("a"), member("b"), member("c")],
                        &BTreeSet::new(),
                    )
                    .unwrap(),
            )
            .unwrap();
        let before = state.active.clone();
        let plan = state
            .plan(
                vec![member("c"), member("crest"), member("a"), member("b")],
                &BTreeSet::new(),
            )
            .unwrap();
        assert!(plan.reordered);
        assert_eq!(plan.created.len(), 1);
        assert_eq!(plan.retained.len(), 3);
        state.commit(plan).unwrap();
        for key in ["a", "b", "c"] {
            assert_eq!(state.active[&member(key)], before[&member(key)]);
        }
    }

    #[test]
    fn remove_tombstones_and_reuse_allocates_new_generation() {
        let mut state = KeyedReconcileState::empty();
        state
            .commit(state.plan(vec![member("a")], &BTreeSet::new()).unwrap())
            .unwrap();
        let first = state.active[&member("a")];
        state
            .commit(state.plan(Vec::new(), &BTreeSet::new()).unwrap())
            .unwrap();
        assert_eq!(state.tombstones[&member("a")], first);
        state
            .commit(state.plan(vec![member("a")], &BTreeSet::new()).unwrap())
            .unwrap();
        let second = state.active[&member("a")];
        assert!(second.allocation > first.allocation);
        assert_eq!(second.generation, first.generation + 1);
    }

    #[test]
    fn restored_older_active_member_never_reuses_an_abandoned_generation() {
        let empty = KeyedReconcileState::empty();
        let after_first = empty
            .plan(vec![member("a")], &BTreeSet::new())
            .unwrap()
            .into_staged();
        let first = after_first.active[&member("a")];
        assert_eq!(first.generation, 0);

        let after_remove = after_first
            .plan(Vec::new(), &BTreeSet::new())
            .unwrap()
            .into_staged();
        let after_second = after_remove
            .plan(vec![member("a")], &BTreeSet::new())
            .unwrap()
            .into_staged();
        let second = after_second.active[&member("a")];
        assert_eq!(second.generation, 1);

        // Undo the second add and then the removal. The visible member is the
        // original generation, but generation 1 remains durably observed.
        let undone_add = after_second.restored_checkpoint(&after_remove).unwrap();
        let undone_remove = undone_add.restored_checkpoint(&after_first).unwrap();
        assert_eq!(undone_remove.active[&member("a")], first);
        assert_eq!(undone_remove.high_water(), second.allocation);
        assert_eq!(undone_remove.generation_high_water(&member("a")), Some(1));

        let divergent_remove = undone_remove
            .plan(Vec::new(), &BTreeSet::new())
            .unwrap()
            .into_staged();
        let divergent_add = divergent_remove
            .plan(vec![member("a")], &BTreeSet::new())
            .unwrap()
            .into_staged();
        let third = divergent_add.active[&member("a")];
        assert!(third.allocation > second.allocation);
        assert_eq!(third.generation, 2);

        let encoded = serde_json::to_string(&divergent_add).unwrap();
        let decoded: KeyedReconcileState = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded, divergent_add);
        assert_eq!(decoded.generation_high_water(&member("a")), Some(2));
    }

    #[test]
    fn rename_is_delete_create_and_external_dependents_fail_closed() {
        let mut state = KeyedReconcileState::empty();
        state
            .commit(state.plan(vec![member("old")], &BTreeSet::new()).unwrap())
            .unwrap();
        let old = state.active[&member("old")];
        let blocked = BTreeSet::from([old]);
        assert!(matches!(
            state.plan(vec![member("new")], &blocked),
            Err(KeyedReconcileError::DanglingExternalDependent { .. })
        ));
        let plan = state.plan(vec![member("new")], &BTreeSet::new()).unwrap();
        assert_eq!(plan.created.len(), 1);
        assert_eq!(plan.removed.len(), 1);
    }

    #[test]
    fn generated_override_is_generation_bound_and_resettable() {
        let mut state = KeyedReconcileState::empty();
        state
            .commit(state.plan(vec![member("a")], &BTreeSet::new()).unwrap())
            .unwrap();
        state
            .set_override(
                &member("a"),
                ManagedValue::Array(vec![ManagedValue::Number(4.0), ManagedValue::Number(5.0)]),
            )
            .unwrap();
        assert!(state.override_for(&member("a")).is_some());
        assert!(state.reset_to_code(&member("a")).unwrap());
        assert!(state.override_for(&member("a")).is_none());
        assert!(!state.reset_to_code(&member("a")).unwrap());
    }

    #[test]
    fn stale_plan_and_corrupt_persistence_are_rejected() {
        let mut state = KeyedReconcileState::empty();
        let old_plan = state.plan(vec![member("a")], &BTreeSet::new()).unwrap();
        state
            .commit(state.plan(vec![member("b")], &BTreeSet::new()).unwrap())
            .unwrap();
        assert_eq!(
            state.commit(old_plan).unwrap_err(),
            KeyedReconcileError::StalePlan
        );

        let mut json = serde_json::to_value(&state).unwrap();
        json["high_water"] = serde_json::json!(0);
        assert!(serde_json::from_value::<KeyedReconcileState>(json).is_err());
    }
}
