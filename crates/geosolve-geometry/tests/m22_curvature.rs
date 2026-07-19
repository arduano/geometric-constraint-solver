use std::f64::consts::FRAC_1_SQRT_2;

use geosolve_geometry::{
    BSplineKnotSide, CurveDifferentialError, CurveJet2, CurveParameterDomain, NurbsCurve2, Point2,
    Vector2, circle_jet, circular_arc_jet, line_jet,
};

#[test]
fn circles_and_canonical_nurbs_report_exact_curvature_and_radius() {
    for scale in [1.0e-6, 1.0, 1.0e6] {
        let radius = 2.5 * scale;
        for angle in [-2.0, -0.3, 0.0, 1.7] {
            let differential = circle_jet(Point2::new(3.0 * scale, -scale), radius, angle)
                .unwrap()
                .differential()
                .unwrap();
            assert_close(
                differential.signed_curvature,
                radius.recip(),
                radius.recip(),
            );
            assert_close(
                differential.unsigned_curvature(),
                radius.recip(),
                radius.recip(),
            );
            assert_close(differential.osculating_radius().unwrap(), radius, radius);
            assert_close(differential.unit_tangent.norm(), 1.0, 1.0);
            assert_close(differential.left_normal.norm(), 1.0, 1.0);
            assert_close(
                differential.unit_tangent.dot(&differential.left_normal),
                0.0,
                1.0,
            );
        }

        let nurbs = NurbsCurve2::try_clamped(
            2,
            vec![
                Point2::new(radius, 0.0),
                Point2::new(radius, radius),
                Point2::new(0.0, radius),
            ],
            vec![1.0, FRAC_1_SQRT_2, 1.0],
            vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
        )
        .unwrap();
        for parameter in [0.1, 0.37, 0.8] {
            let differential = nurbs
                .jet_on_span(nurbs.basis().spans()[0].index(), parameter)
                .unwrap()
                .differential()
                .unwrap();
            assert_close(
                differential.signed_curvature,
                radius.recip(),
                radius.recip(),
            );
            assert_close(differential.osculating_radius().unwrap(), radius, radius);
        }
    }
}

#[test]
fn parameter_reversal_flips_signed_curvature_but_not_curvature_vector() {
    let forward = circle_jet(Point2::origin(), 3.0, 0.7).unwrap();
    let reversed = CurveJet2 {
        first_derivative: -forward.first_derivative,
        second_derivative: forward.second_derivative,
        third_derivative: -forward.third_derivative,
        ..forward
    };
    let forward = forward.differential().unwrap();
    let reversed = reversed.differential().unwrap();
    assert_vector(reversed.unit_tangent, -forward.unit_tangent, 1.0);
    assert_vector(reversed.left_normal, -forward.left_normal, 1.0);
    assert_close(reversed.signed_curvature, -forward.signed_curvature, 1.0);
    assert_close(
        reversed.unsigned_curvature(),
        forward.unsigned_curvature(),
        1.0,
    );
    assert_vector(reversed.curvature_vector(), forward.curvature_vector(), 1.0);
}

#[test]
fn directed_arc_orientation_controls_curvature_sign() {
    let positive = circular_arc_jet(Point2::origin(), 4.0, -0.3, 2.0, 0.4)
        .unwrap()
        .differential()
        .unwrap();
    let negative = circular_arc_jet(Point2::origin(), 4.0, 1.7, -2.0, 0.6)
        .unwrap()
        .differential()
        .unwrap();
    assert_close(positive.signed_curvature, 0.25, 1.0);
    assert_close(negative.signed_curvature, -0.25, 1.0);
    assert_close(positive.unsigned_curvature(), 0.25, 1.0);
    assert_close(negative.unsigned_curvature(), 0.25, 1.0);
}

#[test]
fn similarities_and_reflections_transform_curvature_truthfully() {
    let base = NurbsCurve2::try_clamped(
        3,
        vec![
            Point2::new(-1.0, 0.5),
            Point2::new(0.0, 2.0),
            Point2::new(1.2, -0.3),
            Point2::new(2.0, 1.4),
        ],
        vec![1.0, 0.4, 1.8, 0.75],
        vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0],
    )
    .unwrap();
    let span = base.basis().spans()[0].index();
    let base_differential = base
        .jet_on_span(span, 0.37)
        .unwrap()
        .differential()
        .unwrap();
    for scale in [1.0e-6, 3.0, 1.0e6] {
        let cosine = 0.6;
        let sine = 0.8;
        let mapped = NurbsCurve2::try_clamped(
            3,
            base.controls()
                .iter()
                .map(|point| {
                    Point2::new(
                        scale * (cosine * point.x - sine * point.y + 7.0),
                        scale * (sine * point.x + cosine * point.y - 4.0),
                    )
                })
                .collect(),
            base.weights().to_vec(),
            base.basis().knots().to_vec(),
        )
        .unwrap();
        let differential = mapped
            .jet_on_span(span, 0.37)
            .unwrap()
            .differential()
            .unwrap();
        assert_close(
            differential.signed_curvature,
            base_differential.signed_curvature / scale,
            base_differential.signed_curvature.abs() / scale,
        );
    }

    let reflected = NurbsCurve2::try_clamped(
        3,
        base.controls()
            .iter()
            .map(|point| Point2::new(-point.x, point.y))
            .collect(),
        base.weights().to_vec(),
        base.basis().knots().to_vec(),
    )
    .unwrap();
    assert_close(
        reflected
            .jet_on_span(span, 0.37)
            .unwrap()
            .differential()
            .unwrap()
            .signed_curvature,
        -base_differential.signed_curvature,
        1.0,
    );
}

#[test]
fn line_has_zero_curvature_and_no_finite_osculating_radius() {
    let differential = line_jet(
        Point2::new(-2.0, 1.0),
        Point2::new(3.0, 4.0),
        CurveParameterDomain::SupportingLine,
        0.2,
    )
    .unwrap()
    .differential()
    .unwrap();
    assert_close(differential.signed_curvature, 0.0, 1.0);
    assert_close(differential.unsigned_curvature(), 0.0, 1.0);
    assert_eq!(differential.curvature_vector(), Vector2::zeros());
    assert_eq!(
        differential.osculating_radius(),
        Err(CurveDifferentialError::UndefinedOsculatingRadius)
    );
}

#[test]
#[allow(clippy::too_many_lines)]
fn invalid_differential_geometry_never_returns_nonfinite_success() {
    let zero_speed = CurveJet2 {
        position: Point2::origin(),
        first_derivative: Vector2::zeros(),
        second_derivative: Vector2::new(1.0, 0.0),
        third_derivative: Vector2::zeros(),
        domain: CurveParameterDomain::SupportingLine,
    };
    assert_eq!(
        zero_speed.differential(),
        Err(CurveDifferentialError::ZeroSpeed)
    );
    let nonfinite = CurveJet2 {
        first_derivative: Vector2::new(f64::INFINITY, 0.0),
        ..zero_speed
    };
    assert_eq!(
        nonfinite.differential(),
        Err(CurveDifferentialError::NonFiniteJet)
    );
    let unrepresentable = CurveJet2 {
        first_derivative: Vector2::new(f64::from_bits(1), 0.0),
        second_derivative: Vector2::new(0.0, f64::MAX),
        ..zero_speed
    };
    assert_eq!(
        unrepresentable.differential(),
        Err(CurveDifferentialError::UnrepresentableCurvature)
    );
    let underflowed = CurveJet2 {
        first_derivative: Vector2::new(1.0e200, 0.0),
        second_derivative: Vector2::new(0.0, 1.0e-200),
        ..zero_speed
    };
    assert_eq!(
        underflowed.differential(),
        Err(CurveDifferentialError::UnrepresentableCurvature)
    );
    let near_parallel = CurveJet2 {
        first_derivative: Vector2::new(1.0 + 2.0f64.powi(-27), 1.0),
        second_derivative: Vector2::new(1.0, 1.0 - 2.0f64.powi(-27)),
        ..zero_speed
    }
    .differential()
    .unwrap();
    assert!(near_parallel.signed_curvature < 0.0);
    assert_ne!(near_parallel.signed_curvature.to_bits(), 0.0f64.to_bits());

    let extreme_straight = CurveJet2 {
        first_derivative: Vector2::new(1.0e-200, 0.0),
        second_derivative: Vector2::new(1.0e200, 0.0),
        ..zero_speed
    }
    .differential()
    .unwrap();
    assert_eq!(
        extreme_straight.signed_curvature.to_bits(),
        0.0f64.to_bits()
    );

    let mixed_scale = CurveJet2 {
        first_derivative: Vector2::new(1.0e-100, 0.0),
        second_derivative: Vector2::new(1.0e200, 1.0e-224),
        ..zero_speed
    }
    .differential()
    .unwrap();
    assert_close(mixed_scale.signed_curvature, 1.0e-24, 1.0e-24);

    let subnormal_products = CurveJet2 {
        first_derivative: Vector2::new(1.0e-100 * (1.0 + 2.0f64.powi(-27)), 1.0e-100),
        second_derivative: Vector2::new(1.0e-223, 1.0e-223 * (1.0 - 2.0f64.powi(-27))),
        ..zero_speed
    }
    .differential()
    .unwrap();
    assert_ne!(
        subnormal_products.signed_curvature.to_bits(),
        0.0f64.to_bits()
    );

    let overflowed_projection = CurveJet2 {
        first_derivative: Vector2::new(1.0e308, -1.0e308),
        second_derivative: Vector2::new(f64::MAX, f64::MAX),
        ..zero_speed
    }
    .differential()
    .unwrap();
    assert!(overflowed_projection.signed_curvature.is_finite());
    assert!(overflowed_projection.signed_curvature > 0.0);

    let subnormal_projection = CurveJet2 {
        first_derivative: Vector2::new(1.0e-100, 1.0e-100),
        second_derivative: Vector2::new(0.0, f64::from_bits(1)),
        ..zero_speed
    }
    .differential()
    .unwrap();
    assert!(subnormal_projection.signed_curvature.is_finite());
    assert!(subnormal_projection.signed_curvature > 0.0);

    let periodic = NurbsCurve2::try_periodic(
        2,
        vec![
            Point2::new(0.0, 0.0),
            Point2::new(1.0, 0.0),
            Point2::new(1.0, 1.0),
        ],
        vec![1.0, 0.7, 1.2],
        vec![0.0, 1.0, 2.0, 3.0],
    )
    .unwrap();
    assert!(
        periodic
            .jet_at(0.0, BSplineKnotSide::Right)
            .unwrap()
            .differential()
            .is_ok()
    );
}

fn assert_vector(actual: Vector2<f64>, expected: Vector2<f64>, scale: f64) {
    let error = (actual - expected).norm();
    let denominator = actual.norm().max(expected.norm()).max(scale * 1.0e-10);
    assert!(
        error / denominator <= 1.0e-9,
        "actual={actual:?}, expected={expected:?}, relative_error={}",
        error / denominator
    );
}

fn assert_close(actual: f64, expected: f64, scale: f64) {
    let error = (actual - expected).abs();
    assert!(
        error <= 2.0e-9 * scale.abs().max(1.0e-12),
        "actual={actual}, expected={expected}, error={error}"
    );
}
