// SPDX-License-Identifier: GPL-3.0-or-later

//! Exact M83 regression for non-monotonic host-input authority across strict
//! chronological lineage prefixes.

use geosolve_constraint_editor::{
    RetainedEditorCoordinator, evaluate_lineage_session_cold_with_inputs,
};
use geosolve_sketch::{
    DocumentConstraintDefinition, DocumentEdit, DocumentExternalPointRef, DocumentObjectId,
    DocumentParameterKind, DocumentParameterTarget, DocumentSolveRequest, ExternalFeatureKindV1,
    ExternalSnapshotDigest, ExternalSnapshotEntry, ExternalSnapshotFeatureV1,
    ExternalSnapshotResourcesV1, ExternalSnapshotSet, ParameterBatch, ParameterBatchEntry,
    ParameterValue, RetainedSketchDocumentSession, SketchDocument, SolverConfig,
};
use geosolve_sketch_lineage::LineageSession;

#[test]
fn m83_f002_strict_prefix_retains_input_removed_by_a_later_binding_edit() {
    let mut document = SketchDocument::new(8.0).expect("document");
    let rectangle = document
        .add_rectangle("parameterized rectangle", [0.0, 0.0], 4.0, 3.0)
        .expect("rectangle");
    let parameter = document
        .add_parameter("host parameter", DocumentParameterKind::Length)
        .expect("parameter");
    let target = DocumentParameterTarget::DrivingDimension(rectangle.dimensions[0]);
    document
        .add_parameter_binding(parameter, target)
        .expect("parameter binding");
    let historical = ParameterBatch::new(
        1,
        vec![ParameterBatchEntry {
            parameter,
            value: ParameterValue::Length(4.0),
        }],
    )
    .expect("historical host input");
    let retained = RetainedSketchDocumentSession::new_with_parameter_batch(
        document,
        historical,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .expect("accepted parameterized sketch");
    let mut coordinator = RetainedEditorCoordinator::new(retained).expect("coordinator");

    let current = ParameterBatch::new(2, Vec::new()).expect("empty current input");
    let rejected = coordinator
        .replace_parameter_batch(
            coordinator.session().design_identity(),
            current.clone(),
            DocumentSolveRequest::default(),
        )
        .expect("typed missing-input attempt");
    assert!(rejected.published_accepted.is_none());
    coordinator
        .apply_edit(
            coordinator.session().design_identity(),
            DocumentEdit::RemoveParameterBinding { parameter, target },
        )
        .expect("binding removal under the new input");
    assert!(
        coordinator
            .session()
            .accepted_state_for_current_input()
            .is_some()
    );

    let lineage = LineageSession::from_session_json(
        &coordinator
            .lineage_session_json()
            .expect("canonical lineage session"),
    )
    .expect("lineage session");
    let evidence = evaluate_lineage_session_cold_with_inputs(
        &lineage,
        &current,
        &geosolve_sketch::ExternalSnapshotSet::default(),
    )
    .expect("every strict prefix must use its exact historical input provenance");
    assert_eq!(evidence.validated_prefix_count(), 2);
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "the historical-input regression keeps both external snapshots, the later removal, and strict-prefix evidence in one scenario"
)]
fn strict_prefix_retains_external_snapshot_removed_by_a_later_constraint_edit() {
    let mut document = SketchDocument::new(8.0).expect("document");
    let first_point = document
        .add_point("first local point", [2.0, 3.0])
        .expect("first local point");
    let second_point = document
        .add_point("second local point", [6.0, -1.0])
        .expect("second local point");
    let first_binding = document
        .add_external_binding("first host point", ExternalFeatureKindV1::Point, None)
        .expect("first external binding");
    let second_binding = document
        .add_external_binding("second host point", ExternalFeatureKindV1::Point, None)
        .expect("second external binding");
    let first_constraint = document
        .add_constraint(
            "first external coincidence",
            DocumentConstraintDefinition::ExternalPointCoincident {
                point: first_point,
                external: DocumentExternalPointRef {
                    binding: first_binding,
                },
            },
        )
        .expect("first external constraint");
    document
        .add_constraint(
            "second external coincidence",
            DocumentConstraintDefinition::ExternalPointCoincident {
                point: second_point,
                external: DocumentExternalPointRef {
                    binding: second_binding,
                },
            },
        )
        .expect("second external constraint");
    let snapshot_entry = |binding, position, digest| ExternalSnapshotEntry {
        binding,
        source_revision: 1,
        source_digest: ExternalSnapshotDigest::from_bytes([digest; 32]),
        feature: ExternalSnapshotFeatureV1::Point {
            position,
            scale: 1.0,
            resources: ExternalSnapshotResourcesV1 {
                point_count: 1,
                control_count: 0,
                span_count: 0,
            },
        },
    };
    let historical = ExternalSnapshotSet::new(
        1,
        vec![
            snapshot_entry(first_binding, [2.0, 3.0], 0x83),
            snapshot_entry(second_binding, [6.0, -1.0], 0x84),
        ],
    )
    .expect("historical external snapshot");
    let retained = RetainedSketchDocumentSession::new_with_inputs(
        document,
        ParameterBatch::default(),
        historical,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .expect("accepted externally constrained sketch");
    let mut coordinator = RetainedEditorCoordinator::new(retained).expect("coordinator");

    let current =
        ExternalSnapshotSet::new(2, vec![snapshot_entry(second_binding, [6.0, -1.0], 0x85)])
            .expect("newer snapshot without the first binding");
    let rejected = coordinator
        .replace_external_snapshot_set(
            coordinator.session().design_identity(),
            current.clone(),
            DocumentSolveRequest::default(),
        )
        .expect("typed missing-snapshot attempt");
    assert!(rejected.published_accepted.is_none());
    coordinator
        .apply_edit(
            coordinator.session().design_identity(),
            DocumentEdit::Delete {
                object: DocumentObjectId::Constraint(first_constraint),
            },
        )
        .expect("constraint removal under the new input");
    assert!(
        coordinator
            .session()
            .accepted_state_for_current_input()
            .is_some()
    );

    let lineage = LineageSession::from_session_json(
        &coordinator
            .lineage_session_json()
            .expect("canonical lineage session"),
    )
    .expect("lineage session");
    let evidence =
        evaluate_lineage_session_cold_with_inputs(&lineage, &ParameterBatch::default(), &current)
            .expect("strict prefixes must use their exact historical snapshot provenance");
    assert_eq!(evidence.validated_prefix_count(), 2);
}
