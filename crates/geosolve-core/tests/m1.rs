use std::f64::consts::PI;

use approx::assert_relative_eq;
use geosolve_core::{
    AuditBinding, CoreError, EvaluationError, LocalJacobian, Problem, ResidualBlock,
    ResidualCategory, ResidualEvaluator, ResidualRowAudit, SourceConstraint, SourceConstraintId,
    VariableBlock, VariableId, VariableKind, VariableValue,
};

const FD_STEP: f64 = 1.0e-5;
const FD_TOLERANCE: f64 = 1.0e-6;

fn row(template: &str, name: &str, value: &str, unit: &str) -> ResidualRowAudit {
    ResidualRowAudit::new(template, vec![AuditBinding::new(name, value)], unit)
}

fn add_source(problem: &mut Problem, label: &str) -> SourceConstraintId {
    problem.add_source(SourceConstraint::new(label).unwrap())
}

fn scalar_value(value: VariableValue) -> f64 {
    match value {
        VariableValue::Scalar(value) => value,
        other => panic!("expected scalar, got {other:?}"),
    }
}

#[derive(Debug)]
struct ScalarQuadratic {
    target: f64,
}

impl ResidualEvaluator for ScalarQuadratic {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        let [VariableValue::Scalar(value)] = variables else {
            return Err(EvaluationError::invalid_geometry("expected one scalar"));
        };
        Ok(vec![value * value - self.target])
    }

    fn jacobian(&self, variables: &[VariableValue]) -> Result<Vec<LocalJacobian>, EvaluationError> {
        let [VariableValue::Scalar(value)] = variables else {
            return Err(EvaluationError::invalid_geometry("expected one scalar"));
        };
        Ok(vec![LocalJacobian::new(1, 1, vec![2.0 * value])])
    }
}

#[derive(Debug)]
struct Distance {
    target: f64,
}

impl ResidualEvaluator for Distance {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        let [VariableValue::Vec2(a), VariableValue::Vec2(b)] = variables else {
            return Err(EvaluationError::invalid_geometry(
                "expected two Vec2 blocks",
            ));
        };
        let dx = b[0] - a[0];
        let dy = b[1] - a[1];
        Ok(vec![dx.hypot(dy) - self.target])
    }

    fn jacobian(&self, variables: &[VariableValue]) -> Result<Vec<LocalJacobian>, EvaluationError> {
        let [VariableValue::Vec2(a), VariableValue::Vec2(b)] = variables else {
            return Err(EvaluationError::invalid_geometry(
                "expected two Vec2 blocks",
            ));
        };
        let dx = b[0] - a[0];
        let dy = b[1] - a[1];
        let distance = dx.hypot(dy);
        if distance <= 1.0e-14 {
            return Err(EvaluationError::invalid_geometry(
                "distance derivative is undefined at coincident points",
            ));
        }
        let ux = dx / distance;
        let uy = dy / distance;
        Ok(vec![
            LocalJacobian::new(1, 2, vec![-ux, -uy]),
            LocalJacobian::new(1, 2, vec![ux, uy]),
        ])
    }
}

#[derive(Debug)]
struct TransformedPoint {
    local: [f64; 2],
    target: [f64; 2],
}

impl ResidualEvaluator for TransformedPoint {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        let [VariableValue::Pose2(pose)] = variables else {
            return Err(EvaluationError::invalid_geometry(
                "expected one Pose2 block",
            ));
        };
        let (sin, cos) = pose[2].sin_cos();
        let x = pose[0] + cos * self.local[0] - sin * self.local[1];
        let y = pose[1] + sin * self.local[0] + cos * self.local[1];
        Ok(vec![x - self.target[0], y - self.target[1]])
    }

    fn jacobian(&self, variables: &[VariableValue]) -> Result<Vec<LocalJacobian>, EvaluationError> {
        let [VariableValue::Pose2(pose)] = variables else {
            return Err(EvaluationError::invalid_geometry(
                "expected one Pose2 block",
            ));
        };
        let (sin, cos) = pose[2].sin_cos();
        let angle_derivative = [
            -sin * self.local[0] - cos * self.local[1],
            cos * self.local[0] - sin * self.local[1],
        ];
        Ok(vec![LocalJacobian::new(
            2,
            3,
            vec![1.0, 0.0, angle_derivative[0], 0.0, 1.0, angle_derivative[1]],
        )])
    }
}

#[derive(Debug)]
struct Heterogeneous;

impl ResidualEvaluator for Heterogeneous {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        let [
            VariableValue::Scalar(scalar),
            VariableValue::Vec2(vector),
            VariableValue::Pose2(pose),
        ] = variables
        else {
            return Err(EvaluationError::invalid_geometry(
                "expected Scalar, Vec2, and Pose2",
            ));
        };
        Ok(vec![
            scalar + vector[0] + pose[0],
            2.0 * scalar + vector[1] + pose[2],
        ])
    }

    fn jacobian(
        &self,
        _variables: &[VariableValue],
    ) -> Result<Vec<LocalJacobian>, EvaluationError> {
        Ok(vec![
            LocalJacobian::new(2, 1, vec![1.0, 2.0]),
            LocalJacobian::new(2, 2, vec![1.0, 0.0, 0.0, 1.0]),
            LocalJacobian::new(2, 3, vec![1.0, 0.0, 0.0, 0.0, 0.0, 1.0]),
        ])
    }
}

#[derive(Debug)]
struct Identity {
    offset: f64,
}

impl ResidualEvaluator for Identity {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        let [VariableValue::Scalar(value)] = variables else {
            return Err(EvaluationError::invalid_geometry("expected one scalar"));
        };
        Ok(vec![value - self.offset])
    }

    fn jacobian(
        &self,
        _variables: &[VariableValue],
    ) -> Result<Vec<LocalJacobian>, EvaluationError> {
        Ok(vec![LocalJacobian::new(1, 1, vec![1.0])])
    }
}

fn identity_residual(
    source: SourceConstraintId,
    variable: VariableId,
    category: ResidualCategory,
    offset: f64,
) -> ResidualBlock {
    ResidualBlock::new(
        source,
        category,
        vec![variable],
        1,
        vec![1.0],
        vec![row("(x - target) / scale", "x", "scalar", "1")],
        Identity { offset },
    )
    .unwrap()
}

#[test]
fn scalar_quadratic_is_dimensionless_and_matches_finite_difference() {
    let mut problem = Problem::new();
    let variable = problem.add_variable(VariableBlock::scalar(3.0, 0.5).unwrap());
    let source = add_source(&mut problem, "scalar quadratic");
    let residual = problem
        .add_residual(
            ResidualBlock::new(
                source,
                ResidualCategory::Hard,
                vec![variable],
                1,
                vec![2.0],
                vec![row("(x^2 - target) / scale", "x", "3", "m^2")],
                ScalarQuadratic { target: 4.0 },
            )
            .unwrap(),
        )
        .unwrap();

    let assembly = problem.assemble_dense().unwrap();
    assert_eq!(assembly.residual_range(residual), Some(0..1));
    assert_relative_eq!(assembly.residuals()[0], 2.5, epsilon = 1.0e-14);
    assert_relative_eq!(assembly.jacobian()[(0, 0)], 1.5, epsilon = 1.0e-14);

    let report = problem.check_jacobians(FD_STEP).unwrap();
    assert_eq!(report.blocks.len(), 1);
    assert!(report.all_within(FD_TOLERANCE), "{report:#?}");

    let audit = problem.audit_rows().unwrap();
    assert_eq!(audit.len(), 1);
    assert_eq!(audit[0].residual_id, residual);
    assert_eq!(audit[0].source_id, source);
    assert_eq!(audit[0].source_label, "scalar quadratic");
    assert_eq!(audit[0].category, ResidualCategory::Hard);
    assert_eq!(audit[0].template, "(x^2 - target) / scale");
    assert_eq!(audit[0].bindings[0], AuditBinding::new("x", "3"));
    assert_eq!(audit[0].unit, "m^2");
    assert_relative_eq!(audit[0].scale, 2.0, epsilon = f64::EPSILON);
}

#[test]
fn two_variable_distance_uses_ordered_incidence_and_matches_finite_difference() {
    let mut problem = Problem::new();
    let a = problem.add_variable(VariableBlock::vec2([1.0, 2.0], [2.0, 3.0]).unwrap());
    let b = problem.add_variable(VariableBlock::vec2([4.0, 6.0], [4.0, 5.0]).unwrap());
    let source = add_source(&mut problem, "distance A B");
    problem
        .add_residual(
            ResidualBlock::new(
                source,
                ResidualCategory::Hard,
                vec![a, b],
                1,
                vec![5.0],
                vec![ResidualRowAudit::new(
                    "(norm(B - A) - target) / scale",
                    vec![
                        AuditBinding::new("A", "point A"),
                        AuditBinding::new("B", "point B"),
                    ],
                    "m",
                )],
                Distance { target: 5.0 },
            )
            .unwrap(),
        )
        .unwrap();

    let assembly = problem.assemble_dense().unwrap();
    assert_relative_eq!(assembly.residuals()[0], 0.0, epsilon = 1.0e-14);
    assert_relative_eq!(assembly.jacobian()[(0, 0)], -0.24, epsilon = 1.0e-14);
    assert_relative_eq!(assembly.jacobian()[(0, 1)], -0.48, epsilon = 1.0e-14);
    assert_relative_eq!(assembly.jacobian()[(0, 2)], 0.48, epsilon = 1.0e-14);
    assert_relative_eq!(assembly.jacobian()[(0, 3)], 0.8, epsilon = 1.0e-14);

    let report = problem.check_jacobians(FD_STEP).unwrap();
    assert_eq!(report.blocks.len(), 2);
    assert!(report.all_within(FD_TOLERANCE), "{report:#?}");
}

#[test]
fn pose2_transformed_point_uses_unwrapped_local_increment_and_matches_fd() {
    let mut problem = Problem::new();
    let pose =
        problem.add_variable(VariableBlock::pose2([1.0, -2.0, 0.3], [0.5, 0.25, 0.2]).unwrap());
    let source = add_source(&mut problem, "transformed point");
    problem
        .add_residual(
            ResidualBlock::new(
                source,
                ResidualCategory::Hard,
                vec![pose],
                2,
                vec![2.0, 3.0],
                vec![
                    row("(world(local).x - target.x) / scale", "body", "pose", "m"),
                    row("(world(local).y - target.y) / scale", "body", "pose", "m"),
                ],
                TransformedPoint {
                    local: [0.7, -0.4],
                    target: [1.5, -1.5],
                },
            )
            .unwrap(),
        )
        .unwrap();

    let report = problem.check_jacobians(FD_STEP).unwrap();
    assert_eq!(report.blocks.len(), 1);
    assert_eq!(report.blocks[0].rows, 2);
    assert_eq!(report.blocks[0].columns, 3);
    assert!(report.all_within(FD_TOLERANCE), "{report:#?}");

    problem
        .apply_local_increment(pose, &[1.0, 2.0, 4.0 * PI])
        .unwrap();
    assert_eq!(
        problem.variable(pose).unwrap().value(),
        VariableValue::Pose2([2.0, 0.0, 0.3 + 4.0 * PI])
    );
}

#[test]
fn mixed_blocks_pack_deterministically_and_apply_local_plus() {
    let mut problem = Problem::new();
    let scalar = problem.add_variable(VariableBlock::scalar(2.0, 0.1).unwrap());
    let vector = problem.add_variable(VariableBlock::vec2([3.0, 4.0], [0.2, 0.3]).unwrap());
    let pose =
        problem.add_variable(VariableBlock::pose2([5.0, 6.0, 7.0], [0.4, 0.5, 0.6]).unwrap());

    let first = problem.packed_state().unwrap();
    let second = problem.packed_state().unwrap();
    assert_eq!(first, second);
    assert_eq!(first.layout().ambient_dimension(), 6);
    assert_eq!(first.layout().tangent_dimension(), 6);
    assert_eq!(first.layout().block(scalar).unwrap().ambient_range, 0..1);
    assert_eq!(first.layout().block(scalar).unwrap().tangent_range, 0..1);
    assert_eq!(first.layout().block(vector).unwrap().ambient_range, 1..3);
    assert_eq!(first.layout().block(vector).unwrap().tangent_range, 1..3);
    assert_eq!(first.layout().block(pose).unwrap().ambient_range, 3..6);
    assert_eq!(first.layout().block(pose).unwrap().tangent_range, 3..6);
    assert_eq!(first.ambient().as_slice(), &[2.0, 3.0, 4.0, 5.0, 6.0, 7.0]);

    problem.apply_local_increment(scalar, &[0.5]).unwrap();
    problem.apply_local_increment(vector, &[1.0, -2.0]).unwrap();
    problem
        .apply_local_increment(pose, &[3.0, 4.0, 5.0])
        .unwrap();
    assert_eq!(
        problem.packed_state().unwrap().ambient().as_slice(),
        &[2.5, 4.0, 2.0, 8.0, 10.0, 12.0]
    );
}

#[test]
fn heterogeneous_block_assembles_into_correct_scaled_matrix_ranges() {
    let mut problem = Problem::new();
    let scalar = problem.add_variable(VariableBlock::scalar(2.0, 2.0).unwrap());
    let vector = problem.add_variable(VariableBlock::vec2([3.0, 5.0], [3.0, 4.0]).unwrap());
    let pose =
        problem.add_variable(VariableBlock::pose2([7.0, 11.0, 13.0], [5.0, 6.0, 7.0]).unwrap());
    let source = add_source(&mut problem, "heterogeneous equation");
    let residual = problem
        .add_residual(
            ResidualBlock::new(
                source,
                ResidualCategory::Temporary,
                vec![scalar, vector, pose],
                2,
                vec![10.0, 20.0],
                vec![
                    row("(s + v.x + p.x) / scale", "variables", "s,v,p", "m"),
                    row("(2s + v.y + p.angle) / scale", "variables", "s,v,p", "rad"),
                ],
                Heterogeneous,
            )
            .unwrap(),
        )
        .unwrap();

    let assembly = problem.assemble_dense().unwrap();
    assert_eq!(assembly.residual_range(residual), Some(0..2));
    assert_eq!(
        assembly
            .variable_layout()
            .block(scalar)
            .unwrap()
            .tangent_range,
        0..1
    );
    assert_eq!(
        assembly
            .variable_layout()
            .block(vector)
            .unwrap()
            .tangent_range,
        1..3
    );
    assert_eq!(
        assembly
            .variable_layout()
            .block(pose)
            .unwrap()
            .tangent_range,
        3..6
    );
    assert_eq!(assembly.jacobian().shape(), (2, 6));
    assert_relative_eq!(assembly.residuals()[0], 1.2, epsilon = 1.0e-14);
    assert_relative_eq!(assembly.residuals()[1], 1.1, epsilon = 1.0e-14);
    let expected = [
        [0.2, 0.3, 0.0, 0.5, 0.0, 0.0],
        [0.2, 0.0, 0.2, 0.0, 0.0, 0.35],
    ];
    for row in 0..2 {
        for column in 0..6 {
            assert_relative_eq!(
                assembly.jacobian()[(row, column)],
                expected[row][column],
                epsilon = 1.0e-14
            );
        }
    }

    let report = problem.check_jacobians(FD_STEP).unwrap();
    assert_eq!(report.blocks.len(), 3);
    assert!(report.all_within(FD_TOLERANCE), "{report:#?}");
}

#[test]
fn generational_ids_survive_unrelated_removal_and_slot_reuse() {
    let mut problem = Problem::new();
    let v1 = problem.add_variable(VariableBlock::scalar(1.0, 1.0).unwrap());
    let v2 = problem.add_variable(VariableBlock::scalar(2.0, 1.0).unwrap());
    let v3 = problem.add_variable(VariableBlock::scalar(3.0, 1.0).unwrap());
    let s1 = add_source(&mut problem, "source one");
    let s2 = add_source(&mut problem, "source two");
    let s3 = add_source(&mut problem, "source three");
    let r1 = problem
        .add_residual(identity_residual(s1, v1, ResidualCategory::Hard, 0.0))
        .unwrap();
    let r2 = problem
        .add_residual(identity_residual(s2, v2, ResidualCategory::Temporary, 0.0))
        .unwrap();
    let r3 = problem
        .add_residual(identity_residual(s3, v3, ResidualCategory::Preference, 0.0))
        .unwrap();

    problem.remove_residual(r2).unwrap();
    problem.remove_variable(v2).unwrap();
    problem.remove_source(s2).unwrap();

    let v4 = problem.add_variable(VariableBlock::scalar(4.0, 1.0).unwrap());
    let s4 = add_source(&mut problem, "source four");
    let r4 = problem
        .add_residual(identity_residual(s4, v4, ResidualCategory::Temporary, 0.0))
        .unwrap();

    assert_ne!(v2, v4);
    assert_ne!(s2, s4);
    assert_ne!(r2, r4);
    assert!(problem.variable(v2).is_none());
    assert!(problem.source(s2).is_none());
    assert!(problem.residual(r2).is_none());
    assert_relative_eq!(
        scalar_value(problem.variable(v1).unwrap().value()),
        1.0,
        epsilon = f64::EPSILON
    );
    assert_relative_eq!(
        scalar_value(problem.variable(v3).unwrap().value()),
        3.0,
        epsilon = f64::EPSILON
    );
    assert_eq!(problem.source(s1).unwrap().label(), "source one");
    assert_eq!(problem.source(s3).unwrap().label(), "source three");
    assert!(problem.residual(r1).is_some());
    assert!(problem.residual(r3).is_some());

    let layout_ids: Vec<_> = problem
        .packed_layout()
        .unwrap()
        .blocks()
        .iter()
        .map(|block| block.variable_id)
        .collect();
    assert_eq!(layout_ids, vec![v1, v3, v4]);
    let assembly = problem.assemble_dense().unwrap();
    let residual_ids: Vec<_> = assembly
        .residual_layout()
        .iter()
        .map(|block| block.residual_id)
        .collect();
    assert_eq!(residual_ids, vec![r1, r3, r4]);
    let audit = problem.audit_rows().unwrap();
    assert_eq!(
        audit.iter().map(|item| item.source_id).collect::<Vec<_>>(),
        vec![s1, s3, s4]
    );
    assert_eq!(
        audit.iter().map(|item| item.category).collect::<Vec<_>>(),
        vec![
            ResidualCategory::Hard,
            ResidualCategory::Preference,
            ResidualCategory::Temporary,
        ]
    );
    let report = problem.check_jacobians(FD_STEP).unwrap();
    assert!(report.all_within(FD_TOLERANCE), "{report:#?}");
}

#[derive(Clone, Copy, Debug)]
enum FailureMode {
    NonFiniteResidual,
    NonFiniteJacobian,
    InvalidGeometry,
    WrongResidualDimension,
    WrongJacobianBlocks,
    WrongJacobianShape,
}

#[derive(Debug)]
struct FailureEvaluator(FailureMode);

impl ResidualEvaluator for FailureEvaluator {
    fn evaluate(&self, _variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        match self.0 {
            FailureMode::NonFiniteResidual => Ok(vec![f64::NAN]),
            FailureMode::InvalidGeometry => Err(EvaluationError::invalid_geometry(
                "degenerate synthetic geometry",
            )),
            FailureMode::WrongResidualDimension => Ok(vec![0.0, 1.0]),
            _ => Ok(vec![0.0]),
        }
    }

    fn jacobian(
        &self,
        _variables: &[VariableValue],
    ) -> Result<Vec<LocalJacobian>, EvaluationError> {
        match self.0 {
            FailureMode::NonFiniteJacobian => {
                Ok(vec![LocalJacobian::new(1, 1, vec![f64::INFINITY])])
            }
            FailureMode::InvalidGeometry => Err(EvaluationError::invalid_geometry(
                "degenerate synthetic geometry",
            )),
            FailureMode::WrongJacobianBlocks => Ok(Vec::new()),
            FailureMode::WrongJacobianShape => Ok(vec![LocalJacobian::new(2, 1, vec![1.0, 1.0])]),
            _ => Ok(vec![LocalJacobian::new(1, 1, vec![1.0])]),
        }
    }
}

fn failure_problem(mode: FailureMode) -> Problem {
    let mut problem = Problem::new();
    let variable = problem.add_variable(VariableBlock::scalar(1.0, 1.0).unwrap());
    let source = add_source(&mut problem, "failure injection");
    problem
        .add_residual(
            ResidualBlock::new(
                source,
                ResidualCategory::Hard,
                vec![variable],
                1,
                vec![1.0],
                vec![row("x / scale", "x", "scalar", "1")],
                FailureEvaluator(mode),
            )
            .unwrap(),
        )
        .unwrap();
    problem
}

#[test]
fn invalid_scales_and_non_finite_variables_are_rejected() {
    assert!(matches!(
        VariableBlock::scalar(f64::NAN, 1.0),
        Err(CoreError::NonFiniteValue { .. })
    ));
    assert!(matches!(
        VariableBlock::scalar(f64::INFINITY, 1.0),
        Err(CoreError::NonFiniteValue { .. })
    ));
    for scale in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        assert!(matches!(
            VariableBlock::scalar(0.0, scale),
            Err(CoreError::InvalidScale { .. })
        ));
    }
    assert!(matches!(
        VariableBlock::new(VariableValue::Vec2([0.0, 0.0]), vec![1.0]),
        Err(CoreError::DimensionMismatch { .. })
    ));

    let mut problem = Problem::new();
    let variable = problem.add_variable(VariableBlock::scalar(1.0, 1.0).unwrap());
    let source = add_source(&mut problem, "bad scale");
    for scale in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        assert!(matches!(
            ResidualBlock::new(
                source,
                ResidualCategory::Hard,
                vec![variable],
                1,
                vec![scale],
                vec![row("x / scale", "x", "scalar", "1")],
                Identity { offset: 0.0 },
            ),
            Err(CoreError::InvalidScale { .. })
        ));
    }

    assert!(matches!(
        problem.apply_local_increment(variable, &[f64::INFINITY]),
        Err(CoreError::NonFiniteValue { .. })
    ));
    assert_relative_eq!(
        scalar_value(problem.variable(variable).unwrap().value()),
        1.0,
        epsilon = f64::EPSILON
    );
}

#[test]
fn evaluator_failures_are_rejected_before_dense_data_is_returned() {
    assert!(matches!(
        failure_problem(FailureMode::NonFiniteResidual).assemble_dense(),
        Err(CoreError::NonFiniteValue { .. })
    ));
    assert!(matches!(
        failure_problem(FailureMode::NonFiniteJacobian).assemble_dense(),
        Err(CoreError::NonFiniteValue { .. })
    ));
    assert!(matches!(
        failure_problem(FailureMode::InvalidGeometry).assemble_dense(),
        Err(CoreError::InvalidGeometry { .. })
    ));
    assert!(matches!(
        failure_problem(FailureMode::WrongResidualDimension).assemble_dense(),
        Err(CoreError::DimensionMismatch { .. })
    ));
    assert!(matches!(
        failure_problem(FailureMode::WrongJacobianBlocks).assemble_dense(),
        Err(CoreError::DimensionMismatch { .. })
    ));
    assert!(matches!(
        failure_problem(FailureMode::WrongJacobianShape).assemble_dense(),
        Err(CoreError::DimensionMismatch { .. })
    ));
    assert!(matches!(
        failure_problem(FailureMode::InvalidGeometry).check_jacobians(FD_STEP),
        Err(CoreError::InvalidGeometry { .. })
    ));
}

#[test]
fn invalid_dimensions_ids_and_audit_metadata_are_rejected() {
    let mut problem = Problem::new();
    let stale_variable = problem.add_variable(VariableBlock::scalar(1.0, 1.0).unwrap());
    problem.remove_variable(stale_variable).unwrap();
    let source = add_source(&mut problem, "active source");
    let unknown_variable = identity_residual(source, stale_variable, ResidualCategory::Hard, 0.0);
    assert!(matches!(
        problem.add_residual(unknown_variable),
        Err(CoreError::UnknownVariable(id)) if id == stale_variable
    ));

    let active_variable = problem.add_variable(VariableBlock::scalar(2.0, 1.0).unwrap());
    let stale_source = add_source(&mut problem, "stale source");
    problem.remove_source(stale_source).unwrap();
    assert!(matches!(
        problem.add_residual(identity_residual(
            stale_source,
            active_variable,
            ResidualCategory::Hard,
            0.0,
        )),
        Err(CoreError::UnknownSource(id)) if id == stale_source
    ));

    let duplicate = ResidualBlock::new(
        source,
        ResidualCategory::Hard,
        vec![active_variable, active_variable],
        1,
        vec![1.0],
        vec![row("x / scale", "x", "scalar", "1")],
        Identity { offset: 0.0 },
    )
    .unwrap();
    assert!(matches!(
        problem.add_residual(duplicate),
        Err(CoreError::DuplicateIncidentVariable(id)) if id == active_variable
    ));

    assert!(matches!(
        ResidualBlock::new(
            source,
            ResidualCategory::Hard,
            vec![active_variable],
            0,
            Vec::new(),
            Vec::new(),
            Identity { offset: 0.0 },
        ),
        Err(CoreError::EmptyDimension { .. })
    ));
    assert!(matches!(
        ResidualBlock::new(
            source,
            ResidualCategory::Hard,
            vec![active_variable],
            1,
            vec![1.0],
            Vec::new(),
            Identity { offset: 0.0 },
        ),
        Err(CoreError::DimensionMismatch { .. })
    ));
    assert!(matches!(
        ResidualBlock::new(
            source,
            ResidualCategory::Hard,
            vec![active_variable],
            1,
            vec![1.0],
            vec![ResidualRowAudit::new(
                "",
                vec![AuditBinding::new("x", "value")],
                "1"
            )],
            Identity { offset: 0.0 },
        ),
        Err(CoreError::EmptyAuditMetadata { .. })
    ));
    assert!(matches!(
        SourceConstraint::new("  "),
        Err(CoreError::EmptyAuditMetadata { .. })
    ));
    assert!(matches!(
        problem.check_jacobians(0.0),
        Err(CoreError::InvalidFiniteDifferenceStep(_))
    ));
    assert!(matches!(
        problem.check_jacobians(f64::NAN),
        Err(CoreError::InvalidFiniteDifferenceStep(_))
    ));
}

#[test]
fn variables_and_sources_cannot_be_removed_while_referenced() {
    let mut problem = Problem::new();
    let variable = problem.add_variable(VariableBlock::scalar(1.0, 1.0).unwrap());
    let source = add_source(&mut problem, "referenced source");
    let residual = problem
        .add_residual(identity_residual(
            source,
            variable,
            ResidualCategory::Hard,
            0.0,
        ))
        .unwrap();

    assert!(matches!(
        problem.remove_variable(variable),
        Err(CoreError::VariableInUse(id)) if id == variable
    ));
    assert!(matches!(
        problem.remove_source(source),
        Err(CoreError::SourceInUse(id)) if id == source
    ));
    problem.remove_residual(residual).unwrap();
    problem.remove_variable(variable).unwrap();
    problem.remove_source(source).unwrap();
}

#[test]
fn variable_kind_and_increment_dimensions_are_checked() {
    let mut block = VariableBlock::vec2([1.0, 2.0], [1.0, 1.0]).unwrap();
    assert_eq!(block.kind(), VariableKind::Vec2);
    assert!(matches!(
        block.apply_local_increment(&[1.0]),
        Err(CoreError::DimensionMismatch { .. })
    ));
    assert!(matches!(
        block.set_value(VariableValue::Scalar(3.0)),
        Err(CoreError::VariableKindMismatch { .. })
    ));
    assert_eq!(block.value(), VariableValue::Vec2([1.0, 2.0]));
}
