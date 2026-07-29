// SPDX-License-Identifier: GPL-3.0-or-later
#![allow(
    clippy::too_many_lines,
    reason = "complete action matrices are clearer as contiguous integration scenarios"
)]

use geosolve_constraint_editor::{
    ActionChoice, ActionState, BranchAction, ConstraintActionRequest, ConstraintIntent,
    ConstraintRelationChoice, ContactActionChoice, CoordinatorActionKind, DimensionActionRequest,
    DimensionKind, EditorScene, Modifiers, PointerInput, ResolvedConstraintKind,
    RetainedEditorCoordinator, ScreenPoint, SelectionItem, Viewport,
};
use geosolve_sketch::{
    AlphaScenarioIds, AlphaScenarioKind, ContactBranchEdit, ContactDomain, ContactNeighborhood,
    CurveDefinition, CurveSpan, DocumentAngleOrientation, DocumentConstraintDefinition,
    DocumentCurveContinuity, DocumentCurveCurvatureRelation, DocumentCurveDirectionRelation,
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
                SelectionItem::Curve(fixture.beziers[0]),
            ],
            ConstraintIntent::Parallel,
            ResolvedConstraintKind::CurveTangentDirection,
        ),
        (
            vec![
                SelectionItem::Curve(fixture.lines[0]),
                SelectionItem::Curve(fixture.beziers[0]),
            ],
            ConstraintIntent::Perpendicular,
            ResolvedConstraintKind::CurveNormalDirection,
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

    let mut direction = coordinator(fixture.document.clone());
    direction.editor_mut().set_selection([
        SelectionItem::Curve(fixture.lines[0]),
        SelectionItem::Curve(fixture.beziers[0]),
    ]);
    let outcome = direction
        .apply_constraint_action(
            direction.session().design_identity(),
            ConstraintActionRequest {
                intent: ConstraintIntent::Parallel,
                label: "tangent direction".into(),
                contacts: vec![contact(
                    fixture.beziers[0],
                    0.5,
                    ContactNeighborhood::Interior,
                )],
                relation: Some(ConstraintRelationChoice::CurveDirection(
                    DocumentCurveDirectionRelation::Tangent {
                        orientation: TangentOrientation::Aligned,
                    },
                )),
            },
        )
        .unwrap();
    assert!(matches!(
        direction
            .session()
            .design_document()
            .constraint(outcome.value)
            .unwrap()
            .definition,
        DocumentConstraintDefinition::CurveDirection {
            relation: DocumentCurveDirectionRelation::Tangent {
                orientation: TangentOrientation::Aligned
            },
            ..
        }
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
                intent: ConstraintIntent::Coincident,
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
                relation: None,
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
