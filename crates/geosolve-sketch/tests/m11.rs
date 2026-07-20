use std::collections::BTreeSet;

use geosolve_core::{DiagnosticStatus, HardValidity, SolverConfig};
use geosolve_sketch::{
    ContactDefinition, ContactDomain, ContactNeighborhood, ContactStateEdit, CurveDefinition,
    CurveSpan, DocumentArcSweep, DocumentArcTangencySide, DocumentCircleContainment,
    DocumentCircleTangencyMode, DocumentCommand, DocumentCommandEffect,
    DocumentConstraintDefinition, DocumentDimensionDefinition, DocumentDimensionMode, DocumentEdit,
    DocumentId, DocumentObjectId, DocumentSolveRequest, PersistentId, ScalarDomain, ScalarUnit,
    SketchDocument, SketchDocumentSession, TangentOrientation,
};

const TOLERANCE: f64 = 1.0e-9;

fn line(
    document: &mut SketchDocument,
    label: &str,
    start: geosolve_sketch::DesignPointId,
    end: geosolve_sketch::DesignPointId,
) -> geosolve_sketch::CurveId {
    let first = document.point(start).unwrap().position;
    let second = document.point(end).unwrap().position;
    let direction = [second[0] - first[0], second[1] - first[1]];
    let norm = direction[0].hypot(direction[1]);
    document
        .add_curve(
            label,
            CurveDefinition::Line {
                start,
                end,
                branch_direction: [direction[0] / norm, direction[1] / norm],
            },
        )
        .unwrap()
}

fn rectangle_document() -> (SketchDocument, geosolve_sketch::RectangleIds) {
    let mut document = SketchDocument::new(6.0).unwrap();
    let ids = document
        .add_rectangle("rectangle", [0.0, 0.0], 4.0, 3.0)
        .unwrap();
    (document, ids)
}

fn session(document: SketchDocument) -> SketchDocumentSession {
    SketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap()
}

fn assert_accepted(result: &geosolve_sketch::DocumentCommandOutcome) {
    assert!(result.accepted(), "{:#?}", result.result.solve().rejection);
    assert_eq!(
        result.result.solve().core_report.hard_validity,
        HardValidity::Valid
    );
    assert!(result.result.solve().core_report.hard_residuals_validated);
    assert!(result.result.solve().core_report.hard_residual_max <= TOLERANCE);
}

#[test]
fn persistent_ids_are_fixed_hex_unique_and_never_reused() {
    assert_ne!(
        SketchDocument::new(1.0).unwrap().id(),
        SketchDocument::new(1.0).unwrap().id()
    );
    let mut document = SketchDocument::new(1.0).unwrap();
    let first = document.add_point("first", [0.0, 0.0]).unwrap();
    let encoded = first.to_string();
    assert_eq!(encoded.len(), 32);
    assert!(
        encoded
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    );
    assert_eq!(encoded.parse::<PersistentId>().unwrap(), first.0);

    document.remove(DocumentObjectId::Point(first)).unwrap();
    let second = document.add_point("second", [1.0, 0.0]).unwrap();
    assert_ne!(first, second);
    assert!(second.0.as_u128() > first.0.as_u128());
    document.remove(DocumentObjectId::Point(second)).unwrap();
    let mut reloaded = SketchDocument::from_json(&document.to_canonical_json().unwrap()).unwrap();
    let third = reloaded.add_point("third", [2.0, 0.0]).unwrap();
    assert_ne!(second, third);

    let mut session = session(SketchDocument::new(1.0).unwrap());
    let created = session
        .apply(DocumentCommand::new(
            session.revision(),
            DocumentEdit::CreatePoint {
                label: "history point".into(),
                position: [0.0, 0.0],
            },
        ))
        .unwrap();
    let Some(DocumentCommandEffect::CreatedPoint(historical)) = created.effect else {
        panic!("point effect expected");
    };
    session.undo(session.revision()).unwrap();
    let replacement = session
        .apply(DocumentCommand::new(
            session.revision(),
            DocumentEdit::CreatePoint {
                label: "replacement".into(),
                position: [1.0, 0.0],
            },
        ))
        .unwrap();
    let Some(DocumentCommandEffect::CreatedPoint(replacement)) = replacement.effect else {
        panic!("point effect expected");
    };
    assert_ne!(historical, replacement);
}

#[test]
fn rectangle_macro_expands_to_ordinary_geometry_and_solves() {
    let (document, ids) = rectangle_document();
    assert_eq!(document.points().len(), 4);
    assert_eq!(document.curves().len(), 4);
    assert_eq!(document.constraints().len(), 5);
    assert_eq!(document.dimensions().len(), 2);
    assert!(
        document
            .curves()
            .iter()
            .all(|curve| matches!(curve.definition, CurveDefinition::Line { .. }))
    );

    let session = session(document);
    let positions: Vec<_> = ids
        .points
        .iter()
        .map(|id| session.document().point(*id).unwrap().position)
        .collect();
    assert_eq!(
        positions,
        vec![[0.0, 0.0], [4.0, 0.0], [4.0, 3.0], [0.0, 3.0]]
    );
    assert!(session.runtime().accepted_result().accepted());
    assert!(
        session
            .runtime()
            .accepted_result()
            .core_report
            .hard_residuals_validated
    );
}

#[test]
fn free_line_drag_preserves_inactive_branch_but_enforced_rectangle_branch_does_not_flip() {
    let mut document = SketchDocument::new(2.0).unwrap();
    let start = document.add_point("start", [0.0, 0.0]).unwrap();
    let end = document.add_point("end", [2.0, 0.0]).unwrap();
    let curve = line(&mut document, "free line", start, end);
    let mut session = session(document);
    let outcome = session
        .apply(DocumentCommand::new(
            session.revision(),
            DocumentEdit::SetPointPosition {
                point: end,
                position: [-2.0, 0.0],
            },
        ))
        .unwrap();
    assert_accepted(&outcome);
    assert_eq!(
        session
            .document()
            .point(end)
            .unwrap()
            .position
            .map(f64::to_bits),
        [-2.0, 0.0].map(f64::to_bits)
    );
    let CurveDefinition::Line {
        branch_direction, ..
    } = &session.document().curve(curve).unwrap().definition
    else {
        panic!("line expected");
    };
    assert_eq!(
        branch_direction.map(f64::to_bits),
        [1.0, 0.0].map(f64::to_bits)
    );
    let json = session.export_json().unwrap();
    assert_eq!(
        SketchDocument::from_json(&json)
            .unwrap()
            .to_canonical_json()
            .unwrap(),
        json
    );

    let (mut rectangle, ids) = rectangle_document();
    let error = rectangle
        .set_point_position(ids.points[1], [-4.0, 0.0])
        .unwrap_err();
    assert!(
        matches!(
            error,
            geosolve_sketch::DocumentError::InvalidField {
                field: "curve.branch_direction",
                ..
            }
        ),
        "{error}"
    );
}

#[test]
fn solver_projected_no_op_point_edit_preserves_history_and_redo() {
    let (document, ids) = rectangle_document();
    let mut session = session(document);
    let edited = session
        .apply(DocumentCommand::new(
            session.revision(),
            DocumentEdit::SetScalarValue {
                scalar: ids.targets[0],
                value: 5.0,
            },
        ))
        .unwrap();
    assert_accepted(&edited);
    session.undo(session.revision()).unwrap();
    let before = session.export_json().unwrap();
    let revision = session.revision();
    let cursor = session.history_cursor();
    let no_op = session
        .apply(DocumentCommand::new(
            revision,
            DocumentEdit::SetPointPosition {
                point: ids.points[0],
                position: [1.0, 1.0],
            },
        ))
        .unwrap();
    assert!(no_op.result.accepted());
    assert!(no_op.effect.is_none());
    assert_eq!(session.revision(), revision);
    assert_eq!(session.history_cursor(), cursor);
    assert_eq!(session.export_json().unwrap(), before);
    assert!(session.can_redo());
}

#[test]
fn batch_delete_cascades_from_selected_rectangle_geometry() {
    let (mut document, ids) = rectangle_document();
    let unrelated = document.add_point("unrelated", [20.0, 20.0]).unwrap();
    let diagonal_target = document
        .add_scalar(
            "diagonal target",
            5.0,
            ScalarUnit::Length,
            ScalarDomain::Positive,
        )
        .unwrap();
    document
        .add_dimension(
            "diagonal reference",
            DocumentDimensionDefinition::PointDistance {
                first: ids.points[0],
                second: ids.points[2],
                target: diagonal_target,
            },
            DocumentDimensionMode::Reference,
        )
        .unwrap();
    let selected = ids
        .points
        .into_iter()
        .map(DocumentObjectId::Point)
        .chain(ids.curves.into_iter().map(DocumentObjectId::Curve))
        .collect::<Vec<_>>();
    document.remove_many_with_dependents(&selected).unwrap();
    assert_eq!(document.points().len(), 1);
    assert!(document.point(unrelated).is_some());
    assert!(document.scalars().is_empty());
    assert!(document.curves().is_empty());
    assert!(document.contacts().is_empty());
    assert!(document.constraints().is_empty());
    assert!(document.dimensions().is_empty());
    assert!(document.source_order().is_empty());
}

#[test]
fn delete_command_removes_private_dimension_target_state() {
    let (document, ids) = rectangle_document();
    let mut session = session(document);
    let deleted = session
        .apply(DocumentCommand::new(
            session.revision(),
            DocumentEdit::Delete {
                object: DocumentObjectId::Dimension(ids.dimensions[0]),
            },
        ))
        .unwrap();
    assert_accepted(&deleted);
    assert!(session.document().dimension(ids.dimensions[0]).is_none());
    assert!(session.document().scalar(ids.targets[0]).is_none());
}

#[test]
#[allow(clippy::too_many_lines)]
fn s1_and_s3_lower_through_the_existing_equation_model() {
    let mut triangle = SketchDocument::new(4.0).unwrap();
    let a = triangle.add_point("A", [0.0, 0.0]).unwrap();
    let b = triangle.add_point("B", [4.0, 0.0]).unwrap();
    let c = triangle.add_point("C", [2.2, 2.0]).unwrap();
    let ab = line(&mut triangle, "AB", a, b);
    triangle
        .add_constraint(
            "A fixed",
            DocumentConstraintDefinition::FixedPoint {
                point: a,
                target: [0.0, 0.0],
            },
        )
        .unwrap();
    triangle
        .add_constraint(
            "AB horizontal",
            DocumentConstraintDefinition::Horizontal {
                line: CurveSpan::line(ab),
            },
        )
        .unwrap();
    let length = triangle
        .add_scalar("AB target", 4.0, ScalarUnit::Length, ScalarDomain::Positive)
        .unwrap();
    triangle
        .add_dimension(
            "AB length",
            DocumentDimensionDefinition::CurveLength {
                curve: CurveSpan::line(ab),
                target: length,
            },
            DocumentDimensionMode::Driving,
        )
        .unwrap();
    let distance = triangle
        .add_scalar("AC target", 3.0, ScalarUnit::Length, ScalarDomain::Positive)
        .unwrap();
    triangle
        .add_dimension(
            "AC distance",
            DocumentDimensionDefinition::PointDistance {
                first: a,
                second: c,
                target: distance,
            },
            DocumentDimensionMode::Driving,
        )
        .unwrap();
    let triangle = session(triangle);
    assert!((triangle.document().point(b).unwrap().position[0] - 4.0).abs() <= TOLERANCE);
    let solved_c = triangle.document().point(c).unwrap().position;
    assert!((solved_c[0].hypot(solved_c[1]) - 3.0).abs() <= TOLERANCE);

    let mut circles = SketchDocument::new(5.0).unwrap();
    let first_center = circles.add_point("O1", [0.0, 0.0]).unwrap();
    let second_center = circles.add_point("O2", [5.0, 0.5]).unwrap();
    let center_line = line(&mut circles, "center line", first_center, second_center);
    let first_radius = circles
        .add_scalar("r1", 2.0, ScalarUnit::Length, ScalarDomain::Positive)
        .unwrap();
    let second_radius = circles
        .add_scalar("r2", 1.0, ScalarUnit::Length, ScalarDomain::Positive)
        .unwrap();
    let first_circle = circles
        .add_curve(
            "circle A",
            CurveDefinition::Circle {
                center: first_center,
                radius: first_radius,
            },
        )
        .unwrap();
    let second_circle = circles
        .add_curve(
            "circle B",
            CurveDefinition::Circle {
                center: second_center,
                radius: second_radius,
            },
        )
        .unwrap();
    circles
        .add_constraint(
            "O1 fixed",
            DocumentConstraintDefinition::FixedPoint {
                point: first_center,
                target: [0.0, 0.0],
            },
        )
        .unwrap();
    circles
        .add_constraint(
            "centers horizontal",
            DocumentConstraintDefinition::Horizontal {
                line: CurveSpan::line(center_line),
            },
        )
        .unwrap();
    for (label, curve, value) in [
        ("radius A", first_circle, 2.0),
        ("radius B", second_circle, 1.0),
    ] {
        let target = circles
            .add_scalar(
                format!("{label} target"),
                value,
                ScalarUnit::Length,
                ScalarDomain::Positive,
            )
            .unwrap();
        circles
            .add_dimension(
                label,
                DocumentDimensionDefinition::Radius { curve, target },
                DocumentDimensionMode::Driving,
            )
            .unwrap();
    }
    let tangency = circles
        .add_constraint(
            "external tangency",
            DocumentConstraintDefinition::CircleCircleTangency {
                first: first_circle,
                second: second_circle,
                mode: DocumentCircleTangencyMode::External,
                center_direction: [1.0, 0.0],
            },
        )
        .unwrap();
    let mut circles = session(circles);
    let solved = circles.document().point(second_center).unwrap().position;
    assert!(
        (solved[0] - 3.0).abs() <= 1.0e-8,
        "unexpected second center {solved:?}"
    );
    assert!(solved[1].abs() <= 1.0e-8);
    let source_id = circles.document().constraint(tangency).unwrap().source_id;
    let switched = circles
        .apply(DocumentCommand::new(
            circles.revision(),
            DocumentEdit::SetCircleTangencyBranch {
                constraint: tangency,
                mode: DocumentCircleTangencyMode::Internal {
                    containment: DocumentCircleContainment::FirstContainsSecond,
                },
                center_direction: [1.0, 0.0],
            },
        ))
        .unwrap();
    assert_accepted(&switched);
    let internal = circles.document().point(second_center).unwrap().position;
    assert!((internal[0] - 1.0).abs() <= 1.0e-8, "{internal:?}");
    assert_eq!(
        circles.document().constraint(tangency).unwrap().source_id,
        source_id
    );
    let internal_json = circles.export_json().unwrap();
    let imported = SketchDocument::from_json(&internal_json).unwrap();
    assert!(matches!(
        imported.constraint(tangency).unwrap().definition,
        DocumentConstraintDefinition::CircleCircleTangency {
            mode: DocumentCircleTangencyMode::Internal {
                containment: DocumentCircleContainment::FirstContainsSecond
            },
            center_direction: [1.0, 0.0],
            ..
        }
    ));
    circles.undo(circles.revision()).unwrap();
    assert!(matches!(
        circles.document().constraint(tangency).unwrap().definition,
        DocumentConstraintDefinition::CircleCircleTangency {
            mode: DocumentCircleTangencyMode::External,
            ..
        }
    ));
    circles.redo(circles.revision()).unwrap();
    assert_eq!(circles.export_json().unwrap(), internal_json);
}

#[test]
#[allow(clippy::too_many_lines)]
fn complete_m5_constraint_and_dimension_set_has_persistent_lowering() {
    let mut document = SketchDocument::new(10.0).unwrap();
    let a = document.add_point("A", [0.0, 0.0]).unwrap();
    let a_alias = document.add_point("A alias", [0.0, 0.0]).unwrap();
    let b = document.add_point("B", [4.0, 0.0]).unwrap();
    let c = document.add_point("C", [0.0, 3.0]).unwrap();
    let d = document.add_point("D", [4.0, 3.0]).unwrap();
    let midpoint = document.add_point("M", [2.0, 0.0]).unwrap();
    let reflected_first = document.add_point("R1", [1.0, 1.0]).unwrap();
    let reflected_second = document.add_point("R2", [1.0, -1.0]).unwrap();
    let ab = line(&mut document, "AB", a, b);
    let cd = line(&mut document, "CD", c, d);
    let ac = line(&mut document, "AC", a, c);
    let polyline = document
        .add_curve(
            "polyline",
            CurveDefinition::Polyline {
                points: vec![a, b, d],
                closed: false,
                branch_directions: vec![[1.0, 0.0], [0.0, 1.0]],
            },
        )
        .unwrap();
    let first_radius = document
        .add_scalar(
            "first radius",
            2.0,
            ScalarUnit::Length,
            ScalarDomain::Positive,
        )
        .unwrap();
    let second_radius = document
        .add_scalar(
            "second radius",
            2.0,
            ScalarUnit::Length,
            ScalarDomain::Positive,
        )
        .unwrap();
    let first_circle = document
        .add_curve(
            "first circle",
            CurveDefinition::Circle {
                center: a,
                radius: first_radius,
            },
        )
        .unwrap();
    let second_circle = document
        .add_curve(
            "second circle",
            CurveDefinition::Circle {
                center: c,
                radius: second_radius,
            },
        )
        .unwrap();
    let arc_radius = document
        .add_scalar(
            "arc radius",
            3.0,
            ScalarUnit::Length,
            ScalarDomain::Positive,
        )
        .unwrap();
    let arc_start = document
        .add_scalar("arc start", 0.0, ScalarUnit::Angle, ScalarDomain::Finite)
        .unwrap();
    let arc_end = document
        .add_scalar(
            "arc end",
            std::f64::consts::PI,
            ScalarUnit::Angle,
            ScalarDomain::Finite,
        )
        .unwrap();
    let arc = document
        .add_curve(
            "arc",
            CurveDefinition::CircularArc {
                center: d,
                radius: arc_radius,
                start_angle: arc_start,
                end_angle: arc_end,
                sweep: DocumentArcSweep::Clockwise,
            },
        )
        .unwrap();

    for (label, definition) in [
        (
            "fixed x",
            DocumentConstraintDefinition::FixedCoordinate {
                point: a,
                axis: geosolve_sketch::DocumentCoordinateAxis::X,
                target: 0.0,
            },
        ),
        (
            "coincident",
            DocumentConstraintDefinition::Coincident {
                first: a,
                second: a_alias,
            },
        ),
        (
            "parallel",
            DocumentConstraintDefinition::Parallel {
                first: CurveSpan::line(ab),
                second: CurveSpan::line(cd),
            },
        ),
        (
            "perpendicular",
            DocumentConstraintDefinition::Perpendicular {
                first: CurveSpan::line(ab),
                second: CurveSpan::line(ac),
            },
        ),
        (
            "equal length",
            DocumentConstraintDefinition::EqualLength {
                first: CurveSpan::line(ab),
                second: CurveSpan::line(cd),
            },
        ),
        (
            "equal radius",
            DocumentConstraintDefinition::EqualRadius {
                first: first_circle,
                second: second_circle,
            },
        ),
        (
            "midpoint",
            DocumentConstraintDefinition::Midpoint {
                point: midpoint,
                line: CurveSpan::line(ab),
            },
        ),
        (
            "symmetry",
            DocumentConstraintDefinition::SymmetricAboutLine {
                first: reflected_first,
                second: reflected_second,
                line: CurveSpan::line(ab),
            },
        ),
        (
            "polyline vertical",
            DocumentConstraintDefinition::Vertical {
                line: CurveSpan {
                    curve: polyline,
                    segment: 1,
                },
            },
        ),
    ] {
        document.add_constraint(label, definition).unwrap();
    }

    let targets = [
        document
            .add_scalar(
                "circle diameter",
                4.0,
                ScalarUnit::Length,
                ScalarDomain::Positive,
            )
            .unwrap(),
        document
            .add_scalar(
                "arc radius target",
                3.0,
                ScalarUnit::Length,
                ScalarDomain::Positive,
            )
            .unwrap(),
        document
            .add_scalar(
                "arc diameter",
                6.0,
                ScalarUnit::Length,
                ScalarDomain::Positive,
            )
            .unwrap(),
        document
            .add_scalar(
                "angle target",
                std::f64::consts::FRAC_PI_2,
                ScalarUnit::Angle,
                ScalarDomain::Positive,
            )
            .unwrap(),
        document
            .add_scalar(
                "reference length",
                4.0,
                ScalarUnit::Length,
                ScalarDomain::Positive,
            )
            .unwrap(),
    ];
    let dimensions = [
        DocumentDimensionDefinition::Diameter {
            curve: first_circle,
            target: targets[0],
        },
        DocumentDimensionDefinition::Radius {
            curve: arc,
            target: targets[1],
        },
        DocumentDimensionDefinition::Diameter {
            curve: arc,
            target: targets[2],
        },
        DocumentDimensionDefinition::OrientedAngle {
            first: CurveSpan::line(ab),
            second: CurveSpan::line(ac),
            target: targets[3],
            orientation: geosolve_sketch::DocumentAngleOrientation::CounterClockwise,
        },
        DocumentDimensionDefinition::CurveLength {
            curve: CurveSpan::line(cd),
            target: targets[4],
        },
    ];
    for (index, definition) in dimensions.into_iter().enumerate() {
        let mode = if index == 4 {
            DocumentDimensionMode::Reference
        } else {
            DocumentDimensionMode::Driving
        };
        document
            .add_dimension(format!("dimension {index}"), definition, mode)
            .unwrap();
    }
    let lowered = document.lower().unwrap();
    assert_eq!(lowered.sketch().constraints().count(), 9);
    assert_eq!(lowered.sketch().dimensions().count(), 5);
    assert!(matches!(
        lowered.mappings().runtime_curve(polyline),
        Some(geosolve_sketch::RuntimeCurve::Polyline(segments)) if segments.len() == 2
    ));
}

#[test]
#[allow(clippy::too_many_lines)]
fn accepted_only_history_round_trips_create_edit_suppress_and_delete() {
    let document = SketchDocument::new(6.0).unwrap();
    let mut session = session(document);
    let rectangle = session
        .apply(DocumentCommand::new(
            session.revision(),
            DocumentEdit::CreateRectangle {
                label: "rectangle".into(),
                origin: [0.0, 0.0],
                width: 4.0,
                height: 3.0,
            },
        ))
        .unwrap();
    assert_accepted(&rectangle);
    let DocumentCommandEffect::CreatedRectangle(ids) = rectangle.effect.unwrap() else {
        panic!("rectangle effect expected");
    };
    let ids = *ids;

    let width = session
        .apply(DocumentCommand::new(
            session.revision(),
            DocumentEdit::SetScalarValue {
                scalar: ids.targets[0],
                value: 6.0,
            },
        ))
        .unwrap();
    assert_accepted(&width);
    let height_source = session
        .document()
        .dimension(ids.dimensions[1])
        .unwrap()
        .source_id;
    let suppress = session
        .apply(DocumentCommand::new(
            session.revision(),
            DocumentEdit::SetSourceSuppressed {
                source: height_source,
                suppressed: true,
            },
        ))
        .unwrap();
    assert_accepted(&suppress);
    let create = session
        .apply(DocumentCommand::new(
            session.revision(),
            DocumentEdit::CreatePoint {
                label: "E".into(),
                position: [9.0, 9.0],
            },
        ))
        .unwrap();
    assert_accepted(&create);
    let Some(DocumentCommandEffect::CreatedPoint(point_e)) = create.effect else {
        panic!("point effect expected");
    };
    let delete = session
        .apply(DocumentCommand::new(
            session.revision(),
            DocumentEdit::Delete {
                object: DocumentObjectId::Point(point_e),
            },
        ))
        .unwrap();
    assert_accepted(&delete);
    assert_eq!(session.history_len(), 5);
    let final_json = session.export_json().unwrap();

    session.undo(session.revision()).unwrap();
    assert!(session.document().point(point_e).is_some());
    let redo_cursor = session.history_cursor();
    let redo_json = session.export_json().unwrap();
    assert!(
        session
            .apply(DocumentCommand::new(
                session.revision(),
                DocumentEdit::SetScalarValue {
                    scalar: ids.targets[0],
                    value: -1.0,
                },
            ))
            .is_err()
    );
    assert_eq!(session.history_cursor(), redo_cursor);
    assert_eq!(session.export_json().unwrap(), redo_json);
    assert!(session.can_redo());
    session.undo(session.revision()).unwrap();
    assert!(session.document().point(point_e).is_none());
    session.undo(session.revision()).unwrap();
    assert!(
        !session
            .document()
            .dimension(ids.dimensions[1])
            .unwrap()
            .suppressed
    );
    session.undo(session.revision()).unwrap();
    assert_eq!(
        session
            .document()
            .scalar(ids.targets[0])
            .unwrap()
            .value
            .to_bits(),
        4.0f64.to_bits()
    );
    session.undo(session.revision()).unwrap();
    assert!(session.document().points().is_empty());

    for _ in 0..5 {
        session.redo(session.revision()).unwrap();
    }
    assert_eq!(session.export_json().unwrap(), final_json);
    assert_eq!(session.history_len(), 5);
    assert_eq!(session.history_cursor(), 5);
}

#[test]
fn conflicting_command_retains_document_and_maps_both_persistent_sources() {
    let (mut document, ids) = rectangle_document();
    let width_five = document
        .add_scalar(
            "width five",
            5.0,
            ScalarUnit::Length,
            ScalarDomain::Positive,
        )
        .unwrap();
    let mut session = session(document);
    let before = session.export_json().unwrap();
    let width_four_source = session
        .document()
        .dimension(ids.dimensions[0])
        .unwrap()
        .source_id;
    let outcome = session
        .apply(DocumentCommand::new(
            session.revision(),
            DocumentEdit::CreateDimension {
                label: "width-5".into(),
                definition: DocumentDimensionDefinition::CurveLength {
                    curve: CurveSpan::line(ids.curves[0]),
                    target: width_five,
                },
                mode: DocumentDimensionMode::Driving,
            },
        ))
        .unwrap();
    assert!(!outcome.accepted());
    assert!(outcome.result.solve().rejection.is_some());
    assert_eq!(session.export_json().unwrap(), before);
    assert_eq!(session.history_len(), 0);
    assert_eq!(session.revision(), 0);
    assert_eq!(
        outcome
            .result
            .solve()
            .core_report
            .conflict_diagnostics
            .status,
        DiagnosticStatus::Complete
    );
    let persistent: BTreeSet<_> = outcome
        .result
        .solve()
        .core_report
        .conflicting_sources
        .iter()
        .filter_map(|source| outcome.result.persistent_core_source(*source))
        .collect();
    let width_five_source = outcome
        .result
        .attempted_mappings()
        .source_mappings()
        .iter()
        .find(|mapping| mapping.label == "width-5")
        .unwrap()
        .source_id;
    assert_eq!(
        persistent,
        BTreeSet::from([width_four_source, width_five_source])
    );
}

#[test]
fn canonical_json_round_trip_preserves_ids_and_branch_state() {
    let (mut document, ids) = rectangle_document();
    let height_source = document.dimension(ids.dimensions[1]).unwrap().source_id;
    document.set_source_suppressed(height_source, true).unwrap();
    let json = document.to_canonical_json().unwrap();
    assert!(!json.contains("runtime"));
    let imported = SketchDocument::from_json(&json).unwrap();
    assert_eq!(imported.to_canonical_json().unwrap(), json);
    assert_eq!(imported, document);

    let first_map = document.lower().unwrap();
    let second_map = imported.lower().unwrap();
    assert_eq!(
        first_map
            .mappings()
            .point_mappings()
            .iter()
            .map(|mapping| mapping.persistent)
            .collect::<Vec<_>>(),
        second_map
            .mappings()
            .point_mappings()
            .iter()
            .map(|mapping| mapping.persistent)
            .collect::<Vec<_>>()
    );
    assert!(imported.dimension(ids.dimensions[1]).unwrap().suppressed);
}

fn bounded_parameter(
    document: &mut SketchDocument,
    label: &str,
    value: f64,
) -> geosolve_sketch::DesignScalarId {
    document
        .add_scalar(
            label,
            value,
            ScalarUnit::Parameter,
            ScalarDomain::Bounded {
                lower: 0.0,
                upper: 1.0,
            },
        )
        .unwrap()
}

fn periodic_parameter(
    document: &mut SketchDocument,
    label: &str,
    value: f64,
) -> geosolve_sketch::DesignScalarId {
    document
        .add_scalar(
            label,
            value,
            ScalarUnit::Angle,
            ScalarDomain::Periodic {
                period: std::f64::consts::TAU,
            },
        )
        .unwrap()
}

#[test]
#[allow(clippy::too_many_lines)]
fn contact_slots_round_trip_and_lower_all_m7_contact_roles() {
    let mut document = SketchDocument::new(30.0).unwrap();
    let line_start = document.add_point("L0", [-5.0, 0.0]).unwrap();
    let line_end = document.add_point("L1", [5.0, 0.0]).unwrap();
    let point_on_line = document.add_point("PL", [0.0, 0.0]).unwrap();
    let circle_center = document.add_point("OC", [0.0, 2.0]).unwrap();
    let point_on_circle = document.add_point("PC", [2.0, 2.0]).unwrap();
    let arc_center = document.add_point("OA", [20.0, 0.0]).unwrap();
    let point_on_arc = document.add_point("PA", [25.0, 0.0]).unwrap();
    let tangent_center = document.add_point("OT", [28.0, 0.0]).unwrap();
    let line = line(&mut document, "line", line_start, line_end);
    let circle_radius = document
        .add_scalar(
            "circle radius",
            2.0,
            ScalarUnit::Length,
            ScalarDomain::Positive,
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
    let arc_radius = document
        .add_scalar(
            "arc radius",
            5.0,
            ScalarUnit::Length,
            ScalarDomain::Positive,
        )
        .unwrap();
    let arc_start = document
        .add_scalar("arc start", 0.0, ScalarUnit::Angle, ScalarDomain::Finite)
        .unwrap();
    let arc_end = document
        .add_scalar(
            "arc end",
            std::f64::consts::PI,
            ScalarUnit::Angle,
            ScalarDomain::Finite,
        )
        .unwrap();
    let arc = document
        .add_curve(
            "arc",
            CurveDefinition::CircularArc {
                center: arc_center,
                radius: arc_radius,
                start_angle: arc_start,
                end_angle: arc_end,
                sweep: DocumentArcSweep::CounterClockwise,
            },
        )
        .unwrap();
    let tangent_radius = document
        .add_scalar(
            "tangent radius",
            3.0,
            ScalarUnit::Length,
            ScalarDomain::Positive,
        )
        .unwrap();
    let tangent_circle = document
        .add_curve(
            "tangent circle",
            CurveDefinition::Circle {
                center: tangent_center,
                radius: tangent_radius,
            },
        )
        .unwrap();

    let point_line_parameter = bounded_parameter(&mut document, "point-line parameter", 0.5);
    let point_line_contact = document
        .add_contact(
            "point-line",
            ContactDefinition {
                curve: CurveSpan::line(line),
                parameter: point_line_parameter,
                domain: ContactDomain::Bounded {
                    lower: 0.0,
                    upper: 1.0,
                },
                winding: 0,
                neighborhood: ContactNeighborhood::Interior,
                tangent_orientation: None,
            },
        )
        .unwrap();
    let point_circle_parameter = periodic_parameter(&mut document, "point-circle angle", 0.0);
    let point_circle_contact = document
        .add_contact(
            "point-circle",
            ContactDefinition {
                curve: CurveSpan::line(circle),
                parameter: point_circle_parameter,
                domain: ContactDomain::Periodic {
                    period: std::f64::consts::TAU,
                },
                winding: 0,
                neighborhood: ContactNeighborhood::Interior,
                tangent_orientation: None,
            },
        )
        .unwrap();
    let point_arc_parameter = bounded_parameter(&mut document, "point-arc parameter", 0.0);
    let point_arc_contact = document
        .add_contact(
            "point-arc",
            ContactDefinition {
                curve: CurveSpan::line(arc),
                parameter: point_arc_parameter,
                domain: ContactDomain::Bounded {
                    lower: 0.0,
                    upper: 1.0,
                },
                winding: 0,
                neighborhood: ContactNeighborhood::Start,
                tangent_orientation: None,
            },
        )
        .unwrap();
    for (label, point, contact) in [
        ("point on line", point_on_line, point_line_contact),
        ("point on circle", point_on_circle, point_circle_contact),
        ("point on arc", point_on_arc, point_arc_contact),
    ] {
        document
            .add_constraint(
                label,
                DocumentConstraintDefinition::PointOnCurve { point, contact },
            )
            .unwrap();
    }

    let tangency_line_parameter = bounded_parameter(&mut document, "tangent line parameter", 0.5);
    let tangency_line = document
        .add_contact(
            "tangency line",
            ContactDefinition {
                curve: CurveSpan::line(line),
                parameter: tangency_line_parameter,
                domain: ContactDomain::Bounded {
                    lower: 0.0,
                    upper: 1.0,
                },
                winding: 0,
                neighborhood: ContactNeighborhood::Interior,
                tangent_orientation: Some(TangentOrientation::Aligned),
            },
        )
        .unwrap();
    let tangency_circle_parameter = periodic_parameter(
        &mut document,
        "tangent circle angle",
        1.5 * std::f64::consts::PI,
    );
    let tangency_circle = document
        .add_contact(
            "tangency circle",
            ContactDefinition {
                curve: CurveSpan::line(circle),
                parameter: tangency_circle_parameter,
                domain: ContactDomain::Periodic {
                    period: std::f64::consts::TAU,
                },
                winding: 0,
                neighborhood: ContactNeighborhood::Interior,
                tangent_orientation: Some(TangentOrientation::Aligned),
            },
        )
        .unwrap();
    document
        .add_constraint(
            "line-circle tangency",
            DocumentConstraintDefinition::LineCircleTangency {
                line_contact: tangency_line,
                circle_contact: tangency_circle,
                side: geosolve_sketch::DocumentLineSide::Left,
            },
        )
        .unwrap();

    let arc_circle_parameter = periodic_parameter(
        &mut document,
        "arc tangency circle angle",
        std::f64::consts::PI,
    );
    let arc_tangency_circle = document
        .add_contact(
            "arc tangency circle",
            ContactDefinition {
                curve: CurveSpan::line(tangent_circle),
                parameter: arc_circle_parameter,
                domain: ContactDomain::Periodic {
                    period: std::f64::consts::TAU,
                },
                winding: 0,
                neighborhood: ContactNeighborhood::Interior,
                tangent_orientation: Some(TangentOrientation::Opposed),
            },
        )
        .unwrap();
    let arc_span_parameter = bounded_parameter(&mut document, "arc tangency span", 0.0);
    let arc_tangency_arc = document
        .add_contact(
            "arc tangency arc",
            ContactDefinition {
                curve: CurveSpan::line(arc),
                parameter: arc_span_parameter,
                domain: ContactDomain::Bounded {
                    lower: 0.0,
                    upper: 1.0,
                },
                winding: 0,
                neighborhood: ContactNeighborhood::Start,
                tangent_orientation: Some(TangentOrientation::Opposed),
            },
        )
        .unwrap();
    document
        .add_constraint(
            "circle-arc tangency",
            DocumentConstraintDefinition::CircleArcTangency {
                circle_contact: arc_tangency_circle,
                arc_contact: arc_tangency_arc,
                side: DocumentArcTangencySide::OutsideArc,
            },
        )
        .unwrap();

    assert!(
        document
            .set_scalar_value(point_arc_parameter, 0.25)
            .is_err()
    );
    let before_unrelated_edit = document.to_canonical_json().unwrap();
    assert!(
        document
            .set_contact_states(&[
                ContactStateEdit {
                    contact: point_line_contact,
                    value: 0.5,
                    winding: 0,
                    neighborhood: ContactNeighborhood::Interior,
                    tangent_orientation: None,
                },
                ContactStateEdit {
                    contact: point_circle_contact,
                    value: 0.0,
                    winding: 0,
                    neighborhood: ContactNeighborhood::Interior,
                    tangent_orientation: None,
                },
            ])
            .is_err()
    );
    assert_eq!(document.to_canonical_json().unwrap(), before_unrelated_edit);

    let mut transitioned = document.clone();
    transitioned
        .set_contact_states(&[ContactStateEdit {
            contact: point_arc_contact,
            value: 0.25,
            winding: 0,
            neighborhood: ContactNeighborhood::Interior,
            tangent_orientation: None,
        }])
        .unwrap();
    transitioned
        .set_contact_states(&[
            ContactStateEdit {
                contact: tangency_line,
                value: 0.5,
                winding: 0,
                neighborhood: ContactNeighborhood::Interior,
                tangent_orientation: Some(TangentOrientation::Opposed),
            },
            ContactStateEdit {
                contact: tangency_circle,
                value: std::f64::consts::FRAC_PI_2,
                winding: 0,
                neighborhood: ContactNeighborhood::Interior,
                tangent_orientation: Some(TangentOrientation::Opposed),
            },
        ])
        .unwrap();
    assert!(SketchDocument::from_json(&transitioned.to_canonical_json().unwrap()).is_ok());

    let json = document.to_canonical_json().unwrap();
    let imported = SketchDocument::from_json(&json).unwrap();
    assert_eq!(imported.to_canonical_json().unwrap(), json);
    let lowered = imported.lower().unwrap();
    assert_eq!(lowered.mappings().contact_mappings().len(), 7);
    assert_eq!(lowered.sketch().constraints().count(), 5);
    let solved = session(imported);
    assert_eq!(solved.document().to_canonical_json().unwrap(), json);
}

#[test]
fn malformed_json_variants_domains_finiteness_and_references_reject() {
    let (document, _) = rectangle_document();
    let json = document.to_canonical_json().unwrap();
    assert!(matches!(
        SketchDocument::from_json(&json.replace("\"version\":3", "\"version\":4")),
        Err(geosolve_sketch::DocumentError::UnsupportedVersion { .. })
    ));
    assert!(
        SketchDocument::from_json(&json.replace("\"model_scale\":6.0", "\"model_scale\":1e999"))
            .is_err()
    );
    assert!(
        SketchDocument::from_json(&json.replacen("\"kind\":\"line\"", "\"kind\":\"future\"", 1))
            .is_err()
    );

    let mut dangling: serde_json::Value = serde_json::from_str(&json).unwrap();
    dangling["curves"][0]["definition"]["start"] =
        serde_json::Value::String("ffffffffffffffffffffffffffffffff".into());
    assert!(SketchDocument::from_json(&serde_json::to_string(&dangling).unwrap()).is_err());

    let mut invalid_domain: serde_json::Value = serde_json::from_str(&json).unwrap();
    invalid_domain["scalars"][0]["value"] = serde_json::json!(-1.0);
    assert!(SketchDocument::from_json(&serde_json::to_string(&invalid_domain).unwrap()).is_err());

    let with_unknown_field = json.replacen("\"version\":3", "\"version\":3,\"future\":true", 1);
    assert!(SketchDocument::from_json(&with_unknown_field).is_err());

    let mut wrong_unit: serde_json::Value = serde_json::from_str(&json).unwrap();
    wrong_unit["scalars"][0]["unit"] = serde_json::Value::String("angle".into());
    assert!(SketchDocument::from_json(&serde_json::to_string(&wrong_unit).unwrap()).is_err());

    let mut shared_scalar: serde_json::Value = serde_json::from_str(&json).unwrap();
    shared_scalar["dimensions"][1]["definition"]["target"] =
        shared_scalar["dimensions"][0]["definition"]["target"].clone();
    assert!(SketchDocument::from_json(&serde_json::to_string(&shared_scalar).unwrap()).is_err());

    let mut overflow_direction: serde_json::Value = serde_json::from_str(&json).unwrap();
    overflow_direction["curves"][0]["definition"]["branch_direction"] =
        serde_json::json!([f64::MAX, f64::MAX]);
    assert!(
        SketchDocument::from_json(&serde_json::to_string(&overflow_direction).unwrap()).is_err()
    );

    let mut shuffled: serde_json::Value = serde_json::from_str(&json).unwrap();
    shuffled["points"].as_array_mut().unwrap().reverse();
    shuffled["curves"].as_array_mut().unwrap().reverse();
    let normalized = SketchDocument::from_json(&serde_json::to_string(&shuffled).unwrap()).unwrap();
    assert_eq!(normalized.to_canonical_json().unwrap(), json);
}

#[test]
#[allow(clippy::too_many_lines)]
fn malformed_imports_and_invalid_edits_leave_session_exactly_unchanged() {
    let (document, ids) = rectangle_document();
    let mut session = session(document);
    let history = session
        .apply(DocumentCommand::new(
            session.revision(),
            DocumentEdit::CreatePoint {
                label: "history marker".into(),
                position: [8.0, 8.0],
            },
        ))
        .unwrap();
    assert_accepted(&history);
    session.undo(session.revision()).unwrap();
    assert!(session.can_redo());
    let before_json = session.export_json().unwrap();
    let before_result = session.runtime().accepted_result().clone();
    let before_revision = session.revision();
    let before_history = (session.history_len(), session.history_cursor());

    assert!(
        session
            .apply(DocumentCommand::new(
                session.revision(),
                DocumentEdit::SetScalarValue {
                    scalar: ids.targets[0],
                    value: -1.0,
                },
            ))
            .is_err()
    );

    let mut malformed: serde_json::Value = serde_json::from_str(&before_json).unwrap();
    let points = malformed["points"].as_array_mut().unwrap();
    points[1]["id"] = points[0]["id"].clone();
    let malformed = serde_json::to_string(&malformed).unwrap();
    assert!(session.import_json(session.revision(), &malformed).is_err());

    let unknown_variant = before_json.replacen("\"kind\":\"line\"", "\"kind\":\"future_curve\"", 1);
    assert!(
        session
            .import_json(session.revision(), &unknown_variant)
            .is_err()
    );

    let mut conflicting = SketchDocument::new(6.0).unwrap();
    let conflicting_ids = conflicting
        .add_rectangle("conflict", [100.0, 100.0], 4.0, 3.0)
        .unwrap();
    let width_five = conflicting
        .add_scalar(
            "conflicting width",
            5.0,
            ScalarUnit::Length,
            ScalarDomain::Positive,
        )
        .unwrap();
    conflicting
        .add_dimension(
            "width-5",
            DocumentDimensionDefinition::CurveLength {
                curve: CurveSpan::line(conflicting_ids.curves[0]),
                target: width_five,
            },
            DocumentDimensionMode::Driving,
        )
        .unwrap();
    let rejected = session
        .import_json(
            session.revision(),
            &conflicting.to_canonical_json().unwrap(),
        )
        .unwrap();
    assert!(!rejected.accepted());
    assert_eq!(
        rejected.result.accepted_view().geometry,
        before_result.geometry
    );
    let accepted_points: Vec<_> = session
        .mappings()
        .point_mappings()
        .iter()
        .map(|mapping| mapping.persistent)
        .collect();
    assert_eq!(
        rejected
            .result
            .mappings()
            .point_mappings()
            .iter()
            .map(|mapping| mapping.persistent)
            .collect::<Vec<_>>(),
        accepted_points
    );
    assert_ne!(
        rejected
            .result
            .attempted_mappings()
            .point_mappings()
            .iter()
            .map(|mapping| mapping.persistent)
            .collect::<Vec<_>>(),
        accepted_points
    );
    assert_eq!(session.export_json().unwrap(), before_json);
    assert_eq!(session.revision(), before_revision);
    assert_eq!(
        (session.history_len(), session.history_cursor()),
        before_history
    );
    assert_eq!(session.runtime().accepted_result(), &before_result);
    assert!(session.can_redo());
}

#[test]
fn cross_namespace_import_undo_redo_preserves_each_document_allocator() {
    let mut first =
        SketchDocument::with_id(1.0, DocumentId(PersistentId::from_u128(0x1000))).unwrap();
    first.add_point("first point", [0.0, 0.0]).unwrap();
    let first_json = first.to_canonical_json().unwrap();
    let mut second =
        SketchDocument::with_id(1.0, DocumentId(PersistentId::from_u128(0x10))).unwrap();
    second.add_point("second point", [1.0, 0.0]).unwrap();
    let second_json = second.to_canonical_json().unwrap();
    let mut session = SketchDocumentSession::new(
        first,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();

    let imported = session
        .import_json(session.revision(), &second_json)
        .unwrap();
    assert!(imported.accepted());
    assert_eq!(session.export_json().unwrap(), second_json);
    session.undo(session.revision()).unwrap();
    assert_eq!(session.export_json().unwrap(), first_json);
    session.redo(session.revision()).unwrap();
    assert_eq!(session.export_json().unwrap(), second_json);
}
