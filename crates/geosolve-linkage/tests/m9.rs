// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_core::{HardValidity, SolveTermination, SolverConfig};
use geosolve_geometry::{Point2, Pose2, Vector2};
use geosolve_linkage::{LinkageGeometry, SolveRejection, slider_crank_with_scale};

fn assert_finite_geometry(geometry: &LinkageGeometry) {
    assert!(geometry.bodies.iter().all(|body| {
        body.pose.translation.iter().all(|value| value.is_finite()) && body.pose.angle.is_finite()
    }));
    assert!(geometry.points.iter().all(|point| {
        point.planar.coords.iter().all(|value| value.is_finite())
            && point.world.coords.iter().all(|value| value.is_finite())
    }));
    assert!(geometry.axes.iter().all(|axis| {
        axis.planar.iter().all(|value| value.is_finite())
            && axis.world.iter().all(|value| value.is_finite())
    }));
}

fn assert_same_body_pose(
    first: &LinkageGeometry,
    second: &LinkageGeometry,
    body: geosolve_linkage::BodyId,
) {
    let first = first.body_pose(body).unwrap();
    let second = second.body_pose(body).unwrap();
    assert_eq!(
        first.translation.x.to_bits(),
        second.translation.x.to_bits()
    );
    assert_eq!(
        first.translation.y.to_bits(),
        second.translation.y.to_bits()
    );
    assert_eq!(first.angle.to_bits(), second.angle.to_bits());
}

#[test]
#[allow(clippy::too_many_lines)]
fn crank_driven_slider_crank_crosses_exact_alignment_with_truthful_conditioning() {
    let (mut linkage, ids) = slider_crank_with_scale(1.0).unwrap();
    let near = linkage
        .drive_to(ids.driver, 1.0e-6, SolverConfig::default())
        .unwrap();
    assert!(near.completed(), "{near:#?}");
    let near_solve = &near.samples.last().unwrap().solve;
    assert!(near_solve.accepted(), "{near_solve:#?}");
    assert_eq!(near_solve.core_report.hard_validity, HardValidity::Valid);
    assert_finite_geometry(&near_solve.geometry);
    let crank_pin = near_solve.geometry.point(ids.crank_a).unwrap();
    let slider_pin = near_solve.geometry.point(ids.slider_pin).unwrap();
    assert!((crank_pin.x - 1.25).abs() <= 1.0e-9);
    assert!((crank_pin.y - 1.25e-6).abs() <= 1.0e-9);
    assert!((slider_pin.x - 4.75).abs() <= 1.0e-9);
    assert!(slider_pin.y.abs() <= 1.0e-9);
    assert_eq!(near_solve.core_report.rank, 9);
    assert_eq!(near_solve.core_report.left_nullity, 0);
    assert_eq!(near_solve.core_report.right_nullity, 0);
    assert!(!near_solve.core_report.is_singular);
    assert!(!near_solve.core_report.near_singular);
    let component = near_solve
        .core_report
        .component_solves
        .iter()
        .find(|component| component.singular_values.len() == 9)
        .unwrap();
    assert!(component.rank_is_valid);
    assert_eq!(component.rank, 9);
    assert_eq!((component.left_nullity, component.right_nullity), (0, 0));
    assert!(!component.is_singular);
    assert!(!component.near_singular);
    assert_eq!(
        component.near_singular_factor.to_bits(),
        100.0_f64.to_bits()
    );
    assert!(component.rank_machine_tolerance > 0.0);
    assert!(component.rank_threshold > component.rank_machine_tolerance);
    assert!(component.near_singular_ratio.unwrap() > component.near_singular_factor);
    let ratio = component.smallest_retained_singular_value.unwrap() / component.sigma_max;
    assert!((ratio - 0.117_204_414_858_091_42).abs() <= 1.0e-14);
    assert_eq!(near_solve.diagnostics.singular_value_ratio, Some(ratio));
    assert!(!near_solve.diagnostics.has_rank_warning);

    let near_velocity = linkage.velocity(ids.driver, 1.0).unwrap();
    assert!(near_velocity.rank_is_valid);
    assert_eq!(near_velocity.rank, 9);
    assert_eq!(near_velocity.local_degrees_of_freedom, 0);
    assert!(!near_velocity.is_singular);
    assert_eq!(near_velocity.singular_values.len(), 9);
    assert!(near_velocity.differentiated_residual_max <= 1.0e-9);
    assert!(near_velocity.body_velocities.iter().all(|velocity| {
        velocity.linear.iter().all(|value| value.is_finite()) && velocity.angular.is_finite()
    }));

    let exact = linkage
        .drive_to(ids.driver, 0.0, SolverConfig::default())
        .unwrap();
    assert!(exact.completed(), "{exact:#?}");
    let exact_solve = &exact.samples.last().unwrap().solve;
    assert!(exact_solve.accepted(), "{exact_solve:#?}");
    assert_eq!(exact_solve.core_report.hard_validity, HardValidity::Valid);
    assert_eq!(exact_solve.core_report.rank, 9);
    assert!(!exact_solve.core_report.is_singular);
    assert!(!exact_solve.core_report.near_singular);
    assert!(!exact_solve.diagnostics.has_rank_warning);
    assert_finite_geometry(&exact_solve.geometry);
    let exact_crank_pin = exact_solve.geometry.point(ids.crank_a).unwrap();
    let exact_slider_pin = exact_solve.geometry.point(ids.slider_pin).unwrap();
    assert!((exact_crank_pin.x - 1.25).abs() <= 1.0e-9);
    assert!(exact_crank_pin.y.abs() <= 1.0e-9);
    assert!((exact_slider_pin.x - 4.75).abs() <= 1.0e-9);
    assert!(exact_slider_pin.y.abs() <= 1.0e-9);

    let crossed = linkage
        .drive_to(ids.driver, -1.0e-6, SolverConfig::default())
        .unwrap();
    assert!(crossed.completed(), "{crossed:#?}");
    let crossed_solve = &crossed.samples.last().unwrap().solve;
    assert!(crossed_solve.accepted(), "{crossed_solve:#?}");
    assert_eq!(crossed_solve.core_report.hard_validity, HardValidity::Valid);
    assert_eq!(crossed_solve.core_report.rank, 9);
    assert!(!crossed_solve.core_report.near_singular);
    assert_finite_geometry(&crossed_solve.geometry);

    let retained = linkage.geometry().unwrap();
    let blocker = linkage
        .add_body(
            "incompatible blocker",
            Pose2 {
                translation: Vector2::new(100.0, 0.0),
                angle: 0.0,
            },
            true,
        )
        .unwrap();
    let blocker_pin = linkage
        .add_point_feature("blocker.pin", blocker, Point2::origin())
        .unwrap();
    linkage
        .add_revolute_joint("incompatible aligned closure", ids.slider_pin, blocker_pin)
        .unwrap();

    let rejected = linkage.solve(SolverConfig::default()).unwrap();

    assert!(!rejected.accepted());
    assert_eq!(
        rejected.rejection,
        Some(SolveRejection::CoreTermination(
            SolveTermination::IterationLimit
        ))
    );
    assert_eq!(
        rejected.core_report.termination,
        SolveTermination::IterationLimit
    );
    assert_eq!(rejected.core_report.hard_validity, HardValidity::Invalid);
    assert!(rejected.core_report.hard_residual_max.is_finite());
    assert!(rejected.core_report.hard_residual_max > 1.0);
    assert_finite_geometry(&rejected.geometry);
    assert_same_body_pose(&retained, &rejected.geometry, ids.crank);
    assert_same_body_pose(&retained, &rejected.geometry, ids.rod);
    assert_same_body_pose(&retained, &rejected.geometry, ids.slider);
}
