use geosolve_geometry::{Pose2 as GeometryPose2, Pose3 as GeometryPose3};
use nalgebra::DVector;
use thiserror::Error;

use crate::autodiff::fixed_pose_local_difference_jacobian;
use crate::{
    AuditBinding, CoreError, EvaluationError, LocalJacobian, Problem, ResidualBlock,
    ResidualCategory, ResidualEvaluator, ResidualId, ResidualRowAudit, SourceConstraintId,
    VariableId, VariableValue,
};

/// Input, rank, orientation, or numerical failure in reusable continuation math.
#[derive(Clone, Debug, Error, PartialEq)]
#[non_exhaustive]
pub enum ContinuationError {
    #[error("invalid adaptive continuation policy field {field}: {message}")]
    InvalidPolicy {
        field: &'static str,
        message: &'static str,
    },
    #[error("{context} has dimension {actual}, expected {expected}")]
    DimensionMismatch {
        context: &'static str,
        expected: usize,
        actual: usize,
    },
    #[error("{context} value {index} must be finite, got {value}")]
    NonFiniteValue {
        context: &'static str,
        index: usize,
        value: f64,
    },
    #[error(
        "augmented continuation system has rank {rank}, {columns} columns, and right nullity {right_nullity}; expected one"
    )]
    UnexpectedAugmentedNullity {
        rank: usize,
        columns: usize,
        right_nullity: usize,
    },
    #[error("continuation tangent orientation is numerically ambiguous")]
    AmbiguousOrientation,
    #[error("continuation tangent validation residual {maximum} exceeds {tolerance}")]
    TangentValidationFailed { maximum: f64, tolerance: f64 },
    #[error("adaptive corrector norm must be nonnegative and finite, got {value}")]
    InvalidCorrectionNorm { value: f64 },
    #[error("adaptive path step must be positive and finite, got {value}")]
    InvalidPathStep { value: f64 },
    #[error("continuation numerical failure: {context}")]
    NumericalFailure { context: &'static str },
}

/// Explicit sign used to orient the first augmented continuation tangent.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InitialParameterDirection {
    Increasing,
    Decreasing,
}

/// Deterministic orientation input for an augmented null tangent.
#[derive(Clone, Debug, PartialEq)]
pub enum ContinuationTangentOrientation {
    Initial(InitialParameterDirection),
    Previous(ContinuationTangent),
}

/// Independently validated unit tangent in normalized state/parameter coordinates.
#[derive(Clone, Debug, PartialEq)]
pub struct ContinuationTangent {
    normalized_state: DVector<f64>,
    parameter_component: f64,
    equation_residual_max: f64,
    augmented_rank: usize,
    rank_threshold: f64,
}

impl ContinuationTangent {
    pub(crate) fn new(
        normalized_state: DVector<f64>,
        parameter_component: f64,
        equation_residual_max: f64,
        augmented_rank: usize,
        rank_threshold: f64,
    ) -> Result<Self, ContinuationError> {
        if let Some((index, &value)) = normalized_state
            .iter()
            .enumerate()
            .find(|(_, value)| !value.is_finite())
        {
            return Err(ContinuationError::NonFiniteValue {
                context: "continuation tangent",
                index,
                value,
            });
        }
        for (index, value) in [parameter_component, equation_residual_max, rank_threshold]
            .into_iter()
            .enumerate()
        {
            if !value.is_finite() {
                return Err(ContinuationError::NonFiniteValue {
                    context: "continuation tangent metadata",
                    index,
                    value,
                });
            }
        }
        let norm = normalized_state
            .iter()
            .fold(parameter_component.abs(), |norm, value| norm.hypot(*value));
        if !norm.is_finite() || (norm - 1.0).abs() > 256.0 * f64::EPSILON {
            return Err(ContinuationError::NumericalFailure {
                context: "continuation tangent is not unit length",
            });
        }
        Ok(Self {
            normalized_state,
            parameter_component,
            equation_residual_max,
            augmented_rank,
            rank_threshold,
        })
    }

    #[must_use]
    pub const fn normalized_state(&self) -> &DVector<f64> {
        &self.normalized_state
    }

    #[must_use]
    pub const fn parameter_component(&self) -> f64 {
        self.parameter_component
    }

    #[must_use]
    pub const fn equation_residual_max(&self) -> f64 {
        self.equation_residual_max
    }

    #[must_use]
    pub const fn augmented_rank(&self) -> usize {
        self.augmented_rank
    }

    #[must_use]
    pub const fn rank_threshold(&self) -> f64 {
        self.rank_threshold
    }
}

/// Validated deterministic adaptive continuation policy in normalized path units.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AdaptiveStepPolicy {
    pub initial_step: f64,
    pub minimum_step: f64,
    pub maximum_step: f64,
    pub growth_factor: f64,
    pub shrink_factor: f64,
    pub fast_iterations: usize,
    pub slow_iterations: usize,
    pub small_correction: f64,
    pub large_correction: f64,
    /// Absolute normalized correction accepted as local to a predictor.
    pub maximum_correction: f64,
    /// Maximum correction divided by the attempted normalized path step.
    pub maximum_correction_step_ratio: f64,
    pub max_retries: usize,
    pub max_samples: usize,
}

impl AdaptiveStepPolicy {
    /// Validates all finite ranges before a controller is constructed.
    ///
    /// # Errors
    ///
    /// Returns [`ContinuationError::InvalidPolicy`] for any invalid range,
    /// factor, threshold, or zero retry/sample budget.
    pub fn validate(&self) -> Result<(), ContinuationError> {
        for (field, value) in [
            ("initial_step", self.initial_step),
            ("minimum_step", self.minimum_step),
            ("maximum_step", self.maximum_step),
            ("small_correction", self.small_correction),
            ("large_correction", self.large_correction),
            ("maximum_correction", self.maximum_correction),
            (
                "maximum_correction_step_ratio",
                self.maximum_correction_step_ratio,
            ),
        ] {
            if !value.is_finite() || value <= 0.0 {
                return Err(ContinuationError::InvalidPolicy {
                    field,
                    message: "must be positive and finite",
                });
            }
        }
        if self.minimum_step > self.initial_step || self.initial_step > self.maximum_step {
            return Err(ContinuationError::InvalidPolicy {
                field: "step range",
                message: "must satisfy minimum <= initial <= maximum",
            });
        }
        if !self.growth_factor.is_finite() || self.growth_factor <= 1.0 {
            return Err(ContinuationError::InvalidPolicy {
                field: "growth_factor",
                message: "must be finite and greater than one",
            });
        }
        if !self.shrink_factor.is_finite() || self.shrink_factor <= 0.0 || self.shrink_factor >= 1.0
        {
            return Err(ContinuationError::InvalidPolicy {
                field: "shrink_factor",
                message: "must be finite and strictly between zero and one",
            });
        }
        if self.fast_iterations > self.slow_iterations {
            return Err(ContinuationError::InvalidPolicy {
                field: "iteration range",
                message: "fast_iterations must not exceed slow_iterations",
            });
        }
        if self.small_correction > self.large_correction {
            return Err(ContinuationError::InvalidPolicy {
                field: "correction range",
                message: "small_correction must not exceed large_correction",
            });
        }
        if self.max_retries == 0 || self.max_samples == 0 {
            return Err(ContinuationError::InvalidPolicy {
                field: "budgets",
                message: "retry and sample budgets must be nonzero",
            });
        }
        Ok(())
    }
}

impl Default for AdaptiveStepPolicy {
    fn default() -> Self {
        Self {
            initial_step: 0.02,
            minimum_step: 1.0e-6,
            maximum_step: 0.1,
            growth_factor: 1.5,
            shrink_factor: 0.5,
            fast_iterations: 4,
            slow_iterations: 12,
            small_correction: 0.05,
            large_correction: 0.5,
            maximum_correction: 0.25,
            maximum_correction_step_ratio: 1.0,
            max_retries: 12,
            max_samples: 10_000,
        }
    }
}

/// Outcome of shrinking a rejected adaptive trial.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdaptiveStepDecision {
    Retry,
    MinimumStep,
    RetryLimit,
}

/// Mutable deterministic step-size state. It owns no candidate geometry.
#[derive(Clone, Debug, PartialEq)]
pub struct AdaptiveStepController {
    policy: AdaptiveStepPolicy,
    current_step: f64,
    retries: usize,
    accepted_samples: usize,
}

impl AdaptiveStepController {
    /// Constructs a controller after validating every policy field.
    ///
    /// # Errors
    ///
    /// Returns [`ContinuationError::InvalidPolicy`] when validation fails.
    pub fn new(policy: AdaptiveStepPolicy) -> Result<Self, ContinuationError> {
        policy.validate()?;
        Ok(Self {
            current_step: policy.initial_step,
            policy,
            retries: 0,
            accepted_samples: 0,
        })
    }

    #[must_use]
    pub const fn policy(&self) -> AdaptiveStepPolicy {
        self.policy
    }

    #[must_use]
    pub const fn current_step(&self) -> f64 {
        self.current_step
    }

    #[must_use]
    pub const fn retries(&self) -> usize {
        self.retries
    }

    #[must_use]
    pub const fn accepted_samples(&self) -> usize {
        self.accepted_samples
    }

    #[must_use]
    pub fn sample_limit_reached(&self) -> bool {
        self.accepted_samples >= self.policy.max_samples
    }

    /// Returns the stricter absolute/path-relative correction-locality limit.
    ///
    /// # Errors
    ///
    /// Returns [`ContinuationError::InvalidPathStep`] for a nonpositive or
    /// non-finite attempted path step.
    pub fn correction_limit(&self, path_step: f64) -> Result<f64, ContinuationError> {
        if !path_step.is_finite() || path_step <= 0.0 {
            return Err(ContinuationError::InvalidPathStep { value: path_step });
        }
        Ok(self
            .policy
            .maximum_correction
            .min(self.policy.maximum_correction_step_ratio * path_step))
    }

    /// Records one accepted corrector and deterministically adapts the next step.
    ///
    /// # Errors
    ///
    /// Returns [`ContinuationError::InvalidCorrectionNorm`] when the correction
    /// norm is negative or non-finite.
    pub fn accept(
        &mut self,
        corrector_iterations: usize,
        correction_norm: f64,
    ) -> Result<(), ContinuationError> {
        if !correction_norm.is_finite() || correction_norm < 0.0 {
            return Err(ContinuationError::InvalidCorrectionNorm {
                value: correction_norm,
            });
        }
        self.accepted_samples = self.accepted_samples.saturating_add(1);
        self.retries = 0;
        if corrector_iterations <= self.policy.fast_iterations
            && correction_norm <= self.policy.small_correction
        {
            self.current_step =
                (self.current_step * self.policy.growth_factor).min(self.policy.maximum_step);
        } else if corrector_iterations >= self.policy.slow_iterations
            || correction_norm >= self.policy.large_correction
        {
            self.current_step =
                (self.current_step * self.policy.shrink_factor).max(self.policy.minimum_step);
        }
        Ok(())
    }

    /// Records one rejected candidate without mutating any accepted domain state.
    pub fn reject(&mut self) -> AdaptiveStepDecision {
        self.retries = self.retries.saturating_add(1);
        if self.retries > self.policy.max_retries {
            return AdaptiveStepDecision::RetryLimit;
        }
        let shrunken = self.current_step * self.policy.shrink_factor;
        if self.current_step <= self.policy.minimum_step {
            self.current_step = self.policy.minimum_step;
            AdaptiveStepDecision::MinimumStep
        } else {
            self.current_step = shrunken.max(self.policy.minimum_step);
            AdaptiveStepDecision::Retry
        }
    }
}

/// One incident block in a pseudo-arclength control row.
#[derive(Clone, Debug, PartialEq)]
pub struct PseudoArclengthVariable {
    variable_id: VariableId,
    reference: VariableValue,
    normalized_tangent: Vec<f64>,
}

impl PseudoArclengthVariable {
    /// Constructs and validates one normalized local tangent block.
    ///
    /// # Errors
    ///
    /// Returns a [`CoreError`] for invalid reference geometry, tangent
    /// dimensions, or non-finite tangent data. Authoritative step scales and
    /// the actual variable kind are resolved by [`Problem::add_pseudo_arclength`].
    pub fn new(
        variable_id: VariableId,
        reference: VariableValue,
        normalized_tangent: Vec<f64>,
    ) -> Result<Self, CoreError> {
        let reference = reference.canonicalized()?;
        let expected = reference.kind().tangent_dimension();
        if normalized_tangent.len() != expected {
            return Err(CoreError::DimensionMismatch {
                context: "pseudo-arclength tangent block",
                expected,
                actual: normalized_tangent.len(),
            });
        }
        if let Some((index, &value)) = normalized_tangent
            .iter()
            .enumerate()
            .find(|(_, value)| !value.is_finite())
        {
            return Err(CoreError::NonFiniteValue {
                context: "pseudo-arclength tangent",
                index,
                value,
            });
        }
        Ok(Self {
            variable_id,
            reference,
            normalized_tangent,
        })
    }

    #[must_use]
    pub const fn variable_id(&self) -> VariableId {
        self.variable_id
    }

    #[must_use]
    pub const fn reference(&self) -> VariableValue {
        self.reference
    }

    #[must_use]
    pub fn normalized_tangent(&self) -> &[f64] {
        &self.normalized_tangent
    }
}

#[derive(Clone, Debug)]
struct ResolvedPseudoArclengthVariable {
    variable_id: VariableId,
    reference: VariableValue,
    step_scales: Vec<f64>,
    normalized_tangent: Vec<f64>,
}

impl ResolvedPseudoArclengthVariable {
    fn new(problem: &Problem, variable: &PseudoArclengthVariable) -> Result<Self, CoreError> {
        let block = problem
            .variable(variable.variable_id)
            .ok_or(CoreError::UnknownVariable(variable.variable_id))?;
        let expected_kind = block.kind();
        let actual_kind = variable.reference.kind();
        if actual_kind != expected_kind {
            return Err(CoreError::VariableKindMismatch {
                expected: expected_kind,
                actual: actual_kind,
            });
        }
        let expected = expected_kind.tangent_dimension();
        if variable.normalized_tangent.len() != expected {
            return Err(CoreError::DimensionMismatch {
                context: "pseudo-arclength tangent block",
                expected,
                actual: variable.normalized_tangent.len(),
            });
        }
        let step_scales = block.step_scales().to_vec();
        for (index, (&tangent, &scale)) in variable
            .normalized_tangent
            .iter()
            .zip(&step_scales)
            .enumerate()
        {
            let coefficient = tangent / scale;
            if !coefficient.is_finite() {
                return Err(CoreError::NonFiniteValue {
                    context: "pseudo-arclength raw coefficient",
                    index,
                    value: coefficient,
                });
            }
        }
        Ok(Self {
            variable_id: variable.variable_id,
            reference: variable.reference,
            step_scales,
            normalized_tangent: variable.normalized_tangent.clone(),
        })
    }
}

#[derive(Clone, Debug)]
struct PseudoBlock {
    reference: VariableValue,
    raw_coefficients: Vec<f64>,
}

/// Core-owned pseudo-arclength hard evaluator over normalized local differences.
#[derive(Clone, Debug)]
struct PseudoArclengthResidual {
    blocks: Vec<PseudoBlock>,
    signed_distance: f64,
}

impl PseudoArclengthResidual {
    fn new(
        variables: &[ResolvedPseudoArclengthVariable],
        signed_distance: f64,
    ) -> Result<Self, CoreError> {
        if !signed_distance.is_finite() {
            return Err(CoreError::NonFiniteValue {
                context: "pseudo-arclength signed distance",
                index: 0,
                value: signed_distance,
            });
        }
        let blocks = variables
            .iter()
            .map(|variable| PseudoBlock {
                reference: variable.reference,
                raw_coefficients: variable
                    .normalized_tangent
                    .iter()
                    .zip(&variable.step_scales)
                    .map(|(tangent, scale)| tangent / scale)
                    .collect(),
            })
            .collect();
        Ok(Self {
            blocks,
            signed_distance,
        })
    }
}

impl ResidualEvaluator for PseudoArclengthResidual {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        if variables.len() != self.blocks.len() {
            return Err(EvaluationError::invalid_geometry(
                "pseudo-arclength incidence changed",
            ));
        }
        let mut residual = -self.signed_distance;
        for (value, block) in variables.iter().zip(&self.blocks) {
            let difference = checked_local_difference(block.reference, *value)?;
            if difference.len() != block.raw_coefficients.len() {
                return Err(EvaluationError::invalid_geometry(
                    "pseudo-arclength local difference changed dimension",
                ));
            }
            for (coefficient, difference) in block.raw_coefficients.iter().zip(difference) {
                residual += coefficient * difference;
            }
        }
        if residual.is_finite() {
            Ok(vec![residual])
        } else {
            Err(EvaluationError::invalid_geometry(
                "pseudo-arclength residual is non-finite",
            ))
        }
    }

    fn jacobian(&self, variables: &[VariableValue]) -> Result<Vec<LocalJacobian>, EvaluationError> {
        if variables.len() != self.blocks.len() {
            return Err(EvaluationError::invalid_geometry(
                "pseudo-arclength incidence changed",
            ));
        }
        variables
            .iter()
            .zip(&self.blocks)
            .map(|(value, block)| {
                if value.kind() != block.reference.kind() {
                    return Err(EvaluationError::invalid_geometry(
                        "pseudo-arclength variable kind changed",
                    ));
                }
                let columns = value.kind().tangent_dimension();
                if matches!(value, VariableValue::Pose2(_) | VariableValue::Pose3(_)) {
                    let local = fixed_pose_local_difference_jacobian(block.reference, *value)?;
                    let values = (0..columns)
                        .map(|column| {
                            block
                                .raw_coefficients
                                .iter()
                                .enumerate()
                                .map(|(row, coefficient)| {
                                    coefficient * local.values()[row * columns + column]
                                })
                                .sum()
                        })
                        .collect();
                    Ok(LocalJacobian::new(1, columns, values))
                } else {
                    Ok(LocalJacobian::new(
                        1,
                        columns,
                        block.raw_coefficients.clone(),
                    ))
                }
            })
            .collect()
    }
}

impl Problem {
    /// Adds one dimensionless hard pseudo-arclength control row.
    ///
    /// References and tangent coefficients use the same right/body-local
    /// coordinates and characteristic step scales as ordinary core variables.
    /// The row owns complete structured audit metadata but is intended for an
    /// ephemeral corrector problem, never a published physical report.
    ///
    /// # Errors
    ///
    /// Returns a [`CoreError`] for an unknown source/variable, duplicate or
    /// empty incidence, mismatched variable/reference kinds, invalid tangent
    /// dimensions, non-finite data, or coefficient scaling overflow.
    pub fn add_pseudo_arclength(
        &mut self,
        source: SourceConstraintId,
        variables: &[PseudoArclengthVariable],
        signed_distance: f64,
    ) -> Result<ResidualId, CoreError> {
        if variables.is_empty() {
            return Err(CoreError::EmptyDimension {
                context: "pseudo-arclength incidence",
            });
        }
        if self.source(source).is_none() {
            return Err(CoreError::UnknownSource(source));
        }
        let variables = variables
            .iter()
            .map(|variable| ResolvedPseudoArclengthVariable::new(self, variable))
            .collect::<Result<Vec<_>, _>>()?;
        let evaluator = PseudoArclengthResidual::new(&variables, signed_distance)?;
        let incidence = variables
            .iter()
            .map(|variable| variable.variable_id)
            .collect::<Vec<_>>();
        let residual = ResidualBlock::new(
            source,
            ResidualCategory::Hard,
            incidence,
            1,
            vec![1.0],
            vec![ResidualRowAudit::new(
                "dot(normalized_path_tangent, normalized_local_difference(reference, state)) - signed_distance",
                vec![
                    AuditBinding::new("continuation control", "pseudo-arclength"),
                    AuditBinding::new("signed distance", signed_distance.to_string()),
                    AuditBinding::new("incident blocks", variables.len().to_string()),
                ],
                "normalized arclength",
            )],
            evaluator,
        )?;
        self.add_residual(residual)
    }
}

fn checked_local_difference(
    reference: VariableValue,
    value: VariableValue,
) -> Result<Vec<f64>, EvaluationError> {
    if reference.kind() != value.kind() {
        return Err(EvaluationError::invalid_geometry(
            "pseudo-arclength reference kind changed",
        ));
    }
    match (reference, value) {
        (VariableValue::Scalar(reference), VariableValue::Scalar(value)) => {
            Ok(vec![value - reference])
        }
        (VariableValue::Vec2(reference), VariableValue::Vec2(value)) => Ok(value
            .into_iter()
            .zip(reference)
            .map(|(value, reference)| value - reference)
            .collect()),
        (VariableValue::Vec3(reference), VariableValue::Vec3(value)) => Ok(value
            .into_iter()
            .zip(reference)
            .map(|(value, reference)| value - reference)
            .collect()),
        (VariableValue::Pose2(reference), VariableValue::Pose2(value)) => {
            fixed_pose_local_difference_jacobian(
                VariableValue::Pose2(reference),
                VariableValue::Pose2(value),
            )?;
            let reference = GeometryPose2::from_ambient(reference).map_err(|error| {
                EvaluationError::invalid_geometry(format!(
                    "invalid pseudo-arclength Pose2 reference: {error}"
                ))
            })?;
            let value = GeometryPose2::from_ambient(value).map_err(|error| {
                EvaluationError::invalid_geometry(format!(
                    "invalid pseudo-arclength Pose2 value: {error}"
                ))
            })?;
            reference
                .local_difference(&value)
                .map(Vec::from)
                .map_err(|error| {
                    EvaluationError::invalid_geometry(format!(
                        "invalid pseudo-arclength Pose2 local difference: {error}"
                    ))
                })
        }
        (VariableValue::Pose3(reference), VariableValue::Pose3(value)) => {
            fixed_pose_local_difference_jacobian(
                VariableValue::Pose3(reference),
                VariableValue::Pose3(value),
            )?;
            let reference = GeometryPose3::from_ambient(reference).map_err(|error| {
                EvaluationError::invalid_geometry(format!(
                    "invalid pseudo-arclength Pose3 reference: {error}"
                ))
            })?;
            let value = GeometryPose3::from_ambient(value).map_err(|error| {
                EvaluationError::invalid_geometry(format!(
                    "invalid pseudo-arclength Pose3 value: {error}"
                ))
            })?;
            reference
                .local_difference(&value)
                .map(Vec::from)
                .map_err(|error| {
                    EvaluationError::invalid_geometry(format!(
                        "invalid pseudo-arclength Pose3 local difference: {error}"
                    ))
                })
        }
        _ => Err(EvaluationError::invalid_geometry(
            "pseudo-arclength variable kinds do not match",
        )),
    }
}
