// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_core::SolverConfig;
use geosolve_sketch::{
    AlphaScenarioIds, AlphaScenarioKind, DocumentCommandEffect, DocumentEdit, DocumentSessionError,
    DocumentSolveRequest, ExternalFeatureKindV1, ExternalSnapshotDigest, ExternalSnapshotEntry,
    ExternalSnapshotFeatureV1, ExternalSnapshotResourcesV1, ExternalSnapshotSet, OperationControl,
    OperationLimits, OperationOutcome, ParameterBatch, PreparedSketchCommit, PreparedSketchInput,
    PreparedSketchJob, PreparedSketchOperation, PreparedSketchPatch, PreparedSketchSnapshot,
    RetainedSketchDocumentSession, alpha_scenario,
};

fn fixture() -> (
    RetainedSketchDocumentSession,
    geosolve_sketch::DesignPointId,
) {
    let fixture = alpha_scenario(AlphaScenarioKind::A2, 1.0).unwrap();
    let AlphaScenarioIds::A2(ids) = fixture.ids else {
        panic!("A2 IDs");
    };
    (
        RetainedSketchDocumentSession::new(
            fixture.document,
            fixture.request,
            SolverConfig::default(),
        )
        .unwrap(),
        ids.c,
    )
}

fn pair_bits(value: [f64; 2]) -> [u64; 2] {
    value.map(f64::to_bits)
}

fn completed_patch(outcome: OperationOutcome<PreparedSketchPatch>) -> PreparedSketchPatch {
    match outcome {
        OperationOutcome::Completed { value, .. } => value,
        OperationOutcome::Cancelled { .. } => panic!("prepared job was cancelled"),
        OperationOutcome::WorkExhausted { .. } => panic!("prepared job exhausted work"),
        _ => panic!("unknown operation outcome"),
    }
}

fn assert_send_sync<T: Send + Sync>() {}
fn assert_send<T: Send>() {}

#[test]
fn native_worker_job_is_non_mutating_until_exact_cas_commit() {
    assert_send::<PreparedSketchSnapshot>();
    assert_send::<PreparedSketchJob>();
    assert_send::<PreparedSketchPatch>();
    assert_send_sync::<PreparedSketchInput>();
    assert_send_sync::<PreparedSketchOperation>();
    assert_send_sync::<PreparedSketchCommit>();

    let (mut session, point) = fixture();
    let before_design = session.design_identity();
    let before_attempt = session.last_attempt().identity();
    let before_accepted = session.accepted_state().unwrap().identity();
    let before_position = session.design_document().point(point).unwrap().position;

    let snapshot = session.prepared_snapshot();
    assert_eq!(snapshot.input().design_identity(), before_design);
    assert_eq!(snapshot.input().latest_attempt_identity(), before_attempt);
    assert_eq!(
        snapshot.input().accepted_state_identity(),
        Some(before_accepted)
    );
    assert_eq!(
        snapshot.accepted_state().unwrap().identity(),
        before_accepted
    );

    let job = snapshot.prepare(PreparedSketchOperation::Apply(
        DocumentEdit::SetPointPosition {
            point,
            position: [before_position[0] + 0.25, before_position[1] + 0.1],
        },
    ));
    let outcome = std::thread::spawn(move || job.execute(OperationControl::unlimited()))
        .join()
        .unwrap()
        .unwrap();

    assert_eq!(session.design_identity(), before_design);
    assert_eq!(session.last_attempt().identity(), before_attempt);
    assert_eq!(
        session.accepted_state().unwrap().identity(),
        before_accepted
    );
    assert_eq!(
        pair_bits(session.design_document().point(point).unwrap().position),
        pair_bits(before_position)
    );

    let patch = completed_patch(outcome);
    assert_eq!(patch.base_input().design_identity(), before_design);
    let candidate = patch.proposed_commit();
    assert_eq!(
        candidate.operation(),
        geosolve_sketch::PreparedSketchOperationKind::Apply
    );
    let committed = session.commit_prepared_patch(patch).unwrap();
    assert_eq!(committed, candidate);
    assert_ne!(session.design_identity(), before_design);
    assert_ne!(session.last_attempt().identity(), before_attempt);
    assert_eq!(
        pair_bits(session.design_document().point(point).unwrap().position),
        pair_bits([before_position[0] + 0.25, before_position[1] + 0.1])
    );
}

#[test]
fn stale_out_of_order_patch_cannot_overwrite_a_newer_commit() {
    let (mut session, point) = fixture();
    let position = session.design_document().point(point).unwrap().position;
    let snapshot = session.prepared_snapshot();
    let first = snapshot.clone().prepare(PreparedSketchOperation::Apply(
        DocumentEdit::SetPointPosition {
            point,
            position: [position[0] + 0.1, position[1]],
        },
    ));
    let second = snapshot.prepare(PreparedSketchOperation::Apply(
        DocumentEdit::SetPointPosition {
            point,
            position: [position[0] - 0.2, position[1]],
        },
    ));

    let first = completed_patch(first.execute(OperationControl::unlimited()).unwrap());
    let second = completed_patch(second.execute(OperationControl::unlimited()).unwrap());
    session.commit_prepared_patch(first).unwrap();
    let committed_design = session.design_identity();
    let committed_attempt = session.last_attempt().identity();
    let committed_position = session.design_document().point(point).unwrap().position;

    assert!(matches!(
        session.commit_prepared_patch(second),
        Err(DocumentSessionError::StalePreparedPatch { .. })
    ));
    assert_eq!(session.design_identity(), committed_design);
    assert_eq!(session.last_attempt().identity(), committed_attempt);
    assert_eq!(
        pair_bits(session.design_document().point(point).unwrap().position),
        pair_bits(committed_position)
    );
}

#[test]
fn retained_allocator_high_water_makes_an_allocating_patch_stale() {
    let (mut session, _) = fixture();
    let accepted = session
        .accepted_state_for_current_input()
        .expect("current accepted state")
        .identity();
    let job = session
        .prepared_snapshot()
        .prepare(PreparedSketchOperation::Apply(DocumentEdit::CreatePoint {
            label: "prepared allocation".into(),
            position: [7.0, 3.0],
        }));
    let captured = job.input();

    // Model an abandoned authoring branch whose identity must never be reused.
    // Retaining its cursor intentionally changes no design/attempt revision.
    let mut abandoned = session.design_document().clone();
    let retired = abandoned
        .add_point("retired branch allocation", [8.0, 3.0])
        .unwrap();
    session
        .retain_persistent_identity_high_water(&abandoned.persistent_identity_high_water())
        .unwrap();
    assert_eq!(captured.design_identity(), session.design_identity());
    assert_eq!(
        captured.latest_attempt_identity(),
        session.last_attempt().identity()
    );
    assert_ne!(captured, session.prepared_input());
    assert_eq!(
        session.persistent_identity_high_water(),
        &abandoned.persistent_identity_high_water()
    );
    assert_eq!(
        session
            .accepted_state_for_current_input()
            .expect("allocator-only retention preserves accepted provenance")
            .identity(),
        accepted
    );

    let patch = completed_patch(job.execute(OperationControl::unlimited()).unwrap());
    assert!(matches!(
        session.commit_prepared_patch(patch),
        Err(DocumentSessionError::StalePreparedPatch { .. })
    ));

    let created = session
        .apply(
            session.design_identity(),
            DocumentEdit::CreatePoint {
                label: "live allocation after rejection".into(),
                position: [9.0, 3.0],
            },
        )
        .unwrap()
        .into_value();
    let DocumentCommandEffect::CreatedPoint(created) = created else {
        panic!("created point effect expected");
    };
    assert!(created.0.as_u128() > retired.0.as_u128());
}

#[test]
fn restored_incarnations_have_distinct_prepared_cas_epochs_while_clones_share_one() {
    let (baseline, _) = fixture();
    let document = baseline.design_document().clone();
    let revisions = baseline.revision_high_water();
    let request = baseline.request();
    let left = RetainedSketchDocumentSession::restore_design(
        document.clone(),
        revisions,
        request,
        SolverConfig::default(),
    )
    .expect("left restored incarnation");
    let mut right = RetainedSketchDocumentSession::restore_design(
        document,
        revisions,
        request,
        SolverConfig::default(),
    )
    .expect("right restored incarnation");
    let mut shared_clone = left.clone();

    assert_ne!(left.prepared_input(), right.prepared_input());
    assert_eq!(left.prepared_input(), shared_clone.prepared_input());

    let snapshot = left.prepared_snapshot();
    let foreign_patch = completed_patch(
        snapshot
            .clone()
            .prepare(PreparedSketchOperation::Apply(DocumentEdit::CreatePoint {
                label: "foreign incarnation allocation".into(),
                position: [7.0, 4.0],
            }))
            .execute(OperationControl::unlimited())
            .expect("foreign patch execution"),
    );
    let clone_patch = completed_patch(
        snapshot
            .prepare(PreparedSketchOperation::Apply(DocumentEdit::CreatePoint {
                label: "shared clone allocation".into(),
                position: [8.0, 4.0],
            }))
            .execute(OperationControl::unlimited())
            .expect("clone patch execution"),
    );

    assert!(matches!(
        right.commit_prepared_patch(foreign_patch),
        Err(DocumentSessionError::StalePreparedPatch { .. })
    ));
    shared_clone
        .commit_prepared_patch(clone_patch)
        .expect("an unchanged logical clone accepts the shared-incarnation patch");
}

#[test]
fn cancelled_prepared_work_produces_no_patch_or_session_mutation() {
    let (session, _) = fixture();
    let before = session.prepared_snapshot().input();
    let job = session
        .prepared_snapshot()
        .prepare(PreparedSketchOperation::UpdateParameterBatch {
            batch: ParameterBatch::new(1, Vec::new()).unwrap(),
            request: DocumentSolveRequest::default(),
        });
    let (handle, token) = geosolve_sketch::cancellation_pair();
    handle.cancel();
    let outcome = job
        .execute(OperationControl::new(token, OperationLimits::default()))
        .unwrap();
    assert!(matches!(outcome, OperationOutcome::Cancelled { .. }));
    assert_eq!(session.prepared_snapshot().input(), before);
    assert_eq!(session.parameter_batch().revision(), 0);
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one contiguous assertion matrix proves every captured input domain"
)]
fn prepared_stamp_tracks_policy_activation_parameters_external_and_lifecycle() {
    let fixture = alpha_scenario(AlphaScenarioKind::A2, 1.0).unwrap();
    let mut document = fixture.document;
    let binding = document
        .add_external_binding("worker datum", ExternalFeatureKindV1::Point, None)
        .unwrap();
    let snapshot = |revision, source_revision, digest| {
        ExternalSnapshotSet::new(
            revision,
            vec![ExternalSnapshotEntry {
                binding,
                source_revision,
                source_digest: ExternalSnapshotDigest::from_bytes([digest; 32]),
                feature: ExternalSnapshotFeatureV1::Point {
                    position: [3.0, 4.0],
                    scale: 1.0,
                    resources: ExternalSnapshotResourcesV1 {
                        point_count: 1,
                        control_count: 0,
                        span_count: 0,
                    },
                },
            }],
        )
        .unwrap()
    };
    let mut session = RetainedSketchDocumentSession::new_with_inputs(
        document,
        ParameterBatch::default(),
        snapshot(1, 1, 11),
        fixture.request,
        SolverConfig::default(),
    )
    .unwrap();
    let initial = session.prepared_snapshot().input();
    let input = initial.attempt_input();
    assert_eq!(input.design_identity(), session.design_identity());
    assert_eq!(input.candidate_request(), session.request());
    assert_eq!(input.publication_request(), session.request());
    assert_eq!(
        input.effective_activation_revision(),
        session
            .design_document()
            .effective_activity()
            .activation_revision()
    );
    assert_eq!(
        input.activation_digest(),
        session
            .design_document()
            .effective_activity()
            .activation_digest()
    );
    assert_eq!(
        input.parameter_revision(),
        session.parameter_batch().revision()
    );
    assert_eq!(input.parameter_digest(), session.parameter_batch().digest());
    assert_eq!(
        input.external_snapshot_set_revision(),
        session.external_snapshot_set().revision()
    );
    assert_eq!(
        input.external_snapshot_set_digest(),
        session.external_snapshot_set().digest()
    );

    let parameter_patch = completed_patch(
        session
            .prepared_snapshot()
            .prepare(PreparedSketchOperation::UpdateParameterBatch {
                batch: ParameterBatch::new(1, Vec::new()).unwrap(),
                request: session.request(),
            })
            .execute(OperationControl::unlimited())
            .unwrap(),
    );
    session.commit_prepared_patch(parameter_patch).unwrap();
    let parameter_input = session.prepared_snapshot().input();
    assert_eq!(parameter_input.attempt_input().parameter_revision(), 1);
    assert_ne!(parameter_input, initial);

    let external_patch = completed_patch(
        session
            .prepared_snapshot()
            .prepare(PreparedSketchOperation::UpdateExternalSnapshotSet {
                snapshots: snapshot(2, 2, 12),
                request: session.request(),
            })
            .execute(OperationControl::unlimited())
            .unwrap(),
    );
    session.commit_prepared_patch(external_patch).unwrap();
    let external_input = session.prepared_snapshot().input();
    assert_eq!(
        external_input
            .attempt_input()
            .external_snapshot_set_revision(),
        2
    );
    assert_ne!(external_input, parameter_input);

    let reattempt_patch = completed_patch(
        session
            .prepared_snapshot()
            .prepare(PreparedSketchOperation::Reattempt {
                request: session.request(),
            })
            .execute(OperationControl::unlimited())
            .unwrap(),
    );
    session.commit_prepared_patch(reattempt_patch).unwrap();
    let reattempt_input = session.prepared_snapshot().input();
    assert_eq!(
        reattempt_input.design_identity(),
        external_input.design_identity()
    );
    assert_ne!(
        reattempt_input.latest_attempt_identity(),
        external_input.latest_attempt_identity()
    );
    assert_ne!(reattempt_input, external_input);
}
