use std::hint::black_box;
use std::time::Duration;

use criterion::{BatchSize, BenchmarkId, Criterion, criterion_group, criterion_main};
use geosolve_core::{
    AuditBinding, EvaluationError, LocalJacobian, Problem, ResidualBlock, ResidualCategory,
    ResidualEvaluator, ResidualRowAudit, SolverConfig, SourceConstraint, VariableBlock,
    VariableValue,
};

#[derive(Debug)]
struct DenseAffine {
    matrix: Vec<Vec<f64>>,
    target: Vec<f64>,
}

impl ResidualEvaluator for DenseAffine {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        let values: Result<Vec<_>, _> = variables
            .iter()
            .map(|value| match value {
                VariableValue::Scalar(value) => Ok(*value),
                _ => Err(EvaluationError::invalid_geometry(
                    "benchmark expected scalar variables",
                )),
            })
            .collect();
        let values = values?;
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

    fn jacobian(
        &self,
        _variables: &[VariableValue],
    ) -> Result<Vec<LocalJacobian>, EvaluationError> {
        Ok((0..self.matrix.len())
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

fn dense_problem(dimension: usize) -> Problem {
    let mut problem = Problem::new();
    let dimension_value = f64::from(u32::try_from(dimension).unwrap());
    let variables: Vec<_> = (0..dimension)
        .map(|_| problem.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap()))
        .collect();
    let matrix: Vec<Vec<f64>> = (0..dimension)
        .map(|row| {
            (0..dimension)
                .map(|column| {
                    if row == column {
                        dimension_value + 2.0
                    } else {
                        f64::from(u8::try_from((3 * row + 5 * column) % 7).unwrap()) / 20.0
                    }
                })
                .collect()
        })
        .collect();
    let witness: Vec<_> = (1..=dimension)
        .map(|value| f64::from(u32::try_from(value).unwrap()) / 4.0)
        .collect();
    let target = matrix
        .iter()
        .map(|row| {
            row.iter()
                .zip(&witness)
                .map(|(coefficient, value)| coefficient * value)
                .sum()
        })
        .collect();
    let source = problem.add_source(SourceConstraint::new("dense benchmark system").unwrap());
    let audit_rows = (0..dimension)
        .map(|row| {
            ResidualRowAudit::new(
                format!("dense affine row {row}"),
                vec![AuditBinding::new("x", "benchmark scalar vector")],
                "model unit",
            )
        })
        .collect();
    problem
        .add_residual(
            ResidualBlock::new(
                source,
                ResidualCategory::Hard,
                variables,
                dimension,
                vec![1.0; dimension],
                audit_rows,
                DenseAffine { matrix, target },
            )
            .unwrap(),
        )
        .unwrap();
    problem
}

fn small_dense_solves(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("small_dense_solve");
    for dimension in [2, 4, 8] {
        group.bench_with_input(
            BenchmarkId::from_parameter(dimension),
            &dimension,
            |bencher, &dimension| {
                bencher.iter_batched(
                    || dense_problem(dimension),
                    |mut problem| black_box(problem.solve(SolverConfig::default()).unwrap()),
                    BatchSize::SmallInput,
                );
            },
        );
    }
    group.finish();
}

criterion_group! {
    name = benches;
    config = Criterion::default()
        .warm_up_time(Duration::from_millis(250))
        .measurement_time(Duration::from_secs(1))
        .sample_size(20);
    targets = small_dense_solves
}
criterion_main!(benches);
