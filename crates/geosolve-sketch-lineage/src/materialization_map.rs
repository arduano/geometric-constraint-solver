// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    LineageDocument, LineageDocumentError, LineageDocumentIdentity, LineageEvaluationPolicy,
    LineageOpaqueId, LineageOutputIdentityFlow, LineageOutputKind, LineageOutputRef,
    LineageReservationKind, LineageSemanticKey, LineageStepEvaluationState, LineageStepId,
};

/// Current revision-stamped materialization-map wire version.
pub const LINEAGE_MATERIALIZATION_MAP_VERSION: u32 = 2;
/// Defensive byte limit for one cached materialization/ownership map.
pub const MAX_LINEAGE_MATERIALIZATION_MAP_JSON_BYTES: usize = 64 * 1024 * 1024;

/// One persistent identity in an owning materialization domain.
#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LineageMaterializedIdentity {
    pub kind: LineageReservationKind,
    pub persistent_id: LineageOpaqueId,
}

/// One exact writable leaf within a persistent materialized identity.
///
/// The leaf key is stable semantic state, not a JSON array index or a display
/// label. Two fields of one point/curve/source therefore remain distinct even
/// though they share the same persistent native object identity.
#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LineageMaterializedLeaf {
    pub materialized: LineageMaterializedIdentity,
    pub key: LineageSemanticKey,
}

/// One globally semantic writable leaf owned by a logical output that has no
/// native reservation.
///
/// This is intentionally limited to unique logical state such as host
/// activation. Repeated native state (points, curves, roles, features, and so
/// on) is addressed through [`LineageMaterializedLeaf`] instead.
#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LineageLogicalLeaf {
    pub kind: LineageOutputKind,
    pub key: LineageSemanticKey,
}

/// Exact lineage field allowed to rewrite one materialized value.
#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LineageMaterializationOwner {
    pub step: LineageStepId,
    pub output: LineageOutputRef,
    pub field: LineageSemanticKey,
}

/// One logical output's current materialization and write-back owner.
#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LineageMaterializationBinding {
    pub logical: LineageOutputRef,
    pub leaf: Option<LineageMaterializedLeaf>,
    pub owner: Option<LineageMaterializationOwner>,
}

/// Reverse lookup entry for one currently materialized writable leaf.
#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LineageMaterializationReverseBinding {
    pub leaf: LineageMaterializedLeaf,
    pub logical: LineageOutputRef,
    pub owner: LineageMaterializationOwner,
}

/// Reverse lookup entry for one currently materialized logical-only leaf.
#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LineageLogicalReverseBinding {
    pub leaf: LineageLogicalLeaf,
    pub logical: LineageOutputRef,
    pub owner: LineageMaterializationOwner,
}

/// Revision-stamped bidirectional logical/materialized identity evidence.
///
/// The map is derived only from a validated lineage document's typed identity
/// flow. It never infers identity from geometry, insertion order, proximity, or
/// a digest. Only steps that are `Ready` in the strict chronological dependency
/// plan publish live reverse bindings; suppressed, tombstoned, blocked, and
/// unevaluated steps retain their declarations and reservations.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LineageMaterializationMap {
    version: u32,
    lineage: LineageDocumentIdentity,
    bindings: Vec<LineageMaterializationBinding>,
    declared_reverse: Vec<LineageMaterializationReverseBinding>,
    reverse: Vec<LineageMaterializationReverseBinding>,
    declared_logical_reverse: Vec<LineageLogicalReverseBinding>,
    logical_reverse: Vec<LineageLogicalReverseBinding>,
    /// Complete declared ownership, including suppressed and tombstoned
    /// outputs whose reservations remain part of retained lineage authority.
    owned_by_step: BTreeMap<LineageStepId, Vec<LineageOutputRef>>,
    /// Currently materialized writable ownership after lifecycle and identity
    /// transfer have been resolved.
    live_owned_by_step: BTreeMap<LineageStepId, Vec<LineageOutputRef>>,
    declared_writable_by_step: BTreeMap<LineageStepId, Vec<LineageMaterializationOwner>>,
    live_writable_by_step: BTreeMap<LineageStepId, Vec<LineageMaterializationOwner>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LineageMaterializationMapWire {
    version: u32,
    lineage: LineageDocumentIdentity,
    bindings: Vec<LineageMaterializationBinding>,
    declared_reverse: Vec<LineageMaterializationReverseBinding>,
    reverse: Vec<LineageMaterializationReverseBinding>,
    declared_logical_reverse: Vec<LineageLogicalReverseBinding>,
    logical_reverse: Vec<LineageLogicalReverseBinding>,
    owned_by_step: BTreeMap<LineageStepId, Vec<LineageOutputRef>>,
    live_owned_by_step: BTreeMap<LineageStepId, Vec<LineageOutputRef>>,
    declared_writable_by_step: BTreeMap<LineageStepId, Vec<LineageMaterializationOwner>>,
    live_writable_by_step: BTreeMap<LineageStepId, Vec<LineageMaterializationOwner>>,
}

impl From<LineageMaterializationMapWire> for LineageMaterializationMap {
    fn from(value: LineageMaterializationMapWire) -> Self {
        Self {
            version: value.version,
            lineage: value.lineage,
            bindings: value.bindings,
            declared_reverse: value.declared_reverse,
            reverse: value.reverse,
            declared_logical_reverse: value.declared_logical_reverse,
            logical_reverse: value.logical_reverse,
            owned_by_step: value.owned_by_step,
            live_owned_by_step: value.live_owned_by_step,
            declared_writable_by_step: value.declared_writable_by_step,
            live_writable_by_step: value.live_writable_by_step,
        }
    }
}

#[derive(Clone, Debug)]
enum ResolvedIdentity {
    Logical(LineageOutputRef),
    Materialized(LineageMaterializedLeaf),
}

/// Failure to derive exact materialization ownership from validated lineage.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum LineageMaterializationMapError {
    #[error(transparent)]
    Document(#[from] LineageDocumentError),
    #[error("lineage output {output:?} has no reservation {reservation}")]
    MissingReservation {
        output: LineageOutputRef,
        reservation: crate::LineageReservationId,
    },
    #[error("lineage output {output:?} has no identity-flow declaration")]
    MissingIdentityFlow { output: LineageOutputRef },
    #[error("lineage output {output:?} has no resolved predecessor materialization")]
    MissingPredecessor { output: LineageOutputRef },
    #[error("materialized leaf {persistent_id}/{key} has more than one live owner")]
    DuplicateLiveOwner {
        persistent_id: LineageOpaqueId,
        key: LineageSemanticKey,
    },
    #[error("logical writable leaf {kind:?}/{key} has more than one live owner")]
    DuplicateLiveLogicalOwner {
        kind: LineageOutputKind,
        key: LineageSemanticKey,
    },
    #[error("materialization-map JSON exceeds {limit} bytes")]
    JsonResourceLimit { limit: usize },
    #[error("unsupported materialization-map version {actual}; expected {expected}")]
    UnsupportedVersion { actual: u32, expected: u32 },
    #[error("cached materialization map does not exactly match authoritative lineage")]
    CacheMismatch,
    #[error("invalid materialization-map JSON: {0}")]
    Json(#[from] serde_json::Error),
}

impl LineageMaterializationMap {
    /// Derives a deterministic ownership map for one validated lineage revision.
    ///
    /// # Errors
    ///
    /// Returns a typed error when an otherwise structurally valid identity flow
    /// cannot resolve its exact predecessor or publishes two live owners.
    #[allow(
        clippy::too_many_lines,
        reason = "the forward and reverse identity-flow audit is intentionally one ordered pass"
    )]
    pub fn derive(document: &LineageDocument) -> Result<Self, LineageMaterializationMapError> {
        document.validate()?;

        let materialization_eligibility = document
            .dependency_plan_for(LineageEvaluationPolicy::StrictChronological, [])?
            .into_iter()
            .map(|entry| {
                (
                    entry.step,
                    matches!(entry.state, LineageStepEvaluationState::Ready),
                )
            })
            .collect::<BTreeMap<_, _>>();

        let mut resolved = BTreeMap::<LineageOutputRef, Option<ResolvedIdentity>>::new();
        let mut active = BTreeMap::<
            LineageMaterializedLeaf,
            (LineageOutputRef, LineageMaterializationOwner),
        >::new();
        let mut active_logical =
            BTreeMap::<LineageOutputRef, (LineageOutputRef, LineageMaterializationOwner)>::new();
        let mut owned_by_step = BTreeMap::<LineageStepId, Vec<LineageOutputRef>>::new();

        for step in document.steps() {
            let materializable = materialization_eligibility[&step.id];
            let reservations = step
                .reservations
                .iter()
                .map(|reservation| (reservation.id, reservation))
                .collect::<BTreeMap<_, _>>();

            for output in &step.outputs {
                let logical = LineageOutputRef {
                    document: document.id(),
                    step: step.id,
                    output: output.id,
                    kind: output.kind,
                };
                let identity = step.output_identity(output.id).ok_or(
                    LineageMaterializationMapError::MissingIdentityFlow { output: logical },
                )?;
                if !matches!(identity.flow, LineageOutputIdentityFlow::Aliased { .. }) {
                    owned_by_step.entry(step.id).or_default().push(logical);
                }

                let field = output.key.clone();
                let own = || LineageMaterializationOwner {
                    step: step.id,
                    output: logical,
                    field: field.clone(),
                };

                let leaf = match identity.flow {
                    LineageOutputIdentityFlow::OwnedLogical => {
                        if materializable {
                            active_logical.insert(logical, (logical, own()));
                        }
                        Some(ResolvedIdentity::Logical(logical))
                    }
                    LineageOutputIdentityFlow::Created { reservation } => {
                        let reservation = reservations.get(&reservation).ok_or(
                            LineageMaterializationMapError::MissingReservation {
                                output: logical,
                                reservation,
                            },
                        )?;
                        let leaf = LineageMaterializedLeaf {
                            materialized: LineageMaterializedIdentity {
                                kind: reservation.kind,
                                persistent_id: reservation.persistent_id.clone(),
                            },
                            key: output.key.clone(),
                        };
                        if materializable {
                            let owner = own();
                            if active.insert(leaf.clone(), (logical, owner)).is_some() {
                                return Err(LineageMaterializationMapError::DuplicateLiveOwner {
                                    persistent_id: leaf.materialized.persistent_id,
                                    key: leaf.key,
                                });
                            }
                        }
                        Some(ResolvedIdentity::Materialized(leaf))
                    }
                    LineageOutputIdentityFlow::Aliased { source } => {
                        resolved.get(&source).cloned().ok_or(
                            LineageMaterializationMapError::MissingPredecessor { output: logical },
                        )?
                    }
                    LineageOutputIdentityFlow::Continued { source } => {
                        let predecessor = resolved.get(&source).cloned().ok_or(
                            LineageMaterializationMapError::MissingPredecessor { output: logical },
                        )?;
                        if materializable {
                            let predecessor = predecessor.as_ref().ok_or(
                                LineageMaterializationMapError::MissingPredecessor {
                                    output: logical,
                                },
                            )?;
                            match predecessor {
                                ResolvedIdentity::Logical(token) => {
                                    if active_logical.remove(token).is_some() {
                                        active_logical.insert(*token, (logical, own()));
                                    }
                                }
                                ResolvedIdentity::Materialized(leaf) => {
                                    if active.remove(leaf).is_some() {
                                        active.insert(leaf.clone(), (logical, own()));
                                    }
                                }
                            }
                        }
                        predecessor
                    }
                    LineageOutputIdentityFlow::Retired { source } => {
                        let predecessor = resolved.get(&source).ok_or(
                            LineageMaterializationMapError::MissingPredecessor { output: logical },
                        )?;
                        if materializable && let Some(predecessor) = predecessor {
                            match predecessor {
                                ResolvedIdentity::Logical(token) => {
                                    active_logical.remove(token);
                                }
                                ResolvedIdentity::Materialized(leaf) => {
                                    active.remove(leaf);
                                }
                            }
                        }
                        None
                    }
                };
                resolved.insert(logical, leaf);
            }
        }

        let mut declared_reverse = Vec::new();
        let mut reverse_by_leaf =
            BTreeMap::<LineageMaterializedLeaf, LineageMaterializationReverseBinding>::new();
        let mut declared_logical_reverse = Vec::new();
        let mut logical_reverse_by_leaf =
            BTreeMap::<LineageLogicalLeaf, LineageLogicalReverseBinding>::new();
        let mut declared_writable_by_step =
            BTreeMap::<LineageStepId, Vec<LineageMaterializationOwner>>::new();
        let mut live_writable_by_step =
            BTreeMap::<LineageStepId, Vec<LineageMaterializationOwner>>::new();

        for step in document.steps() {
            let step_is_materializable = materialization_eligibility[&step.id];
            for declaration in &step.writable_leaves {
                let logical = document.output_ref(step.id, declaration.output).ok_or(
                    LineageMaterializationMapError::MissingIdentityFlow {
                        output: LineageOutputRef {
                            document: document.id(),
                            step: step.id,
                            output: declaration.output,
                            kind: LineageOutputKind::Collection,
                        },
                    },
                )?;
                let owner = LineageMaterializationOwner {
                    step: step.id,
                    output: logical,
                    field: declaration.key.clone(),
                };
                declared_writable_by_step
                    .entry(step.id)
                    .or_default()
                    .push(owner.clone());
                let identity_flow = step
                    .output_identity(declaration.output)
                    .ok_or(LineageMaterializationMapError::MissingIdentityFlow { output: logical })?
                    .flow;
                match resolved.get(&logical).and_then(Option::as_ref) {
                    Some(ResolvedIdentity::Materialized(token)) => {
                        let leaf = LineageMaterializedLeaf {
                            materialized: token.materialized.clone(),
                            key: declaration.key.clone(),
                        };
                        let binding = LineageMaterializationReverseBinding {
                            leaf: leaf.clone(),
                            logical,
                            owner: owner.clone(),
                        };
                        declared_reverse.push(binding.clone());
                        // Continuation transfers only the exact writable leaves
                        // it declares. The identity-level forward binding moves
                        // to the continuation, but unrelated predecessor leaves
                        // remain writable through their original owners.
                        let publishes_reverse =
                            step_is_materializable && active.contains_key(token);
                        if publishes_reverse {
                            let previous = reverse_by_leaf.insert(leaf.clone(), binding);
                            if previous.is_some()
                                && !matches!(
                                    identity_flow,
                                    LineageOutputIdentityFlow::Continued { .. }
                                )
                            {
                                return Err(LineageMaterializationMapError::DuplicateLiveOwner {
                                    persistent_id: leaf.materialized.persistent_id,
                                    key: leaf.key,
                                });
                            }
                        }
                    }
                    Some(ResolvedIdentity::Logical(token)) => {
                        let leaf = LineageLogicalLeaf {
                            kind: logical.kind,
                            key: declaration.key.clone(),
                        };
                        let binding = LineageLogicalReverseBinding {
                            leaf: leaf.clone(),
                            logical,
                            owner: owner.clone(),
                        };
                        declared_logical_reverse.push(binding.clone());
                        let publishes_reverse =
                            step_is_materializable && active_logical.contains_key(token);
                        if publishes_reverse {
                            let previous = logical_reverse_by_leaf.insert(leaf.clone(), binding);
                            if previous.is_some()
                                && !matches!(
                                    identity_flow,
                                    LineageOutputIdentityFlow::Continued { .. }
                                )
                            {
                                return Err(
                                    LineageMaterializationMapError::DuplicateLiveLogicalOwner {
                                        kind: leaf.kind,
                                        key: leaf.key,
                                    },
                                );
                            }
                        }
                    }
                    None => {
                        return Err(LineageMaterializationMapError::MissingPredecessor {
                            output: logical,
                        });
                    }
                }
            }
        }
        declared_reverse.sort_unstable();
        declared_logical_reverse.sort_unstable();
        let reverse = reverse_by_leaf.into_values().collect::<Vec<_>>();
        let logical_reverse = logical_reverse_by_leaf.into_values().collect::<Vec<_>>();
        for owner in reverse
            .iter()
            .map(|binding| &binding.owner)
            .chain(logical_reverse.iter().map(|binding| &binding.owner))
        {
            live_writable_by_step
                .entry(owner.step)
                .or_default()
                .push(owner.clone());
        }
        for owners in declared_writable_by_step.values_mut() {
            owners.sort_unstable();
        }
        for owners in live_writable_by_step.values_mut() {
            owners.sort_unstable();
            owners.dedup();
        }

        let bindings = resolved
            .into_iter()
            .map(|(logical, resolved)| {
                let (leaf, owner) = match resolved {
                    Some(ResolvedIdentity::Logical(token)) => {
                        let mut owners = logical_reverse
                            .iter()
                            .filter(|binding| binding.logical == token)
                            .map(|binding| &binding.owner);
                        let owner = owners
                            .next()
                            .filter(|first| owners.all(|candidate| candidate.step == first.step))
                            .cloned();
                        (None, owner)
                    }
                    Some(ResolvedIdentity::Materialized(leaf)) => {
                        let mut owners = reverse
                            .iter()
                            .filter(|binding| binding.leaf.materialized == leaf.materialized)
                            .map(|binding| &binding.owner);
                        let owner = owners
                            .next()
                            .filter(|first| owners.all(|candidate| candidate.step == first.step))
                            .cloned();
                        (Some(leaf), owner)
                    }
                    None => (None, None),
                };
                LineageMaterializationBinding {
                    logical,
                    leaf,
                    owner,
                }
            })
            .collect();
        for outputs in owned_by_step.values_mut() {
            outputs.sort_unstable();
        }
        let mut live_owned_by_step = BTreeMap::<LineageStepId, Vec<LineageOutputRef>>::new();
        for (logical, owner) in reverse
            .iter()
            .map(|binding| (binding.logical, &binding.owner))
            .chain(
                logical_reverse
                    .iter()
                    .map(|binding| (binding.logical, &binding.owner)),
            )
        {
            live_owned_by_step
                .entry(owner.step)
                .or_default()
                .push(logical);
        }
        for outputs in live_owned_by_step.values_mut() {
            outputs.sort_unstable();
            outputs.dedup();
        }
        Ok(Self {
            version: LINEAGE_MATERIALIZATION_MAP_VERSION,
            lineage: document.identity(),
            bindings,
            declared_reverse,
            reverse,
            declared_logical_reverse,
            logical_reverse,
            owned_by_step,
            live_owned_by_step,
            declared_writable_by_step,
            live_writable_by_step,
        })
    }

    /// Serializes this derived map with deterministic struct/vector ordering.
    ///
    /// # Errors
    ///
    /// Returns a JSON error or a resource-limit error if the encoded cache is
    /// unexpectedly larger than the public decoder admits.
    pub fn to_canonical_json(&self) -> Result<String, LineageMaterializationMapError> {
        let json = serde_json::to_string(self)?;
        if json.len() > MAX_LINEAGE_MATERIALIZATION_MAP_JSON_BYTES {
            return Err(LineageMaterializationMapError::JsonResourceLimit {
                limit: MAX_LINEAGE_MATERIALIZATION_MAP_JSON_BYTES,
            });
        }
        Ok(json)
    }

    /// Decodes a disposable cache only when it is byte-semantically identical
    /// to a fresh derivation from the supplied authoritative lineage.
    ///
    /// This closes ordering, duplication, stale-revision, owner, and identity
    /// validation in one comparison. A cache never repairs lineage.
    ///
    /// # Errors
    ///
    /// Returns a bounded decoding error or [`Self::derive`] failure.
    pub fn from_json_for_document(
        json: &str,
        document: &LineageDocument,
    ) -> Result<Self, LineageMaterializationMapError> {
        if json.len() > MAX_LINEAGE_MATERIALIZATION_MAP_JSON_BYTES {
            return Err(LineageMaterializationMapError::JsonResourceLimit {
                limit: MAX_LINEAGE_MATERIALIZATION_MAP_JSON_BYTES,
            });
        }
        let wire = serde_json::from_str::<LineageMaterializationMapWire>(json)?;
        if wire.version != LINEAGE_MATERIALIZATION_MAP_VERSION {
            return Err(LineageMaterializationMapError::UnsupportedVersion {
                actual: wire.version,
                expected: LINEAGE_MATERIALIZATION_MAP_VERSION,
            });
        }
        let cached = Self::from(wire);
        let expected = Self::derive(document)?;
        if cached != expected || serde_json::to_string(&expected)? != json {
            return Err(LineageMaterializationMapError::CacheMismatch);
        }
        Ok(cached)
    }

    /// Returns the authoritative lineage identity stamped into this map.
    #[must_use]
    pub const fn lineage(&self) -> LineageDocumentIdentity {
        self.lineage
    }

    /// Returns the canonically ordered logical-to-materialized bindings.
    #[must_use]
    pub fn bindings(&self) -> &[LineageMaterializationBinding] {
        &self.bindings
    }

    /// Returns the canonically ordered exact-leaf reverse bindings.
    #[must_use]
    pub fn reverse_bindings(&self) -> &[LineageMaterializationReverseBinding] {
        &self.reverse
    }

    /// Returns every retained materialized writable declaration, including
    /// declarations whose step is suppressed, tombstoned, or superseded.
    #[must_use]
    pub fn declared_reverse_bindings(&self) -> &[LineageMaterializationReverseBinding] {
        &self.declared_reverse
    }

    /// Returns the canonically ordered live logical-only reverse bindings.
    #[must_use]
    pub fn logical_reverse_bindings(&self) -> &[LineageLogicalReverseBinding] {
        &self.logical_reverse
    }

    /// Returns all retained logical-only writable declarations.
    #[must_use]
    pub fn declared_logical_reverse_bindings(&self) -> &[LineageLogicalReverseBinding] {
        &self.declared_logical_reverse
    }

    /// Resolves one exact current materialized leaf to its writable owner.
    #[must_use]
    pub fn owner_for_leaf(
        &self,
        leaf: &LineageMaterializedLeaf,
    ) -> Option<&LineageMaterializationOwner> {
        self.reverse
            .binary_search_by(|entry| entry.leaf.cmp(leaf))
            .ok()
            .map(|index| &self.reverse[index].owner)
    }

    /// Resolves one unique current logical-only leaf to its writable owner.
    #[must_use]
    pub fn owner_for_logical_leaf(
        &self,
        leaf: &LineageLogicalLeaf,
    ) -> Option<&LineageMaterializationOwner> {
        self.logical_reverse
            .binary_search_by(|entry| entry.leaf.cmp(leaf))
            .ok()
            .map(|index| &self.logical_reverse[index].owner)
    }

    /// Returns the exact native identity resolved for a logical output.
    #[must_use]
    pub fn materialized_identity_for_output(
        &self,
        output: LineageOutputRef,
    ) -> Option<&LineageMaterializedIdentity> {
        self.bindings
            .binary_search_by_key(&output, |binding| binding.logical)
            .ok()
            .and_then(|index| self.bindings[index].leaf.as_ref())
            .map(|leaf| &leaf.materialized)
    }

    /// Resolves a materialized identity only when it has exactly one writable
    /// leaf. Multi-leaf identities deliberately require [`Self::owner_for_leaf`]
    /// so a caller cannot silently rewrite the wrong component.
    #[must_use]
    pub fn owner_for(
        &self,
        materialized: &LineageMaterializedIdentity,
    ) -> Option<&LineageMaterializationOwner> {
        let mut matches = self
            .reverse
            .iter()
            .filter(|entry| &entry.leaf.materialized == materialized);
        let owner = &matches.next()?.owner;
        matches.next().is_none().then_some(owner)
    }

    /// Returns the complete declared ownership set of one lineage step.
    ///
    /// Suppression and tombstoning remove live materialization authority but
    /// deliberately do not erase these declarations or their reservations.
    #[must_use]
    pub fn owned_outputs(&self, step: LineageStepId) -> &[LineageOutputRef] {
        self.owned_by_step.get(&step).map_or(&[], Vec::as_slice)
    }

    /// Returns the currently materialized writable ownership of one step.
    ///
    /// This set is lifecycle-sensitive and reflects continuation/retirement
    /// transfers. Use [`Self::owned_outputs`] when auditing retained intent.
    #[must_use]
    pub fn live_owned_outputs(&self, step: LineageStepId) -> &[LineageOutputRef] {
        self.live_owned_by_step
            .get(&step)
            .map_or(&[], Vec::as_slice)
    }

    /// Returns every retained writable declaration made by one step.
    #[must_use]
    pub fn declared_writable_owners(&self, step: LineageStepId) -> &[LineageMaterializationOwner] {
        self.declared_writable_by_step
            .get(&step)
            .map_or(&[], Vec::as_slice)
    }

    /// Returns only the step's currently live reverse-write authority.
    #[must_use]
    pub fn live_writable_owners(&self, step: LineageStepId) -> &[LineageMaterializationOwner] {
        self.live_writable_by_step
            .get(&step)
            .map_or(&[], Vec::as_slice)
    }
}

#[allow(
    dead_code,
    reason = "kept exhaustive so a new output/reservation family requires an explicit mapping review"
)]
const fn reservation_kind_for_output(kind: LineageOutputKind) -> Option<LineageReservationKind> {
    Some(match kind {
        LineageOutputKind::Point => LineageReservationKind::Point,
        LineageOutputKind::Scalar => LineageReservationKind::Scalar,
        LineageOutputKind::Curve => LineageReservationKind::Curve,
        LineageOutputKind::CurveSpan => LineageReservationKind::CurveSpan,
        LineageOutputKind::TrimView => LineageReservationKind::TrimView,
        LineageOutputKind::Contact => LineageReservationKind::Contact,
        LineageOutputKind::Constraint => LineageReservationKind::Constraint,
        LineageOutputKind::Dimension => LineageReservationKind::Dimension,
        LineageOutputKind::Source => LineageReservationKind::Source,
        LineageOutputKind::Parameter => LineageReservationKind::Parameter,
        LineageOutputKind::ParameterBinding => LineageReservationKind::ParameterBinding,
        LineageOutputKind::ParameterOutput => LineageReservationKind::ParameterOutput,
        LineageOutputKind::ExternalBinding => LineageReservationKind::ExternalBinding,
        LineageOutputKind::GeometryRole
        | LineageOutputKind::Activation
        | LineageOutputKind::Collection => return None,
        LineageOutputKind::Profile => LineageReservationKind::Profile,
        LineageOutputKind::Chain => LineageReservationKind::Chain,
        LineageOutputKind::Operation => LineageReservationKind::Operation,
        LineageOutputKind::Feature => LineageReservationKind::Feature,
        LineageOutputKind::FeatureCorner => LineageReservationKind::FeatureCorner,
        LineageOutputKind::Annotation => LineageReservationKind::Annotation,
    })
}
