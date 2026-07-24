// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_core::SolverConfig;
use geosolve_geometry::Point2;
use geosolve_sketch::{
    CancellationToken, CurveDefinition, OperationCheckpoint, OperationControl, OperationLimits,
    OperationOutcome, OperationStopReason, OperationWork, OperationWorkCounter,
    RetainedSketchDocumentSession, Sketch, SketchDocument, SketchDocumentSession, SketchPatch,
    SketchSession, SketchSessionPatch, SketchSolveRequest, VisualProfileOptions, cancellation_pair,
    conflicting_rectangle,
};
use geosolve_sketch::{DocumentEdit, DocumentSolveRequest};
use std::sync::{Arc, Barrier};
use std::time::{Duration, Instant};

fn square_document() -> SketchDocument {
    let mut document = SketchDocument::new(4.0).unwrap();
    let points = [[0.0, 0.0], [4.0, 0.0], [4.0, 4.0], [0.0, 4.0]]
        .map(|position| document.add_point("square", position).unwrap());
    document
        .add_curve(
            "square",
            CurveDefinition::Polyline {
                points: points.to_vec(),
                closed: true,
                branch_directions: vec![[1.0, 0.0], [0.0, 1.0], [-1.0, 0.0], [0.0, -1.0]],
            },
        )
        .unwrap();
    document
}

fn disconnected_session() -> (SketchSession, geosolve_sketch::PointId) {
    let mut sketch = Sketch::new(1.0).unwrap();
    let first = sketch.add_point(Point2::new(0.0, 0.0)).unwrap();
    let second = sketch.add_point(Point2::new(5.0, 0.0)).unwrap();
    sketch
        .add_fixed_coordinate(first, geosolve_sketch::CoordinateAxis::Y, 0.0)
        .unwrap();
    sketch
        .add_fixed_coordinate(second, geosolve_sketch::CoordinateAxis::Y, 0.0)
        .unwrap();
    (
        SketchSession::new(
            sketch,
            SketchSolveRequest::default(),
            SolverConfig::default(),
        )
        .unwrap(),
        first,
    )
}

#[test]
fn controlled_patch_matches_incremental_revision_and_component_reuse() {
    let (mut legacy, first) = disconnected_session();
    let (mut controlled, controlled_first) = disconnected_session();
    let compilations = controlled.topology_compilations();
    let edit = SketchPatch::PointPosition {
        point: first,
        position: Point2::new(1.0, 0.0),
    };
    let legacy_result = legacy
        .apply_patch(SketchSessionPatch::new(legacy.revision(), edit))
        .unwrap();
    let controlled_edit = SketchPatch::PointPosition {
        point: controlled_first,
        position: Point2::new(1.0, 0.0),
    };
    let outcome = controlled
        .apply_patch_controlled(
            SketchSessionPatch::new(controlled.revision(), controlled_edit),
            OperationControl::default(),
        )
        .unwrap();
    let OperationOutcome::Completed {
        value: controlled_result,
        ..
    } = outcome
    else {
        panic!("unlimited controlled patch must complete");
    };

    assert!(controlled_result.accepted());
    assert_eq!(controlled.revision(), legacy.revision());
    assert_eq!(controlled.revisions(), legacy.revisions());
    assert_eq!(controlled.topology_compilations(), compilations);
    assert_eq!(
        controlled_result.core_report.component_solves,
        legacy_result.core_report.component_solves
    );
    assert!(
        controlled_result
            .core_report
            .component_solves
            .iter()
            .any(|component| component.reused)
    );
    assert!(
        controlled_result
            .core_report
            .component_solves
            .iter()
            .any(|component| !component.reused)
    );
}

#[test]
fn controlled_incremental_patch_exhaustion_retains_all_public_state() {
    let (mut session, first) = disconnected_session();
    let revision = session.revision();
    let revisions = session.revisions();
    let compilations = session.topology_compilations();
    let geometry = session.sketch().geometry();
    let accepted = session.accepted_result().clone();
    let mut limits = OperationLimits::unlimited();
    limits.component_linearizations = 0;

    let outcome = session
        .apply_patch_controlled(
            SketchSessionPatch::new(
                revision,
                SketchPatch::PointPosition {
                    point: first,
                    position: Point2::new(1.0, 0.0),
                },
            ),
            OperationControl::new(CancellationToken::default(), limits),
        )
        .unwrap();

    assert!(matches!(outcome, OperationOutcome::WorkExhausted { .. }));
    assert_eq!(session.revision(), revision);
    assert_eq!(session.revisions(), revisions);
    assert_eq!(session.topology_compilations(), compilations);
    assert_eq!(session.sketch().geometry(), geometry);
    assert_eq!(session.accepted_result(), &accepted);
}

#[test]
fn profile_cancellation_is_typed_and_persistence_neutral() {
    let document = square_document();
    let before = document.to_canonical_json().unwrap();
    let (handle, token) = cancellation_pair();
    handle.cancel();

    let outcome = document.analyze_visual_profiles_controlled(
        VisualProfileOptions::default(),
        OperationControl::new(token, OperationLimits::unlimited()),
    );
    let OperationOutcome::Cancelled { report } = outcome else {
        panic!("pre-cancelled profile analysis must not complete");
    };
    assert_eq!(
        report.stopping_reason,
        Some(OperationStopReason::Cancelled {
            checkpoint: OperationCheckpoint::ProfileCandidate,
        })
    );
    assert_eq!(document.to_canonical_json().unwrap(), before);
}

#[test]
fn profile_candidate_limit_reports_exact_counter_and_stop_boundary() {
    let document = square_document();
    let mut limits = OperationLimits::unlimited();
    limits.profile_candidate_pairs = 0;

    let outcome = document.analyze_visual_profiles_controlled(
        VisualProfileOptions::default(),
        OperationControl::new(CancellationToken::default(), limits),
    );
    let OperationOutcome::WorkExhausted { report } = outcome else {
        panic!("zero candidate-pair allowance must stop profile analysis");
    };
    assert_eq!(report.configured.profile_candidate_pairs, 0);
    assert_eq!(report.consumed.profile_candidate_pairs, 0);
    assert_eq!(
        report.stopping_reason,
        Some(OperationStopReason::WorkExhausted {
            counter: OperationWorkCounter::ProfileCandidatePairs,
            checkpoint: OperationCheckpoint::ProfileCandidate,
        })
    );
}

#[test]
fn retained_validation_limit_stops_before_lowering_and_preserves_lifecycle() {
    let document = square_document();
    let point = document.points()[0].id;
    let mut session = RetainedSketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let design = session.design_identity();
    let attempt = session.last_attempt().identity();
    let accepted = session
        .accepted_state()
        .map(geosolve_sketch::SketchAcceptedDocumentState::identity);
    let before = session.export_design_json().unwrap();
    let mut limits = OperationLimits::unlimited();
    limits.document_validation_items = 1;

    let outcome = session
        .apply_controlled(
            design,
            DocumentEdit::SetPointPosition {
                point,
                position: [0.5, 0.5],
            },
            OperationControl::new(CancellationToken::default(), limits),
        )
        .unwrap();
    let OperationOutcome::WorkExhausted { report } = outcome else {
        panic!("validation allowance must stop the retained edit");
    };
    assert_eq!(report.consumed.document_validation_items, 1);
    assert_eq!(report.consumed.document_lowering_items, 0);
    assert_eq!(
        report.stopping_reason,
        Some(OperationStopReason::WorkExhausted {
            counter: OperationWorkCounter::DocumentValidationItems,
            checkpoint: OperationCheckpoint::DocumentValidation,
        })
    );
    assert_eq!(session.design_identity(), design);
    assert_eq!(session.last_attempt().identity(), attempt);
    assert_eq!(
        session
            .accepted_state()
            .map(geosolve_sketch::SketchAcceptedDocumentState::identity),
        accepted
    );
    assert_eq!(session.export_design_json().unwrap(), before);
}

#[test]
fn retained_lowering_limit_stops_before_solver_work() {
    let document = square_document();
    let point = document.points()[0].id;
    let mut session = RetainedSketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let design = session.design_identity();
    let mut limits = OperationLimits::unlimited();
    limits.document_lowering_items = 0;

    let outcome = session
        .apply_controlled(
            design,
            DocumentEdit::SetPointPosition {
                point,
                position: [0.5, 0.5],
            },
            OperationControl::new(CancellationToken::default(), limits),
        )
        .unwrap();
    let OperationOutcome::WorkExhausted { report } = outcome else {
        panic!("zero lowering allowance must stop the retained edit");
    };
    assert_eq!(report.consumed.document_lowering_items, 0);
    assert_eq!(report.consumed.component_linearizations, 0);
    assert_eq!(report.consumed.rank_kernels, 0);
    assert_eq!(
        report.stopping_reason,
        Some(OperationStopReason::WorkExhausted {
            counter: OperationWorkCounter::DocumentLoweringItems,
            checkpoint: OperationCheckpoint::DocumentLowering,
        })
    );
    assert_eq!(session.design_identity(), design);
}

#[test]
fn rank_limit_stops_before_diagnostic_kernels() {
    let document = square_document();
    let (mut sketch, _) = document.lower().unwrap().into_parts();
    let before = sketch.geometry();
    let mut limits = OperationLimits::unlimited();
    limits.rank_kernels = 0;

    let outcome = sketch
        .solve_controlled(
            geosolve_sketch::SketchSolveRequest::default(),
            SolverConfig::default(),
            OperationControl::new(CancellationToken::default(), limits),
        )
        .unwrap();
    let OperationOutcome::WorkExhausted { report } = outcome else {
        panic!("zero rank-kernel allowance must stop solving");
    };
    assert_eq!(report.consumed.rank_kernels, 0);
    assert_eq!(report.consumed.diagnostic_candidates, 0);
    assert_eq!(report.consumed.diagnostic_trials, 0);
    assert_eq!(
        report.stopping_reason,
        Some(OperationStopReason::WorkExhausted {
            counter: OperationWorkCounter::RankKernels,
            checkpoint: OperationCheckpoint::BeforeRankKernel,
        })
    );
    assert_eq!(sketch.geometry(), before);
}

#[test]
fn pre_cancelled_sketch_solve_is_bitwise_neutral_before_clone_and_compile() {
    let document = square_document();
    let (mut sketch, _) = document.lower().unwrap().into_parts();
    let before = sketch.geometry();
    let (handle, token) = cancellation_pair();
    handle.cancel();

    let outcome = sketch
        .solve_controlled(
            geosolve_sketch::SketchSolveRequest::default(),
            SolverConfig::default(),
            OperationControl::new(token, OperationLimits::unlimited()),
        )
        .unwrap();

    let OperationOutcome::Cancelled { report } = outcome else {
        panic!("pre-cancelled sketch solve must not complete");
    };
    assert_eq!(
        report.stopping_reason,
        Some(OperationStopReason::Cancelled {
            checkpoint: OperationCheckpoint::DocumentLowering,
        })
    );
    assert_eq!(sketch.geometry(), before);
    assert_eq!(report.consumed, OperationWork::default());
}

#[test]
fn zero_lowering_limit_stops_public_lowering_and_direct_compile() {
    let document = square_document();
    let mut limits = OperationLimits::unlimited();
    limits.document_lowering_items = 0;

    let outcome = document
        .lower_controlled(OperationControl::new(CancellationToken::default(), limits))
        .unwrap();
    let OperationOutcome::WorkExhausted { report } = outcome else {
        panic!("zero lowering allowance must stop public document lowering");
    };
    assert_eq!(report.consumed.document_lowering_items, 0);
    assert_eq!(
        report.stopping_reason,
        Some(OperationStopReason::WorkExhausted {
            counter: OperationWorkCounter::DocumentLoweringItems,
            checkpoint: OperationCheckpoint::DocumentLowering,
        })
    );

    let (mut sketch, _) = document.lower().unwrap().into_parts();
    let before = sketch.geometry();
    let outcome = sketch
        .solve_controlled(
            geosolve_sketch::SketchSolveRequest::default(),
            SolverConfig::default(),
            OperationControl::new(CancellationToken::default(), limits),
        )
        .unwrap();
    let OperationOutcome::WorkExhausted { report } = outcome else {
        panic!("zero lowering allowance must stop direct sketch compilation");
    };
    assert_eq!(report.consumed.document_lowering_items, 0);
    assert_eq!(report.consumed.component_linearizations, 0);
    assert_eq!(sketch.geometry(), before);
}

#[test]
fn pre_cancelled_initial_construction_returns_no_session() {
    let document = square_document();
    let (sketch, _) = document.lower().unwrap().into_parts();
    let (handle, token) = cancellation_pair();
    handle.cancel();

    let sketch_outcome = SketchSession::new_controlled(
        sketch,
        geosolve_sketch::SketchSolveRequest::default(),
        SolverConfig::default(),
        OperationControl::new(token.clone(), OperationLimits::unlimited()),
    )
    .unwrap();
    assert!(matches!(sketch_outcome, OperationOutcome::Cancelled { .. }));

    let document_outcome = SketchDocumentSession::new_controlled(
        document.clone(),
        DocumentSolveRequest::default(),
        SolverConfig::default(),
        OperationControl::new(token.clone(), OperationLimits::unlimited()),
    )
    .unwrap();
    assert!(matches!(
        document_outcome,
        OperationOutcome::Cancelled { .. }
    ));

    let retained_outcome = RetainedSketchDocumentSession::new_controlled(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
        OperationControl::new(token, OperationLimits::unlimited()),
    )
    .unwrap();
    assert!(matches!(
        retained_outcome,
        OperationOutcome::Cancelled { .. }
    ));
}

#[test]
fn zero_lowering_limit_stops_all_initial_constructors() {
    let document = square_document();
    let (sketch, _) = document.lower().unwrap().into_parts();
    let mut limits = OperationLimits::unlimited();
    limits.document_lowering_items = 0;

    let sketch_outcome = SketchSession::new_controlled(
        sketch,
        geosolve_sketch::SketchSolveRequest::default(),
        SolverConfig::default(),
        OperationControl::new(CancellationToken::default(), limits),
    )
    .unwrap();
    assert!(matches!(
        sketch_outcome,
        OperationOutcome::WorkExhausted { .. }
    ));

    let document_outcome = SketchDocumentSession::new_controlled(
        document.clone(),
        DocumentSolveRequest::default(),
        SolverConfig::default(),
        OperationControl::new(CancellationToken::default(), limits),
    )
    .unwrap();
    assert!(matches!(
        document_outcome,
        OperationOutcome::WorkExhausted { .. }
    ));

    let retained_outcome = RetainedSketchDocumentSession::new_controlled(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
        OperationControl::new(CancellationToken::default(), limits),
    )
    .unwrap();
    assert!(matches!(
        retained_outcome,
        OperationOutcome::WorkExhausted { .. }
    ));
}

#[test]
fn accepted_session_construction_recompile_is_charged() {
    let document = square_document();
    let (sketch, _) = document.lower().unwrap().into_parts();
    let mut direct = sketch.clone();
    let direct_outcome = direct
        .solve_controlled(
            geosolve_sketch::SketchSolveRequest::default(),
            SolverConfig::default(),
            OperationControl::unlimited(),
        )
        .unwrap();
    let direct_compile_items = direct_outcome.report().consumed.document_lowering_items;
    assert!(matches!(direct_outcome, OperationOutcome::Completed { .. }));

    let mut limits = OperationLimits::unlimited();
    limits.document_lowering_items = direct_compile_items;
    let outcome = SketchSession::new_controlled(
        sketch,
        geosolve_sketch::SketchSolveRequest::default(),
        SolverConfig::default(),
        OperationControl::new(CancellationToken::default(), limits),
    )
    .unwrap();
    let OperationOutcome::WorkExhausted { report } = outcome else {
        panic!("accepted-session recompilation must consume compile work");
    };
    assert_eq!(
        report.consumed.document_lowering_items,
        direct_compile_items
    );
    assert_eq!(
        report.stopping_reason,
        Some(OperationStopReason::WorkExhausted {
            counter: OperationWorkCounter::DocumentLoweringItems,
            checkpoint: OperationCheckpoint::DocumentLowering,
        })
    );
}

#[test]
fn pre_cancelled_rejecting_construction_is_cancelled_not_rejected() {
    let (sketch, _) = conflicting_rectangle().unwrap();
    let (handle, token) = cancellation_pair();
    handle.cancel();

    let outcome = SketchSession::new_controlled(
        sketch,
        geosolve_sketch::SketchSolveRequest::default(),
        SolverConfig::default(),
        OperationControl::new(token, OperationLimits::unlimited()),
    )
    .unwrap();
    assert!(matches!(outcome, OperationOutcome::Cancelled { .. }));
}

#[test]
fn controlled_sketch_session_rebuild_rolls_back_all_public_state() {
    let document = square_document();
    let (sketch, _) = document.lower().unwrap().into_parts();
    let mut session = SketchSession::new(
        sketch,
        geosolve_sketch::SketchSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let revision = session.revision();
    let geometry = session.sketch().geometry();
    let accepted = session.accepted_result().clone();
    let (handle, token) = cancellation_pair();
    handle.cancel();

    let outcome = session
        .rebuild_request_controlled(
            revision,
            geosolve_sketch::SketchSolveRequest::default(),
            OperationControl::new(token, OperationLimits::unlimited()),
        )
        .unwrap();

    assert!(matches!(outcome, OperationOutcome::Cancelled { .. }));
    assert_eq!(session.revision(), revision);
    assert_eq!(session.sketch().geometry(), geometry);
    assert_eq!(session.accepted_result(), &accepted);
}

#[test]
fn controlled_document_rebuild_rolls_back_revision_geometry_and_audit() {
    let document = square_document();
    let mut session = SketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let revision = session.revision();
    let json = session.export_json().unwrap();
    let result = session.accepted_result();
    let (handle, token) = cancellation_pair();
    handle.cancel();

    let outcome = session
        .rebuild_request_controlled(
            revision,
            DocumentSolveRequest::default(),
            OperationControl::new(token, OperationLimits::unlimited()),
        )
        .unwrap();

    assert!(matches!(outcome, OperationOutcome::Cancelled { .. }));
    assert_eq!(session.revision(), revision);
    assert_eq!(session.export_json().unwrap(), json);
    assert_eq!(
        session.accepted_result().accepted_view(),
        result.accepted_view()
    );
    assert_eq!(session.accepted_result().mappings(), result.mappings());
}

#[test]
fn priority_factorization_limit_stops_before_reprojection_and_publication() {
    let document = square_document();
    let persistent_point = document.points()[0].id;
    let (mut sketch, mappings) = document.lower().unwrap().into_parts();
    let point = mappings.runtime_point(persistent_point).unwrap();
    let before = sketch.geometry();
    let mut limits = OperationLimits::unlimited();
    limits.factorizations = 0;

    let outcome = sketch
        .solve_controlled(
            geosolve_sketch::SketchSolveRequest::default()
                .without_previous_state_preferences()
                .with_drag(point, Point2::new(1.0, 2.0)),
            SolverConfig::default(),
            OperationControl::new(CancellationToken::default(), limits),
        )
        .unwrap();
    let OperationOutcome::WorkExhausted { report } = outcome else {
        panic!("zero factorization allowance must stop the Temporary priority solve");
    };
    assert_eq!(report.consumed.nonlinear_iterations, 1);
    assert_eq!(report.consumed.factorizations, 0);
    assert_eq!(report.consumed.rejected_trials, 0);
    assert_eq!(
        report.stopping_reason,
        Some(OperationStopReason::WorkExhausted {
            counter: OperationWorkCounter::Factorizations,
            checkpoint: OperationCheckpoint::BeforeFactorization,
        })
    );
    assert_eq!(sketch.geometry(), before);
}

#[test]
fn priority_factorization_limit_authorizes_each_kernel_individually() {
    let document = square_document();
    let persistent_point = document.points()[0].id;
    let (mut sketch, mappings) = document.lower().unwrap().into_parts();
    let point = mappings.runtime_point(persistent_point).unwrap();
    let before = sketch.geometry();
    let mut limits = OperationLimits::unlimited();
    limits.factorizations = 1;

    let outcome = sketch
        .solve_controlled(
            geosolve_sketch::SketchSolveRequest::default()
                .without_previous_state_preferences()
                .with_drag(point, Point2::new(1.0, 2.0)),
            SolverConfig::default(),
            OperationControl::new(CancellationToken::default(), limits),
        )
        .unwrap();
    let OperationOutcome::WorkExhausted { report } = outcome else {
        panic!("one factorization allowance must stop before the second kernel");
    };
    assert_eq!(report.consumed.factorizations, 1);
    assert_eq!(
        report.stopping_reason,
        Some(OperationStopReason::WorkExhausted {
            counter: OperationWorkCounter::Factorizations,
            checkpoint: OperationCheckpoint::BeforeFactorization,
        })
    );
    assert_eq!(sketch.geometry(), before);
}

#[test]
fn profile_fragment_limit_stops_before_fragment_construction() {
    let document = square_document();
    let mut limits = OperationLimits::unlimited();
    limits.profile_fragments = 0;

    let outcome = document.analyze_visual_profiles_controlled(
        VisualProfileOptions::default(),
        OperationControl::new(CancellationToken::default(), limits),
    );
    let OperationOutcome::WorkExhausted { report } = outcome else {
        panic!("zero fragment allowance must stop profile analysis");
    };
    assert_eq!(report.consumed.profile_fragments, 0);
    assert_eq!(report.consumed.profile_faces, 0);
    assert_eq!(
        report.stopping_reason,
        Some(OperationStopReason::WorkExhausted {
            counter: OperationWorkCounter::ProfileFragments,
            checkpoint: OperationCheckpoint::ProfileSubdivision,
        })
    );
}

#[test]
fn profile_face_limit_stops_before_face_construction() {
    let document = square_document();
    let mut limits = OperationLimits::unlimited();
    limits.profile_faces = 0;

    let outcome = document.analyze_visual_profiles_controlled(
        VisualProfileOptions::default(),
        OperationControl::new(CancellationToken::default(), limits),
    );
    let OperationOutcome::WorkExhausted { report } = outcome else {
        panic!("zero face allowance must stop profile analysis");
    };
    assert_eq!(report.consumed.profile_fragments, 4);
    assert_eq!(report.consumed.profile_faces, 0);
    assert_eq!(
        report.stopping_reason,
        Some(OperationStopReason::WorkExhausted {
            counter: OperationWorkCounter::ProfileFaces,
            checkpoint: OperationCheckpoint::ProfileFace,
        })
    );
}

#[test]
#[ignore = "manual reproducible cancellation-latency measurement"]
fn measure_native_profile_cancellation_latency() {
    let mut document = SketchDocument::new(2_000.0).unwrap();
    let line_length = 1_000.0_f64.hypot(1.0);
    for index in 0..1_000 {
        let y = f64::from(index) * 2.0;
        let start = document.add_point("latency", [0.0, y]).unwrap();
        let end = document.add_point("latency", [1_000.0, y + 1.0]).unwrap();
        document
            .add_curve(
                "latency",
                CurveDefinition::Line {
                    start,
                    end,
                    branch_direction: [1_000.0 / line_length, 1.0 / line_length],
                },
            )
            .unwrap();
    }
    let options = VisualProfileOptions {
        max_candidate_pairs: 1_000_000,
        ..VisualProfileOptions::default()
    };
    let mut maximum = Duration::ZERO;
    for _ in 0..20 {
        let document = document.clone();
        let (handle, token) = cancellation_pair();
        let started = Arc::new(Barrier::new(2));
        let worker_started = Arc::clone(&started);
        let worker = std::thread::spawn(move || {
            worker_started.wait();
            document.analyze_visual_profiles_controlled(
                options,
                OperationControl::new(token, OperationLimits::unlimited()),
            )
        });
        started.wait();
        std::thread::sleep(Duration::from_millis(1));
        let requested = Instant::now();
        handle.cancel();
        let outcome = worker.join().unwrap();
        maximum = maximum.max(requested.elapsed());
        assert!(matches!(outcome, OperationOutcome::Cancelled { .. }));
    }
    println!("maximum request-to-return latency over 20 runs: {maximum:?}");
}
