// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_core::{AuditEvaluationStatus, AuditSourceSnapshot, ResidualCategory, SolverConfig};
use geosolve_sketch::{
    CurveDefinition, CurveSpan, DesignPointId, DocumentCommand, DocumentCommandEffect,
    DocumentConstraintDefinition, DocumentConstraintId, DocumentCoordinateAxis,
    DocumentDirectionSense, DocumentEdit, DocumentElementId, DocumentError, DocumentLineSupportRef,
    DocumentObjectId, DocumentSolveRequest, DocumentSourceId, RetainedSketchDocumentSession,
    RuntimeSource, SketchAcceptedDocumentState, SketchConstraintKind, SketchDatum, SketchDocument,
    SketchDocumentSession, SketchSolveRequest, SketchSource,
};

const HARD_TOLERANCE: f64 = 1.0e-9;

fn source_id(document: &SketchDocument, constraint: DocumentConstraintId) -> DocumentSourceId {
    document.constraint(constraint).unwrap().source_id
}

fn runtime_row_count(document: &SketchDocument, constraint: DocumentConstraintId) -> Option<usize> {
    let source = source_id(document, constraint);
    let lowered = document.lower().unwrap();
    let RuntimeSource::Constraint(runtime) = lowered.mappings().runtime_source(source)? else {
        panic!("datum relation must lower to one runtime constraint")
    };
    let compiled = lowered
        .sketch()
        .compile(SketchSolveRequest::default().without_previous_state_preferences())
        .unwrap();
    let mapping = compiled
        .source_mappings()
        .iter()
        .find(|mapping| mapping.source == SketchSource::Constraint(runtime))
        .unwrap();
    Some(
        mapping
            .residual_ids
            .iter()
            .map(|residual| {
                compiled
                    .problem()
                    .residual(*residual)
                    .unwrap()
                    .output_dimension()
            })
            .sum(),
    )
}

fn accepted_session(document: SketchDocument) -> RetainedSketchDocumentSession {
    RetainedSketchDocumentSession::new(
        document,
        DocumentSolveRequest::default().without_previous_state_preferences(),
        SolverConfig::default(),
    )
    .unwrap()
}

fn accepted_audit(
    accepted: &SketchAcceptedDocumentState,
    constraint: DocumentConstraintId,
) -> &AuditSourceSnapshot {
    let source = source_id(accepted.document(), constraint);
    let RuntimeSource::Constraint(runtime) = accepted.mappings().runtime_source(source).unwrap()
    else {
        panic!("datum relation must retain a runtime constraint mapping")
    };
    let core_source = accepted
        .solve_result()
        .source_mappings
        .iter()
        .find(|mapping| mapping.source == SketchSource::Constraint(runtime))
        .and_then(|mapping| mapping.core_source_id)
        .unwrap();
    accepted
        .solve_result()
        .display_audit
        .sources
        .iter()
        .find(|audit| audit.source_id == core_source)
        .unwrap()
}

fn assert_finite_hard_audit(audit: &AuditSourceSnapshot, expected_rows: usize) {
    let hard_rows = audit
        .rows
        .iter()
        .filter(|row| row.category == ResidualCategory::Hard)
        .collect::<Vec<_>>();
    assert_eq!(hard_rows.len(), expected_rows);
    assert!(hard_rows.iter().all(|row| {
        row.category == ResidualCategory::Hard
            && row.evaluation_status == AuditEvaluationStatus::Evaluated
            && row.scale.is_finite()
            && row.scale > 0.0
            && row.raw_residual.is_finite()
            && row.normalized_residual.is_finite()
            && row.normalized_residual.abs() <= HARD_TOLERANCE
    }));
}

fn line(
    document: &mut SketchDocument,
    label: &str,
    start: DesignPointId,
    end: DesignPointId,
) -> geosolve_sketch::CurveId {
    let start_position = document.point(start).unwrap().position;
    let end_position = document.point(end).unwrap().position;
    let delta = [
        end_position[0] - start_position[0],
        end_position[1] - start_position[1],
    ];
    let length = delta[0].hypot(delta[1]);
    document
        .add_curve(
            label,
            CurveDefinition::Line {
                start,
                end,
                branch_direction: [delta[0] / length, delta[1] / length],
            },
        )
        .unwrap()
}

fn datum_line_definition(
    curve: geosolve_sketch::CurveId,
    axis: DocumentCoordinateAxis,
) -> DocumentConstraintDefinition {
    DocumentConstraintDefinition::CollinearWithDatumAxis {
        line: DocumentLineSupportRef {
            span: CurveSpan::line(curve),
            direction: DocumentDirectionSense::Forward,
        },
        axis,
    }
}

fn assert_datum_line_audit(
    accepted: &SketchAcceptedDocumentState,
    constraint: DocumentConstraintId,
    axis: DocumentCoordinateAxis,
    scale: f64,
    direction_angle: f64,
    support_normal: f64,
) {
    let audit = accepted_audit(accepted, constraint);
    assert_eq!(audit.rows.len(), 3);
    assert_finite_hard_audit(audit, 2);
    assert!((audit.rows[0].normalized_residual - direction_angle).abs() <= f64::EPSILON * 8.0);
    assert!((audit.rows[1].normalized_residual - support_normal).abs() <= f64::EPSILON * 8.0);
    let axis_name = match axis {
        DocumentCoordinateAxis::X => "X axis",
        DocumentCoordinateAxis::Y => "Y axis",
    };
    assert!(audit.source_label.contains(axis_name));
    assert!(audit.rows[..2].iter().all(|row| {
        row.bindings
            .iter()
            .any(|binding| binding.name == "datum axis" && binding.value == axis_name)
            && row.template.contains("datum_axis")
    }));
    assert_eq!(audit.rows[0].unit, "radian");
    assert_eq!(audit.rows[0].scale.to_bits(), 1.0_f64.to_bits());
    assert_eq!(audit.rows[1].unit, "model-unit");
    assert_eq!(audit.rows[1].scale.to_bits(), scale.to_bits());
    assert_eq!(audit.rows[2].category, ResidualCategory::Preference);
    assert_eq!(audit.rows[2].unit, "model-unit");
    assert_eq!(audit.rows[2].scale.to_bits(), scale.to_bits());
    assert!(audit.rows[2].normalized_residual.is_finite());
    assert!(
        audit.rows[2]
            .bindings
            .iter()
            .any(|binding| binding.name == "preference"
                && binding.value == "retain non-degenerate line length")
    );
    assert_eq!(
        accepted.diagnostics().rank.unwrap().numerical_right_nullity,
        Some(2)
    );
}

fn assert_datum_line_dependency_lifecycle() {
    let mut document = SketchDocument::new(1.0).unwrap();
    let start = document.add_point("start", [1.0, 2.0]).unwrap();
    let end = document.add_point("end", [4.0, 3.0]).unwrap();
    let curve = line(&mut document, "datum line", start, end);
    let constraint = document
        .add_constraint(
            "line on Y axis",
            datum_line_definition(curve, DocumentCoordinateAxis::Y),
        )
        .unwrap();
    let mut conservative = document.clone();
    assert!(matches!(
        conservative.remove(DocumentObjectId::Curve(curve)),
        Err(DocumentError::ObjectInUse(_))
    ));
    let mut cascade = document.clone();
    cascade
        .remove_many_with_dependents(&[DocumentObjectId::Curve(curve)])
        .unwrap();
    assert!(cascade.curve(curve).is_none());
    assert!(cascade.constraint(constraint).is_none());
    assert!(cascade.point(start).is_some() && cascade.point(end).is_some());

    assert!(
        document
            .dependency_closure(constraint)
            .contains(&DocumentElementId::Curve(curve))
    );
    document
        .remove_with_owned_state(DocumentObjectId::Constraint(constraint))
        .unwrap();
    assert!(document.constraint(constraint).is_none());
    assert!(document.curve(curve).is_some());
    accepted_session(document);
}

fn assert_datum_draft_wire(draft: &str) {
    let value: serde_json::Value = serde_json::from_str(draft).unwrap();
    let retained = value["retained_planar_constraints"].as_array().unwrap();
    assert_eq!(retained.len(), 5);
    let definitions = retained
        .iter()
        .map(|record| &record["definition"])
        .collect::<Vec<_>>();
    assert!(definitions.iter().any(|definition| {
        definition["kind"] == "coincident_with_origin" && definition["point"].is_string()
    }));
    for (kind, axis) in [
        ("point_on_datum_axis", "x"),
        ("point_on_datum_axis", "y"),
        ("collinear_with_datum_axis", "x"),
        ("collinear_with_datum_axis", "y"),
    ] {
        assert!(
            definitions
                .iter()
                .any(|definition| { definition["kind"] == kind && definition["axis"] == axis })
        );
    }
    assert!(
        value["document"]["constraints"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn intrinsic_datums_reject_origin_as_an_axis_only_operand() {
    assert_eq!(SketchDatum::Origin.coordinate_axis(), None);
    assert_eq!(
        SketchDatum::XAxis.coordinate_axis(),
        Some(DocumentCoordinateAxis::X)
    );
    assert_eq!(
        SketchDatum::YAxis.coordinate_axis(),
        Some(DocumentCoordinateAxis::Y)
    );
}

#[test]
#[allow(clippy::too_many_lines)]
fn datum_point_relations_lower_exactly_and_solve_with_finite_audit_at_all_scales() {
    for scale in [1.0e-6, 1.0, 1.0e6] {
        let mut origin_document = SketchDocument::new(scale).unwrap();
        let origin_point = origin_document
            .add_point("origin candidate", [2.25 * scale, -3.5 * scale])
            .unwrap();
        let origin_constraint = origin_document
            .add_constraint(
                "coincident with origin",
                DocumentConstraintDefinition::CoincidentWithOrigin {
                    point: origin_point,
                },
            )
            .unwrap();
        assert_eq!(
            runtime_row_count(&origin_document, origin_constraint),
            Some(2)
        );
        let origin_lowered = origin_document.lower().unwrap();
        let origin_source = source_id(&origin_document, origin_constraint);
        let RuntimeSource::Constraint(origin_runtime) = origin_lowered
            .mappings()
            .runtime_source(origin_source)
            .unwrap()
        else {
            panic!("origin relation must lower to a runtime constraint")
        };
        let SketchConstraintKind::CoincidentWithOrigin { point } = origin_lowered
            .sketch()
            .constraint(origin_runtime)
            .unwrap()
            .kind()
        else {
            panic!("origin relation must retain datum-specific runtime semantics")
        };
        assert_eq!(
            point,
            origin_lowered
                .mappings()
                .runtime_point(origin_point)
                .unwrap()
        );

        let origin_session = accepted_session(origin_document);
        let origin_accepted = origin_session.accepted_state().unwrap();
        let solved_origin = origin_accepted
            .solve_result()
            .geometry
            .point(
                origin_accepted
                    .mappings()
                    .runtime_point(origin_point)
                    .unwrap(),
            )
            .unwrap();
        assert!(solved_origin.x.abs() / scale <= HARD_TOLERANCE);
        assert!(solved_origin.y.abs() / scale <= HARD_TOLERANCE);
        let origin_audit = accepted_audit(origin_accepted, origin_constraint);
        assert_finite_hard_audit(origin_audit, 2);
        assert!(origin_audit.rows.iter().all(|row| {
            row.bindings
                .iter()
                .any(|binding| binding.name == "point" && binding.value == "origin candidate")
                && row
                    .bindings
                    .iter()
                    .any(|binding| binding.name == "datum" && binding.value == "Origin")
        }));
        assert_eq!(
            origin_accepted
                .diagnostics()
                .rank
                .unwrap()
                .numerical_right_nullity,
            Some(0)
        );

        for axis in [DocumentCoordinateAxis::X, DocumentCoordinateAxis::Y] {
            let mut document = SketchDocument::new(scale).unwrap();
            let point = document
                .add_point("datum-axis point", [2.5 * scale, -4.25 * scale])
                .unwrap();
            let constraint = document
                .add_constraint(
                    "point on datum axis",
                    DocumentConstraintDefinition::PointOnDatumAxis { point, axis },
                )
                .unwrap();
            assert_eq!(runtime_row_count(&document, constraint), Some(1));

            let lowered = document.lower().unwrap();
            let source = source_id(&document, constraint);
            let RuntimeSource::Constraint(runtime) =
                lowered.mappings().runtime_source(source).unwrap()
            else {
                panic!("point-axis relation must lower to a runtime constraint")
            };
            let SketchConstraintKind::PointOnDatumAxis {
                point: runtime_point,
                axis: runtime_axis,
            } = lowered.sketch().constraint(runtime).unwrap().kind()
            else {
                panic!("point-axis relation must retain datum-specific runtime semantics")
            };
            assert_eq!(
                runtime_point,
                lowered.mappings().runtime_point(point).unwrap()
            );
            assert_eq!(runtime_axis, axis);

            let session = accepted_session(document);
            let accepted = session.accepted_state().unwrap();
            let solved = accepted
                .solve_result()
                .geometry
                .point(accepted.mappings().runtime_point(point).unwrap())
                .unwrap();
            let constrained_value = match axis {
                DocumentCoordinateAxis::X => solved.y,
                DocumentCoordinateAxis::Y => solved.x,
            };
            assert!(constrained_value.abs() / scale <= HARD_TOLERANCE);
            let audit = accepted_audit(accepted, constraint);
            assert_finite_hard_audit(audit, 1);
            let expected_datum_name = match axis {
                DocumentCoordinateAxis::X => "X axis",
                DocumentCoordinateAxis::Y => "Y axis",
            };
            assert!(audit.rows[0].bindings.iter().any(|binding| {
                binding.name == "datum axis" && binding.value == expected_datum_name
            }));
            assert!(
                audit.rows[0]
                    .bindings
                    .iter()
                    .any(|binding| binding.name == "target" && binding.value == "0")
            );
            assert_eq!(
                accepted.diagnostics().rank.unwrap().numerical_right_nullity,
                Some(1)
            );
        }
    }
}

#[test]
fn datum_line_relations_use_oriented_support_math_with_checked_jacobians() {
    for scale in [1.0e-6, 1.0, 1.0e6] {
        for axis in [DocumentCoordinateAxis::X, DocumentCoordinateAxis::Y] {
            let mut document = SketchDocument::new(scale).unwrap();
            let start = document
                .add_point("datum-line start", [1.25 * scale, 2.5 * scale])
                .unwrap();
            let end = document
                .add_point("datum-line end", [4.75 * scale, 5.25 * scale])
                .unwrap();
            let curve = line(&mut document, "datum line", start, end);
            let constraint = document
                .add_constraint("line on datum axis", datum_line_definition(curve, axis))
                .unwrap();
            assert_eq!(runtime_row_count(&document, constraint), Some(3));

            let lowered = document.lower().unwrap();
            let source = source_id(&document, constraint);
            let RuntimeSource::Constraint(runtime) =
                lowered.mappings().runtime_source(source).unwrap()
            else {
                panic!("datum-line relation must lower to a runtime constraint")
            };
            let SketchConstraintKind::DatumLineCollinear {
                segment: _,
                axis: lowered_axis,
            } = lowered.sketch().constraint(runtime).unwrap().kind()
            else {
                panic!("datum-line relation must lower to datum collinearity math")
            };
            assert_eq!(lowered_axis, axis);
            let compiled = lowered
                .sketch()
                .compile(SketchSolveRequest::default().without_previous_state_preferences())
                .unwrap();
            let jacobians = compiled.problem().check_jacobians(1.0e-6).unwrap();
            assert!(
                jacobians.all_within(1.0e-6),
                "{axis:?}, {scale:e}: {jacobians:#?}"
            );

            let session = accepted_session(document);
            let accepted = session.accepted_state().unwrap();
            let solved_start = accepted
                .solve_result()
                .geometry
                .point(accepted.mappings().runtime_point(start).unwrap())
                .unwrap();
            let solved_end = accepted
                .solve_result()
                .geometry
                .point(accepted.mappings().runtime_point(end).unwrap())
                .unwrap();
            let branch = accepted
                .document()
                .curve_branch_direction(CurveSpan::line(curve))
                .unwrap();
            let axis_direction = match axis {
                DocumentCoordinateAxis::X => [1.0, 0.0],
                DocumentCoordinateAxis::Y => [0.0, 1.0],
            };
            let projection = branch[0].mul_add(axis_direction[0], branch[1] * axis_direction[1]);
            let branch_cross = branch[0].mul_add(axis_direction[1], -branch[1] * axis_direction[0]);
            let sign = if projection > 0.0 || (projection == 0.0 && branch_cross >= 0.0) {
                1.0
            } else {
                -1.0
            };
            let datum_direction = [sign * axis_direction[0], sign * axis_direction[1]];
            let delta = solved_end - solved_start;
            let unit = delta / delta.norm();
            let direction_cross = datum_direction[0].mul_add(unit.y, -datum_direction[1] * unit.x);
            let direction_dot = datum_direction[0].mul_add(unit.x, datum_direction[1] * unit.y);
            let direction_angle = direction_cross.atan2(direction_dot);
            let support_normal = match axis {
                DocumentCoordinateAxis::X => solved_start.y / scale,
                DocumentCoordinateAxis::Y => solved_start.x / scale,
            };
            assert!(direction_angle.abs() <= HARD_TOLERANCE);
            assert!(support_normal.abs() <= HARD_TOLERANCE);
            assert_datum_line_audit(
                accepted,
                constraint,
                axis,
                scale,
                direction_angle,
                support_normal,
            );
        }
    }
}

#[test]
fn datum_line_collinearity_escapes_an_exact_perpendicular_reversed_seed() {
    let mut document = SketchDocument::new(1.0).unwrap();
    let first = document.add_point("left", [-2.0, 0.0]).unwrap();
    let second = document.add_point("right", [2.0, 0.0]).unwrap();
    let curve = line(&mut document, "reversed horizontal", second, first);
    let constraint = document
        .add_constraint(
            "reversed horizontal on Y axis",
            datum_line_definition(curve, DocumentCoordinateAxis::Y),
        )
        .unwrap();

    let lowered = document.lower().unwrap();
    let compiled = lowered
        .sketch()
        .compile(SketchSolveRequest::default().without_previous_state_preferences())
        .unwrap();
    assert!(
        compiled
            .problem()
            .check_jacobians(1.0e-6)
            .unwrap()
            .all_within(1.0e-6)
    );

    let session = accepted_session(document);
    let accepted = session.accepted_state().unwrap();
    assert_finite_hard_audit(accepted_audit(accepted, constraint), 2);
    let start = accepted
        .solve_result()
        .geometry
        .point(accepted.mappings().runtime_point(second).unwrap())
        .unwrap();
    let end = accepted
        .solve_result()
        .geometry
        .point(accepted.mappings().runtime_point(first).unwrap())
        .unwrap();
    assert!(start.x.abs() <= HARD_TOLERANCE);
    assert!(end.x.abs() <= HARD_TOLERANCE);
    assert!((end - start).norm() > 1.0);
}

#[test]
fn datum_relations_follow_suppression_dependency_delete_and_history_lifecycle() {
    let mut document = SketchDocument::new(1.0).unwrap();
    let point = document.add_point("axis point", [2.0, 3.0]).unwrap();
    let mut session = SketchDocumentSession::new(
        document,
        DocumentSolveRequest::default().without_previous_state_preferences(),
        SolverConfig::default(),
    )
    .unwrap();
    let created = session
        .apply(DocumentCommand::new(
            session.revision(),
            DocumentEdit::CreateConstraint {
                label: "point on X axis".into(),
                definition: DocumentConstraintDefinition::PointOnDatumAxis {
                    point,
                    axis: DocumentCoordinateAxis::X,
                },
            },
        ))
        .unwrap();
    let Some(DocumentCommandEffect::CreatedConstraint(constraint)) = created.effect else {
        panic!("created datum relation effect expected")
    };
    let source = source_id(session.document(), constraint);
    assert_eq!(runtime_row_count(session.document(), constraint), Some(1));

    session
        .apply(DocumentCommand::new(
            session.revision(),
            DocumentEdit::SetSourceSuppressed {
                source,
                suppressed: true,
            },
        ))
        .unwrap();
    assert!(session.document().source(source).unwrap().suppressed);
    assert_eq!(runtime_row_count(session.document(), constraint), None);
    session.undo(session.revision()).unwrap();
    assert!(!session.document().source(source).unwrap().suppressed);
    assert_eq!(runtime_row_count(session.document(), constraint), Some(1));
    session.redo(session.revision()).unwrap();
    assert!(session.document().source(source).unwrap().suppressed);
    session.undo(session.revision()).unwrap();

    session
        .apply(DocumentCommand::new(
            session.revision(),
            DocumentEdit::Delete {
                object: DocumentObjectId::Constraint(constraint),
            },
        ))
        .unwrap();
    assert!(session.document().constraint(constraint).is_none());
    assert!(session.document().point(point).is_some());
    session.undo(session.revision()).unwrap();
    assert_eq!(source_id(session.document(), constraint), source);
    assert_eq!(runtime_row_count(session.document(), constraint), Some(1));
    session.redo(session.revision()).unwrap();
    assert!(session.document().constraint(constraint).is_none());
    session.undo(session.revision()).unwrap();

    let mut conservative = session.document().clone();
    assert!(matches!(
        conservative.remove(DocumentObjectId::Point(point)),
        Err(DocumentError::ObjectInUse(_))
    ));
    let mut cascade = session.document().clone();
    cascade
        .remove_many_with_dependents(&[DocumentObjectId::Point(point)])
        .unwrap();
    assert!(cascade.point(point).is_none());
    assert!(cascade.constraint(constraint).is_none());

    assert_datum_line_dependency_lifecycle();
}

#[test]
fn datum_relations_round_trip_exactly_only_in_draft_v5() {
    let mut document = SketchDocument::new(2.0).unwrap();
    let origin = document.add_point("origin point", [1.0, -1.0]).unwrap();
    let x_point = document.add_point("X-axis point", [2.0, 3.0]).unwrap();
    let y_point = document.add_point("Y-axis point", [-4.0, 5.0]).unwrap();
    let horizontal_end = document.add_point("horizontal end", [6.0, 7.0]).unwrap();
    let vertical_end = document.add_point("vertical end", [8.0, 9.0]).unwrap();
    let horizontal = line(
        &mut document,
        "horizontal datum line",
        x_point,
        horizontal_end,
    );
    let vertical = line(&mut document, "vertical datum line", y_point, vertical_end);

    let origin_constraint = document
        .add_constraint(
            "origin relation",
            DocumentConstraintDefinition::CoincidentWithOrigin { point: origin },
        )
        .unwrap();
    let x_constraint = document
        .add_constraint(
            "X-axis point relation",
            DocumentConstraintDefinition::PointOnDatumAxis {
                point: x_point,
                axis: DocumentCoordinateAxis::X,
            },
        )
        .unwrap();
    document
        .add_constraint(
            "Y-axis point relation",
            DocumentConstraintDefinition::PointOnDatumAxis {
                point: y_point,
                axis: DocumentCoordinateAxis::Y,
            },
        )
        .unwrap();
    document
        .add_constraint(
            "X-axis line relation",
            datum_line_definition(horizontal, DocumentCoordinateAxis::X),
        )
        .unwrap();
    document
        .add_constraint(
            "Y-axis line relation",
            datum_line_definition(vertical, DocumentCoordinateAxis::Y),
        )
        .unwrap();
    document
        .set_element_user_suppressed(DocumentElementId::Constraint(x_constraint), true)
        .unwrap();

    assert!(matches!(
        document.to_canonical_json(),
        Err(DocumentError::UnsupportedM74State)
    ));
    let draft = document.to_draft_v5_json().unwrap();
    assert_datum_draft_wire(&draft);

    let restored = SketchDocument::from_draft_v5_json(&draft).unwrap();
    assert_eq!(restored.constraints(), document.constraints());
    assert_eq!(restored.source_order(), document.source_order());
    assert_eq!(restored.to_draft_v5_json().unwrap(), draft);
    assert!(
        restored
            .source(source_id(&restored, origin_constraint))
            .is_some()
    );
    assert!(
        restored
            .source(source_id(&restored, x_constraint))
            .unwrap()
            .suppressed
    );
    assert!(matches!(
        restored.to_canonical_json(),
        Err(DocumentError::UnsupportedM74State)
    ));
}
