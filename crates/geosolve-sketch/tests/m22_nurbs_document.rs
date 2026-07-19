#![allow(clippy::too_many_lines)]

use geosolve_core::SolverConfig;
use geosolve_geometry::Point2;
use geosolve_sketch::{
    ContactNeighborhood, CurveDefinition, CurveSpan, DocumentBSplineForm,
    DocumentBSplineSpanDirection, DocumentCommand, DocumentCommandEffect, DocumentEdit,
    DocumentError, DocumentObjectId, DocumentSolveRequest, RuntimeCurve, ScalarDomain, ScalarUnit,
    SketchDocument, SketchDocumentSession, TangentOrientation,
};

#[test]
fn canonical_lowering_projection_and_regauge_preserve_explicit_nurbs_state() {
    let (mut document, curve, _controls, weights) = periodic_nurbs_document();
    let contact = document
        .add_curve_contact(
            "periodic NURBS contact",
            CurveSpan { curve, segment: 31 },
            0.3,
            2,
            ContactNeighborhood::Local {
                lower: 0.1,
                upper: 0.9,
            },
            None,
        )
        .unwrap();
    let expected_contact = document.evaluate_contact_jet(contact).unwrap();
    let before_regauge = sample_all_spans(&document, curve);

    let json = document.to_canonical_json().unwrap();
    let restored = SketchDocument::from_json(&json).unwrap();
    assert_eq!(restored.to_canonical_json().unwrap(), json);
    assert_eq!(restored.contact(contact).unwrap().winding, 2);
    assert!(
        (restored.evaluate_contact_jet(contact).unwrap().position - expected_contact.position)
            .norm()
            <= 1.0e-12
    );

    let lowered = restored.lower().unwrap();
    let RuntimeCurve::Nurbs { nurbs, spans } = lowered
        .mappings()
        .runtime_curve(curve)
        .expect("persistent NURBS must lower")
    else {
        panic!("expected runtime NURBS")
    };
    assert_eq!(
        spans
            .iter()
            .map(|(semantic, _)| *semantic)
            .collect::<Vec<_>>(),
        vec![11, 17, 23, 29, 31]
    );
    let runtime = lowered.sketch().nurbs(*nurbs).unwrap();
    assert_eq!(runtime.gauge_index(), 1);
    assert_eq!(runtime.weights()[1].to_bits(), 1.0f64.to_bits());

    let mut session = SketchDocumentSession::new(
        restored,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let regauged = session
        .apply(DocumentCommand::new(
            session.revision(),
            DocumentEdit::SetNurbsWeightGauge {
                curve,
                gauge_weight: weights[3],
            },
        ))
        .unwrap();
    assert!(matches!(
        regauged.effect,
        Some(DocumentCommandEffect::UpdatedNurbsWeightGauge(id)) if id == curve
    ));
    let CurveDefinition::Nurbs {
        weights: accepted_weights,
        gauge_weight,
        ..
    } = &session.document().curve(curve).unwrap().definition
    else {
        panic!("expected persistent NURBS")
    };
    assert_eq!(*gauge_weight, weights[3]);
    assert_eq!(
        session
            .document()
            .scalar(*gauge_weight)
            .unwrap()
            .value
            .to_bits(),
        1.0f64.to_bits()
    );
    for (actual, expected) in sample_all_spans(session.document(), curve)
        .into_iter()
        .zip(before_regauge)
    {
        assert!((actual - expected).norm() <= 2.0e-12);
    }

    let edited_weight = accepted_weights[4];
    session
        .apply(DocumentCommand::new(
            session.revision(),
            DocumentEdit::SetScalarValue {
                scalar: edited_weight,
                value: 1.75,
            },
        ))
        .unwrap();
    assert_eq!(
        session
            .document()
            .scalar(edited_weight)
            .unwrap()
            .value
            .to_bits(),
        1.75f64.to_bits()
    );
    let runtime_nurbs = session.mappings().runtime_nurbs(curve).unwrap();
    let runtime = session.runtime().sketch().nurbs(runtime_nurbs).unwrap();
    assert_eq!(runtime.weights()[4].to_bits(), 1.75f64.to_bits());
    assert_eq!(runtime.weights()[3].to_bits(), 1.0f64.to_bits());

    let retained = session.export_json().unwrap();
    assert!(matches!(
        session.apply(DocumentCommand::new(
            session.revision(),
            DocumentEdit::SetScalarValue {
                scalar: weights[3],
                value: 2.0,
            },
        )),
        Err(geosolve_sketch::DocumentSessionError::Document(
            DocumentError::InvalidField {
                field: "scalar edit",
                ..
            }
        ))
    ));
    assert_eq!(session.export_json().unwrap(), retained);
}

#[test]
fn malformed_nurbs_weights_and_gauge_reject_without_mutating_the_session() {
    let mut document = SketchDocument::new(1.0).unwrap();
    let controls = [[0.0, 0.0], [1.0, 1.0], [2.0, 0.0]]
        .map(|position| document.add_point("NURBS control", position).unwrap());
    let weights = [1.0, 0.8, 1.2].map(|value| {
        document
            .add_scalar(
                "NURBS weight",
                value,
                ScalarUnit::Parameter,
                ScalarDomain::Positive,
            )
            .unwrap()
    });
    let foreign_gauge = document
        .add_scalar(
            "foreign gauge",
            1.0,
            ScalarUnit::Parameter,
            ScalarDomain::Positive,
        )
        .unwrap();
    let mut session = SketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let retained = session.export_json().unwrap();

    let duplicate_weight = DocumentEdit::CreateCurve {
        label: "duplicate NURBS weight".into(),
        definition: CurveDefinition::Nurbs {
            form: DocumentBSplineForm::Clamped,
            degree: 2,
            controls: controls.to_vec(),
            weights: vec![weights[0], weights[1], weights[1]],
            gauge_weight: weights[0],
            knots: vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
            span_ids: vec![7],
            next_span_id: 8,
        },
    };
    assert!(matches!(
        session.apply(DocumentCommand::new(session.revision(), duplicate_weight)),
        Err(geosolve_sketch::DocumentSessionError::Document(
            DocumentError::InvalidField {
                field: "curve.weights",
                ..
            }
        ))
    ));
    assert_eq!(session.export_json().unwrap(), retained);
    assert_eq!(session.history_len(), 0);

    let foreign = DocumentEdit::CreateCurve {
        label: "foreign NURBS gauge".into(),
        definition: CurveDefinition::Nurbs {
            form: DocumentBSplineForm::Clamped,
            degree: 2,
            controls: controls.to_vec(),
            weights: weights.to_vec(),
            gauge_weight: foreign_gauge,
            knots: vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
            span_ids: vec![7],
            next_span_id: 8,
        },
    };
    assert!(matches!(
        session.apply(DocumentCommand::new(session.revision(), foreign)),
        Err(geosolve_sketch::DocumentSessionError::Document(
            DocumentError::InvalidField {
                field: "curve.gauge_weight",
                ..
            }
        ))
    ));
    assert_eq!(session.export_json().unwrap(), retained);

    let non_unit = DocumentEdit::CreateCurve {
        label: "non-unit NURBS gauge".into(),
        definition: CurveDefinition::Nurbs {
            form: DocumentBSplineForm::Clamped,
            degree: 2,
            controls: controls.to_vec(),
            weights: weights.to_vec(),
            gauge_weight: weights[1],
            knots: vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
            span_ids: vec![7],
            next_span_id: 8,
        },
    };
    assert!(matches!(
        session.apply(DocumentCommand::new(session.revision(), non_unit)),
        Err(geosolve_sketch::DocumentSessionError::Document(
            DocumentError::InvalidField {
                field: "curve.gauge_weight",
                ..
            }
        ))
    ));
    assert_eq!(session.export_json().unwrap(), retained);
    assert_eq!(session.history_len(), 0);
}

#[test]
fn homogeneous_insertion_preserves_geometry_ids_contacts_and_allocator_high_water() {
    let (mut document, curve, original_controls, original_weights) = clamped_nurbs_document();
    let left_contact = document
        .add_curve_contact(
            "left NURBS contact",
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
            "right NURBS contact",
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
    let before_contacts = [left_contact, right_contact]
        .map(|contact| document.evaluate_contact_jet(contact).unwrap().position);
    let native_samples = [0.0, 0.07, 0.2, 0.25, 0.31, 0.49, 0.5, 0.73, 1.0];
    let before =
        native_samples.map(|parameter| document_native_position(&document, curve, parameter));
    let original_gauge = match &document.curve(curve).unwrap().definition {
        CurveDefinition::Nurbs { gauge_weight, .. } => *gauge_weight,
        _ => unreachable!(),
    };

    let mut session = SketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let inserted = session
        .apply(DocumentCommand::new(
            session.revision(),
            DocumentEdit::InsertNurbsKnot {
                curve,
                parameter: 0.25,
            },
        ))
        .unwrap();
    assert!(inserted.accepted(), "{inserted:#?}");
    let Some(DocumentCommandEffect::InsertedNurbsKnot(first)) = inserted.effect else {
        panic!("expected NURBS insertion effect")
    };
    assert_eq!(first.new_span_id, Some(100));
    assert_eq!(first.migrated_contacts, vec![left_contact, right_contact]);
    assert_eq!(session.history_len(), 1);
    let CurveDefinition::Nurbs {
        controls,
        weights,
        gauge_weight,
        span_ids,
        next_span_id,
        ..
    } = &session.document().curve(curve).unwrap().definition
    else {
        panic!("expected persistent NURBS")
    };
    assert_eq!(span_ids, &[41, 100, 73]);
    assert_eq!(*next_span_id, 101);
    assert!(
        original_controls
            .iter()
            .all(|control| controls.contains(control))
    );
    assert!(
        original_weights
            .iter()
            .all(|weight| weights.contains(weight))
    );
    assert!(controls.contains(&first.new_control));
    assert!(weights.contains(&first.new_weight));
    assert_eq!(*gauge_weight, original_gauge);
    assert_eq!(
        session
            .document()
            .scalar(*gauge_weight)
            .unwrap()
            .value
            .to_bits(),
        1.0f64.to_bits()
    );
    for (parameter, expected) in native_samples.into_iter().zip(before) {
        let actual = document_native_position(session.document(), curve, parameter);
        assert!(
            (actual - expected).norm() <= 3.0e-12,
            "parameter {parameter}"
        );
    }
    for (contact, expected) in [left_contact, right_contact]
        .into_iter()
        .zip(before_contacts)
    {
        assert!(
            (session
                .document()
                .evaluate_contact_jet(contact)
                .unwrap()
                .position
                - expected)
                .norm()
                <= 2.0e-12
        );
    }
    assert_eq!(
        session
            .document()
            .contact(left_contact)
            .unwrap()
            .curve
            .segment,
        41
    );
    assert_eq!(
        session
            .document()
            .contact(right_contact)
            .unwrap()
            .curve
            .segment,
        100
    );

    let after_first = session.export_json().unwrap();
    session.undo(session.revision()).unwrap();
    assert_eq!(session.document().points().len(), original_controls.len());
    assert_eq!(
        session.document().scalars().len(),
        original_weights.len() + 2
    );
    session.redo(session.revision()).unwrap();
    assert_eq!(session.export_json().unwrap(), after_first);
    session.undo(session.revision()).unwrap();

    let divergent = session
        .apply(DocumentCommand::new(
            session.revision(),
            DocumentEdit::InsertNurbsKnot {
                curve,
                parameter: 0.75,
            },
        ))
        .unwrap();
    let Some(DocumentCommandEffect::InsertedNurbsKnot(second)) = divergent.effect else {
        panic!("expected divergent NURBS insertion effect")
    };
    assert_ne!(second.new_control, first.new_control);
    assert_ne!(second.new_weight, first.new_weight);
    assert_eq!(second.new_span_id, Some(101));
    assert!(!session.can_redo());
}

#[test]
fn explicit_nurbs_transition_preserves_periodic_seam_and_requires_c1_for_tangency() {
    let (mut periodic, curve, _controls, _weights) = periodic_nurbs_document();
    let contact = periodic
        .add_curve_contact(
            "periodic NURBS seam",
            CurveSpan { curve, segment: 31 },
            1.0,
            2,
            ContactNeighborhood::End,
            Some(TangentOrientation::Aligned),
        )
        .unwrap();
    let seam = periodic.evaluate_contact_jet(contact).unwrap().position;
    let mut session = SketchDocumentSession::new(
        periodic,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let transitioned = session
        .apply(DocumentCommand::new(
            session.revision(),
            DocumentEdit::TransitionNurbsContact {
                contact,
                direction: DocumentBSplineSpanDirection::Next,
            },
        ))
        .unwrap();
    assert!(transitioned.accepted());
    let contact_value = session.document().contact(contact).unwrap();
    assert_eq!(contact_value.curve.segment, 11);
    assert_eq!(contact_value.winding, 3);
    assert_eq!(contact_value.neighborhood, ContactNeighborhood::Start);
    assert!(
        (session
            .document()
            .evaluate_contact_jet(contact)
            .unwrap()
            .position
            - seam)
            .norm()
            <= 2.0e-12
    );

    let mut linear = SketchDocument::new(1.0).unwrap();
    let controls = [[0.0, 0.0], [1.0, 1.0], [2.0, 0.0]]
        .map(|position| linear.add_point("linear NURBS control", position).unwrap());
    let weights = [1.0, 0.8, 1.2].map(|value| {
        linear
            .add_scalar(
                "linear NURBS weight",
                value,
                ScalarUnit::Parameter,
                ScalarDomain::Positive,
            )
            .unwrap()
    });
    let linear_curve = linear
        .add_curve(
            "C0 linear NURBS",
            CurveDefinition::Nurbs {
                form: DocumentBSplineForm::Clamped,
                degree: 1,
                controls: controls.to_vec(),
                weights: weights.to_vec(),
                gauge_weight: weights[0],
                knots: vec![0.0, 0.0, 0.5, 1.0, 1.0],
                span_ids: vec![1, 2],
                next_span_id: 3,
            },
        )
        .unwrap();
    let tangent_contact = linear
        .add_curve_contact(
            "C0 NURBS tangent transition",
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
        linear.transition_nurbs_contact(tangent_contact, DocumentBSplineSpanDirection::Next),
        Err(DocumentError::NurbsEvaluation { .. })
    ));
    assert_eq!(linear.to_canonical_json().unwrap(), retained);
}

#[test]
fn raw_nurbs_curve_deletion_removes_owned_weights_but_retains_controls() {
    let (mut document, curve, controls, weights) = clamped_nurbs_document();

    document.remove(DocumentObjectId::Curve(curve)).unwrap();

    assert!(document.curve(curve).is_none());
    assert!(
        controls
            .iter()
            .all(|control| document.point(*control).is_some())
    );
    assert!(
        weights
            .iter()
            .all(|weight| document.scalar(*weight).is_none())
    );
}

fn periodic_nurbs_document() -> (
    SketchDocument,
    geosolve_sketch::CurveId,
    Vec<geosolve_sketch::DesignPointId>,
    Vec<geosolve_sketch::DesignScalarId>,
) {
    let mut document = SketchDocument::new(1.0).unwrap();
    let controls = [[0.0, 0.0], [1.5, -0.2], [2.0, 1.4], [0.5, 2.2], [-0.8, 1.0]]
        .map(|position| {
            document
                .add_point("periodic NURBS control", position)
                .unwrap()
        })
        .to_vec();
    let weights = [0.75, 1.0, 1.4, 0.9, 1.2]
        .map(|value| {
            document
                .add_scalar(
                    "periodic NURBS weight",
                    value,
                    ScalarUnit::Parameter,
                    ScalarDomain::Positive,
                )
                .unwrap()
        })
        .to_vec();
    let curve = document
        .add_curve(
            "periodic quadratic NURBS",
            CurveDefinition::Nurbs {
                form: DocumentBSplineForm::Periodic,
                degree: 2,
                controls: controls.clone(),
                weights: weights.clone(),
                gauge_weight: weights[1],
                knots: vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0],
                span_ids: vec![11, 17, 23, 29, 31],
                next_span_id: 32,
            },
        )
        .unwrap();
    (document, curve, controls, weights)
}

fn clamped_nurbs_document() -> (
    SketchDocument,
    geosolve_sketch::CurveId,
    Vec<geosolve_sketch::DesignPointId>,
    Vec<geosolve_sketch::DesignScalarId>,
) {
    let mut document = SketchDocument::new(1.0).unwrap();
    let controls = [[0.0, 0.0], [1.0, 2.0], [2.0, -1.0], [3.0, 1.5], [4.0, 0.0]]
        .map(|position| {
            document
                .add_point("clamped NURBS control", position)
                .unwrap()
        })
        .to_vec();
    let weights = [0.8, 1.0, 1.35, 0.7, 1.1]
        .map(|value| {
            document
                .add_scalar(
                    "clamped NURBS weight",
                    value,
                    ScalarUnit::Parameter,
                    ScalarDomain::Positive,
                )
                .unwrap()
        })
        .to_vec();
    let curve = document
        .add_curve(
            "clamped cubic NURBS",
            CurveDefinition::Nurbs {
                form: DocumentBSplineForm::Clamped,
                degree: 3,
                controls: controls.clone(),
                weights: weights.clone(),
                gauge_weight: weights[1],
                knots: vec![0.0, 0.0, 0.0, 0.0, 0.5, 1.0, 1.0, 1.0, 1.0],
                span_ids: vec![41, 73],
                next_span_id: 100,
            },
        )
        .unwrap();
    (document, curve, controls, weights)
}

fn sample_all_spans(
    document: &SketchDocument,
    curve: geosolve_sketch::CurveId,
) -> Vec<Point2<f64>> {
    document
        .curve_spans(curve)
        .unwrap()
        .into_iter()
        .flat_map(|span| {
            [0.0, 0.17, 0.5, 0.83, 1.0].map(move |parameter| {
                document
                    .evaluate_curve_jet(span, parameter)
                    .unwrap()
                    .position
            })
        })
        .collect()
}

fn document_native_position(
    document: &SketchDocument,
    curve: geosolve_sketch::CurveId,
    parameter: f64,
) -> Point2<f64> {
    let CurveDefinition::Nurbs {
        form,
        degree,
        controls,
        knots,
        span_ids,
        ..
    } = &document.curve(curve).unwrap().definition
    else {
        panic!("expected persistent NURBS")
    };
    let basis = match form {
        DocumentBSplineForm::Clamped => {
            geosolve_sketch::BSplineBasis::try_clamped(*degree, controls.len(), knots.clone())
        }
        DocumentBSplineForm::Periodic => {
            geosolve_sketch::BSplineBasis::try_periodic(*degree, controls.len(), knots.clone())
        }
    }
    .unwrap();
    let ordinal = basis
        .spans()
        .iter()
        .position(|span| parameter >= span.lower() && parameter <= span.upper())
        .unwrap();
    let span = &basis.spans()[ordinal];
    let local = (parameter - span.lower()) / (span.upper() - span.lower());
    document
        .evaluate_curve_jet(
            CurveSpan {
                curve,
                segment: span_ids[ordinal],
            },
            local,
        )
        .unwrap()
        .position
}
