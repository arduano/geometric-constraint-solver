// SPDX-License-Identifier: GPL-3.0-or-later
#![allow(
    clippy::too_many_lines,
    reason = "complete action matrices are clearer as contiguous integration scenarios"
)]

use geosolve_constraint_editor::{
    ActionAvailability, ActionChoice, ActionState, AuthoringMutation, AuthoringOperand,
    AuthoringOptions, AuthoringOutcome, AuthoringState, AuthoringTool, BranchAction,
    ConstraintActionRequest, ConstraintIntent, ConstraintRelationChoice, ContactActionChoice,
    CoordinatorActionKind, DimensionActionRequest, DimensionKind, DisabledReason, EditorScene,
    Modifiers, PointerInput, ResolvedConstraintKind, RetainedEditorCoordinator, ScreenPoint,
    SelectionItem, Viewport,
};
use geosolve_sketch::{
    AlphaScenarioIds, AlphaScenarioKind, ContactBranchEdit, ContactDomain, ContactNeighborhood,
    CurveDefinition, CurveSpan, DocumentAngleOrientation, DocumentBSplineForm,
    DocumentConstraintDefinition, DocumentCurveContinuity, DocumentCurveCurvatureRelation,
    DocumentCurveSpanRef, DocumentDimensionDefinition, DocumentDimensionMode, DocumentSolveRequest,
    RetainedSketchDocumentSession, ScalarDomain, ScalarUnit, SketchDocument,
    SketchLifecycleRevisionHighWater, SolveRejection, SolverConfig, TangentOrientation,
    alpha_scenario,
};

struct MatrixFixture {
    document: SketchDocument,
    points: [geosolve_sketch::DesignPointId; 6],
    lines: [CurveSpan; 2],
    circles: [CurveSpan; 2],
    beziers: [CurveSpan; 2],
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
    let first_bezier_controls = [
        document.add_point("bezier 1 start", [-4.0, -4.0]).unwrap(),
        document.add_point("bezier 1 middle", [-2.0, -2.0]).unwrap(),
        document.add_point("bezier 1 end", [0.0, -4.0]).unwrap(),
    ];
    let second_bezier_controls = [
        document.add_point("bezier 2 start", [0.0, -4.0]).unwrap(),
        document.add_point("bezier 2 middle", [2.0, -6.0]).unwrap(),
        document.add_point("bezier 2 end", [4.0, -4.0]).unwrap(),
    ];
    let beziers = [first_bezier_controls, second_bezier_controls].map(|controls| {
        CurveSpan::line(
            document
                .add_curve("quadratic", CurveDefinition::QuadraticBezier { controls })
                .unwrap(),
        )
    });
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
        beziers,
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
    let empty_actions = coordinator.actions();
    for intent in [
        ConstraintIntent::Lock,
        ConstraintIntent::Coincident,
        ConstraintIntent::Horizontal,
        ConstraintIntent::Vertical,
        ConstraintIntent::Parallel,
        ConstraintIntent::Perpendicular,
        ConstraintIntent::Equal,
        ConstraintIntent::Midpoint,
        ConstraintIntent::Symmetric,
        ConstraintIntent::Tangent,
        ConstraintIntent::Continuity,
        ConstraintIntent::Concentric,
        ConstraintIntent::Collinear,
    ] {
        assert!(empty_actions.contains(&ActionAvailability {
            action: CoordinatorActionKind::Constraint(intent),
            state: ActionState::Disabled(DisabledReason::EmptySelection),
        }));
        let mut authoring = AuthoringState::default();
        assert!(matches!(
            authoring.activate(
                coordinator.session().design_document(),
                AuthoringTool::Constraint(intent),
                &[],
            ),
            AuthoringOutcome::ModeEntered {
                tool: AuthoringTool::Constraint(actual),
                ..
            } if actual == intent
        ));
        assert!(authoring.pending().is_empty());
    }

    coordinator
        .editor_mut()
        .set_selection([SelectionItem::Point(fixture.points[0])]);
    assert_enabled(
        &coordinator,
        CoordinatorActionKind::Constraint(ConstraintIntent::Lock),
    );

    coordinator.editor_mut().set_selection(
        fixture.points[0..2]
            .iter()
            .copied()
            .map(SelectionItem::Point),
    );
    assert_enabled(
        &coordinator,
        CoordinatorActionKind::Constraint(ConstraintIntent::Coincident),
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
    for intent in [ConstraintIntent::Horizontal, ConstraintIntent::Vertical] {
        assert_enabled(&coordinator, CoordinatorActionKind::Constraint(intent));
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
    for intent in [ConstraintIntent::Coincident, ConstraintIntent::Midpoint] {
        assert_enabled(&coordinator, CoordinatorActionKind::Constraint(intent));
    }

    coordinator
        .editor_mut()
        .set_selection(fixture.lines.map(SelectionItem::Curve));
    for intent in [
        ConstraintIntent::Parallel,
        ConstraintIntent::Perpendicular,
        ConstraintIntent::Equal,
        ConstraintIntent::Coincident,
        ConstraintIntent::Tangent,
        ConstraintIntent::Continuity,
    ] {
        assert_enabled(&coordinator, CoordinatorActionKind::Constraint(intent));
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
    for intent in [
        ConstraintIntent::Equal,
        ConstraintIntent::Coincident,
        ConstraintIntent::Tangent,
    ] {
        assert_enabled(&coordinator, CoordinatorActionKind::Constraint(intent));
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
        CoordinatorActionKind::Constraint(ConstraintIntent::Symmetric),
    );
}

#[test]
fn contact_action_metadata_exposes_domain_span_neighborhood_winding_and_orientation() {
    let fixture = matrix_fixture();
    let mut coordinator = coordinator(fixture.document);
    coordinator
        .editor_mut()
        .set_selection(fixture.lines.map(SelectionItem::Curve));
    let choices =
        coordinator.action_choices(CoordinatorActionKind::Constraint(ConstraintIntent::Tangent));
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
fn curve_pick_parameters_seed_contact_actions_without_selecting_a_branch() {
    let fixture = matrix_fixture();
    let mut coordinator = coordinator(fixture.document);
    let accepted = coordinator.session().accepted_state().unwrap();
    let scene = EditorScene::from_accepted(
        accepted.identity().revision().get(),
        coordinator.session().design_identity(),
        accepted.document(),
        Viewport::new([1000.0, 700.0], [0.0, 0.0], 100.0).unwrap(),
        0.5,
    )
    .unwrap();
    coordinator.editor_mut().pointer_down(
        &scene,
        PointerInput {
            pointer_id: 1,
            position: ScreenPoint { x: 400.0, y: 350.0 },
            modifiers: Modifiers::default(),
        },
    );
    coordinator.editor_mut().pointer_down(
        &scene,
        PointerInput {
            pointer_id: 2,
            position: ScreenPoint { x: 500.0, y: 250.0 },
            modifiers: Modifiers {
                shift: true,
                ..Modifiers::default()
            },
        },
    );
    let parameters = coordinator
        .action_choices(CoordinatorActionKind::Constraint(ConstraintIntent::Tangent))
        .into_iter()
        .filter_map(|choice| match choice {
            ActionChoice::Contact {
                default_parameter, ..
            } => Some(default_parameter),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(parameters, [0.25, 0.75]);
}

#[test]
fn contextual_intents_publish_the_exact_resolved_definition_family() {
    let fixture = matrix_fixture();
    let mut coordinator = coordinator(fixture.document);

    let cases = [
        (
            vec![
                SelectionItem::Point(fixture.points[0]),
                SelectionItem::Point(fixture.points[1]),
            ],
            ConstraintIntent::Coincident,
            ResolvedConstraintKind::CoincidentPoints,
        ),
        (
            vec![
                SelectionItem::Point(fixture.midpoint),
                SelectionItem::Curve(fixture.lines[0]),
            ],
            ConstraintIntent::Coincident,
            ResolvedConstraintKind::PointOnCurve,
        ),
        (
            fixture.beziers.map(SelectionItem::Curve).to_vec(),
            ConstraintIntent::Coincident,
            ResolvedConstraintKind::CurveContact,
        ),
        (
            fixture.lines.map(SelectionItem::Curve).to_vec(),
            ConstraintIntent::Equal,
            ResolvedConstraintKind::EqualLength,
        ),
        (
            fixture.circles.map(SelectionItem::Curve).to_vec(),
            ConstraintIntent::Equal,
            ResolvedConstraintKind::EqualRadius,
        ),
        (
            fixture.beziers.map(SelectionItem::Curve).to_vec(),
            ConstraintIntent::Equal,
            ResolvedConstraintKind::EqualCurvature,
        ),
        (
            vec![
                SelectionItem::Curve(fixture.lines[0]),
                SelectionItem::Curve(fixture.circles[0]),
            ],
            ConstraintIntent::Perpendicular,
            ResolvedConstraintKind::RadialLine,
        ),
        (
            fixture.beziers.map(SelectionItem::Curve).to_vec(),
            ConstraintIntent::Tangent,
            ResolvedConstraintKind::CurveTangency,
        ),
        (
            fixture.beziers.map(SelectionItem::Curve).to_vec(),
            ConstraintIntent::Continuity,
            ResolvedConstraintKind::EndpointContinuity,
        ),
    ];
    for (selection, intent, expected) in cases {
        coordinator.editor_mut().set_selection(selection);
        assert_eq!(coordinator.resolved_constraint(intent), Some(expected));
    }
    coordinator.editor_mut().set_selection([
        SelectionItem::Curve(fixture.lines[0]),
        SelectionItem::Curve(fixture.beziers[0]),
    ]);
    assert_eq!(
        coordinator.resolved_constraint(ConstraintIntent::Parallel),
        None
    );
    assert_eq!(
        coordinator.resolved_constraint(ConstraintIntent::Perpendicular),
        None
    );
}

#[test]
fn advanced_contextual_branches_lower_to_existing_persistent_definitions() {
    let fixture = matrix_fixture();
    let contact = |span, parameter, neighborhood| ContactActionChoice {
        support: DocumentCurveSpanRef { span, winding: 0 },
        domain: ContactDomain::Bounded {
            lower: 0.0,
            upper: 1.0,
        },
        parameter,
        neighborhood,
        tangent_orientation: None,
    };

    let mut radial_document = fixture.document.clone();
    let radial_start = radial_document
        .add_point("radial start", [-3.0, 1.0])
        .unwrap();
    let radial_end = radial_document
        .add_point("radial end", [-3.0, 5.0])
        .unwrap();
    let radial_line = CurveSpan::line(
        radial_document
            .add_curve(
                "radial line",
                CurveDefinition::Line {
                    start: radial_start,
                    end: radial_end,
                    branch_direction: [0.0, 1.0],
                },
            )
            .unwrap(),
    );
    let mut radial = coordinator(radial_document);
    radial.editor_mut().set_selection([
        SelectionItem::Curve(radial_line),
        SelectionItem::Curve(fixture.circles[0]),
    ]);
    let outcome = radial
        .apply_constraint_action(
            radial.session().design_identity(),
            ConstraintActionRequest {
                intent: ConstraintIntent::Perpendicular,
                label: "normal to circle".into(),
                contacts: vec![ContactActionChoice {
                    support: DocumentCurveSpanRef {
                        span: radial_line,
                        winding: 0,
                    },
                    domain: ContactDomain::SupportingLine,
                    parameter: 0.5,
                    neighborhood: ContactNeighborhood::Interior,
                    tangent_orientation: None,
                }],
                relation: None,
            },
        )
        .unwrap();
    assert!(outcome.published_accepted.is_some());
    assert!(matches!(
        radial
            .session()
            .design_document()
            .constraint(outcome.value)
            .unwrap()
            .definition,
        DocumentConstraintDefinition::PointOnCurve {
            point,
            ..
        } if point == fixture.points[4]
    ));

    let mut curvature = coordinator(fixture.document.clone());
    curvature
        .editor_mut()
        .set_selection(fixture.beziers.map(SelectionItem::Curve));
    let outcome = curvature
        .apply_constraint_action(
            curvature.session().design_identity(),
            ConstraintActionRequest {
                intent: ConstraintIntent::Equal,
                label: "curvature".into(),
                contacts: fixture
                    .beziers
                    .map(|span| contact(span, 0.5, ContactNeighborhood::Interior))
                    .to_vec(),
                relation: Some(ConstraintRelationChoice::EqualCurvature(
                    DocumentCurveCurvatureRelation::MagnitudeOppositeSign,
                )),
            },
        )
        .unwrap();
    assert!(matches!(
        curvature
            .session()
            .design_document()
            .constraint(outcome.value)
            .unwrap()
            .definition,
        DocumentConstraintDefinition::EqualCurvature {
            relation: DocumentCurveCurvatureRelation::MagnitudeOppositeSign,
            ..
        }
    ));

    let mut continuity = coordinator(fixture.document);
    continuity
        .editor_mut()
        .set_selection(fixture.beziers.map(SelectionItem::Curve));
    let outcome = continuity
        .apply_constraint_action(
            continuity.session().design_identity(),
            ConstraintActionRequest {
                intent: ConstraintIntent::Continuity,
                label: "continuity".into(),
                contacts: vec![
                    contact(fixture.beziers[0], 1.0, ContactNeighborhood::End),
                    contact(fixture.beziers[1], 0.0, ContactNeighborhood::Start),
                ],
                relation: Some(ConstraintRelationChoice::Continuity(
                    DocumentCurveContinuity::G1,
                )),
            },
        )
        .unwrap();
    assert!(matches!(
        continuity
            .session()
            .design_document()
            .constraint(outcome.value)
            .unwrap()
            .definition,
        DocumentConstraintDefinition::EndpointContinuity {
            continuity: DocumentCurveContinuity::G1,
            ..
        }
    ));
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
            ConstraintIntent::Lock,
            vec![SelectionItem::Point(fixture.points[0])],
            Vec::new(),
        ),
        (
            ConstraintIntent::Coincident,
            vec![
                SelectionItem::Point(fixture.points[0]),
                SelectionItem::Point(fixture.points[1]),
            ],
            Vec::new(),
        ),
        (
            ConstraintIntent::Horizontal,
            vec![SelectionItem::Curve(fixture.lines[0])],
            Vec::new(),
        ),
        (
            ConstraintIntent::Vertical,
            vec![SelectionItem::Curve(fixture.lines[1])],
            Vec::new(),
        ),
        (
            ConstraintIntent::Coincident,
            vec![
                SelectionItem::Point(fixture.midpoint),
                SelectionItem::Curve(fixture.lines[0]),
            ],
            vec![contact(fixture.lines[0], None)],
        ),
        (
            ConstraintIntent::Parallel,
            vec![
                SelectionItem::Curve(fixture.lines[0]),
                SelectionItem::Curve(fixture.overlapping_line),
            ],
            Vec::new(),
        ),
        (
            ConstraintIntent::Perpendicular,
            fixture.lines.map(SelectionItem::Curve).to_vec(),
            Vec::new(),
        ),
        (
            ConstraintIntent::Equal,
            fixture.lines.map(SelectionItem::Curve).to_vec(),
            Vec::new(),
        ),
        (
            ConstraintIntent::Equal,
            fixture.circles.map(SelectionItem::Curve).to_vec(),
            Vec::new(),
        ),
        (
            ConstraintIntent::Midpoint,
            vec![
                SelectionItem::Point(fixture.midpoint),
                SelectionItem::Curve(fixture.lines[0]),
            ],
            Vec::new(),
        ),
        (
            ConstraintIntent::Symmetric,
            vec![
                SelectionItem::Point(fixture.points[4]),
                SelectionItem::Point(fixture.points[5]),
                SelectionItem::Curve(fixture.lines[1]),
            ],
            Vec::new(),
        ),
        (
            ConstraintIntent::Coincident,
            fixture.lines.map(SelectionItem::Curve).to_vec(),
            fixture.lines.map(|span| contact(span, None)).to_vec(),
        ),
        (
            ConstraintIntent::Tangent,
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
    for (intent, selection, contacts) in cases {
        let mut coordinator = coordinator(fixture.document.clone());
        coordinator.editor_mut().set_selection(selection);
        let expected = coordinator.session().design_identity();
        let outcome = coordinator
            .apply_constraint_action(
                expected,
                ConstraintActionRequest {
                    intent,
                    label: format!("{intent:?}"),
                    contacts,
                    relation: None,
                },
            )
            .unwrap_or_else(|error| panic!("{intent:?} failed: {error}"));
        assert!(
            outcome.published_accepted.is_some(),
            "{intent:?} produced a rejected attempt"
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
fn every_resolved_relation_executes_through_the_authoring_adapter() {
    let mut fixture = matrix_fixture();
    let radial_start = fixture
        .document
        .add_point("radial start", [-4.0, 3.0])
        .unwrap();
    let radial_end = fixture
        .document
        .add_point("radial end", [-2.0, 3.0])
        .unwrap();
    let radial_line = CurveSpan::line(
        fixture
            .document
            .add_curve(
                "radial line",
                CurveDefinition::Line {
                    start: radial_start,
                    end: radial_end,
                    branch_direction: [1.0, 0.0],
                },
            )
            .unwrap(),
    );
    let selected = |item| AuthoringOperand::selected(item);
    let picked =
        |span, parameter| AuthoringOperand::picked(SelectionItem::Curve(span), Some(parameter));
    let cases = [
        (
            ConstraintIntent::Lock,
            ResolvedConstraintKind::FixedPoint,
            vec![selected(SelectionItem::Point(fixture.points[0]))],
        ),
        (
            ConstraintIntent::Coincident,
            ResolvedConstraintKind::CoincidentPoints,
            fixture.points[0..2]
                .iter()
                .copied()
                .map(SelectionItem::Point)
                .map(selected)
                .collect(),
        ),
        (
            ConstraintIntent::Coincident,
            ResolvedConstraintKind::PointOnCurve,
            vec![
                selected(SelectionItem::Point(fixture.midpoint)),
                picked(fixture.lines[0], 0.5),
            ],
        ),
        (
            ConstraintIntent::Coincident,
            ResolvedConstraintKind::CurveContact,
            fixture.lines.map(|span| picked(span, 0.5)).to_vec(),
        ),
        (
            ConstraintIntent::Horizontal,
            ResolvedConstraintKind::HorizontalLine,
            vec![selected(SelectionItem::Curve(fixture.lines[0]))],
        ),
        (
            ConstraintIntent::Vertical,
            ResolvedConstraintKind::VerticalLine,
            vec![selected(SelectionItem::Curve(fixture.lines[1]))],
        ),
        (
            ConstraintIntent::Horizontal,
            ResolvedConstraintKind::HorizontalPoints,
            fixture.points[0..2]
                .iter()
                .copied()
                .map(SelectionItem::Point)
                .map(selected)
                .collect(),
        ),
        (
            ConstraintIntent::Vertical,
            ResolvedConstraintKind::VerticalPoints,
            fixture.points[2..4]
                .iter()
                .copied()
                .map(SelectionItem::Point)
                .map(selected)
                .collect(),
        ),
        (
            ConstraintIntent::Concentric,
            ResolvedConstraintKind::ConcentricCurves,
            fixture
                .circles
                .map(SelectionItem::Curve)
                .map(selected)
                .to_vec(),
        ),
        (
            ConstraintIntent::Collinear,
            ResolvedConstraintKind::CollinearSupports,
            fixture
                .lines
                .map(SelectionItem::Curve)
                .map(selected)
                .to_vec(),
        ),
        (
            ConstraintIntent::Parallel,
            ResolvedConstraintKind::ParallelLines,
            vec![
                selected(SelectionItem::Curve(fixture.lines[0])),
                selected(SelectionItem::Curve(fixture.overlapping_line)),
            ],
        ),
        (
            ConstraintIntent::Perpendicular,
            ResolvedConstraintKind::PerpendicularLines,
            fixture
                .lines
                .map(SelectionItem::Curve)
                .map(selected)
                .to_vec(),
        ),
        (
            ConstraintIntent::Perpendicular,
            ResolvedConstraintKind::RadialLine,
            vec![picked(fixture.circles[0], 0.0), picked(radial_line, 0.5)],
        ),
        (
            ConstraintIntent::Equal,
            ResolvedConstraintKind::EqualLength,
            fixture
                .lines
                .map(SelectionItem::Curve)
                .map(selected)
                .to_vec(),
        ),
        (
            ConstraintIntent::Equal,
            ResolvedConstraintKind::EqualRadius,
            fixture
                .circles
                .map(SelectionItem::Curve)
                .map(selected)
                .to_vec(),
        ),
        (
            ConstraintIntent::Equal,
            ResolvedConstraintKind::EqualCurvature,
            fixture.beziers.map(|span| picked(span, 0.5)).to_vec(),
        ),
        (
            ConstraintIntent::Midpoint,
            ResolvedConstraintKind::Midpoint,
            vec![
                selected(SelectionItem::Point(fixture.midpoint)),
                selected(SelectionItem::Curve(fixture.lines[0])),
            ],
        ),
        (
            ConstraintIntent::Symmetric,
            ResolvedConstraintKind::SymmetricAboutLine,
            vec![
                selected(SelectionItem::Point(fixture.points[4])),
                selected(SelectionItem::Point(fixture.points[5])),
                selected(SelectionItem::Curve(fixture.lines[1])),
            ],
        ),
        (
            ConstraintIntent::Tangent,
            ResolvedConstraintKind::CurveTangency,
            vec![
                picked(fixture.lines[0], 0.5),
                picked(fixture.overlapping_line, 0.5),
            ],
        ),
        (
            ConstraintIntent::Continuity,
            ResolvedConstraintKind::EndpointContinuity,
            vec![
                picked(fixture.beziers[0], 1.0),
                picked(fixture.beziers[1], 0.0),
            ],
        ),
    ];
    assert_eq!(cases.len(), 20);
    for (intent, expected_resolution, operands) in cases {
        let mut coordinator = coordinator(fixture.document.clone());
        let options = AuthoringOptions {
            curvature_relation: DocumentCurveCurvatureRelation::MagnitudeOppositeSign,
            ..AuthoringOptions::default()
        };
        let mut preselected = AuthoringState::default();
        preselected.set_options(options);
        let preselected_application = preselected.activate(
            coordinator.session().design_document(),
            AuthoringTool::Constraint(intent),
            &operands,
        );
        let AuthoringOutcome::Apply(preselected_application) = preselected_application else {
            panic!("{expected_resolution:?} did not produce an application");
        };
        assert_eq!(preselected.active_tool(), None);
        assert!(preselected.pending().is_empty());

        let mut repeated = AuthoringState::default();
        repeated.set_options(options);
        assert!(matches!(
            repeated.activate(
                coordinator.session().design_document(),
                AuthoringTool::Constraint(intent),
                &[],
            ),
            AuthoringOutcome::ModeEntered { .. }
        ));
        let mut repeated_application = None;
        for (index, operand) in operands.iter().copied().enumerate() {
            let outcome = repeated.pick(coordinator.session().design_document(), operand);
            if index + 1 == operands.len() {
                let AuthoringOutcome::Apply(application) = outcome else {
                    panic!("{expected_resolution:?} did not complete in repeated mode");
                };
                repeated_application = Some(application);
            } else {
                assert!(
                    matches!(outcome, AuthoringOutcome::Collecting { .. }),
                    "{expected_resolution:?} did not retain its valid pending prefix"
                );
            }
        }
        let application = repeated_application.expect("complete repeated application");
        assert_eq!(application, preselected_application);
        assert_eq!(
            application.resolved_constraint,
            Some(expected_resolution),
            "{intent:?} resolution"
        );
        let AuthoringMutation::Constraint(outcome) = coordinator
            .apply_authoring(coordinator.session().design_identity(), &application)
            .unwrap_or_else(|error| panic!("{expected_resolution:?}: {error}"))
        else {
            panic!("{expected_resolution:?} did not create a constraint");
        };
        assert!(
            outcome.published_accepted.is_some(),
            "{expected_resolution:?} retained a rejected attempt"
        );
        let accepted = coordinator
            .session()
            .accepted_state()
            .expect("accepted authoring result")
            .document();
        let constraint = accepted
            .constraint(outcome.value)
            .unwrap_or_else(|| panic!("{expected_resolution:?} did not persist its constraint"));
        let assert_contact = |contact,
                              expected_curve,
                              expected_domain,
                              expected_parameter: f64,
                              expected_neighborhood,
                              expected_orientation| {
            let contact = accepted.contact(contact).expect("persistent contact");
            assert_eq!(contact.curve, expected_curve, "{expected_resolution:?}");
            assert_eq!(contact.domain, expected_domain, "{expected_resolution:?}");
            assert_eq!(contact.winding, 0, "{expected_resolution:?}");
            assert_eq!(
                contact.neighborhood, expected_neighborhood,
                "{expected_resolution:?}"
            );
            assert_eq!(
                contact.tangent_orientation, expected_orientation,
                "{expected_resolution:?}"
            );
            assert_eq!(
                accepted
                    .scalar(contact.parameter)
                    .expect("contact parameter")
                    .value
                    .to_bits(),
                expected_parameter.to_bits(),
                "{expected_resolution:?} parameter"
            );
        };
        let bounded = ContactDomain::Bounded {
            lower: 0.0,
            upper: 1.0,
        };
        match (expected_resolution, &constraint.definition) {
            (
                ResolvedConstraintKind::FixedPoint,
                DocumentConstraintDefinition::FixedPoint { point, target },
            ) => {
                assert_eq!(*point, fixture.points[0]);
                assert_eq!(
                    (*target).map(f64::to_bits),
                    [(-2.0_f64).to_bits(), 0.0_f64.to_bits()]
                );
            }
            (
                ResolvedConstraintKind::CoincidentPoints,
                DocumentConstraintDefinition::Coincident { first, second },
            ) => {
                assert_eq!([*first, *second], fixture.points[0..2]);
            }
            (
                ResolvedConstraintKind::PointOnCurve,
                DocumentConstraintDefinition::PointOnCurve { point, contact },
            ) => {
                assert_eq!(*point, fixture.midpoint);
                assert_contact(
                    *contact,
                    fixture.lines[0],
                    bounded,
                    0.5,
                    ContactNeighborhood::Interior,
                    None,
                );
            }
            (
                ResolvedConstraintKind::CurveContact,
                DocumentConstraintDefinition::CurveCurveContact {
                    first_contact,
                    second_contact,
                },
            ) => {
                assert_contact(
                    *first_contact,
                    fixture.lines[0],
                    bounded,
                    0.5,
                    ContactNeighborhood::Interior,
                    None,
                );
                assert_contact(
                    *second_contact,
                    fixture.lines[1],
                    bounded,
                    0.5,
                    ContactNeighborhood::Interior,
                    None,
                );
            }
            (
                ResolvedConstraintKind::HorizontalLine,
                DocumentConstraintDefinition::Horizontal { line },
            ) => assert_eq!(*line, fixture.lines[0]),
            (
                ResolvedConstraintKind::VerticalLine,
                DocumentConstraintDefinition::Vertical { line },
            ) => assert_eq!(*line, fixture.lines[1]),
            (
                ResolvedConstraintKind::HorizontalPoints,
                DocumentConstraintDefinition::HorizontalPoints { first, second },
            ) => assert_eq!([*first, *second], fixture.points[0..2]),
            (
                ResolvedConstraintKind::VerticalPoints,
                DocumentConstraintDefinition::VerticalPoints { first, second },
            ) => assert_eq!([*first, *second], fixture.points[2..4]),
            (
                ResolvedConstraintKind::ConcentricCurves,
                DocumentConstraintDefinition::Concentric { first, second },
            ) => assert_eq!(
                [first.curve, second.curve],
                [fixture.circles[0].curve, fixture.circles[1].curve]
            ),
            (
                ResolvedConstraintKind::CollinearSupports,
                DocumentConstraintDefinition::Collinear { first, second },
            ) => {
                assert_eq!([first.span, second.span], fixture.lines);
                assert_eq!(
                    [first.direction, second.direction],
                    [geosolve_sketch::DocumentDirectionSense::Forward; 2]
                );
            }
            (
                ResolvedConstraintKind::ParallelLines,
                DocumentConstraintDefinition::Parallel { first, second },
            ) => assert_eq!(
                [*first, *second],
                [fixture.lines[0], fixture.overlapping_line]
            ),
            (
                ResolvedConstraintKind::PerpendicularLines,
                DocumentConstraintDefinition::Perpendicular { first, second },
            )
            | (
                ResolvedConstraintKind::EqualLength,
                DocumentConstraintDefinition::EqualLength { first, second },
            ) => assert_eq!([*first, *second], fixture.lines),
            (
                ResolvedConstraintKind::RadialLine,
                DocumentConstraintDefinition::PointOnCurve { point, contact },
            ) => {
                assert_eq!(*point, fixture.points[4]);
                assert_contact(
                    *contact,
                    radial_line,
                    ContactDomain::SupportingLine,
                    0.5,
                    ContactNeighborhood::Interior,
                    None,
                );
            }
            (
                ResolvedConstraintKind::EqualRadius,
                DocumentConstraintDefinition::EqualRadius { first, second },
            ) => assert_eq!(
                [*first, *second],
                [fixture.circles[0].curve, fixture.circles[1].curve]
            ),
            (
                ResolvedConstraintKind::EqualCurvature,
                DocumentConstraintDefinition::EqualCurvature {
                    first_contact,
                    second_contact,
                    relation,
                },
            ) => {
                assert_eq!(
                    *relation,
                    DocumentCurveCurvatureRelation::MagnitudeOppositeSign
                );
                assert_contact(
                    *first_contact,
                    fixture.beziers[0],
                    bounded,
                    0.5,
                    ContactNeighborhood::Interior,
                    None,
                );
                assert_contact(
                    *second_contact,
                    fixture.beziers[1],
                    bounded,
                    0.5,
                    ContactNeighborhood::Interior,
                    None,
                );
            }
            (
                ResolvedConstraintKind::Midpoint,
                DocumentConstraintDefinition::Midpoint { point, line },
            ) => {
                assert_eq!(*point, fixture.midpoint);
                assert_eq!(*line, fixture.lines[0]);
            }
            (
                ResolvedConstraintKind::SymmetricAboutLine,
                DocumentConstraintDefinition::SymmetricAboutLine {
                    first,
                    second,
                    line,
                },
            ) => {
                assert_eq!([*first, *second], fixture.points[4..6]);
                assert_eq!(*line, fixture.lines[1]);
            }
            (
                ResolvedConstraintKind::CurveTangency,
                DocumentConstraintDefinition::CurveCurveTangency {
                    first_contact,
                    second_contact,
                },
            ) => {
                assert_contact(
                    *first_contact,
                    fixture.lines[0],
                    bounded,
                    0.5,
                    ContactNeighborhood::Interior,
                    Some(TangentOrientation::Aligned),
                );
                assert_contact(
                    *second_contact,
                    fixture.overlapping_line,
                    bounded,
                    0.5,
                    ContactNeighborhood::Interior,
                    Some(TangentOrientation::Aligned),
                );
            }
            (
                ResolvedConstraintKind::EndpointContinuity,
                DocumentConstraintDefinition::EndpointContinuity {
                    first_contact,
                    second_contact,
                    continuity,
                },
            ) => {
                assert_eq!(*continuity, DocumentCurveContinuity::G1);
                assert_contact(
                    *first_contact,
                    fixture.beziers[0],
                    bounded,
                    1.0,
                    ContactNeighborhood::End,
                    None,
                );
                assert_contact(
                    *second_contact,
                    fixture.beziers[1],
                    bounded,
                    0.0,
                    ContactNeighborhood::Start,
                    None,
                );
            }
            (_, definition) => {
                panic!("{expected_resolution:?} persisted unexpected definition {definition:?}")
            }
        }
        repeated.transaction_finished();
        assert!(repeated.pending().is_empty());
        assert_eq!(
            repeated.active_tool(),
            Some(AuthoringTool::Constraint(intent))
        );
    }
}

#[test]
fn every_dimension_executes_through_the_authoring_adapter() {
    let fixture = matrix_fixture();
    let selected = |item| AuthoringOperand::selected(item);
    let cases = [
        (
            DimensionKind::PointDistance,
            fixture.points[0..2]
                .iter()
                .copied()
                .map(SelectionItem::Point)
                .map(selected)
                .collect(),
        ),
        (
            DimensionKind::SegmentLength,
            vec![selected(SelectionItem::Curve(fixture.lines[0]))],
        ),
        (
            DimensionKind::Radius,
            vec![selected(SelectionItem::Curve(fixture.circles[0]))],
        ),
        (
            DimensionKind::Diameter,
            vec![selected(SelectionItem::Curve(fixture.circles[0]))],
        ),
        (
            DimensionKind::OrientedAngle,
            fixture
                .lines
                .map(SelectionItem::Curve)
                .map(selected)
                .to_vec(),
        ),
    ];
    assert_eq!(cases.len(), 5);
    for (kind, operands) in cases {
        let mut coordinator = coordinator(fixture.document.clone());
        let mut preselected = AuthoringState::default();
        let preselected_application = preselected.activate(
            coordinator.session().design_document(),
            AuthoringTool::Dimension(kind),
            &operands,
        );
        let AuthoringOutcome::Apply(preselected_application) = preselected_application else {
            panic!("{kind:?} did not produce an application");
        };

        let mut repeated = AuthoringState::default();
        let _ = repeated.activate(
            coordinator.session().design_document(),
            AuthoringTool::Dimension(kind),
            &[],
        );
        let mut repeated_application = None;
        for (index, operand) in operands.iter().copied().enumerate() {
            let outcome = repeated.pick(coordinator.session().design_document(), operand);
            if index + 1 == operands.len() {
                let AuthoringOutcome::Apply(application) = outcome else {
                    panic!("{kind:?} did not complete in repeated mode");
                };
                repeated_application = Some(application);
            } else {
                assert!(
                    matches!(outcome, AuthoringOutcome::Collecting { .. }),
                    "{kind:?} did not retain its valid pending prefix"
                );
            }
        }
        let application = repeated_application.expect("complete repeated application");
        assert_eq!(application, preselected_application);
        assert_eq!(application.resolved_constraint, None);
        let AuthoringMutation::Dimension(outcome) = coordinator
            .apply_authoring(coordinator.session().design_identity(), &application)
            .unwrap_or_else(|error| panic!("{kind:?}: {error}"))
        else {
            panic!("{kind:?} did not create a dimension");
        };
        assert!(
            outcome.published_accepted.is_some(),
            "{kind:?} retained a rejected attempt"
        );
        assert!(
            coordinator
                .session()
                .design_document()
                .dimension(outcome.value)
                .is_some(),
            "{kind:?} did not persist its dimension"
        );
        repeated.transaction_finished();
        assert!(repeated.pending().is_empty());
        assert_eq!(repeated.active_tool(), Some(AuthoringTool::Dimension(kind)));
    }
}

#[test]
fn point_on_curve_authoring_preserves_picks_across_representative_curve_families() {
    let mut document = SketchDocument::new(2.0).unwrap();
    let contact_point = document.add_point("contact", [1.0, 0.0]).unwrap();
    let start = document.add_point("start", [0.0, 0.0]).unwrap();
    let middle = document.add_point("middle", [1.0, 0.0]).unwrap();
    let end = document.add_point("end", [2.0, 0.0]).unwrap();
    let center = document.add_point("center", [0.0, 0.0]).unwrap();
    let radius = document
        .add_scalar("radius", 1.0, ScalarUnit::Length, ScalarDomain::Positive)
        .unwrap();
    let line = CurveSpan::line(
        document
            .add_curve(
                "line",
                CurveDefinition::Line {
                    start,
                    end,
                    branch_direction: [1.0, 0.0],
                },
            )
            .unwrap(),
    );
    let circle = CurveSpan::line(
        document
            .add_curve("circle", CurveDefinition::Circle { center, radius })
            .unwrap(),
    );
    let bezier = CurveSpan::line(
        document
            .add_curve(
                "bezier",
                CurveDefinition::QuadraticBezier {
                    controls: [start, middle, end],
                },
            )
            .unwrap(),
    );
    let weights = [0, 1].map(|index| {
        document
            .add_scalar(
                format!("weight {index}"),
                1.0,
                ScalarUnit::Parameter,
                ScalarDomain::Positive,
            )
            .unwrap()
    });
    let nurbs = CurveSpan {
        curve: document
            .add_curve(
                "NURBS",
                CurveDefinition::Nurbs {
                    form: DocumentBSplineForm::Clamped,
                    degree: 1,
                    controls: vec![start, end],
                    weights: weights.to_vec(),
                    gauge_weight: weights[0],
                    knots: vec![0.0, 0.0, 1.0, 1.0],
                    span_ids: vec![7],
                    next_span_id: 8,
                },
            )
            .unwrap(),
        segment: 7,
    };
    for (label, span, parameter) in [
        ("line", line, 0.5),
        ("circle", circle, 0.0),
        ("Bezier", bezier, 0.5),
        ("NURBS", nurbs, 0.5),
    ] {
        let mut coordinator = coordinator(document.clone());
        let application = AuthoringState::default().activate(
            coordinator.session().design_document(),
            AuthoringTool::Constraint(ConstraintIntent::Coincident),
            &[
                AuthoringOperand::selected(SelectionItem::Point(contact_point)),
                AuthoringOperand::picked(SelectionItem::Curve(span), Some(parameter)),
            ],
        );
        let AuthoringOutcome::Apply(application) = application else {
            panic!("{label} point-on-curve did not produce an application");
        };
        assert_eq!(
            application.resolved_constraint,
            Some(ResolvedConstraintKind::PointOnCurve)
        );
        let AuthoringMutation::Constraint(outcome) = coordinator
            .apply_authoring(coordinator.session().design_identity(), &application)
            .unwrap_or_else(|error| panic!("{label}: {error}"))
        else {
            panic!("{label} did not create a constraint");
        };
        assert!(
            outcome.published_accepted.is_some(),
            "{label} point-on-curve rejected"
        );
        let accepted = coordinator.session().accepted_state().unwrap().document();
        let DocumentConstraintDefinition::PointOnCurve { contact, .. } =
            accepted.constraint(outcome.value).unwrap().definition
        else {
            panic!("{label} did not persist point-on-curve");
        };
        let contact = accepted.contact(contact).unwrap();
        assert_eq!(contact.curve, span);
        assert_eq!(
            accepted.scalar(contact.parameter).unwrap().value.to_bits(),
            parameter.to_bits(),
            "{label} picked parameter"
        );
    }
}

#[test]
fn endpoint_continuity_matches_start_and_end_neighborhoods_in_both_orders() {
    let fixture = matrix_fixture();
    for (first, second, expected_neighborhoods) in [
        (
            (fixture.beziers[0], 1.0),
            (fixture.beziers[1], 0.0),
            [ContactNeighborhood::End, ContactNeighborhood::Start],
        ),
        (
            (fixture.beziers[0], 0.0),
            (fixture.beziers[1], 1.0),
            [ContactNeighborhood::Start, ContactNeighborhood::End],
        ),
    ] {
        let mut coordinator = coordinator(fixture.document.clone());
        let mut authoring = AuthoringState::default();
        authoring.set_options(AuthoringOptions {
            continuity: DocumentCurveContinuity::G0,
            ..AuthoringOptions::default()
        });
        let application = authoring.activate(
            coordinator.session().design_document(),
            AuthoringTool::Constraint(ConstraintIntent::Continuity),
            &[
                AuthoringOperand::picked(SelectionItem::Curve(first.0), Some(first.1)),
                AuthoringOperand::picked(SelectionItem::Curve(second.0), Some(second.1)),
            ],
        );
        let AuthoringOutcome::Apply(application) = application else {
            panic!("endpoint continuity did not produce an application");
        };
        let AuthoringMutation::Constraint(outcome) = coordinator
            .apply_authoring(coordinator.session().design_identity(), &application)
            .unwrap()
        else {
            panic!("continuity did not create a constraint");
        };
        assert!(outcome.published_accepted.is_some());
        let accepted = coordinator.session().accepted_state().unwrap().document();
        let DocumentConstraintDefinition::EndpointContinuity {
            first_contact,
            second_contact,
            ..
        } = accepted.constraint(outcome.value).unwrap().definition
        else {
            panic!("endpoint continuity definition expected");
        };
        for (contact, expected_parameter, expected_neighborhood) in [
            (first_contact, first.1, expected_neighborhoods[0]),
            (second_contact, second.1, expected_neighborhoods[1]),
        ] {
            let contact = accepted.contact(contact).unwrap();
            assert_eq!(contact.neighborhood, expected_neighborhood);
            assert_eq!(
                accepted.scalar(contact.parameter).unwrap().value.to_bits(),
                expected_parameter.to_bits()
            );
        }
    }
}

#[test]
fn authored_line_endpoint_can_use_default_contextual_contact_on_circle() {
    let fixture = matrix_fixture();
    let mut coordinator = coordinator(fixture.document);
    coordinator.editor_mut().set_selection([
        SelectionItem::Point(fixture.points[0]),
        SelectionItem::Curve(fixture.circles[0]),
    ]);
    let action = CoordinatorActionKind::Constraint(ConstraintIntent::Coincident);
    assert_enabled(&coordinator, action);
    assert_eq!(
        coordinator.resolved_constraint(ConstraintIntent::Coincident),
        Some(ResolvedConstraintKind::PointOnCurve)
    );
    let choices = coordinator.action_choices(action);
    let [
        ActionChoice::Contact {
            span,
            domains,
            default_parameter,
            neighborhoods,
            tangent_orientations,
            default_winding,
            ..
        },
    ] = choices.as_slice()
    else {
        panic!("point-on-circle must expose one contact choice");
    };
    assert!(tangent_orientations.is_empty());
    let contact = ContactActionChoice {
        support: DocumentCurveSpanRef {
            span: *span,
            winding: *default_winding,
        },
        domain: domains[0],
        parameter: *default_parameter,
        neighborhood: neighborhoods[0],
        tangent_orientation: None,
    };
    let expected = coordinator.session().design_identity();
    let outcome = coordinator
        .apply_constraint_action(
            expected,
            ConstraintActionRequest {
                intent: ConstraintIntent::Coincident,
                label: "point on circle".into(),
                contacts: vec![contact],
                relation: None,
            },
        )
        .unwrap();
    assert!(outcome.published_accepted.is_some());
    assert!(matches!(
        coordinator
            .session()
            .design_document()
            .constraint(outcome.value)
            .unwrap()
            .definition,
        DocumentConstraintDefinition::PointOnCurve { .. }
    ));
}

#[test]
fn line_circle_tangent_and_normal_intents_enforce_true_incidence() {
    let fixture = matrix_fixture();
    let mut tangent_document = fixture.document.clone();
    let tangent_start = tangent_document
        .add_point("tangent start", [-5.0, 4.0])
        .unwrap();
    let tangent_end = tangent_document
        .add_point("tangent end", [-1.0, 4.0])
        .unwrap();
    let tangent_line = CurveSpan::line(
        tangent_document
            .add_curve(
                "circle tangent",
                CurveDefinition::Line {
                    start: tangent_start,
                    end: tangent_end,
                    branch_direction: [1.0, 0.0],
                },
            )
            .unwrap(),
    );
    let line_contact = ContactActionChoice {
        support: DocumentCurveSpanRef {
            span: tangent_line,
            winding: 0,
        },
        domain: ContactDomain::SupportingLine,
        parameter: 0.5,
        neighborhood: ContactNeighborhood::Interior,
        tangent_orientation: Some(TangentOrientation::Opposed),
    };
    let circle_contact = ContactActionChoice {
        support: DocumentCurveSpanRef {
            span: fixture.circles[0],
            winding: 0,
        },
        domain: ContactDomain::Periodic {
            period: std::f64::consts::TAU,
        },
        parameter: std::f64::consts::FRAC_PI_2,
        neighborhood: ContactNeighborhood::Interior,
        tangent_orientation: Some(TangentOrientation::Opposed),
    };
    let mut tangent = coordinator(tangent_document);
    tangent.editor_mut().set_selection([
        SelectionItem::Curve(tangent_line),
        SelectionItem::Curve(fixture.circles[0]),
    ]);
    assert_eq!(
        tangent.resolved_constraint(ConstraintIntent::Tangent),
        Some(ResolvedConstraintKind::CurveTangency)
    );
    let tangent_outcome = tangent
        .apply_constraint_action(
            tangent.session().design_identity(),
            ConstraintActionRequest {
                intent: ConstraintIntent::Tangent,
                label: "line tangent to circle".into(),
                contacts: vec![line_contact, circle_contact],
                relation: None,
            },
        )
        .unwrap();
    assert!(tangent_outcome.published_accepted.is_some());
    let tangent_document = tangent.session().accepted_state().unwrap().document();
    let DocumentConstraintDefinition::CurveCurveTangency {
        first_contact,
        second_contact,
    } = tangent_document
        .constraint(tangent_outcome.value)
        .unwrap()
        .definition
    else {
        panic!("line/circle Tangent must persist true curve tangency");
    };
    let contact_jet = |contact| {
        let contact = tangent_document.contact(contact).unwrap();
        tangent_document
            .evaluate_curve_jet(
                contact.curve,
                tangent_document.scalar(contact.parameter).unwrap().value,
            )
            .unwrap()
    };
    let first = contact_jet(first_contact);
    let second = contact_jet(second_contact);
    assert!((first.position[0] - second.position[0]).abs() < 1.0e-8);
    assert!((first.position[1] - second.position[1]).abs() < 1.0e-8);
    let cross = first.first_derivative[0] * second.first_derivative[1]
        - first.first_derivative[1] * second.first_derivative[0];
    assert!(cross.abs() < 1.0e-8);

    let mut normal_document = fixture.document;
    let normal_start = normal_document
        .add_point("normal start", [-3.0, 1.0])
        .unwrap();
    let normal_end = normal_document
        .add_point("normal end", [-3.0, 5.0])
        .unwrap();
    let normal_line = CurveSpan::line(
        normal_document
            .add_curve(
                "circle normal",
                CurveDefinition::Line {
                    start: normal_start,
                    end: normal_end,
                    branch_direction: [0.0, 1.0],
                },
            )
            .unwrap(),
    );
    let mut normal = coordinator(normal_document);
    normal.editor_mut().set_selection([
        SelectionItem::Curve(normal_line),
        SelectionItem::Curve(fixture.circles[0]),
    ]);
    assert_eq!(
        normal.resolved_constraint(ConstraintIntent::Perpendicular),
        Some(ResolvedConstraintKind::RadialLine)
    );
    let radial_contact = ContactActionChoice {
        support: DocumentCurveSpanRef {
            span: normal_line,
            winding: 0,
        },
        domain: ContactDomain::SupportingLine,
        parameter: 0.5,
        neighborhood: ContactNeighborhood::Interior,
        tangent_orientation: None,
    };
    let normal_outcome = normal
        .apply_constraint_action(
            normal.session().design_identity(),
            ConstraintActionRequest {
                intent: ConstraintIntent::Perpendicular,
                label: "line normal to circle".into(),
                contacts: vec![radial_contact],
                relation: None,
            },
        )
        .unwrap();
    assert!(normal_outcome.published_accepted.is_some());
    let normal_document = normal.session().accepted_state().unwrap().document();
    let DocumentConstraintDefinition::PointOnCurve { point, contact } = normal_document
        .constraint(normal_outcome.value)
        .unwrap()
        .definition
    else {
        panic!("circle normal must persist centre-on-line incidence");
    };
    let contact = normal_document.contact(contact).unwrap();
    assert_eq!(contact.curve, normal_line);
    let line_point = normal_document
        .evaluate_curve_jet(
            contact.curve,
            normal_document.scalar(contact.parameter).unwrap().value,
        )
        .unwrap()
        .position;
    let center = normal_document.point(point).unwrap().position;
    assert!((line_point[0] - center[0]).abs() < 1.0e-8);
    assert!((line_point[1] - center[1]).abs() < 1.0e-8);
}

#[test]
fn retained_cursed_contact_payload_rejects_ambiguity_and_projected_retry_recovers() {
    let mut accepted = SketchDocument::new(10.0).unwrap();
    let accepted_positions = [
        [-4.108_016_184_188_926, 1.061_352_761_640_428_4],
        [1.615_725_990_263_041_6, 1.849_911_182_145_521],
        [-2.355_914_250_237_194_4, -0.748_926_683_942_713_8],
        [1.305_812_479_089_582_6, 0.757_494_098_512_779_4],
        [0.016_920_189_296_906_276, -1.335_536_308_188_942_2],
    ];
    let points = accepted_positions.map(|position| {
        accepted
            .add_point("payload point", position)
            .expect("finite payload point")
    });
    let radii = [
        accepted
            .add_scalar(
                "first radius",
                2.519_319_919_802_534_4,
                ScalarUnit::Length,
                ScalarDomain::Positive,
            )
            .unwrap(),
        accepted
            .add_scalar(
                "second radius",
                1.135_526_956_548_755_6,
                ScalarUnit::Length,
                ScalarDomain::Positive,
            )
            .unwrap(),
    ];
    let circles = [
        accepted
            .add_curve(
                "first circle",
                CurveDefinition::Circle {
                    center: points[0],
                    radius: radii[0],
                },
            )
            .unwrap(),
        accepted
            .add_curve(
                "second circle",
                CurveDefinition::Circle {
                    center: points[1],
                    radius: radii[1],
                },
            )
            .unwrap(),
    ];
    let center_line = CurveSpan::line(
        accepted
            .add_curve(
                "center line",
                CurveDefinition::Line {
                    start: points[0],
                    end: points[1],
                    branch_direction: [0.997_067_333_546_924_6, -0.076_529_290_952_066_56],
                },
            )
            .unwrap(),
    );
    let tangent_line = CurveSpan::line(
        accepted
            .add_curve(
                "tangent line",
                CurveDefinition::Line {
                    start: points[2],
                    end: points[3],
                    branch_direction: [0.985_233_792_620_910_7, 0.171_214_409_083_512_13],
                },
            )
            .unwrap(),
    );
    let bezier = CurveSpan::line(
        accepted
            .add_curve(
                "quadratic Bezier",
                CurveDefinition::QuadraticBezier {
                    controls: [points[0], points[4], points[1]],
                },
            )
            .unwrap(),
    );
    let periodic = ContactDomain::Periodic {
        period: std::f64::consts::TAU,
    };
    let point_contacts = [
        accepted
            .add_curve_contact_with_domain(
                "right endpoint on second circle",
                CurveSpan::line(circles[1]),
                periodic,
                4.435_956_993_200_776,
                0,
                ContactNeighborhood::Interior,
                None,
            )
            .unwrap(),
        accepted
            .add_curve_contact_with_domain(
                "left endpoint on first circle",
                CurveSpan::line(circles[0]),
                periodic,
                5.481_457_522_095_779,
                0,
                ContactNeighborhood::Interior,
                None,
            )
            .unwrap(),
    ];
    accepted
        .add_constraint(
            "right endpoint contact",
            DocumentConstraintDefinition::PointOnCurve {
                point: points[3],
                contact: point_contacts[0],
            },
        )
        .unwrap();
    accepted
        .add_constraint(
            "left endpoint contact",
            DocumentConstraintDefinition::PointOnCurve {
                point: points[2],
                contact: point_contacts[1],
            },
        )
        .unwrap();
    let bounded = ContactDomain::Bounded {
        lower: 0.0,
        upper: 1.0,
    };
    let tangent_contacts = [
        accepted
            .add_curve_contact_with_domain(
                "line tangent contact",
                tangent_line,
                bounded,
                0.650_751_748_196_425_1,
                0,
                ContactNeighborhood::Interior,
                Some(TangentOrientation::Aligned),
            )
            .unwrap(),
        accepted
            .add_curve_contact_with_domain(
                "Bezier tangent contact",
                bezier,
                bounded,
                0.618_262_446_398_752_7,
                0,
                ContactNeighborhood::Interior,
                Some(TangentOrientation::Aligned),
            )
            .unwrap(),
    ];
    accepted
        .add_constraint(
            "line Bezier tangency",
            DocumentConstraintDefinition::CurveCurveTangency {
                first_contact: tangent_contacts[0],
                second_contact: tangent_contacts[1],
            },
        )
        .unwrap();
    for (label, curve, value) in [
        ("tangent line length", tangent_line, 3.959_488_125_404_189),
        ("center line length", center_line, 5.777_806_578_660_964_5),
    ] {
        let target = accepted
            .add_scalar(label, value, ScalarUnit::Length, ScalarDomain::Positive)
            .unwrap();
        accepted
            .add_dimension(
                label,
                DocumentDimensionDefinition::CurveLength { curve, target },
                DocumentDimensionMode::Driving,
            )
            .unwrap();
    }

    let mut design = accepted.clone();
    for (point, position) in points.into_iter().zip([
        [-4.911_903_637_222_483, 1.479_014_708_252_145_6],
        [1.624_809_741_247_961, 2.160_902_075_070_983],
        [-1.161_523_119_719_15, -0.837_051_003_183_769_5],
        [1.460_216_238_911_857_8, -0.061_110_206_459_898_49],
        [-0.620_715_897_758_882_5, -0.213_947_030_056_782_53],
    ]) {
        design.set_point_position(point, position).unwrap();
    }
    for (radius, value) in radii
        .into_iter()
        .zip([1.577_323_970_902_071_4, 1.639_371_788_184_938_3])
    {
        design.set_scalar_value(radius, value).unwrap();
    }
    for (contact, curve, value) in [
        (
            point_contacts[0],
            CurveSpan::line(circles[1]),
            4.446_883_202_009_624,
        ),
        (
            point_contacts[1],
            CurveSpan::line(circles[0]),
            5.476_769_923_396_541_5,
        ),
    ] {
        design
            .set_contact_branches(&[ContactBranchEdit {
                contact,
                curve,
                domain: periodic,
                value,
                winding: 0,
                neighborhood: ContactNeighborhood::Interior,
                tangent_orientation: None,
            }])
            .unwrap();
    }
    design
        .set_contact_branches(&[
            ContactBranchEdit {
                contact: tangent_contacts[0],
                curve: tangent_line,
                domain: bounded,
                value: 0.688_047_990_609_921_9,
                winding: 0,
                neighborhood: ContactNeighborhood::Interior,
                tangent_orientation: Some(TangentOrientation::Aligned),
            },
            ContactBranchEdit {
                contact: tangent_contacts[1],
                curve: bezier,
                domain: bounded,
                value: 0.625_226_048_869_715_2,
                winding: 0,
                neighborhood: ContactNeighborhood::Interior,
                tangent_orientation: Some(TangentOrientation::Aligned),
            },
        ])
        .unwrap();

    let session = RetainedSketchDocumentSession::restore_design_with_accepted(
        design,
        accepted,
        SketchLifecycleRevisionHighWater::from_raw(42, 44, Some(41)),
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    assert!(matches!(
        session
            .last_attempt()
            .solve_result()
            .and_then(|solve| solve.rejection.as_ref()),
        Some(SolveRejection::AmbiguousContactNeighborhood(_))
    ));
    let mut retry = session.clone();
    let mut target = retry.design_document().point(points[1]).unwrap().position;
    target[0] += 0.01;
    let request = retry
        .last_attempt()
        .input()
        .candidate_request()
        .without_previous_state_preferences()
        .with_drag(points[1], target);
    let expected = retry.design_identity();
    let attempt = retry.reattempt(expected, request).unwrap();
    assert!(attempt.accepted_state_identity().is_some());
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
    for point in points.iter().copied() {
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
    let mut authoring = AuthoringState::default();
    let _ = authoring.activate(
        coordinator.session().design_document(),
        AuthoringTool::Constraint(ConstraintIntent::Coincident),
        &[],
    );
    assert!(matches!(
        authoring.pick(
            coordinator.session().design_document(),
            AuthoringOperand::picked(SelectionItem::Curve(lines[0]), Some(0.25)),
        ),
        AuthoringOutcome::Collecting { .. }
    ));
    let AuthoringOutcome::Apply(application) = authoring.pick(
        coordinator.session().design_document(),
        AuthoringOperand::picked(SelectionItem::Curve(lines[1]), Some(0.25)),
    ) else {
        panic!("impossible contact application expected");
    };
    let AuthoringMutation::Constraint(outcome) = coordinator
        .apply_authoring(coordinator.session().design_identity(), &application)
        .unwrap()
    else {
        panic!("constraint mutation expected");
    };
    assert!(outcome.published_accepted.is_none());
    authoring.transaction_finished();
    assert!(authoring.pending().is_empty());
    assert_eq!(
        authoring.active_tool(),
        Some(AuthoringTool::Constraint(ConstraintIntent::Coincident))
    );
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

    assert!(matches!(
        authoring.pick(
            coordinator.session().design_document(),
            AuthoringOperand::selected(SelectionItem::Point(points[0])),
        ),
        AuthoringOutcome::Collecting { .. }
    ));
    let AuthoringOutcome::Apply(application) = authoring.pick(
        coordinator.session().design_document(),
        AuthoringOperand::picked(SelectionItem::Curve(lines[0]), Some(0.0)),
    ) else {
        panic!("recovery point-on-curve application expected");
    };
    let AuthoringMutation::Constraint(recovered) = coordinator
        .apply_authoring(coordinator.session().design_identity(), &application)
        .unwrap()
    else {
        panic!("recovery constraint mutation expected");
    };
    assert!(recovered.published_accepted.is_some());
    authoring.transaction_finished();
    assert!(authoring.pending().is_empty());
    assert_eq!(
        coordinator.session().design_document().constraints().len(),
        5
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
                intent: ConstraintIntent::Coincident,
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
                relation: None,
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
