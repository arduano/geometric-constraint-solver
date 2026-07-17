use geosolve_core::{SensitivityStatus, SolverConfig, VariableId, VariableKind, VariableValue};
use geosolve_geometry::{Rotation2, Vector2};
use nalgebra::DVector;

use crate::compiler::{AcceptedCompiledLinkage, LinkageGeometry, LinkageSource, cross2};
use crate::model::{
    BodyId, DriverId, DriverKind, JointKind, Linkage, LinkageError, validate_finite,
};

/// Physical planar velocity of one body in deterministic body insertion order.
///
/// `linear` is the body origin's velocity in planar world coordinates, not the
/// body-local/right-trivialized translation rate used by the solver. `angular`
/// is the scalar world/body angular rate, which is identical in 2D.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BodyVelocity {
    pub body_id: BodyId,
    /// Body-origin linear velocity in planar world coordinates.
    pub linear: Vector2<f64>,
    /// Angular velocity in radians per unit time.
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

#[derive(Clone, Debug)]
pub(crate) struct VelocityGaugeComponent {
    pub(crate) bodies: Vec<BodyId>,
    pub(crate) reference: BodyId,
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
    /// The solve uses the accepted core component linearization and rank
    /// thresholds. Returned body-local rates are converted to physical units
    /// and rotated before publication so [`BodyVelocity::linear`] remains a
    /// world-frame origin velocity.
    ///
    /// # Errors
    ///
    /// Returns an error for a stale driver, non-finite rate, a position that
    /// fails fresh hard/branch validation, invalid rank data, or a differentiated
    /// system whose independently evaluated residual exceeds tolerance.
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
        let accepted = compiled
            .into_accepted_session(SolverConfig::default())
            .map_err(|error| LinkageError::PositionNotAccepted(error.to_string()))?;
        require_same_body_state(self, &accepted)?;
        velocity_from_accepted_session(self, &accepted, driver, driver_rate, &[])
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

#[allow(clippy::too_many_lines)]
pub(crate) fn velocity_from_accepted_session(
    linkage: &Linkage,
    accepted: &AcceptedCompiledLinkage,
    driver: DriverId,
    driver_rate: f64,
    gauges: &[VelocityGaugeComponent],
) -> Result<VelocityResult, LinkageError> {
    validate_finite(driver_rate, "driver rate")?;
    linkage
        .drivers
        .get(driver)
        .ok_or(LinkageError::UnknownDriver(driver))?;
    let tolerance = accepted.session().config().normalized_residual_tolerance;
    let geometry = accepted.solved_geometry()?;
    let domain_max = linkage.domain_hard_residual_max(&geometry, None)?;
    if domain_max > tolerance {
        return Err(LinkageError::PositionNotAccepted(format!(
            "domain hard residual {domain_max:e} exceeds {tolerance:e}"
        )));
    }
    if let Some(violation) = linkage.first_branch_violation(&geometry)? {
        return Err(LinkageError::PositionNotAccepted(format!(
            "explicit branch check failed: {violation:?}"
        )));
    }

    let driver_mapping = accepted
        .source_mapping(LinkageSource::Driver(driver))
        .ok_or(LinkageError::UnknownDriver(driver))?;
    let linearization = accepted.session().accepted_hard_linearization()?;
    let mut raw_rates = accepted
        .body_variables()
        .iter()
        .map(|mapping| {
            let rate = linkage
                .require_body(mapping.body_id)?
                .grounded()
                .then_some([0.0; 3]);
            Ok((mapping.body_id, rate))
        })
        .collect::<Result<Vec<_>, LinkageError>>()?;
    let mut selected_rows = 0_usize;

    for component in linearization.components() {
        let mut residual_rate = DVector::zeros(component.hard_rows().len());
        for (index, row) in component.hard_rows().iter().enumerate() {
            if row.row.source_id == driver_mapping.core_source_id
                && driver_mapping.residual_ids.contains(&row.row.residual_id)
                && row.row.row_in_block == 0
            {
                residual_rate[index] = -driver_rate / row.residual_scale;
                selected_rows += 1;
            }
        }
        let solution = component
            .solve_sensitivity(&residual_rate)
            .map_err(|_| LinkageError::VelocityFailure("accepted sensitivity solve failed"))?;
        if !matches!(
            solution.status,
            SensitivityStatus::Unique | SensitivityStatus::UnderdeterminedMinimumNorm
        ) || solution.equation_residual_max > tolerance
        {
            return Err(LinkageError::VelocityFailure(
                "differentiated hard equations did not validate",
            ));
        }
        for block in &solution.raw_tangent_blocks {
            if block.kind != VariableKind::Pose2 || block.values.len() != 3 {
                return Err(LinkageError::VelocityFailure(
                    "accepted tangent block is not a Pose2 velocity",
                ));
            }
            let values = [block.values[0], block.values[1], block.values[2]];
            assign_raw_rate(accepted, &mut raw_rates, block.root, values)?;
            for &alias in &block.alias_members {
                assign_raw_rate(accepted, &mut raw_rates, alias, values)?;
            }
        }
    }
    if selected_rows != 1 {
        return Err(LinkageError::VelocityFailure(
            "selected driver does not map to exactly one accepted hard row",
        ));
    }

    let mut body_velocities = Vec::with_capacity(raw_rates.len());
    for (body_id, raw) in raw_rates {
        let [local_x, local_y, angular] = raw.ok_or(LinkageError::VelocityFailure(
            "free body is absent from the accepted tangent layout",
        ))?;
        let pose = geometry
            .body_pose(body_id)
            .ok_or(LinkageError::UnknownBody(body_id))?;
        let velocity = BodyVelocity {
            body_id,
            linear: pose.transform_vector(Vector2::new(local_x, local_y)),
            angular,
        };
        require_finite_velocity(velocity)?;
        body_velocities.push(velocity);
    }
    apply_velocity_gauges(&geometry, &mut body_velocities, gauges)?;

    let differentiated_residual_max = linkage.differentiated_hard_residual_max(
        &geometry,
        &body_velocities,
        driver,
        driver_rate,
    )?;
    if !differentiated_residual_max.is_finite() || differentiated_residual_max > tolerance {
        return Err(LinkageError::VelocityFailure(
            "differentiated hard equations did not validate",
        ));
    }
    let report = accepted.session().report();
    Ok(VelocityResult {
        driver_id: driver,
        driver_rate,
        body_velocities,
        rank_is_valid: report.rank_is_valid,
        rank: report.rank,
        local_degrees_of_freedom: report.right_nullity,
        is_singular: report.is_singular,
        rank_relative_tolerance: report.rank_relative_tolerance,
        rank_threshold: report.rank_threshold,
        singular_values: report.singular_values.clone(),
        differentiated_residual_max,
    })
}

fn require_same_body_state(
    linkage: &Linkage,
    accepted: &AcceptedCompiledLinkage,
) -> Result<(), LinkageError> {
    for mapping in accepted.body_variables() {
        let expected = linkage.require_body(mapping.body_id)?.pose().ambient();
        let VariableValue::Pose2(actual) = accepted
            .session()
            .problem()
            .variable(mapping.variable_id)
            .ok_or(geosolve_core::CoreError::UnknownVariable(
                mapping.variable_id,
            ))?
            .value()
        else {
            return Err(LinkageError::VelocityFailure(
                "accepted body variable is not Pose2",
            ));
        };
        if actual
            .iter()
            .zip(expected)
            .any(|(actual, expected)| actual.to_bits() != expected.to_bits())
        {
            return Err(LinkageError::PositionNotAccepted(
                "private velocity solve diverged from the current accepted linkage state"
                    .to_owned(),
            ));
        }
    }
    Ok(())
}

fn assign_raw_rate(
    accepted: &AcceptedCompiledLinkage,
    raw_rates: &mut [(BodyId, Option<[f64; 3]>)],
    variable: VariableId,
    values: [f64; 3],
) -> Result<(), LinkageError> {
    let body = accepted
        .body_variables()
        .iter()
        .find_map(|mapping| (mapping.variable_id == variable).then_some(mapping.body_id))
        .ok_or(LinkageError::VelocityFailure(
            "accepted tangent block is not a linkage body",
        ))?;
    let slot = raw_rates
        .iter_mut()
        .find(|(candidate, _)| *candidate == body)
        .ok_or(LinkageError::UnknownBody(body))?;
    if slot.1.replace(values).is_some() {
        return Err(LinkageError::VelocityFailure(
            "accepted body velocity was assigned more than once",
        ));
    }
    Ok(())
}

fn apply_velocity_gauges(
    geometry: &LinkageGeometry,
    velocities: &mut [BodyVelocity],
    gauges: &[VelocityGaugeComponent],
) -> Result<(), LinkageError> {
    for gauge in gauges {
        let reference_velocity = body_velocity(velocities, gauge.reference)?;
        let reference_pose = geometry
            .body_pose(gauge.reference)
            .ok_or(LinkageError::UnknownBody(gauge.reference))?;
        for &body in &gauge.bodies {
            let pose = geometry
                .body_pose(body)
                .ok_or(LinkageError::UnknownBody(body))?;
            let velocity = velocities
                .iter_mut()
                .find(|velocity| velocity.body_id == body)
                .ok_or(LinkageError::UnknownBody(body))?;
            velocity.linear -= reference_velocity.linear
                + perpendicular(pose.translation - reference_pose.translation)
                    * reference_velocity.angular;
            velocity.angular -= reference_velocity.angular;
            require_finite_velocity(*velocity)?;
        }
    }
    Ok(())
}

fn require_finite_velocity(velocity: BodyVelocity) -> Result<(), LinkageError> {
    if velocity.linear.iter().all(|value| value.is_finite()) && velocity.angular.is_finite() {
        Ok(())
    } else {
        Err(LinkageError::VelocityFailure(
            "physical body velocity is non-finite",
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
