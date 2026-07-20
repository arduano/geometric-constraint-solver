// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::BTreeSet;
use std::num::FpCategory;

use geosolve_core::{
    AdaptiveStepController, AdaptiveStepDecision, AdaptiveStepPolicy, ContinuationError,
    ContinuationTangent, ContinuationTangentOrientation, InitialParameterDirection,
    LinearSolveBackend, PseudoArclengthVariable, SolveSession, SourceConstraint,
    SparseFallbackReason, VariableKind, VariableValue,
};
use nalgebra::DVector;

use super::{
    CompiledSpatialAssembly, CoreError, ORIENTATION_BRANCH_MARGIN, ResidualId, SessionError,
    SpatialAssembly, SpatialAssemblyEdit, SpatialAssemblyError, SpatialAssemblySession,
    SpatialAssemblyTransaction, SpatialBodyId, SpatialBoundaryObservation,
    SpatialBoundaryTransition, SpatialBranchBoundaryEvent, SpatialCoordinateId, SpatialGeometry,
    SpatialHingeTarget, SpatialSolveResult, SpatialSolvedBody, SpatialSourceId, SpatialSourceKind,
    SpatialSourceMapping, accepted_coordinate_values, accepted_session,
    apply_spatial_assembly_edit, evaluate_mode_monitors, independent,
    initial_spatial_boundary_evaluations, invalid_field, physical_audit_max,
    physical_domain_residual_max, solved_geometry_from_problem, source_bodies,
    spatial_acceptance_tolerance, update_spatial_boundary_hysteresis, validate_core_acceptance,
    validate_hinge_target, validate_transformed_features, validate_translation_target,
};
use crate::{AdaptiveContinuationMode, ContinuationDirection};

/// One revision-checked adaptive continuation request for a spatial position driver.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpatialAdaptiveContinuationRequest {
    pub driver_source: SpatialSourceId,
    pub mode: AdaptiveContinuationMode,
    pub step_policy: AdaptiveStepPolicy,
}

/// Why spatial continuation stopped after retaining its accepted prefix.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum SpatialAdaptiveContinuationStatus {
    Completed,
    /// Fresh ordinary validation did not reproduce the exact retained entry state.
    InitialRejected,
    /// Natural parameterization reached or attempted to cross a turning point.
    PseudoArclengthRequired,
    /// A predictor or accepted physical endpoint entered a branch-boundary band.
    BranchBoundary(Vec<SpatialBranchBoundaryEvent>),
    /// The corrector left the configured local neighborhood of its predictor.
    CorrectionNotLocal {
        correction: f64,
        limit: f64,
    },
    MinimumStep,
    RetryLimit,
    SampleLimit,
    TangentFailure(ContinuationError),
}

/// One independently validated ordinary fixed-driver spatial sample.
#[derive(Clone, Debug, PartialEq)]
pub struct SpatialAdaptiveContinuationSample {
    pub revision: u64,
    pub driver_target: f64,
    pub path_step: f64,
    pub retries: usize,
    pub corrector_iterations: usize,
    pub corrector_backend: Option<LinearSolveBackend>,
    pub corrector_sparse_fallback_reason: Option<SparseFallbackReason>,
    pub correction_norm: f64,
    pub tangent_parameter_component: f64,
    pub boundary_events: Vec<SpatialBranchBoundaryEvent>,
    pub solve: SpatialSolveResult,
}

/// Accepted-prefix outcome of one spatial adaptive continuation call.
#[derive(Clone, Debug, PartialEq)]
pub struct SpatialAdaptiveContinuationResult {
    pub driver_source: SpatialSourceId,
    pub mode: AdaptiveContinuationMode,
    pub initial_revision: u64,
    pub accepted_revision: u64,
    pub initial_target: f64,
    pub accepted_target: f64,
    pub accepted_path_length: f64,
    pub status: SpatialAdaptiveContinuationStatus,
    /// Fresh ordinary fixed-driver validation of the entry assembly.
    pub initial_solve: SpatialSolveResult,
    pub samples: Vec<SpatialAdaptiveContinuationSample>,
    /// Ordinary physical candidates rejected only by continuation policy.
    pub rejected_attempts: Vec<SpatialSolveResult>,
}

impl SpatialAdaptiveContinuationResult {
    #[must_use]
    pub const fn completed(&self) -> bool {
        matches!(self.status, SpatialAdaptiveContinuationStatus::Completed)
    }
}

#[derive(Clone, Copy, Debug)]
struct SpatialDriverDescriptor {
    coordinate: SpatialCoordinateId,
    target: f64,
    parameter_scale: f64,
    winding: Option<i64>,
}

#[derive(Clone, Copy, Debug)]
struct SpatialBodyTangent {
    body_id: SpatialBodyId,
    normalized: [f64; 6],
    step_scales: [f64; 6],
}

#[derive(Clone, Debug)]
struct SpatialContinuationTangent {
    core: ContinuationTangent,
    bodies: Vec<SpatialBodyTangent>,
}

#[derive(Debug)]
struct SpatialPseudoCandidate {
    bodies: Vec<SpatialSolvedBody>,
    parameter: f64,
    iterations: usize,
    backend: Option<LinearSolveBackend>,
    sparse_fallback_reason: Option<SparseFallbackReason>,
}

impl SpatialAssemblySession {
    /// Continues one hinge or translation position driver from the accepted revision.
    ///
    /// Natural mode stops before a parameter reversal. Pseudo-arclength mode uses
    /// an ephemeral active parameter and control row, then commits only a fresh
    /// ordinary fixed-driver spatial solve after independent physical validation.
    ///
    /// # Errors
    ///
    /// Rejects a stale revision, non-driver source, duplicate driver on the selected
    /// coordinate, invalid target or path request, invalid adaptive policy, or a
    /// domain/core operation that cannot be started. Retried numerical candidates
    /// are represented by the returned status and never replace accepted state.
    #[allow(clippy::manual_let_else, clippy::too_many_lines)]
    pub fn continue_driver(
        &mut self,
        expected_revision: u64,
        request: SpatialAdaptiveContinuationRequest,
    ) -> Result<SpatialAdaptiveContinuationResult, SpatialAssemblyError> {
        self.require_revision(expected_revision)?;
        let descriptor = spatial_driver_descriptor(&self.assembly, request.driver_source)?;
        require_unique_coordinate_driver(&self.assembly, request.driver_source, descriptor)?;
        let initial_target = descriptor.target;
        let initial_direction = match request.mode {
            AdaptiveContinuationMode::Natural { target } => {
                validate_spatial_continuation_target(descriptor, target)?;
                if target >= initial_target {
                    ContinuationDirection::IncreasingParameter
                } else {
                    ContinuationDirection::DecreasingParameter
                }
            }
            AdaptiveContinuationMode::PseudoArclength {
                path_length,
                initial_direction,
            } => {
                if !path_length.is_finite() || path_length <= 0.0 {
                    return invalid_field(
                        "spatial_continuation.path_length",
                        "path length must be positive and finite",
                    );
                }
                initial_direction
            }
        };
        let mut controller = AdaptiveStepController::new(request.step_policy)?;

        // Spatial session fields cannot be mutated without validation, but the
        // continuation contract still requires a fresh ordinary entry solve.
        let entry = Self::new(self.assembly.clone(), self.config)?;
        let entry_descriptor = spatial_driver_descriptor(&entry.assembly, request.driver_source)?;
        if entry_descriptor.target.to_bits() != initial_target.to_bits() {
            return independent(
                "ordinary entry validation changed the selected spatial driver target",
            );
        }
        let initial_solve = entry.accepted_result.clone();
        if entry.assembly != self.assembly {
            return Ok(SpatialAdaptiveContinuationResult {
                driver_source: request.driver_source,
                mode: request.mode,
                initial_revision: expected_revision,
                accepted_revision: self.revision(),
                initial_target,
                accepted_target: initial_target,
                accepted_path_length: 0.0,
                status: SpatialAdaptiveContinuationStatus::InitialRejected,
                initial_solve,
                samples: Vec::new(),
                rejected_attempts: Vec::new(),
            });
        }
        if matches!(
            request.mode,
            AdaptiveContinuationMode::Natural { target }
                if matches!(
                    initial_target.partial_cmp(&target),
                    Some(std::cmp::Ordering::Equal)
                )
        ) {
            return Ok(SpatialAdaptiveContinuationResult {
                driver_source: request.driver_source,
                mode: request.mode,
                initial_revision: expected_revision,
                accepted_revision: self.revision(),
                initial_target,
                accepted_target: initial_target,
                accepted_path_length: 0.0,
                status: SpatialAdaptiveContinuationStatus::Completed,
                initial_solve,
                samples: Vec::new(),
                rejected_attempts: Vec::new(),
            });
        }
        require_unique_spatial_path(self, request.driver_source)?;

        let mut previous_tangent: Option<ContinuationTangent> = None;
        let mut samples = Vec::new();
        let mut rejected_attempts = Vec::new();
        let mut accepted_path_length = 0.0;

        let status = loop {
            let current = spatial_driver_descriptor(&self.assembly, request.driver_source)?;
            let current_target = current.target;
            let remaining_path = match request.mode {
                AdaptiveContinuationMode::Natural { target } => {
                    if matches!(
                        current_target.partial_cmp(&target),
                        Some(std::cmp::Ordering::Equal)
                    ) {
                        break SpatialAdaptiveContinuationStatus::Completed;
                    }
                    f64::INFINITY
                }
                AdaptiveContinuationMode::PseudoArclength { path_length, .. } => {
                    let remaining = path_length - accepted_path_length;
                    if remaining <= 64.0 * f64::EPSILON * path_length {
                        break SpatialAdaptiveContinuationStatus::Completed;
                    }
                    remaining
                }
            };
            if controller.sample_limit_reached() {
                break SpatialAdaptiveContinuationStatus::SampleLimit;
            }
            if self.revision() == u64::MAX {
                return Err(SpatialAssemblyError::RevisionExhausted);
            }

            let orientation = previous_tangent.as_ref().map_or_else(
                || {
                    ContinuationTangentOrientation::Initial(match initial_direction {
                        ContinuationDirection::IncreasingParameter => {
                            InitialParameterDirection::Increasing
                        }
                        ContinuationDirection::DecreasingParameter => {
                            InitialParameterDirection::Decreasing
                        }
                    })
                },
                |previous| ContinuationTangentOrientation::Previous(previous.clone()),
            );
            let tangent = match self.spatial_continuation_tangent(
                request.driver_source,
                current_target,
                &orientation,
            ) {
                Ok(tangent) => tangent,
                Err(SpatialAssemblyError::Continuation(error)) => {
                    break SpatialAdaptiveContinuationStatus::TangentFailure(error);
                }
                Err(error) => return Err(error),
            };
            let parameter_component = tangent.core.parameter_component();
            let mut path_step = controller.current_step().min(remaining_path);

            if let AdaptiveContinuationMode::Natural { target } = request.mode {
                let remaining_parameter = (target - current_target) / current.parameter_scale;
                if !remaining_parameter.is_finite()
                    || parameter_component * remaining_parameter <= 0.0
                    || parameter_component.abs() <= 64.0 * f64::EPSILON
                {
                    break SpatialAdaptiveContinuationStatus::PseudoArclengthRequired;
                }
                path_step = path_step.min((remaining_parameter / parameter_component).abs());
            }
            if !path_step.is_finite() || path_step <= 0.0 {
                break SpatialAdaptiveContinuationStatus::MinimumStep;
            }

            let predicted_bodies = self.predict_spatial_bodies(&tangent, path_step)?;
            let mut predicted_parameter =
                current_target + parameter_component * path_step * current.parameter_scale;
            if let AdaptiveContinuationMode::Natural { target } = request.mode
                && (target - predicted_parameter).signum() != (target - current_target).signum()
            {
                predicted_parameter = target;
            }
            if !spatial_predictor_changed(
                &self.accepted_result.geometry,
                &predicted_bodies,
                current_target,
                predicted_parameter,
            )? {
                break SpatialAdaptiveContinuationStatus::MinimumStep;
            }

            let predictor_events = if let Some(event) =
                self.hinge_principal_cut_crossing_event(current, predicted_parameter)?
            {
                vec![event]
            } else {
                validate_spatial_continuation_target(current, predicted_parameter)?;
                self.predictor_boundary_events(
                    request.driver_source,
                    predicted_parameter,
                    &predicted_bodies,
                )?
            };
            let blocking_predictor_events = predictor_events
                .into_iter()
                .filter(|event| {
                    event.transition == SpatialBoundaryTransition::CrossingAttempted
                        || (event.transition == SpatialBoundaryTransition::Entered
                            && event.clearance <= ORIENTATION_BRANCH_MARGIN)
                })
                .collect::<Vec<_>>();
            if !blocking_predictor_events.is_empty() {
                match controller.reject() {
                    AdaptiveStepDecision::Retry => continue,
                    AdaptiveStepDecision::MinimumStep | AdaptiveStepDecision::RetryLimit => {
                        break SpatialAdaptiveContinuationStatus::BranchBoundary(
                            blocking_predictor_events,
                        );
                    }
                }
            }

            let pseudo = if matches!(
                request.mode,
                AdaptiveContinuationMode::PseudoArclength { .. }
            ) {
                match self.spatial_pseudo_corrector(
                    request.driver_source,
                    current_target,
                    &tangent,
                    path_step,
                    &predicted_bodies,
                    predicted_parameter,
                )? {
                    Some(candidate) => Some(candidate),
                    None => match controller.reject() {
                        AdaptiveStepDecision::Retry => continue,
                        AdaptiveStepDecision::MinimumStep => {
                            break SpatialAdaptiveContinuationStatus::MinimumStep;
                        }
                        AdaptiveStepDecision::RetryLimit => {
                            break SpatialAdaptiveContinuationStatus::RetryLimit;
                        }
                    },
                }
            } else {
                None
            };
            let corrected_bodies = pseudo
                .as_ref()
                .map_or(predicted_bodies.as_slice(), |candidate| {
                    candidate.bodies.as_slice()
                });
            let corrected_parameter = pseudo
                .as_ref()
                .map_or(predicted_parameter, |candidate| candidate.parameter);
            let mut candidate = match self.spatial_physical_trial(
                request.driver_source,
                corrected_parameter,
                corrected_bodies,
            ) {
                Ok(candidate) => candidate,
                Err(error) if retryable_spatial_trial_error(&error) => match controller.reject() {
                    AdaptiveStepDecision::Retry => continue,
                    AdaptiveStepDecision::MinimumStep => {
                        break SpatialAdaptiveContinuationStatus::MinimumStep;
                    }
                    AdaptiveStepDecision::RetryLimit => {
                        break SpatialAdaptiveContinuationStatus::RetryLimit;
                    }
                },
                Err(error) => return Err(error),
            };
            let accepted_parameter =
                spatial_driver_descriptor(&candidate.assembly, request.driver_source)?.target;
            let correction_norm = spatial_correction_norm(
                &predicted_bodies,
                &candidate.accepted_result.geometry,
                predicted_parameter,
                accepted_parameter,
                self.assembly.model_scale,
                current.parameter_scale,
            )?;
            let correction_limit = controller.correction_limit(path_step)?;
            if correction_norm > correction_limit {
                let rejection = SpatialAdaptiveContinuationStatus::CorrectionNotLocal {
                    correction: correction_norm,
                    limit: correction_limit,
                };
                rejected_attempts.push(candidate.accepted_result.clone());
                match controller.reject() {
                    AdaptiveStepDecision::Retry => continue,
                    AdaptiveStepDecision::MinimumStep | AdaptiveStepDecision::RetryLimit => {
                        break rejection;
                    }
                }
            }

            let boundary_events = update_spatial_boundary_hysteresis(
                &self.accepted_result.branch_boundary_evaluations,
                &mut candidate.accepted_result.branch_boundary_evaluations,
                SpatialBoundaryObservation::CorrectedPhysicalEndpoint,
            );
            let entered_boundary_events = boundary_events
                .iter()
                .filter(|event| event.transition == SpatialBoundaryTransition::Entered)
                .cloned()
                .collect::<Vec<_>>();

            if entered_boundary_events.is_empty()
                && matches!(request.mode, AdaptiveContinuationMode::Natural { .. })
            {
                let post_corrector = candidate.spatial_continuation_tangent(
                    request.driver_source,
                    accepted_parameter,
                    &ContinuationTangentOrientation::Previous(tangent.core.clone()),
                );
                match post_corrector {
                    Ok(post_corrector)
                        if post_corrector.core.parameter_component()
                            * match initial_direction {
                                ContinuationDirection::IncreasingParameter => 1.0,
                                ContinuationDirection::DecreasingParameter => -1.0,
                            }
                            > 64.0 * f64::EPSILON => {}
                    Ok(_) => {
                        rejected_attempts.push(candidate.accepted_result.clone());
                        match controller.reject() {
                            AdaptiveStepDecision::Retry => continue,
                            AdaptiveStepDecision::MinimumStep
                            | AdaptiveStepDecision::RetryLimit => {
                                break SpatialAdaptiveContinuationStatus::PseudoArclengthRequired;
                            }
                        }
                    }
                    Err(SpatialAssemblyError::Continuation(error)) => {
                        rejected_attempts.push(candidate.accepted_result.clone());
                        break SpatialAdaptiveContinuationStatus::TangentFailure(error);
                    }
                    Err(error) => return Err(error),
                }
            }

            let pseudo_iterations = pseudo.as_ref().map_or(0, |candidate| candidate.iterations);
            let iterations = pseudo_iterations
                .saturating_add(candidate.scratch_solve.iterations)
                .saturating_add(candidate.accepted_result.core_report.iterations);
            let (corrector_backend, corrector_sparse_fallback_reason) = pseudo.as_ref().map_or(
                (
                    candidate
                        .scratch_solve
                        .backend
                        .or(candidate.accepted_result.core_report.actual_backend),
                    candidate
                        .scratch_solve
                        .sparse_fallback_reason
                        .or(candidate.accepted_result.core_report.sparse_fallback_reason),
                ),
                |candidate| (candidate.backend, candidate.sparse_fallback_reason),
            );
            let retries = controller.retries();
            let next_path_length = accepted_path_length + path_step;
            if !next_path_length.is_finite() {
                return independent("accepted spatial continuation path length is non-finite");
            }
            let sample = SpatialAdaptiveContinuationSample {
                revision: candidate.revision(),
                driver_target: accepted_parameter,
                path_step,
                retries,
                corrector_iterations: iterations,
                corrector_backend,
                corrector_sparse_fallback_reason,
                correction_norm,
                tangent_parameter_component: parameter_component,
                boundary_events,
                solve: candidate.accepted_result.clone(),
            };
            controller.accept(iterations, correction_norm)?;
            accepted_path_length = next_path_length;
            previous_tangent = Some(tangent.core);
            *self = candidate;
            samples.push(sample);
            if !entered_boundary_events.is_empty() {
                break SpatialAdaptiveContinuationStatus::BranchBoundary(entered_boundary_events);
            }
        };

        let accepted_target =
            spatial_driver_descriptor(&self.assembly, request.driver_source)?.target;
        Ok(SpatialAdaptiveContinuationResult {
            driver_source: request.driver_source,
            mode: request.mode,
            initial_revision: expected_revision,
            accepted_revision: self.revision(),
            initial_target,
            accepted_target,
            accepted_path_length,
            status,
            initial_solve,
            samples,
            rejected_attempts,
        })
    }

    #[allow(clippy::too_many_lines)]
    fn spatial_continuation_tangent(
        &self,
        driver: SpatialSourceId,
        parameter: f64,
        orientation: &ContinuationTangentOrientation,
    ) -> Result<SpatialContinuationTangent, SpatialAssemblyError> {
        let mut ordinary = self.assembly.compile_validated()?;
        self.add_continuation_gauges(&mut ordinary)?;
        let ordinary_mapping = ordinary
            .source_mapping(driver)
            .ok_or(SpatialAssemblyError::UnknownSource(driver))?;
        let ordinary_residual = require_single_driver_residual(&ordinary, ordinary_mapping)?;
        let ordinary_session = accepted_session(
            ordinary.problem.clone(),
            self.config,
            "spatial continuation tangent solve",
        )?;
        require_spatial_snapshot(&ordinary, &ordinary_session, &self.accepted_result.geometry)?;
        let ordinary_linearization = ordinary_session.accepted_hard_linearization()?;
        let ordinary_component = ordinary_linearization
            .components()
            .iter()
            .find(|component| {
                component
                    .hard_rows()
                    .iter()
                    .any(|row| row.row.residual_id == ordinary_residual)
            })
            .ok_or_else(|| {
                SpatialAssemblyError::IndependentValidation(
                    "selected spatial driver row is absent from accepted hard linearization"
                        .to_owned(),
                )
            })?;
        let ordinary_row = ordinary_component
            .hard_rows()
            .iter()
            .position(|row| row.row.residual_id == ordinary_residual)
            .ok_or_else(|| {
                SpatialAssemblyError::IndependentValidation(
                    "selected spatial driver row is absent from its hard component".to_owned(),
                )
            })?;

        let (mut parameterized, parameter_variable) = self
            .assembly
            .compile_with_parameterized_driver(driver, parameter)?;
        self.add_continuation_gauges(&mut parameterized)?;
        let parameterized_mapping = parameterized
            .source_mapping(driver)
            .ok_or(SpatialAssemblyError::UnknownSource(driver))?;
        let parameterized_residual =
            require_single_driver_residual(&parameterized, parameterized_mapping)?;
        let parameterized_session = accepted_session(
            parameterized.problem.clone(),
            self.config,
            "parameterized spatial continuation tangent solve",
        )?;
        require_spatial_snapshot(
            &parameterized,
            &parameterized_session,
            &self.accepted_result.geometry,
        )?;
        let VariableValue::Scalar(actual_parameter) = parameterized_session
            .problem()
            .variable(parameter_variable)
            .ok_or(CoreError::UnknownVariable(parameter_variable))?
            .value()
        else {
            return independent("spatial continuation parameter changed variable kind");
        };
        if actual_parameter.to_bits() != parameter.to_bits() {
            return independent("private tangent solve changed the spatial driver parameter");
        }
        let parameterized_linearization = parameterized_session.accepted_hard_linearization()?;
        let parameterized_component = parameterized_linearization
            .components()
            .iter()
            .find(|component| {
                component
                    .hard_rows()
                    .iter()
                    .any(|row| row.row.residual_id == parameterized_residual)
            })
            .ok_or_else(|| {
                SpatialAssemblyError::IndependentValidation(
                    "parameterized spatial driver row is absent from accepted hard linearization"
                        .to_owned(),
                )
            })?;
        let parameterized_row = parameterized_component
            .hard_rows()
            .iter()
            .position(|row| row.row.residual_id == parameterized_residual)
            .ok_or_else(|| {
                SpatialAssemblyError::IndependentValidation(
                    "parameterized spatial driver row is absent from its hard component".to_owned(),
                )
            })?;
        let parameter_block = parameterized_component
            .tangent_blocks()
            .iter()
            .find(|block| block.root == parameter_variable)
            .ok_or_else(|| {
                SpatialAssemblyError::IndependentValidation(
                    "spatial continuation parameter is absent from its tangent component"
                        .to_owned(),
                )
            })?;
        if parameter_block.kind != VariableKind::Scalar || parameter_block.tangent_range.len() != 1
        {
            return independent("spatial continuation parameter has malformed tangent metadata");
        }
        let parameter_derivative = parameterized_component.normalized_jacobian()
            [(parameterized_row, parameter_block.tangent_range.start)];
        if !parameter_derivative.is_finite() {
            return independent("spatial continuation parameter derivative is non-finite");
        }
        let mut parameter_column = DVector::zeros(ordinary_component.hard_rows().len());
        parameter_column[ordinary_row] = parameter_derivative;
        let tangent = ordinary_component
            .augmented_unit_null_tangent(&parameter_column, orientation)
            .map_err(SpatialAssemblyError::Continuation)?;

        let mut bodies = Vec::new();
        for block in ordinary_component.tangent_blocks() {
            if block.kind != VariableKind::Pose3
                || block.tangent_range.len() != 6
                || block.step_scales.len() != 6
            {
                return independent(
                    "spatial continuation component contains a non-Pose3 tangent block",
                );
            }
            let mapping = ordinary
                .body_variables
                .iter()
                .find(|mapping| mapping.variable_id == block.root)
                .ok_or_else(|| {
                    SpatialAssemblyError::IndependentValidation(
                        "spatial continuation tangent block is not an assembly body".to_owned(),
                    )
                })?;
            let start = block.tangent_range.start;
            bodies.push(SpatialBodyTangent {
                body_id: mapping.body_id,
                normalized: std::array::from_fn(|index| tangent.normalized_state()[start + index]),
                step_scales: std::array::from_fn(|index| block.step_scales[index]),
            });
        }
        Ok(SpatialContinuationTangent {
            core: tangent,
            bodies,
        })
    }

    fn add_continuation_gauges(
        &self,
        compiled: &mut CompiledSpatialAssembly,
    ) -> Result<(), SpatialAssemblyError> {
        for reference in self
            .gauge_report
            .components
            .iter()
            .filter_map(|component| component.numerical_reference)
        {
            let accepted = self
                .accepted_result
                .geometry
                .body_pose(reference.body)
                .ok_or(SpatialAssemblyError::UnknownBody(reference.body))?;
            compiled.add_numerical_pose_gauge(
                reference.body,
                accepted,
                self.assembly.model_scale,
            )?;
        }
        Ok(())
    }

    fn predict_spatial_bodies(
        &self,
        tangent: &SpatialContinuationTangent,
        path_step: f64,
    ) -> Result<Vec<SpatialSolvedBody>, SpatialAssemblyError> {
        self.assembly
            .bodies
            .iter()
            .map(|body| {
                let pose = if let Some(block) =
                    tangent.bodies.iter().find(|block| block.body_id == body.id)
                {
                    body.pose_guess.retract(std::array::from_fn(|index| {
                        block.normalized[index] * block.step_scales[index] * path_step
                    }))?
                } else {
                    body.pose_guess
                };
                Ok(SpatialSolvedBody {
                    body_id: body.id,
                    pose,
                })
            })
            .collect()
    }

    fn predictor_boundary_events(
        &self,
        driver: SpatialSourceId,
        predicted_parameter: f64,
        predicted: &[SpatialSolvedBody],
    ) -> Result<Vec<SpatialBranchBoundaryEvent>, SpatialAssemblyError> {
        let mut predicted_assembly = self.assembly.clone();
        let driver_edit = spatial_driver_edit(&predicted_assembly, driver, predicted_parameter)?;
        apply_spatial_assembly_edit(&mut predicted_assembly, driver_edit)?;
        let mut compiled = predicted_assembly.compile_validated()?;
        for body in predicted {
            let variable = compiled
                .variable_for_body(body.body_id)
                .ok_or(SpatialAssemblyError::UnknownBody(body.body_id))?;
            compiled
                .problem
                .set_variable_value(variable, VariableValue::Pose3(body.pose.ambient()))?;
        }
        let geometry = solved_geometry_from_problem(
            &compiled.problem,
            &compiled.body_variables,
            &compiled.point_features,
            &compiled.frame_features,
            &compiled.axis_features,
            &compiled.plane_features,
        )?;
        validate_transformed_features(&predicted_assembly, &geometry)?;
        let mut evaluations = initial_spatial_boundary_evaluations(&predicted_assembly, &geometry)?;
        let events = update_spatial_boundary_hysteresis(
            &self.accepted_result.branch_boundary_evaluations,
            &mut evaluations,
            SpatialBoundaryObservation::PredictorEndpoint,
        );
        if events.iter().any(|event| {
            event.transition == SpatialBoundaryTransition::Entered
                && event.clearance <= ORIENTATION_BRANCH_MARGIN
        }) {
            return Ok(events);
        }
        let coordinate_values = accepted_coordinate_values(&predicted_assembly, &geometry)?;
        physical_domain_residual_max(&predicted_assembly, &geometry, &coordinate_values)?;
        evaluate_mode_monitors(&predicted_assembly, &geometry, &coordinate_values)?;
        Ok(events)
    }

    fn hinge_principal_cut_crossing_event(
        &self,
        descriptor: SpatialDriverDescriptor,
        predicted_parameter: f64,
    ) -> Result<Option<SpatialBranchBoundaryEvent>, SpatialAssemblyError> {
        let Some(winding) = descriptor.winding else {
            return Ok(None);
        };
        if (-std::f64::consts::PI..std::f64::consts::PI).contains(&predicted_parameter) {
            return Ok(None);
        }
        let boundary = super::SpatialBranchBoundary::HingePrincipalCut {
            coordinate: descriptor.coordinate,
            winding,
        };
        let previous = self
            .accepted_result
            .branch_boundary_evaluations
            .iter()
            .find(|evaluation| evaluation.boundary == boundary)
            .ok_or_else(|| {
                SpatialAssemblyError::IndependentValidation(format!(
                    "selected hinge driver boundary {boundary:?} is absent"
                ))
            })?;
        Ok(Some(SpatialBranchBoundaryEvent {
            boundary,
            transition: SpatialBoundaryTransition::CrossingAttempted,
            observation: SpatialBoundaryObservation::PredictorEndpoint,
            previous_clearance: previous.clearance,
            clearance: 0.0,
            raw_metric: if predicted_parameter >= std::f64::consts::PI {
                std::f64::consts::PI
            } else {
                -std::f64::consts::PI
            },
        }))
    }

    #[allow(clippy::too_many_arguments, clippy::too_many_lines)]
    fn spatial_pseudo_corrector(
        &self,
        driver: SpatialSourceId,
        reference_parameter: f64,
        tangent: &SpatialContinuationTangent,
        path_step: f64,
        predicted_bodies: &[SpatialSolvedBody],
        predicted_parameter: f64,
    ) -> Result<Option<SpatialPseudoCandidate>, SpatialAssemblyError> {
        let (mut compiled, parameter_variable) = self
            .assembly
            .compile_with_parameterized_driver(driver, predicted_parameter)?;
        for body in predicted_bodies {
            let variable = compiled
                .variable_for_body(body.body_id)
                .ok_or(SpatialAssemblyError::UnknownBody(body.body_id))?;
            compiled
                .problem
                .set_variable_value(variable, VariableValue::Pose3(body.pose.ambient()))?;
        }
        compiled.problem.set_variable_value(
            parameter_variable,
            VariableValue::Scalar(predicted_parameter),
        )?;
        self.add_continuation_gauges(&mut compiled)?;

        let mut control_variables = Vec::with_capacity(tangent.bodies.len() + 1);
        for block in &tangent.bodies {
            let variable = compiled
                .variable_for_body(block.body_id)
                .ok_or(SpatialAssemblyError::UnknownBody(block.body_id))?;
            let reference = self
                .accepted_result
                .geometry
                .body_pose(block.body_id)
                .ok_or(SpatialAssemblyError::UnknownBody(block.body_id))?;
            control_variables.push(PseudoArclengthVariable::new(
                variable,
                VariableValue::Pose3(reference.ambient()),
                block.normalized.to_vec(),
            )?);
        }
        control_variables.push(PseudoArclengthVariable::new(
            parameter_variable,
            VariableValue::Scalar(reference_parameter),
            vec![tangent.core.parameter_component()],
        )?);
        let source = compiled.problem.add_source(SourceConstraint::new(
            "ephemeral spatial pseudo-arclength control",
        )?);
        compiled
            .problem
            .add_pseudo_arclength(source, &control_variables, path_step)?;

        let CompiledSpatialAssembly {
            problem,
            body_variables,
            source_mappings,
            point_features,
            frame_features,
            axis_features,
            plane_features,
        } = compiled;
        let session = match SolveSession::new(problem, self.config) {
            Ok(session) => session,
            Err(SessionError::InitialRejected(_)) => return Ok(None),
            Err(error) => return Err(SpatialAssemblyError::Session(error)),
        };
        let geometry = solved_geometry_from_problem(
            session.problem(),
            &body_variables,
            &point_features,
            &frame_features,
            &axis_features,
            &plane_features,
        )?;
        let VariableValue::Scalar(parameter) = session
            .problem()
            .variable(parameter_variable)
            .ok_or(CoreError::UnknownVariable(parameter_variable))?
            .value()
        else {
            return independent("pseudo-arclength spatial parameter changed variable kind");
        };
        if !parameter.is_finite() {
            return Ok(None);
        }
        let mut physical_assembly = self.assembly.clone();
        let driver_edit = spatial_driver_edit(&physical_assembly, driver, parameter)?;
        apply_spatial_assembly_edit(&mut physical_assembly, driver_edit)?;
        let coordinate_values = match accepted_coordinate_values(&physical_assembly, &geometry) {
            Ok(values) => values,
            Err(SpatialAssemblyError::IndependentValidation(_)) => return Ok(None),
            Err(error) => return Err(error),
        };
        let tolerance = spatial_acceptance_tolerance(self.config);
        if validate_core_acceptance(session.report(), tolerance).is_err() {
            return Ok(None);
        }
        let core_max = physical_audit_max(&session, &source_mappings, tolerance)?;
        validate_transformed_features(&physical_assembly, &geometry)?;
        let domain_max =
            match physical_domain_residual_max(&physical_assembly, &geometry, &coordinate_values) {
                Ok(maximum) => maximum,
                Err(SpatialAssemblyError::IndependentValidation(_)) => return Ok(None),
                Err(error) => return Err(error),
            };
        if core_max.max(domain_max) > tolerance {
            return Ok(None);
        }
        if let Err(error) =
            evaluate_mode_monitors(&physical_assembly, &geometry, &coordinate_values)
        {
            return match error {
                SpatialAssemblyError::IndependentValidation(_) => Ok(None),
                error => Err(error),
            };
        }
        Ok(Some(SpatialPseudoCandidate {
            bodies: geometry.bodies,
            parameter,
            iterations: session.report().iterations,
            backend: session.report().actual_backend,
            sparse_fallback_reason: session.report().sparse_fallback_reason,
        }))
    }

    fn spatial_physical_trial(
        &self,
        driver: SpatialSourceId,
        target: f64,
        bodies: &[SpatialSolvedBody],
    ) -> Result<Self, SpatialAssemblyError> {
        let mut candidate = self.clone();
        let mut edits = bodies
            .iter()
            .map(|body| SpatialAssemblyEdit::BodyPoseGuess {
                body: body.body_id,
                pose: body.pose,
            })
            .collect::<Vec<_>>();
        edits.push(spatial_driver_edit(&candidate.assembly, driver, target)?);
        candidate
            .apply_transaction(SpatialAssemblyTransaction::new(candidate.revision(), edits))?;
        Ok(candidate)
    }
}

fn spatial_driver_descriptor(
    assembly: &SpatialAssembly,
    source: SpatialSourceId,
) -> Result<SpatialDriverDescriptor, SpatialAssemblyError> {
    match assembly.require_source(source)?.kind {
        SpatialSourceKind::HingePositionDriver { coordinate, target } => {
            Ok(SpatialDriverDescriptor {
                coordinate,
                target: target.principal_phase,
                parameter_scale: 1.0,
                winding: Some(target.winding),
            })
        }
        SpatialSourceKind::TranslationPositionDriver { coordinate, target } => {
            Ok(SpatialDriverDescriptor {
                coordinate,
                target,
                parameter_scale: assembly.model_scale,
                winding: None,
            })
        }
        _ => Err(SpatialAssemblyError::WrongSourceKind {
            source_id: source,
            expected: "a position driver",
        }),
    }
}

fn validate_spatial_continuation_target(
    descriptor: SpatialDriverDescriptor,
    target: f64,
) -> Result<(), SpatialAssemblyError> {
    if let Some(winding) = descriptor.winding {
        validate_hinge_target(SpatialHingeTarget {
            principal_phase: target,
            winding,
        })
    } else {
        validate_translation_target(target)
    }
}

fn require_unique_coordinate_driver(
    assembly: &SpatialAssembly,
    selected: SpatialSourceId,
    descriptor: SpatialDriverDescriptor,
) -> Result<(), SpatialAssemblyError> {
    let count = assembly
        .sources
        .iter()
        .filter(|source| match source.kind {
            SpatialSourceKind::HingePositionDriver { coordinate, .. }
            | SpatialSourceKind::TranslationPositionDriver { coordinate, .. } => {
                coordinate == descriptor.coordinate
            }
            _ => false,
        })
        .count();
    if count == 1 {
        Ok(())
    } else {
        invalid_field(
            "spatial_continuation.driver_source",
            format!(
                "selected source {selected} shares coordinate {} with another hard driver",
                descriptor.coordinate
            ),
        )
    }
}

fn require_unique_spatial_path(
    session: &SpatialAssemblySession,
    driver: SpatialSourceId,
) -> Result<(), SpatialAssemblyError> {
    let component = session
        .gauge_report
        .components
        .iter()
        .find(|component| component.sources.contains(&driver))
        .ok_or_else(|| {
            SpatialAssemblyError::GaugeCertification(format!(
                "selected continuation driver {driver} has no certified spatial component"
            ))
        })?;
    let mapping = session
        .source_mappings
        .iter()
        .find(|mapping| mapping.source == driver)
        .ok_or(SpatialAssemblyError::UnknownSource(driver))?;
    let [driver_residual] = mapping.residual_ids.as_slice() else {
        return independent("selected spatial driver must map to exactly one residual block");
    };
    let accepted = session.core_session.accepted_hard_linearization()?;
    let selected = accepted
        .components()
        .iter()
        .find(|candidate| {
            candidate
                .hard_rows()
                .iter()
                .any(|row| row.row.residual_id == *driver_residual)
        })
        .ok_or_else(|| {
            SpatialAssemblyError::GaugeCertification(format!(
                "selected continuation driver {driver} has no accepted hard component"
            ))
        })?;
    let selected_gauge = if physical_source_component_is_grounded(&session.assembly, driver)? {
        0
    } else {
        6
    };
    if selected.right_nullity() < selected_gauge {
        return Err(SpatialAssemblyError::GaugeCertification(format!(
            "selected continuation component right nullity {} is below its certified gauge DOF {selected_gauge}",
            selected.right_nullity()
        )));
    }
    let selected_internal_mobility = selected.right_nullity() - selected_gauge;
    let external_internal_mobility = component
        .internal_mobility
        .checked_sub(selected_internal_mobility)
        .ok_or_else(|| {
            SpatialAssemblyError::GaugeCertification(format!(
                "selected continuation component internal mobility {selected_internal_mobility} exceeds certified domain internal mobility {}",
                component.internal_mobility
            ))
        })?;
    if external_internal_mobility == 0 {
        Ok(())
    } else {
        invalid_field(
            "spatial_continuation.driver_source",
            format!(
                "selected driver's accepted domain component has {external_internal_mobility} internal mobility outside its physical hard component; continuation requires a unique one-dimensional released-driver path",
            ),
        )
    }
}

fn physical_source_component_is_grounded(
    assembly: &SpatialAssembly,
    driver: SpatialSourceId,
) -> Result<bool, SpatialAssemblyError> {
    let source = assembly.require_source(driver)?;
    let mut connected = source_bodies(assembly, source)?
        .into_iter()
        .collect::<BTreeSet<_>>();
    loop {
        let before = connected.len();
        for source in &assembly.sources {
            let incident = source_bodies(assembly, source)?;
            if incident.iter().any(|body| connected.contains(body)) {
                connected.extend(incident);
            }
        }
        if connected.len() == before {
            break;
        }
    }
    Ok(assembly.sources.iter().any(|source| {
        matches!(
            source.kind,
            SpatialSourceKind::PhysicalGround { body, .. } if connected.contains(&body)
        )
    }))
}

fn spatial_driver_edit(
    assembly: &SpatialAssembly,
    source: SpatialSourceId,
    target: f64,
) -> Result<SpatialAssemblyEdit, SpatialAssemblyError> {
    let descriptor = spatial_driver_descriptor(assembly, source)?;
    validate_spatial_continuation_target(descriptor, target)?;
    Ok(if let Some(winding) = descriptor.winding {
        SpatialAssemblyEdit::HingeDriverTarget {
            source,
            target: SpatialHingeTarget {
                principal_phase: target,
                winding,
            },
        }
    } else {
        SpatialAssemblyEdit::TranslationDriverTarget { source, target }
    })
}

fn require_single_driver_residual(
    compiled: &CompiledSpatialAssembly,
    mapping: &SpatialSourceMapping,
) -> Result<ResidualId, SpatialAssemblyError> {
    let [residual] = mapping.residual_ids.as_slice() else {
        return independent("selected spatial driver must map to exactly one residual block");
    };
    let block = compiled
        .problem
        .residual(*residual)
        .ok_or(CoreError::UnknownResidual(*residual))?;
    if block.output_dimension() != 1 {
        return independent("selected spatial driver residual must contain exactly one row");
    }
    Ok(*residual)
}

fn require_spatial_snapshot(
    compiled: &CompiledSpatialAssembly,
    session: &SolveSession,
    accepted: &SpatialGeometry,
) -> Result<(), SpatialAssemblyError> {
    for mapping in &compiled.body_variables {
        let expected = accepted
            .body_pose(mapping.body_id)
            .ok_or(SpatialAssemblyError::UnknownBody(mapping.body_id))?
            .ambient();
        let VariableValue::Pose3(actual) = session
            .problem()
            .variable(mapping.variable_id)
            .ok_or(CoreError::UnknownVariable(mapping.variable_id))?
            .value()
        else {
            return independent("private spatial tangent solve changed a body variable kind");
        };
        if actual
            .iter()
            .zip(expected)
            .any(|(actual, expected)| actual.to_bits() != expected.to_bits())
        {
            return independent(
                "private spatial tangent solve diverged from the accepted assembly state",
            );
        }
    }
    Ok(())
}

fn spatial_correction_norm(
    predicted: &[SpatialSolvedBody],
    solved: &SpatialGeometry,
    predicted_parameter: f64,
    solved_parameter: f64,
    model_scale: f64,
    parameter_scale: f64,
) -> Result<f64, SpatialAssemblyError> {
    let mut norm = ((solved_parameter - predicted_parameter) / parameter_scale).abs();
    for predicted_body in predicted {
        let solved_pose = solved
            .body_pose(predicted_body.body_id)
            .ok_or(SpatialAssemblyError::UnknownBody(predicted_body.body_id))?;
        let difference = predicted_body.pose.local_difference(&solved_pose)?;
        for value in [
            difference[0] / model_scale,
            difference[1] / model_scale,
            difference[2] / model_scale,
            difference[3],
            difference[4],
            difference[5],
        ] {
            norm = norm.hypot(value);
        }
    }
    if norm.is_finite() {
        Ok(norm)
    } else {
        independent("spatial continuation correction norm is non-finite")
    }
}

fn spatial_predictor_changed(
    accepted: &SpatialGeometry,
    predicted: &[SpatialSolvedBody],
    accepted_parameter: f64,
    predicted_parameter: f64,
) -> Result<bool, SpatialAssemblyError> {
    if !matches!(
        accepted_parameter.partial_cmp(&predicted_parameter),
        Some(std::cmp::Ordering::Equal)
    ) {
        return Ok(true);
    }
    for body in predicted {
        let accepted_pose = accepted
            .body_pose(body.body_id)
            .ok_or(SpatialAssemblyError::UnknownBody(body.body_id))?;
        let difference = accepted_pose.local_difference(&body.pose)?;
        if difference
            .iter()
            .any(|value| value.classify() != FpCategory::Zero)
        {
            return Ok(true);
        }
    }
    Ok(false)
}

fn retryable_spatial_trial_error(error: &SpatialAssemblyError) -> bool {
    matches!(error, SpatialAssemblyError::InitialRejected(_))
}
