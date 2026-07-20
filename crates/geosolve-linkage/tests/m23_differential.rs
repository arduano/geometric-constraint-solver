// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_core::SolverConfig;
use geosolve_geometry::{Pose3, Vector3};
use geosolve_linkage::{
    SpatialAssemblySession, SpatialCoordinateRateKind, SpatialDriverRate, SpatialVelocityOutcome,
    embedded_spatial_slider_crank,
};

#[test]
fn spatial_slider_crank_matches_independent_position_and_velocity_oracle() {
    for scale in [1.0e-6, 1.0, 1.0e6] {
        let embeddings = [
            Pose3::identity(),
            Pose3::exp([2.3 * scale, -1.7 * scale, 0.8 * scale, 0.31, -0.27, 0.19]).unwrap(),
        ];
        for phase in [-1.1_f64, -0.8, -0.4, 0.35, 0.7, 1.05] {
            for embedding in embeddings {
                check_oracle_case(scale, phase, embedding);
            }
        }
    }
}

#[allow(clippy::too_many_lines)]
fn check_oracle_case(scale: f64, phase: f64, embedding: Pose3) {
    let radius = 1.25 * scale;
    let rod_length = 3.5 * scale;
    let crank_pin = Vector3::new(radius * phase.cos(), radius * phase.sin(), 0.0);
    let horizontal = (rod_length * rod_length - crank_pin.y * crank_pin.y).sqrt();
    let slider_x = crank_pin.x + horizontal;
    let rod_phase = (-crank_pin.y).atan2(horizontal);
    let planar_pose = |translation: Vector3<f64>, angle: f64| {
        let half = 0.5 * angle;
        Pose3::try_new(translation, [half.cos(), 0.0, 0.0, half.sin()]).unwrap()
    };
    let expected_poses = [
        embedding,
        embedding
            .compose(&planar_pose(Vector3::zeros(), phase))
            .unwrap(),
        embedding
            .compose(&planar_pose(crank_pin, rod_phase))
            .unwrap(),
        embedding
            .compose(&planar_pose(Vector3::new(slider_x, 0.0, 0.0), 0.0))
            .unwrap(),
    ];

    let fixture = embedded_spatial_slider_crank(scale, embedding, phase).unwrap();
    let ids = fixture.ids;
    let session = SpatialAssemblySession::new(fixture.assembly, SolverConfig::default()).unwrap();
    for (body, expected) in ids.bodies.into_iter().zip(expected_poses) {
        let actual = session.accepted_result().geometry.body_pose(body).unwrap();
        let difference = expected.local_difference(&actual).unwrap();
        assert!(
            difference[..3]
                .iter()
                .all(|value| value.abs() / scale <= 2.0e-8)
        );
        assert!(difference[3..].iter().all(|value| value.abs() <= 2.0e-8));
    }

    let driver_rate = 0.23 * scale;
    let dx_dphase =
        -radius * phase.sin() - radius * radius * phase.sin() * phase.cos() / horizontal;
    let phase_rate = driver_rate / dx_dphase;
    let rod_phase_rate = -radius * phase.cos() / horizontal * phase_rate;
    let crank_origin_rate = Vector3::zeros();
    let crank_pin_rate = Vector3::new(
        -radius * phase.sin() * phase_rate,
        radius * phase.cos() * phase_rate,
        0.0,
    );
    let slider_rate = Vector3::new(driver_rate, 0.0, 0.0);
    let world_z = embedding.try_transform_vector(Vector3::z()).unwrap();

    let SpatialVelocityOutcome::Determinate(velocity) = session
        .velocity(
            session.revision(),
            &[SpatialDriverRate {
                source: ids.driver,
                rate: driver_rate,
            }],
        )
        .unwrap()
    else {
        panic!("regular driven slider-crank velocity must be determinate");
    };
    let expected_linear = [
        Vector3::zeros(),
        embedding.try_transform_vector(crank_origin_rate).unwrap(),
        embedding.try_transform_vector(crank_pin_rate).unwrap(),
        embedding.try_transform_vector(slider_rate).unwrap(),
    ];
    let expected_angular = [
        Vector3::zeros(),
        world_z * phase_rate,
        world_z * rod_phase_rate,
        Vector3::zeros(),
    ];
    for ((body, linear), angular) in ids
        .bodies
        .into_iter()
        .zip(expected_linear)
        .zip(expected_angular)
    {
        let actual = velocity.body(body).unwrap();
        assert!((actual.origin_linear_world - linear).norm() / scale <= 3.0e-8);
        assert!((actual.angular_world - angular).norm() <= 3.0e-8);
    }
    let hinge_rate = velocity
        .coordinate_rates
        .iter()
        .find(|rate| rate.coordinate == ids.crank_hinge)
        .unwrap();
    assert!(matches!(
        hinge_rate.rate,
        SpatialCoordinateRateKind::Hinge { principal_phase_rate }
            if (principal_phase_rate - phase_rate).abs() <= 3.0e-8
    ));
    let translation_rate = velocity
        .coordinate_rates
        .iter()
        .find(|rate| rate.coordinate == ids.slider_translation)
        .unwrap();
    assert!(matches!(
        translation_rate.rate,
        SpatialCoordinateRateKind::AxialTranslation(rate)
            if (rate - driver_rate).abs() / scale <= 3.0e-8
    ));
    let slider_pin_rate = velocity
        .point_velocities
        .iter()
        .find(|item| item.feature_id == ids.points[3])
        .unwrap();
    assert!((slider_pin_rate.linear_world - expected_linear[3]).norm() / scale <= 3.0e-8);
    assert!(velocity.differentiated_residual_max <= 1.0e-9);
}
