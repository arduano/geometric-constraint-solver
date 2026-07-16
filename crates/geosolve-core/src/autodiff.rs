use std::fmt::Debug;

use geosolve_geometry::{
    Pose2 as GeometryPose2, Pose3 as GeometryPose3, QUATERNION_SIGN_TOLERANCE,
};
use num_dual::{DualDVec64, DualNum};

use crate::{
    EvaluationError, LinearizationStorage, LocalJacobian, ResidualEvaluator, VariableValue,
};

pub(crate) enum AdVariableValue {
    Scalar(DualDVec64),
    Vec2([DualDVec64; 2]),
    Pose2([DualDVec64; 3]),
    Vec3([DualDVec64; 3]),
    /// `[t_x, t_y, t_z, q_w, q_x, q_y, q_z]`. Formula outputs using this
    /// representation must be invariant under `q -> -q`.
    Pose3([DualDVec64; 7]),
}

pub(crate) trait LocalAdFormulaClone {
    fn clone_box(&self) -> Box<dyn LocalAdFormula>;
}

impl<T> LocalAdFormulaClone for T
where
    T: LocalAdFormula + Clone + 'static,
{
    fn clone_box(&self) -> Box<dyn LocalAdFormula> {
        Box::new(self.clone())
    }
}

impl Clone for Box<dyn LocalAdFormula> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}

/// A formula in ambient values seeded by the variables' local retractions.
///
/// Implementations accepting [`AdVariableValue::Pose3`] must define equations
/// invariant under quaternion sign (`f(t, q) == f(t, -q)`). Canonical Pose3
/// storage can switch quaternion representative across an exact half turn;
/// odd or linear quaternion-component equations therefore do not define a
/// function on `SE(3)` and are invalid formulas for this adapter. Valid
/// sign-invariant formulas remain differentiable at ordinary half turns.
pub(crate) trait LocalAdFormula: LocalAdFormulaClone + Debug + Send + Sync {
    fn evaluate(&self, variables: &[AdVariableValue]) -> Result<Vec<DualDVec64>, EvaluationError>;
}

#[derive(Clone, Debug)]
pub(crate) struct LocalAdEvaluator {
    formula: Box<dyn LocalAdFormula>,
}

impl LocalAdEvaluator {
    pub(crate) fn new(formula: impl LocalAdFormula + 'static) -> Self {
        Self {
            formula: Box::new(formula),
        }
    }

    fn evaluate_seeded(
        &self,
        variables: &[VariableValue],
        step_scales: &[Vec<f64>],
    ) -> Result<(Vec<DualDVec64>, Vec<usize>), EvaluationError> {
        if variables.len() != step_scales.len()
            || variables
                .iter()
                .zip(step_scales)
                .any(|(value, scales)| value.kind().tangent_dimension() != scales.len())
        {
            return Err(EvaluationError::invalid_geometry(
                "local AD incidence and tangent scales do not match",
            ));
        }
        let width = step_scales.iter().map(Vec::len).sum();
        let mut offsets = Vec::with_capacity(variables.len());
        let mut offset = 0;
        let mut dual_variables = Vec::with_capacity(variables.len());
        for (value, scales) in variables.iter().zip(step_scales) {
            offsets.push(offset);
            dual_variables.push(retract_normalized_tangent(*value, scales, width, offset)?);
            offset += scales.len();
        }
        self.formula
            .evaluate(&dual_variables)
            .map(|values| (values, offsets))
    }
}

impl ResidualEvaluator for LocalAdEvaluator {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        let scales = variables
            .iter()
            .map(|value| vec![1.0; value.kind().tangent_dimension()])
            .collect::<Vec<_>>();
        let (values, _) = self.evaluate_seeded(variables, &scales)?;
        Ok(values.into_iter().map(|value| value.re).collect())
    }

    fn jacobian(&self, variables: &[VariableValue]) -> Result<Vec<LocalJacobian>, EvaluationError> {
        let scales = variables
            .iter()
            .map(|value| vec![1.0; value.kind().tangent_dimension()])
            .collect::<Vec<_>>();
        let (values, offsets) = self.evaluate_seeded(variables, &scales)?;
        let rows = values.len();
        Ok(variables
            .iter()
            .enumerate()
            .map(|(block, variable)| {
                let columns = variable.kind().tangent_dimension();
                let mut derivatives = Vec::with_capacity(rows * columns);
                for value in &values {
                    for column in 0..columns {
                        derivatives.push(derivative(value, offsets[block] + column));
                    }
                }
                LocalJacobian::new(rows, columns, derivatives)
            })
            .collect())
    }

    fn linearize(
        &self,
        variables: &[VariableValue],
        storage: &mut LinearizationStorage<'_, '_>,
    ) -> Option<Result<(), EvaluationError>> {
        Some((|| {
            if variables.len() != storage.jacobian_block_count() {
                return Err(EvaluationError::invalid_geometry(
                    "local AD incidence does not match fused storage",
                ));
            }
            let step_scales = (0..storage.jacobian_block_count())
                .map(|block| {
                    storage
                        .jacobian_block(block)
                        .expect("block index was checked")
                        .step_scales()
                        .to_vec()
                })
                .collect::<Vec<_>>();
            let (values, offsets) = self.evaluate_seeded(variables, &step_scales)?;
            if values.len() != storage.residuals().len() {
                return Err(EvaluationError::invalid_geometry(
                    "local AD output does not match fused residual storage",
                ));
            }
            for (target, value) in storage.residuals_mut().iter_mut().zip(&values) {
                *target = value.re;
            }
            for (block, offset) in offsets.iter().copied().enumerate() {
                let output = storage
                    .jacobian_block_mut(block)
                    .expect("block index was checked");
                let columns = output.columns();
                if output.rows() != values.len() || output.step_scales().len() != columns {
                    return Err(EvaluationError::invalid_geometry(
                        "local AD Jacobian shape does not match fused storage",
                    ));
                }
                for (row, value) in values.iter().enumerate() {
                    for column in 0..columns {
                        // AD was seeded with normalized tangent increments, so these
                        // derivatives must never be converted through a raw 1/scale.
                        output.values_mut()[row * columns + column] =
                            derivative(value, offset + column);
                    }
                }
            }
            storage.mark_normalized_tangent_jacobians();
            Ok(())
        })())
    }
}

// Each ambient dual is seeded with the differential at zero of the same local
// retraction used by VariableValue::plus.
fn retract_normalized_tangent(
    value: VariableValue,
    step_scales: &[f64],
    width: usize,
    offset: usize,
) -> Result<AdVariableValue, EvaluationError> {
    let seed = |real: f64, derivatives: &[(usize, f64)]| {
        let mut dual = DualDVec64::from_re(real).derivative(width, offset);
        let storage = dual.eps.0.as_mut().expect("seeded derivative");
        storage.fill(0.0);
        for &(coordinate, derivative) in derivatives {
            storage[offset + coordinate] = derivative;
        }
        dual
    };
    Ok(match value {
        VariableValue::Scalar(value) => {
            AdVariableValue::Scalar(seed(value, &[(0, step_scales[0])]))
        }
        VariableValue::Vec2(value) => AdVariableValue::Vec2([
            seed(value[0], &[(0, step_scales[0])]),
            seed(value[1], &[(1, step_scales[1])]),
        ]),
        VariableValue::Pose2(value) => {
            let pose = GeometryPose2::from_ambient(value).map_err(|error| {
                EvaluationError::invalid_geometry(format!("invalid Pose2 AD seed: {error}"))
            })?;
            let ambient = pose.ambient();
            let (sine, cosine) = pose.angle.sin_cos();
            AdVariableValue::Pose2([
                seed(
                    ambient[0],
                    &[(0, cosine * step_scales[0]), (1, -sine * step_scales[1])],
                ),
                seed(
                    ambient[1],
                    &[(0, sine * step_scales[0]), (1, cosine * step_scales[1])],
                ),
                seed(ambient[2], &[(2, step_scales[2])]),
            ])
        }
        VariableValue::Vec3(value) => AdVariableValue::Vec3([
            seed(value[0], &[(0, step_scales[0])]),
            seed(value[1], &[(1, step_scales[1])]),
            seed(value[2], &[(2, step_scales[2])]),
        ]),
        VariableValue::Pose3(value) => {
            let pose = GeometryPose3::from_ambient(value).map_err(|error| {
                EvaluationError::invalid_geometry(format!("invalid Pose3 AD seed: {error}"))
            })?;
            let ambient = pose.ambient();
            let rotation = pose.rotation().to_rotation_matrix();
            let rotation = rotation.matrix();
            let translation: [DualDVec64; 3] = std::array::from_fn(|row| {
                seed(
                    ambient[row],
                    &[
                        (0, rotation[(row, 0)] * step_scales[0]),
                        (1, rotation[(row, 1)] * step_scales[1]),
                        (2, rotation[(row, 2)] * step_scales[2]),
                    ],
                )
            });
            let [w, x, y, z] = pose.quaternion();
            let quaternion_differential = [
                [-0.5 * x, -0.5 * y, -0.5 * z],
                [0.5 * w, -0.5 * z, 0.5 * y],
                [0.5 * z, 0.5 * w, -0.5 * x],
                [-0.5 * y, 0.5 * x, 0.5 * w],
            ];
            let quaternion: [DualDVec64; 4] = std::array::from_fn(|row| {
                seed(
                    ambient[3 + row],
                    &[
                        (3, quaternion_differential[row][0] * step_scales[3]),
                        (4, quaternion_differential[row][1] * step_scales[4]),
                        (5, quaternion_differential[row][2] * step_scales[5]),
                    ],
                )
            });
            AdVariableValue::Pose3([
                translation[0].clone(),
                translation[1].clone(),
                translation[2].clone(),
                quaternion[0].clone(),
                quaternion[1].clone(),
                quaternion[2].clone(),
                quaternion[3].clone(),
            ])
        }
    })
}

pub(crate) fn fixed_pose_local_difference_jacobian(
    reference: VariableValue,
    value: VariableValue,
) -> Result<LocalJacobian, EvaluationError> {
    let dimension = value.kind().tangent_dimension();
    let scales = vec![1.0; dimension];
    let reference = constant_ad_value(reference, dimension)?;
    let value = retract_normalized_tangent(value, &scales, dimension, 0)?;
    let outputs = pose_local_difference_dual(&reference, &value)?;
    jacobian_from_dual(&outputs, dimension, 0)
}

pub(crate) fn alias_pose_local_difference_jacobians(
    alias: VariableValue,
    representative: VariableValue,
) -> Result<Vec<LocalJacobian>, EvaluationError> {
    let dimension = alias.kind().tangent_dimension();
    let width = 2 * dimension;
    let scales = vec![1.0; dimension];
    let alias = retract_normalized_tangent(alias, &scales, width, 0)?;
    let representative = retract_normalized_tangent(representative, &scales, width, dimension)?;
    let outputs = pose_local_difference_dual(&representative, &alias)?;
    Ok(vec![
        jacobian_from_dual(&outputs, dimension, 0)?,
        jacobian_from_dual(&outputs, dimension, dimension)?,
    ])
}

fn constant_ad_value(
    value: VariableValue,
    width: usize,
) -> Result<AdVariableValue, EvaluationError> {
    let constant = |real: f64| {
        let mut dual = DualDVec64::from_re(real).derivative(width, 0);
        dual.eps
            .0
            .as_mut()
            .expect("constant derivative storage")
            .fill(0.0);
        dual
    };
    match value {
        VariableValue::Pose2(ambient) => Ok(AdVariableValue::Pose2(ambient.map(constant))),
        VariableValue::Pose3(ambient) => {
            let pose = GeometryPose3::from_ambient(ambient).map_err(|error| {
                EvaluationError::invalid_geometry(format!(
                    "invalid Pose3 local-difference reference: {error}"
                ))
            })?;
            Ok(AdVariableValue::Pose3(pose.ambient().map(constant)))
        }
        _ => Err(EvaluationError::invalid_geometry(
            "pose local-difference AD requires Pose2 or Pose3",
        )),
    }
}

fn pose_local_difference_dual(
    reference: &AdVariableValue,
    value: &AdVariableValue,
) -> Result<Vec<DualDVec64>, EvaluationError> {
    match (reference, value) {
        (AdVariableValue::Pose2(reference), AdVariableValue::Pose2(value)) => {
            pose2_local_difference_dual(reference, value)
        }
        (AdVariableValue::Pose3(reference), AdVariableValue::Pose3(value)) => {
            pose3_local_difference_dual(reference, value)
        }
        _ => Err(EvaluationError::invalid_geometry(
            "pose local-difference AD kinds do not match",
        )),
    }
}

fn pose2_local_difference_dual(
    reference: &[DualDVec64; 3],
    value: &[DualDVec64; 3],
) -> Result<Vec<DualDVec64>, EvaluationError> {
    let translation_x = value[0].clone() - reference[0].clone();
    let translation_y = value[1].clone() - reference[1].clone();
    let (reference_sine, reference_cosine) = reference[2].sin_cos();
    let relative_translation_x = reference_cosine.clone() * translation_x.clone()
        + reference_sine.clone() * translation_y.clone();
    let relative_translation_y = -reference_sine * translation_x + reference_cosine * translation_y;
    let relative_angle = value[2].clone() - reference[2].clone();
    let (angle_sine, angle_cosine) = relative_angle.sin_cos();
    if angle_cosine.re < 0.0 && angle_sine.re.abs() <= QUATERNION_SIGN_TOLERANCE {
        return Err(EvaluationError::nondifferentiable(
            "Pose2 local difference is on the principal logarithm cut",
        ));
    }
    let angle = angle_sine.atan2(angle_cosine);
    let half_angle = angle.clone() * 0.5;
    let inverse_diagonal = if half_angle.re.abs() < 0.5e-4 {
        let squared = half_angle.clone() * half_angle.clone();
        let fourth = squared.clone() * squared.clone();
        DualDVec64::from_re(1.0)
            - squared.clone() / 3.0
            - fourth / 45.0
            - squared.clone() * squared.clone() * squared * (2.0 / 945.0)
    } else {
        half_angle.clone() / half_angle.tan()
    };
    Ok(vec![
        inverse_diagonal.clone() * relative_translation_x.clone()
            + half_angle.clone() * relative_translation_y.clone(),
        -half_angle * relative_translation_x + inverse_diagonal * relative_translation_y,
        angle,
    ])
}

fn pose3_local_difference_dual(
    reference: &[DualDVec64; 7],
    value: &[DualDVec64; 7],
) -> Result<Vec<DualDVec64>, EvaluationError> {
    let reference_quaternion = [
        reference[3].clone(),
        reference[4].clone(),
        reference[5].clone(),
        reference[6].clone(),
    ];
    let value_quaternion = [
        value[3].clone(),
        value[4].clone(),
        value[5].clone(),
        value[6].clone(),
    ];
    let inverse_reference_quaternion = [
        reference_quaternion[0].clone(),
        -reference_quaternion[1].clone(),
        -reference_quaternion[2].clone(),
        -reference_quaternion[3].clone(),
    ];
    let translation_delta = [
        value[0].clone() - reference[0].clone(),
        value[1].clone() - reference[1].clone(),
        value[2].clone() - reference[2].clone(),
    ];
    let relative_translation =
        dual_quaternion_rotate(&inverse_reference_quaternion, &translation_delta);
    let relative_quaternion =
        dual_quaternion_multiply(&inverse_reference_quaternion, &value_quaternion);
    let relative_quaternion = canonical_unit_dual_quaternion(relative_quaternion)?;
    let quaternion_vector = [
        relative_quaternion[1].clone(),
        relative_quaternion[2].clone(),
        relative_quaternion[3].clone(),
    ];
    let sine_half_squared = dual_dot(&quaternion_vector, &quaternion_vector);
    let sine_half = sine_half_squared.re.max(0.0).sqrt();
    let rotation_scale = if sine_half < 1.0e-4 {
        DualDVec64::from_re(2.0)
            + sine_half_squared.clone() / 3.0
            + sine_half_squared.clone() * sine_half_squared * (3.0 / 20.0)
    } else {
        let sine_half_dual = sine_half_squared.sqrt();
        sine_half_dual.atan2(relative_quaternion[0].clone()) * 2.0 / sine_half_dual
    };
    let angular = quaternion_vector.map(|component| component * rotation_scale.clone());
    let angle_squared = dual_dot(&angular, &angular);
    let angle = angle_squared.re.max(0.0).sqrt();
    let inverse_coefficient = if angle < 1.0e-4 {
        DualDVec64::from_re(1.0 / 12.0)
            + angle_squared.clone() / 720.0
            + angle_squared.clone() * angle_squared.clone() / 30_240.0
    } else {
        let angle_dual = angle_squared.sqrt();
        let half_angle = angle_dual.clone() * 0.5;
        (DualDVec64::from_re(1.0) - half_angle.clone() / half_angle.tan()) / angle_squared
    };
    let first_cross = dual_cross(&angular, &relative_translation);
    let second_cross = dual_cross(&angular, &first_cross);
    let velocity: [DualDVec64; 3] = std::array::from_fn(|index| {
        relative_translation[index].clone() - first_cross[index].clone() * 0.5
            + second_cross[index].clone() * inverse_coefficient.clone()
    });
    Ok(vec![
        velocity[0].clone(),
        velocity[1].clone(),
        velocity[2].clone(),
        angular[0].clone(),
        angular[1].clone(),
        angular[2].clone(),
    ])
}

fn canonical_unit_dual_quaternion(
    quaternion: [DualDVec64; 4],
) -> Result<[DualDVec64; 4], EvaluationError> {
    let norm_squared = quaternion
        .iter()
        .map(|component| component.clone() * component.clone())
        .sum::<DualDVec64>();
    let norm = norm_squared.sqrt();
    let mut quaternion = quaternion.map(|component| component / norm.clone());
    let flip = if quaternion[0].re < -QUATERNION_SIGN_TOLERANCE {
        true
    } else if quaternion[0].re > QUATERNION_SIGN_TOLERANCE {
        false
    } else {
        quaternion[1..]
            .iter()
            .find(|component| component.re.abs() > QUATERNION_SIGN_TOLERANCE)
            .is_some_and(|component| component.re.is_sign_negative())
    };
    if flip {
        quaternion = quaternion.map(|component| -component);
    }
    if quaternion[0].re.abs() <= QUATERNION_SIGN_TOLERANCE {
        return Err(EvaluationError::nondifferentiable(
            "Pose3 local difference is on the principal logarithm cut",
        ));
    }
    Ok(quaternion)
}

fn dual_quaternion_multiply(first: &[DualDVec64; 4], second: &[DualDVec64; 4]) -> [DualDVec64; 4] {
    let [first_w, first_x, first_y, first_z] = first;
    let [second_w, second_x, second_y, second_z] = second;
    [
        first_w.clone() * second_w.clone()
            - first_x.clone() * second_x.clone()
            - first_y.clone() * second_y.clone()
            - first_z.clone() * second_z.clone(),
        first_w.clone() * second_x.clone()
            + first_x.clone() * second_w.clone()
            + first_y.clone() * second_z.clone()
            - first_z.clone() * second_y.clone(),
        first_w.clone() * second_y.clone() - first_x.clone() * second_z.clone()
            + first_y.clone() * second_w.clone()
            + first_z.clone() * second_x.clone(),
        first_w.clone() * second_z.clone() + first_x.clone() * second_y.clone()
            - first_y.clone() * second_x.clone()
            + first_z.clone() * second_w.clone(),
    ]
}

fn dual_quaternion_rotate(
    quaternion: &[DualDVec64; 4],
    vector: &[DualDVec64; 3],
) -> [DualDVec64; 3] {
    let quaternion_vector = [
        quaternion[1].clone(),
        quaternion[2].clone(),
        quaternion[3].clone(),
    ];
    let first_cross = dual_cross(&quaternion_vector, vector);
    let second_cross = dual_cross(&quaternion_vector, &first_cross);
    std::array::from_fn(|index| {
        vector[index].clone()
            + (quaternion[0].clone() * first_cross[index].clone() + second_cross[index].clone())
                * 2.0
    })
}

fn dual_cross(first: &[DualDVec64; 3], second: &[DualDVec64; 3]) -> [DualDVec64; 3] {
    [
        first[1].clone() * second[2].clone() - first[2].clone() * second[1].clone(),
        first[2].clone() * second[0].clone() - first[0].clone() * second[2].clone(),
        first[0].clone() * second[1].clone() - first[1].clone() * second[0].clone(),
    ]
}

fn dual_dot(first: &[DualDVec64; 3], second: &[DualDVec64; 3]) -> DualDVec64 {
    first
        .iter()
        .zip(second)
        .map(|(first, second)| first.clone() * second.clone())
        .sum()
}

fn jacobian_from_dual(
    outputs: &[DualDVec64],
    columns: usize,
    offset: usize,
) -> Result<LocalJacobian, EvaluationError> {
    let values = outputs
        .iter()
        .flat_map(|output| (0..columns).map(move |column| derivative(output, offset + column)))
        .collect::<Vec<_>>();
    if outputs.iter().any(|output| !output.re.is_finite())
        || values.iter().any(|value| !value.is_finite())
    {
        return Err(EvaluationError::invalid_geometry(
            "pose local-difference AD produced a non-finite value or derivative",
        ));
    }
    Ok(LocalJacobian::new(outputs.len(), columns, values))
}

fn derivative(value: &DualDVec64, index: usize) -> f64 {
    value
        .eps
        .0
        .as_ref()
        .map_or(0.0, |derivatives| derivatives[index])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        AuditBinding, Problem, ResidualBlock, ResidualCategory, ResidualRowAudit, SourceConstraint,
        VariableBlock,
    };

    #[derive(Clone, Debug)]
    struct MixedFormula {
        scale: f64,
    }

    impl LocalAdFormula for MixedFormula {
        fn evaluate(
            &self,
            variables: &[AdVariableValue],
        ) -> Result<Vec<DualDVec64>, EvaluationError> {
            let [
                AdVariableValue::Scalar(scalar),
                AdVariableValue::Vec2(vector),
                AdVariableValue::Pose2(pose),
            ] = variables
            else {
                return Err(EvaluationError::invalid_geometry(
                    "mixed AD formula expected Scalar, Vec2, and Pose2",
                ));
            };
            let angle_cosine = pose[2].clone().cos();
            let first = scalar * scalar
                + vector[0].clone() * angle_cosine * self.scale
                + &pose[0] * &pose[1];
            let difference = &vector[1] - &pose[1];
            let second = &difference * &difference
                + (pose[2].clone() + scalar.clone() / self.scale).sin() * (self.scale * self.scale)
                + pose[0].clone() * self.scale;
            Ok(vec![first, second])
        }
    }

    #[derive(Clone, Debug)]
    struct MixedAnalytic {
        scale: f64,
    }

    impl ResidualEvaluator for MixedAnalytic {
        fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
            let [
                VariableValue::Scalar(scalar),
                VariableValue::Vec2(vector),
                VariableValue::Pose2(pose),
            ] = variables
            else {
                return Err(EvaluationError::invalid_geometry(
                    "mixed analytic formula expected Scalar, Vec2, and Pose2",
                ));
            };
            let difference = vector[1] - pose[1];
            Ok(vec![
                scalar * scalar + self.scale * vector[0] * pose[2].cos() + pose[0] * pose[1],
                difference * difference
                    + self.scale * self.scale * (pose[2] + scalar / self.scale).sin()
                    + self.scale * pose[0],
            ])
        }

        fn jacobian(
            &self,
            variables: &[VariableValue],
        ) -> Result<Vec<LocalJacobian>, EvaluationError> {
            let [
                VariableValue::Scalar(scalar),
                VariableValue::Vec2(vector),
                VariableValue::Pose2(pose),
            ] = variables
            else {
                return Err(EvaluationError::invalid_geometry(
                    "mixed analytic formula expected Scalar, Vec2, and Pose2",
                ));
            };
            let difference = vector[1] - pose[1];
            let coupled_cosine = (pose[2] + scalar / self.scale).cos();
            let (pose_sine, pose_cosine) = pose[2].sin_cos();
            let first_x = pose[1];
            let first_y = pose[0];
            let second_x = self.scale;
            let second_y = -2.0 * difference;
            Ok(vec![
                LocalJacobian::new(2, 1, vec![2.0 * scalar, self.scale * coupled_cosine]),
                LocalJacobian::new(
                    2,
                    2,
                    vec![self.scale * pose[2].cos(), 0.0, 0.0, 2.0 * difference],
                ),
                LocalJacobian::new(
                    2,
                    3,
                    vec![
                        first_x * pose_cosine + first_y * pose_sine,
                        -first_x * pose_sine + first_y * pose_cosine,
                        -self.scale * vector[0] * pose[2].sin(),
                        second_x * pose_cosine + second_y * pose_sine,
                        -second_x * pose_sine + second_y * pose_cosine,
                        self.scale * self.scale * coupled_cosine,
                    ],
                ),
            ])
        }
    }

    fn row(name: &str) -> ResidualRowAudit {
        ResidualRowAudit::new(
            name,
            vec![AuditBinding::new("variables", "mixed AD fixture")],
            "scale squared",
        )
    }

    fn mixed_problem(scale: f64, ad: bool) -> Problem {
        let mut problem = Problem::new();
        let scalar = problem.add_variable(VariableBlock::scalar(0.4 * scale, scale).unwrap());
        let vector = problem.add_variable(
            VariableBlock::vec2([0.7 * scale, -0.2 * scale], [scale, scale]).unwrap(),
        );
        let pose = problem.add_variable(
            VariableBlock::pose2([0.3 * scale, -0.6 * scale, 0.35], [scale, scale, 1.0]).unwrap(),
        );
        let source = problem.add_source(SourceConstraint::new("mixed local AD").unwrap());
        let evaluator: Box<dyn ResidualEvaluator> = if ad {
            Box::new(LocalAdEvaluator::new(MixedFormula { scale }))
        } else {
            Box::new(MixedAnalytic { scale })
        };
        problem
            .add_residual(
                ResidualBlock::new(
                    source,
                    ResidualCategory::Hard,
                    vec![scalar, vector, pose],
                    2,
                    vec![scale * scale; 2],
                    vec![row("mixed row zero"), row("mixed row one")],
                    evaluator,
                )
                .unwrap(),
            )
            .unwrap();
        problem
    }

    impl ResidualEvaluator for Box<dyn ResidualEvaluator> {
        fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
            self.as_ref().evaluate(variables)
        }

        fn jacobian(
            &self,
            variables: &[VariableValue],
        ) -> Result<Vec<LocalJacobian>, EvaluationError> {
            self.as_ref().jacobian(variables)
        }

        fn linearize(
            &self,
            variables: &[VariableValue],
            storage: &mut LinearizationStorage<'_, '_>,
        ) -> Option<Result<(), EvaluationError>> {
            self.as_ref().linearize(variables, storage)
        }
    }

    #[test]
    fn mixed_local_ad_matches_analytic_and_central_difference_at_all_scales() {
        for scale in [1.0e-6, 1.0, 1.0e6] {
            let ad = mixed_problem(scale, true);
            let analytic = mixed_problem(scale, false);
            let ad_dense = ad.assemble_dense().unwrap();
            let analytic_dense = analytic.assemble_dense().unwrap();
            assert_eq!(ad_dense.residuals().len(), analytic_dense.residuals().len());
            for (actual, expected) in ad_dense.residuals().iter().zip(analytic_dense.residuals()) {
                assert!((actual - expected).abs() <= 2.0e-14, "scale={scale:e}");
            }
            for (actual, expected) in ad_dense.jacobian().iter().zip(analytic_dense.jacobian()) {
                assert!((actual - expected).abs() <= 2.0e-14, "scale={scale:e}");
            }
            let ad_fd = ad.check_jacobians(1.0e-6).unwrap();
            let analytic_fd = analytic.check_jacobians(1.0e-6).unwrap();
            assert!(ad_fd.all_within(1.0e-6), "scale={scale:e}: {ad_fd:#?}");
            assert!(
                analytic_fd.all_within(1.0e-6),
                "scale={scale:e}: {analytic_fd:#?}"
            );
        }
    }

    #[derive(Clone, Copy, Debug)]
    struct SpatialSignInvariantFormula;

    impl LocalAdFormula for SpatialSignInvariantFormula {
        fn evaluate(
            &self,
            variables: &[AdVariableValue],
        ) -> Result<Vec<DualDVec64>, EvaluationError> {
            let [AdVariableValue::Vec3(vector), AdVariableValue::Pose3(pose)] = variables else {
                return Err(EvaluationError::invalid_geometry(
                    "spatial AD formula expected Vec3 and Pose3",
                ));
            };
            let quaternion = [
                pose[3].clone(),
                pose[4].clone(),
                pose[5].clone(),
                pose[6].clone(),
            ];
            let rotated = dual_quaternion_rotate(&quaternion, vector);
            let first = pose[0].clone() + rotated[0].clone() * 2.0 - rotated[1].clone()
                + rotated[2].clone() * 0.5;
            let quaternion_quadratic = quaternion[0].clone() * quaternion[2].clone()
                + quaternion[1].clone() * quaternion[3].clone();
            let second = (pose[1].clone() + rotated[1].clone() + quaternion_quadratic).sin()
                + pose[2].clone();
            Ok(vec![first, second])
        }
    }

    #[test]
    fn sign_invariant_pose3_local_ad_matches_finite_differences_at_exact_half_turn() {
        let mut problem = Problem::new();
        let vector =
            problem.add_variable(VariableBlock::vec3([0.4, -0.7, 1.2], [0.3, 0.8, 1.1]).unwrap());
        let inverse_axis_norm = 1.0 / 6.0_f64.sqrt();
        let pose = GeometryPose3::exp([
            1.0,
            -2.0,
            0.5,
            std::f64::consts::PI * inverse_axis_norm,
            2.0 * std::f64::consts::PI * inverse_axis_norm,
            -std::f64::consts::PI * inverse_axis_norm,
        ])
        .unwrap();
        let pose = problem.add_variable(
            VariableBlock::pose3(pose.ambient(), [0.5, 0.7, 1.3, 0.2, 0.4, 0.6]).unwrap(),
        );
        let source = problem.add_source(SourceConstraint::new("spatial local AD").unwrap());
        problem
            .add_residual(
                ResidualBlock::new(
                    source,
                    ResidualCategory::Hard,
                    vec![vector, pose],
                    2,
                    vec![2.0, 0.5],
                    vec![row("spatial row zero"), row("spatial row one")],
                    LocalAdEvaluator::new(SpatialSignInvariantFormula),
                )
                .unwrap(),
            )
            .unwrap();

        let assembly = problem.assemble_dense().unwrap();
        assert_eq!(assembly.jacobian().shape(), (2, 9));
        assert!(assembly.jacobian().iter().all(|value| value.is_finite()));
        let oracle = problem.check_jacobians(3.0e-6).unwrap();
        assert!(oracle.all_within(1.0e-6), "{oracle:#?}");
    }

    #[derive(Clone, Copy, Debug)]
    struct TinyScaleAtan {
        scale: f64,
    }

    impl LocalAdFormula for TinyScaleAtan {
        fn evaluate(
            &self,
            variables: &[AdVariableValue],
        ) -> Result<Vec<DualDVec64>, EvaluationError> {
            let [AdVariableValue::Scalar(value)] = variables else {
                return Err(EvaluationError::invalid_geometry(
                    "tiny-scale AD formula expected one scalar",
                ));
            };
            Ok(vec![(value.clone() / self.scale).atan()])
        }
    }

    #[test]
    fn normalized_ad_derivative_does_not_require_nonfinite_raw_intermediate() {
        let scale = 1.0e-310;
        let mut problem = Problem::new();
        let variable = problem.add_variable(VariableBlock::scalar(0.0, scale).unwrap());
        let source = problem.add_source(SourceConstraint::new("tiny normalized AD").unwrap());
        problem
            .add_residual(
                ResidualBlock::new(
                    source,
                    ResidualCategory::Hard,
                    vec![variable],
                    1,
                    vec![1.0],
                    vec![row("atan(x / scale)")],
                    LocalAdEvaluator::new(TinyScaleAtan { scale }),
                )
                .unwrap(),
            )
            .unwrap();

        let dense = problem.assemble_dense().unwrap();
        assert_eq!(dense.jacobian().nrows(), 1);
        assert_eq!(dense.jacobian().ncols(), 1);
        assert_eq!(dense.jacobian()[(0, 0)].to_bits(), 1.0_f64.to_bits());
        let finite_difference = problem.check_jacobians(1.0e-6).unwrap();
        assert!(
            finite_difference.all_within(1.0e-6),
            "{finite_difference:#?}"
        );
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum FormulaBranch {
        Positive,
        Negative,
    }

    impl FormulaBranch {
        const fn multiplier(self) -> f64 {
            match self {
                Self::Positive => 1.0,
                Self::Negative => -1.0,
            }
        }
    }

    #[derive(Clone, Copy, Debug)]
    struct BranchedScalarTarget {
        target: f64,
        branch: FormulaBranch,
    }

    impl LocalAdFormula for BranchedScalarTarget {
        fn evaluate(
            &self,
            variables: &[AdVariableValue],
        ) -> Result<Vec<DualDVec64>, EvaluationError> {
            let [AdVariableValue::Scalar(value)] = variables else {
                return Err(EvaluationError::invalid_geometry(
                    "branched AD formula expected one scalar",
                ));
            };
            Ok(vec![value.clone() * self.branch.multiplier() - self.target])
        }
    }

    fn branched_target_problem(value: f64, target: f64, branch: FormulaBranch) -> Problem {
        let mut problem = Problem::new();
        let variable = problem.add_variable(VariableBlock::scalar(value, 1.0).unwrap());
        let source = problem.add_source(SourceConstraint::new("branched AD target").unwrap());
        problem
            .add_residual(
                ResidualBlock::new(
                    source,
                    ResidualCategory::Hard,
                    vec![variable],
                    1,
                    vec![1.0],
                    vec![row("branch * x - target")],
                    LocalAdEvaluator::new(BranchedScalarTarget { target, branch }),
                )
                .unwrap(),
            )
            .unwrap();
        problem
    }

    #[test]
    fn local_ad_solves_exact_and_perturbed_states_without_changing_discrete_formula_branch() {
        for (branch, expected) in [
            (FormulaBranch::Positive, 2.0),
            (FormulaBranch::Negative, -2.0),
        ] {
            let mut exact = branched_target_problem(expected, 2.0, branch);
            let exact_report = exact.solve(crate::SolverConfig::default()).unwrap();
            assert_eq!(exact_report.hard_validity, crate::HardValidity::Valid);
            assert_eq!(
                exact_report.accepted_state.ambient()[0].to_bits(),
                expected.to_bits()
            );
            assert!(exact.check_jacobians(1.0e-6).unwrap().all_within(1.0e-6));

            let mut perturbed = branched_target_problem(expected + 0.25, 2.0, branch);
            let recovered = perturbed.solve(crate::SolverConfig::default()).unwrap();
            assert_eq!(recovered.hard_validity, crate::HardValidity::Valid);
            assert!((recovered.accepted_state.ambient()[0] - expected).abs() <= 1.0e-9);
            assert!(
                perturbed
                    .check_jacobians(1.0e-6)
                    .unwrap()
                    .all_within(1.0e-6)
            );
        }
    }

    #[derive(Clone, Copy, Debug)]
    struct PositiveDomain;

    impl LocalAdFormula for PositiveDomain {
        fn evaluate(
            &self,
            variables: &[AdVariableValue],
        ) -> Result<Vec<DualDVec64>, EvaluationError> {
            let [AdVariableValue::Scalar(value)] = variables else {
                return Err(EvaluationError::invalid_geometry(
                    "positive-domain AD formula expected one scalar",
                ));
            };
            if value.re < 0.0 {
                return Err(EvaluationError::out_of_domain(
                    "AD scalar left its positive domain",
                ));
            }
            Ok(vec![value.clone()])
        }
    }

    #[test]
    fn categorized_local_ad_failure_rolls_back_without_losing_category() {
        let mut problem = Problem::new();
        let variable = problem.add_variable(VariableBlock::scalar(-1.0, 1.0).unwrap());
        let source = problem.add_source(SourceConstraint::new("AD domain failure").unwrap());
        problem
            .add_residual(
                ResidualBlock::new(
                    source,
                    ResidualCategory::Hard,
                    vec![variable],
                    1,
                    vec![1.0],
                    vec![row("positive-domain scalar")],
                    LocalAdEvaluator::new(PositiveDomain),
                )
                .unwrap(),
            )
            .unwrap();
        let initial = problem.packed_state().unwrap();

        let report = problem.solve(crate::SolverConfig::default()).unwrap();

        assert_eq!(report.termination, crate::SolveTermination::InvalidGeometry);
        assert_eq!(report.hard_validity, crate::HardValidity::Invalid);
        assert_eq!(report.accepted_state, initial);
        let audit_row = &report.audit.sources[0].rows[0];
        assert_eq!(
            audit_row.evaluation_status,
            crate::AuditEvaluationStatus::Failed
        );
        assert_eq!(
            audit_row.evaluation_error_category,
            Some(crate::EvaluationErrorCategory::OutOfDomain)
        );
    }
}
