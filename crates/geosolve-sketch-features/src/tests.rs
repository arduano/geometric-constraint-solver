// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_sketch::{
    CancellationToken, CurveDefinition, CurveSpan, DocumentArcSweep, DocumentCurveNormalSide,
    DocumentCurveTrimView, DocumentFilletEndpointOrder, DocumentFilletTrimEndpoint,
    DocumentObjectId, DocumentSolveRequest, DocumentTrimBoundary, DocumentTrimParameter,
    OperationControl, OperationLimits, OperationOutcome, RetainedSketchDocumentSession,
    ScalarDomain, ScalarUnit, SketchDocument, SolverConfig, cancellation_pair,
};

use crate::{
    ComputedEdgeGeometry, ComputedEvaluationAllocator, ComputedEvaluationAllocatorHighWater,
    ComputedEvaluationRevision, ComputedFeatureAllocatorHighWater, ComputedFeatureAuthoringError,
    ComputedFeatureDefinition, ComputedFeatureDocument, ComputedFeatureDocumentError,
    ComputedFeatureDocumentId, ComputedFeatureEvaluationPolicy, ComputedFeatureEvaluationSnapshot,
    ComputedFeatureEvaluationState, ComputedFeatureFailure, ComputedFeatureLifecycleHighWater,
    ComputedFeatureRevision, ComputedFeatureSnapshotError, ComputedFilletAuthoringOptions,
    ComputedFilletContactReseedRequest, ComputedFilletCornerAlternativeKind,
    ComputedFilletCornerAuthoringRequest, ComputedFilletCurvePick, ComputedFilletParent,
    ComputedFilletParentIndex, ContinuedComputedFilletCorner, NativeCurveSpanSource,
    NewComputedFilletCorner,
};

struct PolylineFixture {
    document: SketchDocument,
    curve: geosolve_sketch::CurveId,
    spans: [CurveSpan; 3],
}

fn polyline_fixture() -> PolylineFixture {
    scaled_polyline_fixture(1.0, 0x1000)
}

fn scaled_polyline_fixture(scale: f64, document_id: u128) -> PolylineFixture {
    let mut document = SketchDocument::with_id(
        10.0 * scale,
        geosolve_sketch::DocumentId(geosolve_sketch::PersistentId::from_u128(document_id)),
    )
    .unwrap();
    let points = [
        document.add_point("p0", [0.0, 0.0]).unwrap(),
        document.add_point("p1", [4.0 * scale, 0.0]).unwrap(),
        document
            .add_point("p2", [4.0 * scale, 4.0 * scale])
            .unwrap(),
        document
            .add_point("p3", [8.0 * scale, 4.0 * scale])
            .unwrap(),
    ];
    let curve = document
        .add_curve(
            "three span polyline",
            CurveDefinition::Polyline {
                points: points.to_vec(),
                closed: false,
                branch_directions: vec![[1.0, 0.0], [0.0, 1.0], [1.0, 0.0]],
            },
        )
        .unwrap();
    PolylineFixture {
        document,
        curve,
        spans: [0, 1, 2].map(|segment| CurveSpan { curve, segment }),
    }
}

fn retained(document: SketchDocument) -> RetainedSketchDocumentSession {
    let session = RetainedSketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    assert!(session.accepted_state_for_current_input().is_some());
    session
}

fn source(span: CurveSpan) -> NativeCurveSpanSource {
    NativeCurveSpanSource { span }
}

fn curve_pick(
    document: &SketchDocument,
    span: CurveSpan,
    parameter: f64,
    retained_endpoint_hint: DocumentFilletTrimEndpoint,
) -> ComputedFilletCurvePick {
    let jet = document.evaluate_curve_jet(span, parameter).unwrap();
    ComputedFilletCurvePick {
        source: source(span),
        parameter,
        model_position: [jet.position.x, jet.position.y],
        retained_endpoint_hint: Some(retained_endpoint_hint),
    }
}

fn separated_line_circle_request(
    fixture: &LineCircleFixture,
) -> ComputedFilletCornerAuthoringRequest {
    line_circle_branch_request(fixture, -1.5_f64.sqrt())
}

fn line_circle_branch_request(
    fixture: &LineCircleFixture,
    center_x: f64,
) -> ComputedFilletCornerAuthoringRequest {
    line_circle_branch_request_with_retention(
        fixture,
        center_x,
        DocumentFilletTrimEndpoint::End,
        DocumentFilletTrimEndpoint::End,
    )
}

fn line_circle_branch_request_with_retention(
    fixture: &LineCircleFixture,
    center_x: f64,
    line_retained_endpoint: DocumentFilletTrimEndpoint,
    circle_retained_endpoint: DocumentFilletTrimEndpoint,
) -> ComputedFilletCornerAuthoringRequest {
    let circle_parameter = (-1.25_f64)
        .atan2(center_x)
        .rem_euclid(std::f64::consts::TAU);
    ComputedFilletCornerAuthoringRequest {
        first: curve_pick(
            &fixture.document,
            fixture.line,
            (center_x + 5.0) / 10.0,
            line_retained_endpoint,
        ),
        second: curve_pick(
            &fixture.document,
            fixture.circle,
            circle_parameter,
            circle_retained_endpoint,
        ),
        options: ComputedFilletAuthoringOptions::default(),
    }
}

fn parent(
    span: CurveSpan,
    picked_parameter: f64,
    normal_side: DocumentCurveNormalSide,
    retained_endpoint: DocumentFilletTrimEndpoint,
) -> ComputedFilletParent {
    ComputedFilletParent {
        source: source(span),
        picked_parameter,
        winding: 0,
        neighborhood: geosolve_sketch::ContactNeighborhood::Interior,
        normal_side,
        retained_endpoint,
        periodic_anchor: None,
    }
}

fn first_corner(spans: [CurveSpan; 3]) -> NewComputedFilletCorner {
    NewComputedFilletCorner {
        first: parent(
            spans[0],
            0.875,
            DocumentCurveNormalSide::Left,
            DocumentFilletTrimEndpoint::End,
        ),
        second: parent(
            spans[1],
            0.125,
            DocumentCurveNormalSide::Left,
            DocumentFilletTrimEndpoint::Start,
        ),
        endpoint_order: DocumentFilletEndpointOrder::FirstThenSecond,
        sweep: DocumentArcSweep::CounterClockwise,
    }
}

fn second_corner(spans: [CurveSpan; 3]) -> NewComputedFilletCorner {
    NewComputedFilletCorner {
        first: parent(
            spans[1],
            0.875,
            DocumentCurveNormalSide::Right,
            DocumentFilletTrimEndpoint::End,
        ),
        second: parent(
            spans[2],
            0.125,
            DocumentCurveNormalSide::Right,
            DocumentFilletTrimEndpoint::Start,
        ),
        endpoint_order: DocumentFilletEndpointOrder::SecondThenFirst,
        sweep: DocumentArcSweep::CounterClockwise,
    }
}

fn first_corner_authoring_request(
    document: &SketchDocument,
    spans: [CurveSpan; 3],
) -> ComputedFilletCornerAuthoringRequest {
    let first_parameter = 0.75;
    let second_parameter = 0.25;
    let first_jet = document
        .evaluate_curve_jet(spans[0], first_parameter)
        .unwrap();
    let second_jet = document
        .evaluate_curve_jet(spans[1], second_parameter)
        .unwrap();
    ComputedFilletCornerAuthoringRequest {
        first: ComputedFilletCurvePick {
            source: source(spans[0]),
            parameter: first_parameter,
            model_position: [first_jet.position.x, first_jet.position.y],
            retained_endpoint_hint: Some(DocumentFilletTrimEndpoint::End),
        },
        second: ComputedFilletCurvePick {
            source: source(spans[1]),
            parameter: second_parameter,
            model_position: [second_jet.position.x, second_jet.position.y],
            retained_endpoint_hint: Some(DocumentFilletTrimEndpoint::Start),
        },
        options: ComputedFilletAuthoringOptions::default(),
    }
}

fn add_independent_corner(document: &mut SketchDocument) -> [CurveSpan; 2] {
    let points = [
        document.add_point("q0", [10.0, 0.0]).unwrap(),
        document.add_point("q1", [14.0, 0.0]).unwrap(),
        document.add_point("q2", [14.0, 4.0]).unwrap(),
    ];
    let curve = document
        .add_curve(
            "independent polyline",
            CurveDefinition::Polyline {
                points: points.to_vec(),
                closed: false,
                branch_directions: vec![[1.0, 0.0], [0.0, 1.0]],
            },
        )
        .unwrap();
    [0, 1].map(|segment| CurveSpan { curve, segment })
}

fn independent_corner(spans: [CurveSpan; 2]) -> NewComputedFilletCorner {
    NewComputedFilletCorner {
        first: parent(
            spans[0],
            0.875,
            DocumentCurveNormalSide::Left,
            DocumentFilletTrimEndpoint::End,
        ),
        second: parent(
            spans[1],
            0.125,
            DocumentCurveNormalSide::Left,
            DocumentFilletTrimEndpoint::Start,
        ),
        endpoint_order: DocumentFilletEndpointOrder::FirstThenSecond,
        sweep: DocumentArcSweep::CounterClockwise,
    }
}

struct LineCircleFixture {
    document: SketchDocument,
    line: CurveSpan,
    circle: CurveSpan,
    request: ComputedFilletCornerAuthoringRequest,
}

#[derive(Clone, Copy)]
struct TestSimilarity {
    scale: f64,
    angle: f64,
    translation: [f64; 2],
}

impl TestSimilarity {
    const IDENTITY: Self = Self {
        scale: 1.0,
        angle: 0.0,
        translation: [0.0, 0.0],
    };

    fn point(self, point: [f64; 2]) -> [f64; 2] {
        let (sin, cos) = self.angle.sin_cos();
        [
            self.scale
                .mul_add(cos.mul_add(point[0], -sin * point[1]), self.translation[0]),
            self.scale
                .mul_add(sin.mul_add(point[0], cos * point[1]), self.translation[1]),
        ]
    }

    fn direction(self, direction: [f64; 2]) -> [f64; 2] {
        let (sin, cos) = self.angle.sin_cos();
        [
            cos.mul_add(direction[0], -sin * direction[1]),
            sin.mul_add(direction[0], cos * direction[1]),
        ]
    }
}

fn line_line_similarity_fixture(
    similarity: TestSimilarity,
    turn_angle: f64,
    reverse_parent_order: bool,
    document_id: u128,
) -> (SketchDocument, ComputedFilletCornerAuthoringRequest) {
    let mut document = SketchDocument::with_id(
        10.0 * similarity.scale,
        geosolve_sketch::DocumentId(geosolve_sketch::PersistentId::from_u128(document_id)),
    )
    .unwrap();
    let points = [
        document
            .add_point("first start", similarity.point([-4.0, 0.0]))
            .unwrap(),
        document
            .add_point("shared corner", similarity.point([0.0, 0.0]))
            .unwrap(),
        document
            .add_point(
                "second end",
                similarity.point([4.0 * turn_angle.cos(), 4.0 * turn_angle.sin()]),
            )
            .unwrap(),
    ];
    let curve = document
        .add_curve(
            "two span polyline",
            CurveDefinition::Polyline {
                points: points.to_vec(),
                closed: false,
                branch_directions: vec![
                    similarity.direction([1.0, 0.0]),
                    similarity.direction([turn_angle.cos(), turn_angle.sin()]),
                ],
            },
        )
        .unwrap();
    let spans = [0, 1].map(|segment| CurveSpan { curve, segment });
    let mut request = ComputedFilletCornerAuthoringRequest {
        first: curve_pick(&document, spans[0], 0.75, DocumentFilletTrimEndpoint::End),
        second: curve_pick(&document, spans[1], 0.25, DocumentFilletTrimEndpoint::Start),
        options: ComputedFilletAuthoringOptions::default(),
    };
    if reverse_parent_order {
        std::mem::swap(&mut request.first, &mut request.second);
    }
    (document, request)
}

struct TwoCircleFixture {
    document: SketchDocument,
    spans: [CurveSpan; 2],
    request: ComputedFilletCornerAuthoringRequest,
}

fn two_circle_fixture() -> TwoCircleFixture {
    let mut document = SketchDocument::with_id(
        10.0,
        geosolve_sketch::DocumentId(geosolve_sketch::PersistentId::from_u128(0x2100)),
    )
    .unwrap();
    let centers = [
        document.add_point("first center", [0.0, 0.0]).unwrap(),
        document.add_point("second center", [4.0, 0.0]).unwrap(),
    ];
    let radii = [
        document
            .add_scalar(
                "first radius",
                1.0,
                ScalarUnit::Length,
                ScalarDomain::Positive,
            )
            .unwrap(),
        document
            .add_scalar(
                "second radius",
                1.0,
                ScalarUnit::Length,
                ScalarDomain::Positive,
            )
            .unwrap(),
    ];
    let spans = std::array::from_fn(|index| {
        CurveSpan::line(
            document
                .add_curve(
                    format!("circle {index}"),
                    CurveDefinition::Circle {
                        center: centers[index],
                        radius: radii[index],
                    },
                )
                .unwrap(),
        )
    });
    let parameters = [std::f64::consts::PI, std::f64::consts::PI];
    let picks: [ComputedFilletCurvePick; 2] = std::array::from_fn(|index| {
        let jet = document
            .evaluate_curve_jet(spans[index], parameters[index])
            .unwrap();
        ComputedFilletCurvePick {
            source: source(spans[index]),
            parameter: parameters[index],
            model_position: [jet.position.x, jet.position.y],
            retained_endpoint_hint: Some(DocumentFilletTrimEndpoint::End),
        }
    });
    TwoCircleFixture {
        document,
        spans,
        request: ComputedFilletCornerAuthoringRequest {
            first: picks[0],
            second: picks[1],
            options: ComputedFilletAuthoringOptions::default(),
        },
    }
}

fn periodic_circle_parent(
    span: CurveSpan,
    normal_side: DocumentCurveNormalSide,
) -> ComputedFilletParent {
    ComputedFilletParent {
        source: source(span),
        picked_parameter: std::f64::consts::PI,
        winding: 0,
        neighborhood: geosolve_sketch::ContactNeighborhood::Local {
            lower: std::f64::consts::PI - 0.5,
            upper: std::f64::consts::PI + 0.5,
        },
        normal_side,
        retained_endpoint: DocumentFilletTrimEndpoint::End,
        periodic_anchor: Some(DocumentTrimParameter {
            parameter: 0.0,
            winding: 0,
        }),
    }
}

fn line_circle_fixture() -> LineCircleFixture {
    line_circle_fixture_with_similarity(TestSimilarity::IDENTITY, 0x2000)
}

fn line_circle_fixture_with_similarity(
    similarity: TestSimilarity,
    document_id: u128,
) -> LineCircleFixture {
    let mut document = SketchDocument::with_id(
        10.0 * similarity.scale,
        geosolve_sketch::DocumentId(geosolve_sketch::PersistentId::from_u128(document_id)),
    )
    .unwrap();
    let line_start = document
        .add_point("line start", similarity.point([-5.0, 0.0]))
        .unwrap();
    let line_end = document
        .add_point("line end", similarity.point([5.0, 0.0]))
        .unwrap();
    let line = CurveSpan::line(
        document
            .add_curve(
                "line",
                CurveDefinition::Line {
                    start: line_start,
                    end: line_end,
                    branch_direction: similarity.direction([1.0, 0.0]),
                },
            )
            .unwrap(),
    );
    let center = document
        .add_point("circle center", similarity.point([0.0, 2.0]))
        .unwrap();
    let circle_radius = document
        .add_scalar(
            "circle radius",
            similarity.scale,
            ScalarUnit::Length,
            ScalarDomain::Positive,
        )
        .unwrap();
    let circle = CurveSpan::line(
        document
            .add_curve(
                "circle",
                CurveDefinition::Circle {
                    center,
                    radius: circle_radius,
                },
            )
            .unwrap(),
    );
    let line_parameter = 0.4;
    let circle_parameter = 4.5;
    let line_jet = document.evaluate_curve_jet(line, line_parameter).unwrap();
    let circle_jet = document
        .evaluate_curve_jet(circle, circle_parameter)
        .unwrap();
    let request = ComputedFilletCornerAuthoringRequest {
        first: ComputedFilletCurvePick {
            source: source(line),
            parameter: line_parameter,
            model_position: [line_jet.position.x, line_jet.position.y],
            retained_endpoint_hint: Some(DocumentFilletTrimEndpoint::End),
        },
        second: ComputedFilletCurvePick {
            source: source(circle),
            parameter: circle_parameter,
            model_position: [circle_jet.position.x, circle_jet.position.y],
            retained_endpoint_hint: Some(DocumentFilletTrimEndpoint::End),
        },
        options: ComputedFilletAuthoringOptions::default(),
    };
    LineCircleFixture {
        document,
        line,
        circle,
        request,
    }
}

struct LineQuadraticFixture {
    document: SketchDocument,
    request: ComputedFilletCornerAuthoringRequest,
}

struct HighCurvatureQuadraticFixture {
    document: SketchDocument,
    controls: [geosolve_sketch::DesignPointId; 3],
    prior: NewComputedFilletCorner,
}

fn high_curvature_quadratic_fixture() -> HighCurvatureQuadraticFixture {
    let mut document = SketchDocument::with_id(
        10.0,
        geosolve_sketch::DocumentId(geosolve_sketch::PersistentId::from_u128(0x2700)),
    )
    .unwrap();
    let line_start = document.add_point("line start", [-5.0, 0.0]).unwrap();
    let line_end = document.add_point("line end", [5.0, 0.0]).unwrap();
    let line = CurveSpan::line(
        document
            .add_curve(
                "line",
                CurveDefinition::Line {
                    start: line_start,
                    end: line_end,
                    branch_direction: [1.0, 0.0],
                },
            )
            .unwrap(),
    );
    let controls = [
        document.add_point("quadratic start", [0.5, 0.0]).unwrap(),
        document
            .add_point("quadratic control", [-0.5, 0.5])
            .unwrap(),
        document.add_point("quadratic end", [0.5, 1.0]).unwrap(),
    ];
    let quadratic = CurveSpan::line(
        document
            .add_curve("quadratic", CurveDefinition::QuadraticBezier { controls })
            .unwrap(),
    );
    HighCurvatureQuadraticFixture {
        document,
        controls,
        prior: NewComputedFilletCorner {
            first: parent(
                line,
                0.55,
                DocumentCurveNormalSide::Left,
                DocumentFilletTrimEndpoint::End,
            ),
            second: ComputedFilletParent {
                source: source(quadratic),
                picked_parameter: 0.5,
                winding: 0,
                neighborhood: geosolve_sketch::ContactNeighborhood::Local {
                    lower: 0.0,
                    upper: 1.0,
                },
                normal_side: DocumentCurveNormalSide::Right,
                retained_endpoint: DocumentFilletTrimEndpoint::Start,
                periodic_anchor: None,
            },
            endpoint_order: DocumentFilletEndpointOrder::FirstThenSecond,
            sweep: DocumentArcSweep::CounterClockwise,
        },
    }
}

fn line_quadratic_fixture_with_similarity(
    similarity: TestSimilarity,
    document_id: u128,
) -> LineQuadraticFixture {
    let mut document = SketchDocument::with_id(
        10.0 * similarity.scale,
        geosolve_sketch::DocumentId(geosolve_sketch::PersistentId::from_u128(document_id)),
    )
    .unwrap();
    let line_start = document
        .add_point("line start", similarity.point([6.0, -8.0]))
        .unwrap();
    let line_end = document
        .add_point("line end", similarity.point([6.0, 0.0]))
        .unwrap();
    let line = CurveSpan::line(
        document
            .add_curve(
                "line",
                CurveDefinition::Line {
                    start: line_start,
                    end: line_end,
                    branch_direction: similarity.direction([0.0, 1.0]),
                },
            )
            .unwrap(),
    );
    let controls = [
        document
            .add_point("quadratic start", similarity.point([1.0, -3.0]))
            .unwrap(),
        document
            .add_point("quadratic control", similarity.point([4.0, -7.0]))
            .unwrap(),
        document
            .add_point("quadratic end", similarity.point([8.0, -3.0]))
            .unwrap(),
    ];
    let quadratic = CurveSpan::line(
        document
            .add_curve("quadratic", CurveDefinition::QuadraticBezier { controls })
            .unwrap(),
    );
    let parameters = [0.5, 0.75];
    let spans = [line, quadratic];
    let picks: [ComputedFilletCurvePick; 2] = std::array::from_fn(|index| {
        let jet = document
            .evaluate_curve_jet(spans[index], parameters[index])
            .unwrap();
        ComputedFilletCurvePick {
            source: source(spans[index]),
            parameter: parameters[index],
            model_position: [jet.position.x, jet.position.y],
            retained_endpoint_hint: Some(if index == 0 {
                DocumentFilletTrimEndpoint::Start
            } else {
                DocumentFilletTrimEndpoint::End
            }),
        }
    });
    LineQuadraticFixture {
        document,
        request: ComputedFilletCornerAuthoringRequest {
            first: picks[0],
            second: picks[1],
            options: ComputedFilletAuthoringOptions::default(),
        },
    }
}

fn complete<T: std::fmt::Debug>(outcome: OperationOutcome<T>) -> T {
    match outcome {
        OperationOutcome::Completed { value, .. } => value,
        other => panic!("expected completed operation, got {other:?}"),
    }
}

fn continue_corner(
    authoring: &crate::ComputedFeatureAuthoringSnapshot,
    prior: NewComputedFilletCorner,
    from_radius: f64,
    radius: f64,
) -> ContinuedComputedFilletCorner {
    complete(
        authoring
            .continue_fillet_corner(
                prior,
                from_radius,
                radius,
                ComputedFeatureEvaluationPolicy::default(),
                OperationControl::unlimited(),
            )
            .unwrap(),
    )
}

fn assert_close(actual: f64, expected: f64, tolerance: f64) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "expected {expected:.12e}, got {actual:.12e}, tolerance {tolerance:.3e}"
    );
}

fn assert_radius_sensitivity_matches_finite_difference(
    authoring: &crate::ComputedFeatureAuthoringSnapshot,
    prior: NewComputedFilletCorner,
    radius: f64,
) -> ContinuedComputedFilletCorner {
    let resolved = continue_corner(authoring, prior, radius, radius);
    let step = 1.0e-5 * radius;
    let lower = continue_corner(authoring, prior, radius, radius - step);
    let upper = continue_corner(authoring, prior, radius, radius + step);
    for axis in 0..2 {
        let finite_difference = (upper.arc.center[axis] - lower.arc.center[axis]) / (2.0 * step);
        assert_close(
            resolved.sensitivity.center_derivative[axis],
            finite_difference,
            2.0e-5 * finite_difference.abs().max(1.0),
        );
    }
    for parent in 0..2 {
        let parameter_finite_difference = (upper.arc.contacts[parent].total_parameter
            - lower.arc.contacts[parent].total_parameter)
            / (2.0 * step);
        assert_close(
            resolved.sensitivity.contact_parameter_derivatives[parent],
            parameter_finite_difference,
            2.0e-5 * parameter_finite_difference.abs().max(1.0),
        );
        for axis in 0..2 {
            let position_finite_difference = (upper.arc.contacts[parent].position[axis]
                - lower.arc.contacts[parent].position[axis])
                / (2.0 * step);
            assert_close(
                resolved.sensitivity.contact_position_derivatives[parent][axis],
                position_finite_difference,
                3.0e-5 * position_finite_difference.abs().max(1.0),
            );
        }
    }
    assert!(resolved.sensitivity.transverse_quality > 1.0e-3);
    resolved
}

fn evaluate(
    session: &RetainedSketchDocumentSession,
    features: &ComputedFeatureDocument,
    allocator: &mut ComputedEvaluationAllocator,
) -> crate::ComputedFeatureSnapshot {
    let snapshot = ComputedFeatureEvaluationSnapshot::capture(
        session,
        features,
        ComputedFeatureEvaluationPolicy::default(),
    )
    .unwrap();
    complete(
        snapshot
            .prepare(allocator)
            .unwrap()
            .execute(OperationControl::unlimited())
            .unwrap(),
    )
}

fn evaluate_single_corner_failure(
    document: &SketchDocument,
    radius: f64,
    corner: NewComputedFilletCorner,
) -> ComputedFeatureFailure {
    let session = retained(document.clone());
    let mut features = ComputedFeatureDocument::new(document.id());
    let feature = features
        .create_fillet_set("failure", radius, vec![corner])
        .unwrap();
    let mut allocator = ComputedEvaluationAllocator::default();
    let snapshot = evaluate(&session, &features, &mut allocator);
    match snapshot
        .feature_evaluations()
        .iter()
        .find(|value| value.feature == feature)
        .unwrap()
        .state
        .clone()
    {
        ComputedFeatureEvaluationState::Failed { failure } => failure,
        other => panic!("expected failed feature, got {other:?}"),
    }
}

fn arc_centers(snapshot: &crate::ComputedFeatureSnapshot) -> Vec<[u64; 2]> {
    let mut centers = snapshot
        .edges()
        .iter()
        .filter_map(|edge| match &edge.geometry {
            ComputedEdgeGeometry::CircularArc(arc) => {
                Some([arc.center[0].to_bits(), arc.center[1].to_bits()])
            }
            ComputedEdgeGeometry::NativeSourceFragment { .. } => None,
        })
        .collect::<Vec<_>>();
    centers.sort_unstable();
    centers
}

type SourceGeometrySignature = (NativeCurveSpanSource, u64, u64);
type ComputedGeometrySignature = (Vec<SourceGeometrySignature>, Vec<Vec<u64>>);

fn geometry_signature(snapshot: &crate::ComputedFeatureSnapshot) -> ComputedGeometrySignature {
    let mut sources = Vec::new();
    let mut arcs = Vec::new();
    for edge in snapshot.edges() {
        match &edge.geometry {
            ComputedEdgeGeometry::NativeSourceFragment { source, interval } => {
                sources.push((*source, interval.start.to_bits(), interval.end.to_bits()));
            }
            ComputedEdgeGeometry::CircularArc(arc) => {
                let mut signature = vec![
                    arc.center[0].to_bits(),
                    arc.center[1].to_bits(),
                    arc.radius.to_bits(),
                    arc.start_angle.to_bits(),
                    arc.end_angle.to_bits(),
                    match arc.sweep {
                        DocumentArcSweep::CounterClockwise => 0,
                        DocumentArcSweep::Clockwise => 1,
                    },
                ];
                for contact in arc.contacts {
                    signature.extend([
                        contact.parameter.to_bits(),
                        u64::from_ne_bytes(i64::from(contact.winding).to_ne_bytes()),
                        contact.total_parameter.to_bits(),
                        contact.position[0].to_bits(),
                        contact.position[1].to_bits(),
                    ]);
                }
                arcs.push(signature);
            }
        }
    }
    sources.sort_unstable();
    arcs.sort_unstable();
    (sources, arcs)
}

fn saturated_revision(mut document: ComputedFeatureDocument) -> ComputedFeatureDocument {
    document.set_revision_for_test(ComputedFeatureRevision::from_raw(u64::MAX));
    document
}

fn assert_revision_exhaustion_rolls_back<T>(
    document: ComputedFeatureDocument,
    mutate: impl FnOnce(&mut ComputedFeatureDocument) -> Result<T, ComputedFeatureDocumentError>,
) {
    let mut document = saturated_revision(document);
    let before = document.clone();
    assert!(matches!(
        mutate(&mut document),
        Err(ComputedFeatureDocumentError::RevisionExhausted)
    ));
    assert_eq!(document, before);
}

#[test]
fn strict_json_digest_duplicate_pairs_and_lifecycle_high_water() {
    let fixture = polyline_fixture();
    let mut features = ComputedFeatureDocument::with_id(
        fixture.document.id(),
        ComputedFeatureDocumentId::from_raw(0xabc),
    );
    let checkpoint = features.clone();
    let first = features
        .create_fillet_set("corners", 0.5, vec![first_corner(fixture.spans)])
        .unwrap();
    let first_corner_id = match &features.feature(first).unwrap().definition {
        ComputedFeatureDefinition::FilletSet(fillet) => fillet.corners[0].id,
    };
    assert!(matches!(
        features.add_fillet_corners(first, vec![first_corner(fixture.spans)]),
        Err(ComputedFeatureDocumentError::InvalidField {
            field: "corner parents",
            ..
        })
    ));

    let json = features.to_json().unwrap();
    assert_eq!(ComputedFeatureDocument::from_json(&json).unwrap(), features);
    let mut unknown: serde_json::Value = serde_json::from_str(&json).unwrap();
    unknown["misleading_old_field"] = serde_json::json!(true);
    assert!(matches!(
        ComputedFeatureDocument::from_json(&serde_json::to_string(&unknown).unwrap()),
        Err(ComputedFeatureDocumentError::Json(_))
    ));
    let tampered = json.replace("\"radius\":0.5", "\"radius\":0.6");
    assert!(matches!(
        ComputedFeatureDocument::from_json(&tampered),
        Err(ComputedFeatureDocumentError::DigestMismatch)
    ));

    let retained_high_water = features.lifecycle_high_water();
    let mut restored = checkpoint;
    restored.rebase_after_restore(retained_high_water).unwrap();
    assert!(restored.revision() > retained_high_water.revision);
    let replacement = restored
        .create_fillet_set("replacement", 0.5, vec![first_corner(fixture.spans)])
        .unwrap();
    let replacement_corner = match &restored.feature(replacement).unwrap().definition {
        ComputedFeatureDefinition::FilletSet(fillet) => fillet.corners[0].id,
    };
    assert!(replacement > first);
    assert!(replacement_corner > first_corner_id);
}

#[test]
fn every_revision_exhaustion_path_is_transactional() {
    let fixture = polyline_fixture();
    let mut base = ComputedFeatureDocument::new(fixture.document.id());
    let feature = base
        .create_fillet_set("first", 0.5, vec![first_corner(fixture.spans)])
        .unwrap();
    let corner = match &base.feature(feature).unwrap().definition {
        ComputedFeatureDefinition::FilletSet(fillet) => fillet.corners[0].id,
    };

    assert_revision_exhaustion_rolls_back(base.clone(), |document| {
        document.create_fillet_set("second", 0.5, vec![second_corner(fixture.spans)])
    });
    assert_revision_exhaustion_rolls_back(base.clone(), |document| {
        document.add_fillet_corners(feature, vec![second_corner(fixture.spans)])
    });
    assert_revision_exhaustion_rolls_back(base.clone(), |document| {
        document.set_fillet_radius(feature, 0.75)
    });
    let mut replacement = first_corner(fixture.spans);
    replacement.sweep = DocumentArcSweep::Clockwise;
    assert_revision_exhaustion_rolls_back(base.clone(), |document| {
        document.set_fillet_corner(feature, corner, replacement)
    });
    assert_revision_exhaustion_rolls_back(base.clone(), |document| {
        document.replace_fillet_set(feature, 0.75, vec![(corner, replacement)])
    });
    assert_revision_exhaustion_rolls_back(base.clone(), |document| {
        document.set_suppressed(feature, true)
    });
    assert_revision_exhaustion_rolls_back(base.clone(), |document| {
        document.set_label(feature, "renamed")
    });
    assert_revision_exhaustion_rolls_back(base.clone(), |document| {
        document.remove_corner(feature, corner)
    });
    assert_revision_exhaustion_rolls_back(base.clone(), |document| {
        document.remove_feature(feature)
    });
    let allocator = base.allocator_high_water();
    assert_revision_exhaustion_rolls_back(base.clone(), |document| {
        document.retain_allocator_high_water(ComputedFeatureAllocatorHighWater {
            next_feature_id: crate::ComputedFeatureId::from_raw(
                allocator.next_feature_id.raw() + 10,
            ),
            next_corner_id: crate::ComputedFeatureCornerId::from_raw(
                allocator.next_corner_id.raw() + 10,
            ),
        })
    });

    let mut restored = base;
    let before = restored.clone();
    assert!(matches!(
        restored.rebase_after_restore(ComputedFeatureLifecycleHighWater {
            revision: ComputedFeatureRevision::from_raw(u64::MAX),
            allocator: ComputedFeatureAllocatorHighWater {
                next_feature_id: crate::ComputedFeatureId::from_raw(
                    allocator.next_feature_id.raw() + 10,
                ),
                next_corner_id: crate::ComputedFeatureCornerId::from_raw(
                    allocator.next_corner_id.raw() + 10,
                ),
            },
        }),
        Err(ComputedFeatureDocumentError::RevisionExhausted)
    ));
    assert_eq!(restored, before);
}

#[test]
fn complete_fillet_set_replacement_is_atomic_and_identity_preserving() {
    let fixture = polyline_fixture();
    let mut features = ComputedFeatureDocument::new(fixture.document.id());
    let feature = features
        .create_fillet_set(
            "both corners",
            0.5,
            vec![first_corner(fixture.spans), second_corner(fixture.spans)],
        )
        .unwrap();
    let before_revision = features.revision();
    let before_allocator = features.allocator_high_water();
    let before_corners = match &features.feature(feature).unwrap().definition {
        ComputedFeatureDefinition::FilletSet(fillet) => fillet.corners.clone(),
    };
    let replacements = before_corners
        .iter()
        .map(|corner| {
            let mut replacement = corner.without_id();
            replacement.sweep = DocumentArcSweep::Clockwise;
            (corner.id, replacement)
        })
        .collect::<Vec<_>>();
    features
        .replace_fillet_set(feature, 0.75, replacements.clone())
        .unwrap();
    assert_eq!(features.revision().raw(), before_revision.raw() + 1);
    assert_eq!(features.allocator_high_water(), before_allocator);
    let after = match &features.feature(feature).unwrap().definition {
        ComputedFeatureDefinition::FilletSet(fillet) => fillet.clone(),
    };
    assert_eq!(after.radius.to_bits(), 0.75_f64.to_bits());
    assert_eq!(
        after
            .corners
            .iter()
            .map(|corner| corner.id)
            .collect::<Vec<_>>(),
        before_corners
            .iter()
            .map(|corner| corner.id)
            .collect::<Vec<_>>()
    );
    assert!(
        after
            .corners
            .iter()
            .all(|corner| corner.sweep == DocumentArcSweep::Clockwise)
    );

    let no_op_revision = features.revision();
    features
        .replace_fillet_set(feature, 0.75, replacements)
        .unwrap();
    assert_eq!(features.revision(), no_op_revision);
    assert_eq!(features.allocator_high_water(), before_allocator);

    let current = features.clone();
    let ids = after
        .corners
        .iter()
        .map(|corner| corner.id)
        .collect::<Vec<_>>();
    let replacement = after.corners[0].without_id();
    let invalid_replacements = [
        vec![(ids[0], replacement)],
        vec![(ids[0], replacement), (ids[0], replacement)],
        vec![
            (ids[0], replacement),
            (
                crate::ComputedFeatureCornerId::from_raw(u64::MAX - 1),
                after.corners[1].without_id(),
            ),
        ],
    ];
    for invalid in invalid_replacements {
        let mut candidate = current.clone();
        assert!(candidate.replace_fillet_set(feature, 1.0, invalid).is_err());
        assert_eq!(candidate, current);
    }

    let mut duplicate_pair = current.clone();
    assert!(matches!(
        duplicate_pair.replace_fillet_set(
            feature,
            1.0,
            vec![(ids[0], replacement), (ids[1], replacement)],
        ),
        Err(ComputedFeatureDocumentError::InvalidField {
            field: "corner parents",
            ..
        })
    ));
    assert_eq!(duplicate_pair, current);
}

#[test]
fn invalid_shared_radii_are_rejected_transactionally_by_persistence_and_evaluation() {
    let fixture = polyline_fixture();
    let invalid_radii = [0.0, -1.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY];

    for radius in invalid_radii {
        let mut features = ComputedFeatureDocument::new(fixture.document.id());
        let before = features.clone();
        assert!(matches!(
            features
                .create_fillet_set("invalid radius", radius, vec![first_corner(fixture.spans)],),
            Err(ComputedFeatureDocumentError::InvalidField {
                field: "radius",
                ..
            })
        ));
        assert_eq!(features, before);
    }

    let mut features = ComputedFeatureDocument::new(fixture.document.id());
    let feature = features
        .create_fillet_set("valid", 0.5, vec![first_corner(fixture.spans)])
        .unwrap();
    for radius in invalid_radii {
        let before = features.clone();
        assert!(matches!(
            features.set_fillet_radius(feature, radius),
            Err(ComputedFeatureDocumentError::InvalidField {
                field: "radius",
                ..
            })
        ));
        assert_eq!(features, before);
    }

    let session = retained(fixture.document.clone());
    let before_input = session.prepared_input();
    let before_document = session.design_document().clone();
    let before_accepted = session
        .accepted_state()
        .map(geosolve_sketch::SketchAcceptedDocumentState::identity);
    let authoring = crate::ComputedFeatureAuthoringSnapshot::capture(&session).unwrap();
    let request = first_corner_authoring_request(&fixture.document, fixture.spans);
    for radius in invalid_radii {
        assert!(matches!(
            authoring.resolve_fillet_corner(
                request,
                radius,
                ComputedFeatureEvaluationPolicy::default(),
                OperationControl::unlimited(),
            ),
            Err(ComputedFeatureAuthoringError::InvalidRadius)
        ));
        assert!(matches!(
            authoring.resolve_fillet_corners(
                &[request],
                radius,
                ComputedFeatureEvaluationPolicy::default(),
                OperationControl::unlimited(),
            ),
            Err(ComputedFeatureAuthoringError::InvalidRadius)
        ));

        let mut invalid_features = features.clone();
        invalid_features.set_fillet_radius_unchecked_for_test(feature, radius);
        assert!(matches!(
            ComputedFeatureEvaluationSnapshot::capture(
                &session,
                &invalid_features,
                ComputedFeatureEvaluationPolicy::default(),
            ),
            Err(ComputedFeatureSnapshotError::InvalidFeatureDocument(
                ComputedFeatureDocumentError::InvalidField {
                    field: "radius",
                    ..
                }
            ))
        ));
    }
    assert_eq!(session.prepared_input(), before_input);
    assert_eq!(session.design_document(), &before_document);
    assert_eq!(
        session
            .accepted_state()
            .map(geosolve_sketch::SketchAcceptedDocumentState::identity),
        before_accepted
    );
}

#[test]
fn reversed_parent_order_is_canonical_without_changing_arc_endpoint_semantics() {
    let fixture = polyline_fixture();
    let original = first_corner(fixture.spans);
    let reversed = NewComputedFilletCorner {
        first: original.second,
        second: original.first,
        endpoint_order: DocumentFilletEndpointOrder::SecondThenFirst,
        sweep: original.sweep,
    };
    assert_eq!(reversed.canonicalized(), original);
    let mut features = ComputedFeatureDocument::new(fixture.document.id());
    let id = features
        .create_fillet_set("reverse", 0.5, vec![reversed])
        .unwrap();
    let ComputedFeatureDefinition::FilletSet(fillet) = &features.feature(id).unwrap().definition;
    assert_eq!(fillet.corners[0].without_id(), original);

    let session = retained(fixture.document.clone());
    let mut original_features = ComputedFeatureDocument::new(fixture.document.id());
    original_features
        .create_fillet_set("original", 0.5, vec![original])
        .unwrap();
    let mut allocator = ComputedEvaluationAllocator::default();
    assert_eq!(
        geometry_signature(&evaluate(&session, &original_features, &mut allocator)),
        geometry_signature(&evaluate(&session, &features, &mut allocator))
    );
}

#[test]
fn adjacent_batch_composes_both_middle_endpoints_and_variable_output_count() {
    let fixture = polyline_fixture();
    let session = retained(fixture.document.clone());
    let mut features = ComputedFeatureDocument::new(fixture.document.id());
    let feature = features
        .create_fillet_set(
            "both corners",
            0.5,
            vec![first_corner(fixture.spans), second_corner(fixture.spans)],
        )
        .unwrap();
    let mut allocator = ComputedEvaluationAllocator::default();
    let snapshot = evaluate(&session, &features, &mut allocator);
    assert_eq!(snapshot.edges().len(), 5);
    assert_eq!(arc_centers(&snapshot).len(), 2);
    let middle = snapshot
        .source_fragment_edges(source(fixture.spans[1]))
        .next()
        .unwrap();
    let crate::ComputedEdgeProvenance::SourceFragment {
        interval,
        start_claim,
        end_claim,
        ..
    } = &middle.provenance
    else {
        panic!("expected source fragment");
    };
    assert!(start_claim.is_some() && end_claim.is_some());
    assert!((interval.start - 0.125).abs() < 1.0e-8);
    assert!((interval.end - 0.875).abs() < 1.0e-8);
    let state = &snapshot
        .feature_evaluations()
        .iter()
        .find(|value| value.feature == feature)
        .unwrap()
        .state;
    assert!(matches!(
        state,
        ComputedFeatureEvaluationState::Current { corner_edges } if corner_edges.len() == 2
    ));
}

#[test]
fn sequential_sets_match_batch_geometry_and_suppression_is_local() {
    let fixture = polyline_fixture();
    let session = retained(fixture.document.clone());
    let mut batch = ComputedFeatureDocument::new(fixture.document.id());
    batch
        .create_fillet_set(
            "batch",
            0.5,
            vec![first_corner(fixture.spans), second_corner(fixture.spans)],
        )
        .unwrap();
    let mut sequential = ComputedFeatureDocument::new(fixture.document.id());
    let first = sequential
        .create_fillet_set("first", 0.5, vec![first_corner(fixture.spans)])
        .unwrap();
    sequential
        .create_fillet_set("second", 0.5, vec![second_corner(fixture.spans)])
        .unwrap();
    let mut allocator = ComputedEvaluationAllocator::default();
    let batch_snapshot = evaluate(&session, &batch, &mut allocator);
    let sequential_snapshot = evaluate(&session, &sequential, &mut allocator);
    assert_eq!(
        geometry_signature(&batch_snapshot),
        geometry_signature(&sequential_snapshot)
    );
    assert_ne!(
        batch_snapshot.evaluation_revision(),
        sequential_snapshot.evaluation_revision()
    );
    assert!(
        batch_snapshot
            .edge(sequential_snapshot.edges()[0].id)
            .is_none()
    );

    sequential.set_suppressed(first, true).unwrap();
    let suppressed = evaluate(&session, &sequential, &mut allocator);
    assert_eq!(arc_centers(&suppressed).len(), 1);
    assert!(matches!(
        suppressed
            .feature_evaluations()
            .iter()
            .find(|value| value.feature == first)
            .unwrap()
            .state,
        ComputedFeatureEvaluationState::Suppressed
    ));
}

#[test]
fn crossed_claims_fail_whole_set_atomically_and_radius_edit_recovers() {
    let fixture = polyline_fixture();
    let session = retained(fixture.document.clone());
    let mut features = ComputedFeatureDocument::new(fixture.document.id());
    let feature = features
        .create_fillet_set(
            "conflict",
            2.5,
            vec![first_corner(fixture.spans), second_corner(fixture.spans)],
        )
        .unwrap();
    let mut allocator = ComputedEvaluationAllocator::default();
    let failed = evaluate(&session, &features, &mut allocator);
    assert!(arc_centers(&failed).is_empty());
    let failed_state = &failed
        .feature_evaluations()
        .iter()
        .find(|value| value.feature == feature)
        .unwrap()
        .state;
    assert!(
        matches!(
            failed_state,
            ComputedFeatureEvaluationState::Failed {
                failure: ComputedFeatureFailure::ConsumedSourceInterval { .. }
            }
        ),
        "unexpected crossed-claim state: {failed_state:?}"
    );
    features.set_fillet_radius(feature, 0.5).unwrap();
    let recovered = evaluate(&session, &features, &mut allocator);
    assert_eq!(arc_centers(&recovered).len(), 2);
}

#[test]
fn missing_source_is_repairable_and_does_not_block_unrelated_feature() {
    let mut fixture = polyline_fixture();
    let independent = add_independent_corner(&mut fixture.document);
    let original = retained(fixture.document.clone());
    let mut features = ComputedFeatureDocument::new(fixture.document.id());
    let missing = features
        .create_fillet_set("will be missing", 0.5, vec![first_corner(fixture.spans)])
        .unwrap();
    let other = features
        .create_fillet_set("survives", 0.5, vec![independent_corner(independent)])
        .unwrap();
    let mut deleted = fixture.document.clone();
    deleted
        .remove_with_owned_state(DocumentObjectId::Curve(fixture.curve))
        .unwrap();
    let deleted = retained(deleted);
    let mut allocator = ComputedEvaluationAllocator::default();
    let failed = evaluate(&deleted, &features, &mut allocator);
    assert!(matches!(
        failed
            .feature_evaluations()
            .iter()
            .find(|value| value.feature == missing)
            .unwrap()
            .state,
        ComputedFeatureEvaluationState::Failed {
            failure: ComputedFeatureFailure::MissingSource { .. }
        }
    ));
    assert!(matches!(
        failed
            .feature_evaluations()
            .iter()
            .find(|value| value.feature == other)
            .unwrap()
            .state,
        ComputedFeatureEvaluationState::Current { .. }
    ));
    assert_eq!(arc_centers(&failed).len(), 1);
    let recovered = evaluate(&original, &features, &mut allocator);
    assert!(recovered.feature_evaluations().iter().any(|value| {
        value.feature == missing
            && matches!(value.state, ComputedFeatureEvaluationState::Current { .. })
    }));
    assert!(recovered.feature_evaluations().iter().any(|value| {
        value.feature == other
            && matches!(value.state, ComputedFeatureEvaluationState::Current { .. })
    }));
}

#[test]
fn evaluation_allocator_persists_nonreuse_and_control_stops_without_output() {
    let fixture = polyline_fixture();
    let session = retained(fixture.document.clone());
    let mut features = ComputedFeatureDocument::new(fixture.document.id());
    features
        .create_fillet_set("one", 0.5, vec![first_corner(fixture.spans)])
        .unwrap();
    let mut allocator = ComputedEvaluationAllocator::default();
    let first = evaluate(&session, &features, &mut allocator);
    let high_water = allocator.high_water();
    let encoded = serde_json::to_string(&high_water).unwrap();
    let restored: ComputedEvaluationAllocatorHighWater = serde_json::from_str(&encoded).unwrap();
    let mut restored_allocator = ComputedEvaluationAllocator::from_high_water(restored);
    let second = evaluate(&session, &features, &mut restored_allocator);
    assert!(second.evaluation_revision() > first.evaluation_revision());

    let snapshot = ComputedFeatureEvaluationSnapshot::capture(
        &session,
        &features,
        ComputedFeatureEvaluationPolicy::default(),
    )
    .unwrap();
    let (handle, token) = cancellation_pair();
    handle.cancel();
    assert!(matches!(
        snapshot
            .clone()
            .prepare(&mut restored_allocator)
            .unwrap()
            .execute(OperationControl::new(token, OperationLimits::unlimited()))
            .unwrap(),
        OperationOutcome::Cancelled { .. }
    ));
    let mut limits = OperationLimits::unlimited();
    limits.document_validation_items = 0;
    assert!(matches!(
        snapshot
            .prepare(&mut restored_allocator)
            .unwrap()
            .execute(OperationControl::new(CancellationToken::default(), limits))
            .unwrap(),
        OperationOutcome::WorkExhausted { .. }
    ));
}

#[test]
fn authoring_owns_root_selection_and_rejects_fabricated_pick_positions() {
    let fixture = polyline_fixture();
    let session = retained(fixture.document.clone());
    let authoring = crate::ComputedFeatureAuthoringSnapshot::capture(&session).unwrap();
    let request = first_corner_authoring_request(&fixture.document, fixture.spans);
    let resolved = complete(
        authoring
            .resolve_fillet_corner(
                request,
                0.5,
                ComputedFeatureEvaluationPolicy::default(),
                OperationControl::unlimited(),
            )
            .unwrap(),
    );
    assert!((resolved.arc.radius - 0.5).abs() < 1.0e-12);
    assert_eq!(resolved.corner.first.source, source(fixture.spans[0]));

    let mut fabricated = request;
    fabricated.first.model_position[0] += 1.0;
    assert!(matches!(
        authoring.resolve_fillet_corner(
            fabricated,
            0.5,
            ComputedFeatureEvaluationPolicy::default(),
            OperationControl::unlimited(),
        ),
        Err(crate::ComputedFeatureAuthoringError::StalePick)
    ));
}

#[test]
fn absolute_radius_continuation_preserves_branch_and_round_trips_line_line() {
    let fixture = polyline_fixture();
    let session = retained(fixture.document.clone());
    let authoring = crate::ComputedFeatureAuthoringSnapshot::capture(&session).unwrap();
    let initial = complete(
        authoring
            .resolve_fillet_corner(
                first_corner_authoring_request(&fixture.document, fixture.spans),
                0.5,
                ComputedFeatureEvaluationPolicy::default(),
                OperationControl::unlimited(),
            )
            .unwrap(),
    );
    let forward = continue_corner(&authoring, initial.corner, 0.5, 0.75);
    let returned = continue_corner(&authoring, forward.corner, 0.75, 0.5);
    for candidate in [&forward, &returned] {
        assert_eq!(candidate.sketch_input, initial.sketch_input);
        assert_eq!(candidate.accepted, initial.accepted);
        assert_eq!(candidate.corner.first.source, initial.corner.first.source);
        assert_eq!(candidate.corner.second.source, initial.corner.second.source);
        assert_eq!(
            candidate.corner.first.normal_side,
            initial.corner.first.normal_side
        );
        assert_eq!(
            candidate.corner.second.normal_side,
            initial.corner.second.normal_side
        );
        assert_eq!(
            candidate.corner.first.retained_endpoint,
            initial.corner.first.retained_endpoint
        );
        assert_eq!(
            candidate.corner.second.retained_endpoint,
            initial.corner.second.retained_endpoint
        );
        assert_eq!(
            candidate.corner.endpoint_order,
            initial.corner.endpoint_order
        );
        assert_eq!(candidate.corner.sweep, initial.corner.sweep);
    }
    for axis in 0..2 {
        assert_close(returned.arc.center[axis], initial.arc.center[axis], 1.0e-9);
    }
    for parent in 0..2 {
        assert_close(
            returned.arc.contacts[parent].total_parameter,
            initial.arc.contacts[parent].total_parameter,
            1.0e-9,
        );
        for axis in 0..2 {
            assert_close(
                returned.arc.contacts[parent].position[axis],
                initial.arc.contacts[parent].position[axis],
                1.0e-9,
            );
        }
    }
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one derivative oracle matrix keeps family, scale, transform, order and round-trip coverage together"
)]
fn analytic_radius_sensitivity_matches_central_finite_differences() {
    let line_line_cases = [
        (
            TestSimilarity::IDENTITY,
            std::f64::consts::FRAC_PI_2,
            false,
            0x2400,
        ),
        (
            TestSimilarity {
                scale: 1.0e-3,
                angle: 0.37,
                translation: [120.0, -85.0],
            },
            std::f64::consts::FRAC_PI_3,
            false,
            0x2401,
        ),
        (
            TestSimilarity {
                scale: 1.0e3,
                angle: -0.91,
                translation: [1.0e6, -2.0e6],
            },
            std::f64::consts::FRAC_PI_3,
            true,
            0x2402,
        ),
    ];
    for (similarity, turn_angle, reverse_parent_order, document_id) in line_line_cases {
        let (document, request) =
            line_line_similarity_fixture(similarity, turn_angle, reverse_parent_order, document_id);
        let session = retained(document);
        let authoring = crate::ComputedFeatureAuthoringSnapshot::capture(&session).unwrap();
        let start_radius = 0.5 * similarity.scale;
        let initial = complete(
            authoring
                .resolve_fillet_corner(
                    request,
                    start_radius,
                    ComputedFeatureEvaluationPolicy::default(),
                    OperationControl::unlimited(),
                )
                .unwrap(),
        );
        assert_radius_sensitivity_matches_finite_difference(
            &authoring,
            initial.corner,
            start_radius,
        );
        let forward = assert_radius_sensitivity_matches_finite_difference(
            &authoring,
            initial.corner,
            0.75 * similarity.scale,
        );
        let returned = assert_radius_sensitivity_matches_finite_difference(
            &authoring,
            forward.corner,
            start_radius,
        );
        let round_trip_tolerance = 5.0e-8 * similarity.scale.max(1.0e-3);
        for axis in 0..2 {
            assert_close(
                returned.arc.center[axis],
                initial.arc.center[axis],
                round_trip_tolerance,
            );
        }
    }

    for (similarity, document_id) in [
        (TestSimilarity::IDENTITY, 0x2500),
        (
            TestSimilarity {
                scale: 0.02,
                angle: 0.83,
                translation: [20.0, -30.0],
            },
            0x2501,
        ),
    ] {
        let fixture = line_circle_fixture_with_similarity(similarity, document_id);
        let session = retained(fixture.document.clone());
        let authoring = crate::ComputedFeatureAuthoringSnapshot::capture(&session).unwrap();
        let radius = 0.75 * similarity.scale;
        let line_circle = complete(
            authoring
                .resolve_fillet_corner(
                    separated_line_circle_request(&fixture),
                    radius,
                    ComputedFeatureEvaluationPolicy::default(),
                    OperationControl::unlimited(),
                )
                .unwrap(),
        );
        assert_radius_sensitivity_matches_finite_difference(&authoring, line_circle.corner, radius);
    }

    for (similarity, document_id) in [
        (TestSimilarity::IDENTITY, 0x2600),
        (
            TestSimilarity {
                scale: 50.0,
                angle: -0.43,
                translation: [-200.0, 300.0],
            },
            0x2601,
        ),
    ] {
        let fixture = line_quadratic_fixture_with_similarity(similarity, document_id);
        let session = retained(fixture.document.clone());
        let authoring = crate::ComputedFeatureAuthoringSnapshot::capture(&session).unwrap();
        let radius = 0.5 * similarity.scale;
        let line_quadratic = complete(
            authoring
                .resolve_fillet_corner(
                    fixture.request,
                    radius,
                    ComputedFeatureEvaluationPolicy::default(),
                    OperationControl::unlimited(),
                )
                .unwrap(),
        );
        assert_radius_sensitivity_matches_finite_difference(
            &authoring,
            line_quadratic.corner,
            radius,
        );
    }
}

#[test]
fn line_circle_roots_and_retained_directions_share_the_same_branch_safe_rail() {
    let fixture = line_circle_fixture();
    let session = retained(fixture.document.clone());
    let authoring = crate::ComputedFeatureAuthoringSnapshot::capture(&session).unwrap();
    let endpoints = [
        DocumentFilletTrimEndpoint::Start,
        DocumentFilletTrimEndpoint::End,
    ];

    for center_x in [-1.5_f64.sqrt(), 1.5_f64.sqrt()] {
        for line_retained_endpoint in endpoints {
            for circle_retained_endpoint in endpoints {
                let initial = complete(
                    authoring
                        .resolve_fillet_corner(
                            line_circle_branch_request_with_retention(
                                &fixture,
                                center_x,
                                line_retained_endpoint,
                                circle_retained_endpoint,
                            ),
                            0.75,
                            ComputedFeatureEvaluationPolicy::default(),
                            OperationControl::unlimited(),
                        )
                        .unwrap(),
                );
                assert_eq!(
                    initial.corner.first.retained_endpoint,
                    line_retained_endpoint
                );
                assert_eq!(
                    initial.corner.second.retained_endpoint,
                    circle_retained_endpoint
                );
                assert!(initial.arc.center[0] * center_x > 0.0);

                let current = assert_radius_sensitivity_matches_finite_difference(
                    &authoring,
                    initial.corner,
                    0.75,
                );
                let forward = continue_corner(&authoring, current.corner, 0.75, 1.0);
                let returned = continue_corner(&authoring, forward.corner, 1.0, 0.75);
                for candidate in [&current, &forward, &returned] {
                    assert_eq!(candidate.sketch_input, initial.sketch_input);
                    assert_eq!(candidate.accepted, initial.accepted);
                    assert_eq!(
                        candidate.corner.first.retained_endpoint,
                        line_retained_endpoint
                    );
                    assert_eq!(
                        candidate.corner.second.retained_endpoint,
                        circle_retained_endpoint
                    );
                    assert_eq!(
                        candidate.corner.first.normal_side,
                        initial.corner.first.normal_side
                    );
                    assert_eq!(
                        candidate.corner.second.normal_side,
                        initial.corner.second.normal_side
                    );
                    assert_eq!(
                        candidate.corner.first.neighborhood,
                        initial.corner.first.neighborhood
                    );
                    assert_eq!(
                        candidate.corner.second.neighborhood,
                        initial.corner.second.neighborhood
                    );
                    assert_eq!(
                        candidate.corner.endpoint_order,
                        initial.corner.endpoint_order
                    );
                    assert_eq!(candidate.corner.sweep, initial.corner.sweep);
                    assert!(candidate.arc.center[0] * center_x > 0.0);
                }
                for axis in 0..2 {
                    assert_close(returned.arc.center[axis], initial.arc.center[axis], 1.0e-9);
                }
                for parent in 0..2 {
                    assert_close(
                        returned.arc.contacts[parent].total_parameter,
                        initial.arc.contacts[parent].total_parameter,
                        1.0e-9,
                    );
                }
            }
        }
    }
}

#[test]
fn exact_line_circle_contact_ties_reject_ambiguous_retained_direction() {
    let fixture = line_circle_fixture();
    let session = retained(fixture.document.clone());
    let authoring = crate::ComputedFeatureAuthoringSnapshot::capture(&session).unwrap();
    let initial = complete(
        authoring
            .resolve_fillet_corner(
                separated_line_circle_request(&fixture),
                0.75,
                ComputedFeatureEvaluationPolicy::default(),
                OperationControl::unlimited(),
            )
            .unwrap(),
    );
    let mut picks = [
        curve_pick(
            &fixture.document,
            fixture.line,
            initial.corner.first.picked_parameter,
            initial.corner.first.retained_endpoint,
        ),
        curve_pick(
            &fixture.document,
            fixture.circle,
            initial.corner.second.picked_parameter,
            initial.corner.second.retained_endpoint,
        ),
    ];

    for ambiguous_parent in 0..2 {
        picks[ambiguous_parent].retained_endpoint_hint = None;
        let request = ComputedFilletCornerAuthoringRequest {
            first: picks[0],
            second: picks[1],
            options: ComputedFilletAuthoringOptions::default(),
        };
        assert!(matches!(
            authoring.resolve_fillet_corner(
                request,
                0.75,
                ComputedFeatureEvaluationPolicy::default(),
                OperationControl::unlimited(),
            ),
            Err(ComputedFeatureAuthoringError::AmbiguousRetainedEndpoint)
        ));
        picks[ambiguous_parent].retained_endpoint_hint = Some(if ambiguous_parent == 0 {
            initial.corner.first.retained_endpoint
        } else {
            initial.corner.second.retained_endpoint
        });
    }
}

#[test]
fn exact_fold_numeric_edit_departs_only_through_the_persisted_branch_cell() {
    let fixture = line_circle_fixture();
    let session = retained(fixture.document.clone());
    let authoring = crate::ComputedFeatureAuthoringSnapshot::capture(&session).unwrap();

    for branch_sign in [-1.0, 1.0] {
        let fold = complete(
            authoring
                .resolve_fillet_corner(
                    line_circle_branch_request(&fixture, branch_sign * 1.5_f64.sqrt()),
                    0.5,
                    ComputedFeatureEvaluationPolicy::default(),
                    OperationControl::unlimited(),
                )
                .unwrap(),
        );
        assert_close(fold.arc.center[0], 0.0, 1.0e-6);

        let ordinary = authoring.continue_fillet_corner(
            fold.corner,
            0.5,
            0.55,
            ComputedFeatureEvaluationPolicy::default(),
            OperationControl::unlimited(),
        );
        assert!(
            matches!(
                ordinary,
                Err(ComputedFeatureAuthoringError::IllConditionedRadiusSensitivity)
            ),
            "ordinary continuation unexpectedly departed the {branch_sign:+} fold: {ordinary:?}"
        );

        let mut continued = complete(
            authoring
                .continue_fillet_corners_numeric(
                    &[fold.corner],
                    0.5,
                    0.55,
                    ComputedFeatureEvaluationPolicy::default(),
                    OperationControl::unlimited(),
                )
                .unwrap(),
        );
        let continued = continued.pop().expect("one numeric continuation");
        assert_eq!(continued.arc.radius.to_bits(), 0.55_f64.to_bits());
        assert!(continued.arc.center[0] * branch_sign > 0.0);
        assert!(
            continued
                .sensitivity
                .center_derivative
                .into_iter()
                .chain(continued.sensitivity.contact_parameter_derivatives)
                .all(f64::is_finite)
        );
        for (before, after) in [
            (fold.corner.first, continued.corner.first),
            (fold.corner.second, continued.corner.second),
        ] {
            assert_eq!(after.source, before.source);
            assert_eq!(after.neighborhood, before.neighborhood);
            assert_eq!(after.normal_side, before.normal_side);
            assert_eq!(after.retained_endpoint, before.retained_endpoint);
            assert_eq!(after.winding, before.winding);
        }
        assert_eq!(continued.corner.endpoint_order, fold.corner.endpoint_order);
        assert_eq!(continued.corner.sweep, fold.corner.sweep);
        assert_eq!(continued.sketch_input, fold.sketch_input);
        assert_eq!(continued.accepted, fold.accepted);

        let return_to_fold = authoring.continue_fillet_corner(
            continued.corner,
            0.55,
            0.5,
            ComputedFeatureEvaluationPolicy::default(),
            OperationControl::unlimited(),
        );
        assert!(
            matches!(
                return_to_fold,
                Err(ComputedFeatureAuthoringError::IllConditionedRadiusSensitivity)
            ),
            "ordinary continuation unexpectedly crossed into the exact fold: {return_to_fold:?}"
        );
        let below_fold = authoring.continue_fillet_corners_numeric(
            &[fold.corner],
            0.5,
            0.49,
            ComputedFeatureEvaluationPolicy::default(),
            OperationControl::unlimited(),
        );
        assert!(
            matches!(
                below_fold,
                Err(
                    ComputedFeatureAuthoringError::IllConditionedRadiusSensitivity
                        | ComputedFeatureAuthoringError::NoLocalRoot
                )
            ),
            "numeric continuation unexpectedly crossed the geometric fold: {below_fold:?}"
        );
    }
}

#[test]
fn line_circle_continuation_stops_at_the_certified_fold_without_root_hop() {
    let fixture = line_circle_fixture();
    let session = retained(fixture.document.clone());
    let authoring = crate::ComputedFeatureAuthoringSnapshot::capture(&session).unwrap();
    let barrier = 1.5 * std::f64::consts::PI;

    for center_x in [-1.5_f64.sqrt(), 1.5_f64.sqrt()] {
        let initial = complete(
            authoring
                .resolve_fillet_corner(
                    line_circle_branch_request(&fixture, center_x),
                    0.75,
                    ComputedFeatureEvaluationPolicy::default(),
                    OperationControl::unlimited(),
                )
                .unwrap(),
        );
        let expanded = continue_corner(&authoring, initial.corner, 0.75, 1.25);
        let returned = continue_corner(&authoring, expanded.corner, 1.25, 0.55);

        assert_eq!(
            returned.corner.second.neighborhood,
            initial.corner.second.neighborhood
        );
        assert_eq!(
            returned.corner.second.winding,
            initial.corner.second.winding
        );
        assert!(returned.arc.center[0] * center_x > 0.0);
        if center_x.is_sign_negative() {
            assert!(returned.arc.contacts[1].total_parameter < barrier);
        } else {
            assert!(returned.arc.contacts[1].total_parameter > barrier);
        }

        let fold = authoring.continue_fillet_corner(
            returned.corner,
            0.55,
            0.5,
            ComputedFeatureEvaluationPolicy::default(),
            OperationControl::unlimited(),
        );
        assert!(
            matches!(
                fold,
                Err(ComputedFeatureAuthoringError::IllConditionedRadiusSensitivity)
            ),
            "unexpected exact-fold result: {fold:?}"
        );
        let beyond = authoring.continue_fillet_corner(
            returned.corner,
            0.55,
            0.49,
            ComputedFeatureEvaluationPolicy::default(),
            OperationControl::unlimited(),
        );
        assert!(
            matches!(
                beyond,
                Err(ComputedFeatureAuthoringError::IllConditionedRadiusSensitivity)
            ),
            "unexpected beyond-fold result: {beyond:?}"
        );
    }
}

#[test]
fn high_curvature_quadratic_continuation_stops_before_a_remote_root_hop() {
    let fixture = high_curvature_quadratic_fixture();
    let session = retained(fixture.document.clone());
    let authoring = crate::ComputedFeatureAuthoringSnapshot::capture(&session).unwrap();

    let initial = continue_corner(&authoring, fixture.prior, 0.5, 0.5);
    let at_045 = continue_corner(&authoring, initial.corner, 0.5, 0.45);
    let at_043 = continue_corner(&authoring, at_045.corner, 0.45, 0.43);
    assert_close(at_045.arc.contacts[1].total_parameter, 0.567_824, 2.0e-5);
    assert_close(at_043.arc.contacts[1].total_parameter, 0.639_311, 2.0e-5);

    let returned = continue_corner(&authoring, at_043.corner, 0.43, 0.5);
    assert_close(returned.arc.contacts[1].total_parameter, 0.5, 2.0e-8);
    assert_eq!(
        returned.corner.second.normal_side,
        fixture.prior.second.normal_side
    );
    assert_eq!(
        returned.corner.second.neighborhood,
        fixture.prior.second.neighborhood
    );

    let mut current_features = ComputedFeatureDocument::new(fixture.document.id());
    let current_feature = current_features
        .create_fillet_set("exact local root", 0.43, vec![at_043.corner])
        .unwrap();
    let mut current_allocator = ComputedEvaluationAllocator::default();
    let current_snapshot = evaluate(&session, &current_features, &mut current_allocator);
    assert!(matches!(
        current_snapshot
            .feature_evaluations()
            .iter()
            .find(|evaluation| evaluation.feature == current_feature)
            .unwrap()
            .state,
        ComputedFeatureEvaluationState::Current { .. }
    ));

    let beyond_fold = authoring.continue_fillet_corner(
        at_043.corner,
        0.43,
        0.42,
        ComputedFeatureEvaluationPolicy::default(),
        OperationControl::unlimited(),
    );
    assert!(
        matches!(
            beyond_fold,
            Err(ComputedFeatureAuthoringError::IllConditionedRadiusSensitivity)
        ),
        "unexpected high-curvature fold result: {beyond_fold:?}"
    );

    let mut features = ComputedFeatureDocument::new(fixture.document.id());
    let feature = features
        .create_fillet_set("remote-root guard", 0.42, vec![fixture.prior])
        .unwrap();
    let mut allocator = ComputedEvaluationAllocator::default();
    let snapshot = evaluate(&session, &features, &mut allocator);
    assert!(arc_centers(&snapshot).is_empty());
    assert!(matches!(
        snapshot
            .feature_evaluations()
            .iter()
            .find(|evaluation| evaluation.feature == feature)
            .unwrap()
            .state,
        ComputedFeatureEvaluationState::Failed {
            failure: ComputedFeatureFailure::NoLocalRoot { .. }
        }
    ));
}

#[test]
fn modest_quadratic_source_edit_keeps_persisted_and_continued_contacts_seed_local() {
    let mut fixture = high_curvature_quadratic_fixture();
    let mut features = ComputedFeatureDocument::new(fixture.document.id());
    let feature = features
        .create_fillet_set("edited source", 0.5, vec![fixture.prior])
        .unwrap();
    fixture
        .document
        .set_point_position(fixture.controls[1], [-0.49, 0.5])
        .unwrap();
    let session = retained(fixture.document);
    let mut allocator = ComputedEvaluationAllocator::default();
    let snapshot = evaluate(&session, &features, &mut allocator);
    let state = &snapshot
        .feature_evaluations()
        .iter()
        .find(|evaluation| evaluation.feature == feature)
        .unwrap()
        .state;
    assert!(
        matches!(state, ComputedFeatureEvaluationState::Current { .. }),
        "modest source edit lost the persisted local root: {state:?}"
    );

    let authoring = crate::ComputedFeatureAuthoringSnapshot::capture(&session).unwrap();
    let corrected = continue_corner(&authoring, fixture.prior, 0.5, 0.49);
    assert!((corrected.arc.contacts[1].total_parameter - 0.5).abs() < 0.1);
    assert_eq!(
        corrected.corner.second.neighborhood,
        fixture.prior.second.neighborhood
    );
}

#[test]
fn grouped_radius_continuation_is_ordered_bounded_and_all_or_nothing() {
    let fixture = polyline_fixture();
    let session = retained(fixture.document.clone());
    let authoring = crate::ComputedFeatureAuthoringSnapshot::capture(&session).unwrap();
    let priors = [first_corner(fixture.spans), second_corner(fixture.spans)];
    let continued = complete(
        authoring
            .continue_fillet_corners(
                &priors,
                0.5,
                0.75,
                ComputedFeatureEvaluationPolicy::default(),
                OperationControl::unlimited(),
            )
            .unwrap(),
    );
    assert_eq!(continued.len(), 2);
    assert_eq!(continued[0].corner.first.source, priors[0].first.source);
    assert_eq!(continued[1].corner.first.source, priors[1].first.source);

    let first_only = authoring
        .continue_fillet_corner(
            priors[0],
            0.5,
            0.75,
            ComputedFeatureEvaluationPolicy::default(),
            OperationControl::unlimited(),
        )
        .unwrap();
    let mut later_stop_limits = OperationLimits::unlimited();
    later_stop_limits.profile_subdivisions = first_only.report().consumed.profile_subdivisions;
    assert!(matches!(
        authoring
            .continue_fillet_corners(
                &priors,
                0.5,
                0.75,
                ComputedFeatureEvaluationPolicy::default(),
                OperationControl::new(CancellationToken::default(), later_stop_limits),
            )
            .unwrap(),
        OperationOutcome::WorkExhausted { .. }
    ));
    let mut invalid_second = priors[1];
    invalid_second.second.source = invalid_second.first.source;
    assert!(matches!(
        authoring.continue_fillet_corners(
            &[priors[0], invalid_second],
            0.5,
            0.75,
            ComputedFeatureEvaluationPolicy::default(),
            OperationControl::unlimited(),
        ),
        Err(ComputedFeatureAuthoringError::InvalidContinuationState)
    ));

    let mut limits = OperationLimits::unlimited();
    limits.document_validation_items = 3;
    assert!(matches!(
        authoring
            .continue_fillet_corners(
                &priors,
                0.5,
                0.75,
                ComputedFeatureEvaluationPolicy::default(),
                OperationControl::new(CancellationToken::default(), limits),
            )
            .unwrap(),
        OperationOutcome::WorkExhausted { .. }
    ));
    let (handle, token) = cancellation_pair();
    handle.cancel();
    assert!(matches!(
        authoring
            .continue_fillet_corners(
                &priors,
                0.5,
                0.75,
                ComputedFeatureEvaluationPolicy::default(),
                OperationControl::new(token, OperationLimits::unlimited()),
            )
            .unwrap(),
        OperationOutcome::Cancelled { .. }
    ));
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "the reseed test keeps deterministic root choice, parent naming, branch invariants and rejection in one fixture"
)]
fn exact_contact_reseed_selects_named_line_circle_root_without_branch_drift() {
    let fixture = line_circle_fixture();
    let session = retained(fixture.document.clone());
    let authoring = crate::ComputedFeatureAuthoringSnapshot::capture(&session).unwrap();
    let initial = complete(
        authoring
            .resolve_fillet_corner(
                separated_line_circle_request(&fixture),
                0.75,
                ComputedFeatureEvaluationPolicy::default(),
                OperationControl::unlimited(),
            )
            .unwrap(),
    );
    let request = ComputedFilletContactReseedRequest {
        prior: initial.corner,
        parent: ComputedFilletParentIndex::Second,
        parameter: 5.5,
    };
    let reseeded = complete(
        authoring
            .reseed_fillet_contact(
                request,
                0.75,
                ComputedFeatureEvaluationPolicy::default(),
                OperationControl::unlimited(),
            )
            .unwrap(),
    );
    let repeated = complete(
        authoring
            .reseed_fillet_contact(
                request,
                0.75,
                ComputedFeatureEvaluationPolicy::default(),
                OperationControl::unlimited(),
            )
            .unwrap(),
    );
    assert_eq!(reseeded, repeated);
    let reversed_prior = NewComputedFilletCorner {
        first: initial.corner.second,
        second: initial.corner.first,
        endpoint_order: match initial.corner.endpoint_order {
            DocumentFilletEndpointOrder::FirstThenSecond => {
                DocumentFilletEndpointOrder::SecondThenFirst
            }
            DocumentFilletEndpointOrder::SecondThenFirst => {
                DocumentFilletEndpointOrder::FirstThenSecond
            }
        },
        sweep: initial.corner.sweep,
    };
    let reversed_reseed = complete(
        authoring
            .reseed_fillet_contact(
                ComputedFilletContactReseedRequest {
                    prior: reversed_prior,
                    parent: ComputedFilletParentIndex::First,
                    parameter: 5.5,
                },
                0.75,
                ComputedFeatureEvaluationPolicy::default(),
                OperationControl::unlimited(),
            )
            .unwrap(),
    );
    assert_eq!(reversed_reseed, reseeded);
    let mut wound_prior = initial.corner;
    wound_prior.second.winding = 2;
    let geosolve_sketch::ContactNeighborhood::Local { lower, upper } =
        wound_prior.second.neighborhood
    else {
        panic!("line-circle authoring must certify a local periodic branch");
    };
    wound_prior.second.neighborhood = geosolve_sketch::ContactNeighborhood::Local {
        lower: lower + 2.0 * std::f64::consts::TAU,
        upper: upper + 2.0 * std::f64::consts::TAU,
    };
    wound_prior.second.periodic_anchor = wound_prior.second.periodic_anchor.map(|mut anchor| {
        anchor.winding += 2;
        anchor
    });
    let wound_reseed = complete(
        authoring
            .reseed_fillet_contact(
                ComputedFilletContactReseedRequest {
                    prior: wound_prior,
                    parent: ComputedFilletParentIndex::Second,
                    parameter: 5.5,
                },
                0.75,
                ComputedFeatureEvaluationPolicy::default(),
                OperationControl::unlimited(),
            )
            .unwrap(),
    );
    assert_eq!(wound_reseed.corner.second.winding, 2);
    assert_close(
        wound_reseed.arc.contacts[1].total_parameter,
        reseeded.arc.contacts[1].total_parameter + 2.0 * std::f64::consts::TAU,
        1.0e-9,
    );
    assert_eq!(reseeded.sketch_input, initial.sketch_input);
    assert_eq!(reseeded.accepted, initial.accepted);
    assert!(
        (reseeded.arc.contacts[1].total_parameter - initial.arc.contacts[1].total_parameter).abs()
            > 1.0
    );
    assert!((reseeded.arc.contacts[1].total_parameter - 5.5).abs() < 0.1);
    for parent in 0..2 {
        let before = [initial.corner.first, initial.corner.second][parent];
        let after = [reseeded.corner.first, reseeded.corner.second][parent];
        assert_eq!(after.source, before.source);
        assert_eq!(after.normal_side, before.normal_side);
        assert_eq!(after.retained_endpoint, before.retained_endpoint);
    }
    assert_eq!(
        reseeded.corner.endpoint_order,
        initial.corner.endpoint_order
    );
    assert_eq!(reseeded.corner.sweep, initial.corner.sweep);

    for parameter in [f64::NAN, -0.1, std::f64::consts::TAU] {
        assert!(matches!(
            authoring.reseed_fillet_contact(
                ComputedFilletContactReseedRequest {
                    prior: initial.corner,
                    parent: ComputedFilletParentIndex::Second,
                    parameter,
                },
                0.75,
                ComputedFeatureEvaluationPolicy::default(),
                OperationControl::unlimited(),
            ),
            Err(ComputedFeatureAuthoringError::InvalidContactReseed)
        ));
    }
}

#[test]
fn local_corner_alternatives_are_bounded_deterministic_and_finite() {
    let fixture = polyline_fixture();
    let session = retained(fixture.document.clone());
    let authoring = crate::ComputedFeatureAuthoringSnapshot::capture(&session).unwrap();
    let prior = first_corner(fixture.spans);
    let resolve = || {
        complete(
            authoring
                .local_fillet_corner_alternatives(
                    prior,
                    0.75,
                    ComputedFeatureEvaluationPolicy::default(),
                    OperationControl::unlimited(),
                )
                .unwrap(),
        )
    };
    let first = resolve();
    let second = resolve();
    assert_eq!(first, second);
    assert!(!first.is_empty() && first.len() <= 7);
    assert!(
        first
            .iter()
            .any(|value| value.kind == ComputedFilletCornerAlternativeKind::Current)
    );
    assert!(first.iter().any(|value| matches!(
        value.kind,
        ComputedFilletCornerAlternativeKind::RetainedEndpoint {
            parent: ComputedFilletParentIndex::First,
            ..
        }
    )));
    assert!(first.iter().any(|value| matches!(
        value.kind,
        ComputedFilletCornerAlternativeKind::RetainedEndpoint {
            parent: ComputedFilletParentIndex::Second,
            ..
        }
    )));
    assert!(
        first
            .iter()
            .any(|value| { value.kind == ComputedFilletCornerAlternativeKind::ComplementaryArc })
    );
    for alternative in first {
        assert!(alternative.resolved.arc.radius.is_finite());
        assert!(
            alternative
                .resolved
                .arc
                .center
                .into_iter()
                .all(f64::is_finite)
        );
        assert!(
            alternative
                .resolved
                .sensitivity
                .center_derivative
                .into_iter()
                .all(f64::is_finite)
        );
        assert!(
            alternative
                .resolved
                .sensitivity
                .contact_parameter_derivatives
                .into_iter()
                .all(f64::is_finite)
        );
    }

    let mut limits = OperationLimits::unlimited();
    limits.document_validation_items = 1;
    assert!(matches!(
        authoring
            .local_fillet_corner_alternatives(
                prior,
                0.75,
                ComputedFeatureEvaluationPolicy::default(),
                OperationControl::new(CancellationToken::default(), limits),
            )
            .unwrap(),
        OperationOutcome::WorkExhausted { .. }
    ));
    let (handle, token) = cancellation_pair();
    handle.cancel();
    assert!(matches!(
        authoring
            .local_fillet_corner_alternatives(
                prior,
                0.75,
                ComputedFeatureEvaluationPolicy::default(),
                OperationControl::new(token, OperationLimits::unlimited()),
            )
            .unwrap(),
        OperationOutcome::Cancelled { .. }
    ));
}

#[test]
fn radius_rail_rejects_ill_conditioned_and_invalid_continuation_state() {
    let mut document = SketchDocument::with_id(
        10.0,
        geosolve_sketch::DocumentId(geosolve_sketch::PersistentId::from_u128(0x2300)),
    )
    .unwrap();
    let points = [
        document.add_point("first start", [-1_000.0, 0.0]).unwrap(),
        document.add_point("first end", [1_000.0, 0.0]).unwrap(),
        document
            .add_point("second start", [-1_000.0, -0.0005])
            .unwrap(),
        document.add_point("second end", [1_000.0, 0.0005]).unwrap(),
    ];
    let spans = [
        CurveSpan::line(
            document
                .add_curve(
                    "first",
                    CurveDefinition::Line {
                        start: points[0],
                        end: points[1],
                        branch_direction: [1.0, 0.0],
                    },
                )
                .unwrap(),
        ),
        CurveSpan::line(
            document
                .add_curve(
                    "second",
                    CurveDefinition::Line {
                        start: points[2],
                        end: points[3],
                        branch_direction: {
                            let norm = 1.0_f64.hypot(5.0e-7);
                            [1.0 / norm, 5.0e-7 / norm]
                        },
                    },
                )
                .unwrap(),
        ),
    ];
    let session = retained(document);
    let authoring = crate::ComputedFeatureAuthoringSnapshot::capture(&session).unwrap();
    let prior = NewComputedFilletCorner {
        first: parent(
            spans[0],
            0.5,
            DocumentCurveNormalSide::Left,
            DocumentFilletTrimEndpoint::End,
        ),
        second: parent(
            spans[1],
            0.5,
            DocumentCurveNormalSide::Left,
            DocumentFilletTrimEndpoint::Start,
        ),
        endpoint_order: DocumentFilletEndpointOrder::FirstThenSecond,
        sweep: DocumentArcSweep::CounterClockwise,
    };
    let result = authoring.continue_fillet_corner(
        prior,
        0.5,
        0.5,
        ComputedFeatureEvaluationPolicy::default(),
        OperationControl::unlimited(),
    );
    assert!(
        matches!(
            result,
            Err(ComputedFeatureAuthoringError::IllConditionedRadiusSensitivity)
        ),
        "unexpected radius-rail result: {result:?}"
    );

    let mut invalid = prior;
    invalid.second.source = invalid.first.source;
    assert!(matches!(
        authoring.continue_fillet_corner(
            invalid,
            0.5,
            0.5,
            ComputedFeatureEvaluationPolicy::default(),
            OperationControl::unlimited(),
        ),
        Err(ComputedFeatureAuthoringError::InvalidContinuationState)
    ));
}

#[test]
fn authoring_root_work_stops_are_outcomes_not_no_root_errors() {
    let fixture = polyline_fixture();
    let session = retained(fixture.document.clone());
    let authoring = crate::ComputedFeatureAuthoringSnapshot::capture(&session).unwrap();
    let request = first_corner_authoring_request(&fixture.document, fixture.spans);

    let mut limits = OperationLimits::unlimited();
    limits.profile_subdivisions = 0;
    assert!(matches!(
        authoring
            .resolve_fillet_corner(
                request,
                0.5,
                ComputedFeatureEvaluationPolicy::default(),
                OperationControl::new(CancellationToken::default(), limits),
            )
            .unwrap(),
        OperationOutcome::WorkExhausted { .. }
    ));

    let (handle, token) = cancellation_pair();
    handle.cancel();
    assert!(matches!(
        authoring
            .resolve_fillet_corner(
                request,
                0.5,
                ComputedFeatureEvaluationPolicy::default(),
                OperationControl::new(token, OperationLimits::unlimited()),
            )
            .unwrap(),
        OperationOutcome::Cancelled { .. }
    ));
}

#[test]
fn invalid_feature_document_is_rejected_during_snapshot_capture() {
    let fixture = polyline_fixture();
    let session = retained(fixture.document.clone());
    let invalid = ComputedFeatureDocument::with_id(
        fixture.document.id(),
        ComputedFeatureDocumentId::from_raw(0),
    );
    assert!(matches!(
        ComputedFeatureEvaluationSnapshot::capture(
            &session,
            &invalid,
            ComputedFeatureEvaluationPolicy::default(),
        ),
        Err(ComputedFeatureSnapshotError::InvalidFeatureDocument(
            ComputedFeatureDocumentError::InvalidField {
                field: "document_id",
                ..
            }
        ))
    ));
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "the exact and near-parallel authoring/evaluation cases deliberately share one complete fixture"
)]
fn parallel_parents_are_typed_in_authoring_and_persistent_evaluation() {
    for (document_id, second_rise) in [(0x3000, 0.0), (0x3001, 1.0e-10)] {
        let mut document = SketchDocument::with_id(
            10.0,
            geosolve_sketch::DocumentId(geosolve_sketch::PersistentId::from_u128(document_id)),
        )
        .unwrap();
        let first_start = document.add_point("first start", [0.0, 0.0]).unwrap();
        let first_end = document.add_point("first end", [4.0, 0.0]).unwrap();
        let second_start = document.add_point("second start", [0.0, 1.0]).unwrap();
        let second_end = document
            .add_point("second end", [4.0, 1.0 + second_rise])
            .unwrap();
        let first = CurveSpan::line(
            document
                .add_curve(
                    "first",
                    CurveDefinition::Line {
                        start: first_start,
                        end: first_end,
                        branch_direction: [1.0, 0.0],
                    },
                )
                .unwrap(),
        );
        let second = CurveSpan::line(
            document
                .add_curve(
                    "second",
                    CurveDefinition::Line {
                        start: second_start,
                        end: second_end,
                        branch_direction: [1.0, 0.0],
                    },
                )
                .unwrap(),
        );
        let first_jet = document.evaluate_curve_jet(first, 0.5).unwrap();
        let second_jet = document.evaluate_curve_jet(second, 0.5).unwrap();
        let request = ComputedFilletCornerAuthoringRequest {
            first: ComputedFilletCurvePick {
                source: source(first),
                parameter: 0.5,
                model_position: [first_jet.position.x, first_jet.position.y],
                retained_endpoint_hint: Some(DocumentFilletTrimEndpoint::End),
            },
            second: ComputedFilletCurvePick {
                source: source(second),
                parameter: 0.5,
                model_position: [second_jet.position.x, second_jet.position.y],
                retained_endpoint_hint: Some(DocumentFilletTrimEndpoint::Start),
            },
            options: ComputedFilletAuthoringOptions::default(),
        };
        let session = retained(document.clone());
        let authoring = crate::ComputedFeatureAuthoringSnapshot::capture(&session).unwrap();
        let authoring_result = authoring.resolve_fillet_corner(
            request,
            0.5,
            ComputedFeatureEvaluationPolicy::default(),
            OperationControl::unlimited(),
        );
        assert!(
            matches!(
                authoring_result,
                Err(ComputedFeatureAuthoringError::SingularParents)
            ),
            "unexpected parallel-parent authoring result: {authoring_result:?}"
        );

        let mut features = ComputedFeatureDocument::new(document.id());
        let feature = features
            .create_fillet_set(
                "parallel",
                0.5,
                vec![NewComputedFilletCorner {
                    first: parent(
                        first,
                        0.5,
                        DocumentCurveNormalSide::Left,
                        DocumentFilletTrimEndpoint::End,
                    ),
                    second: parent(
                        second,
                        0.5,
                        DocumentCurveNormalSide::Right,
                        DocumentFilletTrimEndpoint::Start,
                    ),
                    endpoint_order: DocumentFilletEndpointOrder::FirstThenSecond,
                    sweep: DocumentArcSweep::CounterClockwise,
                }],
            )
            .unwrap();
        let mut allocator = ComputedEvaluationAllocator::default();
        let snapshot = evaluate(&session, &features, &mut allocator);
        assert!(matches!(
            snapshot
                .feature_evaluations()
                .iter()
                .find(|value| value.feature == feature)
                .unwrap()
                .state,
            ComputedFeatureEvaluationState::Failed {
                failure: ComputedFeatureFailure::SingularParents { .. }
            }
        ));
    }
}

#[test]
fn affine_non_affine_authoring_certifies_and_persists_periodic_branch_state() {
    let fixture = line_circle_fixture();
    let session = retained(fixture.document.clone());
    let authoring = crate::ComputedFeatureAuthoringSnapshot::capture(&session).unwrap();
    let resolved = complete(
        authoring
            .resolve_fillet_corner(
                fixture.request,
                0.5,
                ComputedFeatureEvaluationPolicy::default(),
                OperationControl::unlimited(),
            )
            .unwrap(),
    );
    assert!(resolved.corner.second.periodic_anchor.is_some());
    assert!(matches!(
        resolved.corner.second.neighborhood,
        geosolve_sketch::ContactNeighborhood::Local { .. }
    ));
    assert_eq!(resolved.corner.first.source, source(fixture.line));
    assert_eq!(resolved.corner.second.source, source(fixture.circle));
    let mut features = ComputedFeatureDocument::new(fixture.document.id());
    let feature = features
        .create_fillet_set("line circle", 0.5, vec![resolved.corner])
        .unwrap();
    let mut allocator = ComputedEvaluationAllocator::default();
    let snapshot = evaluate(&session, &features, &mut allocator);
    let state = &snapshot
        .feature_evaluations()
        .iter()
        .find(|value| value.feature == feature)
        .unwrap()
        .state;
    assert!(
        matches!(state, ComputedFeatureEvaluationState::Current { .. }),
        "unexpected affine/non-affine state: {state:?}"
    );
    assert_eq!(arc_centers(&snapshot).len(), 1);
}

#[test]
fn two_non_affine_parents_are_typed_unsupported_without_sketch_mutation() {
    let fixture = two_circle_fixture();
    let session = retained(fixture.document.clone());
    let before_input = session.prepared_input();
    let before_document = session.design_document().clone();
    let before_accepted = session
        .accepted_state()
        .map(geosolve_sketch::SketchAcceptedDocumentState::identity);

    let authoring = crate::ComputedFeatureAuthoringSnapshot::capture(&session).unwrap();
    assert!(matches!(
        authoring.resolve_fillet_corner(
            fixture.request,
            0.5,
            ComputedFeatureEvaluationPolicy::default(),
            OperationControl::unlimited(),
        ),
        Err(ComputedFeatureAuthoringError::UnsupportedCurvedPair)
    ));

    let mut features = ComputedFeatureDocument::new(fixture.document.id());
    let feature = features
        .create_fillet_set(
            "unsupported curved pair",
            0.5,
            vec![NewComputedFilletCorner {
                first: periodic_circle_parent(fixture.spans[0], DocumentCurveNormalSide::Left),
                second: periodic_circle_parent(fixture.spans[1], DocumentCurveNormalSide::Right),
                endpoint_order: DocumentFilletEndpointOrder::FirstThenSecond,
                sweep: DocumentArcSweep::CounterClockwise,
            }],
        )
        .unwrap();
    let feature_before = features.clone();
    let corner = match &features.feature(feature).unwrap().definition {
        ComputedFeatureDefinition::FilletSet(fillet) => fillet.corners[0].id,
    };
    let mut allocator = ComputedEvaluationAllocator::default();
    let snapshot = evaluate(&session, &features, &mut allocator);
    assert!(snapshot.edges().is_empty());
    assert!(matches!(
        snapshot
            .feature_evaluations()
            .iter()
            .find(|value| value.feature == feature)
            .unwrap()
            .state,
        ComputedFeatureEvaluationState::Failed {
            failure: ComputedFeatureFailure::UnsupportedCurvedPair {
                corner: failed_corner,
            }
        } if failed_corner == corner
    ));
    assert_eq!(features, feature_before);
    assert_eq!(session.prepared_input(), before_input);
    assert_eq!(session.design_document(), &before_document);
    assert_eq!(
        session
            .accepted_state()
            .map(geosolve_sketch::SketchAcceptedDocumentState::identity),
        before_accepted
    );
}

#[test]
fn persistent_parent_domains_are_canonical_and_respect_visible_periodic_intervals() {
    let fixture = line_circle_fixture();
    let session = retained(fixture.document.clone());
    let authoring = crate::ComputedFeatureAuthoringSnapshot::capture(&session).unwrap();
    let resolved = complete(
        authoring
            .resolve_fillet_corner(
                fixture.request,
                0.5,
                ComputedFeatureEvaluationPolicy::default(),
                OperationControl::unlimited(),
            )
            .unwrap(),
    );
    assert_eq!(resolved.corner.first.source, source(fixture.line));
    assert_eq!(resolved.corner.second.source, source(fixture.circle));

    let mut bounded_anchor = resolved.corner;
    bounded_anchor.first.periodic_anchor = Some(DocumentTrimParameter {
        parameter: 0.25,
        winding: 0,
    });
    assert!(matches!(
        evaluate_single_corner_failure(&fixture.document, 0.5, bounded_anchor),
        ComputedFeatureFailure::InvalidParentState { .. }
    ));

    let mut noncanonical_periodic = resolved.corner;
    noncanonical_periodic.second.picked_parameter += std::f64::consts::TAU;
    noncanonical_periodic.second.winding -= 1;
    assert!(matches!(
        evaluate_single_corner_failure(&fixture.document, 0.5, noncanonical_periodic),
        ComputedFeatureFailure::InvalidParentState { .. }
    ));

    let mut outside_branch = resolved.corner;
    let geosolve_sketch::ContactNeighborhood::Local { lower, upper } =
        outside_branch.second.neighborhood
    else {
        panic!("circle parent must retain a local branch cell");
    };
    outside_branch.second.neighborhood = geosolve_sketch::ContactNeighborhood::Local {
        lower: lower + std::f64::consts::TAU,
        upper: upper + std::f64::consts::TAU,
    };
    assert!(matches!(
        evaluate_single_corner_failure(&fixture.document, 0.5, outside_branch),
        ComputedFeatureFailure::InvalidParentState { .. }
    ));

    let mut trimmed = fixture.document.clone();
    trimmed
        .replace_trim_views(
            fixture.circle,
            vec![DocumentCurveTrimView {
                support: fixture.circle,
                start: DocumentTrimBoundary::Fixed(DocumentTrimParameter {
                    parameter: 0.5,
                    winding: 0,
                }),
                end: DocumentTrimBoundary::Fixed(DocumentTrimParameter {
                    parameter: 2.0,
                    winding: 0,
                }),
            }],
        )
        .unwrap();
    assert!(matches!(
        evaluate_single_corner_failure(&trimmed, 0.5, resolved.corner),
        ComputedFeatureFailure::InvalidParentState { .. }
    ));
}

#[test]
fn singular_curved_offset_remains_typed_in_persistent_evaluation() {
    let fixture = line_circle_fixture();
    let session = retained(fixture.document.clone());
    let authoring = crate::ComputedFeatureAuthoringSnapshot::capture(&session).unwrap();
    let mut corner = complete(
        authoring
            .resolve_fillet_corner(
                fixture.request,
                0.5,
                ComputedFeatureEvaluationPolicy::default(),
                OperationControl::unlimited(),
            )
            .unwrap(),
    )
    .corner;
    assert_eq!(corner.second.source, source(fixture.circle));
    corner.second.normal_side = DocumentCurveNormalSide::Left;
    assert!(matches!(
        evaluate_single_corner_failure(&fixture.document, 1.0, corner),
        ComputedFeatureFailure::OffsetSingularity { .. }
    ));
}

#[test]
fn endpoint_claim_tolerance_is_independent_of_model_scale() {
    for (scale, document_id) in [(1.0, 0x4000), (1.0e12, 0x4001)] {
        let fixture = scaled_polyline_fixture(scale, document_id);
        let session = retained(fixture.document.clone());
        let mut features = ComputedFeatureDocument::new(fixture.document.id());
        let feature = features
            .create_fillet_set(
                "narrow surviving middle interval",
                1.999_98 * scale,
                vec![first_corner(fixture.spans), second_corner(fixture.spans)],
            )
            .unwrap();
        let mut allocator = ComputedEvaluationAllocator::default();
        let snapshot = evaluate(&session, &features, &mut allocator);
        let state = &snapshot
            .feature_evaluations()
            .iter()
            .find(|value| value.feature == feature)
            .unwrap()
            .state;
        assert!(
            matches!(state, ComputedFeatureEvaluationState::Current { .. }),
            "unexpected endpoint-claim state at scale {scale}: {state:?}"
        );
        let middle = snapshot
            .source_fragment_edges(source(fixture.spans[1]))
            .next()
            .unwrap();
        let ComputedEdgeGeometry::NativeSourceFragment { interval, .. } = middle.geometry else {
            panic!("expected middle source fragment");
        };
        assert!(interval.start < interval.end);
        assert!((interval.end - interval.start - 1.0e-5).abs() < 1.0e-8);
    }
}

#[test]
fn duplicate_endpoint_claims_fail_every_participating_set() {
    let fixture = polyline_fixture();
    let session = retained(fixture.document.clone());
    let mut features = ComputedFeatureDocument::new(fixture.document.id());
    let first = features
        .create_fillet_set("first", 0.5, vec![first_corner(fixture.spans)])
        .unwrap();
    let duplicate = features
        .create_fillet_set("duplicate", 0.5, vec![first_corner(fixture.spans)])
        .unwrap();
    let mut allocator = ComputedEvaluationAllocator::default();
    let snapshot = evaluate(&session, &features, &mut allocator);
    for feature in [first, duplicate] {
        assert!(matches!(
            snapshot
                .feature_evaluations()
                .iter()
                .find(|value| value.feature == feature)
                .unwrap()
                .state,
            ComputedFeatureEvaluationState::Failed {
                failure: ComputedFeatureFailure::EndpointClaimConflict { .. }
            }
        ));
    }
    assert!(snapshot.edges().is_empty());
}

#[test]
fn explicit_lifecycle_high_water_rebase_is_strictly_monotonic() {
    let fixture = polyline_fixture();
    let mut document = ComputedFeatureDocument::new(fixture.document.id());
    document
        .rebase_after_restore(ComputedFeatureLifecycleHighWater {
            revision: ComputedFeatureRevision::from_raw(50),
            allocator: document.allocator_high_water(),
        })
        .unwrap();
    assert_eq!(document.revision().raw(), 51);
    let mut allocator = ComputedEvaluationAllocator::default();
    allocator.retain_high_water(ComputedEvaluationAllocatorHighWater {
        next_revision: ComputedEvaluationRevision::from_raw(42),
    });
    assert_eq!(allocator.high_water().next_revision.raw(), 42);
}
