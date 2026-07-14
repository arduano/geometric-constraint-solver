use geosolve_core::{EvaluationError, LocalJacobian, ResidualEvaluator, VariableValue};

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
