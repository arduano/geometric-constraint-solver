use geosolve_core::{EvaluationError, LocalJacobian, ResidualEvaluator, VariableValue};
use num_dual::{DualDVec64, DualNum};

use crate::curves::{
    AngleOrientation, CONTACT_PARAMETER_ROUNDOFF_TOLERANCE, CircleContainment, CircleTangencyMode,
    CurveDegeneracy, CurveRef, LineOffsetOrientation, LineParameterDomain, LineSide,
    tangency_distance, unwrap_near,
};
use crate::{
    CurveContinuity, CurveCurvatureRelation, CurveDirectionRelation, CurveNormalSide,
    CurveTangentOrientation, FilletEndpointOrder, SegmentEndpoint,
};

#[derive(Clone, Debug)]
enum SketchAdValue {
    Scalar(DualDVec64),
    Point([DualDVec64; 2]),
}

struct AdCurveJet2 {
    position: [DualDVec64; 2],
    first: [DualDVec64; 2],
    second: [DualDVec64; 2],
}

trait SketchAdFormula {
    fn evaluate_dual(
        &self,
        variables: &[SketchAdValue],
    ) -> Result<Vec<DualDVec64>, EvaluationError>;
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum BezierIncidence {
    Quadratic([usize; 3]),
    Cubic([usize; 4]),
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum CurveParameterIncidence {
    Variable(usize),
    Fixed(f64),
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum NurbsWeightIncidence {
    Variable(usize),
    Fixed(f64),
}

#[derive(Clone, Debug)]
pub(crate) enum GenericCurveIncidence {
    Line {
        points: [usize; 2],
        parameter: CurveParameterIncidence,
        bounded: bool,
    },
    Circle {
        center: usize,
        radius: usize,
        parameter: CurveParameterIncidence,
    },
    Arc {
        center: usize,
        radius: usize,
        start_angle: CurveParameterIncidence,
        end_angle: CurveParameterIncidence,
        turn_offset: i32,
        sweep: crate::ArcSweep,
        parameter: CurveParameterIncidence,
    },
    QuadraticBezier {
        controls: [usize; 3],
        parameter: CurveParameterIncidence,
    },
    CubicBezier {
        controls: [usize; 4],
        parameter: CurveParameterIncidence,
    },
    Ellipse {
        center: usize,
        major_axis_point: usize,
        minor_axis_ratio: usize,
        parameter: CurveParameterIncidence,
    },
    EllipticalArc {
        center: usize,
        major_axis_point: usize,
        minor_axis_ratio: usize,
        start_angle: f64,
        signed_sweep: f64,
        parameter: CurveParameterIncidence,
    },
    RationalQuadratic {
        start: usize,
        weighted_middle: usize,
        middle_weight: usize,
        end: usize,
        parameter: CurveParameterIncidence,
    },
    ParabolaSegment {
        vertex: usize,
        focus: usize,
        trim: geosolve_geometry::DirectedParameterTrim,
        parameter: CurveParameterIncidence,
    },
    HyperbolaSegment {
        center: usize,
        transverse_axis_point: usize,
        semi_conjugate: usize,
        branch: geosolve_geometry::HyperbolaBranch,
        trim: geosolve_geometry::DirectedParameterTrim,
        parameter: CurveParameterIncidence,
    },
    BSpline {
        basis: geosolve_geometry::BSplineBasis,
        span: geosolve_geometry::BSplineSpanIndex,
        controls: Vec<usize>,
        parameter: CurveParameterIncidence,
    },
    Nurbs {
        basis: geosolve_geometry::BSplineBasis,
        span: geosolve_geometry::BSplineSpanIndex,
        controls: Vec<usize>,
        weights: Vec<NurbsWeightIncidence>,
        parameter: CurveParameterIncidence,
    },
}

#[derive(Clone, Debug)]
pub(crate) struct GenericPointOnCurveResidual {
    pub(crate) point: usize,
    pub(crate) curve: GenericCurveIncidence,
}

#[derive(Clone, Debug)]
pub(crate) struct GenericCurvePairResidual {
    pub(crate) first: GenericCurveIncidence,
    pub(crate) second: GenericCurveIncidence,
    pub(crate) orientation: Option<CurveTangentOrientation>,
}

#[derive(Clone, Debug)]
pub(crate) struct GenericCurveDirectionResidual {
    pub(crate) line: [usize; 2],
    pub(crate) curve: GenericCurveIncidence,
    pub(crate) relation: CurveDirectionRelation,
}

#[derive(Clone, Debug)]
pub(crate) struct GenericEqualCurvatureResidual {
    pub(crate) first: GenericCurveIncidence,
    pub(crate) second: GenericCurveIncidence,
    pub(crate) relation: CurveCurvatureRelation,
    pub(crate) model_scale: f64,
}

#[derive(Clone, Debug)]
pub(crate) struct GenericEndpointContinuityResidual {
    pub(crate) first: GenericCurveIncidence,
    pub(crate) second: GenericCurveIncidence,
    pub(crate) first_sign: f64,
    pub(crate) second_sign: f64,
    pub(crate) kind: CurveContinuity,
    pub(crate) model_scale: f64,
}

#[derive(Clone, Debug)]
pub(crate) struct GenericCurveFilletResidual {
    pub(crate) center: usize,
    pub(crate) radius: usize,
    pub(crate) start_angle: usize,
    pub(crate) end_angle: usize,
    pub(crate) first: GenericCurveIncidence,
    pub(crate) first_side: CurveNormalSide,
    pub(crate) second: GenericCurveIncidence,
    pub(crate) second_side: CurveNormalSide,
    pub(crate) endpoint_order: FilletEndpointOrder,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct CircularSweepResidual {
    pub(crate) start_angle: usize,
    pub(crate) end_angle: usize,
    pub(crate) turn_offset: i32,
    pub(crate) target: f64,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct CircularArcLengthResidual {
    pub(crate) radius: usize,
    pub(crate) start_angle: usize,
    pub(crate) end_angle: usize,
    pub(crate) turn_offset: i32,
    pub(crate) target: f64,
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum ConicPropertyResidualKind {
    Ellipse {
        center: usize,
        axis: usize,
        ratio: usize,
        property: crate::model::M38ConicProperty,
    },
    ParabolaFocalDistance {
        vertex: usize,
        focus: usize,
    },
    Hyperbola {
        center: usize,
        axis: usize,
        semi_conjugate: usize,
        property: crate::model::M38ConicProperty,
    },
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ConicPropertyResidual {
    pub(crate) kind: ConicPropertyResidualKind,
    pub(crate) target: f64,
}

#[derive(Clone, Debug)]
pub(crate) struct GenericPathLengthResidual {
    pub(crate) first: GenericCurveIncidence,
    pub(crate) first_interval: [f64; 2],
    pub(crate) second: Option<(GenericCurveIncidence, [f64; 2])>,
    pub(crate) target: f64,
    pub(crate) tolerance: f64,
    pub(crate) max_evaluations: usize,
}

#[derive(Clone, Debug)]
pub(crate) enum M38DimensionResidual {
    CircularSweep(CircularSweepResidual),
    CircularArcLength(CircularArcLengthResidual),
    ConicProperty(ConicPropertyResidual),
    PathLength(Box<GenericPathLengthResidual>),
}

impl ResidualEvaluator for M38DimensionResidual {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        match self {
            Self::CircularSweep(value) => value.evaluate(variables),
            Self::CircularArcLength(value) => value.evaluate(variables),
            Self::ConicProperty(value) => value.evaluate(variables),
            Self::PathLength(value) => value.evaluate(variables),
        }
    }

    fn jacobian(&self, variables: &[VariableValue]) -> Result<Vec<LocalJacobian>, EvaluationError> {
        match self {
            Self::CircularSweep(value) => value.jacobian(variables),
            Self::CircularArcLength(value) => value.jacobian(variables),
            Self::ConicProperty(value) => value.jacobian(variables),
            Self::PathLength(value) => value.jacobian(variables),
        }
    }
}

macro_rules! impl_sketch_ad_residual {
    ($type:ty) => {
        impl ResidualEvaluator for $type {
            fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
                evaluate_sketch_ad(self, variables, false).map(|(values, _)| values)
            }

            fn jacobian(
                &self,
                variables: &[VariableValue],
            ) -> Result<Vec<LocalJacobian>, EvaluationError> {
                evaluate_sketch_ad(self, variables, true).map(|(_, jacobians)| jacobians)
            }
        }
    };
}

impl_sketch_ad_residual!(CircularSweepResidual);
impl_sketch_ad_residual!(CircularArcLengthResidual);
impl_sketch_ad_residual!(ConicPropertyResidual);
impl_sketch_ad_residual!(GenericPathLengthResidual);

impl SketchAdFormula for CircularSweepResidual {
    fn evaluate_dual(
        &self,
        variables: &[SketchAdValue],
    ) -> Result<Vec<DualDVec64>, EvaluationError> {
        let sweep = ad_scalar(variables, self.end_angle, "circular sweep")?
            - ad_scalar(variables, self.start_angle, "circular sweep")?
            + f64::from(self.turn_offset) * std::f64::consts::TAU;
        if !sweep.re.is_finite() || sweep.re == 0.0 || sweep.re.abs() >= std::f64::consts::TAU {
            return Err(EvaluationError::invalid_geometry(
                "circular sweep must retain a finite nonzero branch",
            ));
        }
        Ok(vec![sweep - self.target])
    }
}

impl SketchAdFormula for CircularArcLengthResidual {
    fn evaluate_dual(
        &self,
        variables: &[SketchAdValue],
    ) -> Result<Vec<DualDVec64>, EvaluationError> {
        let radius = ad_scalar(variables, self.radius, "circular arc length")?;
        let sweep = ad_scalar(variables, self.end_angle, "circular arc length")?
            - ad_scalar(variables, self.start_angle, "circular arc length")?
            + f64::from(self.turn_offset) * std::f64::consts::TAU;
        if radius.re <= 0.0
            || !radius.re.is_finite()
            || !sweep.re.is_finite()
            || sweep.re == 0.0
            || sweep.re.abs() >= std::f64::consts::TAU
        {
            return Err(EvaluationError::invalid_geometry(
                "circular arc length requires a positive radius and retained sweep",
            ));
        }
        let sign = sweep.re.signum();
        Ok(vec![radius * sweep * sign - self.target])
    }
}

impl SketchAdFormula for ConicPropertyResidual {
    fn evaluate_dual(
        &self,
        variables: &[SketchAdValue],
    ) -> Result<Vec<DualDVec64>, EvaluationError> {
        let point_distance = |first: usize, second: usize| -> Result<DualDVec64, EvaluationError> {
            let first = ad_point(variables, first, "conic property")?;
            let second = ad_point(variables, second, "conic property")?;
            Ok(((&second[0] - &first[0]).powi(2) + (&second[1] - &first[1]).powi(2)).sqrt())
        };
        let value = match self.kind {
            ConicPropertyResidualKind::Ellipse {
                center,
                axis,
                ratio,
                property,
            } => {
                let a = point_distance(center, axis)?;
                let ratio = ad_scalar(variables, ratio, "ellipse property")?;
                if a.re <= 0.0 || ratio.re <= 0.0 || ratio.re > 1.0 {
                    return Err(EvaluationError::invalid_geometry(
                        "ellipse property requires observable positive axes",
                    ));
                }
                match property {
                    crate::model::M38ConicProperty::MajorAxisLength => a * 2.0,
                    crate::model::M38ConicProperty::MinorAxisLength => a * ratio * 2.0,
                    crate::model::M38ConicProperty::LinearEccentricity => {
                        a * (DualDVec64::from_re(1.0) - ratio.powi(2)).sqrt()
                    }
                    _ => {
                        return Err(EvaluationError::invalid_geometry(
                            "unsupported ellipse property residual",
                        ));
                    }
                }
            }
            ConicPropertyResidualKind::ParabolaFocalDistance { vertex, focus } => {
                point_distance(vertex, focus)?
            }
            ConicPropertyResidualKind::Hyperbola {
                center,
                axis,
                semi_conjugate,
                property,
            } => {
                let a = point_distance(center, axis)?;
                let b = ad_scalar(variables, semi_conjugate, "hyperbola property")?;
                if a.re <= 0.0 || b.re <= 0.0 {
                    return Err(EvaluationError::invalid_geometry(
                        "hyperbola property requires positive semiaxes",
                    ));
                }
                match property {
                    crate::model::M38ConicProperty::FocalDistance => (a.powi(2) + b.powi(2)).sqrt(),
                    crate::model::M38ConicProperty::TransverseAxisLength => a * 2.0,
                    crate::model::M38ConicProperty::ConjugateAxisLength => b.clone() * 2.0,
                    _ => {
                        return Err(EvaluationError::invalid_geometry(
                            "unsupported hyperbola property residual",
                        ));
                    }
                }
            }
        };
        Ok(vec![value - self.target])
    }
}

impl SketchAdFormula for GenericPathLengthResidual {
    fn evaluate_dual(
        &self,
        variables: &[SketchAdValue],
    ) -> Result<Vec<DualDVec64>, EvaluationError> {
        let mut evaluations = 0;
        let first = integrate_ad_curve_length(
            variables,
            &self.first,
            self.first_interval,
            self.tolerance,
            self.max_evaluations,
            &mut evaluations,
        )?;
        let value = if let Some((second, interval)) = &self.second {
            first
                - integrate_ad_curve_length(
                    variables,
                    second,
                    *interval,
                    self.tolerance,
                    self.max_evaluations,
                    &mut evaluations,
                )?
        } else {
            first
        };
        Ok(vec![value - self.target])
    }
}

fn integrate_ad_curve_length(
    variables: &[SketchAdValue],
    curve: &GenericCurveIncidence,
    [start, end]: [f64; 2],
    tolerance: f64,
    max_evaluations: usize,
    evaluations: &mut usize,
) -> Result<DualDVec64, EvaluationError> {
    let speed = |parameter: f64, evaluations: &mut usize| {
        *evaluations = evaluations.saturating_add(1);
        if *evaluations > max_evaluations {
            return Err(EvaluationError::out_of_domain(
                "path-length derivative work bound exhausted",
            ));
        }
        let jet = evaluate_ad_curve(variables, &curve_at_parameter(curve, parameter))?;
        let speed = (jet.first[0].clone().powi(2) + jet.first[1].clone().powi(2)).sqrt();
        if !speed.re.is_finite() || speed.re <= 0.0 {
            return Err(EvaluationError::degenerate(
                "path-length integrand must be finite and regular",
            ));
        }
        Ok(speed)
    };
    let middle = (start + end) * 0.5;
    let first = speed(start, evaluations)?;
    let center = speed(middle, evaluations)?;
    let last = speed(end, evaluations)?;
    let whole = simpson_dual(start, end, &first, &center, &last);
    adaptive_simpson_dual(
        &speed,
        start,
        end,
        first,
        center,
        last,
        whole,
        tolerance,
        0,
        evaluations,
    )
}

#[allow(clippy::too_many_arguments)]
fn adaptive_simpson_dual<F>(
    speed: &F,
    start: f64,
    end: f64,
    first: DualDVec64,
    center: DualDVec64,
    last: DualDVec64,
    whole: DualDVec64,
    tolerance: f64,
    depth: u32,
    evaluations: &mut usize,
) -> Result<DualDVec64, EvaluationError>
where
    F: Fn(f64, &mut usize) -> Result<DualDVec64, EvaluationError>,
{
    if depth >= 24 {
        return Err(EvaluationError::out_of_domain(
            "path-length integration depth exhausted",
        ));
    }
    let middle = (start + end) * 0.5;
    let left_middle = (start + middle) * 0.5;
    let right_middle = (middle + end) * 0.5;
    let left_value = speed(left_middle, evaluations)?;
    let right_value = speed(right_middle, evaluations)?;
    let left = simpson_dual(start, middle, &first, &left_value, &center);
    let right = simpson_dual(middle, end, &center, &right_value, &last);
    let refined = &left + &right;
    let error = (refined.re - whole.re).abs() / 15.0;
    if error <= tolerance {
        return Ok(&refined + (&refined - whole) / 15.0);
    }
    Ok(adaptive_simpson_dual(
        speed,
        start,
        middle,
        first,
        left_value,
        center.clone(),
        left,
        tolerance * 0.5,
        depth + 1,
        evaluations,
    )? + adaptive_simpson_dual(
        speed,
        middle,
        end,
        center,
        right_value,
        last,
        right,
        tolerance * 0.5,
        depth + 1,
        evaluations,
    )?)
}

fn simpson_dual(
    start: f64,
    end: f64,
    first: &DualDVec64,
    center: &DualDVec64,
    last: &DualDVec64,
) -> DualDVec64 {
    (first + center.clone() * 4.0 + last) * ((end - start) / 6.0)
}

#[allow(clippy::too_many_lines)]
fn curve_at_parameter(curve: &GenericCurveIncidence, parameter: f64) -> GenericCurveIncidence {
    let fixed = CurveParameterIncidence::Fixed(parameter);
    match curve {
        GenericCurveIncidence::Line {
            points, bounded, ..
        } => GenericCurveIncidence::Line {
            points: *points,
            parameter: fixed,
            bounded: *bounded,
        },
        GenericCurveIncidence::Circle { center, radius, .. } => GenericCurveIncidence::Circle {
            center: *center,
            radius: *radius,
            parameter: fixed,
        },
        GenericCurveIncidence::Arc {
            center,
            radius,
            start_angle,
            end_angle,
            turn_offset,
            sweep,
            ..
        } => GenericCurveIncidence::Arc {
            center: *center,
            radius: *radius,
            start_angle: *start_angle,
            end_angle: *end_angle,
            turn_offset: *turn_offset,
            sweep: *sweep,
            parameter: fixed,
        },
        GenericCurveIncidence::QuadraticBezier { controls, .. } => {
            GenericCurveIncidence::QuadraticBezier {
                controls: *controls,
                parameter: fixed,
            }
        }
        GenericCurveIncidence::CubicBezier { controls, .. } => GenericCurveIncidence::CubicBezier {
            controls: *controls,
            parameter: fixed,
        },
        GenericCurveIncidence::Ellipse {
            center,
            major_axis_point,
            minor_axis_ratio,
            ..
        } => GenericCurveIncidence::Ellipse {
            center: *center,
            major_axis_point: *major_axis_point,
            minor_axis_ratio: *minor_axis_ratio,
            parameter: fixed,
        },
        GenericCurveIncidence::EllipticalArc {
            center,
            major_axis_point,
            minor_axis_ratio,
            start_angle,
            signed_sweep,
            ..
        } => GenericCurveIncidence::EllipticalArc {
            center: *center,
            major_axis_point: *major_axis_point,
            minor_axis_ratio: *minor_axis_ratio,
            start_angle: *start_angle,
            signed_sweep: *signed_sweep,
            parameter: fixed,
        },
        GenericCurveIncidence::RationalQuadratic {
            start,
            weighted_middle,
            middle_weight,
            end,
            ..
        } => GenericCurveIncidence::RationalQuadratic {
            start: *start,
            weighted_middle: *weighted_middle,
            middle_weight: *middle_weight,
            end: *end,
            parameter: fixed,
        },
        GenericCurveIncidence::ParabolaSegment {
            vertex,
            focus,
            trim,
            ..
        } => GenericCurveIncidence::ParabolaSegment {
            vertex: *vertex,
            focus: *focus,
            trim: *trim,
            parameter: fixed,
        },
        GenericCurveIncidence::HyperbolaSegment {
            center,
            transverse_axis_point,
            semi_conjugate,
            branch,
            trim,
            ..
        } => GenericCurveIncidence::HyperbolaSegment {
            center: *center,
            transverse_axis_point: *transverse_axis_point,
            semi_conjugate: *semi_conjugate,
            branch: *branch,
            trim: *trim,
            parameter: fixed,
        },
        GenericCurveIncidence::BSpline {
            basis,
            span,
            controls,
            ..
        } => GenericCurveIncidence::BSpline {
            basis: basis.clone(),
            span: *span,
            controls: controls.clone(),
            parameter: fixed,
        },
        GenericCurveIncidence::Nurbs {
            basis,
            span,
            controls,
            weights,
            ..
        } => GenericCurveIncidence::Nurbs {
            basis: basis.clone(),
            span: *span,
            controls: controls.clone(),
            weights: weights.clone(),
            parameter: fixed,
        },
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct PointOnBezierResidual {
    pub(crate) point: usize,
    pub(crate) controls: BezierIncidence,
    pub(crate) parameter: usize,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct LineBezierTangencyResidual {
    pub(crate) line: [usize; 2],
    pub(crate) endpoint: SegmentEndpoint,
    pub(crate) controls: BezierIncidence,
    pub(crate) parameter: usize,
    pub(crate) orientation: CurveTangentOrientation,
}

impl ResidualEvaluator for PointOnBezierResidual {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        evaluate_sketch_ad(self, variables, false).map(|(values, _)| values)
    }

    fn jacobian(&self, variables: &[VariableValue]) -> Result<Vec<LocalJacobian>, EvaluationError> {
        evaluate_sketch_ad(self, variables, true).map(|(_, jacobians)| jacobians)
    }
}

impl SketchAdFormula for PointOnBezierResidual {
    fn evaluate_dual(
        &self,
        variables: &[SketchAdValue],
    ) -> Result<Vec<DualDVec64>, EvaluationError> {
        let point = ad_point(variables, self.point, "point-on-Bezier")?;
        let parameter = ad_scalar(variables, self.parameter, "point-on-Bezier")?;
        validate_ad_parameter(parameter, "point-on-Bezier")?;
        let jet = evaluate_ad_bezier(variables, self.controls, parameter)?;
        require_ad_speed(&jet.first, "point-on-Bezier")?;
        Ok(vec![
            &point[0] - &jet.position[0],
            &point[1] - &jet.position[1],
        ])
    }
}

impl ResidualEvaluator for LineBezierTangencyResidual {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        evaluate_sketch_ad(self, variables, false).map(|(values, _)| values)
    }

    fn jacobian(&self, variables: &[VariableValue]) -> Result<Vec<LocalJacobian>, EvaluationError> {
        evaluate_sketch_ad(self, variables, true).map(|(_, jacobians)| jacobians)
    }
}

impl SketchAdFormula for LineBezierTangencyResidual {
    fn evaluate_dual(
        &self,
        variables: &[SketchAdValue],
    ) -> Result<Vec<DualDVec64>, EvaluationError> {
        let start = ad_point(variables, self.line[0], "line-Bezier tangency")?;
        let end = ad_point(variables, self.line[1], "line-Bezier tangency")?;
        let parameter = ad_scalar(variables, self.parameter, "line-Bezier tangency")?;
        validate_ad_parameter(parameter, "line-Bezier tangency")?;
        let line_direction = [&end[0] - &start[0], &end[1] - &start[1]];
        let endpoint = match self.endpoint {
            SegmentEndpoint::Start => start,
            SegmentEndpoint::End => end,
        };
        let jet = evaluate_ad_bezier(variables, self.controls, parameter)?;
        let line_unit = ad_unit(&line_direction, "line-Bezier tangency line")?;
        let curve_unit = ad_unit(&jet.first, "line-Bezier tangency Bezier")?;
        let orientation = (&line_unit[0] * &curve_unit[0] + &line_unit[1] * &curve_unit[1]).re;
        let orientation_valid = match self.orientation {
            CurveTangentOrientation::Aligned => orientation > 0.0,
            CurveTangentOrientation::Opposed => orientation < 0.0,
        };
        if !orientation_valid {
            return Err(EvaluationError::ambiguous(
                "line-Bezier tangency crossed its selected tangent orientation",
            ));
        }
        Ok(vec![
            &endpoint[0] - &jet.position[0],
            &endpoint[1] - &jet.position[1],
            &line_unit[0] * &curve_unit[1] - &line_unit[1] * &curve_unit[0],
        ])
    }
}

impl ResidualEvaluator for GenericPointOnCurveResidual {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        evaluate_sketch_ad(self, variables, false).map(|(values, _)| values)
    }

    fn jacobian(&self, variables: &[VariableValue]) -> Result<Vec<LocalJacobian>, EvaluationError> {
        evaluate_sketch_ad(self, variables, true).map(|(_, jacobians)| jacobians)
    }
}

impl SketchAdFormula for GenericPointOnCurveResidual {
    fn evaluate_dual(
        &self,
        variables: &[SketchAdValue],
    ) -> Result<Vec<DualDVec64>, EvaluationError> {
        let point = ad_point(variables, self.point, "point-on-curve")?;
        let jet = evaluate_ad_curve(variables, &self.curve)?;
        require_ad_speed(&jet.first, "point-on-curve")?;
        Ok(vec![
            &point[0] - &jet.position[0],
            &point[1] - &jet.position[1],
        ])
    }
}

impl ResidualEvaluator for GenericCurvePairResidual {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        evaluate_sketch_ad(self, variables, false).map(|(values, _)| values)
    }

    fn jacobian(&self, variables: &[VariableValue]) -> Result<Vec<LocalJacobian>, EvaluationError> {
        evaluate_sketch_ad(self, variables, true).map(|(_, jacobians)| jacobians)
    }
}

impl SketchAdFormula for GenericCurvePairResidual {
    fn evaluate_dual(
        &self,
        variables: &[SketchAdValue],
    ) -> Result<Vec<DualDVec64>, EvaluationError> {
        let first = evaluate_ad_curve(variables, &self.first)?;
        let second = evaluate_ad_curve(variables, &self.second)?;
        require_ad_speed(&first.first, "first contact curve")?;
        require_ad_speed(&second.first, "second contact curve")?;
        let mut values = vec![
            &first.position[0] - &second.position[0],
            &first.position[1] - &second.position[1],
        ];
        if let Some(orientation) = self.orientation {
            let first_unit = ad_unit(&first.first, "first tangent curve")?;
            let second_unit = ad_unit(&second.first, "second tangent curve")?;
            let cosine = (&first_unit[0] * &second_unit[0] + &first_unit[1] * &second_unit[1]).re;
            let orientation_valid = match orientation {
                CurveTangentOrientation::Aligned => cosine > 0.0,
                CurveTangentOrientation::Opposed => cosine < 0.0,
            };
            if !orientation_valid {
                return Err(EvaluationError::ambiguous(
                    "curve tangency crossed its selected tangent orientation",
                ));
            }
            values.push(&first_unit[0] * &second_unit[1] - &first_unit[1] * &second_unit[0]);
        }
        Ok(values)
    }
}

impl ResidualEvaluator for GenericCurveDirectionResidual {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        evaluate_sketch_ad(self, variables, false).map(|(values, _)| values)
    }

    fn jacobian(&self, variables: &[VariableValue]) -> Result<Vec<LocalJacobian>, EvaluationError> {
        evaluate_sketch_ad(self, variables, true).map(|(_, jacobians)| jacobians)
    }
}

impl SketchAdFormula for GenericCurveDirectionResidual {
    fn evaluate_dual(
        &self,
        variables: &[SketchAdValue],
    ) -> Result<Vec<DualDVec64>, EvaluationError> {
        let start = ad_point(variables, self.line[0], "curve direction line")?;
        let end = ad_point(variables, self.line[1], "curve direction line")?;
        let line = ad_unit(
            &[&end[0] - &start[0], &end[1] - &start[1]],
            "curve direction line",
        )?;
        let curve = evaluate_ad_curve(variables, &self.curve)?;
        let tangent = ad_unit(&curve.first, "curve direction tangent")?;
        match self.relation {
            CurveDirectionRelation::Tangent(orientation) => {
                let cosine = (&line[0] * &tangent[0] + &line[1] * &tangent[1]).re;
                let valid = match orientation {
                    CurveTangentOrientation::Aligned => cosine > 0.0,
                    CurveTangentOrientation::Opposed => cosine < 0.0,
                };
                if !valid {
                    return Err(EvaluationError::ambiguous(
                        "curve tangent crossed its selected line orientation",
                    ));
                }
                Ok(vec![&line[0] * &tangent[1] - &line[1] * &tangent[0]])
            }
            CurveDirectionRelation::Normal(side) => {
                let left_normal = [-tangent[1].clone(), tangent[0].clone()];
                let cosine = (&line[0] * &left_normal[0] + &line[1] * &left_normal[1]).re;
                let valid = match side {
                    CurveNormalSide::Left => cosine > 0.0,
                    CurveNormalSide::Right => cosine < 0.0,
                };
                if !valid {
                    return Err(EvaluationError::ambiguous(
                        "curve normal crossed its selected side",
                    ));
                }
                Ok(vec![&line[0] * &tangent[0] + &line[1] * &tangent[1]])
            }
        }
    }
}

impl ResidualEvaluator for GenericEqualCurvatureResidual {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        evaluate_sketch_ad(self, variables, false).map(|(values, _)| values)
    }

    fn jacobian(&self, variables: &[VariableValue]) -> Result<Vec<LocalJacobian>, EvaluationError> {
        evaluate_sketch_ad(self, variables, true).map(|(_, jacobians)| jacobians)
    }
}

impl SketchAdFormula for GenericEqualCurvatureResidual {
    fn evaluate_dual(
        &self,
        variables: &[SketchAdValue],
    ) -> Result<Vec<DualDVec64>, EvaluationError> {
        let first =
            ad_signed_curvature(&evaluate_ad_curve(variables, &self.first)?, "first curve")?;
        let second =
            ad_signed_curvature(&evaluate_ad_curve(variables, &self.second)?, "second curve")?;
        match self.relation {
            CurveCurvatureRelation::Signed => Ok(vec![(first - second) * self.model_scale]),
            CurveCurvatureRelation::MagnitudeSameSign => {
                if first.re == 0.0
                    || second.re == 0.0
                    || first.re.is_sign_positive() != second.re.is_sign_positive()
                {
                    return Err(EvaluationError::ambiguous(
                        "equal curvature left its selected same-sign magnitude branch",
                    ));
                }
                Ok(vec![(first - second) * self.model_scale])
            }
            CurveCurvatureRelation::MagnitudeOppositeSign => {
                if first.re == 0.0
                    || second.re == 0.0
                    || first.re.is_sign_positive() == second.re.is_sign_positive()
                {
                    return Err(EvaluationError::ambiguous(
                        "equal curvature left its selected opposite-sign magnitude branch",
                    ));
                }
                Ok(vec![(first + second) * self.model_scale])
            }
        }
    }
}

impl ResidualEvaluator for GenericEndpointContinuityResidual {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        evaluate_sketch_ad(self, variables, false).map(|(values, _)| values)
    }

    fn jacobian(&self, variables: &[VariableValue]) -> Result<Vec<LocalJacobian>, EvaluationError> {
        evaluate_sketch_ad(self, variables, true).map(|(_, jacobians)| jacobians)
    }
}

impl SketchAdFormula for GenericEndpointContinuityResidual {
    fn evaluate_dual(
        &self,
        variables: &[SketchAdValue],
    ) -> Result<Vec<DualDVec64>, EvaluationError> {
        let first = evaluate_ad_curve(variables, &self.first)?;
        let second = evaluate_ad_curve(variables, &self.second)?;
        let mut values = vec![
            &first.position[0] - &second.position[0],
            &first.position[1] - &second.position[1],
        ];
        match self.kind {
            CurveContinuity::G0 => {}
            CurveContinuity::G1 | CurveContinuity::G2 => {
                let first_path = [
                    first.first[0].clone() * self.first_sign,
                    first.first[1].clone() * self.first_sign,
                ];
                let second_path = [
                    second.first[0].clone() * self.second_sign,
                    second.first[1].clone() * self.second_sign,
                ];
                let first_unit = ad_unit(&first_path, "first endpoint path tangent")?;
                let second_unit = ad_unit(&second_path, "second endpoint path tangent")?;
                let cosine =
                    (&first_unit[0] * &second_unit[0] + &first_unit[1] * &second_unit[1]).re;
                if cosine <= 0.0 {
                    return Err(EvaluationError::ambiguous(
                        "endpoint continuity crossed its ordered path tangent orientation",
                    ));
                }
                values.push(&first_unit[0] * &second_unit[1] - &first_unit[1] * &second_unit[0]);
                if self.kind == CurveContinuity::G2 {
                    let first_curvature = ad_signed_curvature(&first, "first endpoint curve")?;
                    let second_curvature = ad_signed_curvature(&second, "second endpoint curve")?;
                    values.push(
                        (first_curvature * self.first_sign - second_curvature * self.second_sign)
                            * self.model_scale,
                    );
                }
            }
            CurveContinuity::ParametricC2 {
                first_rate,
                second_rate,
            } => {
                for coordinate in 0..2 {
                    values.push(
                        first.first[coordinate].clone() * (self.first_sign * first_rate)
                            - second.first[coordinate].clone() * (self.second_sign * second_rate),
                    );
                }
                for coordinate in 0..2 {
                    values.push(
                        first.second[coordinate].clone() * first_rate * first_rate
                            - second.second[coordinate].clone() * second_rate * second_rate,
                    );
                }
            }
        }
        Ok(values)
    }
}

impl ResidualEvaluator for GenericCurveFilletResidual {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        evaluate_sketch_ad(self, variables, false).map(|(values, _)| values)
    }

    fn jacobian(&self, variables: &[VariableValue]) -> Result<Vec<LocalJacobian>, EvaluationError> {
        evaluate_sketch_ad(self, variables, true).map(|(_, jacobians)| jacobians)
    }
}

impl SketchAdFormula for GenericCurveFilletResidual {
    fn evaluate_dual(
        &self,
        variables: &[SketchAdValue],
    ) -> Result<Vec<DualDVec64>, EvaluationError> {
        let center = ad_point(variables, self.center, "curve fillet center")?;
        let radius = ad_scalar(variables, self.radius, "curve fillet radius")?;
        if !radius.re.is_finite() || radius.re <= 0.0 {
            return Err(EvaluationError::invalid_geometry(
                "curve fillet radius must be positive and finite",
            ));
        }
        let first = evaluate_ad_curve(variables, &self.first)?;
        let second = evaluate_ad_curve(variables, &self.second)?;
        let first_tangent = ad_unit(&first.first, "first fillet parent")?;
        let second_tangent = ad_unit(&second.first, "second fillet parent")?;
        let first_sign = match self.first_side {
            CurveNormalSide::Left => 1.0,
            CurveNormalSide::Right => -1.0,
        };
        let second_sign = match self.second_side {
            CurveNormalSide::Left => 1.0,
            CurveNormalSide::Right => -1.0,
        };
        let first_normal = [-first_tangent[1].clone(), first_tangent[0].clone()];
        let second_normal = [-second_tangent[1].clone(), second_tangent[0].clone()];
        let first_radial = [
            &first.position[0] - &center[0],
            &first.position[1] - &center[1],
        ];
        let second_radial = [
            &second.position[0] - &center[0],
            &second.position[1] - &center[1],
        ];
        let first_radial = ad_unit(&first_radial, "first fillet parent radial")?;
        let second_radial = ad_unit(&second_radial, "second fillet parent radial")?;
        let (start_parent_radial, end_parent_radial) = match self.endpoint_order {
            FilletEndpointOrder::FirstThenSecond => (&first_radial, &second_radial),
            FilletEndpointOrder::SecondThenFirst => (&second_radial, &first_radial),
        };
        let start_angle = ad_scalar(variables, self.start_angle, "fillet start angle")?;
        let end_angle = ad_scalar(variables, self.end_angle, "fillet end angle")?;
        let start_sine = start_angle.clone().sin();
        let start_cosine = start_angle.clone().cos();
        let end_sine = end_angle.clone().sin();
        let end_cosine = end_angle.clone().cos();
        let start_dot =
            (&start_cosine * &start_parent_radial[0] + &start_sine * &start_parent_radial[1]).re;
        let end_dot = (&end_cosine * &end_parent_radial[0] + &end_sine * &end_parent_radial[1]).re;
        if !start_dot.is_finite() || !end_dot.is_finite() || start_dot <= 0.0 || end_dot <= 0.0 {
            return Err(EvaluationError::ambiguous(
                "curve fillet output radial direction crossed its ordered endpoint branch",
            ));
        }
        Ok(vec![
            &center[0] - &first.position[0] - (radius * &first_normal[0]) * first_sign,
            &center[1] - &first.position[1] - (radius * &first_normal[1]) * first_sign,
            &center[0] - &second.position[0] - (radius * &second_normal[0]) * second_sign,
            &center[1] - &second.position[1] - (radius * &second_normal[1]) * second_sign,
            &start_cosine * &start_parent_radial[1] - &start_sine * &start_parent_radial[0],
            &end_cosine * &end_parent_radial[1] - &end_sine * &end_parent_radial[0],
        ])
    }
}

fn evaluate_sketch_ad(
    formula: &impl SketchAdFormula,
    variables: &[VariableValue],
    seeded: bool,
) -> Result<(Vec<f64>, Vec<LocalJacobian>), EvaluationError> {
    let width = if seeded {
        variables
            .iter()
            .map(|value| value.kind().tangent_dimension())
            .sum()
    } else {
        0
    };
    let mut offsets = Vec::with_capacity(variables.len());
    let mut offset = 0;
    let values = variables
        .iter()
        .map(|value| {
            offsets.push(offset);
            let result = match *value {
                VariableValue::Scalar(value) => {
                    SketchAdValue::Scalar(ad_seed(value, width, offset, seeded))
                }
                VariableValue::Vec2(value) => SketchAdValue::Point([
                    ad_seed(value[0], width, offset, seeded),
                    ad_seed(value[1], width, offset + 1, seeded),
                ]),
                VariableValue::Vec3(_) | VariableValue::Pose2(_) | VariableValue::Pose3(_) => {
                    return Err(EvaluationError::invalid_geometry(
                        "generic sketch curve residual accepts only Scalar and Vec2 incidence",
                    ));
                }
            };
            offset += value.kind().tangent_dimension();
            Ok(result)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let outputs = formula.evaluate_dual(&values)?;
    let real = outputs.iter().map(|value| value.re).collect();
    let jacobians = if seeded {
        variables
            .iter()
            .enumerate()
            .map(|(block, variable)| {
                let columns = variable.kind().tangent_dimension();
                let mut derivatives = Vec::with_capacity(outputs.len() * columns);
                for output in &outputs {
                    for column in 0..columns {
                        derivatives.push(ad_derivative(output, offsets[block] + column));
                    }
                }
                LocalJacobian::new(outputs.len(), columns, derivatives)
            })
            .collect()
    } else {
        Vec::new()
    };
    Ok((real, jacobians))
}

fn ad_seed(real: f64, width: usize, coordinate: usize, seeded: bool) -> DualDVec64 {
    if seeded {
        DualDVec64::from_re(real).derivative(width, coordinate)
    } else {
        DualDVec64::from_re(real)
    }
}

fn ad_derivative(value: &DualDVec64, coordinate: usize) -> f64 {
    value
        .eps
        .0
        .as_ref()
        .map_or(0.0, |derivatives| derivatives[coordinate])
}

fn ad_point<'a>(
    variables: &'a [SketchAdValue],
    index: usize,
    context: &str,
) -> Result<&'a [DualDVec64; 2], EvaluationError> {
    match variables.get(index) {
        Some(SketchAdValue::Point(point)) => Ok(point),
        _ => Err(EvaluationError::invalid_geometry(format!(
            "{context} expected a point at incidence {index}"
        ))),
    }
}

fn ad_scalar<'a>(
    variables: &'a [SketchAdValue],
    index: usize,
    context: &str,
) -> Result<&'a DualDVec64, EvaluationError> {
    match variables.get(index) {
        Some(SketchAdValue::Scalar(value)) => Ok(value),
        _ => Err(EvaluationError::invalid_geometry(format!(
            "{context} expected a scalar at incidence {index}"
        ))),
    }
}

fn validate_ad_parameter(parameter: &DualDVec64, context: &str) -> Result<(), EvaluationError> {
    if parameter.re.is_finite()
        && (-CONTACT_PARAMETER_ROUNDOFF_TOLERANCE..=1.0 + CONTACT_PARAMETER_ROUNDOFF_TOLERANCE)
            .contains(&parameter.re)
    {
        Ok(())
    } else {
        Err(EvaluationError::out_of_domain(format!(
            "{context} parameter escaped bounded span [0, 1]"
        )))
    }
}

fn evaluate_ad_bezier(
    variables: &[SketchAdValue],
    controls: BezierIncidence,
    parameter: &DualDVec64,
) -> Result<AdCurveJet2, EvaluationError> {
    match controls {
        BezierIncidence::Quadratic([first, second, third]) => {
            let first = ad_point(variables, first, "quadratic Bezier")?;
            let second = ad_point(variables, second, "quadratic Bezier")?;
            let third = ad_point(variables, third, "quadratic Bezier")?;
            let one_minus = DualDVec64::from_re(1.0) - parameter;
            let two = DualDVec64::from_re(2.0);
            let position = std::array::from_fn(|coordinate| {
                &one_minus * &one_minus * &first[coordinate]
                    + &two * &one_minus * parameter * &second[coordinate]
                    + parameter * parameter * &third[coordinate]
            });
            let derivative = std::array::from_fn(|coordinate| {
                &two * (&one_minus * (&second[coordinate] - &first[coordinate])
                    + parameter * (&third[coordinate] - &second[coordinate]))
            });
            let second_derivative = std::array::from_fn(|coordinate| {
                &two * (&third[coordinate] - second[coordinate].clone() * 2.0 + &first[coordinate])
            });
            Ok(AdCurveJet2 {
                position,
                first: derivative,
                second: second_derivative,
            })
        }
        BezierIncidence::Cubic([first, second, third, fourth]) => {
            let first = ad_point(variables, first, "cubic Bezier")?;
            let second = ad_point(variables, second, "cubic Bezier")?;
            let third = ad_point(variables, third, "cubic Bezier")?;
            let fourth = ad_point(variables, fourth, "cubic Bezier")?;
            let one_minus = DualDVec64::from_re(1.0) - parameter;
            let three = DualDVec64::from_re(3.0);
            let six = DualDVec64::from_re(6.0);
            let position = std::array::from_fn(|coordinate| {
                &one_minus * &one_minus * &one_minus * &first[coordinate]
                    + &three * &one_minus * &one_minus * parameter * &second[coordinate]
                    + &three * &one_minus * parameter * parameter * &third[coordinate]
                    + parameter * parameter * parameter * &fourth[coordinate]
            });
            let derivative = std::array::from_fn(|coordinate| {
                &three * &one_minus * &one_minus * (&second[coordinate] - &first[coordinate])
                    + &six * &one_minus * parameter * (&third[coordinate] - &second[coordinate])
                    + &three * parameter * parameter * (&fourth[coordinate] - &third[coordinate])
            });
            let second_derivative = std::array::from_fn(|coordinate| {
                &six * (&one_minus
                    * (&third[coordinate] - second[coordinate].clone() * 2.0 + &first[coordinate])
                    + parameter
                        * (&fourth[coordinate] - third[coordinate].clone() * 2.0
                            + &second[coordinate]))
            });
            Ok(AdCurveJet2 {
                position,
                first: derivative,
                second: second_derivative,
            })
        }
    }
}

#[allow(clippy::too_many_lines)]
fn evaluate_ad_curve(
    variables: &[SketchAdValue],
    curve: &GenericCurveIncidence,
) -> Result<AdCurveJet2, EvaluationError> {
    match curve {
        GenericCurveIncidence::Line {
            points: [start, end],
            parameter,
            bounded,
        } => {
            let start = ad_point(variables, *start, "generic line")?;
            let end = ad_point(variables, *end, "generic line")?;
            let parameter = curve_parameter(variables, *parameter, *bounded, "generic line")?;
            let derivative = [&end[0] - &start[0], &end[1] - &start[1]];
            let position = [
                &start[0] + &parameter * &derivative[0],
                &start[1] + &parameter * &derivative[1],
            ];
            Ok(AdCurveJet2 {
                position,
                first: derivative,
                second: std::array::from_fn(|_| DualDVec64::from_re(0.0)),
            })
        }
        GenericCurveIncidence::Circle {
            center,
            radius,
            parameter,
        } => {
            let center = ad_point(variables, *center, "generic circle")?;
            let radius = ad_scalar(variables, *radius, "generic circle")?;
            if !radius.re.is_finite() || radius.re <= 0.0 {
                return Err(EvaluationError::invalid_geometry(
                    "generic circle radius must be positive and finite",
                ));
            }
            let angle = curve_parameter(variables, *parameter, false, "generic circle")?;
            let sine = angle.clone().sin();
            let cosine = angle.cos();
            Ok(AdCurveJet2 {
                position: [&center[0] + radius * &cosine, &center[1] + radius * &sine],
                first: [-radius * &sine, radius * &cosine],
                second: [-radius * cosine, -radius * sine],
            })
        }
        GenericCurveIncidence::Arc {
            center,
            radius,
            start_angle,
            end_angle,
            turn_offset,
            sweep,
            parameter,
        } => {
            let center = ad_point(variables, *center, "generic arc")?;
            let radius = ad_scalar(variables, *radius, "generic arc")?;
            let start_angle = curve_parameter(variables, *start_angle, false, "generic arc start")?;
            let end_angle = curve_parameter(variables, *end_angle, false, "generic arc end")?;
            let signed_sweep =
                &end_angle - &start_angle + f64::from(*turn_offset) * std::f64::consts::TAU;
            let sweep_valid = match sweep {
                crate::ArcSweep::CounterClockwise => signed_sweep.re > 0.0,
                crate::ArcSweep::Clockwise => signed_sweep.re < 0.0,
            };
            if !radius.re.is_finite()
                || radius.re <= 0.0
                || !signed_sweep.re.is_finite()
                || signed_sweep.re == 0.0
                || signed_sweep.re.abs() >= std::f64::consts::TAU
                || !sweep_valid
            {
                return Err(EvaluationError::invalid_geometry(
                    "generic arc definition must be finite and regular",
                ));
            }
            let parameter = curve_parameter(variables, *parameter, true, "generic arc")?;
            let angle = &start_angle + &parameter * &signed_sweep;
            let sine = angle.clone().sin();
            let cosine = angle.cos();
            let first_scale = radius.clone() * &signed_sweep;
            let second_scale = radius.clone() * &signed_sweep * &signed_sweep;
            let jet = AdCurveJet2 {
                position: [&center[0] + radius * &cosine, &center[1] + radius * &sine],
                first: [-&first_scale * &sine, &first_scale * &cosine],
                second: [-&second_scale * cosine, -&second_scale * sine],
            };
            require_finite_ad_jet(&jet.position, &jet.first, &jet.second, "generic arc")?;
            Ok(jet)
        }
        GenericCurveIncidence::QuadraticBezier {
            controls,
            parameter,
        } => {
            let parameter = curve_parameter(variables, *parameter, true, "quadratic Bezier")?;
            evaluate_ad_bezier(variables, BezierIncidence::Quadratic(*controls), &parameter)
        }
        GenericCurveIncidence::CubicBezier {
            controls,
            parameter,
        } => {
            let parameter = curve_parameter(variables, *parameter, true, "cubic Bezier")?;
            evaluate_ad_bezier(variables, BezierIncidence::Cubic(*controls), &parameter)
        }
        GenericCurveIncidence::Ellipse {
            center,
            major_axis_point,
            minor_axis_ratio,
            parameter,
        } => {
            let center = ad_point(variables, *center, "ellipse")?;
            let axis_point = ad_point(variables, *major_axis_point, "ellipse")?;
            let ratio = ad_scalar(variables, *minor_axis_ratio, "ellipse")?;
            validate_ad_ellipse_ratio(ratio, "ellipse")?;
            let angle = curve_parameter(variables, *parameter, false, "ellipse")?;
            evaluate_ad_ellipse(center, axis_point, ratio, &angle, 1.0, "ellipse")
        }
        GenericCurveIncidence::EllipticalArc {
            center,
            major_axis_point,
            minor_axis_ratio,
            start_angle,
            signed_sweep,
            parameter,
        } => {
            if !start_angle.is_finite() || !signed_sweep.is_finite() || *signed_sweep == 0.0 {
                return Err(EvaluationError::invalid_geometry(
                    "elliptical arc angles must be finite with nonzero sweep",
                ));
            }
            let center = ad_point(variables, *center, "elliptical arc")?;
            let axis_point = ad_point(variables, *major_axis_point, "elliptical arc")?;
            let ratio = ad_scalar(variables, *minor_axis_ratio, "elliptical arc")?;
            validate_ad_ellipse_ratio(ratio, "elliptical arc")?;
            let parameter = curve_parameter(variables, *parameter, true, "elliptical arc")?;
            let angle = parameter * *signed_sweep + *start_angle;
            evaluate_ad_ellipse(
                center,
                axis_point,
                ratio,
                &angle,
                *signed_sweep,
                "elliptical arc",
            )
        }
        GenericCurveIncidence::RationalQuadratic {
            start,
            weighted_middle,
            middle_weight,
            end,
            parameter,
        } => {
            let start = ad_point(variables, *start, "rational quadratic")?;
            let weighted_middle = ad_point(
                variables,
                *weighted_middle,
                "rational quadratic weighted middle",
            )?;
            let end = ad_point(variables, *end, "rational quadratic")?;
            let weight = ad_scalar(variables, *middle_weight, "rational quadratic")?;
            if !weight.re.is_finite() || weight.re <= -1.0 {
                return Err(EvaluationError::out_of_domain(
                    "rational quadratic middle weight must be finite and strictly greater than -1",
                ));
            }
            let parameter = curve_parameter(variables, *parameter, true, "rational quadratic")?;
            let one = DualDVec64::from_re(1.0);
            let two = DualDVec64::from_re(2.0);
            let one_minus = &one - &parameter;
            let b0 = &one_minus * &one_minus;
            let b1 = &two * &one_minus * &parameter;
            let b2 = &parameter * &parameter;
            let weighted_b1 = weight * &b1;
            let denominator = &b0 + &weighted_b1 + &b2;
            let condition_scale = b0.re.abs() + weighted_b1.re.abs() + b2.re.abs();
            if !denominator.re.is_finite()
                || !condition_scale.is_finite()
                || denominator.re.abs() <= 64.0 * f64::EPSILON * condition_scale
            {
                return Err(EvaluationError::ambiguous(
                    "rational quadratic denominator is singular or ill-conditioned",
                ));
            }
            let b0_first = -(&two * &one_minus);
            let b1_first = &two * (&one - &two * &parameter);
            let b2_first = &two * &parameter;
            let denominator_first = &b0_first + weight * &b1_first + &b2_first;
            let b0_second = DualDVec64::from_re(2.0);
            let b1_second = DualDVec64::from_re(-4.0);
            let b2_second = DualDVec64::from_re(2.0);
            let denominator_second = &b0_second + weight * &b1_second + &b2_second;
            let position = std::array::from_fn(|coordinate| {
                (&b0 * &start[coordinate]
                    + &b1 * &weighted_middle[coordinate]
                    + &b2 * &end[coordinate])
                    / &denominator
            });
            let derivative = std::array::from_fn(|coordinate| {
                let numerator_first = &b0_first * &start[coordinate]
                    + &b1_first * &weighted_middle[coordinate]
                    + &b2_first * &end[coordinate];
                (numerator_first - &position[coordinate] * &denominator_first) / &denominator
            });
            let second_derivative = std::array::from_fn(|coordinate| {
                let numerator_second = &b0_second * &start[coordinate]
                    + &b1_second * &weighted_middle[coordinate]
                    + &b2_second * &end[coordinate];
                (numerator_second
                    - &position[coordinate] * &denominator_second
                    - &derivative[coordinate] * (denominator_first.clone() * 2.0))
                    / &denominator
            });
            require_finite_ad_jet(
                &position,
                &derivative,
                &second_derivative,
                "rational quadratic",
            )?;
            Ok(AdCurveJet2 {
                position,
                first: derivative,
                second: second_derivative,
            })
        }
        GenericCurveIncidence::ParabolaSegment {
            vertex,
            focus,
            trim,
            parameter,
        } => {
            let vertex = ad_point(variables, *vertex, "parabola")?;
            let focus = ad_point(variables, *focus, "parabola")?;
            let parameter = curve_parameter(variables, *parameter, true, "parabola")?;
            let native = parameter * trim.signed_rate() + trim.start();
            let direction = [&focus[0] - &vertex[0], &focus[1] - &vertex[1]];
            require_ad_axis(&direction, "parabola focus axis")?;
            let normal = [-direction[1].clone(), direction[0].clone()];
            let two = DualDVec64::from_re(2.0);
            let position = std::array::from_fn(|coordinate| {
                &vertex[coordinate]
                    + &direction[coordinate] * &native * &native
                    + &two * &normal[coordinate] * &native
            });
            let derivative = std::array::from_fn(|coordinate| {
                (&two * &direction[coordinate] * &native + &two * &normal[coordinate])
                    * trim.signed_rate()
            });
            let second_derivative = std::array::from_fn(|coordinate| {
                &two * &direction[coordinate] * (trim.signed_rate() * trim.signed_rate())
            });
            require_finite_ad_jet(&position, &derivative, &second_derivative, "parabola")?;
            Ok(AdCurveJet2 {
                position,
                first: derivative,
                second: second_derivative,
            })
        }
        GenericCurveIncidence::HyperbolaSegment {
            center,
            transverse_axis_point,
            semi_conjugate,
            branch,
            trim,
            parameter,
        } => {
            let center = ad_point(variables, *center, "hyperbola")?;
            let axis_point = ad_point(variables, *transverse_axis_point, "hyperbola")?;
            let semi_conjugate = ad_scalar(variables, *semi_conjugate, "hyperbola")?;
            if !semi_conjugate.re.is_finite() || semi_conjugate.re <= 0.0 {
                return Err(EvaluationError::out_of_domain(
                    "hyperbola semi-conjugate axis must be positive and finite",
                ));
            }
            let parameter = curve_parameter(variables, *parameter, true, "hyperbola")?;
            let native = parameter * trim.signed_rate() + trim.start();
            let sine = native.clone().sinh();
            let cosine = native.cosh();
            if !sine.re.is_finite() || !cosine.re.is_finite() {
                return Err(EvaluationError::invalid_geometry(
                    "hyperbola native parameter overflowed sinh/cosh",
                ));
            }
            let direction = [&axis_point[0] - &center[0], &axis_point[1] - &center[1]];
            require_ad_axis(&direction, "hyperbola transverse axis")?;
            let length = (&direction[0] * &direction[0] + &direction[1] * &direction[1]).sqrt();
            let normal = [-&direction[1] / &length, &direction[0] / &length];
            let branch = branch.multiplier();
            let position = std::array::from_fn(|coordinate| {
                &center[coordinate]
                    + direction[coordinate].clone() * branch * &cosine
                    + &normal[coordinate] * semi_conjugate * &sine
            });
            let derivative = std::array::from_fn(|coordinate| {
                (direction[coordinate].clone() * branch * &sine
                    + &normal[coordinate] * semi_conjugate * &cosine)
                    * trim.signed_rate()
            });
            let rate_squared = trim.signed_rate() * trim.signed_rate();
            let second_derivative = std::array::from_fn(|coordinate| {
                (direction[coordinate].clone() * branch * &cosine
                    + &normal[coordinate] * semi_conjugate * &sine)
                    * rate_squared
            });
            require_finite_ad_jet(&position, &derivative, &second_derivative, "hyperbola")?;
            Ok(AdCurveJet2 {
                position,
                first: derivative,
                second: second_derivative,
            })
        }
        GenericCurveIncidence::BSpline {
            basis,
            span,
            controls,
            parameter,
        } => {
            let parameter = curve_parameter(variables, *parameter, true, "B-spline")?;
            let basis_jet = basis
                .basis_jet_on_span(*span, parameter.re)
                .map_err(|error| {
                    EvaluationError::invalid_geometry(format!(
                        "B-spline basis evaluation failed: {error}"
                    ))
                })?;
            if basis_jet.terms.len() != controls.len() {
                return Err(EvaluationError::invalid_geometry(
                    "B-spline active support does not match residual incidence",
                ));
            }
            let parameter_real = parameter.re;
            let delta = parameter - parameter_real;
            let mut position = std::array::from_fn(|_| DualDVec64::from_re(0.0));
            let mut derivative = std::array::from_fn(|_| DualDVec64::from_re(0.0));
            let mut second_derivative = std::array::from_fn(|_| DualDVec64::from_re(0.0));
            for (term, control) in basis_jet.terms.iter().zip(controls) {
                let control = ad_point(variables, *control, "B-spline control")?;
                let position_basis =
                    DualDVec64::from_re(term.derivatives[0]) + delta.clone() * term.derivatives[1];
                let derivative_basis =
                    DualDVec64::from_re(term.derivatives[1]) + delta.clone() * term.derivatives[2];
                let second_basis =
                    DualDVec64::from_re(term.derivatives[2]) + delta.clone() * term.derivatives[3];
                for coordinate in 0..2 {
                    position[coordinate] += &control[coordinate] * &position_basis;
                    derivative[coordinate] += &control[coordinate] * &derivative_basis;
                    second_derivative[coordinate] += &control[coordinate] * &second_basis;
                }
            }
            require_finite_ad_jet(&position, &derivative, &second_derivative, "B-spline")?;
            Ok(AdCurveJet2 {
                position,
                first: derivative,
                second: second_derivative,
            })
        }
        GenericCurveIncidence::Nurbs {
            basis,
            span,
            controls,
            weights,
            parameter,
        } => {
            let parameter = curve_parameter(variables, *parameter, true, "NURBS")?;
            let basis_jet = basis
                .basis_jet_on_span(*span, parameter.re)
                .map_err(|error| {
                    EvaluationError::invalid_geometry(format!(
                        "NURBS basis evaluation failed: {error}"
                    ))
                })?;
            if basis_jet.terms.len() != controls.len() || controls.len() != weights.len() {
                return Err(EvaluationError::invalid_geometry(
                    "NURBS active support does not match residual incidence",
                ));
            }
            let resolved_weights = weights
                .iter()
                .map(|weight| match *weight {
                    NurbsWeightIncidence::Variable(index) => {
                        ad_scalar(variables, index, "NURBS weight").cloned()
                    }
                    NurbsWeightIncidence::Fixed(value) => Ok(DualDVec64::from_re(value)),
                })
                .collect::<Result<Vec<_>, EvaluationError>>()?;
            if resolved_weights
                .iter()
                .any(|weight| !weight.re.is_finite() || weight.re <= 0.0)
            {
                return Err(EvaluationError::out_of_domain(
                    "NURBS weights must be positive and finite",
                ));
            }
            let maximum = resolved_weights
                .iter()
                .map(|weight| weight.re)
                .fold(0.0_f64, f64::max);
            let parameter_real = parameter.re;
            let delta = parameter - parameter_real;
            let resolved_controls = controls
                .iter()
                .map(|control| ad_point(variables, *control, "NURBS control").cloned())
                .collect::<Result<Vec<_>, _>>()?;
            let mut normalized_weights = Vec::with_capacity(resolved_weights.len());
            let mut basis_values = Vec::with_capacity(basis_jet.terms.len());
            let mut position_numerator: [DualDVec64; 2] =
                std::array::from_fn(|_| DualDVec64::from_re(0.0));
            let mut denominator = DualDVec64::from_re(0.0);
            let mut denominator_first = DualDVec64::from_re(0.0);
            let mut condition_scale = 0.0_f64;
            for (index, (term, weight)) in basis_jet.terms.iter().zip(&resolved_weights).enumerate()
            {
                let normalized_weight = weight.clone() / maximum;
                if normalized_weight.re == 0.0 {
                    return Err(EvaluationError::ambiguous(
                        "NURBS active weight ratio is not representable",
                    ));
                }
                let position_basis =
                    DualDVec64::from_re(term.derivatives[0]) + delta.clone() * term.derivatives[1];
                let derivative_basis =
                    DualDVec64::from_re(term.derivatives[1]) + delta.clone() * term.derivatives[2];
                let second_basis =
                    DualDVec64::from_re(term.derivatives[2]) + delta.clone() * term.derivatives[3];
                let position_weight = &normalized_weight * &position_basis;
                let derivative_weight = &normalized_weight * &derivative_basis;
                condition_scale += position_weight.re.abs();
                denominator += position_weight.clone();
                denominator_first += derivative_weight.clone();
                for coordinate in 0..2 {
                    let difference =
                        &resolved_controls[index][coordinate] - &resolved_controls[0][coordinate];
                    position_numerator[coordinate] +=
                        difference * &normalized_weight * &position_basis;
                }
                normalized_weights.push(normalized_weight);
                basis_values.push([position_basis, derivative_basis, second_basis]);
            }
            if !condition_scale.is_finite()
                || !denominator.re.is_finite()
                || denominator.re <= 64.0 * f64::EPSILON * condition_scale
            {
                return Err(EvaluationError::ambiguous(
                    "NURBS denominator is singular or ill-conditioned",
                ));
            }
            let position = std::array::from_fn(|coordinate| {
                &resolved_controls[0][coordinate] + &position_numerator[coordinate] / &denominator
            });
            let centered_first = ad_pairwise_rational_numerator(
                &resolved_controls,
                &normalized_weights,
                &basis_values,
                1,
            )?;
            let derivative = std::array::from_fn(|coordinate| {
                &centered_first[coordinate] / &denominator / &denominator
            });
            let centered_second = ad_pairwise_rational_numerator(
                &resolved_controls,
                &normalized_weights,
                &basis_values,
                2,
            )?;
            let second_derivative = std::array::from_fn(|coordinate| {
                (&centered_second[coordinate] / &denominator
                    - &derivative[coordinate] * (denominator_first.clone() * 2.0))
                    / &denominator
            });
            require_finite_ad_jet(&position, &derivative, &second_derivative, "NURBS")?;
            Ok(AdCurveJet2 {
                position,
                first: derivative,
                second: second_derivative,
            })
        }
    }
}

fn ad_pairwise_rational_numerator(
    controls: &[[DualDVec64; 2]],
    weights: &[DualDVec64],
    basis: &[[DualDVec64; 3]],
    order: usize,
) -> Result<[DualDVec64; 2], EvaluationError> {
    let mut numerator = std::array::from_fn(|_| DualDVec64::from_re(0.0));
    for first in 0..controls.len() {
        for second in first + 1..controls.len() {
            let weight_product = &weights[first] * &weights[second];
            if weight_product.re == 0.0 {
                return Err(EvaluationError::ambiguous(
                    "NURBS active weight product is not representable",
                ));
            }
            let basis_cross =
                &basis[first][order] * &basis[second][0] - &basis[second][order] * &basis[first][0];
            for coordinate in 0..2 {
                let difference = &controls[first][coordinate] - &controls[second][coordinate];
                let weighted = &difference * &weight_product;
                if difference.re != 0.0 && weighted.re == 0.0 {
                    return Err(EvaluationError::ambiguous(
                        "NURBS weighted control difference is not representable",
                    ));
                }
                numerator[coordinate] += weighted * &basis_cross;
            }
        }
    }
    Ok(numerator)
}

fn evaluate_ad_ellipse(
    center: &[DualDVec64; 2],
    axis_point: &[DualDVec64; 2],
    ratio: &DualDVec64,
    angle: &DualDVec64,
    angle_rate: f64,
    context: &str,
) -> Result<AdCurveJet2, EvaluationError> {
    let direction = [&axis_point[0] - &center[0], &axis_point[1] - &center[1]];
    require_ad_axis(&direction, context)?;
    let normal = [-direction[1].clone(), direction[0].clone()];
    let sine = angle.clone().sin();
    let cosine = angle.clone().cos();
    let position = std::array::from_fn(|coordinate| {
        &center[coordinate] + &direction[coordinate] * &cosine + ratio * &normal[coordinate] * &sine
    });
    let derivative = std::array::from_fn(|coordinate| {
        (-&direction[coordinate] * &sine + ratio * &normal[coordinate] * &cosine) * angle_rate
    });
    let second_derivative = std::array::from_fn(|coordinate| {
        (-&direction[coordinate] * &cosine - ratio * &normal[coordinate] * &sine)
            * (angle_rate * angle_rate)
    });
    require_finite_ad_jet(&position, &derivative, &second_derivative, context)?;
    Ok(AdCurveJet2 {
        position,
        first: derivative,
        second: second_derivative,
    })
}

fn validate_ad_ellipse_ratio(ratio: &DualDVec64, context: &str) -> Result<(), EvaluationError> {
    if ratio.re.is_finite() && ratio.re > 0.0 && ratio.re <= 1.0 {
        Ok(())
    } else {
        Err(EvaluationError::out_of_domain(format!(
            "{context} minor-axis ratio must satisfy 0 < ratio <= 1"
        )))
    }
}

fn require_ad_axis(axis: &[DualDVec64; 2], context: &str) -> Result<(), EvaluationError> {
    let length = axis[0].re.hypot(axis[1].re);
    if length.is_finite() && length > 0.0 {
        Ok(())
    } else {
        Err(EvaluationError::degenerate(format!(
            "{context} is collapsed or non-finite"
        )))
    }
}

fn require_finite_ad_jet(
    position: &[DualDVec64; 2],
    derivative: &[DualDVec64; 2],
    second_derivative: &[DualDVec64; 2],
    context: &str,
) -> Result<(), EvaluationError> {
    if position
        .iter()
        .chain(derivative)
        .chain(second_derivative)
        .all(|value| {
            value.re.is_finite()
                && value
                    .eps
                    .0
                    .as_ref()
                    .is_none_or(|derivatives| derivatives.iter().all(|entry| entry.is_finite()))
        })
    {
        Ok(())
    } else {
        Err(EvaluationError::invalid_geometry(format!(
            "{context} produced a non-finite jet"
        )))
    }
}

fn curve_parameter(
    variables: &[SketchAdValue],
    parameter: CurveParameterIncidence,
    bounded: bool,
    context: &str,
) -> Result<DualDVec64, EvaluationError> {
    let value = match parameter {
        CurveParameterIncidence::Variable(index) => ad_scalar(variables, index, context)?.clone(),
        CurveParameterIncidence::Fixed(value) => DualDVec64::from_re(value),
    };
    if !value.re.is_finite() {
        return Err(EvaluationError::out_of_domain(format!(
            "{context} parameter must be finite"
        )));
    }
    if bounded {
        validate_ad_parameter(&value, context)?;
    }
    Ok(value)
}

fn ad_curve_unit_tangent(
    variables: &[SketchAdValue],
    curve: &GenericCurveIncidence,
    jet: &AdCurveJet2,
    context: &str,
) -> Result<[DualDVec64; 2], EvaluationError> {
    require_ad_speed(&jet.first, context)?;
    match curve {
        GenericCurveIncidence::Circle { parameter, .. } => {
            let angle = curve_parameter(variables, *parameter, false, context)?;
            let sine = angle.clone().sin();
            let cosine = angle.cos();
            Ok([-sine, cosine])
        }
        GenericCurveIncidence::Arc {
            start_angle,
            end_angle,
            turn_offset,
            sweep,
            parameter,
            ..
        } => {
            let start = curve_parameter(variables, *start_angle, false, context)?;
            let end = curve_parameter(variables, *end_angle, false, context)?;
            let parameter = curve_parameter(variables, *parameter, true, context)?;
            let signed_sweep = &end - &start + f64::from(*turn_offset) * std::f64::consts::TAU;
            let angle = start + parameter * signed_sweep;
            let sine = angle.clone().sin();
            let cosine = angle.cos();
            let sign = match sweep {
                crate::ArcSweep::CounterClockwise => 1.0,
                crate::ArcSweep::Clockwise => -1.0,
            };
            Ok([-sine * sign, cosine * sign])
        }
        _ => ad_unit(&jet.first, context),
    }
}

fn require_ad_speed(derivative: &[DualDVec64; 2], context: &str) -> Result<(), EvaluationError> {
    let speed = derivative[0].re.hypot(derivative[1].re);
    if speed.is_finite() && speed > 0.0 {
        Ok(())
    } else {
        Err(EvaluationError::degenerate(format!(
            "{context} has zero or non-finite speed"
        )))
    }
}

fn ad_unit(
    derivative: &[DualDVec64; 2],
    context: &str,
) -> Result<[DualDVec64; 2], EvaluationError> {
    require_ad_speed(derivative, context)?;
    let scale = derivative[0].re.abs().max(derivative[1].re.abs());
    let scaled = [derivative[0].clone() / scale, derivative[1].clone() / scale];
    let norm = (&scaled[0] * &scaled[0] + &scaled[1] * &scaled[1]).sqrt();
    Ok([&scaled[0] / &norm, &scaled[1] / &norm])
}

fn ad_signed_curvature(jet: &AdCurveJet2, context: &str) -> Result<DualDVec64, EvaluationError> {
    require_ad_speed(&jet.first, context)?;
    let scale = jet.first[0].re.abs().max(jet.first[1].re.abs());
    let scaled_first = [jet.first[0].clone() / scale, jet.first[1].clone() / scale];
    let scaled_speed_squared =
        &scaled_first[0] * &scaled_first[0] + &scaled_first[1] * &scaled_first[1];
    let scaled_speed = scaled_speed_squared.clone().sqrt();
    let unit_tangent = [
        &scaled_first[0] / &scaled_speed,
        &scaled_first[1] / &scaled_speed,
    ];
    let normal_acceleration =
        -&unit_tangent[1] * &jet.second[0] + &unit_tangent[0] * &jet.second[1];
    let mut curvature = normal_acceleration / scale / scale / scaled_speed_squared;
    let immutable = geosolve_geometry::CurveJet2 {
        position: geosolve_geometry::Point2::origin(),
        first_derivative: geosolve_geometry::Vector2::new(jet.first[0].re, jet.first[1].re),
        second_derivative: geosolve_geometry::Vector2::new(jet.second[0].re, jet.second[1].re),
        third_derivative: geosolve_geometry::Vector2::zeros(),
        domain: geosolve_geometry::CurveParameterDomain::SupportingLine,
    }
    .differential()
    .map_err(|error| {
        EvaluationError::ambiguous(format!(
            "{context} curvature is not finitely resolvable: {error}"
        ))
    })?
    .signed_curvature;
    curvature.re = immutable;
    if curvature.re.is_finite()
        && curvature
            .eps
            .0
            .as_ref()
            .is_none_or(|derivatives| derivatives.iter().all(|entry| entry.is_finite()))
    {
        Ok(curvature)
    } else {
        Err(EvaluationError::ambiguous(format!(
            "{context} curvature is not finitely resolvable"
        )))
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct FixedCoordinateResidual {
    pub(crate) coordinate: usize,
    pub(crate) target: f64,
}

impl ResidualEvaluator for FixedCoordinateResidual {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        let point = one_point(variables, "fixed-coordinate")?;
        Ok(vec![point[self.coordinate] - self.target])
    }

    fn jacobian(&self, variables: &[VariableValue]) -> Result<Vec<LocalJacobian>, EvaluationError> {
        one_point(variables, "fixed-coordinate")?;
        let values = if self.coordinate == 0 {
            vec![1.0, 0.0]
        } else {
            vec![0.0, 1.0]
        };
        Ok(vec![LocalJacobian::new(1, 2, values)])
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct CoincidentResidual;

impl ResidualEvaluator for CoincidentResidual {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        let (first, second) = two_points(variables, "coincident")?;
        Ok(vec![second[0] - first[0], second[1] - first[1]])
    }

    fn jacobian(&self, variables: &[VariableValue]) -> Result<Vec<LocalJacobian>, EvaluationError> {
        two_points(variables, "coincident")?;
        Ok(vec![
            LocalJacobian::new(2, 2, vec![-1.0, 0.0, 0.0, -1.0]),
            LocalJacobian::new(2, 2, vec![1.0, 0.0, 0.0, 1.0]),
        ])
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct AxisDifferenceResidual {
    pub(crate) coordinate: usize,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct AxisDimensionResidual {
    pub(crate) coordinate: usize,
    pub(crate) target: f64,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct CollinearResidual {
    pub(crate) first: [usize; 2],
    pub(crate) second: [usize; 2],
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ExternalLineCollinearResidual {
    pub(crate) native: [usize; 2],
    pub(crate) external_start: [f64; 2],
    pub(crate) external_end: [f64; 2],
}

impl SketchAdFormula for ExternalLineCollinearResidual {
    fn evaluate_dual(
        &self,
        variables: &[SketchAdValue],
    ) -> Result<Vec<DualDVec64>, EvaluationError> {
        let a = ad_point(variables, self.native[0], "native collinear support")?;
        let b = ad_point(variables, self.native[1], "native collinear support")?;
        let native = ad_unit(&[&b[0] - &a[0], &b[1] - &a[1]], "native collinear support")?;
        let dx = self.external_end[0] - self.external_start[0];
        let dy = self.external_end[1] - self.external_start[1];
        let norm = dx.hypot(dy);
        if !norm.is_finite() || norm <= f64::EPSILON {
            return Err(EvaluationError::degenerate(
                "external collinear support is degenerate",
            ));
        }
        let external = [dx / norm, dy / norm];
        Ok(vec![
            native[0].clone() * external[1] - native[1].clone() * external[0],
            native[0].clone() * (DualDVec64::from_re(self.external_start[1]) - a[1].clone())
                - native[1].clone() * (DualDVec64::from_re(self.external_start[0]) - a[0].clone()),
        ])
    }
}

impl ResidualEvaluator for ExternalLineCollinearResidual {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        evaluate_sketch_ad(self, variables, false).map(|(values, _)| values)
    }

    fn jacobian(&self, variables: &[VariableValue]) -> Result<Vec<LocalJacobian>, EvaluationError> {
        evaluate_sketch_ad(self, variables, true).map(|(_, jacobians)| jacobians)
    }
}

impl SketchAdFormula for CollinearResidual {
    fn evaluate_dual(
        &self,
        variables: &[SketchAdValue],
    ) -> Result<Vec<DualDVec64>, EvaluationError> {
        let a = ad_point(variables, self.first[0], "first collinear support")?;
        let b = ad_point(variables, self.first[1], "first collinear support")?;
        let c = ad_point(variables, self.second[0], "second collinear support")?;
        let d = ad_point(variables, self.second[1], "second collinear support")?;
        let first = ad_unit(&[&b[0] - &a[0], &b[1] - &a[1]], "first collinear support")?;
        let second = ad_unit(&[&d[0] - &c[0], &d[1] - &c[1]], "second collinear support")?;
        Ok(vec![
            &first[0] * &second[1] - &first[1] * &second[0],
            &first[0] * (&c[1] - &a[1]) - &first[1] * (&c[0] - &a[0]),
        ])
    }
}

impl ResidualEvaluator for CollinearResidual {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        evaluate_sketch_ad(self, variables, false).map(|(values, _)| values)
    }

    fn jacobian(&self, variables: &[VariableValue]) -> Result<Vec<LocalJacobian>, EvaluationError> {
        evaluate_sketch_ad(self, variables, true).map(|(_, jacobians)| jacobians)
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct EqualDistanceResidual {
    pub(crate) first: [usize; 2],
    pub(crate) second: [usize; 2],
}

impl SketchAdFormula for EqualDistanceResidual {
    fn evaluate_dual(
        &self,
        variables: &[SketchAdValue],
    ) -> Result<Vec<DualDVec64>, EvaluationError> {
        let a = ad_point(variables, self.first[0], "first equal distance")?;
        let b = ad_point(variables, self.first[1], "first equal distance")?;
        let c = ad_point(variables, self.second[0], "second equal distance")?;
        let d = ad_point(variables, self.second[1], "second equal distance")?;
        let first = ((&b[0] - &a[0]).powi(2) + (&b[1] - &a[1]).powi(2)).sqrt();
        let second = ((&d[0] - &c[0]).powi(2) + (&d[1] - &c[1]).powi(2)).sqrt();
        if first.re == 0.0 || second.re == 0.0 {
            return Err(EvaluationError::degenerate(
                "equal distance requires two nonzero point-pair distances",
            ));
        }
        Ok(vec![first - second])
    }
}

impl ResidualEvaluator for EqualDistanceResidual {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        evaluate_sketch_ad(self, variables, false).map(|(values, _)| values)
    }

    fn jacobian(&self, variables: &[VariableValue]) -> Result<Vec<LocalJacobian>, EvaluationError> {
        evaluate_sketch_ad(self, variables, true).map(|(_, jacobians)| jacobians)
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct EqualAngleResidual {
    pub(crate) first: [[usize; 2]; 2],
    pub(crate) second: [[usize; 2]; 2],
    pub(crate) first_orientation: AngleOrientation,
    pub(crate) first_winding: i32,
    pub(crate) second_orientation: AngleOrientation,
    pub(crate) second_winding: i32,
}

impl SketchAdFormula for EqualAngleResidual {
    fn evaluate_dual(
        &self,
        variables: &[SketchAdValue],
    ) -> Result<Vec<DualDVec64>, EvaluationError> {
        let angle = |pair: [[usize; 2]; 2], orientation: AngleOrientation, winding: i32| {
            let a = ad_point(variables, pair[0][0], "equal angle first support")?;
            let b = ad_point(variables, pair[0][1], "equal angle first support")?;
            let c = ad_point(variables, pair[1][0], "equal angle second support")?;
            let d = ad_point(variables, pair[1][1], "equal angle second support")?;
            let first = ad_unit(&[&b[0] - &a[0], &b[1] - &a[1]], "equal angle support")?;
            let second = ad_unit(&[&d[0] - &c[0], &d[1] - &c[1]], "equal angle support")?;
            let cross = &first[0] * &second[1] - &first[1] * &second[0];
            let dot = &first[0] * &second[0] + &first[1] * &second[1];
            Ok::<_, EvaluationError>(
                cross.atan2(dot) * orientation.sign() + f64::from(winding) * std::f64::consts::TAU,
            )
        };
        Ok(vec![
            angle(self.first, self.first_orientation, self.first_winding)?
                - angle(self.second, self.second_orientation, self.second_winding)?,
        ])
    }
}

impl ResidualEvaluator for EqualAngleResidual {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        evaluate_sketch_ad(self, variables, false).map(|(values, _)| values)
    }

    fn jacobian(&self, variables: &[VariableValue]) -> Result<Vec<LocalJacobian>, EvaluationError> {
        evaluate_sketch_ad(self, variables, true).map(|(_, jacobians)| jacobians)
    }
}

impl ResidualEvaluator for AxisDifferenceResidual {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        let (start, end) = two_points(variables, "axis-difference")?;
        Ok(vec![end[self.coordinate] - start[self.coordinate]])
    }

    fn jacobian(&self, variables: &[VariableValue]) -> Result<Vec<LocalJacobian>, EvaluationError> {
        two_points(variables, "axis-difference")?;
        let (start, end) = if self.coordinate == 0 {
            (vec![-1.0, 0.0], vec![1.0, 0.0])
        } else {
            (vec![0.0, -1.0], vec![0.0, 1.0])
        };
        Ok(vec![
            LocalJacobian::new(1, 2, start),
            LocalJacobian::new(1, 2, end),
        ])
    }
}

impl ResidualEvaluator for AxisDimensionResidual {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        let (start, end) = two_points(variables, "axis-dimension")?;
        Ok(vec![
            end[self.coordinate] - start[self.coordinate] - self.target,
        ])
    }

    fn jacobian(&self, variables: &[VariableValue]) -> Result<Vec<LocalJacobian>, EvaluationError> {
        two_points(variables, "axis-dimension")?;
        let (start, end) = if self.coordinate == 0 {
            (vec![-1.0, 0.0], vec![1.0, 0.0])
        } else {
            (vec![0.0, -1.0], vec![0.0, 1.0])
        };
        Ok(vec![
            LocalJacobian::new(1, 2, start),
            LocalJacobian::new(1, 2, end),
        ])
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct DistanceResidual {
    pub(crate) target: f64,
}

impl ResidualEvaluator for DistanceResidual {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        let (first, second) = two_points(variables, "distance")?;
        let distance = (second[0] - first[0]).hypot(second[1] - first[1]);
        Ok(vec![distance - self.target])
    }

    fn jacobian(&self, variables: &[VariableValue]) -> Result<Vec<LocalJacobian>, EvaluationError> {
        let (first, second) = two_points(variables, "distance")?;
        let (dx, dy, distance) = displacement(first, second)?;
        let x = dx / distance;
        let y = dy / distance;
        Ok(vec![
            LocalJacobian::new(1, 2, vec![-x, -y]),
            LocalJacobian::new(1, 2, vec![x, y]),
        ])
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct PointTargetResidual {
    pub(crate) target: [f64; 2],
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ScalarTargetResidual {
    pub(crate) target: f64,
    pub(crate) multiplier: f64,
}

impl ResidualEvaluator for ScalarTargetResidual {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        let value = scalar_at(variables, 0, "scalar-target")?;
        Ok(vec![self.multiplier * value - self.target])
    }

    fn jacobian(&self, variables: &[VariableValue]) -> Result<Vec<LocalJacobian>, EvaluationError> {
        scalar_at(variables, 0, "scalar-target")?;
        Ok(vec![LocalJacobian::new(1, 1, vec![self.multiplier])])
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct PointOnLineResidual {
    pub(crate) point: usize,
    pub(crate) start: usize,
    pub(crate) end: usize,
    pub(crate) parameter: usize,
    pub(crate) domain: LineParameterDomain,
}

impl ResidualEvaluator for PointOnLineResidual {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        let point = point_at(variables, self.point, "point-on-line")?;
        let start = point_at(variables, self.start, "point-on-line")?;
        let end = point_at(variables, self.end, "point-on-line")?;
        let parameter = scalar_at(variables, self.parameter, "point-on-line")?;
        if !line_parameter_is_evaluable(self.domain, parameter) {
            return Err(EvaluationError::out_of_domain(format!(
                "point-on-line parameter escaped {}",
                self.domain.label()
            )));
        }
        let curve = CurveRef::Line {
            start,
            end,
            domain: self.domain,
        }
        .evaluate(parameter);
        Ok(vec![
            point[0] - curve.position[0],
            point[1] - curve.position[1],
        ])
    }

    fn jacobian(&self, variables: &[VariableValue]) -> Result<Vec<LocalJacobian>, EvaluationError> {
        point_at(variables, self.point, "point-on-line")?;
        let start = point_at(variables, self.start, "point-on-line")?;
        let end = point_at(variables, self.end, "point-on-line")?;
        let parameter = scalar_at(variables, self.parameter, "point-on-line")?;
        if !line_parameter_is_evaluable(self.domain, parameter) {
            return Err(EvaluationError::out_of_domain(format!(
                "point-on-line parameter escaped {}",
                self.domain.label()
            )));
        }
        let curve = CurveRef::Line {
            start,
            end,
            domain: self.domain,
        }
        .evaluate(parameter);
        require_regular(curve.degeneracy, "point-on-line")?;
        let mut blocks = zero_blocks(variables, 2)?;
        add_point_matrix(&mut blocks, self.point, [[1.0, 0.0], [0.0, 1.0]])?;
        add_point_matrix(
            &mut blocks,
            self.start,
            [[parameter - 1.0, 0.0], [0.0, parameter - 1.0]],
        )?;
        add_point_matrix(
            &mut blocks,
            self.end,
            [[-parameter, 0.0], [0.0, -parameter]],
        )?;
        add_scalar_column(
            &mut blocks,
            self.parameter,
            &[-curve.first_derivative[0], -curve.first_derivative[1]],
        )?;
        Ok(finish_blocks(blocks, 2))
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct PointOnCircleResidual {
    pub(crate) point: usize,
    pub(crate) center: usize,
    pub(crate) radius: usize,
    pub(crate) angle: usize,
}

impl ResidualEvaluator for PointOnCircleResidual {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        let (point, evaluation) = self.values(variables)?;
        Ok(vec![
            point[0] - evaluation.position[0],
            point[1] - evaluation.position[1],
        ])
    }

    fn jacobian(&self, variables: &[VariableValue]) -> Result<Vec<LocalJacobian>, EvaluationError> {
        let (_, evaluation) = self.values(variables)?;
        require_regular(evaluation.degeneracy, "point-on-circle")?;
        let angle = scalar_at(variables, self.angle, "point-on-circle")?;
        let radius = scalar_at(variables, self.radius, "point-on-circle")?;
        let (cosine, sine) = (angle.cos(), angle.sin());
        let mut blocks = zero_blocks(variables, 2)?;
        add_point_matrix(&mut blocks, self.point, [[1.0, 0.0], [0.0, 1.0]])?;
        add_point_matrix(&mut blocks, self.center, [[-1.0, 0.0], [0.0, -1.0]])?;
        add_scalar_column(&mut blocks, self.radius, &[-cosine, -sine])?;
        add_scalar_column(&mut blocks, self.angle, &[radius * sine, -radius * cosine])?;
        Ok(finish_blocks(blocks, 2))
    }
}

impl PointOnCircleResidual {
    fn values(
        self,
        variables: &[VariableValue],
    ) -> Result<([f64; 2], crate::curves::CurveEvaluation), EvaluationError> {
        let point = point_at(variables, self.point, "point-on-circle")?;
        let center = point_at(variables, self.center, "point-on-circle")?;
        let radius = scalar_at(variables, self.radius, "point-on-circle")?;
        let angle = scalar_at(variables, self.angle, "point-on-circle")?;
        Ok((point, CurveRef::Circle { center, radius }.evaluate(angle)))
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum SegmentPairEquation {
    Parallel,
    Perpendicular,
    EqualLength,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct SegmentPairResidual {
    pub(crate) first: [usize; 2],
    pub(crate) second: [usize; 2],
    pub(crate) equation: SegmentPairEquation,
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum LineOffsetResidualMode {
    SupportingLine,
    ExactTranslatedSegment,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct LineOffsetResidual {
    pub(crate) source: [usize; 2],
    pub(crate) target_segment: [usize; 2],
    pub(crate) target: f64,
    pub(crate) side: LineSide,
    pub(crate) orientation: LineOffsetOrientation,
    pub(crate) mode: LineOffsetResidualMode,
}

impl ResidualEvaluator for LineOffsetResidual {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        evaluate_sketch_ad(self, variables, false).map(|(values, _)| values)
    }

    fn jacobian(&self, variables: &[VariableValue]) -> Result<Vec<LocalJacobian>, EvaluationError> {
        evaluate_sketch_ad(self, variables, true).map(|(_, jacobians)| jacobians)
    }
}

impl SketchAdFormula for LineOffsetResidual {
    fn evaluate_dual(
        &self,
        variables: &[SketchAdValue],
    ) -> Result<Vec<DualDVec64>, EvaluationError> {
        let source_start = ad_point(variables, self.source[0], "line offset source start")?;
        let source_end = ad_point(variables, self.source[1], "line offset source end")?;
        let native_target_start = ad_point(
            variables,
            self.target_segment[0],
            "line offset target start",
        )?;
        let native_target_end =
            ad_point(variables, self.target_segment[1], "line offset target end")?;
        let (target_start, target_end) = self
            .orientation
            .target_endpoints(native_target_start, native_target_end);
        let source_direction = [
            &source_end[0] - &source_start[0],
            &source_end[1] - &source_start[1],
        ];
        let source_unit = ad_unit(&source_direction, "line offset source")?;
        let normal = [-&source_unit[1], source_unit[0].clone()];
        let signed_target = self.side.sign() * self.target;

        match self.mode {
            LineOffsetResidualMode::SupportingLine => {
                let target_direction = [
                    &target_end[0] - &target_start[0],
                    &target_end[1] - &target_start[1],
                ];
                let target_unit = ad_unit(&target_direction, "line offset target")?;
                let displacement = [
                    &target_start[0] - &source_start[0],
                    &target_start[1] - &source_start[1],
                ];
                Ok(vec![
                    &source_unit[0] * &target_unit[1] - &source_unit[1] * &target_unit[0],
                    &displacement[0] * &normal[0] + &displacement[1] * &normal[1] - signed_target,
                ])
            }
            LineOffsetResidualMode::ExactTranslatedSegment => Ok(vec![
                &target_start[0] - &source_start[0] - normal[0].clone() * signed_target,
                &target_start[1] - &source_start[1] - normal[1].clone() * signed_target,
                &target_end[0] - &source_end[0] - normal[0].clone() * signed_target,
                &target_end[1] - &source_end[1] - normal[1].clone() * signed_target,
            ]),
        }
    }
}

impl ResidualEvaluator for SegmentPairResidual {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        let (first, second) = self.directions(variables)?;
        let value = match self.equation {
            SegmentPairEquation::Parallel => match (unit(first), unit(second)) {
                (Some((first, _)), Some((second, _))) => cross(first, second),
                _ => 0.0,
            },
            SegmentPairEquation::Perpendicular => match (unit(first), unit(second)) {
                (Some((first, _)), Some((second, _))) => dot(first, second),
                _ => 0.0,
            },
            SegmentPairEquation::EqualLength => norm(first) - norm(second),
        };
        Ok(vec![value])
    }

    fn jacobian(&self, variables: &[VariableValue]) -> Result<Vec<LocalJacobian>, EvaluationError> {
        let (first, second) = self.directions(variables)?;
        let (first_unit, first_norm) = unit(first).ok_or_else(|| {
            EvaluationError::degenerate("segment-pair derivative requires two nonzero segments")
        })?;
        let (second_unit, second_norm) = unit(second).ok_or_else(|| {
            EvaluationError::degenerate("segment-pair derivative requires two nonzero segments")
        })?;
        let (gradient_first, gradient_second) = match self.equation {
            SegmentPairEquation::Parallel => (
                normalized_direction_gradient(
                    first_unit,
                    first_norm,
                    [second_unit[1], -second_unit[0]],
                ),
                normalized_direction_gradient(
                    second_unit,
                    second_norm,
                    [-first_unit[1], first_unit[0]],
                ),
            ),
            SegmentPairEquation::Perpendicular => (
                normalized_direction_gradient(first_unit, first_norm, second_unit),
                normalized_direction_gradient(second_unit, second_norm, first_unit),
            ),
            SegmentPairEquation::EqualLength => (first_unit, negate(second_unit)),
        };
        let mut blocks = zero_blocks(variables, 1)?;
        add_point_row(&mut blocks, self.first[0], negate(gradient_first))?;
        add_point_row(&mut blocks, self.first[1], gradient_first)?;
        add_point_row(&mut blocks, self.second[0], negate(gradient_second))?;
        add_point_row(&mut blocks, self.second[1], gradient_second)?;
        Ok(finish_blocks(blocks, 1))
    }
}

impl SegmentPairResidual {
    fn directions(
        self,
        variables: &[VariableValue],
    ) -> Result<([f64; 2], [f64; 2]), EvaluationError> {
        let first_start = point_at(variables, self.first[0], "segment-pair")?;
        let first_end = point_at(variables, self.first[1], "segment-pair")?;
        let second_start = point_at(variables, self.second[0], "segment-pair")?;
        let second_end = point_at(variables, self.second[1], "segment-pair")?;
        Ok((
            subtract(first_end, first_start),
            subtract(second_end, second_start),
        ))
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ScalarEqualityResidual;

impl ResidualEvaluator for ScalarEqualityResidual {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        Ok(vec![
            scalar_at(variables, 0, "scalar-equality")?
                - scalar_at(variables, 1, "scalar-equality")?,
        ])
    }

    fn jacobian(&self, variables: &[VariableValue]) -> Result<Vec<LocalJacobian>, EvaluationError> {
        scalar_at(variables, 0, "scalar-equality")?;
        scalar_at(variables, 1, "scalar-equality")?;
        Ok(vec![
            LocalJacobian::new(1, 1, vec![1.0]),
            LocalJacobian::new(1, 1, vec![-1.0]),
        ])
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct MidpointResidual {
    pub(crate) point: usize,
    pub(crate) start: usize,
    pub(crate) end: usize,
}

impl ResidualEvaluator for MidpointResidual {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        let point = point_at(variables, self.point, "midpoint")?;
        let start = point_at(variables, self.start, "midpoint")?;
        let end = point_at(variables, self.end, "midpoint")?;
        Ok(vec![
            point[0] - 0.5 * (start[0] + end[0]),
            point[1] - 0.5 * (start[1] + end[1]),
        ])
    }

    fn jacobian(&self, variables: &[VariableValue]) -> Result<Vec<LocalJacobian>, EvaluationError> {
        self.evaluate(variables)?;
        let mut blocks = zero_blocks(variables, 2)?;
        add_point_matrix(&mut blocks, self.point, [[1.0, 0.0], [0.0, 1.0]])?;
        let half = [[-0.5, 0.0], [0.0, -0.5]];
        add_point_matrix(&mut blocks, self.start, half)?;
        add_point_matrix(&mut blocks, self.end, half)?;
        Ok(finish_blocks(blocks, 2))
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct SymmetryResidual {
    pub(crate) first: usize,
    pub(crate) second: usize,
    pub(crate) line_start: usize,
    pub(crate) line_end: usize,
}

impl ResidualEvaluator for SymmetryResidual {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        let values = self.values(variables)?;
        let pair = subtract(values.second, values.first);
        let Some((axis, _)) = unit(values.direction) else {
            return Ok(vec![0.0, 0.0]);
        };
        let normal = left_normal(axis);
        Ok(vec![dot(normal, values.midpoint_offset), dot(axis, pair)])
    }

    fn jacobian(&self, variables: &[VariableValue]) -> Result<Vec<LocalJacobian>, EvaluationError> {
        let values = self.values(variables)?;
        let (axis, line_length) = unit(values.direction).ok_or_else(|| {
            EvaluationError::degenerate("symmetry derivative requires a nonzero supporting line")
        })?;
        let normal = left_normal(axis);
        let pair = subtract(values.second, values.first);
        let normal_direction_gradient = normalized_direction_gradient(
            axis,
            line_length,
            [values.midpoint_offset[1], -values.midpoint_offset[0]],
        );
        let axis_direction_gradient = normalized_direction_gradient(axis, line_length, pair);
        let mut blocks = zero_blocks(variables, 2)?;

        add_point_rows(
            &mut blocks,
            self.first,
            [[0.5 * normal[0], 0.5 * normal[1]], negate(axis)],
        )?;
        add_point_rows(
            &mut blocks,
            self.second,
            [[0.5 * normal[0], 0.5 * normal[1]], axis],
        )?;
        add_point_rows(
            &mut blocks,
            self.line_start,
            [
                subtract(negate(normal_direction_gradient), normal),
                negate(axis_direction_gradient),
            ],
        )?;
        add_point_rows(
            &mut blocks,
            self.line_end,
            [normal_direction_gradient, axis_direction_gradient],
        )?;
        Ok(finish_blocks(blocks, 2))
    }
}

impl SymmetryResidual {
    fn values(self, variables: &[VariableValue]) -> Result<SymmetryValues, EvaluationError> {
        let first = point_at(variables, self.first, "symmetry")?;
        let second = point_at(variables, self.second, "symmetry")?;
        let start = point_at(variables, self.line_start, "symmetry")?;
        let end = point_at(variables, self.line_end, "symmetry")?;
        let direction = subtract(end, start);
        let midpoint_offset = [
            0.5 * (first[0] + second[0]) - start[0],
            0.5 * (first[1] + second[1]) - start[1],
        ];
        Ok(SymmetryValues {
            first,
            second,
            direction,
            midpoint_offset,
        })
    }
}

struct SymmetryValues {
    first: [f64; 2],
    second: [f64; 2],
    direction: [f64; 2],
    midpoint_offset: [f64; 2],
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct LineCircleTangencyResidual {
    pub(crate) line_start: usize,
    pub(crate) line_end: usize,
    pub(crate) center: usize,
    pub(crate) radius: usize,
    pub(crate) line_parameter: usize,
    pub(crate) circle_angle: usize,
    pub(crate) domain: LineParameterDomain,
}

impl ResidualEvaluator for LineCircleTangencyResidual {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        let values = self.values(variables)?;
        let alignment = unit(values.line.first_derivative).map_or(0.0, |(line_direction, _)| {
            dot(line_direction, values.radial)
        });
        Ok(vec![
            values.line.position[0] - values.circle.position[0],
            values.line.position[1] - values.circle.position[1],
            alignment,
        ])
    }

    fn jacobian(&self, variables: &[VariableValue]) -> Result<Vec<LocalJacobian>, EvaluationError> {
        let values = self.values(variables)?;
        require_regular(values.line.degeneracy, "line-circle tangency line")?;
        require_regular(values.circle.degeneracy, "line-circle tangency circle")?;
        let (cosine, sine) = (values.circle_angle.cos(), values.circle_angle.sin());
        let circle_radius = scalar_at(variables, self.radius, "line-circle tangency")?;
        let line_t = values.line_parameter;
        let direction = values.line.first_derivative;
        let (line_direction, line_length) = unit(direction).ok_or_else(|| {
            EvaluationError::degenerate("line-circle tangency requires a nonzero line direction")
        })?;
        let alignment_direction =
            normalized_direction_gradient(line_direction, line_length, values.radial);
        let alignment_angle = dot(line_direction, [-sine, cosine]);
        let mut blocks = zero_blocks(variables, 3)?;
        add_point_rows(
            &mut blocks,
            self.line_start,
            [
                [1.0 - line_t, 0.0],
                [0.0, 1.0 - line_t],
                negate(alignment_direction),
            ],
        )?;
        add_point_rows(
            &mut blocks,
            self.line_end,
            [[line_t, 0.0], [0.0, line_t], alignment_direction],
        )?;
        add_point_rows(
            &mut blocks,
            self.center,
            [[-1.0, 0.0], [0.0, -1.0], [0.0, 0.0]],
        )?;
        add_scalar_column(&mut blocks, self.radius, &[-cosine, -sine, 0.0])?;
        add_scalar_column(
            &mut blocks,
            self.line_parameter,
            &[direction[0], direction[1], 0.0],
        )?;
        add_scalar_column(
            &mut blocks,
            self.circle_angle,
            &[
                circle_radius * sine,
                -circle_radius * cosine,
                alignment_angle,
            ],
        )?;
        Ok(finish_blocks(blocks, 3))
    }
}

struct LineCircleValues {
    line: crate::curves::CurveEvaluation,
    circle: crate::curves::CurveEvaluation,
    radial: [f64; 2],
    line_parameter: f64,
    circle_angle: f64,
}

impl LineCircleTangencyResidual {
    fn values(self, variables: &[VariableValue]) -> Result<LineCircleValues, EvaluationError> {
        let line_start = point_at(variables, self.line_start, "line-circle tangency")?;
        let line_end = point_at(variables, self.line_end, "line-circle tangency")?;
        let center = point_at(variables, self.center, "line-circle tangency")?;
        let radius = scalar_at(variables, self.radius, "line-circle tangency")?;
        let line_parameter = scalar_at(variables, self.line_parameter, "line-circle tangency")?;
        let circle_angle = scalar_at(variables, self.circle_angle, "line-circle tangency")?;
        if !line_parameter_is_evaluable(self.domain, line_parameter) {
            return Err(EvaluationError::out_of_domain(format!(
                "line-circle tangency parameter escaped {}",
                self.domain.label()
            )));
        }
        Ok(LineCircleValues {
            line: CurveRef::Line {
                start: line_start,
                end: line_end,
                domain: self.domain,
            }
            .evaluate(line_parameter),
            circle: CurveRef::Circle { center, radius }.evaluate(circle_angle),
            radial: [circle_angle.cos(), circle_angle.sin()],
            line_parameter,
            circle_angle,
        })
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct CircleTangencyResidual {
    pub(crate) first_center: usize,
    pub(crate) first_radius: usize,
    pub(crate) second_center: usize,
    pub(crate) second_radius: usize,
    pub(crate) mode: CircleTangencyMode,
}

#[derive(Clone, Debug)]
pub(crate) struct CircleArcTangencyResidual {
    pub(crate) circle: GenericCurveIncidence,
    pub(crate) arc: GenericCurveIncidence,
}

impl ResidualEvaluator for CircleArcTangencyResidual {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        evaluate_sketch_ad(self, variables, false).map(|(values, _)| values)
    }

    fn jacobian(&self, variables: &[VariableValue]) -> Result<Vec<LocalJacobian>, EvaluationError> {
        evaluate_sketch_ad(self, variables, true).map(|(_, jacobians)| jacobians)
    }
}

impl SketchAdFormula for CircleArcTangencyResidual {
    fn evaluate_dual(
        &self,
        variables: &[SketchAdValue],
    ) -> Result<Vec<DualDVec64>, EvaluationError> {
        let circle = evaluate_ad_curve(variables, &self.circle)?;
        let arc = evaluate_ad_curve(variables, &self.arc)?;
        let circle_tangent = ad_curve_unit_tangent(
            variables,
            &self.circle,
            &circle,
            "circle-arc tangency circle",
        )?;
        let arc_tangent =
            ad_curve_unit_tangent(variables, &self.arc, &arc, "circle-arc tangency arc")?;
        Ok(vec![
            &circle.position[0] - &arc.position[0],
            &circle.position[1] - &arc.position[1],
            &circle_tangent[0] * &arc_tangent[1] - &circle_tangent[1] * &arc_tangent[0],
        ])
    }
}

impl ResidualEvaluator for CircleTangencyResidual {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        let values = self.values(variables)?;
        Ok(vec![values.distance - values.target])
    }

    fn jacobian(&self, variables: &[VariableValue]) -> Result<Vec<LocalJacobian>, EvaluationError> {
        let values = self.values(variables)?;
        if values.distance == 0.0 {
            return Err(EvaluationError::degenerate(
                "circle tangency requires distinct centers",
            ));
        }
        if values.target <= 0.0 {
            return Err(EvaluationError::out_of_domain(
                "circle tangency effective radius is not positive",
            ));
        }
        let unit = [
            values.displacement[0] / values.distance,
            values.displacement[1] / values.distance,
        ];
        let (first_radius, second_radius) = match self.mode {
            CircleTangencyMode::External => (-1.0, -1.0),
            CircleTangencyMode::Internal {
                containment: CircleContainment::FirstContainsSecond,
            } => (-1.0, 1.0),
            CircleTangencyMode::Internal {
                containment: CircleContainment::SecondContainsFirst,
            } => (1.0, -1.0),
        };
        let mut blocks = zero_blocks(variables, 1)?;
        add_point_row(&mut blocks, self.first_center, negate(unit))?;
        add_scalar_column(&mut blocks, self.first_radius, &[first_radius])?;
        add_point_row(&mut blocks, self.second_center, unit)?;
        add_scalar_column(&mut blocks, self.second_radius, &[second_radius])?;
        Ok(finish_blocks(blocks, 1))
    }
}

struct CircleTangencyValues {
    displacement: [f64; 2],
    distance: f64,
    target: f64,
}

impl CircleTangencyResidual {
    fn values(self, variables: &[VariableValue]) -> Result<CircleTangencyValues, EvaluationError> {
        let first_center = point_at(variables, self.first_center, "circle tangency")?;
        let first_radius = scalar_at(variables, self.first_radius, "circle tangency")?;
        let second_center = point_at(variables, self.second_center, "circle tangency")?;
        let second_radius = scalar_at(variables, self.second_radius, "circle tangency")?;
        let displacement = subtract(second_center, first_center);
        Ok(CircleTangencyValues {
            displacement,
            distance: norm(displacement),
            target: tangency_distance(first_radius, second_radius, self.mode),
        })
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct OrientedAngleResidual {
    pub(crate) first: [usize; 2],
    pub(crate) second: [usize; 2],
    pub(crate) target: f64,
    pub(crate) orientation: AngleOrientation,
}

impl ResidualEvaluator for OrientedAngleResidual {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        let (first, second) = self.directions(variables)?;
        let principal = self.orientation.sign() * cross(first, second).atan2(dot(first, second));
        Ok(vec![unwrap_near(principal, self.target) - self.target])
    }

    fn jacobian(&self, variables: &[VariableValue]) -> Result<Vec<LocalJacobian>, EvaluationError> {
        let (first, second) = self.directions(variables)?;
        let first_squared = dot(first, first);
        let second_squared = dot(second, second);
        if first_squared == 0.0 || second_squared == 0.0 {
            return Err(EvaluationError::degenerate(
                "oriented angle requires two nonzero segments",
            ));
        }
        let sign = self.orientation.sign();
        let first_gradient = [
            sign * first[1] / first_squared,
            -sign * first[0] / first_squared,
        ];
        let second_gradient = [
            -sign * second[1] / second_squared,
            sign * second[0] / second_squared,
        ];
        let mut blocks = zero_blocks(variables, 1)?;
        add_point_row(&mut blocks, self.first[0], negate(first_gradient))?;
        add_point_row(&mut blocks, self.first[1], first_gradient)?;
        add_point_row(&mut blocks, self.second[0], negate(second_gradient))?;
        add_point_row(&mut blocks, self.second[1], second_gradient)?;
        Ok(finish_blocks(blocks, 1))
    }
}

impl OrientedAngleResidual {
    fn directions(
        self,
        variables: &[VariableValue],
    ) -> Result<([f64; 2], [f64; 2]), EvaluationError> {
        Ok((
            subtract(
                point_at(variables, self.first[1], "oriented angle")?,
                point_at(variables, self.first[0], "oriented angle")?,
            ),
            subtract(
                point_at(variables, self.second[1], "oriented angle")?,
                point_at(variables, self.second[0], "oriented angle")?,
            ),
        ))
    }
}

impl ResidualEvaluator for PointTargetResidual {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        let point = one_point(variables, "point-target")?;
        Ok(vec![point[0] - self.target[0], point[1] - self.target[1]])
    }

    fn jacobian(&self, variables: &[VariableValue]) -> Result<Vec<LocalJacobian>, EvaluationError> {
        one_point(variables, "point-target")?;
        Ok(vec![LocalJacobian::new(2, 2, vec![1.0, 0.0, 0.0, 1.0])])
    }
}

fn one_point(variables: &[VariableValue], context: &str) -> Result<[f64; 2], EvaluationError> {
    let [VariableValue::Vec2(point)] = variables else {
        return Err(EvaluationError::invalid_geometry(format!(
            "{context} residual expected one Vec2 point"
        )));
    };
    Ok(*point)
}

fn point_at(
    variables: &[VariableValue],
    index: usize,
    context: &str,
) -> Result<[f64; 2], EvaluationError> {
    match variables.get(index) {
        Some(VariableValue::Vec2(point)) => Ok(*point),
        _ => Err(EvaluationError::invalid_geometry(format!(
            "{context} residual expected Vec2 variable at incidence {index}"
        ))),
    }
}

fn scalar_at(
    variables: &[VariableValue],
    index: usize,
    context: &str,
) -> Result<f64, EvaluationError> {
    match variables.get(index) {
        Some(VariableValue::Scalar(value)) => Ok(*value),
        _ => Err(EvaluationError::invalid_geometry(format!(
            "{context} residual expected scalar variable at incidence {index}"
        ))),
    }
}

fn require_regular(degeneracy: CurveDegeneracy, context: &str) -> Result<(), EvaluationError> {
    match degeneracy {
        CurveDegeneracy::Regular => Ok(()),
        CurveDegeneracy::ZeroDerivative => Err(EvaluationError::degenerate(format!(
            "{context} curve has a zero derivative"
        ))),
        CurveDegeneracy::InvalidRadius => Err(EvaluationError::out_of_domain(format!(
            "{context} curve has a nonpositive radius"
        ))),
    }
}

fn line_parameter_is_evaluable(domain: LineParameterDomain, parameter: f64) -> bool {
    match domain {
        LineParameterDomain::SupportingLine => parameter.is_finite(),
        LineParameterDomain::BoundedSegment => bounded_parameter_is_evaluable(parameter),
    }
}

fn bounded_parameter_is_evaluable(parameter: f64) -> bool {
    parameter.is_finite()
        && (-CONTACT_PARAMETER_ROUNDOFF_TOLERANCE..=1.0 + CONTACT_PARAMETER_ROUNDOFF_TOLERANCE)
            .contains(&parameter)
}

fn subtract(first: [f64; 2], second: [f64; 2]) -> [f64; 2] {
    [first[0] - second[0], first[1] - second[1]]
}

fn negate(value: [f64; 2]) -> [f64; 2] {
    [-value[0], -value[1]]
}

fn dot(first: [f64; 2], second: [f64; 2]) -> f64 {
    first[0] * second[0] + first[1] * second[1]
}

fn cross(first: [f64; 2], second: [f64; 2]) -> f64 {
    first[0] * second[1] - first[1] * second[0]
}

fn norm(value: [f64; 2]) -> f64 {
    value[0].hypot(value[1])
}

fn unit(value: [f64; 2]) -> Option<([f64; 2], f64)> {
    let length = norm(value);
    (length.is_finite() && length > 0.0).then_some(([value[0] / length, value[1] / length], length))
}

fn left_normal(unit_direction: [f64; 2]) -> [f64; 2] {
    [-unit_direction[1], unit_direction[0]]
}

fn normalized_direction_gradient(
    unit_direction: [f64; 2],
    length: f64,
    gradient_with_respect_to_unit: [f64; 2],
) -> [f64; 2] {
    let axial = dot(unit_direction, gradient_with_respect_to_unit);
    [
        (gradient_with_respect_to_unit[0] - axial * unit_direction[0]) / length,
        (gradient_with_respect_to_unit[1] - axial * unit_direction[1]) / length,
    ]
}

fn zero_blocks(
    variables: &[VariableValue],
    rows: usize,
) -> Result<Vec<(usize, Vec<f64>)>, EvaluationError> {
    variables
        .iter()
        .map(|variable| {
            let columns = match variable {
                VariableValue::Scalar(_) => 1,
                VariableValue::Vec2(_) => 2,
                VariableValue::Vec3(_) | VariableValue::Pose2(_) | VariableValue::Pose3(_) => {
                    return Err(EvaluationError::invalid_geometry(
                        "sketch residual accepts only Scalar and Vec2 incidence",
                    ));
                }
            };
            Ok((columns, vec![0.0; rows * columns]))
        })
        .collect()
}

fn add_point_row(
    blocks: &mut [(usize, Vec<f64>)],
    index: usize,
    row: [f64; 2],
) -> Result<(), EvaluationError> {
    add_point_rows(blocks, index, [row])
}

fn add_point_matrix(
    blocks: &mut [(usize, Vec<f64>)],
    index: usize,
    matrix: [[f64; 2]; 2],
) -> Result<(), EvaluationError> {
    add_point_rows(blocks, index, matrix)
}

fn add_point_rows<const ROWS: usize>(
    blocks: &mut [(usize, Vec<f64>)],
    index: usize,
    rows: [[f64; 2]; ROWS],
) -> Result<(), EvaluationError> {
    let Some((columns, values)) = blocks.get_mut(index) else {
        return Err(EvaluationError::invalid_geometry(
            "point Jacobian incidence is out of range",
        ));
    };
    if *columns != 2 || values.len() != ROWS * 2 {
        return Err(EvaluationError::invalid_geometry(
            "point Jacobian incidence has the wrong shape",
        ));
    }
    for (row_index, row) in rows.into_iter().enumerate() {
        values[2 * row_index] += row[0];
        values[2 * row_index + 1] += row[1];
    }
    Ok(())
}

fn add_scalar_column(
    blocks: &mut [(usize, Vec<f64>)],
    index: usize,
    column: &[f64],
) -> Result<(), EvaluationError> {
    let Some((columns, values)) = blocks.get_mut(index) else {
        return Err(EvaluationError::invalid_geometry(
            "scalar Jacobian incidence is out of range",
        ));
    };
    if *columns != 1 || values.len() != column.len() {
        return Err(EvaluationError::invalid_geometry(
            "scalar Jacobian incidence has the wrong shape",
        ));
    }
    for (value, addition) in values.iter_mut().zip(column) {
        *value += addition;
    }
    Ok(())
}

fn finish_blocks(blocks: Vec<(usize, Vec<f64>)>, rows: usize) -> Vec<LocalJacobian> {
    blocks
        .into_iter()
        .map(|(columns, values)| LocalJacobian::new(rows, columns, values))
        .collect()
}

fn two_points(
    variables: &[VariableValue],
    context: &str,
) -> Result<([f64; 2], [f64; 2]), EvaluationError> {
    let [VariableValue::Vec2(first), VariableValue::Vec2(second)] = variables else {
        return Err(EvaluationError::invalid_geometry(format!(
            "{context} residual expected two Vec2 points"
        )));
    };
    Ok((*first, *second))
}

fn displacement(first: [f64; 2], second: [f64; 2]) -> Result<(f64, f64, f64), EvaluationError> {
    let dx = second[0] - first[0];
    let dy = second[1] - first[1];
    let distance = dx.hypot(dy);
    if distance == 0.0 {
        return Err(EvaluationError::nondifferentiable(
            "distance derivative is undefined for coincident points",
        ));
    }
    Ok((dx, dy, distance))
}

#[cfg(test)]
mod tests {
    use geosolve_core::{
        AuditBinding, Problem, ResidualBlock, ResidualCategory, ResidualRowAudit, SourceConstraint,
        VariableBlock,
    };

    use super::*;

    #[derive(Clone, Copy, Debug)]
    struct CurvePointProofResidual {
        curve: CurveRef,
    }

    impl ResidualEvaluator for CurvePointProofResidual {
        fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
            let point = point_at(variables, 0, "curve point proof")?;
            let parameter = scalar_at(variables, 1, "curve point proof")?;
            let curve = self.curve.evaluate(parameter);
            require_proof_curve(curve)?;
            Ok(vec![
                point[0] - curve.position[0],
                point[1] - curve.position[1],
            ])
        }

        fn jacobian(
            &self,
            variables: &[VariableValue],
        ) -> Result<Vec<LocalJacobian>, EvaluationError> {
            point_at(variables, 0, "curve point proof")?;
            let parameter = scalar_at(variables, 1, "curve point proof")?;
            let curve = self.curve.evaluate(parameter);
            require_proof_curve(curve)?;
            Ok(vec![
                LocalJacobian::new(2, 2, vec![1.0, 0.0, 0.0, 1.0]),
                LocalJacobian::new(
                    2,
                    1,
                    vec![-curve.first_derivative[0], -curve.first_derivative[1]],
                ),
            ])
        }
    }

    #[derive(Clone, Copy, Debug)]
    struct CurveTangencyProofResidual {
        curve: CurveRef,
    }

    impl ResidualEvaluator for CurveTangencyProofResidual {
        fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
            let start = point_at(variables, 0, "curve tangent proof")?;
            let end = point_at(variables, 1, "curve tangent proof")?;
            let parameter = scalar_at(variables, 2, "curve tangent proof")?;
            let curve = self.curve.evaluate(parameter);
            require_proof_curve(curve)?;
            let (line_axis, _) = unit(subtract(end, start)).ok_or_else(|| {
                EvaluationError::invalid_geometry("curve tangent proof has a zero line")
            })?;
            let (curve_axis, _) = unit(curve.first_derivative).ok_or_else(|| {
                EvaluationError::invalid_geometry("curve tangent proof has a zero derivative")
            })?;
            Ok(vec![cross(line_axis, curve_axis)])
        }

        fn jacobian(
            &self,
            variables: &[VariableValue],
        ) -> Result<Vec<LocalJacobian>, EvaluationError> {
            let start = point_at(variables, 0, "curve tangent proof")?;
            let end = point_at(variables, 1, "curve tangent proof")?;
            let parameter = scalar_at(variables, 2, "curve tangent proof")?;
            let curve = self.curve.evaluate(parameter);
            require_proof_curve(curve)?;
            let direction = subtract(end, start);
            let (line_axis, line_length) = unit(direction).ok_or_else(|| {
                EvaluationError::invalid_geometry("curve tangent proof has a zero line")
            })?;
            let (curve_axis, curve_speed) = unit(curve.first_derivative).ok_or_else(|| {
                EvaluationError::invalid_geometry("curve tangent proof has a zero derivative")
            })?;
            let line_gradient = normalized_direction_gradient(
                line_axis,
                line_length,
                [curve_axis[1], -curve_axis[0]],
            );
            let curve_gradient = normalized_direction_gradient(
                curve_axis,
                curve_speed,
                [-line_axis[1], line_axis[0]],
            );
            let second = curve.second_derivative.ok_or_else(|| {
                EvaluationError::invalid_geometry("curve tangent proof needs a second derivative")
            })?;
            Ok(vec![
                LocalJacobian::new(1, 2, negate(line_gradient).to_vec()),
                LocalJacobian::new(1, 2, line_gradient.to_vec()),
                LocalJacobian::new(1, 1, vec![dot(curve_gradient, second)]),
            ])
        }
    }

    fn require_proof_curve(
        evaluation: crate::curves::CurveEvaluation,
    ) -> Result<(), EvaluationError> {
        if evaluation.domain != crate::curves::CurveParameterDomain::BoundedProofCurve {
            return Err(EvaluationError::invalid_geometry(
                "proof residual did not use the bounded internal curve seam",
            ));
        }
        require_regular(evaluation.degeneracy, "curve proof")
    }

    fn row(name: &str) -> ResidualRowAudit {
        ResidualRowAudit::new(name, vec![AuditBinding::new("fixture", name)], "model-unit")
    }

    #[test]
    fn signed_axis_dimension_has_a_finite_difference_checked_jacobian() {
        let mut problem = Problem::new();
        let first = problem.add_variable(VariableBlock::vec2([4.0, -3.0], [2.0, 2.0]).unwrap());
        let second = problem.add_variable(VariableBlock::vec2([1.0, 5.0], [2.0, 2.0]).unwrap());
        let source = problem.add_source(SourceConstraint::new("signed axis dimension").unwrap());
        problem
            .add_residual(
                ResidualBlock::new(
                    source,
                    ResidualCategory::Hard,
                    vec![first, second],
                    1,
                    vec![2.0],
                    vec![row("signed x difference")],
                    AxisDimensionResidual {
                        coordinate: 0,
                        target: -3.0,
                    },
                )
                .unwrap(),
            )
            .unwrap();

        let report = problem.check_jacobians(1.0e-6).unwrap();
        assert!(report.all_within(1.0e-9), "{report:#?}");
    }

    #[test]
    fn m38_dimension_residuals_match_central_differences() {
        let mut problem = Problem::new();
        let sweep_start = problem.add_variable(VariableBlock::scalar(0.2, 1.0).unwrap());
        let sweep_end = problem.add_variable(VariableBlock::scalar(1.4, 1.0).unwrap());
        let radius = problem.add_variable(VariableBlock::scalar(2.4, 2.0).unwrap());
        let arc_start = problem.add_variable(VariableBlock::scalar(-0.4, 1.0).unwrap());
        let arc_end = problem.add_variable(VariableBlock::scalar(1.2, 1.0).unwrap());
        let center = problem.add_variable(VariableBlock::vec2([1.0, -2.0], [3.0, 3.0]).unwrap());
        let axis = problem.add_variable(VariableBlock::vec2([4.0, 0.0], [3.0, 3.0]).unwrap());
        let ratio = problem.add_variable(VariableBlock::scalar(0.4, 1.0).unwrap());
        let p0 = problem.add_variable(VariableBlock::vec2([0.0, 0.0], [2.0, 2.0]).unwrap());
        let p1 = problem.add_variable(VariableBlock::vec2([1.0, 2.0], [2.0, 2.0]).unwrap());
        let p2 = problem.add_variable(VariableBlock::vec2([4.0, 0.5], [2.0, 2.0]).unwrap());

        let cases = [
            (
                "M38 circular sweep",
                vec![sweep_start, sweep_end],
                M38DimensionResidual::CircularSweep(CircularSweepResidual {
                    start_angle: 0,
                    end_angle: 1,
                    turn_offset: 0,
                    target: 1.1,
                }),
                "radian",
            ),
            (
                "M38 circular arc length",
                vec![radius, arc_start, arc_end],
                M38DimensionResidual::CircularArcLength(CircularArcLengthResidual {
                    radius: 0,
                    start_angle: 1,
                    end_angle: 2,
                    turn_offset: 0,
                    target: 3.5,
                }),
                "model-unit",
            ),
            (
                "M38 conic property",
                vec![center, axis, ratio],
                M38DimensionResidual::ConicProperty(ConicPropertyResidual {
                    kind: ConicPropertyResidualKind::Ellipse {
                        center: 0,
                        axis: 1,
                        ratio: 2,
                        property: crate::M38ConicProperty::LinearEccentricity,
                    },
                    target: 2.5,
                }),
                "model-unit",
            ),
            (
                "M38 bounded path length",
                vec![p0, p1, p2],
                M38DimensionResidual::PathLength(Box::new(GenericPathLengthResidual {
                    first: GenericCurveIncidence::QuadraticBezier {
                        controls: [0, 1, 2],
                        parameter: CurveParameterIncidence::Fixed(0.0),
                    },
                    first_interval: [0.1, 0.9],
                    second: None,
                    target: 3.0,
                    tolerance: 1.0e-11,
                    max_evaluations: 8_193,
                })),
                "model-unit",
            ),
        ];

        for (label, incidence, evaluator, unit) in cases {
            let source = problem.add_source(SourceConstraint::new(label).unwrap());
            problem
                .add_residual(
                    ResidualBlock::new(
                        source,
                        ResidualCategory::Hard,
                        incidence,
                        1,
                        vec![2.0],
                        vec![ResidualRowAudit::new(
                            label,
                            vec![AuditBinding::new("dimension", label)],
                            unit,
                        )],
                        evaluator,
                    )
                    .unwrap(),
                )
                .unwrap();
        }

        let report = problem.check_jacobians(1.0e-6).unwrap();
        assert!(
            report.all_within(1.0e-6),
            "M38 residual error={:e}: {report:#?}",
            report.max_relative_error()
        );
    }

    #[test]
    fn private_quadratic_and_cubic_bezier_proofs_have_analytic_jacobians() {
        for curve in [
            CurveRef::QuadraticBezier {
                control: [[0.0, 0.0], [1.0, 2.0], [3.0, 1.0]],
            },
            CurveRef::CubicBezier {
                control: [[0.0, 0.0], [1.0, 2.0], [2.0, -1.0], [4.0, 1.0]],
            },
        ] {
            let mut problem = Problem::new();
            let point = problem.add_variable(VariableBlock::vec2([1.2, 0.8], [2.0, 2.0]).unwrap());
            let line_start =
                problem.add_variable(VariableBlock::vec2([-1.0, 0.5], [2.0, 2.0]).unwrap());
            let line_end =
                problem.add_variable(VariableBlock::vec2([2.5, 1.5], [2.0, 2.0]).unwrap());
            let parameter = problem.add_variable(VariableBlock::scalar(0.37, 1.0).unwrap());

            let point_source =
                problem.add_source(SourceConstraint::new("private Bezier point proof").unwrap());
            problem
                .add_residual(
                    ResidualBlock::new(
                        point_source,
                        ResidualCategory::Hard,
                        vec![point, parameter],
                        2,
                        vec![2.0, 2.0],
                        vec![row("Bezier point x"), row("Bezier point y")],
                        CurvePointProofResidual { curve },
                    )
                    .unwrap(),
                )
                .unwrap();
            let tangent_source =
                problem.add_source(SourceConstraint::new("private Bezier tangent proof").unwrap());
            problem
                .add_residual(
                    ResidualBlock::new(
                        tangent_source,
                        ResidualCategory::Hard,
                        vec![line_start, line_end, parameter],
                        1,
                        vec![1.0],
                        vec![row("Bezier line tangent")],
                        CurveTangencyProofResidual { curve },
                    )
                    .unwrap(),
                )
                .unwrap();

            let report = problem.check_jacobians(1.0e-5).unwrap();
            assert!(
                report.all_within(1.0e-6),
                "Bezier proof error={:e}: {report:#?}",
                report.max_relative_error()
            );
        }
    }

    #[test]
    fn private_bezier_cusp_is_explicitly_invalid() {
        let residual = CurveTangencyProofResidual {
            curve: CurveRef::CubicBezier {
                control: [[1.0, 1.0]; 4],
            },
        };
        let variables = [
            VariableValue::Vec2([0.0, 0.0]),
            VariableValue::Vec2([1.0, 0.0]),
            VariableValue::Scalar(0.5),
        ];
        assert!(matches!(
            residual.jacobian(&variables),
            Err(EvaluationError::Categorized {
                category: geosolve_core::EvaluationErrorCategory::Degenerate,
                ..
            })
        ));
    }

    #[test]
    fn private_nurbs_and_curvature_paths_retain_compensated_values() {
        let basis =
            geosolve_geometry::BSplineBasis::try_clamped(1, 2, vec![0.0, 0.0, 1.0, 1.0]).unwrap();
        let span = basis.spans()[0].index();
        let residual = GenericPointOnCurveResidual {
            point: 0,
            curve: GenericCurveIncidence::Nurbs {
                basis,
                span,
                controls: vec![1, 2],
                weights: vec![
                    NurbsWeightIncidence::Fixed(1.0),
                    NurbsWeightIncidence::Fixed(1.0e-12),
                ],
                parameter: CurveParameterIncidence::Fixed(1.0e-315),
            },
        };
        let values = residual
            .evaluate(&[
                VariableValue::Vec2([0.0, 0.0]),
                VariableValue::Vec2([0.0, 0.0]),
                VariableValue::Vec2([1.0e308, 0.0]),
            ])
            .unwrap();
        assert!(values[0] < 0.0);

        let curvature = ad_signed_curvature(
            &AdCurveJet2 {
                position: std::array::from_fn(|_| DualDVec64::from_re(0.0)),
                first: [DualDVec64::from_re(1.0e-100), DualDVec64::from_re(0.0)],
                second: [DualDVec64::from_re(1.0e200), DualDVec64::from_re(1.0e-224)],
            },
            "mixed-scale curvature proof",
        )
        .unwrap();
        assert!((curvature.re - 1.0e-24).abs() <= 1.0e-36);
    }
}
