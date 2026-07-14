use geosolve_core::{EvaluationError, LocalJacobian, ResidualEvaluator, VariableValue};

use crate::curves::{
    AngleOrientation, CircleContainment, CircleTangencyMode, CurveDegeneracy, CurveRef,
    LineParameterDomain, tangency_distance, unwrap_near,
};

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
pub(crate) struct PointOnArcResidual {
    pub(crate) point: usize,
    pub(crate) center: usize,
    pub(crate) radius: usize,
    pub(crate) parameter: usize,
    pub(crate) start_angle: f64,
    pub(crate) signed_sweep: f64,
}

impl ResidualEvaluator for PointOnArcResidual {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        let (point, evaluation, _) = self.values(variables)?;
        Ok(vec![
            point[0] - evaluation.position[0],
            point[1] - evaluation.position[1],
        ])
    }

    fn jacobian(&self, variables: &[VariableValue]) -> Result<Vec<LocalJacobian>, EvaluationError> {
        let (_, evaluation, parameter) = self.values(variables)?;
        require_regular(evaluation.degeneracy, "point-on-arc")?;
        let angle = self.start_angle + self.signed_sweep * parameter;
        let (cosine, sine) = (angle.cos(), angle.sin());
        let mut blocks = zero_blocks(variables, 2)?;
        add_point_matrix(&mut blocks, self.point, [[1.0, 0.0], [0.0, 1.0]])?;
        add_point_matrix(&mut blocks, self.center, [[-1.0, 0.0], [0.0, -1.0]])?;
        add_scalar_column(&mut blocks, self.radius, &[-cosine, -sine])?;
        add_scalar_column(
            &mut blocks,
            self.parameter,
            &[
                -evaluation.first_derivative[0],
                -evaluation.first_derivative[1],
            ],
        )?;
        Ok(finish_blocks(blocks, 2))
    }
}

impl PointOnArcResidual {
    fn values(
        self,
        variables: &[VariableValue],
    ) -> Result<([f64; 2], crate::curves::CurveEvaluation, f64), EvaluationError> {
        let point = point_at(variables, self.point, "point-on-arc")?;
        let center = point_at(variables, self.center, "point-on-arc")?;
        let radius = scalar_at(variables, self.radius, "point-on-arc")?;
        let parameter = scalar_at(variables, self.parameter, "point-on-arc")?;
        let evaluation = CurveRef::Arc {
            center,
            radius,
            start_angle: self.start_angle,
            signed_sweep: self.signed_sweep,
        }
        .evaluate(parameter);
        Ok((point, evaluation, parameter))
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
            EvaluationError::invalid_geometry(
                "segment-pair derivative requires two nonzero segments",
            )
        })?;
        let (second_unit, second_norm) = unit(second).ok_or_else(|| {
            EvaluationError::invalid_geometry(
                "segment-pair derivative requires two nonzero segments",
            )
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
            EvaluationError::invalid_geometry(
                "symmetry derivative requires a nonzero supporting line",
            )
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
            EvaluationError::invalid_geometry(
                "line-circle tangency requires a nonzero line direction",
            )
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

#[derive(Clone, Copy, Debug)]
pub(crate) struct CircleArcTangencyResidual {
    pub(crate) circle_center: usize,
    pub(crate) circle_radius: usize,
    pub(crate) arc_center: usize,
    pub(crate) arc_radius: usize,
    pub(crate) circle_angle: usize,
    pub(crate) arc_parameter: usize,
    pub(crate) arc_start_angle: f64,
    pub(crate) arc_signed_sweep: f64,
}

impl ResidualEvaluator for CircleArcTangencyResidual {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        let values = self.values(variables)?;
        require_regular(values.circle.degeneracy, "circle-arc tangency circle")?;
        require_regular(values.arc.degeneracy, "circle-arc tangency arc")?;
        let circle_tangent = canonical_unit(values.circle.first_derivative).ok_or_else(|| {
            EvaluationError::invalid_geometry("circle-arc tangency circle has a zero derivative")
        })?;
        let arc_tangent = canonical_unit(values.arc.first_derivative).ok_or_else(|| {
            EvaluationError::invalid_geometry("circle-arc tangency arc has a zero derivative")
        })?;
        Ok(vec![
            values.circle.position[0] - values.arc.position[0],
            values.circle.position[1] - values.arc.position[1],
            cross(circle_tangent, arc_tangent),
        ])
    }

    fn jacobian(&self, variables: &[VariableValue]) -> Result<Vec<LocalJacobian>, EvaluationError> {
        let values = self.values(variables)?;
        require_regular(values.circle.degeneracy, "circle-arc tangency circle")?;
        require_regular(values.arc.degeneracy, "circle-arc tangency arc")?;
        unit(values.circle.first_derivative).ok_or_else(|| {
            EvaluationError::invalid_geometry("circle-arc tangency circle has a zero derivative")
        })?;
        unit(values.arc.first_derivative).ok_or_else(|| {
            EvaluationError::invalid_geometry("circle-arc tangency arc has a zero derivative")
        })?;
        let circle_angle = values.circle_angle;
        let arc_angle = self.arc_start_angle + self.arc_signed_sweep * values.arc_parameter;
        let angle_difference = arc_angle - circle_angle;
        let circle_angle_derivative = -self.arc_signed_sweep.signum() * angle_difference.cos();
        let arc_parameter_derivative = self.arc_signed_sweep.abs() * angle_difference.cos();
        let (circle_cosine, circle_sine) = (circle_angle.cos(), circle_angle.sin());
        let (arc_cosine, arc_sine) = (arc_angle.cos(), arc_angle.sin());

        let mut blocks = zero_blocks(variables, 3)?;
        add_point_rows(
            &mut blocks,
            self.circle_center,
            [[1.0, 0.0], [0.0, 1.0], [0.0, 0.0]],
        )?;
        add_scalar_column(
            &mut blocks,
            self.circle_radius,
            &[circle_cosine, circle_sine, 0.0],
        )?;
        add_point_rows(
            &mut blocks,
            self.arc_center,
            [[-1.0, 0.0], [0.0, -1.0], [0.0, 0.0]],
        )?;
        add_scalar_column(&mut blocks, self.arc_radius, &[-arc_cosine, -arc_sine, 0.0])?;
        add_scalar_column(
            &mut blocks,
            self.circle_angle,
            &[
                values.circle.first_derivative[0],
                values.circle.first_derivative[1],
                circle_angle_derivative,
            ],
        )?;
        add_scalar_column(
            &mut blocks,
            self.arc_parameter,
            &[
                -values.arc.first_derivative[0],
                -values.arc.first_derivative[1],
                arc_parameter_derivative,
            ],
        )?;
        Ok(finish_blocks(blocks, 3))
    }
}

struct CircleArcTangencyValues {
    circle: crate::curves::CurveEvaluation,
    arc: crate::curves::CurveEvaluation,
    circle_angle: f64,
    arc_parameter: f64,
}

impl CircleArcTangencyResidual {
    fn values(
        self,
        variables: &[VariableValue],
    ) -> Result<CircleArcTangencyValues, EvaluationError> {
        let circle_center = point_at(variables, self.circle_center, "circle-arc tangency")?;
        let circle_radius = scalar_at(variables, self.circle_radius, "circle-arc tangency")?;
        let arc_center = point_at(variables, self.arc_center, "circle-arc tangency")?;
        let arc_radius = scalar_at(variables, self.arc_radius, "circle-arc tangency")?;
        let circle_angle = scalar_at(variables, self.circle_angle, "circle-arc tangency")?;
        let arc_parameter = scalar_at(variables, self.arc_parameter, "circle-arc tangency")?;
        Ok(CircleArcTangencyValues {
            circle: CurveRef::Circle {
                center: circle_center,
                radius: circle_radius,
            }
            .evaluate(circle_angle),
            arc: CurveRef::Arc {
                center: arc_center,
                radius: arc_radius,
                start_angle: self.arc_start_angle,
                signed_sweep: self.arc_signed_sweep,
            }
            .evaluate(arc_parameter),
            circle_angle,
            arc_parameter,
        })
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
            return Err(EvaluationError::invalid_geometry(
                "circle tangency requires distinct centers",
            ));
        }
        if values.target <= 0.0 {
            return Err(EvaluationError::invalid_geometry(
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
            return Err(EvaluationError::invalid_geometry(
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
        CurveDegeneracy::ZeroDerivative => Err(EvaluationError::invalid_geometry(format!(
            "{context} curve has a zero derivative"
        ))),
        CurveDegeneracy::InvalidRadius => Err(EvaluationError::invalid_geometry(format!(
            "{context} curve has a nonpositive radius"
        ))),
    }
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

fn canonical_unit(value: [f64; 2]) -> Option<[f64; 2]> {
    let (normalized, _) = unit(value)?;
    let angle = normalized[1].atan2(normalized[0]);
    Some([angle.cos(), angle.sin()])
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
                VariableValue::Pose2(_) => {
                    return Err(EvaluationError::invalid_geometry(
                        "sketch residual does not accept Pose2 incidence",
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
        return Err(EvaluationError::invalid_geometry(
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
            Err(EvaluationError::InvalidGeometry(_))
        ));
    }
}
