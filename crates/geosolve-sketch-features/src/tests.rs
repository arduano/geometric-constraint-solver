// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_sketch::{
    CancellationToken, CurveDefinition, CurveSpan, DocumentArcSweep, DocumentConstraintDefinition,
    DocumentCurveNormalSide, DocumentCurveTrimView, DocumentFilletEndpointOrder,
    DocumentFilletTrimEndpoint, DocumentObjectId, DocumentSolveRequest, DocumentTrimBoundary,
    DocumentTrimParameter, GeometryRole, OperationControl, OperationLimits, OperationOutcome,
    RetainedSketchDocumentSession, ScalarDomain, ScalarUnit, SketchDocument, SketchHardValidity,
    SolverConfig, cancellation_pair,
};

use crate::{
    ComputedEdgeGeometry, ComputedEvaluationAllocator, ComputedEvaluationAllocatorHighWater,
    ComputedEvaluationRevision, ComputedFeatureAllocatorHighWater, ComputedFeatureAuthoringError,
    ComputedFeatureDefinition, ComputedFeatureDocument, ComputedFeatureDocumentError,
    ComputedFeatureDocumentId, ComputedFeatureEvaluationPolicy, ComputedFeatureEvaluationSnapshot,
    ComputedFeatureEvaluationState, ComputedFeatureFailure, ComputedFeatureLifecycleHighWater,
    ComputedFeatureReanchorError, ComputedFeatureRevision, ComputedFeatureSnapshotError,
    ComputedFilletAuthoringOptions, ComputedFilletContactReseedRequest,
    ComputedFilletCornerAlternativeKind, ComputedFilletCornerAuthoringRequest,
    ComputedFilletCurvePick, ComputedFilletParent, ComputedFilletParentIndex,
    ContinuedComputedFilletCorner, NativeCurveSpanSource, NewComputedFilletCorner,
};

struct PolylineFixture {
    document: SketchDocument,
    points: [geosolve_sketch::DesignPointId; 4],
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
        points,
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
struct M70bF004Row {
    payload_fingerprint: &'static str,
    accepted_canonical_json: &'static str,
    line_start: [f64; 2],
    line_end: [f64; 2],
    viable_circle_parameter: f64,
    viable_circle_winding: i32,
}

const M70B_F004_ROWS: [M70bF004Row; 2] = [
    M70bF004Row {
        payload_fingerprint: "4752:daa87c91c75abf9f",
        accepted_canonical_json: r#"{"version":4,"id":"945530003fee983a59bf60604279fed7","next_id":"945530003fee983a59bf60604279fee0","model_scale":10.0,"points":[{"id":"945530003fee983a59bf60604279fed8","label":"draft point","position":[-0.9640476565370273,2.537115794695225]},{"id":"945530003fee983a59bf60604279fedb","label":"draft point","position":[-5.0201018212354995,0.07996993839962943]},{"id":"945530003fee983a59bf60604279fedc","label":"draft point","position":[4.23240434577232,0.07996993839962896]}],"scalars":[{"id":"945530003fee983a59bf60604279fed9","label":"radius","value":1.1815315903695374,"unit":"length","domain":{"kind":"positive"}}],"curves":[{"id":"945530003fee983a59bf60604279feda","label":"circle","definition":{"kind":"circle","center":"945530003fee983a59bf60604279fed8","radius":"945530003fee983a59bf60604279fed9"}},{"id":"945530003fee983a59bf60604279fedd","label":"line","definition":{"kind":"line","start":"945530003fee983a59bf60604279fedb","end":"945530003fee983a59bf60604279fedc","branch_direction":[1.0,0.0]}}],"contacts":[],"trim_views":[],"constraints":[{"id":"945530003fee983a59bf60604279fede","source_id":"945530003fee983a59bf60604279fedf","label":"auto horizontal","suppressed":false,"definition":{"kind":"horizontal","line":{"curve":"945530003fee983a59bf60604279fedd","segment":0}}}],"dimensions":[],"source_order":["945530003fee983a59bf60604279fedf"]}"#,
        line_start: [-5.020_101_821_235_499_5, 0.079_969_938_399_629_43],
        line_end: [4.232_404_345_772_32, 0.079_969_938_399_628_96],
        viable_circle_parameter: 5.551_739_581_930_468,
        viable_circle_winding: 0,
    },
    M70bF004Row {
        payload_fingerprint: "4750:beda1885b15e38b5",
        accepted_canonical_json: r#"{"version":4,"id":"945530003fee983a59bf60604279fed7","next_id":"945530003fee983a59bf60604279fee0","model_scale":10.0,"points":[{"id":"945530003fee983a59bf60604279fed8","label":"draft point","position":[-0.9640476565370273,2.537115794695225]},{"id":"945530003fee983a59bf60604279fedb","label":"draft point","position":[-5.0201018212354995,2.043335287688456]},{"id":"945530003fee983a59bf60604279fedc","label":"draft point","position":[4.5968613866582695,2.043335287688455]}],"scalars":[{"id":"945530003fee983a59bf60604279fed9","label":"radius","value":1.1815315903695374,"unit":"length","domain":{"kind":"positive"}}],"curves":[{"id":"945530003fee983a59bf60604279feda","label":"circle","definition":{"kind":"circle","center":"945530003fee983a59bf60604279fed8","radius":"945530003fee983a59bf60604279fed9"}},{"id":"945530003fee983a59bf60604279fedd","label":"line","definition":{"kind":"line","start":"945530003fee983a59bf60604279fedb","end":"945530003fee983a59bf60604279fedc","branch_direction":[1.0,0.0]}}],"contacts":[],"trim_views":[],"constraints":[{"id":"945530003fee983a59bf60604279fede","source_id":"945530003fee983a59bf60604279fedf","label":"auto horizontal","suppressed":false,"definition":{"kind":"horizontal","line":{"curve":"945530003fee983a59bf60604279fedd","segment":0}}}],"dimensions":[],"source_order":["945530003fee983a59bf60604279fedf"]}"#,
        line_start: [-5.020_101_821_235_499_5, 2.043_335_287_688_456],
        line_end: [4.596_861_386_658_269_5, 2.043_335_287_688_455],
        viable_circle_parameter: 6.517_367_674_350_06,
        viable_circle_winding: 1,
    },
];

const M70B_F005_PAYLOAD_FINGERPRINT: &str = "4228:0823d31f269300af";

const M70B_F005_ACCEPTED_JSON: &str = r#"{"version":4,"id":"7653a0003fed873aee16ee394279fe5e","next_id":"7653a0003fed873aee16ee394279fe65","model_scale":10.0,"points":[{"id":"7653a0003fed873aee16ee394279fe5f","label":"draft point","position":[0.16002449354493023,1.9065418176251467]},{"id":"7653a0003fed873aee16ee394279fe62","label":"draft point","position":[-2.6404041434913528,2.0437056692350866]},{"id":"7653a0003fed873aee16ee394279fe63","label":"draft point","position":[1.371638516099403,4.855564627238864]}],"scalars":[{"id":"7653a0003fed873aee16ee394279fe60","label":"radius","value":2.201783656372145,"unit":"length","domain":{"kind":"positive"}}],"curves":[{"id":"7653a0003fed873aee16ee394279fe61","label":"circle","definition":{"kind":"circle","center":"7653a0003fed873aee16ee394279fe5f","radius":"7653a0003fed873aee16ee394279fe60"}},{"id":"7653a0003fed873aee16ee394279fe64","label":"line","definition":{"kind":"line","start":"7653a0003fed873aee16ee394279fe62","end":"7653a0003fed873aee16ee394279fe63","branch_direction":[0.9748804436785523,0.22272880490208083]}}],"contacts":[],"trim_views":[],"constraints":[],"dimensions":[],"source_order":[]}"#;

const M70B_F005_FEATURE_JSON: &str = r#"{"version":1,"document_id":"1136cf735081f15888738f4d370b9b2d","sketch_document":"7653a0003fed873aee16ee394279fe5e","revision":7,"next_feature_id":"0000000000000002","next_corner_id":"0000000000000002","features":[{"id":"0000000000000001","label":"Fillet 1","suppressed":false,"definition":{"kind":"fillet_set","radius":1.0,"corners":[{"id":"0000000000000001","first":{"source":{"span":{"curve":"7653a0003fed873aee16ee394279fe61","segment":0}},"picked_parameter":0.01630131737160223,"winding":1,"neighborhood":{"local":{"lower":4.959571177211237,"upper":7.857323073392596}},"normal_side":"right","retained_endpoint":"end","periodic_anchor":{"parameter":3.1578939709613953,"winding":0}},"second":{"source":{"span":{"curve":"7653a0003fed873aee16ee394279fe64","segment":0}},"picked_parameter":0.6995120213306758,"winding":0,"neighborhood":"interior","normal_side":"left","retained_endpoint":"start","periodic_anchor":null},"endpoint_order":"first_then_second","sweep":"counter_clockwise"}]}}],"digest":"df8408ece03aa63593d91056ed1d09592f4f1f2654cb2616f205be04cb217081"}"#;

#[allow(
    clippy::too_many_lines,
    reason = "the exact payload-derived sketch and persistent Fillet intent remain one auditable fixture"
)]
fn m70b_f004_fixture(
    row: M70bF004Row,
) -> (
    RetainedSketchDocumentSession,
    ComputedFeatureDocument,
    crate::ComputedFeatureId,
    crate::ComputedFeatureCornerId,
    NewComputedFilletCorner,
) {
    let mut document = SketchDocument::with_id(
        10.0,
        geosolve_sketch::DocumentId(geosolve_sketch::PersistentId::from_u128(
            0x9455_3000_3fee_983a_59bf_6060_4279_fed7,
        )),
    )
    .unwrap();
    let center = document
        .add_point(
            "draft point",
            [-0.964_047_656_537_027_3, 2.537_115_794_695_225],
        )
        .unwrap();
    let radius = document
        .add_scalar(
            "radius",
            1.181_531_590_369_537_4,
            ScalarUnit::Length,
            ScalarDomain::Positive,
        )
        .unwrap();
    let circle = CurveSpan::line(
        document
            .add_curve("circle", CurveDefinition::Circle { center, radius })
            .unwrap(),
    );
    let line_start = document.add_point("draft point", row.line_start).unwrap();
    let line_end = document.add_point("draft point", row.line_end).unwrap();
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
    document
        .add_constraint(
            "auto horizontal",
            DocumentConstraintDefinition::Horizontal { line },
        )
        .unwrap();

    assert_eq!(
        document.to_canonical_json().unwrap(),
        row.accepted_canonical_json,
        "{}: payload-derived sketch transcription drifted",
        row.payload_fingerprint
    );
    let session = retained(document);
    assert_eq!(
        session
            .accepted_state_for_current_input()
            .unwrap()
            .document()
            .to_canonical_json()
            .unwrap(),
        row.accepted_canonical_json,
        "{} did not reconstruct the exact accepted sketch",
        row.payload_fingerprint
    );
    let corner = NewComputedFilletCorner {
        first: ComputedFilletParent {
            source: source(circle),
            picked_parameter: 6.010_678_569_256_539,
            winding: 0,
            neighborhood: geosolve_sketch::ContactNeighborhood::Local {
                lower: 4.712_388_980_384_694,
                upper: 7.853_981_633_974_479,
            },
            normal_side: DocumentCurveNormalSide::Right,
            retained_endpoint: DocumentFilletTrimEndpoint::End,
            periodic_anchor: Some(DocumentTrimParameter {
                parameter: 2.869_085_915_666_746,
                winding: 0,
            }),
        },
        second: ComputedFilletParent {
            source: source(line),
            picked_parameter: 0.634_799_522_276_009_7,
            winding: 0,
            neighborhood: geosolve_sketch::ContactNeighborhood::Interior,
            normal_side: DocumentCurveNormalSide::Left,
            retained_endpoint: DocumentFilletTrimEndpoint::End,
            periodic_anchor: None,
        },
        endpoint_order: DocumentFilletEndpointOrder::FirstThenSecond,
        sweep: DocumentArcSweep::CounterClockwise,
    };
    let mut features = ComputedFeatureDocument::with_id(
        session.design_document().id(),
        ComputedFeatureDocumentId::from_raw(0xf330_5f73_5082_ee5a_3fda_0114_370b_9ba4),
    );
    let feature = features
        .create_fillet_set("Fillet 1", 1.0, vec![corner])
        .unwrap();
    let persisted_corner = match &features.feature(feature).unwrap().definition {
        ComputedFeatureDefinition::FilletSet(fillet) => fillet.corners[0],
    };
    assert_eq!(
        features.digest().to_string(),
        "ddeb29c71705b33e28987876be77574c3491d8afd2559569d648fbed27c6d8e8",
        "{} did not reconstruct the exact persisted feature intent",
        row.payload_fingerprint
    );
    (
        session,
        features,
        feature,
        persisted_corner.id,
        persisted_corner.without_id(),
    )
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

fn normalized_cross(first: [f64; 2], second: [f64; 2]) -> f64 {
    let denominator = first[0].hypot(first[1]) * second[0].hypot(second[1]);
    (first[0].mul_add(second[1], -first[1] * second[0])) / denominator
}

fn tangent_orientation_cell(direction: [f64; 2], parameter: f64) -> (f64, f64) {
    let first_barrier = direction[1].atan2(direction[0]) - std::f64::consts::FRAC_PI_2;
    let lower = ((parameter - first_barrier) / std::f64::consts::PI)
        .floor()
        .mul_add(std::f64::consts::PI, first_barrier);
    (lower, lower + std::f64::consts::PI)
}

fn assert_m70b_f004_branch_state(
    actual: NewComputedFilletCorner,
    persisted: NewComputedFilletCorner,
    payload_fingerprint: &str,
) {
    assert_eq!(
        actual.first.source, persisted.first.source,
        "{payload_fingerprint}"
    );
    assert_eq!(
        actual.second.source, persisted.second.source,
        "{payload_fingerprint}"
    );
    assert_eq!(
        actual.second.winding, persisted.second.winding,
        "{payload_fingerprint}"
    );
    let geosolve_sketch::ContactNeighborhood::Local {
        lower: actual_lower,
        upper: actual_upper,
    } = actual.first.neighborhood
    else {
        panic!("{payload_fingerprint}: circle branch is not Local");
    };
    let geosolve_sketch::ContactNeighborhood::Local {
        lower: persisted_lower,
        upper: persisted_upper,
    } = persisted.first.neighborhood
    else {
        panic!("{payload_fingerprint}: persisted circle branch is not Local");
    };
    assert_close(actual_lower, persisted_lower, 2.0e-15);
    assert_close(actual_upper, persisted_upper, 2.0e-15);
    assert_eq!(
        actual.second.neighborhood,
        geosolve_sketch::ContactNeighborhood::Interior,
        "{payload_fingerprint}"
    );
    assert_eq!(
        actual.first.normal_side,
        DocumentCurveNormalSide::Right,
        "{payload_fingerprint}"
    );
    assert_eq!(
        actual.second.normal_side,
        DocumentCurveNormalSide::Left,
        "{payload_fingerprint}"
    );
    assert_eq!(
        actual.first.retained_endpoint,
        DocumentFilletTrimEndpoint::End,
        "{payload_fingerprint}"
    );
    assert_eq!(
        actual.second.retained_endpoint,
        DocumentFilletTrimEndpoint::End,
        "{payload_fingerprint}"
    );
    let actual_anchor = actual
        .first
        .periodic_anchor
        .unwrap_or_else(|| panic!("{payload_fingerprint}: circle branch lost its anchor"));
    let persisted_anchor = persisted
        .first
        .periodic_anchor
        .unwrap_or_else(|| panic!("{payload_fingerprint}: persisted circle branch has no anchor"));
    let actual_total_parameter =
        actual.first.picked_parameter + f64::from(actual.first.winding) * std::f64::consts::TAU;
    let persisted_total_parameter = persisted.first.picked_parameter
        + f64::from(persisted.first.winding) * std::f64::consts::TAU;
    let actual_anchor_total =
        actual_anchor.parameter + f64::from(actual_anchor.winding) * std::f64::consts::TAU;
    let persisted_anchor_total =
        persisted_anchor.parameter + f64::from(persisted_anchor.winding) * std::f64::consts::TAU;
    assert_close(
        actual_anchor_total - persisted_anchor_total,
        actual_total_parameter - persisted_total_parameter,
        2.0e-12,
    );
    assert_eq!(actual.second.periodic_anchor, None, "{payload_fingerprint}");
    assert_eq!(
        actual.endpoint_order,
        DocumentFilletEndpointOrder::FirstThenSecond,
        "{payload_fingerprint}"
    );
    assert_eq!(
        actual.sweep,
        DocumentArcSweep::CounterClockwise,
        "{payload_fingerprint}"
    );
}

fn assert_m70b_f004_arc_is_independently_valid(
    document: &SketchDocument,
    corner: NewComputedFilletCorner,
    arc: &crate::ComputedCircularArc,
    payload_fingerprint: &str,
) {
    assert!(
        arc.center.into_iter().all(f64::is_finite)
            && arc.radius.is_finite()
            && arc.start_angle.is_finite()
            && arc.end_angle.is_finite(),
        "{payload_fingerprint}: non-finite generated arc"
    );
    assert_eq!(
        arc.radius.to_bits(),
        1.0_f64.to_bits(),
        "{payload_fingerprint}"
    );
    assert_eq!(arc.sweep, corner.sweep, "{payload_fingerprint}");

    let parents = [corner.first, corner.second];
    for (index, (parent, contact)) in parents.into_iter().zip(arc.contacts).enumerate() {
        assert_eq!(contact.source, parent.source, "{payload_fingerprint}");
        assert!(
            contact.parameter.is_finite()
                && contact.total_parameter.is_finite()
                && contact.position.into_iter().all(f64::is_finite),
            "{payload_fingerprint}: non-finite generated contact"
        );
        let expected_total_parameter = if index == 0 {
            let geosolve_sketch::ContactNeighborhood::Local { lower, upper } = parent.neighborhood
            else {
                panic!("{payload_fingerprint}: circle branch is not Local");
            };
            assert!(
                lower < contact.total_parameter && contact.total_parameter < upper,
                "{payload_fingerprint}: evaluated circle root escaped the persisted Local cell"
            );
            contact.parameter + f64::from(contact.winding) * std::f64::consts::TAU
        } else {
            assert_eq!(
                contact.winding, 0,
                "{payload_fingerprint}: bounded line contact acquired a winding"
            );
            assert_eq!(
                parent.neighborhood,
                geosolve_sketch::ContactNeighborhood::Interior,
                "{payload_fingerprint}: line branch is not Interior"
            );
            contact.parameter
        };
        assert_close(contact.total_parameter, expected_total_parameter, 2.0e-12);
        let jet = document
            .evaluate_curve_jet(parent.source.span, contact.total_parameter)
            .unwrap();
        let position_error =
            (jet.position.x - contact.position[0]).hypot(jet.position.y - contact.position[1]);
        assert!(
            position_error <= 1.0e-9,
            "{payload_fingerprint}: source/contact mismatch {position_error:.12e}"
        );
        let radial = [
            arc.center[0] - contact.position[0],
            arc.center[1] - contact.position[1],
        ];
        let radial_length = radial[0].hypot(radial[1]);
        assert_close(radial_length, arc.radius, 1.0e-9);
        let tangent_length = jet.first_derivative.x.hypot(jet.first_derivative.y);
        let left_normal = [
            -jet.first_derivative.y / tangent_length,
            jet.first_derivative.x / tangent_length,
        ];
        let expected_signed_offset = match parent.normal_side {
            DocumentCurveNormalSide::Left => arc.radius,
            DocumentCurveNormalSide::Right => -arc.radius,
        };
        let signed_offset = radial[0] * left_normal[0] + radial[1] * left_normal[1];
        assert_close(signed_offset, expected_signed_offset, 1.0e-9);
        let normalized_tangency =
            (jet.first_derivative.x * radial[0] + jet.first_derivative.y * radial[1]).abs()
                / (tangent_length * radial_length);
        assert!(
            normalized_tangency <= 1.0e-9,
            "{payload_fingerprint}: normalized tangency residual {normalized_tangency:.12e}"
        );
    }

    let (start, end) = match corner.endpoint_order {
        DocumentFilletEndpointOrder::FirstThenSecond => (arc.contacts[0], arc.contacts[1]),
        DocumentFilletEndpointOrder::SecondThenFirst => (arc.contacts[1], arc.contacts[0]),
    };
    for (angle, contact) in [(arc.start_angle, start), (arc.end_angle, end)] {
        let expected =
            (contact.position[1] - arc.center[1]).atan2(contact.position[0] - arc.center[0]);
        let delta = angle - expected;
        let angle_error = delta.sin().atan2(delta.cos()).abs();
        assert!(
            angle_error <= 1.0e-9,
            "{payload_fingerprint}: arc endpoint angle mismatch {angle_error:.12e}"
        );
    }
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

fn construction_geometry_signature(
    snapshot: &crate::ComputedFeatureSnapshot,
) -> Vec<SourceGeometrySignature> {
    let mut values = snapshot
        .construction_fragments()
        .iter()
        .map(|fragment| {
            (
                fragment.source,
                fragment.interval.start.to_bits(),
                fragment.interval.end.to_bits(),
            )
        })
        .collect::<Vec<_>>();
    values.sort_unstable();
    values
}

fn assert_interval(actual: crate::ComputedSourceInterval, expected_start: f64, expected_end: f64) {
    assert_close(actual.start, expected_start, 1.0e-10);
    assert_close(actual.end, expected_end, 1.0e-10);
}

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
    assert!(
        snapshot
            .edges()
            .iter()
            .all(|edge| edge.role == GeometryRole::Profile)
    );
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

    let first_discarded = snapshot
        .source_construction_fragments(source(fixture.spans[0]))
        .collect::<Vec<_>>();
    assert_eq!(first_discarded.len(), 1);
    assert_interval(first_discarded[0].interval, 0.875, 1.0);
    assert_eq!(
        first_discarded[0].provenance.endpoint,
        DocumentFilletTrimEndpoint::End
    );

    let middle_discarded = snapshot
        .source_construction_fragments(source(fixture.spans[1]))
        .collect::<Vec<_>>();
    assert_eq!(middle_discarded.len(), 2);
    assert_interval(middle_discarded[0].interval, 0.0, 0.125);
    assert_interval(middle_discarded[1].interval, 0.875, 1.0);
    assert_eq!(
        middle_discarded[0].provenance.endpoint,
        DocumentFilletTrimEndpoint::Start
    );
    assert_eq!(
        middle_discarded[1].provenance.endpoint,
        DocumentFilletTrimEndpoint::End
    );

    let last_discarded = snapshot
        .source_construction_fragments(source(fixture.spans[2]))
        .collect::<Vec<_>>();
    assert_eq!(last_discarded.len(), 1);
    assert_interval(last_discarded[0].interval, 0.0, 0.125);
    assert_eq!(
        last_discarded[0].provenance.endpoint,
        DocumentFilletTrimEndpoint::Start
    );

    for fragment in snapshot.construction_fragments() {
        assert_eq!(fragment.source_role, GeometryRole::Profile);
        assert_interval(fragment.provenance.base_interval, 0.0, 1.0);
        assert_eq!(snapshot.construction_fragment(fragment.id), Some(fragment));
        assert_eq!(
            snapshot
                .fillet_construction_fragments(fragment.provenance.owner)
                .count(),
            2,
            "each right-angle corner discards one complement from each parent"
        );
    }
    assert!(
        snapshot
            .construction_fragment(crate::ComputedConstructionFragmentId {
                evaluation: crate::ComputedEvaluationRevision::from_raw(
                    snapshot.evaluation_revision().raw() + 1,
                ),
                ordinal: 0,
            })
            .is_none()
    );
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
fn effective_edge_roles_follow_native_sources_and_mixed_parent_fillet_semantics() {
    let mut fixture = line_circle_fixture();
    fixture
        .document
        .set_geometry_role(fixture.line.curve, GeometryRole::Construction)
        .unwrap();
    assert_eq!(
        fixture.document.geometry_role(fixture.line.curve),
        Some(GeometryRole::Construction)
    );
    assert_eq!(
        fixture.document.geometry_role(fixture.circle.curve),
        Some(GeometryRole::Profile)
    );
    let session = retained(fixture.document.clone());
    let authoring = crate::ComputedFeatureAuthoringSnapshot::capture(&session).unwrap();
    let resolved = complete(
        authoring
            .resolve_fillet_corner(
                fixture.request,
                0.75,
                ComputedFeatureEvaluationPolicy::default(),
                OperationControl::unlimited(),
            )
            .unwrap(),
    );
    let mut features = ComputedFeatureDocument::new(fixture.document.id());
    features
        .create_fillet_set("mixed roles", 0.75, vec![resolved.corner])
        .unwrap();
    let evaluated = evaluate(
        &session,
        &features,
        &mut ComputedEvaluationAllocator::default(),
    );

    let line_fragment = evaluated
        .source_fragment_edges(source(fixture.line))
        .next()
        .unwrap();
    assert_eq!(line_fragment.role, GeometryRole::Construction);
    let arc = evaluated
        .edges()
        .iter()
        .find(|edge| matches!(edge.geometry, ComputedEdgeGeometry::CircularArc(_)))
        .unwrap();
    assert_eq!(arc.role, GeometryRole::Construction);
    let discarded = evaluated
        .source_construction_fragments(source(fixture.line))
        .collect::<Vec<_>>();
    assert_eq!(discarded.len(), 1);
    assert_eq!(discarded[0].source_role, GeometryRole::Construction);
    assert_eq!(
        evaluated
            .source_construction_fragments(source(fixture.circle))
            .count(),
        0,
        "a full-period profile parent remains whole even in a mixed-role Fillet"
    );
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
    assert_eq!(
        construction_geometry_signature(&batch_snapshot),
        construction_geometry_signature(&sequential_snapshot)
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
    assert_eq!(suppressed.construction_fragments().len(), 2);
    assert!(
        suppressed
            .construction_fragments()
            .iter()
            .all(|fragment| fragment.provenance.owner.feature != first)
    );
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
    assert!(failed.construction_fragments().is_empty());
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
    assert_eq!(failed.construction_fragments().len(), 2);
    assert!(
        failed
            .construction_fragments()
            .iter()
            .all(|fragment| fragment.provenance.owner.feature == other)
    );
    let recovered = evaluate(&original, &features, &mut allocator);
    assert!(recovered.feature_evaluations().iter().any(|value| {
        value.feature == missing
            && matches!(value.state, ComputedFeatureEvaluationState::Current { .. })
    }));
    assert!(recovered.feature_evaluations().iter().any(|value| {
        value.feature == other
            && matches!(value.state, ComputedFeatureEvaluationState::Current { .. })
    }));
    assert_eq!(recovered.construction_fragments().len(), 4);
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
fn construction_fragments_are_policy_bounded_and_counted_as_publication_work() {
    let fixture = polyline_fixture();
    let session = retained(fixture.document.clone());
    let mut features = ComputedFeatureDocument::new(fixture.document.id());
    features
        .create_fillet_set("one", 0.5, vec![first_corner(fixture.spans)])
        .unwrap();
    let mut allocator = ComputedEvaluationAllocator::default();

    let snapshot = ComputedFeatureEvaluationSnapshot::capture(
        &session,
        &features,
        ComputedFeatureEvaluationPolicy::default(),
    )
    .unwrap();
    let OperationOutcome::Completed { value, report } = snapshot
        .clone()
        .prepare(&mut allocator)
        .unwrap()
        .execute(OperationControl::unlimited())
        .unwrap()
    else {
        panic!("unlimited construction-fragment evaluation must complete");
    };
    assert_eq!(value.edges().len(), 3);
    assert_eq!(value.construction_fragments().len(), 2);
    assert_eq!(report.consumed.profile_fragments, 5);

    let bounded = ComputedFeatureEvaluationPolicy {
        max_construction_fragments: 1,
        ..ComputedFeatureEvaluationPolicy::default()
    };
    let limited = ComputedFeatureEvaluationSnapshot::capture(&session, &features, bounded)
        .unwrap()
        .prepare(&mut allocator)
        .unwrap()
        .execute(OperationControl::unlimited());
    assert!(matches!(
        limited,
        Err(crate::ComputedFeatureEvaluationError::PolicyLimitExceeded {
            resource: "construction fragments",
            actual: 2,
            limit: 1,
        })
    ));

    let mut limits = OperationLimits::unlimited();
    limits.profile_fragments = 4;
    let exhausted = snapshot
        .prepare(&mut allocator)
        .unwrap()
        .execute(OperationControl::new(CancellationToken::default(), limits))
        .unwrap();
    assert!(matches!(
        exhausted,
        OperationOutcome::WorkExhausted { report }
            if report.consumed.profile_fragments == 4
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
fn reverse_line_pick_order_keeps_canonical_parents_and_preview_contacts_aligned() {
    let fixture = polyline_fixture();
    let session = retained(fixture.document.clone());
    let authoring = crate::ComputedFeatureAuthoringSnapshot::capture(&session).unwrap();
    let forward_request = first_corner_authoring_request(&fixture.document, fixture.spans);
    let mut reverse_request = forward_request;
    std::mem::swap(&mut reverse_request.first, &mut reverse_request.second);

    let resolve = |request| {
        complete(
            authoring
                .resolve_fillet_corner(
                    request,
                    0.5,
                    ComputedFeatureEvaluationPolicy::default(),
                    OperationControl::unlimited(),
                )
                .unwrap(),
        )
    };
    let forward = resolve(forward_request);
    let reverse = resolve(reverse_request);

    assert_eq!(reverse.corner, forward.corner);
    for resolved in [forward, reverse] {
        assert_eq!(
            resolved.arc.contacts[0].source,
            resolved.corner.first.source
        );
        assert_eq!(
            resolved.arc.contacts[1].source,
            resolved.corner.second.source
        );
    }
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
fn line_line_continuation_reanchors_after_large_source_point_edits() {
    let mut fixture = polyline_fixture();
    let priors = [first_corner(fixture.spans), second_corner(fixture.spans)];
    let mut features = ComputedFeatureDocument::new(fixture.document.id());
    let feature = features
        .create_fillet_set("edited adjacent corners", 0.5, priors.to_vec())
        .unwrap();
    let mut allocator = ComputedEvaluationAllocator::default();
    let before_edit = evaluate(
        &retained(fixture.document.clone()),
        &features,
        &mut allocator,
    );
    let before_discarded = construction_geometry_signature(&before_edit);

    // Keep both right-angle Fillets visibly regular while moving their line
    // contacts well outside the old one-eighth parameter neighbourhoods.
    fixture
        .document
        .set_point_position(fixture.points[0], [3.0, 0.0])
        .unwrap();
    fixture
        .document
        .set_point_position(fixture.points[3], [5.0, 4.0])
        .unwrap();
    let session = retained(fixture.document);
    let evaluated = evaluate(&session, &features, &mut allocator);
    assert!(matches!(
        evaluated
            .feature_evaluations()
            .iter()
            .find(|evaluation| evaluation.feature == feature)
            .unwrap()
            .state,
        ComputedFeatureEvaluationState::Current { .. }
    ));
    assert_eq!(evaluated.construction_fragments().len(), 4);
    assert_ne!(
        construction_geometry_signature(&evaluated),
        before_discarded,
        "discarded complements must regenerate from the edited accepted sources"
    );

    let authoring = crate::ComputedFeatureAuthoringSnapshot::capture(&session).unwrap();
    let rebased = complete(
        authoring
            .continue_fillet_corners(
                &priors,
                0.5,
                0.5,
                ComputedFeatureEvaluationPolicy::default(),
                OperationControl::unlimited(),
            )
            .expect("current regular line-line branches must re-anchor"),
    );
    assert_eq!(rebased.len(), 2);
    assert_close(rebased[0].arc.contacts[0].total_parameter, 0.5, 1.0e-10);
    assert_close(rebased[1].arc.contacts[1].total_parameter, 0.5, 1.0e-10);

    let continued = complete(
        authoring
            .continue_fillet_corners(
                &priors,
                0.5,
                0.6,
                ComputedFeatureEvaluationPolicy::default(),
                OperationControl::unlimited(),
            )
            .expect("re-anchored regular line-line branches must remain adjustable"),
    );
    for (prior, value) in priors.into_iter().zip(continued) {
        assert_eq!(value.arc.radius.to_bits(), 0.6_f64.to_bits());
        assert_eq!(value.corner.first.source, prior.first.source);
        assert_eq!(value.corner.second.source, prior.second.source);
        assert_eq!(value.corner.first.neighborhood, prior.first.neighborhood);
        assert_eq!(value.corner.second.neighborhood, prior.second.neighborhood);
        assert_eq!(value.corner.first.winding, prior.first.winding);
        assert_eq!(value.corner.second.winding, prior.second.winding);
        assert_eq!(value.corner.first.normal_side, prior.first.normal_side);
        assert_eq!(value.corner.second.normal_side, prior.second.normal_side);
        assert_eq!(
            value.corner.first.retained_endpoint,
            prior.first.retained_endpoint
        );
        assert_eq!(
            value.corner.second.retained_endpoint,
            prior.second.retained_endpoint
        );
        assert_eq!(value.corner.endpoint_order, prior.endpoint_order);
        assert_eq!(value.corner.sweep, prior.sweep);
        assert!(
            value
                .sensitivity
                .center_derivative
                .into_iter()
                .all(f64::is_finite)
        );
        assert!(value.sensitivity.transverse_quality > 0.9);
    }
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "the two-row regression preserves payload, accepted-state, explicit branch and independently validated viable-root evidence together"
)]
fn m70b_f004_line_circle_persisted_evaluation_traverses_complete_radial_branch_cell() {
    for row in M70B_F004_ROWS {
        let (session, features, feature, corner_id, persisted_corner) = m70b_f004_fixture(row);
        let accepted = session
            .accepted_state_for_current_input()
            .expect("payload-derived sketch must be current and accepted");
        assert!(
            accepted
                .document()
                .points()
                .iter()
                .all(|point| { point.position.into_iter().all(f64::is_finite) }),
            "{}: accepted point geometry is non-finite",
            row.payload_fingerprint
        );
        assert!(
            accepted
                .document()
                .scalars()
                .iter()
                .all(|scalar| scalar.value.is_finite()),
            "{}: accepted scalar geometry is non-finite",
            row.payload_fingerprint
        );
        let diagnostics = accepted.diagnostics();
        let solve = diagnostics.solve.expect("accepted solve diagnostics");
        assert_eq!(
            solve.hard_validity,
            SketchHardValidity::Valid,
            "{}",
            row.payload_fingerprint
        );
        assert!(
            solve.hard_residuals_validated,
            "{}",
            row.payload_fingerprint
        );
        assert!(
            solve
                .maximum_normalized_hard_residual
                .is_some_and(|residual| residual <= 1.0e-9),
            "{}: accepted hard residual is not independently valid: {solve:?}",
            row.payload_fingerprint
        );
        assert_eq!(
            diagnostics.rank.expect("rank diagnostics").numerical_rank,
            Some(1),
            "{}",
            row.payload_fingerprint
        );
        let mobility = diagnostics.mobility.expect("mobility diagnostics");
        assert_eq!(
            mobility.equality_degrees_of_freedom,
            Some(6),
            "{}",
            row.payload_fingerprint
        );
        assert_eq!(
            mobility.bidirectional_bounded_degrees_of_freedom,
            Some(6),
            "{}",
            row.payload_fingerprint
        );

        assert_eq!(feature.raw(), 1, "{}", row.payload_fingerprint);
        assert_eq!(corner_id.raw(), 1, "{}", row.payload_fingerprint);
        let ComputedFeatureDefinition::FilletSet(persisted_fillet) =
            &features.feature(feature).unwrap().definition;
        assert_eq!(
            persisted_fillet.radius.to_bits(),
            1.0_f64.to_bits(),
            "{}",
            row.payload_fingerprint
        );
        assert_eq!(
            persisted_corner.first.picked_parameter.to_bits(),
            6.010_678_569_256_539_f64.to_bits(),
            "{}",
            row.payload_fingerprint
        );
        assert_eq!(
            persisted_corner.second.picked_parameter.to_bits(),
            0.634_799_522_276_009_7_f64.to_bits(),
            "{}",
            row.payload_fingerprint
        );
        assert_eq!(
            persisted_corner.first.winding, 0,
            "{}",
            row.payload_fingerprint
        );
        assert_eq!(
            persisted_corner.second.winding, 0,
            "{}",
            row.payload_fingerprint
        );
        assert_eq!(
            persisted_corner.first.periodic_anchor,
            Some(DocumentTrimParameter {
                parameter: 2.869_085_915_666_746,
                winding: 0,
            }),
            "{}",
            row.payload_fingerprint
        );
        assert_m70b_f004_branch_state(persisted_corner, persisted_corner, row.payload_fingerprint);
        let geosolve_sketch::ContactNeighborhood::Local { lower, upper } =
            persisted_corner.first.neighborhood
        else {
            panic!("{}: circle branch is not Local", row.payload_fingerprint);
        };
        assert_eq!(
            lower.to_bits(),
            4.712_388_980_384_694_f64.to_bits(),
            "{}",
            row.payload_fingerprint
        );
        assert_eq!(
            upper.to_bits(),
            7.853_981_633_974_479_f64.to_bits(),
            "{}",
            row.payload_fingerprint
        );

        let accepted_identity = accepted.identity();
        let accepted_json = accepted.document().to_canonical_json().unwrap();
        let feature_identity = features.identity();
        let feature_json = features.to_json().unwrap();
        assert_eq!(
            SketchDocument::from_json(&accepted_json)
                .unwrap()
                .to_canonical_json()
                .unwrap(),
            accepted_json,
            "{}: accepted sketch did not survive canonical restoration",
            row.payload_fingerprint
        );
        assert_eq!(
            ComputedFeatureDocument::from_json(&feature_json).unwrap(),
            features,
            "{}: persisted Fillet intent did not survive restoration",
            row.payload_fingerprint
        );
        let prepared_input = session.prepared_input();
        let mut allocator = ComputedEvaluationAllocator::default();
        let current = evaluate(&session, &features, &mut allocator);
        let evaluation = current
            .feature_evaluations()
            .iter()
            .find(|evaluation| evaluation.feature == feature)
            .expect("payload feature evaluation");
        let ComputedFeatureEvaluationState::Current { corner_edges } = &evaluation.state else {
            panic!(
                "{}: unexpected persisted evaluation: {:?}",
                row.payload_fingerprint, evaluation.state
            );
        };
        assert_eq!(corner_edges.len(), 1, "{}", row.payload_fingerprint);
        assert_eq!(corner_edges[0].0, corner_id, "{}", row.payload_fingerprint);
        let arcs = current
            .edges()
            .iter()
            .filter_map(|edge| match &edge.geometry {
                ComputedEdgeGeometry::CircularArc(arc) => Some(arc),
                ComputedEdgeGeometry::NativeSourceFragment { .. } => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(arcs.len(), 1, "{}", row.payload_fingerprint);
        let persisted_arc = arcs[0];
        assert_close(
            persisted_arc.contacts[0].total_parameter,
            row.viable_circle_parameter,
            2.0e-10,
        );
        assert_eq!(
            persisted_arc.contacts[0].winding, row.viable_circle_winding,
            "{}: persisted evaluation used the wrong seam winding",
            row.payload_fingerprint
        );
        assert!(
            lower < persisted_arc.contacts[0].total_parameter
                && persisted_arc.contacts[0].total_parameter < upper,
            "{}: persisted evaluation escaped the explicit Local cell",
            row.payload_fingerprint
        );
        let legacy_seed_window = 0.125 * (upper - lower);
        assert!(
            (persisted_arc.contacts[0].total_parameter - persisted_corner.first.picked_parameter)
                .abs()
                > legacy_seed_window,
            "{}: regression root did not traverse beyond the former seed window",
            row.payload_fingerprint
        );
        assert_m70b_f004_arc_is_independently_valid(
            accepted.document(),
            persisted_corner,
            persisted_arc,
            row.payload_fingerprint,
        );
        let accepted_after = session
            .accepted_state_for_current_input()
            .expect("evaluation must retain accepted sketch");
        assert_eq!(
            accepted_after.identity(),
            accepted_identity,
            "{}",
            row.payload_fingerprint
        );
        assert_eq!(
            accepted_after.document().to_canonical_json().unwrap(),
            accepted_json,
            "{}",
            row.payload_fingerprint
        );
        assert_eq!(
            features.identity(),
            feature_identity,
            "{}",
            row.payload_fingerprint
        );
        assert_eq!(
            features.to_json().unwrap(),
            feature_json,
            "{}",
            row.payload_fingerprint
        );
        assert_eq!(
            session.prepared_input(),
            prepared_input,
            "{}: feature evaluation mutated the retained sketch input",
            row.payload_fingerprint
        );

        let authoring = crate::ComputedFeatureAuthoringSnapshot::capture(&session).unwrap();
        let reanchored = complete(
            authoring
                .reseed_fillet_contact(
                    ComputedFilletContactReseedRequest {
                        prior: persisted_corner,
                        parent: ComputedFilletParentIndex::First,
                        parameter: row
                            .viable_circle_parameter
                            .rem_euclid(std::f64::consts::TAU),
                    },
                    1.0,
                    ComputedFeatureEvaluationPolicy::default(),
                    OperationControl::unlimited(),
                )
                .unwrap_or_else(|error| {
                    panic!(
                        "{}: viable same-cell contact did not reseed: {error:?}",
                        row.payload_fingerprint
                    )
                }),
        );
        assert_eq!(
            reanchored.accepted, accepted_identity,
            "{}",
            row.payload_fingerprint
        );
        assert_m70b_f004_branch_state(reanchored.corner, persisted_corner, row.payload_fingerprint);
        assert_eq!(
            reanchored.corner.first.winding, row.viable_circle_winding,
            "{}: re-anchored circle parent has the wrong seam winding",
            row.payload_fingerprint
        );
        assert_eq!(
            reanchored.arc.contacts[0].winding, row.viable_circle_winding,
            "{}: published circle contact has the wrong seam winding",
            row.payload_fingerprint
        );
        assert_close(
            reanchored.corner.first.picked_parameter,
            row.viable_circle_parameter
                .rem_euclid(std::f64::consts::TAU),
            2.0e-10,
        );
        let circle_parameter = reanchored.arc.contacts[0].total_parameter;
        assert_close(circle_parameter, row.viable_circle_parameter, 2.0e-10);
        assert!(
            circle_parameter > lower && circle_parameter < upper,
            "{}: viable root escaped the unchanged branch cell",
            row.payload_fingerprint
        );
        let current_seed_window = 0.125 * (upper - lower);
        assert!(
            (circle_parameter - persisted_corner.first.picked_parameter).abs()
                > current_seed_window,
            "{}: viable root did not exercise the current seed-window exclusion",
            row.payload_fingerprint
        );
        assert_m70b_f004_arc_is_independently_valid(
            accepted.document(),
            reanchored.corner,
            &reanchored.arc,
            row.payload_fingerprint,
        );
        for (persisted, reanchored) in persisted_arc.contacts.iter().zip(reanchored.arc.contacts) {
            assert_close(
                persisted.total_parameter,
                reanchored.total_parameter,
                2.0e-10,
            );
            for axis in 0..2 {
                assert_close(persisted.position[axis], reanchored.position[axis], 1.0e-9);
            }
        }

        let mut reanchored_features = features.clone();
        reanchored_features
            .set_fillet_corner(feature, corner_id, reanchored.corner)
            .unwrap();
        assert_eq!(
            reanchored_features.feature(feature).unwrap().id,
            feature,
            "{}",
            row.payload_fingerprint
        );
        assert_eq!(
            reanchored_features.corner(feature, corner_id).unwrap().id,
            corner_id,
            "{}",
            row.payload_fingerprint
        );
        let current = evaluate(
            &session,
            &reanchored_features,
            &mut ComputedEvaluationAllocator::default(),
        );
        assert!(
            matches!(
                current
                    .feature_evaluations()
                    .iter()
                    .find(|evaluation| evaluation.feature == feature)
                    .unwrap()
                    .state,
                ComputedFeatureEvaluationState::Current { .. }
            ),
            "{}: re-anchored same-branch feature did not evaluate Current",
            row.payload_fingerprint
        );
        let arcs = current
            .edges()
            .iter()
            .filter_map(|edge| match &edge.geometry {
                ComputedEdgeGeometry::CircularArc(arc) => Some(arc),
                ComputedEdgeGeometry::NativeSourceFragment { .. } => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(arcs.len(), 1, "{}", row.payload_fingerprint);
        assert_m70b_f004_arc_is_independently_valid(
            accepted.document(),
            reanchored.corner,
            arcs[0],
            row.payload_fingerprint,
        );
        assert_eq!(
            session.prepared_input(),
            prepared_input,
            "{}: viable-root characterization mutated the retained sketch input",
            row.payload_fingerprint
        );
    }
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "the payload-derived regression keeps accepted-state, persistent branch, independent geometry, barrier and read-only evidence together"
)]
fn m70b_f005_line_circle_source_rotation_transports_persisted_branch_cell() {
    let document = SketchDocument::from_json(M70B_F005_ACCEPTED_JSON)
        .expect("payload-derived accepted sketch must decode");
    assert_eq!(
        document.to_canonical_json().unwrap(),
        M70B_F005_ACCEPTED_JSON,
        "{M70B_F005_PAYLOAD_FINGERPRINT}: accepted sketch transcription drifted"
    );
    let session = retained(document);
    let accepted = session
        .accepted_state_for_current_input()
        .expect("payload-derived sketch must be current and accepted");
    assert_eq!(
        accepted.document().to_canonical_json().unwrap(),
        M70B_F005_ACCEPTED_JSON,
        "{M70B_F005_PAYLOAD_FINGERPRINT}: retained accepted sketch drifted"
    );
    assert!(
        accepted
            .document()
            .points()
            .iter()
            .all(|point| point.position.into_iter().all(f64::is_finite)),
        "{M70B_F005_PAYLOAD_FINGERPRINT}: accepted point geometry is non-finite"
    );
    assert!(
        accepted
            .document()
            .scalars()
            .iter()
            .all(|scalar| scalar.value.is_finite()),
        "{M70B_F005_PAYLOAD_FINGERPRINT}: accepted scalar geometry is non-finite"
    );
    let diagnostics = accepted.diagnostics();
    let solve = diagnostics.solve.expect("accepted solve diagnostics");
    assert_eq!(solve.hard_validity, SketchHardValidity::Valid);
    assert!(solve.hard_residuals_validated);
    assert!(
        solve
            .maximum_normalized_hard_residual
            .is_some_and(|residual| residual <= 1.0e-9),
        "{M70B_F005_PAYLOAD_FINGERPRINT}: accepted hard residual is not independently valid: {solve:?}"
    );
    assert_eq!(
        diagnostics.rank.expect("rank diagnostics").numerical_rank,
        Some(0)
    );
    let mobility = diagnostics.mobility.expect("mobility diagnostics");
    assert_eq!(mobility.equality_degrees_of_freedom, Some(7));
    assert_eq!(mobility.bidirectional_bounded_degrees_of_freedom, Some(7));

    let features = ComputedFeatureDocument::from_json(M70B_F005_FEATURE_JSON)
        .expect("payload-derived feature intent must decode");
    assert_eq!(
        features.to_json().unwrap(),
        M70B_F005_FEATURE_JSON,
        "{M70B_F005_PAYLOAD_FINGERPRINT}: persisted feature bytes drifted"
    );
    assert_eq!(features.sketch_document(), accepted.document().id());
    assert_eq!(features.revision().raw(), 7);
    assert_eq!(
        features.digest().to_string(),
        "df8408ece03aa63593d91056ed1d09592f4f1f2654cb2616f205be04cb217081"
    );
    let high_water = features.allocator_high_water();
    assert_eq!(high_water.next_feature_id.raw(), 2);
    assert_eq!(high_water.next_corner_id.raw(), 2);
    assert_eq!(features.features().len(), 1);
    let feature = &features.features()[0];
    assert_eq!(feature.id.raw(), 1);
    assert_eq!(feature.label, "Fillet 1");
    assert!(!feature.suppressed);
    let ComputedFeatureDefinition::FilletSet(fillet) = &feature.definition;
    assert_eq!(fillet.radius.to_bits(), 1.0_f64.to_bits());
    assert_eq!(fillet.corners.len(), 1);
    let persisted = fillet.corners[0];
    assert_eq!(persisted.id.raw(), 1);
    let corner = persisted.without_id();
    assert_eq!(
        corner.first.picked_parameter.to_bits(),
        0.016_301_317_371_602_23_f64.to_bits()
    );
    assert_eq!(corner.first.winding, 1);
    assert_eq!(
        corner.first.neighborhood,
        geosolve_sketch::ContactNeighborhood::Local {
            lower: 4.959_571_177_211_237,
            upper: 7.857_323_073_392_596,
        }
    );
    assert_eq!(corner.first.normal_side, DocumentCurveNormalSide::Right);
    assert_eq!(
        corner.first.retained_endpoint,
        DocumentFilletTrimEndpoint::End
    );
    assert_eq!(
        corner.first.periodic_anchor,
        Some(DocumentTrimParameter {
            parameter: 3.157_893_970_961_395_3,
            winding: 0,
        })
    );
    assert_eq!(
        corner.second.picked_parameter.to_bits(),
        0.699_512_021_330_675_8_f64.to_bits()
    );
    assert_eq!(corner.second.winding, 0);
    assert_eq!(
        corner.second.neighborhood,
        geosolve_sketch::ContactNeighborhood::Interior
    );
    assert_eq!(corner.second.normal_side, DocumentCurveNormalSide::Left);
    assert_eq!(
        corner.second.retained_endpoint,
        DocumentFilletTrimEndpoint::Start
    );
    assert_eq!(corner.second.periodic_anchor, None);
    assert_eq!(
        corner.endpoint_order,
        DocumentFilletEndpointOrder::FirstThenSecond
    );
    assert_eq!(corner.sweep, DocumentArcSweep::CounterClockwise);
    let persisted_circle_total =
        corner.first.picked_parameter + f64::from(corner.first.winding) * std::f64::consts::TAU;
    assert_close(persisted_circle_total, 6.299_486_624_551_188, 2.0e-15);

    assert!(matches!(
        accepted
            .document()
            .curve(corner.first.source.span.curve)
            .expect("circle source")
            .definition,
        CurveDefinition::Circle { .. }
    ));
    let CurveDefinition::Line {
        branch_direction, ..
    } = &accepted
        .document()
        .curve(corner.second.source.span.curve)
        .expect("line source")
        .definition
    else {
        panic!("{M70B_F005_PAYLOAD_FINGERPRINT}: second source is not a line");
    };
    let persistent_line_direction = *branch_direction;
    assert_close(
        persistent_line_direction[0],
        0.974_880_443_678_552_3,
        2.0e-15,
    );
    assert_close(
        persistent_line_direction[1],
        0.222_728_804_902_080_83,
        2.0e-15,
    );

    let accepted_identity = accepted.identity();
    let accepted_json = accepted.document().to_canonical_json().unwrap();
    let feature_identity = features.identity();
    let feature_json = features.to_json().unwrap();
    let prepared_input = session.prepared_input();
    let evaluation_snapshot = ComputedFeatureEvaluationSnapshot::capture(
        &session,
        &features,
        ComputedFeatureEvaluationPolicy::default(),
    )
    .expect("current accepted payload input must capture");
    assert_eq!(
        evaluation_snapshot
            .sketch_document()
            .to_canonical_json()
            .unwrap(),
        accepted_json
    );
    let evaluation_input = evaluation_snapshot.input();
    let mut allocator = ComputedEvaluationAllocator::default();
    let evaluated = complete(
        evaluation_snapshot
            .prepare(&mut allocator)
            .unwrap()
            .execute(OperationControl::unlimited())
            .unwrap(),
    );
    assert_eq!(evaluated.input(), evaluation_input);
    let evaluation = evaluated
        .feature_evaluations()
        .iter()
        .find(|evaluation| evaluation.feature == feature.id)
        .expect("payload feature evaluation");
    let ComputedFeatureEvaluationState::Current { corner_edges } = &evaluation.state else {
        panic!(
            "{M70B_F005_PAYLOAD_FINGERPRINT}: persisted same-branch Fillet did not remain Current: {:?}",
            evaluation.state
        );
    };
    assert_eq!(corner_edges.len(), 1);
    assert_eq!(corner_edges[0].0, persisted.id);
    let edge = evaluated
        .edge(corner_edges[0].1)
        .expect("current corner edge must resolve");
    let ComputedEdgeGeometry::CircularArc(arc) = &edge.geometry else {
        panic!("{M70B_F005_PAYLOAD_FINGERPRINT}: current corner edge is not a Fillet arc");
    };
    assert_eq!(
        evaluated
            .edges()
            .iter()
            .filter(|edge| matches!(edge.geometry, ComputedEdgeGeometry::CircularArc(_)))
            .count(),
        1
    );
    assert_eq!(arc.radius.to_bits(), 1.0_f64.to_bits());
    assert_eq!(arc.sweep, DocumentArcSweep::CounterClockwise);
    assert_close(arc.center[0], -0.017_075_528_971_715_492, 2.0e-9);
    assert_close(arc.center[1], 5.103_423_761_681_947, 2.0e-9);

    let circle_contact = arc.contacts[0];
    let line_contact = arc.contacts[1];
    assert_eq!(circle_contact.source, corner.first.source);
    assert_eq!(line_contact.source, corner.second.source);
    assert_eq!(circle_contact.winding, 1);
    assert_eq!(line_contact.winding, 0);
    assert_close(circle_contact.parameter, 1.626_137_496_883_336_2, 2.0e-9);
    assert_close(
        circle_contact.total_parameter,
        7.909_322_804_062_922,
        2.0e-9,
    );
    assert_close(
        circle_contact.total_parameter,
        circle_contact.parameter + f64::from(circle_contact.winding) * std::f64::consts::TAU,
        2.0e-12,
    );
    assert_close(line_contact.parameter, 0.796_915_905_159_832_2, 2.0e-9);
    assert_close(
        line_contact.total_parameter,
        line_contact.parameter,
        2.0e-12,
    );
    assert!(0.0 < line_contact.parameter && line_contact.parameter < 1.0);

    for (parent, contact) in [corner.first, corner.second].into_iter().zip(arc.contacts) {
        let jet = accepted
            .document()
            .evaluate_curve_jet(parent.source.span, contact.total_parameter)
            .expect("accepted source contact must evaluate");
        let incidence =
            (jet.position.x - contact.position[0]).hypot(jet.position.y - contact.position[1]);
        assert!(
            incidence <= 1.0e-9,
            "{M70B_F005_PAYLOAD_FINGERPRINT}: source incidence residual {incidence:.12e}"
        );
        let radial = [
            arc.center[0] - contact.position[0],
            arc.center[1] - contact.position[1],
        ];
        let radial_length = radial[0].hypot(radial[1]);
        assert_close(radial_length, arc.radius, 1.0e-9);
        let tangent = [jet.first_derivative.x, jet.first_derivative.y];
        let tangent_length = tangent[0].hypot(tangent[1]);
        let left_normal = [-tangent[1] / tangent_length, tangent[0] / tangent_length];
        let signed_offset = radial[0].mul_add(left_normal[0], radial[1] * left_normal[1]);
        let expected_offset = match parent.normal_side {
            DocumentCurveNormalSide::Left => arc.radius,
            DocumentCurveNormalSide::Right => -arc.radius,
        };
        assert_close(signed_offset, expected_offset, 1.0e-9);
        let normalized_tangency = tangent[0].mul_add(radial[0], tangent[1] * radial[1]).abs()
            / (tangent_length * radial_length);
        assert!(
            normalized_tangency <= 1.0e-9,
            "{M70B_F005_PAYLOAD_FINGERPRINT}: normalized tangency residual {normalized_tangency:.12e}"
        );
    }
    for (angle, contact) in [
        (arc.start_angle, circle_contact),
        (arc.end_angle, line_contact),
    ] {
        let expected =
            (contact.position[1] - arc.center[1]).atan2(contact.position[0] - arc.center[0]);
        let delta = angle - expected;
        assert!(
            delta.sin().atan2(delta.cos()).abs() <= 1.0e-9,
            "{M70B_F005_PAYLOAD_FINGERPRINT}: generated arc endpoint is not incident"
        );
    }
    assert_close(
        (arc.end_angle - arc.start_angle).rem_euclid(std::f64::consts::TAU),
        0.555_958_188_733_340,
        2.0e-9,
    );

    let circle_jet = accepted
        .document()
        .evaluate_curve_jet(corner.first.source.span, circle_contact.total_parameter)
        .unwrap();
    let line_jet = accepted
        .document()
        .evaluate_curve_jet(corner.second.source.span, line_contact.total_parameter)
        .unwrap();
    let circle_tangent = [circle_jet.first_derivative.x, circle_jet.first_derivative.y];
    let current_line_direction = [line_jet.first_derivative.x, line_jet.first_derivative.y];
    let persistent_orientation = normalized_cross(persistent_line_direction, circle_tangent);
    let current_orientation = normalized_cross(current_line_direction, circle_tangent);
    assert!(persistent_orientation > 0.0);
    assert_close(current_orientation, 0.527_757_423_204_954, 2.0e-10);

    let (persistent_lower, persistent_upper) =
        tangent_orientation_cell(persistent_line_direction, circle_contact.total_parameter);
    let (current_lower, current_upper) =
        tangent_orientation_cell(current_line_direction, circle_contact.total_parameter);
    assert_close(persistent_lower, 4.937_001_677_565_56, 2.0e-12);
    assert_close(persistent_upper, 8.078_594_331_155_353, 2.0e-12);
    assert_close(current_lower, 5.323_688_339_206_471, 2.0e-12);
    assert_close(current_upper, 8.465_280_992_796_263, 2.0e-12);
    let geosolve_sketch::ContactNeighborhood::Local {
        lower: stored_lower,
        upper: stored_upper,
    } = corner.first.neighborhood
    else {
        unreachable!("exact persisted branch was asserted Local above")
    };
    assert!(stored_upper < circle_contact.total_parameter);
    assert_close(
        circle_contact.total_parameter - stored_upper,
        0.051_999_730_670_326,
        2.0e-9,
    );
    assert!(
        persistent_lower < stored_lower
            && stored_lower < persistent_upper
            && persistent_lower < stored_upper
            && stored_upper < persistent_upper
    );
    assert!(
        persistent_lower < circle_contact.total_parameter
            && circle_contact.total_parameter < persistent_upper
            && current_lower < circle_contact.total_parameter
            && circle_contact.total_parameter < current_upper
    );
    assert!(
        persistent_lower < persisted_circle_total
            && persisted_circle_total < persistent_upper
            && current_lower < persisted_circle_total
            && persisted_circle_total < current_upper
    );

    // This second mathematical root lies beyond both real tangent barriers and
    // has the opposite orientation sign. It is a negative control against
    // widening the search into an implicit branch switch.
    let alternate_parameter = 9.021_239_181_529_605;
    let alternate_jet = accepted
        .document()
        .evaluate_curve_jet(corner.first.source.span, alternate_parameter)
        .unwrap();
    let alternate_tangent = [
        alternate_jet.first_derivative.x,
        alternate_jet.first_derivative.y,
    ];
    let alternate_persistent_orientation =
        normalized_cross(persistent_line_direction, alternate_tangent);
    let alternate_current_orientation = normalized_cross(current_line_direction, alternate_tangent);
    assert!(alternate_parameter > persistent_upper && alternate_parameter > current_upper);
    assert!(alternate_persistent_orientation < 0.0);
    assert!(alternate_current_orientation < 0.0);
    assert!(persistent_orientation * alternate_persistent_orientation < 0.0);
    assert!(current_orientation * alternate_current_orientation < 0.0);
    assert!(
        (circle_contact.total_parameter - alternate_parameter).abs() > 1.0,
        "{M70B_F005_PAYLOAD_FINGERPRINT}: evaluation selected the opposite-branch root"
    );

    assert_eq!(
        evaluated.source_fragment_edges(corner.first.source).count(),
        0,
        "{M70B_F005_PAYLOAD_FINGERPRINT}: full circle was incorrectly trimmed"
    );
    assert_eq!(
        evaluated
            .source_construction_fragments(corner.first.source)
            .count(),
        0,
        "{M70B_F005_PAYLOAD_FINGERPRINT}: full circle gained a discarded complement"
    );
    assert_eq!(
        evaluated
            .source_fragment_edges(corner.second.source)
            .count(),
        1,
        "{M70B_F005_PAYLOAD_FINGERPRINT}: retained line side was not published"
    );
    assert_eq!(evaluated.replaced_sources(), &[corner.second.source]);

    let accepted_after = session
        .accepted_state_for_current_input()
        .expect("feature evaluation must retain the accepted sketch");
    assert_eq!(accepted_after.identity(), accepted_identity);
    assert_eq!(
        accepted_after.document().to_canonical_json().unwrap(),
        accepted_json
    );
    assert_eq!(session.prepared_input(), prepared_input);
    assert_eq!(features.identity(), feature_identity);
    assert_eq!(features.to_json().unwrap(), feature_json);
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "the full-winding regression keeps every accepted continuation sample and branch invariant in one auditable sequence"
)]
fn m70b_f005_line_circle_source_rotation_crosses_stale_seed_barrier_and_returns() {
    let base = SketchDocument::from_json(M70B_F005_ACCEPTED_JSON)
        .expect("payload-derived accepted sketch must decode");
    let features = ComputedFeatureDocument::from_json(M70B_F005_FEATURE_JSON)
        .expect("payload-derived feature intent must decode");
    let ComputedFeatureDefinition::FilletSet(fillet) = &features.features()[0].definition;
    let persisted = fillet.corners[0];
    let circle = persisted.first.source.span;
    let line = persisted.second.source.span;
    let CurveDefinition::Line { start, end, .. } =
        base.curve(line.curve).expect("line source").definition
    else {
        panic!("payload-derived affine parent is not a line");
    };
    let CurveDefinition::Circle { center, .. } =
        base.curve(circle.curve).expect("circle source").definition
    else {
        panic!("payload-derived curved parent is not a circle");
    };
    let center = base.point(center).expect("circle center").position;
    let seed_total = persisted.first.picked_parameter
        + f64::from(persisted.first.winding) * std::f64::consts::TAU;
    let seed_tangent = base
        .evaluate_curve_jet(circle, seed_total)
        .expect("circle seed tangent")
        .first_derivative;
    let stale_seed_barrier_angle = seed_tangent.y.atan2(seed_tangent.x);
    let cardinal_crossing = [
        35.0_f64.to_radians(),
        70.0_f64.to_radians(),
        stale_seed_barrier_angle - 1.0e-6,
        stale_seed_barrier_angle,
        stale_seed_barrier_angle + 1.0e-6,
        100.0_f64.to_radians(),
        115.0_f64.to_radians(),
        123.0_f64.to_radians(),
    ];
    let mut forward = cardinal_crossing.to_vec();
    forward.extend((0..=17).map(|step| (140.0 + 15.0 * f64::from(step)).to_radians()));
    let mut angles = forward.clone();
    angles.extend(forward.iter().rev().skip(1).copied());
    let mut previous_circle_parameter: Option<f64> = None;
    let mut previous_evaluation = None;
    let mut allocator = ComputedEvaluationAllocator::default();
    let mut session = retained(base.clone());
    let mut observed_parameters = Vec::with_capacity(angles.len());
    for (step, angle) in angles.into_iter().enumerate() {
        let direction = [angle.cos(), angle.sin()];
        let expected = session.design_identity();
        let transaction = session
            .transact(expected, |document| {
                document.set_point_position(
                    start,
                    [
                        center[0] - 5.0 * direction[0],
                        center[1] - 5.0 * direction[1],
                    ],
                )?;
                document.set_point_position(
                    end,
                    [
                        center[0] + 5.0 * direction[0],
                        center[1] + 5.0 * direction[1],
                    ],
                )?;
                Ok(())
            })
            .unwrap();
        assert!(
            transaction.published_accepted_identity().is_some(),
            "step {step} did not publish accepted source geometry"
        );
        let captured = if let Some(previous) = previous_evaluation.as_ref() {
            ComputedFeatureEvaluationSnapshot::capture_continuing_from(
                &session,
                &features,
                ComputedFeatureEvaluationPolicy::default(),
                previous,
            )
            .unwrap()
        } else {
            ComputedFeatureEvaluationSnapshot::capture(
                &session,
                &features,
                ComputedFeatureEvaluationPolicy::default(),
            )
            .unwrap()
        };
        let evaluated = complete(
            captured
                .prepare(&mut allocator)
                .unwrap()
                .execute(OperationControl::unlimited())
                .unwrap(),
        );
        let evaluation = &evaluated.feature_evaluations()[0];
        let ComputedFeatureEvaluationState::Current { corner_edges } = &evaluation.state else {
            panic!(
                "{M70B_F005_PAYLOAD_FINGERPRINT}: step {step} at {angle:.16} radians lost the regular persisted branch: {:?}",
                evaluation.state
            );
        };
        let edge = evaluated.edge(corner_edges[0].1).expect("current arc edge");
        let ComputedEdgeGeometry::CircularArc(arc) = &edge.geometry else {
            panic!("step {step} did not publish a Fillet arc");
        };
        let parameter = arc.contacts[0].total_parameter;
        if let Some(previous) = previous_circle_parameter {
            assert!(
                (parameter - previous).abs() < 0.75,
                "step {step} jumped from circle parameter {previous:.16} to {parameter:.16}"
            );
        }
        previous_circle_parameter = Some(parameter);
        observed_parameters.push(parameter);
        previous_evaluation = Some(evaluated);
    }
    let range = observed_parameters.iter().copied().fold(
        (f64::INFINITY, f64::NEG_INFINITY),
        |(lower, upper), value| (lower.min(value), upper.max(value)),
    );
    assert!(
        range.1 - range.0 > 0.9 * std::f64::consts::TAU,
        "the accepted continuation never crossed a complete periodic winding"
    );
    assert_close(
        observed_parameters[0],
        *observed_parameters.last().unwrap(),
        1.0e-8,
    );
}

#[test]
fn reanchored_feature_state_is_cold_reproducible_and_rejects_unrelated_input() {
    let document = SketchDocument::from_json(M70B_F005_ACCEPTED_JSON)
        .expect("payload-derived accepted sketch must decode");
    let session = retained(document);
    let features = ComputedFeatureDocument::from_json(M70B_F005_FEATURE_JSON)
        .expect("payload-derived feature intent must decode");
    let mut allocator = ComputedEvaluationAllocator::default();
    let previous = complete(
        ComputedFeatureEvaluationSnapshot::capture(
            &session,
            &features,
            ComputedFeatureEvaluationPolicy::default(),
        )
        .unwrap()
        .prepare(&mut allocator)
        .unwrap()
        .execute(OperationControl::unlimited())
        .unwrap(),
    );
    let reanchored = previous
        .reanchored_feature_document(&features)
        .expect("current payload feature can derive one exact re-anchor");
    let cold = complete(
        ComputedFeatureEvaluationSnapshot::capture(
            &session,
            &reanchored,
            ComputedFeatureEvaluationPolicy::default(),
        )
        .expect("ordinary capture accepts the derived persistent state")
        .prepare(&mut allocator)
        .unwrap()
        .execute(OperationControl::unlimited())
        .unwrap(),
    );
    let cold_reanchored = cold
        .reanchored_feature_document(&reanchored)
        .expect("ordinary cold evaluation independently reproduces the re-anchor");
    assert_eq!(cold_reanchored.features(), reanchored.features());
    assert_eq!(previous.edges().len(), cold.edges().len());
    for (continued, reproduced) in previous.edges().iter().zip(cold.edges()) {
        assert_eq!(continued.id.ordinal, reproduced.id.ordinal);
        assert_eq!(continued.role, reproduced.role);
        assert_eq!(continued.geometry, reproduced.geometry);
        assert_eq!(continued.provenance, reproduced.provenance);
    }
    assert_eq!(
        previous.construction_fragments().len(),
        cold.construction_fragments().len()
    );
    for (continued, reproduced) in previous
        .construction_fragments()
        .iter()
        .zip(cold.construction_fragments())
    {
        assert_eq!(continued.id.ordinal, reproduced.id.ordinal);
        assert_eq!(continued.source, reproduced.source);
        assert_eq!(continued.interval, reproduced.interval);
        assert_eq!(continued.source_role, reproduced.source_role);
        assert_eq!(continued.provenance, reproduced.provenance);
    }
    assert_eq!(previous.replaced_sources(), cold.replaced_sources());

    let mut unrelated = reanchored.clone();
    unrelated
        .set_fillet_radius(features.features()[0].id, 1.25)
        .expect("structurally valid but unrelated feature edit");
    assert!(matches!(
        previous.reanchored_feature_document(&unrelated),
        Err(ComputedFeatureReanchorError::SnapshotInputMismatch)
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
fn full_periodic_fillet_parent_remains_complete_and_has_no_trim_direction_action() {
    let fixture = line_circle_fixture();
    let session = retained(fixture.document.clone());
    let authoring = crate::ComputedFeatureAuthoringSnapshot::capture(&session).unwrap();
    let resolved = complete(
        authoring
            .resolve_fillet_corner(
                fixture.request,
                0.75,
                ComputedFeatureEvaluationPolicy::default(),
                OperationControl::unlimited(),
            )
            .unwrap(),
    );
    let mut features = ComputedFeatureDocument::new(fixture.document.id());
    let feature = features
        .create_fillet_set("line and full circle", 0.75, vec![resolved.corner])
        .unwrap();
    let mut allocator = ComputedEvaluationAllocator::default();
    let evaluated = evaluate(&session, &features, &mut allocator);

    assert!(matches!(
        evaluated
            .feature_evaluations()
            .iter()
            .find(|value| value.feature == feature)
            .unwrap()
            .state,
        ComputedFeatureEvaluationState::Current { .. }
    ));
    assert_eq!(arc_centers(&evaluated).len(), 1);
    assert_eq!(
        evaluated
            .source_fragment_edges(source(fixture.line))
            .count(),
        1,
        "the bounded line remains trim-capable"
    );
    assert_eq!(
        evaluated
            .source_fragment_edges(source(fixture.circle))
            .count(),
        0,
        "a full periodic parent remains visually complete"
    );
    assert_eq!(
        evaluated
            .source_construction_fragments(source(fixture.circle))
            .count(),
        0,
        "a full periodic parent has no artificial discarded complement"
    );
    assert_eq!(evaluated.construction_fragments().len(), 1);
    assert!(
        evaluated
            .edges()
            .iter()
            .all(|edge| edge.role == GeometryRole::Profile)
    );
    assert_eq!(evaluated.replaced_sources(), &[source(fixture.line)]);

    let alternatives = complete(
        authoring
            .local_fillet_corner_alternatives(
                resolved.corner,
                0.75,
                ComputedFeatureEvaluationPolicy::default(),
                OperationControl::unlimited(),
            )
            .unwrap(),
    );
    assert!(!alternatives.iter().any(|alternative| {
        matches!(
            alternative.kind,
            ComputedFilletCornerAlternativeKind::RetainedEndpoint {
                parent: ComputedFilletParentIndex::Second,
                ..
            }
        )
    }));
    assert!(alternatives.iter().any(|alternative| {
        matches!(
            alternative.kind,
            ComputedFilletCornerAlternativeKind::RetainedEndpoint {
                parent: ComputedFilletParentIndex::First,
                ..
            }
        )
    }));
}

#[test]
fn open_periodic_parent_publishes_only_its_exact_visible_discarded_complement() {
    let mut fixture = line_circle_fixture();
    let visible = DocumentCurveTrimView {
        support: fixture.circle,
        start: DocumentTrimBoundary::Fixed(DocumentTrimParameter {
            parameter: 0.1,
            winding: 0,
        }),
        end: DocumentTrimBoundary::Fixed(DocumentTrimParameter {
            parameter: 6.2,
            winding: 0,
        }),
    };
    fixture
        .document
        .replace_trim_views(fixture.circle, vec![visible])
        .unwrap();
    let session = retained(fixture.document.clone());
    let authoring = crate::ComputedFeatureAuthoringSnapshot::capture(&session).unwrap();
    let resolved = complete(
        authoring
            .resolve_fillet_corner(
                fixture.request,
                0.75,
                ComputedFeatureEvaluationPolicy::default(),
                OperationControl::unlimited(),
            )
            .unwrap(),
    );
    let mut features = ComputedFeatureDocument::new(fixture.document.id());
    features
        .create_fillet_set("open periodic parent", 0.75, vec![resolved.corner])
        .unwrap();
    let evaluated = evaluate(
        &session,
        &features,
        &mut ComputedEvaluationAllocator::default(),
    );

    assert_eq!(
        evaluated
            .source_fragment_edges(source(fixture.circle))
            .count(),
        1,
        "an explicitly open periodic view remains trim-capable"
    );
    let discarded = evaluated
        .source_construction_fragments(source(fixture.circle))
        .collect::<Vec<_>>();
    assert_eq!(discarded.len(), 1);
    let base_interval = discarded[0].provenance.base_interval;
    assert!(base_interval.start >= 0.1);
    assert!(base_interval.end <= 6.2);
    assert!(base_interval.end - base_interval.start < std::f64::consts::TAU);
    assert_eq!(
        discarded[0].provenance.endpoint,
        resolved.corner.second.retained_endpoint
    );
    let retained = evaluated
        .source_fragment_edges(source(fixture.circle))
        .next()
        .unwrap();
    let ComputedEdgeGeometry::NativeSourceFragment {
        interval: retained_interval,
        ..
    } = retained.geometry
    else {
        panic!("expected retained periodic source fragment");
    };
    match discarded[0].provenance.endpoint {
        DocumentFilletTrimEndpoint::Start => {
            assert_eq!(
                discarded[0].interval.start.to_bits(),
                base_interval.start.to_bits()
            );
            assert_eq!(
                discarded[0].interval.end.to_bits(),
                retained_interval.start.to_bits()
            );
            assert_eq!(retained_interval.end.to_bits(), base_interval.end.to_bits());
        }
        DocumentFilletTrimEndpoint::End => {
            assert_eq!(
                retained_interval.start.to_bits(),
                base_interval.start.to_bits()
            );
            assert_eq!(
                retained_interval.end.to_bits(),
                discarded[0].interval.start.to_bits()
            );
            assert_eq!(
                discarded[0].interval.end.to_bits(),
                base_interval.end.to_bits()
            );
        }
    }
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
