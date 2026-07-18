use std::f64::consts::TAU;

use thiserror::Error;

use crate::{
    CurveEvaluationError, CurveJet2, CurveParameterDomain, CurveParameterError,
    CurveRegularityError, Point2, Vector2,
};

const RATIONAL_DENOMINATOR_FACTOR: f64 = 64.0 * f64::EPSILON;

/// Definition failures for immutable analytic and rational conics.
#[derive(Clone, Copy, Debug, Error, PartialEq)]
#[non_exhaustive]
pub enum ConicDefinitionError {
    #[error("conic point coordinates must be finite")]
    NonFinitePoint,
    #[error("conic vector components must be finite")]
    NonFiniteVector,
    #[error("a directed axis must be nonzero")]
    ZeroDirection,
    #[error("conic parameter must be finite, got {parameter}")]
    NonFiniteParameter { parameter: f64 },
    #[error("directed trim endpoints must be distinct, both were {parameter}")]
    EqualTrimParameters { parameter: f64 },
    #[error("the signed trim rate from {start} to {end} is not finite")]
    NonFiniteTrimRate { start: f64, end: f64 },
    #[error(
        "ellipse semiaxes must be finite and satisfy semi_major >= semi_minor > 0, got ({semi_major}, {semi_minor})"
    )]
    InvalidEllipseSemiaxes { semi_major: f64, semi_minor: f64 },
    #[error("elliptical arc sweep must be finite and nonzero, got {signed_sweep}")]
    InvalidSignedSweep { signed_sweep: f64 },
    #[error("focal length must be positive and finite, got {focal_length}")]
    InvalidFocalLength { focal_length: f64 },
    #[error("hyperbola semiaxis must be positive and finite, got {length}")]
    InvalidHyperbolaSemiaxis { length: f64 },
    #[error("rational homogeneous controls must be finite and noncollinear")]
    DegenerateHomogeneousControls,
    #[error("middle weight {middle_weight} gives a rational denominator pole on [0, 1]")]
    RationalDenominatorPole { middle_weight: f64 },
    #[error("an ordinary middle control cannot represent a zero homogeneous weight")]
    ZeroWeightOrdinaryControl,
    #[error("derived conic geometry is not representable: {feature}")]
    NonRepresentableDerivedGeometry { feature: &'static str },
}

/// Typed immutable conic-evaluation failure.
#[derive(Clone, Copy, Debug, Error, PartialEq)]
#[non_exhaustive]
pub enum ConicEvaluationError {
    #[error(transparent)]
    Curve(#[from] CurveEvaluationError),
    #[error(
        "rational denominator {denominator} is singular or ill-conditioned at parameter {parameter} (condition scale {condition_scale})"
    )]
    RationalDenominator {
        parameter: f64,
        denominator: f64,
        condition_scale: f64,
    },
}

/// A finite normalized direction in the plane.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UnitDirection2 {
    vector: Vector2<f64>,
}

impl UnitDirection2 {
    /// Normalizes a finite nonzero vector.
    ///
    /// # Errors
    ///
    /// Returns a typed error when either component is non-finite or both are zero.
    pub fn try_new(vector: Vector2<f64>) -> Result<Self, ConicDefinitionError> {
        validate_vector(vector)?;
        let scale = vector.x.abs().max(vector.y.abs());
        if scale == 0.0 {
            return Err(ConicDefinitionError::ZeroDirection);
        }
        let scaled = vector / scale;
        let norm = scaled.x.hypot(scaled.y);
        let normalized = scaled / norm;
        validate_vector(normalized)?;
        Ok(Self { vector: normalized })
    }

    /// Returns the normalized vector.
    #[must_use]
    pub fn vector(self) -> Vector2<f64> {
        self.vector
    }

    /// Returns the normalized direction obtained by a positive quarter turn.
    #[must_use]
    pub fn left_normal(self) -> Self {
        Self {
            vector: Vector2::new(-self.vector.y, self.vector.x),
        }
    }
}

impl TryFrom<Vector2<f64>> for UnitDirection2 {
    type Error = ConicDefinitionError;

    fn try_from(vector: Vector2<f64>) -> Result<Self, Self::Error> {
        Self::try_new(vector)
    }
}

impl From<UnitDirection2> for Vector2<f64> {
    fn from(direction: UnitDirection2) -> Self {
        direction.vector()
    }
}

/// A finite, nonzero, directed native-parameter interval.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DirectedParameterTrim {
    start: f64,
    end: f64,
    signed_rate: f64,
}

impl DirectedParameterTrim {
    /// Constructs a directed trim without sorting its endpoints.
    ///
    /// # Errors
    ///
    /// Returns a typed error for non-finite, equal, or unrepresentably distant endpoints.
    #[allow(clippy::float_cmp)] // Exact equality is the directed-trim degeneracy condition.
    pub fn try_new(start: f64, end: f64) -> Result<Self, ConicDefinitionError> {
        validate_definition_parameter(start)?;
        validate_definition_parameter(end)?;
        if start == end {
            return Err(ConicDefinitionError::EqualTrimParameters { parameter: start });
        }
        let signed_rate = end - start;
        if !signed_rate.is_finite() {
            return Err(ConicDefinitionError::NonFiniteTrimRate { start, end });
        }
        Ok(Self {
            start,
            end,
            signed_rate,
        })
    }

    #[must_use]
    pub fn start(self) -> f64 {
        self.start
    }

    #[must_use]
    pub fn end(self) -> f64 {
        self.end
    }

    /// Returns `end - start`, preserving trim direction.
    #[must_use]
    pub fn signed_rate(self) -> f64 {
        self.signed_rate
    }

    /// Maps a normalized coordinate to the directed native interval.
    ///
    /// Evaluation functions validate that `normalized_parameter` belongs to `[0, 1]`.
    #[must_use]
    #[allow(clippy::float_cmp)] // Preserve both stored endpoints exactly.
    pub fn parameter_at(self, normalized_parameter: f64) -> f64 {
        if normalized_parameter == 0.0 {
            self.start
        } else if normalized_parameter == 1.0 {
            self.end
        } else {
            self.signed_rate.mul_add(normalized_parameter, self.start)
        }
    }
}

/// Whether unequal ellipse axes make the stored axis orientation observable.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum EllipseAxisObservability {
    Observable { relative_axis_separation: f64 },
    ObservableByDirectedTrim,
    UnobservableCircleLimit,
}

/// A validated ellipse with an explicit directed major axis.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ellipse2 {
    center: Point2<f64>,
    major_axis: UnitDirection2,
    semi_major: f64,
    semi_minor: f64,
}

impl Ellipse2 {
    /// Constructs an ellipse, including the exact circle limit.
    ///
    /// # Errors
    ///
    /// Returns a typed error for a non-finite center or invalid semiaxis lengths.
    pub fn try_new(
        center: Point2<f64>,
        major_axis: UnitDirection2,
        semi_major: f64,
        semi_minor: f64,
    ) -> Result<Self, ConicDefinitionError> {
        validate_point(center)?;
        if !semi_major.is_finite()
            || !semi_minor.is_finite()
            || semi_minor <= 0.0
            || semi_major < semi_minor
        {
            return Err(ConicDefinitionError::InvalidEllipseSemiaxes {
                semi_major,
                semi_minor,
            });
        }
        let ellipse = Self {
            center,
            major_axis,
            semi_major,
            semi_minor,
        };
        validate_ellipse_features(ellipse)?;
        Ok(ellipse)
    }

    #[must_use]
    pub fn center(self) -> Point2<f64> {
        self.center
    }

    #[must_use]
    pub fn semi_major(self) -> f64 {
        self.semi_major
    }

    #[must_use]
    pub fn semi_minor(self) -> f64 {
        self.semi_minor
    }

    #[must_use]
    pub fn directed_major_axis(self) -> UnitDirection2 {
        self.major_axis
    }

    #[must_use]
    pub fn directed_minor_axis(self) -> UnitDirection2 {
        self.major_axis.left_normal()
    }

    #[must_use]
    pub fn major_axis(self) -> UnitDirection2 {
        self.directed_major_axis()
    }

    #[must_use]
    pub fn minor_axis(self) -> UnitDirection2 {
        self.directed_minor_axis()
    }

    /// Returns `sqrt((a - b) * (a + b))`, with an overflow-safe scaled fallback.
    #[must_use]
    pub fn linear_eccentricity(self) -> f64 {
        let difference = self.semi_major - self.semi_minor;
        let product = difference * (self.semi_major + self.semi_minor);
        if product.is_finite() {
            product.sqrt()
        } else {
            self.semi_major
                * ((difference / self.semi_major) * (1.0 + self.semi_minor / self.semi_major))
                    .sqrt()
        }
    }

    #[must_use]
    #[allow(clippy::float_cmp)] // Only exact axis equality is the circle limit.
    pub fn axis_observability(self) -> EllipseAxisObservability {
        if self.semi_major == self.semi_minor {
            EllipseAxisObservability::UnobservableCircleLimit
        } else {
            EllipseAxisObservability::Observable {
                relative_axis_separation: (self.semi_major - self.semi_minor) / self.semi_major,
            }
        }
    }

    /// Returns the negative-axis and positive-axis foci, in that order.
    #[must_use]
    pub fn foci(self) -> [Point2<f64>; 2] {
        let offset = self.major_axis.vector() * self.linear_eccentricity();
        [self.center - offset, self.center + offset]
    }

    /// Returns the negative-axis and positive-axis major endpoints.
    #[must_use]
    pub fn major_axis_endpoints(self) -> [Point2<f64>; 2] {
        let offset = self.major_axis.vector() * self.semi_major;
        [self.center - offset, self.center + offset]
    }

    /// Returns the negative-axis and positive-axis minor endpoints.
    #[must_use]
    pub fn minor_axis_endpoints(self) -> [Point2<f64>; 2] {
        let offset = self.directed_minor_axis().vector() * self.semi_minor;
        [self.center - offset, self.center + offset]
    }

    #[must_use]
    pub fn major_axis_length(self) -> f64 {
        2.0 * self.semi_major
    }

    #[must_use]
    pub fn minor_axis_length(self) -> f64 {
        2.0 * self.semi_minor
    }
}

/// A finite nonzero directed angular trim over an ellipse.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EllipticalArc2 {
    ellipse: Ellipse2,
    start_angle: f64,
    signed_sweep: f64,
}

impl EllipticalArc2 {
    /// Constructs a directed elliptical arc.
    ///
    /// # Errors
    ///
    /// Returns a typed error for a non-finite start angle or finite zero/non-finite sweep.
    pub fn try_new(
        ellipse: Ellipse2,
        start_angle: f64,
        signed_sweep: f64,
    ) -> Result<Self, ConicDefinitionError> {
        validate_definition_parameter(start_angle)?;
        if !signed_sweep.is_finite() || signed_sweep == 0.0 {
            return Err(ConicDefinitionError::InvalidSignedSweep { signed_sweep });
        }
        let arc = Self {
            ellipse,
            start_angle,
            signed_sweep,
        };
        validate_arc_endpoints(arc)?;
        Ok(arc)
    }

    #[must_use]
    pub fn ellipse(self) -> Ellipse2 {
        self.ellipse
    }

    #[must_use]
    pub fn start_angle(self) -> f64 {
        self.start_angle
    }

    #[must_use]
    pub fn signed_sweep(self) -> f64 {
        self.signed_sweep
    }

    /// Reports axis orientation observability for this directed trimmed curve.
    #[must_use]
    pub fn axis_observability(self) -> EllipseAxisObservability {
        match self.ellipse.axis_observability() {
            EllipseAxisObservability::UnobservableCircleLimit => {
                EllipseAxisObservability::ObservableByDirectedTrim
            }
            observable => observable,
        }
    }

    /// Evaluates the first endpoint with independent finite-result validation.
    ///
    /// # Errors
    ///
    /// Returns a typed curve failure if the endpoint is not representable.
    pub fn start_point(self) -> Result<Point2<f64>, ConicEvaluationError> {
        Ok(elliptical_arc_jet(&self, 0.0)?.position)
    }

    /// Evaluates the second endpoint with independent finite-result validation.
    ///
    /// # Errors
    ///
    /// Returns a typed curve failure if the endpoint is not representable.
    pub fn end_point(self) -> Result<Point2<f64>, ConicEvaluationError> {
        Ok(elliptical_arc_jet(&self, 1.0)?.position)
    }
}

/// The proper conic represented by a regular rational quadratic segment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProperConicKind {
    Ellipse,
    Parabola,
    Hyperbola,
}

/// A rational quadratic conic segment with endpoint weights fixed to one.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RationalQuadraticConicSegment2 {
    start: Point2<f64>,
    weighted_middle: Vector2<f64>,
    middle_weight: f64,
    end: Point2<f64>,
}

impl RationalQuadraticConicSegment2 {
    /// Constructs a segment from the homogeneous middle control `(Q, w)`.
    ///
    /// Endpoint controls are `(start, 1)` and `(end, 1)`.
    ///
    /// # Errors
    ///
    /// Returns a typed error for non-finite or collinear homogeneous controls, or
    /// for a denominator pole anywhere on `[0, 1]`.
    pub fn try_new(
        start: Point2<f64>,
        weighted_middle: Vector2<f64>,
        middle_weight: f64,
        end: Point2<f64>,
    ) -> Result<Self, ConicDefinitionError> {
        validate_point(start)?;
        validate_vector(weighted_middle)?;
        validate_definition_parameter(middle_weight)?;
        validate_point(end)?;
        if middle_weight <= -1.0 {
            return Err(ConicDefinitionError::RationalDenominatorPole { middle_weight });
        }
        if !homogeneous_controls_are_nondegenerate(start, weighted_middle, middle_weight, end) {
            return Err(ConicDefinitionError::DegenerateHomogeneousControls);
        }
        Ok(Self {
            start,
            weighted_middle,
            middle_weight,
            end,
        })
    }

    /// Explicitly named homogeneous-middle constructor.
    ///
    /// # Errors
    ///
    /// Returns the same failures as [`Self::try_new`].
    pub fn try_from_homogeneous_middle(
        start: Point2<f64>,
        weighted_middle: Vector2<f64>,
        middle_weight: f64,
        end: Point2<f64>,
    ) -> Result<Self, ConicDefinitionError> {
        Self::try_new(start, weighted_middle, middle_weight, end)
    }

    /// Constructs a segment from an ordinary middle control and its weight.
    ///
    /// # Errors
    ///
    /// In addition to homogeneous-constructor failures, zero weight rejects because
    /// an ordinary point cannot specify the direction represented by `(Q, 0)`.
    pub fn try_from_control_point(
        start: Point2<f64>,
        middle: Point2<f64>,
        middle_weight: f64,
        end: Point2<f64>,
    ) -> Result<Self, ConicDefinitionError> {
        validate_point(start)?;
        validate_point(middle)?;
        validate_definition_parameter(middle_weight)?;
        validate_point(end)?;
        if middle_weight == 0.0 {
            return Err(ConicDefinitionError::ZeroWeightOrdinaryControl);
        }
        let weighted_middle = middle.coords * middle_weight;
        validate_vector(weighted_middle)?;
        Self::try_new(start, weighted_middle, middle_weight, end)
    }

    #[must_use]
    pub fn start(self) -> Point2<f64> {
        self.start
    }

    #[must_use]
    pub fn weighted_middle(self) -> Vector2<f64> {
        self.weighted_middle
    }

    #[must_use]
    pub fn middle_weight(self) -> f64 {
        self.middle_weight
    }

    #[must_use]
    pub fn end(self) -> Point2<f64> {
        self.end
    }

    #[must_use]
    pub fn start_weight(self) -> f64 {
        1.0
    }

    #[must_use]
    pub fn end_weight(self) -> f64 {
        1.0
    }

    /// Returns all three homogeneous controls as `(weighted_coordinates, weight)`.
    #[must_use]
    pub fn homogeneous_controls(self) -> [(Vector2<f64>, f64); 3] {
        [
            (self.start.coords, 1.0),
            (self.weighted_middle, self.middle_weight),
            (self.end.coords, 1.0),
        ]
    }

    #[must_use]
    pub fn start_point(self) -> Point2<f64> {
        self.start
    }

    #[must_use]
    pub fn end_point(self) -> Point2<f64> {
        self.end
    }

    #[must_use]
    #[allow(clippy::float_cmp)] // Proper-conic classification is exact in homogeneous weight.
    pub fn proper_conic_kind(self) -> ProperConicKind {
        if self.middle_weight.abs() < 1.0 {
            ProperConicKind::Ellipse
        } else if self.middle_weight == 1.0 {
            ProperConicKind::Parabola
        } else {
            ProperConicKind::Hyperbola
        }
    }
}

/// A directed, trimmed parabola in its vertex-axis parameterization.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ParabolaSegment2 {
    vertex: Point2<f64>,
    opening_axis: UnitDirection2,
    focal_length: f64,
    trim: DirectedParameterTrim,
}

impl ParabolaSegment2 {
    /// Constructs a directed trimmed parabola.
    ///
    /// # Errors
    ///
    /// Returns a typed error for a non-finite vertex or non-positive focal length.
    pub fn try_new(
        vertex: Point2<f64>,
        opening_axis: UnitDirection2,
        focal_length: f64,
        trim: DirectedParameterTrim,
    ) -> Result<Self, ConicDefinitionError> {
        validate_point(vertex)?;
        if !focal_length.is_finite() || focal_length <= 0.0 {
            return Err(ConicDefinitionError::InvalidFocalLength { focal_length });
        }
        let parabola = Self {
            vertex,
            opening_axis,
            focal_length,
            trim,
        };
        validate_parabola_features(parabola)?;
        Ok(parabola)
    }

    #[must_use]
    pub fn vertex(self) -> Point2<f64> {
        self.vertex
    }

    #[must_use]
    pub fn opening_axis(self) -> UnitDirection2 {
        self.opening_axis
    }

    #[must_use]
    pub fn directed_axis(self) -> UnitDirection2 {
        self.opening_axis
    }

    #[must_use]
    pub fn focal_length(self) -> f64 {
        self.focal_length
    }

    #[must_use]
    pub fn trim(self) -> DirectedParameterTrim {
        self.trim
    }

    #[must_use]
    pub fn focus(self) -> Point2<f64> {
        self.vertex + self.opening_axis.vector() * self.focal_length
    }

    /// Evaluates the first endpoint with independent finite-result validation.
    ///
    /// # Errors
    ///
    /// Returns a typed curve failure if the endpoint is not representable.
    pub fn start_point(self) -> Result<Point2<f64>, ConicEvaluationError> {
        Ok(parabola_segment_jet(&self, 0.0)?.position)
    }

    /// Evaluates the second endpoint with independent finite-result validation.
    ///
    /// # Errors
    ///
    /// Returns a typed curve failure if the endpoint is not representable.
    pub fn end_point(self) -> Result<Point2<f64>, ConicEvaluationError> {
        Ok(parabola_segment_jet(&self, 1.0)?.position)
    }
}

/// Explicit selection of one connected hyperbola branch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HyperbolaBranch {
    Positive,
    Negative,
}

impl HyperbolaBranch {
    #[must_use]
    pub fn multiplier(self) -> f64 {
        match self {
            Self::Positive => 1.0,
            Self::Negative => -1.0,
        }
    }
}

/// A directed, trimmed branch of a hyperbola.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HyperbolaSegment2 {
    center: Point2<f64>,
    transverse_axis: UnitDirection2,
    semi_transverse: f64,
    semi_conjugate: f64,
    branch: HyperbolaBranch,
    trim: DirectedParameterTrim,
}

impl HyperbolaSegment2 {
    /// Constructs one explicit directed hyperbola branch.
    ///
    /// # Errors
    ///
    /// Returns a typed error for a non-finite center or non-positive semiaxis.
    pub fn try_new(
        center: Point2<f64>,
        transverse_axis: UnitDirection2,
        semi_transverse: f64,
        semi_conjugate: f64,
        branch: HyperbolaBranch,
        trim: DirectedParameterTrim,
    ) -> Result<Self, ConicDefinitionError> {
        validate_point(center)?;
        for length in [semi_transverse, semi_conjugate] {
            if !length.is_finite() || length <= 0.0 {
                return Err(ConicDefinitionError::InvalidHyperbolaSemiaxis { length });
            }
        }
        let hyperbola = Self {
            center,
            transverse_axis,
            semi_transverse,
            semi_conjugate,
            branch,
            trim,
        };
        validate_hyperbola_features(hyperbola)?;
        Ok(hyperbola)
    }

    #[must_use]
    pub fn center(self) -> Point2<f64> {
        self.center
    }

    #[must_use]
    pub fn transverse_axis(self) -> UnitDirection2 {
        self.transverse_axis
    }

    #[must_use]
    pub fn directed_transverse_axis(self) -> UnitDirection2 {
        self.transverse_axis
    }

    #[must_use]
    pub fn conjugate_axis(self) -> UnitDirection2 {
        self.transverse_axis.left_normal()
    }

    #[must_use]
    pub fn directed_conjugate_axis(self) -> UnitDirection2 {
        self.conjugate_axis()
    }

    #[must_use]
    pub fn semi_transverse(self) -> f64 {
        self.semi_transverse
    }

    #[must_use]
    pub fn semi_conjugate(self) -> f64 {
        self.semi_conjugate
    }

    #[must_use]
    pub fn branch(self) -> HyperbolaBranch {
        self.branch
    }

    #[must_use]
    pub fn trim(self) -> DirectedParameterTrim {
        self.trim
    }

    #[must_use]
    pub fn focal_distance(self) -> f64 {
        self.semi_transverse.hypot(self.semi_conjugate)
    }

    /// Returns the negative-axis and positive-axis foci, in that order.
    #[must_use]
    pub fn foci(self) -> [Point2<f64>; 2] {
        let offset = self.transverse_axis.vector() * self.focal_distance();
        [self.center - offset, self.center + offset]
    }

    #[must_use]
    pub fn selected_branch_focus(self) -> Point2<f64> {
        self.center
            + self.transverse_axis.vector() * (self.branch.multiplier() * self.focal_distance())
    }

    #[must_use]
    pub fn selected_branch_vertex(self) -> Point2<f64> {
        self.center
            + self.transverse_axis.vector() * (self.branch.multiplier() * self.semi_transverse)
    }

    /// Returns the world-space unit direction whose dot product identifies this branch.
    #[must_use]
    pub fn branch_witness(self) -> Vector2<f64> {
        self.transverse_axis.vector() * self.branch.multiplier()
    }

    #[must_use]
    pub fn transverse_axis_length(self) -> f64 {
        2.0 * self.semi_transverse
    }

    #[must_use]
    pub fn conjugate_axis_length(self) -> f64 {
        2.0 * self.semi_conjugate
    }

    /// Evaluates the first endpoint with independent finite-result validation.
    ///
    /// # Errors
    ///
    /// Returns a typed curve failure if the endpoint is not representable.
    pub fn start_point(self) -> Result<Point2<f64>, ConicEvaluationError> {
        Ok(hyperbola_segment_jet(&self, 0.0)?.position)
    }

    /// Evaluates the second endpoint with independent finite-result validation.
    ///
    /// # Errors
    ///
    /// Returns a typed curve failure if the endpoint is not representable.
    pub fn end_point(self) -> Result<Point2<f64>, ConicEvaluationError> {
        Ok(hyperbola_segment_jet(&self, 1.0)?.position)
    }
}

/// Evaluates an ellipse at one unwrapped angle.
///
/// # Errors
///
/// Returns a typed parameter, zero-speed, or non-finite-jet failure.
pub fn ellipse_jet(ellipse: &Ellipse2, angle: f64) -> Result<CurveJet2, ConicEvaluationError> {
    let domain = periodic_domain();
    validate_evaluation_parameter(domain, angle)?;
    let (sine, cosine) = angle.sin_cos();
    let major = ellipse.major_axis.vector();
    let minor = ellipse.directed_minor_axis().vector();
    let major_cosine = major * (ellipse.semi_major * cosine);
    let minor_sine = minor * (ellipse.semi_minor * sine);
    checked_jet(CurveJet2 {
        position: ellipse.center + major_cosine + minor_sine,
        first_derivative: major * (-ellipse.semi_major * sine)
            + minor * (ellipse.semi_minor * cosine),
        second_derivative: -major_cosine - minor_sine,
        third_derivative: major * (ellipse.semi_major * sine)
            - minor * (ellipse.semi_minor * cosine),
        domain,
    })
}

/// Evaluates a directed elliptical arc over `[0, 1]`.
///
/// # Errors
///
/// Returns a typed parameter, zero-speed, or non-finite-jet failure.
pub fn elliptical_arc_jet(
    arc: &EllipticalArc2,
    parameter: f64,
) -> Result<CurveJet2, ConicEvaluationError> {
    let domain = unit_interval();
    validate_evaluation_parameter(domain, parameter)?;
    let angle = arc.signed_sweep.mul_add(parameter, arc.start_angle);
    if !angle.is_finite() {
        return Err(non_finite_jet_error());
    }
    let base = ellipse_jet(&arc.ellipse, angle)?;
    let rate_squared = arc.signed_sweep * arc.signed_sweep;
    let rate_cubed = rate_squared * arc.signed_sweep;
    checked_jet(CurveJet2 {
        position: base.position,
        first_derivative: base.first_derivative * arc.signed_sweep,
        second_derivative: base.second_derivative * rate_squared,
        third_derivative: base.third_derivative * rate_cubed,
        domain,
    })
}

/// Evaluates a regular rational quadratic conic segment over `[0, 1]`.
///
/// # Errors
///
/// Returns a typed parameter or finite-jet failure. A denominator at or below the
/// documented scale-aware machine band returns [`ConicEvaluationError::RationalDenominator`].
pub fn rational_quadratic_conic_jet(
    conic: &RationalQuadraticConicSegment2,
    parameter: f64,
) -> Result<CurveJet2, ConicEvaluationError> {
    let domain = unit_interval();
    validate_evaluation_parameter(domain, parameter)?;

    let one_minus = 1.0 - parameter;
    let b0 = one_minus * one_minus;
    let b1 = 2.0 * one_minus * parameter;
    let b2 = parameter * parameter;
    let weighted_b1 = conic.middle_weight * b1;
    let denominator = b0 + weighted_b1 + b2;
    let condition_scale = b0.abs() + weighted_b1.abs() + b2.abs();
    if !denominator.is_finite() || !condition_scale.is_finite() {
        return Err(non_finite_jet_error());
    }
    if denominator.abs() <= RATIONAL_DENOMINATOR_FACTOR * condition_scale {
        return Err(ConicEvaluationError::RationalDenominator {
            parameter,
            denominator,
            condition_scale,
        });
    }

    let b0_first = -2.0 * one_minus;
    let b1_first = 2.0 * (1.0 - 2.0 * parameter);
    let b2_first = 2.0 * parameter;
    let numerator = conic.start.coords * b0 + conic.weighted_middle * b1 + conic.end.coords * b2;
    let numerator_first = conic.start.coords * b0_first
        + conic.weighted_middle * b1_first
        + conic.end.coords * b2_first;
    let numerator_second =
        (conic.start.coords - conic.weighted_middle * 2.0 + conic.end.coords) * 2.0;
    let denominator_first = b0_first + conic.middle_weight * b1_first + b2_first;
    let denominator_second = 4.0 * (1.0 - conic.middle_weight);

    let position_coordinates = numerator / denominator;
    let first = (numerator_first - position_coordinates * denominator_first) / denominator;
    let second = (numerator_second
        - position_coordinates * denominator_second
        - first * (2.0 * denominator_first))
        / denominator;
    let third =
        (first * (-3.0 * denominator_second) - second * (3.0 * denominator_first)) / denominator;
    checked_jet(CurveJet2 {
        position: Point2::from(position_coordinates),
        first_derivative: first,
        second_derivative: second,
        third_derivative: third,
        domain,
    })
}

/// Evaluates a directed trimmed parabola over `[0, 1]`.
///
/// # Errors
///
/// Returns a typed parameter, zero-speed, or non-finite-jet failure.
pub fn parabola_segment_jet(
    parabola: &ParabolaSegment2,
    parameter: f64,
) -> Result<CurveJet2, ConicEvaluationError> {
    let domain = unit_interval();
    validate_evaluation_parameter(domain, parameter)?;
    let native_parameter = parabola.trim.parameter_at(parameter);
    let rate = parabola.trim.signed_rate;
    let rate_squared = rate * rate;
    let axis = parabola.opening_axis.vector();
    let normal = parabola.opening_axis.left_normal().vector();
    let twice_focal = 2.0 * parabola.focal_length;
    checked_jet(CurveJet2 {
        position: parabola.vertex
            + axis * (parabola.focal_length * native_parameter * native_parameter)
            + normal * (twice_focal * native_parameter),
        first_derivative: (axis * (twice_focal * native_parameter) + normal * twice_focal) * rate,
        second_derivative: axis * (twice_focal * rate_squared),
        third_derivative: Vector2::zeros(),
        domain,
    })
}

/// Evaluates one explicit directed hyperbola branch over `[0, 1]`.
///
/// # Errors
///
/// Returns a typed parameter, zero-speed, or non-finite-jet failure, including
/// overflow from `sinh`, `cosh`, or chain-rule scaling.
pub fn hyperbola_segment_jet(
    hyperbola: &HyperbolaSegment2,
    parameter: f64,
) -> Result<CurveJet2, ConicEvaluationError> {
    let domain = unit_interval();
    validate_evaluation_parameter(domain, parameter)?;
    let native_parameter = hyperbola.trim.parameter_at(parameter);
    let sine = native_parameter.sinh();
    let cosine = native_parameter.cosh();
    let rate = hyperbola.trim.signed_rate;
    let rate_squared = rate * rate;
    let rate_cubed = rate_squared * rate;
    let axis = hyperbola.transverse_axis.vector();
    let normal = hyperbola.transverse_axis.left_normal().vector();
    let signed_transverse = hyperbola.branch.multiplier() * hyperbola.semi_transverse;
    let position_offset =
        axis * (signed_transverse * cosine) + normal * (hyperbola.semi_conjugate * sine);
    let native_first =
        axis * (signed_transverse * sine) + normal * (hyperbola.semi_conjugate * cosine);
    checked_jet(CurveJet2 {
        position: hyperbola.center + position_offset,
        first_derivative: native_first * rate,
        second_derivative: position_offset * rate_squared,
        third_derivative: native_first * rate_cubed,
        domain,
    })
}

fn validate_point(point: Point2<f64>) -> Result<(), ConicDefinitionError> {
    if point.coords.iter().all(|component| component.is_finite()) {
        Ok(())
    } else {
        Err(ConicDefinitionError::NonFinitePoint)
    }
}

fn validate_vector(vector: Vector2<f64>) -> Result<(), ConicDefinitionError> {
    if vector.iter().all(|component| component.is_finite()) {
        Ok(())
    } else {
        Err(ConicDefinitionError::NonFiniteVector)
    }
}

fn validate_definition_parameter(parameter: f64) -> Result<(), ConicDefinitionError> {
    if parameter.is_finite() {
        Ok(())
    } else {
        Err(ConicDefinitionError::NonFiniteParameter { parameter })
    }
}

fn validate_ellipse_features(ellipse: Ellipse2) -> Result<(), ConicDefinitionError> {
    let mut points = [
        ellipse.foci(),
        ellipse.major_axis_endpoints(),
        ellipse.minor_axis_endpoints(),
    ]
    .into_iter()
    .flatten();
    let scalars = [
        ellipse.linear_eccentricity(),
        ellipse.major_axis_length(),
        ellipse.minor_axis_length(),
    ];
    if points.all(point_is_finite) && scalars.into_iter().all(f64::is_finite) {
        Ok(())
    } else {
        Err(nonrepresentable(
            "ellipse foci, axis endpoints, or measurements",
        ))
    }
}

fn validate_arc_endpoints(arc: EllipticalArc2) -> Result<(), ConicDefinitionError> {
    if arc.start_point().is_ok() && arc.end_point().is_ok() {
        Ok(())
    } else {
        Err(nonrepresentable("elliptical arc endpoints"))
    }
}

fn validate_parabola_features(parabola: ParabolaSegment2) -> Result<(), ConicDefinitionError> {
    if point_is_finite(parabola.focus())
        && parabola.start_point().is_ok()
        && parabola.end_point().is_ok()
    {
        Ok(())
    } else {
        Err(nonrepresentable("parabola focus or endpoints"))
    }
}

fn validate_hyperbola_features(hyperbola: HyperbolaSegment2) -> Result<(), ConicDefinitionError> {
    let mut points = [
        hyperbola.foci(),
        [
            hyperbola.selected_branch_focus(),
            hyperbola.selected_branch_vertex(),
        ],
    ]
    .into_iter()
    .flatten();
    let scalars = [
        hyperbola.focal_distance(),
        hyperbola.transverse_axis_length(),
        hyperbola.conjugate_axis_length(),
    ];
    if points.all(point_is_finite)
        && scalars.into_iter().all(f64::is_finite)
        && hyperbola.start_point().is_ok()
        && hyperbola.end_point().is_ok()
    {
        Ok(())
    } else {
        Err(nonrepresentable(
            "hyperbola foci, vertices, endpoints, or measurements",
        ))
    }
}

fn point_is_finite(point: Point2<f64>) -> bool {
    point.coords.iter().all(|component| component.is_finite())
}

const fn nonrepresentable(feature: &'static str) -> ConicDefinitionError {
    ConicDefinitionError::NonRepresentableDerivedGeometry { feature }
}

fn homogeneous_controls_are_nondegenerate(
    start: Point2<f64>,
    weighted_middle: Vector2<f64>,
    middle_weight: f64,
    end: Point2<f64>,
) -> bool {
    let mut controls = [
        [start.x, start.y, 1.0],
        [weighted_middle.x, weighted_middle.y, middle_weight],
        [end.x, end.y, 1.0],
    ];
    for control in &mut controls {
        let scale = control
            .iter()
            .map(|component| component.abs())
            .fold(0.0, f64::max);
        if scale == 0.0 || !scale.is_finite() {
            return false;
        }
        for component in control {
            *component /= scale;
        }
    }
    let first = controls[0];
    let second = controls[1];
    let third = controls[2];
    let determinant = first[0] * (second[1] * third[2] - second[2] * third[1])
        - first[1] * (second[0] * third[2] - second[2] * third[0])
        + first[2] * (second[0] * third[1] - second[1] * third[0]);
    determinant.is_finite() && determinant != 0.0
}

fn unit_interval() -> CurveParameterDomain {
    CurveParameterDomain::Bounded {
        lower: 0.0,
        upper: 1.0,
    }
}

fn periodic_domain() -> CurveParameterDomain {
    CurveParameterDomain::Periodic { period: TAU }
}

fn validate_evaluation_parameter(
    domain: CurveParameterDomain,
    parameter: f64,
) -> Result<(), ConicEvaluationError> {
    if !parameter.is_finite() {
        return Err(
            CurveEvaluationError::from(CurveParameterError::NonFinite { parameter }).into(),
        );
    }
    if domain.contains(parameter) {
        Ok(())
    } else {
        Err(
            CurveEvaluationError::from(CurveParameterError::OutOfDomain { parameter, domain })
                .into(),
        )
    }
}

fn checked_jet(jet: CurveJet2) -> Result<CurveJet2, ConicEvaluationError> {
    let finite = jet.position.coords.iter().all(|value| value.is_finite())
        && jet.first_derivative.iter().all(|value| value.is_finite())
        && jet.second_derivative.iter().all(|value| value.is_finite())
        && jet.third_derivative.iter().all(|value| value.is_finite());
    if !finite {
        Err(non_finite_jet_error())
    } else if jet.first_derivative.x.hypot(jet.first_derivative.y) == 0.0 {
        Err(CurveEvaluationError::from(CurveRegularityError::ZeroSpeed).into())
    } else {
        Ok(jet)
    }
}

fn non_finite_jet_error() -> ConicEvaluationError {
    CurveEvaluationError::from(CurveRegularityError::NonFiniteJet).into()
}
