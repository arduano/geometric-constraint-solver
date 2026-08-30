// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_sketch::{
    CurveDefinition, CurveSpan, DesignPointId, DocumentConstraintDefinition, DocumentEdit,
    DocumentElementId, DocumentSolveRequest, OperationControl, OperationOutcome,
    RetainedSketchDocumentSession, ScalarDomain, ScalarUnit, SketchDocument, SolverConfig,
};
use geosolve_sketch_topology::{
    EndpointTopologyConsumptionError, EndpointTopologyError, EndpointTopologyIndex,
    EndpointTopologyRequest, EndpointTopologySpan, OffsetEndpointAdjacency,
    OffsetEndpointEligibility, OffsetEndpointRef, OffsetEndpointRole, OffsetJoinOwner,
    OffsetOperandRequest, PreparedEndpointTopologyQuery, PreparedOffsetOperandQuery,
    TopologyCompleteness,
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
    start: DesignPointId,
    end: DesignPointId,
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

fn add_circle(document: &mut SketchDocument, label: &str, center: [f64; 2], radius: f64) {
    let center = document
        .add_point(format!("{label} center"), center)
        .unwrap();
    let radius = document
        .add_scalar(
            format!("{label} radius"),
            radius,
            ScalarUnit::Length,
            ScalarDomain::Positive,
        )
        .unwrap();
    document
        .add_curve(label, CurveDefinition::Circle { center, radius })
        .unwrap();
}

fn endpoint(curve: geosolve_sketch::CurveId, endpoint: OffsetEndpointRole) -> OffsetEndpointRef {
    OffsetEndpointRef {
        span: CurveSpan::line(curve),
        endpoint,
    }
}

fn completed_endpoint_index(session: &RetainedSketchDocumentSession) -> EndpointTopologyIndex {
    PreparedEndpointTopologyQuery::capture(session, EndpointTopologyRequest::default())
        .unwrap()
        .execute()
        .unwrap()
}

fn completed_offset_index(
    session: &RetainedSketchDocumentSession,
) -> geosolve_sketch_topology::OffsetOperandIndex {
    let outcome = PreparedOffsetOperandQuery::capture(session, OffsetOperandRequest::default())
        .unwrap()
        .execute(OperationControl::default())
        .unwrap();
    let OperationOutcome::Completed { value, .. } = outcome else {
        panic!("uncontrolled offset query must complete");
    };
    value.operand_index.expect("complete offset operand index")
}

#[test]
fn endpoint_topology_queries_have_worker_safe_value_semantics() {
    fn assert_send<T: Send>() {}
    fn assert_send_sync<T: Send + Sync>() {}
    fn assert_clone_eq<T: Clone + PartialEq>() {}

    assert_send::<PreparedEndpointTopologyQuery>();
    assert_send_sync::<EndpointTopologyRequest>();
    assert_send_sync::<EndpointTopologyIndex>();
    assert_send_sync::<EndpointTopologySpan>();
    assert_clone_eq::<EndpointTopologyIndex>();
}

#[test]
fn shared_points_define_canonical_topology_without_coordinate_welding() {
    let mut document = SketchDocument::new(10.0).unwrap();
    let first_start = document.add_point("first start", [0.0, 0.0]).unwrap();
    let shared = document.add_point("shared", [2.0, 0.0]).unwrap();
    let second_end = document.add_point("second end", [4.0, 0.0]).unwrap();
    let coordinate_only = document
        .add_point("coordinate-only start", [2.0, 0.0])
        .unwrap();
    let coordinate_only_end = document
        .add_point("coordinate-only end", [2.0, 2.0])
        .unwrap();
    let first = add_line(&mut document, "first", first_start, shared);
    let second = add_line(&mut document, "second", shared, second_end);
    let coordinate_only_curve = add_line(
        &mut document,
        "coordinate-only",
        coordinate_only,
        coordinate_only_end,
    );
    let session = session(document);

    let first_index = completed_endpoint_index(&session);
    let second_index = completed_endpoint_index(&session);
    assert_eq!(first_index, second_index);
    assert_eq!(first_index.adjacencies().len(), 1);
    assert_eq!(
        first_index.adjacencies()[0],
        OffsetEndpointAdjacency {
            endpoints: [
                endpoint(first, OffsetEndpointRole::End),
                endpoint(second, OffsetEndpointRole::Start),
            ],
            owners: vec![OffsetJoinOwner::SharedPoint(shared)],
        }
    );
    assert_eq!(
        first_index
            .adjacent_endpoints(endpoint(first, OffsetEndpointRole::End))
            .collect::<Vec<_>>(),
        vec![endpoint(second, OffsetEndpointRole::Start)]
    );
    let coordinate_only = first_index
        .span(CurveSpan::line(coordinate_only_curve))
        .unwrap()
        .endpoints
        .iter()
        .find(|candidate| candidate.endpoint.endpoint == OffsetEndpointRole::Start)
        .unwrap();
    assert_eq!(
        coordinate_only.eligibility,
        OffsetEndpointEligibility::Terminal
    );
}

#[test]
fn endpoint_topology_remains_available_when_visual_profiles_are_incomplete() {
    let mut document = SketchDocument::new(10.0).unwrap();
    let start = document.add_point("route start", [0.0, 0.0]).unwrap();
    let join = document.add_point("route join", [2.0, 0.0]).unwrap();
    let end = document.add_point("route end", [4.0, 0.0]).unwrap();
    let first = add_line(&mut document, "route first", start, join);
    let second = add_line(&mut document, "route second", join, end);
    add_circle(&mut document, "tangent first", [10.0, 0.0], 1.0);
    add_circle(&mut document, "tangent second", [12.0, 0.0], 1.0);
    let session = session(document);

    let outcome = PreparedOffsetOperandQuery::capture(&session, OffsetOperandRequest::default())
        .unwrap()
        .execute(OperationControl::default())
        .unwrap();
    let OperationOutcome::Completed { value, .. } = outcome else {
        panic!("uncontrolled offset query must complete");
    };
    assert_ne!(value.completeness, TopologyCompleteness::Complete);
    assert!(value.operand_index.is_none());

    let exact = completed_endpoint_index(&session);
    assert_eq!(exact.adjacencies().len(), 1);
    assert_eq!(
        exact
            .adjacent_endpoints(endpoint(first, OffsetEndpointRole::End))
            .collect::<Vec<_>>(),
        vec![endpoint(second, OffsetEndpointRole::Start)]
    );
}

#[test]
fn endpoint_projection_matches_complete_offset_operand_projection() {
    let mut document = SketchDocument::new(10.0).unwrap();
    let start = document.add_point("start", [0.0, 0.0]).unwrap();
    let join = document.add_point("join", [2.0, 0.0]).unwrap();
    let end = document.add_point("end", [4.0, 0.0]).unwrap();
    let first = add_line(&mut document, "first", start, join);
    let second = add_line(&mut document, "second", join, end);
    let session = session(document);

    let exact = completed_endpoint_index(&session);
    let offset = completed_offset_index(&session);
    assert_eq!(exact.adjacencies(), offset.adjacencies());
    for span in [CurveSpan::line(first), CurveSpan::line(second)] {
        let exact_span = exact.span(span).unwrap();
        let offset_span = offset.span(span).unwrap();
        assert_eq!(exact_span.periodic, offset_span.periodic);
        assert_eq!(exact_span.endpoints, offset_span.endpoints);
    }
}

#[test]
fn constraint_activity_transitions_stale_and_rebuild_exact_joins() {
    let mut document = SketchDocument::new(10.0).unwrap();
    let first_start = document.add_point("first start", [0.0, 0.0]).unwrap();
    let first_end = document.add_point("first end", [2.0, 0.0]).unwrap();
    let second_start = document.add_point("second start", [2.0, 0.0]).unwrap();
    let second_end = document.add_point("second end", [4.0, 0.0]).unwrap();
    let first = add_line(&mut document, "first", first_start, first_end);
    let second = add_line(&mut document, "second", second_start, second_end);
    let join = document
        .add_constraint(
            "owned join",
            DocumentConstraintDefinition::Coincident {
                first: first_end,
                second: second_start,
            },
        )
        .unwrap();
    let mut session = session(document);

    let active = completed_endpoint_index(&session);
    active.validate_current(&session).unwrap();
    assert_eq!(active.adjacencies().len(), 1);
    assert_eq!(
        active.adjacencies()[0].owners,
        vec![OffsetJoinOwner::Constraint(join)]
    );

    session
        .transact(session.design_identity(), |document| {
            document.set_element_user_suppressed(DocumentElementId::Constraint(join), true)
        })
        .unwrap();
    assert_eq!(
        active.validate_current(&session),
        Err(EndpointTopologyConsumptionError::Stale)
    );
    let suppressed = completed_endpoint_index(&session);
    suppressed.validate_current(&session).unwrap();
    assert!(suppressed.adjacencies().is_empty());
    for endpoint in [
        endpoint(first, OffsetEndpointRole::End),
        endpoint(second, OffsetEndpointRole::Start),
    ] {
        let candidate = suppressed
            .span(endpoint.span)
            .unwrap()
            .endpoints
            .iter()
            .find(|candidate| candidate.endpoint == endpoint)
            .unwrap();
        assert_eq!(candidate.eligibility, OffsetEndpointEligibility::Terminal);
    }

    session
        .transact(session.design_identity(), |document| {
            document.set_element_user_suppressed(DocumentElementId::Constraint(join), false)
        })
        .unwrap();
    assert_eq!(
        suppressed.validate_current(&session),
        Err(EndpointTopologyConsumptionError::Stale)
    );
    let restored = completed_endpoint_index(&session);
    restored.validate_current(&session).unwrap();
    assert_eq!(
        restored.adjacencies()[0].owners,
        vec![OffsetJoinOwner::Constraint(join)]
    );
}

#[test]
fn successful_point_edit_and_limits_remain_fail_closed() {
    let mut document = SketchDocument::new(10.0).unwrap();
    let center = document.add_point("center", [0.0, 0.0]).unwrap();
    let ends = [
        document.add_point("east", [2.0, 0.0]).unwrap(),
        document.add_point("north", [0.0, 2.0]).unwrap(),
        document.add_point("west", [-2.0, 0.0]).unwrap(),
    ];
    for (index, end) in ends.into_iter().enumerate() {
        add_line(&mut document, &format!("ray {index}"), center, end);
    }
    let probe = document.add_point("probe", [5.0, 5.0]).unwrap();
    let mut session = session(document);

    let limited = PreparedEndpointTopologyQuery::capture(
        &session,
        EndpointTopologyRequest {
            max_spans: 10,
            max_adjacencies: 2,
            max_connection_attempts: 10,
        },
    )
    .unwrap()
    .execute();
    assert_eq!(
        limited,
        Err(EndpointTopologyError::ResourceLimit {
            resource: "endpoint topology adjacencies",
            actual: 3,
            limit: 2,
        })
    );
    let work_limited = PreparedEndpointTopologyQuery::capture(
        &session,
        EndpointTopologyRequest {
            max_spans: 10,
            max_adjacencies: 10,
            max_connection_attempts: 2,
        },
    )
    .unwrap()
    .execute();
    assert_eq!(
        work_limited,
        Err(EndpointTopologyError::ResourceLimit {
            resource: "endpoint topology connection attempts",
            actual: 3,
            limit: 2,
        })
    );

    let before = completed_endpoint_index(&session);
    let edit = session
        .apply(
            session.design_identity(),
            DocumentEdit::SetPointPosition {
                point: probe,
                position: [5.5, 5.25],
            },
        )
        .unwrap();
    assert!(edit.published_accepted_identity().is_some());
    assert_eq!(
        before.validate_current(&session),
        Err(EndpointTopologyConsumptionError::Stale)
    );
    let fresh =
        PreparedEndpointTopologyQuery::capture(&session, EndpointTopologyRequest::default())
            .unwrap();
    assert_eq!(fresh.input(), session.prepared_input());
    fresh.execute().unwrap().validate_current(&session).unwrap();
}
