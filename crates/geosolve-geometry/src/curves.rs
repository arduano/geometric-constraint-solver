use std::f64::consts::TAU;

use thiserror::Error;

use crate::{Point2, Vector2};

/// Parameter domain reported by immutable curve evaluation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CurveParameterDomain {
    SupportingLine,
    Bounded { lower: f64, upper: f64 },
    Periodic { period: f64 },
}

impl CurveParameterDomain {
    /// Reports whether a parameter is finite and belongs to this domain.
    #[must_use]
    pub fn contains(self, parameter: f64) -> bool {
        parameter.is_finite()
            && match self {
                Self::SupportingLine => true,
                Self::Bounded { lower, upper } => {
                    lower.is_finite()
                        && upper.is_finite()
                        && lower < upper
                        && (lower..=upper).contains(&parameter)
                }
                Self::Periodic { period } => period.is_finite() && period > 0.0,
            }
    }
}

/// Finite position and parameter derivatives through order three.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CurveJet2 {
    pub position: Point2<f64>,
    pub first_derivative: Vector2<f64>,
    pub second_derivative: Vector2<f64>,
    pub third_derivative: Vector2<f64>,
    pub domain: CurveParameterDomain,
}

/// Regular planar differential geometry derived from one immutable curve jet.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CurveDifferential2 {
    pub unit_tangent: Vector2<f64>,
    pub left_normal: Vector2<f64>,
    pub signed_curvature: f64,
}

impl CurveDifferential2 {
    #[must_use]
    pub fn unsigned_curvature(self) -> f64 {
        self.signed_curvature.abs()
    }

    #[must_use]
    pub fn curvature_vector(self) -> Vector2<f64> {
        self.left_normal * self.signed_curvature
    }

    /// Returns the finite positive osculating radius.
    ///
    /// # Errors
    ///
    /// Zero curvature has no finite osculating circle. A finite curvature whose
    /// reciprocal is not representable also returns a typed error.
    pub fn osculating_radius(self) -> Result<f64, CurveDifferentialError> {
        if self.signed_curvature == 0.0 {
            return Err(CurveDifferentialError::UndefinedOsculatingRadius);
        }
        let radius = self.signed_curvature.abs().recip();
        if radius.is_finite() && radius > 0.0 {
            Ok(radius)
        } else {
            Err(CurveDifferentialError::UnrepresentableOsculatingRadius)
        }
    }
}

/// Typed differential-geometry failure from an immutable curve jet.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
#[non_exhaustive]
pub enum CurveDifferentialError {
    #[error("curve differential geometry requires a finite jet")]
    NonFiniteJet,
    #[error("curve differential geometry is undefined at zero speed")]
    ZeroSpeed,
    #[error("curve curvature is outside the finite representation")]
    UnrepresentableCurvature,
    #[error("zero curvature has no finite osculating radius")]
    UndefinedOsculatingRadius,
    #[error("the finite osculating radius is outside the finite representation")]
    UnrepresentableOsculatingRadius,
}

impl CurveJet2 {
    /// Derives a unit tangent, left normal, and signed curvature without changing
    /// the curve's parameter orientation.
    ///
    /// # Errors
    ///
    /// Rejects non-finite jets, zero speed, and unrepresentable curvature.
    pub fn differential(self) -> Result<CurveDifferential2, CurveDifferentialError> {
        let finite = self.position.x.is_finite()
            && self.position.y.is_finite()
            && self.first_derivative.x.is_finite()
            && self.first_derivative.y.is_finite()
            && self.second_derivative.x.is_finite()
            && self.second_derivative.y.is_finite()
            && self.third_derivative.x.is_finite()
            && self.third_derivative.y.is_finite();
        if !finite {
            return Err(CurveDifferentialError::NonFiniteJet);
        }

        let speed_scale = self
            .first_derivative
            .x
            .abs()
            .max(self.first_derivative.y.abs());
        if speed_scale == 0.0 {
            return Err(CurveDifferentialError::ZeroSpeed);
        }
        let scaled_velocity = self.first_derivative / speed_scale;
        let scaled_speed = scaled_velocity.x.hypot(scaled_velocity.y);
        let unit_tangent = scaled_velocity / scaled_speed;
        let left_normal = Vector2::new(-unit_tangent.y, unit_tangent.x);

        let acceleration_scale = self
            .second_derivative
            .x
            .abs()
            .max(self.second_derivative.y.abs());
        let signed_curvature = if acceleration_scale == 0.0 {
            0.0
        } else {
            signed_curvature_from_jet(
                self.first_derivative,
                self.second_derivative,
                left_normal,
                speed_scale,
                scaled_speed,
                acceleration_scale,
            )?
        };
        if !unit_tangent.x.is_finite()
            || !unit_tangent.y.is_finite()
            || !signed_curvature.is_finite()
        {
            return Err(CurveDifferentialError::UnrepresentableCurvature);
        }
        Ok(CurveDifferential2 {
            unit_tangent,
            left_normal,
            signed_curvature,
        })
    }
}

fn compensated_dot(first: Vector2<f64>, second: Vector2<f64>) -> f64 {
    let first_product = first.x * second.x;
    let second_product = first.y * second.y;
    let sum = first_product + second_product;
    let second_virtual = sum - first_product;
    let first_virtual = sum - second_virtual;
    let sum_error = (first_product - first_virtual) + (second_product - second_virtual);
    let first_error = first.x.mul_add(second.x, -first_product);
    let second_error = first.y.mul_add(second.y, -second_product);
    sum + (sum_error + first_error + second_error)
}

fn compensated_determinant(first: Vector2<f64>, second: Vector2<f64>) -> f64 {
    compensated_dot(Vector2::new(-first.y, first.x), second)
}

fn signed_curvature_from_jet(
    velocity: Vector2<f64>,
    acceleration: Vector2<f64>,
    left_normal: Vector2<f64>,
    speed_scale: f64,
    scaled_speed: f64,
    acceleration_scale: f64,
) -> Result<f64, CurveDifferentialError> {
    let positive = velocity.x * acceleration.y;
    let negative = velocity.y * acceleration.x;
    let products_represented = positive.is_finite()
        && negative.is_finite()
        && (velocity.x == 0.0 || acceleration.y == 0.0 || positive != 0.0)
        && (velocity.y == 0.0 || acceleration.x == 0.0 || negative != 0.0);
    if products_represented {
        let determinant = compensated_determinant(velocity, acceleration);
        if determinant != 0.0 {
            let normal_acceleration = [
                determinant / speed_scale / scaled_speed,
                determinant / scaled_speed / speed_scale,
            ]
            .into_iter()
            .find(|value| value.is_finite() && *value != 0.0)
            .ok_or(CurveDifferentialError::UnrepresentableCurvature)?;
            return representable_curvature(
                normal_acceleration / (scaled_speed * scaled_speed),
                1.0,
                speed_scale,
            );
        }
    }
    let normal_acceleration = compensated_dot(left_normal, acceleration);
    if normal_acceleration.is_finite()
        && normal_acceleration != 0.0
        && let Ok(curvature) = representable_curvature(
            normal_acceleration / (scaled_speed * scaled_speed),
            1.0,
            speed_scale,
        )
    {
        return Ok(curvature);
    }
    let scaled_acceleration = acceleration / acceleration_scale;
    let scaled_projection = compensated_dot(left_normal, scaled_acceleration);
    if scaled_projection == 0.0 {
        Ok(0.0)
    } else {
        representable_curvature(
            scaled_projection / (scaled_speed * scaled_speed),
            acceleration_scale,
            speed_scale,
        )
    }
}

fn representable_curvature(
    shape: f64,
    acceleration_scale: f64,
    speed_scale: f64,
) -> Result<f64, CurveDifferentialError> {
    [
        (shape * acceleration_scale / speed_scale) / speed_scale,
        (shape / speed_scale) * (acceleration_scale / speed_scale),
        (shape / speed_scale / speed_scale) * acceleration_scale,
    ]
    .into_iter()
    .find(|value| value.is_finite() && *value != 0.0)
    .ok_or(CurveDifferentialError::UnrepresentableCurvature)
}

/// Typed parameter failure from immutable curve evaluation.
#[derive(Clone, Copy, Debug, Error, PartialEq)]
#[non_exhaustive]
pub enum CurveParameterError {
    #[error("curve parameter must be finite, got {parameter}")]
    NonFinite { parameter: f64 },
    #[error("curve parameter {parameter} is outside {domain:?}")]
    OutOfDomain {
        parameter: f64,
        domain: CurveParameterDomain,
    },
}

/// Typed regularity failure from immutable curve evaluation.
#[derive(Clone, Copy, Debug, Error, PartialEq)]
#[non_exhaustive]
pub enum CurveRegularityError {
    #[error("curve definition contains a non-finite value")]
    NonFiniteDefinition,
    #[error("curve radius must be positive and finite, got {radius}")]
    InvalidRadius { radius: f64 },
    #[error("curve has zero speed at the selected parameter")]
    ZeroSpeed,
    #[error("curve evaluation produced a non-finite jet")]
    NonFiniteJet,
}

/// Typed immutable curve-evaluation failure.
#[derive(Clone, Copy, Debug, Error, PartialEq)]
#[non_exhaustive]
pub enum CurveEvaluationError {
    #[error(transparent)]
    Parameter(#[from] CurveParameterError),
    #[error(transparent)]
    Regularity(#[from] CurveRegularityError),
}

/// Evaluates a supporting line or bounded segment.
///
/// # Errors
///
/// Returns a typed parameter, definition, zero-speed, or non-finite-jet failure.
pub fn line_jet(
    start: Point2<f64>,
    end: Point2<f64>,
    domain: CurveParameterDomain,
    parameter: f64,
) -> Result<CurveJet2, CurveEvaluationError> {
    validate_points(&[start, end])?;
    validate_domain(domain, parameter)?;
    if !matches!(
        domain,
        CurveParameterDomain::SupportingLine | CurveParameterDomain::Bounded { .. }
    ) {
        return Err(CurveRegularityError::NonFiniteDefinition.into());
    }
    let derivative = end - start;
    checked_jet(CurveJet2 {
        position: start + derivative * parameter,
        first_derivative: derivative,
        second_derivative: Vector2::zeros(),
        third_derivative: Vector2::zeros(),
        domain,
    })
}

/// Evaluates a positive-radius circle at one unwrapped angle.
///
/// # Errors
///
/// Returns a typed parameter, definition, radius, or non-finite-jet failure.
pub fn circle_jet(
    center: Point2<f64>,
    radius: f64,
    angle: f64,
) -> Result<CurveJet2, CurveEvaluationError> {
    validate_points(&[center])?;
    validate_radius(radius)?;
    validate_domain(CurveParameterDomain::Periodic { period: TAU }, angle)?;
    radial_jet(
        center,
        radius,
        angle,
        1.0,
        CurveParameterDomain::Periodic { period: TAU },
    )
}

/// Evaluates a positive-radius circular arc over its bounded `[0, 1]` span.
///
/// # Errors
///
/// Returns a typed parameter, definition, radius, zero-speed, or non-finite-jet failure.
pub fn circular_arc_jet(
    center: Point2<f64>,
    radius: f64,
    start_angle: f64,
    signed_sweep: f64,
    parameter: f64,
) -> Result<CurveJet2, CurveEvaluationError> {
    validate_points(&[center])?;
    validate_radius(radius)?;
    if !start_angle.is_finite() || !signed_sweep.is_finite() {
        return Err(CurveRegularityError::NonFiniteDefinition.into());
    }
    if signed_sweep == 0.0 {
        return Err(CurveRegularityError::ZeroSpeed.into());
    }
    let domain = unit_interval();
    validate_domain(domain, parameter)?;
    radial_jet(
        center,
        radius,
        start_angle + signed_sweep * parameter,
        signed_sweep,
        domain,
    )
}

/// Evaluates a quadratic Bezier over `[0, 1]`.
///
/// # Errors
///
/// Returns a typed parameter, definition, zero-speed, or non-finite-jet failure.
pub fn quadratic_bezier_jet(
    control: [Point2<f64>; 3],
    parameter: f64,
) -> Result<CurveJet2, CurveEvaluationError> {
    validate_points(&control)?;
    let domain = unit_interval();
    validate_domain(domain, parameter)?;
    let [first, middle, last] = control;
    let one_minus = 1.0 - parameter;
    checked_jet(CurveJet2 {
        position: Point2::from(
            first.coords * (one_minus * one_minus)
                + middle.coords * (2.0 * one_minus * parameter)
                + last.coords * (parameter * parameter),
        ),
        first_derivative: (middle - first) * (2.0 * one_minus)
            + (last - middle) * (2.0 * parameter),
        second_derivative: (last.coords - middle.coords * 2.0 + first.coords) * 2.0,
        third_derivative: Vector2::zeros(),
        domain,
    })
}

/// Evaluates a cubic Bezier over `[0, 1]`.
///
/// # Errors
///
/// Returns a typed parameter, definition, zero-speed, or non-finite-jet failure.
pub fn cubic_bezier_jet(
    control: [Point2<f64>; 4],
    parameter: f64,
) -> Result<CurveJet2, CurveEvaluationError> {
    validate_points(&control)?;
    let domain = unit_interval();
    validate_domain(domain, parameter)?;
    let [first, second, third, last] = control;
    let one_minus = 1.0 - parameter;
    checked_jet(CurveJet2 {
        position: Point2::from(
            first.coords * one_minus.powi(3)
                + second.coords * (3.0 * one_minus * one_minus * parameter)
                + third.coords * (3.0 * one_minus * parameter * parameter)
                + last.coords * parameter.powi(3),
        ),
        first_derivative: (second - first) * (3.0 * one_minus * one_minus)
            + (third - second) * (6.0 * one_minus * parameter)
            + (last - third) * (3.0 * parameter * parameter),
        second_derivative: (third.coords - second.coords * 2.0 + first.coords) * (6.0 * one_minus)
            + (last.coords - third.coords * 2.0 + second.coords) * (6.0 * parameter),
        third_derivative: (last.coords - third.coords * 3.0 + second.coords * 3.0 - first.coords)
            * 6.0,
        domain,
    })
}

fn radial_jet(
    center: Point2<f64>,
    radius: f64,
    angle: f64,
    angle_rate: f64,
    domain: CurveParameterDomain,
) -> Result<CurveJet2, CurveEvaluationError> {
    let (sine, cosine) = angle.sin_cos();
    let rate_squared = angle_rate * angle_rate;
    let rate_cubed = rate_squared * angle_rate;
    checked_jet(CurveJet2 {
        position: center + Vector2::new(radius * cosine, radius * sine),
        first_derivative: Vector2::new(-radius * angle_rate * sine, radius * angle_rate * cosine),
        second_derivative: Vector2::new(
            -radius * rate_squared * cosine,
            -radius * rate_squared * sine,
        ),
        third_derivative: Vector2::new(radius * rate_cubed * sine, -radius * rate_cubed * cosine),
        domain,
    })
}

fn unit_interval() -> CurveParameterDomain {
    CurveParameterDomain::Bounded {
        lower: 0.0,
        upper: 1.0,
    }
}

fn validate_domain(
    domain: CurveParameterDomain,
    parameter: f64,
) -> Result<(), CurveEvaluationError> {
    if !parameter.is_finite() {
        return Err(CurveParameterError::NonFinite { parameter }.into());
    }
    let valid_definition = match domain {
        CurveParameterDomain::SupportingLine => true,
        CurveParameterDomain::Bounded { lower, upper } => {
            lower.is_finite() && upper.is_finite() && lower < upper
        }
        CurveParameterDomain::Periodic { period } => period.is_finite() && period > 0.0,
    };
    if !valid_definition {
        return Err(CurveRegularityError::NonFiniteDefinition.into());
    }
    if domain.contains(parameter) {
        Ok(())
    } else {
        Err(CurveParameterError::OutOfDomain { parameter, domain }.into())
    }
}

fn validate_points(points: &[Point2<f64>]) -> Result<(), CurveEvaluationError> {
    if points
        .iter()
        .all(|point| point.x.is_finite() && point.y.is_finite())
    {
        Ok(())
    } else {
        Err(CurveRegularityError::NonFiniteDefinition.into())
    }
}

fn validate_radius(radius: f64) -> Result<(), CurveEvaluationError> {
    if radius.is_finite() && radius > 0.0 {
        Ok(())
    } else {
        Err(CurveRegularityError::InvalidRadius { radius }.into())
    }
}

fn checked_jet(jet: CurveJet2) -> Result<CurveJet2, CurveEvaluationError> {
    let finite = jet.position.x.is_finite()
        && jet.position.y.is_finite()
        && jet.first_derivative.x.is_finite()
        && jet.first_derivative.y.is_finite()
        && jet.second_derivative.x.is_finite()
        && jet.second_derivative.y.is_finite()
        && jet.third_derivative.x.is_finite()
        && jet.third_derivative.y.is_finite();
    if finite {
        if jet.first_derivative.x.hypot(jet.first_derivative.y) == 0.0 {
            Err(CurveRegularityError::ZeroSpeed.into())
        } else {
            Ok(jet)
        }
    } else {
        Err(CurveRegularityError::NonFiniteJet.into())
    }
}
