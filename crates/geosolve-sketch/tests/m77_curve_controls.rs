// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_core::SolverConfig;
use geosolve_sketch::{
    CurveDefinition, CurveId, DocumentArcSweep, DocumentBSplineForm, DocumentCommand,
    DocumentCommandEffect, DocumentConstraintDefinition, DocumentCurveControlAvailability,
    DocumentCurveControlError, DocumentCurveControlId, DocumentCurveControlKind,
    DocumentCurveControlProjection, DocumentCurveControlTarget,
    DocumentCurveControlWithholdingReason, DocumentDimensionDefinition, DocumentDimensionMode,
    DocumentEdit, DocumentElementId, DocumentError, DocumentHyperbolaBranch,
    DocumentRationalConicControl, DocumentRationalConicControlMode, DocumentSolveRequest,
    DocumentTrimProjectionError, FeatureEndpoint, MIN_RATIONAL_QUADRATIC_MIDDLE_WEIGHT,
    OperationControl, OperationOutcome, PreparedSketchOperation, RetainedSketchDocumentSession,
    ScalarDomain, ScalarUnit, SketchDocument, SketchDocumentSession,
};

#[derive(Clone, Copy)]
struct Gallery {
    circle: CurveId,
    circle_radius: geosolve_sketch::DesignScalarId,
    arc: CurveId,
    ellipse: CurveId,
    ellipse_ratio: geosolve_sketch::DesignScalarId,
    rational: CurveId,
    rational_weight: geosolve_sketch::DesignScalarId,
    parabola: CurveId,
    hyperbola: CurveId,
    hyperbola_conjugate: geosolve_sketch::DesignScalarId,
}

fn ratio_domain() -> ScalarDomain {
    ScalarDomain::Bounded {
        lower: f64::from_bits(1),
        upper: 1.0,
    }
}

fn weight_domain() -> ScalarDomain {
    ScalarDomain::Bounded {
        lower: MIN_RATIONAL_QUADRATIC_MIDDLE_WEIGHT,
        upper: f64::MAX,
    }
}

#[allow(clippy::too_many_lines)]
fn gallery() -> (SketchDocument, Gallery) {
    let mut document = SketchDocument::new(10.0).unwrap();

    let circle_center = document.add_point("circle center", [1.0, 2.0]).unwrap();
    let circle_radius = document
        .add_scalar(
            "circle radius",
            2.0,
            ScalarUnit::Length,
            ScalarDomain::Positive,
        )
        .unwrap();
    let circle = document
        .add_curve(
            "circle",
            CurveDefinition::Circle {
                center: circle_center,
                radius: circle_radius,
            },
        )
        .unwrap();

    let arc_center = document.add_point("arc center", [10.0, 1.0]).unwrap();
    let arc_radius = document
        .add_scalar(
            "arc radius",
            3.0,
            ScalarUnit::Length,
            ScalarDomain::Positive,
        )
        .unwrap();
    let arc_start = document
        .add_scalar("arc start", 0.2, ScalarUnit::Angle, ScalarDomain::Finite)
        .unwrap();
    let arc_end = document
        .add_scalar("arc end", 1.4, ScalarUnit::Angle, ScalarDomain::Finite)
        .unwrap();
    let arc = document
        .add_curve(
            "arc",
            CurveDefinition::CircularArc {
                center: arc_center,
                radius: arc_radius,
                start_angle: arc_start,
                end_angle: arc_end,
                sweep: DocumentArcSweep::CounterClockwise,
            },
        )
        .unwrap();

    let ellipse_center = document.add_point("ellipse center", [20.0, 0.0]).unwrap();
    let ellipse_axis = document.add_point("ellipse axis", [22.4, 1.8]).unwrap();
    let ellipse_ratio = document
        .add_scalar("ellipse ratio", 0.5, ScalarUnit::Parameter, ratio_domain())
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

    let rational_start = document.add_point("rational start", [30.0, 0.0]).unwrap();
    let rational_end = document.add_point("rational end", [34.0, 0.0]).unwrap();
    let rational_weight = document
        .add_scalar(
            "rational weight",
            0.5,
            ScalarUnit::Parameter,
            weight_domain(),
        )
        .unwrap();
    let rational = document
        .add_curve(
            "rational",
            CurveDefinition::RationalQuadraticConic {
                start: rational_start,
                weighted_middle: [16.0, 1.0],
                middle_weight: rational_weight,
                end: rational_end,
            },
        )
        .unwrap();

    let vertex = document.add_point("vertex", [40.0, 0.0]).unwrap();
    let focus = document.add_point("focus", [41.0, 0.0]).unwrap();
    let parabola_start = document
        .add_scalar(
            "parabola start",
            -0.8,
            ScalarUnit::Parameter,
            ScalarDomain::Finite,
        )
        .unwrap();
    let parabola_end = document
        .add_scalar(
            "parabola end",
            1.2,
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

    let hyperbola_center = document
        .add_point("hyperbola center", [50.0, -1.0])
        .unwrap();
    let hyperbola_axis = document.add_point("hyperbola axis", [52.0, 0.5]).unwrap();
    let hyperbola_conjugate = document
        .add_scalar(
            "hyperbola conjugate",
            2.0,
            ScalarUnit::Length,
            ScalarDomain::Positive,
        )
        .unwrap();
    let hyperbola_start = document
        .add_scalar(
            "hyperbola start",
            -0.7,
            ScalarUnit::Parameter,
            ScalarDomain::Finite,
        )
        .unwrap();
    let hyperbola_end = document
        .add_scalar(
            "hyperbola end",
            0.9,
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
                semi_conjugate: hyperbola_conjugate,
                branch: DocumentHyperbolaBranch::Negative,
                trim_start: hyperbola_start,
                trim_end: hyperbola_end,
            },
        )
        .unwrap();

    document.validate().unwrap();
    (
        document,
        Gallery {
            circle,
            circle_radius,
            arc,
            ellipse,
            ellipse_ratio,
            rational,
            rational_weight,
            parabola,
            hyperbola,
            hyperbola_conjugate,
        },
    )
}

fn control(
    document: &SketchDocument,
    curve: CurveId,
    kind: DocumentCurveControlKind,
) -> geosolve_sketch::DocumentCurveControl {
    document
        .curve_controls(curve)
        .unwrap()
        .into_iter()
        .find(|control| control.id.kind == kind)
        .unwrap()
}

fn pair_bits(value: [f64; 2]) -> [u64; 2] {
    value.map(f64::to_bits)
}

fn add_control_points(
    document: &mut SketchDocument,
    positions: &[[f64; 2]],
) -> Vec<geosolve_sketch::DesignPointId> {
    positions
        .iter()
        .map(|position| document.add_point("control", *position).unwrap())
        .collect()
}

#[test]
fn catalog_exposes_persistent_aliases_and_derived_controls_without_schema_state() {
    let (document, ids) = gallery();
    let before = document.to_canonical_json().unwrap();

    let circle = document.curve_controls(ids.circle).unwrap();
    assert_eq!(
        circle
            .iter()
            .map(|control| control.id.kind)
            .collect::<Vec<_>>(),
        vec![
            DocumentCurveControlKind::Center,
            DocumentCurveControlKind::Radius,
        ]
    );
    assert_eq!(pair_bits(circle[1].position), pair_bits([3.0, 2.0]));
    assert_eq!(
        circle[1].target,
        DocumentCurveControlTarget::Scalar(ids.circle_radius)
    );

    for (curve, required) in [
        (
            ids.arc,
            vec![
                DocumentCurveControlKind::Center,
                DocumentCurveControlKind::Radius,
                DocumentCurveControlKind::TrimStart,
                DocumentCurveControlKind::TrimEnd,
            ],
        ),
        (
            ids.ellipse,
            vec![
                DocumentCurveControlKind::Center,
                DocumentCurveControlKind::MajorAxisPoint,
                DocumentCurveControlKind::MinorAxis,
            ],
        ),
        (
            ids.parabola,
            vec![
                DocumentCurveControlKind::Vertex,
                DocumentCurveControlKind::Focus,
                DocumentCurveControlKind::TrimStart,
                DocumentCurveControlKind::TrimEnd,
            ],
        ),
        (
            ids.hyperbola,
            vec![
                DocumentCurveControlKind::Center,
                DocumentCurveControlKind::TransverseAxisPoint,
                DocumentCurveControlKind::ConjugateAxis,
                DocumentCurveControlKind::TrimStart,
                DocumentCurveControlKind::TrimEnd,
            ],
        ),
    ] {
        let actual = document
            .curve_controls(curve)
            .unwrap()
            .into_iter()
            .map(|control| control.id.kind)
            .collect::<Vec<_>>();
        assert_eq!(actual, required);
    }

    let rational = document.curve_controls(ids.rational).unwrap();
    assert_eq!(
        rational[1].id.kind,
        DocumentCurveControlKind::RationalMiddle
    );
    assert_eq!(pair_bits(rational[1].position), pair_bits([32.0, 2.0]));
    assert_eq!(
        rational[1].target,
        DocumentCurveControlTarget::RationalMiddle {
            weight: ids.rational_weight,
            mode: DocumentRationalConicControlMode::Euclidean,
        }
    );
    assert_eq!(document.to_canonical_json().unwrap(), before);
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one catalog matrix keeps every remaining M77 point-control family explicit"
)]
fn point_defined_and_elliptical_arc_catalog_is_complete_and_finite() {
    let mut document = SketchDocument::new(10.0).unwrap();

    let line_points = add_control_points(&mut document, &[[0.0, 0.0], [2.0, 0.5]]);
    let line = document
        .add_curve(
            "line",
            CurveDefinition::Line {
                start: line_points[0],
                end: line_points[1],
                branch_direction: [2.0 / 2.0_f64.hypot(0.5), 0.5 / 2.0_f64.hypot(0.5)],
            },
        )
        .unwrap();

    let polyline_points = add_control_points(&mut document, &[[3.0, 0.0], [4.0, 1.0], [5.0, 0.0]]);
    let polyline = document
        .add_curve(
            "polyline",
            CurveDefinition::Polyline {
                points: polyline_points,
                closed: false,
                branch_directions: vec![
                    [std::f64::consts::FRAC_1_SQRT_2; 2],
                    [
                        std::f64::consts::FRAC_1_SQRT_2,
                        -std::f64::consts::FRAC_1_SQRT_2,
                    ],
                ],
            },
        )
        .unwrap();

    let quadratic_points = add_control_points(&mut document, &[[6.0, 0.0], [7.0, 2.0], [8.0, 0.0]]);
    let quadratic = document
        .add_curve(
            "quadratic",
            CurveDefinition::QuadraticBezier {
                controls: quadratic_points.try_into().unwrap(),
            },
        )
        .unwrap();
    let cubic_points = add_control_points(
        &mut document,
        &[[9.0, 0.0], [10.0, 2.0], [11.0, 2.0], [12.0, 0.0]],
    );
    let cubic = document
        .add_curve(
            "cubic",
            CurveDefinition::CubicBezier {
                controls: cubic_points.try_into().unwrap(),
            },
        )
        .unwrap();

    let ellipse_points = add_control_points(&mut document, &[[14.0, 0.0], [17.0, 0.0]]);
    let ratio = document
        .add_scalar("ratio", 0.5, ScalarUnit::Parameter, ratio_domain())
        .unwrap();
    let ellipse_start = document
        .add_scalar("start", 0.2, ScalarUnit::Angle, ScalarDomain::Finite)
        .unwrap();
    let ellipse_end = document
        .add_scalar("end", 1.4, ScalarUnit::Angle, ScalarDomain::Finite)
        .unwrap();
    let elliptical_arc = document
        .add_curve(
            "elliptical arc",
            CurveDefinition::EllipticalArc {
                center: ellipse_points[0],
                major_axis_point: ellipse_points[1],
                minor_axis_ratio: ratio,
                start_angle: ellipse_start,
                end_angle: ellipse_end,
                sweep: DocumentArcSweep::CounterClockwise,
            },
        )
        .unwrap();

    let spline_points = add_control_points(
        &mut document,
        &[[18.0, 0.0], [19.0, 2.0], [21.0, 2.0], [22.0, 0.0]],
    );
    let knots = vec![0.0, 0.0, 0.0, 0.5, 1.0, 1.0, 1.0];
    let bspline = document
        .add_curve(
            "B-spline",
            CurveDefinition::BSpline {
                form: DocumentBSplineForm::Clamped,
                degree: 2,
                controls: spline_points.clone(),
                knots: knots.clone(),
                span_ids: vec![7, 11],
                next_span_id: 12,
            },
        )
        .unwrap();
    let weights = [1.0, 0.8, 1.2, 1.0]
        .map(|value| {
            document
                .add_scalar(
                    "weight",
                    value,
                    ScalarUnit::Parameter,
                    ScalarDomain::Positive,
                )
                .unwrap()
        })
        .to_vec();
    let nurbs = document
        .add_curve(
            "NURBS",
            CurveDefinition::Nurbs {
                form: DocumentBSplineForm::Clamped,
                degree: 2,
                controls: spline_points,
                gauge_weight: weights[0],
                weights,
                knots,
                span_ids: vec![19, 23],
                next_span_id: 24,
            },
        )
        .unwrap();
    document.validate().unwrap();

    let expected = [
        (
            line,
            vec![
                DocumentCurveControlKind::StartPoint,
                DocumentCurveControlKind::EndPoint,
            ],
        ),
        (
            polyline,
            vec![
                DocumentCurveControlKind::StartPoint,
                DocumentCurveControlKind::ControlPoint { ordinal: 1 },
                DocumentCurveControlKind::EndPoint,
            ],
        ),
        (
            quadratic,
            (0..3)
                .map(|ordinal| DocumentCurveControlKind::ControlPoint { ordinal })
                .collect(),
        ),
        (
            cubic,
            (0..4)
                .map(|ordinal| DocumentCurveControlKind::ControlPoint { ordinal })
                .collect(),
        ),
        (
            elliptical_arc,
            vec![
                DocumentCurveControlKind::Center,
                DocumentCurveControlKind::MajorAxisPoint,
                DocumentCurveControlKind::MinorAxis,
                DocumentCurveControlKind::TrimStart,
                DocumentCurveControlKind::TrimEnd,
            ],
        ),
        (
            bspline,
            (0..4)
                .map(|ordinal| DocumentCurveControlKind::ControlPoint { ordinal })
                .collect(),
        ),
        (
            nurbs,
            (0..4)
                .map(|ordinal| DocumentCurveControlKind::ControlPoint { ordinal })
                .collect(),
        ),
    ];
    for (curve, expected_kinds) in expected {
        let controls = document.curve_controls(curve).unwrap();
        assert_eq!(
            controls
                .iter()
                .map(|control| control.id.kind)
                .collect::<Vec<_>>(),
            expected_kinds,
            "curve {curve}"
        );
        assert!(controls.iter().all(|control| {
            control.position.into_iter().all(f64::is_finite)
                && matches!(
                    control.availability,
                    geosolve_sketch::DocumentCurveControlAvailability::Editable
                )
        }));
    }
    let ellipse_controls = document.curve_controls(elliptical_arc).unwrap();
    assert_eq!(
        ellipse_controls[2].target,
        DocumentCurveControlTarget::Scalar(ratio)
    );
    assert_eq!(
        ellipse_controls[3].target,
        DocumentCurveControlTarget::Scalar(ellipse_start)
    );
    assert_eq!(
        ellipse_controls[4].target,
        DocumentCurveControlTarget::Scalar(ellipse_end)
    );
}

#[test]
fn radius_minor_and_conjugate_grips_inverse_project_in_rotated_frames() {
    let (document, ids) = gallery();

    let radius = DocumentCurveControlId {
        curve: ids.circle,
        kind: DocumentCurveControlKind::Radius,
    };
    assert_eq!(
        document.project_curve_control(radius, [1.0, 6.0]).unwrap(),
        DocumentCurveControlProjection::Scalar {
            scalar: ids.circle_radius,
            value: 4.0,
        }
    );

    let minor = control(&document, ids.ellipse, DocumentCurveControlKind::MinorAxis);
    let center = control(&document, ids.ellipse, DocumentCurveControlKind::Center).position;
    let target = [
        center[0] + 1.5 * (minor.position[0] - center[0]),
        center[1] + 1.5 * (minor.position[1] - center[1]),
    ];
    let DocumentCurveControlProjection::Scalar { scalar, value } =
        document.project_curve_control(minor.id, target).unwrap()
    else {
        panic!("minor scalar projection expected")
    };
    assert_eq!(scalar, ids.ellipse_ratio);
    assert!((value - 0.75).abs() <= 1.0e-12, "ratio={value}");

    let conjugate = control(
        &document,
        ids.hyperbola,
        DocumentCurveControlKind::ConjugateAxis,
    );
    let center = control(&document, ids.hyperbola, DocumentCurveControlKind::Center).position;
    let target = [
        center[0] - 1.5 * (conjugate.position[0] - center[0]),
        center[1] - 1.5 * (conjugate.position[1] - center[1]),
    ];
    let DocumentCurveControlProjection::Scalar { scalar, value } = document
        .project_curve_control(conjugate.id, target)
        .unwrap()
    else {
        panic!("conjugate scalar projection expected")
    };
    assert_eq!(scalar, ids.hyperbola_conjugate);
    assert!((value - 3.0).abs() <= 1.0e-12, "semi-conjugate={value}");
}

#[test]
fn trim_controls_round_trip_start_and_end_without_changing_sweep_or_hyperbola_branch() {
    let (document, ids) = gallery();
    let arc_definition = document.curve(ids.arc).unwrap().definition.clone();
    let hyperbola_definition = document.curve(ids.hyperbola).unwrap().definition.clone();
    for curve in [ids.arc, ids.parabola, ids.hyperbola] {
        for kind in [
            DocumentCurveControlKind::TrimStart,
            DocumentCurveControlKind::TrimEnd,
        ] {
            let control = control(&document, curve, kind);
            let DocumentCurveControlTarget::Scalar(expected_scalar) = control.target else {
                panic!("{kind:?} on {curve} did not retain its scalar identity")
            };
            let DocumentCurveControlProjection::Scalar { scalar, value } = document
                .project_curve_control(control.id, control.position)
                .unwrap()
            else {
                panic!("{kind:?} on {curve} did not inverse-project to its scalar")
            };
            assert_eq!(scalar, expected_scalar);
            let current = document.scalar(scalar).unwrap().value;
            assert!(
                (value - current).abs() <= 1.0e-10,
                "{kind:?} on {curve}: projected {value}, current {current}"
            );
        }
    }
    assert_eq!(document.curve(ids.arc).unwrap().definition, arc_definition);
    assert_eq!(
        document.curve(ids.hyperbola).unwrap().definition,
        hyperbola_definition
    );
}

fn trim_scalar_ids(
    document: &SketchDocument,
    curve: CurveId,
) -> (
    geosolve_sketch::DesignScalarId,
    geosolve_sketch::DesignScalarId,
) {
    match document.curve(curve).unwrap().definition {
        CurveDefinition::ParabolaSegment {
            trim_start,
            trim_end,
            ..
        }
        | CurveDefinition::HyperbolaSegment {
            trim_start,
            trim_end,
            ..
        } => (trim_start, trim_end),
        _ => panic!("expected a non-periodic trimmed conic"),
    }
}

fn trim_target_at(
    document: &SketchDocument,
    curve: CurveId,
    kind: DocumentCurveControlKind,
    value: f64,
) -> [f64; 2] {
    let mut candidate = document.clone();
    let (start, end) = trim_scalar_ids(&candidate, curve);
    let scalar = match kind {
        DocumentCurveControlKind::TrimStart => start,
        DocumentCurveControlKind::TrimEnd => end,
        _ => panic!("expected a trim control"),
    };
    candidate.set_scalar_value(scalar, value).unwrap();
    control(&candidate, curve, kind).position
}

#[test]
fn nonperiodic_trim_controls_reject_crossing_without_reversing_orientation() {
    let (document, ids) = gallery();

    for curve in [ids.parabola, ids.hyperbola] {
        let before = document.to_canonical_json().unwrap();
        for (kind, endpoint, crossing_value) in [
            (
                DocumentCurveControlKind::TrimStart,
                FeatureEndpoint::Start,
                2.0,
            ),
            (
                DocumentCurveControlKind::TrimEnd,
                FeatureEndpoint::End,
                -2.0,
            ),
        ] {
            let target = trim_target_at(&document, curve, kind, crossing_value);
            assert!(
                matches!(
                    document.project_curve_control(
                        DocumentCurveControlId { curve, kind },
                        target,
                    ),
                    Err(DocumentCurveControlError::TrimProjection(
                        DocumentTrimProjectionError::CrossesOppositeEndpoint {
                            curve: rejected,
                            endpoint: rejected_endpoint,
                        }
                    )) if rejected == curve && rejected_endpoint == endpoint
                ),
                "ascending {curve:?} {kind:?} accepted a crossing target"
            );
        }
        assert_eq!(document.to_canonical_json().unwrap(), before);

        let mut descending = document.clone();
        let (start, end) = trim_scalar_ids(&descending, curve);
        descending.set_scalar_value(start, 2.0).unwrap();
        descending.set_scalar_value(end, -2.0).unwrap();
        let before = descending.to_canonical_json().unwrap();
        for (kind, endpoint, crossing_value) in [
            (
                DocumentCurveControlKind::TrimStart,
                FeatureEndpoint::Start,
                -3.0,
            ),
            (DocumentCurveControlKind::TrimEnd, FeatureEndpoint::End, 3.0),
        ] {
            let target = trim_target_at(&descending, curve, kind, crossing_value);
            assert!(
                matches!(
                    descending.project_curve_control(
                        DocumentCurveControlId { curve, kind },
                        target,
                    ),
                    Err(DocumentCurveControlError::TrimProjection(
                        DocumentTrimProjectionError::CrossesOppositeEndpoint {
                            curve: rejected,
                            endpoint: rejected_endpoint,
                        }
                    )) if rejected == curve && rejected_endpoint == endpoint
                ),
                "descending {curve:?} {kind:?} accepted a crossing target"
            );
        }
        assert_eq!(descending.to_canonical_json().unwrap(), before);
    }
}

#[test]
fn radial_handle_reports_driving_dimension_and_equal_radius_ownership() {
    let (mut dimensioned, ids) = gallery();
    let target = dimensioned
        .add_scalar(
            "display radius",
            2.0,
            ScalarUnit::Length,
            ScalarDomain::Positive,
        )
        .unwrap();
    dimensioned
        .add_dimension(
            "reference radius",
            DocumentDimensionDefinition::Radius {
                curve: ids.circle,
                target,
            },
            DocumentDimensionMode::Reference,
        )
        .unwrap();
    assert_eq!(
        control(&dimensioned, ids.circle, DocumentCurveControlKind::Radius).availability,
        DocumentCurveControlAvailability::Editable,
    );
    let driving_target = dimensioned
        .add_scalar(
            "driving radius target",
            2.0,
            ScalarUnit::Length,
            ScalarDomain::Positive,
        )
        .unwrap();
    let driving = dimensioned
        .add_dimension(
            "driving radius",
            DocumentDimensionDefinition::Radius {
                curve: ids.circle,
                target: driving_target,
            },
            DocumentDimensionMode::Driving,
        )
        .unwrap();
    assert_eq!(
        control(&dimensioned, ids.circle, DocumentCurveControlKind::Radius).availability,
        DocumentCurveControlAvailability::ReadOnly(
            DocumentCurveControlWithholdingReason::DrivingDimensionOwned,
        ),
    );
    dimensioned
        .set_element_user_suppressed(DocumentElementId::Dimension(driving), true)
        .unwrap();
    assert_eq!(
        control(&dimensioned, ids.circle, DocumentCurveControlKind::Radius).availability,
        DocumentCurveControlAvailability::Editable,
    );

    let (mut related, ids) = gallery();
    let second_center = related.add_point("peer center", [5.0, 2.0]).unwrap();
    let second_radius = related
        .add_scalar(
            "peer radius",
            2.0,
            ScalarUnit::Length,
            ScalarDomain::Positive,
        )
        .unwrap();
    let second = related
        .add_curve(
            "peer circle",
            CurveDefinition::Circle {
                center: second_center,
                radius: second_radius,
            },
        )
        .unwrap();
    related
        .add_constraint(
            "equal radii",
            DocumentConstraintDefinition::EqualRadius {
                first: ids.circle,
                second,
            },
        )
        .unwrap();
    for curve in [ids.circle, second] {
        assert_eq!(
            control(&related, curve, DocumentCurveControlKind::Radius).availability,
            DocumentCurveControlAvailability::ReadOnly(
                DocumentCurveControlWithholdingReason::EqualRadiusOwned,
            ),
        );
    }
}

#[test]
fn rational_control_modes_are_explicit_atomic_and_round_trip_existing_storage() {
    let (mut document, ids) = gallery();
    assert_eq!(
        document.rational_conic_control(ids.rational).unwrap(),
        DocumentRationalConicControl::Euclidean {
            middle: [32.0, 2.0],
            weight: 0.5,
        }
    );

    document
        .set_rational_conic_control(
            ids.rational,
            DocumentRationalConicControl::Euclidean {
                middle: [31.5, 2.5],
                weight: -0.5,
            },
        )
        .unwrap();
    assert_eq!(
        document.rational_conic_control(ids.rational).unwrap(),
        DocumentRationalConicControl::Euclidean {
            middle: [31.5, 2.5],
            weight: -0.5,
        }
    );
    let CurveDefinition::RationalQuadraticConic {
        weighted_middle, ..
    } = document.curve(ids.rational).unwrap().definition
    else {
        panic!("rational curve expected")
    };
    assert_eq!(pair_bits(weighted_middle), pair_bits([-15.75, -1.25]));

    document
        .set_rational_conic_control(
            ids.rational,
            DocumentRationalConicControl::Projective {
                weighted_middle,
                weight: 0.0,
            },
        )
        .unwrap();
    assert_eq!(
        document.rational_conic_control(ids.rational).unwrap(),
        DocumentRationalConicControl::Projective {
            weighted_middle,
            weight: 0.0,
        }
    );
    let projective = control(
        &document,
        ids.rational,
        DocumentCurveControlKind::RationalMiddle,
    );
    assert_eq!(
        projective.target,
        DocumentCurveControlTarget::RationalMiddle {
            weight: ids.rational_weight,
            mode: DocumentRationalConicControlMode::Projective,
        }
    );
    assert_eq!(pair_bits(projective.position), pair_bits([14.25, -1.25]));
    assert_eq!(
        document
            .project_curve_control(projective.id, [32.0, 3.0])
            .unwrap(),
        DocumentCurveControlProjection::RationalMiddle {
            curve: ids.rational,
            control: DocumentRationalConicControl::Projective {
                weighted_middle: [2.0, 3.0],
                weight: 0.0,
            },
        }
    );

    let before = document.to_canonical_json().unwrap();
    assert!(
        document
            .set_rational_conic_control(
                ids.rational,
                DocumentRationalConicControl::Euclidean {
                    middle: [31.0, 2.0],
                    weight: 0.0,
                },
            )
            .is_err()
    );
    assert!(
        document
            .set_rational_conic_control(
                ids.rational,
                DocumentRationalConicControl::Projective {
                    weighted_middle: [2.0, 3.0],
                    weight: 0.25,
                },
            )
            .is_err()
    );
    assert_eq!(document.to_canonical_json().unwrap(), before);
}

#[test]
fn euclidean_rational_control_rejects_lossy_homogeneous_underflow_atomically() {
    let mut document = SketchDocument::new(2.0).unwrap();
    let start = document.add_point("start", [1.0, 0.0]).unwrap();
    let end = document.add_point("end", [0.0, 1.0]).unwrap();
    let weight = document
        .add_scalar("weight", 1.0e-200, ScalarUnit::Parameter, weight_domain())
        .unwrap();
    let curve = document
        .add_curve(
            "rational",
            CurveDefinition::RationalQuadraticConic {
                start,
                weighted_middle: [0.0, 1.0],
                middle_weight: weight,
                end,
            },
        )
        .unwrap();
    let before = document.to_canonical_json().unwrap();

    let control = DocumentCurveControlId {
        curve,
        kind: DocumentCurveControlKind::RationalMiddle,
    };
    assert!(
        matches!(
            document.project_curve_control(control, [1.0e-200, 1.0e200]),
            Err(DocumentCurveControlError::Document(DocumentError::InvalidField {
                field: "rational homogeneous middle",
                ref message,
            })) if message.contains("loses material precision")
        ),
        "inverse projection must reject a control that cannot survive homogeneous storage"
    );

    let result = document.set_rational_conic_control(
        curve,
        DocumentRationalConicControl::Euclidean {
            middle: [1.0e-200, 1.0e200],
            weight: 1.0e-200,
        },
    );
    assert!(
        matches!(
            result,
            Err(DocumentError::InvalidField {
                field: "rational homogeneous middle",
                ref message,
            }) if message.contains("loses material precision")
        ),
        "a nonzero Euclidean control must not collapse to a different homogeneous point"
    );
    assert_eq!(document.to_canonical_json().unwrap(), before);
}

fn completed_patch(
    outcome: OperationOutcome<geosolve_sketch::PreparedSketchPatch>,
) -> geosolve_sketch::PreparedSketchPatch {
    match outcome {
        OperationOutcome::Completed { value, .. } => value,
        OperationOutcome::Cancelled { .. } => panic!("prepared operation cancelled"),
        OperationOutcome::WorkExhausted { .. } => panic!("prepared operation exhausted"),
        _ => panic!("unknown prepared outcome"),
    }
}

#[test]
fn prepared_preview_is_read_only_exact_and_commits_the_visible_rational_candidate() {
    let (document, ids) = gallery();
    let mut session = RetainedSketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let before_input = session.prepared_input();
    let before_json = session
        .accepted_state_for_current_input()
        .unwrap()
        .document()
        .to_canonical_json()
        .unwrap();
    let patch = completed_patch(
        session
            .prepared_snapshot()
            .prepare(PreparedSketchOperation::Apply(
                DocumentEdit::SetRationalConicControl {
                    curve: ids.rational,
                    control: DocumentRationalConicControl::Euclidean {
                        middle: [31.25, 2.75],
                        weight: 0.8,
                    },
                },
            ))
            .execute(OperationControl::unlimited())
            .unwrap(),
    );

    let preview = patch.preview();
    assert_eq!(preview.base_input(), before_input);
    assert_eq!(
        preview.candidate_input().design_identity(),
        preview.proposed_commit().design_identity()
    );
    assert_eq!(
        preview.candidate_input().latest_attempt_identity(),
        preview.proposed_commit().attempt_identity()
    );
    assert_eq!(
        preview.candidate_input().accepted_state_identity(),
        preview.proposed_commit().accepted_state_identity()
    );
    assert_eq!(
        preview
            .accepted_document()
            .unwrap()
            .rational_conic_control(ids.rational)
            .unwrap(),
        DocumentRationalConicControl::Euclidean {
            middle: [31.25, 2.75],
            weight: 0.8,
        }
    );
    assert_eq!(
        preview
            .accepted_session()
            .unwrap()
            .accepted_state_for_current_input()
            .unwrap()
            .document(),
        preview.accepted_document().unwrap()
    );
    let preview_json = preview
        .accepted_document()
        .unwrap()
        .to_canonical_json()
        .unwrap();
    assert_eq!(session.prepared_input(), before_input);
    assert_eq!(
        session
            .accepted_state_for_current_input()
            .unwrap()
            .document()
            .to_canonical_json()
            .unwrap(),
        before_json
    );

    let proposed = patch.proposed_commit();
    assert_eq!(session.commit_prepared_patch(patch).unwrap(), proposed);
    assert_eq!(
        session
            .accepted_state_for_current_input()
            .unwrap()
            .document()
            .to_canonical_json()
            .unwrap(),
        preview_json
    );
}

#[test]
fn accepted_session_rational_edit_is_one_undoable_transaction() {
    let (document, ids) = gallery();
    let mut session = SketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let before = session.export_json().unwrap();
    let before_history = session.history_len();
    let outcome = session
        .apply(DocumentCommand::new(
            session.revision(),
            DocumentEdit::SetRationalConicControl {
                curve: ids.rational,
                control: DocumentRationalConicControl::Euclidean {
                    middle: [31.5, 2.25],
                    weight: 0.75,
                },
            },
        ))
        .unwrap();
    assert!(outcome.accepted());
    assert_eq!(
        outcome.effect,
        Some(DocumentCommandEffect::UpdatedRationalConicControl(
            ids.rational
        ))
    );
    assert_eq!(session.history_len(), before_history + 1);
    session.undo(session.revision()).unwrap();
    assert_eq!(session.export_json().unwrap(), before);
}

#[test]
fn m77_f013_elliptical_arc_minor_axis_chooses_the_clearer_signed_rail() {
    for (label, start_angle, end_angle, expected_minor) in [
        (
            "positive trim",
            0.0,
            std::f64::consts::FRAC_PI_2,
            [0.0, -2.0],
        ),
        (
            "negative trim",
            -std::f64::consts::FRAC_PI_2,
            0.0,
            [0.0, 2.0],
        ),
    ] {
        let mut document = SketchDocument::new(10.0).expect("document");
        let center = document.add_point("center", [0.0, 0.0]).unwrap();
        let major_axis = document.add_point("major axis", [4.0, 0.0]).unwrap();
        let ratio = document
            .add_scalar("minor ratio", 0.5, ScalarUnit::Parameter, ratio_domain())
            .unwrap();
        let start = document
            .add_scalar(
                "start",
                start_angle,
                ScalarUnit::Angle,
                ScalarDomain::Finite,
            )
            .unwrap();
        let end = document
            .add_scalar("end", end_angle, ScalarUnit::Angle, ScalarDomain::Finite)
            .unwrap();
        let arc = document
            .add_curve(
                label,
                CurveDefinition::EllipticalArc {
                    center,
                    major_axis_point: major_axis,
                    minor_axis_ratio: ratio,
                    start_angle: start,
                    end_angle: end,
                    sweep: DocumentArcSweep::CounterClockwise,
                },
            )
            .unwrap();

        let controls = document.curve_controls(arc).expect("curve controls");
        let position = |kind| {
            controls
                .iter()
                .find(|control| control.id.kind == kind)
                .unwrap_or_else(|| panic!("{label}: missing {kind:?}"))
                .position
        };
        let minor = position(DocumentCurveControlKind::MinorAxis);
        let trim_start = position(DocumentCurveControlKind::TrimStart);
        let trim_end = position(DocumentCurveControlKind::TrimEnd);

        assert_eq!(pair_bits(minor), pair_bits(expected_minor), "{label}");
        assert_ne!(pair_bits(minor), pair_bits(trim_start), "{label}");
        assert_ne!(pair_bits(minor), pair_bits(trim_end), "{label}");
    }
}
