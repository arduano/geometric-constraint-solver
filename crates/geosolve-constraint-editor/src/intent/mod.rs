// SPDX-License-Identifier: GPL-3.0-or-later

//! Deterministic lowering from projectional sketch intent into the existing
//! persistent sketch and retained native solver.
//!
//! This module owns translation and semantic/native ownership. It deliberately
//! owns no residual equation: acceptance remains the responsibility of
//! [`RetainedSketchDocumentSession`].

use std::collections::{BTreeMap, BTreeSet};

use geosolve_sketch::{
    ContactId, CurveDefinition, CurveId, CurveSpan, DesignCurve, DesignPoint, DesignPointId,
    DesignScalar, DesignScalarId, DocumentAngleOrientation, DocumentArcSweep, DocumentCenterRef,
    DocumentCircleContainment, DocumentCircleTangencyMode, DocumentConstraint,
    DocumentConstraintDefinition, DocumentConstraintId, DocumentCoordinateAxis, DocumentDimension,
    DocumentDimensionDefinition, DocumentDimensionId, DocumentDimensionMode,
    DocumentDirectionSense, DocumentError, DocumentExternalBindingId, DocumentHyperbolaBranch,
    DocumentId, DocumentLineOffsetOrientation, DocumentLineSide, DocumentLineSupportRef,
    DocumentParameterId, DocumentSessionError, DocumentSolveRequest, DocumentSourceId,
    MIN_RATIONAL_QUADRATIC_MIDDLE_WEIGHT, PersistentId, RetainedSketchDocumentSession,
    SKETCH_ACCEPTANCE_RESIDUAL_TOLERANCE, ScalarDomain, ScalarUnit, SketchHardValidity,
    SketchMaterializationBatch, SketchMaterializationReservationAllocator,
    SketchPersistentIdentityHighWater, SolverConfig,
};
use geosolve_sketch_intent::{
    ConstraintKind, DimensionKind, GeometryRecipeKind, InputRole, InputSlot,
    IntentAcceptedAuthority, IntentCandidate, IntentEvaluation, IntentEvaluationFailure,
    IntentEvaluationFailureKind, IntentExternalInputs, IntentGraph, IntentGraphError,
    IntentIdentityFlow, IntentInstanceState, IntentKey, IntentLiteral, IntentNativeReservationKind,
    IntentNode, IntentNodeKind, IntentPort, IntentPortKind, IntentPortRef, IntentPortRole,
    IntentPortSelector, IntentReservationLedger, IntentReservationRecord, IntentReservationState,
    IntentSemanticIdentity, IntentUnit, LeafField, LeafRef, MaterializationEvidence, NodeId,
    ReservationId,
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
    Contact(ContactId),
    Constraint(DocumentConstraintId),
    Dimension(DocumentDimensionId),
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
        let output = self.materialize_source(authority)?;
        if output.evidence != authority.evidence {
            return Err(IntentMaterializationError::AcceptedAuthorityEvidenceMismatch);
        }
        Ok(output)
    }

    fn materialize_source(
        &self,
        candidate: &dyn IntentMaterializationSource,
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
                IntentNodeKind::Geometry { recipe } => {
                    lower_geometry(candidate, node, *recipe, &mut batch, &mut state)?;
                }
                IntentNodeKind::Constraint { constraint } => {
                    lower_constraint(node, *constraint, &mut batch, &mut state)?;
                }
                IntentNodeKind::Dimension { dimension } => {
                    lower_dimension(candidate, node, *dimension, &mut batch, &mut state)?;
                }
                _ => unreachable!("preflight admits only implemented declaration families"),
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
        match self.materialize(candidate) {
            Ok(materialized) => {
                let evaluation = IntentEvaluation::Accepted {
                    evidence: materialized.evidence.clone(),
                };
                (evaluation, Some(materialized))
            }
            Err(error) => (
                IntentEvaluation::Failed {
                    failure: IntentEvaluationFailure {
                        kind: error.failure_kind(),
                        failed_nodes: error.failed_node().into_iter().collect(),
                        diagnostic: IntentKey::new(error.diagnostic_key())
                            .expect("static intent materialization diagnostics are valid keys"),
                    },
                },
                None,
            ),
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
    #[error("intent node {node} is not materializable by this editor adapter")]
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
    #[error("geometry node {node} has invalid or incomplete recipe state: {reason}")]
    InvalidGeometry { node: NodeId, reason: &'static str },
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
            Self::AcceptedAuthorityIdentityMismatch => "accepted-authority-identity-mismatch",
            Self::AcceptedAuthorityEvidenceMismatch => "accepted-authority-evidence-mismatch",
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

fn preflight_supported(
    candidate: &dyn IntentMaterializationSource,
) -> Result<(), IntentMaterializationError> {
    if !candidate.external_inputs().parameter_batch.is_empty()
        || !candidate.external_inputs().external_snapshots.is_empty()
    {
        return Err(IntentMaterializationError::UnsupportedHostInputs);
    }
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
            ),
            IntentNodeKind::Constraint { constraint } => !matches!(
                constraint,
                ConstraintKind::ExternalPointCoincident
                    | ConstraintKind::ExternalLineCollinear
                    | ConstraintKind::PointOnCurve
                    | ConstraintKind::LineCircleTangency
                    | ConstraintKind::CircleArcTangency
                    | ConstraintKind::LineCurveTangency
                    | ConstraintKind::CurveCurveContact
                    | ConstraintKind::CurveCurveTangency
                    | ConstraintKind::CurveDirection
                    | ConstraintKind::EqualCurvature
                    | ConstraintKind::EndpointContinuity
                    | ConstraintKind::LineLineFillet
                    | ConstraintKind::CurveCurveFillet
            ),
            IntentNodeKind::Dimension { dimension } => dimension != DimensionKind::ProfileOffset,
            _ => false,
        };
        if !supported {
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
            IntentNativeReservationKind::Contact => {
                IntentNativeBinding::Contact(allocator.reserve_contact()?)
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
                IntentNativeBinding::Source(allocator.reserve_semantic_catalog()?)
            }
            IntentNativeReservationKind::SemanticSource
            | IntentNativeReservationKind::DimensionSource => {
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
        G::Polyline
        | G::MidpointLine
        | G::TwoPointAlignedRectangle
        | G::ThreePointCornerRectangle
        | G::CenterRectangle
        | G::ThreePointCenterRectangle
        | G::TangentArc
        | G::OpenControlNurbs
        | G::PeriodicControlNurbs => {
            Err(IntentMaterializationError::UnsupportedNode { node: node.id })
        }
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
    let curve_port = require_port(node, IntentPortRole::Curve, index)?;
    let curve = match state.port_bindings.get(&curve_port.as_ref(node.id)) {
        Some(IntentNativeBinding::Curve(curve)) => *curve,
        _ => return Err(IntentMaterializationError::NativeKindMismatch { node: node.id }),
    };
    batch.push_curve(DesignCurve {
        id: curve,
        label: if index == 0 {
            node.symbol.as_str().to_owned()
        } else {
            format!("{}.curve{}", node.symbol.as_str(), index + 1)
        },
        definition,
    });
    state.consume_port(node, curve_port);
    if let Some(span_port) = node.port_by_selector(IntentPortSelector::Node {
        role: IntentPortRole::Span,
        index,
    }) {
        state.port_bindings.insert(
            span_port.as_ref(node.id),
            IntentNativeBinding::CurveSpan(CurveSpan::line(curve)),
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

#[allow(
    clippy::too_many_lines,
    reason = "one exhaustive relation table keeps intent operands and native definitions aligned"
)]
fn lower_constraint(
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
        C::Horizontal
        | C::ExternalPointCoincident
        | C::ExternalLineCollinear
        | C::PointOnCurve
        | C::LineCircleTangency
        | C::CircleArcTangency
        | C::LineCurveTangency
        | C::CurveCurveContact
        | C::CurveCurveTangency
        | C::CurveDirection
        | C::EqualCurvature
        | C::EndpointContinuity
        | C::LineLineFillet
        | C::CurveCurveFillet => {
            return Err(IntentMaterializationError::UnsupportedNode { node: node.id });
        }
    };
    materialize_constraint(node, definition, batch, state)
}

fn materialize_constraint(
    node: &IntentNode,
    definition: DocumentConstraintDefinition,
    batch: &mut SketchMaterializationBatch,
    state: &mut LoweringState,
) -> Result<(), IntentMaterializationError> {
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
            return Err(IntentMaterializationError::UnsupportedNode { node: node.id });
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
