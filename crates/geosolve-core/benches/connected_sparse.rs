use std::hint::black_box;
use std::time::{Duration, Instant};

use criterion::{BenchmarkGroup, BenchmarkId, Criterion, criterion_group, criterion_main};
use geosolve_core::{
    AuditBinding, DiagnosticBudget, EvaluationError, HardValidity, LinearSolveBackend,
    LinearSolveBackendPolicy, LocalJacobian, Problem, ResidualBlock, ResidualCategory,
    ResidualEvaluator, ResidualRowAudit, SolveReport, SolverConfig, SourceConstraint,
    VariableBlock, VariableId, VariableValue,
};

const SIZES: [usize; 3] = [64, 128, 256];

#[derive(Clone, Copy, Debug)]
struct ScalarTarget {
    target: f64,
}

impl ResidualEvaluator for ScalarTarget {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        let [VariableValue::Scalar(value)] = variables else {
            return Err(EvaluationError::invalid_geometry("expected one scalar"));
        };
        Ok(vec![value - self.target])
    }

    fn jacobian(
        &self,
        _variables: &[VariableValue],
    ) -> Result<Vec<LocalJacobian>, EvaluationError> {
        Ok(vec![LocalJacobian::new(1, 1, vec![1.0])])
    }
}

#[derive(Clone, Copy, Debug)]
struct ScalarDifference {
    target: f64,
}

impl ResidualEvaluator for ScalarDifference {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        let [VariableValue::Scalar(first), VariableValue::Scalar(second)] = variables else {
            return Err(EvaluationError::invalid_geometry("expected two scalars"));
        };
        Ok(vec![second - first - self.target])
    }

    fn jacobian(
        &self,
        _variables: &[VariableValue],
    ) -> Result<Vec<LocalJacobian>, EvaluationError> {
        Ok(vec![
            LocalJacobian::new(1, 1, vec![-1.0]),
            LocalJacobian::new(1, 1, vec![1.0]),
        ])
    }
}

fn row(label: impl Into<String>) -> Vec<ResidualRowAudit> {
    vec![ResidualRowAudit::new(
        label,
        vec![AuditBinding::new("chain", "connected sparse benchmark")],
        "normalized model unit",
    )]
}

fn connected_problem(size: usize) -> (Problem, VariableId) {
    assert!(size > 0);
    let witness = (0..size)
        .map(|index| {
            let index = f64::from(u32::try_from(index).expect("benchmark size fits u32"));
            0.2 * (index * 0.07).sin() + index * 0.01
        })
        .collect::<Vec<_>>();
    let mut problem = Problem::new();
    let variables = witness
        .iter()
        .enumerate()
        .map(|(index, &value)| {
            let perturbation = if index.is_multiple_of(2) {
                0.02
            } else {
                -0.015
            };
            problem.add_variable(VariableBlock::scalar(value + perturbation, 1.0).unwrap())
        })
        .collect::<Vec<_>>();
    let source = problem.add_source(SourceConstraint::new("connected chain anchor").unwrap());
    problem
        .add_residual(
            ResidualBlock::new(
                source,
                ResidualCategory::Hard,
                vec![variables[0]],
                1,
                vec![1.0],
                row("chain anchor"),
                ScalarTarget { target: witness[0] },
            )
            .unwrap(),
        )
        .unwrap();
    for index in 1..size {
        let source = problem
            .add_source(SourceConstraint::new(format!("connected chain edge {index}")).unwrap());
        problem
            .add_residual(
                ResidualBlock::new(
                    source,
                    ResidualCategory::Hard,
                    vec![variables[index - 1], variables[index]],
                    1,
                    vec![1.0],
                    row(format!("chain edge {index}")),
                    ScalarDifference {
                        target: witness[index] - witness[index - 1],
                    },
                )
                .unwrap(),
            )
            .unwrap();
    }
    (problem, variables[size / 2])
}

fn config(policy: LinearSolveBackendPolicy) -> SolverConfig {
    SolverConfig {
        initial_damping: 1.0e-15,
        minimum_damping: 1.0e-15,
        linear_solve_backend: policy,
        redundancy_diagnostic_budget: DiagnosticBudget {
            enabled: false,
            ..DiagnosticBudget::unlimited()
        },
        conflict_diagnostic_budget: DiagnosticBudget {
            enabled: false,
            ..DiagnosticBudget::unlimited()
        },
        ..SolverConfig::default()
    }
}

fn validate_report(
    report: &SolveReport,
    size: usize,
    expected_backend: LinearSolveBackend,
    expect_symbolic_reuse: bool,
) {
    assert_eq!(report.hard_validity, HardValidity::Valid);
    assert!(report.hard_residuals_validated);
    assert!(report.hard_residual_max <= 1.0e-9);
    assert!(report.rank_is_valid);
    assert_eq!(report.rank, size);
    assert_eq!(report.right_nullity, 0);
    assert_eq!(report.structural_nnz, 2 * size - 1);
    assert_eq!(report.actual_backend, Some(expected_backend));
    assert_eq!(report.sparse_fallback_reason, None);
    assert_eq!(report.symbolic_analysis_reused, expect_symbolic_reuse);
}

fn configure(group: &mut BenchmarkGroup<'_, criterion::measurement::WallTime>, size: usize) {
    let measurement = if size >= 256 { 800 } else { 500 };
    group
        .sample_size(10)
        .warm_up_time(Duration::from_millis(150))
        .measurement_time(Duration::from_millis(measurement));
}

fn dense_numeric_solve(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("m16_connected_dense_numeric_solve");
    for size in SIZES {
        configure(&mut group, size);
        group.bench_function(BenchmarkId::from_parameter(size), |bencher| {
            bencher.iter_custom(|iterations| {
                let mut elapsed = Duration::ZERO;
                for _ in 0..iterations {
                    let (mut problem, _) = connected_problem(size);
                    let start = Instant::now();
                    let report = problem
                        .solve(config(LinearSolveBackendPolicy::DenseOnly))
                        .unwrap();
                    elapsed += start.elapsed();
                    validate_report(&report, size, LinearSolveBackend::Dense, false);
                    black_box(report);
                }
                elapsed
            });
        });
    }
    group.finish();
}

fn sparse_cold_symbolic_numeric_solve(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("m16_connected_sparse_cold_symbolic_numeric_solve");
    for size in SIZES {
        configure(&mut group, size);
        group.bench_function(BenchmarkId::from_parameter(size), |bencher| {
            bencher.iter_custom(|iterations| {
                let mut elapsed = Duration::ZERO;
                for _ in 0..iterations {
                    let (mut problem, _) = connected_problem(size);
                    let start = Instant::now();
                    let report = problem
                        .solve(config(LinearSolveBackendPolicy::SparsePreferred))
                        .unwrap();
                    elapsed += start.elapsed();
                    validate_report(&report, size, LinearSolveBackend::SparseQr, false);
                    black_box(report);
                }
                elapsed
            });
        });
    }
    group.finish();
}

fn sparse_reused_numeric_solve(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("m16_connected_sparse_reused_numeric_solve");
    for size in SIZES {
        configure(&mut group, size);
        let (mut seeded, edit_variable) = connected_problem(size);
        let initial = seeded
            .solve(config(LinearSolveBackendPolicy::SparsePreferred))
            .unwrap();
        validate_report(&initial, size, LinearSolveBackend::SparseQr, false);
        group.bench_function(BenchmarkId::from_parameter(size), |bencher| {
            bencher.iter_custom(|iterations| {
                let mut elapsed = Duration::ZERO;
                for _ in 0..iterations {
                    let mut problem = seeded.clone();
                    problem
                        .apply_local_increment(edit_variable, &[0.01])
                        .unwrap();
                    let start = Instant::now();
                    let report = problem
                        .solve(config(LinearSolveBackendPolicy::SparsePreferred))
                        .unwrap();
                    elapsed += start.elapsed();
                    validate_report(&report, size, LinearSolveBackend::SparseQr, true);
                    black_box(report);
                }
                elapsed
            });
        });
    }
    group.finish();
}

fn connected_sparse_benchmarks(criterion: &mut Criterion) {
    // Validate benchmark-only residual derivatives once, outside every timed
    // boundary, at each represented problem size.
    for size in SIZES {
        let (problem, _) = connected_problem(size);
        let jacobians = problem.check_jacobians(1.0e-6).unwrap();
        assert!(jacobians.all_within(1.0e-8), "{jacobians:#?}");
    }
    dense_numeric_solve(criterion);
    sparse_cold_symbolic_numeric_solve(criterion);
    sparse_reused_numeric_solve(criterion);
}

criterion_group!(benches, connected_sparse_benchmarks);
criterion_main!(benches);
