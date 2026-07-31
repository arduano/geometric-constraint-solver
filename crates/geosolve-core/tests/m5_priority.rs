use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use geosolve_core::{
    AuditBinding, CancellationToken, CoreError, EvaluationError, LinearSolveBackendPolicy,
    LocalJacobian, OperationControl, OperationController, OperationOutcome, PrioritySolveBackend,
    PrioritySolveScope, Problem, ResidualBlock, ResidualCategory, ResidualEvaluator,
    ResidualRowAudit, SecondaryStatus, SolveTermination, SolverConfig, SourceConstraint,
    VariableBlock, VariableId, VariableValue,
};

#[derive(Clone, Debug)]
struct CircleDistance {
    radius: f64,
    minimum_x: Option<f64>,
    invalid_evaluations: Option<Arc<AtomicUsize>>,
}

impl CircleDistance {
    fn unrestricted(radius: f64) -> Self {
        Self {
            radius,
            minimum_x: None,
            invalid_evaluations: None,
        }
    }

    fn limited(radius: f64, minimum_x: f64, invalid_evaluations: Arc<AtomicUsize>) -> Self {
        Self {
            radius,
            minimum_x: Some(minimum_x),
            invalid_evaluations: Some(invalid_evaluations),
        }
    }

    fn point(&self, variables: &[VariableValue]) -> Result<[f64; 2], EvaluationError> {
        let [VariableValue::Vec2(point)] = variables else {
            return Err(EvaluationError::invalid_geometry(
                "circle fixture expected one Vec2",
            ));
        };
        if self.minimum_x.is_some_and(|minimum| point[0] < minimum) {
            if let Some(count) = &self.invalid_evaluations {
                count.fetch_add(1, Ordering::Relaxed);
            }
            return Err(EvaluationError::invalid_geometry(
                "circle fixture left its valid branch",
            ));
        }
        Ok(*point)
    }
}

impl ResidualEvaluator for CircleDistance {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        let [x, y] = self.point(variables)?;
        let norm = x.hypot(y);
        if norm == 0.0 {
            return Err(EvaluationError::invalid_geometry(
                "circle fixture has no radial direction",
            ));
        }
        Ok(vec![norm - self.radius])
    }

    fn jacobian(&self, variables: &[VariableValue]) -> Result<Vec<LocalJacobian>, EvaluationError> {
        let [x, y] = self.point(variables)?;
        let norm = x.hypot(y);
        if norm == 0.0 {
            return Err(EvaluationError::invalid_geometry(
                "circle fixture has no radial direction",
            ));
        }
        Ok(vec![LocalJacobian::new(1, 2, vec![x / norm, y / norm])])
    }
}

#[derive(Clone, Copy, Debug)]
struct PointTarget([f64; 2]);

impl ResidualEvaluator for PointTarget {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        let [VariableValue::Vec2(point)] = variables else {
            return Err(EvaluationError::invalid_geometry(
                "point target fixture expected one Vec2",
            ));
        };
        Ok(vec![point[0] - self.0[0], point[1] - self.0[1]])
    }

    fn jacobian(&self, variables: &[VariableValue]) -> Result<Vec<LocalJacobian>, EvaluationError> {
        let [VariableValue::Vec2(_)] = variables else {
            return Err(EvaluationError::invalid_geometry(
                "point target fixture expected one Vec2",
            ));
        };
        Ok(vec![LocalJacobian::new(2, 2, vec![1.0, 0.0, 0.0, 1.0])])
    }
}

#[derive(Clone, Copy, Debug)]
struct ScalarTarget(f64);

impl ResidualEvaluator for ScalarTarget {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        let [VariableValue::Scalar(value)] = variables else {
            return Err(EvaluationError::invalid_geometry(
                "scalar target fixture expected one scalar",
            ));
        };
        Ok(vec![value - self.0])
    }

    fn jacobian(&self, variables: &[VariableValue]) -> Result<Vec<LocalJacobian>, EvaluationError> {
        let [VariableValue::Scalar(_)] = variables else {
            return Err(EvaluationError::invalid_geometry(
                "scalar target fixture expected one scalar",
            ));
        };
        Ok(vec![LocalJacobian::new(1, 1, vec![1.0])])
    }
}

#[derive(Clone, Copy, Debug)]
struct CoordinateTarget {
    coordinate: usize,
    target: f64,
}

impl ResidualEvaluator for CoordinateTarget {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        let [VariableValue::Vec2(point)] = variables else {
            return Err(EvaluationError::invalid_geometry(
                "coordinate target fixture expected one Vec2",
            ));
        };
        Ok(vec![point[self.coordinate] - self.target])
    }

    fn jacobian(&self, variables: &[VariableValue]) -> Result<Vec<LocalJacobian>, EvaluationError> {
        let [VariableValue::Vec2(_)] = variables else {
            return Err(EvaluationError::invalid_geometry(
                "coordinate target fixture expected one Vec2",
            ));
        };
        let mut jacobian = vec![0.0; 2];
        jacobian[self.coordinate] = 1.0;
        Ok(vec![LocalJacobian::new(1, 2, jacobian)])
    }
}

#[derive(Clone, Copy, Debug)]
struct ScalarDifference;

impl ResidualEvaluator for ScalarDifference {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        let [VariableValue::Scalar(first), VariableValue::Scalar(second)] = variables else {
            return Err(EvaluationError::invalid_geometry(
                "difference fixture expected two scalars",
            ));
        };
        Ok(vec![first - second])
    }

    fn jacobian(&self, variables: &[VariableValue]) -> Result<Vec<LocalJacobian>, EvaluationError> {
        let [VariableValue::Scalar(_), VariableValue::Scalar(_)] = variables else {
            return Err(EvaluationError::invalid_geometry(
                "difference fixture expected two scalars",
            ));
        };
        Ok(vec![
            LocalJacobian::new(1, 1, vec![1.0]),
            LocalJacobian::new(1, 1, vec![-1.0]),
        ])
    }
}

#[derive(Clone, Copy, Debug)]
struct NonFiniteSecondary;

impl ResidualEvaluator for NonFiniteSecondary {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        let [VariableValue::Scalar(_)] = variables else {
            return Err(EvaluationError::invalid_geometry(
                "non-finite fixture expected one scalar",
            ));
        };
        Ok(vec![f64::NAN])
    }

    fn jacobian(&self, variables: &[VariableValue]) -> Result<Vec<LocalJacobian>, EvaluationError> {
        let [VariableValue::Scalar(_)] = variables else {
            return Err(EvaluationError::invalid_geometry(
                "non-finite fixture expected one scalar",
            ));
        };
        Ok(vec![LocalJacobian::new(1, 1, vec![1.0])])
    }
}

#[derive(Clone, Copy, Debug)]
struct MixedSaddle;

impl ResidualEvaluator for MixedSaddle {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        let [VariableValue::Vec2([x, y])] = variables else {
            return Err(EvaluationError::invalid_geometry(
                "mixed saddle fixture expected one Vec2",
            ));
        };
        Ok(vec![1.0 + x * x + y * y - 4.0 * x * y])
    }

    fn jacobian(&self, variables: &[VariableValue]) -> Result<Vec<LocalJacobian>, EvaluationError> {
        let [VariableValue::Vec2([x, y])] = variables else {
            return Err(EvaluationError::invalid_geometry(
                "mixed saddle fixture expected one Vec2",
            ));
        };
        Ok(vec![LocalJacobian::new(
            1,
            2,
            vec![2.0 * x - 4.0 * y, 2.0 * y - 4.0 * x],
        )])
    }
}

fn row(template: &str) -> ResidualRowAudit {
    ResidualRowAudit::new(
        template,
        vec![AuditBinding::new("point", "synthetic point")],
        "model-unit",
    )
}

fn source(problem: &mut Problem, label: &str) -> geosolve_core::SourceConstraintId {
    problem.add_source(SourceConstraint::new(label).unwrap())
}

fn add_circle<E: ResidualEvaluator + 'static>(
    problem: &mut Problem,
    point: VariableId,
    scale: f64,
    evaluator: E,
) {
    let source_id = source(problem, "hard circle");
    problem
        .add_residual(
            ResidualBlock::new(
                source_id,
                ResidualCategory::Hard,
                vec![point],
                1,
                vec![scale],
                vec![row("norm(point) - radius")],
                evaluator,
            )
            .unwrap(),
        )
        .unwrap();
}

fn add_temporary_circle(problem: &mut Problem, point: VariableId, radius: f64, scale: f64) {
    let source_id = source(problem, "temporary circle");
    problem
        .add_residual(
            ResidualBlock::new(
                source_id,
                ResidualCategory::Temporary,
                vec![point],
                1,
                vec![scale],
                vec![row("norm(point) - radius")],
                CircleDistance::unrestricted(radius),
            )
            .unwrap(),
        )
        .unwrap();
}

fn add_point_target(
    problem: &mut Problem,
    point: VariableId,
    category: ResidualCategory,
    target: [f64; 2],
    scale: f64,
) {
    let source_id = source(problem, &format!("{category:?} point target"));
    problem
        .add_residual(
            ResidualBlock::new(
                source_id,
                category,
                vec![point],
                2,
                vec![scale, scale],
                vec![row("point.x - target.x"), row("point.y - target.y")],
                PointTarget(target),
            )
            .unwrap(),
        )
        .unwrap();
}

fn add_scalar_target(
    problem: &mut Problem,
    variable: VariableId,
    category: ResidualCategory,
    target: f64,
    scale: f64,
) {
    let source_id = source(problem, &format!("{category:?} scalar target"));
    problem
        .add_residual(
            ResidualBlock::new(
                source_id,
                category,
                vec![variable],
                1,
                vec![scale],
                vec![row("scalar - target")],
                ScalarTarget(target),
            )
            .unwrap(),
        )
        .unwrap();
}

fn add_coordinate_target(
    problem: &mut Problem,
    point: VariableId,
    category: ResidualCategory,
    coordinate: usize,
    target: f64,
) {
    let source_id = source(
        problem,
        &format!("{category:?} coordinate {coordinate} target"),
    );
    problem
        .add_residual(
            ResidualBlock::new(
                source_id,
                category,
                vec![point],
                1,
                vec![1.0],
                vec![row("point coordinate - target")],
                CoordinateTarget { coordinate, target },
            )
            .unwrap(),
        )
        .unwrap();
}

fn point(problem: &Problem, variable: VariableId) -> [f64; 2] {
    let VariableValue::Vec2(point) = problem.variable(variable).unwrap().value() else {
        panic!("expected Vec2")
    };
    point
}

fn scalar(problem: &Problem, variable: VariableId) -> f64 {
    let VariableValue::Scalar(value) = problem.variable(variable).unwrap().value() else {
        panic!("expected scalar")
    };
    value
}

fn assert_normalized_point(actual: [f64; 2], expected: [f64; 2], scale: f64) {
    assert!(
        (actual[0] - expected[0]).hypot(actual[1] - expected[1]) / scale <= 1.0e-8,
        "actual={actual:?} expected={expected:?} scale={scale:e}"
    );
}

fn audited_category_cost(report: &geosolve_core::SolveReport, category: ResidualCategory) -> f64 {
    report
        .audit
        .sources
        .iter()
        .flat_map(|source| &source.rows)
        .filter(|row| row.category == category)
        .map(|row| 0.5 * row.normalized_residual * row.normalized_residual)
        .sum()
}

fn assert_cost_matches(actual: f64, expected: f64) {
    let tolerance = 32.0 * f64::EPSILON * actual.abs().max(expected.abs());
    assert!(
        (actual - expected).abs() <= tolerance,
        "actual={actual:e} expected={expected:e} tolerance={tolerance:e}"
    );
}

#[test]
fn temporary_point_target_finds_the_known_nearest_circle_point_at_all_scales() {
    for scale in [1.0e-6, 1.0, 1.0e6] {
        let mut problem = Problem::new();
        let variable = problem
            .add_variable(VariableBlock::vec2([0.8 * scale, 0.1 * scale], [scale, scale]).unwrap());
        add_circle(
            &mut problem,
            variable,
            scale,
            CircleDistance::unrestricted(scale),
        );
        add_point_target(
            &mut problem,
            variable,
            ResidualCategory::Temporary,
            [2.0 * scale, scale],
            scale,
        );
        assert!(problem.check_jacobians(1.0e-5).unwrap().all_within(1.0e-6));

        let report = problem.solve(SolverConfig::default()).unwrap();
        assert_eq!(
            report.termination,
            SolveTermination::Converged,
            "{scale:e}: {report:#?}"
        );
        assert!(report.hard_residuals_validated);
        assert!(report.hard_residual_max <= 1.0e-9);
        assert_eq!((report.rank, report.local_degrees_of_freedom), (1, 1));
        assert!(report.trace.records.iter().any(|record| record.accepted));
        let inverse_norm = 1.0 / 5.0_f64.sqrt();
        assert_normalized_point(
            point(&problem, variable),
            [2.0 * scale * inverse_norm, scale * inverse_norm],
            scale,
        );
        let [priority] = report.priority_solves.as_slice() else {
            panic!("expected one priority report: {report:#?}")
        };
        assert_eq!(priority.category, ResidualCategory::Temporary);
        assert_eq!(priority.termination, SolveTermination::Converged);
        assert!(priority.initial_cost.unwrap() > priority.final_cost.unwrap());
    }
}

#[test]
fn preference_alone_restores_a_nearby_point_on_an_underconstrained_circle() {
    let mut problem = Problem::new();
    let variable = problem
        .add_variable(VariableBlock::vec2([0.2_f64.cos(), 0.2_f64.sin()], [1.0, 1.0]).unwrap());
    add_circle(
        &mut problem,
        variable,
        1.0,
        CircleDistance::unrestricted(1.0),
    );
    let preferred = [0.45_f64.cos(), 0.45_f64.sin()];
    add_point_target(
        &mut problem,
        variable,
        ResidualCategory::Preference,
        preferred,
        1.0,
    );

    let report = problem.solve(SolverConfig::default()).unwrap();
    assert_eq!(
        report.termination,
        SolveTermination::Converged,
        "{report:#?}"
    );
    assert_eq!(report.local_degrees_of_freedom, 1);
    assert!(report.hard_residual_max <= 1.0e-9);
    assert_normalized_point(point(&problem, variable), preferred, 1.0);
    assert_eq!(report.priority_solves.len(), 1);
    assert_eq!(
        report.priority_solves[0].category,
        ResidualCategory::Preference
    );
}

#[test]
fn constant_temporary_circle_objective_leaves_the_tangent_for_preference() {
    let mut problem = Problem::new();
    let variable = problem.add_variable(VariableBlock::vec2([1.0, 0.0], [1.0, 1.0]).unwrap());
    add_circle(
        &mut problem,
        variable,
        1.0,
        CircleDistance::unrestricted(1.0),
    );
    add_point_target(
        &mut problem,
        variable,
        ResidualCategory::Temporary,
        [0.0, 0.0],
        1.0,
    );
    add_point_target(
        &mut problem,
        variable,
        ResidualCategory::Preference,
        [0.0, 1.0],
        1.0,
    );

    let report = problem.solve(SolverConfig::default()).unwrap();
    assert_eq!(
        report.termination,
        SolveTermination::Converged,
        "{report:#?}"
    );
    assert_normalized_point(point(&problem, variable), [0.0, 1.0], 1.0);
    assert!(report.hard_residual_max <= 1.0e-9);
    let temporary = report
        .priority_solves
        .iter()
        .find(|item| item.category == ResidualCategory::Temporary)
        .unwrap();
    let preference = report
        .priority_solves
        .iter()
        .find(|item| item.category == ResidualCategory::Preference)
        .unwrap();
    assert_cost_matches(temporary.initial_cost.unwrap(), 0.5);
    assert_cost_matches(temporary.final_cost.unwrap(), 0.5);
    assert_cost_matches(preference.attained_temporary_cost.unwrap(), 0.5);
    assert_cost_matches(
        temporary.final_cost.unwrap(),
        audited_category_cost(&report, ResidualCategory::Temporary),
    );
    assert_cost_matches(
        preference.final_cost.unwrap(),
        audited_category_cost(&report, ResidualCategory::Preference),
    );
    assert_eq!(
        report.iterations,
        report.trace.records.len()
            + report
                .priority_solves
                .iter()
                .map(|item| item.iterations)
                .sum::<usize>()
    );
    assert!(report.iterations > report.trace.records.len());
}

#[test]
fn positive_temporary_level_keeps_a_separable_preference_movable_and_bounded() {
    let mut problem = Problem::new();
    let variable = problem.add_variable(VariableBlock::vec2([0.0, -3.0], [1.0, 1.0]).unwrap());
    add_coordinate_target(&mut problem, variable, ResidualCategory::Hard, 0, 0.0);
    add_coordinate_target(&mut problem, variable, ResidualCategory::Temporary, 0, 1.0);
    add_coordinate_target(&mut problem, variable, ResidualCategory::Preference, 1, 2.0);

    let OperationOutcome::Completed {
        value: report,
        report: work,
    } = problem
        .solve_controlled(SolverConfig::default(), OperationControl::unlimited())
        .unwrap()
    else {
        panic!("unlimited priority solve was interrupted")
    };
    assert_eq!(
        report.termination,
        SolveTermination::Converged,
        "{report:#?}"
    );
    assert_normalized_point(point(&problem, variable), [0.0, 2.0], 1.0);
    let temporary = report
        .priority_solves
        .iter()
        .find(|priority| priority.category == ResidualCategory::Temporary)
        .unwrap();
    let preference = report
        .priority_solves
        .iter()
        .find(|priority| priority.category == ResidualCategory::Preference)
        .unwrap();
    assert_cost_matches(temporary.final_cost.unwrap(), 0.5);
    assert_cost_matches(preference.attained_temporary_cost.unwrap(), 0.5);
    assert!(preference.final_cost.unwrap() <= 1.0e-24, "{preference:#?}");
    assert!(
        preference
            .protected_temporary
            .iter()
            .all(|protected| protected.preserved),
        "{preference:#?}"
    );
    assert!(
        work.consumed.factorizations <= 64 && work.consumed.nonlinear_iterations <= 64,
        "{work:#?}"
    );
}

#[test]
fn singleton_optional_refinement_exhaustion_retains_the_certified_baseline() {
    let mut problem = Problem::new();
    let variable = problem.add_variable(VariableBlock::vec2([0.0, -3.0], [1.0, 1.0]).unwrap());
    add_coordinate_target(&mut problem, variable, ResidualCategory::Hard, 0, 0.0);
    add_coordinate_target(&mut problem, variable, ResidualCategory::Temporary, 0, 1.0);
    add_coordinate_target(&mut problem, variable, ResidualCategory::Preference, 1, 2.0);

    let mut unlimited = problem.clone();
    let OperationOutcome::Completed {
        value: expected,
        report: unlimited_work,
    } = unlimited
        .solve_controlled(SolverConfig::default(), OperationControl::unlimited())
        .unwrap()
    else {
        panic!("unlimited singleton priority solve was interrupted")
    };

    let mut limits = geosolve_core::OperationLimits::unlimited();
    limits.document_dependency_items = 2;
    let OperationOutcome::Completed {
        value: report,
        report: bounded_work,
    } = problem
        .solve_controlled(
            SolverConfig::default(),
            OperationControl::new(CancellationToken::default(), limits),
        )
        .unwrap()
    else {
        panic!("optional singleton refinement exhaustion invalidated its certified baseline")
    };

    assert_eq!(
        report.termination,
        SolveTermination::Converged,
        "{report:#?}"
    );
    assert_normalized_point(point(&problem, variable), [0.0, 2.0], 1.0);
    assert_eq!(
        report.accepted_state.ambient(),
        expected.accepted_state.ambient()
    );
    let preference = report
        .priority_solves
        .iter()
        .find(|priority| priority.category == ResidualCategory::Preference)
        .unwrap();
    assert_eq!(preference.status, SecondaryStatus::Acceptable);
    assert!(
        preference
            .protected_temporary
            .iter()
            .all(|protected| protected.preserved),
        "{preference:#?}"
    );
    assert_eq!(bounded_work.consumed.document_dependency_items, 1);
    assert!(
        unlimited_work.consumed.document_dependency_items
            > bounded_work.consumed.document_dependency_items,
        "{unlimited_work:#?}\n{bounded_work:#?}"
    );
}

#[test]
fn positive_temporary_exact_row_baseline_is_optimized_before_scalar_fallback() {
    let mut problem = Problem::new();
    let variable = problem.add_variable(VariableBlock::vec2([0.0, -3.0], [1.0, 1.0]).unwrap());
    add_coordinate_target(&mut problem, variable, ResidualCategory::Hard, 0, 0.0);
    add_coordinate_target(&mut problem, variable, ResidualCategory::Temporary, 0, 1.0);
    add_coordinate_target(
        &mut problem,
        variable,
        ResidualCategory::Preference,
        1,
        -1.0,
    );
    add_coordinate_target(&mut problem, variable, ResidualCategory::Preference, 1, 1.0);

    let OperationOutcome::Completed {
        value: report,
        report: work,
    } = problem
        .solve_controlled(SolverConfig::default(), OperationControl::unlimited())
        .unwrap()
    else {
        panic!("unlimited priority solve was interrupted")
    };
    assert_eq!(
        report.termination,
        SolveTermination::Converged,
        "{report:#?}"
    );
    assert_normalized_point(point(&problem, variable), [0.0, 0.0], 1.0);
    let preference = report
        .priority_solves
        .iter()
        .find(|priority| priority.category == ResidualCategory::Preference)
        .unwrap();
    assert_eq!(preference.termination, SolveTermination::Converged);
    assert_eq!(preference.status, SecondaryStatus::Acceptable);
    assert_cost_matches(preference.attained_temporary_cost.unwrap(), 0.5);
    assert_cost_matches(preference.final_cost.unwrap(), 1.0);
    assert!(
        preference
            .protected_temporary
            .iter()
            .all(|protected| protected.preserved),
        "{preference:#?}"
    );
    assert!(
        work.consumed.factorizations <= 96 && work.consumed.nonlinear_iterations <= 64,
        "{report:#?}\n{work:#?}"
    );
}

#[test]
fn positive_temporary_baseline_hard_polish_exhaustion_retains_the_certified_state() {
    let mut problem = Problem::new();
    let variable = problem.add_variable(VariableBlock::scalar(5.0e-7, 1.0).unwrap());
    add_scalar_target(&mut problem, variable, ResidualCategory::Hard, 0.0, 1.0);
    add_scalar_target(
        &mut problem,
        variable,
        ResidualCategory::Temporary,
        1.0,
        1.0,
    );
    add_scalar_target(
        &mut problem,
        variable,
        ResidualCategory::Preference,
        0.0,
        1.0,
    );
    let config = SolverConfig {
        normalized_residual_tolerance: 1.0e-6,
        normalized_step_tolerance: 1.0e-6,
        ..SolverConfig::default()
    };
    let mut control = OperationControl::unlimited();
    control.limits.nonlinear_iterations = 14;

    let OperationOutcome::Completed {
        value: report,
        report: work,
    } = problem.solve_controlled(config, control).unwrap()
    else {
        panic!("baseline hard-polish exhaustion invalidated a certified positive-Temporary state")
    };

    assert_eq!(
        report.termination,
        SolveTermination::Converged,
        "{report:#?}"
    );
    assert!(report.hard_residuals_validated);
    assert!(report.hard_residual_max <= config.normalized_residual_tolerance);
    assert_eq!(scalar(&problem, variable).to_bits(), 5.0e-7_f64.to_bits());
    let temporary = report
        .priority_solves
        .iter()
        .find(|priority| priority.category == ResidualCategory::Temporary)
        .unwrap();
    let preference = report
        .priority_solves
        .iter()
        .find(|priority| priority.category == ResidualCategory::Preference)
        .unwrap();
    assert!(temporary.final_cost.is_some_and(|cost| cost > 0.0));
    assert!(matches!(
        preference.status,
        SecondaryStatus::Optimal | SecondaryStatus::Acceptable
    ));
    assert_eq!(work.consumed.nonlinear_iterations, 14);
    assert!(work.stopping_reason.is_none(), "{work:#?}");
}

#[test]
fn zero_cost_temporary_circle_manifold_allows_opposite_preference_at_all_scales() {
    for scale in [1.0e-6, 1.0, 1.0e6] {
        let mut problem = Problem::new();
        let variable =
            problem.add_variable(VariableBlock::vec2([scale, 0.0], [scale, scale]).unwrap());
        add_temporary_circle(&mut problem, variable, scale, scale);
        add_point_target(
            &mut problem,
            variable,
            ResidualCategory::Preference,
            [-scale, 0.0],
            scale,
        );

        let report = problem.solve(SolverConfig::default()).unwrap();
        assert_eq!(
            report.termination,
            SolveTermination::Converged,
            "{scale:e}: {report:#?}"
        );
        assert_normalized_point(point(&problem, variable), [-scale, 0.0], scale);
        let temporary = report
            .priority_solves
            .iter()
            .find(|item| item.category == ResidualCategory::Temporary)
            .unwrap();
        let preference = report
            .priority_solves
            .iter()
            .find(|item| item.category == ResidualCategory::Preference)
            .unwrap();
        assert_eq!(preference.attained_temporary_cost, Some(0.0));
        assert!(temporary.final_cost.unwrap() <= 5.0e-19, "{scale:e}");
        assert!(preference.final_cost.unwrap() <= 5.0e-17, "{scale:e}");
        assert_cost_matches(
            temporary.final_cost.unwrap(),
            audited_category_cost(&report, ResidualCategory::Temporary),
        );
    }
}

#[test]
fn zero_cost_temporary_row_space_blocks_a_conflicting_rank_deficient_preference() {
    let mut problem = Problem::new();
    let variable = problem.add_variable(VariableBlock::vec2([1.0, 0.0], [1.0, 1.0]).unwrap());
    add_circle(
        &mut problem,
        variable,
        1.0,
        CircleDistance::unrestricted(1.0),
    );
    add_point_target(
        &mut problem,
        variable,
        ResidualCategory::Temporary,
        [1.0, 0.0],
        1.0,
    );
    add_point_target(
        &mut problem,
        variable,
        ResidualCategory::Preference,
        [-1.0, 0.0],
        1.0,
    );

    let report = problem.solve(SolverConfig::default()).unwrap();
    assert_eq!(
        report.termination,
        SolveTermination::Converged,
        "{report:#?}"
    );
    assert_normalized_point(point(&problem, variable), [1.0, 0.0], 1.0);
    let temporary = report
        .priority_solves
        .iter()
        .find(|item| item.category == ResidualCategory::Temporary)
        .unwrap();
    let preference = report
        .priority_solves
        .iter()
        .find(|item| item.category == ResidualCategory::Preference)
        .unwrap();
    assert_eq!(temporary.final_cost, Some(0.0));
    assert_eq!(preference.attained_temporary_cost, Some(0.0));
    assert!(
        preference
            .protected_temporary
            .iter()
            .all(|protected| protected.preserved),
        "{preference:#?}"
    );
}

#[test]
fn zero_cost_temporary_jointly_polishes_hard_rows_without_losing_its_exact_target() {
    let mut problem = Problem::new();
    let selected = problem.add_variable(VariableBlock::scalar(1.0, 1.0).unwrap());
    let dependent = problem.add_variable(VariableBlock::scalar(1.0 + 5.0e-10, 1.0).unwrap());
    let hard_source = source(&mut problem, "marginal hard equality");
    problem
        .add_residual(
            ResidualBlock::new(
                hard_source,
                ResidualCategory::Hard,
                vec![selected, dependent],
                1,
                vec![1.0],
                vec![row("selected - dependent")],
                ScalarDifference,
            )
            .unwrap(),
        )
        .unwrap();
    add_scalar_target(
        &mut problem,
        selected,
        ResidualCategory::Temporary,
        1.0,
        1.0,
    );

    let OperationOutcome::Completed { value, report } = problem
        .solve_controlled(SolverConfig::default(), OperationControl::unlimited())
        .unwrap()
    else {
        panic!("zero-Temporary precision solve was interrupted")
    };

    assert_eq!(value.termination, SolveTermination::Converged, "{value:#?}");
    assert!(
        value.hard_residual_max <= 1.0e-12,
        "hard precision was not recovered: {value:#?}"
    );
    assert!((scalar(&problem, selected) - 1.0).abs() <= 1.0e-12);
    assert!((scalar(&problem, dependent) - 1.0).abs() <= 1.0e-12);
    let temporary = value
        .priority_solves
        .iter()
        .find(|priority| priority.category == ResidualCategory::Temporary)
        .unwrap();
    assert_eq!(temporary.termination, SolveTermination::Converged);
    assert!(
        temporary.final_cost.is_some_and(|cost| cost <= 1.0e-24),
        "{temporary:#?}"
    );
    assert!(
        report.consumed.nonlinear_iterations <= 8 && report.consumed.factorizations <= 16,
        "{report:#?}"
    );
}

#[test]
fn zero_temporary_precision_polish_exhaustion_retains_the_certified_baseline() {
    let fixture = || {
        let mut problem = Problem::new();
        let selected = problem.add_variable(VariableBlock::scalar(1.0, 1.0).unwrap());
        let dependent = problem.add_variable(VariableBlock::scalar(1.0 + 5.0e-10, 1.0).unwrap());
        let hard_source = source(&mut problem, "marginal hard equality");
        problem
            .add_residual(
                ResidualBlock::new(
                    hard_source,
                    ResidualCategory::Hard,
                    vec![selected, dependent],
                    1,
                    vec![1.0],
                    vec![row("selected - dependent")],
                    ScalarDifference,
                )
                .unwrap(),
            )
            .unwrap();
        add_scalar_target(
            &mut problem,
            selected,
            ResidualCategory::Temporary,
            1.0,
            1.0,
        );
        (problem, selected, dependent)
    };

    // SparsePreferred intentionally skips the dense-only optional precision
    // target path and therefore exposes the independently certified baseline
    // work needed before that path starts.
    let (mut baseline, _, _) = fixture();
    let sparse_config = SolverConfig {
        linear_solve_backend: LinearSolveBackendPolicy::SparsePreferred,
        ..SolverConfig::default()
    };
    let OperationOutcome::Completed {
        value: baseline_report,
        report: baseline_work,
    } = baseline
        .solve_controlled(sparse_config, OperationControl::unlimited())
        .unwrap()
    else {
        panic!("baseline solve was interrupted")
    };
    assert_eq!(
        baseline_report.termination,
        SolveTermination::Converged,
        "{baseline_report:#?}"
    );

    let (mut problem, selected, dependent) = fixture();
    let mut control = OperationControl::unlimited();
    control.limits.nonlinear_iterations = baseline_work.consumed.nonlinear_iterations;
    let dense_config = SolverConfig {
        linear_solve_backend: LinearSolveBackendPolicy::DenseOnly,
        ..SolverConfig::default()
    };
    let OperationOutcome::Completed {
        value: report,
        report: work,
    } = problem.solve_controlled(dense_config, control).unwrap()
    else {
        panic!("optional precision exhaustion invalidated its certified baseline")
    };

    assert_eq!(
        report.termination,
        SolveTermination::Converged,
        "{report:#?}"
    );
    assert!(report.hard_residuals_validated);
    assert!(report.hard_residual_max <= dense_config.normalized_residual_tolerance);
    assert!((scalar(&problem, selected) - 1.0).abs() <= 1.0e-9);
    assert!((scalar(&problem, dependent) - 1.0).abs() <= 1.0e-9);
    assert_eq!(
        work.consumed.nonlinear_iterations,
        baseline_work.consumed.nonlinear_iterations
    );
    assert!(work.stopping_reason.is_none(), "{work:#?}");
}

#[test]
fn near_zero_temporary_on_a_nonlinear_rank_deficient_manifold_has_bounded_work() {
    let mut problem = Problem::new();
    let initial_angle = 0.2_f64;
    let target_angle = 0.27_f64;
    let initial = [initial_angle.cos(), initial_angle.sin()];
    let target = [target_angle.cos(), target_angle.sin()];
    let variable = problem.add_variable(VariableBlock::vec2(initial, [1.0, 1.0]).unwrap());
    add_circle(
        &mut problem,
        variable,
        1.0,
        CircleDistance::unrestricted(1.0),
    );
    add_point_target(
        &mut problem,
        variable,
        ResidualCategory::Temporary,
        target,
        1.0,
    );
    add_point_target(
        &mut problem,
        variable,
        ResidualCategory::Preference,
        initial,
        1.0,
    );

    let OperationOutcome::Completed { value, report } = problem
        .solve_controlled(SolverConfig::default(), OperationControl::unlimited())
        .unwrap()
    else {
        panic!("unlimited priority solve was interrupted")
    };
    assert_eq!(value.termination, SolveTermination::Converged, "{value:#?}");
    assert!(value.hard_residual_max <= 1.0e-9);
    assert_normalized_point(point(&problem, variable), target, 1.0);
    assert!(
        value
            .priority_solves
            .iter()
            .flat_map(|priority| &priority.protected_temporary)
            .all(|protected| protected.preserved),
        "{value:#?}"
    );
    assert!(
        report.consumed.factorizations <= 20 && report.consumed.nonlinear_iterations <= 20,
        "{report:#?}"
    );
}

#[test]
fn temporary_strictly_dominates_a_conflicting_preference() {
    let mut problem = Problem::new();
    let variable = problem.add_variable(VariableBlock::vec2([1.0, 0.0], [1.0, 1.0]).unwrap());
    add_circle(
        &mut problem,
        variable,
        1.0,
        CircleDistance::unrestricted(1.0),
    );
    add_point_target(
        &mut problem,
        variable,
        ResidualCategory::Temporary,
        [0.0, 2.0],
        1.0,
    );
    add_point_target(
        &mut problem,
        variable,
        ResidualCategory::Preference,
        [1.0, 0.0],
        1.0,
    );

    let OperationOutcome::Completed {
        value: report,
        report: work,
    } = problem
        .solve_controlled(SolverConfig::default(), OperationControl::unlimited())
        .unwrap()
    else {
        panic!("unlimited priority solve was interrupted")
    };
    assert_eq!(report.termination, SolveTermination::Converged);
    assert!(report.hard_residual_max <= 1.0e-9);
    assert_normalized_point(point(&problem, variable), [0.0, 1.0], 1.0);
    assert_eq!(report.priority_solves.len(), 2);
    let temporary = &report.priority_solves[0];
    let preference = &report.priority_solves[1];
    assert_eq!(temporary.category, ResidualCategory::Temporary);
    assert_eq!(preference.category, ResidualCategory::Preference);
    assert!(temporary.initial_cost.unwrap() > temporary.final_cost.unwrap());
    assert_cost_matches(
        preference.attained_temporary_cost.unwrap(),
        temporary.final_cost.unwrap(),
    );
    assert!(preference.final_cost.unwrap() <= preference.initial_cost.unwrap());
    assert_cost_matches(
        temporary.final_cost.unwrap(),
        audited_category_cost(&report, ResidualCategory::Temporary),
    );
    assert!(
        work.consumed.factorizations <= 256 && work.consumed.nonlinear_iterations <= 256,
        "{work:#?}"
    );
}

#[test]
fn temporary_circle_target_escapes_a_strict_maximum() {
    let mut problem = Problem::new();
    let variable = problem.add_variable(VariableBlock::vec2([-1.0, 0.0], [1.0, 1.0]).unwrap());
    add_circle(
        &mut problem,
        variable,
        1.0,
        CircleDistance::unrestricted(1.0),
    );
    add_point_target(
        &mut problem,
        variable,
        ResidualCategory::Temporary,
        [2.0, 0.0],
        1.0,
    );

    let report = problem.solve(SolverConfig::default()).unwrap();
    assert_eq!(
        report.termination,
        SolveTermination::Converged,
        "{report:#?}"
    );
    assert_normalized_point(point(&problem, variable), [1.0, 0.0], 1.0);
    let [temporary] = report.priority_solves.as_slice() else {
        panic!("expected one Temporary report")
    };
    assert!(temporary.iterations > 1);
    assert!(temporary.initial_cost.unwrap() > temporary.final_cost.unwrap());
}

#[test]
fn mixed_direction_saddle_escapes_along_negative_curvature() {
    let mut problem = Problem::new();
    let variable = problem.add_variable(VariableBlock::vec2([0.0, 0.0], [1.0, 1.0]).unwrap());
    let source_id = source(&mut problem, "mixed saddle");
    problem
        .add_residual(
            ResidualBlock::new(
                source_id,
                ResidualCategory::Temporary,
                vec![variable],
                1,
                vec![1.0],
                vec![row("1 + x^2 + y^2 - 4*x*y")],
                MixedSaddle,
            )
            .unwrap(),
        )
        .unwrap();
    assert!(problem.check_jacobians(1.0e-5).unwrap().all_within(1.0e-6));

    let report = problem.solve(SolverConfig::default()).unwrap();
    assert_eq!(
        report.termination,
        SolveTermination::Converged,
        "{report:#?}"
    );
    let solved = point(&problem, variable);
    assert!(solved[0].hypot(solved[1]) > 0.5);
    assert!(report.priority_solves[0].initial_cost.unwrap() >= 0.5);
    assert!(report.priority_solves[0].final_cost.unwrap() <= 1.0e-18);
}

#[test]
fn true_constrained_circle_minimum_reports_converged() {
    let mut problem = Problem::new();
    let variable = problem.add_variable(VariableBlock::vec2([1.0, 0.0], [1.0, 1.0]).unwrap());
    add_circle(
        &mut problem,
        variable,
        1.0,
        CircleDistance::unrestricted(1.0),
    );
    add_point_target(
        &mut problem,
        variable,
        ResidualCategory::Temporary,
        [2.0, 0.0],
        1.0,
    );

    let report = problem.solve(SolverConfig::default()).unwrap();
    assert_eq!(
        report.termination,
        SolveTermination::Converged,
        "{report:#?}"
    );
    assert_normalized_point(point(&problem, variable), [1.0, 0.0], 1.0);
    assert_eq!(report.priority_solves.len(), 1);
    assert_eq!(
        report.priority_solves[0].termination,
        SolveTermination::Converged
    );
    assert_eq!(
        report.priority_solves[0].status,
        geosolve_core::SecondaryStatus::Acceptable
    );
}

#[test]
fn tiny_normalized_unconstrained_objective_is_not_ignored_at_any_scale() {
    for scale in [1.0e-6, 1.0, 1.0e6] {
        let mut problem = Problem::new();
        let variable = problem.add_variable(VariableBlock::scalar(0.0, scale).unwrap());
        let target = scale * 1.0e-8;
        add_scalar_target(
            &mut problem,
            variable,
            ResidualCategory::Temporary,
            target,
            scale,
        );

        let report = problem.solve(SolverConfig::default()).unwrap();
        assert_eq!(report.termination, SolveTermination::Converged, "{scale:e}");
        assert!(scalar(&problem, variable) != 0.0, "{scale:e}");
        assert!((scalar(&problem, variable) - target).abs() / scale <= 1.0e-12);
        let [temporary] = report.priority_solves.as_slice() else {
            panic!("expected one Temporary report")
        };
        assert!(temporary.initial_cost.unwrap() > temporary.final_cost.unwrap());
    }
}

#[test]
fn fully_constrained_hard_system_audits_but_does_not_follow_secondary_rows() {
    let mut problem = Problem::new();
    let variable = problem.add_variable(VariableBlock::vec2([-1.0, 1.0], [1.0, 1.0]).unwrap());
    add_point_target(
        &mut problem,
        variable,
        ResidualCategory::Hard,
        [0.25, -0.5],
        1.0,
    );
    add_point_target(
        &mut problem,
        variable,
        ResidualCategory::Temporary,
        [20.0, 30.0],
        1.0,
    );

    let report = problem.solve(SolverConfig::default()).unwrap();
    assert_eq!(report.termination, SolveTermination::Converged);
    assert_eq!(report.local_degrees_of_freedom, 0);
    assert!(report.hard_residual_max <= 1.0e-9);
    assert_normalized_point(point(&problem, variable), [0.25, -0.5], 1.0);
    assert_eq!(report.priority_solves.len(), 1);
    assert_eq!(report.priority_solves[0].iterations, 0);
    assert_eq!(
        report.priority_solves[0].initial_cost,
        report.priority_solves[0].final_cost
    );
    assert!(report.audit.sources.iter().all(|source| {
        source
            .rows
            .iter()
            .all(|row| row.evaluation_status == geosolve_core::AuditEvaluationStatus::Evaluated)
    }));
}

#[test]
fn fixed_only_secondary_rows_are_evaluated_without_a_step() {
    let mut problem = Problem::new();
    let variable = problem.add_variable(VariableBlock::vec2([3.0, 4.0], [1.0, 1.0]).unwrap());
    let fixed_source = source(&mut problem, "fixed point");
    let fixed_residual = problem
        .add_residual(
            ResidualBlock::fixed_variable(
                fixed_source,
                variable,
                VariableValue::Vec2([0.5, -0.25]),
                vec![1.0, 1.0],
                vec![row("point.x - fixed.x"), row("point.y - fixed.y")],
            )
            .unwrap(),
        )
        .unwrap();
    problem
        .declare_fixed_variable(variable, VariableValue::Vec2([0.5, -0.25]), fixed_residual)
        .unwrap();
    add_point_target(
        &mut problem,
        variable,
        ResidualCategory::Preference,
        [10.0, 20.0],
        1.0,
    );

    let report = problem.solve(SolverConfig::default()).unwrap();
    assert_eq!(report.termination, SolveTermination::Converged);
    assert_eq!(report.local_degrees_of_freedom, 0);
    assert_normalized_point(point(&problem, variable), [0.5, -0.25], 1.0);
    let [priority] = report.priority_solves.as_slice() else {
        panic!("expected one fixed-only priority report")
    };
    assert_eq!(priority.component_index, None);
    assert_eq!(priority.category, ResidualCategory::Preference);
    assert_eq!(priority.iterations, 0);
    assert_eq!(priority.initial_cost, priority.final_cost);
    assert!(priority.initial_cost.is_some());
}

#[test]
fn nonlinear_priority_trials_never_commit_an_invalid_hard_branch() {
    let invalid_evaluations = Arc::new(AtomicUsize::new(0));
    let mut problem = Problem::new();
    let variable = problem.add_variable(VariableBlock::vec2([1.0, 0.0], [1.0, 1.0]).unwrap());
    add_circle(
        &mut problem,
        variable,
        1.0,
        CircleDistance::limited(1.0, 0.5, Arc::clone(&invalid_evaluations)),
    );
    add_point_target(
        &mut problem,
        variable,
        ResidualCategory::Temporary,
        [0.0, 2.0],
        1.0,
    );

    let report = problem.solve(SolverConfig::default()).unwrap();
    assert_ne!(report.termination, SolveTermination::Converged);
    assert!(invalid_evaluations.load(Ordering::Relaxed) > 0);
    assert!(point(&problem, variable)[0] >= 0.5);
    assert!(report.hard_residuals_validated);
    assert!(report.hard_residual_max <= 1.0e-9);
    assert!(
        report
            .accepted_state
            .ambient()
            .iter()
            .all(|value| value.is_finite())
    );
}

#[test]
fn invalid_priority_component_does_not_block_a_healthy_component() {
    let mut problem = Problem::new();
    let invalid = problem.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap());
    let healthy = problem.add_variable(VariableBlock::scalar(-3.0, 1.0).unwrap());
    let invalid_source = source(&mut problem, "invalid temporary");
    problem
        .add_residual(
            ResidualBlock::new(
                invalid_source,
                ResidualCategory::Temporary,
                vec![invalid],
                1,
                vec![1.0],
                vec![row("non-finite temporary")],
                NonFiniteSecondary,
            )
            .unwrap(),
        )
        .unwrap();
    add_scalar_target(&mut problem, healthy, ResidualCategory::Temporary, 2.0, 1.0);
    add_scalar_target(
        &mut problem,
        invalid,
        ResidualCategory::Preference,
        1.0,
        1.0,
    );
    add_scalar_target(
        &mut problem,
        healthy,
        ResidualCategory::Preference,
        3.0,
        1.0,
    );

    let report = problem.solve(SolverConfig::default()).unwrap();
    assert_eq!(report.termination, SolveTermination::NumericalFailure);
    assert!((scalar(&problem, healthy) - 2.0).abs() <= 1.0e-12);
    assert_eq!(report.priority_solves.len(), 4);
    assert!(report.priority_solves.iter().any(|item| {
        item.termination == SolveTermination::NumericalFailure && item.final_cost.is_none()
    }));
    assert!(report.priority_solves.iter().any(|item| {
        item.termination == SolveTermination::Converged && item.final_cost == Some(0.0)
    }));
    let preference_reports: Vec<_> = report
        .priority_solves
        .iter()
        .filter(|item| item.category == ResidualCategory::Preference)
        .collect();
    assert_eq!(preference_reports.len(), 2);
    assert!(preference_reports.iter().any(|item| {
        item.termination == SolveTermination::Converged && item.attained_temporary_cost == Some(0.0)
    }));
}

#[test]
fn cross_component_secondary_incidence_is_optimized_as_one_group() {
    let mut problem = Problem::new();
    let first = problem.add_variable(VariableBlock::scalar(-2.0, 1.0).unwrap());
    let second = problem.add_variable(VariableBlock::scalar(3.0, 1.0).unwrap());
    let healthy = problem.add_variable(VariableBlock::scalar(-4.0, 1.0).unwrap());
    let temporary_source = source(&mut problem, "coupled temporary");
    problem
        .add_residual(
            ResidualBlock::new(
                temporary_source,
                ResidualCategory::Temporary,
                vec![first, second],
                1,
                vec![1.0],
                vec![row("first - second")],
                ScalarDifference,
            )
            .unwrap(),
        )
        .unwrap();
    add_scalar_target(&mut problem, healthy, ResidualCategory::Temporary, 2.0, 1.0);

    let report = problem.solve(SolverConfig::default()).unwrap();
    assert_eq!(report.termination, SolveTermination::Converged);
    assert!(report.hard_residuals_validated);
    assert!(report.hard_residual_max <= 1.0e-9);
    assert!((scalar(&problem, first) - 0.5).abs() <= 1.0e-9);
    assert!((scalar(&problem, second) - 0.5).abs() <= 1.0e-9);
    assert!((scalar(&problem, healthy) - 2.0).abs() <= 1.0e-12);
    let coupled = report
        .priority_solves
        .iter()
        .find(|item| item.component_indices.len() == 2)
        .unwrap();
    assert_eq!(coupled.component_index, None);
    assert_eq!(coupled.scope, PrioritySolveScope::Movable);
    assert_eq!(
        coupled.backend,
        Some(PrioritySolveBackend::DenseBlockNullspace)
    );
    assert_eq!(coupled.termination, SolveTermination::Converged);
    assert_eq!(coupled.final_cost, Some(0.0));
}

#[test]
fn coupled_baseline_hard_polish_exhaustion_retains_the_certified_state() {
    let mut problem = Problem::new();
    let first = problem.add_variable(VariableBlock::scalar(5.0e-7, 1.0).unwrap());
    let second = problem.add_variable(VariableBlock::scalar(1.0 + 5.0e-7, 1.0).unwrap());
    add_scalar_target(&mut problem, first, ResidualCategory::Hard, 0.0, 1.0);
    add_scalar_target(&mut problem, second, ResidualCategory::Hard, 1.0, 1.0);
    let temporary_source = source(&mut problem, "positive coupled temporary");
    problem
        .add_residual(
            ResidualBlock::new(
                temporary_source,
                ResidualCategory::Temporary,
                vec![first, second],
                1,
                vec![1.0],
                vec![row("first - second")],
                ScalarDifference,
            )
            .unwrap(),
        )
        .unwrap();
    add_scalar_target(&mut problem, first, ResidualCategory::Preference, 0.0, 1.0);
    let config = SolverConfig {
        normalized_residual_tolerance: 1.0e-6,
        normalized_step_tolerance: 1.0e-6,
        ..SolverConfig::default()
    };
    let mut control = OperationControl::unlimited();
    control.limits.nonlinear_iterations = 8;

    let OperationOutcome::Completed {
        value: report,
        report: work,
    } = problem.solve_controlled(config, control).unwrap()
    else {
        panic!("coupled baseline hard-polish exhaustion invalidated its certified state")
    };

    assert_eq!(
        report.termination,
        SolveTermination::Converged,
        "{report:#?}"
    );
    assert!(report.hard_residuals_validated);
    assert!(report.hard_residual_max <= config.normalized_residual_tolerance);
    assert_eq!(scalar(&problem, first).to_bits(), 5.0e-7_f64.to_bits());
    assert_eq!(
        scalar(&problem, second).to_bits(),
        (1.0 + 5.0e-7_f64).to_bits()
    );
    let coupled = report
        .priority_solves
        .iter()
        .find(|priority| {
            priority.category == ResidualCategory::Preference
                && priority.component_indices.len() == 2
        })
        .unwrap();
    assert_eq!(coupled.termination, SolveTermination::Converged);
    assert_eq!(coupled.status, SecondaryStatus::Acceptable);
    assert_eq!(work.consumed.nonlinear_iterations, 6);
    assert!(work.stopping_reason.is_none(), "{work:#?}");
}

#[test]
fn current_state_certification_rebuilds_evidence_without_moving_exact_zero_priorities() {
    let mut problem = Problem::new();
    let variable = problem.add_variable(VariableBlock::scalar(2.0, 1.0).unwrap());
    add_scalar_target(&mut problem, variable, ResidualCategory::Hard, 2.0, 1.0);
    add_scalar_target(
        &mut problem,
        variable,
        ResidualCategory::Temporary,
        2.0,
        1.0,
    );
    add_scalar_target(
        &mut problem,
        variable,
        ResidualCategory::Preference,
        2.0,
        1.0,
    );
    let before = problem.packed_state().unwrap();
    let mut controller = OperationController::new(OperationControl::unlimited());

    let report = problem
        .certify_current_state_with_controller(SolverConfig::default(), &mut controller)
        .unwrap()
        .expect("complete certification");

    assert_eq!(problem.packed_state().unwrap(), before);
    assert_eq!(report.accepted_state, before);
    assert_eq!(report.termination, SolveTermination::Converged);
    assert_eq!(report.iterations, 0);
    assert!(report.hard_residuals_validated);
    assert!(report.rank_is_valid);
    assert!(
        report
            .component_solves
            .iter()
            .all(|component| component.iterations == 0 && component.actual_backend.is_none())
    );
    assert_eq!(report.priority_solves.len(), 2);
    assert!(report.priority_solves.iter().all(|priority| {
        priority.iterations == 0
            && priority.final_cost == Some(0.0)
            && priority.status == SecondaryStatus::Optimal
    }));
    let preference = report
        .priority_solves
        .iter()
        .find(|priority| priority.category == ResidualCategory::Preference)
        .unwrap();
    assert_eq!(preference.protected_temporary.len(), 1);
    assert!(preference.protected_temporary[0].preserved);
}

#[test]
fn current_state_certification_rejects_unproved_movable_priority_without_motion() {
    let mut problem = Problem::new();
    let variable = problem.add_variable(VariableBlock::scalar(2.0, 1.0).unwrap());
    add_scalar_target(&mut problem, variable, ResidualCategory::Hard, 2.0, 1.0);
    add_scalar_target(
        &mut problem,
        variable,
        ResidualCategory::Preference,
        3.0,
        1.0,
    );
    let before = problem.packed_state().unwrap();
    let mut controller = OperationController::new(OperationControl::unlimited());

    let report = problem
        .certify_current_state_with_controller(SolverConfig::default(), &mut controller)
        .unwrap()
        .expect("complete rejection evidence");

    assert_eq!(problem.packed_state().unwrap(), before);
    assert_eq!(report.accepted_state, before);
    assert_eq!(report.termination, SolveTermination::Stalled);
    assert_eq!(report.preference_status, SecondaryStatus::Stalled);
    assert_eq!(report.iterations, 0);
}

#[test]
fn current_state_certification_rejects_required_canonicalization_bitwise() {
    let mut problem = Problem::new();
    let variable = problem.add_variable(VariableBlock::scalar(1.0, 1.0).unwrap());
    let source_id = source(&mut problem, "fixed scalar");
    let residual = problem
        .add_residual(
            ResidualBlock::fixed_variable(
                source_id,
                variable,
                VariableValue::Scalar(0.0),
                vec![1.0],
                vec![row("scalar - fixed target")],
            )
            .unwrap(),
        )
        .unwrap();
    problem
        .declare_fixed_variable(variable, VariableValue::Scalar(0.0), residual)
        .unwrap();
    problem
        .set_variable_value(variable, VariableValue::Scalar(1.0))
        .unwrap();
    let before = problem.packed_state().unwrap();
    let mut controller = OperationController::new(OperationControl::unlimited());

    let error = problem
        .certify_current_state_with_controller(SolverConfig::default(), &mut controller)
        .unwrap_err();
    assert!(
        matches!(
            error,
            CoreError::InvalidAcceptedLinearization {
                context: "materialized state requires fixed, alias, or bound canonicalization"
            }
        ),
        "{error:?}"
    );
    assert_eq!(problem.packed_state().unwrap(), before);
}

#[test]
fn current_state_certification_accepts_finite_fixed_only_priority_without_claiming_optimality() {
    let mut problem = Problem::new();
    let variable = problem.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap());
    let source_id = source(&mut problem, "fixed scalar");
    let residual = problem
        .add_residual(
            ResidualBlock::fixed_variable(
                source_id,
                variable,
                VariableValue::Scalar(0.0),
                vec![1.0],
                vec![row("scalar - fixed target")],
            )
            .unwrap(),
        )
        .unwrap();
    problem
        .declare_fixed_variable(variable, VariableValue::Scalar(0.0), residual)
        .unwrap();
    add_scalar_target(
        &mut problem,
        variable,
        ResidualCategory::Preference,
        10.0,
        1.0,
    );
    let before = problem.packed_state().unwrap();
    let mut controller = OperationController::new(OperationControl::unlimited());

    let report = problem
        .certify_current_state_with_controller(SolverConfig::default(), &mut controller)
        .unwrap()
        .expect("complete certification");

    assert_eq!(problem.packed_state().unwrap(), before);
    assert_eq!(report.termination, SolveTermination::Converged);
    assert_eq!(report.preference_status, SecondaryStatus::Acceptable);
    let [preference] = report.priority_solves.as_slice() else {
        panic!("expected one fixed-only priority report")
    };
    assert_eq!(preference.scope, PrioritySolveScope::Fixed);
    assert_eq!(preference.status, SecondaryStatus::Acceptable);
    assert!(preference.final_cost.is_some_and(|cost| cost > 0.0));
}

#[test]
fn current_state_certification_exhaustion_returns_no_partial_report_or_motion() {
    let mut problem = Problem::new();
    let variable = problem.add_variable(VariableBlock::scalar(2.0, 1.0).unwrap());
    add_scalar_target(&mut problem, variable, ResidualCategory::Hard, 2.0, 1.0);
    let before = problem.packed_state().unwrap();
    let mut control = OperationControl::unlimited();
    control.limits.document_dependency_items = 0;
    let mut controller = OperationController::new(control);

    assert!(
        problem
            .certify_current_state_with_controller(SolverConfig::default(), &mut controller)
            .unwrap()
            .is_none()
    );
    assert!(controller.is_stopped());
    assert_eq!(problem.packed_state().unwrap(), before);
}
