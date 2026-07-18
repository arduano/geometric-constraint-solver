use std::f64::consts::{FRAC_1_SQRT_2, FRAC_PI_2, PI};

use geosolve_geometry::{
    ConicDefinitionError, ConicEvaluationError, CurveEvaluationError, CurveJet2,
    CurveParameterDomain, CurveParameterError, DirectedParameterTrim, Ellipse2,
    EllipseAxisObservability, EllipticalArc2, HyperbolaBranch, HyperbolaSegment2, ParabolaSegment2,
    Point2, ProperConicKind, RationalQuadraticConicSegment2, SMatrix, UnitDirection2, Vector2,
    circle_jet, ellipse_jet, elliptical_arc_jet, hyperbola_segment_jet, parabola_segment_jet,
    quadratic_bezier_jet, rational_quadratic_conic_jet,
};

const CLOSE_TOLERANCE: f64 = 2.0e-10;

#[test]
fn exact_jets_cover_every_analytic_family_and_both_hyperbola_branches() {
    let x_axis = unit(Vector2::x());
    let ellipse = Ellipse2::try_new(Point2::new(1.0, 2.0), x_axis, 3.0, 2.0).unwrap();
    let ellipse_jet = ellipse_jet(&ellipse, 0.0).unwrap();
    assert_point(ellipse_jet.position, Point2::new(4.0, 2.0), 1.0);
    assert_vector(ellipse_jet.first_derivative, Vector2::new(0.0, 2.0), 1.0);
    assert_vector(ellipse_jet.second_derivative, Vector2::new(-3.0, 0.0), 1.0);
    assert_vector(ellipse_jet.third_derivative, Vector2::new(0.0, -2.0), 1.0);
    assert_eq!(
        ellipse_jet.domain,
        CurveParameterDomain::Periodic {
            period: std::f64::consts::TAU
        }
    );

    let parabola = ParabolaSegment2::try_new(
        Point2::new(1.0, 2.0),
        x_axis,
        0.5,
        DirectedParameterTrim::try_new(-1.0, 2.0).unwrap(),
    )
    .unwrap();
    let parabola_jet = parabola_segment_jet(&parabola, 1.0 / 3.0).unwrap();
    assert_point(parabola_jet.position, parabola.vertex(), 1.0);
    assert_vector(parabola_jet.first_derivative, Vector2::new(0.0, 3.0), 1.0);
    assert_vector(parabola_jet.second_derivative, Vector2::new(9.0, 0.0), 1.0);
    assert_vector(parabola_jet.third_derivative, Vector2::zeros(), 1.0);

    for branch in [HyperbolaBranch::Positive, HyperbolaBranch::Negative] {
        let hyperbola = HyperbolaSegment2::try_new(
            Point2::new(1.0, 2.0),
            x_axis,
            3.0,
            2.0,
            branch,
            DirectedParameterTrim::try_new(-1.0, 1.0).unwrap(),
        )
        .unwrap();
        let jet = hyperbola_segment_jet(&hyperbola, 0.5).unwrap();
        let multiplier = branch.multiplier();
        assert_point(jet.position, Point2::new(1.0 + 3.0 * multiplier, 2.0), 1.0);
        assert_vector(jet.first_derivative, Vector2::new(0.0, 4.0), 1.0);
        assert_vector(
            jet.second_derivative,
            Vector2::new(12.0 * multiplier, 0.0),
            1.0,
        );
        assert_vector(jet.third_derivative, Vector2::new(0.0, 16.0), 1.0);
    }
}

#[test]
fn elliptical_arcs_preserve_positive_and_negative_sweep_derivatives() {
    let ellipse = Ellipse2::try_new(Point2::origin(), unit(Vector2::x()), 4.0, 2.0).unwrap();
    for signed_sweep in [FRAC_PI_2, -FRAC_PI_2] {
        let arc = EllipticalArc2::try_new(ellipse, 0.0, signed_sweep).unwrap();
        let start = elliptical_arc_jet(&arc, 0.0).unwrap();
        let end = elliptical_arc_jet(&arc, 1.0).unwrap();
        assert_point(start.position, Point2::new(4.0, 0.0), 1.0);
        assert_vector(
            start.first_derivative,
            Vector2::new(0.0, 2.0 * signed_sweep),
            1.0,
        );
        assert_vector(
            start.second_derivative,
            Vector2::new(-4.0 * signed_sweep.powi(2), 0.0),
            1.0,
        );
        assert_vector(
            start.third_derivative,
            Vector2::new(0.0, -2.0 * signed_sweep.powi(3)),
            1.0,
        );
        assert_point(
            end.position,
            Point2::new(0.0, 2.0 * signed_sweep.signum()),
            1.0,
        );
        assert_point(arc.start_point().unwrap(), start.position, 1.0);
        assert_point(arc.end_point().unwrap(), end.position, 1.0);
    }
}

#[test]
fn all_conic_jets_match_central_differences_at_required_scales() {
    for scale in [1.0e-6, 1.0, 1.0e6] {
        let axis = unit(Vector2::new(3.0, 4.0));
        let ellipse = Ellipse2::try_new(
            Point2::new(-2.0 * scale, scale),
            axis,
            3.0 * scale,
            1.25 * scale,
        )
        .unwrap();
        check_parameter_derivatives(scale, 0.37, |parameter| {
            ellipse_jet(&ellipse, parameter).unwrap()
        });

        let arc = EllipticalArc2::try_new(ellipse, -0.4, -1.7).unwrap();
        check_parameter_derivatives(scale, 0.37, |parameter| {
            elliptical_arc_jet(&arc, parameter).unwrap()
        });

        let rational = RationalQuadraticConicSegment2::try_from_control_point(
            Point2::new(-scale, 0.2 * scale),
            Point2::new(0.4 * scale, 2.0 * scale),
            0.65,
            Point2::new(2.0 * scale, -0.3 * scale),
        )
        .unwrap();
        check_parameter_derivatives(scale, 0.37, |parameter| {
            rational_quadratic_conic_jet(&rational, parameter).unwrap()
        });

        let parabola = ParabolaSegment2::try_new(
            Point2::new(-scale, 0.5 * scale),
            axis,
            0.75 * scale,
            DirectedParameterTrim::try_new(-0.8, 1.3).unwrap(),
        )
        .unwrap();
        check_parameter_derivatives(scale, 0.37, |parameter| {
            parabola_segment_jet(&parabola, parameter).unwrap()
        });

        for branch in [HyperbolaBranch::Positive, HyperbolaBranch::Negative] {
            let hyperbola = HyperbolaSegment2::try_new(
                Point2::new(scale, -0.5 * scale),
                axis,
                1.4 * scale,
                0.9 * scale,
                branch,
                DirectedParameterTrim::try_new(-0.7, 0.9).unwrap(),
            )
            .unwrap();
            check_parameter_derivatives(scale, 0.37, |parameter| {
                hyperbola_segment_jet(&hyperbola, parameter).unwrap()
            });
        }
    }
}

#[test]
fn unit_weight_rational_matches_quadratic_bezier_through_third_order() {
    let controls = [
        Point2::new(-1.0, 0.5),
        Point2::new(0.25, 2.0),
        Point2::new(3.0, -0.75),
    ];
    let rational = RationalQuadraticConicSegment2::try_from_control_point(
        controls[0],
        controls[1],
        1.0,
        controls[2],
    )
    .unwrap();
    assert_eq!(rational.proper_conic_kind(), ProperConicKind::Parabola);
    assert_close(rational.start_weight(), 1.0, 1.0);
    assert_close(rational.end_weight(), 1.0, 1.0);
    assert_eq!(rational.start_point(), controls[0]);
    assert_eq!(rational.end_point(), controls[2]);
    assert_eq!(
        rational.homogeneous_controls()[1],
        (controls[1].coords, 1.0)
    );

    for parameter in [0.0, 0.17, 0.5, 0.83, 1.0] {
        assert_jet(
            rational_quadratic_conic_jet(&rational, parameter).unwrap(),
            quadratic_bezier_jet(controls, parameter).unwrap(),
            1.0,
        );
    }
}

#[test]
fn canonical_rational_quarter_circle_is_exact() {
    let conic = RationalQuadraticConicSegment2::try_from_control_point(
        Point2::new(1.0, 0.0),
        Point2::new(1.0, 1.0),
        FRAC_1_SQRT_2,
        Point2::new(0.0, 1.0),
    )
    .unwrap();
    assert_eq!(conic.proper_conic_kind(), ProperConicKind::Ellipse);
    for parameter in [0.0, 0.2, 0.5, 0.8, 1.0] {
        let point = rational_quadratic_conic_jet(&conic, parameter)
            .unwrap()
            .position;
        assert_close(point.coords.norm(), 1.0, 1.0);
    }
    assert_point(
        rational_quadratic_conic_jet(&conic, 0.5).unwrap().position,
        Point2::new(FRAC_1_SQRT_2, FRAC_1_SQRT_2),
        1.0,
    );
}

#[test]
fn rational_jets_are_covariant_under_arbitrary_nonsingular_affine_maps() {
    let matrix = SMatrix::<f64, 2, 2>::new(2.0, 0.5, -0.3, 1.4);
    assert!(matrix.determinant().abs() > 0.1);
    let translation = Vector2::new(-4.0, 7.0);
    let transform_point = |point: Point2<f64>| Point2::from(matrix * point.coords + translation);
    let transform_vector = |vector: Vector2<f64>| matrix * vector;

    let conic = RationalQuadraticConicSegment2::try_new(
        Point2::new(-1.0, 0.25),
        Vector2::new(0.4, 1.7),
        0.6,
        Point2::new(2.0, -0.5),
    )
    .unwrap();
    let transformed = RationalQuadraticConicSegment2::try_new(
        transform_point(conic.start()),
        transform_vector(conic.weighted_middle()) + translation * conic.middle_weight(),
        conic.middle_weight(),
        transform_point(conic.end()),
    )
    .unwrap();

    for parameter in [0.0, 0.23, 0.5, 0.91, 1.0] {
        let base = rational_quadratic_conic_jet(&conic, parameter).unwrap();
        let mapped = rational_quadratic_conic_jet(&transformed, parameter).unwrap();
        assert_point(mapped.position, transform_point(base.position), 1.0);
        assert_vector(
            mapped.first_derivative,
            transform_vector(base.first_derivative),
            1.0,
        );
        assert_vector(
            mapped.second_derivative,
            transform_vector(base.second_derivative),
            1.0,
        );
        assert_vector(
            mapped.third_derivative,
            transform_vector(base.third_derivative),
            1.0,
        );
    }
}

#[test]
fn analytic_conics_are_similarity_covariant_with_covariant_features() {
    let angle: f64 = 0.73;
    let (sine, cosine) = angle.sin_cos();
    let scale = 5.0;
    let translation = Vector2::new(-6.0, 8.0);
    let rotate = |vector: Vector2<f64>| {
        Vector2::new(
            cosine * vector.x - sine * vector.y,
            sine * vector.x + cosine * vector.y,
        )
    };
    let transform_vector = |vector: Vector2<f64>| rotate(vector) * scale;
    let transform_point =
        |point: Point2<f64>| Point2::from(transform_vector(point.coords) + translation);
    let axis = unit(Vector2::new(3.0, 4.0));
    let mapped_axis = unit(rotate(axis.vector()));

    let ellipse = Ellipse2::try_new(Point2::new(1.0, -2.0), axis, 3.0, 1.2).unwrap();
    let mapped_ellipse = Ellipse2::try_new(
        transform_point(ellipse.center()),
        mapped_axis,
        scale * ellipse.semi_major(),
        scale * ellipse.semi_minor(),
    )
    .unwrap();
    assert_similarity_jet(
        ellipse_jet(&ellipse, 0.41).unwrap(),
        ellipse_jet(&mapped_ellipse, 0.41).unwrap(),
        transform_point,
        transform_vector,
    );
    for (base, mapped) in ellipse.foci().into_iter().zip(mapped_ellipse.foci()) {
        assert_point(mapped, transform_point(base), scale);
    }

    let arc = EllipticalArc2::try_new(ellipse, -0.3, 1.8).unwrap();
    let mapped_arc = EllipticalArc2::try_new(mapped_ellipse, -0.3, 1.8).unwrap();
    assert_similarity_jet(
        elliptical_arc_jet(&arc, 0.37).unwrap(),
        elliptical_arc_jet(&mapped_arc, 0.37).unwrap(),
        transform_point,
        transform_vector,
    );
    assert_point(
        mapped_arc.end_point().unwrap(),
        transform_point(arc.end_point().unwrap()),
        scale,
    );

    let trim = DirectedParameterTrim::try_new(-0.8, 1.1).unwrap();
    let parabola = ParabolaSegment2::try_new(Point2::new(2.0, 1.0), axis, 0.7, trim).unwrap();
    let mapped_parabola = ParabolaSegment2::try_new(
        transform_point(parabola.vertex()),
        mapped_axis,
        scale * parabola.focal_length(),
        trim,
    )
    .unwrap();
    assert_similarity_jet(
        parabola_segment_jet(&parabola, 0.37).unwrap(),
        parabola_segment_jet(&mapped_parabola, 0.37).unwrap(),
        transform_point,
        transform_vector,
    );
    assert_point(
        mapped_parabola.focus(),
        transform_point(parabola.focus()),
        scale,
    );

    for branch in [HyperbolaBranch::Positive, HyperbolaBranch::Negative] {
        let hyperbola =
            HyperbolaSegment2::try_new(Point2::new(-1.0, 2.0), axis, 1.4, 0.9, branch, trim)
                .unwrap();
        let mapped_hyperbola = HyperbolaSegment2::try_new(
            transform_point(hyperbola.center()),
            mapped_axis,
            scale * hyperbola.semi_transverse(),
            scale * hyperbola.semi_conjugate(),
            branch,
            trim,
        )
        .unwrap();
        assert_similarity_jet(
            hyperbola_segment_jet(&hyperbola, 0.37).unwrap(),
            hyperbola_segment_jet(&mapped_hyperbola, 0.37).unwrap(),
            transform_point,
            transform_vector,
        );
        assert_point(
            mapped_hyperbola.selected_branch_focus(),
            transform_point(hyperbola.selected_branch_focus()),
            scale,
        );
        assert_point(
            mapped_hyperbola.selected_branch_vertex(),
            transform_point(hyperbola.selected_branch_vertex()),
            scale,
        );
        assert_vector(
            mapped_hyperbola.branch_witness(),
            rotate(hyperbola.branch_witness()),
            scale,
        );
    }
}

#[test]
fn circle_limit_is_exact_and_near_circle_orientation_remains_observable() {
    let center = Point2::new(2.0, -3.0);
    let radius = 4.0;
    let orientation: f64 = 0.61;
    let axis = unit(Vector2::new(orientation.cos(), orientation.sin()));
    let circle_limit = Ellipse2::try_new(center, axis, radius, radius).unwrap();
    assert_eq!(
        circle_limit.axis_observability(),
        EllipseAxisObservability::UnobservableCircleLimit
    );
    assert_close(circle_limit.linear_eccentricity(), 0.0, 1.0);
    assert_eq!(circle_limit.foci(), [center, center]);
    assert_close(circle_limit.major_axis_length(), 2.0 * radius, 1.0);
    assert_close(circle_limit.minor_axis_length(), 2.0 * radius, 1.0);

    let parameter = -0.37;
    assert_jet(
        ellipse_jet(&circle_limit, parameter).unwrap(),
        circle_jet(center, radius, orientation + parameter).unwrap(),
        1.0,
    );

    let phase_shift = 0.93;
    let shifted_axis = unit(Vector2::new(
        (orientation + phase_shift).cos(),
        (orientation + phase_shift).sin(),
    ));
    let shifted = Ellipse2::try_new(center, shifted_axis, radius, radius).unwrap();
    assert_jet(
        ellipse_jet(&shifted, parameter - phase_shift).unwrap(),
        ellipse_jet(&circle_limit, parameter).unwrap(),
        1.0,
    );

    let trimmed = EllipticalArc2::try_new(circle_limit, 0.2, 1.1).unwrap();
    assert_eq!(
        trimmed.axis_observability(),
        EllipseAxisObservability::ObservableByDirectedTrim
    );
    assert_ne!(trimmed.start_point().unwrap(), trimmed.end_point().unwrap());

    let just_smaller = f64::from_bits(radius.to_bits() - 1);
    let near_circle = Ellipse2::try_new(center, axis, radius, just_smaller).unwrap();
    assert!(matches!(
        near_circle.axis_observability(),
        EllipseAxisObservability::Observable {
            relative_axis_separation
        } if relative_axis_separation > 0.0
    ));
    assert!(near_circle.linear_eccentricity() > 0.0);
}

#[test]
fn parabola_focus_directrix_and_hyperbola_focal_identities_hold() {
    let axis = unit(Vector2::new(3.0, 4.0));
    let vertex = Point2::new(-2.0, 1.0);
    let focal_length = 1.7;
    let parabola = ParabolaSegment2::try_new(
        vertex,
        axis,
        focal_length,
        DirectedParameterTrim::try_new(-2.0, 1.5).unwrap(),
    )
    .unwrap();
    let directrix_point = vertex - axis.vector() * focal_length;
    for parameter in [0.0, 0.2, 0.7, 1.0] {
        let point = parabola_segment_jet(&parabola, parameter).unwrap().position;
        let focus_distance = (point - parabola.focus()).norm();
        let directrix_distance = (point - directrix_point).dot(&axis.vector()).abs();
        assert_close(focus_distance, directrix_distance, focal_length);
    }
    assert_point(
        parabola.start_point().unwrap(),
        parabola_segment_jet(&parabola, 0.0).unwrap().position,
        1.0,
    );
    assert_point(
        parabola.end_point().unwrap(),
        parabola_segment_jet(&parabola, 1.0).unwrap().position,
        1.0,
    );

    for branch in [HyperbolaBranch::Positive, HyperbolaBranch::Negative] {
        let hyperbola = HyperbolaSegment2::try_new(
            Point2::new(1.0, -0.5),
            axis,
            2.0,
            1.25,
            branch,
            DirectedParameterTrim::try_new(-1.1, 0.9).unwrap(),
        )
        .unwrap();
        let [negative_focus, positive_focus] = hyperbola.foci();
        let selected_focus = match branch {
            HyperbolaBranch::Positive => positive_focus,
            HyperbolaBranch::Negative => negative_focus,
        };
        assert_point(hyperbola.selected_branch_focus(), selected_focus, 1.0);
        assert_vector(
            hyperbola.selected_branch_vertex() - hyperbola.center(),
            axis.vector() * (branch.multiplier() * hyperbola.semi_transverse()),
            1.0,
        );
        for parameter in [0.0, 0.31, 0.77, 1.0] {
            let point = hyperbola_segment_jet(&hyperbola, parameter)
                .unwrap()
                .position;
            let focal_difference =
                ((point - negative_focus).norm() - (point - positive_focus).norm()).abs();
            assert_close(focal_difference, 2.0 * hyperbola.semi_transverse(), 1.0);
            assert!((point - hyperbola.center()).dot(&hyperbola.branch_witness()) > 0.0);
        }
        assert_point(
            hyperbola.start_point().unwrap(),
            hyperbola_segment_jet(&hyperbola, 0.0).unwrap().position,
            1.0,
        );
        assert_point(
            hyperbola.end_point().unwrap(),
            hyperbola_segment_jet(&hyperbola, 1.0).unwrap().position,
            1.0,
        );
    }
}

#[test]
fn reversed_trims_are_valid_and_invalid_domains_are_typed() {
    let reversed = DirectedParameterTrim::try_new(3.0, -2.0).unwrap();
    assert_close(reversed.start(), 3.0, 1.0);
    assert_close(reversed.end(), -2.0, 1.0);
    assert_close(reversed.signed_rate(), -5.0, 1.0);
    assert_close(reversed.parameter_at(0.0), 3.0, 1.0);
    assert_close(reversed.parameter_at(0.4), 1.0, 1.0);
    assert_close(reversed.parameter_at(1.0), -2.0, 1.0);
    assert!(matches!(
        DirectedParameterTrim::try_new(1.0, 1.0),
        Err(ConicDefinitionError::EqualTrimParameters { .. })
    ));
    assert!(matches!(
        DirectedParameterTrim::try_new(f64::NAN, 1.0),
        Err(ConicDefinitionError::NonFiniteParameter { .. })
    ));
    assert!(DirectedParameterTrim::try_new(0.0, f64::INFINITY).is_err());
    assert!(DirectedParameterTrim::try_new(-f64::MAX, f64::MAX).is_err());

    let axis = unit(Vector2::x());
    let ellipse = Ellipse2::try_new(Point2::origin(), axis, 2.0, 1.0).unwrap();
    let arc = EllipticalArc2::try_new(ellipse, 0.0, -PI).unwrap();
    let rational = RationalQuadraticConicSegment2::try_from_control_point(
        Point2::new(-1.0, 0.0),
        Point2::new(0.0, 1.0),
        0.5,
        Point2::new(1.0, 0.0),
    )
    .unwrap();
    let parabola = ParabolaSegment2::try_new(Point2::origin(), axis, 1.0, reversed).unwrap();
    let hyperbola = HyperbolaSegment2::try_new(
        Point2::origin(),
        axis,
        1.0,
        0.5,
        HyperbolaBranch::Negative,
        reversed,
    )
    .unwrap();
    for result in [
        elliptical_arc_jet(&arc, -0.01),
        rational_quadratic_conic_jet(&rational, 1.01),
        parabola_segment_jet(&parabola, -0.01),
        hyperbola_segment_jet(&hyperbola, 1.01),
    ] {
        assert!(matches!(
            result,
            Err(ConicEvaluationError::Curve(
                CurveEvaluationError::Parameter(CurveParameterError::OutOfDomain { .. })
            ))
        ));
    }
    assert!(matches!(
        ellipse_jet(&ellipse, f64::INFINITY),
        Err(ConicEvaluationError::Curve(
            CurveEvaluationError::Parameter(CurveParameterError::NonFinite { .. })
        ))
    ));
}

#[test]
fn rational_definitions_classify_and_reject_poles_or_degeneracy() {
    let start = Point2::new(-1.0, 0.0);
    let middle = Point2::new(0.0, 1.0);
    let end = Point2::new(1.0, 0.0);
    for (weight, expected) in [
        (-0.5, ProperConicKind::Ellipse),
        (0.5, ProperConicKind::Ellipse),
        (1.0, ProperConicKind::Parabola),
        (2.0, ProperConicKind::Hyperbola),
    ] {
        let conic =
            RationalQuadraticConicSegment2::try_from_control_point(start, middle, weight, end)
                .unwrap();
        assert_eq!(conic.proper_conic_kind(), expected);
    }

    let zero_weight =
        RationalQuadraticConicSegment2::try_new(start, Vector2::new(0.0, 1.0), 0.0, end).unwrap();
    assert_eq!(zero_weight.proper_conic_kind(), ProperConicKind::Ellipse);
    assert!(rational_quadratic_conic_jet(&zero_weight, 0.5).is_ok());
    assert!(matches!(
        RationalQuadraticConicSegment2::try_from_control_point(start, middle, 0.0, end),
        Err(ConicDefinitionError::ZeroWeightOrdinaryControl)
    ));

    for weight in [-1.0, -1.5, -10.0] {
        assert!(matches!(
            RationalQuadraticConicSegment2::try_new(start, middle.coords * weight, weight, end),
            Err(ConicDefinitionError::RationalDenominatorPole { .. })
        ));
    }
    assert!(matches!(
        RationalQuadraticConicSegment2::try_from_control_point(
            Point2::new(0.0, 0.0),
            Point2::new(1.0, 0.0),
            1.0,
            Point2::new(2.0, 0.0)
        ),
        Err(ConicDefinitionError::DegenerateHomogeneousControls)
    ));
    assert!(RationalQuadraticConicSegment2::try_new(start, Vector2::zeros(), 0.0, end).is_err());
}

#[test]
fn defensive_denominator_and_hyperbolic_overflow_never_return_success() {
    let nearly_polar = RationalQuadraticConicSegment2::try_new(
        Point2::new(-1.0, 0.0),
        Vector2::new(0.0, 1.0),
        -1.0 + f64::EPSILON,
        Point2::new(1.0, 0.0),
    )
    .unwrap();
    let denominator_failure = rational_quadratic_conic_jet(&nearly_polar, 0.5);
    assert!(matches!(
        denominator_failure,
        Err(ConicEvaluationError::RationalDenominator {
            parameter: 0.5,
            denominator,
            condition_scale
        }) if denominator.is_finite() && condition_scale.is_finite()
    ));

    assert!(matches!(
        HyperbolaSegment2::try_new(
            Point2::origin(),
            unit(Vector2::x()),
            1.0,
            1.0,
            HyperbolaBranch::Positive,
            DirectedParameterTrim::try_new(0.0, 1_000.0).unwrap(),
        ),
        Err(ConicDefinitionError::NonRepresentableDerivedGeometry { .. })
    ));
}

#[test]
fn definitions_reject_overflowing_promised_features_and_measurements() {
    let axis = unit(Vector2::x());
    assert!(matches!(
        Ellipse2::try_new(Point2::origin(), axis, f64::MAX, f64::MAX),
        Err(ConicDefinitionError::NonRepresentableDerivedGeometry { .. })
    ));

    let trim = DirectedParameterTrim::try_new(-1.0, 1.0).unwrap();
    assert!(matches!(
        ParabolaSegment2::try_new(Point2::new(f64::MAX, 0.0), axis, f64::MAX, trim),
        Err(ConicDefinitionError::NonRepresentableDerivedGeometry { .. })
    ));
    assert!(matches!(
        HyperbolaSegment2::try_new(
            Point2::origin(),
            axis,
            f64::MAX,
            f64::MAX,
            HyperbolaBranch::Positive,
            trim,
        ),
        Err(ConicDefinitionError::NonRepresentableDerivedGeometry { .. })
    ));
}

#[test]
fn definitions_validate_directions_dimensions_and_arc_sweeps() {
    let extreme = UnitDirection2::try_new(Vector2::new(f64::MAX, f64::MAX)).unwrap();
    assert_close(extreme.vector().norm(), 1.0, 1.0);
    let tiny = UnitDirection2::try_new(Vector2::new(f64::from_bits(1), 0.0)).unwrap();
    assert_eq!(tiny.vector(), Vector2::x());
    assert_eq!(tiny.left_normal().vector(), Vector2::y());
    assert!(matches!(
        UnitDirection2::try_new(Vector2::zeros()),
        Err(ConicDefinitionError::ZeroDirection)
    ));
    assert!(UnitDirection2::try_new(Vector2::new(f64::NAN, 0.0)).is_err());

    let axis = unit(Vector2::x());
    assert!(Ellipse2::try_new(Point2::origin(), axis, 1.0, 2.0).is_err());
    assert!(Ellipse2::try_new(Point2::origin(), axis, 1.0, 0.0).is_err());
    let ellipse = Ellipse2::try_new(Point2::origin(), axis, 2.0, 1.0).unwrap();
    assert_eq!(ellipse.major_axis(), axis);
    assert_eq!(ellipse.minor_axis(), axis.left_normal());
    assert_eq!(
        ellipse.major_axis_endpoints(),
        [Point2::new(-2.0, 0.0), Point2::new(2.0, 0.0)]
    );
    assert_eq!(
        ellipse.minor_axis_endpoints(),
        [Point2::new(0.0, -1.0), Point2::new(0.0, 1.0)]
    );
    assert!(EllipticalArc2::try_new(ellipse, f64::NAN, 1.0).is_err());
    assert!(EllipticalArc2::try_new(ellipse, 0.0, 0.0).is_err());
    assert!(EllipticalArc2::try_new(ellipse, 0.0, f64::INFINITY).is_err());

    let trim = DirectedParameterTrim::try_new(-1.0, 1.0).unwrap();
    assert!(ParabolaSegment2::try_new(Point2::origin(), axis, 0.0, trim).is_err());
    assert!(
        HyperbolaSegment2::try_new(
            Point2::origin(),
            axis,
            -1.0,
            1.0,
            HyperbolaBranch::Positive,
            trim
        )
        .is_err()
    );
}

fn unit(vector: Vector2<f64>) -> UnitDirection2 {
    UnitDirection2::try_new(vector).unwrap()
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

fn assert_similarity_jet(
    base: CurveJet2,
    mapped: CurveJet2,
    transform_point: impl Fn(Point2<f64>) -> Point2<f64>,
    transform_vector: impl Fn(Vector2<f64>) -> Vector2<f64>,
) {
    assert_point(mapped.position, transform_point(base.position), 1.0);
    assert_vector(
        mapped.first_derivative,
        transform_vector(base.first_derivative),
        1.0,
    );
    assert_vector(
        mapped.second_derivative,
        transform_vector(base.second_derivative),
        1.0,
    );
    assert_vector(
        mapped.third_derivative,
        transform_vector(base.third_derivative),
        1.0,
    );
}

fn assert_jet(actual: CurveJet2, expected: CurveJet2, scale: f64) {
    assert_eq!(actual.domain, expected.domain);
    assert_point(actual.position, expected.position, scale);
    assert_vector(actual.first_derivative, expected.first_derivative, scale);
    assert_vector(actual.second_derivative, expected.second_derivative, scale);
    assert_vector(actual.third_derivative, expected.third_derivative, scale);
}

fn assert_point(actual: Point2<f64>, expected: Point2<f64>, scale: f64) {
    assert_vector(actual - expected, Vector2::zeros(), scale);
}

fn assert_vector(actual: Vector2<f64>, expected: Vector2<f64>, scale: f64) {
    let error = (actual - expected).norm();
    let reference = actual.norm().max(expected.norm()).max(scale).max(1.0);
    assert!(
        error <= CLOSE_TOLERANCE * reference,
        "actual={actual:?}, expected={expected:?}, error={error}, reference={reference}"
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

fn assert_close(actual: f64, expected: f64, scale: f64) {
    let reference = actual.abs().max(expected.abs()).max(scale).max(1.0);
    assert!(
        (actual - expected).abs() <= CLOSE_TOLERANCE * reference,
        "actual={actual}, expected={expected}"
    );
}
