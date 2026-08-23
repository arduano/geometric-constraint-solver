// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::BTreeSet;
use std::sync::Arc;

use geosolve_sketch::{
    CancellationToken, ContactNeighborhood, CurveDefinition, CurveFilletParentRequest, CurveSpan,
    DocumentArcSweep, DocumentCurveNormalSide, DocumentDimensionMode, DocumentFaceOffsetDirection,
    DocumentFilletEndpointOrder, DocumentFilletTrimEndpoint, DocumentLineSide,
    DocumentOffsetTraversal, DocumentSolveRequest, OperationControl, OperationLimits,
    OperationOutcome, RetainedSketchDocumentSession, SketchDocument, SketchMaterializationBatch,
    SketchMaterializationIdentityKind, SketchMaterializationIdentityReservation,
    SketchMaterializationReservationAllocator, SketchMaterializationReservationSet, SolverConfig,
    cancellation_pair,
};
use geosolve_sketch_ops::{
    LineEndpoint, SketchOperationIdentityChange, SketchOperationOutputPlan,
    SketchOperationProposal, SketchOperationRequest, SketchOperationResult,
    SketchOperationSnapshot, SketchProfileOffsetOperand, SplitRetainedPiece, TrimRetainedSide,
};
use geosolve_sketch_topology::{
    OffsetDirectedSpan, OffsetOperandRequest, OffsetTraversal, PreparedOffsetOperandQuery,
};

fn retained(document: SketchDocument) -> RetainedSketchDocumentSession {
    RetainedSketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .expect("retained session")
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
        document
            .add_point(format!("{label}.start"), start)
            .expect("start"),
        document
            .add_point(format!("{label}.end"), end)
            .expect("end"),
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
        .expect("line");
    (curve, points)
}

fn proposal(
    session: &RetainedSketchDocumentSession,
    request: SketchOperationRequest,
) -> SketchOperationProposal {
    let outcome = SketchOperationSnapshot::capture(session)
        .prepare(request)
        .execute(OperationControl::unlimited())
        .expect("operation preparation");
    let OperationOutcome::Completed { value, .. } = outcome else {
        panic!("unbounded operation preparation must complete");
    };
    let SketchOperationResult::Proposed(proposal) = value else {
        panic!("operation proposal expected");
    };
    *proposal
}

fn reserve_output_plan(
    document: &SketchDocument,
    plan: &SketchOperationOutputPlan,
) -> SketchMaterializationReservationSet {
    let mut allocator =
        SketchMaterializationReservationAllocator::new(document.persistent_identity_high_water())
            .expect("reservation allocator");
    for slot in &plan.slots {
        match slot.kind {
            SketchMaterializationIdentityKind::Point => {
                allocator.reserve_point().expect("point reservation");
            }
            SketchMaterializationIdentityKind::Scalar => {
                allocator.reserve_scalar().expect("scalar reservation");
            }
            SketchMaterializationIdentityKind::Curve => {
                allocator.reserve_curve().expect("curve reservation");
            }
            SketchMaterializationIdentityKind::Contact => {
                allocator.reserve_contact().expect("contact reservation");
            }
            SketchMaterializationIdentityKind::Constraint => {
                allocator
                    .reserve_constraint()
                    .expect("constraint reservation");
            }
            SketchMaterializationIdentityKind::Dimension => {
                allocator
                    .reserve_dimension()
                    .expect("dimension reservation");
            }
            SketchMaterializationIdentityKind::Parameter => {
                allocator
                    .reserve_parameter()
                    .expect("parameter reservation");
            }
            SketchMaterializationIdentityKind::ExternalBinding => {
                allocator
                    .reserve_external_binding()
                    .expect("external-binding reservation");
            }
            SketchMaterializationIdentityKind::SemanticCatalog
            | SketchMaterializationIdentityKind::SemanticSource => {
                panic!("ordinary sketch operations cannot generate semantic reservations");
            }
            _ => panic!("operation inventory gained an unhandled output kind"),
        }
    }
    allocator.finish().expect("operation reservations")
}

fn retire_output_plan(
    session: &mut RetainedSketchDocumentSession,
    plan: &SketchOperationOutputPlan,
) -> Vec<SketchMaterializationIdentityReservation> {
    let set = reserve_output_plan(session.design_document(), plan);
    let reservations = set.reservations().to_vec();
    if !reservations.is_empty() {
        session
            .transact(session.design_identity(), move |document| {
                document.apply_materialization_batch(
                    &SketchMaterializationBatch::retaining_unused_reservations(set),
                )
            })
            .expect("reservation retirement");
    }
    reservations
}

fn reserved_ids(
    reservations: &[SketchMaterializationIdentityReservation],
) -> BTreeSet<geosolve_sketch::PersistentId> {
    let mut ids = BTreeSet::new();
    for reservation in reservations {
        match *reservation {
            SketchMaterializationIdentityReservation::Point { id } => {
                ids.insert(id.0);
            }
            SketchMaterializationIdentityReservation::Scalar { id } => {
                ids.insert(id.0);
            }
            SketchMaterializationIdentityReservation::Curve { id } => {
                ids.insert(id.0);
            }
            SketchMaterializationIdentityReservation::Contact { id } => {
                ids.insert(id.0);
            }
            SketchMaterializationIdentityReservation::Constraint { reservation } => {
                ids.extend([reservation.constraint.0, reservation.source.0]);
            }
            SketchMaterializationIdentityReservation::Dimension { reservation } => {
                ids.extend([reservation.dimension.0, reservation.source.0]);
            }
            SketchMaterializationIdentityReservation::Parameter { id } => {
                ids.insert(id.0);
            }
            SketchMaterializationIdentityReservation::ExternalBinding { id } => {
                ids.insert(id.0);
            }
            SketchMaterializationIdentityReservation::SemanticCatalog { id }
            | SketchMaterializationIdentityReservation::SemanticSource { id, .. } => {
                ids.insert(id.0);
            }
            _ => panic!("reservation inventory gained an unhandled kind"),
        }
    }
    ids
}

fn apply_reserved_case(
    mut session: RetainedSketchDocumentSession,
    request: impl Fn(&RetainedSketchDocumentSession) -> SketchOperationRequest,
) -> RetainedSketchDocumentSession {
    let preliminary = proposal(&session, request(&session));
    let expected_plan = preliminary.output_plan().clone();
    let should_allocate = !matches!(
        expected_plan.kind,
        geosolve_sketch_ops::SketchOperationKind::Split
            | geosolve_sketch_ops::SketchOperationKind::Break
            | geosolve_sketch_ops::SketchOperationKind::Trim
            | geosolve_sketch_ops::SketchOperationKind::Extend
    );
    assert_eq!(
        !expected_plan.slots.is_empty(),
        should_allocate,
        "visibility and edit-only operations allocate nothing; constructive operations expose outputs"
    );
    let reservations = retire_output_plan(&mut session, &expected_plan);

    let proposal = proposal(&session, request(&session));
    assert_eq!(proposal.output_plan(), &expected_plan);
    let retained_high_water = session.design_document().persistent_identity_high_water();
    let outcome = proposal
        .apply_with_reserved_outputs(&mut session, &reservations)
        .expect("reserved operation application");
    assert!(outcome.published_accepted_identity().is_some());
    assert_eq!(
        session.design_document().persistent_identity_high_water(),
        retained_high_water,
        "reserved application must fill tombstones without advancing high-water"
    );

    let proposed_ids = outcome
        .value()
        .identity_changes
        .iter()
        .filter_map(|change| match change {
            SketchOperationIdentityChange::Proposed(element) => Some(element.persistent_id()),
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(proposed_ids, reserved_ids(&reservations));

    let encoded = session
        .design_document()
        .to_draft_v5_json()
        .expect("canonical operation document");
    let restored = SketchDocument::from_draft_v5_json(&encoded).expect("cold operation restore");
    assert_eq!(
        restored.persistent_identity_high_water(),
        session.design_document().persistent_identity_high_water()
    );
    for id in reserved_ids(&reservations) {
        assert_eq!(
            restored.element(id),
            session.design_document().element(id),
            "cold restore must preserve each reserved operation identity"
        );
    }
    session
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one ordered matrix keeps all twelve closed operation variants visibly exhaustive"
)]
fn all_twelve_operations_publish_authenticated_stable_output_inventories() {
    let mut split_document = SketchDocument::new(10.0).expect("document");
    let (split_curve, _) = line(&mut split_document, "split support", [0.0, 0.0], [4.0, 0.0]);
    apply_reserved_case(retained(split_document), |_| {
        SketchOperationRequest::Split {
            support: CurveSpan::line(split_curve),
            parameter: 0.5,
            retained: SplitRetainedPiece::Before,
        }
    });

    let mut break_document = SketchDocument::new(10.0).expect("document");
    let (break_curve, _) = line(&mut break_document, "break support", [0.0, 0.0], [4.0, 0.0]);
    apply_reserved_case(retained(break_document), |_| {
        SketchOperationRequest::Break {
            support: CurveSpan::line(break_curve),
            start: 0.25,
            end: 0.75,
            retained: SplitRetainedPiece::Before,
        }
    });

    let mut trim_document = SketchDocument::new(10.0).expect("document");
    let (trim_curve, _) = line(&mut trim_document, "trim support", [0.0, 0.0], [4.0, 0.0]);
    apply_reserved_case(retained(trim_document), |_| SketchOperationRequest::Trim {
        support: CurveSpan::line(trim_curve),
        parameter: 0.5,
        retained: TrimRetainedSide::After,
    });

    let mut extend_document = SketchDocument::new(10.0).expect("document");
    let (extend_curve, _) = line(
        &mut extend_document,
        "extend source",
        [0.0, 0.0],
        [1.0, 0.0],
    );
    let (extend_target, _) = line(
        &mut extend_document,
        "extend target",
        [2.0, -1.0],
        [2.0, 1.0],
    );
    apply_reserved_case(retained(extend_document), |_| {
        SketchOperationRequest::ExtendLineToLine {
            line: CurveSpan::line(extend_curve),
            endpoint: LineEndpoint::End,
            target: CurveSpan::line(extend_target),
        }
    });

    let mut mirror_document = SketchDocument::new(10.0).expect("document");
    let (mirror_source, _) = line(
        &mut mirror_document,
        "mirror source",
        [1.0, 0.0],
        [2.0, 1.0],
    );
    let (mirror_axis, _) = line(&mut mirror_document, "mirror axis", [0.0, -2.0], [0.0, 2.0]);
    apply_reserved_case(retained(mirror_document), |_| {
        SketchOperationRequest::Mirror {
            label: "reserved mirror".into(),
            source: mirror_source,
            axis: CurveSpan::line(mirror_axis),
        }
    });

    let mut chamfer_document = SketchDocument::new(10.0).expect("document");
    let corner = chamfer_document
        .add_point("corner", [0.0, 0.0])
        .expect("corner");
    let first_end = chamfer_document
        .add_point("first end", [4.0, 0.0])
        .expect("first end");
    let second_end = chamfer_document
        .add_point("second end", [0.0, 4.0])
        .expect("second end");
    let first = chamfer_document
        .add_curve(
            "first",
            CurveDefinition::Line {
                start: corner,
                end: first_end,
                branch_direction: [1.0, 0.0],
            },
        )
        .expect("first line");
    let second = chamfer_document
        .add_curve(
            "second",
            CurveDefinition::Line {
                start: corner,
                end: second_end,
                branch_direction: [0.0, 1.0],
            },
        )
        .expect("second line");
    apply_reserved_case(retained(chamfer_document), |_| {
        SketchOperationRequest::Chamfer {
            label: "reserved chamfer".into(),
            first: CurveSpan::line(first),
            second: CurveSpan::line(second),
            first_distance: 1.0,
            second_distance: 1.0,
        }
    });

    let mut fillet_document = SketchDocument::new(10.0).expect("document");
    let corner = fillet_document
        .add_point("fillet corner", [4.0, 0.0])
        .expect("corner");
    let first_start = fillet_document
        .add_point("fillet first", [0.0, 0.0])
        .expect("first");
    let second_end = fillet_document
        .add_point("fillet second", [4.0, 4.0])
        .expect("second");
    let first = fillet_document
        .add_curve(
            "fillet first line",
            CurveDefinition::Line {
                start: first_start,
                end: corner,
                branch_direction: [1.0, 0.0],
            },
        )
        .expect("first line");
    let second = fillet_document
        .add_curve(
            "fillet second line",
            CurveDefinition::Line {
                start: corner,
                end: second_end,
                branch_direction: [0.0, 1.0],
            },
        )
        .expect("second line");
    let fillet_request = geosolve_sketch::CurveCurveFilletRequest {
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
    apply_reserved_case(retained(fillet_document), |_| {
        SketchOperationRequest::AssociativeFillet {
            label: "reserved fillet".into(),
            request: fillet_request,
        }
    });

    apply_reserved_case(
        retained(SketchDocument::new(10.0).expect("document")),
        |_| SketchOperationRequest::Rectangle {
            label: "reserved rectangle".into(),
            origin: [0.0, 0.0],
            width: 4.0,
            height: 3.0,
        },
    );
    apply_reserved_case(
        retained(SketchDocument::new(10.0).expect("document")),
        |_| SketchOperationRequest::RegularPolygon {
            label: "reserved polygon".into(),
            center: [0.0, 0.0],
            radius: 2.0,
            sides: 5,
            rotation: 0.2,
        },
    );
    apply_reserved_case(
        retained(SketchDocument::new(10.0).expect("document")),
        |_| SketchOperationRequest::Slot {
            label: "reserved slot".into(),
            first_center: [0.0, 0.0],
            second_center: [4.0, 0.0],
            radius: 1.0,
        },
    );

    let mut pattern_document = SketchDocument::new(10.0).expect("document");
    let (pattern_source, _) = line(
        &mut pattern_document,
        "pattern source",
        [0.0, 0.0],
        [1.0, 0.0],
    );
    apply_reserved_case(retained(pattern_document), |_| {
        SketchOperationRequest::LinearPattern {
            label: "reserved pattern".into(),
            sources: vec![pattern_source],
            instances: 3,
            step: [0.0, 2.0],
        }
    });

    let mut offset_document = SketchDocument::new(10.0).expect("document");
    let (offset_source, _) = line(
        &mut offset_document,
        "offset source",
        [0.0, 0.0],
        [4.0, 0.0],
    );
    apply_reserved_case(retained(offset_document), |session| {
        let query = PreparedOffsetOperandQuery::capture(session, OffsetOperandRequest::default())
            .expect("offset operand query");
        let outcome = query
            .execute(OperationControl::unlimited())
            .expect("offset operand analysis");
        let OperationOutcome::Completed { value, .. } = outcome else {
            panic!("offset operand query must complete");
        };
        let index = Arc::new(value.operand_index.expect("offset operand index"));
        SketchOperationRequest::ProfileOffset {
            label: "reserved offset".into(),
            distance: 1.0,
            operand: SketchProfileOffsetOperand::OpenChain {
                spans: vec![OffsetDirectedSpan {
                    span: CurveSpan::line(offset_source),
                    traversal: OffsetTraversal::Forward,
                }],
                side: DocumentLineSide::Left,
            },
            operand_index: index,
        }
    });
}

#[test]
fn reserved_output_kind_mismatch_rejects_without_consuming_or_advancing() {
    let session = retained(SketchDocument::new(10.0).expect("document"));
    let preliminary = proposal(
        &session,
        SketchOperationRequest::Rectangle {
            label: "mismatch".into(),
            origin: [0.0, 0.0],
            width: 2.0,
            height: 1.0,
        },
    );
    assert!(!preliminary.output_plan().slots.is_empty());

    let mut session = session;
    let mut allocator = SketchMaterializationReservationAllocator::new(
        session.design_document().persistent_identity_high_water(),
    )
    .expect("allocator");
    allocator.reserve_scalar().expect("wrong reservation");
    let set = allocator.finish().expect("reservation set");
    session
        .transact(session.design_identity(), |document| {
            document.apply_materialization_batch(
                &SketchMaterializationBatch::retaining_unused_reservations(set.clone()),
            )
        })
        .expect("retire wrong reservation");
    let proposal = proposal(
        &session,
        SketchOperationRequest::Rectangle {
            label: "mismatch".into(),
            origin: [0.0, 0.0],
            width: 2.0,
            height: 1.0,
        },
    );
    let before = session.design_document().clone();
    assert!(
        proposal
            .apply_with_reserved_outputs(&mut session, set.reservations())
            .is_err()
    );
    assert_eq!(session.design_document(), &before);
}

#[test]
fn controlled_reserved_outputs_survive_cancellation_and_exhaustion_until_commit() {
    let mut session = retained(SketchDocument::new(10.0).expect("document"));
    let request = || SketchOperationRequest::Rectangle {
        label: "controlled reserved rectangle".into(),
        origin: [0.0, 0.0],
        width: 2.0,
        height: 1.0,
    };
    let preliminary = proposal(&session, request());
    let expected_plan = preliminary.output_plan().clone();
    let reservations = retire_output_plan(&mut session, &expected_plan);
    assert!(!reservations.is_empty());

    let proposal = proposal(&session, request());
    assert_eq!(proposal.output_plan(), &expected_plan);
    let before_input = session.prepared_input();
    let before_document = session.design_document().clone();
    let before_accepted = session
        .accepted_state()
        .map(geosolve_sketch::SketchAcceptedDocumentState::identity);
    let retained_high_water = before_document.persistent_identity_high_water();

    let (cancel_handle, cancel_token) = cancellation_pair();
    cancel_handle.cancel();
    let cancelled = proposal
        .apply_controlled_with_reserved_outputs(
            &mut session,
            &reservations,
            OperationControl::new(cancel_token, OperationLimits::unlimited()),
        )
        .expect("cancelled reserved application");
    assert!(matches!(cancelled, OperationOutcome::Cancelled { .. }));
    assert_eq!(session.prepared_input(), before_input);
    assert_eq!(session.design_document(), &before_document);
    assert_eq!(
        session
            .accepted_state()
            .map(geosolve_sketch::SketchAcceptedDocumentState::identity),
        before_accepted
    );

    let mut limits = OperationLimits::unlimited();
    limits.document_validation_items = 0;
    let exhausted = proposal
        .apply_controlled_with_reserved_outputs(
            &mut session,
            &reservations,
            OperationControl::new(CancellationToken::default(), limits),
        )
        .expect("exhausted reserved application");
    assert!(matches!(exhausted, OperationOutcome::WorkExhausted { .. }));
    assert_eq!(session.prepared_input(), before_input);
    assert_eq!(session.design_document(), &before_document);
    assert_eq!(
        session
            .accepted_state()
            .map(geosolve_sketch::SketchAcceptedDocumentState::identity),
        before_accepted
    );
    assert!(
        reserved_ids(&reservations)
            .iter()
            .all(|id| session.design_document().element(*id).is_none()),
        "stopped work must leave every retired output available"
    );

    let completed = proposal
        .apply_controlled_with_reserved_outputs(
            &mut session,
            &reservations,
            OperationControl::unlimited(),
        )
        .expect("completed reserved application");
    let OperationOutcome::Completed { value, .. } = completed else {
        panic!("unlimited reserved application must complete");
    };
    assert!(value.published_accepted_identity().is_some());
    assert_eq!(
        session.design_document().persistent_identity_high_water(),
        retained_high_water
    );
    assert!(
        reserved_ids(&reservations)
            .iter()
            .all(|id| session.design_document().element(*id).is_some()),
        "the later completed action must consume every original reservation"
    );
}

// Keep the public topology spellings represented in this cross-crate identity regression.
const _: Option<(DocumentFaceOffsetDirection, DocumentOffsetTraversal)> = None;
