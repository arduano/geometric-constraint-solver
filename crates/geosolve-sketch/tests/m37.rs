// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_geometry::Point2;
use geosolve_sketch::{
    AngleOrientation, ArcSweep, CurveDefinition, CurveSpan, DimensionMode, DocumentAngleOperand,
    DocumentAngleOrientation, DocumentCenterRef, DocumentContactSeed, DocumentDirectionSense,
    DocumentEndpointRef, DocumentLineSupportRef, DocumentPlanarRelation, DocumentPointRef,
    DocumentSemanticCatalogSession, DocumentSemanticSourceCatalog, FeatureEndpoint, Sketch,
    SketchDocument, SketchSolveRequest, TangentOrientation,
};

#[test]
fn m37_runtime_rows_are_executable_and_independently_validated() {
    let mut sketch = Sketch::new(1.0).unwrap();
    let a = sketch.add_point(Point2::new(0.0, 0.0)).unwrap();
    let b = sketch.add_point(Point2::new(2.0, 0.3)).unwrap();
    let c = sketch.add_point(Point2::new(1.0, 1.0)).unwrap();
    let d = sketch.add_point(Point2::new(3.0, 1.2)).unwrap();
    let first = sketch.add_segment(a, b).unwrap();
    let second = sketch.add_segment(c, d).unwrap();

    sketch.add_horizontal_points(a, b).unwrap();
    sketch.add_collinear(first, second).unwrap();
    sketch.add_equal_distance(a, b, c, d).unwrap();
    sketch
        .add_equal_angle(
            first,
            second,
            first,
            second,
            AngleOrientation::CounterClockwise,
            0,
            AngleOrientation::CounterClockwise,
            0,
        )
        .unwrap();
    let result = sketch
        .solve(
            SketchSolveRequest::default().without_previous_state_preferences(),
            geosolve_core::SolverConfig::default(),
        )
        .unwrap();
    assert!(result.accepted());
    assert!(result.acceptance_hard_residual_max.unwrap() <= 1.0e-9);
}

#[test]
#[allow(clippy::many_single_char_names)]
fn m37_new_rows_match_central_finite_differences_at_required_scales() {
    for scale in [1.0e-6, 1.0, 1.0e6] {
        let mut sketch = Sketch::new(scale).unwrap();
        let transform = |x: f64, y: f64| {
            let angle: f64 = 0.37;
            Point2::new(
                scale * (angle.cos() * x - angle.sin() * y + 3.0),
                scale * (angle.sin() * x + angle.cos() * y - 2.0),
            )
        };
        let a = sketch.add_point(transform(0.0, 0.0)).unwrap();
        let b = sketch.add_point(transform(2.0, 0.1)).unwrap();
        let c = sketch.add_point(transform(0.1, 1.0)).unwrap();
        let d = sketch.add_point(transform(2.2, 1.2)).unwrap();
        let e = sketch.add_point(transform(1.0, 0.5)).unwrap();
        let first = sketch.add_segment(a, b).unwrap();
        let second = sketch.add_segment(c, d).unwrap();
        sketch.add_collinear(first, second).unwrap();
        sketch.add_equal_distance(a, b, c, d).unwrap();
        sketch.add_point_symmetry(a, d, e).unwrap();
        sketch
            .add_equal_angle(
                first,
                second,
                second,
                first,
                AngleOrientation::CounterClockwise,
                0,
                AngleOrientation::Clockwise,
                0,
            )
            .unwrap();
        let compiled = sketch
            .compile(SketchSolveRequest::default().without_previous_state_preferences())
            .unwrap();
        let check = compiled.problem().check_jacobians(1.0e-6).unwrap();
        assert!(check.all_within(1.0e-6), "scale {scale}: {check:#?}");
    }
}

#[test]
fn m37_high_level_contact_and_tangent_constructors_allocate_explicit_latents_atomically() {
    let mut document = SketchDocument::new(1.0).unwrap();
    let a = document.add_point("a", [0.0, 0.0]).unwrap();
    let b = document.add_point("b", [2.0, 0.0]).unwrap();
    let c = document.add_point("c", [0.0, 0.0]).unwrap();
    let d = document.add_point("d", [2.0, 0.0]).unwrap();
    let first = document
        .add_curve(
            "first",
            CurveDefinition::Line {
                start: a,
                end: b,
                branch_direction: [1.0, 0.0],
            },
        )
        .unwrap();
    let second = document
        .add_curve(
            "second",
            CurveDefinition::Line {
                start: c,
                end: d,
                branch_direction: [1.0, 0.0],
            },
        )
        .unwrap();
    let seed = |curve| DocumentContactSeed {
        support: geosolve_sketch::DocumentCurveSpanRef {
            span: CurveSpan::line(curve),
            winding: 0,
        },
        parameter: 0.5,
        neighborhood: geosolve_sketch::ContactNeighborhood::Local {
            lower: 0.25,
            upper: 0.75,
        },
    };
    let before_contacts = document.contacts().len();
    let tangent = document
        .add_curve_curve_tangent_relation(
            "tangent",
            seed(first),
            seed(second),
            TangentOrientation::Aligned,
        )
        .unwrap();
    assert_eq!(tangent.contacts.len(), 2);
    assert_eq!(document.contacts().len(), before_contacts + 2);
    for contact in &tangent.contacts {
        assert_eq!(
            document.contact(*contact).unwrap().tangent_orientation,
            Some(TangentOrientation::Aligned)
        );
    }

    let before = document.to_canonical_json().unwrap();
    assert!(
        document
            .add_curve_curve_contact_relation("tautological self contact", seed(first), seed(first))
            .is_err()
    );
    assert_eq!(document.to_canonical_json().unwrap(), before);
    let invalid = DocumentContactSeed {
        parameter: 2.0,
        ..seed(first)
    };
    assert!(
        document
            .add_curve_curve_contact_relation("invalid", invalid, seed(second))
            .is_err()
    );
    assert_eq!(document.to_canonical_json().unwrap(), before);
}

#[test]
fn m37_signed_zero_identical_host_contact_rejects_atomically() {
    let mut document = SketchDocument::new(1.0).unwrap();
    let center = document.add_point("center", [0.0, 0.0]).unwrap();
    let radius = document
        .add_scalar(
            "radius",
            1.0,
            geosolve_sketch::ScalarUnit::Length,
            geosolve_sketch::ScalarDomain::Positive,
        )
        .unwrap();
    let circle = document
        .add_curve("circle", CurveDefinition::Circle { center, radius })
        .unwrap();
    let positive_zero = DocumentContactSeed {
        support: geosolve_sketch::DocumentCurveSpanRef {
            span: CurveSpan::line(circle),
            winding: 0,
        },
        parameter: 0.0,
        neighborhood: geosolve_sketch::ContactNeighborhood::Interior,
    };
    let negative_zero = DocumentContactSeed {
        parameter: -0.0,
        ..positive_zero
    };
    let before = document.to_canonical_json().unwrap();

    assert!(
        document
            .add_curve_curve_contact_relation(
                "signed-zero self contact",
                positive_zero,
                negative_zero,
            )
            .is_err()
    );
    assert_eq!(document.to_canonical_json().unwrap(), before);
}

#[test]
fn m37_semantic_catalog_persists_one_source_and_solves_the_document() {
    let mut document = SketchDocument::new(1.0).unwrap();
    let a = document.add_point("a", [0.0, 0.0]).unwrap();
    let b = document.add_point("b", [2.0, 0.2]).unwrap();
    let c = document.add_point("c", [0.0, 1.0]).unwrap();
    let d = document.add_point("d", [2.0, 1.3]).unwrap();
    let first = document
        .add_curve(
            "first",
            CurveDefinition::Line {
                start: a,
                end: b,
                branch_direction: [1.0, 0.0],
            },
        )
        .unwrap();
    let second = document
        .add_curve(
            "second",
            CurveDefinition::Line {
                start: c,
                end: d,
                branch_direction: [1.0, 0.0],
            },
        )
        .unwrap();
    let mut catalog = DocumentSemanticSourceCatalog::new(&mut document).unwrap();
    let source = catalog
        .add_planar_source(
            &mut document,
            "collinear supports",
            DocumentPlanarRelation::Collinear {
                first: DocumentLineSupportRef {
                    span: CurveSpan::line(first),
                    direction: DocumentDirectionSense::Forward,
                },
                second: DocumentLineSupportRef {
                    span: CurveSpan::line(second),
                    direction: DocumentDirectionSense::Forward,
                },
            },
        )
        .unwrap();
    catalog
        .add_planar_source(
            &mut document,
            "horizontal endpoints",
            DocumentPlanarRelation::HorizontalPoints {
                first: DocumentPointRef::Point { point: a },
                second: DocumentPointRef::Point { point: b },
            },
        )
        .unwrap();

    let document_json = document.to_canonical_json().unwrap();
    let catalog_json = catalog.to_canonical_json().unwrap();
    let mut restored_document = SketchDocument::from_json(&document_json).unwrap();
    let restored =
        DocumentSemanticSourceCatalog::from_json(&mut restored_document, &catalog_json).unwrap();
    assert_eq!(restored.planar_source(source).unwrap().source_id(), source);

    let solved = restored
        .solve_document(
            &restored_document,
            SketchSolveRequest::default().without_previous_state_preferences(),
            geosolve_core::SolverConfig::default(),
        )
        .unwrap();
    assert!(solved.solve_result.accepted());
    assert!(solved.solve_result.acceptance_hard_residual_max.unwrap() <= 1.0e-9);
    let audit = solved
        .audit
        .iter()
        .find(|audit| audit.source_id == source)
        .unwrap();
    assert_eq!(audit.rows.len(), 2);
    assert!(
        audit
            .rows
            .iter()
            .all(|row| row.normalized_residual.abs() <= 1.0e-9)
    );
}

#[test]
fn m37_concentric_and_center_point_symmetry_use_typed_features() {
    let mut document = SketchDocument::new(1.0).unwrap();
    let c1 = document.add_point("c1", [0.0, 0.0]).unwrap();
    let c2 = document.add_point("c2", [0.3, 0.2]).unwrap();
    let p = document.add_point("p", [-1.0, 0.0]).unwrap();
    let q = document.add_point("q", [1.2, 0.0]).unwrap();
    let r1 = document
        .add_scalar(
            "r1",
            1.0,
            geosolve_sketch::ScalarUnit::Length,
            geosolve_sketch::ScalarDomain::Positive,
        )
        .unwrap();
    let r2 = document
        .add_scalar(
            "r2",
            2.0,
            geosolve_sketch::ScalarUnit::Length,
            geosolve_sketch::ScalarDomain::Positive,
        )
        .unwrap();
    let first = document
        .add_curve(
            "first",
            CurveDefinition::Circle {
                center: c1,
                radius: r1,
            },
        )
        .unwrap();
    let second = document
        .add_curve(
            "second",
            CurveDefinition::Circle {
                center: c2,
                radius: r2,
            },
        )
        .unwrap();
    let mut catalog = DocumentSemanticSourceCatalog::new(&mut document).unwrap();
    catalog
        .add_planar_source(
            &mut document,
            "concentric",
            DocumentPlanarRelation::Concentric {
                first: DocumentCenterRef { curve: first },
                second: DocumentCenterRef { curve: second },
            },
        )
        .unwrap();
    catalog
        .add_planar_source(
            &mut document,
            "center symmetry",
            DocumentPlanarRelation::PointSymmetry {
                first: DocumentPointRef::Point { point: p },
                second: DocumentPointRef::Point { point: q },
                center: DocumentPointRef::Center(DocumentCenterRef { curve: first }),
            },
        )
        .unwrap();
    let solved = catalog
        .solve_document(
            &document,
            SketchSolveRequest::default().without_previous_state_preferences(),
            geosolve_core::SolverConfig::default(),
        )
        .unwrap();
    assert!(solved.solve_result.accepted());
}

#[test]
fn m37_controlled_catalog_solve_cancels_without_publication() {
    let mut document = SketchDocument::new(1.0).unwrap();
    let a = document.add_point("a", [0.0, 0.0]).unwrap();
    let b = document.add_point("b", [1.0, 0.2]).unwrap();
    let mut catalog = DocumentSemanticSourceCatalog::new(&mut document).unwrap();
    catalog
        .add_planar_source(
            &mut document,
            "horizontal",
            DocumentPlanarRelation::HorizontalPoints {
                first: DocumentPointRef::Point { point: a },
                second: DocumentPointRef::Point { point: b },
            },
        )
        .unwrap();
    let before = document.to_canonical_json().unwrap();
    let (handle, token) = geosolve_sketch::cancellation_pair();
    handle.cancel();
    let outcome = catalog
        .solve_document_controlled(
            &document,
            SketchSolveRequest::default(),
            geosolve_core::SolverConfig::default(),
            geosolve_sketch::OperationControl::new(
                token,
                geosolve_sketch::OperationLimits::unlimited(),
            ),
        )
        .unwrap();
    assert!(matches!(
        outcome,
        geosolve_sketch::OperationOutcome::Cancelled { .. }
    ));
    assert_eq!(document.to_canonical_json().unwrap(), before);

    let completed = catalog
        .solve_document_controlled(
            &document,
            SketchSolveRequest::default(),
            geosolve_core::SolverConfig::default(),
            geosolve_sketch::OperationControl::new(
                geosolve_sketch::CancellationToken::default(),
                geosolve_sketch::OperationLimits::unlimited(),
            ),
        )
        .unwrap();
    let geosolve_sketch::OperationOutcome::Completed { report, .. } = completed else {
        panic!("unlimited controlled semantic solve must complete");
    };
    let mut limits = geosolve_sketch::OperationLimits::unlimited();
    limits.document_lowering_items = report.consumed.document_lowering_items - 1;
    let exhausted = catalog
        .solve_document_controlled(
            &document,
            SketchSolveRequest::default(),
            geosolve_core::SolverConfig::default(),
            geosolve_sketch::OperationControl::new(
                geosolve_sketch::CancellationToken::default(),
                limits,
            ),
        )
        .unwrap();
    assert!(matches!(
        exhausted,
        geosolve_sketch::OperationOutcome::WorkExhausted { .. }
    ));
    assert_eq!(document.to_canonical_json().unwrap(), before);

    let mut limits = geosolve_sketch::OperationLimits::unlimited();
    limits.component_linearizations = report.consumed.component_linearizations - 1;
    let exhausted_during_fresh_audit = catalog
        .solve_document_controlled(
            &document,
            SketchSolveRequest::default(),
            geosolve_core::SolverConfig::default(),
            geosolve_sketch::OperationControl::new(
                geosolve_sketch::CancellationToken::default(),
                limits,
            ),
        )
        .unwrap();
    assert!(matches!(
        exhausted_during_fresh_audit,
        geosolve_sketch::OperationOutcome::WorkExhausted { .. }
    ));
    assert_eq!(document.to_canonical_json().unwrap(), before);
}

#[test]
fn m37_block_entity_and_retained_session_roll_back_rejected_candidates() {
    let mut document = SketchDocument::new(1.0).unwrap();
    let a = document.add_point("a", [0.0, 0.0]).unwrap();
    let b = document.add_point("b", [1.0, 0.0]).unwrap();
    let line = document
        .add_curve(
            "line",
            CurveDefinition::Line {
                start: a,
                end: b,
                branch_direction: [1.0, 0.0],
            },
        )
        .unwrap();
    let mut catalog = DocumentSemanticSourceCatalog::new(&mut document).unwrap();
    catalog
        .add_block_entity_source(&mut document, "block line", line)
        .unwrap();
    let mut session = DocumentSemanticCatalogSession::new(
        &document,
        catalog,
        SketchSolveRequest::default(),
        geosolve_core::SolverConfig::default(),
    )
    .unwrap();
    let before = session.accepted().document.to_canonical_json().unwrap();
    let mut candidate = session.accepted().document.clone();
    candidate
        .add_constraint(
            "conflicting fixed point",
            geosolve_sketch::DocumentConstraintDefinition::FixedPoint {
                point: b,
                target: [2.0, 1.0],
            },
        )
        .unwrap();
    assert!(session.replace_document(0, &candidate).is_err());
    assert_eq!(session.revision(), 0);
    assert_eq!(
        session.accepted().document.to_canonical_json().unwrap(),
        before
    );

    let mut value: serde_json::Value = serde_json::from_str(&before).unwrap();
    value["curves"][0]["definition"]["branch_direction"] = serde_json::json!([0.0, 1.0]);
    let branch_changed =
        SketchDocument::from_json(&serde_json::to_string(&value).unwrap()).unwrap();
    assert!(session.replace_document(0, &branch_changed).is_err());
    assert_eq!(session.revision(), 0);
    assert_eq!(
        session.accepted().document.to_canonical_json().unwrap(),
        before
    );
}

#[test]
fn m37_m36_equal_scalar_radius_source_is_executable_and_audited() {
    let mut document = SketchDocument::new(1.0).unwrap();
    let c1 = document.add_point("c1", [0.0, 0.0]).unwrap();
    let c2 = document.add_point("c2", [4.0, 0.0]).unwrap();
    let r1 = document
        .add_scalar(
            "r1",
            1.0,
            geosolve_sketch::ScalarUnit::Length,
            geosolve_sketch::ScalarDomain::Positive,
        )
        .unwrap();
    let r2 = document
        .add_scalar(
            "r2",
            1.3,
            geosolve_sketch::ScalarUnit::Length,
            geosolve_sketch::ScalarDomain::Positive,
        )
        .unwrap();
    document
        .add_curve(
            "one",
            CurveDefinition::Circle {
                center: c1,
                radius: r1,
            },
        )
        .unwrap();
    document
        .add_curve(
            "two",
            CurveDefinition::Circle {
                center: c2,
                radius: r2,
            },
        )
        .unwrap();
    let property = |scalar| geosolve_sketch::DocumentScalarPropertyRef {
        scalar,
        unit: geosolve_sketch::DocumentScalarUnit::Length,
        domain: geosolve_sketch::ScalarDomain::Positive,
        branch: geosolve_sketch::DocumentScalarBranch::Unsigned,
    };
    let mut catalog = DocumentSemanticSourceCatalog::new(&mut document).unwrap();
    catalog
        .add_scalar_source(
            &mut document,
            "equal radii",
            geosolve_sketch::DocumentScalarRelation::Equal {
                first: property(r1),
                second: property(r2),
            },
        )
        .unwrap();
    let solved = catalog
        .solve_document(
            &document,
            SketchSolveRequest::default().without_previous_state_preferences(),
            geosolve_core::SolverConfig::default(),
        )
        .unwrap();
    assert_eq!(solved.scalar_audit.len(), 1);
    assert!(solved.scalar_audit[0].rows[0].normalized_value.abs() <= 1.0e-9);
}

#[test]
fn m37_point_symmetry_and_equal_circle_arc_radius_solve() {
    let mut sketch = Sketch::new(1.0).unwrap();
    let first = sketch.add_point(Point2::new(-1.2, 0.0)).unwrap();
    let second = sketch.add_point(Point2::new(1.0, 0.0)).unwrap();
    let center = sketch.add_point(Point2::new(0.0, 0.0)).unwrap();
    sketch.add_point_symmetry(first, second, center).unwrap();

    let circle_center = sketch.add_point(Point2::new(0.0, 3.0)).unwrap();
    let arc_center = sketch.add_point(Point2::new(5.0, 3.0)).unwrap();
    let circle = sketch.add_circle(circle_center, 2.0).unwrap();
    let arc = sketch
        .add_arc(arc_center, 2.4, 0.0, 1.0, ArcSweep::CounterClockwise)
        .unwrap();
    sketch.add_equal_circle_arc_radius(circle, arc).unwrap();
    sketch
        .add_circle_radius(circle, 2.0, DimensionMode::Driving)
        .unwrap();

    let result = sketch
        .solve(
            SketchSolveRequest::default().without_previous_state_preferences(),
            geosolve_core::SolverConfig::default(),
        )
        .unwrap();
    assert!(result.accepted());
    assert!(result.acceptance_hard_residual_max.unwrap() <= 1.0e-9);
}

#[test]
#[allow(clippy::too_many_lines)]
fn m37_remaining_matrix_rows_persist_solve_and_have_complete_audit() {
    let mut document = SketchDocument::new(1.0).unwrap();
    let axis_start = document.add_point("axis start", [0.0, -2.0]).unwrap();
    let axis_end = document.add_point("axis end", [0.0, 2.0]).unwrap();
    let left_start = document.add_point("left start", [-2.0, -1.0]).unwrap();
    let left_end = document.add_point("left end", [-2.0, 1.0]).unwrap();
    let right_start = document.add_point("right start", [2.0, -1.0]).unwrap();
    let right_end = document.add_point("right end", [2.0, 1.0]).unwrap();
    let line = |document: &mut SketchDocument, label, start, end, direction| {
        document
            .add_curve(
                label,
                CurveDefinition::Line {
                    start,
                    end,
                    branch_direction: direction,
                },
            )
            .unwrap()
    };
    let axis = line(&mut document, "axis", axis_start, axis_end, [0.0, 1.0]);
    let left = line(&mut document, "left", left_start, left_end, [0.0, 1.0]);
    let right = line(&mut document, "right", right_start, right_end, [0.0, 1.0]);
    let support = |curve| DocumentLineSupportRef {
        span: CurveSpan::line(curve),
        direction: DocumentDirectionSense::Forward,
    };
    let point = |point| DocumentPointRef::Point { point };

    let circle_center = document.add_point("circle center", [-4.0, 0.0]).unwrap();
    let arc_center_a = document.add_point("arc center a", [4.0, 0.0]).unwrap();
    let arc_center_b = document.add_point("arc center b", [7.0, 0.0]).unwrap();
    let radius = |document: &mut SketchDocument, label| {
        document
            .add_scalar(
                label,
                1.0,
                geosolve_sketch::ScalarUnit::Length,
                geosolve_sketch::ScalarDomain::Positive,
            )
            .unwrap()
    };
    let circle_radius = radius(&mut document, "circle radius");
    let arc_radius_a = radius(&mut document, "arc radius a");
    let arc_radius_b = radius(&mut document, "arc radius b");
    let start_a = document
        .add_scalar(
            "start a",
            0.0,
            geosolve_sketch::ScalarUnit::Angle,
            geosolve_sketch::ScalarDomain::Finite,
        )
        .unwrap();
    let end_a = document
        .add_scalar(
            "end a",
            1.0,
            geosolve_sketch::ScalarUnit::Angle,
            geosolve_sketch::ScalarDomain::Finite,
        )
        .unwrap();
    let start_b = document
        .add_scalar(
            "start b",
            0.2,
            geosolve_sketch::ScalarUnit::Angle,
            geosolve_sketch::ScalarDomain::Finite,
        )
        .unwrap();
    let end_b = document
        .add_scalar(
            "end b",
            1.2,
            geosolve_sketch::ScalarUnit::Angle,
            geosolve_sketch::ScalarDomain::Finite,
        )
        .unwrap();
    let circle = document
        .add_curve(
            "circle",
            CurveDefinition::Circle {
                center: circle_center,
                radius: circle_radius,
            },
        )
        .unwrap();
    let arc_a = document
        .add_curve(
            "arc a",
            CurveDefinition::CircularArc {
                center: arc_center_a,
                radius: arc_radius_a,
                start_angle: start_a,
                end_angle: end_a,
                sweep: geosolve_sketch::DocumentArcSweep::CounterClockwise,
            },
        )
        .unwrap();
    let arc_b = document
        .add_curve(
            "arc b",
            CurveDefinition::CircularArc {
                center: arc_center_b,
                radius: arc_radius_b,
                start_angle: start_b,
                end_angle: end_b,
                sweep: geosolve_sketch::DocumentArcSweep::CounterClockwise,
            },
        )
        .unwrap();

    let mut catalog = DocumentSemanticSourceCatalog::new(&mut document).unwrap();
    catalog
        .add_planar_source(
            &mut document,
            "vertical point features",
            DocumentPlanarRelation::VerticalPoints {
                first: point(left_start),
                second: point(left_end),
            },
        )
        .unwrap();
    let symmetry = catalog
        .add_planar_source(
            &mut document,
            "entity symmetry",
            DocumentPlanarRelation::EntitySymmetry {
                first_entity: left,
                second_entity: right,
                point_pairs: vec![
                    [point(left_start), point(right_start)],
                    [point(left_end), point(right_end)],
                ],
                scalar_pairs: vec![],
                axis: support(axis),
            },
        )
        .unwrap();
    for (label, first, second) in [
        ("circle arc radius", circle, arc_a),
        ("arc arc radius", arc_a, arc_b),
    ] {
        catalog
            .add_planar_source(
                &mut document,
                label,
                DocumentPlanarRelation::EqualCircularRadius { first, second },
            )
            .unwrap();
    }
    catalog
        .add_planar_source(
            &mut document,
            "explicit angle branch",
            DocumentPlanarRelation::EqualAngle {
                first: DocumentAngleOperand {
                    first: support(left),
                    second: support(axis),
                    orientation: DocumentAngleOrientation::CounterClockwise,
                    winding: 0,
                },
                second: DocumentAngleOperand {
                    first: support(right),
                    second: support(axis),
                    orientation: DocumentAngleOrientation::CounterClockwise,
                    winding: 0,
                },
            },
        )
        .unwrap();

    let document_json = document.to_canonical_json().unwrap();
    let catalog_json = catalog.to_canonical_json().unwrap();
    let mut restored_document = SketchDocument::from_json(&document_json).unwrap();
    let restored =
        DocumentSemanticSourceCatalog::from_json(&mut restored_document, &catalog_json).unwrap();
    assert_eq!(restored.to_canonical_json().unwrap(), catalog_json);
    let solved = restored
        .solve_document(
            &restored_document,
            SketchSolveRequest::default().without_previous_state_preferences(),
            geosolve_core::SolverConfig::default(),
        )
        .unwrap();
    assert!(solved.solve_result.accepted());
    assert!(solved.audit.iter().all(|audit| {
        audit.equation_templates.len() == audit.rows.len()
            && audit
                .rows
                .iter()
                .all(|row| row.normalized_residual.abs() <= 1.0e-9)
    }));
    assert_eq!(
        solved
            .audit
            .iter()
            .find(|audit| audit.source_id == symmetry)
            .unwrap()
            .rows
            .len(),
        4
    );
}

#[test]
#[allow(clippy::too_many_lines)]
fn m37_rejects_tautological_relations_without_mutating_catalog_or_document() {
    let mut document = SketchDocument::new(1.0).unwrap();
    let a = document.add_point("a", [0.0, 0.0]).unwrap();
    let b = document.add_point("b", [1.0, 0.0]).unwrap();
    let line = document
        .add_curve(
            "line",
            CurveDefinition::Line {
                start: a,
                end: b,
                branch_direction: [1.0, 0.0],
            },
        )
        .unwrap();
    let support = DocumentLineSupportRef {
        span: CurveSpan::line(line),
        direction: DocumentDirectionSense::Forward,
    };
    let c = document.add_point("c", [0.0, 1.0]).unwrap();
    let d = document.add_point("d", [0.0, 2.0]).unwrap();
    let second_line = document
        .add_curve(
            "second line",
            CurveDefinition::Line {
                start: c,
                end: d,
                branch_direction: [0.0, 1.0],
            },
        )
        .unwrap();
    let second_support = DocumentLineSupportRef {
        span: CurveSpan::line(second_line),
        direction: DocumentDirectionSense::Forward,
    };
    let mut catalog = DocumentSemanticSourceCatalog::new(&mut document).unwrap();
    let before_document = document.to_canonical_json().unwrap();
    let before_catalog = catalog.to_canonical_json().unwrap();
    assert!(
        catalog
            .add_planar_source(
                &mut document,
                "invalid collinear",
                DocumentPlanarRelation::Collinear {
                    first: support,
                    second: support,
                },
            )
            .is_err()
    );
    assert_eq!(document.to_canonical_json().unwrap(), before_document);
    assert_eq!(catalog.to_canonical_json().unwrap(), before_catalog);

    assert!(
        catalog
            .add_planar_source(
                &mut document,
                "incomplete entity symmetry",
                DocumentPlanarRelation::EntitySymmetry {
                    first_entity: line,
                    second_entity: second_line,
                    point_pairs: vec![[
                        DocumentPointRef::Point { point: a },
                        DocumentPointRef::Point { point: c },
                    ]],
                    scalar_pairs: vec![],
                    axis: support,
                },
            )
            .is_err()
    );
    assert_eq!(document.to_canonical_json().unwrap(), before_document);
    assert_eq!(catalog.to_canonical_json().unwrap(), before_catalog);

    assert!(
        catalog
            .add_planar_source(
                &mut document,
                "reversed equal distance",
                DocumentPlanarRelation::EqualDistance {
                    first: [
                        DocumentPointRef::Point { point: a },
                        DocumentPointRef::Point { point: b },
                    ],
                    second: [
                        DocumentPointRef::Point { point: b },
                        DocumentPointRef::Point { point: a },
                    ],
                },
            )
            .is_err()
    );
    assert_eq!(document.to_canonical_json().unwrap(), before_document);
    assert_eq!(catalog.to_canonical_json().unwrap(), before_catalog);

    assert!(
        catalog
            .add_planar_source(
                &mut document,
                "reversed equal angle",
                DocumentPlanarRelation::EqualAngle {
                    first: DocumentAngleOperand {
                        first: support,
                        second: second_support,
                        orientation: DocumentAngleOrientation::CounterClockwise,
                        winding: 0,
                    },
                    second: DocumentAngleOperand {
                        first: second_support,
                        second: support,
                        orientation: DocumentAngleOrientation::Clockwise,
                        winding: 0,
                    },
                },
            )
            .is_err()
    );
    assert_eq!(document.to_canonical_json().unwrap(), before_document);
    assert_eq!(catalog.to_canonical_json().unwrap(), before_catalog);
}

#[test]
fn m37_bounded_derived_endpoint_executes_with_source_owned_incidence_rows() {
    let mut document = SketchDocument::new(1.0).unwrap();
    let center = document.add_point("center", [0.0, 0.0]).unwrap();
    let comparison = document.add_point("comparison", [2.0, 0.0]).unwrap();
    let radius = document
        .add_scalar(
            "radius",
            1.0,
            geosolve_sketch::ScalarUnit::Length,
            geosolve_sketch::ScalarDomain::Positive,
        )
        .unwrap();
    let start = document
        .add_scalar(
            "start",
            0.0,
            geosolve_sketch::ScalarUnit::Angle,
            geosolve_sketch::ScalarDomain::Finite,
        )
        .unwrap();
    let end = document
        .add_scalar(
            "end",
            1.0,
            geosolve_sketch::ScalarUnit::Angle,
            geosolve_sketch::ScalarDomain::Finite,
        )
        .unwrap();
    let arc = document
        .add_curve(
            "arc",
            CurveDefinition::CircularArc {
                center,
                radius,
                start_angle: start,
                end_angle: end,
                sweep: geosolve_sketch::DocumentArcSweep::CounterClockwise,
            },
        )
        .unwrap();
    let endpoint = DocumentPointRef::Endpoint(DocumentEndpointRef {
        curve: arc,
        endpoint: FeatureEndpoint::Start,
    });
    let mut catalog = DocumentSemanticSourceCatalog::new(&mut document).unwrap();
    let source = catalog
        .add_planar_source(
            &mut document,
            "derived endpoint horizontal",
            DocumentPlanarRelation::HorizontalPoints {
                first: endpoint,
                second: DocumentPointRef::Point { point: comparison },
            },
        )
        .unwrap();

    let solved = catalog
        .solve_document(
            &document,
            SketchSolveRequest::default().without_previous_state_preferences(),
            geosolve_core::SolverConfig::default(),
        )
        .unwrap();
    assert!(solved.solve_result.accepted());
    let audit = solved
        .audit
        .iter()
        .find(|audit| audit.source_id == source)
        .unwrap();
    assert_eq!(audit.rows.len(), 3);
    assert_eq!(audit.equation_templates.len(), audit.rows.len());
    assert!(
        audit
            .rows
            .iter()
            .all(|row| row.normalized_residual.abs() <= 1.0e-9)
    );
}

#[test]
fn m37_parabola_endpoints_are_distinct_executable_trim_locations() {
    let mut document = SketchDocument::new(1.0).unwrap();
    let vertex = document.add_point("vertex", [0.0, 0.0]).unwrap();
    let focus = document.add_point("focus", [0.0, 1.0]).unwrap();
    let trim_start = document
        .add_scalar(
            "trim start",
            -1.0,
            geosolve_sketch::ScalarUnit::Parameter,
            geosolve_sketch::ScalarDomain::Finite,
        )
        .unwrap();
    let trim_end = document
        .add_scalar(
            "trim end",
            1.0,
            geosolve_sketch::ScalarUnit::Parameter,
            geosolve_sketch::ScalarDomain::Finite,
        )
        .unwrap();
    let parabola = document
        .add_curve(
            "parabola",
            CurveDefinition::ParabolaSegment {
                vertex,
                focus,
                trim_start,
                trim_end,
            },
        )
        .unwrap();
    let mut catalog = DocumentSemanticSourceCatalog::new(&mut document).unwrap();
    let source = catalog
        .add_planar_source(
            &mut document,
            "parabola endpoint level",
            DocumentPlanarRelation::HorizontalPoints {
                first: DocumentPointRef::Endpoint(DocumentEndpointRef {
                    curve: parabola,
                    endpoint: FeatureEndpoint::Start,
                }),
                second: DocumentPointRef::Endpoint(DocumentEndpointRef {
                    curve: parabola,
                    endpoint: FeatureEndpoint::End,
                }),
            },
        )
        .unwrap();
    let solved = catalog
        .solve_document(
            &document,
            SketchSolveRequest::default().without_previous_state_preferences(),
            geosolve_core::SolverConfig::default(),
        )
        .unwrap();
    let audit = solved
        .audit
        .iter()
        .find(|audit| audit.source_id == source)
        .unwrap();
    assert_eq!(audit.rows.len(), 5);
    assert!(
        audit
            .rows
            .iter()
            .all(|row| row.normalized_residual.abs() <= 1.0e-9)
    );
}
