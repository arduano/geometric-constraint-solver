use std::f64::consts::TAU;

use geosolve_geometry::{Point2, Vector2};

use crate::model::{
    ArcId, CircleId, DimensionKind, DimensionMode, LineSegment, PointId, SegmentId, Sketch,
    SketchConstraintId, SketchConstraintKind, SketchDimensionId, SketchError,
    validate_dimension_value, validate_finite, validate_point,
};

/// Dimensionless roundoff band used only when normalizing solved bounded contacts.
///
/// Values in `[-tol, 0]` or `[1, 1 + tol]` are clamped to the exact endpoint.
/// Larger excursions remain invalid and are never committed.
pub const CONTACT_PARAMETER_ROUNDOFF_TOLERANCE: f64 = 64.0 * f64::EPSILON;

/// Minimum positive direction cosine required by a center-direction branch.
///
/// This angular ambiguity policy is independent of model scale, feature size,
/// and nonlinear residual tolerance. Values at or below the margin are treated
/// as orthogonal/ambiguous rather than as a selected positive branch.
pub const CENTER_DIRECTION_COSINE_MARGIN: f64 = 1.0e-8;

/// Maximum relative error accepted between a solved circle radius and the
/// radius derived from a circle-arc tangency branch.
///
/// The direct center-distance branch relation is compared relative to the
/// circle radius and derived radial gap only. Supporting-arc scale never
/// widens this acceptance tolerance.
pub const CIRCLE_ARC_TANGENCY_RADIUS_RELATIVE_TOLERANCE: f64 = 1.0e-8;

/// Multiplier used to estimate floating uncertainty in subtracting the
/// supporting arc radius from the center distance.
///
/// If this feature-scale uncertainty is larger than the allowed
/// circle-relative error, accepted-state validation reports scale ambiguity
/// instead of widening the tolerance.
pub const CIRCLE_ARC_TANGENCY_SCALE_UNCERTAINTY_MULTIPLIER: f64 = 8.0;

/// Maximum dimensionless cross error and cosine deficit for a selected
/// circle-arc radial contact root.
pub const CIRCLE_ARC_TANGENCY_DIRECTION_TOLERANCE: f64 = 1.0e-8;

/// Direction used to traverse a circular arc from its start angle to its end angle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArcSweep {
    CounterClockwise,
    Clockwise,
}

/// Parameter policy for a directed line segment used as a curve.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LineParameterDomain {
    /// The segment endpoints define an infinite supporting line; any finite parameter is valid.
    SupportingLine,
    /// The parameter is restricted to the closed segment interval `[0, 1]`.
    BoundedSegment,
}

impl LineParameterDomain {
    #[must_use]
    pub fn contains(self, parameter: f64) -> bool {
        parameter.is_finite()
            && match self {
                Self::SupportingLine => true,
                Self::BoundedSegment => (0.0..=1.0).contains(&parameter),
            }
    }

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::SupportingLine => "supporting-line domain",
            Self::BoundedSegment => "bounded-segment domain [0, 1]",
        }
    }

    pub(crate) fn normalize_candidate(self, parameter: f64) -> Option<f64> {
        if !parameter.is_finite() {
            return None;
        }
        match self {
            Self::SupportingLine => Some(parameter),
            Self::BoundedSegment => normalize_bounded_candidate(parameter),
        }
    }
}

/// Selected side of a directed line.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LineSide {
    Left,
    Right,
}

impl LineSide {
    pub(crate) const fn sign(self) -> f64 {
        match self {
            Self::Left => 1.0,
            Self::Right => -1.0,
        }
    }
}

/// Explicit correspondence between the source and target line endpoints.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LineOffsetOrientation {
    /// The target start corresponds to the source start.
    Same,
    /// The target end corresponds to the source start.
    Reversed,
}

impl LineOffsetOrientation {
    pub(crate) const fn target_endpoints<T: Copy>(self, start: T, end: T) -> (T, T) {
        match self {
            Self::Same => (start, end),
            Self::Reversed => (end, start),
        }
    }
}

/// Which circle contains the other for an internal circle-circle tangency.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CircleContainment {
    FirstContainsSecond,
    SecondContainsFirst,
}

/// Explicit circle-circle tangency branch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CircleTangencyMode {
    External,
    Internal { containment: CircleContainment },
}

/// Explicit radial side of a circle tangent to a bounded circular arc.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArcCircleTangencySide {
    /// The circle center is farther from the arc center than the arc radius.
    OutsideArc,
    /// The circle center is noncoincident with and inside the arc's supporting circle.
    InsideArc,
}

impl ArcCircleTangencySide {
    pub(crate) fn accepts(self, center_distance: f64, arc_radius: f64) -> bool {
        center_distance.is_finite()
            && arc_radius.is_finite()
            && match self {
                Self::OutsideArc => center_distance > arc_radius,
                Self::InsideArc => center_distance > 0.0 && center_distance < arc_radius,
            }
    }

    pub(crate) const fn circle_arc_radial_sign(self) -> f64 {
        match self {
            Self::OutsideArc => -1.0,
            Self::InsideArc => 1.0,
        }
    }
}

/// Direction in which the second circle center must remain from the first.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CenterDirectionBranch {
    reference_direction: [f64; 2],
}

impl CenterDirectionBranch {
    /// Creates a branch from a finite nonzero direction and stores its normalized value.
    ///
    /// # Errors
    ///
    /// Returns [`SketchError::InvalidDirectionBranch`] for a zero or non-finite direction.
    pub fn new(reference_direction: [f64; 2]) -> Result<Self, SketchError> {
        let norm = reference_direction[0].hypot(reference_direction[1]);
        if !norm.is_finite() || norm == 0.0 {
            return Err(SketchError::InvalidDirectionBranch);
        }
        Ok(Self {
            reference_direction: [reference_direction[0] / norm, reference_direction[1] / norm],
        })
    }

    #[must_use]
    pub const fn positive_x() -> Self {
        Self {
            reference_direction: [1.0, 0.0],
        }
    }

    #[must_use]
    pub const fn reference_direction(self) -> [f64; 2] {
        self.reference_direction
    }

    /// Signed physical projection from the first center toward the second.
    #[must_use]
    pub fn projection(self, first: Point2<f64>, second: Point2<f64>) -> f64 {
        let displacement = second - first;
        displacement.x * self.reference_direction[0] + displacement.y * self.reference_direction[1]
    }

    /// Cosine between the selected branch direction and center displacement.
    #[must_use]
    pub fn direction_cosine(self, first: Point2<f64>, second: Point2<f64>) -> Option<f64> {
        let displacement = second - first;
        let distance = displacement.norm();
        let projection = self.projection(first, second);
        (projection.is_finite() && distance.is_finite() && distance > 0.0)
            .then_some(projection / distance)
    }

    #[must_use]
    pub fn is_preserved(self, first: Point2<f64>, second: Point2<f64>) -> bool {
        let projection = self.projection(first, second);
        projection.is_finite() && projection > 0.0
    }
}

/// Orientation used to measure an angle from the first directed segment to the second.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AngleOrientation {
    CounterClockwise,
    Clockwise,
}

impl AngleOrientation {
    pub(crate) const fn sign(self) -> f64 {
        match self {
            Self::CounterClockwise => 1.0,
            Self::Clockwise => -1.0,
        }
    }
}

/// Accepted latent parameters owned by a point/contact constraint.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ContactState {
    PointOnLine {
        parameter: f64,
    },
    PointOnCircle {
        angle: f64,
    },
    PointOnArc {
        span_parameter: f64,
    },
    PointOnBezier {
        parameter: f64,
    },
    PointOnCurve {
        parameter: f64,
    },
    LineCircleTangency {
        line_parameter: f64,
        circle_angle: f64,
    },
    CircleArcTangency {
        arc_span_parameter: f64,
        circle_angle: f64,
    },
    LineBezierTangency {
        parameter: f64,
    },
    LineCurveTangency {
        parameter: f64,
    },
    CurveCurveContact {
        first_parameter: f64,
        second_parameter: f64,
    },
    CurveCurveTangency {
        first_parameter: f64,
        second_parameter: f64,
    },
    CurveCurveFillet {
        first_parameter: f64,
        second_parameter: f64,
    },
}

/// A circle whose center is a sketch point and whose radius is a solver scalar.
#[derive(Clone, Debug, PartialEq)]
pub struct Circle {
    pub(crate) center: PointId,
    pub(crate) radius: f64,
    pub(crate) label: String,
}

impl Circle {
    #[must_use]
    pub const fn center(&self) -> PointId {
        self.center
    }

    #[must_use]
    pub const fn radius(&self) -> f64 {
        self.radius
    }

    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Evaluates the accepted circle at an unwrapped angular parameter.
    ///
    /// # Errors
    ///
    /// Returns an error if the supplied center or angle is non-finite.
    pub fn evaluate(&self, center: Point2<f64>, angle: f64) -> Result<Point2<f64>, SketchError> {
        validate_point(center, "circle center")?;
        validate_finite(angle, "circle angle")?;
        let result = center + Vector2::new(self.radius * angle.cos(), self.radius * angle.sin());
        validate_point(result, "circle evaluation")?;
        Ok(result)
    }
}

/// A circular arc with fixed accepted endpoint angles and explicit sweep state.
///
/// M7 solves the radius as a scalar while retaining `start_angle` and `end_angle`
/// as explicit fixed entity state.
#[derive(Clone, Debug, PartialEq)]
pub struct CircularArc {
    pub(crate) center: PointId,
    pub(crate) radius: f64,
    pub(crate) start_angle: f64,
    pub(crate) end_angle: f64,
    pub(crate) sweep: ArcSweep,
    pub(crate) signed_sweep: f64,
    pub(crate) label: String,
}

impl CircularArc {
    #[must_use]
    pub const fn center(&self) -> PointId {
        self.center
    }

    #[must_use]
    pub const fn radius(&self) -> f64 {
        self.radius
    }

    #[must_use]
    pub const fn start_angle(&self) -> f64 {
        self.start_angle
    }

    #[must_use]
    pub const fn end_angle(&self) -> f64 {
        self.end_angle
    }

    #[must_use]
    pub const fn sweep(&self) -> ArcSweep {
        self.sweep
    }

    #[must_use]
    pub const fn signed_sweep(&self) -> f64 {
        self.signed_sweep
    }

    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Evaluates a point on the accepted bounded span, parameterized over `[0, 1]`.
    ///
    /// # Errors
    ///
    /// Returns an error for a non-finite center or a parameter outside `[0, 1]`.
    pub fn evaluate(
        &self,
        center: Point2<f64>,
        span_parameter: f64,
    ) -> Result<Point2<f64>, SketchError> {
        validate_point(center, "arc center")?;
        validate_bounded_parameter(span_parameter, "bounded-arc span [0, 1]")?;
        let angle = self.start_angle + self.signed_sweep * span_parameter;
        let result = center + Vector2::new(self.radius * angle.cos(), self.radius * angle.sin());
        validate_point(result, "arc evaluation")?;
        Ok(result)
    }

    /// Returns the start and end points in sweep order.
    ///
    /// # Errors
    ///
    /// Returns an error for a non-finite center or evaluation overflow.
    pub fn endpoints(
        &self,
        center: Point2<f64>,
    ) -> Result<(Point2<f64>, Point2<f64>), SketchError> {
        Ok((self.evaluate(center, 0.0)?, self.evaluate(center, 1.0)?))
    }
}

/// Parameter-domain metadata returned by the internal curve adapter.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum CurveParameterDomain {
    SupportingLine,
    BoundedSegment,
    PeriodicCircle,
    #[cfg(test)]
    BoundedProofCurve,
}

/// Explicit derivative validity returned by the internal curve adapter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CurveDegeneracy {
    Regular,
    ZeroDerivative,
    InvalidRadius,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct CurveEvaluation {
    pub(crate) position: [f64; 2],
    pub(crate) first_derivative: [f64; 2],
    pub(crate) second_derivative: Option<[f64; 2]>,
    pub(crate) domain: CurveParameterDomain,
    pub(crate) degeneracy: CurveDegeneracy,
}

/// Private value adapter used by point/contact residuals without freezing a public trait.
#[derive(Clone, Copy, Debug)]
pub(crate) enum CurveRef {
    Line {
        start: [f64; 2],
        end: [f64; 2],
        domain: LineParameterDomain,
    },
    Circle {
        center: [f64; 2],
        radius: f64,
    },
    #[cfg(test)]
    QuadraticBezier {
        control: [[f64; 2]; 3],
    },
    #[cfg(test)]
    CubicBezier {
        control: [[f64; 2]; 4],
    },
}

impl CurveRef {
    #[must_use]
    pub(crate) fn evaluate(self, parameter: f64) -> CurveEvaluation {
        match self {
            Self::Line { start, end, domain } => {
                let derivative = [end[0] - start[0], end[1] - start[1]];
                CurveEvaluation {
                    position: [
                        start[0] + parameter * derivative[0],
                        start[1] + parameter * derivative[1],
                    ],
                    first_derivative: derivative,
                    second_derivative: Some([0.0, 0.0]),
                    domain: match domain {
                        LineParameterDomain::SupportingLine => CurveParameterDomain::SupportingLine,
                        LineParameterDomain::BoundedSegment => CurveParameterDomain::BoundedSegment,
                    },
                    degeneracy: if derivative[0].hypot(derivative[1]) == 0.0 {
                        CurveDegeneracy::ZeroDerivative
                    } else {
                        CurveDegeneracy::Regular
                    },
                }
            }
            Self::Circle { center, radius } => radial_evaluation(
                center,
                radius,
                parameter,
                1.0,
                CurveParameterDomain::PeriodicCircle,
            ),
            #[cfg(test)]
            Self::QuadraticBezier { control } => quadratic_bezier_evaluation(control, parameter),
            #[cfg(test)]
            Self::CubicBezier { control } => cubic_bezier_evaluation(control, parameter),
        }
    }
}

fn radial_evaluation(
    center: [f64; 2],
    radius: f64,
    angle: f64,
    angle_rate: f64,
    domain: CurveParameterDomain,
) -> CurveEvaluation {
    let cosine = angle.cos();
    let sine = angle.sin();
    CurveEvaluation {
        position: [center[0] + radius * cosine, center[1] + radius * sine],
        first_derivative: [-radius * angle_rate * sine, radius * angle_rate * cosine],
        second_derivative: Some([
            -radius * angle_rate * angle_rate * cosine,
            -radius * angle_rate * angle_rate * sine,
        ]),
        domain,
        degeneracy: if !radius.is_finite() || radius <= 0.0 {
            CurveDegeneracy::InvalidRadius
        } else if angle_rate == 0.0 {
            CurveDegeneracy::ZeroDerivative
        } else {
            CurveDegeneracy::Regular
        },
    }
}

#[cfg(test)]
fn quadratic_bezier_evaluation(control: [[f64; 2]; 3], parameter: f64) -> CurveEvaluation {
    let [first, middle, last] = control;
    let one_minus = 1.0 - parameter;
    let position = [
        one_minus * one_minus * first[0]
            + 2.0 * one_minus * parameter * middle[0]
            + parameter * parameter * last[0],
        one_minus * one_minus * first[1]
            + 2.0 * one_minus * parameter * middle[1]
            + parameter * parameter * last[1],
    ];
    let derivative = [
        2.0 * (one_minus * (middle[0] - first[0]) + parameter * (last[0] - middle[0])),
        2.0 * (one_minus * (middle[1] - first[1]) + parameter * (last[1] - middle[1])),
    ];
    CurveEvaluation {
        position,
        first_derivative: derivative,
        second_derivative: Some([
            2.0 * (last[0] - 2.0 * middle[0] + first[0]),
            2.0 * (last[1] - 2.0 * middle[1] + first[1]),
        ]),
        domain: CurveParameterDomain::BoundedProofCurve,
        degeneracy: derivative_degeneracy(derivative),
    }
}

#[cfg(test)]
fn cubic_bezier_evaluation(control: [[f64; 2]; 4], parameter: f64) -> CurveEvaluation {
    let [first, second, third, last] = control;
    let one_minus = 1.0 - parameter;
    let position = [
        one_minus.powi(3) * first[0]
            + 3.0 * one_minus * one_minus * parameter * second[0]
            + 3.0 * one_minus * parameter * parameter * third[0]
            + parameter.powi(3) * last[0],
        one_minus.powi(3) * first[1]
            + 3.0 * one_minus * one_minus * parameter * second[1]
            + 3.0 * one_minus * parameter * parameter * third[1]
            + parameter.powi(3) * last[1],
    ];
    let derivative = [
        3.0 * (one_minus * one_minus * (second[0] - first[0])
            + 2.0 * one_minus * parameter * (third[0] - second[0])
            + parameter * parameter * (last[0] - third[0])),
        3.0 * (one_minus * one_minus * (second[1] - first[1])
            + 2.0 * one_minus * parameter * (third[1] - second[1])
            + parameter * parameter * (last[1] - third[1])),
    ];
    CurveEvaluation {
        position,
        first_derivative: derivative,
        second_derivative: Some([
            6.0 * (one_minus * (third[0] - 2.0 * second[0] + first[0])
                + parameter * (last[0] - 2.0 * third[0] + second[0])),
            6.0 * (one_minus * (third[1] - 2.0 * second[1] + first[1])
                + parameter * (last[1] - 2.0 * third[1] + second[1])),
        ]),
        domain: CurveParameterDomain::BoundedProofCurve,
        degeneracy: derivative_degeneracy(derivative),
    }
}

#[cfg(test)]
fn derivative_degeneracy(derivative: [f64; 2]) -> CurveDegeneracy {
    if derivative[0].is_finite()
        && derivative[1].is_finite()
        && derivative[0].hypot(derivative[1]) > 0.0
    {
        CurveDegeneracy::Regular
    } else {
        CurveDegeneracy::ZeroDerivative
    }
}

impl Sketch {
    /// Adds a circle with a deterministic generated label.
    ///
    /// # Errors
    ///
    /// Returns an error for a stale center or invalid radius.
    pub fn add_circle(&mut self, center: PointId, radius: f64) -> Result<CircleId, SketchError> {
        let label = format!("C{}", self.circles.next_ordinal());
        self.add_named_circle(label, center, radius)
    }

    /// Adds a named circle with a positive finite accepted radius.
    ///
    /// # Errors
    ///
    /// Returns an error for an empty label, stale center, or invalid radius.
    pub fn add_named_circle(
        &mut self,
        label: impl Into<String>,
        center: PointId,
        radius: f64,
    ) -> Result<CircleId, SketchError> {
        self.point_position(center)?;
        validate_radius(radius)?;
        let label = validate_label(label, "circle")?;
        Ok(self.circles.insert(Circle {
            center,
            radius,
            label,
        }))
    }

    #[must_use]
    pub fn circle(&self, circle: CircleId) -> Option<&Circle> {
        self.circles.get(circle)
    }

    pub fn circles(&self) -> impl Iterator<Item = (CircleId, &Circle)> {
        self.circles.iter()
    }

    /// Replaces a circle's accepted radius without changing its stable ID.
    ///
    /// # Errors
    ///
    /// Returns an error for a stale circle or invalid radius.
    pub fn set_circle_radius(&mut self, circle: CircleId, radius: f64) -> Result<(), SketchError> {
        validate_radius(radius)?;
        self.circles
            .get_mut(circle)
            .ok_or(SketchError::UnknownCircle(circle))?
            .radius = radius;
        Ok(())
    }

    /// Evaluates an accepted circle using its accepted center and radius.
    ///
    /// # Errors
    ///
    /// Returns an error for a stale circle or non-finite angle.
    pub fn evaluate_circle(
        &self,
        circle: CircleId,
        angle: f64,
    ) -> Result<Point2<f64>, SketchError> {
        let entity = self
            .circles
            .get(circle)
            .ok_or(SketchError::UnknownCircle(circle))?;
        entity.evaluate(self.point_position(entity.center)?, angle)
    }

    /// Removes an unreferenced circle and leaves its stable ID stale.
    ///
    /// # Errors
    ///
    /// Returns an error for a stale or referenced circle.
    pub fn remove_circle(&mut self, circle: CircleId) -> Result<Circle, SketchError> {
        if self.constraint_references_circle(circle) || self.dimension_references_circle(circle) {
            return Err(SketchError::CircleInUse(circle));
        }
        self.circles
            .remove(circle)
            .ok_or(SketchError::UnknownCircle(circle))
    }

    /// Adds a circular arc with a generated label and explicit sweep state.
    ///
    /// # Errors
    ///
    /// Returns an error for a stale center, invalid radius, or invalid sweep.
    pub fn add_arc(
        &mut self,
        center: PointId,
        radius: f64,
        start_angle: f64,
        end_angle: f64,
        sweep: ArcSweep,
    ) -> Result<ArcId, SketchError> {
        let label = format!("A{}", self.arcs.next_ordinal());
        self.add_named_arc(label, center, radius, start_angle, end_angle, sweep)
    }

    /// Adds a named circular arc. Endpoint angles remain fixed entity state in M7.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid entity data or an empty label.
    #[allow(clippy::too_many_arguments)]
    pub fn add_named_arc(
        &mut self,
        label: impl Into<String>,
        center: PointId,
        radius: f64,
        start_angle: f64,
        end_angle: f64,
        sweep: ArcSweep,
    ) -> Result<ArcId, SketchError> {
        self.point_position(center)?;
        validate_radius(radius)?;
        let signed_sweep = arc_signed_sweep(start_angle, end_angle, sweep)?;
        let label = validate_label(label, "arc")?;
        Ok(self.arcs.insert(CircularArc {
            center,
            radius,
            start_angle,
            end_angle,
            sweep,
            signed_sweep,
            label,
        }))
    }

    #[must_use]
    pub fn arc(&self, arc: ArcId) -> Option<&CircularArc> {
        self.arcs.get(arc)
    }

    pub fn arcs(&self) -> impl Iterator<Item = (ArcId, &CircularArc)> {
        self.arcs.iter()
    }

    /// Replaces an arc's accepted radius without changing its fixed span state.
    ///
    /// # Errors
    ///
    /// Returns an error for a stale arc or invalid radius.
    pub fn set_arc_radius(&mut self, arc: ArcId, radius: f64) -> Result<(), SketchError> {
        validate_radius(radius)?;
        self.arcs
            .get_mut(arc)
            .ok_or(SketchError::UnknownArc(arc))?
            .radius = radius;
        Ok(())
    }

    pub(crate) fn set_arc_span(
        &mut self,
        arc: ArcId,
        start_angle: f64,
        end_angle: f64,
    ) -> Result<(), SketchError> {
        let value = self.arcs.get_mut(arc).ok_or(SketchError::UnknownArc(arc))?;
        value.signed_sweep = arc_signed_sweep(start_angle, end_angle, value.sweep)?;
        value.start_angle = start_angle;
        value.end_angle = end_angle;
        Ok(())
    }

    /// Evaluates an accepted arc over its bounded `[0, 1]` span.
    ///
    /// # Errors
    ///
    /// Returns an error for a stale arc or out-of-domain parameter.
    pub fn evaluate_arc(
        &self,
        arc: ArcId,
        span_parameter: f64,
    ) -> Result<Point2<f64>, SketchError> {
        let entity = self.arcs.get(arc).ok_or(SketchError::UnknownArc(arc))?;
        entity.evaluate(self.point_position(entity.center)?, span_parameter)
    }

    /// Removes an unreferenced arc and leaves its stable ID stale.
    ///
    /// # Errors
    ///
    /// Returns an error for a stale or referenced arc.
    pub fn remove_arc(&mut self, arc: ArcId) -> Result<CircularArc, SketchError> {
        if self.constraint_references_arc(arc) || self.dimension_references_arc(arc) {
            return Err(SketchError::ArcInUse(arc));
        }
        self.arcs.remove(arc).ok_or(SketchError::UnknownArc(arc))
    }

    /// Adds a two-row point-on-line contact with an accepted latent parameter.
    ///
    /// # Errors
    ///
    /// Returns an error for stale/degenerate geometry or an invalid parameter.
    pub fn add_point_on_line(
        &mut self,
        point: PointId,
        segment: SegmentId,
        domain: LineParameterDomain,
        parameter: f64,
    ) -> Result<SketchConstraintId, SketchError> {
        self.point_position(point)?;
        self.validate_segment_geometry(segment)?;
        validate_line_parameter(domain, parameter)?;
        Ok(self.insert_constraint(SketchConstraintKind::PointOnLine {
            point,
            segment,
            domain,
            parameter,
        }))
    }

    /// Adds a two-row periodic point-on-circle contact.
    ///
    /// # Errors
    ///
    /// Returns an error for stale geometry or a non-finite angle.
    pub fn add_point_on_circle(
        &mut self,
        point: PointId,
        circle: CircleId,
        angle: f64,
    ) -> Result<SketchConstraintId, SketchError> {
        self.point_position(point)?;
        self.circle_value(circle)?;
        validate_finite(angle, "circle contact angle")?;
        Ok(self.insert_constraint(SketchConstraintKind::PointOnCircle {
            point,
            circle,
            angle,
        }))
    }

    /// Adds a two-row point contact on an arc's bounded span.
    ///
    /// # Errors
    ///
    /// Returns an error for stale geometry or an out-of-domain parameter.
    pub fn add_point_on_arc(
        &mut self,
        point: PointId,
        arc: ArcId,
        span_parameter: f64,
    ) -> Result<SketchConstraintId, SketchError> {
        self.point_position(point)?;
        self.arc_value(arc)?;
        validate_bounded_parameter(span_parameter, "bounded-arc span [0, 1]")?;
        Ok(self.insert_constraint(SketchConstraintKind::PointOnArc {
            point,
            arc,
            span_parameter,
        }))
    }

    /// Constrains two nondegenerate directed segments to be parallel.
    ///
    /// # Errors
    ///
    /// Returns an error for stale, repeated, or degenerate segments.
    pub fn add_parallel(
        &mut self,
        first: SegmentId,
        second: SegmentId,
    ) -> Result<SketchConstraintId, SketchError> {
        self.validate_segment_pair(first, second)?;
        Ok(self.insert_constraint(SketchConstraintKind::Parallel { first, second }))
    }

    /// Constrains two nondegenerate directed segments to be perpendicular.
    ///
    /// # Errors
    ///
    /// Returns an error for stale, repeated, or degenerate segments.
    pub fn add_perpendicular(
        &mut self,
        first: SegmentId,
        second: SegmentId,
    ) -> Result<SketchConstraintId, SketchError> {
        self.validate_segment_pair(first, second)?;
        Ok(self.insert_constraint(SketchConstraintKind::Perpendicular { first, second }))
    }

    /// Constrains two nondegenerate segments to have equal length.
    ///
    /// # Errors
    ///
    /// Returns an error for stale, repeated, or degenerate segments.
    pub fn add_equal_segment_length(
        &mut self,
        first: SegmentId,
        second: SegmentId,
    ) -> Result<SketchConstraintId, SketchError> {
        self.validate_segment_pair(first, second)?;
        Ok(self.insert_constraint(SketchConstraintKind::EqualSegmentLength { first, second }))
    }

    /// Constrains two distinct circle radius scalars to be equal.
    ///
    /// # Errors
    ///
    /// Returns an error for stale or repeated circles.
    pub fn add_equal_circle_radius(
        &mut self,
        first: CircleId,
        second: CircleId,
    ) -> Result<SketchConstraintId, SketchError> {
        self.validate_circle_pair(first, second)?;
        Ok(self.insert_constraint(SketchConstraintKind::EqualCircleRadius { first, second }))
    }

    /// Constrains a point to the midpoint of a segment.
    ///
    /// # Errors
    ///
    /// Returns an error for stale or degenerate geometry.
    pub fn add_midpoint(
        &mut self,
        point: PointId,
        segment: SegmentId,
    ) -> Result<SketchConstraintId, SketchError> {
        self.point_position(point)?;
        self.validate_segment_geometry(segment)?;
        Ok(self.insert_constraint(SketchConstraintKind::Midpoint { point, segment }))
    }

    /// Constrains two points to be mirror images about a supporting line.
    ///
    /// # Errors
    ///
    /// Returns an error for stale, repeated, or degenerate geometry.
    pub fn add_symmetric_about_line(
        &mut self,
        first: PointId,
        second: PointId,
        line: SegmentId,
    ) -> Result<SketchConstraintId, SketchError> {
        if first == second {
            return Err(SketchError::RepeatedPoint);
        }
        self.point_position(first)?;
        self.point_position(second)?;
        self.validate_segment_geometry(line)?;
        Ok(
            self.insert_constraint(SketchConstraintKind::SymmetricAboutLine {
                first,
                second,
                line,
            }),
        )
    }

    #[allow(clippy::too_many_arguments)]
    /// Adds branch-sensitive line-circle tangency with two accepted contacts.
    ///
    /// # Errors
    ///
    /// Returns an error for stale/degenerate geometry or invalid parameters.
    pub fn add_line_circle_tangency(
        &mut self,
        line: SegmentId,
        circle: CircleId,
        domain: LineParameterDomain,
        side: LineSide,
        line_parameter: f64,
        circle_angle: f64,
    ) -> Result<SketchConstraintId, SketchError> {
        self.validate_segment_geometry(line)?;
        self.circle_value(circle)?;
        validate_line_parameter(domain, line_parameter)?;
        validate_finite(circle_angle, "circle contact angle")?;
        Ok(
            self.insert_constraint(SketchConstraintKind::LineCircleTangency {
                line,
                circle,
                domain,
                side,
                line_parameter,
                circle_angle,
            }),
        )
    }

    /// Adds circle tangency with explicit mode, containment, and center direction.
    ///
    /// # Errors
    ///
    /// Returns an error for stale/repeated circles or invalid internal mode.
    pub fn add_circle_circle_tangency(
        &mut self,
        first: CircleId,
        second: CircleId,
        mode: CircleTangencyMode,
        center_direction: CenterDirectionBranch,
    ) -> Result<SketchConstraintId, SketchError> {
        self.validate_circle_pair(first, second)?;
        self.validate_tangency_mode(first, second, mode)?;
        Ok(
            self.insert_constraint(SketchConstraintKind::CircleCircleTangency {
                first,
                second,
                mode,
                center_direction,
            }),
        )
    }

    /// Adds native circle-to-bounded-arc tangency with explicit radial side and contacts.
    ///
    /// The contact values are accepted warm starts and need not exactly satisfy the
    /// tangency equations. The current centers must already select the requested side.
    ///
    /// # Errors
    ///
    /// Returns an error for stale entities, ambiguous centers, invalid contact state,
    /// or a current center position incompatible with `side`.
    pub fn add_circle_arc_tangency(
        &mut self,
        circle: CircleId,
        arc: ArcId,
        side: ArcCircleTangencySide,
        arc_span_parameter: f64,
        circle_angle: f64,
    ) -> Result<SketchConstraintId, SketchError> {
        let circle_value = self.circle_value(circle)?;
        let arc_value = self.arc_value(arc)?;
        validate_radius(circle_value.radius())?;
        validate_radius(arc_value.radius())?;
        validate_bounded_parameter(arc_span_parameter, "bounded-arc span [0, 1]")?;
        validate_finite(circle_angle, "circle contact angle")?;

        if circle_value.center() == arc_value.center() {
            return Err(SketchError::AmbiguousArcCircleTangencyCenters);
        }
        let circle_center = self.point_position(circle_value.center())?;
        let arc_center = self.point_position(arc_value.center())?;
        let center_distance = (circle_center - arc_center).norm();
        if !center_distance.is_finite() || center_distance == 0.0 {
            return Err(SketchError::AmbiguousArcCircleTangencyCenters);
        }
        let derived_radius = (center_distance - arc_value.radius()).abs();
        if !derived_radius.is_finite() || derived_radius <= 0.0 {
            return Err(SketchError::ZeroDerivedCircleRadius);
        }
        if !side.accepts(center_distance, arc_value.radius()) {
            return Err(SketchError::ArcCircleTangencySideMismatch(side));
        }

        Ok(
            self.insert_constraint(SketchConstraintKind::CircleArcTangency {
                circle,
                arc,
                side,
                arc_span_parameter,
                circle_angle,
            }),
        )
    }

    /// Returns accepted latent state for a point/contact constraint.
    ///
    /// # Errors
    ///
    /// Returns an error for a stale source or a source without contact state.
    pub fn contact_state(
        &self,
        constraint: SketchConstraintId,
    ) -> Result<ContactState, SketchError> {
        let constraint_value = self
            .constraints
            .get(constraint)
            .ok_or(SketchError::UnknownConstraint(constraint))?;
        match constraint_value.kind() {
            SketchConstraintKind::PointOnLine { parameter, .. } => {
                Ok(ContactState::PointOnLine { parameter })
            }
            SketchConstraintKind::PointOnCircle { angle, .. } => {
                Ok(ContactState::PointOnCircle { angle })
            }
            SketchConstraintKind::PointOnArc { span_parameter, .. } => {
                Ok(ContactState::PointOnArc { span_parameter })
            }
            SketchConstraintKind::PointOnBezier { parameter, .. } => {
                Ok(ContactState::PointOnBezier { parameter })
            }
            SketchConstraintKind::PointOnCurve { contact, .. } => Ok(ContactState::PointOnCurve {
                parameter: contact.parameter,
            }),
            SketchConstraintKind::LineCircleTangency {
                line_parameter,
                circle_angle,
                ..
            } => Ok(ContactState::LineCircleTangency {
                line_parameter,
                circle_angle,
            }),
            SketchConstraintKind::CircleArcTangency {
                arc_span_parameter,
                circle_angle,
                ..
            } => Ok(ContactState::CircleArcTangency {
                arc_span_parameter,
                circle_angle,
            }),
            SketchConstraintKind::LineBezierTangency {
                bezier_parameter, ..
            } => Ok(ContactState::LineBezierTangency {
                parameter: bezier_parameter,
            }),
            SketchConstraintKind::LineCurveTangency { contact, .. } => {
                Ok(ContactState::LineCurveTangency {
                    parameter: contact.parameter,
                })
            }
            SketchConstraintKind::CurveCurveContact { first, second } => {
                Ok(ContactState::CurveCurveContact {
                    first_parameter: first.parameter,
                    second_parameter: second.parameter,
                })
            }
            SketchConstraintKind::CurveCurveTangency { first, second, .. } => {
                Ok(ContactState::CurveCurveTangency {
                    first_parameter: first.parameter,
                    second_parameter: second.parameter,
                })
            }
            SketchConstraintKind::CurveCurveFillet { first, second, .. } => {
                Ok(ContactState::CurveCurveFillet {
                    first_parameter: first.parameter,
                    second_parameter: second.parameter,
                })
            }
            _ => Err(SketchError::NoContactState(constraint)),
        }
    }

    /// Explicitly replaces a valid accepted contact warm start without changing source identity.
    ///
    /// # Errors
    ///
    /// Returns an error for a stale/mismatched source or invalid parameter.
    #[allow(clippy::too_many_lines)]
    pub fn set_contact_state(
        &mut self,
        constraint: SketchConstraintId,
        state: ContactState,
    ) -> Result<(), SketchError> {
        let current = self
            .constraints
            .get(constraint)
            .ok_or(SketchError::UnknownConstraint(constraint))?
            .kind();
        match (current, state) {
            (
                SketchConstraintKind::PointOnCurve { mut contact, .. },
                ContactState::PointOnCurve { parameter },
            )
            | (
                SketchConstraintKind::LineCurveTangency { mut contact, .. },
                ContactState::LineCurveTangency { parameter },
            ) => {
                contact.parameter = parameter;
                self.validate_curve_contact(contact)?;
            }
            (
                SketchConstraintKind::CurveCurveContact {
                    mut first,
                    mut second,
                },
                ContactState::CurveCurveContact {
                    first_parameter,
                    second_parameter,
                },
            )
            | (
                SketchConstraintKind::CurveCurveTangency {
                    mut first,
                    mut second,
                    ..
                },
                ContactState::CurveCurveTangency {
                    first_parameter,
                    second_parameter,
                },
            )
            | (
                SketchConstraintKind::CurveCurveFillet {
                    mut first,
                    mut second,
                    ..
                },
                ContactState::CurveCurveFillet {
                    first_parameter,
                    second_parameter,
                },
            ) => {
                first.parameter = first_parameter;
                second.parameter = second_parameter;
                self.validate_curve_contact(first)?;
                self.validate_curve_contact(second)?;
            }
            _ => {}
        }
        let kind = &mut self
            .constraints
            .get_mut(constraint)
            .ok_or(SketchError::UnknownConstraint(constraint))?
            .kind;
        match (kind, state) {
            (
                SketchConstraintKind::PointOnLine {
                    domain, parameter, ..
                },
                ContactState::PointOnLine { parameter: value },
            ) => {
                validate_line_parameter(*domain, value)?;
                *parameter = value;
            }
            (
                SketchConstraintKind::PointOnCircle { angle, .. },
                ContactState::PointOnCircle { angle: value },
            ) => {
                validate_finite(value, "circle contact angle")?;
                *angle = value;
            }
            (
                SketchConstraintKind::PointOnArc { span_parameter, .. },
                ContactState::PointOnArc {
                    span_parameter: value,
                },
            ) => {
                validate_bounded_parameter(value, "bounded-arc span [0, 1]")?;
                *span_parameter = value;
            }
            (
                SketchConstraintKind::PointOnBezier { parameter, .. },
                ContactState::PointOnBezier { parameter: value },
            ) => {
                validate_bounded_parameter(value, "bounded Bezier span [0, 1]")?;
                *parameter = value;
            }
            (
                SketchConstraintKind::LineCircleTangency {
                    domain,
                    line_parameter,
                    circle_angle,
                    ..
                },
                ContactState::LineCircleTangency {
                    line_parameter: line_value,
                    circle_angle: circle_value,
                },
            ) => {
                validate_line_parameter(*domain, line_value)?;
                validate_finite(circle_value, "circle contact angle")?;
                *line_parameter = line_value;
                *circle_angle = circle_value;
            }
            (
                SketchConstraintKind::CircleArcTangency {
                    arc_span_parameter,
                    circle_angle,
                    ..
                },
                ContactState::CircleArcTangency {
                    arc_span_parameter: arc_value,
                    circle_angle: circle_value,
                },
            ) => {
                validate_bounded_parameter(arc_value, "bounded-arc span [0, 1]")?;
                validate_finite(circle_value, "circle contact angle")?;
                *arc_span_parameter = arc_value;
                *circle_angle = circle_value;
            }
            (
                SketchConstraintKind::LineBezierTangency {
                    bezier_parameter, ..
                },
                ContactState::LineBezierTangency { parameter: value },
            ) => {
                validate_bounded_parameter(value, "bounded Bezier span [0, 1]")?;
                *bezier_parameter = value;
            }
            (
                SketchConstraintKind::PointOnCurve { contact, .. },
                ContactState::PointOnCurve { parameter },
            )
            | (
                SketchConstraintKind::LineCurveTangency { contact, .. },
                ContactState::LineCurveTangency { parameter },
            ) => {
                let mut updated = *contact;
                updated.parameter = parameter;
                *contact = updated;
            }
            (
                SketchConstraintKind::CurveCurveContact { first, second },
                ContactState::CurveCurveContact {
                    first_parameter,
                    second_parameter,
                },
            )
            | (
                SketchConstraintKind::CurveCurveTangency { first, second, .. },
                ContactState::CurveCurveTangency {
                    first_parameter,
                    second_parameter,
                },
            )
            | (
                SketchConstraintKind::CurveCurveFillet { first, second, .. },
                ContactState::CurveCurveFillet {
                    first_parameter,
                    second_parameter,
                },
            ) => {
                let mut updated_first = *first;
                updated_first.parameter = first_parameter;
                let mut updated_second = *second;
                updated_second.parameter = second_parameter;
                *first = updated_first;
                *second = updated_second;
            }
            _ => return Err(SketchError::NoContactState(constraint)),
        }
        Ok(())
    }

    /// Returns the explicit radial side of a circle-arc tangency source.
    ///
    /// # Errors
    ///
    /// Returns an error for a stale source or a different constraint kind.
    pub fn circle_arc_tangency_side(
        &self,
        constraint: SketchConstraintId,
    ) -> Result<ArcCircleTangencySide, SketchError> {
        match self
            .constraints
            .get(constraint)
            .ok_or(SketchError::UnknownConstraint(constraint))?
            .kind()
        {
            SketchConstraintKind::CircleArcTangency { side, .. } => Ok(side),
            _ => Err(SketchError::NotCircleArcTangency(constraint)),
        }
    }

    /// Changes circle-circle tangency mode as explicit source state.
    ///
    /// # Errors
    ///
    /// Returns an error for a stale/wrong source or invalid containment.
    pub fn set_circle_tangency_mode(
        &mut self,
        constraint: SketchConstraintId,
        mode: CircleTangencyMode,
    ) -> Result<(), SketchError> {
        let SketchConstraintKind::CircleCircleTangency { first, second, .. } = self
            .constraints
            .get(constraint)
            .ok_or(SketchError::UnknownConstraint(constraint))?
            .kind()
        else {
            return Err(SketchError::NotCircleTangency(constraint));
        };
        self.validate_tangency_mode(first, second, mode)?;
        let SketchConstraintKind::CircleCircleTangency {
            mode: current_mode, ..
        } = &mut self
            .constraints
            .get_mut(constraint)
            .ok_or(SketchError::UnknownConstraint(constraint))?
            .kind
        else {
            return Err(SketchError::NotCircleTangency(constraint));
        };
        *current_mode = mode;
        Ok(())
    }

    /// Returns the explicit mode of a circle-circle tangency source.
    ///
    /// # Errors
    ///
    /// Returns an error for a stale source or a non-tangency source.
    pub fn circle_tangency_mode(
        &self,
        constraint: SketchConstraintId,
    ) -> Result<CircleTangencyMode, SketchError> {
        match self
            .constraints
            .get(constraint)
            .ok_or(SketchError::UnknownConstraint(constraint))?
            .kind()
        {
            SketchConstraintKind::CircleCircleTangency { mode, .. } => Ok(mode),
            _ => Err(SketchError::NotCircleTangency(constraint)),
        }
    }

    /// Adds a driving or reference circle-radius dimension.
    ///
    /// # Errors
    ///
    /// Returns an error for a stale circle or invalid target.
    pub fn add_circle_radius(
        &mut self,
        circle: CircleId,
        target: f64,
        mode: DimensionMode,
    ) -> Result<SketchDimensionId, SketchError> {
        self.circle_value(circle)?;
        validate_dimension_value(target)?;
        Ok(self.insert_dimension(DimensionKind::CircleRadius { circle, target }, mode))
    }

    /// Adds a driving or reference circle-diameter dimension.
    ///
    /// # Errors
    ///
    /// Returns an error for a stale circle or invalid target.
    pub fn add_circle_diameter(
        &mut self,
        circle: CircleId,
        target: f64,
        mode: DimensionMode,
    ) -> Result<SketchDimensionId, SketchError> {
        self.circle_value(circle)?;
        validate_dimension_value(target)?;
        Ok(self.insert_dimension(DimensionKind::CircleDiameter { circle, target }, mode))
    }

    /// Adds a driving or reference arc-radius dimension.
    ///
    /// # Errors
    ///
    /// Returns an error for a stale arc or invalid target.
    pub fn add_arc_radius(
        &mut self,
        arc: ArcId,
        target: f64,
        mode: DimensionMode,
    ) -> Result<SketchDimensionId, SketchError> {
        self.arc_value(arc)?;
        validate_dimension_value(target)?;
        Ok(self.insert_dimension(DimensionKind::ArcRadius { arc, target }, mode))
    }

    /// Adds a driving or reference arc-diameter dimension.
    ///
    /// # Errors
    ///
    /// Returns an error for a stale arc or invalid target.
    pub fn add_arc_diameter(
        &mut self,
        arc: ArcId,
        target: f64,
        mode: DimensionMode,
    ) -> Result<SketchDimensionId, SketchError> {
        self.arc_value(arc)?;
        validate_dimension_value(target)?;
        Ok(self.insert_dimension(DimensionKind::ArcDiameter { arc, target }, mode))
    }

    /// Adds a branch-local oriented angle between two directed segments.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid geometry or a nonpositive/non-finite target.
    pub fn add_oriented_angle(
        &mut self,
        first: SegmentId,
        second: SegmentId,
        target: f64,
        orientation: AngleOrientation,
        mode: DimensionMode,
    ) -> Result<SketchDimensionId, SketchError> {
        self.validate_segment_pair(first, second)?;
        validate_angle(target)?;
        Ok(self.insert_dimension(
            DimensionKind::OrientedAngle {
                first,
                second,
                target,
                orientation,
            },
            mode,
        ))
    }

    /// Adds a signed offset between two supporting lines.
    ///
    /// The target segment remains free to slide along the source line direction and to change
    /// length. `orientation` selects which target endpoint corresponds to the source start.
    ///
    /// # Errors
    ///
    /// Returns an error for stale/repeated or degenerate segments, or an invalid target.
    pub fn add_supporting_line_offset(
        &mut self,
        source: SegmentId,
        target_segment: SegmentId,
        target: f64,
        side: LineSide,
        orientation: LineOffsetOrientation,
        mode: DimensionMode,
    ) -> Result<SketchDimensionId, SketchError> {
        self.validate_segment_pair(source, target_segment)?;
        validate_dimension_value(target)?;
        Ok(self.insert_dimension(
            DimensionKind::SupportingLineOffset {
                source,
                target_segment,
                target,
                side,
                orientation,
            },
            mode,
        ))
    }

    /// Adds an exact signed translation between two segments.
    ///
    /// Both target endpoints are the corresponding source endpoints translated by the selected
    /// normal offset, so this mode preserves segment length and endpoint correspondence.
    ///
    /// # Errors
    ///
    /// Returns an error for stale/repeated or degenerate segments, or an invalid target.
    pub fn add_exact_translated_segment_offset(
        &mut self,
        source: SegmentId,
        target_segment: SegmentId,
        target: f64,
        side: LineSide,
        orientation: LineOffsetOrientation,
        mode: DimensionMode,
    ) -> Result<SketchDimensionId, SketchError> {
        self.validate_segment_pair(source, target_segment)?;
        validate_dimension_value(target)?;
        Ok(self.insert_dimension(
            DimensionKind::ExactTranslatedSegmentOffset {
                source,
                target_segment,
                target,
                side,
                orientation,
            },
            mode,
        ))
    }

    pub(crate) fn oriented_angle_value(
        &self,
        first: SegmentId,
        second: SegmentId,
        orientation: AngleOrientation,
        branch_reference: f64,
    ) -> Result<f64, SketchError> {
        let first_direction = self.segment_direction(first)?;
        let second_direction = self.segment_direction(second)?;
        let cross = first_direction.x * second_direction.y - first_direction.y * second_direction.x;
        let dot = first_direction.dot(&second_direction);
        let principal = orientation.sign() * cross.atan2(dot);
        Ok(unwrap_near(principal, branch_reference))
    }

    pub(crate) fn circle_value(&self, circle: CircleId) -> Result<&Circle, SketchError> {
        self.circles
            .get(circle)
            .ok_or(SketchError::UnknownCircle(circle))
    }

    pub(crate) fn arc_value(&self, arc: ArcId) -> Result<&CircularArc, SketchError> {
        self.arcs.get(arc).ok_or(SketchError::UnknownArc(arc))
    }

    pub(crate) fn validate_tangency_mode(
        &self,
        first: CircleId,
        second: CircleId,
        mode: CircleTangencyMode,
    ) -> Result<(), SketchError> {
        let first_radius = self.circle_value(first)?.radius;
        let second_radius = self.circle_value(second)?.radius;
        let effective = tangency_distance(first_radius, second_radius, mode);
        if effective.is_finite() && effective > 0.0 {
            Ok(())
        } else {
            Err(SketchError::InvalidInternalTangency)
        }
    }

    fn segment_direction(&self, segment: SegmentId) -> Result<Vector2<f64>, SketchError> {
        let (start, end) = self.segment_endpoints(segment)?;
        let direction = self.point_position(end)? - self.point_position(start)?;
        if direction.norm() == 0.0 {
            Err(SketchError::DegenerateSegment)
        } else {
            Ok(direction)
        }
    }

    fn validate_segment_pair(
        &self,
        first: SegmentId,
        second: SegmentId,
    ) -> Result<(), SketchError> {
        if first == second {
            return Err(SketchError::RepeatedEntity);
        }
        self.validate_segment_geometry(first)?;
        self.validate_segment_geometry(second)
    }

    fn validate_circle_pair(&self, first: CircleId, second: CircleId) -> Result<(), SketchError> {
        if first == second {
            return Err(SketchError::RepeatedEntity);
        }
        self.circle_value(first)?;
        self.circle_value(second)?;
        Ok(())
    }

    fn constraint_references_circle(&self, circle: CircleId) -> bool {
        self.constraints.iter().any(|(_, constraint)| {
            matches!(
                constraint.kind(),
                SketchConstraintKind::PointOnCircle { circle: id, .. }
                    | SketchConstraintKind::LineCircleTangency { circle: id, .. }
                    | SketchConstraintKind::CircleArcTangency { circle: id, .. }
                    if id == circle
            ) || matches!(
                constraint.kind(),
                SketchConstraintKind::EqualCircleRadius { first, second }
                    | SketchConstraintKind::CircleCircleTangency { first, second, .. }
                    if first == circle || second == circle
            ) || generic_constraint_curves(constraint.kind())
                .iter()
                .any(|curve| matches!(curve, crate::SketchCurve::Circle(id) if *id == circle))
        })
    }

    fn dimension_references_circle(&self, circle: CircleId) -> bool {
        self.dimensions.iter().any(|(_, dimension)| {
            matches!(
                dimension.kind(),
                DimensionKind::CircleRadius { circle: id, .. }
                    | DimensionKind::CircleDiameter { circle: id, .. }
                    if id == circle
            ) || matches!(dimension.kind(), DimensionKind::ProfileOffset { profile, .. }
            if self.profile_offsets.get(profile).is_some_and(|association| {
                association.references_curve(crate::ProfileOffsetCurve::Circle(circle))
            }))
        })
    }

    fn constraint_references_arc(&self, arc: ArcId) -> bool {
        self.constraints.iter().any(|(_, constraint)| {
            matches!(
                constraint.kind(),
                SketchConstraintKind::PointOnArc { arc: id, .. }
                    | SketchConstraintKind::CircleArcTangency { arc: id, .. }
                    | SketchConstraintKind::CurveCurveFillet { arc: id, .. }
                    if id == arc
            ) || generic_constraint_curves(constraint.kind())
                .iter()
                .any(|curve| matches!(curve, crate::SketchCurve::Arc(id) if *id == arc))
        })
    }

    fn dimension_references_arc(&self, arc: ArcId) -> bool {
        self.dimensions.iter().any(|(_, dimension)| {
            matches!(
                dimension.kind(),
                DimensionKind::ArcRadius { arc: id, .. }
                    | DimensionKind::ArcDiameter { arc: id, .. }
                    if id == arc
            ) || matches!(dimension.kind(), DimensionKind::ProfileOffset { profile, .. }
            if self.profile_offsets.get(profile).is_some_and(|association| {
                association.references_curve(crate::ProfileOffsetCurve::CircularArc(arc))
            }))
        })
    }
}

fn generic_constraint_curves(kind: SketchConstraintKind) -> Vec<crate::SketchCurve> {
    match kind {
        SketchConstraintKind::PointOnCurve { contact, .. }
        | SketchConstraintKind::LineCurveTangency { contact, .. }
        | SketchConstraintKind::CurveDirection { contact, .. } => vec![contact.curve],
        SketchConstraintKind::CurveCurveContact { first, second }
        | SketchConstraintKind::CurveCurveTangency { first, second, .. }
        | SketchConstraintKind::EqualCurvature { first, second, .. }
        | SketchConstraintKind::EndpointContinuity { first, second, .. }
        | SketchConstraintKind::CurveCurveFillet { first, second, .. } => {
            vec![first.curve, second.curve]
        }
        _ => Vec::new(),
    }
}

pub(crate) fn tangency_distance(
    first_radius: f64,
    second_radius: f64,
    mode: CircleTangencyMode,
) -> f64 {
    match mode {
        CircleTangencyMode::External => first_radius + second_radius,
        CircleTangencyMode::Internal {
            containment: CircleContainment::FirstContainsSecond,
        } => first_radius - second_radius,
        CircleTangencyMode::Internal {
            containment: CircleContainment::SecondContainsFirst,
        } => second_radius - first_radius,
    }
}

pub(crate) fn unwrap_near(principal: f64, reference: f64) -> f64 {
    principal + ((reference - principal) / TAU).round() * TAU
}

pub(crate) fn validate_radius(radius: f64) -> Result<(), SketchError> {
    if radius.is_finite() && radius > 0.0 {
        Ok(())
    } else {
        Err(SketchError::InvalidRadius(radius))
    }
}

pub(crate) fn validate_line_parameter(
    domain: LineParameterDomain,
    parameter: f64,
) -> Result<(), SketchError> {
    if domain.contains(parameter) {
        Ok(())
    } else {
        Err(SketchError::ParameterOutOfDomain {
            parameter,
            domain: domain.label(),
        })
    }
}

pub(crate) fn validate_bounded_parameter(
    parameter: f64,
    domain: &'static str,
) -> Result<(), SketchError> {
    if parameter.is_finite() && (0.0..=1.0).contains(&parameter) {
        Ok(())
    } else {
        Err(SketchError::ParameterOutOfDomain { parameter, domain })
    }
}

pub(crate) fn normalize_bounded_candidate(parameter: f64) -> Option<f64> {
    if !parameter.is_finite() {
        return None;
    }
    if (-CONTACT_PARAMETER_ROUNDOFF_TOLERANCE..=0.0).contains(&parameter) {
        Some(0.0)
    } else if (1.0..=1.0 + CONTACT_PARAMETER_ROUNDOFF_TOLERANCE).contains(&parameter) {
        Some(1.0)
    } else if (0.0..=1.0).contains(&parameter) {
        Some(parameter)
    } else {
        None
    }
}

pub(crate) fn validate_angle(target: f64) -> Result<(), SketchError> {
    if target.is_finite() && target > 0.0 {
        Ok(())
    } else {
        Err(SketchError::InvalidAngle(target))
    }
}

pub(crate) fn arc_signed_sweep(
    start_angle: f64,
    end_angle: f64,
    sweep: ArcSweep,
) -> Result<f64, SketchError> {
    if !start_angle.is_finite() || !end_angle.is_finite() {
        return Err(SketchError::InvalidArcSweep);
    }
    let magnitude = match sweep {
        ArcSweep::CounterClockwise => (end_angle - start_angle).rem_euclid(TAU),
        ArcSweep::Clockwise => (start_angle - end_angle).rem_euclid(TAU),
    };
    if !magnitude.is_finite() || magnitude == 0.0 {
        return Err(SketchError::InvalidArcSweep);
    }
    Ok(match sweep {
        ArcSweep::CounterClockwise => magnitude,
        ArcSweep::Clockwise => -magnitude,
    })
}

fn validate_label(label: impl Into<String>, kind: &'static str) -> Result<String, SketchError> {
    let label = label.into();
    if label.trim().is_empty() {
        Err(SketchError::EmptyLabel(kind))
    } else {
        Ok(label)
    }
}

pub(crate) fn segment_points(
    sketch: &Sketch,
    segment: SegmentId,
) -> Result<(PointId, PointId, &LineSegment), SketchError> {
    let value = sketch
        .segments
        .get(segment)
        .ok_or(SketchError::UnknownSegment(segment))?;
    Ok((value.start(), value.end(), value))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arc_sweep_is_explicit_and_bounded() {
        let ccw = arc_signed_sweep(1.5, -1.5, ArcSweep::CounterClockwise).unwrap();
        let cw = arc_signed_sweep(1.5, -1.5, ArcSweep::Clockwise).unwrap();
        assert!(ccw > 0.0);
        assert!(cw < 0.0);
        assert!((ccw + cw.abs() - TAU).abs() <= f64::EPSILON);
        assert!(arc_signed_sweep(0.0, TAU, ArcSweep::CounterClockwise).is_err());
    }
}
