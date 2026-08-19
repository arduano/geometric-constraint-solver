// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_sketch::{
    CurveDefinition, CurveSpan, DocumentArcSweep, DocumentCurveNormalSide,
    DocumentFilletEndpointOrder, DocumentNativeLineFilletCreationRequest,
    DocumentNativeLineFilletParent, DocumentSolveRequest, FeatureEndpoint,
    RetainedSketchDocumentSession, SketchDocument, SolverConfig, TangentOrientation,
};
use geosolve_sketch_topology::{
    OffsetEndpointEligibility, OffsetEndpointRef, OffsetEndpointRole, OffsetJoinOwner,
    OffsetOperandCurveFamily, OffsetOperandRequest, PreparedOffsetOperandQuery,
};

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one topology-owner fixture keeps all three eligible spans and both exact tangency-owned joins together"
)]
fn native_line_fillet_is_an_ordinary_offset_eligible_line_arc_line_chain() {
    let mut document = SketchDocument::new(4.0).unwrap();
    let first_outer = document.add_point("first outer", [-3.0, 0.0]).unwrap();
    let corner = document.add_point("sharp corner", [0.0, 0.0]).unwrap();
    let second_outer = document.add_point("second outer", [0.0, 3.0]).unwrap();
    let first_line = document
        .add_curve(
            "first line",
            CurveDefinition::Line {
                start: first_outer,
                end: corner,
                branch_direction: [1.0, 0.0],
            },
        )
        .unwrap();
    let second_line = document
        .add_curve(
            "second line",
            CurveDefinition::Line {
                start: corner,
                end: second_outer,
                branch_direction: [0.0, 1.0],
            },
        )
        .unwrap();
    let ids = document
        .create_native_line_fillet_geometry(DocumentNativeLineFilletCreationRequest {
            label: "native corner".into(),
            first: DocumentNativeLineFilletParent {
                curve: CurveSpan::line(first_line),
                endpoint: FeatureEndpoint::End,
                normal_side: DocumentCurveNormalSide::Left,
                tangent_orientation: TangentOrientation::Aligned,
                contact_position: [-1.0, 0.0],
            },
            second: DocumentNativeLineFilletParent {
                curve: CurveSpan::line(second_line),
                endpoint: FeatureEndpoint::Start,
                normal_side: DocumentCurveNormalSide::Left,
                tangent_orientation: TangentOrientation::Aligned,
                contact_position: [0.0, 1.0],
            },
            endpoint_order: DocumentFilletEndpointOrder::FirstThenSecond,
            center: [-1.0, 1.0],
            radius: 1.0,
            start_angle: -std::f64::consts::FRAC_PI_2,
            end_angle: 0.0,
            sweep: DocumentArcSweep::CounterClockwise,
        })
        .unwrap();
    let session = RetainedSketchDocumentSession::new(
        document,
        DocumentSolveRequest::default().without_previous_state_preferences(),
        SolverConfig::default(),
    )
    .unwrap();
    let result = PreparedOffsetOperandQuery::capture(&session, OffsetOperandRequest::default())
        .unwrap()
        .execute(geosolve_sketch::OperationControl::unlimited())
        .unwrap();
    let geosolve_sketch::OperationOutcome::Completed { value: result, .. } = result else {
        panic!("unbounded native-Fillet topology query must complete");
    };
    let index = result.operand_index.unwrap();

    let spans = [
        (CurveSpan::line(first_line), OffsetOperandCurveFamily::Line),
        (
            CurveSpan::line(ids.arc),
            OffsetOperandCurveFamily::CircularArc,
        ),
        (CurveSpan::line(second_line), OffsetOperandCurveFamily::Line),
    ];
    for (span, family) in spans {
        let candidate = index.span(span).unwrap();
        assert_eq!(candidate.family, family);
        assert!(candidate.eligibility.is_eligible());
    }

    let expected = [
        (
            OffsetEndpointRef {
                span: CurveSpan::line(first_line),
                endpoint: OffsetEndpointRole::End,
            },
            OffsetEndpointRef {
                span: CurveSpan::line(ids.arc),
                endpoint: OffsetEndpointRole::Start,
            },
            ids.tangencies[0],
        ),
        (
            OffsetEndpointRef {
                span: CurveSpan::line(ids.arc),
                endpoint: OffsetEndpointRole::End,
            },
            OffsetEndpointRef {
                span: CurveSpan::line(second_line),
                endpoint: OffsetEndpointRole::Start,
            },
            ids.tangencies[1],
        ),
    ];
    for (first, second, owner) in expected {
        let endpoints = if first < second {
            [first, second]
        } else {
            [second, first]
        };
        let adjacency = index
            .adjacencies()
            .iter()
            .find(|adjacency| adjacency.endpoints == endpoints)
            .expect("native LineCurveTangency must own the exact line/arc endpoints");
        assert_eq!(adjacency.owners, vec![OffsetJoinOwner::Constraint(owner)]);
        for endpoint in [first, second] {
            let candidate = index
                .span(endpoint.span)
                .unwrap()
                .endpoints
                .iter()
                .find(|candidate| candidate.endpoint == endpoint)
                .unwrap();
            assert_eq!(candidate.eligibility, OffsetEndpointEligibility::Joined);
        }
    }
    assert_eq!(index.adjacencies().len(), 2);
    assert_eq!(
        index
            .span(CurveSpan::line(first_line))
            .unwrap()
            .endpoints
            .iter()
            .find(|candidate| candidate.endpoint.endpoint == OffsetEndpointRole::Start)
            .unwrap()
            .eligibility,
        OffsetEndpointEligibility::Terminal
    );
    assert_eq!(
        index
            .span(CurveSpan::line(second_line))
            .unwrap()
            .endpoints
            .iter()
            .find(|candidate| candidate.endpoint.endpoint == OffsetEndpointRole::End)
            .unwrap()
            .eligibility,
        OffsetEndpointEligibility::Terminal
    );
}
