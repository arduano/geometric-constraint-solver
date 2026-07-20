use geosolve_core::{
    AdaptiveStepController, AdaptiveStepDecision, AdaptiveStepPolicy, ContinuationError,
    ContinuationTangent, ContinuationTangentOrientation, HardValidity, InitialParameterDirection,
    LinearSolveBackend, PseudoArclengthVariable, SolveSession, SolveTermination, SolverConfig,
    SourceConstraint, SparseFallbackReason, VariableKind, VariableValue,
};
use nalgebra::DVector;

use crate::compiler::{LinkageSolveResult, LinkageSource, SolvedBody, fresh_hard_audit_max};
use crate::model::{BodyId, BranchViolation, DriverId, DriverKind, Linkage, LinkageError};

/// Explicit orientation of the first linkage continuation tangent.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContinuationDirection {
    IncreasingParameter,
    DecreasingParameter,
}

impl ContinuationDirection {
    const fn core(self) -> InitialParameterDirection {
        match self {
            Self::IncreasingParameter => InitialParameterDirection::Increasing,
            Self::DecreasingParameter => InitialParameterDirection::Decreasing,
        }
    }
}

/// Explicit natural-parameter or pseudo-arclength continuation request.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AdaptiveContinuationMode {
    /// Continue toward one physical driver target and stop at a turning point.
    Natural { target: f64 },
    /// Follow the oriented solution path for a positive normalized distance.
    PseudoArclength {
        path_length: f64,
        initial_direction: ContinuationDirection,
    },
}

/// One validated adaptive continuation request.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AdaptiveContinuationRequest {
    pub driver_id: DriverId,
    pub mode: AdaptiveContinuationMode,
    pub step_policy: AdaptiveStepPolicy,
}

/// Why adaptive continuation stopped after retaining its accepted prefix.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum AdaptiveContinuationStatus {
    Completed,
    /// The natural parameter reached or attempted to cross a turning point.
    PseudoArclengthRequired,
    /// Fresh ordinary validation rejected the stored entry state.
    InitialRejected,
    /// A natural predictor endpoint violates an explicit branch state before correction.
    PredictorBranchEvent(BranchViolation),
    /// The corrector left the documented local neighborhood of its predictor.
    CorrectionNotLocal {
        correction: f64,
        limit: f64,
    },
    MinimumStep,
    RetryLimit,
    SampleLimit,
    TangentFailure(ContinuationError),
}

/// One accepted physical sample. Its report never contains an ephemeral control row.
#[derive(Debug)]
pub struct AdaptiveContinuationSample {
    pub driver_target: f64,
    pub path_step: f64,
    pub retries: usize,
    pub corrector_iterations: usize,
    /// Backend that produced at least one correction step. `None` means the
    /// corrector accepted its initial finite state without a linear solve.
    pub corrector_backend: Option<LinearSolveBackend>,
    /// First typed sparse failure observed by the corrector before dense fallback.
    pub corrector_sparse_fallback_reason: Option<SparseFallbackReason>,
    pub correction_norm: f64,
    pub tangent_parameter_component: f64,
    pub solve: LinkageSolveResult,
}

/// Accepted-prefix outcome of one adaptive continuation call.
#[derive(Debug)]
pub struct AdaptiveContinuationResult {
    pub driver_id: DriverId,
    pub mode: AdaptiveContinuationMode,
    pub initial_target: f64,
    pub accepted_target: f64,
    pub accepted_path_length: f64,
    pub status: AdaptiveContinuationStatus,
    /// Fresh ordinary fixed-driver validation at the stored entry state.
    pub initial_solve: LinkageSolveResult,
    pub samples: Vec<AdaptiveContinuationSample>,
    /// Ordinary fixed-driver physical attempts rejected by physical or
    /// continuation acceptance policy. Ephemeral reports are never published.
    pub rejected_attempts: Vec<LinkageSolveResult>,
}

impl AdaptiveContinuationResult {
    #[must_use]
    pub const fn completed(&self) -> bool {
        matches!(self.status, AdaptiveContinuationStatus::Completed)
    }
}

#[derive(Clone, Copy, Debug)]
struct BodyTangent {
    body_id: BodyId,
    normalized: [f64; 3],
    step_scales: [f64; 3],
}

#[derive(Clone, Debug)]
struct LinkageTangent {
    core: ContinuationTangent,
    bodies: Vec<BodyTangent>,
}

#[derive(Debug)]
struct PseudoCandidate {
    bodies: Vec<SolvedBody>,
    parameter: f64,
    iterations: usize,
    backend: Option<LinearSolveBackend>,
    sparse_fallback_reason: Option<SparseFallbackReason>,
}

impl Linkage {
    /// Runs adaptive predictor-corrector continuation while preserving accepted-prefix state.
    ///
    /// Natural mode never switches to pseudo-arclength. Pseudo mode solves an
    /// ephemeral augmented problem, then publishes and commits only an ordinary
    /// fixed-driver solve after fresh physical and branch validation.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid input, stale model data, or a core operation
    /// that cannot be started. Numerical trial failures are represented by the
    /// returned status while the accepted prefix remains committed.
    #[allow(clippy::too_many_lines)]
    pub fn continue_driver(
        &mut self,
        request: AdaptiveContinuationRequest,
        config: SolverConfig,
    ) -> Result<AdaptiveContinuationResult, LinkageError> {
        let driver = self
            .drivers
            .get(request.driver_id)
            .ok_or(LinkageError::UnknownDriver(request.driver_id))?;
        let initial_target = driver.target();
        let parameter_scale = self.continuation_parameter_scale(request.driver_id)?;
        let maximum_parameter_step = driver.max_continuation_step();
        let initial_direction = match request.mode {
            AdaptiveContinuationMode::Natural { target } => {
                if !target.is_finite() {
                    return Err(LinkageError::NonFiniteValue {
                        context: "requested natural continuation target",
                        value: target,
                    });
                }
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
                    return Err(LinkageError::InvalidContinuationDistance(path_length));
                }
                initial_direction
            }
        };
        let mut controller = AdaptiveStepController::new(request.step_policy)?;
        let mut entry_candidate = self.clone();
        let initial_solve = entry_candidate.solve_attempt(
            Some((request.driver_id, initial_target)),
            None,
            config,
        )?;
        if !initial_solve.accepted() {
            return Ok(AdaptiveContinuationResult {
                driver_id: request.driver_id,
                mode: request.mode,
                initial_target,
                accepted_target: initial_target,
                accepted_path_length: 0.0,
                status: AdaptiveContinuationStatus::InitialRejected,
                initial_solve,
                samples: Vec::new(),
                rejected_attempts: Vec::new(),
            });
        }
        let validated_target = entry_candidate
            .drivers
            .get(request.driver_id)
            .ok_or(LinkageError::UnknownDriver(request.driver_id))?
            .target();
        if validated_target.to_bits() != initial_target.to_bits() {
            return Err(LinkageError::PositionNotAccepted(
                "ordinary entry validation changed the selected driver target".to_owned(),
            ));
        }
        *self = entry_candidate;

        let mut previous_tangent: Option<ContinuationTangent> = None;
        let mut samples = Vec::new();
        let mut rejected_attempts = Vec::new();
        let mut accepted_path_length = 0.0;

        let status = loop {
            let current_target = self
                .drivers
                .get(request.driver_id)
                .ok_or(LinkageError::UnknownDriver(request.driver_id))?
                .target();
            let remaining_path = match request.mode {
                AdaptiveContinuationMode::Natural { target } => {
                    if matches!(
                        current_target.partial_cmp(&target),
                        Some(std::cmp::Ordering::Equal)
                    ) {
                        break AdaptiveContinuationStatus::Completed;
                    }
                    f64::INFINITY
                }
                AdaptiveContinuationMode::PseudoArclength { path_length, .. } => {
                    let remaining = path_length - accepted_path_length;
                    if remaining <= 64.0 * f64::EPSILON * path_length {
                        break AdaptiveContinuationStatus::Completed;
                    }
                    remaining
                }
            };
            if controller.sample_limit_reached() {
                break AdaptiveContinuationStatus::SampleLimit;
            }

            let orientation = previous_tangent.as_ref().map_or_else(
                || ContinuationTangentOrientation::Initial(initial_direction.core()),
                |previous| ContinuationTangentOrientation::Previous(previous.clone()),
            );
            let tangent = match self.continuation_tangent(request.driver_id, &orientation, config) {
                Ok(tangent) => tangent,
                Err(LinkageError::Continuation(error)) => {
                    break AdaptiveContinuationStatus::TangentFailure(error);
                }
                Err(error) => return Err(error),
            };
            let parameter_component = tangent.core.parameter_component();
            let mut path_step = controller.current_step().min(remaining_path);
            if parameter_component.abs() > 64.0 * f64::EPSILON {
                path_step = path_step
                    .min(maximum_parameter_step / (parameter_component.abs() * parameter_scale));
            }

            if let AdaptiveContinuationMode::Natural { target } = request.mode {
                let remaining_parameter = (target - current_target) / parameter_scale;
                if !remaining_parameter.is_finite()
                    || parameter_component * remaining_parameter <= 0.0
                    || parameter_component.abs() <= 64.0 * f64::EPSILON
                {
                    break AdaptiveContinuationStatus::PseudoArclengthRequired;
                }
                path_step = path_step.min((remaining_parameter / parameter_component).abs());
            }
            if !path_step.is_finite() || path_step <= 0.0 {
                break AdaptiveContinuationStatus::MinimumStep;
            }

            let predicted_bodies = self.predict_continuation_bodies(&tangent, path_step)?;
            let mut predicted_parameter =
                current_target + parameter_component * path_step * parameter_scale;
            if let AdaptiveContinuationMode::Natural { target } = request.mode
                && (target - predicted_parameter).signum() != (target - current_target).signum()
            {
                predicted_parameter = target;
            }
            if !predicted_parameter.is_finite() {
                return Err(LinkageError::NonFiniteValue {
                    context: "predicted continuation parameter",
                    value: predicted_parameter,
                });
            }
            let predictor_changed = !matches!(
                predicted_parameter.partial_cmp(&current_target),
                Some(std::cmp::Ordering::Equal)
            ) || predicted_bodies.iter().any(|predicted| {
                self.bodies.get(predicted.body_id).is_some_and(|body| {
                    body.pose()
                        .local_difference(&predicted.pose)
                        .is_ok_and(|difference| {
                            difference
                                .iter()
                                .any(|value| value.classify() != std::num::FpCategory::Zero)
                        })
                })
            });
            if !predictor_changed {
                break AdaptiveContinuationStatus::MinimumStep;
            }

            if matches!(request.mode, AdaptiveContinuationMode::Natural { .. })
                && let Some(violation) = self.predictor_branch_violation(&predicted_bodies)?
            {
                match controller.reject() {
                    AdaptiveStepDecision::Retry => continue,
                    AdaptiveStepDecision::MinimumStep | AdaptiveStepDecision::RetryLimit => {
                        break AdaptiveContinuationStatus::PredictorBranchEvent(violation);
                    }
                }
            }

            let mut candidate_linkage = self.clone();
            let (solve, corrector_iterations, corrector_backend, corrector_sparse_fallback_reason) =
                match request.mode {
                    AdaptiveContinuationMode::Natural { .. } => {
                        let solve = candidate_linkage.solve_attempt(
                            Some((request.driver_id, predicted_parameter)),
                            Some(&predicted_bodies),
                            config,
                        )?;
                        let backend = solve.core_report.actual_backend;
                        let fallback = solve.core_report.sparse_fallback_reason;
                        (solve, None, backend, fallback)
                    }
                    AdaptiveContinuationMode::PseudoArclength { .. } => {
                        let Some(candidate) = self.pseudo_corrector(
                            request.driver_id,
                            current_target,
                            &tangent,
                            path_step,
                            &predicted_bodies,
                            predicted_parameter,
                            config,
                        )?
                        else {
                            match controller.reject() {
                                AdaptiveStepDecision::Retry => continue,
                                AdaptiveStepDecision::MinimumStep => {
                                    break AdaptiveContinuationStatus::MinimumStep;
                                }
                                AdaptiveStepDecision::RetryLimit => {
                                    break AdaptiveContinuationStatus::RetryLimit;
                                }
                            }
                        };
                        let solve = candidate_linkage.solve_attempt(
                            Some((request.driver_id, candidate.parameter)),
                            Some(&candidate.bodies),
                            config,
                        )?;
                        (
                            solve,
                            Some(candidate.iterations),
                            candidate.backend,
                            candidate.sparse_fallback_reason,
                        )
                    }
                };

            if !solve.accepted() {
                rejected_attempts.push(solve);
                match controller.reject() {
                    AdaptiveStepDecision::Retry => continue,
                    AdaptiveStepDecision::MinimumStep => {
                        break AdaptiveContinuationStatus::MinimumStep;
                    }
                    AdaptiveStepDecision::RetryLimit => {
                        break AdaptiveContinuationStatus::RetryLimit;
                    }
                }
            }

            let accepted_parameter = candidate_linkage
                .drivers
                .get(request.driver_id)
                .ok_or(LinkageError::UnknownDriver(request.driver_id))?
                .target();
            let correction_norm = continuation_correction_norm(
                &predicted_bodies,
                &solve.geometry,
                predicted_parameter,
                accepted_parameter,
                self.model_scale,
                parameter_scale,
            )?;
            let correction_limit = controller.correction_limit(path_step)?;
            if correction_norm > correction_limit {
                let rejection = AdaptiveContinuationStatus::CorrectionNotLocal {
                    correction: correction_norm,
                    limit: correction_limit,
                };
                rejected_attempts.push(solve);
                match controller.reject() {
                    AdaptiveStepDecision::Retry => continue,
                    AdaptiveStepDecision::MinimumStep | AdaptiveStepDecision::RetryLimit => {
                        break rejection;
                    }
                }
            }

            if matches!(request.mode, AdaptiveContinuationMode::Natural { .. }) {
                let post_corrector = candidate_linkage.continuation_tangent(
                    request.driver_id,
                    &ContinuationTangentOrientation::Previous(tangent.core.clone()),
                    config,
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
                        rejected_attempts.push(solve);
                        match controller.reject() {
                            AdaptiveStepDecision::Retry => continue,
                            AdaptiveStepDecision::MinimumStep
                            | AdaptiveStepDecision::RetryLimit => {
                                break AdaptiveContinuationStatus::PseudoArclengthRequired;
                            }
                        }
                    }
                    Err(LinkageError::Continuation(error)) => {
                        rejected_attempts.push(solve);
                        break AdaptiveContinuationStatus::TangentFailure(error);
                    }
                    Err(error) => return Err(error),
                }
            }

            let retries = controller.retries();
            let iterations = solve
                .core_report
                .iterations
                .saturating_add(corrector_iterations.unwrap_or(0));
            let next_path_length = accepted_path_length + path_step;
            if !next_path_length.is_finite() {
                return Err(LinkageError::NonFiniteValue {
                    context: "accepted continuation path length",
                    value: next_path_length,
                });
            }
            let sample = AdaptiveContinuationSample {
                driver_target: accepted_parameter,
                path_step,
                retries,
                corrector_iterations: iterations,
                corrector_backend,
                corrector_sparse_fallback_reason,
                correction_norm,
                tangent_parameter_component: parameter_component,
                solve,
            };
            controller.accept(iterations, correction_norm)?;
            accepted_path_length = next_path_length;
            previous_tangent = Some(tangent.core.clone());
            *self = candidate_linkage;
            samples.push(sample);
        };

        let accepted_target = self
            .drivers
            .get(request.driver_id)
            .ok_or(LinkageError::UnknownDriver(request.driver_id))?
            .target();
        Ok(AdaptiveContinuationResult {
            driver_id: request.driver_id,
            mode: request.mode,
            initial_target,
            accepted_target,
            accepted_path_length,
            status,
            initial_solve,
            samples,
            rejected_attempts,
        })
    }

    fn continuation_parameter_scale(&self, driver_id: DriverId) -> Result<f64, LinkageError> {
        let driver = self
            .drivers
            .get(driver_id)
            .ok_or(LinkageError::UnknownDriver(driver_id))?;
        Ok(match driver.kind() {
            DriverKind::Angular { .. } => 1.0,
            DriverKind::Linear { .. } => self.model_scale,
        })
    }

    fn continuation_tangent(
        &self,
        driver_id: DriverId,
        orientation: &ContinuationTangentOrientation,
        config: SolverConfig,
    ) -> Result<LinkageTangent, LinkageError> {
        let compiled = self.compile()?;
        let driver_mapping = compiled
            .source_mapping(LinkageSource::Driver(driver_id))
            .ok_or(LinkageError::UnknownDriver(driver_id))?;
        let residual_id =
            *driver_mapping
                .residual_ids
                .first()
                .ok_or(LinkageError::PositionNotAccepted(
                    "selected driver has no residual row".to_owned(),
                ))?;
        let residual = compiled
            .problem
            .residual(residual_id)
            .ok_or(geosolve_core::CoreError::UnknownResidual(residual_id))?;
        let residual_scale = residual.scales()[0];
        let session = SolveSession::new(compiled.problem.clone(), config)
            .map_err(|error| LinkageError::PositionNotAccepted(error.to_string()))?;
        for mapping in compiled.body_variables() {
            let expected = self.require_body(mapping.body_id)?.pose().ambient();
            let VariableValue::Pose2(actual) = session
                .problem()
                .variable(mapping.variable_id)
                .ok_or(geosolve_core::CoreError::UnknownVariable(
                    mapping.variable_id,
                ))?
                .value()
            else {
                return Err(LinkageError::PositionNotAccepted(
                    "private tangent solve changed a body variable kind".to_owned(),
                ));
            };
            if actual
                .iter()
                .zip(expected)
                .any(|(actual, expected)| actual.to_bits() != expected.to_bits())
            {
                return Err(LinkageError::PositionNotAccepted(
                    "private tangent solve diverged from the freshly accepted linkage state"
                        .to_owned(),
                ));
            }
        }
        let accepted = session.accepted_hard_linearization()?;
        let component = accepted
            .components()
            .iter()
            .find(|component| {
                component
                    .hard_rows()
                    .iter()
                    .any(|row| row.row.residual_id == residual_id)
            })
            .ok_or(LinkageError::PositionNotAccepted(
                "selected driver row is absent from accepted hard linearization".to_owned(),
            ))?;
        let driver_row = component
            .hard_rows()
            .iter()
            .position(|row| row.row.residual_id == residual_id)
            .ok_or(LinkageError::PositionNotAccepted(
                "selected driver row is absent from its hard component".to_owned(),
            ))?;
        let mut parameter_column = DVector::zeros(component.hard_rows().len());
        parameter_column[driver_row] =
            -self.continuation_parameter_scale(driver_id)? / residual_scale;
        let tangent = component.augmented_unit_null_tangent(&parameter_column, orientation)?;

        let mut bodies = Vec::new();
        for block in component.tangent_blocks() {
            if block.kind != VariableKind::Pose2 || block.tangent_range.len() != 3 {
                return Err(LinkageError::PositionNotAccepted(
                    "linkage continuation component contains a non-Pose2 tangent block".to_owned(),
                ));
            }
            let mapping = compiled
                .body_variables()
                .iter()
                .find(|mapping| mapping.variable_id == block.root)
                .ok_or(LinkageError::PositionNotAccepted(
                    "continuation tangent block is not a linkage body".to_owned(),
                ))?;
            bodies.push(BodyTangent {
                body_id: mapping.body_id,
                normalized: [
                    tangent.normalized_state()[block.tangent_range.start],
                    tangent.normalized_state()[block.tangent_range.start + 1],
                    tangent.normalized_state()[block.tangent_range.start + 2],
                ],
                step_scales: [
                    block.step_scales[0],
                    block.step_scales[1],
                    block.step_scales[2],
                ],
            });
        }
        Ok(LinkageTangent {
            core: tangent,
            bodies,
        })
    }

    fn predict_continuation_bodies(
        &self,
        tangent: &LinkageTangent,
        path_step: f64,
    ) -> Result<Vec<SolvedBody>, LinkageError> {
        self.bodies
            .iter()
            .map(|(body_id, body)| {
                let pose = if let Some(block) =
                    tangent.bodies.iter().find(|block| block.body_id == body_id)
                {
                    body.pose()
                        .retract([
                            block.normalized[0] * block.step_scales[0] * path_step,
                            block.normalized[1] * block.step_scales[1] * path_step,
                            block.normalized[2] * block.step_scales[2] * path_step,
                        ])
                        .map_err(|_| LinkageError::NonFinitePose {
                            context: "continuation predictor",
                        })?
                } else {
                    body.pose()
                };
                Ok(SolvedBody { body_id, pose })
            })
            .collect()
    }

    fn predictor_branch_violation(
        &self,
        predicted_bodies: &[SolvedBody],
    ) -> Result<Option<BranchViolation>, LinkageError> {
        let mut predictor = self.clone();
        for body in predicted_bodies {
            predictor.set_body_pose(body.body_id, body.pose)?;
        }
        let geometry = predictor.geometry()?;
        predictor.first_branch_violation(&geometry)
    }

    #[allow(clippy::too_many_arguments)]
    fn pseudo_corrector(
        &self,
        driver_id: DriverId,
        reference_parameter: f64,
        tangent: &LinkageTangent,
        path_step: f64,
        predicted_bodies: &[SolvedBody],
        predicted_parameter: f64,
        config: SolverConfig,
    ) -> Result<Option<PseudoCandidate>, LinkageError> {
        let (mut compiled, parameter_variable) =
            self.compile_with_parameterized_driver(driver_id, predicted_parameter)?;
        for body in predicted_bodies {
            let variable = compiled
                .variable_for_body(body.body_id)
                .ok_or(LinkageError::UnknownBody(body.body_id))?;
            compiled
                .problem
                .set_variable_value(variable, VariableValue::Pose2(body.pose.ambient()))?;
        }
        compiled.problem.set_variable_value(
            parameter_variable,
            VariableValue::Scalar(predicted_parameter),
        )?;

        let mut control_variables = Vec::with_capacity(tangent.bodies.len() + 1);
        for block in &tangent.bodies {
            let variable = compiled
                .variable_for_body(block.body_id)
                .ok_or(LinkageError::UnknownBody(block.body_id))?;
            let reference = self.require_body(block.body_id)?.pose();
            control_variables.push(PseudoArclengthVariable::new(
                variable,
                VariableValue::Pose2(reference.ambient()),
                block.normalized.to_vec(),
            )?);
        }
        control_variables.push(PseudoArclengthVariable::new(
            parameter_variable,
            VariableValue::Scalar(reference_parameter),
            vec![tangent.core.parameter_component()],
        )?);
        let source = compiled
            .problem
            .add_source(SourceConstraint::new("ephemeral pseudo-arclength control")?);
        compiled
            .problem
            .add_pseudo_arclength(source, &control_variables, path_step)?;

        let report = compiled.problem.solve(config)?;
        if report.termination != SolveTermination::Converged
            || report.hard_validity != HardValidity::Valid
            || !report.hard_residuals_validated
            || !report.rank_is_valid
            || report.hard_residual_max > config.normalized_residual_tolerance
        {
            return Ok(None);
        }
        let geometry = compiled.solved_geometry()?;
        let VariableValue::Scalar(parameter) = compiled
            .problem
            .variable(parameter_variable)
            .ok_or(geosolve_core::CoreError::UnknownVariable(
                parameter_variable,
            ))?
            .value()
        else {
            return Err(LinkageError::PositionNotAccepted(
                "pseudo continuation parameter changed kind".to_owned(),
            ));
        };
        if !parameter.is_finite()
            || fresh_hard_audit_max(&compiled.problem)? > config.normalized_residual_tolerance
            || self.domain_hard_residual_max(&geometry, Some((driver_id, parameter)))?
                > config.normalized_residual_tolerance
            || self.first_branch_violation(&geometry)?.is_some()
        {
            return Ok(None);
        }
        Ok(Some(PseudoCandidate {
            bodies: geometry.bodies,
            parameter,
            iterations: report.iterations,
            backend: report.actual_backend,
            sparse_fallback_reason: report.sparse_fallback_reason,
        }))
    }
}

fn continuation_correction_norm(
    predicted: &[SolvedBody],
    solved: &crate::LinkageGeometry,
    predicted_parameter: f64,
    solved_parameter: f64,
    model_scale: f64,
    parameter_scale: f64,
) -> Result<f64, LinkageError> {
    let mut norm = ((solved_parameter - predicted_parameter) / parameter_scale).abs();
    for predicted_body in predicted {
        let solved_pose = solved
            .body_pose(predicted_body.body_id)
            .ok_or(LinkageError::UnknownBody(predicted_body.body_id))?;
        let difference = predicted_body
            .pose
            .local_difference(&solved_pose)
            .map_err(|_| LinkageError::NonFinitePose {
                context: "continuation corrector difference",
            })?;
        for value in [
            difference[0] / model_scale,
            difference[1] / model_scale,
            difference[2],
        ] {
            norm = norm.hypot(value);
        }
    }
    if norm.is_finite() {
        Ok(norm)
    } else {
        Err(LinkageError::NonFiniteValue {
            context: "continuation correction norm",
            value: norm,
        })
    }
}
