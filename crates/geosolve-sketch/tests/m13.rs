use geosolve_core::SolverConfig;
use geosolve_sketch::{
    ContactDefinition, ContactDomain, ContactNeighborhood, CurveDefinition, CurveSpan,
    DocumentConstraintDefinition, DocumentDimensionDefinition, DocumentDimensionMode,
    DocumentObjectId, DocumentSolveRequest, ScalarDomain, ScalarUnit, SketchDocument,
    SketchDocumentSession, TangentOrientation,
};

#[test]
fn compound_geometry_transaction_solves_commits_and_undoes_once() {
    let mut session = SketchDocumentSession::new(
        SketchDocument::new(4.0).unwrap(),
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let transaction = session
        .transact(session.revision(), "create circle", |document| {
            let center = document.add_point("center", [1.0, 2.0])?;
            let radius =
                document.add_scalar("radius", 3.0, ScalarUnit::Length, ScalarDomain::Positive)?;
            let circle =
                document.add_curve("circle", CurveDefinition::Circle { center, radius })?;
            Ok((center, radius, circle))
        })
        .unwrap();
    assert!(transaction.accepted());
    let (center, radius, circle) = transaction.value.unwrap();
    assert!(session.document().point(center).is_some());
    assert!(session.document().scalar(radius).is_some());
    assert!(session.document().curve(circle).is_some());
    assert_eq!(session.history_len(), 1);

    session.undo(session.revision()).unwrap();
    assert!(session.document().point(center).is_none());
    assert!(session.document().scalar(radius).is_none());
    assert!(session.document().curve(circle).is_none());
    session.redo(session.revision()).unwrap();
    assert!(session.document().point(center).is_some());
    assert!(session.document().scalar(radius).is_some());
    assert!(session.document().curve(circle).is_some());
}

#[test]
fn rejected_compound_transaction_publishes_no_ids_or_history() {
    let mut session = SketchDocumentSession::new(
        SketchDocument::new(1.0).unwrap(),
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let transaction = session
        .transact(session.revision(), "impossible distances", |document| {
            let first = document.add_point("first", [0.0, 0.0])?;
            let second = document.add_point("second", [1.0, 0.0])?;
            for (index, value) in [1.0, 2.0].into_iter().enumerate() {
                let target = document.add_scalar(
                    format!("distance {index}"),
                    value,
                    ScalarUnit::Length,
                    ScalarDomain::Positive,
                )?;
                document.add_dimension(
                    format!("distance dimension {index}"),
                    DocumentDimensionDefinition::PointDistance {
                        first,
                        second,
                        target,
                    },
                    DocumentDimensionMode::Driving,
                )?;
            }
            Ok((first, second))
        })
        .unwrap();
    assert!(!transaction.accepted());
    assert!(transaction.value.is_none());
    assert!(session.document().points().is_empty());
    assert_eq!(session.history_len(), 0);
    assert_eq!(session.revision(), 0);
}

#[test]
fn document_preview_drag_projects_a_point_along_a_generic_curve_contact() {
    let mut document = SketchDocument::new(4.0).unwrap();
    let start = document.add_point("start", [-2.0, 0.0]).unwrap();
    let end = document.add_point("end", [2.0, 0.0]).unwrap();
    let point = document.add_point("point", [0.0, 0.0]).unwrap();
    let line = document
        .add_curve(
            "line",
            CurveDefinition::Line {
                start,
                end,
                branch_direction: [1.0, 0.0],
            },
        )
        .unwrap();
    for (label, fixed) in [("fixed start", start), ("fixed end", end)] {
        let target = document.point(fixed).unwrap().position;
        document
            .add_constraint(
                label,
                DocumentConstraintDefinition::FixedPoint {
                    point: fixed,
                    target,
                },
            )
            .unwrap();
    }
    let parameter = document
        .add_scalar(
            "parameter",
            0.5,
            ScalarUnit::Parameter,
            ScalarDomain::Bounded {
                lower: 0.0,
                upper: 1.0,
            },
        )
        .unwrap();
    let contact = document
        .add_contact(
            "contact",
            ContactDefinition {
                curve: CurveSpan::line(line),
                parameter,
                domain: ContactDomain::Bounded {
                    lower: 0.0,
                    upper: 1.0,
                },
                winding: 0,
                neighborhood: ContactNeighborhood::Local {
                    lower: 0.25,
                    upper: 0.75,
                },
                tangent_orientation: None,
            },
        )
        .unwrap();
    document
        .add_constraint(
            "point on line",
            DocumentConstraintDefinition::PointOnCurve { point, contact },
        )
        .unwrap();
    let mut session = SketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let preview = session
        .rebuild_request(
            session.revision(),
            DocumentSolveRequest::default().with_drag(point, [0.5, 0.0]),
        )
        .unwrap();
    assert!(preview.accepted(), "{:#?}", preview.solve());
    let projected = session.document().point(point).unwrap().position;
    assert!((projected[0] - 0.5).abs() <= 1.0e-8, "{projected:?}");
    assert!(projected[1].abs() <= 1.0e-9, "{projected:?}");
}

#[test]
fn public_contact_creation_and_owned_state_deletion_keep_browser_logic_domain_free() {
    let mut document = SketchDocument::new(4.0).unwrap();
    let start = document.add_point("start", [0.0, 0.0]).unwrap();
    let end = document.add_point("end", [2.0, 0.0]).unwrap();
    let line = document
        .add_curve(
            "line",
            CurveDefinition::Line {
                start,
                end,
                branch_direction: [1.0, 0.0],
            },
        )
        .unwrap();
    let span = CurveSpan::line(line);
    assert_eq!(
        document.picked_contact_neighborhood(span, 0.0).unwrap(),
        ContactNeighborhood::Start
    );
    assert!(matches!(
        document.picked_contact_neighborhood(span, 0.4).unwrap(),
        ContactNeighborhood::Local { .. }
    ));
    let contact = document
        .add_curve_contact(
            "line start",
            span,
            0.0,
            0,
            ContactNeighborhood::Start,
            Some(TangentOrientation::Aligned),
        )
        .unwrap();
    assert_eq!(document.contacts().len(), 1);
    assert_eq!(document.scalars().len(), 1);
    document
        .remove_with_owned_state(DocumentObjectId::Contact(contact))
        .unwrap();
    assert!(document.contacts().is_empty());
    assert!(document.scalars().is_empty());

    let center = document.add_point("center", [4.0, 0.0]).unwrap();
    let radius = document
        .add_scalar("radius", 2.0, ScalarUnit::Length, ScalarDomain::Positive)
        .unwrap();
    let circle = document
        .add_curve("circle", CurveDefinition::Circle { center, radius })
        .unwrap();
    document
        .remove_with_owned_state(DocumentObjectId::Curve(circle))
        .unwrap();
    assert!(document.curve(circle).is_none());
    assert!(document.scalar(radius).is_none());
}
