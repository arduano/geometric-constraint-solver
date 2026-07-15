#![allow(clippy::too_many_lines)]

use std::f64::consts::PI;

use geosolve_core::{
    HardValidity, ResidualCategory, SolveTermination, SolverConfig, VariableValue,
};
use geosolve_geometry::{PlaneFrame, Point2, Point3, Pose2, Vector2, Vector3};
use geosolve_linkage::{
    AxisDirectionBranch, BranchMonitorKind, BranchSign, DriverKind, DriverUnit,
    FourBarAssemblyMode, Linkage, LinkageError, LinkageGeometry, LinkageSolveResult, LinkageSource,
    SolveRejection, four_bar_with_scale, slider_crank_with_scale, xy_plane_frame,
};

const TOLERANCE: f64 = 1.0e-9;

fn radians(degrees: f64) -> f64 {
    degrees * PI / 180.0
}

fn assert_accepted(result: &LinkageSolveResult) {
    assert!(result.accepted(), "{:#?}", result.rejection);
    assert_eq!(result.core_report.termination, SolveTermination::Converged);
    assert!(result.core_report.hard_residuals_validated);
    assert!(result.core_report.hard_residual_max <= TOLERANCE);
    assert!(result.acceptance_hard_residual_max.unwrap() <= TOLERANCE);
    assert_eq!(result.display_audit, result.core_report.audit);
}

fn assert_point_close(first: Point2<f64>, second: Point2<f64>, scale: f64) {
    let error = (first - second).norm() / scale;
    assert!(
        error <= TOLERANCE,
        "point error {error:e}: {first:?} {second:?}"
    );
}

fn orientation(
    geometry: &LinkageGeometry,
    start: geosolve_linkage::PointFeatureId,
    end: geosolve_linkage::PointFeatureId,
    observed: geosolve_linkage::PointFeatureId,
) -> f64 {
    let start = geometry.point(start).unwrap();
    let line = geometry.point(end).unwrap() - start;
    let side = geometry.point(observed).unwrap() - start;
    line.x * side.y - line.y * side.x
}

fn assert_four_bar_closure(
    geometry: &LinkageGeometry,
    ids: &geosolve_linkage::FourBarIds,
    scale: f64,
) {
    for (first, second) in [
        (ids.ground_o2, ids.input_o2),
        (ids.input_a, ids.coupler_a),
        (ids.coupler_b, ids.rocker_b),
        (ids.rocker_o4, ids.ground_o4),
    ] {
        assert_point_close(
            geometry.point(first).unwrap(),
            geometry.point(second).unwrap(),
            scale,
        );
    }
}

fn assert_slider_closure(
    geometry: &LinkageGeometry,
    ids: &geosolve_linkage::SliderCrankIds,
    scale: f64,
) {
    for (first, second) in [
        (ids.ground_o, ids.crank_o),
        (ids.crank_a, ids.rod_a),
        (ids.rod_slider, ids.slider_pin),
    ] {
        assert_point_close(
            geometry.point(first).unwrap(),
            geometry.point(second).unwrap(),
            scale,
        );
    }
    let guide = geometry.axis(ids.ground_guide_axis).unwrap();
    let slider_axis = geometry.axis(ids.slider_axis).unwrap();
    let displacement =
        geometry.point(ids.slider_pin).unwrap() - geometry.point(ids.ground_guide_origin).unwrap();
    let normal = Vector2::new(-guide.y, guide.x);
    assert!(normal.dot(&displacement).abs() / scale <= TOLERANCE);
    assert!((guide.x * slider_axis.y - guide.y * slider_axis.x).abs() <= TOLERANCE);
    assert!(guide.dot(&slider_axis) > 0.0);
    assert!(guide.dot(&displacement) > 0.0);
}

fn check_jacobians(linkage: &Linkage) {
    let compiled = linkage.compile().unwrap();
    let check = compiled.problem().check_jacobians(1.0e-5).unwrap();
    assert!(
        check.all_within(1.0e-6),
        "max relative error={:e}: {check:#?}",
        check.max_relative_error()
    );
}

fn linear_slider_fixture(
    initial: Pose2,
    axis_branch: AxisDirectionBranch,
) -> (
    Linkage,
    geosolve_linkage::BodyId,
    geosolve_linkage::DriverId,
) {
    let mut linkage = Linkage::new(2.0, xy_plane_frame()).unwrap();
    let ground = linkage.add_body("ground", Pose2::identity(), true).unwrap();
    let slider = linkage.add_body("slider", initial, false).unwrap();
    let ground_origin = linkage
        .add_point_feature("ground.origin", ground, Point2::origin())
        .unwrap();
    let ground_axis = linkage
        .add_axis_feature("ground.axis", ground, Vector2::x())
        .unwrap();
    let slider_origin = linkage
        .add_point_feature("slider.origin", slider, Point2::origin())
        .unwrap();
    let slider_local_axis = match axis_branch {
        AxisDirectionBranch::Same => Vector2::x(),
        AxisDirectionBranch::Opposite => -Vector2::x(),
    };
    let slider_axis = linkage
        .add_axis_feature("slider.axis", slider, slider_local_axis)
        .unwrap();
    linkage
        .add_prismatic_joint(
            "guide",
            ground_origin,
            ground_axis,
            slider_origin,
            slider_axis,
            axis_branch,
        )
        .unwrap();
    let driver = linkage
        .add_linear_driver(
            "slide",
            ground_origin,
            slider_origin,
            ground_axis,
            2.0,
            0.25,
        )
        .unwrap();
    (linkage, slider, driver)
}

#[test]
fn every_joint_and_driver_row_has_analytic_jacobians_and_recovers_perturbations() {
    let (exact_slider, _, _) = linear_slider_fixture(
        Pose2 {
            translation: Vector2::new(2.0, 0.0),
            angle: 0.0,
        },
        AxisDirectionBranch::Same,
    );
    check_jacobians(&exact_slider);

    let (mut slider, slider_body, _) = linear_slider_fixture(
        Pose2 {
            translation: Vector2::new(2.4, 0.3),
            angle: 0.2,
        },
        AxisDirectionBranch::Same,
    );
    check_jacobians(&slider);
    let solved = slider.solve(SolverConfig::default()).unwrap();
    assert_accepted(&solved);
    let pose = solved.geometry.body_pose(slider_body).unwrap();
    assert!((pose.translation.x - 2.0).abs() / 2.0 <= TOLERANCE);
    assert!(pose.translation.y.abs() / 2.0 <= TOLERANCE);
    assert!(pose.angle.abs() <= TOLERANCE);

    let (mut opposite, opposite_body, _) = linear_slider_fixture(
        Pose2 {
            translation: Vector2::new(1.8, -0.25),
            angle: -0.15,
        },
        AxisDirectionBranch::Opposite,
    );
    check_jacobians(&opposite);
    let solved = opposite.solve(SolverConfig::default()).unwrap();
    assert_accepted(&solved);
    assert!(
        solved
            .geometry
            .body_pose(opposite_body)
            .unwrap()
            .angle
            .abs()
            <= TOLERANCE
    );

    let mut weld = Linkage::new(3.0, xy_plane_frame()).unwrap();
    let ground = weld.add_body("ground", Pose2::identity(), true).unwrap();
    let body = weld
        .add_body(
            "welded",
            Pose2 {
                translation: Vector2::new(2.0, 1.0),
                angle: 0.4,
            },
            false,
        )
        .unwrap();
    let ground_anchor = weld
        .add_point_feature("ground.anchor", ground, Point2::new(2.0, 1.0))
        .unwrap();
    let body_anchor = weld
        .add_point_feature("body.anchor", body, Point2::origin())
        .unwrap();
    weld.add_weld_joint("fixed", ground_anchor, body_anchor)
        .unwrap();
    check_jacobians(&weld);
    weld.set_body_pose(
        body,
        Pose2 {
            translation: Vector2::new(2.3, 0.6),
            angle: 0.1,
        },
    )
    .unwrap();
    check_jacobians(&weld);
    let solved = weld.solve(SolverConfig::default()).unwrap();
    assert_accepted(&solved);
    let pose = solved.geometry.body_pose(body).unwrap();
    assert_point_close(Point2::from(pose.translation), Point2::new(2.0, 1.0), 3.0);
    assert!((pose.angle - 0.4).abs() <= TOLERANCE);

    let (mut four_bar, ids) = four_bar_with_scale(FourBarAssemblyMode::Open, 1.0).unwrap();
    check_jacobians(&four_bar);
    let perturbed = four_bar.body(ids.coupler).unwrap().pose();
    four_bar
        .set_body_pose(
            ids.coupler,
            Pose2 {
                translation: perturbed.translation + Vector2::new(0.04, -0.03),
                angle: perturbed.angle + 0.02,
            },
        )
        .unwrap();
    check_jacobians(&four_bar);
    let solved = four_bar.solve(SolverConfig::default()).unwrap();
    assert_accepted(&solved);
    assert_four_bar_closure(&solved.geometry, &ids, 1.0);
}

#[test]
fn compilation_uses_one_pose_per_body_ground_elimination_and_exact_source_mapping() {
    let (linkage, ids) = four_bar_with_scale(FourBarAssemblyMode::Open, 2.0).unwrap();
    let compiled = linkage.compile().unwrap();
    assert_eq!(compiled.body_variables().len(), 4);
    assert_eq!(
        compiled
            .problem()
            .packed_layout()
            .unwrap()
            .tangent_dimension(),
        12
    );
    assert_eq!(
        compiled
            .problem()
            .variable(compiled.variable_for_body(ids.input).unwrap())
            .unwrap()
            .step_scales(),
        &[2.0, 2.0, 1.0]
    );
    let structural = compiled.problem().structural_summary().unwrap();
    assert_eq!(structural.fixed_eliminated_coordinates, 3);
    assert_eq!(structural.eliminated_rows, 3);
    assert_eq!(structural.scalar_rows, 12);
    let body_order: Vec<_> = compiled
        .body_variables()
        .iter()
        .map(|mapping| mapping.body_id)
        .collect();
    assert_eq!(
        body_order,
        vec![ids.ground, ids.input, ids.coupler, ids.rocker]
    );
    for mapping in compiled.source_mappings() {
        assert_eq!(mapping.residual_ids.len(), 1);
        let residual = compiled
            .problem()
            .residual(mapping.residual_ids[0])
            .unwrap();
        assert_eq!(residual.source(), mapping.core_source_id);
        assert_eq!(
            compiled
                .problem()
                .source(mapping.core_source_id)
                .unwrap()
                .label(),
            mapping.source_label
        );
    }
    assert!(
        compiled
            .source_mapping(LinkageSource::Ground(ids.ground))
            .is_some()
    );
    let driver = linkage.driver(ids.driver).unwrap();
    assert!(matches!(driver.kind(), DriverKind::Angular { .. }));
    assert_eq!(driver.unit(), DriverUnit::Radian);
    assert!((driver.max_continuation_step() - radians(2.0)).abs() <= f64::EPSILON);
    assert!(compiled.problem().audit_rows().unwrap().iter().all(|row| {
        row.category == ResidualCategory::Hard
            && !row.template.trim().is_empty()
            && !row.bindings.is_empty()
            && row.scale.is_finite()
            && row.scale > 0.0
    }));
}

#[test]
fn typed_branch_evaluations_expose_canonical_raw_metrics_and_expected_signs() {
    let (open, open_ids) = four_bar_with_scale(FourBarAssemblyMode::Open, 1.0).unwrap();
    let open_geometry = open.geometry().unwrap();
    let open_evaluation = open
        .evaluate_branch_monitor(open_ids.orientation_monitor, &open_geometry)
        .unwrap();
    assert_eq!(open_evaluation.monitor_id, open_ids.orientation_monitor);
    assert_eq!(open_evaluation.kind, BranchMonitorKind::Orientation);
    assert_eq!(open_evaluation.expected_sign, BranchSign::Positive);
    assert!(open_evaluation.signed_metric.is_finite());
    assert!(open_evaluation.signed_metric > 0.0);
    assert!(open_evaluation.retained);

    let (crossed, crossed_ids) = four_bar_with_scale(FourBarAssemblyMode::Crossed, 1.0).unwrap();
    let crossed_evaluation = crossed
        .evaluate_branch_monitor(
            crossed_ids.orientation_monitor,
            &crossed.geometry().unwrap(),
        )
        .unwrap();
    assert_eq!(crossed_evaluation.kind, BranchMonitorKind::Orientation);
    assert_eq!(crossed_evaluation.expected_sign, BranchSign::Negative);
    assert!(crossed_evaluation.signed_metric.is_finite());
    assert!(crossed_evaluation.signed_metric < 0.0);
    assert!(crossed_evaluation.retained);
    assert!(open_evaluation.signed_metric * crossed_evaluation.signed_metric < 0.0);

    let (slider, slider_ids) = slider_crank_with_scale(1.0).unwrap();
    let slider_geometry = slider.geometry().unwrap();
    let slider_evaluation = slider
        .evaluate_branch_monitor(slider_ids.positive_x_monitor, &slider_geometry)
        .unwrap();
    assert_eq!(
        slider_evaluation.kind,
        BranchMonitorKind::DirectedDisplacement
    );
    assert_eq!(slider_evaluation.expected_sign, BranchSign::Positive);
    assert!(slider_evaluation.signed_metric.is_finite());
    assert!(slider_evaluation.signed_metric > 0.0);
    assert!(slider_evaluation.retained);
    let expected_projection = slider_geometry
        .axis(slider_ids.ground_guide_axis)
        .unwrap()
        .dot(
            &(slider_geometry.point(slider_ids.slider_pin).unwrap()
                - slider_geometry
                    .point(slider_ids.ground_guide_origin)
                    .unwrap()),
        );
    assert!((slider_evaluation.signed_metric - expected_projection).abs() <= f64::EPSILON);
}

#[test]
fn branch_evaluation_rejects_missing_and_nonfinite_geometry() {
    let (linkage, ids) = four_bar_with_scale(FourBarAssemblyMode::Open, 1.0).unwrap();
    let geometry = linkage.geometry().unwrap();

    let mut missing = geometry.clone();
    missing
        .points
        .retain(|point| point.feature_id != ids.coupler_b);
    assert!(matches!(
        linkage.evaluate_branch_monitor(ids.orientation_monitor, &missing),
        Err(LinkageError::UnknownPointFeature(id)) if id == ids.coupler_b
    ));

    let mut nonfinite = geometry;
    nonfinite
        .points
        .iter_mut()
        .find(|point| point.feature_id == ids.coupler_b)
        .unwrap()
        .planar
        .y = f64::NAN;
    assert!(matches!(
        linkage.evaluate_branch_monitor(ids.orientation_monitor, &nonfinite),
        Err(LinkageError::NonFiniteValue {
            context: "branch monitor metric",
            value
        }) if value.is_nan()
    ));
}

#[test]
fn l1_l2_initial_solutions_and_safe_sweeps_preserve_opposite_assembly_signs() {
    let mut signs = Vec::new();
    for mode in [FourBarAssemblyMode::Open, FourBarAssemblyMode::Crossed] {
        let (mut linkage, ids) = four_bar_with_scale(mode, 1.0).unwrap();
        let ground_pose = linkage.body(ids.ground).unwrap().pose();
        let initial = linkage.solve(SolverConfig::default()).unwrap();
        assert_accepted(&initial);
        assert_eq!(initial.core_report.rank, 9);
        assert_eq!(initial.core_report.local_degrees_of_freedom, 0);
        assert_eq!(initial.geometry.body_pose(ids.ground).unwrap(), ground_pose);
        assert_four_bar_closure(&initial.geometry, &ids, 1.0);
        let initial_sign =
            orientation(&initial.geometry, ids.input_a, ids.ground_o4, ids.coupler_b);
        assert_eq!(
            initial_sign > 0.0,
            ids.orientation_sign == BranchSign::Positive
        );
        signs.push(initial_sign > 0.0);

        let down = linkage
            .drive_to(ids.driver, radians(25.0), SolverConfig::default())
            .unwrap();
        assert!(down.completed());
        for sample in &down.samples {
            assert!(sample.step.abs() <= radians(2.0) * (1.0 + 1.0e-14));
            assert_accepted(&sample.solve);
            assert_four_bar_closure(&sample.solve.geometry, &ids, 1.0);
            let sign = orientation(
                &sample.solve.geometry,
                ids.input_a,
                ids.ground_o4,
                ids.coupler_b,
            );
            assert_eq!(sign > 0.0, initial_sign > 0.0);
        }
        let up = linkage
            .drive_to(ids.driver, radians(135.0), SolverConfig::default())
            .unwrap();
        assert!(up.completed());
        for sample in &up.samples {
            assert!(sample.step.abs() <= radians(2.0) * (1.0 + 1.0e-14));
            assert_accepted(&sample.solve);
            assert_four_bar_closure(&sample.solve.geometry, &ids, 1.0);
            let sign = orientation(
                &sample.solve.geometry,
                ids.input_a,
                ids.ground_o4,
                ids.coupler_b,
            );
            assert_eq!(sign > 0.0, initial_sign > 0.0);
        }
        assert_eq!(linkage.body(ids.ground).unwrap().pose(), ground_pose);
    }
    assert_eq!(signs, vec![true, false]);
}

#[test]
fn l3_initial_and_full_safe_sweep_validate_revolute_guide_and_positive_x_branch() {
    let (mut linkage, ids) = slider_crank_with_scale(1.0).unwrap();
    let ground_pose = linkage.body(ids.ground).unwrap().pose();
    let initial = linkage.solve(SolverConfig::default()).unwrap();
    assert_accepted(&initial);
    assert_eq!(initial.core_report.rank, 9);
    assert_eq!(initial.core_report.local_degrees_of_freedom, 0);
    assert_slider_closure(&initial.geometry, &ids, 1.0);
    let down = linkage
        .drive_to(ids.driver, radians(15.0), SolverConfig::default())
        .unwrap();
    assert!(down.completed());
    let up = linkage
        .drive_to(ids.driver, radians(165.0), SolverConfig::default())
        .unwrap();
    assert!(up.completed());
    for sample in down.samples.iter().chain(&up.samples) {
        assert!(
            sample.step.abs() <= radians(2.0) * (1.0 + 1.0e-14),
            "oversized sample step {} degrees",
            sample.step.to_degrees()
        );
        assert_accepted(&sample.solve);
        assert_slider_closure(&sample.solve.geometry, &ids, 1.0);
    }
    assert_eq!(linkage.body(ids.ground).unwrap().pose(), ground_pose);
}

fn assert_velocity_matches_position_oracle(
    linkage: &Linkage,
    driver: geosolve_linkage::DriverId,
    bodies: &[geosolve_linkage::BodyId],
    scale: f64,
) {
    let velocity = linkage.velocity(driver, 1.0).unwrap();
    assert!(velocity.rank_is_valid);
    assert!(velocity.differentiated_residual_max <= TOLERANCE);
    let free_tangent_dimensions = linkage
        .bodies()
        .filter(|(_, body)| !body.grounded())
        .count()
        * 3;
    assert_eq!(velocity.rank, free_tangent_dimensions);
    assert_eq!(velocity.local_degrees_of_freedom, 0);
    for (body_id, _) in linkage.bodies().filter(|(_, body)| body.grounded()) {
        let grounded = velocity.body(body_id).unwrap();
        assert_eq!(grounded.linear, Vector2::zeros());
        assert!(grounded.angular.abs() <= f64::EPSILON);
    }
    let target = linkage.driver(driver).unwrap().target();
    let step = 1.0e-5;
    let mut plus = linkage.clone();
    let mut minus = linkage.clone();
    let plus_result = plus
        .drive_to(driver, target + step, SolverConfig::default())
        .unwrap();
    let minus_result = minus
        .drive_to(driver, target - step, SolverConfig::default())
        .unwrap();
    assert!(plus_result.completed(), "{plus_result:#?}");
    assert!(minus_result.completed(), "{minus_result:#?}");
    let plus_geometry = plus.geometry().unwrap();
    let minus_geometry = minus.geometry().unwrap();
    for &body in bodies {
        let analytic = velocity.body(body).unwrap();
        let plus_pose = plus_geometry.body_pose(body).unwrap();
        let minus_pose = minus_geometry.body_pose(body).unwrap();
        let numeric_linear = (plus_pose.translation - minus_pose.translation) / (2.0 * step);
        let numeric_angular = (plus_pose.angle - minus_pose.angle) / (2.0 * step);
        // Continuation solves positions only to 1e-9 normalized, so dividing by
        // the 1e-5 oracle step limits useful normalized rate accuracy to ~1e-4.
        let linear_error = (analytic.linear - numeric_linear).norm() / scale;
        let angular_error = (analytic.angular - numeric_angular).abs();
        assert!(
            linear_error <= 5.0e-4,
            "body={body:?} linear error={linear_error:e}"
        );
        assert!(
            angular_error <= 5.0e-4,
            "body={body:?} angular error={angular_error:e}"
        );
    }
}

#[test]
fn l1_l2_l3_velocities_match_central_position_continuation_oracles() {
    for mode in [FourBarAssemblyMode::Open, FourBarAssemblyMode::Crossed] {
        let (mut linkage, ids) = four_bar_with_scale(mode, 1.0).unwrap();
        assert!(
            linkage
                .drive_to(ids.driver, radians(90.0), SolverConfig::default())
                .unwrap()
                .completed()
        );
        assert_velocity_matches_position_oracle(
            &linkage,
            ids.driver,
            &[ids.ground, ids.input, ids.coupler, ids.rocker],
            1.0,
        );
    }
    let (mut linkage, ids) = slider_crank_with_scale(1.0).unwrap();
    assert!(
        linkage
            .drive_to(ids.driver, radians(75.0), SolverConfig::default())
            .unwrap()
            .completed()
    );
    assert_velocity_matches_position_oracle(
        &linkage,
        ids.driver,
        &[ids.ground, ids.crank, ids.rod, ids.slider],
        1.0,
    );
}

#[test]
fn linear_driver_continuation_and_velocity_are_physical_and_validated() {
    let (mut linkage, slider, driver) = linear_slider_fixture(
        Pose2 {
            translation: Vector2::new(2.0, 0.0),
            angle: 0.0,
        },
        AxisDirectionBranch::Same,
    );
    let initial = linkage.solve(SolverConfig::default()).unwrap();
    assert_accepted(&initial);
    assert_eq!(
        linkage.driver(driver).unwrap().unit(),
        DriverUnit::ModelUnit
    );
    let continuation = linkage
        .drive_to(driver, 3.1, SolverConfig::default())
        .unwrap();
    assert!(continuation.completed());
    assert!(
        continuation
            .samples
            .iter()
            .all(|sample| sample.step.abs() <= 0.25)
    );
    let pose = linkage.geometry().unwrap().body_pose(slider).unwrap();
    assert!((pose.translation.x - 3.1).abs() / 2.0 <= TOLERANCE);
    let velocity = linkage.velocity(driver, 1.0).unwrap();
    assert!(velocity.differentiated_residual_max <= TOLERANCE);
    let slider_velocity = velocity.body(slider).unwrap();
    assert!((slider_velocity.linear.x - 1.0).abs() <= TOLERANCE);
    assert!(slider_velocity.linear.y.abs() <= TOLERANCE);
    assert!(slider_velocity.angular.abs() <= TOLERANCE);
    assert_velocity_matches_position_oracle(&linkage, driver, &[slider], 2.0);
}

#[test]
fn canonical_solutions_are_scale_invariant_at_required_extremes() {
    for scale in [1.0e-6, 1.0, 1.0e6] {
        for mode in [FourBarAssemblyMode::Open, FourBarAssemblyMode::Crossed] {
            let (mut linkage, ids) = four_bar_with_scale(mode, scale).unwrap();
            let initial = linkage.solve(SolverConfig::default()).unwrap();
            assert_accepted(&initial);
            assert_eq!(initial.core_report.rank, 9);
            assert_eq!(initial.core_report.local_degrees_of_freedom, 0);
            assert_four_bar_closure(&initial.geometry, &ids, scale);
            let driven = linkage
                .drive_to(ids.driver, radians(90.0), SolverConfig::default())
                .unwrap();
            assert!(driven.completed());
            assert_four_bar_closure(&linkage.geometry().unwrap(), &ids, scale);
            let sign = orientation(
                &linkage.geometry().unwrap(),
                ids.input_a,
                ids.ground_o4,
                ids.coupler_b,
            );
            assert_eq!(sign > 0.0, mode == FourBarAssemblyMode::Open);
        }

        let (mut linkage, ids) = slider_crank_with_scale(scale).unwrap();
        let result = linkage.solve(SolverConfig::default()).unwrap();
        assert_accepted(&result);
        assert_eq!(result.core_report.rank, 9);
        assert_slider_closure(&result.geometry, &ids, scale);
        assert!(
            linkage
                .drive_to(ids.driver, radians(100.0), SolverConfig::default())
                .unwrap()
                .completed()
        );
        assert_slider_closure(&linkage.geometry().unwrap(), &ids, scale);
        assert!(
            linkage
                .velocity(ids.driver, 1.0)
                .unwrap()
                .differentiated_residual_max
                <= TOLERANCE
        );
    }
}

#[test]
fn invalid_scale_frame_features_ids_targets_rates_and_step_policies_are_rejected() {
    for scale in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        assert!(matches!(
            Linkage::new(scale, xy_plane_frame()),
            Err(LinkageError::InvalidModelScale(_))
        ));
    }
    for frame in [
        PlaneFrame {
            origin: Point3::new(f64::NAN, 0.0, 0.0),
            u: Vector3::x(),
            v: Vector3::y(),
        },
        PlaneFrame {
            origin: Point3::origin(),
            u: Vector3::new(2.0, 0.0, 0.0),
            v: Vector3::y(),
        },
        PlaneFrame {
            origin: Point3::origin(),
            u: Vector3::x(),
            v: Vector3::x(),
        },
    ] {
        assert!(matches!(
            Linkage::new(1.0, frame),
            Err(LinkageError::InvalidPlaneFrame(_))
        ));
    }
    let mut linkage = Linkage::new(1.0, xy_plane_frame()).unwrap();
    let ground = linkage.add_body("ground", Pose2::identity(), true).unwrap();
    assert!(matches!(
        linkage.add_body(
            "bad",
            Pose2 {
                translation: Vector2::new(f64::NAN, 0.0),
                angle: 0.0,
            },
            false
        ),
        Err(LinkageError::NonFinitePose { .. })
    ));
    assert!(matches!(
        linkage.add_point_feature("bad", ground, Point2::new(f64::INFINITY, 0.0)),
        Err(LinkageError::NonFinitePoint { .. })
    ));
    for axis in [Vector2::zeros(), Vector2::new(f64::NAN, 1.0)] {
        assert!(matches!(
            linkage.add_axis_feature("bad", ground, axis),
            Err(LinkageError::InvalidAxis { .. })
        ));
    }
    let normalized = linkage
        .add_axis_feature("normalized", ground, Vector2::new(3.0, 4.0))
        .unwrap();
    assert!(
        (linkage
            .axis_feature(normalized)
            .unwrap()
            .local_axis()
            .norm()
            - 1.0)
            .abs()
            <= f64::EPSILON
    );

    let stale_body = linkage.add_body("stale", Pose2::identity(), false).unwrap();
    linkage.remove_body(stale_body).unwrap();
    assert!(matches!(
        linkage.add_point_feature("stale", stale_body, Point2::origin()),
        Err(LinkageError::UnknownBody(id)) if id == stale_body
    ));
    let moving = linkage
        .add_body("moving", Pose2::identity(), false)
        .unwrap();
    for step in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        assert!(matches!(
            linkage.add_angular_driver("bad", ground, moving, 0.0, step),
            Err(LinkageError::InvalidContinuationStep(_))
        ));
    }
    let driver = linkage
        .add_angular_driver("angle", ground, moving, 0.0, 0.1)
        .unwrap();
    assert!(matches!(
        linkage.drive_to(driver, f64::NAN, SolverConfig::default()),
        Err(LinkageError::NonFiniteValue { .. })
    ));
    assert!(matches!(
        linkage.velocity(driver, f64::INFINITY),
        Err(LinkageError::NonFiniteValue { .. })
    ));
    linkage.remove_driver(driver).unwrap();
    assert!(matches!(
        linkage.drive_to(driver, 1.0, SolverConfig::default()),
        Err(LinkageError::UnknownDriver(id)) if id == driver
    ));
}

#[test]
fn failed_continuation_retains_geometry_target_and_display_audit() {
    let mut linkage = Linkage::new(1.0, xy_plane_frame()).unwrap();
    let first = linkage.add_body("first", Pose2::identity(), true).unwrap();
    let second = linkage.add_body("second", Pose2::identity(), true).unwrap();
    let driver = linkage
        .add_angular_driver("impossible", first, second, 0.0, 0.2)
        .unwrap();
    let initial = linkage.solve(SolverConfig::default()).unwrap();
    assert_accepted(&initial);
    let retained_geometry = linkage.geometry().unwrap();
    let retained_target = linkage.driver(driver).unwrap().target();
    let drive = linkage
        .drive_to(driver, 0.4, SolverConfig::default())
        .unwrap();
    assert!(!drive.completed());
    assert_eq!(drive.accepted_target.to_bits(), retained_target.to_bits());
    assert_eq!(
        linkage.driver(driver).unwrap().target().to_bits(),
        retained_target.to_bits()
    );
    assert_eq!(linkage.geometry().unwrap(), retained_geometry);
    assert_eq!(drive.samples.len(), 1);
    let failed = drive.samples.last().unwrap();
    assert!(!failed.solve.accepted());
    assert_eq!(failed.solve.geometry, retained_geometry);
    assert_ne!(failed.solve.display_audit, failed.solve.core_report.audit);
    let display_mapping = failed
        .solve
        .source_mappings
        .iter()
        .find(|mapping| mapping.source == LinkageSource::Driver(driver))
        .unwrap();
    let attempt_mapping = failed
        .solve
        .attempt_source_mappings
        .iter()
        .find(|mapping| mapping.source == LinkageSource::Driver(driver))
        .unwrap();
    assert_ne!(display_mapping.source_label, attempt_mapping.source_label);
    let display_source = failed
        .solve
        .display_audit
        .sources
        .iter()
        .find(|source| source.source_id == display_mapping.core_source_id)
        .unwrap();
    assert_eq!(display_source.source_label, display_mapping.source_label);
    let retained_target_binding = display_source.rows[0]
        .bindings
        .iter()
        .find(|binding| binding.name == "target")
        .unwrap();
    assert_eq!(retained_target_binding.value, retained_target.to_string());
    let attempt_source = failed
        .solve
        .core_report
        .audit
        .sources
        .iter()
        .find(|source| source.source_id == attempt_mapping.core_source_id)
        .unwrap();
    assert_eq!(attempt_source.source_label, attempt_mapping.source_label);
}

#[test]
fn unchanged_target_drive_runs_validation_and_rejects_new_impossible_joint() {
    let (mut linkage, ids) = four_bar_with_scale(FourBarAssemblyMode::Open, 1.0).unwrap();
    assert_accepted(&linkage.solve(SolverConfig::default()).unwrap());
    let current_target = linkage.driver(ids.driver).unwrap().target();
    let valid_no_op = linkage
        .drive_to(ids.driver, current_target, SolverConfig::default())
        .unwrap();
    assert!(valid_no_op.completed());
    assert_eq!(valid_no_op.samples.len(), 1);
    assert!(valid_no_op.samples[0].step.abs() <= f64::EPSILON);
    assert_accepted(&valid_no_op.samples[0].solve);
    let obstacle = linkage
        .add_body(
            "fixed obstacle",
            Pose2 {
                translation: Vector2::new(2.0, 0.0),
                angle: 0.0,
            },
            true,
        )
        .unwrap();
    let obstacle_point = linkage
        .add_point_feature("obstacle.point", obstacle, Point2::origin())
        .unwrap();
    linkage
        .add_revolute_joint("new impossible closure", ids.ground_o2, obstacle_point)
        .unwrap();
    let retained = linkage.geometry().unwrap();
    let drive = linkage
        .drive_to(ids.driver, current_target, SolverConfig::default())
        .unwrap();
    assert!(!drive.completed());
    assert_eq!(drive.samples.len(), 1);
    assert!(drive.samples[0].step.abs() <= f64::EPSILON);
    assert!(!drive.samples[0].solve.accepted());
    assert_eq!(drive.samples[0].solve.geometry, retained);
    assert_eq!(linkage.geometry().unwrap(), retained);
    assert!(retained.bodies.iter().all(|body| {
        body.pose.translation.iter().all(|value| value.is_finite()) && body.pose.angle.is_finite()
    }));
}

#[test]
fn retained_redundancy_and_singularity_annotations_survive_failed_drive() {
    let mut linkage = Linkage::new(1.0, xy_plane_frame()).unwrap();
    let ground = linkage.add_body("ground", Pose2::identity(), true).unwrap();
    let moving = linkage
        .add_body("moving", Pose2::identity(), false)
        .unwrap();
    let first = linkage
        .add_angular_driver("first angle", ground, moving, 0.0, 0.1)
        .unwrap();
    let second = linkage
        .add_angular_driver("duplicate angle", ground, moving, 0.0, 0.1)
        .unwrap();
    let accepted = linkage.solve(SolverConfig::default()).unwrap();
    assert_accepted(&accepted);
    assert!(accepted.core_report.is_singular);
    assert!(!accepted.core_report.redundant_rows.is_empty());
    let retained = linkage.geometry().unwrap();

    let drive = linkage
        .drive_to(first, 0.1, SolverConfig::default())
        .unwrap();
    assert!(!drive.completed());
    let failed = drive.samples.last().unwrap();
    assert!(!failed.solve.accepted());
    assert_eq!(failed.solve.geometry, retained);
    assert!(
        failed
            .solve
            .display_audit
            .sources
            .iter()
            .any(|source| source.annotations.redundant || source.annotations.singular)
    );
    for driver in [first, second] {
        let mapping = failed
            .solve
            .source_mappings
            .iter()
            .find(|mapping| mapping.source == LinkageSource::Driver(driver))
            .unwrap();
        assert!(mapping.source_label.contains(" = 0 rad"));
    }
    assert!(matches!(
        linkage.velocity(first, 1.0),
        Err(LinkageError::VelocityFailure(_))
    ));
}

#[test]
fn multiple_drivers_differentiate_only_the_selected_target() {
    let mut linkage = Linkage::new(1.0, xy_plane_frame()).unwrap();
    let ground = linkage.add_body("ground", Pose2::identity(), true).unwrap();
    let ground_origin = linkage
        .add_point_feature("ground.origin", ground, Point2::origin())
        .unwrap();
    let ground_axis = linkage
        .add_axis_feature("ground.axis", ground, Vector2::x())
        .unwrap();

    let first_slider = linkage
        .add_body(
            "first slider",
            Pose2 {
                translation: Vector2::new(2.0, 0.0),
                angle: 0.0,
            },
            false,
        )
        .unwrap();
    let first_point = linkage
        .add_point_feature("first slider.point", first_slider, Point2::origin())
        .unwrap();
    let first_axis = linkage
        .add_axis_feature("first slider.axis", first_slider, Vector2::x())
        .unwrap();
    linkage
        .add_prismatic_joint(
            "first guide",
            ground_origin,
            ground_axis,
            first_point,
            first_axis,
            AxisDirectionBranch::Same,
        )
        .unwrap();
    let first_driver = linkage
        .add_linear_driver(
            "first displacement",
            ground_origin,
            first_point,
            ground_axis,
            2.0,
            0.25,
        )
        .unwrap();

    let second_slider = linkage
        .add_body(
            "second slider",
            Pose2 {
                translation: Vector2::new(3.0, 0.0),
                angle: 0.0,
            },
            false,
        )
        .unwrap();
    let second_point = linkage
        .add_point_feature("second slider.point", second_slider, Point2::origin())
        .unwrap();
    let second_axis = linkage
        .add_axis_feature("second slider.axis", second_slider, Vector2::x())
        .unwrap();
    linkage
        .add_prismatic_joint(
            "second guide",
            ground_origin,
            ground_axis,
            second_point,
            second_axis,
            AxisDirectionBranch::Same,
        )
        .unwrap();
    let second_driver = linkage
        .add_linear_driver(
            "second displacement",
            ground_origin,
            second_point,
            ground_axis,
            3.0,
            0.25,
        )
        .unwrap();

    let position = linkage.solve(SolverConfig::default()).unwrap();
    assert_accepted(&position);
    assert_eq!(position.core_report.rank, 6);
    assert_eq!(position.core_report.local_degrees_of_freedom, 0);
    let velocity = linkage.velocity(first_driver, 1.0).unwrap();
    assert_eq!(velocity.rank, 6);
    assert_eq!(velocity.local_degrees_of_freedom, 0);
    assert!(velocity.differentiated_residual_max <= TOLERANCE);
    let first = velocity.body(first_slider).unwrap();
    assert!((first.linear.x - 1.0).abs() <= TOLERANCE);
    assert!(first.linear.y.abs() <= TOLERANCE);
    assert!(first.angular.abs() <= TOLERANCE);
    let second = velocity.body(second_slider).unwrap();
    assert!(second.linear.norm() <= TOLERANCE);
    assert!(second.angular.abs() <= TOLERANCE);
    let fixed_ground = velocity.body(ground).unwrap();
    assert_eq!(fixed_ground.linear, Vector2::zeros());
    assert!(fixed_ground.angular.abs() <= f64::EPSILON);
    assert!(linkage.velocity(second_driver, 0.0).is_ok());
}

#[test]
fn velocity_truncates_reported_subthreshold_mode_and_rejects_inconsistent_rate() {
    let mut linkage = Linkage::new(1.0, xy_plane_frame()).unwrap();
    let ground = linkage.add_body("ground", Pose2::identity(), true).unwrap();
    let moving = linkage
        .add_body(
            "moving",
            Pose2 {
                translation: Vector2::new(1.0, 0.0),
                angle: 0.0,
            },
            false,
        )
        .unwrap();
    let origin = linkage
        .add_point_feature("origin", ground, Point2::origin())
        .unwrap();
    let measured = linkage
        .add_point_feature("measured", moving, Point2::origin())
        .unwrap();
    let first_axis = linkage
        .add_axis_feature("x axis", ground, Vector2::x())
        .unwrap();
    let second_axis = linkage
        .add_axis_feature("near x axis", ground, Vector2::new(1.0, 1.0e-12))
        .unwrap();
    let first_driver = linkage
        .add_linear_driver("x displacement", origin, measured, first_axis, 1.0, 0.1)
        .unwrap();
    let second_target = linkage.axis_feature(second_axis).unwrap().local_axis().x;
    linkage
        .add_linear_driver(
            "near-x displacement",
            origin,
            measured,
            second_axis,
            second_target,
            0.1,
        )
        .unwrap();
    linkage
        .add_angular_driver("angle", ground, moving, 0.0, 0.1)
        .unwrap();
    let position = linkage.solve(SolverConfig::default()).unwrap();
    assert_accepted(&position);
    assert_eq!(position.core_report.rank, 2);
    assert_eq!(position.core_report.local_degrees_of_freedom, 1);
    assert!(position.core_report.is_singular);
    assert!(matches!(
        linkage.velocity(first_driver, 1.0),
        Err(LinkageError::VelocityFailure(
            "differentiated hard equations did not validate"
        ))
    ));
}

#[test]
fn point_axis_joint_driver_and_monitor_removals_report_in_use_and_stale_ids() {
    let mut linkage = Linkage::new(1.0, xy_plane_frame()).unwrap();
    let ground = linkage.add_body("ground", Pose2::identity(), true).unwrap();
    let moving = linkage
        .add_body(
            "moving",
            Pose2 {
                translation: Vector2::new(1.0, 0.0),
                angle: 0.0,
            },
            false,
        )
        .unwrap();
    let ground_point = linkage
        .add_point_feature("ground.point", ground, Point2::origin())
        .unwrap();
    let moving_point = linkage
        .add_point_feature("moving.point", moving, Point2::origin())
        .unwrap();
    let ground_axis = linkage
        .add_axis_feature("ground.axis", ground, Vector2::x())
        .unwrap();
    let moving_axis = linkage
        .add_axis_feature("moving.axis", moving, Vector2::x())
        .unwrap();

    let joint = linkage
        .add_prismatic_joint(
            "guide",
            ground_point,
            ground_axis,
            moving_point,
            moving_axis,
            AxisDirectionBranch::Same,
        )
        .unwrap();
    assert!(matches!(
        linkage.remove_point_feature(moving_point),
        Err(LinkageError::PointFeatureInUse(id)) if id == moving_point
    ));
    assert!(matches!(
        linkage.remove_axis_feature(ground_axis),
        Err(LinkageError::AxisFeatureInUse(id)) if id == ground_axis
    ));
    linkage.remove_joint(joint).unwrap();
    assert!(matches!(
        linkage.remove_joint(joint),
        Err(LinkageError::UnknownJoint(id)) if id == joint
    ));

    let driver = linkage
        .add_linear_driver(
            "displacement",
            ground_point,
            moving_point,
            ground_axis,
            1.0,
            0.1,
        )
        .unwrap();
    assert!(matches!(
        linkage.remove_point_feature(moving_point),
        Err(LinkageError::PointFeatureInUse(id)) if id == moving_point
    ));
    assert!(matches!(
        linkage.remove_axis_feature(ground_axis),
        Err(LinkageError::AxisFeatureInUse(id)) if id == ground_axis
    ));
    linkage.remove_driver(driver).unwrap();
    assert!(matches!(
        linkage.remove_driver(driver),
        Err(LinkageError::UnknownDriver(id)) if id == driver
    ));

    let monitor = linkage
        .add_directed_displacement_branch_monitor(
            ground_point,
            moving_point,
            ground_axis,
            BranchSign::Positive,
        )
        .unwrap();
    assert!(matches!(
        linkage.remove_point_feature(moving_point),
        Err(LinkageError::PointFeatureInUse(id)) if id == moving_point
    ));
    assert!(matches!(
        linkage.remove_axis_feature(ground_axis),
        Err(LinkageError::AxisFeatureInUse(id)) if id == ground_axis
    ));
    linkage.remove_branch_monitor(monitor).unwrap();
    assert!(matches!(
        linkage.remove_branch_monitor(monitor),
        Err(LinkageError::UnknownBranchMonitor(id)) if id == monitor
    ));

    linkage.remove_point_feature(moving_point).unwrap();
    assert!(matches!(
        linkage.remove_point_feature(moving_point),
        Err(LinkageError::UnknownPointFeature(id)) if id == moving_point
    ));
    linkage.remove_axis_feature(ground_axis).unwrap();
    assert!(matches!(
        linkage.remove_axis_feature(ground_axis),
        Err(LinkageError::UnknownAxisFeature(id)) if id == ground_axis
    ));
}

#[test]
fn converged_opposite_four_bar_root_is_rejected_by_explicit_monitor_without_mutation() {
    let (mut open, open_ids) = four_bar_with_scale(FourBarAssemblyMode::Open, 1.0).unwrap();
    let (crossed, crossed_ids) = four_bar_with_scale(FourBarAssemblyMode::Crossed, 1.0).unwrap();
    open.set_body_pose(
        open_ids.coupler,
        crossed.body(crossed_ids.coupler).unwrap().pose(),
    )
    .unwrap();
    open.set_body_pose(
        open_ids.rocker,
        crossed.body(crossed_ids.rocker).unwrap().pose(),
    )
    .unwrap();
    let retained = open.geometry().unwrap();
    assert!(
        orientation(
            &retained,
            open_ids.input_a,
            open_ids.ground_o4,
            open_ids.coupler_b
        ) < 0.0
    );
    let evaluation = open
        .evaluate_branch_monitor(open_ids.orientation_monitor, &retained)
        .unwrap();
    assert_eq!(evaluation.expected_sign, BranchSign::Positive);
    assert!(evaluation.signed_metric < 0.0);
    assert!(!evaluation.retained);
    let result = open.solve(SolverConfig::default()).unwrap();
    assert_eq!(result.core_report.termination, SolveTermination::Converged);
    assert_eq!(result.core_report.hard_validity, HardValidity::Invalid);
    assert!(result.core_report.hard_residuals_validated);
    assert!(matches!(
        result.rejection,
        Some(SolveRejection::BranchViolation(
            geosolve_linkage::BranchViolation::Monitor(id)
        )) if id == open_ids.orientation_monitor
    ));
    assert_eq!(result.geometry, retained);
    assert_eq!(open.geometry().unwrap(), retained);
    let mapping = result
        .source_mappings
        .iter()
        .find(|mapping| mapping.source == LinkageSource::Joint(open_ids.b_joint))
        .unwrap();
    let display_source = result
        .display_audit
        .sources
        .iter()
        .find(|source| source.source_id == mapping.core_source_id)
        .unwrap();
    let VariableValue::Pose2(display_coupler) = display_source.rows[0].incident_variables[0].value
    else {
        panic!("expected coupler Pose2 audit value")
    };
    let retained_coupler = result.geometry.body_pose(open_ids.coupler).unwrap();
    assert_eq!(
        display_coupler[0].to_bits(),
        retained_coupler.translation.x.to_bits()
    );
    assert_eq!(
        display_coupler[1].to_bits(),
        retained_coupler.translation.y.to_bits()
    );
    assert_eq!(
        display_coupler[2].to_bits(),
        retained_coupler.angle.to_bits()
    );
    assert!(!display_source.annotations.redundant);
    assert!(!display_source.annotations.singular);
}

#[test]
fn unevaluable_domain_branch_monitor_marks_hard_validity_not_evaluated() {
    let mut linkage = Linkage::new(1.0e-310, xy_plane_frame()).unwrap();
    let ground = linkage.add_body("ground", Pose2::identity(), true).unwrap();
    let start = linkage
        .add_point_feature("start", ground, Point2::new(0.0, 0.0))
        .unwrap();
    let end = linkage
        .add_point_feature("end", ground, Point2::new(1.0, 0.0))
        .unwrap();
    let observed = linkage
        .add_point_feature("observed", ground, Point2::new(0.0, 1.0))
        .unwrap();
    linkage
        .add_orientation_branch_monitor(start, end, observed, BranchSign::Positive)
        .unwrap();
    let retained = linkage.geometry().unwrap();

    let result = linkage.solve(SolverConfig::default()).unwrap();

    assert_eq!(result.core_report.hard_validity, HardValidity::NotEvaluated);
    assert!(result.core_report.hard_residuals_validated);
    assert!(matches!(
        result.rejection,
        Some(SolveRejection::IndependentValidationFailed(_))
    ));
    assert_eq!(result.geometry, retained);
    assert_eq!(linkage.geometry().unwrap(), retained);
}

#[test]
fn impossible_grounded_geometry_does_not_claim_convergence_or_change_finite_state() {
    let mut linkage = Linkage::new(1.0, xy_plane_frame()).unwrap();
    let first = linkage.add_body("first", Pose2::identity(), true).unwrap();
    let second = linkage
        .add_body(
            "second",
            Pose2 {
                translation: Vector2::new(2.0, 0.0),
                angle: 0.0,
            },
            true,
        )
        .unwrap();
    let first_point = linkage
        .add_point_feature("first.point", first, Point2::origin())
        .unwrap();
    let second_point = linkage
        .add_point_feature("second.point", second, Point2::origin())
        .unwrap();
    linkage
        .add_revolute_joint("impossible", first_point, second_point)
        .unwrap();
    let retained = linkage.geometry().unwrap();
    let result = linkage.solve(SolverConfig::default()).unwrap();
    assert!(!result.accepted());
    assert_ne!(result.core_report.termination, SolveTermination::Converged);
    assert_eq!(result.geometry, retained);
    assert!(result.geometry.bodies.iter().all(|body| {
        body.pose.translation.iter().all(|value| value.is_finite()) && body.pose.angle.is_finite()
    }));
}

#[test]
fn known_near_toggle_warns_and_exact_toggle_rolls_back_on_branch_ambiguity() {
    let (mut linkage, ids) = four_bar_with_scale(FourBarAssemblyMode::Open, 1.0).unwrap();
    let near_target = PI - 1.0e-3;
    let near = linkage
        .drive_to(ids.driver, near_target, SolverConfig::default())
        .unwrap();
    assert!(near.completed(), "{near:#?}");
    let near_sample = near.samples.last().unwrap();
    assert_accepted(&near_sample.solve);
    assert!(
        near_sample.solve.diagnostics.has_rank_warning || near_sample.solve.core_report.is_singular,
        "known near-toggle target had no conditioning warning: {:#?}",
        near_sample.solve.diagnostics
    );
    assert_eq!(
        near_sample.solve.core_report.near_singular,
        near_sample
            .solve
            .core_report
            .component_solves
            .iter()
            .any(|component| component.near_singular)
    );
    if near_sample.solve.core_report.near_singular {
        assert!(
            near_sample
                .solve
                .core_report
                .component_solves
                .iter()
                .any(|component| {
                    component.near_singular
                        && component.near_singular_ratio.unwrap() <= component.near_singular_factor
                })
        );
    } else {
        // This frozen fixture can enter linkage's earlier 1e-3 conditioning band
        // without entering core M9's much tighter factor-100 rank-threshold band.
        assert!(
            near_sample
                .solve
                .diagnostics
                .singular_value_ratio
                .is_some_and(|ratio| ratio <= 1.0e-3)
        );
    }
    let retained_geometry = linkage.geometry().unwrap();
    assert!(retained_geometry.bodies.iter().all(|body| {
        body.pose.translation.iter().all(|value| value.is_finite()) && body.pose.angle.is_finite()
    }));
    let retained_target = linkage.driver(ids.driver).unwrap().target();
    assert!((retained_target - near_target).abs() <= f64::EPSILON);

    let exact = linkage
        .drive_to(ids.driver, PI, SolverConfig::default())
        .unwrap();
    assert!(!exact.completed());
    assert_eq!(exact.samples.len(), 1);
    let failed = &exact.samples[0].solve;
    assert_eq!(failed.core_report.termination, SolveTermination::Converged);
    assert_eq!(failed.core_report.hard_validity, HardValidity::Invalid);
    assert!(failed.core_report.hard_residuals_validated);
    assert!(matches!(
        failed.rejection,
        Some(SolveRejection::BranchViolation(_))
    ));
    assert_eq!(failed.geometry, retained_geometry);
    assert_eq!(linkage.geometry().unwrap(), retained_geometry);
    assert_eq!(
        linkage.driver(ids.driver).unwrap().target().to_bits(),
        retained_target.to_bits()
    );
}

#[test]
fn geometry_maps_features_through_the_validated_plane_frame() {
    let frame = PlaneFrame {
        origin: Point3::new(10.0, 20.0, 30.0),
        u: Vector3::y(),
        v: Vector3::z(),
    };
    let mut linkage = Linkage::new(1.0, frame).unwrap();
    let body = linkage
        .add_body(
            "body",
            Pose2 {
                translation: Vector2::new(2.0, 3.0),
                angle: PI / 2.0,
            },
            false,
        )
        .unwrap();
    let point = linkage
        .add_point_feature("point", body, Point2::new(1.0, 0.0))
        .unwrap();
    let axis = linkage
        .add_axis_feature("axis", body, Vector2::x())
        .unwrap();
    let geometry = linkage.geometry().unwrap();
    assert_point_close(geometry.point(point).unwrap(), Point2::new(2.0, 4.0), 1.0);
    assert!(
        (geometry.world_point(point).unwrap() - Point3::new(10.0, 22.0, 34.0)).norm() <= TOLERANCE
    );
    assert!((geometry.axis(axis).unwrap() - Vector2::y()).norm() <= TOLERANCE);
    assert!((geometry.world_axis(axis).unwrap() - Vector3::z()).norm() <= TOLERANCE);

    let compiled = linkage.compile().unwrap();
    let VariableValue::Pose2(values) = compiled
        .problem()
        .variable(compiled.variable_for_body(body).unwrap())
        .unwrap()
        .value()
    else {
        panic!("expected pose variable")
    };
    assert!((values[0] - 2.0).abs() <= f64::EPSILON);
    assert!((values[1] - 3.0).abs() <= f64::EPSILON);
    assert!((values[2] - PI / 2.0).abs() <= f64::EPSILON);
}
