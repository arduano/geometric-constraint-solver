use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use geosolve_core::{
    AuditBinding, EvaluationError, LocalJacobian, OperationControl, OperationOutcome,
    PrioritySolveBackend, PrioritySolveScope, Problem, ResidualBlock, ResidualCategory,
    ResidualEvaluator, ResidualRowAudit, SolveTermination, SolverConfig, SourceConstraint,
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
