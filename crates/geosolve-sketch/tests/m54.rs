// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_core::{DiagnosticBudget, SolverConfig};
use geosolve_sketch::{
    AlphaScenarioIds, AlphaScenarioKind, CurveDefinition, CurveSpan, DocumentConstraintDefinition,
    DocumentDimensionDefinition, DocumentDimensionMode, DocumentElementId,
    DocumentExternalLineSupportRef, DocumentLineSupportRef, DocumentParameterKind,
    DocumentParameterTarget, DocumentSolveRequest, ExternalFeatureKindV1,
    ExternalLineOrientationV1, ExternalSnapshotDigest, ExternalSnapshotEntry,
    ExternalSnapshotFeatureV1, ExternalSnapshotResourcesV1, ExternalSnapshotSet,
    ExternalTopologyDigest, InactivityReason, ParameterBatch, ParameterBatchEntry, ParameterValue,
    RetainedSketchDocumentSession, ScalarDomain, ScalarUnit, SketchDiagnosticIncompleteReason,
    SketchDiagnosticProvenance, SketchDiagnosticSearchStatus, SketchExternalReferenceState,
    SketchOneSidedMobility, SketchParameterInputIssue, SketchParameterState,
    SketchRepairSuggestion, SketchStructuralClassification, alpha_scenario,
};

fn retained_scenario(kind: AlphaScenarioKind) -> (RetainedSketchDocumentSession, AlphaScenarioIds) {
    let fixture = alpha_scenario(kind, 1.0).unwrap();
    let session = RetainedSketchDocumentSession::new(
        fixture.document,
        fixture.request,
        SolverConfig::default(),
    )
    .unwrap();
    (session, fixture.ids)
}

fn conflicting_dimensions_document() -> (
    geosolve_sketch::SketchDocument,
    geosolve_sketch::DocumentSourceId,
    geosolve_sketch::DocumentSourceId,
    geosolve_sketch::DesignPointId,
) {
    let mut document = geosolve_sketch::SketchDocument::new(10.0).unwrap();
    let rectangle = document
        .add_rectangle("rectangle", [0.0, 0.0], 4.0, 3.0)
        .unwrap();
    let original = document
        .dimension(rectangle.dimensions[0])
        .unwrap()
        .source_id;
    let target = document
        .add_scalar(
            "conflicting width",
            5.0,
            ScalarUnit::Length,
            ScalarDomain::Positive,
        )
        .unwrap();
    let duplicate = document
        .add_dimension(
            "second width",
            DocumentDimensionDefinition::CurveLength {
                curve: CurveSpan::line(rectangle.curves[0]),
                target,
            },
            DocumentDimensionMode::Driving,
        )
        .unwrap();
    let conflicting = document.dimension(duplicate).unwrap().source_id;
    let isolated = document.add_point("isolated", [8.0, 8.0]).unwrap();
    (document, original, conflicting, isolated)
}

fn parameter_document() -> (
    geosolve_sketch::SketchDocument,
    geosolve_sketch::DocumentParameterId,
) {
    let mut document = geosolve_sketch::SketchDocument::new(10.0).unwrap();
    let rectangle = document
        .add_rectangle("rectangle", [0.0, 0.0], 4.0, 3.0)
        .unwrap();
    let parameter = document
        .add_parameter("width", DocumentParameterKind::Length)
        .unwrap();
    document
        .add_parameter_binding(
            parameter,
            DocumentParameterTarget::DrivingDimension(rectangle.dimensions[0]),
        )
        .unwrap();
    (document, parameter)
}

fn external_line_document() -> (
    geosolve_sketch::SketchDocument,
    geosolve_sketch::DocumentExternalBindingId,
    ExternalTopologyDigest,
) {
    let topology = ExternalTopologyDigest::from_bytes([0x54; 32]);
    let mut document = geosolve_sketch::SketchDocument::new(10.0).unwrap();
    let first = document.add_point("first", [0.0, 0.0]).unwrap();
    let second = document.add_point("second", [2.0, 0.0]).unwrap();
    let line = document
        .add_curve(
            "line",
            CurveDefinition::Line {
                start: first,
                end: second,
                branch_direction: [1.0, 0.0],
            },
        )
        .unwrap();
    let binding = document
        .add_external_binding(
            "external line",
            ExternalFeatureKindV1::LineSegment,
            Some(topology),
        )
        .unwrap();
    document
        .add_constraint(
            "collinear",
            DocumentConstraintDefinition::ExternalLineCollinear {
                line: DocumentLineSupportRef {
                    span: CurveSpan::line(line),
                    direction: geosolve_sketch::DocumentDirectionSense::Forward,
                },
                external: DocumentExternalLineSupportRef {
                    binding,
                    direction: geosolve_sketch::DocumentDirectionSense::Forward,
                },
            },
        )
        .unwrap();
    (document, binding, topology)
}

fn line_snapshot(
    revision: u64,
    binding: geosolve_sketch::DocumentExternalBindingId,
    topology: ExternalTopologyDigest,
) -> ExternalSnapshotSet {
    ExternalSnapshotSet::new(
        revision,
        vec![ExternalSnapshotEntry {
            binding,
            source_revision: 3,
            source_digest: ExternalSnapshotDigest::from_bytes([0x33; 32]),
            feature: ExternalSnapshotFeatureV1::LineSegment {
                start: [0.0, 0.0],
                end: [2.0, 0.0],
                domain: [0.0, 1.0],
                orientation: ExternalLineOrientationV1::StartToEnd,
                scale: 10.0,
                topology_digest: topology,
                resources: ExternalSnapshotResourcesV1 {
                    point_count: 2,
                    control_count: 0,
                    span_count: 1,
                },
            },
        }],
    )
    .unwrap()
}

#[test]
fn accepted_snapshot_has_exact_provenance_and_separate_rank_evidence() {
    let (session, ids) = retained_scenario(AlphaScenarioKind::DiagnosticRankDrop);
    let AlphaScenarioIds::DiagnosticRankDrop(ids) = ids else {
        panic!("rank-drop IDs expected");
    };
    let accepted = session.accepted_state().unwrap();
    let diagnostics = accepted.diagnostics();
    assert_eq!(
        diagnostics.provenance,
        SketchDiagnosticProvenance::Accepted {
            accepted: accepted.identity(),
            originating_attempt: accepted.originating_attempt(),
            design: accepted.design_identity(),
        }
    );
    assert_eq!(diagnostics.input.design, accepted.design_identity());
    let rank = diagnostics.rank.unwrap();
    assert_eq!(rank.numerical_left_nullity, Some(1));
    assert_eq!(rank.numerical_right_nullity, Some(1));
    assert_eq!(rank.structural_left_nullity, 0);
    assert_eq!(rank.structural_right_nullity, 0);
    assert_eq!(
        rank.structural_classification,
        SketchStructuralClassification::Well
    );
    assert!(diagnostics.components.iter().any(|component| {
        component
            .identity
            .elements
            .contains(&DocumentElementId::Point(ids.point))
    }));
}

#[test]
fn endpoint_bounds_keep_equality_bounded_and_one_sided_mobility_distinct() {
    let (session, ids) = retained_scenario(AlphaScenarioKind::DiagnosticEndpointBound);
    let AlphaScenarioIds::DiagnosticEndpointBound(ids) = ids else {
        panic!("endpoint-bound IDs expected");
    };
    let diagnostics = session.accepted_diagnostics().unwrap();
    let mobility = diagnostics.mobility.unwrap();
    assert_eq!(mobility.equality_degrees_of_freedom, Some(2));
    assert_eq!(mobility.bidirectional_bounded_degrees_of_freedom, Some(0));
    assert_eq!(mobility.one_sided, SketchOneSidedMobility::Exists);
    assert!(
        diagnostics
            .bounds
            .iter()
            .any(|bound| bound.target == DocumentElementId::Curve(ids.circle))
    );
    assert!(diagnostics.components.iter().any(|component| {
        component
            .identity
            .elements
            .contains(&DocumentElementId::Contact(ids.contact))
    }));
}

#[test]
fn complete_redundancy_and_conflict_repairs_use_persistent_sources() {
    let (redundant, ids) = retained_scenario(AlphaScenarioKind::DiagnosticRedundancy);
    let AlphaScenarioIds::DiagnosticRedundancy(ids) = ids else {
        panic!("redundancy IDs expected");
    };
    let duplicate_source = redundant
        .design_document()
        .dimension(ids.duplicate_length)
        .unwrap()
        .source_id;
    let redundancy = redundant.accepted_diagnostics().unwrap();
    assert_eq!(
        redundancy.redundancy.status,
        SketchDiagnosticSearchStatus::Complete
    );
    assert!(redundancy.redundancy.candidates.contains(&duplicate_source));
    assert!(
        redundancy
            .sources
            .iter()
            .any(|source| source.source == duplicate_source && source.contains_redundant_rows)
    );

    let (document, first, second, isolated) = conflicting_dimensions_document();
    let before = document.clone();
    let session = RetainedSketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let identity = session.design_identity();
    let diagnostics = session.latest_attempt_diagnostics();
    assert_eq!(
        diagnostics.conflicts.status,
        SketchDiagnosticSearchStatus::Complete
    );
    assert!(diagnostics.conflicts.candidates.contains(&first));
    assert!(diagnostics.conflicts.candidates.contains(&second));
    for source in [first, second] {
        assert!(
            diagnostics
                .repair_suggestions
                .contains(&SketchRepairSuggestion::ReviewOrSuppressSource(source))
        );
    }
    assert!(diagnostics.components.iter().any(|component| {
        component
            .identity
            .elements
            .contains(&DocumentElementId::Point(isolated))
    }));
    assert_eq!(session.design_identity(), identity);
    assert_eq!(session.design_document(), &before);
    assert!(session.accepted_state().is_none());
}

#[test]
fn incomplete_empty_searches_remain_explicitly_incomplete() {
    let (document, _, _, _) = conflicting_dimensions_document();
    let disabled = DiagnosticBudget {
        enabled: false,
        max_component_tangent_dimension: 0,
        max_component_scalar_rows: 0,
        max_candidate_sources: 0,
        max_trials: 0,
    };
    let config = SolverConfig {
        conflict_diagnostic_budget: disabled,
        redundancy_diagnostic_budget: disabled,
        ..SolverConfig::default()
    };
    let session =
        RetainedSketchDocumentSession::new(document, DocumentSolveRequest::default(), config)
            .unwrap();
    let diagnostics = session.latest_attempt_diagnostics();
    for search in [&diagnostics.conflicts, &diagnostics.redundancy] {
        assert_eq!(search.status, SketchDiagnosticSearchStatus::Skipped);
        assert_eq!(
            search.reason,
            Some(SketchDiagnosticIncompleteReason::Disabled)
        );
        assert!(search.candidates.is_empty());
        assert!(!search.budget.enabled);
    }
}

#[test]
fn parameter_failures_target_persistent_parameter_ids() {
    let (document, parameter) = parameter_document();
    let missing = RetainedSketchDocumentSession::new_with_inputs(
        document.clone(),
        ParameterBatch::default(),
        ExternalSnapshotSet::default(),
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let missing_diagnostics = missing.latest_attempt_diagnostics();
    assert_eq!(
        missing
            .last_attempt()
            .failure()
            .unwrap()
            .parameter_input_issue(),
        Some(SketchParameterInputIssue::Missing(parameter))
    );
    assert!(missing_diagnostics.parameters.iter().any(|diagnostic| {
        diagnostic.parameter == parameter && diagnostic.state == SketchParameterState::Missing
    }));
    assert!(
        missing_diagnostics
            .repair_suggestions
            .contains(&SketchRepairSuggestion::SupplyParameter(parameter))
    );

    let wrong_kind = ParameterBatch::new(
        1,
        vec![ParameterBatchEntry {
            parameter,
            value: ParameterValue::Angle(1.0),
        }],
    )
    .unwrap();
    let wrong = RetainedSketchDocumentSession::new_with_inputs(
        document,
        wrong_kind,
        ExternalSnapshotSet::default(),
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let diagnostics = wrong.latest_attempt_diagnostics();
    assert!(diagnostics.parameters.iter().any(|diagnostic| {
        diagnostic.parameter == parameter && diagnostic.state == SketchParameterState::WrongKind
    }));
    assert!(
        diagnostics
            .repair_suggestions
            .contains(&SketchRepairSuggestion::CorrectParameterKind(parameter))
    );
}

#[test]
fn external_failures_target_persistent_binding_ids_and_topology() {
    let (document, binding, topology) = external_line_document();
    let missing = RetainedSketchDocumentSession::new_with_inputs(
        document.clone(),
        ParameterBatch::default(),
        ExternalSnapshotSet::default(),
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let missing_diagnostics = missing.latest_attempt_diagnostics();
    assert!(
        missing_diagnostics
            .external_references
            .iter()
            .any(|diagnostic| {
                diagnostic.binding == binding
                    && diagnostic.state == SketchExternalReferenceState::Missing
            })
    );
    assert!(
        missing_diagnostics
            .repair_suggestions
            .contains(&SketchRepairSuggestion::SupplyExternalSnapshot(binding))
    );

    let topology_mismatch = RetainedSketchDocumentSession::new_with_inputs(
        document,
        ParameterBatch::default(),
        line_snapshot(1, binding, ExternalTopologyDigest::from_bytes([0x55; 32])),
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let diagnostics = topology_mismatch.latest_attempt_diagnostics();
    assert!(diagnostics.external_references.iter().any(|diagnostic| {
        diagnostic.binding == binding
            && diagnostic.state == SketchExternalReferenceState::TopologyMismatch
    }));
    assert!(
        diagnostics
            .repair_suggestions
            .contains(&SketchRepairSuggestion::RebindExternalTopology(binding))
    );
    assert_ne!(topology, ExternalTopologyDigest::from_bytes([0x55; 32]));
}

#[test]
fn activation_dependency_evidence_has_the_exact_stamp() {
    let mut document = geosolve_sketch::SketchDocument::new(10.0).unwrap();
    let first = document.add_point("first", [0.0, 0.0]).unwrap();
    let second = document.add_point("second", [2.0, 0.0]).unwrap();
    let line = document
        .add_curve(
            "line",
            CurveDefinition::Line {
                start: first,
                end: second,
                branch_direction: [1.0, 0.0],
            },
        )
        .unwrap();
    document
        .set_element_user_suppressed(DocumentElementId::Point(first), true)
        .unwrap();
    let activity = document.effective_activity();
    let session = RetainedSketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let diagnostics = session.accepted_diagnostics().unwrap();
    assert_eq!(
        diagnostics.activation_revision,
        activity.activation_revision()
    );
    assert_eq!(diagnostics.activation_digest, activity.activation_digest());
    assert_eq!(
        diagnostics.input.activation_digest,
        activity.activation_digest()
    );
    let line_evidence = diagnostics
        .dependencies
        .iter()
        .find(|entry| entry.element == DocumentElementId::Curve(line))
        .unwrap();
    assert_eq!(
        line_evidence.inactivity,
        Some(InactivityReason::UnavailableDependency {
            dependency: DocumentElementId::Point(first),
        })
    );
    assert!(
        line_evidence
            .dependencies
            .contains(&DocumentElementId::Point(first))
    );
}

#[test]
fn runtime_remapping_preserves_stable_diagnostic_identity() {
    let fixture = alpha_scenario(AlphaScenarioKind::DiagnosticEndpointBound, 1.0).unwrap();
    let json = fixture.document.to_canonical_json().unwrap();
    let restored = geosolve_sketch::SketchDocument::from_json(&json).unwrap();
    let first = RetainedSketchDocumentSession::new(
        fixture.document,
        fixture.request,
        SolverConfig::default(),
    )
    .unwrap();
    let second =
        RetainedSketchDocumentSession::new(restored, fixture.request, SolverConfig::default())
            .unwrap();
    let first = first.accepted_diagnostics().unwrap();
    let second = second.accepted_diagnostics().unwrap();
    assert_eq!(first, second);
    assert!(
        first
            .components
            .iter()
            .flat_map(|component| &component.identity.elements)
            .any(|element| matches!(
                element,
                DocumentElementId::Point(_)
                    | DocumentElementId::Curve(_)
                    | DocumentElementId::Contact(_)
            ))
    );
}

#[test]
fn raw_report_is_only_an_explicit_unstable_compatibility_seam() {
    let (session, _) = retained_scenario(AlphaScenarioKind::DiagnosticRankDrop);
    let accepted = session.accepted_state().unwrap();
    assert!(accepted.solve_result().unstable_core_report().rank_is_valid);
    assert_eq!(
        accepted.solve_result().unstable_core_report().rank,
        accepted.diagnostics().rank.unwrap().numerical_rank.unwrap()
    );
}
