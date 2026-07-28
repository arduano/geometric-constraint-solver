use geosolve_core::{
    AuditBinding, AuditEvaluationStatus, AuditSnapshot, BoundId, CoordinateBound, HardValidity,
    OperationCheckpoint, OperationController, OperationWorkCounter, Problem, ResidualBlock,
    ResidualCategory, ResidualId, ResidualRowAudit, SolveReport, SolveTermination, SolverConfig,
    SourceConstraint, SourceConstraintId, VariableBlock, VariableId, VariableValue,
};
use geosolve_geometry::{Point2, Vector2};

use crate::curves::{
    CENTER_DIRECTION_COSINE_MARGIN, CIRCLE_ARC_TANGENCY_DIRECTION_TOLERANCE,
    CIRCLE_ARC_TANGENCY_RADIUS_RELATIVE_TOLERANCE,
    CIRCLE_ARC_TANGENCY_SCALE_UNCERTAINTY_MULTIPLIER, arc_signed_sweep,
    normalize_bounded_candidate, segment_points, tangency_distance, unwrap_near,
    validate_bounded_parameter, validate_radius,
};
use crate::model::{
    ArcAngleEndpoint, ArcId, CircleId, ConicId, ConicScalarRole, CoordinateAxis,
    CurveContactNeighborhood, DimensionKind, DimensionMode, NurbsId, PersistentSource, PointId,
    SegmentId, Sketch, SketchConstraintId, SketchConstraintKind, SketchCurve, SketchCurveContact,
    SketchDimensionId, SketchError, SketchScalarRef, validate_model_scale, validate_point,
};
use crate::residuals::{
    AxisDifferenceResidual, AxisDimensionResidual, BezierIncidence, CircleArcTangencyResidual,
    CircleTangencyResidual, CircularArcLengthResidual, CircularSweepResidual, CoincidentResidual,
    CollinearResidual, ConicPropertyResidual, ConicPropertyResidualKind, CurveParameterIncidence,
    DistanceResidual, EqualAngleResidual, EqualDistanceResidual, ExternalLineCollinearResidual,
    FixedCoordinateResidual, GenericCurveDirectionResidual, GenericCurveFilletResidual,
    GenericCurveIncidence, GenericCurvePairResidual, GenericEndpointContinuityResidual,
    GenericEqualCurvatureResidual, GenericPathLengthResidual, GenericPointOnCurveResidual,
    LineBezierTangencyResidual, LineCircleTangencyResidual, LineOffsetResidual,
    LineOffsetResidualMode, M38DimensionResidual, MidpointResidual, NurbsWeightIncidence,
    OrientedAngleResidual, PointOnBezierResidual, PointOnCircleResidual, PointOnLineResidual,
    PointTargetResidual, ScalarEqualityResidual, ScalarTargetResidual, SegmentPairEquation,
    SegmentPairResidual, SymmetryResidual,
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

/// Solver-coordinate role for an active associated output arc endpoint angle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArcAngleRole {
    Start,
    End,
}

/// Exact associated-arc-angle-to-core-variable relationship in source order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArcAngleVariableMapping {
    pub arc_id: ArcId,
    pub role: ArcAngleRole,
    pub variable_id: VariableId,
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

/// Exact non-gauge NURBS-weight-to-core-variable relationship.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NurbsWeightVariableMapping {
    pub nurbs_id: NurbsId,
    pub control_index: usize,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DiagnosticVariableOwner {
    Point(PointId),
    Circle(CircleId),
    Arc(ArcId),
    Conic(ConicId),
    Nurbs(NurbsId),
    Contact {
        constraint_id: SketchConstraintId,
        role: LatentVariableRole,
    },
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
    NurbsWeight {
        nurbs_id: NurbsId,
        control_index: usize,
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

/// Unitless margin below which fillet tangent intersections and offset derivatives are
/// considered numerically unresolved.
const CURVE_FILLET_REGULARITY_THRESHOLD: f64 = 1.0e-8;

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
    arc_angle_variables: Vec<ArcAngleVariableMapping>,
    conic_vector_variables: Vec<ConicVectorVariableMapping>,
    conic_scalar_variables: Vec<ConicScalarVariableMapping>,
    nurbs_weight_variables: Vec<NurbsWeightVariableMapping>,
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
    pub fn arc_angle_variables(&self) -> &[ArcAngleVariableMapping] {
        &self.arc_angle_variables
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
    pub fn nurbs_weight_variables(&self) -> &[NurbsWeightVariableMapping] {
        &self.nurbs_weight_variables
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

    pub(crate) fn diagnostic_variable_owners(&self) -> Vec<(VariableId, DiagnosticVariableOwner)> {
        self.point_variables
            .iter()
            .map(|mapping| {
                (
                    mapping.variable_id,
                    DiagnosticVariableOwner::Point(mapping.point_id),
                )
            })
            .chain(self.circle_radius_variables.iter().map(|mapping| {
                (
                    mapping.variable_id,
                    DiagnosticVariableOwner::Circle(mapping.circle_id),
                )
            }))
            .chain(self.arc_radius_variables.iter().map(|mapping| {
                (
                    mapping.variable_id,
                    DiagnosticVariableOwner::Arc(mapping.arc_id),
                )
            }))
            .chain(self.arc_angle_variables.iter().map(|mapping| {
                (
                    mapping.variable_id,
                    DiagnosticVariableOwner::Arc(mapping.arc_id),
                )
            }))
            .chain(self.conic_scalar_variables.iter().map(|mapping| {
                (
                    mapping.variable_id,
                    DiagnosticVariableOwner::Conic(mapping.conic_id),
                )
            }))
            .chain(self.conic_vector_variables.iter().map(|mapping| {
                (
                    mapping.variable_id,
                    DiagnosticVariableOwner::Conic(mapping.conic_id),
                )
            }))
            .chain(self.nurbs_weight_variables.iter().map(|mapping| {
                (
                    mapping.variable_id,
                    DiagnosticVariableOwner::Nurbs(mapping.nurbs_id),
                )
            }))
            .chain(self.latent_variables.iter().map(|mapping| {
                (
                    mapping.variable_id,
                    DiagnosticVariableOwner::Contact {
                        constraint_id: mapping.constraint_id,
                        role: mapping.role,
                    },
                )
            }))
            .collect()
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
    pub fn variable_for_arc_angle(&self, arc: ArcId, role: ArcAngleRole) -> Option<VariableId> {
        self.arc_angle_variables.iter().find_map(|mapping| {
            (mapping.arc_id == arc && mapping.role == role).then_some(mapping.variable_id)
        })
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

    #[must_use]
    pub fn variable_for_nurbs_weight(
        &self,
        nurbs: NurbsId,
        control_index: usize,
    ) -> Option<VariableId> {
        self.nurbs_weight_variables.iter().find_map(|mapping| {
            (mapping.nurbs_id == nurbs && mapping.control_index == control_index)
                .then_some(mapping.variable_id)
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
            let start_variable = arc_angle_variable_optional(
                &self.arc_angle_variables,
                mapping.arc_id,
                ArcAngleRole::Start,
            );
            let end_variable = arc_angle_variable_optional(
                &self.arc_angle_variables,
                mapping.arc_id,
                ArcAngleRole::End,
            );
            let (start_angle, end_angle, signed_sweep) = match (start_variable, end_variable) {
                (None, None) => (arc.start_angle(), arc.end_angle(), arc.signed_sweep()),
                (Some(start), Some(end)) => {
                    let start_angle = scalar_variable(problem, start)?;
                    let end_angle = scalar_variable(problem, end)?;
                    let turn_offset = retained_arc_turn_offset(arc)?;
                    let signed_sweep =
                        end_angle - start_angle + f64::from(turn_offset) * std::f64::consts::TAU;
                    validate_incident_arc_sweep(signed_sweep, arc.sweep())?;
                    (start_angle, end_angle, signed_sweep)
                }
                _ => {
                    return Err(geosolve_core::CoreError::InvalidSolverConfig {
                        field: "associated arc angle mapping",
                        message: "associated output arc must map both endpoint angles",
                    }
                    .into());
                }
            };
            arcs.push(SolvedArc {
                arc_id: mapping.arc_id,
                center: solved_point(&points, arc.center())?,
                radius,
                start_angle,
                end_angle,
                signed_sweep,
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
        let mut nurbs = Vec::with_capacity(sketch.nurbs.iter().count());
        for (nurbs_id, curve) in sketch.nurbs.iter() {
            let mut weights = Vec::with_capacity(curve.weights().len());
            for (control_index, retained) in curve.weights().iter().copied().enumerate() {
                let weight = if control_index == curve.gauge_index() {
                    retained
                } else {
                    nurbs_weight_value(
                        problem,
                        &self.nurbs_weight_variables,
                        nurbs_id,
                        control_index,
                    )?
                };
                weights.push(weight);
            }
            nurbs.push(SolvedNurbs { nurbs_id, weights });
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
                nurbs,
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
                    &self.arc_angle_variables,
                    &self.conic_vector_variables,
                    &self.conic_scalar_variables,
                    &self.nurbs_weight_variables,
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
                    &self.arc_angle_variables,
                    &self.conic_vector_variables,
                    &self.conic_scalar_variables,
                    &self.nurbs_weight_variables,
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

/// One solved circular arc. Ordinary endpoints remain fixed; active fillet endpoints are solved.
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

/// Solved positive weights for one runtime NURBS in control order.
#[derive(Clone, Debug, PartialEq)]
pub struct SolvedNurbs {
    pub nurbs_id: NurbsId,
    pub weights: Vec<f64>,
}

/// Finite geometry returned for display or downstream queries.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SketchGeometry {
    pub points: Vec<SolvedPoint>,
    pub circles: Vec<SolvedCircle>,
    pub arcs: Vec<SolvedArc>,
    pub conics: Vec<SolvedConic>,
    pub nurbs: Vec<SolvedNurbs>,
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

    #[must_use]
    pub fn nurbs(&self, nurbs: NurbsId) -> Option<&SolvedNurbs> {
        self.nurbs.iter().find(|item| item.nurbs_id == nurbs)
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
    InvalidNurbsEntity {
        nurbs: NurbsId,
        source: geosolve_geometry::NurbsDefinitionError,
    },
    NurbsEvaluation {
        constraint: SketchConstraintId,
        nurbs: NurbsId,
        source: geosolve_geometry::NurbsEvaluationError,
    },
    IndependentConstraintResidual {
        constraint: SketchConstraintId,
        maximum: f64,
        tolerance: f64,
    },
    IndependentDimensionResidual {
        dimension: SketchDimensionId,
        maximum: f64,
        tolerance: f64,
    },
    LineOffsetBranchFlipped(SketchDimensionId),
    InvalidFilletGeometry(SketchConstraintId),
    FilletSideFlipped(SketchConstraintId),
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
    /// Complete finite geometry produced by this attempt, when available.
    ///
    /// This geometry is diagnostic and non-authoritative. On an accepted solve it is
    /// identical to `geometry`; on rejection only `geometry` remains retained.
    pub attempted_geometry: Option<SketchGeometry>,
    /// Audit evaluated at exactly `geometry`, suitable for display.
    pub display_audit: AuditSnapshot,
    pub reference_values: Vec<ReferenceDimensionValue>,
    pub source_mappings: Vec<SketchSourceMapping>,
    /// Stable domain identities for every bound in the unstable core report.
    pub bound_mappings: Vec<SketchBoundMapping>,
    pub(crate) diagnostic_variable_owners: Vec<(VariableId, DiagnosticVariableOwner)>,
    /// Raw numerical-kernel report retained behind [`Self::unstable_core_report`].
    ///
    /// Stable hosts should consume sketch-owned diagnostic DTOs from the retained
    /// document session instead.
    pub(crate) core_report: SolveReport,
    pub rejection: Option<SolveRejection>,
    pub acceptance_hard_residual_max: Option<f64>,
}

/// Accepted-only redundancy classification in stable sketch runtime identities.
///
/// A fully redundant source has every active row classified as redundant. The
/// containing set also includes sources with only some redundant rows.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SketchAcceptedRedundancy {
    fully_redundant_sources: Vec<SketchSource>,
    sources_containing_redundant_rows: Vec<SketchSource>,
}

impl SketchAcceptedRedundancy {
    #[must_use]
    pub fn fully_redundant_sources(&self) -> &[SketchSource] {
        &self.fully_redundant_sources
    }

    #[must_use]
    pub fn sources_containing_redundant_rows(&self) -> &[SketchSource] {
        &self.sources_containing_redundant_rows
    }
}

impl SketchSolveResult {
    #[must_use]
    pub const fn accepted(&self) -> bool {
        self.rejection.is_none()
    }

    /// Returns the evolving direct numerical-kernel report.
    ///
    /// This is an explicitly unstable advanced-diagnostic compatibility seam.
    /// Application and editor integrations should consume
    /// [`crate::SketchDiagnosticSnapshot`] instead.
    #[must_use]
    pub const fn unstable_core_report(&self) -> &geosolve_core::SolveReport {
        &self.core_report
    }

    /// Returns authoritative redundancy only for an independently accepted result.
    #[must_use]
    pub fn accepted_redundancy(&self) -> Option<SketchAcceptedRedundancy> {
        if !self.accepted() {
            return None;
        }
        let map_sources = |core_sources: &[SourceConstraintId]| {
            let mut sources = Vec::new();
            for core_source in core_sources {
                if let Some(source) = self.source_mappings.iter().find_map(|mapping| {
                    (mapping.core_source_id == Some(*core_source)).then_some(mapping.source)
                }) && !sources.contains(&source)
                {
                    sources.push(source);
                }
            }
            sources
        };
        Some(SketchAcceptedRedundancy {
            fully_redundant_sources: map_sources(&self.core_report.redundant_sources),
            sources_containing_redundant_rows: map_sources(
                &self.core_report.sources_containing_redundant_rows,
            ),
        })
    }
}

fn compile_item(control: &mut Option<&mut OperationController>) -> bool {
    control.as_deref_mut().is_none_or(|controller| {
        controller
            .charge(
                OperationWorkCounter::DocumentLoweringItems,
                1,
                OperationCheckpoint::DocumentLowering,
            )
            .is_ok()
    })
}

impl Sketch {
    /// Compiles the current accepted geometry and one transient solve request.
    ///
    /// # Errors
    ///
    /// Returns an error for stale IDs, non-finite geometry, invalid scale, or
    /// a rejected core declaration.
    ///
    /// # Panics
    ///
    /// Panics only if the internal uncontrolled path reports an interruption.
    #[allow(clippy::too_many_lines)]
    pub fn compile(&self, request: SketchSolveRequest) -> Result<CompiledSketch, SketchError> {
        self.compile_inner(request, None)
            .map(|compiled| compiled.expect("uncontrolled compilation cannot be interrupted"))
    }

    pub(crate) fn compile_with_controller(
        &self,
        request: SketchSolveRequest,
        controller: &mut OperationController,
    ) -> Result<Option<CompiledSketch>, SketchError> {
        self.compile_inner(request, Some(controller))
    }

    #[allow(clippy::too_many_lines)]
    fn compile_inner(
        &self,
        request: SketchSolveRequest,
        mut control: Option<&mut OperationController>,
    ) -> Result<Option<CompiledSketch>, SketchError> {
        if !compile_item(&mut control) {
            return Ok(None);
        }
        validate_model_scale(self.model_scale)?;
        validate_request(self, request)?;
        if !compile_item(&mut control) {
            return Ok(None);
        }
        self.preflight_segments()?;
        if !compile_item(&mut control) {
            return Ok(None);
        }
        self.preflight_conics()?;

        let mut problem = Problem::new();
        let mut point_variables = Vec::new();
        for (point_id, point) in self.points.iter() {
            if !compile_item(&mut control) {
                return Ok(None);
            }
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
            if !compile_item(&mut control) {
                return Ok(None);
            }
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
            if !compile_item(&mut control) {
                return Ok(None);
            }
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
        let mut arc_angle_variables = Vec::new();
        let mut fillet_angle_arcs = Vec::new();
        for source in &self.source_order {
            if !compile_item(&mut control) {
                return Ok(None);
            }
            let arcs = match *source {
                PersistentSource::Constraint(constraint_id) => {
                    let Some(constraint) = self.constraints.get(constraint_id) else {
                        continue;
                    };
                    match constraint.kind() {
                        SketchConstraintKind::CurveCurveFillet { arc, .. } => vec![(arc, true)],
                        SketchConstraintKind::FixedArcAngle { arc, .. }
                        | SketchConstraintKind::FixedScalar {
                            property: SketchScalarRef::ArcAngle { arc, .. },
                            ..
                        } => vec![(arc, false)],
                        SketchConstraintKind::EqualScalar { first, second, .. } => [first, second]
                            .into_iter()
                            .filter_map(|property| match property {
                                SketchScalarRef::ArcAngle { arc, .. } => Some((arc, false)),
                                _ => None,
                            })
                            .collect(),
                        _ => continue,
                    }
                }
                PersistentSource::Dimension(dimension_id) => {
                    let Some(dimension) = self.dimensions.get(dimension_id) else {
                        continue;
                    };
                    match dimension.kind() {
                        DimensionKind::CircularSweep { arc, .. }
                        | DimensionKind::CircularArcLength { arc, .. } => vec![(arc, false)],
                        _ => continue,
                    }
                }
            };
            for (arc, fillet) in arcs {
                if fillet && fillet_angle_arcs.contains(&arc) {
                    return Err(geosolve_core::CoreError::InvalidSolverConfig {
                        field: "associated arc angle mapping",
                        message: "an output arc has more than one active fillet association",
                    }
                    .into());
                }
                if fillet {
                    fillet_angle_arcs.push(arc);
                }
                if arc_angle_variable_optional(&arc_angle_variables, arc, ArcAngleRole::Start)
                    .is_some()
                {
                    continue;
                }
                let arc_value = self.arc_value(arc)?;
                retained_arc_turn_offset(arc_value)?;
                for (role, value) in [
                    (ArcAngleRole::Start, arc_value.start_angle()),
                    (ArcAngleRole::End, arc_value.end_angle()),
                ] {
                    let variable_id = problem.add_variable(VariableBlock::scalar(value, 1.0)?);
                    arc_angle_variables.push(ArcAngleVariableMapping {
                        arc_id: arc,
                        role,
                        variable_id,
                    });
                }
            }
        }
        let mut conic_vector_variables = Vec::new();
        for (conic_id, conic) in self.conics.iter() {
            if !compile_item(&mut control) {
                return Ok(None);
            }
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
            if !compile_item(&mut control) {
                return Ok(None);
            }
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

        let mut nurbs_weight_variables = Vec::new();
        for (nurbs_id, curve) in self.nurbs.iter() {
            for (control_index, weight) in curve.weights().iter().copied().enumerate() {
                if !compile_item(&mut control) {
                    return Ok(None);
                }
                if control_index == curve.gauge_index() {
                    continue;
                }
                let variable_id = problem.add_variable(VariableBlock::scalar(weight, weight)?);
                nurbs_weight_variables.push(NurbsWeightVariableMapping {
                    nurbs_id,
                    control_index,
                    variable_id,
                });
                let bound_id = problem.add_bound(CoordinateBound::new(
                    variable_id,
                    0,
                    Some(MIN_REPRESENTABLE_RADIUS),
                    None,
                    format!("positive weight {control_index} for {}", curve.label()),
                )?)?;
                bound_mappings.push(SketchBoundMapping {
                    bound: SketchBound::NurbsWeight {
                        nurbs_id,
                        control_index,
                    },
                    bound_id,
                });
            }
        }

        let mut source_mappings = Vec::new();
        let mut latent_variables = Vec::new();
        for source in &self.source_order {
            if !compile_item(&mut control) {
                return Ok(None);
            }
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
                        &arc_angle_variables,
                        &conic_vector_variables,
                        &conic_scalar_variables,
                        &nurbs_weight_variables,
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
                        &arc_angle_variables,
                        &conic_vector_variables,
                        &conic_scalar_variables,
                        &nurbs_weight_variables,
                        dimension_id,
                        dimension,
                    )?);
                }
            }
        }

        if let Some(drag) = request.drag {
            if !compile_item(&mut control) {
                return Ok(None);
            }
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
            if !compile_item(&mut control) {
                return Ok(None);
            }
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
                if !compile_item(&mut control) {
                    return Ok(None);
                }
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
            if !compile_item(&mut control) {
                return Ok(None);
            }
            let position = self.point_position(mapping.point_id)?;
            problem.set_variable_value(
                mapping.variable_id,
                VariableValue::Vec2([position.x, position.y]),
            )?;
        }

        Ok(Some(CompiledSketch {
            problem,
            point_variables,
            circle_radius_variables,
            arc_radius_variables,
            arc_angle_variables,
            conic_vector_variables,
            conic_scalar_variables,
            nurbs_weight_variables,
            latent_variables,
            bound_mappings,
            source_mappings,
        }))
    }

    /// Compiles, solves, independently validates, and conditionally commits a request.
    ///
    /// # Errors
    ///
    /// Returns an error when compilation or the core solve cannot be started.
    /// Numerical and geometric solve failures are returned as rejected results.
    ///
    /// # Panics
    ///
    /// Panics only if the internal unlimited path reports an interruption
    /// without an operation controller.
    #[allow(clippy::too_many_lines)]
    pub fn solve(
        &mut self,
        request: SketchSolveRequest,
        config: SolverConfig,
    ) -> Result<SketchSolveResult, SketchError> {
        self.solve_inner(request, config, None)
            .map(|result| result.expect("uncontrolled sketch solving cannot be interrupted"))
    }

    /// Solves on a scratch sketch under cooperative operation control.
    ///
    /// An interrupted outcome never publishes candidate geometry as accepted
    /// and leaves this sketch unchanged.
    ///
    /// # Errors
    ///
    /// Returns a typed sketch compilation or core construction/evaluation error.
    pub fn solve_controlled(
        &mut self,
        request: SketchSolveRequest,
        config: SolverConfig,
        control: geosolve_core::OperationControl,
    ) -> Result<geosolve_core::OperationOutcome<SketchSolveResult>, SketchError> {
        let mut controller = geosolve_core::OperationController::new(control);
        if controller
            .checkpoint(geosolve_core::OperationCheckpoint::DocumentLowering)
            .is_err()
        {
            return Ok(controller.outcome_unchecked());
        }
        let mut candidate = self.clone();
        let result = candidate.solve_inner(request, config, Some(&mut controller))?;
        let Some(result) = result else {
            return Ok(controller.outcome_unchecked());
        };
        if result.accepted() {
            if controller
                .checkpoint(geosolve_core::OperationCheckpoint::BeforeCommit)
                .is_err()
            {
                return Ok(controller.outcome_unchecked());
            }
            *self = candidate;
        }
        Ok(controller.outcome(result))
    }

    pub(crate) fn solve_with_controller(
        &mut self,
        request: SketchSolveRequest,
        config: SolverConfig,
        controller: &mut geosolve_core::OperationController,
    ) -> Result<Option<SketchSolveResult>, SketchError> {
        if controller
            .checkpoint(geosolve_core::OperationCheckpoint::DocumentLowering)
            .is_err()
        {
            return Ok(None);
        }
        let mut candidate = self.clone();
        let result = candidate.solve_inner(request, config, Some(controller))?;
        if result.as_ref().is_some_and(SketchSolveResult::accepted) {
            *self = candidate;
        }
        Ok(result)
    }

    #[allow(clippy::too_many_lines)]
    fn solve_inner(
        &mut self,
        request: SketchSolveRequest,
        config: SolverConfig,
        mut control: Option<&mut geosolve_core::OperationController>,
    ) -> Result<Option<SketchSolveResult>, SketchError> {
        let config = acceptance_solver_config(config);
        let Some(mut compiled) = self.compile_inner(request, control.as_deref_mut())? else {
            return Ok(None);
        };
        let mut retained_audit = compiled.problem.audit_snapshot_partial();
        let mut core_report = if let Some(controller) = control.as_deref_mut() {
            let Some(report) = compiled.problem.solve_with_controller(config, controller)? else {
                return Ok(None);
            };
            report
        } else {
            compiled.problem.solve(config)?
        };
        let mut candidate = compiled.solved_state(self)?;
        let mut candidate_preparation =
            self.derive_curve_fillet_arcs(&mut candidate, config.normalized_residual_tolerance);
        let mut acceptance_hard_residual_max = None;

        if core_report_is_successful(&core_report, config) {
            let mut analysis_sketch = self.clone();
            for _ in 0..3 {
                if candidate_preparation.is_err()
                    || self.candidate_has_invalid_primitive(&candidate)
                {
                    break;
                }
                let normalized = self.normalize_candidate_latents(&mut candidate);
                candidate_preparation = self
                    .derive_curve_fillet_arcs(&mut candidate, config.normalized_residual_tolerance);
                if candidate_preparation.is_err() {
                    break;
                }
                if !normalized && !analysis_sketch.candidate_latents_differ(&candidate.latents) {
                    break;
                }
                analysis_sketch.commit_solved_state(&candidate)?;
                let Some(recompiled) =
                    analysis_sketch.compile_inner(request, control.as_deref_mut())?
                else {
                    return Ok(None);
                };
                compiled = recompiled;
                core_report = if let Some(controller) = control.as_deref_mut() {
                    let Some(report) =
                        compiled.problem.solve_with_controller(config, controller)?
                    else {
                        return Ok(None);
                    };
                    report
                } else {
                    compiled.problem.solve(config)?
                };
                candidate = compiled.solved_state(&analysis_sketch)?;
                candidate_preparation = self
                    .derive_curve_fillet_arcs(&mut candidate, config.normalized_residual_tolerance);
                if !core_report_is_successful(&core_report, config) {
                    break;
                }
            }
        }

        let core_hard_validity = core_report.hard_validity;
        if let Some(controller) = control.as_deref_mut()
            && controller
                .checkpoint(geosolve_core::OperationCheckpoint::BeforeFinalValidation)
                .is_err()
        {
            return Ok(None);
        }
        let candidate_validation = candidate_preparation.and_then(|()| {
            self.validate_m7_candidate(&candidate, config.normalized_residual_tolerance)
        });
        let independent_advanced_max = match &candidate_validation {
            Ok(maximum) => *maximum,
            Err(rejection) => rejection_residual_max(rejection).unwrap_or(0.0),
        };
        let candidate_domain_rejection = self
            .first_flipped_segment(&candidate.geometry)
            .map(SolveRejection::SegmentBranchFlipped)
            .or_else(|| candidate_validation.err());
        let domain_rejection = if core_hard_validity == HardValidity::Valid {
            match independent_hard_residual_metrics(&compiled.problem) {
                Ok((maximum, _, _)) => {
                    let maximum = maximum.max(independent_advanced_max);
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

        // `solved_state_for_problem` constructs every geometry family in retained
        // order and rejects any non-finite variable before returning the candidate.
        let attempted_geometry = Some(candidate.geometry.clone());
        if let Some(controller) = control.as_deref_mut()
            && controller
                .checkpoint(geosolve_core::OperationCheckpoint::AfterFinalValidation)
                .is_err()
        {
            return Ok(None);
        }
        if rejection.is_none() {
            if let Some(controller) = control
                && controller
                    .checkpoint(geosolve_core::OperationCheckpoint::BeforeCommit)
                    .is_err()
            {
                return Ok(None);
            }
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

        let diagnostic_variable_owners = compiled.diagnostic_variable_owners();
        Ok(Some(SketchSolveResult {
            geometry: self.geometry(),
            attempted_geometry,
            display_audit,
            reference_values: self.reference_values()?,
            source_mappings: compiled.source_mappings,
            bound_mappings: compiled.bound_mappings,
            diagnostic_variable_owners,
            core_report,
            rejection,
            acceptance_hard_residual_max,
        }))
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
            nurbs: self
                .nurbs
                .iter()
                .map(|(nurbs_id, curve)| SolvedNurbs {
                    nurbs_id,
                    weights: curve.weights().to_vec(),
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
    pub(crate) fn derive_curve_fillet_arcs(
        &self,
        candidate: &mut SolvedSketchState,
        tolerance: f64,
    ) -> Result<(), SolveRejection> {
        for (constraint_id, constraint) in self.constraints.iter() {
            let SketchConstraintKind::CurveCurveFillet {
                arc,
                first,
                second,
                endpoint_order,
                ..
            } = constraint.kind()
            else {
                continue;
            };
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
            let first_contact = validate_generic_contact_candidate(
                self,
                candidate,
                constraint_id,
                first,
                first_parameter,
            )?
            .position;
            let second_contact = validate_generic_contact_candidate(
                self,
                candidate,
                constraint_id,
                second,
                second_parameter,
            )?
            .position;
            let retained = self.arc_value(arc).map_err(|_| {
                SolveRejection::IndependentValidationFailed(
                    "curve fillet references a stale output arc".into(),
                )
            })?;
            let solved = candidate
                .geometry
                .arcs
                .iter_mut()
                .find(|solved| solved.arc_id == arc)
                .ok_or_else(|| {
                    SolveRejection::IndependentValidationFailed(
                        "curve fillet output arc is missing from candidate geometry".into(),
                    )
                })?;
            let (start, end) = match endpoint_order {
                crate::FilletEndpointOrder::FirstThenSecond => (first_contact, second_contact),
                crate::FilletEndpointOrder::SecondThenFirst => (second_contact, first_contact),
            };
            let start_offset = start - solved.center;
            let end_offset = end - solved.center;
            let start_norm = start_offset.norm();
            let end_norm = end_offset.norm();
            if !start_norm.is_finite()
                || !end_norm.is_finite()
                || start_norm == 0.0
                || end_norm == 0.0
            {
                return Err(SolveRejection::InvalidFilletGeometry(constraint_id));
            }
            let start_angle = unwrap_near(start_offset.y.atan2(start_offset.x), solved.start_angle);
            let end_angle = unwrap_near(end_offset.y.atan2(end_offset.x), solved.end_angle);
            let signed_sweep = arc_signed_sweep(start_angle, end_angle, solved.sweep)
                .map_err(|_| SolveRejection::InvalidFilletGeometry(constraint_id))?;
            if !start_angle.is_finite() || !end_angle.is_finite() || !signed_sweep.is_finite() {
                return Err(SolveRejection::InvalidFilletGeometry(constraint_id));
            }
            let start_radial = start_offset / start_norm;
            let end_radial = end_offset / end_norm;
            let core_start_radial =
                Vector2::new(solved.start_angle.cos(), solved.start_angle.sin());
            let core_end_radial = Vector2::new(solved.end_angle.cos(), solved.end_angle.sin());
            let start_dot = core_start_radial.dot(&start_radial);
            let end_dot = core_end_radial.dot(&end_radial);
            if !start_dot.is_finite() || !end_dot.is_finite() || start_dot <= 0.0 || end_dot <= 0.0
            {
                return Err(SolveRejection::InvalidFilletGeometry(constraint_id));
            }
            let turn_offset = retained_arc_turn_offset(retained)
                .map_err(|_| SolveRejection::InvalidFilletGeometry(constraint_id))?;
            let core_signed_sweep = solved.end_angle - solved.start_angle
                + f64::from(turn_offset) * std::f64::consts::TAU;
            validate_independent_constraint_rows(
                constraint_id,
                &[
                    start_angle - solved.start_angle,
                    end_angle - solved.end_angle,
                    signed_sweep - core_signed_sweep,
                ],
                tolerance,
            )?;
            solved.start_angle = start_angle;
            solved.end_angle = end_angle;
            solved.signed_sweep = signed_sweep;
        }
        Ok(())
    }

    #[allow(clippy::too_many_lines)]
    pub(crate) fn validate_m7_candidate(
        &self,
        candidate: &SolvedSketchState,
        tolerance: f64,
    ) -> Result<f64, SolveRejection> {
        let mut independent_advanced_max: f64 = 0.0;
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
        for solved in &candidate.geometry.nurbs {
            candidate_nurbs_geometry(self, candidate, solved.nurbs_id).map_err(|source| {
                SolveRejection::InvalidNurbsEntity {
                    nurbs: solved.nurbs_id,
                    source,
                }
            })?;
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
                    .map_err(|error| candidate_curve_rejection(constraint_id, error))?;
                    validate_generic_orientation(
                        constraint_id,
                        line_jet.first_derivative,
                        curve_jet.first_derivative,
                        orientation,
                    )?;
                }
                SketchConstraintKind::CurveDirection {
                    line,
                    contact,
                    relation,
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
                    .map_err(|error| candidate_curve_rejection(constraint_id, error))?;
                    let line_unit = line_jet
                        .differential()
                        .map_err(|_| SolveRejection::DegenerateCurve(constraint_id))?
                        .unit_tangent;
                    let curve_unit = curve_jet
                        .differential()
                        .map_err(|_| SolveRejection::DegenerateCurve(constraint_id))?
                        .unit_tangent;
                    let row = match relation {
                        crate::CurveDirectionRelation::Tangent(_) => {
                            cross_2d(line_unit, curve_unit)
                        }
                        crate::CurveDirectionRelation::Normal(_) => line_unit.dot(&curve_unit),
                    };
                    independent_advanced_max = independent_advanced_max.max(
                        validate_independent_constraint_rows(constraint_id, &[row], tolerance)?,
                    );
                    match relation {
                        crate::CurveDirectionRelation::Tangent(orientation) => {
                            validate_generic_orientation(
                                constraint_id,
                                line_jet.first_derivative,
                                curve_jet.first_derivative,
                                orientation,
                            )?;
                        }
                        crate::CurveDirectionRelation::Normal(side) => {
                            let left = Vector2::new(
                                -curve_jet.first_derivative.y,
                                curve_jet.first_derivative.x,
                            );
                            validate_generic_orientation(
                                constraint_id,
                                line_jet.first_derivative,
                                left,
                                match side {
                                    crate::CurveNormalSide::Left => {
                                        crate::CurveTangentOrientation::Aligned
                                    }
                                    crate::CurveNormalSide::Right => {
                                        crate::CurveTangentOrientation::Opposed
                                    }
                                },
                            )?;
                        }
                    }
                }
                SketchConstraintKind::CurveCurveFillet {
                    arc,
                    first,
                    first_side,
                    second,
                    second_side,
                    endpoint_order,
                } => {
                    independent_advanced_max =
                        independent_advanced_max.max(validate_curve_fillet_candidate(
                            self,
                            candidate,
                            constraint_id,
                            arc,
                            first,
                            first_side,
                            second,
                            second_side,
                            endpoint_order,
                            tolerance,
                        )?);
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
                SketchConstraintKind::EqualCurvature {
                    first,
                    second,
                    relation,
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
                    let first = validate_generic_contact_candidate(
                        self,
                        candidate,
                        constraint_id,
                        first,
                        first_parameter,
                    )?
                    .differential()
                    .map_err(|_| SolveRejection::DegenerateCurve(constraint_id))?;
                    let second = validate_generic_contact_candidate(
                        self,
                        candidate,
                        constraint_id,
                        second,
                        second_parameter,
                    )?
                    .differential()
                    .map_err(|_| SolveRejection::DegenerateCurve(constraint_id))?;
                    let signs_match = first.signed_curvature.is_sign_positive()
                        == second.signed_curvature.is_sign_positive();
                    let branch_valid = match relation {
                        crate::CurveCurvatureRelation::Signed => true,
                        crate::CurveCurvatureRelation::MagnitudeSameSign => {
                            first.signed_curvature != 0.0
                                && second.signed_curvature != 0.0
                                && signs_match
                        }
                        crate::CurveCurvatureRelation::MagnitudeOppositeSign => {
                            first.signed_curvature != 0.0
                                && second.signed_curvature != 0.0
                                && !signs_match
                        }
                    };
                    if !branch_valid {
                        return Err(SolveRejection::CenterDirectionFlipped(constraint_id));
                    }
                    let row = match relation {
                        crate::CurveCurvatureRelation::Signed
                        | crate::CurveCurvatureRelation::MagnitudeSameSign => {
                            (first.signed_curvature - second.signed_curvature) * self.model_scale
                        }
                        crate::CurveCurvatureRelation::MagnitudeOppositeSign => {
                            (first.signed_curvature + second.signed_curvature) * self.model_scale
                        }
                    };
                    independent_advanced_max = independent_advanced_max.max(
                        validate_independent_constraint_rows(constraint_id, &[row], tolerance)?,
                    );
                }
                SketchConstraintKind::EndpointContinuity {
                    first,
                    second,
                    kind,
                } => {
                    let first_jet = validate_generic_contact_candidate(
                        self,
                        candidate,
                        constraint_id,
                        first,
                        first.parameter,
                    )?;
                    let second_jet = validate_generic_contact_candidate(
                        self,
                        candidate,
                        constraint_id,
                        second,
                        second.parameter,
                    )?;
                    let first_sign = if first.neighborhood == CurveContactNeighborhood::Start {
                        -1.0
                    } else {
                        1.0
                    };
                    let second_sign = if second.neighborhood == CurveContactNeighborhood::Start {
                        1.0
                    } else {
                        -1.0
                    };
                    let mut rows = vec![
                        (first_jet.position.x - second_jet.position.x) / self.model_scale,
                        (first_jet.position.y - second_jet.position.y) / self.model_scale,
                    ];
                    if matches!(
                        kind,
                        crate::CurveContinuity::G1 | crate::CurveContinuity::G2
                    ) {
                        validate_generic_orientation(
                            constraint_id,
                            first_jet.first_derivative * first_sign,
                            second_jet.first_derivative * second_sign,
                            crate::CurveTangentOrientation::Aligned,
                        )?;
                        let first_differential = first_jet
                            .differential()
                            .map_err(|_| SolveRejection::DegenerateCurve(constraint_id))?;
                        let second_differential = second_jet
                            .differential()
                            .map_err(|_| SolveRejection::DegenerateCurve(constraint_id))?;
                        rows.push(cross_2d(
                            first_differential.unit_tangent * first_sign,
                            second_differential.unit_tangent * second_sign,
                        ));
                        if kind == crate::CurveContinuity::G2 {
                            rows.push(
                                (first_differential.signed_curvature * first_sign
                                    - second_differential.signed_curvature * second_sign)
                                    * self.model_scale,
                            );
                        }
                    }
                    if let crate::CurveContinuity::ParametricC2 {
                        first_rate,
                        second_rate,
                    } = kind
                    {
                        let first_derivative = first_jet.first_derivative * first_rate * first_sign;
                        let second_derivative =
                            second_jet.first_derivative * second_rate * second_sign;
                        let first_second = first_jet.second_derivative * first_rate * first_rate;
                        let second_second =
                            second_jet.second_derivative * second_rate * second_rate;
                        rows.extend([
                            (first_derivative.x - second_derivative.x) / self.model_scale,
                            (first_derivative.y - second_derivative.y) / self.model_scale,
                            (first_second.x - second_second.x) / self.model_scale,
                            (first_second.y - second_second.y) / self.model_scale,
                        ]);
                    }
                    independent_advanced_max = independent_advanced_max.max(
                        validate_independent_constraint_rows(constraint_id, &rows, tolerance)?,
                    );
                }
                _ => {}
            }
        }
        for (dimension_id, dimension) in self.dimensions.iter() {
            if dimension.mode() != DimensionMode::Driving {
                continue;
            }
            if matches!(
                dimension.kind(),
                DimensionKind::SupportingLineOffset { .. }
                    | DimensionKind::ExactTranslatedSegmentOffset { .. }
            ) {
                independent_advanced_max =
                    independent_advanced_max.max(validate_line_offset_candidate(
                        self,
                        candidate,
                        dimension_id,
                        dimension.kind(),
                        tolerance,
                    )?);
            }
        }
        Ok(independent_advanced_max)
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
                | SketchConstraintKind::LineCurveTangency { contact, .. }
                | SketchConstraintKind::CurveDirection { contact, .. } => {
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
                | SketchConstraintKind::CurveCurveTangency { first, second, .. }
                | SketchConstraintKind::EqualCurvature { first, second, .. }
                | SketchConstraintKind::EndpointContinuity { first, second, .. }
                | SketchConstraintKind::CurveCurveFillet { first, second, .. } => {
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
                            | SketchConstraintKind::CurveCurveTangency { first: contact, .. }
                            | SketchConstraintKind::CurveCurveFillet { first: contact, .. },
                            LatentVariableRole::FirstCurveParameter,
                        )
                        | (
                            SketchConstraintKind::CurveCurveContact {
                                second: contact, ..
                            }
                            | SketchConstraintKind::CurveCurveTangency {
                                second: contact, ..
                            }
                            | SketchConstraintKind::CurveCurveFillet {
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
                        }
                        | crate::ContactState::CurveCurveFillet {
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
                        }
                        | crate::ContactState::CurveCurveFillet {
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
            || candidate
                .geometry
                .nurbs
                .iter()
                .any(|solved| candidate_nurbs_geometry(self, candidate, solved.nurbs_id).is_err())
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
                    | SketchConstraintKind::LineCurveTangency { contact, .. }
                    | SketchConstraintKind::CurveDirection { contact, .. },
                    LatentVariableRole::CurveParameter,
                ) => contact.parameter = latent.value,
                (
                    SketchConstraintKind::CurveCurveContact { first, .. }
                    | SketchConstraintKind::CurveCurveTangency { first, .. }
                    | SketchConstraintKind::EqualCurvature { first, .. }
                    | SketchConstraintKind::EndpointContinuity { first, .. }
                    | SketchConstraintKind::CurveCurveFillet { first, .. },
                    LatentVariableRole::FirstCurveParameter,
                ) => first.parameter = latent.value,
                (
                    SketchConstraintKind::CurveCurveContact { second, .. }
                    | SketchConstraintKind::CurveCurveTangency { second, .. }
                    | SketchConstraintKind::EqualCurvature { second, .. }
                    | SketchConstraintKind::EndpointContinuity { second, .. }
                    | SketchConstraintKind::CurveCurveFillet { second, .. },
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
        let mut staged = self.clone();
        staged.apply_solved_state(candidate)?;
        *self = staged;
        Ok(())
    }

    fn apply_solved_state(&mut self, candidate: &SolvedSketchState) -> Result<(), SketchError> {
        for point in &candidate.geometry.points {
            self.set_point_position(point.point_id, point.position)?;
        }
        for circle in &candidate.geometry.circles {
            self.set_circle_radius(circle.circle_id, circle.radius)?;
        }
        for arc in &candidate.geometry.arcs {
            self.set_arc_radius(arc.arc_id, arc.radius)?;
            self.set_arc_span(arc.arc_id, arc.start_angle, arc.end_angle)?;
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
        for solved in &candidate.geometry.nurbs {
            self.replace_nurbs_weights(solved.nurbs_id, solved.weights.clone())?;
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

pub(crate) fn rejection_residual_max(rejection: &SolveRejection) -> Option<f64> {
    match rejection {
        SolveRejection::HardResidual { maximum, .. }
        | SolveRejection::IndependentConstraintResidual { maximum, .. }
        | SolveRejection::IndependentDimensionResidual { maximum, .. } => Some(*maximum),
        _ => None,
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
    arc_angle_variables: &[ArcAngleVariableMapping],
    conic_vector_variables: &[ConicVectorVariableMapping],
    conic_scalar_variables: &[ConicScalarVariableMapping],
    nurbs_weight_variables: &[NurbsWeightVariableMapping],
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
        SketchConstraintKind::ExternalPoint {
            point,
            target,
            provenance,
        } => {
            let point_name = sketch.point_name(point)?;
            let label = format!(
                "constraint {}: {point_name} coincident with external binding {}",
                constraint.ordinal(),
                provenance.binding
            );
            return compile_external_point_target(
                sketch,
                problem,
                point_variables,
                SketchSource::Constraint(constraint_id),
                point,
                target,
                provenance,
                label,
            );
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
        SketchConstraintKind::HorizontalPoints { first, second }
        | SketchConstraintKind::VerticalPoints { first, second } => {
            let (coordinate, orientation, coordinate_name) = match constraint.kind() {
                SketchConstraintKind::HorizontalPoints { .. } => (1, "horizontal", "y"),
                SketchConstraintKind::VerticalPoints { .. } => (0, "vertical", "x"),
                _ => unreachable!(),
            };
            let first_name = sketch.point_name(first)?;
            let second_name = sketch.point_name(second)?;
            let label = format!(
                "constraint {}: {first_name} and {second_name} {orientation}",
                constraint.ordinal()
            );
            let source_id = problem.add_source(SourceConstraint::new(&label)?);
            let residual = ResidualBlock::new(
                source_id,
                ResidualCategory::Hard,
                vec![
                    point_variable(point_variables, first)?,
                    point_variable(point_variables, second)?,
                ],
                1,
                vec![scale],
                vec![audit_row(
                    format!(
                        "({second_name}.{coordinate_name} - {first_name}.{coordinate_name}) / model_scale"
                    ),
                    pair_bindings(first_name, second_name),
                )],
                AxisDifferenceResidual { coordinate },
            )?;
            (label, residual)
        }
        kind => {
            return compile_curve_constraint(
                sketch,
                problem,
                point_variables,
                circle_radius_variables,
                arc_radius_variables,
                arc_angle_variables,
                conic_vector_variables,
                conic_scalar_variables,
                nurbs_weight_variables,
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
    arc_angle_variables: &[ArcAngleVariableMapping],
    conic_vector_variables: &[ConicVectorVariableMapping],
    conic_scalar_variables: &[ConicScalarVariableMapping],
    nurbs_weight_variables: &[NurbsWeightVariableMapping],
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
            let point = incidence.add(point_variable(point_variables, point)?);
            incidence.add(point_variable(point_variables, arc_value.center())?);
            incidence.add(arc_radius_variable(arc_radius_variables, arc)?);
            let parameter = CurveParameterIncidence::Variable(incidence.add(parameter_variable));
            let evaluator = GenericPointOnCurveResidual {
                point,
                curve: generic_curve_incidence(
                    sketch,
                    point_variables,
                    circle_radius_variables,
                    arc_radius_variables,
                    arc_angle_variables,
                    conic_vector_variables,
                    conic_scalar_variables,
                    nurbs_weight_variables,
                    &mut incidence,
                    SketchCurve::Arc(arc),
                    parameter,
                )?,
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
                        arc_angle_variables,
                        conic_vector_variables,
                        conic_scalar_variables,
                        nurbs_weight_variables,
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
        SketchConstraintKind::Collinear { first, second } => {
            let (_, _, first_value) = segment_points(sketch, first)?;
            let (_, _, second_value) = segment_points(sketch, second)?;
            let first_indices = segment_incidence(sketch, point_variables, &mut incidence, first)?;
            let second_indices =
                segment_incidence(sketch, point_variables, &mut incidence, second)?;
            let bindings = vec![
                AuditBinding::new("first", first_value.label()),
                AuditBinding::new("second", second_value.label()),
            ];
            (
                format!(
                    "constraint {}: {} and {} collinear",
                    constraint.ordinal(),
                    first_value.label(),
                    second_value.label()
                ),
                2,
                vec![1.0, scale],
                vec![
                    audit_row_unit(
                        "cross(unit_direction(first), unit_direction(second))".into(),
                        bindings.clone(),
                        "dimensionless",
                    ),
                    audit_row(
                        "cross(unit_direction(first), second.start - first.start) / model_scale"
                            .into(),
                        bindings,
                    ),
                ],
                Box::new(CollinearResidual {
                    first: first_indices,
                    second: second_indices,
                }),
            )
        }
        SketchConstraintKind::ExternalLineCollinear {
            segment,
            external_start,
            external_end,
            provenance,
        } => {
            let (_, _, native) = segment_points(sketch, segment)?;
            let native_indices =
                segment_incidence(sketch, point_variables, &mut incidence, segment)?;
            let bindings = external_provenance_bindings(provenance);
            (
                format!(
                    "constraint {}: {} collinear with external binding {}",
                    constraint.ordinal(),
                    native.label(),
                    provenance.binding
                ),
                2,
                vec![1.0, scale],
                vec![
                    audit_row_unit(
                        "cross(unit_direction(native), unit_direction(external))".into(),
                        bindings.clone(),
                        "dimensionless",
                    ),
                    audit_row(
                        "cross(unit_direction(native), external.start - native.start) / model_scale"
                            .into(),
                        bindings,
                    ),
                ],
                Box::new(ExternalLineCollinearResidual {
                    native: native_indices,
                    external_start: [external_start.x, external_start.y],
                    external_end: [external_end.x, external_end.y],
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
        SketchConstraintKind::EqualCircleArcRadius { circle, arc } => {
            let circle_value = sketch.circle_value(circle)?;
            let arc_value = sketch.arc_value(arc)?;
            incidence.add(circle_radius_variable(circle_radius_variables, circle)?);
            incidence.add(arc_radius_variable(arc_radius_variables, arc)?);
            (
                format!(
                    "constraint {}: {} and {} equal circular radius",
                    constraint.ordinal(),
                    circle_value.label(),
                    arc_value.label()
                ),
                1,
                vec![scale],
                vec![audit_row(
                    "(radius(circle) - radius(arc)) / model_scale".into(),
                    vec![
                        AuditBinding::new("circle", circle_value.label()),
                        AuditBinding::new("arc", arc_value.label()),
                    ],
                )],
                Box::new(ScalarEqualityResidual),
            )
        }
        SketchConstraintKind::EqualArcRadius { first, second } => {
            let first_value = sketch.arc_value(first)?;
            let second_value = sketch.arc_value(second)?;
            incidence.add(arc_radius_variable(arc_radius_variables, first)?);
            incidence.add(arc_radius_variable(arc_radius_variables, second)?);
            (
                format!(
                    "constraint {}: {} and {} equal circular radius",
                    constraint.ordinal(),
                    first_value.label(),
                    second_value.label()
                ),
                1,
                vec![scale],
                vec![audit_row(
                    "(radius(first arc) - radius(second arc)) / model_scale".into(),
                    vec![
                        AuditBinding::new("first", first_value.label()),
                        AuditBinding::new("second", second_value.label()),
                    ],
                )],
                Box::new(ScalarEqualityResidual),
            )
        }
        SketchConstraintKind::FixedArcAngle {
            arc,
            endpoint,
            target,
        } => {
            let arc_value = sketch.arc_value(arc)?;
            let role = match endpoint {
                ArcAngleEndpoint::Start => ArcAngleRole::Start,
                ArcAngleEndpoint::End => ArcAngleRole::End,
            };
            incidence.add(arc_angle_variable(arc_angle_variables, arc, role)?);
            (
                format!(
                    "constraint {}: {} {endpoint:?} angle fixed",
                    constraint.ordinal(),
                    arc_value.label()
                ),
                1,
                vec![1.0],
                vec![audit_row_unit(
                    "arc endpoint angle - captured angle".into(),
                    vec![
                        AuditBinding::new("arc", arc_value.label()),
                        AuditBinding::new("endpoint", format!("{endpoint:?}")),
                        AuditBinding::new("target", target.to_string()),
                    ],
                    "radian",
                )],
                Box::new(ScalarTargetResidual {
                    target,
                    multiplier: 1.0,
                }),
            )
        }
        SketchConstraintKind::FixedScalar {
            property,
            target,
            residual_scale,
        } => {
            incidence.add(scalar_property_variable(
                circle_radius_variables,
                arc_radius_variables,
                arc_angle_variables,
                conic_scalar_variables,
                nurbs_weight_variables,
                property,
            )?);
            (
                format!("constraint {}: mapped scalar fixed", constraint.ordinal()),
                1,
                vec![residual_scale],
                vec![audit_row_unit(
                    "property - target".into(),
                    vec![
                        AuditBinding::new("property", format!("{property:?}")),
                        AuditBinding::new("target", target.to_string()),
                    ],
                    "property-unit",
                )],
                Box::new(ScalarTargetResidual {
                    target,
                    multiplier: 1.0,
                }),
            )
        }
        SketchConstraintKind::EqualScalar {
            first,
            second,
            residual_scale,
        } => {
            incidence.add(scalar_property_variable(
                circle_radius_variables,
                arc_radius_variables,
                arc_angle_variables,
                conic_scalar_variables,
                nurbs_weight_variables,
                first,
            )?);
            incidence.add(scalar_property_variable(
                circle_radius_variables,
                arc_radius_variables,
                arc_angle_variables,
                conic_scalar_variables,
                nurbs_weight_variables,
                second,
            )?);
            (
                format!("constraint {}: mapped scalars equal", constraint.ordinal()),
                1,
                vec![residual_scale],
                vec![audit_row_unit(
                    "first property - second property".into(),
                    vec![
                        AuditBinding::new("first", format!("{first:?}")),
                        AuditBinding::new("second", format!("{second:?}")),
                    ],
                    "property-unit",
                )],
                Box::new(ScalarEqualityResidual),
            )
        }
        SketchConstraintKind::EqualDistance { first, second } => {
            let first_indices = [
                incidence.add(point_variable(point_variables, first[0])?),
                incidence.add(point_variable(point_variables, first[1])?),
            ];
            let second_indices = [
                incidence.add(point_variable(point_variables, second[0])?),
                incidence.add(point_variable(point_variables, second[1])?),
            ];
            let bindings = vec![
                AuditBinding::new("first start", sketch.point_name(first[0])?),
                AuditBinding::new("first end", sketch.point_name(first[1])?),
                AuditBinding::new("second start", sketch.point_name(second[0])?),
                AuditBinding::new("second end", sketch.point_name(second[1])?),
            ];
            (
                format!(
                    "constraint {}: equal point-pair distances",
                    constraint.ordinal()
                ),
                1,
                vec![scale],
                vec![audit_row(
                    "(distance(first) - distance(second)) / model_scale".into(),
                    bindings,
                )],
                Box::new(EqualDistanceResidual {
                    first: first_indices,
                    second: second_indices,
                }),
            )
        }
        SketchConstraintKind::EqualAngle {
            first,
            second,
            first_orientation,
            first_winding,
            second_orientation,
            second_winding,
        } => {
            let mut pair_indices = |pair: [SegmentId; 2]| -> Result<[[usize; 2]; 2], SketchError> {
                Ok([
                    segment_incidence(sketch, point_variables, &mut incidence, pair[0])?,
                    segment_incidence(sketch, point_variables, &mut incidence, pair[1])?,
                ])
            };
            let first_indices = pair_indices(first)?;
            let second_indices = pair_indices(second)?;
            (
                format!("constraint {}: equal directed angles", constraint.ordinal()),
                1,
                vec![1.0],
                vec![audit_row_unit(
                    "directed_angle(first, orientation, winding) - directed_angle(second, orientation, winding)".into(),
                    vec![
                        AuditBinding::new("first orientation", format!("{first_orientation:?}")),
                        AuditBinding::new("first winding", first_winding.to_string()),
                        AuditBinding::new("second orientation", format!("{second_orientation:?}")),
                        AuditBinding::new("second winding", second_winding.to_string()),
                    ],
                    "radian",
                )],
                Box::new(EqualAngleResidual {
                    first: first_indices,
                    second: second_indices,
                    first_orientation,
                    first_winding,
                    second_orientation,
                    second_winding,
                }),
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
        SketchConstraintKind::PointSymmetry {
            first,
            second,
            center,
        } => {
            let evaluator = MidpointResidual {
                point: incidence.add(point_variable(point_variables, center)?),
                start: incidence.add(point_variable(point_variables, first)?),
                end: incidence.add(point_variable(point_variables, second)?),
            };
            let bindings = vec![
                AuditBinding::new("first", sketch.point_name(first)?),
                AuditBinding::new("second", sketch.point_name(second)?),
                AuditBinding::new("center", sketch.point_name(center)?),
            ];
            (
                format!(
                    "constraint {}: point symmetry about center",
                    constraint.ordinal()
                ),
                2,
                vec![scale, scale],
                vec![
                    audit_row(
                        "(center.x - (first.x + second.x)/2) / model_scale".into(),
                        bindings.clone(),
                    ),
                    audit_row(
                        "(center.y - (first.y + second.y)/2) / model_scale".into(),
                        bindings,
                    ),
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
                arc_angle_variables,
                conic_vector_variables,
                conic_scalar_variables,
                nurbs_weight_variables,
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
                arc_angle_variables,
                conic_vector_variables,
                conic_scalar_variables,
                nurbs_weight_variables,
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
                arc_angle_variables,
                conic_vector_variables,
                conic_scalar_variables,
                nurbs_weight_variables,
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
        SketchConstraintKind::CurveDirection {
            line,
            contact,
            relation,
        } => {
            let (start, end, line_value) = segment_points(sketch, line)?;
            let curve_label = generic_curve_label(sketch, contact.curve)?;
            let parameter = add_curve_contact_latent(
                sketch,
                problem,
                latent_variables,
                bound_mappings,
                constraint_id,
                LatentVariableRole::CurveParameter,
                contact,
            )?;
            let parameter = CurveParameterIncidence::Variable(incidence.add(parameter));
            let curve = generic_curve_incidence(
                sketch,
                point_variables,
                circle_radius_variables,
                arc_radius_variables,
                arc_angle_variables,
                conic_vector_variables,
                conic_scalar_variables,
                nurbs_weight_variables,
                &mut incidence,
                contact.curve,
                parameter,
            )?;
            let evaluator = GenericCurveDirectionResidual {
                line: [
                    incidence.add(point_variable(point_variables, start)?),
                    incidence.add(point_variable(point_variables, end)?),
                ],
                curve,
                relation,
            };
            let (name, equation) = match relation {
                crate::CurveDirectionRelation::Tangent(_) => {
                    ("tangent direction", "cross(unit(line), unit(curve'(t)))")
                }
                crate::CurveDirectionRelation::Normal(_) => {
                    ("normal direction", "dot(unit(line), unit(curve'(t)))")
                }
            };
            let bindings = vec![
                AuditBinding::new("line", line_value.label()),
                AuditBinding::new("curve", curve_label),
                AuditBinding::new("relation", format!("{relation:?}")),
                AuditBinding::new("neighborhood", format!("{:?}", contact.neighborhood)),
            ];
            (
                format!(
                    "constraint {}: {} follows {curve_label} {name}",
                    constraint.ordinal(),
                    line_value.label()
                ),
                1,
                vec![1.0],
                vec![audit_row_unit(equation.into(), bindings, "dimensionless")],
                Box::new(evaluator),
            )
        }
        SketchConstraintKind::EqualCurvature {
            first,
            second,
            relation,
        } => {
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
                arc_angle_variables,
                conic_vector_variables,
                conic_scalar_variables,
                nurbs_weight_variables,
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
                arc_angle_variables,
                conic_vector_variables,
                conic_scalar_variables,
                nurbs_weight_variables,
                &mut incidence,
                second.curve,
                second_parameter,
            )?;
            let evaluator = GenericEqualCurvatureResidual {
                first: first_curve,
                second: second_curve,
                relation,
                model_scale: scale,
            };
            let bindings = vec![
                AuditBinding::new("first curve", first_label),
                AuditBinding::new("second curve", second_label),
                AuditBinding::new("curvature relation", format!("{relation:?}")),
                AuditBinding::new("first neighborhood", format!("{:?}", first.neighborhood)),
                AuditBinding::new("second neighborhood", format!("{:?}", second.neighborhood)),
            ];
            (
                format!(
                    "constraint {}: {first_label} and {second_label} equal curvature",
                    constraint.ordinal()
                ),
                1,
                vec![1.0],
                vec![audit_row_unit(
                    "model_scale * selected_signed_curvature_difference".into(),
                    bindings,
                    "dimensionless",
                )],
                Box::new(evaluator),
            )
        }
        SketchConstraintKind::EndpointContinuity {
            first,
            second,
            kind,
        } => {
            let first_label = generic_curve_label(sketch, first.curve)?;
            let second_label = generic_curve_label(sketch, second.curve)?;
            let first_parameter = CurveParameterIncidence::Fixed(first.parameter);
            let first_curve = generic_curve_incidence(
                sketch,
                point_variables,
                circle_radius_variables,
                arc_radius_variables,
                arc_angle_variables,
                conic_vector_variables,
                conic_scalar_variables,
                nurbs_weight_variables,
                &mut incidence,
                first.curve,
                first_parameter,
            )?;
            let second_parameter = CurveParameterIncidence::Fixed(second.parameter);
            let second_curve = generic_curve_incidence(
                sketch,
                point_variables,
                circle_radius_variables,
                arc_radius_variables,
                arc_angle_variables,
                conic_vector_variables,
                conic_scalar_variables,
                nurbs_weight_variables,
                &mut incidence,
                second.curve,
                second_parameter,
            )?;
            let first_sign = match first.neighborhood {
                CurveContactNeighborhood::Start => -1.0,
                CurveContactNeighborhood::End => 1.0,
                _ => return Err(SketchError::InvalidContinuityEndpoint),
            };
            let second_sign = match second.neighborhood {
                CurveContactNeighborhood::Start => 1.0,
                CurveContactNeighborhood::End => -1.0,
                _ => return Err(SketchError::InvalidContinuityEndpoint),
            };
            let evaluator = GenericEndpointContinuityResidual {
                first: first_curve,
                second: second_curve,
                first_sign,
                second_sign,
                kind,
                model_scale: scale,
            };
            let bindings = vec![
                AuditBinding::new("incoming curve", first_label),
                AuditBinding::new("outgoing curve", second_label),
                AuditBinding::new("continuity", format!("{kind:?}")),
                AuditBinding::new("first endpoint", format!("{:?}", first.neighborhood)),
                AuditBinding::new("second endpoint", format!("{:?}", second.neighborhood)),
            ];
            let mut rows = vec![
                audit_row(
                    "(incoming_endpoint.x - outgoing_endpoint.x) / model_scale".into(),
                    bindings.clone(),
                ),
                audit_row(
                    "(incoming_endpoint.y - outgoing_endpoint.y) / model_scale".into(),
                    bindings.clone(),
                ),
            ];
            let mut scales = vec![scale, scale];
            match kind {
                crate::CurveContinuity::G0 => {}
                crate::CurveContinuity::G1 => {
                    rows.push(audit_row_unit(
                        "cross(unit(incoming_path_tangent), unit(outgoing_path_tangent))".into(),
                        bindings,
                        "dimensionless",
                    ));
                    scales.push(1.0);
                }
                crate::CurveContinuity::G2 => {
                    rows.push(audit_row_unit(
                        "cross(unit(incoming_path_tangent), unit(outgoing_path_tangent))".into(),
                        bindings.clone(),
                        "dimensionless",
                    ));
                    rows.push(audit_row_unit(
                        "model_scale * (incoming_path_curvature - outgoing_path_curvature)".into(),
                        bindings,
                        "dimensionless",
                    ));
                    scales.extend([1.0, 1.0]);
                }
                crate::CurveContinuity::ParametricC2 { .. } => {
                    for coordinate in ["x", "y"] {
                        rows.push(audit_row(
                            format!(
                                "(q1*a1*incoming'.{coordinate} - q2*a2*outgoing'.{coordinate}) / model_scale"
                            ),
                            bindings.clone(),
                        ));
                    }
                    for coordinate in ["x", "y"] {
                        rows.push(audit_row(
                            format!(
                                "(a1^2*incoming''.{coordinate} - a2^2*outgoing''.{coordinate}) / model_scale"
                            ),
                            bindings.clone(),
                        ));
                    }
                    scales.extend([scale; 4]);
                }
            }
            (
                format!(
                    "constraint {}: {first_label} to {second_label} {kind:?}",
                    constraint.ordinal()
                ),
                rows.len(),
                scales,
                rows,
                Box::new(evaluator),
            )
        }
        SketchConstraintKind::CurveCurveFillet {
            arc,
            first,
            first_side,
            second,
            second_side,
            endpoint_order,
        } => {
            let arc_value = sketch.arc_value(arc)?;
            let first_label = generic_curve_label(sketch, first.curve)?;
            let second_label = generic_curve_label(sketch, second.curve)?;
            let first_variable = add_curve_contact_latent(
                sketch,
                problem,
                latent_variables,
                bound_mappings,
                constraint_id,
                LatentVariableRole::FirstCurveParameter,
                first,
            )?;
            let second_variable = add_curve_contact_latent(
                sketch,
                problem,
                latent_variables,
                bound_mappings,
                constraint_id,
                LatentVariableRole::SecondCurveParameter,
                second,
            )?;
            let first_parameter = CurveParameterIncidence::Variable(incidence.add(first_variable));
            let second_parameter =
                CurveParameterIncidence::Variable(incidence.add(second_variable));
            let evaluator = GenericCurveFilletResidual {
                center: incidence.add(point_variable(point_variables, arc_value.center())?),
                radius: incidence.add(arc_radius_variable(arc_radius_variables, arc)?),
                start_angle: incidence.add(arc_angle_variable(
                    arc_angle_variables,
                    arc,
                    ArcAngleRole::Start,
                )?),
                end_angle: incidence.add(arc_angle_variable(
                    arc_angle_variables,
                    arc,
                    ArcAngleRole::End,
                )?),
                first: generic_curve_incidence(
                    sketch,
                    point_variables,
                    circle_radius_variables,
                    arc_radius_variables,
                    arc_angle_variables,
                    conic_vector_variables,
                    conic_scalar_variables,
                    nurbs_weight_variables,
                    &mut incidence,
                    first.curve,
                    first_parameter,
                )?,
                first_side,
                second: generic_curve_incidence(
                    sketch,
                    point_variables,
                    circle_radius_variables,
                    arc_radius_variables,
                    arc_angle_variables,
                    conic_vector_variables,
                    conic_scalar_variables,
                    nurbs_weight_variables,
                    &mut incidence,
                    second.curve,
                    second_parameter,
                )?,
                second_side,
                endpoint_order,
            };
            let bindings = vec![
                AuditBinding::new("arc", arc_value.label()),
                AuditBinding::new("first parent", first_label),
                AuditBinding::new("first side", format!("{first_side:?}")),
                AuditBinding::new("second parent", second_label),
                AuditBinding::new("second side", format!("{second_side:?}")),
                AuditBinding::new("endpoint order", format!("{endpoint_order:?}")),
                AuditBinding::new("sweep", format!("{:?}", arc_value.sweep())),
                AuditBinding::new("first warm-start parameter", first.parameter.to_string()),
                AuditBinding::new("second warm-start parameter", second.parameter.to_string()),
            ];
            let mut rows = [
                "(center.x - first(t).x - first_side*radius*left_normal(first').x) / model_scale",
                "(center.y - first(t).y - first_side*radius*left_normal(first').y) / model_scale",
                "(center.x - second(t).x - second_side*radius*left_normal(second').x) / model_scale",
                "(center.y - second(t).y - second_side*radius*left_normal(second').y) / model_scale",
            ]
            .into_iter()
            .map(|equation| audit_row(equation.into(), bindings.clone()))
            .collect::<Vec<_>>();
            for equation in [
                "cross(output_radial(start_angle), unit(ordered_start_parent_contact - center))",
                "cross(output_radial(end_angle), unit(ordered_end_parent_contact - center))",
            ] {
                rows.push(audit_row_unit(
                    equation.into(),
                    bindings.clone(),
                    "dimensionless",
                ));
            }
            (
                format!(
                    "constraint {}: {first_label} to {second_label} associative curve fillet",
                    constraint.ordinal()
                ),
                6,
                vec![scale, scale, scale, scale, 1.0, 1.0],
                rows,
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
            incidence.add(point_variable(point_variables, circle_value.center())?);
            incidence.add(circle_radius_variable(circle_radius_variables, circle)?);
            incidence.add(point_variable(point_variables, arc_value.center())?);
            incidence.add(arc_radius_variable(arc_radius_variables, arc)?);
            let circle_parameter =
                CurveParameterIncidence::Variable(incidence.add(circle_variable));
            let arc_parameter = CurveParameterIncidence::Variable(incidence.add(arc_variable));
            let evaluator = CircleArcTangencyResidual {
                circle: generic_curve_incidence(
                    sketch,
                    point_variables,
                    circle_radius_variables,
                    arc_radius_variables,
                    arc_angle_variables,
                    conic_vector_variables,
                    conic_scalar_variables,
                    nurbs_weight_variables,
                    &mut incidence,
                    SketchCurve::Circle(circle),
                    circle_parameter,
                )?,
                arc: generic_curve_incidence(
                    sketch,
                    point_variables,
                    circle_radius_variables,
                    arc_radius_variables,
                    arc_angle_variables,
                    conic_vector_variables,
                    conic_scalar_variables,
                    nurbs_weight_variables,
                    &mut incidence,
                    SketchCurve::Arc(arc),
                    arc_parameter,
                )?,
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

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn compile_dimension(
    sketch: &Sketch,
    problem: &mut Problem,
    point_variables: &[PointVariableMapping],
    circle_radius_variables: &[CircleRadiusVariableMapping],
    arc_radius_variables: &[ArcRadiusVariableMapping],
    arc_angle_variables: &[ArcAngleVariableMapping],
    conic_vector_variables: &[ConicVectorVariableMapping],
    conic_scalar_variables: &[ConicScalarVariableMapping],
    nurbs_weight_variables: &[NurbsWeightVariableMapping],
    dimension_id: SketchDimensionId,
    dimension: &crate::SketchDimension,
) -> Result<SketchSourceMapping, SketchError> {
    let kind = dimension.kind();
    if matches!(
        kind,
        DimensionKind::CircularSweep { .. }
            | DimensionKind::CircularArcLength { .. }
            | DimensionKind::ConicProperty { .. }
            | DimensionKind::PathLength { .. }
            | DimensionKind::EqualPathLength { .. }
    ) {
        return compile_m38_dimension(
            sketch,
            problem,
            point_variables,
            circle_radius_variables,
            arc_radius_variables,
            arc_angle_variables,
            conic_vector_variables,
            conic_scalar_variables,
            nurbs_weight_variables,
            dimension_id,
            dimension,
            kind,
        );
    }
    if matches!(kind, DimensionKind::CoordinateDifference { .. }) {
        return compile_coordinate_dimension(
            sketch,
            problem,
            point_variables,
            dimension_id,
            dimension,
            kind,
        );
    }
    if matches!(
        kind,
        DimensionKind::SupportingLineOffset { .. }
            | DimensionKind::ExactTranslatedSegmentOffset { .. }
    ) {
        return compile_line_offset_dimension(
            sketch,
            problem,
            point_variables,
            dimension_id,
            dimension,
            kind,
        );
    }
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

fn compile_coordinate_dimension(
    sketch: &Sketch,
    problem: &mut Problem,
    point_variables: &[PointVariableMapping],
    dimension_id: SketchDimensionId,
    dimension: &crate::SketchDimension,
    kind: DimensionKind,
) -> Result<SketchSourceMapping, SketchError> {
    let DimensionKind::CoordinateDifference {
        first,
        second,
        axis,
        target,
    } = kind
    else {
        unreachable!("non-coordinate dimension reached coordinate compiler");
    };
    let first_name = sketch.point_name(first)?;
    let second_name = sketch.point_name(second)?;
    let coordinate = match axis {
        crate::CoordinateAxis::X => 0,
        crate::CoordinateAxis::Y => 1,
    };
    let subject = format!("signed {axis:?} coordinate {first_name} to {second_name}");
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
            format!("(({second_name}.{axis:?} - {first_name}.{axis:?}) - target) / model_scale"),
            vec![
                AuditBinding::new("first", first_name),
                AuditBinding::new("second", second_name),
                AuditBinding::new("axis", format!("{axis:?}")),
                AuditBinding::new("target", target.to_string()),
            ],
        )],
        AxisDimensionResidual { coordinate, target },
    )?)?;
    Ok(equation_mapping(
        SketchSource::Dimension(dimension_id),
        label,
        source_id,
        residual_id,
    ))
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn compile_m38_dimension(
    sketch: &Sketch,
    problem: &mut Problem,
    point_variables: &[PointVariableMapping],
    circle_radius_variables: &[CircleRadiusVariableMapping],
    arc_radius_variables: &[ArcRadiusVariableMapping],
    arc_angle_variables: &[ArcAngleVariableMapping],
    conic_vector_variables: &[ConicVectorVariableMapping],
    conic_scalar_variables: &[ConicScalarVariableMapping],
    nurbs_weight_variables: &[NurbsWeightVariableMapping],
    dimension_id: SketchDimensionId,
    dimension: &crate::SketchDimension,
    kind: DimensionKind,
) -> Result<SketchSourceMapping, SketchError> {
    if dimension.mode() == DimensionMode::Reference {
        return Ok(SketchSourceMapping {
            source: SketchSource::Dimension(dimension_id),
            source_label: format!(
                "dimension {}: M38 reference measurement",
                dimension.ordinal()
            ),
            core_source_id: None,
            residual_ids: Vec::new(),
        });
    }
    let mut incidence = IncidenceBuilder::default();
    let (subject, target, scale, template, evaluator): (
        String,
        f64,
        f64,
        &'static str,
        M38DimensionResidual,
    ) = match kind {
        DimensionKind::CircularSweep { arc, target } => {
            let value = sketch.arc_value(arc)?;
            (
                format!("circular sweep of {}", value.label()),
                target,
                1.0,
                "(end_angle - start_angle + retained_turns*2*pi) - target",
                M38DimensionResidual::CircularSweep(CircularSweepResidual {
                    start_angle: incidence.add(arc_angle_variable(
                        arc_angle_variables,
                        arc,
                        ArcAngleRole::Start,
                    )?),
                    end_angle: incidence.add(arc_angle_variable(
                        arc_angle_variables,
                        arc,
                        ArcAngleRole::End,
                    )?),
                    turn_offset: retained_arc_turn_offset(value)?,
                    target,
                }),
            )
        }
        DimensionKind::CircularArcLength { arc, target } => {
            let value = sketch.arc_value(arc)?;
            (
                format!("circular arc length of {}", value.label()),
                target,
                sketch.model_scale,
                "radius * abs(end_angle - start_angle + retained_turns*2*pi) - target",
                M38DimensionResidual::CircularArcLength(CircularArcLengthResidual {
                    radius: incidence.add(arc_radius_variable(arc_radius_variables, arc)?),
                    start_angle: incidence.add(arc_angle_variable(
                        arc_angle_variables,
                        arc,
                        ArcAngleRole::Start,
                    )?),
                    end_angle: incidence.add(arc_angle_variable(
                        arc_angle_variables,
                        arc,
                        ArcAngleRole::End,
                    )?),
                    turn_offset: retained_arc_turn_offset(value)?,
                    target,
                }),
            )
        }
        DimensionKind::ConicProperty {
            conic,
            property,
            target,
        } => {
            let value = sketch.conic_value(conic)?;
            let residual_kind = match value.kind() {
                crate::ConicKind::Ellipse {
                    center,
                    major_axis_point,
                    ..
                }
                | crate::ConicKind::EllipticalArc {
                    center,
                    major_axis_point,
                    ..
                } => ConicPropertyResidualKind::Ellipse {
                    center: incidence.add(point_variable(point_variables, center)?),
                    axis: incidence.add(point_variable(point_variables, major_axis_point)?),
                    ratio: incidence.add(conic_scalar_variable(
                        conic_scalar_variables,
                        conic,
                        ConicScalarRole::MinorAxisRatio,
                    )?),
                    property,
                },
                crate::ConicKind::ParabolaSegment { vertex, focus, .. }
                    if property == crate::model::M38ConicProperty::FocalDistance =>
                {
                    ConicPropertyResidualKind::ParabolaFocalDistance {
                        vertex: incidence.add(point_variable(point_variables, vertex)?),
                        focus: incidence.add(point_variable(point_variables, focus)?),
                    }
                }
                crate::ConicKind::HyperbolaSegment {
                    center,
                    transverse_axis_point,
                    ..
                } => ConicPropertyResidualKind::Hyperbola {
                    center: incidence.add(point_variable(point_variables, center)?),
                    axis: incidence.add(point_variable(point_variables, transverse_axis_point)?),
                    semi_conjugate: incidence.add(conic_scalar_variable(
                        conic_scalar_variables,
                        conic,
                        ConicScalarRole::SemiConjugate,
                    )?),
                    property,
                },
                _ => {
                    return Err(geosolve_core::CoreError::InvalidSolverConfig {
                        field: "M38 conic property dimension",
                        message: "property is unsupported for this conic family",
                    }
                    .into());
                }
            };
            (
                format!("{property:?} of {}", value.label()),
                target,
                sketch.model_scale,
                "conic_property(active_geometry) - target",
                M38DimensionResidual::ConicProperty(ConicPropertyResidual {
                    kind: residual_kind,
                    target,
                }),
            )
        }
        DimensionKind::PathLength {
            curve,
            start,
            end,
            target,
        } => {
            let generic = generic_curve_incidence(
                sketch,
                point_variables,
                circle_radius_variables,
                arc_radius_variables,
                arc_angle_variables,
                conic_vector_variables,
                conic_scalar_variables,
                nurbs_weight_variables,
                &mut incidence,
                curve,
                CurveParameterIncidence::Fixed(start),
            )?;
            (
                format!(
                    "bounded path length of {}",
                    generic_curve_label(sketch, curve)?
                ),
                target,
                sketch.model_scale,
                "certified_integral(start,end,norm(curve'(t)),dt) - target",
                M38DimensionResidual::PathLength(Box::new(GenericPathLengthResidual {
                    first: generic,
                    first_interval: [start, end],
                    second: None,
                    target,
                    tolerance: sketch.model_scale * 1.0e-11,
                    max_evaluations: 8193,
                })),
            )
        }
        DimensionKind::EqualPathLength {
            first,
            first_start,
            first_end,
            second,
            second_start,
            second_end,
            target,
        } => {
            let first_curve = generic_curve_incidence(
                sketch,
                point_variables,
                circle_radius_variables,
                arc_radius_variables,
                arc_angle_variables,
                conic_vector_variables,
                conic_scalar_variables,
                nurbs_weight_variables,
                &mut incidence,
                first,
                CurveParameterIncidence::Fixed(first_start),
            )?;
            let second_curve = generic_curve_incidence(
                sketch,
                point_variables,
                circle_radius_variables,
                arc_radius_variables,
                arc_angle_variables,
                conic_vector_variables,
                conic_scalar_variables,
                nurbs_weight_variables,
                &mut incidence,
                second,
                CurveParameterIncidence::Fixed(second_start),
            )?;
            (
                "equal bounded path lengths".into(),
                target,
                sketch.model_scale,
                "certified_length(first) - certified_length(second) - target",
                M38DimensionResidual::PathLength(Box::new(GenericPathLengthResidual {
                    first: first_curve,
                    first_interval: [first_start, first_end],
                    second: Some((second_curve, [second_start, second_end])),
                    target,
                    tolerance: sketch.model_scale * 1.0e-11,
                    max_evaluations: 8193,
                })),
            )
        }
        _ => unreachable!("non-M38 dimension reached M38 compiler"),
    };
    let label = format!(
        "dimension {}: {subject} = {target} (driving)",
        dimension.ordinal()
    );
    let source_id = problem.add_source(SourceConstraint::new(&label)?);
    let residual_id = problem.add_residual(ResidualBlock::new(
        source_id,
        ResidualCategory::Hard,
        incidence.variables,
        1,
        vec![scale],
        vec![audit_row(
            template.into(),
            vec![
                AuditBinding::new("subject", subject),
                AuditBinding::new("target", target.to_string()),
                AuditBinding::new("work bound", "8193 derivative evaluations"),
            ],
        )],
        evaluator,
    )?)?;
    Ok(equation_mapping(
        SketchSource::Dimension(dimension_id),
        label,
        source_id,
        residual_id,
    ))
}

#[allow(clippy::too_many_lines)]
fn compile_line_offset_dimension(
    sketch: &Sketch,
    problem: &mut Problem,
    point_variables: &[PointVariableMapping],
    dimension_id: SketchDimensionId,
    dimension: &crate::SketchDimension,
    kind: DimensionKind,
) -> Result<SketchSourceMapping, SketchError> {
    let (source, target_segment, target, side, orientation, mode, mode_label) = match kind {
        DimensionKind::SupportingLineOffset {
            source,
            target_segment,
            target,
            side,
            orientation,
        } => (
            source,
            target_segment,
            target,
            side,
            orientation,
            LineOffsetResidualMode::SupportingLine,
            "supporting-line offset",
        ),
        DimensionKind::ExactTranslatedSegmentOffset {
            source,
            target_segment,
            target,
            side,
            orientation,
        } => (
            source,
            target_segment,
            target,
            side,
            orientation,
            LineOffsetResidualMode::ExactTranslatedSegment,
            "exact translated-segment offset",
        ),
        _ => unreachable!("non-offset dimension reached offset compiler"),
    };
    let (_, _, source_value) = segment_points(sketch, source)?;
    let (_, _, target_value) = segment_points(sketch, target_segment)?;
    let subject = format!(
        "{mode_label} {} to {} ({side:?}, {orientation:?})",
        source_value.label(),
        target_value.label()
    );
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
    let mut incidence = IncidenceBuilder::default();
    let source_indices = segment_incidence(sketch, point_variables, &mut incidence, source)?;
    let target_indices =
        segment_incidence(sketch, point_variables, &mut incidence, target_segment)?;
    let bindings = || {
        vec![
            AuditBinding::new("source", source_value.label()),
            AuditBinding::new("target_segment", target_value.label()),
            AuditBinding::new("side", format!("{side:?}")),
            AuditBinding::new("orientation", format!("{orientation:?}")),
            AuditBinding::new("target", target.to_string()),
        ]
    };
    let (row_count, scales, rows) = match mode {
        LineOffsetResidualMode::SupportingLine => (
            2,
            vec![1.0, sketch.model_scale],
            vec![
                audit_row(
                    "cross(unit(source), unit(oriented_target))".into(),
                    bindings(),
                ),
                audit_row(
                    "(dot(oriented_target.start - source.start, left_normal(unit(source))) - signed_target) / model_scale".into(),
                    bindings(),
                ),
            ],
        ),
        LineOffsetResidualMode::ExactTranslatedSegment => (
            4,
            vec![sketch.model_scale; 4],
            [
                ("start.x", "x"),
                ("start.y", "y"),
                ("end.x", "x"),
                ("end.y", "y"),
            ]
                .into_iter()
                .map(|(endpoint_coordinate, normal_coordinate)| {
                    audit_row(
                        format!(
                            "(oriented_target.{endpoint_coordinate} - source.{endpoint_coordinate} - signed_target_normal.{normal_coordinate}) / model_scale"
                        ),
                        bindings(),
                    )
                })
                .collect(),
        ),
    };
    let residual_id = problem.add_residual(ResidualBlock::new(
        source_id,
        ResidualCategory::Hard,
        incidence.variables,
        row_count,
        scales,
        rows,
        LineOffsetResidual {
            source: source_indices,
            target_segment: target_indices,
            target,
            side,
            orientation,
            mode,
        },
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

#[allow(clippy::too_many_arguments)]
fn compile_external_point_target(
    sketch: &Sketch,
    problem: &mut Problem,
    point_variables: &[PointVariableMapping],
    source: SketchSource,
    point: PointId,
    target: Point2<f64>,
    provenance: crate::ExternalConstraintProvenance,
    label: String,
) -> Result<SketchSourceMapping, SketchError> {
    let point_name = sketch.point_name(point)?;
    let source_id = problem.add_source(SourceConstraint::new(&label)?);
    let mut bindings = external_provenance_bindings(provenance);
    bindings.push(AuditBinding::new("native point", point_name));
    bindings.push(AuditBinding::new(
        "external target",
        format!("({}, {})", target.x, target.y),
    ));
    let residual_id = problem.add_residual(ResidualBlock::new(
        source_id,
        ResidualCategory::Hard,
        vec![point_variable(point_variables, point)?],
        2,
        vec![sketch.model_scale, sketch.model_scale],
        vec![
            audit_row(
                format!("({point_name}.x - external.x) / model_scale"),
                bindings.clone(),
            ),
            audit_row(
                format!("({point_name}.y - external.y) / model_scale"),
                bindings,
            ),
        ],
        PointTargetResidual {
            target: [target.x, target.y],
        },
    )?)?;
    Ok(equation_mapping(source, label, source_id, residual_id))
}

fn external_provenance_bindings(
    provenance: crate::ExternalConstraintProvenance,
) -> Vec<AuditBinding> {
    let kind_name = |kind| match kind {
        crate::ExternalFeatureKindV1::Point => "point",
        crate::ExternalFeatureKindV1::LineSegment => "line_segment",
    };
    let mut bindings = vec![
        AuditBinding::new("external binding", provenance.binding.to_string()),
        AuditBinding::new(
            "external expected kind",
            kind_name(provenance.expected_kind),
        ),
        AuditBinding::new("external actual kind", kind_name(provenance.actual_kind)),
        AuditBinding::new(
            "external feature scale",
            provenance.feature_scale.to_string(),
        ),
        AuditBinding::new("external set revision", provenance.set_revision.to_string()),
        AuditBinding::new(
            "external set digest",
            hex_digest(provenance.set_digest.bytes()),
        ),
        AuditBinding::new(
            "external source revision",
            provenance.source_revision.to_string(),
        ),
        AuditBinding::new(
            "external source digest",
            hex_digest(provenance.source_digest.bytes()),
        ),
    ];
    if let Some([domain_start, domain_end]) = provenance.line_domain {
        bindings.push(AuditBinding::new(
            "external line domain",
            format!("[{domain_start}, {domain_end}]"),
        ));
    }
    if let Some(orientation) = provenance.line_orientation {
        let value = match orientation {
            crate::ExternalLineOrientationV1::StartToEnd => "start_to_end",
        };
        bindings.push(AuditBinding::new("external line orientation", value));
    }
    if let Some(digest) = provenance.line_topology_digest {
        bindings.push(AuditBinding::new(
            "external line topology digest",
            hex_digest(digest.bytes()),
        ));
    }
    bindings
}

fn hex_digest(bytes: [u8; 32]) -> String {
    use std::fmt::Write as _;

    let mut value = String::with_capacity(64);
    for byte in bytes {
        write!(&mut value, "{byte:02x}").expect("writing to a String cannot fail");
    }
    value
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
    arc_angle_variables: &[ArcAngleVariableMapping],
    conic_vector_variables: &[ConicVectorVariableMapping],
    conic_scalar_variables: &[ConicScalarVariableMapping],
    nurbs_weight_variables: &[NurbsWeightVariableMapping],
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
            let start_angle =
                arc_angle_variable_optional(arc_angle_variables, arc, ArcAngleRole::Start).map_or(
                    CurveParameterIncidence::Fixed(value.start_angle()),
                    |variable| CurveParameterIncidence::Variable(incidence.add(variable)),
                );
            let end_angle =
                arc_angle_variable_optional(arc_angle_variables, arc, ArcAngleRole::End).map_or(
                    CurveParameterIncidence::Fixed(value.end_angle()),
                    |variable| CurveParameterIncidence::Variable(incidence.add(variable)),
                );
            if matches!(start_angle, CurveParameterIncidence::Variable(_))
                != matches!(end_angle, CurveParameterIncidence::Variable(_))
            {
                return Err(geosolve_core::CoreError::InvalidSolverConfig {
                    field: "associated arc angle incidence",
                    message: "associated output arc must include both endpoint angles",
                }
                .into());
            }
            Ok(GenericCurveIncidence::Arc {
                center: incidence.add(point_variable(point_variables, value.center())?),
                radius: incidence.add(arc_radius_variable(arc_radius_variables, arc)?),
                start_angle,
                end_angle,
                turn_offset: retained_arc_turn_offset(value)?,
                sweep: value.sweep(),
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
        SketchCurve::BSpline { spline, span } => {
            let value = sketch
                .bspline(spline)
                .ok_or(SketchError::UnknownBSpline(spline))?;
            let active = value.basis().span(span).ok_or_else(|| {
                SketchError::InvalidBSplineEvaluation(
                    geosolve_geometry::BSplineEvaluationError::InvalidSpan {
                        ordinal: span.ordinal(),
                    },
                )
            })?;
            let controls = active
                .support()
                .iter()
                .map(|index| {
                    let control = value.controls()[*index];
                    Ok(incidence.add(point_variable(point_variables, control)?))
                })
                .collect::<Result<Vec<_>, SketchError>>()?;
            Ok(GenericCurveIncidence::BSpline {
                basis: value.basis().clone(),
                span,
                controls,
                parameter,
            })
        }
        SketchCurve::Nurbs { nurbs, span } => {
            let value = sketch
                .nurbs(nurbs)
                .ok_or(SketchError::UnknownNurbs(nurbs))?;
            let active = value.basis().span(span).ok_or_else(|| {
                SketchError::InvalidNurbsEvaluation(geosolve_geometry::NurbsEvaluationError::Basis(
                    geosolve_geometry::BSplineEvaluationError::InvalidSpan {
                        ordinal: span.ordinal(),
                    },
                ))
            })?;
            let controls = active
                .support()
                .iter()
                .map(|index| {
                    Ok(incidence.add(point_variable(point_variables, value.controls()[*index])?))
                })
                .collect::<Result<Vec<_>, SketchError>>()?;
            let weights = active
                .support()
                .iter()
                .map(|index| {
                    if *index == value.gauge_index() {
                        Ok(NurbsWeightIncidence::Fixed(value.weights()[*index]))
                    } else {
                        Ok(NurbsWeightIncidence::Variable(incidence.add(
                            nurbs_weight_variable(nurbs_weight_variables, nurbs, *index)?,
                        )))
                    }
                })
                .collect::<Result<Vec<_>, SketchError>>()?;
            Ok(GenericCurveIncidence::Nurbs {
                basis: value.basis().clone(),
                span,
                controls,
                weights,
                parameter,
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
    let bounds = match contact.neighborhood {
        CurveContactNeighborhood::Local { lower, upper } => Some((lower, upper)),
        _ if bounded => Some(match contact.neighborhood {
            CurveContactNeighborhood::Start => (0.0, 0.0),
            CurveContactNeighborhood::End => (1.0, 1.0),
            CurveContactNeighborhood::Interior => (0.0, 1.0),
            CurveContactNeighborhood::Local { .. } => unreachable!(),
        }),
        _ => None,
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
        SketchCurve::BSpline { spline, .. } => sketch
            .bspline(spline)
            .map(crate::BSplineCurve::label)
            .ok_or(SketchError::UnknownBSpline(spline)),
        SketchCurve::Nurbs { nurbs, .. } => sketch
            .nurbs(nurbs)
            .map(crate::NurbsCurve::label)
            .ok_or(SketchError::UnknownNurbs(nurbs)),
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

fn arc_angle_variable_optional(
    mappings: &[ArcAngleVariableMapping],
    arc: ArcId,
    role: ArcAngleRole,
) -> Option<VariableId> {
    mappings.iter().find_map(|mapping| {
        (mapping.arc_id == arc && mapping.role == role).then_some(mapping.variable_id)
    })
}

fn arc_angle_variable(
    mappings: &[ArcAngleVariableMapping],
    arc: ArcId,
    role: ArcAngleRole,
) -> Result<VariableId, SketchError> {
    arc_angle_variable_optional(mappings, arc, role).ok_or_else(|| {
        geosolve_core::CoreError::InvalidSolverConfig {
            field: "associated arc angle mapping",
            message: "active fillet output arc has no endpoint angle coordinate",
        }
        .into()
    })
}

#[allow(clippy::cast_possible_truncation)]
fn retained_arc_turn_offset(arc: &crate::CircularArc) -> Result<i32, SketchError> {
    validate_incident_arc_sweep(arc.signed_sweep(), arc.sweep())?;
    let direct = arc.end_angle() - arc.start_angle();
    let turns = (arc.signed_sweep() - direct) / std::f64::consts::TAU;
    if !direct.is_finite()
        || !turns.is_finite()
        || turns < f64::from(i32::MIN)
        || turns > f64::from(i32::MAX)
    {
        return Err(SketchError::InvalidArcSweep);
    }
    let integer_turns = turns.round() as i32;
    let reconstructed = direct + f64::from(integer_turns) * std::f64::consts::TAU;
    let tolerance = 64.0 * f64::EPSILON * arc.signed_sweep().abs().max(1.0);
    if !reconstructed.is_finite() || (reconstructed - arc.signed_sweep()).abs() > tolerance {
        return Err(SketchError::InvalidArcSweep);
    }
    validate_incident_arc_sweep(reconstructed, arc.sweep())?;
    Ok(integer_turns)
}

fn validate_incident_arc_sweep(
    signed_sweep: f64,
    sweep: crate::ArcSweep,
) -> Result<(), SketchError> {
    let direction_valid = match sweep {
        crate::ArcSweep::CounterClockwise => signed_sweep > 0.0,
        crate::ArcSweep::Clockwise => signed_sweep < 0.0,
    };
    if signed_sweep.is_finite()
        && signed_sweep != 0.0
        && signed_sweep.abs() < std::f64::consts::TAU
        && direction_valid
    {
        Ok(())
    } else {
        Err(SketchError::InvalidArcSweep)
    }
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

fn nurbs_weight_variable(
    mappings: &[NurbsWeightVariableMapping],
    nurbs: NurbsId,
    control_index: usize,
) -> Result<VariableId, SketchError> {
    mappings
        .iter()
        .find_map(|mapping| {
            (mapping.nurbs_id == nurbs && mapping.control_index == control_index)
                .then_some(mapping.variable_id)
        })
        .ok_or(SketchError::InvalidNurbsWeightIndex {
            nurbs,
            index: control_index,
        })
}

fn scalar_property_variable(
    circle_mappings: &[CircleRadiusVariableMapping],
    arc_mappings: &[ArcRadiusVariableMapping],
    arc_angle_mappings: &[ArcAngleVariableMapping],
    conic_mappings: &[ConicScalarVariableMapping],
    nurbs_mappings: &[NurbsWeightVariableMapping],
    property: SketchScalarRef,
) -> Result<VariableId, SketchError> {
    match property {
        SketchScalarRef::CircleRadius(circle) => circle_radius_variable(circle_mappings, circle),
        SketchScalarRef::ArcRadius(arc) => arc_radius_variable(arc_mappings, arc),
        SketchScalarRef::ArcAngle { arc, endpoint } => arc_angle_variable(
            arc_angle_mappings,
            arc,
            match endpoint {
                ArcAngleEndpoint::Start => ArcAngleRole::Start,
                ArcAngleEndpoint::End => ArcAngleRole::End,
            },
        ),
        SketchScalarRef::ConicScalar { conic, role } => {
            conic_scalar_variable(conic_mappings, conic, role)
        }
        SketchScalarRef::NurbsWeight {
            nurbs,
            control_index,
        } => nurbs_weight_variable(nurbs_mappings, nurbs, control_index),
    }
}

fn nurbs_weight_value(
    problem: &Problem,
    mappings: &[NurbsWeightVariableMapping],
    nurbs: NurbsId,
    control_index: usize,
) -> Result<f64, SketchError> {
    scalar_variable(
        problem,
        nurbs_weight_variable(mappings, nurbs, control_index)?,
    )
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
        | SketchCurve::Bezier(_)
        | SketchCurve::BSpline { .. }
        | SketchCurve::Nurbs { .. } => true,
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
        SketchCurve::Arc(_)
        | SketchCurve::Bezier(_)
        | SketchCurve::Conic(_)
        | SketchCurve::BSpline { .. }
        | SketchCurve::Nurbs { .. } => unreachable!(),
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
    let bounded = generic_curve_is_bounded(sketch, contact.curve);
    if bounded {
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
    } else {
        let neighborhood_valid = match contact.neighborhood {
            CurveContactNeighborhood::Interior => true,
            CurveContactNeighborhood::Local { lower, upper } => {
                lower.is_finite() && upper.is_finite() && parameter > lower && parameter < upper
            }
            CurveContactNeighborhood::Start | CurveContactNeighborhood::End => false,
        };
        if !neighborhood_valid {
            return Err(SolveRejection::AmbiguousContactNeighborhood(constraint));
        }
    }
    candidate_curve_jet(sketch, candidate, contact.curve, parameter)
        .map_err(|error| candidate_curve_rejection(constraint, error))
}

fn validate_independent_constraint_rows(
    constraint: SketchConstraintId,
    rows: &[f64],
    tolerance: f64,
) -> Result<f64, SolveRejection> {
    let maximum = rows.iter().map(|value| value.abs()).fold(0.0, f64::max);
    if !maximum.is_finite() || maximum > tolerance {
        Err(SolveRejection::IndependentConstraintResidual {
            constraint,
            maximum,
            tolerance,
        })
    } else {
        Ok(maximum)
    }
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn validate_curve_fillet_candidate(
    sketch: &Sketch,
    candidate: &SolvedSketchState,
    constraint: SketchConstraintId,
    arc: ArcId,
    first: SketchCurveContact,
    first_side: crate::CurveNormalSide,
    second: SketchCurveContact,
    second_side: crate::CurveNormalSide,
    endpoint_order: crate::FilletEndpointOrder,
    tolerance: f64,
) -> Result<f64, SolveRejection> {
    let first_parameter = latent_value(
        &candidate.latents,
        constraint,
        LatentVariableRole::FirstCurveParameter,
    )?;
    let second_parameter = latent_value(
        &candidate.latents,
        constraint,
        LatentVariableRole::SecondCurveParameter,
    )?;
    let first_jet =
        validate_generic_contact_candidate(sketch, candidate, constraint, first, first_parameter)?;
    let second_jet = validate_generic_contact_candidate(
        sketch,
        candidate,
        constraint,
        second,
        second_parameter,
    )?;
    let first_differential = first_jet
        .differential()
        .map_err(|_| SolveRejection::DegenerateCurve(constraint))?;
    let second_differential = second_jet
        .differential()
        .map_err(|_| SolveRejection::DegenerateCurve(constraint))?;
    let first_tangent = first_differential.unit_tangent;
    let second_tangent = second_differential.unit_tangent;
    let tangent_cross = cross_2d(first_tangent, second_tangent);
    if !tangent_cross.is_finite() || tangent_cross.abs() <= CURVE_FILLET_REGULARITY_THRESHOLD {
        return Err(SolveRejection::InvalidFilletGeometry(constraint));
    }
    let solved_arc = candidate.geometry.arc(arc).ok_or_else(|| {
        SolveRejection::IndependentValidationFailed(
            "curve fillet output arc is missing from candidate geometry".into(),
        )
    })?;
    if !solved_arc.radius.is_finite() || solved_arc.radius <= 0.0 {
        return Err(SolveRejection::NonPositiveArcRadius(arc));
    }
    let first_normal = Vector2::new(-first_tangent.y, first_tangent.x);
    let second_normal = Vector2::new(-second_tangent.y, second_tangent.x);
    let first_sign = fillet_side_sign(first_side);
    let second_sign = fillet_side_sign(second_side);
    let first_offset_regularity =
        1.0 - first_sign * solved_arc.radius * first_differential.signed_curvature;
    let second_offset_regularity =
        1.0 - second_sign * solved_arc.radius * second_differential.signed_curvature;
    if !first_offset_regularity.is_finite()
        || !second_offset_regularity.is_finite()
        || first_offset_regularity.abs() <= CURVE_FILLET_REGULARITY_THRESHOLD
        || second_offset_regularity.abs() <= CURVE_FILLET_REGULARITY_THRESHOLD
    {
        return Err(SolveRejection::InvalidFilletGeometry(constraint));
    }
    let first_offset = solved_arc.center - first_jet.position;
    let second_offset = solved_arc.center - second_jet.position;
    if first_sign * first_offset.dot(&first_normal) <= 0.0
        || second_sign * second_offset.dot(&second_normal) <= 0.0
    {
        return Err(SolveRejection::FilletSideFlipped(constraint));
    }
    let first_expected = first_normal * (first_sign * solved_arc.radius);
    let second_expected = second_normal * (second_sign * solved_arc.radius);
    let first_radial = first_jet.position - solved_arc.center;
    let second_radial = second_jet.position - solved_arc.center;
    let first_radial_norm = first_radial.norm();
    let second_radial_norm = second_radial.norm();
    if !first_radial_norm.is_finite()
        || !second_radial_norm.is_finite()
        || first_radial_norm == 0.0
        || second_radial_norm == 0.0
    {
        return Err(SolveRejection::InvalidFilletGeometry(constraint));
    }
    let (expected_start, expected_end) = match endpoint_order {
        crate::FilletEndpointOrder::FirstThenSecond => (first_jet.position, second_jet.position),
        crate::FilletEndpointOrder::SecondThenFirst => (second_jet.position, first_jet.position),
    };
    let expected_start_offset = expected_start - solved_arc.center;
    let expected_end_offset = expected_end - solved_arc.center;
    let expected_start_norm = expected_start_offset.norm();
    let expected_end_norm = expected_end_offset.norm();
    if !expected_start_norm.is_finite()
        || !expected_end_norm.is_finite()
        || expected_start_norm == 0.0
        || expected_end_norm == 0.0
    {
        return Err(SolveRejection::InvalidFilletGeometry(constraint));
    }
    let expected_start_radial = expected_start_offset / expected_start_norm;
    let expected_end_radial = expected_end_offset / expected_end_norm;
    let output_start_radial =
        Vector2::new(solved_arc.start_angle.cos(), solved_arc.start_angle.sin());
    let output_end_radial = Vector2::new(solved_arc.end_angle.cos(), solved_arc.end_angle.sin());
    if output_start_radial.dot(&expected_start_radial) <= 0.0
        || output_end_radial.dot(&expected_end_radial) <= 0.0
    {
        return Err(SolveRejection::InvalidFilletGeometry(constraint));
    }
    let expected_start_angle = expected_start_offset.y.atan2(expected_start_offset.x);
    let expected_end_angle = expected_end_offset.y.atan2(expected_end_offset.x);
    let expected_signed_sweep =
        arc_signed_sweep(expected_start_angle, expected_end_angle, solved_arc.sweep)
            .map_err(|_| SolveRejection::InvalidFilletGeometry(constraint))?;
    let stored_signed_sweep = arc_signed_sweep(
        solved_arc.start_angle,
        solved_arc.end_angle,
        solved_arc.sweep,
    )
    .map_err(|_| SolveRejection::InvalidFilletGeometry(constraint))?;
    let (actual_start, actual_end) = solved_arc
        .endpoints()
        .ok_or(SolveRejection::InvalidFilletGeometry(constraint))?;
    let sweep_valid = match solved_arc.sweep {
        crate::ArcSweep::CounterClockwise => solved_arc.signed_sweep > 0.0,
        crate::ArcSweep::Clockwise => solved_arc.signed_sweep < 0.0,
    };
    if !sweep_valid
        || !solved_arc.start_angle.is_finite()
        || !solved_arc.end_angle.is_finite()
        || !solved_arc.signed_sweep.is_finite()
        || solved_arc.signed_sweep.abs() >= std::f64::consts::TAU
    {
        return Err(SolveRejection::InvalidFilletGeometry(constraint));
    }
    let first_center_error = first_offset - first_expected;
    let second_center_error = second_offset - second_expected;
    let start_error = actual_start - expected_start;
    let end_error = actual_end - expected_end;
    validate_independent_constraint_rows(
        constraint,
        &[
            first_center_error.x / sketch.model_scale,
            first_center_error.y / sketch.model_scale,
            second_center_error.x / sketch.model_scale,
            second_center_error.y / sketch.model_scale,
            (first_radial_norm - solved_arc.radius) / sketch.model_scale,
            (second_radial_norm - solved_arc.radius) / sketch.model_scale,
            first_tangent.dot(&(first_radial / first_radial_norm)),
            second_tangent.dot(&(second_radial / second_radial_norm)),
            start_error.x / sketch.model_scale,
            start_error.y / sketch.model_scale,
            end_error.x / sketch.model_scale,
            end_error.y / sketch.model_scale,
            cross_2d(output_start_radial, expected_start_radial),
            cross_2d(output_end_radial, expected_end_radial),
            stored_signed_sweep - solved_arc.signed_sweep,
            expected_signed_sweep - solved_arc.signed_sweep,
        ],
        tolerance,
    )
}

const fn fillet_side_sign(side: crate::CurveNormalSide) -> f64 {
    match side {
        crate::CurveNormalSide::Left => 1.0,
        crate::CurveNormalSide::Right => -1.0,
    }
}

fn validate_line_offset_candidate(
    sketch: &Sketch,
    candidate: &SolvedSketchState,
    dimension: SketchDimensionId,
    kind: DimensionKind,
    tolerance: f64,
) -> Result<f64, SolveRejection> {
    let (source, target_segment, target, side, orientation, exact) = match kind {
        DimensionKind::SupportingLineOffset {
            source,
            target_segment,
            target,
            side,
            orientation,
        } => (source, target_segment, target, side, orientation, false),
        DimensionKind::ExactTranslatedSegmentOffset {
            source,
            target_segment,
            target,
            side,
            orientation,
        } => (source, target_segment, target, side, orientation, true),
        _ => unreachable!("non-offset dimension reached offset validator"),
    };
    let source_value = sketch.segments.get(source).ok_or_else(|| {
        SolveRejection::IndependentValidationFailed(
            "line offset references a stale source segment".into(),
        )
    })?;
    let target_value = sketch.segments.get(target_segment).ok_or_else(|| {
        SolveRejection::IndependentValidationFailed(
            "line offset references a stale target segment".into(),
        )
    })?;
    let source_start = candidate
        .geometry
        .point(source_value.start())
        .ok_or_else(|| {
            SolveRejection::IndependentValidationFailed(
                "line offset source start is missing".into(),
            )
        })?;
    let source_end = candidate
        .geometry
        .point(source_value.end())
        .ok_or_else(|| {
            SolveRejection::IndependentValidationFailed("line offset source end is missing".into())
        })?;
    let native_target_start = candidate
        .geometry
        .point(target_value.start())
        .ok_or_else(|| {
            SolveRejection::IndependentValidationFailed(
                "line offset target start is missing".into(),
            )
        })?;
    let native_target_end = candidate
        .geometry
        .point(target_value.end())
        .ok_or_else(|| {
            SolveRejection::IndependentValidationFailed("line offset target end is missing".into())
        })?;
    let (target_start, target_end) =
        orientation.target_endpoints(native_target_start, native_target_end);
    let source_direction = source_end - source_start;
    let target_direction = target_end - target_start;
    let source_length = source_direction.norm();
    let target_length = target_direction.norm();
    if !source_length.is_finite()
        || !target_length.is_finite()
        || source_length == 0.0
        || target_length == 0.0
    {
        return Err(SolveRejection::LineOffsetBranchFlipped(dimension));
    }
    let source_unit = source_direction / source_length;
    let target_unit = target_direction / target_length;
    let displacement = target_start - source_start;
    let signed_distance = cross_2d(source_unit, displacement);
    if source_unit.dot(&target_unit) <= 0.0 || side.sign() * signed_distance <= 0.0 {
        return Err(SolveRejection::LineOffsetBranchFlipped(dimension));
    }
    let signed_target = side.sign() * target;
    let rows = if exact {
        let normal = Vector2::new(-source_unit.y, source_unit.x) * signed_target;
        let start_error = target_start - source_start - normal;
        let end_error = target_end - source_end - normal;
        vec![
            start_error.x / sketch.model_scale,
            start_error.y / sketch.model_scale,
            end_error.x / sketch.model_scale,
            end_error.y / sketch.model_scale,
        ]
    } else {
        vec![
            cross_2d(source_unit, target_unit),
            (signed_distance - signed_target) / sketch.model_scale,
        ]
    };
    validate_independent_dimension_rows(dimension, &rows, tolerance)
}

fn validate_independent_dimension_rows(
    dimension: SketchDimensionId,
    rows: &[f64],
    tolerance: f64,
) -> Result<f64, SolveRejection> {
    let maximum = rows.iter().map(|value| value.abs()).fold(0.0, f64::max);
    if !maximum.is_finite() || maximum > tolerance {
        Err(SolveRejection::IndependentDimensionResidual {
            dimension,
            maximum,
            tolerance,
        })
    } else {
        Ok(maximum)
    }
}

fn cross_2d(first: Vector2<f64>, second: Vector2<f64>) -> f64 {
    first.x * second.y - first.y * second.x
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

#[derive(Clone, Copy, Debug)]
enum CandidateCurveError {
    Other,
    InvalidNurbs {
        nurbs: NurbsId,
        source: geosolve_geometry::NurbsDefinitionError,
    },
    NurbsEvaluation {
        nurbs: NurbsId,
        source: geosolve_geometry::NurbsEvaluationError,
    },
}

fn candidate_curve_rejection(
    constraint: SketchConstraintId,
    error: CandidateCurveError,
) -> SolveRejection {
    match error {
        CandidateCurveError::Other => SolveRejection::DegenerateCurve(constraint),
        CandidateCurveError::InvalidNurbs { nurbs, source } => {
            SolveRejection::InvalidNurbsEntity { nurbs, source }
        }
        CandidateCurveError::NurbsEvaluation { nurbs, source } => SolveRejection::NurbsEvaluation {
            constraint,
            nurbs,
            source,
        },
    }
}

fn candidate_nurbs_geometry(
    sketch: &Sketch,
    candidate: &SolvedSketchState,
    nurbs: NurbsId,
) -> Result<geosolve_geometry::NurbsCurve2, geosolve_geometry::NurbsDefinitionError> {
    let curve = sketch
        .nurbs(nurbs)
        .expect("candidate NURBS has validated runtime identity");
    let controls = curve
        .controls()
        .iter()
        .map(|control| {
            candidate
                .geometry
                .point(*control)
                .expect("candidate NURBS has every compiled control")
        })
        .collect();
    let weights = candidate
        .geometry
        .nurbs(nurbs)
        .expect("candidate NURBS has compiled solved weights")
        .weights
        .clone();
    geosolve_geometry::NurbsCurve2::try_new(curve.basis().clone(), controls, weights)
}

fn candidate_curve_jet(
    sketch: &Sketch,
    candidate: &SolvedSketchState,
    curve: SketchCurve,
    parameter: f64,
) -> Result<geosolve_geometry::CurveJet2, CandidateCurveError> {
    match curve {
        SketchCurve::Line { segment, domain } => {
            let segment = sketch
                .segments
                .get(segment)
                .ok_or(CandidateCurveError::Other)?;
            let start = candidate
                .geometry
                .point(segment.start())
                .ok_or(CandidateCurveError::Other)?;
            let end = candidate
                .geometry
                .point(segment.end())
                .ok_or(CandidateCurveError::Other)?;
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
            .map_err(|_| CandidateCurveError::Other)
        }
        SketchCurve::Circle(circle) => {
            let circle = candidate
                .geometry
                .circle(circle)
                .ok_or(CandidateCurveError::Other)?;
            geosolve_geometry::circle_jet(circle.center, circle.radius, parameter)
                .map_err(|_| CandidateCurveError::Other)
        }
        SketchCurve::Arc(arc) => {
            let arc = candidate
                .geometry
                .arc(arc)
                .ok_or(CandidateCurveError::Other)?;
            geosolve_geometry::circular_arc_jet(
                arc.center,
                arc.radius,
                arc.start_angle,
                arc.signed_sweep,
                parameter,
            )
            .map_err(|_| CandidateCurveError::Other)
        }
        SketchCurve::Bezier(bezier) => candidate_bezier_jet(sketch, candidate, bezier, parameter)
            .map_err(|_| CandidateCurveError::Other),
        SketchCurve::Conic(conic) => candidate
            .geometry
            .conic(conic)
            .ok_or(CandidateCurveError::Other)?
            .evaluate(parameter)
            .map_err(|_| CandidateCurveError::Other),
        SketchCurve::BSpline { spline, span } => {
            let curve = sketch.bspline(spline).ok_or(CandidateCurveError::Other)?;
            let controls = curve
                .controls()
                .iter()
                .map(|control| {
                    candidate
                        .geometry
                        .point(*control)
                        .ok_or(CandidateCurveError::Other)
                })
                .collect::<Result<Vec<_>, _>>()?;
            geosolve_geometry::BSplineCurve2::try_new(curve.basis().clone(), controls)
                .map_err(|_| CandidateCurveError::Other)?
                .jet_on_span(span, parameter)
                .map_err(|_| CandidateCurveError::Other)
        }
        SketchCurve::Nurbs { nurbs, span } => candidate_nurbs_geometry(sketch, candidate, nurbs)
            .map_err(|source| CandidateCurveError::InvalidNurbs { nurbs, source })?
            .jet_on_span(span, parameter)
            .map_err(|source| CandidateCurveError::NurbsEvaluation { nurbs, source }),
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
    use crate::{
        CurveContactNeighborhood, CurveContinuity, CurveCurvatureRelation, CurveDirectionRelation,
        CurveNormalSide, LineParameterDomain, LineSide, SketchCurve, SketchCurveContact,
    };

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

        let rejection = sketch
            .validate_m7_candidate(&candidate, SKETCH_ACCEPTANCE_RESIDUAL_TOLERANCE)
            .unwrap_err();
        assert!(matches!(
            rejection,
            SolveRejection::IndependentValidationFailed(_)
        ));
        report.hard_validity = domain_hard_validity(report.hard_validity, Some(&rejection));

        assert_eq!(report.hard_validity, HardValidity::NotEvaluated);
        assert!(report.hard_residuals_validated);
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn independent_candidate_validation_recomputes_advanced_rows() {
        let tolerance = SKETCH_ACCEPTANCE_RESIDUAL_TOLERANCE;

        let mut direction_sketch = Sketch::new(1.0).unwrap();
        let line_start = direction_sketch.add_point(Point2::origin()).unwrap();
        let line_end = direction_sketch.add_point(Point2::new(-1.0, 0.0)).unwrap();
        let line = direction_sketch.add_segment(line_start, line_end).unwrap();
        let center = direction_sketch.add_point(Point2::origin()).unwrap();
        let circle = direction_sketch.add_circle(center, 1.0).unwrap();
        let direction = direction_sketch
            .add_curve_direction(
                line,
                SketchCurveContact {
                    curve: SketchCurve::Circle(circle),
                    parameter: 0.0,
                    neighborhood: CurveContactNeighborhood::Interior,
                },
                CurveDirectionRelation::Normal(CurveNormalSide::Left),
            )
            .unwrap();
        let compiled = direction_sketch
            .compile(SketchSolveRequest::default())
            .unwrap();
        let mut candidate = compiled.solved_state(&direction_sketch).unwrap();
        candidate
            .geometry
            .points
            .iter_mut()
            .find(|point| point.point_id == line_end)
            .unwrap()
            .position
            .y = 0.1;
        assert!(matches!(
            direction_sketch.validate_m7_candidate(&candidate, tolerance),
            Err(SolveRejection::IndependentConstraintResidual {
                constraint,
                ..
            }) if constraint == direction
        ));

        let mut curvature_sketch = Sketch::new(1.0).unwrap();
        let first_center = curvature_sketch.add_point(Point2::origin()).unwrap();
        let second_center = curvature_sketch.add_point(Point2::new(4.0, 0.0)).unwrap();
        let first = curvature_sketch.add_circle(first_center, 1.0).unwrap();
        let second = curvature_sketch.add_circle(second_center, 2.0).unwrap();
        let curvature = curvature_sketch
            .add_equal_curvature(
                SketchCurveContact {
                    curve: SketchCurve::Circle(first),
                    parameter: 0.0,
                    neighborhood: CurveContactNeighborhood::Interior,
                },
                SketchCurveContact {
                    curve: SketchCurve::Circle(second),
                    parameter: 0.0,
                    neighborhood: CurveContactNeighborhood::Interior,
                },
                CurveCurvatureRelation::Signed,
            )
            .unwrap();
        let compiled = curvature_sketch
            .compile(SketchSolveRequest::default())
            .unwrap();
        let candidate = compiled.solved_state(&curvature_sketch).unwrap();
        assert!(matches!(
            curvature_sketch.validate_m7_candidate(&candidate, tolerance),
            Err(SolveRejection::IndependentConstraintResidual {
                constraint,
                ..
            }) if constraint == curvature
        ));

        let mut continuity_sketch = Sketch::new(1.0).unwrap();
        let first_start = continuity_sketch.add_point(Point2::new(-1.0, 0.0)).unwrap();
        let seam = continuity_sketch.add_point(Point2::origin()).unwrap();
        let second_end = continuity_sketch.add_point(Point2::new(2.0, 0.0)).unwrap();
        let first_line = continuity_sketch.add_segment(first_start, seam).unwrap();
        let second_line = continuity_sketch.add_segment(seam, second_end).unwrap();
        let continuity = continuity_sketch
            .add_endpoint_continuity(
                SketchCurveContact {
                    curve: SketchCurve::Line {
                        segment: first_line,
                        domain: LineParameterDomain::BoundedSegment,
                    },
                    parameter: 1.0,
                    neighborhood: CurveContactNeighborhood::End,
                },
                SketchCurveContact {
                    curve: SketchCurve::Line {
                        segment: second_line,
                        domain: LineParameterDomain::BoundedSegment,
                    },
                    parameter: 0.0,
                    neighborhood: CurveContactNeighborhood::Start,
                },
                CurveContinuity::ParametricC2 {
                    first_rate: 1.0,
                    second_rate: 1.0,
                },
            )
            .unwrap();
        let compiled = continuity_sketch
            .compile(SketchSolveRequest::default())
            .unwrap();
        let candidate = compiled.solved_state(&continuity_sketch).unwrap();
        assert!(matches!(
            continuity_sketch.validate_m7_candidate(&candidate, tolerance),
            Err(SolveRejection::IndependentConstraintResidual {
                constraint,
                ..
            }) if constraint == continuity
        ));
    }

    #[test]
    fn independent_candidate_validation_preserves_nurbs_conditioning_error() {
        let mut sketch = Sketch::new(1.0).unwrap();
        let controls = [
            Point2::origin(),
            Point2::new(0.5, 1.0),
            Point2::new(1.0, 0.0),
        ]
        .map(|point| sketch.add_point(point).unwrap());
        let nurbs = sketch
            .add_named_nurbs(
                "conditioned line",
                geosolve_geometry::BSplineForm::Clamped,
                2,
                controls.to_vec(),
                vec![1.0, 1.0, 1.0],
                0,
                vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
            )
            .unwrap();
        let mut compiled = sketch.compile(SketchSolveRequest::default()).unwrap();
        for (index, value) in [(1, f64::from_bits(1)), (2, f64::MAX)] {
            let variable = compiled
                .nurbs_weight_variables
                .iter()
                .find(|mapping| mapping.nurbs_id == nurbs && mapping.control_index == index)
                .unwrap()
                .variable_id;
            compiled
                .problem
                .set_variable_value(variable, VariableValue::Scalar(value))
                .unwrap();
        }
        let candidate = compiled.solved_state(&sketch).unwrap();

        assert!(matches!(
            sketch.validate_m7_candidate(&candidate, SKETCH_ACCEPTANCE_RESIDUAL_TOLERANCE),
            Err(SolveRejection::InvalidNurbsEntity {
                nurbs: rejected,
                source: geosolve_geometry::NurbsDefinitionError::MixedWeightScale { .. },
            }) if rejected == nurbs
        ));

        for (index, value) in [(1, -1.0), (2, 1.0)] {
            let variable = compiled
                .nurbs_weight_variables
                .iter()
                .find(|mapping| mapping.nurbs_id == nurbs && mapping.control_index == index)
                .unwrap()
                .variable_id;
            compiled
                .problem
                .set_variable_value(variable, VariableValue::Scalar(value))
                .unwrap();
        }
        let candidate = compiled.solved_state(&sketch).unwrap();
        assert!(matches!(
            sketch.validate_m7_candidate(&candidate, SKETCH_ACCEPTANCE_RESIDUAL_TOLERANCE),
            Err(SolveRejection::InvalidNurbsEntity {
                nurbs: rejected,
                source: geosolve_geometry::NurbsDefinitionError::InvalidWeight {
                    index: 1,
                    ..
                },
            }) if rejected == nurbs
        ));
    }

    #[test]
    fn solved_nurbs_weights_commit_as_one_transaction() {
        let mut sketch = Sketch::new(1.0).unwrap();
        let controls = [
            Point2::origin(),
            Point2::new(1.0, 0.0),
            Point2::new(2.0, 0.0),
        ]
        .map(|point| sketch.add_point(point).unwrap());
        let nurbs = sketch
            .add_named_nurbs(
                "locally scaled polyline",
                geosolve_geometry::BSplineForm::Clamped,
                1,
                controls.to_vec(),
                vec![1.0, 1.0, f64::MAX],
                0,
                vec![0.0, 0.0, 1.0, 2.0, 2.0],
            )
            .unwrap();
        let mut candidate = SolvedSketchState {
            geometry: sketch.geometry(),
            latents: Vec::new(),
        };
        candidate.geometry.nurbs[0].weights = vec![1.0, 1.0e-200, 1.0e-200];
        sketch.commit_solved_state(&candidate).unwrap();
        assert_eq!(
            sketch.nurbs(nurbs).unwrap().weights(),
            [1.0, 1.0e-200, 1.0e-200]
        );

        let retained = sketch.point(controls[0]).unwrap().position();
        let mut rejected = SolvedSketchState {
            geometry: sketch.geometry(),
            latents: Vec::new(),
        };
        rejected.geometry.points[0].position = Point2::new(9.0, 9.0);
        rejected.geometry.nurbs[0].weights = vec![1.0, f64::from_bits(1), f64::MAX];
        assert!(matches!(
            sketch.commit_solved_state(&rejected),
            Err(SketchError::InvalidNurbs(
                geosolve_geometry::NurbsDefinitionError::MixedWeightScale { .. }
            ))
        ));
        assert_eq!(sketch.point(controls[0]).unwrap().position(), retained);
    }

    #[test]
    fn curve_fillet_validation_recomputes_endpoint_and_sweep_state() {
        let mut sketch = Sketch::new(1.0).unwrap();
        let first_start = sketch.add_point(Point2::new(0.0, 0.0)).unwrap();
        let corner = sketch.add_point(Point2::new(4.0, 0.0)).unwrap();
        let second_end = sketch.add_point(Point2::new(4.0, 4.0)).unwrap();
        let center = sketch.add_point(Point2::new(3.0, 1.0)).unwrap();
        let first = sketch.add_segment(first_start, corner).unwrap();
        let second = sketch.add_segment(corner, second_end).unwrap();
        for point in [first_start, corner, second_end] {
            sketch.add_fixed_point(point).unwrap();
        }
        let arc = sketch
            .add_arc(
                center,
                1.0,
                -std::f64::consts::FRAC_PI_2,
                0.0,
                crate::ArcSweep::CounterClockwise,
            )
            .unwrap();
        let contact = |segment, parameter| SketchCurveContact {
            curve: SketchCurve::Line {
                segment,
                domain: LineParameterDomain::BoundedSegment,
            },
            parameter,
            neighborhood: CurveContactNeighborhood::Interior,
        };
        sketch
            .add_line_line_fillet(
                arc,
                contact(first, 0.75),
                CurveNormalSide::Left,
                contact(second, 0.25),
                CurveNormalSide::Left,
                crate::FilletEndpointOrder::FirstThenSecond,
            )
            .unwrap();
        sketch
            .add_arc_radius(arc, 1.0, DimensionMode::Driving)
            .unwrap();
        let mut compiled = sketch.compile(SketchSolveRequest::default()).unwrap();
        let report = compiled.problem.solve(SolverConfig::default()).unwrap();
        assert_eq!(report.termination, SolveTermination::Converged);
        let mut candidate = compiled.solved_state(&sketch).unwrap();
        sketch.normalize_candidate_latents(&mut candidate);
        sketch
            .derive_curve_fillet_arcs(&mut candidate, SKETCH_ACCEPTANCE_RESIDUAL_TOLERANCE)
            .unwrap();
        assert!(
            sketch
                .validate_m7_candidate(&candidate, SKETCH_ACCEPTANCE_RESIDUAL_TOLERANCE)
                .is_ok()
        );

        let solved = candidate
            .geometry
            .arcs
            .iter_mut()
            .find(|value| value.arc_id == arc)
            .unwrap();
        let retained_end = solved.end_angle;
        solved.end_angle += 0.25;
        assert!(matches!(
            sketch.validate_m7_candidate(&candidate, SKETCH_ACCEPTANCE_RESIDUAL_TOLERANCE),
            Err(SolveRejection::IndependentConstraintResidual { .. })
        ));
        let solved = candidate
            .geometry
            .arcs
            .iter_mut()
            .find(|value| value.arc_id == arc)
            .unwrap();
        solved.end_angle = retained_end;
        solved.signed_sweep += TAU;
        assert!(matches!(
            sketch.validate_m7_candidate(&candidate, SKETCH_ACCEPTANCE_RESIDUAL_TOLERANCE),
            Err(SolveRejection::InvalidFilletGeometry(_))
        ));
    }
}
