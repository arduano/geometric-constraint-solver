// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_core::{
    AdaptiveStepPolicy, AuditEvaluationStatus, HardValidity, SolveTermination, SolverConfig,
};
use geosolve_linkage::{
    AdaptiveContinuationMode, SpatialAdaptiveContinuationRequest, SpatialAssemblyEdit,
    SpatialAssemblyError, SpatialAssemblySession, SpatialAssemblyTransaction, SpatialAxisParity,
    SpatialCoordinateKind, SpatialCoordinateValueKind, SpatialExampleIds, SpatialExampleKind,
    SpatialHingeTarget, SpatialModeMonitorKind, SpatialModeSign, SpatialPlanarTranslationAxis,
    SpatialSolveResult, SpatialSourceKind, spatial_example,
};

const RESIDUAL_TOLERANCE: f64 = 1.0e-9;

fn continuation_policy() -> AdaptiveStepPolicy {
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

fn assert_independently_accepted(accepted: &SpatialSolveResult) {
    assert_eq!(
        accepted.core_report.termination,
        SolveTermination::Converged
    );
    assert_eq!(accepted.core_report.hard_validity, HardValidity::Valid);
    assert!(accepted.core_report.hard_residuals_validated);
    assert!(accepted.core_report.rank_is_valid);
    assert!(accepted.acceptance_hard_residual_max <= RESIDUAL_TOLERANCE);
    assert!(accepted.core_report.hard_residual_max <= RESIDUAL_TOLERANCE);
    assert!(
        accepted
            .geometry
            .bodies
            .iter()
            .all(|body| { body.pose.ambient().iter().all(|value| value.is_finite()) })
    );
    assert!(
        accepted
            .geometry
            .points
            .iter()
            .all(|point| { point.world.coords.iter().all(|value| value.is_finite()) })
    );
    assert!(
        accepted
            .geometry
            .frames
            .iter()
            .map(|feature| feature.world)
            .chain(accepted.geometry.axes.iter().map(|feature| feature.world))
            .chain(accepted.geometry.planes.iter().map(|feature| feature.world))
            .all(|frame| {
                frame
                    .origin()
                    .coords
                    .iter()
                    .chain(frame.x_axis().iter())
                    .chain(frame.y_axis().iter())
                    .chain(frame.z_axis().iter())
                    .all(|value| value.is_finite())
            })
    );
    assert!(
        accepted
            .coordinate_values
            .iter()
            .all(|coordinate| match coordinate.value {
                SpatialCoordinateValueKind::Hinge(value) => value.principal_phase.is_finite(),
                SpatialCoordinateValueKind::AxialTranslation(value)
                | SpatialCoordinateValueKind::PlanarTranslation { value, .. } => value.is_finite(),
            })
    );
    assert!(accepted.mode_evaluations.iter().all(|mode| {
        mode.retained
            && mode.retained_normalized_metric.is_finite()
            && mode.fresh_raw_metric.is_some_and(f64::is_finite)
    }));
    assert!(accepted.display_audit.sources.iter().all(|source| {
        source.rows.iter().all(|row| {
            row.evaluation_status == AuditEvaluationStatus::Evaluated
                && row.raw_residual.is_finite()
                && row.normalized_residual.is_finite()
                && row.scale.is_finite()
                && row.scale > 0.0
        })
    }));
}

fn assert_report_source_identity(session: &SpatialAssemblySession) {
    let accepted = session.accepted_result();
    assert_eq!(accepted.source_mappings, session.source_mappings());
    assert_eq!(
        accepted.source_mappings.len(),
        accepted.display_audit.sources.len()
    );
    for mapping in &accepted.source_mappings {
        assert!(!mapping.residual_ids.is_empty());
        let audit = accepted
            .display_audit
            .sources
            .iter()
            .find(|audit| audit.source_id == mapping.core_source_id)
            .expect("every physical source mapping must own one audit source");
        assert_eq!(audit.source_label, mapping.source_label);
    }
}

fn assert_shaft_modes(
    result: &SpatialSolveResult,
    ids: geosolve_linkage::ShaftBearingExampleIds,
    scale: f64,
) {
    assert_eq!(result.mode_evaluations.len(), ids.monitors.len());
    let parity = result
        .mode_evaluations
        .iter()
        .find(|mode| mode.monitor_id == ids.monitors[0])
        .unwrap();
    assert!(matches!(
        parity.kind,
        SpatialModeMonitorKind::AxisParity {
            parity: SpatialAxisParity::Aligned,
            ..
        }
    ));
    assert!(parity.retained_normalized_metric > 0.999);

    let winding = result
        .mode_evaluations
        .iter()
        .find(|mode| mode.monitor_id == ids.monitors[1])
        .unwrap();
    assert!(matches!(
        winding.kind,
        SpatialModeMonitorKind::HingeWinding { coordinate, winding: 2 }
            if coordinate == ids.coordinates[0]
    ));

    let side = result
        .mode_evaluations
        .iter()
        .find(|mode| mode.monitor_id == ids.monitors[2])
        .unwrap();
    assert!(matches!(
        side.kind,
        SpatialModeMonitorKind::PlaneSide { plane, point, side: SpatialModeSign::Positive }
            if plane == ids.translation_plane && point == ids.translation_witness
    ));
    assert!(side.fresh_raw_metric.unwrap() / scale > 0.0);
    assert!(side.retained_normalized_metric > 0.0);
}

fn assert_block_modes(
    result: &SpatialSolveResult,
    ids: &geosolve_linkage::BlockBaseExampleIds,
    scale: f64,
) {
    // The public fixture exposes only these three monitors; it exposes no ordered-volume ID.
    assert_eq!(result.mode_evaluations.len(), ids.monitors.len());
    let parity = result
        .mode_evaluations
        .iter()
        .find(|mode| mode.monitor_id == ids.monitors[0])
        .unwrap();
    assert!(matches!(
        parity.kind,
        SpatialModeMonitorKind::AxisParity {
            parity: SpatialAxisParity::Aligned,
            ..
        }
    ));
    assert!(parity.retained_normalized_metric > 0.999);

    let winding = result
        .mode_evaluations
        .iter()
        .find(|mode| mode.monitor_id == ids.monitors[1])
        .unwrap();
    assert!(matches!(
        winding.kind,
        SpatialModeMonitorKind::HingeWinding { coordinate, winding: 3 }
            if coordinate == ids.coordinates[0]
    ));

    let side = result
        .mode_evaluations
        .iter()
        .find(|mode| mode.monitor_id == ids.monitors[2])
        .unwrap();
    assert!(matches!(
        side.kind,
        SpatialModeMonitorKind::PlaneSide { plane, point, side: SpatialModeSign::Positive }
            if plane == ids.planes[0] && point == ids.side_witness
    ));
    assert!(side.fresh_raw_metric.unwrap() / scale > 0.0);
    assert!(side.retained_normalized_metric > 0.0);
}

fn assert_block_base_initial_state(
    session: &SpatialAssemblySession,
    ids: &geosolve_linkage::BlockBaseExampleIds,
    scale: f64,
) {
    assert_independently_accepted(session.accepted_result());
    assert!(matches!(
        session.assembly().source(ids.joint).unwrap().kind(),
        SpatialSourceKind::PlanarJoint {
            parity: SpatialAxisParity::Aligned,
            ..
        }
    ));
    assert!(matches!(
        session.assembly().coordinate(ids.coordinates[0]).unwrap().kind(),
        SpatialCoordinateKind::Hinge { parent, winding: 3 } if parent == ids.joint
    ));
    for (coordinate, axis) in [
        (ids.coordinates[1], SpatialPlanarTranslationAxis::X),
        (ids.coordinates[2], SpatialPlanarTranslationAxis::Y),
    ] {
        assert!(matches!(
            session.assembly().coordinate(coordinate).unwrap().kind(),
            SpatialCoordinateKind::PlanarTranslation { parent, axis: actual }
                if parent == ids.joint && actual == axis
        ));
    }
    assert_block_modes(session.accepted_result(), ids, scale);
    assert_report_source_identity(session);
    let initial_hinge = session.coordinate_value(ids.coordinates[0]).unwrap();
    let SpatialCoordinateValueKind::Hinge(initial_hinge) = initial_hinge.value else {
        panic!("block hinge coordinate changed kind");
    };
    assert_eq!(initial_hinge.winding, 3);
    assert!((initial_hinge.principal_phase - 0.37).abs() <= 2.0e-9);
    for (coordinate, axis, expected) in [
        (ids.coordinates[1], SpatialPlanarTranslationAxis::X, 1.25),
        (ids.coordinates[2], SpatialPlanarTranslationAxis::Y, -0.8),
    ] {
        let value = session.coordinate_value(coordinate).unwrap();
        let SpatialCoordinateValueKind::PlanarTranslation {
            axis: actual,
            value,
        } = value.value
        else {
            panic!("block translation coordinate changed kind");
        };
        assert_eq!(actual, axis);
        assert!((value / scale - expected).abs() <= 2.0e-9);
    }
}

fn apply_and_assert_block_base_combined_edit(
    session: &mut SpatialAssemblySession,
    ids: &geosolve_linkage::BlockBaseExampleIds,
    scale: f64,
) {
    let revision = session.revision();
    session
        .apply_transaction(SpatialAssemblyTransaction::new(
            revision,
            vec![
                SpatialAssemblyEdit::HingeDriverTarget {
                    source: ids.drivers[0],
                    target: SpatialHingeTarget {
                        principal_phase: -0.42,
                        winding: 3,
                    },
                },
                SpatialAssemblyEdit::TranslationDriverTarget {
                    source: ids.drivers[1],
                    target: -2.1 * scale,
                },
                SpatialAssemblyEdit::TranslationDriverTarget {
                    source: ids.drivers[2],
                    target: 1.6 * scale,
                },
            ],
        ))
        .unwrap();
    assert_eq!(session.revision(), revision + 1);
    assert_independently_accepted(session.accepted_result());
    assert_eq!(session.accepted_result().core_report.rank, 6);
    assert_eq!(session.gauge_report().gauge_dof, 0);
    assert_eq!(session.gauge_report().internal_mobility, 0);
    assert_report_source_identity(session);
    assert_block_modes(session.accepted_result(), ids, scale);
    let hinge = session.coordinate_value(ids.coordinates[0]).unwrap();
    let SpatialCoordinateValueKind::Hinge(hinge) = hinge.value else {
        panic!("block hinge coordinate changed kind");
    };
    assert_eq!(hinge.winding, 3);
    assert!((hinge.principal_phase + 0.42).abs() <= 2.0e-9);
    for (coordinate, axis, expected) in [
        (ids.coordinates[1], SpatialPlanarTranslationAxis::X, -2.1),
        (ids.coordinates[2], SpatialPlanarTranslationAxis::Y, 1.6),
    ] {
        let value = session.coordinate_value(coordinate).unwrap();
        let SpatialCoordinateValueKind::PlanarTranslation {
            axis: actual,
            value,
        } = value.value
        else {
            panic!("block translation coordinate changed kind");
        };
        assert_eq!(actual, axis);
        assert!((value / scale - expected).abs() <= 2.0e-9);
    }
}

fn assert_block_base_invalid_combined_edit_rolls_back(
    session: &mut SpatialAssemblySession,
    ids: &geosolve_linkage::BlockBaseExampleIds,
    scale: f64,
) {
    let retained_revision = session.revision();
    let retained_assembly = session.assembly().clone();
    let retained_result = session.accepted_result().clone();
    let retained_mappings = session.source_mappings().to_vec();
    let retained_gauge = session.gauge_report().clone();
    let retained_report = session.core_session().report().clone();
    let retained_linearization = session
        .core_session()
        .accepted_hard_linearization()
        .unwrap();

    assert!(matches!(
        session.apply_transaction(SpatialAssemblyTransaction::new(
            retained_revision,
            vec![
                SpatialAssemblyEdit::HingeDriverTarget {
                    source: ids.drivers[0],
                    target: SpatialHingeTarget {
                        principal_phase: -0.15,
                        winding: 3,
                    },
                },
                SpatialAssemblyEdit::TranslationDriverTarget {
                    source: ids.drivers[1],
                    target: 0.4 * scale,
                },
                SpatialAssemblyEdit::TranslationDriverTarget {
                    source: ids.drivers[2],
                    target: 2.3 * scale,
                },
                SpatialAssemblyEdit::MonitorPlaneSide {
                    monitor: ids.monitors[2],
                    side: SpatialModeSign::Negative,
                },
            ],
        )),
        Err(SpatialAssemblyError::IndependentValidation(message)) if message.contains("plane side")
    ));
    assert_eq!(session.revision(), retained_revision);
    assert_eq!(session.assembly(), &retained_assembly);
    assert_eq!(session.accepted_result(), &retained_result);
    assert_eq!(session.source_mappings(), retained_mappings);
    assert_eq!(session.gauge_report(), &retained_gauge);
    assert_eq!(session.core_session().report(), &retained_report);
    assert_eq!(
        session
            .core_session()
            .accepted_hard_linearization()
            .unwrap(),
        retained_linearization
    );
}

#[test]
fn legacy_shaft_bearing_consumer_signature_preserves_scale_report_and_continuation_state() {
    for scale in [1.0e-6, 1.0, 1.0e6] {
        let fixture = spatial_example(SpatialExampleKind::ShaftBearing, scale).unwrap();
        let SpatialExampleIds::ShaftBearing(ids) = fixture.ids else {
            unreachable!("shaft-bearing fixture returned the wrong public identity set");
        };
        let mut session =
            SpatialAssemblySession::new(fixture.assembly, SolverConfig::default()).unwrap();

        assert_independently_accepted(session.accepted_result());
        assert_eq!(session.accepted_result().core_report.rank, 6);
        assert_eq!(session.gauge_report().internal_mobility, 0);
        assert_eq!(session.gauge_report().gauge_dof, 0);
        assert_report_source_identity(&session);
        for source in [ids.joint, ids.drivers[0], ids.drivers[1]] {
            assert!(
                session
                    .source_mappings()
                    .iter()
                    .any(|mapping| mapping.source == source)
            );
        }

        let revision = session.revision();
        let expected_mappings = session.source_mappings().to_vec();
        let continuation = session
            .continue_driver(
                revision,
                SpatialAdaptiveContinuationRequest {
                    driver_source: ids.drivers[0],
                    mode: AdaptiveContinuationMode::Natural { target: 0.82 },
                    step_policy: continuation_policy(),
                },
            )
            .unwrap();
        assert!(
            continuation.completed(),
            "scale={scale:e}: {continuation:#?}"
        );
        assert!(!continuation.samples.is_empty());
        assert_eq!(continuation.accepted_revision, session.revision());
        assert_independently_accepted(&continuation.initial_solve);
        assert_eq!(
            continuation.initial_solve.source_mappings,
            expected_mappings
        );
        assert_shaft_modes(&continuation.initial_solve, ids, scale);
        let mut previous_target = continuation.initial_target;
        for sample in &continuation.samples {
            assert_independently_accepted(&sample.solve);
            assert_eq!(sample.solve.source_mappings, expected_mappings);
            assert!(sample.driver_target > previous_target);
            assert!(sample.driver_target <= 0.82);
            assert!(sample.path_step.is_finite() && sample.path_step > 0.0);
            assert!(sample.correction_norm.is_finite() && sample.correction_norm >= 0.0);
            let hinge = sample
                .solve
                .coordinate_values
                .iter()
                .find(|value| value.coordinate == ids.coordinates[0])
                .unwrap();
            let SpatialCoordinateValueKind::Hinge(hinge) = hinge.value else {
                panic!("shaft hinge coordinate changed kind");
            };
            assert_eq!(hinge.winding, 2);
            assert!((hinge.principal_phase - sample.driver_target).abs() <= 2.0e-9);
            let translation = sample
                .solve
                .coordinate_values
                .iter()
                .find(|value| value.coordinate == ids.coordinates[1])
                .unwrap();
            let SpatialCoordinateValueKind::AxialTranslation(translation) = translation.value
            else {
                panic!("shaft translation coordinate changed kind");
            };
            assert!((translation / scale - 1.9).abs() <= 2.0e-9);
            assert_shaft_modes(&sample.solve, ids, scale);
            previous_target = sample.driver_target;
        }
        assert!((continuation.accepted_target - 0.82).abs() <= 2.0e-9);
        assert_independently_accepted(session.accepted_result());
        assert_eq!(session.accepted_result().core_report.rank, 6);
        assert_eq!(session.gauge_report().internal_mobility, 0);
        assert_report_source_identity(&session);
        assert_shaft_modes(session.accepted_result(), ids, scale);

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
fn legacy_block_base_consumer_signature_commits_once_and_rolls_back_combined_invalid_edit() {
    for scale in [1.0e-6, 1.0, 1.0e6] {
        let fixture = spatial_example(SpatialExampleKind::BlockBase, scale).unwrap();
        let SpatialExampleIds::BlockBase(ids) = fixture.ids else {
            unreachable!("block-base fixture returned the wrong public identity set");
        };
        let mut session =
            SpatialAssemblySession::new(fixture.assembly, SolverConfig::default()).unwrap();
        assert_block_base_initial_state(&session, &ids, scale);
        apply_and_assert_block_base_combined_edit(&mut session, &ids, scale);
        assert_block_base_invalid_combined_edit_rolls_back(&mut session, &ids, scale);
    }
}
