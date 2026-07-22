// SPDX-License-Identifier: GPL-3.0-or-later

use std::f64::consts::PI;

use geosolve_core::{
    AdaptiveStepPolicy, HardValidity, LinearSolveBackend, LinearSolveBackendPolicy,
    SolveTermination, SolverConfig,
};
use geosolve_geometry::{Frame3, Point3, Pose3, Vector3};
use geosolve_linkage::{
    AdaptiveContinuationMode, ContinuationDirection, EmbeddedSpatialSliderCrankIds,
    ShaftBearingExampleIds, SpatialAdaptiveContinuationRequest, SpatialAdaptiveContinuationResult,
    SpatialAdaptiveContinuationStatus, SpatialAssembly, SpatialAssemblyEdit, SpatialAssemblyError,
    SpatialAssemblyModeChange, SpatialAssemblySession, SpatialAxisParity,
    SpatialBoundaryHysteresisState, SpatialBoundaryObservation, SpatialBoundaryTransition,
    SpatialBranchBoundary, SpatialCoordinateId, SpatialCoordinateValueKind, SpatialExampleIds,
    SpatialExampleKind, SpatialGaugePolicy, SpatialHingeTarget, SpatialModeChangeTransaction,
    SpatialModeSign, SpatialPrincipalCutDirection, SpatialSourceId, embedded_spatial_slider_crank,
    spatial_example,
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

fn assert_physical_samples(result: &SpatialAdaptiveContinuationResult) {
    assert_eq!(
        result.initial_solve.core_report.termination,
        SolveTermination::Converged
    );
    assert_eq!(
        result.initial_solve.core_report.hard_validity,
        HardValidity::Valid
    );
    for sample in &result.samples {
        assert_eq!(
            sample.solve.core_report.termination,
            SolveTermination::Converged
        );
        assert_eq!(sample.solve.core_report.hard_validity, HardValidity::Valid);
        assert!(sample.solve.core_report.hard_residuals_validated);
        assert!(sample.solve.core_report.hard_residual_max <= TOLERANCE);
        assert!(sample.solve.acceptance_hard_residual_max <= TOLERANCE);
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
        assert!(
            sample
                .solve
                .mode_evaluations
                .iter()
                .all(|mode| mode.retained)
        );
    }
}

#[test]
fn shaft_bearing_natural_continuation_is_monotone_at_all_scales() {
    for scale in [1.0e-6, 1.0, 1.0e6] {
        let fixture = spatial_example(SpatialExampleKind::ShaftBearing, scale).unwrap();
        let SpatialExampleIds::ShaftBearing(ids) = fixture.ids else {
            unreachable!();
        };
        let mut session =
            SpatialAssemblySession::new(fixture.assembly, SolverConfig::default()).unwrap();
        let initial_revision = session.revision();
        let result = session
            .continue_driver(
                initial_revision,
                SpatialAdaptiveContinuationRequest {
                    driver_source: ids.drivers[1],
                    mode: AdaptiveContinuationMode::Natural {
                        target: 2.4 * scale,
                    },
                    step_policy: policy(),
                },
            )
            .unwrap();

        assert!(result.completed(), "scale={scale:e}, result={result:#?}");
        assert_eq!(result.status, SpatialAdaptiveContinuationStatus::Completed);
        assert_physical_samples(&result);
        assert!(!result.samples.is_empty());
        assert_eq!(result.accepted_target.to_bits(), (2.4 * scale).to_bits());
        assert_eq!(result.accepted_revision, session.revision());
        assert_eq!(
            result.accepted_revision,
            initial_revision + result.samples.len() as u64
        );
        assert!(
            result
                .samples
                .iter()
                .all(|sample| sample.tangent_parameter_component > 0.0)
        );
        assert_translation(&session, ids, 2.4 * scale, scale);

        let direct =
            SpatialAssemblySession::new(session.assembly().clone(), SolverConfig::default())
                .unwrap();
        assert_eq!(session.accepted_result(), direct.accepted_result());
    }
}

#[test]
fn shaft_bearing_hinge_continuation_retains_winding_and_translation() {
    for scale in [1.0e-6, 1.0, 1.0e6] {
        let fixture = spatial_example(SpatialExampleKind::ShaftBearing, scale).unwrap();
        let SpatialExampleIds::ShaftBearing(ids) = fixture.ids else {
            unreachable!();
        };
        let mut session =
            SpatialAssemblySession::new(fixture.assembly, SolverConfig::default()).unwrap();
        let result = session
            .continue_driver(
                session.revision(),
                SpatialAdaptiveContinuationRequest {
                    driver_source: ids.drivers[0],
                    mode: AdaptiveContinuationMode::Natural { target: 0.82 },
                    step_policy: policy(),
                },
            )
            .unwrap();

        assert!(result.completed(), "scale={scale:e}, result={result:#?}");
        assert_physical_samples(&result);
        let hinge = session.coordinate_value(ids.coordinates[0]).unwrap();
        let SpatialCoordinateValueKind::Hinge(hinge) = hinge.value else {
            panic!("shaft hinge coordinate changed kind");
        };
        assert_eq!(hinge.winding, 2);
        assert!((hinge.principal_phase - 0.82).abs() <= 2.0e-9);
        let translation = session.coordinate_value(ids.coordinates[1]).unwrap();
        let SpatialCoordinateValueKind::AxialTranslation(translation) = translation.value else {
            panic!("shaft translation coordinate changed kind");
        };
        assert!((translation - 1.9 * scale).abs() / scale <= 2.0e-9);
    }
}

#[test]
#[allow(clippy::too_many_lines)]
fn principal_cut_events_are_hysteretic_and_mode_changes_are_atomic() {
    let clear_positive_cut = PI - 1.0e-2;
    let (assembly, driver, hinge_coordinate) = principal_cut_assembly(clear_positive_cut, 2);
    let mut session = SpatialAssemblySession::new(assembly, SolverConfig::default()).unwrap();
    let near_positive_cut = PI - 1.5e-3;
    let result = session
        .continue_driver(
            session.revision(),
            SpatialAdaptiveContinuationRequest {
                driver_source: driver,
                mode: AdaptiveContinuationMode::Natural {
                    target: near_positive_cut,
                },
                step_policy: policy(),
            },
        )
        .unwrap();

    let SpatialAdaptiveContinuationStatus::BranchBoundary(events) = &result.status else {
        panic!("expected a principal-cut event: {result:#?}");
    };
    assert_eq!(events.len(), 1);
    let event = &events[0];
    assert_eq!(
        event.boundary,
        SpatialBranchBoundary::HingePrincipalCut {
            coordinate: hinge_coordinate,
            winding: 2,
        }
    );
    assert_eq!(event.transition, SpatialBoundaryTransition::Entered);
    assert_eq!(
        event.observation,
        SpatialBoundaryObservation::CorrectedPhysicalEndpoint
    );
    assert!(event.clearance > 1.0e-3 && event.clearance <= 2.0e-3);
    assert_eq!(result.samples.last().unwrap().boundary_events, *events);
    let near_evaluation = session
        .branch_boundary_evaluations()
        .iter()
        .find(|evaluation| evaluation.boundary == event.boundary)
        .unwrap();
    assert_eq!(
        near_evaluation.hysteresis_state,
        SpatialBoundaryHysteresisState::Near
    );
    let principal_cut_boundary = near_evaluation.boundary;

    let deadband = session
        .continue_driver(
            session.revision(),
            SpatialAdaptiveContinuationRequest {
                driver_source: driver,
                mode: AdaptiveContinuationMode::Natural {
                    target: PI - 3.0e-3,
                },
                step_policy: policy(),
            },
        )
        .unwrap();
    assert!(deadband.completed(), "{deadband:#?}");
    assert!(
        deadband
            .samples
            .iter()
            .all(|sample| sample.boundary_events.is_empty())
    );
    assert_eq!(
        session
            .branch_boundary_evaluations()
            .iter()
            .find(|evaluation| evaluation.boundary == principal_cut_boundary)
            .unwrap()
            .hysteresis_state,
        SpatialBoundaryHysteresisState::Near
    );

    let away = session
        .continue_driver(
            session.revision(),
            SpatialAdaptiveContinuationRequest {
                driver_source: driver,
                mode: AdaptiveContinuationMode::Natural {
                    target: clear_positive_cut,
                },
                step_policy: policy(),
            },
        )
        .unwrap();
    assert!(away.completed(), "{away:#?}");
    let left = away
        .samples
        .iter()
        .flat_map(|sample| &sample.boundary_events)
        .filter(|event| event.boundary == principal_cut_boundary)
        .collect::<Vec<_>>();
    assert_eq!(left.len(), 1);
    assert_eq!(left[0].transition, SpatialBoundaryTransition::Left);
    assert!(left[0].clearance >= 4.0e-3);

    let entered_again = session
        .continue_driver(
            session.revision(),
            SpatialAdaptiveContinuationRequest {
                driver_source: driver,
                mode: AdaptiveContinuationMode::Natural {
                    target: near_positive_cut,
                },
                step_policy: policy(),
            },
        )
        .unwrap();
    assert!(matches!(
        entered_again.status,
        SpatialAdaptiveContinuationStatus::BranchBoundary(_)
    ));

    let retained_revision = session.revision();
    let retained = session.accepted_result().clone();
    assert!(
        session
            .change_modes(SpatialModeChangeTransaction {
                expected_revision: retained_revision,
                changes: vec![SpatialAssemblyModeChange::HingePrincipalCut {
                    coordinate: hinge_coordinate,
                    direction: SpatialPrincipalCutDirection::NegativeToPositive,
                    new_principal_phase: -PI + 1.0e-2,
                }],
                companion_edits: Vec::new(),
            })
            .is_err()
    );
    assert_eq!(session.revision(), retained_revision);
    assert_eq!(session.accepted_result(), &retained);

    session
        .change_modes(SpatialModeChangeTransaction {
            expected_revision: retained_revision,
            changes: vec![SpatialAssemblyModeChange::HingePrincipalCut {
                coordinate: hinge_coordinate,
                direction: SpatialPrincipalCutDirection::PositiveToNegative,
                new_principal_phase: -PI + 1.0e-2,
            }],
            companion_edits: Vec::new(),
        })
        .unwrap();
    assert_eq!(session.revision(), retained_revision + 1);
    let hinge = session.coordinate_value(hinge_coordinate).unwrap();
    let SpatialCoordinateValueKind::Hinge(hinge) = hinge.value else {
        panic!("shaft hinge coordinate changed kind");
    };
    assert_eq!(hinge.winding, 3);
    assert!((hinge.principal_phase - (-PI + 1.0e-2)).abs() <= 2.0e-9);
    let changed_boundary = session
        .branch_boundary_evaluations()
        .iter()
        .find(|evaluation| {
            evaluation.boundary
                == SpatialBranchBoundary::HingePrincipalCut {
                    coordinate: hinge_coordinate,
                    winding: 3,
                }
        })
        .unwrap();
    assert!(changed_boundary.clearance >= 4.0e-3);
    assert_eq!(
        changed_boundary.hysteresis_state,
        SpatialBoundaryHysteresisState::Clear
    );
}

#[test]
fn coarse_predictor_cut_crossing_reports_a_typed_event_without_publication() {
    let (assembly, driver, coordinate) = principal_cut_assembly(PI - 1.0e-2, 0);
    let mut session = SpatialAssemblySession::new(assembly, SolverConfig::default()).unwrap();
    let initial_revision = session.revision();
    let mut coarse_policy = policy();
    coarse_policy.initial_step = 2.0e-2;
    coarse_policy.minimum_step = 5.0e-3;
    coarse_policy.maximum_step = 2.0e-2;
    let result = session
        .continue_driver(
            initial_revision,
            SpatialAdaptiveContinuationRequest {
                driver_source: driver,
                mode: AdaptiveContinuationMode::Natural {
                    target: PI - 5.0e-4,
                },
                step_policy: coarse_policy,
            },
        )
        .unwrap();

    let SpatialAdaptiveContinuationStatus::BranchBoundary(events) = &result.status else {
        panic!("expected a predictor boundary event: {result:#?}");
    };
    assert_eq!(events.len(), 1);
    assert_eq!(
        events[0].boundary,
        SpatialBranchBoundary::HingePrincipalCut {
            coordinate,
            winding: 0,
        }
    );
    assert_eq!(events[0].transition, SpatialBoundaryTransition::Entered);
    assert_eq!(
        events[0].observation,
        SpatialBoundaryObservation::PredictorEndpoint
    );
    assert!(events[0].clearance <= 1.0e-3);
    assert!(result.samples.iter().all(|sample| {
        sample.boundary_events.is_empty()
            && sample
                .solve
                .branch_boundary_evaluations
                .iter()
                .find(|evaluation| evaluation.boundary == events[0].boundary)
                .is_some_and(|evaluation| evaluation.clearance > 1.0e-3)
    }));
    assert_eq!(session.revision(), result.accepted_revision);
    assert_eq!(
        session
            .branch_boundary_evaluations()
            .iter()
            .find(|evaluation| evaluation.boundary == events[0].boundary)
            .unwrap()
            .hysteresis_state,
        SpatialBoundaryHysteresisState::Clear
    );
}

#[test]
fn pseudo_arclength_cut_crossing_requires_an_explicit_mode_change() {
    let (assembly, driver, coordinate) = principal_cut_assembly(PI - 1.0e-2, 0);
    let mut session = SpatialAssemblySession::new(assembly, SolverConfig::default()).unwrap();
    let initial_revision = session.revision();
    let mut cut_policy = policy();
    cut_policy.initial_step = 2.0e-2;
    cut_policy.minimum_step = 2.0e-2;
    cut_policy.maximum_step = 2.0e-2;
    let result = session
        .continue_driver(
            initial_revision,
            SpatialAdaptiveContinuationRequest {
                driver_source: driver,
                mode: AdaptiveContinuationMode::PseudoArclength {
                    path_length: 2.0e-2,
                    initial_direction: ContinuationDirection::IncreasingParameter,
                },
                step_policy: cut_policy,
            },
        )
        .unwrap();

    let SpatialAdaptiveContinuationStatus::BranchBoundary(events) = &result.status else {
        panic!("expected a principal-cut crossing event: {result:#?}");
    };
    assert_eq!(events.len(), 1);
    assert_eq!(
        events[0].boundary,
        SpatialBranchBoundary::HingePrincipalCut {
            coordinate,
            winding: 0,
        }
    );
    assert_eq!(
        events[0].transition,
        SpatialBoundaryTransition::CrossingAttempted
    );
    assert_eq!(
        events[0].observation,
        SpatialBoundaryObservation::PredictorEndpoint
    );
    assert_eq!(events[0].clearance.to_bits(), 0.0_f64.to_bits());
    assert!(result.samples.is_empty());
    assert_eq!(result.accepted_revision, initial_revision);
    assert_eq!(session.revision(), initial_revision);
}

fn principal_cut_assembly(
    principal_phase: f64,
    winding: i64,
) -> (SpatialAssembly, SpatialSourceId, SpatialCoordinateId) {
    let mut assembly = SpatialAssembly::new(1.0).unwrap();
    let ground = assembly.add_body("ground", Pose3::identity()).unwrap();
    let moving = assembly
        .add_body("moving", planar_pose(Vector3::zeros(), principal_phase))
        .unwrap();
    let ground_frame = assembly
        .add_frame_feature("ground hinge", ground, identity_frame())
        .unwrap();
    let moving_frame = assembly
        .add_frame_feature("moving hinge", moving, identity_frame())
        .unwrap();
    assembly
        .add_physical_ground("ground fixed", ground)
        .unwrap();
    let joint = assembly
        .add_revolute_joint(
            "revolute",
            ground_frame,
            moving_frame,
            SpatialAxisParity::Aligned,
        )
        .unwrap();
    let coordinate = assembly
        .add_hinge_coordinate("hinge phase", joint, winding)
        .unwrap();
    let driver = assembly
        .add_hinge_position_driver(
            "hinge driver",
            coordinate,
            SpatialHingeTarget {
                principal_phase,
                winding,
            },
        )
        .unwrap();
    assembly
        .add_hinge_winding_monitor("hinge winding", coordinate, winding)
        .unwrap();
    (assembly, driver, coordinate)
}

#[test]
fn explicit_side_mode_change_commits_once_and_invalid_parity_rolls_back() {
    let fixture = spatial_example(SpatialExampleKind::ShaftBearing, 1.0).unwrap();
    let SpatialExampleIds::ShaftBearing(ids) = fixture.ids else {
        unreachable!();
    };
    let mut session =
        SpatialAssemblySession::new(fixture.assembly, SolverConfig::default()).unwrap();
    let revision = session.revision();
    session
        .change_modes(SpatialModeChangeTransaction {
            expected_revision: revision,
            changes: vec![SpatialAssemblyModeChange::MonitorPlaneSide {
                monitor: ids.monitors[2],
                side: SpatialModeSign::Negative,
            }],
            companion_edits: vec![SpatialAssemblyEdit::TranslationDriverTarget {
                source: ids.drivers[1],
                target: -1.9,
            }],
        })
        .unwrap();
    assert_eq!(session.revision(), revision + 1);
    let side = session.mode_evaluation(ids.monitors[2]).unwrap();
    assert!(matches!(
        side.kind,
        geosolve_linkage::SpatialModeMonitorKind::PlaneSide {
            side: SpatialModeSign::Negative,
            ..
        }
    ));
    assert!(side.retained_normalized_metric > 1.0);

    let retained_revision = session.revision();
    let retained = session.accepted_result().clone();
    assert!(
        session
            .change_modes(SpatialModeChangeTransaction {
                expected_revision: retained_revision,
                changes: vec![SpatialAssemblyModeChange::MonitorAxisParity {
                    monitor: ids.monitors[0],
                    parity: SpatialAxisParity::Opposed,
                }],
                companion_edits: Vec::new(),
            })
            .is_err()
    );
    assert_eq!(session.revision(), retained_revision);
    assert_eq!(session.accepted_result(), &retained);
}

#[test]
fn zero_move_revalidates_without_consuming_a_spatial_revision() {
    let fixture = spatial_example(SpatialExampleKind::ShaftBearing, 1.0).unwrap();
    let SpatialExampleIds::ShaftBearing(ids) = fixture.ids else {
        unreachable!();
    };
    let mut session =
        SpatialAssemblySession::new(fixture.assembly, SolverConfig::default()).unwrap();
    let revision = session.revision();
    let retained = session.accepted_result().clone();
    let result = session
        .continue_driver(
            revision,
            SpatialAdaptiveContinuationRequest {
                driver_source: ids.drivers[1],
                mode: AdaptiveContinuationMode::Natural { target: 1.9 },
                step_policy: policy(),
            },
        )
        .unwrap();

    assert!(result.completed());
    assert!(result.samples.is_empty());
    assert_eq!(result.accepted_path_length.to_bits(), 0.0_f64.to_bits());
    assert_eq!(session.revision(), revision);
    assert_eq!(session.accepted_result(), &retained);
}

#[test]
fn floating_spatial_continuation_uses_private_gauge_without_publishing_it() {
    for scale in [1.0e-6, 1.0, 1.0e6] {
        let mut assembly = SpatialAssembly::new(scale).unwrap();
        let reference = assembly.add_body("reference", Pose3::identity()).unwrap();
        let moving = assembly
            .add_body(
                "moving",
                planar_pose(Vector3::new(0.0, 0.0, 1.2 * scale), 0.3),
            )
            .unwrap();
        let reference_axis = assembly
            .add_axis_feature("reference axis", reference, identity_frame())
            .unwrap();
        let moving_axis = assembly
            .add_axis_feature("moving axis", moving, identity_frame())
            .unwrap();
        let joint = assembly
            .add_cylindrical_joint(
                "floating cylindrical",
                reference_axis,
                moving_axis,
                SpatialAxisParity::Aligned,
            )
            .unwrap();
        let hinge = assembly
            .add_hinge_coordinate("floating hinge", joint, 4)
            .unwrap();
        let translation = assembly
            .add_axial_translation_coordinate("floating translation", joint)
            .unwrap();
        assembly
            .add_hinge_position_driver(
                "floating angle driver",
                hinge,
                geosolve_linkage::SpatialHingeTarget {
                    principal_phase: 0.3,
                    winding: 4,
                },
            )
            .unwrap();
        let driver = assembly
            .add_translation_position_driver(
                "floating translation driver",
                translation,
                1.2 * scale,
            )
            .unwrap();
        assembly
            .add_axis_parity_monitor(
                "floating aligned mode",
                reference_axis,
                moving_axis,
                SpatialAxisParity::Aligned,
            )
            .unwrap();
        assembly
            .add_hinge_winding_monitor("floating winding", hinge, 4)
            .unwrap();

        let mut session = SpatialAssemblySession::new(assembly, SolverConfig::default()).unwrap();
        assert_eq!(session.gauge_report().gauge_dof, 6);
        assert_eq!(session.gauge_report().internal_mobility, 0);
        let result = session
            .continue_driver(
                session.revision(),
                SpatialAdaptiveContinuationRequest {
                    driver_source: driver,
                    mode: AdaptiveContinuationMode::Natural {
                        target: 1.7 * scale,
                    },
                    step_policy: policy(),
                },
            )
            .unwrap();

        assert!(result.completed(), "scale={scale:e}, result={result:#?}");
        assert_physical_samples(&result);
        assert_eq!(session.gauge_report().gauge_dof, 6);
        assert_eq!(session.gauge_report().internal_mobility, 0);
        assert!(
            session
                .accepted_result()
                .display_audit
                .sources
                .iter()
                .all(|source| !source
                    .source_label
                    .contains("private spatial numerical gauge"))
        );
    }
}

#[test]
fn spatial_continuation_request_errors_are_transactional() {
    let fixture = spatial_example(SpatialExampleKind::ShaftBearing, 1.0).unwrap();
    let SpatialExampleIds::ShaftBearing(ids) = fixture.ids else {
        unreachable!();
    };
    let mut session =
        SpatialAssemblySession::new(fixture.assembly, SolverConfig::default()).unwrap();
    let revision = session.revision();
    let retained = session.accepted_result().clone();
    let request = |driver_source, mode| SpatialAdaptiveContinuationRequest {
        driver_source,
        mode,
        step_policy: policy(),
    };

    assert!(matches!(
        session.continue_driver(
            revision + 1,
            request(
                ids.drivers[1],
                AdaptiveContinuationMode::Natural { target: 2.0 }
            )
        ),
        Err(SpatialAssemblyError::StaleRevision { .. })
    ));
    assert!(matches!(
        session.continue_driver(
            revision,
            request(ids.joint, AdaptiveContinuationMode::Natural { target: 2.0 })
        ),
        Err(SpatialAssemblyError::WrongSourceKind { .. })
    ));
    assert!(
        session
            .continue_driver(
                revision,
                request(
                    ids.drivers[1],
                    AdaptiveContinuationMode::Natural { target: f64::NAN }
                )
            )
            .is_err()
    );
    assert!(
        session
            .continue_driver(
                revision,
                request(
                    ids.drivers[1],
                    AdaptiveContinuationMode::PseudoArclength {
                        path_length: 0.0,
                        initial_direction: ContinuationDirection::IncreasingParameter,
                    }
                )
            )
            .is_err()
    );
    assert_eq!(session.revision(), revision);
    assert_eq!(session.accepted_result(), &retained);
}

#[test]
fn tiny_spatial_pseudo_requests_never_complete_without_a_sample() {
    let fixture = spatial_example(SpatialExampleKind::ShaftBearing, 1.0).unwrap();
    let SpatialExampleIds::ShaftBearing(ids) = fixture.ids else {
        unreachable!();
    };
    let mut representable =
        SpatialAssemblySession::new(fixture.assembly.clone(), SolverConfig::default()).unwrap();
    let result = representable
        .continue_driver(
            representable.revision(),
            SpatialAdaptiveContinuationRequest {
                driver_source: ids.drivers[1],
                mode: AdaptiveContinuationMode::PseudoArclength {
                    path_length: 1.0e-15,
                    initial_direction: ContinuationDirection::IncreasingParameter,
                },
                step_policy: policy(),
            },
        )
        .unwrap();
    assert!(result.completed(), "{result:#?}");
    assert!(!result.samples.is_empty());
    assert!(result.accepted_path_length > 0.0);

    let mut unrepresentable =
        SpatialAssemblySession::new(fixture.assembly, SolverConfig::default()).unwrap();
    let revision = unrepresentable.revision();
    let result = unrepresentable
        .continue_driver(
            revision,
            SpatialAdaptiveContinuationRequest {
                driver_source: ids.drivers[1],
                mode: AdaptiveContinuationMode::PseudoArclength {
                    path_length: f64::MIN_POSITIVE,
                    initial_direction: ContinuationDirection::IncreasingParameter,
                },
                step_policy: policy(),
            },
        )
        .unwrap();
    assert!(!result.completed());
    assert!(matches!(
        result.status,
        SpatialAdaptiveContinuationStatus::MinimumStep
            | SpatialAdaptiveContinuationStatus::CorrectionNotLocal { .. }
    ));
    assert!(result.samples.is_empty());
    assert_eq!(unrepresentable.revision(), revision);
}

#[test]
fn signed_zero_never_completes_without_a_physical_sample() {
    let mut assembly = SpatialAssembly::new(1.0).unwrap();
    let ground = assembly.add_body("ground", Pose3::identity()).unwrap();
    let slider = assembly.add_body("slider", Pose3::identity()).unwrap();
    let ground_axis = assembly
        .add_axis_feature("ground guide", ground, x_axis_frame())
        .unwrap();
    let slider_axis = assembly
        .add_axis_feature("slider guide", slider, x_axis_frame())
        .unwrap();
    assembly
        .add_physical_ground("ground fixed", ground)
        .unwrap();
    let joint = assembly
        .add_prismatic_joint(
            "slider prismatic",
            ground_axis,
            slider_axis,
            SpatialAxisParity::Aligned,
        )
        .unwrap();
    let coordinate = assembly
        .add_axial_translation_coordinate("signed-zero translation", joint)
        .unwrap();
    let driver = assembly
        .add_translation_position_driver("signed-zero driver", coordinate, -0.0)
        .unwrap();
    let mut session = SpatialAssemblySession::new(assembly, SolverConfig::default()).unwrap();
    let revision = session.revision();
    let retained_pose = session
        .accepted_result()
        .geometry
        .body_pose(slider)
        .unwrap();
    let result = session
        .continue_driver(
            revision,
            SpatialAdaptiveContinuationRequest {
                driver_source: driver,
                mode: AdaptiveContinuationMode::PseudoArclength {
                    path_length: f64::from_bits(1),
                    initial_direction: ContinuationDirection::IncreasingParameter,
                },
                step_policy: policy(),
            },
        )
        .unwrap();

    assert!(result.completed(), "{result:#?}");
    assert!(!result.samples.is_empty());
    assert!(session.revision() > revision);
    let accepted_pose = session
        .accepted_result()
        .geometry
        .body_pose(slider)
        .unwrap();
    let difference = retained_pose.local_difference(&accepted_pose).unwrap();
    assert!(
        difference
            .iter()
            .any(|value| value.classify() != std::num::FpCategory::Zero)
    );
}

#[test]
fn aggressive_hinge_predictor_is_checked_against_its_predicted_target() {
    let fixture = spatial_example(SpatialExampleKind::ShaftBearing, 1.0).unwrap();
    let SpatialExampleIds::ShaftBearing(ids) = fixture.ids else {
        unreachable!();
    };
    let mut session =
        SpatialAssemblySession::new(fixture.assembly, SolverConfig::default()).unwrap();
    let aggressive = AdaptiveStepPolicy {
        initial_step: 8.0,
        minimum_step: 1.0e-6,
        maximum_step: 8.0,
        maximum_correction: 8.0,
        maximum_correction_step_ratio: 8.0,
        max_retries: 32,
        ..policy()
    };
    let result = session
        .continue_driver(
            session.revision(),
            SpatialAdaptiveContinuationRequest {
                driver_source: ids.drivers[0],
                mode: AdaptiveContinuationMode::Natural { target: 2.2 },
                step_policy: aggressive,
            },
        )
        .unwrap();
    assert!(result.completed(), "{result:#?}");
    assert_eq!(result.samples.len(), 1);
    let hinge = session.coordinate_value(ids.coordinates[0]).unwrap();
    let SpatialCoordinateValueKind::Hinge(hinge) = hinge.value else {
        panic!("shaft hinge coordinate changed kind");
    };
    assert_eq!(hinge.winding, 2);
    assert!((hinge.principal_phase - 2.2).abs() <= 2.0e-9);
}

#[test]
fn monitor_split_components_reject_nonunique_paths_for_every_gauge_reference() {
    let mut assembly = SpatialAssembly::new(1.0).unwrap();
    let reference = assembly.add_body("reference", Pose3::identity()).unwrap();
    let moving = assembly
        .add_body("moving", planar_pose(Vector3::new(0.0, 0.0, 1.2), 0.3))
        .unwrap();
    let isolated = assembly.add_body("isolated", Pose3::identity()).unwrap();
    let reference_axis = assembly
        .add_axis_feature("reference axis", reference, identity_frame())
        .unwrap();
    let moving_axis = assembly
        .add_axis_feature("moving axis", moving, identity_frame())
        .unwrap();
    let isolated_axis = assembly
        .add_axis_feature("isolated axis", isolated, identity_frame())
        .unwrap();
    let joint = assembly
        .add_cylindrical_joint(
            "floating cylindrical",
            reference_axis,
            moving_axis,
            SpatialAxisParity::Aligned,
        )
        .unwrap();
    let hinge = assembly
        .add_hinge_coordinate("floating hinge", joint, 0)
        .unwrap();
    let translation = assembly
        .add_axial_translation_coordinate("floating translation", joint)
        .unwrap();
    assembly
        .add_hinge_position_driver(
            "floating angle driver",
            hinge,
            geosolve_linkage::SpatialHingeTarget {
                principal_phase: 0.3,
                winding: 0,
            },
        )
        .unwrap();
    let driver = assembly
        .add_translation_position_driver("floating translation driver", translation, 1.2)
        .unwrap();
    assembly
        .add_axis_parity_monitor(
            "monitor-only component bridge",
            reference_axis,
            isolated_axis,
            SpatialAxisParity::Aligned,
        )
        .unwrap();

    let mut default = SpatialAssemblySession::new(assembly, SolverConfig::default()).unwrap();
    assert_eq!(default.gauge_report().internal_mobility, 6);
    let mut alternate = default.clone();
    alternate
        .set_gauge_policy(
            alternate.revision(),
            SpatialGaugePolicy::ExplicitReferences {
                bodies: vec![isolated],
            },
        )
        .unwrap();
    for session in [&mut default, &mut alternate] {
        let revision = session.revision();
        let retained = session.accepted_result().clone();
        let zero = session
            .continue_driver(
                revision,
                SpatialAdaptiveContinuationRequest {
                    driver_source: driver,
                    mode: AdaptiveContinuationMode::Natural { target: 1.2 },
                    step_policy: policy(),
                },
            )
            .unwrap();
        assert!(zero.completed());
        assert!(zero.samples.is_empty());
        assert_eq!(session.revision(), revision);
        assert!(matches!(
            session.continue_driver(
                revision,
                SpatialAdaptiveContinuationRequest {
                    driver_source: driver,
                    mode: AdaptiveContinuationMode::Natural { target: 1.4 },
                    step_policy: policy(),
                }
            ),
            Err(SpatialAssemblyError::InvalidField {
                field: "spatial_continuation.driver_source",
                ..
            })
        ));
        assert_eq!(session.revision(), revision);
        assert_eq!(session.accepted_result(), &retained);
    }
}

fn assert_translation(
    session: &SpatialAssemblySession,
    ids: ShaftBearingExampleIds,
    expected: f64,
    scale: f64,
) {
    let value = session.coordinate_value(ids.coordinates[1]).unwrap();
    let SpatialCoordinateValueKind::AxialTranslation(value) = value.value else {
        panic!("shaft translation coordinate changed kind");
    };
    assert!((value - expected).abs() / scale <= 2.0e-9);
    let hinge = session.coordinate_value(ids.coordinates[0]).unwrap();
    let SpatialCoordinateValueKind::Hinge(hinge) = hinge.value else {
        panic!("shaft hinge coordinate changed kind");
    };
    assert_eq!(hinge.winding, 2);
    assert!((hinge.principal_phase - 0.48).abs() <= 2.0e-9);
}

type SpatialSliderCrankIds = EmbeddedSpatialSliderCrankIds;

fn spatial_slider_crank(scale: f64) -> (SpatialAssembly, SpatialSliderCrankIds) {
    spatial_slider_crank_with_transform(scale, None)
}

fn spatial_slider_crank_with_transform(
    scale: f64,
    common_left: Option<Pose3>,
) -> (SpatialAssembly, SpatialSliderCrankIds) {
    spatial_slider_crank_at(scale, common_left, 0.05)
}

#[allow(clippy::too_many_lines)]
fn spatial_slider_crank_at(
    scale: f64,
    common_left: Option<Pose3>,
    crank_angle: f64,
) -> (SpatialAssembly, SpatialSliderCrankIds) {
    let fixture = embedded_spatial_slider_crank(
        scale,
        common_left.unwrap_or_else(Pose3::identity),
        crank_angle,
    )
    .unwrap();
    (fixture.assembly, fixture.ids)
}

#[test]
fn transformed_slider_crank_zero_distance_retains_the_bitwise_snapshot() {
    let scale = 1.0;
    let phase = 1.159;
    let embedding = Pose3::exp([0.0, 0.0, 0.0, -0.007, 0.448, 0.268]).unwrap();
    let fixture = embedded_spatial_slider_crank(scale, embedding, phase).unwrap();
    let crank_y = 1.25 * phase.sin();
    let target = 1.25 * phase.cos() + (3.5 * 3.5 - crank_y * crank_y).sqrt();
    let mut session =
        SpatialAssemblySession::new(fixture.assembly, SolverConfig::default()).unwrap();
    let revision = session.revision();
    let accepted = session.accepted_result().clone();

    let result = session
        .continue_driver(
            revision,
            SpatialAdaptiveContinuationRequest {
                driver_source: fixture.ids.driver,
                mode: AdaptiveContinuationMode::Natural { target },
                step_policy: policy(),
            },
        )
        .unwrap();

    assert_eq!(result.status, SpatialAdaptiveContinuationStatus::Completed);
    assert!(result.samples.is_empty());
    assert_eq!(session.revision(), revision);
    assert_eq!(session.accepted_result(), &accepted);
}

fn planar_pose(translation: Vector3<f64>, angle: f64) -> Pose3 {
    let half = 0.5 * angle;
    Pose3::try_new(translation, [half.cos(), 0.0, 0.0, half.sin()]).unwrap()
}

fn identity_frame() -> Frame3 {
    Frame3::try_new(Point3::origin(), Vector3::x(), Vector3::y(), Vector3::z()).unwrap()
}

fn x_axis_frame() -> Frame3 {
    Frame3::try_new(Point3::origin(), Vector3::y(), Vector3::z(), Vector3::x()).unwrap()
}

fn crank_phase(session: &SpatialAssemblySession, coordinate: SpatialCoordinateId) -> f64 {
    let value = session.coordinate_value(coordinate).unwrap();
    let SpatialCoordinateValueKind::Hinge(value) = value.value else {
        panic!("crank phase coordinate changed kind");
    };
    assert_eq!(value.winding, 0);
    value.principal_phase
}

#[test]
fn spatial_natural_continuation_stops_before_the_slider_crank_fold() {
    for scale in [1.0e-6, 1.0, 1.0e6] {
        let (assembly, ids) = spatial_slider_crank(scale);
        let mut session = SpatialAssemblySession::new(assembly, SolverConfig::default()).unwrap();
        assert_eq!(session.accepted_result().core_report.rank, 18);
        let result = session
            .continue_driver(
                session.revision(),
                SpatialAdaptiveContinuationRequest {
                    driver_source: ids.driver,
                    mode: AdaptiveContinuationMode::Natural {
                        target: 4.751 * scale,
                    },
                    step_policy: policy(),
                },
            )
            .unwrap();

        assert!(!result.completed(), "scale={scale:e}, result={result:#?}");
        assert_eq!(
            result.status,
            SpatialAdaptiveContinuationStatus::PseudoArclengthRequired
        );
        assert_physical_samples(&result);
        assert!(!result.samples.is_empty());
        assert!(result.accepted_target > result.initial_target);
        assert!(result.accepted_target < (4.75 + 1.0e-8) * scale);
        assert!(crank_phase(&session, ids.crank_hinge) >= 0.0);
        assert!(session.mode_evaluations().iter().all(|mode| mode.retained));
    }
}

#[test]
fn exact_spatial_fold_reaches_the_augmented_tangent_test() {
    let (assembly, ids) = spatial_slider_crank_at(1.0, None, 0.0);
    let mut session = SpatialAssemblySession::new(assembly, SolverConfig::default()).unwrap();
    assert_eq!(session.accepted_result().core_report.right_nullity, 1);
    assert_eq!(session.gauge_report().internal_mobility, 1);
    let outcome = session
        .continue_driver(
            session.revision(),
            SpatialAdaptiveContinuationRequest {
                driver_source: ids.driver,
                mode: AdaptiveContinuationMode::PseudoArclength {
                    path_length: 0.02,
                    initial_direction: ContinuationDirection::IncreasingParameter,
                },
                step_policy: policy(),
            },
        )
        .unwrap();
    assert!(matches!(
        outcome.status,
        SpatialAdaptiveContinuationStatus::TangentFailure(_)
    ));
    assert!(outcome.samples.is_empty());
}

#[test]
fn spatial_pseudo_arclength_crosses_the_slider_crank_fold() {
    for scale in [1.0e-6, 1.0, 1.0e6] {
        let (assembly, ids) = spatial_slider_crank(scale);
        let mut session = SpatialAssemblySession::new(assembly, SolverConfig::default()).unwrap();
        let initial_target = 4.747_880_210_234_948 * scale;
        let result = session
            .continue_driver(
                session.revision(),
                SpatialAdaptiveContinuationRequest {
                    driver_source: ids.driver,
                    mode: AdaptiveContinuationMode::PseudoArclength {
                        path_length: 0.2,
                        initial_direction: ContinuationDirection::IncreasingParameter,
                    },
                    step_policy: policy(),
                },
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
                .any(|sample| sample.tangent_parameter_component < 0.0)
        );
        assert!(crank_phase(&session, ids.crank_hinge) < 0.0);
        assert!(result.accepted_target < 4.75 * scale);
        assert!(session.mode_evaluations().iter().all(|mode| mode.retained));

        let direct =
            SpatialAssemblySession::new(session.assembly().clone(), SolverConfig::default())
                .unwrap();
        assert_eq!(session.accepted_result(), direct.accepted_result());
    }
}

#[test]
fn spatial_pseudo_orientation_crosses_back_and_moves_away_deterministically() {
    let (assembly, ids) = spatial_slider_crank(1.0);
    let mut session = SpatialAssemblySession::new(assembly, SolverConfig::default()).unwrap();
    let forward = session
        .continue_driver(
            session.revision(),
            SpatialAdaptiveContinuationRequest {
                driver_source: ids.driver,
                mode: AdaptiveContinuationMode::PseudoArclength {
                    path_length: 0.2,
                    initial_direction: ContinuationDirection::IncreasingParameter,
                },
                step_policy: policy(),
            },
        )
        .unwrap();
    assert!(forward.completed(), "{forward:#?}");
    assert!(crank_phase(&session, ids.crank_hinge) < 0.0);

    let reverse = session
        .continue_driver(
            session.revision(),
            SpatialAdaptiveContinuationRequest {
                driver_source: ids.driver,
                mode: AdaptiveContinuationMode::PseudoArclength {
                    path_length: 0.2,
                    initial_direction: ContinuationDirection::IncreasingParameter,
                },
                step_policy: policy(),
            },
        )
        .unwrap();
    assert!(reverse.completed(), "{reverse:#?}");
    assert_physical_samples(&reverse);
    assert!(crank_phase(&session, ids.crank_hinge) > 0.0);
    assert!(
        reverse
            .samples
            .iter()
            .any(|sample| sample.tangent_parameter_component < 0.0)
    );

    let (assembly, ids) = spatial_slider_crank(1.0);
    let mut away = SpatialAssemblySession::new(assembly, SolverConfig::default()).unwrap();
    let initial_target = 4.747_880_210_234_948;
    let result = away
        .continue_driver(
            away.revision(),
            SpatialAdaptiveContinuationRequest {
                driver_source: ids.driver,
                mode: AdaptiveContinuationMode::PseudoArclength {
                    path_length: 0.08,
                    initial_direction: ContinuationDirection::DecreasingParameter,
                },
                step_policy: policy(),
            },
        )
        .unwrap();
    assert!(result.completed(), "{result:#?}");
    assert!(result.accepted_target < initial_target);
    assert!(crank_phase(&away, ids.crank_hinge) > 0.05);
    assert!(
        result
            .samples
            .iter()
            .all(|sample| sample.tangent_parameter_component < 0.0)
    );
}

#[test]
fn spatial_continuation_rejection_retains_every_accepted_view() {
    let (assembly, ids) = spatial_slider_crank(1.0);
    let mut session = SpatialAssemblySession::new(assembly, SolverConfig::default()).unwrap();
    let revision = session.revision();
    let retained_assembly = session.assembly().clone();
    let retained_result = session.accepted_result().clone();
    let retained_gauge = session.gauge_report().clone();
    let retained_report = session.core_session().report().clone();
    let strict_policy = AdaptiveStepPolicy {
        initial_step: 0.04,
        minimum_step: 0.01,
        maximum_step: 0.04,
        maximum_correction: 1.0e-14,
        maximum_correction_step_ratio: 1.0e-12,
        max_retries: 8,
        ..policy()
    };
    let result = session
        .continue_driver(
            revision,
            SpatialAdaptiveContinuationRequest {
                driver_source: ids.driver,
                mode: AdaptiveContinuationMode::Natural { target: 4.72 },
                step_policy: strict_policy,
            },
        )
        .unwrap();

    assert!(matches!(
        result.status,
        SpatialAdaptiveContinuationStatus::CorrectionNotLocal { .. }
    ));
    assert!(result.samples.is_empty());
    assert!(!result.rejected_attempts.is_empty());
    assert_eq!(session.revision(), revision);
    assert_eq!(session.assembly(), &retained_assembly);
    assert_eq!(session.accepted_result(), &retained_result);
    assert_eq!(session.gauge_report(), &retained_gauge);
    assert_eq!(session.core_session().report(), &retained_report);
}

#[test]
fn spatial_pseudo_continuation_is_common_left_se3_equivariant() {
    let transform = Pose3::exp([1.7, -2.1, 0.9, -0.31, 0.22, 0.27]).unwrap();
    let (original_assembly, original_ids) = spatial_slider_crank(1.0);
    let (transformed_assembly, transformed_ids) =
        spatial_slider_crank_with_transform(1.0, Some(transform));
    let mut original =
        SpatialAssemblySession::new(original_assembly, SolverConfig::default()).unwrap();
    let mut transformed =
        SpatialAssemblySession::new(transformed_assembly, SolverConfig::default()).unwrap();
    let request = |driver_source| SpatialAdaptiveContinuationRequest {
        driver_source,
        mode: AdaptiveContinuationMode::PseudoArclength {
            path_length: 0.2,
            initial_direction: ContinuationDirection::IncreasingParameter,
        },
        step_policy: policy(),
    };
    let original_result = original
        .continue_driver(original.revision(), request(original_ids.driver))
        .unwrap();
    let transformed_result = transformed
        .continue_driver(transformed.revision(), request(transformed_ids.driver))
        .unwrap();

    assert!(original_result.completed(), "{original_result:#?}");
    assert!(transformed_result.completed(), "{transformed_result:#?}");
    assert!((original_result.accepted_target - transformed_result.accepted_target).abs() <= 2.0e-9);
    for (original_body, transformed_body) in original
        .accepted_result()
        .geometry
        .bodies
        .iter()
        .zip(&transformed.accepted_result().geometry.bodies)
    {
        let expected = transform.compose(&original_body.pose).unwrap();
        let difference = expected.local_difference(&transformed_body.pose).unwrap();
        assert!(norm3(&difference[0..3]) <= 2.0e-8);
        assert!(norm3(&difference[3..6]) <= 2.0e-8);
    }
}

fn run_pseudo_backend(
    scale: f64,
    backend: LinearSolveBackendPolicy,
) -> (SpatialAssemblySession, SpatialAdaptiveContinuationResult) {
    let (assembly, ids) = spatial_slider_crank(scale);
    let config = SolverConfig {
        linear_solve_backend: backend,
        ..SolverConfig::default()
    };
    let mut session = SpatialAssemblySession::new(assembly, config).unwrap();
    let result = session
        .continue_driver(
            session.revision(),
            SpatialAdaptiveContinuationRequest {
                driver_source: ids.driver,
                mode: AdaptiveContinuationMode::PseudoArclength {
                    path_length: 0.2,
                    initial_direction: ContinuationDirection::IncreasingParameter,
                },
                step_policy: policy(),
            },
        )
        .unwrap();
    assert!(result.completed(), "{result:#?}");
    (session, result)
}

#[test]
fn spatial_pseudo_continuation_has_dense_sparse_physical_parity() {
    for scale in [1.0e-6, 1.0, 1.0e6] {
        let (dense_session, dense) = run_pseudo_backend(scale, LinearSolveBackendPolicy::DenseOnly);
        let (sparse_session, sparse) =
            run_pseudo_backend(scale, LinearSolveBackendPolicy::SparsePreferred);
        assert!(
            sparse
                .samples
                .iter()
                .any(|sample| sample.corrector_backend == Some(LinearSolveBackend::SparseQr)),
            "scale={scale:e}, result={sparse:#?}"
        );
        assert!(
            sparse
                .samples
                .iter()
                .all(|sample| sample.corrector_sparse_fallback_reason.is_none())
        );
        assert_eq!(dense.status, sparse.status);
        assert!((dense.accepted_path_length - sparse.accepted_path_length).abs() <= 1.0e-14);
        assert!((dense.accepted_target - sparse.accepted_target).abs() / scale <= 2.0e-9);
        assert_eq!(
            dense_session.accepted_result().core_report.rank,
            sparse_session.accepted_result().core_report.rank
        );
        assert_eq!(
            dense_session.accepted_result().core_report.left_nullity,
            sparse_session.accepted_result().core_report.left_nullity
        );
        assert_eq!(
            dense_session.accepted_result().core_report.right_nullity,
            sparse_session.accepted_result().core_report.right_nullity
        );
        assert_eq!(
            dense_session.accepted_result().core_report.structural,
            sparse_session.accepted_result().core_report.structural
        );
        assert_eq!(
            dense_session.gauge_report().gauge_dof,
            sparse_session.gauge_report().gauge_dof
        );
        assert_eq!(
            dense_session.gauge_report().internal_mobility,
            sparse_session.gauge_report().internal_mobility
        );
        for (dense_body, sparse_body) in dense_session
            .accepted_result()
            .geometry
            .bodies
            .iter()
            .zip(&sparse_session.accepted_result().geometry.bodies)
        {
            let difference = dense_body.pose.local_difference(&sparse_body.pose).unwrap();
            assert!(norm3(&difference[0..3]) / scale <= 2.0e-8);
            assert!(norm3(&difference[3..6]) <= 2.0e-8);
        }
        for (dense_mode, sparse_mode) in dense_session
            .mode_evaluations()
            .iter()
            .zip(sparse_session.mode_evaluations())
        {
            assert!(dense_mode.retained && sparse_mode.retained);
            assert!(
                (dense_mode.retained_normalized_metric - sparse_mode.retained_normalized_metric)
                    .abs()
                    <= 2.0e-8
            );
        }
        assert_eq!(
            dense_session
                .source_mappings()
                .iter()
                .map(|mapping| (&mapping.source_label, mapping.residual_ids.len()))
                .collect::<Vec<_>>(),
            sparse_session
                .source_mappings()
                .iter()
                .map(|mapping| (&mapping.source_label, mapping.residual_ids.len()))
                .collect::<Vec<_>>()
        );
    }
}

fn norm3(values: &[f64]) -> f64 {
    values[0].hypot(values[1]).hypot(values[2])
}
