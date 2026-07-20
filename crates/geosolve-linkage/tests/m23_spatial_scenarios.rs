// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_core::{
    HardValidity, LinearSolveBackend, LinearSolveBackendPolicy, SolveTermination, SolverConfig,
};
use geosolve_geometry::{Frame3, Point3, Pose3, Vector3};
use geosolve_linkage::{
    SpatialAssembly, SpatialAssemblyEdit, SpatialAssemblySession, SpatialAssemblyTransaction,
    SpatialAxisParity, SpatialHingeTarget, SpatialModeSign, SpatialPlanarTranslationAxis,
};

#[test]
fn nonplanar_universal_ring_retains_chirality_and_two_internal_motions() {
    for (scale_index, scale) in [1.0e-6, 1.0, 1.0e6].into_iter().enumerate() {
        let (assembly, monitor) = universal_ring(scale);
        if scale_index == 1 {
            let compiled = assembly.compile().unwrap();
            let check = compiled.check_jacobians(1.0e-6).unwrap();
            assert!(check.max_absolute_error() <= 1.0e-6);
        }
        let mut session = SpatialAssemblySession::new(assembly, SolverConfig::default()).unwrap();
        let result = session.accepted_result();
        assert_eq!(result.source_mappings.len(), 5);
        assert_eq!(result.core_report.rank, 16);
        assert_eq!(result.core_report.left_nullity, 0);
        assert_eq!(result.core_report.right_nullity, 2);
        assert_eq!(result.core_report.structural_nnz, 144);
        assert_eq!(session.gauge_report().gauge_dof, 0);
        assert_eq!(session.gauge_report().internal_mobility, 2);
        let chirality = session.mode_evaluation(monitor).unwrap();
        assert!(chirality.retained);
        assert!(chirality.retained_normalized_metric > 0.2);
        assert!(result.acceptance_hard_residual_max <= 1.0e-9);

        let revision = session.revision();
        let retained = session.accepted_result().clone();
        assert!(
            session
                .apply_transaction(SpatialAssemblyTransaction::one(
                    revision,
                    SpatialAssemblyEdit::MonitorSignedVolumeOrientation {
                        monitor,
                        orientation: SpatialModeSign::Negative,
                    },
                ))
                .is_err()
        );
        assert_eq!(session.revision(), revision);
        assert_eq!(session.accepted_result(), &retained);
    }
}

#[test]
fn mixed_scale_stage_tool_stack_is_finite_full_rank_and_mode_stable() {
    for scale in [1.0e-6, 1.0, 1.0e6] {
        let assembly = mixed_scale_stage_tool(scale);
        let session = SpatialAssemblySession::new(assembly, SolverConfig::default()).unwrap();
        let result = session.accepted_result();
        assert_eq!(result.source_mappings.len(), 6);
        assert_eq!(result.mode_evaluations.len(), 2);
        assert!(result.mode_evaluations.iter().all(|mode| mode.retained));
        assert_eq!(result.core_report.rank, 12);
        assert_eq!(result.core_report.left_nullity, 0);
        assert_eq!(result.core_report.right_nullity, 0);
        assert_eq!(result.core_report.structural_nnz, 108);
        assert_eq!(session.gauge_report().gauge_dof, 0);
        assert_eq!(session.gauge_report().internal_mobility, 0);
        assert!(result.acceptance_hard_residual_max <= 1.0e-9);
        assert!(result.core_report.audit.sources.iter().all(|source| {
            source.rows.iter().all(|row| {
                row.raw_residual.is_finite()
                    && row.normalized_residual.is_finite()
                    && row.scale.is_finite()
                    && row.scale > 0.0
            })
        }));
    }
}

#[test]
fn large_spatial_chain_uses_sparse_steps_without_changing_dense_rank_policy() {
    let (assembly, last_body) = large_sparse_chain();
    let mut config = SolverConfig {
        linear_solve_backend: LinearSolveBackendPolicy::SparsePreferred,
        ..SolverConfig::default()
    };
    config.redundancy_diagnostic_budget.enabled = false;
    config.conflict_diagnostic_budget.enabled = false;
    let session = SpatialAssemblySession::new(assembly, config).unwrap();
    let accepted = session.accepted_result();
    assert_eq!(accepted.source_mappings.len(), 44);
    assert_eq!(accepted.core_report.rank, 258);
    assert_eq!(accepted.core_report.left_nullity, 0);
    assert_eq!(accepted.core_report.right_nullity, 0);
    assert_eq!(accepted.core_report.structural_nnz, 3_060);
    assert_eq!(session.gauge_report().gauge_dof, 0);
    assert_eq!(session.gauge_report().internal_mobility, 0);
    assert!(accepted.acceptance_hard_residual_max <= 1.0e-9);

    let variable = session
        .body_variables()
        .iter()
        .find(|mapping| mapping.body_id == last_body)
        .unwrap()
        .variable_id;
    let mut problem = session.core_session().problem().clone();
    problem
        .apply_local_increment(variable, &[0.02, 0.0, 0.0, 0.0, 0.0, 0.0])
        .unwrap();
    let report = problem.solve(config).unwrap();
    assert_eq!(report.termination, SolveTermination::Converged);
    assert_eq!(report.hard_validity, HardValidity::Valid);
    assert!(report.hard_residual_max <= 1.0e-9);
    assert_eq!(report.rank, 258);
    assert_eq!(report.left_nullity, 0);
    assert_eq!(report.right_nullity, 0);
    assert_eq!(report.structural_nnz, 3_060);
    assert_eq!(
        report.requested_backend,
        LinearSolveBackendPolicy::SparsePreferred
    );
    assert_eq!(report.actual_backend, Some(LinearSolveBackend::SparseQr));
    assert_eq!(report.sparse_fallback_reason, None);
}

fn universal_ring(scale: f64) -> (SpatialAssembly, geosolve_linkage::SpatialModeMonitorId) {
    let points = [
        Point3::new(0.0, 0.0, 0.0),
        Point3::new(2.0 * scale, 0.0, 0.0),
        Point3::new(2.0 * scale, 2.0 * scale, 1.0 * scale),
        Point3::new(0.0, 2.0 * scale, 2.0 * scale),
    ];
    let origins = [
        midpoint(points[3], points[0]),
        midpoint(points[0], points[1]),
        midpoint(points[1], points[2]),
        midpoint(points[2], points[3]),
    ];
    let mut assembly = SpatialAssembly::new(scale).unwrap();
    let bodies = origins.map(|origin| {
        assembly
            .add_body(
                "ring body",
                Pose3::try_new(origin.coords, [1.0, 0.0, 0.0, 0.0]).unwrap(),
            )
            .unwrap()
    });
    assembly
        .add_physical_ground("ring ground", bodies[0])
        .unwrap();

    let third_joint_axis = (Vector3::x() + Vector3::y()).normalize();
    let fourth_joint_axis = (Vector3::x() + Vector3::z()).normalize();
    let endpoints = [
        (bodies[0], bodies[1], Vector3::x(), Vector3::y()),
        (bodies[1], bodies[2], Vector3::z(), Vector3::x()),
        (bodies[2], bodies[3], third_joint_axis, Vector3::z()),
        (bodies[3], bodies[0], fourth_joint_axis, Vector3::y()),
    ];
    for (index, (first_body, second_body, first_axis, second_axis)) in
        endpoints.into_iter().enumerate()
    {
        let first = assembly
            .add_axis_feature(
                "ring joint first",
                first_body,
                local_axis_frame(origins[index], points[index], first_axis),
            )
            .unwrap();
        let second_index = (index + 1) % 4;
        let second = assembly
            .add_axis_feature(
                "ring joint second",
                second_body,
                local_axis_frame(origins[second_index], points[index], second_axis),
            )
            .unwrap();
        assembly
            .add_universal_joint("ring universal", first, second)
            .unwrap();
    }
    let witnesses = std::array::from_fn(|index| {
        assembly
            .add_point_feature(
                "ring chirality witness",
                bodies[index],
                points[index] - origins[index].coords,
            )
            .unwrap()
    });
    let monitor = assembly
        .add_signed_volume_monitor(
            "positive ring chirality",
            witnesses,
            SpatialModeSign::Positive,
        )
        .unwrap();
    (assembly, monitor)
}

fn mixed_scale_stage_tool(scale: f64) -> SpatialAssembly {
    let macro_x = 1.0e6 * scale;
    let micro_y = 2.0e-6 * scale;
    let stage_pose = planar_pose(Vector3::new(macro_x, micro_y, 0.0), 0.41);
    let offset_rotation = Pose3::exp([0.0, 0.0, 0.0, 0.17, -0.11, 0.13]).unwrap();
    let offset_pose = Pose3::try_new(
        Vector3::new(3.0e-6 * scale, -4.0e-6 * scale, 5.0e-6 * scale),
        offset_rotation.quaternion(),
    )
    .unwrap();
    let tool_pose = stage_pose.compose(&offset_pose).unwrap();
    let identity = identity_frame(Point3::origin());
    let offset_frame = frame_from_pose(offset_pose);

    let mut assembly = SpatialAssembly::new(scale).unwrap();
    let base = assembly.add_body("base", Pose3::identity()).unwrap();
    let stage = assembly.add_body("stage", stage_pose).unwrap();
    let tool = assembly.add_body("tool", tool_pose).unwrap();
    let base_plane = assembly
        .add_plane_feature("base plane", base, identity)
        .unwrap();
    let stage_plane = assembly
        .add_plane_feature("stage plane", stage, identity)
        .unwrap();
    let stage_frame = assembly
        .add_frame_feature("stage tool frame", stage, identity)
        .unwrap();
    let tool_frame = assembly
        .add_frame_feature("tool frame", tool, identity)
        .unwrap();
    let witness = assembly
        .add_point_feature(
            "stage side witness",
            stage,
            Point3::new(0.0, 0.0, 0.75 * scale),
        )
        .unwrap();
    assembly.add_physical_ground("base fixed", base).unwrap();
    let joint = assembly
        .add_planar_joint(
            "stage planar joint",
            base_plane,
            stage_plane,
            SpatialAxisParity::Aligned,
        )
        .unwrap();
    let hinge = assembly
        .add_hinge_coordinate("stage phase", joint, -2)
        .unwrap();
    let x = assembly
        .add_planar_translation_coordinate("stage X", joint, SpatialPlanarTranslationAxis::X)
        .unwrap();
    let y = assembly
        .add_planar_translation_coordinate("stage Y", joint, SpatialPlanarTranslationAxis::Y)
        .unwrap();
    assembly
        .add_hinge_position_driver(
            "stage phase driver",
            hinge,
            SpatialHingeTarget {
                principal_phase: 0.41,
                winding: -2,
            },
        )
        .unwrap();
    assembly
        .add_translation_position_driver("stage X driver", x, macro_x)
        .unwrap();
    assembly
        .add_translation_position_driver("stage Y driver", y, micro_y)
        .unwrap();
    assembly
        .add_frame_offset_mate("stage tool offset", stage_frame, tool_frame, offset_frame)
        .unwrap();
    assembly
        .add_hinge_winding_monitor("stage winding", hinge, -2)
        .unwrap();
    assembly
        .add_plane_side_monitor(
            "stage positive side",
            base_plane,
            witness,
            SpatialModeSign::Positive,
        )
        .unwrap();
    assembly
}

fn large_sparse_chain() -> (SpatialAssembly, geosolve_linkage::SpatialBodyId) {
    const MOVING_BODIES: usize = 43;
    let mut assembly = SpatialAssembly::new(1.0).unwrap();
    let ground = assembly
        .add_body("chain ground", Pose3::identity())
        .unwrap();
    let ground_frame = assembly
        .add_frame_feature(
            "chain ground frame",
            ground,
            identity_frame(Point3::origin()),
        )
        .unwrap();
    assembly
        .add_physical_ground("chain ground fixed", ground)
        .unwrap();
    let mut bodies = Vec::with_capacity(MOVING_BODIES);
    let mut previous_body = ground;
    let mut previous_origin = Point3::origin();
    for index in 0..MOVING_BODIES {
        let position = Vector3::new(f64::from(u32::try_from(index).unwrap()), 0.0, 0.0);
        let body = assembly
            .add_body(
                "chain body",
                Pose3::try_new(position, [1.0, 0.0, 0.0, 0.0]).unwrap(),
            )
            .unwrap();
        let current_origin = Point3::from(position);
        let shared = if index == 0 {
            Point3::origin()
        } else {
            midpoint(previous_origin, current_origin)
        };
        let previous_frame = if index == 0 {
            ground_frame
        } else {
            assembly
                .add_frame_feature(
                    "chain previous frame",
                    previous_body,
                    identity_frame(shared - previous_origin.coords),
                )
                .unwrap()
        };
        let current_frame = assembly
            .add_frame_feature(
                "chain current frame",
                body,
                identity_frame(shared - current_origin.coords),
            )
            .unwrap();
        assembly
            .add_fixed_frame("chain fixed link", previous_frame, current_frame)
            .unwrap();
        bodies.push(body);
        previous_body = body;
        previous_origin = current_origin;
    }
    (assembly, *bodies.last().unwrap())
}

fn local_axis_frame(body_origin: Point3<f64>, joint: Point3<f64>, z: Vector3<f64>) -> Frame3 {
    let x = if z.dot(&Vector3::x()).abs() < 0.9 {
        Vector3::x().cross(&z).normalize()
    } else {
        Vector3::y().cross(&z).normalize()
    };
    let y = z.cross(&x).normalize();
    Frame3::try_new(joint - body_origin.coords, x, y, z).unwrap()
}

fn midpoint(first: Point3<f64>, second: Point3<f64>) -> Point3<f64> {
    Point3::from((first.coords + second.coords) * 0.5)
}

fn planar_pose(translation: Vector3<f64>, angle: f64) -> Pose3 {
    let half = 0.5 * angle;
    Pose3::try_new(translation, [half.cos(), 0.0, 0.0, half.sin()]).unwrap()
}

fn identity_frame(origin: Point3<f64>) -> Frame3 {
    Frame3::try_new(origin, Vector3::x(), Vector3::y(), Vector3::z()).unwrap()
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
