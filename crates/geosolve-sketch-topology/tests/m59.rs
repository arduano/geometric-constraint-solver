// SPDX-License-Identifier: GPL-3.0-or-later

#![allow(clippy::float_cmp)]

use geosolve_sketch::{
    CancellationToken, CurveDefinition, CurveSpan, DocumentCurveTrimView, DocumentParameterKind,
    DocumentParameterTarget, DocumentSolveRequest, DocumentTrimBoundary, DocumentTrimParameter,
    ExternalFeatureKindV1, ExternalLineOrientationV1, ExternalSnapshotDigest,
    ExternalSnapshotEntry, ExternalSnapshotFeatureV1, ExternalSnapshotResourcesV1,
    ExternalSnapshotSet, ExternalTopologyDigest, GeometryRole, OperationControl, OperationLimits,
    OperationOutcome, ParameterBatch, ParameterBatchEntry, ParameterValue,
    RetainedSketchDocumentSession, ScalarDomain, ScalarUnit, SketchDocument, SolverConfig,
    cancellation_pair,
};
use geosolve_sketch_topology::{
    PreparedTopologyQuery, TopologyCompleteness, TopologyExternalGeometryScope, TopologyIssueKind,
    TopologyLimits, TopologyNativeGeometryScope, TopologyOrientation, TopologyProductionProfile,
    TopologyRequest, TopologyResult, TopologySelfIntersectionPolicy, TopologySnapshot,
    TopologySnapshotError, TopologySourceProvenance,
};

fn session(document: SketchDocument) -> RetainedSketchDocumentSession {
    RetainedSketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap()
}

fn add_line(
    document: &mut SketchDocument,
    label: &str,
    start: geosolve_sketch::DesignPointId,
    end: geosolve_sketch::DesignPointId,
) -> geosolve_sketch::CurveId {
    let first = document.point(start).unwrap().position;
    let second = document.point(end).unwrap().position;
    let delta = [second[0] - first[0], second[1] - first[1]];
    let length = delta[0].hypot(delta[1]);
    document
        .add_curve(
            label,
            CurveDefinition::Line {
                start,
                end,
                branch_direction: [delta[0] / length, delta[1] / length],
            },
        )
        .unwrap()
}

fn add_square(
    document: &mut SketchDocument,
    label: &str,
    origin: [f64; 2],
    size: f64,
) -> [geosolve_sketch::CurveId; 4] {
    let [x, y] = origin;
    let points = [
        document.add_point(format!("{label}.p1"), [x, y]).unwrap(),
        document
            .add_point(format!("{label}.p2"), [x + size, y])
            .unwrap(),
        document
            .add_point(format!("{label}.p3"), [x + size, y + size])
            .unwrap(),
        document
            .add_point(format!("{label}.p4"), [x, y + size])
            .unwrap(),
    ];
    [
        add_line(document, &format!("{label}.e1"), points[0], points[1]),
        add_line(document, &format!("{label}.e2"), points[1], points[2]),
        add_line(document, &format!("{label}.e3"), points[2], points[3]),
        add_line(document, &format!("{label}.e4"), points[3], points[0]),
    ]
}

fn completed(
    session: &RetainedSketchDocumentSession,
    request: TopologyRequest,
) -> geosolve_sketch_topology::TopologyResult {
    let outcome = TopologySnapshot::capture(session)
        .unwrap()
        .prepare(request)
        .execute(OperationControl::default())
        .unwrap();
    let OperationOutcome::Completed { value, .. } = outcome else {
        panic!("uncontrolled topology query must complete");
    };
    value
}

#[test]
fn closed_square_publishes_freshly_validated_production_wire_and_region() {
    let mut document = SketchDocument::new(10.0).unwrap();
    let curves = add_square(&mut document, "square", [0.0, 0.0], 4.0);
    let session = session(document);
    let result = completed(&session, TopologyRequest::default());
    assert_eq!(result.completeness, TopologyCompleteness::Complete);
    assert!(result.issues.is_empty());
    let profile = result.production_profile.unwrap();
    assert_eq!(profile.regions().len(), 1);
    assert_eq!(profile.wires().len(), 1);
    assert_eq!(
        profile.wires()[0].orientation,
        TopologyOrientation::CounterClockwise
    );
    assert_eq!(profile.wires()[0].fragments.len(), 4);
    assert_eq!(profile.regions()[0].area, 16.0);
    assert!(profile.regions()[0].holes.is_empty());
    assert!(profile.wires()[0].fragments.iter().all(|fragment| matches!(
        fragment.source,
        TopologySourceProvenance::Native { support, .. }
            if curves.contains(&support.curve)
    )));
    profile.validate_current(&session).unwrap();
}

#[test]
fn nested_circles_publish_outer_hole_nesting_and_exact_source_provenance() {
    let mut document = SketchDocument::new(10.0).unwrap();
    let center = document.add_point("center", [0.0, 0.0]).unwrap();
    let outer_radius = document
        .add_scalar(
            "outer radius",
            4.0,
            ScalarUnit::Length,
            ScalarDomain::Positive,
        )
        .unwrap();
    let inner_radius = document
        .add_scalar(
            "inner radius",
            2.0,
            ScalarUnit::Length,
            ScalarDomain::Positive,
        )
        .unwrap();
    let outer = document
        .add_curve(
            "outer",
            CurveDefinition::Circle {
                center,
                radius: outer_radius,
            },
        )
        .unwrap();
    let inner = document
        .add_curve(
            "inner",
            CurveDefinition::Circle {
                center,
                radius: inner_radius,
            },
        )
        .unwrap();
    let session = session(document);
    let result = completed(&session, TopologyRequest::default());
    assert_eq!(result.completeness, TopologyCompleteness::Complete);
    let profile = result.production_profile.unwrap();
    assert_eq!(profile.regions().len(), 2);
    assert!(
        profile
            .regions()
            .iter()
            .any(|region| region.holes.len() == 1)
    );
    let sources = profile
        .wires()
        .iter()
        .flat_map(|wire| wire.fragments.iter())
        .filter_map(|fragment| match fragment.source {
            TopologySourceProvenance::Native { support, .. } => Some(support.curve),
            TopologySourceProvenance::ExternalLine { .. } => None,
        })
        .collect::<Vec<_>>();
    assert!(sources.contains(&outer));
    assert!(sources.contains(&inner));
}

#[test]
fn construction_scope_is_explicit_and_never_inferred_from_geometry() {
    let mut document = SketchDocument::new(10.0).unwrap();
    let curves = add_square(&mut document, "construction", [0.0, 0.0], 3.0);
    for curve in curves {
        document
            .set_geometry_role(curve, GeometryRole::Construction)
            .unwrap();
    }
    let session = session(document);
    let excluded = completed(&session, TopologyRequest::default());
    assert_eq!(excluded.completeness, TopologyCompleteness::Complete);
    assert!(excluded.scope.eligible_sources.is_empty());
    assert!(excluded.production_profile.unwrap().wires().is_empty());

    let included = completed(
        &session,
        TopologyRequest {
            native_geometry: TopologyNativeGeometryScope::ProfileAndConstruction,
            ..TopologyRequest::default()
        },
    );
    assert_eq!(
        included.completeness,
        TopologyCompleteness::Complete,
        "{:?}",
        included.issues
    );
    assert_eq!(included.production_profile.unwrap().regions().len(), 1);
}

#[test]
fn open_or_ambiguous_geometry_never_produces_consumable_wires() {
    let mut open = SketchDocument::new(10.0).unwrap();
    let first = open.add_point("first", [0.0, 0.0]).unwrap();
    let second = open.add_point("second", [4.0, 0.0]).unwrap();
    add_line(&mut open, "open", first, second);
    let open = completed(&session(open), TopologyRequest::default());
    assert_eq!(open.completeness, TopologyCompleteness::Skipped);
    assert!(open.production_profile.is_none());
    assert!(
        open.issues
            .iter()
            .any(|issue| issue.kind == TopologyIssueKind::UncoveredEligibleSource)
    );

    let mut overlap = SketchDocument::new(10.0).unwrap();
    let first = overlap.add_point("first", [0.0, 0.0]).unwrap();
    let second = overlap.add_point("second", [4.0, 0.0]).unwrap();
    add_line(&mut overlap, "one", first, second);
    add_line(&mut overlap, "two", first, second);
    let overlap = completed(&session(overlap), TopologyRequest::default());
    assert_ne!(overlap.completeness, TopologyCompleteness::Complete);
    assert!(overlap.production_profile.is_none());
    assert!(
        overlap
            .issues
            .iter()
            .any(|issue| issue.kind == TopologyIssueKind::OverlapRejected)
    );
}

#[test]
fn self_intersection_policy_is_explicit_and_fail_closed() {
    let mut document = SketchDocument::new(10.0).unwrap();
    let points = [
        document.add_point("p1", [0.0, 0.0]).unwrap(),
        document.add_point("p2", [4.0, 4.0]).unwrap(),
        document.add_point("p3", [0.0, 4.0]).unwrap(),
        document.add_point("p4", [4.0, 0.0]).unwrap(),
    ];
    document
        .add_curve(
            "bow tie",
            CurveDefinition::Polyline {
                points: points.to_vec(),
                closed: true,
                branch_directions: vec![
                    [std::f64::consts::FRAC_1_SQRT_2; 2],
                    [-1.0, 0.0],
                    [
                        std::f64::consts::FRAC_1_SQRT_2,
                        -std::f64::consts::FRAC_1_SQRT_2,
                    ],
                    [-1.0, 0.0],
                ],
            },
        )
        .unwrap();
    let session = session(document);
    let result = completed(
        &session,
        TopologyRequest {
            policy: geosolve_sketch_topology::TopologyPolicy {
                self_intersections: TopologySelfIntersectionPolicy::Reject,
                ..Default::default()
            },
            ..TopologyRequest::default()
        },
    );
    assert_eq!(result.completeness, TopologyCompleteness::Skipped);
    assert!(result.production_profile.is_none());
    assert!(
        result
            .issues
            .iter()
            .any(|issue| issue.kind == TopologyIssueKind::SelfIntersectionRejected)
    );
}

#[test]
fn external_line_scope_is_explicit_and_carries_binding_provenance() {
    let mut document = SketchDocument::new(10.0).unwrap();
    let points = [
        document.add_point("p1", [0.0, 0.0]).unwrap(),
        document.add_point("p2", [4.0, 0.0]).unwrap(),
        document.add_point("p3", [4.0, 3.0]).unwrap(),
        document.add_point("p4", [0.0, 3.0]).unwrap(),
    ];
    add_line(&mut document, "bottom", points[0], points[1]);
    add_line(&mut document, "right", points[1], points[2]);
    add_line(&mut document, "top", points[2], points[3]);
    let topology = ExternalTopologyDigest::from_bytes([9; 32]);
    let binding = document
        .add_external_binding(
            "left datum",
            ExternalFeatureKindV1::LineSegment,
            Some(topology),
        )
        .unwrap();
    let snapshots = ExternalSnapshotSet::new(
        1,
        vec![ExternalSnapshotEntry {
            binding,
            source_revision: 7,
            source_digest: ExternalSnapshotDigest::from_bytes([4; 32]),
            feature: ExternalSnapshotFeatureV1::LineSegment {
                start: [0.0, 3.0],
                end: [0.0, 0.0],
                domain: [0.0, 1.0],
                orientation: ExternalLineOrientationV1::StartToEnd,
                scale: 1.0,
                topology_digest: topology,
                resources: ExternalSnapshotResourcesV1 {
                    point_count: 2,
                    control_count: 0,
                    span_count: 1,
                },
            },
        }],
    )
    .unwrap();
    let session = RetainedSketchDocumentSession::new_with_inputs(
        document,
        ParameterBatch::default(),
        snapshots,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let excluded = completed(&session, TopologyRequest::default());
    assert_eq!(excluded.completeness, TopologyCompleteness::Skipped);
    let included = completed(
        &session,
        TopologyRequest {
            external_geometry: TopologyExternalGeometryScope::IncludeLineSegments,
            ..TopologyRequest::default()
        },
    );
    assert_eq!(included.completeness, TopologyCompleteness::Skipped);
    assert!(included.production_profile.is_none());
    let external = included
        .scope
        .eligible_sources
        .iter()
        .find_map(|source| match &source.source {
            TopologySourceProvenance::ExternalLine {
                binding, domain, ..
            } => Some((*binding, *domain)),
            TopologySourceProvenance::Native { .. } => None,
        })
        .unwrap();
    assert_eq!(external.0, binding);
    assert_eq!(external.1, [0.0, 1.0]);
    assert_eq!(included.scope.external_lines, vec![binding]);
    assert!(
        included
            .issues
            .iter()
            .any(|issue| issue.kind == TopologyIssueKind::UncoveredEligibleSource)
    );
}

#[test]
fn cancellation_exhaustion_and_stale_consumption_are_distinct() {
    let mut document = SketchDocument::new(10.0).unwrap();
    add_square(&mut document, "square", [0.0, 0.0], 4.0);
    let mut session = session(document);
    let profile = completed(&session, TopologyRequest::default())
        .production_profile
        .unwrap();
    let before_input = session.prepared_input();
    let before_document = session.design_document().to_draft_v5_json().unwrap();

    let (handle, token) = cancellation_pair();
    handle.cancel();
    let cancelled = TopologySnapshot::capture(&session)
        .unwrap()
        .prepare(TopologyRequest::default())
        .execute(OperationControl::new(token, OperationLimits::unlimited()))
        .unwrap();
    assert!(matches!(cancelled, OperationOutcome::Cancelled { .. }));
    assert_eq!(session.prepared_input(), before_input);
    assert_eq!(
        session.design_document().to_draft_v5_json().unwrap(),
        before_document
    );

    let mut limits = OperationLimits::unlimited();
    limits.profile_fragments = 0;
    let exhausted = TopologySnapshot::capture(&session)
        .unwrap()
        .prepare(TopologyRequest::default())
        .execute(OperationControl::new(CancellationToken::default(), limits))
        .unwrap();
    assert!(matches!(exhausted, OperationOutcome::WorkExhausted { .. }));
    assert_eq!(session.prepared_input(), before_input);
    assert_eq!(
        session.design_document().to_draft_v5_json().unwrap(),
        before_document
    );

    session
        .transact(session.design_identity(), |document| {
            document.add_point("newer", [20.0, 20.0])?;
            Ok(())
        })
        .unwrap();
    assert!(profile.validate_current(&session).is_err());
}

#[test]
fn tangency_and_t_junction_policies_reject_ambiguous_production_topology() {
    let mut tangent = SketchDocument::new(10.0).unwrap();
    for (index, center_position) in [[0.0, 0.0], [2.0, 0.0]].into_iter().enumerate() {
        let center = tangent
            .add_point(format!("center {index}"), center_position)
            .unwrap();
        let radius = tangent
            .add_scalar(
                format!("radius {index}"),
                1.0,
                ScalarUnit::Length,
                ScalarDomain::Positive,
            )
            .unwrap();
        tangent
            .add_curve(
                format!("circle {index}"),
                CurveDefinition::Circle { center, radius },
            )
            .unwrap();
    }
    let tangent = completed(&session(tangent), TopologyRequest::default());
    assert_eq!(tangent.completeness, TopologyCompleteness::Skipped);
    assert!(tangent.production_profile.is_none());
    assert!(
        tangent
            .issues
            .iter()
            .any(|issue| issue.kind == TopologyIssueKind::TangencyRejected)
    );

    let mut junction = SketchDocument::new(10.0).unwrap();
    add_square(&mut junction, "square", [0.0, 0.0], 4.0);
    let branch_start = junction.add_point("branch start", [2.0, 0.0]).unwrap();
    let branch_end = junction.add_point("branch end", [2.0, 2.0]).unwrap();
    add_line(&mut junction, "branch", branch_start, branch_end);
    let junction = completed(&session(junction), TopologyRequest::default());
    assert_ne!(junction.completeness, TopologyCompleteness::Complete);
    assert!(junction.production_profile.is_none());
    assert!(
        junction
            .issues
            .iter()
            .any(|issue| issue.kind == TopologyIssueKind::TJunctionRejected),
        "{:?}",
        junction.issues
    );
}

#[test]
fn output_limits_truncate_deterministically_and_repeated_queries_are_identical() {
    let mut document = SketchDocument::new(10.0).unwrap();
    let center = document.add_point("center", [0.0, 0.0]).unwrap();
    for (label, value) in [("outer", 4.0), ("inner", 2.0)] {
        let radius = document
            .add_scalar(
                format!("{label} radius"),
                value,
                ScalarUnit::Length,
                ScalarDomain::Positive,
            )
            .unwrap();
        document
            .add_curve(label, CurveDefinition::Circle { center, radius })
            .unwrap();
    }
    let session = session(document);
    let snapshot = TopologySnapshot::capture(&session).unwrap();
    let request = TopologyRequest {
        limits: TopologyLimits {
            max_wires: 1,
            ..TopologyLimits::default()
        },
        ..TopologyRequest::default()
    };
    let execute = |snapshot: TopologySnapshot| {
        snapshot
            .prepare(request.clone())
            .execute(OperationControl::default())
            .unwrap()
    };
    let first = execute(snapshot.clone());
    let second = execute(snapshot);
    assert_eq!(first, second);
    let OperationOutcome::Completed { value, .. } = first else {
        panic!("bounded topology query must complete");
    };
    assert_eq!(value.completeness, TopologyCompleteness::Truncated);
    assert!(value.production_profile.is_none());
    assert!(value.issues.iter().any(|issue| matches!(
        issue.kind,
        TopologyIssueKind::OutputWireLimitExceeded {
            required: 3,
            limit: 1
        }
    )));
}

#[test]
fn prepared_queries_and_immutable_outputs_have_safe_worker_ownership() {
    fn assert_send<T: Send>() {}
    fn assert_send_sync<T: Send + Sync>() {}

    assert_send_sync::<TopologySnapshot>();
    assert_send::<PreparedTopologyQuery>();
    assert_send_sync::<TopologyRequest>();
    assert_send_sync::<TopologyResult>();
    assert_send_sync::<TopologyProductionProfile>();

    let mut document = SketchDocument::new(10.0).unwrap();
    add_square(&mut document, "worker square", [0.0, 0.0], 4.0);
    let session = session(document);
    let prepared = TopologySnapshot::capture(&session)
        .unwrap()
        .prepare(TopologyRequest::default());
    let outcome = std::thread::spawn(move || prepared.execute(OperationControl::default()))
        .join()
        .unwrap()
        .unwrap();
    assert!(matches!(outcome, OperationOutcome::Completed { .. }));
}

#[test]
fn multi_interval_support_preserves_complete_production_provenance() {
    let mut document = SketchDocument::new(10.0).unwrap();
    let curves = add_square(&mut document, "split square", [0.0, 0.0], 4.0);
    let support = CurveSpan::line(curves[0]);
    let boundary = |parameter| {
        DocumentTrimBoundary::Fixed(DocumentTrimParameter {
            parameter,
            winding: 0,
        })
    };
    document
        .replace_trim_views(
            support,
            vec![
                DocumentCurveTrimView {
                    support,
                    start: boundary(0.0),
                    end: boundary(0.375),
                },
                DocumentCurveTrimView {
                    support,
                    start: boundary(0.375),
                    end: boundary(1.0),
                },
            ],
        )
        .unwrap();

    let result = completed(&session(document), TopologyRequest::default());
    assert_eq!(
        result.completeness,
        TopologyCompleteness::Complete,
        "{:?}",
        result.issues
    );
    let profile = result.production_profile.unwrap();
    assert_eq!(profile.regions().len(), 1);
    assert_eq!(profile.regions()[0].area, 16.0);
    let split_fragments = profile
        .wires()
        .iter()
        .flat_map(|wire| &wire.fragments)
        .filter(|fragment| {
            matches!(
                fragment.source,
                TopologySourceProvenance::Native {
                    support: fragment_support,
                    ..
                } if fragment_support == support
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(split_fragments.len(), 2);
    assert_eq!(split_fragments[0].source_parameters, [0.0, 0.375]);
    assert_eq!(split_fragments[1].source_parameters, [0.375, 1.0]);
}

#[test]
fn external_points_are_explicitly_ignored_without_tainting_native_profiles() {
    let mut document = SketchDocument::new(10.0).unwrap();
    add_square(&mut document, "native square", [0.0, 0.0], 4.0);
    let binding = document
        .add_external_binding("point datum", ExternalFeatureKindV1::Point, None)
        .unwrap();
    let snapshots = ExternalSnapshotSet::new(
        1,
        vec![ExternalSnapshotEntry {
            binding,
            source_revision: 3,
            source_digest: ExternalSnapshotDigest::from_bytes([3; 32]),
            feature: ExternalSnapshotFeatureV1::Point {
                position: [40.0, 40.0],
                scale: 1.0,
                resources: ExternalSnapshotResourcesV1 {
                    point_count: 1,
                    control_count: 0,
                    span_count: 0,
                },
            },
        }],
    )
    .unwrap();
    let session = RetainedSketchDocumentSession::new_with_inputs(
        document,
        ParameterBatch::default(),
        snapshots,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let result = completed(
        &session,
        TopologyRequest {
            external_geometry: TopologyExternalGeometryScope::IncludeLineSegments,
            ..TopologyRequest::default()
        },
    );
    assert_eq!(result.completeness, TopologyCompleteness::Complete);
    assert_eq!(result.scope.ignored_external_points, vec![binding]);
    assert!(result.scope.external_lines.is_empty());
    assert!(result.production_profile.is_some());
}

#[test]
fn retained_older_accepted_state_cannot_be_captured() {
    let mut document = SketchDocument::new(10.0).unwrap();
    let points = [
        document.add_point("first", [0.0, 0.0]).unwrap(),
        document.add_point("second", [1.0, 0.0]).unwrap(),
    ];
    add_line(&mut document, "line", points[0], points[1]);
    document
        .add_constraint(
            "fixed",
            geosolve_sketch::DocumentConstraintDefinition::FixedPoint {
                point: points[0],
                target: [0.0, 0.0],
            },
        )
        .unwrap();
    let mut session = session(document);
    session
        .transact(session.design_identity(), |document| {
            document.add_constraint(
                "conflict",
                geosolve_sketch::DocumentConstraintDefinition::FixedPoint {
                    point: points[0],
                    target: [2.0, 0.0],
                },
            )?;
            Ok(())
        })
        .unwrap();
    assert!(matches!(
        TopologySnapshot::capture(&session),
        Err(TopologySnapshotError::AcceptedStateForDifferentDesign)
    ));
}

#[test]
fn retained_accepted_state_from_older_host_input_cannot_be_captured() {
    let mut document = SketchDocument::new(10.0).unwrap();
    let rectangle = document
        .add_rectangle("parameterized rectangle", [0.0, 0.0], 4.0, 3.0)
        .unwrap();
    let width = document
        .add_parameter("width", DocumentParameterKind::Length)
        .unwrap();
    document
        .add_parameter_binding(
            width,
            DocumentParameterTarget::DrivingDimension(rectangle.dimensions[0]),
        )
        .unwrap();
    let initial = ParameterBatch::new(
        1,
        vec![ParameterBatchEntry {
            parameter: width,
            value: ParameterValue::Length(4.0),
        }],
    )
    .unwrap();
    let mut session = RetainedSketchDocumentSession::new_with_parameter_batch(
        document,
        initial,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    assert!(TopologySnapshot::capture(&session).is_ok());

    session
        .update_parameter_batch(
            session.design_identity(),
            ParameterBatch::new(2, Vec::new()).unwrap(),
            DocumentSolveRequest::default(),
        )
        .unwrap();
    assert!(session.last_attempt().failure().is_some());
    assert!(matches!(
        TopologySnapshot::capture(&session),
        Err(TopologySnapshotError::AcceptedInputMismatch)
    ));
}

#[test]
fn companion_manifest_has_only_accepted_one_way_workspace_dependencies() {
    let manifest = include_str!("../Cargo.toml");
    assert!(manifest.contains("geosolve-sketch ="));
    assert!(manifest.contains("geosolve-geometry ="));
    assert!(!manifest.contains("geosolve-core ="));
    assert!(!manifest.contains("geosolve-linkage ="));
    assert!(!manifest.contains("geosolve-sketch-ops ="));
    assert!(!manifest.contains("geosolve-demo-web ="));
}
