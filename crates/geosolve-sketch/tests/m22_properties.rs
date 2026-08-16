#![allow(clippy::too_many_lines)]

use geosolve_geometry::{BSplineForm, Point2};
use geosolve_sketch::{
    CurveContactNeighborhood, Sketch, SketchCurve, SketchCurveContact, SketchError,
    SketchSolveRequest,
};
use proptest::prelude::*;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(48))]

    #[test]
    fn construct_valid_nurbs_preserves_geometry_and_curvature_oracles(
        ordinates in prop::collection::vec(-4.0_f64..4.0, 6),
        weights in prop::collection::vec(0.2_f64..5.0, 6),
    ) {
        let controls = ordinates
            .iter()
            .enumerate()
            .map(|(index, ordinate)| Point2::new(as_f64(index), *ordinate))
            .collect::<Vec<_>>();
        let knots = vec![0.0, 0.0, 0.0, 0.0, 1.0 / 3.0, 2.0 / 3.0, 1.0, 1.0, 1.0, 1.0];
        let curve = geosolve_geometry::NurbsCurve2::try_clamped(
            3,
            controls.clone(),
            weights.clone(),
            knots.clone(),
        ).unwrap();
        let scaled = geosolve_geometry::NurbsCurve2::try_clamped(
            3,
            controls,
            weights.iter().map(|weight| weight * 1.0e-100).collect(),
            knots,
        ).unwrap();

        for span in curve.basis().spans() {
            for parameter in [0.2, 0.5, 0.8] {
                let actual = curve.jet_on_span(span.index(), parameter).unwrap();
                let common_scaled = scaled.jet_on_span(span.index(), parameter).unwrap();
                prop_assert!((actual.position - common_scaled.position).norm() <= 2.0e-10);
                prop_assert!((actual.first_derivative - common_scaled.first_derivative).norm() <= 2.0e-9);
            }

            let parameter = 0.43;
            let step = 5.0e-6;
            let before = curve
                .jet_on_span(span.index(), parameter - step)
                .unwrap()
                .differential()
                .unwrap();
            let current_jet = curve.jet_on_span(span.index(), parameter).unwrap();
            let current = current_jet.differential().unwrap();
            let after = curve
                .jet_on_span(span.index(), parameter + step)
                .unwrap()
                .differential()
                .unwrap();
            let tangent_rate = (after.unit_tangent - before.unit_tangent) / (2.0 * step);
            let oracle = tangent_rate.dot(&current.left_normal) / current_jet.first_derivative.norm();
            let scale = oracle.abs().max(current.signed_curvature.abs()).max(1.0e-8);
            prop_assert!(
                (oracle - current.signed_curvature).abs() / scale <= 2.0e-5,
                "span={:?} oracle={oracle:.17e} curvature={:.17e} scale={scale:.17e} relative={:.17e}",
                span.index(),
                current.signed_curvature,
                (oracle - current.signed_curvature).abs() / scale,
            );
        }

        let refined = curve.insert_knot(0.47).unwrap();
        for index in 0..=40 {
            let parameter = f64::from(index) / 40.0;
            let side = if index == 40 {
                geosolve_geometry::BSplineKnotSide::Left
            } else {
                geosolve_geometry::BSplineKnotSide::Right
            };
            let before = curve.jet_at(parameter, side).unwrap();
            let after = refined.curve().jet_at(parameter, side).unwrap();
            prop_assert!((before.position - after.position).norm() <= 2.0e-9);
            prop_assert!((before.first_derivative - after.first_derivative).norm() <= 2.0e-8);
        }
    }

    #[test]
    fn malformed_weight_inputs_never_construct_or_mutate(invalid_weight in -1.0e6_f64..=0.0) {
        let mut sketch = Sketch::new(1.0).unwrap();
        let controls = [Point2::origin(), Point2::new(1.0, 1.0), Point2::new(2.0, 0.0)]
            .map(|point| sketch.add_point(point).unwrap());
        let before = sketch.points().count();
        let result = sketch.add_named_nurbs(
            "invalid property NURBS",
            BSplineForm::Clamped,
            2,
            controls.to_vec(),
            vec![1.0, invalid_weight, 1.0],
            0,
            vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
        );
        prop_assert!(matches!(result, Err(SketchError::InvalidNurbs(_))));
        prop_assert_eq!(sketch.nurbs_curves().count(), 0);
        prop_assert_eq!(sketch.points().count(), before);
    }
}

#[test]
fn large_sparse_nurbs_corpus_keeps_every_contact_degree_local() {
    const CONTROL_COUNT: usize = 1_000;
    const CONTACT_COUNT: usize = 128;
    const DEGREE: u32 = 3;

    let mut sketch = Sketch::new(1.0).unwrap();
    let controls = (0..CONTROL_COUNT)
        .map(|index| {
            let x = as_f64(index) * 0.01;
            sketch.add_point(Point2::new(x, (x * 0.7).sin())).unwrap()
        })
        .collect::<Vec<_>>();
    let mut knots = vec![0.0; DEGREE as usize + 1];
    let span_count = CONTROL_COUNT - DEGREE as usize;
    knots.extend((1..span_count).map(|index| as_f64(index) / as_f64(span_count)));
    knots.extend(std::iter::repeat_n(1.0, DEGREE as usize + 1));
    let mut weights = (0..CONTROL_COUNT)
        .map(|index| 0.75 + as_f64(index % 11) * 0.05)
        .collect::<Vec<_>>();
    weights[0] = 1.0;
    let nurbs = sketch
        .add_named_nurbs(
            "large local NURBS",
            BSplineForm::Clamped,
            DEGREE,
            controls,
            weights,
            0,
            knots,
        )
        .unwrap();
    let spans = sketch.nurbs(nurbs).unwrap().basis().spans().to_vec();
    let mut constraints = Vec::new();
    for sample in 0..CONTACT_COUNT {
        let span = spans[sample * spans.len() / CONTACT_COUNT].index();
        let parameter = 0.37;
        let point = sketch
            .add_point(
                sketch
                    .evaluate_nurbs(nurbs, span, parameter)
                    .unwrap()
                    .position,
            )
            .unwrap();
        constraints.push(
            sketch
                .add_point_on_curve(
                    point,
                    SketchCurveContact {
                        curve: SketchCurve::Nurbs { nurbs, span },
                        parameter,
                        neighborhood: CurveContactNeighborhood::Local {
                            lower: 0.1,
                            upper: 0.9,
                        },
                    },
                )
                .unwrap(),
        );
    }

    let compiled = sketch
        .compile(SketchSolveRequest::default().without_previous_state_preferences())
        .unwrap();
    assert_eq!(compiled.nurbs_weight_variables().len(), CONTROL_COUNT - 1);
    for constraint in constraints {
        let mapping = compiled
            .source_mappings()
            .iter()
            .find(|mapping| mapping.source == geosolve_sketch::SketchSource::Constraint(constraint))
            .unwrap();
        let residual = compiled
            .problem()
            .residual(mapping.residual_ids[0])
            .unwrap();
        assert!(
            residual.incident_variables().len() <= 1 + 4 + 4 + 1,
            "contact incidence grew with total control count"
        );
    }
}

fn as_f64(value: usize) -> f64 {
    f64::from(u32::try_from(value).expect("test corpus index fits u32"))
}
