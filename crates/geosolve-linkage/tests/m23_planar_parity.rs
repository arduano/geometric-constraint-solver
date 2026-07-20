// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_core::{AdaptiveStepPolicy, SolverConfig};
use geosolve_geometry::{PlaneFrame, Point3, Pose2, Pose3, Vector2, Vector3};
use geosolve_linkage::{
    AdaptiveContinuationMode, AdaptiveContinuationRequest, AdaptiveContinuationStatus,
    LinkageSource, PlanarDocumentId, PlanarLinkageDocument, PlanarLinkageSession,
    SpatialAdaptiveContinuationRequest, SpatialAdaptiveContinuationStatus, SpatialAssemblySession,
    SpatialCoordinateRateKind, SpatialDriverRate, SpatialVelocityOutcome,
    embedded_spatial_slider_crank, slider_crank_displacement_driven_with_scale,
};

#[test]
#[allow(clippy::too_many_lines)]
fn embedded_slider_crank_matches_planar_position_and_velocity_oracles() {
    for scale in [1.0e-6, 1.0, 1.0e6] {
        let embedding = embedding(scale);
        let plane = PlaneFrame::try_new(
            Point3::from(embedding.translation()),
            embedding.try_transform_vector(Vector3::x()).unwrap(),
            embedding.try_transform_vector(Vector3::y()).unwrap(),
        )
        .unwrap();
        let mut planar = slider_crank_displacement_driven_with_scale(scale).unwrap();
        planar.0.set_plane_frame(plane).unwrap();
        let spatial_fixture = embedded_spatial_slider_crank(scale, embedding, 0.05).unwrap();
        let planar_ids = planar.1;
        let spatial_ids = spatial_fixture.ids;
        let mut spatial =
            SpatialAssemblySession::new(spatial_fixture.assembly, SolverConfig::default()).unwrap();

        let target = 4.70 * scale;
        let planar_path = planar
            .0
            .continue_driver(
                AdaptiveContinuationRequest {
                    driver_id: planar_ids.driver,
                    mode: AdaptiveContinuationMode::Natural { target },
                    step_policy: policy(),
                },
                SolverConfig::default(),
            )
            .unwrap();
        let spatial_path = spatial
            .continue_driver(
                spatial.revision(),
                SpatialAdaptiveContinuationRequest {
                    driver_source: spatial_ids.driver,
                    mode: AdaptiveContinuationMode::Natural { target },
                    step_policy: policy(),
                },
            )
            .unwrap();
        assert_eq!(planar_path.status, AdaptiveContinuationStatus::Completed);
        assert_eq!(
            spatial_path.status,
            SpatialAdaptiveContinuationStatus::Completed
        );
        let planar_solve = &planar_path.samples.last().unwrap().solve;
        let spatial_solve = spatial.accepted_result();
        assert!(planar_solve.accepted());
        assert_eq!(planar_solve.core_report.rank, 9);
        assert_eq!(planar_solve.core_report.right_nullity, 0);
        assert_eq!(spatial_solve.core_report.rank, 18);
        assert_eq!(spatial_solve.core_report.right_nullity, 0);
        assert_eq!(spatial.gauge_report().internal_mobility, 0);
        assert!(
            spatial_solve
                .mode_evaluations
                .iter()
                .all(|evaluation| evaluation.retained)
        );

        let planar_bodies = [
            planar_ids.ground,
            planar_ids.crank,
            planar_ids.rod,
            planar_ids.slider,
        ];
        for (planar_body, spatial_body) in planar_bodies.into_iter().zip(spatial_ids.bodies) {
            let pose = planar_solve.geometry.body_pose(planar_body).unwrap();
            let expected = embedding.compose(&lift_pose(pose)).unwrap();
            let actual = spatial_solve.geometry.body_pose(spatial_body).unwrap();
            let difference = expected.local_difference(&actual).unwrap();
            assert!(
                difference[..3]
                    .iter()
                    .all(|value| value.abs() / scale <= 2.0e-8)
            );
            assert!(difference[3..].iter().all(|value| value.abs() <= 2.0e-8));
        }
        let planar_points = [
            planar_ids.crank_a,
            planar_ids.rod_a,
            planar_ids.rod_slider,
            planar_ids.slider_pin,
        ];
        for (planar_point, spatial_point) in planar_points.into_iter().zip(spatial_ids.points) {
            let expected = planar_solve.geometry.world_point(planar_point).unwrap();
            let actual = spatial_solve.geometry.world_point(spatial_point).unwrap();
            assert!((actual - expected).norm() / scale <= 2.0e-8);
        }

        let driver_rate = -0.4 * scale;
        let compatibility_velocity = planar.0.velocity(planar_ids.driver, driver_rate).unwrap();
        let (document, captured) = PlanarLinkageDocument::from_linkage(
            PlanarDocumentId::from_u128(0x2300_0000 + u128::from(scale.to_bits())),
            &planar.0,
        )
        .unwrap();
        let persistent_driver = captured
            .persistent_source(LinkageSource::Driver(planar_ids.driver))
            .unwrap();
        let persistent = PlanarLinkageSession::new(document, SolverConfig::default()).unwrap();
        let persistent_velocity = persistent.velocity(persistent_driver, driver_rate).unwrap();
        let spatial_velocity = spatial
            .velocity(
                spatial.revision(),
                &[SpatialDriverRate {
                    source: spatial_ids.driver,
                    rate: driver_rate,
                }],
            )
            .unwrap();
        let SpatialVelocityOutcome::Determinate(spatial_velocity) = spatial_velocity else {
            panic!("embedded spatial velocity was not determinate: {spatial_velocity:#?}");
        };
        assert!(compatibility_velocity.differentiated_residual_max <= 1.0e-9);
        assert!(persistent_velocity.differentiated_residual_max <= 1.0e-9);
        assert!(spatial_velocity.differentiated_residual_max <= 1.0e-9);

        let basis_u = plane.u();
        let basis_v = plane.v();
        let normal = plane.normal();
        for (planar_body, spatial_body) in planar_bodies.into_iter().zip(spatial_ids.bodies) {
            let planar_velocity = compatibility_velocity.body(planar_body).unwrap();
            let expected_linear =
                basis_u * planar_velocity.linear.x + basis_v * planar_velocity.linear.y;
            let expected_angular = normal * planar_velocity.angular;
            let actual = spatial_velocity.body(spatial_body).unwrap();
            assert!((actual.origin_linear_world - expected_linear).norm() / scale <= 2.0e-7);
            assert!((actual.angular_world - expected_angular).norm() <= 2.0e-7);

            let persistent_body = captured.persistent_body(planar_body).unwrap();
            let persistent_runtime = persistent
                .runtime_map()
                .runtime_body(persistent_body)
                .unwrap();
            let persistent_body_velocity = persistent_velocity.body(persistent_runtime).unwrap();
            assert!(
                (persistent_body_velocity.linear - planar_velocity.linear).norm() / scale <= 2.0e-9
            );
            assert!((persistent_body_velocity.angular - planar_velocity.angular).abs() <= 2.0e-9);
        }

        for (planar_point, spatial_point) in planar_points.into_iter().zip(spatial_ids.points) {
            let feature = planar.0.point_feature(planar_point).unwrap();
            let body_pose = planar_solve.geometry.body_pose(feature.body()).unwrap();
            let body_velocity = compatibility_velocity.body(feature.body()).unwrap();
            let rotated = rotate(body_pose.angle, feature.local_point().coords);
            let planar_point_velocity =
                body_velocity.linear + Vector2::new(-rotated.y, rotated.x) * body_velocity.angular;
            let expected = basis_u * planar_point_velocity.x + basis_v * planar_point_velocity.y;
            let actual = spatial_velocity
                .point_velocities
                .iter()
                .find(|velocity| velocity.feature_id == spatial_point)
                .unwrap()
                .linear_world;
            assert!((actual - expected).norm() / scale <= 2.0e-7);
        }
        let translation_rate = spatial_velocity
            .coordinate_rates
            .iter()
            .find(|rate| rate.coordinate == spatial_ids.slider_translation)
            .unwrap();
        assert!(matches!(
            translation_rate.rate,
            SpatialCoordinateRateKind::AxialTranslation(rate)
                if (rate - driver_rate).abs() / scale <= 2.0e-9
        ));
        let hinge_rate = spatial_velocity
            .coordinate_rates
            .iter()
            .find(|rate| rate.coordinate == spatial_ids.crank_hinge)
            .unwrap();
        let expected_hinge_rate = compatibility_velocity
            .body(planar_ids.crank)
            .unwrap()
            .angular
            - compatibility_velocity
                .body(planar_ids.ground)
                .unwrap()
                .angular;
        assert!(matches!(
            hinge_rate.rate,
            SpatialCoordinateRateKind::Hinge {
                principal_phase_rate
            } if (principal_phase_rate - expected_hinge_rate).abs() <= 2.0e-7
        ));
    }
}

fn policy() -> AdaptiveStepPolicy {
    AdaptiveStepPolicy {
        initial_step: 0.02,
        minimum_step: 1.0e-6,
        maximum_step: 0.04,
        growth_factor: 1.4,
        shrink_factor: 0.5,
        fast_iterations: 4,
        slow_iterations: 12,
        small_correction: 0.05,
        large_correction: 0.5,
        maximum_correction: 0.25,
        maximum_correction_step_ratio: 1.0,
        max_retries: 16,
        max_samples: 1_000,
    }
}

fn embedding(scale: f64) -> Pose3 {
    let rotation = Pose3::exp([0.0, 0.0, 0.0, 0.37, -0.29, 0.41]).unwrap();
    Pose3::try_new(
        Vector3::new(2.3 * scale, -1.7 * scale, 0.8 * scale),
        rotation.quaternion(),
    )
    .unwrap()
}

fn lift_pose(pose: Pose2) -> Pose3 {
    let half = 0.5 * pose.angle;
    Pose3::try_new(
        Vector3::new(pose.translation.x, pose.translation.y, 0.0),
        [half.cos(), 0.0, 0.0, half.sin()],
    )
    .unwrap()
}

fn rotate(angle: f64, vector: Vector2<f64>) -> Vector2<f64> {
    let (sine, cosine) = angle.sin_cos();
    Vector2::new(
        cosine * vector.x - sine * vector.y,
        sine * vector.x + cosine * vector.y,
    )
}
