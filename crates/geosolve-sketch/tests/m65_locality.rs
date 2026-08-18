use geosolve_core::{OperationControl, ResidualCategory, SecondaryStatus, SolverConfig};
use geosolve_sketch::{
    ContactNeighborhood, CurveDefinition, CurveSpan, DesignPointId, DocumentArcSweep,
    DocumentConstraintDefinition, DocumentCoordinateAxis, DocumentDimensionDefinition,
    DocumentDimensionMode, DocumentDragLocalityPlan, DocumentRuntimeMap, DocumentSolveRequest,
    PointId, RetainedSketchDocumentSession, ScalarDomain, ScalarUnit, SketchDocument,
    SketchSolveResult, SketchSource, TangentOrientation,
};

struct LocalityFixture {
    session: RetainedSketchDocumentSession,
    request: DocumentSolveRequest,
    active: DesignPointId,
}

struct TransientObjectiveInventory {
    plan: DocumentDragLocalityPlan,
    temporary_points: Vec<DesignPointId>,
    previous_state_points: Vec<DesignPointId>,
}

fn add_point_distance(
    document: &mut SketchDocument,
    label: &str,
    first: DesignPointId,
    second: DesignPointId,
    target: f64,
) {
    let target = document
        .add_scalar(
            format!("{label} target"),
            target,
            ScalarUnit::Length,
            ScalarDomain::Positive,
        )
        .unwrap();
    document
        .add_dimension(
            label,
            DocumentDimensionDefinition::PointDistance {
                first,
                second,
                target,
            },
            DocumentDimensionMode::Driving,
        )
        .unwrap();
}

fn rank_gain_fixture() -> (LocalityFixture, DesignPointId, DesignPointId) {
    let mut document = SketchDocument::new(2.0).unwrap();
    let active = document.add_point("active", [0.0, 0.0]).unwrap();
    let one_direction = document.add_point("one direction", [1.0, 0.0]).unwrap();
    let two_directions = document.add_point("two directions", [1.0, 1.0]).unwrap();
    add_point_distance(&mut document, "first link", active, one_direction, 1.0);
    add_point_distance(
        &mut document,
        "second link",
        one_direction,
        two_directions,
        1.0,
    );

    let request = DocumentSolveRequest::default().without_previous_state_preferences();
    let session =
        RetainedSketchDocumentSession::new(document, request, SolverConfig::default()).unwrap();
    (
        LocalityFixture {
            session,
            request,
            active,
        },
        one_direction,
        two_directions,
    )
}

fn mobility_rank_fixture() -> (LocalityFixture, DesignPointId, DesignPointId) {
    let mut document = SketchDocument::new(2.0).unwrap();
    let active = document.add_point("active", [0.0, 0.0]).unwrap();
    let higher_mobility = document.add_point("higher mobility", [0.0, 1.0]).unwrap();
    let lower_mobility = document.add_point("lower mobility", [2.0, 1.0]).unwrap();
    let vertical = document
        .add_curve(
            "vertical link",
            CurveDefinition::Line {
                start: active,
                end: higher_mobility,
                branch_direction: [0.0, 1.0],
            },
        )
        .unwrap();
    let horizontal = document
        .add_curve(
            "horizontal link",
            CurveDefinition::Line {
                start: higher_mobility,
                end: lower_mobility,
                branch_direction: [1.0, 0.0],
            },
        )
        .unwrap();
    for (label, definition) in [
        (
            "active y",
            DocumentConstraintDefinition::FixedCoordinate {
                point: active,
                axis: DocumentCoordinateAxis::Y,
                target: 0.0,
            },
        ),
        (
            "lower-mobility x",
            DocumentConstraintDefinition::FixedCoordinate {
                point: lower_mobility,
                axis: DocumentCoordinateAxis::X,
                target: 2.0,
            },
        ),
        (
            "vertical relation",
            DocumentConstraintDefinition::Vertical {
                line: CurveSpan::line(vertical),
            },
        ),
        (
            "horizontal relation",
            DocumentConstraintDefinition::Horizontal {
                line: CurveSpan::line(horizontal),
            },
        ),
    ] {
        document.add_constraint(label, definition).unwrap();
    }

    let request = DocumentSolveRequest::default().without_previous_state_preferences();
    let session =
        RetainedSketchDocumentSession::new(document, request, SolverConfig::default()).unwrap();
    (
        LocalityFixture {
            session,
            request,
            active,
        },
        higher_mobility,
        lower_mobility,
    )
}

fn compile_order_fixture() -> (LocalityFixture, DesignPointId, DesignPointId) {
    let mut document = SketchDocument::new(2.0).unwrap();
    let active = document.add_point("active", [0.0, 0.0]).unwrap();
    let earlier = document.add_point("earlier candidate", [1.0, 0.0]).unwrap();
    let later = document.add_point("later candidate", [0.0, 1.0]).unwrap();
    add_point_distance(&mut document, "active-earlier", active, earlier, 1.0);
    add_point_distance(&mut document, "active-later", active, later, 1.0);
    add_point_distance(
        &mut document,
        "candidate edge",
        earlier,
        later,
        2.0_f64.sqrt(),
    );

    let request = DocumentSolveRequest::default().without_previous_state_preferences();
    let session =
        RetainedSketchDocumentSession::new(document, request, SolverConfig::default()).unwrap();
    (
        LocalityFixture {
            session,
            request,
            active,
        },
        earlier,
        later,
    )
}

fn persistent_point(mappings: &DocumentRuntimeMap, runtime: PointId) -> DesignPointId {
    mappings
        .point_mappings()
        .iter()
        .find_map(|mapping| (mapping.runtime == runtime).then_some(mapping.persistent))
        .expect("transient source point has a persistent mapping")
}

fn assert_transient_audit_categories(solve: &SketchSolveResult) {
    let mapped_transient_sources = solve
        .source_mappings
        .iter()
        .filter_map(|mapping| {
            mapping
                .core_source_id
                .map(|source| (source, mapping.source))
        })
        .collect::<Vec<_>>();
    for source in &solve.display_audit.sources {
        for row in &source.rows {
            if !matches!(
                row.category,
                ResidualCategory::Temporary | ResidualCategory::Preference
            ) {
                continue;
            }
            let mapped = mapped_transient_sources
                .iter()
                .find_map(|(id, mapped)| (*id == source.source_id).then_some(*mapped))
                .expect("every secondary audit row has a source mapping");
            assert!(matches!(
                (row.category, mapped),
                (ResidualCategory::Temporary, SketchSource::DragTarget(_))
                    | (ResidualCategory::Preference, SketchSource::PreviousState(_))
            ));
        }
    }
}

fn transient_objective_inventory(fixture: &LocalityFixture) -> TransientObjectiveInventory {
    let plan = fixture
        .session
        .drag_locality_plan(fixture.active)
        .expect("locality plan");
    let target = fixture
        .session
        .accepted_state()
        .unwrap()
        .document()
        .point(fixture.active)
        .unwrap()
        .position;
    let mut preview = fixture.session.clone();
    let _ = preview
        .reattempt_with_drag_locality_controlled(
            preview.design_identity(),
            fixture
                .request
                .with_previous_state_preferences()
                .with_drag(fixture.active, target),
            &plan,
            OperationControl::unlimited(),
        )
        .unwrap();
    assert!(
        preview.last_attempt().accepted_state_identity().is_some(),
        "{:#?}",
        preview.last_attempt().solve_result()
    );

    let attempt = preview.last_attempt();
    let solve = attempt.solve_result().expect("reported locality solve");
    let mappings = attempt.mappings().expect("attempt runtime mappings");
    let mut temporary_points = Vec::new();
    let mut previous_state_points = Vec::new();
    for mapping in &solve.source_mappings {
        let expected_category = match mapping.source {
            SketchSource::DragTarget(point) => {
                temporary_points.push(persistent_point(mappings, point));
                Some(ResidualCategory::Temporary)
            }
            SketchSource::PreviousState(point) => {
                previous_state_points.push(persistent_point(mappings, point));
                Some(ResidualCategory::Preference)
            }
            SketchSource::Constraint(_) | SketchSource::Dimension(_) => None,
        };
        let Some(expected_category) = expected_category else {
            continue;
        };
        let source_id = mapping
            .core_source_id
            .expect("every transient objective has a core source");
        let audit = solve
            .display_audit
            .sources
            .iter()
            .find(|source| source.source_id == source_id)
            .expect("transient objective has accepted audit rows");
        assert!(!audit.rows.is_empty());
        assert!(
            audit
                .rows
                .iter()
                .all(|row| row.category == expected_category)
        );
    }
    assert_transient_audit_categories(solve);

    TransientObjectiveInventory {
        plan,
        temporary_points,
        previous_state_points,
    }
}

#[test]
fn drag_locality_prefers_greatest_rank_gain_and_uses_a_minimal_cover() {
    let (fixture, lower_gain, complete_cover) = rank_gain_fixture();
    let inventory = transient_objective_inventory(&fixture);

    assert_eq!(inventory.plan.passive_degrees_of_freedom(), 2);
    assert_eq!(
        inventory.plan.anchor_count(),
        1,
        "one point spans both passive directions, so the cover must not use one anchor per DOF"
    );
    assert_eq!(inventory.previous_state_points, vec![complete_cover]);
    assert_ne!(inventory.previous_state_points, vec![lower_gain]);
}

#[test]
fn drag_locality_breaks_equal_gain_by_lower_point_mobility_rank() {
    let (fixture, earlier_higher_mobility, later_lower_mobility) = mobility_rank_fixture();
    let inventory = transient_objective_inventory(&fixture);

    assert_eq!(inventory.plan.passive_degrees_of_freedom(), 1);
    assert_eq!(inventory.plan.anchor_count(), 1);
    assert_eq!(inventory.previous_state_points, vec![later_lower_mobility]);
    assert_ne!(
        inventory.previous_state_points,
        vec![earlier_higher_mobility],
        "compile order must not outrank the lower-mobility tie-break"
    );
}

#[test]
fn drag_locality_breaks_exact_candidate_ties_by_compile_order() {
    let (fixture, earlier, later) = compile_order_fixture();
    let inventory = transient_objective_inventory(&fixture);

    assert_eq!(inventory.plan.passive_degrees_of_freedom(), 1);
    assert_eq!(inventory.plan.anchor_count(), 1);
    assert_eq!(inventory.previous_state_points, vec![earlier]);
    assert_ne!(inventory.previous_state_points, vec![later]);
}

#[test]
fn drag_locality_compiles_only_the_cursor_and_planned_anchors_as_secondary_sources() {
    let (fixture, unselected, selected) = rank_gain_fixture();
    let inventory = transient_objective_inventory(&fixture);

    assert_eq!(inventory.temporary_points, vec![fixture.active]);
    assert_eq!(inventory.previous_state_points, vec![selected]);
    assert!(!inventory.previous_state_points.contains(&fixture.active));
    assert!(!inventory.previous_state_points.contains(&unselected));
    assert_eq!(
        inventory.previous_state_points.len(),
        inventory.plan.anchor_count()
    );
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "the exact tangency fixture audits both center drags and all retained contact and sweep state"
)]
fn m78_f011_endpoint_tangency_does_not_block_arc_center_locality() {
    let mut document = SketchDocument::new(3.0).unwrap();
    let source_center = document.add_point("source arc center", [0.0, 0.0]).unwrap();
    let tangent_center = document
        .add_point("tangent arc center", [0.0, 3.0])
        .unwrap();
    let unrelated = document.add_point("unrelated point", [8.0, -5.0]).unwrap();
    let mut add_arc = |label: &str, center, radius_value, start_value, end_value, sweep| {
        let radius = document
            .add_scalar(
                format!("{label} radius"),
                radius_value,
                ScalarUnit::Length,
                ScalarDomain::Positive,
            )
            .unwrap();
        let start_angle = document
            .add_scalar(
                format!("{label} start"),
                start_value,
                ScalarUnit::Angle,
                ScalarDomain::Finite,
            )
            .unwrap();
        let end_angle = document
            .add_scalar(
                format!("{label} end"),
                end_value,
                ScalarUnit::Angle,
                ScalarDomain::Finite,
            )
            .unwrap();
        document
            .add_curve(
                label,
                CurveDefinition::CircularArc {
                    center,
                    radius,
                    start_angle,
                    end_angle,
                    sweep,
                },
            )
            .unwrap()
    };
    let source = add_arc(
        "source arc",
        source_center,
        2.0,
        0.0,
        std::f64::consts::FRAC_PI_2,
        DocumentArcSweep::CounterClockwise,
    );
    let tangent = add_arc(
        "tangent arc",
        tangent_center,
        1.0,
        -std::f64::consts::FRAC_PI_2,
        std::f64::consts::PI,
        DocumentArcSweep::Clockwise,
    );
    let first_contact = document
        .add_curve_contact(
            "source end contact",
            CurveSpan::line(source),
            1.0,
            0,
            ContactNeighborhood::End,
            Some(TangentOrientation::Aligned),
        )
        .unwrap();
    let second_contact = document
        .add_curve_contact(
            "tangent start contact",
            CurveSpan::line(tangent),
            0.0,
            0,
            ContactNeighborhood::Start,
            Some(TangentOrientation::Aligned),
        )
        .unwrap();
    document
        .add_constraint(
            "tangent arc relation",
            DocumentConstraintDefinition::CurveCurveTangency {
                first_contact,
                second_contact,
            },
        )
        .unwrap();

    let request = DocumentSolveRequest::default().without_previous_state_preferences();
    let session =
        RetainedSketchDocumentSession::new(document, request, SolverConfig::default()).unwrap();
    let accepted = session.accepted_state().unwrap();
    assert!(
        accepted
            .solve_result()
            .acceptance_hard_residual_max
            .is_some_and(|residual| residual.is_finite() && residual <= 1.0e-9)
    );
    let rank = accepted.diagnostics().rank.unwrap();
    assert_eq!(rank.numerical_right_nullity, Some(7));
    let unrelated_start = accepted.document().point(unrelated).unwrap().position;

    for (active, passive, target) in [
        (tangent_center, source_center, [0.25, 3.2]),
        (source_center, tangent_center, [-0.2, 0.25]),
    ] {
        let plan = session
            .drag_locality_plan(active)
            .expect("scalar and fixed-contact freedom must not prevent point locality");
        assert_eq!(plan.passive_degrees_of_freedom(), 2);
        assert_eq!(plan.anchor_count(), 1);

        let mut preview = session.clone();
        let _ = preview
            .reattempt_with_drag_locality_controlled(
                preview.design_identity(),
                request
                    .with_previous_state_preferences()
                    .with_drag(active, target),
                &plan,
                OperationControl::unlimited(),
            )
            .unwrap();
        let attempted = preview.last_attempt().solve_result().unwrap();
        let report = attempted.unstable_core_report();
        assert!(
            preview.last_attempt().accepted_state_identity().is_some(),
            "termination={:?}, hard={:?}, temporary={:?}, preference={:?}, rejection={:?}",
            report.termination,
            report.hard_termination,
            report.temporary_status,
            report.preference_status,
            attempted.rejection,
        );
        let preview = preview
            .accepted_state()
            .expect("tangent arc center drag preview");
        let active_position = preview.document().point(active).unwrap().position;
        assert!((active_position[0] - target[0]).hypot(active_position[1] - target[1]) <= 1.0e-8);
        let passive_position = preview.document().point(passive).unwrap().position;
        assert!(passive_position.into_iter().all(f64::is_finite));
        assert_eq!(
            preview
                .document()
                .point(unrelated)
                .unwrap()
                .position
                .map(f64::to_bits),
            unrelated_start.map(f64::to_bits)
        );
        assert_eq!(report.temporary_status, SecondaryStatus::Optimal);
        assert!(matches!(
            report.preference_status,
            SecondaryStatus::Optimal | SecondaryStatus::Acceptable
        ));
        assert!(
            preview
                .solve_result()
                .acceptance_hard_residual_max
                .is_some_and(|residual| residual.is_finite() && residual <= 1.0e-9)
        );
        assert_eq!(
            preview
                .document()
                .contact(first_contact)
                .unwrap()
                .neighborhood,
            ContactNeighborhood::End
        );
        assert_eq!(
            preview
                .document()
                .contact(first_contact)
                .unwrap()
                .tangent_orientation,
            Some(TangentOrientation::Aligned)
        );
        assert_eq!(
            preview
                .document()
                .contact(second_contact)
                .unwrap()
                .neighborhood,
            ContactNeighborhood::Start
        );
        assert_eq!(
            preview
                .document()
                .contact(second_contact)
                .unwrap()
                .tangent_orientation,
            Some(TangentOrientation::Aligned)
        );
        for (contact, expected) in [(first_contact, 1.0_f64), (second_contact, 0.0_f64)] {
            let parameter = preview.document().contact(contact).unwrap().parameter;
            assert_eq!(
                preview
                    .document()
                    .scalar(parameter)
                    .unwrap()
                    .value
                    .to_bits(),
                expected.to_bits()
            );
        }
        assert!(matches!(
            preview.document().curve(source).unwrap().definition,
            CurveDefinition::CircularArc {
                sweep: DocumentArcSweep::CounterClockwise,
                ..
            }
        ));
        assert!(matches!(
            preview.document().curve(tangent).unwrap().definition,
            CurveDefinition::CircularArc {
                sweep: DocumentArcSweep::Clockwise,
                ..
            }
        ));
    }
}
