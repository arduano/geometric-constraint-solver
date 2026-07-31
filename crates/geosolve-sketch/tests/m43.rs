// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_core::{OperationControl, OperationOutcome, SolverConfig};
use geosolve_geometry::Point2;
use geosolve_sketch::{
    CurveDefinition, DocumentConstraintDefinition, DocumentDirectionSense, DocumentError,
    DocumentExternalLineSupportRef, DocumentExternalPointRef, DocumentLineSupportRef,
    DocumentParameterKind, DocumentParameterTarget, DocumentSessionError, DocumentSolveRequest,
    ExternalFeatureKindV1, ExternalLineOrientationV1, ExternalSnapshotDigest,
    ExternalSnapshotEntry, ExternalSnapshotFeatureV1, ExternalSnapshotInputError,
    ExternalSnapshotResourcesV1, ExternalSnapshotSetDigest, ExternalSnapshotSetV1,
    ExternalTopologyDigest, InactivityReason, MAX_EXTERNAL_SNAPSHOT_ENTRIES, ParameterBatch,
    ParameterBatchEntry, ParameterValue, RetainedSketchDocumentSession, Sketch,
    SketchAttemptFailureKind, SketchDocument, SketchSolveRequest,
};

fn resources(point_count: u32, control_count: u32, span_count: u32) -> ExternalSnapshotResourcesV1 {
    ExternalSnapshotResourcesV1 {
        point_count,
        control_count,
        span_count,
    }
}

fn point_entry(
    binding: geosolve_sketch::DocumentExternalBindingId,
    position: [f64; 2],
) -> ExternalSnapshotEntry {
    ExternalSnapshotEntry {
        binding,
        source_revision: 1,
        source_digest: ExternalSnapshotDigest::from_bytes([11; 32]),
        feature: ExternalSnapshotFeatureV1::Point {
            position,
            scale: 1.0,
            resources: resources(1, 0, 0),
        },
    }
}

fn line_entry(
    binding: geosolve_sketch::DocumentExternalBindingId,
    topology: ExternalTopologyDigest,
    start: [f64; 2],
    end: [f64; 2],
) -> ExternalSnapshotEntry {
    ExternalSnapshotEntry {
        binding,
        source_revision: 1,
        source_digest: ExternalSnapshotDigest::from_bytes([12; 32]),
        feature: ExternalSnapshotFeatureV1::LineSegment {
            start,
            end,
            domain: [0.0, 1.0],
            orientation: ExternalLineOrientationV1::StartToEnd,
            scale: 1.0,
            topology_digest: topology,
            resources: resources(2, 0, 1),
        },
    }
}

fn external_point_document() -> (
    SketchDocument,
    geosolve_sketch::DesignPointId,
    geosolve_sketch::DocumentExternalBindingId,
    geosolve_sketch::DocumentConstraintId,
) {
    let mut document = SketchDocument::new(10.0).unwrap();
    let point = document.add_point("native point", [9.0, -2.0]).unwrap();
    let binding = document
        .add_external_binding("external datum", ExternalFeatureKindV1::Point, None)
        .unwrap();
    let constraint = document
        .add_constraint(
            "external point source",
            DocumentConstraintDefinition::ExternalPointCoincident {
                point,
                external: DocumentExternalPointRef { binding },
            },
        )
        .unwrap();
    (document, point, binding, constraint)
}

#[test]
fn external_binding_is_local_monotone_and_draft_only() {
    let mut document = SketchDocument::new(10.0).unwrap();
    let first = document
        .add_external_binding("datum", ExternalFeatureKindV1::Point, None)
        .unwrap();
    let topology = ExternalTopologyDigest::from_bytes([7; 32]);
    let second = document
        .add_external_binding("edge", ExternalFeatureKindV1::LineSegment, Some(topology))
        .unwrap();
    assert!(second.0 > first.0);
    assert!(matches!(
        document.to_canonical_json(),
        Err(DocumentError::UnsupportedM43State)
    ));

    let bytes = document.to_draft_v5_json().unwrap();
    assert!(!bytes.contains("host_key"));
    let restored = SketchDocument::from_draft_v5_json(&bytes).unwrap();
    assert_eq!(restored.external_bindings(), document.external_bindings());
    assert_eq!(restored.to_draft_v5_json().unwrap(), bytes);

    document
        .rebind_external_binding(first, ExternalFeatureKindV1::LineSegment, Some(topology))
        .unwrap();
    assert_eq!(
        document.external_binding(first).unwrap().expected_topology,
        Some(topology)
    );
}

#[test]
fn external_operands_are_explicit_and_kind_checked() {
    let mut document = SketchDocument::new(10.0).unwrap();
    let point = document.add_point("p", [0.0, 0.0]).unwrap();
    let start = document.add_point("a", [0.0, 0.0]).unwrap();
    let end = document.add_point("b", [1.0, 0.0]).unwrap();
    let line = document
        .add_curve(
            "line",
            CurveDefinition::Line {
                start,
                end,
                branch_direction: [1.0, 0.0],
            },
        )
        .unwrap();
    let point_binding = document
        .add_external_binding("datum", ExternalFeatureKindV1::Point, None)
        .unwrap();
    let line_binding = document
        .add_external_binding(
            "edge",
            ExternalFeatureKindV1::LineSegment,
            Some(ExternalTopologyDigest::from_bytes([9; 32])),
        )
        .unwrap();
    document
        .add_constraint(
            "point/reference",
            DocumentConstraintDefinition::ExternalPointCoincident {
                point,
                external: DocumentExternalPointRef {
                    binding: point_binding,
                },
            },
        )
        .unwrap();
    document
        .add_constraint(
            "line/reference",
            DocumentConstraintDefinition::ExternalLineCollinear {
                line: DocumentLineSupportRef {
                    span: geosolve_sketch::CurveSpan::line(line),
                    direction: DocumentDirectionSense::Forward,
                },
                external: DocumentExternalLineSupportRef {
                    binding: line_binding,
                    direction: DocumentDirectionSense::Forward,
                },
            },
        )
        .unwrap();
    document.validate().unwrap();
}

#[test]
fn snapshot_set_is_canonical_strict_and_exactly_stamped() {
    let mut document = SketchDocument::new(1.0).unwrap();
    let point_binding = document
        .add_external_binding("point", ExternalFeatureKindV1::Point, None)
        .unwrap();
    let line_binding = document
        .add_external_binding(
            "line",
            ExternalFeatureKindV1::LineSegment,
            Some(ExternalTopologyDigest::from_bytes([3; 32])),
        )
        .unwrap();
    let point = ExternalSnapshotEntry {
        binding: point_binding,
        source_revision: 4,
        source_digest: ExternalSnapshotDigest::from_bytes([1; 32]),
        feature: ExternalSnapshotFeatureV1::Point {
            position: [2.0, -3.0],
            scale: 10.0,
            resources: resources(1, 0, 0),
        },
    };
    let line = ExternalSnapshotEntry {
        binding: line_binding,
        source_revision: 8,
        source_digest: ExternalSnapshotDigest::from_bytes([2; 32]),
        feature: ExternalSnapshotFeatureV1::LineSegment {
            start: [0.0, 0.0],
            end: [4.0, 1.0],
            domain: [0.0, 1.0],
            orientation: ExternalLineOrientationV1::StartToEnd,
            scale: 5.0,
            topology_digest: ExternalTopologyDigest::from_bytes([3; 32]),
            resources: resources(2, 0, 1),
        },
    };
    let first = ExternalSnapshotSetV1::new(12, vec![line.clone(), point.clone()]).unwrap();
    let second = ExternalSnapshotSetV1::new(12, vec![point, line]).unwrap();
    assert_eq!(first, second);
    assert_eq!(first.entries()[0].binding, point_binding);
    let json = first.to_canonical_json().unwrap();
    assert_eq!(ExternalSnapshotSetV1::from_json(&json).unwrap(), first);
    assert!(matches!(
        ExternalSnapshotSetV1::from_json(&json.replacen(
            "\"orientation\":\"start_to_end\"",
            "\"orientation\":\"reversed\"",
            1
        )),
        Err(ExternalSnapshotInputError::Json(_))
    ));
    assert!(matches!(
        ExternalSnapshotSetV1::from_json(&json.replacen("\"revision\":12", "\"revision\":13", 1)),
        Err(ExternalSnapshotInputError::DigestMismatch)
    ));
    assert!(matches!(
        ExternalSnapshotSetV1::from_json(&json.replacen('{', "{\"unknown\":0,", 1)),
        Err(ExternalSnapshotInputError::Json(_))
    ));
}

#[test]
fn malformed_snapshot_features_never_canonicalize() {
    let mut document = SketchDocument::new(1.0).unwrap();
    let binding = document
        .add_external_binding("point", ExternalFeatureKindV1::Point, None)
        .unwrap();
    let entry = |feature| ExternalSnapshotEntry {
        binding,
        source_revision: 1,
        source_digest: ExternalSnapshotDigest::default(),
        feature,
    };
    for feature in [
        ExternalSnapshotFeatureV1::Point {
            position: [f64::NAN, 0.0],
            scale: 1.0,
            resources: resources(1, 0, 0),
        },
        ExternalSnapshotFeatureV1::Point {
            position: [0.0, 0.0],
            scale: 0.0,
            resources: resources(1, 0, 0),
        },
        ExternalSnapshotFeatureV1::Point {
            position: [0.0, 0.0],
            scale: 1.0,
            resources: resources(2, 0, 0),
        },
    ] {
        assert!(matches!(
            ExternalSnapshotSetV1::new(1, vec![entry(feature)]),
            Err(ExternalSnapshotInputError::InvalidFeature { .. })
        ));
    }
}

#[test]
fn snapshot_validation_covers_duplicate_nonfinite_scale_resources_domain_and_degeneracy() {
    let mut document = SketchDocument::new(1.0).unwrap();
    let point = document
        .add_external_binding("point", ExternalFeatureKindV1::Point, None)
        .unwrap();
    let line = document
        .add_external_binding(
            "line",
            ExternalFeatureKindV1::LineSegment,
            Some(ExternalTopologyDigest::from_bytes([4; 32])),
        )
        .unwrap();
    let valid_point = point_entry(point, [0.0, 0.0]);
    assert!(matches!(
        ExternalSnapshotSetV1::new(1, vec![valid_point.clone(), valid_point.clone()]),
        Err(ExternalSnapshotInputError::DuplicateBinding { .. })
    ));
    for feature in [
        ExternalSnapshotFeatureV1::Point {
            position: [f64::INFINITY, 0.0],
            scale: 1.0,
            resources: resources(1, 0, 0),
        },
        ExternalSnapshotFeatureV1::Point {
            position: [0.0, 0.0],
            scale: -1.0,
            resources: resources(1, 0, 0),
        },
        ExternalSnapshotFeatureV1::Point {
            position: [0.0, 0.0],
            scale: 1.0,
            resources: resources(1, 1, 0),
        },
        ExternalSnapshotFeatureV1::LineSegment {
            start: [0.0, 0.0],
            end: [0.0, 0.0],
            domain: [0.0, 1.0],
            orientation: ExternalLineOrientationV1::StartToEnd,
            scale: 1.0,
            topology_digest: ExternalTopologyDigest::from_bytes([4; 32]),
            resources: resources(2, 0, 1),
        },
        ExternalSnapshotFeatureV1::LineSegment {
            start: [0.0, 0.0],
            end: [1.0, 0.0],
            domain: [-0.0, 1.0],
            orientation: ExternalLineOrientationV1::StartToEnd,
            scale: 1.0,
            topology_digest: ExternalTopologyDigest::from_bytes([4; 32]),
            resources: resources(2, 0, 1),
        },
    ] {
        assert!(matches!(
            ExternalSnapshotSetV1::new(
                1,
                vec![ExternalSnapshotEntry {
                    binding: line,
                    source_revision: 1,
                    source_digest: ExternalSnapshotDigest::default(),
                    feature
                }]
            ),
            Err(ExternalSnapshotInputError::InvalidFeature { .. })
        ));
    }
    assert!(matches!(
        ExternalSnapshotSetV1::new(1, vec![valid_point; MAX_EXTERNAL_SNAPSHOT_ENTRIES + 1]),
        Err(ExternalSnapshotInputError::ResourceLimit { .. })
    ));
}

#[test]
#[allow(clippy::too_many_lines)]
fn point_and_line_snapshots_constrain_only_native_geometry_with_exact_stamps_and_audit() {
    let (document, point, point_binding, _) = external_point_document();
    let point_set =
        ExternalSnapshotSetV1::new(7, vec![point_entry(point_binding, [2.0, 3.0])]).unwrap();
    let mut point_session = RetainedSketchDocumentSession::new_with_inputs(
        document,
        ParameterBatch::default(),
        point_set.clone(),
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let point_accepted = point_session.accepted_state().unwrap();
    assert_eq!(point_accepted.input().external_snapshot_set_revision(), 7);
    assert_eq!(
        point_accepted.input().external_snapshot_set_digest(),
        point_set.digest()
    );
    assert_eq!(point_session.last_attempt().input(), point_accepted.input());
    let solved = point_accepted
        .solve_result()
        .geometry
        .point(point_accepted.mappings().runtime_point(point).unwrap())
        .unwrap();
    assert!((solved.x - 2.0).abs() < 1e-9 && (solved.y - 3.0).abs() < 1e-9);
    let audit = &point_accepted.solve_result().display_audit.sources[0].rows;
    assert!(audit.iter().all(|row| {
        row.bindings
            .iter()
            .any(|binding| binding.name == "external binding")
    }));
    for name in [
        "external expected kind",
        "external actual kind",
        "external feature scale",
    ] {
        assert!(
            audit
                .iter()
                .all(|row| row.bindings.iter().any(|binding| binding.name == name))
        );
    }
    assert!(audit.iter().all(|row| {
        row.bindings
            .iter()
            .any(|binding| binding.name == "external source digest")
    }));
    assert!(audit.iter().all(|row| row.incident_variables.len() == 1));
    let repeated_geometry = point_accepted.solve_result().geometry.clone();
    let repeated_audit = point_accepted.solve_result().display_audit.clone();
    let repeated_input = point_accepted.input();
    let original_preference = repeated_audit
        .sources
        .iter()
        .find(|source| source.source_label == "previous-state preference for native point")
        .expect("native point PreviousState audit");
    assert!(original_preference.rows.iter().all(|row| {
        row.bindings
            .iter()
            .any(|binding| binding.name == "target" && binding.value == "(9, -2)")
    }));
    point_session
        .reattempt(
            point_session.design_identity(),
            DocumentSolveRequest::default(),
        )
        .unwrap();
    let repeated = point_session.accepted_state().unwrap();
    assert_eq!(repeated.input(), repeated_input);
    assert_eq!(repeated.solve_result().geometry, repeated_geometry);
    assert_eq!(repeated.solve_result().display_audit, repeated_audit);
    let controlled = point_session
        .reattempt_controlled(
            point_session.design_identity(),
            DocumentSolveRequest::default(),
            OperationControl::unlimited(),
        )
        .unwrap();
    assert!(matches!(controlled, OperationOutcome::Completed { .. }));
    let controlled_repeated = point_session.accepted_state().unwrap();
    assert_eq!(controlled_repeated.input(), repeated_input);
    assert_eq!(
        controlled_repeated.solve_result().geometry,
        repeated_geometry
    );
    assert_eq!(
        controlled_repeated.solve_result().display_audit,
        repeated_audit
    );
    assert_eq!(point_session.external_snapshot_set(), &point_set);

    let mut document = SketchDocument::new(10.0).unwrap();
    let start = document.add_point("native start", [0.0, 2.0]).unwrap();
    let end = document.add_point("native end", [3.0, 4.0]).unwrap();
    let native = document
        .add_curve(
            "native line",
            CurveDefinition::Line {
                start,
                end,
                branch_direction: [1.0, 0.0],
            },
        )
        .unwrap();
    let topology = ExternalTopologyDigest::from_bytes([13; 32]);
    let binding = document
        .add_external_binding(
            "external edge",
            ExternalFeatureKindV1::LineSegment,
            Some(topology),
        )
        .unwrap();
    document
        .add_constraint(
            "external line source",
            DocumentConstraintDefinition::ExternalLineCollinear {
                line: DocumentLineSupportRef {
                    span: geosolve_sketch::CurveSpan::line(native),
                    direction: DocumentDirectionSense::Forward,
                },
                external: DocumentExternalLineSupportRef {
                    binding,
                    direction: DocumentDirectionSense::Forward,
                },
            },
        )
        .unwrap();
    let set = ExternalSnapshotSetV1::new(
        8,
        vec![line_entry(binding, topology, [-1.0, 0.0], [4.0, 0.0])],
    )
    .unwrap();
    let session = RetainedSketchDocumentSession::new_with_inputs(
        document,
        ParameterBatch::default(),
        set.clone(),
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let accepted = session.accepted_state().unwrap();
    assert_eq!(accepted.input().external_snapshot_set_revision(), 8);
    assert_eq!(
        accepted.input().external_snapshot_set_digest(),
        set.digest()
    );
    for id in [start, end] {
        assert!(
            accepted
                .solve_result()
                .geometry
                .point(accepted.mappings().runtime_point(id).unwrap())
                .unwrap()
                .y
                .abs()
                < 1e-9
        );
    }
    let rows = &accepted.solve_result().display_audit.sources[0].rows;
    assert!(rows.iter().all(|row| {
        row.bindings
            .iter()
            .any(|binding| binding.name == "external set revision" && binding.value == "8")
    }));
    for name in [
        "external line domain",
        "external line orientation",
        "external line topology digest",
    ] {
        assert!(
            rows.iter()
                .all(|row| row.bindings.iter().any(|binding| binding.name == name))
        );
    }
    assert!(rows.iter().all(|row| row.incident_variables.len() == 2));
}

#[test]
fn external_line_rows_match_finite_differences_with_all_native_incidence() {
    let (_, _, binding, _) = external_point_document();
    let topology = ExternalTopologyDigest::from_bytes([23; 32]);
    let mut sketch = Sketch::new(7.0).unwrap();
    let start = sketch
        .add_named_point("start", Point2::new(1.0, 2.0))
        .unwrap();
    let end = sketch
        .add_named_point("end", Point2::new(4.0, 5.0))
        .unwrap();
    let segment = sketch.add_named_segment("line", start, end).unwrap();
    sketch
        .add_external_line_collinear(
            segment,
            Point2::new(-2.0, 0.5),
            Point2::new(6.0, 1.5),
            geosolve_sketch::ExternalConstraintProvenance {
                binding,
                expected_kind: ExternalFeatureKindV1::LineSegment,
                actual_kind: ExternalFeatureKindV1::LineSegment,
                feature_scale: 3.0,
                line_domain: Some([0.0, 1.0]),
                line_orientation: Some(ExternalLineOrientationV1::StartToEnd),
                line_topology_digest: Some(topology),
                set_revision: 4,
                set_digest: ExternalSnapshotSetDigest::default(),
                source_revision: 9,
                source_digest: ExternalSnapshotDigest::default(),
            },
        )
        .unwrap();
    let compiled = sketch.compile(SketchSolveRequest::default()).unwrap();
    let report = compiled.problem().check_jacobians(1.0e-6).unwrap();
    assert!(report.all_within(1.0e-6), "{report:#?}");
    let audit = compiled.problem().audit_snapshot().unwrap();
    let external = audit
        .sources
        .iter()
        .find(|source| {
            source.rows.iter().any(|row| {
                row.bindings
                    .iter()
                    .any(|binding| binding.name == "external binding")
            })
        })
        .unwrap();
    assert_eq!(external.rows.len(), 2);
    assert!(
        external
            .rows
            .iter()
            .all(|row| row.incident_variables.len() == 2)
    );
}

#[test]
#[allow(clippy::too_many_lines)]
fn topology_mismatch_is_typed_atomic_and_requires_rebind_before_recovery() {
    let topology_a = ExternalTopologyDigest::from_bytes([31; 32]);
    let topology_b = ExternalTopologyDigest::from_bytes([32; 32]);
    let mut document = SketchDocument::new(10.0).unwrap();
    let start = document.add_point("start", [0.0, 2.0]).unwrap();
    let end = document.add_point("end", [4.0, 4.0]).unwrap();
    let line = document
        .add_curve(
            "native line",
            CurveDefinition::Line {
                start,
                end,
                branch_direction: [1.0, 0.0],
            },
        )
        .unwrap();
    let binding = document
        .add_external_binding(
            "external edge",
            ExternalFeatureKindV1::LineSegment,
            Some(topology_a),
        )
        .unwrap();
    document
        .add_constraint(
            "external collinearity",
            DocumentConstraintDefinition::ExternalLineCollinear {
                line: DocumentLineSupportRef {
                    span: geosolve_sketch::CurveSpan::line(line),
                    direction: DocumentDirectionSense::Forward,
                },
                external: DocumentExternalLineSupportRef {
                    binding,
                    direction: DocumentDirectionSense::Forward,
                },
            },
        )
        .unwrap();
    let initial_set = ExternalSnapshotSetV1::new(
        10,
        vec![line_entry(binding, topology_a, [-1.0, 0.0], [5.0, 0.0])],
    )
    .unwrap();
    let mut session = RetainedSketchDocumentSession::new_with_inputs(
        document,
        ParameterBatch::default(),
        initial_set.clone(),
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let retained_identity = session.accepted_state().unwrap().identity();
    let retained_geometry = session
        .accepted_state()
        .unwrap()
        .solve_result()
        .geometry
        .clone();

    let incompatible = ExternalSnapshotSetV1::new(
        11,
        vec![line_entry(binding, topology_b, [-1.0, 0.0], [5.0, 0.0])],
    )
    .unwrap();
    session
        .update_external_snapshot_set(
            session.design_identity(),
            incompatible,
            DocumentSolveRequest::default(),
        )
        .unwrap();
    assert!(matches!(
        session
            .last_attempt()
            .failure()
            .and_then(|failure| failure.external_snapshot_error()),
        Some(ExternalSnapshotInputError::TopologyMismatch { binding: actual })
            if *actual == binding
    ));
    assert_eq!(session.external_snapshot_set(), &initial_set);
    assert_eq!(
        session.accepted_state().unwrap().identity(),
        retained_identity
    );
    assert_eq!(
        session.accepted_state().unwrap().solve_result().geometry,
        retained_geometry
    );

    session
        .transact(session.design_identity(), |candidate| {
            candidate.rebind_external_binding(
                binding,
                ExternalFeatureKindV1::LineSegment,
                Some(topology_b),
            )
        })
        .unwrap();
    assert!(matches!(
        session
            .last_attempt()
            .failure()
            .and_then(|failure| failure.external_snapshot_error()),
        Some(ExternalSnapshotInputError::TopologyMismatch { .. })
    ));
    assert_eq!(
        session.accepted_state().unwrap().identity(),
        retained_identity
    );

    let rebound_set = ExternalSnapshotSetV1::new(
        12,
        vec![line_entry(binding, topology_b, [-1.0, 0.0], [5.0, 0.0])],
    )
    .unwrap();
    session
        .update_external_snapshot_set(
            session.design_identity(),
            rebound_set.clone(),
            DocumentSolveRequest::default(),
        )
        .unwrap();
    assert!(session.last_attempt().failure().is_none());
    assert_eq!(session.external_snapshot_set(), &rebound_set);
    assert_eq!(
        session
            .accepted_state()
            .unwrap()
            .input()
            .external_snapshot_set_digest(),
        rebound_set.digest()
    );
    assert_ne!(
        session.accepted_state().unwrap().identity(),
        retained_identity
    );
}

#[test]
fn unavailable_external_activity_preserves_user_and_parameter_inactivity_precedence() {
    let mut document = SketchDocument::new(10.0).unwrap();
    let mut constraints = Vec::new();
    let mut bindings = Vec::new();
    for index in 0..3 {
        let point = document
            .add_point(format!("point {index}"), [f64::from(index), 0.0])
            .unwrap();
        let binding = document
            .add_external_binding(
                format!("binding {index}"),
                ExternalFeatureKindV1::Point,
                None,
            )
            .unwrap();
        let constraint = document
            .add_constraint(
                format!("constraint {index}"),
                DocumentConstraintDefinition::ExternalPointCoincident {
                    point,
                    external: DocumentExternalPointRef { binding },
                },
            )
            .unwrap();
        bindings.push(binding);
        constraints.push(constraint);
    }
    document
        .set_element_user_suppressed(constraints[0].into(), true)
        .unwrap();
    let activation = document
        .add_parameter("host activation", DocumentParameterKind::Activation)
        .unwrap();
    document
        .add_parameter_binding(
            activation,
            DocumentParameterTarget::Activation(constraints[1].into()),
        )
        .unwrap();
    let parameters = ParameterBatch::new(
        3,
        vec![ParameterBatchEntry {
            parameter: activation,
            value: ParameterValue::Activation(false),
        }],
    )
    .unwrap();
    let session = RetainedSketchDocumentSession::new_with_inputs(
        document,
        parameters.clone(),
        ExternalSnapshotSetV1::default(),
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let failure = session.last_attempt().failure().unwrap();
    assert_eq!(
        failure.kind(),
        SketchAttemptFailureKind::ExternalSnapshotInput
    );
    assert!(matches!(
        failure.external_snapshot_error(),
        Some(ExternalSnapshotInputError::MissingBinding { binding })
            if *binding == bindings[2]
    ));
    let activity = failure.effective_activity().unwrap();
    assert_eq!(
        activity.reason(constraints[0]),
        Some(InactivityReason::UserSuppressed)
    );
    assert_eq!(
        activity.reason(constraints[1]),
        Some(InactivityReason::HostConfigurationInactive)
    );
    assert_eq!(
        activity.reason(constraints[2]),
        Some(InactivityReason::UnavailableDependency {
            dependency: bindings[2].into(),
        })
    );
    assert_eq!(session.last_attempt().input().parameter_revision(), 3);
    assert_eq!(
        session.last_attempt().input().parameter_digest(),
        parameters.digest()
    );
}

#[test]
fn external_input_failures_are_typed_and_atomic_and_rebind_is_explicit() {
    let (mut document, _point, binding, constraint) = external_point_document();
    let unused = document
        .add_external_binding("unused external", ExternalFeatureKindV1::Point, None)
        .unwrap();
    let accepted_set =
        ExternalSnapshotSetV1::new(5, vec![point_entry(binding, [2.0, 3.0])]).unwrap();
    let mut session = RetainedSketchDocumentSession::new_with_inputs(
        document.clone(),
        ParameterBatch::default(),
        accepted_set.clone(),
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let accepted = session.accepted_state().unwrap();
    let identity = accepted.identity();
    let geometry = accepted.solve_result().geometry.clone();
    for candidate in [
        ExternalSnapshotSetV1::new(6, vec![point_entry(unused, [0.0, 0.0])]).unwrap(),
        ExternalSnapshotSetV1::new(
            7,
            vec![line_entry(
                binding,
                ExternalTopologyDigest::from_bytes([1; 32]),
                [0.0, 0.0],
                [1.0, 0.0],
            )],
        )
        .unwrap(),
    ] {
        session
            .update_external_snapshot_set(
                session.design_identity(),
                candidate,
                DocumentSolveRequest::default(),
            )
            .unwrap();
        assert_eq!(session.accepted_state().unwrap().identity(), identity);
        assert_eq!(
            session.accepted_state().unwrap().solve_result().geometry,
            geometry
        );
        assert_eq!(session.external_snapshot_set(), &accepted_set);
        assert!(matches!(
            session
                .last_attempt()
                .failure()
                .map(geosolve_sketch::SketchAttemptFailure::kind),
            Some(SketchAttemptFailureKind::ExternalSnapshotInput)
        ));
    }
    assert!(matches!(
        session
            .last_attempt()
            .failure()
            .unwrap()
            .external_snapshot_error(),
        Some(ExternalSnapshotInputError::WrongKind { .. })
    ));
    let activity = session
        .last_attempt()
        .failure()
        .unwrap()
        .effective_activity()
        .unwrap();
    assert_eq!(
        activity.reason(binding),
        Some(InactivityReason::UnavailableExternalReference)
    );
    assert_eq!(
        activity.reason(constraint),
        Some(InactivityReason::UnavailableDependency {
            dependency: binding.into()
        })
    );
    assert!(matches!(
        session.update_external_snapshot_set(
            session.design_identity(),
            ExternalSnapshotSetV1::new(5, vec![point_entry(binding, [3.0, 3.0])]).unwrap(),
            DocumentSolveRequest::default()
        ),
        Err(DocumentSessionError::StaleExternalSnapshotRevision {
            actual: 5,
            retained: 5
        })
    ));
}
