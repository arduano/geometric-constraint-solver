// SPDX-License-Identifier: GPL-3.0-or-later
#![allow(clippy::too_many_lines)]

use geosolve_core::{HardValidity, SolveTermination, SolverConfig};
use geosolve_geometry::Point2;
use geosolve_sketch::{
    ArcAngleEndpoint, ArcAngleRole, ArcSweep, ContactDefinition, ContactDomain,
    ContactNeighborhood, ContactState, CurveContactNeighborhood, CurveDefinition,
    CurveMeasurementKind, CurveNormalSide, CurveSpan, DimensionMode, DocumentArcSweep,
    DocumentCommand, DocumentCommandEffect, DocumentConstraintDefinition, DocumentCurveNormalSide,
    DocumentDimensionMode, DocumentEdit, DocumentFilletEndpointOrder, DocumentObjectId,
    DocumentSolveRequest, FilletEndpointOrder, LatentVariableRole, LineLineFilletIds,
    LineLineFilletRequest, LineParameterDomain, RuntimeCurve, ScalarDomain, ScalarUnit, Sketch,
    SketchBound, SketchCurve, SketchCurveContact, SketchDocument, SketchDocumentSession,
    SketchSession, SketchSolveRequest, SolveRejection,
};

fn assert_fillet_runtime_is_bitwise_certified(
    session: &SketchDocumentSession,
    persistent_arcs: &[geosolve_sketch::CurveId],
) {
    let runtime = session.runtime();
    let compiled = runtime.sketch().compile(runtime.request()).unwrap();
    let packed = compiled.problem().packed_state().unwrap();
    let accepted = &runtime
        .accepted_result()
        .unstable_core_report()
        .accepted_state;
    assert_eq!(packed.layout(), accepted.layout());
    assert_eq!(
        packed
            .ambient()
            .iter()
            .map(|value| value.to_bits())
            .collect::<Vec<_>>(),
        accepted
            .ambient()
            .iter()
            .map(|value| value.to_bits())
            .collect::<Vec<_>>()
    );

    let geometry = runtime.sketch().geometry();
    for persistent_arc in persistent_arcs {
        let Some(RuntimeCurve::CircularArc(arc)) =
            session.mappings().runtime_curve(*persistent_arc)
        else {
            panic!("persistent Fillet output must lower to one circular arc")
        };
        let solved = geometry.arc(*arc).unwrap();
        for (role, expected) in [
            (ArcAngleRole::Start, solved.start_angle),
            (ArcAngleRole::End, solved.end_angle),
        ] {
            let variable = compiled.variable_for_arc_angle(*arc, role).unwrap();
            let geosolve_core::VariableValue::Scalar(actual) =
                compiled.problem().variable(variable).unwrap().value()
            else {
                panic!("arc angle coordinate must be scalar")
            };
            assert_eq!(actual.to_bits(), expected.to_bits());
        }
    }
}

fn assert_sketch_fillet_runtime_is_bitwise_certified(
    session: &SketchSession,
    arcs: &[geosolve_sketch::ArcId],
) {
    let compiled = session.sketch().compile(session.request()).unwrap();
    let packed = compiled.problem().packed_state().unwrap();
    let accepted = &session
        .accepted_result()
        .unstable_core_report()
        .accepted_state;
    assert_eq!(packed.layout(), accepted.layout());
    assert_eq!(
        packed
            .ambient()
            .iter()
            .map(|value| value.to_bits())
            .collect::<Vec<_>>(),
        accepted
            .ambient()
            .iter()
            .map(|value| value.to_bits())
            .collect::<Vec<_>>()
    );
    assert!(compiled.arc_angle_variables().len() >= 2 * arcs.len());

    let geometry = session.sketch().geometry();
    for arc in arcs {
        let solved = geometry.arc(*arc).unwrap();
        for (role, expected) in [
            (ArcAngleRole::Start, solved.start_angle),
            (ArcAngleRole::End, solved.end_angle),
        ] {
            let variable = compiled.variable_for_arc_angle(*arc, role).unwrap();
            let geosolve_core::VariableValue::Scalar(actual) =
                compiled.problem().variable(variable).unwrap().value()
            else {
                panic!("arc angle coordinate must be scalar")
            };
            assert_eq!(actual.to_bits(), expected.to_bits());
        }
    }
}

fn transformed(point: [f64; 2], scale: f64, angle: f64, offset: [f64; 2]) -> [f64; 2] {
    let (sine, cosine) = angle.sin_cos();
    [
        scale * (cosine * point[0] - sine * point[1]) + offset[0],
        scale * (sine * point[0] + cosine * point[1]) + offset[1],
    ]
}

fn crossing_parents(
    scale: f64,
    angle: f64,
    offset: [f64; 2],
    fixed: bool,
) -> (
    SketchDocument,
    [geosolve_sketch::CurveId; 2],
    [geosolve_sketch::DesignPointId; 4],
) {
    let mut document = SketchDocument::new(scale).unwrap();
    let positions = [[-4.0, 0.0], [4.0, 0.0], [0.0, -4.0], [0.0, 4.0]]
        .map(|point| transformed(point, scale, angle, offset));
    let points = positions.map(|position| document.add_point("parent point", position).unwrap());
    let (sine, cosine) = angle.sin_cos();
    let curves = [
        document
            .add_curve(
                "first parent",
                CurveDefinition::Line {
                    start: points[0],
                    end: points[1],
                    branch_direction: [cosine, sine],
                },
            )
            .unwrap(),
        document
            .add_curve(
                "second parent",
                CurveDefinition::Line {
                    start: points[2],
                    end: points[3],
                    branch_direction: [-sine, cosine],
                },
            )
            .unwrap(),
    ];
    if fixed {
        for (index, (point, target)) in points.into_iter().zip(positions).enumerate() {
            document
                .add_constraint(
                    format!("fixed parent {index}"),
                    DocumentConstraintDefinition::FixedPoint { point, target },
                )
                .unwrap();
        }
    }
    (document, curves, points)
}

fn request(
    curves: [geosolve_sketch::CurveId; 2],
    first_side: DocumentCurveNormalSide,
    second_side: DocumentCurveNormalSide,
    endpoint_order: DocumentFilletEndpointOrder,
    sweep: DocumentArcSweep,
    radius: f64,
    radius_mode: DocumentDimensionMode,
) -> LineLineFilletRequest {
    LineLineFilletRequest {
        first: CurveSpan::line(curves[0]),
        first_side,
        second: CurveSpan::line(curves[1]),
        second_side,
        endpoint_order,
        sweep,
        radius,
        radius_mode,
    }
}

fn add_fixed_right_angle_runtime_fillet(
    sketch: &mut Sketch,
    offset: f64,
) -> (geosolve_sketch::ArcId, geosolve_sketch::PointId) {
    let first_start = sketch.add_point(Point2::new(offset, 0.0)).unwrap();
    let corner = sketch.add_point(Point2::new(offset + 4.0, 0.0)).unwrap();
    let second_end = sketch.add_point(Point2::new(offset + 4.0, 4.0)).unwrap();
    let center = sketch.add_point(Point2::new(offset + 3.0, 1.0)).unwrap();
    let first = sketch.add_segment(first_start, corner).unwrap();
    let second = sketch.add_segment(corner, second_end).unwrap();
    for point in [first_start, corner, second_end] {
        sketch.add_fixed_point(point).unwrap();
    }
    let arc = sketch
        .add_arc(
            center,
            1.0,
            -std::f64::consts::FRAC_PI_2,
            0.0,
            ArcSweep::CounterClockwise,
        )
        .unwrap();
    let contact = |segment, parameter| SketchCurveContact {
        curve: SketchCurve::Line {
            segment,
            domain: LineParameterDomain::BoundedSegment,
        },
        parameter,
        neighborhood: CurveContactNeighborhood::Interior,
    };
    sketch
        .add_line_line_fillet(
            arc,
            contact(first, 0.75),
            CurveNormalSide::Left,
            contact(second, 0.25),
            CurveNormalSide::Left,
            FilletEndpointOrder::FirstThenSecond,
        )
        .unwrap();
    sketch
        .add_arc_radius(arc, 1.0, DimensionMode::Reference)
        .unwrap();
    (arc, center)
}

#[test]
fn runtime_line_fillet_rows_derive_the_accepted_arc() {
    let mut sketch = Sketch::new(1.0).unwrap();
    let first_start = sketch.add_point(Point2::new(0.0, 0.0)).unwrap();
    let corner = sketch.add_point(Point2::new(4.0, 0.0)).unwrap();
    let second_end = sketch.add_point(Point2::new(4.0, 4.0)).unwrap();
    let center = sketch.add_point(Point2::new(3.2, 0.8)).unwrap();
    let first = sketch.add_segment(first_start, corner).unwrap();
    let second = sketch.add_segment(corner, second_end).unwrap();
    for point in [first_start, corner, second_end] {
        sketch.add_fixed_point(point).unwrap();
    }
    let arc = sketch
        .add_arc(
            center,
            0.8,
            -std::f64::consts::FRAC_PI_2,
            0.0,
            ArcSweep::CounterClockwise,
        )
        .unwrap();
    let contact = |segment, parameter| SketchCurveContact {
        curve: SketchCurve::Line {
            segment,
            domain: LineParameterDomain::BoundedSegment,
        },
        parameter,
        neighborhood: CurveContactNeighborhood::Interior,
    };
    let mut preused = sketch.clone();
    preused.add_point_on_arc(first_start, arc, 0.5).unwrap();
    assert!(
        preused
            .add_line_line_fillet(
                arc,
                contact(first, 0.8),
                CurveNormalSide::Left,
                contact(second, 0.2),
                CurveNormalSide::Left,
                FilletEndpointOrder::FirstThenSecond,
            )
            .is_err()
    );
    sketch
        .add_line_line_fillet(
            arc,
            contact(first, 0.8),
            CurveNormalSide::Left,
            contact(second, 0.2),
            CurveNormalSide::Left,
            FilletEndpointOrder::FirstThenSecond,
        )
        .unwrap();
    sketch
        .add_arc_radius(arc, 1.0, DimensionMode::Driving)
        .unwrap();

    let compiled = sketch.compile(SketchSolveRequest::default()).unwrap();
    let jacobians = compiled.problem().check_jacobians(1.0e-6).unwrap();
    assert!(jacobians.all_within(2.0e-6), "{jacobians:#?}");
    assert_eq!(
        compiled
            .problem()
            .audit_rows()
            .unwrap()
            .iter()
            .filter(|row| row.template.contains("left_normal"))
            .count(),
        4
    );

    let result = sketch
        .solve(SketchSolveRequest::default(), SolverConfig::default())
        .unwrap();
    assert!(result.accepted(), "{:#?}", result.rejection);
    assert_eq!(
        result.unstable_core_report().termination,
        SolveTermination::Converged
    );
    assert_eq!(
        result.unstable_core_report().hard_validity,
        HardValidity::Valid
    );
    let solved = result.geometry.arc(arc).unwrap();
    assert!((solved.center.x - 3.0).abs() <= 1.0e-9);
    assert!((solved.center.y - 1.0).abs() <= 1.0e-9);
    assert!((solved.radius - 1.0).abs() <= 1.0e-9);
    let (start, end) = solved.endpoints().unwrap();
    assert!((start - Point2::new(3.0, 0.0)).norm() <= 1.0e-9);
    assert!((end - Point2::new(4.0, 1.0)).norm() <= 1.0e-9);
    assert_eq!(result.unstable_core_report().local_degrees_of_freedom, 0);
    assert!(
        (sketch
            .measure_curve(
                SketchCurveContact {
                    curve: SketchCurve::Arc(arc),
                    parameter: 0.5,
                    neighborhood: CurveContactNeighborhood::Interior,
                },
                CurveMeasurementKind::UnsignedCurvature,
            )
            .unwrap()
            - 1.0)
            .abs()
            <= 1.0e-9
    );
    assert!(sketch.add_point_on_arc(first_start, arc, 0.5).is_ok());
    assert!(
        sketch
            .add_line_line_fillet(
                arc,
                contact(first, 0.75),
                CurveNormalSide::Left,
                contact(second, 0.25),
                CurveNormalSide::Left,
                FilletEndpointOrder::FirstThenSecond,
            )
            .is_err()
    );
}

#[test]
fn runtime_curve_fillet_uses_local_periodic_and_unbounded_bounds() {
    let mut sketch = Sketch::new(1.0).unwrap();
    let circle_center = sketch.add_point(Point2::origin()).unwrap();
    let line_start = sketch.add_point(Point2::new(0.0, 1.0)).unwrap();
    let line_end = sketch.add_point(Point2::new(6.0, 1.0)).unwrap();
    let fillet_center = sketch.add_point(Point2::new(3.1, 0.1)).unwrap();
    let circle = sketch.add_circle(circle_center, 2.0).unwrap();
    let line = sketch.add_segment(line_start, line_end).unwrap();
    for point in [circle_center, line_start, line_end] {
        sketch.add_fixed_point(point).unwrap();
    }
    sketch
        .add_circle_radius(circle, 2.0, DimensionMode::Driving)
        .unwrap();
    let arc = sketch
        .add_arc(
            fillet_center,
            0.9,
            std::f64::consts::FRAC_PI_2,
            std::f64::consts::PI,
            ArcSweep::CounterClockwise,
        )
        .unwrap();
    let fillet = sketch
        .add_curve_curve_fillet(
            arc,
            SketchCurveContact {
                curve: SketchCurve::Circle(circle),
                parameter: 0.0,
                neighborhood: CurveContactNeighborhood::Local {
                    lower: -0.4,
                    upper: 0.4,
                },
            },
            CurveNormalSide::Right,
            SketchCurveContact {
                curve: SketchCurve::Line {
                    segment: line,
                    domain: LineParameterDomain::SupportingLine,
                },
                parameter: 0.5,
                neighborhood: CurveContactNeighborhood::Local {
                    lower: 0.25,
                    upper: 0.75,
                },
            },
            CurveNormalSide::Right,
            FilletEndpointOrder::SecondThenFirst,
        )
        .unwrap();
    sketch
        .add_arc_radius(arc, 1.0, DimensionMode::Driving)
        .unwrap();

    let compiled = sketch.compile(SketchSolveRequest::default()).unwrap();
    let expected_bounds: [(LatentVariableRole, f64, f64); 2] = [
        (LatentVariableRole::FirstCurveParameter, -0.4, 0.4),
        (LatentVariableRole::SecondCurveParameter, 0.25, 0.75),
    ];
    for (role, expected_lower, expected_upper) in expected_bounds {
        let mapping = compiled
            .bound_mappings()
            .iter()
            .find(|mapping| {
                mapping.bound
                    == SketchBound::Contact {
                        constraint_id: fillet,
                        role,
                    }
            })
            .unwrap();
        let bound = compiled.problem().bound(mapping.bound_id).unwrap();
        assert_eq!(bound.lower(), Some(expected_lower.next_up()));
        assert_eq!(bound.upper(), Some(expected_upper.next_down()));
    }
    assert_eq!(
        compiled
            .problem()
            .audit_rows()
            .unwrap()
            .iter()
            .filter(|row| row.source_label.contains("associative curve fillet"))
            .count(),
        6
    );
    let jacobians = compiled.problem().check_jacobians(1.0e-6).unwrap();
    assert!(jacobians.all_within(2.0e-6), "{jacobians:#?}");

    let result = sketch
        .solve(SketchSolveRequest::default(), SolverConfig::default())
        .unwrap();
    assert!(result.accepted(), "{:#?}", result.rejection);
    let solved = result.geometry.arc(arc).unwrap();
    assert!((solved.center - Point2::new(3.0, 0.0)).norm() <= 1.0e-9);
    let (start, end) = solved.endpoints().unwrap();
    assert!((start - Point2::new(3.0, 1.0)).norm() <= 1.0e-9);
    assert!((end - Point2::new(2.0, 0.0)).norm() <= 1.0e-9);
    assert!(matches!(
        sketch.contact_state(fillet).unwrap(),
        ContactState::CurveCurveFillet { .. }
    ));
    assert!(sketch.add_point_on_arc(circle_center, arc, 0.5).is_ok());
}

#[test]
fn runtime_curve_fillet_rejects_unresolved_offset_regularity() {
    let mut sketch = Sketch::new(1.0).unwrap();
    let radius = 1.0 - 5.0e-9;
    let center_x = 1.0 - radius;
    let circle_center = sketch.add_point(Point2::origin()).unwrap();
    let line_start = sketch.add_point(Point2::new(-2.0, -radius)).unwrap();
    let line_end = sketch.add_point(Point2::new(2.0, -radius)).unwrap();
    let fillet_center = sketch.add_point(Point2::new(center_x, 0.0)).unwrap();
    let circle = sketch.add_circle(circle_center, 1.0).unwrap();
    let line = sketch.add_segment(line_start, line_end).unwrap();
    for point in [circle_center, line_start, line_end, fillet_center] {
        sketch.add_fixed_point(point).unwrap();
    }
    sketch
        .add_circle_radius(circle, 1.0, DimensionMode::Driving)
        .unwrap();
    let arc = sketch
        .add_arc(
            fillet_center,
            radius,
            0.0,
            -std::f64::consts::FRAC_PI_2,
            ArcSweep::Clockwise,
        )
        .unwrap();
    let fillet = sketch
        .add_curve_curve_fillet(
            arc,
            SketchCurveContact {
                curve: SketchCurve::Circle(circle),
                parameter: 0.0,
                neighborhood: CurveContactNeighborhood::Local {
                    lower: -0.25,
                    upper: 0.25,
                },
            },
            CurveNormalSide::Left,
            SketchCurveContact {
                curve: SketchCurve::Line {
                    segment: line,
                    domain: LineParameterDomain::BoundedSegment,
                },
                parameter: (center_x + 2.0) / 4.0,
                neighborhood: CurveContactNeighborhood::Interior,
            },
            CurveNormalSide::Left,
            FilletEndpointOrder::FirstThenSecond,
        )
        .unwrap();
    sketch
        .add_arc_radius(arc, radius, DimensionMode::Driving)
        .unwrap();

    let result = sketch
        .solve(SketchSolveRequest::default(), SolverConfig::default())
        .unwrap();
    assert!(!result.accepted());
    assert!(matches!(
        result.rejection,
        Some(SolveRejection::InvalidFilletGeometry(rejected)) if rejected == fillet
    ));
    assert!(result.geometry.arc(arc).unwrap().center.x.is_finite());
}

#[test]
fn persistent_line_fillet_round_trips_and_projects_derived_endpoints() {
    let mut document = SketchDocument::new(1.0).unwrap();
    let first_start = document.add_point("first start", [0.0, 0.0]).unwrap();
    let corner = document.add_point("corner", [4.0, 0.0]).unwrap();
    let second_end = document.add_point("second end", [4.0, 4.0]).unwrap();
    let first = document
        .add_curve(
            "first",
            CurveDefinition::Line {
                start: first_start,
                end: corner,
                branch_direction: [1.0, 0.0],
            },
        )
        .unwrap();
    let second = document
        .add_curve(
            "second",
            CurveDefinition::Line {
                start: corner,
                end: second_end,
                branch_direction: [0.0, 1.0],
            },
        )
        .unwrap();
    for (label, point, target) in [
        ("fix first", first_start, [0.0, 0.0]),
        ("fix corner", corner, [4.0, 0.0]),
        ("fix second", second_end, [4.0, 4.0]),
    ] {
        document
            .add_constraint(
                label,
                DocumentConstraintDefinition::FixedPoint { point, target },
            )
            .unwrap();
    }
    let fillet = document
        .add_line_line_fillet(
            "fillet",
            LineLineFilletRequest {
                first: CurveSpan::line(first),
                first_side: DocumentCurveNormalSide::Left,
                second: CurveSpan::line(second),
                second_side: DocumentCurveNormalSide::Left,
                endpoint_order: DocumentFilletEndpointOrder::FirstThenSecond,
                sweep: geosolve_sketch::DocumentArcSweep::CounterClockwise,
                radius: 1.0,
                radius_mode: DocumentDimensionMode::Driving,
            },
        )
        .unwrap();
    let canonical = document.to_canonical_json().unwrap();
    assert!(canonical.contains("\"version\":4"));
    assert!(canonical.contains("\"kind\":\"line_line_fillet\""));
    assert_eq!(
        SketchDocument::from_json(&canonical)
            .unwrap()
            .to_canonical_json()
            .unwrap(),
        canonical
    );
    for old_version in [1, 2, 3] {
        assert!(
            SketchDocument::from_json(&canonical.replacen(
                "\"version\":4",
                &format!("\"version\":{old_version}"),
                1
            ))
            .is_err()
        );
    }
    let mut frozen_v3: serde_json::Value = serde_json::from_str(&canonical).unwrap();
    frozen_v3["version"] = 3.into();
    frozen_v3.as_object_mut().unwrap().remove("trim_views");
    let migrated_v3 =
        SketchDocument::from_json(&serde_json::to_string(&frozen_v3).unwrap()).unwrap();
    assert_eq!(migrated_v3.version(), 4);
    assert!(migrated_v3.constraints().iter().any(|constraint| matches!(
        constraint.definition,
        DocumentConstraintDefinition::LineLineFillet { .. }
    )));

    let session = SketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    assert!(session.runtime().accepted_result().accepted());
    let accepted = session.document();
    let start = accepted
        .evaluate_curve_jet(CurveSpan::line(fillet.arc), 0.0)
        .unwrap()
        .position;
    let end = accepted
        .evaluate_curve_jet(CurveSpan::line(fillet.arc), 1.0)
        .unwrap()
        .position;
    assert!((start - Point2::new(3.0, 0.0)).norm() <= 1.0e-9);
    assert!((end - Point2::new(4.0, 1.0)).norm() <= 1.0e-9);
    assert!(
        accepted
            .project_curve_trim_endpoint(
                fillet.arc,
                geosolve_sketch::FeatureEndpoint::Start,
                [3.0, 0.0]
            )
            .is_err()
    );
    assert!(
        accepted
            .clone()
            .set_scalar_value(fillet.start_angle, 0.25)
            .is_err()
    );
    let mut consumer = accepted.clone();
    let consumer_point = consumer.add_point("consumer", [3.0, 0.0]).unwrap();
    let consumer_parameter = consumer
        .add_scalar(
            "consumer parameter",
            0.5,
            ScalarUnit::Parameter,
            ScalarDomain::Bounded {
                lower: 0.0,
                upper: 1.0,
            },
        )
        .unwrap();
    let consumer_contact = consumer
        .add_contact(
            "consumer contact",
            ContactDefinition {
                curve: CurveSpan::line(fillet.arc),
                parameter: consumer_parameter,
                domain: ContactDomain::Bounded {
                    lower: 0.0,
                    upper: 1.0,
                },
                winding: 0,
                neighborhood: ContactNeighborhood::Interior,
                tangent_orientation: None,
            },
        )
        .unwrap();
    assert!(
        consumer
            .add_constraint(
                "associated fillet consumer",
                DocumentConstraintDefinition::PointOnCurve {
                    point: consumer_point,
                    contact: consumer_contact,
                }
            )
            .is_ok()
    );
}

#[test]
fn every_side_order_and_sweep_is_similarity_covariant_at_all_scales() {
    let angle = 0.37;
    for scale in [1.0e-6, 1.0, 1.0e6] {
        let offset = [7.0 * scale, -3.0 * scale];
        for first_side in [
            DocumentCurveNormalSide::Left,
            DocumentCurveNormalSide::Right,
        ] {
            for second_side in [
                DocumentCurveNormalSide::Left,
                DocumentCurveNormalSide::Right,
            ] {
                for endpoint_order in [
                    DocumentFilletEndpointOrder::FirstThenSecond,
                    DocumentFilletEndpointOrder::SecondThenFirst,
                ] {
                    for sweep in [
                        DocumentArcSweep::CounterClockwise,
                        DocumentArcSweep::Clockwise,
                    ] {
                        let (mut document, curves, _) =
                            crossing_parents(scale, angle, offset, true);
                        let ids = document
                            .add_line_line_fillet(
                                "branch fillet",
                                request(
                                    curves,
                                    first_side,
                                    second_side,
                                    endpoint_order,
                                    sweep,
                                    scale,
                                    DocumentDimensionMode::Driving,
                                ),
                            )
                            .unwrap();
                        if first_side == DocumentCurveNormalSide::Left
                            && second_side == DocumentCurveNormalSide::Left
                            && endpoint_order == DocumentFilletEndpointOrder::FirstThenSecond
                            && sweep == DocumentArcSweep::CounterClockwise
                        {
                            let lowered = document.lower().unwrap();
                            let compiled = lowered
                                .sketch()
                                .compile(SketchSolveRequest::default())
                                .unwrap();
                            let jacobians = compiled.problem().check_jacobians(1.0e-6).unwrap();
                            assert!(jacobians.all_within(2.0e-6), "{jacobians:#?}");
                        }
                        let session = SketchDocumentSession::new(
                            document,
                            DocumentSolveRequest::default(),
                            SolverConfig::default(),
                        )
                        .unwrap();
                        assert!(session.runtime().accepted_result().accepted());
                        assert_fillet_runtime_is_bitwise_certified(&session, &[ids.arc]);
                        assert_eq!(
                            session
                                .runtime()
                                .accepted_result()
                                .unstable_core_report()
                                .local_degrees_of_freedom,
                            0
                        );
                        let first_sign = if first_side == DocumentCurveNormalSide::Left {
                            1.0
                        } else {
                            -1.0
                        };
                        let second_sign = if second_side == DocumentCurveNormalSide::Left {
                            1.0
                        } else {
                            -1.0
                        };
                        let first_contact = transformed([-second_sign, 0.0], scale, angle, offset);
                        let second_contact = transformed([0.0, first_sign], scale, angle, offset);
                        let (expected_start, expected_end) = match endpoint_order {
                            DocumentFilletEndpointOrder::FirstThenSecond => {
                                (first_contact, second_contact)
                            }
                            DocumentFilletEndpointOrder::SecondThenFirst => {
                                (second_contact, first_contact)
                            }
                        };
                        let accepted = session.document();
                        let start = accepted
                            .evaluate_curve_jet(CurveSpan::line(ids.arc), 0.0)
                            .unwrap()
                            .position;
                        let end = accepted
                            .evaluate_curve_jet(CurveSpan::line(ids.arc), 1.0)
                            .unwrap()
                            .position;
                        let tolerance = 2.0e-9 * scale;
                        assert!(
                            (start - Point2::new(expected_start[0], expected_start[1])).norm()
                                <= tolerance,
                            "scale={scale:e} {first_side:?} {second_side:?} {endpoint_order:?} {sweep:?}"
                        );
                        assert!(
                            (end - Point2::new(expected_end[0], expected_end[1])).norm()
                                <= tolerance
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn multiple_fillet_outputs_are_bitwise_certified_in_one_drag_solve() {
    let mut sketch = Sketch::new(16.0).unwrap();
    let (first_arc, first_center) = add_fixed_right_angle_runtime_fillet(&mut sketch, 0.0);
    let (second_arc, _) = add_fixed_right_angle_runtime_fillet(&mut sketch, 10.0);
    let ordinary_center = sketch.add_point(Point2::new(20.0, 2.0)).unwrap();
    sketch.add_fixed_point(ordinary_center).unwrap();
    let ordinary_arc = sketch
        .add_arc(ordinary_center, 2.0, 0.25, 1.25, ArcSweep::CounterClockwise)
        .unwrap();
    sketch
        .add_fixed_arc_angle(ordinary_arc, ArcAngleEndpoint::Start, 0.25)
        .unwrap();
    sketch
        .add_fixed_arc_angle(ordinary_arc, ArcAngleEndpoint::End, 1.25)
        .unwrap();
    sketch
        .add_arc_radius(ordinary_arc, 2.0, DimensionMode::Driving)
        .unwrap();
    let second_before = {
        let geometry = sketch.geometry();
        let arc = geometry.arc(second_arc).unwrap();
        [arc.start_angle.to_bits(), arc.end_angle.to_bits()]
    };
    let request = SketchSolveRequest::default().with_drag(first_center, Point2::new(2.0, 2.0));

    let session = SketchSession::new(sketch, request, SolverConfig::default()).unwrap();
    assert!(session.accepted_result().accepted());
    assert_sketch_fillet_runtime_is_bitwise_certified(&session, &[first_arc, second_arc]);
    let compiled = session.sketch().compile(session.request()).unwrap();
    assert_eq!(compiled.arc_angle_variables().len(), 6);
    let geometry = session.sketch().geometry();
    let ordinary = geometry.arc(ordinary_arc).unwrap();
    for (role, expected) in [
        (ArcAngleRole::Start, ordinary.start_angle),
        (ArcAngleRole::End, ordinary.end_angle),
    ] {
        let variable = compiled.variable_for_arc_angle(ordinary_arc, role).unwrap();
        let geosolve_core::VariableValue::Scalar(actual) =
            compiled.problem().variable(variable).unwrap().value()
        else {
            panic!("ordinary arc angle coordinate must be scalar")
        };
        assert_eq!(actual.to_bits(), expected.to_bits());
    }
    assert_eq!(ordinary.start_angle.to_bits(), 0.25_f64.to_bits());
    assert_eq!(ordinary.end_angle.to_bits(), 1.25_f64.to_bits());
    let first_center_after = session.sketch().geometry().point(first_center).unwrap();
    assert!((first_center_after - Point2::new(2.0, 2.0)).norm() <= 1.0e-9);
    let second_after = {
        let geometry = session.sketch().geometry();
        let arc = geometry.arc(second_arc).unwrap();
        [arc.start_angle.to_bits(), arc.end_angle.to_bits()]
    };
    assert_eq!(second_after, second_before);
}

#[test]
fn reference_radius_keeps_one_fillet_dof_and_reports_its_measurement() {
    let (mut document, curves, _) = crossing_parents(1.0, 0.0, [0.0, 0.0], true);
    let ids = document
        .add_line_line_fillet(
            "reference fillet",
            request(
                curves,
                DocumentCurveNormalSide::Left,
                DocumentCurveNormalSide::Left,
                DocumentFilletEndpointOrder::FirstThenSecond,
                DocumentArcSweep::CounterClockwise,
                1.0,
                DocumentDimensionMode::Reference,
            ),
        )
        .unwrap();
    let session = SketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    assert_eq!(
        session
            .runtime()
            .accepted_result()
            .unstable_core_report()
            .local_degrees_of_freedom,
        1
    );
    let result = session.accepted_result();
    assert!(
        (result
            .accepted_reference_value(session.document(), ids.radius_dimension)
            .unwrap()
            - 1.0)
            .abs()
            <= 1.0e-9
    );
}

#[test]
fn parent_edit_rederives_contacts_without_trimming_parent_definitions() {
    let (mut document, curves, points) = crossing_parents(1.0, 0.0, [0.0, 0.0], false);
    let ids = document
        .add_line_line_fillet(
            "editable fillet",
            request(
                curves,
                DocumentCurveNormalSide::Left,
                DocumentCurveNormalSide::Left,
                DocumentFilletEndpointOrder::FirstThenSecond,
                DocumentArcSweep::CounterClockwise,
                1.0,
                DocumentDimensionMode::Driving,
            ),
        )
        .unwrap();
    let mut session = SketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let before = session
        .document()
        .evaluate_curve_jet(CurveSpan::line(ids.arc), 0.0)
        .unwrap()
        .position;
    let edit = session
        .apply(DocumentCommand::new(
            session.revision(),
            DocumentEdit::SetPointPosition {
                point: points[1],
                position: [4.0, 1.0],
            },
        ))
        .unwrap();
    assert!(edit.accepted(), "{:#?}", edit.result.solve().rejection);
    let accepted = session.document();
    let after = accepted
        .evaluate_curve_jet(CurveSpan::line(ids.arc), 0.0)
        .unwrap()
        .position;
    assert!((after - before).norm() > 1.0e-3);
    let first_contact = accepted
        .evaluate_contact_jet(ids.contacts[0])
        .unwrap()
        .position;
    assert!((after - first_contact).norm() <= 1.0e-9);
    for curve in curves {
        assert!(matches!(
            accepted.curve(curve).unwrap().definition,
            CurveDefinition::Line { .. }
        ));
    }
}

#[test]
fn creation_branch_history_explode_and_failed_escape_are_atomic() {
    let (document, curves, _) = crossing_parents(1.0, 0.0, [0.0, 0.0], true);
    let mut session = SketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let created = session
        .apply(DocumentCommand::new(
            session.revision(),
            DocumentEdit::CreateLineLineFillet {
                label: "command fillet".into(),
                request: request(
                    curves,
                    DocumentCurveNormalSide::Left,
                    DocumentCurveNormalSide::Left,
                    DocumentFilletEndpointOrder::FirstThenSecond,
                    DocumentArcSweep::CounterClockwise,
                    1.0,
                    DocumentDimensionMode::Driving,
                ),
            },
        ))
        .unwrap();
    assert!(created.accepted());
    let DocumentCommandEffect::CreatedLineLineFillet(ids) = created.effect.unwrap() else {
        panic!("unexpected creation effect")
    };
    let ids: LineLineFilletIds = *ids;
    let accepted = session.export_json().unwrap();
    session.undo(session.revision()).unwrap();
    session.redo(session.revision()).unwrap();
    assert_eq!(session.export_json().unwrap(), accepted);

    let before_escape = session.export_json().unwrap();
    let history = session.history_len();
    let escaped = session
        .apply(DocumentCommand::new(
            session.revision(),
            DocumentEdit::SetScalarValue {
                scalar: ids.radius_target,
                value: 10.0,
            },
        ))
        .unwrap();
    assert!(!escaped.accepted());
    assert_eq!(session.export_json().unwrap(), before_escape);
    assert_eq!(session.history_len(), history);

    let branch = session
        .apply(DocumentCommand::new(
            session.revision(),
            DocumentEdit::SetLineLineFilletBranch {
                constraint: ids.constraint,
                first_side: DocumentCurveNormalSide::Left,
                second_side: DocumentCurveNormalSide::Left,
                endpoint_order: DocumentFilletEndpointOrder::SecondThenFirst,
                sweep: DocumentArcSweep::Clockwise,
            },
        ))
        .unwrap();
    assert!(branch.accepted());

    let source = session
        .document()
        .constraint(ids.constraint)
        .unwrap()
        .source_id;
    assert!(
        session
            .apply(DocumentCommand::new(
                session.revision(),
                DocumentEdit::SetSourceSuppressed {
                    source,
                    suppressed: true,
                },
            ))
            .unwrap()
            .accepted()
    );
    assert!(
        session
            .apply(DocumentCommand::new(
                session.revision(),
                DocumentEdit::SetScalarValue {
                    scalar: ids.start_angle,
                    value: 0.2,
                },
            ))
            .unwrap()
            .accepted()
    );
    assert!(
        session
            .apply(DocumentCommand::new(
                session.revision(),
                DocumentEdit::SetSourceSuppressed {
                    source,
                    suppressed: false,
                },
            ))
            .unwrap()
            .accepted()
    );
    assert!(
        (session
            .document()
            .evaluate_curve_jet(CurveSpan::line(ids.arc), 0.0)
            .unwrap()
            .position
            - Point2::new(0.0, 1.0))
        .norm()
            <= 1.0e-9
    );

    let mut selected_arc_delete = session.document().clone();
    assert!(matches!(
        selected_arc_delete.remove_many_with_dependents(&[DocumentObjectId::Curve(ids.arc)]),
        Err(geosolve_sketch::DocumentError::ObjectInUse(id)) if id == ids.arc.0
    ));
    for object in [
        DocumentObjectId::Point(ids.center),
        DocumentObjectId::Scalar(ids.radius),
        DocumentObjectId::Scalar(ids.start_angle),
    ] {
        let mut indirect = session.document().clone();
        assert!(matches!(
            indirect.remove_many_with_dependents(&[object]),
            Err(geosolve_sketch::DocumentError::ObjectInUse(id)) if id == ids.arc.0
        ));
    }
    let mut suppressed_owner = session.document().clone();
    suppressed_owner
        .set_source_suppressed(source, true)
        .unwrap();
    assert!(suppressed_owner.line_line_fillet_for_arc(ids.arc).is_none());
    assert!(matches!(
        suppressed_owner.remove_many_with_dependents(&[DocumentObjectId::Curve(ids.arc)]),
        Err(geosolve_sketch::DocumentError::ObjectInUse(id)) if id == ids.arc.0
    ));

    let deleted = session
        .apply(DocumentCommand::new(
            session.revision(),
            DocumentEdit::Delete {
                object: DocumentObjectId::Constraint(ids.constraint),
            },
        ))
        .unwrap();
    assert!(deleted.accepted());
    assert!(session.document().curve(ids.arc).is_some());
    assert!(
        ids.contacts
            .iter()
            .all(|contact| session.document().contact(*contact).is_none())
    );
    assert!(
        session
            .document()
            .line_line_fillet_for_arc(ids.arc)
            .is_none()
    );
    session.undo(session.revision()).unwrap();
    assert!(
        session
            .document()
            .line_line_fillet_for_arc(ids.arc)
            .is_some()
    );
    session.redo(session.revision()).unwrap();
    assert!(session.document().curve(ids.arc).is_some());
}

#[test]
fn invalid_parallel_escaped_radius_and_nonfinite_inputs_allocate_nothing() {
    let (mut escaped, curves, _) = crossing_parents(1.0, 0.0, [0.0, 0.0], false);
    let before = escaped.to_canonical_json().unwrap();
    assert!(
        escaped
            .add_line_line_fillet(
                "escaped",
                request(
                    curves,
                    DocumentCurveNormalSide::Left,
                    DocumentCurveNormalSide::Left,
                    DocumentFilletEndpointOrder::FirstThenSecond,
                    DocumentArcSweep::CounterClockwise,
                    5.0,
                    DocumentDimensionMode::Driving,
                )
            )
            .is_err()
    );
    assert_eq!(escaped.to_canonical_json().unwrap(), before);
    for radius in [0.0, f64::NAN, f64::INFINITY] {
        assert!(
            escaped
                .add_line_line_fillet(
                    "invalid radius",
                    request(
                        curves,
                        DocumentCurveNormalSide::Left,
                        DocumentCurveNormalSide::Left,
                        DocumentFilletEndpointOrder::FirstThenSecond,
                        DocumentArcSweep::CounterClockwise,
                        radius,
                        DocumentDimensionMode::Driving,
                    )
                )
                .is_err()
        );
    }

    for slope in [0.0, 1.0e-10] {
        let mut parallel = SketchDocument::new(1.0).unwrap();
        let points = [
            [-4.0, 0.0],
            [4.0, 0.0],
            [-4.0, 1.0],
            [4.0, 1.0 + 8.0 * slope],
        ]
        .map(|position| parallel.add_point("parallel", position).unwrap());
        let first = parallel
            .add_curve(
                "parallel first",
                CurveDefinition::Line {
                    start: points[0],
                    end: points[1],
                    branch_direction: [1.0, 0.0],
                },
            )
            .unwrap();
        let direction = [
            1.0 / (1.0 + slope * slope).sqrt(),
            slope / (1.0 + slope * slope).sqrt(),
        ];
        let second = parallel
            .add_curve(
                "parallel second",
                CurveDefinition::Line {
                    start: points[2],
                    end: points[3],
                    branch_direction: direction,
                },
            )
            .unwrap();
        assert!(
            parallel
                .add_line_line_fillet(
                    "parallel fillet",
                    request(
                        [first, second],
                        DocumentCurveNormalSide::Left,
                        DocumentCurveNormalSide::Right,
                        DocumentFilletEndpointOrder::FirstThenSecond,
                        DocumentArcSweep::CounterClockwise,
                        0.5,
                        DocumentDimensionMode::Driving,
                    )
                )
                .is_err()
        );
    }
}
