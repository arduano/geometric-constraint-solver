// SPDX-License-Identifier: GPL-3.0-or-later
#![allow(clippy::too_many_lines)]

use geosolve_core::{HardValidity, SolverConfig};
use geosolve_geometry::Point2;
use geosolve_sketch::{
    AlphaScenarioIds, AlphaScenarioKind, ContactNeighborhood, CurveDefinition, CurveSpan,
    DocumentAngleOrientation, DocumentBSplineSpanDirection, DocumentCommand,
    DocumentConstraintDefinition, DocumentCurveMeasurementKind, DocumentCurveNormalSide,
    DocumentDimensionDefinition, DocumentDimensionMode, DocumentEdit, DocumentFilletEndpointOrder,
    DocumentFilletTrimEndpoint, DocumentLineOffsetOrientation, DocumentLineSide,
    DocumentSolveRequest, SketchDocument, SketchDocumentSession, alpha_scenario,
};

const M30_SCENARIOS: [(AlphaScenarioKind, usize, usize); 12] = [
    (AlphaScenarioKind::SupportingOffset, 2, 2),
    (AlphaScenarioKind::ExactTranslatedOffset, 1, 1),
    (AlphaScenarioKind::EntityMirror, 1, 1),
    (AlphaScenarioKind::DirectedAngle, 1, 1),
    (AlphaScenarioKind::M27ReferenceFillet, 1, 1),
    (AlphaScenarioKind::FilletLineCircle, 1, 1),
    (AlphaScenarioKind::FilletLineBezier, 1, 1),
    (AlphaScenarioKind::FilletNurbsLine, 3, 3),
    (AlphaScenarioKind::NurbsQuarterCircle, 4, 4),
    (AlphaScenarioKind::NurbsLocalSupport, 13, 13),
    (AlphaScenarioKind::NurbsPeriodic, 13, 12),
    (AlphaScenarioKind::NurbsDifferential, 10, 10),
];

fn session(kind: AlphaScenarioKind) -> (SketchDocumentSession, geosolve_sketch::AlphaScenarioIds) {
    let fixture = alpha_scenario(kind, 1.0).unwrap();
    let ids = fixture.ids;
    let session =
        SketchDocumentSession::new(fixture.document, fixture.request, SolverConfig::default())
            .unwrap();
    (session, ids)
}

fn assert_accepted(session: &SketchDocumentSession) {
    let accepted = session.accepted_result();
    let result = accepted.accepted_view();
    assert!(result.accepted(), "{:#?}", result.rejection);
    assert_eq!(result.core_report.hard_validity, HardValidity::Valid);
    assert!(result.core_report.hard_residuals_validated);
    assert!(result.core_report.hard_residual_max <= 1.0e-9);
    assert!(result.acceptance_hard_residual_max.unwrap() <= 1.0e-9);
}

fn point(document: &SketchDocument, id: geosolve_sketch::DesignPointId) -> Point2<f64> {
    Point2::from(document.point(id).unwrap().position)
}

fn distance(first: Point2<f64>, second: Point2<f64>) -> f64 {
    (first - second).norm()
}

fn assert_round_trip(session: &SketchDocumentSession) {
    let json = session.export_json().unwrap();
    assert_eq!(
        SketchDocument::from_json(&json)
            .unwrap()
            .to_canonical_json()
            .unwrap(),
        json
    );
}

#[test]
fn m30_scenarios_start_accepted_with_exact_documented_dof() {
    for (kind, equality_dof, bounded_dof) in M30_SCENARIOS {
        let (session, _) = session(kind);
        assert_accepted(&session);
        let accepted = session.accepted_result();
        let report = &accepted.accepted_view().core_report;
        assert_eq!(report.right_nullity, equality_dof, "{}", kind.key());
        assert_eq!(
            report.bidirectional_degrees_of_freedom,
            bounded_dof,
            "{}",
            kind.key()
        );
        let uat = kind.uat().expect("M30 scenario UAT metadata");
        assert_eq!(uat.expected_equality_dof, equality_dof);
        assert_eq!(uat.expected_bounded_dof, bounded_dof);
        assert!(!uat.instructions.is_empty());
        assert!(!uat.primary_drag.is_empty());
        assert_round_trip(&session);
    }
}

#[test]
fn offsets_and_mirror_accept_projected_drags_move_associated_geometry_and_keep_history() {
    let (mut supporting, ids) = session(AlphaScenarioKind::SupportingOffset);
    let AlphaScenarioIds::SupportingOffset(ids) = ids else {
        panic!("supporting offset IDs")
    };
    let source_before = supporting
        .document()
        .evaluate_curve_jet(CurveSpan::line(ids.source), 1.0)
        .unwrap()
        .position;
    let target_start_before = point(supporting.document(), ids.target_points[0]);
    let moved = supporting
        .apply(DocumentCommand::new(
            supporting.revision(),
            DocumentEdit::SetPointPosition {
                point: ids.target_points[1],
                position: [3.5, 0.0],
            },
        ))
        .unwrap();
    assert!(moved.accepted(), "{moved:#?}");
    assert_eq!(supporting.history_len(), 1);
    assert!(
        distance(
            point(supporting.document(), ids.target_points[0]),
            target_start_before
        ) > 0.1
    );
    assert_eq!(
        supporting
            .document()
            .evaluate_curve_jet(CurveSpan::line(ids.source), 1.0)
            .unwrap()
            .position,
        source_before
    );
    let target_start = point(supporting.document(), ids.target_points[0]);
    let target_end = point(supporting.document(), ids.target_points[1]);
    assert!((target_start.y - 0.0).abs() <= 1.0e-9);
    assert!((target_end.y - 0.0).abs() <= 1.0e-9);
    let accepted = supporting.export_json().unwrap();
    supporting.undo(supporting.revision()).unwrap();
    supporting.redo(supporting.revision()).unwrap();
    assert_eq!(supporting.export_json().unwrap(), accepted);

    let (mut exact, ids) = session(AlphaScenarioKind::ExactTranslatedOffset);
    let AlphaScenarioIds::ExactTranslatedOffset(ids) = ids else {
        panic!("exact offset IDs")
    };
    let target_before = point(exact.document(), ids.target_points[1]);
    let rotated = exact
        .apply(DocumentCommand::new(
            exact.revision(),
            DocumentEdit::SetPointPosition {
                point: ids.source_end,
                position: [-3.0, 3.0],
            },
        ))
        .unwrap();
    assert!(rotated.accepted(), "{rotated:#?}");
    let source = exact.document().curve(ids.source).unwrap();
    let target = exact.document().curve(ids.target).unwrap();
    let (
        CurveDefinition::Line {
            start: source_start,
            end: source_end,
            ..
        },
        CurveDefinition::Line {
            start: target_start,
            end: target_end,
            ..
        },
    ) = (&source.definition, &target.definition)
    else {
        panic!("line offset parents")
    };
    let source_vector =
        point(exact.document(), *source_end) - point(exact.document(), *source_start);
    let target_vector =
        point(exact.document(), *target_end) - point(exact.document(), *target_start);
    assert!((source_vector - target_vector).norm() <= 1.0e-9);
    assert!(distance(point(exact.document(), *target_end), target_before) > 1.0);
    assert_eq!(exact.history_len(), 1);

    let (mut mirror, ids) = session(AlphaScenarioKind::EntityMirror);
    let AlphaScenarioIds::EntityMirror(ids) = ids else {
        panic!("mirror IDs")
    };
    let counterpart_before = point(mirror.document(), ids.mirrored_end);
    let edited = mirror
        .apply(DocumentCommand::new(
            mirror.revision(),
            DocumentEdit::SetPointPosition {
                point: ids.source_end,
                position: [-2.0, 1.0 + 10.0_f64.sqrt()],
            },
        ))
        .unwrap();
    assert!(edited.accepted(), "{edited:#?}");
    let source = point(mirror.document(), ids.source_end);
    let reflected = point(mirror.document(), ids.mirrored_end);
    assert!((source.x - reflected.x).abs() <= 1.0e-9);
    assert!((source.y + reflected.y).abs() <= 1.0e-9);
    assert!(distance(reflected, counterpart_before) > 1.0);
    assert_eq!(mirror.history_len(), 1);
    assert_round_trip(&mirror);
}

#[test]
fn directed_angle_crosses_cut_then_target_mode_and_orientation_edit_transactionally() {
    let (mut session, ids) = session(AlphaScenarioKind::DirectedAngle);
    let AlphaScenarioIds::DirectedAngle(ids) = ids else {
        panic!("directed angle IDs")
    };
    let initial = point(session.document(), ids.moving_tip);
    let angle = 170.0_f64.to_radians();
    let dragged = session
        .apply(DocumentCommand::new(
            session.revision(),
            DocumentEdit::SetPointPosition {
                point: ids.moving_tip,
                position: [3.0 * angle.cos(), 3.0 * angle.sin()],
            },
        ))
        .unwrap();
    assert!(dragged.accepted(), "{dragged:#?}");
    assert!(distance(point(session.document(), ids.moving_tip), initial) > 0.5);
    assert_eq!(session.history_len(), 1);

    let target = 5.0_f64.to_radians();
    let driven = session
        .transact(session.revision(), "drive directed angle", |document| {
            document.set_scalar_value(ids.target, target)?;
            document.set_oriented_angle_orientation(
                ids.dimension,
                DocumentAngleOrientation::Clockwise,
            )?;
            document.set_dimension_mode(ids.dimension, DocumentDimensionMode::Driving)
        })
        .unwrap();
    assert!(driven.accepted(), "{:#?}", driven.outcome.result.solve());
    assert_eq!(
        session
            .accepted_result()
            .accepted_view()
            .core_report
            .right_nullity,
        0
    );
    assert!(matches!(
        session
            .document()
            .dimension(ids.dimension)
            .unwrap()
            .definition,
        DocumentDimensionDefinition::OrientedAngle {
            orientation: DocumentAngleOrientation::Clockwise,
            ..
        }
    ));
    let retained = session.export_json().unwrap();
    let history = session.history_len();
    let rejected = session.apply(DocumentCommand::new(
        session.revision(),
        DocumentEdit::SetScalarValue {
            scalar: ids.target,
            value: f64::NAN,
        },
    ));
    assert!(rejected.is_err());
    assert_eq!(session.export_json().unwrap(), retained);
    assert_eq!(session.history_len(), history);
}

fn assert_fillet_association(
    document: &SketchDocument,
    ids: &geosolve_sketch::CurveCurveFilletIds,
) {
    let start = document
        .evaluate_curve_jet(CurveSpan::line(ids.arc), 0.0)
        .unwrap()
        .position;
    let end = document
        .evaluate_curve_jet(CurveSpan::line(ids.arc), 1.0)
        .unwrap()
        .position;
    let contacts = ids
        .contacts
        .map(|contact| document.evaluate_contact_jet(contact).unwrap().position);
    let constraint = document.constraint(ids.constraint).unwrap();
    let DocumentConstraintDefinition::CurveCurveFillet { endpoint_order, .. } =
        constraint.definition
    else {
        panic!("generic fillet")
    };
    let expected = match endpoint_order {
        DocumentFilletEndpointOrder::FirstThenSecond => contacts,
        DocumentFilletEndpointOrder::SecondThenFirst => [contacts[1], contacts[0]],
    };
    assert!((start - expected[0]).norm() <= 1.0e-8);
    assert!((end - expected[1]).norm() <= 1.0e-8);
}

#[test]
fn line_and_generic_fillet_drags_move_contacts_output_and_trim_state() {
    let (mut line, ids) = session(AlphaScenarioKind::M27ReferenceFillet);
    let AlphaScenarioIds::M27ReferenceFillet(ids) = ids else {
        panic!("M27 IDs")
    };
    assert!(line.document().trim_views().is_empty());
    let center_before = point(line.document(), ids.fillet.center);
    let line_contact_before = line
        .document()
        .evaluate_contact_jet(ids.fillet.contacts[0])
        .unwrap()
        .position;
    let moved = line
        .apply(DocumentCommand::new(
            line.revision(),
            DocumentEdit::SetPointPosition {
                point: ids.fillet.center,
                position: [-2.0, 2.0],
            },
        ))
        .unwrap();
    assert!(moved.accepted(), "{moved:#?}");
    assert!(distance(point(line.document(), ids.fillet.center), center_before) > 1.0);
    assert!(
        (line
            .document()
            .evaluate_contact_jet(ids.fillet.contacts[0])
            .unwrap()
            .position
            - line_contact_before)
            .norm()
            > 1.0
    );
    assert!(line.document().trim_views().is_empty());

    for kind in [
        AlphaScenarioKind::FilletLineCircle,
        AlphaScenarioKind::FilletLineBezier,
        AlphaScenarioKind::FilletNurbsLine,
    ] {
        let (mut session, ids) = session(kind);
        let (AlphaScenarioIds::FilletLineCircle(ids)
        | AlphaScenarioIds::FilletLineBezier(ids)
        | AlphaScenarioIds::FilletNurbsLine(ids)) = ids
        else {
            panic!("generic fillet IDs")
        };
        let center_before = point(session.document(), ids.fillet.center);
        let contacts_before = ids.fillet.contacts.map(|contact| {
            session
                .document()
                .evaluate_contact_jet(contact)
                .unwrap()
                .position
        });
        let intervals_before = ids
            .parents
            .map(|curve| session.document().visible_curve_intervals(curve).unwrap());
        let requested = [center_before.x + 0.2, center_before.y + 0.15];
        if kind == AlphaScenarioKind::FilletNurbsLine {
            let mut preview = session.clone();
            for step in 1..=8 {
                let fraction = f64::from(step) / 8.0;
                let target = [
                    (requested[0] - center_before.x).mul_add(fraction, center_before.x),
                    (requested[1] - center_before.y).mul_add(fraction, center_before.y),
                ];
                let moved = preview
                    .rebuild_request(
                        preview.revision(),
                        DocumentSolveRequest::default()
                            .without_previous_state_preferences()
                            .with_drag(ids.fillet.center, target),
                    )
                    .unwrap();
                assert!(
                    moved.accepted(),
                    "{} step {step}: {:#?}",
                    kind.key(),
                    moved.solve()
                );
            }
            preview
                .rebuild_request(
                    preview.revision(),
                    DocumentSolveRequest::default().without_previous_state_preferences(),
                )
                .unwrap();
            let accepted_preview = preview.document().clone();
            let committed = session
                .transact(
                    session.revision(),
                    "projected NURBS fillet drag",
                    move |document| {
                        *document = accepted_preview;
                        Ok(())
                    },
                )
                .unwrap();
            assert!(
                committed.accepted(),
                "{}: {:#?}",
                kind.key(),
                committed.outcome.result.solve()
            );
        } else {
            let moved = session
                .apply(DocumentCommand::new(
                    session.revision(),
                    DocumentEdit::SetPointPosition {
                        point: ids.fillet.center,
                        position: requested,
                    },
                ))
                .unwrap();
            assert!(moved.accepted(), "{}: {moved:#?}", kind.key());
        }
        let center_after = point(session.document(), ids.fillet.center);
        assert!(
            distance(center_after, center_before) > 1.0e-4,
            "{}",
            kind.key()
        );
        let contacts_after = ids.fillet.contacts.map(|contact| {
            session
                .document()
                .evaluate_contact_jet(contact)
                .unwrap()
                .position
        });
        assert!(
            contacts_before
                .iter()
                .zip(contacts_after)
                .any(|(before, after)| (*before - after).norm() > 1.0e-4),
            "{}",
            kind.key()
        );
        let intervals_after = ids
            .parents
            .map(|curve| session.document().visible_curve_intervals(curve).unwrap());
        assert_ne!(intervals_before, intervals_after, "{}", kind.key());
        assert_fillet_association(session.document(), &ids.fillet);
        assert_eq!(session.history_len(), 1);
        assert_round_trip(&session);
    }
}

#[test]
fn fillet_branch_radius_history_and_invalid_edit_are_atomic() {
    let (mut fillet_session, ids) = session(AlphaScenarioKind::M27ReferenceFillet);
    let AlphaScenarioIds::M27ReferenceFillet(ids) = ids else {
        panic!("M27 IDs")
    };
    let edited = fillet_session
        .transact(
            fillet_session.revision(),
            "fillet branch and radius",
            |document| {
                document.set_line_line_fillet_branch(
                    ids.fillet.constraint,
                    DocumentCurveNormalSide::Left,
                    DocumentCurveNormalSide::Left,
                    DocumentFilletEndpointOrder::SecondThenFirst,
                    geosolve_sketch::DocumentArcSweep::Clockwise,
                )?;
                document
                    .set_dimension_mode(ids.fillet.radius_dimension, DocumentDimensionMode::Driving)
            },
        )
        .unwrap();
    assert!(edited.accepted(), "{:#?}", edited.outcome.result.solve());
    assert_eq!(fillet_session.history_len(), 1);
    let accepted = fillet_session.export_json().unwrap();
    fillet_session.undo(fillet_session.revision()).unwrap();
    fillet_session.redo(fillet_session.revision()).unwrap();
    assert_eq!(fillet_session.export_json().unwrap(), accepted);

    let history = fillet_session.history_len();
    let retained = fillet_session.export_json().unwrap();
    let rejected = fillet_session.apply(DocumentCommand::new(
        fillet_session.revision(),
        DocumentEdit::SetScalarValue {
            scalar: ids.fillet.radius_target,
            value: 10.0,
        },
    ));
    assert!(rejected.is_err() || !rejected.unwrap().accepted());
    assert_eq!(fillet_session.history_len(), history);
    assert_eq!(fillet_session.export_json().unwrap(), retained);

    let (mut generic, ids) = session(AlphaScenarioKind::FilletLineBezier);
    let AlphaScenarioIds::FilletLineBezier(ids) = ids else {
        panic!("generic IDs")
    };
    let branch = generic
        .apply(DocumentCommand::new(
            generic.revision(),
            DocumentEdit::SetCurveCurveFilletBranch {
                constraint: ids.fillet.constraint,
                first_side: DocumentCurveNormalSide::Left,
                first_trim_endpoint: DocumentFilletTrimEndpoint::Start,
                second_side: DocumentCurveNormalSide::Left,
                second_trim_endpoint: DocumentFilletTrimEndpoint::End,
                endpoint_order: DocumentFilletEndpointOrder::SecondThenFirst,
                sweep: geosolve_sketch::DocumentArcSweep::Clockwise,
            },
        ))
        .unwrap();
    assert!(branch.accepted(), "{branch:#?}");
    assert_fillet_association(generic.document(), &ids.fillet);
}

#[test]
fn nurbs_weight_gauge_insertion_transition_and_differential_controls_are_transactional() {
    let (mut quarter, ids) = session(AlphaScenarioKind::NurbsQuarterCircle);
    let AlphaScenarioIds::NurbsQuarterCircle(ids) = ids else {
        panic!("quarter-circle IDs")
    };
    let midpoint_before = quarter
        .document()
        .evaluate_curve_jet(
            CurveSpan {
                curve: ids.curve,
                segment: 7,
            },
            0.5,
        )
        .unwrap()
        .position;
    let weight_edit = quarter
        .apply(DocumentCommand::new(
            quarter.revision(),
            DocumentEdit::SetScalarValue {
                scalar: ids.weights[1],
                value: 0.45,
            },
        ))
        .unwrap();
    assert!(weight_edit.accepted());
    let midpoint_after = quarter
        .document()
        .evaluate_curve_jet(
            CurveSpan {
                curve: ids.curve,
                segment: 7,
            },
            0.5,
        )
        .unwrap()
        .position;
    assert!((midpoint_after - midpoint_before).norm() > 0.05);
    let before_gauge = [0.1, 0.5, 0.9].map(|parameter| {
        quarter
            .document()
            .evaluate_curve_jet(
                CurveSpan {
                    curve: ids.curve,
                    segment: 7,
                },
                parameter,
            )
            .unwrap()
            .position
    });
    let regauged = quarter
        .apply(DocumentCommand::new(
            quarter.revision(),
            DocumentEdit::SetNurbsWeightGauge {
                curve: ids.curve,
                gauge_weight: ids.weights[2],
            },
        ))
        .unwrap();
    assert!(regauged.accepted());
    for (parameter, before) in [0.1, 0.5, 0.9].into_iter().zip(before_gauge) {
        let after = quarter
            .document()
            .evaluate_curve_jet(
                CurveSpan {
                    curve: ids.curve,
                    segment: 7,
                },
                parameter,
            )
            .unwrap()
            .position;
        assert!((after - before).norm() <= 1.0e-10);
    }
    assert_eq!(quarter.history_len(), 2);

    let (mut local, ids) = session(AlphaScenarioKind::NurbsLocalSupport);
    let AlphaScenarioIds::NurbsLocalSupport(ids) = ids else {
        panic!("local-support IDs")
    };
    let before = local.document().curve_spans(ids.curve).unwrap();
    let insertion = local
        .apply(DocumentCommand::new(
            local.revision(),
            DocumentEdit::InsertNurbsKnot {
                curve: ids.curve,
                parameter: 0.5,
            },
        ))
        .unwrap();
    assert!(insertion.accepted(), "{insertion:#?}");
    assert_eq!(
        local.document().curve_spans(ids.curve).unwrap().len(),
        before.len() + 1
    );
    let inserted = local.export_json().unwrap();
    local.undo(local.revision()).unwrap();
    assert_eq!(local.document().curve_spans(ids.curve).unwrap(), before);
    local.redo(local.revision()).unwrap();
    assert_eq!(local.export_json().unwrap(), inserted);

    let (mut periodic, ids) = session(AlphaScenarioKind::NurbsPeriodic);
    let AlphaScenarioIds::NurbsPeriodic(ids) = ids else {
        panic!("periodic IDs")
    };
    let contact = ids.contact.unwrap();
    let seam = periodic
        .document()
        .evaluate_contact_jet(contact)
        .unwrap()
        .position;
    let transitioned = periodic
        .apply(DocumentCommand::new(
            periodic.revision(),
            DocumentEdit::TransitionNurbsContact {
                contact,
                direction: DocumentBSplineSpanDirection::Next,
            },
        ))
        .unwrap();
    assert!(transitioned.accepted());
    let slot = periodic.document().contact(contact).unwrap();
    assert_eq!(slot.curve.segment, 11);
    assert_eq!(slot.winding, 3);
    assert_eq!(slot.neighborhood, ContactNeighborhood::Start);
    assert!(
        (periodic
            .document()
            .evaluate_contact_jet(contact)
            .unwrap()
            .position
            - seam)
            .norm()
            <= 1.0e-10
    );
    let previous = periodic
        .apply(DocumentCommand::new(
            periodic.revision(),
            DocumentEdit::TransitionNurbsContact {
                contact,
                direction: DocumentBSplineSpanDirection::Previous,
            },
        ))
        .unwrap();
    assert!(previous.accepted());
    assert_eq!(periodic.document().contact(contact).unwrap().winding, 2);

    let retained = periodic.export_json().unwrap();
    let history = periodic.history_len();
    let invalid = periodic.apply(DocumentCommand::new(
        periodic.revision(),
        DocumentEdit::SetScalarValue {
            scalar: ids.weights[1],
            value: 2.0,
        },
    ));
    assert!(invalid.is_err());
    assert_eq!(periodic.export_json().unwrap(), retained);
    assert_eq!(periodic.history_len(), history);

    let (mut differential, ids) = session(AlphaScenarioKind::NurbsDifferential);
    let AlphaScenarioIds::NurbsDifferential(ids) = ids else {
        panic!("differential IDs")
    };
    for contact in ids.contacts {
        let curvature = differential
            .document()
            .measure_curve_contact(contact, DocumentCurveMeasurementKind::SignedCurvature)
            .unwrap();
        assert!(curvature.is_finite());
    }
    let seam_before = point(differential.document(), ids.seam);
    let moved = differential
        .apply(DocumentCommand::new(
            differential.revision(),
            DocumentEdit::SetPointPosition {
                point: ids.seam,
                position: [0.25, 0.2],
            },
        ))
        .unwrap();
    assert!(moved.accepted(), "{moved:#?}");
    assert!(distance(point(differential.document(), ids.seam), seam_before) > 0.1);
    let first = differential
        .document()
        .evaluate_contact_jet(ids.contacts[0])
        .unwrap();
    let second = differential
        .document()
        .evaluate_contact_jet(ids.contacts[1])
        .unwrap();
    assert!((first.position - second.position).norm() <= 1.0e-8);
    assert_round_trip(&differential);
}

#[test]
fn construction_creation_commands_use_public_side_orientation_target_and_mode_state() {
    let mut document = SketchDocument::new(1.0).unwrap();
    let points = [[-3.0, 0.0], [3.0, 0.0], [-2.0, 2.0], [2.0, 2.0]].map(|position| {
        document
            .add_point("offset command point", position)
            .unwrap()
    });
    let curves = [
        document
            .add_curve(
                "offset command source",
                CurveDefinition::Line {
                    start: points[0],
                    end: points[1],
                    branch_direction: [1.0, 0.0],
                },
            )
            .unwrap(),
        document
            .add_curve(
                "offset command target",
                CurveDefinition::Line {
                    start: points[3],
                    end: points[2],
                    branch_direction: [-1.0, 0.0],
                },
            )
            .unwrap(),
    ];
    for point in points {
        let target = document.point(point).unwrap().position;
        document
            .add_constraint(
                "offset command fixed point",
                DocumentConstraintDefinition::FixedPoint { point, target },
            )
            .unwrap();
    }
    let mut session = SketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let created = session
        .transact(session.revision(), "create supporting offset", |document| {
            let target = document.add_scalar(
                "browser offset target",
                2.0,
                geosolve_sketch::ScalarUnit::Length,
                geosolve_sketch::ScalarDomain::Positive,
            )?;
            document.add_dimension(
                "browser supporting offset",
                DocumentDimensionDefinition::SupportingLineOffset {
                    source: CurveSpan::line(curves[0]),
                    target_segment: CurveSpan::line(curves[1]),
                    target,
                    side: DocumentLineSide::Left,
                    orientation: DocumentLineOffsetOrientation::Reversed,
                },
                DocumentDimensionMode::Driving,
            )
        })
        .unwrap();
    assert!(created.accepted());
    let dimension = created.value.unwrap();
    assert!(matches!(
        session.document().dimension(dimension).unwrap().definition,
        DocumentDimensionDefinition::SupportingLineOffset {
            side: DocumentLineSide::Left,
            orientation: DocumentLineOffsetOrientation::Reversed,
            ..
        }
    ));
    let json = session.export_json().unwrap();
    session.undo(session.revision()).unwrap();
    session.redo(session.revision()).unwrap();
    assert_eq!(session.export_json().unwrap(), json);
}
