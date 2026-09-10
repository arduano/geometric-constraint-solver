// SPDX-License-Identifier: GPL-3.0-or-later
//! Server-owned object lifetimes and exact dependent deletion closures.
//!
//! Object names come from the compiler host's stable file/declaration identity,
//! not presentation selection or coordinates. The host records changes in the
//! same durable transaction as the independently validated model publication.

use crate::protocol::MAX_REVISION;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SemanticTarget {
    pub object: String,
    pub generation: u64,
}

/// Serializable client intent, reauthenticated by comparing the entire closure.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DeletionPlan {
    pub roots: Vec<SemanticTarget>,
    pub closure: Vec<SemanticTarget>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TargetLimits {
    pub max_objects_including_tombstones: usize,
    pub max_dependencies_per_object: usize,
    pub max_total_dependencies: usize,
}
impl Default for TargetLimits {
    fn default() -> Self {
        Self {
            max_objects_including_tombstones: 100_000,
            max_dependencies_per_object: 1024,
            max_total_dependencies: 1_000_000,
        }
    }
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum TargetError {
    #[error("invalid stable semantic identity")]
    Invalid,
    #[error("semantic target is deleted, absent or belongs to a different lifetime")]
    Stale,
    #[error("semantic object already exists")]
    Exists,
    #[error("semantic identity/dependency resource limit")]
    Limit,
    #[error("deletion dependencies changed; review the current closure")]
    DeletionChanged,
}

#[derive(Clone, Debug)]
struct Object {
    generation: u64,
    alive: bool,
    dependencies: BTreeSet<String>,
    dependents: BTreeSet<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TargetSnapshot {
    format: String,
    high_water: u64,
    objects: Vec<ObjectSnapshot>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ObjectSnapshot {
    target: SemanticTarget,
    alive: bool,
    dependencies: Vec<SemanticTarget>,
}

/// This ledger is authored server state, never reconstructed from a client scene.
#[derive(Clone, Debug)]
pub struct TargetLedger {
    limits: TargetLimits,
    high_water: u64,
    objects: BTreeMap<String, Object>,
    dependency_count: usize,
}

impl TargetLedger {
    /// # Errors
    /// Rejects unusable resource limits.
    pub fn new(limits: TargetLimits) -> Result<Self, TargetError> {
        if limits.max_objects_including_tombstones == 0
            || limits.max_dependencies_per_object == 0
            || limits.max_total_dependencies == 0
        {
            return Err(TargetError::Limit);
        }
        Ok(Self {
            limits,
            high_water: 0,
            objects: BTreeMap::new(),
            dependency_count: 0,
        })
    }
    pub fn high_water(&self) -> u64 {
        self.high_water
    }
    pub fn dependency_count(&self) -> usize {
        self.dependency_count
    }

    /// Serializes authored lifetime/dependency state for the host's durable journal.
    /// No solved geometry or client presentation is stored here.
    ///
    /// # Errors
    /// Rejects snapshots exceeding the bounded 64 MiB transport.
    pub fn to_json(&self) -> Result<String, TargetError> {
        let objects = self
            .objects
            .iter()
            .map(|(name, object)| ObjectSnapshot {
                target: SemanticTarget {
                    object: name.clone(),
                    generation: object.generation,
                },
                alive: object.alive,
                dependencies: object
                    .dependencies
                    .iter()
                    .filter_map(|name| self.current(name))
                    .collect(),
            })
            .collect();
        let json = serde_json::to_string(&TargetSnapshot {
            format: "geosolve-target-ledger-v1".into(),
            high_water: self.high_water,
            objects,
        })
        .map_err(|_| TargetError::Invalid)?;
        if json.len() > 64 * 1024 * 1024 {
            return Err(TargetError::Limit);
        }
        Ok(json)
    }

    /// Restores a structurally validated ledger from trusted durable storage.
    /// Journal authentication and accepted-model reconstruction remain host-owned.
    ///
    /// # Errors
    /// Rejects oversized/corrupt lifetimes, duplicate objects/generations, stale
    /// dependencies, tombstone references or configured resource exhaustion.
    pub fn from_json(json: &str, limits: TargetLimits) -> Result<Self, TargetError> {
        if json.len() > 64 * 1024 * 1024 {
            return Err(TargetError::Limit);
        }
        let snapshot: TargetSnapshot =
            serde_json::from_str(json).map_err(|_| TargetError::Invalid)?;
        if snapshot.format != "geosolve-target-ledger-v1" || snapshot.high_water > MAX_REVISION {
            return Err(TargetError::Invalid);
        }
        let mut state = Self::new(limits)?;
        if snapshot.objects.len() > state.limits.max_objects_including_tombstones {
            return Err(TargetError::Limit);
        }
        let mut generations = BTreeSet::new();
        for object in &snapshot.objects {
            validate_name(&object.target.object)?;
            if object.target.generation == 0
                || object.target.generation > snapshot.high_water
                || !generations.insert(object.target.generation)
                || state.objects.contains_key(&object.target.object)
                || (!object.alive && !object.dependencies.is_empty())
            {
                return Err(TargetError::Invalid);
            }
            state.objects.insert(
                object.target.object.clone(),
                Object {
                    generation: object.target.generation,
                    alive: object.alive,
                    dependencies: BTreeSet::new(),
                    dependents: BTreeSet::new(),
                },
            );
        }
        for object in &snapshot.objects {
            if object.alive {
                state.set_dependencies(&object.target, &object.dependencies)?;
            }
        }
        state.high_water = snapshot.high_water;
        Ok(state)
    }

    /// Allocates a fresh lifetime centrally, even when reusing a tombstoned name.
    /// Rejection consumes no generation. Provisional client names are mapped by
    /// the host to these returned persistent targets in the commit receipt.
    ///
    /// # Errors
    /// Rejects duplicate/invalid objects and exhausted lifetime/storage bounds.
    pub fn create(&mut self, object: &str) -> Result<SemanticTarget, TargetError> {
        validate_name(object)?;
        if let Some(existing) = self.objects.get(object) {
            if existing.alive {
                return Err(TargetError::Exists);
            }
        } else if self.objects.len() >= self.limits.max_objects_including_tombstones {
            return Err(TargetError::Limit);
        }
        let generation = self
            .high_water
            .checked_add(1)
            .filter(|value| *value <= MAX_REVISION)
            .ok_or(TargetError::Limit)?;
        self.objects.insert(
            object.into(),
            Object {
                generation,
                alive: true,
                dependencies: BTreeSet::new(),
                dependents: BTreeSet::new(),
            },
        );
        self.high_water = generation;
        Ok(SemanticTarget {
            object: object.into(),
            generation,
        })
    }

    /// # Errors
    /// Rejects deleted, missing, forged and changed-generation targets.
    pub fn authenticate(&self, target: &SemanticTarget) -> Result<(), TargetError> {
        validate_name(&target.object)?;
        let object = self.objects.get(&target.object).ok_or(TargetError::Stale)?;
        if !object.alive || object.generation != target.generation {
            return Err(TargetError::Stale);
        }
        Ok(())
    }

    pub fn current(&self, object: &str) -> Option<SemanticTarget> {
        self.objects
            .get(object)
            .filter(|entry| entry.alive)
            .map(|entry| SemanticTarget {
                object: object.into(),
                generation: entry.generation,
            })
    }

    /// Authenticates the exact most recent dead lifetime. A later recreation,
    /// even one deleted again, cannot authorize restoration of an older object.
    ///
    /// # Errors
    /// Rejects live, missing, forged or superseded tombstones.
    pub fn authenticate_tombstone(&self, target: &SemanticTarget) -> Result<(), TargetError> {
        validate_name(&target.object)?;
        match self.objects.get(&target.object) {
            Some(object) if !object.alive && object.generation == target.generation => Ok(()),
            _ => Err(TargetError::Stale),
        }
    }

    /// Exact current dependency generations for trusted contribution recording.
    ///
    /// # Errors
    /// Rejects stale target lifetimes.
    pub fn dependencies(
        &self,
        target: &SemanticTarget,
    ) -> Result<Vec<SemanticTarget>, TargetError> {
        self.authenticate(target)?;
        Ok(self.objects[&target.object]
            .dependencies
            .iter()
            .filter_map(|name| self.current(name))
            .collect())
    }

    /// Deterministic current inventory, excluding retained tombstones.
    pub fn live_targets(&self) -> Vec<SemanticTarget> {
        self.objects
            .keys()
            .filter_map(|name| self.current(name))
            .collect()
    }

    /// Replaces compiler-derived dependencies after model validation. The entire
    /// proposed relation set is checked before any adjacency is changed.
    ///
    /// # Errors
    /// Rejects stale/duplicate/self references and resource exhaustion atomically.
    pub fn set_dependencies(
        &mut self,
        target: &SemanticTarget,
        dependencies: &[SemanticTarget],
    ) -> Result<(), TargetError> {
        self.authenticate(target)?;
        if dependencies.len() > self.limits.max_dependencies_per_object {
            return Err(TargetError::Limit);
        }
        let mut names = BTreeSet::new();
        for dependency in dependencies {
            self.authenticate(dependency)?;
            if dependency.object == target.object || !names.insert(dependency.object.clone()) {
                return Err(TargetError::Invalid);
            }
        }
        let previous = &self.objects[&target.object].dependencies;
        let total = self
            .dependency_count
            .saturating_sub(previous.len())
            .saturating_add(names.len());
        if total > self.limits.max_total_dependencies {
            return Err(TargetError::Limit);
        }
        let previous = previous.clone();
        for name in previous.difference(&names) {
            if let Some(parent) = self.objects.get_mut(name) {
                parent.dependents.remove(&target.object);
            }
        }
        for name in names.difference(&previous) {
            if let Some(parent) = self.objects.get_mut(name) {
                parent.dependents.insert(target.object.clone());
            }
        }
        if let Some(object) = self.objects.get_mut(&target.object) {
            object.dependencies = names;
        }
        self.dependency_count = total;
        Ok(())
    }

    /// Captures the exact recursive dependent closure visible to the requester.
    ///
    /// # Errors
    /// Rejects empty/duplicate/stale roots and oversized requests.
    pub fn plan_delete(&self, roots: &[SemanticTarget]) -> Result<DeletionPlan, TargetError> {
        if roots.is_empty() || roots.len() > self.limits.max_objects_including_tombstones {
            return Err(TargetError::Invalid);
        }
        let mut sorted_roots = BTreeSet::new();
        for root in roots {
            self.authenticate(root)?;
            if !sorted_roots.insert(root.clone()) {
                return Err(TargetError::Invalid);
            }
        }
        let mut visited = BTreeSet::new();
        let mut pending = sorted_roots
            .iter()
            .map(|root| root.object.clone())
            .collect::<Vec<_>>();
        while let Some(name) = pending.pop() {
            if !visited.insert(name.clone()) {
                continue;
            }
            pending.extend(self.objects[&name].dependents.iter().cloned());
        }
        let closure = visited
            .into_iter()
            .filter_map(|object| self.current(&object))
            .collect();
        Ok(DeletionPlan {
            roots: sorted_roots.into_iter().collect(),
            closure,
        })
    }

    /// No new dependency may be silently added to an old Delete command. Even an
    /// old descendant deleted/recreated with identical source invalidates the plan.
    ///
    /// # Errors
    /// Rejects changed closure, lifetime, duplicates or ordering without deleting anything.
    pub fn authenticate_delete(&self, plan: &DeletionPlan) -> Result<(), TargetError> {
        if plan.closure.len() > self.limits.max_objects_including_tombstones {
            return Err(TargetError::Limit);
        }
        let current = self.plan_delete(&plan.roots)?;
        if &current != plan {
            return Err(TargetError::DeletionChanged);
        }
        Ok(())
    }

    /// Applies only the reauthenticated exact closure, retaining lifetime tombstones.
    /// Caller must publish this staged ledger with its accepted domain transaction.
    ///
    /// # Errors
    /// Rejects stale/expanded/forged plans atomically.
    pub fn delete(&mut self, plan: &DeletionPlan) -> Result<(), TargetError> {
        self.authenticate_delete(plan)?;
        for target in &plan.closure {
            let dependencies = self.objects[&target.object].dependencies.clone();
            for name in &dependencies {
                if let Some(parent) = self.objects.get_mut(name) {
                    parent.dependents.remove(&target.object);
                }
            }
            self.dependency_count -= dependencies.len();
            if let Some(object) = self.objects.get_mut(&target.object) {
                object.alive = false;
                object.dependencies.clear();
                object.dependents.clear();
            }
        }
        Ok(())
    }
}

fn validate_name(name: &str) -> Result<(), TargetError> {
    if name.is_empty() || name.len() > 512 || name.chars().any(char::is_control) {
        return Err(TargetError::Invalid);
    }
    Ok(())
}
