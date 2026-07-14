#![allow(clippy::too_many_lines)]
#![allow(clippy::many_single_char_names)]

use std::f64::consts::{FRAC_PI_2, PI};

use geosolve_core::{SolveTermination, SolverConfig};
use geosolve_geometry::Point2;
use geosolve_sketch::{
    AngleOrientation, ArcSweep, CenterDirectionBranch, CircleContainment, CircleTangencyMode,
    ContactState, DimensionKind, DimensionMode, LatentVariableRole, LineParameterDomain, LineSide,
    Sketch, SketchError, SketchSolveRequest, SketchSource, SolveRejection, tangent_circles,
};

const TOLERANCE: f64 = 1.0e-9;

fn solve(sketch: &mut Sketch) -> geosolve_sketch::SketchSolveResult {
    sketch
        .solve(SketchSolveRequest::default(), SolverConfig::default())
        .unwrap()
}

fn assert_accepted(result: &geosolve_sketch::SketchSolveResult) {
    assert!(result.accepted(), "{:#?}", result.rejection);
    assert_eq!(result.core_report.termination, SolveTermination::Converged);
    assert!(result.core_report.hard_residuals_validated);
    assert!(result.core_report.hard_residual_max <= TOLERANCE);
    assert!(result.acceptance_hard_residual_max.unwrap() <= TOLERANCE);
    assert_eq!(result.display_audit, result.core_report.audit);
}

fn assert_point(actual: Point2<f64>, expected: Point2<f64>, scale: f64, tolerance: f64) {
    let error = (actual - expected).norm() / scale;
    assert!(
        error <= tolerance,
        "actual={actual:?}, expected={expected:?}, normalized error={error:e}"
    );
}

fn transformed(point: Point2<f64>, scale: f64, angle: f64, offset: [f64; 2]) -> Point2<f64> {
    let (sine, cosine) = angle.sin_cos();
    Point2::new(
        scale * (cosine * point.x - sine * point.y) + offset[0],
        scale * (sine * point.x + cosine * point.y) + offset[1],
    )
}

#[test]
fn circle_and_arc_stores_have_stable_ids_labels_geometry_and_guarded_edits() {
    let mut sketch = Sketch::new(2.0).unwrap();
    let center = sketch.add_named_point("O", Point2::new(3.0, -2.0)).unwrap();
    let circle = sketch.add_circle(center, 2.0).unwrap();
    let arc = sketch
        .add_named_arc(
            "quarter",
            center,
            3.0,
            0.0,
            FRAC_PI_2,
            ArcSweep::CounterClockwise,
        )
        .unwrap();
    assert_eq!(sketch.circle(circle).unwrap().label(), "C1");
    assert_eq!(sketch.arc(arc).unwrap().label(), "quarter");
    assert_eq!(
        sketch.circles().map(|(id, _)| id).collect::<Vec<_>>(),
        vec![circle]
    );
    assert_eq!(
        sketch.arcs().map(|(id, _)| id).collect::<Vec<_>>(),
        vec![arc]
    );
    assert_point(
        sketch.evaluate_circle(circle, PI).unwrap(),
        Point2::new(1.0, -2.0),
        2.0,
        1.0e-14,
    );
    let endpoints = sketch
        .arc(arc)
        .unwrap()
        .endpoints(sketch.point(center).unwrap().position())
        .unwrap();
    assert_point(endpoints.0, Point2::new(6.0, -2.0), 2.0, 1.0e-14);
    assert_point(endpoints.1, Point2::new(3.0, 1.0), 2.0, 1.0e-14);

    let geometry = sketch.geometry();
    assert!((geometry.circle(circle).unwrap().radius - 2.0).abs() <= f64::EPSILON);
    assert_eq!(geometry.arc(arc).unwrap().sweep, ArcSweep::CounterClockwise);
    assert_eq!(geometry.arc(arc).unwrap().endpoints().unwrap(), endpoints);

    for radius in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        assert!(matches!(
            sketch.set_circle_radius(circle, radius),
            Err(SketchError::InvalidRadius(_))
        ));
        assert!(matches!(
            sketch.set_arc_radius(arc, radius),
            Err(SketchError::InvalidRadius(_))
        ));
    }
    assert!((sketch.circle(circle).unwrap().radius() - 2.0).abs() <= f64::EPSILON);
    assert!((sketch.arc(arc).unwrap().radius() - 3.0).abs() <= f64::EPSILON);

    for (start, end) in [(0.0, 0.0), (0.0, 2.0 * PI), (f64::NAN, 1.0)] {
        assert_eq!(
            sketch.add_arc(center, 1.0, start, end, ArcSweep::CounterClockwise),
            Err(SketchError::InvalidArcSweep)
        );
    }

    let stale_circle = sketch.add_circle(center, 1.0).unwrap();
    sketch.remove_circle(stale_circle).unwrap();
    assert!(matches!(
        sketch.set_circle_radius(stale_circle, 2.0),
        Err(SketchError::UnknownCircle(id)) if id == stale_circle
    ));
    let stale_arc = sketch
        .add_arc(center, 1.0, 0.0, PI, ArcSweep::Clockwise)
        .unwrap();
    sketch.remove_arc(stale_arc).unwrap();
    assert!(matches!(
        sketch.set_arc_radius(stale_arc, 2.0),
        Err(SketchError::UnknownArc(id)) if id == stale_arc
    ));
}

#[test]
fn s3_external_and_internal_modes_are_explicit_transactional_and_scale_invariant() {
    let (mut sketch, ids) = tangent_circles().unwrap();
    let external = solve(&mut sketch);
    assert_accepted(&external);
    assert_point(
        external.geometry.point(ids.center_b).unwrap(),
        Point2::new(3.0, 0.0),
        1.0,
        3.0e-9,
    );
    assert!((external.geometry.circle(ids.circle_a).unwrap().radius - 2.0).abs() <= TOLERANCE);
    assert!((external.geometry.circle(ids.circle_b).unwrap().radius - 1.0).abs() <= TOLERANCE);

    sketch
        .set_circle_tangency_mode(
            ids.tangency,
            CircleTangencyMode::Internal {
                containment: CircleContainment::FirstContainsSecond,
            },
        )
        .unwrap();
    let internal = solve(&mut sketch);
    assert_accepted(&internal);
    assert_point(
        internal.geometry.point(ids.center_b).unwrap(),
        Point2::new(1.0, 0.0),
        1.0,
        3.0e-9,
    );

    let retained = sketch.geometry();
    assert_eq!(
        sketch.set_circle_tangency_mode(
            ids.tangency,
            CircleTangencyMode::Internal {
                containment: CircleContainment::SecondContainsFirst,
            },
        ),
        Err(SketchError::InvalidInternalTangency)
    );
    assert_eq!(sketch.geometry(), retained);

    for scale in [1.0e-6, 1.0, 1.0e6] {
        let mut scaled = Sketch::new(scale).unwrap();
        let a = scaled.add_named_point("A", Point2::new(0.0, 0.0)).unwrap();
        let b = scaled
            .add_named_point("B", Point2::new(5.0 * scale, 0.5 * scale))
            .unwrap();
        let centers = scaled.add_named_segment("centers", a, b).unwrap();
        let first = scaled.add_named_circle("A circle", a, 2.0 * scale).unwrap();
        let second = scaled.add_named_circle("B circle", b, scale).unwrap();
        scaled.add_fixed_point(a).unwrap();
        scaled.add_horizontal(centers).unwrap();
        scaled
            .add_circle_radius(first, 2.0 * scale, DimensionMode::Driving)
            .unwrap();
        scaled
            .add_circle_radius(second, scale, DimensionMode::Driving)
            .unwrap();
        let tangency = scaled
            .add_circle_circle_tangency(
                first,
                second,
                CircleTangencyMode::External,
                CenterDirectionBranch::positive_x(),
            )
            .unwrap();
        let result = solve(&mut scaled);
        assert_accepted(&result);
        assert_point(
            result.geometry.point(b).unwrap(),
            Point2::new(3.0 * scale, 0.0),
            scale,
            2.0e-9,
        );
        scaled
            .set_circle_tangency_mode(
                tangency,
                CircleTangencyMode::Internal {
                    containment: CircleContainment::FirstContainsSecond,
                },
            )
            .unwrap();
        let result = solve(&mut scaled);
        assert_accepted(&result);
        assert_point(
            result.geometry.point(b).unwrap(),
            Point2::new(scale, 0.0),
            scale,
            2.0e-9,
        );
    }
}

#[test]
fn bounded_line_and_arc_contacts_accept_interior_and_endpoints_but_reject_escape() {
    for parameter in [0.0, 0.35, 1.0] {
        let mut sketch = Sketch::new(2.0).unwrap();
        let a = sketch.add_named_point("A", Point2::new(0.0, 0.0)).unwrap();
        let b = sketch.add_named_point("B", Point2::new(2.0, 0.0)).unwrap();
        let point = sketch
            .add_named_point("P", Point2::new(2.0 * parameter, 0.0))
            .unwrap();
        let segment = sketch.add_named_segment("AB", a, b).unwrap();
        sketch.add_fixed_point(a).unwrap();
        sketch.add_fixed_point(b).unwrap();
        sketch.add_fixed_point(point).unwrap();
        let contact = sketch
            .add_point_on_line(
                point,
                segment,
                LineParameterDomain::BoundedSegment,
                parameter,
            )
            .unwrap();
        let result = solve(&mut sketch);
        assert_accepted(&result);
        assert_eq!(
            sketch.contact_state(contact).unwrap(),
            ContactState::PointOnLine { parameter }
        );
    }

    let mut recovered_line = Sketch::new(2.0).unwrap();
    let a = recovered_line.add_point(Point2::new(0.0, 0.0)).unwrap();
    let b = recovered_line.add_point(Point2::new(2.0, 0.0)).unwrap();
    let point = recovered_line.add_point(Point2::new(0.8, 0.6)).unwrap();
    let line = recovered_line.add_segment(a, b).unwrap();
    recovered_line.add_fixed_point(a).unwrap();
    recovered_line.add_fixed_point(b).unwrap();
    let contact = recovered_line
        .add_point_on_line(point, line, LineParameterDomain::BoundedSegment, 0.4)
        .unwrap();
    let result = solve(&mut recovered_line);
    assert_accepted(&result);
    assert!(result.geometry.point(point).unwrap().y.abs() <= TOLERANCE);
    let ContactState::PointOnLine { parameter } = recovered_line.contact_state(contact).unwrap()
    else {
        panic!("wrong recovered line state")
    };
    assert!((0.0..=1.0).contains(&parameter));

    for parameter in [0.0, 0.4, 1.0] {
        let mut sketch = Sketch::new(2.0).unwrap();
        let center = sketch.add_named_point("O", Point2::new(0.0, 0.0)).unwrap();
        let angle = PI * parameter;
        let point = sketch
            .add_named_point("P", Point2::new(2.0 * angle.cos(), 2.0 * angle.sin()))
            .unwrap();
        let arc = sketch
            .add_named_arc("upper", center, 2.0, 0.0, PI, ArcSweep::CounterClockwise)
            .unwrap();
        sketch.add_fixed_point(center).unwrap();
        sketch.add_fixed_point(point).unwrap();
        sketch
            .add_arc_radius(arc, 2.0, DimensionMode::Driving)
            .unwrap();
        let contact = sketch.add_point_on_arc(point, arc, parameter).unwrap();
        let result = solve(&mut sketch);
        assert_accepted(&result);
        let ContactState::PointOnArc { span_parameter } = sketch.contact_state(contact).unwrap()
        else {
            panic!("wrong contact state")
        };
        assert!((span_parameter - parameter).abs() <= TOLERANCE);
    }

    let mut recovered_arc = Sketch::new(2.0).unwrap();
    let center = recovered_arc.add_point(Point2::new(0.0, 0.0)).unwrap();
    let point = recovered_arc.add_point(Point2::new(0.4, 1.7)).unwrap();
    let arc = recovered_arc
        .add_arc(center, 2.0, 0.0, PI, ArcSweep::CounterClockwise)
        .unwrap();
    recovered_arc.add_fixed_point(center).unwrap();
    recovered_arc
        .add_arc_radius(arc, 2.0, DimensionMode::Driving)
        .unwrap();
    let contact = recovered_arc.add_point_on_arc(point, arc, 0.45).unwrap();
    let result = solve(&mut recovered_arc);
    assert_accepted(&result);
    let ContactState::PointOnArc { span_parameter } = recovered_arc.contact_state(contact).unwrap()
    else {
        panic!("wrong recovered arc state")
    };
    assert!((0.0..=1.0).contains(&span_parameter));
    assert_point(
        result
            .geometry
            .arc(arc)
            .unwrap()
            .evaluate(span_parameter)
            .unwrap(),
        result.geometry.point(point).unwrap(),
        2.0,
        TOLERANCE,
    );

    let mut sketch = Sketch::new(1.0).unwrap();
    let a = sketch.add_named_point("A", Point2::new(0.0, 0.0)).unwrap();
    let b = sketch.add_named_point("B", Point2::new(1.0, 0.0)).unwrap();
    let outside = sketch.add_named_point("P", Point2::new(2.0, 0.0)).unwrap();
    let segment = sketch.add_named_segment("AB", a, b).unwrap();
    sketch.add_fixed_point(a).unwrap();
    sketch.add_fixed_point(b).unwrap();
    sketch.add_fixed_point(outside).unwrap();
    let contact = sketch
        .add_point_on_line(outside, segment, LineParameterDomain::BoundedSegment, 0.5)
        .unwrap();
    let retained = sketch.geometry();
    let result = solve(&mut sketch);
    assert_eq!(
        result.rejection,
        Some(SolveRejection::ContactParameterOutOfDomain(contact))
    );
    assert_eq!(result.geometry, retained);
    assert_eq!(
        sketch.contact_state(contact).unwrap(),
        ContactState::PointOnLine { parameter: 0.5 }
    );

    sketch
        .set_contact_state(contact, ContactState::PointOnLine { parameter: 1.0 })
        .unwrap();
    assert!(matches!(
        sketch.set_contact_state(contact, ContactState::PointOnLine { parameter: 1.1 }),
        Err(SketchError::ParameterOutOfDomain { .. })
    ));

    let mut supporting = Sketch::new(1.0).unwrap();
    let a = supporting.add_point(Point2::new(0.0, 0.0)).unwrap();
    let b = supporting.add_point(Point2::new(1.0, 0.0)).unwrap();
    let point = supporting.add_point(Point2::new(2.0, 0.0)).unwrap();
    let line = supporting.add_segment(a, b).unwrap();
    supporting.add_fixed_point(a).unwrap();
    supporting.add_fixed_point(b).unwrap();
    supporting.add_fixed_point(point).unwrap();
    let contact = supporting
        .add_point_on_line(point, line, LineParameterDomain::SupportingLine, 1.7)
        .unwrap();
    let result = solve(&mut supporting);
    assert_accepted(&result);
    let ContactState::PointOnLine { parameter } = supporting.contact_state(contact).unwrap() else {
        panic!("wrong supporting-line contact state")
    };
    assert!((parameter - 2.0).abs() <= TOLERANCE);

    let mut arc_escape = Sketch::new(2.0).unwrap();
    let center = arc_escape.add_point(Point2::new(0.0, 0.0)).unwrap();
    let point = arc_escape.add_point(Point2::new(0.0, -2.0)).unwrap();
    let arc = arc_escape
        .add_arc(center, 2.0, 0.0, PI, ArcSweep::CounterClockwise)
        .unwrap();
    arc_escape.add_fixed_point(center).unwrap();
    arc_escape.add_fixed_point(point).unwrap();
    arc_escape
        .add_arc_radius(arc, 2.0, DimensionMode::Driving)
        .unwrap();
    let contact = arc_escape.add_point_on_arc(point, arc, 0.1).unwrap();
    let retained = arc_escape.geometry();
    let result = solve(&mut arc_escape);
    assert_eq!(
        result.rejection,
        Some(SolveRejection::ContactParameterOutOfDomain(contact))
    );
    assert_eq!(result.geometry, retained);
    assert_eq!(
        arc_escape.contact_state(contact).unwrap(),
        ContactState::PointOnArc {
            span_parameter: 0.1
        }
    );
}

#[test]
fn point_on_circle_is_periodic_and_recovers_with_accepted_latent_state() {
    for (scale, rotation, translation) in [
        (1.0e-6, -0.4, [3.0e-6, -2.0e-6]),
        (1.0, 0.7, [9.0, -4.0]),
        (1.0e6, 1.1, [-3.0e6, 5.0e6]),
    ] {
        let center_position = transformed(Point2::new(0.0, 0.0), scale, rotation, translation);
        let target = transformed(Point2::new(2.0, 0.0), scale, rotation, translation);
        let mut sketch = Sketch::new(scale).unwrap();
        let center = sketch.add_named_point("O", center_position).unwrap();
        let point = sketch
            .add_named_point("P", target + Point2::new(0.2 * scale, -0.1 * scale).coords)
            .unwrap();
        let circle = sketch
            .add_named_circle("circle", center, 2.0 * scale)
            .unwrap();
        sketch.add_fixed_point(center).unwrap();
        sketch
            .add_circle_radius(circle, 2.0 * scale, DimensionMode::Driving)
            .unwrap();
        let contact = sketch
            .add_point_on_circle(point, circle, rotation + 2.0 * PI)
            .unwrap();
        let result = solve(&mut sketch);
        assert_accepted(&result);
        let solved_point = result.geometry.point(point).unwrap();
        let solved_circle = result.geometry.circle(circle).unwrap();
        assert!(
            ((solved_point - solved_circle.center).norm() - 2.0 * scale).abs() / scale <= TOLERANCE
        );
        let ContactState::PointOnCircle { angle } = sketch.contact_state(contact).unwrap() else {
            panic!("wrong contact state")
        };
        assert_point(
            solved_circle.evaluate(angle).unwrap(),
            solved_point,
            scale,
            TOLERANCE,
        );
        assert!((angle - (rotation + 2.0 * PI)).abs() <= 0.2);
    }
}

#[test]
fn segment_pair_midpoint_and_symmetry_constraints_recover_and_transform() {
    for (scale, rotation, translation) in [
        (1.0e-6, -0.3, [2.0e-6, 7.0e-6]),
        (1.0, 0.8, [10.0, -6.0]),
        (1.0e6, 1.2, [-4.0e6, 3.0e6]),
    ] {
        let mut parallel = Sketch::new(scale).unwrap();
        let points: Vec<_> = [
            Point2::new(0.0, 0.0),
            Point2::new(2.0, 0.0),
            Point2::new(0.0, 1.0),
            Point2::new(1.5, 1.4),
        ]
        .into_iter()
        .map(|point| {
            parallel
                .add_point(transformed(point, scale, rotation, translation))
                .unwrap()
        })
        .collect();
        let first = parallel.add_segment(points[0], points[1]).unwrap();
        let second = parallel.add_segment(points[2], points[3]).unwrap();
        parallel.add_fixed_point(points[0]).unwrap();
        parallel.add_fixed_point(points[1]).unwrap();
        parallel.add_fixed_point(points[2]).unwrap();
        parallel.add_parallel(first, second).unwrap();
        parallel
            .add_segment_length(second, 2.0 * scale, DimensionMode::Driving)
            .unwrap();
        let result = solve(&mut parallel);
        assert_accepted(&result);
        assert_point(
            result.geometry.point(points[3]).unwrap(),
            transformed(Point2::new(2.0, 1.0), scale, rotation, translation),
            scale,
            2.0e-8,
        );

        let mut perpendicular = Sketch::new(scale).unwrap();
        let a = perpendicular
            .add_point(transformed(
                Point2::new(0.0, 0.0),
                scale,
                rotation,
                translation,
            ))
            .unwrap();
        let b = perpendicular
            .add_point(transformed(
                Point2::new(2.0, 0.0),
                scale,
                rotation,
                translation,
            ))
            .unwrap();
        let c = perpendicular
            .add_point(transformed(
                Point2::new(0.0, 0.0),
                scale,
                rotation,
                translation,
            ))
            .unwrap();
        let d = perpendicular
            .add_point(transformed(
                Point2::new(0.3, 1.8),
                scale,
                rotation,
                translation,
            ))
            .unwrap();
        let ab = perpendicular.add_segment(a, b).unwrap();
        let cd = perpendicular.add_segment(c, d).unwrap();
        perpendicular.add_fixed_point(a).unwrap();
        perpendicular.add_fixed_point(b).unwrap();
        perpendicular.add_fixed_point(c).unwrap();
        perpendicular.add_perpendicular(ab, cd).unwrap();
        perpendicular
            .add_segment_length(cd, 2.0 * scale, DimensionMode::Driving)
            .unwrap();
        let result = solve(&mut perpendicular);
        assert_accepted(&result);
        assert_point(
            result.geometry.point(d).unwrap(),
            transformed(Point2::new(0.0, 2.0), scale, rotation, translation),
            scale,
            2.0e-8,
        );
    }

    let mut equal = Sketch::new(3.0).unwrap();
    let a = equal.add_point(Point2::new(0.0, 0.0)).unwrap();
    let b = equal.add_point(Point2::new(3.0, 0.0)).unwrap();
    let c = equal.add_point(Point2::new(0.0, 2.0)).unwrap();
    let d = equal.add_point(Point2::new(1.0, 2.0)).unwrap();
    let ab = equal.add_segment(a, b).unwrap();
    let cd = equal.add_segment(c, d).unwrap();
    equal.add_fixed_point(a).unwrap();
    equal.add_fixed_point(b).unwrap();
    equal.add_fixed_point(c).unwrap();
    equal.add_horizontal(cd).unwrap();
    equal.add_equal_segment_length(ab, cd).unwrap();
    let result = solve(&mut equal);
    assert_accepted(&result);
    assert_point(
        result.geometry.point(d).unwrap(),
        Point2::new(3.0, 2.0),
        3.0,
        2.0e-9,
    );

    let mut midpoint = Sketch::new(4.0).unwrap();
    let a = midpoint.add_point(Point2::new(0.0, 0.0)).unwrap();
    let b = midpoint.add_point(Point2::new(4.0, 2.0)).unwrap();
    let p = midpoint.add_point(Point2::new(1.0, 0.0)).unwrap();
    let ab = midpoint.add_segment(a, b).unwrap();
    midpoint.add_fixed_point(a).unwrap();
    midpoint.add_fixed_point(b).unwrap();
    midpoint.add_midpoint(p, ab).unwrap();
    let result = solve(&mut midpoint);
    assert_accepted(&result);
    assert_point(
        result.geometry.point(p).unwrap(),
        Point2::new(2.0, 1.0),
        4.0,
        2.0e-9,
    );

    let mut symmetry = Sketch::new(3.0).unwrap();
    let l0 = symmetry.add_point(Point2::new(-2.0, 0.0)).unwrap();
    let l1 = symmetry.add_point(Point2::new(2.0, 0.0)).unwrap();
    let first = symmetry.add_point(Point2::new(1.0, 2.0)).unwrap();
    let second = symmetry.add_point(Point2::new(0.5, -1.0)).unwrap();
    let line = symmetry.add_segment(l0, l1).unwrap();
    symmetry.add_fixed_point(l0).unwrap();
    symmetry.add_fixed_point(l1).unwrap();
    symmetry.add_fixed_point(first).unwrap();
    symmetry
        .add_symmetric_about_line(first, second, line)
        .unwrap();
    let result = solve(&mut symmetry);
    assert_accepted(&result);
    assert_point(
        result.geometry.point(second).unwrap(),
        Point2::new(1.0, -2.0),
        3.0,
        2.0e-9,
    );
}

#[test]
fn line_circle_tangency_preserves_domain_side_and_contact_transactionally() {
    let mut sketch = Sketch::new(2.0).unwrap();
    let a = sketch.add_named_point("A", Point2::new(0.0, 0.0)).unwrap();
    let b = sketch.add_named_point("B", Point2::new(2.0, 0.0)).unwrap();
    let center = sketch.add_named_point("O", Point2::new(1.0, 1.0)).unwrap();
    let line = sketch.add_named_segment("AB", a, b).unwrap();
    let circle = sketch.add_named_circle("circle", center, 1.0).unwrap();
    sketch.add_fixed_point(a).unwrap();
    sketch.add_fixed_point(b).unwrap();
    sketch.add_fixed_point(center).unwrap();
    sketch
        .add_circle_radius(circle, 1.0, DimensionMode::Driving)
        .unwrap();
    let tangency = sketch
        .add_line_circle_tangency(
            line,
            circle,
            LineParameterDomain::BoundedSegment,
            LineSide::Left,
            0.35,
            -1.2,
        )
        .unwrap();
    let result = solve(&mut sketch);
    assert_accepted(&result);
    let ContactState::LineCircleTangency {
        line_parameter,
        circle_angle,
    } = sketch.contact_state(tangency).unwrap()
    else {
        panic!("wrong tangency state")
    };
    assert!((line_parameter - 0.5).abs() <= 2.0e-9);
    assert!((circle_angle + FRAC_PI_2).abs() <= 2.0e-9);

    for (scale, rotation, translation) in [
        (1.0e-6, -0.4, [3.0e-6, 5.0e-6]),
        (1.0, 0.8, [11.0, -7.0]),
        (1.0e6, 1.3, [-4.0e6, 2.0e6]),
    ] {
        let mut transformed_tangent = Sketch::new(scale).unwrap();
        let a = transformed_tangent
            .add_point(transformed(
                Point2::new(0.0, 0.0),
                scale,
                rotation,
                translation,
            ))
            .unwrap();
        let b = transformed_tangent
            .add_point(transformed(
                Point2::new(2.0, 0.0),
                scale,
                rotation,
                translation,
            ))
            .unwrap();
        let center = transformed_tangent
            .add_point(transformed(
                Point2::new(1.0, 1.0),
                scale,
                rotation,
                translation,
            ))
            .unwrap();
        let line = transformed_tangent.add_segment(a, b).unwrap();
        let circle = transformed_tangent.add_circle(center, scale).unwrap();
        transformed_tangent.add_fixed_point(a).unwrap();
        transformed_tangent.add_fixed_point(b).unwrap();
        transformed_tangent.add_fixed_point(center).unwrap();
        transformed_tangent
            .add_circle_radius(circle, scale, DimensionMode::Driving)
            .unwrap();
        let tangency = transformed_tangent
            .add_line_circle_tangency(
                line,
                circle,
                LineParameterDomain::BoundedSegment,
                LineSide::Left,
                0.45,
                rotation - FRAC_PI_2 + 0.1,
            )
            .unwrap();
        let result = solve(&mut transformed_tangent);
        assert_accepted(&result);
        let ContactState::LineCircleTangency {
            line_parameter,
            circle_angle,
        } = transformed_tangent.contact_state(tangency).unwrap()
        else {
            panic!("wrong transformed tangency state")
        };
        assert!((line_parameter - 0.5).abs() <= 3.0e-9);
        assert!((circle_angle - (rotation - FRAC_PI_2)).abs() <= 3.0e-9);
    }

    let mut escaped = Sketch::new(1.0).unwrap();
    let a = escaped.add_point(Point2::new(0.0, 0.0)).unwrap();
    let b = escaped.add_point(Point2::new(1.0, 0.0)).unwrap();
    let center = escaped.add_point(Point2::new(2.0, 1.0)).unwrap();
    let line = escaped.add_segment(a, b).unwrap();
    let circle = escaped.add_circle(center, 1.0).unwrap();
    escaped.add_fixed_point(a).unwrap();
    escaped.add_fixed_point(b).unwrap();
    escaped.add_fixed_point(center).unwrap();
    escaped
        .add_circle_radius(circle, 1.0, DimensionMode::Driving)
        .unwrap();
    let tangency = escaped
        .add_line_circle_tangency(
            line,
            circle,
            LineParameterDomain::BoundedSegment,
            LineSide::Left,
            0.5,
            -FRAC_PI_2,
        )
        .unwrap();
    let retained = escaped.geometry();
    let result = solve(&mut escaped);
    assert_eq!(
        result.rejection,
        Some(SolveRejection::ContactParameterOutOfDomain(tangency))
    );
    assert_eq!(result.geometry, retained);
    assert_eq!(
        escaped.contact_state(tangency).unwrap(),
        ContactState::LineCircleTangency {
            line_parameter: 0.5,
            circle_angle: -FRAC_PI_2,
        }
    );

    let mut wrong_side = Sketch::new(2.0).unwrap();
    let a = wrong_side.add_point(Point2::new(0.0, 0.0)).unwrap();
    let b = wrong_side.add_point(Point2::new(2.0, 0.0)).unwrap();
    let center = wrong_side.add_point(Point2::new(1.0, -1.0)).unwrap();
    let line = wrong_side.add_segment(a, b).unwrap();
    let circle = wrong_side.add_circle(center, 1.0).unwrap();
    wrong_side.add_fixed_point(a).unwrap();
    wrong_side.add_fixed_point(b).unwrap();
    wrong_side.add_fixed_point(center).unwrap();
    wrong_side
        .add_circle_radius(circle, 1.0, DimensionMode::Driving)
        .unwrap();
    let tangency = wrong_side
        .add_line_circle_tangency(
            line,
            circle,
            LineParameterDomain::SupportingLine,
            LineSide::Left,
            0.5,
            FRAC_PI_2,
        )
        .unwrap();
    let result = solve(&mut wrong_side);
    assert_eq!(
        result.rejection,
        Some(SolveRejection::LineSideFlipped(tangency))
    );
}

#[test]
fn equal_radii_clockwise_angles_and_center_direction_flips_are_explicit() {
    let mut equal = Sketch::new(2.0).unwrap();
    let a = equal.add_point(Point2::new(0.0, 0.0)).unwrap();
    let b = equal.add_point(Point2::new(4.0, 0.0)).unwrap();
    let first = equal.add_circle(a, 2.0).unwrap();
    let second = equal.add_circle(b, 0.8).unwrap();
    equal
        .add_circle_radius(first, 2.0, DimensionMode::Driving)
        .unwrap();
    equal.add_equal_circle_radius(first, second).unwrap();
    let result = solve(&mut equal);
    assert_accepted(&result);
    assert!((result.geometry.circle(second).unwrap().radius - 2.0).abs() <= TOLERANCE);

    let mut clockwise = Sketch::new(1.0).unwrap();
    let o = clockwise.add_point(Point2::new(0.0, 0.0)).unwrap();
    let x = clockwise.add_point(Point2::new(1.0, 0.0)).unwrap();
    let p = clockwise.add_point(Point2::new(0.2, -0.8)).unwrap();
    let first = clockwise.add_segment(o, x).unwrap();
    let second = clockwise.add_segment(o, p).unwrap();
    clockwise.add_fixed_point(o).unwrap();
    clockwise.add_fixed_point(x).unwrap();
    clockwise
        .add_segment_length(second, 1.0, DimensionMode::Driving)
        .unwrap();
    clockwise
        .add_oriented_angle(
            first,
            second,
            2.0 * PI + FRAC_PI_2,
            AngleOrientation::Clockwise,
            DimensionMode::Driving,
        )
        .unwrap();
    let result = solve(&mut clockwise);
    assert_accepted(&result);
    assert_point(
        result.geometry.point(p).unwrap(),
        Point2::new(0.0, -1.0),
        1.0,
        2.0e-9,
    );

    let (mut s3, ids) = tangent_circles().unwrap();
    s3.set_point_position(ids.center_b, Point2::new(-5.0, 0.0))
        .unwrap();
    let retained = s3.geometry();
    let result = solve(&mut s3);
    assert_eq!(result.core_report.termination, SolveTermination::Converged);
    assert_eq!(
        result.rejection,
        Some(SolveRejection::CenterDirectionFlipped(ids.tangency))
    );
    assert_eq!(result.geometry, retained);
    assert_eq!(
        s3.circle_tangency_mode(ids.tangency).unwrap(),
        CircleTangencyMode::External
    );
}

#[test]
fn radius_diameter_and_oriented_angle_driving_reference_modes_change_rows_dof_and_values() {
    let mut sketch = Sketch::new(2.0).unwrap();
    let center = sketch.add_point(Point2::new(0.0, 0.0)).unwrap();
    let circle = sketch.add_circle(center, 1.5).unwrap();
    let arc = sketch
        .add_arc(center, 2.5, 0.0, PI, ArcSweep::CounterClockwise)
        .unwrap();
    sketch.add_fixed_point(center).unwrap();
    let circle_radius = sketch
        .add_circle_radius(circle, 2.0, DimensionMode::Driving)
        .unwrap();
    let arc_diameter = sketch
        .add_arc_diameter(arc, 6.0, DimensionMode::Reference)
        .unwrap();
    let compiled = sketch
        .compile(SketchSolveRequest::default().without_previous_state_preferences())
        .unwrap();
    assert_eq!(compiled.point_variables().len(), 1);
    assert_eq!(compiled.circle_radius_variables().len(), 1);
    assert_eq!(compiled.arc_radius_variables().len(), 1);
    assert_eq!(compiled.problem().audit_rows().unwrap().len(), 3);
    let result = solve(&mut sketch);
    assert_accepted(&result);
    assert_eq!(result.core_report.local_degrees_of_freedom, 1);
    assert!((result.geometry.circle(circle).unwrap().radius - 2.0).abs() <= TOLERANCE);
    assert_eq!(result.reference_values[0].dimension_id, arc_diameter);
    assert!((result.reference_values[0].value - 5.0).abs() <= TOLERANCE);

    let source_order: Vec<_> = result
        .source_mappings
        .iter()
        .map(|mapping| mapping.source)
        .collect();
    sketch
        .set_dimension_mode(circle_radius, DimensionMode::Reference)
        .unwrap();
    sketch
        .set_dimension_mode(arc_diameter, DimensionMode::Driving)
        .unwrap();
    let toggled = solve(&mut sketch);
    assert_accepted(&toggled);
    assert_eq!(toggled.core_report.local_degrees_of_freedom, 1);
    assert!((toggled.geometry.arc(arc).unwrap().radius - 3.0).abs() <= TOLERANCE);
    assert_eq!(toggled.reference_values[0].dimension_id, circle_radius);
    assert!((toggled.reference_values[0].value - 2.0).abs() <= TOLERANCE);
    assert_eq!(
        toggled
            .source_mappings
            .iter()
            .map(|mapping| mapping.source)
            .collect::<Vec<_>>(),
        source_order
    );

    let mut angle = Sketch::new(1.0).unwrap();
    let o = angle.add_named_point("O", Point2::new(0.0, 0.0)).unwrap();
    let x = angle.add_named_point("X", Point2::new(1.0, 0.0)).unwrap();
    let p = angle.add_named_point("P", Point2::new(0.2, 0.8)).unwrap();
    let first = angle.add_named_segment("OX", o, x).unwrap();
    let second = angle.add_named_segment("OP", o, p).unwrap();
    angle.add_fixed_point(o).unwrap();
    angle.add_fixed_point(x).unwrap();
    angle
        .add_segment_length(second, 1.0, DimensionMode::Driving)
        .unwrap();
    let angle_dimension = angle
        .add_oriented_angle(
            first,
            second,
            FRAC_PI_2,
            AngleOrientation::CounterClockwise,
            DimensionMode::Driving,
        )
        .unwrap();
    let result = solve(&mut angle);
    assert_accepted(&result);
    assert_point(
        result.geometry.point(p).unwrap(),
        Point2::new(0.0, 1.0),
        1.0,
        2.0e-9,
    );
    angle
        .set_dimension_mode(angle_dimension, DimensionMode::Reference)
        .unwrap();
    let reference = solve(&mut angle);
    assert_accepted(&reference);
    assert_eq!(reference.reference_values[0].dimension_id, angle_dimension);
    assert!((reference.reference_values[0].value - FRAC_PI_2).abs() <= TOLERANCE);
    assert!(matches!(
        angle.dimension(angle_dimension).unwrap().kind(),
        DimensionKind::OrientedAngle {
            orientation: AngleOrientation::CounterClockwise,
            ..
        }
    ));
}

#[test]
fn all_new_executable_rows_have_analytic_jacobians_and_deterministic_mappings() {
    let mut sketch = Sketch::new(4.0).unwrap();
    let p0 = sketch.add_named_point("P0", Point2::new(0.0, 0.0)).unwrap();
    let p1 = sketch.add_named_point("P1", Point2::new(4.0, 0.0)).unwrap();
    let p2 = sketch.add_named_point("P2", Point2::new(0.0, 2.0)).unwrap();
    let p3 = sketch.add_named_point("P3", Point2::new(4.0, 2.5)).unwrap();
    let p4 = sketch.add_named_point("P4", Point2::new(2.0, 1.0)).unwrap();
    let p5 = sketch
        .add_named_point("P5", Point2::new(3.0, -1.0))
        .unwrap();
    let first = sketch.add_named_segment("first", p0, p1).unwrap();
    let second = sketch.add_named_segment("second", p2, p3).unwrap();
    let circle_a = sketch.add_named_circle("circle A", p2, 2.0).unwrap();
    let circle_b = sketch.add_named_circle("circle B", p3, 1.0).unwrap();
    let arc = sketch
        .add_named_arc("arc", p0, 3.0, 0.1, 2.2, ArcSweep::CounterClockwise)
        .unwrap();
    sketch
        .add_point_on_line(p4, first, LineParameterDomain::SupportingLine, 0.4)
        .unwrap();
    sketch.add_point_on_circle(p5, circle_a, -0.8).unwrap();
    sketch.add_point_on_arc(p4, arc, 0.45).unwrap();
    sketch.add_parallel(first, second).unwrap();
    sketch.add_perpendicular(first, second).unwrap();
    sketch.add_equal_segment_length(first, second).unwrap();
    sketch.add_equal_circle_radius(circle_a, circle_b).unwrap();
    sketch.add_midpoint(p4, first).unwrap();
    sketch.add_symmetric_about_line(p4, p5, first).unwrap();
    sketch
        .add_line_circle_tangency(
            first,
            circle_a,
            LineParameterDomain::SupportingLine,
            LineSide::Left,
            0.5,
            -FRAC_PI_2,
        )
        .unwrap();
    sketch
        .add_circle_circle_tangency(
            circle_a,
            circle_b,
            CircleTangencyMode::External,
            CenterDirectionBranch::new([1.0, 0.2]).unwrap(),
        )
        .unwrap();
    sketch
        .add_circle_diameter(circle_a, 4.0, DimensionMode::Driving)
        .unwrap();
    sketch
        .add_arc_radius(arc, 3.0, DimensionMode::Driving)
        .unwrap();
    sketch
        .add_oriented_angle(
            first,
            second,
            0.2,
            AngleOrientation::CounterClockwise,
            DimensionMode::Driving,
        )
        .unwrap();

    let request = SketchSolveRequest::default().without_previous_state_preferences();
    let first_compile = sketch.compile(request).unwrap();
    let second_compile = sketch.compile(request).unwrap();
    assert_eq!(
        first_compile.point_variables(),
        second_compile.point_variables()
    );
    assert_eq!(
        first_compile.circle_radius_variables(),
        second_compile.circle_radius_variables()
    );
    assert_eq!(
        first_compile.arc_radius_variables(),
        second_compile.arc_radius_variables()
    );
    assert_eq!(
        first_compile.latent_variables(),
        second_compile.latent_variables()
    );
    assert_eq!(
        first_compile.source_mappings(),
        second_compile.source_mappings()
    );
    assert_eq!(first_compile.latent_variables().len(), 5);
    assert_eq!(
        first_compile
            .latent_variables()
            .iter()
            .map(|mapping| mapping.role)
            .collect::<Vec<_>>(),
        vec![
            LatentVariableRole::LineParameter,
            LatentVariableRole::CircleAngle,
            LatentVariableRole::ArcSpanParameter,
            LatentVariableRole::LineParameter,
            LatentVariableRole::CircleAngle,
        ]
    );
    let report = first_compile.problem().check_jacobians(1.0e-5).unwrap();
    assert!(
        report.all_within(1.0e-6),
        "maximum relative error={:e}: {report:#?}",
        report.max_relative_error()
    );
    assert!(
        first_compile
            .problem()
            .audit_rows()
            .unwrap()
            .iter()
            .all(|row| {
                !row.template.trim().is_empty()
                    && !row.bindings.is_empty()
                    && row.scale.is_finite()
                    && row.scale > 0.0
            })
    );
    assert!(
        first_compile
            .source_mappings()
            .iter()
            .all(|mapping| matches!(
                mapping.source,
                SketchSource::Constraint(_) | SketchSource::Dimension(_)
            ))
    );
}

#[test]
fn zero_segments_bad_branches_nonfinite_state_and_stale_curve_ids_are_explicit() {
    assert!(matches!(
        CenterDirectionBranch::new([0.0, 0.0]),
        Err(SketchError::InvalidDirectionBranch)
    ));
    assert!(matches!(
        CenterDirectionBranch::new([f64::NAN, 1.0]),
        Err(SketchError::InvalidDirectionBranch)
    ));

    let mut sketch = Sketch::new(1.0).unwrap();
    let a = sketch.add_point(Point2::new(0.0, 0.0)).unwrap();
    let b = sketch.add_point(Point2::new(1.0, 0.0)).unwrap();
    let c = sketch.add_point(Point2::new(0.0, 1.0)).unwrap();
    let d = sketch.add_point(Point2::new(1.0, 1.0)).unwrap();
    let first = sketch.add_segment(a, b).unwrap();
    let second = sketch.add_segment(c, d).unwrap();
    sketch.add_parallel(first, second).unwrap();
    sketch.set_point_position(b, Point2::new(0.0, 0.0)).unwrap();
    let retained = sketch.geometry();
    assert!(matches!(
        sketch.compile(SketchSolveRequest::default()),
        Err(SketchError::InvalidSegmentEntity(id)) if id == first
    ));
    assert!(matches!(
        sketch.solve(SketchSolveRequest::default(), SolverConfig::default()),
        Err(SketchError::InvalidSegmentEntity(id)) if id == first
    ));
    assert_eq!(sketch.geometry(), retained);

    let mut stale = Sketch::new(1.0).unwrap();
    let center = stale.add_point(Point2::new(0.0, 0.0)).unwrap();
    let circle = stale.add_circle(center, 1.0).unwrap();
    stale.remove_circle(circle).unwrap();
    assert!(matches!(
        stale.add_circle_radius(circle, 2.0, DimensionMode::Driving),
        Err(SketchError::UnknownCircle(id)) if id == circle
    ));
    let arc = stale
        .add_arc(center, 1.0, 0.0, PI, ArcSweep::CounterClockwise)
        .unwrap();
    stale.remove_arc(arc).unwrap();
    let point = stale.add_point(Point2::new(1.0, 0.0)).unwrap();
    assert!(matches!(
        stale.add_point_on_arc(point, arc, 0.5),
        Err(SketchError::UnknownArc(id)) if id == arc
    ));
    assert!(matches!(
        stale.add_point_on_circle(point, circle, f64::INFINITY),
        Err(SketchError::UnknownCircle(id)) if id == circle
    ));
}
