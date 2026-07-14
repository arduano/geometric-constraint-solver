use geosolve_core::{ResidualCategory, SolverConfig};
use geosolve_geometry::{Rotation2, Vector2};
use nalgebra::{DMatrix, DVector};

use crate::compiler::{LinkageGeometry, LinkageSource, cross2};
use crate::model::{
    BodyId, DriverId, DriverKind, JointKind, Linkage, LinkageError, validate_finite,
};

/// Physical planar velocity of one body in deterministic body insertion order.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BodyVelocity {
    pub body_id: BodyId,
    pub linear: Vector2<f64>,
    pub angular: f64,
}

/// Validated differentiated hard-equation solution.
#[derive(Clone, Debug, PartialEq)]
pub struct VelocityResult {
    pub driver_id: DriverId,
    pub driver_rate: f64,
    pub body_velocities: Vec<BodyVelocity>,
    pub rank_is_valid: bool,
    pub rank: usize,
    pub local_degrees_of_freedom: usize,
    pub is_singular: bool,
    pub rank_relative_tolerance: f64,
    pub rank_threshold: f64,
    pub singular_values: Vec<f64>,
    pub differentiated_residual_max: f64,
}

#[derive(Clone, Copy, Debug)]
struct ReducedBodyLayout {
    body_id: BodyId,
    reduced_start: usize,
    step_scales: [f64; 3],
}

impl VelocityResult {
    #[must_use]
    pub fn body(&self, body: BodyId) -> Option<BodyVelocity> {
        self.body_velocities
            .iter()
            .find(|velocity| velocity.body_id == body)
            .copied()
    }
}

impl Linkage {
    /// Solves `J_q q_dot + J_s s_dot = 0` at the current accepted position.
    ///
    /// Dense columns are normalized by core variable step scales; the returned
    /// body rates are converted back to physical `[vx, vy, omega]` units.
    ///
    /// # Errors
    ///
    /// Returns an error for a stale driver, non-finite rate, a position that
    /// fails fresh hard/branch validation, invalid rank data, or a differentiated
    /// system whose independently evaluated residual exceeds tolerance.
    #[allow(clippy::too_many_lines)]
    pub fn velocity(
        &self,
        driver: DriverId,
        driver_rate: f64,
    ) -> Result<VelocityResult, LinkageError> {
        validate_finite(driver_rate, "driver rate")?;
        self.drivers
            .get(driver)
            .ok_or(LinkageError::UnknownDriver(driver))?;
        let compiled = self.compile()?;
        let geometry = compiled.solved_geometry()?;
        let tolerance = SolverConfig::default().normalized_residual_tolerance;
        let domain_max = self.domain_hard_residual_max(&geometry, None)?;
        if domain_max > tolerance {
            return Err(LinkageError::PositionNotAccepted(format!(
                "domain hard residual {domain_max:e} exceeds {tolerance:e}"
            )));
        }
        if let Some(violation) = self.first_branch_violation(&geometry) {
            return Err(LinkageError::PositionNotAccepted(format!(
                "explicit branch check failed: {violation:?}"
            )));
        }
        let audit = compiled.problem.audit_snapshot()?;
        let audit_max = audit
            .sources
            .iter()
            .flat_map(|source| &source.rows)
            .filter(|row| row.category == ResidualCategory::Hard)
            .try_fold(0.0_f64, |maximum, row| {
                row.normalized_residual
                    .is_finite()
                    .then_some(maximum.max(row.normalized_residual.abs()))
            })
            .ok_or(LinkageError::VelocityFailure(
                "hard audit contains a non-finite row",
            ))?;
        if audit_max > tolerance {
            return Err(LinkageError::PositionNotAccepted(format!(
                "hard audit residual {audit_max:e} exceeds {tolerance:e}"
            )));
        }

        let assembly = compiled.problem.assemble_dense()?;
        let grounded_residuals: Vec<_> = compiled
            .source_mappings()
            .iter()
            .filter(|mapping| matches!(mapping.source, LinkageSource::Ground(_)))
            .flat_map(|mapping| mapping.residual_ids.iter().copied())
            .collect();
        let hard_rows: Vec<_> = assembly
            .residual_layout()
            .iter()
            .filter(|layout| {
                compiled
                    .problem
                    .residual(layout.residual_id)
                    .is_some_and(|residual| residual.category() == ResidualCategory::Hard)
                    && !grounded_residuals.contains(&layout.residual_id)
            })
            .flat_map(|layout| layout.row_range.clone())
            .collect();
        let mut active_columns = Vec::new();
        let mut reduced_body_layouts = Vec::new();
        for mapping in compiled.body_variables() {
            if self.require_body(mapping.body_id)?.grounded() {
                continue;
            }
            let block = assembly
                .variable_layout()
                .block(mapping.variable_id)
                .ok_or(geosolve_core::CoreError::UnknownVariable(
                    mapping.variable_id,
                ))?;
            if block.tangent_range.len() != 3 || block.step_scales.len() != 3 {
                return Err(LinkageError::VelocityFailure(
                    "body variable does not have a Pose2 tangent layout",
                ));
            }
            let reduced_start = active_columns.len();
            active_columns.extend(block.tangent_range.clone());
            reduced_body_layouts.push(ReducedBodyLayout {
                body_id: mapping.body_id,
                reduced_start,
                step_scales: [
                    block.step_scales[0],
                    block.step_scales[1],
                    block.step_scales[2],
                ],
            });
        }
        let mut matrix = DMatrix::zeros(hard_rows.len(), active_columns.len());
        for (target_row, &source_row) in hard_rows.iter().enumerate() {
            for (target_column, &source_column) in active_columns.iter().enumerate() {
                matrix[(target_row, target_column)] =
                    assembly.jacobian()[(source_row, source_column)];
            }
        }
        let driver_mapping = compiled
            .source_mapping(LinkageSource::Driver(driver))
            .ok_or(LinkageError::UnknownDriver(driver))?;
        let driver_residual_id =
            *driver_mapping
                .residual_ids
                .first()
                .ok_or(LinkageError::VelocityFailure(
                    "selected driver has no executable row",
                ))?;
        let driver_range =
            assembly
                .residual_range(driver_residual_id)
                .ok_or(LinkageError::VelocityFailure(
                    "selected driver row is absent from dense assembly",
                ))?;
        let source_driver_row = driver_range.start;
        let hard_driver_row = hard_rows
            .iter()
            .position(|&row| row == source_driver_row)
            .ok_or(LinkageError::VelocityFailure(
                "selected driver is not a hard row",
            ))?;
        let driver_residual = compiled.problem.residual(driver_residual_id).ok_or(
            geosolve_core::CoreError::UnknownResidual(driver_residual_id),
        )?;
        let mut right_hand_side = DVector::zeros(hard_rows.len());
        // Both driver residuals contain `-target`; rows are normalized by their
        // own physical residual scale.
        right_hand_side[hard_driver_row] = driver_rate / driver_residual.scales()[0];

        let rank_relative_tolerance = SolverConfig::default().rank_relative_tolerance;
        let singular_values: Vec<_> = matrix
            .clone()
            .svd(false, false)
            .singular_values
            .iter()
            .copied()
            .collect();
        if singular_values.iter().any(|value| !value.is_finite()) {
            return Err(LinkageError::VelocityFailure(
                "singular-value calculation returned non-finite data",
            ));
        }
        let largest = singular_values.iter().copied().fold(0.0_f64, f64::max);
        let dimension = u32::try_from(matrix.nrows().max(matrix.ncols()))
            .map_err(|_| LinkageError::VelocityFailure("velocity system dimension is too large"))?;
        let configured_threshold = largest * rank_relative_tolerance;
        let machine_threshold = f64::EPSILON * f64::from(dimension) * largest;
        let rank_threshold = configured_threshold.max(machine_threshold);
        if !rank_threshold.is_finite() {
            return Err(LinkageError::VelocityFailure("rank threshold is invalid"));
        }
        let rank = singular_values
            .iter()
            .filter(|&&value| value > rank_threshold)
            .count();
        let normalized_rates =
            solve_velocity_system(&matrix, &right_hand_side, rank, rank_threshold)?;

        let mut body_velocities = Vec::with_capacity(compiled.body_variables().len());
        for mapping in compiled.body_variables() {
            let velocity = if self.require_body(mapping.body_id)?.grounded() {
                BodyVelocity {
                    body_id: mapping.body_id,
                    linear: Vector2::zeros(),
                    angular: 0.0,
                }
            } else {
                let layout = reduced_body_layouts
                    .iter()
                    .find(|layout| layout.body_id == mapping.body_id)
                    .ok_or(LinkageError::VelocityFailure(
                        "free body is absent from the reduced velocity layout",
                    ))?;
                BodyVelocity {
                    body_id: mapping.body_id,
                    linear: Vector2::new(
                        normalized_rates[layout.reduced_start] * layout.step_scales[0],
                        normalized_rates[layout.reduced_start + 1] * layout.step_scales[1],
                    ),
                    angular: normalized_rates[layout.reduced_start + 2] * layout.step_scales[2],
                }
            };
            if !velocity.linear.iter().all(|value| value.is_finite())
                || !velocity.angular.is_finite()
            {
                return Err(LinkageError::VelocityFailure(
                    "physical body velocity is non-finite",
                ));
            }
            body_velocities.push(velocity);
        }

        let linearized_residual = &matrix * &normalized_rates - &right_hand_side;
        let matrix_residual_max = linearized_residual
            .iter()
            .map(|value| value.abs())
            .fold(0.0_f64, f64::max);
        let differentiated_residual_max = self.differentiated_hard_residual_max(
            &geometry,
            &body_velocities,
            driver,
            driver_rate,
        )?;
        if !matrix_residual_max.is_finite()
            || !differentiated_residual_max.is_finite()
            || matrix_residual_max > tolerance
            || differentiated_residual_max > tolerance
        {
            return Err(LinkageError::VelocityFailure(
                "differentiated hard equations did not validate",
            ));
        }

        let columns = matrix.ncols();
        Ok(VelocityResult {
            driver_id: driver,
            driver_rate,
            body_velocities,
            rank_is_valid: true,
            rank,
            local_degrees_of_freedom: columns.saturating_sub(rank),
            is_singular: rank < matrix.nrows().min(columns),
            rank_relative_tolerance,
            rank_threshold,
            singular_values,
            differentiated_residual_max,
        })
    }

    #[allow(clippy::too_many_lines)]
    fn differentiated_hard_residual_max(
        &self,
        geometry: &LinkageGeometry,
        velocities: &[BodyVelocity],
        selected_driver: DriverId,
        driver_rate: f64,
    ) -> Result<f64, LinkageError> {
        let mut maximum = 0.0_f64;
        for (body_id, _) in self.bodies.iter().filter(|(_, body)| body.grounded()) {
            let velocity = body_velocity(velocities, body_id)?;
            update_max(&mut maximum, velocity.linear.x / self.model_scale)?;
            update_max(&mut maximum, velocity.linear.y / self.model_scale)?;
            update_max(&mut maximum, velocity.angular)?;
        }
        for (_, joint) in self.joints.iter() {
            match joint.kind() {
                JointKind::Revolute { first, second } => {
                    let relative = point_velocity(self, geometry, velocities, second)?
                        - point_velocity(self, geometry, velocities, first)?;
                    update_max(&mut maximum, relative.x / self.model_scale)?;
                    update_max(&mut maximum, relative.y / self.model_scale)?;
                }
                JointKind::Prismatic {
                    first_anchor,
                    first_axis,
                    second_anchor,
                    second_axis,
                    axis_branch,
                } => {
                    let first_direction = geometry
                        .axis(first_axis)
                        .ok_or(LinkageError::UnknownAxisFeature(first_axis))?;
                    let second_direction = geometry
                        .axis(second_axis)
                        .ok_or(LinkageError::UnknownAxisFeature(second_axis))?
                        * axis_branch.multiplier();
                    let first_body = self.require_axis_feature(first_axis)?.body();
                    let second_body = self.require_axis_feature(second_axis)?.body();
                    let first_angular = body_velocity(velocities, first_body)?.angular;
                    let second_angular = body_velocity(velocities, second_body)?.angular;
                    let first_direction_rate = perpendicular(first_direction) * first_angular;
                    let second_direction_rate = perpendicular(second_direction) * second_angular;
                    let normal = perpendicular(first_direction);
                    let normal_rate = perpendicular(first_direction_rate);
                    let displacement = geometry
                        .point(second_anchor)
                        .ok_or(LinkageError::UnknownPointFeature(second_anchor))?
                        - geometry
                            .point(first_anchor)
                            .ok_or(LinkageError::UnknownPointFeature(first_anchor))?;
                    let displacement_rate =
                        point_velocity(self, geometry, velocities, second_anchor)?
                            - point_velocity(self, geometry, velocities, first_anchor)?;
                    update_max(
                        &mut maximum,
                        (normal_rate.dot(&displacement) + normal.dot(&displacement_rate))
                            / self.model_scale,
                    )?;
                    update_max(
                        &mut maximum,
                        cross2(first_direction_rate, second_direction)
                            + cross2(first_direction, second_direction_rate),
                    )?;
                }
                JointKind::Weld { first, second, .. } => {
                    let relative = point_velocity(self, geometry, velocities, second)?
                        - point_velocity(self, geometry, velocities, first)?;
                    update_max(&mut maximum, relative.x / self.model_scale)?;
                    update_max(&mut maximum, relative.y / self.model_scale)?;
                    let first_body = self.require_point_feature(first)?.body();
                    let second_body = self.require_point_feature(second)?.body();
                    update_max(
                        &mut maximum,
                        body_velocity(velocities, second_body)?.angular
                            - body_velocity(velocities, first_body)?.angular,
                    )?;
                }
            }
        }
        for (driver_id, driver) in self.drivers.iter() {
            let target_rate = if driver_id == selected_driver {
                driver_rate
            } else {
                0.0
            };
            match driver.kind() {
                DriverKind::Angular { reference, driven } => update_max(
                    &mut maximum,
                    body_velocity(velocities, driven)?.angular
                        - body_velocity(velocities, reference)?.angular
                        - target_rate,
                )?,
                DriverKind::Linear {
                    origin,
                    measured,
                    guide_axis,
                } => {
                    let guide = geometry
                        .axis(guide_axis)
                        .ok_or(LinkageError::UnknownAxisFeature(guide_axis))?;
                    let guide_body = self.require_axis_feature(guide_axis)?.body();
                    let guide_rate =
                        perpendicular(guide) * body_velocity(velocities, guide_body)?.angular;
                    let displacement = geometry
                        .point(measured)
                        .ok_or(LinkageError::UnknownPointFeature(measured))?
                        - geometry
                            .point(origin)
                            .ok_or(LinkageError::UnknownPointFeature(origin))?;
                    let displacement_rate = point_velocity(self, geometry, velocities, measured)?
                        - point_velocity(self, geometry, velocities, origin)?;
                    update_max(
                        &mut maximum,
                        (guide_rate.dot(&displacement) + guide.dot(&displacement_rate)
                            - target_rate)
                            / self.model_scale,
                    )?;
                }
            }
        }
        Ok(maximum)
    }
}

fn solve_velocity_system(
    matrix: &DMatrix<f64>,
    right_hand_side: &DVector<f64>,
    rank: usize,
    rank_threshold: f64,
) -> Result<DVector<f64>, LinkageError> {
    let rows = matrix.nrows();
    let columns = matrix.ncols();
    if right_hand_side.len() != rows {
        return Err(LinkageError::VelocityFailure(
            "velocity right-hand side has the wrong dimension",
        ));
    }
    if columns == 0 {
        return Ok(DVector::zeros(0));
    }
    if rows >= columns && rank == columns {
        let qr = matrix.clone().qr();
        let mut transformed = right_hand_side.clone();
        qr.q_tr_mul(&mut transformed);
        let triangular = qr.r();
        let transformed_top = transformed.rows(0, columns).into_owned();
        if let Some(solution) = triangular.solve_upper_triangular(&transformed_top)
            && solution.iter().all(|value| value.is_finite())
        {
            return Ok(solution);
        }
    }
    let svd = matrix.clone().svd(true, true);
    let solution = svd
        .solve(right_hand_side, rank_threshold)
        .map_err(|_| LinkageError::VelocityFailure("QR and SVD velocity solves failed"))?;
    if solution.iter().all(|value| value.is_finite()) {
        Ok(solution)
    } else {
        Err(LinkageError::VelocityFailure(
            "velocity solution is non-finite",
        ))
    }
}

fn point_velocity(
    linkage: &Linkage,
    geometry: &LinkageGeometry,
    velocities: &[BodyVelocity],
    point: crate::PointFeatureId,
) -> Result<Vector2<f64>, LinkageError> {
    let feature = linkage.require_point_feature(point)?;
    let body_pose = geometry
        .body_pose(feature.body())
        .ok_or(LinkageError::UnknownBody(feature.body()))?;
    let rotated_local = Rotation2::new(body_pose.angle) * feature.local_point().coords;
    let velocity = body_velocity(velocities, feature.body())?;
    Ok(velocity.linear + perpendicular(rotated_local) * velocity.angular)
}

fn body_velocity(velocities: &[BodyVelocity], body: BodyId) -> Result<BodyVelocity, LinkageError> {
    velocities
        .iter()
        .find(|velocity| velocity.body_id == body)
        .copied()
        .ok_or(LinkageError::UnknownBody(body))
}

fn perpendicular(vector: Vector2<f64>) -> Vector2<f64> {
    Vector2::new(-vector.y, vector.x)
}

fn update_max(maximum: &mut f64, value: f64) -> Result<(), LinkageError> {
    if !value.is_finite() {
        return Err(LinkageError::VelocityFailure(
            "independent differentiated residual is non-finite",
        ));
    }
    *maximum = maximum.max(value.abs());
    Ok(())
}
