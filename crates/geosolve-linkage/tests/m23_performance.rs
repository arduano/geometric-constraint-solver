// SPDX-License-Identifier: GPL-3.0-or-later

use std::time::{Duration, Instant};

use geosolve_core::{
    HardValidity, LinearSolveBackend, LinearSolveBackendPolicy, SolveTermination, SolverConfig,
};
use geosolve_geometry::{Frame3, Point3, Pose3, Vector3};
use geosolve_linkage::{SpatialAssembly, SpatialAssemblySession, SpatialBodyId};

#[test]
fn spatial_performance_corpus_shapes_are_deterministic() {
    for moving_bodies in [43, 255, 256] {
        let (assembly, _) = fixed_frame_chain(moving_bodies);
        let compiled = assembly.compile().unwrap();
        assert_eq!(compiled.body_variables().len(), moving_bodies + 1);
        assert_eq!(compiled.source_mappings().len(), moving_bodies + 1);
    }
}

#[test]
#[ignore = "explicit release performance gate; dense-authoritative rank is intentionally expensive"]
fn exact_auto_sparse_crossover_solves_and_validates_256_moving_body_chain() {
    const MOVING_BODIES: usize = 256;
    const RELEASE_BUDGET: Duration = Duration::from_secs(180);
    let (assembly, last_body) = fixed_frame_chain(MOVING_BODIES);
    let mut config = SolverConfig {
        linear_solve_backend: LinearSolveBackendPolicy::Auto,
        ..SolverConfig::default()
    };
    config.redundancy_diagnostic_budget.enabled = false;
    config.conflict_diagnostic_budget.enabled = false;

    let started = Instant::now();
    let session = SpatialAssemblySession::new(assembly, config).unwrap();
    assert_eq!(
        session.accepted_result().core_report.rank,
        6 * MOVING_BODIES
    );
    assert_eq!(session.accepted_result().core_report.left_nullity, 0);
    assert_eq!(session.accepted_result().core_report.right_nullity, 0);
    assert!(session.accepted_result().acceptance_hard_residual_max <= 1.0e-9);

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
    let elapsed = started.elapsed();
    assert_eq!(report.termination, SolveTermination::Converged);
    assert_eq!(report.hard_validity, HardValidity::Valid);
    assert_eq!(report.rank, 6 * MOVING_BODIES);
    assert_eq!(report.actual_backend, Some(LinearSolveBackend::SparseQr));
    assert_eq!(report.sparse_fallback_reason, None);
    assert!(report.hard_residual_max <= 1.0e-9);
    assert!(
        elapsed <= RELEASE_BUDGET,
        "256-moving-body release corpus took {elapsed:?}"
    );
}

fn fixed_frame_chain(moving_bodies: usize) -> (SpatialAssembly, SpatialBodyId) {
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
    let mut previous_body = ground;
    let mut previous_origin = Point3::origin();
    let mut last_body = ground;
    for index in 0..moving_bodies {
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
            Point3::from((previous_origin.coords + current_origin.coords) * 0.5)
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
        previous_body = body;
        previous_origin = current_origin;
        last_body = body;
    }
    (assembly, last_body)
}

fn identity_frame(origin: Point3<f64>) -> Frame3 {
    Frame3::try_new(origin, Vector3::x(), Vector3::y(), Vector3::z()).unwrap()
}
