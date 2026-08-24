// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::{
    CellId, IntentExternalInputs, IntentFieldKey, IntentKey, IntentLiteral, IntentNodeDraft,
    IntentPortRef, IntentPortSelector, IntentSessionIdentity, LeafRef, NodeId, PatchPortRef,
};

/// Whether a structurally valid but unsolved candidate may become retained
/// design intent. Direct manipulation always uses `RequireAccepted`.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum IntentPatchPolicy {
    RetainFailedIntent,
    RequireAccepted,
}

/// Exact deletion contract. A cascade is accepted only when its caller-stamped
/// set exactly equals Rust's current dependent closure.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "policy", rename_all = "snake_case", deny_unknown_fields)]
pub enum DeletePolicy {
    RejectDependents,
    Cascade {
        exact_nodes: BTreeSet<NodeId>,
    },
    /// Deletes several explicitly selected semantic roots plus their exact
    /// current dependent closure as one unordered transaction.
    ///
    /// This is used when one user-facing declaration owns typed helper
    /// declarations which necessarily precede it in the dependency DAG. The
    /// caller must stamp both the complete root set and Rust-computed closure;
    /// mutable names and patch-array order never participate.
    CascadeRoots {
        exact_roots: BTreeSet<NodeId>,
        exact_nodes: BTreeSet<NodeId>,
    },
}

/// Stable or transaction-local target cell.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(tag = "target", rename_all = "snake_case", deny_unknown_fields)]
pub enum CellTarget {
    Stable { cell: CellId },
    Alias { alias: IntentKey },
}

/// Closed unordered patch vocabulary. Array position carries no semantics.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum IntentPatchOperation {
    CreateNode {
        alias: IntentKey,
        draft: Box<IntentNodeDraft>,
        cell: Option<CellTarget>,
    },
    DeleteNode {
        node: NodeId,
        policy: DeletePolicy,
    },
    SetSuppressed {
        node: NodeId,
        suppressed: bool,
    },
    SetDefinitionField {
        node: NodeId,
        field: IntentFieldKey,
        value: IntentLiteral,
    },
    SetInstanceLeaf {
        leaf: LeafRef,
        value: IntentLiteral,
    },
    RebindInput {
        node: NodeId,
        slot: crate::InputSlot,
        source: PatchPortRef,
    },
    /// Explicitly promotes one exact historical native Point declaration to
    /// the supported typed Sketch Point recipe in place.
    EjectBootstrapPoint {
        node: NodeId,
    },
    RenameNode {
        node: NodeId,
        name: IntentKey,
    },
    MoveDeclaration {
        node: NodeId,
        cell: CellTarget,
        before: Option<NodeId>,
    },
    CreateCell {
        alias: IntentKey,
        name: IntentKey,
        before: Option<CellId>,
    },
    DeleteCell {
        cell: CellId,
    },
    ReorderCells {
        exact_order: Vec<CellId>,
    },
    ReplaceExternalInputs {
        inputs: IntentExternalInputs,
    },
}

/// Closed deterministic operation category used by transaction/history
/// projection. Payloads remain authenticated by the exact plan and session
/// digests; this compact category is intended for durable UI/audit copy.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum IntentPatchOperationKind {
    CreateNode,
    DeleteNode,
    SetSuppressed,
    SetDefinitionField,
    SetInstanceLeaf,
    RebindInput,
    EjectBootstrapPoint,
    RenameNode,
    MoveDeclaration,
    CreateCell,
    DeleteCell,
    ReorderCells,
    ReplaceExternalInputs,
}

impl IntentPatchOperation {
    #[must_use]
    pub const fn kind(&self) -> IntentPatchOperationKind {
        match self {
            Self::CreateNode { .. } => IntentPatchOperationKind::CreateNode,
            Self::DeleteNode { .. } => IntentPatchOperationKind::DeleteNode,
            Self::SetSuppressed { .. } => IntentPatchOperationKind::SetSuppressed,
            Self::SetDefinitionField { .. } => IntentPatchOperationKind::SetDefinitionField,
            Self::SetInstanceLeaf { .. } => IntentPatchOperationKind::SetInstanceLeaf,
            Self::RebindInput { .. } => IntentPatchOperationKind::RebindInput,
            Self::EjectBootstrapPoint { .. } => IntentPatchOperationKind::EjectBootstrapPoint,
            Self::RenameNode { .. } => IntentPatchOperationKind::RenameNode,
            Self::MoveDeclaration { .. } => IntentPatchOperationKind::MoveDeclaration,
            Self::CreateCell { .. } => IntentPatchOperationKind::CreateCell,
            Self::DeleteCell { .. } => IntentPatchOperationKind::DeleteCell,
            Self::ReorderCells { .. } => IntentPatchOperationKind::ReorderCells,
            Self::ReplaceExternalInputs { .. } => IntentPatchOperationKind::ReplaceExternalInputs,
        }
    }

    pub(crate) fn canonical_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).expect("intent patch operation is infallibly serializable")
    }
}

/// Exact-CAS unordered edit set.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IntentPatch {
    pub expected: IntentSessionIdentity,
    pub policy: IntentPatchPolicy,
    operations: Vec<IntentPatchOperation>,
}

impl IntentPatch {
    #[must_use]
    pub fn new(
        expected: IntentSessionIdentity,
        policy: IntentPatchPolicy,
        mut operations: Vec<IntentPatchOperation>,
    ) -> Self {
        operations.sort_by_key(IntentPatchOperation::canonical_bytes);
        Self {
            expected,
            policy,
            operations,
        }
    }

    #[must_use]
    pub fn operations(&self) -> &[IntentPatchOperation] {
        &self.operations
    }

    pub(crate) fn into_operations(mut self) -> Vec<IntentPatchOperation> {
        self.operations
            .sort_by_key(IntentPatchOperation::canonical_bytes);
        self.operations
    }
}

/// Stable IDs allocated for transaction-local aliases.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IntentAliasMap {
    pub nodes: BTreeMap<IntentKey, NodeId>,
    pub ports: BTreeMap<IntentKey, BTreeMap<IntentPortSelector, IntentPortRef>>,
    pub cells: BTreeMap<IntentKey, CellId>,
}

impl IntentAliasMap {
    #[must_use]
    pub fn node(&self, alias: &IntentKey) -> Option<NodeId> {
        self.nodes.get(alias).copied()
    }

    #[must_use]
    pub fn port(&self, alias: &IntentKey, selector: IntentPortSelector) -> Option<IntentPortRef> {
        self.ports
            .get(alias)
            .and_then(|ports| ports.get(&selector))
            .copied()
    }

    #[must_use]
    pub fn cell(&self, alias: &IntentKey) -> Option<CellId> {
        self.cells.get(alias).copied()
    }
}

/// Canonical behavioral effect summary returned before commit.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "independent component-change flags are a compact public transaction audit"
)]
pub struct IntentSemanticDiff {
    pub created_nodes: BTreeSet<NodeId>,
    pub deleted_nodes: BTreeSet<NodeId>,
    pub definition_nodes: BTreeSet<NodeId>,
    pub instance_nodes: BTreeSet<NodeId>,
    pub organization_nodes: BTreeSet<NodeId>,
    pub graph_changed: bool,
    pub instance_changed: bool,
    pub organization_changed: bool,
    pub external_inputs_changed: bool,
}

impl IntentSemanticDiff {
    #[must_use]
    pub const fn requires_materialization(&self) -> bool {
        self.graph_changed || self.instance_changed || self.external_inputs_changed
    }

    /// All declaration identities affected by the transaction, independent
    /// of which projection authored it.
    #[must_use]
    pub fn affected_nodes(&self) -> BTreeSet<NodeId> {
        self.created_nodes
            .iter()
            .chain(&self.deleted_nodes)
            .chain(&self.definition_nodes)
            .chain(&self.instance_nodes)
            .chain(&self.organization_nodes)
            .copied()
            .collect()
    }

    pub(crate) fn absorb_deleted(&mut self, nodes: &BTreeSet<NodeId>) {
        self.deleted_nodes.extend(nodes.iter().copied());
        self.definition_nodes.extend(nodes.iter().copied());
        self.graph_changed |= !nodes.is_empty();
        self.instance_changed |= !nodes.is_empty();
        self.organization_changed |= !nodes.is_empty();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{IntentPortKind, IntentPortRole, PortId};

    const PORT_ROLES: [IntentPortRole; 29] = [
        IntentPortRole::Primary,
        IntentPortRole::Start,
        IntentPortRole::End,
        IntentPortRole::Center,
        IntentPortRole::Midpoint,
        IntentPortRole::Corner,
        IntentPortRole::Control,
        IntentPortRole::MajorAxisPoint,
        IntentPortRole::MinorAxisPoint,
        IntentPortRole::Curve,
        IntentPortRole::Span,
        IntentPortRole::Contact,
        IntentPortRole::Target,
        IntentPortRole::Constraint,
        IntentPortRole::Dimension,
        IntentPortRole::Source,
        IntentPortRole::Catalog,
        IntentPortRole::Operation,
        IntentPortRole::Feature,
        IntentPortRole::FeatureCorner,
        IntentPortRole::Parameter,
        IntentPortRole::Binding,
        IntentPortRole::Output,
        IntentPortRole::External,
        IntentPortRole::Annotation,
        IntentPortRole::Collection,
        IntentPortRole::Profile,
        IntentPortRole::Chain,
        IntentPortRole::Result,
    ];
    const PORT_KINDS: [IntentPortKind; 21] = [
        IntentPortKind::Point,
        IntentPortKind::HandlePoint,
        IntentPortKind::Scalar,
        IntentPortKind::Curve,
        IntentPortKind::CurveSpan,
        IntentPortKind::Contact,
        IntentPortKind::Constraint,
        IntentPortKind::Dimension,
        IntentPortKind::Source,
        IntentPortKind::Parameter,
        IntentPortKind::ParameterBinding,
        IntentPortKind::ParameterOutput,
        IntentPortKind::ExternalBinding,
        IntentPortKind::SemanticCatalog,
        IntentPortKind::Profile,
        IntentPortKind::Chain,
        IntentPortKind::Operation,
        IntentPortKind::Feature,
        IntentPortKind::FeatureCorner,
        IntentPortKind::Annotation,
        IntentPortKind::Collection,
    ];

    fn encoded_port_map(selector: IntentPortSelector, kind: IntentPortKind) -> usize {
        serde_json::to_vec(&BTreeMap::from([(
            selector,
            IntentPortRef {
                node: NodeId::from_raw(u64::MAX),
                port: PortId::from_raw(u64::MAX),
                kind,
            },
        )]))
        .unwrap()
        .len()
    }

    #[test]
    fn every_selector_and_port_kind_encoding_fits_the_128_byte_receipt_charge() {
        let mut maximum = 0;
        for role in PORT_ROLES {
            for kind in PORT_KINDS {
                for selector in [
                    IntentPortSelector::Node {
                        role,
                        index: u16::MAX,
                    },
                    IntentPortSelector::InitialChild {
                        ordinal: u16::MAX,
                        role,
                        index: u16::MAX,
                    },
                ] {
                    let encoded = encoded_port_map(selector, kind);
                    maximum = maximum.max(encoded);
                    assert!(
                        encoded <= 128,
                        "{selector} with {kind:?} encoded to {encoded} bytes"
                    );
                }
            }
        }
        assert_eq!(maximum, 117);
    }

    #[test]
    fn worst_json_escaped_aliases_fit_the_receipt_alias_charges() {
        const ALIAS_FIXED_UPPER_BYTES: usize = 512;
        const PORT_UPPER_BYTES: usize = 128;

        let alias = IntentKey::new("\"".repeat(crate::ids::MAX_INTENT_KEY_BYTES)).unwrap();
        let selector = IntentPortSelector::InitialChild {
            ordinal: u16::MAX,
            role: IntentPortRole::MinorAxisPoint,
            index: u16::MAX,
        };
        let port = IntentPortRef {
            node: NodeId::from_raw(u64::MAX),
            port: PortId::from_raw(u64::MAX),
            kind: IntentPortKind::ParameterBinding,
        };
        let node_aliases = IntentAliasMap {
            nodes: BTreeMap::from([(alias.clone(), NodeId::from_raw(u64::MAX))]),
            ports: BTreeMap::from([(alias.clone(), BTreeMap::from([(selector, port)]))]),
            cells: BTreeMap::new(),
        };
        let node_bound = alias
            .as_str()
            .len()
            .checked_mul(12)
            .and_then(|bytes| bytes.checked_add(ALIAS_FIXED_UPPER_BYTES))
            .and_then(|bytes| bytes.checked_add(PORT_UPPER_BYTES))
            .unwrap();
        assert!(serde_json::to_vec(&node_aliases).unwrap().len() <= node_bound);

        let cell_aliases = IntentAliasMap {
            nodes: BTreeMap::new(),
            ports: BTreeMap::new(),
            cells: BTreeMap::from([(alias.clone(), CellId::from_raw(u64::MAX))]),
        };
        let cell_bound = alias
            .as_str()
            .len()
            .checked_mul(6)
            .and_then(|bytes| bytes.checked_add(ALIAS_FIXED_UPPER_BYTES))
            .unwrap();
        assert!(serde_json::to_vec(&cell_aliases).unwrap().len() <= cell_bound);
    }
}
