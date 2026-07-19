use geosolve_geometry::{
    BSplineBasis, BSplineContinuity, BSplineCurve2, BSplineDefinitionError, BSplineEvaluationError,
    BSplineInsertionError, BSplineKnotSide, CurveJet2, MAX_BSPLINE_DEGREE, Point2, SMatrix,
    Vector2, cubic_bezier_jet, quadratic_bezier_jet,
};

const CLOSE_TOLERANCE: f64 = 5.0e-10;

#[test]
fn clamped_bezier_bases_match_polynomial_jets_through_order_three() {
    let quadratic_controls = [
        Point2::new(-1.0, 0.5),
        Point2::new(0.25, 2.0),
        Point2::new(3.0, -0.75),
    ];
    let quadratic = BSplineCurve2::try_clamped(
        2,
        quadratic_controls.to_vec(),
        vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
    )
    .unwrap();
    let cubic_controls = [
        Point2::new(-1.0, 0.5),
        Point2::new(0.25, 2.0),
        Point2::new(2.0, 1.25),
        Point2::new(3.0, -0.75),
    ];
    let cubic = BSplineCurve2::try_clamped(
        3,
        cubic_controls.to_vec(),
        vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0],
    )
    .unwrap();

    for parameter in [0.0, 0.17, 0.5, 0.83, 1.0] {
        assert_jet(
            quadratic
                .jet_on_span(quadratic.basis().spans()[0].index(), parameter)
                .unwrap(),
            quadratic_bezier_jet(quadratic_controls, parameter).unwrap(),
            1.0,
        );
        assert_jet(
            cubic
                .jet_on_span(cubic.basis().spans()[0].index(), parameter)
                .unwrap(),
            cubic_bezier_jet(cubic_controls, parameter).unwrap(),
            1.0,
        );
    }
}

#[test]
fn basis_partition_derivatives_and_local_support_are_exactly_bounded() {
    let basis = BSplineBasis::try_clamped(
        3,
        7,
        vec![0.0, 0.0, 0.0, 0.0, 0.2, 0.55, 0.8, 1.0, 1.0, 1.0, 1.0],
    )
    .unwrap();
    for span in basis.spans() {
        assert_eq!(span.support().len(), 4);
        for parameter in [0.0, 0.19, 0.5, 0.81, 1.0] {
            let jet = basis.jet_on_span_for_test(span.index(), parameter);
            assert_eq!(jet.terms.len(), 4);
            for derivative in 0..4 {
                let sum = jet
                    .terms
                    .iter()
                    .map(|term| term.derivatives[derivative])
                    .sum::<f64>();
                let expected = if derivative == 0 { 1.0 } else { 0.0 };
                assert_close(sum, expected, 1.0);
            }
        }
    }

    let controls = (0..7)
        .map(|index| {
            let value = f64::from(index);
            Point2::new(value, (0.7 * value).sin())
        })
        .collect::<Vec<_>>();
    let curve = BSplineCurve2::try_new(basis, controls.clone()).unwrap();
    let selected = curve.basis().spans()[1].clone();
    let before = curve.jet_on_span(selected.index(), 0.37).unwrap();
    let inactive = (0..controls.len())
        .find(|index| !selected.support().contains(index))
        .unwrap();
    let mut changed = controls;
    changed[inactive] += Vector2::new(1000.0, -700.0);
    let changed = BSplineCurve2::try_new(curve.basis().clone(), changed).unwrap();
    assert_jet(
        changed.jet_on_span(selected.index(), 0.37).unwrap(),
        before,
        1.0,
    );
}

#[test]
fn clamped_jets_match_parameter_differences_and_affine_covariance() {
    let matrix = SMatrix::<f64, 2, 2>::new(1.7, 0.4, -0.2, 1.3);
    let translation = Vector2::new(-4.0, 7.0);
    for scale in [1.0e-6, 1.0, 1.0e6] {
        let controls = vec![
            Point2::new(-scale, 0.5 * scale),
            Point2::new(0.0, 2.0 * scale),
            Point2::new(1.2 * scale, -0.3 * scale),
            Point2::new(2.0 * scale, 1.4 * scale),
            Point2::new(3.0 * scale, 0.2 * scale),
        ];
        let curve = BSplineCurve2::try_clamped(
            3,
            controls.clone(),
            vec![0.0, 0.0, 0.0, 0.0, 0.4, 1.0, 1.0, 1.0, 1.0],
        )
        .unwrap();
        for span in curve.basis().spans() {
            check_parameter_derivatives(scale, 0.37, |parameter| {
                curve.jet_on_span(span.index(), parameter).unwrap()
            });
        }

        let mapped_controls = controls
            .into_iter()
            .map(|point| Point2::from(matrix * point.coords + translation))
            .collect::<Vec<_>>();
        let mapped = BSplineCurve2::try_clamped(
            3,
            mapped_controls,
            vec![0.0, 0.0, 0.0, 0.0, 0.4, 1.0, 1.0, 1.0, 1.0],
        )
        .unwrap();
        for (base_span, mapped_span) in curve.basis().spans().iter().zip(mapped.basis().spans()) {
            let base = curve.jet_on_span(base_span.index(), 0.37).unwrap();
            let transformed = mapped.jet_on_span(mapped_span.index(), 0.37).unwrap();
            assert_point(
                transformed.position,
                Point2::from(matrix * base.position.coords + translation),
                scale,
            );
            assert_vector(
                transformed.first_derivative,
                matrix * base.first_derivative,
                scale,
            );
            assert_vector(
                transformed.second_derivative,
                matrix * base.second_derivative,
                scale,
            );
            assert_vector(
                transformed.third_derivative,
                matrix * base.third_derivative,
                scale,
            );
        }
    }
}

#[test]
fn explicit_knot_sides_and_multiplicity_report_guaranteed_continuity() {
    let controls = vec![
        Point2::new(0.0, 0.0),
        Point2::new(1.0, 1.0),
        Point2::new(2.0, -0.5),
        Point2::new(3.0, 1.5),
        Point2::new(4.0, -1.0),
        Point2::new(5.0, 1.0),
        Point2::new(6.0, 0.0),
    ];
    let curve = BSplineCurve2::try_clamped(
        3,
        controls,
        vec![0.0, 0.0, 0.0, 0.0, 0.3, 0.5, 0.5, 1.0, 1.0, 1.0, 1.0],
    )
    .unwrap();
    assert_eq!(
        curve.basis().continuity_at(0.3).unwrap(),
        Some(BSplineContinuity::Guaranteed {
            multiplicity: 1,
            order: 2
        })
    );
    assert_eq!(
        curve.basis().continuity_at(0.5).unwrap(),
        Some(BSplineContinuity::Guaranteed {
            multiplicity: 2,
            order: 1
        })
    );
    curve.basis().require_continuity(0.5, 1).unwrap();
    assert!(matches!(
        curve.basis().require_continuity(0.5, 2),
        Err(BSplineEvaluationError::InsufficientContinuity { available: 1, .. })
    ));

    let left_span = curve
        .basis()
        .locate_span(0.5, BSplineKnotSide::Left)
        .unwrap();
    let right_span = curve
        .basis()
        .locate_span(0.5, BSplineKnotSide::Right)
        .unwrap();
    assert_ne!(left_span, right_span);
    let left = curve.jet_at(0.5, BSplineKnotSide::Left).unwrap();
    let right = curve.jet_at(0.5, BSplineKnotSide::Right).unwrap();
    assert_point(left.position, right.position, 1.0);
    assert_vector(left.first_derivative, right.first_derivative, 1.0);
    assert!((left.second_derivative - right.second_derivative).norm() > 1.0e-3);

    assert!(matches!(
        curve.basis().locate_span(0.0, BSplineKnotSide::Left),
        Err(BSplineEvaluationError::UnavailableKnotSide { .. })
    ));
    assert!(matches!(
        curve.basis().locate_span(1.0, BSplineKnotSide::Right),
        Err(BSplineEvaluationError::UnavailableKnotSide { .. })
    ));
}

#[test]
fn periodic_curves_wrap_unique_controls_and_preserve_seam_sides() {
    let curve = periodic_curve();
    let period = 5.0;
    for parameter in [0.17, 1.3, 3.8, 4.91] {
        let base = curve.jet_at(parameter, BSplineKnotSide::Right).unwrap();
        for winding in [-3.0, -1.0, 1.0, 4.0] {
            assert_jet(
                curve
                    .jet_at(parameter + winding * period, BSplineKnotSide::Right)
                    .unwrap(),
                base,
                1.0,
            );
        }
    }
    let left = curve.jet_at(0.0, BSplineKnotSide::Left).unwrap();
    let right = curve.jet_at(0.0, BSplineKnotSide::Right).unwrap();
    assert_point(left.position, right.position, 1.0);
    assert_vector(left.first_derivative, right.first_derivative, 1.0);
    assert_eq!(curve.basis().spans().len(), 5);
    assert_eq!(curve.basis().spans()[4].support(), &[4, 0, 1]);
}

#[test]
fn nonuniform_periodic_cubic_jets_partition_wrap_and_refine() {
    let controls = vec![
        Point2::new(0.0, 0.0),
        Point2::new(1.5, -0.2),
        Point2::new(2.0, 1.4),
        Point2::new(1.2, 2.4),
        Point2::new(0.1, 2.1),
        Point2::new(-1.0, 1.2),
        Point2::new(-0.7, 0.3),
    ];
    let curve =
        BSplineCurve2::try_periodic(3, controls, vec![0.0, 0.8, 1.7, 1.7, 2.9, 4.1, 4.6, 5.0])
            .unwrap();
    for span in curve.basis().spans() {
        let jet = curve.basis().basis_jet_on_span(span.index(), 0.37).unwrap();
        assert_eq!(jet.terms.len(), 4);
        for derivative in 0..4 {
            let sum = jet
                .terms
                .iter()
                .map(|term| term.derivatives[derivative])
                .sum::<f64>();
            assert_close(sum, if derivative == 0 { 1.0 } else { 0.0 }, 1.0);
        }
        check_parameter_derivatives(1.0, 0.37, |parameter| {
            curve.jet_on_span(span.index(), parameter).unwrap()
        });
    }
    for parameter in [0.31, 1.2, 2.3, 4.83] {
        assert_jet(
            curve
                .jet_at(parameter + 10.0, BSplineKnotSide::Right)
                .unwrap(),
            curve.jet_at(parameter, BSplineKnotSide::Right).unwrap(),
            1.0,
        );
    }
    let refined = curve.insert_knot(3.4).unwrap();
    assert_geometry_invariant(&curve, refined.curve(), 0.0, 5.0);
    let repeated = refined.curve().insert_knot(1.7).unwrap();
    assert_geometry_invariant(refined.curve(), repeated.curve(), 0.0, 5.0);
}

#[test]
fn clamped_and_periodic_knot_insertion_preserve_parameterized_geometry() {
    let clamped = BSplineCurve2::try_clamped(
        3,
        vec![
            Point2::new(0.0, 0.0),
            Point2::new(1.0, 2.0),
            Point2::new(2.0, -1.0),
            Point2::new(3.0, 1.5),
            Point2::new(4.0, 0.0),
        ],
        vec![0.0, 0.0, 0.0, 0.0, 0.5, 1.0, 1.0, 1.0, 1.0],
    )
    .unwrap();
    let refined = clamped.insert_knot(0.37).unwrap();
    assert!(refined.split_span().is_some());
    assert_eq!(refined.control_stencils().len(), 6);
    assert_geometry_invariant(&clamped, refined.curve(), 0.0, 1.0);
    let repeated = refined.curve().insert_knot(0.5).unwrap();
    assert_eq!(repeated.split_span(), None);
    assert_geometry_invariant(refined.curve(), repeated.curve(), 0.0, 1.0);

    let periodic = periodic_curve();
    let periodic_refined = periodic.insert_knot(2.4).unwrap();
    assert!(periodic_refined.split_span().is_some());
    assert_eq!(periodic_refined.control_stencils().len(), 6);
    assert_geometry_invariant(&periodic, periodic_refined.curve(), 0.0, 5.0);
    let seam_refined = periodic.insert_knot(0.0).unwrap();
    assert_eq!(seam_refined.split_span(), None);
    assert_geometry_invariant(&periodic, seam_refined.curve(), 0.0, 5.0);
}

#[test]
fn malformed_knots_controls_and_excessive_insertion_reject_typed() {
    assert!(matches!(
        BSplineBasis::try_clamped(0, 2, vec![0.0; 3]),
        Err(BSplineDefinitionError::InvalidDegree { .. })
    ));
    assert!(matches!(
        BSplineBasis::try_clamped(3, 3, vec![0.0; 7]),
        Err(BSplineDefinitionError::InsufficientControls { .. })
    ));
    assert!(matches!(
        BSplineBasis::try_clamped(MAX_BSPLINE_DEGREE + 1, 66, Vec::new()),
        Err(BSplineDefinitionError::DegreeLimit { .. })
    ));
    assert!(matches!(
        BSplineBasis::try_clamped(2, 4, vec![0.0, 0.0, 0.0, 0.7, 0.6, 1.0, 1.0]),
        Err(BSplineDefinitionError::DecreasingKnots { .. })
    ));
    assert!(matches!(
        BSplineBasis::try_periodic(2, 4, vec![1.0, 2.0, 3.0, 4.0, 5.0]),
        Err(BSplineDefinitionError::InvalidPeriodicOrigin { .. })
    ));
    assert!(matches!(
        BSplineBasis::try_periodic(2, 4, vec![0.0, 0.0, 0.0, 1.0, 2.0]),
        Err(BSplineDefinitionError::KnotMultiplicity { .. })
    ));
    assert!(matches!(
        BSplineBasis::try_clamped(1, 2, vec![-f64::MAX, -f64::MAX, f64::MAX, f64::MAX]),
        Err(BSplineDefinitionError::EmptyDomain)
    ));
    assert!(matches!(
        BSplineCurve2::try_periodic(
            2,
            vec![
                Point2::origin(),
                Point2::new(1.0, 0.0),
                Point2::new(f64::NAN, 0.0),
            ],
            vec![0.0, 1.0, 2.0, 3.0]
        ),
        Err(BSplineDefinitionError::NonFiniteControl { index: 2 })
    ));

    let maximum = BSplineCurve2::try_clamped(
        2,
        vec![
            Point2::new(0.0, 0.0),
            Point2::new(1.0, 1.0),
            Point2::new(2.0, 0.5),
            Point2::new(3.0, -0.5),
            Point2::new(4.0, 0.0),
        ],
        vec![0.0, 0.0, 0.0, 0.5, 0.5, 1.0, 1.0, 1.0],
    )
    .unwrap();
    assert!(matches!(
        maximum.insert_knot(0.5),
        Err(BSplineInsertionError::MaximumMultiplicity { .. })
    ));
    assert!(matches!(
        maximum.insert_knot(0.0),
        Err(BSplineInsertionError::ClampedEndpoint { .. })
    ));

    let basis = BSplineBasis::try_clamped(1, 2, vec![0.0, 0.0, 1.0, 1.0]).unwrap();
    assert!(matches!(
        BSplineCurve2::try_new(
            basis,
            vec![
                Point2::origin(),
                Point2::new(1.0, 0.0),
                Point2::new(2.0, 0.0)
            ]
        ),
        Err(BSplineDefinitionError::ControlCount {
            expected: 2,
            actual: 3
        })
    ));
}

#[test]
fn extreme_finite_span_width_preserves_zero_high_derivatives() {
    let curve = BSplineCurve2::try_clamped(
        1,
        vec![Point2::new(-2.0, 1.0), Point2::new(3.0, 4.0)],
        vec![0.0, 0.0, 1.0e200, 1.0e200],
    )
    .unwrap();
    let jet = curve
        .jet_on_span(curve.basis().spans()[0].index(), 0.5)
        .unwrap();
    assert_point(jet.position, Point2::new(0.5, 2.5), 1.0);
    assert_vector(jet.first_derivative, Vector2::new(5.0, 3.0), 1.0);
    assert_eq!(jet.second_derivative, Vector2::zeros());
    assert_eq!(jet.third_derivative, Vector2::zeros());
}

trait BasisTestExtension {
    fn jet_on_span_for_test(
        &self,
        span: geosolve_geometry::BSplineSpanIndex,
        parameter: f64,
    ) -> geosolve_geometry::BSplineBasisJet;
}

impl BasisTestExtension for BSplineBasis {
    fn jet_on_span_for_test(
        &self,
        span: geosolve_geometry::BSplineSpanIndex,
        parameter: f64,
    ) -> geosolve_geometry::BSplineBasisJet {
        self.basis_jet_on_span(span, parameter).unwrap()
    }
}

fn periodic_curve() -> BSplineCurve2 {
    BSplineCurve2::try_periodic(
        2,
        vec![
            Point2::new(0.0, 0.0),
            Point2::new(1.5, -0.2),
            Point2::new(2.0, 1.4),
            Point2::new(0.5, 2.2),
            Point2::new(-0.8, 1.0),
        ],
        vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0],
    )
    .unwrap()
}

fn assert_geometry_invariant(
    before: &BSplineCurve2,
    after: &BSplineCurve2,
    lower: f64,
    upper: f64,
) {
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
        (actual - expected).abs() <= CLOSE_TOLERANCE * scale.max(1.0),
        "actual={actual}, expected={expected}"
    );
}
