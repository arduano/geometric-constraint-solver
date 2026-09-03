// SPDX-License-Identifier: GPL-3.0-or-later

//! Deterministic lowering from projectional sketch intent into the existing
//! persistent sketch and retained native solver.
//!
//! This module owns translation and semantic/native ownership. It deliberately
//! owns no residual equation: acceptance remains the responsibility of
//! [`RetainedSketchDocumentSession`].

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use geosolve_sketch::{
    ContactAdmissibleRange, ContactDomain, ContactId, ContactNeighborhood, ContactSlot,
    CurveCurveFilletRequest, CurveDefinition, CurveFilletParentRequest, CurveId, CurveSpan,
    DesignCurve, DesignPoint, DesignPointId, DesignScalar, DesignScalarId,
    DocumentAngleOrientation, DocumentArcSweep, DocumentArcTangencySide, DocumentBSplineForm,
    DocumentCenterRef, DocumentCircleContainment, DocumentCircleTangencyMode, DocumentConstraint,
    DocumentConstraintDefinition, DocumentConstraintId, DocumentCoordinateAxis,
    DocumentCurveContinuity, DocumentCurveCurvatureRelation, DocumentCurveDirectionRelation,
    DocumentCurveNormalSide, DocumentCurveTrimView, DocumentDimension, DocumentDimensionDefinition,
    DocumentDimensionId, DocumentDimensionMode, DocumentDirectedProfileOffsetCurve,
    DocumentDirectionSense, DocumentElementId, DocumentError, DocumentExternalBinding,
    DocumentExternalBindingId, DocumentExternalLineSupportRef, DocumentExternalPointRef,
    DocumentFaceOffsetDirection, DocumentFilletEndpointOrder, DocumentFilletTrimEndpoint,
    DocumentHyperbolaBranch, DocumentId, DocumentLineOffsetOrientation, DocumentLineSide,
    DocumentLineSupportRef, DocumentOffsetTraversal, DocumentParameter, DocumentParameterBinding,
    DocumentParameterId, DocumentParameterKind, DocumentParameterOutput, DocumentParameterTarget,
    DocumentProfileOffsetChain, DocumentProfileOffsetEdgePair, DocumentProfileOffsetJunctionOwner,
    DocumentProfileOffsetLoop, DocumentProfileOffsetOperand, DocumentProfileOffsetTerminalPolicy,
    DocumentScalarBranch, DocumentScalarPropertyRef, DocumentScalarUnit, DocumentSessionError,
    DocumentSolveRequest, DocumentSourceId, DocumentTrimBoundary, DocumentTrimParameter,
    ExternalFeatureKindV1, ExternalSnapshotSet, ExternalTopologyDigest, FeatureEndpoint,
    GeometryRole, GeometryRoleEdit, MIN_RATIONAL_QUADRATIC_MIDDLE_WEIGHT, OperationControl,
    OperationOutcome, ParameterBatch, PersistentId, RetainedSketchDocumentSession,
    SKETCH_ACCEPTANCE_RESIDUAL_TOLERANCE, ScalarDomain, ScalarUnit, SketchHardValidity,
    SketchMaterializationBatch, SketchMaterializationConstraintReservation,
    SketchMaterializationDimensionReservation, SketchMaterializationIdentityKind,
    SketchMaterializationIdentityReservation, SketchMaterializationReservationAllocator,
    SketchPersistentIdentityHighWater, SolverConfig, TangentOrientation,
};
use geosolve_sketch_intent::{
    AggregateKind, BootstrapNativeKind, ConstraintKind, DimensionKind, ExternalIntentKind,
    GeometryRecipeKind, InputRole, InputSlot, IntentAcceptedAuthority, IntentAllocatorHighWater,
    IntentCandidate, IntentEvaluation, IntentEvaluationFailure, IntentEvaluationFailureKind,
    IntentExternalInputs, IntentGraph, IntentGraphError, IntentIdentityFlow, IntentInstanceState,
    IntentKey, IntentLiteral, IntentNativeReservationKind, IntentNode, IntentNodeKind,
    IntentOperationOutput, IntentOperationOutputKind, IntentPatch, IntentPlanError, IntentPort,
    IntentPortKind, IntentPortRef, IntentPortRole, IntentPortSelector, IntentReservationLedger,
    IntentReservationRecord, IntentReservationState, IntentSemanticIdentity, IntentSession,
    IntentSessionError, IntentSessionIdentity, IntentUnit, LeafField, LeafRef,
    MaterializationEvidence, NodeId, OperationKind as IntentOperationKind, ParameterIntentKind,
    ReservationId,
};
use geosolve_sketch_ops::{
    LineEndpoint, SketchOperationKind, SketchOperationOutputPathSegment, SketchOperationOutputPlan,
    SketchOperationOutputRole, SketchOperationProposal, SketchOperationRequest,
    SketchOperationResult, SketchOperationSnapshot, SketchProfileOffsetOperand, SplitRetainedPiece,
    TrimRetainedSide,
};
use geosolve_sketch_topology::{
    EndpointTopologyIndex, EndpointTopologyRequest, OffsetDirectedSpan, OffsetEndpointRef,
    OffsetEndpointRole, OffsetOperandIndex, OffsetOperandRequest, OffsetTraversal,
    PreparedEndpointTopologyQuery, PreparedOffsetOperandQuery,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::intent_inputs::decode_intent_external_inputs;

/// One native identity or semantic span bound to a stable intent output.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum IntentNativeBinding {
    Point(DesignPointId),
    Scalar(DesignScalarId),
    Curve(CurveId),
    CurveSpan(CurveSpan),
    Contact(ContactId),
    Constraint(DocumentConstraintId),
    Dimension(DocumentDimensionId),
    Source(DocumentSourceId),
    Parameter(DocumentParameterId),
    ExternalBinding(DocumentExternalBindingId),
    /// Stable persistent computed-feature identity owned by one declaration.
    ComputedFeature(geosolve_sketch_features::ComputedFeatureId),
    /// Stable persistent corner identity owned by one variable child.
    ComputedFeatureCorner(geosolve_sketch_features::ComputedFeatureCornerId),
    /// Stable equation-free output identity retained entirely by the intent
    /// graph (annotations, aggregates, declarations and identity transitions).
    Logical(IntentPortRef),
}

/// Native variable leaf that may route an accepted direct-manipulation result
/// back to exactly one writable intent leaf.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum IntentNativeWritableLeaf {
    PointX { point: DesignPointId },
    PointY { point: DesignPointId },
    ScalarValue { scalar: DesignScalarId },
}

/// Native objects reserved by one declaration. Aliased operands are absent;
/// their logical ports still appear in [`IntentMaterializationMap::ports`].
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IntentNodeMaterialization {
    pub node: NodeId,
    pub owned: Vec<IntentNativeBinding>,
}

/// Equation-free ordered native spans retained by one logical aggregate output.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IntentAggregateMaterialization {
    pub port: IntentPortRef,
    pub spans: Vec<CurveSpan>,
    pub closed: bool,
}

/// Complete deterministic logical/native ownership and reverse writable-leaf map.
///
/// Vectors are kept in stable-key order so the same value is canonical JSON on
/// native and WASM targets without relying on JSON object-key coercion.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IntentMaterializationMap {
    pub semantic: IntentSemanticIdentity,
    pub nodes: Vec<IntentNodeMaterialization>,
    pub ports: Vec<(IntentPortRef, IntentNativeBinding)>,
    pub reservations: Vec<(ReservationId, IntentNativeBinding)>,
    pub writable_leaves: Vec<(IntentNativeWritableLeaf, LeafRef)>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub aggregates: Vec<IntentAggregateMaterialization>,
}

impl IntentMaterializationMap {
    /// Resolves deterministic native ownership for one declaration.
    #[must_use]
    pub fn node(&self, node: NodeId) -> Option<&IntentNodeMaterialization> {
        self.nodes
            .binary_search_by_key(&node, |candidate| candidate.node)
            .ok()
            .map(|index| &self.nodes[index])
    }

    /// Returns the sole declaration owner of a native binding.
    #[must_use]
    pub fn exact_owner(&self, binding: IntentNativeBinding) -> Option<NodeId> {
        let mut owners = self
            .nodes
            .iter()
            .filter(|node| node.owned.binary_search(&binding).is_ok())
            .map(|node| node.node);
        let owner = owners.next()?;
        owners.next().is_none().then_some(owner)
    }

    /// Resolves one stable logical output.
    #[must_use]
    pub fn port(&self, port: IntentPortRef) -> Option<IntentNativeBinding> {
        self.ports
            .binary_search_by_key(&port, |(candidate, _)| *candidate)
            .ok()
            .map(|index| self.ports[index].1)
    }

    /// Resolves one never-reused native reservation, including a retired one.
    #[must_use]
    pub fn reservation(&self, reservation: ReservationId) -> Option<IntentNativeBinding> {
        self.reservations
            .binary_search_by_key(&reservation, |(candidate, _)| *candidate)
            .ok()
            .map(|index| self.reservations[index].1)
    }

    /// Returns the exact writable intent owner for a native direct-manipulation leaf.
    #[must_use]
    pub fn writable_leaf(&self, native: IntentNativeWritableLeaf) -> Option<LeafRef> {
        self.writable_leaves
            .binary_search_by_key(&native, |(candidate, _)| *candidate)
            .ok()
            .map(|index| self.writable_leaves[index].1)
    }

    /// Resolves one equation-free chain, profile, or geometry collection output.
    #[must_use]
    pub fn aggregate(&self, port: IntentPortRef) -> Option<&IntentAggregateMaterialization> {
        self.aggregates
            .binary_search_by_key(&port, |candidate| candidate.port)
            .ok()
            .map(|index| &self.aggregates[index])
    }

    /// Resolves the ordered native spans owned by one logical aggregate output.
    #[must_use]
    pub fn aggregate_spans(&self, port: IntentPortRef) -> Option<&[CurveSpan]> {
        self.aggregate(port)
            .map(|aggregate| aggregate.spans.as_slice())
    }

    /// Reports whether one resolved aggregate has closed-profile semantics.
    #[must_use]
    pub fn aggregate_is_closed(&self, port: IntentPortRef) -> Option<bool> {
        self.aggregate(port).map(|aggregate| aggregate.closed)
    }

    /// Independently checks decoded or freshly produced ownership evidence
    /// against its exact intent and native authorities.
    ///
    /// # Errors
    ///
    /// Returns a typed materialization error when semantic identity, stable
    /// ordering, graph/instance ownership, native existence, reservation
    /// provenance, reverse writable ownership, or aggregate topology does not
    /// match the supplied authorities.
    pub fn validate_against(
        &self,
        expected_semantic: IntentSemanticIdentity,
        graph: &IntentGraph,
        instance: &IntentInstanceState,
        reservations: &IntentReservationLedger,
        document: &geosolve_sketch::SketchDocument,
        features: &geosolve_sketch_features::ComputedFeatureDocument,
    ) -> Result<(), IntentMaterializationError> {
        if self.semantic != expected_semantic {
            return Err(IntentMaterializationError::OwnershipSemanticMismatch);
        }
        graph.validate()?;
        self.validate_ordering()?;
        self.validate_reservations(graph, reservations, document, features)?;
        self.validate_ports(graph, reservations, document, features)?;
        self.validate_owned_logical_provenance(graph, document, features)?;
        self.validate_nodes(graph, reservations, document, features)?;
        self.validate_writable_leaves(graph, instance, document)?;
        self.validate_aggregates(graph, document)
    }

    fn validate_ordering(&self) -> Result<(), IntentMaterializationError> {
        if !strictly_sorted_unique_by(&self.nodes, |node| node.node)
            || self
                .nodes
                .iter()
                .any(|node| !strictly_sorted_unique(&node.owned))
            || !strictly_sorted_unique_by(&self.ports, |(port, _)| *port)
            || !strictly_sorted_unique_by(&self.reservations, |(reservation, _)| *reservation)
            || !strictly_sorted_unique_by(&self.writable_leaves, |(native, _)| *native)
            || !strictly_sorted_unique_by(&self.aggregates, |aggregate| aggregate.port)
        {
            return Err(IntentMaterializationError::InvalidOwnershipOrdering);
        }
        Ok(())
    }

    fn validate_reservations(
        &self,
        graph: &IntentGraph,
        reservations: &IntentReservationLedger,
        document: &geosolve_sketch::SketchDocument,
        features: &geosolve_sketch_features::ComputedFeatureDocument,
    ) -> Result<(), IntentMaterializationError> {
        let mut native_reservations = BTreeMap::new();
        for (reservation, binding) in &self.reservations {
            let record = reservations.entries().get(reservation).ok_or(
                IntentMaterializationError::UnknownOwnershipReservation {
                    reservation: *reservation,
                },
            )?;
            if !binding_matches_reservation(record.kind, *binding) {
                return Err(
                    IntentMaterializationError::OwnershipReservationKindMismatch {
                        reservation: *reservation,
                    },
                );
            }
            if native_reservations.insert(*binding, *reservation).is_some() {
                return Err(IntentMaterializationError::DuplicateReservationBinding {
                    binding: *binding,
                });
            }
            if record.state == IntentReservationState::Declared {
                validate_native_binding(*binding, document, features)?;
            }
        }

        for (reservation, record) in reservations.entries() {
            if self.reservation(*reservation).is_none() {
                return Err(IntentMaterializationError::MissingOwnershipReservation {
                    reservation: *reservation,
                });
            }
            if record.state == IntentReservationState::Tombstoned {
                continue;
            }
            let reference = IntentPortRef {
                node: record.owner_node,
                port: record.owner_port,
                kind: record.kind.port_kind(),
            };
            let Some(port) = graph.port(reference) else {
                return Err(
                    IntentMaterializationError::OwnershipReservationOwnerMismatch {
                        reservation: *reservation,
                    },
                );
            };
            if port.flow
                != (IntentIdentityFlow::Created {
                    reservation: *reservation,
                })
            {
                return Err(
                    IntentMaterializationError::OwnershipReservationOwnerMismatch {
                        reservation: *reservation,
                    },
                );
            }
        }
        Ok(())
    }

    fn validate_ports(
        &self,
        graph: &IntentGraph,
        reservations: &IntentReservationLedger,
        document: &geosolve_sketch::SketchDocument,
        features: &geosolve_sketch_features::ComputedFeatureDocument,
    ) -> Result<(), IntentMaterializationError> {
        for (reference, binding) in &self.ports {
            let port = graph
                .port(*reference)
                .ok_or(IntentMaterializationError::UnknownOwnershipPort { port: *reference })?;
            if !binding_matches_port(*reference, port.kind, *binding) {
                return Err(IntentMaterializationError::OwnershipPortKindMismatch {
                    port: *reference,
                });
            }
        }

        for node in graph.nodes().values() {
            for port in node.ports.values() {
                let reference = port.as_ref(node.id);
                let actual = self.port(reference);
                if matches!(port.flow, IntentIdentityFlow::Retired { .. }) {
                    if actual.is_some() {
                        return Err(IntentMaterializationError::UnexpectedOwnershipPort {
                            port: reference,
                        });
                    }
                    continue;
                }
                let binding = actual
                    .ok_or(IntentMaterializationError::MissingOwnershipPort { port: reference })?;
                if !binding_matches_port(reference, port.kind, binding) {
                    return Err(IntentMaterializationError::OwnershipPortKindMismatch {
                        port: reference,
                    });
                }
                let exact = match port.flow {
                    IntentIdentityFlow::Created { reservation } => {
                        let record = reservations.entries().get(&reservation).ok_or(
                            IntentMaterializationError::MissingOwnershipReservation { reservation },
                        )?;
                        record.owner_node == node.id
                            && record.owner_port == port.id
                            && self.reservation(reservation) == Some(binding)
                    }
                    IntentIdentityFlow::Aliased { source }
                    | IntentIdentityFlow::Continued { source, .. } => {
                        self.port(source) == Some(binding)
                    }
                    IntentIdentityFlow::OwnedLogical => match binding {
                        IntentNativeBinding::Logical(logical) => logical == reference,
                        IntentNativeBinding::CurveSpan(_) => {
                            port.kind == IntentPortKind::CurveSpan && !node.suppressed
                        }
                        IntentNativeBinding::ComputedFeature(_) => {
                            port.kind == IntentPortKind::Feature
                        }
                        IntentNativeBinding::ComputedFeatureCorner(_) => {
                            port.kind == IntentPortKind::FeatureCorner
                        }
                        _ => false,
                    },
                    IntentIdentityFlow::Retired { .. } => unreachable!("handled above"),
                };
                if !exact {
                    return Err(IntentMaterializationError::OwnershipPortBindingMismatch {
                        port: reference,
                    });
                }
                if !node.suppressed {
                    validate_native_binding(binding, document, features)?;
                }
            }
        }
        Ok(())
    }

    fn validate_nodes(
        &self,
        graph: &IntentGraph,
        reservations: &IntentReservationLedger,
        document: &geosolve_sketch::SketchDocument,
        features: &geosolve_sketch_features::ComputedFeatureDocument,
    ) -> Result<(), IntentMaterializationError> {
        let mut actual = BTreeMap::<NodeId, Vec<IntentNativeBinding>>::new();
        let mut native_owners = BTreeMap::new();
        let reservation_dispositions = self
            .reservations
            .iter()
            .filter_map(|(reservation, binding)| {
                reservations
                    .entries()
                    .get(reservation)
                    .map(|record| (*binding, record.state))
            })
            .collect::<BTreeMap<_, _>>();
        for node in &self.nodes {
            for binding in &node.owned {
                let disposition = reservation_dispositions.get(binding).copied();
                if disposition.is_none_or(|state| state == IntentReservationState::Declared) {
                    validate_native_binding(*binding, document, features)?;
                }
                if native_owners.insert(*binding, node.node).is_some() {
                    return Err(IntentMaterializationError::DuplicateNativeOwner {
                        binding: *binding,
                    });
                }
            }
            actual.insert(node.node, node.owned.clone());
        }

        let mut expected = BTreeMap::<NodeId, Vec<IntentNativeBinding>>::new();
        for (reservation, record) in reservations.entries() {
            expected.entry(record.owner_node).or_default().push(
                self.reservation(*reservation).ok_or(
                    IntentMaterializationError::MissingOwnershipReservation {
                        reservation: *reservation,
                    },
                )?,
            );
        }
        for node in graph.nodes().values() {
            if matches!(node.kind, IntentNodeKind::Bootstrap { .. })
                || node.bootstrap_origin.is_some()
            {
                expected.entry(node.id).or_default();
            }
            for port in node.ports.values() {
                let Some(binding) = self.port(port.as_ref(node.id)) else {
                    continue;
                };
                if matches!(
                    binding,
                    IntentNativeBinding::ComputedFeature(_)
                        | IntentNativeBinding::ComputedFeatureCorner(_)
                ) {
                    expected.entry(node.id).or_default().push(binding);
                }
                if matches!(node.kind, IntentNodeKind::Bootstrap { .. })
                    && let IntentNativeBinding::ComputedFeature(feature) = binding
                {
                    let feature = features.feature(feature).ok_or(
                        IntentMaterializationError::MissingNativeOwnershipBinding { binding },
                    )?;
                    let geosolve_sketch_features::ComputedFeatureDefinition::FilletSet(fillet) =
                        &feature.definition;
                    expected.entry(node.id).or_default().extend(
                        fillet
                            .corners
                            .iter()
                            .map(|corner| IntentNativeBinding::ComputedFeatureCorner(corner.id)),
                    );
                }
            }
        }
        for owned in expected.values_mut() {
            owned.sort_unstable();
            owned.dedup();
        }
        if actual != expected {
            let node = actual
                .keys()
                .chain(expected.keys())
                .copied()
                .find(|node| actual.get(node) != expected.get(node))
                .expect("different ownership maps have one differing node");
            return Err(IntentMaterializationError::OwnershipNodeMismatch { node });
        }
        Ok(())
    }

    fn validate_owned_logical_provenance(
        &self,
        graph: &IntentGraph,
        document: &geosolve_sketch::SketchDocument,
        features: &geosolve_sketch_features::ComputedFeatureDocument,
    ) -> Result<(), IntentMaterializationError> {
        for node in graph.nodes().values() {
            for port in node
                .ports
                .values()
                .filter(|port| port.flow == IntentIdentityFlow::OwnedLogical)
            {
                let reference = port.as_ref(node.id);
                let expected = match port.kind {
                    IntentPortKind::CurveSpan if node.suppressed => {
                        IntentNativeBinding::Logical(reference)
                    }
                    IntentPortKind::CurveSpan => IntentNativeBinding::CurveSpan(
                        expected_logical_curve_span(node, port, self, document)?,
                    ),
                    IntentPortKind::Feature => IntentNativeBinding::ComputedFeature(
                        expected_logical_feature(node, reference, features)?.id,
                    ),
                    IntentPortKind::FeatureCorner => {
                        let feature = expected_logical_feature(node, reference, features)?;
                        let ordinal = match port.selector {
                            IntentPortSelector::InitialChild {
                                ordinal,
                                role: IntentPortRole::FeatureCorner,
                                index: 0,
                            } => usize::from(ordinal),
                            _ => {
                                return Err(
                                    IntentMaterializationError::OwnershipPortBindingMismatch {
                                        port: reference,
                                    },
                                );
                            }
                        };
                        let geosolve_sketch_features::ComputedFeatureDefinition::FilletSet(fillet) =
                            &feature.definition;
                        IntentNativeBinding::ComputedFeatureCorner(
                            fillet
                                .corners
                                .get(ordinal)
                                .ok_or(IntentMaterializationError::OwnershipPortBindingMismatch {
                                    port: reference,
                                })?
                                .id,
                        )
                    }
                    _ => IntentNativeBinding::Logical(reference),
                };
                if self.port(reference) != Some(expected) {
                    return Err(IntentMaterializationError::OwnershipPortBindingMismatch {
                        port: reference,
                    });
                }
            }
        }
        Ok(())
    }

    fn validate_writable_leaves(
        &self,
        graph: &IntentGraph,
        instance: &IntentInstanceState,
        document: &geosolve_sketch::SketchDocument,
    ) -> Result<(), IntentMaterializationError> {
        let mut leaves = BTreeSet::new();
        let mut actual = BTreeMap::new();
        for (native, leaf) in &self.writable_leaves {
            graph.writable_leaf(*leaf)?;
            if !leaves.insert(*leaf) {
                return Err(IntentMaterializationError::DuplicateWritableLeaf { leaf: *leaf });
            }
            validate_writable_binding(*native, *leaf, self, instance, document)?;
            actual.insert(*native, *leaf);
        }

        let mut expected = BTreeMap::new();
        for node in graph.nodes().values().filter(|node| !node.suppressed) {
            for port in node.ports.values() {
                let reference = port.as_ref(node.id);
                let Some(binding) = self.port(reference) else {
                    continue;
                };
                for field in &port.writable {
                    let leaf = LeafRef {
                        node: node.id,
                        port: port.id,
                        field: *field,
                    };
                    let native = native_writable_leaf(binding, *field)
                        .ok_or(IntentMaterializationError::InvalidWritableOwnership { leaf })?;
                    if expected.insert(native, leaf).is_some() {
                        return Err(IntentMaterializationError::DuplicateWritableOwner { native });
                    }
                }
            }
        }
        for (native, leaf) in &expected {
            if actual.get(native) != Some(leaf) {
                return Err(IntentMaterializationError::MissingWritableOwnership { leaf: *leaf });
            }
        }
        for (native, leaf) in actual {
            if expected.get(&native) != Some(&leaf) {
                return Err(IntentMaterializationError::InvalidWritableOwnership { leaf });
            }
        }
        Ok(())
    }

    fn validate_aggregates(
        &self,
        graph: &IntentGraph,
        document: &geosolve_sketch::SketchDocument,
    ) -> Result<(), IntentMaterializationError> {
        let mut expected = BTreeMap::new();
        for node in graph.nodes().values().filter(|node| !node.suppressed) {
            let aggregate = match node.kind {
                IntentNodeKind::Geometry {
                    recipe: GeometryRecipeKind::Polyline,
                } => {
                    let port = require_port(node, IntentPortRole::Collection, 0)?.as_ref(node.id);
                    let Some(IntentNativeBinding::Curve(curve)) =
                        self.port(require_port(node, IntentPortRole::Curve, 0)?.as_ref(node.id))
                    else {
                        return Err(IntentMaterializationError::InvalidOwnershipAggregate { port });
                    };
                    let curve_value = document
                        .curve(curve)
                        .ok_or(IntentMaterializationError::InvalidOwnershipAggregate { port })?;
                    let CurveDefinition::Polyline { closed, .. } = &curve_value.definition else {
                        return Err(IntentMaterializationError::InvalidOwnershipAggregate { port });
                    };
                    let spans = document.curve_spans(curve).map_err(|_| {
                        IntentMaterializationError::InvalidOwnershipAggregate { port }
                    })?;
                    Some(IntentAggregateMaterialization {
                        port,
                        spans,
                        closed: *closed,
                    })
                }
                IntentNodeKind::Aggregate { aggregate } => {
                    let (role, closed) = match aggregate {
                        AggregateKind::OpenChain => (IntentPortRole::Chain, false),
                        AggregateKind::ClosedProfile => (IntentPortRole::Profile, true),
                    };
                    let port = require_port(node, role, 0)?.as_ref(node.id);
                    let mut spans = Vec::with_capacity(node.inputs.len());
                    for index in 0..node.inputs.len() {
                        let index = u16::try_from(index).map_err(|_| {
                            IntentMaterializationError::InvalidOwnershipAggregate { port }
                        })?;
                        let source = node
                            .inputs
                            .get(&InputSlot::new(InputRole::Span, index))
                            .ok_or(IntentMaterializationError::InvalidOwnershipAggregate {
                                port,
                            })?;
                        let span = match self.port(*source) {
                            Some(IntentNativeBinding::CurveSpan(span)) => span,
                            Some(IntentNativeBinding::Curve(curve)) => CurveSpan::line(curve),
                            _ => {
                                return Err(
                                    IntentMaterializationError::InvalidOwnershipAggregate { port },
                                );
                            }
                        };
                        spans.push(span);
                    }
                    Some(IntentAggregateMaterialization {
                        port,
                        spans,
                        closed,
                    })
                }
                _ => None,
            };
            if let Some(aggregate) = aggregate {
                expected.insert(aggregate.port, aggregate);
            }
        }

        for aggregate in &self.aggregates {
            let port = graph.port(aggregate.port).ok_or(
                IntentMaterializationError::UnknownOwnershipPort {
                    port: aggregate.port,
                },
            )?;
            if !matches!(
                port.kind,
                IntentPortKind::Profile | IntentPortKind::Chain | IntentPortKind::Collection
            ) || aggregate
                .spans
                .iter()
                .any(|span| validate_curve_span(*span, document).is_err())
                || expected.get(&aggregate.port) != Some(aggregate)
            {
                return Err(IntentMaterializationError::InvalidOwnershipAggregate {
                    port: aggregate.port,
                });
            }
        }
        if let Some(port) = expected
            .keys()
            .find(|port| self.aggregate(**port).is_none())
            .copied()
        {
            return Err(IntentMaterializationError::InvalidOwnershipAggregate { port });
        }
        Ok(())
    }
}

fn expected_logical_curve_span(
    node: &IntentNode,
    port: &IntentPort,
    ownership: &IntentMaterializationMap,
    document: &geosolve_sketch::SketchDocument,
) -> Result<CurveSpan, IntentMaterializationError> {
    let reference = port.as_ref(node.id);
    if let IntentNodeKind::Bootstrap { object } = &node.kind {
        match object.kind {
            BootstrapNativeKind::Curve => {
                let curve: DesignCurve = serde_json::from_slice(&object.payload).map_err(|_| {
                    IntentMaterializationError::OwnershipPortBindingMismatch { port: reference }
                })?;
                if document.curve(curve.id) != Some(&curve) {
                    return Err(IntentMaterializationError::OwnershipPortBindingMismatch {
                        port: reference,
                    });
                }
            }
            BootstrapNativeKind::CurveTrimView => {
                let trim: DocumentCurveTrimView =
                    serde_json::from_slice(&object.payload).map_err(|_| {
                        IntentMaterializationError::OwnershipPortBindingMismatch { port: reference }
                    })?;
                validate_curve_span(trim.support, document)?;
                return Ok(trim.support);
            }
            _ => {}
        }
    }

    let mut curves = node
        .ports
        .values()
        .filter_map(|candidate| {
            let binding = ownership.port(candidate.as_ref(node.id));
            match binding {
                Some(IntentNativeBinding::Curve(curve)) => Some((candidate.selector, curve)),
                _ => None,
            }
        })
        .collect::<Vec<_>>();
    curves.sort_unstable_by_key(|(selector, _)| *selector);
    let mut spans = Vec::new();
    for (_, curve) in curves {
        spans.extend(document.curve_spans(curve).map_err(|_| {
            IntentMaterializationError::OwnershipPortBindingMismatch { port: reference }
        })?);
    }
    let ordinal = match port.selector {
        IntentPortSelector::Node {
            role: IntentPortRole::Span,
            index,
        } => usize::from(index),
        IntentPortSelector::InitialChild {
            ordinal,
            role: IntentPortRole::Span,
            index: 0,
        } => usize::from(ordinal),
        _ => {
            return Err(IntentMaterializationError::OwnershipPortBindingMismatch {
                port: reference,
            });
        }
    };
    spans
        .get(ordinal)
        .copied()
        .ok_or(IntentMaterializationError::OwnershipPortBindingMismatch { port: reference })
}

fn expected_logical_feature<'a>(
    node: &IntentNode,
    reference: IntentPortRef,
    features: &'a geosolve_sketch_features::ComputedFeatureDocument,
) -> Result<&'a geosolve_sketch_features::ComputedFeature, IntentMaterializationError> {
    if let IntentNodeKind::Bootstrap { object } = &node.kind
        && object.kind == BootstrapNativeKind::ComputedFeature
    {
        let expected: geosolve_sketch_features::ComputedFeature =
            serde_json::from_slice(&object.payload).map_err(|_| {
                IntentMaterializationError::OwnershipPortBindingMismatch { port: reference }
            })?;
        let actual = features
            .feature(expected.id)
            .ok_or(IntentMaterializationError::OwnershipPortBindingMismatch { port: reference })?;
        if actual != &expected {
            return Err(IntentMaterializationError::OwnershipPortBindingMismatch {
                port: reference,
            });
        }
        return Ok(actual);
    }
    if !matches!(node.kind, IntentNodeKind::ComputedFeature { .. }) {
        return Err(IntentMaterializationError::OwnershipPortBindingMismatch { port: reference });
    }
    let expected_label = match field_value(node, "name") {
        Some(IntentLiteral::Text(value)) => value.as_str(),
        None => node.symbol.as_str(),
        Some(_) => {
            return Err(IntentMaterializationError::OwnershipPortBindingMismatch {
                port: reference,
            });
        }
    };
    let mut matching = features
        .features()
        .iter()
        .filter(|feature| feature.label == expected_label);
    let feature = matching
        .next()
        .ok_or(IntentMaterializationError::OwnershipPortBindingMismatch { port: reference })?;
    if matching.next().is_some() {
        return Err(IntentMaterializationError::OwnershipPortBindingMismatch { port: reference });
    }
    Ok(feature)
}

fn strictly_sorted_unique<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn strictly_sorted_unique_by<T, K: Ord + Copy>(values: &[T], key: impl Fn(&T) -> K) -> bool {
    values.windows(2).all(|pair| key(&pair[0]) < key(&pair[1]))
}

fn binding_matches_port(
    reference: IntentPortRef,
    kind: IntentPortKind,
    binding: IntentNativeBinding,
) -> bool {
    matches!(
        (kind, binding),
        (IntentPortKind::Point, IntentNativeBinding::Point(_))
            | (IntentPortKind::Scalar, IntentNativeBinding::Scalar(_))
            | (IntentPortKind::Curve, IntentNativeBinding::Curve(_))
            | (IntentPortKind::CurveSpan, IntentNativeBinding::CurveSpan(_))
            | (IntentPortKind::Contact, IntentNativeBinding::Contact(_))
            | (
                IntentPortKind::Constraint,
                IntentNativeBinding::Constraint(_)
            )
            | (IntentPortKind::Dimension, IntentNativeBinding::Dimension(_))
            | (
                IntentPortKind::Source | IntentPortKind::SemanticCatalog,
                IntentNativeBinding::Source(_)
            )
            | (IntentPortKind::Parameter, IntentNativeBinding::Parameter(_))
            | (
                IntentPortKind::ExternalBinding,
                IntentNativeBinding::ExternalBinding(_)
            )
            | (
                IntentPortKind::Feature,
                IntentNativeBinding::ComputedFeature(_)
            )
            | (
                IntentPortKind::FeatureCorner,
                IntentNativeBinding::ComputedFeatureCorner(_)
            )
    ) || matches!(binding, IntentNativeBinding::Logical(logical) if logical == reference)
}

const fn binding_matches_reservation(
    kind: IntentNativeReservationKind,
    binding: IntentNativeBinding,
) -> bool {
    matches!(
        (kind, binding),
        (
            IntentNativeReservationKind::Point,
            IntentNativeBinding::Point(_)
        ) | (
            IntentNativeReservationKind::Scalar,
            IntentNativeBinding::Scalar(_)
        ) | (
            IntentNativeReservationKind::Curve,
            IntentNativeBinding::Curve(_)
        ) | (
            IntentNativeReservationKind::Contact,
            IntentNativeBinding::Contact(_)
        ) | (
            IntentNativeReservationKind::Constraint,
            IntentNativeBinding::Constraint(_)
        ) | (
            IntentNativeReservationKind::ConstraintSource
                | IntentNativeReservationKind::DimensionSource
                | IntentNativeReservationKind::SemanticCatalog
                | IntentNativeReservationKind::SemanticSource,
            IntentNativeBinding::Source(_)
        ) | (
            IntentNativeReservationKind::Dimension,
            IntentNativeBinding::Dimension(_)
        ) | (
            IntentNativeReservationKind::Parameter,
            IntentNativeBinding::Parameter(_)
        ) | (
            IntentNativeReservationKind::ExternalBinding,
            IntentNativeBinding::ExternalBinding(_)
        )
    )
}

fn validate_native_binding(
    binding: IntentNativeBinding,
    document: &geosolve_sketch::SketchDocument,
    features: &geosolve_sketch_features::ComputedFeatureDocument,
) -> Result<(), IntentMaterializationError> {
    let exists = match binding {
        IntentNativeBinding::Point(id) => document.point(id).is_some(),
        IntentNativeBinding::Scalar(id) => document.scalar(id).is_some(),
        IntentNativeBinding::Curve(id) => document.curve(id).is_some(),
        IntentNativeBinding::CurveSpan(span) => {
            validate_curve_span(span, document)?;
            true
        }
        IntentNativeBinding::Contact(id) => document.contact(id).is_some(),
        IntentNativeBinding::Constraint(id) => document.constraint(id).is_some(),
        IntentNativeBinding::Dimension(id) => document.dimension(id).is_some(),
        IntentNativeBinding::Source(id) => document.source(id).is_some(),
        IntentNativeBinding::Parameter(id) => document.parameter(id).is_some(),
        IntentNativeBinding::ExternalBinding(id) => document.external_binding(id).is_some(),
        IntentNativeBinding::ComputedFeature(id) => features.feature(id).is_some(),
        IntentNativeBinding::ComputedFeatureCorner(id) => features
            .features()
            .iter()
            .any(|feature| features.corner(feature.id, id).is_some()),
        IntentNativeBinding::Logical(_) => true,
    };
    if exists {
        Ok(())
    } else {
        Err(IntentMaterializationError::MissingNativeOwnershipBinding { binding })
    }
}

fn validate_curve_span(
    span: CurveSpan,
    document: &geosolve_sketch::SketchDocument,
) -> Result<(), IntentMaterializationError> {
    if document.curve(span.curve).is_none() {
        return Err(IntentMaterializationError::MissingNativeOwnershipBinding {
            binding: IntentNativeBinding::CurveSpan(span),
        });
    }
    let spans = document
        .curve_spans(span.curve)
        .map_err(|_| IntentMaterializationError::InvalidOwnershipCurveSpan { span })?;
    if spans.contains(&span) {
        Ok(())
    } else {
        Err(IntentMaterializationError::InvalidOwnershipCurveSpan { span })
    }
}

const fn native_writable_leaf(
    binding: IntentNativeBinding,
    field: LeafField,
) -> Option<IntentNativeWritableLeaf> {
    match (binding, field) {
        (IntentNativeBinding::Point(point), LeafField::X) => {
            Some(IntentNativeWritableLeaf::PointX { point })
        }
        (IntentNativeBinding::Point(point), LeafField::Y) => {
            Some(IntentNativeWritableLeaf::PointY { point })
        }
        (
            IntentNativeBinding::Scalar(scalar),
            LeafField::Value | LeafField::Angle | LeafField::Weight | LeafField::Parameter,
        ) => Some(IntentNativeWritableLeaf::ScalarValue { scalar }),
        _ => None,
    }
}

fn validate_writable_binding(
    native: IntentNativeWritableLeaf,
    leaf: LeafRef,
    ownership: &IntentMaterializationMap,
    instance: &IntentInstanceState,
    document: &geosolve_sketch::SketchDocument,
) -> Result<(), IntentMaterializationError> {
    let expected = ownership
        .ports
        .iter()
        .find_map(|(reference, binding)| {
            (reference.node == leaf.node && reference.port == leaf.port).then_some(*binding)
        })
        .ok_or(IntentMaterializationError::UnknownOwnershipLeaf { leaf })?;
    let unit = match (native, expected, leaf.field) {
        (
            IntentNativeWritableLeaf::PointX { point },
            IntentNativeBinding::Point(expected),
            LeafField::X,
        )
        | (
            IntentNativeWritableLeaf::PointY { point },
            IntentNativeBinding::Point(expected),
            LeafField::Y,
        ) if point == expected && document.point(point).is_some() => IntentUnit::Length,
        (
            IntentNativeWritableLeaf::ScalarValue { scalar },
            IntentNativeBinding::Scalar(expected),
            field,
        ) if scalar == expected && document.scalar(scalar).is_some() => scalar_leaf_intent_unit(
            field,
            document.scalar(scalar).expect("checked scalar exists").unit,
        )
        .ok_or(IntentMaterializationError::InvalidWritableOwnership { leaf })?,
        _ => return Err(IntentMaterializationError::InvalidWritableOwnership { leaf }),
    };
    if let Some(IntentLiteral::Quantity { unit: actual, .. }) = instance.values().get(&leaf)
        && *actual != unit
    {
        return Err(IntentMaterializationError::WritableOwnershipUnitMismatch { leaf });
    }
    Ok(())
}

/// Returns the authored Intent unit for one native scalar-backed writable leaf.
///
/// `Parameter` is dimensionless in authored source even when a periodic curve
/// stores it as a native angle. `Weight` is dimensionless over native parameter
/// storage, while only the generic `Value` leaf inherits any native scalar unit.
pub(crate) const fn scalar_leaf_intent_unit(
    field: LeafField,
    native_unit: ScalarUnit,
) -> Option<IntentUnit> {
    match (field, native_unit) {
        (LeafField::Value, ScalarUnit::Length) => Some(IntentUnit::Length),
        (LeafField::Value | LeafField::Angle, ScalarUnit::Angle) => Some(IntentUnit::Angle),
        (LeafField::Value | LeafField::Weight | LeafField::Parameter, ScalarUnit::Parameter)
        | (LeafField::Parameter, ScalarUnit::Angle) => Some(IntentUnit::Dimensionless),
        _ => None,
    }
}

#[cfg(test)]
mod scalar_leaf_intent_unit_tests {
    use super::scalar_leaf_intent_unit;
    use geosolve_sketch::ScalarUnit;
    use geosolve_sketch_intent::{IntentUnit, LeafField};

    #[test]
    fn authored_scalar_leaf_units_form_one_closed_native_storage_matrix() {
        let valid = [
            (LeafField::Value, ScalarUnit::Length, IntentUnit::Length),
            (LeafField::Value, ScalarUnit::Angle, IntentUnit::Angle),
            (
                LeafField::Value,
                ScalarUnit::Parameter,
                IntentUnit::Dimensionless,
            ),
            (LeafField::Angle, ScalarUnit::Angle, IntentUnit::Angle),
            (
                LeafField::Weight,
                ScalarUnit::Parameter,
                IntentUnit::Dimensionless,
            ),
            (
                LeafField::Parameter,
                ScalarUnit::Parameter,
                IntentUnit::Dimensionless,
            ),
            (
                LeafField::Parameter,
                ScalarUnit::Angle,
                IntentUnit::Dimensionless,
            ),
        ];
        for (field, native, authored) in valid {
            assert_eq!(scalar_leaf_intent_unit(field, native), Some(authored));
        }

        let invalid = [
            (LeafField::X, ScalarUnit::Length),
            (LeafField::X, ScalarUnit::Angle),
            (LeafField::X, ScalarUnit::Parameter),
            (LeafField::Y, ScalarUnit::Length),
            (LeafField::Y, ScalarUnit::Angle),
            (LeafField::Y, ScalarUnit::Parameter),
            (LeafField::Angle, ScalarUnit::Length),
            (LeafField::Angle, ScalarUnit::Parameter),
            (LeafField::Weight, ScalarUnit::Length),
            (LeafField::Weight, ScalarUnit::Angle),
            (LeafField::Parameter, ScalarUnit::Length),
        ];
        for (field, native) in invalid {
            assert_eq!(scalar_leaf_intent_unit(field, native), None);
        }
    }
}

/// Compact host-created proof that the materialized document was accepted by
/// the native retained solver and independently hard-residual validated.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IntentValidationEvidence {
    pub semantic: IntentSemanticIdentity,
    pub document: DocumentId,
    pub point_count: usize,
    pub curve_count: usize,
    pub constraint_count: usize,
    pub hard_residuals_validated: bool,
    pub maximum_normalized_hard_residual: Option<f64>,
    pub feature_document: geosolve_sketch_features::ComputedFeatureDocumentId,
    pub feature_revision: geosolve_sketch_features::ComputedFeatureRevision,
    pub feature_digest: geosolve_sketch_features::ComputedFeatureDocumentDigest,
    pub feature_count: usize,
    pub computed_edge_count: usize,
    pub all_active_features_current: bool,
}

/// One complete cold reconstruction suitable for atomic coordinator staging.
#[derive(Clone, Debug)]
pub struct ColdIntentMaterialization {
    pub session: RetainedSketchDocumentSession,
    pub features: geosolve_sketch_features::ComputedFeatureDocument,
    pub feature_lifecycle_high_water: geosolve_sketch_features::ComputedFeatureLifecycleHighWater,
    pub computed: geosolve_sketch_features::ComputedFeatureSnapshot,
    pub computed_evaluation_high_water:
        geosolve_sketch_features::ComputedEvaluationAllocatorHighWater,
    pub ownership: IntentMaterializationMap,
    pub validation: IntentValidationEvidence,
    pub evidence: MaterializationEvidence,
}

/// Persistent-ID-free semantic owner of one dynamic operation-result member.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PreparedIntentOperationSourceRef {
    /// Stable graph symbol of the source declaration. Code adapters map this
    /// through their own authenticated declaration provenance.
    pub declaration: IntentKey,
    /// Exact schema selector within `declaration`.
    pub selector: IntentPortSelector,
    /// Independently authenticated source-port kind.
    pub kind: IntentPortKind,
}

/// Source-relative segment in one native-authenticated operation result path.
///
/// Persistent native IDs never cross this seam. Every dynamic path owner is
/// projected back to its exact declaration and schema selector before the
/// plan reaches an authored-code adapter.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[non_exhaustive]
pub enum PreparedIntentOperationPathSegment {
    Field(String),
    Index(usize),
    SourceControl {
        source: PreparedIntentOperationSourceRef,
    },
    SourceCurve {
        source: PreparedIntentOperationSourceRef,
    },
    SourceSpan {
        source: PreparedIntentOperationSourceRef,
    },
    SourceJunction {
        source: PreparedIntentOperationSourceRef,
    },
}

/// Semantic role of one native-authenticated operation output.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[non_exhaustive]
pub enum PreparedIntentOperationOutputRole {
    Geometry,
    ContactParameter,
    Contact,
    Constraint,
    DimensionTarget,
    Dimension,
    Parameter,
    ExternalBinding,
}

/// One exact output reserved by a prepared native operation.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PreparedIntentOperationOutput {
    pub output: IntentOperationOutput,
    pub path: Vec<PreparedIntentOperationPathSegment>,
    pub role: PreparedIntentOperationOutputRole,
}

/// Non-publishing native result inventory for one provisional intent
/// operation declaration.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PreparedIntentOperationPlan {
    pub operation: IntentOperationKind,
    pub outputs: Vec<PreparedIntentOperationOutput>,
}

/// Failure to stage or natively prepare one provisional operation manifest.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum PreparedIntentOperationPlanError {
    #[error(transparent)]
    Intent(#[from] IntentSessionError),
    #[error(transparent)]
    Plan(#[from] IntentPlanError),
    #[error(transparent)]
    Materialization(#[from] IntentMaterializationError),
    #[error("operation planning expected a pristine intent-session identity")]
    NonPristineSession,
    #[error("operation planning callback was not evaluated")]
    NotEvaluated,
}

/// Stages one provisional create-only patch and prepares the target
/// operation's native result inventory without publishing the patch or a
/// native document.
///
/// This is the compiler-host seam used by executed managed sketches. The
/// target operation is forced active only for planning; the returned manifest
/// may then be attached to the original suppressed or active declaration and
/// is authenticated again during ordinary cold materialization.
///
/// # Errors
///
/// Rejects a non-pristine exact-CAS identity, invalid patch, missing target,
/// invalid accepted prefix, or incomplete native operation proposal.
pub fn prepare_intent_operation_output_plan(
    expected: IntentSessionIdentity,
    operations: &[geosolve_sketch_intent::IntentPatchOperation],
    symbol: &IntentKey,
    document: DocumentId,
    model_scale: f64,
) -> Result<PreparedIntentOperationPlan, PreparedIntentOperationPlanError> {
    let session = IntentSession::with_id(expected.session)?;
    if session.identity() != expected {
        return Err(PreparedIntentOperationPlanError::NonPristineSession);
    }
    let mut operations = operations.to_vec();
    let mut target_present = false;
    for operation in &mut operations {
        if let geosolve_sketch_intent::IntentPatchOperation::CreateNode { draft, .. } = operation
            && &draft.symbol == symbol
        {
            draft.suppressed = false;
            target_present = true;
        }
    }
    if !target_present {
        return Err(PreparedIntentOperationPlanError::NotEvaluated);
    }
    let materializer = ColdIntentMaterializer::with_default_policy(document, model_scale)?;
    let prepared = std::cell::RefCell::new(None);
    let probe = IntentPatch::new(
        expected,
        geosolve_sketch_intent::IntentPatchPolicy::RetainFailedIntent,
        operations,
    );
    let _ = session.plan_patch(probe, |candidate| {
        let target = candidate
            .graph()
            .nodes()
            .values()
            .find(|node| &node.symbol == symbol)
            .map(|node| node.id);
        let result = materializer.prepare_operation_output_plan(candidate, symbol);
        prepared.replace(Some(result));
        IntentEvaluation::Failed {
            failure: IntentEvaluationFailure {
                kind: IntentEvaluationFailureKind::MaterializationRejected,
                failed_nodes: target.into_iter().collect(),
                diagnostic: symbol.clone(),
            },
        }
    })?;
    prepared
        .into_inner()
        .ok_or(PreparedIntentOperationPlanError::NotEvaluated)?
        .map_err(Into::into)
}

trait IntentMaterializationSource {
    fn graph(&self) -> &IntentGraph;
    fn instance(&self) -> &IntentInstanceState;
    fn reservations(&self) -> &IntentReservationLedger;
    fn external_inputs(&self) -> &IntentExternalInputs;
    fn semantic_identity(&self) -> IntentSemanticIdentity;
}

impl IntentMaterializationSource for IntentCandidate {
    fn graph(&self) -> &IntentGraph {
        self.graph()
    }

    fn instance(&self) -> &IntentInstanceState {
        self.instance()
    }

    fn reservations(&self) -> &IntentReservationLedger {
        self.reservations()
    }

    fn external_inputs(&self) -> &IntentExternalInputs {
        self.external_inputs()
    }

    fn semantic_identity(&self) -> IntentSemanticIdentity {
        self.semantic_identity()
    }
}

impl IntentMaterializationSource for IntentSession {
    fn graph(&self) -> &IntentGraph {
        self.graph()
    }

    fn instance(&self) -> &IntentInstanceState {
        self.instance()
    }

    fn reservations(&self) -> &IntentReservationLedger {
        self.reservations()
    }

    fn external_inputs(&self) -> &IntentExternalInputs {
        self.external_inputs()
    }

    fn semantic_identity(&self) -> IntentSemanticIdentity {
        self.semantic_identity()
    }
}

impl IntentMaterializationSource for IntentAcceptedAuthority {
    fn graph(&self) -> &IntentGraph {
        &self.graph
    }

    fn instance(&self) -> &IntentInstanceState {
        &self.instance
    }

    fn reservations(&self) -> &IntentReservationLedger {
        &self.reservations
    }

    fn external_inputs(&self) -> &IntentExternalInputs {
        &self.external_inputs
    }

    fn semantic_identity(&self) -> IntentSemanticIdentity {
        self.target
    }
}

fn authority_semantic_identity(authority: &IntentAcceptedAuthority) -> IntentSemanticIdentity {
    IntentSemanticIdentity {
        graph: authority.graph.identity(),
        instance: authority.instance.identity(),
        reservations: authority.reservations.identity(),
        external_inputs: authority.external_inputs.identity(),
    }
}

/// Editor-owned cold materializer configuration.
#[derive(Clone, Debug)]
pub struct ColdIntentMaterializer {
    document: DocumentId,
    model_scale: f64,
    request: DocumentSolveRequest,
    config: SolverConfig,
    bootstrap: Option<ColdIntentBootstrapSeed>,
}

#[derive(Clone, Debug)]
struct ColdIntentBootstrapSeed {
    document: geosolve_sketch::SketchDocument,
    features: geosolve_sketch_features::ComputedFeatureDocument,
    feature_lifecycle_high_water: geosolve_sketch_features::ComputedFeatureLifecycleHighWater,
    declarations: BTreeMap<NodeId, IntentNode>,
    reservations: BTreeMap<ReservationId, IntentReservationRecord>,
    ownership: IntentMaterializationMap,
}

impl ColdIntentMaterializer {
    /// Creates a materializer with an explicit persistent native namespace and
    /// solve policy. Construction validates the document namespace and scale.
    ///
    /// # Errors
    ///
    /// Returns a document error for a zero/exhausted namespace or invalid scale.
    pub fn new(
        document: DocumentId,
        model_scale: f64,
        request: DocumentSolveRequest,
        config: SolverConfig,
    ) -> Result<Self, IntentMaterializationError> {
        SketchDocumentBuilder::empty(document, model_scale)?;
        Ok(Self {
            document,
            model_scale,
            request,
            config,
            bootstrap: None,
        })
    }

    /// Installs one strictly decoded historical native seed beneath later
    /// projectional declarations.
    ///
    /// The seed remains an implementation input, not a second scene authority:
    /// every later accepted publication is independently rebuilt and validated
    /// from the immutable per-object declarations plus current typed intent.
    pub(crate) fn with_authenticated_bootstrap(
        mut self,
        decoded: &crate::DecodedFlatIntentBootstrap,
        ownership: IntentMaterializationMap,
    ) -> Result<Self, IntentMaterializationError> {
        if decoded.document.id() != self.document
            || decoded.document.model_scale().to_bits() != self.model_scale.to_bits()
        {
            return Err(IntentMaterializationError::BootstrapSeedMismatch);
        }
        self.bootstrap = Some(ColdIntentBootstrapSeed {
            document: decoded.document.clone(),
            features: decoded.features.clone(),
            feature_lifecycle_high_water: decoded.feature_lifecycle_high_water,
            declarations: decoded.declarations.clone(),
            reservations: decoded.reservations.clone(),
            ownership,
        });
        Ok(self)
    }

    /// Creates a materializer with the native sketch defaults.
    ///
    /// # Errors
    ///
    /// Returns a document error for a zero/exhausted namespace or invalid scale.
    pub fn with_default_policy(
        document: DocumentId,
        model_scale: f64,
    ) -> Result<Self, IntentMaterializationError> {
        Self::new(
            document,
            model_scale,
            DocumentSolveRequest::default(),
            SolverConfig::default(),
        )
    }

    /// Cold-reconstructs declarations in canonical dependency order and
    /// publishes only a currently accepted native session.
    ///
    /// # Errors
    ///
    /// Returns a typed graph, lowering, reservation, document, or native-solver
    /// rejection. Scratch state is dropped on every error.
    pub fn materialize(
        &self,
        candidate: &IntentCandidate,
    ) -> Result<ColdIntentMaterialization, IntentMaterializationError> {
        self.materialize_source(candidate)
    }

    /// Prepares the exact native output inventory for one provisional
    /// operation in a structurally validated candidate without publishing an
    /// intent plan, native document, or history entry.
    ///
    /// The target operation must still have an empty `operation_outputs`
    /// vector. Every dependency is reconstructed through the ordinary cold
    /// lowering path, and preceding operations are applied to scratch state.
    /// Dynamic native path owners are converted to typed source-input slots so
    /// persistent native IDs never become authored-code identity.
    ///
    /// # Errors
    ///
    /// Rejects a missing/non-operation target, an already claimed output
    /// shape, any invalid accepted prefix, or a native operation which cannot
    /// produce one complete proposal for that exact prefix.
    #[allow(
        clippy::too_many_lines,
        reason = "the non-publishing planner deliberately mirrors the accepted-prefix cold lowering transaction"
    )]
    pub fn prepare_operation_output_plan(
        &self,
        candidate: &IntentCandidate,
        symbol: &IntentKey,
    ) -> Result<PreparedIntentOperationPlan, IntentMaterializationError> {
        candidate.graph().validate()?;
        preflight_supported(candidate)?;
        let target = candidate
            .graph()
            .nodes()
            .values()
            .find(|node| &node.symbol == symbol)
            .ok_or_else(|| {
                IntentMaterializationError::HostInput(format!(
                    "provisional operation `{symbol}` is absent from its exact candidate prefix"
                ))
            })?;
        let IntentNodeKind::Operation { operation } = target.kind else {
            return Err(IntentMaterializationError::UnsupportedNode { node: target.id });
        };
        if !target.operation_outputs.is_empty() {
            return Err(invalid_operation(
                target,
                "provisional operation already claims a native output shape",
            ));
        }

        let host_inputs = decode_intent_external_inputs(candidate.external_inputs())
            .map_err(|error| IntentMaterializationError::HostInput(error.to_string()))?;
        let (document, bootstrap_ownership, bootstrap_nodes) = if let Some(seed) = &self.bootstrap {
            validate_bootstrap_seed(candidate, seed)?;
            (
                seed.document.clone(),
                Some(seed.ownership.clone()),
                seed.declarations.keys().copied().collect::<BTreeSet<_>>(),
            )
        } else {
            (
                SketchDocumentBuilder::empty(self.document, self.model_scale)?,
                None,
                BTreeSet::new(),
            )
        };
        let allocated = allocate_reservation_ledger(
            document.persistent_identity_high_water().clone(),
            candidate.reservations().entries(),
            candidate.graph(),
        )?;
        let initial_parameters = self
            .bootstrap
            .as_ref()
            .map_or_else(ParameterBatch::default, |_| host_inputs.parameters.clone());
        let initial_snapshots = self
            .bootstrap
            .as_ref()
            .map_or_else(ExternalSnapshotSet::default, |_| {
                host_inputs.external_snapshots.clone()
            });
        let mut session = RetainedSketchDocumentSession::new_with_inputs(
            document,
            initial_parameters,
            initial_snapshots,
            self.request,
            self.config,
        )?;
        require_current_acceptance(&session)?;
        for reservations in &allocated.stages {
            apply_materialization_stage(
                &mut session,
                SketchMaterializationBatch::retaining_unused_reservations(reservations.clone()),
            )?;
        }
        let mut state = LoweringState::new(
            candidate.semantic_identity(),
            allocated.bindings,
            bootstrap_ownership,
            session.design_document(),
        );
        let mut owner_reservations = allocated.live;
        for node_id in candidate.graph().canonical_schedule()? {
            let node = candidate
                .graph()
                .node(node_id)
                .ok_or(IntentMaterializationError::UnknownNode(node_id))?;
            state.bind_schema_ports(node)?;
            let owner_reservation = owner_reservations.remove(&node.id);
            if bootstrap_nodes.contains(&node.id) {
                if node.id == target.id {
                    return Err(invalid_operation(
                        node,
                        "a bootstrap declaration cannot be a provisional operation",
                    ));
                }
                continue;
            }
            if node.id == target.id {
                if node.suppressed {
                    return Err(invalid_operation(
                        node,
                        "a suppressed operation has no native output plan",
                    ));
                }
                try_publish_host_inputs(
                    &mut session,
                    &host_inputs.parameters,
                    &host_inputs.external_snapshots,
                    self.request,
                )?;
                let proposal = prepare_operation_proposal(node, operation, &session, &state)?;
                return prepared_intent_operation_plan(
                    candidate.graph(),
                    node,
                    operation,
                    &state,
                    proposal.output_plan(),
                );
            }
            if node.suppressed {
                try_publish_host_inputs(
                    &mut session,
                    &host_inputs.parameters,
                    &host_inputs.external_snapshots,
                    self.request,
                )?;
                continue;
            }
            if let IntentNodeKind::Operation { operation } = node.kind {
                try_publish_host_inputs(
                    &mut session,
                    &host_inputs.parameters,
                    &host_inputs.external_snapshots,
                    self.request,
                )?;
                lower_operation(candidate, node, operation, &mut session, &mut state)?;
                continue;
            }
            let reservations = owner_reservation.unwrap_or(
                SketchMaterializationReservationAllocator::new(
                    session
                        .design_document()
                        .persistent_identity_high_water()
                        .clone(),
                )?
                .finish()?,
            );
            let mut batch = SketchMaterializationBatch::new(reservations);
            if lower_declarative_node(
                candidate,
                node,
                session.external_snapshot_set(),
                &mut batch,
                &mut state,
            )? {
                apply_reserved_node_materialization(&mut session, batch)?;
            }
            try_publish_host_inputs(
                &mut session,
                &host_inputs.parameters,
                &host_inputs.external_snapshots,
                self.request,
            )?;
        }
        Err(IntentMaterializationError::UnknownNode(target.id))
    }

    /// Independently materializes one pristine empty semantic session so its
    /// native evidence can be installed through
    /// [`IntentSession::install_pristine_empty_acceptance`].
    ///
    /// This does not admit caller-issued no-op patches. It is only the
    /// history-free initialization seam for an exact revision-zero session.
    ///
    /// # Errors
    ///
    /// Rejects any retained graph, instance, reservation, attempt, acceptance,
    /// history, allocation, organization declaration, or external input.
    pub fn materialize_pristine_empty(
        &self,
        session: &IntentSession,
    ) -> Result<ColdIntentMaterialization, IntentMaterializationError> {
        let pristine = session.identity().revision.raw() == 0
            && session.graph().nodes().is_empty()
            && session.instance().values().is_empty()
            && session.reservations().entries().is_empty()
            && session.organization().node_names().is_empty()
            && session.external_inputs() == &IntentExternalInputs::default()
            && session.latest_attempt().is_none()
            && session.accepted().is_none()
            && session.undo_len() == 0
            && session.redo_len() == 0
            && session.allocator_high_water() == IntentAllocatorHighWater::initial();
        if !pristine {
            return Err(IntentMaterializationError::AcceptedAuthorityIdentityMismatch);
        }
        self.materialize_source(session)
    }

    /// Cold-reconstructs one persisted accepted authority without fabricating
    /// a patch candidate. The complete semantic identity and every stored host
    /// artifact are authenticated against the independently rebuilt output.
    ///
    /// # Errors
    ///
    /// Returns a typed identity, evidence, graph, lowering, document, or solver
    /// rejection. No native state is published on failure.
    pub fn materialize_accepted_authority(
        &self,
        authority: &IntentAcceptedAuthority,
    ) -> Result<ColdIntentMaterialization, IntentMaterializationError> {
        let identity = authority_semantic_identity(authority);
        if identity != authority.target {
            return Err(IntentMaterializationError::AcceptedAuthorityIdentityMismatch);
        }
        let authenticated = MaterializationEvidence::new_host_artifacts(
            authority.evidence.external_inputs,
            authority.evidence.materialization.clone(),
            authority.evidence.ownership.clone(),
            authority.evidence.host_validation.clone(),
        )?;
        if authenticated != authority.evidence
            || authority.evidence.external_inputs != authority.external_inputs.identity()
        {
            return Err(IntentMaterializationError::AcceptedAuthorityEvidenceMismatch);
        }
        let continuation_json = std::str::from_utf8(&authority.evidence.materialization)
            .map_err(|_| IntentMaterializationError::AcceptedAuthorityEvidenceMismatch)?;
        let continuation = geosolve_sketch::SketchDocument::from_draft_v5_json(continuation_json)?;
        let output = self.materialize_source_with_accepted_continuation_and_work(
            authority,
            Some(&continuation),
            None,
            &mut crate::InteractionWorkReceipt::default(),
        )?;
        if output.evidence != authority.evidence {
            return Err(IntentMaterializationError::AcceptedAuthorityEvidenceMismatch);
        }
        Ok(output)
    }

    #[allow(
        clippy::too_many_lines,
        reason = "one auditable cold transaction retains validation and evidence publication in one scope"
    )]
    fn materialize_source(
        &self,
        candidate: &dyn IntentMaterializationSource,
    ) -> Result<ColdIntentMaterialization, IntentMaterializationError> {
        self.materialize_source_with_work(candidate, &mut crate::InteractionWorkReceipt::default())
    }

    #[allow(
        clippy::too_many_lines,
        reason = "one auditable cold transaction retains validation, exact work evidence and evidence publication in one scope"
    )]
    fn materialize_source_with_work(
        &self,
        candidate: &dyn IntentMaterializationSource,
        work: &mut crate::InteractionWorkReceipt,
    ) -> Result<ColdIntentMaterialization, IntentMaterializationError> {
        self.materialize_source_with_accepted_continuation_and_work(candidate, None, None, work)
    }

    #[allow(
        clippy::too_many_lines,
        reason = "one auditable cold transaction retains validation, exact continuation certification, work evidence and evidence publication in one scope"
    )]
    fn materialize_source_with_accepted_continuation_and_work(
        &self,
        candidate: &dyn IntentMaterializationSource,
        accepted_continuation: Option<&geosolve_sketch::SketchDocument>,
        structural_continuation: Option<(
            &geosolve_sketch::SketchDocument,
            &geosolve_sketch::SketchDocument,
        )>,
        work: &mut crate::InteractionWorkReceipt,
    ) -> Result<ColdIntentMaterialization, IntentMaterializationError> {
        candidate.graph().validate()?;
        preflight_supported(candidate)?;
        if self.bootstrap.is_none()
            && candidate.graph().nodes().values().any(|node| {
                matches!(node.kind, IntentNodeKind::Bootstrap { .. })
                    || node.bootstrap_origin.is_some()
            })
        {
            return Err(IntentMaterializationError::BootstrapSeedMismatch);
        }
        let host_inputs = decode_intent_external_inputs(candidate.external_inputs())
            .map_err(|error| IntentMaterializationError::HostInput(error.to_string()))?;

        let (mut document, bootstrap_ownership, bootstrap_nodes) =
            if let Some(seed) = &self.bootstrap {
                validate_bootstrap_seed(candidate, seed)?;
                (
                    seed.document.clone(),
                    Some(seed.ownership.clone()),
                    seed.declarations.keys().copied().collect::<BTreeSet<_>>(),
                )
            } else {
                (
                    SketchDocumentBuilder::empty(self.document, self.model_scale)?,
                    None,
                    BTreeSet::new(),
                )
            };
        if let Some(seed) = &self.bootstrap {
            apply_bootstrap_instance(candidate, seed, &mut document)?;
        }
        let desired_parameters = host_inputs.parameters;
        let desired_snapshots = host_inputs.external_snapshots;
        let materialized_reservations = candidate
            .reservations()
            .entries()
            .iter()
            .filter(|(_, record)| !bootstrap_nodes.contains(&record.owner_node))
            .map(|(id, record)| (*id, *record))
            .collect::<BTreeMap<_, _>>();
        let allocated = allocate_reservation_ledger(
            document.persistent_identity_high_water().clone(),
            &materialized_reservations,
            candidate.graph(),
        )?;
        let schedule = candidate.graph().canonical_schedule()?;
        let contains_operation = schedule.iter().any(|node| {
            candidate.graph().node(*node).is_some_and(|node| {
                !node.suppressed
                    && !bootstrap_nodes.contains(&node.id)
                    && matches!(node.kind, IntentNodeKind::Operation { .. })
            })
        });
        let (mut session, state) = if contains_operation {
            self.lower_operation_graph(
                candidate,
                document,
                bootstrap_ownership,
                &bootstrap_nodes,
                &schedule,
                allocated,
                &desired_parameters,
                &desired_snapshots,
                structural_continuation,
            )?
        } else {
            self.lower_declarative_graph(
                candidate,
                document,
                bootstrap_ownership,
                &bootstrap_nodes,
                &schedule,
                allocated,
                &desired_parameters,
                &desired_snapshots,
                structural_continuation,
            )?
        };
        if let Some(accepted_continuation) = accepted_continuation {
            work.record_native_preview_attempt();
            let expected = session.prepared_input();
            // Intent history deliberately retains every allocator cursor ever
            // observed, even when Undo restores an older accepted topology.
            // The authenticated historical materialization therefore carries
            // the same durable objects but may predate the retained cursor.
            // Advance only that host-owned high-water metadata before asking
            // the document seam to project continuous values; every object,
            // topology row, branch and source field must still match exactly.
            let mut accepted_continuation = accepted_continuation.clone();
            accepted_continuation.retain_persistent_identity_high_water(
                &session.design_document().persistent_identity_high_water(),
            )?;
            let accepted = session
                .design_document()
                .project_accepted_numerical_continuation(&accepted_continuation)?;
            session.replace_current_accepted_materialization(expected, accepted)?;
        }
        state.validate_declared_consumption(candidate)?;
        let accepted = session
            .accepted_state_for_current_input()
            .ok_or(IntentMaterializationError::SolverRejected)?;
        let diagnostics = accepted.diagnostics();
        let solve = diagnostics
            .solve
            .ok_or(IntentMaterializationError::MissingValidationEvidence)?;
        let maximum = solve.maximum_normalized_hard_residual;
        if !solve.accepted
            || solve.hard_validity != SketchHardValidity::Valid
            || !solve.hard_residuals_validated
            || maximum.is_some_and(|value| {
                !value.is_finite() || value > SKETCH_ACCEPTANCE_RESIDUAL_TOLERANCE
            })
        {
            return Err(IntentMaterializationError::IndependentValidationFailed);
        }

        state.validate_aggregates(&session)?;

        let mut ownership = state.finish(candidate.reservations().entries());
        work.record_computed_evaluation_attempt();
        let computed = crate::intent_computed::materialize_computed_features(
            candidate.graph(),
            accepted.document().id(),
            &session,
            &mut ownership,
            self.bootstrap
                .as_ref()
                .map(|seed| (&seed.features, seed.feature_lifecycle_high_water)),
        )?;
        ownership.validate_against(
            candidate.semantic_identity(),
            candidate.graph(),
            candidate.instance(),
            candidate.reservations(),
            accepted.document(),
            &computed.features,
        )?;
        let validation = IntentValidationEvidence {
            semantic: candidate.semantic_identity(),
            document: accepted.document().id(),
            point_count: accepted.document().points().len(),
            curve_count: accepted.document().curves().len(),
            constraint_count: accepted.document().constraints().len(),
            hard_residuals_validated: solve.hard_residuals_validated,
            maximum_normalized_hard_residual: maximum,
            feature_document: computed.features.id(),
            feature_revision: computed.features.revision(),
            feature_digest: computed.features.digest(),
            feature_count: computed.features.features().len(),
            computed_edge_count: computed.snapshot.edges().len(),
            all_active_features_current: computed.snapshot.feature_evaluations().iter().all(
                |feature| {
                    matches!(
                        feature.state,
                        geosolve_sketch_features::ComputedFeatureEvaluationState::Current { .. }
                            | geosolve_sketch_features::ComputedFeatureEvaluationState::Suppressed
                    )
                },
            ),
        };
        let materialization = accepted.document().to_draft_v5_json()?.into_bytes();
        let ownership_bytes = serde_json::to_vec(&ownership)?;
        let validation_bytes = serde_json::to_vec(&validation)?;
        let evidence = MaterializationEvidence::new_host_artifacts(
            candidate.external_inputs().identity(),
            materialization,
            ownership_bytes,
            validation_bytes,
        )?;
        Ok(ColdIntentMaterialization {
            session,
            features: computed.features,
            feature_lifecycle_high_water: computed.feature_lifecycle_high_water,
            computed: computed.snapshot,
            computed_evaluation_high_water: computed.evaluation_high_water,
            ownership,
            validation,
            evidence,
        })
    }

    #[allow(
        clippy::too_many_arguments,
        clippy::too_many_lines,
        reason = "the operation path keeps every accepted native prefix and host-input transition explicit"
    )]
    fn lower_operation_graph(
        &self,
        candidate: &dyn IntentMaterializationSource,
        document: geosolve_sketch::SketchDocument,
        bootstrap_ownership: Option<IntentMaterializationMap>,
        bootstrap_nodes: &BTreeSet<NodeId>,
        schedule: &[NodeId],
        allocated: AllocatedReservationStage,
        desired_parameters: &ParameterBatch,
        desired_snapshots: &ExternalSnapshotSet,
        structural_continuation: Option<(
            &geosolve_sketch::SketchDocument,
            &geosolve_sketch::SketchDocument,
        )>,
    ) -> Result<(RetainedSketchDocumentSession, LoweringState), IntentMaterializationError> {
        let initial_parameters = self
            .bootstrap
            .as_ref()
            .map_or_else(ParameterBatch::default, |_| desired_parameters.clone());
        let initial_snapshots = self
            .bootstrap
            .as_ref()
            .map_or_else(ExternalSnapshotSet::default, |_| desired_snapshots.clone());
        let mut session =
            if let Some((upstream_design, upstream_accepted)) = structural_continuation {
                RetainedSketchDocumentSession::new_with_inputs_from_numerical_continuation(
                    document,
                    upstream_design,
                    upstream_accepted,
                    initial_parameters,
                    initial_snapshots,
                    self.request,
                    self.config,
                )?
            } else {
                RetainedSketchDocumentSession::new_with_inputs(
                    document,
                    initial_parameters,
                    initial_snapshots,
                    self.request,
                    self.config,
                )?
            };
        require_current_acceptance(&session)?;
        for reservations in &allocated.stages {
            apply_materialization_stage(
                &mut session,
                SketchMaterializationBatch::retaining_unused_reservations(reservations.clone()),
            )?;
        }
        let mut state = LoweringState::new(
            candidate.semantic_identity(),
            allocated.bindings,
            bootstrap_ownership,
            session.design_document(),
        );
        let mut owner_reservations = allocated.live;

        for node_id in schedule {
            let node = candidate
                .graph()
                .node(*node_id)
                .ok_or(IntentMaterializationError::UnknownNode(*node_id))?;
            state.bind_schema_ports(node)?;
            let owner_reservation = owner_reservations.remove(&node.id);
            if bootstrap_nodes.contains(&node.id) {
                continue;
            }
            if node.suppressed {
                try_publish_host_inputs(
                    &mut session,
                    desired_parameters,
                    desired_snapshots,
                    self.request,
                )?;
                continue;
            }
            if let IntentNodeKind::Operation { operation } = node.kind {
                try_publish_host_inputs(
                    &mut session,
                    desired_parameters,
                    desired_snapshots,
                    self.request,
                )?;
                lower_operation(candidate, node, operation, &mut session, &mut state)?;
                continue;
            }
            let reservations = owner_reservation.unwrap_or(
                SketchMaterializationReservationAllocator::new(
                    session
                        .design_document()
                        .persistent_identity_high_water()
                        .clone(),
                )?
                .finish()?,
            );
            let mut batch = SketchMaterializationBatch::new(reservations);
            let apply_batch = lower_declarative_node(
                candidate,
                node,
                session.external_snapshot_set(),
                &mut batch,
                &mut state,
            )?;
            if apply_batch {
                apply_reserved_node_materialization(&mut session, batch)?;
            }
            try_publish_host_inputs(
                &mut session,
                desired_parameters,
                desired_snapshots,
                self.request,
            )?;
        }
        if !owner_reservations.is_empty() {
            return Err(IntentMaterializationError::InvalidReservationOrder);
        }
        if let Some((upstream_design, upstream_accepted)) = structural_continuation {
            session = RetainedSketchDocumentSession::new_with_inputs_from_numerical_continuation(
                session.design_document().clone(),
                upstream_design,
                upstream_accepted,
                desired_parameters.clone(),
                desired_snapshots.clone(),
                self.request,
                self.config,
            )?;
        }
        require_exact_host_inputs(&session, desired_parameters, desired_snapshots)?;
        Ok((session, state))
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "cold structural lowering carries the complete authenticated inputs into one native solve"
    )]
    fn lower_declarative_graph(
        &self,
        candidate: &dyn IntentMaterializationSource,
        mut document: geosolve_sketch::SketchDocument,
        bootstrap_ownership: Option<IntentMaterializationMap>,
        bootstrap_nodes: &BTreeSet<NodeId>,
        schedule: &[NodeId],
        allocated: AllocatedReservationStage,
        desired_parameters: &ParameterBatch,
        desired_snapshots: &ExternalSnapshotSet,
        structural_continuation: Option<(
            &geosolve_sketch::SketchDocument,
            &geosolve_sketch::SketchDocument,
        )>,
    ) -> Result<(RetainedSketchDocumentSession, LoweringState), IntentMaterializationError> {
        for reservations in &allocated.stages {
            document.apply_materialization_batch(
                &SketchMaterializationBatch::retaining_unused_reservations(reservations.clone()),
            )?;
        }
        let mut state = LoweringState::new(
            candidate.semantic_identity(),
            allocated.bindings,
            bootstrap_ownership,
            &document,
        );
        let mut owner_reservations = allocated.live;

        for node_id in schedule {
            let node = candidate
                .graph()
                .node(*node_id)
                .ok_or(IntentMaterializationError::UnknownNode(*node_id))?;
            state.bind_schema_ports(node)?;
            let owner_reservation = owner_reservations.remove(&node.id);
            if bootstrap_nodes.contains(&node.id) || node.suppressed {
                continue;
            }
            if matches!(node.kind, IntentNodeKind::Operation { .. }) {
                return Err(IntentMaterializationError::InvalidReservationOrder);
            }
            let reservations = owner_reservation.unwrap_or(
                SketchMaterializationReservationAllocator::new(
                    document.persistent_identity_high_water().clone(),
                )?
                .finish()?,
            );
            let mut batch = SketchMaterializationBatch::new(reservations);
            if lower_declarative_node(candidate, node, desired_snapshots, &mut batch, &mut state)? {
                document.apply_retired_materialization_batch(&batch)?;
            }
        }
        if !owner_reservations.is_empty() {
            return Err(IntentMaterializationError::InvalidReservationOrder);
        }
        let session = if let Some((upstream_design, upstream_accepted)) = structural_continuation {
            RetainedSketchDocumentSession::new_with_inputs_from_numerical_continuation(
                document,
                upstream_design,
                upstream_accepted,
                desired_parameters.clone(),
                desired_snapshots.clone(),
                self.request,
                self.config,
            )?
        } else {
            RetainedSketchDocumentSession::new_with_inputs(
                document,
                desired_parameters.clone(),
                desired_snapshots.clone(),
                self.request,
                self.config,
            )?
        };
        require_exact_host_inputs(&session, desired_parameters, desired_snapshots)?;
        Ok((session, state))
    }

    /// Adapter for [`geosolve_sketch_intent::IntentSession::plan_patch`].
    /// Native artifacts are still created and validated here rather than trusted
    /// from persistence.
    ///
    /// # Panics
    ///
    /// Panics only if a static, source-controlled diagnostic identifier stops
    /// satisfying [`IntentKey`]'s bounded key contract.
    #[must_use]
    pub fn evaluate(&self, candidate: &IntentCandidate) -> IntentEvaluation {
        self.evaluate_with_materialization(candidate).0
    }

    /// Evaluates one candidate once while retaining the independently validated
    /// native result for an atomic coordinator publication.
    ///
    /// Rejected candidates return no native state. Keeping this seam crate-local
    /// prevents the coordinator from repeating an expensive cold solve merely
    /// to translate the materializer-owned typed failure.
    pub(crate) fn evaluate_with_materialization(
        &self,
        candidate: &IntentCandidate,
    ) -> (IntentEvaluation, Option<ColdIntentMaterialization>) {
        let audited = self.evaluate_with_materialization_audited(candidate);
        (audited.0, audited.1)
    }

    pub(crate) fn evaluate_with_materialization_audited(
        &self,
        candidate: &IntentCandidate,
    ) -> (
        IntentEvaluation,
        Option<ColdIntentMaterialization>,
        crate::InteractionWorkReceipt,
    ) {
        self.evaluate_with_materialization_and_continuation_audited(candidate, None, None)
    }

    /// Evaluates one delegated semantic candidate while certifying a complete
    /// independently accepted numerical continuation under the candidate's
    /// newly retained source design. The continuation contributes no source
    /// declarations or temporary drag request; it can only replace the
    /// candidate's accepted coordinates after exact native certification.
    pub(crate) fn evaluate_with_materialization_from_accepted_continuation_audited(
        &self,
        candidate: &IntentCandidate,
        accepted_continuation: &geosolve_sketch::SketchDocument,
    ) -> (
        IntentEvaluation,
        Option<ColdIntentMaterialization>,
        crate::InteractionWorkReceipt,
    ) {
        self.evaluate_with_materialization_and_continuation_audited(
            candidate,
            Some(accepted_continuation),
            None,
        )
    }

    /// Evaluates a structurally changed candidate while carrying forward only
    /// matching numerical values from the previous retained/accepted pair.
    /// The candidate remains the sole source of topology, branches, ranges,
    /// and host inputs.
    pub(crate) fn evaluate_with_materialization_from_structural_continuation_audited(
        &self,
        candidate: &IntentCandidate,
        upstream_design: &geosolve_sketch::SketchDocument,
        upstream_accepted: &geosolve_sketch::SketchDocument,
    ) -> (
        IntentEvaluation,
        Option<ColdIntentMaterialization>,
        crate::InteractionWorkReceipt,
    ) {
        self.evaluate_with_materialization_and_continuation_audited(
            candidate,
            None,
            Some((upstream_design, upstream_accepted)),
        )
    }

    fn evaluate_with_materialization_and_continuation_audited(
        &self,
        candidate: &IntentCandidate,
        accepted_continuation: Option<&geosolve_sketch::SketchDocument>,
        structural_continuation: Option<(
            &geosolve_sketch::SketchDocument,
            &geosolve_sketch::SketchDocument,
        )>,
    ) -> (
        IntentEvaluation,
        Option<ColdIntentMaterialization>,
        crate::InteractionWorkReceipt,
    ) {
        let mut work = crate::InteractionWorkReceipt::default();
        work.record_intent_materialization_attempt();
        match self.materialize_source_with_accepted_continuation_and_work(
            candidate,
            accepted_continuation,
            structural_continuation,
            &mut work,
        ) {
            Ok(materialized) => {
                let evaluation = IntentEvaluation::Accepted {
                    evidence: materialized.evidence.clone(),
                };
                (evaluation, Some(materialized), work)
            }
            Err(error) => {
                let mut failed_nodes = error.failed_node().into_iter().collect::<BTreeSet<_>>();
                if failed_nodes.is_empty() {
                    failed_nodes.extend(
                        candidate
                            .diff()
                            .affected_nodes()
                            .into_iter()
                            .filter(|node| candidate.graph().node(*node).is_some()),
                    );
                }
                if failed_nodes.is_empty() && !candidate.graph().nodes().is_empty() {
                    // A global native solve/validation or host-input rejection
                    // has no lowerer's local node. Attribute it to the bounded
                    // live declaration set so retained diagnostics remain
                    // structurally valid and never invent a non-graph owner.
                    failed_nodes.extend(candidate.graph().nodes().keys().copied());
                }
                let evaluation = IntentEvaluation::Failed {
                    failure: IntentEvaluationFailure {
                        kind: error.failure_kind(),
                        failed_nodes,
                        diagnostic: IntentKey::new(error.diagnostic_key())
                            .expect("static intent materialization diagnostics are valid keys"),
                    },
                };
                (evaluation, None, work)
            }
        }
    }
}

/// Focused cold-materialization failure.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum IntentMaterializationError {
    #[error(transparent)]
    Graph(#[from] IntentGraphError),
    #[error(transparent)]
    Document(#[from] DocumentError),
    #[error(transparent)]
    Session(#[from] DocumentSessionError),
    #[error(transparent)]
    Evidence(#[from] geosolve_sketch_intent::IntentModelError),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error("computed-feature materialization rejected: {message}")]
    Computed {
        node: Option<NodeId>,
        message: String,
    },
    #[error("invalid intent host inputs: {0}")]
    HostInput(String),
    #[error("intent node {node} is not materializable by this editor adapter")]
    UnsupportedNode { node: NodeId },
    #[error("unknown intent node {0}")]
    UnknownNode(NodeId),
    #[error("intent node {node} is missing port {selector:?}")]
    MissingPort {
        node: NodeId,
        selector: IntentPortSelector,
    },
    #[error("intent node {node} is missing input {slot}")]
    MissingInput { node: NodeId, slot: InputSlot },
    #[error("intent node {node} input {slot} has no native binding")]
    UnboundInput { node: NodeId, slot: InputSlot },
    #[error("intent node {node} has an incompatible native binding")]
    NativeKindMismatch { node: NodeId },
    #[error("intent node {node} aliases point {point} before its position is available")]
    MissingPointPosition { node: NodeId, point: DesignPointId },
    #[error("reservation {reservation} has invalid paired reservation metadata")]
    InvalidReservationPair { reservation: ReservationId },
    #[error("reservation {reservation} has no translated native identity")]
    MissingReservation { reservation: ReservationId },
    #[error("intent reservation owner order does not match the canonical dependency schedule")]
    InvalidReservationOrder,
    #[error("declared reservation {reservation} was not materialized")]
    UnconsumedReservation { reservation: ReservationId },
    #[error("writable leaf {leaf} must be a finite length quantity")]
    InvalidWritableLeaf { leaf: LeafRef },
    #[error("line node {node} has an invalid explicit branch direction")]
    InvalidBranchDirection { node: NodeId },
    #[error("geometry node {node} has invalid or incomplete recipe state: {reason}")]
    InvalidGeometry { node: NodeId, reason: &'static str },
    #[error("operation node {node} rejected deterministic lowering: {reason}")]
    InvalidOperation { node: NodeId, reason: &'static str },
    #[error("aggregate node {node} has invalid topology: {reason}")]
    InvalidAggregate { node: NodeId, reason: &'static str },
    #[error("native solver rejected the cold materialization")]
    SolverRejected,
    #[error("native solver omitted stable acceptance evidence")]
    MissingValidationEvidence,
    #[error("native independent hard-residual validation did not pass")]
    IndependentValidationFailed,
    #[error("persisted accepted intent authority has a stale or malformed semantic identity")]
    AcceptedAuthorityIdentityMismatch,
    #[error("persisted accepted intent authority does not match cold reconstructed evidence")]
    AcceptedAuthorityEvidenceMismatch,
    #[error("the immutable historical bootstrap seed does not match its typed declarations")]
    BootstrapSeedMismatch,
    #[error("intent ownership evidence identifies a different semantic authority")]
    OwnershipSemanticMismatch,
    #[error("intent ownership evidence is not in strict stable-key order")]
    InvalidOwnershipOrdering,
    #[error("stable output {port:?} is absent from ownership evidence")]
    UnknownOwnershipPort { port: IntentPortRef },
    #[error("required stable output {port:?} is missing from ownership evidence")]
    MissingOwnershipPort { port: IntentPortRef },
    #[error("retired stable output {port:?} unexpectedly has ownership evidence")]
    UnexpectedOwnershipPort { port: IntentPortRef },
    #[error("stable output {port:?} has an incompatible native binding")]
    OwnershipPortKindMismatch { port: IntentPortRef },
    #[error("stable output {port:?} does not match its exact identity flow")]
    OwnershipPortBindingMismatch { port: IntentPortRef },
    #[error("reservation {reservation} is absent from ownership evidence")]
    MissingOwnershipReservation { reservation: ReservationId },
    #[error("ownership evidence names unknown reservation {reservation}")]
    UnknownOwnershipReservation { reservation: ReservationId },
    #[error("reservation {reservation} has an incompatible native binding")]
    OwnershipReservationKindMismatch { reservation: ReservationId },
    #[error("reservation {reservation} does not match its exact declaration owner and port")]
    OwnershipReservationOwnerMismatch { reservation: ReservationId },
    #[error("declaration {node} does not own exactly its reserved native objects")]
    OwnershipNodeMismatch { node: NodeId },
    #[error("native binding {binding:?} is assigned to multiple declaration owners")]
    DuplicateNativeOwner { binding: IntentNativeBinding },
    #[error("native binding {binding:?} is assigned to multiple reservations")]
    DuplicateReservationBinding { binding: IntentNativeBinding },
    #[error("native binding {binding:?} named by ownership evidence does not exist")]
    MissingNativeOwnershipBinding { binding: IntentNativeBinding },
    #[error("writable intent leaf {leaf} has multiple native owners")]
    DuplicateWritableLeaf { leaf: LeafRef },
    #[error("writable intent leaf {leaf} is missing its exact native owner")]
    MissingWritableOwnership { leaf: LeafRef },
    #[error("writable intent leaf {leaf} is absent from stable output evidence")]
    UnknownOwnershipLeaf { leaf: LeafRef },
    #[error("writable intent leaf {leaf} has an incompatible native owner")]
    InvalidWritableOwnership { leaf: LeafRef },
    #[error("writable intent leaf {leaf} has an incompatible unit")]
    WritableOwnershipUnitMismatch { leaf: LeafRef },
    #[error("aggregate output {port:?} has invalid native spans")]
    InvalidOwnershipAggregate { port: IntentPortRef },
    #[error("curve span {span:?} is outside its exact native curve topology")]
    InvalidOwnershipCurveSpan { span: CurveSpan },
    #[error("native writable target {native:?} already has a reverse owner")]
    DuplicateWritableOwner { native: IntentNativeWritableLeaf },
}

impl From<crate::intent_computed::ComputedIntentMaterializationError>
    for IntentMaterializationError
{
    fn from(error: crate::intent_computed::ComputedIntentMaterializationError) -> Self {
        Self::Computed {
            node: error.failed_node(),
            message: error.to_string(),
        }
    }
}

impl IntentMaterializationError {
    const fn failed_node(&self) -> Option<NodeId> {
        match self {
            Self::UnsupportedNode { node }
            | Self::MissingPort { node, .. }
            | Self::MissingInput { node, .. }
            | Self::UnboundInput { node, .. }
            | Self::NativeKindMismatch { node }
            | Self::MissingPointPosition { node, .. }
            | Self::InvalidBranchDirection { node }
            | Self::InvalidGeometry { node, .. }
            | Self::InvalidOperation { node, .. }
            | Self::InvalidAggregate { node, .. }
            | Self::UnknownNode(node) => Some(*node),
            Self::Computed { node, .. } => *node,
            _ => None,
        }
    }

    const fn failure_kind(&self) -> IntentEvaluationFailureKind {
        match self {
            Self::SolverRejected | Self::IndependentValidationFailed => {
                IntentEvaluationFailureKind::SolverRejected
            }
            _ => IntentEvaluationFailureKind::MaterializationRejected,
        }
    }

    const fn diagnostic_key(&self) -> &'static str {
        match self {
            Self::UnsupportedNode { .. } => "unsupported-intent-node",
            Self::HostInput(_) => "invalid-intent-host-inputs",
            Self::SolverRejected => "native-solver-rejected",
            Self::IndependentValidationFailed | Self::MissingValidationEvidence => {
                "native-validation-rejected"
            }
            Self::AcceptedAuthorityIdentityMismatch => "accepted-authority-identity-mismatch",
            Self::AcceptedAuthorityEvidenceMismatch => "accepted-authority-evidence-mismatch",
            Self::BootstrapSeedMismatch => "bootstrap-seed-mismatch",
            Self::Graph(_) => "invalid-intent-graph",
            Self::Document(_) | Self::Session(_) => "native-materialization-rejected",
            Self::Evidence(_) | Self::Json(_) => "materialization-evidence-rejected",
            Self::Computed { .. } => "computed-feature-materialization-rejected",
            _ => "invalid-intent-lowering",
        }
    }
}

fn validate_bootstrap_seed(
    candidate: &dyn IntentMaterializationSource,
    seed: &ColdIntentBootstrapSeed,
) -> Result<(), IntentMaterializationError> {
    for (id, expected) in &seed.declarations {
        let actual = candidate
            .graph()
            .node(*id)
            .ok_or(IntentMaterializationError::BootstrapSeedMismatch)?;
        if !crate::intent_bootstrap::bootstrap_declaration_matches_candidate(expected, actual)
            .map_err(|_| IntentMaterializationError::BootstrapSeedMismatch)?
        {
            return Err(IntentMaterializationError::BootstrapSeedMismatch);
        }
    }
    if candidate.graph().nodes().values().any(|node| {
        (matches!(node.kind, IntentNodeKind::Bootstrap { .. }) || node.bootstrap_origin.is_some())
            && !seed.declarations.contains_key(&node.id)
    }) {
        return Err(IntentMaterializationError::BootstrapSeedMismatch);
    }
    let reservation_owners = seed.declarations.keys().copied().collect::<BTreeSet<_>>();
    let reservations = candidate
        .reservations()
        .entries()
        .iter()
        .filter(|(_, record)| reservation_owners.contains(&record.owner_node))
        .map(|(id, record)| (*id, *record))
        .collect::<BTreeMap<_, _>>();
    if reservations != seed.reservations {
        return Err(IntentMaterializationError::BootstrapSeedMismatch);
    }
    Ok(())
}

fn apply_bootstrap_instance(
    candidate: &dyn IntentMaterializationSource,
    seed: &ColdIntentBootstrapSeed,
    document: &mut geosolve_sketch::SketchDocument,
) -> Result<(), IntentMaterializationError> {
    for (native, leaf) in &seed.ownership.writable_leaves {
        let Some(value) = candidate.instance().values().get(leaf) else {
            return Err(IntentMaterializationError::BootstrapSeedMismatch);
        };
        let IntentLiteral::Quantity { value, unit } = value else {
            return Err(IntentMaterializationError::InvalidWritableLeaf { leaf: *leaf });
        };
        if !value.is_finite() {
            return Err(IntentMaterializationError::InvalidWritableLeaf { leaf: *leaf });
        }
        match native {
            IntentNativeWritableLeaf::PointX { point } => {
                if *unit != IntentUnit::Length {
                    return Err(IntentMaterializationError::InvalidWritableLeaf { leaf: *leaf });
                }
                let mut position = document
                    .point(*point)
                    .ok_or(IntentMaterializationError::BootstrapSeedMismatch)?
                    .position;
                position[0] = *value;
                document.set_point_position(*point, position)?;
            }
            IntentNativeWritableLeaf::PointY { point } => {
                if *unit != IntentUnit::Length {
                    return Err(IntentMaterializationError::InvalidWritableLeaf { leaf: *leaf });
                }
                let mut position = document
                    .point(*point)
                    .ok_or(IntentMaterializationError::BootstrapSeedMismatch)?
                    .position;
                position[1] = *value;
                document.set_point_position(*point, position)?;
            }
            IntentNativeWritableLeaf::ScalarValue { scalar } => {
                let native = document
                    .scalar(*scalar)
                    .ok_or(IntentMaterializationError::BootstrapSeedMismatch)?;
                let expected = scalar_leaf_intent_unit(leaf.field, native.unit)
                    .ok_or(IntentMaterializationError::InvalidWritableLeaf { leaf: *leaf })?;
                if *unit != expected {
                    return Err(IntentMaterializationError::InvalidWritableLeaf { leaf: *leaf });
                }
                document.set_scalar_value(*scalar, *value)?;
            }
        }
    }
    Ok(())
}

struct SketchDocumentBuilder;

impl SketchDocumentBuilder {
    fn empty(
        document: DocumentId,
        model_scale: f64,
    ) -> Result<geosolve_sketch::SketchDocument, DocumentError> {
        geosolve_sketch::SketchDocument::with_id(model_scale, document)
    }
}

fn preflight_supported(
    candidate: &dyn IntentMaterializationSource,
) -> Result<(), IntentMaterializationError> {
    for node in candidate.graph().nodes().values() {
        let supported = match node.kind {
            IntentNodeKind::Geometry { recipe } => matches!(
                recipe,
                GeometryRecipeKind::SketchPoint
                    | GeometryRecipeKind::Segment
                    | GeometryRecipeKind::CenterRadiusCircle
                    | GeometryRecipeKind::TwoPointDiameterCircle
                    | GeometryRecipeKind::ThreePointCircle
                    | GeometryRecipeKind::CenterArc
                    | GeometryRecipeKind::ThreePointArc
                    | GeometryRecipeKind::CenterAxesEllipse
                    | GeometryRecipeKind::AxisEndpointsEllipse
                    | GeometryRecipeKind::CenterAxesEllipticalArc
                    | GeometryRecipeKind::AxisEndpointsEllipticalArc
                    | GeometryRecipeKind::QuadraticBezier
                    | GeometryRecipeKind::CubicBezier
                    | GeometryRecipeKind::RationalQuadraticConic
                    | GeometryRecipeKind::Parabola
                    | GeometryRecipeKind::Hyperbola
                    | GeometryRecipeKind::Polyline
                    | GeometryRecipeKind::MidpointLine
                    | GeometryRecipeKind::TwoPointAlignedRectangle
                    | GeometryRecipeKind::ThreePointCornerRectangle
                    | GeometryRecipeKind::CenterRectangle
                    | GeometryRecipeKind::ThreePointCenterRectangle
                    | GeometryRecipeKind::TangentArc
                    | GeometryRecipeKind::OpenControlBSpline
                    | GeometryRecipeKind::PeriodicControlBSpline
                    | GeometryRecipeKind::OpenControlNurbs
                    | GeometryRecipeKind::PeriodicControlNurbs
            ),
            IntentNodeKind::Constraint { .. }
            | IntentNodeKind::ComputedFeature { .. }
            | IntentNodeKind::Aggregate { .. }
            | IntentNodeKind::Operation { .. }
            | IntentNodeKind::Parameter { .. }
            | IntentNodeKind::External { .. }
            | IntentNodeKind::Bootstrap { .. }
            | IntentNodeKind::Annotation
            | IntentNodeKind::Identity { .. }
            | IntentNodeKind::Dimension { .. } => true,
        };
        if !supported {
            return Err(IntentMaterializationError::UnsupportedNode { node: node.id });
        }
    }
    Ok(())
}

struct AllocatedReservationStage {
    stages: Vec<geosolve_sketch::SketchMaterializationReservationSet>,
    live: BTreeMap<NodeId, geosolve_sketch::SketchMaterializationReservationSet>,
    bindings: BTreeMap<ReservationId, IntentNativeBinding>,
}

#[allow(
    clippy::too_many_lines,
    reason = "one closed typed allocator table keeps durable ledger order and native binding parity auditable"
)]
fn allocate_reservation_ledger(
    mut high_water: SketchPersistentIdentityHighWater,
    records: &BTreeMap<ReservationId, IntentReservationRecord>,
    graph: &IntentGraph,
) -> Result<AllocatedReservationStage, IntentMaterializationError> {
    let mut stages = Vec::new();
    let mut live = BTreeMap::new();
    let mut bindings = BTreeMap::new();
    let mut semantic_catalogs = BTreeMap::new();
    let mut cursor = records.iter().peekable();
    while let Some((_, first)) = cursor.peek().copied() {
        let owner = first.owner_node;
        let mut owner_records = BTreeMap::new();
        while cursor
            .peek()
            .is_some_and(|(_, record)| record.owner_node == owner)
        {
            let (id, record) = cursor.next().expect("peeked owner record");
            owner_records.insert(*id, *record);
        }
        let mut allocator = SketchMaterializationReservationAllocator::new(high_water)?;
        let mut paired = BTreeSet::new();
        for (reservation, record) in &owner_records {
            if paired.contains(reservation) {
                continue;
            }
            let binding = match record.kind {
                IntentNativeReservationKind::Point => {
                    IntentNativeBinding::Point(allocator.reserve_point()?)
                }
                IntentNativeReservationKind::Scalar => {
                    IntentNativeBinding::Scalar(allocator.reserve_scalar()?)
                }
                IntentNativeReservationKind::Curve => {
                    IntentNativeBinding::Curve(allocator.reserve_curve()?)
                }
                IntentNativeReservationKind::Contact => {
                    IntentNativeBinding::Contact(allocator.reserve_contact()?)
                }
                IntentNativeReservationKind::Constraint => {
                    let companion = require_pair(
                        *reservation,
                        record,
                        &owner_records,
                        IntentNativeReservationKind::ConstraintSource,
                    )?;
                    let native = allocator.reserve_constraint()?;
                    bindings.insert(companion, IntentNativeBinding::Source(native.source));
                    paired.insert(companion);
                    IntentNativeBinding::Constraint(native.constraint)
                }
                IntentNativeReservationKind::ConstraintSource => {
                    return Err(IntentMaterializationError::InvalidReservationPair {
                        reservation: *reservation,
                    });
                }
                IntentNativeReservationKind::Dimension => {
                    let companion = require_pair(
                        *reservation,
                        record,
                        &owner_records,
                        IntentNativeReservationKind::DimensionSource,
                    )?;
                    let native = allocator.reserve_dimension()?;
                    bindings.insert(companion, IntentNativeBinding::Source(native.source));
                    paired.insert(companion);
                    IntentNativeBinding::Dimension(native.dimension)
                }
                IntentNativeReservationKind::Parameter => {
                    IntentNativeBinding::Parameter(allocator.reserve_parameter()?)
                }
                IntentNativeReservationKind::ExternalBinding => {
                    IntentNativeBinding::ExternalBinding(allocator.reserve_external_binding()?)
                }
                IntentNativeReservationKind::SemanticCatalog => {
                    let catalog = allocator.reserve_semantic_catalog()?;
                    semantic_catalogs.insert(record.owner_node, catalog);
                    IntentNativeBinding::Source(catalog)
                }
                IntentNativeReservationKind::SemanticSource => {
                    let catalog = semantic_catalogs
                        .get(&record.owner_node)
                        .copied()
                        .ok_or(IntentMaterializationError::InvalidReservationOrder)?;
                    IntentNativeBinding::Source(allocator.reserve_semantic_source(catalog)?)
                }
                IntentNativeReservationKind::DimensionSource => {
                    return Err(IntentMaterializationError::UnsupportedNode {
                        node: record.owner_node,
                    });
                }
            };
            bindings.insert(*reservation, binding);
        }
        if let Some(node) = graph.node(owner) {
            reserve_spline_span_cursor(&mut allocator, &bindings, node)?;
        }
        let stage = allocator.finish()?;
        high_water = stage.resulting_high_water().clone();
        stages.push(stage.clone());
        if first.state != IntentReservationState::Tombstoned {
            if owner_records
                .values()
                .any(|record| record.state == IntentReservationState::Tombstoned)
            {
                return Err(IntentMaterializationError::InvalidReservationOrder);
            }
            live.insert(owner, stage);
        }
    }
    Ok(AllocatedReservationStage {
        stages,
        live,
        bindings,
    })
}

fn reserve_spline_span_cursor(
    allocator: &mut SketchMaterializationReservationAllocator,
    bindings: &BTreeMap<ReservationId, IntentNativeBinding>,
    node: &IntentNode,
) -> Result<(), IntentMaterializationError> {
    let IntentNodeKind::Geometry { recipe } = node.kind else {
        return Ok(());
    };
    if node.suppressed
        || !matches!(
            recipe,
            GeometryRecipeKind::OpenControlBSpline
                | GeometryRecipeKind::PeriodicControlBSpline
                | GeometryRecipeKind::OpenControlNurbs
                | GeometryRecipeKind::PeriodicControlNurbs
        )
    {
        return Ok(());
    }
    let control_count = node.child_order.len();
    let degree = field_natural(node, "degree", 3)?;
    let degree =
        usize::try_from(degree).map_err(|_| IntentMaterializationError::InvalidGeometry {
            node: node.id,
            reason: "spline degree exceeds platform limits",
        })?;
    if degree == 0 || control_count <= degree {
        return Ok(());
    }
    let curve_port = require_port(node, IntentPortRole::Curve, 0)?;
    let IntentIdentityFlow::Created { reservation } = curve_port.flow else {
        return Err(IntentMaterializationError::NativeKindMismatch { node: node.id });
    };
    let Some(IntentNativeBinding::Curve(curve)) = bindings.get(&reservation).copied() else {
        return Err(IntentMaterializationError::MissingReservation { reservation });
    };
    let span_count = match recipe {
        GeometryRecipeKind::OpenControlBSpline | GeometryRecipeKind::OpenControlNurbs => {
            control_count - degree
        }
        GeometryRecipeKind::PeriodicControlBSpline | GeometryRecipeKind::PeriodicControlNurbs => {
            control_count
        }
        _ => unreachable!("guarded spline recipe"),
    };
    let next_span_id = u32::try_from(span_count)
        .ok()
        .and_then(|count| count.checked_add(1))
        .ok_or(IntentMaterializationError::InvalidGeometry {
            node: node.id,
            reason: "spline span identity high-water overflow",
        })?;
    allocator.reserve_spline_span_cursor(curve, next_span_id)?;
    Ok(())
}

fn require_pair(
    reservation: ReservationId,
    record: &IntentReservationRecord,
    records: &BTreeMap<ReservationId, IntentReservationRecord>,
    expected_kind: IntentNativeReservationKind,
) -> Result<ReservationId, IntentMaterializationError> {
    let companion = record
        .paired_with
        .ok_or(IntentMaterializationError::InvalidReservationPair { reservation })?;
    let valid = records.get(&companion).is_some_and(|candidate| {
        candidate.kind == expected_kind && candidate.paired_with == Some(reservation)
    }) && companion.raw() == reservation.raw().checked_add(1).unwrap_or_default();
    if !valid {
        return Err(IntentMaterializationError::InvalidReservationPair { reservation });
    }
    Ok(companion)
}

fn apply_materialization_stage(
    session: &mut RetainedSketchDocumentSession,
    batch: SketchMaterializationBatch,
) -> Result<(), IntentMaterializationError> {
    session.transact(session.design_identity(), move |document| {
        document.apply_materialization_batch(&batch)
    })?;
    Ok(())
}

fn apply_reserved_node_materialization(
    session: &mut RetainedSketchDocumentSession,
    staged: SketchMaterializationBatch,
) -> Result<(), IntentMaterializationError> {
    session.transact(session.design_identity(), move |document| {
        document.apply_retired_materialization_batch(&staged)
    })?;
    Ok(())
}

fn lower_declarative_node(
    candidate: &dyn IntentMaterializationSource,
    node: &IntentNode,
    snapshots: &ExternalSnapshotSet,
    batch: &mut SketchMaterializationBatch,
    state: &mut LoweringState,
) -> Result<bool, IntentMaterializationError> {
    match &node.kind {
        IntentNodeKind::Geometry { recipe } => {
            lower_geometry(candidate, node, *recipe, batch, state)?;
        }
        IntentNodeKind::Constraint { constraint } => {
            lower_constraint(candidate, node, *constraint, batch, state)?;
        }
        IntentNodeKind::Dimension { dimension } => {
            lower_dimension(candidate, node, *dimension, batch, state)?;
        }
        IntentNodeKind::Aggregate { aggregate } => {
            lower_aggregate(node, *aggregate, state)?;
            return Ok(false);
        }
        IntentNodeKind::Parameter { parameter } => {
            lower_parameter(node, *parameter, batch, state)?;
        }
        IntentNodeKind::External { external } => {
            lower_external(node, *external, snapshots, batch, state)?;
        }
        IntentNodeKind::ComputedFeature { .. }
        | IntentNodeKind::Annotation
        | IntentNodeKind::Identity { .. } => {
            lower_logical_declaration(node, state)?;
            return Ok(false);
        }
        IntentNodeKind::Operation { .. } | IntentNodeKind::Bootstrap { .. } => {
            unreachable!("operation and bootstrap declarations are handled by their owning path")
        }
    }
    Ok(true)
}

fn require_current_acceptance(
    session: &RetainedSketchDocumentSession,
) -> Result<(), IntentMaterializationError> {
    session
        .accepted_state_for_current_input()
        .filter(|accepted| accepted.design_identity() == session.design_identity())
        .ok_or(IntentMaterializationError::SolverRejected)?;
    Ok(())
}

fn try_publish_host_inputs(
    session: &mut RetainedSketchDocumentSession,
    parameters: &ParameterBatch,
    snapshots: &ExternalSnapshotSet,
    request: DocumentSolveRequest,
) -> Result<(), IntentMaterializationError> {
    if session.parameter_batch() != parameters {
        let mut candidate = session.clone();
        candidate.update_parameter_batch(
            candidate.design_identity(),
            parameters.clone(),
            request,
        )?;
        if candidate.accepted_state_for_current_input().is_some() {
            *session = candidate;
        }
    }
    if session.external_snapshot_set() != snapshots {
        let mut candidate = session.clone();
        candidate.update_external_snapshot_set(
            candidate.design_identity(),
            snapshots.clone(),
            request,
        )?;
        if candidate.accepted_state_for_current_input().is_some()
            && candidate.external_snapshot_set() == snapshots
        {
            *session = candidate;
        }
    }
    Ok(())
}

fn require_exact_host_inputs(
    session: &RetainedSketchDocumentSession,
    parameters: &ParameterBatch,
    snapshots: &ExternalSnapshotSet,
) -> Result<(), IntentMaterializationError> {
    if session.parameter_batch() != parameters || session.external_snapshot_set() != snapshots {
        return Err(IntentMaterializationError::HostInput(
            "exact host inputs never became valid for the canonical declaration prefix".into(),
        ));
    }
    require_current_acceptance(session)
}

struct LoweringState {
    semantic: IntentSemanticIdentity,
    reservation_bindings: BTreeMap<ReservationId, IntentNativeBinding>,
    port_bindings: BTreeMap<IntentPortRef, IntentNativeBinding>,
    reverse_leaves: BTreeMap<IntentNativeWritableLeaf, LeafRef>,
    point_positions: BTreeMap<DesignPointId, [f64; 2]>,
    scalar_domains: BTreeMap<DesignScalarId, ScalarDomain>,
    curve_definitions: BTreeMap<CurveId, CurveDefinition>,
    parameter_kinds: BTreeMap<DocumentParameterId, DocumentParameterKind>,
    aggregate_bindings: BTreeMap<IntentPortRef, IntentAggregateMaterialization>,
    contact_states: BTreeMap<ContactId, (CurveSpan, i32, ContactDomain)>,
    topology_aggregate_nodes: BTreeMap<IntentPortRef, NodeId>,
    consumed_reservations: BTreeSet<ReservationId>,
    seed_node_ownership: BTreeMap<NodeId, Vec<IntentNativeBinding>>,
}

impl LoweringState {
    fn new(
        semantic: IntentSemanticIdentity,
        mut reservation_bindings: BTreeMap<ReservationId, IntentNativeBinding>,
        bootstrap: Option<IntentMaterializationMap>,
        document: &geosolve_sketch::SketchDocument,
    ) -> Self {
        let mut port_bindings = BTreeMap::new();
        let mut reverse_leaves = BTreeMap::new();
        let mut aggregate_bindings = BTreeMap::new();
        let mut consumed_reservations = BTreeSet::new();
        let mut seed_node_ownership = BTreeMap::new();
        if let Some(bootstrap) = bootstrap {
            for (reservation, binding) in bootstrap.reservations {
                reservation_bindings.insert(reservation, binding);
                consumed_reservations.insert(reservation);
            }
            port_bindings.extend(bootstrap.ports);
            reverse_leaves.extend(bootstrap.writable_leaves);
            aggregate_bindings.extend(
                bootstrap
                    .aggregates
                    .into_iter()
                    .map(|aggregate| (aggregate.port, aggregate)),
            );
            seed_node_ownership.extend(
                bootstrap
                    .nodes
                    .into_iter()
                    .map(|owner| (owner.node, owner.owned)),
            );
        }
        Self {
            semantic,
            reservation_bindings,
            port_bindings,
            reverse_leaves,
            point_positions: document
                .points()
                .iter()
                .map(|point| (point.id, point.position))
                .collect(),
            scalar_domains: document
                .scalars()
                .iter()
                .map(|scalar| (scalar.id, scalar.domain))
                .collect(),
            curve_definitions: document
                .curves()
                .iter()
                .map(|curve| (curve.id, curve.definition.clone()))
                .collect(),
            parameter_kinds: document
                .parameters()
                .iter()
                .map(|parameter| (parameter.id, parameter.kind))
                .collect(),
            aggregate_bindings,
            contact_states: document
                .contacts()
                .iter()
                .map(|contact| (contact.id, (contact.curve, contact.winding, contact.domain)))
                .collect(),
            topology_aggregate_nodes: BTreeMap::new(),
            consumed_reservations,
            seed_node_ownership,
        }
    }

    fn bind_schema_ports(&mut self, node: &IntentNode) -> Result<(), IntentMaterializationError> {
        for port in node.ports.values() {
            let reference = port.as_ref(node.id);
            if let Some(binding) = self.port_bindings.get(&reference).copied() {
                if !node.suppressed && matches!(port.flow, IntentIdentityFlow::Created { .. }) {
                    self.bind_reverse_leaves(node.id, port, binding)?;
                }
                continue;
            }
            let binding = match port.flow {
                IntentIdentityFlow::Created { reservation } => Some(
                    *self
                        .reservation_bindings
                        .get(&reservation)
                        .ok_or(IntentMaterializationError::MissingReservation { reservation })?,
                ),
                IntentIdentityFlow::Aliased { source }
                | IntentIdentityFlow::Continued { source, .. } => {
                    self.port_bindings.get(&source).copied()
                }
                IntentIdentityFlow::OwnedLogical => Some(IntentNativeBinding::Logical(reference)),
                IntentIdentityFlow::Retired { .. } => None,
            };
            if let Some(binding) = binding {
                self.port_bindings.insert(reference, binding);
                if !node.suppressed && matches!(port.flow, IntentIdentityFlow::Created { .. }) {
                    self.bind_reverse_leaves(node.id, port, binding)?;
                }
            }
        }
        Ok(())
    }

    fn bind_reverse_leaves(
        &mut self,
        node: NodeId,
        port: &IntentPort,
        binding: IntentNativeBinding,
    ) -> Result<(), IntentMaterializationError> {
        for field in &port.writable {
            let native = match (binding, field) {
                (IntentNativeBinding::Point(point), LeafField::X) => {
                    IntentNativeWritableLeaf::PointX { point }
                }
                (IntentNativeBinding::Point(point), LeafField::Y) => {
                    IntentNativeWritableLeaf::PointY { point }
                }
                (
                    IntentNativeBinding::Scalar(scalar),
                    LeafField::Value | LeafField::Angle | LeafField::Weight | LeafField::Parameter,
                ) => IntentNativeWritableLeaf::ScalarValue { scalar },
                _ => return Err(IntentMaterializationError::NativeKindMismatch { node }),
            };
            let leaf = LeafRef {
                node,
                port: port.id,
                field: *field,
            };
            if self
                .reverse_leaves
                .insert(native, leaf)
                .is_some_and(|existing| existing != leaf)
            {
                return Err(IntentMaterializationError::DuplicateWritableOwner { native });
            }
        }
        Ok(())
    }

    fn consume(&mut self, reservation: ReservationId) {
        self.consumed_reservations.insert(reservation);
    }

    fn consume_port(&mut self, node: &IntentNode, port: &IntentPort) {
        if let IntentIdentityFlow::Created { reservation } = port.flow {
            self.consume(reservation);
            if let Some(companion) = node
                .reservations
                .get(&reservation)
                .and_then(|entry| entry.paired_with)
            {
                self.consume(companion);
            }
        }
    }

    fn bind_aggregate(
        &mut self,
        node: &IntentNode,
        port: &IntentPort,
        spans: Vec<CurveSpan>,
        closed: bool,
        validate_topology: bool,
    ) {
        let reference = port.as_ref(node.id);
        self.aggregate_bindings.insert(
            reference,
            IntentAggregateMaterialization {
                port: reference,
                spans,
                closed,
            },
        );
        if validate_topology {
            self.topology_aggregate_nodes.insert(reference, node.id);
        }
    }

    fn validate_aggregates(
        &self,
        session: &RetainedSketchDocumentSession,
    ) -> Result<(), IntentMaterializationError> {
        if self.topology_aggregate_nodes.is_empty() {
            return Ok(());
        }
        let index =
            PreparedEndpointTopologyQuery::capture(session, EndpointTopologyRequest::default())
                .map_err(|_| IntentMaterializationError::InvalidAggregate {
                    node: *self
                        .topology_aggregate_nodes
                        .values()
                        .next()
                        .expect("nonempty topology aggregate map"),
                    reason: "accepted topology snapshot is unavailable",
                })?
                .execute()
                .map_err(|_| IntentMaterializationError::InvalidAggregate {
                    node: *self
                        .topology_aggregate_nodes
                        .values()
                        .next()
                        .expect("nonempty topology aggregate map"),
                    reason: "accepted endpoint topology exceeds deterministic limits",
                })?;
        for (port, node) in &self.topology_aggregate_nodes {
            let aggregate = self
                .aggregate_bindings
                .get(port)
                .expect("topology aggregate has an equation-free binding");
            validate_aggregate_topology(*node, aggregate, &index)?;
        }
        Ok(())
    }

    fn validate_declared_consumption(
        &self,
        candidate: &dyn IntentMaterializationSource,
    ) -> Result<(), IntentMaterializationError> {
        for (reservation, record) in candidate.reservations().entries() {
            if record.state == IntentReservationState::Declared
                && !self.consumed_reservations.contains(reservation)
            {
                return Err(IntentMaterializationError::UnconsumedReservation {
                    reservation: *reservation,
                });
            }
        }
        Ok(())
    }

    fn finish(
        self,
        records: &BTreeMap<ReservationId, IntentReservationRecord>,
    ) -> IntentMaterializationMap {
        let mut owned = self.seed_node_ownership;
        for (reservation, binding) in &self.reservation_bindings {
            if let Some(record) = records.get(reservation) {
                owned.entry(record.owner_node).or_default().push(*binding);
            }
        }
        let nodes = owned
            .into_iter()
            .map(|(node, mut owned)| {
                owned.sort_unstable();
                owned.dedup();
                IntentNodeMaterialization { node, owned }
            })
            .collect();
        IntentMaterializationMap {
            semantic: self.semantic,
            nodes,
            ports: self.port_bindings.into_iter().collect(),
            reservations: self.reservation_bindings.into_iter().collect(),
            writable_leaves: self.reverse_leaves.into_iter().collect(),
            aggregates: self.aggregate_bindings.into_values().collect(),
        }
    }
}

fn lower_point(
    candidate: &dyn IntentMaterializationSource,
    node: &IntentNode,
    batch: &mut SketchMaterializationBatch,
    state: &mut LoweringState,
) -> Result<(), IntentMaterializationError> {
    let port = require_port(node, IntentPortRole::Primary, 0)?;
    if let IntentIdentityFlow::Created { .. } = port.flow {
        let point = require_point_binding(node.id, state.port_bindings.get(&port.as_ref(node.id)))?;
        let position = point_position(candidate, node, port, [0.0, 0.0])?;
        batch.push_point(DesignPoint {
            id: point,
            label: node.symbol.as_str().to_owned(),
            position,
        });
        state.point_positions.insert(point, position);
        state.consume_port(node, port);
    }
    Ok(())
}

#[allow(
    clippy::too_many_lines,
    reason = "one exhaustive geometry lowering table keeps the closed recipe catalog auditable"
)]
fn lower_geometry(
    candidate: &dyn IntentMaterializationSource,
    node: &IntentNode,
    recipe: GeometryRecipeKind,
    batch: &mut SketchMaterializationBatch,
    state: &mut LoweringState,
) -> Result<(), IntentMaterializationError> {
    use GeometryRecipeKind as G;
    use IntentPortRole as R;

    match recipe {
        G::SketchPoint => lower_point(candidate, node, batch, state),
        G::Segment => lower_segment(candidate, node, batch, state),
        G::Polyline => lower_polyline(candidate, node, batch, state),
        G::MidpointLine => lower_midpoint_line(candidate, node, batch, state),
        G::TwoPointAlignedRectangle
        | G::ThreePointCornerRectangle
        | G::CenterRectangle
        | G::ThreePointCenterRectangle => lower_rectangle(candidate, node, recipe, batch, state),
        G::TangentArc => lower_tangent_arc(candidate, node, batch, state),
        G::OpenControlBSpline
        | G::PeriodicControlBSpline
        | G::OpenControlNurbs
        | G::PeriodicControlNurbs => lower_control_spline(candidate, node, recipe, batch, state),
        G::CenterRadiusCircle | G::TwoPointDiameterCircle | G::ThreePointCircle => {
            let (center, _) = materialize_point(
                candidate,
                node,
                R::Center,
                0,
                [0.0, 0.0],
                "center",
                batch,
                state,
            )?;
            let radius = materialize_scalar(
                candidate,
                node,
                0,
                LeafField::Value,
                1.0,
                IntentUnit::Length,
                ScalarUnit::Length,
                ScalarDomain::Positive,
                "radius",
                batch,
                state,
            )?;
            materialize_curve(
                node,
                0,
                CurveDefinition::Circle { center, radius },
                batch,
                state,
            )
        }
        G::CenterArc | G::ThreePointArc => {
            let (center, _) = materialize_point(
                candidate,
                node,
                R::Center,
                0,
                [0.0, 0.0],
                "center",
                batch,
                state,
            )?;
            let radius = materialize_scalar(
                candidate,
                node,
                0,
                LeafField::Value,
                1.0,
                IntentUnit::Length,
                ScalarUnit::Length,
                ScalarDomain::Positive,
                "radius",
                batch,
                state,
            )?;
            let start_angle = materialize_scalar(
                candidate,
                node,
                1,
                LeafField::Angle,
                0.0,
                IntentUnit::Angle,
                ScalarUnit::Angle,
                ScalarDomain::Finite,
                "start angle",
                batch,
                state,
            )?;
            let end_angle = materialize_scalar(
                candidate,
                node,
                2,
                LeafField::Angle,
                std::f64::consts::FRAC_PI_2,
                IntentUnit::Angle,
                ScalarUnit::Angle,
                ScalarDomain::Finite,
                "end angle",
                batch,
                state,
            )?;
            materialize_curve(
                node,
                0,
                CurveDefinition::CircularArc {
                    center,
                    radius,
                    start_angle,
                    end_angle,
                    sweep: arc_sweep(node)?,
                },
                batch,
                state,
            )
        }
        G::CenterAxesEllipse | G::AxisEndpointsEllipse => {
            let (center, _) = materialize_point(
                candidate,
                node,
                R::Center,
                0,
                [0.0, 0.0],
                "center",
                batch,
                state,
            )?;
            let (major_axis_point, _) = materialize_point(
                candidate,
                node,
                R::MajorAxisPoint,
                0,
                [2.0, 0.0],
                "major axis",
                batch,
                state,
            )?;
            let minor_axis_ratio = materialize_scalar(
                candidate,
                node,
                0,
                LeafField::Parameter,
                0.5,
                IntentUnit::Dimensionless,
                ScalarUnit::Parameter,
                ScalarDomain::Bounded {
                    lower: f64::from_bits(1),
                    upper: 1.0,
                },
                "minor-axis ratio",
                batch,
                state,
            )?;
            materialize_curve(
                node,
                0,
                CurveDefinition::Ellipse {
                    center,
                    major_axis_point,
                    minor_axis_ratio,
                },
                batch,
                state,
            )
        }
        G::CenterAxesEllipticalArc | G::AxisEndpointsEllipticalArc => {
            let (center, _) = materialize_point(
                candidate,
                node,
                R::Center,
                0,
                [0.0, 0.0],
                "center",
                batch,
                state,
            )?;
            let (major_axis_point, _) = materialize_point(
                candidate,
                node,
                R::MajorAxisPoint,
                0,
                [2.0, 0.0],
                "major axis",
                batch,
                state,
            )?;
            let minor_axis_ratio = materialize_scalar(
                candidate,
                node,
                0,
                LeafField::Parameter,
                0.5,
                IntentUnit::Dimensionless,
                ScalarUnit::Parameter,
                ScalarDomain::Bounded {
                    lower: f64::from_bits(1),
                    upper: 1.0,
                },
                "minor-axis ratio",
                batch,
                state,
            )?;
            let start_angle = materialize_scalar(
                candidate,
                node,
                1,
                LeafField::Angle,
                0.0,
                IntentUnit::Angle,
                ScalarUnit::Angle,
                ScalarDomain::Finite,
                "start angle",
                batch,
                state,
            )?;
            let end_angle = materialize_scalar(
                candidate,
                node,
                2,
                LeafField::Angle,
                std::f64::consts::FRAC_PI_2,
                IntentUnit::Angle,
                ScalarUnit::Angle,
                ScalarDomain::Finite,
                "end angle",
                batch,
                state,
            )?;
            materialize_curve(
                node,
                0,
                CurveDefinition::EllipticalArc {
                    center,
                    major_axis_point,
                    minor_axis_ratio,
                    start_angle,
                    end_angle,
                    sweep: arc_sweep(node)?,
                },
                batch,
                state,
            )
        }
        G::QuadraticBezier => {
            let controls = [
                materialize_point(
                    candidate,
                    node,
                    R::Start,
                    0,
                    [0.0, 0.0],
                    "start",
                    batch,
                    state,
                )?
                .0,
                materialize_point(
                    candidate,
                    node,
                    R::Control,
                    0,
                    [1.0, 1.0],
                    "control",
                    batch,
                    state,
                )?
                .0,
                materialize_point(candidate, node, R::End, 0, [2.0, 0.0], "end", batch, state)?.0,
            ];
            materialize_curve(
                node,
                0,
                CurveDefinition::QuadraticBezier { controls },
                batch,
                state,
            )
        }
        G::CubicBezier => {
            let controls = [
                materialize_point(
                    candidate,
                    node,
                    R::Start,
                    0,
                    [0.0, 0.0],
                    "start",
                    batch,
                    state,
                )?
                .0,
                materialize_point(
                    candidate,
                    node,
                    R::Control,
                    0,
                    [1.0, 1.0],
                    "control 1",
                    batch,
                    state,
                )?
                .0,
                materialize_point(
                    candidate,
                    node,
                    R::Control,
                    1,
                    [2.0, 1.0],
                    "control 2",
                    batch,
                    state,
                )?
                .0,
                materialize_point(candidate, node, R::End, 0, [3.0, 0.0], "end", batch, state)?.0,
            ];
            materialize_curve(
                node,
                0,
                CurveDefinition::CubicBezier { controls },
                batch,
                state,
            )
        }
        G::RationalQuadraticConic => {
            let start = materialize_point(
                candidate,
                node,
                R::Start,
                0,
                [0.0, 0.0],
                "start",
                batch,
                state,
            )?
            .0;
            let end =
                materialize_point(candidate, node, R::End, 0, [2.0, 0.0], "end", batch, state)?.0;
            let middle_weight = materialize_scalar(
                candidate,
                node,
                0,
                LeafField::Weight,
                1.0,
                IntentUnit::Dimensionless,
                ScalarUnit::Parameter,
                ScalarDomain::Bounded {
                    lower: MIN_RATIONAL_QUADRATIC_MIDDLE_WEIGHT,
                    upper: f64::MAX,
                },
                "middle weight",
                batch,
                state,
            )?;
            let weighted_middle = point_field(node, "weighted_middle")?.unwrap_or([1.0, 1.0]);
            materialize_curve(
                node,
                0,
                CurveDefinition::RationalQuadraticConic {
                    start,
                    weighted_middle,
                    middle_weight,
                    end,
                },
                batch,
                state,
            )
        }
        G::Parabola => {
            let vertex = materialize_point(
                candidate,
                node,
                R::Center,
                0,
                [0.0, 0.0],
                "vertex",
                batch,
                state,
            )?
            .0;
            let focus = materialize_point(
                candidate,
                node,
                R::Control,
                0,
                [0.0, 1.0],
                "focus",
                batch,
                state,
            )?
            .0;
            let trim_start = materialize_scalar(
                candidate,
                node,
                0,
                LeafField::Parameter,
                -1.0,
                IntentUnit::Dimensionless,
                ScalarUnit::Parameter,
                ScalarDomain::Finite,
                "trim start",
                batch,
                state,
            )?;
            let trim_end = materialize_scalar(
                candidate,
                node,
                1,
                LeafField::Parameter,
                1.0,
                IntentUnit::Dimensionless,
                ScalarUnit::Parameter,
                ScalarDomain::Finite,
                "trim end",
                batch,
                state,
            )?;
            materialize_curve(
                node,
                0,
                CurveDefinition::ParabolaSegment {
                    vertex,
                    focus,
                    trim_start,
                    trim_end,
                },
                batch,
                state,
            )
        }
        G::Hyperbola => {
            let center = materialize_point(
                candidate,
                node,
                R::Center,
                0,
                [0.0, 0.0],
                "center",
                batch,
                state,
            )?
            .0;
            let transverse_axis_point = materialize_point(
                candidate,
                node,
                R::Control,
                0,
                [1.0, 0.0],
                "transverse axis",
                batch,
                state,
            )?
            .0;
            let semi_conjugate = materialize_scalar(
                candidate,
                node,
                0,
                LeafField::Value,
                1.0,
                IntentUnit::Length,
                ScalarUnit::Length,
                ScalarDomain::Positive,
                "semi-conjugate",
                batch,
                state,
            )?;
            let trim_start = materialize_scalar(
                candidate,
                node,
                1,
                LeafField::Parameter,
                -1.0,
                IntentUnit::Dimensionless,
                ScalarUnit::Parameter,
                ScalarDomain::Finite,
                "trim start",
                batch,
                state,
            )?;
            let trim_end = materialize_scalar(
                candidate,
                node,
                2,
                LeafField::Parameter,
                1.0,
                IntentUnit::Dimensionless,
                ScalarUnit::Parameter,
                ScalarDomain::Finite,
                "trim end",
                batch,
                state,
            )?;
            materialize_curve(
                node,
                0,
                CurveDefinition::HyperbolaSegment {
                    center,
                    transverse_axis_point,
                    semi_conjugate,
                    branch: hyperbola_branch(node)?,
                    trim_start,
                    trim_end,
                },
                batch,
                state,
            )
        }
    }
}

fn child_port(
    node: &IntentNode,
    order_index: usize,
    role: IntentPortRole,
) -> Result<&IntentPort, IntentMaterializationError> {
    let child = node
        .child_order
        .get(order_index)
        .and_then(|child| node.children.get(child))
        .ok_or(IntentMaterializationError::InvalidGeometry {
            node: node.id,
            reason: "dynamic child order is incomplete",
        })?;
    child
        .ports
        .iter()
        .filter_map(|port| node.port(*port))
        .find(|port| {
            matches!(
                port.selector,
                IntentPortSelector::InitialChild {
                    role: candidate,
                    index: 0,
                    ..
                } if candidate == role
            )
        })
        .ok_or(IntentMaterializationError::MissingPort {
            node: node.id,
            selector: IntentPortSelector::InitialChild {
                ordinal: u16::try_from(order_index).unwrap_or(u16::MAX),
                role,
                index: 0,
            },
        })
}

#[allow(clippy::too_many_arguments)]
fn materialize_point_port(
    candidate: &dyn IntentMaterializationSource,
    node: &IntentNode,
    port: &IntentPort,
    default: [f64; 2],
    label: &str,
    batch: &mut SketchMaterializationBatch,
    state: &mut LoweringState,
) -> Result<(DesignPointId, [f64; 2]), IntentMaterializationError> {
    let point = require_point_binding(node.id, state.port_bindings.get(&port.as_ref(node.id)))?;
    let position = resolve_point_position(candidate, node, port, point, default, state)?;
    if matches!(port.flow, IntentIdentityFlow::Created { .. }) {
        batch.push_point(DesignPoint {
            id: point,
            label: format!("{}.{}", node.symbol.as_str(), label),
            position,
        });
        state.point_positions.insert(point, position);
        state.consume_port(node, port);
    }
    Ok((point, position))
}

#[allow(clippy::too_many_arguments)]
fn materialize_scalar_port(
    candidate: &dyn IntentMaterializationSource,
    node: &IntentNode,
    port: &IntentPort,
    field: LeafField,
    default: f64,
    intent_unit: IntentUnit,
    unit: ScalarUnit,
    domain: ScalarDomain,
    label: &str,
    batch: &mut SketchMaterializationBatch,
    state: &mut LoweringState,
) -> Result<(DesignScalarId, f64), IntentMaterializationError> {
    let scalar = match state.port_bindings.get(&port.as_ref(node.id)) {
        Some(IntentNativeBinding::Scalar(scalar)) => *scalar,
        _ => return Err(IntentMaterializationError::NativeKindMismatch { node: node.id }),
    };
    let value = if matches!(port.flow, IntentIdentityFlow::Created { .. }) {
        let value = leaf_quantity(candidate, node.id, port.id, field, default, intent_unit)?;
        batch.push_scalar(DesignScalar {
            id: scalar,
            label: format!("{}.{}", node.symbol.as_str(), label),
            value,
            unit,
            domain,
        });
        state.scalar_domains.insert(scalar, domain);
        state.consume_port(node, port);
        value
    } else {
        return Err(IntentMaterializationError::InvalidGeometry {
            node: node.id,
            reason: "aliased recipe scalar values are not supported",
        });
    };
    Ok((scalar, value))
}

fn finite_direction(
    node: NodeId,
    start: [f64; 2],
    end: [f64; 2],
) -> Result<[f64; 2], IntentMaterializationError> {
    let direction = [end[0] - start[0], end[1] - start[1]];
    let length = direction[0].hypot(direction[1]);
    if !(length.is_finite() && length > 0.0) {
        return Err(IntentMaterializationError::InvalidGeometry {
            node,
            reason: "curve endpoints must define a finite nonzero span",
        });
    }
    Ok([direction[0] / length, direction[1] / length])
}

fn lower_polyline(
    candidate: &dyn IntentMaterializationSource,
    node: &IntentNode,
    batch: &mut SketchMaterializationBatch,
    state: &mut LoweringState,
) -> Result<(), IntentMaterializationError> {
    let count = node.child_order.len();
    if count < 2 {
        return Err(IntentMaterializationError::InvalidGeometry {
            node: node.id,
            reason: "an open polyline requires at least two vertices",
        });
    }
    let mut points = Vec::with_capacity(count);
    let mut positions = Vec::with_capacity(count);
    for index in 0..count {
        let port = child_port(node, index, IntentPortRole::Corner)?;
        let ordinal = u32::try_from(index).map_or(f64::MAX, f64::from);
        let default = [ordinal, if index % 2 == 0 { 0.0 } else { 1.0 }];
        let (point, position) = materialize_point_port(
            candidate,
            node,
            port,
            default,
            &format!("vertex{}", index + 1),
            batch,
            state,
        )?;
        points.push(point);
        positions.push(position);
    }
    let closed = field_boolean(node, "closed", false)?;
    let span_count = count - 1 + usize::from(closed);
    let branch_directions = (0..span_count)
        .map(|index| {
            point_field(node, &format!("branch_direction_{index:04}"))?.map_or_else(
                || finite_direction(node.id, positions[index], positions[(index + 1) % count]),
                |direction| {
                    let magnitude = direction[0].hypot(direction[1]);
                    if magnitude.is_finite()
                        && magnitude > 0.0
                        && (magnitude - 1.0).abs() <= 64.0 * f64::EPSILON
                    {
                        Ok(direction)
                    } else {
                        Err(IntentMaterializationError::InvalidGeometry {
                            node: node.id,
                            reason: "polyline branch direction must be finite, nonzero, and normalized",
                        })
                    }
                },
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    materialize_curve(
        node,
        0,
        CurveDefinition::Polyline {
            points,
            closed,
            branch_directions,
        },
        batch,
        state,
    )?;
    let curve = curve_binding(node, 0, state)?;
    let spans = (0..span_count)
        .map(|segment| {
            Ok(CurveSpan {
                curve,
                segment: u32::try_from(segment).map_err(|_| {
                    IntentMaterializationError::InvalidGeometry {
                        node: node.id,
                        reason: "polyline span index exceeds persistent limits",
                    }
                })?,
            })
        })
        .collect::<Result<Vec<_>, IntentMaterializationError>>()?;
    for (index, span) in spans.iter().copied().enumerate() {
        let port = child_port(node, index, IntentPortRole::Span)?;
        state
            .port_bindings
            .insert(port.as_ref(node.id), IntentNativeBinding::CurveSpan(span));
    }
    if let Some(collection) = node.port_by_selector(IntentPortSelector::Node {
        role: IntentPortRole::Collection,
        index: 0,
    }) {
        state.bind_aggregate(node, collection, spans, closed, false);
    }
    Ok(())
}

fn reflected_point(
    node: NodeId,
    center: [f64; 2],
    sample: [f64; 2],
) -> Result<[f64; 2], IntentMaterializationError> {
    let reflected = [
        center[0] + (center[0] - sample[0]),
        center[1] + (center[1] - sample[1]),
    ];
    reflected
        .into_iter()
        .all(f64::is_finite)
        .then_some(reflected)
        .ok_or(IntentMaterializationError::InvalidGeometry {
            node,
            reason: "reflected recipe point is non-finite",
        })
}

fn lower_midpoint_line(
    candidate: &dyn IntentMaterializationSource,
    node: &IntentNode,
    batch: &mut SketchMaterializationBatch,
    state: &mut LoweringState,
) -> Result<(), IntentMaterializationError> {
    let (midpoint, midpoint_position) = materialize_point(
        candidate,
        node,
        IntentPortRole::Midpoint,
        0,
        [0.0, 0.0],
        "midpoint",
        batch,
        state,
    )?;
    let (end, end_position) = materialize_point(
        candidate,
        node,
        IntentPortRole::End,
        0,
        [1.0, 0.0],
        "end",
        batch,
        state,
    )?;
    let reflected = reflected_point(node.id, midpoint_position, end_position)?;
    let (start, start_position) = materialize_point(
        candidate,
        node,
        IntentPortRole::Start,
        0,
        reflected,
        "start",
        batch,
        state,
    )?;
    let branch_direction = branch_direction(node, start_position, end_position)?;
    materialize_curve(
        node,
        0,
        CurveDefinition::Line {
            start,
            end,
            branch_direction,
        },
        batch,
        state,
    )?;
    materialize_indexed_constraint(
        node,
        0,
        DocumentConstraintDefinition::Midpoint {
            point: midpoint,
            line: CurveSpan::line(curve_binding(node, 0, state)?),
        },
        batch,
        state,
    )
}

fn materialize_rectangle_point(
    candidate: &dyn IntentMaterializationSource,
    node: &IntentNode,
    index: u16,
    default: [f64; 2],
    batch: &mut SketchMaterializationBatch,
    state: &mut LoweringState,
) -> Result<(DesignPointId, [f64; 2]), IntentMaterializationError> {
    materialize_point(
        candidate,
        node,
        IntentPortRole::Corner,
        index,
        default,
        &format!("corner{}", index + 1),
        batch,
        state,
    )
}

fn finite_parallelogram_corner(
    node: NodeId,
    first: [f64; 2],
    adjacent: [f64; 2],
    third: [f64; 2],
) -> Result<[f64; 2], IntentMaterializationError> {
    let fourth = [
        first[0] + (third[0] - adjacent[0]),
        first[1] + (third[1] - adjacent[1]),
    ];
    fourth
        .into_iter()
        .all(f64::is_finite)
        .then_some(fourth)
        .ok_or(IntentMaterializationError::InvalidGeometry {
            node,
            reason: "derived rectangle corner is non-finite",
        })
}

type MaterializedPoint = (DesignPointId, [f64; 2]);

fn materialize_aligned_rectangle_corners(
    candidate: &dyn IntentMaterializationSource,
    node: &IntentNode,
    batch: &mut SketchMaterializationBatch,
    state: &mut LoweringState,
) -> Result<[MaterializedPoint; 4], IntentMaterializationError> {
    let first = materialize_rectangle_point(candidate, node, 0, [0.0, 0.0], batch, state)?;
    let third = materialize_rectangle_point(candidate, node, 2, [2.0, 1.0], batch, state)?;
    let second =
        materialize_rectangle_point(candidate, node, 1, [third.1[0], first.1[1]], batch, state)?;
    let fourth =
        materialize_rectangle_point(candidate, node, 3, [first.1[0], third.1[1]], batch, state)?;
    Ok([first, second, third, fourth])
}

fn materialize_corner_rectangle_corners(
    candidate: &dyn IntentMaterializationSource,
    node: &IntentNode,
    batch: &mut SketchMaterializationBatch,
    state: &mut LoweringState,
) -> Result<[MaterializedPoint; 4], IntentMaterializationError> {
    let first = materialize_rectangle_point(candidate, node, 0, [0.0, 0.0], batch, state)?;
    let second = materialize_rectangle_point(candidate, node, 1, [2.0, 0.0], batch, state)?;
    let third = materialize_rectangle_point(candidate, node, 2, [2.0, 1.0], batch, state)?;
    let fourth_position = finite_parallelogram_corner(node.id, first.1, second.1, third.1)?;
    let fourth = materialize_rectangle_point(candidate, node, 3, fourth_position, batch, state)?;
    Ok([first, second, third, fourth])
}

fn materialize_center_rectangle_corners(
    candidate: &dyn IntentMaterializationSource,
    node: &IntentNode,
    batch: &mut SketchMaterializationBatch,
    state: &mut LoweringState,
) -> Result<([MaterializedPoint; 4], MaterializedPoint), IntentMaterializationError> {
    let center = materialize_point(
        candidate,
        node,
        IntentPortRole::Center,
        0,
        [0.0, 0.0],
        "center",
        batch,
        state,
    )?;
    let first = materialize_rectangle_point(candidate, node, 0, [1.0, 1.0], batch, state)?;
    let third_position = reflected_point(node.id, center.1, first.1)?;
    let third = materialize_rectangle_point(candidate, node, 2, third_position, batch, state)?;
    let second =
        materialize_rectangle_point(candidate, node, 1, [third.1[0], first.1[1]], batch, state)?;
    let fourth =
        materialize_rectangle_point(candidate, node, 3, [first.1[0], third.1[1]], batch, state)?;
    Ok(([first, second, third, fourth], center))
}

fn materialize_three_point_center_rectangle_corners(
    candidate: &dyn IntentMaterializationSource,
    node: &IntentNode,
    batch: &mut SketchMaterializationBatch,
    state: &mut LoweringState,
) -> Result<([MaterializedPoint; 4], MaterializedPoint), IntentMaterializationError> {
    let center = materialize_point(
        candidate,
        node,
        IntentPortRole::Center,
        0,
        [0.0, 0.0],
        "center",
        batch,
        state,
    )?;
    let first = materialize_rectangle_point(candidate, node, 0, [1.0, 1.0], batch, state)?;
    let side_midpoint = point_field(node, "side_midpoint")?.unwrap_or([center.1[0], first.1[1]]);
    validate_perpendicular_rectangle_axes(node.id, center.1, side_midpoint, first.1)?;
    let second_position = reflected_point(node.id, side_midpoint, first.1)?;
    let second = materialize_rectangle_point(candidate, node, 1, second_position, batch, state)?;
    let third_position = reflected_point(node.id, center.1, first.1)?;
    let third = materialize_rectangle_point(candidate, node, 2, third_position, batch, state)?;
    let fourth_position = reflected_point(node.id, center.1, second.1)?;
    let fourth = materialize_rectangle_point(candidate, node, 3, fourth_position, batch, state)?;
    Ok(([first, second, third, fourth], center))
}

fn validate_perpendicular_rectangle_axes(
    node: NodeId,
    center: [f64; 2],
    side_midpoint: [f64; 2],
    corner: [f64; 2],
) -> Result<(), IntentMaterializationError> {
    let half_height = [side_midpoint[0] - center[0], side_midpoint[1] - center[1]];
    let half_width = [corner[0] - side_midpoint[0], corner[1] - side_midpoint[1]];
    let height = half_height[0].hypot(half_height[1]);
    let width = half_width[0].hypot(half_width[1]);
    let normalized_dot =
        half_height[0].mul_add(half_width[0], half_height[1] * half_width[1]) / (height * width);
    if height.is_finite()
        && height > 0.0
        && width.is_finite()
        && width > 0.0
        && normalized_dot.is_finite()
        && normalized_dot.abs() <= 1.0e-9
    {
        Ok(())
    } else {
        Err(IntentMaterializationError::InvalidGeometry {
            node,
            reason: "three-point centre rectangle samples must define perpendicular axes",
        })
    }
}

fn materialize_rectangle_corners(
    candidate: &dyn IntentMaterializationSource,
    node: &IntentNode,
    recipe: GeometryRecipeKind,
    batch: &mut SketchMaterializationBatch,
    state: &mut LoweringState,
) -> Result<([MaterializedPoint; 4], Option<MaterializedPoint>), IntentMaterializationError> {
    use GeometryRecipeKind as G;

    match recipe {
        G::TwoPointAlignedRectangle => Ok((
            materialize_aligned_rectangle_corners(candidate, node, batch, state)?,
            None,
        )),
        G::ThreePointCornerRectangle => Ok((
            materialize_corner_rectangle_corners(candidate, node, batch, state)?,
            None,
        )),
        G::CenterRectangle => {
            let (corners, center) =
                materialize_center_rectangle_corners(candidate, node, batch, state)?;
            Ok((corners, Some(center)))
        }
        G::ThreePointCenterRectangle => {
            let (corners, center) =
                materialize_three_point_center_rectangle_corners(candidate, node, batch, state)?;
            Ok((corners, Some(center)))
        }
        _ => unreachable!("rectangle lowering called with non-rectangle recipe"),
    }
}

fn materialize_rectangle_curves(
    node: &IntentNode,
    corners: &[MaterializedPoint; 4],
    has_center: bool,
    batch: &mut SketchMaterializationBatch,
    state: &mut LoweringState,
) -> Result<(), IntentMaterializationError> {
    let role = geometry_role(node)?;
    for index in 0..4 {
        let next = (index + 1) % 4;
        let branch_direction = finite_direction(node.id, corners[index].1, corners[next].1)?;
        materialize_curve_with_role(
            node,
            u16::try_from(index).expect("four rectangle edges fit u16"),
            CurveDefinition::Line {
                start: corners[index].0,
                end: corners[next].0,
                branch_direction,
            },
            role,
            batch,
            state,
        )?;
    }
    if has_center {
        let branch_direction = finite_direction(node.id, corners[0].1, corners[2].1)?;
        materialize_curve_with_role(
            node,
            4,
            CurveDefinition::Line {
                start: corners[0].0,
                end: corners[2].0,
                branch_direction,
            },
            GeometryRole::Construction,
            batch,
            state,
        )?;
    }
    Ok(())
}

fn lower_rectangle(
    candidate: &dyn IntentMaterializationSource,
    node: &IntentNode,
    recipe: GeometryRecipeKind,
    batch: &mut SketchMaterializationBatch,
    state: &mut LoweringState,
) -> Result<(), IntentMaterializationError> {
    use GeometryRecipeKind as G;

    let (corners, center) = materialize_rectangle_corners(candidate, node, recipe, batch, state)?;
    materialize_rectangle_curves(node, &corners, center.is_some(), batch, state)?;
    let spans = (0..4)
        .map(|index| {
            Ok(CurveSpan::line(curve_binding(
                node,
                u16::try_from(index).expect("rectangle edge index fits u16"),
                state,
            )?))
        })
        .collect::<Result<Vec<_>, IntentMaterializationError>>()?;
    let mut definitions = match recipe {
        G::TwoPointAlignedRectangle | G::CenterRectangle => vec![
            DocumentConstraintDefinition::Horizontal { line: spans[0] },
            DocumentConstraintDefinition::Vertical { line: spans[1] },
            DocumentConstraintDefinition::Horizontal { line: spans[2] },
            DocumentConstraintDefinition::Vertical { line: spans[3] },
        ],
        G::ThreePointCornerRectangle | G::ThreePointCenterRectangle => vec![
            DocumentConstraintDefinition::Perpendicular {
                first: spans[0],
                second: spans[1],
            },
            DocumentConstraintDefinition::Parallel {
                first: spans[0],
                second: spans[2],
            },
            DocumentConstraintDefinition::Parallel {
                first: spans[1],
                second: spans[3],
            },
        ],
        _ => unreachable!("guarded rectangle recipe"),
    };
    if let Some((center, _)) = center {
        definitions.push(DocumentConstraintDefinition::Midpoint {
            point: center,
            line: CurveSpan::line(curve_binding(node, 4, state)?),
        });
    }
    if field_boolean(node, "regularized", false)? {
        definitions.push(DocumentConstraintDefinition::EqualLength {
            first: spans[0],
            second: spans[1],
        });
    }
    for (index, definition) in definitions.into_iter().enumerate() {
        materialize_indexed_constraint(
            node,
            u16::try_from(index).expect("bounded rectangle relation index fits u16"),
            definition,
            batch,
            state,
        )?;
    }
    Ok(())
}

fn lower_tangent_arc(
    candidate: &dyn IntentMaterializationSource,
    node: &IntentNode,
    batch: &mut SketchMaterializationBatch,
    state: &mut LoweringState,
) -> Result<(), IntentMaterializationError> {
    materialize_tangent_arc_curve(candidate, node, batch, state)?;
    materialize_tangent_arc_relation(candidate, node, batch, state)
}

fn materialize_tangent_arc_curve(
    candidate: &dyn IntentMaterializationSource,
    node: &IntentNode,
    batch: &mut SketchMaterializationBatch,
    state: &mut LoweringState,
) -> Result<(), IntentMaterializationError> {
    let center = materialize_point(
        candidate,
        node,
        IntentPortRole::Center,
        0,
        [0.0, 0.0],
        "center",
        batch,
        state,
    )?
    .0;
    let radius = materialize_scalar(
        candidate,
        node,
        0,
        LeafField::Value,
        1.0,
        IntentUnit::Length,
        ScalarUnit::Length,
        ScalarDomain::Positive,
        "radius",
        batch,
        state,
    )?;
    let start_angle = materialize_scalar(
        candidate,
        node,
        1,
        LeafField::Angle,
        0.0,
        IntentUnit::Angle,
        ScalarUnit::Angle,
        ScalarDomain::Finite,
        "start angle",
        batch,
        state,
    )?;
    let end_angle = materialize_scalar(
        candidate,
        node,
        2,
        LeafField::Angle,
        std::f64::consts::FRAC_PI_2,
        IntentUnit::Angle,
        ScalarUnit::Angle,
        ScalarDomain::Finite,
        "end angle",
        batch,
        state,
    )?;
    materialize_curve(
        node,
        0,
        CurveDefinition::CircularArc {
            center,
            radius,
            start_angle,
            end_angle,
            sweep: arc_sweep(node)?,
        },
        batch,
        state,
    )
}

fn materialize_tangent_arc_relation(
    candidate: &dyn IntentMaterializationSource,
    node: &IntentNode,
    batch: &mut SketchMaterializationBatch,
    state: &mut LoweringState,
) -> Result<(), IntentMaterializationError> {
    let source = input_span(node, state, 0)?;
    let orientation = match enum_field(node, "orientation")? {
        None | Some("aligned") => TangentOrientation::Aligned,
        Some("opposed") => TangentOrientation::Opposed,
        Some(_) => {
            return Err(IntentMaterializationError::InvalidGeometry {
                node: node.id,
                reason: "tangent orientation must be aligned or opposed",
            });
        }
    };
    let source_domain = inferred_contact_domain(node, "source", source, state)?;
    let source_range = relation_contact_range(node, "source")?;
    let source_neighborhood = tangent_source_neighborhood(node)?;
    let source_winding =
        i32::try_from(field_integer(node, "source_winding", 0)?).map_err(|_| {
            IntentMaterializationError::InvalidGeometry {
                node: node.id,
                reason: "tangent source winding exceeds persistent limits",
            }
        })?;
    let source_parameter =
        field_quantity(node, "source_parameter", IntentUnit::Dimensionless)?.unwrap_or(1.0);
    let created_span = CurveSpan::line(curve_binding(node, 0, state)?);
    let source_contact = materialize_recipe_contact(
        candidate,
        node,
        0,
        source,
        source_parameter,
        source_domain,
        source_range,
        source_winding,
        source_neighborhood,
        Some(orientation),
        batch,
        state,
    )?;
    let created_contact = materialize_recipe_contact(
        candidate,
        node,
        1,
        created_span,
        0.0,
        ContactDomain::Bounded {
            lower: 0.0,
            upper: 1.0,
        },
        None,
        0,
        ContactNeighborhood::Start,
        Some(orientation),
        batch,
        state,
    )?;
    materialize_indexed_constraint(
        node,
        0,
        DocumentConstraintDefinition::CurveCurveTangency {
            first_contact: source_contact,
            second_contact: created_contact,
        },
        batch,
        state,
    )
}

fn tangent_source_neighborhood(
    node: &IntentNode,
) -> Result<ContactNeighborhood, IntentMaterializationError> {
    match enum_field(node, "source_neighborhood")? {
        None | Some("end") => Ok(ContactNeighborhood::End),
        Some("start") => Ok(ContactNeighborhood::Start),
        Some("interior") => Ok(ContactNeighborhood::Interior),
        Some("local") => Ok(ContactNeighborhood::Local {
            lower: field_quantity(node, "source_neighborhood_lower", IntentUnit::Dimensionless)?
                .unwrap_or(0.0),
            upper: field_quantity(node, "source_neighborhood_upper", IntentUnit::Dimensionless)?
                .unwrap_or(1.0),
        }),
        Some(_) => Err(IntentMaterializationError::InvalidGeometry {
            node: node.id,
            reason: "tangent source neighborhood is invalid",
        }),
    }
}

#[allow(clippy::too_many_arguments)]
fn materialize_recipe_contact(
    candidate: &dyn IntentMaterializationSource,
    node: &IntentNode,
    index: u16,
    curve: CurveSpan,
    parameter_value: f64,
    domain: ContactDomain,
    admissible_range: Option<ContactAdmissibleRange>,
    winding: i32,
    neighborhood: ContactNeighborhood,
    orientation: Option<TangentOrientation>,
    batch: &mut SketchMaterializationBatch,
    state: &mut LoweringState,
) -> Result<ContactId, IntentMaterializationError> {
    let parameter_port = require_port(node, IntentPortRole::Parameter, index)?;
    let (unit, scalar_domain) = match domain {
        ContactDomain::SupportingLine => (ScalarUnit::Parameter, ScalarDomain::Finite),
        ContactDomain::Bounded { lower, upper } => (
            ScalarUnit::Parameter,
            ScalarDomain::Bounded { lower, upper },
        ),
        ContactDomain::Periodic { period } => {
            (ScalarUnit::Angle, ScalarDomain::Periodic { period })
        }
    };
    let (parameter, _) = materialize_scalar_port(
        candidate,
        node,
        parameter_port,
        LeafField::Parameter,
        parameter_value,
        IntentUnit::Dimensionless,
        unit,
        scalar_domain,
        &format!("contact{} parameter", index + 1),
        batch,
        state,
    )?;
    let contact_port = require_port(node, IntentPortRole::Contact, index)?;
    let contact = match state.port_bindings.get(&contact_port.as_ref(node.id)) {
        Some(IntentNativeBinding::Contact(contact)) => *contact,
        _ => return Err(IntentMaterializationError::NativeKindMismatch { node: node.id }),
    };
    batch.push_contact(ContactSlot {
        id: contact,
        label: format!("{}.contact{}", node.symbol.as_str(), index + 1),
        curve,
        parameter,
        domain,
        admissible_range,
        winding,
        neighborhood,
        tangent_orientation: orientation,
    });
    state
        .contact_states
        .insert(contact, (curve, winding, domain));
    state.consume_port(node, contact_port);
    Ok(contact)
}

fn spline_topology(
    node: NodeId,
    form: DocumentBSplineForm,
    degree: u32,
    control_count: usize,
) -> Result<(Vec<f64>, Vec<u32>, u32), IntentMaterializationError> {
    let degree =
        usize::try_from(degree).map_err(|_| IntentMaterializationError::InvalidGeometry {
            node,
            reason: "spline degree exceeds platform limits",
        })?;
    let span_count = match form {
        DocumentBSplineForm::Clamped => control_count.checked_sub(degree),
        DocumentBSplineForm::Periodic => Some(control_count),
    }
    .filter(|count| *count > 0)
    .ok_or(IntentMaterializationError::InvalidGeometry {
        node,
        reason: "spline control count must exceed its positive degree",
    })?;
    let span_count_u32 =
        u32::try_from(span_count).map_err(|_| IntentMaterializationError::InvalidGeometry {
            node,
            reason: "spline span count exceeds persistent limits",
        })?;
    let span_ids = (1..=span_count_u32).collect::<Vec<_>>();
    let next_span_id =
        span_count_u32
            .checked_add(1)
            .ok_or(IntentMaterializationError::InvalidGeometry {
                node,
                reason: "spline span identity high-water overflow",
            })?;
    let knots = match form {
        DocumentBSplineForm::Clamped => {
            let mut knots = vec![0.0; degree + 1];
            knots.extend((1..span_count_u32).map(f64::from));
            knots.extend(std::iter::repeat_n(f64::from(span_count_u32), degree + 1));
            knots
        }
        DocumentBSplineForm::Periodic => (0..=span_count_u32).map(f64::from).collect(),
    };
    Ok((knots, span_ids, next_span_id))
}

fn lower_control_spline(
    candidate: &dyn IntentMaterializationSource,
    node: &IntentNode,
    recipe: GeometryRecipeKind,
    batch: &mut SketchMaterializationBatch,
    state: &mut LoweringState,
) -> Result<(), IntentMaterializationError> {
    let count = node.child_order.len();
    let degree = u32::try_from(field_natural(node, "degree", 3)?).map_err(|_| {
        IntentMaterializationError::InvalidGeometry {
            node: node.id,
            reason: "spline degree exceeds persistent limits",
        }
    })?;
    let degree_usize =
        usize::try_from(degree).map_err(|_| IntentMaterializationError::InvalidGeometry {
            node: node.id,
            reason: "spline degree exceeds platform limits",
        })?;
    if degree == 0 || count <= degree_usize {
        return Err(IntentMaterializationError::InvalidGeometry {
            node: node.id,
            reason: "spline control count must exceed its positive degree",
        });
    }
    let rational = matches!(
        recipe,
        GeometryRecipeKind::OpenControlNurbs | GeometryRecipeKind::PeriodicControlNurbs
    );
    let gauge_index = if rational {
        let gauge_index =
            usize::try_from(field_natural(node, "gauge_index", 0)?).map_err(|_| {
                IntentMaterializationError::InvalidGeometry {
                    node: node.id,
                    reason: "NURBS gauge index exceeds platform limits",
                }
            })?;
        if gauge_index >= count {
            return Err(IntentMaterializationError::InvalidGeometry {
                node: node.id,
                reason: "NURBS gauge index is outside the control list",
            });
        }
        Some(gauge_index)
    } else {
        None
    };
    let mut controls = Vec::with_capacity(count);
    let mut weights = rational.then(|| Vec::with_capacity(count));
    for index in 0..count {
        let ordinal = u32::try_from(index).map_or(f64::MAX, f64::from);
        let point_port = child_port(node, index, IntentPortRole::Control)?;
        let (point, _) = materialize_point_port(
            candidate,
            node,
            point_port,
            [ordinal, if index % 2 == 0 { 0.0 } else { 1.0 }],
            &format!("control{}", index + 1),
            batch,
            state,
        )?;
        controls.push(point);
        if let Some(weights) = weights.as_mut() {
            let weight_port = child_port(node, index, IntentPortRole::Target)?;
            let (weight, value) = materialize_scalar_port(
                candidate,
                node,
                weight_port,
                LeafField::Weight,
                1.0,
                IntentUnit::Dimensionless,
                ScalarUnit::Parameter,
                ScalarDomain::Positive,
                &format!("weight{}", index + 1),
                batch,
                state,
            )?;
            if !(value.is_finite() && value > 0.0) {
                return Err(IntentMaterializationError::InvalidGeometry {
                    node: node.id,
                    reason: "NURBS weights must be finite and positive",
                });
            }
            if Some(index) == gauge_index && value.to_bits() != 1.0_f64.to_bits() {
                return Err(IntentMaterializationError::InvalidGeometry {
                    node: node.id,
                    reason: "the selected NURBS gauge weight must be exactly one",
                });
            }
            weights.push(weight);
        }
    }
    let form = match recipe {
        GeometryRecipeKind::OpenControlBSpline | GeometryRecipeKind::OpenControlNurbs => {
            DocumentBSplineForm::Clamped
        }
        GeometryRecipeKind::PeriodicControlBSpline | GeometryRecipeKind::PeriodicControlNurbs => {
            DocumentBSplineForm::Periodic
        }
        _ => unreachable!("spline lowering called with non-spline recipe"),
    };
    let (knots, span_ids, next_span_id) = spline_topology(node.id, form, degree, count)?;
    let logical_span_ids = span_ids.clone();
    let definition = if let Some(weights) = weights {
        let gauge_weight = weights[gauge_index.expect("rational spline has a gauge")];
        CurveDefinition::Nurbs {
            form,
            degree,
            controls,
            weights,
            gauge_weight,
            knots,
            span_ids,
            next_span_id,
        }
    } else {
        CurveDefinition::BSpline {
            form,
            degree,
            controls,
            knots,
            span_ids,
            next_span_id,
        }
    };
    materialize_spline_curve(node, definition, &logical_span_ids, batch, state)
}

#[allow(clippy::too_many_arguments)]
fn materialize_point(
    candidate: &dyn IntentMaterializationSource,
    node: &IntentNode,
    role: IntentPortRole,
    index: u16,
    default: [f64; 2],
    label: &str,
    batch: &mut SketchMaterializationBatch,
    state: &mut LoweringState,
) -> Result<(DesignPointId, [f64; 2]), IntentMaterializationError> {
    let port = require_port(node, role, index)?;
    let point = require_point_binding(node.id, state.port_bindings.get(&port.as_ref(node.id)))?;
    let position = resolve_point_position(candidate, node, port, point, default, state)?;
    if matches!(port.flow, IntentIdentityFlow::Created { .. }) {
        batch.push_point(DesignPoint {
            id: point,
            label: format!("{}.{}", node.symbol.as_str(), label),
            position,
        });
        state.point_positions.insert(point, position);
        state.consume_port(node, port);
    }
    Ok((point, position))
}

#[allow(clippy::too_many_arguments)]
fn materialize_scalar(
    candidate: &dyn IntentMaterializationSource,
    node: &IntentNode,
    index: u16,
    field: LeafField,
    default: f64,
    intent_unit: IntentUnit,
    unit: ScalarUnit,
    domain: ScalarDomain,
    label: &str,
    batch: &mut SketchMaterializationBatch,
    state: &mut LoweringState,
) -> Result<DesignScalarId, IntentMaterializationError> {
    let port = require_port(node, IntentPortRole::Target, index)?;
    let scalar = match state.port_bindings.get(&port.as_ref(node.id)) {
        Some(IntentNativeBinding::Scalar(scalar)) => *scalar,
        _ => return Err(IntentMaterializationError::NativeKindMismatch { node: node.id }),
    };
    if matches!(port.flow, IntentIdentityFlow::Created { .. }) {
        let value = leaf_quantity(candidate, node.id, port.id, field, default, intent_unit)?;
        batch.push_scalar(DesignScalar {
            id: scalar,
            label: format!("{}.{}", node.symbol.as_str(), label),
            value,
            unit,
            domain,
        });
        state.scalar_domains.insert(scalar, domain);
        state.consume_port(node, port);
    }
    Ok(scalar)
}

fn materialize_curve(
    node: &IntentNode,
    index: u16,
    definition: CurveDefinition,
    batch: &mut SketchMaterializationBatch,
    state: &mut LoweringState,
) -> Result<(), IntentMaterializationError> {
    materialize_curve_with_role(node, index, definition, geometry_role(node)?, batch, state)
}

fn curve_binding(
    node: &IntentNode,
    index: u16,
    state: &LoweringState,
) -> Result<CurveId, IntentMaterializationError> {
    let curve_port = require_port(node, IntentPortRole::Curve, index)?;
    match state.port_bindings.get(&curve_port.as_ref(node.id)) {
        Some(IntentNativeBinding::Curve(curve)) => Ok(*curve),
        _ => Err(IntentMaterializationError::NativeKindMismatch { node: node.id }),
    }
}

fn materialize_curve_with_role(
    node: &IntentNode,
    index: u16,
    definition: CurveDefinition,
    role: GeometryRole,
    batch: &mut SketchMaterializationBatch,
    state: &mut LoweringState,
) -> Result<(), IntentMaterializationError> {
    let curve_port = require_port(node, IntentPortRole::Curve, index)?;
    let curve = curve_binding(node, index, state)?;
    let primary_segment = match &definition {
        CurveDefinition::BSpline { span_ids, .. } | CurveDefinition::Nurbs { span_ids, .. } => {
            span_ids
                .first()
                .copied()
                .ok_or(IntentMaterializationError::InvalidGeometry {
                    node: node.id,
                    reason: "spline topology has no semantic span",
                })?
        }
        _ => 0,
    };
    batch.push_curve(DesignCurve {
        id: curve,
        label: if index == 0 {
            node.symbol.as_str().to_owned()
        } else {
            format!("{}.curve{}", node.symbol.as_str(), index + 1)
        },
        definition: definition.clone(),
    });
    state.curve_definitions.insert(curve, definition);
    if role == GeometryRole::Construction {
        batch.push_geometry_role(GeometryRoleEdit::new(curve, role));
    }
    state.consume_port(node, curve_port);
    if let Some(span_port) = node.port_by_selector(IntentPortSelector::Node {
        role: IntentPortRole::Span,
        index,
    }) {
        state.port_bindings.insert(
            span_port.as_ref(node.id),
            IntentNativeBinding::CurveSpan(CurveSpan {
                curve,
                segment: primary_segment,
            }),
        );
    }
    Ok(())
}

fn materialize_spline_curve(
    node: &IntentNode,
    definition: CurveDefinition,
    span_ids: &[u32],
    batch: &mut SketchMaterializationBatch,
    state: &mut LoweringState,
) -> Result<(), IntentMaterializationError> {
    materialize_curve(node, 0, definition, batch, state)?;
    let curve = curve_binding(node, 0, state)?;
    for (index, segment) in span_ids.iter().copied().enumerate() {
        let index =
            u16::try_from(index).map_err(|_| IntentMaterializationError::InvalidGeometry {
                node: node.id,
                reason: "spline logical span index exceeds persistent limits",
            })?;
        let port = require_port(node, IntentPortRole::Span, index)?;
        state.port_bindings.insert(
            port.as_ref(node.id),
            IntentNativeBinding::CurveSpan(CurveSpan { curve, segment }),
        );
    }
    Ok(())
}

fn lower_segment(
    candidate: &dyn IntentMaterializationSource,
    node: &IntentNode,
    batch: &mut SketchMaterializationBatch,
    state: &mut LoweringState,
) -> Result<(), IntentMaterializationError> {
    let start_port = require_port(node, IntentPortRole::Start, 0)?;
    let end_port = require_port(node, IntentPortRole::End, 0)?;
    let start = require_point_binding(
        node.id,
        state.port_bindings.get(&start_port.as_ref(node.id)),
    )?;
    let end = require_point_binding(node.id, state.port_bindings.get(&end_port.as_ref(node.id)))?;
    let start_position =
        resolve_point_position(candidate, node, start_port, start, [0.0, 0.0], state)?;
    let end_position = resolve_point_position(candidate, node, end_port, end, [1.0, 0.0], state)?;
    for (port, point, position, suffix) in [
        (start_port, start, start_position, "start"),
        (end_port, end, end_position, "end"),
    ] {
        if matches!(port.flow, IntentIdentityFlow::Created { .. }) {
            batch.push_point(DesignPoint {
                id: point,
                label: format!("{}.{}", node.symbol.as_str(), suffix),
                position,
            });
            state.point_positions.insert(point, position);
            state.consume_port(node, port);
        }
    }
    let curve_port = require_port(node, IntentPortRole::Curve, 0)?;
    let curve = match state.port_bindings.get(&curve_port.as_ref(node.id)) {
        Some(IntentNativeBinding::Curve(curve)) => *curve,
        _ => return Err(IntentMaterializationError::NativeKindMismatch { node: node.id }),
    };
    let branch_direction = branch_direction(node, start_position, end_position)?;
    let definition = CurveDefinition::Line {
        start,
        end,
        branch_direction,
    };
    batch.push_curve(DesignCurve {
        id: curve,
        label: node.symbol.as_str().to_owned(),
        definition: definition.clone(),
    });
    state.curve_definitions.insert(curve, definition);
    let role = geometry_role(node)?;
    if role == GeometryRole::Construction {
        batch.push_geometry_role(GeometryRoleEdit::new(curve, role));
    }
    state.consume_port(node, curve_port);
    let span_port = require_port(node, IntentPortRole::Span, 0)?;
    state.port_bindings.insert(
        span_port.as_ref(node.id),
        IntentNativeBinding::CurveSpan(CurveSpan::line(curve)),
    );
    Ok(())
}

fn lower_aggregate(
    node: &IntentNode,
    kind: AggregateKind,
    state: &mut LoweringState,
) -> Result<(), IntentMaterializationError> {
    let spans = (0..node.inputs.len())
        .map(|index| {
            let index =
                u16::try_from(index).map_err(|_| IntentMaterializationError::InvalidAggregate {
                    node: node.id,
                    reason: "aggregate input count exceeds persistent limits",
                })?;
            input_span(node, state, index)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let (role, closed) = match kind {
        AggregateKind::OpenChain => (IntentPortRole::Chain, false),
        AggregateKind::ClosedProfile => (IntentPortRole::Profile, true),
    };
    let port = require_port(node, role, 0)?;
    state.bind_aggregate(node, port, spans, closed, true);
    Ok(())
}

trait AggregateEndpointTopology {
    fn span_is_periodic(&self, span: CurveSpan) -> Option<bool>;
    fn contains_endpoint(&self, endpoint: OffsetEndpointRef) -> bool;
    fn endpoints_are_adjacent(&self, first: OffsetEndpointRef, second: OffsetEndpointRef) -> bool;
}

impl AggregateEndpointTopology for EndpointTopologyIndex {
    fn span_is_periodic(&self, span: CurveSpan) -> Option<bool> {
        self.span(span).map(|candidate| candidate.periodic)
    }

    fn contains_endpoint(&self, endpoint: OffsetEndpointRef) -> bool {
        self.span(endpoint.span).is_some_and(|candidate| {
            candidate
                .endpoints
                .iter()
                .any(|candidate| candidate.endpoint == endpoint)
        })
    }

    fn endpoints_are_adjacent(&self, first: OffsetEndpointRef, second: OffsetEndpointRef) -> bool {
        self.adjacent_endpoints(first)
            .any(|candidate| candidate == second)
    }
}

impl AggregateEndpointTopology for OffsetOperandIndex {
    fn span_is_periodic(&self, span: CurveSpan) -> Option<bool> {
        self.span(span).map(|candidate| candidate.periodic)
    }

    fn contains_endpoint(&self, endpoint: OffsetEndpointRef) -> bool {
        self.span(endpoint.span).is_some_and(|candidate| {
            candidate
                .endpoints
                .iter()
                .any(|candidate| candidate.endpoint == endpoint)
        })
    }

    fn endpoints_are_adjacent(&self, first: OffsetEndpointRef, second: OffsetEndpointRef) -> bool {
        self.adjacent_endpoints(first)
            .any(|candidate| candidate == second)
    }
}

fn endpoint_pair<I: AggregateEndpointTopology>(
    node: NodeId,
    span: CurveSpan,
    index: &I,
) -> Result<[OffsetEndpointRef; 2], IntentMaterializationError> {
    if index.span_is_periodic(span).is_none() {
        return Err(IntentMaterializationError::InvalidAggregate {
            node,
            reason: "aggregate span is absent from accepted topology",
        });
    }
    let start = OffsetEndpointRef {
        span,
        endpoint: OffsetEndpointRole::Start,
    };
    let end = OffsetEndpointRef {
        span,
        endpoint: OffsetEndpointRole::End,
    };
    if !index.contains_endpoint(start) || !index.contains_endpoint(end) {
        return Err(IntentMaterializationError::InvalidAggregate {
            node,
            reason: "aggregate span lacks topology-owned bounded endpoints",
        });
    }
    Ok([start, end])
}

fn endpoints_connected<I: AggregateEndpointTopology>(
    index: &I,
    first: OffsetEndpointRef,
    second: OffsetEndpointRef,
) -> bool {
    first == second || index.endpoints_are_adjacent(first, second)
}

fn validate_aggregate_topology<I: AggregateEndpointTopology>(
    node: NodeId,
    aggregate: &IntentAggregateMaterialization,
    index: &I,
) -> Result<(), IntentMaterializationError> {
    if aggregate.spans.is_empty() {
        return Err(IntentMaterializationError::InvalidAggregate {
            node,
            reason: "aggregate must contain at least one span",
        });
    }
    let unique = aggregate.spans.iter().copied().collect::<BTreeSet<_>>();
    if unique.len() != aggregate.spans.len() {
        return Err(IntentMaterializationError::InvalidAggregate {
            node,
            reason: "aggregate must not repeat a native span",
        });
    }
    let periodic = aggregate
        .spans
        .iter()
        .filter(|span| index.span_is_periodic(**span) == Some(true))
        .count();
    if periodic > 0 {
        if aggregate.closed && aggregate.spans.len() == 1 && periodic == 1 {
            return Ok(());
        }
        return Err(IntentMaterializationError::InvalidAggregate {
            node,
            reason: "a periodic curve must be the sole span of a closed profile",
        });
    }

    let pairs = aggregate
        .spans
        .iter()
        .copied()
        .map(|span| endpoint_pair(node, span, index))
        .collect::<Result<Vec<_>, _>>()?;
    let first = pairs[0];
    let mut states = BTreeSet::from([(first[0], first[1]), (first[1], first[0])]);
    for pair in pairs.iter().skip(1) {
        let orientations = [(pair[0], pair[1]), (pair[1], pair[0])];
        let mut next_states = BTreeSet::new();
        for (path_start, path_end) in &states {
            for (next_start, next_end) in orientations {
                if endpoints_connected(index, *path_end, next_start) {
                    next_states.insert((*path_start, next_end));
                }
            }
        }
        if next_states.is_empty() {
            return Err(IntentMaterializationError::InvalidAggregate {
                node,
                reason: "ordered aggregate spans are not continuously connected",
            });
        }
        states = next_states;
    }
    let valid = states.into_iter().any(|(start, end)| {
        let closes = endpoints_connected(index, end, start);
        if aggregate.closed { closes } else { !closes }
    });
    if !valid {
        return Err(IntentMaterializationError::InvalidAggregate {
            node,
            reason: if aggregate.closed {
                "closed profile spans do not form a topology-owned cycle"
            } else {
                "open chain terminals are not distinct"
            },
        });
    }
    Ok(())
}

#[allow(
    clippy::too_many_lines,
    reason = "one exhaustive adapter keeps the closed existing operation request catalog auditable"
)]
fn operation_request(
    node: &IntentNode,
    operation: IntentOperationKind,
    session: &RetainedSketchDocumentSession,
    state: &LoweringState,
) -> Result<SketchOperationRequest, IntentMaterializationError> {
    use IntentOperationKind as O;
    let label = node.symbol.as_str().to_owned();
    Ok(match operation {
        O::Split => SketchOperationRequest::Split {
            support: input_span(node, state, 0)?,
            parameter: required_quantity(node, "parameter", IntentUnit::Dimensionless)?,
            retained: split_retained_piece(node, "retained")?,
        },
        O::Break => SketchOperationRequest::Break {
            support: input_span(node, state, 0)?,
            start: required_quantity(node, "start", IntentUnit::Dimensionless)?,
            end: required_quantity(node, "end", IntentUnit::Dimensionless)?,
            retained: split_retained_piece(node, "retained")?,
        },
        O::Trim => SketchOperationRequest::Trim {
            support: input_span(node, state, 0)?,
            parameter: required_quantity(node, "parameter", IntentUnit::Dimensionless)?,
            retained: trim_retained_side(node, "retained")?,
        },
        O::Extend => SketchOperationRequest::ExtendLineToLine {
            line: input_span(node, state, 0)?,
            endpoint: line_endpoint(node, "endpoint")?,
            target: input_span(node, state, 1)?,
        },
        O::Mirror => SketchOperationRequest::Mirror {
            label,
            source: input_curve(node, state, 0)?,
            axis: input_binding(node, state, InputRole::Span, 0).and_then(
                |binding| match binding {
                    IntentNativeBinding::CurveSpan(span) => Ok(*span),
                    IntentNativeBinding::Curve(curve) => Ok(CurveSpan::line(*curve)),
                    _ => Err(IntentMaterializationError::NativeKindMismatch { node: node.id }),
                },
            )?,
        },
        O::Chamfer => SketchOperationRequest::Chamfer {
            label,
            first: input_span(node, state, 0)?,
            second: input_span(node, state, 1)?,
            first_distance: required_quantity(node, "first_distance", IntentUnit::Length)?,
            second_distance: required_quantity(node, "second_distance", IntentUnit::Length)?,
        },
        O::AssociativeFillet => SketchOperationRequest::AssociativeFillet {
            label,
            request: CurveCurveFilletRequest {
                first: operation_fillet_parent(node, state, "first", 0)?,
                second: operation_fillet_parent(node, state, "second", 1)?,
                endpoint_order: required_fillet_endpoint_order(node)?,
                sweep: required_arc_sweep(node)?,
                radius: required_quantity(node, "radius", IntentUnit::Length)?,
                radius_mode: required_dimension_mode(node, "radius_mode")?,
            },
        },
        O::Rectangle => SketchOperationRequest::Rectangle {
            label,
            origin: required_point(node, "origin")?,
            width: required_quantity(node, "width", IntentUnit::Length)?,
            height: required_quantity(node, "height", IntentUnit::Length)?,
        },
        O::RegularPolygon => SketchOperationRequest::RegularPolygon {
            label,
            center: required_point(node, "center")?,
            radius: required_quantity(node, "radius", IntentUnit::Length)?,
            sides: required_usize(node, "sides")?,
            rotation: required_quantity(node, "rotation", IntentUnit::Angle)?,
        },
        O::Slot => SketchOperationRequest::Slot {
            label,
            first_center: required_point(node, "first_center")?,
            second_center: required_point(node, "second_center")?,
            radius: required_quantity(node, "radius", IntentUnit::Length)?,
        },
        O::LinearPattern => {
            let source_count = node
                .inputs
                .keys()
                .filter(|slot| slot.role == InputRole::Curve)
                .count();
            let sources = (0..source_count)
                .map(|index| {
                    u16::try_from(index)
                        .map_err(|_| {
                            invalid_operation(node, "linear-pattern source count exceeds limits")
                        })
                        .and_then(|index| input_curve(node, state, index))
                })
                .collect::<Result<Vec<_>, _>>()?;
            SketchOperationRequest::LinearPattern {
                label,
                sources,
                instances: required_usize(node, "instances")?,
                step: required_point(node, "step")?,
            }
        }
        O::ProfileOffset => {
            let (operand, operand_index) = operation_profile_offset_operand(node, session, state)?;
            SketchOperationRequest::ProfileOffset {
                label,
                distance: required_quantity(node, "distance", IntentUnit::Length)?,
                operand,
                operand_index,
            }
        }
    })
}

fn lower_operation(
    _candidate: &dyn IntentMaterializationSource,
    node: &IntentNode,
    operation: IntentOperationKind,
    session: &mut RetainedSketchDocumentSession,
    state: &mut LoweringState,
) -> Result<(), IntentMaterializationError> {
    let proposal = prepare_operation_proposal(node, operation, session, state)?;
    let reservations = operation_output_reservations(node, state)?;
    let claimed = node
        .operation_outputs
        .iter()
        .map(|output| operation_output_native_kind(output.kind))
        .collect::<Vec<_>>();
    let authenticated = proposal
        .output_plan()
        .slots
        .iter()
        .map(|slot| slot.kind)
        .collect::<Vec<_>>();
    if claimed != authenticated {
        return Err(invalid_operation(
            node,
            "persisted output shape disagrees with native proposal",
        ));
    }
    if node
        .operation_outputs
        .iter()
        .zip(&proposal.output_plan().slots)
        .any(|(claimed, authenticated)| {
            claimed.kind == IntentOperationOutputKind::Curve
                && claimed.curve_span_count != authenticated.curve_span_count
        })
    {
        return Err(invalid_operation(
            node,
            "persisted curve-span shape disagrees with native proposal",
        ));
    }
    let outcome = proposal
        .apply_with_reserved_outputs(session, &reservations)
        .map_err(|_| invalid_operation(node, "reserved native operation application failed"))?;
    if outcome.published_accepted_identity().is_none() {
        return Err(IntentMaterializationError::SolverRejected);
    }
    require_current_acceptance(session)?;
    bind_operation_outputs(node, session.design_document(), state)
}

fn prepare_operation_proposal(
    node: &IntentNode,
    operation: IntentOperationKind,
    session: &RetainedSketchDocumentSession,
    state: &LoweringState,
) -> Result<Box<SketchOperationProposal>, IntentMaterializationError> {
    let request = operation_request(node, operation, session, state)?;
    let source_free = match operation {
        IntentOperationKind::Rectangle
        | IntentOperationKind::RegularPolygon
        | IntentOperationKind::Slot => required_geometry_role(node)?,
        _ => GeometryRole::Profile,
    };
    let prepared = SketchOperationSnapshot::capture(session)
        .prepare_with_geometry_role(request, source_free)
        .execute(OperationControl::unlimited())
        .map_err(|_| invalid_operation(node, "native operation preparation failed"))?;
    let result = match prepared {
        OperationOutcome::Completed { value, .. } => value,
        OperationOutcome::Cancelled { .. } | OperationOutcome::WorkExhausted { .. } => {
            return Err(invalid_operation(
                node,
                "native operation preparation did not complete",
            ));
        }
        _ => {
            return Err(invalid_operation(
                node,
                "native operation returned an unknown outcome",
            ));
        }
    };
    let proposal = match result {
        SketchOperationResult::Proposed(proposal) => proposal,
        SketchOperationResult::Unsupported(_) => {
            return Err(invalid_operation(
                node,
                "native operation is unsupported for the exact accepted prefix",
            ));
        }
        SketchOperationResult::Incomplete(_) => {
            return Err(invalid_operation(
                node,
                "native operation is incomplete for the exact accepted prefix",
            ));
        }
        _ => {
            return Err(invalid_operation(
                node,
                "native operation returned an unknown proposal state",
            ));
        }
    };
    if proposal.output_plan().kind != sketch_operation_kind(operation) {
        return Err(invalid_operation(
            node,
            "native operation kind authentication failed",
        ));
    }
    Ok(proposal)
}

const fn sketch_operation_kind(operation: IntentOperationKind) -> SketchOperationKind {
    match operation {
        IntentOperationKind::Split => SketchOperationKind::Split,
        IntentOperationKind::Break => SketchOperationKind::Break,
        IntentOperationKind::Trim => SketchOperationKind::Trim,
        IntentOperationKind::Extend => SketchOperationKind::Extend,
        IntentOperationKind::Mirror => SketchOperationKind::Mirror,
        IntentOperationKind::Chamfer => SketchOperationKind::Chamfer,
        IntentOperationKind::AssociativeFillet => SketchOperationKind::AssociativeFillet,
        IntentOperationKind::Rectangle => SketchOperationKind::Rectangle,
        IntentOperationKind::RegularPolygon => SketchOperationKind::RegularPolygon,
        IntentOperationKind::Slot => SketchOperationKind::Slot,
        IntentOperationKind::LinearPattern => SketchOperationKind::LinearPattern,
        IntentOperationKind::ProfileOffset => SketchOperationKind::ProfileOffset,
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "native output kinds, source-relative paths, and semantic roles are authenticated together"
)]
fn prepared_intent_operation_plan(
    graph: &IntentGraph,
    node: &IntentNode,
    operation: IntentOperationKind,
    state: &LoweringState,
    plan: &SketchOperationOutputPlan,
) -> Result<PreparedIntentOperationPlan, IntentMaterializationError> {
    if plan.kind != sketch_operation_kind(operation) {
        return Err(invalid_operation(
            node,
            "native operation kind authentication failed",
        ));
    }
    let mut outputs = Vec::with_capacity(plan.slots.len());
    for (ordinal, slot) in plan.slots.iter().enumerate() {
        if slot.ordinal != ordinal {
            return Err(invalid_operation(
                node,
                "native operation output order is not canonical",
            ));
        }
        let output = match slot.kind {
            SketchMaterializationIdentityKind::Point => {
                IntentOperationOutput::native(IntentOperationOutputKind::Point)
            }
            SketchMaterializationIdentityKind::Scalar => {
                IntentOperationOutput::native(IntentOperationOutputKind::Scalar)
            }
            SketchMaterializationIdentityKind::Curve => {
                IntentOperationOutput::curve(slot.curve_span_count)
            }
            SketchMaterializationIdentityKind::Contact => {
                IntentOperationOutput::native(IntentOperationOutputKind::Contact)
            }
            SketchMaterializationIdentityKind::Constraint => {
                IntentOperationOutput::native(IntentOperationOutputKind::Constraint)
            }
            SketchMaterializationIdentityKind::Dimension => {
                IntentOperationOutput::native(IntentOperationOutputKind::Dimension)
            }
            SketchMaterializationIdentityKind::Parameter => {
                IntentOperationOutput::native(IntentOperationOutputKind::Parameter)
            }
            SketchMaterializationIdentityKind::ExternalBinding => {
                IntentOperationOutput::native(IntentOperationOutputKind::ExternalBinding)
            }
            _ => {
                return Err(invalid_operation(
                    node,
                    "native operation produced an unsupported output kind",
                ));
            }
        };
        let mut path = Vec::with_capacity(slot.path.len());
        for segment in &slot.path {
            path.push(match *segment {
                SketchOperationOutputPathSegment::Field(name) => {
                    PreparedIntentOperationPathSegment::Field(name.to_owned())
                }
                SketchOperationOutputPathSegment::Index(index) => {
                    PreparedIntentOperationPathSegment::Index(index)
                }
                SketchOperationOutputPathSegment::SourceControl { point, .. } => {
                    PreparedIntentOperationPathSegment::SourceControl {
                        source: operation_source_reference(
                            graph,
                            node,
                            state,
                            IntentNativeBinding::Point(point),
                        )?,
                    }
                }
                SketchOperationOutputPathSegment::SourceCurve(curve) => {
                    PreparedIntentOperationPathSegment::SourceCurve {
                        source: operation_source_reference(
                            graph,
                            node,
                            state,
                            IntentNativeBinding::Curve(curve),
                        )?,
                    }
                }
                SketchOperationOutputPathSegment::SourceSpan(span) => {
                    PreparedIntentOperationPathSegment::SourceSpan {
                        source: operation_source_reference(
                            graph,
                            node,
                            state,
                            IntentNativeBinding::CurveSpan(span),
                        )?,
                    }
                }
                SketchOperationOutputPathSegment::SourceJunction(owner) => {
                    let binding = match owner {
                        DocumentProfileOffsetJunctionOwner::SharedPoint(point) => {
                            IntentNativeBinding::Point(point)
                        }
                        DocumentProfileOffsetJunctionOwner::Constraint(constraint) => {
                            IntentNativeBinding::Constraint(constraint)
                        }
                    };
                    PreparedIntentOperationPathSegment::SourceJunction {
                        source: operation_source_reference(graph, node, state, binding)?,
                    }
                }
                _ => {
                    return Err(invalid_operation(
                        node,
                        "native operation produced an unsupported path segment",
                    ));
                }
            });
        }
        let role = match slot.role {
            SketchOperationOutputRole::Geometry => PreparedIntentOperationOutputRole::Geometry,
            SketchOperationOutputRole::ContactParameter => {
                PreparedIntentOperationOutputRole::ContactParameter
            }
            SketchOperationOutputRole::Contact => PreparedIntentOperationOutputRole::Contact,
            SketchOperationOutputRole::Constraint => PreparedIntentOperationOutputRole::Constraint,
            SketchOperationOutputRole::DimensionTarget => {
                PreparedIntentOperationOutputRole::DimensionTarget
            }
            SketchOperationOutputRole::Dimension => PreparedIntentOperationOutputRole::Dimension,
            SketchOperationOutputRole::Parameter => PreparedIntentOperationOutputRole::Parameter,
            SketchOperationOutputRole::ExternalBinding => {
                PreparedIntentOperationOutputRole::ExternalBinding
            }
            _ => {
                return Err(invalid_operation(
                    node,
                    "native operation produced an unsupported semantic role",
                ));
            }
        };
        if !role.accepts_output_kind(output.kind) {
            return Err(invalid_operation(
                node,
                "native operation semantic role disagrees with its output kind",
            ));
        }
        outputs.push(PreparedIntentOperationOutput { output, path, role });
    }
    Ok(PreparedIntentOperationPlan { operation, outputs })
}

fn operation_source_reference(
    graph: &IntentGraph,
    node: &IntentNode,
    state: &LoweringState,
    binding: IntentNativeBinding,
) -> Result<PreparedIntentOperationSourceRef, IntentMaterializationError> {
    // Logical span ports do not own a native reservation. Anchor their lookup
    // to the exact declaration which owns the underlying curve reservation so
    // an unrelated alias of the same span cannot rename a result member.
    let owner_binding = match binding {
        IntentNativeBinding::CurveSpan(span) => IntentNativeBinding::Curve(span.curve),
        binding => binding,
    };
    let owner_nodes = state
        .port_bindings
        .iter()
        .filter_map(|(reference, candidate)| {
            if *candidate != owner_binding {
                return None;
            }
            let source = graph.node(reference.node)?;
            let port = source.port(reference.port)?;
            matches!(port.flow, IntentIdentityFlow::Created { .. }).then_some(reference.node)
        })
        .collect::<BTreeSet<_>>();
    let preferred_owner = (owner_nodes.len() == 1)
        .then(|| owner_nodes.iter().next().copied())
        .flatten();

    let mut candidates = state
        .port_bindings
        .iter()
        .filter_map(|(reference, candidate)| {
            if *candidate != binding || preferred_owner.is_some_and(|owner| reference.node != owner)
            {
                return None;
            }
            let source = graph.node(reference.node)?;
            let port = source.port(reference.port)?;
            Some(PreparedIntentOperationSourceRef {
                declaration: source.symbol.clone(),
                selector: port.selector,
                kind: port.kind,
            })
        })
        .collect::<Vec<_>>();
    candidates.sort_unstable_by(|left, right| {
        (&left.declaration, left.selector, left.kind).cmp(&(
            &right.declaration,
            right.selector,
            right.kind,
        ))
    });
    candidates.dedup();
    match candidates.as_slice() {
        [source] => Ok(source.clone()),
        [] => Err(invalid_operation(
            node,
            "native operation path names an object without a semantic source port",
        )),
        _ => Err(invalid_operation(
            node,
            "native operation path names an ambiguously aliased semantic source port",
        )),
    }
}

impl PreparedIntentOperationOutputRole {
    /// Whether this semantic role accepts one independently authenticated
    /// native operation-output kind.
    #[must_use]
    pub const fn accepts_output_kind(self, kind: IntentOperationOutputKind) -> bool {
        match self {
            Self::Geometry => matches!(
                kind,
                IntentOperationOutputKind::Point
                    | IntentOperationOutputKind::Scalar
                    | IntentOperationOutputKind::Curve
            ),
            Self::ContactParameter | Self::DimensionTarget => {
                matches!(kind, IntentOperationOutputKind::Scalar)
            }
            Self::Contact => matches!(kind, IntentOperationOutputKind::Contact),
            Self::Constraint => matches!(kind, IntentOperationOutputKind::Constraint),
            Self::Dimension => matches!(kind, IntentOperationOutputKind::Dimension),
            Self::Parameter => matches!(kind, IntentOperationOutputKind::Parameter),
            Self::ExternalBinding => {
                matches!(kind, IntentOperationOutputKind::ExternalBinding)
            }
        }
    }
}

const fn operation_output_native_kind(
    kind: IntentOperationOutputKind,
) -> SketchMaterializationIdentityKind {
    match kind {
        IntentOperationOutputKind::Point => SketchMaterializationIdentityKind::Point,
        IntentOperationOutputKind::Scalar => SketchMaterializationIdentityKind::Scalar,
        IntentOperationOutputKind::Curve => SketchMaterializationIdentityKind::Curve,
        IntentOperationOutputKind::Contact => SketchMaterializationIdentityKind::Contact,
        IntentOperationOutputKind::Constraint => SketchMaterializationIdentityKind::Constraint,
        IntentOperationOutputKind::Dimension => SketchMaterializationIdentityKind::Dimension,
        IntentOperationOutputKind::Parameter => SketchMaterializationIdentityKind::Parameter,
        IntentOperationOutputKind::ExternalBinding => {
            SketchMaterializationIdentityKind::ExternalBinding
        }
    }
}

fn operation_output_reservations(
    node: &IntentNode,
    state: &LoweringState,
) -> Result<Vec<SketchMaterializationIdentityReservation>, IntentMaterializationError> {
    node.operation_outputs
        .iter()
        .enumerate()
        .map(|(ordinal, output)| {
            let index = u16::try_from(ordinal)
                .map_err(|_| invalid_operation(node, "operation output count exceeds limits"))?;
            let result = require_port(node, IntentPortRole::Result, index)?;
            let binding = state
                .port_bindings
                .get(&result.as_ref(node.id))
                .copied()
                .ok_or(IntentMaterializationError::NativeKindMismatch { node: node.id })?;
            match (output.kind, binding) {
                (IntentOperationOutputKind::Point, IntentNativeBinding::Point(id)) => {
                    Ok(SketchMaterializationIdentityReservation::Point { id })
                }
                (IntentOperationOutputKind::Scalar, IntentNativeBinding::Scalar(id)) => {
                    Ok(SketchMaterializationIdentityReservation::Scalar { id })
                }
                (IntentOperationOutputKind::Curve, IntentNativeBinding::Curve(id)) => {
                    Ok(SketchMaterializationIdentityReservation::Curve { id })
                }
                (IntentOperationOutputKind::Contact, IntentNativeBinding::Contact(id)) => {
                    Ok(SketchMaterializationIdentityReservation::Contact { id })
                }
                (IntentOperationOutputKind::Constraint, IntentNativeBinding::Constraint(id)) => {
                    let source = require_port(node, IntentPortRole::Source, index)?;
                    let Some(IntentNativeBinding::Source(source)) =
                        state.port_bindings.get(&source.as_ref(node.id)).copied()
                    else {
                        return Err(IntentMaterializationError::NativeKindMismatch {
                            node: node.id,
                        });
                    };
                    Ok(SketchMaterializationIdentityReservation::Constraint {
                        reservation: SketchMaterializationConstraintReservation {
                            constraint: id,
                            source,
                        },
                    })
                }
                (IntentOperationOutputKind::Dimension, IntentNativeBinding::Dimension(id)) => {
                    let source = require_port(node, IntentPortRole::Source, index)?;
                    let Some(IntentNativeBinding::Source(source)) =
                        state.port_bindings.get(&source.as_ref(node.id)).copied()
                    else {
                        return Err(IntentMaterializationError::NativeKindMismatch {
                            node: node.id,
                        });
                    };
                    Ok(SketchMaterializationIdentityReservation::Dimension {
                        reservation: SketchMaterializationDimensionReservation {
                            dimension: id,
                            source,
                        },
                    })
                }
                (IntentOperationOutputKind::Parameter, IntentNativeBinding::Parameter(id)) => {
                    Ok(SketchMaterializationIdentityReservation::Parameter { id })
                }
                (
                    IntentOperationOutputKind::ExternalBinding,
                    IntentNativeBinding::ExternalBinding(id),
                ) => Ok(SketchMaterializationIdentityReservation::ExternalBinding { id }),
                _ => Err(IntentMaterializationError::NativeKindMismatch { node: node.id }),
            }
        })
        .collect()
}

fn bind_operation_outputs(
    node: &IntentNode,
    document: &geosolve_sketch::SketchDocument,
    state: &mut LoweringState,
) -> Result<(), IntentMaterializationError> {
    let mut span_index = 0_u16;
    for (ordinal, output) in node.operation_outputs.iter().enumerate() {
        let index = u16::try_from(ordinal)
            .map_err(|_| invalid_operation(node, "operation output count exceeds limits"))?;
        let port = require_port(node, IntentPortRole::Result, index)?;
        let binding = state
            .port_bindings
            .get(&port.as_ref(node.id))
            .copied()
            .ok_or(IntentMaterializationError::NativeKindMismatch { node: node.id })?;
        match (output.kind, binding) {
            (IntentOperationOutputKind::Point, IntentNativeBinding::Point(id)) => {
                let point = document.point(id).ok_or_else(|| {
                    invalid_operation(node, "reserved point output was not materialized")
                })?;
                state.point_positions.insert(id, point.position);
            }
            (IntentOperationOutputKind::Scalar, IntentNativeBinding::Scalar(id)) => {
                let scalar = document.scalar(id).ok_or_else(|| {
                    invalid_operation(node, "reserved scalar output was not materialized")
                })?;
                state.scalar_domains.insert(id, scalar.domain);
            }
            (IntentOperationOutputKind::Curve, IntentNativeBinding::Curve(id)) => {
                let curve = document.curve(id).ok_or_else(|| {
                    invalid_operation(node, "reserved curve output was not materialized")
                })?;
                state.curve_definitions.insert(id, curve.definition.clone());
                let spans = document.curve_spans(id)?;
                if spans.len() != usize::from(output.curve_span_count) {
                    return Err(invalid_operation(
                        node,
                        "materialized curve span count disagrees with persisted output shape",
                    ));
                }
                for span in spans {
                    let span_port = require_port(node, IntentPortRole::Span, span_index)?;
                    state.port_bindings.insert(
                        span_port.as_ref(node.id),
                        IntentNativeBinding::CurveSpan(span),
                    );
                    span_index = span_index
                        .checked_add(1)
                        .ok_or_else(|| invalid_operation(node, "operation span count overflow"))?;
                }
            }
            (IntentOperationOutputKind::Contact, IntentNativeBinding::Contact(id)) => {
                let contact = document.contact(id).ok_or_else(|| {
                    invalid_operation(node, "reserved contact output was not materialized")
                })?;
                state
                    .contact_states
                    .insert(id, (contact.curve, contact.winding, contact.domain));
            }
            (IntentOperationOutputKind::Constraint, IntentNativeBinding::Constraint(id)) => {
                document.constraint(id).ok_or_else(|| {
                    invalid_operation(node, "reserved constraint output was not materialized")
                })?;
            }
            (IntentOperationOutputKind::Dimension, IntentNativeBinding::Dimension(id)) => {
                document.dimension(id).ok_or_else(|| {
                    invalid_operation(node, "reserved dimension output was not materialized")
                })?;
            }
            (IntentOperationOutputKind::Parameter, IntentNativeBinding::Parameter(id)) => {
                let parameter = document.parameter(id).ok_or_else(|| {
                    invalid_operation(node, "reserved parameter output was not materialized")
                })?;
                state.parameter_kinds.insert(id, parameter.kind);
            }
            (
                IntentOperationOutputKind::ExternalBinding,
                IntentNativeBinding::ExternalBinding(id),
            ) => {
                document.external_binding(id).ok_or_else(|| {
                    invalid_operation(node, "reserved external output was not materialized")
                })?;
            }
            _ => return Err(IntentMaterializationError::NativeKindMismatch { node: node.id }),
        }
        state.consume_port(node, port);
    }
    Ok(())
}

fn operation_fillet_parent(
    node: &IntentNode,
    state: &LoweringState,
    prefix: &'static str,
    index: u16,
) -> Result<CurveFilletParentRequest, IntentMaterializationError> {
    let parameter_name = format!("{prefix}_parameter");
    let winding_name = format!("{prefix}_winding");
    let neighborhood_name = format!("{prefix}_neighborhood");
    let lower_name = format!("{prefix}_local_lower");
    let upper_name = format!("{prefix}_local_upper");
    let side_name = format!("{prefix}_normal_side");
    let endpoint_name = format!("{prefix}_trim_endpoint");
    let periodic_name = format!("{prefix}_periodic_anchor");
    let anchor_parameter_name = format!("{prefix}_anchor_parameter");
    let anchor_winding_name = format!("{prefix}_anchor_winding");
    let parameter = required_quantity(node, &parameter_name, IntentUnit::Dimensionless)?;
    let neighborhood = match required_enum(node, &neighborhood_name)? {
        "interior" => {
            require_absent(node, &[&lower_name, &upper_name])?;
            ContactNeighborhood::Interior
        }
        "local" => ContactNeighborhood::Local {
            lower: required_quantity(node, &lower_name, IntentUnit::Dimensionless)?,
            upper: required_quantity(node, &upper_name, IntentUnit::Dimensionless)?,
        },
        "start" => {
            require_absent(node, &[&lower_name, &upper_name])?;
            ContactNeighborhood::Start
        }
        "end" => {
            require_absent(node, &[&lower_name, &upper_name])?;
            ContactNeighborhood::End
        }
        _ => return Err(invalid_operation(node, "fillet neighborhood is invalid")),
    };
    let periodic_anchor = if required_boolean(node, &periodic_name)? {
        Some(DocumentTrimParameter {
            parameter: required_quantity(node, &anchor_parameter_name, IntentUnit::Dimensionless)?,
            winding: required_i32(node, &anchor_winding_name)?,
        })
    } else {
        require_absent(node, &[&anchor_parameter_name, &anchor_winding_name])?;
        None
    };
    Ok(CurveFilletParentRequest {
        curve: input_span(node, state, index)?,
        parameter,
        winding: required_i32(node, &winding_name)?,
        neighborhood,
        side: required_curve_normal_side(node, &side_name)?,
        trim_endpoint: required_fillet_trim_endpoint(node, &endpoint_name)?,
        periodic_anchor,
    })
}

#[allow(
    clippy::too_many_lines,
    reason = "face matching and explicit open-chain traversal form one fail-closed topology adapter"
)]
fn operation_profile_offset_operand(
    node: &IntentNode,
    session: &RetainedSketchDocumentSession,
    state: &LoweringState,
) -> Result<(SketchProfileOffsetOperand, Arc<OffsetOperandIndex>), IntentMaterializationError> {
    let query = PreparedOffsetOperandQuery::capture(session, OffsetOperandRequest::default())
        .map_err(|_| invalid_operation(node, "accepted Profile Offset topology is unavailable"))?;
    let outcome = query
        .execute(OperationControl::unlimited())
        .map_err(|_| invalid_operation(node, "Profile Offset topology analysis failed"))?;
    let result = match outcome {
        OperationOutcome::Completed { value, .. } => value,
        OperationOutcome::Cancelled { .. } | OperationOutcome::WorkExhausted { .. } => {
            return Err(invalid_operation(
                node,
                "Profile Offset topology analysis did not complete",
            ));
        }
        _ => {
            return Err(invalid_operation(
                node,
                "Profile Offset topology returned an unknown outcome",
            ));
        }
    };
    let index = result
        .operand_index
        .ok_or_else(|| invalid_operation(node, "Profile Offset topology index is incomplete"))?;
    let profile_count = node
        .inputs
        .keys()
        .filter(|slot| slot.role == InputRole::Profile)
        .count();
    let operand = if profile_count > 0 {
        if node
            .inputs
            .contains_key(&InputSlot::new(InputRole::Chain, 0))
        {
            return Err(invalid_operation(
                node,
                "Profile Offset mixes face and chain operands",
            ));
        }
        let mut aggregates = Vec::with_capacity(profile_count);
        for ordinal in 0..profile_count {
            let ordinal = u16::try_from(ordinal).map_err(|_| {
                invalid_operation(node, "Profile Offset profile count exceeds limits")
            })?;
            let aggregate = operation_input_aggregate(node, state, InputRole::Profile, ordinal)?;
            if !aggregate.closed {
                return Err(invalid_operation(
                    node,
                    "Profile Offset face operand must be closed",
                ));
            }
            validate_aggregate_topology(node.id, aggregate, &index)?;
            aggregates.push(aggregate);
        }
        let outer = undirected_span_key(&aggregates[0].spans);
        let mut holes = aggregates
            .iter()
            .skip(1)
            .map(|aggregate| undirected_span_key(&aggregate.spans))
            .collect::<Vec<_>>();
        holes.sort();
        let mut matches = index.faces().iter().filter(|candidate| {
            let candidate_outer = undirected_directed_span_key(&candidate.key.outer.spans);
            let mut candidate_holes = candidate
                .key
                .holes
                .iter()
                .map(|hole| undirected_directed_span_key(&hole.spans))
                .collect::<Vec<_>>();
            candidate_holes.sort();
            candidate_outer == outer && candidate_holes == holes
        });
        let face = matches.next().ok_or_else(|| {
            invalid_operation(node, "Profile Offset face operands match no accepted face")
        })?;
        if matches.next().is_some() {
            return Err(invalid_operation(
                node,
                "Profile Offset face operands are ambiguous",
            ));
        }
        let direction = match required_enum(node, "direction")? {
            "outward" => DocumentFaceOffsetDirection::Outward,
            "inward" => DocumentFaceOffsetDirection::Inward,
            _ => {
                return Err(invalid_operation(
                    node,
                    "Profile Offset face direction is invalid",
                ));
            }
        };
        require_absent(node, &["side", "first_traversal"])?;
        SketchProfileOffsetOperand::Face {
            key: face.key.clone(),
            direction,
        }
    } else {
        let aggregate = operation_input_aggregate(node, state, InputRole::Chain, 0)?;
        if aggregate.closed {
            return Err(invalid_operation(
                node,
                "Profile Offset chain operand must be open",
            ));
        }
        validate_aggregate_topology(node.id, aggregate, &index)?;
        require_absent(node, &["direction"])?;
        let side = match required_enum(node, "side")? {
            "left" => DocumentLineSide::Left,
            "right" => DocumentLineSide::Right,
            _ => {
                return Err(invalid_operation(
                    node,
                    "Profile Offset chain side is invalid",
                ));
            }
        };
        let first = match required_enum(node, "first_traversal")? {
            "forward" => OffsetTraversal::Forward,
            "reverse" => OffsetTraversal::Reverse,
            _ => {
                return Err(invalid_operation(
                    node,
                    "Profile Offset first traversal is invalid",
                ));
            }
        };
        let spans = directed_open_chain(node, aggregate, &index, first)?;
        SketchProfileOffsetOperand::OpenChain { spans, side }
    };
    Ok((operand, Arc::new(index)))
}

fn operation_input_aggregate<'a>(
    node: &IntentNode,
    state: &'a LoweringState,
    role: InputRole,
    index: u16,
) -> Result<&'a IntentAggregateMaterialization, IntentMaterializationError> {
    let slot = InputSlot::new(role, index);
    let source = node
        .inputs
        .get(&slot)
        .ok_or(IntentMaterializationError::MissingInput {
            node: node.id,
            slot,
        })?;
    state
        .aggregate_bindings
        .get(source)
        .ok_or(IntentMaterializationError::UnboundInput {
            node: node.id,
            slot,
        })
}

fn undirected_span_key(spans: &[CurveSpan]) -> Vec<CurveSpan> {
    let mut value = spans.to_vec();
    value.sort_unstable();
    value
}

fn undirected_directed_span_key(spans: &[OffsetDirectedSpan]) -> Vec<CurveSpan> {
    undirected_span_key(
        &spans
            .iter()
            .map(|directed| directed.span)
            .collect::<Vec<_>>(),
    )
}

fn directed_open_chain(
    node: &IntentNode,
    aggregate: &IntentAggregateMaterialization,
    index: &OffsetOperandIndex,
    first: OffsetTraversal,
) -> Result<Vec<OffsetDirectedSpan>, IntentMaterializationError> {
    let Some(first_span) = aggregate.spans.first().copied() else {
        return Err(invalid_operation(node, "Profile Offset chain is empty"));
    };
    let mut directed = vec![OffsetDirectedSpan {
        span: first_span,
        traversal: first,
    }];
    for next_span in aggregate.spans.iter().copied().skip(1) {
        let previous = *directed.last().expect("open chain starts nonempty");
        let exit = OffsetEndpointRef {
            span: previous.span,
            endpoint: match previous.traversal {
                OffsetTraversal::Forward => OffsetEndpointRole::End,
                OffsetTraversal::Reverse => OffsetEndpointRole::Start,
            },
        };
        let mut matches = index
            .adjacent_endpoints(exit)
            .filter(|endpoint| endpoint.span == next_span);
        let entry = matches.next().ok_or_else(|| {
            invalid_operation(node, "Profile Offset chain has a disconnected ordered join")
        })?;
        if matches.next().is_some() {
            return Err(invalid_operation(
                node,
                "Profile Offset chain traversal is ambiguous",
            ));
        }
        directed.push(OffsetDirectedSpan {
            span: next_span,
            traversal: match entry.endpoint {
                OffsetEndpointRole::Start => OffsetTraversal::Forward,
                OffsetEndpointRole::End => OffsetTraversal::Reverse,
            },
        });
    }
    Ok(directed)
}

fn lower_horizontal(
    node: &IntentNode,
    batch: &mut SketchMaterializationBatch,
    state: &mut LoweringState,
) -> Result<(), IntentMaterializationError> {
    let span_slot = InputSlot::new(InputRole::Span, 0);
    let curve_slot = InputSlot::new(InputRole::Curve, 0);
    let (slot, source) = node
        .inputs
        .get(&span_slot)
        .map(|source| (span_slot, source))
        .or_else(|| {
            node.inputs
                .get(&curve_slot)
                .map(|source| (curve_slot, source))
        })
        .ok_or(IntentMaterializationError::MissingInput {
            node: node.id,
            slot: span_slot,
        })?;
    let line = match state.port_bindings.get(source) {
        Some(IntentNativeBinding::CurveSpan(span)) => *span,
        Some(IntentNativeBinding::Curve(curve)) => CurveSpan::line(*curve),
        None => {
            return Err(IntentMaterializationError::UnboundInput {
                node: node.id,
                slot,
            });
        }
        _ => return Err(IntentMaterializationError::NativeKindMismatch { node: node.id }),
    };
    let constraint_port = require_port(node, IntentPortRole::Constraint, 0)?;
    let source_port = require_port(node, IntentPortRole::Source, 0)?;
    let constraint = match state.port_bindings.get(&constraint_port.as_ref(node.id)) {
        Some(IntentNativeBinding::Constraint(id)) => *id,
        _ => return Err(IntentMaterializationError::NativeKindMismatch { node: node.id }),
    };
    let source_id = match state.port_bindings.get(&source_port.as_ref(node.id)) {
        Some(IntentNativeBinding::Source(id)) => *id,
        _ => return Err(IntentMaterializationError::NativeKindMismatch { node: node.id }),
    };
    batch.push_constraint(DocumentConstraint {
        id: constraint,
        source_id,
        label: node.symbol.as_str().to_owned(),
        suppressed: false,
        definition: DocumentConstraintDefinition::Horizontal { line },
    });
    batch.push_source(source_id);
    state.consume_port(node, constraint_port);
    Ok(())
}

#[derive(Clone, Copy)]
struct RelationContactDefaults {
    parameter: f64,
    winding: i32,
    neighborhood: ContactNeighborhood,
    orientation: Option<TangentOrientation>,
}

impl RelationContactDefaults {
    const fn bounded(parameter: f64) -> Self {
        Self {
            parameter,
            winding: 0,
            neighborhood: ContactNeighborhood::Interior,
            orientation: None,
        }
    }

    const fn periodic(parameter: f64) -> Self {
        Self {
            parameter,
            winding: 0,
            neighborhood: ContactNeighborhood::Interior,
            orientation: None,
        }
    }

    const fn oriented(mut self, orientation: TangentOrientation) -> Self {
        self.orientation = Some(orientation);
        self
    }

    const fn neighborhood(mut self, neighborhood: ContactNeighborhood) -> Self {
        self.neighborhood = neighborhood;
        self
    }
}

fn inferred_contact_domain(
    node: &IntentNode,
    prefix: &str,
    curve: CurveSpan,
    state: &LoweringState,
) -> Result<ContactDomain, IntentMaterializationError> {
    let definition = state.curve_definitions.get(&curve.curve).ok_or(
        IntentMaterializationError::InvalidGeometry {
            node: node.id,
            reason: "contact curve topology is unavailable",
        },
    )?;
    let valid_span = match definition {
        CurveDefinition::Polyline {
            branch_directions, ..
        } => usize::try_from(curve.segment).is_ok_and(|index| index < branch_directions.len()),
        CurveDefinition::BSpline { span_ids, .. } | CurveDefinition::Nurbs { span_ids, .. } => {
            span_ids.contains(&curve.segment)
        }
        _ => curve.segment == 0,
    };
    if !valid_span {
        return Err(IntentMaterializationError::InvalidGeometry {
            node: node.id,
            reason: "contact curve span is outside its intrinsic topology",
        });
    }
    let support = enum_field(node, &format!("{prefix}_support"))?;
    if let Some(value) = support {
        if value != "supporting_line"
            || !matches!(
                definition,
                CurveDefinition::Line { .. } | CurveDefinition::Polyline { .. }
            )
        {
            return Err(IntentMaterializationError::InvalidGeometry {
                node: node.id,
                reason: "supporting-line contact is valid only for a line or polyline span",
            });
        }
        return Ok(ContactDomain::SupportingLine);
    }
    Ok(
        if matches!(
            definition,
            CurveDefinition::Circle { .. } | CurveDefinition::Ellipse { .. }
        ) {
            ContactDomain::Periodic {
                period: std::f64::consts::TAU,
            }
        } else {
            ContactDomain::Bounded {
                lower: 0.0,
                upper: 1.0,
            }
        },
    )
}

fn relation_contact_range(
    node: &IntentNode,
    prefix: &str,
) -> Result<Option<ContactAdmissibleRange>, IntentMaterializationError> {
    let lower = field_quantity(
        node,
        &format!("{prefix}_range_lower"),
        IntentUnit::Dimensionless,
    )?;
    let upper = field_quantity(
        node,
        &format!("{prefix}_range_upper"),
        IntentUnit::Dimensionless,
    )?;
    match (lower, upper) {
        (None, None) => Ok(None),
        (Some(lower), Some(upper)) if lower.is_finite() && upper.is_finite() && lower <= upper => {
            Ok(Some(ContactAdmissibleRange { lower, upper }))
        }
        (Some(_), Some(_)) => Err(IntentMaterializationError::InvalidGeometry {
            node: node.id,
            reason: "contact admissible range must be finite and ordered",
        }),
        (Some(_), None) | (None, Some(_)) => Err(IntentMaterializationError::InvalidGeometry {
            node: node.id,
            reason: "contact admissible range requires both lower and upper limits",
        }),
    }
}

fn relation_contact_neighborhood(
    node: &IntentNode,
    prefix: &str,
    default: ContactNeighborhood,
) -> Result<ContactNeighborhood, IntentMaterializationError> {
    let neighborhood_field = format!("{prefix}_neighborhood");
    match enum_field(node, &neighborhood_field)? {
        None => Ok(default),
        Some("interior") => Ok(ContactNeighborhood::Interior),
        Some("start") => Ok(ContactNeighborhood::Start),
        Some("end") => Ok(ContactNeighborhood::End),
        Some("local") => {
            let (default_lower, default_upper) = match default {
                ContactNeighborhood::Local { lower, upper } => (lower, upper),
                ContactNeighborhood::Interior
                | ContactNeighborhood::Start
                | ContactNeighborhood::End => (0.0, 1.0),
            };
            let lower = field_quantity(
                node,
                &format!("{prefix}_neighborhood_lower"),
                IntentUnit::Dimensionless,
            )?
            .unwrap_or(default_lower);
            let upper = field_quantity(
                node,
                &format!("{prefix}_neighborhood_upper"),
                IntentUnit::Dimensionless,
            )?
            .unwrap_or(default_upper);
            Ok(ContactNeighborhood::Local { lower, upper })
        }
        Some(_) => Err(IntentMaterializationError::InvalidGeometry {
            node: node.id,
            reason: "contact neighborhood is invalid",
        }),
    }
}

fn relation_contact_orientation(
    node: &IntentNode,
    prefix: &str,
    default: Option<TangentOrientation>,
) -> Result<Option<TangentOrientation>, IntentMaterializationError> {
    match enum_field(node, &format!("{prefix}_orientation"))? {
        None => Ok(default),
        Some("none" | "unoriented") => Ok(None),
        Some("aligned") => Ok(Some(TangentOrientation::Aligned)),
        Some("opposed") => Ok(Some(TangentOrientation::Opposed)),
        Some(_) => Err(IntentMaterializationError::InvalidGeometry {
            node: node.id,
            reason: "contact tangent orientation is invalid",
        }),
    }
}

#[allow(clippy::too_many_arguments)]
fn materialize_relation_contact(
    candidate: &dyn IntentMaterializationSource,
    node: &IntentNode,
    index: u16,
    prefix: &str,
    curve: CurveSpan,
    defaults: RelationContactDefaults,
    batch: &mut SketchMaterializationBatch,
    state: &mut LoweringState,
) -> Result<ContactId, IntentMaterializationError> {
    let parameter = field_quantity(
        node,
        &format!("{prefix}_parameter"),
        IntentUnit::Dimensionless,
    )?
    .unwrap_or(defaults.parameter);
    let winding = i32::try_from(field_integer(
        node,
        &format!("{prefix}_winding"),
        i64::from(defaults.winding),
    )?)
    .map_err(|_| IntentMaterializationError::InvalidGeometry {
        node: node.id,
        reason: "contact winding exceeds persistent limits",
    })?;
    let domain = inferred_contact_domain(node, prefix, curve, state)?;
    let range = relation_contact_range(node, prefix)?;
    let neighborhood = relation_contact_neighborhood(node, prefix, defaults.neighborhood)?;
    let orientation = relation_contact_orientation(node, prefix, defaults.orientation)?;
    materialize_recipe_contact(
        candidate,
        node,
        index,
        curve,
        parameter,
        domain,
        range,
        winding,
        neighborhood,
        orientation,
        batch,
        state,
    )
}

#[allow(
    clippy::too_many_lines,
    reason = "one exhaustive relation table keeps intent operands and native definitions aligned"
)]
fn lower_constraint(
    candidate: &dyn IntentMaterializationSource,
    node: &IntentNode,
    kind: ConstraintKind,
    batch: &mut SketchMaterializationBatch,
    state: &mut LoweringState,
) -> Result<(), IntentMaterializationError> {
    use ConstraintKind as C;

    if kind == C::Horizontal {
        return lower_horizontal(node, batch, state);
    }
    let definition = match kind {
        C::FixedPoint => {
            let point = input_point(node, state, 0)?;
            let target = point_field(node, "target")?
                .or_else(|| state.point_positions.get(&point).copied())
                .ok_or(IntentMaterializationError::InvalidGeometry {
                    node: node.id,
                    reason: "fixed-point target is unavailable",
                })?;
            DocumentConstraintDefinition::FixedPoint { point, target }
        }
        C::FixedCoordinate => DocumentConstraintDefinition::FixedCoordinate {
            point: input_point(node, state, 0)?,
            axis: coordinate_axis(node, "axis")?,
            target: field_quantity(node, "target", IntentUnit::Length)?.unwrap_or(0.0),
        },
        C::CoincidentWithOrigin => DocumentConstraintDefinition::CoincidentWithOrigin {
            point: input_point(node, state, 0)?,
        },
        C::PointOnDatumAxis => DocumentConstraintDefinition::PointOnDatumAxis {
            point: input_point(node, state, 0)?,
            axis: coordinate_axis(node, "axis")?,
        },
        C::Coincident => DocumentConstraintDefinition::Coincident {
            first: input_point(node, state, 0)?,
            second: input_point(node, state, 1)?,
        },
        C::ExternalPointCoincident => DocumentConstraintDefinition::ExternalPointCoincident {
            point: input_point(node, state, 0)?,
            external: DocumentExternalPointRef {
                binding: input_external(node, state, 0)?,
            },
        },
        C::Vertical => DocumentConstraintDefinition::Vertical {
            line: input_span(node, state, 0)?,
        },
        C::HorizontalPoints => DocumentConstraintDefinition::HorizontalPoints {
            first: input_point(node, state, 0)?,
            second: input_point(node, state, 1)?,
        },
        C::VerticalPoints => DocumentConstraintDefinition::VerticalPoints {
            first: input_point(node, state, 0)?,
            second: input_point(node, state, 1)?,
        },
        C::HorizontalPointToMidpoint => DocumentConstraintDefinition::HorizontalPointToMidpoint {
            point: input_point(node, state, 0)?,
            line: input_span(node, state, 0)?,
        },
        C::VerticalPointToMidpoint => DocumentConstraintDefinition::VerticalPointToMidpoint {
            point: input_point(node, state, 0)?,
            line: input_span(node, state, 0)?,
        },
        C::Parallel => DocumentConstraintDefinition::Parallel {
            first: input_span(node, state, 0)?,
            second: input_span(node, state, 1)?,
        },
        C::Perpendicular => DocumentConstraintDefinition::Perpendicular {
            first: input_span(node, state, 0)?,
            second: input_span(node, state, 1)?,
        },
        C::ExternalLineCollinear => DocumentConstraintDefinition::ExternalLineCollinear {
            line: DocumentLineSupportRef {
                span: input_span(node, state, 0)?,
                direction: direction_sense(node, "direction")?,
            },
            external: DocumentExternalLineSupportRef {
                binding: input_external(node, state, 0)?,
                direction: DocumentDirectionSense::Forward,
            },
        },
        C::CollinearWithDatumAxis => DocumentConstraintDefinition::CollinearWithDatumAxis {
            line: DocumentLineSupportRef {
                span: input_span(node, state, 0)?,
                direction: direction_sense(node, "direction")?,
            },
            axis: coordinate_axis(node, "axis")?,
        },
        C::Concentric => DocumentConstraintDefinition::Concentric {
            first: DocumentCenterRef {
                curve: input_curve(node, state, 0)?,
            },
            second: DocumentCenterRef {
                curve: input_curve(node, state, 1)?,
            },
        },
        C::Collinear => DocumentConstraintDefinition::Collinear {
            first: DocumentLineSupportRef {
                span: input_span(node, state, 0)?,
                direction: direction_sense(node, "first_direction")?,
            },
            second: DocumentLineSupportRef {
                span: input_span(node, state, 1)?,
                direction: direction_sense(node, "second_direction")?,
            },
        },
        C::EqualLength => DocumentConstraintDefinition::EqualLength {
            first: input_span(node, state, 0)?,
            second: input_span(node, state, 1)?,
        },
        C::EqualRadius => DocumentConstraintDefinition::EqualRadius {
            first: input_curve(node, state, 0)?,
            second: input_curve(node, state, 1)?,
        },
        C::Midpoint => DocumentConstraintDefinition::Midpoint {
            point: input_point(node, state, 0)?,
            line: input_span(node, state, 0)?,
        },
        C::SymmetricAboutLine => DocumentConstraintDefinition::SymmetricAboutLine {
            first: input_point(node, state, 0)?,
            second: input_point(node, state, 1)?,
            line: input_span(node, state, 0)?,
        },
        C::SymmetricAboutDatumAxis => DocumentConstraintDefinition::SymmetricAboutDatumAxis {
            first: input_point(node, state, 0)?,
            second: input_point(node, state, 1)?,
            axis: coordinate_axis(node, "axis")?,
        },
        C::CircleCircleTangency => DocumentConstraintDefinition::CircleCircleTangency {
            first: input_curve(node, state, 0)?,
            second: input_curve(node, state, 1)?,
            mode: circle_tangency_mode(node)?,
            center_direction: point_field(node, "center_direction")?.unwrap_or([1.0, 0.0]),
        },
        C::PointOnCurve => DocumentConstraintDefinition::PointOnCurve {
            point: input_point(node, state, 0)?,
            contact: materialize_relation_contact(
                candidate,
                node,
                0,
                "contact",
                input_span(node, state, 0)?,
                RelationContactDefaults::bounded(0.5),
                batch,
                state,
            )?,
        },
        C::LineCircleTangency => DocumentConstraintDefinition::LineCircleTangency {
            line_contact: materialize_relation_contact(
                candidate,
                node,
                0,
                "first_contact",
                input_span(node, state, 0)?,
                RelationContactDefaults::bounded(0.5).oriented(TangentOrientation::Aligned),
                batch,
                state,
            )?,
            circle_contact: materialize_relation_contact(
                candidate,
                node,
                1,
                "second_contact",
                CurveSpan::line(input_curve(node, state, 0)?),
                RelationContactDefaults::periodic(0.0).oriented(TangentOrientation::Aligned),
                batch,
                state,
            )?,
            side: line_side(node)?,
        },
        C::CircleArcTangency => DocumentConstraintDefinition::CircleArcTangency {
            circle_contact: materialize_relation_contact(
                candidate,
                node,
                0,
                "first_contact",
                CurveSpan::line(input_curve(node, state, 0)?),
                RelationContactDefaults::periodic(0.0).oriented(TangentOrientation::Aligned),
                batch,
                state,
            )?,
            arc_contact: materialize_relation_contact(
                candidate,
                node,
                1,
                "second_contact",
                CurveSpan::line(input_curve(node, state, 1)?),
                RelationContactDefaults::bounded(0.5).oriented(TangentOrientation::Aligned),
                batch,
                state,
            )?,
            side: arc_tangency_side(node)?,
        },
        C::LineCurveTangency => DocumentConstraintDefinition::LineCurveTangency {
            line: input_span(node, state, 0)?,
            endpoint: feature_endpoint(node, "endpoint")?,
            curve_contact: materialize_relation_contact(
                candidate,
                node,
                0,
                "contact",
                input_span(node, state, 1)?,
                RelationContactDefaults::bounded(0.0)
                    .neighborhood(ContactNeighborhood::Start)
                    .oriented(TangentOrientation::Aligned),
                batch,
                state,
            )?,
        },
        C::CurveCurveContact => DocumentConstraintDefinition::CurveCurveContact {
            first_contact: materialize_relation_contact(
                candidate,
                node,
                0,
                "first_contact",
                input_span(node, state, 0)?,
                RelationContactDefaults::bounded(0.5),
                batch,
                state,
            )?,
            second_contact: materialize_relation_contact(
                candidate,
                node,
                1,
                "second_contact",
                input_span(node, state, 1)?,
                RelationContactDefaults::bounded(0.5),
                batch,
                state,
            )?,
        },
        C::CurveCurveTangency => DocumentConstraintDefinition::CurveCurveTangency {
            first_contact: materialize_relation_contact(
                candidate,
                node,
                0,
                "first_contact",
                input_span(node, state, 0)?,
                RelationContactDefaults::bounded(0.5).oriented(TangentOrientation::Aligned),
                batch,
                state,
            )?,
            second_contact: materialize_relation_contact(
                candidate,
                node,
                1,
                "second_contact",
                input_span(node, state, 1)?,
                RelationContactDefaults::bounded(0.5).oriented(TangentOrientation::Aligned),
                batch,
                state,
            )?,
        },
        C::CurveDirection => DocumentConstraintDefinition::CurveDirection {
            line: input_span(node, state, 0)?,
            curve_contact: materialize_relation_contact(
                candidate,
                node,
                0,
                "contact",
                input_span(node, state, 1)?,
                RelationContactDefaults::bounded(0.5),
                batch,
                state,
            )?,
            relation: curve_direction_relation(node)?,
        },
        C::EqualCurvature => DocumentConstraintDefinition::EqualCurvature {
            first_contact: materialize_relation_contact(
                candidate,
                node,
                0,
                "first_contact",
                input_span(node, state, 0)?,
                RelationContactDefaults::bounded(0.5),
                batch,
                state,
            )?,
            second_contact: materialize_relation_contact(
                candidate,
                node,
                1,
                "second_contact",
                input_span(node, state, 1)?,
                RelationContactDefaults::bounded(0.5),
                batch,
                state,
            )?,
            relation: curvature_relation(node)?,
        },
        C::EndpointContinuity => DocumentConstraintDefinition::EndpointContinuity {
            first_contact: materialize_relation_contact(
                candidate,
                node,
                0,
                "first_contact",
                input_span(node, state, 0)?,
                RelationContactDefaults::bounded(1.0).neighborhood(ContactNeighborhood::End),
                batch,
                state,
            )?,
            second_contact: materialize_relation_contact(
                candidate,
                node,
                1,
                "second_contact",
                input_span(node, state, 1)?,
                RelationContactDefaults::bounded(0.0).neighborhood(ContactNeighborhood::Start),
                batch,
                state,
            )?,
            continuity: curve_continuity(node)?,
        },
        C::LineLineFillet => DocumentConstraintDefinition::LineLineFillet {
            arc: input_curve(node, state, 0)?,
            first_contact: materialize_relation_contact(
                candidate,
                node,
                0,
                "first_contact",
                input_span(node, state, 0)?,
                RelationContactDefaults::bounded(0.5),
                batch,
                state,
            )?,
            first_side: curve_normal_side(node, "first_side")?,
            second_contact: materialize_relation_contact(
                candidate,
                node,
                1,
                "second_contact",
                input_span(node, state, 1)?,
                RelationContactDefaults::bounded(0.5),
                batch,
                state,
            )?,
            second_side: curve_normal_side(node, "second_side")?,
            endpoint_order: fillet_endpoint_order(node)?,
        },
        C::CurveCurveFillet => DocumentConstraintDefinition::CurveCurveFillet {
            arc: input_curve(node, state, 0)?,
            first_contact: materialize_relation_contact(
                candidate,
                node,
                0,
                "first_contact",
                input_span(node, state, 0)?,
                RelationContactDefaults::bounded(0.5),
                batch,
                state,
            )?,
            first_side: curve_normal_side(node, "first_side")?,
            first_trim_endpoint: fillet_trim_endpoint(node, "first_trim_endpoint")?,
            second_contact: materialize_relation_contact(
                candidate,
                node,
                1,
                "second_contact",
                input_span(node, state, 1)?,
                RelationContactDefaults::bounded(0.5),
                batch,
                state,
            )?,
            second_side: curve_normal_side(node, "second_side")?,
            second_trim_endpoint: fillet_trim_endpoint(node, "second_trim_endpoint")?,
            endpoint_order: fillet_endpoint_order(node)?,
        },
        C::Horizontal => unreachable!("horizontal is lowered through its dual-input adapter"),
    };
    materialize_constraint(node, definition, batch, state)
}

fn materialize_constraint(
    node: &IntentNode,
    definition: DocumentConstraintDefinition,
    batch: &mut SketchMaterializationBatch,
    state: &mut LoweringState,
) -> Result<(), IntentMaterializationError> {
    materialize_indexed_constraint(node, 0, definition, batch, state)
}

fn materialize_indexed_constraint(
    node: &IntentNode,
    index: u16,
    definition: DocumentConstraintDefinition,
    batch: &mut SketchMaterializationBatch,
    state: &mut LoweringState,
) -> Result<(), IntentMaterializationError> {
    let constraint_port = require_port(node, IntentPortRole::Constraint, index)?;
    let source_port = require_port(node, IntentPortRole::Source, index)?;
    let constraint = match state.port_bindings.get(&constraint_port.as_ref(node.id)) {
        Some(IntentNativeBinding::Constraint(id)) => *id,
        _ => return Err(IntentMaterializationError::NativeKindMismatch { node: node.id }),
    };
    let source_id = match state.port_bindings.get(&source_port.as_ref(node.id)) {
        Some(IntentNativeBinding::Source(id)) => *id,
        _ => return Err(IntentMaterializationError::NativeKindMismatch { node: node.id }),
    };
    if let DocumentConstraintDefinition::CurveCurveFillet {
        first_contact,
        first_trim_endpoint,
        second_contact,
        second_trim_endpoint,
        ..
    } = &definition
    {
        for (contact, endpoint) in [
            (*first_contact, *first_trim_endpoint),
            (*second_contact, *second_trim_endpoint),
        ] {
            let (support, winding, domain) = state.contact_states.get(&contact).copied().ok_or(
                IntentMaterializationError::InvalidGeometry {
                    node: node.id,
                    reason: "fillet contact state is unavailable",
                },
            )?;
            if matches!(domain, ContactDomain::Periodic { .. }) {
                return Err(IntentMaterializationError::InvalidGeometry {
                    node: node.id,
                    reason: "periodic fillet parents require an explicit trim anchor",
                });
            }
            let fixed = DocumentTrimBoundary::Fixed(DocumentTrimParameter {
                parameter: match endpoint {
                    DocumentFilletTrimEndpoint::Start => 1.0,
                    DocumentFilletTrimEndpoint::End => 0.0,
                },
                winding,
            });
            let owned = DocumentTrimBoundary::FilletContact {
                owner: constraint,
                contact,
            };
            let (start, end) = match endpoint {
                DocumentFilletTrimEndpoint::Start => (owned, fixed),
                DocumentFilletTrimEndpoint::End => (fixed, owned),
            };
            batch.push_trim_view(DocumentCurveTrimView {
                support,
                start,
                end,
            });
        }
    }
    batch.push_constraint(DocumentConstraint {
        id: constraint,
        source_id,
        label: node.symbol.as_str().to_owned(),
        suppressed: false,
        definition,
    });
    batch.push_source(source_id);
    state.consume_port(node, constraint_port);
    Ok(())
}

fn input_binding<'a>(
    node: &IntentNode,
    state: &'a LoweringState,
    role: InputRole,
    index: u16,
) -> Result<&'a IntentNativeBinding, IntentMaterializationError> {
    let slot = InputSlot::new(role, index);
    let source = node
        .inputs
        .get(&slot)
        .ok_or(IntentMaterializationError::MissingInput {
            node: node.id,
            slot,
        })?;
    state
        .port_bindings
        .get(source)
        .ok_or(IntentMaterializationError::UnboundInput {
            node: node.id,
            slot,
        })
}

fn input_point(
    node: &IntentNode,
    state: &LoweringState,
    index: u16,
) -> Result<DesignPointId, IntentMaterializationError> {
    match input_binding(node, state, InputRole::Point, index)? {
        IntentNativeBinding::Point(point) => Ok(*point),
        _ => Err(IntentMaterializationError::NativeKindMismatch { node: node.id }),
    }
}

fn input_span(
    node: &IntentNode,
    state: &LoweringState,
    index: u16,
) -> Result<CurveSpan, IntentMaterializationError> {
    let binding = input_binding(node, state, InputRole::Span, index)
        .or_else(|_| input_binding(node, state, InputRole::Curve, index))?;
    match binding {
        IntentNativeBinding::CurveSpan(span) => Ok(*span),
        IntentNativeBinding::Curve(curve) => Ok(CurveSpan::line(*curve)),
        _ => Err(IntentMaterializationError::NativeKindMismatch { node: node.id }),
    }
}

fn input_curve(
    node: &IntentNode,
    state: &LoweringState,
    index: u16,
) -> Result<CurveId, IntentMaterializationError> {
    let binding = input_binding(node, state, InputRole::Curve, index)
        .or_else(|_| input_binding(node, state, InputRole::Span, index))?;
    match binding {
        IntentNativeBinding::Curve(curve) => Ok(*curve),
        IntentNativeBinding::CurveSpan(span) => Ok(span.curve),
        _ => Err(IntentMaterializationError::NativeKindMismatch { node: node.id }),
    }
}

fn input_external(
    node: &IntentNode,
    state: &LoweringState,
    index: u16,
) -> Result<DocumentExternalBindingId, IntentMaterializationError> {
    match input_binding(node, state, InputRole::External, index)? {
        IntentNativeBinding::ExternalBinding(binding) => Ok(*binding),
        _ => Err(IntentMaterializationError::NativeKindMismatch { node: node.id }),
    }
}

fn input_parameter(
    node: &IntentNode,
    state: &LoweringState,
) -> Result<DocumentParameterId, IntentMaterializationError> {
    match input_binding(node, state, InputRole::Parameter, 0)? {
        IntentNativeBinding::Parameter(parameter) => Ok(*parameter),
        _ => Err(IntentMaterializationError::NativeKindMismatch { node: node.id }),
    }
}

fn parameter_kind(node: &IntentNode) -> Result<DocumentParameterKind, IntentMaterializationError> {
    match enum_field(node, "kind")? {
        Some("length") => Ok(DocumentParameterKind::Length),
        Some("angle") => Ok(DocumentParameterKind::Angle),
        Some("dimensionless") => Ok(DocumentParameterKind::Dimensionless),
        Some("activation") => Ok(DocumentParameterKind::Activation),
        Some(_) | None => Err(IntentMaterializationError::InvalidGeometry {
            node: node.id,
            reason: "parameter kind must be length, angle, dimensionless, or activation",
        }),
    }
}

fn input_activation_element(
    node: &IntentNode,
    state: &LoweringState,
) -> Result<DocumentElementId, IntentMaterializationError> {
    let candidates = [
        InputRole::Point,
        InputRole::Contact,
        InputRole::Curve,
        InputRole::Scalar,
        InputRole::Constraint,
        InputRole::Dimension,
        InputRole::External,
        InputRole::Source,
    ];
    for role in candidates {
        let slot = InputSlot::new(role, 0);
        if !node.inputs.contains_key(&slot) {
            continue;
        }
        return match input_binding(node, state, role, 0)? {
            IntentNativeBinding::Point(id) => Ok(DocumentElementId::Point(*id)),
            IntentNativeBinding::Contact(id) => Ok(DocumentElementId::Contact(*id)),
            IntentNativeBinding::Curve(id) => Ok(DocumentElementId::Curve(*id)),
            IntentNativeBinding::CurveSpan(span) => Ok(DocumentElementId::Curve(span.curve)),
            IntentNativeBinding::Scalar(id) => Ok(DocumentElementId::Scalar(*id)),
            IntentNativeBinding::Constraint(id) => Ok(DocumentElementId::Constraint(*id)),
            IntentNativeBinding::Dimension(id) => Ok(DocumentElementId::Dimension(*id)),
            IntentNativeBinding::ExternalBinding(id) => Ok(DocumentElementId::ExternalBinding(*id)),
            IntentNativeBinding::Source(id) => Ok(DocumentElementId::Source(*id)),
            _ => Err(IntentMaterializationError::NativeKindMismatch { node: node.id }),
        };
    }
    Err(IntentMaterializationError::MissingInput {
        node: node.id,
        slot: InputSlot::new(InputRole::Source, 0),
    })
}

fn lower_parameter(
    node: &IntentNode,
    kind: ParameterIntentKind,
    batch: &mut SketchMaterializationBatch,
    state: &mut LoweringState,
) -> Result<(), IntentMaterializationError> {
    match kind {
        ParameterIntentKind::Parameter => {
            let port = require_port(node, IntentPortRole::Parameter, 0)?;
            let parameter = match state.port_bindings.get(&port.as_ref(node.id)) {
                Some(IntentNativeBinding::Parameter(parameter)) => *parameter,
                _ => {
                    return Err(IntentMaterializationError::NativeKindMismatch { node: node.id });
                }
            };
            let kind = parameter_kind(node)?;
            batch.push_parameter(DocumentParameter {
                id: parameter,
                label: node.symbol.as_str().to_owned(),
                kind,
            });
            state.parameter_kinds.insert(parameter, kind);
            state.consume_port(node, port);
        }
        ParameterIntentKind::Binding => {
            let parameter = input_parameter(node, state)?;
            let parameter_kind = state
                .parameter_kinds
                .get(&parameter)
                .copied()
                .ok_or(IntentMaterializationError::NativeKindMismatch { node: node.id })?;
            let target = match parameter_kind {
                DocumentParameterKind::Length | DocumentParameterKind::Angle => {
                    match input_binding(node, state, InputRole::Dimension, 0)? {
                        IntentNativeBinding::Dimension(dimension) => {
                            DocumentParameterTarget::DrivingDimension(*dimension)
                        }
                        _ => {
                            return Err(IntentMaterializationError::NativeKindMismatch {
                                node: node.id,
                            });
                        }
                    }
                }
                DocumentParameterKind::Dimensionless => {
                    let scalar = match input_binding(node, state, InputRole::Scalar, 0)? {
                        IntentNativeBinding::Scalar(scalar) => *scalar,
                        _ => {
                            return Err(IntentMaterializationError::NativeKindMismatch {
                                node: node.id,
                            });
                        }
                    };
                    let domain =
                        state.scalar_domains.get(&scalar).copied().ok_or(
                            IntentMaterializationError::NativeKindMismatch { node: node.id },
                        )?;
                    DocumentParameterTarget::DimensionlessFixedScalar(DocumentScalarPropertyRef {
                        scalar,
                        unit: DocumentScalarUnit::Dimensionless,
                        domain,
                        branch: DocumentScalarBranch::Dimensionless,
                    })
                }
                DocumentParameterKind::Activation => {
                    DocumentParameterTarget::Activation(input_activation_element(node, state)?)
                }
            };
            batch.push_parameter_binding(DocumentParameterBinding { parameter, target });
        }
        ParameterIntentKind::Output => {
            let parameter = input_parameter(node, state)?;
            let dimension = match input_binding(node, state, InputRole::Dimension, 0)? {
                IntentNativeBinding::Dimension(dimension) => *dimension,
                _ => {
                    return Err(IntentMaterializationError::NativeKindMismatch { node: node.id });
                }
            };
            batch.push_parameter_output(DocumentParameterOutput {
                parameter,
                dimension,
            });
        }
    }
    Ok(())
}

fn external_feature_kind(
    node: &IntentNode,
) -> Result<ExternalFeatureKindV1, IntentMaterializationError> {
    match enum_field(node, "feature_kind")? {
        Some("point") => Ok(ExternalFeatureKindV1::Point),
        Some("line_segment") => Ok(ExternalFeatureKindV1::LineSegment),
        Some(_) | None => Err(IntentMaterializationError::InvalidGeometry {
            node: node.id,
            reason: "external feature kind must be point or line_segment",
        }),
    }
}

fn external_topology_digest(
    node: &IntentNode,
) -> Result<Option<ExternalTopologyDigest>, IntentMaterializationError> {
    let Some(encoded) = text_field(node, "topology_digest")? else {
        return Ok(None);
    };
    if encoded.len() != 64
        || !encoded
            .as_bytes()
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
    {
        return Err(IntentMaterializationError::InvalidGeometry {
            node: node.id,
            reason: "external topology digest must be exactly 64 lowercase hexadecimal digits",
        });
    }
    let mut bytes = [0_u8; 32];
    for (index, pair) in encoded.as_bytes().chunks_exact(2).enumerate() {
        let high = hex_nibble(pair[0]);
        let low = hex_nibble(pair[1]);
        bytes[index] = high * 16 + low;
    }
    Ok(Some(ExternalTopologyDigest::from_bytes(bytes)))
}

const fn hex_nibble(byte: u8) -> u8 {
    match byte {
        b'0'..=b'9' => byte - b'0',
        b'a'..=b'f' => byte - b'a' + 10,
        _ => 0,
    }
}

fn lower_external(
    node: &IntentNode,
    kind: ExternalIntentKind,
    snapshots: &ExternalSnapshotSet,
    batch: &mut SketchMaterializationBatch,
    state: &mut LoweringState,
) -> Result<(), IntentMaterializationError> {
    match kind {
        ExternalIntentKind::Binding => {
            let port = require_port(node, IntentPortRole::External, 0)?;
            let binding = match state.port_bindings.get(&port.as_ref(node.id)) {
                Some(IntentNativeBinding::ExternalBinding(binding)) => *binding,
                _ => {
                    return Err(IntentMaterializationError::NativeKindMismatch { node: node.id });
                }
            };
            let expected_kind = external_feature_kind(node)?;
            let expected_topology = external_topology_digest(node)?;
            match (expected_kind, expected_topology) {
                (ExternalFeatureKindV1::Point, Some(_))
                | (ExternalFeatureKindV1::LineSegment, None) => {
                    return Err(IntentMaterializationError::InvalidGeometry {
                        node: node.id,
                        reason: "point externals forbid topology and line externals require it",
                    });
                }
                (ExternalFeatureKindV1::Point, None)
                | (ExternalFeatureKindV1::LineSegment, Some(_)) => {}
            }
            batch.push_external_binding(DocumentExternalBinding {
                id: binding,
                label: node.symbol.as_str().to_owned(),
                expected_kind,
                expected_topology,
            });
            state.consume_port(node, port);
        }
        ExternalIntentKind::SnapshotReference => {
            let binding = input_external(node, state, 0)?;
            let revision = field_natural(node, "snapshot_revision", 0)?;
            if revision != snapshots.revision()
                || !snapshots
                    .entries()
                    .iter()
                    .any(|entry| entry.binding == binding)
            {
                return Err(IntentMaterializationError::InvalidGeometry {
                    node: node.id,
                    reason: "snapshot reference must name an entry at the exact supplied set revision",
                });
            }
        }
    }
    Ok(())
}

fn lower_logical_declaration(
    node: &IntentNode,
    state: &mut LoweringState,
) -> Result<(), IntentMaterializationError> {
    if matches!(node.kind, IntentNodeKind::Annotation) {
        let _ = point_field(node, "offset")?;
    }
    for (slot, source) in &node.inputs {
        if !state.port_bindings.contains_key(source) {
            return Err(IntentMaterializationError::UnboundInput {
                node: node.id,
                slot: *slot,
            });
        }
    }
    Ok(())
}

fn input_aggregate<'a>(
    node: &IntentNode,
    state: &'a LoweringState,
) -> Result<&'a IntentAggregateMaterialization, IntentMaterializationError> {
    let source = [InputRole::Profile, InputRole::Chain]
        .into_iter()
        .find_map(|role| node.inputs.get(&InputSlot::new(role, 0)).copied())
        .ok_or(IntentMaterializationError::MissingInput {
            node: node.id,
            slot: InputSlot::new(InputRole::Chain, 0),
        })?;
    state
        .aggregate_bindings
        .get(&source)
        .ok_or(IntentMaterializationError::UnboundInput {
            node: node.id,
            slot: InputSlot::new(InputRole::Chain, 0),
        })
}

fn offset_traversal(
    node: &IntentNode,
    name: &str,
) -> Result<DocumentOffsetTraversal, IntentMaterializationError> {
    match enum_field(node, name)? {
        Some("forward") => Ok(DocumentOffsetTraversal::Forward),
        Some("reverse") => Ok(DocumentOffsetTraversal::Reverse),
        Some(_) | None => Err(IntentMaterializationError::InvalidGeometry {
            node: node.id,
            reason: "profile-offset traversal must be explicitly forward or reverse",
        }),
    }
}

fn profile_offset_operand(
    node: &IntentNode,
    state: &LoweringState,
) -> Result<DocumentProfileOffsetOperand, IntentMaterializationError> {
    let aggregate = input_aggregate(node, state)?;
    if aggregate.spans.len() != 1 {
        return Err(IntentMaterializationError::InvalidAggregate {
            node: node.id,
            reason: "multi-edge profile offsets require explicit junction owners and branches",
        });
    }
    let source = DocumentDirectedProfileOffsetCurve {
        curve: aggregate.spans[0],
        traversal: offset_traversal(node, "source_traversal")?,
    };
    let target = DocumentDirectedProfileOffsetCurve {
        curve: input_span(node, state, 0)?,
        traversal: offset_traversal(node, "target_traversal")?,
    };
    let edge = DocumentProfileOffsetEdgePair { source, target };
    if aggregate.closed {
        let direction = match enum_field(node, "direction")? {
            Some("outward") => DocumentFaceOffsetDirection::Outward,
            Some("inward") => DocumentFaceOffsetDirection::Inward,
            Some(_) | None => {
                return Err(IntentMaterializationError::InvalidGeometry {
                    node: node.id,
                    reason: "closed profile offset direction must be explicitly outward or inward",
                });
            }
        };
        Ok(DocumentProfileOffsetOperand::Face {
            direction,
            outer: DocumentProfileOffsetLoop {
                edges: vec![edge],
                junctions: Vec::new(),
            },
            holes: Vec::new(),
        })
    } else {
        let side = match enum_field(node, "side")? {
            Some("left") => DocumentLineSide::Left,
            Some("right") => DocumentLineSide::Right,
            Some(_) | None => {
                return Err(IntentMaterializationError::InvalidGeometry {
                    node: node.id,
                    reason: "open-chain profile offset side must be explicitly left or right",
                });
            }
        };
        Ok(DocumentProfileOffsetOperand::OpenChain {
            side,
            chain: DocumentProfileOffsetChain {
                edges: vec![edge],
                junctions: Vec::new(),
                start_terminal: DocumentProfileOffsetTerminalPolicy::NormalTranslation,
                end_terminal: DocumentProfileOffsetTerminalPolicy::NormalTranslation,
            },
        })
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "the closed native dimension catalog stays auditable in one lowering dispatcher"
)]
fn lower_dimension(
    candidate: &dyn IntentMaterializationSource,
    node: &IntentNode,
    kind: DimensionKind,
    batch: &mut SketchMaterializationBatch,
    state: &mut LoweringState,
) -> Result<(), IntentMaterializationError> {
    let (intent_unit, scalar_unit, default, domain) = if kind == DimensionKind::OrientedAngle {
        (
            IntentUnit::Angle,
            ScalarUnit::Angle,
            std::f64::consts::FRAC_PI_2,
            ScalarDomain::Positive,
        )
    } else {
        (
            IntentUnit::Length,
            ScalarUnit::Length,
            1.0,
            ScalarDomain::Positive,
        )
    };
    let target = materialize_scalar(
        candidate,
        node,
        0,
        LeafField::Value,
        default,
        intent_unit,
        scalar_unit,
        domain,
        "target",
        batch,
        state,
    )?;
    let definition = match kind {
        DimensionKind::PointDistance => DocumentDimensionDefinition::PointDistance {
            first: input_point(node, state, 0)?,
            second: input_point(node, state, 1)?,
            target,
        },
        DimensionKind::CurveLength => DocumentDimensionDefinition::CurveLength {
            curve: input_span(node, state, 0)?,
            target,
        },
        DimensionKind::Radius => DocumentDimensionDefinition::Radius {
            curve: input_curve(node, state, 0)?,
            target,
        },
        DimensionKind::Diameter => DocumentDimensionDefinition::Diameter {
            curve: input_curve(node, state, 0)?,
            target,
        },
        DimensionKind::OrientedAngle => DocumentDimensionDefinition::OrientedAngle {
            first: input_span(node, state, 0)?,
            second: input_span(node, state, 1)?,
            target,
            orientation: angle_orientation(node)?,
        },
        DimensionKind::SupportingLineOffset => DocumentDimensionDefinition::SupportingLineOffset {
            source: input_span(node, state, 0)?,
            target_segment: input_span(node, state, 1)?,
            target,
            side: line_side(node)?,
            orientation: line_offset_orientation(node)?,
        },
        DimensionKind::ExactTranslatedSegmentOffset => {
            DocumentDimensionDefinition::ExactTranslatedSegmentOffset {
                source: input_span(node, state, 0)?,
                target_segment: input_span(node, state, 1)?,
                target,
                side: line_side(node)?,
                orientation: line_offset_orientation(node)?,
            }
        }
        DimensionKind::ProfileOffset => {
            if dimension_mode(node)? != DocumentDimensionMode::Driving {
                return Err(IntentMaterializationError::InvalidGeometry {
                    node: node.id,
                    reason: "profile offset dimensions must be driving",
                });
            }
            DocumentDimensionDefinition::ProfileOffset {
                target,
                operand: profile_offset_operand(node, state)?,
            }
        }
    };
    let dimension_port = require_port(node, IntentPortRole::Dimension, 0)?;
    let source_port = require_port(node, IntentPortRole::Source, 0)?;
    let dimension = match state.port_bindings.get(&dimension_port.as_ref(node.id)) {
        Some(IntentNativeBinding::Dimension(id)) => *id,
        _ => return Err(IntentMaterializationError::NativeKindMismatch { node: node.id }),
    };
    let source_id = match state.port_bindings.get(&source_port.as_ref(node.id)) {
        Some(IntentNativeBinding::Source(id)) => *id,
        _ => return Err(IntentMaterializationError::NativeKindMismatch { node: node.id }),
    };
    batch.push_dimension(DocumentDimension {
        id: dimension,
        source_id,
        label: node.symbol.as_str().to_owned(),
        mode: dimension_mode(node)?,
        suppressed: false,
        definition,
    });
    batch.push_source(source_id);
    state.consume_port(node, dimension_port);
    Ok(())
}

fn require_port(
    node: &IntentNode,
    role: IntentPortRole,
    index: u16,
) -> Result<&IntentPort, IntentMaterializationError> {
    let selector = IntentPortSelector::Node { role, index };
    node.port_by_selector(selector)
        .ok_or(IntentMaterializationError::MissingPort {
            node: node.id,
            selector,
        })
}

fn require_point_binding(
    node: NodeId,
    binding: Option<&IntentNativeBinding>,
) -> Result<DesignPointId, IntentMaterializationError> {
    match binding {
        Some(IntentNativeBinding::Point(point)) => Ok(*point),
        _ => Err(IntentMaterializationError::NativeKindMismatch { node }),
    }
}

fn point_position(
    candidate: &dyn IntentMaterializationSource,
    node: &IntentNode,
    port: &IntentPort,
    default: [f64; 2],
) -> Result<[f64; 2], IntentMaterializationError> {
    Ok([
        leaf_value(candidate, node.id, port.id, LeafField::X, default[0])?,
        leaf_value(candidate, node.id, port.id, LeafField::Y, default[1])?,
    ])
}

fn resolve_point_position(
    candidate: &dyn IntentMaterializationSource,
    node: &IntentNode,
    port: &IntentPort,
    point: DesignPointId,
    default: [f64; 2],
    state: &LoweringState,
) -> Result<[f64; 2], IntentMaterializationError> {
    if matches!(port.flow, IntentIdentityFlow::Created { .. }) {
        point_position(candidate, node, port, default)
    } else {
        state.point_positions.get(&point).copied().ok_or(
            IntentMaterializationError::MissingPointPosition {
                node: node.id,
                point,
            },
        )
    }
}

fn leaf_value(
    candidate: &dyn IntentMaterializationSource,
    node: NodeId,
    port: geosolve_sketch_intent::PortId,
    field: LeafField,
    default: f64,
) -> Result<f64, IntentMaterializationError> {
    leaf_quantity(candidate, node, port, field, default, IntentUnit::Length)
}

fn leaf_quantity(
    candidate: &dyn IntentMaterializationSource,
    node: NodeId,
    port: geosolve_sketch_intent::PortId,
    field: LeafField,
    default: f64,
    expected_unit: IntentUnit,
) -> Result<f64, IntentMaterializationError> {
    let leaf = LeafRef { node, port, field };
    let Some(value) = candidate.instance().values().get(&leaf) else {
        return Ok(default);
    };
    match value {
        IntentLiteral::Quantity {
            value,
            unit: actual_unit,
        } if *actual_unit == expected_unit && value.is_finite() => Ok(*value),
        _ => Err(IntentMaterializationError::InvalidWritableLeaf { leaf }),
    }
}

fn field_value<'a>(node: &'a IntentNode, name: &str) -> Option<&'a IntentLiteral> {
    node.fields
        .iter()
        .find_map(|(key, value)| (key.0.as_str() == name).then_some(value))
}

const fn invalid_operation(node: &IntentNode, reason: &'static str) -> IntentMaterializationError {
    IntentMaterializationError::InvalidOperation {
        node: node.id,
        reason,
    }
}

fn required_point(node: &IntentNode, name: &str) -> Result<[f64; 2], IntentMaterializationError> {
    point_field(node, name)?
        .ok_or_else(|| invalid_operation(node, "required operation point field is missing"))
}

fn required_quantity(
    node: &IntentNode,
    name: &str,
    unit: IntentUnit,
) -> Result<f64, IntentMaterializationError> {
    field_quantity(node, name, unit)?
        .ok_or_else(|| invalid_operation(node, "required operation quantity field is missing"))
}

fn required_enum<'a>(
    node: &'a IntentNode,
    name: &str,
) -> Result<&'a str, IntentMaterializationError> {
    enum_field(node, name)?
        .ok_or_else(|| invalid_operation(node, "required operation enum field is missing"))
}

fn required_boolean(node: &IntentNode, name: &str) -> Result<bool, IntentMaterializationError> {
    match field_value(node, name) {
        Some(IntentLiteral::Boolean(value)) => Ok(*value),
        Some(_) => Err(invalid_operation(
            node,
            "operation boolean field has the wrong type",
        )),
        None => Err(invalid_operation(
            node,
            "required operation boolean field is missing",
        )),
    }
}

fn required_i32(node: &IntentNode, name: &str) -> Result<i32, IntentMaterializationError> {
    match field_value(node, name) {
        Some(IntentLiteral::Integer(value)) => i32::try_from(*value)
            .map_err(|_| invalid_operation(node, "operation integer exceeds i32 limits")),
        Some(_) => Err(invalid_operation(
            node,
            "operation integer field has the wrong type",
        )),
        None => Err(invalid_operation(
            node,
            "required operation integer field is missing",
        )),
    }
}

fn required_usize(node: &IntentNode, name: &str) -> Result<usize, IntentMaterializationError> {
    match field_value(node, name) {
        Some(IntentLiteral::Natural(value)) => usize::try_from(*value)
            .map_err(|_| invalid_operation(node, "operation natural exceeds platform limits")),
        Some(_) => Err(invalid_operation(
            node,
            "operation natural field has the wrong type",
        )),
        None => Err(invalid_operation(
            node,
            "required operation natural field is missing",
        )),
    }
}

fn require_absent(node: &IntentNode, names: &[&str]) -> Result<(), IntentMaterializationError> {
    if names.iter().any(|name| field_value(node, name).is_some()) {
        return Err(invalid_operation(
            node,
            "operation carries fields which are invalid for its explicit branch",
        ));
    }
    Ok(())
}

fn split_retained_piece(
    node: &IntentNode,
    name: &str,
) -> Result<SplitRetainedPiece, IntentMaterializationError> {
    match required_enum(node, name)? {
        "before" => Ok(SplitRetainedPiece::Before),
        "after" => Ok(SplitRetainedPiece::After),
        _ => Err(invalid_operation(node, "split retained piece is invalid")),
    }
}

fn trim_retained_side(
    node: &IntentNode,
    name: &str,
) -> Result<TrimRetainedSide, IntentMaterializationError> {
    match required_enum(node, name)? {
        "before" => Ok(TrimRetainedSide::Before),
        "after" => Ok(TrimRetainedSide::After),
        _ => Err(invalid_operation(node, "trim retained side is invalid")),
    }
}

fn line_endpoint(
    node: &IntentNode,
    name: &str,
) -> Result<LineEndpoint, IntentMaterializationError> {
    match required_enum(node, name)? {
        "start" => Ok(LineEndpoint::Start),
        "end" => Ok(LineEndpoint::End),
        _ => Err(invalid_operation(node, "line endpoint is invalid")),
    }
}

fn required_dimension_mode(
    node: &IntentNode,
    name: &str,
) -> Result<DocumentDimensionMode, IntentMaterializationError> {
    match required_enum(node, name)? {
        "driving" => Ok(DocumentDimensionMode::Driving),
        "reference" => Ok(DocumentDimensionMode::Reference),
        _ => Err(invalid_operation(
            node,
            "operation dimension mode is invalid",
        )),
    }
}

fn required_arc_sweep(node: &IntentNode) -> Result<DocumentArcSweep, IntentMaterializationError> {
    match required_enum(node, "sweep")? {
        "counter_clockwise" => Ok(DocumentArcSweep::CounterClockwise),
        "clockwise" => Ok(DocumentArcSweep::Clockwise),
        _ => Err(invalid_operation(node, "operation arc sweep is invalid")),
    }
}

fn required_fillet_endpoint_order(
    node: &IntentNode,
) -> Result<DocumentFilletEndpointOrder, IntentMaterializationError> {
    match required_enum(node, "endpoint_order")? {
        "first_then_second" => Ok(DocumentFilletEndpointOrder::FirstThenSecond),
        "second_then_first" => Ok(DocumentFilletEndpointOrder::SecondThenFirst),
        _ => Err(invalid_operation(
            node,
            "operation fillet endpoint order is invalid",
        )),
    }
}

fn required_curve_normal_side(
    node: &IntentNode,
    name: &str,
) -> Result<DocumentCurveNormalSide, IntentMaterializationError> {
    match required_enum(node, name)? {
        "left" => Ok(DocumentCurveNormalSide::Left),
        "right" => Ok(DocumentCurveNormalSide::Right),
        _ => Err(invalid_operation(
            node,
            "operation curve normal side is invalid",
        )),
    }
}

fn required_fillet_trim_endpoint(
    node: &IntentNode,
    name: &str,
) -> Result<DocumentFilletTrimEndpoint, IntentMaterializationError> {
    match required_enum(node, name)? {
        "start" => Ok(DocumentFilletTrimEndpoint::Start),
        "end" => Ok(DocumentFilletTrimEndpoint::End),
        _ => Err(invalid_operation(
            node,
            "operation fillet trim endpoint is invalid",
        )),
    }
}

fn required_geometry_role(node: &IntentNode) -> Result<GeometryRole, IntentMaterializationError> {
    match required_enum(node, "role")? {
        "profile" => Ok(GeometryRole::Profile),
        "construction" => Ok(GeometryRole::Construction),
        _ => Err(invalid_operation(
            node,
            "source-free operation geometry role is invalid",
        )),
    }
}

fn text_field<'a>(
    node: &'a IntentNode,
    name: &str,
) -> Result<Option<&'a str>, IntentMaterializationError> {
    match field_value(node, name) {
        Some(IntentLiteral::Text(value)) => Ok(Some(value.as_str())),
        Some(_) => Err(IntentMaterializationError::InvalidGeometry {
            node: node.id,
            reason: "text-valued declaration field is invalid",
        }),
        None => Ok(None),
    }
}

fn point_field(
    node: &IntentNode,
    name: &str,
) -> Result<Option<[f64; 2]>, IntentMaterializationError> {
    match field_value(node, name) {
        Some(IntentLiteral::Point(point)) if point.iter().copied().all(f64::is_finite) => {
            Ok(Some(*point))
        }
        Some(_) => Err(IntentMaterializationError::InvalidGeometry {
            node: node.id,
            reason: "point-valued recipe field is invalid",
        }),
        None => Ok(None),
    }
}

fn field_quantity(
    node: &IntentNode,
    name: &str,
    expected_unit: IntentUnit,
) -> Result<Option<f64>, IntentMaterializationError> {
    match field_value(node, name) {
        Some(IntentLiteral::Quantity { value, unit })
            if *unit == expected_unit && value.is_finite() =>
        {
            Ok(Some(*value))
        }
        Some(_) => Err(IntentMaterializationError::InvalidGeometry {
            node: node.id,
            reason: "quantity-valued definition field is invalid",
        }),
        None => Ok(None),
    }
}

fn field_boolean(
    node: &IntentNode,
    name: &str,
    default: bool,
) -> Result<bool, IntentMaterializationError> {
    match field_value(node, name) {
        Some(IntentLiteral::Boolean(value)) => Ok(*value),
        Some(_) => Err(IntentMaterializationError::InvalidGeometry {
            node: node.id,
            reason: "boolean-valued definition field is invalid",
        }),
        None => Ok(default),
    }
}

fn field_integer(
    node: &IntentNode,
    name: &str,
    default: i64,
) -> Result<i64, IntentMaterializationError> {
    match field_value(node, name) {
        Some(IntentLiteral::Integer(value)) => Ok(*value),
        Some(_) => Err(IntentMaterializationError::InvalidGeometry {
            node: node.id,
            reason: "integer-valued definition field is invalid",
        }),
        None => Ok(default),
    }
}

fn field_natural(
    node: &IntentNode,
    name: &str,
    default: u64,
) -> Result<u64, IntentMaterializationError> {
    match field_value(node, name) {
        Some(IntentLiteral::Natural(value)) => Ok(*value),
        Some(_) => Err(IntentMaterializationError::InvalidGeometry {
            node: node.id,
            reason: "natural-valued definition field is invalid",
        }),
        None => Ok(default),
    }
}

fn enum_field<'a>(
    node: &'a IntentNode,
    name: &str,
) -> Result<Option<&'a str>, IntentMaterializationError> {
    match field_value(node, name) {
        Some(IntentLiteral::Enum(value)) => Ok(Some(value.as_str())),
        Some(_) => Err(IntentMaterializationError::InvalidGeometry {
            node: node.id,
            reason: "enum-valued definition field is invalid",
        }),
        None => Ok(None),
    }
}

fn geometry_role(node: &IntentNode) -> Result<GeometryRole, IntentMaterializationError> {
    match enum_field(node, "role")? {
        None | Some("profile") => Ok(GeometryRole::Profile),
        Some("construction") => Ok(GeometryRole::Construction),
        Some(_) => Err(IntentMaterializationError::InvalidGeometry {
            node: node.id,
            reason: "geometry role must be profile or construction",
        }),
    }
}

fn coordinate_axis(
    node: &IntentNode,
    name: &str,
) -> Result<DocumentCoordinateAxis, IntentMaterializationError> {
    match enum_field(node, name)? {
        None | Some("x") => Ok(DocumentCoordinateAxis::X),
        Some("y") => Ok(DocumentCoordinateAxis::Y),
        Some(_) => Err(IntentMaterializationError::InvalidGeometry {
            node: node.id,
            reason: "coordinate axis must be x or y",
        }),
    }
}

fn direction_sense(
    node: &IntentNode,
    name: &str,
) -> Result<DocumentDirectionSense, IntentMaterializationError> {
    match enum_field(node, name)? {
        None | Some("forward") => Ok(DocumentDirectionSense::Forward),
        Some("reverse") => Ok(DocumentDirectionSense::Reverse),
        Some(_) => Err(IntentMaterializationError::InvalidGeometry {
            node: node.id,
            reason: "direction sense must be forward or reverse",
        }),
    }
}

fn circle_tangency_mode(
    node: &IntentNode,
) -> Result<DocumentCircleTangencyMode, IntentMaterializationError> {
    match enum_field(node, "mode")? {
        None | Some("external") => Ok(DocumentCircleTangencyMode::External),
        Some("first_contains_second") => Ok(DocumentCircleTangencyMode::Internal {
            containment: DocumentCircleContainment::FirstContainsSecond,
        }),
        Some("second_contains_first") => Ok(DocumentCircleTangencyMode::Internal {
            containment: DocumentCircleContainment::SecondContainsFirst,
        }),
        Some(_) => Err(IntentMaterializationError::InvalidGeometry {
            node: node.id,
            reason: "circle tangency mode is invalid",
        }),
    }
}

fn dimension_mode(node: &IntentNode) -> Result<DocumentDimensionMode, IntentMaterializationError> {
    match enum_field(node, "mode")? {
        None | Some("driving") => Ok(DocumentDimensionMode::Driving),
        Some("reference") => Ok(DocumentDimensionMode::Reference),
        Some(_) => Err(IntentMaterializationError::InvalidGeometry {
            node: node.id,
            reason: "dimension mode must be driving or reference",
        }),
    }
}

fn angle_orientation(
    node: &IntentNode,
) -> Result<DocumentAngleOrientation, IntentMaterializationError> {
    match enum_field(node, "orientation")? {
        None | Some("counter_clockwise") => Ok(DocumentAngleOrientation::CounterClockwise),
        Some("clockwise") => Ok(DocumentAngleOrientation::Clockwise),
        Some(_) => Err(IntentMaterializationError::InvalidGeometry {
            node: node.id,
            reason: "angle orientation must be counter_clockwise or clockwise",
        }),
    }
}

fn line_side(node: &IntentNode) -> Result<DocumentLineSide, IntentMaterializationError> {
    match enum_field(node, "side")? {
        None | Some("left") => Ok(DocumentLineSide::Left),
        Some("right") => Ok(DocumentLineSide::Right),
        Some(_) => Err(IntentMaterializationError::InvalidGeometry {
            node: node.id,
            reason: "line side must be left or right",
        }),
    }
}

fn line_offset_orientation(
    node: &IntentNode,
) -> Result<DocumentLineOffsetOrientation, IntentMaterializationError> {
    match enum_field(node, "orientation")? {
        None | Some("same") => Ok(DocumentLineOffsetOrientation::Same),
        Some("reversed") => Ok(DocumentLineOffsetOrientation::Reversed),
        Some(_) => Err(IntentMaterializationError::InvalidGeometry {
            node: node.id,
            reason: "line-offset orientation must be same or reversed",
        }),
    }
}

fn arc_tangency_side(
    node: &IntentNode,
) -> Result<DocumentArcTangencySide, IntentMaterializationError> {
    match enum_field(node, "side")? {
        None | Some("outside_arc") => Ok(DocumentArcTangencySide::OutsideArc),
        Some("inside_arc") => Ok(DocumentArcTangencySide::InsideArc),
        Some(_) => Err(IntentMaterializationError::InvalidGeometry {
            node: node.id,
            reason: "arc tangency side must be outside_arc or inside_arc",
        }),
    }
}

fn feature_endpoint(
    node: &IntentNode,
    name: &str,
) -> Result<FeatureEndpoint, IntentMaterializationError> {
    match enum_field(node, name)? {
        None | Some("start") => Ok(FeatureEndpoint::Start),
        Some("end") => Ok(FeatureEndpoint::End),
        Some(_) => Err(IntentMaterializationError::InvalidGeometry {
            node: node.id,
            reason: "feature endpoint must be start or end",
        }),
    }
}

fn tangent_orientation(
    node: &IntentNode,
    name: &str,
) -> Result<TangentOrientation, IntentMaterializationError> {
    match enum_field(node, name)? {
        None | Some("aligned") => Ok(TangentOrientation::Aligned),
        Some("opposed") => Ok(TangentOrientation::Opposed),
        Some(_) => Err(IntentMaterializationError::InvalidGeometry {
            node: node.id,
            reason: "tangent orientation must be aligned or opposed",
        }),
    }
}

fn curve_normal_side(
    node: &IntentNode,
    name: &str,
) -> Result<DocumentCurveNormalSide, IntentMaterializationError> {
    match enum_field(node, name)? {
        None | Some("left") => Ok(DocumentCurveNormalSide::Left),
        Some("right") => Ok(DocumentCurveNormalSide::Right),
        Some(_) => Err(IntentMaterializationError::InvalidGeometry {
            node: node.id,
            reason: "curve normal side must be left or right",
        }),
    }
}

fn curve_direction_relation(
    node: &IntentNode,
) -> Result<DocumentCurveDirectionRelation, IntentMaterializationError> {
    match enum_field(node, "relation")? {
        None | Some("tangent") => Ok(DocumentCurveDirectionRelation::Tangent {
            orientation: tangent_orientation(node, "orientation")?,
        }),
        Some("normal") => Ok(DocumentCurveDirectionRelation::Normal {
            side: curve_normal_side(node, "side")?,
        }),
        Some(_) => Err(IntentMaterializationError::InvalidGeometry {
            node: node.id,
            reason: "curve direction relation must be tangent or normal",
        }),
    }
}

fn curvature_relation(
    node: &IntentNode,
) -> Result<DocumentCurveCurvatureRelation, IntentMaterializationError> {
    match enum_field(node, "relation")? {
        None | Some("signed") => Ok(DocumentCurveCurvatureRelation::Signed),
        Some("magnitude_same_sign") => Ok(DocumentCurveCurvatureRelation::MagnitudeSameSign),
        Some("magnitude_opposite_sign") => {
            Ok(DocumentCurveCurvatureRelation::MagnitudeOppositeSign)
        }
        Some(_) => Err(IntentMaterializationError::InvalidGeometry {
            node: node.id,
            reason: "curve curvature relation is invalid",
        }),
    }
}

fn curve_continuity(
    node: &IntentNode,
) -> Result<DocumentCurveContinuity, IntentMaterializationError> {
    match enum_field(node, "continuity")? {
        None | Some("g0") => Ok(DocumentCurveContinuity::G0),
        Some("g1") => Ok(DocumentCurveContinuity::G1),
        Some("g2") => Ok(DocumentCurveContinuity::G2),
        Some("parametric_c2") => {
            let first_rate = field_quantity(node, "first_rate", IntentUnit::Dimensionless)?
                .filter(|value| *value > 0.0)
                .ok_or(IntentMaterializationError::InvalidGeometry {
                    node: node.id,
                    reason: "parametric C2 first rate must be finite and positive",
                })?;
            let second_rate = field_quantity(node, "second_rate", IntentUnit::Dimensionless)?
                .filter(|value| *value > 0.0)
                .ok_or(IntentMaterializationError::InvalidGeometry {
                    node: node.id,
                    reason: "parametric C2 second rate must be finite and positive",
                })?;
            Ok(DocumentCurveContinuity::ParametricC2 {
                first_rate,
                second_rate,
            })
        }
        Some(_) => Err(IntentMaterializationError::InvalidGeometry {
            node: node.id,
            reason: "curve continuity must be g0, g1, g2, or parametric_c2",
        }),
    }
}

fn fillet_endpoint_order(
    node: &IntentNode,
) -> Result<DocumentFilletEndpointOrder, IntentMaterializationError> {
    match enum_field(node, "endpoint_order")? {
        None | Some("first_then_second") => Ok(DocumentFilletEndpointOrder::FirstThenSecond),
        Some("second_then_first") => Ok(DocumentFilletEndpointOrder::SecondThenFirst),
        Some(_) => Err(IntentMaterializationError::InvalidGeometry {
            node: node.id,
            reason: "fillet endpoint order is invalid",
        }),
    }
}

fn fillet_trim_endpoint(
    node: &IntentNode,
    name: &str,
) -> Result<DocumentFilletTrimEndpoint, IntentMaterializationError> {
    match enum_field(node, name)? {
        None | Some("start") => Ok(DocumentFilletTrimEndpoint::Start),
        Some("end") => Ok(DocumentFilletTrimEndpoint::End),
        Some(_) => Err(IntentMaterializationError::InvalidGeometry {
            node: node.id,
            reason: "fillet trim endpoint must be start or end",
        }),
    }
}

fn arc_sweep(node: &IntentNode) -> Result<DocumentArcSweep, IntentMaterializationError> {
    match field_value(node, "sweep") {
        None => Ok(DocumentArcSweep::CounterClockwise),
        Some(IntentLiteral::Enum(value)) if value.as_str() == "counter_clockwise" => {
            Ok(DocumentArcSweep::CounterClockwise)
        }
        Some(IntentLiteral::Enum(value)) if value.as_str() == "clockwise" => {
            Ok(DocumentArcSweep::Clockwise)
        }
        Some(_) => Err(IntentMaterializationError::InvalidGeometry {
            node: node.id,
            reason: "arc sweep must be counter_clockwise or clockwise",
        }),
    }
}

fn hyperbola_branch(
    node: &IntentNode,
) -> Result<DocumentHyperbolaBranch, IntentMaterializationError> {
    match field_value(node, "branch") {
        None => Ok(DocumentHyperbolaBranch::Positive),
        Some(IntentLiteral::Enum(value)) if value.as_str() == "positive" => {
            Ok(DocumentHyperbolaBranch::Positive)
        }
        Some(IntentLiteral::Enum(value)) if value.as_str() == "negative" => {
            Ok(DocumentHyperbolaBranch::Negative)
        }
        Some(_) => Err(IntentMaterializationError::InvalidGeometry {
            node: node.id,
            reason: "hyperbola branch must be positive or negative",
        }),
    }
}

fn branch_direction(
    node: &IntentNode,
    start: [f64; 2],
    end: [f64; 2],
) -> Result<[f64; 2], IntentMaterializationError> {
    let explicit = node
        .fields
        .iter()
        .find(|(field, _)| field.0.as_str() == "branch_direction")
        .map(|(_, value)| value);
    let direction = match explicit {
        Some(IntentLiteral::Point(direction)) => {
            let length = direction[0].hypot(direction[1]);
            if length.is_finite() && length > 0.0 && (length - 1.0).abs() <= 64.0 * f64::EPSILON {
                return Ok(*direction);
            }
            return Err(IntentMaterializationError::InvalidBranchDirection { node: node.id });
        }
        Some(_) => {
            return Err(IntentMaterializationError::InvalidBranchDirection { node: node.id });
        }
        None => [end[0] - start[0], end[1] - start[1]],
    };
    let length = direction[0].hypot(direction[1]);
    if !length.is_finite() {
        return Err(IntentMaterializationError::InvalidBranchDirection { node: node.id });
    }
    if length <= f64::EPSILON {
        return Ok([1.0, 0.0]);
    }
    Ok([direction[0] / length, direction[1] / length])
}

// Keep the typed native namespace constructor visible without inventing a
// process-global materializer identity.
const _: Option<(DocumentId, PersistentId, IntentPortKind)> = None;
