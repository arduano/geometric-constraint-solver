use std::f64::consts::PI;

use geosolve_core::{
    AuditBinding, CoordinateBound, CoreError, EvaluationError, EvaluationErrorCategory,
    HardValidity, LocalJacobian, Problem, ResidualBlock, ResidualCategory, ResidualEvaluator,
    ResidualRowAudit, SensitivityError, SensitivityStatus, SessionPatch, SolveSession,
    SolverConfig, SourceConstraint, VariableBlock, VariableKind, VariableValue,
};
use geosolve_geometry::{Pose2 as GeometryPose2, Pose3 as GeometryPose3};
use nalgebra::DVector;

const JACOBIAN_TOLERANCE: f64 = 1.0e-6;

fn rows(count: usize, label: &str) -> Vec<ResidualRowAudit> {
    (0..count)
        .map(|coordinate| {
            ResidualRowAudit::new(
                format!("{label} local coordinate {coordinate}"),
                vec![AuditBinding::new("variable", label)],
                "local tangent",
            )
        })
        .collect()
}

fn assert_slice_close(actual: &[f64], expected: &[f64], tolerance: f64) {
    assert_eq!(actual.len(), expected.len());
    for (index, (&actual, &expected)) in actual.iter().zip(expected).enumerate() {
        assert!(
            (actual - expected).abs() <= tolerance,
            "index={index}, actual={actual}, expected={expected}, tolerance={tolerance}"
        );
    }
}

#[derive(Clone, Copy, Debug)]
struct ScalarPairEquation {
    coefficients: [f64; 2],
    target: f64,
}

impl ResidualEvaluator for ScalarPairEquation {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        let [VariableValue::Scalar(first), VariableValue::Scalar(second)] = variables else {
            return Err(EvaluationError::invalid_geometry("expected two scalars"));
        };
        Ok(vec![
            self.coefficients[0] * first + self.coefficients[1] * second - self.target,
        ])
    }

    fn jacobian(&self, _: &[VariableValue]) -> Result<Vec<LocalJacobian>, EvaluationError> {
        Ok(self
            .coefficients
            .iter()
            .map(|&coefficient| LocalJacobian::new(1, 1, vec![coefficient]))
            .collect())
    }
}

#[derive(Clone, Debug)]
struct Vec2Rows {
    coefficients: Vec<[f64; 2]>,
    targets: Vec<f64>,
}

impl ResidualEvaluator for Vec2Rows {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        let [VariableValue::Vec2(value)] = variables else {
            return Err(EvaluationError::invalid_geometry("expected Vec2"));
        };
        Ok(self
            .coefficients
            .iter()
            .zip(&self.targets)
            .map(|(row, target)| row[0] * value[0] + row[1] * value[1] - target)
            .collect())
    }

    fn jacobian(&self, _: &[VariableValue]) -> Result<Vec<LocalJacobian>, EvaluationError> {
        Ok(vec![LocalJacobian::new(
            self.coefficients.len(),
            2,
            self.coefficients
                .iter()
                .flat_map(|row| row.iter().copied())
                .collect(),
        )])
    }
}

#[derive(Clone, Debug)]
struct ScalarRows {
    coefficients: Vec<f64>,
    targets: Vec<f64>,
}

impl ResidualEvaluator for ScalarRows {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        let [VariableValue::Scalar(value)] = variables else {
            return Err(EvaluationError::invalid_geometry("expected scalar"));
        };
        Ok(self
            .coefficients
            .iter()
            .zip(&self.targets)
            .map(|(coefficient, target)| coefficient * value - target)
            .collect())
    }

    fn jacobian(&self, _: &[VariableValue]) -> Result<Vec<LocalJacobian>, EvaluationError> {
        Ok(vec![LocalJacobian::new(
            self.coefficients.len(),
            1,
            self.coefficients.clone(),
        )])
    }
}

#[test]
fn vec3_and_pose3_pack_with_distinct_ambient_and_tangent_dimensions() {
    assert_eq!(VariableKind::Vec3.ambient_dimension(), 3);
    assert_eq!(VariableKind::Vec3.tangent_dimension(), 3);
    assert_eq!(VariableKind::Pose3.ambient_dimension(), 7);
    assert_eq!(VariableKind::Pose3.tangent_dimension(), 6);

    let mut problem = Problem::new();
    let vector =
        problem.add_variable(VariableBlock::vec3([1.0, 2.0, 3.0], [0.1, 0.2, 0.3]).unwrap());
    let pose = problem.add_variable(
        VariableBlock::pose3(
            [4.0, 5.0, 6.0, -0.5, 0.5, -0.5, 0.5],
            [1.0, 1.0, 1.0, 0.1, 0.1, 0.1],
        )
        .unwrap(),
    );

    let state = problem.packed_state().unwrap();
    assert_eq!(state.layout().ambient_dimension(), 10);
    assert_eq!(state.layout().tangent_dimension(), 9);
    assert_eq!(state.layout().block(vector).unwrap().ambient_range, 0..3);
    assert_eq!(state.layout().block(vector).unwrap().tangent_range, 0..3);
    assert_eq!(state.layout().block(pose).unwrap().ambient_range, 3..10);
    assert_eq!(state.layout().block(pose).unwrap().tangent_range, 3..9);
    assert_eq!(
        state.ambient().as_slice(),
        &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 0.5, -0.5, 0.5, -0.5]
    );
}

#[test]
fn pose3_constructor_and_replacement_canonicalize_sign_and_reject_invalid_quaternions() {
    let positive = VariableBlock::pose3([1.0, 2.0, 3.0, 0.5, -0.5, 0.5, -0.5], [1.0; 6]).unwrap();
    let negative = VariableBlock::pose3([1.0, 2.0, 3.0, -0.5, 0.5, -0.5, 0.5], [1.0; 6]).unwrap();
    assert_eq!(positive.value(), negative.value());

    let mut problem = Problem::new();
    let pose = problem.add_variable(positive);
    problem
        .set_variable_value(
            pose,
            VariableValue::Pose3([1.0, 2.0, 3.0, -0.5, 0.5, -0.5, 0.5]),
        )
        .unwrap();
    assert_eq!(problem.variable(pose).unwrap().value(), negative.value());

    for quaternion in [[0.0, 0.0, 0.0, 0.0], [2.0, 0.0, 0.0, 0.0]] {
        let result = VariableBlock::pose3(
            [
                0.0,
                0.0,
                0.0,
                quaternion[0],
                quaternion[1],
                quaternion[2],
                quaternion[3],
            ],
            [1.0; 6],
        );
        assert!(matches!(
            result,
            Err(CoreError::InvalidVariableValue {
                kind: VariableKind::Pose3,
                ..
            })
        ));
    }
}

#[test]
fn pose2_and_pose3_local_increments_are_exact_geometry_right_retractions() {
    let pose2_ambient = [3.0, -2.0, 0.7];
    let pose2_delta = [0.4, -0.2, 0.35];
    let pose3_ambient = GeometryPose3::exp([1.0, -2.0, 0.5, 0.3, -0.2, 0.4])
        .unwrap()
        .ambient();
    let pose3_delta = [0.2, -0.1, 0.3, -0.15, 0.25, 0.05];
    let mut problem = Problem::new();
    let pose2 = problem.add_variable(VariableBlock::pose2(pose2_ambient, [1.0; 3]).unwrap());
    let pose3 = problem.add_variable(VariableBlock::pose3(pose3_ambient, [1.0; 6]).unwrap());

    problem.apply_local_increment(pose2, &pose2_delta).unwrap();
    problem.apply_local_increment(pose3, &pose3_delta).unwrap();

    let VariableValue::Pose2(actual_pose2) = problem.variable(pose2).unwrap().value() else {
        panic!("expected Pose2")
    };
    let VariableValue::Pose3(actual_pose3) = problem.variable(pose3).unwrap().value() else {
        panic!("expected Pose3")
    };
    let expected_pose2 = GeometryPose2::from_ambient(pose2_ambient)
        .unwrap()
        .retract(pose2_delta)
        .unwrap()
        .ambient();
    let expected_pose3 = GeometryPose3::from_ambient(pose3_ambient)
        .unwrap()
        .retract(pose3_delta)
        .unwrap()
        .ambient();
    assert_slice_close(&actual_pose2, &expected_pose2, 1.0e-14);
    assert_slice_close(&actual_pose3, &expected_pose3, 1.0e-14);
}

#[test]
fn pose_fixed_residuals_use_off_equality_local_differences_and_oracle_jacobians() {
    for scale in [1.0e-6, 1.0, 1.0e6] {
        let pose2_target = GeometryPose2::from_ambient([2.0 * scale, -scale, 0.4]).unwrap();
        for delta in [
            [0.3 * scale, -0.2 * scale, 0.25],
            [-0.17 * scale, 0.28 * scale, -0.45],
        ] {
            assert_fixed_pose(
                VariableBlock::pose2(
                    pose2_target.retract(delta).unwrap().ambient(),
                    [0.7 * scale, 1.3 * scale, 0.8],
                )
                .unwrap(),
                VariableValue::Pose2(pose2_target.ambient()),
                &delta,
                &[1.4 * scale, 0.6 * scale, 1.2],
            );
        }

        let pose3_target =
            GeometryPose3::exp([2.0 * scale, -scale, 0.5 * scale, 0.2, -0.3, 0.4]).unwrap();
        for delta in [
            [0.3 * scale, -0.2 * scale, 0.1 * scale, 0.15, -0.1, 0.2],
            [
                -0.18 * scale,
                0.22 * scale,
                -0.27 * scale,
                -0.35,
                0.18,
                -0.12,
            ],
        ] {
            assert_fixed_pose(
                VariableBlock::pose3(
                    pose3_target.retract(delta).unwrap().ambient(),
                    [0.7 * scale, 1.2 * scale, 0.9 * scale, 0.8, 1.1, 0.6],
                )
                .unwrap(),
                VariableValue::Pose3(pose3_target.ambient()),
                &delta,
                &[1.4 * scale, 0.6 * scale, 1.1 * scale, 1.2, 0.7, 1.3],
            );
        }
    }
}

#[test]
fn pose_alias_residuals_use_off_equality_local_differences_and_oracle_jacobians() {
    for scale in [1.0e-6, 1.0, 1.0e6] {
        let pose2_root = GeometryPose2::from_ambient([-scale, 3.0 * scale, -0.6]).unwrap();
        let pose2_steps = [0.9 * scale, 1.4 * scale, 0.7];
        for delta in [
            [-0.25 * scale, 0.15 * scale, 0.3],
            [0.19 * scale, -0.31 * scale, -0.52],
        ] {
            assert_alias_pose(
                VariableBlock::pose2(pose2_root.retract(delta).unwrap().ambient(), pose2_steps)
                    .unwrap(),
                VariableBlock::pose2(pose2_root.ambient(), pose2_steps).unwrap(),
                VariableKind::Pose2,
                &delta,
                &[0.8 * scale, 1.5 * scale, 1.1],
            );
        }

        let pose3_root =
            GeometryPose3::exp([-scale, 3.0 * scale, 0.2 * scale, -0.3, 0.1, 0.5]).unwrap();
        let pose3_steps = [0.9 * scale, 1.4 * scale, 0.8 * scale, 0.7, 1.2, 0.6];
        for delta in [
            [-0.25 * scale, 0.15 * scale, 0.05 * scale, 0.2, -0.15, 0.1],
            [
                0.21 * scale,
                -0.29 * scale,
                0.17 * scale,
                -0.28,
                0.23,
                -0.19,
            ],
        ] {
            assert_alias_pose(
                VariableBlock::pose3(pose3_root.retract(delta).unwrap().ambient(), pose3_steps)
                    .unwrap(),
                VariableBlock::pose3(pose3_root.ambient(), pose3_steps).unwrap(),
                VariableKind::Pose3,
                &delta,
                &[0.8 * scale, 1.5 * scale, 1.1 * scale, 1.2, 0.7, 1.3],
            );
        }
    }
}

#[test]
fn pose_fixed_and_alias_jacobians_reject_exact_principal_log_cuts() {
    let pose2_reference = VariableValue::Pose2([0.0, 0.0, 0.0]);
    let pose2_cut = VariableValue::Pose2([0.0, 0.0, PI]);
    let pose3_reference = VariableValue::Pose3(GeometryPose3::identity().ambient());
    let pose3_cut = VariableValue::Pose3(
        GeometryPose3::exp([0.0, 0.0, 0.0, PI, 0.0, 0.0])
            .unwrap()
            .ambient(),
    );
    for (reference, cut) in [(pose2_reference, pose2_cut), (pose3_reference, pose3_cut)] {
        assert_principal_cut_rejected(reference, cut, false);
        assert_principal_cut_rejected(reference, cut, true);
    }
}

#[test]
fn coordinate_bounds_accept_vec3_and_typed_reject_pose_manifolds() {
    let mut problem = Problem::new();
    let vector = problem.add_variable(VariableBlock::vec3([1.0, 2.0, 3.0], [1.0; 3]).unwrap());
    problem
        .add_bound(CoordinateBound::new(vector, 2, Some(0.0), Some(4.0), "vec3 z").unwrap())
        .unwrap();

    let pose2 = problem.add_variable(VariableBlock::pose2([0.0, 0.0, 0.0], [1.0; 3]).unwrap());
    let pose3 = problem
        .add_variable(VariableBlock::pose3([0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0], [1.0; 6]).unwrap());
    for (variable, kind) in [(pose2, VariableKind::Pose2), (pose3, VariableKind::Pose3)] {
        let error = problem
            .add_bound(
                CoordinateBound::new(variable, 0, Some(-1.0), Some(1.0), "pose coordinate")
                    .unwrap(),
            )
            .unwrap_err();
        assert_eq!(
            error,
            CoreError::UnsupportedBoundVariableKind { variable, kind }
        );
    }
}

#[test]
#[allow(clippy::float_cmp, clippy::too_many_lines)]
fn accepted_hard_linearization_is_deterministic_reduced_and_revision_stamped() {
    let mut problem = Problem::new();
    let fixed = problem.add_variable(VariableBlock::scalar(5.0, 1.0).unwrap());
    let alias = problem.add_variable(VariableBlock::scalar(-4.0, 2.0).unwrap());
    let root = problem.add_variable(VariableBlock::scalar(3.0, 2.0).unwrap());
    let vector = problem.add_variable(VariableBlock::vec2([4.0, -2.0], [3.0, 5.0]).unwrap());

    let fixed_source = problem.add_source(SourceConstraint::new("fixed component").unwrap());
    let fixed_residual = problem
        .add_residual(
            ResidualBlock::fixed_variable(
                fixed_source,
                fixed,
                VariableValue::Scalar(5.0),
                vec![7.0],
                rows(1, "fixed"),
            )
            .unwrap(),
        )
        .unwrap();
    problem
        .declare_fixed_variable(fixed, VariableValue::Scalar(5.0), fixed_residual)
        .unwrap();

    let alias_source = problem.add_source(SourceConstraint::new("alias declaration").unwrap());
    let alias_residual = problem
        .add_residual(
            ResidualBlock::exact_alias(
                alias_source,
                alias,
                root,
                VariableKind::Scalar,
                vec![11.0],
                rows(1, "alias"),
            )
            .unwrap(),
        )
        .unwrap();
    problem
        .declare_exact_alias(alias, root, alias_residual)
        .unwrap();

    let pair_source = problem.add_source(SourceConstraint::new("reduced alias equation").unwrap());
    let pair_residual = problem
        .add_residual(
            ResidualBlock::new(
                pair_source,
                ResidualCategory::Hard,
                vec![alias, root],
                1,
                vec![3.0],
                rows(1, "pair"),
                ScalarPairEquation {
                    coefficients: [1.0, 2.0],
                    target: 9.0,
                },
            )
            .unwrap(),
        )
        .unwrap();

    let vector_source = problem.add_source(SourceConstraint::new("vector rows").unwrap());
    let vector_residual = problem
        .add_residual(
            ResidualBlock::new(
                vector_source,
                ResidualCategory::Hard,
                vec![vector],
                2,
                vec![2.0, 4.0],
                rows(2, "vector"),
                Vec2Rows {
                    coefficients: vec![[1.0, 0.0], [0.0, 1.0]],
                    targets: vec![4.0, -2.0],
                },
            )
            .unwrap(),
        )
        .unwrap();

    let mut session = SolveSession::new(problem, SolverConfig::default()).unwrap();
    let accepted = session.accepted_hard_linearization().unwrap();
    assert_eq!(accepted.revisions(), session.revisions());
    assert_eq!(accepted.accepted_state(), &session.report().accepted_state);
    assert_eq!(accepted.components().len(), 3);
    assert!(
        accepted
            .components()
            .iter()
            .all(|component| component.revisions() == accepted.revisions())
    );
    assert_eq!(
        accepted,
        session.accepted_hard_linearization().unwrap(),
        "identical accepted state must produce an identical snapshot"
    );

    let fixed_component = accepted
        .components()
        .iter()
        .find(|component| component.tangent_blocks().is_empty())
        .unwrap();
    assert!(fixed_component.hard_rows().is_empty());
    assert_eq!(fixed_component.normalized_jacobian().shape(), (0, 0));

    let alias_component = accepted
        .components()
        .iter()
        .find(|component| {
            component
                .tangent_blocks()
                .iter()
                .any(|block| block.root == root)
        })
        .unwrap();
    assert_eq!(alias_component.component_index(), 1);
    assert_eq!(
        alias_component.pattern_signature(),
        session.report().component_solves[1].pattern_signature
    );
    assert_eq!(alias_component.tangent_blocks().len(), 1);
    assert_eq!(alias_component.tangent_blocks()[0].root, root);
    assert_eq!(
        alias_component.tangent_blocks()[0].alias_members,
        vec![alias]
    );
    assert_eq!(
        alias_component.tangent_blocks()[0].kind,
        VariableKind::Scalar
    );
    assert_eq!(alias_component.tangent_blocks()[0].tangent_range, 0..1);
    assert_eq!(alias_component.tangent_blocks()[0].step_scales, vec![2.0]);
    assert_eq!(alias_component.hard_rows().len(), 1);
    assert_eq!(
        alias_component.hard_rows()[0].row.residual_id,
        pair_residual
    );
    assert_eq!(alias_component.hard_rows()[0].row.source_id, pair_source);
    assert_eq!(alias_component.hard_rows()[0].residual_scale, 3.0);
    assert_eq!(alias_component.normalized_jacobian().shape(), (1, 1));
    assert_eq!(alias_component.normalized_jacobian()[(0, 0)], 2.0);
    assert_eq!(alias_component.normalized_residual()[0], 0.0);
    assert_eq!(
        (
            alias_component.rank(),
            alias_component.left_nullity(),
            alias_component.right_nullity()
        ),
        (1, 0, 0)
    );
    assert_eq!(
        alias_component.rank_threshold(),
        session.report().component_solves[1].rank_threshold
    );
    assert_eq!(
        alias_component.singular_values(),
        session.report().component_solves[1].singular_values
    );

    let vector_component = accepted
        .components()
        .iter()
        .find(|component| {
            component
                .tangent_blocks()
                .iter()
                .any(|block| block.root == vector)
        })
        .unwrap();
    assert_eq!(vector_component.tangent_blocks()[0].tangent_range, 0..2);
    assert_eq!(vector_component.hard_rows().len(), 2);
    assert_eq!(
        vector_component.hard_rows()[0].row.residual_id,
        vector_residual
    );
    assert_eq!(vector_component.hard_rows()[0].row.row_in_block, 0);
    assert_eq!(vector_component.hard_rows()[0].residual_scale, 2.0);
    assert_eq!(vector_component.hard_rows()[1].row.row_in_block, 1);
    assert_eq!(vector_component.hard_rows()[1].residual_scale, 4.0);
    assert_eq!(vector_component.normalized_jacobian()[(0, 0)], 1.5);
    assert_eq!(vector_component.normalized_jacobian()[(0, 1)], 0.0);
    assert_eq!(vector_component.normalized_jacobian()[(1, 0)], 0.0);
    assert_eq!(vector_component.normalized_jacobian()[(1, 1)], 1.25);

    let unique = alias_component
        .solve_sensitivity(&DVector::from_vec(vec![-4.0]))
        .unwrap();
    assert_eq!(unique.revisions, accepted.revisions());
    assert_eq!(unique.status, SensitivityStatus::Unique);
    assert_eq!(unique.normalized_tangent.as_slice(), &[2.0]);
    assert_eq!(unique.raw_tangent_blocks[0].root, root);
    assert_eq!(unique.raw_tangent_blocks[0].alias_members, vec![alias]);
    assert_eq!(unique.raw_tangent_blocks[0].values.as_slice(), &[4.0]);
    assert!(unique.equation_residual_max <= session.config().normalized_residual_tolerance);

    let old_revisions = accepted.revisions();
    let mut patch = SessionPatch::new(session.revisions());
    patch.set_variable_value(root, VariableValue::Scalar(2.5));
    assert!(session.apply(patch).unwrap().committed());
    let edited = session.accepted_hard_linearization().unwrap();
    assert_eq!(edited.revisions(), session.revisions());
    assert!(
        edited
            .components()
            .iter()
            .all(|component| component.revisions() == edited.revisions())
    );
    assert!(edited.revisions().state > old_revisions.state);
    assert_eq!(accepted.revisions(), old_revisions);
}

#[test]
fn sensitivity_distinguishes_minimum_norm_and_inconsistent_rates_and_rejects_input() {
    let mut under_problem = Problem::new();
    let vector = under_problem.add_variable(VariableBlock::vec2([0.0, 0.0], [1.0; 2]).unwrap());
    let source = under_problem.add_source(SourceConstraint::new("one row in two columns").unwrap());
    under_problem
        .add_residual(
            ResidualBlock::new(
                source,
                ResidualCategory::Hard,
                vec![vector],
                1,
                vec![1.0],
                rows(1, "under"),
                Vec2Rows {
                    coefficients: vec![[1.0, 1.0]],
                    targets: vec![0.0],
                },
            )
            .unwrap(),
        )
        .unwrap();
    let under_session = SolveSession::new(under_problem, SolverConfig::default()).unwrap();
    let under = under_session.accepted_hard_linearization().unwrap();
    let component = &under.components()[0];
    let solution = component
        .solve_sensitivity(&DVector::from_vec(vec![-2.0]))
        .unwrap();
    assert_eq!(
        solution.status,
        SensitivityStatus::UnderdeterminedMinimumNorm
    );
    assert_slice_close(solution.normalized_tangent.as_slice(), &[1.0, 1.0], 1.0e-14);
    assert_slice_close(
        solution.raw_tangent_blocks[0].values.as_slice(),
        &[1.0, 1.0],
        1.0e-14,
    );
    assert!(solution.equation_residual_max <= under_session.config().normalized_residual_tolerance);

    let mut inconsistent_problem = Problem::new();
    let scalar = inconsistent_problem.add_variable(VariableBlock::scalar(0.0, 1.0).unwrap());
    let source =
        inconsistent_problem.add_source(SourceConstraint::new("duplicate scalar rows").unwrap());
    inconsistent_problem
        .add_residual(
            ResidualBlock::new(
                source,
                ResidualCategory::Hard,
                vec![scalar],
                2,
                vec![1.0; 2],
                rows(2, "duplicate"),
                ScalarRows {
                    coefficients: vec![1.0, 1.0],
                    targets: vec![0.0, 0.0],
                },
            )
            .unwrap(),
        )
        .unwrap();
    let inconsistent_session =
        SolveSession::new(inconsistent_problem, SolverConfig::default()).unwrap();
    let inconsistent = inconsistent_session.accepted_hard_linearization().unwrap();
    let component = &inconsistent.components()[0];
    let solution = component
        .solve_sensitivity(&DVector::from_vec(vec![1.0, -1.0]))
        .unwrap();
    assert_eq!(solution.status, SensitivityStatus::Inconsistent);
    assert!(!solution.status.is_success_like());
    assert!(
        solution.equation_residual_max
            > inconsistent_session.config().normalized_residual_tolerance
    );
    assert!((solution.equation_residual_max - 1.0).abs() <= 1.0e-14);
    assert!((solution.equation_residual_l2 - 2.0_f64.sqrt()).abs() <= 1.0e-14);

    assert_eq!(
        component
            .solve_sensitivity(&DVector::from_vec(vec![0.0]))
            .unwrap_err(),
        SensitivityError::DimensionMismatch {
            expected: 2,
            actual: 1,
        }
    );
    assert!(matches!(
        component.solve_sensitivity(&DVector::from_vec(vec![f64::NAN, 0.0])),
        Err(SensitivityError::NonFiniteRightHandSide { index: 0, value }) if value.is_nan()
    ));
}

#[test]
fn sensitivity_rejects_smallest_subnormal_raw_tangent_underflow() {
    let smallest_subnormal = f64::from_bits(1);
    let mut problem = Problem::new();
    let variable = problem.add_variable(VariableBlock::scalar(0.0, smallest_subnormal).unwrap());
    let source = problem.add_source(SourceConstraint::new("subnormal sensitivity").unwrap());
    problem
        .add_residual(
            ResidualBlock::new(
                source,
                ResidualCategory::Hard,
                vec![variable],
                1,
                vec![smallest_subnormal],
                rows(1, "subnormal sensitivity"),
                ScalarRows {
                    coefficients: vec![1.0],
                    targets: vec![0.0],
                },
            )
            .unwrap(),
        )
        .unwrap();
    let session = SolveSession::new(problem, SolverConfig::default()).unwrap();
    let accepted = session.accepted_hard_linearization().unwrap();
    assert_eq!(
        accepted.components()[0].normalized_jacobian()[(0, 0)].to_bits(),
        1.0_f64.to_bits()
    );

    assert!(matches!(
        accepted.components()[0].solve_sensitivity(&DVector::from_vec(vec![-0.5])),
        Err(SensitivityError::NumericalFailure {
            context: "raw tangent scaling loses material precision"
        })
    ));
}

#[test]
fn sensitivity_rejects_huge_jacobian_amplification_of_raw_roundoff() {
    let target_normalized_tangent = 0.7;
    let step_scale = 3.0;
    let requested_jacobian = 2.0_f64.powi(900);
    let mut problem = Problem::new();
    let variable = problem.add_variable(VariableBlock::scalar(0.0, step_scale).unwrap());
    let source = problem.add_source(SourceConstraint::new("huge sensitivity Jacobian").unwrap());
    problem
        .add_residual(
            ResidualBlock::new(
                source,
                ResidualCategory::Hard,
                vec![variable],
                1,
                vec![1.0],
                rows(1, "huge sensitivity Jacobian"),
                ScalarRows {
                    coefficients: vec![requested_jacobian / step_scale],
                    targets: vec![0.0],
                },
            )
            .unwrap(),
        )
        .unwrap();
    let session = SolveSession::new(problem, SolverConfig::default()).unwrap();
    let accepted = session.accepted_hard_linearization().unwrap();
    let component = &accepted.components()[0];
    let jacobian = component.normalized_jacobian()[(0, 0)];
    let rhs = DVector::from_vec(vec![-jacobian * target_normalized_tangent]);

    let raw = target_normalized_tangent * step_scale;
    let recovered = raw / step_scale;
    assert!(
        (recovered - target_normalized_tangent).abs()
            <= 64.0 * f64::EPSILON * target_normalized_tangent
    );
    assert!(
        (jacobian * (recovered - target_normalized_tangent)).abs()
            > session.config().normalized_residual_tolerance
    );
    assert!(matches!(
        component.solve_sensitivity(&rhs),
        Err(SensitivityError::NumericalFailure {
            context: "recoverable raw tangent violates differentiated equations"
        })
    ));
}

#[test]
fn sensitivity_matches_a_central_target_perturbation_oracle() {
    let mut problem = Problem::new();
    let variable = problem.add_variable(VariableBlock::scalar(0.0, 2.0).unwrap());
    let source = problem.add_source(SourceConstraint::new("moving target").unwrap());
    let residual = problem
        .add_residual(
            ResidualBlock::new(
                source,
                ResidualCategory::Hard,
                vec![variable],
                1,
                vec![4.0],
                rows(1, "moving target"),
                ScalarRows {
                    coefficients: vec![1.0],
                    targets: vec![2.0],
                },
            )
            .unwrap(),
        )
        .unwrap();
    let config = SolverConfig {
        normalized_residual_tolerance: 1.0e-12,
        ..SolverConfig::default()
    };
    let session = SolveSession::new(problem, config).unwrap();
    let accepted = session.accepted_hard_linearization().unwrap();
    assert!((accepted.components()[0].normalized_jacobian()[(0, 0)] - 0.5).abs() <= f64::EPSILON);
    let sensitivity = accepted.components()[0]
        .solve_sensitivity(&DVector::from_vec(vec![-0.25]))
        .unwrap();
    assert_eq!(sensitivity.status, SensitivityStatus::Unique);

    let h = 1.0e-3;
    let solve_perturbed = |target: f64| {
        let mut perturbed = session.clone();
        let replacement = ResidualBlock::new(
            source,
            ResidualCategory::Hard,
            vec![variable],
            1,
            vec![4.0],
            rows(1, "moving target"),
            ScalarRows {
                coefficients: vec![1.0],
                targets: vec![target],
            },
        )
        .unwrap();
        let mut patch = SessionPatch::new(perturbed.revisions());
        patch.replace_residual(residual, replacement);
        assert!(perturbed.apply(patch).unwrap().committed());
        let VariableValue::Scalar(value) = perturbed.problem().variable(variable).unwrap().value()
        else {
            panic!("expected scalar")
        };
        value
    };
    let central_raw_rate = (solve_perturbed(2.0 + h) - solve_perturbed(2.0 - h)) / (2.0 * h);
    assert!(
        (central_raw_rate - sensitivity.raw_tangent_blocks[0].values[0]).abs() <= 1.0e-7,
        "central={central_raw_rate:e}, sensitivity={:e}",
        sensitivity.raw_tangent_blocks[0].values[0]
    );
    assert!((central_raw_rate / 2.0 - sensitivity.normalized_tangent[0]).abs() <= 1.0e-7);
    assert!(sensitivity.equation_residual_max <= session.config().normalized_residual_tolerance);
}

fn assert_fixed_pose(
    block: VariableBlock,
    target: VariableValue,
    expected: &[f64],
    residual_scales: &[f64],
) {
    let dimension = target.kind().tangent_dimension();
    assert_eq!(residual_scales.len(), dimension);
    let mut problem = Problem::new();
    let variable = problem.add_variable(block);
    let source = problem.add_source(SourceConstraint::new("M15 fixed pose").unwrap());
    let residual = problem
        .add_residual(
            ResidualBlock::fixed_variable(
                source,
                variable,
                target,
                residual_scales.to_vec(),
                rows(dimension, "fixed pose"),
            )
            .unwrap(),
        )
        .unwrap();
    let assembly = problem.assemble_dense().unwrap();
    assert_eq!(assembly.jacobian().shape(), (dimension, dimension));
    let expected = expected
        .iter()
        .zip(residual_scales)
        .map(|(value, scale)| value / scale)
        .collect::<Vec<_>>();
    assert_slice_close(assembly.residuals().as_slice(), &expected, 2.0e-9);
    let oracle = problem.check_jacobians(1.0e-5).unwrap();
    assert!(oracle.all_within(JACOBIAN_TOLERANCE), "{oracle:#?}");

    problem
        .declare_fixed_variable(variable, target, residual)
        .unwrap();
    assert_eq!(problem.variable(variable).unwrap().value(), target);
    let report = problem.solve(SolverConfig::default()).unwrap();
    assert_eq!(report.hard_validity, HardValidity::Valid, "{report:#?}");
    assert!(report.hard_residual_max <= 1.0e-12, "{report:#?}");
}

fn assert_alias_pose(
    alias_block: VariableBlock,
    root_block: VariableBlock,
    kind: VariableKind,
    expected: &[f64],
    residual_scales: &[f64],
) {
    let dimension = kind.tangent_dimension();
    assert_eq!(residual_scales.len(), dimension);
    let mut problem = Problem::new();
    let alias = problem.add_variable(alias_block);
    let root = problem.add_variable(root_block);
    let root_value = problem.variable(root).unwrap().value();
    let source = problem.add_source(SourceConstraint::new("M15 alias pose").unwrap());
    let residual = problem
        .add_residual(
            ResidualBlock::exact_alias(
                source,
                alias,
                root,
                kind,
                residual_scales.to_vec(),
                rows(dimension, "alias pose"),
            )
            .unwrap(),
        )
        .unwrap();
    let assembly = problem.assemble_dense().unwrap();
    assert_eq!(assembly.jacobian().shape(), (dimension, 2 * dimension));
    let expected = expected
        .iter()
        .zip(residual_scales)
        .map(|(value, scale)| value / scale)
        .collect::<Vec<_>>();
    assert_slice_close(assembly.residuals().as_slice(), &expected, 2.0e-9);
    let oracle = problem.check_jacobians(1.0e-5).unwrap();
    assert!(oracle.all_within(JACOBIAN_TOLERANCE), "{oracle:#?}");

    problem.declare_exact_alias(alias, root, residual).unwrap();
    assert_eq!(problem.variable(alias).unwrap().value(), root_value);
    let report = problem.solve(SolverConfig::default()).unwrap();
    assert_eq!(report.hard_validity, HardValidity::Valid, "{report:#?}");
    assert!(report.hard_residual_max <= 1.0e-12, "{report:#?}");
}

fn assert_principal_cut_rejected(reference: VariableValue, value: VariableValue, alias: bool) {
    let dimension = reference.kind().tangent_dimension();
    let mut problem = Problem::new();
    let reference_id =
        problem.add_variable(VariableBlock::new(reference, vec![1.0; dimension]).unwrap());
    let value_id = problem.add_variable(VariableBlock::new(value, vec![1.0; dimension]).unwrap());
    let source = problem.add_source(SourceConstraint::new("principal log cut").unwrap());
    let residual = if alias {
        ResidualBlock::exact_alias(
            source,
            value_id,
            reference_id,
            reference.kind(),
            vec![1.0; dimension],
            rows(dimension, "principal-cut alias"),
        )
        .unwrap()
    } else {
        ResidualBlock::fixed_variable(
            source,
            value_id,
            reference,
            vec![1.0; dimension],
            rows(dimension, "principal-cut fixed"),
        )
        .unwrap()
    };
    problem.add_residual(residual).unwrap();
    assert!(matches!(
        problem.assemble_dense(),
        Err(CoreError::CategorizedEvaluation {
            category: EvaluationErrorCategory::Nondifferentiable,
            ..
        })
    ));
}
