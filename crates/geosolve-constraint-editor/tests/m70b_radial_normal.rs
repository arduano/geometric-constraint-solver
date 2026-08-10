// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_constraint_editor::{
    ActionChoice, AuthoringMutation, AuthoringOperand, AuthoringOutcome, AuthoringState,
    AuthoringTool, ConstraintActionRequest, ConstraintIntent, ContactActionChoice,
    CoordinatorActionKind, ResolvedConstraintKind, RetainedEditorCoordinator, SelectionItem,
};
use geosolve_sketch::{
    ContactDomain, ContactNeighborhood, CurveDefinition, CurveSpan, DocumentArcSweep,
    DocumentConstraintDefinition, DocumentCurveSpanRef, DocumentEdit, DocumentSolveRequest,
    RetainedSketchDocumentSession, ScalarDomain, ScalarUnit, SketchDocument, SketchHardValidity,
    SolverConfig,
};

fn reproduction() -> (RetainedEditorCoordinator, CurveSpan, CurveSpan) {
    let mut document = SketchDocument::new(10.0).expect("document");
    let center = document
        .add_point(
            "circle center",
            [0.983_007_603_204_571_3, 2.569_500_433_739_858],
        )
        .expect("circle center");
    let radius = document
        .add_scalar(
            "radius",
            1.764_309_937_774_669_6,
            ScalarUnit::Length,
            ScalarDomain::Positive,
        )
        .expect("radius");
    let circle = CurveSpan::line(
        document
            .add_curve("circle", CurveDefinition::Circle { center, radius })
            .expect("circle"),
    );
    let line_start = document
        .add_point(
            "line start",
            [-2.297_494_514_466_500_4, -0.322_370_771_036_382_84],
        )
        .expect("line start");
    let line_end = document
        .add_point(
            "line end",
            [-0.449_639_186_081_166_5, 1.539_785_553_940_733_2],
        )
        .expect("line end");
    let line = CurveSpan::line(
        document
            .add_curve(
                "line",
                CurveDefinition::Line {
                    start: line_start,
                    end: line_end,
                    branch_direction: [0.704_375_860_830_434_2, 0.709_827_194_942_110_4],
                },
            )
            .expect("line"),
    );
    let circle_contact = document
        .add_curve_contact_with_domain(
            "circle contact",
            circle,
            ContactDomain::Periodic {
                period: std::f64::consts::TAU,
            },
            3.764_791_983_523_859_5,
            0,
            ContactNeighborhood::Interior,
            None,
        )
        .expect("circle contact");
    document
        .add_constraint(
            "line endpoint on circle",
            DocumentConstraintDefinition::PointOnCurve {
                point: line_end,
                contact: circle_contact,
            },
        )
        .expect("point on circle");
    let session = RetainedSketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .expect("accepted reproduction parent");
    (
        RetainedEditorCoordinator::new(session).expect("coordinator"),
        line,
        circle,
    )
}

fn external_segment_reproduction() -> (RetainedEditorCoordinator, CurveSpan, CurveSpan) {
    external_segment_reproduction_with_radial_curve(false)
}

fn external_segment_reproduction_with_radial_curve(
    circular_arc: bool,
) -> (RetainedEditorCoordinator, CurveSpan, CurveSpan) {
    let mut document = SketchDocument::new(1.0).expect("document");
    let center = document
        .add_point("fixed circle center", [0.0, 0.0])
        .expect("center");
    let radius = document
        .add_scalar("radius", 1.0, ScalarUnit::Length, ScalarDomain::Positive)
        .expect("radius");
    let radial_curve = if circular_arc {
        let start_angle = document
            .add_scalar("arc start", 0.0, ScalarUnit::Angle, ScalarDomain::Finite)
            .expect("arc start");
        let end_angle = document
            .add_scalar(
                "arc end",
                std::f64::consts::FRAC_PI_2,
                ScalarUnit::Angle,
                ScalarDomain::Finite,
            )
            .expect("arc end");
        CurveSpan::line(
            document
                .add_curve(
                    "circular arc",
                    CurveDefinition::CircularArc {
                        center,
                        radius,
                        start_angle,
                        end_angle,
                        sweep: DocumentArcSweep::CounterClockwise,
                    },
                )
                .expect("circular arc"),
        )
    } else {
        CurveSpan::line(
            document
                .add_curve("circle", CurveDefinition::Circle { center, radius })
                .expect("circle"),
        )
    };
    let start = document
        .add_point("fixed line start", [2.0, 0.0])
        .expect("line start");
    let end = document
        .add_point("fixed line end", [3.0, 0.0])
        .expect("line end");
    let line = CurveSpan::line(
        document
            .add_curve(
                "external radial segment",
                CurveDefinition::Line {
                    start,
                    end,
                    branch_direction: [1.0, 0.0],
                },
            )
            .expect("line"),
    );
    for point in [center, start, end] {
        let target = document.point(point).expect("fixed point").position;
        document
            .add_constraint(
                format!("fix {point}"),
                DocumentConstraintDefinition::FixedPoint { point, target },
            )
            .expect("fixed point constraint");
    }
    let session = RetainedSketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .expect("accepted external-segment parent");
    (
        RetainedEditorCoordinator::new(session).expect("coordinator"),
        line,
        radial_curve,
    )
}

#[test]
fn radial_normal_from_payload_remains_an_accepted_finite_scene() {
    let (mut coordinator, line, circle) = reproduction();
    let application = AuthoringState::default().activate(
        coordinator.session().design_document(),
        AuthoringTool::Constraint(ConstraintIntent::Perpendicular),
        &[
            AuthoringOperand::picked(SelectionItem::Curve(line), Some(0.523_728_158_808_117_7)),
            AuthoringOperand::picked(SelectionItem::Curve(circle), Some(3.764_791_983_523_859_5)),
        ],
    );
    let AuthoringOutcome::Apply(application) = application else {
        panic!("radial normal authoring application");
    };
    let AuthoringMutation::Constraint(outcome) = coordinator
        .apply_authoring(coordinator.session().design_identity(), &application)
        .expect("valid radial-normal transaction")
    else {
        panic!("radial normal constraint mutation");
    };

    assert!(outcome.published_accepted.is_some());
    let diagnostics = coordinator.session().latest_attempt_diagnostics();
    let solve = diagnostics.solve.expect("solve diagnostics");
    assert!(solve.accepted);
    assert_eq!(solve.hard_validity, SketchHardValidity::Valid);
    assert!(solve.hard_residuals_validated);
    assert!(
        solve
            .maximum_normalized_hard_residual
            .is_some_and(|residual| residual <= 1.0e-9)
    );

    let design = coordinator.session().design_document();
    let DocumentConstraintDefinition::PointOnCurve { point, contact } = design
        .constraint(outcome.value)
        .expect("radial-normal constraint")
        .definition
    else {
        panic!("radial normal must lower to center-on-line incidence");
    };
    let contact = design.contact(contact).expect("radial-normal contact");
    assert_eq!(contact.domain, ContactDomain::SupportingLine);
    assert_eq!(contact.neighborhood, ContactNeighborhood::Interior);
    let seed_parameter = design
        .scalar(contact.parameter)
        .expect("contact parameter")
        .value;
    assert!((seed_parameter - 1.663_278_758_074_294_5).abs() <= 1.0e-15);

    let accepted = coordinator
        .session()
        .accepted_state_for_current_input()
        .expect("current accepted radial normal")
        .document();
    let CurveDefinition::Circle { radius, .. } = &accepted
        .curve(circle.curve)
        .expect("accepted circle")
        .definition
    else {
        panic!("payload circle definition");
    };
    let accepted_radius = accepted
        .scalar(*radius)
        .expect("accepted circle radius")
        .value;
    assert!(
        accepted_radius.is_finite() && accepted_radius > 0.5,
        "radial Normal must not collapse the visible circle: {accepted_radius}"
    );
    let contact = accepted.contact(contact.id).expect("accepted contact");
    let line_point = accepted
        .evaluate_contact_jet(contact.id)
        .expect("accepted line point")
        .position;
    let center = accepted
        .point(point)
        .expect("accepted circle center")
        .position;
    assert!((line_point[0] - center[0]).hypot(line_point[1] - center[1]) <= 1.0e-9);
}

#[test]
fn radial_normal_rejects_bounded_or_local_metadata_before_retaining_a_bad_design() {
    for (domain, parameter, neighborhood) in [
        (
            ContactDomain::Bounded {
                lower: 0.0,
                upper: 1.0,
            },
            0.523_728_158_808_117_7,
            ContactNeighborhood::Interior,
        ),
        (
            ContactDomain::SupportingLine,
            1.663_278_758_074_294_5,
            ContactNeighborhood::Local {
                lower: 1.0,
                upper: 2.0,
            },
        ),
    ] {
        let (mut coordinator, line, circle) = reproduction();
        coordinator
            .editor_mut()
            .set_selection([SelectionItem::Curve(line), SelectionItem::Curve(circle)]);
        let design_before = coordinator.session().design_identity();
        let accepted_before = coordinator
            .session()
            .accepted_state()
            .expect("accepted parent")
            .identity();
        let attempt_before = coordinator.session().last_attempt().identity();
        let coordinator_history_before = (coordinator.history_len(), coordinator.history_cursor());
        let transcript_len_before = coordinator.transcript().len();
        let error = coordinator
            .apply_constraint_action(
                design_before,
                ConstraintActionRequest {
                    intent: ConstraintIntent::Perpendicular,
                    label: "invalid restricted normal".into(),
                    contacts: vec![ContactActionChoice {
                        support: DocumentCurveSpanRef {
                            span: line,
                            winding: 0,
                        },
                        domain,
                        parameter,
                        neighborhood,
                        tangent_orientation: None,
                    }],
                    relation: None,
                },
            )
            .expect_err("bounded or local metadata is not radial-normal semantics");
        assert!(error.to_string().contains("complete supporting line"));
        assert_eq!(coordinator.session().design_identity(), design_before);
        assert_eq!(
            coordinator.session().last_attempt().identity(),
            attempt_before
        );
        assert_eq!(
            (coordinator.history_len(), coordinator.history_cursor()),
            coordinator_history_before
        );
        assert_eq!(coordinator.transcript().len(), transcript_len_before);
        assert_eq!(
            coordinator
                .session()
                .accepted_state()
                .expect("accepted parent retained")
                .identity(),
            accepted_before
        );
    }
}

#[test]
fn radial_normal_is_commutative_and_uses_an_external_line_projection() {
    for circular_arc in [false, true] {
        for line_first in [true, false] {
            let (mut coordinator, line, radial_curve) =
                external_segment_reproduction_with_radial_curve(circular_arc);
            let selection = if line_first {
                [
                    SelectionItem::Curve(line),
                    SelectionItem::Curve(radial_curve),
                ]
            } else {
                [
                    SelectionItem::Curve(radial_curve),
                    SelectionItem::Curve(line),
                ]
            };
            coordinator.editor_mut().set_selection(selection);
            assert_eq!(
                coordinator.resolved_constraint(ConstraintIntent::Perpendicular),
                Some(ResolvedConstraintKind::RadialLine)
            );
            let choices = coordinator.action_choices(CoordinatorActionKind::Constraint(
                ConstraintIntent::Perpendicular,
            ));
            let [
                ActionChoice::Contact {
                    operand,
                    span,
                    domains,
                    default_parameter,
                    neighborhoods,
                    tangent_orientations,
                    default_winding,
                },
            ] = choices.as_slice()
            else {
                panic!("one radial supporting-line choice");
            };
            assert_eq!(*operand, u8::from(!line_first));
            assert_eq!(*span, line);
            assert_eq!(domains, &[ContactDomain::SupportingLine]);
            assert_eq!(default_parameter.to_bits(), (-2.0_f64).to_bits());
            assert_eq!(neighborhoods, &[ContactNeighborhood::Interior]);
            assert!(tangent_orientations.is_empty());
            assert_eq!(*default_winding, 0);

            let operands = selection.map(|item| {
                AuthoringOperand::picked(
                    item,
                    Some(if item == SelectionItem::Curve(line) {
                        0.5
                    } else {
                        0.0
                    }),
                )
            });
            let AuthoringOutcome::Apply(application) = AuthoringState::default().activate(
                coordinator.session().design_document(),
                AuthoringTool::Constraint(ConstraintIntent::Perpendicular),
                &operands,
            ) else {
                panic!("radial-normal application");
            };
            let AuthoringMutation::Constraint(outcome) = coordinator
                .apply_authoring(coordinator.session().design_identity(), &application)
                .expect("external supporting-line normal")
            else {
                panic!("radial-normal mutation");
            };
            assert!(outcome.published_accepted.is_some());
            let design = coordinator.session().design_document();
            let DocumentConstraintDefinition::PointOnCurve { contact, .. } = design
                .constraint(outcome.value)
                .expect("radial-normal constraint")
                .definition
            else {
                panic!("center-on-line incidence");
            };
            let contact = design.contact(contact).expect("radial-normal contact");
            assert_eq!(contact.domain, ContactDomain::SupportingLine);
            assert_eq!(contact.neighborhood, ContactNeighborhood::Interior);
            assert_eq!(
                design
                    .scalar(contact.parameter)
                    .expect("projection seed")
                    .value
                    .to_bits(),
                (-2.0_f64).to_bits()
            );
        }
    }
}

#[test]
fn radial_normal_uses_historical_accepted_geometry_beneath_a_rejected_design() {
    let (mut coordinator, line, circle) = external_segment_reproduction();
    let center = match &coordinator
        .session()
        .design_document()
        .curve(circle.curve)
        .expect("circle")
        .definition
    {
        CurveDefinition::Circle { center, .. } => *center,
        _ => panic!("circle definition"),
    };
    let rejected = coordinator
        .apply_edit(
            coordinator.session().design_identity(),
            DocumentEdit::SetPointPosition {
                point: center,
                position: [100.0, 0.0],
            },
        )
        .expect("retained rejected fixed-point move");
    assert!(rejected.published_accepted.is_none());
    assert!(
        coordinator
            .session()
            .accepted_state_for_current_input()
            .is_none()
    );
    assert_eq!(
        coordinator
            .session()
            .design_document()
            .point(center)
            .expect("attempted center")
            .position
            .map(f64::to_bits),
        [100.0, 0.0].map(f64::to_bits)
    );
    assert_eq!(
        coordinator
            .session()
            .accepted_state()
            .expect("historical accepted state")
            .document()
            .point(center)
            .expect("accepted center")
            .position
            .map(f64::to_bits),
        [0.0, 0.0].map(f64::to_bits)
    );

    let selection = [SelectionItem::Curve(line), SelectionItem::Curve(circle)];
    coordinator.editor_mut().set_selection(selection);
    let choices = coordinator.action_choices(CoordinatorActionKind::Constraint(
        ConstraintIntent::Perpendicular,
    ));
    let [
        ActionChoice::Contact {
            default_parameter, ..
        },
    ] = choices.as_slice()
    else {
        panic!("historical accepted radial choice");
    };
    assert_eq!(default_parameter.to_bits(), (-2.0_f64).to_bits());

    let operands = selection.map(|item| AuthoringOperand::picked(item, Some(0.5)));
    let AuthoringOutcome::Apply(application) = AuthoringState::default().activate(
        coordinator.session().design_document(),
        AuthoringTool::Constraint(ConstraintIntent::Perpendicular),
        &operands,
    ) else {
        panic!("radial authoring application");
    };
    let AuthoringMutation::Constraint(outcome) = coordinator
        .apply_authoring(coordinator.session().design_identity(), &application)
        .expect("retained radial authoring beneath rejection")
    else {
        panic!("radial authoring mutation");
    };
    assert!(outcome.published_accepted.is_some());
    let design = coordinator.session().design_document();
    let DocumentConstraintDefinition::PointOnCurve { contact, .. } = design
        .constraint(outcome.value)
        .expect("retained radial constraint")
        .definition
    else {
        panic!("radial center-on-support definition");
    };
    let contact = design.contact(contact).expect("retained radial contact");
    assert_eq!(contact.domain, ContactDomain::SupportingLine);
    assert_eq!(
        design
            .scalar(contact.parameter)
            .expect("accepted-geometry projection seed")
            .value
            .to_bits(),
        (-2.0_f64).to_bits()
    );
}
