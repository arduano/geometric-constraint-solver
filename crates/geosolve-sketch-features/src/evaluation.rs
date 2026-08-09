// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::{BTreeMap, BTreeSet};

use geosolve_sketch::{
    ContactDomain, ContactNeighborhood, CurveDefinition, CurveSpan, DocumentArcSweep,
    DocumentCurveNormalSide, DocumentFilletEndpointOrder, DocumentFilletTrimEndpoint,
    DocumentTrimBoundary, DocumentTrimParameter, GeometryRole, OperationCheckpoint,
    OperationControl, OperationController, OperationOutcome, OperationWorkCounter,
    PreparedSketchInput, RetainedSketchDocumentSession, SketchAcceptedStateIdentity,
    SketchDocument,
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
        let result = evaluate_snapshot(&self.snapshot, self.evaluation, controller)?;
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
    construction_fragments: Vec<ComputedConstructionFragment>,
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
    participates_in_trimming: bool,
}

#[derive(Clone, Debug)]
struct SourceComposition {
    source: NativeCurveSpanSource,
    base_interval: ComputedSourceInterval,
    start: Option<EndpointClaim>,
    end: Option<EndpointClaim>,
}

#[derive(Clone, Copy, Debug)]
struct DiscardedSourceComplement {
    interval: ComputedSourceInterval,
    claim: EndpointClaim,
}

#[derive(Clone, Debug)]
struct ComposedSourceOutput {
    effective_interval: ComputedSourceInterval,
    discarded: Vec<DiscardedSourceComplement>,
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
    let mut construction_fragments = Vec::new();
    let mut replaced_sources = Vec::new();
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
        construction_fragments,
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
        construction_fragments: Vec::new(),
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
    let sides = [corner.first.normal_side, corner.second.normal_side];
    let exact = exact_seed_solution(sketch, &root_parents, sides, radius).map_err(|failure| {
        EvaluateFeatureError::Failure(match failure {
            OffsetGeometryFailure::OffsetSingularity => {
                ComputedFeatureFailure::OffsetSingularity { corner: corner.id }
            }
            OffsetGeometryFailure::Invalid => {
                ComputedFeatureFailure::InvalidParentState { corner: corner.id }
            }
        })
    })?;
    let (root_parents, solution) = if let Some(solution) = exact {
        (root_parents, solution)
    } else {
        // A source-tangent branch cell can still contain an offset cusp where
        // `1 - side * radius * curvature` changes sign. Correct non-affine
        // persisted seeds only inside a bounded neighbourhood and reject
        // multiple genuine roots; evaluation must never use a remote
        // whole-cell root as a silent repair after a source edit or import.
        let root_parents = current_branch_root_parents(&root_parents, affine).ok_or({
            EvaluateFeatureError::Failure(ComputedFeatureFailure::InvalidParentState {
                corner: corner.id,
            })
        })?;
        let (solutions, root_failure) =
            match local_fillet_roots(sketch, &root_parents, sides, radius, policy, controller) {
                RootSearchResult::Completed { solutions, failure } => (solutions, failure),
                RootSearchResult::Stopped => return Err(EvaluateFeatureError::Stopped),
            };
        let solution =
            select_seed_connected_solution(sketch, &root_parents, &solutions).map_err(|kind| {
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
        (root_parents, solution)
    };
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
        endpoint_claim(
            owner,
            corner.first,
            solution.parameters[0],
            root_parents[0].topology,
        ),
        endpoint_claim(
            owner,
            corner.second,
            solution.parameters[1],
            root_parents[1].topology,
        ),
    ];
    Ok(EvaluatedCorner {
        owner,
        role: combined_source_role(sketch, parents),
        arc,
        claims,
    })
}

fn endpoint_claim(
    owner: ComputedCornerRef,
    parent: ComputedFilletParent,
    parameter: f64,
    topology: SourceTopology,
) -> EndpointClaim {
    EndpointClaim {
        owner,
        source: parent.source,
        endpoint: parent.retained_endpoint,
        parameter,
        base_interval: topology.base_interval,
        participates_in_trimming: topology.participates_in_trimming(),
    }
}

fn combined_source_role(
    sketch: &SketchDocument,
    parents: [ComputedFilletParent; 2],
) -> GeometryRole {
    if parents.into_iter().any(|parent| {
        sketch.geometry_role(parent.source.span.curve) == Some(GeometryRole::Construction)
    }) {
        GeometryRole::Construction
    } else {
        GeometryRole::Profile
    }
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
            if claim.participates_in_trimming {
                claims.entry(claim.source).or_default().push(claim);
            }
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
        .filter(|claim| claim.participates_in_trimming)
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

fn compose_source_output(
    composition: &SourceComposition,
) -> Result<ComposedSourceOutput, ComputedFeatureEvaluationError> {
    let effective_interval = ComputedSourceInterval {
        start: composition
            .start
            .map_or(composition.base_interval.start, |claim| claim.parameter),
        end: composition
            .end
            .map_or(composition.base_interval.end, |claim| claim.parameter),
    };
    let mut discarded = Vec::with_capacity(2);
    if let Some(claim) = composition.start {
        let interval = ComputedSourceInterval {
            start: composition.base_interval.start,
            end: claim.parameter,
        };
        if material_interval(interval, composition.base_interval) {
            discarded.push(DiscardedSourceComplement { interval, claim });
        }
    }
    if let Some(claim) = composition.end {
        let interval = ComputedSourceInterval {
            start: claim.parameter,
            end: composition.base_interval.end,
        };
        if material_interval(interval, composition.base_interval) {
            discarded.push(DiscardedSourceComplement { interval, claim });
        }
    }
    let output = ComposedSourceOutput {
        effective_interval,
        discarded,
    };
    validate_composed_source_output(composition, &output)?;
    Ok(output)
}

fn validate_composed_source_output(
    composition: &SourceComposition,
    output: &ComposedSourceOutput,
) -> Result<(), ComputedFeatureEvaluationError> {
    let invalid = || ComputedFeatureEvaluationError::InvalidGeneratedTopology {
        resource: "source composition",
    };
    if !strict_finite_interval(composition.base_interval)
        || !strict_finite_interval(output.effective_interval)
    {
        return Err(invalid());
    }
    let start_discarded = composition.start.map(|claim| ComputedSourceInterval {
        start: composition.base_interval.start,
        end: claim.parameter,
    });
    let end_discarded = composition.end.map(|claim| ComputedSourceInterval {
        start: claim.parameter,
        end: composition.base_interval.end,
    });
    let expected_count = usize::from(
        start_discarded
            .is_some_and(|interval| material_interval(interval, composition.base_interval)),
    ) + usize::from(
        end_discarded
            .is_some_and(|interval| material_interval(interval, composition.base_interval)),
    );
    if output.discarded.len() != expected_count {
        return Err(invalid());
    }

    let mut discarded_index = 0;
    if let Some(claim) = composition.start {
        if claim.source != composition.source
            || claim.endpoint != DocumentFilletTrimEndpoint::Start
            || !same_interval(claim.base_interval, composition.base_interval)
            || !same_parameter(output.effective_interval.start, claim.parameter)
        {
            return Err(invalid());
        }
        if let Some(interval) = start_discarded
            && material_interval(interval, composition.base_interval)
        {
            let complement = output.discarded.get(discarded_index).ok_or_else(invalid)?;
            discarded_index += 1;
            if !same_parameter(complement.interval.start, interval.start)
                || !same_parameter(complement.interval.end, interval.end)
                || !same_claim(complement.claim, claim)
            {
                return Err(invalid());
            }
        }
    } else if !same_parameter(
        output.effective_interval.start,
        composition.base_interval.start,
    ) {
        return Err(invalid());
    }

    if let Some(claim) = composition.end {
        if claim.source != composition.source
            || claim.endpoint != DocumentFilletTrimEndpoint::End
            || !same_interval(claim.base_interval, composition.base_interval)
            || !same_parameter(output.effective_interval.end, claim.parameter)
        {
            return Err(invalid());
        }
        if let Some(interval) = end_discarded
            && material_interval(interval, composition.base_interval)
        {
            let complement = output.discarded.get(discarded_index).ok_or_else(invalid)?;
            discarded_index += 1;
            if !same_parameter(complement.interval.start, interval.start)
                || !same_parameter(complement.interval.end, interval.end)
                || !same_claim(complement.claim, claim)
            {
                return Err(invalid());
            }
        }
    } else if !same_parameter(output.effective_interval.end, composition.base_interval.end) {
        return Err(invalid());
    }
    if discarded_index != output.discarded.len() {
        return Err(invalid());
    }

    // The theoretical complements share exact boundaries with the retained
    // interval. Every material complement that is actually published must
    // remain strictly disjoint from that interval and from its sibling.
    if output.discarded.iter().any(|complement| {
        complement.interval.start < output.effective_interval.end
            && output.effective_interval.start < complement.interval.end
    }) {
        return Err(invalid());
    }
    Ok(())
}

fn same_claim(first: EndpointClaim, second: EndpointClaim) -> bool {
    first.owner == second.owner
        && first.source == second.source
        && first.endpoint == second.endpoint
        && same_parameter(first.parameter, second.parameter)
        && same_interval(first.base_interval, second.base_interval)
        && first.participates_in_trimming == second.participates_in_trimming
}

fn same_interval(first: ComputedSourceInterval, second: ComputedSourceInterval) -> bool {
    same_parameter(first.start, second.start) && same_parameter(first.end, second.end)
}

fn same_parameter(first: f64, second: f64) -> bool {
    first.to_bits() == second.to_bits()
}

fn material_interval(
    interval: ComputedSourceInterval,
    base_interval: ComputedSourceInterval,
) -> bool {
    strict_finite_interval(interval)
        && interval.end - interval.start > parameter_tolerance(base_interval)
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

fn construction_fragment_id(
    evaluation: ComputedEvaluationRevision,
    index: usize,
) -> Result<ComputedConstructionFragmentId, ComputedFeatureEvaluationError> {
    Ok(ComputedConstructionFragmentId {
        evaluation,
        ordinal: u32::try_from(index).map_err(|_| {
            ComputedFeatureEvaluationError::PolicyLimitExceeded {
                resource: "construction fragment ordinals",
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
        | ComputedFeatureFailure::ConsumedSourceInterval { .. } => {
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
            ),
            Err(ArcValidationFailure::SingularParents)
        ));
    }
}
