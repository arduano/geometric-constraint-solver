use geosolve_core::{
    AuditBinding, AuditEvaluationStatus, AuditSnapshot, BoundId, CoordinateBound, HardValidity,
    Problem, ResidualBlock, ResidualCategory, ResidualId, ResidualRowAudit, SolveReport,
    SolveTermination, SolverConfig, SourceConstraint, SourceConstraintId, VariableBlock,
    VariableId, VariableValue,
};
use geosolve_geometry::{Point2, Vector2};

use crate::curves::{
    CENTER_DIRECTION_COSINE_MARGIN, CIRCLE_ARC_TANGENCY_DIRECTION_TOLERANCE,
    CIRCLE_ARC_TANGENCY_RADIUS_RELATIVE_TOLERANCE,
    CIRCLE_ARC_TANGENCY_SCALE_UNCERTAINTY_MULTIPLIER, normalize_bounded_candidate, segment_points,
    tangency_distance, unwrap_near, validate_bounded_parameter, validate_radius,
};
use crate::model::{
    ArcId, CircleId, ConicId, CoordinateAxis, CurveContactNeighborhood, DimensionKind,
    DimensionMode, PersistentSource, PointId, SegmentId, Sketch, SketchConstraintId,
    SketchConstraintKind, SketchCurve, SketchCurveContact, SketchDimensionId, SketchError,
    validate_model_scale, validate_point,
};
use crate::residuals::{
    AxisDifferenceResidual, BezierIncidence, CircleArcTangencyResidual, CircleTangencyResidual,
    CoincidentResidual, CurveParameterIncidence, DistanceResidual, FixedCoordinateResidual,
    GenericCurveIncidence, GenericCurvePairResidual, GenericPointOnCurveResidual,
    LineBezierTangencyResidual, LineCircleTangencyResidual, MidpointResidual,
    OrientedAngleResidual, PointOnArcResidual, PointOnBezierResidual, PointOnCircleResidual,
    PointOnLineResidual, PointTargetResidual, ScalarEqualityResidual, ScalarTargetResidual,
    SegmentPairEquation, SegmentPairResidual, SymmetryResidual,
};

/// Temporary point target supplied for one solve only.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DragTarget {
    pub point: PointId,
    pub target: Point2<f64>,
}

/// Per-solve interaction and minimum-motion objectives.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SketchSolveRequest {
    pub drag: Option<DragTarget>,
    pub stability_target: Option<DragTarget>,
    pub previous_state_preferences: bool,
}

impl SketchSolveRequest {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            drag: None,
            stability_target: None,
            previous_state_preferences: true,
        }
    }

    #[must_use]
    pub const fn without_previous_state_preferences(mut self) -> Self {
        self.previous_state_preferences = false;
        self
    }

    #[must_use]
    pub const fn with_drag(mut self, point: PointId, target: Point2<f64>) -> Self {
        self.drag = Some(DragTarget { point, target });
        self
    }

    /// Adds a second compatible temporary target that keeps an unrelated point stable.
    #[must_use]
    pub const fn with_stability_target(mut self, point: PointId, target: Point2<f64>) -> Self {
        self.stability_target = Some(DragTarget { point, target });
        self
    }
}

impl Default for SketchSolveRequest {
    fn default() -> Self {
        Self::new()
    }
}

/// Domain identity corresponding to one deterministic compiled source entry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SketchSource {
    Constraint(SketchConstraintId),
    Dimension(SketchDimensionId),
    DragTarget(PointId),
    PreviousState(PointId),
}

/// Exact relationship between a domain source and zero or one core source.
#[derive(Clone, Debug, PartialEq)]
pub struct SketchSourceMapping {
    pub source: SketchSource,
    pub source_label: String,
    /// `None` only for a reference dimension, which intentionally has no equation.
    pub core_source_id: Option<SourceConstraintId>,
    pub residual_ids: Vec<ResidualId>,
}

/// Exact point-to-core-variable relationship in point insertion order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PointVariableMapping {
    pub point_id: PointId,
    pub variable_id: VariableId,
}

/// Exact circle-radius-to-core-variable relationship in circle insertion order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CircleRadiusVariableMapping {
    pub circle_id: CircleId,
    pub variable_id: VariableId,
}

/// Exact arc-radius-to-core-variable relationship in arc insertion order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArcRadiusVariableMapping {
    pub arc_id: ArcId,
    pub variable_id: VariableId,
}

/// Scalar shape coordinate owned by one runtime conic.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConicScalarRole {
    MinorAxisRatio,
    MiddleWeight,
    SemiConjugate,
}

/// Exact conic-shape-scalar-to-core-variable relationship in conic insertion order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConicScalarVariableMapping {
    pub conic_id: ConicId,
    pub role: ConicScalarRole,
    pub variable_id: VariableId,
}

/// Vector shape coordinate owned by one runtime conic.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConicVectorRole {
    WeightedMiddle,
}

/// Exact conic-shape-vector-to-core-variable relationship in conic insertion order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConicVectorVariableMapping {
    pub conic_id: ConicId,
    pub role: ConicVectorRole,
    pub variable_id: VariableId,
}

/// Semantic role of an ordinary scalar variable retained inside a source constraint.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LatentVariableRole {
    LineParameter,
    CircleAngle,
    ArcSpanParameter,
    BezierParameter,
    CurveParameter,
    FirstCurveParameter,
    SecondCurveParameter,
}

/// Deterministic mapping for one accepted latent source parameter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LatentVariableMapping {
    pub constraint_id: SketchConstraintId,
    pub role: LatentVariableRole,
    pub variable_id: VariableId,
}

/// Domain role of one generated core coordinate bound.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SketchBound {
    CircleRadius(CircleId),
    ArcRadius(ArcId),
    ConicScalar {
        conic_id: ConicId,
        role: ConicScalarRole,
    },
    Contact {
        constraint_id: SketchConstraintId,
        role: LatentVariableRole,
    },
}

/// Stable domain-to-core bound mapping in deterministic compile order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SketchBoundMapping {
    pub bound: SketchBound,
    pub bound_id: BoundId,
}

/// The positive-radius policy preserves every finite `radius > 0` accepted by
/// the baseline model, including subnormal fixtures.
pub const MIN_REPRESENTABLE_RADIUS: f64 = f64::from_bits(1);

/// Inclusive representable core bound implementing the strict geometric `w > -1` domain.
pub const MIN_RATIONAL_QUADRATIC_MIDDLE_WEIGHT: f64 = -1.0 + f64::EPSILON;

/// Mandatory sketch-domain acceptance ceiling for normalized hard residuals.
pub const SKETCH_ACCEPTANCE_RESIDUAL_TOLERANCE: f64 = 1.0e-9;

pub(crate) fn acceptance_solver_config(mut config: SolverConfig) -> SolverConfig {
    if config.normalized_residual_tolerance.is_finite()
        && config.normalized_residual_tolerance > SKETCH_ACCEPTANCE_RESIDUAL_TOLERANCE
    {
        config.normalized_residual_tolerance = SKETCH_ACCEPTANCE_RESIDUAL_TOLERANCE;
    }
    config
}

/// Read-only compilation seam for audit, incidence, and Jacobian verification.
#[derive(Clone, Debug)]
pub struct CompiledSketch {
    problem: Problem,
    point_variables: Vec<PointVariableMapping>,
    circle_radius_variables: Vec<CircleRadiusVariableMapping>,
    arc_radius_variables: Vec<ArcRadiusVariableMapping>,
    conic_vector_variables: Vec<ConicVectorVariableMapping>,
    conic_scalar_variables: Vec<ConicScalarVariableMapping>,
    latent_variables: Vec<LatentVariableMapping>,
    bound_mappings: Vec<SketchBoundMapping>,
    source_mappings: Vec<SketchSourceMapping>,
}

#[derive(Clone, Debug)]
pub(crate) struct CompiledSourcePatch {
    pub(crate) source_id: SourceConstraintId,
    pub(crate) source: SourceConstraint,
    pub(crate) residuals: Vec<(ResidualId, ResidualBlock)>,
    pub(crate) variable_values: Vec<(VariableId, VariableValue)>,
    pub(crate) bounds: Vec<(BoundId, CoordinateBound)>,
}

impl CompiledSketch {
    #[must_use]
    pub const fn problem(&self) -> &Problem {
        &self.problem
    }

    #[must_use]
    pub fn point_variables(&self) -> &[PointVariableMapping] {
        &self.point_variables
    }

    #[must_use]
    pub fn circle_radius_variables(&self) -> &[CircleRadiusVariableMapping] {
        &self.circle_radius_variables
    }

    #[must_use]
    pub fn arc_radius_variables(&self) -> &[ArcRadiusVariableMapping] {
        &self.arc_radius_variables
    }

    #[must_use]
    pub fn conic_scalar_variables(&self) -> &[ConicScalarVariableMapping] {
        &self.conic_scalar_variables
    }

    #[must_use]
    pub fn conic_vector_variables(&self) -> &[ConicVectorVariableMapping] {
        &self.conic_vector_variables
    }

    #[must_use]
    pub fn latent_variables(&self) -> &[LatentVariableMapping] {
        &self.latent_variables
    }

    #[must_use]
    pub fn bound_mappings(&self) -> &[SketchBoundMapping] {
        &self.bound_mappings
    }

    #[must_use]
    pub fn source_mappings(&self) -> &[SketchSourceMapping] {
        &self.source_mappings
    }

    #[must_use]
    pub fn variable_for_point(&self, point: PointId) -> Option<VariableId> {
        self.point_variables
            .iter()
            .find_map(|mapping| (mapping.point_id == point).then_some(mapping.variable_id))
    }

    #[must_use]
    pub fn variable_for_circle_radius(&self, circle: CircleId) -> Option<VariableId> {
        self.circle_radius_variables
            .iter()
            .find_map(|mapping| (mapping.circle_id == circle).then_some(mapping.variable_id))
    }

    #[must_use]
    pub fn variable_for_arc_radius(&self, arc: ArcId) -> Option<VariableId> {
        self.arc_radius_variables
            .iter()
            .find_map(|mapping| (mapping.arc_id == arc).then_some(mapping.variable_id))
    }

    #[must_use]
    pub fn variable_for_conic_scalar(
        &self,
        conic: ConicId,
        role: ConicScalarRole,
    ) -> Option<VariableId> {
        self.conic_scalar_variables.iter().find_map(|mapping| {
            (mapping.conic_id == conic && mapping.role == role).then_some(mapping.variable_id)
        })
    }

    #[must_use]
    pub fn variable_for_conic_vector(
        &self,
        conic: ConicId,
        role: ConicVectorRole,
    ) -> Option<VariableId> {
        self.conic_vector_variables.iter().find_map(|mapping| {
            (mapping.conic_id == conic && mapping.role == role).then_some(mapping.variable_id)
        })
    }

    fn solved_state(&self, sketch: &Sketch) -> Result<SolvedSketchState, SketchError> {
        self.solved_state_for_problem(&self.problem, sketch)
    }

    #[allow(clippy::too_many_lines)]
    pub(crate) fn solved_state_for_problem(
        &self,
        problem: &Problem,
        sketch: &Sketch,
    ) -> Result<SolvedSketchState, SketchError> {
        let mut points = Vec::with_capacity(self.point_variables.len());
        for mapping in &self.point_variables {
            let variable = problem.variable(mapping.variable_id).ok_or(
                geosolve_core::CoreError::UnknownVariable(mapping.variable_id),
            )?;
            let VariableValue::Vec2([x, y]) = variable.value() else {
                return Err(geosolve_core::CoreError::VariableKindMismatch {
                    expected: geosolve_core::VariableKind::Vec2,
                    actual: variable.kind(),
                }
                .into());
            };
            let position = Point2::new(x, y);
            validate_point(position, "solved point")?;
            points.push(SolvedPoint {
                point_id: mapping.point_id,
                position,
            });
        }
        let mut circles = Vec::with_capacity(self.circle_radius_variables.len());
        for mapping in &self.circle_radius_variables {
            let circle = sketch.circle_value(mapping.circle_id)?;
            let radius = scalar_variable(problem, mapping.variable_id)?;
            circles.push(SolvedCircle {
                circle_id: mapping.circle_id,
                center: solved_point(&points, circle.center())?,
                radius,
            });
        }
        let mut arcs = Vec::with_capacity(self.arc_radius_variables.len());
        for mapping in &self.arc_radius_variables {
            let arc = sketch.arc_value(mapping.arc_id)?;
            let radius = scalar_variable(problem, mapping.variable_id)?;
            arcs.push(SolvedArc {
                arc_id: mapping.arc_id,
                center: solved_point(&points, arc.center())?,
                radius,
                start_angle: arc.start_angle(),
                end_angle: arc.end_angle(),
                signed_sweep: arc.signed_sweep(),
                sweep: arc.sweep(),
            });
        }
        let mut conics = Vec::with_capacity(sketch.conics.iter().count());
        for (conic_id, conic) in sketch.conics.iter() {
            let kind = match conic.kind() {
                crate::ConicKind::Ellipse {
                    center,
                    major_axis_point,
                    ..
                } => SolvedConicKind::Ellipse {
                    center: solved_point(&points, center)?,
                    major_axis_point: solved_point(&points, major_axis_point)?,
                    minor_axis_ratio: conic_scalar_value(
                        problem,
                        &self.conic_scalar_variables,
                        conic_id,
                        ConicScalarRole::MinorAxisRatio,
                    )?,
                },
                crate::ConicKind::EllipticalArc {
                    center,
                    major_axis_point,
                    start_angle,
                    signed_sweep,
                    ..
                } => SolvedConicKind::EllipticalArc {
                    center: solved_point(&points, center)?,
                    major_axis_point: solved_point(&points, major_axis_point)?,
                    minor_axis_ratio: conic_scalar_value(
                        problem,
                        &self.conic_scalar_variables,
                        conic_id,
                        ConicScalarRole::MinorAxisRatio,
                    )?,
                    start_angle,
                    signed_sweep,
                },
                crate::ConicKind::RationalQuadratic { start, end, .. } => {
                    SolvedConicKind::RationalQuadratic {
                        start: solved_point(&points, start)?,
                        weighted_middle: conic_vector_value(
                            problem,
                            &self.conic_vector_variables,
                            conic_id,
                            ConicVectorRole::WeightedMiddle,
                        )?,
                        middle_weight: conic_scalar_value(
                            problem,
                            &self.conic_scalar_variables,
                            conic_id,
                            ConicScalarRole::MiddleWeight,
                        )?,
                        end: solved_point(&points, end)?,
                    }
                }
                crate::ConicKind::ParabolaSegment {
                    vertex,
                    focus,
                    trim,
                } => SolvedConicKind::ParabolaSegment {
                    vertex: solved_point(&points, vertex)?,
                    focus: solved_point(&points, focus)?,
                    trim,
                },
                crate::ConicKind::HyperbolaSegment {
                    center,
                    transverse_axis_point,
                    branch,
                    trim,
                    ..
                } => SolvedConicKind::HyperbolaSegment {
                    center: solved_point(&points, center)?,
                    transverse_axis_point: solved_point(&points, transverse_axis_point)?,
                    semi_conjugate: conic_scalar_value(
                        problem,
                        &self.conic_scalar_variables,
                        conic_id,
                        ConicScalarRole::SemiConjugate,
                    )?,
                    branch,
                    trim,
                },
            };
            conics.push(SolvedConic { conic_id, kind });
        }
        let mut latents = Vec::with_capacity(self.latent_variables.len());
        for mapping in &self.latent_variables {
            latents.push(SolvedLatent {
                constraint_id: mapping.constraint_id,
                role: mapping.role,
                value: scalar_variable(problem, mapping.variable_id)?,
            });
        }
        Ok(SolvedSketchState {
            geometry: SketchGeometry {
                points,
                circles,
                arcs,
                conics,
            },
            latents,
        })
    }

    pub(crate) fn replace_problem(&mut self, problem: Problem) {
        self.problem = problem;
    }

    pub(crate) fn replace_source_label(
        &mut self,
        source: SketchSource,
        label: String,
    ) -> Result<(), SketchError> {
        let mapping = self
            .source_mappings
            .iter_mut()
            .find(|mapping| mapping.source == source)
            .ok_or(geosolve_core::CoreError::InvalidSolverConfig {
                field: "sketch source label",
                message: "source is not present in retained compilation",
            })?;
        mapping.source_label = label;
        Ok(())
    }

    /// Rebuilds one source payload against retained runtime mappings.
    #[allow(clippy::too_many_lines)]
    pub(crate) fn source_patch(
        &self,
        sketch: &Sketch,
        request: SketchSolveRequest,
        source: SketchSource,
    ) -> Result<Option<CompiledSourcePatch>, SketchError> {
        let retained = self
            .source_mappings
            .iter()
            .find(|mapping| mapping.source == source)
            .ok_or(geosolve_core::CoreError::InvalidSolverConfig {
                field: "sketch source patch",
                message: "source is not present in retained compilation",
            })?;
        let Some(retained_source_id) = retained.core_source_id else {
            return Ok(None);
        };

        let mut scratch = self.problem.clone();
        let mut generated_latents = Vec::new();
        let mut generated_bounds = Vec::new();
        let generated = match source {
            SketchSource::Constraint(constraint_id) => {
                let constraint = sketch
                    .constraints
                    .get(constraint_id)
                    .ok_or(SketchError::UnknownConstraint(constraint_id))?;
                compile_constraint(
                    sketch,
                    &mut scratch,
                    &self.point_variables,
                    &self.circle_radius_variables,
                    &self.arc_radius_variables,
                    &self.conic_vector_variables,
                    &self.conic_scalar_variables,
                    &mut generated_latents,
                    &mut generated_bounds,
                    constraint_id,
                    constraint,
                )?
            }
            SketchSource::Dimension(dimension_id) => {
                let dimension = sketch
                    .dimensions
                    .get(dimension_id)
                    .ok_or(SketchError::UnknownDimension(dimension_id))?;
                compile_dimension(
                    sketch,
                    &mut scratch,
                    &self.point_variables,
                    &self.circle_radius_variables,
                    &self.arc_radius_variables,
                    dimension_id,
                    dimension,
                )?
            }
            SketchSource::DragTarget(point) => {
                let drag = request.drag.filter(|drag| drag.point == point).ok_or(
                    geosolve_core::CoreError::InvalidSolverConfig {
                        field: "sketch drag source patch",
                        message: "retained drag source has no matching request",
                    },
                )?;
                compile_point_target(
                    sketch,
                    &mut scratch,
                    &self.point_variables,
                    source,
                    point,
                    drag.target,
                    ResidualCategory::Temporary,
                    format!("temporary drag target for {}", sketch.point_name(point)?),
                )?
            }
            SketchSource::PreviousState(point) => {
                let target = sketch.point_position(point)?;
                compile_point_target(
                    sketch,
                    &mut scratch,
                    &self.point_variables,
                    source,
                    point,
                    target,
                    ResidualCategory::Preference,
                    format!(
                        "previous-state preference for {}",
                        sketch
                            .point(point)
                            .ok_or(SketchError::UnknownPoint(point))?
                            .label()
                    ),
                )?
            }
        };
        let generated_source_id =
            generated
                .core_source_id
                .ok_or(geosolve_core::CoreError::InvalidSolverConfig {
                    field: "sketch source patch",
                    message: "equation source unexpectedly compiled without a core source",
                })?;
        if generated.residual_ids.len() != retained.residual_ids.len() {
            return Err(geosolve_core::CoreError::DimensionMismatch {
                context: "source-local residual replacement",
                expected: retained.residual_ids.len(),
                actual: generated.residual_ids.len(),
            }
            .into());
        }

        let mut variable_remaps = Vec::new();
        let mut variable_values = Vec::new();
        for generated_latent in generated_latents {
            let retained_variable = self
                .latent_variables
                .iter()
                .find(|mapping| {
                    mapping.constraint_id == generated_latent.constraint_id
                        && mapping.role == generated_latent.role
                })
                .ok_or(geosolve_core::CoreError::InvalidSolverConfig {
                    field: "source-local latent mapping",
                    message: "generated latent has no retained runtime identity",
                })?
                .variable_id;
            let value = scratch
                .variable(generated_latent.variable_id)
                .ok_or(geosolve_core::CoreError::UnknownVariable(
                    generated_latent.variable_id,
                ))?
                .value();
            variable_remaps.push((generated_latent.variable_id, retained_variable));
            variable_values.push((retained_variable, value));
        }

        let mut residuals = Vec::with_capacity(retained.residual_ids.len());
        for (&retained_residual_id, &generated_residual_id) in
            retained.residual_ids.iter().zip(&generated.residual_ids)
        {
            let residual = scratch
                .residual(generated_residual_id)
                .ok_or(geosolve_core::CoreError::UnknownResidual(
                    generated_residual_id,
                ))?
                .clone()
                .remap_for_compatible_replacement(retained_source_id, &variable_remaps)?;
            residuals.push((retained_residual_id, residual));
        }
        let retained_bounds = self
            .bound_mappings
            .iter()
            .filter(|mapping| {
                matches!(
                    mapping.bound,
                    SketchBound::Contact {
                        constraint_id: id,
                        ..
                    } if id == match source {
                        SketchSource::Constraint(id) => id,
                        _ => return false,
                    }
                )
            })
            .collect::<Vec<_>>();
        if retained_bounds.len() != generated_bounds.len() {
            return Err(geosolve_core::CoreError::DimensionMismatch {
                context: "source-local bound replacement",
                expected: retained_bounds.len(),
                actual: generated_bounds.len(),
            }
            .into());
        }
        let mut bounds = Vec::with_capacity(generated_bounds.len());
        for generated_mapping in generated_bounds {
            let retained_mapping = retained_bounds
                .iter()
                .find(|mapping| mapping.bound == generated_mapping.bound)
                .ok_or(geosolve_core::CoreError::InvalidSolverConfig {
                    field: "source-local bound mapping",
                    message: "generated bound has no retained semantic identity",
                })?;
            let generated_bound = scratch.bound(generated_mapping.bound_id).ok_or(
                geosolve_core::CoreError::UnknownBound(generated_mapping.bound_id),
            )?;
            let retained_variable = variable_remaps
                .iter()
                .find_map(|(generated, retained)| {
                    (*generated == generated_bound.variable_id()).then_some(*retained)
                })
                .ok_or(geosolve_core::CoreError::InvalidSolverConfig {
                    field: "source-local bound variable",
                    message: "generated bound variable has no retained mapping",
                })?;
            let replacement = CoordinateBound::new(
                retained_variable,
                generated_bound.coordinate(),
                generated_bound.lower(),
                generated_bound.upper(),
                generated_bound.label(),
            )?;
            if self.problem.bound(retained_mapping.bound_id) != Some(&replacement) {
                bounds.push((retained_mapping.bound_id, replacement));
            }
        }
        let source = scratch
            .source(generated_source_id)
            .ok_or(geosolve_core::CoreError::UnknownSource(generated_source_id))?
            .clone();
        Ok(Some(CompiledSourcePatch {
            source_id: retained_source_id,
            source,
            residuals,
            variable_values,
            bounds,
        }))
    }
}

/// One solved point in deterministic insertion order.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SolvedPoint {
    pub point_id: PointId,
    pub position: Point2<f64>,
}

/// One solved circle in deterministic insertion order.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SolvedCircle {
    pub circle_id: CircleId,
    pub center: Point2<f64>,
    pub radius: f64,
}

impl SolvedCircle {
    #[must_use]
    pub fn evaluate(self, angle: f64) -> Option<Point2<f64>> {
        if !angle.is_finite() || !self.radius.is_finite() || self.radius <= 0.0 {
            return None;
        }
        Some(Point2::new(
            self.center.x + self.radius * angle.cos(),
            self.center.y + self.radius * angle.sin(),
        ))
    }
}

/// One solved circular arc with fixed M7 span state.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SolvedArc {
    pub arc_id: ArcId,
    pub center: Point2<f64>,
    pub radius: f64,
    pub start_angle: f64,
    pub end_angle: f64,
    pub signed_sweep: f64,
    pub sweep: crate::ArcSweep,
}

impl SolvedArc {
    #[must_use]
    pub fn evaluate(self, span_parameter: f64) -> Option<Point2<f64>> {
        if !(0.0..=1.0).contains(&span_parameter) || !self.radius.is_finite() || self.radius <= 0.0
        {
            return None;
        }
        let angle = self.start_angle + self.signed_sweep * span_parameter;
        Some(Point2::new(
            self.center.x + self.radius * angle.cos(),
            self.center.y + self.radius * angle.sin(),
        ))
    }

    #[must_use]
    pub fn endpoints(self) -> Option<(Point2<f64>, Point2<f64>)> {
        Some((self.evaluate(0.0)?, self.evaluate(1.0)?))
    }
}

/// Solved point/scalar data for one runtime conic family.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SolvedConicKind {
    Ellipse {
        center: Point2<f64>,
        major_axis_point: Point2<f64>,
        minor_axis_ratio: f64,
    },
    EllipticalArc {
        center: Point2<f64>,
        major_axis_point: Point2<f64>,
        minor_axis_ratio: f64,
        start_angle: f64,
        signed_sweep: f64,
    },
    RationalQuadratic {
        start: Point2<f64>,
        weighted_middle: Vector2<f64>,
        middle_weight: f64,
        end: Point2<f64>,
    },
    ParabolaSegment {
        vertex: Point2<f64>,
        focus: Point2<f64>,
        trim: geosolve_geometry::DirectedParameterTrim,
    },
    HyperbolaSegment {
        center: Point2<f64>,
        transverse_axis_point: Point2<f64>,
        semi_conjugate: f64,
        branch: geosolve_geometry::HyperbolaBranch,
        trim: geosolve_geometry::DirectedParameterTrim,
    },
}

/// One independently reconstructable solved conic in deterministic insertion order.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SolvedConic {
    pub conic_id: ConicId,
    pub kind: SolvedConicKind,
}

#[allow(clippy::missing_errors_doc)]
impl SolvedConic {
    #[must_use]
    pub const fn kind(self) -> SolvedConicKind {
        self.kind
    }

    /// Reconstructs a validated immutable conic from solved controls and shape scalars.
    pub fn geometry(self) -> Result<crate::ConicGeometry, SketchError> {
        solved_conic_geometry(self.kind)
    }

    /// Evaluates a solved conic through the immutable geometry jet API.
    pub fn evaluate(self, parameter: f64) -> Result<geosolve_geometry::CurveJet2, SketchError> {
        self.geometry()?
            .evaluate(parameter)
            .map_err(SketchError::InvalidConicEvaluation)
    }

    pub fn endpoints(self) -> Result<Option<[Point2<f64>; 2]>, SketchError> {
        self.geometry()?
            .endpoints()
            .map_err(SketchError::InvalidConicEvaluation)
    }

    pub fn foci(self) -> Result<Option<[Point2<f64>; 2]>, SketchError> {
        Ok(self.geometry()?.foci())
    }

    pub fn focus(self) -> Result<Option<Point2<f64>>, SketchError> {
        Ok(self.geometry()?.focus())
    }

    pub fn axis_observability(
        self,
    ) -> Result<Option<geosolve_geometry::EllipseAxisObservability>, SketchError> {
        Ok(self.geometry()?.axis_observability())
    }

    pub fn major_axis_endpoints(self) -> Result<Option<[Point2<f64>; 2]>, SketchError> {
        Ok(self.geometry()?.major_axis_endpoints())
    }

    pub fn minor_axis_endpoints(self) -> Result<Option<[Point2<f64>; 2]>, SketchError> {
        Ok(self.geometry()?.minor_axis_endpoints())
    }

    pub fn major_axis_length(self) -> Result<Option<f64>, SketchError> {
        Ok(self.geometry()?.major_axis_length())
    }

    pub fn minor_axis_length(self) -> Result<Option<f64>, SketchError> {
        Ok(self.geometry()?.minor_axis_length())
    }

    pub fn linear_eccentricity(self) -> Result<Option<f64>, SketchError> {
        Ok(self.geometry()?.linear_eccentricity())
    }

    pub fn proper_conic_kind(
        self,
    ) -> Result<Option<geosolve_geometry::ProperConicKind>, SketchError> {
        Ok(self.geometry()?.proper_conic_kind())
    }

    pub fn selected_branch_focus(self) -> Result<Option<Point2<f64>>, SketchError> {
        Ok(self.geometry()?.selected_branch_focus())
    }

    pub fn selected_branch_vertex(self) -> Result<Option<Point2<f64>>, SketchError> {
        Ok(self.geometry()?.selected_branch_vertex())
    }

    pub fn focal_distance(self) -> Result<Option<f64>, SketchError> {
        Ok(self.geometry()?.focal_distance())
    }

    pub fn transverse_axis_length(self) -> Result<Option<f64>, SketchError> {
        Ok(self.geometry()?.transverse_axis_length())
    }

    pub fn conjugate_axis_length(self) -> Result<Option<f64>, SketchError> {
        Ok(self.geometry()?.conjugate_axis_length())
    }
}

/// Finite geometry returned for display or downstream queries.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SketchGeometry {
    pub points: Vec<SolvedPoint>,
    pub circles: Vec<SolvedCircle>,
    pub arcs: Vec<SolvedArc>,
    pub conics: Vec<SolvedConic>,
}

impl SketchGeometry {
    #[must_use]
    pub fn point(&self, point: PointId) -> Option<Point2<f64>> {
        self.points
            .iter()
            .find_map(|item| (item.point_id == point).then_some(item.position))
    }

    #[must_use]
    pub fn circle(&self, circle: CircleId) -> Option<&SolvedCircle> {
        self.circles.iter().find(|item| item.circle_id == circle)
    }

    #[must_use]
    pub fn arc(&self, arc: ArcId) -> Option<&SolvedArc> {
        self.arcs.iter().find(|item| item.arc_id == arc)
    }

    #[must_use]
    pub fn conic(&self, conic: ConicId) -> Option<&SolvedConic> {
        self.conics.iter().find(|item| item.conic_id == conic)
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct SolvedLatent {
    pub(crate) constraint_id: SketchConstraintId,
    pub(crate) role: LatentVariableRole,
    pub(crate) value: f64,
}

#[derive(Clone, Debug)]
pub(crate) struct SolvedSketchState {
    pub(crate) geometry: SketchGeometry,
    pub(crate) latents: Vec<SolvedLatent>,
}

/// Value of one equation-free reference dimension after the solve attempt.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ReferenceDimensionValue {
    pub dimension_id: SketchDimensionId,
    pub value: f64,
}

/// Why a core-returned state was not committed to the sketch.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum SolveRejection {
    CoreTermination(SolveTermination),
    HardResidual {
        maximum: f64,
        tolerance: f64,
    },
    IndependentValidationFailed(String),
    SegmentBranchFlipped(SegmentId),
    NonPositiveCircleRadius(CircleId),
    NonPositiveArcRadius(ArcId),
    DegenerateSegment(SegmentId),
    DegenerateCurve(SketchConstraintId),
    InvalidConicEntity(ConicId),
    ContactParameterOutOfDomain(SketchConstraintId),
    AmbiguousContactNeighborhood(SketchConstraintId),
    LineSideFlipped(SketchConstraintId),
    /// The explicit tangency branch is invalid, including a branch-derived radius mismatch.
    InvalidTangencyMode(SketchConstraintId),
    /// Circle and supporting-arc scales do not resolve the selected tangency gap reliably.
    AmbiguousTangencyScale(SketchConstraintId),
    /// The selected center/contact direction root was not retained.
    CenterDirectionFlipped(SketchConstraintId),
    /// Core accepted-state bound validation failed for this stable bound.
    BoundViolation(BoundId),
}

/// Domain solve outcome. `geometry` is always the geometry retained by the sketch.
#[derive(Clone, Debug, PartialEq)]
pub struct SketchSolveResult {
    pub geometry: SketchGeometry,
    /// Audit evaluated at exactly `geometry`, suitable for display.
    pub display_audit: AuditSnapshot,
    pub reference_values: Vec<ReferenceDimensionValue>,
    pub source_mappings: Vec<SketchSourceMapping>,
    /// Stable domain identities for every bound in `core_report.bounds`.
    pub bound_mappings: Vec<SketchBoundMapping>,
    /// Attempt report whose `hard_validity` includes sketch equation/domain/branch validation.
    /// Its audit remains the candidate-state audit on rejection.
    pub core_report: SolveReport,
    pub rejection: Option<SolveRejection>,
    pub acceptance_hard_residual_max: Option<f64>,
}

impl SketchSolveResult {
    #[must_use]
    pub const fn accepted(&self) -> bool {
        self.rejection.is_none()
    }
}

impl Sketch {
    /// Compiles the current accepted geometry and one transient solve request.
    ///
    /// # Errors
    ///
    /// Returns an error for stale IDs, non-finite geometry, invalid scale, or
    /// a rejected core declaration.
    #[allow(clippy::too_many_lines)]
    pub fn compile(&self, request: SketchSolveRequest) -> Result<CompiledSketch, SketchError> {
        validate_model_scale(self.model_scale)?;
        validate_request(self, request)?;
        self.preflight_segments()?;
        self.preflight_conics()?;

        let mut problem = Problem::new();
        let mut point_variables = Vec::new();
        for (point_id, point) in self.points.iter() {
            validate_point(point.position(), "point position")?;
            let variable_id = problem.add_variable(VariableBlock::vec2(
                [point.position().x, point.position().y],
                [self.model_scale, self.model_scale],
            )?);
            point_variables.push(PointVariableMapping {
                point_id,
                variable_id,
            });
        }

        let mut circle_radius_variables = Vec::new();
        let mut bound_mappings = Vec::new();
        for (circle_id, circle) in self.circles.iter() {
            validate_radius(circle.radius())?;
            let variable_id =
                problem.add_variable(VariableBlock::scalar(circle.radius(), self.model_scale)?);
            circle_radius_variables.push(CircleRadiusVariableMapping {
                circle_id,
                variable_id,
            });
            let bound_id = problem.add_bound(CoordinateBound::new(
                variable_id,
                0,
                Some(MIN_REPRESENTABLE_RADIUS),
                None,
                format!("positive radius for {}", circle.label()),
            )?)?;
            bound_mappings.push(SketchBoundMapping {
                bound: SketchBound::CircleRadius(circle_id),
                bound_id,
            });
        }
        let mut arc_radius_variables = Vec::new();
        for (arc_id, arc) in self.arcs.iter() {
            validate_radius(arc.radius())?;
            let variable_id =
                problem.add_variable(VariableBlock::scalar(arc.radius(), self.model_scale)?);
            arc_radius_variables.push(ArcRadiusVariableMapping {
                arc_id,
                variable_id,
            });
            let bound_id = problem.add_bound(CoordinateBound::new(
                variable_id,
                0,
                Some(MIN_REPRESENTABLE_RADIUS),
                None,
                format!("positive radius for {}", arc.label()),
            )?)?;
            bound_mappings.push(SketchBoundMapping {
                bound: SketchBound::ArcRadius(arc_id),
                bound_id,
            });
        }
        let mut conic_vector_variables = Vec::new();
        for (conic_id, conic) in self.conics.iter() {
            let crate::ConicKind::RationalQuadratic {
                weighted_middle, ..
            } = conic.kind()
            else {
                continue;
            };
            let variable_id = problem.add_variable(VariableBlock::vec2(
                [weighted_middle.x, weighted_middle.y],
                [self.model_scale, self.model_scale],
            )?);
            conic_vector_variables.push(ConicVectorVariableMapping {
                conic_id,
                role: ConicVectorRole::WeightedMiddle,
                variable_id,
            });
        }
        let mut conic_scalar_variables = Vec::new();
        for (conic_id, conic) in self.conics.iter() {
            let scalar = match conic.kind() {
                crate::ConicKind::Ellipse {
                    minor_axis_ratio, ..
                }
                | crate::ConicKind::EllipticalArc {
                    minor_axis_ratio, ..
                } => Some((
                    ConicScalarRole::MinorAxisRatio,
                    minor_axis_ratio,
                    1.0,
                    Some(MIN_REPRESENTABLE_RADIUS),
                    Some(1.0),
                )),
                crate::ConicKind::RationalQuadratic { middle_weight, .. } => Some((
                    ConicScalarRole::MiddleWeight,
                    middle_weight,
                    1.0,
                    Some(MIN_RATIONAL_QUADRATIC_MIDDLE_WEIGHT),
                    None,
                )),
                crate::ConicKind::HyperbolaSegment { semi_conjugate, .. } => Some((
                    ConicScalarRole::SemiConjugate,
                    semi_conjugate,
                    self.model_scale,
                    Some(MIN_REPRESENTABLE_RADIUS),
                    None,
                )),
                crate::ConicKind::ParabolaSegment { .. } => None,
            };
            let Some((role, value, step, lower, upper)) = scalar else {
                continue;
            };
            let variable_id = problem.add_variable(VariableBlock::scalar(value, step)?);
            conic_scalar_variables.push(ConicScalarVariableMapping {
                conic_id,
                role,
                variable_id,
            });
            let bound_id = problem.add_bound(CoordinateBound::new(
                variable_id,
                0,
                lower,
                upper,
                format!("{role:?} for {}", conic.label()),
            )?)?;
            bound_mappings.push(SketchBoundMapping {
                bound: SketchBound::ConicScalar { conic_id, role },
                bound_id,
            });
        }

        let mut source_mappings = Vec::new();
        let mut latent_variables = Vec::new();
        for source in &self.source_order {
            match *source {
                PersistentSource::Constraint(constraint_id) => {
                    let Some(constraint) = self.constraints.get(constraint_id) else {
                        continue;
                    };
                    source_mappings.push(compile_constraint(
                        self,
                        &mut problem,
                        &point_variables,
                        &circle_radius_variables,
                        &arc_radius_variables,
                        &conic_vector_variables,
                        &conic_scalar_variables,
                        &mut latent_variables,
                        &mut bound_mappings,
                        constraint_id,
                        constraint,
                    )?);
                }
                PersistentSource::Dimension(dimension_id) => {
                    let Some(dimension) = self.dimensions.get(dimension_id) else {
                        continue;
                    };
                    source_mappings.push(compile_dimension(
                        self,
                        &mut problem,
                        &point_variables,
                        &circle_radius_variables,
                        &arc_radius_variables,
                        dimension_id,
                        dimension,
                    )?);
                }
            }
        }

        if let Some(drag) = request.drag {
            source_mappings.push(compile_point_target(
                self,
                &mut problem,
                &point_variables,
                SketchSource::DragTarget(drag.point),
                drag.point,
                drag.target,
                ResidualCategory::Temporary,
                format!("temporary drag target for {}", self.point_name(drag.point)?),
            )?);
        }
        if let Some(stability) = request.stability_target {
            source_mappings.push(compile_point_target(
                self,
                &mut problem,
                &point_variables,
                SketchSource::DragTarget(stability.point),
                stability.point,
                stability.target,
                ResidualCategory::Temporary,
                format!(
                    "temporary stability target for {}",
                    self.point_name(stability.point)?
                ),
            )?);
        }
        if request.previous_state_preferences {
            for (point_id, point) in self.points.iter() {
                if request.drag.is_some_and(|drag| drag.point == point_id)
                    || request
                        .stability_target
                        .is_some_and(|stability| stability.point == point_id)
                {
                    continue;
                }
                source_mappings.push(compile_point_target(
                    self,
                    &mut problem,
                    &point_variables,
                    SketchSource::PreviousState(point_id),
                    point_id,
                    point.position(),
                    ResidualCategory::Preference,
                    format!("previous-state preference for {}", point.label()),
                )?);
            }
        }

        // Fixed declarations synchronize eagerly in core. Restore the domain's
        // retained coordinates so the compile seam and pre-attempt audit use
        // the exact warm-start state; solve synchronization remains trusted.
        for mapping in &point_variables {
            let position = self.point_position(mapping.point_id)?;
            problem.set_variable_value(
                mapping.variable_id,
                VariableValue::Vec2([position.x, position.y]),
            )?;
        }

        Ok(CompiledSketch {
            problem,
            point_variables,
            circle_radius_variables,
            arc_radius_variables,
            conic_vector_variables,
            conic_scalar_variables,
            latent_variables,
            bound_mappings,
            source_mappings,
        })
    }

    /// Compiles, solves, independently validates, and conditionally commits a request.
    ///
    /// # Errors
    ///
    /// Returns an error when compilation or the core solve cannot be started.
    /// Numerical and geometric solve failures are returned as rejected results.
    #[allow(clippy::too_many_lines)]
    pub fn solve(
        &mut self,
        request: SketchSolveRequest,
        config: SolverConfig,
    ) -> Result<SketchSolveResult, SketchError> {
        let config = acceptance_solver_config(config);
        let mut compiled = self.compile(request)?;
        let mut retained_audit = compiled.problem.audit_snapshot_partial();
        let mut core_report = compiled.problem.solve(config)?;
        let mut candidate = compiled.solved_state(self)?;
        let mut acceptance_hard_residual_max = None;

        if core_report_is_successful(&core_report, config) {
            let mut analysis_sketch = self.clone();
            for _ in 0..3 {
                if self.candidate_has_invalid_primitive(&candidate) {
                    break;
                }
                let normalized = self.normalize_candidate_latents(&mut candidate);
                if !normalized && !analysis_sketch.candidate_latents_differ(&candidate.latents) {
                    break;
                }
                analysis_sketch.commit_solved_state(&candidate)?;
                compiled = analysis_sketch.compile(request)?;
                core_report = compiled.problem.solve(config)?;
                candidate = compiled.solved_state(&analysis_sketch)?;
                if !core_report_is_successful(&core_report, config) {
                    break;
                }
            }
        }

        let core_hard_validity = core_report.hard_validity;
        let candidate_domain_rejection = self
            .first_flipped_segment(&candidate.geometry)
            .map(SolveRejection::SegmentBranchFlipped)
            .or_else(|| self.validate_m7_candidate(&candidate).err());
        let domain_rejection = if core_hard_validity == HardValidity::Valid {
            match independent_hard_residual_metrics(&compiled.problem) {
                Ok((maximum, _, _)) => {
                    acceptance_hard_residual_max = Some(maximum);
                    if maximum > config.normalized_residual_tolerance {
                        Some(SolveRejection::HardResidual {
                            maximum,
                            tolerance: config.normalized_residual_tolerance,
                        })
                    } else {
                        candidate_domain_rejection
                    }
                }
                Err(error) => candidate_domain_rejection.or_else(|| {
                    Some(SolveRejection::IndependentValidationFailed(
                        error.to_string(),
                    ))
                }),
            }
        } else {
            candidate_domain_rejection
        };
        core_report.hard_validity =
            domain_hard_validity(core_hard_validity, domain_rejection.as_ref());

        let mut rejection = if let Some(rejection) = domain_rejection {
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
        if let Some(constraint) = self.drag_requests_zero_circle_arc_radius(request) {
            core_report.termination = SolveTermination::Stalled;
            rejection = Some(SolveRejection::AmbiguousTangencyScale(constraint));
        } else if self
            .validate_drag_selected_span(request, &candidate)
            .is_err()
        {
            core_report.termination = SolveTermination::Stalled;
            rejection = Some(SolveRejection::CoreTermination(SolveTermination::Stalled));
        } else if request.drag.is_some()
            && matches!(rejection, Some(SolveRejection::AmbiguousTangencyScale(_)))
        {
            core_report.termination = SolveTermination::Stalled;
        }

        if rejection.is_none() {
            self.commit_solved_state(&candidate)?;
        }
        let display_audit = if rejection.is_none() {
            core_report.audit.clone()
        } else {
            if !matches!(
                rejection,
                Some(SolveRejection::ContactParameterOutOfDomain(_))
            ) {
                merge_conflicting_annotations(&mut retained_audit, &core_report.audit);
            }
            retained_audit
        };

        Ok(SketchSolveResult {
            geometry: self.geometry(),
            display_audit,
            reference_values: self.reference_values()?,
            source_mappings: compiled.source_mappings,
            bound_mappings: compiled.bound_mappings,
            core_report,
            rejection,
            acceptance_hard_residual_max,
        })
    }

    #[must_use]
    pub fn geometry(&self) -> SketchGeometry {
        SketchGeometry {
            points: self
                .points
                .iter()
                .map(|(point_id, point)| SolvedPoint {
                    point_id,
                    position: point.position(),
                })
                .collect(),
            circles: self
                .circles
                .iter()
                .filter_map(|(circle_id, circle)| {
                    Some(SolvedCircle {
                        circle_id,
                        center: self.point_position(circle.center()).ok()?,
                        radius: circle.radius(),
                    })
                })
                .collect(),
            arcs: self
                .arcs
                .iter()
                .filter_map(|(arc_id, arc)| {
                    Some(SolvedArc {
                        arc_id,
                        center: self.point_position(arc.center()).ok()?,
                        radius: arc.radius(),
                        start_angle: arc.start_angle(),
                        end_angle: arc.end_angle(),
                        signed_sweep: arc.signed_sweep(),
                        sweep: arc.sweep(),
                    })
                })
                .collect(),
            conics: self
                .conics
                .iter()
                .filter_map(|(conic_id, conic)| {
                    solved_conic_from_runtime(self, conic_id, conic).ok()
                })
                .collect(),
        }
    }

    pub(crate) fn reference_values(&self) -> Result<Vec<ReferenceDimensionValue>, SketchError> {
        self.dimensions
            .iter()
            .filter(|(_, dimension)| dimension.mode() == DimensionMode::Reference)
            .map(|(dimension_id, dimension)| {
                Ok(ReferenceDimensionValue {
                    dimension_id,
                    value: self.dimension_value(dimension)?,
                })
            })
            .collect()
    }

    pub(crate) fn first_flipped_segment(&self, geometry: &SketchGeometry) -> Option<SegmentId> {
        self.segments.iter().find_map(|(segment_id, segment)| {
            if !self.segment_branch_is_enforced(segment_id) {
                return None;
            }
            let start = geometry.point(segment.start())?;
            let end = geometry.point(segment.end())?;
            (!segment.branch().is_preserved(start, end)).then_some(segment_id)
        })
    }

    #[allow(clippy::too_many_lines)]
    pub(crate) fn validate_m7_candidate(
        &self,
        candidate: &SolvedSketchState,
    ) -> Result<(), SolveRejection> {
        for (segment_id, segment) in self.segments.iter() {
            let start = candidate.geometry.point(segment.start()).ok_or_else(|| {
                SolveRejection::IndependentValidationFailed(
                    "segment start point is missing from candidate geometry".into(),
                )
            })?;
            let end = candidate.geometry.point(segment.end()).ok_or_else(|| {
                SolveRejection::IndependentValidationFailed(
                    "segment end point is missing from candidate geometry".into(),
                )
            })?;
            let length = (end - start).norm();
            if !length.is_finite() || length == 0.0 {
                return Err(SolveRejection::DegenerateSegment(segment_id));
            }
        }
        for circle in &candidate.geometry.circles {
            if !circle.radius.is_finite() || circle.radius <= 0.0 {
                return Err(SolveRejection::NonPositiveCircleRadius(circle.circle_id));
            }
        }
        for arc in &candidate.geometry.arcs {
            if !arc.radius.is_finite() || arc.radius <= 0.0 {
                return Err(SolveRejection::NonPositiveArcRadius(arc.arc_id));
            }
        }
        for conic in &candidate.geometry.conics {
            validate_solved_conic_entity(*conic)
                .map_err(|_| SolveRejection::InvalidConicEntity(conic.conic_id))?;
        }
        for (constraint_id, constraint) in self.constraints.iter() {
            match constraint.kind() {
                SketchConstraintKind::PointOnLine {
                    point,
                    segment,
                    domain,
                    ..
                } => {
                    let parameter = latent_value(
                        &candidate.latents,
                        constraint_id,
                        LatentVariableRole::LineParameter,
                    )?;
                    if !domain.contains(parameter) {
                        return Err(SolveRejection::ContactParameterOutOfDomain(constraint_id));
                    }
                    if domain == crate::LineParameterDomain::BoundedSegment {
                        let segment = self.segments.get(segment).ok_or(
                            SolveRejection::IndependentValidationFailed(
                                "point-on-line references a stale segment".into(),
                            ),
                        )?;
                        let start = candidate.geometry.point(segment.start()).ok_or_else(|| {
                            SolveRejection::IndependentValidationFailed(
                                "point-on-line start point is missing".into(),
                            )
                        })?;
                        let end = candidate.geometry.point(segment.end()).ok_or_else(|| {
                            SolveRejection::IndependentValidationFailed(
                                "point-on-line end point is missing".into(),
                            )
                        })?;
                        let point = candidate.geometry.point(point).ok_or_else(|| {
                            SolveRejection::IndependentValidationFailed(
                                "point-on-line point is missing".into(),
                            )
                        })?;
                        let implied =
                            projected_line_parameter(start, end, point).ok_or_else(|| {
                                SolveRejection::IndependentValidationFailed(
                                    "point-on-line projection could not be evaluated".into(),
                                )
                            })?;
                        if domain.normalize_candidate(implied).is_none() {
                            return Err(SolveRejection::ContactParameterOutOfDomain(constraint_id));
                        }
                    }
                }
                SketchConstraintKind::PointOnCircle { .. } => {
                    let angle = latent_value(
                        &candidate.latents,
                        constraint_id,
                        LatentVariableRole::CircleAngle,
                    )?;
                    if !angle.is_finite() {
                        return Err(SolveRejection::ContactParameterOutOfDomain(constraint_id));
                    }
                }
                SketchConstraintKind::PointOnArc { point, arc, .. } => {
                    let parameter = latent_value(
                        &candidate.latents,
                        constraint_id,
                        LatentVariableRole::ArcSpanParameter,
                    )?;
                    if validate_bounded_parameter(parameter, "bounded-arc span [0, 1]").is_err() {
                        return Err(SolveRejection::ContactParameterOutOfDomain(constraint_id));
                    }
                    let solved_arc = candidate.geometry.arc(arc).ok_or_else(|| {
                        SolveRejection::IndependentValidationFailed(
                            "point-on-arc arc is missing".into(),
                        )
                    })?;
                    let point = candidate.geometry.point(point).ok_or_else(|| {
                        SolveRejection::IndependentValidationFailed(
                            "point-on-arc point is missing".into(),
                        )
                    })?;
                    let offset = point - solved_arc.center;
                    let angle = offset.y.atan2(offset.x);
                    let reference = solved_arc.start_angle + solved_arc.signed_sweep * parameter;
                    let implied = (unwrap_near(angle, reference) - solved_arc.start_angle)
                        / solved_arc.signed_sweep;
                    if normalize_bounded_candidate(implied).is_none() {
                        return Err(SolveRejection::ContactParameterOutOfDomain(constraint_id));
                    }
                }
                SketchConstraintKind::PointOnBezier { bezier, .. } => {
                    let parameter = latent_value(
                        &candidate.latents,
                        constraint_id,
                        LatentVariableRole::BezierParameter,
                    )?;
                    if normalize_bounded_candidate(parameter).is_none() {
                        return Err(SolveRejection::ContactParameterOutOfDomain(constraint_id));
                    }
                    candidate_bezier_jet(self, candidate, bezier, parameter)
                        .map_err(|_| SolveRejection::DegenerateCurve(constraint_id))?;
                }
                SketchConstraintKind::LineCircleTangency {
                    line,
                    circle,
                    domain,
                    side,
                    ..
                } => {
                    let parameter = latent_value(
                        &candidate.latents,
                        constraint_id,
                        LatentVariableRole::LineParameter,
                    )?;
                    let angle = latent_value(
                        &candidate.latents,
                        constraint_id,
                        LatentVariableRole::CircleAngle,
                    )?;
                    if !domain.contains(parameter) || !angle.is_finite() {
                        return Err(SolveRejection::ContactParameterOutOfDomain(constraint_id));
                    }
                    let segment = self.segments.get(line).ok_or(
                        SolveRejection::IndependentValidationFailed(
                            "line tangency references a stale segment".into(),
                        ),
                    )?;
                    let start = candidate.geometry.point(segment.start()).ok_or_else(|| {
                        SolveRejection::IndependentValidationFailed(
                            "line tangency start point is missing".into(),
                        )
                    })?;
                    let end = candidate.geometry.point(segment.end()).ok_or_else(|| {
                        SolveRejection::IndependentValidationFailed(
                            "line tangency end point is missing".into(),
                        )
                    })?;
                    let center = candidate
                        .geometry
                        .circle(circle)
                        .ok_or_else(|| {
                            SolveRejection::IndependentValidationFailed(
                                "line tangency circle is missing".into(),
                            )
                        })?
                        .center;
                    if domain == crate::LineParameterDomain::BoundedSegment {
                        let circle = candidate.geometry.circle(circle).ok_or_else(|| {
                            SolveRejection::IndependentValidationFailed(
                                "line tangency circle is missing".into(),
                            )
                        })?;
                        let contact = circle.evaluate(angle).ok_or_else(|| {
                            SolveRejection::IndependentValidationFailed(
                                "line tangency circle contact is invalid".into(),
                            )
                        })?;
                        let implied =
                            projected_line_parameter(start, end, contact).ok_or_else(|| {
                                SolveRejection::IndependentValidationFailed(
                                    "line tangency projection could not be evaluated".into(),
                                )
                            })?;
                        if domain.normalize_candidate(implied).is_none() {
                            return Err(SolveRejection::ContactParameterOutOfDomain(constraint_id));
                        }
                    }
                    let direction = end - start;
                    let offset = center - start;
                    let signed_side = direction.x * offset.y - direction.y * offset.x;
                    if !signed_side.is_finite() || side.sign() * signed_side <= 0.0 {
                        return Err(SolveRejection::LineSideFlipped(constraint_id));
                    }
                }
                SketchConstraintKind::CircleCircleTangency {
                    first,
                    second,
                    mode,
                    center_direction,
                } => {
                    let first_circle = candidate.geometry.circle(first).ok_or_else(|| {
                        SolveRejection::IndependentValidationFailed(
                            "first tangent circle is missing".into(),
                        )
                    })?;
                    let second_circle = candidate.geometry.circle(second).ok_or_else(|| {
                        SolveRejection::IndependentValidationFailed(
                            "second tangent circle is missing".into(),
                        )
                    })?;
                    let effective =
                        tangency_distance(first_circle.radius, second_circle.radius, mode);
                    if !effective.is_finite() || effective <= 0.0 {
                        return Err(SolveRejection::InvalidTangencyMode(constraint_id));
                    }
                    let direction_cosine = center_direction
                        .direction_cosine(first_circle.center, second_circle.center);
                    if !direction_cosine.is_some_and(|value| value > CENTER_DIRECTION_COSINE_MARGIN)
                    {
                        return Err(SolveRejection::CenterDirectionFlipped(constraint_id));
                    }
                }
                SketchConstraintKind::CircleArcTangency {
                    circle, arc, side, ..
                } => {
                    let arc_parameter = latent_value(
                        &candidate.latents,
                        constraint_id,
                        LatentVariableRole::ArcSpanParameter,
                    )?;
                    let circle_angle = latent_value(
                        &candidate.latents,
                        constraint_id,
                        LatentVariableRole::CircleAngle,
                    )?;
                    if validate_bounded_parameter(arc_parameter, "bounded-arc span [0, 1]").is_err()
                        || !circle_angle.is_finite()
                    {
                        return Err(SolveRejection::ContactParameterOutOfDomain(constraint_id));
                    }
                    let solved_circle = candidate.geometry.circle(circle).ok_or_else(|| {
                        SolveRejection::IndependentValidationFailed(
                            "circle-arc tangency circle is missing".into(),
                        )
                    })?;
                    let solved_arc = candidate.geometry.arc(arc).ok_or_else(|| {
                        SolveRejection::IndependentValidationFailed(
                            "circle-arc tangency arc is missing".into(),
                        )
                    })?;
                    let circle_contact = solved_circle.evaluate(circle_angle).ok_or_else(|| {
                        SolveRejection::IndependentValidationFailed(
                            "circle-arc tangency circle contact is invalid".into(),
                        )
                    })?;
                    let arc_offset = circle_contact - solved_arc.center;
                    let contact_angle = arc_offset.y.atan2(arc_offset.x);
                    let reference =
                        solved_arc.start_angle + solved_arc.signed_sweep * arc_parameter;
                    let implied_arc_parameter = (unwrap_near(contact_angle, reference)
                        - solved_arc.start_angle)
                        / solved_arc.signed_sweep;
                    if normalize_bounded_candidate(implied_arc_parameter).is_none() {
                        return Err(SolveRejection::ContactParameterOutOfDomain(constraint_id));
                    }
                    let center_offset = solved_circle.center - solved_arc.center;
                    let center_distance = center_offset.norm();
                    if !center_distance.is_finite() || center_distance == 0.0 {
                        return Err(SolveRejection::InvalidTangencyMode(constraint_id));
                    }
                    if !side.accepts(center_distance, solved_arc.radius) {
                        return Err(SolveRejection::InvalidTangencyMode(constraint_id));
                    }
                    let derived_radius = match side {
                        crate::ArcCircleTangencySide::OutsideArc => {
                            center_distance - solved_arc.radius
                        }
                        crate::ArcCircleTangencySide::InsideArc => {
                            solved_arc.radius - center_distance
                        }
                    };
                    if !derived_radius.is_finite() || derived_radius <= 0.0 {
                        return Err(SolveRejection::InvalidTangencyMode(constraint_id));
                    }
                    match circle_arc_radius_validation(
                        solved_circle.radius,
                        derived_radius,
                        center_distance,
                        solved_arc.radius,
                        side,
                    ) {
                        CircleArcRadiusValidation::Valid => {}
                        CircleArcRadiusValidation::Mismatch => {
                            return Err(SolveRejection::InvalidTangencyMode(constraint_id));
                        }
                        CircleArcRadiusValidation::AmbiguousScale => {
                            return Err(SolveRejection::AmbiguousTangencyScale(constraint_id));
                        }
                    }
                    let arc_contact = solved_arc.evaluate(arc_parameter).ok_or_else(|| {
                        SolveRejection::IndependentValidationFailed(
                            "circle-arc tangency arc contact is invalid".into(),
                        )
                    })?;
                    let center_direction = [
                        center_offset.x / center_distance,
                        center_offset.y / center_distance,
                    ];
                    let arc_radial = normalized_direction(
                        arc_contact.x - solved_arc.center.x,
                        arc_contact.y - solved_arc.center.y,
                    )
                    .ok_or_else(|| {
                        SolveRejection::IndependentValidationFailed(
                            "circle-arc tangency arc radial is degenerate".into(),
                        )
                    })?;
                    let circle_radial = normalized_direction(
                        circle_contact.x - solved_circle.center.x,
                        circle_contact.y - solved_circle.center.y,
                    )
                    .ok_or_else(|| {
                        SolveRejection::IndependentValidationFailed(
                            "circle-arc tangency circle radial is degenerate".into(),
                        )
                    })?;
                    let expected_circle_radial = [
                        side.circle_arc_radial_sign() * arc_radial[0],
                        side.circle_arc_radial_sign() * arc_radial[1],
                    ];
                    if !directions_match(arc_radial, center_direction)
                        || !directions_match(circle_radial, expected_circle_radial)
                    {
                        return Err(SolveRejection::CenterDirectionFlipped(constraint_id));
                    }
                }
                SketchConstraintKind::LineBezierTangency {
                    line,
                    bezier,
                    orientation,
                    ..
                } => {
                    let parameter = latent_value(
                        &candidate.latents,
                        constraint_id,
                        LatentVariableRole::BezierParameter,
                    )?;
                    if normalize_bounded_candidate(parameter).is_none() {
                        return Err(SolveRejection::ContactParameterOutOfDomain(constraint_id));
                    }
                    let jet = candidate_bezier_jet(self, candidate, bezier, parameter)
                        .map_err(|_| SolveRejection::DegenerateCurve(constraint_id))?;
                    let segment = self.segments.get(line).ok_or_else(|| {
                        SolveRejection::IndependentValidationFailed(
                            "line-Bezier tangency references a stale line".into(),
                        )
                    })?;
                    let start = candidate.geometry.point(segment.start()).ok_or_else(|| {
                        SolveRejection::IndependentValidationFailed(
                            "line-Bezier tangency start is missing".into(),
                        )
                    })?;
                    let end = candidate.geometry.point(segment.end()).ok_or_else(|| {
                        SolveRejection::IndependentValidationFailed(
                            "line-Bezier tangency end is missing".into(),
                        )
                    })?;
                    let line_direction = end - start;
                    let line_norm = line_direction.norm();
                    let curve_norm = jet.first_derivative.norm();
                    if !line_norm.is_finite()
                        || !curve_norm.is_finite()
                        || line_norm == 0.0
                        || curve_norm == 0.0
                    {
                        return Err(SolveRejection::DegenerateCurve(constraint_id));
                    }
                    let cosine =
                        line_direction.dot(&jet.first_derivative) / (line_norm * curve_norm);
                    let valid = match orientation {
                        crate::CurveTangentOrientation::Aligned => cosine > 0.0,
                        crate::CurveTangentOrientation::Opposed => cosine < 0.0,
                    };
                    if !valid {
                        return Err(SolveRejection::CenterDirectionFlipped(constraint_id));
                    }
                }
                SketchConstraintKind::PointOnCurve { contact, .. } => {
                    let parameter = latent_value(
                        &candidate.latents,
                        constraint_id,
                        LatentVariableRole::CurveParameter,
                    )?;
                    validate_generic_contact_candidate(
                        self,
                        candidate,
                        constraint_id,
                        contact,
                        parameter,
                    )?;
                }
                SketchConstraintKind::LineCurveTangency {
                    line,
                    contact,
                    orientation,
                    ..
                } => {
                    let parameter = latent_value(
                        &candidate.latents,
                        constraint_id,
                        LatentVariableRole::CurveParameter,
                    )?;
                    let curve_jet = validate_generic_contact_candidate(
                        self,
                        candidate,
                        constraint_id,
                        contact,
                        parameter,
                    )?;
                    let line_jet = candidate_curve_jet(
                        self,
                        candidate,
                        SketchCurve::Line {
                            segment: line,
                            domain: crate::LineParameterDomain::BoundedSegment,
                        },
                        0.0,
                    )
                    .map_err(|()| SolveRejection::DegenerateCurve(constraint_id))?;
                    validate_generic_orientation(
                        constraint_id,
                        line_jet.first_derivative,
                        curve_jet.first_derivative,
                        orientation,
                    )?;
                }
                SketchConstraintKind::CurveCurveContact { first, second }
                | SketchConstraintKind::CurveCurveTangency {
                    first,
                    second,
                    orientation: _,
                } => {
                    let first_parameter = latent_value(
                        &candidate.latents,
                        constraint_id,
                        LatentVariableRole::FirstCurveParameter,
                    )?;
                    let second_parameter = latent_value(
                        &candidate.latents,
                        constraint_id,
                        LatentVariableRole::SecondCurveParameter,
                    )?;
                    let first_jet = validate_generic_contact_candidate(
                        self,
                        candidate,
                        constraint_id,
                        first,
                        first_parameter,
                    )?;
                    let second_jet = validate_generic_contact_candidate(
                        self,
                        candidate,
                        constraint_id,
                        second,
                        second_parameter,
                    )?;
                    if let SketchConstraintKind::CurveCurveTangency { orientation, .. } =
                        constraint.kind()
                    {
                        validate_generic_orientation(
                            constraint_id,
                            first_jet.first_derivative,
                            second_jet.first_derivative,
                            orientation,
                        )?;
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    #[allow(clippy::too_many_lines)]
    pub(crate) fn normalize_candidate_latents(&self, candidate: &mut SolvedSketchState) -> bool {
        let mut changed = false;
        for (constraint_id, constraint) in self.constraints.iter() {
            match constraint.kind() {
                SketchConstraintKind::PointOnLine { domain, .. } => {
                    if let Some(latent) = latent_mut(
                        &mut candidate.latents,
                        constraint_id,
                        LatentVariableRole::LineParameter,
                    ) {
                        let normalized = domain
                            .normalize_candidate(latent.value)
                            .unwrap_or(latent.value);
                        changed |= normalized.to_bits() != latent.value.to_bits();
                        latent.value = normalized;
                    }
                }
                SketchConstraintKind::PointOnCircle { angle, .. } => {
                    if let Some(latent) = latent_mut(
                        &mut candidate.latents,
                        constraint_id,
                        LatentVariableRole::CircleAngle,
                    ) {
                        let normalized = unwrap_near(latent.value, angle);
                        changed |= normalized.to_bits() != latent.value.to_bits();
                        latent.value = normalized;
                    }
                }
                SketchConstraintKind::PointOnArc { .. } => {
                    if let Some(latent) = latent_mut(
                        &mut candidate.latents,
                        constraint_id,
                        LatentVariableRole::ArcSpanParameter,
                    ) {
                        let normalized =
                            normalize_bounded_candidate(latent.value).unwrap_or(latent.value);
                        changed |= normalized.to_bits() != latent.value.to_bits();
                        latent.value = normalized;
                    }
                }
                SketchConstraintKind::PointOnBezier { .. }
                | SketchConstraintKind::LineBezierTangency { .. } => {
                    if let Some(latent) = latent_mut(
                        &mut candidate.latents,
                        constraint_id,
                        LatentVariableRole::BezierParameter,
                    ) {
                        let normalized =
                            normalize_bounded_candidate(latent.value).unwrap_or(latent.value);
                        changed |= normalized.to_bits() != latent.value.to_bits();
                        latent.value = normalized;
                    }
                }
                SketchConstraintKind::PointOnCurve { contact, .. }
                | SketchConstraintKind::LineCurveTangency { contact, .. } => {
                    normalize_generic_latent(
                        self,
                        &mut candidate.latents,
                        constraint_id,
                        LatentVariableRole::CurveParameter,
                        contact,
                        &mut changed,
                    );
                }
                SketchConstraintKind::CurveCurveContact { first, second }
                | SketchConstraintKind::CurveCurveTangency { first, second, .. } => {
                    normalize_generic_latent(
                        self,
                        &mut candidate.latents,
                        constraint_id,
                        LatentVariableRole::FirstCurveParameter,
                        first,
                        &mut changed,
                    );
                    normalize_generic_latent(
                        self,
                        &mut candidate.latents,
                        constraint_id,
                        LatentVariableRole::SecondCurveParameter,
                        second,
                        &mut changed,
                    );
                }
                SketchConstraintKind::LineCircleTangency {
                    domain,
                    circle_angle,
                    ..
                } => {
                    if let Some(latent) = latent_mut(
                        &mut candidate.latents,
                        constraint_id,
                        LatentVariableRole::LineParameter,
                    ) {
                        let normalized = domain
                            .normalize_candidate(latent.value)
                            .unwrap_or(latent.value);
                        changed |= normalized.to_bits() != latent.value.to_bits();
                        latent.value = normalized;
                    }
                    if let Some(latent) = latent_mut(
                        &mut candidate.latents,
                        constraint_id,
                        LatentVariableRole::CircleAngle,
                    ) {
                        let normalized = unwrap_near(latent.value, circle_angle);
                        changed |= normalized.to_bits() != latent.value.to_bits();
                        latent.value = normalized;
                    }
                }
                SketchConstraintKind::CircleArcTangency {
                    arc_span_parameter: _,
                    circle_angle,
                    ..
                } => {
                    if let Some(latent) = latent_mut(
                        &mut candidate.latents,
                        constraint_id,
                        LatentVariableRole::CircleAngle,
                    ) {
                        let normalized = unwrap_near(latent.value, circle_angle);
                        changed |= normalized.to_bits() != latent.value.to_bits();
                        latent.value = normalized;
                    }
                    if let Some(latent) = latent_mut(
                        &mut candidate.latents,
                        constraint_id,
                        LatentVariableRole::ArcSpanParameter,
                    ) {
                        let normalized =
                            normalize_bounded_candidate(latent.value).unwrap_or(latent.value);
                        changed |= normalized.to_bits() != latent.value.to_bits();
                        latent.value = normalized;
                    }
                }
                _ => {}
            }
        }
        changed
    }

    #[allow(clippy::too_many_lines)]
    fn candidate_latents_differ(&self, latents: &[SolvedLatent]) -> bool {
        latents.iter().any(|latent| {
            let is_bounded =
                self.constraint(latent.constraint_id)
                    .is_some_and(|constraint| match (constraint.kind(), latent.role) {
                        (
                            SketchConstraintKind::PointOnLine {
                                domain: crate::LineParameterDomain::BoundedSegment,
                                ..
                            }
                            | SketchConstraintKind::LineCircleTangency {
                                domain: crate::LineParameterDomain::BoundedSegment,
                                ..
                            },
                            LatentVariableRole::LineParameter,
                        )
                        | (
                            SketchConstraintKind::PointOnArc { .. }
                            | SketchConstraintKind::CircleArcTangency { .. },
                            LatentVariableRole::ArcSpanParameter,
                        )
                        | (
                            SketchConstraintKind::PointOnBezier { .. }
                            | SketchConstraintKind::LineBezierTangency { .. },
                            LatentVariableRole::BezierParameter,
                        ) => true,
                        (
                            SketchConstraintKind::PointOnCurve { contact, .. }
                            | SketchConstraintKind::LineCurveTangency { contact, .. },
                            LatentVariableRole::CurveParameter,
                        )
                        | (
                            SketchConstraintKind::CurveCurveContact { first: contact, .. }
                            | SketchConstraintKind::CurveCurveTangency { first: contact, .. },
                            LatentVariableRole::FirstCurveParameter,
                        )
                        | (
                            SketchConstraintKind::CurveCurveContact {
                                second: contact, ..
                            }
                            | SketchConstraintKind::CurveCurveTangency {
                                second: contact, ..
                            },
                            LatentVariableRole::SecondCurveParameter,
                        ) => generic_curve_is_bounded(self, contact.curve),
                        _ => false,
                    });
            if !is_bounded {
                return false;
            }
            self.contact_state(latent.constraint_id)
                .ok()
                .and_then(|state| match (state, latent.role) {
                    (
                        crate::ContactState::PointOnLine { parameter },
                        LatentVariableRole::LineParameter,
                    )
                    | (
                        crate::ContactState::PointOnBezier { parameter }
                        | crate::ContactState::LineBezierTangency { parameter },
                        LatentVariableRole::BezierParameter,
                    )
                    | (
                        crate::ContactState::PointOnCurve { parameter }
                        | crate::ContactState::LineCurveTangency { parameter },
                        LatentVariableRole::CurveParameter,
                    ) => Some(parameter),
                    (
                        crate::ContactState::PointOnCircle { angle },
                        LatentVariableRole::CircleAngle,
                    ) => Some(angle),
                    (
                        crate::ContactState::PointOnArc { span_parameter },
                        LatentVariableRole::ArcSpanParameter,
                    ) => Some(span_parameter),
                    (
                        crate::ContactState::LineCircleTangency { line_parameter, .. },
                        LatentVariableRole::LineParameter,
                    ) => Some(line_parameter),
                    (
                        crate::ContactState::LineCircleTangency { circle_angle, .. }
                        | crate::ContactState::CircleArcTangency { circle_angle, .. },
                        LatentVariableRole::CircleAngle,
                    ) => Some(circle_angle),
                    (
                        crate::ContactState::CircleArcTangency {
                            arc_span_parameter, ..
                        },
                        LatentVariableRole::ArcSpanParameter,
                    ) => Some(arc_span_parameter),
                    (
                        crate::ContactState::CurveCurveContact {
                            first_parameter, ..
                        }
                        | crate::ContactState::CurveCurveTangency {
                            first_parameter, ..
                        },
                        LatentVariableRole::FirstCurveParameter,
                    ) => Some(first_parameter),
                    (
                        crate::ContactState::CurveCurveContact {
                            second_parameter, ..
                        }
                        | crate::ContactState::CurveCurveTangency {
                            second_parameter, ..
                        },
                        LatentVariableRole::SecondCurveParameter,
                    ) => Some(second_parameter),
                    _ => None,
                })
                .is_none_or(|accepted| accepted.to_bits() != latent.value.to_bits())
        })
    }

    fn candidate_has_invalid_primitive(&self, candidate: &SolvedSketchState) -> bool {
        self.segments.iter().any(|(_, segment)| {
            let Some(start) = candidate.geometry.point(segment.start()) else {
                return true;
            };
            let Some(end) = candidate.geometry.point(segment.end()) else {
                return true;
            };
            let length = (end - start).norm();
            !length.is_finite() || length == 0.0
        }) || candidate
            .geometry
            .circles
            .iter()
            .any(|circle| !circle.radius.is_finite() || circle.radius <= 0.0)
            || candidate
                .geometry
                .arcs
                .iter()
                .any(|arc| !arc.radius.is_finite() || arc.radius <= 0.0)
            || candidate
                .geometry
                .conics
                .iter()
                .any(|conic| validate_solved_conic_entity(*conic).is_err())
    }

    fn commit_latents(&mut self, latents: &[SolvedLatent]) -> Result<(), SketchError> {
        for latent in latents {
            let kind = &mut self
                .constraints
                .get_mut(latent.constraint_id)
                .ok_or(SketchError::UnknownConstraint(latent.constraint_id))?
                .kind;
            match (kind, latent.role) {
                (
                    SketchConstraintKind::PointOnLine { parameter, .. },
                    LatentVariableRole::LineParameter,
                )
                | (
                    SketchConstraintKind::PointOnBezier { parameter, .. },
                    LatentVariableRole::BezierParameter,
                ) => *parameter = latent.value,
                (
                    SketchConstraintKind::PointOnCircle { angle, .. },
                    LatentVariableRole::CircleAngle,
                ) => *angle = latent.value,
                (
                    SketchConstraintKind::PointOnArc { span_parameter, .. },
                    LatentVariableRole::ArcSpanParameter,
                ) => *span_parameter = latent.value,
                (
                    SketchConstraintKind::LineCircleTangency { line_parameter, .. },
                    LatentVariableRole::LineParameter,
                ) => *line_parameter = latent.value,
                (
                    SketchConstraintKind::LineCircleTangency { circle_angle, .. }
                    | SketchConstraintKind::CircleArcTangency { circle_angle, .. },
                    LatentVariableRole::CircleAngle,
                ) => *circle_angle = latent.value,
                (
                    SketchConstraintKind::CircleArcTangency {
                        arc_span_parameter, ..
                    },
                    LatentVariableRole::ArcSpanParameter,
                ) => *arc_span_parameter = latent.value,
                (
                    SketchConstraintKind::LineBezierTangency {
                        bezier_parameter, ..
                    },
                    LatentVariableRole::BezierParameter,
                ) => *bezier_parameter = latent.value,
                (
                    SketchConstraintKind::PointOnCurve { contact, .. }
                    | SketchConstraintKind::LineCurveTangency { contact, .. },
                    LatentVariableRole::CurveParameter,
                ) => contact.parameter = latent.value,
                (
                    SketchConstraintKind::CurveCurveContact { first, .. }
                    | SketchConstraintKind::CurveCurveTangency { first, .. },
                    LatentVariableRole::FirstCurveParameter,
                ) => first.parameter = latent.value,
                (
                    SketchConstraintKind::CurveCurveContact { second, .. }
                    | SketchConstraintKind::CurveCurveTangency { second, .. },
                    LatentVariableRole::SecondCurveParameter,
                ) => second.parameter = latent.value,
                _ => return Err(SketchError::NoContactState(latent.constraint_id)),
            }
        }
        Ok(())
    }

    pub(crate) fn commit_solved_state(
        &mut self,
        candidate: &SolvedSketchState,
    ) -> Result<(), SketchError> {
        for point in &candidate.geometry.points {
            self.set_point_position(point.point_id, point.position)?;
        }
        for circle in &candidate.geometry.circles {
            self.set_circle_radius(circle.circle_id, circle.radius)?;
        }
        for arc in &candidate.geometry.arcs {
            self.set_arc_radius(arc.arc_id, arc.radius)?;
        }
        for conic in &candidate.geometry.conics {
            match conic.kind {
                SolvedConicKind::Ellipse {
                    minor_axis_ratio, ..
                }
                | SolvedConicKind::EllipticalArc {
                    minor_axis_ratio, ..
                } => self.set_conic_minor_axis_ratio(conic.conic_id, minor_axis_ratio)?,
                SolvedConicKind::RationalQuadratic {
                    weighted_middle,
                    middle_weight,
                    ..
                } => self.set_rational_quadratic_homogeneous(
                    conic.conic_id,
                    weighted_middle,
                    middle_weight,
                )?,
                SolvedConicKind::HyperbolaSegment { semi_conjugate, .. } => {
                    self.set_conic_semi_conjugate(conic.conic_id, semi_conjugate)?;
                }
                SolvedConicKind::ParabolaSegment { .. } => {}
            }
        }
        self.commit_latents(&candidate.latents)
    }

    fn point_name(&self, point: PointId) -> Result<&str, SketchError> {
        self.points
            .get(point)
            .map(crate::SketchPoint::label)
            .ok_or(SketchError::UnknownPoint(point))
    }
}

impl Sketch {
    fn drag_requests_zero_circle_arc_radius(
        &self,
        request: SketchSolveRequest,
    ) -> Option<SketchConstraintId> {
        let drag = request.drag?;
        self.constraints
            .iter()
            .find_map(|(constraint_id, constraint)| {
                let SketchConstraintKind::CircleArcTangency { circle, arc, .. } = constraint.kind()
                else {
                    return None;
                };
                let circle = self.circle_value(circle).ok()?;
                if circle.center() != drag.point {
                    return None;
                }
                let arc = self.arc_value(arc).ok()?;
                let center = self.point_position(arc.center()).ok()?;
                let distance = (drag.target - center).norm();
                let tolerance = CIRCLE_ARC_TANGENCY_RADIUS_RELATIVE_TOLERANCE
                    * distance.abs().max(arc.radius().abs());
                ((distance - arc.radius()).abs() <= tolerance).then_some(constraint_id)
            })
    }

    #[allow(clippy::too_many_lines)]
    pub(crate) fn validate_drag_selected_span(
        &self,
        request: SketchSolveRequest,
        candidate: &SolvedSketchState,
    ) -> Result<(), SolveRejection> {
        let Some(drag) = request.drag else {
            return Ok(());
        };
        for (constraint_id, constraint) in self.constraints.iter() {
            let escaped = match constraint.kind() {
                SketchConstraintKind::PointOnLine {
                    point,
                    segment,
                    domain: crate::LineParameterDomain::BoundedSegment,
                    ..
                } if point == drag.point => {
                    let segment = self.segments.get(segment).ok_or_else(|| {
                        SolveRejection::IndependentValidationFailed(
                            "drag contact references a stale segment".into(),
                        )
                    })?;
                    let start = candidate.geometry.point(segment.start()).ok_or_else(|| {
                        SolveRejection::IndependentValidationFailed(
                            "drag contact start point is missing".into(),
                        )
                    })?;
                    let end = candidate.geometry.point(segment.end()).ok_or_else(|| {
                        SolveRejection::IndependentValidationFailed(
                            "drag contact end point is missing".into(),
                        )
                    })?;
                    projected_line_parameter(start, end, drag.target)
                        .and_then(normalize_bounded_candidate)
                        .is_none()
                }
                SketchConstraintKind::PointOnArc { point, arc, .. } if point == drag.point => {
                    let arc = candidate.geometry.arc(arc).ok_or_else(|| {
                        SolveRejection::IndependentValidationFailed(
                            "drag contact arc is missing".into(),
                        )
                    })?;
                    target_arc_parameter(*arc, drag.target, constraint_id, &candidate.latents)?
                        .and_then(normalize_bounded_candidate)
                        .is_none()
                }
                SketchConstraintKind::LineCircleTangency {
                    line,
                    circle,
                    domain: crate::LineParameterDomain::BoundedSegment,
                    ..
                } if self
                    .circle_value(circle)
                    .is_ok_and(|circle| circle.center() == drag.point) =>
                {
                    let line = self.segments.get(line).ok_or_else(|| {
                        SolveRejection::IndependentValidationFailed(
                            "drag tangency references a stale segment".into(),
                        )
                    })?;
                    let start = candidate.geometry.point(line.start()).ok_or_else(|| {
                        SolveRejection::IndependentValidationFailed(
                            "drag tangency start point is missing".into(),
                        )
                    })?;
                    let end = candidate.geometry.point(line.end()).ok_or_else(|| {
                        SolveRejection::IndependentValidationFailed(
                            "drag tangency end point is missing".into(),
                        )
                    })?;
                    projected_line_parameter(start, end, drag.target)
                        .and_then(normalize_bounded_candidate)
                        .is_none()
                }
                SketchConstraintKind::CircleArcTangency {
                    circle, arc, side, ..
                } if self
                    .circle_value(circle)
                    .is_ok_and(|circle| circle.center() == drag.point) =>
                {
                    let arc = candidate.geometry.arc(arc).ok_or_else(|| {
                        SolveRejection::IndependentValidationFailed(
                            "drag tangency arc is missing".into(),
                        )
                    })?;
                    let center_distance = (drag.target - arc.center).norm();
                    let side_gap = center_distance - arc.radius;
                    let side_tolerance = 64.0
                        * f64::EPSILON
                        * center_distance
                            .abs()
                            .max(arc.radius.abs())
                            .max(f64::MIN_POSITIVE);
                    if side_gap.abs() > side_tolerance && !side.accepts(center_distance, arc.radius)
                    {
                        return Err(SolveRejection::InvalidTangencyMode(constraint_id));
                    }
                    target_arc_parameter(*arc, drag.target, constraint_id, &candidate.latents)?
                        .and_then(normalize_bounded_candidate)
                        .is_none()
                }
                _ => false,
            };
            if escaped {
                return Err(SolveRejection::ContactParameterOutOfDomain(constraint_id));
            }
        }
        Ok(())
    }
}

fn target_arc_parameter(
    arc: SolvedArc,
    target: Point2<f64>,
    constraint: SketchConstraintId,
    latents: &[SolvedLatent],
) -> Result<Option<f64>, SolveRejection> {
    let offset = target - arc.center;
    if !offset.x.is_finite() || !offset.y.is_finite() || offset.norm() == 0.0 {
        return Ok(None);
    }
    let retained = latent_value(latents, constraint, LatentVariableRole::ArcSpanParameter)?;
    let angle = offset.y.atan2(offset.x);
    let reference = arc.start_angle + arc.signed_sweep * retained;
    let parameter = (unwrap_near(angle, reference) - arc.start_angle) / arc.signed_sweep;
    Ok(parameter.is_finite().then_some(parameter))
}

pub(crate) fn validate_request(
    sketch: &Sketch,
    request: SketchSolveRequest,
) -> Result<(), SketchError> {
    if let Some(drag) = request.drag {
        sketch.point_position(drag.point)?;
        validate_point(drag.target, "drag target")?;
    }
    Ok(())
}

fn core_report_is_successful(report: &SolveReport, config: SolverConfig) -> bool {
    report.termination == SolveTermination::Converged
        && report.hard_validity == HardValidity::Valid
        && report.hard_residuals_validated
        && report.hard_residual_max <= config.normalized_residual_tolerance
}

pub(crate) fn rejection_hard_validity(rejection: &SolveRejection) -> HardValidity {
    if matches!(rejection, SolveRejection::IndependentValidationFailed(_)) {
        HardValidity::NotEvaluated
    } else {
        HardValidity::Invalid
    }
}

pub(crate) fn domain_hard_validity(
    core_hard_validity: HardValidity,
    rejection: Option<&SolveRejection>,
) -> HardValidity {
    if core_hard_validity == HardValidity::Valid {
        rejection.map_or(HardValidity::Valid, rejection_hard_validity)
    } else {
        core_hard_validity
    }
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn compile_constraint(
    sketch: &Sketch,
    problem: &mut Problem,
    point_variables: &[PointVariableMapping],
    circle_radius_variables: &[CircleRadiusVariableMapping],
    arc_radius_variables: &[ArcRadiusVariableMapping],
    conic_vector_variables: &[ConicVectorVariableMapping],
    conic_scalar_variables: &[ConicScalarVariableMapping],
    latent_variables: &mut Vec<LatentVariableMapping>,
    bound_mappings: &mut Vec<SketchBoundMapping>,
    constraint_id: SketchConstraintId,
    constraint: &crate::SketchConstraint,
) -> Result<SketchSourceMapping, SketchError> {
    let scale = sketch.model_scale;
    let (label, residual) = match constraint.kind() {
        SketchConstraintKind::FixedPoint { point, target } => {
            let point_name = sketch.point_name(point)?;
            let label = format!(
                "constraint {}: fixed point {point_name}",
                constraint.ordinal()
            );
            let source_id = problem.add_source(SourceConstraint::new(&label)?);
            let variable = point_variable(point_variables, point)?;
            let rows = vec![
                audit_row(
                    format!("({point_name}.x - target.x) / model_scale"),
                    point_bindings(point_name, target),
                ),
                audit_row(
                    format!("({point_name}.y - target.y) / model_scale"),
                    point_bindings(point_name, target),
                ),
            ];
            let residual = ResidualBlock::fixed_variable(
                source_id,
                variable,
                VariableValue::Vec2([target.x, target.y]),
                vec![scale, scale],
                rows,
            )?;
            let residual_id = problem.add_residual(residual)?;
            problem.declare_fixed_variable(
                variable,
                VariableValue::Vec2([target.x, target.y]),
                residual_id,
            )?;
            return Ok(equation_mapping(
                SketchSource::Constraint(constraint_id),
                label,
                source_id,
                residual_id,
            ));
        }
        SketchConstraintKind::FixedCoordinate {
            point,
            axis,
            target,
        } => {
            let point_name = sketch.point_name(point)?;
            let coordinate = match axis {
                CoordinateAxis::X => 0,
                CoordinateAxis::Y => 1,
            };
            let axis_name = match axis {
                CoordinateAxis::X => "x",
                CoordinateAxis::Y => "y",
            };
            let label = format!(
                "constraint {}: fixed {point_name}.{axis_name}",
                constraint.ordinal()
            );
            let source_id = problem.add_source(SourceConstraint::new(&label)?);
            let residual = ResidualBlock::new(
                source_id,
                ResidualCategory::Hard,
                vec![point_variable(point_variables, point)?],
                1,
                vec![scale],
                vec![audit_row(
                    format!("({point_name}.{axis_name} - target) / model_scale"),
                    vec![
                        AuditBinding::new("point", point_name),
                        AuditBinding::new("coordinate", axis_name),
                        AuditBinding::new("target", target.to_string()),
                    ],
                )],
                FixedCoordinateResidual { coordinate, target },
            )?;
            (label, residual)
        }
        SketchConstraintKind::Coincident { first, second } => {
            let first_name = sketch.point_name(first)?;
            let second_name = sketch.point_name(second)?;
            let label = format!(
                "constraint {}: {first_name} coincident with {second_name}",
                constraint.ordinal()
            );
            let source_id = problem.add_source(SourceConstraint::new(&label)?);
            let bindings = pair_bindings(first_name, second_name);
            let residual = ResidualBlock::new(
                source_id,
                ResidualCategory::Hard,
                vec![
                    point_variable(point_variables, first)?,
                    point_variable(point_variables, second)?,
                ],
                2,
                vec![scale, scale],
                vec![
                    audit_row(
                        format!("({second_name}.x - {first_name}.x) / model_scale"),
                        bindings.clone(),
                    ),
                    audit_row(
                        format!("({second_name}.y - {first_name}.y) / model_scale"),
                        bindings,
                    ),
                ],
                CoincidentResidual,
            )?;
            (label, residual)
        }
        SketchConstraintKind::Horizontal { segment } => compile_axis_constraint(
            sketch,
            problem,
            point_variables,
            constraint,
            segment,
            1,
            "horizontal",
            "y",
        )?,
        SketchConstraintKind::Vertical { segment } => compile_axis_constraint(
            sketch,
            problem,
            point_variables,
            constraint,
            segment,
            0,
            "vertical",
            "x",
        )?,
        kind => {
            return compile_curve_constraint(
                sketch,
                problem,
                point_variables,
                circle_radius_variables,
                arc_radius_variables,
                conic_vector_variables,
                conic_scalar_variables,
                latent_variables,
                bound_mappings,
                constraint_id,
                constraint,
                kind,
            );
        }
    };
    let source_id = residual.source();
    let residual_id = problem.add_residual(residual)?;
    Ok(equation_mapping(
        SketchSource::Constraint(constraint_id),
        label,
        source_id,
        residual_id,
    ))
}

#[allow(clippy::too_many_arguments)]
fn compile_axis_constraint(
    sketch: &Sketch,
    problem: &mut Problem,
    point_variables: &[PointVariableMapping],
    constraint: &crate::SketchConstraint,
    segment_id: SegmentId,
    coordinate: usize,
    orientation: &str,
    coordinate_name: &str,
) -> Result<(String, ResidualBlock), SketchError> {
    let segment = sketch
        .segments
        .get(segment_id)
        .ok_or(SketchError::UnknownSegment(segment_id))?;
    let start_name = sketch.point_name(segment.start())?;
    let end_name = sketch.point_name(segment.end())?;
    let label = format!(
        "constraint {}: {} {orientation}",
        constraint.ordinal(),
        segment.label()
    );
    let source_id = problem.add_source(SourceConstraint::new(&label)?);
    let residual = ResidualBlock::new(
        source_id,
        ResidualCategory::Hard,
        vec![
            point_variable(point_variables, segment.start())?,
            point_variable(point_variables, segment.end())?,
        ],
        1,
        vec![sketch.model_scale],
        vec![audit_row(
            format!(
                "({end_name}.{coordinate_name} - {start_name}.{coordinate_name}) / model_scale"
            ),
            vec![
                AuditBinding::new("segment", segment.label()),
                AuditBinding::new("start", start_name),
                AuditBinding::new("end", end_name),
            ],
        )],
        AxisDifferenceResidual { coordinate },
    )?;
    Ok((label, residual))
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn compile_curve_constraint(
    sketch: &Sketch,
    problem: &mut Problem,
    point_variables: &[PointVariableMapping],
    circle_radius_variables: &[CircleRadiusVariableMapping],
    arc_radius_variables: &[ArcRadiusVariableMapping],
    conic_vector_variables: &[ConicVectorVariableMapping],
    conic_scalar_variables: &[ConicScalarVariableMapping],
    latent_variables: &mut Vec<LatentVariableMapping>,
    bound_mappings: &mut Vec<SketchBoundMapping>,
    constraint_id: SketchConstraintId,
    constraint: &crate::SketchConstraint,
    kind: SketchConstraintKind,
) -> Result<SketchSourceMapping, SketchError> {
    let scale = sketch.model_scale;
    let mut incidence = IncidenceBuilder::default();
    let (label, output_dimension, scales, rows, evaluator): (
        String,
        usize,
        Vec<f64>,
        Vec<ResidualRowAudit>,
        Box<dyn geosolve_core::ResidualEvaluator>,
    ) = match kind {
        SketchConstraintKind::PointOnLine {
            point,
            segment,
            domain,
            parameter,
        } => {
            let (start, end, line) = segment_points(sketch, segment)?;
            let point_name = sketch.point_name(point)?;
            let parameter_variable = add_latent(
                problem,
                latent_variables,
                constraint_id,
                LatentVariableRole::LineParameter,
                parameter,
                (domain == crate::LineParameterDomain::BoundedSegment).then_some((0.0, 1.0)),
                bound_mappings,
            )?;
            let evaluator = PointOnLineResidual {
                point: incidence.add(point_variable(point_variables, point)?),
                start: incidence.add(point_variable(point_variables, start)?),
                end: incidence.add(point_variable(point_variables, end)?),
                parameter: incidence.add(parameter_variable),
                domain,
            };
            let bindings = vec![
                AuditBinding::new("point", point_name),
                AuditBinding::new("line", line.label()),
                AuditBinding::new("domain", domain.label()),
                AuditBinding::new("warm-start parameter", parameter.to_string()),
            ];
            (
                format!(
                    "constraint {}: {point_name} on {} ({})",
                    constraint.ordinal(),
                    line.label(),
                    domain.label()
                ),
                2,
                vec![scale, scale],
                vec![
                    audit_row("(P.x - line(t).x) / model_scale".into(), bindings.clone()),
                    audit_row("(P.y - line(t).y) / model_scale".into(), bindings),
                ],
                Box::new(evaluator),
            )
        }
        SketchConstraintKind::PointOnCircle {
            point,
            circle,
            angle,
        } => {
            let circle_value = sketch.circle_value(circle)?;
            let point_name = sketch.point_name(point)?;
            let angle_variable = add_latent(
                problem,
                latent_variables,
                constraint_id,
                LatentVariableRole::CircleAngle,
                angle,
                None,
                bound_mappings,
            )?;
            let evaluator = PointOnCircleResidual {
                point: incidence.add(point_variable(point_variables, point)?),
                center: incidence.add(point_variable(point_variables, circle_value.center())?),
                radius: incidence.add(circle_radius_variable(circle_radius_variables, circle)?),
                angle: incidence.add(angle_variable),
            };
            let bindings = vec![
                AuditBinding::new("point", point_name),
                AuditBinding::new("circle", circle_value.label()),
                AuditBinding::new("warm-start angle", angle.to_string()),
            ];
            (
                format!(
                    "constraint {}: {point_name} on {}",
                    constraint.ordinal(),
                    circle_value.label()
                ),
                2,
                vec![scale, scale],
                vec![
                    audit_row(
                        "(P.x - circle(angle).x) / model_scale".into(),
                        bindings.clone(),
                    ),
                    audit_row("(P.y - circle(angle).y) / model_scale".into(), bindings),
                ],
                Box::new(evaluator),
            )
        }
        SketchConstraintKind::PointOnArc {
            point,
            arc,
            span_parameter,
        } => {
            let arc_value = sketch.arc_value(arc)?;
            let point_name = sketch.point_name(point)?;
            let parameter_variable = add_latent(
                problem,
                latent_variables,
                constraint_id,
                LatentVariableRole::ArcSpanParameter,
                span_parameter,
                Some((0.0, 1.0)),
                bound_mappings,
            )?;
            let evaluator = PointOnArcResidual {
                point: incidence.add(point_variable(point_variables, point)?),
                center: incidence.add(point_variable(point_variables, arc_value.center())?),
                radius: incidence.add(arc_radius_variable(arc_radius_variables, arc)?),
                parameter: incidence.add(parameter_variable),
                start_angle: arc_value.start_angle(),
                signed_sweep: arc_value.signed_sweep(),
            };
            let bindings = vec![
                AuditBinding::new("point", point_name),
                AuditBinding::new("arc", arc_value.label()),
                AuditBinding::new("warm-start span", span_parameter.to_string()),
                AuditBinding::new("sweep", format!("{:?}", arc_value.sweep())),
            ];
            (
                format!(
                    "constraint {}: {point_name} on {} bounded span",
                    constraint.ordinal(),
                    arc_value.label()
                ),
                2,
                vec![scale, scale],
                vec![
                    audit_row("(P.x - arc(u).x) / model_scale".into(), bindings.clone()),
                    audit_row("(P.y - arc(u).y) / model_scale".into(), bindings),
                ],
                Box::new(evaluator),
            )
        }
        SketchConstraintKind::PointOnBezier {
            point,
            bezier,
            parameter,
        } => {
            let curve = sketch
                .bezier(bezier)
                .ok_or(SketchError::UnknownBezier(bezier))?;
            let point_name = sketch.point_name(point)?;
            let parameter_variable = add_latent(
                problem,
                latent_variables,
                constraint_id,
                LatentVariableRole::BezierParameter,
                parameter,
                Some((0.0, 1.0)),
                bound_mappings,
            )?;
            let evaluator = PointOnBezierResidual {
                point: incidence.add(point_variable(point_variables, point)?),
                controls: bezier_incidence(curve, point_variables, &mut incidence)?,
                parameter: incidence.add(parameter_variable),
            };
            let bindings = vec![
                AuditBinding::new("point", point_name),
                AuditBinding::new("Bezier", curve.label()),
                AuditBinding::new("domain", "bounded span [0, 1]"),
                AuditBinding::new("warm-start parameter", parameter.to_string()),
            ];
            (
                format!(
                    "constraint {}: {point_name} on {} bounded span",
                    constraint.ordinal(),
                    curve.label()
                ),
                2,
                vec![scale, scale],
                vec![
                    audit_row("(P.x - Bezier(t).x) / model_scale".into(), bindings.clone()),
                    audit_row("(P.y - Bezier(t).y) / model_scale".into(), bindings),
                ],
                Box::new(evaluator),
            )
        }
        SketchConstraintKind::PointOnCurve { point, contact } => {
            let point_name = sketch.point_name(point)?;
            let curve_label = generic_curve_label(sketch, contact.curve)?;
            let parameter_variable = add_curve_contact_latent(
                sketch,
                problem,
                latent_variables,
                bound_mappings,
                constraint_id,
                LatentVariableRole::CurveParameter,
                contact,
            )?;
            let evaluator = GenericPointOnCurveResidual {
                point: incidence.add(point_variable(point_variables, point)?),
                curve: {
                    let parameter =
                        CurveParameterIncidence::Variable(incidence.add(parameter_variable));
                    generic_curve_incidence(
                        sketch,
                        point_variables,
                        circle_radius_variables,
                        arc_radius_variables,
                        conic_vector_variables,
                        conic_scalar_variables,
                        &mut incidence,
                        contact.curve,
                        parameter,
                    )?
                },
            };
            let bindings = vec![
                AuditBinding::new("point", point_name),
                AuditBinding::new("curve", curve_label),
                AuditBinding::new("warm-start parameter", contact.parameter.to_string()),
                AuditBinding::new("neighborhood", format!("{:?}", contact.neighborhood)),
            ];
            (
                format!(
                    "constraint {}: {point_name} on {curve_label}",
                    constraint.ordinal()
                ),
                2,
                vec![scale, scale],
                vec![
                    audit_row("(P.x - curve(t).x) / model_scale".into(), bindings.clone()),
                    audit_row("(P.y - curve(t).y) / model_scale".into(), bindings),
                ],
                Box::new(evaluator),
            )
        }
        SketchConstraintKind::Parallel { first, second }
        | SketchConstraintKind::Perpendicular { first, second }
        | SketchConstraintKind::EqualSegmentLength { first, second } => {
            let (_, _, first_value) = segment_points(sketch, first)?;
            let (_, _, second_value) = segment_points(sketch, second)?;
            let first_indices = segment_incidence(sketch, point_variables, &mut incidence, first)?;
            let second_indices =
                segment_incidence(sketch, point_variables, &mut incidence, second)?;
            let (equation, name, template, residual_scale, unit) = match kind {
                SketchConstraintKind::Parallel { .. } => (
                    SegmentPairEquation::Parallel,
                    "parallel",
                    "cross(unit_direction(first), unit_direction(second))",
                    1.0,
                    "dimensionless",
                ),
                SketchConstraintKind::Perpendicular { .. } => (
                    SegmentPairEquation::Perpendicular,
                    "perpendicular",
                    "dot(unit_direction(first), unit_direction(second))",
                    1.0,
                    "dimensionless",
                ),
                SketchConstraintKind::EqualSegmentLength { .. } => (
                    SegmentPairEquation::EqualLength,
                    "equal length",
                    "(length(first) - length(second)) / model_scale",
                    scale,
                    "model-unit",
                ),
                _ => unreachable!(),
            };
            (
                format!(
                    "constraint {}: {} and {} {name}",
                    constraint.ordinal(),
                    first_value.label(),
                    second_value.label()
                ),
                1,
                vec![residual_scale],
                vec![audit_row_unit(
                    template.into(),
                    vec![
                        AuditBinding::new("first", first_value.label()),
                        AuditBinding::new("second", second_value.label()),
                    ],
                    unit,
                )],
                Box::new(SegmentPairResidual {
                    first: first_indices,
                    second: second_indices,
                    equation,
                }),
            )
        }
        SketchConstraintKind::EqualCircleRadius { first, second } => {
            let first_value = sketch.circle_value(first)?;
            let second_value = sketch.circle_value(second)?;
            incidence.add(circle_radius_variable(circle_radius_variables, first)?);
            incidence.add(circle_radius_variable(circle_radius_variables, second)?);
            (
                format!(
                    "constraint {}: {} and {} equal radius",
                    constraint.ordinal(),
                    first_value.label(),
                    second_value.label()
                ),
                1,
                vec![scale],
                vec![audit_row(
                    "(radius(first) - radius(second)) / model_scale".into(),
                    vec![
                        AuditBinding::new("first", first_value.label()),
                        AuditBinding::new("second", second_value.label()),
                    ],
                )],
                Box::new(ScalarEqualityResidual),
            )
        }
        SketchConstraintKind::Midpoint { point, segment } => {
            let (start, end, segment_value) = segment_points(sketch, segment)?;
            let point_name = sketch.point_name(point)?;
            let evaluator = MidpointResidual {
                point: incidence.add(point_variable(point_variables, point)?),
                start: incidence.add(point_variable(point_variables, start)?),
                end: incidence.add(point_variable(point_variables, end)?),
            };
            let bindings = vec![
                AuditBinding::new("point", point_name),
                AuditBinding::new("segment", segment_value.label()),
            ];
            (
                format!(
                    "constraint {}: {point_name} midpoint of {}",
                    constraint.ordinal(),
                    segment_value.label()
                ),
                2,
                vec![scale, scale],
                vec![
                    audit_row(
                        "(P.x - (A.x + B.x)/2) / model_scale".into(),
                        bindings.clone(),
                    ),
                    audit_row("(P.y - (A.y + B.y)/2) / model_scale".into(), bindings),
                ],
                Box::new(evaluator),
            )
        }
        SketchConstraintKind::SymmetricAboutLine {
            first,
            second,
            line,
        } => {
            let (line_start, line_end, line_value) = segment_points(sketch, line)?;
            let first_name = sketch.point_name(first)?;
            let second_name = sketch.point_name(second)?;
            let evaluator = SymmetryResidual {
                first: incidence.add(point_variable(point_variables, first)?),
                second: incidence.add(point_variable(point_variables, second)?),
                line_start: incidence.add(point_variable(point_variables, line_start)?),
                line_end: incidence.add(point_variable(point_variables, line_end)?),
            };
            let bindings = vec![
                AuditBinding::new("first", first_name),
                AuditBinding::new("second", second_name),
                AuditBinding::new("line", line_value.label()),
            ];
            (
                format!(
                    "constraint {}: {first_name} and {second_name} symmetric about {}",
                    constraint.ordinal(),
                    line_value.label()
                ),
                2,
                vec![scale, scale],
                vec![
                    audit_row(
                        "dot(unit_line_normal, pair_midpoint - line_start) / model_scale".into(),
                        bindings.clone(),
                    ),
                    audit_row(
                        "dot(unit_line_axis, second - first) / model_scale".into(),
                        bindings,
                    ),
                ],
                Box::new(evaluator),
            )
        }
        SketchConstraintKind::LineCircleTangency {
            line,
            circle,
            domain,
            side,
            line_parameter,
            circle_angle,
        } => {
            let (line_start, line_end, line_value) = segment_points(sketch, line)?;
            let circle_value = sketch.circle_value(circle)?;
            let line_variable = add_latent(
                problem,
                latent_variables,
                constraint_id,
                LatentVariableRole::LineParameter,
                line_parameter,
                (domain == crate::LineParameterDomain::BoundedSegment).then_some((0.0, 1.0)),
                bound_mappings,
            )?;
            let circle_variable = add_latent(
                problem,
                latent_variables,
                constraint_id,
                LatentVariableRole::CircleAngle,
                circle_angle,
                None,
                bound_mappings,
            )?;
            let evaluator = LineCircleTangencyResidual {
                line_start: incidence.add(point_variable(point_variables, line_start)?),
                line_end: incidence.add(point_variable(point_variables, line_end)?),
                center: incidence.add(point_variable(point_variables, circle_value.center())?),
                radius: incidence.add(circle_radius_variable(circle_radius_variables, circle)?),
                line_parameter: incidence.add(line_variable),
                circle_angle: incidence.add(circle_variable),
                domain,
            };
            let bindings = vec![
                AuditBinding::new("line", line_value.label()),
                AuditBinding::new("circle", circle_value.label()),
                AuditBinding::new("domain", domain.label()),
                AuditBinding::new("side", format!("{side:?}")),
                AuditBinding::new("warm-start line parameter", line_parameter.to_string()),
                AuditBinding::new("warm-start circle angle", circle_angle.to_string()),
            ];
            (
                format!(
                    "constraint {}: {} tangent to {} ({side:?}, {})",
                    constraint.ordinal(),
                    line_value.label(),
                    circle_value.label(),
                    domain.label()
                ),
                3,
                vec![scale, scale, 1.0],
                vec![
                    audit_row(
                        "(line(t).x - circle(angle).x) / model_scale".into(),
                        bindings.clone(),
                    ),
                    audit_row(
                        "(line(t).y - circle(angle).y) / model_scale".into(),
                        bindings.clone(),
                    ),
                    audit_row_unit(
                        "dot(unit_line_direction, unit_circle_radial)".into(),
                        bindings,
                        "dimensionless",
                    ),
                ],
                Box::new(evaluator),
            )
        }
        SketchConstraintKind::LineBezierTangency {
            line,
            endpoint,
            bezier,
            bezier_parameter,
            orientation,
        } => {
            let (start, end, line_value) = segment_points(sketch, line)?;
            let curve = sketch
                .bezier(bezier)
                .ok_or(SketchError::UnknownBezier(bezier))?;
            let parameter_variable = add_latent(
                problem,
                latent_variables,
                constraint_id,
                LatentVariableRole::BezierParameter,
                bezier_parameter,
                Some((bezier_parameter, bezier_parameter)),
                bound_mappings,
            )?;
            let evaluator = LineBezierTangencyResidual {
                line: [
                    incidence.add(point_variable(point_variables, start)?),
                    incidence.add(point_variable(point_variables, end)?),
                ],
                endpoint,
                controls: bezier_incidence(curve, point_variables, &mut incidence)?,
                parameter: incidence.add(parameter_variable),
                orientation,
            };
            let bindings = vec![
                AuditBinding::new("line", line_value.label()),
                AuditBinding::new("line endpoint", format!("{endpoint:?}")),
                AuditBinding::new("Bezier", curve.label()),
                AuditBinding::new("warm-start parameter", bezier_parameter.to_string()),
                AuditBinding::new("tangent orientation", format!("{orientation:?}")),
            ];
            (
                format!(
                    "constraint {}: {} endpoint tangent to {}",
                    constraint.ordinal(),
                    line_value.label(),
                    curve.label()
                ),
                3,
                vec![scale, scale, 1.0],
                vec![
                    audit_row(
                        "(line_endpoint.x - Bezier(t).x) / model_scale".into(),
                        bindings.clone(),
                    ),
                    audit_row(
                        "(line_endpoint.y - Bezier(t).y) / model_scale".into(),
                        bindings.clone(),
                    ),
                    audit_row_unit(
                        "cross(unit(line tangent), unit(Bezier tangent))".into(),
                        bindings,
                        "dimensionless",
                    ),
                ],
                Box::new(evaluator),
            )
        }
        SketchConstraintKind::LineCurveTangency {
            line,
            endpoint,
            contact,
            orientation,
        } => {
            let (start, end, line_value) = segment_points(sketch, line)?;
            let curve_label = generic_curve_label(sketch, contact.curve)?;
            let parameter_variable = add_curve_contact_latent(
                sketch,
                problem,
                latent_variables,
                bound_mappings,
                constraint_id,
                LatentVariableRole::CurveParameter,
                contact,
            )?;
            let line_curve = GenericCurveIncidence::Line {
                points: [
                    incidence.add(point_variable(point_variables, start)?),
                    incidence.add(point_variable(point_variables, end)?),
                ],
                parameter: CurveParameterIncidence::Fixed(match endpoint {
                    crate::SegmentEndpoint::Start => 0.0,
                    crate::SegmentEndpoint::End => 1.0,
                }),
                bounded: true,
            };
            let parameter = CurveParameterIncidence::Variable(incidence.add(parameter_variable));
            let contact_curve = generic_curve_incidence(
                sketch,
                point_variables,
                circle_radius_variables,
                arc_radius_variables,
                conic_vector_variables,
                conic_scalar_variables,
                &mut incidence,
                contact.curve,
                parameter,
            )?;
            let evaluator = GenericCurvePairResidual {
                first: line_curve,
                second: contact_curve,
                orientation: Some(orientation),
            };
            let bindings = vec![
                AuditBinding::new("line", line_value.label()),
                AuditBinding::new("line endpoint", format!("{endpoint:?}")),
                AuditBinding::new("curve", curve_label),
                AuditBinding::new("warm-start parameter", contact.parameter.to_string()),
                AuditBinding::new("neighborhood", format!("{:?}", contact.neighborhood)),
                AuditBinding::new("tangent orientation", format!("{orientation:?}")),
            ];
            (
                format!(
                    "constraint {}: {} endpoint tangent to {curve_label}",
                    constraint.ordinal(),
                    line_value.label()
                ),
                3,
                vec![scale, scale, 1.0],
                generic_curve_pair_audit_rows(bindings, true),
                Box::new(evaluator),
            )
        }
        SketchConstraintKind::CurveCurveContact { first, second }
        | SketchConstraintKind::CurveCurveTangency {
            first,
            second,
            orientation: _,
        } => {
            let orientation = match kind {
                SketchConstraintKind::CurveCurveTangency { orientation, .. } => Some(orientation),
                SketchConstraintKind::CurveCurveContact { .. } => None,
                _ => unreachable!(),
            };
            let first_label = generic_curve_label(sketch, first.curve)?;
            let second_label = generic_curve_label(sketch, second.curve)?;
            let first_parameter = add_curve_contact_latent(
                sketch,
                problem,
                latent_variables,
                bound_mappings,
                constraint_id,
                LatentVariableRole::FirstCurveParameter,
                first,
            )?;
            let second_parameter = add_curve_contact_latent(
                sketch,
                problem,
                latent_variables,
                bound_mappings,
                constraint_id,
                LatentVariableRole::SecondCurveParameter,
                second,
            )?;
            let first_parameter = CurveParameterIncidence::Variable(incidence.add(first_parameter));
            let first_curve = generic_curve_incidence(
                sketch,
                point_variables,
                circle_radius_variables,
                arc_radius_variables,
                conic_vector_variables,
                conic_scalar_variables,
                &mut incidence,
                first.curve,
                first_parameter,
            )?;
            let second_parameter =
                CurveParameterIncidence::Variable(incidence.add(second_parameter));
            let second_curve = generic_curve_incidence(
                sketch,
                point_variables,
                circle_radius_variables,
                arc_radius_variables,
                conic_vector_variables,
                conic_scalar_variables,
                &mut incidence,
                second.curve,
                second_parameter,
            )?;
            let tangency = orientation.is_some();
            let evaluator = GenericCurvePairResidual {
                first: first_curve,
                second: second_curve,
                orientation,
            };
            let bindings = vec![
                AuditBinding::new("first curve", first_label),
                AuditBinding::new("second curve", second_label),
                AuditBinding::new("first warm-start parameter", first.parameter.to_string()),
                AuditBinding::new("second warm-start parameter", second.parameter.to_string()),
                AuditBinding::new("first neighborhood", format!("{:?}", first.neighborhood)),
                AuditBinding::new("second neighborhood", format!("{:?}", second.neighborhood)),
                AuditBinding::new("tangent orientation", format!("{orientation:?}")),
            ];
            (
                format!(
                    "constraint {}: {first_label} and {second_label} {}",
                    constraint.ordinal(),
                    if tangency {
                        "tangent contact"
                    } else {
                        "contact"
                    }
                ),
                if tangency { 3 } else { 2 },
                if tangency {
                    vec![scale, scale, 1.0]
                } else {
                    vec![scale, scale]
                },
                generic_curve_pair_audit_rows(bindings, tangency),
                Box::new(evaluator),
            )
        }
        SketchConstraintKind::CircleCircleTangency {
            first,
            second,
            mode,
            center_direction,
        } => {
            let first_value = sketch.circle_value(first)?;
            let second_value = sketch.circle_value(second)?;
            let evaluator = CircleTangencyResidual {
                first_center: incidence.add(point_variable(point_variables, first_value.center())?),
                first_radius: incidence
                    .add(circle_radius_variable(circle_radius_variables, first)?),
                second_center: incidence
                    .add(point_variable(point_variables, second_value.center())?),
                second_radius: incidence
                    .add(circle_radius_variable(circle_radius_variables, second)?),
                mode,
            };
            (
                format!(
                    "constraint {}: {} and {} {mode:?} tangency",
                    constraint.ordinal(),
                    first_value.label(),
                    second_value.label()
                ),
                1,
                vec![scale],
                vec![audit_row(
                    "(center_distance - selected_radius_combination) / model_scale".into(),
                    vec![
                        AuditBinding::new("first", first_value.label()),
                        AuditBinding::new("second", second_value.label()),
                        AuditBinding::new("mode", format!("{mode:?}")),
                        AuditBinding::new(
                            "center_direction",
                            format!("{:?}", center_direction.reference_direction()),
                        ),
                    ],
                )],
                Box::new(evaluator),
            )
        }
        SketchConstraintKind::CircleArcTangency {
            circle,
            arc,
            side,
            arc_span_parameter,
            circle_angle,
        } => {
            let circle_value = sketch.circle_value(circle)?;
            let arc_value = sketch.arc_value(arc)?;
            let circle_variable = add_latent(
                problem,
                latent_variables,
                constraint_id,
                LatentVariableRole::CircleAngle,
                circle_angle,
                None,
                bound_mappings,
            )?;
            let arc_variable = add_latent(
                problem,
                latent_variables,
                constraint_id,
                LatentVariableRole::ArcSpanParameter,
                arc_span_parameter,
                Some((0.0, 1.0)),
                bound_mappings,
            )?;
            let evaluator = CircleArcTangencyResidual {
                circle_center: incidence
                    .add(point_variable(point_variables, circle_value.center())?),
                circle_radius: incidence
                    .add(circle_radius_variable(circle_radius_variables, circle)?),
                arc_center: incidence.add(point_variable(point_variables, arc_value.center())?),
                arc_radius: incidence.add(arc_radius_variable(arc_radius_variables, arc)?),
                circle_angle: incidence.add(circle_variable),
                arc_parameter: incidence.add(arc_variable),
                arc_start_angle: arc_value.start_angle(),
                arc_signed_sweep: arc_value.signed_sweep(),
            };
            let bindings = vec![
                AuditBinding::new("circle", circle_value.label()),
                AuditBinding::new("arc", arc_value.label()),
                AuditBinding::new("side", format!("{side:?}")),
                AuditBinding::new("domain", "bounded-arc domain [0, 1]"),
                AuditBinding::new("sweep", format!("{:?}", arc_value.sweep())),
                AuditBinding::new("warm-start circle angle", circle_angle.to_string()),
                AuditBinding::new("warm-start arc span", arc_span_parameter.to_string()),
            ];
            (
                format!(
                    "constraint {}: {} tangent to {} bounded span ({side:?})",
                    constraint.ordinal(),
                    circle_value.label(),
                    arc_value.label()
                ),
                3,
                vec![scale, scale, 1.0],
                vec![
                    audit_row(
                        "(circle(angle).x - arc(u).x) / model_scale".into(),
                        bindings.clone(),
                    ),
                    audit_row(
                        "(circle(angle).y - arc(u).y) / model_scale".into(),
                        bindings.clone(),
                    ),
                    audit_row_unit(
                        "cross(unit_tangent(circle, angle), unit_tangent(arc, u))".into(),
                        bindings,
                        "dimensionless",
                    ),
                ],
                Box::new(evaluator),
            )
        }
        _ => unreachable!("M5 constraint reached M7 compiler"),
    };

    let source_id = problem.add_source(SourceConstraint::new(&label)?);
    let residual_id = problem.add_residual(ResidualBlock::new(
        source_id,
        ResidualCategory::Hard,
        incidence.variables,
        output_dimension,
        scales,
        rows,
        BoxedEvaluator(evaluator),
    )?)?;
    Ok(equation_mapping(
        SketchSource::Constraint(constraint_id),
        label,
        source_id,
        residual_id,
    ))
}

#[derive(Clone, Debug)]
struct BoxedEvaluator(Box<dyn geosolve_core::ResidualEvaluator>);

impl geosolve_core::ResidualEvaluator for BoxedEvaluator {
    fn evaluate(
        &self,
        variables: &[VariableValue],
    ) -> Result<Vec<f64>, geosolve_core::EvaluationError> {
        self.0.evaluate(variables)
    }

    fn jacobian(
        &self,
        variables: &[VariableValue],
    ) -> Result<Vec<geosolve_core::LocalJacobian>, geosolve_core::EvaluationError> {
        self.0.jacobian(variables)
    }
}

fn compile_dimension(
    sketch: &Sketch,
    problem: &mut Problem,
    point_variables: &[PointVariableMapping],
    circle_radius_variables: &[CircleRadiusVariableMapping],
    arc_radius_variables: &[ArcRadiusVariableMapping],
    dimension_id: SketchDimensionId,
    dimension: &crate::SketchDimension,
) -> Result<SketchSourceMapping, SketchError> {
    let (first, second, target, subject) = match dimension.kind() {
        DimensionKind::PointDistance {
            first,
            second,
            target,
        } => (
            first,
            second,
            target,
            format!(
                "distance {}-{}",
                sketch.point_name(first)?,
                sketch.point_name(second)?
            ),
        ),
        DimensionKind::SegmentLength { segment, target } => {
            let segment_value = sketch
                .segments
                .get(segment)
                .ok_or(SketchError::UnknownSegment(segment))?;
            (
                segment_value.start(),
                segment_value.end(),
                target,
                format!("length {}", segment_value.label()),
            )
        }
        kind => {
            return compile_curve_dimension(
                sketch,
                problem,
                point_variables,
                circle_radius_variables,
                arc_radius_variables,
                dimension_id,
                dimension,
                kind,
            );
        }
    };
    if dimension.mode() == DimensionMode::Reference {
        return Ok(SketchSourceMapping {
            source: SketchSource::Dimension(dimension_id),
            source_label: format!(
                "dimension {}: reference measurement of {subject}",
                dimension.ordinal()
            ),
            core_source_id: None,
            residual_ids: Vec::new(),
        });
    }

    let label = format!(
        "dimension {}: {subject} = {target} (driving)",
        dimension.ordinal()
    );
    let first_name = sketch.point_name(first)?;
    let second_name = sketch.point_name(second)?;
    let source_id = problem.add_source(SourceConstraint::new(&label)?);
    let residual_id = problem.add_residual(ResidualBlock::new(
        source_id,
        ResidualCategory::Hard,
        vec![
            point_variable(point_variables, first)?,
            point_variable(point_variables, second)?,
        ],
        1,
        vec![sketch.model_scale],
        vec![audit_row(
            format!("(distance({first_name}, {second_name}) - target) / model_scale"),
            vec![
                AuditBinding::new("first", first_name),
                AuditBinding::new("second", second_name),
                AuditBinding::new("target", target.to_string()),
            ],
        )],
        DistanceResidual { target },
    )?)?;
    Ok(equation_mapping(
        SketchSource::Dimension(dimension_id),
        label,
        source_id,
        residual_id,
    ))
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn compile_curve_dimension(
    sketch: &Sketch,
    problem: &mut Problem,
    point_variables: &[PointVariableMapping],
    circle_radius_variables: &[CircleRadiusVariableMapping],
    arc_radius_variables: &[ArcRadiusVariableMapping],
    dimension_id: SketchDimensionId,
    dimension: &crate::SketchDimension,
    kind: DimensionKind,
) -> Result<SketchSourceMapping, SketchError> {
    let (subject, target) = match kind {
        DimensionKind::CircleRadius { circle, target } => (
            format!("radius {}", sketch.circle_value(circle)?.label()),
            target,
        ),
        DimensionKind::CircleDiameter { circle, target } => (
            format!("diameter {}", sketch.circle_value(circle)?.label()),
            target,
        ),
        DimensionKind::ArcRadius { arc, target } => {
            (format!("radius {}", sketch.arc_value(arc)?.label()), target)
        }
        DimensionKind::ArcDiameter { arc, target } => (
            format!("diameter {}", sketch.arc_value(arc)?.label()),
            target,
        ),
        DimensionKind::OrientedAngle {
            first,
            second,
            target,
            orientation,
        } => {
            let (_, _, first_value) = segment_points(sketch, first)?;
            let (_, _, second_value) = segment_points(sketch, second)?;
            (
                format!(
                    "{orientation:?} angle {} to {}",
                    first_value.label(),
                    second_value.label()
                ),
                target,
            )
        }
        _ => unreachable!("M5 dimension reached M7 compiler"),
    };
    if dimension.mode() == DimensionMode::Reference {
        return Ok(SketchSourceMapping {
            source: SketchSource::Dimension(dimension_id),
            source_label: format!(
                "dimension {}: reference measurement of {subject}",
                dimension.ordinal()
            ),
            core_source_id: None,
            residual_ids: Vec::new(),
        });
    }

    let label = format!(
        "dimension {}: {subject} = {target} (driving)",
        dimension.ordinal()
    );
    let source_id = problem.add_source(SourceConstraint::new(&label)?);
    let (variables, scale, row, evaluator): (
        Vec<VariableId>,
        f64,
        ResidualRowAudit,
        Box<dyn geosolve_core::ResidualEvaluator>,
    ) = match kind {
        DimensionKind::CircleRadius { circle, target } => (
            vec![circle_radius_variable(circle_radius_variables, circle)?],
            sketch.model_scale,
            audit_row(
                "(circle.radius - target) / model_scale".into(),
                vec![
                    AuditBinding::new("circle", sketch.circle_value(circle)?.label()),
                    AuditBinding::new("target", target.to_string()),
                ],
            ),
            Box::new(ScalarTargetResidual {
                target,
                multiplier: 1.0,
            }),
        ),
        DimensionKind::CircleDiameter { circle, target } => (
            vec![circle_radius_variable(circle_radius_variables, circle)?],
            sketch.model_scale,
            audit_row(
                "(2 * circle.radius - target) / model_scale".into(),
                vec![
                    AuditBinding::new("circle", sketch.circle_value(circle)?.label()),
                    AuditBinding::new("target", target.to_string()),
                ],
            ),
            Box::new(ScalarTargetResidual {
                target,
                multiplier: 2.0,
            }),
        ),
        DimensionKind::ArcRadius { arc, target } => (
            vec![arc_radius_variable(arc_radius_variables, arc)?],
            sketch.model_scale,
            audit_row(
                "(arc.radius - target) / model_scale".into(),
                vec![
                    AuditBinding::new("arc", sketch.arc_value(arc)?.label()),
                    AuditBinding::new("target", target.to_string()),
                ],
            ),
            Box::new(ScalarTargetResidual {
                target,
                multiplier: 1.0,
            }),
        ),
        DimensionKind::ArcDiameter { arc, target } => (
            vec![arc_radius_variable(arc_radius_variables, arc)?],
            sketch.model_scale,
            audit_row(
                "(2 * arc.radius - target) / model_scale".into(),
                vec![
                    AuditBinding::new("arc", sketch.arc_value(arc)?.label()),
                    AuditBinding::new("target", target.to_string()),
                ],
            ),
            Box::new(ScalarTargetResidual {
                target,
                multiplier: 2.0,
            }),
        ),
        DimensionKind::OrientedAngle {
            first,
            second,
            target,
            orientation,
        } => {
            let mut incidence = IncidenceBuilder::default();
            let first_indices = segment_incidence(sketch, point_variables, &mut incidence, first)?;
            let second_indices =
                segment_incidence(sketch, point_variables, &mut incidence, second)?;
            let (_, _, first_value) = segment_points(sketch, first)?;
            let (_, _, second_value) = segment_points(sketch, second)?;
            (
                incidence.variables,
                1.0,
                audit_row_unit(
                    "unwrap(oriented_angle(first, second), target) - target".into(),
                    vec![
                        AuditBinding::new("first", first_value.label()),
                        AuditBinding::new("second", second_value.label()),
                        AuditBinding::new("orientation", format!("{orientation:?}")),
                        AuditBinding::new("target", target.to_string()),
                    ],
                    "radian",
                ),
                Box::new(OrientedAngleResidual {
                    first: first_indices,
                    second: second_indices,
                    target,
                    orientation,
                }),
            )
        }
        _ => unreachable!("M5 dimension reached M7 compiler"),
    };
    let residual_id = problem.add_residual(ResidualBlock::new(
        source_id,
        ResidualCategory::Hard,
        variables,
        1,
        vec![scale],
        vec![row],
        BoxedEvaluator(evaluator),
    )?)?;
    Ok(equation_mapping(
        SketchSource::Dimension(dimension_id),
        label,
        source_id,
        residual_id,
    ))
}

#[allow(clippy::too_many_arguments)]
fn compile_point_target(
    sketch: &Sketch,
    problem: &mut Problem,
    point_variables: &[PointVariableMapping],
    source: SketchSource,
    point: PointId,
    target: Point2<f64>,
    category: ResidualCategory,
    label: String,
) -> Result<SketchSourceMapping, SketchError> {
    let point_name = sketch.point_name(point)?;
    let source_id = problem.add_source(SourceConstraint::new(&label)?);
    let residual_id = problem.add_residual(ResidualBlock::new(
        source_id,
        category,
        vec![point_variable(point_variables, point)?],
        2,
        vec![sketch.model_scale, sketch.model_scale],
        vec![
            audit_row(
                format!("({point_name}.x - target.x) / model_scale"),
                point_bindings(point_name, target),
            ),
            audit_row(
                format!("({point_name}.y - target.y) / model_scale"),
                point_bindings(point_name, target),
            ),
        ],
        PointTargetResidual {
            target: [target.x, target.y],
        },
    )?)?;
    Ok(equation_mapping(source, label, source_id, residual_id))
}

fn equation_mapping(
    source: SketchSource,
    source_label: String,
    core_source_id: SourceConstraintId,
    residual_id: ResidualId,
) -> SketchSourceMapping {
    SketchSourceMapping {
        source,
        source_label,
        core_source_id: Some(core_source_id),
        residual_ids: vec![residual_id],
    }
}

fn point_variable(
    mappings: &[PointVariableMapping],
    point: PointId,
) -> Result<VariableId, SketchError> {
    mappings
        .iter()
        .find_map(|mapping| (mapping.point_id == point).then_some(mapping.variable_id))
        .ok_or(SketchError::UnknownPoint(point))
}

fn audit_row(template: String, bindings: Vec<AuditBinding>) -> ResidualRowAudit {
    ResidualRowAudit::new(template, bindings, "model-unit")
}

fn audit_row_unit(template: String, bindings: Vec<AuditBinding>, unit: &str) -> ResidualRowAudit {
    ResidualRowAudit::new(template, bindings, unit)
}

fn point_bindings(point_name: &str, target: Point2<f64>) -> Vec<AuditBinding> {
    vec![
        AuditBinding::new("point", point_name),
        AuditBinding::new("target", format!("({}, {})", target.x, target.y)),
    ]
}

fn pair_bindings(first_name: &str, second_name: &str) -> Vec<AuditBinding> {
    vec![
        AuditBinding::new("first", first_name),
        AuditBinding::new("second", second_name),
    ]
}

fn independent_hard_residual_metrics(
    problem: &Problem,
) -> Result<(f64, f64, AuditSnapshot), geosolve_core::CoreError> {
    let audit = problem.audit_snapshot()?;
    let mut maximum = 0.0_f64;
    let mut squared_norm = 0.0_f64;
    for row in audit
        .sources
        .iter()
        .flat_map(|source| &source.rows)
        .filter(|row| row.category == ResidualCategory::Hard)
    {
        if row.evaluation_status != AuditEvaluationStatus::Evaluated
            || !row.normalized_residual.is_finite()
        {
            return Err(geosolve_core::CoreError::NonFiniteValue {
                context: "sketch independent hard validation",
                index: row.row_in_block,
                value: row.normalized_residual,
            });
        }
        maximum = maximum.max(row.normalized_residual.abs());
        squared_norm += row.normalized_residual * row.normalized_residual;
        if !squared_norm.is_finite() {
            return Err(geosolve_core::CoreError::NonFiniteValue {
                context: "sketch independent hard validation norm",
                index: row.row_in_block,
                value: squared_norm,
            });
        }
    }
    Ok((maximum, squared_norm.sqrt(), audit))
}

fn merge_conflicting_annotations(retained: &mut AuditSnapshot, attempted: &AuditSnapshot) {
    for retained_source in &mut retained.sources {
        let Some(attempted_source) = attempted
            .sources
            .iter()
            .find(|source| source.source_id == retained_source.source_id)
        else {
            continue;
        };
        retained_source.annotations.conflicting |= attempted_source.annotations.conflicting;
        for retained_row in &mut retained_source.rows {
            if attempted_source.rows.iter().any(|row| {
                row.residual_id == retained_row.residual_id
                    && row.row_in_block == retained_row.row_in_block
                    && row.annotations.conflicting
            }) {
                retained_row.annotations.conflicting = true;
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CircleArcRadiusValidation {
    Valid,
    Mismatch,
    AmbiguousScale,
}

fn circle_arc_radius_validation(
    actual_radius: f64,
    derived_radius: f64,
    center_distance: f64,
    arc_radius: f64,
    side: crate::ArcCircleTangencySide,
) -> CircleArcRadiusValidation {
    if !actual_radius.is_finite()
        || !derived_radius.is_finite()
        || actual_radius <= 0.0
        || derived_radius <= 0.0
    {
        return CircleArcRadiusValidation::Mismatch;
    }
    let radius_scale = actual_radius.abs().max(derived_radius.abs());
    let allowed_error = CIRCLE_ARC_TANGENCY_RADIUS_RELATIVE_TOLERANCE * radius_scale;
    let supporting_scale = center_distance.abs().max(arc_radius.abs());
    let floating_uncertainty =
        CIRCLE_ARC_TANGENCY_SCALE_UNCERTAINTY_MULTIPLIER * f64::EPSILON * supporting_scale;
    if !allowed_error.is_finite()
        || !floating_uncertainty.is_finite()
        || floating_uncertainty > allowed_error
    {
        return CircleArcRadiusValidation::AmbiguousScale;
    }
    let expected_distance = match side {
        crate::ArcCircleTangencySide::OutsideArc => arc_radius + actual_radius,
        crate::ArcCircleTangencySide::InsideArc => arc_radius - actual_radius,
    };
    if !expected_distance.is_finite() || expected_distance <= 0.0 {
        return CircleArcRadiusValidation::Mismatch;
    }
    if (center_distance - expected_distance).abs() <= allowed_error {
        CircleArcRadiusValidation::Valid
    } else {
        CircleArcRadiusValidation::Mismatch
    }
}

fn normalized_direction(x: f64, y: f64) -> Option<[f64; 2]> {
    let norm = x.hypot(y);
    (x.is_finite() && y.is_finite() && norm.is_finite() && norm > 0.0)
        .then_some([x / norm, y / norm])
}

fn projected_line_parameter(
    start: Point2<f64>,
    end: Point2<f64>,
    point: Point2<f64>,
) -> Option<f64> {
    let direction = end - start;
    let squared_length = direction.dot(&direction);
    let parameter = direction.dot(&(point - start)) / squared_length;
    (squared_length.is_finite() && squared_length > 0.0 && parameter.is_finite())
        .then_some(parameter)
}

fn directions_match(first: [f64; 2], second: [f64; 2]) -> bool {
    let dot = first[0] * second[0] + first[1] * second[1];
    let cross = first[0] * second[1] - first[1] * second[0];
    dot.is_finite()
        && cross.is_finite()
        && dot >= 1.0 - CIRCLE_ARC_TANGENCY_DIRECTION_TOLERANCE
        && cross.abs() <= CIRCLE_ARC_TANGENCY_DIRECTION_TOLERANCE
}

#[derive(Default)]
struct IncidenceBuilder {
    variables: Vec<VariableId>,
}

impl IncidenceBuilder {
    fn add(&mut self, variable: VariableId) -> usize {
        if let Some(index) = self
            .variables
            .iter()
            .position(|candidate| *candidate == variable)
        {
            index
        } else {
            let index = self.variables.len();
            self.variables.push(variable);
            index
        }
    }
}

fn segment_incidence(
    sketch: &Sketch,
    point_variables: &[PointVariableMapping],
    incidence: &mut IncidenceBuilder,
    segment: SegmentId,
) -> Result<[usize; 2], SketchError> {
    let (start, end) = sketch.segment_endpoints(segment)?;
    Ok([
        incidence.add(point_variable(point_variables, start)?),
        incidence.add(point_variable(point_variables, end)?),
    ])
}

fn bezier_incidence(
    curve: &crate::BezierCurve,
    point_variables: &[PointVariableMapping],
    incidence: &mut IncidenceBuilder,
) -> Result<BezierIncidence, SketchError> {
    Ok(match curve.kind() {
        crate::BezierKind::Quadratic {
            controls: [first, second, third],
        } => BezierIncidence::Quadratic([
            incidence.add(point_variable(point_variables, first)?),
            incidence.add(point_variable(point_variables, second)?),
            incidence.add(point_variable(point_variables, third)?),
        ]),
        crate::BezierKind::Cubic {
            controls: [first, second, third, fourth],
        } => BezierIncidence::Cubic([
            incidence.add(point_variable(point_variables, first)?),
            incidence.add(point_variable(point_variables, second)?),
            incidence.add(point_variable(point_variables, third)?),
            incidence.add(point_variable(point_variables, fourth)?),
        ]),
    })
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn generic_curve_incidence(
    sketch: &Sketch,
    point_variables: &[PointVariableMapping],
    circle_radius_variables: &[CircleRadiusVariableMapping],
    arc_radius_variables: &[ArcRadiusVariableMapping],
    conic_vector_variables: &[ConicVectorVariableMapping],
    conic_scalar_variables: &[ConicScalarVariableMapping],
    incidence: &mut IncidenceBuilder,
    curve: SketchCurve,
    parameter: CurveParameterIncidence,
) -> Result<GenericCurveIncidence, SketchError> {
    match curve {
        SketchCurve::Line { segment, domain } => {
            let (start, end, _) = segment_points(sketch, segment)?;
            Ok(GenericCurveIncidence::Line {
                points: [
                    incidence.add(point_variable(point_variables, start)?),
                    incidence.add(point_variable(point_variables, end)?),
                ],
                parameter,
                bounded: domain == crate::LineParameterDomain::BoundedSegment,
            })
        }
        SketchCurve::Circle(circle) => {
            let value = sketch.circle_value(circle)?;
            Ok(GenericCurveIncidence::Circle {
                center: incidence.add(point_variable(point_variables, value.center())?),
                radius: incidence.add(circle_radius_variable(circle_radius_variables, circle)?),
                parameter,
            })
        }
        SketchCurve::Arc(arc) => {
            let value = sketch.arc_value(arc)?;
            Ok(GenericCurveIncidence::Arc {
                center: incidence.add(point_variable(point_variables, value.center())?),
                radius: incidence.add(arc_radius_variable(arc_radius_variables, arc)?),
                start_angle: value.start_angle(),
                signed_sweep: value.signed_sweep(),
                parameter,
            })
        }
        SketchCurve::Bezier(bezier) => {
            let value = sketch
                .bezier(bezier)
                .ok_or(SketchError::UnknownBezier(bezier))?;
            Ok(match bezier_incidence(value, point_variables, incidence)? {
                BezierIncidence::Quadratic(controls) => GenericCurveIncidence::QuadraticBezier {
                    controls,
                    parameter,
                },
                BezierIncidence::Cubic(controls) => GenericCurveIncidence::CubicBezier {
                    controls,
                    parameter,
                },
            })
        }
        SketchCurve::Conic(conic) => {
            let value = sketch.conic_value(conic)?;
            Ok(match value.kind() {
                crate::ConicKind::Ellipse {
                    center,
                    major_axis_point,
                    ..
                } => GenericCurveIncidence::Ellipse {
                    center: incidence.add(point_variable(point_variables, center)?),
                    major_axis_point: incidence
                        .add(point_variable(point_variables, major_axis_point)?),
                    minor_axis_ratio: incidence.add(conic_scalar_variable(
                        conic_scalar_variables,
                        conic,
                        ConicScalarRole::MinorAxisRatio,
                    )?),
                    parameter,
                },
                crate::ConicKind::EllipticalArc {
                    center,
                    major_axis_point,
                    start_angle,
                    signed_sweep,
                    ..
                } => GenericCurveIncidence::EllipticalArc {
                    center: incidence.add(point_variable(point_variables, center)?),
                    major_axis_point: incidence
                        .add(point_variable(point_variables, major_axis_point)?),
                    minor_axis_ratio: incidence.add(conic_scalar_variable(
                        conic_scalar_variables,
                        conic,
                        ConicScalarRole::MinorAxisRatio,
                    )?),
                    start_angle,
                    signed_sweep,
                    parameter,
                },
                crate::ConicKind::RationalQuadratic { start, end, .. } => {
                    GenericCurveIncidence::RationalQuadratic {
                        start: incidence.add(point_variable(point_variables, start)?),
                        weighted_middle: incidence.add(conic_vector_variable(
                            conic_vector_variables,
                            conic,
                            ConicVectorRole::WeightedMiddle,
                        )?),
                        middle_weight: incidence.add(conic_scalar_variable(
                            conic_scalar_variables,
                            conic,
                            ConicScalarRole::MiddleWeight,
                        )?),
                        end: incidence.add(point_variable(point_variables, end)?),
                        parameter,
                    }
                }
                crate::ConicKind::ParabolaSegment {
                    vertex,
                    focus,
                    trim,
                } => GenericCurveIncidence::ParabolaSegment {
                    vertex: incidence.add(point_variable(point_variables, vertex)?),
                    focus: incidence.add(point_variable(point_variables, focus)?),
                    trim,
                    parameter,
                },
                crate::ConicKind::HyperbolaSegment {
                    center,
                    transverse_axis_point,
                    branch,
                    trim,
                    ..
                } => GenericCurveIncidence::HyperbolaSegment {
                    center: incidence.add(point_variable(point_variables, center)?),
                    transverse_axis_point: incidence
                        .add(point_variable(point_variables, transverse_axis_point)?),
                    semi_conjugate: incidence.add(conic_scalar_variable(
                        conic_scalar_variables,
                        conic,
                        ConicScalarRole::SemiConjugate,
                    )?),
                    branch,
                    trim,
                    parameter,
                },
            })
        }
    }
}

fn add_curve_contact_latent(
    sketch: &Sketch,
    problem: &mut Problem,
    mappings: &mut Vec<LatentVariableMapping>,
    bound_mappings: &mut Vec<SketchBoundMapping>,
    constraint_id: SketchConstraintId,
    role: LatentVariableRole,
    contact: SketchCurveContact,
) -> Result<VariableId, SketchError> {
    let bounded = generic_curve_is_bounded(sketch, contact.curve);
    let bounds = if bounded {
        Some(match contact.neighborhood {
            CurveContactNeighborhood::Start => (0.0, 0.0),
            CurveContactNeighborhood::End => (1.0, 1.0),
            CurveContactNeighborhood::Interior => (0.0, 1.0),
            CurveContactNeighborhood::Local { lower, upper } => (lower, upper),
        })
    } else {
        None
    };
    add_latent(
        problem,
        mappings,
        constraint_id,
        role,
        contact.parameter,
        bounds,
        bound_mappings,
    )
}

fn generic_curve_label(sketch: &Sketch, curve: SketchCurve) -> Result<&str, SketchError> {
    match curve {
        SketchCurve::Line { segment, .. } => sketch
            .segment(segment)
            .map(crate::LineSegment::label)
            .ok_or(SketchError::UnknownSegment(segment)),
        SketchCurve::Circle(circle) => sketch.circle_value(circle).map(crate::Circle::label),
        SketchCurve::Arc(arc) => sketch.arc_value(arc).map(crate::CircularArc::label),
        SketchCurve::Bezier(bezier) => sketch
            .bezier(bezier)
            .map(crate::BezierCurve::label)
            .ok_or(SketchError::UnknownBezier(bezier)),
        SketchCurve::Conic(conic) => sketch.conic_value(conic).map(crate::ConicCurve::label),
    }
}

fn generic_curve_pair_audit_rows(
    bindings: Vec<AuditBinding>,
    tangency: bool,
) -> Vec<ResidualRowAudit> {
    let mut rows = vec![
        audit_row(
            "(first_curve(t1).x - second_curve(t2).x) / model_scale".into(),
            bindings.clone(),
        ),
        audit_row(
            "(first_curve(t1).y - second_curve(t2).y) / model_scale".into(),
            bindings.clone(),
        ),
    ];
    if tangency {
        rows.push(audit_row_unit(
            "cross(unit(first_curve'(t1)), unit(second_curve'(t2)))".into(),
            bindings,
            "dimensionless",
        ));
    }
    rows
}

fn add_latent(
    problem: &mut Problem,
    mappings: &mut Vec<LatentVariableMapping>,
    constraint_id: SketchConstraintId,
    role: LatentVariableRole,
    value: f64,
    bounds: Option<(f64, f64)>,
    bound_mappings: &mut Vec<SketchBoundMapping>,
) -> Result<VariableId, SketchError> {
    validate_point(Point2::new(value, 0.0), "latent parameter")?;
    let variable_id = problem.add_variable(VariableBlock::scalar(value, 1.0)?);
    mappings.push(LatentVariableMapping {
        constraint_id,
        role,
        variable_id,
    });
    if let Some((lower, upper)) = bounds {
        let bound_id = problem.add_bound(CoordinateBound::new(
            variable_id,
            0,
            Some(lower),
            Some(upper),
            format!("bounded {role:?} for constraint {constraint_id:?}"),
        )?)?;
        bound_mappings.push(SketchBoundMapping {
            bound: SketchBound::Contact {
                constraint_id,
                role,
            },
            bound_id,
        });
    }
    Ok(variable_id)
}

fn circle_radius_variable(
    mappings: &[CircleRadiusVariableMapping],
    circle: CircleId,
) -> Result<VariableId, SketchError> {
    mappings
        .iter()
        .find_map(|mapping| (mapping.circle_id == circle).then_some(mapping.variable_id))
        .ok_or(SketchError::UnknownCircle(circle))
}

fn arc_radius_variable(
    mappings: &[ArcRadiusVariableMapping],
    arc: ArcId,
) -> Result<VariableId, SketchError> {
    mappings
        .iter()
        .find_map(|mapping| (mapping.arc_id == arc).then_some(mapping.variable_id))
        .ok_or(SketchError::UnknownArc(arc))
}

fn conic_scalar_variable(
    mappings: &[ConicScalarVariableMapping],
    conic: ConicId,
    role: ConicScalarRole,
) -> Result<VariableId, SketchError> {
    mappings
        .iter()
        .find_map(|mapping| {
            (mapping.conic_id == conic && mapping.role == role).then_some(mapping.variable_id)
        })
        .ok_or(SketchError::InvalidConicScalarRole(conic))
}

fn conic_vector_variable(
    mappings: &[ConicVectorVariableMapping],
    conic: ConicId,
    role: ConicVectorRole,
) -> Result<VariableId, SketchError> {
    mappings
        .iter()
        .find_map(|mapping| {
            (mapping.conic_id == conic && mapping.role == role).then_some(mapping.variable_id)
        })
        .ok_or(SketchError::InvalidConicScalarRole(conic))
}

fn conic_scalar_value(
    problem: &Problem,
    mappings: &[ConicScalarVariableMapping],
    conic: ConicId,
    role: ConicScalarRole,
) -> Result<f64, SketchError> {
    scalar_variable(problem, conic_scalar_variable(mappings, conic, role)?)
}

fn conic_vector_value(
    problem: &Problem,
    mappings: &[ConicVectorVariableMapping],
    conic: ConicId,
    role: ConicVectorRole,
) -> Result<Vector2<f64>, SketchError> {
    let variable = conic_vector_variable(mappings, conic, role)?;
    let block = problem
        .variable(variable)
        .ok_or(geosolve_core::CoreError::UnknownVariable(variable))?;
    let VariableValue::Vec2([x, y]) = block.value() else {
        return Err(geosolve_core::CoreError::VariableKindMismatch {
            expected: geosolve_core::VariableKind::Vec2,
            actual: block.kind(),
        }
        .into());
    };
    let value = Vector2::new(x, y);
    if value.iter().all(|component| component.is_finite()) {
        Ok(value)
    } else {
        Err(SketchError::InvalidConic(
            geosolve_geometry::ConicDefinitionError::NonFiniteVector,
        ))
    }
}

fn scalar_variable(problem: &Problem, variable: VariableId) -> Result<f64, SketchError> {
    let block = problem
        .variable(variable)
        .ok_or(geosolve_core::CoreError::UnknownVariable(variable))?;
    let VariableValue::Scalar(value) = block.value() else {
        return Err(geosolve_core::CoreError::VariableKindMismatch {
            expected: geosolve_core::VariableKind::Scalar,
            actual: block.kind(),
        }
        .into());
    };
    if value.is_finite() {
        Ok(value)
    } else {
        Err(SketchError::NonFiniteValue {
            context: "solved scalar",
            value,
        })
    }
}

fn solved_point(points: &[SolvedPoint], point: PointId) -> Result<Point2<f64>, SketchError> {
    points
        .iter()
        .find_map(|candidate| (candidate.point_id == point).then_some(candidate.position))
        .ok_or(SketchError::UnknownPoint(point))
}

fn solved_conic_from_runtime(
    sketch: &Sketch,
    conic_id: ConicId,
    conic: &crate::ConicCurve,
) -> Result<SolvedConic, SketchError> {
    let point = |id| sketch.point_position(id);
    let kind = match conic.kind() {
        crate::ConicKind::Ellipse {
            center,
            major_axis_point,
            minor_axis_ratio,
        } => SolvedConicKind::Ellipse {
            center: point(center)?,
            major_axis_point: point(major_axis_point)?,
            minor_axis_ratio,
        },
        crate::ConicKind::EllipticalArc {
            center,
            major_axis_point,
            minor_axis_ratio,
            start_angle,
            signed_sweep,
        } => SolvedConicKind::EllipticalArc {
            center: point(center)?,
            major_axis_point: point(major_axis_point)?,
            minor_axis_ratio,
            start_angle,
            signed_sweep,
        },
        crate::ConicKind::RationalQuadratic {
            start,
            weighted_middle,
            middle_weight,
            end,
        } => SolvedConicKind::RationalQuadratic {
            start: point(start)?,
            weighted_middle,
            middle_weight,
            end: point(end)?,
        },
        crate::ConicKind::ParabolaSegment {
            vertex,
            focus,
            trim,
        } => SolvedConicKind::ParabolaSegment {
            vertex: point(vertex)?,
            focus: point(focus)?,
            trim,
        },
        crate::ConicKind::HyperbolaSegment {
            center,
            transverse_axis_point,
            semi_conjugate,
            branch,
            trim,
        } => SolvedConicKind::HyperbolaSegment {
            center: point(center)?,
            transverse_axis_point: point(transverse_axis_point)?,
            semi_conjugate,
            branch,
            trim,
        },
    };
    Ok(SolvedConic { conic_id, kind })
}

fn solved_conic_geometry(kind: SolvedConicKind) -> Result<crate::ConicGeometry, SketchError> {
    use geosolve_geometry::{
        Ellipse2, EllipticalArc2, HyperbolaSegment2, ParabolaSegment2,
        RationalQuadraticConicSegment2, UnitDirection2,
    };

    match kind {
        SolvedConicKind::Ellipse {
            center,
            major_axis_point,
            minor_axis_ratio,
        } => {
            crate::conics::validate_minor_axis_ratio(minor_axis_ratio)?;
            let axis = major_axis_point - center;
            let semi_major = axis.norm();
            let direction = UnitDirection2::try_new(axis).map_err(SketchError::InvalidConic)?;
            Ok(crate::ConicGeometry::Ellipse(
                Ellipse2::try_new(center, direction, semi_major, semi_major * minor_axis_ratio)
                    .map_err(SketchError::InvalidConic)?,
            ))
        }
        SolvedConicKind::EllipticalArc {
            center,
            major_axis_point,
            minor_axis_ratio,
            start_angle,
            signed_sweep,
        } => {
            crate::conics::validate_minor_axis_ratio(minor_axis_ratio)?;
            let axis = major_axis_point - center;
            let semi_major = axis.norm();
            let direction = UnitDirection2::try_new(axis).map_err(SketchError::InvalidConic)?;
            let ellipse =
                Ellipse2::try_new(center, direction, semi_major, semi_major * minor_axis_ratio)
                    .map_err(SketchError::InvalidConic)?;
            Ok(crate::ConicGeometry::EllipticalArc(
                EllipticalArc2::try_new(ellipse, start_angle, signed_sweep)
                    .map_err(SketchError::InvalidConic)?,
            ))
        }
        SolvedConicKind::RationalQuadratic {
            start,
            weighted_middle,
            middle_weight,
            end,
        } => Ok(crate::ConicGeometry::RationalQuadratic(
            RationalQuadraticConicSegment2::try_new(start, weighted_middle, middle_weight, end)
                .map_err(SketchError::InvalidConic)?,
        )),
        SolvedConicKind::ParabolaSegment {
            vertex,
            focus,
            trim,
        } => {
            let axis = focus - vertex;
            let focal_length = axis.norm();
            let direction = UnitDirection2::try_new(axis).map_err(SketchError::InvalidConic)?;
            Ok(crate::ConicGeometry::ParabolaSegment(
                ParabolaSegment2::try_new(vertex, direction, focal_length, trim)
                    .map_err(SketchError::InvalidConic)?,
            ))
        }
        SolvedConicKind::HyperbolaSegment {
            center,
            transverse_axis_point,
            semi_conjugate,
            branch,
            trim,
        } => {
            crate::conics::validate_positive_conic_scalar(semi_conjugate)?;
            let axis = transverse_axis_point - center;
            let semi_transverse = axis.norm();
            let direction = UnitDirection2::try_new(axis).map_err(SketchError::InvalidConic)?;
            Ok(crate::ConicGeometry::HyperbolaSegment(
                HyperbolaSegment2::try_new(
                    center,
                    direction,
                    semi_transverse,
                    semi_conjugate,
                    branch,
                    trim,
                )
                .map_err(SketchError::InvalidConic)?,
            ))
        }
    }
}

fn validate_solved_conic_entity(conic: SolvedConic) -> Result<(), SketchError> {
    let geometry = conic.geometry()?;
    crate::conics::validate_conic_geometry(geometry)?;
    if let crate::ConicGeometry::RationalQuadratic(_) = geometry {
        for index in 0..=16 {
            geometry
                .evaluate(f64::from(index) / 16.0)
                .map_err(SketchError::InvalidConicEvaluation)?;
        }
    }
    if let crate::ConicGeometry::HyperbolaSegment(value) = geometry {
        for parameter in [0.0, 0.5, 1.0] {
            let point = geometry
                .evaluate(parameter)
                .map_err(SketchError::InvalidConicEvaluation)?
                .position;
            let witness = (point - value.center()).dot(&value.branch_witness());
            if !witness.is_finite() || witness <= 0.0 {
                return Err(SketchError::InvalidConic(
                    geosolve_geometry::ConicDefinitionError::ZeroDirection,
                ));
            }
        }
    }
    Ok(())
}

fn generic_curve_is_bounded(sketch: &Sketch, curve: SketchCurve) -> bool {
    match curve {
        SketchCurve::Line {
            domain: crate::LineParameterDomain::BoundedSegment,
            ..
        }
        | SketchCurve::Arc(_)
        | SketchCurve::Bezier(_) => true,
        SketchCurve::Conic(conic) => sketch
            .conic(conic)
            .is_some_and(|value| !value.is_periodic()),
        SketchCurve::Line { .. } | SketchCurve::Circle(_) => false,
    }
}

fn normalize_generic_latent(
    sketch: &Sketch,
    latents: &mut [SolvedLatent],
    constraint: SketchConstraintId,
    role: LatentVariableRole,
    contact: SketchCurveContact,
    changed: &mut bool,
) {
    let Some(latent) = latent_mut(latents, constraint, role) else {
        return;
    };
    let normalized = match contact.curve {
        SketchCurve::Circle(_) => unwrap_near(latent.value, contact.parameter),
        SketchCurve::Conic(conic)
            if sketch
                .conic(conic)
                .is_some_and(crate::ConicCurve::is_periodic) =>
        {
            unwrap_near(latent.value, contact.parameter)
        }
        curve if generic_curve_is_bounded(sketch, curve) => {
            normalize_bounded_candidate(latent.value).unwrap_or(latent.value)
        }
        SketchCurve::Line { .. } => latent.value,
        SketchCurve::Arc(_) | SketchCurve::Bezier(_) | SketchCurve::Conic(_) => unreachable!(),
    };
    *changed |= normalized.to_bits() != latent.value.to_bits();
    latent.value = normalized;
}

fn validate_generic_contact_candidate(
    sketch: &Sketch,
    candidate: &SolvedSketchState,
    constraint: SketchConstraintId,
    contact: SketchCurveContact,
    parameter: f64,
) -> Result<geosolve_geometry::CurveJet2, SolveRejection> {
    if !parameter.is_finite() {
        return Err(SolveRejection::ContactParameterOutOfDomain(constraint));
    }
    if generic_curve_is_bounded(sketch, contact.curve) {
        let Some(normalized) = normalize_bounded_candidate(parameter) else {
            return Err(SolveRejection::ContactParameterOutOfDomain(constraint));
        };
        let neighborhood_valid = match contact.neighborhood {
            CurveContactNeighborhood::Start => normalized.to_bits() == 0.0f64.to_bits(),
            CurveContactNeighborhood::End => normalized.to_bits() == 1.0f64.to_bits(),
            CurveContactNeighborhood::Interior => normalized > 0.0 && normalized < 1.0,
            CurveContactNeighborhood::Local { lower, upper } => {
                normalized > lower && normalized < upper
            }
        };
        if !neighborhood_valid {
            return Err(SolveRejection::AmbiguousContactNeighborhood(constraint));
        }
    } else if contact.neighborhood != CurveContactNeighborhood::Interior {
        return Err(SolveRejection::AmbiguousContactNeighborhood(constraint));
    }
    candidate_curve_jet(sketch, candidate, contact.curve, parameter)
        .map_err(|()| SolveRejection::DegenerateCurve(constraint))
}

fn validate_generic_orientation(
    constraint: SketchConstraintId,
    first: geosolve_geometry::Vector2<f64>,
    second: geosolve_geometry::Vector2<f64>,
    orientation: crate::CurveTangentOrientation,
) -> Result<(), SolveRejection> {
    let first_norm = first.norm();
    let second_norm = second.norm();
    if !first_norm.is_finite()
        || !second_norm.is_finite()
        || first_norm == 0.0
        || second_norm == 0.0
    {
        return Err(SolveRejection::DegenerateCurve(constraint));
    }
    let cosine = first.dot(&second) / (first_norm * second_norm);
    let valid = match orientation {
        crate::CurveTangentOrientation::Aligned => cosine > 0.0,
        crate::CurveTangentOrientation::Opposed => cosine < 0.0,
    };
    if valid {
        Ok(())
    } else {
        Err(SolveRejection::CenterDirectionFlipped(constraint))
    }
}

fn candidate_curve_jet(
    sketch: &Sketch,
    candidate: &SolvedSketchState,
    curve: SketchCurve,
    parameter: f64,
) -> Result<geosolve_geometry::CurveJet2, ()> {
    match curve {
        SketchCurve::Line { segment, domain } => {
            let segment = sketch.segments.get(segment).ok_or(())?;
            let start = candidate.geometry.point(segment.start()).ok_or(())?;
            let end = candidate.geometry.point(segment.end()).ok_or(())?;
            geosolve_geometry::line_jet(
                start,
                end,
                match domain {
                    crate::LineParameterDomain::SupportingLine => {
                        geosolve_geometry::CurveParameterDomain::SupportingLine
                    }
                    crate::LineParameterDomain::BoundedSegment => {
                        geosolve_geometry::CurveParameterDomain::Bounded {
                            lower: 0.0,
                            upper: 1.0,
                        }
                    }
                },
                parameter,
            )
            .map_err(|_| ())
        }
        SketchCurve::Circle(circle) => {
            let circle = candidate.geometry.circle(circle).ok_or(())?;
            geosolve_geometry::circle_jet(circle.center, circle.radius, parameter).map_err(|_| ())
        }
        SketchCurve::Arc(arc) => {
            let arc = candidate.geometry.arc(arc).ok_or(())?;
            geosolve_geometry::circular_arc_jet(
                arc.center,
                arc.radius,
                arc.start_angle,
                arc.signed_sweep,
                parameter,
            )
            .map_err(|_| ())
        }
        SketchCurve::Bezier(bezier) => {
            candidate_bezier_jet(sketch, candidate, bezier, parameter).map_err(|_| ())
        }
        SketchCurve::Conic(conic) => candidate
            .geometry
            .conic(conic)
            .ok_or(())?
            .evaluate(parameter)
            .map_err(|_| ()),
    }
}

fn candidate_bezier_jet(
    sketch: &Sketch,
    candidate: &SolvedSketchState,
    bezier: crate::BezierId,
    parameter: f64,
) -> Result<geosolve_geometry::CurveJet2, geosolve_geometry::CurveEvaluationError> {
    let curve = sketch
        .bezier(bezier)
        .expect("validated Bezier constraint reference");
    let point = |id| {
        candidate
            .geometry
            .point(id)
            .expect("compiled Bezier control point")
    };
    match curve.kind() {
        crate::BezierKind::Quadratic {
            controls: [first, second, third],
        } => geosolve_geometry::quadratic_bezier_jet(
            [point(first), point(second), point(third)],
            parameter,
        ),
        crate::BezierKind::Cubic {
            controls: [first, second, third, fourth],
        } => geosolve_geometry::cubic_bezier_jet(
            [point(first), point(second), point(third), point(fourth)],
            parameter,
        ),
    }
}

fn latent_value(
    latents: &[SolvedLatent],
    constraint: SketchConstraintId,
    role: LatentVariableRole,
) -> Result<f64, SolveRejection> {
    latents
        .iter()
        .find_map(|latent| {
            (latent.constraint_id == constraint && latent.role == role).then_some(latent.value)
        })
        .ok_or_else(|| {
            SolveRejection::IndependentValidationFailed(format!(
                "missing {role:?} for constraint {constraint:?}"
            ))
        })
}

fn latent_mut(
    latents: &mut [SolvedLatent],
    constraint: SketchConstraintId,
    role: LatentVariableRole,
) -> Option<&mut SolvedLatent> {
    latents
        .iter_mut()
        .find(|latent| latent.constraint_id == constraint && latent.role == role)
}

#[cfg(test)]
mod tests {
    use std::f64::consts::TAU;

    use super::*;
    use crate::{LineParameterDomain, LineSide};

    #[test]
    fn equivalent_periodic_candidates_are_unwrapped_to_the_retained_local_branch() {
        let mut sketch = Sketch::new(2.0).unwrap();
        let line_start = sketch.add_point(Point2::new(0.0, 0.0)).unwrap();
        let line_end = sketch.add_point(Point2::new(2.0, 0.0)).unwrap();
        let center = sketch.add_point(Point2::new(1.0, 1.0)).unwrap();
        let point = sketch.add_point(Point2::new(2.0, 1.0)).unwrap();
        let line = sketch.add_segment(line_start, line_end).unwrap();
        let circle = sketch.add_circle(center, 1.0).unwrap();
        let point_contact = sketch.add_point_on_circle(point, circle, 0.25).unwrap();
        let tangency = sketch
            .add_line_circle_tangency(
                line,
                circle,
                LineParameterDomain::BoundedSegment,
                LineSide::Left,
                0.5,
                -1.25,
            )
            .unwrap();
        let compiled = sketch
            .compile(SketchSolveRequest::default().without_previous_state_preferences())
            .unwrap();
        let mut candidate = compiled.solved_state(&sketch).unwrap();
        latent_mut(
            &mut candidate.latents,
            point_contact,
            LatentVariableRole::CircleAngle,
        )
        .unwrap()
        .value += 3.0 * TAU;
        latent_mut(
            &mut candidate.latents,
            tangency,
            LatentVariableRole::CircleAngle,
        )
        .unwrap()
        .value -= 2.0 * TAU;

        sketch.normalize_candidate_latents(&mut candidate);

        assert!(
            (latent_value(
                &candidate.latents,
                point_contact,
                LatentVariableRole::CircleAngle
            )
            .unwrap()
                - 0.25)
                .abs()
                <= f64::EPSILON
        );
        assert!(
            (latent_value(
                &candidate.latents,
                tangency,
                LatentVariableRole::CircleAngle
            )
            .unwrap()
                + 1.25)
                .abs()
                <= f64::EPSILON
        );
    }

    #[test]
    fn unavailable_domain_validation_maps_valid_core_rows_to_not_evaluated() {
        let mut sketch = Sketch::new(1.0).unwrap();
        let start = sketch.add_point(Point2::new(0.0, 0.0)).unwrap();
        let end = sketch.add_point(Point2::new(1.0, 0.0)).unwrap();
        let point = sketch.add_point(Point2::new(0.5, 0.0)).unwrap();
        let line = sketch.add_segment(start, end).unwrap();
        let constraint = sketch
            .add_point_on_line(point, line, LineParameterDomain::BoundedSegment, 0.5)
            .unwrap();
        let mut compiled = sketch
            .compile(SketchSolveRequest::default().without_previous_state_preferences())
            .unwrap();
        let mut report = compiled.problem.solve(SolverConfig::default()).unwrap();
        assert_eq!(report.hard_validity, HardValidity::Valid);
        assert!(report.hard_residuals_validated);
        let mut candidate = compiled.solved_state(&sketch).unwrap();
        candidate
            .latents
            .retain(|latent| latent.constraint_id != constraint);

        let rejection = sketch.validate_m7_candidate(&candidate).unwrap_err();
        assert!(matches!(
            rejection,
            SolveRejection::IndependentValidationFailed(_)
        ));
        report.hard_validity = domain_hard_validity(report.hard_validity, Some(&rejection));

        assert_eq!(report.hard_validity, HardValidity::NotEvaluated);
        assert!(report.hard_residuals_validated);
    }
}
