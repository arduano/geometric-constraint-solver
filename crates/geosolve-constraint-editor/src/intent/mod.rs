// SPDX-License-Identifier: GPL-3.0-or-later

//! Deterministic lowering from projectional sketch intent into the existing
//! persistent sketch and retained native solver.
//!
//! This module owns translation and semantic/native ownership. It deliberately
//! owns no residual equation: acceptance remains the responsibility of
//! [`RetainedSketchDocumentSession`].

use std::collections::{BTreeMap, BTreeSet};

use geosolve_sketch::{
    CurveDefinition, CurveId, CurveSpan, DesignCurve, DesignPoint, DesignPointId, DesignScalarId,
    DocumentConstraint, DocumentConstraintDefinition, DocumentConstraintId, DocumentError,
    DocumentExternalBindingId, DocumentId, DocumentParameterId, DocumentSessionError,
    DocumentSolveRequest, DocumentSourceId, PersistentId, RetainedSketchDocumentSession,
    SKETCH_ACCEPTANCE_RESIDUAL_TOLERANCE, SketchHardValidity, SketchMaterializationBatch,
    SketchMaterializationReservationAllocator, SketchPersistentIdentityHighWater, SolverConfig,
};
use geosolve_sketch_intent::{
    ConstraintKind, GeometryRecipeKind, InputRole, InputSlot, IntentCandidate, IntentEvaluation,
    IntentEvaluationFailure, IntentEvaluationFailureKind, IntentGraphError, IntentIdentityFlow,
    IntentKey, IntentLiteral, IntentNativeReservationKind, IntentNode, IntentNodeKind, IntentPort,
    IntentPortKind, IntentPortRef, IntentPortRole, IntentPortSelector, IntentReservationRecord,
    IntentReservationState, IntentSemanticIdentity, IntentUnit, LeafField, LeafRef,
    MaterializationEvidence, NodeId, ReservationId,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

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
    Constraint(DocumentConstraintId),
    Source(DocumentSourceId),
    Parameter(DocumentParameterId),
    ExternalBinding(DocumentExternalBindingId),
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
}

impl IntentMaterializationMap {
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
}

/// One complete cold reconstruction suitable for atomic coordinator staging.
#[derive(Clone, Debug)]
pub struct ColdIntentMaterialization {
    pub session: RetainedSketchDocumentSession,
    pub ownership: IntentMaterializationMap,
    pub validation: IntentValidationEvidence,
    pub evidence: MaterializationEvidence,
}

/// Editor-owned cold materializer configuration.
#[derive(Clone, Debug)]
pub struct ColdIntentMaterializer {
    document: DocumentId,
    model_scale: f64,
    request: DocumentSolveRequest,
    config: SolverConfig,
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
        })
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

    /// Cold-reconstructs Point, Segment and Horizontal declarations in canonical
    /// dependency order and publishes only a currently accepted native session.
    ///
    /// # Errors
    ///
    /// Returns a typed graph, lowering, reservation, document, or native-solver
    /// rejection. Scratch state is dropped on every error.
    pub fn materialize(
        &self,
        candidate: &IntentCandidate,
    ) -> Result<ColdIntentMaterialization, IntentMaterializationError> {
        candidate.graph().validate()?;
        preflight_supported(candidate)?;

        let mut document = SketchDocumentBuilder::empty(self.document, self.model_scale)?;
        let (reservation_set, reservation_bindings) = allocate_reservations(
            document.persistent_identity_high_water().clone(),
            candidate.reservations().entries(),
        )?;
        let retains_unused = candidate
            .reservations()
            .entries()
            .values()
            .any(|record| record.state != IntentReservationState::Declared);
        let mut batch = if retains_unused {
            SketchMaterializationBatch::retaining_unused_reservations(reservation_set)
        } else {
            SketchMaterializationBatch::new(reservation_set)
        };
        let mut state = LoweringState::new(candidate.semantic_identity(), reservation_bindings);

        for node_id in candidate.graph().canonical_schedule()? {
            let node = candidate
                .graph()
                .node(node_id)
                .ok_or(IntentMaterializationError::UnknownNode(node_id))?;
            state.bind_schema_ports(node)?;
            if node.suppressed {
                continue;
            }
            match &node.kind {
                IntentNodeKind::Geometry {
                    recipe: GeometryRecipeKind::SketchPoint,
                } => lower_point(candidate, node, &mut batch, &mut state)?,
                IntentNodeKind::Geometry {
                    recipe: GeometryRecipeKind::Segment,
                } => lower_segment(candidate, node, &mut batch, &mut state)?,
                IntentNodeKind::Constraint {
                    constraint: ConstraintKind::Horizontal,
                } => lower_horizontal(node, &mut batch, &mut state)?,
                _ => unreachable!("preflight admits only the focused cold-lowering slice"),
            }
        }
        state.validate_declared_consumption(candidate)?;
        document.apply_materialization_batch(&batch)?;

        let session = RetainedSketchDocumentSession::new(document, self.request, self.config)?;
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

        let ownership = state.finish(candidate.reservations().entries());
        let validation = IntentValidationEvidence {
            semantic: candidate.semantic_identity(),
            document: accepted.document().id(),
            point_count: accepted.document().points().len(),
            curve_count: accepted.document().curves().len(),
            constraint_count: accepted.document().constraints().len(),
            hard_residuals_validated: solve.hard_residuals_validated,
            maximum_normalized_hard_residual: maximum,
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
            ownership,
            validation,
            evidence,
        })
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
        match self.materialize(candidate) {
            Ok(materialized) => IntentEvaluation::Accepted {
                evidence: materialized.evidence,
            },
            Err(error) => IntentEvaluation::Failed {
                failure: IntentEvaluationFailure {
                    kind: error.failure_kind(),
                    failed_nodes: error.failed_node().into_iter().collect(),
                    diagnostic: IntentKey::new(error.diagnostic_key())
                        .expect("static intent materialization diagnostics are valid keys"),
                },
            },
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
    #[error("intent node {node} is outside the focused Point/Segment/Horizontal materializer")]
    UnsupportedNode { node: NodeId },
    #[error("the focused cold materializer does not yet decode non-empty host inputs")]
    UnsupportedHostInputs,
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
    #[error("declared reservation {reservation} was not materialized")]
    UnconsumedReservation { reservation: ReservationId },
    #[error("writable leaf {leaf} must be a finite length quantity")]
    InvalidWritableLeaf { leaf: LeafRef },
    #[error("line node {node} has an invalid explicit branch direction")]
    InvalidBranchDirection { node: NodeId },
    #[error("native solver rejected the cold materialization")]
    SolverRejected,
    #[error("native solver omitted stable acceptance evidence")]
    MissingValidationEvidence,
    #[error("native independent hard-residual validation did not pass")]
    IndependentValidationFailed,
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
            | Self::UnknownNode(node) => Some(*node),
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
            Self::UnsupportedHostInputs => "unsupported-intent-host-inputs",
            Self::SolverRejected => "native-solver-rejected",
            Self::IndependentValidationFailed | Self::MissingValidationEvidence => {
                "native-validation-rejected"
            }
            Self::Graph(_) => "invalid-intent-graph",
            Self::Document(_) | Self::Session(_) => "native-materialization-rejected",
            Self::Evidence(_) | Self::Json(_) => "materialization-evidence-rejected",
            _ => "invalid-intent-lowering",
        }
    }
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

fn preflight_supported(candidate: &IntentCandidate) -> Result<(), IntentMaterializationError> {
    if !candidate.external_inputs().parameter_batch.is_empty()
        || !candidate.external_inputs().external_snapshots.is_empty()
    {
        return Err(IntentMaterializationError::UnsupportedHostInputs);
    }
    for node in candidate.graph().nodes().values() {
        if !matches!(
            node.kind,
            IntentNodeKind::Geometry {
                recipe: GeometryRecipeKind::SketchPoint | GeometryRecipeKind::Segment
            } | IntentNodeKind::Constraint {
                constraint: ConstraintKind::Horizontal
            }
        ) {
            return Err(IntentMaterializationError::UnsupportedNode { node: node.id });
        }
    }
    Ok(())
}

fn allocate_reservations(
    high_water: SketchPersistentIdentityHighWater,
    records: &BTreeMap<ReservationId, IntentReservationRecord>,
) -> Result<
    (
        geosolve_sketch::SketchMaterializationReservationSet,
        BTreeMap<ReservationId, IntentNativeBinding>,
    ),
    IntentMaterializationError,
> {
    let mut allocator = SketchMaterializationReservationAllocator::new(high_water)?;
    let mut bindings = BTreeMap::new();
    let mut paired = BTreeSet::new();
    for (reservation, record) in records {
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
            IntentNativeReservationKind::Constraint => {
                let companion = require_pair(
                    *reservation,
                    record,
                    records,
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
                    records,
                    IntentNativeReservationKind::DimensionSource,
                )?;
                let native = allocator.reserve_dimension()?;
                // The focused map has no dimension binding yet, but the source
                // identity still cannot be silently reused.
                bindings.insert(companion, IntentNativeBinding::Source(native.source));
                paired.insert(companion);
                return Err(IntentMaterializationError::UnsupportedNode {
                    node: record.owner_node,
                });
            }
            IntentNativeReservationKind::DimensionSource
            | IntentNativeReservationKind::Contact
            | IntentNativeReservationKind::Parameter
            | IntentNativeReservationKind::ExternalBinding
            | IntentNativeReservationKind::SemanticCatalog
            | IntentNativeReservationKind::SemanticSource => {
                return Err(IntentMaterializationError::UnsupportedNode {
                    node: record.owner_node,
                });
            }
        };
        bindings.insert(*reservation, binding);
    }
    Ok((allocator.finish()?, bindings))
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

struct LoweringState {
    semantic: IntentSemanticIdentity,
    reservation_bindings: BTreeMap<ReservationId, IntentNativeBinding>,
    port_bindings: BTreeMap<IntentPortRef, IntentNativeBinding>,
    reverse_leaves: BTreeMap<IntentNativeWritableLeaf, LeafRef>,
    point_positions: BTreeMap<DesignPointId, [f64; 2]>,
    consumed_reservations: BTreeSet<ReservationId>,
}

impl LoweringState {
    fn new(
        semantic: IntentSemanticIdentity,
        reservation_bindings: BTreeMap<ReservationId, IntentNativeBinding>,
    ) -> Self {
        Self {
            semantic,
            reservation_bindings,
            port_bindings: BTreeMap::new(),
            reverse_leaves: BTreeMap::new(),
            point_positions: BTreeMap::new(),
            consumed_reservations: BTreeSet::new(),
        }
    }

    fn bind_schema_ports(&mut self, node: &IntentNode) -> Result<(), IntentMaterializationError> {
        for port in node.ports.values() {
            let reference = port.as_ref(node.id);
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
                IntentIdentityFlow::OwnedLogical | IntentIdentityFlow::Retired { .. } => None,
            };
            if let Some(binding) = binding {
                self.port_bindings.insert(reference, binding);
                if matches!(port.flow, IntentIdentityFlow::Created { .. }) {
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
            self.reverse_leaves.insert(
                native,
                LeafRef {
                    node,
                    port: port.id,
                    field: *field,
                },
            );
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

    fn validate_declared_consumption(
        &self,
        candidate: &IntentCandidate,
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
        let mut owned = BTreeMap::<NodeId, Vec<IntentNativeBinding>>::new();
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
        }
    }
}

fn lower_point(
    candidate: &IntentCandidate,
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

fn lower_segment(
    candidate: &IntentCandidate,
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
    batch.push_curve(DesignCurve {
        id: curve,
        label: node.symbol.as_str().to_owned(),
        definition: CurveDefinition::Line {
            start,
            end,
            branch_direction,
        },
    });
    state.consume_port(node, curve_port);
    let span_port = require_port(node, IntentPortRole::Span, 0)?;
    state.port_bindings.insert(
        span_port.as_ref(node.id),
        IntentNativeBinding::CurveSpan(CurveSpan::line(curve)),
    );
    Ok(())
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
    candidate: &IntentCandidate,
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
    candidate: &IntentCandidate,
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
    candidate: &IntentCandidate,
    node: NodeId,
    port: geosolve_sketch_intent::PortId,
    field: LeafField,
    default: f64,
) -> Result<f64, IntentMaterializationError> {
    let leaf = LeafRef { node, port, field };
    let Some(value) = candidate.instance().values().get(&leaf) else {
        return Ok(default);
    };
    match value {
        IntentLiteral::Quantity {
            value,
            unit: IntentUnit::Length,
        } if value.is_finite() => Ok(*value),
        _ => Err(IntentMaterializationError::InvalidWritableLeaf { leaf }),
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
        Some(IntentLiteral::Point(direction)) => *direction,
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
