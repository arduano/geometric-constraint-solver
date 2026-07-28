// SPDX-License-Identifier: GPL-3.0-or-later

#![allow(clippy::float_cmp)]

use geosolve_sketch::{
    CancellationToken, ContactNeighborhood, CurveDefinition, CurveFilletParentRequest, CurveSpan,
    DocumentArcSweep, DocumentConstraintDefinition, DocumentCurveNormalSide, DocumentCurveTrimView,
    DocumentDimensionMode, DocumentError, DocumentFilletEndpointOrder, DocumentFilletTrimEndpoint,
    DocumentObjectId, DocumentSolveRequest, DocumentTrimBoundary, DocumentTrimParameter,
    OperationControl, OperationLimits, OperationOutcome, RetainedSketchDocumentSession,
    ScalarDomain, ScalarUnit, SketchDocument, SolverConfig, VisualProfileOptions,
    VisualProfileStatus, cancellation_pair,
};
use geosolve_sketch_ops::{
    LineEndpoint, PreparedSketchOperation, SketchOperationApplyError,
    SketchOperationIncompleteReason, SketchOperationKind, SketchOperationProposal,
    SketchOperationRequest, SketchOperationResult, SketchOperationSnapshot,
    SketchOperationUnsupportedReason, SplitRetainedPiece, TrimRetainedSide,
};

fn session(document: SketchDocument) -> RetainedSketchDocumentSession {
    RetainedSketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap()
}

fn line(
    document: &mut SketchDocument,
    label: &str,
    start: [f64; 2],
    end: [f64; 2],
) -> (
    geosolve_sketch::CurveId,
    [geosolve_sketch::DesignPointId; 2],
) {
    let points = [
        document.add_point(format!("{label}.start"), start).unwrap(),
        document.add_point(format!("{label}.end"), end).unwrap(),
    ];
    let delta = [end[0] - start[0], end[1] - start[1]];
    let length = delta[0].hypot(delta[1]);
    let curve = document
        .add_curve(
            label,
            CurveDefinition::Line {
                start: points[0],
                end: points[1],
                branch_direction: [delta[0] / length, delta[1] / length],
            },
        )
        .unwrap();
    (curve, points)
}

fn proposed(
    session: &RetainedSketchDocumentSession,
    request: SketchOperationRequest,
) -> SketchOperationProposal {
    let outcome = SketchOperationSnapshot::capture(session)
        .prepare(request)
        .execute(OperationControl::default())
        .unwrap();
    let OperationOutcome::Completed { value, .. } = outcome else {
        panic!("uncontrolled operation must complete");
    };
    let SketchOperationResult::Proposed(proposal) = value else {
        panic!("proposal expected");
    };
    *proposal
}

#[test]
fn split_break_and_trim_publish_ordered_multi_interval_visibility() {
    let mut document = SketchDocument::new(10.0).unwrap();
    let (curve, _) = line(&mut document, "support", [0.0, 0.0], [10.0, 0.0]);
    let support = CurveSpan::line(curve);
    let mut session = session(document);

    let split = proposed(
        &session,
        SketchOperationRequest::Split {
            support,
            parameter: 0.5,
            retained: SplitRetainedPiece::Before,
        },
    );
    assert_eq!(split.input(), session.prepared_input());
    let outcome = split.apply(&mut session).unwrap();
    assert!(outcome.published_accepted_identity().is_some());
    let intervals = session
        .design_document()
        .visible_intervals(support)
        .unwrap();
    assert_eq!(intervals.len(), 2);
    assert_eq!([intervals[0].start, intervals[0].end], [0.0, 0.5]);
    assert_eq!([intervals[1].start, intervals[1].end], [0.5, 1.0]);
    assert!(session.design_document().visible_interval(support).is_err());

    proposed(
        &session,
        SketchOperationRequest::Break {
            support,
            start: 0.1,
            end: 0.2,
            retained: SplitRetainedPiece::After,
        },
    )
    .apply(&mut session)
    .unwrap();
    assert_eq!(
        session
            .design_document()
            .visible_intervals(support)
            .unwrap()
            .iter()
            .map(|interval| [interval.start, interval.end])
            .collect::<Vec<_>>(),
        vec![[0.0, 0.1], [0.2, 0.5], [0.5, 1.0]]
    );

    proposed(
        &session,
        SketchOperationRequest::Trim {
            support,
            parameter: 0.3,
            retained: TrimRetainedSide::After,
        },
    )
    .apply(&mut session)
    .unwrap();
    assert_eq!(
        session
            .design_document()
            .visible_intervals(support)
            .unwrap()
            .iter()
            .map(|interval| [interval.start, interval.end])
            .collect::<Vec<_>>(),
        vec![[0.3, 0.5], [0.5, 1.0]]
    );
    let profiles = session
        .accepted_state()
        .unwrap()
        .document()
        .analyze_visual_profiles(VisualProfileOptions::default());
    assert!(profiles.issues.is_empty());
    assert!(session.design_document().to_canonical_json().is_err());
    let draft = session.design_document().to_draft_v5_json().unwrap();
    assert_eq!(
        SketchDocument::from_draft_v5_json(&draft)
            .unwrap()
            .visible_intervals(support)
            .unwrap()
            .len(),
        2
    );
}

#[test]
fn prepared_proposals_are_exact_input_cas_and_worker_movable() {
    fn assert_send<T: Send>() {}
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send::<PreparedSketchOperation>();
    assert_send_sync::<SketchOperationRequest>();
    assert_send_sync::<SketchOperationProposal>();

    let document = SketchDocument::new(10.0).unwrap();
    let mut session = session(document);
    let proposal = proposed(
        &session,
        SketchOperationRequest::Rectangle {
            label: "prepared rectangle".into(),
            origin: [0.0, 0.0],
            width: 4.0,
            height: 3.0,
        },
    );
    session
        .transact(session.design_identity(), |document| {
            document.add_point("winner", [20.0, 20.0])?;
            Ok(())
        })
        .unwrap();
    let before = session.design_document().clone();
    assert!(matches!(
        proposal.apply(&mut session),
        Err(SketchOperationApplyError::StaleInput { .. })
    ));
    assert_eq!(session.design_document(), &before);
}

#[test]
fn cancellation_and_work_exhaustion_produce_no_proposal() {
    let mut document = SketchDocument::new(10.0).unwrap();
    let (source, _) = line(&mut document, "source", [0.0, 0.0], [1.0, 0.0]);
    let session = session(document);
    let (handle, token) = cancellation_pair();
    handle.cancel();
    let before_input = session.prepared_input();
    let before_document = session.design_document().clone();
    let cancelled = SketchOperationSnapshot::capture(&session)
        .prepare(SketchOperationRequest::Rectangle {
            label: "cancelled".into(),
            origin: [0.0, 0.0],
            width: 2.0,
            height: 1.0,
        })
        .execute(OperationControl::new(token, OperationLimits::unlimited()))
        .unwrap();
    assert!(matches!(cancelled, OperationOutcome::Cancelled { .. }));
    assert_eq!(session.prepared_input(), before_input);
    assert_eq!(session.design_document(), &before_document);

    let mut limits = OperationLimits::unlimited();
    limits.document_dependency_items = 0;
    let exhausted = SketchOperationSnapshot::capture(&session)
        .prepare(SketchOperationRequest::LinearPattern {
            label: "exhausted".into(),
            sources: vec![source],
            instances: 2,
            step: [1.0, 0.0],
        })
        .execute(OperationControl::new(CancellationToken::default(), limits))
        .unwrap();
    assert!(matches!(exhausted, OperationOutcome::WorkExhausted { .. }));
    assert_eq!(session.prepared_input(), before_input);
    assert_eq!(session.design_document(), &before_document);
}

#[test]
fn line_extension_uses_matching_accepted_geometry_and_retains_identity() {
    let mut document = SketchDocument::new(10.0).unwrap();
    let (source, points) = line(&mut document, "source", [0.0, 0.0], [1.0, 0.0]);
    let (target, _) = line(&mut document, "target", [2.0, -1.0], [2.0, 1.0]);
    let mut session = session(document);
    let proposal = proposed(
        &session,
        SketchOperationRequest::ExtendLineToLine {
            line: CurveSpan::line(source),
            endpoint: LineEndpoint::End,
            target: CurveSpan::line(target),
        },
    );
    assert_eq!(
        proposal.expected_application().kind,
        SketchOperationKind::Extend
    );
    let outcome = proposal.apply(&mut session).unwrap();
    assert!(outcome.published_accepted_identity().is_some());
    assert_eq!(
        session.design_document().point(points[1]).unwrap().position,
        [2.0, 0.0]
    );
}

#[test]
fn exact_mirror_and_linear_pattern_expand_to_ordinary_geometry() {
    let mut document = SketchDocument::new(10.0).unwrap();
    let (source, _) = line(&mut document, "source", [1.0, 0.0], [2.0, 1.0]);
    let (axis, _) = line(&mut document, "axis", [0.0, -2.0], [0.0, 2.0]);
    let mut session = session(document);
    let mirrored = proposed(
        &session,
        SketchOperationRequest::Mirror {
            label: "mirror".into(),
            source,
            axis: CurveSpan::line(axis),
        },
    )
    .apply(&mut session)
    .unwrap();
    assert!(mirrored.published_accepted_identity().is_some());
    assert_eq!(session.design_document().curves().len(), 3);

    let patterned = proposed(
        &session,
        SketchOperationRequest::LinearPattern {
            label: "pattern".into(),
            sources: vec![source],
            instances: 3,
            step: [0.0, 3.0],
        },
    )
    .apply(&mut session)
    .unwrap();
    assert!(patterned.published_accepted_identity().is_some());
    assert_eq!(session.design_document().curves().len(), 5);
}

#[test]
fn unsupported_exact_family_is_typed_and_never_approximated() {
    let mut document = SketchDocument::new(10.0).unwrap();
    let center = document.add_point("center", [0.0, 0.0]).unwrap();
    let radius = document
        .add_scalar("radius", 1.0, ScalarUnit::Length, ScalarDomain::Positive)
        .unwrap();
    let circle = document
        .add_curve("circle", CurveDefinition::Circle { center, radius })
        .unwrap();
    let (axis, _) = line(&mut document, "axis", [0.0, -2.0], [0.0, 2.0]);
    let session = session(document);
    let outcome = SketchOperationSnapshot::capture(&session)
        .prepare(SketchOperationRequest::Mirror {
            label: "unsupported".into(),
            source: circle,
            axis: CurveSpan::line(axis),
        })
        .execute(OperationControl::default())
        .unwrap();
    let OperationOutcome::Completed { value, .. } = outcome else {
        panic!("operation must complete");
    };
    let SketchOperationResult::Unsupported(unsupported) = value else {
        panic!("typed unsupported result expected");
    };
    assert_eq!(
        unsupported.reason,
        SketchOperationUnsupportedReason::CurveFamily {
            curve: circle,
            operation: "mirror"
        }
    );
}

#[test]
fn chamfer_uses_ordinary_contacts_dimensions_and_owned_trim_boundaries() {
    let mut document = SketchDocument::new(10.0).unwrap();
    let corner = document.add_point("corner", [0.0, 0.0]).unwrap();
    let first_end = document.add_point("first end", [4.0, 0.0]).unwrap();
    let second_end = document.add_point("second end", [0.0, 4.0]).unwrap();
    let first = document
        .add_curve(
            "first",
            CurveDefinition::Line {
                start: corner,
                end: first_end,
                branch_direction: [1.0, 0.0],
            },
        )
        .unwrap();
    let second = document
        .add_curve(
            "second",
            CurveDefinition::Line {
                start: corner,
                end: second_end,
                branch_direction: [0.0, 1.0],
            },
        )
        .unwrap();
    let mut session = session(document);
    let outcome = proposed(
        &session,
        SketchOperationRequest::Chamfer {
            label: "corner chamfer".into(),
            first: CurveSpan::line(first),
            second: CurveSpan::line(second),
            first_distance: 1.0,
            second_distance: 1.5,
        },
    )
    .apply(&mut session)
    .unwrap();
    assert!(outcome.published_accepted_identity().is_some());
    assert_eq!(session.design_document().curves().len(), 3);
    assert!(matches!(
        session
            .design_document()
            .visible_interval(CurveSpan::line(first))
            .unwrap()
            .start_boundary,
        geosolve_sketch::DocumentTrimBoundary::ConstraintContact { .. }
    ));
    assert!(matches!(
        session
            .design_document()
            .visible_interval(CurveSpan::line(second))
            .unwrap()
            .start_boundary,
        geosolve_sketch::DocumentTrimBoundary::ConstraintContact { .. }
    ));
}

#[test]
fn associative_fillet_is_only_a_public_sketch_transaction_wrapper() {
    let mut document = SketchDocument::new(10.0).unwrap();
    let corner = document.add_point("corner", [4.0, 0.0]).unwrap();
    let first_start = document.add_point("first start", [0.0, 0.0]).unwrap();
    let second_end = document.add_point("second end", [4.0, 4.0]).unwrap();
    let first = document
        .add_curve(
            "first",
            CurveDefinition::Line {
                start: first_start,
                end: corner,
                branch_direction: [1.0, 0.0],
            },
        )
        .unwrap();
    let second = document
        .add_curve(
            "second",
            CurveDefinition::Line {
                start: corner,
                end: second_end,
                branch_direction: [0.0, 1.0],
            },
        )
        .unwrap();
    let mut session = session(document);
    let request = geosolve_sketch::CurveCurveFilletRequest {
        first: CurveFilletParentRequest {
            curve: CurveSpan::line(first),
            parameter: 0.75,
            winding: 0,
            neighborhood: ContactNeighborhood::Local {
                lower: 0.5,
                upper: 0.95,
            },
            side: DocumentCurveNormalSide::Left,
            trim_endpoint: DocumentFilletTrimEndpoint::End,
            periodic_anchor: None,
        },
        second: CurveFilletParentRequest {
            curve: CurveSpan::line(second),
            parameter: 0.25,
            winding: 0,
            neighborhood: ContactNeighborhood::Local {
                lower: 0.05,
                upper: 0.5,
            },
            side: DocumentCurveNormalSide::Left,
            trim_endpoint: DocumentFilletTrimEndpoint::Start,
            periodic_anchor: None,
        },
        endpoint_order: DocumentFilletEndpointOrder::FirstThenSecond,
        sweep: DocumentArcSweep::CounterClockwise,
        radius: 1.0,
        radius_mode: DocumentDimensionMode::Driving,
    };
    let outcome = proposed(
        &session,
        SketchOperationRequest::AssociativeFillet {
            label: "fillet".into(),
            request,
        },
    )
    .apply(&mut session)
    .unwrap();
    assert!(outcome.published_accepted_identity().is_some());
    assert_eq!(session.design_document().constraints().len(), 1);
}

#[test]
fn rectangle_polygon_and_slot_macros_are_ordinary_expansions() {
    let mut session = session(SketchDocument::new(20.0).unwrap());
    for request in [
        SketchOperationRequest::Rectangle {
            label: "rectangle".into(),
            origin: [0.0, 0.0],
            width: 4.0,
            height: 3.0,
        },
        SketchOperationRequest::RegularPolygon {
            label: "hexagon".into(),
            center: [10.0, 0.0],
            radius: 2.0,
            sides: 6,
            rotation: 0.0,
        },
        SketchOperationRequest::Slot {
            label: "slot".into(),
            first_center: [0.0, 8.0],
            second_center: [6.0, 8.0],
            radius: 1.0,
        },
    ] {
        let outcome = proposed(&session, request).apply(&mut session).unwrap();
        assert!(outcome.published_accepted_identity().is_some());
    }
    assert!(session.design_document().curves().len() >= 14);
}

#[test]
fn companion_manifest_has_only_the_accepted_one_way_dependencies() {
    let manifest = include_str!("../Cargo.toml");
    assert!(manifest.contains("geosolve-sketch ="));
    assert!(manifest.contains("geosolve-geometry ="));
    assert!(!manifest.contains("geosolve-core ="));
    assert!(!manifest.contains("geosolve-linkage ="));
    assert!(!manifest.contains("geosolve-sketch-topology ="));
}

#[test]
fn non_extending_intersection_is_incomplete_not_a_guessed_operation() {
    let mut document = SketchDocument::new(10.0).unwrap();
    let (source, _) = line(&mut document, "source", [0.0, 0.0], [1.0, 0.0]);
    let (target, _) = line(&mut document, "target", [2.0, -1.0], [2.0, 1.0]);
    let session = session(document);
    let snapshot = SketchOperationSnapshot::capture(&session);
    let request = SketchOperationRequest::ExtendLineToLine {
        line: CurveSpan::line(source),
        endpoint: LineEndpoint::Start,
        target: CurveSpan::line(target),
    };
    let outcome = snapshot
        .prepare(request)
        .execute(OperationControl::default())
        .unwrap();
    let OperationOutcome::Completed { value, .. } = outcome else {
        panic!("operation must complete");
    };
    let SketchOperationResult::Incomplete(incomplete) = value else {
        panic!("incomplete expected");
    };
    assert_eq!(
        incomplete.reason,
        SketchOperationIncompleteReason::IntersectionDoesNotExtendSelectedEndpoint
    );
}

#[test]
fn trim_replacement_rejects_out_of_order_and_overlapping_intervals_atomically() {
    let mut document = SketchDocument::new(10.0).unwrap();
    let (curve, _) = line(&mut document, "support", [0.0, 0.0], [10.0, 0.0]);
    let support = CurveSpan::line(curve);
    let original = document.clone();
    let view = |start, end| DocumentCurveTrimView {
        support,
        start: DocumentTrimBoundary::Fixed(DocumentTrimParameter {
            parameter: start,
            winding: 0,
        }),
        end: DocumentTrimBoundary::Fixed(DocumentTrimParameter {
            parameter: end,
            winding: 0,
        }),
    };

    assert!(
        document
            .replace_trim_views(support, vec![view(0.5, 1.0), view(0.0, 0.5)])
            .is_err()
    );
    assert_eq!(document, original);
    assert!(
        document
            .replace_trim_views(support, vec![view(0.0, 0.6), view(0.5, 1.0)])
            .is_err()
    );
    assert_eq!(document, original);
}

#[test]
fn frozen_v4_parser_rejects_m58_only_multi_interval_state() {
    let mut document = SketchDocument::new(10.0).unwrap();
    let (curve, _) = line(&mut document, "support", [0.0, 0.0], [10.0, 0.0]);
    let support = CurveSpan::line(curve);
    let boundary = |parameter| {
        DocumentTrimBoundary::Fixed(DocumentTrimParameter {
            parameter,
            winding: 0,
        })
    };
    document
        .replace_trim_views(
            support,
            vec![
                DocumentCurveTrimView {
                    support,
                    start: boundary(0.0),
                    end: boundary(0.25),
                },
                DocumentCurveTrimView {
                    support,
                    start: boundary(0.75),
                    end: boundary(1.0),
                },
            ],
        )
        .unwrap();
    let draft: serde_json::Value =
        serde_json::from_str(&document.to_draft_v5_json().unwrap()).unwrap();
    let disguised_v4 = serde_json::to_string(&draft["document"]).unwrap();
    assert!(matches!(
        SketchDocument::from_json(&disguised_v4),
        Err(DocumentError::UnsupportedM58State)
    ));
}

#[test]
fn exact_split_parameter_preserves_profile_adjacency_without_proximity_welding() {
    let mut document = SketchDocument::new(10.0).unwrap();
    let rectangle = document
        .add_rectangle("profile", [0.0, 0.0], 4.0, 3.0)
        .unwrap();
    let support = CurveSpan::line(rectangle.curves[0]);
    let mut session = session(document);
    proposed(
        &session,
        SketchOperationRequest::Split {
            support,
            parameter: 0.375,
            retained: SplitRetainedPiece::Before,
        },
    )
    .apply(&mut session)
    .unwrap();

    let profiles = session
        .accepted_state()
        .unwrap()
        .document()
        .analyze_visual_profiles(VisualProfileOptions::default());
    assert_eq!(profiles.status, VisualProfileStatus::Complete);
    assert_eq!(profiles.faces.len(), 1);
    assert!(profiles.issues.is_empty());
}

#[test]
fn deleting_a_chamfer_contact_owner_freezes_its_visible_boundary() {
    let mut document = SketchDocument::new(10.0).unwrap();
    let corner = document.add_point("corner", [0.0, 0.0]).unwrap();
    let first_end = document.add_point("first end", [4.0, 0.0]).unwrap();
    let second_end = document.add_point("second end", [0.0, 4.0]).unwrap();
    let first = document
        .add_curve(
            "first",
            CurveDefinition::Line {
                start: corner,
                end: first_end,
                branch_direction: [1.0, 0.0],
            },
        )
        .unwrap();
    let second = document
        .add_curve(
            "second",
            CurveDefinition::Line {
                start: corner,
                end: second_end,
                branch_direction: [0.0, 1.0],
            },
        )
        .unwrap();
    let mut session = session(document);
    proposed(
        &session,
        SketchOperationRequest::Chamfer {
            label: "chamfer".into(),
            first: CurveSpan::line(first),
            second: CurveSpan::line(second),
            first_distance: 1.0,
            second_distance: 1.0,
        },
    )
    .apply(&mut session)
    .unwrap();
    let boundary = session
        .design_document()
        .visible_interval(CurveSpan::line(first))
        .unwrap()
        .start_boundary;
    let DocumentTrimBoundary::ConstraintContact { owner, contact } = boundary else {
        panic!("chamfer must own the selected trim boundary");
    };

    session
        .transact(session.design_identity(), |document| {
            document.remove_with_owned_state(DocumentObjectId::Constraint(owner))
        })
        .unwrap();
    assert!(session.design_document().constraint(owner).is_none());
    assert!(session.design_document().contact(contact).is_none());
    assert!(matches!(
        session
            .design_document()
            .visible_interval(CurveSpan::line(first))
            .unwrap()
            .start_boundary,
        DocumentTrimBoundary::Fixed(_)
    ));
}

#[test]
fn identical_snapshots_produce_the_same_proposal_mapping() {
    let session = session(SketchDocument::new(10.0).unwrap());
    let request = SketchOperationRequest::Rectangle {
        label: "deterministic".into(),
        origin: [1.0, 2.0],
        width: 4.0,
        height: 3.0,
    };
    let first = proposed(&session, request.clone());
    let second = proposed(&session, request);
    assert_eq!(first.input(), second.input());
    assert_eq!(first.request(), second.request());
    assert_eq!(first.expected_application(), second.expected_application());
}

#[test]
fn geometry_operations_reject_an_accepted_state_for_an_older_design() {
    let mut document = SketchDocument::new(10.0).unwrap();
    let (source, points) = line(&mut document, "source", [0.0, 0.0], [1.0, 0.0]);
    let (axis, _) = line(&mut document, "axis", [0.0, -2.0], [0.0, 2.0]);
    document
        .add_constraint(
            "fixed start",
            DocumentConstraintDefinition::FixedPoint {
                point: points[0],
                target: [0.0, 0.0],
            },
        )
        .unwrap();
    let mut session = session(document);
    let accepted = session.accepted_state().unwrap().identity();
    let rejected = session
        .transact(session.design_identity(), |document| {
            document.add_constraint(
                "conflicting start",
                DocumentConstraintDefinition::FixedPoint {
                    point: points[0],
                    target: [2.0, 0.0],
                },
            )?;
            Ok(())
        })
        .unwrap();
    assert!(rejected.published_accepted_identity().is_none());
    assert_eq!(session.accepted_state().unwrap().identity(), accepted);

    let outcome = SketchOperationSnapshot::capture(&session)
        .prepare(SketchOperationRequest::Mirror {
            label: "stale accepted geometry".into(),
            source,
            axis: CurveSpan::line(axis),
        })
        .execute(OperationControl::default())
        .unwrap();
    let OperationOutcome::Completed { value, .. } = outcome else {
        panic!("operation must complete");
    };
    let SketchOperationResult::Incomplete(incomplete) = value else {
        panic!("foreign accepted design must be incomplete");
    };
    assert_eq!(
        incomplete.reason,
        SketchOperationIncompleteReason::AcceptedStateForDifferentDesign
    );
}

#[test]
fn invalid_numeric_and_resource_inputs_fail_before_any_session_mutation() {
    let mut document = SketchDocument::new(10.0).unwrap();
    let (source, _) = line(&mut document, "source", [0.0, 0.0], [1.0, 0.0]);
    let session = session(document);
    let before_input = session.prepared_input();
    let before_document = session.design_document().clone();
    let before_accepted = session
        .accepted_state()
        .map(geosolve_sketch::SketchAcceptedDocumentState::identity);
    for request in [
        SketchOperationRequest::Rectangle {
            label: "nonfinite".into(),
            origin: [f64::NAN, 0.0],
            width: 1.0,
            height: 1.0,
        },
        SketchOperationRequest::RegularPolygon {
            label: "too many sides".into(),
            center: [0.0, 0.0],
            radius: 1.0,
            sides: 257,
            rotation: 0.0,
        },
        SketchOperationRequest::LinearPattern {
            label: "too many instances".into(),
            sources: vec![source],
            instances: 257,
            step: [1.0, 0.0],
        },
    ] {
        assert!(
            SketchOperationSnapshot::capture(&session)
                .prepare(request)
                .execute(OperationControl::default())
                .is_err()
        );
    }
    assert_eq!(session.prepared_input(), before_input);
    assert_eq!(session.design_document(), &before_document);
    assert_eq!(
        session
            .accepted_state()
            .map(geosolve_sketch::SketchAcceptedDocumentState::identity),
        before_accepted
    );
}
