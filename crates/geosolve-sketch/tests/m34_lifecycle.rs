use geosolve_core::{
    HardValidity, OperationControl, OperationOutcome, ResidualCategory, SolverConfig,
};
use geosolve_sketch::{
    CurveDefinition, CurveSpan, DesignPointId, DocumentCommandEffect, DocumentConstraintDefinition,
    DocumentCurveBranchEdit, DocumentDimensionDefinition, DocumentDimensionMode, DocumentEdit,
    DocumentSessionError, DocumentSolveRequest, RetainedSketchDocumentSession, ScalarDomain,
    ScalarUnit, SketchDocument, SketchSessionExecutionKind, SketchSource,
};

fn rectangle_design() -> (SketchDocument, geosolve_sketch::RectangleIds) {
    let mut document = SketchDocument::new(6.0).unwrap();
    let rectangle = document
        .add_rectangle("rectangle", [0.0, 0.0], 4.0, 3.0)
        .unwrap();
    (document, rectangle)
}

fn add_width_five_target(document: &mut SketchDocument) -> geosolve_sketch::DesignScalarId {
    document
        .add_scalar(
            "width five",
            5.0,
            ScalarUnit::Length,
            ScalarDomain::Positive,
        )
        .unwrap()
}

fn add_width_five_dimension(
    document: &mut SketchDocument,
    curve: geosolve_sketch::CurveId,
    target: geosolve_sketch::DesignScalarId,
) -> geosolve_sketch::DocumentDimensionId {
    document
        .add_dimension(
            "width-5",
            DocumentDimensionDefinition::CurveLength {
                curve: CurveSpan::line(curve),
                target,
            },
            DocumentDimensionMode::Driving,
        )
        .unwrap()
}

fn duplicate_distance_design() -> (
    SketchDocument,
    geosolve_sketch::DesignScalarId,
    geosolve_sketch::DocumentSourceId,
    geosolve_sketch::DocumentSourceId,
) {
    let mut document = SketchDocument::new(4.0).unwrap();
    let first = document.add_point("first", [0.0, 0.0]).unwrap();
    let second = document.add_point("second", [2.0, 0.0]).unwrap();
    document
        .add_constraint(
            "fix first",
            DocumentConstraintDefinition::FixedPoint {
                point: first,
                target: [0.0, 0.0],
            },
        )
        .unwrap();
    let first_target = document
        .add_scalar(
            "first distance two",
            2.0,
            ScalarUnit::Length,
            ScalarDomain::Positive,
        )
        .unwrap();
    let duplicate_target = document
        .add_scalar(
            "duplicate distance two",
            2.0,
            ScalarUnit::Length,
            ScalarDomain::Positive,
        )
        .unwrap();
    let add_distance =
        |document: &mut SketchDocument, label: &str, target: geosolve_sketch::DesignScalarId| {
            let dimension = document
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
            document.dimension(dimension).unwrap().source_id
        };
    let first_distance = add_distance(&mut document, "first distance", first_target);
    let duplicate_distance = add_distance(&mut document, "duplicate distance", duplicate_target);
    (document, first_target, first_distance, duplicate_distance)
}

fn two_link_session() -> (
    RetainedSketchDocumentSession,
    [DesignPointId; 3],
    [geosolve_sketch::CurveId; 2],
) {
    let mut document = SketchDocument::new(1.0).unwrap();
    let base = document.add_point("base", [0.0, 0.0]).unwrap();
    let elbow = document.add_point("elbow", [1.0, 1.0]).unwrap();
    let end = document.add_point("end", [2.0, 0.0]).unwrap();
    let diagonal = 0.5_f64.sqrt();
    let first_link = document
        .add_curve(
            "first link",
            CurveDefinition::Line {
                start: base,
                end: elbow,
                branch_direction: [diagonal, diagonal],
            },
        )
        .unwrap();
    let second_link = document
        .add_curve(
            "second link",
            CurveDefinition::Line {
                start: elbow,
                end,
                branch_direction: [0.0, -1.0],
            },
        )
        .unwrap();
    document
        .add_constraint(
            "fixed base",
            DocumentConstraintDefinition::FixedPoint {
                point: base,
                target: [0.0, 0.0],
            },
        )
        .unwrap();
    for (label, first, second) in [("first length", base, elbow), ("second length", elbow, end)] {
        let target = document
            .add_scalar(
                label,
                2.0_f64.sqrt(),
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
    (
        RetainedSketchDocumentSession::new(
            document,
            DocumentSolveRequest::default(),
            SolverConfig::default(),
        )
        .unwrap(),
        [base, elbow, end],
        [first_link, second_link],
    )
}

fn accepted_two_link_preview(
    session: &RetainedSketchDocumentSession,
    end: DesignPointId,
) -> RetainedSketchDocumentSession {
    let mut preview = session.clone();
    preview
        .reattempt(
            preview.design_identity(),
            DocumentSolveRequest::default()
                .without_previous_state_preferences()
                .with_drag(end, [0.0, 0.0]),
        )
        .unwrap();
    assert!(preview.last_attempt().accepted_state_identity().is_some());
    preview
}

#[test]
fn accepted_redundancy_is_persistent_provenance_and_survives_rejected_attempt() {
    let (document, first_target, first_distance, duplicate_distance) = duplicate_distance_design();
    let mut session = RetainedSketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let accepted = session.accepted_state().unwrap();
    let redundancy = accepted.accepted_redundancy().clone();
    assert_eq!(redundancy.accepted_state_identity(), accepted.identity());
    assert_eq!(redundancy.design_identity(), accepted.design_identity());
    assert_eq!(redundancy.fully_redundant_sources(), [duplicate_distance]);
    assert_eq!(
        redundancy.sources_containing_redundant_rows(),
        [duplicate_distance]
    );
    assert!(
        !redundancy
            .fully_redundant_sources()
            .contains(&first_distance)
    );

    session
        .apply(
            session.design_identity(),
            DocumentEdit::SetScalarValue {
                scalar: first_target,
                value: 3.0,
            },
        )
        .unwrap();
    assert!(
        session
            .last_attempt()
            .solve_result()
            .unwrap()
            .accepted_redundancy()
            .is_none()
    );
    assert_eq!(
        session.accepted_state().unwrap().accepted_redundancy(),
        &redundancy
    );
}

#[test]
fn accepted_nonredundant_state_publishes_empty_source_sets() {
    let mut document = SketchDocument::new(1.0).unwrap();
    document.add_point("free", [0.0, 0.0]).unwrap();
    let session = RetainedSketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let accepted = session.accepted_state().unwrap();
    let redundancy = accepted.accepted_redundancy();
    assert_eq!(redundancy.accepted_state_identity(), accepted.identity());
    assert_eq!(redundancy.design_identity(), accepted.design_identity());
    assert!(redundancy.fully_redundant_sources().is_empty());
    assert!(redundancy.sources_containing_redundant_rows().is_empty());
}

#[test]
fn initial_conflict_has_design_and_attempt_but_no_accepted_state() {
    let (mut document, rectangle) = rectangle_design();
    let target = add_width_five_target(&mut document);
    add_width_five_dimension(&mut document, rectangle.curves[0], target);

    let session = RetainedSketchDocumentSession::new(
        document.clone(),
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();

    assert_eq!(session.design_document(), &document);
    assert_eq!(session.design_identity().revision().get(), 0);
    assert_eq!(session.last_attempt().identity().revision().get(), 0);
    assert_eq!(
        session.last_attempt().design_identity(),
        session.design_identity()
    );
    assert!(session.last_attempt().parent_accepted_identity().is_none());
    assert!(session.last_attempt().accepted_state_identity().is_none());
    assert!(session.accepted_state().is_none());
    let solve = session.last_attempt().solve_result().unwrap();
    assert!(!solve.accepted());
    assert!(session.last_attempt().attempted_geometry().is_some());
}

#[test]
fn conflicting_edit_is_retained_and_an_ordinary_repair_is_accepted() {
    let (mut document, rectangle) = rectangle_design();
    let target = add_width_five_target(&mut document);
    let mut session = RetainedSketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let accepted_zero = session.accepted_state().unwrap().identity();
    let accepted_zero_json = session.export_accepted_json().unwrap().unwrap();
    assert_eq!(accepted_zero.revision().get(), 0);

    let conflict = session
        .apply(
            session.design_identity(),
            DocumentEdit::CreateDimension {
                label: "width-5".into(),
                definition: DocumentDimensionDefinition::CurveLength {
                    curve: CurveSpan::line(rectangle.curves[0]),
                    target,
                },
                mode: DocumentDimensionMode::Driving,
            },
        )
        .unwrap();
    let DocumentCommandEffect::CreatedDimension(conflicting_dimension) = conflict.value() else {
        panic!("created dimension effect expected");
    };
    assert_eq!(conflict.design_identity().revision().get(), 1);
    assert_eq!(conflict.attempt_identity().revision().get(), 1);
    assert!(conflict.published_accepted_identity().is_none());
    assert_eq!(session.accepted_state().unwrap().identity(), accepted_zero);
    assert_eq!(
        session.export_accepted_json().unwrap().unwrap(),
        accepted_zero_json
    );
    assert!(
        session
            .design_document()
            .dimension(*conflicting_dimension)
            .is_some()
    );
    assert!(!session.last_attempt().solve_result().unwrap().accepted());

    let source = session
        .design_document()
        .dimension(*conflicting_dimension)
        .unwrap()
        .source_id;
    let repaired = session
        .apply(
            session.design_identity(),
            DocumentEdit::SetSourceSuppressed {
                source,
                suppressed: true,
            },
        )
        .unwrap();
    let accepted_one = repaired.published_accepted_identity().unwrap();
    assert_eq!(repaired.design_identity().revision().get(), 2);
    assert_eq!(repaired.attempt_identity().revision().get(), 2);
    assert_eq!(accepted_one.revision().get(), 1);
    let accepted = session.accepted_state().unwrap();
    assert_eq!(accepted.identity(), accepted_one);
    assert_eq!(accepted.design_identity(), session.design_identity());
    assert_eq!(accepted.originating_attempt(), repaired.attempt_identity());
    assert!(accepted.solve_result().accepted());
    assert_eq!(
        accepted.solve_result().unstable_core_report().hard_validity,
        HardValidity::Valid
    );
}

#[test]
fn failed_unsuppression_remains_in_design_and_resuppression_repairs_it() {
    let (mut document, rectangle) = rectangle_design();
    let target = add_width_five_target(&mut document);
    let dimension = add_width_five_dimension(&mut document, rectangle.curves[0], target);
    let source = document.dimension(dimension).unwrap().source_id;
    document.set_source_suppressed(source, true).unwrap();
    let mut session = RetainedSketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let accepted_zero = session.accepted_state().unwrap().identity();

    let unsuppressed = session
        .apply(
            session.design_identity(),
            DocumentEdit::SetSourceSuppressed {
                source,
                suppressed: false,
            },
        )
        .unwrap();
    assert!(unsuppressed.published_accepted_identity().is_none());
    assert!(!session.design_document().source(source).unwrap().suppressed);
    assert_eq!(session.accepted_state().unwrap().identity(), accepted_zero);
    assert!(
        session
            .accepted_state()
            .unwrap()
            .document()
            .source(source)
            .unwrap()
            .suppressed
    );

    let repaired = session
        .apply(
            session.design_identity(),
            DocumentEdit::SetSourceSuppressed {
                source,
                suppressed: true,
            },
        )
        .unwrap();
    assert!(repaired.published_accepted_identity().is_some());
    assert!(session.design_document().source(source).unwrap().suppressed);
}

#[test]
fn invalid_design_edit_allocates_no_design_or_attempt_revision() {
    let (document, rectangle) = rectangle_design();
    let mut session = RetainedSketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let design = session.design_identity();
    let attempt = session.last_attempt().identity();
    let accepted = session.accepted_state().unwrap().identity();
    let design_json = session.export_design_json().unwrap();

    assert!(
        session
            .apply(
                design,
                DocumentEdit::SetPointPosition {
                    point: rectangle.points[0],
                    position: [f64::NAN, 0.0],
                },
            )
            .is_err()
    );
    assert_eq!(session.design_identity(), design);
    assert_eq!(session.last_attempt().identity(), attempt);
    assert_eq!(session.accepted_state().unwrap().identity(), accepted);
    assert_eq!(session.export_design_json().unwrap(), design_json);

    assert!(
        session
            .transact(design, |candidate| {
                candidate.add_point("invalid", [0.0, f64::INFINITY])?;
                Ok(())
            })
            .is_err()
    );
    assert_eq!(session.design_identity(), design);
    assert_eq!(session.last_attempt().identity(), attempt);
}

#[test]
fn topology_changing_conflict_preserves_old_accepted_view_by_identity() {
    let (mut document, rectangle) = rectangle_design();
    let target = add_width_five_target(&mut document);
    let mut session = RetainedSketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let accepted_identity = session.accepted_state().unwrap().identity();
    let accepted_document = session.accepted_state().unwrap().document().clone();
    let accepted_result = session.accepted_state().unwrap().solve_result().clone();

    let outcome = session
        .transact(session.design_identity(), |candidate| {
            let draft_point = candidate.add_point("draft only", [9.0, 9.0])?;
            let dimension = candidate.add_dimension(
                "width-5",
                DocumentDimensionDefinition::CurveLength {
                    curve: CurveSpan::line(rectangle.curves[0]),
                    target,
                },
                DocumentDimensionMode::Driving,
            )?;
            Ok((draft_point, dimension))
        })
        .unwrap();
    let (draft_point, _) = *outcome.value();

    assert!(outcome.published_accepted_identity().is_none());
    assert!(session.design_document().point(draft_point).is_some());
    let accepted = session.accepted_state().unwrap();
    assert_eq!(accepted.identity(), accepted_identity);
    assert_eq!(accepted.document(), &accepted_document);
    assert!(accepted.document().point(draft_point).is_none());
    assert!(
        session
            .last_attempt()
            .mappings()
            .unwrap()
            .runtime_point(draft_point)
            .is_some()
    );
    assert!(accepted.mappings().runtime_point(draft_point).is_none());
    assert_eq!(accepted.solve_result().geometry, accepted_result.geometry);
    assert_eq!(
        accepted.solve_result().display_audit,
        accepted_result.display_audit
    );
    assert_eq!(
        session.last_attempt().parent_accepted_identity(),
        Some(accepted_identity)
    );
    assert!(session.last_attempt().mappings().is_some());
}

#[test]
fn reattempt_changes_only_attempt_and_accepted_identity() {
    let (document, _) = rectangle_design();
    let mut session = RetainedSketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let design = session.design_identity();
    let parent = session.accepted_state().unwrap().identity();

    let attempt = session
        .reattempt(design, DocumentSolveRequest::default())
        .unwrap();
    assert_eq!(attempt.design_identity(), design);
    assert_eq!(attempt.identity().revision().get(), 1);
    assert_eq!(attempt.parent_accepted_identity(), Some(parent));
    assert_eq!(
        attempt.accepted_state_identity().unwrap().revision().get(),
        1
    );
    assert_eq!(session.design_identity(), design);
    assert_eq!(session.accepted_state().unwrap().design_identity(), design);
}

#[test]
fn underconstrained_parent_warm_start_is_joined_by_persistent_identity() {
    let mut document = SketchDocument::new(1.0).unwrap();
    let point = document.add_point("free", [0.0, 0.0]).unwrap();
    let initial_request = DocumentSolveRequest::default().with_drag(point, [3.0, 4.0]);
    let mut session =
        RetainedSketchDocumentSession::new(document, initial_request, SolverConfig::default())
            .unwrap();
    let parent = session.accepted_state().unwrap().identity();
    assert_eq!(
        session
            .accepted_state()
            .unwrap()
            .document()
            .point(point)
            .unwrap()
            .position
            .map(f64::to_bits),
        [3.0, 4.0].map(f64::to_bits)
    );
    assert_eq!(
        session
            .accepted_state()
            .unwrap()
            .solve_result()
            .unstable_core_report()
            .right_nullity,
        2
    );

    let attempt = session
        .reattempt(session.design_identity(), DocumentSolveRequest::default())
        .unwrap();
    assert_eq!(attempt.parent_accepted_identity(), Some(parent));
    assert!(attempt.accepted_state_identity().is_some());
    assert_eq!(
        session
            .accepted_state()
            .unwrap()
            .document()
            .point(point)
            .unwrap()
            .position
            .map(f64::to_bits),
        [3.0, 4.0].map(f64::to_bits),
        "default preferences must start from the accepted parent, not stale design coordinates"
    );
}

#[test]
fn point_edit_records_both_candidate_and_publication_requests() {
    let (document, rectangle) = rectangle_design();
    let mut session = RetainedSketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    session
        .apply(
            session.design_identity(),
            DocumentEdit::SetPointPosition {
                point: rectangle.points[1],
                position: [5.0, 0.0],
            },
        )
        .unwrap();

    let input = session.last_attempt().input();
    assert_eq!(
        input.candidate_request().drag.unwrap().point,
        rectangle.points[1]
    );
    assert!(!input.candidate_request().previous_state_preferences);
    assert!(input.publication_request().drag.is_none());
    assert!(input.publication_request().previous_state_preferences);
    assert_eq!(
        session.accepted_state().unwrap().input(),
        input,
        "accepted publication must repeat the complete implemented attempt input"
    );
}

#[test]
fn preview_seeded_point_apply_preserves_the_accepted_mechanism_configuration() {
    let (mut session, points, curves) = two_link_session();
    let preview = accepted_two_link_preview(&session, points[2]);
    let preview_document = preview.accepted_state().unwrap().document().clone();
    let position = preview_document.point(points[2]).unwrap().position;

    let outcome = session
        .apply_point_position_from_preview(session.design_identity(), points[2], position, &preview)
        .unwrap();

    assert!(outcome.published_accepted_identity().is_some());
    let committed = session.accepted_state().unwrap().document();
    for point in points {
        let expected = preview_document.point(point).unwrap().position;
        let actual = committed.point(point).unwrap().position;
        for axis in 0..2 {
            assert!((expected[axis] - actual[axis]).abs() <= 1.0e-10);
        }
    }
    for curve in curves {
        let branch = |document: &SketchDocument| match &document.curve(curve).unwrap().definition {
            CurveDefinition::Line {
                branch_direction, ..
            } => *branch_direction,
            _ => panic!("line expected"),
        };
        assert_eq!(
            branch(committed).map(f64::to_bits),
            branch(&preview_document).map(f64::to_bits)
        );
    }
}

#[test]
fn accepted_preview_continuation_retains_base_provenance_and_commits_exactly() {
    let (mut session, points, _) = two_link_session();
    let base_accepted = session.accepted_state().unwrap().identity();
    let first = accepted_two_link_preview(&session, points[2]);
    let mut second = session.clone();
    let request = DocumentSolveRequest::default()
        .with_previous_state_preferences()
        .with_drag(points[2], [0.5, 0.25]);
    second
        .reattempt_from_accepted_preview_controlled(
            second.design_identity(),
            request,
            &first,
            OperationControl::unlimited(),
        )
        .unwrap();

    let second_accepted = second.accepted_state().expect("continued preview accepted");
    assert_eq!(
        second.last_attempt().parent_accepted_identity(),
        Some(base_accepted)
    );
    assert_eq!(
        second.last_attempt().accepted_state_identity(),
        Some(second_accepted.identity())
    );
    let position = second_accepted
        .document()
        .point(points[2])
        .unwrap()
        .position;
    let outcome = session
        .apply_point_position_from_preview(session.design_identity(), points[2], position, &second)
        .unwrap();
    assert!(outcome.published_accepted_identity().is_some());
    assert_eq!(
        session
            .accepted_state()
            .unwrap()
            .document()
            .point(points[2])
            .unwrap()
            .position
            .map(f64::to_bits),
        position.map(f64::to_bits)
    );
}

#[test]
#[allow(clippy::too_many_lines)]
fn accepted_preview_continuation_keeps_authoritative_previous_state_reference() {
    let mut document = SketchDocument::new(1.0).unwrap();
    let active = document.add_point("active", [0.0, 0.0]).unwrap();
    let passive = document.add_point("passive", [4.0, -2.0]).unwrap();
    document
        .add_constraint(
            "fixed active",
            DocumentConstraintDefinition::FixedPoint {
                point: active,
                target: [0.0, 0.0],
            },
        )
        .unwrap();
    let session = RetainedSketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let authoritative_passive = session
        .accepted_state()
        .unwrap()
        .document()
        .point(passive)
        .unwrap()
        .position;

    // Deliberately create a valid warm-start preview whose independent passive
    // point is far from the authoritative state.
    let displaced_passive = [-7.0, 6.0];
    let mut first = session.clone();
    first
        .reattempt_controlled(
            first.design_identity(),
            DocumentSolveRequest::default()
                .without_previous_state_preferences()
                .with_drag(passive, displaced_passive),
            OperationControl::unlimited(),
        )
        .unwrap();
    let first_passive = first
        .accepted_state()
        .expect("first preview accepted")
        .document()
        .point(passive)
        .unwrap()
        .position;
    assert!(
        (first_passive[0] - displaced_passive[0]).hypot(first_passive[1] - displaced_passive[1])
            <= 1.0e-10
    );

    // Continuation seeds numerically from `first`, but its PreviousState
    // preference must still come from the unchanged authoritative session.
    let mut continued = session.clone();
    continued
        .reattempt_from_accepted_preview_controlled(
            continued.design_identity(),
            DocumentSolveRequest::default()
                .with_previous_state_preferences()
                .with_drag(active, [1.25, 0.75]),
            &first,
            OperationControl::unlimited(),
        )
        .unwrap();
    let continued_passive = continued
        .accepted_state()
        .expect("continued preview accepted")
        .document()
        .point(passive)
        .unwrap()
        .position;
    assert!(
        (continued_passive[0] - authoritative_passive[0])
            .hypot(continued_passive[1] - authoritative_passive[1])
            <= 1.0e-10,
        "passive point followed warm seed from {authoritative_passive:?} to {continued_passive:?}"
    );

    let accepted = continued.accepted_state().unwrap();
    let solve = accepted.solve_result();
    let runtime_passive = accepted.mappings().runtime_point(passive).unwrap();
    let preference_source = solve
        .source_mappings
        .iter()
        .find(|mapping| mapping.source == SketchSource::PreviousState(runtime_passive))
        .and_then(|mapping| mapping.core_source_id)
        .expect("passive PreviousState source");
    let preference_audit = solve
        .display_audit
        .sources
        .iter()
        .find(|source| source.source_id == preference_source)
        .expect("passive PreviousState audit");
    assert_eq!(preference_audit.rows.len(), 2);
    assert!(preference_audit.rows.iter().all(|row| {
        row.category == ResidualCategory::Preference
            && row
                .bindings
                .iter()
                .any(|binding| binding.name == "target" && binding.value == "(4, -2)")
    }));
    for (axis, row) in preference_audit.rows.iter().enumerate() {
        assert!(
            (row.raw_residual - (continued_passive[axis] - authoritative_passive[axis])).abs()
                <= 1.0e-12
        );
    }
    let audit_preference_cost = solve
        .display_audit
        .sources
        .iter()
        .flat_map(|source| &source.rows)
        .filter(|row| row.category == ResidualCategory::Preference)
        .map(|row| 0.5 * row.normalized_residual.powi(2))
        .sum::<f64>();
    let preference_reports = solve
        .unstable_core_report()
        .priority_solves
        .iter()
        .filter(|priority| priority.category == ResidualCategory::Preference)
        .collect::<Vec<_>>();
    assert!(!preference_reports.is_empty());
    let reported_preference_cost = preference_reports
        .into_iter()
        .map(|priority| {
            priority
                .final_cost
                .expect("accepted Preference group has a returned-state cost")
        })
        .sum::<f64>();
    assert!(audit_preference_cost.is_finite());
    assert!(reported_preference_cost.is_finite());
    assert!(
        (audit_preference_cost - reported_preference_cost).abs()
            <= 1.0e-12
                * audit_preference_cost
                    .abs()
                    .max(reported_preference_cost.abs())
                    .max(1.0),
        "Preference audit cost {audit_preference_cost} disagrees with report cost \
         {reported_preference_cost}"
    );

    let runtime_active = accepted.mappings().runtime_point(active).unwrap();
    let temporary_source = solve
        .source_mappings
        .iter()
        .find(|mapping| mapping.source == SketchSource::DragTarget(runtime_active))
        .and_then(|mapping| mapping.core_source_id)
        .expect("active Temporary source");
    let temporary_audit = solve
        .display_audit
        .sources
        .iter()
        .find(|source| source.source_id == temporary_source)
        .expect("active Temporary audit");
    assert!(
        temporary_audit
            .rows
            .iter()
            .any(|row| row.category == ResidualCategory::Temporary
                && row.normalized_residual.abs() > 0.5),
        "test must retain a genuinely positive attained Temporary level"
    );

    let mut third = session.clone();
    third
        .reattempt_from_accepted_preview_controlled(
            third.design_identity(),
            DocumentSolveRequest::default()
                .with_previous_state_preferences()
                .with_drag(active, [1.5, 1.0]),
            &continued,
            OperationControl::unlimited(),
        )
        .unwrap();
    let third_passive = third
        .accepted_state()
        .expect("second continued preview accepted")
        .document()
        .point(passive)
        .unwrap()
        .position;
    assert!(
        (third_passive[0] - authoritative_passive[0])
            .hypot(third_passive[1] - authoritative_passive[1])
            <= 1.0e-10
    );
}

#[test]
#[allow(clippy::too_many_lines)]
fn drag_locality_plan_uses_accepted_visible_targets_and_freezes_them_through_continuation() {
    let mut document = SketchDocument::new(5.0).unwrap();
    let active = document.add_point("active", [0.0, 0.0]).unwrap();
    let passive = document.add_point("passive", [3.0, 0.0]).unwrap();
    let midpoint = document.add_point("midpoint", [1.5, 0.0]).unwrap();
    let span = document
        .add_curve(
            "span",
            CurveDefinition::Line {
                start: active,
                end: passive,
                branch_direction: [1.0, 0.0],
            },
        )
        .unwrap();
    document
        .add_constraint(
            "midpoint",
            DocumentConstraintDefinition::Midpoint {
                point: midpoint,
                line: CurveSpan::line(span),
            },
        )
        .unwrap();
    let distance = document
        .add_scalar("distance", 5.0, ScalarUnit::Length, ScalarDomain::Positive)
        .unwrap();
    document
        .add_dimension(
            "distance",
            DocumentDimensionDefinition::PointDistance {
                first: active,
                second: passive,
                target: distance,
            },
            DocumentDimensionMode::Driving,
        )
        .unwrap();

    let session = RetainedSketchDocumentSession::new(
        document,
        DocumentSolveRequest::default().without_previous_state_preferences(),
        SolverConfig::default(),
    )
    .unwrap();
    let accepted = session.accepted_state().expect("accepted mechanism");
    let accepted_active = accepted.document().point(active).unwrap().position;
    let accepted_passive = accepted.document().point(passive).unwrap().position;
    assert!(
        (accepted_passive[0] - 3.0).hypot(accepted_passive[1]) > 0.5,
        "fixture must distinguish accepted geometry from its pre-solve seed"
    );

    let plan = session.drag_locality_plan(active).expect("locality plan");
    assert_eq!(plan.design_identity(), session.design_identity());
    assert_eq!(plan.accepted_state_identity(), accepted.identity());
    assert_eq!(plan.point(), active);
    assert_eq!(plan.hard_degrees_of_freedom(), 3);
    assert_eq!(plan.active_rank(), 2);
    assert_eq!(plan.passive_degrees_of_freedom(), 1);
    assert_eq!(plan.anchors().len(), 1);
    assert_eq!(plan.anchors()[0].point(), passive);
    assert_eq!(
        plan.anchors()[0].target().map(f64::to_bits),
        accepted_passive.map(f64::to_bits),
        "gesture target must come from accepted visible geometry, not the older runtime reference"
    );

    let point_on_radius = |angle: f64| {
        [
            accepted_passive[0] - 5.0 * angle.cos(),
            accepted_passive[1] - 5.0 * angle.sin(),
        ]
    };
    let mut first = session.clone();
    first
        .reattempt_with_drag_locality_controlled(
            first.design_identity(),
            DocumentSolveRequest::default()
                .with_previous_state_preferences()
                .with_drag(active, point_on_radius(0.08)),
            &plan,
            OperationControl::unlimited(),
        )
        .unwrap();
    assert!(first.last_attempt().accepted_state_identity().is_some());
    let first_accepted = first.accepted_state().unwrap();
    let first_passive = first_accepted.document().point(passive).unwrap().position;
    assert!(
        (first_passive[0] - accepted_passive[0]).hypot(first_passive[1] - accepted_passive[1])
            <= 1.0e-8,
        "passive anchor moved from {accepted_passive:?} to {first_passive:?}"
    );
    let runtime_passive = first_accepted
        .mappings()
        .runtime_point(passive)
        .expect("runtime passive");
    let preference_sources = first_accepted
        .solve_result()
        .source_mappings
        .iter()
        .filter_map(|mapping| match mapping.source {
            SketchSource::PreviousState(point) => Some(point),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(preference_sources, vec![runtime_passive]);

    let continued_target = point_on_radius(0.16);
    let mut continued = session.clone();
    continued
        .reattempt_from_accepted_preview_with_drag_locality_controlled(
            continued.design_identity(),
            DocumentSolveRequest::default()
                .with_previous_state_preferences()
                .with_drag(active, continued_target),
            &first,
            &plan,
            OperationControl::unlimited(),
        )
        .unwrap();
    assert!(continued.last_attempt().accepted_state_identity().is_some());
    let continued_accepted = continued.accepted_state().unwrap();
    let continued_active = continued_accepted
        .document()
        .point(active)
        .unwrap()
        .position;
    let continued_passive = continued_accepted
        .document()
        .point(passive)
        .unwrap()
        .position;
    assert!(
        (continued_active[0] - continued_target[0])
            .hypot(continued_active[1] - continued_target[1])
            <= 1.0e-8,
        "continued active point used a stale drag target: expected {continued_target:?}, \
         got {continued_active:?}"
    );
    assert!(
        (continued_passive[0] - accepted_passive[0])
            .hypot(continued_passive[1] - accepted_passive[1])
            <= 1.0e-8,
        "continued passive anchor moved from {accepted_passive:?} to {continued_passive:?}"
    );
    assert!(
        (accepted_active[0] - continued_target[0]).hypot(accepted_active[1] - continued_target[1])
            > 0.1,
        "fixture must perform real accepted motion"
    );

    let runtime_active = continued_accepted
        .mappings()
        .runtime_point(active)
        .expect("continued runtime active point");
    let solve = continued_accepted.solve_result();
    let drag_source = solve
        .source_mappings
        .iter()
        .find(|mapping| mapping.source == SketchSource::DragTarget(runtime_active))
        .and_then(|mapping| mapping.core_source_id)
        .expect("continued drag source");
    let drag_audit = solve
        .display_audit
        .sources
        .iter()
        .find(|source| source.source_id == drag_source)
        .expect("continued drag audit");
    assert_eq!(drag_audit.rows.len(), 2);
    for row in &drag_audit.rows {
        assert!(row.row_in_block < 2, "{row:#?}");
        let coordinate = row.row_in_block;
        let audited_target = continued_active[coordinate] - row.raw_residual;
        assert!(
            (audited_target - continued_target[coordinate]).abs() <= 1.0e-12,
            "continued drag audit retained a stale target: {drag_audit:#?}"
        );
    }

    let mut exhausted = session.clone();
    let before_exhaustion = (
        exhausted.design_identity(),
        exhausted.last_attempt().identity(),
        exhausted.accepted_state().unwrap().identity(),
        exhausted.export_design_json().unwrap(),
        exhausted.export_accepted_json().unwrap(),
        exhausted.revision_high_water(),
    );
    let mut exhausted_control = OperationControl::unlimited();
    exhausted_control.limits.document_validation_items = 0;
    let stopped = exhausted
        .apply_point_position_from_preview_with_drag_locality_controlled(
            exhausted.design_identity(),
            active,
            continued_active,
            &continued,
            &plan,
            exhausted_control,
        )
        .expect("controlled release outcome");
    assert!(matches!(stopped, OperationOutcome::WorkExhausted { .. }));
    assert_eq!(exhausted.design_identity(), before_exhaustion.0);
    assert_eq!(exhausted.last_attempt().identity(), before_exhaustion.1);
    assert_eq!(
        exhausted.accepted_state().unwrap().identity(),
        before_exhaustion.2
    );
    assert_eq!(exhausted.export_design_json().unwrap(), before_exhaustion.3);
    assert_eq!(
        exhausted.export_accepted_json().unwrap(),
        before_exhaustion.4
    );
    assert_eq!(exhausted.revision_high_water(), before_exhaustion.5);

    let mut released = session.clone();
    let publication = released
        .apply_point_position_from_preview_with_drag_locality_controlled(
            released.design_identity(),
            active,
            continued_active,
            &continued,
            &plan,
            OperationControl::unlimited(),
        )
        .expect("controlled release");
    assert!(matches!(publication, OperationOutcome::Completed { .. }));
    let mut late_exhausted = session.clone();
    let before_late_exhaustion = (
        late_exhausted.design_identity(),
        late_exhausted.last_attempt().identity(),
        late_exhausted.accepted_state().unwrap().identity(),
        late_exhausted.export_design_json().unwrap(),
        late_exhausted.export_accepted_json().unwrap(),
        late_exhausted.revision_high_water(),
    );
    let mut late_control = OperationControl::unlimited();
    late_control.limits.document_lowering_items = publication
        .report()
        .consumed
        .document_lowering_items
        .saturating_sub(1);
    let late_stopped = late_exhausted
        .apply_point_position_from_preview_with_drag_locality_controlled(
            late_exhausted.design_identity(),
            active,
            continued_active,
            &continued,
            &plan,
            late_control,
        )
        .expect("late controlled release outcome");
    assert!(matches!(
        late_stopped,
        OperationOutcome::WorkExhausted { ref report }
            if matches!(
                report.stopping_reason,
                Some(geosolve_core::OperationStopReason::WorkExhausted {
                    counter: geosolve_core::OperationWorkCounter::DocumentLoweringItems,
                    checkpoint: geosolve_core::OperationCheckpoint::DocumentLowering,
                })
            )
    ));
    assert_eq!(late_exhausted.design_identity(), before_late_exhaustion.0);
    assert_eq!(
        late_exhausted.last_attempt().identity(),
        before_late_exhaustion.1
    );
    assert_eq!(
        late_exhausted.accepted_state().unwrap().identity(),
        before_late_exhaustion.2
    );
    assert_eq!(
        late_exhausted.export_design_json().unwrap(),
        before_late_exhaustion.3
    );
    assert_eq!(
        late_exhausted.export_accepted_json().unwrap(),
        before_late_exhaustion.4
    );
    assert_eq!(
        late_exhausted.revision_high_water(),
        before_late_exhaustion.5
    );
    assert!(
        !released.request().previous_state_preferences,
        "the authoritative retained request deliberately remains disabled"
    );
    let published = released.accepted_state().expect("published release");
    assert_eq!(
        released.export_accepted_json().unwrap(),
        continued.export_accepted_json().unwrap(),
        "release must preserve the complete accepted preview bytes"
    );
    assert_eq!(
        published.runtime().execution_summary().kind,
        SketchSessionExecutionKind::NoMotionCertification
    );
    assert_eq!(
        published.solve_result().unstable_core_report().iterations,
        0
    );
    assert!(
        published
            .solve_result()
            .unstable_core_report()
            .priority_solves
            .iter()
            .all(|priority| priority.iterations == 0)
    );
    let input = published.input();
    assert!(input.candidate_request().previous_state_preferences);
    assert!(input.candidate_request().drag.is_some());
    assert!(input.publication_request().previous_state_preferences);
    assert!(input.publication_request().drag.is_none());
    let runtime_passive = published
        .mappings()
        .runtime_point(passive)
        .expect("published passive point");
    let preference_sources = published
        .solve_result()
        .source_mappings
        .iter()
        .filter_map(|mapping| match mapping.source {
            SketchSource::PreviousState(point) => Some(point),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        preference_sources,
        vec![runtime_passive],
        "publication must compile only the frozen anchor, never every preview-seeded point"
    );
    assert!(
        published
            .solve_result()
            .source_mappings
            .iter()
            .all(|mapping| !matches!(mapping.source, SketchSource::DragTarget(_))),
        "the cursor target is attempt evidence and must not enter the publication runtime"
    );
    assert!(
        (published.document().point(active).unwrap().position[0] - continued_active[0])
            .hypot(published.document().point(active).unwrap().position[1] - continued_active[1])
            <= 1.0e-8
    );
    assert!(
        (published.document().point(passive).unwrap().position[0] - accepted_passive[0])
            .hypot(published.document().point(passive).unwrap().position[1] - accepted_passive[1])
            <= 1.0e-8
    );
}

#[test]
fn drag_locality_plan_is_stale_after_release_and_fixed_points_use_an_empty_plan() {
    let mut document = SketchDocument::new(1.0).unwrap();
    let fixed = document.add_point("fixed", [0.0, 0.0]).unwrap();
    document
        .add_constraint(
            "fixed",
            DocumentConstraintDefinition::FixedPoint {
                point: fixed,
                target: [0.0, 0.0],
            },
        )
        .unwrap();
    let session = RetainedSketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let plan = session.drag_locality_plan(fixed).expect("fixed plan");
    assert_eq!(plan.hard_degrees_of_freedom(), 0);
    assert_eq!(plan.active_rank(), 0);
    assert_eq!(plan.passive_degrees_of_freedom(), 0);
    assert!(plan.anchors().is_empty());

    let mut preview = session.clone();
    preview
        .reattempt_with_drag_locality_controlled(
            preview.design_identity(),
            DocumentSolveRequest::default().with_drag(fixed, [1.0, 1.0]),
            &plan,
            OperationControl::unlimited(),
        )
        .unwrap();
    assert!(preview.last_attempt().accepted_state_identity().is_some());
    assert_eq!(
        preview
            .accepted_state()
            .unwrap()
            .document()
            .point(fixed)
            .unwrap()
            .position
            .map(f64::to_bits),
        [0.0, 0.0].map(f64::to_bits)
    );

    let mut released = session.clone();
    released
        .apply_point_position_from_preview(released.design_identity(), fixed, [0.0, 0.0], &preview)
        .unwrap();
    let before = released.revision_high_water();
    let actual_design = released.design_identity();
    let actual_accepted = released
        .accepted_state()
        .map(geosolve_sketch::SketchAcceptedDocumentState::identity);
    let error = released
        .reattempt_with_drag_locality_controlled(
            released.design_identity(),
            DocumentSolveRequest::default().with_drag(fixed, [0.0, 0.0]),
            &plan,
            OperationControl::unlimited(),
        )
        .unwrap_err();
    let DocumentSessionError::StaleDragLocalityPlan { evidence } = error else {
        panic!("expected structured stale drag-locality evidence");
    };
    assert_eq!(evidence.expected_design, plan.design_identity());
    assert_eq!(evidence.expected_accepted, plan.accepted_state_identity());
    assert_eq!(evidence.actual_design, actual_design);
    assert_eq!(evidence.actual_accepted, actual_accepted);
    assert_eq!(released.revision_high_water(), before);
}

#[test]
fn drag_locality_plan_rejects_a_divergent_same_revision_accepted_parent() {
    let mut document = SketchDocument::new(1.0).unwrap();
    let point = document.add_point("free", [0.0, 0.0]).unwrap();
    let base = RetainedSketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let mut first = base.clone();
    let mut second = base;

    first
        .apply(
            first.design_identity(),
            DocumentEdit::SetPointPosition {
                point,
                position: [1.0, 0.0],
            },
        )
        .unwrap();
    second
        .apply(
            second.design_identity(),
            DocumentEdit::SetPointPosition {
                point,
                position: [-1.0, 0.0],
            },
        )
        .unwrap();
    assert_eq!(first.design_identity(), second.design_identity());
    assert_eq!(
        first.accepted_state().unwrap().identity(),
        second.accepted_state().unwrap().identity(),
        "divergent lifecycle clones intentionally occupy the same numeric revision"
    );

    let first_plan = first.drag_locality_plan(point).unwrap();
    let second_plan = second.drag_locality_plan(point).unwrap();
    assert_ne!(
        first_plan, second_plan,
        "locality equality must include exact accepted-state provenance"
    );
    let before = (
        second.revision_high_water(),
        second.export_design_json().unwrap(),
        second.export_accepted_json().unwrap(),
    );
    assert!(matches!(
        second.reattempt_with_drag_locality_controlled(
            second.design_identity(),
            DocumentSolveRequest::default().with_drag(point, [-2.0, 0.0]),
            &first_plan,
            OperationControl::unlimited(),
        ),
        Err(DocumentSessionError::StaleDragLocalityPlan { .. })
    ));
    assert_eq!(second.revision_high_water(), before.0);
    assert_eq!(second.export_design_json().unwrap(), before.1);
    assert_eq!(second.export_accepted_json().unwrap(), before.2);
}

#[test]
fn drag_publication_rejects_a_preview_that_moved_its_frozen_anchor_atomically() {
    let mut document = SketchDocument::new(1.0).unwrap();
    let active = document.add_point("active", [0.0, 0.0]).unwrap();
    let passive = document.add_point("passive", [2.0, 0.0]).unwrap();
    let line = document
        .add_curve(
            "horizontal pair",
            CurveDefinition::Line {
                start: active,
                end: passive,
                branch_direction: [1.0, 0.0],
            },
        )
        .unwrap();
    document
        .add_constraint(
            "horizontal",
            DocumentConstraintDefinition::Horizontal {
                line: CurveSpan::line(line),
            },
        )
        .unwrap();
    let mut session = RetainedSketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let locality = session.drag_locality_plan(active).expect("locality plan");
    assert_eq!(locality.anchors().len(), 1);
    assert_eq!(locality.anchors()[0].point(), passive);

    let mut preview = session.clone();
    preview
        .reattempt(
            preview.design_identity(),
            DocumentSolveRequest::default()
                .without_previous_state_preferences()
                .with_drag(active, [1.0, 1.0]),
        )
        .unwrap();
    let preview_document = preview.accepted_state().unwrap().document();
    let active_position = preview_document.point(active).unwrap().position;
    let passive_position = preview_document.point(passive).unwrap().position;
    assert!(
        (passive_position[0] - locality.anchors()[0].target()[0])
            .hypot(passive_position[1] - locality.anchors()[0].target()[1])
            > 0.1,
        "the deliberately non-local preview must move its passive point"
    );

    let before = (
        session.design_identity(),
        session.last_attempt().identity(),
        session.accepted_state().unwrap().identity(),
        session.export_design_json().unwrap(),
        session.export_accepted_json().unwrap(),
        session.revision_high_water(),
    );
    assert!(matches!(
        session.apply_point_position_from_preview_with_drag_locality(
            session.design_identity(),
            active,
            active_position,
            &preview,
            &locality,
        ),
        Err(DocumentSessionError::DragPublicationContinuity {
            context: "a preview anchor moved from its gesture-start target"
        })
    ));
    assert_eq!(session.design_identity(), before.0);
    assert_eq!(session.last_attempt().identity(), before.1);
    assert_eq!(session.accepted_state().unwrap().identity(), before.2);
    assert_eq!(session.export_design_json().unwrap(), before.3);
    assert_eq!(session.export_accepted_json().unwrap(), before.4);
    assert_eq!(session.revision_high_water(), before.5);
}

#[test]
fn accepted_preview_continuation_rejects_stale_foreign_and_unpublished_inputs_atomically() {
    let (mut session, points, _) = two_link_session();
    let preview = accepted_two_link_preview(&session, points[2]);
    let before = (
        session.design_identity(),
        session.last_attempt().identity(),
        session.accepted_state().unwrap().identity(),
        session.export_design_json().unwrap(),
        session.export_accepted_json().unwrap(),
    );
    let request = DocumentSolveRequest::default()
        .with_previous_state_preferences()
        .with_drag(points[2], [0.5, 0.25]);

    let (foreign_base, foreign_points, _) = two_link_session();
    let foreign = accepted_two_link_preview(&foreign_base, foreign_points[2]);
    assert!(matches!(
        session.reattempt_from_accepted_preview_controlled(
            session.design_identity(),
            request,
            &foreign,
            OperationControl::unlimited(),
        ),
        Err(DocumentSessionError::PreviewForeignDocument)
    ));
    assert_eq!(session.design_identity(), before.0);
    assert_eq!(session.last_attempt().identity(), before.1);
    assert_eq!(session.accepted_state().unwrap().identity(), before.2);
    assert_eq!(session.export_design_json().unwrap(), before.3);
    assert_eq!(session.export_accepted_json().unwrap(), before.4);

    session
        .reattempt(session.design_identity(), DocumentSolveRequest::default())
        .unwrap();
    assert!(matches!(
        session.reattempt_from_accepted_preview_controlled(
            session.design_identity(),
            request,
            &preview,
            OperationControl::unlimited(),
        ),
        Err(DocumentSessionError::PreviewAcceptedProvenance)
    ));

    let (mut rejected_base, rejected_points, _) = two_link_session();
    let conflict = DocumentEdit::CreateConstraint {
        label: "impossible fixed end".into(),
        definition: DocumentConstraintDefinition::FixedPoint {
            point: rejected_points[2],
            target: [5.0, 5.0],
        },
    };
    rejected_base
        .apply(rejected_base.design_identity(), conflict)
        .unwrap();
    assert!(
        rejected_base
            .last_attempt()
            .accepted_state_identity()
            .is_none()
    );
    let rejected = rejected_base.clone();
    assert!(matches!(
        rejected_base.reattempt_from_accepted_preview_controlled(
            rejected_base.design_identity(),
            request,
            &rejected,
            OperationControl::unlimited(),
        ),
        Err(DocumentSessionError::PreviewNotAccepted)
    ));

    let (mut unchanged, unchanged_points, _) = two_link_session();
    let stale = accepted_two_link_preview(&unchanged, unchanged_points[2]);
    unchanged
        .apply(
            unchanged.design_identity(),
            DocumentEdit::CreatePoint {
                label: "advance".into(),
                position: [3.0, 3.0],
            },
        )
        .unwrap();
    assert!(matches!(
        unchanged.reattempt_from_accepted_preview_controlled(
            unchanged.design_identity(),
            request,
            &stale,
            OperationControl::unlimited(),
        ),
        Err(DocumentSessionError::PreviewStaleDesign)
    ));
}

#[test]
fn accepted_preview_continuation_rejects_divergent_same_revision_design_content_atomically() {
    let (base, points, _) = two_link_session();
    let mut authoritative = base.clone();
    let mut divergent = base;
    authoritative
        .apply(
            authoritative.design_identity(),
            DocumentEdit::CreatePoint {
                label: "authoritative-only".into(),
                position: [3.0, 1.0],
            },
        )
        .unwrap();
    divergent
        .apply(
            divergent.design_identity(),
            DocumentEdit::CreatePoint {
                label: "divergent-only".into(),
                position: [-3.0, -1.0],
            },
        )
        .unwrap();
    assert_eq!(
        authoritative.design_identity(),
        divergent.design_identity(),
        "independent clones deliberately collide in the revision-only identity"
    );
    assert_ne!(
        authoritative.design_document(),
        divergent.design_document(),
        "fixture must carry different retained content"
    );
    let mut preview = divergent.clone();
    preview
        .reattempt(
            preview.design_identity(),
            DocumentSolveRequest::default()
                .without_previous_state_preferences()
                .with_drag(points[2], [1.5, 0.5]),
        )
        .unwrap();
    assert!(preview.last_attempt().accepted_state_identity().is_some());

    let before = (
        authoritative.design_identity(),
        authoritative.last_attempt().identity(),
        authoritative.accepted_state().unwrap().identity(),
        authoritative.export_design_json().unwrap(),
        authoritative.export_accepted_json().unwrap(),
    );
    assert!(matches!(
        authoritative.reattempt_from_accepted_preview_controlled(
            authoritative.design_identity(),
            DocumentSolveRequest::default().with_drag(points[2], [1.25, 0.75]),
            &preview,
            OperationControl::unlimited(),
        ),
        Err(DocumentSessionError::PreviewStaleDesign)
    ));
    assert_eq!(authoritative.design_identity(), before.0);
    assert_eq!(authoritative.last_attempt().identity(), before.1);
    assert_eq!(authoritative.accepted_state().unwrap().identity(), before.2);
    assert_eq!(authoritative.export_design_json().unwrap(), before.3);
    assert_eq!(authoritative.export_accepted_json().unwrap(), before.4);
}

#[test]
fn accepted_preview_rejects_signed_zero_divergent_design_provenance_atomically() {
    let mut document = SketchDocument::new(1.0).unwrap();
    let point = document.add_point("free", [0.0, 0.0]).unwrap();
    let base = RetainedSketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let mut authoritative = base.clone();
    let mut divergent = base;
    authoritative
        .apply(
            authoritative.design_identity(),
            DocumentEdit::SetPointPosition {
                point,
                position: [0.0, 0.0],
            },
        )
        .unwrap();
    divergent
        .apply(
            divergent.design_identity(),
            DocumentEdit::SetPointPosition {
                point,
                position: [-0.0, 0.0],
            },
        )
        .unwrap();
    assert_eq!(authoritative.design_identity(), divergent.design_identity());
    assert_eq!(
        authoritative.design_document(),
        divergent.design_document(),
        "ordinary floating equality deliberately cannot distinguish the fixture"
    );
    assert_ne!(
        authoritative.export_design_json().unwrap(),
        divergent.export_design_json().unwrap(),
        "canonical bytes must preserve the signed-zero distinction"
    );

    let mut preview = divergent.clone();
    preview
        .reattempt(
            preview.design_identity(),
            DocumentSolveRequest::default().with_drag(point, [1.0, 0.0]),
        )
        .unwrap();
    assert!(preview.last_attempt().accepted_state_identity().is_some());

    let before = (
        authoritative.design_identity(),
        authoritative.last_attempt().identity(),
        authoritative.accepted_state().unwrap().identity(),
        authoritative.export_design_json().unwrap(),
        authoritative.export_accepted_json().unwrap(),
    );
    assert!(matches!(
        authoritative.reattempt_from_accepted_preview_controlled(
            authoritative.design_identity(),
            DocumentSolveRequest::default().with_drag(point, [0.5, 0.0]),
            &preview,
            OperationControl::unlimited(),
        ),
        Err(DocumentSessionError::PreviewStaleDesign)
    ));
    assert_eq!(authoritative.design_identity(), before.0);
    assert_eq!(authoritative.last_attempt().identity(), before.1);
    assert_eq!(authoritative.accepted_state().unwrap().identity(), before.2);
    assert_eq!(authoritative.export_design_json().unwrap(), before.3);
    assert_eq!(authoritative.export_accepted_json().unwrap(), before.4);
}

#[test]
fn accepted_preview_continuation_rejects_divergent_same_identity_parent_atomically() {
    let (base, points, _) = two_link_session();
    let mut authoritative = base.clone();
    let mut divergent_parent = base;
    authoritative
        .reattempt(
            authoritative.design_identity(),
            DocumentSolveRequest::default()
                .without_previous_state_preferences()
                .with_drag(points[2], [0.0, 0.0]),
        )
        .unwrap();
    divergent_parent
        .reattempt(
            divergent_parent.design_identity(),
            DocumentSolveRequest::default()
                .without_previous_state_preferences()
                .with_drag(points[2], [1.5, 0.5]),
        )
        .unwrap();
    assert_eq!(
        authoritative.accepted_state().unwrap().identity(),
        divergent_parent.accepted_state().unwrap().identity(),
        "independent accepted publications deliberately collide in revision identity"
    );
    let mut preview = divergent_parent.clone();
    preview
        .reattempt(
            preview.design_identity(),
            DocumentSolveRequest::default()
                .without_previous_state_preferences()
                .with_drag(points[2], [1.25, 0.75]),
        )
        .unwrap();
    assert!(preview.last_attempt().accepted_state_identity().is_some());

    let before = (
        authoritative.design_identity(),
        authoritative.last_attempt().identity(),
        authoritative.accepted_state().unwrap().identity(),
        authoritative.export_design_json().unwrap(),
        authoritative.export_accepted_json().unwrap(),
    );
    assert!(matches!(
        authoritative.reattempt_from_accepted_preview_controlled(
            authoritative.design_identity(),
            DocumentSolveRequest::default().with_drag(points[2], [1.0, 1.0]),
            &preview,
            OperationControl::unlimited(),
        ),
        Err(DocumentSessionError::PreviewAcceptedProvenance)
    ));
    assert_eq!(authoritative.design_identity(), before.0);
    assert_eq!(authoritative.last_attempt().identity(), before.1);
    assert_eq!(authoritative.accepted_state().unwrap().identity(), before.2);
    assert_eq!(authoritative.export_design_json().unwrap(), before.3);
    assert_eq!(authoritative.export_accepted_json().unwrap(), before.4);
}

#[test]
fn preview_seeded_point_apply_rejects_mismatched_point_and_position_without_mutation() {
    let (session, points, _) = two_link_session();
    let preview = accepted_two_link_preview(&session, points[2]);
    let preview_document = preview.accepted_state().unwrap().document();
    let end_position = preview_document.point(points[2]).unwrap().position;

    for (point, position) in [
        (
            points[1],
            preview_document.point(points[1]).unwrap().position,
        ),
        (
            points[2],
            [
                f64::from_bits(end_position[0].to_bits() + 1),
                end_position[1],
            ],
        ),
    ] {
        let mut attempt = session.clone();
        let before = (
            attempt.design_identity(),
            attempt.last_attempt().identity(),
            attempt.accepted_state().unwrap().identity(),
            attempt.export_design_json().unwrap(),
            attempt.export_accepted_json().unwrap(),
        );
        assert!(matches!(
            attempt.apply_point_position_from_preview(
                attempt.design_identity(),
                point,
                position,
                &preview
            ),
            Err(DocumentSessionError::PreviewPointMismatch)
        ));
        assert_eq!(attempt.design_identity(), before.0);
        assert_eq!(attempt.last_attempt().identity(), before.1);
        assert_eq!(attempt.accepted_state().unwrap().identity(), before.2);
        assert_eq!(attempt.export_design_json().unwrap(), before.3);
        assert_eq!(attempt.export_accepted_json().unwrap(), before.4);
    }
}

#[test]
fn preview_seeded_point_apply_rejects_stale_and_foreign_preview_without_mutation() {
    let (mut session, points, _) = two_link_session();
    let stale = accepted_two_link_preview(&session, points[2]);
    session
        .apply(
            session.design_identity(),
            DocumentEdit::CreatePoint {
                label: "advance".into(),
                position: [3.0, 3.0],
            },
        )
        .unwrap();
    let before_design = session.design_identity();
    let before_attempt = session.last_attempt().identity();
    let before_accepted = session.accepted_state().unwrap().identity();
    let before_design_json = session.export_design_json().unwrap();
    let before_accepted_json = session.export_accepted_json().unwrap();
    let position = stale
        .accepted_state()
        .unwrap()
        .document()
        .point(points[2])
        .unwrap()
        .position;
    assert!(matches!(
        session.apply_point_position_from_preview(
            session.design_identity(),
            points[2],
            position,
            &stale
        ),
        Err(DocumentSessionError::PreviewStaleDesign)
    ));
    assert_eq!(session.design_identity(), before_design);
    assert_eq!(session.last_attempt().identity(), before_attempt);
    assert_eq!(
        session.accepted_state().unwrap().identity(),
        before_accepted
    );
    assert_eq!(session.export_design_json().unwrap(), before_design_json);
    assert_eq!(
        session.export_accepted_json().unwrap(),
        before_accepted_json
    );

    let (foreign_base, foreign_points, _) = two_link_session();
    let foreign = accepted_two_link_preview(&foreign_base, foreign_points[2]);
    assert!(matches!(
        session.apply_point_position_from_preview(
            session.design_identity(),
            points[2],
            position,
            &foreign
        ),
        Err(DocumentSessionError::PreviewForeignDocument)
    ));
    assert_eq!(session.design_identity(), before_design);
    assert_eq!(session.last_attempt().identity(), before_attempt);
    assert_eq!(
        session.accepted_state().unwrap().identity(),
        before_accepted
    );
    assert_eq!(session.export_design_json().unwrap(), before_design_json);
    assert_eq!(
        session.export_accepted_json().unwrap(),
        before_accepted_json
    );
}

#[test]
fn branch_preview_publication_rejects_tampered_missing_and_extra_edits_atomically() {
    let (session, points, curves) = two_link_session();
    let diagonal = 0.5_f64.sqrt();
    let branches = [
        DocumentCurveBranchEdit {
            curve: CurveSpan::line(curves[0]),
            direction: [diagonal, -diagonal],
        },
        DocumentCurveBranchEdit {
            curve: CurveSpan::line(curves[1]),
            direction: [diagonal, diagonal],
        },
    ];
    let position = [1.0, -1.0];
    let mut preview = session.clone();
    preview
        .attempt_point_and_curve_branches(preview.design_identity(), points[1], position, &branches)
        .expect("accepted exact branch preview");
    assert_eq!(
        preview
            .accepted_state()
            .unwrap()
            .document()
            .point(points[1])
            .unwrap()
            .position
            .map(f64::to_bits),
        position.map(f64::to_bits)
    );

    let malformed = [
        vec![branches[0]],
        vec![
            branches[0],
            DocumentCurveBranchEdit {
                curve: branches[1].curve,
                direction: [1.0, 0.0],
            },
        ],
        vec![branches[0], branches[1], branches[0]],
    ];
    for candidate_branches in malformed {
        let mut attempt = session.clone();
        let before = (
            attempt.design_identity(),
            attempt.last_attempt().identity(),
            attempt.accepted_state().unwrap().identity(),
            attempt.export_design_json().unwrap(),
            attempt.export_accepted_json().unwrap(),
        );
        assert!(matches!(
            attempt.apply_point_and_curve_branches_from_preview(
                attempt.design_identity(),
                points[1],
                position,
                &candidate_branches,
                &preview,
            ),
            Err(DocumentSessionError::PreviewBranchMismatch)
        ));
        assert_eq!(attempt.design_identity(), before.0);
        assert_eq!(attempt.last_attempt().identity(), before.1);
        assert_eq!(attempt.accepted_state().unwrap().identity(), before.2);
        assert_eq!(attempt.export_design_json().unwrap(), before.3);
        assert_eq!(attempt.export_accepted_json().unwrap(), before.4);
    }

    let ghost_json = preview.export_accepted_json().unwrap();
    let mut accepted = session;
    let outcome = accepted
        .apply_point_and_curve_branches_from_preview(
            accepted.design_identity(),
            points[1],
            position,
            &branches,
            &preview,
        )
        .expect("untampered complete branch publication");
    assert!(outcome.published_accepted_identity().is_some());
    assert_eq!(accepted.export_accepted_json().unwrap(), ghost_json);
}

#[test]
fn branch_preview_seeded_canonical_attempt_requires_exact_branch_payload() {
    let (session, points, curves) = two_link_session();
    let diagonal = 0.5_f64.sqrt();
    let search_branches = [
        DocumentCurveBranchEdit {
            curve: CurveSpan::line(curves[0]),
            direction: [diagonal, -diagonal],
        },
        DocumentCurveBranchEdit {
            curve: CurveSpan::line(curves[1]),
            direction: [diagonal, diagonal],
        },
    ];
    let position = [1.0, -1.0];
    let mut seed_preview = session.clone();
    seed_preview
        .attempt_point_and_curve_branches(
            seed_preview.design_identity(),
            points[1],
            position,
            &search_branches,
        )
        .expect("accepted branch seed");
    let exact_branches = search_branches.map(|branch| DocumentCurveBranchEdit {
        curve: branch.curve,
        direction: seed_preview
            .design_document()
            .curve_branch_direction(branch.curve)
            .expect("seed branch"),
    });

    let mut canonical = session.clone();
    let outcome = canonical
        .attempt_point_and_curve_branches_with_preview_seed_controlled(
            canonical.design_identity(),
            points[1],
            position,
            &exact_branches,
            &seed_preview,
            OperationControl::unlimited(),
        )
        .expect("preview-seeded canonical attempt");
    let OperationOutcome::Completed { value, .. } = outcome else {
        panic!("unlimited canonical attempt stopped");
    };
    assert!(value.published_accepted_identity().is_some());
    assert_eq!(
        canonical.export_accepted_json().unwrap(),
        seed_preview.export_accepted_json().unwrap()
    );

    let mut mismatched = exact_branches;
    mismatched[0].direction = [1.0, 0.0];
    let mut rejected = session;
    let before = (
        rejected.design_identity(),
        rejected.last_attempt().identity(),
        rejected.accepted_state().unwrap().identity(),
        rejected.export_design_json().unwrap(),
        rejected.export_accepted_json().unwrap(),
    );
    assert!(matches!(
        rejected.attempt_point_and_curve_branches_with_preview_seed_controlled(
            rejected.design_identity(),
            points[1],
            position,
            &mismatched,
            &seed_preview,
            OperationControl::unlimited(),
        ),
        Err(DocumentSessionError::PreviewBranchMismatch)
    ));
    assert_eq!(rejected.design_identity(), before.0);
    assert_eq!(rejected.last_attempt().identity(), before.1);
    assert_eq!(rejected.accepted_state().unwrap().identity(), before.2);
    assert_eq!(rejected.export_design_json().unwrap(), before.3);
    assert_eq!(rejected.export_accepted_json().unwrap(), before.4);
}

#[test]
fn branch_preview_rejects_signed_zero_divergent_branch_bytes_atomically() {
    let (mut session, points, curves) = two_link_session();
    let preview_branches = [DocumentCurveBranchEdit {
        curve: CurveSpan::line(curves[1]),
        direction: [-0.0, -1.0],
    }];
    let requested_branches = [DocumentCurveBranchEdit {
        curve: CurveSpan::line(curves[1]),
        direction: [0.0, -1.0],
    }];
    let position = session
        .accepted_state()
        .unwrap()
        .document()
        .point(points[2])
        .unwrap()
        .position;
    let mut preview = session.clone();
    preview
        .attempt_point_and_curve_branches(
            preview.design_identity(),
            points[2],
            position,
            &preview_branches,
        )
        .expect("signed-zero branch preview");
    assert_eq!(
        session.design_document(),
        preview.design_document(),
        "ordinary floating equality deliberately cannot distinguish the branch"
    );
    assert_ne!(
        session.export_design_json().unwrap(),
        preview.export_design_json().unwrap(),
        "exact serialized bytes must preserve the branch sign bit"
    );

    let before = (
        session.design_identity(),
        session.last_attempt().identity(),
        session.accepted_state().unwrap().identity(),
        session.export_design_json().unwrap(),
        session.export_accepted_json().unwrap(),
    );
    assert!(matches!(
        session.apply_point_and_curve_branches_from_preview(
            session.design_identity(),
            points[2],
            position,
            &requested_branches,
            &preview,
        ),
        Err(DocumentSessionError::PreviewBranchMismatch)
    ));
    assert_eq!(session.design_identity(), before.0);
    assert_eq!(session.last_attempt().identity(), before.1);
    assert_eq!(session.accepted_state().unwrap().identity(), before.2);
    assert_eq!(session.export_design_json().unwrap(), before.3);
    assert_eq!(session.export_accepted_json().unwrap(), before.4);
}

#[test]
fn branch_preview_rejects_same_document_stale_design_revision_atomically() {
    let (mut session, points, curves) = two_link_session();
    let diagonal = 0.5_f64.sqrt();
    let branches = [
        DocumentCurveBranchEdit {
            curve: CurveSpan::line(curves[0]),
            direction: [diagonal, -diagonal],
        },
        DocumentCurveBranchEdit {
            curve: CurveSpan::line(curves[1]),
            direction: [diagonal, diagonal],
        },
    ];
    let mut preview = session.clone();
    preview
        .attempt_point_and_curve_branches(
            preview.design_identity(),
            points[1],
            [1.0, -1.0],
            &branches,
        )
        .expect("accepted branch preview");
    let preview_accepted = preview.accepted_state().expect("preview accepted");
    let preview_position = preview_accepted
        .document()
        .point(points[1])
        .expect("preview elbow")
        .position;

    let base_accepted = session.accepted_state().expect("base accepted").identity();
    session
        .apply(
            session.design_identity(),
            DocumentEdit::CreateConstraint {
                label: "rejected divergent edit".into(),
                definition: DocumentConstraintDefinition::FixedPoint {
                    point: points[2],
                    target: [5.0, 5.0],
                },
            },
        )
        .expect("retained rejected edit");
    assert!(session.last_attempt().accepted_state_identity().is_none());
    assert_eq!(
        session.accepted_state().expect("retained base").identity(),
        base_accepted
    );
    assert_eq!(
        preview.design_identity(),
        session.design_identity(),
        "divergent same-document clones can occupy the same revision number"
    );

    let before = (
        session.design_identity(),
        session.last_attempt().identity(),
        session.accepted_state().expect("base accepted").identity(),
        session.export_design_json().expect("design JSON"),
        session.export_accepted_json().expect("accepted JSON"),
    );
    assert!(matches!(
        session.apply_point_and_curve_branches_from_preview(
            session.design_identity(),
            points[1],
            preview_position,
            &branches,
            &preview,
        ),
        Err(DocumentSessionError::PreviewStaleDesign)
    ));
    assert_eq!(session.design_identity(), before.0);
    assert_eq!(session.last_attempt().identity(), before.1);
    assert_eq!(
        session.accepted_state().expect("base accepted").identity(),
        before.2
    );
    assert_eq!(session.export_design_json().expect("design JSON"), before.3);
    assert_eq!(
        session.export_accepted_json().expect("accepted JSON"),
        before.4
    );
}

#[test]
fn preview_seeded_point_apply_rejects_rejected_latest_preview_without_mutation() {
    let (mut session, points, _) = two_link_session();
    let conflict = DocumentEdit::CreateConstraint {
        label: "impossible fixed end".into(),
        definition: DocumentConstraintDefinition::FixedPoint {
            point: points[2],
            target: [5.0, 5.0],
        },
    };
    session.apply(session.design_identity(), conflict).unwrap();
    let rejected = session.clone();
    assert_eq!(session.design_identity(), rejected.design_identity());
    assert!(rejected.last_attempt().accepted_state_identity().is_none());
    let before_design = session.design_identity();
    let before_attempt = session.last_attempt().identity();
    let before_accepted = session.accepted_state().unwrap().identity();
    let before_design_json = session.export_design_json().unwrap();
    let before_accepted_json = session.export_accepted_json().unwrap();
    let position = rejected
        .accepted_state()
        .unwrap()
        .document()
        .point(points[2])
        .unwrap()
        .position;

    assert!(matches!(
        session.apply_point_position_from_preview(
            session.design_identity(),
            points[2],
            position,
            &rejected
        ),
        Err(DocumentSessionError::PreviewNotAccepted)
    ));
    assert_eq!(session.design_identity(), before_design);
    assert_eq!(session.last_attempt().identity(), before_attempt);
    assert_eq!(
        session.accepted_state().unwrap().identity(),
        before_accepted
    );
    assert_eq!(session.export_design_json().unwrap(), before_design_json);
    assert_eq!(
        session.export_accepted_json().unwrap(),
        before_accepted_json
    );
}

#[test]
fn separate_design_and_accepted_v4_graphs_restore_without_lifecycle_fields() {
    let (mut document, rectangle) = rectangle_design();
    let target = add_width_five_target(&mut document);
    let mut session = RetainedSketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let draft_point = *session
        .transact(session.design_identity(), |candidate| {
            let draft_point = candidate.add_point("restored draft", [9.0, 9.0])?;
            candidate.add_dimension(
                "width-5",
                DocumentDimensionDefinition::CurveLength {
                    curve: CurveSpan::line(rectangle.curves[0]),
                    target,
                },
                DocumentDimensionMode::Driving,
            )?;
            Ok(draft_point)
        })
        .unwrap()
        .value();
    let design_json = session.export_design_json().unwrap();
    let accepted_json = session.export_accepted_json().unwrap().unwrap();
    let revisions = session.revision_high_water();
    assert_ne!(design_json, accepted_json);
    assert!(!design_json.contains("design_revision"));
    assert!(!design_json.contains("attempt_revision"));
    assert!(!design_json.contains("accepted_revision"));

    let restored = RetainedSketchDocumentSession::restore_design_with_accepted(
        SketchDocument::from_json(&design_json).unwrap(),
        SketchDocument::from_json(&accepted_json).unwrap(),
        revisions,
        DocumentSolveRequest::default().with_drag(draft_point, [10.0, 10.0]),
        SolverConfig::default(),
    )
    .unwrap();
    assert_eq!(restored.export_design_json().unwrap(), design_json);
    assert_eq!(
        restored.export_accepted_json().unwrap().unwrap(),
        accepted_json
    );
    assert_eq!(restored.design_identity().revision().get(), 3);
    assert_eq!(restored.last_attempt().identity().revision().get(), 3);
    assert!(!restored.last_attempt().solve_result().unwrap().accepted());
    assert_eq!(
        restored
            .accepted_state()
            .unwrap()
            .design_identity()
            .revision()
            .get(),
        2
    );
    assert_eq!(
        restored
            .accepted_state()
            .unwrap()
            .identity()
            .revision()
            .get(),
        1
    );
}

#[test]
fn signed_zero_distinct_design_restore_uses_exact_serialized_bytes() {
    let mut document = SketchDocument::new(1.0).unwrap();
    let point = document.add_point("free", [0.0, 0.0]).unwrap();
    let session = RetainedSketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let accepted = session.accepted_state().unwrap().document().clone();
    let mut design = accepted.clone();
    design.set_point_position(point, [-0.0, 0.0]).unwrap();
    assert_eq!(
        design, accepted,
        "ordinary floating equality deliberately cannot distinguish the fixture"
    );
    let design_json = design.to_canonical_json().unwrap();
    let accepted_json = accepted.to_canonical_json().unwrap();
    assert_ne!(design_json, accepted_json);

    let restored = RetainedSketchDocumentSession::restore_design_with_accepted(
        design,
        accepted,
        session.revision_high_water(),
        DocumentSolveRequest::default()
            .without_temporary_targets()
            .without_previous_state_preferences(),
        SolverConfig::default(),
    )
    .unwrap();
    assert_eq!(
        restored.export_design_json().unwrap(),
        design_json,
        "restore must retain the exact cross-process design bytes"
    );
    assert_eq!(
        restored.export_accepted_json().unwrap().unwrap(),
        accepted_json,
        "restore must independently reproduce the exact accepted bytes"
    );
}

#[test]
fn unsolved_restore_retains_prior_accepted_revision_high_water() {
    let (document, rectangle) = rectangle_design();
    let accepted_session = RetainedSketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let revisions = accepted_session.revision_high_water();
    let mut conflicting = accepted_session.design_document().clone();
    let target = add_width_five_target(&mut conflicting);
    let dimension = add_width_five_dimension(&mut conflicting, rectangle.curves[0], target);
    let source = conflicting.dimension(dimension).unwrap().source_id;

    let mut restored = RetainedSketchDocumentSession::restore_design(
        conflicting,
        revisions,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    assert!(restored.accepted_state().is_none());
    assert_eq!(restored.revision_high_water().accepted().unwrap().get(), 0);

    let repaired = restored
        .apply(
            restored.design_identity(),
            DocumentEdit::SetSourceSuppressed {
                source,
                suppressed: true,
            },
        )
        .unwrap();
    assert_eq!(
        repaired
            .published_accepted_identity()
            .unwrap()
            .revision()
            .get(),
        1,
        "restoration without accepted bytes must still not reuse accepted revision zero"
    );
}

#[test]
fn same_design_restore_evaluates_the_supplied_request() {
    let mut document = SketchDocument::new(1.0).unwrap();
    let point = document.add_point("free", [0.0, 0.0]).unwrap();
    let session = RetainedSketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let graph = session.accepted_state().unwrap().document().clone();
    let request = DocumentSolveRequest::default().with_drag(point, [5.0, 6.0]);

    let restored = RetainedSketchDocumentSession::restore_design_with_accepted(
        graph.clone(),
        graph,
        session.revision_high_water(),
        request,
        SolverConfig::default(),
    )
    .unwrap();
    assert_eq!(restored.request(), request);
    assert_eq!(restored.last_attempt().input().candidate_request(), request);
    assert_eq!(
        restored
            .accepted_state()
            .unwrap()
            .document()
            .point(point)
            .unwrap()
            .position
            .map(f64::to_bits),
        [5.0, 6.0].map(f64::to_bits)
    );
}
