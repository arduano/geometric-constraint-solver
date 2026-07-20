// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_core::SolverConfig;
use geosolve_geometry::{Frame3, Point3, Pose3, Vector3};
use geosolve_linkage::{
    SpatialAssembly, SpatialAssemblyEdit, SpatialAssemblySession, SpatialAssemblyTransaction,
    SpatialAxisParity, SpatialCoordinateRateKind, SpatialDriverRate, SpatialExampleIds,
    SpatialExampleKind, SpatialGaugePolicy, SpatialHingeTarget, SpatialSourceId,
    SpatialVelocityOptions, SpatialVelocityOutcome, spatial_example,
};

const TOLERANCE: f64 = 1.0e-9;

#[test]
fn simultaneous_shaft_rates_are_determinate_scale_stable_and_order_independent() {
    for scale in [1.0e-6, 1.0, 1.0e6] {
        let fixture = spatial_example(SpatialExampleKind::ShaftBearing, scale).unwrap();
        let SpatialExampleIds::ShaftBearing(ids) = fixture.ids else {
            unreachable!();
        };
        let session =
            SpatialAssemblySession::new(fixture.assembly, SolverConfig::default()).unwrap();
        let rates = [
            SpatialDriverRate {
                source: ids.drivers[1],
                rate: -0.4 * scale,
            },
            SpatialDriverRate {
                source: ids.drivers[0],
                rate: 0.7,
            },
        ];
        let outcome = session.velocity(session.revision(), &rates).unwrap();
        let SpatialVelocityOutcome::Determinate(solution) = outcome else {
            panic!("expected determinate shaft velocity at scale {scale:e}: {outcome:#?}");
        };
        assert_eq!(solution.driver_rates[0].source, ids.drivers[0]);
        assert_eq!(solution.driver_rates[1].source, ids.drivers[1]);
        assert_eq!(solution.gauge_dof, 0);
        assert_eq!(solution.internal_mobility, 0);
        assert!(solution.differentiated_residual_max <= TOLERANCE);
        assert_zero_body_velocity(solution.body(ids.bodies[0]).unwrap(), scale);
        let shaft = solution.body(ids.bodies[1]).unwrap();
        assert!(shaft.angular_world.norm() > 0.6);
        assert!(shaft.origin_linear_world.norm() / scale > 0.1);
        assert_eq!(solution.frame_velocities.len(), 2);
        assert_eq!(solution.axis_velocities.len(), 2);
        assert_eq!(solution.plane_velocities.len(), 1);
        assert_eq!(solution.point_velocities.len(), 1);
        let hinge_rate = solution
            .coordinate_rates
            .iter()
            .find(|rate| rate.coordinate == ids.coordinates[0])
            .unwrap();
        assert!(matches!(
            hinge_rate.rate,
            SpatialCoordinateRateKind::Hinge {
                principal_phase_rate
            } if (principal_phase_rate - 0.7).abs() <= 2.0e-9
        ));
        let translation_rate = solution
            .coordinate_rates
            .iter()
            .find(|rate| rate.coordinate == ids.coordinates[1])
            .unwrap();
        assert!(matches!(
            translation_rate.rate,
            SpatialCoordinateRateKind::AxialTranslation(rate)
                if (rate + 0.4 * scale).abs() / scale <= 2.0e-9
        ));
        assert_shaft_position_difference_oracle(&session, ids, &solution, scale);

        let reversed = session
            .velocity(session.revision(), &[rates[1], rates[0]])
            .unwrap();
        assert_eq!(reversed, SpatialVelocityOutcome::Determinate(solution));
    }
}

#[test]
fn block_base_returns_every_feature_and_planar_coordinate_rate() {
    let fixture = spatial_example(SpatialExampleKind::BlockBase, 1.0).unwrap();
    let SpatialExampleIds::BlockBase(ids) = fixture.ids else {
        unreachable!();
    };
    let session = SpatialAssemblySession::new(fixture.assembly, SolverConfig::default()).unwrap();
    let outcome = session
        .velocity(
            session.revision(),
            &[
                SpatialDriverRate {
                    source: ids.drivers[2],
                    rate: -0.7,
                },
                SpatialDriverRate {
                    source: ids.drivers[0],
                    rate: 0.3,
                },
                SpatialDriverRate {
                    source: ids.drivers[1],
                    rate: 0.5,
                },
            ],
        )
        .unwrap();
    let SpatialVelocityOutcome::Determinate(solution) = outcome else {
        panic!("expected determinate block/base velocity: {outcome:#?}");
    };
    assert_eq!(solution.body_velocities.len(), 2);
    assert_eq!(solution.frame_velocities.len(), 2);
    assert_eq!(solution.axis_velocities.len(), 2);
    assert_eq!(solution.plane_velocities.len(), 2);
    assert_eq!(solution.point_velocities.len(), 1);
    assert_eq!(solution.coordinate_rates.len(), 3);
    assert!(matches!(
        solution.coordinate_rates[0].rate,
        SpatialCoordinateRateKind::Hinge {
            principal_phase_rate
        } if (principal_phase_rate - 0.3).abs() <= 2.0e-9
    ));
    assert!(matches!(
        solution.coordinate_rates[1].rate,
        SpatialCoordinateRateKind::PlanarTranslation { rate, .. }
            if (rate - 0.5).abs() <= 2.0e-9
    ));
    assert!(matches!(
        solution.coordinate_rates[2].rate,
        SpatialCoordinateRateKind::PlanarTranslation { rate, .. }
            if (rate + 0.7).abs() <= 2.0e-9
    ));
    assert!(solution.differentiated_residual_max <= TOLERANCE);
}

#[test]
fn floating_fully_driven_cylinder_is_determinate_modulo_six_dof_gauge() {
    let (assembly, ids) = cylindrical_velocity_fixture(false, 2);
    let session = SpatialAssemblySession::new(assembly, SolverConfig::default()).unwrap();
    let outcome = session
        .velocity(
            session.revision(),
            &[
                SpatialDriverRate {
                    source: ids.hinge_driver,
                    rate: -0.35,
                },
                SpatialDriverRate {
                    source: ids.translation_drivers[0],
                    rate: 0.6,
                },
                SpatialDriverRate {
                    source: ids.translation_drivers[1],
                    rate: 0.6,
                },
            ],
        )
        .unwrap();
    let SpatialVelocityOutcome::Determinate(solution) = outcome else {
        panic!("expected determinate floating velocity: {outcome:#?}");
    };
    assert_eq!(solution.gauge_dof, 6);
    assert_eq!(solution.internal_mobility, 0);
    assert_zero_body_velocity(solution.body(ids.bodies[0]).unwrap(), 1.0);
    assert!(solution.body(ids.bodies[1]).unwrap().angular_world.norm() > 0.3);
    assert!(solution.differentiated_residual_max <= TOLERANCE);
}

#[test]
fn one_cylindrical_driver_reports_remaining_internal_motion() {
    let (assembly, ids) = cylindrical_velocity_fixture(true, 0);
    let session = SpatialAssemblySession::new(assembly, SolverConfig::default()).unwrap();
    let outcome = session
        .velocity(
            session.revision(),
            &[SpatialDriverRate {
                source: ids.hinge_driver,
                rate: 0.25,
            }],
        )
        .unwrap();
    let SpatialVelocityOutcome::Underdetermined(solution) = outcome else {
        panic!("expected one remaining cylindrical motion: {outcome:#?}");
    };
    assert_eq!(solution.gauge_dof, 0);
    assert_eq!(solution.internal_mobility, 1);
    assert!(solution.differentiated_residual_max <= TOLERANCE);
}

#[test]
fn optional_motion_basis_uses_accepted_physical_nullity_and_is_deterministic() {
    let (assembly, ids) = cylindrical_velocity_fixture(true, 0);
    let session = SpatialAssemblySession::new(assembly, SolverConfig::default()).unwrap();
    let request = [SpatialDriverRate {
        source: ids.hinge_driver,
        rate: 0.25,
    }];
    let solve = || {
        session
            .velocity_with_options(
                session.revision(),
                &request,
                SpatialVelocityOptions {
                    include_motion_basis: true,
                },
            )
            .unwrap()
    };
    let first = solve();
    let second = solve();
    assert_eq!(first, second);
    let SpatialVelocityOutcome::Underdetermined(solution) = first else {
        panic!("expected underdetermined basis result: {first:#?}");
    };
    assert_eq!(solution.numerical_right_nullity, 1);
    assert_eq!(solution.motion_basis.len(), 1);
    let basis = &solution.motion_basis[0];
    let normalized_norm = basis
        .normalized_body_tangents
        .iter()
        .flat_map(|block| block.normalized)
        .fold(0.0_f64, f64::hypot);
    assert!((normalized_norm - 1.0).abs() <= 2.0e-12);
    assert!(basis.differentiated_residual_max <= TOLERANCE);
    assert_eq!(basis.coordinate_rates.len(), 2);
}

#[test]
fn floating_motion_basis_retains_all_six_physical_world_actions() {
    let (assembly, ids) = cylindrical_velocity_fixture(false, 2);
    let session = SpatialAssemblySession::new(assembly, SolverConfig::default()).unwrap();
    let outcome = session
        .velocity_with_options(
            session.revision(),
            &[
                SpatialDriverRate {
                    source: ids.hinge_driver,
                    rate: 0.2,
                },
                SpatialDriverRate {
                    source: ids.translation_drivers[0],
                    rate: -0.3,
                },
                SpatialDriverRate {
                    source: ids.translation_drivers[1],
                    rate: -0.3,
                },
            ],
            SpatialVelocityOptions {
                include_motion_basis: true,
            },
        )
        .unwrap();
    let SpatialVelocityOutcome::Determinate(solution) = outcome else {
        panic!("expected floating determinate basis result: {outcome:#?}");
    };
    assert_eq!(solution.gauge_dof, 6);
    assert_eq!(solution.numerical_right_nullity, 6);
    assert_eq!(solution.motion_basis.len(), 6);
    assert!(solution.motion_basis.iter().all(|basis| {
        basis.differentiated_residual_max <= TOLERANCE
            && basis.body_velocities.len() == 2
            && basis.axis_velocities.len() == 2
    }));
    assert!(solution.motion_basis.iter().any(|basis| {
        let reference = basis
            .body_velocities
            .iter()
            .find(|velocity| velocity.body_id == ids.bodies[0])
            .unwrap();
        reference.origin_linear_world.norm() > 0.1 || reference.angular_world.norm() > 0.1
    }));
}

#[test]
fn spatial_velocity_is_common_left_se3_equivariant() {
    let transform = Pose3::exp([4.0, -2.0, 3.0, 0.31, -0.27, 0.18]).unwrap();
    let (original_assembly, original_ids) =
        cylindrical_velocity_fixture_with_transform(true, 1, None);
    let (transformed_assembly, transformed_ids) =
        cylindrical_velocity_fixture_with_transform(true, 1, Some(transform));
    let original = SpatialAssemblySession::new(original_assembly, SolverConfig::default()).unwrap();
    let transformed =
        SpatialAssemblySession::new(transformed_assembly, SolverConfig::default()).unwrap();
    let rates = |ids: CylindricalVelocityIds| {
        [
            SpatialDriverRate {
                source: ids.hinge_driver,
                rate: 0.37,
            },
            SpatialDriverRate {
                source: ids.translation_drivers[0],
                rate: -0.42,
            },
        ]
    };
    let SpatialVelocityOutcome::Determinate(original_solution) = original
        .velocity(original.revision(), &rates(original_ids))
        .unwrap()
    else {
        panic!("original velocity was not determinate");
    };
    let SpatialVelocityOutcome::Determinate(transformed_solution) = transformed
        .velocity(transformed.revision(), &rates(transformed_ids))
        .unwrap()
    else {
        panic!("transformed velocity was not determinate");
    };
    for index in 0..2 {
        let original_velocity = original_solution.body(original_ids.bodies[index]).unwrap();
        let transformed_velocity = transformed_solution
            .body(transformed_ids.bodies[index])
            .unwrap();
        let expected_linear = transform
            .try_transform_vector(original_velocity.origin_linear_world)
            .unwrap();
        let expected_angular = transform
            .try_transform_vector(original_velocity.angular_world)
            .unwrap();
        assert!((transformed_velocity.origin_linear_world - expected_linear).norm() <= 2.0e-9);
        assert!((transformed_velocity.angular_world - expected_angular).norm() <= 2.0e-9);
    }
    for index in 0..2 {
        let original_axis = original_solution.axis_velocities[index];
        let transformed_axis = transformed_solution.axis_velocities[index];
        assert!(
            (transformed_axis.origin_linear_world
                - transform
                    .try_transform_vector(original_axis.origin_linear_world)
                    .unwrap())
            .norm()
                <= 2.0e-9
        );
        assert!(
            (transformed_axis.direction_rate_world
                - transform
                    .try_transform_vector(original_axis.direction_rate_world)
                    .unwrap())
            .norm()
                <= 2.0e-9
        );
    }
}

#[test]
fn alternative_velocity_references_differ_by_one_common_world_twist() {
    let (assembly, ids) = cylindrical_velocity_fixture(false, 1);
    let first_reference = SpatialAssemblySession::new(assembly, SolverConfig::default()).unwrap();
    let mut second_reference = first_reference.clone();
    second_reference
        .set_gauge_policy(
            second_reference.revision(),
            SpatialGaugePolicy::ExplicitReferences {
                bodies: vec![ids.bodies[1]],
            },
        )
        .unwrap();
    let rates = [
        SpatialDriverRate {
            source: ids.hinge_driver,
            rate: -0.23,
        },
        SpatialDriverRate {
            source: ids.translation_drivers[0],
            rate: 0.61,
        },
    ];
    let SpatialVelocityOutcome::Determinate(first) = first_reference
        .velocity(first_reference.revision(), &rates)
        .unwrap()
    else {
        panic!("first-reference velocity was not determinate");
    };
    let SpatialVelocityOutcome::Determinate(second) = second_reference
        .velocity(second_reference.revision(), &rates)
        .unwrap()
    else {
        panic!("second-reference velocity was not determinate");
    };
    assert_zero_body_velocity(first.body(ids.bodies[0]).unwrap(), 1.0);
    assert_zero_body_velocity(second.body(ids.bodies[1]).unwrap(), 1.0);
    let first_reference_pose = first_reference
        .accepted_result()
        .geometry
        .body_pose(ids.bodies[0])
        .unwrap();
    let common_linear = second.body(ids.bodies[0]).unwrap().origin_linear_world;
    let common_angular = second.body(ids.bodies[0]).unwrap().angular_world;
    for body in ids.bodies {
        let pose = first_reference
            .accepted_result()
            .geometry
            .body_pose(body)
            .unwrap();
        let expected_difference = common_linear
            + common_angular.cross(&(pose.translation() - first_reference_pose.translation()));
        let actual_difference = second.body(body).unwrap().origin_linear_world
            - first.body(body).unwrap().origin_linear_world;
        assert!((actual_difference - expected_difference).norm() <= 2.0e-9);
        assert!(
            (second.body(body).unwrap().angular_world
                - first.body(body).unwrap().angular_world
                - common_angular)
                .norm()
                <= 2.0e-9
        );
    }
}

#[test]
fn duplicate_driver_rates_distinguish_consistency_without_publishing_bad_velocity() {
    let (assembly, ids) = cylindrical_velocity_fixture(true, 2);
    let session = SpatialAssemblySession::new(assembly, SolverConfig::default()).unwrap();
    let equal = session
        .velocity(
            session.revision(),
            &[
                SpatialDriverRate {
                    source: ids.translation_drivers[0],
                    rate: -0.8,
                },
                SpatialDriverRate {
                    source: ids.translation_drivers[1],
                    rate: -0.8,
                },
                SpatialDriverRate {
                    source: ids.hinge_driver,
                    rate: 0.1,
                },
            ],
        )
        .unwrap();
    assert!(matches!(equal, SpatialVelocityOutcome::Determinate(_)));

    let inconsistent = session
        .velocity(
            session.revision(),
            &[
                SpatialDriverRate {
                    source: ids.translation_drivers[0],
                    rate: -0.8,
                },
                SpatialDriverRate {
                    source: ids.translation_drivers[1],
                    rate: 0.2,
                },
            ],
        )
        .unwrap();
    let SpatialVelocityOutcome::Inconsistent(inconsistency) = inconsistent else {
        panic!("expected inconsistent duplicate rates: {inconsistent:#?}");
    };
    assert!(!inconsistency.inconsistent_component_indices.is_empty());
    assert!(inconsistency.equation_residual_max > TOLERANCE);

    let implicit_zero = session
        .velocity(
            session.revision(),
            &[SpatialDriverRate {
                source: ids.translation_drivers[0],
                rate: 0.3,
            }],
        )
        .unwrap();
    assert!(matches!(
        implicit_zero,
        SpatialVelocityOutcome::Inconsistent(_)
    ));
}

#[test]
fn spatial_velocity_request_validation_is_read_only() {
    let (assembly, ids) = cylindrical_velocity_fixture(true, 2);
    let session = SpatialAssemblySession::new(assembly, SolverConfig::default()).unwrap();
    let revision = session.revision();
    let retained = session.accepted_result().clone();

    assert!(session.velocity(revision, &[]).is_err());
    assert!(
        session
            .velocity(
                revision,
                &[
                    SpatialDriverRate {
                        source: ids.hinge_driver,
                        rate: 0.0,
                    },
                    SpatialDriverRate {
                        source: ids.hinge_driver,
                        rate: 0.0,
                    },
                ],
            )
            .is_err()
    );
    assert!(
        session
            .velocity(
                revision,
                &[SpatialDriverRate {
                    source: ids.hinge_driver,
                    rate: f64::NAN,
                }],
            )
            .is_err()
    );
    assert!(
        session
            .velocity(
                revision + 1,
                &[SpatialDriverRate {
                    source: ids.hinge_driver,
                    rate: 0.0,
                }],
            )
            .is_err()
    );
    assert_eq!(session.revision(), revision);
    assert_eq!(session.accepted_result(), &retained);
}

#[derive(Clone, Copy)]
struct CylindricalVelocityIds {
    bodies: [geosolve_linkage::SpatialBodyId; 2],
    hinge_driver: SpatialSourceId,
    translation_drivers: [SpatialSourceId; 2],
}

fn cylindrical_velocity_fixture(
    grounded: bool,
    translation_driver_count: usize,
) -> (SpatialAssembly, CylindricalVelocityIds) {
    cylindrical_velocity_fixture_with_transform(grounded, translation_driver_count, None)
}

fn cylindrical_velocity_fixture_with_transform(
    grounded: bool,
    translation_driver_count: usize,
    common_left: Option<Pose3>,
) -> (SpatialAssembly, CylindricalVelocityIds) {
    let mut assembly = SpatialAssembly::new(1.0).unwrap();
    let first_pose = common_left.unwrap_or_else(Pose3::identity);
    let relative_pose = Pose3::exp([0.0, 0.0, 1.25, 0.0, 0.0, 0.4]).unwrap();
    let second_pose = common_left.map_or(relative_pose, |transform| {
        transform.compose(&relative_pose).unwrap()
    });
    let first = assembly.add_body("first", first_pose).unwrap();
    let second = assembly.add_body("second", second_pose).unwrap();
    let frame = identity_frame();
    let first_axis = assembly
        .add_axis_feature("first axis", first, frame)
        .unwrap();
    let second_axis = assembly
        .add_axis_feature("second axis", second, frame)
        .unwrap();
    if grounded {
        assembly.add_physical_ground("first fixed", first).unwrap();
    }
    let joint = assembly
        .add_cylindrical_joint(
            "cylinder",
            first_axis,
            second_axis,
            SpatialAxisParity::Aligned,
        )
        .unwrap();
    let hinge = assembly.add_hinge_coordinate("hinge", joint, 0).unwrap();
    let translation = assembly
        .add_axial_translation_coordinate("translation", joint)
        .unwrap();
    let hinge_driver = assembly
        .add_hinge_position_driver(
            "hinge driver",
            hinge,
            SpatialHingeTarget {
                principal_phase: 0.4,
                winding: 0,
            },
        )
        .unwrap();
    let first_translation_driver = if translation_driver_count > 0 {
        assembly
            .add_translation_position_driver("translation driver", translation, 1.25)
            .unwrap()
    } else {
        hinge_driver
    };
    let second_translation_driver = if translation_driver_count > 1 {
        assembly
            .add_translation_position_driver("duplicate translation driver", translation, 1.25)
            .unwrap()
    } else {
        first_translation_driver
    };
    (
        assembly,
        CylindricalVelocityIds {
            bodies: [first, second],
            hinge_driver,
            translation_drivers: [first_translation_driver, second_translation_driver],
        },
    )
}

fn identity_frame() -> Frame3 {
    Frame3::try_new(Point3::origin(), Vector3::x(), Vector3::y(), Vector3::z()).unwrap()
}

fn assert_shaft_position_difference_oracle(
    session: &SpatialAssemblySession,
    ids: geosolve_linkage::ShaftBearingExampleIds,
    solution: &geosolve_linkage::SpatialVelocitySolution,
    scale: f64,
) {
    let step = 1.0e-3;
    let shifted = |sign: f64| {
        let mut candidate = session.clone();
        candidate
            .apply_transaction(SpatialAssemblyTransaction::new(
                candidate.revision(),
                vec![
                    SpatialAssemblyEdit::HingeDriverTarget {
                        source: ids.drivers[0],
                        target: SpatialHingeTarget {
                            principal_phase: 0.48 + sign * 0.7 * step,
                            winding: 2,
                        },
                    },
                    SpatialAssemblyEdit::TranslationDriverTarget {
                        source: ids.drivers[1],
                        target: 1.9 * scale + sign * (-0.4 * scale) * step,
                    },
                ],
            ))
            .unwrap();
        candidate
    };
    let plus = shifted(1.0);
    let minus = shifted(-1.0);
    for body in ids.bodies {
        let center = session.accepted_result().geometry.body_pose(body).unwrap();
        let plus_pose = plus.accepted_result().geometry.body_pose(body).unwrap();
        let minus_pose = minus.accepted_result().geometry.body_pose(body).unwrap();
        let plus_difference = center.local_difference(&plus_pose).unwrap();
        let minus_difference = center.local_difference(&minus_pose).unwrap();
        let local = std::array::from_fn::<_, 6, _>(|index| {
            (plus_difference[index] - minus_difference[index]) / (2.0 * step)
        });
        let expected_linear = center
            .try_transform_vector(Vector3::new(local[0], local[1], local[2]))
            .unwrap();
        let expected_angular = center
            .try_transform_vector(Vector3::new(local[3], local[4], local[5]))
            .unwrap();
        let actual = solution.body(body).unwrap();
        let linear_error = (actual.origin_linear_world - expected_linear).norm() / scale;
        let angular_error = (actual.angular_world - expected_angular).norm();
        assert!(
            linear_error <= 5.0e-6,
            "body={body:?} scale={scale:e} linear_error={linear_error:e} actual={:?} expected={expected_linear:?}",
            actual.origin_linear_world
        );
        assert!(
            angular_error <= 5.0e-6,
            "body={body:?} scale={scale:e} angular_error={angular_error:e} actual={:?} expected={expected_angular:?}",
            actual.angular_world
        );
    }
    let plus_point = plus
        .accepted_result()
        .geometry
        .world_point(ids.translation_witness)
        .unwrap();
    let minus_point = minus
        .accepted_result()
        .geometry
        .world_point(ids.translation_witness)
        .unwrap();
    let expected_point = (plus_point - minus_point) / (2.0 * step);
    let actual_point = solution
        .point_velocities
        .iter()
        .find(|velocity| velocity.feature_id == ids.translation_witness)
        .unwrap()
        .linear_world;
    assert!((actual_point - expected_point).norm() / scale <= 5.0e-6);
}

fn assert_zero_body_velocity(velocity: geosolve_linkage::SpatialBodyVelocity, scale: f64) {
    assert!(velocity.origin_linear_world.norm() / scale <= 2.0e-9);
    assert!(velocity.angular_world.norm() <= 2.0e-9);
}
