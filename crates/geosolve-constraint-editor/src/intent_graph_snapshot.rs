// SPDX-License-Identifier: GPL-3.0-or-later

//! Bounded, presentation-independent query projection of design intent.
//!
//! This projection exposes stable graph identities and current data-only
//! values without becoming materialization authority. Opaque bootstrap bytes
//! are represented only by their authenticated size and SHA-256 digest.

use geosolve_sketch_intent::{
    AggregateKind, BootstrapNativeKind, CellId, ChildId, ComputedFeatureKind, ConstraintKind,
    ContentDigest, DimensionKind, ExternalIntentKind, GeometryRecipeKind, IdentityTransitionKind,
    InputSlot, IntentBootstrapObject, IntentChildSchema, IntentDeclarationDescriptor,
    IntentFieldKey, IntentKey, IntentLiteral, IntentNode, IntentNodeKind, IntentOperationOutput,
    IntentPortKind, IntentPortRef, IntentSession, IntentSessionIdentity, LeafRef, NodeId,
    OperationKind, ParameterIntentKind, intent_content_digest,
};
use serde::{Deserialize, Serialize};

/// Compact authenticated metadata for one opaque bootstrap object.
///
/// Raw payload bytes are deliberately absent. The digest uses the same
/// canonical SHA-256 content identity as the intent persistence layer.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IntentBootstrapMetadata {
    pub kind: BootstrapNativeKind,
    pub codec: IntentKey,
    pub payload_bytes: u64,
    pub payload_sha256: ContentDigest,
}

impl IntentBootstrapMetadata {
    pub(crate) fn from_object(object: &IntentBootstrapObject) -> Self {
        Self {
            kind: object.kind,
            codec: object.codec.clone(),
            payload_bytes: u64::try_from(object.payload.len())
                .expect("bounded bootstrap payload length fits u64"),
            payload_sha256: intent_content_digest(&object.payload),
        }
    }
}

/// Closed node-kind metadata with opaque bootstrap payloads compacted.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "family", rename_all = "snake_case", deny_unknown_fields)]
pub enum IntentGraphNodeKind {
    Geometry {
        recipe: GeometryRecipeKind,
    },
    Constraint {
        constraint: ConstraintKind,
    },
    Dimension {
        dimension: DimensionKind,
    },
    Operation {
        operation: OperationKind,
    },
    ComputedFeature {
        feature: ComputedFeatureKind,
    },
    Aggregate {
        aggregate: AggregateKind,
    },
    Parameter {
        parameter: ParameterIntentKind,
    },
    External {
        external: ExternalIntentKind,
    },
    Bootstrap {
        object: IntentBootstrapMetadata,
    },
    Annotation,
    Identity {
        transition: IdentityTransitionKind,
        port_kind: IntentPortKind,
    },
}

impl IntentGraphNodeKind {
    pub(crate) fn from_kind(kind: &IntentNodeKind) -> Self {
        match kind {
            IntentNodeKind::Geometry { recipe } => Self::Geometry { recipe: *recipe },
            IntentNodeKind::Constraint { constraint } => Self::Constraint {
                constraint: *constraint,
            },
            IntentNodeKind::Dimension { dimension } => Self::Dimension {
                dimension: *dimension,
            },
            IntentNodeKind::Operation { operation } => Self::Operation {
                operation: *operation,
            },
            IntentNodeKind::ComputedFeature { feature } => {
                Self::ComputedFeature { feature: *feature }
            }
            IntentNodeKind::Aggregate { aggregate } => Self::Aggregate {
                aggregate: *aggregate,
            },
            IntentNodeKind::Parameter { parameter } => Self::Parameter {
                parameter: *parameter,
            },
            IntentNodeKind::External { external } => Self::External {
                external: *external,
            },
            IntentNodeKind::Bootstrap { object } => Self::Bootstrap {
                object: IntentBootstrapMetadata::from_object(object),
            },
            IntentNodeKind::Annotation => Self::Annotation,
            IntentNodeKind::Identity {
                transition,
                port_kind,
            } => Self::Identity {
                transition: *transition,
                port_kind: *port_kind,
            },
        }
    }
}

/// One stable typed input binding.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IntentGraphInput {
    pub slot: InputSlot,
    pub source: IntentPortRef,
}

/// One exact data-only declaration field.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IntentGraphDefinitionField {
    pub field: IntentFieldKey,
    pub value: IntentLiteral,
}

/// One currently stored writable instance value.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IntentGraphInstanceLeaf {
    pub leaf: LeafRef,
    pub value: IntentLiteral,
}

/// One stable variable-cardinality child in its declaration-owned order.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IntentGraphChild {
    pub child: ChildId,
    pub schema: IntentChildSchema,
    pub ports: Vec<IntentPortRef>,
}

/// Complete stable query record for one declaration.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IntentGraphDeclaration {
    pub node: NodeId,
    pub symbol: IntentKey,
    pub name: IntentKey,
    pub kind: IntentGraphNodeKind,
    pub bootstrap_origin: Option<IntentBootstrapMetadata>,
    pub suppressed: bool,
    pub inputs: Vec<IntentGraphInput>,
    pub definition_fields: Vec<IntentGraphDefinitionField>,
    pub instance_leaves: Vec<IntentGraphInstanceLeaf>,
    pub operation_outputs: Vec<IntentOperationOutput>,
    pub children: Vec<IntentGraphChild>,
    pub descriptor: IntentDeclarationDescriptor,
    pub dependencies: Vec<NodeId>,
}

/// One presentation-ordered organization cell and its complete declarations.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IntentGraphCell {
    pub cell: CellId,
    pub name: IntentKey,
    pub declarations: Vec<IntentGraphDeclaration>,
}

/// Complete bounded stable query projection of one current intent session.
///
/// Cell and declaration order come solely from organization state. Every
/// nested graph collection retains its stable-ID ordering, while bootstrap
/// payload size is constant in the projection regardless of payload length.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IntentGraphSnapshot {
    pub identity: IntentSessionIdentity,
    pub cells: Vec<IntentGraphCell>,
}

impl IntentGraphSnapshot {
    /// Builds a read-only stable query projection from validated session state.
    ///
    /// # Panics
    ///
    /// Panics only if a previously validated organization references a
    /// missing cell, declaration, or display name.
    #[must_use]
    pub fn from_session(session: &IntentSession) -> Self {
        let graph = session.graph();
        let organization = session.organization();
        let cells = organization
            .cell_order()
            .iter()
            .map(|cell_id| {
                let cell = &organization.cells()[cell_id];
                IntentGraphCell {
                    cell: *cell_id,
                    name: cell.name.clone(),
                    declarations: cell
                        .declarations
                        .iter()
                        .map(|node_id| {
                            declaration(
                                &graph.nodes()[node_id],
                                organization.node_names()[node_id].clone(),
                                session,
                            )
                        })
                        .collect(),
                }
            })
            .collect();
        Self {
            identity: session.identity(),
            cells,
        }
    }
}

fn declaration(
    node: &IntentNode,
    name: IntentKey,
    session: &IntentSession,
) -> IntentGraphDeclaration {
    let instance_leaves = node
        .ports
        .values()
        .flat_map(|port| {
            port.writable.iter().filter_map(|field| {
                let leaf = LeafRef {
                    node: node.id,
                    port: port.id,
                    field: *field,
                };
                session
                    .instance()
                    .values()
                    .get(&leaf)
                    .cloned()
                    .map(|value| IntentGraphInstanceLeaf { leaf, value })
            })
        })
        .collect();
    IntentGraphDeclaration {
        node: node.id,
        symbol: node.symbol.clone(),
        name,
        kind: IntentGraphNodeKind::from_kind(&node.kind),
        bootstrap_origin: node
            .bootstrap_origin
            .as_ref()
            .map(IntentBootstrapMetadata::from_object),
        suppressed: node.suppressed,
        inputs: node
            .inputs
            .iter()
            .map(|(slot, source)| IntentGraphInput {
                slot: *slot,
                source: *source,
            })
            .collect(),
        definition_fields: node
            .fields
            .iter()
            .map(|(field, value)| IntentGraphDefinitionField {
                field: field.clone(),
                value: value.clone(),
            })
            .collect(),
        instance_leaves,
        operation_outputs: node.operation_outputs.clone(),
        children: node
            .child_order
            .iter()
            .map(|child_id| {
                let child = &node.children[child_id];
                IntentGraphChild {
                    child: *child_id,
                    schema: child.schema,
                    ports: child
                        .ports
                        .iter()
                        .map(|port_id| node.ports[port_id].as_ref(node.id))
                        .collect(),
                }
            })
            .collect(),
        descriptor: node.descriptor(),
        dependencies: node.dependencies().into_iter().collect(),
    }
}
