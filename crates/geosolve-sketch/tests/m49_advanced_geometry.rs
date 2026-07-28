// SPDX-License-Identifier: GPL-3.0-or-later

use std::f64::consts::TAU;

use geosolve_core::{HardValidity, SolverConfig};
use geosolve_sketch::{
    AlphaScenarioIds, AlphaScenarioKind, CurveDefinition, CurveId, DesignScalarId,
    DocumentBSplineForm, DocumentCommand, DocumentCommandEffect, DocumentEdit,
    DocumentHyperbolaBranch, DocumentObjectId, ScalarDomain, ScalarUnit, SketchDocument,
    SketchDocumentSession, VisualProfileOptions, VisualProfileStatus, alpha_scenario,
};

fn assert_independently_valid(session: &SketchDocumentSession) {
    let accepted = session.accepted_result();
    let solve = accepted.solve();
    assert!(accepted.accepted(), "{solve:#?}");
    assert_eq!(
        solve.core_report.hard_validity,
        HardValidity::Valid,
        "{solve:#?}"
    );
    assert!(
        solve.acceptance_hard_residual_max.unwrap() <= 1.0e-9,
        "{solve:#?}"
    );
}

fn assert_point_close(actual: [f64; 2], expected: [f64; 2]) {
    assert!(
        (actual[0] - expected[0]).hypot(actual[1] - expected[1]) <= 1.0e-12,
        "actual={actual:?}, expected={expected:?}"
    );
}

fn assert_straight_visible_intervals() {
    let mut straight = SketchDocument::new(4.0).unwrap();
    let straight_points = [[-1.0, 0.5], [2.0, 0.5], [2.0, 3.0]]
        .map(|position| straight.add_point("straight point", position).unwrap());
    let line = straight
        .add_curve(
            "line",
            CurveDefinition::Line {
                start: straight_points[0],
                end: straight_points[1],
                branch_direction: [1.0, 0.0],
            },
        )
        .unwrap();
    let polyline = straight
        .add_curve(
            "polyline",
            CurveDefinition::Polyline {
                points: straight_points.to_vec(),
                closed: false,
                branch_directions: vec![[1.0, 0.0], [0.0, 1.0]],
            },
        )
        .unwrap();
    for (curve, expected_segments) in [
        (line, vec![[straight_points[0], straight_points[1]]]),
        (
            polyline,
            vec![
                [straight_points[0], straight_points[1]],
                [straight_points[1], straight_points[2]],
            ],
        ),
    ] {
        let intervals = straight.visible_curve_intervals(curve).unwrap();
        assert_eq!(intervals.len(), expected_segments.len());
        for (interval, [expected_start, expected_end]) in intervals.iter().zip(expected_segments) {
            assert_eq!(interval.start.to_bits(), 0.0f64.to_bits());
            assert_eq!(interval.end.to_bits(), 1.0f64.to_bits());
            let start = straight
                .evaluate_curve_jet(interval.support, interval.start)
                .unwrap()
                .position;
            let end = straight
                .evaluate_curve_jet(interval.support, interval.end)
                .unwrap()
                .position;
            assert_point_close(
                [start.x, start.y],
                straight.point(expected_start).unwrap().position,
            );
            assert_point_close(
                [end.x, end.y],
                straight.point(expected_end).unwrap().position,
            );
        }
    }
}

fn assert_full_ellipse_visible_interval() {
    let mut ellipse = SketchDocument::new(2.0).unwrap();
    let center = ellipse.add_point("center", [1.0, -2.0]).unwrap();
    let axis = ellipse.add_point("axis", [3.0, -2.0]).unwrap();
    let ratio = ellipse
        .add_scalar(
            "ratio",
            0.5,
            ScalarUnit::Parameter,
            ScalarDomain::Bounded {
                lower: f64::from_bits(1),
                upper: 1.0,
            },
        )
        .unwrap();
    let full = ellipse
        .add_curve(
            "full ellipse",
            CurveDefinition::Ellipse {
                center,
                major_axis_point: axis,
                minor_axis_ratio: ratio,
            },
        )
        .unwrap();
    let intervals = ellipse.visible_curve_intervals(full).unwrap();
    assert_eq!(intervals.len(), 1);
    assert_eq!(intervals[0].start.to_bits(), 0.0f64.to_bits());
    assert_eq!(intervals[0].end.to_bits(), TAU.to_bits());
    let start = ellipse
        .evaluate_curve_jet(intervals[0].support, intervals[0].start)
        .unwrap()
        .position;
    let end = ellipse
        .evaluate_curve_jet(intervals[0].support, intervals[0].end)
        .unwrap()
        .position;
    assert_point_close([start.x, start.y], [end.x, end.y]);
}

fn assert_spline_visible_intervals() {
    let mut spline = SketchDocument::new(1.0).unwrap();
    let controls = [[0.0, 0.0], [1.0, 2.0], [2.0, -1.0], [3.0, 1.5], [4.0, 0.0]]
        .map(|point| spline.add_point("control", point).unwrap());
    let curve = spline
        .add_curve(
            "B-spline",
            CurveDefinition::BSpline {
                form: DocumentBSplineForm::Clamped,
                degree: 3,
                controls: controls.to_vec(),
                knots: vec![0.0, 0.0, 0.0, 0.0, 0.5, 1.0, 1.0, 1.0, 1.0],
                span_ids: vec![41, 73],
                next_span_id: 74,
            },
        )
        .unwrap();
    let intervals = spline.visible_curve_intervals(curve).unwrap();
    assert_eq!(
        intervals
            .iter()
            .map(|interval| interval.support.segment)
            .collect::<Vec<_>>(),
        [41, 73]
    );
    for interval in &intervals {
        spline
            .evaluate_curve_jet(interval.support, interval.start)
            .unwrap();
        spline
            .evaluate_curve_jet(interval.support, interval.end)
            .unwrap();
    }
    for boundary in intervals.windows(2) {
        let first_end = spline
            .evaluate_curve_jet(boundary[0].support, boundary[0].end)
            .unwrap()
            .position;
        let second_start = spline
            .evaluate_curve_jet(boundary[1].support, boundary[1].start)
            .unwrap()
            .position;
        assert_point_close([first_end.x, first_end.y], [second_start.x, second_start.y]);
    }
}

fn assert_failed_nurbs_has_no_connector() {
    let mut collapsed = SketchDocument::new(1.0).unwrap();
    let controls = [[0.0, 0.0], [0.0, 0.0]]
        .map(|point| collapsed.add_point("collapsed control", point).unwrap());
    let weights = [1.0, 1.0].map(|weight| {
        collapsed
            .add_scalar(
                "weight",
                weight,
                ScalarUnit::Parameter,
                ScalarDomain::Positive,
            )
            .unwrap()
    });
    let failed = collapsed
        .add_curve(
            "collapsed NURBS",
            CurveDefinition::Nurbs {
                form: DocumentBSplineForm::Clamped,
                degree: 1,
                controls: controls.to_vec(),
                weights: weights.to_vec(),
                gauge_weight: weights[0],
                knots: vec![0.0, 0.0, 1.0, 1.0],
                span_ids: vec![9],
                next_span_id: 10,
            },
        )
        .unwrap();
    let interval = collapsed.visible_curve_intervals(failed).unwrap().remove(0);
    let failure = collapsed.evaluate_curve_jet(interval.support, interval.start);
    assert!(
        failure.is_err(),
        "collapsed NURBS must not fabricate a connector: {failure:#?}"
    );
}

#[test]
fn accepted_nurbs_edit_refreshes_profile_roots_and_directed_edges_keep_parameters() {
    let fixture = alpha_scenario(AlphaScenarioKind::ProfileNurbsSelfIntersection, 1.0).unwrap();
    let AlphaScenarioIds::ProfileNurbsSelfIntersection(ids) = fixture.ids else {
        panic!("NURBS profile IDs expected");
    };
    let mut session =
        SketchDocumentSession::new(fixture.document, fixture.request, SolverConfig::default())
            .unwrap();
    assert_independently_valid(&session);
    let before = session
        .document()
        .analyze_visual_profiles(VisualProfileOptions::default());
    assert_eq!(before.status, VisualProfileStatus::Complete, "{before:#?}");
    let before_root = before
        .intersections
        .iter()
        .find(|root| root.first_span.curve == ids.curve && root.second_span.curve == ids.curve)
        .cloned()
        .expect("certified self root before edit");

    let control = ids.primary_control;
    let previous = session.document().point(control).unwrap().position;
    let edit = session
        .apply(DocumentCommand::new(
            session.revision(),
            DocumentEdit::SetPointPosition {
                point: control,
                position: [previous[0] + 0.05, previous[1] - 0.03],
            },
        ))
        .unwrap();
    assert!(edit.accepted(), "{edit:#?}");
    assert_independently_valid(&session);
    let after = session
        .document()
        .analyze_visual_profiles(VisualProfileOptions::default());
    assert_eq!(after.status, VisualProfileStatus::Complete, "{after:#?}");
    let after_root = after
        .intersections
        .iter()
        .find(|root| root.first_span.curve == ids.curve && root.second_span.curve == ids.curve)
        .expect("fresh certified self root after edit");
    assert_ne!(
        after_root.position_enclosure,
        before_root.position_enclosure
    );

    let curved = alpha_scenario(AlphaScenarioKind::ProfileCurvedTopology, 1.0)
        .unwrap()
        .document
        .analyze_visual_profiles(VisualProfileOptions::default());
    assert_eq!(curved.status, VisualProfileStatus::Complete, "{curved:#?}");
    let reverse = curved
        .faces
        .iter()
        .flat_map(|face| &face.contours)
        .flat_map(|contour| &contour.edges)
        .find(|edge| edge.source_parameters[0] > edge.source_parameters[1])
        .expect("a reverse-directed curved profile edge");
    let start = alpha_scenario(AlphaScenarioKind::ProfileCurvedTopology, 1.0)
        .unwrap()
        .document;
    let start_jet = start
        .evaluate_curve_jet(reverse.source_span, reverse.source_parameters[0])
        .unwrap();
    let end_jet = start
        .evaluate_curve_jet(reverse.source_span, reverse.source_parameters[1])
        .unwrap();
    assert_point_close([start_jet.position.x, start_jet.position.y], reverse.start);
    assert_point_close([end_jet.position.x, end_jet.position.y], reverse.end);
}

#[test]
fn public_intervals_cover_ellipse_period_spline_spans_and_failed_nurbs_has_no_connector() {
    assert_straight_visible_intervals();
    assert_full_ellipse_visible_interval();
    assert_spline_visible_intervals();
    assert_failed_nurbs_has_no_connector();
}

fn assert_conic_scenarios_are_independently_valid() {
    for scale in [1.0e-6, 1.0, 1.0e6] {
        for kind in [
            AlphaScenarioKind::ConicGallery,
            AlphaScenarioKind::ConicTangency,
            AlphaScenarioKind::ConicCircleLimit,
        ] {
            let fixture = alpha_scenario(kind, scale).unwrap();
            let session = SketchDocumentSession::new(
                fixture.document,
                fixture.request,
                SolverConfig::default(),
            )
            .unwrap();
            assert_independently_valid(&session);
        }
    }
}

fn assert_curve_deletion_lifecycle(
    session: &mut SketchDocumentSession,
    curve: CurveId,
    scalars: &[DesignScalarId],
) {
    let remaining_curves = session
        .document()
        .curves()
        .iter()
        .map(|candidate| candidate.id)
        .filter(|candidate| *candidate != curve)
        .collect::<Vec<_>>();
    let history = session.history_len();
    let deleted = session
        .apply(DocumentCommand::new(
            session.revision(),
            DocumentEdit::Delete {
                object: DocumentObjectId::Curve(curve),
            },
        ))
        .unwrap();
    assert!(deleted.accepted(), "{deleted:#?}");
    assert_eq!(
        deleted.effect,
        Some(DocumentCommandEffect::Deleted(DocumentObjectId::Curve(
            curve
        )))
    );
    assert_eq!(session.history_len(), history + 1);
    assert_independently_valid(session);
    assert!(session.document().curve(curve).is_none());
    assert!(
        scalars
            .iter()
            .all(|scalar| session.document().scalar(*scalar).is_none())
    );
    assert!(
        remaining_curves
            .iter()
            .all(|other| session.document().curve(*other).is_some())
    );

    let undone = session.undo(session.revision()).unwrap();
    assert!(undone.accepted(), "{undone:#?}");
    assert_independently_valid(session);
    assert!(session.document().curve(curve).is_some());
    assert!(
        scalars
            .iter()
            .all(|scalar| session.document().scalar(*scalar).is_some())
    );

    let redone = session.redo(session.revision()).unwrap();
    assert!(redone.accepted(), "{redone:#?}");
    assert_independently_valid(session);
    assert!(session.document().curve(curve).is_none());
}

fn assert_conic_gallery_lifecycle() {
    let fixture = alpha_scenario(AlphaScenarioKind::ConicGallery, 1.0).unwrap();
    let AlphaScenarioIds::ConicGallery(ids) = fixture.ids else {
        panic!("conic gallery IDs expected");
    };
    let mut session =
        SketchDocumentSession::new(fixture.document, fixture.request, SolverConfig::default())
            .unwrap();
    assert_independently_valid(&session);
    let [ellipse, arc, rational, parabola, hyperbola] = ids.curves;
    let CurveDefinition::Ellipse {
        minor_axis_ratio, ..
    } = session.document().curve(ellipse).unwrap().definition
    else {
        panic!("ellipse definition expected");
    };
    let CurveDefinition::EllipticalArc {
        minor_axis_ratio: arc_minor_axis_ratio,
        start_angle: arc_start_angle,
        end_angle: arc_end_angle,
        sweep: geosolve_sketch::DocumentArcSweep::Clockwise,
        ..
    } = session.document().curve(arc).unwrap().definition
    else {
        panic!("clockwise elliptical-arc definition expected");
    };
    let CurveDefinition::RationalQuadraticConic { middle_weight, .. } =
        session.document().curve(rational).unwrap().definition
    else {
        panic!("rational conic definition expected");
    };
    let CurveDefinition::ParabolaSegment {
        trim_start: parabola_trim_start,
        trim_end: parabola_trim_end,
        ..
    } = session.document().curve(parabola).unwrap().definition
    else {
        panic!("parabola definition expected");
    };
    assert!(matches!(
        session.document().curve(hyperbola).unwrap().definition,
        CurveDefinition::HyperbolaSegment {
            branch: DocumentHyperbolaBranch::Negative,
            ..
        }
    ));

    let history = session.history_len();
    let branch = session
        .apply(DocumentCommand::new(
            session.revision(),
            DocumentEdit::SetHyperbolaBranch {
                curve: hyperbola,
                branch: DocumentHyperbolaBranch::Positive,
            },
        ))
        .unwrap();
    assert!(branch.accepted());
    assert_eq!(session.history_len(), history + 1);
    assert_independently_valid(&session);
    let CurveDefinition::HyperbolaSegment {
        semi_conjugate,
        trim_start: hyperbola_trim_start,
        trim_end: hyperbola_trim_end,
        branch: DocumentHyperbolaBranch::Positive,
        ..
    } = session.document().curve(hyperbola).unwrap().definition
    else {
        panic!("positive hyperbola branch expected");
    };

    let owned_scalars = [
        (ellipse, vec![minor_axis_ratio]),
        (
            arc,
            vec![arc_minor_axis_ratio, arc_start_angle, arc_end_angle],
        ),
        (rational, vec![middle_weight]),
        (parabola, vec![parabola_trim_start, parabola_trim_end]),
        (
            hyperbola,
            vec![semi_conjugate, hyperbola_trim_start, hyperbola_trim_end],
        ),
    ];
    for (curve, scalars) in owned_scalars {
        assert_curve_deletion_lifecycle(&mut session, curve, &scalars);
    }
}

#[test]
fn five_conic_lifecycle_and_legacy_signatures_preserve_typed_state_and_validation() {
    assert_conic_scenarios_are_independently_valid();
    assert_conic_gallery_lifecycle();
}
