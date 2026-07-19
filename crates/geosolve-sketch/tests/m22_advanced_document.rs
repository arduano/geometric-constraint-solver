#![allow(clippy::too_many_lines)]

use geosolve_core::SolverConfig;
use geosolve_geometry::CurveDifferentialError;
use geosolve_sketch::{
    ContactId, ContactNeighborhood, CurveDefinition, CurveSpan, DocumentBSplineForm,
    DocumentBSplineSpanDirection, DocumentCommand, DocumentConstraintDefinition,
    DocumentCurveContinuity, DocumentCurveCurvatureRelation, DocumentCurveDirectionRelation,
    DocumentCurveMeasurementError, DocumentCurveMeasurementKind, DocumentCurveNormalSide,
    DocumentEdit, DocumentError, DocumentObjectId, DocumentSessionError, DocumentSolveRequest,
    PersistentId, RuntimeCurve, RuntimeSource, ScalarDomain, ScalarUnit, SketchConstraintKind,
    SketchCurve, SketchDocument, SketchDocumentSession,
};

#[test]
fn canonical_json_and_lowering_preserve_every_advanced_discrete_choice() {
    let mut document = SketchDocument::new(1.0).unwrap();
    let first_circle = add_circle(&mut document, "first circle", [0.0, 0.0], 2.0);
    let second_circle = add_circle(&mut document, "second circle", [5.0, 0.0], 2.0);
    let normal_start = document.add_point("normal start", [2.0, 0.0]).unwrap();
    let normal_end = document.add_point("normal end", [1.0, 0.0]).unwrap();
    let normal_line = document
        .add_curve(
            "left normal",
            CurveDefinition::Line {
                start: normal_start,
                end: normal_end,
                branch_direction: [-1.0, 0.0],
            },
        )
        .unwrap();
    let normal_contact = document
        .add_curve_contact(
            "normal contact",
            CurveSpan::line(first_circle),
            0.0,
            0,
            ContactNeighborhood::Interior,
            None,
        )
        .unwrap();
    let direction = document
        .add_constraint(
            "explicit left normal",
            DocumentConstraintDefinition::CurveDirection {
                line: CurveSpan::line(normal_line),
                curve_contact: normal_contact,
                relation: DocumentCurveDirectionRelation::Normal {
                    side: DocumentCurveNormalSide::Left,
                },
            },
        )
        .unwrap();

    let first_curvature = document
        .add_curve_contact(
            "first curvature",
            CurveSpan::line(first_circle),
            1.0,
            0,
            ContactNeighborhood::Interior,
            None,
        )
        .unwrap();
    let second_curvature = document
        .add_curve_contact(
            "second curvature",
            CurveSpan::line(second_circle),
            1.0,
            0,
            ContactNeighborhood::Interior,
            None,
        )
        .unwrap();
    let equal_curvature = document
        .add_constraint(
            "same-sign curvature",
            DocumentConstraintDefinition::EqualCurvature {
                first_contact: first_curvature,
                second_contact: second_curvature,
                relation: DocumentCurveCurvatureRelation::MagnitudeSameSign,
            },
        )
        .unwrap();

    let first_controls = [[-1.0, 1.0], [-0.5, 0.0], [0.0, 0.0]]
        .map(|position| document.add_point("incoming control", position).unwrap());
    let second_controls = [
        first_controls[2],
        document.add_point("outgoing control", [1.0, 0.0]).unwrap(),
        document.add_point("outgoing control", [2.0, 4.0]).unwrap(),
    ];
    let incoming = document
        .add_curve(
            "incoming parabola",
            CurveDefinition::QuadraticBezier {
                controls: first_controls,
            },
        )
        .unwrap();
    let outgoing = document
        .add_curve(
            "outgoing reparameterized parabola",
            CurveDefinition::QuadraticBezier {
                controls: second_controls,
            },
        )
        .unwrap();
    let incoming_end = document
        .add_curve_contact(
            "incoming end",
            CurveSpan::line(incoming),
            1.0,
            0,
            ContactNeighborhood::End,
            None,
        )
        .unwrap();
    let outgoing_start = document
        .add_curve_contact(
            "outgoing start",
            CurveSpan::line(outgoing),
            0.0,
            0,
            ContactNeighborhood::Start,
            None,
        )
        .unwrap();
    let continuity = document
        .add_constraint(
            "rate-explicit C2",
            DocumentConstraintDefinition::EndpointContinuity {
                first_contact: incoming_end,
                second_contact: outgoing_start,
                continuity: DocumentCurveContinuity::ParametricC2 {
                    first_rate: 2.0,
                    second_rate: 1.0,
                },
            },
        )
        .unwrap();

    let json = document.to_canonical_json().unwrap();
    let restored = SketchDocument::from_json(&json).unwrap();
    assert_eq!(restored.to_canonical_json().unwrap(), json);
    assert!(matches!(
        restored.constraint(direction).unwrap().definition,
        DocumentConstraintDefinition::CurveDirection {
            relation: DocumentCurveDirectionRelation::Normal {
                side: DocumentCurveNormalSide::Left
            },
            ..
        }
    ));
    assert!(matches!(
        restored.constraint(equal_curvature).unwrap().definition,
        DocumentConstraintDefinition::EqualCurvature {
            relation: DocumentCurveCurvatureRelation::MagnitudeSameSign,
            ..
        }
    ));
    assert!(matches!(
        restored.constraint(continuity).unwrap().definition,
        DocumentConstraintDefinition::EndpointContinuity {
            first_contact,
            second_contact,
            continuity: DocumentCurveContinuity::ParametricC2 {
                first_rate: 2.0,
                second_rate: 1.0,
            },
        } if first_contact == incoming_end && second_contact == outgoing_start
    ));

    let session = SketchDocumentSession::new(
        restored,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let direction_runtime = runtime_constraint(&session, direction);
    assert!(matches!(
        direction_runtime,
        SketchConstraintKind::CurveDirection {
            relation: geosolve_sketch::CurveDirectionRelation::Normal(
                geosolve_sketch::CurveNormalSide::Left
            ),
            ..
        }
    ));
    let curvature_runtime = runtime_constraint(&session, equal_curvature);
    assert!(matches!(
        curvature_runtime,
        SketchConstraintKind::EqualCurvature {
            relation: geosolve_sketch::CurveCurvatureRelation::MagnitudeSameSign,
            ..
        }
    ));
    let continuity_runtime = runtime_constraint(&session, continuity);
    let RuntimeCurve::QuadraticBezier(incoming_runtime) =
        session.mappings().runtime_curve(incoming).unwrap()
    else {
        panic!("expected incoming runtime Bezier")
    };
    let RuntimeCurve::QuadraticBezier(outgoing_runtime) =
        session.mappings().runtime_curve(outgoing).unwrap()
    else {
        panic!("expected outgoing runtime Bezier")
    };
    assert!(matches!(
        continuity_runtime,
        SketchConstraintKind::EndpointContinuity {
            first,
            second,
            kind: geosolve_sketch::CurveContinuity::ParametricC2 {
                first_rate: 2.0,
                second_rate: 1.0,
            },
        } if first.curve == SketchCurve::Bezier(*incoming_runtime)
            && second.curve == SketchCurve::Bezier(*outgoing_runtime)
    ));
    assert_eq!(session.mappings().contact_mappings().len(), 5);

    let mut suppressed = session.document().clone();
    let curvature_source = suppressed.constraint(equal_curvature).unwrap().source_id;
    suppressed
        .set_source_suppressed(curvature_source, true)
        .unwrap();
    let lowered = suppressed.lower().unwrap();
    assert_eq!(lowered.mappings().runtime_source(curvature_source), None);
}

#[test]
fn document_curvature_measurements_follow_similarity_and_reflection_rules() {
    let (base, base_contact) = measurement_bezier(1.0, false, [0.0, 0.0]);
    let signed = base
        .measure_curve_contact(base_contact, DocumentCurveMeasurementKind::SignedCurvature)
        .unwrap();
    let unsigned = base
        .measure_curve_contact(
            base_contact,
            DocumentCurveMeasurementKind::UnsignedCurvature,
        )
        .unwrap();
    let radius = base
        .measure_curve_contact(base_contact, DocumentCurveMeasurementKind::OsculatingRadius)
        .unwrap();
    assert!(signed > 0.0);
    assert_relative(unsigned, signed.abs());

    let scale = 7.5;
    let (transformed, transformed_contact) = measurement_bezier(scale, true, [11.0, -3.0]);
    assert_relative(
        transformed
            .measure_curve_contact(
                transformed_contact,
                DocumentCurveMeasurementKind::SignedCurvature,
            )
            .unwrap(),
        -signed / scale,
    );
    assert_relative(
        transformed
            .measure_curve_contact(
                transformed_contact,
                DocumentCurveMeasurementKind::UnsignedCurvature,
            )
            .unwrap(),
        unsigned / scale,
    );
    assert_relative(
        transformed
            .measure_curve_contact(
                transformed_contact,
                DocumentCurveMeasurementKind::OsculatingRadius,
            )
            .unwrap(),
        radius * scale,
    );

    let mut line_document = SketchDocument::new(1.0).unwrap();
    let start = line_document.add_point("line start", [0.0, 0.0]).unwrap();
    let end = line_document.add_point("line end", [2.0, 0.0]).unwrap();
    let line = line_document
        .add_curve(
            "straight line",
            CurveDefinition::Line {
                start,
                end,
                branch_direction: [1.0, 0.0],
            },
        )
        .unwrap();
    let contact = line_document
        .add_curve_contact(
            "line interior",
            CurveSpan::line(line),
            0.4,
            0,
            ContactNeighborhood::Interior,
            None,
        )
        .unwrap();
    assert!(matches!(
        line_document
            .measure_curve_contact(contact, DocumentCurveMeasurementKind::OsculatingRadius),
        Err(DocumentCurveMeasurementError::Differential(
            CurveDifferentialError::UndefinedOsculatingRadius
        ))
    ));
}

#[test]
fn c2_consumers_block_c1_only_bspline_and_nurbs_transitions_transactionally() {
    let (bspline_document, bspline_contact) = c1_only_spline_document(false, C2Consumer::G2);
    let mut bspline_session = SketchDocumentSession::new(
        bspline_document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let retained = bspline_session.export_json().unwrap();
    assert!(matches!(
        bspline_session.apply(DocumentCommand::new(
            bspline_session.revision(),
            DocumentEdit::TransitionBSplineContact {
                contact: bspline_contact,
                direction: DocumentBSplineSpanDirection::Next,
            },
        )),
        Err(DocumentSessionError::Document(
            DocumentError::BSplineEvaluation { .. }
        ))
    ));
    assert_eq!(bspline_session.export_json().unwrap(), retained);
    assert_eq!(bspline_session.history_len(), 0);

    let (c2_document, c2_contact) = c1_only_spline_document(false, C2Consumer::ParametricC2);
    let mut c2_session = SketchDocumentSession::new(
        c2_document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let retained = c2_session.export_json().unwrap();
    assert!(matches!(
        c2_session.apply(DocumentCommand::new(
            c2_session.revision(),
            DocumentEdit::TransitionBSplineContact {
                contact: c2_contact,
                direction: DocumentBSplineSpanDirection::Next,
            },
        )),
        Err(DocumentSessionError::Document(
            DocumentError::BSplineEvaluation { .. }
        ))
    ));
    assert_eq!(c2_session.export_json().unwrap(), retained);
    assert_eq!(c2_session.history_len(), 0);

    let (nurbs_document, nurbs_contact) = c1_only_spline_document(true, C2Consumer::EqualCurvature);
    let mut nurbs_session = SketchDocumentSession::new(
        nurbs_document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let retained = nurbs_session.export_json().unwrap();
    assert!(matches!(
        nurbs_session.apply(DocumentCommand::new(
            nurbs_session.revision(),
            DocumentEdit::TransitionNurbsContact {
                contact: nurbs_contact,
                direction: DocumentBSplineSpanDirection::Next,
            },
        )),
        Err(DocumentSessionError::Document(
            DocumentError::NurbsEvaluation { .. }
        ))
    ));
    assert_eq!(nurbs_session.export_json().unwrap(), retained);
    assert_eq!(nurbs_session.history_len(), 0);
}

#[test]
fn malformed_advanced_sources_and_dangling_dependencies_reject_atomically() {
    let mut document = SketchDocument::new(1.0).unwrap();
    let controls = [[0.0, 0.0], [1.0, 1.0], [2.0, 0.0]]
        .map(|position| document.add_point("Bezier control", position).unwrap());
    let curve = document
        .add_curve("test Bezier", CurveDefinition::QuadraticBezier { controls })
        .unwrap();
    let start = document
        .add_curve_contact(
            "start",
            CurveSpan::line(curve),
            0.0,
            0,
            ContactNeighborhood::Start,
            None,
        )
        .unwrap();
    let end = document
        .add_curve_contact(
            "end",
            CurveSpan::line(curve),
            1.0,
            0,
            ContactNeighborhood::End,
            None,
        )
        .unwrap();
    let interior = document
        .add_curve_contact(
            "interior",
            CurveSpan::line(curve),
            0.5,
            0,
            ContactNeighborhood::Interior,
            None,
        )
        .unwrap();
    let mut session = SketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let retained = session.export_json().unwrap();

    for definition in [
        DocumentConstraintDefinition::EndpointContinuity {
            first_contact: start,
            second_contact: end,
            continuity: DocumentCurveContinuity::ParametricC2 {
                first_rate: 0.0,
                second_rate: 1.0,
            },
        },
        DocumentConstraintDefinition::EndpointContinuity {
            first_contact: start,
            second_contact: end,
            continuity: DocumentCurveContinuity::ParametricC2 {
                first_rate: f64::NAN,
                second_rate: 1.0,
            },
        },
        DocumentConstraintDefinition::EndpointContinuity {
            first_contact: interior,
            second_contact: end,
            continuity: DocumentCurveContinuity::G0,
        },
        DocumentConstraintDefinition::EndpointContinuity {
            first_contact: start,
            second_contact: ContactId(PersistentId::from_u128(u128::MAX - 1)),
            continuity: DocumentCurveContinuity::G1,
        },
    ] {
        assert!(
            session
                .apply(DocumentCommand::new(
                    session.revision(),
                    DocumentEdit::CreateConstraint {
                        label: "malformed advanced source".into(),
                        definition,
                    },
                ))
                .is_err()
        );
        assert_eq!(session.export_json().unwrap(), retained);
        assert_eq!(session.history_len(), 0);
    }

    let source = session
        .transact(session.revision(), "valid G0 source", |candidate| {
            candidate.add_constraint(
                "valid G0 source",
                DocumentConstraintDefinition::EndpointContinuity {
                    first_contact: start,
                    second_contact: end,
                    continuity: DocumentCurveContinuity::G0,
                },
            )
        })
        .unwrap();
    assert!(source.accepted());
    let retained = session.export_json().unwrap();
    assert!(matches!(
        session
            .document()
            .clone()
            .remove(DocumentObjectId::Contact(start)),
        Err(DocumentError::ObjectInUse(_))
    ));
    assert_eq!(session.export_json().unwrap(), retained);
    session
        .transact(session.revision(), "cascade advanced source", |candidate| {
            candidate.remove_many_with_dependents(&[DocumentObjectId::Contact(start)])
        })
        .unwrap();
    assert!(session.document().contact(start).is_none());
    assert!(
        session
            .document()
            .constraint(source.value.unwrap())
            .is_none()
    );
}

fn add_circle(
    document: &mut SketchDocument,
    label: &str,
    center: [f64; 2],
    radius: f64,
) -> geosolve_sketch::CurveId {
    let center = document
        .add_point(format!("{label} center"), center)
        .unwrap();
    let radius = document
        .add_scalar(
            format!("{label} radius"),
            radius,
            ScalarUnit::Length,
            ScalarDomain::Positive,
        )
        .unwrap();
    document
        .add_curve(label, CurveDefinition::Circle { center, radius })
        .unwrap()
}

fn runtime_constraint(
    session: &SketchDocumentSession,
    persistent: geosolve_sketch::DocumentConstraintId,
) -> SketchConstraintKind {
    let source = session.document().constraint(persistent).unwrap().source_id;
    let RuntimeSource::Constraint(runtime) = session.mappings().runtime_source(source).unwrap()
    else {
        panic!("expected runtime constraint")
    };
    session
        .runtime()
        .sketch()
        .constraint(runtime)
        .unwrap()
        .kind()
}

fn measurement_bezier(
    scale: f64,
    reflect: bool,
    translation: [f64; 2],
) -> (SketchDocument, ContactId) {
    let mut document = SketchDocument::new(scale).unwrap();
    let transform = |point: [f64; 2]| {
        [
            translation[0] + scale * point[0],
            translation[1] + scale * if reflect { -point[1] } else { point[1] },
        ]
    };
    let controls = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0]]
        .map(transform)
        .map(|position| document.add_point("measurement control", position).unwrap());
    let curve = document
        .add_curve(
            "measurement Bezier",
            CurveDefinition::QuadraticBezier { controls },
        )
        .unwrap();
    let contact = document
        .add_curve_contact(
            "measurement contact",
            CurveSpan::line(curve),
            0.37,
            0,
            ContactNeighborhood::Interior,
            None,
        )
        .unwrap();
    (document, contact)
}

#[derive(Clone, Copy)]
enum C2Consumer {
    G2,
    ParametricC2,
    EqualCurvature,
}

fn c1_only_spline_document(rational: bool, consumer: C2Consumer) -> (SketchDocument, ContactId) {
    let mut document = SketchDocument::new(1.0).unwrap();
    let controls = [[0.0, 0.0], [0.25, 0.0], [0.75, 0.0], [1.0, 0.0]]
        .map(|position| document.add_point("spline control", position).unwrap())
        .to_vec();
    let definition = if rational {
        let weights = [1.0, 0.8, 1.2, 0.9]
            .map(|value| {
                document
                    .add_scalar(
                        "NURBS weight",
                        value,
                        ScalarUnit::Parameter,
                        ScalarDomain::Positive,
                    )
                    .unwrap()
            })
            .to_vec();
        CurveDefinition::Nurbs {
            form: DocumentBSplineForm::Clamped,
            degree: 2,
            controls,
            weights: weights.clone(),
            gauge_weight: weights[0],
            knots: vec![0.0, 0.0, 0.0, 0.5, 1.0, 1.0, 1.0],
            span_ids: vec![1, 2],
            next_span_id: 3,
        }
    } else {
        CurveDefinition::BSpline {
            form: DocumentBSplineForm::Clamped,
            degree: 2,
            controls,
            knots: vec![0.0, 0.0, 0.0, 0.5, 1.0, 1.0, 1.0],
            span_ids: vec![1, 2],
            next_span_id: 3,
        }
    };
    let curve = document.add_curve("C1-only spline", definition).unwrap();
    let first = document
        .add_curve_contact(
            "left knot side",
            CurveSpan { curve, segment: 1 },
            1.0,
            0,
            ContactNeighborhood::End,
            None,
        )
        .unwrap();
    let (second_segment, second_parameter, second_neighborhood) = match consumer {
        C2Consumer::EqualCurvature => (1, 1.0, ContactNeighborhood::End),
        C2Consumer::G2 | C2Consumer::ParametricC2 => (2, 0.0, ContactNeighborhood::Start),
    };
    let second = document
        .add_curve_contact(
            "second C2 contact",
            CurveSpan {
                curve,
                segment: second_segment,
            },
            second_parameter,
            0,
            second_neighborhood,
            None,
        )
        .unwrap();
    let definition = match consumer {
        C2Consumer::G2 => DocumentConstraintDefinition::EndpointContinuity {
            first_contact: first,
            second_contact: second,
            continuity: DocumentCurveContinuity::G2,
        },
        C2Consumer::ParametricC2 => DocumentConstraintDefinition::EndpointContinuity {
            first_contact: first,
            second_contact: second,
            continuity: DocumentCurveContinuity::ParametricC2 {
                first_rate: 1.0,
                second_rate: 1.0,
            },
        },
        C2Consumer::EqualCurvature => DocumentConstraintDefinition::EqualCurvature {
            first_contact: first,
            second_contact: second,
            relation: DocumentCurveCurvatureRelation::Signed,
        },
    };
    document.add_constraint("C2 consumer", definition).unwrap();
    (document, first)
}

fn assert_relative(actual: f64, expected: f64) {
    let error = (actual - expected).abs();
    let scale = actual.abs().max(expected.abs()).max(1.0e-12);
    assert!(
        error / scale <= 1.0e-10,
        "actual={actual}, expected={expected}, relative={}",
        error / scale
    );
}
