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
