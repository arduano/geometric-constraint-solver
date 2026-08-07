// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::{BTreeMap, BTreeSet};

use geosolve_sketch::{
    ContactDomain, ContactNeighborhood, CurveDefinition, CurveSpan, DocumentArcSweep,
    DocumentCurveNormalSide, DocumentFilletEndpointOrder, DocumentFilletTrimEndpoint,
    DocumentTrimBoundary, DocumentTrimParameter, OperationCheckpoint, OperationControl,
    OperationController, OperationOutcome, OperationWorkCounter, PreparedSketchInput,
    RetainedSketchDocumentSession, SketchAcceptedStateIdentity, SketchDocument,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::document::{
    ComputedFeature, ComputedFeatureCornerId, ComputedFeatureDefinition, ComputedFeatureDocument,
    ComputedFeatureDocumentError, ComputedFeatureDocumentIdentity, ComputedFeatureId,
    ComputedFilletCorner, ComputedFilletParent, NativeCurveSpanSource, NewComputedFilletCorner,
};

const PARAMETER_EPSILON_FACTOR: f64 = 1.0e-10;
const GEOMETRY_TOLERANCE_FACTOR: f64 = 1.0e-8;
const ROOT_DEDUPLICATION_FACTOR: f64 = 1.0e-7;
const OFFSET_SINGULARITY_TOLERANCE: f64 = 1.0e-8;
const PARENT_SINGULARITY_TOLERANCE: f64 = 1.0e-8;
const TANGENCY_TOLERANCE: f64 = 2.0e-7;

/// Bounded deterministic computed-feature evaluation policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ComputedFeatureEvaluationPolicy {
    pub max_features: usize,
    pub max_corners: usize,
    pub max_edges: usize,
    pub root_seed_grid: usize,
    pub max_root_iterations: usize,
    pub max_line_search_steps: usize,
}

impl Default for ComputedFeatureEvaluationPolicy {
    fn default() -> Self {
        Self {
            max_features: 10_000,
            max_corners: 20_000,
            max_edges: 100_000,
            root_seed_grid: 3,
            max_root_iterations: 32,
            max_line_search_steps: 8,
        }
    }
}

impl ComputedFeatureEvaluationPolicy {
    fn validate(self) -> Result<(), ComputedFeatureEvaluationError> {
        let values = [
            ("max_features", self.max_features),
            ("max_corners", self.max_corners),
            ("max_edges", self.max_edges),
            ("root_seed_grid", self.root_seed_grid),
            ("max_root_iterations", self.max_root_iterations),
            ("max_line_search_steps", self.max_line_search_steps),
        ];
        if let Some((field, _)) = values.into_iter().find(|(_, value)| *value == 0) {
            return Err(ComputedFeatureEvaluationError::InvalidPolicy {
                field,
                message: "must be positive",
            });
        }
        if self.root_seed_grid > 16 {
            return Err(ComputedFeatureEvaluationError::InvalidPolicy {
                field: "root_seed_grid",
                message: "must not exceed 16",
            });
        }
        if self.max_root_iterations > 256 {
            return Err(ComputedFeatureEvaluationError::InvalidPolicy {
                field: "max_root_iterations",
                message: "must not exceed 256",
            });
        }
        if self.max_line_search_steps > 32 {
            return Err(ComputedFeatureEvaluationError::InvalidPolicy {
                field: "max_line_search_steps",
                message: "must not exceed 32",
            });
        }
        Ok(())
    }
}

/// Complete exact input stamp for one computed-feature evaluation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ComputedFeatureEvaluationInput {
    pub sketch: PreparedSketchInput,
    pub accepted: SketchAcceptedStateIdentity,
    pub features: ComputedFeatureDocumentIdentity,
    pub policy: ComputedFeatureEvaluationPolicy,
}

/// Coherent immutable accepted sketch plus feature intent.
#[derive(Clone, Debug)]
pub struct ComputedFeatureEvaluationSnapshot {
    input: ComputedFeatureEvaluationInput,
    sketch: SketchDocument,
    features: ComputedFeatureDocument,
}

#[allow(
    clippy::missing_errors_doc,
    reason = "capture and preparation failures are closed typed snapshot/evaluation errors"
)]
impl ComputedFeatureEvaluationSnapshot {
    /// Captures only current independently accepted sketch geometry and a sidecar
    /// bound to the same sketch namespace.
    pub fn capture(
        session: &RetainedSketchDocumentSession,
        features: &ComputedFeatureDocument,
        policy: ComputedFeatureEvaluationPolicy,
    ) -> Result<Self, ComputedFeatureSnapshotError> {
        let accepted = session
            .accepted_state_for_current_input()
            .ok_or(ComputedFeatureSnapshotError::CurrentAcceptedStateRequired)?;
        if features.sketch_document() != session.design_document().id() {
            return Err(ComputedFeatureSnapshotError::FeatureDocumentForDifferentSketch);
        }
        features
            .validate()
            .map_err(ComputedFeatureSnapshotError::InvalidFeatureDocument)?;
        let sketch = session
            .accepted_prepared_input()
            .ok_or(ComputedFeatureSnapshotError::AcceptedInputMismatch)?;
        if sketch.accepted_state_identity() != Some(accepted.identity())
            || accepted.input() != sketch.attempt_input()
        {
            return Err(ComputedFeatureSnapshotError::AcceptedInputMismatch);
        }
        Ok(Self {
            input: ComputedFeatureEvaluationInput {
                sketch,
                accepted: accepted.identity(),
                features: features.identity(),
                policy,
            },
            sketch: accepted.document().clone(),
            features: features.clone(),
        })
    }

    #[must_use]
    pub const fn input(&self) -> ComputedFeatureEvaluationInput {
        self.input
    }

    #[must_use]
    pub const fn sketch_document(&self) -> &SketchDocument {
        &self.sketch
    }

    #[must_use]
    pub const fn feature_document(&self) -> &ComputedFeatureDocument {
        &self.features
    }

    pub fn prepare(
        self,
        allocator: &mut ComputedEvaluationAllocator,
    ) -> Result<PreparedComputedFeatureEvaluation, ComputedFeatureEvaluationError> {
        Ok(PreparedComputedFeatureEvaluation {
            snapshot: self,
            evaluation: allocator.allocate()?,
        })
    }
}

/// Immutable accepted sketch used to resolve authoring picks into persistent intent.
#[derive(Clone, Debug)]
pub struct ComputedFeatureAuthoringSnapshot {
    sketch_input: PreparedSketchInput,
    accepted: SketchAcceptedStateIdentity,
    sketch: SketchDocument,
}

#[allow(
    clippy::missing_errors_doc,
    reason = "authoring capture and resolution failures are enumerated by their public error types"
)]
impl ComputedFeatureAuthoringSnapshot {
    /// Captures the same current accepted boundary used by evaluation.
    pub fn capture(
        session: &RetainedSketchDocumentSession,
    ) -> Result<Self, ComputedFeatureSnapshotError> {
        let accepted = session
            .accepted_state_for_current_input()
            .ok_or(ComputedFeatureSnapshotError::CurrentAcceptedStateRequired)?;
        let sketch_input = session
            .accepted_prepared_input()
            .ok_or(ComputedFeatureSnapshotError::AcceptedInputMismatch)?;
        if sketch_input.accepted_state_identity() != Some(accepted.identity())
            || accepted.input() != sketch_input.attempt_input()
        {
            return Err(ComputedFeatureSnapshotError::AcceptedInputMismatch);
        }
        Ok(Self {
            sketch_input,
            accepted: accepted.identity(),
            sketch: accepted.document().clone(),
        })
    }

    #[must_use]
    pub const fn sketch_input(&self) -> PreparedSketchInput {
        self.sketch_input
    }

    #[must_use]
    pub const fn accepted_state_identity(&self) -> SketchAcceptedStateIdentity {
        self.accepted
    }

    /// Resolves two ordered native picks into canonical branch-explicit Fillet intent.
    pub fn resolve_fillet_corner(
        &self,
        request: ComputedFilletCornerAuthoringRequest,
        radius: f64,
        policy: ComputedFeatureEvaluationPolicy,
        control: OperationControl,
    ) -> Result<OperationOutcome<ResolvedComputedFilletCorner>, ComputedFeatureAuthoringError> {
        policy.validate()?;
        if !radius.is_finite() || radius <= 0.0 {
            return Err(ComputedFeatureAuthoringError::InvalidRadius);
        }
        let mut controller = OperationController::new(control);
        if controller
            .charge(
                OperationWorkCounter::DocumentValidationItems,
                2,
                OperationCheckpoint::DocumentValidation,
            )
            .is_err()
        {
            return Ok(controller.outcome_unchecked());
        }
        match resolve_authoring_corner(&self.sketch, request, radius, policy, &mut controller)? {
            AuthoringCornerResolution::Stopped => Ok(controller.outcome_unchecked()),
            AuthoringCornerResolution::Completed(resolved) => {
                Ok(controller.outcome(ResolvedComputedFilletCorner {
                    sketch_input: self.sketch_input,
                    accepted: self.accepted,
                    corner: resolved.corner,
                    arc: resolved.arc,
                }))
            }
        }
    }

    /// Resolves one ordered corner batch under one aggregate bounded work
    /// envelope. A stopped or failed later corner never publishes a partial
    /// result vector.
    pub fn resolve_fillet_corners(
        &self,
        requests: &[ComputedFilletCornerAuthoringRequest],
        radius: f64,
        policy: ComputedFeatureEvaluationPolicy,
        control: OperationControl,
    ) -> Result<OperationOutcome<Vec<ResolvedComputedFilletCorner>>, ComputedFeatureAuthoringError>
    {
        policy.validate()?;
        if !radius.is_finite() || radius <= 0.0 {
            return Err(ComputedFeatureAuthoringError::InvalidRadius);
        }
        let mut controller = OperationController::new(control);
        if controller
            .charge(
                OperationWorkCounter::DocumentValidationItems,
                requests.len().saturating_mul(2),
                OperationCheckpoint::DocumentValidation,
            )
            .is_err()
        {
            return Ok(controller.outcome_unchecked());
        }
        let mut values = Vec::with_capacity(requests.len());
        for request in requests {
            let resolved = match resolve_authoring_corner(
                &self.sketch,
                *request,
                radius,
                policy,
                &mut controller,
            )? {
                AuthoringCornerResolution::Stopped => return Ok(controller.outcome_unchecked()),
                AuthoringCornerResolution::Completed(resolved) => resolved,
            };
            values.push(ResolvedComputedFilletCorner {
                sketch_input: self.sketch_input,
                accepted: self.accepted,
                corner: resolved.corner,
                arc: resolved.arc,
            });
        }
        Ok(controller.outcome(values))
    }
}

/// Worker-movable computed-feature evaluation.
#[derive(Debug)]
pub struct PreparedComputedFeatureEvaluation {
    snapshot: ComputedFeatureEvaluationSnapshot,
    evaluation: ComputedEvaluationRevision,
}

#[allow(
    clippy::missing_errors_doc,
    reason = "execution setup failures are enumerated by ComputedFeatureEvaluationError"
)]
impl PreparedComputedFeatureEvaluation {
    #[must_use]
    pub const fn input(&self) -> ComputedFeatureEvaluationInput {
        self.snapshot.input
    }

    /// Evaluates read-only feature intent under bounded cooperative control.
    pub fn execute(
        self,
        control: OperationControl,
    ) -> Result<OperationOutcome<ComputedFeatureSnapshot>, ComputedFeatureEvaluationError> {
        let policy = self.snapshot.input.policy;
        policy.validate()?;
        let feature_count = self.snapshot.features.features().len();
        let corner_count = self
            .snapshot
            .features
            .features()
            .iter()
            .map(|feature| {
                let ComputedFeatureDefinition::FilletSet(fillet) = &feature.definition;
                fillet.corners.len()
            })
            .sum::<usize>();
        if feature_count > policy.max_features {
            return Err(ComputedFeatureEvaluationError::PolicyLimitExceeded {
                resource: "features",
                actual: feature_count,
                limit: policy.max_features,
            });
        }
        if corner_count > policy.max_corners {
            return Err(ComputedFeatureEvaluationError::PolicyLimitExceeded {
                resource: "corners",
                actual: corner_count,
                limit: policy.max_corners,
            });
        }
        let mut controller = OperationController::new(control);
        if controller
            .charge(
                OperationWorkCounter::DocumentValidationItems,
                feature_count.saturating_add(corner_count),
                OperationCheckpoint::DocumentValidation,
            )
            .is_err()
        {
            return Ok(controller.outcome_unchecked());
        }
        let result = evaluate_snapshot(&self.snapshot, self.evaluation, &mut controller)?;
        if controller.is_stopped() {
            return Ok(controller.outcome_unchecked());
        }
        if controller
            .checkpoint(OperationCheckpoint::BeforeFinalValidation)
            .is_err()
        {
            return Ok(controller.outcome_unchecked());
        }
        Ok(controller.outcome(result))
    }
}

/// Typed inability to capture coherent current accepted geometry.
#[derive(Debug, Error)]
pub enum ComputedFeatureSnapshotError {
    #[error("computed features require the current independently accepted sketch state")]
    CurrentAcceptedStateRequired,
    #[error("computed-feature sidecar belongs to a different sketch document")]
    FeatureDocumentForDifferentSketch,
    #[error("accepted sketch state does not match the complete prepared input")]
    AcceptedInputMismatch,
    #[error("computed-feature sidecar is structurally invalid: {0}")]
    InvalidFeatureDocument(#[source] ComputedFeatureDocumentError),
}

/// Evaluation setup failure. Individual feature geometry failures are result data.
#[derive(Clone, Debug, Error, PartialEq)]
#[non_exhaustive]
pub enum ComputedFeatureEvaluationError {
    #[error("invalid computed-feature evaluation policy `{field}`: {message}")]
    InvalidPolicy {
        field: &'static str,
        message: &'static str,
    },
    #[error("computed-feature policy limit exceeded for {resource}: {actual} > {limit}")]
    PolicyLimitExceeded {
        resource: &'static str,
        actual: usize,
        limit: usize,
    },
    #[error("computed-feature evaluation identity space is exhausted")]
    EvaluationIdentityExhausted,
    #[error("computed-feature evaluator refused invalid generated {resource}")]
    InvalidGeneratedTopology { resource: &'static str },
}

/// One accepted native curve pick at the authoring boundary.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ComputedFilletCurvePick {
    pub source: NativeCurveSpanSource,
    pub parameter: f64,
    pub model_position: [f64; 2],
    pub retained_endpoint_hint: Option<DocumentFilletTrimEndpoint>,
}

/// Branch-correction state for resolving one Fillet corner.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ComputedFilletAuthoringOptions {
    pub flip_first_side: bool,
    pub flip_second_side: bool,
    pub alternate_arc: bool,
}

/// Complete two-pick authoring request owned by the feature layer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ComputedFilletCornerAuthoringRequest {
    pub first: ComputedFilletCurvePick,
    pub second: ComputedFilletCurvePick,
    pub options: ComputedFilletAuthoringOptions,
}

/// Resolved persistent intent plus independently validated preview geometry.
#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedComputedFilletCorner {
    pub sketch_input: PreparedSketchInput,
    pub accepted: SketchAcceptedStateIdentity,
    pub corner: NewComputedFilletCorner,
    pub arc: ComputedCircularArc,
}

/// Typed authoring/root-selection failure before persistent intent exists.
#[derive(Clone, Debug, Error, PartialEq)]
#[non_exhaustive]
pub enum ComputedFeatureAuthoringError {
    #[error(transparent)]
    Evaluation(#[from] ComputedFeatureEvaluationError),
    #[error("Fillet radius must be finite and positive")]
    InvalidRadius,
    #[error("a Fillet pick is non-finite")]
    NonFinitePick,
    #[error("a Fillet pick references a missing or invalid native span")]
    StalePick,
    #[error("a Fillet corner requires two distinct native spans")]
    DuplicateSource,
    #[error("same-curve Fillet parents must be adjacent spans of one open polyline")]
    UnsupportedSameCurvePair,
    #[error("two non-affine Fillet parents require pairwise continuation")]
    UnsupportedCurvedPair,
    #[error("a selected Fillet source has unsupported existing topology")]
    UnsupportedSourceTopology,
    #[error("the selected parents are parallel, singular or zero-speed")]
    SingularParents,
    #[error("no Fillet root exists in the selected local branches")]
    NoLocalRoot,
    #[error("multiple materially distinct Fillet roots are equally close")]
    AmbiguousLocalRoot,
    #[error("the selected side correction has no Fillet root")]
    SideCorrectionUnavailable,
    #[error("the pick does not identify which source endpoint to retain")]
    AmbiguousRetainedEndpoint,
    #[error("the curved parent branch cannot be certified")]
    UncertifiedCurvedBranch,
    #[error("the selected radius reaches a singular parent offset")]
    OffsetSingularity,
    #[error("resolved Fillet geometry failed independent validation")]
    InvalidResolvedGeometry,
}

/// Evaluation-local identity. It is invalid after any regeneration.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct ComputedEvaluationRevision(u64);

impl ComputedEvaluationRevision {
    #[must_use]
    pub const fn from_raw(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn raw(self) -> u64 {
        self.0
    }
}

/// Persistable next-revision high-water for generated output IDs.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ComputedEvaluationAllocatorHighWater {
    pub next_revision: ComputedEvaluationRevision,
}

/// Host-owned monotonic allocator for evaluation-local generated IDs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ComputedEvaluationAllocator {
    next_revision: ComputedEvaluationRevision,
}

impl Default for ComputedEvaluationAllocator {
    fn default() -> Self {
        Self {
            next_revision: ComputedEvaluationRevision(1),
        }
    }
}

impl ComputedEvaluationAllocator {
    #[must_use]
    pub const fn from_high_water(high_water: ComputedEvaluationAllocatorHighWater) -> Self {
        Self {
            next_revision: high_water.next_revision,
        }
    }

    #[must_use]
    pub const fn high_water(&self) -> ComputedEvaluationAllocatorHighWater {
        ComputedEvaluationAllocatorHighWater {
            next_revision: self.next_revision,
        }
    }

    pub fn retain_high_water(&mut self, retained: ComputedEvaluationAllocatorHighWater) {
        self.next_revision = self.next_revision.max(retained.next_revision);
    }

    fn allocate(&mut self) -> Result<ComputedEvaluationRevision, ComputedFeatureEvaluationError> {
        if self.next_revision.0 == 0 {
            return Err(ComputedFeatureEvaluationError::EvaluationIdentityExhausted);
        }
        let revision = self.next_revision;
        self.next_revision = ComputedEvaluationRevision(
            self.next_revision
                .0
                .checked_add(1)
                .ok_or(ComputedFeatureEvaluationError::EvaluationIdentityExhausted)?,
        );
        Ok(revision)
    }
}

/// Generated edge identity scoped to one evaluation revision.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ComputedEdgeId {
    pub evaluation: ComputedEvaluationRevision,
    pub ordinal: u32,
}

/// Stable source interval produced after composing all successful endpoint claims.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ComputedSourceInterval {
    pub start: f64,
    pub end: f64,
}

/// Stable source/corner attribution for one endpoint claim.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ComputedCornerRef {
    pub feature: ComputedFeatureId,
    pub corner: ComputedFeatureCornerId,
}

/// One independently validated Fillet contact.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ComputedFilletContact {
    pub source: NativeCurveSpanSource,
    pub parameter: f64,
    pub winding: i32,
    pub total_parameter: f64,
    pub position: [f64; 2],
}

/// One computed circular arc outside the constrained sketch graph.
#[derive(Clone, Debug, PartialEq)]
pub struct ComputedCircularArc {
    pub center: [f64; 2],
    pub radius: f64,
    pub start_angle: f64,
    pub end_angle: f64,
    pub sweep: DocumentArcSweep,
    pub contacts: [ComputedFilletContact; 2],
}

/// Geometry carried by one generated edge. The vector result permits future
/// topology-changing operations to publish any finite fragment count.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum ComputedEdgeGeometry {
    NativeSourceFragment {
        source: NativeCurveSpanSource,
        interval: ComputedSourceInterval,
    },
    CircularArc(ComputedCircularArc),
}

/// Stable provenance for revision-local generated geometry.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum ComputedEdgeProvenance {
    SourceFragment {
        source: NativeCurveSpanSource,
        interval: ComputedSourceInterval,
        start_claim: Option<ComputedCornerRef>,
        end_claim: Option<ComputedCornerRef>,
    },
    FilletArc {
        owner: ComputedCornerRef,
        sources: [NativeCurveSpanSource; 2],
    },
}

/// One revision-local generated edge and its stable provenance.
#[derive(Clone, Debug, PartialEq)]
pub struct ComputedEdge {
    pub id: ComputedEdgeId,
    pub geometry: ComputedEdgeGeometry,
    pub provenance: ComputedEdgeProvenance,
}

/// Endpoint involved in a composition conflict.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ComputedClaimEndpoint {
    Start,
    End,
    Both,
}

/// Typed feature-local failure. Geometry for the failed whole set is withheld.
#[derive(Clone, Debug, Error, PartialEq)]
#[non_exhaustive]
pub enum ComputedFeatureFailure {
    #[error("corner {corner:?} references missing native span {span_source:?}")]
    MissingSource {
        corner: ComputedFeatureCornerId,
        span_source: NativeCurveSpanSource,
    },
    #[error("corner {corner:?} references a source with M28 association-owned trim topology")]
    AssociationOwnedSource {
        corner: ComputedFeatureCornerId,
        span_source: NativeCurveSpanSource,
    },
    #[error("corner {corner:?} references a source with multiple native visible intervals")]
    MultiIntervalSource {
        corner: ComputedFeatureCornerId,
        span_source: NativeCurveSpanSource,
    },
    #[error("corner {corner:?} has incompatible persisted parent state")]
    InvalidParentState { corner: ComputedFeatureCornerId },
    #[error("corner {corner:?} uses two non-affine parents")]
    UnsupportedCurvedPair { corner: ComputedFeatureCornerId },
    #[error("corner {corner:?} has no root in its explicit branch")]
    NoLocalRoot { corner: ComputedFeatureCornerId },
    #[error("corner {corner:?} has an ambiguous local root")]
    AmbiguousLocalRoot { corner: ComputedFeatureCornerId },
    #[error("corner {corner:?} cannot certify its line/curve branch cell")]
    UncertifiedBranch { corner: ComputedFeatureCornerId },
    #[error("corner {corner:?} has parallel or near-singular parents")]
    SingularParents { corner: ComputedFeatureCornerId },
    #[error("corner {corner:?} reaches a singular parent offset")]
    OffsetSingularity { corner: ComputedFeatureCornerId },
    #[error("corner {corner:?} produced non-finite or invalid geometry")]
    InvalidGeometry { corner: ComputedFeatureCornerId },
    #[error("source endpoint claims conflict on {span_source:?}")]
    EndpointClaimConflict {
        span_source: NativeCurveSpanSource,
        endpoint: ComputedClaimEndpoint,
        participants: Vec<ComputedCornerRef>,
    },
    #[error("source endpoint claims consume or cross the remaining interval on {span_source:?}")]
    ConsumedSourceInterval {
        span_source: NativeCurveSpanSource,
        participants: Vec<ComputedCornerRef>,
    },
}

/// Per-feature evaluation state used directly by feature trees and diagnostics.
#[derive(Clone, Debug, PartialEq)]
pub enum ComputedFeatureEvaluationState {
    Current {
        corner_edges: Vec<(ComputedFeatureCornerId, ComputedEdgeId)>,
    },
    Failed {
        failure: ComputedFeatureFailure,
    },
    Suppressed,
}

/// Stable feature identity joined to one evaluation state.
#[derive(Clone, Debug, PartialEq)]
pub struct ComputedFeatureEvaluation {
    pub feature: ComputedFeatureId,
    pub state: ComputedFeatureEvaluationState,
}

/// Complete exact-stamped generated geometry snapshot.
#[derive(Clone, Debug)]
pub struct ComputedFeatureSnapshot {
    input: ComputedFeatureEvaluationInput,
    evaluation: ComputedEvaluationRevision,
    edges: Vec<ComputedEdge>,
    features: Vec<ComputedFeatureEvaluation>,
    replaced_sources: Vec<NativeCurveSpanSource>,
}

impl ComputedFeatureSnapshot {
    #[must_use]
    pub const fn input(&self) -> ComputedFeatureEvaluationInput {
        self.input
    }

    #[must_use]
    pub const fn evaluation_revision(&self) -> ComputedEvaluationRevision {
        self.evaluation
    }

    #[must_use]
    pub fn edges(&self) -> &[ComputedEdge] {
        &self.edges
    }

    #[must_use]
    pub fn feature_evaluations(&self) -> &[ComputedFeatureEvaluation] {
        &self.features
    }

    #[must_use]
    pub fn replaced_sources(&self) -> &[NativeCurveSpanSource] {
        &self.replaced_sources
    }

    #[must_use]
    pub fn edge(&self, id: ComputedEdgeId) -> Option<&ComputedEdge> {
        (id.evaluation == self.evaluation)
            .then(|| self.edges.get(id.ordinal as usize))
            .flatten()
    }

    /// Resolves the generated Fillet arc for one stable feature/corner pair.
    #[must_use]
    pub fn fillet_arc_edge(&self, owner: ComputedCornerRef) -> Option<&ComputedEdge> {
        self.edges.iter().find(|edge| {
            matches!(
                edge.provenance,
                ComputedEdgeProvenance::FilletArc { owner: current, .. } if current == owner
            )
        })
    }

    /// Resolves the composed visible replacement for one stable native source.
    pub fn source_fragment_edges(
        &self,
        source: NativeCurveSpanSource,
    ) -> impl Iterator<Item = &ComputedEdge> {
        self.edges.iter().filter(move |edge| {
            matches!(
                edge.provenance,
                ComputedEdgeProvenance::SourceFragment { source: current, .. }
                    if current == source
            )
        })
    }
}

#[derive(Clone, Copy, Debug)]
struct SourceTopology {
    domain: SourceDomain,
    base_interval: ComputedSourceInterval,
}

#[derive(Clone, Copy, Debug)]
enum SourceDomain {
    Bounded { lower: f64, upper: f64 },
    Periodic { period: f64 },
}

#[derive(Clone, Copy, Debug)]
struct RootParent {
    parent: ComputedFilletParent,
    topology: SourceTopology,
    seed_total: f64,
    bounds: (f64, f64),
}

#[derive(Clone, Copy, Debug)]
struct LocalFilletSolution {
    parameters: [f64; 2],
    sides: [DocumentCurveNormalSide; 2],
    center: [f64; 2],
    score: f64,
}

enum AuthoringCornerResolution {
    Completed(Box<ResolvedAuthoringCorner>),
    Stopped,
}

struct ResolvedAuthoringCorner {
    corner: NewComputedFilletCorner,
    arc: ComputedCircularArc,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RootSearchFailure {
    NoLocalRoot,
    SingularParents,
    OffsetSingularity,
}

impl RootSearchFailure {
    const fn merge(self, other: Self) -> Self {
        match (self, other) {
            (Self::OffsetSingularity, _) | (_, Self::OffsetSingularity) => Self::OffsetSingularity,
            (Self::SingularParents, _) | (_, Self::SingularParents) => Self::SingularParents,
            (Self::NoLocalRoot, Self::NoLocalRoot) => Self::NoLocalRoot,
        }
    }
}

enum RootSearchResult {
    Completed {
        solutions: Vec<LocalFilletSolution>,
        failure: RootSearchFailure,
    },
    Stopped,
}

enum RootAttempt {
    Solution(LocalFilletSolution),
    Failed(RootSearchFailure),
    Stopped,
}

#[derive(Clone, Debug)]
struct EvaluatedCorner {
    owner: ComputedCornerRef,
    arc: ComputedCircularArc,
    claims: [EndpointClaim; 2],
}

#[derive(Clone, Debug)]
struct EvaluatedFeatureCandidate {
    feature: ComputedFeatureId,
    corners: Vec<EvaluatedCorner>,
}

#[derive(Clone, Copy, Debug)]
struct EndpointClaim {
    owner: ComputedCornerRef,
    source: NativeCurveSpanSource,
    endpoint: DocumentFilletTrimEndpoint,
    parameter: f64,
    base_interval: ComputedSourceInterval,
}

#[derive(Clone, Debug)]
struct SourceComposition {
    source: NativeCurveSpanSource,
    base_interval: ComputedSourceInterval,
    start: Option<EndpointClaim>,
    end: Option<EndpointClaim>,
}

#[allow(
    clippy::too_many_lines,
    reason = "atomic feature evaluation and edge publication remain one auditable staged path"
)]
fn evaluate_snapshot(
    snapshot: &ComputedFeatureEvaluationSnapshot,
    evaluation: ComputedEvaluationRevision,
    controller: &mut OperationController,
) -> Result<ComputedFeatureSnapshot, ComputedFeatureEvaluationError> {
    let mut candidates = Vec::new();
    let mut evaluations = Vec::new();

    for feature in snapshot.features.features() {
        if feature.suppressed {
            evaluations.push(ComputedFeatureEvaluation {
                feature: feature.id,
                state: ComputedFeatureEvaluationState::Suppressed,
            });
            continue;
        }
        match evaluate_feature(&snapshot.sketch, feature, snapshot.input.policy, controller) {
            Ok(candidate) => candidates.push(candidate),
            Err(EvaluateFeatureError::Stopped) => {
                return Ok(empty_interrupted_snapshot(&snapshot.input, evaluation));
            }
            Err(EvaluateFeatureError::Failure(failure)) => {
                evaluations.push(ComputedFeatureEvaluation {
                    feature: feature.id,
                    state: ComputedFeatureEvaluationState::Failed { failure },
                });
            }
        }
    }

    let conflicts = composition_failures(&candidates);
    let failed_features = conflicts.keys().copied().collect::<BTreeSet<_>>();
    for (feature, failure) in conflicts {
        evaluations.push(ComputedFeatureEvaluation {
            feature,
            state: ComputedFeatureEvaluationState::Failed { failure },
        });
    }
    candidates.retain(|candidate| !failed_features.contains(&candidate.feature));

    let compositions = compose_sources(&candidates);
    let mut edges = Vec::new();
    let mut replaced_sources = Vec::new();
    for composition in compositions.values() {
        if controller
            .charge(
                OperationWorkCounter::ProfileFragments,
                1,
                OperationCheckpoint::DocumentLowering,
            )
            .is_err()
        {
            return Ok(empty_interrupted_snapshot(&snapshot.input, evaluation));
        }
        if edges.len() >= snapshot.input.policy.max_edges {
            return Err(ComputedFeatureEvaluationError::PolicyLimitExceeded {
                resource: "edges",
                actual: edges.len().saturating_add(1),
                limit: snapshot.input.policy.max_edges,
            });
        }
        let interval = ComputedSourceInterval {
            start: composition
                .start
                .map_or(composition.base_interval.start, |claim| claim.parameter),
            end: composition
                .end
                .map_or(composition.base_interval.end, |claim| claim.parameter),
        };
        if !strict_finite_interval(interval) {
            return Err(ComputedFeatureEvaluationError::InvalidGeneratedTopology {
                resource: "source interval",
            });
        }
        let id = edge_id(evaluation, edges.len())?;
        edges.push(ComputedEdge {
            id,
            geometry: ComputedEdgeGeometry::NativeSourceFragment {
                source: composition.source,
                interval,
            },
            provenance: ComputedEdgeProvenance::SourceFragment {
                source: composition.source,
                interval,
                start_claim: composition.start.map(|claim| claim.owner),
                end_claim: composition.end.map(|claim| claim.owner),
            },
        });
        replaced_sources.push(composition.source);
    }

    for candidate in &candidates {
        let mut corner_edges = Vec::with_capacity(candidate.corners.len());
        for corner in &candidate.corners {
            if controller
                .charge(
                    OperationWorkCounter::ProfileFragments,
                    1,
                    OperationCheckpoint::DocumentLowering,
                )
                .is_err()
            {
                return Ok(empty_interrupted_snapshot(&snapshot.input, evaluation));
            }
            if edges.len() >= snapshot.input.policy.max_edges {
                return Err(ComputedFeatureEvaluationError::PolicyLimitExceeded {
                    resource: "edges",
                    actual: edges.len().saturating_add(1),
                    limit: snapshot.input.policy.max_edges,
                });
            }
            let id = edge_id(evaluation, edges.len())?;
            edges.push(ComputedEdge {
                id,
                geometry: ComputedEdgeGeometry::CircularArc(corner.arc.clone()),
                provenance: ComputedEdgeProvenance::FilletArc {
                    owner: corner.owner,
                    sources: corner.claims.map(|claim| claim.source),
                },
            });
            corner_edges.push((corner.owner.corner, id));
        }
        evaluations.push(ComputedFeatureEvaluation {
            feature: candidate.feature,
            state: ComputedFeatureEvaluationState::Current { corner_edges },
        });
    }
    evaluations.sort_by_key(|evaluation| evaluation.feature);
    replaced_sources.sort();
    replaced_sources.dedup();
    Ok(ComputedFeatureSnapshot {
        input: snapshot.input,
        evaluation,
        edges,
        features: evaluations,
        replaced_sources,
    })
}

fn empty_interrupted_snapshot(
    input: &ComputedFeatureEvaluationInput,
    evaluation: ComputedEvaluationRevision,
) -> ComputedFeatureSnapshot {
    ComputedFeatureSnapshot {
        input: *input,
        evaluation,
        edges: Vec::new(),
        features: Vec::new(),
        replaced_sources: Vec::new(),
    }
}

enum EvaluateFeatureError {
    Stopped,
    Failure(ComputedFeatureFailure),
}

fn evaluate_feature(
    sketch: &SketchDocument,
    feature: &ComputedFeature,
    policy: ComputedFeatureEvaluationPolicy,
    controller: &mut OperationController,
) -> Result<EvaluatedFeatureCandidate, EvaluateFeatureError> {
    let ComputedFeatureDefinition::FilletSet(fillet) = &feature.definition;
    let mut corners = Vec::with_capacity(fillet.corners.len());
    for corner in &fillet.corners {
        if controller
            .checkpoint(OperationCheckpoint::DocumentDependency)
            .is_err()
        {
            return Err(EvaluateFeatureError::Stopped);
        }
        let owner = ComputedCornerRef {
            feature: feature.id,
            corner: corner.id,
        };
        let evaluated =
            evaluate_persistent_corner(sketch, owner, *corner, fillet.radius, policy, controller)?;
        corners.push(evaluated);
    }
    Ok(EvaluatedFeatureCandidate {
        feature: feature.id,
        corners,
    })
}

fn evaluate_persistent_corner(
    sketch: &SketchDocument,
    owner: ComputedCornerRef,
    corner: ComputedFilletCorner,
    radius: f64,
    policy: ComputedFeatureEvaluationPolicy,
    controller: &mut OperationController,
) -> Result<EvaluatedCorner, EvaluateFeatureError> {
    let parents = [corner.first, corner.second];
    let root_parents = prepare_root_parents(sketch, parents, Some(corner.id))
        .map_err(EvaluateFeatureError::Failure)?;
    let affine = parents.map(|parent| is_affine_line_span(sketch, parent.source.span));
    if affine == [false, false] {
        return Err(EvaluateFeatureError::Failure(
            ComputedFeatureFailure::UnsupportedCurvedPair { corner: corner.id },
        ));
    }
    let root_parents = certify_persistent_branch(sketch, root_parents, affine, corner.id)
        .map_err(EvaluateFeatureError::Failure)?;
    let (solutions, root_failure) = match local_fillet_roots(
        sketch,
        &root_parents,
        [corner.first.normal_side, corner.second.normal_side],
        radius,
        policy,
        controller,
    ) {
        RootSearchResult::Completed { solutions, failure } => (solutions, failure),
        RootSearchResult::Stopped => return Err(EvaluateFeatureError::Stopped),
    };
    let solution = select_solution(sketch, &solutions).map_err(|kind| {
        EvaluateFeatureError::Failure(match kind {
            RootSelectionFailure::None => match root_failure {
                RootSearchFailure::NoLocalRoot => {
                    ComputedFeatureFailure::NoLocalRoot { corner: corner.id }
                }
                RootSearchFailure::SingularParents => {
                    ComputedFeatureFailure::SingularParents { corner: corner.id }
                }
                RootSearchFailure::OffsetSingularity => {
                    ComputedFeatureFailure::OffsetSingularity { corner: corner.id }
                }
            },
            RootSelectionFailure::Ambiguous => {
                ComputedFeatureFailure::AmbiguousLocalRoot { corner: corner.id }
            }
        })
    })?;
    let arc = build_and_validate_arc(
        sketch,
        parents,
        solution,
        radius,
        corner.endpoint_order,
        corner.sweep,
    )
    .map_err(|failure| {
        EvaluateFeatureError::Failure(match failure {
            ArcValidationFailure::OffsetSingularity => {
                ComputedFeatureFailure::OffsetSingularity { corner: corner.id }
            }
            ArcValidationFailure::SingularParents => {
                ComputedFeatureFailure::SingularParents { corner: corner.id }
            }
            ArcValidationFailure::Invalid => {
                ComputedFeatureFailure::InvalidGeometry { corner: corner.id }
            }
        })
    })?;
    let claims = [
        EndpointClaim {
            owner,
            source: corner.first.source,
            endpoint: corner.first.retained_endpoint,
            parameter: solution.parameters[0],
            base_interval: root_parents[0].topology.base_interval,
        },
        EndpointClaim {
            owner,
            source: corner.second.source,
            endpoint: corner.second.retained_endpoint,
            parameter: solution.parameters[1],
            base_interval: root_parents[1].topology.base_interval,
        },
    ];
    Ok(EvaluatedCorner { owner, arc, claims })
}

fn composition_failures(
    candidates: &[EvaluatedFeatureCandidate],
) -> BTreeMap<ComputedFeatureId, ComputedFeatureFailure> {
    let mut claims = BTreeMap::<NativeCurveSpanSource, Vec<EndpointClaim>>::new();
    for corner in candidates
        .iter()
        .flat_map(|candidate| candidate.corners.iter())
    {
        for claim in corner.claims {
            claims.entry(claim.source).or_default().push(claim);
        }
    }
    let mut failures = BTreeMap::new();
    for (source, mut source_claims) in claims {
        source_claims.sort_by_key(|claim| claim.owner);
        source_claims.retain(|claim| {
            let valid = strict_finite_interval(claim.base_interval)
                && parameter_strictly_inside(claim.parameter, claim.base_interval);
            if !valid {
                failures.entry(claim.owner.feature).or_insert(
                    ComputedFeatureFailure::InvalidParentState {
                        corner: claim.owner.corner,
                    },
                );
            }
            valid
        });
        if source_claims.is_empty() {
            continue;
        }
        let starts = source_claims
            .iter()
            .copied()
            .filter(|claim| claim.endpoint == DocumentFilletTrimEndpoint::Start)
            .collect::<Vec<_>>();
        let ends = source_claims
            .iter()
            .copied()
            .filter(|claim| claim.endpoint == DocumentFilletTrimEndpoint::End)
            .collect::<Vec<_>>();
        if starts.len() > 1 {
            insert_conflict_failures(
                &mut failures,
                &source_claims,
                &ComputedFeatureFailure::EndpointClaimConflict {
                    span_source: source,
                    endpoint: ComputedClaimEndpoint::Start,
                    participants: starts.iter().map(|claim| claim.owner).collect(),
                },
            );
        }
        if ends.len() > 1 {
            insert_conflict_failures(
                &mut failures,
                &source_claims,
                &ComputedFeatureFailure::EndpointClaimConflict {
                    span_source: source,
                    endpoint: ComputedClaimEndpoint::End,
                    participants: ends.iter().map(|claim| claim.owner).collect(),
                },
            );
        }
        if let ([start], [end]) = (starts.as_slice(), ends.as_slice()) {
            let tolerance = parameter_tolerance(ComputedSourceInterval {
                start: start.base_interval.start.min(end.base_interval.start),
                end: start.base_interval.end.max(end.base_interval.end),
            });
            if start.parameter + tolerance >= end.parameter {
                let participants = vec![start.owner, end.owner];
                insert_conflict_failures(
                    &mut failures,
                    &source_claims,
                    &ComputedFeatureFailure::ConsumedSourceInterval {
                        span_source: source,
                        participants,
                    },
                );
            }
        }
        let base_mismatch = source_claims.iter().skip(1).any(|claim| {
            claim.base_interval.start.to_bits() != source_claims[0].base_interval.start.to_bits()
                || claim.base_interval.end.to_bits() != source_claims[0].base_interval.end.to_bits()
        });
        if base_mismatch {
            insert_conflict_failures(
                &mut failures,
                &source_claims,
                &ComputedFeatureFailure::EndpointClaimConflict {
                    span_source: source,
                    endpoint: ComputedClaimEndpoint::Both,
                    participants: source_claims.iter().map(|claim| claim.owner).collect(),
                },
            );
        }
    }
    failures
}

fn insert_conflict_failures(
    failures: &mut BTreeMap<ComputedFeatureId, ComputedFeatureFailure>,
    source_claims: &[EndpointClaim],
    failure: &ComputedFeatureFailure,
) {
    let (ComputedFeatureFailure::EndpointClaimConflict { participants, .. }
    | ComputedFeatureFailure::ConsumedSourceInterval { participants, .. }) = failure
    else {
        return;
    };
    for feature in participants.iter().map(|owner| owner.feature) {
        let mut attributed = (*failure).clone();
        if let ComputedFeatureFailure::EndpointClaimConflict {
            participants: owners,
            ..
        }
        | ComputedFeatureFailure::ConsumedSourceInterval {
            participants: owners,
            ..
        } = &mut attributed
        {
            owners.sort();
            owners.dedup();
        }
        failures.entry(feature).or_insert(attributed);
    }
    // A whole set fails atomically. If another claim from that same set is in
    // this source group, retain deterministic attribution to the same conflict.
    let failed_set_ids = participants
        .iter()
        .map(|owner| owner.feature)
        .collect::<BTreeSet<_>>();
    for feature in source_claims
        .iter()
        .map(|claim| claim.owner.feature)
        .filter(|feature| failed_set_ids.contains(feature))
    {
        failures
            .entry(feature)
            .or_insert_with(|| (*failure).clone());
    }
}

fn compose_sources(
    candidates: &[EvaluatedFeatureCandidate],
) -> BTreeMap<NativeCurveSpanSource, SourceComposition> {
    let mut compositions = BTreeMap::new();
    for claim in candidates
        .iter()
        .flat_map(|candidate| candidate.corners.iter())
        .flat_map(|corner| corner.claims)
    {
        let composition = compositions
            .entry(claim.source)
            .or_insert(SourceComposition {
                source: claim.source,
                base_interval: claim.base_interval,
                start: None,
                end: None,
            });
        match claim.endpoint {
            DocumentFilletTrimEndpoint::Start => composition.start = Some(claim),
            DocumentFilletTrimEndpoint::End => composition.end = Some(claim),
        }
    }
    compositions
}

fn edge_id(
    evaluation: ComputedEvaluationRevision,
    index: usize,
) -> Result<ComputedEdgeId, ComputedFeatureEvaluationError> {
    Ok(ComputedEdgeId {
        evaluation,
        ordinal: u32::try_from(index).map_err(|_| {
            ComputedFeatureEvaluationError::PolicyLimitExceeded {
                resource: "edge ordinals",
                actual: index,
                limit: u32::MAX as usize,
            }
        })?,
    })
}

#[allow(
    clippy::too_many_lines,
    reason = "branch search, correction and persistent intent synthesis form one auditable operation"
)]
fn resolve_authoring_corner(
    sketch: &SketchDocument,
    request: ComputedFilletCornerAuthoringRequest,
    radius: f64,
    policy: ComputedFeatureEvaluationPolicy,
    controller: &mut OperationController,
) -> Result<AuthoringCornerResolution, ComputedFeatureAuthoringError> {
    validate_authoring_pick(sketch, request.first)?;
    validate_authoring_pick(sketch, request.second)?;
    if request.first.source == request.second.source {
        return Err(ComputedFeatureAuthoringError::DuplicateSource);
    }
    if request.first.source.span.curve == request.second.source.span.curve
        && !same_open_polyline_adjacent_spans(
            sketch,
            request.first.source.span,
            request.second.source.span,
        )
    {
        return Err(ComputedFeatureAuthoringError::UnsupportedSameCurvePair);
    }
    let affine = [
        is_affine_line_span(sketch, request.first.source.span),
        is_affine_line_span(sketch, request.second.source.span),
    ];
    if affine == [false, false] {
        return Err(ComputedFeatureAuthoringError::UnsupportedCurvedPair);
    }
    let topologies = [
        source_topology_for_authoring(sketch, request.first.source)?,
        source_topology_for_authoring(sketch, request.second.source)?,
    ];
    let picks = [request.first, request.second];
    let mut solutions = Vec::new();
    let mut root_failure = RootSearchFailure::NoLocalRoot;
    for first_side in [
        DocumentCurveNormalSide::Left,
        DocumentCurveNormalSide::Right,
    ] {
        for second_side in [
            DocumentCurveNormalSide::Left,
            DocumentCurveNormalSide::Right,
        ] {
            let parents = authoring_root_parents(
                sketch,
                picks,
                topologies,
                [first_side, second_side],
                affine,
            )?;
            match local_fillet_roots(
                sketch,
                &parents,
                [first_side, second_side],
                radius,
                policy,
                controller,
            ) {
                RootSearchResult::Completed {
                    solutions: mut roots,
                    failure,
                } => {
                    root_failure = root_failure.merge(failure);
                    solutions.append(&mut roots);
                }
                RootSearchResult::Stopped => return Ok(AuthoringCornerResolution::Stopped),
            }
        }
    }
    solutions.sort_by(|left, right| {
        left.score
            .total_cmp(&right.score)
            .then_with(|| side_rank(left.sides).cmp(&side_rank(right.sides)))
    });
    let mut solution = select_solution(sketch, &solutions).map_err(|failure| match failure {
        RootSelectionFailure::None => match root_failure {
            RootSearchFailure::NoLocalRoot => ComputedFeatureAuthoringError::NoLocalRoot,
            RootSearchFailure::SingularParents => ComputedFeatureAuthoringError::SingularParents,
            RootSearchFailure::OffsetSingularity => {
                ComputedFeatureAuthoringError::OffsetSingularity
            }
        },
        RootSelectionFailure::Ambiguous => ComputedFeatureAuthoringError::AmbiguousLocalRoot,
    })?;
    if request.options.flip_first_side || request.options.flip_second_side {
        let desired = [
            if request.options.flip_first_side {
                flip_side(solution.sides[0])
            } else {
                solution.sides[0]
            },
            if request.options.flip_second_side {
                flip_side(solution.sides[1])
            } else {
                solution.sides[1]
            },
        ];
        let corrected = solutions
            .iter()
            .copied()
            .filter(|candidate| candidate.sides == desired)
            .collect::<Vec<_>>();
        solution = select_solution(sketch, &corrected)
            .map_err(|_| ComputedFeatureAuthoringError::SideCorrectionUnavailable)?;
    }
    let persistent_parents =
        build_persistent_authoring_parents(sketch, picks, topologies, affine, solution)?;
    let contact_angles = contact_angles(sketch, persistent_parents, solution, radius)
        .map_err(map_arc_authoring_failure)?;
    let ccw = (contact_angles[1] - contact_angles[0]).rem_euclid(std::f64::consts::TAU);
    if !ccw.is_finite() || ccw <= 1.0e-10 || ccw >= std::f64::consts::TAU - 1.0e-10 {
        return Err(ComputedFeatureAuthoringError::InvalidResolvedGeometry);
    }
    let mut endpoint_order = if ccw <= std::f64::consts::PI {
        DocumentFilletEndpointOrder::FirstThenSecond
    } else {
        DocumentFilletEndpointOrder::SecondThenFirst
    };
    if request.options.alternate_arc {
        endpoint_order = flip_endpoint_order(endpoint_order);
    }
    let corner = NewComputedFilletCorner {
        first: persistent_parents[0],
        second: persistent_parents[1],
        endpoint_order,
        sweep: DocumentArcSweep::CounterClockwise,
    };
    let arc = build_and_validate_arc(
        sketch,
        persistent_parents,
        solution,
        radius,
        endpoint_order,
        DocumentArcSweep::CounterClockwise,
    )
    .map_err(map_arc_authoring_failure)?;
    Ok(AuthoringCornerResolution::Completed(Box::new(
        ResolvedAuthoringCorner {
            corner: corner.canonicalized(),
            arc,
        },
    )))
}

fn validate_authoring_pick(
    sketch: &SketchDocument,
    pick: ComputedFilletCurvePick,
) -> Result<(), ComputedFeatureAuthoringError> {
    if !pick.parameter.is_finite() || !pick.model_position.into_iter().all(f64::is_finite) {
        return Err(ComputedFeatureAuthoringError::NonFinitePick);
    }
    let jet = sketch
        .evaluate_curve_jet(pick.source.span, pick.parameter)
        .map_err(|_| ComputedFeatureAuthoringError::StalePick)?;
    let tolerance = (sketch.model_scale() * GEOMETRY_TOLERANCE_FACTOR).max(1.0e-10);
    if (jet.position.x - pick.model_position[0]).hypot(jet.position.y - pick.model_position[1])
        > tolerance
    {
        return Err(ComputedFeatureAuthoringError::StalePick);
    }
    Ok(())
}

fn source_topology_for_authoring(
    sketch: &SketchDocument,
    source: NativeCurveSpanSource,
) -> Result<SourceTopology, ComputedFeatureAuthoringError> {
    source_topology(sketch, source).map_err(|failure| match failure {
        SourceTopologyFailure::Missing | SourceTopologyFailure::InvalidDomain => {
            ComputedFeatureAuthoringError::StalePick
        }
        SourceTopologyFailure::AssociationOwned | SourceTopologyFailure::MultiInterval => {
            ComputedFeatureAuthoringError::UnsupportedSourceTopology
        }
    })
}

fn authoring_root_parents(
    sketch: &SketchDocument,
    picks: [ComputedFilletCurvePick; 2],
    topologies: [SourceTopology; 2],
    sides: [DocumentCurveNormalSide; 2],
    affine: [bool; 2],
) -> Result<[RootParent; 2], ComputedFeatureAuthoringError> {
    let mut parents = [0, 1].map(|index| {
        let topology = topologies[index];
        let seed_total = total_parameter(topology.domain, picks[index].parameter, 0)
            .unwrap_or(picks[index].parameter);
        let neighborhood = if affine[index] {
            ContactNeighborhood::Interior
        } else {
            ContactNeighborhood::Local {
                lower: seed_total,
                upper: seed_total,
            }
        };
        RootParent {
            parent: ComputedFilletParent {
                source: picks[index].source,
                picked_parameter: picks[index].parameter,
                winding: 0,
                neighborhood,
                normal_side: sides[index],
                retained_endpoint: picks[index]
                    .retained_endpoint_hint
                    .unwrap_or(DocumentFilletTrimEndpoint::End),
                periodic_anchor: None,
            },
            topology,
            seed_total,
            bounds: (0.0, 0.0),
        }
    });
    for index in 0..2 {
        parents[index].bounds = authoring_parameter_bounds(
            sketch,
            picks[index],
            topologies[index],
            affine[index],
            picks[1 - index].source.span,
        )?;
    }
    Ok(parents)
}

fn authoring_parameter_bounds(
    sketch: &SketchDocument,
    pick: ComputedFilletCurvePick,
    topology: SourceTopology,
    affine: bool,
    other: CurveSpan,
) -> Result<(f64, f64), ComputedFeatureAuthoringError> {
    let total = total_parameter(topology.domain, pick.parameter, 0)
        .ok_or(ComputedFeatureAuthoringError::StalePick)?;
    if affine {
        return interior_bounds(topology.base_interval)
            .ok_or(ComputedFeatureAuthoringError::SingularParents);
    }
    let (support_lower, support_upper) = match topology.domain {
        SourceDomain::Bounded { lower, upper } => (lower, upper),
        SourceDomain::Periodic { period } => (total - 0.5 * period, total + 0.5 * period),
    };
    let neighborhood = sketch
        .certify_line_curve_fillet_branch_cell(
            other,
            pick.source.span,
            total,
            support_lower,
            support_upper,
        )
        .map_err(|_| ComputedFeatureAuthoringError::UncertifiedCurvedBranch)?;
    let ContactNeighborhood::Local { lower, upper } = neighborhood else {
        return Err(ComputedFeatureAuthoringError::UncertifiedCurvedBranch);
    };
    interior_bounds(ComputedSourceInterval {
        start: lower,
        end: upper,
    })
    .ok_or(ComputedFeatureAuthoringError::UncertifiedCurvedBranch)
}

fn build_persistent_authoring_parents(
    sketch: &SketchDocument,
    picks: [ComputedFilletCurvePick; 2],
    topologies: [SourceTopology; 2],
    affine: [bool; 2],
    solution: LocalFilletSolution,
) -> Result<[ComputedFilletParent; 2], ComputedFeatureAuthoringError> {
    let mut parents = Vec::with_capacity(2);
    for index in 0..2 {
        let topology = topologies[index];
        let total = solution.parameters[index];
        let (parameter, winding) = normalize_parameter(topology.domain, total)
            .ok_or(ComputedFeatureAuthoringError::InvalidResolvedGeometry)?;
        let retained_endpoint = match picks[index].retained_endpoint_hint {
            Some(endpoint) => endpoint,
            None => infer_retained_endpoint(sketch, picks[index], total, topology)?,
        };
        let neighborhood = if affine[index] {
            ContactNeighborhood::Interior
        } else {
            let (support_lower, support_upper) = match topology.domain {
                SourceDomain::Bounded { lower, upper } => (lower, upper),
                SourceDomain::Periodic { period } => (total - 0.5 * period, total + 0.5 * period),
            };
            sketch
                .certify_line_curve_fillet_branch_cell(
                    picks[1 - index].source.span,
                    picks[index].source.span,
                    total,
                    support_lower,
                    support_upper,
                )
                .map_err(|_| ComputedFeatureAuthoringError::UncertifiedCurvedBranch)?
        };
        let periodic_anchor = match topology.domain {
            SourceDomain::Bounded { .. } => None,
            SourceDomain::Periodic { period } => {
                let anchor_total = match retained_endpoint {
                    DocumentFilletTrimEndpoint::End => total - 0.5 * period,
                    DocumentFilletTrimEndpoint::Start => total + 0.5 * period,
                };
                let (parameter, winding) = normalize_parameter(topology.domain, anchor_total)
                    .ok_or(ComputedFeatureAuthoringError::InvalidResolvedGeometry)?;
                Some(DocumentTrimParameter { parameter, winding })
            }
        };
        parents.push(ComputedFilletParent {
            source: picks[index].source,
            picked_parameter: parameter,
            winding,
            neighborhood,
            normal_side: solution.sides[index],
            retained_endpoint,
            periodic_anchor,
        });
    }
    parents
        .try_into()
        .map_err(|_| ComputedFeatureAuthoringError::InvalidResolvedGeometry)
}

fn infer_retained_endpoint(
    sketch: &SketchDocument,
    pick: ComputedFilletCurvePick,
    contact_parameter: f64,
    topology: SourceTopology,
) -> Result<DocumentFilletTrimEndpoint, ComputedFeatureAuthoringError> {
    let contact = sketch
        .evaluate_curve_jet(pick.source.span, contact_parameter)
        .map_err(|_| ComputedFeatureAuthoringError::InvalidResolvedGeometry)?;
    let pick_total = total_parameter(topology.domain, pick.parameter, 0)
        .ok_or(ComputedFeatureAuthoringError::InvalidResolvedGeometry)?;
    let parameter_scale = match topology.domain {
        SourceDomain::Bounded { lower, upper } => upper - lower,
        SourceDomain::Periodic { period } => period,
    };
    let parameter_tolerance = (parameter_scale.abs() * 1.0e-9).max(1.0e-12);
    let position_tolerance = (sketch.model_scale() * GEOMETRY_TOLERANCE_FACTOR).max(1.0e-10);
    let distance = (contact.position.x - pick.model_position[0])
        .hypot(contact.position.y - pick.model_position[1]);
    if (pick_total - contact_parameter).abs() <= parameter_tolerance
        || distance <= position_tolerance
    {
        return Err(ComputedFeatureAuthoringError::AmbiguousRetainedEndpoint);
    }
    Ok(if pick_total < contact_parameter {
        DocumentFilletTrimEndpoint::End
    } else {
        DocumentFilletTrimEndpoint::Start
    })
}

fn prepare_root_parents(
    sketch: &SketchDocument,
    parents: [ComputedFilletParent; 2],
    corner: Option<ComputedFeatureCornerId>,
) -> Result<[RootParent; 2], ComputedFeatureFailure> {
    let corner = corner.unwrap_or(ComputedFeatureCornerId::from_raw(0));
    let mut result = Vec::with_capacity(2);
    for parent in parents {
        let mut topology =
            source_topology(sketch, parent.source).map_err(|failure| match failure {
                SourceTopologyFailure::Missing => ComputedFeatureFailure::MissingSource {
                    corner,
                    span_source: parent.source,
                },
                SourceTopologyFailure::AssociationOwned => {
                    ComputedFeatureFailure::AssociationOwnedSource {
                        corner,
                        span_source: parent.source,
                    }
                }
                SourceTopologyFailure::MultiInterval => {
                    ComputedFeatureFailure::MultiIntervalSource {
                        corner,
                        span_source: parent.source,
                    }
                }
                SourceTopologyFailure::InvalidDomain => {
                    ComputedFeatureFailure::InvalidParentState { corner }
                }
            })?;
        let seed_total = total_parameter(topology.domain, parent.picked_parameter, parent.winding)
            .ok_or(ComputedFeatureFailure::InvalidParentState { corner })?;
        match topology.domain {
            SourceDomain::Bounded { .. } => {
                if parent.winding != 0
                    || parent.periodic_anchor.is_some()
                    || !parameter_strictly_inside(seed_total, topology.base_interval)
                {
                    return Err(ComputedFeatureFailure::InvalidParentState { corner });
                }
            }
            SourceDomain::Periodic { period } => {
                let visible_interval = topology.base_interval;
                let anchor = parent
                    .periodic_anchor
                    .ok_or(ComputedFeatureFailure::InvalidParentState { corner })?;
                let anchor_total =
                    total_parameter(topology.domain, anchor.parameter, anchor.winding)
                        .ok_or(ComputedFeatureFailure::InvalidParentState { corner })?;
                let retained_interval = match parent.retained_endpoint {
                    DocumentFilletTrimEndpoint::End => ComputedSourceInterval {
                        start: anchor_total,
                        end: anchor_total + period,
                    },
                    DocumentFilletTrimEndpoint::Start => ComputedSourceInterval {
                        start: anchor_total - period,
                        end: anchor_total,
                    },
                };
                topology.base_interval = intersect_periodic_visible_interval(
                    visible_interval,
                    retained_interval,
                    seed_total,
                    period,
                )
                .ok_or(ComputedFeatureFailure::InvalidParentState { corner })?;
                if !strict_finite_interval(topology.base_interval)
                    || !parameter_strictly_inside(seed_total, topology.base_interval)
                {
                    return Err(ComputedFeatureFailure::InvalidParentState { corner });
                }
            }
        }
        let bounds = persistent_parent_bounds(topology, parent, seed_total)
            .ok_or(ComputedFeatureFailure::InvalidParentState { corner })?;
        result.push(RootParent {
            parent,
            topology,
            seed_total,
            bounds,
        });
    }
    result
        .try_into()
        .map_err(|_| ComputedFeatureFailure::InvalidParentState { corner })
}

fn certify_persistent_branch(
    sketch: &SketchDocument,
    mut parents: [RootParent; 2],
    affine: [bool; 2],
    corner: ComputedFeatureCornerId,
) -> Result<[RootParent; 2], ComputedFeatureFailure> {
    for index in 0..2 {
        if affine[index] {
            if parents[index].parent.neighborhood != ContactNeighborhood::Interior
                || parents[index].parent.winding != 0
                || parents[index].parent.periodic_anchor.is_some()
            {
                return Err(ComputedFeatureFailure::InvalidParentState { corner });
            }
            continue;
        }
        let ContactNeighborhood::Local {
            lower: stored_lower,
            upper: stored_upper,
        } = parents[index].parent.neighborhood
        else {
            return Err(ComputedFeatureFailure::InvalidParentState { corner });
        };
        let (support_lower, support_upper) = match parents[index].topology.domain {
            SourceDomain::Bounded { lower, upper } => (lower, upper),
            SourceDomain::Periodic { period } => (
                parents[index].seed_total - 0.5 * period,
                parents[index].seed_total + 0.5 * period,
            ),
        };
        let certified = sketch
            .certify_line_curve_fillet_branch_cell(
                parents[1 - index].parent.source.span,
                parents[index].parent.source.span,
                parents[index].seed_total,
                support_lower,
                support_upper,
            )
            .map_err(|_| ComputedFeatureFailure::UncertifiedBranch { corner })?;
        let ContactNeighborhood::Local {
            lower: current_lower,
            upper: current_upper,
        } = certified
        else {
            return Err(ComputedFeatureFailure::UncertifiedBranch { corner });
        };
        let lower = stored_lower.max(current_lower);
        let upper = stored_upper.min(current_upper);
        parents[index].bounds = interior_bounds(ComputedSourceInterval {
            start: lower,
            end: upper,
        })
        .ok_or(ComputedFeatureFailure::UncertifiedBranch { corner })?;
    }
    Ok(parents)
}

#[derive(Clone, Copy, Debug)]
enum SourceTopologyFailure {
    Missing,
    AssociationOwned,
    MultiInterval,
    InvalidDomain,
}

fn source_topology(
    sketch: &SketchDocument,
    source: NativeCurveSpanSource,
) -> Result<SourceTopology, SourceTopologyFailure> {
    let domains = sketch
        .curve_contact_domains(source.span)
        .map_err(|_| SourceTopologyFailure::Missing)?;
    let views = sketch.trim_views_for_span(source.span).collect::<Vec<_>>();
    if views.len() > 1 {
        return Err(SourceTopologyFailure::MultiInterval);
    }
    if views.iter().any(|view| {
        matches!(view.start, DocumentTrimBoundary::FilletContact { .. })
            || matches!(view.end, DocumentTrimBoundary::FilletContact { .. })
    }) {
        return Err(SourceTopologyFailure::AssociationOwned);
    }
    let intervals = sketch
        .visible_intervals(source.span)
        .map_err(|_| SourceTopologyFailure::Missing)?;
    let [interval] = intervals.as_slice() else {
        return Err(SourceTopologyFailure::MultiInterval);
    };
    let domain = domains
        .iter()
        .find_map(|domain| match *domain {
            ContactDomain::Periodic { period } => Some(SourceDomain::Periodic { period }),
            ContactDomain::Bounded { lower, upper } => Some(SourceDomain::Bounded { lower, upper }),
            ContactDomain::SupportingLine => None,
        })
        .ok_or(SourceTopologyFailure::Missing)?;
    let base_interval = ComputedSourceInterval {
        start: interval.start,
        end: interval.end,
    };
    if !source_domain_and_interval_are_valid(domain, base_interval) {
        return Err(SourceTopologyFailure::InvalidDomain);
    }
    Ok(SourceTopology {
        domain,
        base_interval,
    })
}

fn persistent_parent_bounds(
    topology: SourceTopology,
    parent: ComputedFilletParent,
    seed_total: f64,
) -> Option<(f64, f64)> {
    match (topology.domain, parent.neighborhood) {
        (SourceDomain::Bounded { .. }, ContactNeighborhood::Interior) => {
            interior_bounds(topology.base_interval)
        }
        (
            SourceDomain::Bounded { lower, upper },
            ContactNeighborhood::Local {
                lower: local_lower,
                upper: local_upper,
            },
        ) if lower <= local_lower
            && local_lower < seed_total
            && seed_total < local_upper
            && local_upper <= upper =>
        {
            interior_bounds(ComputedSourceInterval {
                start: local_lower.max(topology.base_interval.start),
                end: local_upper.min(topology.base_interval.end),
            })
        }
        (SourceDomain::Periodic { period }, ContactNeighborhood::Local { lower, upper })
            if lower < seed_total
                && seed_total < upper
                && upper - lower <= period * (1.0 + 1.0e-10)
                && topology.base_interval.start <= lower
                && upper <= topology.base_interval.end
                && parent.periodic_anchor.is_some() =>
        {
            interior_bounds(ComputedSourceInterval {
                start: lower,
                end: upper,
            })
        }
        _ => None,
    }
}

fn interior_bounds(interval: ComputedSourceInterval) -> Option<(f64, f64)> {
    if !strict_finite_interval(interval) {
        return None;
    }
    let epsilon = parameter_tolerance(interval);
    let lower = interval.start + epsilon;
    let upper = interval.end - epsilon;
    (lower < upper).then_some((lower, upper))
}

fn total_parameter(domain: SourceDomain, parameter: f64, winding: i32) -> Option<f64> {
    if !parameter.is_finite() {
        return None;
    }
    let total = match domain {
        SourceDomain::Bounded { lower, upper } => {
            if winding != 0 || parameter < lower || parameter > upper {
                return None;
            }
            parameter
        }
        SourceDomain::Periodic { period }
            if period.is_finite() && period > 0.0 && 0.0 <= parameter && parameter < period =>
        {
            parameter + f64::from(winding) * period
        }
        SourceDomain::Periodic { .. } => return None,
    };
    total.is_finite().then_some(total)
}

fn source_domain_and_interval_are_valid(
    domain: SourceDomain,
    interval: ComputedSourceInterval,
) -> bool {
    if !strict_finite_interval(interval) {
        return false;
    }
    match domain {
        SourceDomain::Bounded { lower, upper } => {
            lower.is_finite()
                && upper.is_finite()
                && lower < upper
                && lower <= interval.start
                && interval.end <= upper
        }
        SourceDomain::Periodic { period } => {
            period.is_finite()
                && period > 0.0
                && interval.end - interval.start
                    <= period
                        + parameter_tolerance(ComputedSourceInterval {
                            start: 0.0,
                            end: period,
                        })
        }
    }
}

fn strict_finite_interval(interval: ComputedSourceInterval) -> bool {
    interval.start.is_finite() && interval.end.is_finite() && interval.start < interval.end
}

fn parameter_strictly_inside(parameter: f64, interval: ComputedSourceInterval) -> bool {
    parameter.is_finite()
        && strict_finite_interval(interval)
        && interval.start < parameter
        && parameter < interval.end
}

fn parameter_tolerance(interval: ComputedSourceInterval) -> f64 {
    let width = (interval.end - interval.start).abs();
    let magnitude = interval.start.abs().max(interval.end.abs()).max(width);
    (width * PARAMETER_EPSILON_FACTOR)
        .max(magnitude * 32.0 * f64::EPSILON)
        .max(f64::MIN_POSITIVE)
}

fn intersect_periodic_visible_interval(
    visible: ComputedSourceInterval,
    retained: ComputedSourceInterval,
    seed_total: f64,
    period: f64,
) -> Option<ComputedSourceInterval> {
    if !strict_finite_interval(visible)
        || !strict_finite_interval(retained)
        || !seed_total.is_finite()
        || !period.is_finite()
        || period <= 0.0
    {
        return None;
    }
    let full_period_tolerance = parameter_tolerance(ComputedSourceInterval {
        start: 0.0,
        end: period,
    });
    let visible_width = visible.end - visible.start;
    let aligned_visible = if (visible_width - period).abs() <= full_period_tolerance {
        retained
    } else {
        let midpoint = visible.start + 0.5 * visible_width;
        let base_shift = ((seed_total - midpoint) / period).round();
        [-1.0, 0.0, 1.0]
            .into_iter()
            .map(|offset| {
                let shift = (base_shift + offset) * period;
                ComputedSourceInterval {
                    start: visible.start + shift,
                    end: visible.end + shift,
                }
            })
            .find(|candidate| parameter_strictly_inside(seed_total, *candidate))?
    };
    let intersection = ComputedSourceInterval {
        start: aligned_visible.start.max(retained.start),
        end: aligned_visible.end.min(retained.end),
    };
    (strict_finite_interval(intersection) && parameter_strictly_inside(seed_total, intersection))
        .then_some(intersection)
}

fn normalize_parameter(domain: SourceDomain, total: f64) -> Option<(f64, i32)> {
    if !total.is_finite() {
        return None;
    }
    match domain {
        SourceDomain::Bounded { lower, upper } => {
            (lower <= total && total <= upper).then_some((total, 0))
        }
        SourceDomain::Periodic { period } if period.is_finite() && period > 0.0 => {
            let parameter = total.rem_euclid(period);
            let winding = ((total - parameter) / period).round();
            if winding < f64::from(i32::MIN) || winding > f64::from(i32::MAX) {
                None
            } else {
                #[allow(clippy::cast_possible_truncation)]
                Some((parameter, winding as i32))
            }
        }
        SourceDomain::Periodic { .. } => None,
    }
}

fn local_fillet_roots(
    sketch: &SketchDocument,
    parents: &[RootParent; 2],
    sides: [DocumentCurveNormalSide; 2],
    radius: f64,
    policy: ComputedFeatureEvaluationPolicy,
    controller: &mut OperationController,
) -> RootSearchResult {
    let seeds = [
        parameter_seeds(&parents[0], policy.root_seed_grid),
        parameter_seeds(&parents[1], policy.root_seed_grid),
    ];
    let mut solutions = Vec::new();
    let mut failure = RootSearchFailure::NoLocalRoot;
    for first in &seeds[0] {
        for second in &seeds[1] {
            if controller
                .charge(
                    OperationWorkCounter::ProfileRoots,
                    1,
                    OperationCheckpoint::ProfileCandidate,
                )
                .is_err()
            {
                return RootSearchResult::Stopped;
            }
            let solution = match local_fillet_root_from_seed(
                sketch,
                parents,
                sides,
                radius,
                [*first, *second],
                policy,
                controller,
            ) {
                RootAttempt::Solution(solution) => solution,
                RootAttempt::Failed(reason) => {
                    failure = failure.merge(reason);
                    continue;
                }
                RootAttempt::Stopped => return RootSearchResult::Stopped,
            };
            if solutions
                .iter()
                .all(|existing| solutions_materially_distinct(sketch, *existing, solution))
            {
                solutions.push(solution);
            }
        }
    }
    RootSearchResult::Completed { solutions, failure }
}

fn parameter_seeds(parent: &RootParent, grid: usize) -> Vec<f64> {
    let mut seeds = vec![parent.seed_total.clamp(parent.bounds.0, parent.bounds.1)];
    let width = parent.bounds.1 - parent.bounds.0;
    let grid_u32 = u32::try_from(grid).expect("validated root seed grid fits u32");
    for index in 0..grid_u32 {
        let fraction = (f64::from(index) + 0.5) / f64::from(grid_u32);
        let value = parent.bounds.0 + fraction * width;
        if seeds
            .iter()
            .all(|existing| existing.to_bits() != value.to_bits())
        {
            seeds.push(value);
        }
    }
    seeds
}

#[allow(
    clippy::too_many_lines,
    reason = "bounded Newton iteration and line search remain one auditable root kernel"
)]
fn local_fillet_root_from_seed(
    sketch: &SketchDocument,
    parents: &[RootParent; 2],
    sides: [DocumentCurveNormalSide; 2],
    radius: f64,
    mut parameters: [f64; 2],
    policy: ComputedFeatureEvaluationPolicy,
    controller: &mut OperationController,
) -> RootAttempt {
    let tolerance = (sketch.model_scale() * GEOMETRY_TOLERANCE_FACTOR).max(1.0e-11);
    let mut observed_failure = RootSearchFailure::NoLocalRoot;
    for _ in 0..policy.max_root_iterations {
        if controller
            .charge(
                OperationWorkCounter::ProfileSubdivisions,
                1,
                OperationCheckpoint::ProfileSubdivision,
            )
            .is_err()
        {
            return RootAttempt::Stopped;
        }
        let first = match offset_geometry(
            sketch,
            parents[0].parent.source.span,
            parameters[0],
            sides[0],
            radius,
        ) {
            Ok(value) => value,
            Err(OffsetGeometryFailure::OffsetSingularity) => {
                return RootAttempt::Failed(RootSearchFailure::OffsetSingularity);
            }
            Err(OffsetGeometryFailure::Invalid) => {
                return RootAttempt::Failed(RootSearchFailure::NoLocalRoot);
            }
        };
        let second = match offset_geometry(
            sketch,
            parents[1].parent.source.span,
            parameters[1],
            sides[1],
            radius,
        ) {
            Ok(value) => value,
            Err(OffsetGeometryFailure::OffsetSingularity) => {
                return RootAttempt::Failed(RootSearchFailure::OffsetSingularity);
            }
            Err(OffsetGeometryFailure::Invalid) => {
                return RootAttempt::Failed(RootSearchFailure::NoLocalRoot);
            }
        };
        let residual = [
            first.point[0] - second.point[0],
            first.point[1] - second.point[1],
        ];
        let norm = residual[0].hypot(residual[1]);
        if norm <= tolerance {
            if !offset_derivatives_are_transverse(first.derivative, second.derivative) {
                return RootAttempt::Failed(RootSearchFailure::SingularParents);
            }
            let center = [
                0.5 * (first.point[0] + second.point[0]),
                0.5 * (first.point[1] + second.point[1]),
            ];
            let score = parents
                .iter()
                .enumerate()
                .map(|(index, parent)| {
                    (parameters[index] - parent.seed_total).abs()
                        / (parent.bounds.1 - parent.bounds.0)
                })
                .sum();
            return RootAttempt::Solution(LocalFilletSolution {
                parameters,
                sides,
                center,
                score,
            });
        }
        let matrix = [
            [first.derivative[0], -second.derivative[0]],
            [first.derivative[1], -second.derivative[1]],
        ];
        let determinant = matrix[0][0] * matrix[1][1] - matrix[0][1] * matrix[1][0];
        let scale = first.derivative[0].hypot(first.derivative[1])
            * second.derivative[0].hypot(second.derivative[1]);
        if !determinant.is_finite()
            || !scale.is_finite()
            || scale <= 0.0
            || determinant.abs() <= PARENT_SINGULARITY_TOLERANCE * scale
        {
            return RootAttempt::Failed(RootSearchFailure::SingularParents);
        }
        let step = [
            (-residual[0] * matrix[1][1] + matrix[0][1] * residual[1]) / determinant,
            (-matrix[0][0] * residual[1] + residual[0] * matrix[1][0]) / determinant,
        ];
        if !step.into_iter().all(f64::is_finite) {
            return RootAttempt::Failed(RootSearchFailure::NoLocalRoot);
        }
        let mut factor = 1.0;
        let mut accepted = false;
        for _ in 0..policy.max_line_search_steps {
            if controller
                .charge(
                    OperationWorkCounter::RejectedTrials,
                    1,
                    OperationCheckpoint::ProfileSubdivision,
                )
                .is_err()
            {
                return RootAttempt::Stopped;
            }
            let candidate = [
                (parameters[0] + factor * step[0]).clamp(parents[0].bounds.0, parents[0].bounds.1),
                (parameters[1] + factor * step[1]).clamp(parents[1].bounds.0, parents[1].bounds.1),
            ];
            let next_first = match offset_geometry(
                sketch,
                parents[0].parent.source.span,
                candidate[0],
                sides[0],
                radius,
            ) {
                Ok(value) => value,
                Err(OffsetGeometryFailure::OffsetSingularity) => {
                    observed_failure = observed_failure.merge(RootSearchFailure::OffsetSingularity);
                    factor *= 0.5;
                    continue;
                }
                Err(OffsetGeometryFailure::Invalid) => {
                    factor *= 0.5;
                    continue;
                }
            };
            let next_second = match offset_geometry(
                sketch,
                parents[1].parent.source.span,
                candidate[1],
                sides[1],
                radius,
            ) {
                Ok(value) => value,
                Err(OffsetGeometryFailure::OffsetSingularity) => {
                    observed_failure = observed_failure.merge(RootSearchFailure::OffsetSingularity);
                    factor *= 0.5;
                    continue;
                }
                Err(OffsetGeometryFailure::Invalid) => {
                    factor *= 0.5;
                    continue;
                }
            };
            let next_norm = (next_first.point[0] - next_second.point[0])
                .hypot(next_first.point[1] - next_second.point[1]);
            if next_norm < norm {
                parameters = candidate;
                accepted = true;
                break;
            }
            factor *= 0.5;
        }
        if !accepted {
            return RootAttempt::Failed(observed_failure);
        }
    }
    RootAttempt::Failed(observed_failure)
}

#[derive(Clone, Copy, Debug)]
struct OffsetGeometry {
    point: [f64; 2],
    derivative: [f64; 2],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum OffsetGeometryFailure {
    Invalid,
    OffsetSingularity,
}

fn offset_geometry(
    sketch: &SketchDocument,
    span: CurveSpan,
    parameter: f64,
    side: DocumentCurveNormalSide,
    radius: f64,
) -> Result<OffsetGeometry, OffsetGeometryFailure> {
    let jet = sketch
        .evaluate_curve_jet(span, parameter)
        .map_err(|_| OffsetGeometryFailure::Invalid)?;
    let differential = jet
        .differential()
        .map_err(|_| OffsetGeometryFailure::Invalid)?;
    let sign = side_sign(side);
    let factor = 1.0 - sign * radius * differential.signed_curvature;
    if !factor.is_finite() || factor.abs() <= OFFSET_SINGULARITY_TOLERANCE {
        return Err(OffsetGeometryFailure::OffsetSingularity);
    }
    let point = [
        jet.position.x + sign * radius * differential.left_normal.x,
        jet.position.y + sign * radius * differential.left_normal.y,
    ];
    let derivative = [
        factor * jet.first_derivative.x,
        factor * jet.first_derivative.y,
    ];
    point
        .into_iter()
        .chain(derivative)
        .all(f64::is_finite)
        .then_some(OffsetGeometry { point, derivative })
        .ok_or(OffsetGeometryFailure::Invalid)
}

fn offset_derivatives_are_transverse(first: [f64; 2], second: [f64; 2]) -> bool {
    let determinant = first[0] * second[1] - first[1] * second[0];
    let scale = first[0].hypot(first[1]) * second[0].hypot(second[1]);
    determinant.is_finite()
        && scale.is_finite()
        && scale > 0.0
        && determinant.abs() > PARENT_SINGULARITY_TOLERANCE * scale
}

#[derive(Clone, Copy, Debug)]
enum RootSelectionFailure {
    None,
    Ambiguous,
}

fn select_solution(
    sketch: &SketchDocument,
    solutions: &[LocalFilletSolution],
) -> Result<LocalFilletSolution, RootSelectionFailure> {
    let Some(solution) = solutions
        .iter()
        .copied()
        .min_by(|left, right| left.score.total_cmp(&right.score))
    else {
        return Err(RootSelectionFailure::None);
    };
    if solutions.iter().any(|other| {
        scores_nearly_tied(solution.score, other.score)
            && solutions_materially_distinct(sketch, solution, *other)
    }) {
        return Err(RootSelectionFailure::Ambiguous);
    }
    Ok(solution)
}

fn scores_nearly_tied(first: f64, second: f64) -> bool {
    let scale = first.abs().max(second.abs()).max(1.0);
    (first - second).abs() <= 1.0e-7 * scale
}

fn solutions_materially_distinct(
    sketch: &SketchDocument,
    first: LocalFilletSolution,
    second: LocalFilletSolution,
) -> bool {
    let position_tolerance = (sketch.model_scale() * ROOT_DEDUPLICATION_FACTOR).max(1.0e-10);
    (first.center[0] - second.center[0]).hypot(first.center[1] - second.center[1])
        > position_tolerance
        || first
            .parameters
            .into_iter()
            .zip(second.parameters)
            .any(|(left, right)| (left - right).abs() > 1.0e-8)
}

#[derive(Clone, Copy, Debug)]
enum ArcValidationFailure {
    OffsetSingularity,
    SingularParents,
    Invalid,
}

#[allow(
    clippy::too_many_lines,
    reason = "independent parent, contact, tangency and arc publication checks remain one fail-closed audit path"
)]
fn build_and_validate_arc(
    sketch: &SketchDocument,
    parents: [ComputedFilletParent; 2],
    solution: LocalFilletSolution,
    radius: f64,
    endpoint_order: DocumentFilletEndpointOrder,
    sweep: DocumentArcSweep,
) -> Result<ComputedCircularArc, ArcValidationFailure> {
    if !radius.is_finite() || radius <= 0.0 || !solution.center.into_iter().all(f64::is_finite) {
        return Err(ArcValidationFailure::Invalid);
    }
    if solution.sides != parents.map(|parent| parent.normal_side) {
        return Err(ArcValidationFailure::Invalid);
    }
    let affine = parents.map(|parent| is_affine_line_span(sketch, parent.source.span));
    if affine == [false, false] {
        return Err(ArcValidationFailure::Invalid);
    }
    let publication_parents = prepare_root_parents(sketch, parents, None)
        .and_then(|prepared| {
            certify_persistent_branch(
                sketch,
                prepared,
                affine,
                ComputedFeatureCornerId::from_raw(0),
            )
        })
        .map_err(|_| ArcValidationFailure::Invalid)?;
    if solution
        .parameters
        .iter()
        .enumerate()
        .any(|(index, parameter)| {
            !parameter_strictly_inside(
                *parameter,
                publication_parents[index].topology.base_interval,
            ) || *parameter < publication_parents[index].bounds.0
                || *parameter > publication_parents[index].bounds.1
        })
    {
        return Err(ArcValidationFailure::Invalid);
    }
    let mut contacts = Vec::with_capacity(2);
    let mut angles = [0.0; 2];
    let mut offset_derivatives = [[0.0; 2]; 2];
    let tolerance = (sketch.model_scale() * GEOMETRY_TOLERANCE_FACTOR).max(1.0e-10);
    for index in 0..2 {
        let span = parents[index].source.span;
        let topology = source_topology(sketch, parents[index].source)
            .map_err(|_| ArcValidationFailure::Invalid)?;
        let jet = sketch
            .evaluate_curve_jet(span, solution.parameters[index])
            .map_err(|_| ArcValidationFailure::Invalid)?;
        let differential = jet
            .differential()
            .map_err(|_| ArcValidationFailure::Invalid)?;
        let sign = side_sign(parents[index].normal_side);
        let regular = 1.0 - sign * radius * differential.signed_curvature;
        if !regular.is_finite() || regular.abs() <= OFFSET_SINGULARITY_TOLERANCE {
            return Err(ArcValidationFailure::OffsetSingularity);
        }
        offset_derivatives[index] = [
            regular * jet.first_derivative.x,
            regular * jet.first_derivative.y,
        ];
        let expected_center = [
            jet.position.x + sign * radius * differential.left_normal.x,
            jet.position.y + sign * radius * differential.left_normal.y,
        ];
        if (expected_center[0] - solution.center[0]).hypot(expected_center[1] - solution.center[1])
            > tolerance
        {
            return Err(ArcValidationFailure::Invalid);
        }
        let radial = [
            jet.position.x - solution.center[0],
            jet.position.y - solution.center[1],
        ];
        let radial_length = radial[0].hypot(radial[1]);
        if !radial_length.is_finite()
            || (radial_length - radius).abs() > tolerance.max(radius * 1.0e-8)
        {
            return Err(ArcValidationFailure::Invalid);
        }
        let tangent_length = jet.first_derivative.norm();
        if !tangent_length.is_finite()
            || tangent_length <= 0.0
            || (radial[0] * jet.first_derivative.x + radial[1] * jet.first_derivative.y).abs()
                > TANGENCY_TOLERANCE * radius * tangent_length
        {
            return Err(ArcValidationFailure::Invalid);
        }
        angles[index] = radial[1].atan2(radial[0]);
        let (parameter, winding) = normalize_parameter(topology.domain, solution.parameters[index])
            .ok_or(ArcValidationFailure::Invalid)?;
        contacts.push(ComputedFilletContact {
            source: parents[index].source,
            parameter,
            winding,
            total_parameter: solution.parameters[index],
            position: [jet.position.x, jet.position.y],
        });
    }
    if !offset_derivatives_are_transverse(offset_derivatives[0], offset_derivatives[1]) {
        return Err(ArcValidationFailure::SingularParents);
    }
    let [first, second]: [ComputedFilletContact; 2] = contacts
        .try_into()
        .map_err(|_| ArcValidationFailure::Invalid)?;
    let (start_angle, end_angle) = match endpoint_order {
        DocumentFilletEndpointOrder::FirstThenSecond => (angles[0], angles[1]),
        DocumentFilletEndpointOrder::SecondThenFirst => (angles[1], angles[0]),
    };
    let sweep_angle = match sweep {
        DocumentArcSweep::CounterClockwise => {
            (end_angle - start_angle).rem_euclid(std::f64::consts::TAU)
        }
        DocumentArcSweep::Clockwise => (start_angle - end_angle).rem_euclid(std::f64::consts::TAU),
    };
    if !sweep_angle.is_finite()
        || sweep_angle <= 1.0e-10
        || sweep_angle >= std::f64::consts::TAU - 1.0e-10
    {
        return Err(ArcValidationFailure::Invalid);
    }
    Ok(ComputedCircularArc {
        center: solution.center,
        radius,
        start_angle,
        end_angle,
        sweep,
        contacts: [first, second],
    })
}

fn contact_angles(
    sketch: &SketchDocument,
    parents: [ComputedFilletParent; 2],
    solution: LocalFilletSolution,
    radius: f64,
) -> Result<[f64; 2], ArcValidationFailure> {
    let arc = build_and_validate_arc(
        sketch,
        parents,
        solution,
        radius,
        DocumentFilletEndpointOrder::FirstThenSecond,
        DocumentArcSweep::CounterClockwise,
    )?;
    Ok([
        (arc.contacts[0].position[1] - arc.center[1])
            .atan2(arc.contacts[0].position[0] - arc.center[0]),
        (arc.contacts[1].position[1] - arc.center[1])
            .atan2(arc.contacts[1].position[0] - arc.center[0]),
    ])
}

const fn map_arc_authoring_failure(failure: ArcValidationFailure) -> ComputedFeatureAuthoringError {
    match failure {
        ArcValidationFailure::OffsetSingularity => ComputedFeatureAuthoringError::OffsetSingularity,
        ArcValidationFailure::SingularParents => ComputedFeatureAuthoringError::SingularParents,
        ArcValidationFailure::Invalid => ComputedFeatureAuthoringError::InvalidResolvedGeometry,
    }
}

const fn side_sign(side: DocumentCurveNormalSide) -> f64 {
    match side {
        DocumentCurveNormalSide::Left => 1.0,
        DocumentCurveNormalSide::Right => -1.0,
    }
}

const fn flip_side(side: DocumentCurveNormalSide) -> DocumentCurveNormalSide {
    match side {
        DocumentCurveNormalSide::Left => DocumentCurveNormalSide::Right,
        DocumentCurveNormalSide::Right => DocumentCurveNormalSide::Left,
    }
}

const fn flip_endpoint_order(order: DocumentFilletEndpointOrder) -> DocumentFilletEndpointOrder {
    match order {
        DocumentFilletEndpointOrder::FirstThenSecond => {
            DocumentFilletEndpointOrder::SecondThenFirst
        }
        DocumentFilletEndpointOrder::SecondThenFirst => {
            DocumentFilletEndpointOrder::FirstThenSecond
        }
    }
}

fn side_rank(sides: [DocumentCurveNormalSide; 2]) -> u8 {
    match sides {
        [DocumentCurveNormalSide::Left, DocumentCurveNormalSide::Left] => 0,
        [
            DocumentCurveNormalSide::Left,
            DocumentCurveNormalSide::Right,
        ] => 1,
        [
            DocumentCurveNormalSide::Right,
            DocumentCurveNormalSide::Left,
        ] => 2,
        [
            DocumentCurveNormalSide::Right,
            DocumentCurveNormalSide::Right,
        ] => 3,
    }
}

fn is_affine_line_span(sketch: &SketchDocument, span: CurveSpan) -> bool {
    sketch
        .curve(span.curve)
        .is_some_and(|curve| match &curve.definition {
            CurveDefinition::Line { .. } => span.segment == 0,
            CurveDefinition::Polyline { points, closed, .. } => {
                let count = if *closed {
                    points.len()
                } else {
                    points.len().saturating_sub(1)
                };
                usize::try_from(span.segment).is_ok_and(|index| index < count)
            }
            _ => false,
        })
}

fn same_open_polyline_adjacent_spans(
    sketch: &SketchDocument,
    first: CurveSpan,
    second: CurveSpan,
) -> bool {
    first.curve == second.curve
        && first.segment.abs_diff(second.segment) == 1
        && sketch.curve(first.curve).is_some_and(|curve| {
            matches!(
                curve.definition,
                CurveDefinition::Polyline { closed: false, .. }
            )
        })
}

#[cfg(test)]
mod publication_tests {
    use super::*;
    use geosolve_sketch::{DocumentId, PersistentId};

    #[test]
    fn independent_arc_publication_rejects_parallel_parent_tangents() {
        let mut sketch =
            SketchDocument::with_id(10.0, DocumentId(PersistentId::from_u128(0x5000))).unwrap();
        let first_start = sketch.add_point("first start", [0.0, 0.0]).unwrap();
        let first_end = sketch.add_point("first end", [4.0, 0.0]).unwrap();
        let second_start = sketch.add_point("second start", [0.0, 1.0]).unwrap();
        let second_end = sketch.add_point("second end", [4.0, 1.0]).unwrap();
        let first = CurveSpan::line(
            sketch
                .add_curve(
                    "first",
                    CurveDefinition::Line {
                        start: first_start,
                        end: first_end,
                        branch_direction: [1.0, 0.0],
                    },
                )
                .unwrap(),
        );
        let second = CurveSpan::line(
            sketch
                .add_curve(
                    "second",
                    CurveDefinition::Line {
                        start: second_start,
                        end: second_end,
                        branch_direction: [1.0, 0.0],
                    },
                )
                .unwrap(),
        );
        let parents = [
            ComputedFilletParent {
                source: NativeCurveSpanSource { span: first },
                picked_parameter: 0.5,
                winding: 0,
                neighborhood: ContactNeighborhood::Interior,
                normal_side: DocumentCurveNormalSide::Left,
                retained_endpoint: DocumentFilletTrimEndpoint::End,
                periodic_anchor: None,
            },
            ComputedFilletParent {
                source: NativeCurveSpanSource { span: second },
                picked_parameter: 0.5,
                winding: 0,
                neighborhood: ContactNeighborhood::Interior,
                normal_side: DocumentCurveNormalSide::Right,
                retained_endpoint: DocumentFilletTrimEndpoint::Start,
                periodic_anchor: None,
            },
        ];
        let solution = LocalFilletSolution {
            parameters: [0.5, 0.5],
            sides: [
                DocumentCurveNormalSide::Left,
                DocumentCurveNormalSide::Right,
            ],
            center: [2.0, 0.5],
            score: 0.0,
        };
        assert!(matches!(
            build_and_validate_arc(
                &sketch,
                parents,
                solution,
                0.5,
                DocumentFilletEndpointOrder::FirstThenSecond,
                DocumentArcSweep::CounterClockwise,
            ),
            Err(ArcValidationFailure::SingularParents)
        ));
    }
}
