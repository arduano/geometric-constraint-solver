// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::BTreeSet;

use geosolve_sketch::{
    ContactDomain, ContactNeighborhood, CurveDefinition, CurveOffsetCertificate, CurveOffsetError,
    CurveOffsetGeometry, CurveOffsetOptions, CurveOffsetTraversal, CurveSpan, DocumentArcSweep,
    DocumentBSplineForm, DocumentConstraintDefinition, DocumentCurveNormalSide,
    DocumentFilletEndpointOrder, DocumentFilletTrimEndpoint, DocumentTrimBoundary,
    DocumentTrimParameter, GeometryRole, OperationCheckpoint, OperationControl,
    OperationController, OperationOutcome, OperationWorkCounter, PreparedSketchInput,
    RetainedSketchDocumentSession, ScalarDomain, ScalarUnit, SketchAcceptedStateIdentity,
    SketchDocument, TangentOrientation, VisualProfileOptions, VisualProfileOrientation,
    VisualProfileStatus, compute_curve_offset_with_controller,
};
use geosolve_sketch_topology::{
    OffsetContourKey, OffsetDirectedSpan, OffsetEndpointRef, OffsetEndpointRole, OffsetFaceKey,
    OffsetJoinOwner, OffsetOperandIndex, OffsetOperandRequest, OffsetTraversal,
    PreparedOffsetOperandQuery,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::document::{
    ComputedCurveOffset, ComputedCurveOffsetDirectedSpan, ComputedCurveOffsetJunction,
    ComputedCurveOffsetJunctionBranch, ComputedCurveOffsetJunctionProvenance,
    ComputedCurveOffsetLoop, ComputedCurveOffsetOperand, ComputedCurveOffsetTraversal,
    ComputedFeature, ComputedFeatureCornerId, ComputedFeatureDefinition, ComputedFeatureDocument,
    ComputedFeatureDocumentError, ComputedFeatureDocumentIdentity, ComputedFeatureId,
    ComputedFilletCorner, ComputedFilletParent, NativeCurveSpanSource, NewComputedFilletCorner,
};

mod composition;

#[cfg(test)]
use composition::SourceComposition;
use composition::{
    EndpointClaim, combined_source_role, compose_source_output, compose_sources,
    composition_failures, construction_fragment_id, edge_id, endpoint_claim,
};

const PARAMETER_EPSILON_FACTOR: f64 = 1.0e-10;
const GEOMETRY_TOLERANCE_FACTOR: f64 = 1.0e-8;
// Newton roots are polished below the independent publication envelope so a
// coalescing root does not appear as several materially different contacts
// merely because each seed first entered the wider geometry tolerance at a
// different point.
const ROOT_POLISH_TOLERANCE_FACTOR: f64 = 1.0e-14;
const ROOT_DEDUPLICATION_FACTOR: f64 = 1.0e-7;
const OFFSET_SINGULARITY_TOLERANCE: f64 = 1.0e-8;
const PARENT_SINGULARITY_TOLERANCE: f64 = 1.0e-8;
// Root acceptance is position-scaled, so an exact fold can be represented by
// a tiny but nonzero transverse angle. Keep the published rail comfortably
// above that numerical root envelope; this dimensionless threshold still
// admits ordinary acute corners while withholding explosive sensitivities.
const RADIUS_SENSITIVITY_MIN_TRANSVERSE_QUALITY: f64 = 1.0e-3;
const RADIUS_RAIL_MIN_NORM: f64 = 1.0e-10;
const CONTINUATION_MAX_ATTEMPTS: usize = 128;
const CONTINUATION_MAX_PARAMETER_FRACTION: f64 = 0.125;
const CONTINUATION_MIN_BRACKET_FRACTION: f64 = 1.0e-6;
const CONTINUATION_MAX_CORRECTION_FRACTION: f64 = 0.25;
const TANGENCY_TOLERANCE: f64 = 2.0e-7;
// An accepted continuation step may turn either regular offset tangent by less
// than one quarter turn. Larger source jumps are rejected so two unseen folds
// cannot masquerade as same-sign branch continuity in one pointer sample.
const CONTINUATION_MIN_TANGENT_DIRECTION_DOT: f64 = 0.0;

/// Bounded deterministic computed-feature evaluation policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ComputedFeatureEvaluationPolicy {
    pub max_features: usize,
    pub max_corners: usize,
    pub max_edges: usize,
    pub max_construction_fragments: usize,
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
            max_construction_fragments: 100_000,
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
            (
                "max_construction_fragments",
                self.max_construction_fragments,
            ),
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
    offset_operand_index: Option<OffsetOperandIndex>,
    offset_operand_query: Option<PreparedOffsetOperandQuery>,
    continuation_hints: Vec<ComputedFilletContinuation>,
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
        let offset_operand_query = capture_offset_operand_query(session, features);
        Ok(Self {
            input: ComputedFeatureEvaluationInput {
                sketch,
                accepted: accepted.identity(),
                features: features.identity(),
                policy,
            },
            sketch: accepted.document().clone(),
            features: features.clone(),
            offset_operand_index: None,
            offset_operand_query,
            continuation_hints: Vec::new(),
        })
    }

    /// Captures current accepted geometry while carrying forward only
    /// authenticated, presentation-independent Fillet branch continuity from
    /// one previous computed snapshot of the same feature document namespace.
    ///
    /// The prior snapshot contributes no geometry authority: current source
    /// geometry is captured from `session`, and every hint is matched again to
    /// the current persistent corner before it can participate in root
    /// selection.
    pub fn capture_continuing_from(
        session: &RetainedSketchDocumentSession,
        features: &ComputedFeatureDocument,
        policy: ComputedFeatureEvaluationPolicy,
        previous: &ComputedFeatureSnapshot,
    ) -> Result<Self, ComputedFeatureSnapshotError> {
        let mut captured = Self::capture(session, features, policy)?;
        if previous.input.features.document != captured.input.features.document
            || previous.input.accepted.document() != captured.input.accepted.document()
        {
            return Err(ComputedFeatureSnapshotError::ContinuationForDifferentWorkspace);
        }
        if previous.input.features != captured.input.features {
            return Err(ComputedFeatureSnapshotError::ContinuationForDifferentFeatureState);
        }
        let same_exact_accepted_input = previous.input.accepted == captured.input.accepted
            && previous.input.sketch == captured.input.sketch;
        let direct_accepted_successor = previous.input.accepted != captured.input.accepted
            && session.last_attempt().parent_accepted_identity() == Some(previous.input.accepted);
        let exact_preview_continuation =
            session.last_attempt().continuation_parent_input() == Some(previous.input.sketch);
        if !same_exact_accepted_input && !direct_accepted_successor && !exact_preview_continuation {
            return Err(ComputedFeatureSnapshotError::ContinuationForUnrelatedAcceptedState);
        }
        captured.continuation_hints = previous
            .fillet_continuations
            .iter()
            .chain(&previous.retained_fillet_continuations)
            .copied()
            .collect();
        Ok(captured)
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

fn capture_offset_operand_query(
    session: &RetainedSketchDocumentSession,
    features: &ComputedFeatureDocument,
) -> Option<PreparedOffsetOperandQuery> {
    let required = features.features().iter().any(|feature| {
        matches!(
            &feature.definition,
            ComputedFeatureDefinition::CurveOffset(_)
        )
    });
    if !required {
        return None;
    }
    PreparedOffsetOperandQuery::capture(session, OffsetOperandRequest::default()).ok()
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

    /// Returns the exact accepted sketch document captured for authoring.
    #[must_use]
    pub const fn sketch_document(&self) -> &SketchDocument {
        &self.sketch
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

    /// Continues one previously resolved corner at a new radius without
    /// re-running coordinate-derived branch selection.
    ///
    /// `from_radius` must reproduce the prior contact seeds exactly. Source
    /// identities, normal sides, retained endpoints, contact neighbourhoods,
    /// endpoint order and sweep remain absolute. A bounded adaptive homotopy
    /// advances from that exact root; a disconnected correction, fold or loss
    /// of offset regularity rejects rather than selecting a remote root.
    pub fn continue_fillet_corner(
        &self,
        prior: NewComputedFilletCorner,
        from_radius: f64,
        radius: f64,
        policy: ComputedFeatureEvaluationPolicy,
        control: OperationControl,
    ) -> Result<OperationOutcome<ContinuedComputedFilletCorner>, ComputedFeatureAuthoringError>
    {
        match self.continue_fillet_corners(&[prior], from_radius, radius, policy, control)? {
            OperationOutcome::Completed { mut value, report } => value
                .pop()
                .map(|value| OperationOutcome::Completed { value, report })
                .ok_or(ComputedFeatureAuthoringError::InvalidContinuationState),
            OperationOutcome::Cancelled { report } => Ok(OperationOutcome::Cancelled { report }),
            OperationOutcome::WorkExhausted { report } => {
                Ok(OperationOutcome::WorkExhausted { report })
            }
            _ => Err(ComputedFeatureAuthoringError::InvalidContinuationState),
        }
    }

    /// Continues an ordered corner batch under one aggregate bounded work
    /// envelope from one explicit prior shared radius. Every corner preserves
    /// its own absolute branch state; a stopped or failed later corner never
    /// publishes a partial result vector.
    pub fn continue_fillet_corners(
        &self,
        priors: &[NewComputedFilletCorner],
        from_radius: f64,
        radius: f64,
        policy: ComputedFeatureEvaluationPolicy,
        control: OperationControl,
    ) -> Result<OperationOutcome<Vec<ContinuedComputedFilletCorner>>, ComputedFeatureAuthoringError>
    {
        policy.validate()?;
        validate_authoring_radius(from_radius)?;
        validate_authoring_radius(radius)?;
        let mut controller = OperationController::new(control);
        if !charge_fillet_corner_validation(&mut controller, priors.len()) {
            return Ok(controller.outcome_unchecked());
        }
        let mut values = Vec::with_capacity(priors.len());
        for prior in priors {
            let resolved = match continue_absolute_corner(
                &self.sketch,
                prior.canonicalized(),
                from_radius,
                radius,
                policy,
                &mut controller,
            )? {
                AbsoluteCornerResolution::Stopped => return Ok(controller.outcome_unchecked()),
                AbsoluteCornerResolution::Completed(resolved) => resolved,
            };
            values.push(self.stamp_continuation(*resolved));
        }
        Ok(controller.outcome(values))
    }

    /// Applies an explicit numeric shared-radius edit to an ordered corner batch.
    ///
    /// Regular origins use the same adaptive continuation as pointer dragging.
    /// An exact affine/non-affine origin whose radius rail is withheld at a fold
    /// may instead depart through its already-persisted local branch cell. That
    /// fallback remains bounded and rejects an absent, tied or remote target
    /// root; it does not make the fold itself draggable or globally enumerate
    /// alternatives.
    pub fn continue_fillet_corners_numeric(
        &self,
        priors: &[NewComputedFilletCorner],
        from_radius: f64,
        radius: f64,
        policy: ComputedFeatureEvaluationPolicy,
        control: OperationControl,
    ) -> Result<OperationOutcome<Vec<ContinuedComputedFilletCorner>>, ComputedFeatureAuthoringError>
    {
        policy.validate()?;
        validate_authoring_radius(from_radius)?;
        validate_authoring_radius(radius)?;
        let mut controller = OperationController::new(control);
        if !charge_fillet_corner_validation(&mut controller, priors.len()) {
            return Ok(controller.outcome_unchecked());
        }
        let mut values = Vec::with_capacity(priors.len());
        for prior in priors {
            let resolved = match continue_numeric_absolute_corner(
                &self.sketch,
                prior.canonicalized(),
                from_radius,
                radius,
                policy,
                &mut controller,
            )? {
                AbsoluteCornerResolution::Stopped => return Ok(controller.outcome_unchecked()),
                AbsoluteCornerResolution::Completed(resolved) => resolved,
            };
            values.push(self.stamp_continuation(*resolved));
        }
        Ok(controller.outcome(values))
    }

    /// Reseeds one named parent from an exact native-source hit while retaining
    /// the other parent and every explicit absolute branch choice.
    ///
    /// Periodic parameters are aligned to the winding nearest the prior contact.
    /// Multiple materially distinct roots tied to the named hit are reported as
    /// typed ambiguity rather than guessed.
    pub fn reseed_fillet_contact(
        &self,
        request: ComputedFilletContactReseedRequest,
        radius: f64,
        policy: ComputedFeatureEvaluationPolicy,
        control: OperationControl,
    ) -> Result<OperationOutcome<ContinuedComputedFilletCorner>, ComputedFeatureAuthoringError>
    {
        policy.validate()?;
        validate_authoring_radius(radius)?;
        let mut controller = OperationController::new(control);
        if !charge_fillet_corner_validation(&mut controller, 1) {
            return Ok(controller.outcome_unchecked());
        }
        if request.prior.first.source == request.prior.second.source {
            return Err(ComputedFeatureAuthoringError::InvalidContinuationState);
        }
        let prior = request.prior.canonicalized();
        let parent = if prior.first.source == request.prior.first.source {
            request.parent
        } else {
            flip_parent_index(request.parent)
        };
        let prior = reseeded_absolute_corner(&self.sketch, prior, parent, request.parameter)?;
        match resolve_explicit_absolute_corner(
            &self.sketch,
            prior,
            radius,
            policy,
            &mut controller,
            Some(parent),
        )? {
            AbsoluteCornerResolution::Stopped => Ok(controller.outcome_unchecked()),
            AbsoluteCornerResolution::Completed(resolved) => {
                Ok(controller.outcome(self.stamp_continuation(*resolved)))
            }
        }
    }

    /// Enumerates a small deterministic set of independently validated local
    /// alternatives around one absolute corner.
    ///
    /// The closed set contains the current continuation, viable normal-side
    /// pairs, each single-parent retained-direction reversal and the
    /// complementary arc. There is no global root enumeration and no partial
    /// result is published after cancellation or bounded-work exhaustion.
    #[allow(
        clippy::too_many_lines,
        reason = "the fixed seven-action alternative catalog remains one auditable bounded order"
    )]
    pub fn local_fillet_corner_alternatives(
        &self,
        prior: NewComputedFilletCorner,
        radius: f64,
        policy: ComputedFeatureEvaluationPolicy,
        control: OperationControl,
    ) -> Result<OperationOutcome<Vec<ComputedFilletCornerAlternative>>, ComputedFeatureAuthoringError>
    {
        policy.validate()?;
        validate_authoring_radius(radius)?;
        let mut controller = OperationController::new(control);
        if !charge_fillet_corner_validation(&mut controller, 1) {
            return Ok(controller.outcome_unchecked());
        }
        let prior = prior.canonicalized();
        let base = match resolve_seed_connected_absolute_corner(
            &self.sketch,
            prior,
            radius,
            policy,
            &mut controller,
        )? {
            AbsoluteCornerResolution::Completed(value) => *value,
            AbsoluteCornerResolution::Stopped => return Ok(controller.outcome_unchecked()),
        };
        let mut alternatives = vec![ComputedFilletCornerAlternative {
            kind: ComputedFilletCornerAlternativeKind::Current,
            resolved: self.stamp_continuation(base.clone()),
        }];

        for first in [
            DocumentCurveNormalSide::Left,
            DocumentCurveNormalSide::Right,
        ] {
            for second in [
                DocumentCurveNormalSide::Left,
                DocumentCurveNormalSide::Right,
            ] {
                if [first, second] == [prior.first.normal_side, prior.second.normal_side] {
                    continue;
                }
                let mut candidate = prior;
                candidate.first.normal_side = first;
                candidate.second.normal_side = second;
                match resolve_explicit_absolute_corner(
                    &self.sketch,
                    candidate,
                    radius,
                    policy,
                    &mut controller,
                    None,
                ) {
                    Ok(AbsoluteCornerResolution::Completed(resolved)) => {
                        alternatives.push(ComputedFilletCornerAlternative {
                            kind: ComputedFilletCornerAlternativeKind::NormalSides {
                                first,
                                second,
                            },
                            resolved: self.stamp_continuation(*resolved),
                        });
                    }
                    Ok(AbsoluteCornerResolution::Stopped) => {
                        return Ok(controller.outcome_unchecked());
                    }
                    Err(error) if local_alternative_is_unavailable(&error) => {}
                    Err(error) => return Err(error),
                }
            }
        }

        for parent in [
            ComputedFilletParentIndex::First,
            ComputedFilletParentIndex::Second,
        ] {
            let mut candidate = base.corner;
            let intent = match parent {
                ComputedFilletParentIndex::First => candidate.first,
                ComputedFilletParentIndex::Second => candidate.second,
            };
            if !source_topology(&self.sketch, intent.source)
                .is_ok_and(SourceTopology::participates_in_trimming)
            {
                continue;
            }
            let endpoint = flip_trim_endpoint(match parent {
                ComputedFilletParentIndex::First => candidate.first.retained_endpoint,
                ComputedFilletParentIndex::Second => candidate.second.retained_endpoint,
            });
            if let Err(error) =
                set_retained_endpoint(&self.sketch, &mut candidate, parent, endpoint)
            {
                if local_alternative_is_unavailable(&error) {
                    continue;
                }
                return Err(error);
            }
            match resolve_exact_absolute_corner(&self.sketch, candidate, radius) {
                Ok(resolved) => {
                    alternatives.push(ComputedFilletCornerAlternative {
                        kind: ComputedFilletCornerAlternativeKind::RetainedEndpoint {
                            parent,
                            endpoint,
                        },
                        resolved: self.stamp_continuation(resolved),
                    });
                }
                Err(error) if local_alternative_is_unavailable(&error) => {}
                Err(error) => return Err(error),
            }
        }

        let mut complement = base.corner;
        complement.endpoint_order = flip_endpoint_order(complement.endpoint_order);
        match resolve_exact_absolute_corner(&self.sketch, complement, radius) {
            Ok(resolved) => {
                alternatives.push(ComputedFilletCornerAlternative {
                    kind: ComputedFilletCornerAlternativeKind::ComplementaryArc,
                    resolved: self.stamp_continuation(resolved),
                });
            }
            Err(error) if local_alternative_is_unavailable(&error) => {}
            Err(error) => return Err(error),
        }
        debug_assert!(alternatives.len() <= 7);
        Ok(controller.outcome(alternatives))
    }

    fn stamp_continuation(
        &self,
        resolved: AbsoluteCornerContinuation,
    ) -> ContinuedComputedFilletCorner {
        ContinuedComputedFilletCorner {
            sketch_input: self.sketch_input,
            accepted: self.accepted,
            corner: resolved.corner,
            arc: resolved.arc,
            sensitivity: resolved.sensitivity,
        }
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
        let mut controller = OperationController::new(control);
        let Some(result) = self.execute_in_controller(&mut controller)? else {
            return Ok(controller.outcome_unchecked());
        };
        Ok(controller.outcome(result))
    }

    /// Evaluates this snapshot inside a caller-owned compound operation.
    ///
    /// A stopped controller returns `Ok(None)` without publishing any state;
    /// callers may continue to use its exact cancellation/work report.
    ///
    /// # Errors
    ///
    /// Returns the same structural and policy failures as [`Self::execute`].
    pub fn execute_in_controller(
        self,
        controller: &mut OperationController,
    ) -> Result<Option<ComputedFeatureSnapshot>, ComputedFeatureEvaluationError> {
        let policy = self.snapshot.input.policy;
        policy.validate()?;
        let feature_count = self.snapshot.features.features().len();
        let corner_count = self
            .snapshot
            .features
            .features()
            .iter()
            .map(|feature| match &feature.definition {
                ComputedFeatureDefinition::FilletSet(fillet) => fillet.corners.len(),
                ComputedFeatureDefinition::CurveOffset(_) => 0,
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
        if controller
            .charge(
                OperationWorkCounter::DocumentValidationItems,
                feature_count.saturating_add(corner_count),
                OperationCheckpoint::DocumentValidation,
            )
            .is_err()
        {
            return Ok(None);
        }
        let mut snapshot = self.snapshot;
        if let Some(query) = snapshot.offset_operand_query.take() {
            let outcome = query.execute(controller.child_control());
            snapshot.offset_operand_index = match outcome {
                Ok(OperationOutcome::Completed { value, report }) => {
                    if controller.absorb_child_report(report).is_err() {
                        return Ok(None);
                    }
                    value.operand_index
                }
                Ok(
                    OperationOutcome::Cancelled { report }
                    | OperationOutcome::WorkExhausted { report },
                ) => {
                    let _ = controller.absorb_child_report(report);
                    return Ok(None);
                }
                Err(_) => None,
                Ok(_) => return Ok(None),
            };
        }
        let result = evaluate_snapshot(&snapshot, self.evaluation, controller)?;
        if controller.is_stopped() {
            return Ok(None);
        }
        if controller
            .checkpoint(OperationCheckpoint::BeforeFinalValidation)
            .is_err()
        {
            return Ok(None);
        }
        Ok(Some(result))
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
    #[error("computed-feature continuation belongs to a different workspace namespace")]
    ContinuationForDifferentWorkspace,
    #[error("computed-feature continuation does not match the current persistent feature state")]
    ContinuationForDifferentFeatureState,
    #[error("computed-feature continuation is not the same accepted input or its direct successor")]
    ContinuationForUnrelatedAcceptedState,
}

/// Failure to turn one coherent completed evaluation into refreshed persistent
/// contact seeds and Local certificates after a native-source edit.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ComputedFeatureReanchorError {
    #[error("computed-feature snapshot does not match the supplied feature document")]
    SnapshotInputMismatch,
    #[error("computed-feature snapshot is incomplete or inconsistent with feature state")]
    IncompleteEvaluation,
    #[error(transparent)]
    Document(#[from] ComputedFeatureDocumentError),
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

/// Finite first-order response of one absolute Fillet branch to its shared
/// radius. Derivatives are with respect to one model-unit increase in radius;
/// contact parameter derivatives use each source's total parameter, including
/// winding for periodic curves.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ComputedFilletRadiusSensitivity {
    pub center_derivative: [f64; 2],
    pub contact_parameter_derivatives: [f64; 2],
    pub contact_position_derivatives: [[f64; 2]; 2],
    /// Scale-independent sine-like quality of the two offset tangents. Values
    /// close to zero approach a branch fold and are not exposed as a drag rail.
    pub transverse_quality: f64,
}

/// Same-branch continuation result for one previously resolved absolute corner.
#[derive(Clone, Debug, PartialEq)]
pub struct ContinuedComputedFilletCorner {
    pub sketch_input: PreparedSketchInput,
    pub accepted: SketchAcceptedStateIdentity,
    pub corner: NewComputedFilletCorner,
    pub arc: ComputedCircularArc,
    pub sensitivity: ComputedFilletRadiusSensitivity,
}

/// Stable semantic index of one Fillet parent.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ComputedFilletParentIndex {
    First,
    Second,
}

impl ComputedFilletParentIndex {
    const fn index(self) -> usize {
        match self {
            Self::First => 0,
            Self::Second => 1,
        }
    }
}

const fn flip_parent_index(parent: ComputedFilletParentIndex) -> ComputedFilletParentIndex {
    match parent {
        ComputedFilletParentIndex::First => ComputedFilletParentIndex::Second,
        ComputedFilletParentIndex::Second => ComputedFilletParentIndex::First,
    }
}

/// Exact accepted-source contact reseed. The source identity and every other
/// absolute branch choice come from `prior`; presentation supplies only a
/// finite parameter on the named source.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ComputedFilletContactReseedRequest {
    pub prior: NewComputedFilletCorner,
    pub parent: ComputedFilletParentIndex,
    pub parameter: f64,
}

/// Closed local alternatives around one absolute corner. These are semantic
/// actions rather than relative booleans; each result carries complete
/// independently validated replacement intent.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ComputedFilletCornerAlternativeKind {
    Current,
    NormalSides {
        first: DocumentCurveNormalSide,
        second: DocumentCurveNormalSide,
    },
    RetainedEndpoint {
        parent: ComputedFilletParentIndex,
        endpoint: DocumentFilletTrimEndpoint,
    },
    ComplementaryArc,
}

/// One bounded local branch alternative and its finite radius rail.
#[derive(Clone, Debug, PartialEq)]
pub struct ComputedFilletCornerAlternative {
    pub kind: ComputedFilletCornerAlternativeKind,
    pub resolved: ContinuedComputedFilletCorner,
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
    #[error(
        "same-curve Fillet parents must be adjacent or explicitly Coincident-joined spans of one open polyline"
    )]
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
    #[error("the prior Fillet corner is not valid absolute continuation state")]
    InvalidContinuationState,
    #[error("the selected Fillet contact reseed is outside its native source domain")]
    InvalidContactReseed,
    #[error("the Fillet radius rail is ill-conditioned at a branch fold")]
    IllConditionedRadiusSensitivity,
    #[error("the Fillet radius sensitivity produced a non-finite result")]
    NonFiniteRadiusSensitivity,
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

/// Generated construction-fragment identity scoped to one evaluation revision.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ComputedConstructionFragmentId {
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

/// Explicit orientation of the two regular offset tangents at one Fillet root.
/// The sign can change only at a genuine transverse branch fold.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ComputedFilletTransverseOrientation {
    Negative,
    Positive,
}

/// Authenticated current branch metadata emitted by feature evaluation for a
/// later accepted native-source edit. `corner` is re-anchored to the published
/// contacts and carries refreshed explicit Local/winding/periodic state.
#[derive(Clone, Copy, Debug, PartialEq)]
struct ComputedFilletContinuation {
    owner: ComputedCornerRef,
    radius: f64,
    corner: NewComputedFilletCorner,
    transverse_orientation: ComputedFilletTransverseOrientation,
    offset_tangent_directions: [[f64; 2]; 2],
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
    /// Source-derivative versus arc-derivative branch at each first/second contact.
    pub tangent_orientations: [TangentOrientation; 2],
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
    CurveOffset(CurveOffsetGeometry),
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
    CurveOffset {
        owner: ComputedFeatureId,
        source: NativeCurveSpanSource,
        /// Native source parameters paired with this generated edge's traversal endpoints.
        /// `None` identifies a junction connector that has no honest inverse-edit
        /// correspondence to the attributed source span.
        source_parameters: Option<[f64; 2]>,
    },
}

/// One revision-local generated edge and its stable provenance.
#[derive(Clone, Debug, PartialEq)]
pub struct ComputedEdge {
    pub id: ComputedEdgeId,
    /// Effective profile eligibility inherited from the accepted native sources.
    pub role: GeometryRole,
    pub geometry: ComputedEdgeGeometry,
    pub provenance: ComputedEdgeProvenance,
}

/// Exact native-source attribution for one interval discarded by a successful
/// computed Fillet claim.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ComputedConstructionFragmentProvenance {
    /// Stable feature/corner that owns the successful endpoint claim.
    pub owner: ComputedCornerRef,
    /// Endpoint of the retained source interval created at the Fillet contact.
    pub endpoint: DocumentFilletTrimEndpoint,
    /// Complete accepted visible interval from which this complement was cut.
    pub base_interval: ComputedSourceInterval,
}

/// One evaluation-local discarded native interval presented as implicit
/// construction geometry outside the constrained sketch graph.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ComputedConstructionFragment {
    pub id: ComputedConstructionFragmentId,
    pub source: NativeCurveSpanSource,
    pub interval: ComputedSourceInterval,
    /// Persistent role of the native source before computed-feature composition.
    pub source_role: GeometryRole,
    pub provenance: ComputedConstructionFragmentProvenance,
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
    #[error("Curve Offset references missing native span {span_source:?}")]
    OffsetMissingSource { span_source: NativeCurveSpanSource },
    #[error("Curve Offset source {span_source:?} cannot produce a regular parallel curve: {kind}")]
    OffsetCurveFailure {
        span_source: NativeCurveSpanSource,
        kind: &'static str,
    },
    #[error("Curve Offset has an invalid or unavailable {kind} junction")]
    OffsetJunctionFailure { kind: &'static str },
    #[error("Curve Offset output self-intersects, touches, or changes topology")]
    OffsetTopologyChange,
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
        generated_edges: Vec<ComputedEdgeId>,
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
    construction_fragments: Vec<ComputedConstructionFragment>,
    features: Vec<ComputedFeatureEvaluation>,
    replaced_sources: Vec<NativeCurveSpanSource>,
    fillet_continuations: Vec<ComputedFilletContinuation>,
    retained_fillet_continuations: Vec<ComputedFilletContinuation>,
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

    /// Returns all discarded source complements produced by successful
    /// computed-feature composition. These are always implicit construction
    /// geometry and never persistent sketch objects.
    #[must_use]
    pub fn construction_fragments(&self) -> &[ComputedConstructionFragment] {
        &self.construction_fragments
    }

    #[must_use]
    pub fn feature_evaluations(&self) -> &[ComputedFeatureEvaluation] {
        &self.features
    }

    #[must_use]
    pub fn replaced_sources(&self) -> &[NativeCurveSpanSource] {
        &self.replaced_sources
    }

    /// Produces a feature document whose explicit Fillet contact parameters,
    /// windings, periodic anchors and Local certificates match every Current
    /// set in this evaluation. Failed sets retain their prior persistent intent,
    /// and retained-last-valid hints are never promoted. The input document is
    /// never mutated on failure.
    ///
    /// # Errors
    ///
    /// Returns an error for a mismatched input snapshot, an incomplete or
    /// inconsistent feature evaluation, or an invalid atomic replacement.
    pub fn reanchored_feature_document(
        &self,
        features: &ComputedFeatureDocument,
    ) -> Result<ComputedFeatureDocument, ComputedFeatureReanchorError> {
        if self.input.features != features.identity() {
            return Err(ComputedFeatureReanchorError::SnapshotInputMismatch);
        }
        let mut reanchored = features.clone();
        for feature in features.features() {
            let evaluation = self
                .features
                .iter()
                .find(|evaluation| evaluation.feature == feature.id)
                .ok_or(ComputedFeatureReanchorError::IncompleteEvaluation)?;
            let ComputedFeatureDefinition::FilletSet(fillet) = &feature.definition else {
                if matches!(
                    (&evaluation.state, feature.suppressed),
                    (ComputedFeatureEvaluationState::Suppressed, true)
                        | (
                            ComputedFeatureEvaluationState::Failed { .. }
                                | ComputedFeatureEvaluationState::Current { .. },
                            false,
                        )
                ) {
                    continue;
                }
                return Err(ComputedFeatureReanchorError::IncompleteEvaluation);
            };
            match (&evaluation.state, feature.suppressed) {
                (ComputedFeatureEvaluationState::Suppressed, true)
                | (ComputedFeatureEvaluationState::Failed { .. }, false) => {}
                (ComputedFeatureEvaluationState::Current { .. }, false) => {
                    let corners = fillet
                        .corners
                        .iter()
                        .map(|corner| {
                            self.fillet_continuations
                                .iter()
                                .find(|continuation| {
                                    continuation.owner
                                        == (ComputedCornerRef {
                                            feature: feature.id,
                                            corner: corner.id,
                                        })
                                })
                                .map(|continuation| (corner.id, continuation.corner))
                        })
                        .collect::<Option<Vec<_>>>()
                        .ok_or(ComputedFeatureReanchorError::IncompleteEvaluation)?;
                    reanchored.replace_fillet_set(feature.id, fillet.radius, corners)?;
                }
                (
                    ComputedFeatureEvaluationState::Current { .. }
                    | ComputedFeatureEvaluationState::Failed { .. },
                    true,
                )
                | (ComputedFeatureEvaluationState::Suppressed, false) => {
                    return Err(ComputedFeatureReanchorError::IncompleteEvaluation);
                }
            }
        }
        Ok(reanchored)
    }

    #[must_use]
    pub fn edge(&self, id: ComputedEdgeId) -> Option<&ComputedEdge> {
        (id.evaluation == self.evaluation)
            .then(|| self.edges.get(id.ordinal as usize))
            .flatten()
    }

    /// Resolves one revision-local discarded construction fragment.
    #[must_use]
    pub fn construction_fragment(
        &self,
        id: ComputedConstructionFragmentId,
    ) -> Option<&ComputedConstructionFragment> {
        (id.evaluation == self.evaluation)
            .then(|| self.construction_fragments.get(id.ordinal as usize))
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

    /// Resolves the discarded construction complements for one native source.
    pub fn source_construction_fragments(
        &self,
        source: NativeCurveSpanSource,
    ) -> impl Iterator<Item = &ComputedConstructionFragment> {
        self.construction_fragments
            .iter()
            .filter(move |fragment| fragment.source == source)
    }

    /// Resolves discarded construction complements owned by one Fillet corner.
    pub fn fillet_construction_fragments(
        &self,
        owner: ComputedCornerRef,
    ) -> impl Iterator<Item = &ComputedConstructionFragment> {
        self.construction_fragments
            .iter()
            .filter(move |fragment| fragment.provenance.owner == owner)
    }
}

#[derive(Clone, Copy, Debug)]
struct SourceTopology {
    domain: SourceDomain,
    base_interval: ComputedSourceInterval,
}

impl SourceTopology {
    fn participates_in_trimming(self) -> bool {
        match self.domain {
            SourceDomain::Bounded { .. } => true,
            SourceDomain::Periodic { period } => {
                let full_period = ComputedSourceInterval {
                    start: 0.0,
                    end: period,
                };
                (self.base_interval.end - self.base_interval.start - period).abs()
                    > parameter_tolerance(full_period)
            }
        }
    }
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

enum AbsoluteCornerResolution {
    Completed(Box<AbsoluteCornerContinuation>),
    Stopped,
}

#[derive(Clone)]
struct AbsoluteCornerContinuation {
    radius: f64,
    corner: NewComputedFilletCorner,
    arc: ComputedCircularArc,
    sensitivity: ComputedFilletRadiusSensitivity,
    signed_transverse_quality: f64,
}

type PreparedAbsoluteCorner = ([ComputedFilletParent; 2], [bool; 2], [RootParent; 2]);

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
    role: GeometryRole,
    arc: ComputedCircularArc,
    claims: [EndpointClaim; 2],
    continuation: ComputedFilletContinuation,
}

#[derive(Clone, Debug)]
struct EvaluatedFeatureCandidate {
    feature: ComputedFeatureId,
    corners: Vec<EvaluatedCorner>,
}

#[derive(Clone, Debug)]
struct EvaluatedCurveOffsetEdge {
    role: GeometryRole,
    source: NativeCurveSpanSource,
    /// Native source parameters paired with the generated edge's traversal endpoints.
    /// Junction-only connector geometry deliberately carries no correspondence.
    source_parameters: Option<[f64; 2]>,
    geometry: CurveOffsetGeometry,
    certificate: CurveOffsetCertificate,
    /// Position-error bounds at each fitted patch endpoint. Fresh Hermite patches are exact at
    /// both ends. Curved inner-miter trimming can introduce a certified nonzero error at the new
    /// endpoint, which the mathematical error-tube validator must retain rather than silently
    /// treating the split point as another exact Hermite boundary.
    patch_endpoint_position_errors: Vec<[f64; 2]>,
}

#[derive(Clone, Debug)]
struct EvaluatedCurveOffsetPath {
    closed: bool,
    edges: Vec<EvaluatedCurveOffsetEdge>,
}

#[derive(Clone, Debug)]
struct EvaluatedCurveOffsetCandidate {
    feature: ComputedFeatureId,
    edges: Vec<EvaluatedCurveOffsetEdge>,
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
    let mut offset_candidates = Vec::new();
    let mut evaluations = Vec::new();

    for feature in snapshot.features.features() {
        if feature.suppressed {
            evaluations.push(ComputedFeatureEvaluation {
                feature: feature.id,
                state: ComputedFeatureEvaluationState::Suppressed,
            });
            continue;
        }
        let evaluated = match &feature.definition {
            ComputedFeatureDefinition::FilletSet(_) => evaluate_feature(
                &snapshot.sketch,
                feature,
                &snapshot.continuation_hints,
                snapshot.input.policy,
                controller,
            )
            .map(EvaluatedCandidate::Fillet),
            ComputedFeatureDefinition::CurveOffset(offset) => evaluate_curve_offset_feature(
                &snapshot.sketch,
                snapshot.offset_operand_index.as_ref(),
                feature.id,
                offset,
                snapshot.input.policy,
                controller,
            )
            .map(EvaluatedCandidate::CurveOffset),
        };
        match evaluated {
            Ok(EvaluatedCandidate::Fillet(candidate)) => candidates.push(candidate),
            Ok(EvaluatedCandidate::CurveOffset(candidate)) => offset_candidates.push(candidate),
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

    // Active computed Fillets own source replacement before any computed Offset is composed.
    // Curve Offset remains source-only in V1/V2 and therefore cannot silently consume either the
    // discarded native interval or the revision-local Fillet arc.
    let fillet_replaced_sources = candidates
        .iter()
        .flat_map(|candidate| candidate.corners.iter())
        .flat_map(|corner| corner.claims)
        .map(|claim| claim.source)
        .collect::<BTreeSet<_>>();
    let mut offset_source_conflicts = Vec::new();
    offset_candidates.retain(|candidate| {
        let conflict = candidate
            .edges
            .iter()
            .map(|edge| edge.source)
            .find(|source| fillet_replaced_sources.contains(source));
        if let Some(span_source) = conflict {
            offset_source_conflicts.push((candidate.feature, span_source));
            false
        } else {
            true
        }
    });
    evaluations.extend(
        offset_source_conflicts
            .into_iter()
            .map(|(feature, span_source)| ComputedFeatureEvaluation {
                feature,
                state: ComputedFeatureEvaluationState::Failed {
                    failure: ComputedFeatureFailure::OffsetCurveFailure {
                        span_source,
                        kind: "active computed Fillet source replacement",
                    },
                },
            }),
    );

    let compositions = compose_sources(&candidates);
    let mut edges = Vec::new();
    let mut construction_fragments = Vec::new();
    let mut replaced_sources = Vec::new();
    let mut fillet_continuations = Vec::new();
    for composition in compositions.values() {
        let output = compose_source_output(composition)?;
        let source_role = snapshot
            .sketch
            .geometry_role(composition.source.span.curve)
            .ok_or(ComputedFeatureEvaluationError::InvalidGeneratedTopology {
                resource: "source role",
            })?;
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
            role: source_role,
            geometry: ComputedEdgeGeometry::NativeSourceFragment {
                source: composition.source,
                interval: output.effective_interval,
            },
            provenance: ComputedEdgeProvenance::SourceFragment {
                source: composition.source,
                interval: output.effective_interval,
                start_claim: composition.start.map(|claim| claim.owner),
                end_claim: composition.end.map(|claim| claim.owner),
            },
        });
        for discarded in output.discarded {
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
            if construction_fragments.len() >= snapshot.input.policy.max_construction_fragments {
                return Err(ComputedFeatureEvaluationError::PolicyLimitExceeded {
                    resource: "construction fragments",
                    actual: construction_fragments.len().saturating_add(1),
                    limit: snapshot.input.policy.max_construction_fragments,
                });
            }
            let id = construction_fragment_id(evaluation, construction_fragments.len())?;
            construction_fragments.push(ComputedConstructionFragment {
                id,
                source: composition.source,
                interval: discarded.interval,
                source_role,
                provenance: ComputedConstructionFragmentProvenance {
                    owner: discarded.claim.owner,
                    endpoint: discarded.claim.endpoint,
                    base_interval: discarded.claim.base_interval,
                },
            });
        }
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
                role: corner.role,
                geometry: ComputedEdgeGeometry::CircularArc(corner.arc.clone()),
                provenance: ComputedEdgeProvenance::FilletArc {
                    owner: corner.owner,
                    sources: corner.claims.map(|claim| claim.source),
                },
            });
            corner_edges.push((corner.owner.corner, id));
            fillet_continuations.push(corner.continuation);
        }
        evaluations.push(ComputedFeatureEvaluation {
            feature: candidate.feature,
            state: ComputedFeatureEvaluationState::Current {
                corner_edges,
                generated_edges: Vec::new(),
            },
        });
    }
    for candidate in offset_candidates {
        let mut generated_edges = Vec::with_capacity(candidate.edges.len());
        for edge in candidate.edges {
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
            generated_edges.push(id);
            edges.push(ComputedEdge {
                id,
                role: edge.role,
                geometry: ComputedEdgeGeometry::CurveOffset(edge.geometry),
                provenance: ComputedEdgeProvenance::CurveOffset {
                    owner: candidate.feature,
                    source: edge.source,
                    source_parameters: edge.source_parameters,
                },
            });
        }
        evaluations.push(ComputedFeatureEvaluation {
            feature: candidate.feature,
            state: ComputedFeatureEvaluationState::Current {
                corner_edges: Vec::new(),
                generated_edges,
            },
        });
    }
    evaluations.sort_by_key(|evaluation| evaluation.feature);
    replaced_sources.sort();
    replaced_sources.dedup();
    let mut owners = fillet_continuations
        .iter()
        .map(|continuation| continuation.owner)
        .collect::<BTreeSet<_>>();
    let retained_fillet_continuations = snapshot
        .continuation_hints
        .iter()
        .filter(|continuation| {
            continuation_matches_feature_document(continuation, &snapshot.features)
                && owners.insert(continuation.owner)
        })
        .copied()
        .collect();
    Ok(ComputedFeatureSnapshot {
        input: snapshot.input,
        evaluation,
        edges,
        construction_fragments,
        features: evaluations,
        replaced_sources,
        fillet_continuations,
        retained_fillet_continuations,
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
        construction_fragments: Vec::new(),
        features: Vec::new(),
        replaced_sources: Vec::new(),
        fillet_continuations: Vec::new(),
        retained_fillet_continuations: Vec::new(),
    }
}

enum EvaluateFeatureError {
    Stopped,
    Failure(ComputedFeatureFailure),
}

enum EvaluatedCandidate {
    Fillet(EvaluatedFeatureCandidate),
    CurveOffset(EvaluatedCurveOffsetCandidate),
}

fn authenticate_curve_offset_operand(
    index: &OffsetOperandIndex,
    operand: &ComputedCurveOffsetOperand,
) -> Result<(), EvaluateFeatureError> {
    match operand {
        ComputedCurveOffsetOperand::Face { outer, holes, .. } => {
            let key = OffsetFaceKey {
                outer: offset_contour_key(outer),
                holes: holes.iter().map(offset_contour_key).collect(),
            };
            let candidate = index
                .face(&key)
                .filter(|candidate| candidate.computed_eligibility.is_eligible())
                .ok_or_else(offset_topology_change)?;
            if candidate.key != key {
                return Err(offset_topology_change());
            }
            authenticate_curve_offset_loop(index, outer)?;
            for hole in holes {
                authenticate_curve_offset_loop(index, hole)?;
            }
        }
        ComputedCurveOffsetOperand::OpenChain { chain, .. } => {
            if chain.spans.is_empty()
                || chain.spans.iter().any(|directed| {
                    index.span(directed.source.span).is_none_or(|candidate| {
                        candidate.periodic || !candidate.computed_eligibility.is_eligible()
                    })
                })
                || chain.junctions.len() != chain.spans.len().saturating_sub(1)
            {
                return Err(offset_topology_change());
            }
            for (pair, junction) in chain.spans.windows(2).zip(&chain.junctions) {
                authenticate_curve_offset_junction(index, pair[0], pair[1], *junction)?;
            }
        }
    }
    Ok(())
}

fn authenticate_curve_offset_loop(
    index: &OffsetOperandIndex,
    loop_intent: &ComputedCurveOffsetLoop,
) -> Result<(), EvaluateFeatureError> {
    if loop_intent.spans.is_empty()
        || loop_intent.spans.iter().any(|directed| {
            index
                .span(directed.source.span)
                .is_none_or(|candidate| !candidate.computed_eligibility.is_eligible())
        })
    {
        return Err(offset_topology_change());
    }
    let periodic_single = loop_intent.spans.len() == 1
        && index
            .span(loop_intent.spans[0].source.span)
            .is_some_and(|candidate| candidate.periodic);
    if periodic_single {
        return loop_intent
            .junctions
            .is_empty()
            .then_some(())
            .ok_or_else(offset_topology_change);
    }
    if loop_intent.junctions.len() != loop_intent.spans.len() {
        return Err(offset_topology_change());
    }
    for index_in_loop in 0..loop_intent.spans.len() {
        authenticate_curve_offset_junction(
            index,
            loop_intent.spans[index_in_loop],
            loop_intent.spans[(index_in_loop + 1) % loop_intent.spans.len()],
            loop_intent.junctions[index_in_loop],
        )?;
    }
    Ok(())
}

fn offset_contour_key(loop_intent: &ComputedCurveOffsetLoop) -> OffsetContourKey {
    OffsetContourKey {
        spans: loop_intent
            .spans
            .iter()
            .copied()
            .map(topology_directed_span)
            .collect(),
    }
}

const fn topology_directed_span(directed: ComputedCurveOffsetDirectedSpan) -> OffsetDirectedSpan {
    OffsetDirectedSpan {
        span: directed.source.span,
        traversal: match directed.traversal {
            ComputedCurveOffsetTraversal::Forward => OffsetTraversal::Forward,
            ComputedCurveOffsetTraversal::Reverse => OffsetTraversal::Reverse,
        },
    }
}

const fn topology_directed_endpoint(
    directed: ComputedCurveOffsetDirectedSpan,
    start: bool,
) -> OffsetEndpointRef {
    let endpoint = match (directed.traversal, start) {
        (ComputedCurveOffsetTraversal::Forward, true)
        | (ComputedCurveOffsetTraversal::Reverse, false) => OffsetEndpointRole::Start,
        (ComputedCurveOffsetTraversal::Forward, false)
        | (ComputedCurveOffsetTraversal::Reverse, true) => OffsetEndpointRole::End,
    };
    OffsetEndpointRef {
        span: directed.source.span,
        endpoint,
    }
}

fn authenticate_curve_offset_junction(
    index: &OffsetOperandIndex,
    current: ComputedCurveOffsetDirectedSpan,
    next: ComputedCurveOffsetDirectedSpan,
    junction: ComputedCurveOffsetJunction,
) -> Result<(), EvaluateFeatureError> {
    let current_end = topology_directed_endpoint(current, false);
    let next_start = topology_directed_endpoint(next, true);
    let retained_owner = match junction.provenance {
        ComputedCurveOffsetJunctionProvenance::SharedPoint(point) => {
            OffsetJoinOwner::SharedPoint(point)
        }
        ComputedCurveOffsetJunctionProvenance::Constraint(constraint) => {
            OffsetJoinOwner::Constraint(constraint)
        }
        ComputedCurveOffsetJunctionProvenance::IntrinsicSpanBoundary => {
            OffsetJoinOwner::IntrinsicSpanBoundary
        }
    };
    if index
        .adjacency_owners(current_end, next_start)
        .is_none_or(|owners| !owners.contains(&retained_owner))
    {
        return Err(offset_topology_change());
    }
    Ok(())
}

fn evaluate_curve_offset_feature(
    sketch: &SketchDocument,
    operand_index: Option<&OffsetOperandIndex>,
    feature: ComputedFeatureId,
    offset: &ComputedCurveOffset,
    policy: ComputedFeatureEvaluationPolicy,
    controller: &mut OperationController,
) -> Result<EvaluatedCurveOffsetCandidate, EvaluateFeatureError> {
    let mut options = CurveOffsetOptions::for_model_scale(sketch.model_scale());
    options.max_patches = options.max_patches.min(policy.max_edges);
    let mut paths = Vec::new();
    match &offset.operand {
        ComputedCurveOffsetOperand::OpenChain { side, chain } => {
            let signed_distance = match side {
                geosolve_sketch::DocumentLineSide::Left => offset.distance,
                geosolve_sketch::DocumentLineSide::Right => -offset.distance,
            };
            paths.push(evaluate_curve_offset_path(
                sketch,
                &chain.spans,
                &chain.junctions,
                false,
                signed_distance,
                options,
                controller,
            )?);
        }
        ComputedCurveOffsetOperand::Face {
            direction,
            outer,
            holes,
        } => {
            let signed_distance = match direction {
                geosolve_sketch::DocumentFaceOffsetDirection::Outward => -offset.distance,
                geosolve_sketch::DocumentFaceOffsetDirection::Inward => offset.distance,
            };
            paths.push(evaluate_curve_offset_loop(
                sketch,
                outer,
                signed_distance,
                options,
                controller,
            )?);
            for hole in holes {
                paths.push(evaluate_curve_offset_loop(
                    sketch,
                    hole,
                    signed_distance,
                    options,
                    controller,
                )?);
            }
        }
    }
    authenticate_curve_offset_operand(
        operand_index.ok_or_else(offset_topology_change)?,
        &offset.operand,
    )?;
    validate_curve_offset_topology(
        &paths,
        sketch.model_scale(),
        matches!(offset.operand, ComputedCurveOffsetOperand::Face { .. }),
        controller,
    )?;
    let edges = paths.into_iter().flat_map(|path| path.edges).collect();
    Ok(EvaluatedCurveOffsetCandidate { feature, edges })
}

fn evaluate_curve_offset_loop(
    sketch: &SketchDocument,
    source: &ComputedCurveOffsetLoop,
    signed_distance: f64,
    options: CurveOffsetOptions,
    controller: &mut OperationController,
) -> Result<EvaluatedCurveOffsetPath, EvaluateFeatureError> {
    evaluate_curve_offset_path(
        sketch,
        &source.spans,
        &source.junctions,
        true,
        signed_distance,
        options,
        controller,
    )
}

#[allow(
    clippy::too_many_lines,
    reason = "the ordered source, junction and terminal transaction is clearer as one fail-closed path evaluator"
)]
fn evaluate_curve_offset_path(
    sketch: &SketchDocument,
    sources: &[ComputedCurveOffsetDirectedSpan],
    junctions: &[ComputedCurveOffsetJunction],
    closed: bool,
    signed_distance: f64,
    options: CurveOffsetOptions,
    controller: &mut OperationController,
) -> Result<EvaluatedCurveOffsetPath, EvaluateFeatureError> {
    let valid_junction_count = if closed && sources.len() == 1 {
        junctions.len() <= 1
    } else {
        let expected = if closed {
            sources.len()
        } else {
            sources.len().saturating_sub(1)
        };
        junctions.len() == expected
    };
    if sources.is_empty() || !valid_junction_count {
        return Err(EvaluateFeatureError::Failure(
            ComputedFeatureFailure::OffsetJunctionFailure {
                kind: "source topology",
            },
        ));
    }
    let mut source_edges = Vec::with_capacity(sources.len());
    for source in sources {
        if controller
            .checkpoint(OperationCheckpoint::ProfileSubdivision)
            .is_err()
        {
            return Err(EvaluateFeatureError::Stopped);
        }
        let role =
            sketch
                .geometry_role(source.source.span.curve)
                .ok_or(EvaluateFeatureError::Failure(
                    ComputedFeatureFailure::OffsetMissingSource {
                        span_source: source.source,
                    },
                ))?;
        if role != GeometryRole::Profile {
            return Err(EvaluateFeatureError::Failure(
                ComputedFeatureFailure::OffsetCurveFailure {
                    span_source: source.source,
                    kind: "non-profile source",
                },
            ));
        }
        let traversal = match source.traversal {
            ComputedCurveOffsetTraversal::Forward => CurveOffsetTraversal::Forward,
            ComputedCurveOffsetTraversal::Reverse => CurveOffsetTraversal::Reverse,
        };
        let Some(result) = compute_curve_offset_with_controller(
            sketch,
            source.source.span,
            traversal,
            signed_distance,
            options,
            controller,
        )
        .map_err(|error| {
            EvaluateFeatureError::Failure(ComputedFeatureFailure::OffsetCurveFailure {
                span_source: source.source,
                kind: curve_offset_error_kind(&error),
            })
        })?
        else {
            return Err(EvaluateFeatureError::Stopped);
        };
        source_edges.push(EvaluatedCurveOffsetEdge {
            role,
            source: source.source,
            source_parameters: Some(result.source_parameters),
            patch_endpoint_position_errors: curve_offset_patch_endpoint_errors(&result.geometry),
            geometry: result.geometry,
            certificate: result.certificate,
        });
    }

    let mut junction_edges = vec![Vec::new(); source_edges.len()];
    for index in 0..source_edges.len() {
        let Some(junction) = junctions.get(index) else {
            continue;
        };
        let next_index = (index + 1) % source_edges.len();
        let resolution = curve_offset_junction_edges(
            &source_edges[index],
            &source_edges[next_index],
            *junction,
            signed_distance.abs(),
            sketch.model_scale(),
            options,
            controller,
        )?;
        if let Some(edge) = resolution.current_edge {
            source_edges[index] = edge;
        }
        if let Some(edge) = resolution.next_edge {
            source_edges[next_index] = edge;
        }
        junction_edges[index] = resolution.connectors;
    }
    let mut output = Vec::new();
    for (edge, connectors) in source_edges.into_iter().zip(junction_edges) {
        output.push(edge);
        output.extend(connectors);
    }
    Ok(EvaluatedCurveOffsetPath {
        closed,
        edges: output,
    })
}

fn curve_offset_error_kind(error: &CurveOffsetError) -> &'static str {
    match error {
        CurveOffsetError::InvalidDistance => "invalid distance",
        CurveOffsetError::InvalidOptions => "invalid evaluation policy",
        CurveOffsetError::InvalidSource => "missing or invalid source",
        CurveOffsetError::InvalidGeometry => "non-finite or rational-pole geometry",
        CurveOffsetError::IrregularSource => "uncertified zero-speed source",
        CurveOffsetError::OffsetCusp => "curvature cusp",
        CurveOffsetError::ApproximationToleranceUnmet(_) => "approximation tolerance",
        CurveOffsetError::PatchLimitExceeded(_) => "approximation budget",
        _ => "unsupported curve-offset failure",
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "tangent and miter branch certification intentionally stays in one explicit junction dispatcher"
)]
fn curve_offset_junction_edges(
    current: &EvaluatedCurveOffsetEdge,
    next: &EvaluatedCurveOffsetEdge,
    junction: ComputedCurveOffsetJunction,
    distance: f64,
    model_scale: f64,
    options: CurveOffsetOptions,
    controller: &mut OperationController,
) -> Result<CurveOffsetJunctionResolution, EvaluateFeatureError> {
    let (_, current_end, _, current_tangent) = curve_offset_terminals(&current.geometry)?;
    let (next_start, _, next_tangent, _) = curve_offset_terminals(&next.geometry)?;
    let scale = model_scale
        .abs()
        .max(distance)
        .max(norm(current_end))
        .max(norm(next_start))
        .max(1.0);
    let tolerance = 1.0e-8 * model_scale.abs() + 256.0 * f64::EPSILON * scale;
    match junction.branch {
        ComputedCurveOffsetJunctionBranch::Tangent => {
            if distance_between(current_end, next_start) > tolerance
                || dot(current_tangent, next_tangent) < 1.0 - TANGENCY_TOLERANCE
            {
                return Err(EvaluateFeatureError::Failure(
                    ComputedFeatureFailure::OffsetJunctionFailure { kind: "tangent" },
                ));
            }
            Ok(CurveOffsetJunctionResolution::default())
        }
        ComputedCurveOffsetJunctionBranch::Miter { turn } => {
            let turn_cross = cross(current_tangent, next_tangent);
            let turn_matches = match turn {
                crate::ComputedCurveOffsetTurn::Left => turn_cross > TANGENCY_TOLERANCE,
                crate::ComputedCurveOffsetTurn::Right => turn_cross < -TANGENCY_TOLERANCE,
            };
            let denominator = turn_cross;
            if !turn_matches || denominator.abs() <= TANGENCY_TOLERANCE {
                return Err(EvaluateFeatureError::Failure(
                    ComputedFeatureFailure::OffsetJunctionFailure {
                        kind: "miter branch",
                    },
                ));
            }
            let displacement = subtract(next_start, current_end);
            let first_parameter = cross(displacement, next_tangent) / denominator;
            let second_parameter = cross(displacement, current_tangent) / denominator;
            let miter = add(current_end, scale_vector(current_tangent, first_parameter));
            let remote_limit = 1_000.0 * distance.max(tolerance);
            if !miter.into_iter().all(f64::is_finite)
                || distance_between(current_end, miter) > remote_limit
                || distance_between(next_start, miter) > remote_limit
            {
                return Err(EvaluateFeatureError::Failure(
                    ComputedFeatureFailure::OffsetJunctionFailure {
                        kind: "remote miter",
                    },
                ));
            }
            let trims_current = first_parameter < -tolerance;
            let trims_next = second_parameter > tolerance;
            if trims_current != trims_next {
                return Err(EvaluateFeatureError::Failure(
                    ComputedFeatureFailure::OffsetJunctionFailure {
                        kind: "inconsistent miter extent",
                    },
                ));
            }
            if !trims_current {
                let mut connectors = Vec::new();
                if distance_between(current_end, miter) > tolerance {
                    connectors.push(exact_offset_connector(current.source, current_end, miter));
                }
                if distance_between(miter, next_start) > tolerance {
                    connectors.push(exact_offset_connector(next.source, miter, next_start));
                }
                return Ok(CurveOffsetJunctionResolution {
                    current_edge: None,
                    next_edge: None,
                    connectors,
                });
            }

            if matches!(current.geometry, CurveOffsetGeometry::Line { .. })
                && matches!(next.geometry, CurveOffsetGeometry::Line { .. })
            {
                let CurveOffsetGeometry::Line {
                    start,
                    end: current_end,
                } = current.geometry
                else {
                    unreachable!("line-line branch was checked")
                };
                let CurveOffsetGeometry::Line {
                    start: next_start,
                    end,
                } = next.geometry
                else {
                    unreachable!("line-line branch was checked")
                };
                let current_fraction = line_parameter_at_point(start, current_end, miter)?;
                let next_fraction = line_parameter_at_point(next_start, end, miter)?;
                let mut current_edge = current.clone();
                current_edge.geometry = CurveOffsetGeometry::Line { start, end: miter };
                current_edge.source_parameters = trimmed_source_parameters(
                    current.source_parameters,
                    CurveOffsetTrimSide::Prefix,
                    current_fraction,
                )?;
                let mut next_edge = next.clone();
                next_edge.geometry = CurveOffsetGeometry::Line { start: miter, end };
                next_edge.source_parameters = trimmed_source_parameters(
                    next.source_parameters,
                    CurveOffsetTrimSide::Suffix,
                    next_fraction,
                )?;
                return Ok(CurveOffsetJunctionResolution {
                    current_edge: Some(current_edge),
                    next_edge: Some(next_edge),
                    connectors: Vec::new(),
                });
            }

            let intersection = certify_curved_miter_intersection(
                current,
                next,
                tolerance,
                remote_limit,
                model_scale,
                controller,
            )?;
            let current_edge = trim_curve_offset_edge(
                current,
                intersection.current,
                CurveOffsetTrimSide::Prefix,
                intersection.position,
                tolerance,
            )?;
            let next_edge = trim_curve_offset_edge(
                next,
                intersection.next,
                CurveOffsetTrimSide::Suffix,
                intersection.position,
                tolerance,
            )?;
            certify_trimmed_curve_offset_fit(&current_edge, options)?;
            certify_trimmed_curve_offset_fit(&next_edge, options)?;
            let (_, trimmed_current_end, _, _) = curve_offset_terminals(&current_edge.geometry)?;
            let (trimmed_next_start, _, _, _) = curve_offset_terminals(&next_edge.geometry)?;
            if distance_between(trimmed_current_end, trimmed_next_start) > tolerance {
                return Err(EvaluateFeatureError::Failure(
                    ComputedFeatureFailure::OffsetJunctionFailure {
                        kind: "uncertified curved miter closure",
                    },
                ));
            }
            Ok(CurveOffsetJunctionResolution {
                current_edge: Some(current_edge),
                next_edge: Some(next_edge),
                connectors: Vec::new(),
            })
        }
    }
}

#[derive(Default)]
struct CurveOffsetJunctionResolution {
    current_edge: Option<EvaluatedCurveOffsetEdge>,
    next_edge: Option<EvaluatedCurveOffsetEdge>,
    connectors: Vec<EvaluatedCurveOffsetEdge>,
}

#[derive(Clone, Copy, Debug)]
struct CurveOffsetGeometryParameter {
    part: usize,
    parameter: f64,
}

#[derive(Clone, Copy, Debug)]
struct CertifiedCurvedMiterIntersection {
    current: CurveOffsetGeometryParameter,
    next: CurveOffsetGeometryParameter,
    position: [f64; 2],
}

#[derive(Clone, Copy, Debug)]
enum CurveOffsetTrimSide {
    Prefix,
    Suffix,
}

fn line_parameter_at_point(
    start: [f64; 2],
    end: [f64; 2],
    point: [f64; 2],
) -> Result<f64, EvaluateFeatureError> {
    let direction = subtract(end, start);
    let denominator = dot(direction, direction);
    let parameter = dot(subtract(point, start), direction) / denominator;
    let tolerance = 4_096.0 * f64::EPSILON;
    if !parameter.is_finite()
        || denominator <= 0.0
        || parameter < -tolerance
        || parameter > 1.0 + tolerance
    {
        return Err(curved_miter_failure(
            "invalid analytic miter correspondence",
        ));
    }
    Ok(parameter.clamp(0.0, 1.0))
}

fn trimmed_source_parameters(
    source_parameters: Option<[f64; 2]>,
    side: CurveOffsetTrimSide,
    parameter: f64,
) -> Result<Option<[f64; 2]>, EvaluateFeatureError> {
    let [start, end] =
        source_parameters.ok_or_else(|| curved_miter_failure("missing source correspondence"))?;
    let middle = (end - start).mul_add(parameter, start);
    if !start.is_finite()
        || !end.is_finite()
        || !parameter.is_finite()
        || !(0.0..=1.0).contains(&parameter)
        || !middle.is_finite()
    {
        return Err(curved_miter_failure("invalid source correspondence"));
    }
    Ok(Some(match side {
        CurveOffsetTrimSide::Prefix => [start, middle],
        CurveOffsetTrimSide::Suffix => [middle, end],
    }))
}

fn certify_curved_miter_intersection(
    current: &EvaluatedCurveOffsetEdge,
    next: &EvaluatedCurveOffsetEdge,
    tolerance: f64,
    remote_limit: f64,
    model_scale: f64,
    controller: &mut OperationController,
) -> Result<CertifiedCurvedMiterIntersection, EvaluateFeatureError> {
    let mut document = SketchDocument::new(model_scale.abs().max(f64::MIN_POSITIVE))
        .map_err(|_| curved_miter_failure("curved miter intersection setup"))?;
    let current_spans = add_fitted_miter_operand(&mut document, current, 0, tolerance)?;
    let next_spans = add_fitted_miter_operand(&mut document, next, 1, tolerance)?;
    let span_count = current_spans
        .len()
        .checked_add(next_spans.len())
        .ok_or_else(|| curved_miter_failure("curved miter intersection budget"))?;
    let candidate_pairs = span_count
        .checked_mul(span_count.saturating_add(1))
        .and_then(|value| value.checked_div(2))
        .filter(|value| *value <= 2_000_000)
        .ok_or_else(|| curved_miter_failure("curved miter intersection budget"))?;
    let mut options = VisualProfileOptions::default();
    options.max_candidate_pairs = options.max_candidate_pairs.max(candidate_pairs);
    let analysis =
        match document.analyze_visual_profiles_controlled(options, controller.child_control()) {
            OperationOutcome::Completed { value, report } => {
                controller
                    .absorb_child_report(report)
                    .map_err(|_| EvaluateFeatureError::Stopped)?;
                value
            }
            OperationOutcome::Cancelled { report } | OperationOutcome::WorkExhausted { report } => {
                let _ = controller.absorb_child_report(report);
                return Err(EvaluateFeatureError::Stopped);
            }
            _ => return Err(EvaluateFeatureError::Stopped),
        };
    if analysis.status != VisualProfileStatus::Complete || !analysis.issues.is_empty() {
        return Err(curved_miter_failure(
            "uncertified curved miter intersection",
        ));
    }

    let mut roots = Vec::new();
    for root in &analysis.intersections {
        let direct = fitted_miter_parameter(
            &current_spans,
            root.first_span,
            root.first_parameter_enclosure,
        )
        .zip(fitted_miter_parameter(
            &next_spans,
            root.second_span,
            root.second_parameter_enclosure,
        ));
        let reverse = fitted_miter_parameter(
            &current_spans,
            root.second_span,
            root.second_parameter_enclosure,
        )
        .zip(fitted_miter_parameter(
            &next_spans,
            root.first_span,
            root.first_parameter_enclosure,
        ));
        match (direct, reverse) {
            (Some(root), None) | (None, Some(root)) => roots.push(root),
            _ => {
                return Err(curved_miter_failure("ambiguous curved miter intersection"));
            }
        }
    }
    let [(current_parameter, next_parameter)] = roots.as_slice() else {
        return Err(curved_miter_failure("non-unique curved miter intersection"));
    };
    let (current_parameter, next_parameter, current_position, next_position) =
        refine_fitted_miter_intersection(
            &current.geometry,
            *current_parameter,
            &next.geometry,
            *next_parameter,
            tolerance,
        )?;
    let current_is_fitted = matches!(current.geometry, CurveOffsetGeometry::CubicPatches(_));
    let next_is_fitted = matches!(next.geometry, CurveOffsetGeometry::CubicPatches(_));
    let position = match (current_is_fitted, next_is_fitted) {
        (true, false) => next_position,
        (false, true | false) => current_position,
        (true, true) => scale_vector(add(current_position, next_position), 0.5),
    };
    let (_, current_end, _, _) = curve_offset_terminals(&current.geometry)?;
    let (next_start, _, _, _) = curve_offset_terminals(&next.geometry)?;
    if !position.into_iter().all(f64::is_finite)
        || distance_between(current_position, next_position) > tolerance
        || distance_between(current_end, position) > remote_limit
        || distance_between(next_start, position) > remote_limit
    {
        return Err(curved_miter_failure("remote curved miter intersection"));
    }
    Ok(CertifiedCurvedMiterIntersection {
        current: current_parameter,
        next: next_parameter,
        position,
    })
}

fn add_fitted_miter_operand(
    document: &mut SketchDocument,
    edge: &EvaluatedCurveOffsetEdge,
    operand: usize,
    tolerance: f64,
) -> Result<Vec<CurveSpan>, EvaluateFeatureError> {
    if matches!(
        edge.geometry,
        CurveOffsetGeometry::CircularArc { closed: true, .. }
    ) {
        return Err(curved_miter_failure("closed curved miter operand"));
    }
    let (start, end, _, _) = curve_offset_terminals(&edge.geometry)?;
    let start_point = document
        .add_point(format!("curved miter operand {operand} start"), start)
        .map_err(|_| curved_miter_failure("curved miter intersection setup"))?;
    let end_point = document
        .add_point(format!("curved miter operand {operand} end"), end)
        .map_err(|_| curved_miter_failure("curved miter intersection setup"))?;
    add_fitted_offset_geometry(
        document,
        &edge.geometry,
        Some(start_point),
        Some(end_point),
        operand,
        0,
        tolerance,
    )
    .map_err(|error| match error {
        EvaluateFeatureError::Stopped => EvaluateFeatureError::Stopped,
        EvaluateFeatureError::Failure(_) => curved_miter_failure("curved miter intersection setup"),
    })
}

fn fitted_miter_parameter(
    spans: &[CurveSpan],
    span: CurveSpan,
    enclosure: [f64; 2],
) -> Option<CurveOffsetGeometryParameter> {
    let part = spans.iter().position(|candidate| *candidate == span)?;
    let [lower, upper] = enclosure;
    if !lower.is_finite() || !upper.is_finite() || lower > upper {
        return None;
    }
    let parameter = lower + 0.5 * (upper - lower);
    let parameter_tolerance = 4_096.0 * f64::EPSILON;
    if parameter < -parameter_tolerance || parameter > 1.0 + parameter_tolerance {
        return None;
    }
    Some(CurveOffsetGeometryParameter {
        part,
        parameter: parameter.clamp(0.0, 1.0),
    })
}

fn refine_fitted_miter_intersection(
    current: &CurveOffsetGeometry,
    mut current_parameter: CurveOffsetGeometryParameter,
    next: &CurveOffsetGeometry,
    mut next_parameter: CurveOffsetGeometryParameter,
    tolerance: f64,
) -> Result<
    (
        CurveOffsetGeometryParameter,
        CurveOffsetGeometryParameter,
        [f64; 2],
        [f64; 2],
    ),
    EvaluateFeatureError,
> {
    for _ in 0..24 {
        let (current_position, current_derivative) =
            curve_offset_geometry_sample(current, current_parameter)?;
        let (next_position, next_derivative) = curve_offset_geometry_sample(next, next_parameter)?;
        let residual = subtract(current_position, next_position);
        let coordinate_scale = norm(current_position).max(norm(next_position)).max(1.0);
        let root_tolerance = (4_096.0 * f64::EPSILON * coordinate_scale)
            .min(tolerance * 1.0e-3)
            .max(f64::MIN_POSITIVE);
        if norm(residual) <= root_tolerance {
            return Ok((
                current_parameter,
                next_parameter,
                current_position,
                next_position,
            ));
        }
        let second_column = scale_vector(next_derivative, -1.0);
        let determinant = cross(current_derivative, second_column);
        let derivative_scale = norm(current_derivative)
            .max(norm(next_derivative))
            .max(f64::MIN_POSITIVE);
        if !determinant.is_finite()
            || determinant.abs() <= 4_096.0 * f64::EPSILON * derivative_scale.powi(2)
        {
            break;
        }
        let right_hand_side = scale_vector(residual, -1.0);
        let current_step = cross(right_hand_side, second_column) / determinant;
        let next_step = cross(current_derivative, right_hand_side) / determinant;
        let updated_current = current_parameter.parameter + current_step;
        let updated_next = next_parameter.parameter + next_step;
        if !updated_current.is_finite()
            || !updated_next.is_finite()
            || !(-1.0e-10..=1.0 + 1.0e-10).contains(&updated_current)
            || !(-1.0e-10..=1.0 + 1.0e-10).contains(&updated_next)
        {
            break;
        }
        current_parameter.parameter = updated_current.clamp(0.0, 1.0);
        next_parameter.parameter = updated_next.clamp(0.0, 1.0);
    }
    Err(curved_miter_failure(
        "uncertified curved miter intersection",
    ))
}

fn curve_offset_geometry_sample(
    geometry: &CurveOffsetGeometry,
    parameter: CurveOffsetGeometryParameter,
) -> Result<([f64; 2], [f64; 2]), EvaluateFeatureError> {
    if !parameter.parameter.is_finite() || !(0.0..=1.0).contains(&parameter.parameter) {
        return Err(curved_miter_failure("invalid curved miter parameter"));
    }
    let result = match geometry {
        CurveOffsetGeometry::Line { start, end } if parameter.part == 0 => (
            lerp_point(*start, *end, parameter.parameter),
            subtract(*end, *start),
        ),
        CurveOffsetGeometry::CircularArc {
            center,
            radius,
            start_angle,
            sweep,
            ..
        } if parameter.part == 0 => {
            let angle = sweep.mul_add(parameter.parameter, *start_angle);
            (
                [
                    radius.mul_add(angle.cos(), center[0]),
                    radius.mul_add(angle.sin(), center[1]),
                ],
                scale_vector([-angle.sin(), angle.cos()], radius * sweep),
            )
        }
        CurveOffsetGeometry::CubicPatches(patches) => {
            let patch = patches
                .get(parameter.part)
                .ok_or_else(|| curved_miter_failure("invalid curved miter patch"))?;
            cubic_point_and_derivative(patch.controls, parameter.parameter)
        }
        CurveOffsetGeometry::Line { .. } | CurveOffsetGeometry::CircularArc { .. } => {
            return Err(curved_miter_failure("invalid curved miter part"));
        }
    };
    result
        .0
        .into_iter()
        .chain(result.1)
        .all(f64::is_finite)
        .then_some(result)
        .ok_or_else(|| curved_miter_failure("non-finite curved miter geometry"))
}

#[allow(
    clippy::too_many_lines,
    reason = "each analytic and fitted trim case must preserve one explicit all-or-nothing certificate update"
)]
fn trim_curve_offset_edge(
    edge: &EvaluatedCurveOffsetEdge,
    location: CurveOffsetGeometryParameter,
    side: CurveOffsetTrimSide,
    fitted_join: [f64; 2],
    tolerance: f64,
) -> Result<EvaluatedCurveOffsetEdge, EvaluateFeatureError> {
    let parameter_epsilon = 4_096.0 * f64::EPSILON;
    let mut trimmed = edge.clone();
    match &edge.geometry {
        CurveOffsetGeometry::Line { start, end } if location.part == 0 => {
            let intersection = lerp_point(*start, *end, location.parameter);
            trimmed.geometry = match side {
                CurveOffsetTrimSide::Prefix if location.parameter > parameter_epsilon => {
                    CurveOffsetGeometry::Line {
                        start: *start,
                        end: intersection,
                    }
                }
                CurveOffsetTrimSide::Suffix if location.parameter < 1.0 - parameter_epsilon => {
                    CurveOffsetGeometry::Line {
                        start: intersection,
                        end: *end,
                    }
                }
                _ => return Err(curved_miter_failure("degenerate curved miter trim")),
            };
            trimmed.source_parameters =
                trimmed_source_parameters(edge.source_parameters, side, location.parameter)?;
        }
        CurveOffsetGeometry::CircularArc {
            center,
            radius,
            start_angle,
            sweep,
            ..
        } if location.part == 0 => {
            let (trimmed_start, trimmed_sweep) = match side {
                CurveOffsetTrimSide::Prefix if location.parameter > parameter_epsilon => {
                    (*start_angle, sweep * location.parameter)
                }
                CurveOffsetTrimSide::Suffix if location.parameter < 1.0 - parameter_epsilon => (
                    sweep.mul_add(location.parameter, *start_angle),
                    sweep * (1.0 - location.parameter),
                ),
                _ => return Err(curved_miter_failure("degenerate curved miter trim")),
            };
            trimmed.geometry = CurveOffsetGeometry::CircularArc {
                center: *center,
                radius: *radius,
                start_angle: trimmed_start,
                sweep: trimmed_sweep,
                closed: false,
            };
            trimmed.source_parameters =
                trimmed_source_parameters(edge.source_parameters, side, location.parameter)?;
        }
        CurveOffsetGeometry::CubicPatches(patches) => {
            let endpoint_errors = &edge.patch_endpoint_position_errors;
            if patches.is_empty()
                || endpoint_errors.len() != patches.len()
                || location.part >= patches.len()
            {
                return Err(curved_miter_failure("invalid curved miter patch"));
            }
            let patch = &patches[location.part];
            let parameter = location.parameter;
            let [mut first, mut second] = split_cubic_controls_at(patch.controls, parameter);
            let source_middle = patch.source_parameters[0]
                + parameter * (patch.source_parameters[1] - patch.source_parameters[0]);
            let split_position_error = patch_position_error_at_parameter(
                patch.maximum_position_error,
                patch.maximum_local_derivative_error,
                endpoint_errors[location.part],
                parameter,
            );
            let (mut retained_patches, mut retained_errors, retained_patch, retained_error) =
                match side {
                    CurveOffsetTrimSide::Prefix
                        if location.part > 0 || parameter > parameter_epsilon =>
                    {
                        let mut retained = patches[..=location.part].to_vec();
                        let mut errors = endpoint_errors[..=location.part].to_vec();
                        first[3] = fitted_join;
                        let adjustment = distance_between(
                            split_cubic_controls_at(patch.controls, parameter)[0][3],
                            fitted_join,
                        );
                        let retained_patch = adjusted_trimmed_cubic_patch(
                            patch,
                            first,
                            [patch.source_parameters[0], source_middle],
                            parameter,
                            adjustment,
                            false,
                        )?;
                        let retained_error = [
                            endpoint_errors[location.part][0],
                            next_up(split_position_error + adjustment),
                        ];
                        retained.pop();
                        errors.pop();
                        (retained, errors, retained_patch, retained_error)
                    }
                    CurveOffsetTrimSide::Suffix
                        if location.part + 1 < patches.len()
                            || parameter < 1.0 - parameter_epsilon =>
                    {
                        let mut retained = patches[location.part..].to_vec();
                        let mut errors = endpoint_errors[location.part..].to_vec();
                        second[0] = fitted_join;
                        let adjustment = distance_between(
                            split_cubic_controls_at(patch.controls, parameter)[1][0],
                            fitted_join,
                        );
                        let retained_patch = adjusted_trimmed_cubic_patch(
                            patch,
                            second,
                            [source_middle, patch.source_parameters[1]],
                            1.0 - parameter,
                            adjustment,
                            true,
                        )?;
                        let retained_error = [
                            next_up(split_position_error + adjustment),
                            endpoint_errors[location.part][1],
                        ];
                        retained.remove(0);
                        errors.remove(0);
                        (retained, errors, retained_patch, retained_error)
                    }
                    _ => return Err(curved_miter_failure("degenerate curved miter trim")),
                };
            match side {
                CurveOffsetTrimSide::Prefix => {
                    retained_patches.push(retained_patch);
                    retained_errors.push(retained_error);
                }
                CurveOffsetTrimSide::Suffix => {
                    retained_patches.insert(0, retained_patch);
                    retained_errors.insert(0, retained_error);
                }
            }
            trimmed.certificate.maximum_position_error =
                trimmed.certificate.maximum_position_error.max(
                    retained_patches
                        .iter()
                        .map(|patch| patch.maximum_position_error)
                        .fold(0.0, f64::max),
                );
            trimmed.certificate.maximum_tangent_error_radians =
                trimmed.certificate.maximum_tangent_error_radians.max(
                    retained_patches
                        .iter()
                        .map(|patch| patch.maximum_tangent_error_radians)
                        .fold(0.0, f64::max),
                );
            let source_start = retained_patches
                .first()
                .ok_or_else(|| curved_miter_failure("empty curved miter trim"))?
                .source_parameters[0];
            let source_end = retained_patches
                .last()
                .ok_or_else(|| curved_miter_failure("empty curved miter trim"))?
                .source_parameters[1];
            if !source_start.is_finite() || !source_end.is_finite() {
                return Err(curved_miter_failure("invalid source correspondence"));
            }
            trimmed.source_parameters = Some([source_start, source_end]);
            trimmed.geometry = CurveOffsetGeometry::CubicPatches(retained_patches);
            trimmed.patch_endpoint_position_errors = retained_errors;
        }
        CurveOffsetGeometry::Line { .. } | CurveOffsetGeometry::CircularArc { .. } => {
            return Err(curved_miter_failure("invalid curved miter part"));
        }
    }
    let (start, end, _, _) = curve_offset_terminals(&trimmed.geometry)?;
    if distance_between(start, end) <= tolerance {
        return Err(curved_miter_failure("degenerate curved miter trim"));
    }
    Ok(trimmed)
}

fn certify_trimmed_curve_offset_fit(
    edge: &EvaluatedCurveOffsetEdge,
    options: CurveOffsetOptions,
) -> Result<(), EvaluateFeatureError> {
    let coordinate_scale = match &edge.geometry {
        CurveOffsetGeometry::Line { start, end } => start
            .iter()
            .chain(end)
            .fold(1.0_f64, |scale, value| scale.max(value.abs())),
        CurveOffsetGeometry::CircularArc { center, radius, .. } => center
            .iter()
            .fold(radius.abs().max(1.0), |scale, value| scale.max(value.abs())),
        CurveOffsetGeometry::CubicPatches(patches) => patches
            .iter()
            .flat_map(|patch| patch.controls.iter().flatten())
            .fold(1.0_f64, |scale, value| scale.max(value.abs())),
    };
    let position_limit = options
        .position_tolerance
        .max(256.0 * f64::EPSILON * coordinate_scale);
    let valid = edge.certificate.maximum_position_error.is_finite()
        && edge.certificate.maximum_position_error <= position_limit
        && edge.certificate.maximum_tangent_error_radians.is_finite()
        && edge.certificate.maximum_tangent_error_radians <= options.tangent_tolerance_radians
        && edge.certificate.minimum_regularity_factor.is_finite()
        && edge.certificate.minimum_regularity_factor > options.regularity_margin;
    valid
        .then_some(())
        .ok_or_else(|| curved_miter_failure("curved miter fit tolerance"))
}

fn adjusted_trimmed_cubic_patch(
    original: &geosolve_sketch::CurveOffsetCubicPatch,
    controls: [[f64; 2]; 4],
    source_parameters: [f64; 2],
    retained_fraction: f64,
    endpoint_adjustment: f64,
    adjusted_start: bool,
) -> Result<geosolve_sketch::CurveOffsetCubicPatch, EvaluateFeatureError> {
    if !retained_fraction.is_finite()
        || retained_fraction <= 0.0
        || !endpoint_adjustment.is_finite()
    {
        return Err(curved_miter_failure("invalid curved miter trim"));
    }
    let [old_first, old_second] = split_cubic_controls_at(
        original.controls,
        if adjusted_start {
            1.0 - retained_fraction
        } else {
            retained_fraction
        },
    );
    let old_controls = if adjusted_start {
        old_second
    } else {
        old_first
    };
    let old_tangent = if adjusted_start {
        subtract(old_controls[1], old_controls[0])
    } else {
        subtract(old_controls[3], old_controls[2])
    };
    let new_tangent = if adjusted_start {
        subtract(controls[1], controls[0])
    } else {
        subtract(controls[3], controls[2])
    };
    let tangent_adjustment = angle_between_vectors(old_tangent, new_tangent)?;
    Ok(geosolve_sketch::CurveOffsetCubicPatch {
        source_parameters,
        controls,
        maximum_position_error: next_up(original.maximum_position_error + endpoint_adjustment),
        maximum_local_derivative_error: next_up(
            original.maximum_local_derivative_error * retained_fraction + 3.0 * endpoint_adjustment,
        ),
        maximum_tangent_error_radians: next_up(
            original.maximum_tangent_error_radians + tangent_adjustment,
        ),
    })
}

fn patch_position_error_at_parameter(
    maximum_position_error: f64,
    maximum_local_derivative_error: f64,
    endpoint_errors: [f64; 2],
    parameter: f64,
) -> f64 {
    let from_start = endpoint_errors[0] + maximum_local_derivative_error * parameter;
    let from_end = endpoint_errors[1] + maximum_local_derivative_error * (1.0 - parameter);
    next_up(maximum_position_error.min(from_start).min(from_end))
}

fn angle_between_vectors(first: [f64; 2], second: [f64; 2]) -> Result<f64, EvaluateFeatureError> {
    let first =
        normalized(first).ok_or_else(|| curved_miter_failure("singular trimmed tangent"))?;
    let second =
        normalized(second).ok_or_else(|| curved_miter_failure("singular trimmed tangent"))?;
    let angle = cross(first, second).atan2(dot(first, second)).abs();
    angle
        .is_finite()
        .then_some(angle)
        .ok_or_else(|| curved_miter_failure("singular trimmed tangent"))
}

fn split_cubic_controls_at(controls: [[f64; 2]; 4], parameter: f64) -> [[[f64; 2]; 4]; 2] {
    let first_level = [
        lerp_point(controls[0], controls[1], parameter),
        lerp_point(controls[1], controls[2], parameter),
        lerp_point(controls[2], controls[3], parameter),
    ];
    let second_level = [
        lerp_point(first_level[0], first_level[1], parameter),
        lerp_point(first_level[1], first_level[2], parameter),
    ];
    let middle = lerp_point(second_level[0], second_level[1], parameter);
    [
        [controls[0], first_level[0], second_level[0], middle],
        [middle, second_level[1], first_level[2], controls[3]],
    ]
}

fn cubic_point_and_derivative(controls: [[f64; 2]; 4], parameter: f64) -> ([f64; 2], [f64; 2]) {
    let [first, second] = split_cubic_controls_at(controls, parameter);
    (first[3], scale_vector(subtract(second[1], first[3]), 3.0))
}

fn lerp_point(start: [f64; 2], end: [f64; 2], parameter: f64) -> [f64; 2] {
    [
        (end[0] - start[0]).mul_add(parameter, start[0]),
        (end[1] - start[1]).mul_add(parameter, start[1]),
    ]
}

fn curved_miter_failure(kind: &'static str) -> EvaluateFeatureError {
    EvaluateFeatureError::Failure(ComputedFeatureFailure::OffsetJunctionFailure { kind })
}

fn exact_offset_connector(
    source: NativeCurveSpanSource,
    start: [f64; 2],
    end: [f64; 2],
) -> EvaluatedCurveOffsetEdge {
    EvaluatedCurveOffsetEdge {
        role: GeometryRole::Profile,
        source,
        source_parameters: None,
        geometry: CurveOffsetGeometry::Line { start, end },
        certificate: CurveOffsetCertificate {
            maximum_position_error: 0.0,
            maximum_tangent_error_radians: 0.0,
            minimum_regularity_factor: 1.0,
            subdivision_count: 0,
        },
        patch_endpoint_position_errors: Vec::new(),
    }
}

fn curve_offset_patch_endpoint_errors(geometry: &CurveOffsetGeometry) -> Vec<[f64; 2]> {
    match geometry {
        CurveOffsetGeometry::CubicPatches(patches) => vec![[0.0, 0.0]; patches.len()],
        CurveOffsetGeometry::Line { .. } | CurveOffsetGeometry::CircularArc { .. } => Vec::new(),
    }
}

#[allow(
    clippy::type_complexity,
    reason = "the four vectors are one private start/end position-and-tangent tuple used only by certification"
)]
fn curve_offset_terminals(
    geometry: &CurveOffsetGeometry,
) -> Result<([f64; 2], [f64; 2], [f64; 2], [f64; 2]), EvaluateFeatureError> {
    let invalid = || {
        EvaluateFeatureError::Failure(ComputedFeatureFailure::OffsetJunctionFailure {
            kind: "invalid generated terminal",
        })
    };
    let (start, end, start_tangent, end_tangent) = match geometry {
        CurveOffsetGeometry::Line { start, end } => {
            let tangent = normalized(subtract(*end, *start)).ok_or_else(invalid)?;
            (*start, *end, tangent, tangent)
        }
        CurveOffsetGeometry::CircularArc {
            center,
            radius,
            start_angle,
            sweep,
            ..
        } => {
            let end_angle = start_angle + sweep;
            let sign = sweep.signum();
            (
                [
                    radius.mul_add(start_angle.cos(), center[0]),
                    radius.mul_add(start_angle.sin(), center[1]),
                ],
                [
                    radius.mul_add(end_angle.cos(), center[0]),
                    radius.mul_add(end_angle.sin(), center[1]),
                ],
                [-start_angle.sin() * sign, start_angle.cos() * sign],
                [-end_angle.sin() * sign, end_angle.cos() * sign],
            )
        }
        CurveOffsetGeometry::CubicPatches(patches) => {
            let first = patches.first().ok_or_else(invalid)?;
            let last = patches.last().ok_or_else(invalid)?;
            (
                first.controls[0],
                last.controls[3],
                normalized(subtract(first.controls[1], first.controls[0])).ok_or_else(invalid)?,
                normalized(subtract(last.controls[3], last.controls[2])).ok_or_else(invalid)?,
            )
        }
    };
    Ok((start, end, start_tangent, end_tangent))
}

#[allow(
    clippy::too_many_lines,
    reason = "the temporary-document topology oracle is a single fail-closed certification transaction"
)]
fn certify_fitted_curve_offset_topology(
    paths: &[EvaluatedCurveOffsetPath],
    model_scale: f64,
    face: bool,
    controller: &mut OperationController,
) -> Result<(), EvaluateFeatureError> {
    let model_scale = model_scale.abs().max(f64::MIN_POSITIVE);
    let mut document = SketchDocument::new(model_scale).map_err(|_| offset_topology_change())?;
    let coordinate_scale = paths.iter().flat_map(|path| &path.edges).try_fold(
        model_scale.max(1.0),
        |scale, edge| {
            let (start, end, _, _) = curve_offset_terminals(&edge.geometry)?;
            Ok::<_, EvaluateFeatureError>(
                start
                    .into_iter()
                    .chain(end)
                    .fold(scale, |scale, value| scale.max(value.abs())),
            )
        },
    )?;
    let tolerance = 1.0e-8 * model_scale + 256.0 * f64::EPSILON * coordinate_scale;
    let mut path_spans = Vec::with_capacity(paths.len());

    for (path_index, path) in paths.iter().enumerate() {
        let mut fitted_spans = BTreeSet::new();
        let first_geometry = &path
            .edges
            .first()
            .ok_or_else(offset_topology_change)?
            .geometry;
        if path.closed
            && path.edges.len() == 1
            && matches!(
                first_geometry,
                CurveOffsetGeometry::CircularArc { closed: true, .. }
            )
        {
            fitted_spans.extend(add_fitted_offset_geometry(
                &mut document,
                first_geometry,
                None,
                None,
                path_index,
                0,
                tolerance,
            )?);
            path_spans.push(fitted_spans);
            continue;
        }

        let (first_position, _, _, _) = curve_offset_terminals(first_geometry)?;
        let first_point = document
            .add_point(format!("offset path {path_index} start"), first_position)
            .map_err(|_| offset_topology_change())?;
        let mut current_point = first_point;
        let mut current_position = first_position;
        for (edge_index, edge) in path.edges.iter().enumerate() {
            let (start, end, _, _) = curve_offset_terminals(&edge.geometry)?;
            if distance_between(current_position, start) > tolerance {
                return Err(offset_topology_change());
            }
            let final_edge = edge_index + 1 == path.edges.len();
            let end_point = if final_edge && path.closed {
                if distance_between(end, first_position) > tolerance {
                    return Err(offset_topology_change());
                }
                first_point
            } else {
                document
                    .add_point(
                        format!("offset path {path_index} edge {edge_index} end"),
                        end,
                    )
                    .map_err(|_| offset_topology_change())?
            };
            fitted_spans.extend(add_fitted_offset_geometry(
                &mut document,
                &edge.geometry,
                Some(current_point),
                Some(end_point),
                path_index,
                edge_index,
                tolerance,
            )?);
            current_point = end_point;
            current_position = end;
        }
        path_spans.push(fitted_spans);
    }

    let generated_span_count = path_spans.iter().try_fold(0_usize, |count, spans| {
        count
            .checked_add(spans.len())
            .ok_or_else(offset_topology_change)
    })?;
    // Visual-profile analysis partitions every exact full-period circle into two source pieces at
    // its authenticated seam. Account for those extra pieces before reserving all distinct pairs
    // plus one conservative self-contact candidate per piece. Using only the public span count
    // under-reserved mixed fitted/closed-analytic faces once their cubic patch chain exceeded the
    // default candidate allowance.
    let full_periodic_splits = paths
        .iter()
        .flat_map(|path| &path.edges)
        .filter(|edge| {
            matches!(
                edge.geometry,
                CurveOffsetGeometry::CircularArc { closed: true, .. }
            )
        })
        .count();
    let source_piece_upper = generated_span_count
        .checked_add(full_periodic_splits)
        .ok_or_else(offset_topology_change)?;
    let candidate_pairs = source_piece_upper
        .checked_mul(source_piece_upper.saturating_add(1))
        .and_then(|value| value.checked_div(2))
        .filter(|value| *value <= 2_000_000)
        .ok_or_else(offset_topology_change)?;
    let mut topology_options = VisualProfileOptions::default();
    topology_options.max_candidate_pairs =
        topology_options.max_candidate_pairs.max(candidate_pairs);
    let analysis = match document
        .analyze_visual_profiles_controlled(topology_options, controller.child_control())
    {
        OperationOutcome::Completed { value, report } => {
            controller
                .absorb_child_report(report)
                .map_err(|_| EvaluateFeatureError::Stopped)?;
            value
        }
        OperationOutcome::Cancelled { report } | OperationOutcome::WorkExhausted { report } => {
            let _ = controller.absorb_child_report(report);
            return Err(EvaluateFeatureError::Stopped);
        }
        _ => return Err(EvaluateFeatureError::Stopped),
    };
    let complete = analysis.status == VisualProfileStatus::Complete
        && analysis.issues.is_empty()
        && analysis.intersections.is_empty();
    let expected_shape = if face {
        analysis
            .faces
            .iter()
            .filter(|candidate| fitted_face_matches_paths(candidate, &path_spans))
            .count()
            == 1
    } else {
        analysis.faces.is_empty()
    };
    (complete && expected_shape)
        .then_some(())
        .ok_or_else(offset_topology_change)
}

#[allow(
    clippy::too_many_arguments,
    clippy::too_many_lines,
    reason = "the generated-curve certification adapter retains explicit path/edge provenance"
)]
fn add_fitted_offset_geometry(
    document: &mut SketchDocument,
    geometry: &CurveOffsetGeometry,
    start_point: Option<geosolve_sketch::DesignPointId>,
    end_point: Option<geosolve_sketch::DesignPointId>,
    path_index: usize,
    edge_index: usize,
    tolerance: f64,
) -> Result<Vec<CurveSpan>, EvaluateFeatureError> {
    let label = |part: &str| format!("offset path {path_index} edge {edge_index} {part}");
    let mut spans = Vec::new();
    match geometry {
        CurveOffsetGeometry::Line { start, end } => {
            let start_point = start_point.ok_or_else(offset_topology_change)?;
            let end_point = end_point.ok_or_else(offset_topology_change)?;
            let direction = subtract(*end, *start);
            let length = norm(direction);
            if !length.is_finite() || length <= tolerance {
                return Err(offset_topology_change());
            }
            let curve = document
                .add_curve(
                    label("line"),
                    CurveDefinition::Line {
                        start: start_point,
                        end: end_point,
                        branch_direction: [direction[0] / length, direction[1] / length],
                    },
                )
                .map_err(|_| offset_topology_change())?;
            spans.push(CurveSpan::line(curve));
        }
        CurveOffsetGeometry::CircularArc {
            center,
            radius,
            start_angle,
            sweep,
            closed,
        } => {
            let center_point = document
                .add_point(label("centre"), *center)
                .map_err(|_| offset_topology_change())?;
            let radius_scalar = document
                .add_scalar(
                    label("radius"),
                    *radius,
                    ScalarUnit::Length,
                    ScalarDomain::Positive,
                )
                .map_err(|_| offset_topology_change())?;
            if *closed {
                if start_point.is_some() || end_point.is_some() {
                    return Err(offset_topology_change());
                }
                let curve = document
                    .add_curve(
                        label("circle"),
                        CurveDefinition::Circle {
                            center: center_point,
                            radius: radius_scalar,
                        },
                    )
                    .map_err(|_| offset_topology_change())?;
                spans.push(CurveSpan::line(curve));
            } else {
                let start_point = start_point.ok_or_else(offset_topology_change)?;
                let end_point = end_point.ok_or_else(offset_topology_change)?;
                let start_scalar = document
                    .add_scalar(
                        label("start angle"),
                        *start_angle,
                        ScalarUnit::Angle,
                        ScalarDomain::Finite,
                    )
                    .map_err(|_| offset_topology_change())?;
                let end_scalar = document
                    .add_scalar(
                        label("end angle"),
                        start_angle + sweep,
                        ScalarUnit::Angle,
                        ScalarDomain::Finite,
                    )
                    .map_err(|_| offset_topology_change())?;
                let span = CurveSpan::line(
                    document
                        .add_curve(
                            label("arc"),
                            CurveDefinition::CircularArc {
                                center: center_point,
                                radius: radius_scalar,
                                start_angle: start_scalar,
                                end_angle: end_scalar,
                                sweep: if *sweep > 0.0 {
                                    DocumentArcSweep::CounterClockwise
                                } else {
                                    DocumentArcSweep::Clockwise
                                },
                            },
                        )
                        .map_err(|_| offset_topology_change())?,
                );
                add_fitted_arc_endpoint(document, span, start_point, true, &label)?;
                add_fitted_arc_endpoint(document, span, end_point, false, &label)?;
                spans.push(span);
            }
        }
        CurveOffsetGeometry::CubicPatches(patches) => {
            let first_patch = patches.first().ok_or_else(offset_topology_change)?;
            let last_patch = patches.last().ok_or_else(offset_topology_change)?;
            let start_point = start_point.ok_or_else(offset_topology_change)?;
            let requested_end_point = end_point.ok_or_else(offset_topology_change)?;
            if distance_between(
                document
                    .point(start_point)
                    .ok_or_else(offset_topology_change)?
                    .position,
                first_patch.controls[0],
            ) > tolerance
                || distance_between(
                    document
                        .point(requested_end_point)
                        .ok_or_else(offset_topology_change)?
                        .position,
                    last_patch.controls[3],
                ) > tolerance
            {
                return Err(offset_topology_change());
            }
            for pair in patches.windows(2) {
                if distance_between(pair[0].controls[3], pair[1].controls[0]) > tolerance {
                    return Err(offset_topology_change());
                }
            }

            // A clamped B-spline requires distinct persistent control identities. For a
            // closed fitted loop, retain structural closure with one explicit coincidence
            // rather than repeating the first control identity at the final endpoint.
            let spline_end_point = if start_point == requested_end_point {
                let duplicate = document
                    .add_point(label("closed spline end"), last_patch.controls[3])
                    .map_err(|_| offset_topology_change())?;
                document
                    .add_constraint(
                        label("closed spline join"),
                        DocumentConstraintDefinition::Coincident {
                            first: duplicate,
                            second: requested_end_point,
                        },
                    )
                    .map_err(|_| offset_topology_change())?;
                duplicate
            } else {
                requested_end_point
            };

            let control_count = patches
                .len()
                .checked_mul(3)
                .and_then(|count| count.checked_add(1))
                .ok_or_else(offset_topology_change)?;
            let mut controls = Vec::with_capacity(control_count);
            controls.push(start_point);
            for (patch_index, patch) in patches.iter().enumerate() {
                controls.push(
                    document
                        .add_point(label("first cubic control"), patch.controls[1])
                        .map_err(|_| offset_topology_change())?,
                );
                controls.push(
                    document
                        .add_point(label("second cubic control"), patch.controls[2])
                        .map_err(|_| offset_topology_change())?,
                );
                controls.push(if patch_index + 1 == patches.len() {
                    spline_end_point
                } else {
                    document
                        .add_point(label("patch end"), patch.controls[3])
                        .map_err(|_| offset_topology_change())?
                });
            }

            let patch_count = u32::try_from(patches.len()).map_err(|_| offset_topology_change())?;
            let mut knots = Vec::with_capacity(
                controls
                    .len()
                    .checked_add(4)
                    .ok_or_else(offset_topology_change)?,
            );
            knots.extend(std::iter::repeat_n(0.0, 4));
            for boundary in 1..patch_count {
                knots.extend(std::iter::repeat_n(f64::from(boundary), 3));
            }
            knots.extend(std::iter::repeat_n(f64::from(patch_count), 4));
            let span_ids = (0..patch_count).collect::<Vec<_>>();
            let curve = document
                .add_curve(
                    label("cubic patch chain"),
                    CurveDefinition::BSpline {
                        form: DocumentBSplineForm::Clamped,
                        degree: 3,
                        controls,
                        knots,
                        span_ids: span_ids.clone(),
                        next_span_id: patch_count,
                    },
                )
                .map_err(|_| offset_topology_change())?;
            spans.extend(
                span_ids
                    .into_iter()
                    .map(|segment| CurveSpan { curve, segment }),
            );
        }
    }
    Ok(spans)
}

fn fitted_face_matches_paths(
    face: &geosolve_sketch::VisualProfileFace,
    path_spans: &[BTreeSet<CurveSpan>],
) -> bool {
    if path_spans.is_empty()
        || face.contours.len() != path_spans.len()
        || face.contours[0].orientation != VisualProfileOrientation::CounterClockwise
        || face.contours[0]
            .edges
            .iter()
            .map(|edge| edge.source_span)
            .collect::<BTreeSet<_>>()
            != path_spans[0]
    {
        return false;
    }
    let mut unmatched_holes = path_spans.iter().skip(1).collect::<Vec<_>>();
    for contour in face.contours.iter().skip(1) {
        if contour.orientation != VisualProfileOrientation::Clockwise {
            return false;
        }
        let spans = contour
            .edges
            .iter()
            .map(|edge| edge.source_span)
            .collect::<BTreeSet<_>>();
        let Some(index) = unmatched_holes
            .iter()
            .position(|expected| **expected == spans)
        else {
            return false;
        };
        unmatched_holes.remove(index);
    }
    unmatched_holes.is_empty()
}

fn add_fitted_arc_endpoint(
    document: &mut SketchDocument,
    span: CurveSpan,
    point: geosolve_sketch::DesignPointId,
    start: bool,
    label: &impl Fn(&str) -> String,
) -> Result<(), EvaluateFeatureError> {
    let contact = document
        .add_curve_contact(
            label(if start { "arc start" } else { "arc end" }),
            span,
            if start { 0.0 } else { 1.0 },
            0,
            if start {
                ContactNeighborhood::Start
            } else {
                ContactNeighborhood::End
            },
            None,
        )
        .map_err(|_| offset_topology_change())?;
    document
        .add_constraint(
            label("arc endpoint join"),
            DocumentConstraintDefinition::PointOnCurve { point, contact },
        )
        .map_err(|_| offset_topology_change())?;
    Ok(())
}

fn validate_curve_offset_topology(
    paths: &[EvaluatedCurveOffsetPath],
    model_scale: f64,
    face: bool,
    controller: &mut OperationController,
) -> Result<(), EvaluateFeatureError> {
    if paths.is_empty()
        || (face && paths.iter().any(|path| !path.closed))
        || (!face && (paths.len() != 1 || paths[0].closed))
    {
        return Err(offset_topology_change());
    }
    validate_curve_offset_source_correspondence(paths)?;
    certify_fitted_curve_offset_topology(paths, model_scale, face, controller)?;
    certify_mathematical_curve_offset_topology(paths, model_scale, controller)
}

fn validate_curve_offset_source_correspondence(
    paths: &[EvaluatedCurveOffsetPath],
) -> Result<(), EvaluateFeatureError> {
    for edge in paths.iter().flat_map(|path| &path.edges) {
        match (&edge.geometry, edge.source_parameters) {
            (
                CurveOffsetGeometry::Line { .. } | CurveOffsetGeometry::CircularArc { .. },
                Some([start, end]),
            ) if start.is_finite() && end.is_finite() && start.to_bits() != end.to_bits() => {}
            (CurveOffsetGeometry::Line { .. }, None) => {
                // Junction connectors are rendered and feature-selectable, but have no honest
                // inverse-edit parameterization on either adjacent native source.
            }
            (CurveOffsetGeometry::CubicPatches(patches), Some([start, end]))
                if !patches.is_empty()
                    && start.is_finite()
                    && end.is_finite()
                    && patches[0].source_parameters[0].to_bits() == start.to_bits()
                    && patches.last().is_some_and(|patch| {
                        patch.source_parameters[1].to_bits() == end.to_bits()
                    })
                    && patches.windows(2).all(|pair| {
                        pair[0].source_parameters[1].to_bits()
                            == pair[1].source_parameters[0].to_bits()
                    }) => {}
            _ => return Err(offset_topology_change()),
        }
    }
    Ok(())
}

const OFFSET_TUBE_MAX_DEPTH: usize = 40;
const OFFSET_TUBE_MAX_PAIR_VISITS: usize = 2_000_000;
const OFFSET_TUBE_ARC_CELL_ANGLE: f64 = std::f64::consts::PI / 8.0;

#[derive(Clone, Debug)]
struct OffsetTubeCell {
    path: usize,
    ordinal: usize,
    path_cell_count: usize,
    path_closed: bool,
    geometry: OffsetTubeGeometry,
    error: Option<OffsetTubeError>,
}

#[derive(Clone, Debug)]
enum OffsetTubeGeometry {
    Line {
        start: [f64; 2],
        end: [f64; 2],
    },
    CircularArc {
        center: [f64; 2],
        radius: f64,
        start_angle: f64,
        sweep: f64,
    },
    Cubic {
        controls: [[f64; 2]; 4],
    },
}

#[derive(Clone, Copy, Debug)]
struct OffsetTubeError {
    maximum_position_error: f64,
    maximum_local_derivative_error: f64,
    endpoint_position_errors: [f64; 2],
    patch_interval: [f64; 2],
}

#[derive(Clone, Copy, Debug)]
struct OffsetTubeBounds {
    minimum: [f64; 2],
    maximum: [f64; 2],
}

fn certify_mathematical_curve_offset_topology(
    paths: &[EvaluatedCurveOffsetPath],
    model_scale: f64,
    controller: &mut OperationController,
) -> Result<(), EvaluateFeatureError> {
    let coordinate_scale = paths.iter().flat_map(|path| &path.edges).try_fold(
        model_scale.abs().max(1.0),
        |scale, edge| {
            let (start, end, _, _) = curve_offset_terminals(&edge.geometry)?;
            Ok::<_, EvaluateFeatureError>(
                start
                    .into_iter()
                    .chain(end)
                    .fold(scale, |scale, value| scale.max(value.abs())),
            )
        },
    )?;
    let tolerance = 1.0e-8 * model_scale.abs() + 256.0 * f64::EPSILON * coordinate_scale.max(1.0);
    let mut cells = Vec::new();
    for (path_index, path) in paths.iter().enumerate() {
        let path_start = cells.len();
        for edge in &path.edges {
            append_offset_tube_edge_cells(path_index, path.closed, edge, controller, &mut cells)?;
        }
        let path_cell_count = cells.len().saturating_sub(path_start);
        if path_cell_count == 0 {
            return Err(offset_topology_change());
        }
        for (ordinal, cell) in cells[path_start..].iter_mut().enumerate() {
            cell.ordinal = ordinal;
            cell.path_cell_count = path_cell_count;
        }
        let first = cell_start(&cells[path_start]);
        let last = cell_end(cells.last().ok_or_else(offset_topology_change)?);
        if path.closed != (distance_between(first, last) <= tolerance) {
            return Err(offset_topology_change());
        }
    }

    for cell in &cells {
        certify_offset_tube_cell_monotone(cell)?;
    }
    for (path_index, path) in paths.iter().enumerate() {
        let path_cells = cells
            .iter()
            .filter(|cell| cell.path == path_index)
            .collect::<Vec<_>>();
        for pair in path_cells.windows(2) {
            certify_adjacent_offset_tube_cells(pair[0], pair[1], tolerance)?;
        }
        if path.closed {
            certify_adjacent_offset_tube_cells(
                path_cells.last().ok_or_else(offset_topology_change)?,
                path_cells.first().ok_or_else(offset_topology_change)?,
                tolerance,
            )?;
        }
    }

    // Sweep fitted cells by their already-inflated x enclosures. A disjoint enclosure proves
    // separation of the mathematical parallels because the continuous fit error is included.
    let bounds = cells
        .iter()
        .map(|cell| offset_tube_bounds(cell, tolerance))
        .collect::<Result<Vec<_>, _>>()?;
    let mut order = (0..cells.len()).collect::<Vec<_>>();
    let mut pair_visits = 0_usize;
    order.sort_by(|first, second| bounds[*first].minimum[0].total_cmp(&bounds[*second].minimum[0]));
    for (order_index, first_index) in order.iter().copied().enumerate() {
        for second_index in order.iter().copied().skip(order_index + 1) {
            if bounds[second_index].minimum[0] > bounds[first_index].maximum[0] {
                break;
            }
            if offset_tube_cells_are_adjacent(&cells[first_index], &cells[second_index])
                || bounds[first_index].maximum[1] < bounds[second_index].minimum[1]
                || bounds[second_index].maximum[1] < bounds[first_index].minimum[1]
            {
                continue;
            }
            certify_separated_offset_tubes(
                &cells[first_index],
                &cells[second_index],
                tolerance,
                0,
                controller,
                &mut pair_visits,
            )?;
        }
    }
    Ok(())
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::too_many_lines,
    reason = "finite arc sweeps are explicitly capped before their bounded deterministic cell count is used"
)]
fn append_offset_tube_edge_cells(
    path: usize,
    path_closed: bool,
    edge: &EvaluatedCurveOffsetEdge,
    controller: &mut OperationController,
    cells: &mut Vec<OffsetTubeCell>,
) -> Result<(), EvaluateFeatureError> {
    match &edge.geometry {
        CurveOffsetGeometry::Line { start, end } => {
            if edge.certificate.maximum_position_error != 0.0
                || edge.certificate.maximum_tangent_error_radians != 0.0
            {
                return Err(offset_topology_change());
            }
            append_monotone_offset_tube_cell(
                OffsetTubeCell {
                    path,
                    ordinal: 0,
                    path_cell_count: 0,
                    path_closed,
                    geometry: OffsetTubeGeometry::Line {
                        start: *start,
                        end: *end,
                    },
                    error: None,
                },
                0,
                controller,
                cells,
            )
        }
        CurveOffsetGeometry::CircularArc {
            center,
            radius,
            start_angle,
            sweep,
            ..
        } => {
            if edge.certificate.maximum_position_error != 0.0
                || edge.certificate.maximum_tangent_error_radians != 0.0
                || !radius.is_finite()
                || *radius <= 0.0
                || !start_angle.is_finite()
                || !sweep.is_finite()
                || *sweep == 0.0
            {
                return Err(offset_topology_change());
            }
            let cell_count = ((sweep.abs() / OFFSET_TUBE_ARC_CELL_ANGLE).ceil() as usize).max(1);
            if cell_count > 4_096 {
                return Err(offset_topology_change());
            }
            for index in 0..cell_count {
                let cell_start = sweep.mul_add(index as f64 / cell_count as f64, *start_angle);
                append_monotone_offset_tube_cell(
                    OffsetTubeCell {
                        path,
                        ordinal: 0,
                        path_cell_count: 0,
                        path_closed,
                        geometry: OffsetTubeGeometry::CircularArc {
                            center: *center,
                            radius: *radius,
                            start_angle: cell_start,
                            sweep: *sweep / cell_count as f64,
                        },
                        error: None,
                    },
                    0,
                    controller,
                    cells,
                )?;
            }
            Ok(())
        }
        CurveOffsetGeometry::CubicPatches(patches) => {
            if patches.is_empty() || edge.patch_endpoint_position_errors.len() != patches.len() {
                return Err(offset_topology_change());
            }
            for (patch, endpoint_position_errors) in
                patches.iter().zip(&edge.patch_endpoint_position_errors)
            {
                let finite = patch
                    .controls
                    .iter()
                    .flatten()
                    .all(|value| value.is_finite())
                    && patch.maximum_position_error.is_finite()
                    && patch.maximum_position_error >= 0.0
                    && patch.maximum_local_derivative_error.is_finite()
                    && patch.maximum_local_derivative_error >= 0.0
                    && patch.maximum_tangent_error_radians.is_finite()
                    && patch.maximum_tangent_error_radians >= 0.0
                    && endpoint_position_errors
                        .iter()
                        .all(|error| error.is_finite() && *error >= 0.0);
                if !finite
                    || patch.maximum_position_error > edge.certificate.maximum_position_error
                    || patch.maximum_tangent_error_radians
                        > edge.certificate.maximum_tangent_error_radians
                {
                    return Err(offset_topology_change());
                }
                append_monotone_offset_tube_cell(
                    OffsetTubeCell {
                        path,
                        ordinal: 0,
                        path_cell_count: 0,
                        path_closed,
                        geometry: OffsetTubeGeometry::Cubic {
                            controls: patch.controls,
                        },
                        error: Some(OffsetTubeError {
                            maximum_position_error: patch.maximum_position_error,
                            maximum_local_derivative_error: patch.maximum_local_derivative_error,
                            endpoint_position_errors: *endpoint_position_errors,
                            patch_interval: [0.0, 1.0],
                        }),
                    },
                    0,
                    controller,
                    cells,
                )?;
            }
            Ok(())
        }
    }
}

fn append_monotone_offset_tube_cell(
    cell: OffsetTubeCell,
    depth: usize,
    controller: &mut OperationController,
    cells: &mut Vec<OffsetTubeCell>,
) -> Result<(), EvaluateFeatureError> {
    if certify_offset_tube_cell_monotone(&cell).is_ok() {
        cells.push(cell);
        return Ok(());
    }
    if depth >= OFFSET_TUBE_MAX_DEPTH {
        return Err(offset_topology_change());
    }
    controller
        .charge(
            OperationWorkCounter::ProfileSubdivisions,
            1,
            OperationCheckpoint::ProfileSubdivision,
        )
        .map_err(|_| EvaluateFeatureError::Stopped)?;
    let [first, second] = split_offset_tube_cell(&cell);
    append_monotone_offset_tube_cell(first, depth + 1, controller, cells)?;
    append_monotone_offset_tube_cell(second, depth + 1, controller, cells)
}

fn certify_offset_tube_cell_monotone(cell: &OffsetTubeCell) -> Result<(), EvaluateFeatureError> {
    let axis = normalized(cell_middle_tangent(cell)).ok_or_else(offset_topology_change)?;
    certify_offset_tube_projection(cell, axis)
}

fn certify_adjacent_offset_tube_cells(
    first: &OffsetTubeCell,
    second: &OffsetTubeCell,
    tolerance: f64,
) -> Result<(), EvaluateFeatureError> {
    if distance_between(cell_end(first), cell_start(second)) > tolerance {
        return Err(offset_topology_change());
    }
    let first_tangent = normalized(cell_end_tangent(first)).ok_or_else(offset_topology_change)?;
    let second_tangent =
        normalized(cell_start_tangent(second)).ok_or_else(offset_topology_change)?;
    let axis = normalized(add(first_tangent, second_tangent)).ok_or_else(offset_topology_change)?;
    certify_offset_tube_projection(first, axis)?;
    certify_offset_tube_projection(second, axis)
}

fn certify_offset_tube_projection(
    cell: &OffsetTubeCell,
    axis: [f64; 2],
) -> Result<(), EvaluateFeatureError> {
    let lower = fitted_derivative_projection_lower(cell, axis)?;
    let derivative_error = cell_derivative_error(cell);
    let upper = fitted_derivative_norm_upper(cell)?;
    let numerical_margin = 4_096.0 * f64::EPSILON * upper.max(f64::MIN_POSITIVE);
    (lower - derivative_error > numerical_margin)
        .then_some(())
        .ok_or_else(offset_topology_change)
}

fn offset_tube_cells_are_adjacent(first: &OffsetTubeCell, second: &OffsetTubeCell) -> bool {
    first.path == second.path
        && (first.ordinal.abs_diff(second.ordinal) == 1
            || (first.path_closed
                && ((first.ordinal == 0 && second.ordinal + 1 == second.path_cell_count)
                    || (second.ordinal == 0 && first.ordinal + 1 == first.path_cell_count))))
}

fn certify_separated_offset_tubes(
    first: &OffsetTubeCell,
    second: &OffsetTubeCell,
    tolerance: f64,
    depth: usize,
    controller: &mut OperationController,
    pair_visits: &mut usize,
) -> Result<(), EvaluateFeatureError> {
    *pair_visits = pair_visits
        .checked_add(1)
        .filter(|visits| *visits <= OFFSET_TUBE_MAX_PAIR_VISITS)
        .ok_or_else(offset_topology_change)?;
    controller
        .charge(
            OperationWorkCounter::ProfileCandidatePairs,
            1,
            OperationCheckpoint::ProfileCandidate,
        )
        .map_err(|_| EvaluateFeatureError::Stopped)?;
    let first_bounds = offset_tube_bounds(first, tolerance)?;
    let second_bounds = offset_tube_bounds(second, tolerance)?;
    if offset_tube_bounds_are_separated(first_bounds, second_bounds) {
        return Ok(());
    }
    if depth >= OFFSET_TUBE_MAX_DEPTH {
        return Err(offset_topology_change());
    }
    controller
        .charge(
            OperationWorkCounter::ProfileSubdivisions,
            1,
            OperationCheckpoint::ProfileSubdivision,
        )
        .map_err(|_| EvaluateFeatureError::Stopped)?;
    let first_size = offset_tube_bounds_size(first_bounds);
    let second_size = offset_tube_bounds_size(second_bounds);
    if first_size >= second_size {
        for child in split_offset_tube_cell(first) {
            certify_separated_offset_tubes(
                &child,
                second,
                tolerance,
                depth + 1,
                controller,
                pair_visits,
            )?;
        }
    } else {
        for child in split_offset_tube_cell(second) {
            certify_separated_offset_tubes(
                first,
                &child,
                tolerance,
                depth + 1,
                controller,
                pair_visits,
            )?;
        }
    }
    Ok(())
}

fn offset_tube_bounds(
    cell: &OffsetTubeCell,
    tolerance: f64,
) -> Result<OffsetTubeBounds, EvaluateFeatureError> {
    let (mut minimum, mut maximum) = match &cell.geometry {
        OffsetTubeGeometry::Line { start, end } => (
            [start[0].min(end[0]), start[1].min(end[1])],
            [start[0].max(end[0]), start[1].max(end[1])],
        ),
        OffsetTubeGeometry::Cubic { controls } => {
            let mut minimum = [f64::INFINITY; 2];
            let mut maximum = [f64::NEG_INFINITY; 2];
            for control in controls {
                for axis in 0..2 {
                    minimum[axis] = minimum[axis].min(control[axis]);
                    maximum[axis] = maximum[axis].max(control[axis]);
                }
            }
            (minimum, maximum)
        }
        OffsetTubeGeometry::CircularArc {
            center,
            radius,
            start_angle,
            sweep,
        } => arc_bounds(*center, *radius, *start_angle, *sweep)?,
    };
    let inflation = next_up(cell_position_error(cell) + tolerance);
    if !inflation.is_finite() || !minimum.into_iter().chain(maximum).all(f64::is_finite) {
        return Err(offset_topology_change());
    }
    for axis in 0..2 {
        minimum[axis] = next_down(minimum[axis] - inflation);
        maximum[axis] = next_up(maximum[axis] + inflation);
    }
    Ok(OffsetTubeBounds { minimum, maximum })
}

fn arc_bounds(
    center: [f64; 2],
    radius: f64,
    start_angle: f64,
    sweep: f64,
) -> Result<([f64; 2], [f64; 2]), EvaluateFeatureError> {
    let mut angles = vec![start_angle, start_angle + sweep];
    let lower = start_angle.min(start_angle + sweep);
    let upper = start_angle.max(start_angle + sweep);
    for quarter in -8..=8 {
        let angle = f64::from(quarter) * std::f64::consts::FRAC_PI_2;
        if angle > lower && angle < upper {
            angles.push(angle);
        }
    }
    let mut minimum = [f64::INFINITY; 2];
    let mut maximum = [f64::NEG_INFINITY; 2];
    for angle in angles {
        let point = [
            radius.mul_add(angle.cos(), center[0]),
            radius.mul_add(angle.sin(), center[1]),
        ];
        for axis in 0..2 {
            minimum[axis] = minimum[axis].min(point[axis]);
            maximum[axis] = maximum[axis].max(point[axis]);
        }
    }
    minimum
        .into_iter()
        .chain(maximum)
        .all(f64::is_finite)
        .then_some((minimum, maximum))
        .ok_or_else(offset_topology_change)
}

fn offset_tube_bounds_are_separated(first: OffsetTubeBounds, second: OffsetTubeBounds) -> bool {
    first.maximum[0] < second.minimum[0]
        || second.maximum[0] < first.minimum[0]
        || first.maximum[1] < second.minimum[1]
        || second.maximum[1] < first.minimum[1]
}

fn offset_tube_bounds_size(bounds: OffsetTubeBounds) -> f64 {
    (bounds.maximum[0] - bounds.minimum[0]).hypot(bounds.maximum[1] - bounds.minimum[1])
}

fn split_offset_tube_cell(cell: &OffsetTubeCell) -> [OffsetTubeCell; 2] {
    let (first_geometry, second_geometry) = match &cell.geometry {
        OffsetTubeGeometry::Line { start, end } => {
            let middle = scale_vector(add(*start, *end), 0.5);
            (
                OffsetTubeGeometry::Line {
                    start: *start,
                    end: middle,
                },
                OffsetTubeGeometry::Line {
                    start: middle,
                    end: *end,
                },
            )
        }
        OffsetTubeGeometry::CircularArc {
            center,
            radius,
            start_angle,
            sweep,
        } => (
            OffsetTubeGeometry::CircularArc {
                center: *center,
                radius: *radius,
                start_angle: *start_angle,
                sweep: *sweep * 0.5,
            },
            OffsetTubeGeometry::CircularArc {
                center: *center,
                radius: *radius,
                start_angle: sweep.mul_add(0.5, *start_angle),
                sweep: *sweep * 0.5,
            },
        ),
        OffsetTubeGeometry::Cubic { controls } => {
            let [first, second] = split_cubic_controls(*controls);
            (
                OffsetTubeGeometry::Cubic { controls: first },
                OffsetTubeGeometry::Cubic { controls: second },
            )
        }
    };
    let (first_error, second_error) = if let Some(error) = cell.error {
        let middle = 0.5 * (error.patch_interval[0] + error.patch_interval[1]);
        (
            Some(OffsetTubeError {
                patch_interval: [error.patch_interval[0], middle],
                ..error
            }),
            Some(OffsetTubeError {
                patch_interval: [middle, error.patch_interval[1]],
                ..error
            }),
        )
    } else {
        (None, None)
    };
    [
        OffsetTubeCell {
            geometry: first_geometry,
            error: first_error,
            ..cell.clone()
        },
        OffsetTubeCell {
            geometry: second_geometry,
            error: second_error,
            ..cell.clone()
        },
    ]
}

fn split_cubic_controls(controls: [[f64; 2]; 4]) -> [[[f64; 2]; 4]; 2] {
    let first_level = [
        scale_vector(add(controls[0], controls[1]), 0.5),
        scale_vector(add(controls[1], controls[2]), 0.5),
        scale_vector(add(controls[2], controls[3]), 0.5),
    ];
    let second_level = [
        scale_vector(add(first_level[0], first_level[1]), 0.5),
        scale_vector(add(first_level[1], first_level[2]), 0.5),
    ];
    let middle = scale_vector(add(second_level[0], second_level[1]), 0.5);
    [
        [controls[0], first_level[0], second_level[0], middle],
        [middle, second_level[1], first_level[2], controls[3]],
    ]
}

fn cell_position_error(cell: &OffsetTubeCell) -> f64 {
    let Some(error) = cell.error else {
        return 0.0;
    };
    let from_start = next_up(
        error.endpoint_position_errors[0]
            + error.maximum_local_derivative_error * error.patch_interval[1].max(0.0),
    );
    let from_end = next_up(
        error.endpoint_position_errors[1]
            + error.maximum_local_derivative_error * (1.0 - error.patch_interval[0]).max(0.0),
    );
    error.maximum_position_error.min(from_start).min(from_end)
}

fn cell_derivative_error(cell: &OffsetTubeCell) -> f64 {
    let Some(error) = cell.error else {
        return 0.0;
    };
    next_up(
        error.maximum_local_derivative_error
            * (error.patch_interval[1] - error.patch_interval[0]).abs(),
    )
}

fn cell_start(cell: &OffsetTubeCell) -> [f64; 2] {
    match &cell.geometry {
        OffsetTubeGeometry::Line { start, .. } => *start,
        OffsetTubeGeometry::CircularArc {
            center,
            radius,
            start_angle,
            ..
        } => [
            radius.mul_add(start_angle.cos(), center[0]),
            radius.mul_add(start_angle.sin(), center[1]),
        ],
        OffsetTubeGeometry::Cubic { controls } => controls[0],
    }
}

fn cell_end(cell: &OffsetTubeCell) -> [f64; 2] {
    match &cell.geometry {
        OffsetTubeGeometry::Line { end, .. } => *end,
        OffsetTubeGeometry::CircularArc {
            center,
            radius,
            start_angle,
            sweep,
        } => {
            let angle = start_angle + sweep;
            [
                radius.mul_add(angle.cos(), center[0]),
                radius.mul_add(angle.sin(), center[1]),
            ]
        }
        OffsetTubeGeometry::Cubic { controls } => controls[3],
    }
}

fn cell_start_tangent(cell: &OffsetTubeCell) -> [f64; 2] {
    match &cell.geometry {
        OffsetTubeGeometry::Line { start, end } => subtract(*end, *start),
        OffsetTubeGeometry::CircularArc {
            radius,
            start_angle,
            sweep,
            ..
        } => scale_vector([-start_angle.sin(), start_angle.cos()], radius * sweep),
        OffsetTubeGeometry::Cubic { controls } => {
            scale_vector(subtract(controls[1], controls[0]), 3.0)
        }
    }
}

fn cell_end_tangent(cell: &OffsetTubeCell) -> [f64; 2] {
    match &cell.geometry {
        OffsetTubeGeometry::Line { start, end } => subtract(*end, *start),
        OffsetTubeGeometry::CircularArc {
            radius,
            start_angle,
            sweep,
            ..
        } => {
            let angle = start_angle + sweep;
            scale_vector([-angle.sin(), angle.cos()], radius * sweep)
        }
        OffsetTubeGeometry::Cubic { controls } => {
            scale_vector(subtract(controls[3], controls[2]), 3.0)
        }
    }
}

fn cell_middle_tangent(cell: &OffsetTubeCell) -> [f64; 2] {
    match &cell.geometry {
        OffsetTubeGeometry::Line { start, end } => subtract(*end, *start),
        OffsetTubeGeometry::CircularArc {
            radius,
            start_angle,
            sweep,
            ..
        } => {
            let angle = sweep.mul_add(0.5, *start_angle);
            scale_vector([-angle.sin(), angle.cos()], radius * sweep)
        }
        OffsetTubeGeometry::Cubic { controls } => {
            let derivatives = cubic_derivative_controls(*controls);
            add(
                scale_vector(add(derivatives[0], derivatives[2]), 0.25),
                scale_vector(derivatives[1], 0.5),
            )
        }
    }
}

fn cubic_derivative_controls(controls: [[f64; 2]; 4]) -> [[f64; 2]; 3] {
    [
        scale_vector(subtract(controls[1], controls[0]), 3.0),
        scale_vector(subtract(controls[2], controls[1]), 3.0),
        scale_vector(subtract(controls[3], controls[2]), 3.0),
    ]
}

fn fitted_derivative_projection_lower(
    cell: &OffsetTubeCell,
    axis: [f64; 2],
) -> Result<f64, EvaluateFeatureError> {
    let lower = match &cell.geometry {
        OffsetTubeGeometry::Line { start, end } => dot(subtract(*end, *start), axis),
        OffsetTubeGeometry::Cubic { controls } => cubic_derivative_controls(*controls)
            .into_iter()
            .map(|derivative| dot(derivative, axis))
            .fold(f64::INFINITY, f64::min),
        OffsetTubeGeometry::CircularArc {
            radius,
            start_angle,
            sweep,
            ..
        } => arc_derivative_projection_lower(*radius, *start_angle, *sweep, axis),
    };
    lower
        .is_finite()
        .then(|| next_down(lower))
        .ok_or_else(offset_topology_change)
}

fn arc_derivative_projection_lower(
    radius: f64,
    start_angle: f64,
    sweep: f64,
    axis: [f64; 2],
) -> f64 {
    let end_angle = start_angle + sweep;
    let lower = start_angle.min(end_angle);
    let upper = start_angle.max(end_angle);
    let value = |angle: f64| radius * sweep * (-axis[0] * angle.sin() + axis[1] * angle.cos());
    let mut result = value(start_angle).min(value(end_angle));
    let phase = axis[1].atan2(axis[0]);
    for half_turn in -8..=8 {
        let angle =
            phase + std::f64::consts::FRAC_PI_2 + f64::from(half_turn) * std::f64::consts::PI;
        if angle > lower && angle < upper {
            result = result.min(value(angle));
        }
    }
    result
}

fn fitted_derivative_norm_upper(cell: &OffsetTubeCell) -> Result<f64, EvaluateFeatureError> {
    let upper = match &cell.geometry {
        OffsetTubeGeometry::Line { start, end } => norm(subtract(*end, *start)),
        OffsetTubeGeometry::CircularArc { radius, sweep, .. } => radius * sweep.abs(),
        OffsetTubeGeometry::Cubic { controls } => cubic_derivative_controls(*controls)
            .into_iter()
            .map(norm)
            .fold(0.0, f64::max),
    };
    (upper.is_finite() && upper > 0.0)
        .then(|| next_up(upper))
        .ok_or_else(offset_topology_change)
}

fn next_up(value: f64) -> f64 {
    if value.is_nan() || value == f64::INFINITY {
        return value;
    }
    if value == -0.0 {
        return f64::from_bits(1);
    }
    if value >= 0.0 {
        f64::from_bits(value.to_bits() + 1)
    } else {
        f64::from_bits(value.to_bits() - 1)
    }
}

fn next_down(value: f64) -> f64 {
    if value.is_nan() || value == f64::NEG_INFINITY {
        return value;
    }
    if value == 0.0 {
        return -f64::from_bits(1);
    }
    if value > 0.0 {
        f64::from_bits(value.to_bits() - 1)
    } else {
        f64::from_bits(value.to_bits() + 1)
    }
}

fn offset_topology_change() -> EvaluateFeatureError {
    EvaluateFeatureError::Failure(ComputedFeatureFailure::OffsetTopologyChange)
}

fn normalized(value: [f64; 2]) -> Option<[f64; 2]> {
    let length = norm(value);
    (length.is_finite() && length > 0.0).then(|| scale_vector(value, length.recip()))
}

fn add(first: [f64; 2], second: [f64; 2]) -> [f64; 2] {
    [first[0] + second[0], first[1] + second[1]]
}

fn subtract(first: [f64; 2], second: [f64; 2]) -> [f64; 2] {
    [first[0] - second[0], first[1] - second[1]]
}

fn scale_vector(value: [f64; 2], factor: f64) -> [f64; 2] {
    [value[0] * factor, value[1] * factor]
}

fn dot(first: [f64; 2], second: [f64; 2]) -> f64 {
    first[0].mul_add(second[0], first[1] * second[1])
}

fn cross(first: [f64; 2], second: [f64; 2]) -> f64 {
    first[0].mul_add(second[1], -first[1] * second[0])
}

fn norm(value: [f64; 2]) -> f64 {
    value[0].hypot(value[1])
}

fn distance_between(first: [f64; 2], second: [f64; 2]) -> f64 {
    norm(subtract(first, second))
}

fn evaluate_feature(
    sketch: &SketchDocument,
    feature: &ComputedFeature,
    continuation_hints: &[ComputedFilletContinuation],
    policy: ComputedFeatureEvaluationPolicy,
    controller: &mut OperationController,
) -> Result<EvaluatedFeatureCandidate, EvaluateFeatureError> {
    let ComputedFeatureDefinition::FilletSet(fillet) = &feature.definition else {
        return Err(EvaluateFeatureError::Failure(
            ComputedFeatureFailure::OffsetJunctionFailure {
                kind: "feature dispatch",
            },
        ));
    };
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
        let continuation = continuation_hints
            .iter()
            .find(|continuation| continuation.owner == owner);
        let evaluated = evaluate_persistent_corner(
            sketch,
            owner,
            *corner,
            fillet.radius,
            continuation,
            policy,
            controller,
        )?;
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
    continuation: Option<&ComputedFilletContinuation>,
    policy: ComputedFeatureEvaluationPolicy,
    controller: &mut OperationController,
) -> Result<EvaluatedCorner, EvaluateFeatureError> {
    let parents = [corner.first, corner.second];
    let prepared_root_parents = prepare_root_parents(sketch, parents, Some(corner.id))
        .map_err(EvaluateFeatureError::Failure)?;
    let affine = parents.map(|parent| is_affine_line_span(sketch, parent.source.span));
    if affine == [false, false] {
        return Err(EvaluateFeatureError::Failure(
            ComputedFeatureFailure::UnsupportedCurvedPair { corner: corner.id },
        ));
    }
    let certified = certify_persistent_branch(sketch, prepared_root_parents, affine, corner.id);
    let sides = [corner.first.normal_side, corner.second.normal_side];
    let root_input = PersistentCornerRootInput {
        corner: corner.id,
        persistent: corner.without_id(),
        prepared: prepared_root_parents,
        certified,
        affine,
        sides,
        radius,
        policy,
        continuation,
    };
    let resolved = resolve_persistent_corner_root(sketch, &root_input, controller)?;
    let arc = build_and_validate_arc(
        sketch,
        parents,
        resolved.solution,
        radius,
        corner.endpoint_order,
        corner.sweep,
        resolved.branch_validation,
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
        endpoint_claim(
            owner,
            corner.first,
            resolved.solution.parameters[0],
            resolved.root_parents[0].topology,
        ),
        endpoint_claim(
            owner,
            corner.second,
            resolved.solution.parameters[1],
            resolved.root_parents[1].topology,
        ),
    ];
    let continuation =
        build_source_fillet_continuation(sketch, owner, corner, radius, affine, resolved.solution)?;
    Ok(EvaluatedCorner {
        owner,
        role: combined_source_role(sketch, parents),
        arc,
        claims,
        continuation,
    })
}

fn build_source_fillet_continuation(
    sketch: &SketchDocument,
    owner: ComputedCornerRef,
    corner: ComputedFilletCorner,
    radius: f64,
    affine: [bool; 2],
    solution: LocalFilletSolution,
) -> Result<ComputedFilletContinuation, EvaluateFeatureError> {
    let continued_parents =
        build_source_continued_parents(sketch, [corner.first, corner.second], affine, solution)
            .map_err(|_| {
                EvaluateFeatureError::Failure(ComputedFeatureFailure::UncertifiedBranch {
                    corner: corner.id,
                })
            })?;
    let continued_corner = NewComputedFilletCorner {
        first: continued_parents[0],
        second: continued_parents[1],
        endpoint_order: corner.endpoint_order,
        sweep: corner.sweep,
    }
    .canonicalized();
    let offset_tangent_directions = fillet_offset_tangent_directions(
        sketch,
        [continued_corner.first, continued_corner.second],
        solution,
        radius,
    )
    .ok_or(EvaluateFeatureError::Failure(
        ComputedFeatureFailure::SingularParents { corner: corner.id },
    ))?;
    let transverse_orientation = fillet_transverse_orientation_for_derivatives(
        offset_tangent_directions[0],
        offset_tangent_directions[1],
    )
    .ok_or(EvaluateFeatureError::Failure(
        ComputedFeatureFailure::SingularParents { corner: corner.id },
    ))?;
    Ok(ComputedFilletContinuation {
        owner,
        radius,
        corner: continued_corner,
        transverse_orientation,
        offset_tangent_directions,
    })
}

#[derive(Clone)]
struct PersistentCornerRootInput<'a> {
    corner: ComputedFeatureCornerId,
    persistent: NewComputedFilletCorner,
    prepared: [RootParent; 2],
    certified: Result<[RootParent; 2], ComputedFeatureFailure>,
    affine: [bool; 2],
    sides: [DocumentCurveNormalSide; 2],
    radius: f64,
    policy: ComputedFeatureEvaluationPolicy,
    continuation: Option<&'a ComputedFilletContinuation>,
}

struct PersistentCornerRootResolution<'a> {
    root_parents: [RootParent; 2],
    solution: LocalFilletSolution,
    branch_validation: ArcBranchValidation<'a>,
}

#[allow(
    clippy::too_many_lines,
    reason = "the persisted fast path, typed failure routing and narrowly gated transport fallback remain one auditable branch-selection transaction"
)]
fn resolve_persistent_corner_root<'a>(
    sketch: &SketchDocument,
    input: &'a PersistentCornerRootInput<'a>,
    controller: &mut OperationController,
) -> Result<PersistentCornerRootResolution<'a>, EvaluateFeatureError> {
    let matching_continuation = input
        .continuation
        .filter(|continuation| continuation_matches_persistent_corner(continuation, input));
    let matches_continuation = |solution: LocalFilletSolution| {
        matching_continuation.is_none_or(|continuation| {
            continuation_accepts_solution(sketch, input, continuation, solution)
        })
    };
    let mut ordinary_failure = None;
    if let Ok(certified) = &input.certified {
        let exact = exact_seed_solution(sketch, certified, input.sides, input.radius).map_err(
            |failure| {
                EvaluateFeatureError::Failure(match failure {
                    OffsetGeometryFailure::OffsetSingularity => {
                        ComputedFeatureFailure::OffsetSingularity {
                            corner: input.corner,
                        }
                    }
                    OffsetGeometryFailure::Invalid => ComputedFeatureFailure::InvalidParentState {
                        corner: input.corner,
                    },
                })
            },
        )?;
        if let Some(solution) = exact.filter(|solution| matches_continuation(*solution)) {
            return Ok(PersistentCornerRootResolution {
                root_parents: *certified,
                solution,
                branch_validation: ArcBranchValidation::PersistedCell,
            });
        }
        // A persisted circular parent has constant offset regularity, so its
        // complete certified tangent-orientation cell contains at most one
        // transverse line-offset root. Its picked parameter is therefore only
        // a search seed after a source edit, not another hidden branch bound.
        // General curved parents retain the tighter seed-connected guard
        // because an offset cusp can introduce a disconnected remote root
        // inside one tangent-orientation cell.
        let root_parents = persistent_evaluation_root_parents(sketch, certified, input.affine)
            .ok_or({
                EvaluateFeatureError::Failure(ComputedFeatureFailure::InvalidParentState {
                    corner: input.corner,
                })
            })?;
        let (solutions, root_failure) = match local_fillet_roots(
            sketch,
            &root_parents,
            input.sides,
            input.radius,
            input.policy,
            controller,
        ) {
            RootSearchResult::Completed { solutions, failure } => (solutions, failure),
            RootSearchResult::Stopped => return Err(EvaluateFeatureError::Stopped),
        };
        let solutions = solutions
            .into_iter()
            .filter(|solution| matches_continuation(*solution))
            .collect::<Vec<_>>();
        match select_seed_connected_solution(sketch, &root_parents, &solutions) {
            Ok(solution) => {
                return Ok(PersistentCornerRootResolution {
                    root_parents,
                    solution,
                    branch_validation: ArcBranchValidation::PersistedCell,
                });
            }
            Err(RootSelectionFailure::None)
                if matching_continuation.is_some()
                    || root_failure == RootSearchFailure::NoLocalRoot =>
            {
                ordinary_failure = Some(ComputedFeatureFailure::NoLocalRoot {
                    corner: input.corner,
                });
            }
            Err(kind) => {
                return Err(EvaluateFeatureError::Failure(map_persistent_root_failure(
                    input.corner,
                    kind,
                    root_failure,
                )));
            }
        }
    } else if let Err(failure) = &input.certified {
        if !matches!(failure, ComputedFeatureFailure::UncertifiedBranch { .. }) {
            return Err(EvaluateFeatureError::Failure(failure.clone()));
        }
        ordinary_failure = Some(failure.clone());
    }

    let proof = matching_continuation
        .map(CircularAffineBranchProof::AcceptedContinuation)
        .or_else(|| {
            matches!(
                ordinary_failure,
                Some(ComputedFeatureFailure::NoLocalRoot { .. })
            )
            .then_some(CircularAffineBranchProof::PersistedCellOverlap)
        })
        .ok_or_else(|| {
            EvaluateFeatureError::Failure(ordinary_failure.unwrap_or(
                ComputedFeatureFailure::NoLocalRoot {
                    corner: input.corner,
                },
            ))
        })?;
    resolve_transported_circular_affine_root(sketch, input, proof, controller)
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum CircularAffineBranchProof<'a> {
    PersistedCellOverlap,
    AcceptedContinuation(&'a ComputedFilletContinuation),
}

fn continuation_matches_persistent_corner(
    continuation: &ComputedFilletContinuation,
    input: &PersistentCornerRootInput<'_>,
) -> bool {
    let current = input.persistent;
    let previous = continuation.corner;
    continuation.radius.to_bits() == input.radius.to_bits()
        && previous.first.source == current.first.source
        && previous.second.source == current.second.source
        && previous.first.normal_side == current.first.normal_side
        && previous.second.normal_side == current.second.normal_side
        && previous.first.retained_endpoint == current.first.retained_endpoint
        && previous.second.retained_endpoint == current.second.retained_endpoint
        && previous.endpoint_order == current.endpoint_order
        && previous.sweep == current.sweep
}

fn continuation_matches_feature_document(
    continuation: &ComputedFilletContinuation,
    features: &ComputedFeatureDocument,
) -> bool {
    let Some(feature) = features.feature(continuation.owner.feature) else {
        return false;
    };
    let ComputedFeatureDefinition::FilletSet(fillet) = &feature.definition else {
        return false;
    };
    let Some(corner) = fillet
        .corners
        .iter()
        .find(|corner| corner.id == continuation.owner.corner)
    else {
        return false;
    };
    let current = corner.without_id();
    let previous = continuation.corner;
    !feature.suppressed
        && continuation.radius.to_bits() == fillet.radius.to_bits()
        && previous.first.source == current.first.source
        && previous.second.source == current.second.source
        && previous.first.normal_side == current.first.normal_side
        && previous.second.normal_side == current.second.normal_side
        && previous.first.retained_endpoint == current.first.retained_endpoint
        && previous.second.retained_endpoint == current.second.retained_endpoint
        && previous.endpoint_order == current.endpoint_order
        && previous.sweep == current.sweep
}

fn continuation_accepts_solution(
    sketch: &SketchDocument,
    input: &PersistentCornerRootInput<'_>,
    continuation: &ComputedFilletContinuation,
    solution: LocalFilletSolution,
) -> bool {
    if !continuation_cell_contains_solution(continuation, input.affine, solution) {
        return false;
    }
    let Some(directions) = fillet_offset_tangent_directions(
        sketch,
        input.prepared.map(|parent| parent.parent),
        solution,
        input.radius,
    ) else {
        return false;
    };
    fillet_transverse_orientation_for_derivatives(directions[0], directions[1])
        == Some(continuation.transverse_orientation)
        && directions
            .into_iter()
            .zip(continuation.offset_tangent_directions)
            .all(|(current, previous)| {
                current[0].mul_add(previous[0], current[1] * previous[1])
                    > CONTINUATION_MIN_TANGENT_DIRECTION_DOT
            })
}

fn continuation_cell_contains_solution(
    continuation: &ComputedFilletContinuation,
    affine: [bool; 2],
    solution: LocalFilletSolution,
) -> bool {
    let parents = [continuation.corner.first, continuation.corner.second];
    (0..2).all(|index| {
        if affine[index] {
            parents[index].neighborhood == ContactNeighborhood::Interior
        } else {
            matches!(
                parents[index].neighborhood,
                ContactNeighborhood::Local { lower, upper }
                    if lower < solution.parameters[index] && solution.parameters[index] < upper
            )
        }
    })
}

fn resolve_transported_circular_affine_root<'a>(
    sketch: &SketchDocument,
    input: &'a PersistentCornerRootInput<'a>,
    proof: CircularAffineBranchProof<'a>,
    controller: &mut OperationController,
) -> Result<PersistentCornerRootResolution<'a>, EvaluateFeatureError> {
    let proof_parents = circular_affine_branch_proof_parents(sketch, &input.prepared, proof)
        .ok_or({
            EvaluateFeatureError::Failure(ComputedFeatureFailure::NoLocalRoot {
                corner: input.corner,
            })
        })?;
    let transported_parents =
        circular_affine_transport_search_parents(sketch, &proof_parents, input.affine).ok_or({
            EvaluateFeatureError::Failure(ComputedFeatureFailure::NoLocalRoot {
                corner: input.corner,
            })
        })?;
    let (transported_solutions, transported_failure) = match local_fillet_roots(
        sketch,
        &transported_parents,
        input.sides,
        input.radius,
        input.policy,
        controller,
    ) {
        RootSearchResult::Completed { solutions, failure } => (solutions, failure),
        RootSearchResult::Stopped => return Err(EvaluateFeatureError::Stopped),
    };
    let transported_solutions = transported_solutions
        .into_iter()
        .filter(|solution| {
            transported_circular_affine_solution_is_certified(
                sketch,
                &proof_parents,
                input.affine,
                input.radius,
                *solution,
                proof,
            )
        })
        .collect::<Vec<_>>();
    let solution =
        select_seed_connected_solution(sketch, &transported_parents, &transported_solutions)
            .map_err(|kind| {
                EvaluateFeatureError::Failure(map_persistent_root_failure(
                    input.corner,
                    kind,
                    transported_failure,
                ))
            })?;
    Ok(PersistentCornerRootResolution {
        root_parents: transported_parents,
        solution,
        branch_validation: ArcBranchValidation::TransportedCircularAffine(proof),
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
        && !same_open_polyline_joined_spans(
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
    let source_spans = picks.map(|pick| pick.source.span);
    let mut solution =
        select_solution(sketch, source_spans, &solutions).map_err(|failure| match failure {
            RootSelectionFailure::None => match root_failure {
                RootSearchFailure::NoLocalRoot => ComputedFeatureAuthoringError::NoLocalRoot,
                RootSearchFailure::SingularParents => {
                    ComputedFeatureAuthoringError::SingularParents
                }
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
        solution = select_solution(sketch, source_spans, &corrected)
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
    let mut arc = build_and_validate_arc(
        sketch,
        persistent_parents,
        solution,
        radius,
        endpoint_order,
        DocumentArcSweep::CounterClockwise,
        ArcBranchValidation::PersistedCell,
    )
    .map_err(map_arc_authoring_failure)?;
    let canonical_corner = corner.canonicalized();
    if canonical_corner.first.source != corner.first.source {
        arc.contacts.swap(0, 1);
        arc.tangent_orientations.swap(0, 1);
    }
    Ok(AuthoringCornerResolution::Completed(Box::new(
        ResolvedAuthoringCorner {
            corner: canonical_corner,
            arc,
        },
    )))
}

fn validate_authoring_radius(radius: f64) -> Result<(), ComputedFeatureAuthoringError> {
    if !radius.is_finite() || radius <= 0.0 {
        return Err(ComputedFeatureAuthoringError::InvalidRadius);
    }
    Ok(())
}

fn charge_fillet_corner_validation(
    controller: &mut OperationController,
    corner_count: usize,
) -> bool {
    controller
        .charge(
            OperationWorkCounter::DocumentValidationItems,
            corner_count.saturating_mul(2),
            OperationCheckpoint::DocumentValidation,
        )
        .is_ok()
}

#[allow(
    clippy::too_many_lines,
    reason = "absolute continuation, validation and sensitivity publication are one auditable path"
)]
fn continue_absolute_corner(
    sketch: &SketchDocument,
    prior: NewComputedFilletCorner,
    from_radius: f64,
    target_radius: f64,
    policy: ComputedFeatureEvaluationPolicy,
    controller: &mut OperationController,
) -> Result<AbsoluteCornerResolution, ComputedFeatureAuthoringError> {
    let mut current = match resolve_seed_connected_absolute_corner(
        sketch,
        prior,
        from_radius,
        policy,
        controller,
    )? {
        AbsoluteCornerResolution::Completed(value) => *value,
        AbsoluteCornerResolution::Stopped => return Ok(AbsoluteCornerResolution::Stopped),
    };
    if from_radius.to_bits() == target_radius.to_bits() {
        return Ok(AbsoluteCornerResolution::Completed(Box::new(current)));
    }

    let mut step_fraction = 1.0;
    let mut last_failure = ComputedFeatureAuthoringError::IllConditionedRadiusSensitivity;
    for _ in 0..CONTINUATION_MAX_ATTEMPTS {
        let remaining = target_radius - current.radius;
        if !remaining.is_finite() {
            return Err(ComputedFeatureAuthoringError::InvalidRadius);
        }
        if remaining == 0.0 {
            return Ok(AbsoluteCornerResolution::Completed(Box::new(current)));
        }
        let step = remaining * step_fraction;
        let next_radius = current.radius + step;
        if !step.is_finite()
            || !next_radius.is_finite()
            || next_radius <= 0.0
            || next_radius.to_bits() == current.radius.to_bits()
        {
            return Err(last_failure);
        }
        match continue_connected_radius_step(sketch, &current, next_radius, policy, controller) {
            Ok(AbsoluteCornerResolution::Completed(next)) => {
                current = *next;
                if current.radius.to_bits() == target_radius.to_bits() {
                    return Ok(AbsoluteCornerResolution::Completed(Box::new(current)));
                }
                step_fraction = (step_fraction * 2.0_f64).min(1.0);
            }
            Ok(AbsoluteCornerResolution::Stopped) => {
                return Ok(AbsoluteCornerResolution::Stopped);
            }
            Err(error) if connected_step_can_subdivide(&error) => {
                last_failure = error;
                step_fraction *= 0.5;
            }
            Err(error) => return Err(error),
        }
    }
    Err(last_failure)
}

fn continue_numeric_absolute_corner(
    sketch: &SketchDocument,
    prior: NewComputedFilletCorner,
    from_radius: f64,
    target_radius: f64,
    policy: ComputedFeatureEvaluationPolicy,
    controller: &mut OperationController,
) -> Result<AbsoluteCornerResolution, ComputedFeatureAuthoringError> {
    match continue_absolute_corner(
        sketch,
        prior,
        from_radius,
        target_radius,
        policy,
        controller,
    ) {
        Ok(resolved) => return Ok(resolved),
        Err(ComputedFeatureAuthoringError::IllConditionedRadiusSensitivity)
            if from_radius.to_bits() != target_radius.to_bits() => {}
        Err(error) => return Err(error),
    }

    validate_exact_numeric_fold_origin(sketch, prior, from_radius)?;
    resolve_seed_connected_absolute_corner(sketch, prior, target_radius, policy, controller)
}

fn validate_exact_numeric_fold_origin(
    sketch: &SketchDocument,
    prior: NewComputedFilletCorner,
    radius: f64,
) -> Result<(), ComputedFeatureAuthoringError> {
    let (parents, affine, root_parents) = prepare_absolute_corner(sketch, prior)?;
    if affine == [true, true] {
        return Err(ComputedFeatureAuthoringError::IllConditionedRadiusSensitivity);
    }
    let sides = parents.map(|parent| parent.normal_side);
    let solution = exact_seed_solution(sketch, &root_parents, sides, radius)
        .map_err(map_offset_continuation_failure)?
        .ok_or(ComputedFeatureAuthoringError::InvalidContinuationState)?;
    let (corner, _) =
        complete_absolute_corner_geometry(sketch, parents, affine, solution, radius, prior)?;
    let quality = raw_signed_radius_transverse_quality(
        sketch,
        [corner.first, corner.second],
        solution,
        radius,
    )?;
    if quality.abs() > RADIUS_SENSITIVITY_MIN_TRANSVERSE_QUALITY {
        return Err(ComputedFeatureAuthoringError::IllConditionedRadiusSensitivity);
    }
    Ok(())
}

fn resolve_seed_connected_absolute_corner(
    sketch: &SketchDocument,
    prior: NewComputedFilletCorner,
    radius: f64,
    policy: ComputedFeatureEvaluationPolicy,
    controller: &mut OperationController,
) -> Result<AbsoluteCornerResolution, ComputedFeatureAuthoringError> {
    match resolve_exact_absolute_corner(sketch, prior, radius) {
        Ok(resolved) => {
            return Ok(AbsoluteCornerResolution::Completed(Box::new(resolved)));
        }
        Err(ComputedFeatureAuthoringError::InvalidContinuationState) => {}
        Err(error) => return Err(error),
    }
    let (parents, affine, root_parents) = prepare_absolute_corner(sketch, prior)?;
    let root_parents = current_branch_root_parents(&root_parents, affine)
        .ok_or(ComputedFeatureAuthoringError::InvalidContinuationState)?;
    let (solutions, root_failure) = match local_fillet_roots(
        sketch,
        &root_parents,
        parents.map(|parent| parent.normal_side),
        radius,
        policy,
        controller,
    ) {
        RootSearchResult::Completed { solutions, failure } => (solutions, failure),
        RootSearchResult::Stopped => return Ok(AbsoluteCornerResolution::Stopped),
    };
    let solution = select_seed_connected_solution(sketch, &root_parents, &solutions).map_err(
        |failure| match (failure, root_failure) {
            (RootSelectionFailure::None, RootSearchFailure::OffsetSingularity) => {
                ComputedFeatureAuthoringError::OffsetSingularity
            }
            (
                RootSelectionFailure::None,
                RootSearchFailure::NoLocalRoot | RootSearchFailure::SingularParents,
            )
            | (RootSelectionFailure::Ambiguous, _) => {
                ComputedFeatureAuthoringError::IllConditionedRadiusSensitivity
            }
        },
    )?;
    let resolved = complete_absolute_corner(sketch, parents, affine, solution, radius, prior)?;
    Ok(AbsoluteCornerResolution::Completed(Box::new(resolved)))
}

fn resolve_exact_absolute_corner(
    sketch: &SketchDocument,
    prior: NewComputedFilletCorner,
    radius: f64,
) -> Result<AbsoluteCornerContinuation, ComputedFeatureAuthoringError> {
    let (parents, affine, root_parents) = prepare_absolute_corner(sketch, prior)?;
    let sides = parents.map(|parent| parent.normal_side);
    let solution = exact_seed_solution(sketch, &root_parents, sides, radius)
        .map_err(map_offset_continuation_failure)?
        .ok_or(ComputedFeatureAuthoringError::InvalidContinuationState)?;
    complete_absolute_corner(sketch, parents, affine, solution, radius, prior)
}

fn exact_seed_solution(
    sketch: &SketchDocument,
    parents: &[RootParent; 2],
    sides: [DocumentCurveNormalSide; 2],
    radius: f64,
) -> Result<Option<LocalFilletSolution>, OffsetGeometryFailure> {
    let offsets = [0, 1].map(|index| {
        offset_geometry(
            sketch,
            parents[index].parent.source.span,
            parents[index].seed_total,
            sides[index],
            radius,
        )
    });
    let [first, second] = [offsets[0]?, offsets[1]?];
    let tolerance = (sketch.model_scale() * GEOMETRY_TOLERANCE_FACTOR).max(1.0e-11);
    if (first.point[0] - second.point[0]).hypot(first.point[1] - second.point[1]) > tolerance {
        return Ok(None);
    }
    Ok(Some(LocalFilletSolution {
        parameters: parents.map(|parent| parent.seed_total),
        sides,
        center: [
            0.5 * (first.point[0] + second.point[0]),
            0.5 * (first.point[1] + second.point[1]),
        ],
        score: 0.0,
    }))
}

fn continue_connected_radius_step(
    sketch: &SketchDocument,
    current: &AbsoluteCornerContinuation,
    radius: f64,
    policy: ComputedFeatureEvaluationPolicy,
    controller: &mut OperationController,
) -> Result<AbsoluteCornerResolution, ComputedFeatureAuthoringError> {
    let prior = current.corner;
    let (parents, affine, mut root_parents) = prepare_absolute_corner(sketch, prior)?;
    let radius_step = radius - current.radius;
    let current_parameters = current.arc.contacts.map(|contact| contact.total_parameter);
    let mut predictors = [0.0; 2];
    let mut widths = [0.0; 2];
    for index in 0..2 {
        let width = root_parents[index].bounds.1 - root_parents[index].bounds.0;
        let predictor = current.sensitivity.contact_parameter_derivatives[index]
            .mul_add(radius_step, current_parameters[index]);
        if !width.is_finite() || width <= 0.0 || !predictor.is_finite() {
            return Err(ComputedFeatureAuthoringError::NonFiniteRadiusSensitivity);
        }
        let predicted_motion = (predictor - current_parameters[index]).abs();
        if predicted_motion > CONTINUATION_MAX_PARAMETER_FRACTION * width
            || predictor <= root_parents[index].bounds.0
            || predictor >= root_parents[index].bounds.1
        {
            return Err(ComputedFeatureAuthoringError::IllConditionedRadiusSensitivity);
        }
        let margin = (CONTINUATION_MIN_BRACKET_FRACTION * width)
            .max(CONTINUATION_MAX_CORRECTION_FRACTION * predicted_motion);
        let lower = root_parents[index]
            .bounds
            .0
            .max(current_parameters[index].min(predictor) - margin);
        let upper = root_parents[index]
            .bounds
            .1
            .min(current_parameters[index].max(predictor) + margin);
        if !lower.is_finite() || !upper.is_finite() || lower >= upper {
            return Err(ComputedFeatureAuthoringError::IllConditionedRadiusSensitivity);
        }
        predictors[index] = predictor;
        widths[index] = width;
        root_parents[index].seed_total = predictor;
        root_parents[index].bounds = (lower, upper);
    }

    let (solutions, root_failure) = match local_fillet_roots(
        sketch,
        &root_parents,
        parents.map(|parent| parent.normal_side),
        radius,
        policy,
        controller,
    ) {
        RootSearchResult::Completed { solutions, failure } => (solutions, failure),
        RootSearchResult::Stopped => return Ok(AbsoluteCornerResolution::Stopped),
    };
    let solution = select_seed_connected_solution(sketch, &root_parents, &solutions).map_err(
        |failure| match (failure, root_failure) {
            (RootSelectionFailure::None, RootSearchFailure::OffsetSingularity) => {
                ComputedFeatureAuthoringError::OffsetSingularity
            }
            (
                RootSelectionFailure::None,
                RootSearchFailure::NoLocalRoot | RootSearchFailure::SingularParents,
            )
            | (RootSelectionFailure::Ambiguous, _) => {
                ComputedFeatureAuthoringError::IllConditionedRadiusSensitivity
            }
        },
    )?;
    for index in 0..2 {
        let actual_motion = (solution.parameters[index] - current_parameters[index]).abs();
        let correction = (solution.parameters[index] - predictors[index]).abs();
        let correction_limit = (CONTINUATION_MIN_BRACKET_FRACTION * widths[index]).max(
            CONTINUATION_MAX_CORRECTION_FRACTION
                * (predictors[index] - current_parameters[index]).abs(),
        );
        if actual_motion > CONTINUATION_MAX_PARAMETER_FRACTION * widths[index]
            || correction > correction_limit
        {
            return Err(ComputedFeatureAuthoringError::IllConditionedRadiusSensitivity);
        }
    }
    let resolved = complete_absolute_corner(sketch, parents, affine, solution, radius, prior)?;
    if current.signed_transverse_quality * resolved.signed_transverse_quality <= 0.0 {
        return Err(ComputedFeatureAuthoringError::IllConditionedRadiusSensitivity);
    }
    for index in 0..2 {
        let expected_motion = 0.5
            * (current.sensitivity.contact_parameter_derivatives[index]
                + resolved.sensitivity.contact_parameter_derivatives[index])
            * radius_step;
        let actual_motion =
            resolved.arc.contacts[index].total_parameter - current_parameters[index];
        let error_limit = CONTINUATION_MIN_BRACKET_FRACTION * widths[index]
            + CONTINUATION_MAX_CORRECTION_FRACTION * expected_motion.abs().max(actual_motion.abs());
        if (actual_motion - expected_motion).abs() > error_limit {
            return Err(ComputedFeatureAuthoringError::IllConditionedRadiusSensitivity);
        }
    }
    Ok(AbsoluteCornerResolution::Completed(Box::new(resolved)))
}

fn resolve_explicit_absolute_corner(
    sketch: &SketchDocument,
    prior: NewComputedFilletCorner,
    radius: f64,
    policy: ComputedFeatureEvaluationPolicy,
    controller: &mut OperationController,
    reseeded_parent: Option<ComputedFilletParentIndex>,
) -> Result<AbsoluteCornerResolution, ComputedFeatureAuthoringError> {
    let (parents, affine, root_parents) = prepare_absolute_corner(sketch, prior)?;
    let (solutions, root_failure) = match local_fillet_roots(
        sketch,
        &root_parents,
        parents.map(|parent| parent.normal_side),
        radius,
        policy,
        controller,
    ) {
        RootSearchResult::Completed { solutions, failure } => (solutions, failure),
        RootSearchResult::Stopped => return Ok(AbsoluteCornerResolution::Stopped),
    };
    let solution = match reseeded_parent {
        Some(parent) => select_reseeded_solution(sketch, &root_parents, &solutions, parent),
        None => select_solution(
            sketch,
            root_parents.map(|parent| parent.parent.source.span),
            &solutions,
        ),
    }
    .map_err(|failure| map_root_selection_failure(failure, root_failure))?;
    let resolved = complete_absolute_corner(sketch, parents, affine, solution, radius, prior)?;
    Ok(AbsoluteCornerResolution::Completed(Box::new(resolved)))
}

fn prepare_absolute_corner(
    sketch: &SketchDocument,
    prior: NewComputedFilletCorner,
) -> Result<PreparedAbsoluteCorner, ComputedFeatureAuthoringError> {
    if prior.first.source == prior.second.source {
        return Err(ComputedFeatureAuthoringError::InvalidContinuationState);
    }
    let parents = [prior.first, prior.second];
    let affine = parents.map(|parent| is_affine_line_span(sketch, parent.source.span));
    if affine == [false, false] {
        return Err(ComputedFeatureAuthoringError::UnsupportedCurvedPair);
    }
    let root_parents = prepare_root_parents(sketch, parents, None)
        .and_then(|prepared| {
            certify_persistent_branch(
                sketch,
                prepared,
                affine,
                ComputedFeatureCornerId::from_raw(0),
            )
        })
        .map_err(|failure| map_continuation_failure(&failure))?;
    Ok((parents, affine, root_parents))
}

fn complete_absolute_corner(
    sketch: &SketchDocument,
    parents: [ComputedFilletParent; 2],
    affine: [bool; 2],
    solution: LocalFilletSolution,
    radius: f64,
    prior: NewComputedFilletCorner,
) -> Result<AbsoluteCornerContinuation, ComputedFeatureAuthoringError> {
    let (corner, arc) =
        complete_absolute_corner_geometry(sketch, parents, affine, solution, radius, prior)?;
    let sensitivity =
        fillet_radius_sensitivity(sketch, [corner.first, corner.second], solution, radius)?;
    let signed_transverse_quality =
        signed_radius_transverse_quality(sketch, [corner.first, corner.second], solution, radius)?;
    Ok(AbsoluteCornerContinuation {
        radius,
        corner,
        arc,
        sensitivity,
        signed_transverse_quality,
    })
}

fn complete_absolute_corner_geometry(
    sketch: &SketchDocument,
    parents: [ComputedFilletParent; 2],
    affine: [bool; 2],
    solution: LocalFilletSolution,
    radius: f64,
    prior: NewComputedFilletCorner,
) -> Result<(NewComputedFilletCorner, ComputedCircularArc), ComputedFeatureAuthoringError> {
    let continued_parents = build_continued_parents(sketch, parents, affine, solution)?;
    let corner = NewComputedFilletCorner {
        first: continued_parents[0],
        second: continued_parents[1],
        endpoint_order: prior.endpoint_order,
        sweep: prior.sweep,
    }
    .canonicalized();
    let arc = build_and_validate_arc(
        sketch,
        [corner.first, corner.second],
        solution,
        radius,
        corner.endpoint_order,
        corner.sweep,
        ArcBranchValidation::PersistedCell,
    )
    .map_err(map_arc_continuation_failure)?;
    Ok((corner, arc))
}

const fn connected_step_can_subdivide(error: &ComputedFeatureAuthoringError) -> bool {
    matches!(
        error,
        ComputedFeatureAuthoringError::NoLocalRoot
            | ComputedFeatureAuthoringError::AmbiguousLocalRoot
            | ComputedFeatureAuthoringError::OffsetSingularity
            | ComputedFeatureAuthoringError::InvalidResolvedGeometry
            | ComputedFeatureAuthoringError::IllConditionedRadiusSensitivity
            | ComputedFeatureAuthoringError::NonFiniteRadiusSensitivity
    )
}

fn select_seed_connected_solution(
    sketch: &SketchDocument,
    parents: &[RootParent; 2],
    solutions: &[LocalFilletSolution],
) -> Result<LocalFilletSolution, RootSelectionFailure> {
    let Some(selected) = solutions
        .iter()
        .copied()
        .min_by(|left, right| left.score.total_cmp(&right.score))
    else {
        return Err(RootSelectionFailure::None);
    };
    if solutions
        .iter()
        .copied()
        .any(|candidate| !connected_solutions_share_geometry(sketch, parents, selected, candidate))
    {
        return Err(RootSelectionFailure::Ambiguous);
    }
    Ok(selected)
}

fn connected_solutions_share_geometry(
    sketch: &SketchDocument,
    parents: &[RootParent; 2],
    first: LocalFilletSolution,
    second: LocalFilletSolution,
) -> bool {
    let tolerance = (sketch.model_scale() * ROOT_DEDUPLICATION_FACTOR).max(1.0e-10);
    if (first.center[0] - second.center[0]).hypot(first.center[1] - second.center[1]) > tolerance {
        return false;
    }
    (0..2).all(|index| {
        let Ok(first_contact) =
            sketch.evaluate_curve_jet(parents[index].parent.source.span, first.parameters[index])
        else {
            return false;
        };
        let Ok(second_contact) =
            sketch.evaluate_curve_jet(parents[index].parent.source.span, second.parameters[index])
        else {
            return false;
        };
        (first_contact.position.x - second_contact.position.x)
            .hypot(first_contact.position.y - second_contact.position.y)
            <= tolerance
    })
}

const fn map_offset_continuation_failure(
    failure: OffsetGeometryFailure,
) -> ComputedFeatureAuthoringError {
    match failure {
        OffsetGeometryFailure::OffsetSingularity => {
            ComputedFeatureAuthoringError::OffsetSingularity
        }
        OffsetGeometryFailure::Invalid => ComputedFeatureAuthoringError::NonFiniteRadiusSensitivity,
    }
}

fn map_root_selection_failure(
    failure: RootSelectionFailure,
    root_failure: RootSearchFailure,
) -> ComputedFeatureAuthoringError {
    match failure {
        RootSelectionFailure::None => match root_failure {
            RootSearchFailure::NoLocalRoot => ComputedFeatureAuthoringError::NoLocalRoot,
            // A previously valid absolute corner reaching a transverse offset
            // singularity is a continuation limit, not a fresh-authoring
            // diagnosis that its source parents are intrinsically singular.
            RootSearchFailure::SingularParents => {
                ComputedFeatureAuthoringError::IllConditionedRadiusSensitivity
            }
            RootSearchFailure::OffsetSingularity => {
                ComputedFeatureAuthoringError::OffsetSingularity
            }
        },
        RootSelectionFailure::Ambiguous => ComputedFeatureAuthoringError::AmbiguousLocalRoot,
    }
}

fn map_continuation_failure(failure: &ComputedFeatureFailure) -> ComputedFeatureAuthoringError {
    match failure {
        ComputedFeatureFailure::MissingSource { .. } => ComputedFeatureAuthoringError::StalePick,
        ComputedFeatureFailure::AssociationOwnedSource { .. }
        | ComputedFeatureFailure::MultiIntervalSource { .. } => {
            ComputedFeatureAuthoringError::UnsupportedSourceTopology
        }
        ComputedFeatureFailure::UnsupportedCurvedPair { .. } => {
            ComputedFeatureAuthoringError::UnsupportedCurvedPair
        }
        ComputedFeatureFailure::NoLocalRoot { .. } => ComputedFeatureAuthoringError::NoLocalRoot,
        ComputedFeatureFailure::AmbiguousLocalRoot { .. } => {
            ComputedFeatureAuthoringError::AmbiguousLocalRoot
        }
        ComputedFeatureFailure::UncertifiedBranch { .. } => {
            ComputedFeatureAuthoringError::UncertifiedCurvedBranch
        }
        ComputedFeatureFailure::SingularParents { .. } => {
            ComputedFeatureAuthoringError::SingularParents
        }
        ComputedFeatureFailure::OffsetSingularity { .. } => {
            ComputedFeatureAuthoringError::OffsetSingularity
        }
        ComputedFeatureFailure::InvalidParentState { .. }
        | ComputedFeatureFailure::InvalidGeometry { .. }
        | ComputedFeatureFailure::EndpointClaimConflict { .. }
        | ComputedFeatureFailure::ConsumedSourceInterval { .. }
        | ComputedFeatureFailure::OffsetMissingSource { .. }
        | ComputedFeatureFailure::OffsetCurveFailure { .. }
        | ComputedFeatureFailure::OffsetJunctionFailure { .. }
        | ComputedFeatureFailure::OffsetTopologyChange => {
            ComputedFeatureAuthoringError::InvalidContinuationState
        }
    }
}

fn build_continued_parents(
    sketch: &SketchDocument,
    prior: [ComputedFilletParent; 2],
    affine: [bool; 2],
    solution: LocalFilletSolution,
) -> Result<[ComputedFilletParent; 2], ComputedFeatureAuthoringError> {
    let mut continued = Vec::with_capacity(2);
    for index in 0..2 {
        let topology = source_topology_for_authoring(sketch, prior[index].source)?;
        let total = solution.parameters[index];
        let (parameter, winding) = normalize_parameter(topology.domain, total)
            .ok_or(ComputedFeatureAuthoringError::InvalidResolvedGeometry)?;
        let neighborhood = prior[index].neighborhood;
        if affine[index] != matches!(neighborhood, ContactNeighborhood::Interior) {
            return Err(ComputedFeatureAuthoringError::InvalidContinuationState);
        }
        let periodic_anchor =
            periodic_anchor_for(topology.domain, total, prior[index].retained_endpoint)?;
        continued.push(ComputedFilletParent {
            source: prior[index].source,
            picked_parameter: parameter,
            winding,
            neighborhood,
            normal_side: solution.sides[index],
            retained_endpoint: prior[index].retained_endpoint,
            periodic_anchor,
        });
    }
    continued
        .try_into()
        .map_err(|_| ComputedFeatureAuthoringError::InvalidResolvedGeometry)
}

/// Re-anchors one independently validated source-edit result to its current
/// contacts. Unlike radius-only continuation, native-source movement refreshes
/// the curved Local certificate so the next accepted edit never depends on a
/// stale numeric cell edge.
fn build_source_continued_parents(
    sketch: &SketchDocument,
    prior: [ComputedFilletParent; 2],
    affine: [bool; 2],
    solution: LocalFilletSolution,
) -> Result<[ComputedFilletParent; 2], ComputedFeatureAuthoringError> {
    let mut continued = Vec::with_capacity(2);
    for index in 0..2 {
        let topology = source_topology_for_authoring(sketch, prior[index].source)?;
        let total = solution.parameters[index];
        let (parameter, winding) = normalize_parameter(topology.domain, total)
            .ok_or(ComputedFeatureAuthoringError::InvalidResolvedGeometry)?;
        let neighborhood = if affine[index] {
            if prior[index].neighborhood != ContactNeighborhood::Interior {
                return Err(ComputedFeatureAuthoringError::InvalidContinuationState);
            }
            ContactNeighborhood::Interior
        } else {
            let (support_lower, support_upper) = match topology.domain {
                SourceDomain::Bounded { lower, upper } => (lower, upper),
                SourceDomain::Periodic { period } => (total - 0.5 * period, total + 0.5 * period),
            };
            sketch
                .certify_line_curve_fillet_branch_cell(
                    prior[1 - index].source.span,
                    prior[index].source.span,
                    total,
                    support_lower,
                    support_upper,
                )
                .map_err(|_| ComputedFeatureAuthoringError::UncertifiedCurvedBranch)?
        };
        let periodic_anchor =
            periodic_anchor_for(topology.domain, total, prior[index].retained_endpoint)?;
        continued.push(ComputedFilletParent {
            source: prior[index].source,
            picked_parameter: parameter,
            winding,
            neighborhood,
            normal_side: solution.sides[index],
            retained_endpoint: prior[index].retained_endpoint,
            periodic_anchor,
        });
    }
    continued
        .try_into()
        .map_err(|_| ComputedFeatureAuthoringError::InvalidResolvedGeometry)
}

fn periodic_anchor_for(
    domain: SourceDomain,
    contact_total: f64,
    retained_endpoint: DocumentFilletTrimEndpoint,
) -> Result<Option<DocumentTrimParameter>, ComputedFeatureAuthoringError> {
    let SourceDomain::Periodic { period } = domain else {
        return Ok(None);
    };
    let anchor_total = match retained_endpoint {
        DocumentFilletTrimEndpoint::End => contact_total - 0.5 * period,
        DocumentFilletTrimEndpoint::Start => contact_total + 0.5 * period,
    };
    let (parameter, winding) = normalize_parameter(domain, anchor_total)
        .ok_or(ComputedFeatureAuthoringError::InvalidResolvedGeometry)?;
    Ok(Some(DocumentTrimParameter { parameter, winding }))
}

fn reseeded_absolute_corner(
    sketch: &SketchDocument,
    mut prior: NewComputedFilletCorner,
    selected: ComputedFilletParentIndex,
    parameter: f64,
) -> Result<NewComputedFilletCorner, ComputedFeatureAuthoringError> {
    if !parameter.is_finite() {
        return Err(ComputedFeatureAuthoringError::InvalidContactReseed);
    }
    let index = selected.index();
    let mut parents = [prior.first, prior.second];
    let topology = source_topology_for_authoring(sketch, parents[index].source)?;
    let prior_total = total_parameter(
        topology.domain,
        parents[index].picked_parameter,
        parents[index].winding,
    )
    .ok_or(ComputedFeatureAuthoringError::InvalidContinuationState)?;
    let (parameter, winding, total) = align_reseed_parameter(topology, prior_total, parameter)?;
    sketch
        .evaluate_curve_jet(parents[index].source.span, total)
        .map_err(|_| ComputedFeatureAuthoringError::InvalidContactReseed)?;
    let affine = is_affine_line_span(sketch, parents[index].source.span);
    let neighborhood = if affine {
        ContactNeighborhood::Interior
    } else {
        let (support_lower, support_upper) = match topology.domain {
            SourceDomain::Bounded { lower, upper } => (lower, upper),
            SourceDomain::Periodic { period } => (total - 0.5 * period, total + 0.5 * period),
        };
        sketch
            .certify_line_curve_fillet_branch_cell(
                parents[1 - index].source.span,
                parents[index].source.span,
                total,
                support_lower,
                support_upper,
            )
            .map_err(|_| ComputedFeatureAuthoringError::InvalidContactReseed)?
    };
    parents[index].picked_parameter = parameter;
    parents[index].winding = winding;
    parents[index].neighborhood = neighborhood;
    parents[index].periodic_anchor =
        periodic_anchor_for(topology.domain, total, parents[index].retained_endpoint)?;
    prior.first = parents[0];
    prior.second = parents[1];
    Ok(prior)
}

fn align_reseed_parameter(
    topology: SourceTopology,
    prior_total: f64,
    parameter: f64,
) -> Result<(f64, i32, f64), ComputedFeatureAuthoringError> {
    match topology.domain {
        SourceDomain::Bounded { lower, upper }
            if lower <= parameter
                && parameter <= upper
                && parameter_strictly_inside(parameter, topology.base_interval) =>
        {
            Ok((parameter, 0, parameter))
        }
        SourceDomain::Periodic { period }
            if 0.0 <= parameter && parameter < period && prior_total.is_finite() =>
        {
            let winding = ((prior_total - parameter) / period).round();
            if winding < f64::from(i32::MIN) || winding > f64::from(i32::MAX) {
                return Err(ComputedFeatureAuthoringError::InvalidContactReseed);
            }
            #[allow(clippy::cast_possible_truncation)]
            let winding = winding as i32;
            let total = parameter + f64::from(winding) * period;
            total
                .is_finite()
                .then_some((parameter, winding, total))
                .ok_or(ComputedFeatureAuthoringError::InvalidContactReseed)
        }
        SourceDomain::Bounded { .. } | SourceDomain::Periodic { .. } => {
            Err(ComputedFeatureAuthoringError::InvalidContactReseed)
        }
    }
}

fn select_reseeded_solution(
    sketch: &SketchDocument,
    parents: &[RootParent; 2],
    solutions: &[LocalFilletSolution],
    selected: ComputedFilletParentIndex,
) -> Result<LocalFilletSolution, RootSelectionFailure> {
    let selected = selected.index();
    let other = 1 - selected;
    let normalized_distance = |solution: LocalFilletSolution, index: usize| {
        (solution.parameters[index] - parents[index].seed_total).abs()
            / (parents[index].bounds.1 - parents[index].bounds.0)
    };
    let Some(solution) = solutions.iter().copied().min_by(|left, right| {
        normalized_distance(*left, selected)
            .total_cmp(&normalized_distance(*right, selected))
            .then_with(|| {
                normalized_distance(*left, other).total_cmp(&normalized_distance(*right, other))
            })
            .then_with(|| side_rank(left.sides).cmp(&side_rank(right.sides)))
    }) else {
        return Err(RootSelectionFailure::None);
    };
    let primary = normalized_distance(solution, selected);
    let secondary = normalized_distance(solution, other);
    if solutions.iter().copied().any(|candidate| {
        solutions_materially_distinct(
            sketch,
            parents.map(|parent| parent.parent.source.span),
            solution,
            candidate,
        ) && scores_nearly_tied(primary, normalized_distance(candidate, selected))
            && scores_nearly_tied(secondary, normalized_distance(candidate, other))
    }) {
        return Err(RootSelectionFailure::Ambiguous);
    }
    Ok(solution)
}

fn fillet_radius_sensitivity(
    sketch: &SketchDocument,
    parents: [ComputedFilletParent; 2],
    solution: LocalFilletSolution,
    radius: f64,
) -> Result<ComputedFilletRadiusSensitivity, ComputedFeatureAuthoringError> {
    let mut offset_derivatives = [[0.0; 2]; 2];
    let mut normals = [[0.0; 2]; 2];
    let mut source_derivatives = [[0.0; 2]; 2];
    for index in 0..2 {
        let offset = offset_geometry(
            sketch,
            parents[index].source.span,
            solution.parameters[index],
            parents[index].normal_side,
            radius,
        )
        .map_err(|failure| match failure {
            OffsetGeometryFailure::OffsetSingularity => {
                ComputedFeatureAuthoringError::OffsetSingularity
            }
            OffsetGeometryFailure::Invalid => {
                ComputedFeatureAuthoringError::NonFiniteRadiusSensitivity
            }
        })?;
        let jet = sketch
            .evaluate_curve_jet(parents[index].source.span, solution.parameters[index])
            .map_err(|_| ComputedFeatureAuthoringError::NonFiniteRadiusSensitivity)?;
        let differential = jet
            .differential()
            .map_err(|_| ComputedFeatureAuthoringError::NonFiniteRadiusSensitivity)?;
        offset_derivatives[index] = offset.derivative;
        normals[index] = [differential.left_normal.x, differential.left_normal.y];
        source_derivatives[index] = [jet.first_derivative.x, jet.first_derivative.y];
    }
    let first = offset_derivatives[0];
    let second = offset_derivatives[1];
    let determinant = first[1].mul_add(second[0], -(first[0] * second[1]));
    let scale = first[0].hypot(first[1]) * second[0].hypot(second[1]);
    if !determinant.is_finite() || !scale.is_finite() || scale <= 0.0 {
        return Err(ComputedFeatureAuthoringError::NonFiniteRadiusSensitivity);
    }
    let transverse_quality = determinant.abs() / scale;
    if !transverse_quality.is_finite()
        || transverse_quality <= RADIUS_SENSITIVITY_MIN_TRANSVERSE_QUALITY
    {
        return Err(ComputedFeatureAuthoringError::IllConditionedRadiusSensitivity);
    }
    let signs = parents.map(|parent| side_sign(parent.normal_side));
    let rhs = [
        signs[1].mul_add(normals[1][0], -(signs[0] * normals[0][0])),
        signs[1].mul_add(normals[1][1], -(signs[0] * normals[0][1])),
    ];
    // [first, -second] * [dt1/dr, dt2/dr] = rhs.
    let parameter_derivatives = [
        (second[0] * rhs[1] - rhs[0] * second[1]) / determinant,
        (first[0] * rhs[1] - rhs[0] * first[1]) / determinant,
    ];
    let center_from_first = [
        first[0].mul_add(parameter_derivatives[0], signs[0] * normals[0][0]),
        first[1].mul_add(parameter_derivatives[0], signs[0] * normals[0][1]),
    ];
    let center_from_second = [
        second[0].mul_add(parameter_derivatives[1], signs[1] * normals[1][0]),
        second[1].mul_add(parameter_derivatives[1], signs[1] * normals[1][1]),
    ];
    let center_derivative = [
        0.5 * (center_from_first[0] + center_from_second[0]),
        0.5 * (center_from_first[1] + center_from_second[1]),
    ];
    let contact_position_derivatives = [0, 1].map(|index| {
        [
            source_derivatives[index][0] * parameter_derivatives[index],
            source_derivatives[index][1] * parameter_derivatives[index],
        ]
    });
    let values_are_finite = parameter_derivatives
        .into_iter()
        .chain(center_derivative)
        .chain(center_from_first)
        .chain(center_from_second)
        .chain(contact_position_derivatives.into_iter().flatten())
        .all(f64::is_finite);
    if !values_are_finite {
        return Err(ComputedFeatureAuthoringError::NonFiniteRadiusSensitivity);
    }
    let center_scale = center_from_first
        .into_iter()
        .chain(center_from_second)
        .map(f64::abs)
        .fold(1.0, f64::max);
    if (center_from_first[0] - center_from_second[0])
        .hypot(center_from_first[1] - center_from_second[1])
        > 1.0e-8 * center_scale
        || center_derivative[0].hypot(center_derivative[1]) <= RADIUS_RAIL_MIN_NORM
    {
        return Err(ComputedFeatureAuthoringError::IllConditionedRadiusSensitivity);
    }
    Ok(ComputedFilletRadiusSensitivity {
        center_derivative,
        contact_parameter_derivatives: parameter_derivatives,
        contact_position_derivatives,
        transverse_quality,
    })
}

fn signed_radius_transverse_quality(
    sketch: &SketchDocument,
    parents: [ComputedFilletParent; 2],
    solution: LocalFilletSolution,
    radius: f64,
) -> Result<f64, ComputedFeatureAuthoringError> {
    let quality = raw_signed_radius_transverse_quality(sketch, parents, solution, radius)?;
    if quality.abs() <= RADIUS_SENSITIVITY_MIN_TRANSVERSE_QUALITY {
        return Err(ComputedFeatureAuthoringError::IllConditionedRadiusSensitivity);
    }
    Ok(quality)
}

fn raw_signed_radius_transverse_quality(
    sketch: &SketchDocument,
    parents: [ComputedFilletParent; 2],
    solution: LocalFilletSolution,
    radius: f64,
) -> Result<f64, ComputedFeatureAuthoringError> {
    let mut derivatives = [[0.0; 2]; 2];
    for index in 0..2 {
        derivatives[index] = offset_geometry(
            sketch,
            parents[index].source.span,
            solution.parameters[index],
            parents[index].normal_side,
            radius,
        )
        .map_err(map_offset_continuation_failure)?
        .derivative;
    }
    let determinant =
        derivatives[0][1].mul_add(derivatives[1][0], -(derivatives[0][0] * derivatives[1][1]));
    let scale =
        derivatives[0][0].hypot(derivatives[0][1]) * derivatives[1][0].hypot(derivatives[1][1]);
    let quality = determinant / scale;
    if !quality.is_finite() {
        return Err(ComputedFeatureAuthoringError::NonFiniteRadiusSensitivity);
    }
    Ok(quality)
}

fn fillet_offset_tangent_directions(
    sketch: &SketchDocument,
    parents: [ComputedFilletParent; 2],
    solution: LocalFilletSolution,
    radius: f64,
) -> Option<[[f64; 2]; 2]> {
    let mut directions = [[0.0; 2]; 2];
    for index in 0..2 {
        let derivative = offset_geometry(
            sketch,
            parents[index].source.span,
            solution.parameters[index],
            parents[index].normal_side,
            radius,
        )
        .ok()?
        .derivative;
        let norm = derivative[0].hypot(derivative[1]);
        if !norm.is_finite() || norm <= 0.0 {
            return None;
        }
        directions[index] = [derivative[0] / norm, derivative[1] / norm];
    }
    Some(directions)
}

fn fillet_transverse_orientation_for_derivatives(
    first: [f64; 2],
    second: [f64; 2],
) -> Option<ComputedFilletTransverseOrientation> {
    let determinant = first[1].mul_add(second[0], -(first[0] * second[1]));
    let scale = first[0].hypot(first[1]) * second[0].hypot(second[1]);
    if !determinant.is_finite()
        || !scale.is_finite()
        || scale <= 0.0
        || determinant.abs() <= PARENT_SINGULARITY_TOLERANCE * scale
    {
        return None;
    }
    Some(if determinant < 0.0 {
        ComputedFilletTransverseOrientation::Negative
    } else {
        ComputedFilletTransverseOrientation::Positive
    })
}

const fn flip_trim_endpoint(endpoint: DocumentFilletTrimEndpoint) -> DocumentFilletTrimEndpoint {
    match endpoint {
        DocumentFilletTrimEndpoint::Start => DocumentFilletTrimEndpoint::End,
        DocumentFilletTrimEndpoint::End => DocumentFilletTrimEndpoint::Start,
    }
}

fn set_retained_endpoint(
    sketch: &SketchDocument,
    corner: &mut NewComputedFilletCorner,
    selected: ComputedFilletParentIndex,
    endpoint: DocumentFilletTrimEndpoint,
) -> Result<(), ComputedFeatureAuthoringError> {
    let parent = match selected {
        ComputedFilletParentIndex::First => &mut corner.first,
        ComputedFilletParentIndex::Second => &mut corner.second,
    };
    let topology = source_topology_for_authoring(sketch, parent.source)?;
    let total = total_parameter(topology.domain, parent.picked_parameter, parent.winding)
        .ok_or(ComputedFeatureAuthoringError::InvalidContinuationState)?;
    parent.retained_endpoint = endpoint;
    parent.periodic_anchor = periodic_anchor_for(topology.domain, total, endpoint)?;
    Ok(())
}

const fn local_alternative_is_unavailable(error: &ComputedFeatureAuthoringError) -> bool {
    matches!(
        error,
        ComputedFeatureAuthoringError::NoLocalRoot
            | ComputedFeatureAuthoringError::AmbiguousLocalRoot
            | ComputedFeatureAuthoringError::SingularParents
            | ComputedFeatureAuthoringError::OffsetSingularity
            | ComputedFeatureAuthoringError::InvalidResolvedGeometry
            | ComputedFeatureAuthoringError::IllConditionedRadiusSensitivity
            | ComputedFeatureAuthoringError::NonFiniteRadiusSensitivity
    )
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

fn seed_connected_root_parents(mut parents: [RootParent; 2]) -> Option<[RootParent; 2]> {
    for parent in &mut parents {
        let width = parent.bounds.1 - parent.bounds.0;
        if !width.is_finite() || width <= 0.0 || !parent.seed_total.is_finite() {
            return None;
        }
        let radius = CONTINUATION_MAX_PARAMETER_FRACTION * width;
        let lower = parent.bounds.0.max(parent.seed_total - radius);
        let upper = parent.bounds.1.min(parent.seed_total + radius);
        if !lower.is_finite() || !upper.is_finite() || lower >= upper {
            return None;
        }
        parent.bounds = (lower, upper);
    }
    Some(parents)
}

/// Returns the bounded search cells for the branch that is current after a
/// native-source edit.
///
/// Two transverse affine supports have exactly one offset intersection for
/// fixed normal sides and radius. Their complete certified cells are therefore
/// branch-local even when a source edit moves the new contact far from its
/// persisted parameter seed. A non-affine parent can have multiple roots in
/// one certified cell, so it retains the narrow seed-connected guard against a
/// remote root hop.
fn current_branch_root_parents(
    parents: &[RootParent; 2],
    affine: [bool; 2],
) -> Option<[RootParent; 2]> {
    if affine == [true, true] {
        Some(*parents)
    } else {
        seed_connected_root_parents(*parents)
    }
}

/// Returns the bounded search cells used to re-evaluate persisted intent after
/// native source geometry changes.
///
/// Circular parents have constant signed curvature, so a nonsingular fixed
/// radius offset cannot fold inside one tangent-orientation cell. Searching
/// that complete explicit cell remains branch-local. Other non-affine parents
/// retain the narrower seed-connected cell because their offset regularity can
/// change without crossing a tangent-parallel barrier.
fn persistent_evaluation_root_parents(
    sketch: &SketchDocument,
    parents: &[RootParent; 2],
    affine: [bool; 2],
) -> Option<[RootParent; 2]> {
    if affine == [true, true]
        || (0..2).any(|index| {
            !affine[index]
                && is_constant_curvature_circular_span(sketch, parents[index].parent.source.span)
        })
    {
        Some(*parents)
    } else {
        seed_connected_root_parents(*parents)
    }
}

/// Broadens one persisted circular/affine Fillet only to the visible retained
/// circular support. Candidate roots still have to prove that their freshly
/// certified tangent-orientation cell overlaps the cell at the persisted seed;
/// these bounds are therefore a search envelope, not permission to cross a
/// branch barrier.
fn circular_affine_transport_search_parents(
    sketch: &SketchDocument,
    parents: &[RootParent; 2],
    affine: [bool; 2],
) -> Option<[RootParent; 2]> {
    let (curved_index, affine_index) = circular_affine_parent_indices(sketch, parents, affine)?;
    if parents[affine_index].parent.neighborhood != ContactNeighborhood::Interior {
        return None;
    }
    let mut transported = *parents;
    transported[curved_index].bounds =
        interior_bounds(parents[curved_index].topology.base_interval)?;
    Some(transported)
}

/// Selects the exact periodic/contact frame that owns a circular/affine
/// transport proof. A persisted-cell proof uses durable intent directly. An
/// accepted continuation uses its freshly re-anchored contact frame so a
/// continuous drag may cross the old periodic-anchor seam without widening
/// past the previous accepted branch cell.
fn circular_affine_branch_proof_parents(
    sketch: &SketchDocument,
    persisted: &[RootParent; 2],
    proof: CircularAffineBranchProof<'_>,
) -> Option<[RootParent; 2]> {
    match proof {
        CircularAffineBranchProof::PersistedCellOverlap => Some(*persisted),
        CircularAffineBranchProof::AcceptedContinuation(continuation) => prepare_root_parents(
            sketch,
            [continuation.corner.first, continuation.corner.second],
            Some(continuation.owner.corner),
        )
        .ok(),
    }
}

fn circular_affine_parent_indices(
    sketch: &SketchDocument,
    parents: &[RootParent; 2],
    affine: [bool; 2],
) -> Option<(usize, usize)> {
    let (curved_index, affine_index) = match affine {
        [false, true] => (0, 1),
        [true, false] => (1, 0),
        _ => return None,
    };
    is_constant_curvature_circular_span(sketch, parents[curved_index].parent.source.span)
        .then_some((curved_index, affine_index))
}

/// Proves that a root displaced by a native-source edit remains in the same
/// current tangent-orientation branch as the persisted contact seed.
///
/// The stored Local interval is a durable branch witness, but its numeric edge
/// is only a conservative certificate from the geometry that existed when the
/// Fillet was authored. Fresh cells around the old seed and proposed root may
/// overlap beyond that stale edge. Their overlap is a proof of connected,
/// nonzero tangent orientation; opposite sides of a real parallel-tangent
/// barrier cannot both certify the same open parameter interval.
fn transported_circular_affine_solution_is_certified(
    sketch: &SketchDocument,
    parents: &[RootParent; 2],
    affine: [bool; 2],
    radius: f64,
    solution: LocalFilletSolution,
    proof: CircularAffineBranchProof<'_>,
) -> bool {
    let Some((curved_index, affine_index)) =
        circular_affine_parent_indices(sketch, parents, affine)
    else {
        return false;
    };
    let Some(search_parents) = circular_affine_transport_search_parents(sketch, parents, affine)
    else {
        return false;
    };
    if solution.sides != parents.map(|parent| parent.parent.normal_side)
        || solution
            .parameters
            .iter()
            .enumerate()
            .any(|(index, parameter)| {
                !parameter.is_finite()
                    || *parameter < search_parents[index].bounds.0
                    || *parameter > search_parents[index].bounds.1
                    || !parameter_strictly_inside(
                        *parameter,
                        search_parents[index].topology.base_interval,
                    )
            })
    {
        return false;
    }
    let Some(candidate_directions) = fillet_offset_tangent_directions(
        sketch,
        parents.map(|parent| parent.parent),
        solution,
        radius,
    ) else {
        return false;
    };
    let Some(candidate_orientation) = fillet_transverse_orientation_for_derivatives(
        candidate_directions[0],
        candidate_directions[1],
    ) else {
        return false;
    };
    if let CircularAffineBranchProof::AcceptedContinuation(continuation) = proof {
        return continuation_cell_contains_solution(continuation, affine, solution)
            && candidate_orientation == continuation.transverse_orientation
            && candidate_directions
                .into_iter()
                .zip(continuation.offset_tangent_directions)
                .all(|(current, previous)| {
                    current[0].mul_add(previous[0], current[1] * previous[1])
                        > CONTINUATION_MIN_TANGENT_DIRECTION_DOT
                });
    }
    let ContactNeighborhood::Local {
        lower: stored_lower,
        upper: stored_upper,
    } = parents[curved_index].parent.neighborhood
    else {
        return false;
    };
    let stored_cell = ComputedSourceInterval {
        start: stored_lower,
        end: stored_upper,
    };
    let Some(seed_cell) = certify_current_circular_affine_cell(
        sketch,
        parents,
        curved_index,
        affine_index,
        parents[curved_index].seed_total,
    ) else {
        return false;
    };
    let Some(candidate_cell) = certify_current_circular_affine_cell(
        sketch,
        parents,
        curved_index,
        affine_index,
        solution.parameters[curved_index],
    ) else {
        return false;
    };
    certified_cells_overlap(stored_cell, seed_cell)
        && certified_cells_overlap(seed_cell, candidate_cell)
        && persisted_current_and_candidate_orientations_agree(
            sketch,
            parents,
            curved_index,
            affine_index,
            radius,
            solution,
            candidate_orientation,
        )
}

fn persisted_current_and_candidate_orientations_agree(
    sketch: &SketchDocument,
    parents: &[RootParent; 2],
    curved_index: usize,
    affine_index: usize,
    radius: f64,
    solution: LocalFilletSolution,
    candidate_orientation: ComputedFilletTransverseOrientation,
) -> bool {
    let Some(branch_direction) =
        sketch.curve_branch_direction(parents[affine_index].parent.source.span)
    else {
        return false;
    };
    let Ok(curved_seed) = offset_geometry(
        sketch,
        parents[curved_index].parent.source.span,
        parents[curved_index].seed_total,
        parents[curved_index].parent.normal_side,
        radius,
    ) else {
        return false;
    };
    let Ok(current_affine) = offset_geometry(
        sketch,
        parents[affine_index].parent.source.span,
        solution.parameters[affine_index],
        parents[affine_index].parent.normal_side,
        radius,
    ) else {
        return false;
    };
    let ordered = |affine_derivative| {
        if curved_index == 0 {
            fillet_transverse_orientation_for_derivatives(curved_seed.derivative, affine_derivative)
        } else {
            fillet_transverse_orientation_for_derivatives(affine_derivative, curved_seed.derivative)
        }
    };
    ordered(branch_direction) == Some(candidate_orientation)
        && ordered(current_affine.derivative) == Some(candidate_orientation)
}

fn certify_current_circular_affine_cell(
    sketch: &SketchDocument,
    parents: &[RootParent; 2],
    curved_index: usize,
    affine_index: usize,
    parameter: f64,
) -> Option<ComputedSourceInterval> {
    let support = parents[curved_index].topology.base_interval;
    if !parameter_strictly_inside(parameter, support) {
        return None;
    }
    let ContactNeighborhood::Local { lower, upper } = sketch
        .certify_line_curve_fillet_branch_cell(
            parents[affine_index].parent.source.span,
            parents[curved_index].parent.source.span,
            parameter,
            support.start,
            support.end,
        )
        .ok()?
    else {
        return None;
    };
    Some(ComputedSourceInterval {
        start: lower,
        end: upper,
    })
}

fn certified_cells_overlap(first: ComputedSourceInterval, second: ComputedSourceInterval) -> bool {
    interior_bounds(ComputedSourceInterval {
        start: first.start.max(second.start),
        end: first.end.min(second.end),
    })
    .is_some()
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
            if solutions.iter().all(|existing| {
                solutions_materially_distinct(
                    sketch,
                    parents.map(|parent| parent.parent.source.span),
                    *existing,
                    solution,
                )
            }) {
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
    let tolerance = (sketch.model_scale() * ROOT_POLISH_TOLERANCE_FACTOR).max(1.0e-15);
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

const fn map_persistent_root_failure(
    corner: ComputedFeatureCornerId,
    selection: RootSelectionFailure,
    search: RootSearchFailure,
) -> ComputedFeatureFailure {
    match selection {
        RootSelectionFailure::None => match search {
            RootSearchFailure::NoLocalRoot => ComputedFeatureFailure::NoLocalRoot { corner },
            RootSearchFailure::SingularParents => {
                ComputedFeatureFailure::SingularParents { corner }
            }
            RootSearchFailure::OffsetSingularity => {
                ComputedFeatureFailure::OffsetSingularity { corner }
            }
        },
        RootSelectionFailure::Ambiguous => ComputedFeatureFailure::AmbiguousLocalRoot { corner },
    }
}

fn select_solution(
    sketch: &SketchDocument,
    source_spans: [CurveSpan; 2],
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
            && solutions_materially_distinct(sketch, source_spans, solution, *other)
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
    source_spans: [CurveSpan; 2],
    first: LocalFilletSolution,
    second: LocalFilletSolution,
) -> bool {
    let position_tolerance = (sketch.model_scale() * ROOT_DEDUPLICATION_FACTOR).max(1.0e-10);
    if (first.center[0] - second.center[0]).hypot(first.center[1] - second.center[1])
        > position_tolerance
    {
        return true;
    }
    (0..2).any(|index| {
        let Ok(first_contact) =
            sketch.evaluate_curve_jet(source_spans[index], first.parameters[index])
        else {
            return true;
        };
        let Ok(second_contact) =
            sketch.evaluate_curve_jet(source_spans[index], second.parameters[index])
        else {
            return true;
        };
        (first_contact.position.x - second_contact.position.x)
            .hypot(first_contact.position.y - second_contact.position.y)
            > position_tolerance
    })
}

#[derive(Clone, Copy, Debug)]
enum ArcValidationFailure {
    OffsetSingularity,
    SingularParents,
    Invalid,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum ArcBranchValidation<'a> {
    PersistedCell,
    TransportedCircularAffine(CircularAffineBranchProof<'a>),
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
    branch_validation: ArcBranchValidation<'_>,
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
    let prepared =
        prepare_root_parents(sketch, parents, None).map_err(|_| ArcValidationFailure::Invalid)?;
    let publication_parents = match branch_validation {
        ArcBranchValidation::PersistedCell => certify_persistent_branch(
            sketch,
            prepared,
            affine,
            ComputedFeatureCornerId::from_raw(0),
        )
        .map_err(|_| ArcValidationFailure::Invalid)?,
        ArcBranchValidation::TransportedCircularAffine(proof) => {
            let proof_parents = circular_affine_branch_proof_parents(sketch, &prepared, proof)
                .ok_or(ArcValidationFailure::Invalid)?;
            if !transported_circular_affine_solution_is_certified(
                sketch,
                &proof_parents,
                affine,
                radius,
                solution,
                proof,
            ) {
                return Err(ArcValidationFailure::Invalid);
            }
            circular_affine_transport_search_parents(sketch, &proof_parents, affine)
                .ok_or(ArcValidationFailure::Invalid)?
        }
    };
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
    let mut tangent_orientations = [TangentOrientation::Aligned; 2];
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
        let arc_tangent = match sweep {
            DocumentArcSweep::CounterClockwise => [-radial[1], radial[0]],
            DocumentArcSweep::Clockwise => [radial[1], -radial[0]],
        };
        let tangent_dot = jet
            .first_derivative
            .x
            .mul_add(arc_tangent[0], jet.first_derivative.y * arc_tangent[1]);
        if !tangent_dot.is_finite() || tangent_dot.abs() <= f64::EPSILON * radius * tangent_length {
            return Err(ArcValidationFailure::Invalid);
        }
        tangent_orientations[index] = if tangent_dot.is_sign_positive() {
            TangentOrientation::Aligned
        } else {
            TangentOrientation::Opposed
        };
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
        tangent_orientations,
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
        ArcBranchValidation::PersistedCell,
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

const fn map_arc_continuation_failure(
    failure: ArcValidationFailure,
) -> ComputedFeatureAuthoringError {
    match failure {
        ArcValidationFailure::OffsetSingularity => ComputedFeatureAuthoringError::OffsetSingularity,
        ArcValidationFailure::SingularParents => {
            ComputedFeatureAuthoringError::IllConditionedRadiusSensitivity
        }
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

fn is_constant_curvature_circular_span(sketch: &SketchDocument, span: CurveSpan) -> bool {
    span.segment == 0
        && sketch.curve(span.curve).is_some_and(|curve| {
            matches!(
                curve.definition,
                CurveDefinition::Circle { .. } | CurveDefinition::CircularArc { .. }
            )
        })
}

fn same_open_polyline_joined_spans(
    sketch: &SketchDocument,
    first: CurveSpan,
    second: CurveSpan,
) -> bool {
    if first.curve != second.curve || first == second {
        return false;
    }
    let Some(curve) = sketch.curve(first.curve) else {
        return false;
    };
    let CurveDefinition::Polyline {
        points,
        closed: false,
        ..
    } = &curve.definition
    else {
        return false;
    };
    let (Ok(first_index), Ok(second_index)) = (
        usize::try_from(first.segment),
        usize::try_from(second.segment),
    ) else {
        return false;
    };
    let Some(first_end) = first_index.checked_add(1) else {
        return false;
    };
    let Some(second_end) = second_index.checked_add(1) else {
        return false;
    };
    if first_end >= points.len() || second_end >= points.len() {
        return false;
    }
    if first.segment.abs_diff(second.segment) == 1 {
        return true;
    }
    let representatives = sketch.point_coincidence_representatives();
    let first_endpoints = [points[first_index], points[first_end]];
    let second_endpoints = [points[second_index], points[second_end]];
    first_endpoints
        .into_iter()
        .flat_map(|first| {
            second_endpoints
                .into_iter()
                .map(move |second| (first, second))
        })
        .filter(|(first, second)| representatives.get(first) == representatives.get(second))
        .count()
        == 1
}

#[cfg(test)]
mod publication_tests {
    use super::*;
    use geosolve_sketch::{
        DocumentCurveTrimView, DocumentId, DocumentTrimBoundary, DocumentTrimParameter,
        PersistentId, ScalarDomain, ScalarUnit,
    };

    #[test]
    fn tolerance_empty_discarded_complements_preserve_effective_output_without_publication() {
        let mut sketch =
            SketchDocument::with_id(10.0, DocumentId(PersistentId::from_u128(0x5002))).unwrap();
        let start = sketch.add_point("start", [0.0, 0.0]).unwrap();
        let end = sketch.add_point("end", [4.0, 0.0]).unwrap();
        let source = NativeCurveSpanSource {
            span: CurveSpan::line(
                sketch
                    .add_curve(
                        "line",
                        CurveDefinition::Line {
                            start,
                            end,
                            branch_direction: [1.0, 0.0],
                        },
                    )
                    .unwrap(),
            ),
        };
        let base_interval = ComputedSourceInterval {
            start: 0.0,
            end: 1.0,
        };
        let owner = ComputedCornerRef {
            feature: ComputedFeatureId::from_raw(1),
            corner: ComputedFeatureCornerId::from_raw(1),
        };
        let parameter = 0.5 * parameter_tolerance(base_interval);
        let claim = EndpointClaim {
            owner,
            source,
            endpoint: DocumentFilletTrimEndpoint::Start,
            parameter,
            base_interval,
            participates_in_trimming: true,
        };
        let output = compose_source_output(&SourceComposition {
            source,
            base_interval,
            start: Some(claim),
            end: None,
        })
        .expect("a strictly interior claim remains valid");
        assert_eq!(
            output.effective_interval,
            ComputedSourceInterval {
                start: parameter,
                end: base_interval.end,
            }
        );
        assert!(output.discarded.is_empty());
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "one compact topology fixture compares every relevant open/closed source class"
    )]
    fn trim_participation_distinguishes_closed_periodic_loops_from_open_curves() {
        let mut sketch =
            SketchDocument::with_id(10.0, DocumentId(PersistentId::from_u128(0x5001))).unwrap();
        let center = sketch.add_point("center", [0.0, 0.0]).unwrap();
        let axis = sketch.add_point("ellipse axis", [2.0, 0.0]).unwrap();
        let radius = sketch
            .add_scalar("radius", 2.0, ScalarUnit::Length, ScalarDomain::Positive)
            .unwrap();
        let arc_radius = sketch
            .add_scalar(
                "arc radius",
                2.0,
                ScalarUnit::Length,
                ScalarDomain::Positive,
            )
            .unwrap();
        let ratio = sketch
            .add_scalar(
                "ellipse ratio",
                0.5,
                ScalarUnit::Parameter,
                ScalarDomain::Bounded {
                    lower: f64::from_bits(1),
                    upper: 1.0,
                },
            )
            .unwrap();
        let start_angle = sketch
            .add_scalar("arc start", 0.0, ScalarUnit::Angle, ScalarDomain::Finite)
            .unwrap();
        let end_angle = sketch
            .add_scalar(
                "arc end",
                std::f64::consts::PI,
                ScalarUnit::Angle,
                ScalarDomain::Finite,
            )
            .unwrap();
        let circle = sketch
            .add_curve("circle", CurveDefinition::Circle { center, radius })
            .unwrap();
        let ellipse = sketch
            .add_curve(
                "ellipse",
                CurveDefinition::Ellipse {
                    center,
                    major_axis_point: axis,
                    minor_axis_ratio: ratio,
                },
            )
            .unwrap();
        let arc = sketch
            .add_curve(
                "arc",
                CurveDefinition::CircularArc {
                    center,
                    radius: arc_radius,
                    start_angle,
                    end_angle,
                    sweep: DocumentArcSweep::CounterClockwise,
                },
            )
            .unwrap();

        for curve in [circle, ellipse] {
            assert!(
                !source_topology(
                    &sketch,
                    NativeCurveSpanSource {
                        span: CurveSpan::line(curve),
                    }
                )
                .unwrap()
                .participates_in_trimming(),
                "full circles and ellipses remain complete"
            );
        }
        assert!(
            source_topology(
                &sketch,
                NativeCurveSpanSource {
                    span: CurveSpan::line(arc),
                }
            )
            .unwrap()
            .participates_in_trimming(),
            "a directed arc remains trim-capable"
        );

        sketch
            .replace_trim_views(
                CurveSpan::line(circle),
                vec![DocumentCurveTrimView {
                    support: CurveSpan::line(circle),
                    start: DocumentTrimBoundary::Fixed(DocumentTrimParameter {
                        parameter: 0.5,
                        winding: 0,
                    }),
                    end: DocumentTrimBoundary::Fixed(DocumentTrimParameter {
                        parameter: 2.0,
                        winding: 0,
                    }),
                }],
            )
            .unwrap();
        assert!(
            source_topology(
                &sketch,
                NativeCurveSpanSource {
                    span: CurveSpan::line(circle),
                }
            )
            .unwrap()
            .participates_in_trimming(),
            "an explicitly open view of periodic support remains trim-capable"
        );
    }

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
                ArcBranchValidation::PersistedCell,
            ),
            Err(ArcValidationFailure::SingularParents)
        ));
    }
}
