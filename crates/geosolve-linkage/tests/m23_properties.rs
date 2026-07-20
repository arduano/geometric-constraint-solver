// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_core::SolverConfig;
use geosolve_geometry::Pose3;
use geosolve_linkage::{
    SpatialAssemblyDocument, SpatialAssemblyDocumentSession, SpatialAssemblySession,
    SpatialDocumentId, SpatialDriverRate, SpatialVelocityOutcome, embedded_spatial_slider_crank,
};
use proptest::prelude::*;

const DOCUMENT_ID: SpatialDocumentId =
    SpatialDocumentId::from_u128(0x7a11_0000_0000_0000_0000_0000_0000_0023);

proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]

    #[test]
    fn embedded_slider_crank_is_se3_scale_and_persistence_equivariant(
        scale_exponent in -6_i32..7,
        phase_milli in prop_oneof![-1_200_i32..-200, 200_i32..1_200],
        tx in -8_i32..9,
        ty in -8_i32..9,
        tz in -8_i32..9,
        rx_milli in -500_i32..501,
        ry_milli in -500_i32..501,
        rz_milli in -500_i32..501,
    ) {
        let scale = 10.0_f64.powi(scale_exponent);
        let phase = f64::from(phase_milli) * 1.0e-3;
        let embedding = Pose3::exp([
            f64::from(tx) * scale,
            f64::from(ty) * scale,
            f64::from(tz) * scale,
            f64::from(rx_milli) * 1.0e-3,
            f64::from(ry_milli) * 1.0e-3,
            f64::from(rz_milli) * 1.0e-3,
        ]).unwrap();
        let base_fixture = embedded_spatial_slider_crank(scale, Pose3::identity(), phase).unwrap();
        let embedded_fixture = embedded_spatial_slider_crank(scale, embedding, phase).unwrap();
        let base_ids = base_fixture.ids;
        let embedded_ids = embedded_fixture.ids;
        let base = SpatialAssemblySession::new(base_fixture.assembly, SolverConfig::default()).unwrap();
        let embedded = SpatialAssemblySession::new(
            embedded_fixture.assembly,
            SolverConfig::default(),
        ).unwrap();

        prop_assert_eq!(base.accepted_result().core_report.rank, embedded.accepted_result().core_report.rank);
        prop_assert!(embedded.accepted_result().acceptance_hard_residual_max <= 1.0e-9);
        prop_assert!(embedded.mode_evaluations().iter().all(|mode| mode.retained));
        for (base_body, embedded_body) in base_ids.bodies.into_iter().zip(embedded_ids.bodies) {
            let expected = embedding.compose(
                &base.accepted_result().geometry.body_pose(base_body).unwrap(),
            ).unwrap();
            let actual = embedded.accepted_result().geometry.body_pose(embedded_body).unwrap();
            let difference = expected.local_difference(&actual).unwrap();
            prop_assert!(difference[..3].iter().all(|value| value.abs() / scale <= 2.0e-8));
            prop_assert!(difference[3..].iter().all(|value| value.abs() <= 2.0e-8));
        }

        let rate = 0.37 * scale;
        let SpatialVelocityOutcome::Determinate(base_velocity) = base.velocity(
            base.revision(),
            &[SpatialDriverRate { source: base_ids.driver, rate }],
        ).unwrap() else {
            prop_assert!(false, "base velocity was not determinate");
            return Ok(());
        };
        let SpatialVelocityOutcome::Determinate(embedded_velocity) = embedded.velocity(
            embedded.revision(),
            &[SpatialDriverRate { source: embedded_ids.driver, rate }],
        ).unwrap() else {
            prop_assert!(false, "embedded velocity was not determinate");
            return Ok(());
        };
        for (base_body, embedded_body) in base_ids.bodies.into_iter().zip(embedded_ids.bodies) {
            let base_body_velocity = base_velocity.body(base_body).unwrap();
            let embedded_body_velocity = embedded_velocity.body(embedded_body).unwrap();
            let expected_linear = embedding
                .try_transform_vector(base_body_velocity.origin_linear_world)
                .unwrap();
            let expected_angular = embedding
                .try_transform_vector(base_body_velocity.angular_world)
                .unwrap();
            prop_assert!((expected_linear - embedded_body_velocity.origin_linear_world).norm() / scale <= 3.0e-8);
            prop_assert!((expected_angular - embedded_body_velocity.angular_world).norm() <= 3.0e-8);
        }

        let persistent = SpatialAssemblyDocumentSession::from_accepted_session(DOCUMENT_ID, &embedded).unwrap();
        let json = persistent.to_json().unwrap();
        let restored = SpatialAssemblyDocumentSession::from_json(&json, SolverConfig::default()).unwrap();
        prop_assert_eq!(restored.to_json().unwrap(), json);
        prop_assert_eq!(restored.accepted_result().core_report.rank, embedded.accepted_result().core_report.rank);
    }

    #[test]
    fn mutated_spatial_document_bytes_never_panic_or_publish_unvalidated_success(
        byte_index in any::<usize>(),
        replacement in any::<u8>(),
    ) {
        let fixture = embedded_spatial_slider_crank(1.0, Pose3::identity(), 0.73).unwrap();
        let accepted = SpatialAssemblySession::new(fixture.assembly, SolverConfig::default()).unwrap();
        let persistent = SpatialAssemblyDocumentSession::from_accepted_session(DOCUMENT_ID, &accepted).unwrap();
        let mut bytes = persistent.to_json().unwrap().into_bytes();
        let index = byte_index % bytes.len();
        bytes[index] = replacement;
        let mutated = String::from_utf8_lossy(&bytes);
        if let Ok(document) = SpatialAssemblyDocument::from_json(&mutated)
            && let Ok(restored) = SpatialAssemblyDocumentSession::new(document, SolverConfig::default())
        {
            prop_assert!(restored.accepted_result().acceptance_hard_residual_max.is_finite());
            prop_assert!(restored.accepted_result().acceptance_hard_residual_max <= 1.0e-9);
            prop_assert!(restored.accepted_result().core_report.hard_residual_max.is_finite());
        }
    }
}
