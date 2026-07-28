// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::BTreeSet;

use geosolve_core::{
    CancellationToken, OperationControl, OperationLimits, OperationOutcome, SolverConfig,
};
use geosolve_sketch::{
    ContactBranchEdit, ContactDomain, ContactNeighborhood, CurveDefinition, CurveSpan,
    DocumentConstraintDefinition, DocumentEdit, DocumentElementId, DocumentExternalPointRef,
    DocumentParameterKind, DocumentParameterTarget, DocumentSolveRequest, ExternalFeatureKindV1,
    ExternalSnapshotDigest, ExternalSnapshotEntry, ExternalSnapshotFeatureV1,
    ExternalSnapshotResourcesV1, ExternalSnapshotSet, HostConfigurationActivation,
    MAX_DOCUMENT_JSON_BYTES, ParameterBatch, ParameterBatchEntry, ParameterValue,
    RetainedSketchDocumentSession, SketchDocument, SketchSessionExecutionKind,
    VisualProfileOptions,
};

fn two_rectangles() -> (
    SketchDocument,
    geosolve_sketch::RectangleIds,
    geosolve_sketch::RectangleIds,
) {
    let mut document = SketchDocument::new(20.0).unwrap();
    let first = document
        .add_rectangle("first", [0.0, 0.0], 4.0, 3.0)
        .unwrap();
    let second = document
        .add_rectangle("second", [10.0, 0.0], 5.0, 2.0)
        .unwrap();
    (document, first, second)
}

fn accepted(
    session: &RetainedSketchDocumentSession,
) -> &geosolve_sketch::SketchAcceptedDocumentState {
    session.accepted_state().expect("fixture must be accepted")
}

fn assert_fresh_evidence(session: &RetainedSketchDocumentSession) {
    let accepted = accepted(session);
    let summary = accepted.runtime().execution_summary();
    let scale = accepted.runtime().production_scale_assessment();
    assert!(summary.freshly_validated_hard_rows);
    assert!(summary.rank_valid);
    assert!(scale.supported);
    assert!(scale.maximum_active_rows <= scale.component_limit);
    assert!(scale.maximum_active_tangent_dimensions <= scale.component_limit);
    assert!(
        accepted
            .solve_result()
            .unstable_core_report()
            .component_solves
            .iter()
            .all(|component| component.rank_is_valid)
    );
}

#[test]
fn reverse_dependency_closure_is_canonical_and_component_local() {
    let (document, first, second) = two_rectangles();
    let closure = document.dependent_closure(first.points[1]);
    assert!(closure.contains(&DocumentElementId::Curve(first.curves[0])));
    assert!(closure.contains(&DocumentElementId::Dimension(first.dimensions[0])));
    assert!(!closure.contains(&DocumentElementId::Curve(second.curves[0])));
    assert!(!closure.contains(&DocumentElementId::Dimension(second.dimensions[0])));
    assert_eq!(closure, document.dependent_closure(first.points[1]));
    assert_eq!(
        closure.len(),
        closure.iter().copied().collect::<BTreeSet<_>>().len()
    );
}

#[test]
fn local_geometry_edit_retains_runtime_and_reuses_clean_component() {
    let (document, first, _) = two_rectangles();
    let mut session = RetainedSketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let initial_runtime_revision = accepted(&session).runtime().revision();
    let initial_topology_compilations = accepted(&session).runtime().topology_compilations();
    let position = session
        .design_document()
        .point(first.points[1])
        .unwrap()
        .position;

    let outcome = session
        .apply(
            session.design_identity(),
            DocumentEdit::SetPointPosition {
                point: first.points[1],
                position: [position[0] + 0.25, position[1] + 0.1],
            },
        )
        .unwrap();
    assert!(outcome.published_accepted_identity().is_some());
    let accepted_state = accepted(&session);
    let summary = accepted_state.runtime().execution_summary();
    assert_eq!(summary.kind, SketchSessionExecutionKind::IncrementalUpdate);
    assert_eq!(
        accepted_state.runtime().revision(),
        initial_runtime_revision + 1
    );
    assert_eq!(
        accepted_state.runtime().topology_compilations(),
        initial_topology_compilations
    );
    assert!(summary.component_count >= 2);
    assert!(summary.reused_component_count >= 1);
    assert!(summary.reused_component_count < summary.component_count);
    assert_fresh_evidence(&session);

    let fresh = RetainedSketchDocumentSession::new(
        accepted_state.document().clone(),
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    assert_eq!(accepted_state.document(), accepted(&fresh).document());
    assert_eq!(
        accepted_state.solve_result().unstable_core_report().rank,
        accepted(&fresh).solve_result().unstable_core_report().rank
    );
}

#[test]
fn parameter_update_dirties_only_its_runtime_source_component() {
    let (mut document, first, _) = two_rectangles();
    let width = document
        .add_parameter("first width", DocumentParameterKind::Length)
        .unwrap();
    document
        .add_parameter_binding(
            width,
            DocumentParameterTarget::DrivingDimension(first.dimensions[0]),
        )
        .unwrap();
    let batch = |revision, value| {
        ParameterBatch::new(
            revision,
            vec![ParameterBatchEntry {
                parameter: width,
                value: ParameterValue::Length(value),
            }],
        )
        .unwrap()
    };
    let mut session = RetainedSketchDocumentSession::new_with_parameter_batch(
        document,
        batch(1, 4.0),
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let initial_compilations = accepted(&session).runtime().topology_compilations();

    let attempt = session
        .update_parameter_batch(
            session.design_identity(),
            batch(2, 4.5),
            DocumentSolveRequest::default(),
        )
        .unwrap();
    assert!(attempt.accepted_state_identity().is_some());
    let summary = accepted(&session).runtime().execution_summary();
    assert_eq!(summary.kind, SketchSessionExecutionKind::IncrementalUpdate);
    assert_eq!(
        accepted(&session).runtime().topology_compilations(),
        initial_compilations
    );
    assert!(summary.reused_component_count >= 1);
    assert!(summary.reused_component_count < summary.component_count);
    assert_fresh_evidence(&session);

    let fresh = RetainedSketchDocumentSession::new_with_parameter_batch(
        session.design_document().clone(),
        batch(2, 4.5),
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    assert_eq!(accepted(&session).document(), accepted(&fresh).document());
}

#[test]
fn topology_change_uses_explicit_full_rebuild_path() {
    let (document, _, _) = two_rectangles();
    let mut session = RetainedSketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let outcome = session
        .apply(
            session.design_identity(),
            DocumentEdit::CreatePoint {
                label: "new free point".into(),
                position: [18.0, 7.0],
            },
        )
        .unwrap();
    assert!(outcome.published_accepted_identity().is_some());
    assert_eq!(
        accepted(&session).runtime().execution_summary().kind,
        SketchSessionExecutionKind::FullRebuild
    );
    assert_fresh_evidence(&session);
}

#[test]
fn contact_rebind_with_changed_residual_incidence_uses_full_rebuild() {
    let mut document = SketchDocument::new(4.0).unwrap();
    let points = [
        document.add_point("start", [-1.0, 0.0]).unwrap(),
        document.add_point("join", [0.0, 0.0]).unwrap(),
        document.add_point("end", [1.0, 0.0]).unwrap(),
    ];
    let point = document.add_point("contact point", [0.0, 0.0]).unwrap();
    let curve = document
        .add_curve(
            "two-span polyline",
            CurveDefinition::Polyline {
                points: points.to_vec(),
                closed: false,
                branch_directions: vec![[1.0, 0.0], [1.0, 0.0]],
            },
        )
        .unwrap();
    let first_span = CurveSpan { curve, segment: 0 };
    let second_span = CurveSpan { curve, segment: 1 };
    let contact = document
        .add_curve_contact(
            "join contact",
            first_span,
            1.0,
            0,
            ContactNeighborhood::End,
            None,
        )
        .unwrap();
    document
        .add_constraint(
            "point at join",
            DocumentConstraintDefinition::PointOnCurve { point, contact },
        )
        .unwrap();
    let mut session = RetainedSketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();

    let outcome = session
        .apply(
            session.design_identity(),
            DocumentEdit::SetContactBranches {
                edits: vec![ContactBranchEdit {
                    contact,
                    curve: second_span,
                    domain: ContactDomain::Bounded {
                        lower: 0.0,
                        upper: 1.0,
                    },
                    value: 0.0,
                    winding: 0,
                    neighborhood: ContactNeighborhood::Start,
                    tangent_orientation: None,
                }],
            },
        )
        .unwrap();
    assert!(outcome.published_accepted_identity().is_some());
    assert_eq!(
        accepted(&session).runtime().execution_summary().kind,
        SketchSessionExecutionKind::FullRebuild
    );
    assert_eq!(
        session.design_document().contact(contact).unwrap().curve,
        second_span
    );
    assert_fresh_evidence(&session);
}

#[test]
fn external_reference_update_reuses_unaffected_component() {
    let mut document = SketchDocument::new(20.0).unwrap();
    let point = document.add_point("external point", [1.0, 2.0]).unwrap();
    let binding = document
        .add_external_binding("host datum", ExternalFeatureKindV1::Point, None)
        .unwrap();
    document
        .add_constraint(
            "point on host datum",
            DocumentConstraintDefinition::ExternalPointCoincident {
                point,
                external: DocumentExternalPointRef { binding },
            },
        )
        .unwrap();
    document
        .add_rectangle("independent", [10.0, 0.0], 4.0, 3.0)
        .unwrap();
    let snapshots = |revision, position: [f64; 2]| {
        ExternalSnapshotSet::new(
            revision,
            vec![ExternalSnapshotEntry {
                binding,
                source_revision: revision,
                source_digest: ExternalSnapshotDigest::from_bytes(
                    [u8::try_from(revision).unwrap(); 32],
                ),
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
        .unwrap()
    };
    let mut session = RetainedSketchDocumentSession::new_with_inputs(
        document,
        ParameterBatch::default(),
        snapshots(1, [1.0, 2.0]),
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let attempt = session
        .update_external_snapshot_set(
            session.design_identity(),
            snapshots(2, [2.0, 3.0]),
            DocumentSolveRequest::default(),
        )
        .unwrap();
    assert!(attempt.accepted_state_identity().is_some());
    let summary = accepted(&session).runtime().execution_summary();
    assert_eq!(summary.kind, SketchSessionExecutionKind::IncrementalUpdate);
    assert!(summary.reused_component_count >= 1);
    assert!(summary.reused_component_count < summary.component_count);
    assert_fresh_evidence(&session);
}

#[test]
fn activation_revision_without_shape_change_reuses_every_component() {
    let (document, _, _) = two_rectangles();
    let mut session = RetainedSketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let outcome = session
        .apply(
            session.design_identity(),
            DocumentEdit::SetHostConfigurationActivation {
                activation: HostConfigurationActivation::new(1, Vec::new()).unwrap(),
            },
        )
        .unwrap();
    assert!(outcome.published_accepted_identity().is_some());
    let summary = accepted(&session).runtime().execution_summary();
    assert_eq!(summary.kind, SketchSessionExecutionKind::IncrementalUpdate);
    assert_eq!(summary.reused_component_count, summary.component_count);
    assert_fresh_evidence(&session);
}

#[test]
fn profile_cache_is_owned_by_one_accepted_revision() {
    let (document, first, _) = two_rectangles();
    let mut session = RetainedSketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    assert_eq!(accepted(&session).visual_profile_cache_entries(), 0);
    let first_analysis =
        accepted(&session).analyze_visual_profiles_cached(VisualProfileOptions::default());
    assert_eq!(accepted(&session).visual_profile_cache_entries(), 1);
    assert_eq!(
        first_analysis,
        accepted(&session).analyze_visual_profiles_cached(VisualProfileOptions::default())
    );
    assert_eq!(accepted(&session).visual_profile_cache_entries(), 1);

    let position = session
        .design_document()
        .point(first.points[1])
        .unwrap()
        .position;
    session
        .apply(
            session.design_identity(),
            DocumentEdit::SetPointPosition {
                point: first.points[1],
                position: [position[0] + 0.1, position[1]],
            },
        )
        .unwrap();
    assert_eq!(accepted(&session).visual_profile_cache_entries(), 0);
}

#[test]
fn medium_disconnected_workload_has_bounded_cold_warm_and_storage_evidence() {
    let mut document = SketchDocument::new(200.0).unwrap();
    let mut edited = None;
    for index in 0..16 {
        let rectangle = document
            .add_rectangle(
                &format!("cell {index}"),
                [f64::from(index) * 10.0, 0.0],
                4.0,
                3.0,
            )
            .unwrap();
        edited.get_or_insert(rectangle.points[1]);
    }
    let json = document.to_canonical_json().unwrap();
    assert!(json.len() < MAX_DOCUMENT_JSON_BYTES);
    let mut session = RetainedSketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let cold = accepted(&session).runtime().production_scale_assessment();
    assert!(cold.supported);
    assert!(cold.component_count >= 16);

    let point = edited.unwrap();
    let position = session.design_document().point(point).unwrap().position;
    session
        .apply(
            session.design_identity(),
            DocumentEdit::SetPointPosition {
                point,
                position: [position[0] + 0.2, position[1]],
            },
        )
        .unwrap();
    let warm = accepted(&session).runtime().execution_summary();
    assert_eq!(warm.kind, SketchSessionExecutionKind::IncrementalUpdate);
    assert!(warm.reused_component_count >= 15);
    assert_fresh_evidence(&session);
}

#[test]
fn exhausted_incremental_workload_publishes_no_state() {
    let (mut document, first, _) = two_rectangles();
    let width = document
        .add_parameter("first width", DocumentParameterKind::Length)
        .unwrap();
    document
        .add_parameter_binding(
            width,
            DocumentParameterTarget::DrivingDimension(first.dimensions[0]),
        )
        .unwrap();
    let batch = |revision, value| {
        ParameterBatch::new(
            revision,
            vec![ParameterBatchEntry {
                parameter: width,
                value: ParameterValue::Length(value),
            }],
        )
        .unwrap()
    };
    let mut session = RetainedSketchDocumentSession::new_with_parameter_batch(
        document,
        batch(1, 4.0),
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let before = session.prepared_snapshot().input();
    let before_document = accepted(&session).document().clone();
    let mut limits = OperationLimits::unlimited();
    limits.component_linearizations = 0;
    let outcome = session
        .update_parameter_batch_controlled(
            session.design_identity(),
            batch(2, 4.5),
            DocumentSolveRequest::default(),
            OperationControl::new(CancellationToken::default(), limits),
        )
        .unwrap();
    assert!(matches!(outcome, OperationOutcome::WorkExhausted { .. }));
    assert_eq!(session.prepared_snapshot().input(), before);
    assert_eq!(accepted(&session).document(), &before_document);
}
