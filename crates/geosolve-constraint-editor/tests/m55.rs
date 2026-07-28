// SPDX-License-Identifier: GPL-3.0-or-later
#![allow(
    clippy::too_many_lines,
    reason = "complete action matrices are clearer as contiguous integration scenarios"
)]

use geosolve_constraint_editor::{
    ActionChoice, ActionState, BranchAction, ConstraintActionRequest, ConstraintKind,
    ContactActionChoice, CoordinatorActionKind, DimensionActionRequest, DimensionKind,
    RetainedEditorCoordinator, SelectionItem,
};
use geosolve_sketch::{
    AlphaScenarioIds, AlphaScenarioKind, ContactDomain, ContactNeighborhood, CurveDefinition,
    CurveSpan, DocumentAngleOrientation, DocumentCurveSpanRef, DocumentDimensionDefinition,
    DocumentDimensionMode, DocumentSolveRequest, RetainedSketchDocumentSession, ScalarDomain,
    ScalarUnit, SketchDocument, TangentOrientation, alpha_scenario,
};

struct MatrixFixture {
    document: SketchDocument,
    points: [geosolve_sketch::DesignPointId; 6],
    lines: [CurveSpan; 2],
    circles: [CurveSpan; 2],
    midpoint: geosolve_sketch::DesignPointId,
    overlapping_line: CurveSpan,
}

fn matrix_fixture() -> MatrixFixture {
    let mut document = SketchDocument::new(10.0).unwrap();
    let points = [
        document.add_point("a", [-2.0, 0.0]).unwrap(),
        document.add_point("b", [2.0, 0.0]).unwrap(),
        document.add_point("c", [0.0, -2.0]).unwrap(),
        document.add_point("d", [0.0, 2.0]).unwrap(),
        document.add_point("e", [-3.0, 3.0]).unwrap(),
        document.add_point("f", [3.0, 3.0]).unwrap(),
    ];
    let first_line = document
        .add_curve(
            "horizontal",
            CurveDefinition::Line {
                start: points[0],
                end: points[1],
                branch_direction: [1.0, 0.0],
            },
        )
        .unwrap();
    let second_line = document
        .add_curve(
            "vertical",
            CurveDefinition::Line {
                start: points[2],
                end: points[3],
                branch_direction: [0.0, 1.0],
            },
        )
        .unwrap();
    let first_radius = document
        .add_scalar(
            "first radius",
            1.0,
            ScalarUnit::Length,
            ScalarDomain::Positive,
        )
        .unwrap();
    let first_circle = document
        .add_curve(
            "first circle",
            CurveDefinition::Circle {
                center: points[4],
                radius: first_radius,
            },
        )
        .unwrap();
    let second_radius = document
        .add_scalar(
            "second radius",
            1.0,
            ScalarUnit::Length,
            ScalarDomain::Positive,
        )
        .unwrap();
    let second_circle = document
        .add_curve(
            "second circle",
            CurveDefinition::Circle {
                center: points[5],
                radius: second_radius,
            },
        )
        .unwrap();
    let midpoint = document.add_point("midpoint", [0.0, 0.0]).unwrap();
    let overlap_start = document.add_point("overlap a", [-2.0, 0.0]).unwrap();
    let overlap_end = document.add_point("overlap b", [2.0, 0.0]).unwrap();
    let overlapping_line = CurveSpan::line(
        document
            .add_curve(
                "overlapping line",
                CurveDefinition::Line {
                    start: overlap_start,
                    end: overlap_end,
                    branch_direction: [1.0, 0.0],
                },
            )
            .unwrap(),
    );
    MatrixFixture {
        document,
        points,
        lines: [CurveSpan::line(first_line), CurveSpan::line(second_line)],
        circles: [
            CurveSpan::line(first_circle),
            CurveSpan::line(second_circle),
        ],
        midpoint,
        overlapping_line,
    }
}

fn coordinator(document: SketchDocument) -> RetainedEditorCoordinator {
    #[allow(clippy::default_trait_access)]
    let session = RetainedSketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        Default::default(),
    )
    .unwrap();
    RetainedEditorCoordinator::new(session).unwrap()
}

fn assert_enabled(coordinator: &RetainedEditorCoordinator, action: CoordinatorActionKind) {
    assert!(
        coordinator
            .actions()
            .iter()
            .any(|value| value.action == action && value.state == ActionState::Enabled),
        "{action:?} was not enabled for {:?}",
        coordinator.editor().selection()
    );
}

#[test]
fn complete_relation_and_dimension_matrix_is_headless_and_selection_scoped() {
    let fixture = matrix_fixture();
    let mut coordinator = coordinator(fixture.document);

    coordinator
        .editor_mut()
        .set_selection([SelectionItem::Point(fixture.points[0])]);
    assert_enabled(
        &coordinator,
        CoordinatorActionKind::Constraint(ConstraintKind::Fixed),
    );

    coordinator.editor_mut().set_selection(
        fixture.points[0..2]
            .iter()
            .copied()
            .map(SelectionItem::Point),
    );
    assert_enabled(
        &coordinator,
        CoordinatorActionKind::Constraint(ConstraintKind::Coincident),
    );
    for mode in [
        DocumentDimensionMode::Driving,
        DocumentDimensionMode::Reference,
    ] {
        assert_enabled(
            &coordinator,
            CoordinatorActionKind::Dimension(DimensionKind::PointDistance, mode),
        );
    }

    coordinator
        .editor_mut()
        .set_selection([SelectionItem::Curve(fixture.lines[0])]);
    for kind in [ConstraintKind::Horizontal, ConstraintKind::Vertical] {
        assert_enabled(&coordinator, CoordinatorActionKind::Constraint(kind));
    }
    for mode in [
        DocumentDimensionMode::Driving,
        DocumentDimensionMode::Reference,
    ] {
        assert_enabled(
            &coordinator,
            CoordinatorActionKind::Dimension(DimensionKind::SegmentLength, mode),
        );
    }

    coordinator.editor_mut().set_selection([
        SelectionItem::Point(fixture.points[0]),
        SelectionItem::Curve(fixture.lines[0]),
    ]);
    for kind in [ConstraintKind::PointOnCurve, ConstraintKind::Midpoint] {
        assert_enabled(&coordinator, CoordinatorActionKind::Constraint(kind));
    }

    coordinator
        .editor_mut()
        .set_selection(fixture.lines.map(SelectionItem::Curve));
    for kind in [
        ConstraintKind::Parallel,
        ConstraintKind::Perpendicular,
        ConstraintKind::EqualLength,
        ConstraintKind::GenericContact,
        ConstraintKind::GenericTangency,
    ] {
        assert_enabled(&coordinator, CoordinatorActionKind::Constraint(kind));
    }
    for mode in [
        DocumentDimensionMode::Driving,
        DocumentDimensionMode::Reference,
    ] {
        assert_enabled(
            &coordinator,
            CoordinatorActionKind::Dimension(DimensionKind::OrientedAngle, mode),
        );
    }

    coordinator
        .editor_mut()
        .set_selection(fixture.circles.map(SelectionItem::Curve));
    for kind in [
        ConstraintKind::EqualRadius,
        ConstraintKind::GenericContact,
        ConstraintKind::GenericTangency,
    ] {
        assert_enabled(&coordinator, CoordinatorActionKind::Constraint(kind));
    }
    coordinator
        .editor_mut()
        .set_selection([SelectionItem::Curve(fixture.circles[0])]);
    for mode in [
        DocumentDimensionMode::Driving,
        DocumentDimensionMode::Reference,
    ] {
        for kind in [DimensionKind::Radius, DimensionKind::Diameter] {
            assert_enabled(&coordinator, CoordinatorActionKind::Dimension(kind, mode));
        }
    }

    coordinator.editor_mut().set_selection([
        SelectionItem::Point(fixture.points[4]),
        SelectionItem::Point(fixture.points[5]),
        SelectionItem::Curve(fixture.lines[0]),
    ]);
    assert_enabled(
        &coordinator,
        CoordinatorActionKind::Constraint(ConstraintKind::Symmetry),
    );
}

#[test]
fn contact_action_metadata_exposes_domain_span_neighborhood_winding_and_orientation() {
    let fixture = matrix_fixture();
    let mut coordinator = coordinator(fixture.document);
    coordinator
        .editor_mut()
        .set_selection(fixture.lines.map(SelectionItem::Curve));
    let choices = coordinator.action_choices(CoordinatorActionKind::Constraint(
        ConstraintKind::GenericTangency,
    ));
    assert_eq!(choices.len(), 2);
    for (index, choice) in choices.iter().enumerate() {
        let ActionChoice::Contact {
            operand,
            span,
            domains,
            neighborhoods,
            tangent_orientations,
            default_winding,
            ..
        } = choice
        else {
            panic!("contact choice expected");
        };
        assert_eq!(usize::from(*operand), index);
        assert_eq!(*span, fixture.lines[index]);
        assert!(domains.contains(&ContactDomain::SupportingLine));
        assert!(domains.contains(&ContactDomain::Bounded {
            lower: 0.0,
            upper: 1.0,
        }));
        assert!(neighborhoods.contains(&ContactNeighborhood::Interior));
        assert_eq!(
            tangent_orientations,
            &[TangentOrientation::Aligned, TangentOrientation::Opposed]
        );
        assert_eq!(*default_winding, 0);
    }
}

#[test]
fn every_required_relation_executes_through_the_typed_coordinator_action() {
    let fixture = matrix_fixture();
    let contact = |span, orientation| ContactActionChoice {
        support: DocumentCurveSpanRef { span, winding: 0 },
        domain: ContactDomain::Bounded {
            lower: 0.0,
            upper: 1.0,
        },
        parameter: 0.5,
        neighborhood: ContactNeighborhood::Interior,
        tangent_orientation: orientation,
    };
    let cases = vec![
        (
            ConstraintKind::Fixed,
            vec![SelectionItem::Point(fixture.points[0])],
            Vec::new(),
        ),
        (
            ConstraintKind::Coincident,
            vec![
                SelectionItem::Point(fixture.points[0]),
                SelectionItem::Point(fixture.points[1]),
            ],
            Vec::new(),
        ),
        (
            ConstraintKind::Horizontal,
            vec![SelectionItem::Curve(fixture.lines[0])],
            Vec::new(),
        ),
        (
            ConstraintKind::Vertical,
            vec![SelectionItem::Curve(fixture.lines[1])],
            Vec::new(),
        ),
        (
            ConstraintKind::PointOnCurve,
            vec![
                SelectionItem::Point(fixture.midpoint),
                SelectionItem::Curve(fixture.lines[0]),
            ],
            vec![contact(fixture.lines[0], None)],
        ),
        (
            ConstraintKind::Parallel,
            vec![
                SelectionItem::Curve(fixture.lines[0]),
                SelectionItem::Curve(fixture.overlapping_line),
            ],
            Vec::new(),
        ),
        (
            ConstraintKind::Perpendicular,
            fixture.lines.map(SelectionItem::Curve).to_vec(),
            Vec::new(),
        ),
        (
            ConstraintKind::EqualLength,
            fixture.lines.map(SelectionItem::Curve).to_vec(),
            Vec::new(),
        ),
        (
            ConstraintKind::EqualRadius,
            fixture.circles.map(SelectionItem::Curve).to_vec(),
            Vec::new(),
        ),
        (
            ConstraintKind::Midpoint,
            vec![
                SelectionItem::Point(fixture.midpoint),
                SelectionItem::Curve(fixture.lines[0]),
            ],
            Vec::new(),
        ),
        (
            ConstraintKind::Symmetry,
            vec![
                SelectionItem::Point(fixture.points[4]),
                SelectionItem::Point(fixture.points[5]),
                SelectionItem::Curve(fixture.lines[1]),
            ],
            Vec::new(),
        ),
        (
            ConstraintKind::GenericContact,
            fixture.lines.map(SelectionItem::Curve).to_vec(),
            fixture.lines.map(|span| contact(span, None)).to_vec(),
        ),
        (
            ConstraintKind::GenericTangency,
            vec![
                SelectionItem::Curve(fixture.lines[0]),
                SelectionItem::Curve(fixture.overlapping_line),
            ],
            vec![
                contact(fixture.lines[0], Some(TangentOrientation::Aligned)),
                contact(fixture.overlapping_line, Some(TangentOrientation::Aligned)),
            ],
        ),
    ];
    for (kind, selection, contacts) in cases {
        let mut coordinator = coordinator(fixture.document.clone());
        coordinator.editor_mut().set_selection(selection);
        let expected = coordinator.session().design_identity();
        let outcome = coordinator
            .apply_constraint_action(
                expected,
                ConstraintActionRequest {
                    kind,
                    label: format!("{kind:?}"),
                    contacts,
                },
            )
            .unwrap_or_else(|error| panic!("{kind:?} failed: {error}"));
        assert!(
            outcome.published_accepted.is_some(),
            "{kind:?} produced a rejected attempt"
        );
        assert!(
            coordinator
                .session()
                .design_document()
                .constraints()
                .iter()
                .any(|constraint| constraint.id == outcome.value)
        );
    }
}

#[test]
fn all_alpha_dimension_families_execute_as_reference_actions() {
    let fixture = matrix_fixture();
    let points = fixture.points;
    let lines = fixture.lines;
    let circles = fixture.circles;
    let mut editor = coordinator(fixture.document);
    let cases = [
        (
            vec![
                SelectionItem::Point(points[0]),
                SelectionItem::Point(points[1]),
            ],
            DimensionKind::PointDistance,
        ),
        (
            vec![SelectionItem::Curve(lines[0])],
            DimensionKind::SegmentLength,
        ),
        (
            vec![SelectionItem::Curve(circles[0])],
            DimensionKind::Radius,
        ),
        (
            vec![SelectionItem::Curve(circles[1])],
            DimensionKind::Diameter,
        ),
        (
            vec![
                SelectionItem::Curve(lines[0]),
                SelectionItem::Curve(lines[1]),
            ],
            DimensionKind::OrientedAngle,
        ),
    ];
    for (selection, kind) in cases {
        editor.editor_mut().set_selection(selection);
        let expected = editor.session().design_identity();
        let outcome = editor
            .apply_dimension_action(
                expected,
                DimensionActionRequest {
                    kind,
                    mode: DocumentDimensionMode::Reference,
                    label: format!("{kind:?}"),
                    angle_orientation: DocumentAngleOrientation::CounterClockwise,
                },
            )
            .unwrap();
        assert!(outcome.published_accepted.is_some());
    }
    let definitions = editor
        .session()
        .design_document()
        .dimensions()
        .iter()
        .map(|dimension| &dimension.definition)
        .collect::<Vec<_>>();
    assert!(matches!(
        definitions[0],
        DocumentDimensionDefinition::PointDistance { .. }
    ));
    assert!(matches!(
        definitions[1],
        DocumentDimensionDefinition::CurveLength { .. }
    ));
    assert!(matches!(
        definitions[2],
        DocumentDimensionDefinition::Radius { .. }
    ));
    assert!(matches!(
        definitions[3],
        DocumentDimensionDefinition::Diameter { .. }
    ));
    assert!(matches!(
        definitions[4],
        DocumentDimensionDefinition::OrientedAngle { .. }
    ));

    let angle = editor.session().design_document().dimensions()[4].id;
    editor
        .editor_mut()
        .set_selection([SelectionItem::Dimension(angle)]);
    assert!(matches!(
        editor.branch_actions().as_slice(),
        [BranchAction::AngleOrientation {
            dimension,
            current: DocumentAngleOrientation::CounterClockwise,
        }] if *dimension == angle
    ));
    let expected = editor.session().design_identity();
    let outcome = editor
        .set_selected_angle_orientation(expected, DocumentAngleOrientation::Clockwise)
        .unwrap();
    assert!(outcome.published_accepted.is_some());

    for kind in [
        DimensionKind::PointDistance,
        DimensionKind::SegmentLength,
        DimensionKind::Radius,
        DimensionKind::Diameter,
        DimensionKind::OrientedAngle,
    ] {
        let fixture = matrix_fixture();
        let selection = match kind {
            DimensionKind::PointDistance => vec![
                SelectionItem::Point(fixture.points[0]),
                SelectionItem::Point(fixture.points[1]),
            ],
            DimensionKind::SegmentLength => vec![SelectionItem::Curve(fixture.lines[0])],
            DimensionKind::Radius => vec![SelectionItem::Curve(fixture.circles[0])],
            DimensionKind::Diameter => vec![SelectionItem::Curve(fixture.circles[1])],
            DimensionKind::OrientedAngle => vec![
                SelectionItem::Curve(fixture.lines[0]),
                SelectionItem::Curve(fixture.lines[1]),
            ],
        };
        let mut driving = coordinator(fixture.document);
        driving.editor_mut().set_selection(selection);
        let expected = driving.session().design_identity();
        let outcome = driving
            .apply_dimension_action(
                expected,
                DimensionActionRequest {
                    kind,
                    mode: DocumentDimensionMode::Driving,
                    label: format!("driving {kind:?}"),
                    angle_orientation: DocumentAngleOrientation::CounterClockwise,
                },
            )
            .unwrap();
        assert!(
            outcome.published_accepted.is_some(),
            "driving {kind:?} rejected"
        );
    }
}

#[test]
fn rejected_generic_contact_retains_accepted_state_and_undo_recovers() {
    let mut document = SketchDocument::new(10.0).unwrap();
    let points = [
        document.add_point("a", [-2.0, 0.0]).unwrap(),
        document.add_point("b", [2.0, 0.0]).unwrap(),
        document.add_point("c", [-2.0, 2.0]).unwrap(),
        document.add_point("d", [2.0, 2.0]).unwrap(),
    ];
    let lines = [
        CurveSpan::line(
            document
                .add_curve(
                    "first",
                    CurveDefinition::Line {
                        start: points[0],
                        end: points[1],
                        branch_direction: [1.0, 0.0],
                    },
                )
                .unwrap(),
        ),
        CurveSpan::line(
            document
                .add_curve(
                    "second",
                    CurveDefinition::Line {
                        start: points[2],
                        end: points[3],
                        branch_direction: [1.0, 0.0],
                    },
                )
                .unwrap(),
        ),
    ];
    for point in points {
        let position = document.point(point).unwrap().position;
        document
            .add_constraint(
                format!("fix {point}"),
                geosolve_sketch::DocumentConstraintDefinition::FixedPoint {
                    point,
                    target: position,
                },
            )
            .unwrap();
    }
    let mut coordinator = coordinator(document);
    let accepted_before = coordinator.session().accepted_state().unwrap().identity();
    coordinator
        .editor_mut()
        .set_selection(lines.map(SelectionItem::Curve));
    let expected = coordinator.session().design_identity();
    let outcome = coordinator
        .apply_constraint_action(
            expected,
            ConstraintActionRequest {
                kind: ConstraintKind::GenericContact,
                label: "impossible fixed contact".into(),
                contacts: lines
                    .into_iter()
                    .map(|span| ContactActionChoice {
                        support: DocumentCurveSpanRef { span, winding: 0 },
                        domain: ContactDomain::Bounded {
                            lower: 0.0,
                            upper: 1.0,
                        },
                        parameter: 0.25,
                        neighborhood: ContactNeighborhood::Interior,
                        tangent_orientation: None,
                    })
                    .collect(),
            },
        )
        .unwrap();
    assert!(outcome.published_accepted.is_none());
    assert_eq!(
        coordinator.session().accepted_state().unwrap().identity(),
        accepted_before
    );
    assert!(coordinator.current_problem_metadata().is_some());
    coordinator.undo().unwrap();
    assert!(coordinator.current_problem_metadata().is_none());
    assert_eq!(
        coordinator.session().design_document().constraints().len(),
        4
    );
}

#[test]
fn selected_contact_source_exposes_and_applies_complete_branch_state() {
    let fixture = matrix_fixture();
    let mut coordinator = coordinator(fixture.document);
    coordinator
        .editor_mut()
        .set_selection(fixture.lines.map(SelectionItem::Curve));
    let expected = coordinator.session().design_identity();
    let created = coordinator
        .apply_constraint_action(
            expected,
            ConstraintActionRequest {
                kind: ConstraintKind::GenericContact,
                label: "crossing contact".into(),
                contacts: fixture
                    .lines
                    .into_iter()
                    .map(|span| ContactActionChoice {
                        support: DocumentCurveSpanRef { span, winding: 0 },
                        domain: ContactDomain::Bounded {
                            lower: 0.0,
                            upper: 1.0,
                        },
                        parameter: 0.5,
                        neighborhood: ContactNeighborhood::Interior,
                        tangent_orientation: None,
                    })
                    .collect(),
            },
        )
        .unwrap();
    assert!(created.published_accepted.is_some());
    coordinator
        .editor_mut()
        .set_selection([SelectionItem::Constraint(created.value)]);
    let branches = coordinator.branch_actions();
    assert_eq!(branches.len(), 2);
    let edits = branches
        .iter()
        .map(|branch| {
            let BranchAction::Contact(branch) = branch else {
                panic!("contact branch expected");
            };
            assert!(branch.domains.contains(&ContactDomain::SupportingLine));
            geosolve_sketch::ContactBranchEdit {
                domain: ContactDomain::SupportingLine,
                ..branch.current
            }
        })
        .collect::<Vec<_>>();
    let contacts = edits.iter().map(|edit| edit.contact).collect::<Vec<_>>();
    let expected = coordinator.session().design_identity();
    let outcome = coordinator.set_contact_branches(expected, edits).unwrap();
    assert!(outcome.published_accepted.is_some());
    let updated = coordinator.branch_actions();
    assert_eq!(
        updated
            .iter()
            .map(|branch| match branch {
                BranchAction::Contact(branch) => branch.current.contact,
                BranchAction::AngleOrientation { .. } => panic!("contact branch expected"),
            })
            .collect::<Vec<_>>(),
        contacts
    );
    assert!(updated.iter().all(|branch| matches!(
        branch,
        BranchAction::Contact(branch)
            if branch.current.domain == ContactDomain::SupportingLine
    )));
}

#[test]
fn complete_branch_edits_apply_span_parameter_neighborhood_winding_and_orientation() {
    let mut document = SketchDocument::new(4.0).unwrap();
    let points = [
        document.add_point("polyline start", [-1.0, 0.0]).unwrap(),
        document.add_point("polyline join", [0.0, 0.0]).unwrap(),
        document.add_point("polyline end", [1.0, 0.0]).unwrap(),
    ];
    let point = document.add_point("contact point", [0.0, 0.0]).unwrap();
    let curve = document
        .add_curve(
            "two-span polyline",
            CurveDefinition::Polyline {
                points: points.to_vec(),
                closed: false,
                branch_directions: vec![[1.0, 0.0], [1.0, 0.0]],
            },
        )
        .unwrap();
    let first_span = CurveSpan { curve, segment: 0 };
    let second_span = CurveSpan { curve, segment: 1 };
    let contact = document
        .add_curve_contact(
            "join contact",
            first_span,
            1.0,
            0,
            ContactNeighborhood::End,
            None,
        )
        .unwrap();
    let constraint = document
        .add_constraint(
            "point at polyline join",
            geosolve_sketch::DocumentConstraintDefinition::PointOnCurve { point, contact },
        )
        .unwrap();
    let parameter = document
        .contacts()
        .iter()
        .find(|candidate| candidate.id == contact)
        .unwrap()
        .parameter;
    let mut editor = coordinator(document);
    editor
        .editor_mut()
        .set_selection([SelectionItem::Constraint(constraint)]);
    let branches = editor.branch_actions();
    let [BranchAction::Contact(branch)] = branches.as_slice() else {
        panic!("one point-contact branch action expected");
    };
    assert_eq!(branch.spans, [first_span, second_span]);
    let expected = editor.session().design_identity();
    let outcome = editor
        .set_contact_branches(
            expected,
            vec![geosolve_sketch::ContactBranchEdit {
                curve: second_span,
                value: 0.0,
                neighborhood: ContactNeighborhood::Start,
                ..branch.current
            }],
        )
        .unwrap();
    assert!(outcome.published_accepted.is_some());
    let updated = editor
        .session()
        .design_document()
        .contacts()
        .iter()
        .find(|candidate| candidate.id == contact)
        .unwrap();
    assert_eq!(updated.parameter, parameter);
    assert_eq!(updated.curve, second_span);
    assert_eq!(updated.neighborhood, ContactNeighborhood::Start);
    assert_eq!(
        editor
            .session()
            .design_document()
            .scalar(parameter)
            .unwrap()
            .value
            .to_bits(),
        0.0_f64.to_bits()
    );

    let fixture = alpha_scenario(AlphaScenarioKind::A3, 1.0).unwrap();
    let AlphaScenarioIds::A3(ids) = fixture.ids else {
        panic!("A3 IDs expected");
    };
    #[allow(clippy::default_trait_access)]
    let session =
        RetainedSketchDocumentSession::new(fixture.document, fixture.request, Default::default())
            .unwrap();
    let mut editor = RetainedEditorCoordinator::new(session).unwrap();
    editor
        .editor_mut()
        .set_selection([SelectionItem::Constraint(ids.tangency)]);
    let accepted = editor.session().accepted_state().unwrap().identity();
    let before = editor.branch_actions();
    let contact_ids = before
        .iter()
        .map(|action| match action {
            BranchAction::Contact(branch) => branch.current.contact,
            BranchAction::AngleOrientation { .. } => panic!("contact branch expected"),
        })
        .collect::<Vec<_>>();
    let edits = before
        .iter()
        .map(|action| {
            let BranchAction::Contact(branch) = action else {
                panic!("contact branch expected");
            };
            let orientation = match branch.current.tangent_orientation {
                Some(TangentOrientation::Aligned) => TangentOrientation::Opposed,
                Some(TangentOrientation::Opposed) => TangentOrientation::Aligned,
                None => panic!("A3 tangency orientation expected"),
            };
            let mut edit = geosolve_sketch::ContactBranchEdit {
                tangent_orientation: Some(orientation),
                ..branch.current
            };
            match edit.domain {
                ContactDomain::Bounded { .. } => {}
                ContactDomain::Periodic { period } => {
                    edit.value = (edit.value + period * 0.5).rem_euclid(period);
                    edit.neighborhood = ContactNeighborhood::Interior;
                }
                ContactDomain::SupportingLine => unreachable!("A3 line starts bounded"),
            }
            edit
        })
        .collect::<Vec<_>>();
    let expected = editor.session().design_identity();
    let outcome = editor
        .set_contact_branches(expected, edits.clone())
        .unwrap();
    assert!(outcome.published_accepted.is_none());
    assert_eq!(
        editor.session().accepted_state().unwrap().identity(),
        accepted
    );
    let oriented = editor.branch_actions();
    assert_eq!(
        oriented
            .iter()
            .map(|action| match action {
                BranchAction::Contact(branch) => branch.current,
                BranchAction::AngleOrientation { .. } => panic!("contact branch expected"),
            })
            .collect::<Vec<_>>(),
        edits
    );
    let winding_edits = oriented
        .iter()
        .map(|action| {
            let BranchAction::Contact(branch) = action else {
                panic!("contact branch expected");
            };
            let mut edit = branch.current;
            if matches!(edit.domain, ContactDomain::Periodic { .. }) {
                edit.winding += 1;
            }
            edit
        })
        .collect::<Vec<_>>();
    let expected = editor.session().design_identity();
    let outcome = editor
        .set_contact_branches(expected, winding_edits.clone())
        .unwrap();
    assert!(outcome.published_accepted.is_none());
    assert_eq!(
        editor.session().accepted_state().unwrap().identity(),
        accepted
    );
    let after = editor.branch_actions();
    assert_eq!(
        after
            .iter()
            .map(|action| match action {
                BranchAction::Contact(branch) => branch.current.contact,
                BranchAction::AngleOrientation { .. } => panic!("contact branch expected"),
            })
            .collect::<Vec<_>>(),
        contact_ids
    );
    assert_eq!(
        after
            .iter()
            .map(|action| match action {
                BranchAction::Contact(branch) => branch.current,
                BranchAction::AngleOrientation { .. } => panic!("contact branch expected"),
            })
            .collect::<Vec<_>>(),
        winding_edits
    );
    editor.undo().unwrap();
    assert!(editor.current_problem_metadata().is_some());
    editor.undo().unwrap();
    assert!(editor.current_problem_metadata().is_none());
    assert_ne!(editor.branch_actions(), after);
}
