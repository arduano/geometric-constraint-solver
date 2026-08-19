// SPDX-License-Identifier: GPL-3.0-or-later

use std::sync::Arc;

use geosolve_sketch::{
    CurveDefinition, CurveSpan, DocumentArcSweep, DocumentConstraintId, DocumentCurveNormalSide,
    DocumentDimensionDefinition, DocumentFaceOffsetDirection, DocumentFilletEndpointOrder,
    DocumentLineSide, DocumentNativeLineFilletCreationRequest, DocumentNativeLineFilletIds,
    DocumentNativeLineFilletParent, DocumentProfileOffsetJunctionBranch,
    DocumentProfileOffsetJunctionOwner, DocumentProfileOffsetOperand, DocumentSolveRequest,
    FeatureEndpoint, OperationControl, OperationOutcome, RetainedSketchDocumentSession,
    SketchDocument, SolverConfig, TangentOrientation,
};
use geosolve_sketch_ops::{
    SketchOperationProposal, SketchOperationRequest, SketchOperationResult,
    SketchOperationSnapshot, SketchProfileOffsetOperand,
};
use geosolve_sketch_topology::{
    OffsetDirectedSpan, OffsetOperandIndex, OffsetOperandRequest, OffsetTraversal,
    PreparedOffsetOperandQuery,
};

fn operand_index(session: &RetainedSketchDocumentSession) -> Arc<OffsetOperandIndex> {
    let query = PreparedOffsetOperandQuery::capture(session, OffsetOperandRequest::default())
        .expect("current accepted native topology");
    let OperationOutcome::Completed { value, .. } = query
        .execute(OperationControl::unlimited())
        .expect("native topology query")
    else {
        panic!("unbounded native topology query must complete");
    };
    Arc::new(value.operand_index.expect("complete native operand index"))
}

fn proposal(
    session: &RetainedSketchDocumentSession,
    request: SketchOperationRequest,
) -> SketchOperationProposal {
    let OperationOutcome::Completed { value, .. } = SketchOperationSnapshot::capture(session)
        .prepare(request)
        .execute(OperationControl::unlimited())
        .expect("native Fillet Offset operation")
    else {
        panic!("unbounded native Fillet Offset operation must complete");
    };
    let SketchOperationResult::Proposed(proposal) = value else {
        panic!("native line-arc-line path must produce an Offset proposal");
    };
    *proposal
}

fn native_fillet_session() -> (
    RetainedSketchDocumentSession,
    DocumentNativeLineFilletIds,
    [CurveSpan; 3],
) {
    let mut document = SketchDocument::new(4.0).unwrap();
    let first_outer = document.add_point("first outer", [-3.0, 0.0]).unwrap();
    let corner = document.add_point("sharp corner", [0.0, 0.0]).unwrap();
    let second_outer = document.add_point("second outer", [0.0, 3.0]).unwrap();
    let first_line = document
        .add_curve(
            "first line",
            CurveDefinition::Line {
                start: first_outer,
                end: corner,
                branch_direction: [1.0, 0.0],
            },
        )
        .unwrap();
    let second_line = document
        .add_curve(
            "second line",
            CurveDefinition::Line {
                start: corner,
                end: second_outer,
                branch_direction: [0.0, 1.0],
            },
        )
        .unwrap();
    let ids = document
        .create_native_line_fillet_geometry(DocumentNativeLineFilletCreationRequest {
            label: "native corner".into(),
            first: DocumentNativeLineFilletParent {
                curve: CurveSpan::line(first_line),
                endpoint: FeatureEndpoint::End,
                normal_side: DocumentCurveNormalSide::Left,
                tangent_orientation: TangentOrientation::Aligned,
                contact_position: [-1.0, 0.0],
            },
            second: DocumentNativeLineFilletParent {
                curve: CurveSpan::line(second_line),
                endpoint: FeatureEndpoint::Start,
                normal_side: DocumentCurveNormalSide::Left,
                tangent_orientation: TangentOrientation::Aligned,
                contact_position: [0.0, 1.0],
            },
            endpoint_order: DocumentFilletEndpointOrder::FirstThenSecond,
            center: [-1.0, 1.0],
            radius: 1.0,
            start_angle: -std::f64::consts::FRAC_PI_2,
            end_angle: 0.0,
            sweep: DocumentArcSweep::CounterClockwise,
        })
        .unwrap();
    let spans = [
        CurveSpan::line(first_line),
        CurveSpan::line(ids.arc),
        CurveSpan::line(second_line),
    ];
    let session = RetainedSketchDocumentSession::new(
        document,
        DocumentSolveRequest::default().without_previous_state_preferences(),
        SolverConfig::default(),
    )
    .expect("native line-arc-line session");
    (session, ids, spans)
}

fn assert_same_curve_family(document: &SketchDocument, source: CurveSpan, target: CurveSpan) {
    let source = &document.curve(source.curve).unwrap().definition;
    let target = &document.curve(target.curve).unwrap().definition;
    assert!(matches!(
        (source, target),
        (CurveDefinition::Line { .. }, CurveDefinition::Line { .. })
            | (
                CurveDefinition::CircularArc { .. },
                CurveDefinition::CircularArc { .. }
            )
    ));
}

fn assert_current_hard_valid(session: &RetainedSketchDocumentSession) {
    let accepted = session
        .accepted_state_for_current_input()
        .expect("current accepted native Offset");
    let report = accepted.solve_result().unstable_core_report();
    assert!(accepted.solve_result().accepted(), "{report:#?}");
    assert!(report.hard_residuals_validated, "{report:#?}");
    assert!(report.hard_residual_max <= 1.0e-9, "{report:#?}");
    assert!(
        accepted
            .document()
            .points()
            .iter()
            .all(|point| { point.position[0].is_finite() && point.position[1].is_finite() })
    );
    assert!(
        accepted
            .document()
            .scalars()
            .iter()
            .all(|scalar| scalar.value.is_finite())
    );
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "forward and reverse native line-arc-line traversals jointly prove unchanged Offset consumption and branch ownership"
)]
fn native_fillet_offsets_as_an_ordinary_mixed_tangent_chain_in_both_traversals() {
    for reverse in [false, true] {
        let (mut session, native_ids, forward_spans) = native_fillet_session();
        let spans = if reverse {
            forward_spans
                .map(|span| OffsetDirectedSpan {
                    span,
                    traversal: OffsetTraversal::Reverse,
                })
                .into_iter()
                .rev()
                .collect::<Vec<_>>()
        } else {
            forward_spans
                .map(|span| OffsetDirectedSpan {
                    span,
                    traversal: OffsetTraversal::Forward,
                })
                .to_vec()
        };
        let expected_owners: [DocumentConstraintId; 2] = if reverse {
            [native_ids.tangencies[1], native_ids.tangencies[0]]
        } else {
            native_ids.tangencies
        };
        let request = SketchOperationRequest::ProfileOffset {
            label: if reverse {
                "reverse native Fillet Offset".into()
            } else {
                "forward native Fillet Offset".into()
            },
            distance: 0.2,
            operand: SketchProfileOffsetOperand::OpenChain {
                spans: spans.clone(),
                side: DocumentLineSide::Left,
            },
            operand_index: operand_index(&session),
        };
        let outcome = proposal(&session, request)
            .apply(&mut session)
            .expect("native Fillet Offset publication");
        assert!(outcome.published_accepted_identity().is_some());
        assert_current_hard_valid(&session);

        let dimension = session.design_document().dimensions().last().unwrap();
        let DocumentDimensionDefinition::ProfileOffset { operand, .. } = &dimension.definition
        else {
            panic!("native Fillet Offset dimension expected");
        };
        let DocumentProfileOffsetOperand::OpenChain { chain, .. } = operand else {
            panic!("native Fillet Offset must remain an open chain");
        };
        assert_eq!(chain.edges.len(), 3);
        assert_eq!(chain.junctions.len(), 2);
        assert_eq!(
            chain
                .edges
                .iter()
                .map(|edge| edge.source.curve)
                .collect::<Vec<_>>(),
            spans.iter().map(|span| span.span).collect::<Vec<_>>()
        );
        for edge in &chain.edges {
            assert_same_curve_family(
                session.design_document(),
                edge.source.curve,
                edge.target.curve,
            );
        }
        for (junction, expected_owner) in chain.junctions.iter().zip(expected_owners) {
            assert_eq!(
                junction.branch,
                DocumentProfileOffsetJunctionBranch::Tangent
            );
            assert_eq!(
                junction.source_owner,
                DocumentProfileOffsetJunctionOwner::Constraint(expected_owner)
            );
            assert!(matches!(
                junction.target_owner,
                DocumentProfileOffsetJunctionOwner::Constraint(_)
            ));
            assert_ne!(junction.source_owner, junction.target_owner);
        }
    }
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one closed-face regression keeps native construction, offset creation, topology, and residual invariants together"
)]
fn native_fillet_offsets_as_an_ordinary_closed_face_corner() {
    let (session, native_ids, spans) = native_fillet_session();
    let mut document = session.design_document().clone();
    let CurveDefinition::Line {
        start: first_outer, ..
    } = document.curve(spans[0].curve).unwrap().definition.clone()
    else {
        panic!("first native parent must remain a Line");
    };
    let CurveDefinition::Line {
        end: second_outer, ..
    } = document.curve(spans[2].curve).unwrap().definition.clone()
    else {
        panic!("second native parent must remain a Line");
    };
    let opposite = document.add_point("opposite corner", [-3.0, 3.0]).unwrap();
    document
        .add_curve(
            "top face edge",
            CurveDefinition::Line {
                start: second_outer,
                end: opposite,
                branch_direction: [-1.0, 0.0],
            },
        )
        .unwrap();
    document
        .add_curve(
            "left face edge",
            CurveDefinition::Line {
                start: opposite,
                end: first_outer,
                branch_direction: [0.0, -1.0],
            },
        )
        .unwrap();
    let mut session = RetainedSketchDocumentSession::new(
        document,
        DocumentSolveRequest::default().without_previous_state_preferences(),
        SolverConfig::default(),
    )
    .expect("closed native-Fillet face session");
    let index = operand_index(&session);
    let face = index
        .faces()
        .iter()
        .find(|face| {
            face.eligibility.is_eligible()
                && face
                    .key
                    .outer
                    .spans
                    .iter()
                    .any(|edge| edge.span == CurveSpan::line(native_ids.arc))
        })
        .expect("one eligible face containing the native Fillet arc")
        .key
        .clone();
    let outcome = proposal(
        &session,
        SketchOperationRequest::ProfileOffset {
            label: "native Fillet face Offset".into(),
            distance: 0.2,
            operand: SketchProfileOffsetOperand::Face {
                key: face,
                direction: DocumentFaceOffsetDirection::Outward,
            },
            operand_index: index,
        },
    )
    .apply(&mut session)
    .expect("native Fillet face Offset publication");
    assert!(outcome.published_accepted_identity().is_some());
    assert_current_hard_valid(&session);

    let dimension = session.design_document().dimensions().last().unwrap();
    let DocumentDimensionDefinition::ProfileOffset { operand, .. } = &dimension.definition else {
        panic!("native Fillet face Offset dimension expected");
    };
    let DocumentProfileOffsetOperand::Face { outer, holes, .. } = operand else {
        panic!("native Fillet loop must remain a face operand");
    };
    assert!(holes.is_empty());
    assert_eq!(outer.edges.len(), 5);
    assert_eq!(outer.junctions.len(), 5);
    assert!(
        outer
            .edges
            .iter()
            .any(|edge| edge.source.curve == CurveSpan::line(native_ids.arc))
    );
    for edge in &outer.edges {
        assert_same_curve_family(
            session.design_document(),
            edge.source.curve,
            edge.target.curve,
        );
    }
    for tangency in native_ids.tangencies {
        assert!(outer.junctions.iter().any(|junction| {
            junction.branch == DocumentProfileOffsetJunctionBranch::Tangent
                && junction.source_owner == DocumentProfileOffsetJunctionOwner::Constraint(tangency)
        }));
    }
}
