#![allow(clippy::too_many_lines)]

use geosolve_core::{HardValidity, SolverConfig};
use geosolve_geometry::{BSplineForm, Point2, Vector2};
use geosolve_sketch::{
    ArcSweep, ContactNeighborhood, CurveContactNeighborhood, CurveDefinition, CurveSpan,
    CurveTangentOrientation, DocumentBSplineForm, DocumentBSplineSpanDirection, DocumentCommand,
    DocumentCommandEffect, DocumentConstraintDefinition, DocumentEdit, DocumentError,
    DocumentSolveRequest, RuntimeCurve, Sketch, SketchCurve, SketchCurveContact, SketchDocument,
    SketchDocumentSession, SketchError, SketchSolveRequest, TangentOrientation,
};

#[test]
fn line_arc_tangency_activates_arc_angles_and_matches_finite_differences() {
    let mut sketch = Sketch::new(4.0).unwrap();
    let outer = sketch
        .add_point(Point2::new(1.356_127_064_091_652_7, -2.944_921_788_410_946))
        .unwrap();
    let contact = sketch
        .add_point(Point2::new(
            1.947_570_879_993_241_2,
            -0.618_939_369_596_958_4,
        ))
        .unwrap();
    let center = sketch.add_point(Point2::new(0.3, -0.2)).unwrap();
    let line = sketch.add_segment(outer, contact).unwrap();
    let arc = sketch
        .add_named_arc(
            "native arc",
            center,
            1.7,
            -1.1,
            1.2,
            ArcSweep::CounterClockwise,
        )
        .unwrap();
    let tangency = sketch
        .add_line_curve_tangency(
            line,
            geosolve_sketch::SegmentEndpoint::End,
            SketchCurveContact {
                curve: SketchCurve::Arc(arc),
                parameter: 0.37,
                neighborhood: CurveContactNeighborhood::Local {
                    lower: 0.2,
                    upper: 0.7,
                },
            },
            CurveTangentOrientation::Aligned,
        )
        .unwrap();

    let compiled = sketch.compile(SketchSolveRequest::default()).unwrap();
    assert_eq!(compiled.arc_angle_variables().len(), 2);
    let source = compiled
        .source_mappings()
        .iter()
        .find(|mapping| mapping.source == geosolve_sketch::SketchSource::Constraint(tangency))
        .unwrap();
    let residual = compiled.problem().residual(source.residual_ids[0]).unwrap();
    assert_eq!(residual.incident_variables().len(), 7);
    let jacobians = compiled.problem().check_jacobians(1.0e-6).unwrap();
    assert!(jacobians.max_absolute_error() <= 1.0e-6, "{jacobians:#?}");
}

#[test]
fn runtime_bspline_contacts_use_only_active_support_and_match_finite_differences() {
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
        let spline = sketch
            .add_named_bspline(
                "local cubic",
                BSplineForm::Clamped,
                3,
                controls.to_vec(),
                vec![0.0, 0.0, 0.0, 0.0, 0.25, 0.6, 0.8, 1.0, 1.0, 1.0, 1.0],
            )
            .unwrap();
        let span = sketch.bspline(spline).unwrap().basis().spans()[1].index();
        let parameter = 0.37;
        let jet = sketch.evaluate_bspline(spline, span, parameter).unwrap();
        for control in controls {
            sketch.add_fixed_point(control).unwrap();
        }
        let normal = Vector2::new(-jet.first_derivative.y, jet.first_derivative.x).normalize();
        let point = sketch
            .add_point(jet.position + normal * (0.15 * scale))
            .unwrap();
        let point_constraint = sketch
            .add_point_on_curve(
                point,
                SketchCurveContact {
                    curve: SketchCurve::BSpline { spline, span },
                    parameter,
                    neighborhood: CurveContactNeighborhood::Local {
                        lower: 0.1,
                        upper: 0.9,
                    },
                },
            )
            .unwrap();

        let line_start = sketch.add_point(jet.position).unwrap();
        let line_end = sketch
            .add_point(jet.position + jet.first_derivative.normalize() * scale)
            .unwrap();
        let line = sketch.add_segment(line_start, line_end).unwrap();
        let tangent_constraint = sketch
            .add_line_curve_tangency(
                line,
                geosolve_sketch::SegmentEndpoint::Start,
                SketchCurveContact {
                    curve: SketchCurve::BSpline { spline, span },
                    parameter,
                    neighborhood: CurveContactNeighborhood::Local {
                        lower: 0.1,
                        upper: 0.9,
                    },
                },
                CurveTangentOrientation::Aligned,
            )
            .unwrap();

        let compiled = sketch.compile(SketchSolveRequest::default()).unwrap();
        let jacobians = compiled.problem().check_jacobians(1.0e-6).unwrap();
        assert!(jacobians.all_within(1.0e-6), "{jacobians:#?}");

        let active = sketch.bspline(spline).unwrap().basis().span(span).unwrap();
        let inactive = controls
            .iter()
            .enumerate()
            .filter(|(index, _)| !active.support().contains(index))
            .map(|(_, point)| {
                compiled
                    .point_variables()
                    .iter()
                    .find(|mapping| mapping.point_id == *point)
                    .unwrap()
                    .variable_id
            })
            .collect::<Vec<_>>();
        for (constraint, expected_incidence) in [(point_constraint, 6), (tangent_constraint, 7)] {
            let mapping = compiled
                .source_mappings()
                .iter()
                .find(|mapping| {
                    mapping.source == geosolve_sketch::SketchSource::Constraint(constraint)
                })
                .unwrap();
            let residual = compiled
                .problem()
                .residual(mapping.residual_ids[0])
                .unwrap();
            assert_eq!(residual.incident_variables().len(), expected_incidence);
            assert!(
                inactive
                    .iter()
                    .all(|variable| !residual.incident_variables().contains(variable))
            );
        }

        let solved = sketch
            .solve(SketchSolveRequest::default(), SolverConfig::default())
            .unwrap();
        assert!(solved.accepted(), "{solved:#?}");
        assert_eq!(
            solved.unstable_core_report().hard_validity,
            HardValidity::Valid
        );
        assert!(solved.acceptance_hard_residual_max.unwrap() <= 1.0e-9);
        assert!(
            solved
                .display_audit
                .sources
                .iter()
                .all(|source| { source.rows.iter().all(|row| row.raw_residual.is_finite()) })
        );
    }
}

#[test]
fn persistent_bspline_spans_round_trip_lower_and_preserve_periodic_winding() {
    let mut document = SketchDocument::new(1.0).unwrap();
    let controls = [[0.0, 0.0], [1.5, -0.2], [2.0, 1.4], [0.5, 2.2], [-0.8, 1.0]]
        .map(|position| document.add_point("periodic control", position).unwrap());
    let curve = document
        .add_curve(
            "periodic quadratic",
            CurveDefinition::BSpline {
                form: DocumentBSplineForm::Periodic,
                degree: 2,
                controls: controls.to_vec(),
                knots: vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0],
                span_ids: vec![11, 17, 23, 29, 31],
                next_span_id: 32,
            },
        )
        .unwrap();
    let spans = document.curve_spans(curve).unwrap();
    assert_eq!(
        spans.iter().map(|span| span.segment).collect::<Vec<_>>(),
        vec![11, 17, 23, 29, 31]
    );
    let selected = CurveSpan { curve, segment: 31 };
    let expected = document.evaluate_curve_jet(selected, 0.3).unwrap();
    let contact = document
        .add_curve_contact(
            "periodic span contact",
            selected,
            0.3,
            2,
            ContactNeighborhood::Local {
                lower: 0.1,
                upper: 0.9,
            },
            None,
        )
        .unwrap();
    assert_eq!(document.contact(contact).unwrap().winding, 2);
    let contacted = document.evaluate_contact_jet(contact).unwrap();
    assert!((contacted.position - expected.position).norm() <= 1.0e-12);
    assert_eq!(
        document.bspline_continuity_at(curve, 0.0).unwrap(),
        Some(geosolve_sketch::BSplineContinuity::Guaranteed {
            multiplicity: 1,
            order: 1
        })
    );

    let point = document
        .add_point("contact point", [expected.position.x, expected.position.y])
        .unwrap();
    document
        .add_constraint(
            "point on periodic B-spline",
            DocumentConstraintDefinition::PointOnCurve { point, contact },
        )
        .unwrap();
    let lowered = document.lower().unwrap();
    assert!(matches!(
        lowered.mappings().runtime_curve(curve),
        Some(RuntimeCurve::BSpline { spans, .. }) if spans.len() == 5
    ));
    let compiled = lowered
        .sketch()
        .compile(SketchSolveRequest::default())
        .unwrap();
    assert!(
        compiled
            .problem()
            .check_jacobians(1.0e-6)
            .unwrap()
            .all_within(1.0e-6)
    );
    let session = SketchDocumentSession::new(
        document.clone(),
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    assert_eq!(session.document().contact(contact).unwrap().winding, 2);

    let json = document.to_canonical_json().unwrap();
    let restored = SketchDocument::from_json(&json).unwrap();
    assert_eq!(restored.to_canonical_json().unwrap(), json);
    assert_eq!(restored.contact(contact).unwrap().winding, 2);
    assert!(
        (restored.evaluate_contact_jet(contact).unwrap().position - expected.position).norm()
            <= 1.0e-12
    );
}

#[test]
fn malformed_persistent_bspline_identity_and_knots_reject_atomically() {
    let mut document = SketchDocument::new(1.0).unwrap();
    let controls = [
        document.add_point("p0", [0.0, 0.0]).unwrap(),
        document.add_point("p1", [1.0, 1.0]).unwrap(),
        document.add_point("p2", [2.0, 0.0]).unwrap(),
    ];
    let accepted_points = document.points().to_vec();
    assert!(matches!(
        document.add_curve(
            "duplicate controls",
            CurveDefinition::BSpline {
                form: DocumentBSplineForm::Clamped,
                degree: 2,
                controls: vec![controls[0], controls[1], controls[1]],
                knots: vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
                span_ids: vec![7],
                next_span_id: 8,
            }
        ),
        Err(DocumentError::InvalidField {
            field: "curve.controls",
            ..
        })
    ));
    assert_eq!(document.points(), accepted_points);
    assert!(document.curves().is_empty());

    assert!(matches!(
        document.add_curve(
            "bad spans",
            CurveDefinition::BSpline {
                form: DocumentBSplineForm::Clamped,
                degree: 2,
                controls: controls.to_vec(),
                knots: vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
                span_ids: vec![7, 9],
                next_span_id: 10,
            }
        ),
        Err(DocumentError::InvalidField {
            field: "curve.span_ids",
            ..
        })
    ));
    assert!(matches!(
        document.add_curve(
            "bad knots",
            CurveDefinition::BSpline {
                form: DocumentBSplineForm::Clamped,
                degree: 2,
                controls: controls.to_vec(),
                knots: vec![0.0, 0.0, 0.0, 0.8, 0.7, 1.0],
                span_ids: vec![7],
                next_span_id: 8,
            }
        ),
        Err(DocumentError::BSplineDefinition { .. })
    ));
    assert_eq!(document.points(), accepted_points);
    assert!(document.curves().is_empty());

    let mut runtime = Sketch::new(1.0).unwrap();
    let collapsed = [
        runtime.add_point(Point2::origin()).unwrap(),
        runtime.add_point(Point2::origin()).unwrap(),
        runtime.add_point(Point2::origin()).unwrap(),
    ];
    let spline = runtime
        .add_named_bspline(
            "collapsed",
            BSplineForm::Clamped,
            2,
            collapsed.to_vec(),
            vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
        )
        .unwrap();
    let point = runtime.add_point(Point2::origin()).unwrap();
    assert!(matches!(
        runtime.add_point_on_curve(
            point,
            SketchCurveContact {
                curve: SketchCurve::BSpline {
                    spline,
                    span: runtime.bspline(spline).unwrap().basis().spans()[0].index(),
                },
                parameter: 0.5,
                neighborhood: CurveContactNeighborhood::Interior,
            }
        ),
        Err(SketchError::InvalidCurveContact(_))
    ));

    let mut removable = Sketch::new(1.0).unwrap();
    let controls = [
        removable.add_point(Point2::new(0.0, 0.0)).unwrap(),
        removable.add_point(Point2::new(1.0, 1.0)).unwrap(),
        removable.add_point(Point2::new(2.0, 0.0)).unwrap(),
    ];
    let spline = removable
        .add_named_bspline(
            "removable",
            BSplineForm::Clamped,
            2,
            controls.to_vec(),
            vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
        )
        .unwrap();
    let span = removable.bspline(spline).unwrap().basis().spans()[0].index();
    let point = removable
        .add_point(
            removable
                .evaluate_bspline(spline, span, 0.5)
                .unwrap()
                .position,
        )
        .unwrap();
    let constraint = removable
        .add_point_on_curve(
            point,
            SketchCurveContact {
                curve: SketchCurve::BSpline { spline, span },
                parameter: 0.5,
                neighborhood: CurveContactNeighborhood::Interior,
            },
        )
        .unwrap();
    assert!(matches!(
        removable.remove_bspline(spline),
        Err(SketchError::BSplineInUse(id)) if id == spline
    ));
    removable.remove_constraint(constraint).unwrap();
    assert_eq!(
        removable.remove_bspline(spline).unwrap().controls(),
        controls
    );
}

#[test]
fn knot_insertion_preserves_geometry_ids_and_remaps_split_span_contacts() {
    let (mut document, curve, original_controls) = clamped_document();
    let left_contact = document
        .add_curve_contact(
            "left contact",
            CurveSpan { curve, segment: 41 },
            0.2,
            0,
            ContactNeighborhood::Local {
                lower: 0.1,
                upper: 0.3,
            },
            None,
        )
        .unwrap();
    let right_contact = document
        .add_curve_contact(
            "right contact",
            CurveSpan { curve, segment: 41 },
            0.8,
            0,
            ContactNeighborhood::Local {
                lower: 0.7,
                upper: 0.9,
            },
            None,
        )
        .unwrap();
    let knot_contact = document
        .add_curve_contact(
            "inserted knot contact",
            CurveSpan { curve, segment: 41 },
            0.5,
            0,
            ContactNeighborhood::Local {
                lower: 0.4,
                upper: 0.6,
            },
            None,
        )
        .unwrap();
    let left_position = document
        .evaluate_contact_jet(left_contact)
        .unwrap()
        .position;
    let right_position = document
        .evaluate_contact_jet(right_contact)
        .unwrap()
        .position;
    let samples = [0.0, 0.1, 0.25, 0.4, 0.5, 0.75, 1.0]
        .map(|native| document_bspline_position(&document, curve, native, &[41, 73]));

    let insertion = document.insert_bspline_knot(curve, 0.25).unwrap();
    assert_eq!(insertion.curve, curve);
    assert_eq!(insertion.new_span_id, Some(100));
    assert_eq!(
        insertion.migrated_contacts,
        vec![left_contact, right_contact, knot_contact]
    );
    assert_eq!(
        document
            .curve_spans(curve)
            .unwrap()
            .iter()
            .map(|span| span.segment)
            .collect::<Vec<_>>(),
        vec![41, 100, 73]
    );
    let CurveDefinition::BSpline {
        controls,
        next_span_id,
        ..
    } = &document.curve(curve).unwrap().definition
    else {
        panic!("expected B-spline")
    };
    assert_eq!(controls.len(), original_controls.len() + 1);
    assert_eq!(*next_span_id, 101);
    assert!(
        original_controls
            .iter()
            .all(|control| controls.contains(control))
    );
    assert!(controls.contains(&insertion.new_control));

    let left = document.contact(left_contact).unwrap();
    assert_eq!(left.curve.segment, 41);
    assert!((document.scalar(left.parameter).unwrap().value - 0.4).abs() <= 1.0e-12);
    assert!(matches!(
        left.neighborhood,
        ContactNeighborhood::Local { lower, upper }
            if (lower - 0.2).abs() <= 1.0e-12 && (upper - 0.6).abs() <= 1.0e-12
    ));
    let knot = document.contact(knot_contact).unwrap();
    assert_eq!(knot.curve.segment, 41);
    assert_eq!(knot.neighborhood, ContactNeighborhood::End);
    assert_eq!(
        document.scalar(knot.parameter).unwrap().value.to_bits(),
        1.0f64.to_bits()
    );
    let right = document.contact(right_contact).unwrap();
    assert_eq!(right.curve.segment, 100);
    assert!((document.scalar(right.parameter).unwrap().value - 0.6).abs() <= 1.0e-12);
    assert!(matches!(
        right.neighborhood,
        ContactNeighborhood::Local { lower, upper }
            if (lower - 0.4).abs() <= 1.0e-12 && (upper - 0.8).abs() <= 1.0e-12
    ));
    assert!(
        (document
            .evaluate_contact_jet(left_contact)
            .unwrap()
            .position
            - left_position)
            .norm()
            <= 1.0e-12
    );
    assert!(
        (document
            .evaluate_contact_jet(right_contact)
            .unwrap()
            .position
            - right_position)
            .norm()
            <= 1.0e-12
    );
    for (native, expected) in [0.0, 0.1, 0.25, 0.4, 0.5, 0.75, 1.0]
        .into_iter()
        .zip(samples)
    {
        let actual = document_bspline_position(&document, curve, native, &[41, 100, 73]);
        assert!((actual - expected).norm() <= 2.0e-12);
    }

    let repeated = document.insert_bspline_knot(curve, 0.5).unwrap();
    assert_eq!(repeated.new_span_id, None);
    assert_eq!(
        document
            .curve_spans(curve)
            .unwrap()
            .iter()
            .map(|span| span.segment)
            .collect::<Vec<_>>(),
        vec![41, 100, 73]
    );
    let accepted = document.to_canonical_json().unwrap();
    assert!(matches!(
        document.insert_bspline_knot(curve, 0.0),
        Err(DocumentError::BSplineInsertion { .. })
    ));
    assert_eq!(document.to_canonical_json().unwrap(), accepted);
}

#[test]
fn knot_insertion_is_one_accepted_undoable_structural_command() {
    let (document, curve, _) = clamped_document();
    let before = document.to_canonical_json().unwrap();
    let before_points = document.points().to_vec();
    let mut before_definition = document.curve(curve).unwrap().definition.clone();
    let mut session = SketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let outcome = session
        .apply(DocumentCommand::new(
            session.revision(),
            DocumentEdit::InsertBSplineKnot {
                curve,
                parameter: 0.25,
            },
        ))
        .unwrap();
    assert!(outcome.accepted(), "{outcome:#?}");
    assert!(matches!(
        outcome.effect,
        Some(DocumentCommandEffect::InsertedBSplineKnot(ref effect))
            if effect.curve == curve && effect.new_span_id == Some(100)
    ));
    let after = session.export_json().unwrap();
    assert_ne!(after, before);
    assert_eq!(session.history_len(), 1);
    session.undo(session.revision()).unwrap();
    assert_ne!(session.export_json().unwrap(), before);
    assert_eq!(session.document().points(), before_points);
    let CurveDefinition::BSpline { next_span_id, .. } = &mut before_definition else {
        panic!("expected B-spline")
    };
    *next_span_id = 101;
    assert_eq!(
        session.document().curve(curve).unwrap().definition,
        before_definition
    );
    session.redo(session.revision()).unwrap();
    assert_eq!(session.export_json().unwrap(), after);
}

#[test]
fn divergent_history_never_reuses_a_consumed_semantic_span_id() {
    let (document, curve, _) = clamped_document();
    let mut session = SketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let first = session
        .apply(DocumentCommand::new(
            session.revision(),
            DocumentEdit::InsertBSplineKnot {
                curve,
                parameter: 0.25,
            },
        ))
        .unwrap();
    assert!(matches!(
        first.effect,
        Some(DocumentCommandEffect::InsertedBSplineKnot(ref effect))
            if effect.new_span_id == Some(100)
    ));
    session.undo(session.revision()).unwrap();
    let divergent = session
        .apply(DocumentCommand::new(
            session.revision(),
            DocumentEdit::InsertBSplineKnot {
                curve,
                parameter: 0.75,
            },
        ))
        .unwrap();
    assert!(matches!(
        divergent.effect,
        Some(DocumentCommandEffect::InsertedBSplineKnot(ref effect))
            if effect.new_span_id == Some(101)
    ));
    assert_eq!(
        session
            .document()
            .curve_spans(curve)
            .unwrap()
            .iter()
            .map(|span| span.segment)
            .collect::<Vec<_>>(),
        vec![41, 73, 101]
    );
    assert!(!session.can_redo());
}

#[test]
fn span_transition_command_is_undoable_and_failed_transition_retains_history() {
    let (mut document, curve, _) = clamped_document();
    let contact = document
        .add_curve_contact(
            "transition command",
            CurveSpan { curve, segment: 41 },
            1.0,
            0,
            ContactNeighborhood::End,
            None,
        )
        .unwrap();
    let mut session = SketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let transitioned = session
        .apply(DocumentCommand::new(
            session.revision(),
            DocumentEdit::TransitionBSplineContact {
                contact,
                direction: DocumentBSplineSpanDirection::Next,
            },
        ))
        .unwrap();
    assert!(transitioned.accepted());
    assert_eq!(
        session.document().contact(contact).unwrap().curve.segment,
        73
    );
    assert_eq!(session.history_len(), 1);
    assert!(
        session
            .apply(DocumentCommand::new(
                session.revision(),
                DocumentEdit::TransitionBSplineContact {
                    contact,
                    direction: DocumentBSplineSpanDirection::Next,
                },
            ))
            .is_err()
    );
    assert_eq!(session.history_len(), 1);
    session.undo(session.revision()).unwrap();
    assert_eq!(
        session.document().contact(contact).unwrap().curve.segment,
        41
    );
    session.redo(session.revision()).unwrap();
    assert_eq!(
        session.document().contact(contact).unwrap().curve.segment,
        73
    );
}

#[test]
fn explicit_span_transitions_preserve_limits_winding_and_continuity_policy() {
    let (mut clamped, curve, _) = clamped_document();
    let contact = clamped
        .add_curve_contact(
            "clamped transition",
            CurveSpan { curve, segment: 41 },
            1.0,
            0,
            ContactNeighborhood::End,
            None,
        )
        .unwrap();
    let position = clamped.evaluate_contact_jet(contact).unwrap().position;
    clamped
        .transition_bspline_contact(contact, DocumentBSplineSpanDirection::Next)
        .unwrap();
    let transitioned = clamped.contact(contact).unwrap();
    assert_eq!(transitioned.curve.segment, 73);
    assert_eq!(transitioned.neighborhood, ContactNeighborhood::Start);
    assert_eq!(
        clamped
            .scalar(transitioned.parameter)
            .unwrap()
            .value
            .to_bits(),
        0.0f64.to_bits()
    );
    assert!((clamped.evaluate_contact_jet(contact).unwrap().position - position).norm() <= 1.0e-12);
    clamped
        .transition_bspline_contact(contact, DocumentBSplineSpanDirection::Previous)
        .unwrap();
    assert_eq!(clamped.contact(contact).unwrap().curve.segment, 41);

    let mut periodic = SketchDocument::new(1.0).unwrap();
    let controls = [[0.0, 0.0], [1.5, -0.2], [2.0, 1.4], [0.5, 2.2], [-0.8, 1.0]]
        .map(|value| periodic.add_point("periodic control", value).unwrap());
    let periodic_curve = periodic
        .add_curve(
            "periodic transition",
            CurveDefinition::BSpline {
                form: DocumentBSplineForm::Periodic,
                degree: 2,
                controls: controls.to_vec(),
                knots: vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0],
                span_ids: vec![11, 17, 23, 29, 31],
                next_span_id: 32,
            },
        )
        .unwrap();
    let periodic_contact = periodic
        .add_curve_contact(
            "periodic seam",
            CurveSpan {
                curve: periodic_curve,
                segment: 31,
            },
            1.0,
            2,
            ContactNeighborhood::End,
            None,
        )
        .unwrap();
    let seam = periodic
        .evaluate_contact_jet(periodic_contact)
        .unwrap()
        .position;
    periodic
        .transition_bspline_contact(periodic_contact, DocumentBSplineSpanDirection::Next)
        .unwrap();
    let transitioned = periodic.contact(periodic_contact).unwrap();
    assert_eq!(transitioned.curve.segment, 11);
    assert_eq!(transitioned.winding, 3);
    assert!(
        (periodic
            .evaluate_contact_jet(periodic_contact)
            .unwrap()
            .position
            - seam)
            .norm()
            <= 1.0e-12
    );

    let mut linear = SketchDocument::new(1.0).unwrap();
    let controls = [[0.0, 0.0], [1.0, 1.0], [2.0, 0.0]]
        .map(|value| linear.add_point("linear control", value).unwrap());
    let linear_curve = linear
        .add_curve(
            "C0 linear spline",
            CurveDefinition::BSpline {
                form: DocumentBSplineForm::Clamped,
                degree: 1,
                controls: controls.to_vec(),
                knots: vec![0.0, 0.0, 0.5, 1.0, 1.0],
                span_ids: vec![1, 2],
                next_span_id: 3,
            },
        )
        .unwrap();
    let tangent_contact = linear
        .add_curve_contact(
            "C0 tangent transition",
            CurveSpan {
                curve: linear_curve,
                segment: 1,
            },
            1.0,
            0,
            ContactNeighborhood::End,
            Some(TangentOrientation::Aligned),
        )
        .unwrap();
    let retained = linear.to_canonical_json().unwrap();
    assert!(matches!(
        linear.transition_bspline_contact(tangent_contact, DocumentBSplineSpanDirection::Next),
        Err(DocumentError::BSplineEvaluation { .. })
    ));
    assert_eq!(linear.to_canonical_json().unwrap(), retained);
}

#[test]
fn periodic_document_refinement_preserves_geometry_across_span_and_seam_insertions() {
    let mut document = SketchDocument::new(1.0).unwrap();
    let original_controls = [[0.0, 0.0], [1.5, -0.2], [2.0, 1.4], [0.5, 2.2], [-0.8, 1.0]]
        .map(|value| document.add_point("periodic control", value).unwrap());
    let curve = document
        .add_curve(
            "periodic refinement",
            CurveDefinition::BSpline {
                form: DocumentBSplineForm::Periodic,
                degree: 2,
                controls: original_controls.to_vec(),
                knots: vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0],
                span_ids: vec![11, 17, 23, 29, 31],
                next_span_id: 32,
            },
        )
        .unwrap();
    let native_parameters = [0.0, 0.2, 1.7, 2.4, 2.8, 4.9, 5.0];
    let before = native_parameters.map(|native| {
        periodic_document_position(&document, curve, native, &[11, 17, 23, 29, 31], None)
    });

    let insertion = document.insert_bspline_knot(curve, 2.4).unwrap();
    assert_eq!(insertion.new_span_id, Some(32));
    let split_spans = [11, 17, 23, 32, 29, 31];
    assert_eq!(
        document
            .curve_spans(curve)
            .unwrap()
            .iter()
            .map(|span| span.segment)
            .collect::<Vec<_>>(),
        split_spans
    );
    for (native, expected) in native_parameters.into_iter().zip(before) {
        let actual = periodic_document_position(&document, curve, native, &split_spans, Some(2.4));
        assert!((actual - expected).norm() <= 2.0e-12);
    }
    let CurveDefinition::BSpline { controls, .. } = &document.curve(curve).unwrap().definition
    else {
        panic!("expected B-spline")
    };
    assert!(
        original_controls
            .iter()
            .all(|control| controls.contains(control))
    );

    let before_seam = native_parameters.map(|native| {
        periodic_document_position(&document, curve, native, &split_spans, Some(2.4))
    });
    let seam = document.insert_bspline_knot(curve, 0.0).unwrap();
    assert_eq!(seam.new_span_id, None);
    assert_eq!(
        document
            .curve_spans(curve)
            .unwrap()
            .iter()
            .map(|span| span.segment)
            .collect::<Vec<_>>(),
        split_spans
    );
    for (native, expected) in native_parameters.into_iter().zip(before_seam) {
        let actual = periodic_document_position(&document, curve, native, &split_spans, Some(2.4));
        assert!((actual - expected).norm() <= 3.0e-12);
    }
}

fn clamped_document() -> (
    SketchDocument,
    geosolve_sketch::CurveId,
    Vec<geosolve_sketch::DesignPointId>,
) {
    let mut document = SketchDocument::new(1.0).unwrap();
    let controls = [[0.0, 0.0], [1.0, 2.0], [2.0, -1.0], [3.0, 1.5], [4.0, 0.0]]
        .map(|position| document.add_point("clamped control", position).unwrap())
        .to_vec();
    let curve = document
        .add_curve(
            "clamped cubic",
            CurveDefinition::BSpline {
                form: DocumentBSplineForm::Clamped,
                degree: 3,
                controls: controls.clone(),
                knots: vec![0.0, 0.0, 0.0, 0.0, 0.5, 1.0, 1.0, 1.0, 1.0],
                span_ids: vec![41, 73],
                next_span_id: 100,
            },
        )
        .unwrap();
    (document, curve, controls)
}

fn document_bspline_position(
    document: &SketchDocument,
    curve: geosolve_sketch::CurveId,
    native: f64,
    spans: &[u32],
) -> Point2<f64> {
    let (segment, local) = if spans.len() == 2 {
        if native <= 0.5 {
            (spans[0], native / 0.5)
        } else {
            (spans[1], (native - 0.5) / 0.5)
        }
    } else if native <= 0.25 {
        (spans[0], native / 0.25)
    } else if native <= 0.5 {
        (spans[1], (native - 0.25) / 0.25)
    } else {
        (spans[2], (native - 0.5) / 0.5)
    };
    document
        .evaluate_curve_jet(CurveSpan { curve, segment }, local)
        .unwrap()
        .position
}

fn periodic_document_position(
    document: &SketchDocument,
    curve: geosolve_sketch::CurveId,
    native: f64,
    spans: &[u32],
    inserted: Option<f64>,
) -> Point2<f64> {
    let native = if native.to_bits() == 5.0f64.to_bits() {
        5.0
    } else {
        native.rem_euclid(5.0)
    };
    let mut breaks = vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0];
    if let Some(inserted) = inserted {
        breaks.insert(3, inserted);
    }
    let ordinal = breaks
        .windows(2)
        .position(|pair| native >= pair[0] && native <= pair[1])
        .unwrap();
    let local = (native - breaks[ordinal]) / (breaks[ordinal + 1] - breaks[ordinal]);
    document
        .evaluate_curve_jet(
            CurveSpan {
                curve,
                segment: spans[ordinal],
            },
            local,
        )
        .unwrap()
        .position
}
