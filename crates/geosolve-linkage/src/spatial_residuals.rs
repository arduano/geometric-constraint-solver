use geosolve_core::{EvaluationError, LocalJacobian, ResidualEvaluator, VariableValue};
use geosolve_geometry::{Frame3, Matrix3, Point3, Pose3, SMatrix, Vector3};

const TANGENT_DIMENSION: usize = 6;

#[derive(Clone, Copy, Debug)]
pub(crate) struct SpatialBallResidual {
    pub(crate) first_local: [f64; 3],
    pub(crate) second_local: [f64; 3],
}

impl ResidualEvaluator for SpatialBallResidual {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        let (first, second) = two_poses(variables, "spatial ball")?;
        let first_world = transform_point(first, self.first_local, "spatial ball first point")?;
        let second_world = transform_point(second, self.second_local, "spatial ball second point")?;
        checked_residual(
            (second_world - first_world).as_slice().to_vec(),
            "spatial ball",
        )
    }

    fn jacobian(&self, variables: &[VariableValue]) -> Result<Vec<LocalJacobian>, EvaluationError> {
        let (first, second) = two_poses(variables, "spatial ball")?;
        let first_point = point(self.first_local);
        let second_point = point(self.second_local);
        let first_derivative = point_derivative(first, first_point, "spatial ball first point")?;
        let second_derivative =
            point_derivative(second, second_point, "spatial ball second point")?;

        let mut first_values = Vec::with_capacity(3 * TANGENT_DIMENSION);
        let mut second_values = Vec::with_capacity(3 * TANGENT_DIMENSION);
        push_point_rows(&mut first_values, &first_derivative, -1.0);
        push_point_rows(&mut second_values, &second_derivative, 1.0);
        checked_jacobians(3, first_values, second_values, "spatial ball")
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct SpatialPointDistanceResidual {
    pub(crate) first_local: [f64; 3],
    pub(crate) second_local: [f64; 3],
    pub(crate) distance: f64,
}

impl ResidualEvaluator for SpatialPointDistanceResidual {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        validate_positive_distance(self.distance)?;
        let (first, second) = two_poses(variables, "spatial point distance")?;
        let first_world = transform_point(
            first,
            self.first_local,
            "spatial point distance first point",
        )?;
        let second_world = transform_point(
            second,
            self.second_local,
            "spatial point distance second point",
        )?;
        let (_, separation) =
            regular_point_separation(first_world, second_world, "spatial point distance")?;
        checked_residual(vec![separation - self.distance], "spatial point distance")
    }

    fn jacobian(&self, variables: &[VariableValue]) -> Result<Vec<LocalJacobian>, EvaluationError> {
        validate_positive_distance(self.distance)?;
        let (first, second) = two_poses(variables, "spatial point distance")?;
        let first_point = point(self.first_local);
        let second_point = point(self.second_local);
        let first_world = first
            .try_transform_point(first_point)
            .map_err(|error| geometry_error("spatial point distance first point", error))?;
        let second_world = second
            .try_transform_point(second_point)
            .map_err(|error| geometry_error("spatial point distance second point", error))?;
        let (direction, _) =
            regular_point_separation(first_world, second_world, "spatial point distance")?;
        let first_derivative =
            point_derivative(first, first_point, "spatial point distance first point")?;
        let second_derivative =
            point_derivative(second, second_point, "spatial point distance second point")?;
        let mut first_values = Vec::with_capacity(TANGENT_DIMENSION);
        let mut second_values = Vec::with_capacity(TANGENT_DIMENSION);
        for column in 0..TANGENT_DIMENSION {
            first_values.push(-direction.dot(&first_derivative.column(column)));
            second_values.push(direction.dot(&second_derivative.column(column)));
        }
        checked_jacobians(1, first_values, second_values, "spatial point distance")
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct SpatialAxisAngleResidual {
    pub(crate) first_local: Frame3,
    pub(crate) second_local: Frame3,
    pub(crate) angle: f64,
}

impl ResidualEvaluator for SpatialAxisAngleResidual {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        validate_interior_angle(self.angle)?;
        let frames = frame_pair(
            variables,
            self.first_local,
            self.second_local,
            "spatial axis angle",
        )?;
        checked_residual(
            vec![
                frames
                    .first_world
                    .z_axis()
                    .dot(&frames.second_world.z_axis())
                    - self.angle.cos(),
            ],
            "spatial axis angle",
        )
    }

    fn jacobian(&self, variables: &[VariableValue]) -> Result<Vec<LocalJacobian>, EvaluationError> {
        validate_interior_angle(self.angle)?;
        let frames = frame_pair(
            variables,
            self.first_local,
            self.second_local,
            "spatial axis angle",
        )?;
        require_regular_axis_angle(
            frames.first_world.z_axis(),
            frames.second_world.z_axis(),
            "spatial axis angle",
        )?;
        let (first, second) = orientation_derivatives(
            frames.first_pose,
            frames.second_pose,
            frames.first_local.z_axis(),
            frames.second_local.z_axis(),
            "spatial axis angle",
        )?;
        let mut first_values = Vec::with_capacity(TANGENT_DIMENSION);
        let mut second_values = Vec::with_capacity(TANGENT_DIMENSION);
        push_orientation_row(&mut first_values, first);
        push_orientation_row(&mut second_values, second);
        checked_jacobians(1, first_values, second_values, "spatial axis angle")
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct SpatialAxisAlignmentResidual {
    pub(crate) first_local: Frame3,
    pub(crate) second_local: Frame3,
    pub(crate) parity_multiplier: f64,
}

impl ResidualEvaluator for SpatialAxisAlignmentResidual {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        validate_parity(self.parity_multiplier)?;
        let frames = frame_pair(
            variables,
            self.first_local,
            self.second_local,
            "spatial axis alignment",
        )?;
        let second_axis = frames.second_world.z_axis() * self.parity_multiplier;
        checked_residual(
            vec![
                frames.first_world.x_axis().dot(&second_axis),
                frames.first_world.y_axis().dot(&second_axis),
            ],
            "spatial axis alignment",
        )
    }

    fn jacobian(&self, variables: &[VariableValue]) -> Result<Vec<LocalJacobian>, EvaluationError> {
        validate_parity(self.parity_multiplier)?;
        let frames = frame_pair(
            variables,
            self.first_local,
            self.second_local,
            "spatial axis alignment",
        )?;
        let second_axis = frames.second_local.z_axis() * self.parity_multiplier;
        let mut first_values = Vec::with_capacity(2 * TANGENT_DIMENSION);
        let mut second_values = Vec::with_capacity(2 * TANGENT_DIMENSION);
        for (first_axis, context) in [
            (frames.first_local.x_axis(), "spatial axis alignment x-z"),
            (frames.first_local.y_axis(), "spatial axis alignment y-z"),
        ] {
            let (first, second) = orientation_derivatives(
                frames.first_pose,
                frames.second_pose,
                first_axis,
                second_axis,
                context,
            )?;
            push_orientation_row(&mut first_values, first);
            push_orientation_row(&mut second_values, second);
        }
        checked_jacobians(2, first_values, second_values, "spatial axis alignment")
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct SpatialHingePositionResidual {
    pub(crate) first_local: Frame3,
    pub(crate) second_local: Frame3,
    pub(crate) parity_multiplier: f64,
    pub(crate) target_principal_phase: f64,
}

impl ResidualEvaluator for SpatialHingePositionResidual {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        validate_parity(self.parity_multiplier)?;
        validate_canonical_phase(self.target_principal_phase)?;
        let frames = frame_pair(
            variables,
            self.first_local,
            self.second_local,
            "spatial hinge position driver",
        )?;
        let (target_sine, target_cosine) = self.target_principal_phase.sin_cos();
        let target_normal =
            frames.first_world.y_axis() * target_cosine - frames.first_world.x_axis() * target_sine;
        checked_residual(
            vec![target_normal.dot(&frames.second_world.x_axis())],
            "spatial hinge position driver",
        )
    }

    fn jacobian(&self, variables: &[VariableValue]) -> Result<Vec<LocalJacobian>, EvaluationError> {
        validate_parity(self.parity_multiplier)?;
        validate_canonical_phase(self.target_principal_phase)?;
        let frames = frame_pair(
            variables,
            self.first_local,
            self.second_local,
            "spatial hinge position driver",
        )?;
        let (target_sine, target_cosine) = self.target_principal_phase.sin_cos();
        let first_target_normal =
            frames.first_local.y_axis() * target_cosine - frames.first_local.x_axis() * target_sine;
        let (first, second) = orientation_derivatives(
            frames.first_pose,
            frames.second_pose,
            first_target_normal,
            frames.second_local.x_axis(),
            "spatial hinge position driver",
        )?;
        let mut first_values = Vec::with_capacity(TANGENT_DIMENSION);
        let mut second_values = Vec::with_capacity(TANGENT_DIMENSION);
        push_orientation_row(&mut first_values, first);
        push_orientation_row(&mut second_values, second);
        checked_jacobians(
            1,
            first_values,
            second_values,
            "spatial hinge position driver",
        )
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct SpatialTranslationPositionResidual {
    pub(crate) first_local: Frame3,
    pub(crate) second_local: Frame3,
    pub(crate) first_local_axis: Vector3<f64>,
    pub(crate) parity_multiplier: f64,
    pub(crate) target: f64,
}

impl ResidualEvaluator for SpatialTranslationPositionResidual {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        validate_parity(self.parity_multiplier)?;
        validate_finite_translation_target(self.target)?;
        let frames = frame_pair(
            variables,
            self.first_local,
            self.second_local,
            "spatial translation position driver",
        )?;
        let displacement = frames.second_world.origin() - frames.first_world.origin();
        let first_axis = frames
            .first_pose
            .try_transform_vector(self.first_local_axis)
            .map_err(|error| geometry_error("spatial translation position driver axis", error))?;
        checked_residual(
            vec![first_axis.dot(&displacement) - self.target],
            "spatial translation position driver",
        )
    }

    fn jacobian(&self, variables: &[VariableValue]) -> Result<Vec<LocalJacobian>, EvaluationError> {
        validate_parity(self.parity_multiplier)?;
        validate_finite_translation_target(self.target)?;
        let frames = frame_pair(
            variables,
            self.first_local,
            self.second_local,
            "spatial translation position driver",
        )?;
        let (first, second) = moving_axis_displacement_derivatives(
            &frames,
            self.first_local_axis,
            "spatial translation position driver",
        )?;
        checked_jacobians(
            1,
            first.to_vec(),
            second.to_vec(),
            "spatial translation position driver",
        )
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct SpatialFixedFrameResidual {
    pub(crate) first_local: Frame3,
    pub(crate) second_local: Frame3,
}

impl ResidualEvaluator for SpatialFixedFrameResidual {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        let frames = frame_pair(
            variables,
            self.first_local,
            self.second_local,
            "spatial fixed frame",
        )?;
        let displacement = frames.second_world.origin() - frames.first_world.origin();
        checked_residual(
            vec![
                displacement.x,
                displacement.y,
                displacement.z,
                frames
                    .first_world
                    .y_axis()
                    .dot(&frames.second_world.x_axis()),
                frames
                    .first_world
                    .z_axis()
                    .dot(&frames.second_world.x_axis()),
                frames
                    .first_world
                    .z_axis()
                    .dot(&frames.second_world.y_axis()),
            ],
            "spatial fixed frame",
        )
    }

    fn jacobian(&self, variables: &[VariableValue]) -> Result<Vec<LocalJacobian>, EvaluationError> {
        let frames = frame_pair(
            variables,
            self.first_local,
            self.second_local,
            "spatial fixed frame",
        )?;
        let first_point = point_derivative(
            frames.first_pose,
            frames.first_local.origin(),
            "spatial fixed frame first origin",
        )?;
        let second_point = point_derivative(
            frames.second_pose,
            frames.second_local.origin(),
            "spatial fixed frame second origin",
        )?;
        let orientation_rows = [
            orientation_derivatives(
                frames.first_pose,
                frames.second_pose,
                frames.first_local.y_axis(),
                frames.second_local.x_axis(),
                "spatial fixed frame y-x orientation",
            )?,
            orientation_derivatives(
                frames.first_pose,
                frames.second_pose,
                frames.first_local.z_axis(),
                frames.second_local.x_axis(),
                "spatial fixed frame z-x orientation",
            )?,
            orientation_derivatives(
                frames.first_pose,
                frames.second_pose,
                frames.first_local.z_axis(),
                frames.second_local.y_axis(),
                "spatial fixed frame z-y orientation",
            )?,
        ];

        let mut first_values = Vec::with_capacity(6 * TANGENT_DIMENSION);
        let mut second_values = Vec::with_capacity(6 * TANGENT_DIMENSION);
        push_point_rows(&mut first_values, &first_point, -1.0);
        push_point_rows(&mut second_values, &second_point, 1.0);
        for (first_angular, second_angular) in orientation_rows {
            push_orientation_row(&mut first_values, first_angular);
            push_orientation_row(&mut second_values, second_angular);
        }
        checked_jacobians(6, first_values, second_values, "spatial fixed frame")
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct SpatialRevoluteResidual {
    pub(crate) first_local: Frame3,
    pub(crate) second_local: Frame3,
    pub(crate) parity_multiplier: f64,
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum SpatialRelationKind {
    Prismatic,
    Cylindrical,
    Planar,
    Universal,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct SpatialRelationResidual {
    pub(crate) first_local: Frame3,
    pub(crate) second_local: Frame3,
    pub(crate) parity_multiplier: f64,
    pub(crate) kind: SpatialRelationKind,
}

impl ResidualEvaluator for SpatialRelationResidual {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        validate_parity(self.parity_multiplier)?;
        let context = self.context();
        let frames = frame_pair(variables, self.first_local, self.second_local, context)?;
        let displacement = frames.second_world.origin() - frames.first_world.origin();
        let second_axis = frames.second_world.z_axis() * self.parity_multiplier;
        let values = match self.kind {
            SpatialRelationKind::Prismatic => vec![
                frames.first_world.x_axis().dot(&displacement),
                frames.first_world.y_axis().dot(&displacement),
                frames.first_world.x_axis().dot(&second_axis),
                frames.first_world.y_axis().dot(&second_axis),
                frames
                    .first_world
                    .y_axis()
                    .dot(&frames.second_world.x_axis()),
            ],
            SpatialRelationKind::Cylindrical => vec![
                frames.first_world.x_axis().dot(&displacement),
                frames.first_world.y_axis().dot(&displacement),
                frames.first_world.x_axis().dot(&second_axis),
                frames.first_world.y_axis().dot(&second_axis),
            ],
            SpatialRelationKind::Planar => vec![
                frames.first_world.z_axis().dot(&displacement),
                frames.first_world.x_axis().dot(&second_axis),
                frames.first_world.y_axis().dot(&second_axis),
            ],
            SpatialRelationKind::Universal => vec![
                displacement.x,
                displacement.y,
                displacement.z,
                frames
                    .first_world
                    .z_axis()
                    .dot(&frames.second_world.z_axis()),
            ],
        };
        checked_residual(values, context)
    }

    #[allow(clippy::too_many_lines)]
    fn jacobian(&self, variables: &[VariableValue]) -> Result<Vec<LocalJacobian>, EvaluationError> {
        validate_parity(self.parity_multiplier)?;
        let context = self.context();
        let frames = frame_pair(variables, self.first_local, self.second_local, context)?;
        let rows = self.rows();
        let mut first_values = Vec::with_capacity(rows * TANGENT_DIMENSION);
        let mut second_values = Vec::with_capacity(rows * TANGENT_DIMENSION);

        match self.kind {
            SpatialRelationKind::Prismatic => {
                for (axis, name) in [
                    (frames.first_local.x_axis(), "x-displacement"),
                    (frames.first_local.y_axis(), "y-displacement"),
                ] {
                    let (first, second) = moving_axis_displacement_derivatives(
                        &frames,
                        axis,
                        &format!("{context} {name}"),
                    )?;
                    push_scalar_row(&mut first_values, first);
                    push_scalar_row(&mut second_values, second);
                }
                for (first_axis, second_axis, name) in [
                    (
                        frames.first_local.x_axis(),
                        frames.second_local.z_axis() * self.parity_multiplier,
                        "x-axis alignment",
                    ),
                    (
                        frames.first_local.y_axis(),
                        frames.second_local.z_axis() * self.parity_multiplier,
                        "y-axis alignment",
                    ),
                    (
                        frames.first_local.y_axis(),
                        frames.second_local.x_axis(),
                        "clock alignment",
                    ),
                ] {
                    let (first, second) = orientation_derivatives(
                        frames.first_pose,
                        frames.second_pose,
                        first_axis,
                        second_axis,
                        &format!("{context} {name}"),
                    )?;
                    push_orientation_row(&mut first_values, first);
                    push_orientation_row(&mut second_values, second);
                }
            }
            SpatialRelationKind::Cylindrical => {
                for (axis, name) in [
                    (frames.first_local.x_axis(), "x-displacement"),
                    (frames.first_local.y_axis(), "y-displacement"),
                ] {
                    let (first, second) = moving_axis_displacement_derivatives(
                        &frames,
                        axis,
                        &format!("{context} {name}"),
                    )?;
                    push_scalar_row(&mut first_values, first);
                    push_scalar_row(&mut second_values, second);
                }
                for (first_axis, name) in [
                    (frames.first_local.x_axis(), "x-axis alignment"),
                    (frames.first_local.y_axis(), "y-axis alignment"),
                ] {
                    let (first, second) = orientation_derivatives(
                        frames.first_pose,
                        frames.second_pose,
                        first_axis,
                        frames.second_local.z_axis() * self.parity_multiplier,
                        &format!("{context} {name}"),
                    )?;
                    push_orientation_row(&mut first_values, first);
                    push_orientation_row(&mut second_values, second);
                }
            }
            SpatialRelationKind::Planar => {
                let (first, second) = moving_axis_displacement_derivatives(
                    &frames,
                    frames.first_local.z_axis(),
                    &format!("{context} normal displacement"),
                )?;
                push_scalar_row(&mut first_values, first);
                push_scalar_row(&mut second_values, second);
                for (first_axis, name) in [
                    (frames.first_local.x_axis(), "x-normal alignment"),
                    (frames.first_local.y_axis(), "y-normal alignment"),
                ] {
                    let (first, second) = orientation_derivatives(
                        frames.first_pose,
                        frames.second_pose,
                        first_axis,
                        frames.second_local.z_axis() * self.parity_multiplier,
                        &format!("{context} {name}"),
                    )?;
                    push_orientation_row(&mut first_values, first);
                    push_orientation_row(&mut second_values, second);
                }
            }
            SpatialRelationKind::Universal => {
                let first_point = point_derivative(
                    frames.first_pose,
                    frames.first_local.origin(),
                    &format!("{context} first origin"),
                )?;
                let second_point = point_derivative(
                    frames.second_pose,
                    frames.second_local.origin(),
                    &format!("{context} second origin"),
                )?;
                push_point_rows(&mut first_values, &first_point, -1.0);
                push_point_rows(&mut second_values, &second_point, 1.0);
                let (first, second) = orientation_derivatives(
                    frames.first_pose,
                    frames.second_pose,
                    frames.first_local.z_axis(),
                    frames.second_local.z_axis(),
                    &format!("{context} axis orthogonality"),
                )?;
                push_orientation_row(&mut first_values, first);
                push_orientation_row(&mut second_values, second);
            }
        }

        checked_jacobians(rows, first_values, second_values, context)
    }
}

impl SpatialRelationResidual {
    const fn context(self) -> &'static str {
        match self.kind {
            SpatialRelationKind::Prismatic => "spatial prismatic",
            SpatialRelationKind::Cylindrical => "spatial cylindrical",
            SpatialRelationKind::Planar => "spatial planar",
            SpatialRelationKind::Universal => "spatial universal",
        }
    }

    const fn rows(self) -> usize {
        match self.kind {
            SpatialRelationKind::Prismatic => 5,
            SpatialRelationKind::Cylindrical | SpatialRelationKind::Universal => 4,
            SpatialRelationKind::Planar => 3,
        }
    }
}

impl ResidualEvaluator for SpatialRevoluteResidual {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        validate_parity(self.parity_multiplier)?;
        let frames = frame_pair(
            variables,
            self.first_local,
            self.second_local,
            "spatial revolute",
        )?;
        let displacement = frames.second_world.origin() - frames.first_world.origin();
        let second_axis = frames.second_world.z_axis() * self.parity_multiplier;
        checked_residual(
            vec![
                displacement.x,
                displacement.y,
                displacement.z,
                frames.first_world.x_axis().dot(&second_axis),
                frames.first_world.y_axis().dot(&second_axis),
            ],
            "spatial revolute",
        )
    }

    fn jacobian(&self, variables: &[VariableValue]) -> Result<Vec<LocalJacobian>, EvaluationError> {
        validate_parity(self.parity_multiplier)?;
        let frames = frame_pair(
            variables,
            self.first_local,
            self.second_local,
            "spatial revolute",
        )?;
        let first_point = point_derivative(
            frames.first_pose,
            frames.first_local.origin(),
            "spatial revolute first origin",
        )?;
        let second_point = point_derivative(
            frames.second_pose,
            frames.second_local.origin(),
            "spatial revolute second origin",
        )?;
        let second_axis = frames.second_local.z_axis() * self.parity_multiplier;
        let orientation_rows = [
            orientation_derivatives(
                frames.first_pose,
                frames.second_pose,
                frames.first_local.x_axis(),
                second_axis,
                "spatial revolute x-z orientation",
            )?,
            orientation_derivatives(
                frames.first_pose,
                frames.second_pose,
                frames.first_local.y_axis(),
                second_axis,
                "spatial revolute y-z orientation",
            )?,
        ];

        let mut first_values = Vec::with_capacity(5 * TANGENT_DIMENSION);
        let mut second_values = Vec::with_capacity(5 * TANGENT_DIMENSION);
        push_point_rows(&mut first_values, &first_point, -1.0);
        push_point_rows(&mut second_values, &second_point, 1.0);
        for (first_angular, second_angular) in orientation_rows {
            push_orientation_row(&mut first_values, first_angular);
            push_orientation_row(&mut second_values, second_angular);
        }
        checked_jacobians(5, first_values, second_values, "spatial revolute")
    }
}

#[derive(Clone, Copy, Debug)]
struct FramePair {
    first_pose: Pose3,
    second_pose: Pose3,
    first_local: Frame3,
    second_local: Frame3,
    first_world: Frame3,
    second_world: Frame3,
}

fn two_poses(
    variables: &[VariableValue],
    context: &str,
) -> Result<(Pose3, Pose3), EvaluationError> {
    let [VariableValue::Pose3(first), VariableValue::Pose3(second)] = variables else {
        return Err(EvaluationError::invalid_geometry(format!(
            "{context} residual expected exactly two Pose3 variables"
        )));
    };
    Ok((
        Pose3::from_ambient(*first)
            .map_err(|error| geometry_error(&format!("{context} first pose"), error))?,
        Pose3::from_ambient(*second)
            .map_err(|error| geometry_error(&format!("{context} second pose"), error))?,
    ))
}

fn frame_pair(
    variables: &[VariableValue],
    first_local: Frame3,
    second_local: Frame3,
    context: &str,
) -> Result<FramePair, EvaluationError> {
    let (first_pose, second_pose) = two_poses(variables, context)?;
    let first_local = checked_frame(first_local, &format!("{context} first local frame"))?;
    let second_local = checked_frame(second_local, &format!("{context} second local frame"))?;
    let first_world = world_frame(
        first_pose,
        first_local,
        &format!("{context} first world frame"),
    )?;
    let second_world = world_frame(
        second_pose,
        second_local,
        &format!("{context} second world frame"),
    )?;
    Ok(FramePair {
        first_pose,
        second_pose,
        first_local,
        second_local,
        first_world,
        second_world,
    })
}

fn checked_frame(frame: Frame3, context: &str) -> Result<Frame3, EvaluationError> {
    Frame3::try_new(
        frame.origin(),
        frame.x_axis(),
        frame.y_axis(),
        frame.z_axis(),
    )
    .map_err(|error| geometry_error(context, error))
}

fn world_frame(pose: Pose3, frame: Frame3, context: &str) -> Result<Frame3, EvaluationError> {
    let origin = pose
        .try_transform_point(frame.origin())
        .map_err(|error| geometry_error(context, error))?;
    let x_axis = pose
        .try_transform_vector(frame.x_axis())
        .map_err(|error| geometry_error(context, error))?;
    let y_axis = pose
        .try_transform_vector(frame.y_axis())
        .map_err(|error| geometry_error(context, error))?;
    let z_axis = pose
        .try_transform_vector(frame.z_axis())
        .map_err(|error| geometry_error(context, error))?;
    Frame3::try_new(origin, x_axis, y_axis, z_axis).map_err(|error| geometry_error(context, error))
}

fn transform_point(
    pose: Pose3,
    local: [f64; 3],
    context: &str,
) -> Result<Point3<f64>, EvaluationError> {
    pose.try_transform_point(point(local))
        .map_err(|error| geometry_error(context, error))
}

fn regular_point_separation(
    first: Point3<f64>,
    second: Point3<f64>,
    context: &str,
) -> Result<(Vector3<f64>, f64), EvaluationError> {
    let displacement = second - first;
    if !displacement.iter().all(|value| value.is_finite()) {
        return Err(EvaluationError::invalid_geometry(format!(
            "{context} displacement is non-finite"
        )));
    }
    let distance = robust_norm(displacement);
    if !distance.is_finite() {
        return Err(EvaluationError::invalid_geometry(format!(
            "{context} distance is non-finite"
        )));
    }
    if distance == 0.0 {
        return Err(EvaluationError::nondifferentiable(format!(
            "{context} derivative is undefined for coincident points"
        )));
    }
    let direction = displacement / distance;
    if !direction.iter().all(|value| value.is_finite()) {
        return Err(EvaluationError::invalid_geometry(format!(
            "{context} direction is non-finite"
        )));
    }
    Ok((direction, distance))
}

fn require_regular_axis_angle(
    first: Vector3<f64>,
    second: Vector3<f64>,
    context: &str,
) -> Result<(), EvaluationError> {
    let cross = first.cross(&second);
    if !cross.iter().all(|value| value.is_finite()) {
        return Err(EvaluationError::invalid_geometry(format!(
            "{context} cross product is non-finite"
        )));
    }
    let sine = robust_norm(cross);
    if !sine.is_finite() {
        return Err(EvaluationError::invalid_geometry(format!(
            "{context} principal-angle sine is non-finite"
        )));
    }
    if sine == 0.0 {
        return Err(EvaluationError::nondifferentiable(format!(
            "{context} derivative is singular at principal-angle endpoints"
        )));
    }
    Ok(())
}

const fn point(value: [f64; 3]) -> Point3<f64> {
    Point3::new(value[0], value[1], value[2])
}

fn point_derivative(
    pose: Pose3,
    local: Point3<f64>,
    context: &str,
) -> Result<SMatrix<f64, 3, 6>, EvaluationError> {
    pose.try_transform_point(local)
        .map_err(|error| geometry_error(context, error))?;
    let rotation = pose.rotation().to_rotation_matrix();
    let point_cross = skew(local.coords);
    let angular = -(rotation.matrix() * point_cross);
    let mut derivative = SMatrix::<f64, 3, 6>::zeros();
    derivative
        .fixed_view_mut::<3, 3>(0, 0)
        .copy_from(rotation.matrix());
    derivative.fixed_view_mut::<3, 3>(0, 3).copy_from(&angular);
    if derivative.iter().all(|value| value.is_finite()) {
        Ok(derivative)
    } else {
        Err(EvaluationError::invalid_geometry(format!(
            "{context} derivative is non-finite"
        )))
    }
}

fn orientation_derivatives(
    first_pose: Pose3,
    second_pose: Pose3,
    first_axis: Vector3<f64>,
    second_axis: Vector3<f64>,
    context: &str,
) -> Result<(Vector3<f64>, Vector3<f64>), EvaluationError> {
    let second_world = second_pose
        .try_transform_vector(second_axis)
        .map_err(|error| geometry_error(context, error))?;
    let second_in_first = first_pose
        .try_inverse_transform_vector(second_world)
        .map_err(|error| geometry_error(context, error))?;
    let first_world = first_pose
        .try_transform_vector(first_axis)
        .map_err(|error| geometry_error(context, error))?;
    let first_in_second = second_pose
        .try_inverse_transform_vector(first_world)
        .map_err(|error| geometry_error(context, error))?;
    let first_angular = first_axis.cross(&second_in_first);
    let second_angular = second_axis.cross(&first_in_second);
    if first_angular
        .iter()
        .chain(second_angular.iter())
        .all(|value| value.is_finite())
    {
        Ok((first_angular, second_angular))
    } else {
        Err(EvaluationError::invalid_geometry(format!(
            "{context} derivative is non-finite"
        )))
    }
}

fn moving_axis_displacement_derivatives(
    frames: &FramePair,
    first_local_axis: Vector3<f64>,
    context: &str,
) -> Result<([f64; TANGENT_DIMENSION], [f64; TANGENT_DIMENSION]), EvaluationError> {
    let first_axis = frames
        .first_pose
        .try_transform_vector(first_local_axis)
        .map_err(|error| geometry_error(context, error))?;
    let displacement = frames.second_world.origin() - frames.first_world.origin();
    let first_axis_derivative = direction_derivative(
        frames.first_pose,
        first_local_axis,
        &format!("{context} first axis"),
    )?;
    let first_point_derivative = point_derivative(
        frames.first_pose,
        frames.first_local.origin(),
        &format!("{context} first origin"),
    )?;
    let second_point_derivative = point_derivative(
        frames.second_pose,
        frames.second_local.origin(),
        &format!("{context} second origin"),
    )?;
    let mut first = [0.0; TANGENT_DIMENSION];
    let mut second = [0.0; TANGENT_DIMENSION];
    for column in 0..TANGENT_DIMENSION {
        first[column] = first_axis_derivative.column(column).dot(&displacement)
            - first_axis.dot(&first_point_derivative.column(column));
        second[column] = first_axis.dot(&second_point_derivative.column(column));
    }
    if first.iter().chain(&second).all(|value| value.is_finite()) {
        Ok((first, second))
    } else {
        Err(EvaluationError::invalid_geometry(format!(
            "{context} derivative is non-finite"
        )))
    }
}

fn direction_derivative(
    pose: Pose3,
    local: Vector3<f64>,
    context: &str,
) -> Result<SMatrix<f64, 3, 6>, EvaluationError> {
    pose.try_transform_vector(local)
        .map_err(|error| geometry_error(context, error))?;
    let rotation = pose.rotation().to_rotation_matrix();
    let angular = -(rotation.matrix() * skew(local));
    let mut derivative = SMatrix::<f64, 3, 6>::zeros();
    derivative.fixed_view_mut::<3, 3>(0, 3).copy_from(&angular);
    if derivative.iter().all(|value| value.is_finite()) {
        Ok(derivative)
    } else {
        Err(EvaluationError::invalid_geometry(format!(
            "{context} derivative is non-finite"
        )))
    }
}

fn push_point_rows(values: &mut Vec<f64>, derivative: &SMatrix<f64, 3, 6>, sign: f64) {
    for row in 0..3 {
        for column in 0..TANGENT_DIMENSION {
            values.push(sign * derivative[(row, column)]);
        }
    }
}

fn push_orientation_row(values: &mut Vec<f64>, angular: Vector3<f64>) {
    values.extend([0.0, 0.0, 0.0, angular.x, angular.y, angular.z]);
}

fn push_scalar_row(values: &mut Vec<f64>, row: [f64; TANGENT_DIMENSION]) {
    values.extend(row);
}

fn checked_residual(values: Vec<f64>, context: &str) -> Result<Vec<f64>, EvaluationError> {
    if values.iter().all(|value| value.is_finite()) {
        Ok(values)
    } else {
        Err(EvaluationError::invalid_geometry(format!(
            "{context} residual is non-finite"
        )))
    }
}

fn checked_jacobians(
    rows: usize,
    first_values: Vec<f64>,
    second_values: Vec<f64>,
    context: &str,
) -> Result<Vec<LocalJacobian>, EvaluationError> {
    let expected = rows * TANGENT_DIMENSION;
    if first_values.len() != expected || second_values.len() != expected {
        return Err(EvaluationError::invalid_geometry(format!(
            "{context} derivative has malformed dimensions"
        )));
    }
    if !first_values
        .iter()
        .chain(&second_values)
        .all(|value| value.is_finite())
    {
        return Err(EvaluationError::invalid_geometry(format!(
            "{context} derivative is non-finite"
        )));
    }
    Ok(vec![
        LocalJacobian::new(rows, TANGENT_DIMENSION, first_values),
        LocalJacobian::new(rows, TANGENT_DIMENSION, second_values),
    ])
}

fn validate_parity(parity_multiplier: f64) -> Result<(), EvaluationError> {
    let valid_bits = [1.0_f64.to_bits(), (-1.0_f64).to_bits()];
    if valid_bits.contains(&parity_multiplier.to_bits()) {
        Ok(())
    } else {
        Err(EvaluationError::invalid_geometry(
            "spatial axis parity multiplier must be exactly +1 or -1",
        ))
    }
}

fn validate_positive_distance(distance: f64) -> Result<(), EvaluationError> {
    if distance.is_finite() && distance > 0.0 {
        Ok(())
    } else {
        Err(EvaluationError::out_of_domain(
            "spatial point-distance target must be strictly positive and finite",
        ))
    }
}

fn validate_interior_angle(angle: f64) -> Result<(), EvaluationError> {
    if angle.is_finite() && angle > 0.0 && angle < std::f64::consts::PI {
        Ok(())
    } else {
        Err(EvaluationError::out_of_domain(
            "spatial axis-angle target must be finite and strictly inside (0, PI)",
        ))
    }
}

fn validate_canonical_phase(phase: f64) -> Result<(), EvaluationError> {
    if phase.is_finite() && (-std::f64::consts::PI..std::f64::consts::PI).contains(&phase) {
        Ok(())
    } else {
        Err(EvaluationError::out_of_domain(
            "spatial hinge target phase must be finite and canonical in [-PI, PI)",
        ))
    }
}

fn validate_finite_translation_target(target: f64) -> Result<(), EvaluationError> {
    if target.is_finite() {
        Ok(())
    } else {
        Err(EvaluationError::out_of_domain(
            "spatial translation target must be finite",
        ))
    }
}

fn geometry_error(context: &str, error: geosolve_geometry::GeometryError) -> EvaluationError {
    EvaluationError::invalid_geometry(format!("{context}: {error}"))
}

fn skew(vector: Vector3<f64>) -> Matrix3<f64> {
    Matrix3::new(
        0.0, -vector.z, vector.y, vector.z, 0.0, -vector.x, -vector.y, vector.x, 0.0,
    )
}

fn robust_norm(vector: Vector3<f64>) -> f64 {
    vector.x.hypot(vector.y).hypot(vector.z)
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIFFERENCE_STEP: f64 = 1.0e-7;

    #[test]
    fn spatial_residual_jacobians_match_right_tangent_finite_differences() {
        let first_pose = Pose3::exp([0.7, -0.4, 0.2, 0.3, -0.2, 0.5]).unwrap();
        let second_pose = Pose3::exp([-0.3, 0.8, 0.6, -0.4, 0.1, 0.2]).unwrap();
        let variables = [
            VariableValue::Pose3(first_pose.ambient()),
            VariableValue::Pose3(second_pose.ambient()),
        ];
        let first_frame = Frame3::try_new(
            Point3::new(0.2, -0.5, 0.7),
            Vector3::x(),
            Vector3::y(),
            Vector3::z(),
        )
        .unwrap();
        let second_frame = Frame3::try_new(
            Point3::new(-0.6, 0.1, 0.4),
            Vector3::y(),
            Vector3::z(),
            Vector3::x(),
        )
        .unwrap();
        let evaluators: [Box<dyn ResidualEvaluator>; 12] = [
            Box::new(SpatialBallResidual {
                first_local: [0.2, -0.5, 0.7],
                second_local: [-0.6, 0.1, 0.4],
            }),
            Box::new(SpatialPointDistanceResidual {
                first_local: [0.2, -0.5, 0.7],
                second_local: [-0.6, 0.1, 0.4],
                distance: 1.7,
            }),
            Box::new(SpatialFixedFrameResidual {
                first_local: first_frame,
                second_local: second_frame,
            }),
            Box::new(SpatialAxisAngleResidual {
                first_local: first_frame,
                second_local: second_frame,
                angle: 0.8,
            }),
            Box::new(SpatialAxisAlignmentResidual {
                first_local: first_frame,
                second_local: second_frame,
                parity_multiplier: -1.0,
            }),
            Box::new(SpatialHingePositionResidual {
                first_local: first_frame,
                second_local: second_frame,
                parity_multiplier: -1.0,
                target_principal_phase: -0.7,
            }),
            Box::new(SpatialTranslationPositionResidual {
                first_local: first_frame,
                second_local: second_frame,
                first_local_axis: first_frame.z_axis(),
                parity_multiplier: -1.0,
                target: -1.3,
            }),
            Box::new(SpatialRevoluteResidual {
                first_local: first_frame,
                second_local: second_frame,
                parity_multiplier: -1.0,
            }),
            Box::new(SpatialRelationResidual {
                first_local: first_frame,
                second_local: second_frame,
                parity_multiplier: -1.0,
                kind: SpatialRelationKind::Prismatic,
            }),
            Box::new(SpatialRelationResidual {
                first_local: first_frame,
                second_local: second_frame,
                parity_multiplier: -1.0,
                kind: SpatialRelationKind::Cylindrical,
            }),
            Box::new(SpatialRelationResidual {
                first_local: first_frame,
                second_local: second_frame,
                parity_multiplier: -1.0,
                kind: SpatialRelationKind::Planar,
            }),
            Box::new(SpatialRelationResidual {
                first_local: first_frame,
                second_local: second_frame,
                parity_multiplier: 1.0,
                kind: SpatialRelationKind::Universal,
            }),
        ];

        for evaluator in evaluators {
            assert_finite_difference(evaluator.as_ref(), &variables);
        }
    }

    #[test]
    fn spatial_residuals_reject_invalid_evaluator_inputs() {
        let valid = VariableValue::Pose3(Pose3::identity().ambient());
        let frame =
            Frame3::try_new(Point3::origin(), Vector3::x(), Vector3::y(), Vector3::z()).unwrap();
        let ball = SpatialBallResidual {
            first_local: [f64::NAN, 0.0, 0.0],
            second_local: [0.0; 3],
        };
        assert!(ball.evaluate(&[valid, valid]).is_err());
        assert!(
            ball.jacobian(&[VariableValue::Vec3([0.0; 3]), valid])
                .is_err()
        );

        let revolute = SpatialRevoluteResidual {
            first_local: frame,
            second_local: frame,
            parity_multiplier: 0.0,
        };
        assert!(revolute.evaluate(&[valid, valid]).is_err());
        assert!(revolute.jacobian(&[valid, valid]).is_err());

        let relation = SpatialRelationResidual {
            first_local: frame,
            second_local: frame,
            parity_multiplier: 0.0,
            kind: SpatialRelationKind::Prismatic,
        };
        assert!(relation.evaluate(&[valid, valid]).is_err());
        assert!(relation.jacobian(&[valid, valid]).is_err());
        let relation = SpatialRelationResidual {
            parity_multiplier: 1.0,
            ..relation
        };
        assert!(relation.evaluate(&[valid]).is_err());
        assert!(
            relation
                .jacobian(&[VariableValue::Vec3([0.0; 3]), valid])
                .is_err()
        );

        let hinge = SpatialHingePositionResidual {
            first_local: frame,
            second_local: frame,
            parity_multiplier: 1.0,
            target_principal_phase: std::f64::consts::PI,
        };
        assert!(hinge.evaluate(&[valid, valid]).is_err());
        assert!(hinge.jacobian(&[valid, valid]).is_err());

        let translation = SpatialTranslationPositionResidual {
            first_local: frame,
            second_local: frame,
            first_local_axis: frame.z_axis(),
            parity_multiplier: 1.0,
            target: f64::NAN,
        };
        assert!(translation.evaluate(&[valid, valid]).is_err());
        assert!(translation.jacobian(&[valid, valid]).is_err());
    }

    fn assert_finite_difference(evaluator: &dyn ResidualEvaluator, variables: &[VariableValue; 2]) {
        let residuals = evaluator.evaluate(variables).unwrap();
        let analytic = evaluator.jacobian(variables).unwrap();
        assert_eq!(analytic.len(), 2);
        for (body, block) in analytic.iter().enumerate() {
            assert_eq!(block.rows(), residuals.len());
            assert_eq!(block.columns(), TANGENT_DIMENSION);
            for column in 0..TANGENT_DIMENSION {
                let positive = perturbed(variables, body, column, DIFFERENCE_STEP);
                let negative = perturbed(variables, body, column, -DIFFERENCE_STEP);
                let positive = evaluator.evaluate(&positive).unwrap();
                let negative = evaluator.evaluate(&negative).unwrap();
                for row in 0..residuals.len() {
                    let numeric = (positive[row] - negative[row]) / (2.0 * DIFFERENCE_STEP);
                    let expected = block.values()[row * TANGENT_DIMENSION + column];
                    assert!(
                        (numeric - expected).abs() <= 1.0e-6 * (1.0 + numeric.abs()),
                        "body {body}, row {row}, column {column}: analytic {expected}, numeric {numeric}"
                    );
                }
            }
        }
    }

    fn perturbed(
        variables: &[VariableValue; 2],
        body: usize,
        column: usize,
        amount: f64,
    ) -> [VariableValue; 2] {
        let mut perturbed = *variables;
        let VariableValue::Pose3(ambient) = perturbed[body] else {
            unreachable!();
        };
        let mut tangent = [0.0; TANGENT_DIMENSION];
        tangent[column] = amount;
        perturbed[body] = VariableValue::Pose3(
            Pose3::from_ambient(ambient)
                .unwrap()
                .retract(tangent)
                .unwrap()
                .ambient(),
        );
        perturbed
    }
}
