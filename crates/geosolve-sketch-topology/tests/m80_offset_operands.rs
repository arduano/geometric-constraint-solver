// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_sketch::{
    CancellationToken, ContactDomain, ContactNeighborhood, CurveDefinition, CurveSpan,
    DocumentArcSweep, DocumentBSplineForm, DocumentConstraintDefinition, DocumentCurveTrimView,
    DocumentEdit, DocumentHyperbolaBranch, DocumentSolveRequest, DocumentTrimBoundary,
    DocumentTrimParameter, GeometryRole, MIN_RATIONAL_QUADRATIC_MIDDLE_WEIGHT, OperationControl,
    OperationLimits, OperationOutcome, RetainedSketchDocumentSession, ScalarDomain, ScalarUnit,
    SketchDocument, SolverConfig, cancellation_pair,
};
use geosolve_sketch_topology::{
    OffsetEndpointEligibility, OffsetEndpointRef, OffsetEndpointRole, OffsetFaceLookup,
    OffsetJoinOwner, OffsetOperandCurveFamily, OffsetOperandEligibility, OffsetOperandIndex,
    OffsetOperandIneligibility, OffsetOperandRequest, OffsetOperandResult, OffsetTraversal,
    PreparedOffsetOperandQuery, TopologyCompleteness, TopologyLimits, TopologySnapshot,
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
    let points = [
        document.add_point(format!("{label}.p0"), origin).unwrap(),
        document
            .add_point(format!("{label}.p1"), [origin[0] + size, origin[1]])
            .unwrap(),
        document
            .add_point(format!("{label}.p2"), [origin[0] + size, origin[1] + size])
            .unwrap(),
        document
            .add_point(format!("{label}.p3"), [origin[0], origin[1] + size])
            .unwrap(),
    ];
    [
        add_line(document, &format!("{label}.e0"), points[0], points[1]),
        add_line(document, &format!("{label}.e1"), points[1], points[2]),
        add_line(document, &format!("{label}.e2"), points[2], points[3]),
        add_line(document, &format!("{label}.e3"), points[3], points[0]),
    ]
}

fn completed(
    session: &RetainedSketchDocumentSession,
    request: OffsetOperandRequest,
) -> OffsetOperandResult {
    let outcome = PreparedOffsetOperandQuery::capture(session, request)
        .unwrap()
        .execute(OperationControl::default())
        .unwrap();
    let OperationOutcome::Completed { value, .. } = outcome else {
        panic!("uncontrolled offset-operand query must complete");
    };
    value
}

fn disabled_for(
    eligibility: &OffsetOperandEligibility,
    reason: OffsetOperandIneligibility,
) -> bool {
    matches!(
        eligibility,
        OffsetOperandEligibility::Disabled { reasons } if reasons.contains(&reason)
    )
}

#[test]
fn offset_operand_queries_and_values_have_worker_safe_value_semantics() {
    fn assert_send<T: Send>() {}
    fn assert_send_sync<T: Send + Sync>() {}
    fn assert_clone_eq<T: Clone + PartialEq>() {}

    assert_send::<PreparedOffsetOperandQuery>();
    assert_send_sync::<OffsetOperandRequest>();
    assert_send_sync::<OffsetOperandResult>();
    assert_send_sync::<OffsetOperandIndex>();
    assert_clone_eq::<OffsetOperandResult>();
    assert_clone_eq::<OffsetOperandIndex>();
}

#[test]
fn successful_point_edit_remains_current_for_fresh_offset_operand_capture() {
    let mut document = SketchDocument::new(10.0).unwrap();
    add_square(&mut document, "square", [0.0, 0.0], 4.0);
    let probe = document.add_point("free probe", [8.0, 1.0]).unwrap();
    let mut session = session(document);

    let edit = session
        .apply(
            session.design_identity(),
            DocumentEdit::SetPointPosition {
                point: probe,
                position: [8.5, 1.25],
            },
        )
        .unwrap();
    assert!(edit.published_accepted_identity().is_some());
    let accepted = session
        .accepted_state_for_current_input()
        .expect("successful point edit remains current");
    assert_ne!(
        accepted.input().candidate_request(),
        session.prepared_input().attempt_input().candidate_request(),
        "one-shot point-edit guidance must remain accepted audit provenance only"
    );

    let query = PreparedOffsetOperandQuery::capture(&session, OffsetOperandRequest::default())
        .expect("current accepted point edit must admit a fresh Offset operand query");
    assert_eq!(query.input(), session.prepared_input());
    let OperationOutcome::Completed { value, .. } =
        query.execute(OperationControl::default()).unwrap()
    else {
        panic!("uncontrolled Offset operand query must complete");
    };
    let index = value
        .operand_index
        .expect("the edited current square remains an eligible face");
    index.validate_current(&session).unwrap();
}

#[test]
fn complete_index_retains_square_beside_open_and_unsupported_geometry() {
    let mut document = SketchDocument::new(10.0).unwrap();
    let square = add_square(&mut document, "square", [0.0, 0.0], 4.0);
    let open_start = document.add_point("open start", [10.0, 0.0]).unwrap();
    let open_end = document.add_point("open end", [14.0, 0.0]).unwrap();
    let open = add_line(&mut document, "open", open_start, open_end);
    let controls = [
        document.add_point("q0", [20.0, 0.0]).unwrap(),
        document.add_point("q1", [22.0, 2.0]).unwrap(),
        document.add_point("q2", [24.0, 0.0]).unwrap(),
    ];
    let unsupported = document
        .add_curve("quadratic", CurveDefinition::QuadraticBezier { controls })
        .unwrap();
    let session = session(document);

    let first = completed(&session, OffsetOperandRequest::default());
    let second = completed(&session, OffsetOperandRequest::default());
    assert_eq!(first.completeness, TopologyCompleteness::Complete);
    assert!(first.issues.is_empty());
    let first = first.operand_index.unwrap();
    let second = second.operand_index.unwrap();
    assert_eq!(first.faces(), second.faces());

    let eligible_faces = first
        .faces()
        .iter()
        .filter(|face| face.eligibility.is_eligible())
        .collect::<Vec<_>>();
    assert_eq!(eligible_faces.len(), 1);
    assert_eq!(eligible_faces[0].key.outer.spans.len(), 4);
    let minimum = square.into_iter().map(CurveSpan::line).min().unwrap();
    assert_eq!(eligible_faces[0].key.outer.spans[0].span, minimum);
    assert_eq!(
        eligible_faces[0].key.outer.spans[0].traversal,
        OffsetTraversal::Forward
    );

    assert!(
        first
            .span(CurveSpan::line(open))
            .unwrap()
            .eligibility
            .is_eligible()
    );
    let unsupported = first.span(CurveSpan::line(unsupported)).unwrap();
    assert_eq!(
        unsupported.family,
        OffsetOperandCurveFamily::QuadraticBezier
    );
    assert!(disabled_for(
        &unsupported.eligibility,
        OffsetOperandIneligibility::UnsupportedCurveFamily
    ));

    assert_eq!(
        first.face_at_point([2.0, 2.0]),
        OffsetFaceLookup::Hit(eligible_faces[0].key.clone())
    );
    assert_eq!(first.face_at_point([8.0, 8.0]), OffsetFaceLookup::None);
    assert!(matches!(
        first.face_at_point([0.0, 2.0]),
        OffsetFaceLookup::BoundaryAmbiguous { .. }
    ));
    first.validate_current(&session).unwrap();
}

#[test]
fn closed_polyline_publishes_one_eligible_face_with_four_semantic_spans() {
    let mut document = SketchDocument::new(10.0).unwrap();
    let points = [
        document.add_point("p0", [0.0, 0.0]).unwrap(),
        document.add_point("p1", [3.0, 0.0]).unwrap(),
        document.add_point("p2", [3.0, 2.0]).unwrap(),
        document.add_point("p3", [0.0, 2.0]).unwrap(),
    ];
    let polyline = document
        .add_curve(
            "polyline",
            CurveDefinition::Polyline {
                points: points.to_vec(),
                closed: true,
                branch_directions: vec![[1.0, 0.0], [0.0, 1.0], [-1.0, 0.0], [0.0, -1.0]],
            },
        )
        .unwrap();
    let result = completed(&session(document), OffsetOperandRequest::default());
    let index = result.operand_index.unwrap();
    let face = index
        .faces()
        .iter()
        .find(|face| face.eligibility.is_eligible())
        .expect("polyline face");
    assert_eq!(face.key.outer.spans.len(), 4);
    assert!(
        face.key
            .outer
            .spans
            .iter()
            .enumerate()
            .all(|(segment, edge)| {
                edge.span.curve == polyline && edge.span.segment == u32::try_from(segment).unwrap()
            })
    );
}

#[test]
fn full_circles_coalesce_to_one_semantic_span_and_lookup_respects_holes() {
    let mut document = SketchDocument::new(10.0).unwrap();
    let center = document.add_point("center", [0.0, 0.0]).unwrap();
    let mut circles = Vec::new();
    for (label, value) in [("outer", 4.0), ("inner", 2.0)] {
        let radius = document
            .add_scalar(label, value, ScalarUnit::Length, ScalarDomain::Positive)
            .unwrap();
        circles.push(
            document
                .add_curve(label, CurveDefinition::Circle { center, radius })
                .unwrap(),
        );
    }
    let result = completed(&session(document), OffsetOperandRequest::default());
    let index = result.operand_index.unwrap();
    assert_eq!(index.faces().len(), 2);
    assert!(index.faces().iter().all(|face| {
        face.key.outer.spans.len() == 1
            && face.key.holes.iter().all(|hole| hole.spans.len() == 1)
            && face.eligibility.is_eligible()
    }));
    for circle in circles {
        let span = index.span(CurveSpan::line(circle)).unwrap();
        assert!(span.periodic);
        assert!(span.endpoints.is_empty());
    }

    let annulus = match index.face_at_point([3.0, 0.0]) {
        OffsetFaceLookup::Hit(key) => key,
        other => panic!("expected annulus hit, got {other:?}"),
    };
    assert_eq!(annulus.holes.len(), 1);
    let disk = match index.face_at_point([0.0, 0.0]) {
        OffsetFaceLookup::Hit(key) => key,
        other => panic!("expected inner disk hit, got {other:?}"),
    };
    assert!(disk.holes.is_empty());
    assert!(matches!(
        index.face_at_point([2.0, 0.0]),
        OffsetFaceLookup::BoundaryAmbiguous { candidates } if candidates.len() == 2
    ));
}

#[test]
fn unsupported_closed_family_is_enumerated_as_a_disabled_face() {
    let mut document = SketchDocument::new(10.0).unwrap();
    let center = document.add_point("center", [0.0, 0.0]).unwrap();
    let major_axis_point = document.add_point("major", [4.0, 0.0]).unwrap();
    let ratio = document
        .add_scalar(
            "ratio",
            0.5,
            ScalarUnit::Parameter,
            ScalarDomain::Bounded {
                lower: f64::from_bits(1),
                upper: 1.0,
            },
        )
        .unwrap();
    let ellipse = document
        .add_curve(
            "ellipse",
            CurveDefinition::Ellipse {
                center,
                major_axis_point,
                minor_axis_ratio: ratio,
            },
        )
        .unwrap();
    let result = completed(&session(document), OffsetOperandRequest::default());
    let index = result.operand_index.unwrap();
    assert_eq!(index.faces().len(), 1);
    assert!(disabled_for(
        &index.faces()[0].eligibility,
        OffsetOperandIneligibility::UnsupportedCurveFamily
    ));
    let span = index.span(CurveSpan::line(ellipse)).unwrap();
    assert_eq!(span.family, OffsetOperandCurveFamily::Ellipse);
    assert!(disabled_for(
        &span.eligibility,
        OffsetOperandIneligibility::UnsupportedCurveFamily
    ));
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one exhaustive public-family matrix prevents any unsupported curve from acquiring an approximate Offset path"
)]
fn every_unsupported_native_curve_family_is_typed_and_disabled_without_approximation() {
    let mut document = SketchDocument::new(100.0).unwrap();
    let mut expected = Vec::new();

    let ellipse_center = document.add_point("ellipse center", [0.0, 0.0]).unwrap();
    let ellipse_axis = document.add_point("ellipse axis", [2.0, 0.0]).unwrap();
    let ellipse_ratio = document
        .add_scalar(
            "ellipse ratio",
            0.5,
            ScalarUnit::Parameter,
            ScalarDomain::Bounded {
                lower: f64::from_bits(1),
                upper: 1.0,
            },
        )
        .unwrap();
    let ellipse = document
        .add_curve(
            "ellipse",
            CurveDefinition::Ellipse {
                center: ellipse_center,
                major_axis_point: ellipse_axis,
                minor_axis_ratio: ellipse_ratio,
            },
        )
        .unwrap();
    expected.push((ellipse, OffsetOperandCurveFamily::Ellipse));

    let arc_center = document
        .add_point("elliptical arc center", [10.0, 0.0])
        .unwrap();
    let arc_axis = document
        .add_point("elliptical arc axis", [12.0, 0.0])
        .unwrap();
    let arc_ratio = document
        .add_scalar(
            "elliptical arc ratio",
            0.6,
            ScalarUnit::Parameter,
            ScalarDomain::Bounded {
                lower: f64::from_bits(1),
                upper: 1.0,
            },
        )
        .unwrap();
    let arc_start = document
        .add_scalar(
            "elliptical arc start",
            0.0,
            ScalarUnit::Angle,
            ScalarDomain::Finite,
        )
        .unwrap();
    let arc_end = document
        .add_scalar(
            "elliptical arc end",
            std::f64::consts::FRAC_PI_2,
            ScalarUnit::Angle,
            ScalarDomain::Finite,
        )
        .unwrap();
    let elliptical_arc = document
        .add_curve(
            "elliptical arc",
            CurveDefinition::EllipticalArc {
                center: arc_center,
                major_axis_point: arc_axis,
                minor_axis_ratio: arc_ratio,
                start_angle: arc_start,
                end_angle: arc_end,
                sweep: DocumentArcSweep::CounterClockwise,
            },
        )
        .unwrap();
    expected.push((elliptical_arc, OffsetOperandCurveFamily::EllipticalArc));

    let rational_start = document.add_point("rational start", [20.0, 0.0]).unwrap();
    let rational_end = document.add_point("rational end", [22.0, 0.0]).unwrap();
    let rational_weight = document
        .add_scalar(
            "rational weight",
            0.7,
            ScalarUnit::Parameter,
            ScalarDomain::Bounded {
                lower: MIN_RATIONAL_QUADRATIC_MIDDLE_WEIGHT,
                upper: f64::MAX,
            },
        )
        .unwrap();
    let rational = document
        .add_curve(
            "rational conic",
            CurveDefinition::RationalQuadraticConic {
                start: rational_start,
                weighted_middle: [14.7, 1.4],
                middle_weight: rational_weight,
                end: rational_end,
            },
        )
        .unwrap();
    expected.push((rational, OffsetOperandCurveFamily::RationalQuadraticConic));

    let vertex = document.add_point("parabola vertex", [30.0, 0.0]).unwrap();
    let focus = document.add_point("parabola focus", [30.0, 1.0]).unwrap();
    let parabola_start = document
        .add_scalar(
            "parabola start",
            -1.0,
            ScalarUnit::Parameter,
            ScalarDomain::Finite,
        )
        .unwrap();
    let parabola_end = document
        .add_scalar(
            "parabola end",
            1.0,
            ScalarUnit::Parameter,
            ScalarDomain::Finite,
        )
        .unwrap();
    let parabola = document
        .add_curve(
            "parabola",
            CurveDefinition::ParabolaSegment {
                vertex,
                focus,
                trim_start: parabola_start,
                trim_end: parabola_end,
            },
        )
        .unwrap();
    expected.push((parabola, OffsetOperandCurveFamily::Parabola));

    let hyperbola_center = document.add_point("hyperbola center", [40.0, 0.0]).unwrap();
    let hyperbola_axis = document.add_point("hyperbola axis", [42.0, 0.0]).unwrap();
    let semi_conjugate = document
        .add_scalar(
            "hyperbola semi conjugate",
            1.0,
            ScalarUnit::Length,
            ScalarDomain::Positive,
        )
        .unwrap();
    let hyperbola_start = document
        .add_scalar(
            "hyperbola start",
            -0.5,
            ScalarUnit::Parameter,
            ScalarDomain::Finite,
        )
        .unwrap();
    let hyperbola_end = document
        .add_scalar(
            "hyperbola end",
            0.5,
            ScalarUnit::Parameter,
            ScalarDomain::Finite,
        )
        .unwrap();
    let hyperbola = document
        .add_curve(
            "hyperbola",
            CurveDefinition::HyperbolaSegment {
                center: hyperbola_center,
                transverse_axis_point: hyperbola_axis,
                semi_conjugate,
                branch: DocumentHyperbolaBranch::Positive,
                trim_start: hyperbola_start,
                trim_end: hyperbola_end,
            },
        )
        .unwrap();
    expected.push((hyperbola, OffsetOperandCurveFamily::Hyperbola));

    let quadratic_controls = [
        document.add_point("quadratic start", [50.0, 0.0]).unwrap(),
        document.add_point("quadratic middle", [51.0, 1.0]).unwrap(),
        document.add_point("quadratic end", [52.0, 0.0]).unwrap(),
    ];
    let quadratic = document
        .add_curve(
            "quadratic bezier",
            CurveDefinition::QuadraticBezier {
                controls: quadratic_controls,
            },
        )
        .unwrap();
    expected.push((quadratic, OffsetOperandCurveFamily::QuadraticBezier));

    let cubic_controls = [
        document.add_point("cubic start", [60.0, 0.0]).unwrap(),
        document.add_point("cubic first", [60.5, 1.0]).unwrap(),
        document.add_point("cubic second", [61.5, -1.0]).unwrap(),
        document.add_point("cubic end", [62.0, 0.0]).unwrap(),
    ];
    let cubic = document
        .add_curve(
            "cubic bezier",
            CurveDefinition::CubicBezier {
                controls: cubic_controls,
            },
        )
        .unwrap();
    expected.push((cubic, OffsetOperandCurveFamily::CubicBezier));

    let spline_controls = vec![
        document.add_point("spline start", [70.0, 0.0]).unwrap(),
        document.add_point("spline end", [72.0, 0.5]).unwrap(),
    ];
    let spline = document
        .add_curve(
            "b spline",
            CurveDefinition::BSpline {
                form: DocumentBSplineForm::Clamped,
                degree: 1,
                controls: spline_controls,
                knots: vec![0.0, 0.0, 1.0, 1.0],
                span_ids: vec![0],
                next_span_id: 1,
            },
        )
        .unwrap();
    expected.push((spline, OffsetOperandCurveFamily::BSpline));

    let nurbs_controls = vec![
        document.add_point("nurbs start", [80.0, 0.0]).unwrap(),
        document.add_point("nurbs end", [82.0, 0.5]).unwrap(),
    ];
    let nurbs_weights = ["nurbs first weight", "nurbs second weight"]
        .map(|label| {
            document
                .add_scalar(label, 1.0, ScalarUnit::Parameter, ScalarDomain::Positive)
                .unwrap()
        })
        .to_vec();
    let nurbs = document
        .add_curve(
            "nurbs",
            CurveDefinition::Nurbs {
                form: DocumentBSplineForm::Clamped,
                degree: 1,
                controls: nurbs_controls,
                weights: nurbs_weights.clone(),
                gauge_weight: nurbs_weights[0],
                knots: vec![0.0, 0.0, 1.0, 1.0],
                span_ids: vec![0],
                next_span_id: 1,
            },
        )
        .unwrap();
    expected.push((nurbs, OffsetOperandCurveFamily::Nurbs));

    let result = completed(&session(document), OffsetOperandRequest::default());
    assert_eq!(result.completeness, TopologyCompleteness::Complete);
    let index = result.operand_index.expect("complete operand index");
    for (curve, family) in expected {
        let candidates = index
            .spans()
            .iter()
            .filter(|candidate| candidate.span.curve == curve)
            .collect::<Vec<_>>();
        assert!(!candidates.is_empty(), "missing {family:?} candidate");
        assert!(candidates.iter().all(|candidate| {
            candidate.family == family
                && disabled_for(
                    &candidate.eligibility,
                    OffsetOperandIneligibility::UnsupportedCurveFamily,
                )
        }));
    }
}

#[test]
fn arrangement_intersection_fragments_are_typed_and_never_become_complete_operands() {
    let mut document = SketchDocument::new(10.0).unwrap();
    let points = [
        document.add_point("first start", [-2.0, -2.0]).unwrap(),
        document.add_point("first end", [2.0, 2.0]).unwrap(),
        document.add_point("second start", [-2.0, 2.0]).unwrap(),
        document.add_point("second end", [2.0, -2.0]).unwrap(),
    ];
    let first = add_line(&mut document, "first", points[0], points[1]);
    let second = add_line(&mut document, "second", points[2], points[3]);

    let result = completed(&session(document), OffsetOperandRequest::default());
    assert_eq!(result.completeness, TopologyCompleteness::Complete);
    let index = result.operand_index.expect("complete operand index");
    for span in [CurveSpan::line(first), CurveSpan::line(second)] {
        let candidate = index.span(span).expect("intersected native span");
        assert_eq!(candidate.family, OffsetOperandCurveFamily::Line);
        assert!(disabled_for(
            &candidate.eligibility,
            OffsetOperandIneligibility::ArrangementDerivedFragment,
        ));
    }
    assert!(
        index
            .faces()
            .iter()
            .all(|face| !face.eligibility.is_eligible())
    );
}

#[test]
fn endpoint_adjacency_distinguishes_terminals_joins_and_branches() {
    let mut document = SketchDocument::new(10.0).unwrap();
    let points = [
        document.add_point("p0", [0.0, 0.0]).unwrap(),
        document.add_point("p1", [2.0, 0.0]).unwrap(),
        document.add_point("p2", [4.0, 0.0]).unwrap(),
        document.add_point("p3", [6.0, 0.0]).unwrap(),
        document.add_point("branch", [2.0, 2.0]).unwrap(),
    ];
    let first = add_line(&mut document, "first", points[0], points[1]);
    let second = add_line(&mut document, "second", points[1], points[2]);
    let third = add_line(&mut document, "third", points[2], points[3]);
    let branch = add_line(&mut document, "branch", points[1], points[4]);
    let result = completed(&session(document), OffsetOperandRequest::default());
    let index = result.operand_index.unwrap();

    let endpoint = |curve, role| OffsetEndpointRef {
        span: CurveSpan::line(curve),
        endpoint: role,
    };
    assert_eq!(
        index.span(CurveSpan::line(first)).unwrap().endpoints[0].eligibility,
        OffsetEndpointEligibility::Terminal
    );
    assert_eq!(
        index.span(CurveSpan::line(second)).unwrap().endpoints[1].eligibility,
        OffsetEndpointEligibility::Joined
    );
    assert_eq!(
        index.span(CurveSpan::line(first)).unwrap().endpoints[1].eligibility,
        OffsetEndpointEligibility::Branched { adjacent: 2 }
    );
    let peers = index
        .adjacent_endpoints(endpoint(first, OffsetEndpointRole::End))
        .collect::<Vec<_>>();
    assert_eq!(
        peers,
        vec![
            endpoint(second, OffsetEndpointRole::Start),
            endpoint(branch, OffsetEndpointRole::Start)
        ]
    );
    assert_eq!(
        index
            .adjacent_endpoints(endpoint(second, OffsetEndpointRole::End))
            .collect::<Vec<_>>(),
        vec![endpoint(third, OffsetEndpointRole::Start)]
    );
}

#[test]
fn supporting_line_contacts_at_endpoint_coordinates_do_not_own_offset_adjacency() {
    let mut document = SketchDocument::new(10.0).unwrap();
    let first_start = document.add_point("first start", [0.0, 0.0]).unwrap();
    let first_end = document.add_point("first end", [2.0, 0.0]).unwrap();
    let second_start = document.add_point("second start", [2.0, 0.0]).unwrap();
    let second_end = document.add_point("second end", [4.0, 0.0]).unwrap();
    let first = add_line(&mut document, "first", first_start, first_end);
    let second = add_line(&mut document, "second", second_start, second_end);
    let first_contact = document
        .add_curve_contact_with_domain(
            "first support sample",
            CurveSpan::line(first),
            ContactDomain::SupportingLine,
            1.0,
            0,
            ContactNeighborhood::Interior,
            None,
        )
        .unwrap();
    let second_contact = document
        .add_curve_contact_with_domain(
            "second support sample",
            CurveSpan::line(second),
            ContactDomain::SupportingLine,
            0.0,
            0,
            ContactNeighborhood::Interior,
            None,
        )
        .unwrap();
    document
        .add_constraint(
            "support-only coincidence",
            DocumentConstraintDefinition::CurveCurveContact {
                first_contact,
                second_contact,
            },
        )
        .unwrap();

    let result = completed(&session(document), OffsetOperandRequest::default());
    let index = result.operand_index.unwrap();
    assert!(index.adjacencies().is_empty());
    assert_eq!(
        index.span(CurveSpan::line(first)).unwrap().endpoints[1].eligibility,
        OffsetEndpointEligibility::Terminal
    );
    assert_eq!(
        index.span(CurveSpan::line(second)).unwrap().endpoints[0].eligibility,
        OffsetEndpointEligibility::Terminal
    );
}

#[test]
fn distinct_line_endpoints_retain_their_active_coincident_constraint_as_join_owner() {
    let mut document = SketchDocument::new(10.0).unwrap();
    let corners = [[0.0, 0.0], [4.0, 0.0], [4.0, 3.0], [0.0, 3.0]];
    let mut lines = Vec::new();
    let mut joins = Vec::new();
    let mut previous_end = None;
    let mut first_start = None;
    for edge in 0..4 {
        let start = document
            .add_point(format!("edge {edge} start"), corners[edge])
            .unwrap();
        let end = document
            .add_point(format!("edge {edge} end"), corners[(edge + 1) % 4])
            .unwrap();
        lines.push(add_line(&mut document, &format!("edge {edge}"), start, end));
        if let Some(previous_end) = previous_end {
            joins.push(
                document
                    .add_constraint(
                        format!("join {}", edge - 1),
                        DocumentConstraintDefinition::Coincident {
                            first: previous_end,
                            second: start,
                        },
                    )
                    .unwrap(),
            );
        } else {
            first_start = Some(start);
        }
        previous_end = Some(end);
    }
    joins.push(
        document
            .add_constraint(
                "join 3",
                DocumentConstraintDefinition::Coincident {
                    first: previous_end.unwrap(),
                    second: first_start.unwrap(),
                },
            )
            .unwrap(),
    );

    let result = completed(&session(document), OffsetOperandRequest::default());
    let index = result.operand_index.unwrap();
    let face = index
        .faces()
        .iter()
        .find(|face| face.eligibility.is_eligible())
        .expect("Coincident-connected square face");
    assert_eq!(face.key.outer.spans.len(), lines.len());
    assert_eq!(index.adjacencies().len(), joins.len());
    assert!(index.adjacencies().iter().all(|adjacency| {
        adjacency.owners.len() == 1
            && matches!(adjacency.owners[0], OffsetJoinOwner::Constraint(owner) if joins.contains(&owner))
    }));
}

#[test]
fn circular_arc_is_an_eligible_single_curve_with_two_free_terminals() {
    let mut document = SketchDocument::new(10.0).unwrap();
    let center = document.add_point("center", [0.0, 0.0]).unwrap();
    let radius = document
        .add_scalar("radius", 3.0, ScalarUnit::Length, ScalarDomain::Positive)
        .unwrap();
    let start = document
        .add_scalar("start", 0.0, ScalarUnit::Angle, ScalarDomain::Finite)
        .unwrap();
    let end = document
        .add_scalar(
            "end",
            std::f64::consts::FRAC_PI_2,
            ScalarUnit::Angle,
            ScalarDomain::Finite,
        )
        .unwrap();
    let arc = document
        .add_curve(
            "arc",
            CurveDefinition::CircularArc {
                center,
                radius,
                start_angle: start,
                end_angle: end,
                sweep: DocumentArcSweep::CounterClockwise,
            },
        )
        .unwrap();
    let result = completed(&session(document), OffsetOperandRequest::default());
    let index = result.operand_index.unwrap();
    let arc = index.span(CurveSpan::line(arc)).unwrap();
    assert_eq!(arc.family, OffsetOperandCurveFamily::CircularArc);
    assert!(arc.eligibility.is_eligible());
    assert_eq!(arc.endpoints.len(), 2);
    assert!(arc.endpoints.iter().all(|endpoint| {
        endpoint.eligibility == OffsetEndpointEligibility::Terminal
            && endpoint.position.into_iter().all(f64::is_finite)
    }));
}

#[test]
fn mixed_line_arc_face_uses_explicit_endpoint_ownership_and_exact_lookup() {
    let mut document = SketchDocument::new(10.0).unwrap();
    let center = document.add_point("center", [0.0, 0.0]).unwrap();
    let radius = document
        .add_scalar("radius", 2.0, ScalarUnit::Length, ScalarDomain::Positive)
        .unwrap();
    let start = document
        .add_scalar(
            "start",
            -std::f64::consts::FRAC_PI_2,
            ScalarUnit::Angle,
            ScalarDomain::Finite,
        )
        .unwrap();
    let end = document
        .add_scalar(
            "end",
            std::f64::consts::FRAC_PI_2,
            ScalarUnit::Angle,
            ScalarDomain::Finite,
        )
        .unwrap();
    let arc = document
        .add_curve(
            "arc",
            CurveDefinition::CircularArc {
                center,
                radius,
                start_angle: start,
                end_angle: end,
                sweep: DocumentArcSweep::CounterClockwise,
            },
        )
        .unwrap();
    let top = document.add_point("top", [0.0, 2.0]).unwrap();
    let bottom = document.add_point("bottom", [0.0, -2.0]).unwrap();
    let line = add_line(&mut document, "diameter", top, bottom);
    let arc_span = CurveSpan::line(arc);
    let arc_end = document
        .add_curve_contact("arc end", arc_span, 1.0, 0, ContactNeighborhood::End, None)
        .unwrap();
    let arc_start = document
        .add_curve_contact(
            "arc start",
            arc_span,
            0.0,
            0,
            ContactNeighborhood::Start,
            None,
        )
        .unwrap();
    document
        .add_constraint(
            "top join",
            DocumentConstraintDefinition::PointOnCurve {
                point: top,
                contact: arc_end,
            },
        )
        .unwrap();
    document
        .add_constraint(
            "bottom join",
            DocumentConstraintDefinition::PointOnCurve {
                point: bottom,
                contact: arc_start,
            },
        )
        .unwrap();

    let result = completed(&session(document), OffsetOperandRequest::default());
    let index = result.operand_index.unwrap();
    let face = index
        .faces()
        .iter()
        .find(|face| face.eligibility.is_eligible())
        .expect("mixed line/arc face");
    assert_eq!(face.key.outer.spans.len(), 2);
    assert!(
        face.key
            .outer
            .spans
            .iter()
            .any(|edge| edge.span == CurveSpan::line(line))
    );
    assert!(
        face.key
            .outer
            .spans
            .iter()
            .any(|edge| edge.span == arc_span)
    );
    assert_eq!(
        index.face_at_point([1.0, 0.0]),
        OffsetFaceLookup::Hit(face.key.clone())
    );
    assert_eq!(index.face_at_point([-1.0, 0.0]), OffsetFaceLookup::None);
    assert!(matches!(
        index.face_at_point([2.0, 0.0]),
        OffsetFaceLookup::BoundaryAmbiguous { .. }
    ));
}

#[test]
fn trimmed_and_non_profile_spans_remain_visible_with_typed_disabled_reasons() {
    let mut document = SketchDocument::new(10.0).unwrap();
    let points = [
        document.add_point("p0", [0.0, 0.0]).unwrap(),
        document.add_point("p1", [4.0, 0.0]).unwrap(),
        document.add_point("p2", [0.0, 3.0]).unwrap(),
        document.add_point("p3", [4.0, 3.0]).unwrap(),
    ];
    let trimmed = add_line(&mut document, "trimmed", points[0], points[1]);
    let construction = add_line(&mut document, "construction", points[2], points[3]);
    document
        .replace_trim_views(
            CurveSpan::line(trimmed),
            vec![DocumentCurveTrimView {
                support: CurveSpan::line(trimmed),
                start: DocumentTrimBoundary::Fixed(DocumentTrimParameter {
                    parameter: 0.0,
                    winding: 0,
                }),
                end: DocumentTrimBoundary::Fixed(DocumentTrimParameter {
                    parameter: 0.5,
                    winding: 0,
                }),
            }],
        )
        .unwrap();
    document
        .set_geometry_role(construction, GeometryRole::Construction)
        .unwrap();
    let result = completed(&session(document), OffsetOperandRequest::default());
    let index = result.operand_index.unwrap();
    assert!(disabled_for(
        &index.span(CurveSpan::line(trimmed)).unwrap().eligibility,
        OffsetOperandIneligibility::TrimmedOrPartialSpan
    ));
    assert!(disabled_for(
        &index
            .span(CurveSpan::line(construction))
            .unwrap()
            .eligibility,
        OffsetOperandIneligibility::NonProfileGeometry
    ));
}

#[test]
fn incomplete_cancelled_and_exhausted_queries_publish_no_operand_prefix() {
    let mut document = SketchDocument::new(10.0).unwrap();
    let center = document.add_point("center", [0.0, 0.0]).unwrap();
    for value in [2.0, 4.0] {
        let radius = document
            .add_scalar("radius", value, ScalarUnit::Length, ScalarDomain::Positive)
            .unwrap();
        document
            .add_curve("circle", CurveDefinition::Circle { center, radius })
            .unwrap();
    }
    let accepted_session = session(document);
    let incomplete = completed(
        &accepted_session,
        OffsetOperandRequest {
            limits: TopologyLimits {
                max_regions: 1,
                ..TopologyLimits::default()
            },
        },
    );
    assert_ne!(incomplete.completeness, TopologyCompleteness::Complete);
    assert!(incomplete.operand_index.is_none());

    let mut skipped_document = SketchDocument::new(10.0).unwrap();
    for (label, center_position) in [
        ("tangent first", [10.0, 0.0]),
        ("tangent second", [12.0, 0.0]),
    ] {
        let center = skipped_document
            .add_point(format!("{label} center"), center_position)
            .unwrap();
        let radius = skipped_document
            .add_scalar(
                format!("{label} radius"),
                1.0,
                ScalarUnit::Length,
                ScalarDomain::Positive,
            )
            .unwrap();
        skipped_document
            .add_curve(label, CurveDefinition::Circle { center, radius })
            .unwrap();
    }
    let skipped = completed(&session(skipped_document), OffsetOperandRequest::default());
    assert_eq!(skipped.completeness, TopologyCompleteness::Skipped);
    assert!(skipped.operand_index.is_none());

    let (handle, token) = cancellation_pair();
    handle.cancel();
    let cancelled = TopologySnapshot::capture(&accepted_session)
        .unwrap()
        .prepare_offset_operands(OffsetOperandRequest::default())
        .execute(OperationControl::new(token, OperationLimits::unlimited()))
        .unwrap();
    assert!(matches!(cancelled, OperationOutcome::Cancelled { .. }));

    let mut limits = OperationLimits::unlimited();
    limits.profile_fragments = 0;
    let exhausted = TopologySnapshot::capture(&accepted_session)
        .unwrap()
        .prepare_offset_operands(OffsetOperandRequest::default())
        .execute(OperationControl::new(CancellationToken::default(), limits))
        .unwrap();
    assert!(matches!(exhausted, OperationOutcome::WorkExhausted { .. }));
}

#[test]
fn accepted_input_transition_stales_the_complete_operand_index() {
    let mut document = SketchDocument::new(10.0).unwrap();
    add_square(&mut document, "square", [0.0, 0.0], 4.0);
    let mut session = session(document);
    let index = completed(&session, OffsetOperandRequest::default())
        .operand_index
        .unwrap();
    session
        .transact(session.design_identity(), |document| {
            document.add_point("new accepted point", [20.0, 20.0])?;
            Ok(())
        })
        .unwrap();
    assert!(index.validate_current(&session).is_err());
}
