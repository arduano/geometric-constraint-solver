// SPDX-License-Identifier: GPL-3.0-or-later

//! Deterministic computed-feature projection for the intent materializer.
//!
//! This adapter reconstructs the existing computed-feature domain exactly. It
//! owns no geometry equations and never chooses a Fillet branch: every parent,
//! contact, neighbourhood, side, retained endpoint and arc orientation comes
//! from typed intent fields.

use std::collections::BTreeMap;

use geosolve_sketch::{
    ContactNeighborhood, DocumentArcSweep, DocumentCurveNormalSide, DocumentFilletEndpointOrder,
    DocumentFilletTrimEndpoint, DocumentId, DocumentTrimParameter, OperationControl,
    OperationOutcome, RetainedSketchDocumentSession,
};
use geosolve_sketch_features::{
    ComputedEvaluationAllocator, ComputedEvaluationAllocatorHighWater, ComputedFeature,
    ComputedFeatureAllocatorHighWater, ComputedFeatureCornerId, ComputedFeatureDefinition,
    ComputedFeatureDocument, ComputedFeatureDocumentError, ComputedFeatureDocumentId,
    ComputedFeatureEvaluationError, ComputedFeatureEvaluationPolicy,
    ComputedFeatureEvaluationSnapshot, ComputedFeatureEvaluationState, ComputedFeatureId,
    ComputedFeatureLifecycleHighWater, ComputedFeatureObjectBootstrap, ComputedFeatureRevision,
    ComputedFeatureSnapshot, ComputedFeatureSnapshotError, ComputedFilletCorner,
    ComputedFilletParent, ComputedFilletSet, NativeCurveSpanSource,
};
use geosolve_sketch_intent::{
    ComputedFeatureKind, InputRole, InputSlot, IntentGraph, IntentLiteral, IntentNode,
    IntentNodeKind, IntentPortKind, IntentPortRole, IntentPortSelector, IntentUnit, NodeId,
};
use thiserror::Error;

use crate::{IntentMaterializationMap, IntentNativeBinding, IntentNodeMaterialization};

const FEATURE_DOCUMENT_NAMESPACE: u128 = 0x6d38_335f_696e_7465_6e74_5f66_6561_7475;

/// Fully evaluated computed sidecar belonging to one cold intent result.
#[derive(Clone, Debug)]
pub(crate) struct ComputedIntentMaterialization {
    pub features: ComputedFeatureDocument,
    pub feature_lifecycle_high_water: ComputedFeatureLifecycleHighWater,
    pub snapshot: ComputedFeatureSnapshot,
    pub evaluation_high_water: ComputedEvaluationAllocatorHighWater,
}

/// Typed failure from exact computed-feature reconstruction or evaluation.
#[derive(Debug, Error)]
pub(crate) enum ComputedIntentMaterializationError {
    #[error("computed-feature node {node} has invalid explicit state: {reason}")]
    InvalidNode { node: NodeId, reason: &'static str },
    #[error("computed-feature identity allocation is exhausted")]
    IdentityExhausted,
    #[error("computed-feature node {node} failed deterministic evaluation: {reason}")]
    EvaluationRejected { node: NodeId, reason: String },
    #[error(transparent)]
    Document(#[from] ComputedFeatureDocumentError),
    #[error(transparent)]
    Snapshot(#[from] ComputedFeatureSnapshotError),
    #[error(transparent)]
    Evaluation(#[from] ComputedFeatureEvaluationError),
}

impl ComputedIntentMaterializationError {
    #[must_use]
    pub(crate) const fn failed_node(&self) -> Option<NodeId> {
        match self {
            Self::InvalidNode { node, .. } | Self::EvaluationRejected { node, .. } => Some(*node),
            Self::IdentityExhausted
            | Self::Document(_)
            | Self::Snapshot(_)
            | Self::Evaluation(_) => None,
        }
    }
}

/// Reconstructs and evaluates the computed-feature sidecar, replacing logical
/// placeholder ownership with exact stable feature/corner identities.
#[allow(clippy::too_many_lines)]
pub(crate) fn materialize_computed_features(
    graph: &IntentGraph,
    document: DocumentId,
    session: &RetainedSketchDocumentSession,
    ownership: &mut IntentMaterializationMap,
    base: Option<(&ComputedFeatureDocument, ComputedFeatureLifecycleHighWater)>,
) -> Result<ComputedIntentMaterialization, ComputedIntentMaterializationError> {
    let mut features = base
        .map(|(features, _)| features.features().to_vec())
        .unwrap_or_default();
    let mut feature_nodes = BTreeMap::new();
    let mut feature_suppression = BTreeMap::new();
    let feature_base = base.map_or(0, |(_, lifecycle)| {
        lifecycle.allocator.next_feature_id.raw().saturating_sub(1)
    });
    let corner_base = base.map_or(0, |(_, lifecycle)| {
        lifecycle.allocator.next_corner_id.raw().saturating_sub(1)
    });
    let mut maximum_feature = feature_base;
    let mut maximum_corner = corner_base;
    if let Some((base_features, _)) = base {
        for feature in base_features.features() {
            let node = ownership
                .nodes
                .iter()
                .find(|owner| {
                    owner
                        .owned
                        .contains(&IntentNativeBinding::ComputedFeature(feature.id))
                })
                .map(|owner| owner.node)
                .ok_or(ComputedIntentMaterializationError::InvalidNode {
                    node: NodeId::from_raw(0),
                    reason: "bootstrap computed feature has no exact logical owner",
                })?;
            feature_nodes.insert(feature.id, node);
            feature_suppression.insert(feature.id, feature.suppressed);
        }
    }

    for node_id in
        graph
            .canonical_schedule()
            .map_err(|_| ComputedIntentMaterializationError::InvalidNode {
                node: NodeId::from_raw(0),
                reason: "canonical dependency schedule is invalid",
            })?
    {
        let node = graph
            .node(node_id)
            .ok_or(ComputedIntentMaterializationError::InvalidNode {
                node: node_id,
                reason: "scheduled declaration is missing",
            })?;
        let IntentNodeKind::ComputedFeature { feature } = node.kind else {
            continue;
        };
        let feature_id = ComputedFeatureId::from_raw(
            feature_base
                .checked_add(node.id.raw())
                .ok_or(ComputedIntentMaterializationError::IdentityExhausted)?,
        );
        if feature_id.raw() == 0 {
            return Err(ComputedIntentMaterializationError::InvalidNode {
                node: node.id,
                reason: "feature identity must be nonzero",
            });
        }
        maximum_feature = maximum_feature.max(feature_id.raw());
        bind_feature_owner(node, feature_id, ownership)?;
        let definition = match feature {
            ComputedFeatureKind::FilletSet => {
                let mut corners = Vec::with_capacity(node.child_order.len());
                for (ordinal, child_id) in node.child_order.iter().copied().enumerate() {
                    let child = node.children.get(&child_id).ok_or(
                        ComputedIntentMaterializationError::InvalidNode {
                            node: node.id,
                            reason: "ordered Fillet child is missing",
                        },
                    )?;
                    let corner_id = ComputedFeatureCornerId::from_raw(
                        corner_base
                            .checked_add(child.id.raw())
                            .ok_or(ComputedIntentMaterializationError::IdentityExhausted)?,
                    );
                    if corner_id.raw() == 0 {
                        return Err(ComputedIntentMaterializationError::InvalidNode {
                            node: node.id,
                            reason: "corner identity must be nonzero",
                        });
                    }
                    maximum_corner = maximum_corner.max(corner_id.raw());
                    bind_corner_owner(node, ordinal, child, corner_id, ownership)?;
                    corners.push(fillet_corner(node, ownership, ordinal, corner_id)?);
                }
                ComputedFeatureDefinition::FilletSet(ComputedFilletSet {
                    radius: quantity(node, "radius", IntentUnit::Length)?,
                    corners,
                })
            }
        };
        feature_nodes.insert(feature_id, node.id);
        feature_suppression.insert(feature_id, node.suppressed);
        features.push(ComputedFeature {
            id: feature_id,
            label: native_feature_name(node)?.to_owned(),
            suppressed: node.suppressed,
            definition,
        });
    }

    let next_feature_id = maximum_feature
        .checked_add(1)
        .filter(|value| *value != 0)
        .map(ComputedFeatureId::from_raw)
        .ok_or(ComputedIntentMaterializationError::IdentityExhausted)?;
    let next_corner_id = maximum_corner
        .checked_add(1)
        .filter(|value| *value != 0)
        .map(ComputedFeatureCornerId::from_raw)
        .ok_or(ComputedIntentMaterializationError::IdentityExhausted)?;
    if !graph
        .nodes()
        .values()
        .any(|node| matches!(node.kind, IntentNodeKind::ComputedFeature { .. }))
        && let Some((features, lifecycle)) = base
    {
        return evaluate_computed_features(
            session,
            features.clone(),
            lifecycle,
            &feature_nodes,
            &feature_suppression,
        );
    }
    let revision_raw = base.map_or(graph.identity().0.revision.raw(), |(_, lifecycle)| {
        lifecycle
            .revision
            .raw()
            .saturating_add(graph.identity().0.revision.raw())
    });
    if revision_raw == u64::MAX {
        return Err(ComputedIntentMaterializationError::IdentityExhausted);
    }
    let revision = ComputedFeatureRevision::from_raw(revision_raw);
    let allocator = ComputedFeatureAllocatorHighWater {
        next_feature_id,
        next_corner_id,
    };
    let lifecycle = ComputedFeatureLifecycleHighWater {
        revision,
        allocator,
    };
    let document_id = base.map_or_else(
        || {
            let raw_document = document.0.as_u128() ^ FEATURE_DOCUMENT_NAMESPACE;
            ComputedFeatureDocumentId::from_raw(if raw_document == 0 { 1 } else { raw_document })
        },
        |(features, _)| features.id(),
    );
    let mut bootstrap =
        ComputedFeatureObjectBootstrap::new(document, document_id, revision, allocator, lifecycle)?;
    for feature in features {
        bootstrap.push_feature(feature);
    }
    let features = bootstrap.finish()?;

    evaluate_computed_features(
        session,
        features,
        lifecycle,
        &feature_nodes,
        &feature_suppression,
    )
}

fn evaluate_computed_features(
    session: &RetainedSketchDocumentSession,
    features: ComputedFeatureDocument,
    feature_lifecycle_high_water: ComputedFeatureLifecycleHighWater,
    feature_nodes: &BTreeMap<ComputedFeatureId, NodeId>,
    feature_suppression: &BTreeMap<ComputedFeatureId, bool>,
) -> Result<ComputedIntentMaterialization, ComputedIntentMaterializationError> {
    let mut evaluation_allocator = ComputedEvaluationAllocator::default();
    let outcome = ComputedFeatureEvaluationSnapshot::capture(
        session,
        &features,
        ComputedFeatureEvaluationPolicy::default(),
    )?
    .prepare(&mut evaluation_allocator)?
    .execute(OperationControl::unlimited())?;
    let snapshot = match outcome {
        OperationOutcome::Completed { value, .. } => value,
        OperationOutcome::Cancelled { .. } | OperationOutcome::WorkExhausted { .. } => {
            return Err(ComputedIntentMaterializationError::EvaluationRejected {
                node: feature_nodes
                    .values()
                    .next()
                    .copied()
                    .unwrap_or(NodeId::from_raw(0)),
                reason: "unlimited cold evaluation did not complete".to_owned(),
            });
        }
        _ => {
            return Err(ComputedIntentMaterializationError::EvaluationRejected {
                node: feature_nodes
                    .values()
                    .next()
                    .copied()
                    .unwrap_or(NodeId::from_raw(0)),
                reason: "computed evaluator returned an unknown outcome".to_owned(),
            });
        }
    };
    for evaluation in snapshot.feature_evaluations() {
        let node = feature_nodes.get(&evaluation.feature).copied().ok_or(
            ComputedIntentMaterializationError::EvaluationRejected {
                node: NodeId::from_raw(0),
                reason: "evaluation returned an unowned feature".to_owned(),
            },
        )?;
        let suppressed = feature_suppression
            .get(&evaluation.feature)
            .copied()
            .ok_or(ComputedIntentMaterializationError::EvaluationRejected {
                node,
                reason: "evaluation suppression has no exact owner".to_owned(),
            })?;
        match (&evaluation.state, suppressed) {
            (ComputedFeatureEvaluationState::Current { .. }, false)
            | (ComputedFeatureEvaluationState::Suppressed, true) => {}
            (ComputedFeatureEvaluationState::Failed { failure }, false) => {
                return Err(ComputedIntentMaterializationError::EvaluationRejected {
                    node,
                    reason: failure.to_string(),
                });
            }
            _ => {
                return Err(ComputedIntentMaterializationError::EvaluationRejected {
                    node,
                    reason: "evaluation state disagrees with explicit suppression".to_owned(),
                });
            }
        }
    }
    if snapshot.feature_evaluations().len() != feature_nodes.len() {
        return Err(ComputedIntentMaterializationError::EvaluationRejected {
            node: feature_nodes
                .values()
                .next()
                .copied()
                .unwrap_or(NodeId::from_raw(0)),
            reason: "evaluation omitted a declared feature".to_owned(),
        });
    }

    Ok(ComputedIntentMaterialization {
        features,
        feature_lifecycle_high_water,
        snapshot,
        evaluation_high_water: evaluation_allocator.high_water(),
    })
}

fn bind_feature_owner(
    node: &IntentNode,
    feature: ComputedFeatureId,
    ownership: &mut IntentMaterializationMap,
) -> Result<(), ComputedIntentMaterializationError> {
    let port = node
        .port_by_selector(IntentPortSelector::Node {
            role: IntentPortRole::Feature,
            index: 0,
        })
        .ok_or(ComputedIntentMaterializationError::InvalidNode {
            node: node.id,
            reason: "feature output port is missing",
        })?;
    replace_owner_binding(
        node.id,
        port.as_ref(node.id),
        IntentNativeBinding::ComputedFeature(feature),
        ownership,
    )
}

fn bind_corner_owner(
    node: &IntentNode,
    ordinal: usize,
    child: &geosolve_sketch_intent::IntentChild,
    corner: ComputedFeatureCornerId,
    ownership: &mut IntentMaterializationMap,
) -> Result<(), ComputedIntentMaterializationError> {
    let port = child
        .ports
        .iter()
        .filter_map(|id| node.port(*id))
        .find(|port| {
            port.kind == IntentPortKind::FeatureCorner
                && port.selector
                    == (IntentPortSelector::InitialChild {
                        ordinal: u16::try_from(ordinal).unwrap_or(u16::MAX),
                        role: IntentPortRole::FeatureCorner,
                        index: 0,
                    })
        })
        .ok_or(ComputedIntentMaterializationError::InvalidNode {
            node: node.id,
            reason: "Fillet corner output port is missing",
        })?;
    replace_owner_binding(
        node.id,
        port.as_ref(node.id),
        IntentNativeBinding::ComputedFeatureCorner(corner),
        ownership,
    )
}

fn replace_owner_binding(
    node: NodeId,
    port: geosolve_sketch_intent::IntentPortRef,
    binding: IntentNativeBinding,
    ownership: &mut IntentMaterializationMap,
) -> Result<(), ComputedIntentMaterializationError> {
    let index = ownership
        .ports
        .binary_search_by_key(&port, |(candidate, _)| *candidate)
        .map_err(|_| ComputedIntentMaterializationError::InvalidNode {
            node,
            reason: "logical output is absent from ownership",
        })?;
    ownership.ports[index].1 = binding;
    match ownership
        .nodes
        .binary_search_by_key(&node, |materialized| materialized.node)
    {
        Ok(index) => {
            ownership.nodes[index].owned.push(binding);
            ownership.nodes[index].owned.sort_unstable();
            ownership.nodes[index].owned.dedup();
        }
        Err(index) => ownership.nodes.insert(
            index,
            IntentNodeMaterialization {
                node,
                owned: vec![binding],
            },
        ),
    }
    Ok(())
}

fn fillet_corner(
    node: &IntentNode,
    ownership: &IntentMaterializationMap,
    ordinal: usize,
    id: ComputedFeatureCornerId,
) -> Result<ComputedFilletCorner, ComputedIntentMaterializationError> {
    let first_index = u16::try_from(ordinal.saturating_mul(2)).map_err(|_| {
        ComputedIntentMaterializationError::InvalidNode {
            node: node.id,
            reason: "Fillet input index exceeds intent limits",
        }
    })?;
    let second_index =
        first_index
            .checked_add(1)
            .ok_or(ComputedIntentMaterializationError::InvalidNode {
                node: node.id,
                reason: "Fillet input index exceeds intent limits",
            })?;
    let corner = geosolve_sketch_features::NewComputedFilletCorner {
        first: fillet_parent(node, ownership, ordinal, "first", first_index)?,
        second: fillet_parent(node, ownership, ordinal, "second", second_index)?,
        endpoint_order: match enum_value(node, &corner_field(ordinal, "endpoint_order"))? {
            "first_then_second" => DocumentFilletEndpointOrder::FirstThenSecond,
            "second_then_first" => DocumentFilletEndpointOrder::SecondThenFirst,
            _ => return Err(invalid(node, "Fillet endpoint order is invalid")),
        },
        sweep: match enum_value(node, &corner_field(ordinal, "sweep"))? {
            "counter_clockwise" => DocumentArcSweep::CounterClockwise,
            "clockwise" => DocumentArcSweep::Clockwise,
            _ => return Err(invalid(node, "Fillet arc sweep is invalid")),
        },
    }
    .canonicalized();
    Ok(ComputedFilletCorner {
        id,
        first: corner.first,
        second: corner.second,
        endpoint_order: corner.endpoint_order,
        sweep: corner.sweep,
    })
}

fn fillet_parent(
    node: &IntentNode,
    ownership: &IntentMaterializationMap,
    ordinal: usize,
    parent: &str,
    input_index: u16,
) -> Result<ComputedFilletParent, ComputedIntentMaterializationError> {
    let prefix = format!("corner_{ordinal:04}_{parent}");
    let source_port = node
        .inputs
        .get(&InputSlot::new(InputRole::Span, input_index))
        .copied()
        .ok_or(ComputedIntentMaterializationError::InvalidNode {
            node: node.id,
            reason: "Fillet span input is missing",
        })?;
    let source = match ownership.port(source_port) {
        Some(IntentNativeBinding::CurveSpan(span)) => NativeCurveSpanSource { span },
        _ => {
            return Err(ComputedIntentMaterializationError::InvalidNode {
                node: node.id,
                reason: "Fillet input is not an exact native span",
            });
        }
    };
    let neighborhood = match enum_value(node, &format!("{prefix}_neighborhood"))? {
        "interior" => {
            require_absent(node, &format!("{prefix}_local_lower"))?;
            require_absent(node, &format!("{prefix}_local_upper"))?;
            ContactNeighborhood::Interior
        }
        "local" => ContactNeighborhood::Local {
            lower: quantity(
                node,
                &format!("{prefix}_local_lower"),
                IntentUnit::Dimensionless,
            )?,
            upper: quantity(
                node,
                &format!("{prefix}_local_upper"),
                IntentUnit::Dimensionless,
            )?,
        },
        "start" => {
            require_absent(node, &format!("{prefix}_local_lower"))?;
            require_absent(node, &format!("{prefix}_local_upper"))?;
            ContactNeighborhood::Start
        }
        "end" => {
            require_absent(node, &format!("{prefix}_local_lower"))?;
            require_absent(node, &format!("{prefix}_local_upper"))?;
            ContactNeighborhood::End
        }
        _ => return Err(invalid(node, "Fillet contact neighbourhood is invalid")),
    };
    let periodic_anchor = if boolean(node, &format!("{prefix}_periodic_anchor"))? {
        Some(DocumentTrimParameter {
            parameter: quantity(
                node,
                &format!("{prefix}_anchor_parameter"),
                IntentUnit::Dimensionless,
            )?,
            winding: integer(node, &format!("{prefix}_anchor_winding"))?,
        })
    } else {
        require_absent(node, &format!("{prefix}_anchor_parameter"))?;
        require_absent(node, &format!("{prefix}_anchor_winding"))?;
        None
    };
    Ok(ComputedFilletParent {
        source,
        picked_parameter: quantity(
            node,
            &format!("{prefix}_parameter"),
            IntentUnit::Dimensionless,
        )?,
        winding: integer(node, &format!("{prefix}_winding"))?,
        neighborhood,
        normal_side: match enum_value(node, &format!("{prefix}_normal_side"))? {
            "left" => DocumentCurveNormalSide::Left,
            "right" => DocumentCurveNormalSide::Right,
            _ => return Err(invalid(node, "Fillet normal side is invalid")),
        },
        retained_endpoint: match enum_value(node, &format!("{prefix}_trim_endpoint"))? {
            "start" => DocumentFilletTrimEndpoint::Start,
            "end" => DocumentFilletTrimEndpoint::End,
            _ => return Err(invalid(node, "Fillet retained endpoint is invalid")),
        },
        periodic_anchor,
    })
}

fn corner_field(ordinal: usize, suffix: &str) -> String {
    format!("corner_{ordinal:04}_{suffix}")
}

fn field<'a>(node: &'a IntentNode, name: &str) -> Option<&'a IntentLiteral> {
    node.fields
        .iter()
        .find_map(|(key, value)| (key.0.as_str() == name).then_some(value))
}

fn native_feature_name(node: &IntentNode) -> Result<&str, ComputedIntentMaterializationError> {
    match field(node, "name") {
        Some(IntentLiteral::Text(value)) => Ok(value.as_str()),
        None => Ok(node.symbol.as_str()),
        Some(_) => Err(invalid(node, "computed feature name is invalid")),
    }
}

fn quantity(
    node: &IntentNode,
    name: &str,
    unit: IntentUnit,
) -> Result<f64, ComputedIntentMaterializationError> {
    match field(node, name) {
        Some(IntentLiteral::Quantity {
            value,
            unit: actual,
        }) if *actual == unit => Ok(*value),
        _ => Err(invalid(
            node,
            "required Fillet quantity is missing or invalid",
        )),
    }
}

fn integer(node: &IntentNode, name: &str) -> Result<i32, ComputedIntentMaterializationError> {
    match field(node, name) {
        Some(IntentLiteral::Integer(value)) => {
            i32::try_from(*value).map_err(|_| invalid(node, "Fillet winding exceeds i32 limits"))
        }
        _ => Err(invalid(
            node,
            "required Fillet integer is missing or invalid",
        )),
    }
}

fn boolean(node: &IntentNode, name: &str) -> Result<bool, ComputedIntentMaterializationError> {
    match field(node, name) {
        Some(IntentLiteral::Boolean(value)) => Ok(*value),
        _ => Err(invalid(
            node,
            "required Fillet boolean is missing or invalid",
        )),
    }
}

fn enum_value<'a>(
    node: &'a IntentNode,
    name: &str,
) -> Result<&'a str, ComputedIntentMaterializationError> {
    match field(node, name) {
        Some(IntentLiteral::Enum(value)) => Ok(value.as_str()),
        _ => Err(invalid(node, "required Fillet enum is missing or invalid")),
    }
}

fn require_absent(node: &IntentNode, name: &str) -> Result<(), ComputedIntentMaterializationError> {
    if field(node, name).is_some() {
        return Err(invalid(
            node,
            "inactive conditional Fillet metadata must be absent",
        ));
    }
    Ok(())
}

const fn invalid(node: &IntentNode, reason: &'static str) -> ComputedIntentMaterializationError {
    ComputedIntentMaterializationError::InvalidNode {
        node: node.id,
        reason,
    }
}
