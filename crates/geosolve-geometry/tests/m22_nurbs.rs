use std::f64::consts::FRAC_1_SQRT_2;

use geosolve_geometry::{
    BSplineCurve2, BSplineKnotSide, CurveEvaluationError, CurveJet2, CurveRegularityError,
    NurbsControlProvenance, NurbsCurve2, NurbsDefinitionError, NurbsEvaluationError, Point2,
    RationalQuadraticConicSegment2, SMatrix, Vector2, rational_quadratic_conic_jet,
};

#[test]
fn unit_weight_clamped_and_periodic_nurbs_reproduce_bsplines() {
    let controls = vec![
        Point2::new(-1.0, 0.5),
        Point2::new(0.25, 2.0),
        Point2::new(1.5, -0.75),
        Point2::new(2.4, 1.25),
        Point2::new(3.0, -0.5),
    ];
    let knots = vec![0.0, 0.0, 0.0, 0.0, 0.4, 1.0, 1.0, 1.0, 1.0];
    let bspline = BSplineCurve2::try_clamped(3, controls.clone(), knots.clone()).unwrap();
    let nurbs = NurbsCurve2::try_clamped(3, controls, vec![1.0; 5], knots).unwrap();
    for span in bspline.basis().spans() {
        for parameter in [0.0, 0.17, 0.5, 0.83, 1.0] {
            assert_jet(
                nurbs.jet_on_span(span.index(), parameter).unwrap(),
                bspline.jet_on_span(span.index(), parameter).unwrap(),
                1.0,
            );
        }
    }

    let periodic_controls = vec![
        Point2::new(0.0, 0.0),
        Point2::new(1.5, -0.2),
        Point2::new(2.0, 1.4),
        Point2::new(0.5, 2.2),
        Point2::new(-0.8, 1.0),
    ];
    let period_knots = vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0];
    let bspline =
        BSplineCurve2::try_periodic(2, periodic_controls.clone(), period_knots.clone()).unwrap();
    let nurbs =
        NurbsCurve2::try_periodic(2, periodic_controls, vec![1.0; 5], period_knots).unwrap();
    for parameter in [0.0, 0.23, 1.7, 4.9, 5.0, 11.2] {
        assert_jet(
            nurbs.jet_at(parameter, BSplineKnotSide::Right).unwrap(),
            bspline.jet_at(parameter, BSplineKnotSide::Right).unwrap(),
            1.0,
        );
    }
    assert_jet(
        nurbs.jet_at(0.0, BSplineKnotSide::Left).unwrap(),
        bspline.jet_at(0.0, BSplineKnotSide::Left).unwrap(),
        1.0,
    );
}

#[test]
fn quadratic_nurbs_reproduce_positive_rational_conics() {
    let controls = [
        Point2::new(1.0, 0.0),
        Point2::new(1.0, 1.0),
        Point2::new(0.0, 1.0),
    ];
    let conic = RationalQuadraticConicSegment2::try_from_control_point(
        controls[0],
        controls[1],
        FRAC_1_SQRT_2,
        controls[2],
    )
    .unwrap();
    let nurbs = NurbsCurve2::try_clamped(
        2,
        controls.to_vec(),
        vec![1.0, FRAC_1_SQRT_2, 1.0],
        vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
    )
    .unwrap();
    for parameter in [0.0, 0.2, 0.5, 0.8, 1.0] {
        let actual = nurbs
            .jet_on_span(nurbs.basis().spans()[0].index(), parameter)
            .unwrap();
        assert_jet(
            actual,
            rational_quadratic_conic_jet(&conic, parameter).unwrap(),
            1.0,
        );
        assert_close(actual.position.coords.norm(), 1.0, 1.0);
    }
}

#[test]
fn rational_jets_match_parameter_and_weight_derivative_oracles() {
    for scale in [1.0e-6, 1.0, 1.0e6] {
        let curve = NurbsCurve2::try_clamped(
            3,
            vec![
                Point2::new(-scale, 0.5 * scale),
                Point2::new(0.0, 2.0 * scale),
                Point2::new(1.2 * scale, -0.3 * scale),
                Point2::new(2.0 * scale, 1.4 * scale),
                Point2::new(3.0 * scale, 0.2 * scale),
            ],
            vec![1.0, 0.4, 1.8, 0.75, 1.25],
            vec![0.0, 0.0, 0.0, 0.0, 0.4, 1.0, 1.0, 1.0, 1.0],
        )
        .unwrap();
        for span in curve.basis().spans() {
            check_parameter_derivatives(scale, 0.37, |parameter| {
                curve.jet_on_span(span.index(), parameter).unwrap()
            });

            let basis = curve.basis().basis_jet_on_span(span.index(), 0.37).unwrap();
            let jet = curve.jet_on_span(span.index(), 0.37).unwrap();
            for term in &basis.terms {
                let expected = weight_derivative(&basis, &curve, term.control_index, jet);
                let weight = curve.weights()[term.control_index];
                let step = weight * 1.0e-6;
                let mut lower_weights = curve.weights().to_vec();
                lower_weights[term.control_index] -= step;
                let lower = NurbsCurve2::try_new(
                    curve.basis().clone(),
                    curve.controls().to_vec(),
                    lower_weights,
                )
                .unwrap()
                .jet_on_span(span.index(), 0.37)
                .unwrap();
                let mut upper_weights = curve.weights().to_vec();
                upper_weights[term.control_index] += step;
                let upper = NurbsCurve2::try_new(
                    curve.basis().clone(),
                    curve.controls().to_vec(),
                    upper_weights,
                )
                .unwrap()
                .jet_on_span(span.index(), 0.37)
                .unwrap();
                let actual = [
                    (upper.position - lower.position) / (2.0 * step),
                    (upper.first_derivative - lower.first_derivative) / (2.0 * step),
                    (upper.second_derivative - lower.second_derivative) / (2.0 * step),
                    (upper.third_derivative - lower.third_derivative) / (2.0 * step),
                ];
                for order in 0..4 {
                    assert_vector(actual[order], expected[order], scale / weight);
                }
            }
        }
    }
}

#[test]
fn common_weight_scaling_affine_covariance_and_local_support_are_exact() {
    let matrix = SMatrix::<f64, 2, 2>::new(1.7, 0.4, -0.2, 1.3);
    let translation = Vector2::new(-4.0, 7.0);
    let controls = vec![
        Point2::new(-1.0, 0.5),
        Point2::new(0.0, 2.0),
        Point2::new(1.2, -0.3),
        Point2::new(2.0, 1.4),
        Point2::new(3.0, 0.2),
        Point2::new(4.0, -0.7),
    ];
    let weights = vec![1.0, 0.4, 1.8, 0.75, 1.25, 0.6];
    let knots = vec![0.0, 0.0, 0.0, 0.0, 0.3, 0.7, 1.0, 1.0, 1.0, 1.0];
    let base =
        NurbsCurve2::try_clamped(3, controls.clone(), weights.clone(), knots.clone()).unwrap();
    let span = base.basis().spans()[0].clone();
    let expected = base.jet_on_span(span.index(), 0.37).unwrap();

    for factor in [1.0e-200, 1.0, 1.0e200] {
        let scaled = NurbsCurve2::try_clamped(
            3,
            controls.clone(),
            weights.iter().map(|weight| weight * factor).collect(),
            knots.clone(),
        )
        .unwrap();
        assert_jet(
            scaled.jet_on_span(span.index(), 0.37).unwrap(),
            expected,
            1.0,
        );
    }

    let mapped = NurbsCurve2::try_clamped(
        3,
        controls
            .iter()
            .map(|point| Point2::from(matrix * point.coords + translation))
            .collect(),
        weights.clone(),
        knots.clone(),
    )
    .unwrap();
    let transformed = mapped.jet_on_span(span.index(), 0.37).unwrap();
    assert_point(
        transformed.position,
        Point2::from(matrix * expected.position.coords + translation),
        1.0,
    );
    assert_vector(
        transformed.first_derivative,
        matrix * expected.first_derivative,
        1.0,
    );
    assert_vector(
        transformed.second_derivative,
        matrix * expected.second_derivative,
        1.0,
    );
    assert_vector(
        transformed.third_derivative,
        matrix * expected.third_derivative,
        1.0,
    );

    let inactive = (0..controls.len())
        .find(|index| !span.support().contains(index))
        .unwrap();
    let mut changed_controls = controls;
    changed_controls[inactive] += Vector2::new(1000.0, -700.0);
    let mut changed_weights = weights;
    changed_weights[inactive] *= 1000.0;
    let changed = NurbsCurve2::try_clamped(3, changed_controls, changed_weights, knots).unwrap();
    assert_jet(
        changed.jet_on_span(span.index(), 0.37).unwrap(),
        expected,
        1.0,
    );
}

#[test]
fn clamped_and_periodic_homogeneous_insertion_preserves_geometry() {
    let clamped = NurbsCurve2::try_clamped(
        3,
        vec![
            Point2::new(0.0, 0.0),
            Point2::new(1.0, 2.0),
            Point2::new(2.0, -1.0),
            Point2::new(3.0, 1.5),
            Point2::new(4.0, 0.0),
        ],
        vec![1.0, 0.5, 1.7, 0.8, 1.3],
        vec![0.0, 0.0, 0.0, 0.0, 0.5, 1.0, 1.0, 1.0, 1.0],
    )
    .unwrap();
    let refined = clamped.insert_knot(0.37).unwrap();
    assert!(refined.split_span().is_some());
    assert_eq!(refined.control_provenance().len(), 6);
    for (output, source) in refined.control_provenance().iter().enumerate() {
        if let NurbsControlProvenance::Copy { control } = source {
            assert_eq!(
                refined.curve().controls()[output],
                clamped.controls()[*control]
            );
            assert_eq!(
                refined.curve().weights()[output].to_bits(),
                clamped.weights()[*control].to_bits()
            );
        }
    }
    assert_geometry_invariant(&clamped, refined.curve(), 0.0, 1.0);
    let repeated = refined.curve().insert_knot(0.5).unwrap();
    assert_eq!(repeated.split_span(), None);
    assert_geometry_invariant(refined.curve(), repeated.curve(), 0.0, 1.0);

    let periodic = NurbsCurve2::try_periodic(
        2,
        vec![
            Point2::new(0.0, 0.0),
            Point2::new(1.5, -0.2),
            Point2::new(2.0, 1.4),
            Point2::new(0.5, 2.2),
            Point2::new(-0.8, 1.0),
        ],
        vec![1.0, 0.6, 1.5, 0.75, 1.2],
        vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0],
    )
    .unwrap();
    let refined = periodic.insert_knot(2.4).unwrap();
    assert_geometry_invariant(&periodic, refined.curve(), 0.0, 5.0);
    let seam = periodic.insert_knot(0.0).unwrap();
    assert_geometry_invariant(&periodic, seam.curve(), 0.0, 5.0);
}

#[test]
fn rational_linear_parameterization_has_nonzero_higher_derivatives() {
    let curve = NurbsCurve2::try_clamped(
        1,
        vec![Point2::new(0.0, 0.0), Point2::new(2.0, 1.0)],
        vec![1.0, 3.0],
        vec![0.0, 0.0, 1.0, 1.0],
    )
    .unwrap();
    let jet = curve
        .jet_on_span(curve.basis().spans()[0].index(), 0.37)
        .unwrap();
    assert!(jet.second_derivative.norm() > 0.1);
    assert!(jet.third_derivative.norm() > 0.1);
    check_parameter_derivatives(1.0, 0.37, |parameter| {
        curve
            .jet_on_span(curve.basis().spans()[0].index(), parameter)
            .unwrap()
    });
}

#[test]
fn invalid_weights_mixed_scales_and_zero_speed_reject_typed() {
    let basis =
        geosolve_geometry::BSplineBasis::try_clamped(1, 2, vec![0.0, 0.0, 1.0, 1.0]).unwrap();
    assert!(matches!(
        NurbsCurve2::try_new(
            basis.clone(),
            vec![Point2::origin(), Point2::new(1.0, 0.0)],
            vec![1.0]
        ),
        Err(NurbsDefinitionError::WeightCount { .. })
    ));
    for weight in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        assert!(matches!(
            NurbsCurve2::try_new(
                basis.clone(),
                vec![Point2::origin(), Point2::new(1.0, 0.0)],
                vec![1.0, weight]
            ),
            Err(NurbsDefinitionError::InvalidWeight { index: 1, .. })
        ));
    }

    let mixed = NurbsCurve2::try_new(
        basis.clone(),
        vec![Point2::origin(), Point2::new(1.0, 0.0)],
        vec![f64::from_bits(1), f64::MAX],
    );
    assert!(matches!(
        mixed,
        Err(NurbsDefinitionError::MixedWeightScale { .. })
    ));

    let compensated_product = NurbsCurve2::try_new(
        basis.clone(),
        vec![Point2::origin(), Point2::new(1.0e308, 0.0)],
        vec![1.0, 1.0e-12],
    )
    .unwrap();
    let jet = compensated_product
        .jet_on_span(compensated_product.basis().spans()[0].index(), 1.0e-315)
        .unwrap();
    assert!(jet.position.x > 0.0);

    let cancellation_safe = NurbsCurve2::try_new(
        basis.clone(),
        vec![Point2::origin(), Point2::new(1.0, 0.0)],
        vec![1.0, 1.0e16],
    )
    .unwrap();
    let jet = cancellation_safe
        .jet_on_span(cancellation_safe.basis().spans()[0].index(), 0.5)
        .unwrap();
    assert_close(jet.first_derivative.x, 4.0e-16, 4.0e-16);
    assert!(jet.first_derivative.x.is_sign_positive());

    let translated = NurbsCurve2::try_new(
        basis.clone(),
        vec![Point2::new(1.0e15, 0.0), Point2::new(1.0e15 + 0.125, 0.0)],
        vec![1.0, 3.0],
    )
    .unwrap();
    let translated_jet = translated
        .jet_on_span(translated.basis().spans()[0].index(), 0.37)
        .unwrap();
    assert!(translated_jet.first_derivative.x.is_sign_positive());
    assert_close(
        translated_jet.first_derivative.x,
        0.375 / 1.74_f64.powi(2),
        1.0,
    );

    let locally_scaled = NurbsCurve2::try_clamped(
        1,
        (0..5)
            .map(|index| Point2::new(f64::from(index), 0.0))
            .collect(),
        vec![f64::from_bits(1), 1.0e-200, 1.0e-100, 1.0, 1.0e100],
        vec![0.0, 0.0, 1.0, 2.0, 3.0, 4.0, 4.0],
    )
    .unwrap();
    assert!(locally_scaled.insert_knot(0.5).is_ok());

    let collapsed = NurbsCurve2::try_new(
        basis,
        vec![Point2::new(2.0, 3.0), Point2::new(2.0, 3.0)],
        vec![1.0, 2.0],
    )
    .unwrap();
    assert!(matches!(
        collapsed.jet_on_span(collapsed.basis().spans()[0].index(), 0.5),
        Err(NurbsEvaluationError::Curve(
            CurveEvaluationError::Regularity(CurveRegularityError::ZeroSpeed)
        ))
    ));
}

fn weight_derivative(
    basis: &geosolve_geometry::BSplineBasisJet,
    curve: &NurbsCurve2,
    control_index: usize,
    jet: CurveJet2,
) -> [Vector2<f64>; 4] {
    let term = basis
        .terms
        .iter()
        .find(|term| term.control_index == control_index)
        .unwrap();
    let mut denominator = [0.0; 4];
    for basis_term in &basis.terms {
        for (order, derivative) in basis_term.derivatives.into_iter().enumerate() {
            denominator[order] += derivative * curve.weights()[basis_term.control_index];
        }
    }
    let derivatives = [
        jet.position.coords,
        jet.first_derivative,
        jet.second_derivative,
        jet.third_derivative,
    ];
    let binomial = [
        [1.0, 0.0, 0.0, 0.0],
        [1.0, 1.0, 0.0, 0.0],
        [1.0, 2.0, 1.0, 0.0],
        [1.0, 3.0, 3.0, 1.0],
    ];
    let mut result = [Vector2::zeros(); 4];
    for order in 0..4 {
        let mut numerator = curve.controls()[control_index].coords * term.derivatives[order]
            - derivatives[order] * term.derivatives[0];
        for inner in 1..=order {
            numerator -= (derivatives[order - inner] * term.derivatives[inner]
                + result[order - inner] * denominator[inner])
                * binomial[order][inner];
        }
        result[order] = numerator / denominator[0];
    }
    result
}

fn assert_geometry_invariant(before: &NurbsCurve2, after: &NurbsCurve2, lower: f64, upper: f64) {
    for index in 0..=100 {
        let fraction = f64::from(index) / 100.0;
        let parameter = (upper - lower).mul_add(fraction, lower);
        let side = if index == 100
            && matches!(
                before.basis().parameter_domain(),
                geosolve_geometry::CurveParameterDomain::Bounded { .. }
            ) {
            BSplineKnotSide::Left
        } else {
            BSplineKnotSide::Right
        };
        assert_jet(
            after.jet_at(parameter, side).unwrap(),
            before.jet_at(parameter, side).unwrap(),
            1.0,
        );
    }
}

fn check_parameter_derivatives(scale: f64, parameter: f64, evaluate: impl Fn(f64) -> CurveJet2) {
    let step = 1.0e-5;
    let before = evaluate(parameter - step);
    let current = evaluate(parameter);
    let after = evaluate(parameter + step);
    assert_vector(
        (after.position - before.position) / (2.0 * step),
        current.first_derivative,
        scale,
    );
    assert_vector(
        (after.first_derivative - before.first_derivative) / (2.0 * step),
        current.second_derivative,
        scale,
    );
    assert_vector(
        (after.second_derivative - before.second_derivative) / (2.0 * step),
        current.third_derivative,
        scale,
    );
}

fn assert_jet(actual: CurveJet2, expected: CurveJet2, scale: f64) {
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
    let denominator = actual.norm().max(expected.norm()).max(scale * 1.0e-8);
    assert!(
        error / denominator <= 1.0e-6,
        "actual={actual:?}, expected={expected:?}, relative_error={}",
        error / denominator
    );
}

fn assert_close(actual: f64, expected: f64, scale: f64) {
    assert!(
        (actual - expected).abs() <= 5.0e-10 * scale.max(1.0),
        "actual={actual}, expected={expected}"
    );
}
