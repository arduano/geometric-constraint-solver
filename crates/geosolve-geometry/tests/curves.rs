use geosolve_geometry::{
    CurveEvaluationError, CurveJet2, CurveParameterDomain, CurveParameterError,
    CurveRegularityError, Point2, Vector2, circle_jet, circular_arc_jet, cubic_bezier_jet,
    line_jet, quadratic_bezier_jet,
};

const TOLERANCE: f64 = 1.0e-9;

fn assert_vector(actual: [f64; 2], expected: [f64; 2], scale: f64) {
    assert!((actual[0] - expected[0]).abs() <= TOLERANCE * scale.max(1.0));
    assert!((actual[1] - expected[1]).abs() <= TOLERANCE * scale.max(1.0));
}

#[test]
fn exact_jets_cover_every_alpha_curve_family() {
    let line = line_jet(
        Point2::new(1.0, 2.0),
        Point2::new(5.0, 4.0),
        CurveParameterDomain::SupportingLine,
        0.25,
    )
    .unwrap();
    assert_vector(line.position.coords.into(), [2.0, 2.5], 1.0);
    assert_vector(line.first_derivative.into(), [4.0, 2.0], 1.0);
    assert_vector(line.second_derivative.into(), [0.0, 0.0], 1.0);
    assert_vector(line.third_derivative.into(), [0.0, 0.0], 1.0);

    let circle = circle_jet(Point2::new(1.0, 2.0), 3.0, 0.0).unwrap();
    assert_vector(circle.position.coords.into(), [4.0, 2.0], 1.0);
    assert_vector(circle.first_derivative.into(), [0.0, 3.0], 1.0);
    assert_vector(circle.second_derivative.into(), [-3.0, 0.0], 1.0);
    assert_vector(circle.third_derivative.into(), [0.0, -3.0], 1.0);

    let arc = circular_arc_jet(
        Point2::origin(),
        2.0,
        0.0,
        -std::f64::consts::FRAC_PI_2,
        0.0,
    )
    .unwrap();
    assert_vector(arc.position.coords.into(), [2.0, 0.0], 1.0);
    assert_vector(
        arc.first_derivative.into(),
        [0.0, -std::f64::consts::PI],
        1.0,
    );
    assert_vector(
        arc.second_derivative.into(),
        [-0.5 * std::f64::consts::PI.powi(2), 0.0],
        1.0,
    );
    assert_vector(
        arc.third_derivative.into(),
        [0.0, 0.25 * std::f64::consts::PI.powi(3)],
        1.0,
    );
    assert_eq!(
        arc.domain,
        CurveParameterDomain::Bounded {
            lower: 0.0,
            upper: 1.0
        }
    );

    let quadratic = quadratic_bezier_jet(
        [
            Point2::new(0.0, 0.0),
            Point2::new(1.0, 1.0),
            Point2::new(2.0, 0.0),
        ],
        0.5,
    )
    .unwrap();
    assert_vector(quadratic.position.coords.into(), [1.0, 0.5], 1.0);
    assert_vector(quadratic.first_derivative.into(), [2.0, 0.0], 1.0);
    assert_vector(quadratic.second_derivative.into(), [0.0, -4.0], 1.0);
    assert_vector(quadratic.third_derivative.into(), [0.0, 0.0], 1.0);

    let cubic = cubic_bezier_jet(
        [
            Point2::new(0.0, 0.0),
            Point2::new(1.0, 0.0),
            Point2::new(2.0, 1.0),
            Point2::new(3.0, 1.0),
        ],
        0.5,
    )
    .unwrap();
    assert_vector(cubic.position.coords.into(), [1.5, 0.5], 1.0);
    assert_vector(cubic.first_derivative.into(), [3.0, 1.5], 1.0);
    assert_vector(cubic.second_derivative.into(), [0.0, 0.0], 1.0);
    assert_vector(cubic.third_derivative.into(), [0.0, -12.0], 1.0);
}

#[test]
fn interior_jets_match_central_differences_at_all_scales() {
    for scale in [1.0e-6, 1.0, 1.0e6] {
        let parameter = 0.37;
        check_parameter_derivatives(scale, parameter, |value| {
            line_jet(
                Point2::new(-scale, 0.25 * scale),
                Point2::new(2.0 * scale, scale),
                CurveParameterDomain::SupportingLine,
                value,
            )
            .unwrap()
        });
        check_parameter_derivatives(scale, parameter, |value| {
            circle_jet(Point2::new(scale, -2.0 * scale), 1.5 * scale, value).unwrap()
        });
        check_parameter_derivatives(scale, parameter, |value| {
            circular_arc_jet(
                Point2::new(scale, -2.0 * scale),
                1.5 * scale,
                -0.4,
                1.7,
                value,
            )
            .unwrap()
        });
        let quadratic = [
            Point2::new(0.0, 0.0),
            Point2::new(scale, 0.25 * scale),
            Point2::new(2.0 * scale, scale),
        ];
        check_parameter_derivatives(scale, parameter, |value| {
            quadratic_bezier_jet(quadratic, value).unwrap()
        });
        let cubic = [
            Point2::new(0.0, 0.0),
            Point2::new(scale, 0.25 * scale),
            Point2::new(2.0 * scale, scale),
            Point2::new(3.0 * scale, scale),
        ];
        check_parameter_derivatives(scale, parameter, |value| {
            cubic_bezier_jet(cubic, value).unwrap()
        });
    }
}

#[test]
fn bezier_jets_are_similarity_covariant() {
    let angle: f64 = 0.63;
    let (sine, cosine) = angle.sin_cos();
    let scale = 7.0;
    let translation = Vector2::new(-4.0, 9.0);
    let transform_point = |point: Point2<f64>| {
        Point2::from(
            translation
                + Vector2::new(
                    scale * (cosine * point.x - sine * point.y),
                    scale * (sine * point.x + cosine * point.y),
                ),
        )
    };
    let transform_vector = |vector: Vector2<f64>| {
        Vector2::new(
            scale * (cosine * vector.x - sine * vector.y),
            scale * (sine * vector.x + cosine * vector.y),
        )
    };
    let quadratic = [
        Point2::new(-1.0, 0.5),
        Point2::new(0.0, 2.0),
        Point2::new(3.0, -1.0),
    ];
    let cubic = [
        Point2::new(-1.0, 0.5),
        Point2::new(0.0, 2.0),
        Point2::new(2.0, 1.0),
        Point2::new(3.0, -1.0),
    ];
    for (base, transformed) in [
        (
            quadratic_bezier_jet(quadratic, 0.37).unwrap(),
            quadratic_bezier_jet(quadratic.map(transform_point), 0.37).unwrap(),
        ),
        (
            cubic_bezier_jet(cubic, 0.37).unwrap(),
            cubic_bezier_jet(cubic.map(transform_point), 0.37).unwrap(),
        ),
    ] {
        assert_relative(
            transformed.position - transform_point(base.position),
            Vector2::zeros(),
            scale,
        );
        assert_relative(
            transformed.first_derivative,
            transform_vector(base.first_derivative),
            scale,
        );
        assert_relative(
            transformed.second_derivative,
            transform_vector(base.second_derivative),
            scale,
        );
        assert_relative(
            transformed.third_derivative,
            transform_vector(base.third_derivative),
            scale,
        );
    }
}

#[test]
fn parameter_regularity_and_nonfinite_failures_are_typed() {
    let bounded = CurveParameterDomain::Bounded {
        lower: 0.0,
        upper: 1.0,
    };
    assert!(matches!(
        line_jet(Point2::origin(), Point2::new(1.0, 0.0), bounded, 1.1),
        Err(CurveEvaluationError::Parameter(
            CurveParameterError::OutOfDomain { .. }
        ))
    ));
    assert!(matches!(
        cubic_bezier_jet([Point2::origin(); 4], 0.0),
        Err(CurveEvaluationError::Regularity(
            CurveRegularityError::ZeroSpeed
        ))
    ));
    assert!(matches!(
        circle_jet(Point2::origin(), 0.0, 0.0),
        Err(CurveEvaluationError::Regularity(
            CurveRegularityError::InvalidRadius { .. }
        ))
    ));
    assert!(matches!(
        quadratic_bezier_jet(
            [
                Point2::origin(),
                Point2::new(f64::MAX, 0.0),
                Point2::new(0.0, 1.0)
            ],
            0.5
        ),
        Err(CurveEvaluationError::Regularity(
            CurveRegularityError::ZeroSpeed | CurveRegularityError::NonFiniteJet
        ))
    ));
    assert!(matches!(
        circle_jet(Point2::origin(), 1.0, f64::INFINITY),
        Err(CurveEvaluationError::Parameter(
            CurveParameterError::NonFinite { .. }
        ))
    ));
    assert!(matches!(
        line_jet(
            Point2::new(f64::NAN, 0.0),
            Point2::new(1.0, 0.0),
            CurveParameterDomain::SupportingLine,
            0.0
        ),
        Err(CurveEvaluationError::Regularity(
            CurveRegularityError::NonFiniteDefinition
        ))
    ));
    assert!(matches!(
        circular_arc_jet(Point2::origin(), 1.0, 0.0, 0.0, 0.5),
        Err(CurveEvaluationError::Regularity(
            CurveRegularityError::ZeroSpeed
        ))
    ));
}

fn check_parameter_derivatives(scale: f64, parameter: f64, evaluate: impl Fn(f64) -> CurveJet2) {
    let step = 1.0e-5;
    let before = evaluate(parameter - step);
    let current = evaluate(parameter);
    let after = evaluate(parameter + step);
    assert_relative(
        (after.position - before.position) / (2.0 * step),
        current.first_derivative,
        scale,
    );
    assert_relative(
        (after.first_derivative - before.first_derivative) / (2.0 * step),
        current.second_derivative,
        scale,
    );
    assert_relative(
        (after.second_derivative - before.second_derivative) / (2.0 * step),
        current.third_derivative,
        scale,
    );
}

fn assert_relative(actual: Vector2<f64>, expected: Vector2<f64>, scale: f64) {
    let error = (actual - expected).norm();
    let denominator = actual.norm().max(expected.norm()).max(scale * 1.0e-9);
    assert!(
        error / denominator <= 1.0e-6,
        "actual={actual:?}, expected={expected:?}, relative_error={}",
        error / denominator
    );
}
