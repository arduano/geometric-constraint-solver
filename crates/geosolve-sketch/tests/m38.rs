// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_sketch::{
    CancellationToken, CurveDefinition, CurveSpan, DocumentAngleOrientation,
    DocumentBoundedCurveInterval, DocumentConicMeasurement, DocumentCoordinateAxis,
    DocumentCurveSpanRef, DocumentDatumAxis, DocumentDimensionMode, DocumentDirectionSense,
    DocumentLineSide, DocumentLineSupportRef, DocumentM38DimensionDefinition,
    DocumentMeasurementCatalog, DocumentMeasurementDefinition, DocumentMeasurementProvenance,
    DocumentPointRef, DocumentSolveRequest, OperationControl, OperationLimits, OperationOutcome,
    RetainedSketchDocumentSession, ScalarDomain, ScalarUnit, SketchDocument, cancellation_pair,
};

fn point(id: geosolve_sketch::DesignPointId) -> DocumentPointRef {
    DocumentPointRef::Point { point: id }
}

fn support(curve: geosolve_sketch::CurveId) -> DocumentLineSupportRef {
    DocumentLineSupportRef {
        span: CurveSpan::line(curve),
        direction: DocumentDirectionSense::Forward,
    }
}

#[test]
#[allow(clippy::too_many_lines)]
fn signed_coordinate_spacing_angle_and_arc_dimensions_share_reference_values() {
    let mut document = SketchDocument::new(1.0).unwrap();
    let datum = document.add_point("datum", [1.0, 2.0]).unwrap();
    let a = document.add_point("a", [2.0, 3.0]).unwrap();
    let b = document.add_point("b", [5.0, 7.0]).unwrap();
    let c = document.add_point("c", [2.0, 6.0]).unwrap();
    let l1 = document
        .add_curve(
            "l1",
            CurveDefinition::Line {
                start: a,
                end: b,
                branch_direction: [1.0, 0.0],
            },
        )
        .unwrap();
    let d = document.add_point("d", [5.0, 10.0]).unwrap();
    let l2 = document
        .add_curve(
            "l2",
            CurveDefinition::Line {
                start: c,
                end: d,
                branch_direction: [1.0, 0.0],
            },
        )
        .unwrap();
    let radius = document
        .add_scalar("r", 2.0, ScalarUnit::Length, ScalarDomain::Positive)
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
                center: datum,
                radius,
                start_angle: start,
                end_angle: end,
                sweep: geosolve_sketch::DocumentArcSweep::CounterClockwise,
            },
        )
        .unwrap();
    let support = |curve| DocumentLineSupportRef {
        span: CurveSpan::line(curve),
        direction: DocumentDirectionSense::Forward,
    };

    let cases = [
        (
            DocumentM38DimensionDefinition::RelativeHorizontal {
                first: point(a),
                second: point(b),
            },
            3.0,
        ),
        (
            DocumentM38DimensionDefinition::RelativeVertical {
                first: point(a),
                second: point(b),
            },
            4.0,
        ),
        (
            DocumentM38DimensionDefinition::DatumCoordinate {
                point: point(b),
                datum: DocumentDatumAxis {
                    origin: point(datum),
                    axis: DocumentCoordinateAxis::X,
                },
            },
            4.0,
        ),
        (
            DocumentM38DimensionDefinition::PointLineDistance {
                point: point(c),
                line: support(l1),
                side: DocumentLineSide::Left,
            },
            1.8,
        ),
        (
            DocumentM38DimensionDefinition::ParallelLineSeparation {
                first: support(l1),
                second: support(l2),
                side: DocumentLineSide::Left,
            },
            1.8,
        ),
        (
            DocumentM38DimensionDefinition::TwoLineAngle {
                first: support(l1),
                second: support(l2),
                orientation: DocumentAngleOrientation::CounterClockwise,
                winding: 0,
            },
            0.0,
        ),
        (
            DocumentM38DimensionDefinition::ThreePointAngle {
                first: point(a),
                vertex: point(b),
                second: point(c),
                orientation: DocumentAngleOrientation::CounterClockwise,
                winding: 0,
            },
            -0.605_544_663_604_970_1,
        ),
        (
            DocumentM38DimensionDefinition::CircularSweep { arc },
            std::f64::consts::FRAC_PI_2,
        ),
        (
            DocumentM38DimensionDefinition::CircularArcLength { arc },
            std::f64::consts::PI,
        ),
        (
            DocumentM38DimensionDefinition::SegmentLength { line: support(l1) },
            5.0,
        ),
    ];
    let mut catalog = DocumentMeasurementCatalog::new(&mut document).unwrap();
    for (definition, expected) in cases {
        let reference = catalog
            .add_dimension(
                &mut document,
                "reference",
                definition.clone(),
                DocumentDimensionMode::Reference,
                expected,
            )
            .unwrap();
        let driving = catalog
            .add_dimension(
                &mut document,
                "driving",
                definition,
                DocumentDimensionMode::Driving,
                expected,
            )
            .unwrap();
        let rv = catalog.evaluate_dimension(&document, reference).unwrap();
        let dv = catalog.evaluate_dimension(&document, driving).unwrap();
        assert!((rv.value - expected).abs() <= 1e-9, "{rv:?}");
        assert_eq!(rv.value.to_bits(), dv.value.to_bits());
        assert!(dv.residual.unwrap().abs() <= 1e-9);
    }
}

#[test]
fn persistent_curve_measurements_and_bounded_lengths_are_typed_and_bounded() {
    let mut document = SketchDocument::new(1.0).unwrap();
    let p0 = document.add_point("p0", [0.0, 0.0]).unwrap();
    let p1 = document.add_point("p1", [1.0, 0.0]).unwrap();
    let p2 = document.add_point("p2", [1.0, 1.0]).unwrap();
    let curve = document
        .add_curve(
            "q",
            CurveDefinition::QuadraticBezier {
                controls: [p0, p1, p2],
            },
        )
        .unwrap();
    let contact = document
        .add_curve_contact(
            "contact",
            CurveSpan::line(curve),
            0.5,
            0,
            geosolve_sketch::ContactNeighborhood::Interior,
            None,
        )
        .unwrap();
    let interval = DocumentBoundedCurveInterval {
        support: DocumentCurveSpanRef {
            span: CurveSpan::line(curve),
            winding: 0,
        },
        start: 0.0,
        end: 1.0,
    };
    let mut catalog = DocumentMeasurementCatalog::new(&mut document).unwrap();
    let curvature = catalog
        .add_measurement(
            &mut document,
            "k",
            DocumentMeasurementDefinition::SignedCurvature { contact },
            DocumentMeasurementProvenance::AcceptedDocument { revision: 0 },
        )
        .unwrap();
    let length = catalog
        .add_measurement(
            &mut document,
            "length",
            DocumentMeasurementDefinition::BoundedCurveLength { interval },
            DocumentMeasurementProvenance::AcceptedDocument { revision: 0 },
        )
        .unwrap();
    let json = catalog.to_canonical_json().unwrap();
    let document_json = document.to_canonical_json().unwrap();
    let mut restored_document = SketchDocument::from_json(&document_json).unwrap();
    let restored = DocumentMeasurementCatalog::from_json(&mut restored_document, &json).unwrap();
    let session = RetainedSketchDocumentSession::new(
        restored_document,
        DocumentSolveRequest::default(),
        geosolve_core::SolverConfig::default(),
    )
    .unwrap();
    let k = restored.evaluate_measurement(&session, curvature).unwrap();
    assert_eq!(k.unit, geosolve_sketch::DocumentScalarUnit::Curvature);
    assert!(k.value.is_finite());
    let l = restored.evaluate_measurement(&session, length).unwrap();
    assert_eq!(l.unit, geosolve_sketch::DocumentScalarUnit::Length);
    assert!((l.value - 1.623_225_240_140_230_5).abs() < 1e-8);
    assert!(l.work.integrations > 0 && l.work.derivative_evaluations > 0);

    let mut limits = OperationLimits::unlimited();
    limits.measurement_integrations = 0;
    let exhausted = restored
        .evaluate_measurement_controlled(
            &session,
            length,
            OperationControl::new(CancellationToken::default(), limits),
        )
        .unwrap();
    assert!(matches!(exhausted, OperationOutcome::WorkExhausted { .. }));

    let before_cancel = session.design_document().to_canonical_json().unwrap();
    let (handle, token) = cancellation_pair();
    handle.cancel();
    let cancelled = restored
        .evaluate_measurement_controlled(
            &session,
            length,
            OperationControl::new(token, OperationLimits::unlimited()),
        )
        .unwrap();
    assert!(matches!(cancelled, OperationOutcome::Cancelled { .. }));
    assert_eq!(
        session.design_document().to_canonical_json().unwrap(),
        before_cancel
    );
}

#[test]
fn persistent_measurements_require_current_lifecycle_provenance() {
    let mut document = SketchDocument::new(1.0).unwrap();
    let a = document.add_point("a", [0.0, 0.0]).unwrap();
    let b = document.add_point("b", [2.0, 0.0]).unwrap();
    let line = document
        .add_curve(
            "line",
            CurveDefinition::Line {
                start: a,
                end: b,
                branch_direction: [1.0, 0.0],
            },
        )
        .unwrap();
    let definition = DocumentMeasurementDefinition::BoundedCurveLength {
        interval: DocumentBoundedCurveInterval {
            support: DocumentCurveSpanRef {
                span: CurveSpan::line(line),
                winding: 0,
            },
            start: 0.0,
            end: 1.0,
        },
    };
    let mut catalog = DocumentMeasurementCatalog::new(&mut document).unwrap();
    let accepted = catalog
        .add_measurement(
            &mut document,
            "accepted",
            definition.clone(),
            DocumentMeasurementProvenance::AcceptedDocument { revision: 0 },
        )
        .unwrap();
    let retained = catalog
        .add_measurement(
            &mut document,
            "retained",
            definition.clone(),
            DocumentMeasurementProvenance::RetainedDesign { revision: 0 },
        )
        .unwrap();
    let stale = catalog
        .add_measurement(
            &mut document,
            "stale",
            definition,
            DocumentMeasurementProvenance::AcceptedDocument { revision: 1 },
        )
        .unwrap();
    let session = RetainedSketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        geosolve_core::SolverConfig::default(),
    )
    .unwrap();
    let before = session.design_document().to_canonical_json().unwrap();

    for source in [accepted, retained] {
        let value = catalog.evaluate_measurement(&session, source).unwrap();
        assert!((value.value - 2.0).abs() <= f64::EPSILON);
        assert_eq!(
            value.audit.provenance,
            Some(if source == accepted {
                DocumentMeasurementProvenance::AcceptedDocument { revision: 0 }
            } else {
                DocumentMeasurementProvenance::RetainedDesign { revision: 0 }
            })
        );
    }
    assert!(catalog.evaluate_measurement(&session, stale).is_err());

    let foreign = RetainedSketchDocumentSession::new(
        SketchDocument::new(1.0).unwrap(),
        DocumentSolveRequest::default(),
        geosolve_core::SolverConfig::default(),
    )
    .unwrap();
    assert!(catalog.evaluate_measurement(&foreign, accepted).is_err());
    assert_eq!(
        session.design_document().to_canonical_json().unwrap(),
        before
    );
}

#[test]
fn coordinate_residual_central_difference_is_scale_and_translation_stable() {
    for scale in [1.0e-6, 1.0, 1.0e6] {
        let mut document = SketchDocument::new(scale).unwrap();
        let first = document
            .add_point("first", [7.0 * scale, -11.0 * scale])
            .unwrap();
        let second = document
            .add_point("second", [10.0 * scale, -5.0 * scale])
            .unwrap();
        let mut catalog = DocumentMeasurementCatalog::new(&mut document).unwrap();
        let source = catalog
            .add_dimension(
                &mut document,
                "dx",
                DocumentM38DimensionDefinition::RelativeHorizontal {
                    first: point(first),
                    second: point(second),
                },
                DocumentDimensionMode::Driving,
                3.0 * scale,
            )
            .unwrap();
        let step = scale * 1.0e-6;
        document
            .set_point_position(second, [10.0 * scale + step, -5.0 * scale])
            .unwrap();
        let plus = catalog
            .evaluate_dimension(&document, source)
            .unwrap()
            .residual
            .unwrap();
        document
            .set_point_position(second, [10.0 * scale - step, -5.0 * scale])
            .unwrap();
        let minus = catalog
            .evaluate_dimension(&document, source)
            .unwrap()
            .residual
            .unwrap();
        let derivative = (plus - minus) / (2.0 * step);
        assert!((derivative - 1.0).abs() < 1.0e-7, "scale={scale:e}");
    }
}

#[test]
fn driving_coordinate_dimensions_lower_into_the_solver_and_commit_atomically() {
    let mut document = SketchDocument::new(10.0).unwrap();
    let origin = document.add_point("origin", [0.0, 0.0]).unwrap();
    let a = document.add_point("a", [1.0, 2.0]).unwrap();
    let b = document.add_point("b", [3.0, 4.0]).unwrap();
    let mut catalog = DocumentMeasurementCatalog::new(&mut document).unwrap();
    for (definition, target) in [
        (
            DocumentM38DimensionDefinition::DatumCoordinate {
                point: point(a),
                datum: DocumentDatumAxis {
                    origin: point(origin),
                    axis: DocumentCoordinateAxis::X,
                },
            },
            1.0,
        ),
        (
            DocumentM38DimensionDefinition::DatumCoordinate {
                point: point(a),
                datum: DocumentDatumAxis {
                    origin: point(origin),
                    axis: DocumentCoordinateAxis::Y,
                },
            },
            2.0,
        ),
        (
            DocumentM38DimensionDefinition::DatumCoordinate {
                point: point(b),
                datum: DocumentDatumAxis {
                    origin: point(origin),
                    axis: DocumentCoordinateAxis::Y,
                },
            },
            -1.0,
        ),
        (
            DocumentM38DimensionDefinition::RelativeHorizontal {
                first: point(a),
                second: point(b),
            },
            5.0,
        ),
        (
            DocumentM38DimensionDefinition::RelativeVertical {
                first: point(a),
                second: point(b),
            },
            -3.0,
        ),
    ] {
        catalog
            .add_dimension(
                &mut document,
                "coordinate drive",
                definition,
                DocumentDimensionMode::Driving,
                target,
            )
            .unwrap();
    }
    let solved = catalog
        .solve_document(
            &document,
            geosolve_sketch::SketchSolveRequest::default().without_previous_state_preferences(),
            geosolve_core::SolverConfig::default(),
        )
        .unwrap();
    assert!(solved.solve_result.accepted());
    assert_eq!(solved.audit.len(), 5);
    let solved_origin = solved.document.point(origin).unwrap().position;
    let solved_a = solved.document.point(a).unwrap().position;
    let solved_b = solved.document.point(b).unwrap().position;
    assert!((solved_a[0] - solved_origin[0] - 1.0).abs() < 1.0e-9);
    assert!((solved_b[0] - solved_origin[0] - 6.0).abs() < 1.0e-9);
    assert!((solved_b[0] - solved_a[0] - 5.0).abs() < 1.0e-9);
    assert!((solved_b[1] - solved_origin[1] + 1.0).abs() < 1.0e-9);
    assert!((solved_b[1] - solved_a[1] + 3.0).abs() < 1.0e-9);
    assert!(
        solved
            .audit
            .iter()
            .all(|row| row.residual.unwrap().abs() <= 1.0e-9)
    );
}

#[test]
fn driving_parallel_separation_emits_parallelism_and_signed_offset_rows() {
    let mut document = SketchDocument::new(5.0).unwrap();
    let a = document.add_point("a", [0.0, 0.0]).unwrap();
    let b = document.add_point("b", [4.0, 0.0]).unwrap();
    let c = document.add_point("c", [0.0, 3.0]).unwrap();
    let d = document.add_point("d", [3.0, 4.0]).unwrap();
    let first = document
        .add_curve(
            "first",
            CurveDefinition::Line {
                start: a,
                end: b,
                branch_direction: [1.0, 0.0],
            },
        )
        .unwrap();
    let second = document
        .add_curve(
            "second",
            CurveDefinition::Line {
                start: c,
                end: d,
                branch_direction: [1.0, 0.0],
            },
        )
        .unwrap();
    let mut catalog = DocumentMeasurementCatalog::new(&mut document).unwrap();
    let source = catalog
        .add_dimension(
            &mut document,
            "parallel offset",
            DocumentM38DimensionDefinition::ParallelLineSeparation {
                first: support(first),
                second: support(second),
                side: DocumentLineSide::Left,
            },
            DocumentDimensionMode::Driving,
            2.0,
        )
        .unwrap();
    let solved = catalog
        .solve_document(
            &document,
            geosolve_sketch::SketchSolveRequest::default().without_previous_state_preferences(),
            geosolve_core::SolverConfig::default(),
        )
        .unwrap();
    let value = catalog
        .evaluate_dimension(&solved.document, source)
        .unwrap();
    assert!(
        (value.value - 2.0).abs() / solved.document.model_scale()
            <= geosolve_sketch::SKETCH_ACCEPTANCE_RESIDUAL_TOLERANCE,
        "{value:?}"
    );
    assert!(
        value.residual.unwrap().abs() / solved.document.model_scale()
            <= geosolve_sketch::SKETCH_ACCEPTANCE_RESIDUAL_TOLERANCE
    );
    assert!(solved.solve_result.accepted());
}

#[test]
fn driving_segment_length_and_two_line_angle_use_the_reference_measurements() {
    let mut document = SketchDocument::new(2.0).unwrap();
    let a = document.add_point("a", [0.0, 0.0]).unwrap();
    let b = document.add_point("b", [3.0, 0.0]).unwrap();
    let c = document.add_point("c", [0.0, 0.0]).unwrap();
    let d = document.add_point("d", [1.0, 1.0]).unwrap();
    let first = document
        .add_curve(
            "first",
            CurveDefinition::Line {
                start: a,
                end: b,
                branch_direction: [1.0, 0.0],
            },
        )
        .unwrap();
    let second = document
        .add_curve(
            "second",
            CurveDefinition::Line {
                start: c,
                end: d,
                branch_direction: [std::f64::consts::FRAC_1_SQRT_2; 2],
            },
        )
        .unwrap();
    let mut catalog = DocumentMeasurementCatalog::new(&mut document).unwrap();
    let length = catalog
        .add_dimension(
            &mut document,
            "first length",
            DocumentM38DimensionDefinition::SegmentLength {
                line: support(first),
            },
            DocumentDimensionMode::Driving,
            4.0,
        )
        .unwrap();
    let angle = catalog
        .add_dimension(
            &mut document,
            "line angle",
            DocumentM38DimensionDefinition::TwoLineAngle {
                first: support(first),
                second: support(second),
                orientation: DocumentAngleOrientation::CounterClockwise,
                winding: 1,
            },
            DocumentDimensionMode::Driving,
            std::f64::consts::FRAC_PI_3,
        )
        .unwrap();

    let solved = catalog
        .solve_document(
            &document,
            geosolve_sketch::SketchSolveRequest::default().without_previous_state_preferences(),
            geosolve_core::SolverConfig::default(),
        )
        .unwrap();

    for (source, expected) in [
        (length, 4.0),
        (angle, std::f64::consts::FRAC_PI_3 + std::f64::consts::TAU),
    ] {
        let value = catalog
            .evaluate_dimension(&solved.document, source)
            .unwrap();
        assert!((value.value - expected).abs() / solved.document.model_scale() <= 1.0e-9);
        assert!(value.residual.unwrap().abs() / solved.document.model_scale() <= 1.0e-9);
    }
}

#[test]
fn driving_point_line_distance_and_three_point_angle_use_semantic_operands() {
    let mut document = SketchDocument::new(3.0).unwrap();
    let a = document.add_point("a", [0.0, 0.0]).unwrap();
    let b = document.add_point("b", [4.0, 0.0]).unwrap();
    let p = document.add_point("p", [1.0, 1.0]).unwrap();
    let q = document.add_point("q", [2.0, 2.0]).unwrap();
    let line = document
        .add_curve(
            "line",
            CurveDefinition::Line {
                start: a,
                end: b,
                branch_direction: [1.0, 0.0],
            },
        )
        .unwrap();
    let mut catalog = DocumentMeasurementCatalog::new(&mut document).unwrap();
    let distance = catalog
        .add_dimension(
            &mut document,
            "point-line distance",
            DocumentM38DimensionDefinition::PointLineDistance {
                point: point(p),
                line: support(line),
                side: DocumentLineSide::Left,
            },
            DocumentDimensionMode::Driving,
            2.0,
        )
        .unwrap();
    let angle = catalog
        .add_dimension(
            &mut document,
            "three-point angle",
            DocumentM38DimensionDefinition::ThreePointAngle {
                first: point(a),
                vertex: point(p),
                second: point(q),
                orientation: DocumentAngleOrientation::CounterClockwise,
                winding: -1,
            },
            DocumentDimensionMode::Driving,
            std::f64::consts::FRAC_PI_2,
        )
        .unwrap();

    let solved = catalog
        .solve_document(
            &document,
            geosolve_sketch::SketchSolveRequest::default().without_previous_state_preferences(),
            geosolve_core::SolverConfig::default(),
        )
        .unwrap();

    for (source, expected) in [
        (distance, 2.0),
        (angle, std::f64::consts::FRAC_PI_2 - std::f64::consts::TAU),
    ] {
        let value = catalog
            .evaluate_dimension(&solved.document, source)
            .unwrap();
        assert!((value.value - expected).abs() / solved.document.model_scale() <= 1.0e-9);
        assert!(value.residual.unwrap().abs() / solved.document.model_scale() <= 1.0e-9);
    }
}

#[test]
fn ellipse_axes_and_conic_properties_share_typed_persistent_measurements() {
    let mut document = SketchDocument::new(3.0).unwrap();
    let center = document.add_point("center", [2.0, -4.0]).unwrap();
    let axis = document.add_point("major", [5.0, -4.0]).unwrap();
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
                major_axis_point: axis,
                minor_axis_ratio: ratio,
            },
        )
        .unwrap();
    let mut catalog = DocumentMeasurementCatalog::new(&mut document).unwrap();
    let properties = [
        (
            DocumentM38DimensionDefinition::EllipseMajorAxis { curve: ellipse },
            6.0,
        ),
        (
            DocumentM38DimensionDefinition::EllipseMinorAxis { curve: ellipse },
            3.0,
        ),
        (
            DocumentM38DimensionDefinition::ConicLinearEccentricity { curve: ellipse },
            6.75_f64.sqrt(),
        ),
    ];
    for (definition, expected) in properties {
        for mode in [
            DocumentDimensionMode::Driving,
            DocumentDimensionMode::Reference,
        ] {
            let source = catalog
                .add_dimension(
                    &mut document,
                    "ellipse property",
                    definition.clone(),
                    mode,
                    expected,
                )
                .unwrap();
            let value = catalog.evaluate_dimension(&document, source).unwrap();
            assert_eq!(value.unit, geosolve_sketch::DocumentScalarUnit::Length);
            assert!((value.value - expected).abs() < 1.0e-12);
        }
    }
    let measurement = catalog
        .add_measurement(
            &mut document,
            "major axis query",
            DocumentMeasurementDefinition::ConicProperty {
                curve: ellipse,
                property: DocumentConicMeasurement::MajorAxisLength,
            },
            DocumentMeasurementProvenance::RetainedDesign { revision: 0 },
        )
        .unwrap();
    let json = catalog.to_canonical_json().unwrap();
    assert!(json.contains("retained_design") && json.contains("major_axis_length"));
    let session = RetainedSketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        geosolve_core::SolverConfig::default(),
    )
    .unwrap();
    assert!(
        (catalog
            .evaluate_measurement(&session, measurement)
            .unwrap()
            .value
            - 6.0)
            .abs()
            <= f64::EPSILON
    );
}

#[test]
#[allow(clippy::too_many_lines)]
fn driving_arc_conic_and_path_dimensions_publish_solver_owned_audit() {
    let mut document = SketchDocument::new(4.0).unwrap();
    let center = document.add_point("center", [0.0, 0.0]).unwrap();
    let radius = document
        .add_scalar("radius", 2.0, ScalarUnit::Length, ScalarDomain::Positive)
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
                sweep: geosolve_sketch::DocumentArcSweep::CounterClockwise,
            },
        )
        .unwrap();
    let major = document.add_point("major", [3.0, 0.0]).unwrap();
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
                major_axis_point: major,
                minor_axis_ratio: ratio,
            },
        )
        .unwrap();
    let p0 = document.add_point("p0", [0.0, 0.0]).unwrap();
    let p1 = document.add_point("p1", [1.0, 2.0]).unwrap();
    let p2 = document.add_point("p2", [3.0, 1.0]).unwrap();
    let bezier = document
        .add_curve(
            "bezier",
            CurveDefinition::QuadraticBezier {
                controls: [p0, p1, p2],
            },
        )
        .unwrap();
    let path_interval = DocumentBoundedCurveInterval {
        support: DocumentCurveSpanRef {
            span: CurveSpan::line(bezier),
            winding: 0,
        },
        start: 0.0,
        end: 1.0,
    };
    let arc_interval = DocumentBoundedCurveInterval {
        support: DocumentCurveSpanRef {
            span: CurveSpan::line(arc),
            winding: 0,
        },
        start: 0.0,
        end: 1.0,
    };
    let mut catalog = DocumentMeasurementCatalog::new(&mut document).unwrap();
    let dimensions = [
        (
            DocumentM38DimensionDefinition::CircularSweep { arc },
            std::f64::consts::FRAC_PI_3,
        ),
        (
            DocumentM38DimensionDefinition::CircularArcLength { arc },
            4.0,
        ),
        (
            DocumentM38DimensionDefinition::EllipseMajorAxis { curve: ellipse },
            8.0,
        ),
        (
            DocumentM38DimensionDefinition::PathLength {
                interval: path_interval,
            },
            4.0,
        ),
        (
            DocumentM38DimensionDefinition::EqualPathLength {
                first: path_interval,
                second: arc_interval,
            },
            0.0,
        ),
    ];
    let mut source_ids = Vec::new();
    for (definition, target) in dimensions {
        source_ids.push(
            catalog
                .add_dimension(
                    &mut document,
                    "driving integral dimension",
                    definition,
                    DocumentDimensionMode::Driving,
                    target,
                )
                .unwrap(),
        );
    }

    let solved = catalog
        .solve_document(
            &document,
            geosolve_sketch::SketchSolveRequest::default().without_previous_state_preferences(),
            geosolve_core::SolverConfig::default(),
        )
        .unwrap();
    assert!(solved.solve_result.accepted(), "{:#?}", solved.solve_result);
    assert_eq!(solved.audit.len(), source_ids.len());
    for (source, value) in source_ids.into_iter().zip(&solved.audit) {
        let independently_evaluated = catalog
            .evaluate_dimension(&solved.document, source)
            .unwrap();
        assert!((independently_evaluated.value - value.value).abs() <= 1.0e-9);
        assert!(value.residual.unwrap().abs() / solved.document.model_scale() <= 1.0e-9);
        assert!(value.audit.independently_evaluated);
        assert!(!value.audit.rows.is_empty());
        assert!(
            value
                .audit
                .rows
                .iter()
                .all(|row| row.raw_residual.is_finite() && row.normalized_residual.is_finite())
        );
    }
}

#[test]
fn malformed_intervals_and_tampered_catalogs_reject_without_mutation() {
    let mut document = SketchDocument::new(1.0).unwrap();
    let a = document.add_point("a", [0.0, 0.0]).unwrap();
    let b = document.add_point("b", [1.0, 0.0]).unwrap();
    let line = document
        .add_curve(
            "line",
            CurveDefinition::Line {
                start: a,
                end: b,
                branch_direction: [1.0, 0.0],
            },
        )
        .unwrap();
    let mut catalog = DocumentMeasurementCatalog::new(&mut document).unwrap();
    let before = document.to_canonical_json().unwrap();
    assert!(
        catalog
            .add_measurement(
                &mut document,
                "bad",
                DocumentMeasurementDefinition::BoundedCurveLength {
                    interval: DocumentBoundedCurveInterval {
                        support: DocumentCurveSpanRef {
                            span: CurveSpan::line(line),
                            winding: 0
                        },
                        start: 1.0,
                        end: 0.0
                    }
                },
                DocumentMeasurementProvenance::AcceptedDocument { revision: 0 }
            )
            .is_err()
    );
    assert_eq!(document.to_canonical_json().unwrap(), before);
    assert!(
        catalog
            .add_dimension(
                &mut document,
                "self datum",
                DocumentM38DimensionDefinition::DatumCoordinate {
                    point: point(a),
                    datum: DocumentDatumAxis {
                        origin: point(a),
                        axis: DocumentCoordinateAxis::X,
                    },
                },
                DocumentDimensionMode::Reference,
                0.0,
            )
            .is_err()
    );
    assert_eq!(document.to_canonical_json().unwrap(), before);
    catalog
        .add_dimension(
            &mut document,
            "dx",
            DocumentM38DimensionDefinition::RelativeHorizontal {
                first: point(a),
                second: point(b),
            },
            DocumentDimensionMode::Reference,
            1.0,
        )
        .unwrap();
    let encoded = catalog.to_canonical_json().unwrap();
    let tampered = encoded.replacen("\"version\":1", "\"version\":99", 1);
    assert!(DocumentMeasurementCatalog::from_json(&mut document, &tampered).is_err());

    let mut restored = SketchDocument::from_json(&document.to_canonical_json().unwrap()).unwrap();
    let restored_before = restored.to_canonical_json().unwrap();
    let invalid_label = encoded.replacen("\"label\":\"dx\"", "\"label\":\"\"", 1);
    assert!(DocumentMeasurementCatalog::from_json(&mut restored, &invalid_label).is_err());
    assert_eq!(restored.to_canonical_json().unwrap(), restored_before);
}
