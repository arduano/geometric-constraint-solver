use geosolve_core::{
    AuditBinding, EvaluationError, LocalJacobian, Problem, ResidualBlock, ResidualCategory,
    ResidualEvaluator, ResidualRowAudit, SolveReport, SourceConstraint, SourceConstraintId,
    VariableBlock, VariableId, VariableValue,
};

pub const FD_STEP: f64 = 1.0e-5;
pub const FD_TOLERANCE: f64 = 1.0e-6;
// Trace equality allows only relative floating-point bookkeeping roundoff.
pub const TRACE_COST_TOLERANCE_FACTOR: f64 = 128.0;

pub fn audit_row(template: impl Into<String>, binding: impl Into<String>) -> ResidualRowAudit {
    ResidualRowAudit::new(
        template,
        vec![AuditBinding::new(binding, "M3 synthetic fixture")],
        "model unit",
    )
}

pub fn add_source(problem: &mut Problem, label: &str) -> SourceConstraintId {
    problem.add_source(SourceConstraint::new(label).unwrap())
}

pub fn scalar_value(problem: &Problem, variable: VariableId) -> f64 {
    let VariableValue::Scalar(value) = problem.variable(variable).unwrap().value() else {
        panic!("expected scalar variable")
    };
    value
}

pub fn vec2_value(problem: &Problem, variable: VariableId) -> [f64; 2] {
    let VariableValue::Vec2(value) = problem.variable(variable).unwrap().value() else {
        panic!("expected Vec2 variable")
    };
    value
}

pub fn assert_jacobians(problem: &Problem) {
    let report = problem.check_jacobians(FD_STEP).unwrap();
    assert!(report.all_within(FD_TOLERANCE), "{report:#?}");
}

#[derive(Clone, Debug)]
pub struct AffineScalars {
    pub matrix: Vec<Vec<f64>>,
    pub target: Vec<f64>,
}

impl AffineScalars {
    fn scalar_values(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        let columns = self.matrix.first().map_or(variables.len(), Vec::len);
        if variables.len() != columns || self.matrix.iter().any(|row| row.len() != columns) {
            return Err(EvaluationError::invalid_geometry(
                "affine fixture dimensions do not match incidence",
            ));
        }
        variables
            .iter()
            .map(|value| match value {
                VariableValue::Scalar(value) => Ok(*value),
                _ => Err(EvaluationError::invalid_geometry(
                    "affine fixture expected scalar blocks",
                )),
            })
            .collect()
    }
}

impl ResidualEvaluator for AffineScalars {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        if self.matrix.len() != self.target.len() {
            return Err(EvaluationError::invalid_geometry(
                "affine fixture target dimension mismatch",
            ));
        }
        let values = self.scalar_values(variables)?;
        Ok(self
            .matrix
            .iter()
            .zip(&self.target)
            .map(|(row, target)| {
                row.iter()
                    .zip(&values)
                    .map(|(coefficient, value)| coefficient * value)
                    .sum::<f64>()
                    - target
            })
            .collect())
    }

    fn jacobian(&self, variables: &[VariableValue]) -> Result<Vec<LocalJacobian>, EvaluationError> {
        let values = self.scalar_values(variables)?;
        Ok((0..values.len())
            .map(|column| {
                LocalJacobian::new(
                    self.matrix.len(),
                    1,
                    self.matrix.iter().map(|row| row[column]).collect(),
                )
            })
            .collect())
    }
}

pub fn add_affine_residual(
    problem: &mut Problem,
    source: SourceConstraintId,
    variables: Vec<VariableId>,
    matrix: Vec<Vec<f64>>,
    target: Vec<f64>,
    scale: f64,
) {
    let rows = matrix.len();
    let audit_rows = (0..rows)
        .map(|row| audit_row(format!("affine row {row}"), "scalar variables"))
        .collect();
    problem
        .add_residual(
            ResidualBlock::new(
                source,
                ResidualCategory::Hard,
                variables,
                rows,
                vec![scale; rows],
                audit_rows,
                AffineScalars { matrix, target },
            )
            .unwrap(),
        )
        .unwrap();
}

#[derive(Clone, Copy, Debug)]
pub struct Similarity {
    pub scale: f64,
    pub rotation: f64,
    pub translation: [f64; 2],
}

impl Similarity {
    pub fn apply(self, point: [f64; 2]) -> [f64; 2] {
        let (sin, cos) = self.rotation.sin_cos();
        [
            self.translation[0] + self.scale * (cos * point[0] - sin * point[1]),
            self.translation[1] + self.scale * (sin * point[0] + cos * point[1]),
        ]
    }

    pub fn inverse(self, point: [f64; 2]) -> [f64; 2] {
        let translated = [
            (point[0] - self.translation[0]) / self.scale,
            (point[1] - self.translation[1]) / self.scale,
        ];
        let (sin, cos) = self.rotation.sin_cos();
        [
            cos * translated[0] + sin * translated[1],
            -sin * translated[0] + cos * translated[1],
        ]
    }
}

#[derive(Clone, Copy, Debug)]
pub struct CircleGeometry {
    pub centers: [[f64; 2]; 2],
    pub radii: [f64; 2],
    pub scale: f64,
    pub expected: [f64; 2],
}

#[derive(Clone, Copy, Debug)]
pub struct CirclePair {
    pub centers: [[f64; 2]; 2],
    pub radii: [f64; 2],
}

impl ResidualEvaluator for CirclePair {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        let [VariableValue::Vec2(point)] = variables else {
            return Err(EvaluationError::invalid_geometry(
                "circle pair expected one Vec2 block",
            ));
        };
        Ok(self
            .centers
            .iter()
            .zip(self.radii)
            .map(|(center, radius)| (point[0] - center[0]).hypot(point[1] - center[1]) - radius)
            .collect())
    }

    fn jacobian(&self, variables: &[VariableValue]) -> Result<Vec<LocalJacobian>, EvaluationError> {
        let [VariableValue::Vec2(point)] = variables else {
            return Err(EvaluationError::invalid_geometry(
                "circle pair expected one Vec2 block",
            ));
        };
        let mut values = Vec::with_capacity(4);
        for center in self.centers {
            let delta = [point[0] - center[0], point[1] - center[1]];
            let distance = delta[0].hypot(delta[1]);
            if distance <= 1.0e-14 * self.radii[0].max(self.radii[1]).max(1.0) {
                return Err(EvaluationError::invalid_geometry(
                    "circle derivative is undefined at a center",
                ));
            }
            values.extend([delta[0] / distance, delta[1] / distance]);
        }
        Ok(vec![LocalJacobian::new(2, 2, values)])
    }
}

pub fn circle_fixture(
    transform: Similarity,
    normalized_perturbation: [f64; 2],
) -> (Problem, VariableId, CircleGeometry) {
    let base_centers = [[-2.0, 0.0], [2.0, 0.0]];
    let base_expected = [0.5, 3.0];
    let base_initial = [
        base_expected[0] + normalized_perturbation[0],
        base_expected[1] + normalized_perturbation[1],
    ];
    let base_radii = base_centers
        .map(|center| (base_expected[0] - center[0]).hypot(base_expected[1] - center[1]));
    let geometry = CircleGeometry {
        centers: base_centers.map(|center| transform.apply(center)),
        radii: base_radii.map(|radius| radius * transform.scale),
        scale: transform.scale,
        expected: transform.apply(base_expected),
    };

    let mut problem = Problem::new();
    let point = problem.add_variable(
        VariableBlock::vec2(
            transform.apply(base_initial),
            [transform.scale, transform.scale],
        )
        .unwrap(),
    );
    let source = add_source(&mut problem, "two-circle constructed solution");
    problem
        .add_residual(
            ResidualBlock::new(
                source,
                ResidualCategory::Hard,
                vec![point],
                2,
                vec![transform.scale; 2],
                vec![
                    audit_row("(distance(P, A) - radius_a) / scale", "P,A"),
                    audit_row("(distance(P, B) - radius_b) / scale", "P,B"),
                ],
                CirclePair {
                    centers: geometry.centers,
                    radii: geometry.radii,
                },
            )
            .unwrap(),
        )
        .unwrap();
    (problem, point, geometry)
}

pub fn normalized_circle_residuals(geometry: CircleGeometry, point: [f64; 2]) -> [f64; 2] {
    std::array::from_fn(|index| {
        let center = geometry.centers[index];
        ((point[0] - center[0]).hypot(point[1] - center[1]) - geometry.radii[index])
            / geometry.scale
    })
}

#[derive(Clone, Copy, Debug)]
pub struct MixedKinds {
    pub target: [f64; 2],
}

impl ResidualEvaluator for MixedKinds {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        let [
            VariableValue::Scalar(scalar),
            VariableValue::Vec2(vector),
            VariableValue::Pose2(pose),
        ] = variables
        else {
            return Err(EvaluationError::invalid_geometry(
                "mixed fixture expected Scalar, Vec2, and Pose2",
            ));
        };
        Ok(vec![
            scalar + vector[0] + pose[0] + pose[2].cos() - self.target[0],
            vector[1] + pose[1] + pose[2].sin() - self.target[1],
        ])
    }

    fn jacobian(&self, variables: &[VariableValue]) -> Result<Vec<LocalJacobian>, EvaluationError> {
        let [
            VariableValue::Scalar(_),
            VariableValue::Vec2(_),
            VariableValue::Pose2(pose),
        ] = variables
        else {
            return Err(EvaluationError::invalid_geometry(
                "mixed fixture expected Scalar, Vec2, and Pose2",
            ));
        };
        Ok(vec![
            LocalJacobian::new(2, 1, vec![1.0, 0.0]),
            LocalJacobian::new(2, 2, vec![1.0, 0.0, 0.0, 1.0]),
            LocalJacobian::new(
                2,
                3,
                vec![1.0, 0.0, -pose[2].sin(), 0.0, 1.0, pose[2].cos()],
            ),
        ])
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ScalarQuadratic(pub f64);

impl ResidualEvaluator for ScalarQuadratic {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        let [VariableValue::Scalar(value)] = variables else {
            return Err(EvaluationError::invalid_geometry(
                "quadratic fixture expected one scalar",
            ));
        };
        Ok(vec![value * value - self.0])
    }

    fn jacobian(&self, variables: &[VariableValue]) -> Result<Vec<LocalJacobian>, EvaluationError> {
        let [VariableValue::Scalar(value)] = variables else {
            return Err(EvaluationError::invalid_geometry(
                "quadratic fixture expected one scalar",
            ));
        };
        Ok(vec![LocalJacobian::new(1, 1, vec![2.0 * value])])
    }
}

pub fn assert_report_finite(report: &SolveReport) {
    let report_values = [
        report.hard_residual_max,
        report.hard_residual_l2,
        report.rank_relative_tolerance,
        report.rank_threshold,
    ];
    assert!(report_values.into_iter().all(f64::is_finite), "{report:#?}");
    assert!(
        report
            .accepted_state
            .ambient()
            .iter()
            .chain(&report.singular_values)
            .all(|value| value.is_finite()),
        "{report:#?}"
    );
    for block in report.accepted_state.layout().blocks() {
        assert!(block.step_scales.iter().all(|value| value.is_finite()));
    }
    for record in &report.trace.records {
        let values = [
            record.cost_before,
            record.trial_cost,
            record.cost,
            record.damping,
            record.actual_reduction,
            record.predicted_reduction,
            record.reduction_ratio,
            record.normalized_step_max,
        ];
        assert!(values.into_iter().all(f64::is_finite), "{record:#?}");
    }
    for source in &report.audit.sources {
        for row in &source.rows {
            assert!(row.scale.is_finite() && row.scale > 0.0, "{row:#?}");
            assert!(row.raw_residual.is_finite(), "{row:#?}");
            assert!(row.normalized_residual.is_finite(), "{row:#?}");
            for variable in &row.incident_variables {
                assert!(
                    variable
                        .value
                        .ambient_values()
                        .iter()
                        .all(|value| value.is_finite()),
                    "{variable:#?}"
                );
            }
        }
    }
}

pub fn assert_trace_invariants(report: &SolveReport, returned_cost: f64) {
    let Some(first) = report.trace.records.first() else {
        assert_close(returned_cost, 0.0, "empty trace returned cost");
        return;
    };
    let mut prior_accepted_cost = first.cost_before;
    for (index, record) in report.trace.records.iter().enumerate() {
        assert_eq!(record.iteration, index + 1);
        assert_close(
            record.cost_before,
            prior_accepted_cost,
            "record starts from prior accepted cost",
        );
        assert_close(
            record.actual_reduction,
            record.cost_before - record.trial_cost,
            "actual reduction matches trial transition",
        );
        if record.accepted {
            assert!(
                record.trial_valid,
                "accepted trial must be valid: {record:#?}"
            );
            assert!(
                record.cost
                    <= record.cost_before + trace_tolerance(record.cost, record.cost_before),
                "accepted objective increased: {record:#?}"
            );
            assert_close(
                record.cost,
                record.trial_cost,
                "accepted cost is trial cost",
            );
        } else {
            assert_close(
                record.cost,
                record.cost_before,
                "rejected trial retains accepted cost",
            );
        }
        prior_accepted_cost = record.cost;
    }
    assert_close(
        returned_cost,
        prior_accepted_cost,
        "returned state has last accepted objective",
    );
}

fn assert_close(actual: f64, expected: f64, context: &str) {
    let tolerance = trace_tolerance(actual, expected);
    assert!(
        (actual - expected).abs() <= tolerance,
        "{context}: actual={actual:e}, expected={expected:e}, tolerance={tolerance:e}"
    );
}

fn trace_tolerance(first: f64, second: f64) -> f64 {
    TRACE_COST_TOLERANCE_FACTOR * f64::EPSILON * (1.0 + first.abs().max(second.abs()))
}
