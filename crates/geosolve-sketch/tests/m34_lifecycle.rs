use geosolve_core::{
    CancellationToken, HardValidity, OperationCheckpoint, OperationControl, OperationLimits,
    OperationOutcome, OperationStopReason, OperationWorkCounter, SecondaryStatus, SolveTermination,
    SolverConfig,
};
use geosolve_sketch::{
    AlphaScenarioIds, AlphaScenarioKind, ContactNeighborhood, ContactStateEdit, CurveDefinition,
    CurveSpan, DesignPointId, DocumentCommandEffect, DocumentConstraintDefinition,
    DocumentDimensionDefinition, DocumentDimensionMode, DocumentEdit, DocumentObjectId,
    DocumentSessionError, DocumentSolveRequest, RetainedSketchDocumentSession, ScalarDomain,
    ScalarUnit, SketchDocument, alpha_scenario,
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
fn preview_seeded_point_apply_rejects_rejected_latest_preview_without_mutation() {
    let (mut session, points, _) = two_link_session();
    let mut rejected = session.clone();
    let conflict = DocumentEdit::CreateConstraint {
        label: "impossible fixed end".into(),
        definition: DocumentConstraintDefinition::FixedPoint {
            point: points[2],
            target: [5.0, 5.0],
        },
    };
    session
        .apply(session.design_identity(), conflict.clone())
        .unwrap();
    rejected
        .apply(rejected.design_identity(), conflict)
        .unwrap();
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

#[test]
fn same_design_restore_without_a_temporary_target_preserves_exact_fillet_bytes() {
    let fixture = alpha_scenario(AlphaScenarioKind::FilletLineCircle, 1.0).unwrap();
    let AlphaScenarioIds::FilletLineCircle(ids) = fixture.ids else {
        panic!("line-circle fillet IDs")
    };
    let mut session = RetainedSketchDocumentSession::new(
        fixture.document,
        fixture.request,
        SolverConfig::default(),
    )
    .unwrap();
    let deleted = session
        .apply(
            session.design_identity(),
            DocumentEdit::Delete {
                object: DocumentObjectId::Dimension(ids.fillet.radius_dimension),
            },
        )
        .unwrap();
    assert!(deleted.published_accepted_identity().is_some());
    let accepted = session.accepted_state().unwrap().document().clone();
    let accepted_json = accepted.to_canonical_json().unwrap();
    let request = DocumentSolveRequest::default().without_previous_state_preferences();

    let restored = RetainedSketchDocumentSession::restore_design_with_accepted(
        accepted.clone(),
        accepted,
        session.revision_high_water(),
        request,
        SolverConfig::default(),
    )
    .unwrap();
    assert_eq!(restored.request(), request);
    assert_eq!(
        restored.accepted_state().unwrap().document(),
        restored.design_document()
    );
    assert_eq!(
        restored.export_accepted_json().unwrap().unwrap(),
        accepted_json
    );
}

#[test]
fn current_design_restore_rejects_tampered_same_topology_dimension_value() {
    let (document, rectangle) = rectangle_design();
    let session = RetainedSketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let accepted = session.accepted_state().unwrap().document().clone();
    let mut tampered_design = session.design_document().clone();
    tampered_design
        .set_scalar_value(rectangle.targets[0], 5.0)
        .unwrap();
    assert_eq!(tampered_design.id(), accepted.id());
    assert_eq!(
        tampered_design.effective_activity().activation_digest(),
        accepted.effective_activity().activation_digest(),
        "the same-topology target tamper must retain the activation closure"
    );

    assert!(matches!(
        RetainedSketchDocumentSession::restore_current_design_with_accepted(
            tampered_design,
            accepted,
            session.revision_high_water(),
            DocumentSolveRequest::default(),
            SolverConfig::default(),
        ),
        Err(DocumentSessionError::InvalidAcceptedSnapshot)
    ));
}

fn retained_position(session: &RetainedSketchDocumentSession, point: DesignPointId) -> [f64; 2] {
    session
        .accepted_state()
        .expect("accepted state")
        .document()
        .point(point)
        .expect("accepted point")
        .position
}

fn assert_position_near(actual: [f64; 2], expected: [f64; 2], tolerance: f64) {
    let distance = (actual[0] - expected[0]).hypot(actual[1] - expected[1]);
    assert!(
        distance <= tolerance,
        "position {actual:?} is {distance} from expected {expected:?}, tolerance {tolerance}"
    );
}

fn rotate_about(point: [f64; 2], center: [f64; 2], angle: f64) -> [f64; 2] {
    let offset = [point[0] - center[0], point[1] - center[1]];
    let (sin, cos) = angle.sin_cos();
    [
        center[0] + cos * offset[0] - sin * offset[1],
        center[1] + sin * offset[0] + cos * offset[1],
    ]
}

fn roller_center_target(parameter: f64) -> [f64; 2] {
    let tangent: [f64; 2] = [8.0, 8.0 - 16.0 * parameter];
    let tangent_norm = tangent[0].hypot(tangent[1]);
    [
        -4.0 + 8.0 * parameter - tangent[1] / tangent_norm,
        8.0 * parameter * (1.0 - parameter) + tangent[0] / tangent_norm,
    ]
}

#[test]
fn pantograph_locality_keeps_the_opposite_independent_arm_stationary() {
    let fixture = alpha_scenario(AlphaScenarioKind::MotionPantograph, 1.0).unwrap();
    let AlphaScenarioIds::MotionPantograph(ids) = fixture.ids else {
        panic!("pantograph IDs expected");
    };
    let request = fixture.request;
    let session =
        RetainedSketchDocumentSession::new(fixture.document, request, SolverConfig::default())
            .unwrap();
    let anchor = retained_position(&session, ids.anchor);
    let input = retained_position(&session, ids.input);
    let guide = retained_position(&session, ids.guide);

    for (active, passive, target) in [
        (ids.input, ids.guide, rotate_about(input, anchor, 0.08)),
        (ids.guide, ids.input, rotate_about(guide, anchor, -0.08)),
    ] {
        let plan = session.drag_locality_plan(active).expect("locality plan");
        assert_eq!(plan.passive_degrees_of_freedom(), 1);
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
        assert!(
            preview.last_attempt().accepted_state_identity().is_some(),
            "{:#?}",
            preview.last_attempt().solve_result()
        );
        assert_position_near(retained_position(&preview, active), target, 1.0e-8);
        assert_position_near(
            retained_position(&preview, passive),
            retained_position(&session, passive),
            1.0e-8,
        );
    }
}

#[test]
fn twin_roller_locality_is_symmetric_and_frozen_through_continuation() {
    let fixture = alpha_scenario(AlphaScenarioKind::MotionCam, 1.0).unwrap();
    let AlphaScenarioIds::MotionCam(ids) = fixture.ids else {
        panic!("cam IDs expected");
    };
    let request = fixture.request;
    let session =
        RetainedSketchDocumentSession::new(fixture.document, request, SolverConfig::default())
            .unwrap();

    for (active, passive, first_parameter, second_parameter) in [
        (ids.left_center, ids.right_center, 0.26, 0.28),
        (ids.right_center, ids.left_center, 0.74, 0.72),
    ] {
        let passive_start = retained_position(&session, passive);
        let active_start = retained_position(&session, active);
        let plan = session.drag_locality_plan(active).expect("locality plan");
        assert_eq!(plan.passive_degrees_of_freedom(), 1);
        assert_eq!(plan.anchor_count(), 1);

        let first_target = roller_center_target(first_parameter);
        let mut first = session.clone();
        let _ = first
            .reattempt_with_drag_locality_controlled(
                first.design_identity(),
                request
                    .with_previous_state_preferences()
                    .with_drag(active, first_target),
                &plan,
                OperationControl::unlimited(),
            )
            .unwrap();
        assert!(first.last_attempt().accepted_state_identity().is_some());
        assert_position_near(retained_position(&first, active), first_target, 5.0e-8);
        assert_position_near(retained_position(&first, passive), passive_start, 1.0e-8);

        let second_target = roller_center_target(second_parameter);
        let mut continued = session.clone();
        let _ = continued
            .reattempt_from_accepted_preview_with_drag_locality_controlled(
                continued.design_identity(),
                request
                    .with_previous_state_preferences()
                    .with_drag(active, second_target),
                &first,
                &plan,
                OperationControl::unlimited(),
            )
            .unwrap();
        assert!(continued.last_attempt().accepted_state_identity().is_some());
        assert_position_near(retained_position(&continued, active), second_target, 5.0e-8);
        assert_position_near(
            retained_position(&continued, passive),
            passive_start,
            1.0e-8,
        );
        assert!(
            (retained_position(&continued, active)[0] - active_start[0])
                .hypot(retained_position(&continued, active)[1] - active_start[1])
                > 0.05,
            "active roller did not exercise real continuation motion"
        );
    }
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one lifecycle regression keeps planning, continuation, exhaustion, release, and rollback evidence together"
)]
fn drag_locality_targets_use_accepted_geometry_and_remain_frozen() {
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

    let request = DocumentSolveRequest::default().without_previous_state_preferences();
    let session =
        RetainedSketchDocumentSession::new(document, request, SolverConfig::default()).unwrap();
    let accepted_passive = retained_position(&session, passive);
    assert!(
        (accepted_passive[0] - 3.0).hypot(accepted_passive[1]) > 0.5,
        "fixture must distinguish accepted visible geometry from its unsolved seed"
    );
    let plan = session.drag_locality_plan(active).expect("locality plan");
    assert_eq!(plan.passive_degrees_of_freedom(), 1);
    assert_eq!(plan.anchor_count(), 1);

    let target = |angle: f64| {
        [
            accepted_passive[0] - 5.0 * angle.cos(),
            accepted_passive[1] - 5.0 * angle.sin(),
        ]
    };
    let mut first = session.clone();
    let _ = first
        .reattempt_with_drag_locality_controlled(
            first.design_identity(),
            request
                .with_previous_state_preferences()
                .with_drag(active, target(0.08)),
            &plan,
            OperationControl::unlimited(),
        )
        .unwrap();
    assert!(first.last_attempt().accepted_state_identity().is_some());
    assert_position_near(retained_position(&first, passive), accepted_passive, 1.0e-8);

    let continued_target = target(0.16);
    let mut continued = session.clone();
    let _ = continued
        .reattempt_from_accepted_preview_with_drag_locality_controlled(
            continued.design_identity(),
            request
                .with_previous_state_preferences()
                .with_drag(active, continued_target),
            &first,
            &plan,
            OperationControl::unlimited(),
        )
        .unwrap();
    assert!(continued.last_attempt().accepted_state_identity().is_some());
    assert_position_near(
        retained_position(&continued, active),
        continued_target,
        1.0e-8,
    );
    assert_position_near(
        retained_position(&continued, passive),
        accepted_passive,
        1.0e-8,
    );

    let continued_position = retained_position(&continued, active);
    let continued_json = continued.export_accepted_json().unwrap();
    let mut interrupted = session.clone();
    let interrupted_before = (
        interrupted.design_identity(),
        interrupted.last_attempt().identity(),
        interrupted.accepted_state().unwrap().identity(),
        interrupted.export_design_json().unwrap(),
        interrupted.export_accepted_json().unwrap(),
    );
    let mut limits = OperationLimits::unlimited();
    limits.document_dependency_items = 0;
    let stopped = interrupted
        .apply_point_position_from_preview_with_drag_locality_controlled(
            interrupted.design_identity(),
            active,
            continued_position,
            &continued,
            &plan,
            OperationControl::new(CancellationToken::default(), limits),
        )
        .unwrap();
    let OperationOutcome::WorkExhausted { report } = stopped else {
        panic!("frozen-anchor validation must respect the document dependency budget");
    };
    assert_eq!(report.consumed.document_dependency_items, 0);
    assert_eq!(
        report.stopping_reason,
        Some(OperationStopReason::WorkExhausted {
            counter: OperationWorkCounter::DocumentDependencyItems,
            checkpoint: OperationCheckpoint::DocumentDependency,
        })
    );
    assert_eq!(interrupted.design_identity(), interrupted_before.0);
    assert_eq!(interrupted.last_attempt().identity(), interrupted_before.1);
    assert_eq!(
        interrupted.accepted_state().unwrap().identity(),
        interrupted_before.2
    );
    assert_eq!(
        interrupted.export_design_json().unwrap(),
        interrupted_before.3
    );
    assert_eq!(
        interrupted.export_accepted_json().unwrap(),
        interrupted_before.4
    );

    let mut released = session.clone();
    let outcome = released
        .apply_point_position_from_preview_with_drag_locality(
            released.design_identity(),
            active,
            continued_position,
            &continued,
            &plan,
        )
        .unwrap();
    assert!(outcome.published_accepted_identity().is_some());
    assert_eq!(
        released.export_accepted_json().unwrap(),
        continued_json,
        "release must reproduce the complete visible preview"
    );
    assert_position_near(
        retained_position(&released, passive),
        accepted_passive,
        1.0e-8,
    );
    let release_report = released
        .accepted_state()
        .unwrap()
        .solve_result()
        .unstable_core_report();
    assert_eq!(release_report.hard_termination, SolveTermination::Converged);
    assert_eq!(release_report.termination, SolveTermination::Stalled);
    assert_eq!(release_report.preference_status, SecondaryStatus::Stalled);

    let mut mismatch = session.clone();
    let before = (
        mismatch.design_identity(),
        mismatch.last_attempt().identity(),
        mismatch.accepted_state().unwrap().identity(),
        mismatch.export_design_json().unwrap(),
    );
    let mismatched_position = [
        f64::from_bits(continued_position[0].to_bits() + 1),
        continued_position[1],
    ];
    assert!(matches!(
        mismatch.apply_point_position_from_preview_with_drag_locality(
            mismatch.design_identity(),
            active,
            mismatched_position,
            &continued,
            &plan,
        ),
        Err(DocumentSessionError::PreviewPointMismatch)
    ));
    assert_eq!(mismatch.design_identity(), before.0);
    assert_eq!(mismatch.last_attempt().identity(), before.1);
    assert_eq!(mismatch.accepted_state().unwrap().identity(), before.2);
    assert_eq!(mismatch.export_design_json().unwrap(), before.3);
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one scale-loop regression keeps winding continuation, release, and explicit-edit ownership together"
)]
fn periodic_contact_winding_seeds_drag_continuation_and_preserves_explicit_edits() {
    for scale in [1.0e-6, 1.0, 1.0e6] {
        let mut document = SketchDocument::new(scale).unwrap();
        let angle = 0.1_f64;
        let (sin, cos) = angle.sin_cos();
        let center = document.add_point("circle center", [0.0, 0.0]).unwrap();
        let contact_point = document
            .add_point("fixed contact point", [scale * cos, scale * sin])
            .unwrap();
        let active = document
            .add_point("independent drag point", [3.0 * scale, 2.0 * scale])
            .unwrap();
        let radius = document
            .add_scalar(
                "circle radius",
                scale,
                ScalarUnit::Length,
                ScalarDomain::Positive,
            )
            .unwrap();
        let circle = document
            .add_curve(
                "periodic circle",
                CurveDefinition::Circle { center, radius },
            )
            .unwrap();
        for (label, point, target) in [
            ("fix circle center", center, [0.0, 0.0]),
            (
                "fix contact point",
                contact_point,
                [scale * cos, scale * sin],
            ),
        ] {
            document
                .add_constraint(
                    label,
                    DocumentConstraintDefinition::FixedPoint { point, target },
                )
                .unwrap();
        }
        let contact = document
            .add_curve_contact(
                "periodic contact",
                CurveSpan::line(circle),
                std::f64::consts::TAU - angle,
                0,
                ContactNeighborhood::Interior,
                None,
            )
            .unwrap();
        document
            .add_constraint(
                "fixed point on circle",
                DocumentConstraintDefinition::PointOnCurve {
                    point: contact_point,
                    contact,
                },
            )
            .unwrap();

        let request = DocumentSolveRequest::default().without_previous_state_preferences();
        let session =
            RetainedSketchDocumentSession::new(document, request, SolverConfig::default()).unwrap();
        let design_contact = session.design_document().contact(contact).unwrap();
        let accepted_document = session.accepted_state().unwrap().document();
        let accepted_contact = accepted_document.contact(contact).unwrap();
        let accepted_parameter = accepted_document
            .scalar(accepted_contact.parameter)
            .unwrap()
            .value;
        assert_eq!(design_contact.winding, 0);
        assert_eq!(
            accepted_contact.winding, 1,
            "scale={scale:e}: initial solve must cross the periodic seam"
        );
        assert!(
            (accepted_parameter - angle).abs() <= 1.0e-8,
            "scale={scale:e}: accepted principal parameter={accepted_parameter:e}"
        );

        let locality = session.drag_locality_plan(active).unwrap();
        assert_eq!(locality.passive_degrees_of_freedom(), 0);
        assert_eq!(locality.anchor_count(), 0);

        let first_target = [4.0 * scale, 2.5 * scale];
        let mut first = session.clone();
        let _ = first
            .reattempt_with_drag_locality_controlled(
                first.design_identity(),
                request
                    .with_previous_state_preferences()
                    .with_drag(active, first_target),
                &locality,
                OperationControl::unlimited(),
            )
            .unwrap();
        assert!(first.last_attempt().accepted_state_identity().is_some());
        assert_eq!(
            first
                .accepted_state()
                .unwrap()
                .document()
                .contact(contact)
                .unwrap()
                .winding,
            1,
            "scale={scale:e}: first drag sample lost accepted periodic winding"
        );

        let continued_target = [4.5 * scale, 3.0 * scale];
        let mut continued = session.clone();
        let _ = continued
            .reattempt_from_accepted_preview_with_drag_locality_controlled(
                continued.design_identity(),
                request
                    .with_previous_state_preferences()
                    .with_drag(active, continued_target),
                &first,
                &locality,
                OperationControl::unlimited(),
            )
            .unwrap();
        assert!(continued.last_attempt().accepted_state_identity().is_some());
        assert_eq!(
            continued
                .accepted_state()
                .unwrap()
                .document()
                .contact(contact)
                .unwrap()
                .winding,
            1,
            "scale={scale:e}: continued drag sample lost accepted periodic winding"
        );

        let continued_position = retained_position(&continued, active);
        let continued_json = continued.export_accepted_json().unwrap();
        let mut released = session.clone();
        let release = released
            .apply_point_position_from_preview_with_drag_locality(
                released.design_identity(),
                active,
                continued_position,
                &continued,
                &locality,
            )
            .unwrap();
        assert!(release.published_accepted_identity().is_some());
        assert_eq!(released.export_accepted_json().unwrap(), continued_json);
        assert_eq!(
            released
                .accepted_state()
                .unwrap()
                .document()
                .contact(contact)
                .unwrap()
                .winding,
            1,
            "scale={scale:e}: exact release lost accepted periodic winding"
        );

        let accepted_slot = session
            .accepted_state()
            .unwrap()
            .document()
            .contact(contact)
            .unwrap()
            .clone();
        let explicit_edit = |winding| ContactStateEdit {
            contact,
            value: accepted_parameter,
            winding,
            neighborhood: accepted_slot.neighborhood,
            tangent_orientation: accepted_slot.tangent_orientation,
        };

        let mut parameter_edit = session.clone();
        let parameter_outcome = parameter_edit
            .transact(parameter_edit.design_identity(), |document| {
                document.set_contact_states(&[explicit_edit(0)])
            })
            .unwrap();
        assert!(parameter_outcome.published_accepted_identity().is_some());
        assert_eq!(
            parameter_edit
                .accepted_state()
                .unwrap()
                .document()
                .contact(contact)
                .unwrap()
                .winding,
            0,
            "scale={scale:e}: explicit parameter edit inherited stale accepted winding"
        );

        let mut branch_edit = session.clone();
        let branch_outcome = branch_edit
            .transact(branch_edit.design_identity(), |document| {
                document.set_contact_states(&[explicit_edit(2)])
            })
            .unwrap();
        assert!(branch_outcome.published_accepted_identity().is_some());
        assert_eq!(
            branch_edit
                .accepted_state()
                .unwrap()
                .document()
                .contact(contact)
                .unwrap()
                .winding,
            2,
            "scale={scale:e}: explicit winding edit was overwritten"
        );
    }
}
