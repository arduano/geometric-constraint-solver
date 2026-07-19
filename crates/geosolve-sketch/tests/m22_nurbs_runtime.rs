use geosolve_core::{HardValidity, SolverConfig};
use geosolve_geometry::{BSplineForm, Point2};
use geosolve_sketch::{
    CurveContactNeighborhood, Sketch, SketchCurve, SketchCurveContact, SketchError,
    SketchSolveRequest,
};

#[test]
fn runtime_nurbs_uses_local_control_and_weight_incidence_with_gauge_removed() {
    for scale in [1.0e-6, 1.0, 1.0e6] {
        let mut sketch = Sketch::new(scale).unwrap();
        let controls = [
            Point2::new(0.0, 0.0),
            Point2::new(1.0 * scale, 1.5 * scale),
            Point2::new(2.0 * scale, -0.4 * scale),
            Point2::new(3.0 * scale, 1.2 * scale),
            Point2::new(4.0 * scale, -0.3 * scale),
            Point2::new(5.0 * scale, 0.8 * scale),
            Point2::new(6.0 * scale, 0.0),
        ]
        .map(|point| sketch.add_point(point).unwrap());
        let nurbs = sketch
            .add_named_nurbs(
                "local rational cubic",
                BSplineForm::Clamped,
                3,
                controls.to_vec(),
                vec![0.8, 1.2, 0.6, 1.0, 1.7, 0.75, 1.3],
                3,
                vec![0.0, 0.0, 0.0, 0.0, 0.25, 0.6, 0.8, 1.0, 1.0, 1.0, 1.0],
            )
            .unwrap();
        let span = sketch.nurbs(nurbs).unwrap().basis().spans()[1].index();
        let parameter = 0.37;
        let jet = sketch.evaluate_nurbs(nurbs, span, parameter).unwrap();
        for control in controls {
            sketch.add_fixed_point(control).unwrap();
        }
        let point = sketch.add_point(jet.position).unwrap();
        let constraint = sketch
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
            .unwrap();

        let compiled = sketch.compile(SketchSolveRequest::default()).unwrap();
        let jacobians = compiled.problem().check_jacobians(1.0e-6).unwrap();
        assert!(jacobians.all_within(1.0e-6), "{jacobians:#?}");
        assert_eq!(compiled.nurbs_weight_variables().len(), 6);
        assert!(compiled.variable_for_nurbs_weight(nurbs, 3).is_none());

        let active = sketch.nurbs(nurbs).unwrap().basis().span(span).unwrap();
        let mapping = compiled
            .source_mappings()
            .iter()
            .find(|mapping| mapping.source == geosolve_sketch::SketchSource::Constraint(constraint))
            .unwrap();
        let residual = compiled
            .problem()
            .residual(mapping.residual_ids[0])
            .unwrap();
        let active_non_gauge = active.support().iter().filter(|index| **index != 3).count();
        assert_eq!(
            residual.incident_variables().len(),
            1 + active.support().len() + active_non_gauge + 1
        );

        let inactive_controls = controls
            .iter()
            .enumerate()
            .filter(|(index, _)| !active.support().contains(index))
            .map(|(_, point)| compiled.variable_for_point(*point).unwrap());
        assert!(
            inactive_controls
                .into_iter()
                .all(|variable| !residual.incident_variables().contains(&variable))
        );
        let inactive_weights = compiled
            .nurbs_weight_variables()
            .iter()
            .filter(|mapping| !active.support().contains(&mapping.control_index));
        assert!(
            inactive_weights
                .into_iter()
                .all(|mapping| !residual.incident_variables().contains(&mapping.variable_id))
        );

        let solved = sketch
            .solve(SketchSolveRequest::default(), SolverConfig::default())
            .unwrap();
        assert!(solved.accepted(), "{solved:#?}");
        assert_eq!(solved.core_report.hard_validity, HardValidity::Valid);
        assert!(solved.acceptance_hard_residual_max.unwrap() <= 1.0e-9);
        assert_eq!(
            solved.geometry.nurbs(nurbs).unwrap().weights[3].to_bits(),
            1.0f64.to_bits()
        );
    }
}

#[test]
fn runtime_nurbs_rejects_direct_gauge_and_invalid_weight_edits() {
    let mut sketch = Sketch::new(1.0).unwrap();
    let controls = [
        Point2::origin(),
        Point2::new(1.0, 1.0),
        Point2::new(2.0, 0.0),
    ]
    .map(|point| sketch.add_point(point).unwrap());
    let nurbs = sketch
        .add_named_nurbs(
            "quadratic NURBS",
            BSplineForm::Clamped,
            2,
            controls.to_vec(),
            vec![1.0, 0.7, 1.2],
            0,
            vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
        )
        .unwrap();
    assert_eq!(
        sketch.set_nurbs_weight(nurbs, 0, 2.0),
        Err(SketchError::NurbsGaugeWeightEdit(nurbs))
    );
    assert!(matches!(
        sketch.set_nurbs_weight(nurbs, 1, 0.0),
        Err(SketchError::InvalidNurbsWeight { index: 1, .. })
    ));
    assert_eq!(sketch.nurbs(nurbs).unwrap().weights(), &[1.0, 0.7, 1.2]);
}
