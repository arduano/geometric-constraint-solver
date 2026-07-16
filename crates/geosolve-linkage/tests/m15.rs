// SPDX-License-Identifier: GPL-3.0-or-later

use std::f64::consts::PI;

use geosolve_core::{
    AuditEvaluationStatus, HardValidity, JacobianCheckReport, SolveTermination, SolverConfig,
};
use geosolve_geometry::{Point2, Pose2, Vector2};
use geosolve_linkage::{
    AxisDirectionBranch, BodyId, CompiledLinkage, FourBarAssemblyMode, Linkage, LinkageGeometry,
    LinkageSolveResult, LinkageSource, four_bar, slider_crank, xy_plane_frame,
};

const TOLERANCE: f64 = 1.0e-9;
const TRANSFORM_TOLERANCE: f64 = 2.0e-9;

fn pose(x: f64, y: f64, angle: f64) -> Pose2 {
    Pose2::try_new(Vector2::new(x, y), angle).unwrap()
}

fn assert_accepted(result: &LinkageSolveResult) {
    assert!(result.accepted(), "{:#?}", result.rejection);
    assert_eq!(result.core_report.termination, SolveTermination::Converged);
    assert_eq!(result.core_report.hard_validity, HardValidity::Valid);
    assert!(result.core_report.hard_residuals_validated);
    assert!(result.core_report.hard_residual_max <= TOLERANCE);
    assert!(result.acceptance_hard_residual_max.unwrap() <= TOLERANCE);
}

fn assert_vector_close(actual: Vector2<f64>, expected: Vector2<f64>, tolerance: f64) {
    let error = (actual - expected).norm();
    assert!(
        error <= tolerance,
        "vector error {error:e}: actual={actual:?}, expected={expected:?}"
    );
}

fn assert_pose_close(actual: Pose2, expected: Pose2, scale: f64) {
    let difference = expected.local_difference(&actual).unwrap();
    let translation_error = difference[0].hypot(difference[1]) / scale;
    assert!(
        translation_error <= TRANSFORM_TOLERANCE,
        "pose translation error {translation_error:e}: actual={actual:?}, expected={expected:?}"
    );
    assert!(
        difference[2].abs() <= TRANSFORM_TOLERANCE,
        "pose angle error {:e}: actual={actual:?}, expected={expected:?}",
        difference[2]
    );
}

fn left_transform(linkage: &mut Linkage, transform: Pose2) {
    let poses: Vec<_> = linkage
        .bodies()
        .map(|(body_id, body)| (body_id, body.pose()))
        .collect();
    for (body_id, body_pose) in poses {
        linkage
            .set_body_pose(body_id, transform.compose(&body_pose).unwrap())
            .unwrap();
    }
}

fn assert_residual_jacobians(compiled: &CompiledLinkage) -> JacobianCheckReport {
    let check = compiled.problem().check_jacobians(2.0e-6).unwrap();
    let ground_residuals: Vec<_> = compiled
        .source_mappings()
        .iter()
        .filter(|mapping| matches!(mapping.source, LinkageSource::Ground(_)))
        .flat_map(|mapping| mapping.residual_ids.iter().copied())
        .collect();
    for block in &check.blocks {
        if ground_residuals.contains(&block.residual_id) {
            // Exact manifold-fixed rows have analytically zero cross entries;
            // use absolute error because relative error is undefined at zero.
            assert!(
                block.max_absolute_error <= 1.0e-8,
                "ground FD block failed: {block:#?}"
            );
        } else {
            assert!(
                block.max_relative_error <= 1.0e-6,
                "linkage FD block failed: {block:#?}"
            );
        }
    }
    check
}

fn assert_left_transformed_geometry(
    original: &LinkageGeometry,
    transformed: &LinkageGeometry,
    transform: Pose2,
    scale: f64,
) {
    assert_eq!(original.bodies.len(), transformed.bodies.len());
    for original_body in &original.bodies {
        let expected = transform.compose(&original_body.pose).unwrap();
        assert_pose_close(
            transformed.body_pose(original_body.body_id).unwrap(),
            expected,
            scale,
        );
    }

    assert_eq!(original.points.len(), transformed.points.len());
    for original_point in &original.points {
        let transformed_point = transformed
            .points
            .iter()
            .find(|point| point.feature_id == original_point.feature_id)
            .unwrap();
        let expected_planar = transform.transform_point(original_point.planar);
        assert_vector_close(
            transformed_point.planar - expected_planar,
            Vector2::zeros(),
            TRANSFORM_TOLERANCE * scale,
        );
        let expected_world = transformed
            .plane_frame
            .try_map_point(expected_planar)
            .unwrap();
        assert!((transformed_point.world - expected_world).norm() <= TRANSFORM_TOLERANCE * scale);
    }

    assert_eq!(original.axes.len(), transformed.axes.len());
    for original_axis in &original.axes {
        let transformed_axis = transformed
            .axes
            .iter()
            .find(|axis| axis.feature_id == original_axis.feature_id)
            .unwrap();
        let expected_planar = transform.transform_vector(original_axis.planar);
        assert_vector_close(
            transformed_axis.planar,
            expected_planar,
            TRANSFORM_TOLERANCE,
        );
        let expected_world = transformed
            .plane_frame
            .try_map_vector(expected_planar)
            .unwrap();
        assert!((transformed_axis.world - expected_world).norm() <= TRANSFORM_TOLERANCE);
    }
}

#[allow(clippy::too_many_lines)]
fn all_residual_fixture() -> (Linkage, Vec<BodyId>) {
    let mut linkage = Linkage::new(4.0, xy_plane_frame()).unwrap();

    let revolute_ground_pose = pose(3.2, -1.7, 0.73);
    let revolute_body_pose = pose(-0.8, 2.6, -0.41);
    let revolute_ground = linkage
        .add_body("revolute ground", revolute_ground_pose, true)
        .unwrap();
    let revolute_body = linkage
        .add_body("revolute body", revolute_body_pose, false)
        .unwrap();
    let revolute_world_anchor = Point2::new(1.4, 0.9);
    let revolute_ground_anchor = linkage
        .add_point_feature(
            "revolute ground anchor",
            revolute_ground,
            revolute_ground_pose.inverse_transform_point(revolute_world_anchor),
        )
        .unwrap();
    let revolute_body_anchor = linkage
        .add_point_feature(
            "revolute body anchor",
            revolute_body,
            revolute_body_pose.inverse_transform_point(revolute_world_anchor),
        )
        .unwrap();
    linkage
        .add_revolute_joint(
            "transformed revolute",
            revolute_ground_anchor,
            revolute_body_anchor,
        )
        .unwrap();
    linkage
        .add_angular_driver(
            "transformed angle",
            revolute_ground,
            revolute_body,
            revolute_body_pose.angle - revolute_ground_pose.angle,
            0.1,
        )
        .unwrap();

    let prismatic_ground_pose = pose(-4.1, 3.3, -0.62);
    let guide_axis = Vector2::new(0.8, 0.6);
    let displacement = 2.3;
    let prismatic_body_pose = Pose2::try_new(
        prismatic_ground_pose.translation
            + prismatic_ground_pose.transform_vector(guide_axis * displacement),
        prismatic_ground_pose.angle,
    )
    .unwrap();
    let prismatic_ground = linkage
        .add_body("prismatic ground", prismatic_ground_pose, true)
        .unwrap();
    let prismatic_body = linkage
        .add_body("prismatic body", prismatic_body_pose, false)
        .unwrap();
    let common_anchor = Point2::new(0.35, -0.55);
    let prismatic_ground_anchor = linkage
        .add_point_feature("prismatic ground anchor", prismatic_ground, common_anchor)
        .unwrap();
    let prismatic_body_anchor = linkage
        .add_point_feature("prismatic body anchor", prismatic_body, common_anchor)
        .unwrap();
    let prismatic_ground_axis = linkage
        .add_axis_feature("prismatic ground axis", prismatic_ground, guide_axis)
        .unwrap();
    let prismatic_body_axis = linkage
        .add_axis_feature("prismatic body axis", prismatic_body, guide_axis)
        .unwrap();
    linkage
        .add_prismatic_joint(
            "transformed prismatic",
            prismatic_ground_anchor,
            prismatic_ground_axis,
            prismatic_body_anchor,
            prismatic_body_axis,
            AxisDirectionBranch::Same,
        )
        .unwrap();
    linkage
        .add_linear_driver(
            "transformed displacement",
            prismatic_ground_anchor,
            prismatic_body_anchor,
            prismatic_ground_axis,
            displacement,
            0.2,
        )
        .unwrap();

    let weld_ground_pose = pose(6.0, 4.5, 1.17);
    let weld_body_pose = pose(3.7, -2.8, -1.04);
    let weld_ground = linkage
        .add_body("weld ground", weld_ground_pose, true)
        .unwrap();
    let weld_body = linkage
        .add_body("weld body", weld_body_pose, false)
        .unwrap();
    let weld_world_anchor = Point2::new(5.2, 1.1);
    let weld_ground_anchor = linkage
        .add_point_feature(
            "weld ground anchor",
            weld_ground,
            weld_ground_pose.inverse_transform_point(weld_world_anchor),
        )
        .unwrap();
    let weld_body_anchor = linkage
        .add_point_feature(
            "weld body anchor",
            weld_body,
            weld_body_pose.inverse_transform_point(weld_world_anchor),
        )
        .unwrap();
    linkage
        .add_weld_joint_with_angle(
            "transformed weld",
            weld_ground_anchor,
            weld_body_anchor,
            weld_body_pose.angle - weld_ground_pose.angle,
        )
        .unwrap();

    (linkage, vec![revolute_body, prismatic_body, weld_body])
}

#[test]
fn every_linkage_residual_matches_retraction_fd_at_transformed_poses() {
    let (mut linkage, moving_bodies) = all_residual_fixture();
    let exact = linkage.compile().unwrap();
    assert_residual_jacobians(&exact);

    let ground_mappings: Vec<_> = exact
        .source_mappings()
        .iter()
        .filter(|mapping| matches!(mapping.source, LinkageSource::Ground(_)))
        .collect();
    assert_eq!(ground_mappings.len(), 3);
    let audit = exact.problem().audit_snapshot().unwrap();
    for mapping in ground_mappings {
        let source = audit
            .sources
            .iter()
            .find(|source| source.source_id == mapping.core_source_id)
            .unwrap();
        assert_eq!(source.rows.len(), 3);
        assert!(source.rows.iter().all(|row| {
            row.template.contains("local_difference(accepted_pose")
                && row.evaluation_status == AuditEvaluationStatus::Evaluated
                && row.raw_residual.abs() <= f64::EPSILON
                && row.normalized_residual.abs() <= f64::EPSILON
        }));
    }

    let perturbations = [
        [0.11, -0.07, 0.09],
        [-0.08, 0.13, -0.12],
        [0.06, 0.04, 0.14],
    ];
    for (body_id, delta) in moving_bodies.into_iter().zip(perturbations) {
        let body_pose = linkage.body(body_id).unwrap().pose();
        linkage
            .set_body_pose(body_id, body_pose.retract(delta).unwrap())
            .unwrap();
    }
    let perturbed = linkage.compile().unwrap();
    let check = assert_residual_jacobians(&perturbed);
    for mapping in perturbed.source_mappings() {
        for residual_id in &mapping.residual_ids {
            assert!(
                check
                    .blocks
                    .iter()
                    .any(|block| block.residual_id == *residual_id),
                "source {:?} residual {residual_id:?} was not checked",
                mapping.source
            );
        }
    }

    let solved = linkage.solve(SolverConfig::default()).unwrap();
    assert_accepted(&solved);
    assert_eq!(solved.core_report.rank, 9);
    assert_eq!(solved.core_report.left_nullity, 0);
    assert_eq!(solved.core_report.right_nullity, 0);
}

#[test]
fn l1_l2_common_left_se2_preserves_geometry_branch_rank_and_source_order() {
    let transform = pose(7.3, -4.8, 0.91);
    for mode in [FourBarAssemblyMode::Open, FourBarAssemblyMode::Crossed] {
        let (mut original, ids) = four_bar(mode).unwrap();
        let original_solve = original.solve(SolverConfig::default()).unwrap();
        assert_accepted(&original_solve);

        let mut transformed = original.clone();
        left_transform(&mut transformed, transform);
        let transformed_solve = transformed.solve(SolverConfig::default()).unwrap();
        assert_accepted(&transformed_solve);

        assert_left_transformed_geometry(
            &original_solve.geometry,
            &transformed_solve.geometry,
            transform,
            original.model_scale(),
        );
        assert_pose_close(
            transformed_solve.geometry.body_pose(ids.ground).unwrap(),
            transform,
            original.model_scale(),
        );
        assert_eq!(
            transformed_solve.source_mappings,
            original_solve.source_mappings
        );
        assert_eq!(
            transformed_solve.core_report.rank,
            original_solve.core_report.rank
        );
        assert_eq!(
            transformed_solve.core_report.left_nullity,
            original_solve.core_report.left_nullity
        );
        assert_eq!(
            transformed_solve.core_report.right_nullity,
            original_solve.core_report.right_nullity
        );
        let original_branch = original
            .evaluate_branch_monitor(ids.orientation_monitor, &original_solve.geometry)
            .unwrap();
        let transformed_branch = transformed
            .evaluate_branch_monitor(ids.orientation_monitor, &transformed_solve.geometry)
            .unwrap();
        assert_eq!(transformed_branch.kind, original_branch.kind);
        assert_eq!(
            transformed_branch.expected_sign,
            original_branch.expected_sign
        );
        assert_eq!(transformed_branch.retained, original_branch.retained);
        assert!(
            (transformed_branch.signed_metric - original_branch.signed_metric).abs()
                <= TRANSFORM_TOLERANCE
        );
    }
}

#[test]
fn l3_world_body_velocity_is_left_se2_equivariant_and_matches_continuation() {
    let target = 75.0 * PI / 180.0;
    let transform = pose(-5.4, 8.2, -0.77);
    let (mut original, ids) = slider_crank().unwrap();
    let original_drive = original
        .drive_to(ids.driver, target, SolverConfig::default())
        .unwrap();
    assert!(original_drive.completed(), "{original_drive:#?}");
    let original_geometry = original.geometry().unwrap();
    let original_velocity = original.velocity(ids.driver, 1.0).unwrap();

    let mut transformed = original.clone();
    left_transform(&mut transformed, transform);
    let transformed_solve = transformed.solve(SolverConfig::default()).unwrap();
    assert_accepted(&transformed_solve);
    assert_left_transformed_geometry(
        &original_geometry,
        &transformed_solve.geometry,
        transform,
        original.model_scale(),
    );
    let transformed_velocity = transformed.velocity(ids.driver, 1.0).unwrap();
    assert_eq!(transformed_velocity.rank, original_velocity.rank);
    assert_eq!(
        transformed_velocity.local_degrees_of_freedom,
        original_velocity.local_degrees_of_freedom
    );
    assert!(transformed_velocity.differentiated_residual_max <= TOLERANCE);
    for original_body_velocity in &original_velocity.body_velocities {
        let transformed_body_velocity = transformed_velocity
            .body(original_body_velocity.body_id)
            .unwrap();
        assert_vector_close(
            transformed_body_velocity.linear,
            transform.transform_vector(original_body_velocity.linear),
            TRANSFORM_TOLERANCE,
        );
        assert!(
            (transformed_body_velocity.angular - original_body_velocity.angular).abs()
                <= TRANSFORM_TOLERANCE
        );
    }

    let oracle_step = 1.0e-5;
    let mut plus = transformed.clone();
    let mut minus = transformed.clone();
    assert!(
        plus.drive_to(ids.driver, target + oracle_step, SolverConfig::default())
            .unwrap()
            .completed()
    );
    assert!(
        minus
            .drive_to(ids.driver, target - oracle_step, SolverConfig::default())
            .unwrap()
            .completed()
    );
    let plus_geometry = plus.geometry().unwrap();
    let minus_geometry = minus.geometry().unwrap();
    for (body_id, body) in transformed.bodies().filter(|(_, body)| !body.grounded()) {
        let analytic = transformed_velocity.body(body_id).unwrap();
        let plus_pose = plus_geometry.body_pose(body_id).unwrap();
        let minus_pose = minus_geometry.body_pose(body_id).unwrap();
        let numeric_linear = (plus_pose.translation - minus_pose.translation) / (2.0 * oracle_step);
        let numeric_angular = (plus_pose.angle - minus_pose.angle) / (2.0 * oracle_step);
        let linear_error = (analytic.linear - numeric_linear).norm() / original.model_scale();
        let angular_error = (analytic.angular - numeric_angular).abs();
        assert!(
            linear_error <= 5.0e-4,
            "body {body_id:?} ({}) linear oracle error {linear_error:e}",
            body.label()
        );
        assert!(
            angular_error <= 5.0e-4,
            "body {body_id:?} ({}) angular oracle error {angular_error:e}",
            body.label()
        );
    }
}

#[test]
fn branch_monitors_are_common_left_se2_equivariant() {
    let transform = pose(4.6, -9.1, 1.31);

    let (four_bar, four_bar_ids) = four_bar(FourBarAssemblyMode::Open).unwrap();
    let original_four_bar_geometry = four_bar.geometry().unwrap();
    let original_orientation = four_bar
        .evaluate_branch_monitor(
            four_bar_ids.orientation_monitor,
            &original_four_bar_geometry,
        )
        .unwrap();
    let mut transformed_four_bar = four_bar.clone();
    left_transform(&mut transformed_four_bar, transform);
    let transformed_orientation = transformed_four_bar
        .evaluate_branch_monitor(
            four_bar_ids.orientation_monitor,
            &transformed_four_bar.geometry().unwrap(),
        )
        .unwrap();
    assert_eq!(transformed_orientation.kind, original_orientation.kind);
    assert_eq!(
        transformed_orientation.expected_sign,
        original_orientation.expected_sign
    );
    assert_eq!(
        transformed_orientation.retained,
        original_orientation.retained
    );
    assert!(
        (transformed_orientation.signed_metric - original_orientation.signed_metric).abs()
            <= TRANSFORM_TOLERANCE
    );

    let (slider_crank, slider_ids) = slider_crank().unwrap();
    let original_slider_geometry = slider_crank.geometry().unwrap();
    let original_displacement = slider_crank
        .evaluate_branch_monitor(slider_ids.positive_x_monitor, &original_slider_geometry)
        .unwrap();
    let mut transformed_slider = slider_crank.clone();
    left_transform(&mut transformed_slider, transform);
    let transformed_displacement = transformed_slider
        .evaluate_branch_monitor(
            slider_ids.positive_x_monitor,
            &transformed_slider.geometry().unwrap(),
        )
        .unwrap();
    assert_eq!(transformed_displacement.kind, original_displacement.kind);
    assert_eq!(
        transformed_displacement.expected_sign,
        original_displacement.expected_sign
    );
    assert_eq!(
        transformed_displacement.retained,
        original_displacement.retained
    );
    assert!(
        (transformed_displacement.signed_metric - original_displacement.signed_metric).abs()
            <= TRANSFORM_TOLERANCE
    );
}
