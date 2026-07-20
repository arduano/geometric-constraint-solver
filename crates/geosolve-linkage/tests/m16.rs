// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_core::{
    AdaptiveStepPolicy, HardValidity, LinearSolveBackend, LinearSolveBackendPolicy,
    SolveTermination, SolverConfig,
};
use geosolve_geometry::{Pose2, Vector2};
use geosolve_linkage::{
    AdaptiveContinuationMode, AdaptiveContinuationRequest, AdaptiveContinuationStatus,
    ContinuationDirection, FourBarAssemblyMode, Linkage, LinkageGeometry, LinkageSource,
    SolveRejection, four_bar_with_scale, slider_crank_displacement_driven_with_scale,
};

const TOLERANCE: f64 = 1.0e-9;

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

fn left_transform(linkage: &mut Linkage, transform: Pose2) {
    let poses = linkage
        .bodies()
        .map(|(body_id, body)| (body_id, body.pose()))
        .collect::<Vec<_>>();
    for (body_id, pose) in poses {
        linkage
            .set_body_pose(body_id, transform.compose(&pose).unwrap())
            .unwrap();
    }
}

fn assert_physical_samples(result: &geosolve_linkage::AdaptiveContinuationResult) {
    assert!(
        result.initial_solve.accepted(),
        "{:#?}",
        result.initial_solve.rejection
    );
    for sample in &result.samples {
        assert!(sample.solve.accepted(), "{:#?}", sample.solve.rejection);
        assert_eq!(
            sample.solve.core_report.termination,
            SolveTermination::Converged
        );
        assert_eq!(sample.solve.core_report.hard_validity, HardValidity::Valid);
        assert!(sample.solve.core_report.hard_residuals_validated);
        assert!(sample.solve.core_report.hard_residual_max <= TOLERANCE);
        assert!(sample.solve.acceptance_hard_residual_max.unwrap() <= TOLERANCE);
        assert!(sample.path_step.is_finite() && sample.path_step > 0.0);
        assert!(sample.correction_norm.is_finite() && sample.correction_norm >= 0.0);
        assert!(sample.tangent_parameter_component.is_finite());
        assert!(sample.solve.source_mappings.iter().all(|mapping| {
            !mapping.source_label.contains("pseudo-arclength")
                && !mapping.source_label.contains("continuation parameter")
        }));
        assert!(sample.solve.core_report.audit.sources.iter().all(|source| {
            !source.source_label.contains("pseudo-arclength")
                && !source.source_label.contains("continuation parameter")
        }));
    }
}

fn assert_endpoint_matches_direct_physical_solve(
    linkage: &Linkage,
    result: &geosolve_linkage::AdaptiveContinuationResult,
    config: SolverConfig,
) {
    let endpoint = &result.samples.last().unwrap().solve;
    let mut direct_linkage = linkage.clone();
    let direct = direct_linkage.solve(config).unwrap();
    assert!(direct.accepted(), "{direct:#?}");
    assert_eq!(endpoint.geometry, direct.geometry);
    assert_eq!(endpoint.source_mappings, direct.source_mappings);
    assert_eq!(endpoint.display_audit, direct.display_audit);
    assert_eq!(endpoint.core_report.audit, direct.core_report.audit);
    assert_eq!(endpoint.diagnostics, direct.diagnostics);
    assert_eq!(endpoint.core_report.rank, direct.core_report.rank);
    assert_eq!(
        endpoint.core_report.left_nullity,
        direct.core_report.left_nullity
    );
    assert_eq!(
        endpoint.core_report.right_nullity,
        direct.core_report.right_nullity
    );
    assert_eq!(
        endpoint.core_report.structural,
        direct.core_report.structural
    );
    assert_eq!(
        endpoint.core_report.structural_nnz,
        direct.core_report.structural_nnz
    );
}

fn assert_same_geometry(first: &LinkageGeometry, second: &LinkageGeometry, scale: f64) {
    assert_eq!(first.bodies.len(), second.bodies.len());
    for first_body in &first.bodies {
        let second_pose = second.body_pose(first_body.body_id).unwrap();
        let difference = first_body.pose.local_difference(&second_pose).unwrap();
        assert!(difference[0].hypot(difference[1]) / scale <= 2.0e-8);
        assert!(difference[2].abs() <= 2.0e-8);
    }
}

fn assert_same_audit_structure(
    first: &geosolve_core::AuditSnapshot,
    second: &geosolve_core::AuditSnapshot,
) {
    assert_eq!(first.sources.len(), second.sources.len());
    for (first_source, second_source) in first.sources.iter().zip(&second.sources) {
        assert_eq!(first_source.source_id, second_source.source_id);
        assert_eq!(first_source.source_label, second_source.source_label);
        assert_eq!(first_source.annotations, second_source.annotations);
        assert_eq!(first_source.active_bounds, second_source.active_bounds);
        assert_eq!(first_source.rows.len(), second_source.rows.len());
        for (first_row, second_row) in first_source.rows.iter().zip(&second_source.rows) {
            assert_eq!(first_row.residual_id, second_row.residual_id);
            assert_eq!(first_row.category, second_row.category);
            assert_eq!(first_row.row_in_block, second_row.row_in_block);
            assert_eq!(first_row.template, second_row.template);
            assert_eq!(first_row.bindings, second_row.bindings);
            assert_eq!(first_row.unit, second_row.unit);
            assert_eq!(first_row.scale.to_bits(), second_row.scale.to_bits());
            assert_eq!(first_row.evaluation_status, second_row.evaluation_status);
            assert_eq!(
                first_row.evaluation_error_category,
                second_row.evaluation_error_category
            );
            assert_eq!(first_row.evaluation_error, second_row.evaluation_error);
            assert_eq!(first_row.annotations, second_row.annotations);
            assert_eq!(first_row.active_bounds, second_row.active_bounds);
            assert_eq!(
                first_row
                    .incident_variables
                    .iter()
                    .map(|variable| variable.variable_id)
                    .collect::<Vec<_>>(),
                second_row
                    .incident_variables
                    .iter()
                    .map(|variable| variable.variable_id)
                    .collect::<Vec<_>>()
            );
            assert!(first_row.normalized_residual.abs() <= TOLERANCE);
            assert!(second_row.normalized_residual.abs() <= TOLERANCE);
        }
    }
}

#[test]
fn displacement_driven_l3_fixture_has_the_documented_start_and_physical_audit() {
    for scale in [1.0e-6, 1.0, 1.0e6] {
        let (mut linkage, ids) = slider_crank_displacement_driven_with_scale(scale).unwrap();
        let expected = 4.747_880_210_234_948 * scale;
        assert!((linkage.driver(ids.driver).unwrap().target() - expected).abs() / scale <= 1.0e-14);
        assert!((linkage.body(ids.crank).unwrap().pose().angle - 0.05).abs() <= f64::EPSILON);

        let solve = linkage.solve(SolverConfig::default()).unwrap();
        assert!(solve.accepted(), "{solve:#?}");
        assert_eq!(solve.core_report.rank, 9);
        assert_eq!(
            (
                solve.core_report.left_nullity,
                solve.core_report.right_nullity
            ),
            (0, 0)
        );
        let driver_mapping = solve
            .source_mappings
            .iter()
            .find(|mapping| mapping.source == LinkageSource::Driver(ids.driver))
            .unwrap();
        assert!(driver_mapping.source_label.contains("slider displacement"));
        assert!(solve.core_report.audit.sources.iter().all(|source| {
            !source.source_label.contains("pseudo-arclength")
                && !source.source_label.contains("continuation parameter")
        }));
    }
}

#[test]
fn natural_continuation_requires_pseudo_at_l3_fold_at_all_scales() {
    for scale in [1.0e-6, 1.0, 1.0e6] {
        let (mut linkage, ids) = slider_crank_displacement_driven_with_scale(scale).unwrap();
        assert!(linkage.solve(SolverConfig::default()).unwrap().accepted());
        let result = linkage
            .continue_driver(
                AdaptiveContinuationRequest {
                    driver_id: ids.driver,
                    mode: AdaptiveContinuationMode::Natural {
                        target: 4.751 * scale,
                    },
                    step_policy: policy(),
                },
                SolverConfig::default(),
            )
            .unwrap();

        assert!(!result.completed(), "scale={scale:e}, result={result:#?}");
        assert_eq!(
            result.status,
            AdaptiveContinuationStatus::PseudoArclengthRequired
        );
        assert!(
            !result.samples.is_empty(),
            "scale={scale:e}, result={result:#?}"
        );
        assert_physical_samples(&result);
        assert!(
            result
                .rejected_attempts
                .iter()
                .any(geosolve_linkage::LinkageSolveResult::accepted),
            "the accepted physical corrector rejected at the turning point must be retained"
        );
        assert!(
            result.accepted_target < (4.75 + 1.0e-8) * scale,
            "scale={scale:e}, accepted_target={:.17}",
            result.accepted_target,
        );
        assert!(result.accepted_target > result.initial_target);
        assert!(linkage.body(ids.crank).unwrap().pose().angle >= 0.0);
        assert_eq!(
            linkage.driver(ids.driver).unwrap().target().to_bits(),
            result.accepted_target.to_bits()
        );
        assert_eq!(
            linkage.geometry().unwrap(),
            result.samples.last().unwrap().solve.geometry
        );
    }
}

#[test]
fn explicit_pseudo_arclength_crosses_l3_fold_at_all_model_scales() {
    for scale in [1.0e-6, 1.0, 1.0e6] {
        let (mut linkage, ids) = slider_crank_displacement_driven_with_scale(scale).unwrap();
        assert!(linkage.solve(SolverConfig::default()).unwrap().accepted());
        let initial_target = linkage.driver(ids.driver).unwrap().target();
        let result = linkage
            .continue_driver(
                AdaptiveContinuationRequest {
                    driver_id: ids.driver,
                    mode: AdaptiveContinuationMode::PseudoArclength {
                        path_length: 0.2,
                        initial_direction: ContinuationDirection::IncreasingParameter,
                    },
                    step_policy: policy(),
                },
                SolverConfig::default(),
            )
            .unwrap();

        assert!(result.completed(), "scale={scale:e}, result={result:#?}");
        assert_physical_samples(&result);
        assert!((result.accepted_path_length - 0.2).abs() <= 1.0e-12);
        assert!(
            result
                .samples
                .iter()
                .any(|sample| sample.driver_target > initial_target)
        );
        assert!(
            result
                .samples
                .iter()
                .any(|sample| { sample.tangent_parameter_component < 0.0 })
        );
        assert!(linkage.body(ids.crank).unwrap().pose().angle < 0.0);
        assert!(result.accepted_target < 4.75 * scale);
        let geometry = linkage.geometry().unwrap();
        let branch = linkage
            .evaluate_branch_monitor(ids.positive_x_monitor, &geometry)
            .unwrap();
        assert!(branch.retained);
        assert!(
            geometry.bodies.iter().all(|body| body
                .pose
                .ambient()
                .iter()
                .all(|value| value.is_finite()))
        );
        assert_endpoint_matches_direct_physical_solve(&linkage, &result, SolverConfig::default());
    }
}

#[test]
fn pseudo_arclength_crosses_back_and_decreasing_direction_is_deterministic() {
    let (mut linkage, ids) = slider_crank_displacement_driven_with_scale(1.0).unwrap();
    assert!(linkage.solve(SolverConfig::default()).unwrap().accepted());
    let forward = linkage
        .continue_driver(
            AdaptiveContinuationRequest {
                driver_id: ids.driver,
                mode: AdaptiveContinuationMode::PseudoArclength {
                    path_length: 0.2,
                    initial_direction: ContinuationDirection::IncreasingParameter,
                },
                step_policy: policy(),
            },
            SolverConfig::default(),
        )
        .unwrap();
    assert!(forward.completed(), "{forward:#?}");
    assert!(linkage.body(ids.crank).unwrap().pose().angle < 0.0);

    // On the negative-angle side, increasing displacement explicitly orients
    // the accepted endpoint tangent back toward the maximum.
    let reverse_direction = if linkage.body(ids.crank).unwrap().pose().angle < 0.0 {
        ContinuationDirection::IncreasingParameter
    } else {
        ContinuationDirection::DecreasingParameter
    };
    let reverse = linkage
        .continue_driver(
            AdaptiveContinuationRequest {
                driver_id: ids.driver,
                mode: AdaptiveContinuationMode::PseudoArclength {
                    path_length: 0.2,
                    initial_direction: reverse_direction,
                },
                step_policy: policy(),
            },
            SolverConfig::default(),
        )
        .unwrap();
    assert!(reverse.completed(), "{reverse:#?}");
    assert_physical_samples(&reverse);
    assert!(linkage.body(ids.crank).unwrap().pose().angle > 0.0);
    assert!(
        reverse
            .samples
            .iter()
            .any(|sample| sample.tangent_parameter_component < 0.0)
    );

    let (mut decreasing, decreasing_ids) =
        slider_crank_displacement_driven_with_scale(1.0).unwrap();
    assert!(
        decreasing
            .solve(SolverConfig::default())
            .unwrap()
            .accepted()
    );
    let initial_target = decreasing.driver(decreasing_ids.driver).unwrap().target();
    let away = decreasing
        .continue_driver(
            AdaptiveContinuationRequest {
                driver_id: decreasing_ids.driver,
                mode: AdaptiveContinuationMode::PseudoArclength {
                    path_length: 0.08,
                    initial_direction: ContinuationDirection::DecreasingParameter,
                },
                step_policy: policy(),
            },
            SolverConfig::default(),
        )
        .unwrap();
    assert!(away.completed(), "{away:#?}");
    assert!(away.accepted_target < initial_target);
    assert!(decreasing.body(decreasing_ids.crank).unwrap().pose().angle > 0.05);
    assert!(
        away.samples
            .iter()
            .all(|sample| sample.tangent_parameter_component < 0.0)
    );
}

fn run_pseudo(
    scale: f64,
    backend: LinearSolveBackendPolicy,
) -> (Linkage, geosolve_linkage::AdaptiveContinuationResult) {
    let (mut linkage, ids) = slider_crank_displacement_driven_with_scale(scale).unwrap();
    let config = SolverConfig {
        linear_solve_backend: backend,
        ..SolverConfig::default()
    };
    assert!(linkage.solve(config).unwrap().accepted());
    let result = linkage
        .continue_driver(
            AdaptiveContinuationRequest {
                driver_id: ids.driver,
                mode: AdaptiveContinuationMode::PseudoArclength {
                    path_length: 0.2,
                    initial_direction: ContinuationDirection::IncreasingParameter,
                },
                step_policy: policy(),
            },
            config,
        )
        .unwrap();
    assert!(result.completed(), "{result:#?}");
    assert_physical_samples(&result);
    (linkage, result)
}

#[test]
fn l3_pseudo_crossing_has_dense_sparse_physical_parity() {
    for scale in [1.0e-6, 1.0, 1.0e6] {
        let (dense_linkage, dense) = run_pseudo(scale, LinearSolveBackendPolicy::DenseOnly);
        let (sparse_linkage, sparse) = run_pseudo(scale, LinearSolveBackendPolicy::SparsePreferred);
        assert!(
            sparse
                .samples
                .iter()
                .any(|sample| { sample.corrector_backend == Some(LinearSolveBackend::SparseQr) }),
            "scale={scale:e}, sparse result={sparse:#?}"
        );
        assert!(
            sparse
                .samples
                .iter()
                .all(|sample| sample.corrector_sparse_fallback_reason.is_none())
        );
        assert_eq!(dense.status, sparse.status);
        assert!((dense.accepted_path_length - sparse.accepted_path_length).abs() <= 1.0e-14);
        let dense_endpoint = dense.samples.last().unwrap();
        let sparse_endpoint = sparse.samples.last().unwrap();
        assert_same_geometry(
            &dense_endpoint.solve.geometry,
            &sparse_endpoint.solve.geometry,
            scale,
        );
        assert_eq!(
            dense_endpoint.solve.rejection,
            sparse_endpoint.solve.rejection
        );
        assert_eq!(
            dense_endpoint.solve.source_mappings,
            sparse_endpoint.solve.source_mappings
        );
        assert_eq!(
            dense_endpoint.solve.display_audit,
            dense_endpoint.solve.core_report.audit
        );
        assert_eq!(
            sparse_endpoint.solve.display_audit,
            sparse_endpoint.solve.core_report.audit
        );
        assert_same_audit_structure(
            &dense_endpoint.solve.core_report.audit,
            &sparse_endpoint.solve.core_report.audit,
        );
        assert_eq!(
            dense_endpoint.solve.diagnostics.has_rank_warning,
            sparse_endpoint.solve.diagnostics.has_rank_warning
        );
        let dense_ratio = dense_endpoint
            .solve
            .diagnostics
            .singular_value_ratio
            .unwrap();
        let sparse_ratio = sparse_endpoint
            .solve
            .diagnostics
            .singular_value_ratio
            .unwrap();
        assert!((dense_ratio - sparse_ratio).abs() <= 1.0e-10);
        assert_eq!(
            dense_endpoint.solve.core_report.rank,
            sparse_endpoint.solve.core_report.rank
        );
        assert_eq!(
            dense_endpoint.solve.core_report.left_nullity,
            sparse_endpoint.solve.core_report.left_nullity
        );
        assert_eq!(
            dense_endpoint.solve.core_report.right_nullity,
            sparse_endpoint.solve.core_report.right_nullity
        );
        assert_eq!(
            dense_endpoint.solve.core_report.structural,
            sparse_endpoint.solve.core_report.structural
        );
        assert_eq!(
            dense_endpoint.solve.core_report.structural_nnz,
            sparse_endpoint.solve.core_report.structural_nnz
        );
        let dense_target = dense_linkage.drivers().next().unwrap().1.target();
        let sparse_target = sparse_linkage.drivers().next().unwrap().1.target();
        assert!((dense_target - sparse_target).abs() / scale <= 2.0e-9);
        let dense_monitor = dense_linkage.branch_monitors().next().unwrap().0;
        let sparse_monitor = sparse_linkage.branch_monitors().next().unwrap().0;
        let dense_branch = dense_linkage
            .evaluate_branch_monitor(dense_monitor, &dense_endpoint.solve.geometry)
            .unwrap();
        let sparse_branch = sparse_linkage
            .evaluate_branch_monitor(sparse_monitor, &sparse_endpoint.solve.geometry)
            .unwrap();
        assert_eq!(dense_branch.kind, sparse_branch.kind);
        assert_eq!(dense_branch.expected_sign, sparse_branch.expected_sign);
        assert_eq!(dense_branch.retained, sparse_branch.retained);
        assert!((dense_branch.signed_metric - sparse_branch.signed_metric).abs() / scale <= 2.0e-8);
    }
}

#[test]
fn pseudo_arclength_continuation_is_common_left_se2_equivariant() {
    let (mut original, _) = slider_crank_displacement_driven_with_scale(1.0).unwrap();
    let mut transformed = original.clone();
    let transform = Pose2::try_new(Vector2::new(4.5, -7.0), 0.83).unwrap();
    left_transform(&mut transformed, transform);
    let original_driver = original.drivers().next().unwrap().0;
    let transformed_driver = transformed.drivers().next().unwrap().0;
    let request = |driver_id| AdaptiveContinuationRequest {
        driver_id,
        mode: AdaptiveContinuationMode::PseudoArclength {
            path_length: 0.2,
            initial_direction: ContinuationDirection::IncreasingParameter,
        },
        step_policy: policy(),
    };

    let original_result = original
        .continue_driver(request(original_driver), SolverConfig::default())
        .unwrap();
    let transformed_result = transformed
        .continue_driver(request(transformed_driver), SolverConfig::default())
        .unwrap();
    assert!(original_result.completed(), "{original_result:#?}");
    assert!(transformed_result.completed(), "{transformed_result:#?}");
    assert_eq!(original_result.status, transformed_result.status);
    assert!((original_result.accepted_target - transformed_result.accepted_target).abs() <= 2.0e-9);
    let original_geometry = &original_result.samples.last().unwrap().solve.geometry;
    let transformed_geometry = &transformed_result.samples.last().unwrap().solve.geometry;
    for original_body in &original_geometry.bodies {
        let expected = transform.compose(&original_body.pose).unwrap();
        let actual = transformed_geometry
            .body_pose(original_body.body_id)
            .unwrap();
        let difference = expected.local_difference(&actual).unwrap();
        assert!(difference[0].hypot(difference[1]) <= 2.0e-8);
        assert!(difference[2].abs() <= 2.0e-8);
    }
}

#[test]
fn legacy_drive_to_beyond_displacement_fold_rejects_and_rolls_back() {
    for scale in [1.0e-6, 1.0, 1.0e6] {
        let (mut linkage, ids) = slider_crank_displacement_driven_with_scale(scale).unwrap();
        assert!(linkage.solve(SolverConfig::default()).unwrap().accepted());
        let retained_geometry = linkage.geometry().unwrap();
        let retained_target = linkage.driver(ids.driver).unwrap().target();
        let result = linkage
            .drive_to(ids.driver, 4.751 * scale, SolverConfig::default())
            .unwrap();

        assert!(!result.completed(), "scale={scale:e}, result={result:#?}");
        assert!(!result.samples.is_empty());
        assert!(!result.samples.last().unwrap().solve.accepted());
        assert_eq!(result.accepted_target.to_bits(), retained_target.to_bits());
        assert_eq!(
            linkage.driver(ids.driver).unwrap().target().to_bits(),
            retained_target.to_bits()
        );
        assert_eq!(linkage.geometry().unwrap(), retained_geometry);
    }
}

#[test]
fn continuation_request_validation_and_zero_natural_move_are_deterministic() {
    let (mut linkage, ids) = slider_crank_displacement_driven_with_scale(1.0).unwrap();
    let current = linkage.driver(ids.driver).unwrap().target();
    let no_move = linkage
        .continue_driver(
            AdaptiveContinuationRequest {
                driver_id: ids.driver,
                mode: AdaptiveContinuationMode::Natural { target: current },
                step_policy: policy(),
            },
            SolverConfig::default(),
        )
        .unwrap();
    assert!(no_move.completed());
    assert!(no_move.samples.is_empty());
    assert!(no_move.initial_solve.accepted());
    assert_eq!(
        no_move.initial_solve.core_report.hard_validity,
        HardValidity::Valid
    );
    assert_eq!(no_move.accepted_path_length.to_bits(), 0.0_f64.to_bits());
    let legacy = linkage
        .drive_to(ids.driver, current, SolverConfig::default())
        .unwrap();
    assert!(legacy.completed());
    assert_eq!(legacy.samples.len(), 1);
    assert!(legacy.samples[0].solve.accepted());

    for distance in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        assert!(
            linkage
                .continue_driver(
                    AdaptiveContinuationRequest {
                        driver_id: ids.driver,
                        mode: AdaptiveContinuationMode::PseudoArclength {
                            path_length: distance,
                            initial_direction: ContinuationDirection::IncreasingParameter,
                        },
                        step_policy: policy(),
                    },
                    SolverConfig::default(),
                )
                .is_err()
        );
    }
    assert!(
        linkage
            .continue_driver(
                AdaptiveContinuationRequest {
                    driver_id: ids.driver,
                    mode: AdaptiveContinuationMode::Natural { target: f64::NAN },
                    step_policy: policy(),
                },
                SolverConfig::default(),
            )
            .is_err()
    );
}

#[test]
fn tiny_pseudo_requests_never_complete_without_a_representable_sample() {
    let (mut representable, ids) = slider_crank_displacement_driven_with_scale(1.0).unwrap();
    let result = representable
        .continue_driver(
            AdaptiveContinuationRequest {
                driver_id: ids.driver,
                mode: AdaptiveContinuationMode::PseudoArclength {
                    path_length: 1.0e-15,
                    initial_direction: ContinuationDirection::IncreasingParameter,
                },
                step_policy: policy(),
            },
            SolverConfig::default(),
        )
        .unwrap();
    assert!(result.completed(), "{result:#?}");
    assert!(!result.samples.is_empty());
    assert!(result.accepted_path_length > 0.0);

    let (mut unrepresentable, ids) = slider_crank_displacement_driven_with_scale(1.0).unwrap();
    let result = unrepresentable
        .continue_driver(
            AdaptiveContinuationRequest {
                driver_id: ids.driver,
                mode: AdaptiveContinuationMode::PseudoArclength {
                    path_length: f64::MIN_POSITIVE,
                    initial_direction: ContinuationDirection::IncreasingParameter,
                },
                step_policy: policy(),
            },
            SolverConfig::default(),
        )
        .unwrap();
    assert!(result.completed(), "{result:#?}");
    assert!(!result.samples.is_empty());
    assert!(result.accepted_path_length > 0.0);
}

#[test]
fn zero_move_rejects_directly_injected_opposite_branch_without_false_completion() {
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
    let target = open.driver(open_ids.driver).unwrap().target();
    let result = open
        .continue_driver(
            AdaptiveContinuationRequest {
                driver_id: open_ids.driver,
                mode: AdaptiveContinuationMode::Natural { target },
                step_policy: policy(),
            },
            SolverConfig::default(),
        )
        .unwrap();

    assert_eq!(result.status, AdaptiveContinuationStatus::InitialRejected);
    assert!(!result.completed());
    assert!(result.samples.is_empty());
    assert!(matches!(
        result.initial_solve.rejection,
        Some(SolveRejection::BranchViolation(_))
    ));
    assert_eq!(open.geometry().unwrap(), retained);
}

#[test]
fn direct_stale_pose_is_ordinary_solved_before_entry_tangent_is_used() {
    let (mut linkage, ids) = slider_crank_displacement_driven_with_scale(1.0).unwrap();
    let stale = linkage.body(ids.rod).unwrap().pose();
    linkage
        .set_body_pose(
            ids.rod,
            Pose2::try_new(
                stale.translation + Vector2::new(0.02, -0.01),
                stale.angle + 0.01,
            )
            .unwrap(),
        )
        .unwrap();
    let target = linkage.driver(ids.driver).unwrap().target() - 0.01;
    let result = linkage
        .continue_driver(
            AdaptiveContinuationRequest {
                driver_id: ids.driver,
                mode: AdaptiveContinuationMode::Natural { target },
                step_policy: policy(),
            },
            SolverConfig::default(),
        )
        .unwrap();

    assert!(result.initial_solve.accepted(), "{result:#?}");
    assert!(result.initial_solve.core_report.iterations > 0);
    assert!(result.completed(), "{result:#?}");
    assert_physical_samples(&result);
    assert_eq!(
        linkage.driver(ids.driver).unwrap().target().to_bits(),
        target.to_bits()
    );
}

#[test]
fn nonlocal_corrector_and_extreme_entry_failure_leave_retained_state_unchanged() {
    let (mut linkage, ids) = slider_crank_displacement_driven_with_scale(1.0).unwrap();
    assert!(linkage.solve(SolverConfig::default()).unwrap().accepted());
    let retained = linkage.geometry().unwrap();
    let retained_target = linkage.driver(ids.driver).unwrap().target();
    let strict_policy = AdaptiveStepPolicy {
        initial_step: 0.04,
        minimum_step: 0.01,
        maximum_step: 0.04,
        maximum_correction: 1.0e-14,
        maximum_correction_step_ratio: 1.0e-12,
        max_retries: 8,
        ..policy()
    };
    let result = linkage
        .continue_driver(
            AdaptiveContinuationRequest {
                driver_id: ids.driver,
                mode: AdaptiveContinuationMode::Natural {
                    target: retained_target - 0.02,
                },
                step_policy: strict_policy,
            },
            SolverConfig::default(),
        )
        .unwrap();
    assert!(matches!(
        result.status,
        AdaptiveContinuationStatus::CorrectionNotLocal { .. }
    ));
    assert!(result.samples.is_empty());
    assert!(!result.rejected_attempts.is_empty());
    assert_eq!(linkage.geometry().unwrap(), retained);
    assert_eq!(
        linkage.driver(ids.driver).unwrap().target().to_bits(),
        retained_target.to_bits()
    );

    let extreme = Pose2::try_new(Vector2::new(f64::MAX, f64::MAX), f64::MAX).unwrap();
    linkage.set_body_pose(ids.rod, extreme).unwrap();
    let retained_extreme = linkage.body(ids.rod).unwrap().pose();
    let outcome = linkage.continue_driver(
        AdaptiveContinuationRequest {
            driver_id: ids.driver,
            mode: AdaptiveContinuationMode::Natural {
                target: retained_target,
            },
            step_policy: policy(),
        },
        SolverConfig::default(),
    );
    assert!(outcome.is_err() || outcome.is_ok_and(|result| !result.completed()));
    assert_eq!(linkage.body(ids.rod).unwrap().pose(), retained_extreme);
}

#[test]
fn natural_predictor_endpoint_rejects_aggressive_four_bar_toggle_step() {
    let (mut linkage, ids) = four_bar_with_scale(FourBarAssemblyMode::Open, 1.0).unwrap();
    let near_target = std::f64::consts::PI - 1.0e-3;
    assert!(
        linkage
            .drive_to(ids.driver, near_target, SolverConfig::default())
            .unwrap()
            .completed()
    );
    linkage.remove_driver(ids.driver).unwrap();
    let aggressive = linkage
        .add_angular_driver(
            "aggressive toggle skip",
            ids.ground,
            ids.input,
            near_target,
            std::f64::consts::TAU,
        )
        .unwrap();
    let aggressive_policy = AdaptiveStepPolicy {
        initial_step: 8.0,
        minimum_step: 1.0e-6,
        maximum_step: 8.0,
        maximum_correction: 8.0,
        maximum_correction_step_ratio: 8.0,
        max_retries: 32,
        ..policy()
    };
    let result = linkage
        .continue_driver(
            AdaptiveContinuationRequest {
                driver_id: aggressive,
                mode: AdaptiveContinuationMode::Natural {
                    target: std::f64::consts::TAU - 1.0e-3,
                },
                step_policy: aggressive_policy,
            },
            SolverConfig::default(),
        )
        .unwrap();

    assert!(!result.completed(), "{result:#?}");
    assert!(matches!(
        result.status,
        AdaptiveContinuationStatus::PredictorBranchEvent(_)
    ));
    let geometry = linkage.geometry().unwrap();
    assert!(
        linkage
            .evaluate_branch_monitor(ids.orientation_monitor, &geometry)
            .unwrap()
            .retained
    );
    assert!(linkage.driver(aggressive).unwrap().target() < std::f64::consts::PI);
}
