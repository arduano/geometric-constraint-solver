use geosolve_core::SolverConfig;
use geosolve_geometry::{BSplineForm, DirectedParameterTrim, HyperbolaBranch, Point2, Vector2};
use geosolve_sketch::{
    ArcSweep, SketchSolveRequest, SolvedConicKind, conflicting_rectangle, underconstrained_triangle,
};

#[test]
fn finite_conflicting_rectangle_exposes_non_authoritative_attempted_geometry() {
    let (mut sketch, _) = conflicting_rectangle().unwrap();
    let controls = [
        sketch.add_point(Point2::new(10.0, 0.0)).unwrap(),
        sketch.add_point(Point2::new(12.0, 0.0)).unwrap(),
        sketch.add_point(Point2::new(10.0, 1.0)).unwrap(),
    ];
    sketch.add_circle(controls[0], 1.0).unwrap();
    sketch
        .add_arc(controls[1], 0.5, 0.0, 1.0, ArcSweep::CounterClockwise)
        .unwrap();
    sketch.add_ellipse(controls[0], controls[1], 0.5).unwrap();
    sketch
        .add_elliptical_arc(controls[0], controls[1], 0.5, 0.1, 1.5)
        .unwrap();
    sketch
        .add_rational_quadratic(controls[0], Vector2::new(0.5, 0.5), 0.75, controls[1])
        .unwrap();
    let trim = DirectedParameterTrim::try_new(-1.0, 1.0).unwrap();
    sketch
        .add_parabola_segment(controls[0], controls[2], trim)
        .unwrap();
    sketch
        .add_hyperbola_segment(
            controls[0],
            controls[1],
            0.5,
            HyperbolaBranch::Positive,
            trim,
        )
        .unwrap();
    sketch
        .add_named_nurbs(
            "preview NURBS",
            BSplineForm::Clamped,
            2,
            controls.to_vec(),
            vec![1.0; 3],
            0,
            vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
        )
        .unwrap();
    let retained = sketch.geometry();

    let result = sketch
        .solve(
            SketchSolveRequest::default().without_previous_state_preferences(),
            SolverConfig::default(),
        )
        .unwrap();

    assert!(!result.accepted());
    assert_eq!(result.geometry, retained);
    assert_eq!(sketch.geometry(), retained);
    let attempted = result
        .attempted_geometry
        .as_ref()
        .expect("finite conflicting solve should expose its attempted geometry");
    assert_ne!(attempted, &retained);
    assert_eq!(attempted.points.len(), retained.points.len());
    assert!(
        attempted
            .points
            .iter()
            .all(|point| { point.position.x.is_finite() && point.position.y.is_finite() })
    );
    assert_eq!(attempted.circles.len(), 1);
    assert_eq!(attempted.arcs.len(), 1);
    assert_eq!(attempted.conics.len(), 5);
    assert_eq!(attempted.nurbs.len(), 1);
    assert!(attempted.circles[0].radius.is_finite());
    assert!(attempted.arcs[0].radius.is_finite());
    assert!(attempted.arcs[0].start_angle.is_finite());
    assert!(attempted.arcs[0].end_angle.is_finite());
    for conic in &attempted.conics {
        match conic.kind {
            SolvedConicKind::Ellipse {
                minor_axis_ratio, ..
            }
            | SolvedConicKind::EllipticalArc {
                minor_axis_ratio, ..
            } => assert!(minor_axis_ratio.is_finite()),
            SolvedConicKind::RationalQuadratic {
                weighted_middle,
                middle_weight,
                ..
            } => {
                assert!(weighted_middle.iter().all(|value| value.is_finite()));
                assert!(middle_weight.is_finite());
            }
            SolvedConicKind::ParabolaSegment { trim, .. }
            | SolvedConicKind::HyperbolaSegment { trim, .. } => {
                assert!(trim.start().is_finite());
                assert!(trim.end().is_finite());
            }
        }
    }
    assert!(
        attempted.nurbs[0]
            .weights
            .iter()
            .all(|weight| weight.is_finite())
    );
}

#[test]
fn accepted_solve_attempted_geometry_equals_retained_geometry() {
    let (mut sketch, _) = underconstrained_triangle().unwrap();
    let result = sketch
        .solve(SketchSolveRequest::default(), SolverConfig::default())
        .unwrap();

    assert!(result.accepted(), "{:#?}", result.rejection);
    assert_eq!(result.attempted_geometry.as_ref(), Some(&result.geometry));
    assert_eq!(sketch.geometry(), result.geometry);
}
