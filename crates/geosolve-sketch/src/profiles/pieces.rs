// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_geometry::{BSplineBasis, BSplineForm, BSplineSpanIndex, Point2, Vector2};

use super::VisualProfileCurveFamily;
use super::interval::{
    Interval, Polynomial, atan2_point, cross_interval, polynomial_from_bernstein,
};
use crate::document::document_arc_signed_sweep;
use crate::{ConicGeometry, CurveDefinition, CurveSpan, SketchDocument};

#[derive(Clone, Copy, Debug)]
pub(super) struct Box2 {
    pub x: Interval,
    pub y: Interval,
}

impl Box2 {
    pub fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite()
    }

    pub fn disjoint(self, other: Self) -> bool {
        self.x.upper < other.x.lower
            || other.x.upper < self.x.lower
            || self.y.upper < other.y.lower
            || other.y.upper < self.y.lower
    }

    pub fn include(self, other: Self) -> Self {
        Self {
            x: Interval {
                lower: self.x.lower.min(other.x.lower),
                upper: self.x.upper.max(other.x.upper),
            },
            y: Interval {
                lower: self.y.lower.min(other.y.lower),
                upper: self.y.upper.max(other.y.upper),
            },
        }
    }

    pub fn contains_box(self, other: Self) -> bool {
        self.x.lower <= other.x.lower
            && self.x.upper >= other.x.upper
            && self.y.lower <= other.y.lower
            && self.y.upper >= other.y.upper
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum PieceEvaluationError {
    Pole,
    NonFinite,
}

#[derive(Clone, Debug)]
pub(super) struct CurvePiece {
    pub family: VisualProfileCurveFamily,
    pub kind: PieceKind,
}

#[derive(Clone, Debug)]
pub(super) enum PieceKind {
    Linear {
        start: [f64; 2],
        delta: [Interval; 2],
    },
    Circular {
        center: [f64; 2],
        radius: f64,
        angle_offset: Interval,
        angle_rate: f64,
    },
    Elliptic {
        center: [f64; 2],
        major: [f64; 2],
        minor: [f64; 2],
        angle_offset: f64,
        angle_rate: f64,
    },
    Polynomial {
        x: Polynomial,
        y: Polynomial,
    },
    Rational {
        x: Polynomial,
        y: Polynomial,
        weight: Polynomial,
    },
    Hyperbolic {
        center: [f64; 2],
        transverse: [f64; 2],
        conjugate: [f64; 2],
        native_offset: f64,
        native_rate: f64,
    },
}

impl CurvePiece {
    pub fn position(&self, parameter: Interval) -> Result<Box2, PieceEvaluationError> {
        let bounds = match &self.kind {
            PieceKind::Linear { start, delta } => Box2 {
                x: Interval::point(start[0]).add(parameter.mul(delta[0])),
                y: Interval::point(start[1]).add(parameter.mul(delta[1])),
            },
            PieceKind::Circular {
                center,
                radius,
                angle_offset,
                angle_rate,
            } => {
                let angle = angle_offset.add(parameter.mul(Interval::point(*angle_rate)));
                let cosine = angle.cos().map_err(|_| PieceEvaluationError::NonFinite)?;
                let sine = angle.sin().map_err(|_| PieceEvaluationError::NonFinite)?;
                Box2 {
                    x: Interval::point(center[0]).add(cosine.mul(Interval::point(*radius))),
                    y: Interval::point(center[1]).add(sine.mul(Interval::point(*radius))),
                }
            }
            PieceKind::Elliptic {
                center,
                major,
                minor,
                angle_offset,
                angle_rate,
            } => {
                let angle =
                    Interval::point(*angle_offset).add(parameter.mul(Interval::point(*angle_rate)));
                let cosine = angle.cos().map_err(|_| PieceEvaluationError::NonFinite)?;
                let sine = angle.sin().map_err(|_| PieceEvaluationError::NonFinite)?;
                Box2 {
                    x: Interval::point(center[0])
                        .add(cosine.mul(Interval::point(major[0])))
                        .add(sine.mul(Interval::point(minor[0]))),
                    y: Interval::point(center[1])
                        .add(cosine.mul(Interval::point(major[1])))
                        .add(sine.mul(Interval::point(minor[1]))),
                }
            }
            PieceKind::Polynomial { x, y } => Box2 {
                x: x.bezier_bound(parameter),
                y: y.bezier_bound(parameter),
            },
            PieceKind::Rational { x, y, weight } => {
                let denominator = weight.bezier_bound(parameter);
                Box2 {
                    x: x.bezier_bound(parameter)
                        .div(denominator)
                        .ok_or(PieceEvaluationError::Pole)?,
                    y: y.bezier_bound(parameter)
                        .div(denominator)
                        .ok_or(PieceEvaluationError::Pole)?,
                }
            }
            PieceKind::Hyperbolic {
                center,
                transverse,
                conjugate,
                native_offset,
                native_rate,
            } => {
                let native = Interval::point(*native_offset)
                    .add(parameter.mul(Interval::point(*native_rate)));
                let cosine = native.cosh().map_err(|_| PieceEvaluationError::NonFinite)?;
                let sine = native.sinh().map_err(|_| PieceEvaluationError::NonFinite)?;
                Box2 {
                    x: Interval::point(center[0])
                        .add(cosine.mul(Interval::point(transverse[0])))
                        .add(sine.mul(Interval::point(conjugate[0]))),
                    y: Interval::point(center[1])
                        .add(cosine.mul(Interval::point(transverse[1])))
                        .add(sine.mul(Interval::point(conjugate[1]))),
                }
            }
        };
        bounds
            .is_finite()
            .then_some(bounds)
            .ok_or(PieceEvaluationError::NonFinite)
    }

    pub fn derivative(&self, parameter: Interval) -> Result<[Interval; 2], PieceEvaluationError> {
        let derivative = match &self.kind {
            PieceKind::Linear { delta, .. } => *delta,
            PieceKind::Circular {
                radius,
                angle_offset,
                angle_rate,
                ..
            } => {
                let angle = angle_offset.add(parameter.mul(Interval::point(*angle_rate)));
                let rate = Interval::scalar_product(*radius, *angle_rate);
                [
                    angle
                        .sin()
                        .map_err(|_| PieceEvaluationError::NonFinite)?
                        .mul(rate.neg()),
                    angle
                        .cos()
                        .map_err(|_| PieceEvaluationError::NonFinite)?
                        .mul(rate),
                ]
            }
            PieceKind::Elliptic {
                major,
                minor,
                angle_offset,
                angle_rate,
                ..
            } => {
                let angle =
                    Interval::point(*angle_offset).add(parameter.mul(Interval::point(*angle_rate)));
                let sine = angle.sin().map_err(|_| PieceEvaluationError::NonFinite)?;
                let cosine = angle.cos().map_err(|_| PieceEvaluationError::NonFinite)?;
                [
                    sine.mul(Interval::scalar_product(-major[0], *angle_rate))
                        .add(cosine.mul(Interval::scalar_product(minor[0], *angle_rate))),
                    sine.mul(Interval::scalar_product(-major[1], *angle_rate))
                        .add(cosine.mul(Interval::scalar_product(minor[1], *angle_rate))),
                ]
            }
            PieceKind::Polynomial { x, y } => [
                x.derivative().bezier_bound(parameter),
                y.derivative().bezier_bound(parameter),
            ],
            PieceKind::Rational { x, y, weight } => {
                let denominator = weight.bezier_bound(parameter);
                if !denominator.excludes_zero() {
                    return Err(PieceEvaluationError::Pole);
                }
                let weight_derivative = weight.derivative();
                let denominator_squared = denominator.square();
                [
                    x.derivative()
                        .mul(weight)
                        .sub(&x.mul(&weight_derivative))
                        .bezier_bound(parameter)
                        .div(denominator_squared)
                        .ok_or(PieceEvaluationError::Pole)?,
                    y.derivative()
                        .mul(weight)
                        .sub(&y.mul(&weight_derivative))
                        .bezier_bound(parameter)
                        .div(denominator_squared)
                        .ok_or(PieceEvaluationError::Pole)?,
                ]
            }
            PieceKind::Hyperbolic {
                transverse,
                conjugate,
                native_offset,
                native_rate,
                ..
            } => {
                let native = Interval::point(*native_offset)
                    .add(parameter.mul(Interval::point(*native_rate)));
                let sine = native.sinh().map_err(|_| PieceEvaluationError::NonFinite)?;
                let cosine = native.cosh().map_err(|_| PieceEvaluationError::NonFinite)?;
                [
                    sine.mul(Interval::scalar_product(transverse[0], *native_rate))
                        .add(cosine.mul(Interval::scalar_product(conjugate[0], *native_rate))),
                    sine.mul(Interval::scalar_product(transverse[1], *native_rate))
                        .add(cosine.mul(Interval::scalar_product(conjugate[1], *native_rate))),
                ]
            }
        };
        derivative
            .iter()
            .all(|value| value.is_finite())
            .then_some(derivative)
            .ok_or(PieceEvaluationError::NonFinite)
    }

    pub fn second_derivative(
        &self,
        parameter: Interval,
    ) -> Result<[Interval; 2], PieceEvaluationError> {
        self.derivative_order(parameter, 2)
    }

    pub fn third_derivative(
        &self,
        parameter: Interval,
    ) -> Result<[Interval; 2], PieceEvaluationError> {
        self.derivative_order(parameter, 3)
    }

    pub fn fourth_derivative(
        &self,
        parameter: Interval,
    ) -> Result<[Interval; 2], PieceEvaluationError> {
        self.derivative_order(parameter, 4)
    }

    #[allow(
        clippy::too_many_lines,
        reason = "the closed family match keeps all higher-jet formulas together for audit"
    )]
    fn derivative_order(
        &self,
        parameter: Interval,
        order: u32,
    ) -> Result<[Interval; 2], PieceEvaluationError> {
        debug_assert!((2..=4).contains(&order));
        let derivative = match &self.kind {
            PieceKind::Linear { .. } => [Interval::ZERO; 2],
            PieceKind::Circular {
                radius,
                angle_offset,
                angle_rate,
                ..
            } => {
                let angle = angle_offset.add(parameter.mul(Interval::point(*angle_rate)));
                let sine = angle.sin().map_err(|_| PieceEvaluationError::NonFinite)?;
                let cosine = angle.cos().map_err(|_| PieceEvaluationError::NonFinite)?;
                match order {
                    2 => {
                        let scale = Interval::scalar_product(-*radius, angle_rate.powi(2));
                        [cosine.mul(scale), sine.mul(scale)]
                    }
                    3 => {
                        let scale = Interval::scalar_product(*radius, angle_rate.powi(3));
                        [sine.mul(scale), cosine.mul(scale.neg())]
                    }
                    4 => {
                        let scale = Interval::scalar_product(*radius, angle_rate.powi(4));
                        [cosine.mul(scale), sine.mul(scale)]
                    }
                    _ => unreachable!(),
                }
            }
            PieceKind::Elliptic {
                major,
                minor,
                angle_offset,
                angle_rate,
                ..
            } => {
                let angle =
                    Interval::point(*angle_offset).add(parameter.mul(Interval::point(*angle_rate)));
                let sine = angle.sin().map_err(|_| PieceEvaluationError::NonFinite)?;
                let cosine = angle.cos().map_err(|_| PieceEvaluationError::NonFinite)?;
                let rate = angle_rate.powi(i32::try_from(order).expect("small derivative order"));
                match order {
                    2 => [
                        cosine
                            .mul(Interval::scalar_product(-major[0], rate))
                            .add(sine.mul(Interval::scalar_product(-minor[0], rate))),
                        cosine
                            .mul(Interval::scalar_product(-major[1], rate))
                            .add(sine.mul(Interval::scalar_product(-minor[1], rate))),
                    ],
                    3 => [
                        sine.mul(Interval::scalar_product(major[0], rate))
                            .add(cosine.mul(Interval::scalar_product(-minor[0], rate))),
                        sine.mul(Interval::scalar_product(major[1], rate))
                            .add(cosine.mul(Interval::scalar_product(-minor[1], rate))),
                    ],
                    4 => [
                        cosine
                            .mul(Interval::scalar_product(major[0], rate))
                            .add(sine.mul(Interval::scalar_product(minor[0], rate))),
                        cosine
                            .mul(Interval::scalar_product(major[1], rate))
                            .add(sine.mul(Interval::scalar_product(minor[1], rate))),
                    ],
                    _ => unreachable!(),
                }
            }
            PieceKind::Polynomial { x, y } => {
                let derivative = |polynomial: &Polynomial| {
                    let mut value = polynomial.clone();
                    for _ in 0..order {
                        value = value.derivative();
                    }
                    value.bezier_bound(parameter)
                };
                [derivative(x), derivative(y)]
            }
            PieceKind::Rational { x, y, weight } => [
                rational_derivative_bound(x, weight, parameter, order)?,
                rational_derivative_bound(y, weight, parameter, order)?,
            ],
            PieceKind::Hyperbolic {
                transverse,
                conjugate,
                native_offset,
                native_rate,
                ..
            } => {
                let native = Interval::point(*native_offset)
                    .add(parameter.mul(Interval::point(*native_rate)));
                let sine = native.sinh().map_err(|_| PieceEvaluationError::NonFinite)?;
                let cosine = native.cosh().map_err(|_| PieceEvaluationError::NonFinite)?;
                let rate = native_rate.powi(i32::try_from(order).expect("small derivative order"));
                let (first, second) = if order.is_multiple_of(2) {
                    (cosine, sine)
                } else {
                    (sine, cosine)
                };
                [
                    first
                        .mul(Interval::scalar_product(transverse[0], rate))
                        .add(second.mul(Interval::scalar_product(conjugate[0], rate))),
                    first
                        .mul(Interval::scalar_product(transverse[1], rate))
                        .add(second.mul(Interval::scalar_product(conjugate[1], rate))),
                ]
            }
        };
        derivative
            .iter()
            .all(|value| value.is_finite())
            .then_some(derivative)
            .ok_or(PieceEvaluationError::NonFinite)
    }

    pub fn point(&self, parameter: f64) -> Result<[f64; 2], PieceEvaluationError> {
        let value = self.position(Interval::point(parameter))?;
        Ok([value.x.midpoint(), value.y.midpoint()])
    }

    pub fn tangent(&self, parameter: f64) -> Result<[f64; 2], PieceEvaluationError> {
        let value = self.derivative(Interval::point(parameter))?;
        Ok([value[0].midpoint(), value[1].midpoint()])
    }

    pub fn denominator_excludes_zero(
        &self,
        parameter: Interval,
    ) -> Result<bool, PieceEvaluationError> {
        match &self.kind {
            PieceKind::Rational { weight, .. } => {
                let value = weight.bezier_bound(parameter);
                if value.is_finite() {
                    Ok(value.excludes_zero())
                } else {
                    Err(PieceEvaluationError::NonFinite)
                }
            }
            _ => Ok(true),
        }
    }

    pub fn exact_area(
        &self,
        parameter: Interval,
        origin: [f64; 2],
    ) -> Result<Option<Interval>, PieceEvaluationError> {
        Ok(match &self.kind {
            PieceKind::Linear { start, delta } => {
                let translated_start = [
                    Interval::point(start[0]).sub(Interval::point(origin[0])),
                    Interval::point(start[1]).sub(Interval::point(origin[1])),
                ];
                let first = [
                    translated_start[0].add(delta[0].mul(Interval::point(parameter.lower))),
                    translated_start[1].add(delta[1].mul(Interval::point(parameter.lower))),
                ];
                let second = [
                    translated_start[0].add(delta[0].mul(Interval::point(parameter.upper))),
                    translated_start[1].add(delta[1].mul(Interval::point(parameter.upper))),
                ];
                Some(Interval::point(0.5).mul(cross_interval(first, second)))
            }
            PieceKind::Circular {
                center,
                radius,
                angle_offset,
                angle_rate,
            } => Some(trigonometric_area(
                *center,
                [*radius, 0.0],
                [0.0, *radius],
                *angle_offset,
                *angle_rate,
                parameter,
                origin,
            )?),
            PieceKind::Elliptic {
                center,
                major,
                minor,
                angle_offset,
                angle_rate,
            } => Some(trigonometric_area(
                *center,
                *major,
                *minor,
                Interval::point(*angle_offset),
                *angle_rate,
                parameter,
                origin,
            )?),
            PieceKind::Polynomial { x, y } => {
                let translated_x = x.sub(&Polynomial::constant(Interval::point(origin[0])));
                let translated_y = y.sub(&Polynomial::constant(Interval::point(origin[1])));
                let integrand = translated_x
                    .mul(&translated_y.derivative())
                    .sub(&translated_y.mul(&translated_x.derivative()))
                    .scale(Interval::point(0.5))
                    .integral();
                Some(
                    integrand
                        .evaluate_point(parameter.upper)
                        .sub(integrand.evaluate_point(parameter.lower)),
                )
            }
            PieceKind::Hyperbolic {
                center,
                transverse,
                conjugate,
                native_offset,
                native_rate,
            } => Some(hyperbolic_area(
                *center,
                *transverse,
                *conjugate,
                *native_offset,
                *native_rate,
                parameter,
                origin,
            )?),
            PieceKind::Rational { .. } => None,
        })
    }

    pub fn area_integrand(
        &self,
        parameter: Interval,
        origin: [f64; 2],
    ) -> Result<Interval, PieceEvaluationError> {
        if let PieceKind::Rational { x, y, weight } = &self.kind {
            let numerator = rational_area_numerator(x, y, weight, origin);
            let denominator = weight.bezier_bound(parameter);
            return numerator
                .bezier_bound(parameter)
                .div(denominator.square())
                .ok_or(PieceEvaluationError::Pole);
        }
        let position = self.position(parameter)?;
        let derivative = self.derivative(parameter)?;
        Ok(cross_interval(
            [
                position.x.sub(Interval::point(origin[0])),
                position.y.sub(Interval::point(origin[1])),
            ],
            derivative,
        )
        .scale(0.5))
    }

    pub fn area_integrand_fourth_derivative(
        &self,
        parameter: Interval,
        origin: [f64; 2],
    ) -> Result<Option<Interval>, PieceEvaluationError> {
        let PieceKind::Rational { x, y, weight } = &self.kind else {
            return Ok(None);
        };
        let mut numerator = rational_area_numerator(x, y, weight, origin);
        let weight_derivative = weight.derivative();
        let mut denominator_power = 2_u32;
        for _ in 0..4 {
            numerator = numerator.derivative().mul(weight).sub(
                &numerator
                    .mul(&weight_derivative)
                    .scale(Interval::point(f64::from(denominator_power))),
            );
            denominator_power += 1;
        }
        let denominator = weight.bezier_bound(parameter);
        if !denominator.excludes_zero() {
            return Err(PieceEvaluationError::Pole);
        }
        Ok(Some(
            numerator
                .bezier_bound(parameter)
                .div(denominator.powi(denominator_power))
                .ok_or(PieceEvaluationError::Pole)?,
        ))
    }

    pub fn is_linear(&self) -> bool {
        matches!(self.kind, PieceKind::Linear { .. })
    }

    pub fn may_self_intersect(&self) -> bool {
        matches!(
            self.family,
            VisualProfileCurveFamily::QuadraticBezier
                | VisualProfileCurveFamily::CubicBezier
                | VisualProfileCurveFamily::ClampedBSpline
                | VisualProfileCurveFamily::PeriodicBSpline
                | VisualProfileCurveFamily::ClampedNurbs
                | VisualProfileCurveFamily::PeriodicNurbs
        )
    }
}

fn rational_derivative_bound(
    numerator: &Polynomial,
    denominator: &Polynomial,
    parameter: Interval,
    order: u32,
) -> Result<Interval, PieceEvaluationError> {
    let denominator_bound = denominator.bezier_bound(parameter);
    if !denominator_bound.excludes_zero() {
        return Err(PieceEvaluationError::Pole);
    }
    let denominator_derivative = denominator.derivative();
    let mut derivative_numerator = numerator.clone();
    let mut denominator_power = 0_u32;
    for derivative_order in 0..order {
        derivative_numerator = derivative_numerator.derivative().mul(denominator).sub(
            &derivative_numerator
                .mul(&denominator_derivative)
                .scale(Interval::point(f64::from(derivative_order + 1))),
        );
        denominator_power += 1;
    }
    derivative_numerator
        .bezier_bound(parameter)
        .div(denominator_bound.powi(denominator_power + 1))
        .ok_or(PieceEvaluationError::Pole)
}

fn rational_area_numerator(
    x: &Polynomial,
    y: &Polynomial,
    weight: &Polynomial,
    origin: [f64; 2],
) -> Polynomial {
    let translated_x = x.sub(&weight.scale(Interval::point(origin[0])));
    let translated_y = y.sub(&weight.scale(Interval::point(origin[1])));
    translated_x
        .mul(&translated_y.derivative())
        .sub(&translated_y.mul(&translated_x.derivative()))
        .scale(Interval::point(0.5))
}

#[allow(clippy::too_many_lines)]
pub(super) fn piece_for_span(
    document: &SketchDocument,
    span: CurveSpan,
) -> Result<CurvePiece, PieceEvaluationError> {
    let definition = &document
        .curve(span.curve)
        .ok_or(PieceEvaluationError::NonFinite)?
        .definition;
    let point = |id| {
        document
            .point(id)
            .map(|value| value.position)
            .ok_or(PieceEvaluationError::NonFinite)
    };
    let scalar = |id| {
        document
            .scalar(id)
            .map(|value| value.value)
            .filter(|value| value.is_finite())
            .ok_or(PieceEvaluationError::NonFinite)
    };
    let (family, kind) = match definition {
        CurveDefinition::Line { start, end, .. } => {
            let start = point(*start)?;
            let end = point(*end)?;
            (
                VisualProfileCurveFamily::Line,
                PieceKind::Linear {
                    start,
                    delta: [
                        Interval::point(end[0]).sub(Interval::point(start[0])),
                        Interval::point(end[1]).sub(Interval::point(start[1])),
                    ],
                },
            )
        }
        CurveDefinition::Polyline { points, closed, .. } => {
            let index = span.segment as usize;
            let start = point(points[index])?;
            let next = if index + 1 == points.len() {
                if *closed {
                    0
                } else {
                    return Err(PieceEvaluationError::NonFinite);
                }
            } else {
                index + 1
            };
            let end = point(points[next])?;
            (
                VisualProfileCurveFamily::Polyline,
                PieceKind::Linear {
                    start,
                    delta: [
                        Interval::point(end[0]).sub(Interval::point(start[0])),
                        Interval::point(end[1]).sub(Interval::point(start[1])),
                    ],
                },
            )
        }
        CurveDefinition::Circle { center, radius } => (
            VisualProfileCurveFamily::Circle,
            PieceKind::Circular {
                center: point(*center)?,
                radius: scalar(*radius)?,
                angle_offset: Interval::ZERO,
                angle_rate: 1.0,
            },
        ),
        CurveDefinition::CircularArc {
            center,
            radius,
            start_angle,
            end_angle,
            sweep,
        } => {
            let start = scalar(*start_angle)?;
            let end = scalar(*end_angle)?;
            let signed_sweep = document_arc_signed_sweep(start, end, *sweep)
                .map_err(|_| PieceEvaluationError::NonFinite)?;
            let center_value = point(*center)?;
            (
                VisualProfileCurveFamily::CircularArc,
                PieceKind::Circular {
                    center: center_value,
                    radius: scalar(*radius)?,
                    angle_offset: Interval::point(start),
                    angle_rate: signed_sweep,
                },
            )
        }
        CurveDefinition::QuadraticBezier { controls } => {
            let controls = controls
                .iter()
                .map(|id| point(*id))
                .collect::<Result<Vec<_>, _>>()?;
            (
                VisualProfileCurveFamily::QuadraticBezier,
                polynomial_piece(&controls),
            )
        }
        CurveDefinition::CubicBezier { controls } => {
            let controls = controls
                .iter()
                .map(|id| point(*id))
                .collect::<Result<Vec<_>, _>>()?;
            (
                VisualProfileCurveFamily::CubicBezier,
                polynomial_piece(&controls),
            )
        }
        CurveDefinition::BSpline { form, .. } => {
            let geometry = document
                .bspline_geometry(definition)
                .map_err(|_| PieceEvaluationError::NonFinite)?;
            let span_index = SketchDocument::spline_span_index(definition, span.segment)
                .map_err(|_| PieceEvaluationError::NonFinite)?;
            let basis = basis_polynomials(geometry.basis(), span_index)?;
            let (x, y) = polynomial_coordinates(&basis, geometry.controls(), None);
            (
                match form {
                    crate::DocumentBSplineForm::Clamped => VisualProfileCurveFamily::ClampedBSpline,
                    crate::DocumentBSplineForm::Periodic => {
                        VisualProfileCurveFamily::PeriodicBSpline
                    }
                },
                PieceKind::Polynomial { x, y },
            )
        }
        CurveDefinition::Nurbs { form, .. } => {
            let geometry = document
                .nurbs_geometry(definition)
                .map_err(|_| PieceEvaluationError::NonFinite)?;
            let span_index = SketchDocument::spline_span_index(definition, span.segment)
                .map_err(|_| PieceEvaluationError::NonFinite)?;
            let basis = basis_polynomials(geometry.basis(), span_index)?;
            let uniform_weights = geometry
                .weights()
                .iter()
                .all(|weight| weight.to_bits() == geometry.weights()[0].to_bits());
            let (x, y) = polynomial_coordinates(
                &basis,
                geometry.controls(),
                (!uniform_weights).then_some(geometry.weights()),
            );
            (
                match form {
                    crate::DocumentBSplineForm::Clamped => VisualProfileCurveFamily::ClampedNurbs,
                    crate::DocumentBSplineForm::Periodic => VisualProfileCurveFamily::PeriodicNurbs,
                },
                if uniform_weights {
                    PieceKind::Polynomial { x, y }
                } else {
                    PieceKind::Rational {
                        x,
                        y,
                        weight: polynomial_weight(&basis, geometry.weights()),
                    }
                },
            )
        }
        CurveDefinition::Ellipse { .. }
        | CurveDefinition::EllipticalArc { .. }
        | CurveDefinition::RationalQuadraticConic { .. }
        | CurveDefinition::ParabolaSegment { .. }
        | CurveDefinition::HyperbolaSegment { .. } => conic_piece(document, definition)?,
    };
    Ok(CurvePiece { family, kind })
}

#[allow(clippy::too_many_lines)]
fn conic_piece(
    document: &SketchDocument,
    definition: &CurveDefinition,
) -> Result<(VisualProfileCurveFamily, PieceKind), PieceEvaluationError> {
    let geometry = document
        .conic_geometry(definition)
        .map_err(|_| PieceEvaluationError::NonFinite)?;
    Ok(match geometry {
        ConicGeometry::Ellipse(value) => {
            let center = point_array(value.center());
            let major = vector_array(value.major_axis().vector() * value.semi_major());
            let minor = vector_array(value.minor_axis().vector() * value.semi_minor());
            if value.semi_major().to_bits() == value.semi_minor().to_bits() {
                return Ok((
                    VisualProfileCurveFamily::Ellipse,
                    PieceKind::Circular {
                        center,
                        radius: value.semi_major(),
                        angle_offset: atan2_point(major[1], major[0])
                            .map_err(|_| PieceEvaluationError::NonFinite)?,
                        angle_rate: 1.0,
                    },
                ));
            }
            (
                VisualProfileCurveFamily::Ellipse,
                PieceKind::Elliptic {
                    center,
                    major,
                    minor,
                    angle_offset: 0.0,
                    angle_rate: 1.0,
                },
            )
        }
        ConicGeometry::EllipticalArc(value) => {
            let ellipse = value.ellipse();
            let major = vector_array(ellipse.major_axis().vector() * ellipse.semi_major());
            if ellipse.semi_major().to_bits() == ellipse.semi_minor().to_bits() {
                return Ok((
                    VisualProfileCurveFamily::EllipticalArc,
                    PieceKind::Circular {
                        center: point_array(ellipse.center()),
                        radius: ellipse.semi_major(),
                        angle_offset: atan2_point(major[1], major[0])
                            .map_err(|_| PieceEvaluationError::NonFinite)?
                            .add(Interval::point(value.start_angle())),
                        angle_rate: value.signed_sweep(),
                    },
                ));
            }
            (
                VisualProfileCurveFamily::EllipticalArc,
                PieceKind::Elliptic {
                    center: point_array(ellipse.center()),
                    major,
                    minor: vector_array(ellipse.minor_axis().vector() * ellipse.semi_minor()),
                    angle_offset: value.start_angle(),
                    angle_rate: value.signed_sweep(),
                },
            )
        }
        ConicGeometry::RationalQuadratic(value) => {
            let controls = value.homogeneous_controls();
            if value.middle_weight().to_bits() == 1.0_f64.to_bits() {
                let ordinary = controls
                    .iter()
                    .map(|(point, _)| [point.x, point.y])
                    .collect::<Vec<_>>();
                return Ok((
                    VisualProfileCurveFamily::RationalQuadraticConic,
                    polynomial_piece(&ordinary),
                ));
            }
            let x = polynomial_from_bernstein(
                &controls
                    .iter()
                    .map(|(point, _)| Interval::point(point.x))
                    .collect::<Vec<_>>(),
            );
            let y = polynomial_from_bernstein(
                &controls
                    .iter()
                    .map(|(point, _)| Interval::point(point.y))
                    .collect::<Vec<_>>(),
            );
            let weight = polynomial_from_bernstein(
                &controls
                    .iter()
                    .map(|(_, weight)| Interval::point(*weight))
                    .collect::<Vec<_>>(),
            );
            (
                VisualProfileCurveFamily::RationalQuadraticConic,
                PieceKind::Rational { x, y, weight },
            )
        }
        ConicGeometry::ParabolaSegment(value) => {
            let start = value.trim().start();
            let rate = value.trim().signed_rate();
            let axis = value.opening_axis().vector();
            let normal = value.opening_axis().left_normal().vector();
            let focal = value.focal_length();
            let native = Polynomial::linear(Interval::point(start), Interval::point(rate));
            let squared = native.mul(&native);
            let x = Polynomial::constant(Interval::point(value.vertex().x))
                .add(&squared.scale(Interval::scalar_product(axis.x, focal)))
                .add(
                    &native
                        .scale(Interval::scalar_product(normal.x, 2.0).mul(Interval::point(focal))),
                );
            let y = Polynomial::constant(Interval::point(value.vertex().y))
                .add(&squared.scale(Interval::scalar_product(axis.y, focal)))
                .add(
                    &native
                        .scale(Interval::scalar_product(normal.y, 2.0).mul(Interval::point(focal))),
                );
            (
                VisualProfileCurveFamily::Parabola,
                PieceKind::Polynomial { x, y },
            )
        }
        ConicGeometry::HyperbolaSegment(value) => {
            let sign = value.branch().multiplier();
            (
                VisualProfileCurveFamily::Hyperbola,
                PieceKind::Hyperbolic {
                    center: point_array(value.center()),
                    transverse: vector_array(
                        value.transverse_axis().vector() * (sign * value.semi_transverse()),
                    ),
                    conjugate: vector_array(
                        value.conjugate_axis().vector() * value.semi_conjugate(),
                    ),
                    native_offset: value.trim().start(),
                    native_rate: value.trim().signed_rate(),
                },
            )
        }
    })
}

fn polynomial_piece(controls: &[[f64; 2]]) -> PieceKind {
    PieceKind::Polynomial {
        x: polynomial_from_bernstein(
            &controls
                .iter()
                .map(|point| Interval::point(point[0]))
                .collect::<Vec<_>>(),
        ),
        y: polynomial_from_bernstein(
            &controls
                .iter()
                .map(|point| Interval::point(point[1]))
                .collect::<Vec<_>>(),
        ),
    }
}

fn basis_polynomials(
    basis: &BSplineBasis,
    span_index: BSplineSpanIndex,
) -> Result<Vec<(usize, Polynomial)>, PieceEvaluationError> {
    let degree = usize::try_from(basis.degree()).map_err(|_| PieceEvaluationError::NonFinite)?;
    let span = basis
        .span(span_index)
        .ok_or(PieceEvaluationError::NonFinite)?;
    let raw_span = raw_span(basis, span_index)?;
    let base = raw_span - isize::try_from(degree).map_err(|_| PieceEvaluationError::NonFinite)?;
    let width = Interval::point(span.upper()).sub(Interval::point(span.lower()));
    let native = Polynomial::linear(Interval::point(span.lower()), width);
    let mut levels = vec![vec![Polynomial::zero(); degree + 2]; degree + 1];
    levels[0][degree] = Polynomial::constant(Interval::ONE);
    for current_degree in 1..=degree {
        for slot in (degree - current_degree)..=degree {
            let index =
                base + isize::try_from(slot).map_err(|_| PieceEvaluationError::NonFinite)?;
            let degree_index =
                isize::try_from(current_degree).map_err(|_| PieceEvaluationError::NonFinite)?;
            let first = if basis_knots_equal(basis, index + degree_index, index)? {
                Polynomial::zero()
            } else {
                let reciprocal = basis_knot_bound(basis, index + degree_index)?
                    .sub(basis_knot_bound(basis, index)?)
                    .reciprocal()
                    .ok_or(PieceEvaluationError::NonFinite)?;
                native
                    .sub(&Polynomial::constant(basis_knot_bound(basis, index)?))
                    .scale(reciprocal)
                    .mul(&levels[current_degree - 1][slot])
            };
            let second = if basis_knots_equal(basis, index + degree_index + 1, index + 1)? {
                Polynomial::zero()
            } else {
                let reciprocal = basis_knot_bound(basis, index + degree_index + 1)?
                    .sub(basis_knot_bound(basis, index + 1)?)
                    .reciprocal()
                    .ok_or(PieceEvaluationError::NonFinite)?;
                Polynomial::constant(basis_knot_bound(basis, index + degree_index + 1)?)
                    .sub(&native)
                    .scale(reciprocal)
                    .mul(&levels[current_degree - 1][slot + 1])
            };
            levels[current_degree][slot] = first.add(&second);
        }
    }
    Ok(span
        .support()
        .iter()
        .copied()
        .enumerate()
        .map(|(slot, control)| (control, levels[degree][slot].clone()))
        .collect())
}

fn raw_span(
    basis: &BSplineBasis,
    selected: BSplineSpanIndex,
) -> Result<isize, PieceEvaluationError> {
    let ordinal =
        usize::try_from(selected.ordinal()).map_err(|_| PieceEvaluationError::NonFinite)?;
    let degree = usize::try_from(basis.degree()).map_err(|_| PieceEvaluationError::NonFinite)?;
    let mut positive = 0_usize;
    match basis.form() {
        BSplineForm::Clamped => {
            for raw in degree..basis.control_count() {
                if basis.knots()[raw] < basis.knots()[raw + 1] {
                    if positive == ordinal {
                        return isize::try_from(raw).map_err(|_| PieceEvaluationError::NonFinite);
                    }
                    positive += 1;
                }
            }
        }
        BSplineForm::Periodic => {
            for knot_span in 0..basis.control_count() {
                if basis.knots()[knot_span] < basis.knots()[knot_span + 1] {
                    if positive == ordinal {
                        return isize::try_from(degree + knot_span)
                            .map_err(|_| PieceEvaluationError::NonFinite);
                    }
                    positive += 1;
                }
            }
        }
    }
    Err(PieceEvaluationError::NonFinite)
}

fn basis_knot_bound(basis: &BSplineBasis, index: isize) -> Result<Interval, PieceEvaluationError> {
    Ok(match basis.form() {
        BSplineForm::Clamped => {
            let index = usize::try_from(index).map_err(|_| PieceEvaluationError::NonFinite)?;
            Interval::point(basis.knots()[index])
        }
        BSplineForm::Periodic => {
            let count = isize::try_from(basis.control_count())
                .map_err(|_| PieceEvaluationError::NonFinite)?;
            let degree =
                isize::try_from(basis.degree()).map_err(|_| PieceEvaluationError::NonFinite)?;
            let shifted = index - degree;
            let period_offset = shifted.div_euclid(count);
            let knot_index = usize::try_from(shifted.rem_euclid(count))
                .map_err(|_| PieceEvaluationError::NonFinite)?;
            let period = *basis
                .knots()
                .last()
                .ok_or(PieceEvaluationError::NonFinite)?;
            let period_offset =
                i32::try_from(period_offset).map_err(|_| PieceEvaluationError::NonFinite)?;
            Interval::point(basis.knots()[knot_index])
                .add(Interval::scalar_product(f64::from(period_offset), period))
        }
    })
}

fn basis_knots_equal(
    basis: &BSplineBasis,
    first: isize,
    second: isize,
) -> Result<bool, PieceEvaluationError> {
    Ok(match basis.form() {
        BSplineForm::Clamped => {
            let first = usize::try_from(first).map_err(|_| PieceEvaluationError::NonFinite)?;
            let second = usize::try_from(second).map_err(|_| PieceEvaluationError::NonFinite)?;
            basis.knots()[first].to_bits() == basis.knots()[second].to_bits()
        }
        BSplineForm::Periodic => {
            let count = isize::try_from(basis.control_count())
                .map_err(|_| PieceEvaluationError::NonFinite)?;
            let degree =
                isize::try_from(basis.degree()).map_err(|_| PieceEvaluationError::NonFinite)?;
            let symbolic = |index: isize| -> Result<(usize, isize), PieceEvaluationError> {
                let shifted = index - degree;
                Ok((
                    usize::try_from(shifted.rem_euclid(count))
                        .map_err(|_| PieceEvaluationError::NonFinite)?,
                    shifted.div_euclid(count),
                ))
            };
            let (first_index, first_period) = symbolic(first)?;
            let (second_index, second_period) = symbolic(second)?;
            first_period == second_period
                && basis.knots()[first_index].to_bits() == basis.knots()[second_index].to_bits()
        }
    })
}

fn polynomial_coordinates(
    basis: &[(usize, Polynomial)],
    controls: &[Point2<f64>],
    weights: Option<&[f64]>,
) -> (Polynomial, Polynomial) {
    basis.iter().fold(
        (Polynomial::zero(), Polynomial::zero()),
        |(x, y), (index, polynomial)| {
            let weight = weights.map_or(1.0, |values| values[*index]);
            (
                x.add(&polynomial.scale(Interval::scalar_product(controls[*index].x, weight))),
                y.add(&polynomial.scale(Interval::scalar_product(controls[*index].y, weight))),
            )
        },
    )
}

fn polynomial_weight(basis: &[(usize, Polynomial)], weights: &[f64]) -> Polynomial {
    basis
        .iter()
        .fold(Polynomial::zero(), |value, (index, polynomial)| {
            value.add(&polynomial.scale(Interval::point(weights[*index])))
        })
}

fn trigonometric_area(
    center: [f64; 2],
    major: [f64; 2],
    minor: [f64; 2],
    angle_offset: Interval,
    angle_rate: f64,
    parameter: Interval,
    origin: [f64; 2],
) -> Result<Interval, PieceEvaluationError> {
    let translated = [
        Interval::point(center[0]).sub(Interval::point(origin[0])),
        Interval::point(center[1]).sub(Interval::point(origin[1])),
    ];
    let major = major.map(Interval::point);
    let minor = minor.map(Interval::point);
    let center_major = cross_interval(translated, major);
    let center_minor = cross_interval(translated, minor);
    let axes = cross_interval(major, minor);
    let primitive = |parameter: f64| -> Result<Interval, PieceEvaluationError> {
        let angle = angle_offset.add(Interval::scalar_product(angle_rate, parameter));
        Ok(center_major
            .mul(angle.cos().map_err(|_| PieceEvaluationError::NonFinite)?)
            .add(center_minor.mul(angle.sin().map_err(|_| PieceEvaluationError::NonFinite)?))
            .add(axes.mul(angle))
            .scale(0.5))
    };
    Ok(primitive(parameter.upper)?.sub(primitive(parameter.lower)?))
}

fn hyperbolic_area(
    center: [f64; 2],
    transverse: [f64; 2],
    conjugate: [f64; 2],
    native_offset: f64,
    native_rate: f64,
    parameter: Interval,
    origin: [f64; 2],
) -> Result<Interval, PieceEvaluationError> {
    let translated = [
        Interval::point(center[0]).sub(Interval::point(origin[0])),
        Interval::point(center[1]).sub(Interval::point(origin[1])),
    ];
    let transverse = transverse.map(Interval::point);
    let conjugate = conjugate.map(Interval::point);
    let center_transverse = cross_interval(translated, transverse);
    let center_conjugate = cross_interval(translated, conjugate);
    let axes = cross_interval(transverse, conjugate);
    let primitive = |parameter: f64| -> Result<Interval, PieceEvaluationError> {
        let native =
            Interval::point(native_offset).add(Interval::scalar_product(native_rate, parameter));
        Ok(center_transverse
            .mul(native.cosh().map_err(|_| PieceEvaluationError::NonFinite)?)
            .add(center_conjugate.mul(native.sinh().map_err(|_| PieceEvaluationError::NonFinite)?))
            .add(axes.mul(native))
            .scale(0.5))
    };
    Ok(primitive(parameter.upper)?.sub(primitive(parameter.lower)?))
}

fn point_array(value: Point2<f64>) -> [f64; 2] {
    [value.x, value.y]
}

fn vector_array(value: Vector2<f64>) -> [f64; 2] {
    [value.x, value.y]
}
