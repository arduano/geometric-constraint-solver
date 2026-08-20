// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_sketch::{
    CurveDefinition, CurveSpan, DocumentArcSweep, DocumentCurveNormalSide,
    DocumentFilletEndpointOrder, DocumentFilletTrimEndpoint, DocumentSolveRequest,
    OperationControl, OperationOutcome, RetainedSketchDocumentSession, ScalarDomain, ScalarUnit,
    SketchDocument, SketchHardValidity, SolverConfig,
};
use geosolve_sketch_features::{
    ComputedCircularArc, ComputedEdgeGeometry, ComputedEvaluationAllocator,
    ComputedFeatureDefinition, ComputedFeatureDocument, ComputedFeatureEvaluationPolicy,
    ComputedFeatureEvaluationSnapshot, ComputedFeatureEvaluationState, ComputedFeatureId,
    ComputedFeatureSnapshot, ComputedFilletParent, NativeCurveSpanSource, NewComputedFilletCorner,
};

const ARC_START: f64 = -1.396_263_401_595_463_6;
const ARC_END: f64 = 1.396_263_401_595_463_6;
const STALE_LOWER: f64 = 0.45;
const STALE_UPPER: f64 = 0.55;
const VALID_MOVED_LINE_Y: f64 = 1.7;
const ENDPOINT_BLOCKED_LINE_Y: f64 = 1.97;

struct CircularArcLineFixture {
    session: RetainedSketchDocumentSession,
    features: ComputedFeatureDocument,
    feature: ComputedFeatureId,
    arc: CurveSpan,
    line: CurveSpan,
    line_points: [geosolve_sketch::DesignPointId; 2],
    persistent_corner: NewComputedFilletCorner,
}

fn complete<T: std::fmt::Debug>(outcome: OperationOutcome<T>) -> T {
    match outcome {
        OperationOutcome::Completed { value, .. } => value,
        other => panic!("expected completed operation, got {other:?}"),
    }
}

fn source(span: CurveSpan) -> NativeCurveSpanSource {
    NativeCurveSpanSource { span }
}

#[allow(
    clippy::too_many_lines,
    reason = "the public fixture keeps the finite arc, affine parent and exact persistent branch together"
)]
fn fixture(reverse_parents: bool) -> CircularArcLineFixture {
    let mut document = SketchDocument::new(10.0).expect("document");
    let center = document.add_point("arc center", [0.0, 0.0]).unwrap();
    let radius = document
        .add_scalar(
            "arc radius",
            2.0,
            ScalarUnit::Length,
            ScalarDomain::Positive,
        )
        .unwrap();
    let start_angle = document
        .add_scalar(
            "arc start",
            ARC_START,
            ScalarUnit::Angle,
            ScalarDomain::Finite,
        )
        .unwrap();
    let end_angle = document
        .add_scalar("arc end", ARC_END, ScalarUnit::Angle, ScalarDomain::Finite)
        .unwrap();
    let arc = CurveSpan::line(
        document
            .add_curve(
                "finite circular arc",
                CurveDefinition::CircularArc {
                    center,
                    radius,
                    start_angle,
                    end_angle,
                    sweep: DocumentArcSweep::CounterClockwise,
                },
            )
            .unwrap(),
    );
    let line_points = [
        document.add_point("line start", [-5.0, -1.0]).unwrap(),
        document.add_point("line end", [5.0, -1.0]).unwrap(),
    ];
    let line = CurveSpan::line(
        document
            .add_curve(
                "affine line",
                CurveDefinition::Line {
                    start: line_points[0],
                    end: line_points[1],
                    branch_direction: [1.0, 0.0],
                },
            )
            .unwrap(),
    );

    let curved = ComputedFilletParent {
        source: source(arc),
        picked_parameter: 0.5,
        winding: 0,
        neighborhood: geosolve_sketch::ContactNeighborhood::Local {
            lower: STALE_LOWER,
            upper: STALE_UPPER,
        },
        normal_side: DocumentCurveNormalSide::Right,
        retained_endpoint: DocumentFilletTrimEndpoint::End,
        periodic_anchor: None,
    };
    let affine = ComputedFilletParent {
        source: source(line),
        picked_parameter: 0.8,
        winding: 0,
        neighborhood: geosolve_sketch::ContactNeighborhood::Interior,
        normal_side: DocumentCurveNormalSide::Left,
        retained_endpoint: DocumentFilletTrimEndpoint::Start,
        periodic_anchor: None,
    };
    let persistent_corner = if reverse_parents {
        NewComputedFilletCorner {
            first: affine,
            second: curved,
            endpoint_order: DocumentFilletEndpointOrder::SecondThenFirst,
            sweep: DocumentArcSweep::CounterClockwise,
        }
    } else {
        NewComputedFilletCorner {
            first: curved,
            second: affine,
            endpoint_order: DocumentFilletEndpointOrder::FirstThenSecond,
            sweep: DocumentArcSweep::CounterClockwise,
        }
    };
    let mut features = ComputedFeatureDocument::new(document.id());
    let feature = features
        .create_fillet_set("arc-line Fillet", 1.0, vec![persistent_corner])
        .unwrap();
    let session = RetainedSketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    CircularArcLineFixture {
        session,
        features,
        feature,
        arc,
        line,
        line_points,
        persistent_corner,
    }
}

fn move_line(fixture: &mut CircularArcLineFixture, y: f64) {
    let expected = fixture.session.design_identity();
    let transaction = fixture
        .session
        .transact(expected, |document| {
            document.set_point_position(fixture.line_points[0], [-5.0, y])?;
            document.set_point_position(fixture.line_points[1], [5.0, y])?;
            Ok(())
        })
        .unwrap();
    assert!(transaction.published_accepted_identity().is_some());
    let accepted = fixture
        .session
        .accepted_state_for_current_input()
        .expect("moved native geometry is accepted");
    assert_eq!(
        accepted.diagnostics().solve.unwrap().hard_validity,
        SketchHardValidity::Valid
    );
    assert!(
        accepted
            .document()
            .points()
            .iter()
            .all(|point| { point.position[0].is_finite() && point.position[1].is_finite() })
    );
}

fn evaluate(fixture: &CircularArcLineFixture) -> ComputedFeatureSnapshot {
    let mut allocator = ComputedEvaluationAllocator::default();
    complete(
        ComputedFeatureEvaluationSnapshot::capture(
            &fixture.session,
            &fixture.features,
            ComputedFeatureEvaluationPolicy::default(),
        )
        .unwrap()
        .prepare(&mut allocator)
        .unwrap()
        .execute(OperationControl::unlimited())
        .unwrap(),
    )
}

fn current_arc<'a>(
    fixture: &CircularArcLineFixture,
    snapshot: &'a ComputedFeatureSnapshot,
) -> &'a ComputedCircularArc {
    let evaluation = snapshot
        .feature_evaluations()
        .iter()
        .find(|evaluation| evaluation.feature == fixture.feature)
        .expect("feature evaluation");
    let ComputedFeatureEvaluationState::Current { corner_edges, .. } = &evaluation.state else {
        panic!(
            "expected Current arc-line Fillet, got {:?}",
            evaluation.state
        )
    };
    assert_eq!(corner_edges.len(), 1);
    let edge = snapshot.edge(corner_edges[0].1).expect("generated edge");
    let ComputedEdgeGeometry::CircularArc(arc) = &edge.geometry else {
        panic!("Fillet output is not a circular arc")
    };
    arc
}

fn assert_independent_arc_invariants(
    fixture: &CircularArcLineFixture,
    generated: &ComputedCircularArc,
) -> ([f64; 2], [f64; 2]) {
    assert!(
        generated.center.into_iter().all(f64::is_finite)
            && generated.radius.is_finite()
            && generated.start_angle.is_finite()
            && generated.end_angle.is_finite()
    );
    assert_eq!(generated.radius.to_bits(), 1.0_f64.to_bits());
    assert_eq!(generated.sweep, fixture.persistent_corner.sweep);

    let mut arc_contact = None;
    let mut line_contact = None;
    for contact in generated.contacts {
        let parent = [
            fixture.persistent_corner.first,
            fixture.persistent_corner.second,
        ]
        .into_iter()
        .find(|parent| parent.source == contact.source)
        .expect("contact has a persistent parent");
        assert_eq!(contact.winding, 0);
        assert!(
            contact.parameter.is_finite()
                && contact.total_parameter.is_finite()
                && contact.position.into_iter().all(f64::is_finite)
        );
        let accepted = fixture
            .session
            .accepted_state_for_current_input()
            .unwrap()
            .document();
        let jet = accepted
            .evaluate_curve_jet(parent.source.span, contact.total_parameter)
            .unwrap();
        assert!(
            (jet.position.x - contact.position[0]).hypot(jet.position.y - contact.position[1])
                <= 1.0e-9
        );
        let radial = [
            generated.center[0] - contact.position[0],
            generated.center[1] - contact.position[1],
        ];
        let radial_length = radial[0].hypot(radial[1]);
        assert!((radial_length - generated.radius).abs() <= 1.0e-9);
        let tangent_length = jet.first_derivative.x.hypot(jet.first_derivative.y);
        let left_normal = [
            -jet.first_derivative.y / tangent_length,
            jet.first_derivative.x / tangent_length,
        ];
        let signed_offset = radial[0].mul_add(left_normal[0], radial[1] * left_normal[1]);
        let expected_offset = match parent.normal_side {
            DocumentCurveNormalSide::Left => generated.radius,
            DocumentCurveNormalSide::Right => -generated.radius,
        };
        assert!((signed_offset - expected_offset).abs() <= 1.0e-9);
        let normalized_tangency = jet
            .first_derivative
            .x
            .mul_add(radial[0], jet.first_derivative.y * radial[1])
            .abs()
            / (tangent_length * radial_length);
        assert!(normalized_tangency <= 1.0e-9);

        if contact.source == source(fixture.arc) {
            assert!(
                STALE_UPPER < contact.total_parameter && contact.total_parameter < 1.0,
                "the arc contact did not cross the stale certificate edge"
            );
            arc_contact = Some(contact.position);
        } else {
            assert_eq!(contact.source, source(fixture.line));
            assert!(0.0 < contact.total_parameter && contact.total_parameter < 1.0);
            line_contact = Some(contact.position);
        }
    }
    (
        arc_contact.expect("arc contact"),
        line_contact.expect("line contact"),
    )
}

#[test]
fn circular_arc_transport_crosses_stale_cell_and_stops_at_endpoint_in_both_orders() {
    let mut order_results = Vec::new();
    for reverse_parents in [false, true] {
        let mut fixture = fixture(reverse_parents);
        let feature_json = fixture.features.to_json().unwrap();
        let feature_identity = fixture.features.identity();

        assert!(matches!(
            fixture
                .features
                .feature(fixture.feature)
                .expect("persistent Fillet")
                .definition,
            ComputedFeatureDefinition::FilletSet(_)
        ));
        assert!(matches!(
            fixture
                .session
                .accepted_state_for_current_input()
                .unwrap()
                .diagnostics()
                .solve
                .unwrap()
                .hard_validity,
            SketchHardValidity::Valid
        ));
        let initial = evaluate(&fixture);
        let _ = current_arc(&fixture, &initial);

        move_line(&mut fixture, VALID_MOVED_LINE_Y);
        let moved = evaluate(&fixture);
        let moved_arc = current_arc(&fixture, &moved);
        order_results.push((
            moved_arc.center,
            assert_independent_arc_invariants(&fixture, moved_arc),
        ));
        assert_eq!(fixture.features.identity(), feature_identity);
        assert_eq!(fixture.features.to_json().unwrap(), feature_json);

        move_line(&mut fixture, ENDPOINT_BLOCKED_LINE_Y);
        let supporting_circle_root = ((ENDPOINT_BLOCKED_LINE_Y + 1.0) / 3.0).asin();
        assert!(
            ARC_END < supporting_circle_root
                && supporting_circle_root < std::f64::consts::FRAC_PI_2,
            "the negative control must have a regular same-orientation root just beyond the finite arc endpoint"
        );
        let escaped_arc_parameter = (supporting_circle_root - ARC_START) / (ARC_END - ARC_START);
        assert!(escaped_arc_parameter > 1.0 && escaped_arc_parameter.is_finite());
        let blocked = evaluate(&fixture);
        let blocked_evaluation = blocked
            .feature_evaluations()
            .iter()
            .find(|evaluation| evaluation.feature == fixture.feature)
            .unwrap();
        assert!(matches!(
            blocked_evaluation.state,
            ComputedFeatureEvaluationState::Failed { .. }
        ));
        assert!(blocked.edges().is_empty());
        assert_eq!(fixture.features.identity(), feature_identity);
        assert_eq!(fixture.features.to_json().unwrap(), feature_json);

        move_line(&mut fixture, VALID_MOVED_LINE_Y);
        let recovered = evaluate(&fixture);
        let recovered_arc = current_arc(&fixture, &recovered);
        let recovered_contacts = assert_independent_arc_invariants(&fixture, recovered_arc);
        assert_eq!(
            recovered_arc.center.map(f64::to_bits),
            moved_arc.center.map(f64::to_bits)
        );
        assert_eq!(
            recovered_contacts.0.map(f64::to_bits),
            order_results.last().unwrap().1.0.map(f64::to_bits)
        );
        assert_eq!(
            recovered_contacts.1.map(f64::to_bits),
            order_results.last().unwrap().1.1.map(f64::to_bits)
        );
    }

    assert_eq!(order_results.len(), 2);
    assert_eq!(
        order_results[0].0.map(f64::to_bits),
        order_results[1].0.map(f64::to_bits)
    );
    assert_eq!(
        order_results[0].1.0.map(f64::to_bits),
        order_results[1].1.0.map(f64::to_bits)
    );
    assert_eq!(
        order_results[0].1.1.map(f64::to_bits),
        order_results[1].1.1.map(f64::to_bits)
    );
}
