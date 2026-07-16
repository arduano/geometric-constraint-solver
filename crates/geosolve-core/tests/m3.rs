mod support;

use std::collections::BTreeMap;
use std::fmt::Write as _;

use geosolve_core::{
    CoreError, EvaluationError, LocalJacobian, Problem, ResidualBlock, ResidualCategory,
    ResidualEvaluator, SolveReport, SolveTermination, SolverConfig, SourceConstraintId,
    VariableBlock, VariableId, VariableValue,
};
use proptest::collection::vec;
use proptest::prelude::*;
use proptest::test_runner::{Config, RngAlgorithm, TestCaseError, TestRng, TestRunner};

use support::{
    AffineScalars, CircleGeometry, MixedKinds, ScalarQuadratic, Similarity, add_affine_residual,
    add_source, assert_jacobians, assert_report_finite, assert_trace_invariants, audit_row,
    circle_fixture, normalized_circle_residuals, scalar_value, vec2_value,
};

const PROPERTY_BASE_SEED_HEX: &str =
    "4d33a7419c2e5b7088d4f1036ac952ef117b8d60c4aa39e275018bc6de42f90a";
const PROPERTY_BASE_SEED: [u8; 32] = [
    0x4d, 0x33, 0xa7, 0x41, 0x9c, 0x2e, 0x5b, 0x70, 0x88, 0xd4, 0xf1, 0x03, 0x6a, 0xc9, 0x52, 0xef,
    0x11, 0x7b, 0x8d, 0x60, 0xc4, 0xaa, 0x39, 0xe2, 0x75, 0x01, 0x8b, 0xc6, 0xde, 0x42, 0xf9, 0x0a,
];
const PROPERTY_CASES_PER_SHAPE: u32 = 32;
const LOCAL_BASIN_PERTURBATIONS: [[f64; 2]; 5] = [
    [-0.45, 0.35],
    [0.55, -0.40],
    [0.20, 0.60],
    [-0.60, -0.25],
    [0.35, 0.15],
];
const SCALES: [f64; 3] = [1.0e-6, 1.0, 1.0e6];

#[derive(Clone, Copy, Debug)]
enum LinearShape {
    Exact,
    Underdetermined,
    Overdetermined,
}

impl LinearShape {
    const ALL: [Self; 3] = [Self::Exact, Self::Underdetermined, Self::Overdetermined];

    const fn seed_tag(self) -> u8 {
        match self {
            Self::Exact => 0x11,
            Self::Underdetermined => 0x22,
            Self::Overdetermined => 0x33,
        }
    }
}

#[derive(Clone, Debug)]
struct LinearCase {
    shape: LinearShape,
    rows: usize,
    columns: usize,
    rank: usize,
    matrix: Vec<Vec<f64>>,
    target: Vec<f64>,
    witness: Vec<f64>,
    initial: Vec<f64>,
    row_permutation: Vec<usize>,
    column_permutation: Vec<usize>,
}

fn linear_case_strategy(shape: LinearShape) -> impl Strategy<Value = LinearCase> {
    (
        1_usize..=5,
        any::<[u8; 4]>(),
        vec(-2_i8..=2, 96),
        vec(-3_i8..=3, 6),
        vec(-3_i8..=3, 6),
    )
        .prop_map(
            move |(base_columns, selectors, entropy, witness, initial)| {
                construct_linear_case(shape, base_columns, selectors, &entropy, &witness, &initial)
            },
        )
}

fn construct_linear_case(
    shape: LinearShape,
    base_columns: usize,
    selectors: [u8; 4],
    entropy: &[i8],
    witness_values: &[i8],
    initial_values: &[i8],
) -> LinearCase {
    let (rows, columns) = match shape {
        LinearShape::Exact => (base_columns, base_columns),
        LinearShape::Underdetermined => {
            let columns = base_columns + 1;
            (1 + usize::from(selectors[0]) % (columns - 1), columns)
        }
        LinearShape::Overdetermined => (
            base_columns + 1 + usize::from(selectors[0]) % 3,
            base_columns,
        ),
    };
    let rank = usize::from(selectors[1]) % (rows.min(columns) + 1);
    let mut entropy = entropy.iter().copied().cycle();
    let mut independent_rows = vec![vec![0.0; columns]; rank];
    for (pivot, row) in independent_rows.iter_mut().enumerate() {
        row[pivot] = 1.0;
        for value in &mut row[rank..] {
            *value = f64::from(entropy.next().unwrap());
        }
    }
    let mut base_matrix = independent_rows.clone();
    while base_matrix.len() < rows {
        let coefficients: Vec<_> = (0..rank)
            .map(|_| f64::from(entropy.next().unwrap()))
            .collect();
        base_matrix.push(
            (0..columns)
                .map(|column| {
                    coefficients
                        .iter()
                        .zip(&independent_rows)
                        .map(|(coefficient, row)| coefficient * row[column])
                        .sum()
                })
                .collect(),
        );
    }
    let base_witness: Vec<_> = witness_values[..columns]
        .iter()
        .map(|value| f64::from(*value))
        .collect();
    let base_initial: Vec<_> = initial_values[..columns]
        .iter()
        .map(|value| f64::from(*value))
        .collect();
    let row_permutation = deterministic_permutation(rows, selectors[2]);
    let column_permutation = deterministic_permutation(columns, selectors[3]);
    let matrix: Vec<Vec<f64>> = row_permutation
        .iter()
        .map(|&row| {
            column_permutation
                .iter()
                .map(|&column| base_matrix[row][column])
                .collect()
        })
        .collect();
    let witness: Vec<f64> = column_permutation
        .iter()
        .map(|&column| base_witness[column])
        .collect();
    let initial: Vec<f64> = column_permutation
        .iter()
        .map(|&column| base_initial[column])
        .collect();
    let target = multiply(&matrix, &witness);
    LinearCase {
        shape,
        rows,
        columns,
        rank,
        matrix,
        target,
        witness,
        initial,
        row_permutation,
        column_permutation,
    }
}

fn deterministic_permutation(size: usize, selector: u8) -> Vec<usize> {
    let mut permutation: Vec<_> = (0..size).collect();
    if size > 1 {
        permutation.rotate_left(usize::from(selector) % size);
        if (usize::from(selector) / size) % 2 == 1 {
            permutation.reverse();
        }
    }
    permutation
}

fn is_index_permutation(values: &[usize], size: usize) -> bool {
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    sorted == (0..size).collect::<Vec<_>>()
}

fn multiply(matrix: &[Vec<f64>], values: &[f64]) -> Vec<f64> {
    matrix
        .iter()
        .map(|row| {
            row.iter()
                .zip(values)
                .map(|(coefficient, value)| coefficient * value)
                .sum()
        })
        .collect()
}

fn linear_oracle(case: &LinearCase, values: &[f64]) -> f64 {
    multiply(&case.matrix, values)
        .iter()
        .zip(&case.target)
        .map(|(actual, expected)| (actual - expected).abs())
        .fold(0.0, f64::max)
}

fn linear_problem(case: &LinearCase) -> (Problem, Vec<VariableId>) {
    let mut problem = Problem::new();
    let variables: Vec<_> = case
        .initial
        .iter()
        .map(|&value| problem.add_variable(VariableBlock::scalar(value, 1.0).unwrap()))
        .collect();
    let source = add_source(&mut problem, "rank-by-construction affine system");
    add_affine_residual(
        &mut problem,
        source,
        variables.clone(),
        case.matrix.clone(),
        case.target.clone(),
        1.0,
    );
    (problem, variables)
}

fn property_seed(shape: LinearShape) -> [u8; 32] {
    let mut seed = PROPERTY_BASE_SEED;
    seed[31] ^= shape.seed_tag();
    seed
}

fn seed_hex(seed: &[u8]) -> String {
    seed.iter()
        .fold(String::with_capacity(seed.len() * 2), |mut output, byte| {
            write!(output, "{byte:02x}").unwrap();
            output
        })
}

fn verify_linear_case(case: &LinearCase, seed: &str) -> Result<(), TestCaseError> {
    let context = || format!("seed={seed}; case={case:#?}");
    let shape_matches_dimensions = match case.shape {
        LinearShape::Exact => case.rows == case.columns,
        LinearShape::Underdetermined => case.rows < case.columns,
        LinearShape::Overdetermined => case.rows > case.columns,
    };
    prop_assert!(
        shape_matches_dimensions,
        "generated shape does not match dimensions; {}",
        context()
    );
    prop_assert!(
        is_index_permutation(&case.row_permutation, case.rows)
            && is_index_permutation(&case.column_permutation, case.columns),
        "invalid row/column permutation; {}",
        context()
    );
    prop_assert!(
        linear_oracle(case, &case.witness) <= f64::EPSILON,
        "constructed witness is invalid; {}",
        context()
    );
    let (mut problem, variables) = linear_problem(case);
    let report = problem
        .solve(SolverConfig::default())
        .map_err(|error| TestCaseError::fail(format!("solve error {error}; {}", context())))?;
    let values: Vec<_> = variables
        .iter()
        .map(|&variable| scalar_value(&problem, variable))
        .collect();
    let residual = linear_oracle(case, &values);
    prop_assert!(
        report.termination == SolveTermination::Converged,
        "wrong termination {:?}; {}",
        report.termination,
        context()
    );
    prop_assert!(
        report.hard_residuals_validated && residual <= 1.0e-9,
        "independent residual={residual:e}, report={report:#?}; {}",
        context()
    );
    prop_assert!(
        report.rank == case.rank,
        "reported rank {}, expected {}; report={report:#?}; {}",
        report.rank,
        case.rank,
        context()
    );
    prop_assert!(
        report.local_degrees_of_freedom == case.columns - case.rank,
        "reported nullity {}, expected {}; report={report:#?}; {}",
        report.local_degrees_of_freedom,
        case.columns - case.rank,
        context()
    );
    prop_assert!(
        report.is_singular == (case.rank < case.rows.min(case.columns)),
        "singularity diagnosis mismatch; report={report:#?}; {}",
        context()
    );
    prop_assert!(
        report
            .accepted_state
            .ambient()
            .iter()
            .chain(&report.singular_values)
            .all(|value| value.is_finite()),
        "non-finite report state; report={report:#?}; {}",
        context()
    );
    if case.rank == case.columns {
        let error = values
            .iter()
            .zip(&case.witness)
            .map(|(actual, expected)| (actual - expected).abs())
            .fold(0.0, f64::max);
        prop_assert!(
            error <= 2.0e-9,
            "unique solution error={error:e}; values={values:?}; {}",
            context()
        );
    }
    Ok(())
}

#[test]
fn property_linear_systems_have_constructed_rank_nullity_and_solution() {
    for shape in LinearShape::ALL {
        let seed = property_seed(shape);
        let seed_text = seed_hex(&seed);
        let config = Config {
            cases: PROPERTY_CASES_PER_SHAPE,
            max_shrink_iters: 2_048,
            failure_persistence: None,
            ..Config::default()
        };
        let mut runner =
            TestRunner::new_with_rng(config, TestRng::from_seed(RngAlgorithm::ChaCha, &seed));
        let result = runner.run(&linear_case_strategy(shape), |case| {
            verify_linear_case(&case, &seed_text)
        });
        if let Err(error) = result {
            panic!(
                "M3 property failure: base_seed={PROPERTY_BASE_SEED_HEX}; seed={seed_text}; \
                 shape={shape:?}; cases={PROPERTY_CASES_PER_SHAPE}; max_shrink_iters=2048; \
                 reproduce with `cargo test -p geosolve-core --test m3 \
                 property_linear_systems_have_constructed_rank_nullity_and_solution -- --exact \
                 --nocapture`; {error}"
            );
        }
    }
}

#[test]
fn constructed_rank_zero_one_and_full_edges_are_explicitly_verified() {
    for expected_rank in [0, 1, 3] {
        let case = construct_linear_case(
            LinearShape::Exact,
            3,
            [0, expected_rank, 1, 2],
            &[1; 96],
            &[1, 2, 3, 0, 0, 0],
            &[-1, -2, -3, 0, 0, 0],
        );
        assert_eq!(case.rank, usize::from(expected_rank));
        assert_eq!(case.row_permutation, vec![1, 2, 0]);
        assert_eq!(case.column_permutation, vec![2, 0, 1]);
        for (actual, expected) in case.witness.iter().zip([3.0, 1.0, 2.0]) {
            assert!((*actual - expected).abs() <= f64::EPSILON);
        }
        for (actual, expected) in case.initial.iter().zip([-3.0, -1.0, -2.0]) {
            assert!((*actual - expected).abs() <= f64::EPSILON);
        }
        assert!(linear_oracle(&case, &case.witness) <= f64::EPSILON);

        let (mut problem, variables) = linear_problem(&case);
        let report = problem.solve(SolverConfig::default()).unwrap();
        let values: Vec<_> = variables
            .iter()
            .map(|&variable| scalar_value(&problem, variable))
            .collect();
        assert_eq!(report.termination, SolveTermination::Converged);
        assert_eq!(report.rank, usize::from(expected_rank));
        assert_eq!(
            report.local_degrees_of_freedom,
            case.columns - usize::from(expected_rank)
        );
        assert!(linear_oracle(&case, &values) <= 1.0e-9);
        if usize::from(expected_rank) == case.columns {
            for (actual, expected) in values.iter().zip(&case.witness) {
                assert!((actual - expected).abs() <= 2.0e-9);
            }
        }
        assert_report_finite(&report);
    }
}

#[test]
fn construct_valid_nonlinear_system_recovers_inside_documented_local_basin() {
    for perturbation in LOCAL_BASIN_PERTURBATIONS {
        let (mut problem, point, geometry) = circle_fixture(
            Similarity {
                scale: 1.0,
                rotation: 0.0,
                translation: [0.0, 0.0],
            },
            perturbation,
        );
        let report = problem.solve(SolverConfig::default()).unwrap();
        let solved = vec2_value(&problem, point);
        let independent = normalized_circle_residuals(geometry, solved);
        assert_eq!(
            report.termination,
            SolveTermination::Converged,
            "perturbation={perturbation:?}; report={report:#?}"
        );
        assert!(
            independent.iter().all(|value| value.abs() <= 1.0e-9),
            "perturbation={perturbation:?}; residuals={independent:?}"
        );
        for coordinate in 0..2 {
            assert!(
                (solved[coordinate] - geometry.expected[coordinate]).abs() <= 2.0e-9,
                "perturbation={perturbation:?}; solved={solved:?}; expected={:?}",
                geometry.expected
            );
        }
        assert_eq!((report.rank, report.local_degrees_of_freedom), (2, 0));
        assert_report_finite(&report);
    }
}

#[derive(Debug, PartialEq)]
struct MetamorphicDiagnosis {
    termination: SolveTermination,
    rank: usize,
    degrees_of_freedom: usize,
    singular: bool,
    source_labels: Vec<String>,
    redundant_labels: Vec<String>,
    conflicting_labels: Vec<String>,
}

fn diagnosis(problem: &Problem, report: &SolveReport) -> MetamorphicDiagnosis {
    let labels = |sources: &[SourceConstraintId]| {
        sources
            .iter()
            .map(|source| problem.source(*source).unwrap().label().to_owned())
            .collect()
    };
    MetamorphicDiagnosis {
        termination: report.termination,
        rank: report.rank,
        degrees_of_freedom: report.local_degrees_of_freedom,
        singular: report.is_singular,
        source_labels: report
            .audit
            .sources
            .iter()
            .map(|source| source.source_label.clone())
            .collect(),
        redundant_labels: labels(&report.redundant_sources),
        conflicting_labels: labels(&report.conflicting_sources),
    }
}

fn solve_transformed_circle(
    transform: Similarity,
) -> (Problem, SolveReport, [f64; 2], [f64; 2], CircleGeometry) {
    let (mut problem, point, geometry) = circle_fixture(transform, [0.35, -0.20]);
    let initial_residuals = normalized_circle_residuals(geometry, vec2_value(&problem, point));
    let report = problem.solve(SolverConfig::default()).unwrap();
    let solved = vec2_value(&problem, point);
    (problem, report, initial_residuals, solved, geometry)
}

#[test]
fn similarity_metamorphisms_preserve_normalized_geometry_rank_dof_and_diagnosis() {
    let identity = Similarity {
        scale: 1.0,
        rotation: 0.0,
        translation: [0.0, 0.0],
    };
    let (base_problem, base_report, base_initial_residuals, _, _) =
        solve_transformed_circle(identity);
    let base_diagnosis = diagnosis(&base_problem, &base_report);
    let mut transforms = vec![
        Similarity {
            scale: 1.0,
            rotation: 0.0,
            translation: [7.25, -11.5],
        },
        Similarity {
            scale: 1.0,
            rotation: 0.731,
            translation: [0.0, 0.0],
        },
    ];
    transforms.extend(SCALES.map(|scale| Similarity {
        scale,
        rotation: 0.0,
        translation: [0.0, 0.0],
    }));

    for transform in transforms {
        let (problem, report, initial_residuals, solved, geometry) =
            solve_transformed_circle(transform);
        let independent = normalized_circle_residuals(geometry, solved);
        let normalized_solved = transform.inverse(solved);
        let normalized_expected = transform.inverse(geometry.expected);
        let reflected_root = [normalized_expected[0], -normalized_expected[1]];
        for row in 0..2 {
            assert!(
                (initial_residuals[row] - base_initial_residuals[row]).abs() <= 2.0e-10,
                "transform={transform:?}; transformed={initial_residuals:?}; \
                 baseline={base_initial_residuals:?}"
            );
            assert!(
                independent[row].abs() <= 1.0e-9,
                "transform={transform:?}; independent={independent:?}; report={report:#?}"
            );
            assert!(
                (normalized_solved[row] - normalized_expected[row]).abs() <= 2.0e-9,
                "transform={transform:?}; normalized_solved={normalized_solved:?}; \
                 normalized_expected={normalized_expected:?}"
            );
        }
        let reflected_distance = (normalized_solved[0] - reflected_root[0])
            .hypot(normalized_solved[1] - reflected_root[1]);
        assert!(
            reflected_distance >= 5.0,
            "transform={transform:?}; branch flipped to reflected root {reflected_root:?}; \
             normalized_solved={normalized_solved:?}"
        );
        assert_eq!(
            diagnosis(&problem, &report),
            base_diagnosis,
            "{transform:?}"
        );
        assert_report_finite(&report);
    }
}

const VARIABLE_PERMUTATIONS: [[usize; 3]; 6] = [
    [0, 1, 2],
    [0, 2, 1],
    [1, 0, 2],
    [1, 2, 0],
    [2, 0, 1],
    [2, 1, 0],
];
const RESIDUAL_PERMUTATIONS: [[usize; 5]; 5] = [
    [0, 1, 2, 3, 4],
    [4, 3, 2, 1, 0],
    [2, 3, 4, 0, 1],
    [1, 3, 0, 4, 2],
    [3, 0, 4, 2, 1],
];
const VARIABLE_LABELS: [&str; 3] = ["x", "y", "z"];
const SOURCE_LABELS: [&str; 5] = [
    "x fixed",
    "y fixed",
    "z fixed",
    "sum redundant",
    "scaled sum redundant",
];

fn permutation_problem(
    variable_order: [usize; 3],
    residual_order: [usize; 5],
) -> (Problem, [VariableId; 3]) {
    let mut problem = Problem::new();
    let mut variables = [None; 3];
    let initial = [-2.0, 4.0, 0.0];
    for semantic in variable_order {
        variables[semantic] =
            Some(problem.add_variable(VariableBlock::scalar(initial[semantic], 1.0).unwrap()));
    }
    let variables = variables.map(Option::unwrap);
    let sources: Vec<_> = SOURCE_LABELS
        .iter()
        .map(|label| add_source(&mut problem, label))
        .collect();

    for equation in residual_order {
        let (incidence, matrix, target, scale) = match equation {
            0 => (vec![variables[0]], vec![vec![1.0]], vec![1.0], 1.0),
            1 => (vec![variables[1]], vec![vec![1.0]], vec![2.0], 1.0),
            2 => (vec![variables[2]], vec![vec![1.0]], vec![3.0], 1.0),
            3 => (
                vec![variables[0], variables[1]],
                vec![vec![1.0, 1.0]],
                vec![3.0],
                1.0,
            ),
            4 => (
                vec![variables[0], variables[1]],
                vec![vec![2.0, 2.0]],
                vec![6.0],
                2.0,
            ),
            _ => unreachable!(),
        };
        add_affine_residual(
            &mut problem,
            sources[equation],
            incidence,
            matrix,
            target,
            scale,
        );
    }
    (problem, variables)
}

fn semantic_geometry(problem: &Problem, variables: [VariableId; 3]) -> BTreeMap<&'static str, f64> {
    VARIABLE_LABELS
        .into_iter()
        .zip(variables.map(|variable| scalar_value(problem, variable)))
        .collect()
}

fn source_labels(problem: &Problem, sources: &[SourceConstraintId]) -> Vec<String> {
    sources
        .iter()
        .map(|source| problem.source(*source).unwrap().label().to_owned())
        .collect()
}

fn assert_flat_audit_matches_dense_layout(problem: &Problem) {
    let descriptors = problem.audit_rows().unwrap();
    let assembly = problem.assemble_dense().unwrap();
    let mut descriptor_index = 0;
    for layout in assembly.residual_layout() {
        for row_in_block in 0..layout.row_range.len() {
            let descriptor = &descriptors[descriptor_index];
            let dense_row = layout.row_range.start + row_in_block;
            assert_eq!(descriptor.residual_id, layout.residual_id);
            assert_eq!(descriptor.row_in_block, row_in_block);
            assert_eq!(dense_row, descriptor_index);
            descriptor_index += 1;
        }
    }
    assert_eq!(descriptor_index, descriptors.len());
    assert_eq!(descriptor_index, assembly.residuals().len());
}

#[test]
fn flat_audit_descriptors_map_directly_to_dense_residual_rows() {
    let mut problem = Problem::new();
    let x = problem.add_variable(VariableBlock::scalar(0.5, 1.0).unwrap());
    let y = problem.add_variable(VariableBlock::scalar(-0.5, 1.0).unwrap());
    let first_source = add_source(&mut problem, "first source in store");
    let second_source = add_source(&mut problem, "second source in store");
    add_affine_residual(
        &mut problem,
        second_source,
        vec![x],
        vec![vec![1.0]],
        vec![0.0],
        1.0,
    );
    add_affine_residual(
        &mut problem,
        first_source,
        vec![x, y],
        vec![vec![1.0, 0.0], vec![0.0, 1.0]],
        vec![0.0, 0.0],
        1.0,
    );

    assert_flat_audit_matches_dense_layout(&problem);
    let flat_labels: Vec<_> = problem
        .audit_rows()
        .unwrap()
        .iter()
        .map(|row| row.source_label.clone())
        .collect();
    assert_eq!(
        flat_labels,
        [
            "second source in store",
            "first source in store",
            "first source in store"
        ]
    );
    let snapshot_labels: Vec<_> = problem
        .audit_snapshot()
        .unwrap()
        .sources
        .iter()
        .map(|source| source.source_label.clone())
        .collect();
    assert_eq!(
        snapshot_labels,
        ["first source in store", "second source in store"]
    );
}

#[test]
fn insertion_permutations_preserve_semantic_geometry_and_source_diagnostics_order() {
    let mut baseline_geometry: Option<BTreeMap<&'static str, f64>> = None;
    for variable_order in VARIABLE_PERMUTATIONS {
        for residual_order in RESIDUAL_PERMUTATIONS {
            let (mut problem, variables) = permutation_problem(variable_order, residual_order);
            let report = problem.solve(SolverConfig::default()).unwrap();
            assert_eq!(report.termination, SolveTermination::Converged);
            assert_eq!((report.rank, report.local_degrees_of_freedom), (3, 0));
            let geometry = semantic_geometry(&problem, variables);
            for (label, expected) in VARIABLE_LABELS.into_iter().zip([1.0, 2.0, 3.0]) {
                assert!(
                    (geometry[label] - expected).abs() <= 2.0e-9,
                    "variable_order={variable_order:?}; residual_order={residual_order:?}; \
                     expected {label}={expected}, geometry={geometry:?}"
                );
            }
            let x = geometry["x"];
            let y = geometry["y"];
            let z = geometry["z"];
            let equations = [
                x - 1.0,
                y - 2.0,
                z - 3.0,
                x + y - 3.0,
                2.0 * x + 2.0 * y - 6.0,
            ];
            assert!(
                equations.iter().all(|value| value.abs() <= 2.0e-9),
                "variable_order={variable_order:?}; residual_order={residual_order:?}; \
                 equations={equations:?}; geometry={geometry:?}"
            );
            if let Some(baseline) = &baseline_geometry {
                for label in VARIABLE_LABELS {
                    assert!(
                        (geometry[label] - baseline[label]).abs() <= 2.0e-9,
                        "variable_order={variable_order:?}; residual_order={residual_order:?}; \
                         geometry={geometry:?}; baseline={baseline:?}"
                    );
                }
            } else {
                baseline_geometry = Some(geometry.clone());
            }
            let audit_labels: Vec<_> = report
                .audit
                .sources
                .iter()
                .map(|source| source.source_label.as_str())
                .collect();
            assert_eq!(audit_labels, SOURCE_LABELS);
            assert_flat_audit_matches_dense_layout(&problem);
            let static_audit_labels: Vec<_> = problem
                .audit_rows()
                .unwrap()
                .iter()
                .map(|row| row.source_label.clone())
                .collect();
            let expected_flat_labels: Vec<_> = residual_order
                .iter()
                .map(|&equation| SOURCE_LABELS[equation])
                .collect();
            assert_eq!(static_audit_labels, expected_flat_labels);
            assert_eq!(
                source_labels(&problem, &report.redundant_sources),
                &SOURCE_LABELS[3..]
            );
            assert!(report.conflicting_sources.is_empty());
        }
    }
}

#[test]
fn jacobian_checker_covers_every_valid_m3_residual_and_variable_kind() {
    let case = construct_linear_case(
        LinearShape::Exact,
        2,
        [0, 2, 1, 1],
        &[1; 96],
        &[1; 6],
        &[2; 6],
    );
    let (affine, _) = linear_problem(&case);
    assert_jacobians(&affine);

    let (circle, _, _) = circle_fixture(
        Similarity {
            scale: 1.0,
            rotation: 0.2,
            translation: [1.0, -3.0],
        },
        [0.2, -0.1],
    );
    assert_jacobians(&circle);

    let mut mixed = Problem::new();
    let scalar = mixed.add_variable(VariableBlock::scalar(0.7, 0.5).unwrap());
    let vector = mixed.add_variable(VariableBlock::vec2([1.2, -0.4], [0.8, 1.1]).unwrap());
    let pose = mixed.add_variable(VariableBlock::pose2([0.3, 0.9, 0.4], [0.6, 0.7, 0.2]).unwrap());
    let source = add_source(&mut mixed, "mixed variable kinds");
    mixed
        .add_residual(
            ResidualBlock::new(
                source,
                ResidualCategory::Hard,
                vec![scalar, vector, pose],
                2,
                vec![1.3, 0.9],
                vec![
                    audit_row("mixed row zero", "s,v,p"),
                    audit_row("mixed row one", "s,v,p"),
                ],
                MixedKinds { target: [2.5, 1.0] },
            )
            .unwrap(),
        )
        .unwrap();
    let mixed_report = mixed.check_jacobians(support::FD_STEP).unwrap();
    assert_eq!(
        mixed_report
            .blocks
            .iter()
            .map(|block| block.columns)
            .collect::<Vec<_>>(),
        vec![1, 2, 3]
    );
    assert!(
        mixed_report.all_within(support::FD_TOLERANCE),
        "{mixed_report:#?}"
    );

    let mut quadratic = Problem::new();
    let variable = quadratic.add_variable(VariableBlock::scalar(0.8, 1.0).unwrap());
    let source = add_source(&mut quadratic, "quadratic checker fixture");
    quadratic
        .add_residual(
            ResidualBlock::new(
                source,
                ResidualCategory::Hard,
                vec![variable],
                1,
                vec![1.0],
                vec![audit_row("x^2 - target", "x")],
                ScalarQuadratic(1.0),
            )
            .unwrap(),
        )
        .unwrap();
    assert_jacobians(&quadratic);
}

#[derive(Clone, Copy, Debug)]
enum NonFiniteFailure {
    Residual,
    Jacobian,
}

#[derive(Clone, Debug)]
struct NonFiniteEvaluator(NonFiniteFailure);

impl ResidualEvaluator for NonFiniteEvaluator {
    fn evaluate(&self, _variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        match self.0 {
            NonFiniteFailure::Residual => Ok(vec![f64::NAN]),
            NonFiniteFailure::Jacobian => Ok(vec![1.0]),
        }
    }

    fn jacobian(
        &self,
        _variables: &[VariableValue],
    ) -> Result<Vec<LocalJacobian>, EvaluationError> {
        let value = match self.0 {
            NonFiniteFailure::Residual => 1.0,
            NonFiniteFailure::Jacobian => f64::INFINITY,
        };
        Ok(vec![LocalJacobian::new(1, 1, vec![value])])
    }
}

fn nonfinite_problem(mode: NonFiniteFailure) -> (Problem, VariableId) {
    let mut problem = Problem::new();
    let variable = problem.add_variable(VariableBlock::scalar(2.0, 1.0).unwrap());
    let source = add_source(&mut problem, "non-finite failure injection");
    problem
        .add_residual(
            ResidualBlock::new(
                source,
                ResidualCategory::Hard,
                vec![variable],
                1,
                vec![1.0],
                vec![audit_row("injected non-finite row", "x")],
                NonFiniteEvaluator(mode),
            )
            .unwrap(),
        )
        .unwrap();
    (problem, variable)
}

#[derive(Clone, Debug)]
struct RejectEveryTrial;

impl ResidualEvaluator for RejectEveryTrial {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        let [VariableValue::Scalar(value)] = variables else {
            return Err(EvaluationError::invalid_geometry("expected one scalar"));
        };
        if *value == 0.0 {
            Ok(vec![-1.0])
        } else {
            Err(EvaluationError::invalid_geometry(
                "injected trial-state rejection",
            ))
        }
    }

    fn jacobian(
        &self,
        _variables: &[VariableValue],
    ) -> Result<Vec<LocalJacobian>, EvaluationError> {
        Ok(vec![LocalJacobian::new(1, 1, vec![1.0])])
    }
}

#[derive(Clone, Copy, Debug)]
enum PostAcceptFailureMode {
    InvalidGeometry,
    NonFiniteResidual,
}

#[derive(Clone, Debug)]
struct AcceptOneThenFail(PostAcceptFailureMode);

impl ResidualEvaluator for AcceptOneThenFail {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        let [VariableValue::Scalar(value)] = variables else {
            return Err(EvaluationError::invalid_geometry("expected one scalar"));
        };
        if *value <= 1.0 + 1.0e-12 {
            return Ok(vec![value - 3.0]);
        }
        match self.0 {
            PostAcceptFailureMode::InvalidGeometry => Err(EvaluationError::invalid_geometry(
                "injected failure after one accepted step",
            )),
            PostAcceptFailureMode::NonFiniteResidual => Ok(vec![f64::NAN]),
        }
    }

    fn jacobian(
        &self,
        _variables: &[VariableValue],
    ) -> Result<Vec<LocalJacobian>, EvaluationError> {
        Ok(vec![LocalJacobian::new(1, 1, vec![1.0])])
    }
}

#[test]
fn failure_injection_rejects_nonfinite_values_invalid_scales_and_singular_systems() {
    for mode in [NonFiniteFailure::Residual, NonFiniteFailure::Jacobian] {
        let (mut problem, variable) = nonfinite_problem(mode);
        let report = problem.solve(SolverConfig::default()).unwrap();
        assert_eq!(report.termination, SolveTermination::NumericalFailure);
        assert!((scalar_value(&problem, variable) - 2.0).abs() <= f64::EPSILON);
        assert_eq!(report.accepted_state.ambient().as_slice(), &[2.0]);
        assert_report_finite(&report);
    }

    for scale in [0.0, -1.0] {
        assert!(matches!(
            VariableBlock::scalar(0.0, scale),
            Err(CoreError::InvalidScale { .. })
        ));
        let mut problem = Problem::new();
        let variable = problem.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap());
        let source = add_source(&mut problem, "invalid residual scale");
        assert!(matches!(
            ResidualBlock::new(
                source,
                ResidualCategory::Hard,
                vec![variable],
                1,
                vec![scale],
                vec![audit_row("invalid scale", "x")],
                AffineScalars {
                    matrix: vec![vec![1.0]],
                    target: vec![0.0],
                },
            ),
            Err(CoreError::InvalidScale { .. })
        ));
    }

    let singular_case = LinearCase {
        shape: LinearShape::Exact,
        rows: 2,
        columns: 2,
        rank: 1,
        matrix: vec![vec![1.0, 1.0], vec![2.0, 2.0]],
        target: vec![3.0, 6.0],
        witness: vec![1.0, 2.0],
        initial: vec![0.0, 0.0],
        row_permutation: vec![0, 1],
        column_permutation: vec![0, 1],
    };
    let (mut singular, variables) = linear_problem(&singular_case);
    let report = singular.solve(SolverConfig::default()).unwrap();
    let values: Vec<_> = variables
        .iter()
        .map(|&variable| scalar_value(&singular, variable))
        .collect();
    assert_eq!(report.termination, SolveTermination::Converged);
    assert_eq!((report.rank, report.local_degrees_of_freedom), (1, 1));
    assert!(report.is_singular);
    assert!(linear_oracle(&singular_case, &values) <= 1.0e-9);
    assert_report_finite(&report);
}

#[test]
fn repeated_rejected_steps_stagnate_without_committing_invalid_trials() {
    let mut problem = Problem::new();
    let variable = problem.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap());
    let source = add_source(&mut problem, "reject every nonzero trial");
    problem
        .add_residual(
            ResidualBlock::new(
                source,
                ResidualCategory::Hard,
                vec![variable],
                1,
                vec![1.0],
                vec![audit_row("x - 1 with trial boundary", "x")],
                RejectEveryTrial,
            )
            .unwrap(),
        )
        .unwrap();
    let report = problem
        .solve(SolverConfig {
            maximum_damping: 10.0,
            max_iterations: 20,
            ..SolverConfig::default()
        })
        .unwrap();
    assert_eq!(report.termination, SolveTermination::Stalled);
    assert!(report.trace.records.len() >= 4, "{report:#?}");
    assert!(
        report
            .trace
            .records
            .iter()
            .all(|record| !record.accepted && !record.trial_valid)
    );
    assert!(scalar_value(&problem, variable).abs() <= f64::EPSILON);
    assert_eq!(report.accepted_state.ambient().as_slice(), &[0.0]);
    assert!((report.hard_residual_max - 1.0).abs() <= f64::EPSILON);
    assert_trace_invariants(&report, 0.5);
    assert_report_finite(&report);
}

#[test]
fn accepted_intermediate_state_survives_later_invalid_and_nonfinite_trials() {
    for mode in [
        PostAcceptFailureMode::InvalidGeometry,
        PostAcceptFailureMode::NonFiniteResidual,
    ] {
        let mut problem = Problem::new();
        let variable = problem.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap());
        let source = add_source(&mut problem, "accept one step then fail");
        problem
            .add_residual(
                ResidualBlock::new(
                    source,
                    ResidualCategory::Hard,
                    vec![variable],
                    1,
                    vec![1.0],
                    vec![audit_row("x - 3 with post-accept failure", "x")],
                    AcceptOneThenFail(mode),
                )
                .unwrap(),
            )
            .unwrap();

        let report = problem
            .solve(SolverConfig {
                maximum_damping: 10.0,
                max_iterations: 20,
                ..SolverConfig::default()
            })
            .unwrap();
        assert_eq!(report.termination, SolveTermination::Stalled, "{mode:?}");
        assert!(report.trace.records.len() >= 2, "{mode:?}: {report:#?}");
        assert!(report.trace.records[0].accepted, "{mode:?}: {report:#?}");
        assert!(
            report.trace.records[1..]
                .iter()
                .all(|record| !record.accepted && !record.trial_valid),
            "{mode:?}: {report:#?}"
        );
        let value = scalar_value(&problem, variable);
        assert!(value > 0.5, "must retain an accepted non-initial state");
        assert!((value - 1.0).abs() <= 1.0e-12, "{mode:?}: {value:e}");
        assert_eq!(report.accepted_state, problem.packed_state().unwrap());
        let independent_residual = value - 3.0;
        assert!(
            (report.hard_residual_max - independent_residual.abs()).abs() <= f64::EPSILON,
            "{mode:?}: {report:#?}"
        );
        assert_trace_invariants(&report, 0.5 * independent_residual * independent_residual);
        assert_report_finite(&report);
    }
}

fn quadratic_problem(initial: f64, target: f64) -> (Problem, VariableId) {
    let mut problem = Problem::new();
    let variable = problem.add_variable(VariableBlock::scalar(initial, 1.0).unwrap());
    let source = add_source(&mut problem, "quadratic trace fixture");
    problem
        .add_residual(
            ResidualBlock::new(
                source,
                ResidualCategory::Hard,
                vec![variable],
                1,
                vec![1.0],
                vec![audit_row("x^2 - target", "x")],
                ScalarQuadratic(target),
            )
            .unwrap(),
        )
        .unwrap();
    (problem, variable)
}

fn quadratic_cost(value: f64, target: f64) -> f64 {
    0.5 * (value * value - target).powi(2)
}

#[test]
fn trace_invariants_hold_and_valid_rejected_state_is_never_committed() {
    let (mut problem, variable) = quadratic_problem(0.1, 1.0);
    let initial = problem.packed_state().unwrap();
    let report = problem
        .solve(SolverConfig {
            initial_damping: 1.0e-8,
            minimum_damping: 1.0e-10,
            maximum_damping: 1.0e-8,
            max_block_normalized_step: 10.0,
            ..SolverConfig::default()
        })
        .unwrap();
    assert_eq!(report.termination, SolveTermination::Stalled);
    assert_eq!(report.trace.records.len(), 1);
    let rejected = &report.trace.records[0];
    assert!(!rejected.accepted && rejected.trial_valid);
    assert!(rejected.trial_cost > rejected.cost_before);
    assert_eq!(problem.packed_state().unwrap(), initial);
    assert_eq!(report.accepted_state, initial);
    let value = scalar_value(&problem, variable);
    let independent_residual = value * value - 1.0;
    assert!((report.hard_residual_max - independent_residual.abs()).abs() <= f64::EPSILON);
    assert_trace_invariants(&report, quadratic_cost(value, 1.0));
    assert_report_finite(&report);

    let (mut converging, variable) = quadratic_problem(0.1, 1.0);
    let report = converging
        .solve(SolverConfig {
            initial_damping: 1.0e-8,
            max_block_normalized_step: 10.0,
            max_iterations: 100,
            ..SolverConfig::default()
        })
        .unwrap();
    assert_eq!(report.termination, SolveTermination::Converged);
    assert!(report.trace.records.iter().any(|record| record.accepted));
    assert!(
        report
            .trace
            .records
            .iter()
            .any(|record| !record.accepted && record.trial_valid)
    );
    let value = scalar_value(&converging, variable);
    assert_eq!(
        report.accepted_state,
        converging.packed_state().unwrap(),
        "returned state must be the committed accepted state"
    );
    assert!((value * value - 1.0).abs() <= 1.0e-9);
    assert_trace_invariants(&report, quadratic_cost(value, 1.0));
    assert_report_finite(&report);
}

#[test]
fn iteration_limit_returns_the_last_finite_independently_evaluated_state() {
    let (mut problem, variable) = quadratic_problem(1.0, 2.0);
    let report = problem
        .solve(SolverConfig {
            max_iterations: 1,
            ..SolverConfig::default()
        })
        .unwrap();
    let value = scalar_value(&problem, variable);
    let independent_residual = value * value - 2.0;
    assert_eq!(report.termination, SolveTermination::IterationLimit);
    assert_eq!(report.trace.records.len(), 1);
    assert!(report.trace.records[0].accepted);
    assert!(report.hard_residuals_validated);
    assert!((report.hard_residual_max - independent_residual.abs()).abs() <= f64::EPSILON);
    assert_eq!(report.accepted_state, problem.packed_state().unwrap());
    assert_trace_invariants(&report, quadratic_cost(value, 2.0));
    assert_report_finite(&report);
}
