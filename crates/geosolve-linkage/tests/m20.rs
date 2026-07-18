use std::f64::consts::PI;

use geosolve_core::{AuditEvaluationStatus, HardValidity, SolverConfig};
use geosolve_geometry::{Frame3, Point3, Pose3, Vector3};
use geosolve_linkage::{
    SpatialAssembly, SpatialAssemblyEdit, SpatialAssemblyError, SpatialAssemblySession,
    SpatialAssemblyTransaction, SpatialAxisFeature, SpatialAxisFeatureId, SpatialAxisParity,
    SpatialBodyId, SpatialCoordinateId, SpatialCoordinateKind, SpatialCoordinateValueKind,
    SpatialExampleIds, SpatialExampleKind, SpatialFrameFeatureId, SpatialHingeTarget,
    SpatialModeMonitorId, SpatialModeMonitorKind, SpatialModeSign, SpatialPatch,
    SpatialPlanarTranslationAxis, SpatialPlaneFeature, SpatialPlaneFeatureId,
    SpatialPointFeatureId, SpatialSourceId, SpatialSourceKind, SpatialWorldActionCertification,
    spatial_example,
};

const DIRECTION_TOLERANCE: f64 = 2.0e-12;
const RESIDUAL_TOLERANCE: f64 = 1.0e-9;

#[derive(Clone, Copy, Debug)]
enum JointPrimitive {
    Prismatic(SpatialAxisParity),
    Cylindrical(SpatialAxisParity),
    Planar(SpatialAxisParity),
    Universal,
}

impl JointPrimitive {
    const fn expected_rows(self) -> usize {
        match self {
            Self::Prismatic(_) => 5,
            Self::Cylindrical(_) | Self::Universal => 4,
            Self::Planar(_) => 3,
        }
    }

    const fn expected_internal_mobility(self) -> usize {
        6 - self.expected_rows()
    }
}

#[derive(Clone, Copy, Debug)]
enum JointFeaturePair {
    Axes(SpatialAxisFeatureId, SpatialAxisFeatureId),
    Planes(SpatialPlaneFeatureId, SpatialPlaneFeatureId),
}

struct JointFixture {
    assembly: SpatialAssembly,
    first_body: SpatialBodyId,
    second_body: SpatialBodyId,
    source: SpatialSourceId,
    features: JointFeaturePair,
}

#[test]
fn clocked_axis_and_plane_features_transform_exactly_on_multiple_bodies() {
    let first_pose = Pose3::exp([1.7, -0.8, 0.45, 0.31, -0.27, 0.19]).unwrap();
    let second_pose = Pose3::exp([-0.6, 1.2, -1.1, -0.24, 0.37, 0.28]).unwrap();
    let first_axis_local = clocked_frame(Point3::new(0.4, -0.7, 0.9), [0.43, -0.31, 0.22]);
    let first_plane_local = clocked_frame(Point3::new(-1.1, 0.3, 0.6), [-0.29, 0.41, 0.34]);
    let second_axis_local = clocked_frame(Point3::new(0.8, 1.3, -0.5), [0.26, 0.38, -0.47]);
    let second_plane_local = clocked_frame(Point3::new(-0.2, -0.9, 1.4), [-0.35, -0.24, 0.39]);

    let mut assembly = SpatialAssembly::new(1.0).unwrap();
    let first_body = assembly.add_body("first body", first_pose).unwrap();
    let second_body = assembly.add_body("second body", second_pose).unwrap();
    let first_axis = assembly
        .add_axis_feature("first clocked axis", first_body, first_axis_local)
        .unwrap();
    let second_axis = assembly
        .add_axis_feature("second clocked axis", second_body, second_axis_local)
        .unwrap();
    let first_plane = assembly
        .add_plane_feature("first clocked plane", first_body, first_plane_local)
        .unwrap();
    let second_plane = assembly
        .add_plane_feature("second clocked plane", second_body, second_plane_local)
        .unwrap();
    assembly
        .add_physical_ground("first ground", first_body)
        .unwrap();
    assembly
        .add_physical_ground("second ground", second_body)
        .unwrap();

    let session = SpatialAssemblySession::new(assembly, SolverConfig::default()).unwrap();
    let geometry = &session.accepted_result().geometry;
    assert_eq!(
        geometry
            .axes
            .iter()
            .map(|feature| feature.feature_id)
            .collect::<Vec<_>>(),
        vec![first_axis, second_axis]
    );
    assert_eq!(
        geometry
            .planes
            .iter()
            .map(|feature| feature.feature_id)
            .collect::<Vec<_>>(),
        vec![first_plane, second_plane]
    );

    for (feature, body_pose, local) in [
        (first_axis, first_pose, first_axis_local),
        (second_axis, second_pose, second_axis_local),
    ] {
        let transformed = *geometry.axis_feature(feature).unwrap();
        let expected = transform_frame(body_pose, local);
        assert_frame_close(transformed.world_frame(), expected, 2.0e-12);
        assert_point_close(transformed.origin(), expected.origin(), 2.0e-12);
        assert_vector_close(
            transformed.direction(),
            expected.z_axis(),
            DIRECTION_TOLERANCE,
        );
        assert_vector_close(transformed.axis(), expected.z_axis(), DIRECTION_TOLERANCE);
        assert_vector_close(
            transformed.x_clock(),
            expected.x_axis(),
            DIRECTION_TOLERANCE,
        );
        assert_vector_close(
            transformed.y_clock(),
            expected.y_axis(),
            DIRECTION_TOLERANCE,
        );
        assert_eq!(geometry.world_axis_frame(feature), Some(transformed.world));
    }
    for (feature, body_pose, local) in [
        (first_plane, first_pose, first_plane_local),
        (second_plane, second_pose, second_plane_local),
    ] {
        let transformed = *geometry.plane_feature(feature).unwrap();
        let expected = transform_frame(body_pose, local);
        assert_frame_close(transformed.world_frame(), expected, 2.0e-12);
        assert_point_close(transformed.origin(), expected.origin(), 2.0e-12);
        assert_vector_close(transformed.normal(), expected.z_axis(), DIRECTION_TOLERANCE);
        assert_vector_close(
            transformed.x_clock(),
            expected.x_axis(),
            DIRECTION_TOLERANCE,
        );
        assert_vector_close(
            transformed.y_clock(),
            expected.y_axis(),
            DIRECTION_TOLERANCE,
        );
        assert_eq!(geometry.world_plane_frame(feature), Some(transformed.world));
    }
}

#[test]
fn axis_and_plane_world_frames_are_scale_stable_and_common_left_equivariant() {
    let mut reference_axis = None;
    let mut reference_plane = None;
    for scale in [1.0e-6, 1.0, 1.0e6] {
        let (base_axis, base_plane) = solved_scale_features(scale, None);
        let common_left =
            Pose3::exp([0.9 * scale, -1.4 * scale, 0.7 * scale, -0.32, 0.21, 0.36]).unwrap();
        let (moved_axis, moved_plane) = solved_scale_features(scale, Some(common_left));
        let length_tolerance = 2.0e-12 * scale.max(1.0);

        assert_frame_close(
            moved_axis,
            transform_frame(common_left, base_axis),
            length_tolerance,
        );
        assert_frame_close(
            moved_plane,
            transform_frame(common_left, base_plane),
            length_tolerance,
        );

        let normalized_axis = scale_frame_origin(base_axis, 1.0 / scale);
        let normalized_plane = scale_frame_origin(base_plane, 1.0 / scale);
        if let Some(reference) = reference_axis {
            assert_frame_close(normalized_axis, reference, 2.0e-10);
        } else {
            reference_axis = Some(normalized_axis);
        }
        if let Some(reference) = reference_plane {
            assert_frame_close(normalized_plane, reference, 2.0e-10);
        } else {
            reference_plane = Some(normalized_plane);
        }
    }
}

#[test]
#[allow(clippy::too_many_lines)]
fn feature_ids_and_iteration_are_global_and_unused_features_add_no_equations() {
    let mut assembly = SpatialAssembly::new(1.0).unwrap();
    let first_body = assembly.add_body("first", Pose3::identity()).unwrap();
    let second_body = assembly.add_body("second", Pose3::identity()).unwrap();
    let first_point = assembly
        .add_point_feature("first point", first_body, Point3::origin())
        .unwrap();
    let second_point = assembly
        .add_point_feature("second point", second_body, Point3::origin())
        .unwrap();
    let frame = clocked_frame(Point3::new(0.2, -0.1, 0.3), [0.3, -0.2, 0.4]);
    let unused_frame = assembly
        .add_frame_feature("unused frame", first_body, frame)
        .unwrap();
    let ball = assembly
        .add_ball_joint("physical ball", first_point, second_point)
        .unwrap();
    let baseline = assembly.clone();

    let first_axis = assembly
        .add_axis_feature("axis one", second_body, frame)
        .unwrap();
    let first_plane = assembly
        .add_plane_feature("plane one", first_body, frame)
        .unwrap();
    let second_axis = assembly
        .add_axis_feature("axis two", first_body, frame)
        .unwrap();
    let second_plane = assembly
        .add_plane_feature("plane two", second_body, frame)
        .unwrap();

    assert_eq!(
        [
            first_body.as_u64(),
            second_body.as_u64(),
            first_point.as_u64(),
            second_point.as_u64(),
            unused_frame.as_u64(),
            ball.as_u64(),
            first_axis.as_u64(),
            first_plane.as_u64(),
            second_axis.as_u64(),
            second_plane.as_u64(),
        ],
        [1, 2, 3, 4, 5, 6, 7, 8, 9, 10]
    );
    assert_eq!(
        assembly
            .axis_features()
            .iter()
            .map(SpatialAxisFeature::id)
            .collect::<Vec<_>>(),
        vec![first_axis, second_axis]
    );
    assert_eq!(
        assembly
            .plane_features()
            .iter()
            .map(SpatialPlaneFeature::id)
            .collect::<Vec<_>>(),
        vec![first_plane, second_plane]
    );
    assert_eq!(
        assembly.axis_feature(first_axis).unwrap().label(),
        "axis one"
    );
    assert_eq!(
        assembly.plane_feature(second_plane).unwrap().body(),
        second_body
    );

    let compiled = assembly.compile().unwrap();
    assert_eq!(
        compiled
            .axis_features()
            .iter()
            .map(SpatialAxisFeature::id)
            .collect::<Vec<_>>(),
        vec![first_axis, second_axis]
    );
    assert_eq!(
        compiled
            .plane_features()
            .iter()
            .map(SpatialPlaneFeature::id)
            .collect::<Vec<_>>(),
        vec![first_plane, second_plane]
    );

    let baseline = SpatialAssemblySession::new(baseline, SolverConfig::default()).unwrap();
    let enriched = SpatialAssemblySession::new(assembly, SolverConfig::default()).unwrap();
    assert_eq!(
        enriched.accepted_result().core_report,
        baseline.accepted_result().core_report
    );
    assert_eq!(enriched.gauge_report(), baseline.gauge_report());
    assert_eq!(enriched.source_mappings(), baseline.source_mappings());
    assert_eq!(
        enriched
            .core_session()
            .accepted_hard_linearization()
            .unwrap(),
        baseline
            .core_session()
            .accepted_hard_linearization()
            .unwrap()
    );
    assert_eq!(enriched.accepted_result().geometry.axes.len(), 2);
    assert_eq!(enriched.accepted_result().geometry.planes.len(), 2);
    assert_eq!(enriched.accepted_result().core_report.rank, 3);
    assert_eq!(enriched.gauge_report().gauge_dof, 6);
    assert_eq!(enriched.gauge_report().internal_mobility, 3);
    assert_eq!(audit_row_count(&enriched), 3);
}

#[test]
fn checked_frames_and_checked_feature_additions_reject_malformed_input() {
    assert!(
        Frame3::try_new(
            Point3::new(f64::NAN, 0.0, 0.0),
            Vector3::x(),
            Vector3::y(),
            Vector3::z(),
        )
        .is_err()
    );
    assert!(
        Frame3::try_new(
            Point3::origin(),
            Vector3::zeros(),
            Vector3::y(),
            Vector3::z(),
        )
        .is_err()
    );
    assert!(Frame3::try_new(Point3::origin(), Vector3::x(), Vector3::x(), Vector3::z(),).is_err());
    assert!(Frame3::try_new(Point3::origin(), Vector3::x(), Vector3::y(), -Vector3::z(),).is_err());
    assert!(
        Frame3::try_new(
            Point3::origin(),
            Vector3::new(f64::INFINITY, 0.0, 0.0),
            Vector3::y(),
            Vector3::z(),
        )
        .is_err()
    );

    let frame = clocked_frame(Point3::origin(), [0.2, 0.3, -0.4]);
    let mut assembly = SpatialAssembly::new(1.0).unwrap();
    let body = assembly.add_body("body", Pose3::identity()).unwrap();
    assert!(matches!(
        assembly.add_axis_feature("", body, frame),
        Err(SpatialAssemblyError::InvalidLabel { .. })
    ));
    assert!(matches!(
        assembly.add_plane_feature("   ", body, frame),
        Err(SpatialAssemblyError::InvalidLabel { .. })
    ));

    let mut foreign = SpatialAssembly::new(1.0).unwrap();
    foreign
        .add_body("foreign first", Pose3::identity())
        .unwrap();
    let unknown_body = foreign
        .add_body("foreign second", Pose3::identity())
        .unwrap();
    assert!(matches!(
        assembly.add_axis_feature("unknown body axis", unknown_body, frame),
        Err(SpatialAssemblyError::UnknownBody(id)) if id == unknown_body
    ));
    assert!(matches!(
        assembly.add_plane_feature("unknown body plane", unknown_body, frame),
        Err(SpatialAssemblyError::UnknownBody(id)) if id == unknown_body
    ));

    let axis = assembly
        .add_axis_feature("valid axis", body, frame)
        .unwrap();
    let plane = assembly
        .add_plane_feature("valid plane", body, frame)
        .unwrap();
    assert_eq!(axis.as_u64(), 2);
    assert_eq!(plane.as_u64(), 3);
    assembly.compile().unwrap();
}

#[test]
#[allow(clippy::too_many_lines)]
fn axis_and_plane_patches_commit_once_and_rejections_retain_every_view() {
    let huge_pose = Pose3::try_new(Vector3::new(f64::MAX, 0.0, 0.0), [1.0, 0.0, 0.0, 0.0]).unwrap();
    let identity_frame =
        Frame3::try_new(Point3::origin(), Vector3::x(), Vector3::y(), Vector3::z()).unwrap();
    let mut assembly = SpatialAssembly::new(1.0).unwrap();
    let body = assembly.add_body("huge grounded body", huge_pose).unwrap();
    let axis = assembly
        .add_axis_feature("editable axis", body, identity_frame)
        .unwrap();
    let plane = assembly
        .add_plane_feature("editable plane", body, identity_frame)
        .unwrap();
    assembly.add_physical_ground("huge ground", body).unwrap();
    let mut session = SpatialAssemblySession::new(assembly, SolverConfig::default()).unwrap();

    let axis_edit = clocked_frame(Point3::new(1.0, -2.0, 0.5), [0.31, -0.22, 0.27]);
    let initial_revision = session.revision();
    session
        .apply_patch(
            initial_revision,
            SpatialPatch::AxisLocal {
                feature: axis,
                local_frame: axis_edit,
            },
        )
        .unwrap();
    assert_eq!(session.revision(), initial_revision + 1);
    assert_eq!(
        session.assembly().axis_feature(axis).unwrap().local_frame(),
        axis_edit
    );

    let plane_edit = clocked_frame(Point3::new(-3.0, 0.25, 2.0), [-0.18, 0.36, 0.29]);
    let axis_revision = session.revision();
    session
        .apply_patch(
            axis_revision,
            SpatialPatch::PlaneLocal {
                feature: plane,
                local_frame: plane_edit,
            },
        )
        .unwrap();
    assert_eq!(session.revision(), axis_revision + 1);
    assert_eq!(
        session
            .assembly()
            .plane_feature(plane)
            .unwrap()
            .local_frame(),
        plane_edit
    );

    let accepted_revision = session.revision();
    let accepted_assembly = session.assembly().clone();
    let accepted_geometry = session.accepted_result().geometry.clone();
    let accepted_audit = session.accepted_result().display_audit.clone();
    let accepted_mappings = session.source_mappings().to_vec();
    let accepted_gauge = session.gauge_report().clone();
    let accepted_core = session.core_session().report().clone();
    let accepted_linearization = session
        .core_session()
        .accepted_hard_linearization()
        .unwrap();
    let assert_retained = |candidate: &SpatialAssemblySession| {
        assert_eq!(candidate.revision(), accepted_revision);
        assert_eq!(candidate.assembly(), &accepted_assembly);
        assert_eq!(candidate.accepted_result().geometry, accepted_geometry);
        assert_eq!(candidate.accepted_result().display_audit, accepted_audit);
        assert_eq!(candidate.source_mappings(), accepted_mappings);
        assert_eq!(candidate.gauge_report(), &accepted_gauge);
        assert_eq!(candidate.core_session().report(), &accepted_core);
        assert_eq!(
            candidate
                .core_session()
                .accepted_hard_linearization()
                .unwrap(),
            accepted_linearization
        );
    };

    assert!(matches!(
        session.apply_patch(
            accepted_revision - 1,
            SpatialPatch::PlaneLocal {
                feature: plane,
                local_frame: identity_frame,
            },
        ),
        Err(SpatialAssemblyError::StaleRevision { .. })
    ));
    assert_retained(&session);

    let unknown_axis = foreign_axis_id();
    assert!(matches!(
        session.apply_patch(
            accepted_revision,
            SpatialPatch::AxisLocal {
                feature: unknown_axis,
                local_frame: identity_frame,
            },
        ),
        Err(SpatialAssemblyError::UnknownAxisFeature(id)) if id == unknown_axis
    ));
    assert_retained(&session);

    let overflow_frame = Frame3::try_new(
        Point3::new(f64::MAX, 0.0, 0.0),
        Vector3::x(),
        Vector3::y(),
        Vector3::z(),
    )
    .unwrap();
    assert!(
        session
            .apply_patch(
                accepted_revision,
                SpatialPatch::AxisLocal {
                    feature: axis,
                    local_frame: overflow_frame,
                },
            )
            .is_err()
    );
    assert_retained(&session);
}

#[test]
fn m20_joints_report_exact_floating_and_grounded_rows_rank_and_mobility() {
    for primitive in standard_joints() {
        for grounded in [false, true] {
            let fixture = joint_fixture(primitive, 1.0, grounded, false, None);
            assert_true_feature_offsets(&fixture);
            let source = fixture.source;
            let session = SpatialAssemblySession::new(fixture.assembly, SolverConfig::default())
                .unwrap_or_else(|error| panic!("{primitive:?}, grounded={grounded}: {error:#?}"));
            assert_joint_accepted(&session);
            let expected_internal = primitive.expected_internal_mobility();
            assert_eq!(
                session.accepted_result().core_report.rank,
                primitive.expected_rows()
            );
            assert_eq!(session.accepted_result().core_report.left_nullity, 0);
            assert_eq!(
                session.accepted_result().core_report.right_nullity,
                if grounded {
                    expected_internal
                } else {
                    6 + expected_internal
                }
            );
            assert_eq!(
                session.gauge_report().gauge_dof,
                if grounded { 0 } else { 6 }
            );
            assert_eq!(session.gauge_report().internal_mobility, expected_internal);
            assert_eq!(
                session.gauge_report().components[0].world_action,
                if grounded {
                    SpatialWorldActionCertification::PhysicallyGrounded
                } else {
                    SpatialWorldActionCertification::FloatingSe3
                }
            );
            assert_eq!(
                joint_audit_rows(&session, source),
                primitive.expected_rows()
            );
        }
    }
}

#[test]
fn m20_joint_jacobians_match_exact_and_off_solution_right_tangent_differences_at_all_scales() {
    for scale in [1.0e-6, 1.0, 1.0e6] {
        for primitive in standard_joints() {
            for off_solution in [false, true] {
                let fixture = joint_fixture(primitive, scale, false, off_solution, None);
                assert_true_feature_offsets(&fixture);
                let report = fixture
                    .assembly
                    .compile()
                    .unwrap()
                    .check_jacobians(1.0e-6)
                    .unwrap_or_else(|error| {
                        panic!(
                            "{primitive:?}, scale={scale:e}, off_solution={off_solution}: {error:#?}"
                        )
                    });
                assert_eq!(report.blocks.len(), 2);
                assert!(report.blocks.iter().all(|block| {
                    block.rows == primitive.expected_rows() && block.columns == 6
                }));
                assert!(
                    report.max_relative_error() <= 1.0e-6,
                    "{primitive:?}, scale={scale:e}, off_solution={off_solution}: {report:#?}"
                );
                assert!(
                    report.max_absolute_error() <= 1.0e-6,
                    "zero/near-zero derivative absolute check failed for {primitive:?}, scale={scale:e}, off_solution={off_solution}: {report:#?}"
                );
            }
        }
    }
}

#[test]
fn every_m20_joint_recovers_at_all_scales_and_under_common_left_se3() {
    for scale in [1.0e-6, 1.0, 1.0e6] {
        let common_left =
            Pose3::exp([1.9 * scale, -2.7 * scale, 0.85 * scale, -0.33, 0.24, 0.29]).unwrap();
        for primitive in standard_joints() {
            for transform in [None, Some(common_left)] {
                let fixture = joint_fixture(primitive, scale, true, true, transform);
                let first_body = fixture.first_body;
                let second_body = fixture.second_body;
                let features = fixture.features;
                let session =
                    SpatialAssemblySession::new(fixture.assembly, SolverConfig::default())
                        .unwrap_or_else(|error| {
                            panic!(
                                "{primitive:?}, scale={scale:e}, transformed={}: {error:#?}",
                                transform.is_some()
                            )
                        });
                assert_joint_accepted(&session);
                assert_eq!(
                    session.accepted_result().core_report.rank,
                    primitive.expected_rows()
                );
                assert_eq!(
                    session.gauge_report().internal_mobility,
                    primitive.expected_internal_mobility()
                );
                assert_joint_geometry(
                    primitive,
                    features,
                    &session.accepted_result().geometry,
                    scale,
                );
                assert!(
                    session
                        .accepted_result()
                        .geometry
                        .body_pose(first_body)
                        .unwrap()
                        .ambient()
                        .iter()
                        .chain(
                            session
                                .accepted_result()
                                .geometry
                                .body_pose(second_body)
                                .unwrap()
                                .ambient()
                                .iter()
                        )
                        .all(|value| value.is_finite())
                );
            }
        }
    }
}

#[test]
fn aligned_and_opposed_joint_parity_is_retained_and_prismatic_pi_clock_is_rejected() {
    for parity in [SpatialAxisParity::Aligned, SpatialAxisParity::Opposed] {
        for primitive in [
            JointPrimitive::Prismatic(parity),
            JointPrimitive::Cylindrical(parity),
            JointPrimitive::Planar(parity),
        ] {
            let fixture = joint_fixture(primitive, 1.0, true, true, None);
            let features = fixture.features;
            let session = SpatialAssemblySession::new(fixture.assembly, SolverConfig::default())
                .unwrap_or_else(|error| panic!("{primitive:?}: {error:#?}"));
            assert_joint_accepted(&session);
            assert_joint_geometry(
                primitive,
                features,
                &session.accepted_result().geometry,
                1.0,
            );
        }
    }

    let (false_root, ..) = locked_prismatic_assembly(true);
    assert!(
        false_root
            .compile()
            .unwrap()
            .check_jacobians(1.0e-6)
            .is_ok()
    );
    assert!(matches!(
        SpatialAssemblySession::new(false_root, SolverConfig::default()),
        Err(SpatialAssemblyError::IndependentValidation(message))
            if message.contains("positive clock branch")
    ));
}

#[test]
#[allow(clippy::too_many_lines)]
fn m20_joint_constructors_reject_invalid_same_body_and_stale_features_without_reordering() {
    let frame = identity_frame(Point3::origin());
    let mut assembly = SpatialAssembly::new(1.0).unwrap();
    let first_body = assembly.add_body("first", Pose3::identity()).unwrap();
    let second_body = assembly.add_body("second", Pose3::identity()).unwrap();
    let first_axis = assembly
        .add_axis_feature("first axis", first_body, frame)
        .unwrap();
    let same_body_axis = assembly
        .add_axis_feature("same body axis", first_body, frame)
        .unwrap();
    let second_axis = assembly
        .add_axis_feature("second axis", second_body, frame)
        .unwrap();
    let first_plane = assembly
        .add_plane_feature("first plane", first_body, frame)
        .unwrap();
    let same_body_plane = assembly
        .add_plane_feature("same body plane", first_body, frame)
        .unwrap();
    let second_plane = assembly
        .add_plane_feature("second plane", second_body, frame)
        .unwrap();

    for result in [
        assembly.add_prismatic_joint(
            "same prismatic",
            first_axis,
            same_body_axis,
            SpatialAxisParity::Aligned,
        ),
        assembly.add_cylindrical_joint(
            "same cylindrical",
            first_axis,
            same_body_axis,
            SpatialAxisParity::Aligned,
        ),
        assembly.add_universal_joint("same universal", first_axis, same_body_axis),
        assembly.add_planar_joint(
            "same planar",
            first_plane,
            same_body_plane,
            SpatialAxisParity::Aligned,
        ),
    ] {
        assert!(matches!(
            result,
            Err(SpatialAssemblyError::SameBodyJointEndpoints(id)) if id == first_body
        ));
    }
    assert!(assembly.sources().is_empty());
    assert!(matches!(
        assembly.add_prismatic_joint(
            "stale axis",
            first_axis,
            foreign_axis_id(),
            SpatialAxisParity::Aligned,
        ),
        Err(SpatialAssemblyError::UnknownAxisFeature(_))
    ));
    assert!(matches!(
        assembly.add_planar_joint(
            "stale plane",
            first_plane,
            foreign_plane_id(),
            SpatialAxisParity::Aligned,
        ),
        Err(SpatialAssemblyError::UnknownPlaneFeature(_))
    ));
    assert!(matches!(
        assembly.add_universal_joint("", first_axis, second_axis),
        Err(SpatialAssemblyError::InvalidLabel { .. })
    ));
    assert!(assembly.sources().is_empty());

    let prismatic = assembly
        .add_prismatic_joint(
            "prismatic",
            first_axis,
            second_axis,
            SpatialAxisParity::Aligned,
        )
        .unwrap();
    let cylindrical = assembly
        .add_cylindrical_joint(
            "cylindrical",
            first_axis,
            second_axis,
            SpatialAxisParity::Opposed,
        )
        .unwrap();
    let planar = assembly
        .add_planar_joint(
            "planar",
            first_plane,
            second_plane,
            SpatialAxisParity::Aligned,
        )
        .unwrap();
    let universal = assembly
        .add_universal_joint("universal", first_axis, second_axis)
        .unwrap();
    assert_eq!(
        assembly
            .sources()
            .iter()
            .map(geosolve_linkage::SpatialSource::id)
            .collect::<Vec<_>>(),
        vec![prismatic, cylindrical, planar, universal]
    );
    assert!(matches!(
        assembly.sources()[0].kind(),
        SpatialSourceKind::PrismaticJoint { first, second, parity }
            if first == first_axis
                && second == second_axis
                && parity == SpatialAxisParity::Aligned
    ));
    assert!(matches!(
        assembly.sources()[1].kind(),
        SpatialSourceKind::CylindricalJoint {
            parity: SpatialAxisParity::Opposed,
            ..
        }
    ));
    assert!(matches!(
        assembly.sources()[2].kind(),
        SpatialSourceKind::PlanarJoint { first, second, .. }
            if first == first_plane && second == second_plane
    ));
    assert!(matches!(
        assembly.sources()[3].kind(),
        SpatialSourceKind::UniversalJoint { first, second }
            if first == first_axis && second == second_axis
    ));
}

#[test]
fn branch_invalid_prismatic_equation_root_rolls_back_every_accepted_view() {
    let (assembly, second_axis, pi_clock) = locked_prismatic_assembly(false);
    let mut session = SpatialAssemblySession::new(assembly, SolverConfig::default()).unwrap();
    assert_joint_accepted(&session);
    let revision = session.revision();
    let accepted_assembly = session.assembly().clone();
    let accepted_result = session.accepted_result().clone();
    let accepted_mappings = session.source_mappings().to_vec();
    let accepted_gauge = session.gauge_report().clone();
    let accepted_report = session.core_session().report().clone();
    let accepted_linearization = session
        .core_session()
        .accepted_hard_linearization()
        .unwrap();

    assert!(matches!(
        session.apply_patch(
            revision,
            SpatialPatch::AxisLocal {
                feature: second_axis,
                local_frame: pi_clock,
            },
        ),
        Err(SpatialAssemblyError::IndependentValidation(message))
            if message.contains("positive clock branch")
    ));
    assert_eq!(session.revision(), revision);
    assert_eq!(session.assembly(), &accepted_assembly);
    assert_eq!(session.accepted_result(), &accepted_result);
    assert_eq!(session.source_mappings(), accepted_mappings);
    assert_eq!(session.gauge_report(), &accepted_gauge);
    assert_eq!(session.core_session().report(), &accepted_report);
    assert_eq!(
        session
            .core_session()
            .accepted_hard_linearization()
            .unwrap(),
        accepted_linearization
    );
}

#[test]
fn joint_source_mappings_and_audit_are_deterministic_and_private_gauges_are_fully_isolated() {
    for primitive in standard_joints() {
        let fixture = joint_fixture(primitive, 2.5, false, false, None);
        let source_id = fixture.source;
        let assembly = fixture.assembly;
        let repeated = assembly.clone();
        let session = SpatialAssemblySession::new(assembly, SolverConfig::default()).unwrap();
        let second = SpatialAssemblySession::new(repeated, SolverConfig::default()).unwrap();
        assert_eq!(session.source_mappings(), second.source_mappings());
        assert_eq!(
            session.accepted_result().display_audit,
            second.accepted_result().display_audit
        );
        assert_eq!(session.source_mappings().len(), 1);
        assert_eq!(session.source_mappings()[0].source, source_id);
        assert_eq!(session.source_mappings()[0].residual_ids.len(), 1);
        assert_eq!(session.accepted_result().display_audit.sources.len(), 1);
        let audit = &session.accepted_result().display_audit.sources[0];
        assert_eq!(audit.source_id, session.source_mappings()[0].core_source_id);
        assert_eq!(audit.rows.len(), primitive.expected_rows());
        let (templates, units, scales, binding_names) = expected_audit(primitive, 2.5);
        assert_eq!(
            audit
                .rows
                .iter()
                .map(|row| row.template.as_str())
                .collect::<Vec<_>>(),
            templates
        );
        assert_eq!(
            audit
                .rows
                .iter()
                .map(|row| row.unit.as_str())
                .collect::<Vec<_>>(),
            units
        );
        assert_eq!(
            audit.rows.iter().map(|row| row.scale).collect::<Vec<_>>(),
            scales
        );
        assert!(audit.rows.iter().all(|row| {
            row.evaluation_status == AuditEvaluationStatus::Evaluated
                && row.category == geosolve_core::ResidualCategory::Hard
                && row.raw_residual.is_finite()
                && row.normalized_residual.is_finite()
                && row
                    .bindings
                    .iter()
                    .map(|binding| binding.name.as_str())
                    .collect::<Vec<_>>()
                    == binding_names
        }));
        assert_private_gauges_absent(&session);
    }
}

const fn standard_joints() -> [JointPrimitive; 4] {
    [
        JointPrimitive::Prismatic(SpatialAxisParity::Aligned),
        JointPrimitive::Cylindrical(SpatialAxisParity::Aligned),
        JointPrimitive::Planar(SpatialAxisParity::Aligned),
        JointPrimitive::Universal,
    ]
}

#[allow(clippy::too_many_lines)]
fn joint_fixture(
    primitive: JointPrimitive,
    scale: f64,
    grounded: bool,
    perturb_second: bool,
    common_left: Option<Pose3>,
) -> JointFixture {
    let mut first_pose =
        Pose3::exp([20.0 * scale, -3.4 * scale, 1.7 * scale, 0.31, -0.22, 0.17]).unwrap();
    let mut second_pose = Pose3::exp([
        -0.75 * scale,
        0.42 * scale,
        -0.28 * scale,
        -0.27,
        0.19,
        0.23,
    ])
    .unwrap();
    let desired_pose =
        Pose3::exp([2.6 * scale, -1.3 * scale, 0.72 * scale, 0.28, -0.31, 0.16]).unwrap();
    let mut first_world = frame_from_pose(desired_pose);
    let mut second_world = match primitive {
        JointPrimitive::Prismatic(SpatialAxisParity::Aligned) => {
            translated_frame(first_world, first_world.z_axis() * (4.3 * scale))
        }
        JointPrimitive::Prismatic(SpatialAxisParity::Opposed) => translated_frame(
            opposed_frame(first_world),
            first_world.z_axis() * (4.3 * scale),
        ),
        JointPrimitive::Cylindrical(SpatialAxisParity::Aligned) => translated_frame(
            rotate_frame_about_z(first_world, 0.67),
            first_world.z_axis() * (4.3 * scale),
        ),
        JointPrimitive::Cylindrical(SpatialAxisParity::Opposed) => translated_frame(
            rotate_frame_about_z(opposed_frame(first_world), 0.67),
            first_world.z_axis() * (4.3 * scale),
        ),
        JointPrimitive::Planar(SpatialAxisParity::Aligned) => translated_frame(
            rotate_frame_about_z(first_world, 0.53),
            first_world.x_axis() * (3.7 * scale) - first_world.y_axis() * (1.9 * scale),
        ),
        JointPrimitive::Planar(SpatialAxisParity::Opposed) => translated_frame(
            rotate_frame_about_z(opposed_frame(first_world), 0.53),
            first_world.x_axis() * (3.7 * scale) - first_world.y_axis() * (1.9 * scale),
        ),
        JointPrimitive::Universal => Frame3::try_new(
            first_world.origin(),
            first_world.y_axis(),
            first_world.z_axis(),
            first_world.x_axis(),
        )
        .unwrap(),
    };
    if let Some(transform) = common_left {
        first_pose = transform.compose(&first_pose).unwrap();
        second_pose = transform.compose(&second_pose).unwrap();
        first_world = transform_frame(transform, first_world);
        second_world = transform_frame(transform, second_world);
    }
    let second_guess = if perturb_second {
        second_pose
            .retract([0.09 * scale, -0.07 * scale, 0.05 * scale, 0.06, -0.04, 0.05])
            .unwrap()
    } else {
        second_pose
    };

    let mut assembly = SpatialAssembly::new(scale).unwrap();
    let first_body = assembly.add_body("first body", first_pose).unwrap();
    let second_body = assembly.add_body("second body", second_guess).unwrap();
    if grounded {
        assembly
            .add_physical_ground("first body ground", first_body)
            .unwrap();
    }
    let first_local = local_frame(first_pose, first_world);
    let second_local = local_frame(second_pose, second_world);
    let (source, features) = match primitive {
        JointPrimitive::Prismatic(parity) => {
            let first = assembly
                .add_axis_feature("first prismatic axis", first_body, first_local)
                .unwrap();
            let second = assembly
                .add_axis_feature("second prismatic axis", second_body, second_local)
                .unwrap();
            (
                assembly
                    .add_prismatic_joint("prismatic joint", first, second, parity)
                    .unwrap(),
                JointFeaturePair::Axes(first, second),
            )
        }
        JointPrimitive::Cylindrical(parity) => {
            let first = assembly
                .add_axis_feature("first cylindrical axis", first_body, first_local)
                .unwrap();
            let second = assembly
                .add_axis_feature("second cylindrical axis", second_body, second_local)
                .unwrap();
            (
                assembly
                    .add_cylindrical_joint("cylindrical joint", first, second, parity)
                    .unwrap(),
                JointFeaturePair::Axes(first, second),
            )
        }
        JointPrimitive::Planar(parity) => {
            let first = assembly
                .add_plane_feature("first planar plane", first_body, first_local)
                .unwrap();
            let second = assembly
                .add_plane_feature("second planar plane", second_body, second_local)
                .unwrap();
            (
                assembly
                    .add_planar_joint("planar joint", first, second, parity)
                    .unwrap(),
                JointFeaturePair::Planes(first, second),
            )
        }
        JointPrimitive::Universal => {
            let first = assembly
                .add_axis_feature("first universal axis", first_body, first_local)
                .unwrap();
            let second = assembly
                .add_axis_feature("second universal axis", second_body, second_local)
                .unwrap();
            (
                assembly
                    .add_universal_joint("universal joint", first, second)
                    .unwrap(),
                JointFeaturePair::Axes(first, second),
            )
        }
    };
    JointFixture {
        assembly,
        first_body,
        second_body,
        source,
        features,
    }
}

fn assert_true_feature_offsets(fixture: &JointFixture) {
    let (first, second) = match fixture.features {
        JointFeaturePair::Axes(first, second) => (
            fixture
                .assembly
                .axis_feature(first)
                .unwrap()
                .local_origin()
                .coords
                .norm(),
            fixture
                .assembly
                .axis_feature(second)
                .unwrap()
                .local_origin()
                .coords
                .norm(),
        ),
        JointFeaturePair::Planes(first, second) => (
            fixture
                .assembly
                .plane_feature(first)
                .unwrap()
                .local_origin()
                .coords
                .norm(),
            fixture
                .assembly
                .plane_feature(second)
                .unwrap()
                .local_origin()
                .coords
                .norm(),
        ),
    };
    assert!(first.is_finite() && second.is_finite());
    assert!(first.min(second) > 0.1 * fixture.assembly.model_scale());
    assert!(
        first.max(second) / first.min(second) > 2.0,
        "feature-origin magnitudes are not sufficiently mixed: first={first:e}, second={second:e}"
    );
}

fn assert_joint_accepted(session: &SpatialAssemblySession) {
    let result = session.accepted_result();
    assert_eq!(result.core_report.hard_validity, HardValidity::Valid);
    assert!(result.core_report.hard_residuals_validated);
    assert!(result.core_report.rank_is_valid);
    assert!(result.acceptance_hard_residual_max <= RESIDUAL_TOLERANCE);
    assert!(result.geometry.axes.iter().all(|axis| {
        axis.world
            .origin()
            .coords
            .iter()
            .chain(axis.world.x_axis().iter())
            .chain(axis.world.y_axis().iter())
            .chain(axis.world.z_axis().iter())
            .all(|value| value.is_finite())
    }));
    assert!(result.geometry.planes.iter().all(|plane| {
        plane
            .world
            .origin()
            .coords
            .iter()
            .chain(plane.world.x_axis().iter())
            .chain(plane.world.y_axis().iter())
            .chain(plane.world.z_axis().iter())
            .all(|value| value.is_finite())
    }));
}

fn assert_joint_geometry(
    primitive: JointPrimitive,
    features: JointFeaturePair,
    geometry: &geosolve_linkage::SpatialGeometry,
    scale: f64,
) {
    match (primitive, features) {
        (
            JointPrimitive::Prismatic(parity) | JointPrimitive::Cylindrical(parity),
            JointFeaturePair::Axes(first, second),
        ) => {
            let first = geometry.world_axis_frame(first).unwrap();
            let second = geometry.world_axis_frame(second).unwrap();
            let difference = second.origin() - first.origin();
            assert!(first.x_axis().dot(&difference).abs() / scale <= RESIDUAL_TOLERANCE);
            assert!(first.y_axis().dot(&difference).abs() / scale <= RESIDUAL_TOLERANCE);
            assert!(first.z_axis().dot(&(second.z_axis() * parity.multiplier())) > 0.999);
            if matches!(primitive, JointPrimitive::Prismatic(_)) {
                assert!(first.x_axis().dot(&second.x_axis()) > 0.999);
            }
        }
        (JointPrimitive::Planar(parity), JointFeaturePair::Planes(first, second)) => {
            let first = geometry.world_plane_frame(first).unwrap();
            let second = geometry.world_plane_frame(second).unwrap();
            assert!(
                first
                    .z_axis()
                    .dot(&(second.origin() - first.origin()))
                    .abs()
                    / scale
                    <= RESIDUAL_TOLERANCE
            );
            assert!(first.z_axis().dot(&(second.z_axis() * parity.multiplier())) > 0.999);
        }
        (JointPrimitive::Universal, JointFeaturePair::Axes(first, second)) => {
            let first = geometry.world_axis_frame(first).unwrap();
            let second = geometry.world_axis_frame(second).unwrap();
            assert!((second.origin() - first.origin()).norm() / scale <= RESIDUAL_TOLERANCE);
            assert!(first.z_axis().dot(&second.z_axis()).abs() <= RESIDUAL_TOLERANCE);
        }
        _ => panic!("joint fixture feature type does not match {primitive:?}"),
    }
}

fn joint_audit_rows(session: &SpatialAssemblySession, source: SpatialSourceId) -> usize {
    let mapping = session
        .source_mappings()
        .iter()
        .find(|mapping| mapping.source == source)
        .unwrap();
    session
        .accepted_result()
        .display_audit
        .sources
        .iter()
        .find(|audit| audit.source_id == mapping.core_source_id)
        .unwrap()
        .rows
        .len()
}

fn expected_audit(
    primitive: JointPrimitive,
    scale: f64,
) -> (
    Vec<&'static str>,
    Vec<&'static str>,
    Vec<f64>,
    Vec<&'static str>,
) {
    match primitive {
        JointPrimitive::Prismatic(_) => (
            vec![
                "prismatic joint first x dot (second origin - first origin)",
                "prismatic joint first y dot (second origin - first origin)",
                "prismatic joint first x dot parity-adjusted second z",
                "prismatic joint first y dot parity-adjusted second z",
                "prismatic joint first y dot second x",
            ],
            vec![
                "model-unit",
                "model-unit",
                "dimensionless",
                "dimensionless",
                "dimensionless",
            ],
            vec![scale, scale, 1.0, 1.0, 1.0],
            vec![
                "first_body",
                "first_axis_feature",
                "second_body",
                "second_axis_feature",
                "axis_parity",
            ],
        ),
        JointPrimitive::Cylindrical(_) => (
            vec![
                "cylindrical joint first x dot (second origin - first origin)",
                "cylindrical joint first y dot (second origin - first origin)",
                "cylindrical joint first x dot parity-adjusted second z",
                "cylindrical joint first y dot parity-adjusted second z",
            ],
            vec!["model-unit", "model-unit", "dimensionless", "dimensionless"],
            vec![scale, scale, 1.0, 1.0],
            vec![
                "first_body",
                "first_axis_feature",
                "second_body",
                "second_axis_feature",
                "axis_parity",
            ],
        ),
        JointPrimitive::Planar(_) => (
            vec![
                "planar joint first z dot (second origin - first origin)",
                "planar joint first x dot parity-adjusted second z",
                "planar joint first y dot parity-adjusted second z",
            ],
            vec!["model-unit", "dimensionless", "dimensionless"],
            vec![scale, 1.0, 1.0],
            vec![
                "first_body",
                "first_plane_feature",
                "second_body",
                "second_plane_feature",
                "normal_parity",
            ],
        ),
        JointPrimitive::Universal => (
            vec![
                "universal joint second origin x - first origin x",
                "universal joint second origin y - first origin y",
                "universal joint second origin z - first origin z",
                "universal joint first z dot second z",
            ],
            vec!["model-unit", "model-unit", "model-unit", "dimensionless"],
            vec![scale, scale, scale, 1.0],
            vec![
                "first_body",
                "first_axis_feature",
                "second_body",
                "second_axis_feature",
            ],
        ),
    }
}

fn assert_private_gauges_absent(session: &SpatialAssemblySession) {
    let result = session.accepted_result();
    assert_eq!(
        result.source_mappings.len(),
        session.assembly().sources().len()
    );
    assert_eq!(
        result.display_audit.sources.len(),
        session.assembly().sources().len()
    );
    assert_eq!(session.gauge_report().gauge_dof, 6);
    assert!(
        result
            .display_audit
            .sources
            .iter()
            .all(|source| !source.source_label.contains("numerical gauge"))
    );
    let linearization = session
        .core_session()
        .accepted_hard_linearization()
        .unwrap();
    let physical_rows = linearization
        .components()
        .iter()
        .flat_map(geosolve_core::AcceptedHardComponentLinearization::hard_rows)
        .collect::<Vec<_>>();
    assert!(physical_rows.iter().all(|row| {
        result.source_mappings.iter().any(|mapping| {
            mapping.core_source_id == row.row.source_id
                && mapping.residual_ids.contains(&row.row.residual_id)
        })
    }));
    assert!(result.core_report.conflicting_sources.iter().all(|source| {
        result
            .source_mappings
            .iter()
            .any(|mapping| mapping.core_source_id == *source)
    }));
    assert!(result.core_report.redundant_rows.iter().all(|row| {
        result
            .source_mappings
            .iter()
            .any(|mapping| mapping.core_source_id == row.row.source_id)
    }));
}

fn locked_prismatic_assembly(
    start_at_false_root: bool,
) -> (SpatialAssembly, SpatialAxisFeatureId, Frame3) {
    let identity = identity_frame(Point3::origin());
    let pi_clock =
        Frame3::try_new(Point3::origin(), -Vector3::x(), -Vector3::y(), Vector3::z()).unwrap();
    let mut assembly = SpatialAssembly::new(1.0).unwrap();
    let first_body = assembly.add_body("first", Pose3::identity()).unwrap();
    let second_body = assembly.add_body("second", Pose3::identity()).unwrap();
    let first_axis = assembly
        .add_axis_feature("first axis", first_body, identity)
        .unwrap();
    let second_axis = assembly
        .add_axis_feature(
            "second axis",
            second_body,
            if start_at_false_root {
                pi_clock
            } else {
                identity
            },
        )
        .unwrap();
    assembly
        .add_physical_ground("first ground", first_body)
        .unwrap();
    assembly
        .add_physical_ground("second ground", second_body)
        .unwrap();
    assembly
        .add_prismatic_joint(
            "locked prismatic",
            first_axis,
            second_axis,
            SpatialAxisParity::Aligned,
        )
        .unwrap();
    (assembly, second_axis, pi_clock)
}

fn solved_scale_features(scale: f64, common_left: Option<Pose3>) -> (Frame3, Frame3) {
    let body_pose =
        Pose3::exp([1.2 * scale, -0.7 * scale, 0.35 * scale, 0.28, -0.33, 0.17]).unwrap();
    let body_pose = common_left.map_or(body_pose, |transform| {
        transform.compose(&body_pose).unwrap()
    });
    let axis_local = clocked_frame(
        Point3::new(0.45 * scale, -0.25 * scale, 0.8 * scale),
        [0.37, -0.21, 0.42],
    );
    let plane_local = clocked_frame(
        Point3::new(-0.65 * scale, 0.55 * scale, -0.3 * scale),
        [-0.26, 0.44, 0.31],
    );
    let mut assembly = SpatialAssembly::new(scale).unwrap();
    let body = assembly.add_body("scaled body", body_pose).unwrap();
    let axis = assembly
        .add_axis_feature("scaled axis", body, axis_local)
        .unwrap();
    let plane = assembly
        .add_plane_feature("scaled plane", body, plane_local)
        .unwrap();
    assembly.add_physical_ground("scaled ground", body).unwrap();
    let session = SpatialAssemblySession::new(assembly, SolverConfig::default()).unwrap();
    assert_eq!(
        session.accepted_result().core_report.hard_validity,
        HardValidity::Valid
    );
    assert!(session.accepted_result().acceptance_hard_residual_max <= 1.0e-9);
    assert_eq!(session.accepted_result().core_report.rank, 0);
    assert_eq!(session.gauge_report().internal_mobility, 0);
    (
        session
            .accepted_result()
            .geometry
            .axis_feature(axis)
            .unwrap()
            .world,
        session
            .accepted_result()
            .geometry
            .plane_feature(plane)
            .unwrap()
            .world,
    )
}

fn foreign_axis_id() -> SpatialAxisFeatureId {
    let mut assembly = SpatialAssembly::new(1.0).unwrap();
    for index in 0..8 {
        assembly
            .add_body(format!("foreign body {index}"), Pose3::identity())
            .unwrap();
    }
    let body = assembly.bodies()[0].id();
    assembly
        .add_axis_feature(
            "foreign axis",
            body,
            Frame3::try_new(Point3::origin(), Vector3::x(), Vector3::y(), Vector3::z()).unwrap(),
        )
        .unwrap()
}

fn foreign_plane_id() -> SpatialPlaneFeatureId {
    let mut assembly = SpatialAssembly::new(1.0).unwrap();
    for index in 0..9 {
        assembly
            .add_body(format!("foreign plane body {index}"), Pose3::identity())
            .unwrap();
    }
    let body = assembly.bodies()[0].id();
    assembly
        .add_plane_feature("foreign plane", body, identity_frame(Point3::origin()))
        .unwrap()
}

fn audit_row_count(session: &SpatialAssemblySession) -> usize {
    session
        .accepted_result()
        .display_audit
        .sources
        .iter()
        .map(|source| source.rows.len())
        .sum()
}

fn clocked_frame(origin: Point3<f64>, angular: [f64; 3]) -> Frame3 {
    let orientation = Pose3::exp([0.0, 0.0, 0.0, angular[0], angular[1], angular[2]]).unwrap();
    Frame3::try_new(
        origin,
        orientation.try_transform_vector(Vector3::x()).unwrap(),
        orientation.try_transform_vector(Vector3::y()).unwrap(),
        orientation.try_transform_vector(Vector3::z()).unwrap(),
    )
    .unwrap()
}

fn frame_from_pose(pose: Pose3) -> Frame3 {
    Frame3::try_new(
        Point3::from(pose.translation()),
        pose.try_transform_vector(Vector3::x()).unwrap(),
        pose.try_transform_vector(Vector3::y()).unwrap(),
        pose.try_transform_vector(Vector3::z()).unwrap(),
    )
    .unwrap()
}

fn identity_frame(origin: Point3<f64>) -> Frame3 {
    Frame3::try_new(origin, Vector3::x(), Vector3::y(), Vector3::z()).unwrap()
}

fn local_frame(body: Pose3, world: Frame3) -> Frame3 {
    Frame3::try_new(
        body.try_inverse_transform_point(world.origin()).unwrap(),
        body.try_inverse_transform_vector(world.x_axis()).unwrap(),
        body.try_inverse_transform_vector(world.y_axis()).unwrap(),
        body.try_inverse_transform_vector(world.z_axis()).unwrap(),
    )
    .unwrap()
}

fn translated_frame(frame: Frame3, translation: Vector3<f64>) -> Frame3 {
    Frame3::try_new(
        frame.origin() + translation,
        frame.x_axis(),
        frame.y_axis(),
        frame.z_axis(),
    )
    .unwrap()
}

fn rotate_frame_about_z(frame: Frame3, angle: f64) -> Frame3 {
    let (sine, cosine) = angle.sin_cos();
    Frame3::try_new(
        frame.origin(),
        frame.x_axis() * cosine + frame.y_axis() * sine,
        -frame.x_axis() * sine + frame.y_axis() * cosine,
        frame.z_axis(),
    )
    .unwrap()
}

fn opposed_frame(frame: Frame3) -> Frame3 {
    Frame3::try_new(
        frame.origin(),
        frame.x_axis(),
        -frame.y_axis(),
        -frame.z_axis(),
    )
    .unwrap()
}

fn transform_frame(pose: Pose3, local: Frame3) -> Frame3 {
    Frame3::try_new(
        pose.try_transform_point(local.origin()).unwrap(),
        pose.try_transform_vector(local.x_axis()).unwrap(),
        pose.try_transform_vector(local.y_axis()).unwrap(),
        pose.try_transform_vector(local.z_axis()).unwrap(),
    )
    .unwrap()
}

fn scale_frame_origin(frame: Frame3, scale: f64) -> Frame3 {
    Frame3::try_new(
        Point3::from(frame.origin().coords * scale),
        frame.x_axis(),
        frame.y_axis(),
        frame.z_axis(),
    )
    .unwrap()
}

fn assert_frame_close(actual: Frame3, expected: Frame3, length_tolerance: f64) {
    assert_point_close(actual.origin(), expected.origin(), length_tolerance);
    assert_vector_close(actual.x_axis(), expected.x_axis(), DIRECTION_TOLERANCE);
    assert_vector_close(actual.y_axis(), expected.y_axis(), DIRECTION_TOLERANCE);
    assert_vector_close(actual.z_axis(), expected.z_axis(), DIRECTION_TOLERANCE);
}

fn assert_point_close(actual: Point3<f64>, expected: Point3<f64>, tolerance: f64) {
    let error = (actual - expected).norm();
    assert!(
        error <= tolerance,
        "point error {error:e} exceeds {tolerance:e}: actual={actual:?}, expected={expected:?}"
    );
}

fn assert_vector_close(actual: Vector3<f64>, expected: Vector3<f64>, tolerance: f64) {
    let error = (actual - expected).norm();
    assert!(
        error <= tolerance,
        "vector error {error:e} exceeds {tolerance:e}: actual={actual:?}, expected={expected:?}"
    );
}

#[derive(Clone, Copy, Debug)]
enum MatePrimitive {
    PointDistance,
    AxisAngle,
    AxisAlignment(SpatialAxisParity),
    FrameOffset,
}

impl MatePrimitive {
    const fn expected_rows(self) -> usize {
        match self {
            Self::PointDistance | Self::AxisAngle => 1,
            Self::AxisAlignment(_) => 2,
            Self::FrameOffset => 6,
        }
    }

    const fn expected_internal_mobility(self) -> usize {
        6 - self.expected_rows()
    }
}

#[derive(Clone, Copy, Debug)]
enum MateFeaturePair {
    Points(SpatialPointFeatureId, SpatialPointFeatureId),
    Axes(SpatialAxisFeatureId, SpatialAxisFeatureId),
    Frames(SpatialFrameFeatureId, SpatialFrameFeatureId),
}

struct MateFixture {
    assembly: SpatialAssembly,
    source: SpatialSourceId,
    features: MateFeaturePair,
}

#[test]
fn m20_mates_report_exact_floating_and_grounded_rows_rank_and_mobility() {
    for primitive in standard_mates() {
        for grounded in [false, true] {
            let fixture = mate_fixture(primitive, 1.0, grounded, false, None);
            let source = fixture.source;
            let session = SpatialAssemblySession::new(fixture.assembly, SolverConfig::default())
                .unwrap_or_else(|error| panic!("{primitive:?}, grounded={grounded}: {error:#?}"));
            assert_mate_accepted(&session);
            assert_eq!(
                session.accepted_result().core_report.rank,
                primitive.expected_rows()
            );
            assert_eq!(session.accepted_result().core_report.left_nullity, 0);
            assert_eq!(
                session.accepted_result().core_report.right_nullity,
                primitive.expected_internal_mobility() + if grounded { 0 } else { 6 }
            );
            assert_eq!(
                session.gauge_report().gauge_dof,
                if grounded { 0 } else { 6 }
            );
            assert_eq!(
                session.gauge_report().internal_mobility,
                primitive.expected_internal_mobility()
            );
            assert_eq!(mate_audit_rows(&session, source), primitive.expected_rows());
        }
    }
}

#[test]
fn m20_mate_jacobians_match_exact_and_off_solution_right_tangent_differences_at_all_scales() {
    for scale in [1.0e-6, 1.0, 1.0e6] {
        for primitive in standard_mates() {
            for off_solution in [false, true] {
                let fixture = mate_fixture(primitive, scale, false, off_solution, None);
                let report = fixture
                    .assembly
                    .compile()
                    .unwrap()
                    .check_jacobians(1.0e-6)
                    .unwrap_or_else(|error| {
                        panic!(
                            "{primitive:?}, scale={scale:e}, off_solution={off_solution}: {error:#?}"
                        )
                    });
                assert_eq!(report.blocks.len(), 2);
                assert!(report.blocks.iter().all(|block| {
                    block.rows == primitive.expected_rows() && block.columns == 6
                }));
                assert!(
                    report.max_relative_error() <= 1.0e-6,
                    "{primitive:?}, scale={scale:e}, off_solution={off_solution}: {report:#?}"
                );
                assert!(
                    report.max_absolute_error() <= 1.0e-6,
                    "absolute derivative check failed for {primitive:?}, scale={scale:e}, off_solution={off_solution}: {report:#?}"
                );
            }
        }
    }
}

#[test]
fn every_m20_mate_recovers_at_all_scales_under_arbitrary_common_left_se3() {
    for scale in [1.0e-6, 1.0, 1.0e6] {
        let common_left =
            Pose3::exp([2.1 * scale, -3.2 * scale, 0.65 * scale, -0.36, 0.18, 0.29]).unwrap();
        for primitive in standard_mates() {
            for transform in [None, Some(common_left)] {
                let fixture = mate_fixture(primitive, scale, true, true, transform);
                let features = fixture.features;
                let session =
                    SpatialAssemblySession::new(fixture.assembly, SolverConfig::default())
                        .unwrap_or_else(|error| {
                            panic!(
                                "{primitive:?}, scale={scale:e}, transformed={}: {error:#?}",
                                transform.is_some()
                            )
                        });
                assert_mate_accepted(&session);
                assert_eq!(
                    session.accepted_result().core_report.rank,
                    primitive.expected_rows()
                );
                assert_eq!(
                    session.gauge_report().internal_mobility,
                    primitive.expected_internal_mobility()
                );
                assert_mate_geometry(
                    primitive,
                    features,
                    &session.accepted_result().geometry,
                    scale,
                );
            }
        }
    }
}

#[test]
fn frame_offset_identity_matches_fixed_frame_and_relative_half_turns_are_validated() {
    let fixed = identity_frame_equivalence_session(false);
    let offset = identity_frame_equivalence_session(true);
    assert_eq!(fixed.accepted_result().core_report.rank, 6);
    assert_eq!(offset.accepted_result().core_report.rank, 6);
    assert_frame_close(
        fixed.accepted_result().geometry.frames[1].world,
        offset.accepted_result().geometry.frames[1].world,
        2.0e-11,
    );

    let identity = identity_frame(Point3::origin());
    let requested_half_turn = Frame3::try_new(
        Point3::new(0.7, -0.4, 1.2),
        Vector3::x(),
        -Vector3::y(),
        -Vector3::z(),
    )
    .unwrap();
    let false_relative_root = compose_frame(
        requested_half_turn,
        Frame3::try_new(Point3::origin(), Vector3::x(), -Vector3::y(), -Vector3::z()).unwrap(),
    );
    let requested =
        locked_frame_offset_assembly(identity, requested_half_turn, requested_half_turn);
    let requested = SpatialAssemblySession::new(requested, SolverConfig::default()).unwrap();
    assert_mate_accepted(&requested);
    assert_eq!(requested.accepted_result().core_report.rank, 0);

    let false_root =
        locked_frame_offset_assembly(identity, false_relative_root, requested_half_turn);
    assert!(
        false_root
            .compile()
            .unwrap()
            .check_jacobians(1.0e-6)
            .is_ok()
    );
    assert!(matches!(
        SpatialAssemblySession::new(false_root, SolverConfig::default()),
        Err(SpatialAssemblyError::IndependentValidation(message))
            if message.contains("false half-turn relative to its target")
    ));
}

#[test]
fn axis_alignment_retains_both_parities_and_rejects_the_wrong_equation_root() {
    for parity in [SpatialAxisParity::Aligned, SpatialAxisParity::Opposed] {
        let fixture = mate_fixture(MatePrimitive::AxisAlignment(parity), 1.0, true, true, None);
        let features = fixture.features;
        let session = SpatialAssemblySession::new(fixture.assembly, SolverConfig::default())
            .unwrap_or_else(|error| panic!("{parity:?}: {error:#?}"));
        assert_mate_accepted(&session);
        assert_mate_geometry(
            MatePrimitive::AxisAlignment(parity),
            features,
            &session.accepted_result().geometry,
            1.0,
        );
    }

    let identity = identity_frame(Point3::origin());
    let opposed = opposed_frame(identity);
    let mut wrong = SpatialAssembly::new(1.0).unwrap();
    let first_body = wrong.add_body("first", Pose3::identity()).unwrap();
    let second_body = wrong.add_body("second", Pose3::identity()).unwrap();
    let first = wrong
        .add_axis_feature("first axis", first_body, identity)
        .unwrap();
    let second = wrong
        .add_axis_feature("opposed second axis", second_body, opposed)
        .unwrap();
    wrong
        .add_physical_ground("first ground", first_body)
        .unwrap();
    wrong
        .add_physical_ground("second ground", second_body)
        .unwrap();
    wrong
        .add_axis_alignment_mate(
            "wrong aligned parity",
            first,
            second,
            SpatialAxisParity::Aligned,
        )
        .unwrap();
    assert!(wrong.compile().unwrap().check_jacobians(1.0e-6).is_ok());
    assert!(matches!(
        SpatialAssemblySession::new(wrong, SolverConfig::default()),
        Err(SpatialAssemblyError::IndependentValidation(message))
            if message.contains("axis-alignment mate") && message.contains("Aligned parity")
    ));
}

#[test]
#[allow(clippy::too_many_lines)]
fn m20_mate_constructors_reject_invalid_targets_geometry_offsets_and_features() {
    let identity = identity_frame(Point3::origin());
    let mut assembly = SpatialAssembly::new(1.0).unwrap();
    let first_body = assembly.add_body("first", Pose3::identity()).unwrap();
    let second_body = assembly
        .add_body(
            "second",
            Pose3::exp([2.0, 0.0, 0.0, 0.0, 0.4, 0.0]).unwrap(),
        )
        .unwrap();
    let first_point = assembly
        .add_point_feature("first point", first_body, Point3::origin())
        .unwrap();
    let same_point = assembly
        .add_point_feature("same body point", first_body, Point3::new(1.0, 0.0, 0.0))
        .unwrap();
    let second_point = assembly
        .add_point_feature("second point", second_body, Point3::origin())
        .unwrap();
    let first_axis = assembly
        .add_axis_feature("first axis", first_body, identity)
        .unwrap();
    let same_axis = assembly
        .add_axis_feature("same body axis", first_body, identity)
        .unwrap();
    let second_axis = assembly
        .add_axis_feature("second axis", second_body, identity)
        .unwrap();
    let first_frame = assembly
        .add_frame_feature("first frame", first_body, identity)
        .unwrap();
    let same_frame = assembly
        .add_frame_feature("same body frame", first_body, identity)
        .unwrap();
    for distance in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        assert!(matches!(
            assembly.add_point_distance_mate(
                "invalid distance",
                first_point,
                second_point,
                distance,
            ),
            Err(SpatialAssemblyError::InvalidField { field, .. })
                if field == "point_distance_mate.distance"
        ));
    }
    for angle in [0.0, PI, -0.1, f64::NAN, f64::INFINITY] {
        assert!(matches!(
            assembly.add_axis_angle_mate("invalid angle", first_axis, second_axis, angle),
            Err(SpatialAssemblyError::InvalidField { field, .. })
                if field == "axis_angle_mate.angle"
        ));
    }
    for result in [
        assembly.add_point_distance_mate("same points", first_point, same_point, 1.0),
        assembly.add_axis_angle_mate("same angle", first_axis, same_axis, 0.7),
        assembly.add_axis_alignment_mate(
            "same alignment",
            first_axis,
            same_axis,
            SpatialAxisParity::Aligned,
        ),
        assembly.add_frame_offset_mate("same frames", first_frame, same_frame, identity),
    ] {
        assert!(matches!(
            result,
            Err(SpatialAssemblyError::SameBodyJointEndpoints(id)) if id == first_body
        ));
    }
    assert!(matches!(
        assembly.add_point_distance_mate("stale point", first_point, foreign_point_id(), 1.0,),
        Err(SpatialAssemblyError::UnknownPointFeature(_))
    ));
    assert!(matches!(
        assembly.add_axis_angle_mate("stale angle", first_axis, foreign_axis_id(), 0.7),
        Err(SpatialAssemblyError::UnknownAxisFeature(_))
    ));
    assert!(matches!(
        assembly.add_axis_alignment_mate(
            "stale alignment",
            first_axis,
            foreign_axis_id(),
            SpatialAxisParity::Aligned,
        ),
        Err(SpatialAssemblyError::UnknownAxisFeature(_))
    ));
    assert!(matches!(
        assembly.add_frame_offset_mate("stale frame", first_frame, foreign_frame_id(), identity,),
        Err(SpatialAssemblyError::UnknownFrameFeature(_))
    ));
    assert!(assembly.sources().is_empty());

    let mut coincident = SpatialAssembly::new(1.0).unwrap();
    let first_body = coincident.add_body("first", Pose3::identity()).unwrap();
    let second_body = coincident.add_body("second", Pose3::identity()).unwrap();
    let first = coincident
        .add_point_feature("first", first_body, Point3::origin())
        .unwrap();
    let second = coincident
        .add_point_feature("second", second_body, Point3::origin())
        .unwrap();
    assert!(matches!(
        coincident.add_point_distance_mate("coincident", first, second, 1.0),
        Err(SpatialAssemblyError::InvalidField { field, .. })
            if field == "point_distance_mate.candidate"
    ));

    let huge = identity_frame(Point3::new(f64::MAX, 0.0, 0.0));
    let mut malformed = SpatialAssembly::new(1.0).unwrap();
    let first_body = malformed.add_body("first", Pose3::identity()).unwrap();
    let second_body = malformed.add_body("second", Pose3::identity()).unwrap();
    let first = malformed
        .add_frame_feature("huge first", first_body, huge)
        .unwrap();
    let second = malformed
        .add_frame_feature("second", second_body, identity)
        .unwrap();
    assert!(
        malformed
            .add_frame_offset_mate("overflowing offset", first, second, huge)
            .is_err()
    );

    let mut singular_angle = SpatialAssembly::new(1.0).unwrap();
    let first_body = singular_angle.add_body("first", Pose3::identity()).unwrap();
    let second_body = singular_angle
        .add_body("second", Pose3::identity())
        .unwrap();
    let first = singular_angle
        .add_axis_feature("first", first_body, identity)
        .unwrap();
    let second = singular_angle
        .add_axis_feature("second", second_body, identity)
        .unwrap();
    singular_angle
        .add_axis_angle_mate("singular candidate", first, second, 0.7)
        .unwrap();
    assert!(
        singular_angle
            .compile()
            .unwrap()
            .check_jacobians(1.0e-6)
            .is_err()
    );
}

#[test]
fn mate_source_mappings_audit_and_variants_are_deterministic() {
    for primitive in standard_mates() {
        let fixture = mate_fixture(primitive, 2.5, false, false, None);
        let source_id = fixture.source;
        let definition = fixture.assembly.source(source_id).unwrap().kind();
        assert_mate_source_variant(primitive, definition);
        let repeated = fixture.assembly.clone();
        let session =
            SpatialAssemblySession::new(fixture.assembly, SolverConfig::default()).unwrap();
        let second = SpatialAssemblySession::new(repeated, SolverConfig::default()).unwrap();
        assert_eq!(session.source_mappings(), second.source_mappings());
        assert_eq!(
            session.accepted_result().display_audit,
            second.accepted_result().display_audit
        );
        assert_eq!(session.source_mappings().len(), 1);
        assert_eq!(session.source_mappings()[0].source, source_id);
        assert_eq!(session.source_mappings()[0].residual_ids.len(), 1);
        let audit = &session.accepted_result().display_audit.sources[0];
        let (templates, units, scales, bindings) = expected_mate_audit(primitive, 2.5);
        assert_eq!(
            audit
                .rows
                .iter()
                .map(|row| row.template.as_str())
                .collect::<Vec<_>>(),
            templates
        );
        assert_eq!(
            audit
                .rows
                .iter()
                .map(|row| row.unit.as_str())
                .collect::<Vec<_>>(),
            units
        );
        assert_eq!(
            audit.rows.iter().map(|row| row.scale).collect::<Vec<_>>(),
            scales
        );
        assert!(audit.rows.iter().all(|row| {
            row.evaluation_status == AuditEvaluationStatus::Evaluated
                && row.category == geosolve_core::ResidualCategory::Hard
                && row.raw_residual.is_finite()
                && row.normalized_residual.is_finite()
                && row
                    .bindings
                    .iter()
                    .map(|binding| binding.name.as_str())
                    .collect::<Vec<_>>()
                    == bindings
        }));
        assert_private_gauges_absent(&session);
    }
}

#[test]
fn mate_branch_and_degeneracy_failures_roll_back_every_accepted_view() {
    let identity = identity_frame(Point3::origin());
    let opposed = opposed_frame(identity);
    let mut alignment = SpatialAssembly::new(1.0).unwrap();
    let first_body = alignment.add_body("first", Pose3::identity()).unwrap();
    let second_body = alignment.add_body("second", Pose3::identity()).unwrap();
    let first = alignment
        .add_axis_feature("first", first_body, identity)
        .unwrap();
    let second = alignment
        .add_axis_feature("second", second_body, identity)
        .unwrap();
    alignment
        .add_physical_ground("first ground", first_body)
        .unwrap();
    alignment
        .add_physical_ground("second ground", second_body)
        .unwrap();
    alignment
        .add_axis_alignment_mate("aligned", first, second, SpatialAxisParity::Aligned)
        .unwrap();
    let alignment = SpatialAssemblySession::new(alignment, SolverConfig::default()).unwrap();
    let error = rejected_patch_retains(
        alignment,
        SpatialPatch::AxisLocal {
            feature: second,
            local_frame: opposed,
        },
    );
    assert!(matches!(
        error,
        SpatialAssemblyError::IndependentValidation(message)
            if message.contains("axis-alignment mate")
    ));

    let mut distance = SpatialAssembly::new(1.0).unwrap();
    let first_body = distance.add_body("first", Pose3::identity()).unwrap();
    let second_body = distance.add_body("second", Pose3::identity()).unwrap();
    let first = distance
        .add_point_feature("first", first_body, Point3::origin())
        .unwrap();
    let second = distance
        .add_point_feature("second", second_body, Point3::new(1.0, 0.0, 0.0))
        .unwrap();
    distance
        .add_physical_ground("first ground", first_body)
        .unwrap();
    distance
        .add_physical_ground("second ground", second_body)
        .unwrap();
    distance
        .add_point_distance_mate("distance", first, second, 1.0)
        .unwrap();
    let distance = SpatialAssemblySession::new(distance, SolverConfig::default()).unwrap();
    let error = rejected_patch_retains(
        distance,
        SpatialPatch::PointLocal {
            feature: second,
            local_point: Point3::origin(),
        },
    );
    assert!(matches!(
        error,
        SpatialAssemblyError::InvalidField { field, .. }
            if field == "point_distance_mate.candidate"
    ));
}

const fn standard_mates() -> [MatePrimitive; 4] {
    [
        MatePrimitive::PointDistance,
        MatePrimitive::AxisAngle,
        MatePrimitive::AxisAlignment(SpatialAxisParity::Aligned),
        MatePrimitive::FrameOffset,
    ]
}

#[allow(clippy::too_many_lines)]
fn mate_fixture(
    primitive: MatePrimitive,
    scale: f64,
    grounded: bool,
    perturb_second: bool,
    common_left: Option<Pose3>,
) -> MateFixture {
    let mut first_pose =
        Pose3::exp([17.0 * scale, -2.8 * scale, 1.4 * scale, 0.29, -0.24, 0.16]).unwrap();
    let mut second_pose = Pose3::exp([
        -0.62 * scale,
        0.51 * scale,
        -0.33 * scale,
        -0.25,
        0.17,
        0.21,
    ])
    .unwrap();
    let feature_pose =
        Pose3::exp([2.3 * scale, -1.1 * scale, 0.68 * scale, 0.27, -0.3, 0.14]).unwrap();
    let mut first_world = frame_from_pose(feature_pose);
    let distance = 3.7 * scale;
    let angle = 0.83;
    let offset = mate_offset(scale);
    let direction = Vector3::new(2.0, -1.0, 3.0) / 14.0_f64.sqrt();
    let mut first_point = first_world.origin();
    let mut second_point = first_point + direction * distance;
    let mut second_world = match primitive {
        MatePrimitive::PointDistance => translated_frame(
            rotate_frame_about_z(first_world, 0.41),
            direction * distance,
        ),
        MatePrimitive::AxisAngle => translated_frame(
            rotate_frame_about_x(first_world, angle),
            Vector3::new(1.7 * scale, -0.8 * scale, 2.1 * scale),
        ),
        MatePrimitive::AxisAlignment(SpatialAxisParity::Aligned) => translated_frame(
            rotate_frame_about_z(first_world, 0.57),
            Vector3::new(-1.4 * scale, 2.2 * scale, 0.9 * scale),
        ),
        MatePrimitive::AxisAlignment(SpatialAxisParity::Opposed) => translated_frame(
            rotate_frame_about_z(opposed_frame(first_world), 0.57),
            Vector3::new(-1.4 * scale, 2.2 * scale, 0.9 * scale),
        ),
        MatePrimitive::FrameOffset => compose_frame(first_world, offset),
    };
    if let Some(transform) = common_left {
        first_pose = transform.compose(&first_pose).unwrap();
        second_pose = transform.compose(&second_pose).unwrap();
        first_world = transform_frame(transform, first_world);
        second_world = transform_frame(transform, second_world);
        first_point = transform.try_transform_point(first_point).unwrap();
        second_point = transform.try_transform_point(second_point).unwrap();
    }
    let second_guess = if perturb_second {
        second_pose
            .retract([
                0.08 * scale,
                -0.06 * scale,
                0.045 * scale,
                0.05,
                -0.035,
                0.04,
            ])
            .unwrap()
    } else {
        second_pose
    };

    let mut assembly = SpatialAssembly::new(scale).unwrap();
    let first_body = assembly.add_body("first body", first_pose).unwrap();
    let second_body = assembly.add_body("second body", second_guess).unwrap();
    if grounded {
        assembly
            .add_physical_ground("first body ground", first_body)
            .unwrap();
    }
    let (source, features) = match primitive {
        MatePrimitive::PointDistance => {
            let first = assembly
                .add_point_feature(
                    "first distance point",
                    first_body,
                    first_pose.try_inverse_transform_point(first_point).unwrap(),
                )
                .unwrap();
            let second = assembly
                .add_point_feature(
                    "second distance point",
                    second_body,
                    second_pose
                        .try_inverse_transform_point(second_point)
                        .unwrap(),
                )
                .unwrap();
            (
                assembly
                    .add_point_distance_mate("point distance mate", first, second, distance)
                    .unwrap(),
                MateFeaturePair::Points(first, second),
            )
        }
        MatePrimitive::AxisAngle => {
            let first = assembly
                .add_axis_feature(
                    "first angle axis",
                    first_body,
                    local_frame(first_pose, first_world),
                )
                .unwrap();
            let second = assembly
                .add_axis_feature(
                    "second angle axis",
                    second_body,
                    local_frame(second_pose, second_world),
                )
                .unwrap();
            (
                assembly
                    .add_axis_angle_mate("axis angle mate", first, second, angle)
                    .unwrap(),
                MateFeaturePair::Axes(first, second),
            )
        }
        MatePrimitive::AxisAlignment(parity) => {
            let first = assembly
                .add_axis_feature(
                    "first alignment axis",
                    first_body,
                    local_frame(first_pose, first_world),
                )
                .unwrap();
            let second = assembly
                .add_axis_feature(
                    "second alignment axis",
                    second_body,
                    local_frame(second_pose, second_world),
                )
                .unwrap();
            (
                assembly
                    .add_axis_alignment_mate("axis alignment mate", first, second, parity)
                    .unwrap(),
                MateFeaturePair::Axes(first, second),
            )
        }
        MatePrimitive::FrameOffset => {
            let first = assembly
                .add_frame_feature(
                    "first offset frame",
                    first_body,
                    local_frame(first_pose, first_world),
                )
                .unwrap();
            let second = assembly
                .add_frame_feature(
                    "second offset frame",
                    second_body,
                    local_frame(second_pose, second_world),
                )
                .unwrap();
            (
                assembly
                    .add_frame_offset_mate("frame offset mate", first, second, offset)
                    .unwrap(),
                MateFeaturePair::Frames(first, second),
            )
        }
    };
    MateFixture {
        assembly,
        source,
        features,
    }
}

fn assert_mate_accepted(session: &SpatialAssemblySession) {
    let result = session.accepted_result();
    assert_eq!(result.core_report.hard_validity, HardValidity::Valid);
    assert!(result.core_report.hard_residuals_validated);
    assert!(result.core_report.rank_is_valid);
    assert!(result.acceptance_hard_residual_max <= RESIDUAL_TOLERANCE);
    assert!(
        result
            .geometry
            .bodies
            .iter()
            .all(|body| { body.pose.ambient().iter().all(|value| value.is_finite()) })
    );
}

fn assert_mate_geometry(
    primitive: MatePrimitive,
    features: MateFeaturePair,
    geometry: &geosolve_linkage::SpatialGeometry,
    scale: f64,
) {
    match (primitive, features) {
        (MatePrimitive::PointDistance, MateFeaturePair::Points(first, second)) => {
            let measured = (geometry.world_point(second).unwrap()
                - geometry.world_point(first).unwrap())
            .norm();
            assert!((measured - 3.7 * scale).abs() / scale <= RESIDUAL_TOLERANCE);
        }
        (MatePrimitive::AxisAngle, MateFeaturePair::Axes(first, second)) => {
            let first = geometry.world_axis_frame(first).unwrap().z_axis();
            let second = geometry.world_axis_frame(second).unwrap().z_axis();
            let principal = first.cross(&second).norm().atan2(first.dot(&second));
            assert!((principal - 0.83).abs() <= RESIDUAL_TOLERANCE);
        }
        (MatePrimitive::AxisAlignment(parity), MateFeaturePair::Axes(first, second)) => {
            let first = geometry.world_axis_frame(first).unwrap();
            let adjusted =
                geometry.world_axis_frame(second).unwrap().z_axis() * parity.multiplier();
            assert!(first.z_axis().dot(&adjusted) > 0.999);
            assert!(first.x_axis().dot(&adjusted).abs() <= RESIDUAL_TOLERANCE);
            assert!(first.y_axis().dot(&adjusted).abs() <= RESIDUAL_TOLERANCE);
        }
        (MatePrimitive::FrameOffset, MateFeaturePair::Frames(first, second)) => {
            let expected = compose_frame(geometry.world_frame(first).unwrap(), mate_offset(scale));
            let actual = geometry.world_frame(second).unwrap();
            assert_point_close(actual.origin(), expected.origin(), 2.0e-9 * scale);
            assert_vector_close(actual.x_axis(), expected.x_axis(), 2.0e-9);
            assert_vector_close(actual.y_axis(), expected.y_axis(), 2.0e-9);
            assert_vector_close(actual.z_axis(), expected.z_axis(), 2.0e-9);
        }
        _ => panic!("mate fixture feature type does not match {primitive:?}"),
    }
}

fn mate_audit_rows(session: &SpatialAssemblySession, source: SpatialSourceId) -> usize {
    joint_audit_rows(session, source)
}

fn assert_mate_source_variant(primitive: MatePrimitive, kind: SpatialSourceKind) {
    match (primitive, kind) {
        (MatePrimitive::PointDistance, SpatialSourceKind::PointDistanceMate { distance, .. }) => {
            assert_eq!(distance.to_bits(), 9.25_f64.to_bits());
        }
        (MatePrimitive::AxisAngle, SpatialSourceKind::AxisAngleMate { angle, .. }) => {
            assert_eq!(angle.to_bits(), 0.83_f64.to_bits());
        }
        (
            MatePrimitive::AxisAlignment(expected),
            SpatialSourceKind::AxisAlignmentMate { parity, .. },
        ) => assert_eq!(parity, expected),
        (MatePrimitive::FrameOffset, SpatialSourceKind::FrameOffsetMate { offset, .. }) => {
            assert_eq!(offset, mate_offset(2.5));
        }
        _ => panic!("unexpected source kind {kind:?} for {primitive:?}"),
    }
}

fn expected_mate_audit(
    primitive: MatePrimitive,
    scale: f64,
) -> (
    Vec<&'static str>,
    Vec<&'static str>,
    Vec<f64>,
    Vec<&'static str>,
) {
    match primitive {
        MatePrimitive::PointDistance => (
            vec![
                "point distance mate norm(second world point - first world point) - target distance",
            ],
            vec!["model-unit"],
            vec![scale],
            vec![
                "first_body",
                "first_point_feature",
                "second_body",
                "second_point_feature",
                "target_distance",
            ],
        ),
        MatePrimitive::AxisAngle => (
            vec!["axis angle mate first z dot second z - cos(target angle)"],
            vec!["dimensionless"],
            vec![1.0],
            vec![
                "first_body",
                "first_axis_feature",
                "second_body",
                "second_axis_feature",
                "target_angle",
            ],
        ),
        MatePrimitive::AxisAlignment(_) => (
            vec![
                "axis alignment mate first x dot parity-adjusted second z",
                "axis alignment mate first y dot parity-adjusted second z",
            ],
            vec!["dimensionless", "dimensionless"],
            vec![1.0, 1.0],
            vec![
                "first_body",
                "first_axis_feature",
                "second_body",
                "second_axis_feature",
                "axis_parity",
            ],
        ),
        MatePrimitive::FrameOffset => (
            vec![
                "frame offset mate second origin x - expected origin x",
                "frame offset mate second origin y - expected origin y",
                "frame offset mate second origin z - expected origin z",
                "frame offset mate expected y dot second x",
                "frame offset mate expected z dot second x",
                "frame offset mate expected z dot second y",
            ],
            vec![
                "model-unit",
                "model-unit",
                "model-unit",
                "dimensionless",
                "dimensionless",
                "dimensionless",
            ],
            vec![scale, scale, scale, 1.0, 1.0, 1.0],
            vec![
                "first_body",
                "first_frame_feature",
                "second_body",
                "second_frame_feature",
                "offset_in_first_frame",
            ],
        ),
    }
}

fn identity_frame_equivalence_session(use_offset: bool) -> SpatialAssemblySession {
    let identity = identity_frame(Point3::origin());
    let mut assembly = SpatialAssembly::new(1.0).unwrap();
    let first_body = assembly.add_body("first", Pose3::identity()).unwrap();
    let second_body = assembly
        .add_body(
            "second",
            Pose3::exp([0.08, -0.06, 0.04, 0.05, -0.03, 0.04]).unwrap(),
        )
        .unwrap();
    let first = assembly
        .add_frame_feature("first frame", first_body, identity)
        .unwrap();
    let second = assembly
        .add_frame_feature("second frame", second_body, identity)
        .unwrap();
    assembly
        .add_physical_ground("first ground", first_body)
        .unwrap();
    if use_offset {
        assembly
            .add_frame_offset_mate("identity offset", first, second, identity)
            .unwrap();
    } else {
        assembly
            .add_fixed_frame("fixed frame", first, second)
            .unwrap();
    }
    SpatialAssemblySession::new(assembly, SolverConfig::default()).unwrap()
}

fn locked_frame_offset_assembly(
    first_local: Frame3,
    second_local: Frame3,
    offset: Frame3,
) -> SpatialAssembly {
    let mut assembly = SpatialAssembly::new(1.0).unwrap();
    let first_body = assembly.add_body("first", Pose3::identity()).unwrap();
    let second_body = assembly.add_body("second", Pose3::identity()).unwrap();
    let first = assembly
        .add_frame_feature("first", first_body, first_local)
        .unwrap();
    let second = assembly
        .add_frame_feature("second", second_body, second_local)
        .unwrap();
    assembly
        .add_physical_ground("first ground", first_body)
        .unwrap();
    assembly
        .add_physical_ground("second ground", second_body)
        .unwrap();
    assembly
        .add_frame_offset_mate("frame offset", first, second, offset)
        .unwrap();
    assembly
}

fn rejected_patch_retains(
    mut session: SpatialAssemblySession,
    patch: SpatialPatch,
) -> SpatialAssemblyError {
    let revision = session.revision();
    let assembly = session.assembly().clone();
    let result = session.accepted_result().clone();
    let mappings = session.source_mappings().to_vec();
    let gauge = session.gauge_report().clone();
    let report = session.core_session().report().clone();
    let linearization = session
        .core_session()
        .accepted_hard_linearization()
        .unwrap();
    let error = session.apply_patch(revision, patch).unwrap_err();
    assert_eq!(session.revision(), revision);
    assert_eq!(session.assembly(), &assembly);
    assert_eq!(session.accepted_result(), &result);
    assert_eq!(session.source_mappings(), mappings);
    assert_eq!(session.gauge_report(), &gauge);
    assert_eq!(session.core_session().report(), &report);
    assert_eq!(
        session
            .core_session()
            .accepted_hard_linearization()
            .unwrap(),
        linearization
    );
    error
}

fn mate_offset(scale: f64) -> Frame3 {
    clocked_frame(
        Point3::new(0.8 * scale, -0.45 * scale, 1.1 * scale),
        [0.34, -0.26, 0.21],
    )
}

fn rotate_frame_about_x(frame: Frame3, angle: f64) -> Frame3 {
    let (sine, cosine) = angle.sin_cos();
    Frame3::try_new(
        frame.origin(),
        frame.x_axis(),
        frame.y_axis() * cosine + frame.z_axis() * sine,
        -frame.y_axis() * sine + frame.z_axis() * cosine,
    )
    .unwrap()
}

fn compose_frame(parent: Frame3, child: Frame3) -> Frame3 {
    Frame3::try_new(
        parent.transform_point(child.origin()).unwrap(),
        parent.transform_vector(child.x_axis()).unwrap(),
        parent.transform_vector(child.y_axis()).unwrap(),
        parent.transform_vector(child.z_axis()).unwrap(),
    )
    .unwrap()
}

fn foreign_point_id() -> SpatialPointFeatureId {
    let mut assembly = SpatialAssembly::new(1.0).unwrap();
    for index in 0..20 {
        assembly
            .add_body(format!("foreign point body {index}"), Pose3::identity())
            .unwrap();
    }
    let body = assembly.bodies()[0].id();
    assembly
        .add_point_feature("foreign point", body, Point3::origin())
        .unwrap()
}

fn foreign_frame_id() -> SpatialFrameFeatureId {
    let mut assembly = SpatialAssembly::new(1.0).unwrap();
    for index in 0..21 {
        assembly
            .add_body(format!("foreign frame body {index}"), Pose3::identity())
            .unwrap();
    }
    let body = assembly.bodies()[0].id();
    assembly
        .add_frame_feature("foreign frame", body, identity_frame(Point3::origin()))
        .unwrap()
}

#[derive(Clone, Copy, Debug)]
enum CoordinateRelation {
    Revolute,
    Prismatic,
    Cylindrical,
}

struct CoordinateFixture {
    assembly: SpatialAssembly,
    parent: SpatialSourceId,
    hinge: Option<SpatialCoordinateId>,
    translation: Option<SpatialCoordinateId>,
    hinge_driver: Option<SpatialSourceId>,
    translation_driver: Option<SpatialSourceId>,
    second_body: SpatialBodyId,
}

#[test]
fn ordered_coordinates_are_global_row_free_and_measure_both_parities() {
    for grounded in [false, true] {
        for parity in [SpatialAxisParity::Aligned, SpatialAxisParity::Opposed] {
            let revolute = coordinate_fixture(
                CoordinateRelation::Revolute,
                1.0,
                grounded,
                parity,
                -1.17,
                0.0,
                -4,
                false,
                false,
                false,
                None,
            );
            let hinge = revolute.hinge.unwrap();
            assert!(revolute.translation.is_none());
            assert!(hinge.as_u64() > revolute.parent.as_u64());
            let session =
                SpatialAssemblySession::new(revolute.assembly, SolverConfig::default()).unwrap();
            assert_eq!(session.accepted_result().core_report.rank, 5);
            assert_eq!(session.source_mappings().len(), 1 + usize::from(grounded));
            assert_hinge_value(&session, hinge, -1.17, -4);

            for translation in [-2.6, 3.4] {
                let prismatic = coordinate_fixture(
                    CoordinateRelation::Prismatic,
                    1.0,
                    grounded,
                    parity,
                    0.0,
                    translation,
                    0,
                    false,
                    false,
                    false,
                    None,
                );
                let coordinate = prismatic.translation.unwrap();
                let session =
                    SpatialAssemblySession::new(prismatic.assembly, SolverConfig::default())
                        .unwrap();
                assert_eq!(session.accepted_result().core_report.rank, 5);
                assert_eq!(session.coordinate_values().len(), 1);
                assert_translation_value(&session, coordinate, translation, 2.0e-12);
            }

            let cylindrical = coordinate_fixture(
                CoordinateRelation::Cylindrical,
                1.0,
                grounded,
                parity,
                0.91,
                -2.3,
                7,
                false,
                false,
                false,
                None,
            );
            let hinge = cylindrical.hinge.unwrap();
            let translation = cylindrical.translation.unwrap();
            assert_eq!(
                cylindrical
                    .assembly
                    .coordinates()
                    .iter()
                    .map(geosolve_linkage::SpatialCoordinate::id)
                    .collect::<Vec<_>>(),
                vec![hinge, translation]
            );
            let session =
                SpatialAssemblySession::new(cylindrical.assembly, SolverConfig::default()).unwrap();
            assert_eq!(session.accepted_result().core_report.rank, 4);
            assert_eq!(session.coordinate_values().len(), 2);
            assert_hinge_value(&session, hinge, 0.91, 7);
            assert_translation_value(&session, translation, -2.3, 2.0e-12);
        }
    }
}

#[test]
#[allow(clippy::too_many_lines)]
fn coordinate_and_driver_guards_reject_stale_wrong_kind_and_incompatible_targets() {
    let mut fixture = coordinate_fixture(
        CoordinateRelation::Cylindrical,
        1.0,
        true,
        SpatialAxisParity::Aligned,
        0.4,
        1.2,
        3,
        false,
        false,
        false,
        None,
    );
    let hinge = fixture.hinge.unwrap();
    let translation = fixture.translation.unwrap();
    assert!(matches!(
        fixture
            .assembly
            .add_hinge_position_driver("wrong coordinate", translation, SpatialHingeTarget {
                principal_phase: 0.4,
                winding: 3,
            }),
        Err(SpatialAssemblyError::WrongCoordinateKind { coordinate, .. })
            if coordinate == translation
    ));
    assert!(matches!(
        fixture
            .assembly
            .add_translation_position_driver("wrong translation", hinge, 1.2),
        Err(SpatialAssemblyError::WrongCoordinateKind { coordinate, .. })
            if coordinate == hinge
    ));
    for phase in [PI, f64::NAN, f64::INFINITY] {
        assert!(matches!(
            fixture.assembly.add_hinge_position_driver(
                "invalid hinge target",
                hinge,
                SpatialHingeTarget {
                    principal_phase: phase,
                    winding: 3,
                },
            ),
            Err(SpatialAssemblyError::InvalidField { field, .. })
                if field == "hinge_position_driver.target"
        ));
    }
    assert!(matches!(
        fixture.assembly.add_hinge_position_driver(
            "wrong winding",
            hinge,
            SpatialHingeTarget {
                principal_phase: 0.4,
                winding: 4,
            },
        ),
        Err(SpatialAssemblyError::WindingMismatch { coordinate, .. }) if coordinate == hinge
    ));
    assert!(matches!(
        fixture
            .assembly
            .add_translation_position_driver("nonfinite", translation, f64::NAN),
        Err(SpatialAssemblyError::InvalidField { field, .. })
            if field == "translation_position_driver.target"
    ));

    fixture
        .assembly
        .add_hinge_position_driver(
            "first hinge driver",
            hinge,
            SpatialHingeTarget {
                principal_phase: 0.4,
                winding: 3,
            },
        )
        .unwrap();
    fixture
        .assembly
        .add_hinge_position_driver(
            "equal duplicate hinge driver",
            hinge,
            SpatialHingeTarget {
                principal_phase: 0.4,
                winding: 3,
            },
        )
        .unwrap();
    assert!(matches!(
        fixture.assembly.add_hinge_position_driver(
            "incompatible duplicate hinge driver",
            hinge,
            SpatialHingeTarget {
                principal_phase: 0.5,
                winding: 3,
            },
        ),
        Err(SpatialAssemblyError::IncompatibleDriverTargets { coordinate })
            if coordinate == hinge
    ));

    let mut wrong_parent = SpatialAssembly::new(1.0).unwrap();
    let first_body = wrong_parent.add_body("first", Pose3::identity()).unwrap();
    let second_body = wrong_parent.add_body("second", Pose3::identity()).unwrap();
    let first = wrong_parent
        .add_point_feature("first", first_body, Point3::origin())
        .unwrap();
    let second = wrong_parent
        .add_point_feature("second", second_body, Point3::origin())
        .unwrap();
    let ball = wrong_parent.add_ball_joint("ball", first, second).unwrap();
    assert!(matches!(
        wrong_parent.add_hinge_coordinate("wrong parent", ball, 0),
        Err(SpatialAssemblyError::WrongCoordinateParent { source_id, .. })
            if source_id == ball
    ));
    assert!(matches!(
        wrong_parent.add_axial_translation_coordinate("stale parent", foreign_source_id()),
        Err(SpatialAssemblyError::UnknownSource(_))
    ));
}

#[test]
#[allow(clippy::too_many_lines)]
fn one_position_driver_removes_one_internal_dof_with_exact_sources_rows_and_audit() {
    for (relation, hinge_driver, translation_driver, expected_rank, expected_internal) in [
        (CoordinateRelation::Revolute, true, false, 6, 0),
        (CoordinateRelation::Prismatic, false, true, 6, 0),
        (CoordinateRelation::Cylindrical, true, false, 5, 1),
        (CoordinateRelation::Cylindrical, false, true, 5, 1),
        (CoordinateRelation::Cylindrical, true, true, 6, 0),
    ] {
        let fixture = coordinate_fixture(
            relation,
            2.5,
            true,
            SpatialAxisParity::Opposed,
            -0.73,
            -3.1,
            -2,
            hinge_driver,
            translation_driver,
            false,
            None,
        );
        let expected_sources = 2 + usize::from(hinge_driver) + usize::from(translation_driver);
        let hinge_source = fixture.hinge_driver;
        let translation_source = fixture.translation_driver;
        let expected_source_order = fixture
            .assembly
            .sources()
            .iter()
            .map(geosolve_linkage::SpatialSource::id)
            .collect::<Vec<_>>();
        let session = SpatialAssemblySession::new(fixture.assembly, SolverConfig::default())
            .unwrap_or_else(|error| panic!("{relation:?}: {error:#?}"));
        assert_eq!(session.source_mappings().len(), expected_sources);
        assert_eq!(
            session
                .source_mappings()
                .iter()
                .map(|mapping| mapping.source)
                .collect::<Vec<_>>(),
            expected_source_order
        );
        assert_eq!(session.accepted_result().core_report.rank, expected_rank);
        assert_eq!(session.gauge_report().internal_mobility, expected_internal);
        assert_eq!(session.accepted_result().core_report.left_nullity, 0);
        for source in [hinge_source, translation_source].into_iter().flatten() {
            let mapping = session
                .source_mappings()
                .iter()
                .find(|mapping| mapping.source == source)
                .unwrap();
            assert_eq!(mapping.residual_ids.len(), 1);
            let audit = session
                .accepted_result()
                .display_audit
                .sources
                .iter()
                .find(|audit| audit.source_id == mapping.core_source_id)
                .unwrap();
            assert_eq!(audit.rows.len(), 1);
            assert_eq!(
                audit.rows[0].evaluation_status,
                AuditEvaluationStatus::Evaluated
            );
            let (template, unit, bindings): (&str, &str, &[&str]) = if Some(source) == hinge_source
            {
                (
                    "hinge position driver cos(target) * (y1 dot x2) - sin(target) * (x1 dot x2)",
                    "dimensionless",
                    &[
                        "coordinate",
                        "coordinate_label",
                        "parent_source",
                        "first_body",
                        "second_body",
                        "axis_parity",
                        "target_principal_phase_rad",
                        "target_winding",
                    ],
                )
            } else {
                (
                    "translation position driver first z dot (second origin - first origin) - target",
                    "model-unit",
                    &[
                        "coordinate",
                        "coordinate_label",
                        "parent_source",
                        "first_body",
                        "second_body",
                        "axis_parity",
                        "target_translation",
                    ],
                )
            };
            assert_eq!(audit.rows[0].template, template);
            assert_eq!(audit.rows[0].unit, unit);
            assert_eq!(
                audit.rows[0]
                    .bindings
                    .iter()
                    .map(|binding| binding.name.as_str())
                    .collect::<Vec<_>>(),
                bindings
            );
            let expected_scale: f64 = if Some(source) == hinge_source {
                1.0
            } else {
                2.5
            };
            assert_eq!(audit.rows[0].scale.to_bits(), expected_scale.to_bits());
            assert!(audit.rows[0].raw_residual.abs() <= RESIDUAL_TOLERANCE * audit.rows[0].scale);
        }
    }
}

#[test]
fn equal_duplicate_drivers_are_redundant_but_incompatible_duplicates_never_solve() {
    let mut fixture = coordinate_fixture(
        CoordinateRelation::Cylindrical,
        1.0,
        true,
        SpatialAxisParity::Aligned,
        0.62,
        0.8,
        1,
        true,
        false,
        false,
        None,
    );
    let hinge = fixture.hinge.unwrap();
    fixture
        .assembly
        .add_hinge_position_driver(
            "equal duplicate",
            hinge,
            SpatialHingeTarget {
                principal_phase: 0.62,
                winding: 1,
            },
        )
        .unwrap();
    let session = SpatialAssemblySession::new(fixture.assembly, SolverConfig::default()).unwrap();
    assert_eq!(session.accepted_result().core_report.rank, 5);
    assert_eq!(session.accepted_result().core_report.left_nullity, 1);
    assert_eq!(session.gauge_report().internal_mobility, 1);
}

#[test]
fn position_driver_jacobians_and_perturbed_recovery_hold_at_all_scales_and_common_left_se3() {
    for scale in [1.0e-6, 1.0, 1.0e6] {
        let common_left =
            Pose3::exp([1.7 * scale, -2.1 * scale, 0.9 * scale, -0.31, 0.22, 0.27]).unwrap();
        for relation in [
            CoordinateRelation::Revolute,
            CoordinateRelation::Prismatic,
            CoordinateRelation::Cylindrical,
        ] {
            for transform in [None, Some(common_left)] {
                let (hinge_driver, translation_driver) = match relation {
                    CoordinateRelation::Revolute => (true, false),
                    CoordinateRelation::Prismatic => (false, true),
                    CoordinateRelation::Cylindrical => (true, true),
                };
                let phase = if matches!(relation, CoordinateRelation::Prismatic) {
                    0.0
                } else {
                    -0.81
                };
                let fixture = coordinate_fixture(
                    relation,
                    scale,
                    false,
                    SpatialAxisParity::Opposed,
                    phase,
                    -2.4 * scale,
                    -5,
                    hinge_driver,
                    translation_driver,
                    true,
                    transform,
                );
                let hinge = fixture.hinge;
                let translation = fixture.translation;
                let jacobians = fixture
                    .assembly
                    .compile()
                    .unwrap()
                    .check_jacobians(1.0e-6)
                    .unwrap_or_else(|error| panic!("{relation:?}, scale={scale:e}: {error:#?}"));
                assert!(
                    jacobians.max_relative_error() <= 1.0e-6,
                    "{relation:?}, scale={scale:e}, transformed={}: {jacobians:#?}",
                    transform.is_some()
                );
                assert!(
                    jacobians.max_absolute_error() <= 1.0e-6,
                    "{relation:?}, scale={scale:e}, transformed={}: {jacobians:#?}",
                    transform.is_some()
                );
                let session =
                    SpatialAssemblySession::new(fixture.assembly, SolverConfig::default())
                        .unwrap_or_else(|error| {
                            panic!("{relation:?}, scale={scale:e}: {error:#?}")
                        });
                assert!(session.accepted_result().acceptance_hard_residual_max <= 1.0e-9);
                if let Some(hinge) = hinge {
                    assert_hinge_value(&session, hinge, phase, -5);
                }
                if let Some(translation) = translation {
                    assert_translation_value(&session, translation, -2.4 * scale, 2.0e-9 * scale);
                }
            }
        }
    }
}

#[test]
fn shaft_bearing_driver_stage_matrix_reports_internal_two_one_one_zero() {
    for (hinge_driver, translation_driver, expected_internal, expected_rank) in [
        (false, false, 2, 4),
        (true, false, 1, 5),
        (false, true, 1, 5),
        (true, true, 0, 6),
    ] {
        let fixture = coordinate_fixture(
            CoordinateRelation::Cylindrical,
            1.0,
            true,
            SpatialAxisParity::Aligned,
            0.48,
            1.9,
            2,
            hinge_driver,
            translation_driver,
            false,
            None,
        );
        let session =
            SpatialAssemblySession::new(fixture.assembly, SolverConfig::default()).unwrap();
        assert_eq!(session.gauge_report().internal_mobility, expected_internal);
        assert_eq!(session.accepted_result().core_report.rank, expected_rank);
    }
}

#[test]
fn two_target_principal_cut_transaction_commits_once_and_updates_winding_atomically() {
    let fixture = coordinate_fixture(
        CoordinateRelation::Cylindrical,
        1.0,
        true,
        SpatialAxisParity::Aligned,
        PI - 0.04,
        0.6,
        5,
        true,
        true,
        false,
        None,
    );
    let hinge = fixture.hinge.unwrap();
    let translation = fixture.translation.unwrap();
    let hinge_driver = fixture.hinge_driver.unwrap();
    let translation_driver = fixture.translation_driver.unwrap();
    let mut session =
        SpatialAssemblySession::new(fixture.assembly, SolverConfig::default()).unwrap();
    let revision = session.revision();
    session
        .apply_transaction(SpatialAssemblyTransaction::new(
            revision,
            vec![
                SpatialAssemblyEdit::HingeWinding {
                    coordinate: hinge,
                    winding: 6,
                },
                SpatialAssemblyEdit::HingeDriverTarget {
                    source: hinge_driver,
                    target: SpatialHingeTarget {
                        principal_phase: -PI + 0.04,
                        winding: 6,
                    },
                },
                SpatialAssemblyEdit::TranslationDriverTarget {
                    source: translation_driver,
                    target: -0.7,
                },
            ],
        ))
        .unwrap();
    assert_eq!(session.revision(), revision + 1);
    assert_hinge_value(&session, hinge, -PI + 0.04, 6);
    assert_translation_value(&session, translation, -0.7, 2.0e-9);
    assert!(matches!(
        session.assembly().coordinate(hinge).unwrap().kind(),
        SpatialCoordinateKind::Hinge { winding: 6, .. }
    ));
    assert!(matches!(
        session.assembly().source(hinge_driver).unwrap().kind(),
        SpatialSourceKind::HingePositionDriver {
            target: SpatialHingeTarget { winding: 6, .. },
            ..
        }
    ));
}

#[test]
#[allow(clippy::too_many_lines)]
fn transaction_prevalidation_rejects_duplicate_stale_wrong_kind_nonfinite_and_winding_mismatch() {
    let fixture = coordinate_fixture(
        CoordinateRelation::Cylindrical,
        1.0,
        true,
        SpatialAxisParity::Aligned,
        0.35,
        1.1,
        2,
        true,
        true,
        false,
        None,
    );
    let hinge = fixture.hinge.unwrap();
    let translation = fixture.translation.unwrap();
    let hinge_driver = fixture.hinge_driver.unwrap();
    let translation_driver = fixture.translation_driver.unwrap();
    let parent = fixture.parent;
    let mut session =
        SpatialAssemblySession::new(fixture.assembly, SolverConfig::default()).unwrap();
    let revision = session.revision();

    let duplicate = SpatialAssemblyTransaction::new(
        revision,
        vec![
            SpatialAssemblyEdit::TranslationDriverTarget {
                source: translation_driver,
                target: 1.2,
            },
            SpatialAssemblyEdit::TranslationDriverTarget {
                source: translation_driver,
                target: 1.3,
            },
        ],
    );
    assert!(matches!(
        rejected_transaction_retains(&mut session, duplicate),
        SpatialAssemblyError::DuplicateEdit { .. }
    ));
    assert!(matches!(
        rejected_transaction_retains(
            &mut session,
            SpatialAssemblyTransaction::one(
                revision + 1,
                SpatialAssemblyEdit::SourceAxisParity {
                    source: parent,
                    parity: SpatialAxisParity::Opposed,
                },
            ),
        ),
        SpatialAssemblyError::StaleRevision { .. }
    ));
    assert!(matches!(
        rejected_transaction_retains(
            &mut session,
            SpatialAssemblyTransaction::one(
                revision,
                SpatialAssemblyEdit::HingeWinding {
                    coordinate: translation,
                    winding: 2,
                },
            ),
        ),
        SpatialAssemblyError::WrongCoordinateKind { coordinate, .. }
            if coordinate == translation
    ));
    assert!(matches!(
        rejected_transaction_retains(
            &mut session,
            SpatialAssemblyTransaction::one(
                revision,
                SpatialAssemblyEdit::HingeDriverTarget {
                    source: translation_driver,
                    target: SpatialHingeTarget {
                        principal_phase: 0.35,
                        winding: 2,
                    },
                },
            ),
        ),
        SpatialAssemblyError::WrongSourceKind { source_id, .. }
            if source_id == translation_driver
    ));
    assert!(matches!(
        rejected_transaction_retains(
            &mut session,
            SpatialAssemblyTransaction::one(
                revision,
                SpatialAssemblyEdit::TranslationDriverTarget {
                    source: translation_driver,
                    target: f64::NAN,
                },
            ),
        ),
        SpatialAssemblyError::InvalidField { field, .. }
            if field == "translation_position_driver.target"
    ));
    assert!(matches!(
        rejected_transaction_retains(
            &mut session,
            SpatialAssemblyTransaction::one(
                revision,
                SpatialAssemblyEdit::HingeDriverTarget {
                    source: hinge_driver,
                    target: SpatialHingeTarget {
                        principal_phase: 0.35,
                        winding: 3,
                    },
                },
            ),
        ),
        SpatialAssemblyError::WindingMismatch { coordinate, .. } if coordinate == hinge
    ));
}

#[test]
fn incompatible_duplicate_target_and_branch_invalid_parity_transactions_retain_every_view() {
    let mut duplicate = coordinate_fixture(
        CoordinateRelation::Cylindrical,
        1.0,
        true,
        SpatialAxisParity::Aligned,
        0.52,
        0.9,
        -1,
        true,
        false,
        false,
        None,
    );
    let hinge = duplicate.hinge.unwrap();
    let second_driver = duplicate
        .assembly
        .add_hinge_position_driver(
            "second equal hinge driver",
            hinge,
            SpatialHingeTarget {
                principal_phase: 0.52,
                winding: -1,
            },
        )
        .unwrap();
    let mut duplicate =
        SpatialAssemblySession::new(duplicate.assembly, SolverConfig::default()).unwrap();
    let revision = duplicate.revision();
    assert!(matches!(
        rejected_transaction_retains(
            &mut duplicate,
            SpatialAssemblyTransaction::one(
                revision,
                SpatialAssemblyEdit::HingeDriverTarget {
                    source: second_driver,
                    target: SpatialHingeTarget {
                        principal_phase: 0.61,
                        winding: -1,
                    },
                },
            ),
        ),
        SpatialAssemblyError::IncompatibleDriverTargets { coordinate } if coordinate == hinge
    ));

    let mut branch = coordinate_fixture(
        CoordinateRelation::Cylindrical,
        1.0,
        true,
        SpatialAxisParity::Aligned,
        -0.44,
        1.3,
        0,
        true,
        true,
        false,
        None,
    );
    branch
        .assembly
        .add_physical_ground("lock second body", branch.second_body)
        .unwrap();
    let parent = branch.parent;
    let mut branch = SpatialAssemblySession::new(branch.assembly, SolverConfig::default()).unwrap();
    let revision = branch.revision();
    assert!(matches!(
        rejected_transaction_retains(
            &mut branch,
            SpatialAssemblyTransaction::one(
                revision,
                SpatialAssemblyEdit::SourceAxisParity {
                    source: parent,
                    parity: SpatialAxisParity::Opposed,
                },
            ),
        ),
        SpatialAssemblyError::IndependentValidation(message)
            if message.contains("parity")
    ));
}

#[test]
fn loose_solver_tolerance_cannot_weaken_position_driver_acceptance() {
    let mut fixture = coordinate_fixture(
        CoordinateRelation::Cylindrical,
        1.0,
        true,
        SpatialAxisParity::Aligned,
        0.2,
        0.7,
        0,
        true,
        true,
        false,
        None,
    );
    fixture
        .assembly
        .add_physical_ground("lock second body", fixture.second_body)
        .unwrap();
    let hinge_driver = fixture.hinge_driver.unwrap();
    let config = SolverConfig {
        normalized_residual_tolerance: 1.0e-2,
        ..SolverConfig::default()
    };
    let mut session = SpatialAssemblySession::new(fixture.assembly, config).unwrap();
    let revision = session.revision();
    assert!(matches!(
        rejected_transaction_retains(
            &mut session,
            SpatialAssemblyTransaction::one(
                revision,
                SpatialAssemblyEdit::HingeDriverTarget {
                    source: hinge_driver,
                    target: SpatialHingeTarget {
                        principal_phase: 0.2 + 1.0e-5,
                        winding: 0,
                    },
                },
            ),
        ),
        SpatialAssemblyError::IndependentValidation(message)
            if message.contains("1e-9") || message.contains("exceeds")
    ));
}

#[allow(
    clippy::fn_params_excessive_bools,
    clippy::too_many_arguments,
    clippy::too_many_lines
)]
fn coordinate_fixture(
    relation: CoordinateRelation,
    scale: f64,
    grounded: bool,
    parity: SpatialAxisParity,
    phase: f64,
    translation: f64,
    winding: i64,
    hinge_driver: bool,
    translation_driver: bool,
    perturb_second: bool,
    common_left: Option<Pose3>,
) -> CoordinateFixture {
    let mut first_pose =
        Pose3::exp([8.0 * scale, -1.7 * scale, 0.8 * scale, 0.27, -0.19, 0.23]).unwrap();
    let mut second_pose =
        Pose3::exp([-0.9 * scale, 0.6 * scale, -0.35 * scale, -0.22, 0.31, 0.18]).unwrap();
    let feature_pose =
        Pose3::exp([1.4 * scale, -0.75 * scale, 0.55 * scale, 0.38, -0.29, 0.21]).unwrap();
    let mut first_world = frame_from_pose(feature_pose);
    let measured_phase = if matches!(relation, CoordinateRelation::Prismatic) {
        0.0
    } else {
        phase
    };
    let axial_translation = if matches!(relation, CoordinateRelation::Revolute) {
        0.0
    } else {
        translation
    };
    let mut second_world =
        coordinate_second_frame(first_world, parity, measured_phase, axial_translation);
    if let Some(transform) = common_left {
        first_pose = transform.compose(&first_pose).unwrap();
        second_pose = transform.compose(&second_pose).unwrap();
        first_world = transform_frame(transform, first_world);
        second_world = transform_frame(transform, second_world);
    }
    let second_guess = if perturb_second {
        second_pose
            .retract([
                0.07 * scale,
                -0.05 * scale,
                0.04 * scale,
                0.045,
                -0.035,
                0.04,
            ])
            .unwrap()
    } else {
        second_pose
    };
    let mut assembly = SpatialAssembly::new(scale).unwrap();
    let first_body = assembly.add_body("bearing", first_pose).unwrap();
    let second_body = assembly.add_body("shaft", second_guess).unwrap();
    if grounded {
        assembly
            .add_physical_ground("bearing ground", first_body)
            .unwrap();
    }
    let first_local = local_frame(first_pose, first_world);
    let second_local = local_frame(second_pose, second_world);
    let parent = match relation {
        CoordinateRelation::Revolute => {
            let first = assembly
                .add_frame_feature("bearing hinge frame", first_body, first_local)
                .unwrap();
            let second = assembly
                .add_frame_feature("shaft hinge frame", second_body, second_local)
                .unwrap();
            assembly
                .add_revolute_joint("ordered revolute", first, second, parity)
                .unwrap()
        }
        CoordinateRelation::Prismatic | CoordinateRelation::Cylindrical => {
            let first = assembly
                .add_axis_feature("bearing clocked axis", first_body, first_local)
                .unwrap();
            let second = assembly
                .add_axis_feature("shaft clocked axis", second_body, second_local)
                .unwrap();
            if matches!(relation, CoordinateRelation::Prismatic) {
                assembly
                    .add_prismatic_joint("ordered prismatic", first, second, parity)
                    .unwrap()
            } else {
                assembly
                    .add_cylindrical_joint("ordered cylindrical", first, second, parity)
                    .unwrap()
            }
        }
    };
    let hinge = matches!(
        relation,
        CoordinateRelation::Revolute | CoordinateRelation::Cylindrical
    )
    .then(|| {
        assembly
            .add_hinge_coordinate("retained hinge coordinate", parent, winding)
            .unwrap()
    });
    let translation_coordinate = matches!(
        relation,
        CoordinateRelation::Prismatic | CoordinateRelation::Cylindrical
    )
    .then(|| {
        assembly
            .add_axial_translation_coordinate("retained translation coordinate", parent)
            .unwrap()
    });
    let hinge_driver_id = hinge_driver.then(|| {
        assembly
            .add_hinge_position_driver(
                "hinge position driver",
                hinge.unwrap(),
                SpatialHingeTarget {
                    principal_phase: measured_phase,
                    winding,
                },
            )
            .unwrap()
    });
    let translation_driver_id = translation_driver.then(|| {
        assembly
            .add_translation_position_driver(
                "translation position driver",
                translation_coordinate.unwrap(),
                axial_translation,
            )
            .unwrap()
    });
    CoordinateFixture {
        assembly,
        parent,
        hinge,
        translation: translation_coordinate,
        hinge_driver: hinge_driver_id,
        translation_driver: translation_driver_id,
        second_body,
    }
}

fn coordinate_second_frame(
    first: Frame3,
    parity: SpatialAxisParity,
    phase: f64,
    translation: f64,
) -> Frame3 {
    let (sine, cosine) = phase.sin_cos();
    let x = first.x_axis() * cosine + first.y_axis() * sine;
    let z = first.z_axis() * parity.multiplier();
    let y = z.cross(&x);
    Frame3::try_new(first.origin() + first.z_axis() * translation, x, y, z).unwrap()
}

fn assert_hinge_value(
    session: &SpatialAssemblySession,
    coordinate: SpatialCoordinateId,
    expected_phase: f64,
    expected_winding: i64,
) {
    let value = session.coordinate_value(coordinate).unwrap();
    let SpatialCoordinateValueKind::Hinge(value) = value.value else {
        panic!("coordinate {coordinate} is not a hinge value");
    };
    assert_eq!(value.winding, expected_winding);
    let error = (value.principal_phase - expected_phase + PI).rem_euclid(2.0 * PI) - PI;
    assert!(
        error.abs() <= 2.0e-9,
        "hinge phase error {error:e}: actual={}, expected={expected_phase}",
        value.principal_phase
    );
    assert!((-PI..PI).contains(&value.principal_phase));
}

fn assert_translation_value(
    session: &SpatialAssemblySession,
    coordinate: SpatialCoordinateId,
    expected: f64,
    tolerance: f64,
) {
    let value = session.coordinate_value(coordinate).unwrap();
    let SpatialCoordinateValueKind::AxialTranslation(value) = value.value else {
        panic!("coordinate {coordinate} is not a translation value");
    };
    assert!(
        (value - expected).abs() <= tolerance.max(2.0e-15),
        "translation error {:e} exceeds {:e}: actual={value}, expected={expected}",
        (value - expected).abs(),
        tolerance
    );
}

fn rejected_transaction_retains(
    session: &mut SpatialAssemblySession,
    transaction: SpatialAssemblyTransaction,
) -> SpatialAssemblyError {
    let revision = session.revision();
    let assembly = session.assembly().clone();
    let result = session.accepted_result().clone();
    let mappings = session.source_mappings().to_vec();
    let gauge = session.gauge_report().clone();
    let report = session.core_session().report().clone();
    let linearization = session
        .core_session()
        .accepted_hard_linearization()
        .unwrap();
    let error = session.apply_transaction(transaction).unwrap_err();
    assert_eq!(session.revision(), revision);
    assert_eq!(session.assembly(), &assembly);
    assert_eq!(session.accepted_result(), &result);
    assert_eq!(session.source_mappings(), mappings);
    assert_eq!(session.gauge_report(), &gauge);
    assert_eq!(session.core_session().report(), &report);
    assert_eq!(
        session
            .core_session()
            .accepted_hard_linearization()
            .unwrap(),
        linearization
    );
    error
}

fn foreign_source_id() -> SpatialSourceId {
    let mut assembly = SpatialAssembly::new(1.0).unwrap();
    for index in 0..30 {
        assembly
            .add_body(format!("foreign source body {index}"), Pose3::identity())
            .unwrap();
    }
    let body = assembly.bodies()[0].id();
    assembly
        .add_physical_ground("foreign source", body)
        .unwrap()
}

struct MonitorCatalogFixture {
    assembly: SpatialAssembly,
    monitors: [SpatialModeMonitorId; 8],
    editable_axis: SpatialAxisFeatureId,
    editable_plane_point: SpatialPointFeatureId,
    editable_volume_point: SpatialPointFeatureId,
    hinge_coordinates: [SpatialCoordinateId; 2],
}

#[test]
#[allow(clippy::float_cmp, clippy::too_many_lines)]
fn explicit_mode_monitor_catalog_is_deterministic_finite_and_row_free() {
    let fixture = monitor_catalog_fixture(1.0, None);
    assert!(
        fixture
            .monitors
            .windows(2)
            .all(|pair| pair[0].as_u64() < pair[1].as_u64())
    );
    assert_eq!(
        fixture
            .assembly
            .mode_monitors()
            .iter()
            .map(geosolve_linkage::SpatialModeMonitor::id)
            .collect::<Vec<_>>(),
        fixture.monitors
    );
    assert_eq!(
        fixture
            .assembly
            .mode_monitor(fixture.monitors[0])
            .unwrap()
            .label(),
        "aligned axis parity"
    );

    let repeated = fixture.assembly.clone();
    let session = SpatialAssemblySession::new(fixture.assembly, SolverConfig::default()).unwrap();
    let second = SpatialAssemblySession::new(repeated, SolverConfig::default()).unwrap();
    assert_eq!(session.mode_evaluations(), second.mode_evaluations());
    assert_eq!(session.mode_evaluations().len(), fixture.monitors.len());
    assert_eq!(
        session
            .mode_evaluations()
            .iter()
            .map(|evaluation| evaluation.monitor_id)
            .collect::<Vec<_>>(),
        fixture.monitors
    );
    assert!(session.mode_evaluations().iter().all(|evaluation| {
        evaluation.retained
            && evaluation.retained_normalized_metric.is_finite()
            && evaluation.retained_normalized_metric > 1.0e-3
            && evaluation.fresh_raw_metric.is_none_or(f64::is_finite)
            && !evaluation.involved_bodies.is_empty()
            && !evaluation.involved_features.is_empty()
    }));

    let axis_positive = session.mode_evaluation(fixture.monitors[0]).unwrap();
    let axis_negative = session.mode_evaluation(fixture.monitors[1]).unwrap();
    assert_eq!(axis_positive.fresh_raw_metric, Some(1.0));
    assert_eq!(axis_negative.fresh_raw_metric, Some(-1.0));
    assert_eq!(axis_positive.retained_normalized_metric, 1.0);
    assert_eq!(axis_negative.retained_normalized_metric, 1.0);
    assert!(matches!(
        axis_negative.kind,
        SpatialModeMonitorKind::AxisParity {
            parity: SpatialAxisParity::Opposed,
            ..
        }
    ));

    for (index, winding, coordinate) in [
        (2, 4, fixture.hinge_coordinates[0]),
        (3, -7, fixture.hinge_coordinates[1]),
    ] {
        let evaluation = session.mode_evaluation(fixture.monitors[index]).unwrap();
        assert_eq!(evaluation.coordinate, Some(coordinate));
        assert_eq!(evaluation.winding, Some(winding));
        assert!(evaluation.fresh_raw_metric.unwrap().is_finite());
        assert!(evaluation.retained_normalized_metric > 0.999);
    }
    assert_eq!(
        session
            .mode_evaluation(fixture.monitors[4])
            .unwrap()
            .fresh_raw_metric,
        Some(2.0)
    );
    assert_eq!(
        session
            .mode_evaluation(fixture.monitors[5])
            .unwrap()
            .fresh_raw_metric,
        Some(-2.0)
    );
    assert_eq!(
        session
            .mode_evaluation(fixture.monitors[6])
            .unwrap()
            .fresh_raw_metric,
        Some(1.0)
    );
    assert_eq!(
        session
            .mode_evaluation(fixture.monitors[7])
            .unwrap()
            .fresh_raw_metric,
        Some(-1.0)
    );

    assert_eq!(
        session.source_mappings().len(),
        session.assembly().sources().len()
    );
    assert_eq!(
        session.accepted_result().display_audit.sources.len(),
        session.assembly().sources().len()
    );
    assert!(
        session
            .accepted_result()
            .display_audit
            .sources
            .iter()
            .all(|source| !session
                .assembly()
                .mode_monitors()
                .iter()
                .any(|monitor| monitor.label() == source.source_label))
    );
    assert!(
        session
            .accepted_result()
            .display_audit
            .sources
            .iter()
            .flat_map(|source| &source.rows)
            .all(
                |row| row.evaluation_status == AuditEvaluationStatus::Evaluated
                    && row.raw_residual.is_finite()
                    && row.normalized_residual.is_finite()
                    && row.scale.is_finite()
                    && row.scale > 0.0
            )
    );
}

#[test]
fn monitor_only_connectivity_separates_domain_gauge_from_physical_core_components() {
    let identity = identity_frame(Point3::origin());
    let mut assembly = SpatialAssembly::new(1.0).unwrap();
    let first_body = assembly.add_body("first free", Pose3::identity()).unwrap();
    let second_body = assembly.add_body("second free", Pose3::identity()).unwrap();
    let first = assembly
        .add_axis_feature("first mode axis", first_body, identity)
        .unwrap();
    let second = assembly
        .add_axis_feature("second mode axis", second_body, identity)
        .unwrap();
    let monitor = assembly
        .add_axis_parity_monitor(
            "only relationship",
            first,
            second,
            SpatialAxisParity::Aligned,
        )
        .unwrap();
    let session = SpatialAssemblySession::new(assembly, SolverConfig::default()).unwrap();

    assert!(session.source_mappings().is_empty());
    assert!(session.accepted_result().display_audit.sources.is_empty());
    assert_eq!(
        session.accepted_result().core_report.structural.scalar_rows,
        0
    );
    assert_eq!(session.accepted_result().core_report.right_nullity, 12);
    assert_eq!(session.gauge_report().numerical_equality_right_nullity, 12);
    assert_eq!(session.gauge_report().gauge_dof, 6);
    assert_eq!(session.gauge_report().internal_mobility, 6);
    assert_eq!(session.gauge_report().components.len(), 1);
    assert_eq!(
        session.gauge_report().components[0].bodies,
        vec![first_body, second_body]
    );
    assert!(session.gauge_report().components[0].sources.is_empty());
    assert_eq!(
        session.gauge_report().components[0].mode_monitors,
        vec![monitor]
    );
    assert_eq!(
        session.gauge_report().components[0]
            .core_component_indices
            .len(),
        2
    );
}

#[test]
fn common_left_se3_and_required_scales_preserve_all_mode_metrics_and_winding() {
    let mut reference: Option<Vec<(f64, Option<i64>)>> = None;
    for scale in [1.0e-6, 1.0, 1.0e6] {
        let common_left =
            Pose3::exp([1.3 * scale, -0.9 * scale, 0.7 * scale, -0.31, 0.24, 0.28]).unwrap();
        let base = monitor_catalog_fixture(scale, None);
        let moved = monitor_catalog_fixture(scale, Some(common_left));
        let base = SpatialAssemblySession::new(base.assembly, SolverConfig::default()).unwrap();
        let moved = SpatialAssemblySession::new(moved.assembly, SolverConfig::default()).unwrap();
        let base_metrics = base
            .mode_evaluations()
            .iter()
            .map(|evaluation| (evaluation.retained_normalized_metric, evaluation.winding))
            .collect::<Vec<_>>();
        let moved_metrics = moved
            .mode_evaluations()
            .iter()
            .map(|evaluation| (evaluation.retained_normalized_metric, evaluation.winding))
            .collect::<Vec<_>>();
        for (base, moved) in base_metrics.iter().zip(&moved_metrics) {
            assert!((base.0 - moved.0).abs() <= 2.0e-9, "scale={scale:e}");
            assert_eq!(base.1, moved.1);
        }
        if let Some(reference) = &reference {
            for (expected, actual) in reference.iter().zip(&base_metrics) {
                assert!((expected.0 - actual.0).abs() <= 2.0e-9, "scale={scale:e}");
                assert_eq!(expected.1, actual.1);
            }
        } else {
            reference = Some(base_metrics);
        }
    }
}

#[test]
#[allow(clippy::too_many_lines)]
fn mode_boundaries_and_degenerate_signed_volumes_never_accept() {
    let identity = identity_frame(Point3::origin());
    let perpendicular =
        Frame3::try_new(Point3::origin(), Vector3::y(), Vector3::z(), Vector3::x()).unwrap();
    let mut parity = SpatialAssembly::new(1.0).unwrap();
    let first_body = parity.add_body("first", Pose3::identity()).unwrap();
    let second_body = parity.add_body("second", Pose3::identity()).unwrap();
    let first = parity
        .add_axis_feature("first", first_body, identity)
        .unwrap();
    let second = parity
        .add_axis_feature("perpendicular", second_body, perpendicular)
        .unwrap();
    parity
        .add_physical_ground("first ground", first_body)
        .unwrap();
    parity
        .add_physical_ground("second ground", second_body)
        .unwrap();
    parity
        .add_axis_parity_monitor(
            "perpendicular parity",
            first,
            second,
            SpatialAxisParity::Aligned,
        )
        .unwrap();
    assert!(matches!(
        SpatialAssemblySession::new(parity, SolverConfig::default()),
        Err(SpatialAssemblyError::IndependentValidation(message))
            if message.contains("axis parity")
    ));

    let mut side = SpatialAssembly::new(1.0).unwrap();
    let body = side.add_body("body", Pose3::identity()).unwrap();
    let plane = side.add_plane_feature("plane", body, identity).unwrap();
    let point = side
        .add_point_feature("point on plane", body, Point3::origin())
        .unwrap();
    side.add_physical_ground("ground", body).unwrap();
    side.add_plane_side_monitor("zero side", plane, point, SpatialModeSign::Positive)
        .unwrap();
    assert!(SpatialAssemblySession::new(side, SolverConfig::default()).is_err());

    let mut repeated = SpatialAssembly::new(1.0).unwrap();
    let body = repeated.add_body("body", Pose3::identity()).unwrap();
    let a = repeated
        .add_point_feature("A", body, Point3::origin())
        .unwrap();
    let b = repeated
        .add_point_feature("B", body, Point3::new(1.0, 0.0, 0.0))
        .unwrap();
    let c = repeated
        .add_point_feature("C", body, Point3::new(0.0, 1.0, 0.0))
        .unwrap();
    assert!(matches!(
        repeated.add_signed_volume_monitor("repeated", [a, b, c, a], SpatialModeSign::Positive),
        Err(SpatialAssemblyError::InvalidField { field, .. })
            if field == "signed_volume_monitor.points"
    ));

    for (label, points) in [
        (
            "collapsed",
            [
                Point3::origin(),
                Point3::origin(),
                Point3::new(0.0, 1.0, 0.0),
                Point3::new(0.0, 0.0, 1.0),
            ],
        ),
        (
            "collinear",
            [
                Point3::origin(),
                Point3::new(1.0, 0.0, 0.0),
                Point3::new(2.0, 0.0, 0.0),
                Point3::new(0.0, 0.0, 1.0),
            ],
        ),
        (
            "coplanar",
            [
                Point3::origin(),
                Point3::new(1.0, 0.0, 0.0),
                Point3::new(0.0, 1.0, 0.0),
                Point3::new(1.0, 1.0, 0.0),
            ],
        ),
    ] {
        assert_degenerate_volume_rejected(label, points);
    }

    let mut undefined = SpatialAssembly::new(1.0).unwrap();
    let first_body = undefined.add_body("first", Pose3::identity()).unwrap();
    let second_body = undefined.add_body("second", Pose3::identity()).unwrap();
    let first = undefined
        .add_axis_feature("first", first_body, identity)
        .unwrap();
    let second = undefined
        .add_axis_feature("undefined clock", second_body, perpendicular)
        .unwrap();
    undefined
        .add_physical_ground("first ground", first_body)
        .unwrap();
    undefined
        .add_physical_ground("second ground", second_body)
        .unwrap();
    let parent = undefined
        .add_cylindrical_joint(
            "invalid parent root",
            first,
            second,
            SpatialAxisParity::Aligned,
        )
        .unwrap();
    let coordinate = undefined.add_hinge_coordinate("hinge", parent, 0).unwrap();
    undefined
        .add_hinge_winding_monitor("undefined hinge clock", coordinate, 0)
        .unwrap();
    assert!(SpatialAssemblySession::new(undefined, SolverConfig::default()).is_err());
}

#[test]
fn reflected_distance_root_and_mirrored_universal_root_reject_transactionally() {
    let identity = identity_frame(Point3::origin());
    let mut side = SpatialAssembly::new(1.0).unwrap();
    let base = side.add_body("base", Pose3::identity()).unwrap();
    let observed = side
        .add_body(
            "observed",
            Pose3::exp([0.0, 0.0, 1.0, 0.0, 0.0, 0.0]).unwrap(),
        )
        .unwrap();
    let plane = side
        .add_plane_feature("base plane", base, identity)
        .unwrap();
    let base_point = side
        .add_point_feature("base point", base, Point3::origin())
        .unwrap();
    let observed_point = side
        .add_point_feature("observed point", observed, Point3::origin())
        .unwrap();
    side.add_physical_ground("base ground", base).unwrap();
    side.add_point_distance_mate("unit distance", base_point, observed_point, 1.0)
        .unwrap();
    side.add_plane_side_monitor(
        "positive distance root",
        plane,
        observed_point,
        SpatialModeSign::Positive,
    )
    .unwrap();
    let mut side = SpatialAssemblySession::new(side, SolverConfig::default()).unwrap();
    let revision = side.revision();
    let error = rejected_transaction_retains(
        &mut side,
        SpatialAssemblyTransaction::one(
            revision,
            SpatialAssemblyEdit::BodyPoseGuess {
                body: observed,
                pose: Pose3::exp([0.0, 0.0, -1.0, 0.0, 0.0, 0.0]).unwrap(),
            },
        ),
    );
    assert!(matches!(
        error,
        SpatialAssemblyError::IndependentValidation(message) if message.contains("plane side")
    ));

    let universal_axis =
        Frame3::try_new(Point3::origin(), Vector3::y(), Vector3::z(), Vector3::x()).unwrap();
    let mut volume = SpatialAssembly::new(1.0).unwrap();
    let base = volume.add_body("base", Pose3::identity()).unwrap();
    let moving = volume.add_body("moving", Pose3::identity()).unwrap();
    let first_axis = volume
        .add_axis_feature("base universal axis", base, identity)
        .unwrap();
    let second_axis = volume
        .add_axis_feature("moving universal axis", moving, universal_axis)
        .unwrap();
    let a = volume
        .add_point_feature("A", base, Point3::origin())
        .unwrap();
    let b = volume
        .add_point_feature("B", base, Point3::new(1.0, 0.0, 0.0))
        .unwrap();
    let c = volume
        .add_point_feature("C", base, Point3::new(0.0, 1.0, 0.0))
        .unwrap();
    let d = volume
        .add_point_feature("D", moving, Point3::new(0.0, 0.0, 1.0))
        .unwrap();
    volume.add_physical_ground("base ground", base).unwrap();
    volume
        .add_universal_joint("closed universal", first_axis, second_axis)
        .unwrap();
    volume
        .add_signed_volume_monitor(
            "positive closed orientation",
            [a, b, c, d],
            SpatialModeSign::Positive,
        )
        .unwrap();
    let mut volume = SpatialAssemblySession::new(volume, SolverConfig::default()).unwrap();
    let revision = volume.revision();
    let error = rejected_transaction_retains(
        &mut volume,
        SpatialAssemblyTransaction::one(
            revision,
            SpatialAssemblyEdit::BodyPoseGuess {
                body: moving,
                pose: Pose3::exp([0.0, 0.0, 0.0, PI, 0.0, 0.0]).unwrap(),
            },
        ),
    );
    assert!(matches!(
        error,
        SpatialAssemblyError::IndependentValidation(message) if message.contains("signed volume")
    ));
}

#[test]
#[allow(clippy::too_many_lines)]
fn atomic_hinge_winding_transition_updates_coordinate_driver_and_monitor_once() {
    let mut fixture = coordinate_fixture(
        CoordinateRelation::Cylindrical,
        1.0,
        true,
        SpatialAxisParity::Aligned,
        PI - 0.03,
        0.5,
        8,
        true,
        false,
        false,
        None,
    );
    let coordinate = fixture.hinge.unwrap();
    let driver = fixture.hinge_driver.unwrap();
    let monitor = fixture
        .assembly
        .add_hinge_winding_monitor("explicit winding", coordinate, 8)
        .unwrap();
    let mut session =
        SpatialAssemblySession::new(fixture.assembly, SolverConfig::default()).unwrap();
    let revision = session.revision();

    for edits in [
        vec![SpatialAssemblyEdit::HingeWinding {
            coordinate,
            winding: 9,
        }],
        vec![
            SpatialAssemblyEdit::HingeWinding {
                coordinate,
                winding: 9,
            },
            SpatialAssemblyEdit::HingeDriverTarget {
                source: driver,
                target: SpatialHingeTarget {
                    principal_phase: -PI + 0.03,
                    winding: 9,
                },
            },
        ],
        vec![SpatialAssemblyEdit::MonitorHingeWinding {
            monitor,
            winding: 9,
        }],
    ] {
        assert!(matches!(
            rejected_transaction_retains(
                &mut session,
                SpatialAssemblyTransaction::new(revision, edits),
            ),
            SpatialAssemblyError::WindingMismatch { coordinate: id, .. } if id == coordinate
        ));
    }

    session
        .apply_transaction(SpatialAssemblyTransaction::new(
            revision,
            vec![
                SpatialAssemblyEdit::HingeWinding {
                    coordinate,
                    winding: 9,
                },
                SpatialAssemblyEdit::HingeDriverTarget {
                    source: driver,
                    target: SpatialHingeTarget {
                        principal_phase: -PI + 0.03,
                        winding: 9,
                    },
                },
                SpatialAssemblyEdit::MonitorHingeWinding {
                    monitor,
                    winding: 9,
                },
            ],
        ))
        .unwrap();
    assert_eq!(session.revision(), revision + 1);
    assert_hinge_value(&session, coordinate, -PI + 0.03, 9);
    assert_eq!(session.mode_evaluation(monitor).unwrap().winding, Some(9));
    assert!(matches!(
        session.assembly().mode_monitor(monitor).unwrap().kind(),
        SpatialModeMonitorKind::HingeWinding { winding: 9, .. }
    ));
    assert!(matches!(
        session.assembly().source(driver).unwrap().kind(),
        SpatialSourceKind::HingePositionDriver {
            target: SpatialHingeTarget { winding: 9, .. },
            ..
        }
    ));

    let revision = session.revision();
    assert!(matches!(
        rejected_transaction_retains(
            &mut session,
            SpatialAssemblyTransaction::one(
                revision,
                SpatialAssemblyEdit::HingeDriverTarget {
                    source: driver,
                    target: SpatialHingeTarget {
                        principal_phase: f64::NAN,
                        winding: 9,
                    },
                },
            ),
        ),
        SpatialAssemblyError::InvalidField { field, .. }
            if field == "hinge_position_driver.target"
    ));
}

#[test]
#[allow(clippy::too_many_lines)]
fn monitor_state_edits_commit_once_and_all_invalid_edits_retain_complete_views() {
    let fixture = monitor_catalog_fixture(1.0, None);
    let [axis, _, _, _, plane, _, volume, _] = fixture.monitors;
    let mut session =
        SpatialAssemblySession::new(fixture.assembly, SolverConfig::default()).unwrap();
    let revision = session.revision();
    session
        .apply_transaction(SpatialAssemblyTransaction::new(
            revision,
            vec![
                SpatialAssemblyEdit::AxisLocal {
                    feature: fixture.editable_axis,
                    local_frame: opposed_frame(identity_frame(Point3::origin())),
                },
                SpatialAssemblyEdit::MonitorAxisParity {
                    monitor: axis,
                    parity: SpatialAxisParity::Opposed,
                },
                SpatialAssemblyEdit::PointLocal {
                    feature: fixture.editable_plane_point,
                    local_point: Point3::new(0.0, 0.0, -2.0),
                },
                SpatialAssemblyEdit::MonitorPlaneSide {
                    monitor: plane,
                    side: SpatialModeSign::Negative,
                },
                SpatialAssemblyEdit::PointLocal {
                    feature: fixture.editable_volume_point,
                    local_point: Point3::new(0.0, 0.0, -1.0),
                },
                SpatialAssemblyEdit::MonitorSignedVolumeOrientation {
                    monitor: volume,
                    orientation: SpatialModeSign::Negative,
                },
            ],
        ))
        .unwrap();
    assert_eq!(session.revision(), revision + 1);
    assert!(session.mode_evaluation(axis).unwrap().retained);
    assert!(session.mode_evaluation(plane).unwrap().retained);
    assert!(session.mode_evaluation(volume).unwrap().retained);

    let revision = session.revision();
    let duplicate = SpatialAssemblyTransaction::new(
        revision,
        vec![
            SpatialAssemblyEdit::MonitorAxisParity {
                monitor: axis,
                parity: SpatialAxisParity::Aligned,
            },
            SpatialAssemblyEdit::MonitorPlaneSide {
                monitor: axis,
                side: SpatialModeSign::Positive,
            },
        ],
    );
    assert!(matches!(
        rejected_transaction_retains(&mut session, duplicate),
        SpatialAssemblyError::DuplicateEdit { .. }
    ));
    assert!(matches!(
        rejected_transaction_retains(
            &mut session,
            SpatialAssemblyTransaction::one(
                revision + 1,
                SpatialAssemblyEdit::MonitorAxisParity {
                    monitor: axis,
                    parity: SpatialAxisParity::Aligned,
                },
            ),
        ),
        SpatialAssemblyError::StaleRevision { .. }
    ));
    assert!(matches!(
        rejected_transaction_retains(
            &mut session,
            SpatialAssemblyTransaction::one(
                revision,
                SpatialAssemblyEdit::MonitorPlaneSide {
                    monitor: axis,
                    side: SpatialModeSign::Positive,
                },
            ),
        ),
        SpatialAssemblyError::WrongModeMonitorKind { monitor_id, .. } if monitor_id == axis
    ));
    let stale = foreign_monitor_id();
    assert!(matches!(
        rejected_transaction_retains(
            &mut session,
            SpatialAssemblyTransaction::one(
                revision,
                SpatialAssemblyEdit::MonitorAxisParity {
                    monitor: stale,
                    parity: SpatialAxisParity::Aligned,
                },
            ),
        ),
        SpatialAssemblyError::UnknownModeMonitor(id) if id == stale
    ));
    assert!(matches!(
        rejected_transaction_retains(
            &mut session,
            SpatialAssemblyTransaction::one(
                revision,
                SpatialAssemblyEdit::MonitorAxisParity {
                    monitor: axis,
                    parity: SpatialAxisParity::Aligned,
                },
            ),
        ),
        SpatialAssemblyError::IndependentValidation(message)
            if message.contains("axis parity")
    ));
}

#[allow(clippy::too_many_lines)]
fn monitor_catalog_fixture(scale: f64, common_left: Option<Pose3>) -> MonitorCatalogFixture {
    let pose = common_left.unwrap_or_else(Pose3::identity);
    let identity = identity_frame(Point3::origin());
    let mut assembly = SpatialAssembly::new(scale).unwrap();
    let first_body = assembly.add_body("mode body one", pose).unwrap();
    let second_body = assembly.add_body("mode body two", pose).unwrap();
    assembly
        .add_physical_ground("mode body one ground", first_body)
        .unwrap();
    assembly
        .add_physical_ground("mode body two ground", second_body)
        .unwrap();

    let first_axis = assembly
        .add_axis_feature("parity first", first_body, identity)
        .unwrap();
    let editable_axis = assembly
        .add_axis_feature("parity aligned second", second_body, identity)
        .unwrap();
    let opposed_axis = assembly
        .add_axis_feature(
            "parity opposed second",
            second_body,
            opposed_frame(identity),
        )
        .unwrap();
    let aligned_monitor = assembly
        .add_axis_parity_monitor(
            "aligned axis parity",
            first_axis,
            editable_axis,
            SpatialAxisParity::Aligned,
        )
        .unwrap();
    let opposed_monitor = assembly
        .add_axis_parity_monitor(
            "opposed axis parity",
            first_axis,
            opposed_axis,
            SpatialAxisParity::Opposed,
        )
        .unwrap();

    let hinge_first = assembly
        .add_axis_feature("hinge first", first_body, identity)
        .unwrap();
    let hinge_second = assembly
        .add_axis_feature(
            "hinge second",
            second_body,
            rotate_frame_about_z(identity, 0.4),
        )
        .unwrap();
    let hinge_parent = assembly
        .add_cylindrical_joint(
            "monitor cylindrical parent",
            hinge_first,
            hinge_second,
            SpatialAxisParity::Aligned,
        )
        .unwrap();
    let hinge_coordinate = assembly
        .add_hinge_coordinate("shared hinge coordinate", hinge_parent, 4)
        .unwrap();
    let positive_winding = assembly
        .add_hinge_winding_monitor("positive hinge winding", hinge_coordinate, 4)
        .unwrap();
    let negative_coordinate = assembly
        .add_hinge_coordinate("negative hinge coordinate", hinge_parent, -7)
        .unwrap();
    let negative_winding = assembly
        .add_hinge_winding_monitor("negative hinge winding", negative_coordinate, -7)
        .unwrap();

    let plane = assembly
        .add_plane_feature("side plane", first_body, identity)
        .unwrap();
    let editable_plane_point = assembly
        .add_point_feature(
            "positive side point",
            second_body,
            Point3::new(0.0, 0.0, 2.0 * scale),
        )
        .unwrap();
    let negative_point = assembly
        .add_point_feature(
            "negative side point",
            second_body,
            Point3::new(0.0, 0.0, -2.0 * scale),
        )
        .unwrap();
    let positive_side = assembly
        .add_plane_side_monitor(
            "positive plane side",
            plane,
            editable_plane_point,
            SpatialModeSign::Positive,
        )
        .unwrap();
    let negative_side = assembly
        .add_plane_side_monitor(
            "negative plane side",
            plane,
            negative_point,
            SpatialModeSign::Negative,
        )
        .unwrap();

    let a = assembly
        .add_point_feature("volume A", first_body, Point3::origin())
        .unwrap();
    let b = assembly
        .add_point_feature("volume B", first_body, Point3::new(scale, 0.0, 0.0))
        .unwrap();
    let c = assembly
        .add_point_feature("volume C", first_body, Point3::new(0.0, scale, 0.0))
        .unwrap();
    let editable_volume_point = assembly
        .add_point_feature(
            "volume positive D",
            first_body,
            Point3::new(0.0, 0.0, scale),
        )
        .unwrap();
    let negative_d = assembly
        .add_point_feature(
            "volume negative D",
            first_body,
            Point3::new(0.0, 0.0, -scale),
        )
        .unwrap();
    let positive_volume = assembly
        .add_signed_volume_monitor(
            "positive signed volume",
            [a, b, c, editable_volume_point],
            SpatialModeSign::Positive,
        )
        .unwrap();
    let negative_volume = assembly
        .add_signed_volume_monitor(
            "negative signed volume",
            [a, b, c, negative_d],
            SpatialModeSign::Negative,
        )
        .unwrap();
    MonitorCatalogFixture {
        assembly,
        monitors: [
            aligned_monitor,
            opposed_monitor,
            positive_winding,
            negative_winding,
            positive_side,
            negative_side,
            positive_volume,
            negative_volume,
        ],
        editable_axis,
        editable_plane_point,
        editable_volume_point,
        hinge_coordinates: [hinge_coordinate, negative_coordinate],
    }
}

fn assert_degenerate_volume_rejected(label: &str, points: [Point3<f64>; 4]) {
    let mut assembly = SpatialAssembly::new(1.0).unwrap();
    let body = assembly.add_body("body", Pose3::identity()).unwrap();
    let features = points.map(|point| {
        assembly
            .add_point_feature(format!("{label} point"), body, point)
            .unwrap()
    });
    assembly.add_physical_ground("ground", body).unwrap();
    assembly
        .add_signed_volume_monitor(label, features, SpatialModeSign::Positive)
        .unwrap();
    assert!(SpatialAssemblySession::new(assembly, SolverConfig::default()).is_err());
}

fn foreign_monitor_id() -> SpatialModeMonitorId {
    let mut assembly = SpatialAssembly::new(1.0).unwrap();
    for index in 0..40 {
        assembly
            .add_body(format!("foreign monitor body {index}"), Pose3::identity())
            .unwrap();
    }
    let first_body = assembly.bodies()[0].id();
    let second_body = assembly.bodies()[1].id();
    let first = assembly
        .add_axis_feature("first", first_body, identity_frame(Point3::origin()))
        .unwrap();
    let second = assembly
        .add_axis_feature("second", second_body, identity_frame(Point3::origin()))
        .unwrap();
    assembly
        .add_axis_parity_monitor("foreign", first, second, SpatialAxisParity::Aligned)
        .unwrap()
}

struct BlockBaseFixture {
    assembly: SpatialAssembly,
    parent: SpatialSourceId,
    coordinates: [SpatialCoordinateId; 3],
    drivers: Vec<SpatialSourceId>,
    side_monitor: SpatialModeMonitorId,
}

#[test]
fn block_base_planar_coordinates_report_three_two_one_zero_internal_mobility() {
    for driver_count in 0..=3 {
        let fixture = block_base_fixture(driver_count);
        let [hinge, x, y] = fixture.coordinates;
        assert!(matches!(
            fixture.assembly.coordinate(hinge).unwrap().kind(),
            SpatialCoordinateKind::Hinge {
                parent,
                winding: 3
            } if parent == fixture.parent
        ));
        assert!(matches!(
            fixture.assembly.coordinate(x).unwrap().kind(),
            SpatialCoordinateKind::PlanarTranslation {
                parent,
                axis: SpatialPlanarTranslationAxis::X,
            } if parent == fixture.parent
        ));
        assert!(matches!(
            fixture.assembly.coordinate(y).unwrap().kind(),
            SpatialCoordinateKind::PlanarTranslation {
                parent,
                axis: SpatialPlanarTranslationAxis::Y,
            } if parent == fixture.parent
        ));

        let jacobians = fixture
            .assembly
            .compile()
            .unwrap()
            .check_jacobians(1.0e-6)
            .unwrap();
        assert!(jacobians.max_relative_error() <= 1.0e-6);
        let session =
            SpatialAssemblySession::new(fixture.assembly, SolverConfig::default()).unwrap();
        assert_eq!(session.accepted_result().core_report.rank, 3 + driver_count);
        assert_eq!(session.accepted_result().core_report.left_nullity, 0);
        assert_eq!(session.gauge_report().gauge_dof, 0);
        assert_eq!(session.gauge_report().internal_mobility, 3 - driver_count);
        assert_hinge_value(&session, hinge, 0.37, 3);
        assert_planar_translation_value(
            &session,
            x,
            SpatialPlanarTranslationAxis::X,
            1.25,
            2.0e-12,
        );
        assert_planar_translation_value(
            &session,
            y,
            SpatialPlanarTranslationAxis::Y,
            -0.8,
            2.0e-12,
        );
        assert!(
            session
                .mode_evaluation(fixture.side_monitor)
                .unwrap()
                .retained
        );

        for (driver_index, source) in fixture.drivers.iter().copied().enumerate() {
            let mapping = session
                .source_mappings()
                .iter()
                .find(|mapping| mapping.source == source)
                .unwrap();
            let row = &session
                .accepted_result()
                .display_audit
                .sources
                .iter()
                .find(|audit| audit.source_id == mapping.core_source_id)
                .unwrap()
                .rows[0];
            assert_eq!(row.evaluation_status, AuditEvaluationStatus::Evaluated);
            assert_eq!(row.scale.to_bits(), 1.0_f64.to_bits());
            if driver_index == 1 {
                assert!(row.template.contains("first x dot"));
            } else if driver_index == 2 {
                assert!(row.template.contains("first y dot"));
            }
        }
    }
}

#[test]
#[allow(clippy::too_many_lines)]
fn block_base_three_target_transaction_commits_once_and_failures_roll_back_all_state() {
    let fixture = block_base_fixture(3);
    let [hinge, x, y] = fixture.coordinates;
    let [hinge_driver, x_driver, y_driver] = fixture.drivers.as_slice() else {
        panic!("three-driver block/base fixture did not create three drivers");
    };
    let [hinge_driver, x_driver, y_driver] = [*hinge_driver, *x_driver, *y_driver];
    let mut session =
        SpatialAssemblySession::new(fixture.assembly, SolverConfig::default()).unwrap();
    let revision = session.revision();
    session
        .apply_transaction(SpatialAssemblyTransaction::new(
            revision,
            vec![
                SpatialAssemblyEdit::HingeDriverTarget {
                    source: hinge_driver,
                    target: SpatialHingeTarget {
                        principal_phase: -0.42,
                        winding: 3,
                    },
                },
                SpatialAssemblyEdit::TranslationDriverTarget {
                    source: x_driver,
                    target: -2.1,
                },
                SpatialAssemblyEdit::TranslationDriverTarget {
                    source: y_driver,
                    target: 1.6,
                },
            ],
        ))
        .unwrap();
    assert_eq!(session.revision(), revision + 1);
    assert_eq!(session.accepted_result().core_report.rank, 6);
    assert_eq!(session.gauge_report().internal_mobility, 0);
    assert_hinge_value(&session, hinge, -0.42, 3);
    assert_planar_translation_value(&session, x, SpatialPlanarTranslationAxis::X, -2.1, 2.0e-9);
    assert_planar_translation_value(&session, y, SpatialPlanarTranslationAxis::Y, 1.6, 2.0e-9);

    let mut incompatible = block_base_fixture(3);
    let incompatible_x = incompatible.coordinates[1];
    let [hinge_driver, x_driver, y_driver] = incompatible.drivers.as_slice() else {
        panic!("three-driver block/base fixture did not create three drivers");
    };
    let transaction_sources = [*hinge_driver, *x_driver, *y_driver];
    incompatible
        .assembly
        .add_translation_position_driver("duplicate plane-X driver", incompatible_x, 1.25)
        .unwrap();
    let mut incompatible =
        SpatialAssemblySession::new(incompatible.assembly, SolverConfig::default()).unwrap();
    let revision = incompatible.revision();
    assert!(matches!(
        rejected_transaction_retains(
            &mut incompatible,
            SpatialAssemblyTransaction::new(
                revision,
                vec![
                    SpatialAssemblyEdit::HingeDriverTarget {
                        source: transaction_sources[0],
                        target: SpatialHingeTarget {
                            principal_phase: 0.2,
                            winding: 3,
                        },
                    },
                    SpatialAssemblyEdit::TranslationDriverTarget {
                        source: transaction_sources[1],
                        target: 2.0,
                    },
                    SpatialAssemblyEdit::TranslationDriverTarget {
                        source: transaction_sources[2],
                        target: -1.4,
                    },
                ],
            ),
        ),
        SpatialAssemblyError::IncompatibleDriverTargets { coordinate } if coordinate == incompatible_x
    ));

    let fixture = block_base_fixture(3);
    let [hinge_driver, x_driver, y_driver] = fixture.drivers.as_slice() else {
        panic!("three-driver block/base fixture did not create three drivers");
    };
    let sources = [*hinge_driver, *x_driver, *y_driver];
    let side_monitor = fixture.side_monitor;
    let mut mode_invalid =
        SpatialAssemblySession::new(fixture.assembly, SolverConfig::default()).unwrap();
    let revision = mode_invalid.revision();
    assert!(matches!(
        rejected_transaction_retains(
            &mut mode_invalid,
            SpatialAssemblyTransaction::new(
                revision,
                vec![
                    SpatialAssemblyEdit::HingeDriverTarget {
                        source: sources[0],
                        target: SpatialHingeTarget {
                            principal_phase: -0.15,
                            winding: 3,
                        },
                    },
                    SpatialAssemblyEdit::TranslationDriverTarget {
                        source: sources[1],
                        target: 0.4,
                    },
                    SpatialAssemblyEdit::TranslationDriverTarget {
                        source: sources[2],
                        target: 2.3,
                    },
                    SpatialAssemblyEdit::MonitorPlaneSide {
                        monitor: side_monitor,
                        side: SpatialModeSign::Negative,
                    },
                ],
            ),
        ),
        SpatialAssemblyError::IndependentValidation(message) if message.contains("plane side")
    ));
}

#[test]
fn block_base_frame_offset_variant_has_rank_six_and_zero_internal_mobility() {
    let mut assembly = SpatialAssembly::new(1.0).unwrap();
    let base = assembly.add_body("base", Pose3::identity()).unwrap();
    let block = assembly
        .add_body(
            "block",
            Pose3::exp([0.08, -0.05, 0.04, 0.03, -0.04, 0.02]).unwrap(),
        )
        .unwrap();
    let base_frame = assembly
        .add_frame_feature("base frame", base, identity_frame(Point3::origin()))
        .unwrap();
    let block_frame = assembly
        .add_frame_feature("block frame", block, identity_frame(Point3::origin()))
        .unwrap();
    assembly.add_physical_ground("base ground", base).unwrap();
    assembly
        .add_frame_offset_mate(
            "full block offset",
            base_frame,
            block_frame,
            identity_frame(Point3::origin()),
        )
        .unwrap();
    let session = SpatialAssemblySession::new(assembly, SolverConfig::default()).unwrap();
    assert_eq!(session.accepted_result().core_report.rank, 6);
    assert_eq!(session.accepted_result().core_report.right_nullity, 0);
    assert_eq!(session.gauge_report().internal_mobility, 0);
    assert!(session.accepted_result().acceptance_hard_residual_max <= RESIDUAL_TOLERANCE);
}

fn block_base_fixture(driver_count: usize) -> BlockBaseFixture {
    let phase = 0.37;
    let x_target = 1.25;
    let y_target = -0.8;
    let base_plane = identity_frame(Point3::origin());
    let block_plane = translated_frame(
        rotate_frame_about_z(base_plane, phase),
        Vector3::new(x_target, y_target, 0.0),
    );
    let mut assembly = SpatialAssembly::new(1.0).unwrap();
    let base = assembly
        .add_body("grounded base", Pose3::identity())
        .unwrap();
    let block = assembly
        .add_body("planar block", Pose3::identity())
        .unwrap();
    assembly.add_physical_ground("base ground", base).unwrap();
    let first = assembly
        .add_plane_feature("base clocked plane", base, base_plane)
        .unwrap();
    let second = assembly
        .add_plane_feature("block clocked plane", block, block_plane)
        .unwrap();
    let parent = assembly
        .add_planar_joint(
            "block/base planar joint",
            first,
            second,
            SpatialAxisParity::Aligned,
        )
        .unwrap();
    let hinge = assembly
        .add_hinge_coordinate("block normal rotation", parent, 3)
        .unwrap();
    let x = assembly
        .add_planar_translation_coordinate("block plane X", parent, SpatialPlanarTranslationAxis::X)
        .unwrap();
    let y = assembly
        .add_planar_translation_coordinate("block plane Y", parent, SpatialPlanarTranslationAxis::Y)
        .unwrap();
    let side_point = assembly
        .add_point_feature(
            "block positive-normal witness",
            block,
            block_plane.origin() + block_plane.z_axis(),
        )
        .unwrap();
    let side_monitor = assembly
        .add_plane_side_monitor(
            "block positive normal side",
            first,
            side_point,
            SpatialModeSign::Positive,
        )
        .unwrap();
    let mut drivers = Vec::new();
    if driver_count >= 1 {
        drivers.push(
            assembly
                .add_hinge_position_driver(
                    "block rotation driver",
                    hinge,
                    SpatialHingeTarget {
                        principal_phase: phase,
                        winding: 3,
                    },
                )
                .unwrap(),
        );
    }
    if driver_count >= 2 {
        drivers.push(
            assembly
                .add_translation_position_driver("block X driver", x, x_target)
                .unwrap(),
        );
    }
    if driver_count >= 3 {
        drivers.push(
            assembly
                .add_translation_position_driver("block Y driver", y, y_target)
                .unwrap(),
        );
    }
    BlockBaseFixture {
        assembly,
        parent,
        coordinates: [hinge, x, y],
        drivers,
        side_monitor,
    }
}

fn assert_planar_translation_value(
    session: &SpatialAssemblySession,
    coordinate: SpatialCoordinateId,
    expected_axis: SpatialPlanarTranslationAxis,
    expected: f64,
    tolerance: f64,
) {
    let value = session.coordinate_value(coordinate).unwrap();
    let SpatialCoordinateValueKind::PlanarTranslation { axis, value } = value.value else {
        panic!("coordinate {coordinate} is not a planar translation value");
    };
    assert_eq!(axis, expected_axis);
    assert!(
        (value - expected).abs() <= tolerance.max(2.0e-15),
        "planar translation error {:e} exceeds {:e}: actual={value}, expected={expected}",
        (value - expected).abs(),
        tolerance
    );
}

#[derive(Clone, Copy)]
struct ProvenanceCatalog {
    bodies: [SpatialBodyId; 2],
    points: [SpatialPointFeatureId; 2],
    frame: SpatialFrameFeatureId,
    axes: [SpatialAxisFeatureId; 2],
    plane: SpatialPlaneFeatureId,
    source: SpatialSourceId,
    coordinate: SpatialCoordinateId,
    monitor: SpatialModeMonitorId,
}

#[test]
#[allow(clippy::too_many_lines)]
fn every_spatial_id_rejects_same_ordinal_foreign_provenance() {
    let (mut local, local_ids) = provenance_catalog();
    let (foreign, foreign_ids) = provenance_catalog();
    macro_rules! assert_foreign_pair {
        ($local:expr, $foreign:expr) => {{
            assert_eq!($local.as_u64(), $foreign.as_u64());
            assert_ne!($local, $foreign);
            assert_eq!($local.to_string(), $foreign.to_string());
            assert_eq!(format!("{:?}", $local), format!("{:?}", $foreign));
        }};
    }
    for (local_id, foreign_id) in local_ids.bodies.into_iter().zip(foreign_ids.bodies) {
        assert_foreign_pair!(local_id, foreign_id);
        assert!(local_ids.bodies.contains(&local_id));
        assert!(local.body(foreign_id).is_none());
    }
    for (local_id, foreign_id) in local_ids.points.into_iter().zip(foreign_ids.points) {
        assert_foreign_pair!(local_id, foreign_id);
        assert!(local.point_feature(foreign_id).is_none());
    }
    assert_foreign_pair!(local_ids.frame, foreign_ids.frame);
    assert_foreign_pair!(local_ids.axes[0], foreign_ids.axes[0]);
    assert_foreign_pair!(local_ids.plane, foreign_ids.plane);
    assert_foreign_pair!(local_ids.source, foreign_ids.source);
    assert_foreign_pair!(local_ids.coordinate, foreign_ids.coordinate);
    assert_foreign_pair!(local_ids.monitor, foreign_ids.monitor);
    assert!(local.frame_feature(foreign_ids.frame).is_none());
    assert!(local.axis_feature(foreign_ids.axes[0]).is_none());
    assert!(local.plane_feature(foreign_ids.plane).is_none());
    assert!(local.source(foreign_ids.source).is_none());
    assert!(local.coordinate(foreign_ids.coordinate).is_none());
    assert!(local.mode_monitor(foreign_ids.monitor).is_none());
    assert_eq!(local.clone(), local);
    assert_ne!(local, foreign);

    assert!(matches!(
        local.add_point_feature("foreign-body point", foreign_ids.bodies[0], Point3::origin()),
        Err(SpatialAssemblyError::UnknownBody(id)) if id == foreign_ids.bodies[0]
    ));
    assert!(matches!(
        local.add_hinge_coordinate("foreign parent", foreign_ids.source, 0),
        Err(SpatialAssemblyError::UnknownSource(id)) if id == foreign_ids.source
    ));
    assert!(matches!(
        local.add_hinge_position_driver(
            "foreign coordinate",
            foreign_ids.coordinate,
            SpatialHingeTarget {
                principal_phase: 0.0,
                winding: 0,
            },
        ),
        Err(SpatialAssemblyError::UnknownCoordinate(id)) if id == foreign_ids.coordinate
    ));
    assert!(matches!(
        local.add_axis_parity_monitor(
            "foreign monitor feature",
            local_ids.axes[0],
            foreign_ids.axes[1],
            SpatialAxisParity::Aligned,
        ),
        Err(SpatialAssemblyError::UnknownAxisFeature(id)) if id == foreign_ids.axes[1]
    ));

    let mut session = SpatialAssemblySession::new(local, SolverConfig::default()).unwrap();
    let revision = session.revision();
    let accepted_assembly = session.assembly().clone();
    let accepted_result = session.accepted_result().clone();
    assert!(matches!(
        session.set_gauge_policy(
            revision,
            geosolve_linkage::SpatialGaugePolicy::ExplicitReferences {
                bodies: vec![foreign_ids.bodies[0]],
            },
        ),
        Err(SpatialAssemblyError::UnknownBody(id)) if id == foreign_ids.bodies[0]
    ));
    assert_eq!(session.revision(), revision);
    assert_eq!(session.assembly(), &accepted_assembly);
    assert_eq!(session.accepted_result(), &accepted_result);
    assert!(matches!(
        rejected_transaction_retains(
            &mut session,
            SpatialAssemblyTransaction::one(
                revision,
                SpatialAssemblyEdit::SourceAxisParity {
                    source: foreign_ids.source,
                    parity: SpatialAxisParity::Opposed,
                },
            ),
        ),
        SpatialAssemblyError::UnknownSource(id) if id == foreign_ids.source
    ));
    assert!(matches!(
        rejected_transaction_retains(
            &mut session,
            SpatialAssemblyTransaction::one(
                revision,
                SpatialAssemblyEdit::HingeWinding {
                    coordinate: foreign_ids.coordinate,
                    winding: 0,
                },
            ),
        ),
        SpatialAssemblyError::UnknownCoordinate(id) if id == foreign_ids.coordinate
    ));
    assert!(matches!(
        rejected_transaction_retains(
            &mut session,
            SpatialAssemblyTransaction::one(
                revision,
                SpatialAssemblyEdit::MonitorAxisParity {
                    monitor: foreign_ids.monitor,
                    parity: SpatialAxisParity::Aligned,
                },
            ),
        ),
        SpatialAssemblyError::UnknownModeMonitor(id) if id == foreign_ids.monitor
    ));
}

fn provenance_catalog() -> (SpatialAssembly, ProvenanceCatalog) {
    let identity = identity_frame(Point3::origin());
    let mut assembly = SpatialAssembly::new(1.0).unwrap();
    let first_body = assembly.add_body("first", Pose3::identity()).unwrap();
    let second_body = assembly.add_body("second", Pose3::identity()).unwrap();
    let first_point = assembly
        .add_point_feature("first point", first_body, Point3::origin())
        .unwrap();
    let second_point = assembly
        .add_point_feature("second point", second_body, Point3::origin())
        .unwrap();
    let frame = assembly
        .add_frame_feature("frame", first_body, identity)
        .unwrap();
    let first_axis = assembly
        .add_axis_feature("first axis", first_body, identity)
        .unwrap();
    let second_axis = assembly
        .add_axis_feature("second axis", second_body, identity)
        .unwrap();
    let plane = assembly
        .add_plane_feature("plane", first_body, identity)
        .unwrap();
    let source = assembly
        .add_cylindrical_joint(
            "cylindrical",
            first_axis,
            second_axis,
            SpatialAxisParity::Aligned,
        )
        .unwrap();
    let coordinate = assembly.add_hinge_coordinate("hinge", source, 0).unwrap();
    let monitor = assembly
        .add_axis_parity_monitor(
            "parity",
            first_axis,
            second_axis,
            SpatialAxisParity::Aligned,
        )
        .unwrap();
    (
        assembly,
        ProvenanceCatalog {
            bodies: [first_body, second_body],
            points: [first_point, second_point],
            frame,
            axes: [first_axis, second_axis],
            plane,
            source,
            coordinate,
            monitor,
        },
    )
}

#[test]
fn cylindrical_and_planar_wrong_parity_equation_roots_reject_for_both_directions() {
    for parity in [SpatialAxisParity::Aligned, SpatialAxisParity::Opposed] {
        for planar in [false, true] {
            let (assembly, source) = locked_parity_relation(planar, parity);
            assert!(assembly.compile().unwrap().check_jacobians(1.0e-6).is_ok());
            let mut session =
                SpatialAssemblySession::new(assembly, SolverConfig::default()).unwrap();
            let revision = session.revision();
            let opposite = match parity {
                SpatialAxisParity::Aligned => SpatialAxisParity::Opposed,
                SpatialAxisParity::Opposed => SpatialAxisParity::Aligned,
            };
            assert!(matches!(
                rejected_transaction_retains(
                    &mut session,
                    SpatialAssemblyTransaction::one(
                        revision,
                        SpatialAssemblyEdit::SourceAxisParity {
                            source,
                            parity: opposite,
                        },
                    ),
                ),
                SpatialAssemblyError::IndependentValidation(message) if message.contains("parity")
            ));
        }
    }
}

fn locked_parity_relation(
    planar: bool,
    parity: SpatialAxisParity,
) -> (SpatialAssembly, SpatialSourceId) {
    let first_frame = identity_frame(Point3::origin());
    let second_frame = if parity == SpatialAxisParity::Aligned {
        first_frame
    } else {
        opposed_frame(first_frame)
    };
    let mut assembly = SpatialAssembly::new(1.0).unwrap();
    let first_body = assembly.add_body("first", Pose3::identity()).unwrap();
    let second_body = assembly.add_body("second", Pose3::identity()).unwrap();
    assembly
        .add_physical_ground("first ground", first_body)
        .unwrap();
    assembly
        .add_physical_ground("second ground", second_body)
        .unwrap();
    let source = if planar {
        let first = assembly
            .add_plane_feature("first plane", first_body, first_frame)
            .unwrap();
        let second = assembly
            .add_plane_feature("second plane", second_body, second_frame)
            .unwrap();
        assembly
            .add_planar_joint("locked planar", first, second, parity)
            .unwrap()
    } else {
        let first = assembly
            .add_axis_feature("first axis", first_body, first_frame)
            .unwrap();
        let second = assembly
            .add_axis_feature("second axis", second_body, second_frame)
            .unwrap();
        assembly
            .add_cylindrical_joint("locked cylindrical", first, second, parity)
            .unwrap()
    };
    (assembly, source)
}

#[test]
fn axis_angle_endpoint_candidates_reject_session_rebuild_and_retain_every_view() {
    let identity = identity_frame(Point3::origin());
    let mut assembly = SpatialAssembly::new(1.0).unwrap();
    let first_body = assembly.add_body("first", Pose3::identity()).unwrap();
    let second_body = assembly.add_body("second", Pose3::identity()).unwrap();
    let first = assembly
        .add_axis_feature("first axis", first_body, identity)
        .unwrap();
    let second = assembly
        .add_axis_feature(
            "second axis",
            second_body,
            rotate_frame_about_x(identity, 0.7),
        )
        .unwrap();
    assembly
        .add_physical_ground("first ground", first_body)
        .unwrap();
    assembly
        .add_physical_ground("second ground", second_body)
        .unwrap();
    assembly
        .add_axis_angle_mate("interior angle", first, second, 0.7)
        .unwrap();
    let session = SpatialAssemblySession::new(assembly, SolverConfig::default()).unwrap();
    for endpoint in [identity, opposed_frame(identity)] {
        let error = rejected_patch_retains(
            session.clone(),
            SpatialPatch::AxisLocal {
                feature: second,
                local_frame: endpoint,
            },
        );
        assert!(matches!(
            error,
            SpatialAssemblyError::InitialRejected(_)
                | SpatialAssemblyError::IndependentValidation(_)
                | SpatialAssemblyError::Session(_)
        ));
    }
}

#[test]
#[allow(clippy::too_many_lines)]
fn monitor_only_gauge_adversaries_preserve_physical_rank_mappings_and_row_isolation() {
    let identity = identity_frame(Point3::origin());

    let mut alternate = SpatialAssembly::new(1.0).unwrap();
    let first_body = alternate.add_body("first free", Pose3::identity()).unwrap();
    let second_body = alternate
        .add_body("second free", Pose3::identity())
        .unwrap();
    let first_axis = alternate
        .add_axis_feature("first axis", first_body, identity)
        .unwrap();
    let second_axis = alternate
        .add_axis_feature("second axis", second_body, identity)
        .unwrap();
    alternate
        .add_axis_parity_monitor(
            "monitor-only link",
            first_axis,
            second_axis,
            SpatialAxisParity::Aligned,
        )
        .unwrap();
    let mut alternate = SpatialAssemblySession::new(alternate, SolverConfig::default()).unwrap();
    let before_relative = spatial_relative_pose(&alternate, first_body, second_body);
    let before_rank = alternate.accepted_result().core_report.rank;
    let before_nullity = alternate.accepted_result().core_report.right_nullity;
    let revision = alternate.revision();
    alternate
        .set_gauge_policy(
            revision,
            geosolve_linkage::SpatialGaugePolicy::ExplicitReferences {
                bodies: vec![second_body],
            },
        )
        .unwrap();
    assert_eq!(alternate.revision(), revision + 1);
    assert_eq!(
        alternate.gauge_report().components[0]
            .numerical_reference
            .unwrap()
            .body,
        second_body
    );
    assert_eq!(alternate.accepted_result().core_report.rank, before_rank);
    assert_eq!(
        alternate.accepted_result().core_report.right_nullity,
        before_nullity
    );
    assert_eq!(alternate.accepted_result().core_report.right_nullity, 12);
    assert_eq!(alternate.gauge_report().gauge_dof, 6);
    assert_eq!(alternate.gauge_report().internal_mobility, 6);
    assert_pose_close(
        spatial_relative_pose(&alternate, first_body, second_body),
        before_relative,
        2.0e-12,
    );
    assert!(alternate.source_mappings().is_empty());
    assert!(alternate.accepted_result().display_audit.sources.is_empty());
    assert_no_private_spatial_rows(&alternate);

    let mut grounded_free = SpatialAssembly::new(1.0).unwrap();
    let grounded = grounded_free
        .add_body("grounded", Pose3::identity())
        .unwrap();
    let free = grounded_free.add_body("free", Pose3::identity()).unwrap();
    let grounded_axis = grounded_free
        .add_axis_feature("grounded axis", grounded, identity)
        .unwrap();
    let free_axis = grounded_free
        .add_axis_feature("free axis", free, identity)
        .unwrap();
    grounded_free
        .add_physical_ground("physical ground", grounded)
        .unwrap();
    let monitor = grounded_free
        .add_axis_parity_monitor(
            "grounded/free mode link",
            grounded_axis,
            free_axis,
            SpatialAxisParity::Aligned,
        )
        .unwrap();
    let grounded_free =
        SpatialAssemblySession::new(grounded_free, SolverConfig::default()).unwrap();
    assert_eq!(grounded_free.accepted_result().core_report.rank, 0);
    assert_eq!(grounded_free.accepted_result().core_report.right_nullity, 6);
    assert_eq!(grounded_free.gauge_report().gauge_dof, 0);
    assert_eq!(grounded_free.gauge_report().internal_mobility, 6);
    assert_eq!(
        grounded_free.gauge_report().components[0].mode_monitors,
        vec![monitor]
    );
    assert_eq!(grounded_free.source_mappings().len(), 1);
    assert_eq!(
        grounded_free.accepted_result().display_audit.sources.len(),
        1
    );
    assert_pose_close(
        spatial_relative_pose(&grounded_free, grounded, free),
        Pose3::identity(),
        2.0e-12,
    );
    assert!(
        grounded_free
            .accepted_result()
            .display_audit
            .sources
            .iter()
            .all(|source| !source.source_label.contains("numerical gauge"))
    );
    assert_no_private_spatial_rows(&grounded_free);

    let mut paired = SpatialAssembly::new(1.0).unwrap();
    let bodies =
        ["A1", "A2", "B1", "B2"].map(|label| paired.add_body(label, Pose3::identity()).unwrap());
    let frames = bodies.map(|body| {
        paired
            .add_frame_feature("pair frame", body, identity)
            .unwrap()
    });
    let first_fixed = paired
        .add_fixed_frame("first constrained component", frames[0], frames[1])
        .unwrap();
    let second_fixed = paired
        .add_fixed_frame("second constrained component", frames[2], frames[3])
        .unwrap();
    let first_monitor_axis = paired
        .add_axis_feature("first component monitor axis", bodies[1], identity)
        .unwrap();
    let second_monitor_axis = paired
        .add_axis_feature("second component monitor axis", bodies[2], identity)
        .unwrap();
    let pair_monitor = paired
        .add_axis_parity_monitor(
            "connect constrained components",
            first_monitor_axis,
            second_monitor_axis,
            SpatialAxisParity::Aligned,
        )
        .unwrap();
    let paired = SpatialAssemblySession::new(paired, SolverConfig::default()).unwrap();
    assert_eq!(paired.accepted_result().core_report.rank, 12);
    assert_eq!(paired.accepted_result().core_report.right_nullity, 12);
    assert_eq!(paired.gauge_report().gauge_dof, 6);
    assert_eq!(paired.gauge_report().internal_mobility, 6);
    assert_eq!(paired.gauge_report().components.len(), 1);
    assert_eq!(
        paired.gauge_report().components[0]
            .core_component_indices
            .len(),
        2
    );
    assert_eq!(
        paired.gauge_report().components[0].mode_monitors,
        vec![pair_monitor]
    );
    assert_eq!(
        paired
            .source_mappings()
            .iter()
            .map(|mapping| mapping.source)
            .collect::<Vec<_>>(),
        vec![first_fixed, second_fixed]
    );
    assert_eq!(audit_row_count(&paired), 12);
    assert_private_gauges_absent(&paired);
    assert_no_private_spatial_rows(&paired);
    assert_pose_close(
        spatial_relative_pose(&paired, bodies[0], bodies[1]),
        Pose3::identity(),
        2.0e-12,
    );
    assert_pose_close(
        spatial_relative_pose(&paired, bodies[2], bodies[3]),
        Pose3::identity(),
        2.0e-12,
    );
}

fn spatial_relative_pose(
    session: &SpatialAssemblySession,
    first: SpatialBodyId,
    second: SpatialBodyId,
) -> Pose3 {
    let first = session.accepted_result().geometry.body_pose(first).unwrap();
    let second = session
        .accepted_result()
        .geometry
        .body_pose(second)
        .unwrap();
    first.inverse().unwrap().compose(&second).unwrap()
}

fn assert_pose_close(actual: Pose3, expected: Pose3, tolerance: f64) {
    let difference = expected.local_difference(&actual).unwrap();
    assert!(
        difference.iter().all(|value| value.abs() <= tolerance),
        "pose difference {difference:?} exceeds {tolerance:e}"
    );
}

fn assert_no_private_spatial_rows(session: &SpatialAssemblySession) {
    assert!(
        session
            .accepted_result()
            .display_audit
            .sources
            .iter()
            .all(|source| !source.source_label.contains("numerical gauge"))
    );
    let linearization = session
        .core_session()
        .accepted_hard_linearization()
        .unwrap();
    let physical_rows = linearization
        .components()
        .iter()
        .flat_map(geosolve_core::AcceptedHardComponentLinearization::hard_rows)
        .collect::<Vec<_>>();
    assert!(physical_rows.iter().all(|row| {
        session.source_mappings().iter().any(|mapping| {
            mapping.core_source_id == row.row.source_id
                && mapping.residual_ids.contains(&row.row.residual_id)
        })
    }));
}

#[test]
#[allow(clippy::too_many_lines)]
fn shaft_bearing_translation_side_and_failed_combined_edit_retain_targets_modes_and_views() {
    let mut fixture = coordinate_fixture(
        CoordinateRelation::Cylindrical,
        1.0,
        true,
        SpatialAxisParity::Aligned,
        0.48,
        1.9,
        2,
        true,
        true,
        false,
        None,
    );
    let first_axis = fixture.assembly.axis_features()[0].clone();
    let second_axis = fixture.assembly.axis_features()[1].clone();
    let plane = fixture
        .assembly
        .add_plane_feature(
            "bearing translation plane",
            first_axis.body(),
            first_axis.local_frame(),
        )
        .unwrap();
    let point = fixture
        .assembly
        .add_point_feature(
            "shaft translation witness",
            second_axis.body(),
            second_axis.local_origin(),
        )
        .unwrap();
    let side = fixture
        .assembly
        .add_plane_side_monitor(
            "positive shaft translation side",
            plane,
            point,
            SpatialModeSign::Positive,
        )
        .unwrap();
    let hinge_driver = fixture.hinge_driver.unwrap();
    let translation_driver = fixture.translation_driver.unwrap();
    let mut session =
        SpatialAssemblySession::new(fixture.assembly, SolverConfig::default()).unwrap();
    assert!(
        (session
            .mode_evaluation(side)
            .unwrap()
            .fresh_raw_metric
            .unwrap()
            - 1.9)
            .abs()
            <= 2.0e-12
    );
    let revision = session.revision();
    assert!(matches!(
        rejected_transaction_retains(
            &mut session,
            SpatialAssemblyTransaction::new(
                revision,
                vec![
                    SpatialAssemblyEdit::HingeDriverTarget {
                        source: hinge_driver,
                        target: SpatialHingeTarget {
                            principal_phase: -0.35,
                            winding: 2,
                        },
                    },
                    SpatialAssemblyEdit::TranslationDriverTarget {
                        source: translation_driver,
                        target: 0.75,
                    },
                    SpatialAssemblyEdit::MonitorPlaneSide {
                        monitor: side,
                        side: SpatialModeSign::Negative,
                    },
                ],
            ),
        ),
        SpatialAssemblyError::IndependentValidation(message) if message.contains("plane side")
    ));
    assert!(matches!(
        session.assembly().source(hinge_driver).unwrap().kind(),
        SpatialSourceKind::HingePositionDriver {
            target: SpatialHingeTarget {
                principal_phase: 0.48,
                winding: 2,
            },
            ..
        }
    ));
    assert!(matches!(
        session
            .assembly()
            .source(translation_driver)
            .unwrap()
            .kind(),
        SpatialSourceKind::TranslationPositionDriver { target: 1.9, .. }
    ));
    assert!(matches!(
        session.assembly().mode_monitor(side).unwrap().kind(),
        SpatialModeMonitorKind::PlaneSide {
            side: SpatialModeSign::Positive,
            ..
        }
    ));
}

#[test]
fn every_m20_joint_has_literal_1e_minus_6_to_1e6_relative_geometry_at_model_scale_one() {
    for primitive in standard_joints() {
        let (assembly, source) = literal_mixed_scale_joint(primitive, 1.0e6);
        assert_literal_mixed_local_offsets(&assembly);
        if !matches!(primitive, JointPrimitive::Universal) {
            let jacobians = assembly
                .compile()
                .unwrap()
                .check_jacobians(1.0e-6)
                .unwrap_or_else(|error| panic!("mixed-scale {primitive:?}: {error:#?}"));
            assert!(
                jacobians.max_relative_error() <= 1.0e-6,
                "mixed-scale {primitive:?}: {jacobians:#?}"
            );
        }
        let session = SpatialAssemblySession::new(assembly, SolverConfig::default()).unwrap();
        assert_mixed_scale_publication(&session, source, primitive.expected_rows());
        assert_eq!(
            session.accepted_result().core_report.rank,
            primitive.expected_rows()
        );
        assert_eq!(session.accepted_result().core_report.left_nullity, 0);
        assert_eq!(
            session.gauge_report().gauge_dof,
            if matches!(primitive, JointPrimitive::Universal) {
                6
            } else {
                0
            }
        );
        assert_eq!(
            session.gauge_report().internal_mobility,
            primitive.expected_internal_mobility()
        );
        match (primitive, session.assembly().source(source).unwrap().kind()) {
            (
                JointPrimitive::Prismatic(expected),
                SpatialSourceKind::PrismaticJoint { parity, .. },
            )
            | (
                JointPrimitive::Cylindrical(expected),
                SpatialSourceKind::CylindricalJoint { parity, .. },
            )
            | (JointPrimitive::Planar(expected), SpatialSourceKind::PlanarJoint { parity, .. }) => {
                assert_eq!(parity, expected);
            }
            (JointPrimitive::Universal, SpatialSourceKind::UniversalJoint { .. }) => {}
            _ => panic!("mixed-scale joint source kind changed for {primitive:?}"),
        }
    }
}

#[test]
fn every_m20_mate_has_literal_1e_minus_6_to_1e6_relative_geometry_at_model_scale_one() {
    for primitive in standard_mates() {
        let (assembly, source) = literal_mixed_scale_mate(primitive, 1.0e6);
        assert_literal_mixed_local_offsets(&assembly);
        if matches!(
            primitive,
            MatePrimitive::AxisAngle | MatePrimitive::AxisAlignment(_)
        ) {
            let jacobians = assembly
                .compile()
                .unwrap()
                .check_jacobians(1.0e-6)
                .unwrap_or_else(|error| panic!("mixed-scale {primitive:?}: {error:#?}"));
            assert!(
                jacobians.max_relative_error() <= 1.0e-6,
                "mixed-scale {primitive:?}: {jacobians:#?}"
            );
        }
        let session = SpatialAssemblySession::new(assembly, SolverConfig::default()).unwrap();
        assert_mixed_scale_publication(&session, source, primitive.expected_rows());
        assert_eq!(
            session.accepted_result().core_report.rank,
            primitive.expected_rows()
        );
        assert_eq!(session.accepted_result().core_report.left_nullity, 0);
        assert_eq!(session.gauge_report().gauge_dof, 0);
        assert_eq!(
            session.gauge_report().internal_mobility,
            primitive.expected_internal_mobility()
        );
        match (primitive, session.assembly().source(source).unwrap().kind()) {
            (
                MatePrimitive::PointDistance,
                SpatialSourceKind::PointDistanceMate { distance, .. },
            ) => {
                assert_eq!(distance.to_bits(), 1.0e6_f64.to_bits());
            }
            (MatePrimitive::AxisAngle, SpatialSourceKind::AxisAngleMate { angle, .. }) => {
                assert_eq!(angle.to_bits(), 0.83_f64.to_bits());
            }
            (
                MatePrimitive::AxisAlignment(expected),
                SpatialSourceKind::AxisAlignmentMate { parity, .. },
            ) => assert_eq!(parity, expected),
            (MatePrimitive::FrameOffset, SpatialSourceKind::FrameOffsetMate { offset, .. }) => {
                assert_eq!(offset, mate_offset(1.0e6 / 0.8));
            }
            _ => panic!("mixed-scale mate source kind changed for {primitive:?}"),
        }
    }
}

#[test]
fn block_base_mixes_micro_and_macro_length_rows_with_an_angle_driver() {
    let first_frame = identity_frame(Point3::new(1.0e-6, -2.0e-6, 3.0e-6));
    let second_frame = translated_frame(
        rotate_frame_about_z(first_frame, 0.41),
        Vector3::new(0.0, 2.0e-6, 0.0),
    );
    let mut assembly = SpatialAssembly::new(1.0).unwrap();
    let base = assembly.add_body("mixed base", Pose3::identity()).unwrap();
    let block = assembly
        .add_body(
            "mixed block",
            Pose3::exp([1.0e6, 0.0, 0.0, 0.0, 0.0, 0.0]).unwrap(),
        )
        .unwrap();
    assembly.add_physical_ground("base ground", base).unwrap();
    let first = assembly
        .add_plane_feature("microscopic base plane", base, first_frame)
        .unwrap();
    let second = assembly
        .add_plane_feature("macroscopic block plane", block, second_frame)
        .unwrap();
    let joint = assembly
        .add_planar_joint(
            "mixed planar joint",
            first,
            second,
            SpatialAxisParity::Aligned,
        )
        .unwrap();
    let hinge = assembly
        .add_hinge_coordinate("mixed hinge", joint, -2)
        .unwrap();
    let x = assembly
        .add_planar_translation_coordinate("mixed plane X", joint, SpatialPlanarTranslationAxis::X)
        .unwrap();
    let y = assembly
        .add_planar_translation_coordinate("mixed plane Y", joint, SpatialPlanarTranslationAxis::Y)
        .unwrap();
    let hinge_driver = assembly
        .add_hinge_position_driver(
            "mixed angle driver",
            hinge,
            SpatialHingeTarget {
                principal_phase: 0.41,
                winding: -2,
            },
        )
        .unwrap();
    let x_driver = assembly
        .add_translation_position_driver("macro X driver", x, 1.0e6)
        .unwrap();
    let y_driver = assembly
        .add_translation_position_driver("micro Y driver", y, 2.0e-6)
        .unwrap();
    let session = SpatialAssemblySession::new(assembly, SolverConfig::default()).unwrap();
    assert_eq!(session.accepted_result().core_report.rank, 6);
    assert_eq!(session.gauge_report().internal_mobility, 0);
    assert_hinge_value(&session, hinge, 0.41, -2);
    assert_planar_translation_value(&session, x, SpatialPlanarTranslationAxis::X, 1.0e6, 2.0e-9);
    assert_planar_translation_value(&session, y, SpatialPlanarTranslationAxis::Y, 2.0e-6, 2.0e-9);
    for (source, unit) in [
        (hinge_driver, "dimensionless"),
        (x_driver, "model-unit"),
        (y_driver, "model-unit"),
    ] {
        let mapping = session
            .source_mappings()
            .iter()
            .find(|mapping| mapping.source == source)
            .unwrap();
        let row = &session
            .accepted_result()
            .display_audit
            .sources
            .iter()
            .find(|audit| audit.source_id == mapping.core_source_id)
            .unwrap()
            .rows[0];
        assert_eq!(row.unit, unit);
        assert_eq!(row.scale.to_bits(), 1.0_f64.to_bits());
        assert!(row.raw_residual.is_finite() && row.normalized_residual.is_finite());
    }
}

#[test]
fn cancellation_limited_mixed_offset_jacobians_use_documented_1e4_span() {
    // At a literal 1e6 cancellation offset, an h=1e-6 translation difference loses
    // roughly 1e-5 to subtraction. A 1e4 span is the largest decimal span retained
    // here that keeps the universal and frame-offset translation columns below 1e-6.
    let (universal, _) = literal_mixed_scale_joint(JointPrimitive::Universal, 1.0e4);
    let universal = universal
        .compile()
        .unwrap()
        .check_jacobians(1.0e-6)
        .unwrap();
    assert!(universal.max_relative_error() <= 1.0e-6, "{universal:#?}");

    for primitive in [MatePrimitive::PointDistance, MatePrimitive::FrameOffset] {
        let (mate, _) = literal_mixed_scale_mate(primitive, 1.0e4);
        let report = mate.compile().unwrap().check_jacobians(1.0e-6).unwrap();
        assert!(
            report.max_relative_error() <= 1.0e-6,
            "{primitive:?}: {report:#?}"
        );
    }
}

#[test]
fn planar_mixed_length_driver_jacobians_use_documented_1e_minus_3_to_1_span() {
    // The literal 1e-6/1e6 acceptance fixture intentionally exceeds central-
    // difference resolution for derivatives of the microscopic transverse target.
    let first_frame = identity_frame(Point3::new(1.0e-3, 0.0, 0.0));
    let second_frame = translated_frame(
        rotate_frame_about_z(first_frame, 0.41),
        Vector3::new(1.0, 1.0e-3, 0.0),
    );
    let mut assembly = SpatialAssembly::new(1.0).unwrap();
    let base = assembly.add_body("base", Pose3::identity()).unwrap();
    let block = assembly.add_body("block", Pose3::identity()).unwrap();
    assembly.add_physical_ground("base ground", base).unwrap();
    let first = assembly
        .add_plane_feature("first", base, first_frame)
        .unwrap();
    let second = assembly
        .add_plane_feature("second", block, second_frame)
        .unwrap();
    let joint = assembly
        .add_planar_joint("planar", first, second, SpatialAxisParity::Aligned)
        .unwrap();
    let hinge = assembly.add_hinge_coordinate("hinge", joint, 0).unwrap();
    let x = assembly
        .add_planar_translation_coordinate("x", joint, SpatialPlanarTranslationAxis::X)
        .unwrap();
    let y = assembly
        .add_planar_translation_coordinate("y", joint, SpatialPlanarTranslationAxis::Y)
        .unwrap();
    assembly
        .add_hinge_position_driver(
            "angle",
            hinge,
            SpatialHingeTarget {
                principal_phase: 0.41,
                winding: 0,
            },
        )
        .unwrap();
    assembly
        .add_translation_position_driver("x driver", x, 1.0)
        .unwrap();
    assembly
        .add_translation_position_driver("y driver", y, 1.0e-3)
        .unwrap();
    let report = assembly.compile().unwrap().check_jacobians(1.0e-6).unwrap();
    assert!(report.max_relative_error() <= 1.0e-6, "{report:#?}");
}

#[allow(clippy::too_many_lines)]
fn literal_mixed_scale_joint(
    primitive: JointPrimitive,
    span: f64,
) -> (SpatialAssembly, SpatialSourceId) {
    let micro = 1.0e-6;
    let first_world = identity_frame(Point3::new(micro, -2.0 * micro, 3.0 * micro));
    let (second_pose, second_world) = match primitive {
        JointPrimitive::Prismatic(SpatialAxisParity::Aligned) => {
            let frame = translated_frame(first_world, first_world.z_axis() * span);
            (mixed_body_pose_for_frame(frame), frame)
        }
        JointPrimitive::Prismatic(SpatialAxisParity::Opposed) => {
            let frame = translated_frame(opposed_frame(first_world), first_world.z_axis() * span);
            (mixed_body_pose_for_frame(frame), frame)
        }
        JointPrimitive::Cylindrical(SpatialAxisParity::Aligned) => {
            let frame = translated_frame(
                rotate_frame_about_z(first_world, 0.63),
                first_world.z_axis() * span,
            );
            (mixed_body_pose_for_frame(frame), frame)
        }
        JointPrimitive::Cylindrical(SpatialAxisParity::Opposed) => {
            let frame = translated_frame(
                rotate_frame_about_z(opposed_frame(first_world), 0.63),
                first_world.z_axis() * span,
            );
            (mixed_body_pose_for_frame(frame), frame)
        }
        JointPrimitive::Planar(SpatialAxisParity::Aligned) => {
            let frame = translated_frame(
                rotate_frame_about_z(first_world, 0.47),
                first_world.x_axis() * span + first_world.y_axis() * micro,
            );
            (mixed_body_pose_for_frame(frame), frame)
        }
        JointPrimitive::Planar(SpatialAxisParity::Opposed) => {
            let frame = translated_frame(
                rotate_frame_about_z(opposed_frame(first_world), 0.47),
                first_world.x_axis() * span + first_world.y_axis() * micro,
            );
            (mixed_body_pose_for_frame(frame), frame)
        }
        JointPrimitive::Universal => {
            let pose = Pose3::exp([span, -span, 0.5 * span, 0.0, 0.0, 0.0]).unwrap();
            (
                pose,
                Frame3::try_new(
                    first_world.origin(),
                    first_world.y_axis(),
                    first_world.z_axis(),
                    first_world.x_axis(),
                )
                .unwrap(),
            )
        }
    };
    let mut assembly = SpatialAssembly::new(1.0).unwrap();
    let first_body = assembly.add_body("micro body", Pose3::identity()).unwrap();
    let second_body = assembly.add_body("macro body", second_pose).unwrap();
    if !matches!(primitive, JointPrimitive::Universal) {
        assembly
            .add_physical_ground("micro body ground", first_body)
            .unwrap();
    }
    let second_local = local_frame(second_pose, second_world);
    let source = match primitive {
        JointPrimitive::Prismatic(parity) => {
            let first = assembly
                .add_axis_feature("micro prismatic axis", first_body, first_world)
                .unwrap();
            let second = assembly
                .add_axis_feature("macro prismatic axis", second_body, second_local)
                .unwrap();
            assembly
                .add_prismatic_joint("mixed prismatic", first, second, parity)
                .unwrap()
        }
        JointPrimitive::Cylindrical(parity) => {
            let first = assembly
                .add_axis_feature("micro cylindrical axis", first_body, first_world)
                .unwrap();
            let second = assembly
                .add_axis_feature("macro cylindrical axis", second_body, second_local)
                .unwrap();
            assembly
                .add_cylindrical_joint("mixed cylindrical", first, second, parity)
                .unwrap()
        }
        JointPrimitive::Planar(parity) => {
            let first = assembly
                .add_plane_feature("micro planar plane", first_body, first_world)
                .unwrap();
            let second = assembly
                .add_plane_feature("macro planar plane", second_body, second_local)
                .unwrap();
            assembly
                .add_planar_joint("mixed planar", first, second, parity)
                .unwrap()
        }
        JointPrimitive::Universal => {
            let first = assembly
                .add_axis_feature("micro universal axis", first_body, first_world)
                .unwrap();
            let second = assembly
                .add_axis_feature("macro universal axis", second_body, second_local)
                .unwrap();
            assembly
                .add_universal_joint("mixed universal", first, second)
                .unwrap()
        }
    };
    (assembly, source)
}

#[allow(clippy::too_many_lines)]
fn literal_mixed_scale_mate(
    primitive: MatePrimitive,
    macro_length: f64,
) -> (SpatialAssembly, SpatialSourceId) {
    let micro = 1.0e-6;
    let first_frame = identity_frame(Point3::new(micro, -2.0 * micro, 3.0 * micro));
    let second_frame = match primitive {
        MatePrimitive::PointDistance => translated_frame(first_frame, Vector3::x() * macro_length),
        MatePrimitive::AxisAngle => translated_frame(
            rotate_frame_about_x(first_frame, 0.83),
            Vector3::new(macro_length, micro, -micro),
        ),
        MatePrimitive::AxisAlignment(SpatialAxisParity::Aligned) => translated_frame(
            rotate_frame_about_z(first_frame, 0.57),
            Vector3::new(macro_length, micro, -micro),
        ),
        MatePrimitive::AxisAlignment(SpatialAxisParity::Opposed) => translated_frame(
            rotate_frame_about_z(opposed_frame(first_frame), 0.57),
            Vector3::new(macro_length, micro, -micro),
        ),
        MatePrimitive::FrameOffset => compose_frame(first_frame, mate_offset(macro_length / 0.8)),
    };
    let second_pose = match primitive {
        MatePrimitive::PointDistance => {
            Pose3::exp([macro_length, 0.0, 0.0, 0.0, 0.0, 0.0]).unwrap()
        }
        MatePrimitive::FrameOffset => {
            Pose3::try_new(second_frame.origin().coords, [1.0, 0.0, 0.0, 0.0]).unwrap()
        }
        _ => mixed_body_pose_for_frame(second_frame),
    };
    let mut assembly = SpatialAssembly::new(1.0).unwrap();
    let first_body = assembly.add_body("micro body", Pose3::identity()).unwrap();
    let second_body = assembly.add_body("macro body", second_pose).unwrap();
    assembly
        .add_physical_ground("micro body ground", first_body)
        .unwrap();
    let source = match primitive {
        MatePrimitive::PointDistance => {
            let first_world = Point3::new(micro, 0.0, 0.0);
            let second_world = first_world + Vector3::x() * macro_length;
            let first = assembly
                .add_point_feature("micro point", first_body, first_world)
                .unwrap();
            let second = assembly
                .add_point_feature(
                    "macro point",
                    second_body,
                    second_pose
                        .try_inverse_transform_point(second_world)
                        .unwrap(),
                )
                .unwrap();
            assembly
                .add_point_distance_mate("mixed distance", first, second, macro_length)
                .unwrap()
        }
        MatePrimitive::AxisAngle => {
            let first = assembly
                .add_axis_feature("micro angle axis", first_body, first_frame)
                .unwrap();
            let second = assembly
                .add_axis_feature(
                    "macro angle axis",
                    second_body,
                    local_frame(second_pose, second_frame),
                )
                .unwrap();
            assembly
                .add_axis_angle_mate("mixed angle", first, second, 0.83)
                .unwrap()
        }
        MatePrimitive::AxisAlignment(parity) => {
            let first = assembly
                .add_axis_feature("micro alignment axis", first_body, first_frame)
                .unwrap();
            let second = assembly
                .add_axis_feature(
                    "macro alignment axis",
                    second_body,
                    local_frame(second_pose, second_frame),
                )
                .unwrap();
            assembly
                .add_axis_alignment_mate("mixed alignment", first, second, parity)
                .unwrap()
        }
        MatePrimitive::FrameOffset => {
            let first = assembly
                .add_frame_feature("micro offset frame", first_body, first_frame)
                .unwrap();
            let second = assembly
                .add_frame_feature(
                    "macro offset frame",
                    second_body,
                    local_frame(second_pose, second_frame),
                )
                .unwrap();
            assembly
                .add_frame_offset_mate(
                    "mixed frame offset",
                    first,
                    second,
                    mate_offset(macro_length / 0.8),
                )
                .unwrap()
        }
    };
    (assembly, source)
}

fn assert_literal_mixed_local_offsets(assembly: &SpatialAssembly) {
    let magnitudes = assembly
        .bodies()
        .iter()
        .map(|body| body.pose_guess().translation().norm())
        .chain(
            assembly
                .point_features()
                .iter()
                .map(|feature| feature.local_point().coords.norm()),
        )
        .chain(
            assembly
                .frame_features()
                .iter()
                .map(|feature| feature.local_frame().origin().coords.norm()),
        )
        .chain(
            assembly
                .axis_features()
                .iter()
                .map(|feature| feature.local_origin().coords.norm()),
        )
        .chain(
            assembly
                .plane_features()
                .iter()
                .map(|feature| feature.local_origin().coords.norm()),
        )
        .collect::<Vec<_>>();
    assert!(
        magnitudes
            .iter()
            .any(|value| *value > 0.0 && *value < 1.0e-5)
    );
    assert!(magnitudes.iter().any(|value| *value > 5.0e5));
}

fn mixed_body_pose_for_frame(world: Frame3) -> Pose3 {
    let local_origin = Point3::new(2.0e-6, -1.0e-6, 3.0e-6);
    Pose3::try_new(world.origin() - local_origin, [1.0, 0.0, 0.0, 0.0]).unwrap()
}

fn assert_mixed_scale_publication(
    session: &SpatialAssemblySession,
    source: SpatialSourceId,
    expected_rows: usize,
) {
    assert!(session.accepted_result().acceptance_hard_residual_max <= RESIDUAL_TOLERANCE);
    assert!(
        session
            .accepted_result()
            .geometry
            .bodies
            .iter()
            .all(|body| body.pose.ambient().iter().all(|value| value.is_finite()))
    );
    assert!(
        session
            .accepted_result()
            .geometry
            .points
            .iter()
            .all(|point| point.world.coords.iter().all(|value| value.is_finite()))
    );
    assert!(
        session
            .accepted_result()
            .geometry
            .frames
            .iter()
            .map(|frame| frame.world)
            .chain(
                session
                    .accepted_result()
                    .geometry
                    .axes
                    .iter()
                    .map(|axis| axis.world)
            )
            .chain(
                session
                    .accepted_result()
                    .geometry
                    .planes
                    .iter()
                    .map(|plane| plane.world)
            )
            .all(|frame| frame
                .origin()
                .coords
                .iter()
                .chain(frame.x_axis().iter())
                .chain(frame.y_axis().iter())
                .chain(frame.z_axis().iter())
                .all(|value| value.is_finite()))
    );
    let mapping = session
        .source_mappings()
        .iter()
        .find(|mapping| mapping.source == source)
        .unwrap();
    let audit = session
        .accepted_result()
        .display_audit
        .sources
        .iter()
        .find(|audit| audit.source_id == mapping.core_source_id)
        .unwrap();
    assert_eq!(audit.rows.len(), expected_rows);
    assert!(audit.rows.iter().all(|row| {
        row.evaluation_status == AuditEvaluationStatus::Evaluated
            && row.scale.to_bits() == 1.0_f64.to_bits()
            && row.raw_residual.is_finite()
            && row.normalized_residual.is_finite()
    }));
}

#[test]
#[allow(clippy::too_many_lines)]
fn public_spatial_examples_are_exact_finite_and_deterministic_at_all_scales() {
    for (kind, key) in [
        (SpatialExampleKind::ShaftBearing, "shaft-bearing"),
        (SpatialExampleKind::BlockBase, "block-base"),
    ] {
        assert_eq!(kind.key(), key);
        for scale in [1.0e-6, 1.0, 1.0e6] {
            let first = spatial_example(kind, scale).unwrap();
            let second = spatial_example(kind, scale).unwrap();
            assert_ne!(
                first.assembly.bodies()[0].id(),
                second.assembly.bodies()[0].id()
            );
            assert_eq!(
                spatial_example_order(&first.assembly),
                spatial_example_order(&second.assembly)
            );
            assert_eq!(
                spatial_example_id_ordinals(&first.ids),
                spatial_example_id_ordinals(&second.ids)
            );
            let source_order = first
                .assembly
                .sources()
                .iter()
                .map(geosolve_linkage::SpatialSource::id)
                .collect::<Vec<_>>();
            let ids = first.ids;
            let session = SpatialAssemblySession::new(first.assembly, SolverConfig::default())
                .unwrap_or_else(|error| panic!("{key}, scale={scale:e}: {error:#?}"));
            let result = session.accepted_result();
            assert_eq!(result.core_report.hard_validity, HardValidity::Valid);
            assert!(result.core_report.hard_residuals_validated);
            assert!(result.acceptance_hard_residual_max <= 1.0e-9);
            assert_eq!(result.core_report.rank, 6);
            assert_eq!(result.core_report.left_nullity, 0);
            assert_eq!(result.core_report.right_nullity, 0);
            assert_eq!(session.gauge_report().gauge_dof, 0);
            assert_eq!(session.gauge_report().internal_mobility, 0);
            assert_eq!(
                result
                    .source_mappings
                    .iter()
                    .map(|mapping| mapping.source)
                    .collect::<Vec<_>>(),
                source_order
            );
            assert!(
                result.geometry.bodies.iter().all(|body| body
                    .pose
                    .ambient()
                    .iter()
                    .all(|value| value.is_finite()))
            );
            assert!(
                result.geometry.points.iter().all(|point| point
                    .world
                    .coords
                    .iter()
                    .all(|value| value.is_finite()))
            );
            assert!(
                result
                    .geometry
                    .frames
                    .iter()
                    .map(|feature| feature.world)
                    .chain(result.geometry.axes.iter().map(|feature| feature.world))
                    .chain(result.geometry.planes.iter().map(|feature| feature.world))
                    .all(frame_is_finite)
            );
            assert!(
                result
                    .coordinate_values
                    .iter()
                    .all(|coordinate| match coordinate.value {
                        SpatialCoordinateValueKind::Hinge(value) => {
                            value.principal_phase.is_finite()
                        }
                        SpatialCoordinateValueKind::AxialTranslation(value)
                        | SpatialCoordinateValueKind::PlanarTranslation { value, .. } =>
                            value.is_finite(),
                    })
            );
            assert!(
                result
                    .mode_evaluations
                    .iter()
                    .all(|evaluation| evaluation.retained
                        && evaluation.retained_normalized_metric.is_finite()
                        && evaluation.fresh_raw_metric.is_some_and(f64::is_finite))
            );
            assert!(
                result
                    .display_audit
                    .sources
                    .iter()
                    .all(|source| source.rows.iter().all(|row| row.evaluation_status
                        == AuditEvaluationStatus::Evaluated
                        && row.scale.is_finite()
                        && row.raw_residual.is_finite()
                        && row.normalized_residual.is_finite()))
            );

            match ids {
                SpatialExampleIds::ShaftBearing(ids) => {
                    assert!(matches!(
                        session.assembly().source(ids.joint).unwrap().kind(),
                        SpatialSourceKind::CylindricalJoint {
                            first,
                            second,
                            parity: SpatialAxisParity::Aligned,
                        } if [first, second] == ids.axes
                    ));
                    assert!(matches!(
                        session.assembly().coordinate(ids.coordinates[0]).unwrap().kind(),
                        SpatialCoordinateKind::Hinge { parent, winding: 2 } if parent == ids.joint
                    ));
                    assert!(matches!(
                        session.assembly().coordinate(ids.coordinates[1]).unwrap().kind(),
                        SpatialCoordinateKind::AxialTranslation { parent } if parent == ids.joint
                    ));
                    assert!(matches!(
                        session.assembly().source(ids.drivers[0]).unwrap().kind(),
                        SpatialSourceKind::HingePositionDriver {
                            coordinate,
                            target: SpatialHingeTarget {
                                principal_phase: 0.48,
                                winding: 2,
                            },
                        } if coordinate == ids.coordinates[0]
                    ));
                    assert!(matches!(
                        session.assembly().source(ids.drivers[1]).unwrap().kind(),
                        SpatialSourceKind::TranslationPositionDriver { coordinate, target }
                            if coordinate == ids.coordinates[1]
                                && target.to_bits() == (1.9 * scale).to_bits()
                    ));
                    assert!(matches!(
                        session
                            .assembly()
                            .mode_monitor(ids.monitors[0])
                            .unwrap()
                            .kind(),
                        SpatialModeMonitorKind::AxisParity {
                            parity: SpatialAxisParity::Aligned,
                            ..
                        }
                    ));
                    assert!(matches!(
                        session.assembly().mode_monitor(ids.monitors[1]).unwrap().kind(),
                        SpatialModeMonitorKind::HingeWinding { coordinate, winding: 2 }
                            if coordinate == ids.coordinates[0]
                    ));
                    assert!(matches!(
                        session.assembly().mode_monitor(ids.monitors[2]).unwrap().kind(),
                        SpatialModeMonitorKind::PlaneSide {
                            plane,
                            point,
                            side: SpatialModeSign::Positive,
                        } if plane == ids.translation_plane && point == ids.translation_witness
                    ));
                    let SpatialCoordinateValueKind::Hinge(hinge) =
                        session.coordinate_value(ids.coordinates[0]).unwrap().value
                    else {
                        panic!("shaft hinge coordinate expected");
                    };
                    assert!((hinge.principal_phase - 0.48).abs() <= 2.0e-12);
                    assert_eq!(hinge.winding, 2);
                    let SpatialCoordinateValueKind::AxialTranslation(translation) =
                        session.coordinate_value(ids.coordinates[1]).unwrap().value
                    else {
                        panic!("shaft axial coordinate expected");
                    };
                    assert!((translation / scale - 1.9).abs() <= 2.0e-9);
                    let side = session.mode_evaluation(ids.monitors[2]).unwrap();
                    assert!((side.fresh_raw_metric.unwrap() / scale - 1.9).abs() <= 2.0e-9);
                }
                SpatialExampleIds::BlockBase(ids) => {
                    assert!(matches!(
                        session.assembly().source(ids.joint).unwrap().kind(),
                        SpatialSourceKind::PlanarJoint {
                            first,
                            second,
                            parity: SpatialAxisParity::Aligned,
                        } if [first, second] == ids.planes
                    ));
                    assert!(matches!(
                        session.assembly().coordinate(ids.coordinates[0]).unwrap().kind(),
                        SpatialCoordinateKind::Hinge { parent, winding: 3 } if parent == ids.joint
                    ));
                    for (coordinate, axis) in [
                        (ids.coordinates[1], SpatialPlanarTranslationAxis::X),
                        (ids.coordinates[2], SpatialPlanarTranslationAxis::Y),
                    ] {
                        assert!(matches!(
                            session.assembly().coordinate(coordinate).unwrap().kind(),
                            SpatialCoordinateKind::PlanarTranslation {
                                parent,
                                axis: actual,
                            } if parent == ids.joint && actual == axis
                        ));
                    }
                    assert!(matches!(
                        session.assembly().source(ids.drivers[0]).unwrap().kind(),
                        SpatialSourceKind::HingePositionDriver {
                            coordinate,
                            target: SpatialHingeTarget {
                                principal_phase: 0.37,
                                winding: 3,
                            },
                        } if coordinate == ids.coordinates[0]
                    ));
                    for (driver, coordinate, target) in [
                        (ids.drivers[1], ids.coordinates[1], 1.25 * scale),
                        (ids.drivers[2], ids.coordinates[2], -0.8 * scale),
                    ] {
                        assert!(matches!(
                            session.assembly().source(driver).unwrap().kind(),
                            SpatialSourceKind::TranslationPositionDriver {
                                coordinate: actual,
                                target: actual_target,
                            } if actual == coordinate
                                && actual_target.to_bits() == target.to_bits()
                        ));
                    }
                    assert!(matches!(
                        session
                            .assembly()
                            .mode_monitor(ids.monitors[0])
                            .unwrap()
                            .kind(),
                        SpatialModeMonitorKind::AxisParity {
                            parity: SpatialAxisParity::Aligned,
                            ..
                        }
                    ));
                    assert!(matches!(
                        session.assembly().mode_monitor(ids.monitors[1]).unwrap().kind(),
                        SpatialModeMonitorKind::HingeWinding { coordinate, winding: 3 }
                            if coordinate == ids.coordinates[0]
                    ));
                    assert!(matches!(
                        session.assembly().mode_monitor(ids.monitors[2]).unwrap().kind(),
                        SpatialModeMonitorKind::PlaneSide {
                            plane,
                            point,
                            side: SpatialModeSign::Positive,
                        } if plane == ids.planes[0] && point == ids.side_witness
                    ));
                    assert_hinge_value(&session, ids.coordinates[0], 0.37, 3);
                    assert_planar_translation_value(
                        &session,
                        ids.coordinates[1],
                        SpatialPlanarTranslationAxis::X,
                        1.25 * scale,
                        2.0e-9 * scale,
                    );
                    assert_planar_translation_value(
                        &session,
                        ids.coordinates[2],
                        SpatialPlanarTranslationAxis::Y,
                        -0.8 * scale,
                        2.0e-9 * scale,
                    );
                }
            }
        }
    }
}

fn frame_is_finite(frame: Frame3) -> bool {
    frame
        .origin()
        .coords
        .iter()
        .chain(frame.x_axis().iter())
        .chain(frame.y_axis().iter())
        .chain(frame.z_axis().iter())
        .all(|value| value.is_finite())
}

fn spatial_example_order(assembly: &SpatialAssembly) -> Vec<(&'static str, u64, String)> {
    let mut order = Vec::new();
    order.extend(
        assembly
            .bodies()
            .iter()
            .map(|item| ("body", item.id().as_u64(), item.label().to_owned())),
    );
    order.extend(
        assembly
            .point_features()
            .iter()
            .map(|item| ("point", item.id().as_u64(), item.label().to_owned())),
    );
    order.extend(
        assembly
            .frame_features()
            .iter()
            .map(|item| ("frame", item.id().as_u64(), item.label().to_owned())),
    );
    order.extend(
        assembly
            .axis_features()
            .iter()
            .map(|item| ("axis", item.id().as_u64(), item.label().to_owned())),
    );
    order.extend(
        assembly
            .plane_features()
            .iter()
            .map(|item| ("plane", item.id().as_u64(), item.label().to_owned())),
    );
    order.extend(
        assembly
            .coordinates()
            .iter()
            .map(|item| ("coordinate", item.id().as_u64(), item.label().to_owned())),
    );
    order.extend(
        assembly
            .mode_monitors()
            .iter()
            .map(|item| ("monitor", item.id().as_u64(), item.label().to_owned())),
    );
    order.extend(
        assembly
            .sources()
            .iter()
            .map(|item| ("source", item.id().as_u64(), item.label().to_owned())),
    );
    order.sort_by_key(|item| item.1);
    order
}

fn spatial_example_id_ordinals(ids: &SpatialExampleIds) -> Vec<u64> {
    match ids {
        SpatialExampleIds::ShaftBearing(ids) => vec![
            ids.bodies[0].as_u64(),
            ids.bodies[1].as_u64(),
            ids.frames[0].as_u64(),
            ids.frames[1].as_u64(),
            ids.axes[0].as_u64(),
            ids.axes[1].as_u64(),
            ids.translation_plane.as_u64(),
            ids.translation_witness.as_u64(),
            ids.joint.as_u64(),
            ids.coordinates[0].as_u64(),
            ids.coordinates[1].as_u64(),
            ids.drivers[0].as_u64(),
            ids.drivers[1].as_u64(),
            ids.monitors[0].as_u64(),
            ids.monitors[1].as_u64(),
            ids.monitors[2].as_u64(),
        ],
        SpatialExampleIds::BlockBase(ids) => vec![
            ids.bodies[0].as_u64(),
            ids.bodies[1].as_u64(),
            ids.frames[0].as_u64(),
            ids.frames[1].as_u64(),
            ids.axes[0].as_u64(),
            ids.axes[1].as_u64(),
            ids.planes[0].as_u64(),
            ids.planes[1].as_u64(),
            ids.side_witness.as_u64(),
            ids.joint.as_u64(),
            ids.coordinates[0].as_u64(),
            ids.coordinates[1].as_u64(),
            ids.coordinates[2].as_u64(),
            ids.drivers[0].as_u64(),
            ids.drivers[1].as_u64(),
            ids.drivers[2].as_u64(),
            ids.monitors[0].as_u64(),
            ids.monitors[1].as_u64(),
            ids.monitors[2].as_u64(),
        ],
    }
}
