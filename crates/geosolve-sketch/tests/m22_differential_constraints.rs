use std::f64::consts::FRAC_1_SQRT_2;

use geosolve_core::{HardValidity, SolverConfig};
use geosolve_geometry::{BSplineForm, Point2};
use geosolve_sketch::{
    CurveContactNeighborhood, CurveContinuity, CurveCurvatureRelation, CurveDirectionRelation,
    CurveMeasurementKind, CurveNormalSide, DimensionMode, Sketch, SketchCurve, SketchCurveContact,
    SketchError, SketchSolveRequest,
};

#[test]
fn nurbs_curvature_and_normal_constraints_validate_all_active_derivatives() {
    for scale in [1.0e-6, 1.0, 1.0e6] {
        let mut sketch = Sketch::new(scale).unwrap();
        let controls = [
            Point2::new(scale, 0.0),
            Point2::new(scale, scale),
            Point2::new(0.0, scale),
        ]
        .map(|point| sketch.add_point(point).unwrap());
        let nurbs = sketch
            .add_named_nurbs(
                "quarter circle NURBS",
                BSplineForm::Clamped,
                2,
                controls.to_vec(),
                vec![1.0, FRAC_1_SQRT_2, 1.0],
                0,
                vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
            )
            .unwrap();
        let span = sketch.nurbs(nurbs).unwrap().basis().spans()[0].index();
        let parameter = 0.37;
        let contact = SketchCurveContact {
            curve: SketchCurve::Nurbs { nurbs, span },
            parameter,
            neighborhood: CurveContactNeighborhood::Local {
                lower: 0.1,
                upper: 0.9,
            },
        };
        let jet = sketch.evaluate_nurbs(nurbs, span, parameter).unwrap();
        let differential = jet.differential().unwrap();
        assert_relative(
            sketch
                .measure_curve(contact, CurveMeasurementKind::SignedCurvature)
                .unwrap(),
            scale.recip(),
        );
        assert_relative(
            sketch
                .measure_curve(contact, CurveMeasurementKind::UnsignedCurvature)
                .unwrap(),
            scale.recip(),
        );
        assert_relative(
            sketch
                .measure_curve(contact, CurveMeasurementKind::OsculatingRadius)
                .unwrap(),
            scale,
        );

        let center = sketch.add_point(Point2::origin()).unwrap();
        let circle = sketch.add_circle(center, scale).unwrap();
        sketch
            .add_circle_radius(circle, scale, DimensionMode::Driving)
            .unwrap();
        let circle_contact = SketchCurveContact {
            curve: SketchCurve::Circle(circle),
            parameter: jet.position.y.atan2(jet.position.x),
            neighborhood: CurveContactNeighborhood::Interior,
        };
        sketch
            .add_equal_curvature(contact, circle_contact, CurveCurvatureRelation::Signed)
            .unwrap();

        let line_start = sketch.add_point(jet.position).unwrap();
        let line_end = sketch
            .add_point(jet.position + differential.left_normal * scale)
            .unwrap();
        let normal = sketch.add_segment(line_start, line_end).unwrap();
        sketch
            .add_curve_direction(
                normal,
                contact,
                CurveDirectionRelation::Normal(CurveNormalSide::Left),
            )
            .unwrap();
        for point in controls.into_iter().chain([center, line_start, line_end]) {
            sketch.add_fixed_point(point).unwrap();
        }

        let compiled = sketch.compile(SketchSolveRequest::default()).unwrap();
        let jacobians = compiled.problem().check_jacobians(1.0e-6).unwrap();
        assert_jacobians(&jacobians);
        let solved = sketch
            .solve(SketchSolveRequest::default(), SolverConfig::default())
            .unwrap();
        assert!(solved.accepted(), "{solved:#?}");
        assert_eq!(solved.core_report.hard_validity, HardValidity::Valid);
        assert!(solved.acceptance_hard_residual_max.unwrap() <= 1.0e-9);
    }
}

#[test]
fn g2_and_rate_explicit_parametric_c2_are_distinct_and_differentiable() {
    for scale in [1.0e-6, 1.0, 1.0e6] {
        let mut sketch = Sketch::new(scale).unwrap();
        let first_controls = [
            Point2::new(-scale, scale),
            Point2::new(-0.5 * scale, 0.0),
            Point2::origin(),
        ]
        .map(|point| sketch.add_point(point).unwrap());
        let second_controls = [
            first_controls[2],
            sketch.add_point(Point2::new(scale, 0.0)).unwrap(),
            sketch
                .add_point(Point2::new(2.0 * scale, 4.0 * scale))
                .unwrap(),
        ];
        let first = sketch
            .add_quadratic_bezier("incoming parabola", first_controls)
            .unwrap();
        let second = sketch
            .add_quadratic_bezier("outgoing reparameterized parabola", second_controls)
            .unwrap();
        let first_contact = SketchCurveContact {
            curve: SketchCurve::Bezier(first),
            parameter: 1.0,
            neighborhood: CurveContactNeighborhood::End,
        };
        let second_contact = SketchCurveContact {
            curve: SketchCurve::Bezier(second),
            parameter: 0.0,
            neighborhood: CurveContactNeighborhood::Start,
        };
        sketch
            .add_endpoint_continuity(first_contact, second_contact, CurveContinuity::G2)
            .unwrap();
        sketch
            .add_endpoint_continuity(
                first_contact,
                second_contact,
                CurveContinuity::ParametricC2 {
                    first_rate: 2.0,
                    second_rate: 1.0,
                },
            )
            .unwrap();
        for point in [
            first_controls[0],
            first_controls[1],
            first_controls[2],
            second_controls[1],
            second_controls[2],
        ] {
            sketch.add_fixed_point(point).unwrap();
        }

        let first_jet = sketch.evaluate_bezier(first, 1.0).unwrap();
        let second_jet = sketch.evaluate_bezier(second, 0.0).unwrap();
        assert!((first_jet.first_derivative - second_jet.first_derivative).norm() > 0.5 * scale);
        assert_relative(
            first_jet.differential().unwrap().signed_curvature,
            second_jet.differential().unwrap().signed_curvature,
        );

        let compiled = sketch.compile(SketchSolveRequest::default()).unwrap();
        let jacobians = compiled.problem().check_jacobians(1.0e-6).unwrap();
        assert_jacobians(&jacobians);
        let solved = sketch
            .solve(SketchSolveRequest::default(), SolverConfig::default())
            .unwrap();
        assert!(solved.accepted(), "{solved:#?}");
        assert_eq!(solved.core_report.hard_validity, HardValidity::Valid);
        assert!(solved.acceptance_hard_residual_max.unwrap() <= 1.0e-9);
    }
}

#[test]
fn invalid_continuity_and_zero_curvature_measurements_are_typed() {
    let mut sketch = Sketch::new(1.0).unwrap();
    let first = sketch.add_point(Point2::origin()).unwrap();
    let second = sketch.add_point(Point2::new(1.0, 0.0)).unwrap();
    let line = sketch.add_segment(first, second).unwrap();
    let interior = SketchCurveContact {
        curve: SketchCurve::Line {
            segment: line,
            domain: geosolve_sketch::LineParameterDomain::BoundedSegment,
        },
        parameter: 0.5,
        neighborhood: CurveContactNeighborhood::Interior,
    };
    assert_eq!(
        sketch.add_endpoint_continuity(interior, interior, CurveContinuity::G0),
        Err(SketchError::InvalidContinuityEndpoint)
    );
    let start = SketchCurveContact {
        parameter: 0.0,
        neighborhood: CurveContactNeighborhood::Start,
        ..interior
    };
    let end = SketchCurveContact {
        parameter: 1.0,
        neighborhood: CurveContactNeighborhood::End,
        ..interior
    };
    assert_eq!(
        sketch.add_endpoint_continuity(
            start,
            end,
            CurveContinuity::ParametricC2 {
                first_rate: 0.0,
                second_rate: 1.0,
            }
        ),
        Err(SketchError::InvalidContinuityRate)
    );
    assert!(matches!(
        sketch.measure_curve(interior, CurveMeasurementKind::OsculatingRadius),
        Err(SketchError::InvalidCurveDifferential(
            geosolve_geometry::CurveDifferentialError::UndefinedOsculatingRadius
        ))
    ));
}

fn assert_relative(actual: f64, expected: f64) {
    let error = (actual - expected).abs();
    let scale = actual.abs().max(expected.abs()).max(1.0e-12);
    assert!(
        error / scale <= 1.0e-8,
        "actual={actual}, expected={expected}, relative={}",
        error / scale
    );
}

fn assert_jacobians(report: &geosolve_core::JacobianCheckReport) {
    assert!(
        report.blocks.iter().all(|block| {
            block.max_relative_error <= 1.0e-6 || block.max_absolute_error <= 1.0e-8
        }),
        "{report:#?}"
    );
}
