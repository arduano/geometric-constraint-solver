// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_sketch::{
    ContactNeighborhood, CurveCurveFilletRequest, CurveDefinition, CurveFilletParentRequest,
    CurveSpan, DesignPointId, DocumentArcSweep, DocumentConstraintDefinition,
    DocumentCurveNormalSide, DocumentDimensionMode, DocumentFilletEndpointOrder,
    DocumentFilletTrimEndpoint, DocumentSolveRequest, OperationControl, OperationLimits,
    OperationOutcome, RetainedSketchDocumentSession, SketchDocument, SolverConfig,
    cancellation_pair,
};
use geosolve_sketch_ops::{
    SketchOperationProposal, SketchOperationRequest, SketchOperationResult, SketchOperationSnapshot,
};

fn session(document: SketchDocument) -> RetainedSketchDocumentSession {
    RetainedSketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap()
}

fn fix_points(document: &mut SketchDocument, points: &[(DesignPointId, [f64; 2])]) {
    for (index, (point, target)) in points.iter().copied().enumerate() {
        document
            .add_constraint(
                format!("fixed source point {}", index + 1),
                DocumentConstraintDefinition::FixedPoint { point, target },
            )
            .unwrap();
    }
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

fn fillet_fixture() -> (RetainedSketchDocumentSession, SketchOperationRequest) {
    let mut document = SketchDocument::new(4.0).unwrap();
    let corner = document.add_point("corner", [0.0, 0.0]).unwrap();
    let first_start = document.add_point("first start", [-4.0, 0.0]).unwrap();
    let second_end = document.add_point("second end", [0.0, 4.0]).unwrap();
    let first = document
        .add_curve(
            "first parent",
            CurveDefinition::Line {
                start: first_start,
                end: corner,
                branch_direction: [1.0, 0.0],
            },
        )
        .unwrap();
    let second = document
        .add_curve(
            "second parent",
            CurveDefinition::Line {
                start: corner,
                end: second_end,
                branch_direction: [0.0, 1.0],
            },
        )
        .unwrap();
    fix_points(
        &mut document,
        &[
            (first_start, [-4.0, 0.0]),
            (corner, [0.0, 0.0]),
            (second_end, [0.0, 4.0]),
        ],
    );
    let request = SketchOperationRequest::AssociativeFillet {
        label: "fillet".into(),
        request: CurveCurveFilletRequest {
            first: CurveFilletParentRequest {
                curve: CurveSpan::line(first),
                parameter: 0.75,
                winding: 0,
                neighborhood: ContactNeighborhood::Interior,
                side: DocumentCurveNormalSide::Left,
                trim_endpoint: DocumentFilletTrimEndpoint::End,
                periodic_anchor: None,
            },
            second: CurveFilletParentRequest {
                curve: CurveSpan::line(second),
                parameter: 0.25,
                winding: 0,
                neighborhood: ContactNeighborhood::Interior,
                side: DocumentCurveNormalSide::Left,
                trim_endpoint: DocumentFilletTrimEndpoint::Start,
                periodic_anchor: None,
            },
            endpoint_order: DocumentFilletEndpointOrder::FirstThenSecond,
            sweep: DocumentArcSweep::CounterClockwise,
            radius: 1.0,
            radius_mode: DocumentDimensionMode::Driving,
        },
    };
    (session(document), request)
}

#[test]
fn controlled_proposal_apply_is_mutation_free_when_cancelled_or_exhausted() {
    let (mut session, request) = fillet_fixture();
    let proposal = proposed(&session, request);
    let before_input = session.prepared_input();
    let before_document = session.design_document().clone();
    let before_accepted = session
        .accepted_state()
        .map(geosolve_sketch::SketchAcceptedDocumentState::identity);

    let (handle, token) = cancellation_pair();
    handle.cancel();
    let cancelled = proposal
        .apply_controlled(
            &mut session,
            OperationControl::new(token, OperationLimits::unlimited()),
        )
        .unwrap();
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
        .apply_controlled(
            &mut session,
            OperationControl::new(geosolve_sketch::CancellationToken::default(), limits),
        )
        .unwrap();
    assert!(matches!(exhausted, OperationOutcome::WorkExhausted { .. }));
    assert_eq!(session.prepared_input(), before_input);
    assert_eq!(session.design_document(), &before_document);
    assert_eq!(
        session
            .accepted_state()
            .map(geosolve_sketch::SketchAcceptedDocumentState::identity),
        before_accepted
    );
}
