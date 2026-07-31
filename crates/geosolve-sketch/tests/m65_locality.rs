use geosolve_core::{OperationControl, ResidualCategory, SolverConfig};
use geosolve_sketch::{
    CurveDefinition, CurveSpan, DesignPointId, DocumentConstraintDefinition,
    DocumentCoordinateAxis, DocumentDimensionDefinition, DocumentDimensionMode,
    DocumentDragLocalityPlan, DocumentRuntimeMap, DocumentSolveRequest, PointId,
    RetainedSketchDocumentSession, ScalarDomain, ScalarUnit, SketchDocument, SketchSolveResult,
    SketchSource,
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
