// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_core::{
    AcceptedHardLinearization, SensitivityStatus, SolveSession, VariableId, VariableKind,
    VariableValue,
};
use geosolve_geometry::Vector3;
use nalgebra::DVector;

use super::{
    CompiledSpatialAssembly, CoreError, SpatialAssemblyError, SpatialAssemblySession,
    SpatialBodyId, SpatialGeometry, SpatialSourceId, SpatialSourceKind, SpatialSourceMapping,
    accepted_coordinate_values, accepted_session, independent, validate_physical_candidate,
};

/// One prescribed raw position-driver rate.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpatialDriverRate {
    pub source: SpatialSourceId,
    /// Radians/time for a hinge driver or model-units/time for a translation driver.
    pub rate: f64,
}

/// Physical spatial velocity of one body in deterministic body insertion order.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpatialBodyVelocity {
    pub body_id: SpatialBodyId,
    /// Body-origin linear velocity in world coordinates.
    pub origin_linear_world: Vector3<f64>,
    /// Angular velocity in world coordinates.
    pub angular_world: Vector3<f64>,
}

/// World velocity of one body-local point feature.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpatialPointVelocity {
    pub feature_id: super::SpatialPointFeatureId,
    pub body_id: SpatialBodyId,
    pub linear_world: Vector3<f64>,
}

/// World origin/twist and basis-axis rates of one body-local frame feature.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpatialFrameVelocity {
    pub feature_id: super::SpatialFrameFeatureId,
    pub body_id: SpatialBodyId,
    pub origin_linear_world: Vector3<f64>,
    pub angular_world: Vector3<f64>,
    pub x_axis_rate_world: Vector3<f64>,
    pub y_axis_rate_world: Vector3<f64>,
    pub z_axis_rate_world: Vector3<f64>,
}

/// World origin/twist and clock rates of one directed axis feature.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpatialAxisVelocity {
    pub feature_id: super::SpatialAxisFeatureId,
    pub body_id: SpatialBodyId,
    pub origin_linear_world: Vector3<f64>,
    pub angular_world: Vector3<f64>,
    pub direction_rate_world: Vector3<f64>,
    pub x_clock_rate_world: Vector3<f64>,
    pub y_clock_rate_world: Vector3<f64>,
}

/// World origin/twist and clock rates of one directed plane feature.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpatialPlaneVelocity {
    pub feature_id: super::SpatialPlaneFeatureId,
    pub body_id: SpatialBodyId,
    pub origin_linear_world: Vector3<f64>,
    pub angular_world: Vector3<f64>,
    pub normal_rate_world: Vector3<f64>,
    pub x_clock_rate_world: Vector3<f64>,
    pub y_clock_rate_world: Vector3<f64>,
}

/// Typed derivative of one accepted topology-only spatial coordinate.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SpatialCoordinateRateKind {
    Hinge {
        principal_phase_rate: f64,
    },
    AxialTranslation(f64),
    PlanarTranslation {
        axis: super::SpatialPlanarTranslationAxis,
        rate: f64,
    },
}

/// One accepted coordinate derivative in coordinate insertion order.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpatialCoordinateRate {
    pub coordinate: super::SpatialCoordinateId,
    pub rate: SpatialCoordinateRateKind,
}

/// Optional publication controls for a spatial velocity query.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SpatialVelocityOptions {
    /// Include the accepted physical right-nullspace basis without removing gauge modes.
    pub include_motion_basis: bool,
}

/// One normalized body-local tangent block in a physical motion basis vector.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpatialNormalizedBodyTangent {
    pub body_id: SpatialBodyId,
    pub normalized: [f64; 6],
}

/// One deterministic accepted-rank physical motion/nullspace direction.
#[derive(Clone, Debug, PartialEq)]
pub struct SpatialMotionBasisVector {
    pub core_component_index: usize,
    pub normalized_body_tangents: Vec<SpatialNormalizedBodyTangent>,
    pub body_velocities: Vec<SpatialBodyVelocity>,
    pub point_velocities: Vec<SpatialPointVelocity>,
    pub frame_velocities: Vec<SpatialFrameVelocity>,
    pub axis_velocities: Vec<SpatialAxisVelocity>,
    pub plane_velocities: Vec<SpatialPlaneVelocity>,
    pub coordinate_rates: Vec<SpatialCoordinateRate>,
    pub differentiated_residual_max: f64,
}

/// One finite, independently validated spatial velocity representative.
#[derive(Clone, Debug, PartialEq)]
pub struct SpatialVelocitySolution {
    pub accepted_revision: u64,
    pub driver_rates: Vec<SpatialDriverRate>,
    pub body_velocities: Vec<SpatialBodyVelocity>,
    pub point_velocities: Vec<SpatialPointVelocity>,
    pub frame_velocities: Vec<SpatialFrameVelocity>,
    pub axis_velocities: Vec<SpatialAxisVelocity>,
    pub plane_velocities: Vec<SpatialPlaneVelocity>,
    pub coordinate_rates: Vec<SpatialCoordinateRate>,
    /// Empty unless explicitly requested. Gauge directions remain physical here.
    pub motion_basis: Vec<SpatialMotionBasisVector>,
    pub rank: usize,
    pub numerical_right_nullity: usize,
    pub gauge_dof: usize,
    pub internal_mobility: usize,
    pub is_singular: bool,
    pub rank_relative_tolerance: f64,
    pub rank_threshold: f64,
    pub singular_values: Vec<f64>,
    pub differentiated_residual_max: f64,
}

impl SpatialVelocitySolution {
    #[must_use]
    pub fn body(&self, body: SpatialBodyId) -> Option<SpatialBodyVelocity> {
        self.body_velocities
            .iter()
            .find(|velocity| velocity.body_id == body)
            .copied()
    }
}

/// Finite evidence that prescribed rates do not satisfy the accepted hard system.
#[derive(Clone, Debug, PartialEq)]
pub struct SpatialVelocityInconsistency {
    pub accepted_revision: u64,
    pub driver_rates: Vec<SpatialDriverRate>,
    pub inconsistent_component_indices: Vec<usize>,
    pub equation_residual_max: f64,
}

/// Gauge-aware classification of a spatial velocity request.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum SpatialVelocityOutcome {
    /// The representative is unique after removing certified world gauge motion.
    Determinate(SpatialVelocitySolution),
    /// Internal physical motion remains after applying the requested rates.
    Underdetermined(SpatialVelocitySolution),
    /// No body velocity is published for incompatible prescribed rates.
    Inconsistent(SpatialVelocityInconsistency),
}

#[derive(Clone, Debug)]
struct DriverDerivative {
    request: SpatialDriverRate,
    mapping: SpatialSourceMapping,
    normalized_parameter_derivative: f64,
    parameter_scale: f64,
}

#[derive(Clone, Copy, Debug)]
struct KinematicFrame {
    frame: geosolve_geometry::Frame3,
    origin_rate: Vector3<f64>,
    angular: Vector3<f64>,
}

impl KinematicFrame {
    fn x_rate(self) -> Vector3<f64> {
        self.angular.cross(&self.frame.x_axis())
    }

    fn y_rate(self) -> Vector3<f64> {
        self.angular.cross(&self.frame.y_axis())
    }

    fn z_rate(self) -> Vector3<f64> {
        self.angular.cross(&self.frame.z_axis())
    }
}

impl SpatialAssemblySession {
    /// Solves `J_q q_dot + sum(J_s s_dot) = 0` at the accepted spatial position.
    ///
    /// Unlisted position drivers have zero target rate. Consistent solutions use
    /// the accepted physical component rank thresholds and then remove only the
    /// configured numerical-reference world motion. Inconsistent requests never
    /// publish a body velocity.
    ///
    /// # Errors
    ///
    /// Rejects a stale revision, an empty/duplicate/non-finite request, a source
    /// that is not a position driver, stale accepted position evidence, malformed
    /// source mappings, or a sensitivity result that fails independent validation.
    #[allow(clippy::too_many_lines)]
    pub fn velocity(
        &self,
        expected_revision: u64,
        driver_rates: &[SpatialDriverRate],
    ) -> Result<SpatialVelocityOutcome, SpatialAssemblyError> {
        self.velocity_with_options(
            expected_revision,
            driver_rates,
            SpatialVelocityOptions::default(),
        )
    }

    /// Spatial velocity query with optional physical motion-basis publication.
    ///
    /// # Errors
    ///
    /// Returns the same errors as [`Self::velocity`] and additionally rejects a
    /// basis that disagrees with accepted component rank or differentiated rows.
    #[allow(clippy::too_many_lines)]
    pub fn velocity_with_options(
        &self,
        expected_revision: u64,
        driver_rates: &[SpatialDriverRate],
        options: SpatialVelocityOptions,
    ) -> Result<SpatialVelocityOutcome, SpatialAssemblyError> {
        self.require_revision(expected_revision)?;
        let driver_rates = self.validate_spatial_driver_rates(driver_rates)?;
        let coordinate_values =
            accepted_coordinate_values(&self.assembly, &self.accepted_result.geometry)?;
        validate_physical_candidate(
            &self.assembly,
            &self.accepted_result.geometry,
            &coordinate_values,
            &self.core_session,
            &self.source_mappings,
            self.config,
        )?;

        let derivatives = driver_rates
            .iter()
            .copied()
            .map(|request| self.spatial_driver_derivative(request))
            .collect::<Result<Vec<_>, SpatialAssemblyError>>()?;
        let linearization = self.core_session.accepted_hard_linearization()?;
        let tolerance = super::spatial_acceptance_tolerance(self.config);
        let mut raw_rates = self
            .assembly
            .bodies
            .iter()
            .map(|body| {
                let grounded = self.assembly.sources.iter().any(|source| {
                    matches!(
                        source.kind,
                        SpatialSourceKind::PhysicalGround { body: candidate, .. }
                            if candidate == body.id
                    )
                });
                (body.id, grounded.then_some([0.0; 6]))
            })
            .collect::<Vec<_>>();
        let mut mapped_rows = vec![0_usize; derivatives.len()];
        let mut inconsistent_components = Vec::new();
        let mut equation_residual_max = 0.0_f64;

        for component in linearization.components() {
            let mut residual_rate = DVector::zeros(component.hard_rows().len());
            for (row_index, row) in component.hard_rows().iter().enumerate() {
                for (driver_index, derivative) in derivatives.iter().enumerate() {
                    if row.row.source_id == derivative.mapping.core_source_id
                        && derivative
                            .mapping
                            .residual_ids
                            .contains(&row.row.residual_id)
                        && row.row.row_in_block == 0
                    {
                        let normalized_rate = derivative.request.rate / derivative.parameter_scale;
                        residual_rate[row_index] +=
                            derivative.normalized_parameter_derivative * normalized_rate;
                        mapped_rows[driver_index] += 1;
                    }
                }
            }
            let solution = component
                .solve_sensitivity(&residual_rate)
                .map_err(|error| {
                    SpatialAssemblyError::IndependentValidation(format!(
                        "accepted spatial velocity sensitivity failed: {error}"
                    ))
                })?;
            equation_residual_max = equation_residual_max.max(solution.equation_residual_max);
            match solution.status {
                SensitivityStatus::Inconsistent => {
                    inconsistent_components.push(component.component_index());
                }
                SensitivityStatus::Unique | SensitivityStatus::UnderdeterminedMinimumNorm => {
                    if solution.equation_residual_max > tolerance {
                        return independent(format!(
                            "accepted spatial velocity residual {} exceeds {tolerance:e}",
                            solution.equation_residual_max
                        ));
                    }
                    for block in &solution.raw_tangent_blocks {
                        if block.kind != VariableKind::Pose3 || block.values.len() != 6 {
                            return independent(
                                "accepted spatial velocity tangent contains a non-Pose3 block",
                            );
                        }
                        let values = std::array::from_fn(|index| block.values[index]);
                        assign_spatial_raw_rate(self, &mut raw_rates, block.root, values)?;
                        for &alias in &block.alias_members {
                            assign_spatial_raw_rate(self, &mut raw_rates, alias, values)?;
                        }
                    }
                }
                _ => return independent("unknown accepted spatial sensitivity status"),
            }
        }
        for (index, count) in mapped_rows.into_iter().enumerate() {
            if count != 1 {
                return independent(format!(
                    "spatial velocity driver {} maps to {count} accepted hard rows",
                    derivatives[index].request.source
                ));
            }
        }
        if !equation_residual_max.is_finite() {
            return independent("spatial velocity equation residual is non-finite");
        }
        if !inconsistent_components.is_empty() {
            return Ok(SpatialVelocityOutcome::Inconsistent(
                SpatialVelocityInconsistency {
                    accepted_revision: self.revision(),
                    driver_rates,
                    inconsistent_component_indices: inconsistent_components,
                    equation_residual_max,
                },
            ));
        }

        let mut body_velocities =
            spatial_body_velocities(&self.accepted_result.geometry, raw_rates)?;
        apply_spatial_velocity_gauges(
            &self.accepted_result.geometry,
            &mut body_velocities,
            &self.gauge_report,
        )?;
        let differentiated_residual_max =
            self.differentiated_spatial_residual_max(&body_velocities, &driver_rates)?;
        if differentiated_residual_max > tolerance {
            return independent(format!(
                "independent spatial velocity residual {differentiated_residual_max:e} exceeds {tolerance:e}"
            ));
        }
        let point_velocities = self.spatial_point_velocities(&body_velocities)?;
        let frame_velocities = self.spatial_frame_velocities(&body_velocities)?;
        let axis_velocities = self.spatial_axis_velocities(&body_velocities)?;
        let plane_velocities = self.spatial_plane_velocities(&body_velocities)?;
        let coordinate_rates = self.spatial_coordinate_rates(&body_velocities)?;
        let motion_basis = if options.include_motion_basis {
            self.spatial_motion_basis(&linearization, tolerance)?
        } else {
            Vec::new()
        };
        let report = self.core_session.report();
        let solution = SpatialVelocitySolution {
            accepted_revision: self.revision(),
            driver_rates,
            body_velocities,
            point_velocities,
            frame_velocities,
            axis_velocities,
            plane_velocities,
            coordinate_rates,
            motion_basis,
            rank: report.rank,
            numerical_right_nullity: report.right_nullity,
            gauge_dof: self.gauge_report.gauge_dof,
            internal_mobility: self.gauge_report.internal_mobility,
            is_singular: report.is_singular,
            rank_relative_tolerance: report.rank_relative_tolerance,
            rank_threshold: report.rank_threshold,
            singular_values: report.singular_values.clone(),
            differentiated_residual_max,
        };
        if solution.internal_mobility == 0 {
            Ok(SpatialVelocityOutcome::Determinate(solution))
        } else {
            Ok(SpatialVelocityOutcome::Underdetermined(solution))
        }
    }

    fn validate_spatial_driver_rates(
        &self,
        driver_rates: &[SpatialDriverRate],
    ) -> Result<Vec<SpatialDriverRate>, SpatialAssemblyError> {
        if driver_rates.is_empty() {
            return super::invalid_field(
                "spatial_velocity.driver_rates",
                "at least one prescribed driver rate is required",
            );
        }
        let mut rates = driver_rates.to_vec();
        rates.sort_by_key(|request| request.source.as_u64());
        for (index, request) in rates.iter().enumerate() {
            if !request.rate.is_finite() {
                return super::invalid_field(
                    "spatial_velocity.driver_rate",
                    "driver rate must be finite",
                );
            }
            let source = self.assembly.require_source(request.source)?;
            if !matches!(
                source.kind,
                SpatialSourceKind::HingePositionDriver { .. }
                    | SpatialSourceKind::TranslationPositionDriver { .. }
            ) {
                return Err(SpatialAssemblyError::WrongSourceKind {
                    source_id: request.source,
                    expected: "position-driver velocity",
                });
            }
            if index > 0 && rates[index - 1].source == request.source {
                return super::invalid_field(
                    "spatial_velocity.driver_rates",
                    "one source rate was prescribed more than once",
                );
            }
        }
        Ok(rates)
    }

    #[allow(clippy::too_many_lines)]
    fn spatial_driver_derivative(
        &self,
        request: SpatialDriverRate,
    ) -> Result<DriverDerivative, SpatialAssemblyError> {
        let source = self.assembly.require_source(request.source)?;
        let (parameter, parameter_scale) = match source.kind {
            SpatialSourceKind::HingePositionDriver { target, .. } => (target.principal_phase, 1.0),
            SpatialSourceKind::TranslationPositionDriver { target, .. } => {
                (target, self.assembly.model_scale)
            }
            _ => {
                return Err(SpatialAssemblyError::WrongSourceKind {
                    source_id: request.source,
                    expected: "position-driver velocity",
                });
            }
        };
        let mapping = self
            .source_mappings
            .iter()
            .find(|mapping| mapping.source == request.source)
            .cloned()
            .ok_or(SpatialAssemblyError::UnknownSource(request.source))?;
        let (mut parameterized, parameter_variable) = self
            .assembly
            .compile_with_parameterized_driver(request.source, parameter)?;
        add_spatial_velocity_gauges(self, &mut parameterized)?;
        let parameterized_mapping = parameterized
            .source_mapping(request.source)
            .ok_or(SpatialAssemblyError::UnknownSource(request.source))?;
        let parameterized_residual =
            require_single_spatial_driver_residual(&parameterized, parameterized_mapping)?;
        let parameterized_session = accepted_session(
            parameterized.problem.clone(),
            self.config,
            "parameterized spatial velocity derivative solve",
        )?;
        require_spatial_velocity_snapshot(
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
            return independent("spatial velocity parameter changed variable kind");
        };
        if actual_parameter.to_bits() != parameter.to_bits() {
            return independent("private velocity solve changed a spatial driver parameter");
        }
        let linearization = parameterized_session.accepted_hard_linearization()?;
        let component = linearization
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
                    "parameterized spatial velocity row is absent from its component".to_owned(),
                )
            })?;
        let row = component
            .hard_rows()
            .iter()
            .position(|row| row.row.residual_id == parameterized_residual)
            .ok_or_else(|| {
                SpatialAssemblyError::IndependentValidation(
                    "parameterized spatial velocity row index is absent".to_owned(),
                )
            })?;
        let parameter_block = component
            .tangent_blocks()
            .iter()
            .find(|block| block.root == parameter_variable)
            .ok_or_else(|| {
                SpatialAssemblyError::IndependentValidation(
                    "spatial velocity parameter tangent block is absent".to_owned(),
                )
            })?;
        if parameter_block.kind != VariableKind::Scalar || parameter_block.tangent_range.len() != 1
        {
            return independent("spatial velocity parameter tangent metadata is malformed");
        }
        let normalized_parameter_derivative =
            component.normalized_jacobian()[(row, parameter_block.tangent_range.start)];
        if !normalized_parameter_derivative.is_finite() {
            return independent("spatial velocity parameter derivative is non-finite");
        }
        Ok(DriverDerivative {
            request,
            mapping,
            normalized_parameter_derivative,
            parameter_scale,
        })
    }

    #[allow(clippy::too_many_lines)]
    fn differentiated_spatial_residual_max(
        &self,
        velocities: &[SpatialBodyVelocity],
        driver_rates: &[SpatialDriverRate],
    ) -> Result<f64, SpatialAssemblyError> {
        let mut maximum = 0.0_f64;
        let scale = self.assembly.model_scale;
        for source in &self.assembly.sources {
            match source.kind {
                SpatialSourceKind::PhysicalGround { body, .. } => {
                    let velocity = spatial_body_velocity(velocities, body)?;
                    include_spatial_velocity_rates(
                        &mut maximum,
                        &[
                            velocity.origin_linear_world.x / scale,
                            velocity.origin_linear_world.y / scale,
                            velocity.origin_linear_world.z / scale,
                            velocity.angular_world.x,
                            velocity.angular_world.y,
                            velocity.angular_world.z,
                        ],
                    )?;
                }
                SpatialSourceKind::BallJoint { first, second } => {
                    let relative = self.point_velocity(second, velocities)?
                        - self.point_velocity(first, velocities)?;
                    include_spatial_velocity_rates(
                        &mut maximum,
                        &[relative.x / scale, relative.y / scale, relative.z / scale],
                    )?;
                }
                SpatialSourceKind::PointDistanceMate { first, second, .. } => {
                    let first_point = self
                        .accepted_result
                        .geometry
                        .world_point(first)
                        .ok_or(SpatialAssemblyError::UnknownPointFeature(first))?;
                    let second_point = self
                        .accepted_result
                        .geometry
                        .world_point(second)
                        .ok_or(SpatialAssemblyError::UnknownPointFeature(second))?;
                    let difference = second_point - first_point;
                    let distance =
                        super::regular_distance(difference, "spatial velocity point distance")?;
                    let relative = self.point_velocity(second, velocities)?
                        - self.point_velocity(first, velocities)?;
                    include_spatial_velocity_rates(
                        &mut maximum,
                        &[difference.dot(&relative) / (distance * scale)],
                    )?;
                }
                SpatialSourceKind::FixedFrame { first, second } => {
                    let first = self.frame_velocity(first, velocities)?;
                    let second = self.frame_velocity(second, velocities)?;
                    self.include_fixed_frame_rates(&mut maximum, first, second)?;
                }
                SpatialSourceKind::FrameOffsetMate {
                    first,
                    second,
                    offset,
                } => {
                    let first = self.frame_velocity(first, velocities)?;
                    let second = self.frame_velocity(second, velocities)?;
                    let expected_frame = super::compose_frames(first.frame, offset)?;
                    let expected = KinematicFrame {
                        frame: expected_frame,
                        origin_rate: first.origin_rate
                            + first
                                .angular
                                .cross(&(expected_frame.origin() - first.frame.origin())),
                        angular: first.angular,
                    };
                    self.include_fixed_frame_rates(&mut maximum, expected, second)?;
                }
                SpatialSourceKind::RevoluteJoint {
                    first,
                    second,
                    parity,
                } => {
                    let first = self.frame_velocity(first, velocities)?;
                    let second = self.frame_velocity(second, velocities)?;
                    let relative = second.origin_rate - first.origin_rate;
                    let multiplier = parity.multiplier();
                    let second_axis = second.frame.z_axis() * multiplier;
                    let second_axis_rate = second.z_rate() * multiplier;
                    include_spatial_velocity_rates(
                        &mut maximum,
                        &[
                            relative.x / scale,
                            relative.y / scale,
                            relative.z / scale,
                            differentiated_dot(
                                first.frame.x_axis(),
                                first.x_rate(),
                                second_axis,
                                second_axis_rate,
                            ),
                            differentiated_dot(
                                first.frame.y_axis(),
                                first.y_rate(),
                                second_axis,
                                second_axis_rate,
                            ),
                        ],
                    )?;
                }
                SpatialSourceKind::PrismaticJoint {
                    first,
                    second,
                    parity,
                } => {
                    let first = self.axis_velocity(first, velocities)?;
                    let second = self.axis_velocity(second, velocities)?;
                    self.include_prismatic_rates(&mut maximum, first, second, parity, true)?;
                }
                SpatialSourceKind::CylindricalJoint {
                    first,
                    second,
                    parity,
                } => {
                    let first = self.axis_velocity(first, velocities)?;
                    let second = self.axis_velocity(second, velocities)?;
                    self.include_prismatic_rates(&mut maximum, first, second, parity, false)?;
                }
                SpatialSourceKind::PlanarJoint {
                    first,
                    second,
                    parity,
                } => {
                    let first = self.plane_velocity(first, velocities)?;
                    let second = self.plane_velocity(second, velocities)?;
                    let difference = second.frame.origin() - first.frame.origin();
                    let difference_rate = second.origin_rate - first.origin_rate;
                    let multiplier = parity.multiplier();
                    let second_normal = second.frame.z_axis() * multiplier;
                    let second_normal_rate = second.z_rate() * multiplier;
                    include_spatial_velocity_rates(
                        &mut maximum,
                        &[
                            differentiated_dot(
                                first.frame.z_axis(),
                                first.z_rate(),
                                difference,
                                difference_rate,
                            ) / scale,
                            differentiated_dot(
                                first.frame.x_axis(),
                                first.x_rate(),
                                second_normal,
                                second_normal_rate,
                            ),
                            differentiated_dot(
                                first.frame.y_axis(),
                                first.y_rate(),
                                second_normal,
                                second_normal_rate,
                            ),
                        ],
                    )?;
                }
                SpatialSourceKind::UniversalJoint { first, second } => {
                    let first = self.axis_velocity(first, velocities)?;
                    let second = self.axis_velocity(second, velocities)?;
                    let relative = second.origin_rate - first.origin_rate;
                    include_spatial_velocity_rates(
                        &mut maximum,
                        &[
                            relative.x / scale,
                            relative.y / scale,
                            relative.z / scale,
                            differentiated_dot(
                                first.frame.z_axis(),
                                first.z_rate(),
                                second.frame.z_axis(),
                                second.z_rate(),
                            ),
                        ],
                    )?;
                }
                SpatialSourceKind::AxisAngleMate { first, second, .. } => {
                    let first = self.axis_velocity(first, velocities)?;
                    let second = self.axis_velocity(second, velocities)?;
                    let first_axis = first.frame.z_axis();
                    let second_axis = second.frame.z_axis();
                    let cosine_rate = differentiated_dot(
                        first_axis,
                        first.z_rate(),
                        second_axis,
                        second.z_rate(),
                    );
                    let sine = super::regular_distance(
                        first_axis.cross(&second_axis),
                        "spatial velocity axis angle sine",
                    )?;
                    include_spatial_velocity_rates(
                        &mut maximum,
                        &[cosine_rate, -cosine_rate / sine],
                    )?;
                }
                SpatialSourceKind::AxisAlignmentMate {
                    first,
                    second,
                    parity,
                } => {
                    let first = self.axis_velocity(first, velocities)?;
                    let second = self.axis_velocity(second, velocities)?;
                    let multiplier = parity.multiplier();
                    let second_axis = second.frame.z_axis() * multiplier;
                    let second_axis_rate = second.z_rate() * multiplier;
                    include_spatial_velocity_rates(
                        &mut maximum,
                        &[
                            differentiated_dot(
                                first.frame.x_axis(),
                                first.x_rate(),
                                second_axis,
                                second_axis_rate,
                            ),
                            differentiated_dot(
                                first.frame.y_axis(),
                                first.y_rate(),
                                second_axis,
                                second_axis_rate,
                            ),
                        ],
                    )?;
                }
                SpatialSourceKind::HingePositionDriver { coordinate, target } => {
                    let (first, second) =
                        self.coordinate_frame_velocities(coordinate, velocities)?;
                    let sine = first.frame.y_axis().dot(&second.frame.x_axis());
                    let cosine = first.frame.x_axis().dot(&second.frame.x_axis());
                    let sine_rate = differentiated_dot(
                        first.frame.y_axis(),
                        first.y_rate(),
                        second.frame.x_axis(),
                        second.x_rate(),
                    );
                    let cosine_rate = differentiated_dot(
                        first.frame.x_axis(),
                        first.x_rate(),
                        second.frame.x_axis(),
                        second.x_rate(),
                    );
                    let target_rate = requested_spatial_driver_rate(driver_rates, source.id);
                    let (target_sine, target_cosine) = target.principal_phase.sin_cos();
                    let smooth_rate = target_cosine * sine_rate - target_sine * cosine_rate
                        + (-target_sine * sine - target_cosine * cosine) * target_rate;
                    let phase_denominator = sine * sine + cosine * cosine;
                    if !phase_denominator.is_finite() || phase_denominator <= 0.0 {
                        return independent("spatial velocity hinge phase denominator is invalid");
                    }
                    let phase_rate = (cosine * sine_rate - sine * cosine_rate) / phase_denominator;
                    include_spatial_velocity_rates(
                        &mut maximum,
                        &[smooth_rate, phase_rate - target_rate],
                    )?;
                }
                SpatialSourceKind::TranslationPositionDriver { coordinate, .. } => {
                    let definition = self.assembly.require_coordinate(coordinate)?;
                    let (first, second) =
                        self.coordinate_frame_velocities(coordinate, velocities)?;
                    let (axis, axis_rate) = match definition.kind {
                        super::SpatialCoordinateKind::AxialTranslation { .. } => {
                            (first.frame.z_axis(), first.z_rate())
                        }
                        super::SpatialCoordinateKind::PlanarTranslation {
                            axis: super::SpatialPlanarTranslationAxis::X,
                            ..
                        } => (first.frame.x_axis(), first.x_rate()),
                        super::SpatialCoordinateKind::PlanarTranslation {
                            axis: super::SpatialPlanarTranslationAxis::Y,
                            ..
                        } => (first.frame.y_axis(), first.y_rate()),
                        super::SpatialCoordinateKind::Hinge { .. } => {
                            return Err(SpatialAssemblyError::WrongCoordinateKind {
                                coordinate,
                                expected: "translation",
                            });
                        }
                    };
                    let difference = second.frame.origin() - first.frame.origin();
                    let difference_rate = second.origin_rate - first.origin_rate;
                    let measured_rate =
                        differentiated_dot(axis, axis_rate, difference, difference_rate);
                    let target_rate = requested_spatial_driver_rate(driver_rates, source.id);
                    include_spatial_velocity_rates(
                        &mut maximum,
                        &[(measured_rate - target_rate) / scale],
                    )?;
                }
            }
        }
        Ok(maximum)
    }

    fn include_fixed_frame_rates(
        &self,
        maximum: &mut f64,
        first: KinematicFrame,
        second: KinematicFrame,
    ) -> Result<(), SpatialAssemblyError> {
        let relative = second.origin_rate - first.origin_rate;
        include_spatial_velocity_rates(
            maximum,
            &[
                relative.x / self.assembly.model_scale,
                relative.y / self.assembly.model_scale,
                relative.z / self.assembly.model_scale,
                differentiated_dot(
                    first.frame.y_axis(),
                    first.y_rate(),
                    second.frame.x_axis(),
                    second.x_rate(),
                ),
                differentiated_dot(
                    first.frame.z_axis(),
                    first.z_rate(),
                    second.frame.x_axis(),
                    second.x_rate(),
                ),
                differentiated_dot(
                    first.frame.z_axis(),
                    first.z_rate(),
                    second.frame.y_axis(),
                    second.y_rate(),
                ),
            ],
        )
    }

    fn include_prismatic_rates(
        &self,
        maximum: &mut f64,
        first: KinematicFrame,
        second: KinematicFrame,
        parity: super::SpatialAxisParity,
        include_clock: bool,
    ) -> Result<(), SpatialAssemblyError> {
        let difference = second.frame.origin() - first.frame.origin();
        let difference_rate = second.origin_rate - first.origin_rate;
        let multiplier = parity.multiplier();
        let second_axis = second.frame.z_axis() * multiplier;
        let second_axis_rate = second.z_rate() * multiplier;
        let mut rates = vec![
            differentiated_dot(
                first.frame.x_axis(),
                first.x_rate(),
                difference,
                difference_rate,
            ) / self.assembly.model_scale,
            differentiated_dot(
                first.frame.y_axis(),
                first.y_rate(),
                difference,
                difference_rate,
            ) / self.assembly.model_scale,
            differentiated_dot(
                first.frame.x_axis(),
                first.x_rate(),
                second_axis,
                second_axis_rate,
            ),
            differentiated_dot(
                first.frame.y_axis(),
                first.y_rate(),
                second_axis,
                second_axis_rate,
            ),
        ];
        if include_clock {
            rates.push(differentiated_dot(
                first.frame.y_axis(),
                first.y_rate(),
                second.frame.x_axis(),
                second.x_rate(),
            ));
        }
        include_spatial_velocity_rates(maximum, &rates)
    }

    fn point_velocity(
        &self,
        point: super::SpatialPointFeatureId,
        velocities: &[SpatialBodyVelocity],
    ) -> Result<Vector3<f64>, SpatialAssemblyError> {
        let feature = self.assembly.require_point_feature(point)?;
        let world = self
            .accepted_result
            .geometry
            .world_point(point)
            .ok_or(SpatialAssemblyError::UnknownPointFeature(point))?;
        self.rigid_point_velocity(feature.body, world, velocities)
    }

    fn rigid_point_velocity(
        &self,
        body: SpatialBodyId,
        world: geosolve_geometry::Point3<f64>,
        velocities: &[SpatialBodyVelocity],
    ) -> Result<Vector3<f64>, SpatialAssemblyError> {
        let pose = self
            .accepted_result
            .geometry
            .body_pose(body)
            .ok_or(SpatialAssemblyError::UnknownBody(body))?;
        let velocity = spatial_body_velocity(velocities, body)?;
        let result = velocity.origin_linear_world
            + velocity
                .angular_world
                .cross(&(world.coords - pose.translation()));
        if result.iter().all(|value| value.is_finite()) {
            Ok(result)
        } else {
            independent("spatial feature velocity is non-finite")
        }
    }

    fn frame_velocity(
        &self,
        feature: super::SpatialFrameFeatureId,
        velocities: &[SpatialBodyVelocity],
    ) -> Result<KinematicFrame, SpatialAssemblyError> {
        let definition = self.assembly.require_frame_feature(feature)?;
        let frame = self
            .accepted_result
            .geometry
            .world_frame(feature)
            .ok_or(SpatialAssemblyError::UnknownFrameFeature(feature))?;
        self.kinematic_frame(definition.body, frame, velocities)
    }

    fn axis_velocity(
        &self,
        feature: super::SpatialAxisFeatureId,
        velocities: &[SpatialBodyVelocity],
    ) -> Result<KinematicFrame, SpatialAssemblyError> {
        let definition = self.assembly.require_axis_feature(feature)?;
        let frame = self
            .accepted_result
            .geometry
            .world_axis_frame(feature)
            .ok_or(SpatialAssemblyError::UnknownAxisFeature(feature))?;
        self.kinematic_frame(definition.body, frame, velocities)
    }

    fn plane_velocity(
        &self,
        feature: super::SpatialPlaneFeatureId,
        velocities: &[SpatialBodyVelocity],
    ) -> Result<KinematicFrame, SpatialAssemblyError> {
        let definition = self.assembly.require_plane_feature(feature)?;
        let frame = self
            .accepted_result
            .geometry
            .world_plane_frame(feature)
            .ok_or(SpatialAssemblyError::UnknownPlaneFeature(feature))?;
        self.kinematic_frame(definition.body, frame, velocities)
    }

    fn kinematic_frame(
        &self,
        body: SpatialBodyId,
        frame: geosolve_geometry::Frame3,
        velocities: &[SpatialBodyVelocity],
    ) -> Result<KinematicFrame, SpatialAssemblyError> {
        Ok(KinematicFrame {
            origin_rate: self.rigid_point_velocity(body, frame.origin(), velocities)?,
            angular: spatial_body_velocity(velocities, body)?.angular_world,
            frame,
        })
    }

    fn coordinate_frame_velocities(
        &self,
        coordinate: super::SpatialCoordinateId,
        velocities: &[SpatialBodyVelocity],
    ) -> Result<(KinematicFrame, KinematicFrame), SpatialAssemblyError> {
        let definition = self.assembly.require_coordinate(coordinate)?;
        let parent = match definition.kind {
            super::SpatialCoordinateKind::Hinge { parent, .. }
            | super::SpatialCoordinateKind::AxialTranslation { parent }
            | super::SpatialCoordinateKind::PlanarTranslation { parent, .. } => parent,
        };
        match self.assembly.require_source(parent)?.kind {
            SpatialSourceKind::RevoluteJoint { first, second, .. } => Ok((
                self.frame_velocity(first, velocities)?,
                self.frame_velocity(second, velocities)?,
            )),
            SpatialSourceKind::PrismaticJoint { first, second, .. }
            | SpatialSourceKind::CylindricalJoint { first, second, .. } => Ok((
                self.axis_velocity(first, velocities)?,
                self.axis_velocity(second, velocities)?,
            )),
            SpatialSourceKind::PlanarJoint { first, second, .. } => Ok((
                self.plane_velocity(first, velocities)?,
                self.plane_velocity(second, velocities)?,
            )),
            _ => Err(SpatialAssemblyError::WrongCoordinateParent {
                source_id: parent,
                expected: "spatial velocity coordinate",
            }),
        }
    }

    fn spatial_point_velocities(
        &self,
        velocities: &[SpatialBodyVelocity],
    ) -> Result<Vec<SpatialPointVelocity>, SpatialAssemblyError> {
        self.assembly
            .point_features
            .iter()
            .map(|feature| {
                Ok(SpatialPointVelocity {
                    feature_id: feature.id,
                    body_id: feature.body,
                    linear_world: self.point_velocity(feature.id, velocities)?,
                })
            })
            .collect()
    }

    fn spatial_frame_velocities(
        &self,
        velocities: &[SpatialBodyVelocity],
    ) -> Result<Vec<SpatialFrameVelocity>, SpatialAssemblyError> {
        self.assembly
            .frame_features
            .iter()
            .map(|feature| {
                let velocity = self.frame_velocity(feature.id, velocities)?;
                Ok(SpatialFrameVelocity {
                    feature_id: feature.id,
                    body_id: feature.body,
                    origin_linear_world: velocity.origin_rate,
                    angular_world: velocity.angular,
                    x_axis_rate_world: velocity.x_rate(),
                    y_axis_rate_world: velocity.y_rate(),
                    z_axis_rate_world: velocity.z_rate(),
                })
            })
            .collect()
    }

    fn spatial_axis_velocities(
        &self,
        velocities: &[SpatialBodyVelocity],
    ) -> Result<Vec<SpatialAxisVelocity>, SpatialAssemblyError> {
        self.assembly
            .axis_features
            .iter()
            .map(|feature| {
                let velocity = self.axis_velocity(feature.id, velocities)?;
                Ok(SpatialAxisVelocity {
                    feature_id: feature.id,
                    body_id: feature.body,
                    origin_linear_world: velocity.origin_rate,
                    angular_world: velocity.angular,
                    direction_rate_world: velocity.z_rate(),
                    x_clock_rate_world: velocity.x_rate(),
                    y_clock_rate_world: velocity.y_rate(),
                })
            })
            .collect()
    }

    fn spatial_plane_velocities(
        &self,
        velocities: &[SpatialBodyVelocity],
    ) -> Result<Vec<SpatialPlaneVelocity>, SpatialAssemblyError> {
        self.assembly
            .plane_features
            .iter()
            .map(|feature| {
                let velocity = self.plane_velocity(feature.id, velocities)?;
                Ok(SpatialPlaneVelocity {
                    feature_id: feature.id,
                    body_id: feature.body,
                    origin_linear_world: velocity.origin_rate,
                    angular_world: velocity.angular,
                    normal_rate_world: velocity.z_rate(),
                    x_clock_rate_world: velocity.x_rate(),
                    y_clock_rate_world: velocity.y_rate(),
                })
            })
            .collect()
    }

    fn spatial_coordinate_rates(
        &self,
        velocities: &[SpatialBodyVelocity],
    ) -> Result<Vec<SpatialCoordinateRate>, SpatialAssemblyError> {
        self.assembly
            .coordinates
            .iter()
            .map(|coordinate| {
                let (first, second) =
                    self.coordinate_frame_velocities(coordinate.id, velocities)?;
                let difference = second.frame.origin() - first.frame.origin();
                let difference_rate = second.origin_rate - first.origin_rate;
                let rate = match coordinate.kind {
                    super::SpatialCoordinateKind::Hinge { .. } => {
                        let sine = first.frame.y_axis().dot(&second.frame.x_axis());
                        let cosine = first.frame.x_axis().dot(&second.frame.x_axis());
                        let sine_rate = differentiated_dot(
                            first.frame.y_axis(),
                            first.y_rate(),
                            second.frame.x_axis(),
                            second.x_rate(),
                        );
                        let cosine_rate = differentiated_dot(
                            first.frame.x_axis(),
                            first.x_rate(),
                            second.frame.x_axis(),
                            second.x_rate(),
                        );
                        let denominator = sine * sine + cosine * cosine;
                        if !denominator.is_finite() || denominator <= 0.0 {
                            return independent(
                                "spatial coordinate hinge-rate denominator is invalid",
                            );
                        }
                        SpatialCoordinateRateKind::Hinge {
                            principal_phase_rate: (cosine * sine_rate - sine * cosine_rate)
                                / denominator,
                        }
                    }
                    super::SpatialCoordinateKind::AxialTranslation { .. } => {
                        SpatialCoordinateRateKind::AxialTranslation(differentiated_dot(
                            first.frame.z_axis(),
                            first.z_rate(),
                            difference,
                            difference_rate,
                        ))
                    }
                    super::SpatialCoordinateKind::PlanarTranslation { axis, .. } => {
                        let (direction, direction_rate) = match axis {
                            super::SpatialPlanarTranslationAxis::X => {
                                (first.frame.x_axis(), first.x_rate())
                            }
                            super::SpatialPlanarTranslationAxis::Y => {
                                (first.frame.y_axis(), first.y_rate())
                            }
                        };
                        SpatialCoordinateRateKind::PlanarTranslation {
                            axis,
                            rate: differentiated_dot(
                                direction,
                                direction_rate,
                                difference,
                                difference_rate,
                            ),
                        }
                    }
                };
                let scalar = match rate {
                    SpatialCoordinateRateKind::Hinge {
                        principal_phase_rate,
                    } => principal_phase_rate,
                    SpatialCoordinateRateKind::AxialTranslation(rate)
                    | SpatialCoordinateRateKind::PlanarTranslation { rate, .. } => rate,
                };
                if !scalar.is_finite() {
                    return independent("spatial coordinate velocity is non-finite");
                }
                Ok(SpatialCoordinateRate {
                    coordinate: coordinate.id,
                    rate,
                })
            })
            .collect()
    }

    fn spatial_motion_basis(
        &self,
        linearization: &AcceptedHardLinearization,
        tolerance: f64,
    ) -> Result<Vec<SpatialMotionBasisVector>, SpatialAssemblyError> {
        let mut result = Vec::new();
        for component in linearization.components() {
            let basis = component.right_nullspace_basis().map_err(|error| {
                SpatialAssemblyError::IndependentValidation(format!(
                    "accepted spatial motion basis failed: {error}"
                ))
            })?;
            if basis.right_nullity != component.right_nullity()
                || basis.vectors.len() != basis.right_nullity
            {
                return independent(
                    "accepted spatial motion basis dimension changed during construction",
                );
            }
            for vector in basis.vectors {
                let mut raw_rates = self
                    .assembly
                    .bodies
                    .iter()
                    .map(|body| (body.id, None))
                    .collect::<Vec<_>>();
                for block in &vector.raw_tangent_blocks {
                    if block.kind != VariableKind::Pose3 || block.values.len() != 6 {
                        return independent(
                            "accepted spatial motion basis contains a non-Pose3 block",
                        );
                    }
                    let values = std::array::from_fn(|index| block.values[index]);
                    assign_spatial_raw_rate(self, &mut raw_rates, block.root, values)?;
                    for &alias in &block.alias_members {
                        assign_spatial_raw_rate(self, &mut raw_rates, alias, values)?;
                    }
                }
                for (_, raw) in &mut raw_rates {
                    if raw.is_none() {
                        *raw = Some([0.0; 6]);
                    }
                }
                let body_velocities =
                    spatial_body_velocities(&self.accepted_result.geometry, raw_rates)?;
                let differentiated_residual_max = self
                    .differentiated_spatial_residual_max(&body_velocities, &[])?
                    .max(vector.equation_residual_max);
                if !differentiated_residual_max.is_finite()
                    || differentiated_residual_max > tolerance
                {
                    return independent(format!(
                        "spatial motion-basis residual {differentiated_residual_max:e} exceeds {tolerance:e}"
                    ));
                }
                let normalized_body_tangents = component
                    .tangent_blocks()
                    .iter()
                    .map(|block| {
                        if block.kind != VariableKind::Pose3 || block.tangent_range.len() != 6 {
                            return independent(
                                "accepted spatial motion basis tangent metadata is malformed",
                            );
                        }
                        let body_id = self
                            .body_variables
                            .iter()
                            .find_map(|mapping| {
                                (mapping.variable_id == block.root).then_some(mapping.body_id)
                            })
                            .ok_or_else(|| {
                                SpatialAssemblyError::IndependentValidation(
                                    "spatial motion-basis tangent is not an assembly body"
                                        .to_owned(),
                                )
                            })?;
                        Ok(SpatialNormalizedBodyTangent {
                            body_id,
                            normalized: std::array::from_fn(|index| {
                                vector.normalized_tangent[block.tangent_range.start + index]
                            }),
                        })
                    })
                    .collect::<Result<Vec<_>, SpatialAssemblyError>>()?;
                result.push(SpatialMotionBasisVector {
                    core_component_index: component.component_index(),
                    normalized_body_tangents,
                    point_velocities: self.spatial_point_velocities(&body_velocities)?,
                    frame_velocities: self.spatial_frame_velocities(&body_velocities)?,
                    axis_velocities: self.spatial_axis_velocities(&body_velocities)?,
                    plane_velocities: self.spatial_plane_velocities(&body_velocities)?,
                    coordinate_rates: self.spatial_coordinate_rates(&body_velocities)?,
                    body_velocities,
                    differentiated_residual_max,
                });
            }
        }
        if result.len() != self.core_session.report().right_nullity {
            return independent(format!(
                "spatial motion basis has {} vectors but accepted right nullity is {}",
                result.len(),
                self.core_session.report().right_nullity
            ));
        }
        Ok(result)
    }
}

fn differentiated_dot(
    first: Vector3<f64>,
    first_rate: Vector3<f64>,
    second: Vector3<f64>,
    second_rate: Vector3<f64>,
) -> f64 {
    first_rate.dot(&second) + first.dot(&second_rate)
}

fn include_spatial_velocity_rates(
    maximum: &mut f64,
    rates: &[f64],
) -> Result<(), SpatialAssemblyError> {
    for &rate in rates {
        if !rate.is_finite() {
            return independent("independent spatial velocity residual is non-finite");
        }
        *maximum = maximum.max(rate.abs());
    }
    Ok(())
}

fn requested_spatial_driver_rate(rates: &[SpatialDriverRate], source: SpatialSourceId) -> f64 {
    rates
        .iter()
        .find_map(|request| (request.source == source).then_some(request.rate))
        .unwrap_or(0.0)
}

fn assign_spatial_raw_rate(
    session: &SpatialAssemblySession,
    raw_rates: &mut [(SpatialBodyId, Option<[f64; 6]>)],
    variable: VariableId,
    values: [f64; 6],
) -> Result<(), SpatialAssemblyError> {
    let body = session
        .body_variables
        .iter()
        .find_map(|mapping| (mapping.variable_id == variable).then_some(mapping.body_id))
        .ok_or_else(|| {
            SpatialAssemblyError::IndependentValidation(
                "accepted spatial velocity tangent is not an assembly body".to_owned(),
            )
        })?;
    let slot = raw_rates
        .iter_mut()
        .find(|(candidate, _)| *candidate == body)
        .ok_or(SpatialAssemblyError::UnknownBody(body))?;
    if slot.1.replace(values).is_some() {
        return independent("accepted spatial body velocity was assigned more than once");
    }
    Ok(())
}

fn spatial_body_velocities(
    geometry: &SpatialGeometry,
    raw_rates: Vec<(SpatialBodyId, Option<[f64; 6]>)>,
) -> Result<Vec<SpatialBodyVelocity>, SpatialAssemblyError> {
    raw_rates
        .into_iter()
        .map(|(body_id, raw)| {
            let raw = raw.ok_or_else(|| {
                SpatialAssemblyError::IndependentValidation(
                    "free spatial body is absent from the accepted tangent layout".to_owned(),
                )
            })?;
            let pose = geometry
                .body_pose(body_id)
                .ok_or(SpatialAssemblyError::UnknownBody(body_id))?;
            let velocity = SpatialBodyVelocity {
                body_id,
                origin_linear_world: pose
                    .try_transform_vector(Vector3::new(raw[0], raw[1], raw[2]))?,
                angular_world: pose.try_transform_vector(Vector3::new(raw[3], raw[4], raw[5]))?,
            };
            require_finite_spatial_body_velocity(velocity)?;
            Ok(velocity)
        })
        .collect()
}

fn apply_spatial_velocity_gauges(
    geometry: &SpatialGeometry,
    velocities: &mut [SpatialBodyVelocity],
    gauge_report: &super::SpatialGaugeReport,
) -> Result<(), SpatialAssemblyError> {
    for component in &gauge_report.components {
        let Some(reference) = component.numerical_reference else {
            continue;
        };
        let reference_velocity = spatial_body_velocity(velocities, reference.body)?;
        let reference_pose = geometry
            .body_pose(reference.body)
            .ok_or(SpatialAssemblyError::UnknownBody(reference.body))?;
        for &body in &component.bodies {
            let pose = geometry
                .body_pose(body)
                .ok_or(SpatialAssemblyError::UnknownBody(body))?;
            let velocity = velocities
                .iter_mut()
                .find(|velocity| velocity.body_id == body)
                .ok_or(SpatialAssemblyError::UnknownBody(body))?;
            velocity.origin_linear_world -= reference_velocity.origin_linear_world
                + reference_velocity
                    .angular_world
                    .cross(&(pose.translation() - reference_pose.translation()));
            velocity.angular_world -= reference_velocity.angular_world;
            require_finite_spatial_body_velocity(*velocity)?;
        }
    }
    Ok(())
}

fn spatial_body_velocity(
    velocities: &[SpatialBodyVelocity],
    body: SpatialBodyId,
) -> Result<SpatialBodyVelocity, SpatialAssemblyError> {
    velocities
        .iter()
        .find(|velocity| velocity.body_id == body)
        .copied()
        .ok_or(SpatialAssemblyError::UnknownBody(body))
}

fn require_finite_spatial_body_velocity(
    velocity: SpatialBodyVelocity,
) -> Result<(), SpatialAssemblyError> {
    if velocity
        .origin_linear_world
        .iter()
        .chain(velocity.angular_world.iter())
        .all(|value| value.is_finite())
    {
        Ok(())
    } else {
        independent("physical spatial body velocity is non-finite")
    }
}

fn add_spatial_velocity_gauges(
    session: &SpatialAssemblySession,
    compiled: &mut CompiledSpatialAssembly,
) -> Result<(), SpatialAssemblyError> {
    for reference in session
        .gauge_report
        .components
        .iter()
        .filter_map(|component| component.numerical_reference)
    {
        let accepted = session
            .accepted_result
            .geometry
            .body_pose(reference.body)
            .ok_or(SpatialAssemblyError::UnknownBody(reference.body))?;
        compiled.add_numerical_pose_gauge(
            reference.body,
            accepted,
            session.assembly.model_scale,
        )?;
    }
    Ok(())
}

fn require_single_spatial_driver_residual(
    compiled: &CompiledSpatialAssembly,
    mapping: &SpatialSourceMapping,
) -> Result<super::ResidualId, SpatialAssemblyError> {
    let [residual] = mapping.residual_ids.as_slice() else {
        return independent("spatial velocity driver must map to exactly one residual block");
    };
    let block = compiled
        .problem
        .residual(*residual)
        .ok_or(CoreError::UnknownResidual(*residual))?;
    if block.output_dimension() != 1 {
        return independent("spatial velocity driver residual must contain exactly one row");
    }
    Ok(*residual)
}

fn require_spatial_velocity_snapshot(
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
            return independent("private spatial velocity solve changed a body variable kind");
        };
        if actual
            .iter()
            .zip(expected)
            .any(|(actual, expected)| actual.to_bits() != expected.to_bits())
        {
            return independent(
                "private spatial velocity solve diverged from the accepted assembly state",
            );
        }
    }
    Ok(())
}
