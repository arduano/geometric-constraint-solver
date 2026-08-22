// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_sketch::{
    CurveDefinition, CurveSpan, DocumentConstraintDefinition, DocumentEdit,
    DocumentExternalPointRef, DocumentSessionError, DocumentSolveRequest, ExternalFeatureKindV1,
    ExternalSnapshotDigest, ExternalSnapshotEntry, ExternalSnapshotFeatureV1,
    ExternalSnapshotResourcesV1, ExternalSnapshotSet, OperationControl, OperationOutcome,
    ParameterBatch, PreparedSketchOperation, RetainedSketchDocumentSession, SketchDocument,
    SolverConfig,
};

fn fixture() -> (
    RetainedSketchDocumentSession,
    geosolve_sketch::DesignPointId,
    geosolve_sketch::DesignPointId,
    geosolve_sketch::DocumentExternalBindingId,
) {
    let mut document = SketchDocument::new(1.0).expect("document");
    let start = document.add_point("start", [0.0, 0.0]).expect("start");
    let end = document.add_point("end", [2.0, 0.0]).expect("end");
    let line = document
        .add_curve(
            "line",
            CurveDefinition::Line {
                start,
                end,
                branch_direction: [1.0, 0.0],
            },
        )
        .expect("line");
    document
        .add_constraint(
            "horizontal",
            DocumentConstraintDefinition::Horizontal {
                line: CurveSpan::line(line),
            },
        )
        .expect("horizontal");
    let binding = document
        .add_external_binding("unused host datum", ExternalFeatureKindV1::Point, None)
        .expect("external binding");
    (
        RetainedSketchDocumentSession::new(
            document,
            DocumentSolveRequest::default(),
            SolverConfig::default(),
        )
        .expect("accepted fixture"),
        start,
        end,
        binding,
    )
}

fn accepted_json(session: &RetainedSketchDocumentSession) -> String {
    session
        .accepted_state_for_current_input()
        .expect("current accepted state")
        .document()
        .to_draft_v5_json()
        .expect("canonical accepted document")
}

fn external_point_snapshots(
    revision: u64,
    binding: geosolve_sketch::DocumentExternalBindingId,
    position: [f64; 2],
) -> ExternalSnapshotSet {
    let digest_byte = u8::try_from(revision).expect("fixture revision fits one digest byte");
    ExternalSnapshotSet::new(
        revision,
        vec![ExternalSnapshotEntry {
            binding,
            source_revision: revision,
            source_digest: ExternalSnapshotDigest::from_bytes([digest_byte; 32]),
            feature: ExternalSnapshotFeatureV1::Point {
                position,
                scale: 1.0,
                resources: ExternalSnapshotResourcesV1 {
                    point_count: 1,
                    control_count: 0,
                    span_count: 0,
                },
            },
        }],
    )
    .expect("external point snapshots")
}

#[test]
#[allow(clippy::too_many_lines)]
fn rejected_external_snapshot_attempt_promotes_exact_evidence_transactionally() {
    let mut document = SketchDocument::new(1.0).expect("document");
    let point = document
        .add_point("externally owned point", [0.0, 0.0])
        .expect("point");
    let binding = document
        .add_external_binding("host point", ExternalFeatureKindV1::Point, None)
        .expect("external binding");
    document
        .add_constraint(
            "host point coincidence",
            DocumentConstraintDefinition::ExternalPointCoincident {
                point,
                external: DocumentExternalPointRef { binding },
            },
        )
        .expect("external coincidence");
    let initial_snapshots = external_point_snapshots(10, binding, [0.0, 0.0]);
    let mut session = RetainedSketchDocumentSession::new_with_inputs(
        document,
        ParameterBatch::default(),
        initial_snapshots.clone(),
        DocumentSolveRequest::default(),
        SolverConfig {
            max_iterations: 1,
            ..SolverConfig::default()
        },
    )
    .expect("exact initial acceptance");
    let design_identity = session.design_identity();
    let initial_revisions = session.revision_high_water();
    let initial_accepted = session
        .accepted_state_for_current_input()
        .expect("initial accepted authority");
    let initial_accepted_identity = initial_accepted.identity();
    let initial_accepted_json = initial_accepted
        .document()
        .to_draft_v5_json()
        .expect("initial accepted JSON");
    assert_eq!(
        session.accepted_external_snapshot_set(),
        Some(&initial_snapshots)
    );

    let attempted_snapshots = external_point_snapshots(11, binding, [100.0, 50.0]);
    session
        .update_external_snapshot_set(
            design_identity,
            attempted_snapshots.clone(),
            DocumentSolveRequest::default(),
        )
        .expect("retained rejected host-input attempt");

    let rejected_attempt = session.last_attempt().identity();
    let rejected_input = session.last_attempt().input();
    let rejected_revisions = session.revision_high_water();
    let rejected_prepared_input = session.prepared_input();
    assert_eq!(session.design_identity(), design_identity);
    assert_eq!(
        rejected_revisions.design().get(),
        initial_revisions.design().get()
    );
    assert_eq!(
        rejected_revisions.attempt().get(),
        initial_revisions.attempt().get() + 1
    );
    assert_eq!(rejected_revisions.accepted(), initial_revisions.accepted());
    assert_eq!(
        session.last_attempt().parent_accepted_identity(),
        Some(initial_accepted_identity)
    );
    assert!(
        session
            .last_attempt()
            .solve_result()
            .is_some_and(|solve| !solve.accepted())
    );
    assert!(session.last_attempt().accepted_state_identity().is_none());
    assert!(session.accepted_state_for_current_input().is_none());
    assert_eq!(session.external_snapshot_set(), &initial_snapshots);
    assert_eq!(
        session.latest_attempt_external_snapshot_set(),
        &attempted_snapshots
    );
    assert_eq!(
        session.accepted_external_snapshot_set(),
        Some(&initial_snapshots)
    );
    assert_eq!(
        rejected_input.external_snapshot_set_revision(),
        attempted_snapshots.revision()
    );
    assert_eq!(
        rejected_input.external_snapshot_set_digest(),
        attempted_snapshots.digest()
    );
    let retained_accepted = session
        .accepted_state()
        .expect("historical authority retained");
    assert_eq!(retained_accepted.identity(), initial_accepted_identity);
    assert_eq!(
        retained_accepted
            .document()
            .to_draft_v5_json()
            .expect("retained accepted JSON"),
        initial_accepted_json
    );

    let retained_design_json = session
        .design_document()
        .to_draft_v5_json()
        .expect("retained design JSON");
    let mut invalid = session.design_document().clone();
    invalid
        .set_point_position(point, [99.0, 50.0])
        .expect("finite invalid materialization");
    assert!(matches!(
        session.replace_current_accepted_materialization(rejected_prepared_input, invalid),
        Err(DocumentSessionError::InvalidAcceptedSnapshot)
    ));
    assert_eq!(session.prepared_input(), rejected_prepared_input);
    assert_eq!(session.revision_high_water(), rejected_revisions);
    assert_eq!(session.design_identity(), design_identity);
    assert_eq!(session.last_attempt().identity(), rejected_attempt);
    assert_eq!(session.last_attempt().input(), rejected_input);
    assert_eq!(session.external_snapshot_set(), &initial_snapshots);
    assert_eq!(
        session.latest_attempt_external_snapshot_set(),
        &attempted_snapshots
    );
    assert_eq!(
        session.accepted_external_snapshot_set(),
        Some(&initial_snapshots)
    );
    assert_eq!(
        session
            .design_document()
            .to_draft_v5_json()
            .expect("design after rejected promotion"),
        retained_design_json
    );
    assert_eq!(
        session
            .accepted_state()
            .expect("historical authority after rejected promotion")
            .document()
            .to_draft_v5_json()
            .expect("accepted JSON after rejected promotion"),
        initial_accepted_json
    );

    let mut canonical = session.design_document().clone();
    canonical
        .set_point_position(point, [100.0, 50.0])
        .expect("exact attempted host materialization");
    session
        .replace_current_accepted_materialization(rejected_prepared_input, canonical.clone())
        .expect("promote exact rejected-attempt evidence");

    let promoted_revisions = session.revision_high_water();
    assert_eq!(session.design_identity(), design_identity);
    assert_eq!(promoted_revisions.design(), rejected_revisions.design());
    assert_eq!(promoted_revisions.attempt(), rejected_revisions.attempt());
    assert_eq!(
        session
            .design_document()
            .to_draft_v5_json()
            .expect("retained design after promotion"),
        retained_design_json
    );
    assert_eq!(session.last_attempt().identity(), rejected_attempt);
    assert_eq!(session.last_attempt().input(), rejected_input);
    assert_eq!(
        session.last_attempt().parent_accepted_identity(),
        Some(initial_accepted_identity)
    );
    let promoted = session
        .accepted_state_for_current_input()
        .expect("promoted current authority");
    assert_eq!(promoted.document(), &canonical);
    assert_eq!(promoted.design_identity(), design_identity);
    assert_eq!(promoted.originating_attempt(), rejected_attempt);
    assert_eq!(
        promoted.identity().revision().get(),
        initial_accepted_identity.revision().get() + 1
    );
    assert_eq!(
        promoted_revisions.accepted(),
        Some(promoted.identity().revision())
    );
    assert_eq!(promoted.input(), rejected_input);
    assert_eq!(
        promoted.input().external_snapshot_set_revision(),
        attempted_snapshots.revision()
    );
    assert_eq!(
        promoted.input().external_snapshot_set_digest(),
        attempted_snapshots.digest()
    );
    assert_eq!(session.external_snapshot_set(), &attempted_snapshots);
    assert_eq!(
        session.latest_attempt_external_snapshot_set(),
        &attempted_snapshots
    );
    assert_eq!(
        session.accepted_external_snapshot_set(),
        Some(&attempted_snapshots)
    );
    assert!(promoted.solve_result().accepted());
    assert!(
        promoted
            .solve_result()
            .unstable_core_report()
            .hard_residuals_validated
    );
    assert!(
        promoted
            .solve_result()
            .acceptance_hard_residual_max
            .is_some_and(|residual| residual <= 1.0e-9)
    );
}

#[test]
#[allow(clippy::too_many_lines)]
fn changed_canonical_accepted_replacement_allocates_a_fresh_accepted_identity() {
    let (mut session, start, end, _binding) = fixture();
    let expected = session.prepared_input();
    let revisions = session.revision_high_water();
    let persistent_high_water = session.persistent_identity_high_water().clone();
    let design = session.design_document().clone();
    let attempt_identity = session.last_attempt().identity();
    let attempt_input = session.last_attempt().input();
    let parent_accepted = session.last_attempt().parent_accepted_identity();
    let continuation_parent_input = session.last_attempt().continuation_parent_input();
    let accepted_before = session
        .accepted_state_for_current_input()
        .expect("accepted fixture");
    let accepted_identity = accepted_before.identity();
    let accepted_input = accepted_before.input();
    let accepted_originating_attempt = accepted_before.originating_attempt();
    let accepted_redundancy = accepted_before.accepted_redundancy().clone();
    let accepted_parameter_outputs = accepted_before.parameter_output_proposals().to_vec();
    let accepted_parameters = session.accepted_parameter_batch().cloned();
    let accepted_snapshots = session.accepted_external_snapshot_set().cloned();
    let mut canonical = design.clone();
    canonical
        .set_point_position(start, [3.0, 2.0])
        .expect("move start");
    canonical
        .set_point_position(end, [5.0, 2.0])
        .expect("move end");

    session
        .replace_current_accepted_materialization(expected, canonical.clone())
        .expect("exact canonical replacement");

    assert_ne!(session.prepared_input(), expected);
    let replaced_revisions = session.revision_high_water();
    assert_eq!(replaced_revisions.design(), revisions.design());
    assert_eq!(replaced_revisions.attempt(), revisions.attempt());
    assert_eq!(
        replaced_revisions
            .accepted()
            .expect("replacement accepted high-water")
            .get(),
        revisions
            .accepted()
            .expect("initial accepted high-water")
            .get()
            + 1
    );
    assert_eq!(
        session.persistent_identity_high_water(),
        &persistent_high_water
    );
    assert_eq!(session.design_document(), &design);
    assert_eq!(session.last_attempt().identity(), attempt_identity);
    assert_eq!(session.last_attempt().input(), attempt_input);
    assert_eq!(
        session.last_attempt().parent_accepted_identity(),
        parent_accepted
    );
    assert_eq!(
        session.last_attempt().continuation_parent_input(),
        continuation_parent_input
    );
    let accepted = session
        .accepted_state_for_current_input()
        .expect("replacement remains current");
    assert_ne!(accepted.identity(), accepted_identity);
    assert_eq!(
        accepted.identity().revision(),
        replaced_revisions
            .accepted()
            .expect("replacement accepted revision")
    );
    assert_eq!(accepted.input(), accepted_input);
    assert_eq!(accepted.originating_attempt(), accepted_originating_attempt);
    assert_eq!(accepted.document(), &canonical);
    assert_eq!(
        accepted.accepted_redundancy().fully_redundant_sources(),
        accepted_redundancy.fully_redundant_sources()
    );
    assert_eq!(
        accepted
            .accepted_redundancy()
            .sources_containing_redundant_rows(),
        accepted_redundancy.sources_containing_redundant_rows()
    );
    assert_eq!(
        accepted.accepted_redundancy().accepted_state_identity(),
        accepted.identity()
    );
    assert_eq!(
        accepted.parameter_output_proposals(),
        accepted_parameter_outputs
    );
    assert!(accepted.solve_result().accepted());
    assert!(
        accepted
            .solve_result()
            .unstable_core_report()
            .hard_residuals_validated
    );
    assert!(
        accepted
            .solve_result()
            .acceptance_hard_residual_max
            .is_some_and(|residual| residual <= 1.0e-9)
    );
    assert_eq!(
        session.accepted_parameter_batch(),
        accepted_parameters.as_ref()
    );
    assert_eq!(
        session.accepted_external_snapshot_set(),
        accepted_snapshots.as_ref()
    );
}

#[test]
fn byte_identical_canonical_evidence_is_a_complete_identity_noop() {
    let (mut session, _start, _end, _binding) = fixture();
    let expected = session.prepared_input();
    let revisions = session.revision_high_water();
    let persistent_high_water = session.persistent_identity_high_water().clone();
    let attempt = format!("{:?}", session.last_attempt());
    let accepted = session
        .accepted_state_for_current_input()
        .expect("current accepted authority");
    let accepted_document = accepted.document().clone();
    let accepted = format!("{accepted:?}");

    session
        .replace_current_accepted_materialization(expected, accepted_document)
        .expect("identical canonical evidence");

    assert_eq!(session.prepared_input(), expected);
    assert_eq!(session.revision_high_water(), revisions);
    assert_eq!(
        session.persistent_identity_high_water(),
        &persistent_high_water
    );
    assert_eq!(format!("{:?}", session.last_attempt()), attempt);
    assert_eq!(
        format!(
            "{:?}",
            session
                .accepted_state_for_current_input()
                .expect("unchanged accepted authority")
        ),
        accepted
    );
}

#[test]
fn canonical_signed_zero_byte_change_allocates_a_fresh_accepted_identity() {
    let mut document = SketchDocument::new(1.0).expect("document");
    let point = document
        .add_point("signed zero", [0.0, 0.0])
        .expect("free point");
    let mut session = RetainedSketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .expect("accepted free point");
    let expected = session.prepared_input();
    let before = session
        .accepted_state_for_current_input()
        .expect("initial accepted authority");
    let before_identity = before.identity();
    let before_json = before
        .document()
        .to_draft_v5_json()
        .expect("initial canonical bytes");
    let mut canonical = before.document().clone();
    canonical
        .set_point_position(point, [-0.0, 0.0])
        .expect("change only the signed-zero bytes");
    let after_json = canonical
        .to_draft_v5_json()
        .expect("replacement canonical bytes");
    assert_ne!(before_json, after_json);

    session
        .replace_current_accepted_materialization(expected, canonical)
        .expect("signed-zero replacement");

    let after = session
        .accepted_state_for_current_input()
        .expect("replacement accepted authority");
    assert_ne!(after.identity(), before_identity);
    assert_eq!(
        after
            .document()
            .point(point)
            .expect("replacement point")
            .position[0]
            .to_bits(),
        (-0.0_f64).to_bits()
    );
    assert_eq!(
        after
            .document()
            .to_draft_v5_json()
            .expect("published canonical bytes"),
        after_json
    );
}

#[test]
#[allow(clippy::too_many_lines)]
fn canonical_replacement_preserves_preview_continuation_and_nondefault_host_inputs() {
    let (mut session, start, end, binding) = fixture();
    session
        .update_parameter_batch(
            session.design_identity(),
            ParameterBatch::new(7, Vec::new()).expect("nondefault parameter batch"),
            DocumentSolveRequest::default(),
        )
        .expect("accepted parameter update");
    session
        .update_external_snapshot_set(
            session.design_identity(),
            ExternalSnapshotSet::new(
                9,
                vec![ExternalSnapshotEntry {
                    binding,
                    source_revision: 9,
                    source_digest: ExternalSnapshotDigest::from_bytes([9; 32]),
                    feature: ExternalSnapshotFeatureV1::Point {
                        position: [8.0, 3.0],
                        scale: 1.0,
                        resources: ExternalSnapshotResourcesV1 {
                            point_count: 1,
                            control_count: 0,
                            span_count: 0,
                        },
                    },
                }],
            )
            .expect("nondefault snapshot set"),
            DocumentSolveRequest::default(),
        )
        .expect("accepted external update");

    let mut preview = session.clone();
    preview
        .reattempt(
            preview.design_identity(),
            DocumentSolveRequest::default()
                .without_previous_state_preferences()
                .with_drag(end, [4.0, 2.0]),
        )
        .expect("accepted point preview");
    let preview_position = preview
        .accepted_state_for_current_input()
        .expect("current preview authority")
        .document()
        .point(end)
        .expect("preview end")
        .position;
    session
        .apply_point_position_from_preview(
            session.design_identity(),
            end,
            preview_position,
            &preview,
        )
        .expect("accepted preview publication");
    assert!(session.last_attempt().continuation_parent_input().is_some());

    let expected = session.prepared_input();
    let revisions = session.revision_high_water();
    let high_water = session.persistent_identity_high_water().clone();
    let attempt_identity = session.last_attempt().identity();
    let attempt_input = session.last_attempt().input();
    let parent_accepted = session.last_attempt().parent_accepted_identity();
    let continuation_parent_input = session
        .last_attempt()
        .continuation_parent_input()
        .expect("nonempty continuation provenance");
    let accepted_identity = session
        .accepted_state_for_current_input()
        .expect("current accepted preview publication")
        .identity();
    let parameters = session.parameter_batch().clone();
    let snapshots = session.external_snapshot_set().clone();
    assert_eq!(parameters.revision(), 7);
    assert_eq!(snapshots.revision(), 9);

    let mut canonical = session
        .accepted_state_for_current_input()
        .expect("accepted graph")
        .document()
        .clone();
    for point in [start, end] {
        let position = canonical.point(point).expect("line endpoint").position;
        canonical
            .set_point_position(point, [position[0] + 1.0, position[1] + 1.0])
            .expect("translate accepted endpoint");
    }
    session
        .replace_current_accepted_materialization(expected, canonical.clone())
        .expect("replace preview-authored accepted graph");

    let replaced_revisions = session.revision_high_water();
    assert_eq!(replaced_revisions.design(), revisions.design());
    assert_eq!(replaced_revisions.attempt(), revisions.attempt());
    assert_eq!(
        replaced_revisions
            .accepted()
            .expect("replacement accepted revision")
            .get(),
        revisions
            .accepted()
            .expect("preview accepted revision")
            .get()
            + 1
    );
    assert_eq!(session.persistent_identity_high_water(), &high_water);
    assert_eq!(session.last_attempt().identity(), attempt_identity);
    assert_eq!(session.last_attempt().input(), attempt_input);
    assert_eq!(
        session.last_attempt().parent_accepted_identity(),
        parent_accepted
    );
    assert_eq!(
        session.last_attempt().continuation_parent_input(),
        Some(continuation_parent_input)
    );
    let accepted = session
        .accepted_state_for_current_input()
        .expect("replacement remains current");
    assert_ne!(accepted.identity(), accepted_identity);
    assert_eq!(
        accepted.identity().revision(),
        replaced_revisions
            .accepted()
            .expect("replacement accepted revision")
    );
    assert_eq!(accepted.document(), &canonical);
    assert_eq!(session.parameter_batch(), &parameters);
    assert_eq!(session.external_snapshot_set(), &snapshots);
    assert_eq!(session.accepted_parameter_batch(), Some(&parameters));
    assert_eq!(session.accepted_external_snapshot_set(), Some(&snapshots));
}

#[test]
fn canonical_replacement_invalidates_outstanding_prepared_work() {
    let (mut session, start, end, _binding) = fixture();
    let expected = session.prepared_input();
    let patch = match session
        .prepared_snapshot()
        .prepare(PreparedSketchOperation::Apply(
            DocumentEdit::SetPointPosition {
                point: end,
                position: [4.0, 0.0],
            },
        ))
        .execute(OperationControl::unlimited())
        .expect("prepared job")
    {
        OperationOutcome::Completed { value, .. } => value,
        stopped => panic!(
            "prepared job stopped: {:?}",
            stopped.report().stopping_reason
        ),
    };
    let revisions = session.revision_high_water();
    let accepted_identity = session
        .accepted_state_for_current_input()
        .expect("accepted fixture")
        .identity();
    let mut canonical = session.design_document().clone();
    canonical
        .set_point_position(start, [3.0, 2.0])
        .expect("move start");
    canonical
        .set_point_position(end, [5.0, 2.0])
        .expect("move end");

    session
        .replace_current_accepted_materialization(expected, canonical.clone())
        .expect("canonical replacement");

    let replaced_revisions = session.revision_high_water();
    assert_eq!(replaced_revisions.design(), revisions.design());
    assert_eq!(replaced_revisions.attempt(), revisions.attempt());
    assert_eq!(
        replaced_revisions
            .accepted()
            .expect("replacement accepted revision")
            .get(),
        revisions
            .accepted()
            .expect("initial accepted revision")
            .get()
            + 1
    );
    let replaced_identity = session
        .accepted_state_for_current_input()
        .expect("canonical current state")
        .identity();
    assert_ne!(replaced_identity, accepted_identity);
    assert_eq!(
        replaced_identity.revision(),
        replaced_revisions
            .accepted()
            .expect("replacement accepted revision")
    );
    assert!(matches!(
        session.commit_prepared_patch(patch),
        Err(DocumentSessionError::StalePreparedPatch { .. })
    ));
    assert_eq!(
        session
            .accepted_state_for_current_input()
            .expect("canonical state retained after stale patch")
            .document(),
        &canonical
    );
}

#[test]
fn stale_canonical_replacement_rejects_without_mutation() {
    let (mut session, _start, _end, _binding) = fixture();
    let stale = session.prepared_input();
    let design = session.design_identity();
    session
        .reattempt(design, DocumentSolveRequest::default())
        .expect("new attempt");
    let before_input = session.prepared_input();
    let before_revisions = session.revision_high_water();
    let before_accepted = accepted_json(&session);
    let candidate = session
        .accepted_state_for_current_input()
        .expect("accepted reattempt")
        .document()
        .clone();

    assert!(matches!(
        session.replace_current_accepted_materialization(stale, candidate),
        Err(DocumentSessionError::StalePreparedPatch { .. })
    ));
    assert_eq!(session.prepared_input(), before_input);
    assert_eq!(session.revision_high_water(), before_revisions);
    assert_eq!(accepted_json(&session), before_accepted);
}

#[test]
fn invalid_canonical_replacement_rejects_without_mutation() {
    let (mut session, start, _end, _binding) = fixture();
    let expected = session.prepared_input();
    let before_revisions = session.revision_high_water();
    let before_accepted = accepted_json(&session);
    let mut invalid = session.design_document().clone();
    invalid
        .set_point_position(start, [0.0, 1.0])
        .expect("finite but constraint-invalid candidate");

    assert!(matches!(
        session.replace_current_accepted_materialization(expected, invalid),
        Err(DocumentSessionError::InvalidAcceptedSnapshot)
    ));
    assert_eq!(session.prepared_input(), expected);
    assert_eq!(session.revision_high_water(), before_revisions);
    assert_eq!(accepted_json(&session), before_accepted);
}

#[test]
fn foreign_canonical_replacement_rejects_without_mutation() {
    let (mut session, _start, _end, _binding) = fixture();
    let expected = session.prepared_input();
    let before_revisions = session.revision_high_water();
    let before_high_water = session.persistent_identity_high_water().clone();
    let before_design = session
        .design_document()
        .to_draft_v5_json()
        .expect("retained design");
    let before_accepted = accepted_json(&session);
    let foreign = SketchDocument::new(1.0).expect("foreign document");

    assert!(matches!(
        session.replace_current_accepted_materialization(expected, foreign),
        Err(DocumentSessionError::ForeignDesign { .. })
    ));
    assert_eq!(session.prepared_input(), expected);
    assert_eq!(session.revision_high_water(), before_revisions);
    assert_eq!(session.persistent_identity_high_water(), &before_high_water);
    assert_eq!(
        session
            .design_document()
            .to_draft_v5_json()
            .expect("retained design after rejection"),
        before_design
    );
    assert_eq!(accepted_json(&session), before_accepted);
}

#[test]
fn topology_and_activation_incompatible_replacements_are_atomic() {
    let (mut session, _start, _end, _binding) = fixture();

    for candidate in {
        let accepted = session
            .accepted_state_for_current_input()
            .expect("accepted fixture")
            .document();
        let mut topology = accepted.clone();
        topology
            .add_point("foreign topology", [4.0, 5.0])
            .expect("structurally valid extra point");
        let mut activation = accepted.clone();
        let source = activation.source_order()[0];
        activation
            .set_source_suppressed(source, true)
            .expect("structurally valid activation change");
        [topology, activation]
    } {
        let expected = session.prepared_input();
        let before_revisions = session.revision_high_water();
        let before_high_water = session.persistent_identity_high_water().clone();
        let before_accepted = accepted_json(&session);

        assert!(matches!(
            session.replace_current_accepted_materialization(expected, candidate),
            Err(DocumentSessionError::InvalidAcceptedSnapshot)
        ));
        assert_eq!(session.prepared_input(), expected);
        assert_eq!(session.revision_high_water(), before_revisions);
        assert_eq!(session.persistent_identity_high_water(), &before_high_water);
        assert_eq!(accepted_json(&session), before_accepted);
    }
}

#[test]
fn older_design_replacement_rejects_without_mutation() {
    let (mut session, start, _end, _binding) = fixture();
    let stale = session.prepared_input();
    let stale_candidate = session
        .accepted_state_for_current_input()
        .expect("accepted fixture")
        .document()
        .clone();
    session
        .apply(
            session.design_identity(),
            DocumentEdit::SetPointPosition {
                point: start,
                position: [3.0, 2.0],
            },
        )
        .expect("newer retained design");
    let current = session.prepared_input();
    let before_revisions = session.revision_high_water();
    let before_accepted = accepted_json(&session);

    assert!(matches!(
        session.replace_current_accepted_materialization(stale, stale_candidate),
        Err(DocumentSessionError::StalePreparedPatch { .. })
    ));
    assert_eq!(session.prepared_input(), current);
    assert_eq!(session.revision_high_water(), before_revisions);
    assert_eq!(accepted_json(&session), before_accepted);
}
