// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_sketch::{
    ContactNeighborhood, CurveDefinition, CurveSpan, DocumentBSplineForm, DocumentDimensionMode,
    DocumentEndpointRef, DocumentM38DimensionDefinition, DocumentMeasurementCatalog,
    DocumentPointRef, FeatureEndpoint, ScalarDomain, ScalarUnit, SketchDocument,
};

fn endpoint(curve: geosolve_sketch::CurveId, endpoint: FeatureEndpoint) -> DocumentEndpointRef {
    DocumentEndpointRef { curve, endpoint }
}

fn add_open_polyline(document: &mut SketchDocument) -> geosolve_sketch::CurveId {
    let points = [[0.0, 0.0], [2.0, 0.0], [2.0, 3.0]]
        .map(|position| document.add_point("polyline point", position).unwrap());
    document
        .add_curve(
            "open polyline",
            CurveDefinition::Polyline {
                points: points.to_vec(),
                closed: false,
                branch_directions: vec![[1.0, 0.0], [0.0, 1.0]],
            },
        )
        .unwrap()
}

fn assert_endpoint_rejected(document: &SketchDocument, curve: geosolve_sketch::CurveId) {
    for selected in [FeatureEndpoint::Start, FeatureEndpoint::End] {
        let endpoint = endpoint(curve, selected);
        assert!(document.validate_endpoint_ref(endpoint).is_err());
        assert!(document.curve_endpoint_contact_seed(endpoint).is_err());
    }
}

#[test]
fn endpoint_contact_seed_uses_first_and_last_semantic_spans() {
    let mut document = SketchDocument::new(1.0).unwrap();
    let polyline = add_open_polyline(&mut document);

    let controls = [[-1.0, 0.0], [0.0, 2.0], [1.0, -1.0], [2.0, 2.0], [3.0, 0.0]]
        .map(|position| document.add_point("spline control", position).unwrap());
    let spline = document
        .add_curve(
            "open spline",
            CurveDefinition::BSpline {
                form: DocumentBSplineForm::Clamped,
                degree: 2,
                controls: controls.to_vec(),
                knots: vec![0.0, 0.0, 0.0, 1.0, 2.0, 3.0, 3.0, 3.0],
                span_ids: vec![11, 17, 29],
                next_span_id: 30,
            },
        )
        .unwrap();

    for (curve, first_segment, last_segment) in [(polyline, 0, 1), (spline, 11, 29)] {
        let start = document
            .curve_endpoint_contact_seed(endpoint(curve, FeatureEndpoint::Start))
            .unwrap();
        assert_eq!(
            start.support.span,
            CurveSpan {
                curve,
                segment: first_segment
            }
        );
        assert_eq!(start.support.winding, 0);
        assert_eq!(start.parameter.to_bits(), 0.0f64.to_bits());
        assert_eq!(start.neighborhood, ContactNeighborhood::Start);

        let end = document
            .curve_endpoint_contact_seed(endpoint(curve, FeatureEndpoint::End))
            .unwrap();
        assert_eq!(
            end.support.span,
            CurveSpan {
                curve,
                segment: last_segment
            }
        );
        assert_eq!(end.support.winding, 0);
        assert_eq!(end.parameter.to_bits(), 1.0f64.to_bits());
        assert_eq!(end.neighborhood, ContactNeighborhood::End);
    }
}

#[test]
fn endpoint_capability_rejects_every_closed_or_periodic_topology() {
    let mut document = SketchDocument::new(1.0).unwrap();
    let points = [[0.0, 0.0], [2.0, 0.0], [1.0, 1.0]]
        .map(|position| document.add_point("closed point", position).unwrap());
    let closed_polyline = document
        .add_curve(
            "closed polyline",
            CurveDefinition::Polyline {
                points: points.to_vec(),
                closed: true,
                branch_directions: vec![
                    [1.0, 0.0],
                    [
                        -std::f64::consts::FRAC_1_SQRT_2,
                        std::f64::consts::FRAC_1_SQRT_2,
                    ],
                    [-std::f64::consts::FRAC_1_SQRT_2; 2],
                ],
            },
        )
        .unwrap();

    let center = document.add_point("center", [0.0, 0.0]).unwrap();
    let major = document.add_point("major", [2.0, 0.0]).unwrap();
    let radius = document
        .add_scalar("radius", 2.0, ScalarUnit::Length, ScalarDomain::Positive)
        .unwrap();
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
    let circle = document
        .add_curve("circle", CurveDefinition::Circle { center, radius })
        .unwrap();
    let ellipse = document
        .add_curve(
            "ellipse",
            CurveDefinition::Ellipse {
                center,
                major_axis_point: major,
                minor_axis_ratio: ratio,
            },
        )
        .unwrap();

    let spline_controls = [[0.0, 0.0], [1.5, -0.2], [2.0, 1.4], [0.5, 2.2], [-0.8, 1.0]]
        .map(|position| document.add_point("periodic control", position).unwrap());
    let periodic_bspline = document
        .add_curve(
            "periodic B-spline",
            CurveDefinition::BSpline {
                form: DocumentBSplineForm::Periodic,
                degree: 2,
                controls: spline_controls.to_vec(),
                knots: vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0],
                span_ids: vec![11, 17, 23, 29, 31],
                next_span_id: 32,
            },
        )
        .unwrap();
    let weights = [0.75, 1.0, 1.4, 0.9, 1.2].map(|value| {
        document
            .add_scalar(
                "periodic weight",
                value,
                ScalarUnit::Parameter,
                ScalarDomain::Positive,
            )
            .unwrap()
    });
    let periodic_nurbs = document
        .add_curve(
            "periodic NURBS",
            CurveDefinition::Nurbs {
                form: DocumentBSplineForm::Periodic,
                degree: 2,
                controls: spline_controls.to_vec(),
                weights: weights.to_vec(),
                gauge_weight: weights[1],
                knots: vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0],
                span_ids: vec![41, 43, 47, 53, 59],
                next_span_id: 60,
            },
        )
        .unwrap();

    for curve in [
        closed_polyline,
        circle,
        ellipse,
        periodic_bspline,
        periodic_nurbs,
    ] {
        assert_endpoint_rejected(&document, curve);
    }
}

#[test]
fn m38_endpoint_measurement_evaluates_the_last_polyline_span() {
    let mut document = SketchDocument::new(1.0).unwrap();
    let polyline = add_open_polyline(&mut document);
    let mut catalog = DocumentMeasurementCatalog::new(&mut document).unwrap();
    let source = catalog
        .add_dimension(
            &mut document,
            "polyline endpoint height",
            DocumentM38DimensionDefinition::RelativeVertical {
                first: DocumentPointRef::Endpoint(endpoint(polyline, FeatureEndpoint::Start)),
                second: DocumentPointRef::Endpoint(endpoint(polyline, FeatureEndpoint::End)),
            },
            DocumentDimensionMode::Reference,
            0.0,
        )
        .unwrap();

    let measured = catalog.evaluate_dimension(&document, source).unwrap();
    assert_eq!(measured.value.to_bits(), 3.0f64.to_bits());
}
