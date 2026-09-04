// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_sketch::{
    ContactNeighborhood, CurveCurveFilletRequest, CurveDefinition, CurveFilletParentRequest,
    CurveSpan, DocumentArcSweep, DocumentCurveNormalSide, DocumentCurveTrimView,
    DocumentDimensionMode, DocumentFilletEndpointOrder, DocumentFilletTrimEndpoint,
    DocumentTrimBoundary, DocumentTrimParameter, SketchDocument,
};

fn line(document: &mut SketchDocument, label: &str, start: [f64; 2], end: [f64; 2]) -> CurveSpan {
    let start_id = document
        .add_point(format!("{label} start"), start)
        .expect("line start");
    let end_id = document
        .add_point(format!("{label} end"), end)
        .expect("line end");
    let delta = [end[0] - start[0], end[1] - start[1]];
    let length = delta[0].hypot(delta[1]);
    let curve = document
        .add_curve(
            label,
            CurveDefinition::Line {
                start: start_id,
                end: end_id,
                branch_direction: [delta[0] / length, delta[1] / length],
            },
        )
        .expect("line curve");
    CurveSpan::line(curve)
}

fn fixed_trim_view(support: CurveSpan, start: f64, end: f64) -> DocumentCurveTrimView {
    DocumentCurveTrimView {
        support,
        start: DocumentTrimBoundary::Fixed(DocumentTrimParameter {
            parameter: start,
            winding: 0,
        }),
        end: DocumentTrimBoundary::Fixed(DocumentTrimParameter {
            parameter: end,
            winding: 0,
        }),
    }
}

fn fillet_parent(
    curve: CurveSpan,
    parameter: f64,
    side: DocumentCurveNormalSide,
    trim_endpoint: DocumentFilletTrimEndpoint,
) -> CurveFilletParentRequest {
    CurveFilletParentRequest {
        curve,
        parameter,
        winding: 0,
        neighborhood: ContactNeighborhood::Interior,
        side,
        trim_endpoint,
        periodic_anchor: None,
    }
}

#[test]
fn trim_mutations_preserve_canonical_order_across_draft_checkpoint_round_trip() {
    let mut document = SketchDocument::new(10.0).expect("document");
    let fillet_horizontal = line(&mut document, "fillet horizontal", [-2.0, 0.0], [2.0, 0.0]);
    let fillet_vertical = line(&mut document, "fillet vertical", [0.0, -2.0], [0.0, 2.0]);
    let later_support = line(&mut document, "later support", [-2.0, 4.0], [2.0, 4.0]);
    let latest_support = line(&mut document, "latest support", [-2.0, 6.0], [2.0, 6.0]);

    // Operation scheduling may replace later-created supports first.
    document
        .replace_trim_views(
            latest_support,
            vec![fixed_trim_view(latest_support, 0.2, 0.8)],
        )
        .expect("latest replacement");
    document
        .replace_trim_views(
            later_support,
            vec![fixed_trim_view(later_support, 0.1, 0.9)],
        )
        .expect("later replacement");
    assert_eq!(
        document
            .trim_views()
            .iter()
            .map(|view| view.support)
            .collect::<Vec<_>>(),
        vec![later_support, latest_support],
    );

    // A later operation can also append contact-owned views for lower-ID supports.
    document
        .add_curve_curve_fillet(
            "lower support fillet",
            CurveCurveFilletRequest {
                first: fillet_parent(
                    fillet_horizontal,
                    0.75,
                    DocumentCurveNormalSide::Left,
                    DocumentFilletTrimEndpoint::End,
                ),
                second: fillet_parent(
                    fillet_vertical,
                    0.75,
                    DocumentCurveNormalSide::Right,
                    DocumentFilletTrimEndpoint::Start,
                ),
                endpoint_order: DocumentFilletEndpointOrder::SecondThenFirst,
                sweep: DocumentArcSweep::CounterClockwise,
                radius: 1.0,
                radius_mode: DocumentDimensionMode::Driving,
            },
        )
        .expect("lower-ID support fillet");

    let expected_supports = vec![
        fillet_horizontal,
        fillet_vertical,
        later_support,
        latest_support,
    ];
    assert_eq!(
        document
            .trim_views()
            .iter()
            .map(|view| view.support)
            .collect::<Vec<_>>(),
        expected_supports,
    );
    assert_eq!(
        document.visible_intervals(later_support).unwrap()[0]
            .start
            .to_bits(),
        0.1_f64.to_bits(),
    );
    assert_eq!(
        document.visible_intervals(latest_support).unwrap()[0]
            .end
            .to_bits(),
        0.8_f64.to_bits(),
    );

    let checkpoint = document.to_draft_v5_json().expect("draft-v5 checkpoint");
    let restored = SketchDocument::from_draft_v5_json(&checkpoint).expect("restored checkpoint");
    assert_eq!(restored, document);
    assert_eq!(
        restored
            .trim_views()
            .iter()
            .map(|view| view.support)
            .collect::<Vec<_>>(),
        expected_supports,
    );
}
