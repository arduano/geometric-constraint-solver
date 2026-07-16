use geosolve_core::{
    AuditBinding, AuditEvaluationStatus, AuditSnapshot, HardValidity, Problem, ResidualBlock,
    ResidualCategory, ResidualId, ResidualRowAudit, SolveReport, SolveTermination, SolverConfig,
    SourceConstraint, SourceConstraintId, VariableBlock, VariableId, VariableValue,
};
use geosolve_geometry::{PlaneFrame, Point2, Point3, Pose2, Vector2, Vector3};

use crate::model::{
    AxisFeatureId, BodyId, BranchMonitor, BranchMonitorId, BranchSign, BranchViolation, DriverId,
    DriverKind, JointId, JointKind, Linkage, LinkageError, PointFeatureId, validate_finite,
    validate_model_scale, validate_plane_frame, validate_point, validate_pose,
};
use crate::residuals::{
    AngularDriverResidual, LinearDriverResidual, PrismaticResidual, RevoluteResidual, WeldResidual,
};

const MAX_CONTINUATION_SAMPLES: usize = 1_000_000;
const MINIMUM_NORMALIZED_BRANCH_MARGIN: f64 = 1.0e-3;
const RANK_WARNING_SINGULAR_VALUE_RATIO: f64 = 1.0e-3;

/// Domain identity corresponding to one deterministic compiled source.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LinkageSource {
    Ground(BodyId),
    Joint(JointId),
    Driver(DriverId),
}

/// Exact mapping from a high-level linkage source to executable core rows.
#[derive(Clone, Debug, PartialEq)]
pub struct LinkageSourceMapping {
    pub source: LinkageSource,
    pub source_label: String,
    pub core_source_id: SourceConstraintId,
    pub residual_ids: Vec<ResidualId>,
}

/// Exact body-to-core-variable relationship in body insertion order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BodyVariableMapping {
    pub body_id: BodyId,
    pub variable_id: VariableId,
}

#[derive(Clone, Copy, Debug)]
struct PointFeatureSpec {
    id: PointFeatureId,
    body: BodyId,
    local: Point2<f64>,
}

#[derive(Clone, Copy, Debug)]
struct AxisFeatureSpec {
    id: AxisFeatureId,
    body: BodyId,
    local: Vector2<f64>,
}

/// Narrow read-only core seam for audit, Jacobian, and velocity verification.
#[derive(Debug)]
pub struct CompiledLinkage {
    pub(crate) problem: Problem,
    body_variables: Vec<BodyVariableMapping>,
    source_mappings: Vec<LinkageSourceMapping>,
    plane_frame: PlaneFrame,
    point_features: Vec<PointFeatureSpec>,
    axis_features: Vec<AxisFeatureSpec>,
}

impl CompiledLinkage {
    #[must_use]
    pub const fn problem(&self) -> &Problem {
        &self.problem
    }

    #[must_use]
    pub fn body_variables(&self) -> &[BodyVariableMapping] {
        &self.body_variables
    }

    #[must_use]
    pub fn source_mappings(&self) -> &[LinkageSourceMapping] {
        &self.source_mappings
    }

    #[must_use]
    pub fn variable_for_body(&self, body: BodyId) -> Option<VariableId> {
        self.body_variables
            .iter()
            .find_map(|mapping| (mapping.body_id == body).then_some(mapping.variable_id))
    }

    #[must_use]
    pub fn source_mapping(&self, source: LinkageSource) -> Option<&LinkageSourceMapping> {
        self.source_mappings
            .iter()
            .find(|mapping| mapping.source == source)
    }

    pub(crate) fn solved_geometry(&self) -> Result<LinkageGeometry, LinkageError> {
        let mut bodies = Vec::with_capacity(self.body_variables.len());
        for mapping in &self.body_variables {
            let variable = self.problem.variable(mapping.variable_id).ok_or(
                geosolve_core::CoreError::UnknownVariable(mapping.variable_id),
            )?;
            let VariableValue::Pose2([x, y, angle]) = variable.value() else {
                return Err(geosolve_core::CoreError::VariableKindMismatch {
                    expected: geosolve_core::VariableKind::Pose2,
                    actual: variable.kind(),
                }
                .into());
            };
            let pose =
                Pose2::from_ambient([x, y, angle]).map_err(|_| LinkageError::NonFinitePose {
                    context: "solved body",
                })?;
            validate_pose(pose, "solved body")?;
            bodies.push(SolvedBody {
                body_id: mapping.body_id,
                pose,
            });
        }
        geometry_from_parts(
            self.plane_frame,
            bodies,
            &self.point_features,
            &self.axis_features,
        )
    }
}

/// One solved body pose in deterministic insertion order.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SolvedBody {
    pub body_id: BodyId,
    pub pose: Pose2,
}

/// One body-local point transformed into planar and embedded world coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TransformedPointFeature {
    pub feature_id: PointFeatureId,
    pub body_id: BodyId,
    pub planar: Point2<f64>,
    pub world: Point3<f64>,
}

/// One body-local axis transformed into planar and embedded world coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TransformedAxisFeature {
    pub feature_id: AxisFeatureId,
    pub body_id: BodyId,
    pub planar: Vector2<f64>,
    pub world: Vector3<f64>,
}

/// Accepted finite mechanism geometry and its 2D-to-3D mapping.
#[derive(Clone, Debug, PartialEq)]
pub struct LinkageGeometry {
    pub plane_frame: PlaneFrame,
    pub bodies: Vec<SolvedBody>,
    pub points: Vec<TransformedPointFeature>,
    pub axes: Vec<TransformedAxisFeature>,
}

/// The geometric operation used by an explicit branch monitor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BranchMonitorKind {
    Orientation,
    DirectedDisplacement,
}

/// Typed evaluation of one branch monitor against supplied linkage geometry.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BranchEvaluation {
    pub monitor_id: BranchMonitorId,
    pub kind: BranchMonitorKind,
    pub expected_sign: BranchSign,
    /// Raw signed cross product or guide-axis projection in model coordinates.
    pub signed_metric: f64,
    /// Whether the signed, scale-normalized metric clears the branch margin.
    pub retained: bool,
}

impl LinkageGeometry {
    #[must_use]
    pub fn body_pose(&self, body: BodyId) -> Option<Pose2> {
        self.bodies
            .iter()
            .find_map(|item| (item.body_id == body).then_some(item.pose))
    }

    #[must_use]
    pub fn point(&self, feature: PointFeatureId) -> Option<Point2<f64>> {
        self.points
            .iter()
            .find_map(|item| (item.feature_id == feature).then_some(item.planar))
    }

    #[must_use]
    pub fn world_point(&self, feature: PointFeatureId) -> Option<Point3<f64>> {
        self.points
            .iter()
            .find_map(|item| (item.feature_id == feature).then_some(item.world))
    }

    #[must_use]
    pub fn axis(&self, feature: AxisFeatureId) -> Option<Vector2<f64>> {
        self.axes
            .iter()
            .find_map(|item| (item.feature_id == feature).then_some(item.planar))
    }

    #[must_use]
    pub fn world_axis(&self, feature: AxisFeatureId) -> Option<Vector3<f64>> {
        self.axes
            .iter()
            .find_map(|item| (item.feature_id == feature).then_some(item.world))
    }
}

/// Why a core-returned candidate was not committed to the linkage.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum SolveRejection {
    CoreTermination(SolveTermination),
    HardResidual { maximum: f64, tolerance: f64 },
    IndependentValidationFailed(String),
    BranchViolation(BranchViolation),
}

/// One transactional position solve. `geometry` and `display_audit` are retained state.
#[derive(Debug)]
pub struct LinkageSolveResult {
    pub geometry: LinkageGeometry,
    pub display_audit: AuditSnapshot,
    /// Source identities and labels corresponding exactly to `display_audit`.
    pub source_mappings: Vec<LinkageSourceMapping>,
    /// Source identities and labels corresponding to the attempted `core_report`.
    pub attempt_source_mappings: Vec<LinkageSourceMapping>,
    /// Attempt report whose `hard_validity` includes linkage equation/branch validation.
    pub core_report: SolveReport,
    pub diagnostics: LinkageSolveDiagnostics,
    pub rejection: Option<SolveRejection>,
    pub acceptance_hard_residual_max: Option<f64>,
}

impl LinkageSolveResult {
    #[must_use]
    pub const fn accepted(&self) -> bool {
        self.rejection.is_none()
    }
}

/// Domain-level continuation diagnostics evaluated from the attempted report.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LinkageSolveDiagnostics {
    /// Minimum within-component positive singular-value ratio, when rank data exists.
    pub singular_value_ratio: Option<f64>,
    /// Core singular/near-singular state or the frozen per-component linkage warning policy.
    pub has_rank_warning: bool,
}

/// One deterministic warm-started continuation sample.
#[derive(Debug)]
pub struct DriveSample {
    pub target: f64,
    pub step: f64,
    pub solve: LinkageSolveResult,
}

/// Complete bounded-step continuation outcome, including the first failed sample.
#[derive(Debug)]
pub struct DriveResult {
    pub driver_id: DriverId,
    pub requested_target: f64,
    pub initial_target: f64,
    pub accepted_target: f64,
    pub samples: Vec<DriveSample>,
}

impl DriveResult {
    #[must_use]
    pub fn completed(&self) -> bool {
        matches!(
            self.accepted_target.partial_cmp(&self.requested_target),
            Some(std::cmp::Ordering::Equal)
        ) && !self.samples.is_empty()
            && self.samples.iter().all(|sample| sample.solve.accepted())
    }
}

impl Linkage {
    /// Compiles accepted poses, hard joints, grounded eliminations, and drivers.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid model geometry, stale references, or a
    /// rejected core declaration.
    pub fn compile(&self) -> Result<CompiledLinkage, LinkageError> {
        self.compile_with_driver_override(None)
    }

    #[allow(clippy::too_many_lines)]
    pub(crate) fn compile_with_driver_override(
        &self,
        driver_override: Option<(DriverId, f64)>,
    ) -> Result<CompiledLinkage, LinkageError> {
        validate_model_scale(self.model_scale)?;
        validate_plane_frame(self.plane_frame)?;
        if let Some((driver, target)) = driver_override {
            self.drivers
                .get(driver)
                .ok_or(LinkageError::UnknownDriver(driver))?;
            validate_finite(target, "driver target")?;
        }
        self.geometry()?;

        let mut problem = Problem::new();
        let mut body_variables = Vec::new();
        for (body_id, body) in self.bodies.iter() {
            validate_pose(body.pose(), "rigid body")?;
            let variable_id = problem.add_variable(VariableBlock::pose2(
                body.pose().ambient(),
                [self.model_scale, self.model_scale, 1.0],
            )?);
            body_variables.push(BodyVariableMapping {
                body_id,
                variable_id,
            });
        }

        let mut source_mappings = Vec::new();
        for (body_id, body) in self.bodies.iter().filter(|(_, body)| body.grounded()) {
            let label = format!("grounded body {}", body.label());
            let source_id = problem.add_source(SourceConstraint::new(&label)?);
            let variable_id = body_variable(&body_variables, body_id)?;
            let fixed = VariableValue::Pose2(body.pose().ambient());
            let residual_id = problem.add_residual(ResidualBlock::fixed_variable(
                source_id,
                variable_id,
                fixed,
                vec![self.model_scale, self.model_scale, 1.0],
                vec![
                    audit_row(
                        format!(
                            "local_difference(accepted_pose, {}.pose).v_x / model_scale",
                            body.label()
                        ),
                        vec![AuditBinding::new("body", body.label())],
                        "model-unit",
                    ),
                    audit_row(
                        format!(
                            "local_difference(accepted_pose, {}.pose).v_y / model_scale",
                            body.label()
                        ),
                        vec![AuditBinding::new("body", body.label())],
                        "model-unit",
                    ),
                    audit_row(
                        format!(
                            "local_difference(accepted_pose, {}.pose).omega / 1 rad",
                            body.label()
                        ),
                        vec![AuditBinding::new("body", body.label())],
                        "rad",
                    ),
                ],
            )?)?;
            problem.declare_fixed_variable(variable_id, fixed, residual_id)?;
            source_mappings.push(source_mapping(
                LinkageSource::Ground(body_id),
                label,
                source_id,
                residual_id,
            ));
        }

        for (joint_id, joint) in self.joints.iter() {
            source_mappings.push(compile_joint(
                self,
                &mut problem,
                &body_variables,
                joint_id,
                joint,
            )?);
        }
        for (driver_id, driver) in self.drivers.iter() {
            let target = driver_override
                .filter(|(id, _)| *id == driver_id)
                .map_or(driver.target(), |(_, target)| target);
            source_mappings.push(compile_driver(
                self,
                &mut problem,
                &body_variables,
                driver_id,
                driver,
                target,
            )?);
        }

        // Fixed declarations synchronize eagerly. Restore the exact retained
        // warm start; solve-time synchronization remains trusted in core.
        for mapping in &body_variables {
            let pose = self.require_body(mapping.body_id)?.pose();
            problem
                .set_variable_value(mapping.variable_id, VariableValue::Pose2(pose.ambient()))?;
        }

        Ok(CompiledLinkage {
            problem,
            body_variables,
            source_mappings,
            plane_frame: self.plane_frame,
            point_features: self
                .point_features
                .iter()
                .map(|(id, feature)| PointFeatureSpec {
                    id,
                    body: feature.body(),
                    local: feature.local_point(),
                })
                .collect(),
            axis_features: self
                .axis_features
                .iter()
                .map(|(id, feature)| AxisFeatureSpec {
                    id,
                    body: feature.body(),
                    local: feature.local_axis(),
                })
                .collect(),
        })
    }

    /// Solves current targets, validates independently, and commits only on acceptance.
    ///
    /// # Errors
    ///
    /// Returns an error when compilation or the core solve cannot be started.
    /// Numerical solve failures are returned as rejected results.
    pub fn solve(&mut self, config: SolverConfig) -> Result<LinkageSolveResult, LinkageError> {
        self.solve_attempt(None, config)
    }

    /// Drives one target with deterministic samples no larger than its policy step.
    ///
    /// # Errors
    ///
    /// Returns an error for a stale driver, non-finite target, excessive sample
    /// count, or when a sample solve cannot be started.
    pub fn drive_to(
        &mut self,
        driver: DriverId,
        target: f64,
        config: SolverConfig,
    ) -> Result<DriveResult, LinkageError> {
        validate_finite(target, "requested driver target")?;
        let driver_value = self
            .drivers
            .get(driver)
            .ok_or(LinkageError::UnknownDriver(driver))?;
        let initial_target = driver_value.target();
        let max_step = driver_value.max_continuation_step();
        let change = target - initial_target;
        if !change.is_finite() {
            return Err(LinkageError::NonFiniteValue {
                context: "driver target change",
                value: change,
            });
        }
        let direction = change.signum();
        let conservative_step = max_step * (1.0 - 1024.0 * f64::EPSILON);
        if change.abs() / conservative_step > 1_000_000.0 {
            return Err(LinkageError::ContinuationSampleOverflow);
        }
        let mut samples = Vec::new();
        if matches!(
            initial_target.partial_cmp(&target),
            Some(std::cmp::Ordering::Equal)
        ) {
            let solve = self.solve_attempt(Some((driver, target)), config)?;
            samples.push(DriveSample {
                target,
                step: 0.0,
                solve,
            });
        }
        loop {
            let previous_target = self
                .drivers
                .get(driver)
                .ok_or(LinkageError::UnknownDriver(driver))?
                .target();
            if matches!(
                previous_target.partial_cmp(&target),
                Some(std::cmp::Ordering::Equal)
            ) {
                break;
            }
            if samples.len() >= MAX_CONTINUATION_SAMPLES {
                return Err(LinkageError::ContinuationSampleOverflow);
            }
            let remaining = target - previous_target;
            let sample_target = if remaining.abs() <= max_step {
                target
            } else {
                previous_target + direction * conservative_step
            };
            let step = sample_target - previous_target;
            if !step.is_finite() || step == 0.0 || step.abs() > max_step {
                return Err(LinkageError::ContinuationSampleOverflow);
            }
            let solve = self.solve_attempt(Some((driver, sample_target)), config)?;
            let accepted = solve.accepted();
            samples.push(DriveSample {
                target: sample_target,
                step,
                solve,
            });
            if !accepted {
                break;
            }
        }
        let accepted_target = self
            .drivers
            .get(driver)
            .ok_or(LinkageError::UnknownDriver(driver))?
            .target();
        Ok(DriveResult {
            driver_id: driver,
            requested_target: target,
            initial_target,
            accepted_target,
            samples,
        })
    }

    /// Returns accepted body and transformed feature geometry.
    ///
    /// # Errors
    ///
    /// Returns an error if stored geometry is stale, invalid, or transforms to
    /// non-finite planar/world values.
    pub fn geometry(&self) -> Result<LinkageGeometry, LinkageError> {
        validate_plane_frame(self.plane_frame)?;
        let bodies: Vec<_> = self
            .bodies
            .iter()
            .map(|(body_id, body)| {
                validate_pose(body.pose(), "rigid body")?;
                Ok(SolvedBody {
                    body_id,
                    pose: body.pose(),
                })
            })
            .collect::<Result<_, LinkageError>>()?;
        let points: Vec<_> = self
            .point_features
            .iter()
            .map(|(id, feature)| PointFeatureSpec {
                id,
                body: feature.body(),
                local: feature.local_point(),
            })
            .collect();
        let axes: Vec<_> = self
            .axis_features
            .iter()
            .map(|(id, feature)| AxisFeatureSpec {
                id,
                body: feature.body(),
                local: feature.local_axis(),
            })
            .collect();
        geometry_from_parts(self.plane_frame, bodies, &points, &axes)
    }

    fn solve_attempt(
        &mut self,
        driver_override: Option<(DriverId, f64)>,
        config: SolverConfig,
    ) -> Result<LinkageSolveResult, LinkageError> {
        let mut retained_compiled = self.compile()?;
        let retained_geometry = retained_compiled.solved_geometry()?;
        let retained_audit = retained_compiled.problem.audit_snapshot_partial();
        let retained_source_mappings = retained_compiled.source_mappings.clone();
        let mut compiled = self.compile_with_driver_override(driver_override)?;
        let mut core_report = compiled.problem.solve(config)?;
        let attempt_source_mappings = compiled.source_mappings.clone();
        let candidate = compiled.solved_geometry();
        let mut acceptance_hard_residual_max = None;

        let core_hard_validity = core_report.hard_validity;
        let domain_rejection = if core_hard_validity == HardValidity::Valid {
            if let Err(error) = &candidate {
                Some(SolveRejection::IndependentValidationFailed(
                    error.to_string(),
                ))
            } else {
                let candidate = candidate.as_ref().expect("candidate checked above");
                match fresh_hard_audit_max(&compiled.problem).and_then(|audit_max| {
                    self.domain_hard_residual_max(candidate, driver_override)
                        .map(|domain_max| audit_max.max(domain_max))
                }) {
                    Ok(maximum) => {
                        acceptance_hard_residual_max = Some(maximum);
                        if maximum > config.normalized_residual_tolerance {
                            Some(SolveRejection::HardResidual {
                                maximum,
                                tolerance: config.normalized_residual_tolerance,
                            })
                        } else {
                            match self.first_branch_violation(candidate) {
                                Ok(violation) => violation.map(SolveRejection::BranchViolation),
                                Err(error) => Some(SolveRejection::IndependentValidationFailed(
                                    error.to_string(),
                                )),
                            }
                        }
                    }
                    Err(error) => Some(SolveRejection::IndependentValidationFailed(
                        error.to_string(),
                    )),
                }
            }
        } else {
            None
        };
        core_report.hard_validity =
            domain_hard_validity(core_hard_validity, domain_rejection.as_ref());

        let rejection = if let Some(rejection) = domain_rejection {
            Some(rejection)
        } else if core_report.termination != SolveTermination::Converged {
            Some(SolveRejection::CoreTermination(core_report.termination))
        } else if core_hard_validity != HardValidity::Valid
            || !core_report.hard_residuals_validated
            || core_report.hard_residual_max > config.normalized_residual_tolerance
        {
            Some(SolveRejection::HardResidual {
                maximum: core_report.hard_residual_max,
                tolerance: config.normalized_residual_tolerance,
            })
        } else {
            None
        };

        if rejection.is_none() {
            let candidate = candidate.expect("accepted solve has finite candidate");
            for body in &candidate.bodies {
                self.set_body_pose(body.body_id, body.pose)?;
            }
            if let Some((driver, target)) = driver_override {
                self.set_driver_target_accepted(driver, target)?;
            }
        }
        let (display_audit, source_mappings) = if rejection.is_none() {
            (core_report.audit.clone(), attempt_source_mappings.clone())
        } else {
            let annotated =
                refresh_retained_audit(self, &mut retained_compiled, &retained_geometry, config)
                    .unwrap_or(retained_audit);
            (annotated, retained_source_mappings)
        };
        let diagnostics = linkage_diagnostics(&core_report);
        Ok(LinkageSolveResult {
            geometry: self.geometry()?,
            display_audit,
            source_mappings,
            attempt_source_mappings,
            core_report,
            diagnostics,
            rejection,
            acceptance_hard_residual_max,
        })
    }

    pub(crate) fn domain_hard_residual_max(
        &self,
        geometry: &LinkageGeometry,
        driver_override: Option<(DriverId, f64)>,
    ) -> Result<f64, LinkageError> {
        let mut maximum = 0.0_f64;
        for (body_id, body) in self.bodies.iter().filter(|(_, body)| body.grounded()) {
            let solved = geometry
                .body_pose(body_id)
                .ok_or(LinkageError::UnknownBody(body_id))?;
            let difference =
                body.pose()
                    .local_difference(&solved)
                    .map_err(|_| LinkageError::NonFinitePose {
                        context: "ground local difference",
                    })?;
            update_max(&mut maximum, difference[0] / self.model_scale)?;
            update_max(&mut maximum, difference[1] / self.model_scale)?;
            update_max(&mut maximum, difference[2])?;
        }
        for (_, joint) in self.joints.iter() {
            match joint.kind() {
                JointKind::Revolute { first, second } => {
                    let displacement =
                        geometry_point(geometry, second)? - geometry_point(geometry, first)?;
                    update_max(&mut maximum, displacement.x / self.model_scale)?;
                    update_max(&mut maximum, displacement.y / self.model_scale)?;
                }
                JointKind::Prismatic {
                    first_anchor,
                    first_axis,
                    second_anchor,
                    second_axis,
                    axis_branch,
                } => {
                    let first_direction = geometry_axis(geometry, first_axis)?;
                    let second_direction =
                        geometry_axis(geometry, second_axis)? * axis_branch.multiplier();
                    let normal = Vector2::new(-first_direction.y, first_direction.x);
                    let displacement = geometry_point(geometry, second_anchor)?
                        - geometry_point(geometry, first_anchor)?;
                    update_max(&mut maximum, normal.dot(&displacement) / self.model_scale)?;
                    update_max(&mut maximum, cross2(first_direction, second_direction))?;
                }
                JointKind::Weld {
                    first,
                    second,
                    relative_angle,
                } => {
                    let displacement =
                        geometry_point(geometry, second)? - geometry_point(geometry, first)?;
                    update_max(&mut maximum, displacement.x / self.model_scale)?;
                    update_max(&mut maximum, displacement.y / self.model_scale)?;
                    let first_body = self.require_point_feature(first)?.body();
                    let second_body = self.require_point_feature(second)?.body();
                    update_max(
                        &mut maximum,
                        geometry_body(geometry, second_body)?.angle
                            - geometry_body(geometry, first_body)?.angle
                            - relative_angle,
                    )?;
                }
            }
        }
        for (driver_id, driver) in self.drivers.iter() {
            let target = driver_override
                .filter(|(id, _)| *id == driver_id)
                .map_or(driver.target(), |(_, value)| value);
            match driver.kind() {
                DriverKind::Angular { reference, driven } => update_max(
                    &mut maximum,
                    geometry_body(geometry, driven)?.angle
                        - geometry_body(geometry, reference)?.angle
                        - target,
                )?,
                DriverKind::Linear {
                    origin,
                    measured,
                    guide_axis,
                } => {
                    let displacement =
                        geometry_point(geometry, measured)? - geometry_point(geometry, origin)?;
                    update_max(
                        &mut maximum,
                        (geometry_axis(geometry, guide_axis)?.dot(&displacement) - target)
                            / self.model_scale,
                    )?;
                }
            }
        }
        Ok(maximum)
    }

    /// Evaluates one explicit branch monitor without duplicating its geometry math.
    ///
    /// # Errors
    ///
    /// Returns an error for a stale monitor, missing transformed features, or a
    /// non-finite cross-product/projection metric.
    pub fn evaluate_branch_monitor(
        &self,
        monitor_id: BranchMonitorId,
        geometry: &LinkageGeometry,
    ) -> Result<BranchEvaluation, LinkageError> {
        let monitor = *self
            .branch_monitors
            .get(monitor_id)
            .ok_or(LinkageError::UnknownBranchMonitor(monitor_id))?;
        let (kind, expected_sign, signed_metric, normalized_metric) = match monitor {
            BranchMonitor::Orientation {
                line_start,
                line_end,
                observed,
                sign,
            } => {
                let start = geometry_point(geometry, line_start)?;
                let line = geometry_point(geometry, line_end)? - start;
                let observed = geometry_point(geometry, observed)? - start;
                let metric = cross2(line, observed);
                (
                    BranchMonitorKind::Orientation,
                    sign,
                    metric,
                    metric / self.model_scale / self.model_scale,
                )
            }
            BranchMonitor::DirectedDisplacement {
                origin,
                measured,
                axis,
                sign,
            } => {
                let displacement =
                    geometry_point(geometry, measured)? - geometry_point(geometry, origin)?;
                let metric = geometry_axis(geometry, axis)?.dot(&displacement);
                (
                    BranchMonitorKind::DirectedDisplacement,
                    sign,
                    metric,
                    metric / self.model_scale,
                )
            }
        };
        if !signed_metric.is_finite() {
            return Err(LinkageError::NonFiniteValue {
                context: "branch monitor metric",
                value: signed_metric,
            });
        }
        if !normalized_metric.is_finite() {
            return Err(LinkageError::NonFiniteValue {
                context: "normalized branch monitor metric",
                value: normalized_metric,
            });
        }
        Ok(BranchEvaluation {
            monitor_id,
            kind,
            expected_sign,
            signed_metric,
            retained: normalized_metric * expected_sign.multiplier()
                > MINIMUM_NORMALIZED_BRANCH_MARGIN,
        })
    }

    pub(crate) fn first_branch_violation(
        &self,
        geometry: &LinkageGeometry,
    ) -> Result<Option<BranchViolation>, LinkageError> {
        for (joint_id, joint) in self.joints.iter() {
            if let JointKind::Prismatic {
                first_axis,
                second_axis,
                axis_branch,
                ..
            } = joint.kind()
            {
                let first = geometry_axis(geometry, first_axis)?;
                let second = geometry_axis(geometry, second_axis)?;
                let branch_projection = first.dot(&second) * axis_branch.multiplier();
                if !branch_projection.is_finite() {
                    return Err(LinkageError::NonFiniteValue {
                        context: "prismatic axis branch metric",
                        value: branch_projection,
                    });
                }
                if branch_projection <= MINIMUM_NORMALIZED_BRANCH_MARGIN {
                    return Ok(Some(BranchViolation::PrismaticAxis(joint_id)));
                }
            }
        }
        for (monitor_id, _) in self.branch_monitors.iter() {
            if !self.evaluate_branch_monitor(monitor_id, geometry)?.retained {
                return Ok(Some(BranchViolation::Monitor(monitor_id)));
            }
        }
        Ok(None)
    }
}

#[allow(clippy::too_many_lines)]
fn compile_joint(
    linkage: &Linkage,
    problem: &mut Problem,
    body_variables: &[BodyVariableMapping],
    joint_id: JointId,
    joint: &crate::Joint,
) -> Result<LinkageSourceMapping, LinkageError> {
    let label = format!("joint {}: {}", joint.ordinal(), joint.label());
    let source_id = problem.add_source(SourceConstraint::new(&label)?);
    let residual = match joint.kind() {
        JointKind::Revolute { first, second } => {
            let first_feature = linkage.require_point_feature(first)?;
            let second_feature = linkage.require_point_feature(second)?;
            let bindings = point_pair_bindings(first_feature.label(), second_feature.label());
            ResidualBlock::new(
                source_id,
                ResidualCategory::Hard,
                vec![
                    body_variable(body_variables, first_feature.body())?,
                    body_variable(body_variables, second_feature.body())?,
                ],
                2,
                vec![linkage.model_scale, linkage.model_scale],
                vec![
                    audit_row(
                        "(world(second_anchor).x - world(first_anchor).x) / model_scale",
                        bindings.clone(),
                        "model-unit",
                    ),
                    audit_row(
                        "(world(second_anchor).y - world(first_anchor).y) / model_scale",
                        bindings,
                        "model-unit",
                    ),
                ],
                RevoluteResidual {
                    first_local: point_array(first_feature.local_point()),
                    second_local: point_array(second_feature.local_point()),
                },
            )?
        }
        JointKind::Prismatic {
            first_anchor,
            first_axis,
            second_anchor,
            second_axis,
            axis_branch,
        } => {
            let first_point = linkage.require_point_feature(first_anchor)?;
            let second_point = linkage.require_point_feature(second_anchor)?;
            let first_direction = linkage.require_axis_feature(first_axis)?;
            let second_direction = linkage.require_axis_feature(second_axis)?;
            let bindings = vec![
                AuditBinding::new("first anchor", first_point.label()),
                AuditBinding::new("first guide axis", first_direction.label()),
                AuditBinding::new("second anchor", second_point.label()),
                AuditBinding::new("second axis", second_direction.label()),
                AuditBinding::new("axis branch", format!("{axis_branch:?}")),
            ];
            ResidualBlock::new(
                source_id,
                ResidualCategory::Hard,
                vec![
                    body_variable(body_variables, first_point.body())?,
                    body_variable(body_variables, second_point.body())?,
                ],
                2,
                vec![linkage.model_scale, 1.0],
                vec![
                    audit_row(
                        "dot(normal(world(first_axis)), world(second_anchor)-world(first_anchor)) / model_scale",
                        bindings.clone(),
                        "model-unit",
                    ),
                    audit_row(
                        "cross(world(first_axis), branch*world(second_axis)) / 1",
                        bindings,
                        "dimensionless",
                    ),
                ],
                PrismaticResidual {
                    first_anchor: point_array(first_point.local_point()),
                    first_axis: vector_array(first_direction.local_axis()),
                    second_anchor: point_array(second_point.local_point()),
                    second_axis: vector_array(second_direction.local_axis()),
                    branch_multiplier: axis_branch.multiplier(),
                },
            )?
        }
        JointKind::Weld {
            first,
            second,
            relative_angle,
        } => {
            let first_feature = linkage.require_point_feature(first)?;
            let second_feature = linkage.require_point_feature(second)?;
            let mut bindings = point_pair_bindings(first_feature.label(), second_feature.label());
            bindings.push(AuditBinding::new(
                "relative angle",
                relative_angle.to_string(),
            ));
            ResidualBlock::new(
                source_id,
                ResidualCategory::Hard,
                vec![
                    body_variable(body_variables, first_feature.body())?,
                    body_variable(body_variables, second_feature.body())?,
                ],
                3,
                vec![linkage.model_scale, linkage.model_scale, 1.0],
                vec![
                    audit_row(
                        "(world(second_anchor).x - world(first_anchor).x) / model_scale",
                        bindings.clone(),
                        "model-unit",
                    ),
                    audit_row(
                        "(world(second_anchor).y - world(first_anchor).y) / model_scale",
                        bindings.clone(),
                        "model-unit",
                    ),
                    audit_row(
                        "(second.angle - first.angle - relative_angle) / 1 rad",
                        bindings,
                        "rad",
                    ),
                ],
                WeldResidual {
                    first_local: point_array(first_feature.local_point()),
                    second_local: point_array(second_feature.local_point()),
                    relative_angle,
                },
            )?
        }
    };
    let residual_id = problem.add_residual(residual)?;
    Ok(source_mapping(
        LinkageSource::Joint(joint_id),
        label,
        source_id,
        residual_id,
    ))
}

fn compile_driver(
    linkage: &Linkage,
    problem: &mut Problem,
    body_variables: &[BodyVariableMapping],
    driver_id: DriverId,
    driver: &crate::Driver,
    target: f64,
) -> Result<LinkageSourceMapping, LinkageError> {
    validate_finite(target, "driver target")?;
    let label = format!(
        "driver {}: {} = {} {}",
        driver.ordinal(),
        driver.label(),
        target,
        driver.unit().symbol()
    );
    let source_id = problem.add_source(SourceConstraint::new(&label)?);
    let residual = match driver.kind() {
        DriverKind::Angular { reference, driven } => {
            let reference_body = linkage.require_body(reference)?;
            let driven_body = linkage.require_body(driven)?;
            ResidualBlock::new(
                source_id,
                ResidualCategory::Hard,
                vec![
                    body_variable(body_variables, reference)?,
                    body_variable(body_variables, driven)?,
                ],
                1,
                vec![1.0],
                vec![audit_row(
                    "(driven.angle - reference.angle - target) / 1 rad",
                    vec![
                        AuditBinding::new("reference body", reference_body.label()),
                        AuditBinding::new("driven body", driven_body.label()),
                        AuditBinding::new("target", target.to_string()),
                        AuditBinding::new("unit", driver.unit().symbol()),
                        AuditBinding::new(
                            "max continuation step",
                            driver.max_continuation_step().to_string(),
                        ),
                    ],
                    "rad",
                )],
                AngularDriverResidual { target },
            )?
        }
        DriverKind::Linear {
            origin,
            measured,
            guide_axis,
        } => {
            let origin_feature = linkage.require_point_feature(origin)?;
            let measured_feature = linkage.require_point_feature(measured)?;
            let guide_feature = linkage.require_axis_feature(guide_axis)?;
            ResidualBlock::new(
                source_id,
                ResidualCategory::Hard,
                vec![
                    body_variable(body_variables, origin_feature.body())?,
                    body_variable(body_variables, measured_feature.body())?,
                ],
                1,
                vec![linkage.model_scale],
                vec![audit_row(
                    "(dot(world(guide_axis), world(measured)-world(origin)) - target) / model_scale",
                    vec![
                        AuditBinding::new("origin", origin_feature.label()),
                        AuditBinding::new("measured", measured_feature.label()),
                        AuditBinding::new("guide axis", guide_feature.label()),
                        AuditBinding::new("target", target.to_string()),
                        AuditBinding::new("unit", driver.unit().symbol()),
                        AuditBinding::new(
                            "max continuation step",
                            driver.max_continuation_step().to_string(),
                        ),
                    ],
                    "model-unit",
                )],
                LinearDriverResidual {
                    origin_local: point_array(origin_feature.local_point()),
                    measured_local: point_array(measured_feature.local_point()),
                    guide_axis: vector_array(guide_feature.local_axis()),
                    target,
                },
            )?
        }
    };
    let residual_id = problem.add_residual(residual)?;
    Ok(source_mapping(
        LinkageSource::Driver(driver_id),
        label,
        source_id,
        residual_id,
    ))
}

fn geometry_from_parts(
    plane_frame: PlaneFrame,
    bodies: Vec<SolvedBody>,
    point_features: &[PointFeatureSpec],
    axis_features: &[AxisFeatureSpec],
) -> Result<LinkageGeometry, LinkageError> {
    let mut points = Vec::with_capacity(point_features.len());
    for feature in point_features {
        let pose = bodies
            .iter()
            .find_map(|body| (body.body_id == feature.body).then_some(body.pose))
            .ok_or(LinkageError::UnknownBody(feature.body))?;
        let planar = pose.transform_point(feature.local);
        validate_point(planar, "transformed point feature")?;
        let world =
            plane_frame
                .try_map_point(planar)
                .map_err(|_| LinkageError::NonFinitePoint {
                    context: "world point feature",
                })?;
        points.push(TransformedPointFeature {
            feature_id: feature.id,
            body_id: feature.body,
            planar,
            world,
        });
    }
    let mut axes = Vec::with_capacity(axis_features.len());
    for feature in axis_features {
        let pose = bodies
            .iter()
            .find_map(|body| (body.body_id == feature.body).then_some(body.pose))
            .ok_or(LinkageError::UnknownBody(feature.body))?;
        let planar = pose.transform_vector(feature.local);
        if !planar.iter().all(|value| value.is_finite()) {
            return Err(LinkageError::InvalidAxis {
                context: "transformed axis feature",
            });
        }
        let world = plane_frame
            .try_map_vector(planar)
            .map_err(|_| LinkageError::InvalidAxis {
                context: "world axis feature",
            })?;
        axes.push(TransformedAxisFeature {
            feature_id: feature.id,
            body_id: feature.body,
            planar,
            world,
        });
    }
    Ok(LinkageGeometry {
        plane_frame,
        bodies,
        points,
        axes,
    })
}

fn source_mapping(
    source: LinkageSource,
    source_label: String,
    core_source_id: SourceConstraintId,
    residual_id: ResidualId,
) -> LinkageSourceMapping {
    LinkageSourceMapping {
        source,
        source_label,
        core_source_id,
        residual_ids: vec![residual_id],
    }
}

fn body_variable(
    mappings: &[BodyVariableMapping],
    body: BodyId,
) -> Result<VariableId, LinkageError> {
    mappings
        .iter()
        .find_map(|mapping| (mapping.body_id == body).then_some(mapping.variable_id))
        .ok_or(LinkageError::UnknownBody(body))
}

fn point_pair_bindings(first: &str, second: &str) -> Vec<AuditBinding> {
    vec![
        AuditBinding::new("first anchor", first),
        AuditBinding::new("second anchor", second),
    ]
}

fn audit_row(
    template: impl Into<String>,
    bindings: Vec<AuditBinding>,
    unit: impl Into<String>,
) -> ResidualRowAudit {
    ResidualRowAudit::new(template, bindings, unit)
}

fn point_array(point: Point2<f64>) -> [f64; 2] {
    [point.x, point.y]
}

fn vector_array(vector: Vector2<f64>) -> [f64; 2] {
    [vector.x, vector.y]
}

fn fresh_hard_audit_max(problem: &Problem) -> Result<f64, LinkageError> {
    let audit = problem.audit_snapshot()?;
    let mut maximum = 0.0_f64;
    for row in audit
        .sources
        .iter()
        .flat_map(|source| &source.rows)
        .filter(|row| row.category == ResidualCategory::Hard)
    {
        if row.evaluation_status != AuditEvaluationStatus::Evaluated
            || !row.normalized_residual.is_finite()
        {
            return Err(LinkageError::NonFiniteValue {
                context: "linkage independent hard audit",
                value: row.normalized_residual,
            });
        }
        maximum = maximum.max(row.normalized_residual.abs());
    }
    Ok(maximum)
}

fn rejection_hard_validity(rejection: &SolveRejection) -> HardValidity {
    if matches!(rejection, SolveRejection::IndependentValidationFailed(_)) {
        HardValidity::NotEvaluated
    } else {
        HardValidity::Invalid
    }
}

fn domain_hard_validity(
    core_hard_validity: HardValidity,
    rejection: Option<&SolveRejection>,
) -> HardValidity {
    if core_hard_validity == HardValidity::Valid {
        rejection.map_or(HardValidity::Valid, rejection_hard_validity)
    } else {
        core_hard_validity
    }
}

fn geometry_point(
    geometry: &LinkageGeometry,
    feature: PointFeatureId,
) -> Result<Point2<f64>, LinkageError> {
    geometry
        .point(feature)
        .ok_or(LinkageError::UnknownPointFeature(feature))
}

fn geometry_axis(
    geometry: &LinkageGeometry,
    feature: AxisFeatureId,
) -> Result<Vector2<f64>, LinkageError> {
    geometry
        .axis(feature)
        .ok_or(LinkageError::UnknownAxisFeature(feature))
}

fn geometry_body(geometry: &LinkageGeometry, body: BodyId) -> Result<Pose2, LinkageError> {
    geometry
        .body_pose(body)
        .ok_or(LinkageError::UnknownBody(body))
}

fn update_max(maximum: &mut f64, value: f64) -> Result<(), LinkageError> {
    if !value.is_finite() {
        return Err(LinkageError::NonFiniteValue {
            context: "domain hard residual",
            value,
        });
    }
    *maximum = maximum.max(value.abs());
    Ok(())
}

pub(crate) fn cross2(first: Vector2<f64>, second: Vector2<f64>) -> f64 {
    first.x * second.y - first.y * second.x
}

fn refresh_retained_audit(
    linkage: &Linkage,
    compiled: &mut CompiledLinkage,
    retained_geometry: &LinkageGeometry,
    config: SolverConfig,
) -> Option<AuditSnapshot> {
    let report = compiled.problem.solve(config).ok()?;
    if report.termination != SolveTermination::Converged
        || report.hard_validity != HardValidity::Valid
        || !report.hard_residuals_validated
        || report.hard_residual_max > config.normalized_residual_tolerance
    {
        return None;
    }
    let solved_geometry = compiled.solved_geometry().ok()?;
    if !same_body_poses(retained_geometry, &solved_geometry) {
        return None;
    }
    let audit_max = fresh_hard_audit_max(&compiled.problem).ok()?;
    let domain_max = linkage
        .domain_hard_residual_max(&solved_geometry, None)
        .ok()?;
    if audit_max.max(domain_max) > config.normalized_residual_tolerance
        || linkage
            .first_branch_violation(&solved_geometry)
            .ok()?
            .is_some()
    {
        return None;
    }
    Some(report.audit)
}

fn same_body_poses(first: &LinkageGeometry, second: &LinkageGeometry) -> bool {
    first.bodies.len() == second.bodies.len()
        && first
            .bodies
            .iter()
            .zip(&second.bodies)
            .all(|(first, second)| {
                first.body_id == second.body_id
                    && first.pose.translation.x.to_bits() == second.pose.translation.x.to_bits()
                    && first.pose.translation.y.to_bits() == second.pose.translation.y.to_bits()
                    && first.pose.angle.to_bits() == second.pose.angle.to_bits()
            })
}

fn linkage_diagnostics(report: &SolveReport) -> LinkageSolveDiagnostics {
    let singular_value_ratio = report
        .component_solves
        .iter()
        .filter(|component| component.rank_is_valid && component.sigma_max > 0.0)
        .filter_map(|component| {
            component
                .singular_values
                .iter()
                .copied()
                .filter(|value| *value > 0.0)
                .min_by(f64::total_cmp)
                .map(|smallest| smallest / component.sigma_max)
                .filter(|ratio| ratio.is_finite())
        })
        .min_by(f64::total_cmp);
    LinkageSolveDiagnostics {
        singular_value_ratio,
        has_rank_warning: report
            .component_solves
            .iter()
            .any(|component| component.is_singular || component.near_singular)
            || singular_value_ratio.is_some_and(|ratio| ratio <= RANK_WARNING_SINGULAR_VALUE_RATIO),
    }
}

#[cfg(test)]
mod tests {
    use geosolve_core::{EvaluationError, LocalJacobian, ResidualEvaluator, SecondaryStatus};

    use super::*;

    #[derive(Clone, Copy, Debug)]
    struct StalledTemporary;

    impl ResidualEvaluator for StalledTemporary {
        fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
            let [VariableValue::Scalar(value)] = variables else {
                return Err(EvaluationError::invalid_geometry("expected one scalar"));
            };
            if *value == 0.0 {
                Ok(vec![-1.0])
            } else {
                Err(EvaluationError::out_of_domain(
                    "temporary trial left its fixed branch",
                ))
            }
        }

        fn jacobian(
            &self,
            _variables: &[VariableValue],
        ) -> Result<Vec<LocalJacobian>, EvaluationError> {
            Ok(vec![LocalJacobian::new(1, 1, vec![1.0])])
        }
    }

    #[derive(Clone, Copy, Debug)]
    struct ScalarCoefficient(f64);

    impl ResidualEvaluator for ScalarCoefficient {
        fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
            let [VariableValue::Scalar(value)] = variables else {
                return Err(EvaluationError::invalid_geometry("expected one scalar"));
            };
            Ok(vec![self.0 * value])
        }

        fn jacobian(
            &self,
            _variables: &[VariableValue],
        ) -> Result<Vec<LocalJacobian>, EvaluationError> {
            Ok(vec![LocalJacobian::new(1, 1, vec![self.0])])
        }
    }

    fn add_scalar_row(
        problem: &mut Problem,
        variable: VariableId,
        label: &str,
        evaluator: impl ResidualEvaluator + 'static,
        category: ResidualCategory,
    ) {
        let source = problem.add_source(SourceConstraint::new(label).unwrap());
        problem
            .add_residual(
                ResidualBlock::new(
                    source,
                    category,
                    vec![variable],
                    1,
                    vec![1.0],
                    vec![ResidualRowAudit::new(
                        label,
                        vec![AuditBinding::new("x", label)],
                        "1",
                    )],
                    evaluator,
                )
                .unwrap(),
            )
            .unwrap();
    }

    #[test]
    fn valid_linkage_domain_does_not_turn_secondary_stall_into_hard_invalidity() {
        let mut problem = Problem::new();
        let variable = problem.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap());
        add_scalar_row(
            &mut problem,
            variable,
            "stalled temporary",
            StalledTemporary,
            ResidualCategory::Temporary,
        );
        let mut report = problem.solve(SolverConfig::default()).unwrap();
        assert_eq!(report.temporary_status, SecondaryStatus::Stalled);
        assert_eq!(report.hard_validity, HardValidity::Valid);

        let linkage = Linkage::new(1.0, crate::xy_plane_frame()).unwrap();
        let geometry = linkage.geometry().unwrap();
        assert_eq!(
            linkage
                .domain_hard_residual_max(&geometry, None)
                .unwrap()
                .to_bits(),
            0.0_f64.to_bits()
        );
        assert!(linkage.first_branch_violation(&geometry).unwrap().is_none());
        report.hard_validity = domain_hard_validity(report.hard_validity, None);

        assert_eq!(report.hard_validity, HardValidity::Valid);
        assert!(report.hard_residuals_validated);
    }

    #[test]
    fn ground_domain_validation_uses_pose_local_difference() {
        let mut linkage = Linkage::new(2.0, crate::xy_plane_frame()).unwrap();
        let ground = linkage
            .add_body(
                "transformed ground",
                Pose2::try_new(Vector2::new(3.0, -4.0), 0.7).unwrap(),
                true,
            )
            .unwrap();
        let mut equivalent_geometry = linkage.geometry().unwrap();
        equivalent_geometry
            .bodies
            .iter_mut()
            .find(|body| body.body_id == ground)
            .unwrap()
            .pose
            .angle += std::f64::consts::TAU;

        let maximum = linkage
            .domain_hard_residual_max(&equivalent_geometry, None)
            .unwrap();

        assert!(maximum <= 1.0e-12, "manifold ground residual {maximum:e}");
    }

    #[test]
    fn linkage_conditioning_never_compares_extrema_across_components() {
        let mut problem = Problem::new();
        let ordinary = problem.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap());
        let large = problem.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap());
        add_scalar_row(
            &mut problem,
            ordinary,
            "ordinary component",
            ScalarCoefficient(1.0),
            ResidualCategory::Hard,
        );
        add_scalar_row(
            &mut problem,
            large,
            "large component",
            ScalarCoefficient(1.0e12),
            ResidualCategory::Hard,
        );
        let report = problem.solve(SolverConfig::default()).unwrap();

        let diagnostics = linkage_diagnostics(&report);

        assert_eq!(diagnostics.singular_value_ratio, Some(1.0));
        assert!(!diagnostics.has_rank_warning);
        assert!(
            report
                .component_solves
                .iter()
                .all(|component| !component.near_singular && !component.is_singular)
        );
    }
}
