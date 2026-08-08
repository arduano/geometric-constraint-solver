use geosolve_geometry::{
    Pose2 as GeometryPose2, Pose3 as GeometryPose3, QUATERNION_SIGN_TOLERANCE,
};
use num_dual::{DualDVec64, DualNum};

use crate::{EvaluationError, LocalJacobian, VariableValue};

enum AdPoseValue {
    Pose2([DualDVec64; 3]),
    Pose3([DualDVec64; 7]),
}

// Each ambient dual is seeded with the differential at zero of the same pose
// retraction used by VariableValue::plus.
fn retract_pose_tangent(
    value: VariableValue,
    step_scales: &[f64],
    width: usize,
    offset: usize,
) -> Result<AdPoseValue, EvaluationError> {
    let dimension = value.kind().tangent_dimension();
    if step_scales.len() != dimension || offset.checked_add(dimension).is_none_or(|end| end > width)
    {
        return Err(EvaluationError::invalid_geometry(
            "pose AD tangent scales do not match the derivative storage",
        ));
    }
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
        VariableValue::Pose2(value) => {
            let pose = GeometryPose2::from_ambient(value).map_err(|error| {
                EvaluationError::invalid_geometry(format!("invalid Pose2 AD seed: {error}"))
            })?;
            let ambient = pose.ambient();
            let (sine, cosine) = pose.angle.sin_cos();
            AdPoseValue::Pose2([
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
            AdPoseValue::Pose3([
                translation[0].clone(),
                translation[1].clone(),
                translation[2].clone(),
                quaternion[0].clone(),
                quaternion[1].clone(),
                quaternion[2].clone(),
                quaternion[3].clone(),
            ])
        }
        _ => {
            return Err(EvaluationError::invalid_geometry(
                "pose local-difference AD requires Pose2 or Pose3",
            ));
        }
    })
}

pub(crate) fn fixed_pose_local_difference_jacobian(
    reference: VariableValue,
    value: VariableValue,
) -> Result<LocalJacobian, EvaluationError> {
    let dimension = value.kind().tangent_dimension();
    let scales = vec![1.0; dimension];
    let reference = constant_ad_pose(reference, dimension)?;
    let value = retract_pose_tangent(value, &scales, dimension, 0)?;
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
    let alias = retract_pose_tangent(alias, &scales, width, 0)?;
    let representative = retract_pose_tangent(representative, &scales, width, dimension)?;
    let outputs = pose_local_difference_dual(&representative, &alias)?;
    Ok(vec![
        jacobian_from_dual(&outputs, dimension, 0)?,
        jacobian_from_dual(&outputs, dimension, dimension)?,
    ])
}

fn constant_ad_pose(value: VariableValue, width: usize) -> Result<AdPoseValue, EvaluationError> {
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
        VariableValue::Pose2(ambient) => Ok(AdPoseValue::Pose2(ambient.map(constant))),
        VariableValue::Pose3(ambient) => {
            let pose = GeometryPose3::from_ambient(ambient).map_err(|error| {
                EvaluationError::invalid_geometry(format!(
                    "invalid Pose3 local-difference reference: {error}"
                ))
            })?;
            Ok(AdPoseValue::Pose3(pose.ambient().map(constant)))
        }
        _ => Err(EvaluationError::invalid_geometry(
            "pose local-difference AD requires Pose2 or Pose3",
        )),
    }
}

fn pose_local_difference_dual(
    reference: &AdPoseValue,
    value: &AdPoseValue,
) -> Result<Vec<DualDVec64>, EvaluationError> {
    match (reference, value) {
        (AdPoseValue::Pose2(reference), AdPoseValue::Pose2(value)) => {
            pose2_local_difference_dual(reference, value)
        }
        (AdPoseValue::Pose3(reference), AdPoseValue::Pose3(value)) => {
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
