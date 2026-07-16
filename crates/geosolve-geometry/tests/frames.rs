use geosolve_geometry::{
    FRAME_ORTHONORMAL_TOLERANCE, Frame3, GeometryError, PlaneFrame, Point2, Point3, Pose3, Vec3,
    Vector2, Vector3, Workplane,
};

const TOLERANCE: f64 = 2.0e-10;

#[test]
fn validated_vec3_accepts_only_finite_components() {
    let vector = Vec3::try_new(1.0, -2.0, 3.0).unwrap();
    for (actual, expected) in vector.as_array().into_iter().zip([1.0, -2.0, 3.0]) {
        assert!((actual - expected).abs() <= f64::EPSILON);
    }
    assert!((vector.x() - 1.0).abs() <= f64::EPSILON);
    assert!((vector.y() + 2.0).abs() <= f64::EPSILON);
    assert!((vector.z() - 3.0).abs() <= f64::EPSILON);
    assert_eq!(vector.into_vector(), Vector3::new(1.0, -2.0, 3.0));
    assert!(matches!(
        Vec3::try_new(f64::NAN, 0.0, 0.0),
        Err(GeometryError::NonFiniteVector)
    ));
    assert!(matches!(
        Vec3::try_from([0.0, f64::INFINITY, 0.0]),
        Err(GeometryError::NonFiniteVector)
    ));
}

#[test]
fn frame3_forward_and_inverse_point_vector_transforms_round_trip() {
    let frame = Frame3::try_new(
        Point3::new(10.0, -4.0, 3.0),
        Vector3::y(),
        Vector3::z(),
        Vector3::x(),
    )
    .unwrap();
    let point = Point3::new(2.0, -1.0, 0.5);
    let vector = Vector3::new(-0.3, 1.2, 4.0);
    let world_point = frame.transform_point(point).unwrap();
    let world_vector = frame.transform_vector(vector).unwrap();
    assert_point3(world_point, Point3::new(10.5, -2.0, 2.0), TOLERANCE);
    assert_vector3(world_vector, Vector3::new(4.0, -0.3, 1.2), TOLERANCE);
    assert_point3(
        frame.inverse_transform_point(world_point).unwrap(),
        point,
        TOLERANCE,
    );
    assert_vector3(
        frame.inverse_transform_vector(world_vector).unwrap(),
        vector,
        TOLERANCE,
    );
}

#[test]
fn workplane_forward_and_inverse_point_vector_transforms_round_trip() {
    let workplane =
        Workplane::try_new(Point3::new(10.0, 20.0, 30.0), Vector3::x(), Vector3::z()).unwrap();
    let point = Point2::new(2.0, 3.0);
    let vector = Vector2::new(-4.0, 0.5);
    let world_point = workplane.try_map_point(point).unwrap();
    let world_vector = workplane.try_map_vector(vector).unwrap();
    assert_point3(world_point, Point3::new(12.0, 20.0, 33.0), TOLERANCE);
    assert_vector3(world_vector, Vector3::new(-4.0, 0.0, 0.5), TOLERANCE);
    assert_point2(
        workplane.inverse_map_point(world_point).unwrap(),
        point,
        TOLERANCE,
    );
    assert_vector2(
        workplane.inverse_map_vector(world_vector).unwrap(),
        vector,
        TOLERANCE,
    );
    assert_vector3(workplane.normal(), -Vector3::y(), TOLERANCE);
}

#[test]
fn frame_and_workplane_transforms_are_global_transform_equivariant() {
    let frame = Frame3::try_new(
        Point3::new(1.0, -2.0, 0.5),
        Vector3::x(),
        Vector3::y(),
        Vector3::z(),
    )
    .unwrap();
    let workplane = PlaneFrame::try_new(frame.origin(), frame.x_axis(), frame.y_axis()).unwrap();
    let global = Pose3::exp([3.0, -4.0, 2.0, 0.4, -0.3, 0.2]).unwrap();
    let transformed_frame = Frame3::try_new(
        global.transform_point(frame.origin()),
        global.transform_vector(frame.x_axis()),
        global.transform_vector(frame.y_axis()),
        global.transform_vector(frame.z_axis()),
    )
    .unwrap();
    let transformed_workplane = Workplane::try_new(
        global.transform_point(workplane.origin()),
        global.transform_vector(workplane.u()),
        global.transform_vector(workplane.v()),
    )
    .unwrap();

    let local_point3 = Point3::new(0.5, -0.7, 1.1);
    let local_vector3 = Vector3::new(-0.4, 0.2, 0.9);
    assert_point3(
        transformed_frame.transform_point(local_point3).unwrap(),
        global.transform_point(frame.transform_point(local_point3).unwrap()),
        5.0e-10,
    );
    assert_vector3(
        transformed_frame.transform_vector(local_vector3).unwrap(),
        global.transform_vector(frame.transform_vector(local_vector3).unwrap()),
        5.0e-10,
    );

    let local_point2 = Point2::new(0.5, -0.7);
    let local_vector2 = Vector2::new(-0.4, 0.2);
    assert_point3(
        transformed_workplane.try_map_point(local_point2).unwrap(),
        global.transform_point(workplane.try_map_point(local_point2).unwrap()),
        5.0e-10,
    );
    assert_vector3(
        transformed_workplane.try_map_vector(local_vector2).unwrap(),
        global.transform_vector(workplane.try_map_vector(local_vector2).unwrap()),
        5.0e-10,
    );
}

#[test]
fn invalid_frames_and_off_plane_inverse_inputs_reject() {
    assert!(matches!(
        Frame3::try_new(Point3::origin(), Vector3::x(), Vector3::y(), -Vector3::z()),
        Err(GeometryError::LeftHandedFrame)
    ));
    assert!(matches!(
        Frame3::try_new(
            Point3::origin(),
            Vector3::x(),
            Vector3::new(1.0, 1.0, 0.0).normalize(),
            Vector3::z()
        ),
        Err(GeometryError::NonOrthonormalFrame)
    ));
    assert!(matches!(
        Frame3::try_new(
            Point3::origin(),
            Vector3::zeros(),
            Vector3::y(),
            Vector3::z()
        ),
        Err(GeometryError::DegenerateFrameAxis)
    ));
    assert!(matches!(
        Workplane::try_new(Point3::origin(), Vector3::x() * 2.0, Vector3::y()),
        Err(GeometryError::NonOrthonormalFrame)
    ));
    assert!(matches!(
        Workplane::try_new(Point3::origin(), Vector3::x(), Vector3::x()),
        Err(GeometryError::NonOrthonormalFrame)
    ));
    assert!(matches!(
        Workplane::try_new(Point3::new(f64::NAN, 0.0, 0.0), Vector3::x(), Vector3::y()),
        Err(GeometryError::NonFinitePoint)
    ));

    let workplane = Workplane::try_new(Point3::origin(), Vector3::x(), Vector3::y()).unwrap();
    assert!(matches!(
        workplane.inverse_map_point(Point3::new(1.0, 2.0, 0.01)),
        Err(GeometryError::OffWorkplane { .. })
    ));
    assert!(matches!(
        workplane.inverse_map_vector(Vector3::new(1.0, 2.0, 0.01)),
        Err(GeometryError::OffWorkplane { .. })
    ));

    assert!(matches!(
        PlaneFrame::try_new(Point3::origin(), Vector3::x(), Vector3::x()),
        Err(GeometryError::NonOrthonormalFrame)
    ));
}

#[test]
fn workplane_extreme_finite_inputs_do_not_bypass_plane_or_finite_checks() {
    let workplane = Workplane::try_new(Point3::origin(), Vector3::x(), Vector3::y()).unwrap();
    let in_plane_point = Point3::new(1.0e308, -1.0e308, 0.0);
    let in_plane_vector = Vector3::new(-1.0e308, 1.0e308, 0.0);
    let local_point = workplane.inverse_map_point(in_plane_point).unwrap();
    let local_vector = workplane.inverse_map_vector(in_plane_vector).unwrap();
    assert_eq!(local_point.x.to_bits(), 1.0e308_f64.to_bits());
    assert_eq!(local_point.y.to_bits(), (-1.0e308_f64).to_bits());
    assert_eq!(local_vector.x.to_bits(), (-1.0e308_f64).to_bits());
    assert_eq!(local_vector.y.to_bits(), 1.0e308_f64.to_bits());

    assert!(matches!(
        workplane.inverse_map_point(Point3::new(1.0e308, 1.0e308, 1.0e308)),
        Err(GeometryError::OffWorkplane { .. })
    ));
    assert!(matches!(
        workplane.inverse_map_vector(Vector3::new(1.0e308, 1.0e308, 1.0e308)),
        Err(GeometryError::OffWorkplane { .. })
    ));

    let normal = Vector3::new(1.0, 1.0, 1.0).normalize();
    let u = Vector3::new(1.0, -1.0, 0.0).normalize();
    let v = normal.cross(&u);
    let oblique = Workplane::try_new(Point3::origin(), u, v).unwrap();
    assert!(matches!(
        oblique.inverse_map_vector(Vector3::repeat(1.1e308)),
        Err(GeometryError::NonFiniteResult)
    ));

    let separated =
        Workplane::try_new(Point3::new(-1.0e308, 0.0, 0.0), Vector3::x(), Vector3::y()).unwrap();
    assert!(matches!(
        separated.inverse_map_point(Point3::new(1.0e308, 0.0, 0.0)),
        Err(GeometryError::NonFiniteResult)
    ));
}

#[test]
fn frame_construction_normalizes_near_unit_axes_and_rejects_outside_tolerance() {
    let inside = 0.5 * FRAME_ORTHONORMAL_TOLERANCE;
    let workplane = Workplane::try_new(
        Point3::new(1.0, 2.0, 3.0),
        Vector3::x() * (1.0 + inside),
        Vector3::new(inside, 1.0, 0.0),
    )
    .unwrap();
    assert!((workplane.u().norm() - 1.0).abs() <= 4.0 * f64::EPSILON);
    assert!((workplane.v().norm() - 1.0).abs() <= 4.0 * f64::EPSILON);
    assert_eq!(workplane.origin(), Point3::new(1.0, 2.0, 3.0));

    let frame = Frame3::try_new(
        Point3::origin(),
        Vector3::x() * (1.0 - inside),
        Vector3::y(),
        Vector3::z(),
    )
    .unwrap();
    assert!((frame.x_axis().norm() - 1.0).abs() <= 4.0 * f64::EPSILON);

    let outside = 2.0 * FRAME_ORTHONORMAL_TOLERANCE;
    assert!(matches!(
        Workplane::try_new(
            Point3::origin(),
            Vector3::x() * (1.0 + outside),
            Vector3::y()
        ),
        Err(GeometryError::NonOrthonormalFrame)
    ));
    assert!(matches!(
        Workplane::try_new(
            Point3::origin(),
            Vector3::x(),
            Vector3::new(outside, 1.0, 0.0)
        ),
        Err(GeometryError::NonOrthonormalFrame)
    ));
}

#[test]
fn workplane_public_mapping_rejects_nonfinite_inputs_and_results() {
    let workplane = Workplane::try_new(Point3::origin(), Vector3::x(), Vector3::y()).unwrap();
    assert!(matches!(
        workplane.map_point(Point2::new(f64::NAN, 0.0)),
        Err(GeometryError::NonFinitePoint)
    ));
    assert!(matches!(
        workplane.map_vector(Vector2::new(0.0, f64::INFINITY)),
        Err(GeometryError::NonFiniteVector)
    ));

    let diagonal = std::f64::consts::FRAC_1_SQRT_2;
    let oblique = Workplane::try_new(
        Point3::origin(),
        Vector3::new(diagonal, diagonal, 0.0),
        Vector3::new(-diagonal, diagonal, 0.0),
    )
    .unwrap();
    assert!(matches!(
        oblique.map_point(Point2::new(f64::MAX, -f64::MAX)),
        Err(GeometryError::NonFiniteResult)
    ));
    assert!(matches!(
        oblique.map_vector(Vector2::new(f64::MAX, -f64::MAX)),
        Err(GeometryError::NonFiniteResult)
    ));
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
