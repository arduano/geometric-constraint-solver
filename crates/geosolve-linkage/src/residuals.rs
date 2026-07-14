use geosolve_core::{EvaluationError, LocalJacobian, ResidualEvaluator, VariableValue};

#[derive(Clone, Copy, Debug)]
pub(crate) struct RevoluteResidual {
    pub(crate) first_local: [f64; 2],
    pub(crate) second_local: [f64; 2],
}

impl ResidualEvaluator for RevoluteResidual {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        let (first, second) = two_poses(variables, "revolute")?;
        let first_point = transformed_point(first, self.first_local);
        let second_point = transformed_point(second, self.second_local);
        Ok(vec![
            second_point[0] - first_point[0],
            second_point[1] - first_point[1],
        ])
    }

    fn jacobian(&self, variables: &[VariableValue]) -> Result<Vec<LocalJacobian>, EvaluationError> {
        let (first, second) = two_poses(variables, "revolute")?;
        let first_rotated = rotated(first[2], self.first_local);
        let second_rotated = rotated(second[2], self.second_local);
        let first_angle = perpendicular(first_rotated);
        let second_angle = perpendicular(second_rotated);
        Ok(vec![
            LocalJacobian::new(
                2,
                3,
                vec![-1.0, 0.0, -first_angle[0], 0.0, -1.0, -first_angle[1]],
            ),
            LocalJacobian::new(
                2,
                3,
                vec![1.0, 0.0, second_angle[0], 0.0, 1.0, second_angle[1]],
            ),
        ])
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct PrismaticResidual {
    pub(crate) first_anchor: [f64; 2],
    pub(crate) first_axis: [f64; 2],
    pub(crate) second_anchor: [f64; 2],
    pub(crate) second_axis: [f64; 2],
    pub(crate) branch_multiplier: f64,
}

impl ResidualEvaluator for PrismaticResidual {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        let (first, second) = two_poses(variables, "prismatic")?;
        let first_point = transformed_point(first, self.first_anchor);
        let second_point = transformed_point(second, self.second_anchor);
        let first_axis = rotated(first[2], self.first_axis);
        let second_axis = scale(rotated(second[2], self.second_axis), self.branch_multiplier);
        let normal = perpendicular(first_axis);
        let displacement = subtract(second_point, first_point);
        Ok(vec![
            dot(normal, displacement),
            cross(first_axis, second_axis),
        ])
    }

    fn jacobian(&self, variables: &[VariableValue]) -> Result<Vec<LocalJacobian>, EvaluationError> {
        let (first, second) = two_poses(variables, "prismatic")?;
        let first_rotated_point = rotated(first[2], self.first_anchor);
        let second_rotated_point = rotated(second[2], self.second_anchor);
        let first_point = add_translation(first, first_rotated_point);
        let second_point = add_translation(second, second_rotated_point);
        let displacement = subtract(second_point, first_point);
        let first_axis = rotated(first[2], self.first_axis);
        let second_axis = scale(rotated(second[2], self.second_axis), self.branch_multiplier);
        let normal = perpendicular(first_axis);
        let first_point_angle = perpendicular(first_rotated_point);
        let second_point_angle = perpendicular(second_rotated_point);
        let first_transverse_angle =
            -dot(first_axis, displacement) - dot(normal, first_point_angle);
        let second_transverse_angle = dot(normal, second_point_angle);
        let first_alignment_angle = cross(perpendicular(first_axis), second_axis);
        let second_alignment_angle = cross(first_axis, perpendicular(second_axis));

        Ok(vec![
            LocalJacobian::new(
                2,
                3,
                vec![
                    -normal[0],
                    -normal[1],
                    first_transverse_angle,
                    0.0,
                    0.0,
                    first_alignment_angle,
                ],
            ),
            LocalJacobian::new(
                2,
                3,
                vec![
                    normal[0],
                    normal[1],
                    second_transverse_angle,
                    0.0,
                    0.0,
                    second_alignment_angle,
                ],
            ),
        ])
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct WeldResidual {
    pub(crate) first_local: [f64; 2],
    pub(crate) second_local: [f64; 2],
    pub(crate) relative_angle: f64,
}

impl ResidualEvaluator for WeldResidual {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        let (first, second) = two_poses(variables, "weld")?;
        let first_point = transformed_point(first, self.first_local);
        let second_point = transformed_point(second, self.second_local);
        Ok(vec![
            second_point[0] - first_point[0],
            second_point[1] - first_point[1],
            second[2] - first[2] - self.relative_angle,
        ])
    }

    fn jacobian(&self, variables: &[VariableValue]) -> Result<Vec<LocalJacobian>, EvaluationError> {
        let (first, second) = two_poses(variables, "weld")?;
        let first_angle = perpendicular(rotated(first[2], self.first_local));
        let second_angle = perpendicular(rotated(second[2], self.second_local));
        Ok(vec![
            LocalJacobian::new(
                3,
                3,
                vec![
                    -1.0,
                    0.0,
                    -first_angle[0],
                    0.0,
                    -1.0,
                    -first_angle[1],
                    0.0,
                    0.0,
                    -1.0,
                ],
            ),
            LocalJacobian::new(
                3,
                3,
                vec![
                    1.0,
                    0.0,
                    second_angle[0],
                    0.0,
                    1.0,
                    second_angle[1],
                    0.0,
                    0.0,
                    1.0,
                ],
            ),
        ])
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct AngularDriverResidual {
    pub(crate) target: f64,
}

impl ResidualEvaluator for AngularDriverResidual {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        let (reference, driven) = two_poses(variables, "angular driver")?;
        Ok(vec![driven[2] - reference[2] - self.target])
    }

    fn jacobian(&self, variables: &[VariableValue]) -> Result<Vec<LocalJacobian>, EvaluationError> {
        two_poses(variables, "angular driver")?;
        Ok(vec![
            LocalJacobian::new(1, 3, vec![0.0, 0.0, -1.0]),
            LocalJacobian::new(1, 3, vec![0.0, 0.0, 1.0]),
        ])
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct LinearDriverResidual {
    pub(crate) origin_local: [f64; 2],
    pub(crate) measured_local: [f64; 2],
    pub(crate) guide_axis: [f64; 2],
    pub(crate) target: f64,
}

impl ResidualEvaluator for LinearDriverResidual {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        let (reference, measured) = two_poses(variables, "linear driver")?;
        let origin = transformed_point(reference, self.origin_local);
        let measured = transformed_point(measured, self.measured_local);
        let guide = rotated(reference[2], self.guide_axis);
        Ok(vec![dot(guide, subtract(measured, origin)) - self.target])
    }

    fn jacobian(&self, variables: &[VariableValue]) -> Result<Vec<LocalJacobian>, EvaluationError> {
        let (reference, measured) = two_poses(variables, "linear driver")?;
        let origin_rotated = rotated(reference[2], self.origin_local);
        let measured_rotated = rotated(measured[2], self.measured_local);
        let origin = add_translation(reference, origin_rotated);
        let measured_point = add_translation(measured, measured_rotated);
        let displacement = subtract(measured_point, origin);
        let guide = rotated(reference[2], self.guide_axis);
        let reference_angle =
            dot(perpendicular(guide), displacement) - dot(guide, perpendicular(origin_rotated));
        let measured_angle = dot(guide, perpendicular(measured_rotated));
        Ok(vec![
            LocalJacobian::new(1, 3, vec![-guide[0], -guide[1], reference_angle]),
            LocalJacobian::new(1, 3, vec![guide[0], guide[1], measured_angle]),
        ])
    }
}

fn two_poses(
    variables: &[VariableValue],
    context: &str,
) -> Result<([f64; 3], [f64; 3]), EvaluationError> {
    let [VariableValue::Pose2(first), VariableValue::Pose2(second)] = variables else {
        return Err(EvaluationError::invalid_geometry(format!(
            "{context} residual expected two Pose2 variables"
        )));
    };
    Ok((*first, *second))
}

pub(crate) fn rotated(angle: f64, local: [f64; 2]) -> [f64; 2] {
    let cosine = angle.cos();
    let sine = angle.sin();
    [
        cosine * local[0] - sine * local[1],
        sine * local[0] + cosine * local[1],
    ]
}

fn transformed_point(pose: [f64; 3], local: [f64; 2]) -> [f64; 2] {
    add_translation(pose, rotated(pose[2], local))
}

fn add_translation(pose: [f64; 3], vector: [f64; 2]) -> [f64; 2] {
    [pose[0] + vector[0], pose[1] + vector[1]]
}

pub(crate) const fn perpendicular(vector: [f64; 2]) -> [f64; 2] {
    [-vector[1], vector[0]]
}

pub(crate) const fn subtract(first: [f64; 2], second: [f64; 2]) -> [f64; 2] {
    [first[0] - second[0], first[1] - second[1]]
}

pub(crate) const fn dot(first: [f64; 2], second: [f64; 2]) -> f64 {
    first[0] * second[0] + first[1] * second[1]
}

pub(crate) const fn cross(first: [f64; 2], second: [f64; 2]) -> f64 {
    first[0] * second[1] - first[1] * second[0]
}

const fn scale(vector: [f64; 2], multiplier: f64) -> [f64; 2] {
    [vector[0] * multiplier, vector[1] * multiplier]
}
