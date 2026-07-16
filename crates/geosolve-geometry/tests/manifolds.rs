use std::f64::consts::PI;

use geosolve_geometry::{
    GeometryError, Point2, Point3, Pose2, Pose3, QUATERNION_NORM_TOLERANCE,
    QUATERNION_SIGN_TOLERANCE, Rotation2, Vector2, Vector3,
};
use nalgebra::{SVector, Unit};

const TOLERANCE: f64 = 2.0e-10;

#[test]
fn pose2_identity_inverse_and_composition_are_rigid_transform_operations() {
    let point = Point2::new(0.7, -1.3);
    let vector = Vector2::new(-0.2, 0.9);
    let identity = Pose2::identity();
    assert_point2(identity.transform_point(point), point, TOLERANCE);
    assert_vector2(identity.transform_vector(vector), vector, TOLERANCE);

    for ambient in [
        [0.0, 0.0, 0.0],
        [2.0, -3.0, 0.7],
        [-1.0e6, 2.0e6, -2.4],
        [1.0e-6, -2.0e-6, 7.0],
    ] {
        let pose = Pose2::from_ambient(ambient).unwrap();
        let inverse = pose.inverse().unwrap();
        assert_pose2(
            &pose.compose(&inverse).unwrap(),
            &Pose2::identity(),
            TOLERANCE * ambient[0].abs().max(ambient[1].abs()).max(1.0),
        );
        assert_pose2(
            &inverse.compose(&pose).unwrap(),
            &Pose2::identity(),
            TOLERANCE * ambient[0].abs().max(ambient[1].abs()).max(1.0),
        );
        assert_point2(
            pose.inverse_transform_point(pose.transform_point(point)),
            point,
            2.0e-9,
        );
    }

    let parent = Pose2::from_ambient([3.0, -2.0, 0.4]).unwrap();
    let child = Pose2::from_ambient([-0.5, 1.2, -0.9]).unwrap();
    let composed = parent.compose(&child).unwrap();
    assert_point2(
        composed.transform_point(point),
        parent.transform_point(child.transform_point(point)),
        TOLERANCE,
    );
    assert_vector2(
        composed.transform_vector(vector),
        parent.transform_vector(child.transform_vector(vector)),
        TOLERANCE,
    );

    let third = Pose2::from_ambient([0.8, 0.3, 1.1]).unwrap();
    assert_pose2(
        &parent.compose(&child).unwrap().compose(&third).unwrap(),
        &parent.compose(&child.compose(&third).unwrap()).unwrap(),
        TOLERANCE,
    );
}

#[test]
fn pose2_exp_log_retraction_and_local_difference_are_stable() {
    let twists = [
        [0.0, 0.0, 0.0],
        [1.2, -0.7, 0.8],
        [-2.0, 0.4, -2.6],
        [0.25, -0.5, 1.0e-12],
        [0.3, 0.2, PI - 1.0e-9],
    ];
    for twist in twists {
        let pose = Pose2::exp(twist).unwrap();
        assert_array(pose.log().unwrap(), twist, 2.0e-9);
        assert_pose2(&Pose2::exp(pose.log().unwrap()).unwrap(), &pose, 2.0e-9);
    }

    let reference = Pose2::from_ambient([4.0, -7.0, 0.9]).unwrap();
    for delta in [
        [0.2, -0.1, 0.3],
        [-1.0e-8, 2.0e-8, -3.0e-9],
        [1.0, 0.5, -1.2],
    ] {
        let retracted = reference.retract(delta).unwrap();
        assert_array(
            reference.local_difference(&retracted).unwrap(),
            delta,
            2.0e-9,
        );
    }
}

#[test]
fn pose2_adjoint_matches_group_conjugation_and_global_equivariance() {
    let transform = Pose2::from_ambient([1.5, -0.8, 0.6]).unwrap();
    let tangent = [0.3, -0.7, 0.2];
    let mapped = transform.adjoint().unwrap() * Vector3::from(tangent);
    let conjugated = transform
        .compose(&Pose2::exp(tangent).unwrap())
        .unwrap()
        .compose(&transform.inverse().unwrap())
        .unwrap();
    assert_pose2(&Pose2::exp(mapped.into()).unwrap(), &conjugated, 2.0e-10);

    let global = Pose2::from_ambient([-8.0, 3.0, -1.1]).unwrap();
    let pose = Pose2::from_ambient([2.0, 5.0, 0.4]).unwrap();
    let delta = [0.4, -0.2, 0.15];
    let globally_retracted = global.compose(&pose).unwrap().retract(delta).unwrap();
    let transformed_retraction = global.compose(&pose.retract(delta).unwrap()).unwrap();
    assert_pose2(&globally_retracted, &transformed_retraction, TOLERANCE);
    assert_array(
        global
            .compose(&pose)
            .unwrap()
            .local_difference(&global.compose(&pose.retract(delta).unwrap()).unwrap())
            .unwrap(),
        delta,
        2.0e-10,
    );
}

#[test]
fn pose2_right_tangent_transform_matches_central_differences() {
    let pose = Pose2::from_ambient([2.0, -3.0, 0.73]).unwrap();
    let local_point = Point2::new(0.4, -1.2);
    let rotation = Rotation2::new(pose.angle);
    let expected = [
        rotation * Vector2::x(),
        rotation * Vector2::y(),
        rotation * Vector2::new(-local_point.y, local_point.x),
    ];
    let step = 1.0e-6;
    for coordinate in 0..3 {
        let mut positive = [0.0; 3];
        let mut negative = [0.0; 3];
        positive[coordinate] = step;
        negative[coordinate] = -step;
        let derivative = (pose.retract(positive).unwrap().transform_point(local_point)
            - pose.retract(negative).unwrap().transform_point(local_point))
            / (2.0 * step);
        assert_vector2(derivative, expected[coordinate], 2.0e-9);
    }
}

#[test]
fn pose3_identity_inverse_composition_and_ambient_dimensions_are_correct() {
    assert_eq!(Pose3::AMBIENT_DIMENSION, 7);
    assert_eq!(Pose3::TANGENT_DIMENSION, 6);
    let point = Point3::new(0.7, -1.3, 2.1);
    let vector = Vector3::new(-0.2, 0.9, 1.4);
    let identity = Pose3::identity();
    assert_point3(identity.transform_point(point), point, TOLERANCE);
    assert_vector3(identity.transform_vector(vector), vector, TOLERANCE);

    let first = Pose3::exp([1.0, -2.0, 0.5, 0.3, -0.2, 0.4]).unwrap();
    let second = Pose3::exp([-0.2, 0.7, 1.1, -0.5, 0.1, 0.2]).unwrap();
    let composed = first.compose(&second).unwrap();
    assert_point3(
        composed.transform_point(point),
        first.transform_point(second.transform_point(point)),
        2.0e-10,
    );
    assert_vector3(
        composed.transform_vector(vector),
        first.transform_vector(second.transform_vector(vector)),
        2.0e-10,
    );
    assert_pose3(
        &first.compose(&first.inverse().unwrap()).unwrap(),
        &Pose3::identity(),
        2.0e-10,
    );
    assert_pose3(
        &first.inverse().unwrap().compose(&first).unwrap(),
        &Pose3::identity(),
        2.0e-10,
    );
    assert_point3(
        first.inverse_transform_point(first.transform_point(point)),
        point,
        2.0e-10,
    );

    let third = Pose3::exp([0.8, 0.3, -0.9, 0.2, 0.5, -0.4]).unwrap();
    assert_pose3(
        &first.compose(&second).unwrap().compose(&third).unwrap(),
        &first.compose(&second.compose(&third).unwrap()).unwrap(),
        8.0e-10,
    );
}

#[test]
fn pose3_exp_log_handles_tiny_angles_and_near_pi_rotations() {
    let axis = Unit::new_normalize(Vector3::new(1.0, -2.0, 0.5));
    let near_pi = axis.into_inner() * (PI - 1.0e-9);
    let twists = [
        [0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        [1.0, -0.4, 0.2, 0.3, -0.6, 0.1],
        [0.2, 0.5, -0.7, 1.0e-12, -2.0e-12, 3.0e-12],
        [-0.3, 0.8, 1.1, near_pi.x, near_pi.y, near_pi.z],
    ];
    for twist in twists {
        let pose = Pose3::exp(twist).unwrap();
        let recovered = pose.log().unwrap();
        assert_array(recovered, twist, 8.0e-9);
        assert_pose3(&Pose3::exp(recovered).unwrap(), &pose, 3.0e-9);
    }

    let reference = Pose3::exp([4.0, -2.0, 1.0, 0.4, -0.2, 0.7]).unwrap();
    for delta in [
        [0.2, -0.1, 0.3, 0.1, 0.05, -0.2],
        [-1.0e-8, 2.0e-8, 3.0e-8, -4.0e-9, 5.0e-9, -6.0e-9],
    ] {
        let retracted = reference.retract(delta).unwrap();
        assert_array(
            reference.local_difference(&retracted).unwrap(),
            delta,
            2.0e-9,
        );
    }
}

#[test]
fn pose3_adjoint_matches_group_conjugation_and_global_equivariance() {
    let transform = Pose3::exp([1.5, -0.8, 0.4, 0.6, -0.3, 0.2]).unwrap();
    let tangent = [0.3, -0.7, 0.2, -0.1, 0.15, 0.25];
    let mapped = transform.adjoint() * SVector::<f64, 6>::from(tangent);
    let conjugated = transform
        .compose(&Pose3::exp(tangent).unwrap())
        .unwrap()
        .compose(&transform.inverse().unwrap())
        .unwrap();
    assert_pose3(&Pose3::exp(mapped.into()).unwrap(), &conjugated, 8.0e-10);

    let global = Pose3::exp([-8.0, 3.0, 2.0, -0.2, 0.7, -0.4]).unwrap();
    let pose = Pose3::exp([2.0, 5.0, -1.0, 0.4, 0.1, -0.3]).unwrap();
    let delta = [0.4, -0.2, 0.1, 0.15, -0.05, 0.2];
    let globally_retracted = global.compose(&pose).unwrap().retract(delta).unwrap();
    let transformed_retraction = global.compose(&pose.retract(delta).unwrap()).unwrap();
    assert_pose3(&globally_retracted, &transformed_retraction, 8.0e-10);
    assert_array(
        global
            .compose(&pose)
            .unwrap()
            .local_difference(&global.compose(&pose.retract(delta).unwrap()).unwrap())
            .unwrap(),
        delta,
        2.0e-9,
    );
}

#[test]
fn pose3_right_tangent_transform_matches_central_differences() {
    let pose = Pose3::exp([2.0, -3.0, 1.0, 0.4, -0.2, 0.7]).unwrap();
    let local_point = Point3::new(0.4, -1.2, 0.8);
    let rotation = pose.rotation();
    let expected = [
        rotation.transform_vector(&Vector3::x()),
        rotation.transform_vector(&Vector3::y()),
        rotation.transform_vector(&Vector3::z()),
        rotation.transform_vector(&Vector3::x().cross(&local_point.coords)),
        rotation.transform_vector(&Vector3::y().cross(&local_point.coords)),
        rotation.transform_vector(&Vector3::z().cross(&local_point.coords)),
    ];
    let step = 1.0e-6;
    for coordinate in 0..6 {
        let mut positive = [0.0; 6];
        let mut negative = [0.0; 6];
        positive[coordinate] = step;
        negative[coordinate] = -step;
        let derivative = (pose.retract(positive).unwrap().transform_point(local_point)
            - pose.retract(negative).unwrap().transform_point(local_point))
            / (2.0 * step);
        assert_vector3(derivative, expected[coordinate], 3.0e-9);
    }
}

#[test]
fn quaternion_sign_is_canonical_and_invalid_quaternions_reject() {
    let positive = Pose3::from_ambient([1.0, 2.0, 3.0, 0.5, -0.5, 0.5, -0.5]).unwrap();
    let negative = Pose3::from_ambient([1.0, 2.0, 3.0, -0.5, 0.5, -0.5, 0.5]).unwrap();
    assert_array(positive.ambient(), negative.ambient(), 0.0);
    assert_same_bits(&positive.ambient()[3..], &negative.ambient()[3..]);

    let pi_positive = Pose3::from_ambient([0.0, 0.0, 0.0, 0.0, -1.0, 0.0, 0.0]).unwrap();
    let pi_negative = Pose3::from_ambient([0.0, 0.0, 0.0, -0.0, 1.0, -0.0, -0.0]).unwrap();
    assert_array(pi_positive.ambient(), pi_negative.ambient(), 0.0);
    assert_same_bits(&pi_positive.ambient()[3..], &pi_negative.ambient()[3..]);
    assert!(pi_positive.quaternion()[1].is_sign_positive());

    for quaternion in [
        [-1.0, -0.0, 0.0, -0.0],
        [0.0, -1.0, -0.0, 0.0],
        [-0.0, 0.0, -1.0, -0.0],
    ] {
        let canonical = Pose3::try_new(Vector3::zeros(), quaternion)
            .unwrap()
            .quaternion();
        for component in canonical {
            if component == 0.0 {
                assert_eq!(component.to_bits(), 0.0_f64.to_bits());
            }
        }
    }

    let near_unit = 1.0 + 0.5 * QUATERNION_NORM_TOLERANCE;
    let normalized = Pose3::from_ambient([0.0, 0.0, 0.0, near_unit, 0.0, 0.0, 0.0]).unwrap();
    assert_array(normalized.quaternion(), [1.0, 0.0, 0.0, 0.0], 1.0e-15);

    assert!(matches!(
        Pose3::from_ambient([0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]),
        Err(GeometryError::InvalidQuaternionNorm { .. })
    ));
    assert!(matches!(
        Pose3::from_ambient([0.0, 0.0, 0.0, 2.0, 0.0, 0.0, 0.0]),
        Err(GeometryError::InvalidQuaternionNorm { .. })
    ));
    assert!(matches!(
        Pose3::from_ambient([0.0, 0.0, 0.0, f64::NAN, 0.0, 0.0, 0.0]),
        Err(GeometryError::NonFiniteQuaternion)
    ));
    assert!(matches!(
        Pose3::from_ambient([f64::INFINITY, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0]),
        Err(GeometryError::NonFinitePose)
    ));
}

#[test]
fn quaternion_norm_validation_accepts_the_band_and_rejects_beyond_it() {
    for norm in [
        (1.0 - QUATERNION_NORM_TOLERANCE).next_up(),
        (1.0 + QUATERNION_NORM_TOLERANCE).next_down(),
    ] {
        let pose = Pose3::try_new(Vector3::zeros(), [norm, 0.0, 0.0, 0.0]).unwrap();
        assert!((pose.quaternion()[0] - 1.0).abs() <= f64::EPSILON);
    }

    for norm in [
        (1.0 - QUATERNION_NORM_TOLERANCE).next_down(),
        (1.0 + QUATERNION_NORM_TOLERANCE).next_up(),
    ] {
        assert!(matches!(
            Pose3::try_new(Vector3::zeros(), [norm, 0.0, 0.0, 0.0]),
            Err(GeometryError::InvalidQuaternionNorm { .. })
        ));
    }
    assert!(matches!(
        Pose3::try_new(Vector3::zeros(), [1.0e308, 0.0, 0.0, 0.0]),
        Err(GeometryError::InvalidQuaternionNorm { .. })
    ));
}

#[test]
fn pose3_log_is_principal_immediately_on_both_sides_of_pi_tie_band() {
    let delta = 1.5 * QUATERNION_SIGN_TOLERANCE;
    let principal_angle = PI - delta;
    for raw_axis in [
        Vector3::x(),
        Vector3::new(0.0, 1.0, 2.0),
        Vector3::z(),
        Vector3::new(1.0, -2.0, 0.5),
    ] {
        let axis = raw_axis.normalize();
        let below = pose_from_axis_angle(axis, PI - delta);
        let exact = pose_from_axis_angle(axis, PI);
        let above = pose_from_axis_angle(axis, PI + delta);
        assert!(below.quaternion()[0] > 0.0);
        assert!(above.quaternion()[0] < 0.0);
        assert!(above.quaternion()[0].abs() < QUATERNION_SIGN_TOLERANCE);
        let below_rotation = Vector3::from_row_slice(&below.log().unwrap()[3..]);
        let exact_rotation = Vector3::from_row_slice(&exact.log().unwrap()[3..]);
        let above_rotation = Vector3::from_row_slice(&above.log().unwrap()[3..]);

        assert!(below_rotation.norm() < PI);
        assert!(above_rotation.norm() < PI);
        assert!(below_rotation.dot(&axis) > 0.0);
        assert!(above_rotation.dot(&axis) < 0.0);
        assert_vector3(below_rotation, axis * principal_angle, 3.0e-14);
        assert_vector3(exact_rotation, axis * PI, 3.0e-14);
        assert_vector3(above_rotation, -axis * principal_angle, 3.0e-14);
    }
}

#[test]
fn pose3_exp_exact_positive_and_negative_pi_share_canonical_half_turn_bits() {
    for angular in [
        Vector3::new(PI, 0.0, 0.0),
        Vector3::new(0.0, PI, 0.0),
        Vector3::new(0.0, 0.0, PI),
        Vector3::new(0.6 * PI, -0.8 * PI, 0.0),
        Vector3::new(0.0, 0.6 * PI, -0.8 * PI),
    ] {
        assert_eq!(
            angular.x.hypot(angular.y).hypot(angular.z).to_bits(),
            PI.to_bits()
        );
        let positive = Pose3::exp([0.0, 0.0, 0.0, angular.x, angular.y, angular.z]).unwrap();
        let negative = Pose3::exp([0.0, 0.0, 0.0, -angular.x, -angular.y, -angular.z]).unwrap();
        let positive_quaternion = positive.quaternion();
        let negative_quaternion = negative.quaternion();

        assert_eq!(positive_quaternion[0].to_bits(), 0.0_f64.to_bits());
        assert_same_bits(&positive_quaternion, &negative_quaternion);
        let canonical_axis = Vector3::new(
            positive_quaternion[1],
            positive_quaternion[2],
            positive_quaternion[3],
        );
        assert!(
            canonical_axis
                .iter()
                .find(|component| component.abs() > QUATERNION_SIGN_TOLERANCE)
                .is_some_and(|component| component.is_sign_positive())
        );

        let positive_log = positive.log().unwrap();
        let negative_log = negative.log().unwrap();
        assert_same_bits(&positive_log[3..], &negative_log[3..]);
        assert_vector3(
            Vector3::from_row_slice(&positive_log[3..]),
            canonical_axis * PI,
            3.0e-14,
        );
        for point in [Point3::new(0.3, -0.8, 1.2), Point3::new(-2.0, 0.5, 0.7)] {
            assert_point3(
                positive.transform_point(point),
                negative.transform_point(point),
                2.0e-14,
            );
        }
    }

    let below = Pose3::exp([0.0, 0.0, 0.0, PI.next_down(), 0.0, 0.0]).unwrap();
    let above = Pose3::exp([0.0, 0.0, 0.0, PI.next_up(), 0.0, 0.0]).unwrap();
    assert!(below.quaternion()[0] > 0.0);
    assert!(above.quaternion()[0] < 0.0);
    assert!(below.log().unwrap()[3] < PI);
    assert!(above.log().unwrap()[3] < 0.0);
    assert!(above.log().unwrap()[3].abs() <= PI);
}

#[test]
fn checked_pose_transforms_reject_nonfinite_inputs_and_extreme_finite_overflow() {
    let pose2 = Pose2::identity();
    assert!(matches!(
        pose2.try_transform_point(Point2::new(f64::NAN, 0.0)),
        Err(GeometryError::NonFinitePoint)
    ));
    assert!(matches!(
        pose2.try_transform_vector(Vector2::new(0.0, f64::INFINITY)),
        Err(GeometryError::NonFiniteVector)
    ));
    assert!(
        pose2
            .try_transform_vector(Vector2::new(f64::MAX, 0.0))
            .is_ok()
    );
    let extreme2 = Pose2::try_new(Vector2::new(f64::MAX, 0.0), 0.0).unwrap();
    assert!(matches!(
        extreme2.try_transform_point(Point2::new(f64::MAX, 0.0)),
        Err(GeometryError::NonFiniteResult)
    ));
    assert!(matches!(
        extreme2.try_inverse_transform_point(Point2::new(-f64::MAX, 0.0)),
        Err(GeometryError::NonFiniteResult)
    ));
    assert!(matches!(
        extreme2.compose(&extreme2),
        Err(GeometryError::NonFiniteResult)
    ));

    let pose3 = Pose3::identity();
    assert!(matches!(
        pose3.try_transform_point(Point3::new(0.0, f64::NAN, 0.0)),
        Err(GeometryError::NonFinitePoint)
    ));
    assert!(matches!(
        pose3.try_inverse_transform_vector(Vector3::new(0.0, 0.0, f64::INFINITY)),
        Err(GeometryError::NonFiniteVector)
    ));
    assert!(
        pose3
            .try_transform_vector(Vector3::new(f64::MAX, 0.0, 0.0))
            .is_ok()
    );
    let extreme3 = Pose3::try_new(Vector3::new(f64::MAX, 0.0, 0.0), [1.0, 0.0, 0.0, 0.0]).unwrap();
    assert!(matches!(
        extreme3.try_transform_point(Point3::new(f64::MAX, 0.0, 0.0)),
        Err(GeometryError::NonFiniteResult)
    ));
    assert!(matches!(
        extreme3.try_inverse_transform_point(Point3::new(-f64::MAX, 0.0, 0.0)),
        Err(GeometryError::NonFiniteResult)
    ));
    assert!(matches!(
        extreme3.compose(&extreme3),
        Err(GeometryError::NonFiniteResult)
    ));

    let diagonal_rotation = Pose3::exp([0.0, 0.0, 0.0, 0.0, 0.0, PI / 4.0]).unwrap();
    assert!(matches!(
        diagonal_rotation.try_transform_vector(Vector3::new(f64::MAX, f64::MAX, 0.0)),
        Err(GeometryError::NonFiniteResult)
    ));
}

#[test]
fn nonfinite_pose_and_tangent_inputs_reject() {
    assert!(matches!(
        Pose2::from_ambient([0.0, f64::NAN, 0.0]),
        Err(GeometryError::NonFinitePose)
    ));
    assert!(matches!(
        Pose2::exp([0.0, 0.0, f64::INFINITY]),
        Err(GeometryError::NonFiniteTangent)
    ));
    assert!(matches!(
        Pose3::exp([0.0, 0.0, 0.0, f64::NAN, 0.0, 0.0]),
        Err(GeometryError::NonFiniteTangent)
    ));
}

fn pose_from_axis_angle(axis: Vector3<f64>, angle: f64) -> Pose3 {
    let half_angle = 0.5 * angle;
    let sine = half_angle.sin();
    Pose3::try_new(
        Vector3::zeros(),
        [
            half_angle.cos(),
            axis.x * sine,
            axis.y * sine,
            axis.z * sine,
        ],
    )
    .unwrap()
}

fn assert_pose2(actual: &Pose2, expected: &Pose2, tolerance: f64) {
    for point in [
        Point2::origin(),
        Point2::new(1.0, 0.0),
        Point2::new(-0.3, 1.7),
    ] {
        assert_point2(
            actual.transform_point(point),
            expected.transform_point(point),
            tolerance,
        );
    }
}

fn assert_pose3(actual: &Pose3, expected: &Pose3, tolerance: f64) {
    for point in [
        Point3::origin(),
        Point3::new(1.0, 0.0, 0.0),
        Point3::new(-0.3, 1.7, 0.8),
    ] {
        assert_point3(
            actual.transform_point(point),
            expected.transform_point(point),
            tolerance,
        );
    }
}

fn assert_point2(actual: Point2<f64>, expected: Point2<f64>, tolerance: f64) {
    assert_vector2(actual - expected, Vector2::zeros(), tolerance);
}

fn assert_vector2(actual: Vector2<f64>, expected: Vector2<f64>, tolerance: f64) {
    let error = (actual - expected).norm();
    assert!(
        error <= tolerance,
        "actual={actual:?}, expected={expected:?}, error={error}, tolerance={tolerance}"
    );
}

fn assert_point3(actual: Point3<f64>, expected: Point3<f64>, tolerance: f64) {
    assert_vector3(actual - expected, Vector3::zeros(), tolerance);
}

fn assert_vector3(actual: Vector3<f64>, expected: Vector3<f64>, tolerance: f64) {
    let error = (actual - expected).norm();
    assert!(
        error <= tolerance,
        "actual={actual:?}, expected={expected:?}, error={error}, tolerance={tolerance}"
    );
}

fn assert_array<const N: usize>(actual: [f64; N], expected: [f64; N], tolerance: f64) {
    for index in 0..N {
        assert!(
            (actual[index] - expected[index]).abs() <= tolerance,
            "actual={actual:?}, expected={expected:?}, index={index}, tolerance={tolerance}"
        );
    }
}

fn assert_same_bits(actual: &[f64], expected: &[f64]) {
    assert_eq!(actual.len(), expected.len());
    for (actual, expected) in actual.iter().zip(expected) {
        assert_eq!(actual.to_bits(), expected.to_bits());
    }
}
