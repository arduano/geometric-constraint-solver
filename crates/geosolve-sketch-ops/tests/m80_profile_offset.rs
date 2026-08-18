// SPDX-License-Identifier: GPL-3.0-or-later

use std::sync::Arc;

use geosolve_sketch::{
    CancellationToken, ContactNeighborhood, CurveDefinition, CurveSpan, DocumentArcSweep,
    DocumentConstraintDefinition, DocumentDimensionDefinition, DocumentEdit,
    DocumentFaceOffsetDirection, DocumentLineSide, DocumentObjectId, DocumentOffsetTraversal,
    DocumentProfileOffsetJunctionBranch, DocumentProfileOffsetJunctionOwner,
    DocumentProfileOffsetOperand, DocumentProfileOffsetTerminalPolicy, DocumentSolveRequest,
    GeometryRole, OperationControl, OperationLimits, OperationOutcome,
    PreparedSketchOperation as PreparedDocumentOperation, RetainedSketchDocumentSession,
    ScalarDomain, ScalarUnit, SketchDocument, SolverConfig, cancellation_pair,
};
use geosolve_sketch_ops::{
    SketchOperationApplyError, SketchOperationIncompleteReason, SketchOperationKind,
    SketchOperationProposal, SketchOperationRequest, SketchOperationResult,
    SketchOperationSnapshot, SketchOperationUnsupportedReason, SketchProfileOffsetOperand,
};
use geosolve_sketch_topology::{
    OffsetDirectedSpan, OffsetOperandIndex, OffsetOperandIneligibility, OffsetOperandRequest,
    OffsetTraversal, PreparedOffsetOperandQuery,
};

fn session(document: SketchDocument) -> RetainedSketchDocumentSession {
    RetainedSketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .expect("fixture must solve")
}

fn line(
    document: &mut SketchDocument,
    label: &str,
    start: geosolve_sketch::DesignPointId,
    end: geosolve_sketch::DesignPointId,
) -> CurveSpan {
    let first = document.point(start).expect("start point").position;
    let second = document.point(end).expect("end point").position;
    let delta = [second[0] - first[0], second[1] - first[1]];
    let length = delta[0].hypot(delta[1]);
    CurveSpan::line(
        document
            .add_curve(
                label,
                CurveDefinition::Line {
                    start,
                    end,
                    branch_direction: [delta[0] / length, delta[1] / length],
                },
            )
            .expect("line"),
    )
}

fn operand_index(session: &RetainedSketchDocumentSession) -> Arc<OffsetOperandIndex> {
    let query = PreparedOffsetOperandQuery::capture(session, OffsetOperandRequest::default())
        .expect("current accepted topology");
    let OperationOutcome::Completed { value, .. } = query
        .execute(OperationControl::unlimited())
        .expect("topology query")
    else {
        panic!("unbounded topology query must complete");
    };
    Arc::new(value.operand_index.expect("complete operand index"))
}

fn execute(
    session: &RetainedSketchDocumentSession,
    request: SketchOperationRequest,
) -> SketchOperationResult {
    let OperationOutcome::Completed { value, .. } = SketchOperationSnapshot::capture(session)
        .prepare(request)
        .execute(OperationControl::unlimited())
        .expect("operation execution")
    else {
        panic!("unbounded operation must complete");
    };
    value
}

fn proposal(
    session: &RetainedSketchDocumentSession,
    request: SketchOperationRequest,
) -> SketchOperationProposal {
    let SketchOperationResult::Proposed(proposal) = execute(session, request) else {
        panic!("proposal expected");
    };
    *proposal
}

fn assert_current_hard_valid(session: &RetainedSketchDocumentSession) {
    let accepted = session
        .accepted_state_for_current_input()
        .expect("current independently accepted state");
    let report = accepted.solve_result().unstable_core_report();
    assert!(accepted.solve_result().accepted());
    assert!(report.hard_residuals_validated);
    assert!(report.hard_residual_max <= 1.0e-9, "{report:#?}");
    assert!(
        accepted
            .solve_result()
            .geometry
            .points
            .iter()
            .all(|point| point.position.x.is_finite() && point.position.y.is_finite())
    );
}

fn polygon(
    document: &mut SketchDocument,
    label: &str,
    positions: &[[f64; 2]],
) -> (Vec<geosolve_sketch::DesignPointId>, Vec<CurveSpan>) {
    let points = positions
        .iter()
        .enumerate()
        .map(|(index, position)| {
            document
                .add_point(format!("{label}.point_{}", index + 1), *position)
                .expect("polygon point")
        })
        .collect::<Vec<_>>();
    let spans = (0..points.len())
        .map(|index| {
            line(
                document,
                &format!("{label}.edge_{}", index + 1),
                points[index],
                points[(index + 1) % points.len()],
            )
        })
        .collect();
    (points, spans)
}

fn profile_offset_operand(document: &SketchDocument) -> &DocumentProfileOffsetOperand {
    let dimension = document
        .dimensions()
        .last()
        .expect("profile offset dimension");
    let DocumentDimensionDefinition::ProfileOffset { operand, .. } = &dimension.definition else {
        panic!("ProfileOffset expected");
    };
    operand
}

fn directed_line_points(
    document: &SketchDocument,
    curve: geosolve_sketch::DocumentDirectedProfileOffsetCurve,
) -> [[f64; 2]; 2] {
    let CurveDefinition::Line { start, end, .. } = &document
        .curve(curve.curve.curve)
        .expect("offset line")
        .definition
    else {
        panic!("line offset pair expected");
    };
    let mut points = [
        document.point(*start).expect("line start").position,
        document.point(*end).expect("line end").position,
    ];
    if curve.traversal == DocumentOffsetTraversal::Reverse {
        points.reverse();
    }
    points
}

fn assert_signed_line_offset(
    document: &SketchDocument,
    edge: &geosolve_sketch::DocumentProfileOffsetEdgePair,
    expected: f64,
) {
    let source = directed_line_points(document, edge.source);
    let target = directed_line_points(document, edge.target);
    let source_delta = [source[1][0] - source[0][0], source[1][1] - source[0][1]];
    let target_delta = [target[1][0] - target[0][0], target[1][1] - target[0][1]];
    let source_length = source_delta[0].hypot(source_delta[1]);
    let target_length = target_delta[0].hypot(target_delta[1]);
    let parallel_cross = (source_delta[0] * target_delta[1] - source_delta[1] * target_delta[0])
        / (source_length * target_length);
    let aligned_dot = (source_delta[0] * target_delta[0] + source_delta[1] * target_delta[1])
        / (source_length * target_length);
    let start_delta = [target[0][0] - source[0][0], target[0][1] - source[0][1]];
    let signed_distance =
        (source_delta[0] * start_delta[1] - source_delta[1] * start_delta[0]) / source_length;
    assert!(parallel_cross.abs() <= 1.0e-9, "{parallel_cross}");
    assert!(aligned_dot >= 1.0 - 1.0e-9, "{aligned_dot}");
    assert!(
        (signed_distance - expected).abs() <= 1.0e-8,
        "expected signed offset {expected}, got {signed_distance}"
    );
}

fn circle_radius(document: &SketchDocument, span: CurveSpan) -> f64 {
    let CurveDefinition::Circle { radius, .. } = &document
        .curve(span.curve)
        .expect("offset circle")
        .definition
    else {
        panic!("circle expected");
    };
    document.scalar(*radius).expect("circle radius").value
}

fn line_crosses_circle(
    document: &SketchDocument,
    line: CurveSpan,
    center: [f64; 2],
    radius: f64,
) -> bool {
    let CurveDefinition::Line { start, end, .. } = &document
        .curve(line.curve)
        .expect("candidate target line")
        .definition
    else {
        return false;
    };
    let start = document.point(*start).unwrap().position;
    let end = document.point(*end).unwrap().position;
    let delta = [end[0] - start[0], end[1] - start[1]];
    let relative = [start[0] - center[0], start[1] - center[1]];
    let a = delta[0].mul_add(delta[0], delta[1] * delta[1]);
    let b = 2.0 * relative[0].mul_add(delta[0], relative[1] * delta[1]);
    let c = relative[0].mul_add(relative[0], relative[1] * relative[1]) - radius * radius;
    let discriminant = b.mul_add(b, -4.0 * a * c);
    if discriminant <= 1.0e-12 {
        return false;
    }
    let root = discriminant.sqrt();
    [(-b - root) / (2.0 * a), (-b + root) / (2.0 * a)]
        .into_iter()
        .any(|parameter| parameter > 1.0e-9 && parameter < 1.0 - 1.0e-9)
}

#[test]
fn authenticated_face_becomes_one_atomic_preview_and_exact_cas_commit() {
    let mut document = SketchDocument::new(10.0).expect("document");
    document
        .add_rectangle("source", [0.0, 0.0], 4.0, 3.0)
        .expect("rectangle");
    let mut session = session(document);
    let index = operand_index(&session);
    let face = index.faces().first().expect("rectangle face").key.clone();
    let source_curves = session.design_document().curves().len();
    let source_dimensions = session.design_document().dimensions().len();

    let proposal = proposal(
        &session,
        SketchOperationRequest::ProfileOffset {
            label: "outward offset".into(),
            distance: 0.5,
            operand: SketchProfileOffsetOperand::Face {
                key: face.clone(),
                direction: DocumentFaceOffsetDirection::Outward,
            },
            operand_index: index,
        },
    );
    assert_eq!(proposal.input(), session.prepared_input());
    assert_eq!(
        proposal.expected_application().kind,
        SketchOperationKind::ProfileOffset
    );
    assert_eq!(
        proposal
            .expected_application()
            .identity_changes
            .iter()
            .filter(|change| matches!(
                change,
                geosolve_sketch_ops::SketchOperationIdentityChange::Retained(_)
            ))
            .count(),
        face.outer.spans.len()
    );

    let edit = proposal
        .profile_offset_document_edit()
        .expect("profile offset exposes its authenticated edit");
    assert!(matches!(
        edit,
        DocumentEdit::CreatePreparedProfileOffsetGeometry { .. }
    ));
    let OperationOutcome::Completed { value: patch, .. } = session
        .prepared_snapshot()
        .prepare(PreparedDocumentOperation::Apply(edit))
        .execute(OperationControl::unlimited())
        .expect("prepared profile-offset patch")
    else {
        panic!("unbounded prepared edit must complete");
    };
    assert_eq!(patch.base_input(), proposal.input());
    let preview = patch
        .preview()
        .accepted_document()
        .expect("independently accepted preview")
        .clone();
    assert_eq!(preview.curves().len(), source_curves + 4);
    assert_eq!(preview.dimensions().len(), source_dimensions + 1);
    let dimension = preview.dimensions().last().expect("grouped dimension");
    let DocumentDimensionDefinition::ProfileOffset { target, operand } = &dimension.definition
    else {
        panic!("one grouped ProfileOffset dimension expected");
    };
    assert!((preview.scalar(*target).expect("target scalar").value - 0.5).abs() <= f64::EPSILON);
    let geosolve_sketch::DocumentProfileOffsetOperand::Face {
        direction,
        outer,
        holes,
    } = operand
    else {
        panic!("face operand expected");
    };
    assert_eq!(*direction, DocumentFaceOffsetDirection::Outward);
    assert!(holes.is_empty());
    assert_eq!(outer.edges.len(), 4);
    assert_eq!(outer.junctions.len(), 4);
    assert!(outer.junctions.iter().all(|junction| {
        matches!(
            junction.branch,
            DocumentProfileOffsetJunctionBranch::Miter { .. }
        ) && junction.source_owner != junction.target_owner
    }));

    session
        .commit_prepared_patch(patch)
        .expect("exact-input CAS commit");
    assert_eq!(
        session
            .accepted_state_for_current_input()
            .expect("committed acceptance")
            .document(),
        &preview
    );
}

#[test]
fn closed_polyline_spans_construct_independent_native_line_targets() {
    let mut document = SketchDocument::new(10.0).expect("document");
    let points = [
        document.add_point("p0", [0.0, 0.0]).unwrap(),
        document.add_point("p1", [4.0, 0.0]).unwrap(),
        document.add_point("p2", [4.0, 3.0]).unwrap(),
        document.add_point("p3", [0.0, 3.0]).unwrap(),
    ];
    let source = document
        .add_curve(
            "closed polyline",
            CurveDefinition::Polyline {
                points: points.to_vec(),
                closed: true,
                branch_directions: vec![[1.0, 0.0], [0.0, 1.0], [-1.0, 0.0], [0.0, -1.0]],
            },
        )
        .unwrap();
    let session = session(document);
    let index = operand_index(&session);
    let face = index
        .faces()
        .iter()
        .find(|face| {
            face.eligibility.is_eligible()
                && face.key.outer.spans.len() == 4
                && face
                    .key
                    .outer
                    .spans
                    .iter()
                    .all(|edge| edge.span.curve == source)
        })
        .expect("closed polyline face")
        .key
        .clone();
    let edit = proposal(
        &session,
        SketchOperationRequest::ProfileOffset {
            label: "polyline offset".into(),
            distance: 0.5,
            operand: SketchProfileOffsetOperand::Face {
                key: face,
                direction: DocumentFaceOffsetDirection::Outward,
            },
            operand_index: index,
        },
    )
    .profile_offset_document_edit()
    .expect("polyline prepared edit");
    let OperationOutcome::Completed { value: patch, .. } = session
        .prepared_snapshot()
        .prepare(PreparedDocumentOperation::Apply(edit))
        .execute(OperationControl::unlimited())
        .expect("polyline patch")
    else {
        panic!("polyline patch must complete");
    };
    let patch_preview = patch.preview();
    let preview = patch_preview
        .accepted_document()
        .expect("accepted polyline offset");
    let DocumentProfileOffsetOperand::Face { outer, .. } = profile_offset_operand(preview) else {
        panic!("face operand expected");
    };
    assert_eq!(outer.edges.len(), 4);
    assert!(outer.edges.iter().all(|edge| {
        edge.source.curve.curve == source
            && matches!(
                preview
                    .curve(edge.target.curve.curve)
                    .expect("standalone target")
                    .definition,
                CurveDefinition::Line { .. }
            )
            && edge.target.curve.segment == 0
    }));
    assert_eq!(
        outer
            .edges
            .iter()
            .map(|edge| edge.target.curve.curve)
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        4
    );
}

#[test]
fn one_line_chain_constructs_both_sides_for_forward_and_reverse_traversal() {
    for (traversal, side, expected_signed_distance) in [
        (OffsetTraversal::Forward, DocumentLineSide::Left, 0.5),
        (OffsetTraversal::Forward, DocumentLineSide::Right, -0.5),
        (OffsetTraversal::Reverse, DocumentLineSide::Left, 0.5),
        (OffsetTraversal::Reverse, DocumentLineSide::Right, -0.5),
    ] {
        let mut document = SketchDocument::new(10.0).expect("document");
        let start = document.add_point("start", [0.0, 0.0]).unwrap();
        let end = document.add_point("end", [4.0, 0.0]).unwrap();
        let source = line(&mut document, "source", start, end);
        let session = session(document);
        let edit = proposal(
            &session,
            SketchOperationRequest::ProfileOffset {
                label: format!("{traversal:?} {side:?} line offset"),
                distance: 0.5,
                operand: SketchProfileOffsetOperand::OpenChain {
                    spans: vec![OffsetDirectedSpan {
                        span: source,
                        traversal,
                    }],
                    side,
                },
                operand_index: operand_index(&session),
            },
        )
        .profile_offset_document_edit()
        .expect("one-line edit");
        let OperationOutcome::Completed { value: patch, .. } = session
            .prepared_snapshot()
            .prepare(PreparedDocumentOperation::Apply(edit))
            .execute(OperationControl::unlimited())
            .expect("one-line patch")
        else {
            panic!("one-line patch must complete");
        };
        let patch_preview = patch.preview();
        let preview = patch_preview
            .accepted_document()
            .expect("accepted one-line offset");
        let DocumentProfileOffsetOperand::OpenChain {
            side: stored_side,
            chain,
        } = profile_offset_operand(preview)
        else {
            panic!("open-chain operand expected");
        };
        assert_eq!(*stored_side, side);
        assert_eq!(chain.edges.len(), 1);
        assert!(chain.junctions.is_empty());
        assert_eq!(
            chain.start_terminal,
            DocumentProfileOffsetTerminalPolicy::NormalTranslation
        );
        assert_eq!(
            chain.end_terminal,
            DocumentProfileOffsetTerminalPolicy::NormalTranslation
        );
        assert_eq!(
            chain.edges[0].source.traversal,
            match traversal {
                OffsetTraversal::Forward => DocumentOffsetTraversal::Forward,
                OffsetTraversal::Reverse => DocumentOffsetTraversal::Reverse,
            }
        );
        assert_signed_line_offset(preview, &chain.edges[0], expected_signed_distance);
        assert_current_hard_valid(
            patch_preview
                .accepted_session()
                .expect("accepted preview session"),
        );
    }
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one exhaustive traversal/side matrix keeps the exact arc construction contract together"
)]
fn one_circular_arc_chain_constructs_both_sides_for_forward_and_reverse_traversal() {
    for (traversal, side, expected_radius) in [
        (OffsetTraversal::Forward, DocumentLineSide::Left, 2.5),
        (OffsetTraversal::Forward, DocumentLineSide::Right, 3.5),
        (OffsetTraversal::Reverse, DocumentLineSide::Left, 3.5),
        (OffsetTraversal::Reverse, DocumentLineSide::Right, 2.5),
    ] {
        let mut document = SketchDocument::new(10.0).expect("document");
        let center = document.add_point("center", [0.0, 0.0]).unwrap();
        let radius = document
            .add_scalar("radius", 3.0, ScalarUnit::Length, ScalarDomain::Positive)
            .unwrap();
        let start_angle = document
            .add_scalar("start", 0.0, ScalarUnit::Angle, ScalarDomain::Finite)
            .unwrap();
        let end_angle = document
            .add_scalar(
                "end",
                std::f64::consts::FRAC_PI_2,
                ScalarUnit::Angle,
                ScalarDomain::Finite,
            )
            .unwrap();
        let source = CurveSpan::line(
            document
                .add_curve(
                    "source arc",
                    CurveDefinition::CircularArc {
                        center,
                        radius,
                        start_angle,
                        end_angle,
                        sweep: DocumentArcSweep::CounterClockwise,
                    },
                )
                .unwrap(),
        );
        let session = session(document);
        let edit = proposal(
            &session,
            SketchOperationRequest::ProfileOffset {
                label: format!("{traversal:?} {side:?} arc offset"),
                distance: 0.5,
                operand: SketchProfileOffsetOperand::OpenChain {
                    spans: vec![OffsetDirectedSpan {
                        span: source,
                        traversal,
                    }],
                    side,
                },
                operand_index: operand_index(&session),
            },
        )
        .profile_offset_document_edit()
        .expect("one-arc edit");
        let OperationOutcome::Completed { value: patch, .. } = session
            .prepared_snapshot()
            .prepare(PreparedDocumentOperation::Apply(edit))
            .execute(OperationControl::unlimited())
            .expect("one-arc patch")
        else {
            panic!("one-arc patch must complete");
        };
        let patch_preview = patch.preview();
        let preview = patch_preview
            .accepted_document()
            .expect("accepted one-arc offset");
        let DocumentProfileOffsetOperand::OpenChain {
            side: stored_side,
            chain,
        } = profile_offset_operand(preview)
        else {
            panic!("open-chain operand expected");
        };
        assert_eq!(*stored_side, side);
        let [edge] = chain.edges.as_slice() else {
            panic!("one target arc expected");
        };
        assert_eq!(
            edge.source.traversal,
            match traversal {
                OffsetTraversal::Forward => DocumentOffsetTraversal::Forward,
                OffsetTraversal::Reverse => DocumentOffsetTraversal::Reverse,
            }
        );
        let CurveDefinition::CircularArc { radius, sweep, .. } = &preview
            .curve(edge.target.curve.curve)
            .expect("native target arc")
            .definition
        else {
            panic!("target must remain a circular arc");
        };
        assert_eq!(*sweep, DocumentArcSweep::CounterClockwise);
        assert!(
            (preview.scalar(*radius).unwrap().value - expected_radius).abs() <= 1.0e-9,
            "{traversal:?} {side:?}"
        );
        assert_current_hard_valid(
            patch_preview
                .accepted_session()
                .expect("accepted preview session"),
        );
    }
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one retained-boundary regression keeps accepted seeding, independent validation, exact CAS, and stale-plan atomicity together"
)]
fn accepted_geometry_seeds_offset_without_rewriting_retained_source_intent() {
    let mut document = SketchDocument::new(10.0).expect("document");
    let source_points = [
        document.add_point("source start", [0.0, 0.0]).unwrap(),
        document.add_point("source end", [2.0, 0.0]).unwrap(),
    ];
    let source = line(
        &mut document,
        "constrained source",
        source_points[0],
        source_points[1],
    );
    for (label, point, target) in [
        ("move source start", source_points[0], [3.0, 4.0]),
        ("move source end", source_points[1], [5.0, 4.0]),
    ] {
        document
            .add_constraint(
                label,
                DocumentConstraintDefinition::FixedPoint { point, target },
            )
            .unwrap();
    }
    let retained_seed = document.clone();
    let mut session = session(document);
    let accepted_before = session
        .accepted_state_for_current_input()
        .expect("constrained source accepts");
    assert_ne!(accepted_before.document(), session.design_document());
    assert_eq!(
        source_points.map(|point| accepted_before.document().point(point).unwrap().position),
        [[3.0, 4.0], [5.0, 4.0]]
    );

    let proposal = proposal(
        &session,
        SketchOperationRequest::ProfileOffset {
            label: "accepted-seeded offset".into(),
            distance: 1.0,
            operand: SketchProfileOffsetOperand::OpenChain {
                spans: vec![OffsetDirectedSpan {
                    span: source,
                    traversal: OffsetTraversal::Forward,
                }],
                side: DocumentLineSide::Left,
            },
            operand_index: operand_index(&session),
        },
    );
    let edit = proposal
        .profile_offset_document_edit()
        .expect("accepted-seeded prepared edit");
    let OperationOutcome::Completed { value: patch, .. } = session
        .prepared_snapshot()
        .prepare(PreparedDocumentOperation::Apply(edit.clone()))
        .execute(OperationControl::unlimited())
        .expect("prepared accepted-seeded patch")
    else {
        panic!("unbounded patch must complete");
    };
    let patch_preview = patch.preview();
    let preview = patch_preview
        .accepted_document()
        .expect("accepted-seeded preview");
    let dimension = preview
        .dimensions()
        .last()
        .expect("profile offset dimension");
    let DocumentDimensionDefinition::ProfileOffset { operand, .. } = &dimension.definition else {
        panic!("ProfileOffset expected");
    };
    let geosolve_sketch::DocumentProfileOffsetOperand::OpenChain { chain, .. } = operand else {
        panic!("open chain expected");
    };
    let [edge] = chain.edges.as_slice() else {
        panic!("one target edge expected");
    };
    let CurveDefinition::Line { start, end, .. } = &preview
        .curve(edge.target.curve.curve)
        .expect("target line")
        .definition
    else {
        panic!("target remains a native line");
    };
    assert_eq!(
        [
            preview.point(*start).unwrap().position,
            preview.point(*end).unwrap().position,
        ],
        [[3.0, 5.0], [5.0, 5.0]]
    );
    let accepted = patch_preview
        .accepted_state()
        .expect("independently accepted preview");
    assert!(accepted.solve_result().accepted());
    assert!(
        accepted
            .solve_result()
            .unstable_core_report()
            .hard_residuals_validated
    );
    assert!(
        accepted
            .solve_result()
            .unstable_core_report()
            .hard_residual_max
            <= 1.0e-9
    );

    session
        .commit_prepared_patch(patch)
        .expect("exact accepted-seeded commit");
    for point in source_points {
        assert_eq!(
            session
                .design_document()
                .point(point)
                .unwrap()
                .position
                .map(f64::to_bits),
            retained_seed
                .point(point)
                .unwrap()
                .position
                .map(f64::to_bits),
            "accepted source coordinates must not overwrite retained design intent"
        );
    }

    session
        .apply(
            session.design_identity(),
            DocumentEdit::SetGeometryRole {
                curve: source.curve,
                role: GeometryRole::Construction,
            },
        )
        .expect("make the prepared source structurally stale");
    let before_stale_apply = session.design_document().clone();
    assert!(
        session.apply(session.design_identity(), edit).is_err(),
        "a prepared accepted seed must fail closed after source-role change"
    );
    assert_eq!(session.design_document(), &before_stale_apply);
}

#[test]
fn authenticated_annular_face_offsets_outer_and_hole_together_and_rejects_hole_loss() {
    let mut document = SketchDocument::new(10.0).expect("document");
    let center = document.add_point("center", [0.0, 0.0]).unwrap();
    for (label, value) in [("outer", 4.0), ("hole", 2.0)] {
        let radius = document
            .add_scalar(label, value, ScalarUnit::Length, ScalarDomain::Positive)
            .unwrap();
        document
            .add_curve(label, CurveDefinition::Circle { center, radius })
            .unwrap();
    }
    let session = session(document);
    let index = operand_index(&session);
    let face = index
        .faces()
        .iter()
        .find(|face| face.key.holes.len() == 1)
        .expect("annular face")
        .key
        .clone();
    let proposal = proposal(
        &session,
        SketchOperationRequest::ProfileOffset {
            label: "annular outward offset".into(),
            distance: 0.5,
            operand: SketchProfileOffsetOperand::Face {
                key: face.clone(),
                direction: DocumentFaceOffsetDirection::Outward,
            },
            operand_index: Arc::clone(&index),
        },
    );
    let edit = proposal
        .profile_offset_document_edit()
        .expect("authenticated annular edit");
    let OperationOutcome::Completed { value: patch, .. } = session
        .prepared_snapshot()
        .prepare(PreparedDocumentOperation::Apply(edit))
        .execute(OperationControl::unlimited())
        .expect("annular prepared patch")
    else {
        panic!("unbounded annular patch must complete");
    };
    let preview = patch
        .preview()
        .accepted_document()
        .expect("independently accepted annular preview")
        .clone();
    let DocumentDimensionDefinition::ProfileOffset { operand, .. } = &preview
        .dimensions()
        .last()
        .expect("grouped dimension")
        .definition
    else {
        panic!("grouped annular dimension expected");
    };
    let geosolve_sketch::DocumentProfileOffsetOperand::Face { outer, holes, .. } = operand else {
        panic!("face operand expected");
    };
    assert_eq!(outer.edges.len(), 1);
    assert_eq!(holes.len(), 1);
    assert_eq!(holes[0].edges.len(), 1);
    let radius_of = |span: CurveSpan| {
        let CurveDefinition::Circle { radius, .. } =
            &preview.curve(span.curve).expect("target circle").definition
        else {
            panic!("annular target must stay circular");
        };
        preview.scalar(*radius).expect("target radius").value
    };
    assert!((radius_of(outer.edges[0].target.curve) - 4.5).abs() <= 1.0e-9);
    assert!((radius_of(holes[0].edges[0].target.curve) - 1.5).abs() <= 1.0e-9);

    let before = session.design_document().clone();
    let stopped = SketchOperationSnapshot::capture(&session)
        .prepare(SketchOperationRequest::ProfileOffset {
            label: "annular hole-loss barrier".into(),
            distance: 2.0,
            operand: SketchProfileOffsetOperand::Face {
                key: face,
                direction: DocumentFaceOffsetDirection::Outward,
            },
            operand_index: index,
        })
        .execute(OperationControl::unlimited());
    assert!(
        stopped.is_err(),
        "a collapsed hole must not produce a proposal"
    );
    assert_eq!(session.design_document(), &before);
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one end-to-end fixture keeps the mixed native face, exact junction provenance, and target-family assertions together"
)]
fn mixed_line_arc_face_keeps_native_families_and_explicit_miter_branches() {
    let mut document = SketchDocument::new(10.0).expect("document");
    let center = document.add_point("center", [0.0, 0.0]).unwrap();
    let radius = document
        .add_scalar("radius", 2.0, ScalarUnit::Length, ScalarDomain::Positive)
        .unwrap();
    let start = document
        .add_scalar(
            "start",
            -std::f64::consts::FRAC_PI_2,
            ScalarUnit::Angle,
            ScalarDomain::Finite,
        )
        .unwrap();
    let end = document
        .add_scalar(
            "end",
            std::f64::consts::FRAC_PI_2,
            ScalarUnit::Angle,
            ScalarDomain::Finite,
        )
        .unwrap();
    let arc = CurveSpan::line(
        document
            .add_curve(
                "semicircle",
                CurveDefinition::CircularArc {
                    center,
                    radius,
                    start_angle: start,
                    end_angle: end,
                    sweep: DocumentArcSweep::CounterClockwise,
                },
            )
            .unwrap(),
    );
    let top = document.add_point("top", [0.0, 2.0]).unwrap();
    let bottom = document.add_point("bottom", [0.0, -2.0]).unwrap();
    let diameter = line(&mut document, "diameter", top, bottom);
    for (label, point, parameter, neighborhood) in [
        ("top join", top, 1.0, ContactNeighborhood::End),
        ("bottom join", bottom, 0.0, ContactNeighborhood::Start),
    ] {
        let contact = document
            .add_curve_contact(label, arc, parameter, 0, neighborhood, None)
            .unwrap();
        document
            .add_constraint(
                label,
                DocumentConstraintDefinition::PointOnCurve { point, contact },
            )
            .unwrap();
    }
    let session = session(document);
    let index = operand_index(&session);
    let face = index
        .faces()
        .iter()
        .find(|face| {
            face.eligibility.is_eligible()
                && face.key.outer.spans.len() == 2
                && face
                    .key
                    .outer
                    .spans
                    .iter()
                    .any(|edge| edge.span == diameter)
                && face.key.outer.spans.iter().any(|edge| edge.span == arc)
        })
        .expect("mixed native face")
        .key
        .clone();
    let proposal = proposal(
        &session,
        SketchOperationRequest::ProfileOffset {
            label: "mixed face offset".into(),
            distance: 0.25,
            operand: SketchProfileOffsetOperand::Face {
                key: face,
                direction: DocumentFaceOffsetDirection::Outward,
            },
            operand_index: index,
        },
    );
    let edit = proposal.profile_offset_document_edit().expect("mixed edit");
    let OperationOutcome::Completed { value: patch, .. } = session
        .prepared_snapshot()
        .prepare(PreparedDocumentOperation::Apply(edit))
        .execute(OperationControl::unlimited())
        .expect("mixed prepared patch")
    else {
        panic!("mixed patch must complete");
    };
    let preview = patch
        .preview()
        .accepted_document()
        .expect("independently accepted mixed face preview")
        .clone();
    let DocumentDimensionDefinition::ProfileOffset { operand, .. } = &preview
        .dimensions()
        .last()
        .expect("grouped dimension")
        .definition
    else {
        panic!("mixed grouped dimension expected");
    };
    let geosolve_sketch::DocumentProfileOffsetOperand::Face { outer, holes, .. } = operand else {
        panic!("mixed face operand expected");
    };
    assert!(holes.is_empty());
    assert_eq!(outer.edges.len(), 2);
    assert!(outer.junctions.iter().all(|junction| matches!(
        junction.branch,
        DocumentProfileOffsetJunctionBranch::Miter { .. }
    )));
    let target_families = outer
        .edges
        .iter()
        .map(|edge| {
            match preview
                .curve(edge.target.curve.curve)
                .expect("target curve")
                .definition
            {
                CurveDefinition::Line { .. } => "line",
                CurveDefinition::CircularArc { .. } => "arc",
                _ => "unsupported",
            }
        })
        .collect::<Vec<_>>();
    assert!(target_families.contains(&"line"));
    assert!(target_families.contains(&"arc"));
    assert!(!target_families.contains(&"unsupported"));
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one closed mixed-loop fixture proves tangent and miter branches coexist in a single authenticated operation"
)]
fn closed_mixed_loop_persists_tangent_and_miter_junctions_together() {
    let mut document = SketchDocument::new(10.0).expect("document");
    let a = document.add_point("a", [-2.0, 0.0]).unwrap();
    let b = document.add_point("b", [0.0, 0.0]).unwrap();
    let c = document.add_point("c", [1.0, 1.0]).unwrap();
    let d = document.add_point("d", [-2.0, 2.0]).unwrap();
    let tangent_line = line(&mut document, "tangent line", a, b);
    let diagonal = line(&mut document, "diagonal", c, d);
    let closing = line(&mut document, "closing", d, a);

    let center = document.add_point("arc center", [0.0, 1.0]).unwrap();
    let radius = document
        .add_scalar(
            "arc radius",
            1.0,
            ScalarUnit::Length,
            ScalarDomain::Positive,
        )
        .unwrap();
    let start_angle = document
        .add_scalar(
            "arc start",
            -std::f64::consts::FRAC_PI_2,
            ScalarUnit::Angle,
            ScalarDomain::Finite,
        )
        .unwrap();
    let end_angle = document
        .add_scalar("arc end", 0.0, ScalarUnit::Angle, ScalarDomain::Finite)
        .unwrap();
    let arc = CurveSpan::line(
        document
            .add_curve(
                "arc",
                CurveDefinition::CircularArc {
                    center,
                    radius,
                    start_angle,
                    end_angle,
                    sweep: DocumentArcSweep::CounterClockwise,
                },
            )
            .unwrap(),
    );
    for (label, point, parameter, neighborhood) in [
        ("arc start join", b, 0.0, ContactNeighborhood::Start),
        ("arc end join", c, 1.0, ContactNeighborhood::End),
    ] {
        let contact = document
            .add_curve_contact(label, arc, parameter, 0, neighborhood, None)
            .unwrap();
        document
            .add_constraint(
                label,
                DocumentConstraintDefinition::PointOnCurve { point, contact },
            )
            .unwrap();
    }

    let session = session(document);
    let index = operand_index(&session);
    let expected_spans = [tangent_line, arc, diagonal, closing]
        .into_iter()
        .collect::<std::collections::BTreeSet<_>>();
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
                    .map(|edge| edge.span)
                    .collect::<std::collections::BTreeSet<_>>()
                    == expected_spans
        })
        .expect("closed mixed line/arc face")
        .key
        .clone();
    let edit = proposal(
        &session,
        SketchOperationRequest::ProfileOffset {
            label: "mixed tangent miter face".into(),
            distance: 0.2,
            operand: SketchProfileOffsetOperand::Face {
                key: face,
                direction: DocumentFaceOffsetDirection::Outward,
            },
            operand_index: index,
        },
    )
    .profile_offset_document_edit()
    .expect("mixed tangent/miter edit");
    let OperationOutcome::Completed { value: patch, .. } = session
        .prepared_snapshot()
        .prepare(PreparedDocumentOperation::Apply(edit))
        .execute(OperationControl::unlimited())
        .expect("mixed tangent/miter patch")
    else {
        panic!("mixed tangent/miter patch must complete");
    };
    let patch_preview = patch.preview();
    let preview = patch_preview
        .accepted_document()
        .expect("accepted mixed tangent/miter preview");
    let DocumentProfileOffsetOperand::Face { outer, holes, .. } = profile_offset_operand(preview)
    else {
        panic!("face operand expected");
    };
    assert!(holes.is_empty());
    assert_eq!(outer.edges.len(), 4);
    assert_eq!(outer.junctions.len(), 4);
    assert_eq!(
        outer
            .junctions
            .iter()
            .filter(|junction| { junction.branch == DocumentProfileOffsetJunctionBranch::Tangent })
            .count(),
        1
    );
    assert_eq!(
        outer
            .junctions
            .iter()
            .filter(|junction| matches!(
                junction.branch,
                DocumentProfileOffsetJunctionBranch::Miter { .. }
            ))
            .count(),
        3
    );
    assert_current_hard_valid(
        patch_preview
            .accepted_session()
            .expect("accepted preview session"),
    );
}

#[test]
fn ordered_open_chain_preserves_side_traversal_joins_and_terminal_policy() {
    let mut document = SketchDocument::new(10.0).expect("document");
    let points = [
        document.add_point("a", [0.0, 0.0]).unwrap(),
        document.add_point("b", [4.0, 0.0]).unwrap(),
        document.add_point("c", [4.0, 3.0]).unwrap(),
    ];
    let first = line(&mut document, "first", points[0], points[1]);
    let second = line(&mut document, "second", points[1], points[2]);
    let mut session = session(document);
    let index = operand_index(&session);
    let spans = vec![
        OffsetDirectedSpan {
            span: first,
            traversal: OffsetTraversal::Forward,
        },
        OffsetDirectedSpan {
            span: second,
            traversal: OffsetTraversal::Forward,
        },
    ];

    let proposal = proposal(
        &session,
        SketchOperationRequest::ProfileOffset {
            label: "left chain".into(),
            distance: 1.0,
            operand: SketchProfileOffsetOperand::OpenChain {
                spans: spans.clone(),
                side: DocumentLineSide::Left,
            },
            operand_index: index,
        },
    );
    let outcome = proposal.apply(&mut session).expect("chain transaction");
    assert!(outcome.published_accepted_identity().is_some());
    let dimension = session
        .design_document()
        .dimensions()
        .last()
        .expect("offset dimension");
    let DocumentDimensionDefinition::ProfileOffset { operand, .. } = &dimension.definition else {
        panic!("ProfileOffset expected");
    };
    let geosolve_sketch::DocumentProfileOffsetOperand::OpenChain { side, chain } = operand else {
        panic!("open chain expected");
    };
    assert_eq!(*side, DocumentLineSide::Left);
    assert_eq!(chain.edges.len(), 2);
    assert_eq!(chain.junctions.len(), 1);
    assert_eq!(
        chain.start_terminal,
        DocumentProfileOffsetTerminalPolicy::NormalTranslation
    );
    assert_eq!(
        chain.end_terminal,
        DocumentProfileOffsetTerminalPolicy::NormalTranslation
    );
    assert_eq!(
        chain
            .edges
            .iter()
            .map(|edge| edge.source.curve)
            .collect::<Vec<_>>(),
        spans.iter().map(|span| span.span).collect::<Vec<_>>()
    );

    let dimension_id = dimension.id;
    session
        .transact(session.design_identity(), |document| {
            document.remove_with_owned_state(DocumentObjectId::Dimension(dimension_id))
        })
        .expect("delete association");
    assert_eq!(session.design_document().curves().len(), 4);
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one end-to-end fixture keeps distinct coincident endpoints and exact source/target owner assertions together"
)]
fn coincident_connected_chain_persists_the_exact_constraint_owner() {
    let mut document = SketchDocument::new(10.0).expect("document");
    let first_start = document.add_point("first start", [0.0, 0.0]).unwrap();
    let first_end = document.add_point("first end", [4.0, 0.0]).unwrap();
    let second_start = document.add_point("second start", [4.0, 0.0]).unwrap();
    let second_end = document.add_point("second end", [4.0, 3.0]).unwrap();
    let first = line(&mut document, "first", first_start, first_end);
    let second = line(&mut document, "second", second_start, second_end);
    let source_owner = document
        .add_constraint(
            "source endpoint join",
            DocumentConstraintDefinition::Coincident {
                first: first_end,
                second: second_start,
            },
        )
        .unwrap();
    let mut session = session(document);
    let index = operand_index(&session);
    let spans = vec![
        OffsetDirectedSpan {
            span: first,
            traversal: OffsetTraversal::Forward,
        },
        OffsetDirectedSpan {
            span: second,
            traversal: OffsetTraversal::Forward,
        },
    ];

    proposal(
        &session,
        SketchOperationRequest::ProfileOffset {
            label: "coincident chain".into(),
            distance: 0.5,
            operand: SketchProfileOffsetOperand::OpenChain {
                spans: spans.clone(),
                side: DocumentLineSide::Left,
            },
            operand_index: index,
        },
    )
    .apply(&mut session)
    .expect("Coincident-connected offset");

    let dimension = session.design_document().dimensions().last().unwrap();
    let dimension_id = dimension.id;
    let DocumentDimensionDefinition::ProfileOffset { operand, .. } = &dimension.definition else {
        panic!("ProfileOffset expected");
    };
    let geosolve_sketch::DocumentProfileOffsetOperand::OpenChain { chain, .. } = operand else {
        panic!("open chain expected");
    };
    let [junction] = chain.junctions.as_slice() else {
        panic!("one retained junction expected");
    };
    assert_eq!(
        junction.source_owner,
        DocumentProfileOffsetJunctionOwner::Constraint(source_owner)
    );
    assert!(matches!(
        junction.target_owner,
        DocumentProfileOffsetJunctionOwner::SharedPoint(_)
            | DocumentProfileOffsetJunctionOwner::Constraint(_)
    ));

    let accepted = session
        .accepted_state_for_current_input()
        .expect("offset construction must publish an accepted solve");
    assert!(accepted.solve_result().accepted());
    assert!(
        accepted
            .solve_result()
            .unstable_core_report()
            .hard_residuals_validated
    );
    assert!(
        accepted
            .solve_result()
            .unstable_core_report()
            .hard_residual_max
            <= 1.0e-9
    );
    assert!(
        accepted
            .solve_result()
            .geometry
            .points
            .iter()
            .all(|point| point.position.x.is_finite() && point.position.y.is_finite())
    );

    session
        .apply(
            session.design_identity(),
            DocumentEdit::SetElementUserSuppressed {
                element: source_owner.into(),
                suppressed: true,
            },
        )
        .expect("suppress retained Coincident owner");
    let activity = session.design_document().effective_activity();
    assert!(!activity.is_active(source_owner));
    assert!(!activity.is_active(dimension_id));
    let suppressed_index = operand_index(&session);
    let before = session.design_document().clone();
    let unavailable = execute(
        &session,
        SketchOperationRequest::ProfileOffset {
            label: "suppressed Coincident chain".into(),
            distance: 0.5,
            operand: SketchProfileOffsetOperand::OpenChain {
                spans,
                side: DocumentLineSide::Left,
            },
            operand_index: suppressed_index,
        },
    );
    assert!(matches!(
        unavailable,
        SketchOperationResult::Incomplete(incomplete)
            if incomplete.reason == SketchOperationIncompleteReason::ProfileOffsetDisconnectedJoin {
                incoming: first,
                outgoing: second,
            }
    ));
    assert_eq!(session.design_document(), &before);
}

#[test]
#[allow(clippy::too_many_lines)]
fn tangent_line_arc_chain_captures_the_tangent_branch_and_both_join_owners() {
    let mut document = SketchDocument::new(10.0).expect("document");
    let line_points = [
        document.add_point("line start", [-2.0, 0.0]).unwrap(),
        document.add_point("line end", [0.0, 0.0]).unwrap(),
    ];
    let line = line(&mut document, "line", line_points[0], line_points[1]);
    let center = document.add_point("arc center", [0.0, 1.0]).unwrap();
    let radius = document
        .add_scalar(
            "arc radius",
            1.0,
            ScalarUnit::Length,
            ScalarDomain::Positive,
        )
        .unwrap();
    let start_angle = document
        .add_scalar(
            "arc start",
            -std::f64::consts::FRAC_PI_2,
            ScalarUnit::Angle,
            ScalarDomain::Finite,
        )
        .unwrap();
    let end_angle = document
        .add_scalar("arc end", 0.0, ScalarUnit::Angle, ScalarDomain::Finite)
        .unwrap();
    let arc = CurveSpan::line(
        document
            .add_curve(
                "arc",
                CurveDefinition::CircularArc {
                    center,
                    radius,
                    start_angle,
                    end_angle,
                    sweep: DocumentArcSweep::CounterClockwise,
                },
            )
            .unwrap(),
    );
    let arc_start = document
        .add_curve_contact(
            "arc start endpoint",
            arc,
            0.0,
            0,
            ContactNeighborhood::Start,
            None,
        )
        .unwrap();
    let source_owner = document
        .add_constraint(
            "line arc join",
            DocumentConstraintDefinition::PointOnCurve {
                point: line_points[1],
                contact: arc_start,
            },
        )
        .unwrap();
    let mut session = session(document);
    let index = operand_index(&session);
    let proposal = proposal(
        &session,
        SketchOperationRequest::ProfileOffset {
            label: "tangent chain".into(),
            distance: 0.2,
            operand: SketchProfileOffsetOperand::OpenChain {
                spans: vec![
                    OffsetDirectedSpan {
                        span: line,
                        traversal: OffsetTraversal::Forward,
                    },
                    OffsetDirectedSpan {
                        span: arc,
                        traversal: OffsetTraversal::Forward,
                    },
                ],
                side: DocumentLineSide::Left,
            },
            operand_index: index,
        },
    );
    proposal.apply(&mut session).expect("tangent offset");
    let dimension = session.design_document().dimensions().last().unwrap();
    let DocumentDimensionDefinition::ProfileOffset { operand, .. } = &dimension.definition else {
        panic!("ProfileOffset expected");
    };
    let geosolve_sketch::DocumentProfileOffsetOperand::OpenChain { chain, .. } = operand else {
        panic!("open chain expected");
    };
    let [junction] = chain.junctions.as_slice() else {
        panic!("one tangent junction expected");
    };
    assert_eq!(
        junction.branch,
        DocumentProfileOffsetJunctionBranch::Tangent
    );
    assert_eq!(
        junction.source_owner,
        DocumentProfileOffsetJunctionOwner::Constraint(source_owner)
    );
    assert!(matches!(
        junction.target_owner,
        DocumentProfileOffsetJunctionOwner::Constraint(_)
    ));
    assert_ne!(junction.source_owner, junction.target_owner);
}

#[test]
fn unsupported_provenance_and_periodic_chain_are_typed_before_allocation() {
    let mut document = SketchDocument::new(10.0).expect("document");
    let points = [
        document.add_point("a", [0.0, 0.0]).unwrap(),
        document.add_point("b", [2.0, 0.0]).unwrap(),
    ];
    let construction = line(&mut document, "construction", points[0], points[1]);
    document
        .set_geometry_role(construction.curve, GeometryRole::Construction)
        .unwrap();
    let center = document.add_point("center", [5.0, 0.0]).unwrap();
    let radius = document
        .add_scalar("radius", 2.0, ScalarUnit::Length, ScalarDomain::Positive)
        .unwrap();
    let circle = CurveSpan::line(
        document
            .add_curve("circle", CurveDefinition::Circle { center, radius })
            .unwrap(),
    );
    let session = session(document);
    let index = operand_index(&session);

    let result = execute(
        &session,
        SketchOperationRequest::ProfileOffset {
            label: "unsupported role".into(),
            distance: 1.0,
            operand: SketchProfileOffsetOperand::OpenChain {
                spans: vec![OffsetDirectedSpan {
                    span: construction,
                    traversal: OffsetTraversal::Forward,
                }],
                side: DocumentLineSide::Left,
            },
            operand_index: Arc::clone(&index),
        },
    );
    let SketchOperationResult::Unsupported(unsupported) = result else {
        panic!("typed unsupported role expected");
    };
    assert_eq!(unsupported.kind, SketchOperationKind::ProfileOffset);
    assert_eq!(
        unsupported.reason,
        SketchOperationUnsupportedReason::ProfileOffsetSpan {
            span: construction,
            reasons: vec![OffsetOperandIneligibility::NonProfileGeometry],
        }
    );

    let circle_face = index
        .faces()
        .iter()
        .find(|face| face.key.outer.spans[0].span == circle)
        .expect("full-circle face")
        .key
        .clone();
    assert!(matches!(
        execute(
            &session,
            SketchOperationRequest::ProfileOffset {
                label: "circle face".into(),
                distance: 1.0,
                operand: SketchProfileOffsetOperand::Face {
                    key: circle_face,
                    direction: DocumentFaceOffsetDirection::Outward,
                },
                operand_index: Arc::clone(&index),
            },
        ),
        SketchOperationResult::Proposed(_)
    ));

    let result = execute(
        &session,
        SketchOperationRequest::ProfileOffset {
            label: "periodic chain".into(),
            distance: 1.0,
            operand: SketchProfileOffsetOperand::OpenChain {
                spans: vec![OffsetDirectedSpan {
                    span: circle,
                    traversal: OffsetTraversal::Forward,
                }],
                side: DocumentLineSide::Left,
            },
            operand_index: index,
        },
    );
    let SketchOperationResult::Unsupported(unsupported) = result else {
        panic!("full circle chain must be unsupported");
    };
    assert_eq!(
        unsupported.reason,
        SketchOperationUnsupportedReason::ProfileOffsetPeriodicChain { span: circle }
    );
}

#[test]
fn stale_index_and_disconnected_or_duplicate_chain_never_propose() {
    let mut document = SketchDocument::new(10.0).expect("document");
    let points = [
        document.add_point("a", [0.0, 0.0]).unwrap(),
        document.add_point("b", [2.0, 0.0]).unwrap(),
        document.add_point("c", [5.0, 0.0]).unwrap(),
        document.add_point("d", [7.0, 0.0]).unwrap(),
    ];
    let first = line(&mut document, "first", points[0], points[1]);
    let second = line(&mut document, "second", points[2], points[3]);
    let mut session = session(document);
    let index = operand_index(&session);

    let disconnected = execute(
        &session,
        SketchOperationRequest::ProfileOffset {
            label: "disconnected".into(),
            distance: 0.5,
            operand: SketchProfileOffsetOperand::OpenChain {
                spans: vec![
                    OffsetDirectedSpan {
                        span: first,
                        traversal: OffsetTraversal::Forward,
                    },
                    OffsetDirectedSpan {
                        span: second,
                        traversal: OffsetTraversal::Forward,
                    },
                ],
                side: DocumentLineSide::Left,
            },
            operand_index: Arc::clone(&index),
        },
    );
    assert!(matches!(
        disconnected,
        SketchOperationResult::Incomplete(incomplete)
            if incomplete.reason == SketchOperationIncompleteReason::ProfileOffsetDisconnectedJoin {
                incoming: first,
                outgoing: second,
            }
    ));

    let duplicate = execute(
        &session,
        SketchOperationRequest::ProfileOffset {
            label: "duplicate".into(),
            distance: 0.5,
            operand: SketchProfileOffsetOperand::OpenChain {
                spans: vec![
                    OffsetDirectedSpan {
                        span: first,
                        traversal: OffsetTraversal::Forward,
                    },
                    OffsetDirectedSpan {
                        span: first,
                        traversal: OffsetTraversal::Reverse,
                    },
                ],
                side: DocumentLineSide::Left,
            },
            operand_index: Arc::clone(&index),
        },
    );
    assert!(matches!(
        duplicate,
        SketchOperationResult::Incomplete(incomplete)
            if incomplete.reason == SketchOperationIncompleteReason::ProfileOffsetDuplicateSpan {
                span: first,
            }
    ));

    session
        .apply(
            session.design_identity(),
            DocumentEdit::CreatePoint {
                label: "new accepted input".into(),
                position: [10.0, 10.0],
            },
        )
        .expect("new accepted input");
    let before = session.design_document().clone();
    let stale = execute(
        &session,
        SketchOperationRequest::ProfileOffset {
            label: "stale".into(),
            distance: 0.5,
            operand: SketchProfileOffsetOperand::OpenChain {
                spans: vec![OffsetDirectedSpan {
                    span: first,
                    traversal: OffsetTraversal::Forward,
                }],
                side: DocumentLineSide::Right,
            },
            operand_index: index,
        },
    );
    assert!(matches!(
        stale,
        SketchOperationResult::Incomplete(incomplete)
            if incomplete.reason
                == SketchOperationIncompleteReason::ProfileOffsetIndexForDifferentInput
    ));
    assert_eq!(session.design_document(), &before);
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one non-axis fixture compares both material directions and then exercises retained source/target edits against the same grouped association"
)]
fn non_axis_polygon_offsets_both_face_directions_and_survives_source_and_target_edits() {
    let mut document = SketchDocument::new(10.0).expect("document");
    let (source_points, source_spans) = polygon(
        &mut document,
        "oblique pentagon",
        &[
            [-4.0, -1.0],
            [1.0, -2.5],
            [4.5, 0.5],
            [2.0, 4.0],
            [-3.5, 3.0],
        ],
    );

    for (direction, expected_signed_distance) in [
        (DocumentFaceOffsetDirection::Outward, -0.4),
        (DocumentFaceOffsetDirection::Inward, 0.4),
    ] {
        let mut session = session(document.clone());
        let index = operand_index(&session);
        let face = index
            .faces()
            .iter()
            .find(|face| {
                face.eligibility.is_eligible()
                    && face.key.holes.is_empty()
                    && face.key.outer.spans.len() == source_spans.len()
                    && source_spans
                        .iter()
                        .all(|span| face.key.outer.spans.iter().any(|edge| edge.span == *span))
            })
            .expect("eligible non-axis polygon face")
            .key
            .clone();
        proposal(
            &session,
            SketchOperationRequest::ProfileOffset {
                label: format!("{direction:?} oblique offset"),
                distance: 0.4,
                operand: SketchProfileOffsetOperand::Face {
                    key: face,
                    direction,
                },
                operand_index: index,
            },
        )
        .apply(&mut session)
        .expect("non-axis offset transaction");
        assert_current_hard_valid(&session);

        let operand_before_edits = profile_offset_operand(session.design_document()).clone();
        let DocumentProfileOffsetOperand::Face {
            direction: retained_direction,
            outer,
            holes,
        } = &operand_before_edits
        else {
            panic!("face operand expected");
        };
        assert_eq!(*retained_direction, direction);
        assert!(holes.is_empty());
        assert_eq!(outer.edges.len(), source_spans.len());
        assert_eq!(outer.junctions.len(), source_spans.len());
        assert!(outer.junctions.iter().all(|junction| matches!(
            junction.branch,
            DocumentProfileOffsetJunctionBranch::Miter { .. }
        )));
        for edge in &outer.edges {
            assert_signed_line_offset(
                session
                    .accepted_state_for_current_input()
                    .unwrap()
                    .document(),
                edge,
                expected_signed_distance,
            );
        }

        if direction == DocumentFaceOffsetDirection::Outward {
            let source = source_points[1];
            let source_before = session
                .accepted_state_for_current_input()
                .unwrap()
                .document()
                .point(source)
                .unwrap()
                .position;
            let source_edit = session
                .apply(
                    session.design_identity(),
                    DocumentEdit::SetPointPosition {
                        point: source,
                        position: [source_before[0] + 0.15, source_before[1] + 0.1],
                    },
                )
                .expect("regular retained source edit");
            assert!(source_edit.published_accepted_identity().is_some());
            assert_current_hard_valid(&session);
            let source_after = session
                .accepted_state_for_current_input()
                .unwrap()
                .document()
                .point(source)
                .unwrap()
                .position;
            assert!(
                (source_after[0] - source_before[0]).hypot(source_after[1] - source_before[1])
                    > 1.0e-4
            );

            let target = match outer.junctions[0].target_owner {
                DocumentProfileOffsetJunctionOwner::SharedPoint(point) => point,
                DocumentProfileOffsetJunctionOwner::Constraint(_) => {
                    panic!("line-only target junction must own a shared point")
                }
            };
            let target_before = session
                .accepted_state_for_current_input()
                .unwrap()
                .document()
                .point(target)
                .unwrap()
                .position;
            let target_edit = session
                .apply(
                    session.design_identity(),
                    DocumentEdit::SetPointPosition {
                        point: target,
                        position: [target_before[0] - 0.12, target_before[1] + 0.08],
                    },
                )
                .expect("regular retained target edit");
            assert!(target_edit.published_accepted_identity().is_some());
            assert_current_hard_valid(&session);
            let target_after = session
                .accepted_state_for_current_input()
                .unwrap()
                .document()
                .point(target)
                .unwrap()
                .position;
            assert!(
                (target_after[0] - target_before[0]).hypot(target_after[1] - target_before[1])
                    > 1.0e-4
            );
            assert_eq!(
                profile_offset_operand(session.design_document()),
                &operand_before_edits,
                "coordinate edits must preserve every explicit offset branch and operand identity"
            );
        }
    }
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one face-level fixture keeps two mixed holes, both material directions, and their atomic collapse/contact barriers directly comparable"
)]
fn mixed_multi_hole_face_offsets_as_one_operand_and_rejects_collapse_or_contact_atomically() {
    let mut document = SketchDocument::new(20.0).expect("document");
    let (_, outer_spans) = polygon(
        &mut document,
        "outer",
        &[[-20.0, -12.0], [20.0, -12.0], [20.0, 12.0], [-20.0, 12.0]],
    );
    let (_, polygon_hole_spans) = polygon(
        &mut document,
        "polygon hole",
        &[[-7.0, -2.0], [-3.0, -2.0], [-3.0, 2.0], [-7.0, 2.0]],
    );
    let circle_center = document
        .add_point("circle hole center", [5.0, 0.0])
        .unwrap();
    let circle_radius_id = document
        .add_scalar(
            "circle hole radius",
            1.5,
            ScalarUnit::Length,
            ScalarDomain::Positive,
        )
        .unwrap();
    let circle_hole = CurveSpan::line(
        document
            .add_curve(
                "circle hole",
                CurveDefinition::Circle {
                    center: circle_center,
                    radius: circle_radius_id,
                },
            )
            .unwrap(),
    );

    let source_session = session(document.clone());
    let source_index = operand_index(&source_session);
    let face = source_index
        .faces()
        .iter()
        .find(|face| {
            face.eligibility.is_eligible()
                && face.key.outer.spans.len() == outer_spans.len()
                && face.key.holes.len() == 2
                && outer_spans
                    .iter()
                    .all(|span| face.key.outer.spans.iter().any(|edge| edge.span == *span))
                && face
                    .key
                    .holes
                    .iter()
                    .any(|hole| hole.spans.len() == 1 && hole.spans[0].span == circle_hole)
                && polygon_hole_spans.iter().all(|span| {
                    face.key
                        .holes
                        .iter()
                        .flat_map(|hole| &hole.spans)
                        .any(|edge| edge.span == *span)
                })
        })
        .expect("one eligible mixed two-hole face")
        .key
        .clone();

    for (direction, signed_line_distance, expected_circle_radius) in [
        (DocumentFaceOffsetDirection::Outward, -0.5, 1.0),
        (DocumentFaceOffsetDirection::Inward, 0.5, 2.0),
    ] {
        let mut session = session(document.clone());
        let index = operand_index(&session);
        proposal(
            &session,
            SketchOperationRequest::ProfileOffset {
                label: format!("{direction:?} mixed-hole offset"),
                distance: 0.5,
                operand: SketchProfileOffsetOperand::Face {
                    key: face.clone(),
                    direction,
                },
                operand_index: index,
            },
        )
        .apply(&mut session)
        .expect("mixed-hole offset transaction");
        assert_current_hard_valid(&session);

        let accepted = session
            .accepted_state_for_current_input()
            .unwrap()
            .document();
        let DocumentProfileOffsetOperand::Face {
            direction: retained_direction,
            outer,
            holes,
        } = profile_offset_operand(accepted)
        else {
            panic!("mixed-hole face operand expected");
        };
        assert_eq!(*retained_direction, direction);
        assert_eq!(outer.edges.len(), 4);
        assert_eq!(holes.len(), 2);
        assert_eq!(holes.iter().map(|hole| hole.edges.len()).sum::<usize>(), 5);
        for edge in outer
            .edges
            .iter()
            .chain(holes.iter().flat_map(|hole| &hole.edges))
        {
            if matches!(
                accepted.curve(edge.source.curve.curve).unwrap().definition,
                CurveDefinition::Line { .. }
            ) {
                assert_signed_line_offset(accepted, edge, signed_line_distance);
            }
        }
        let circle_pair = holes
            .iter()
            .flat_map(|hole| &hole.edges)
            .find(|edge| edge.source.curve == circle_hole)
            .expect("circular hole pair");
        assert!(
            (circle_radius(accepted, circle_pair.target.curve) - expected_circle_radius).abs()
                <= 1.0e-9
        );
    }

    let before_input = source_session.prepared_input();
    let before_document = source_session.design_document().clone();
    let before_accepted = source_session
        .accepted_state()
        .map(geosolve_sketch::SketchAcceptedDocumentState::identity);
    let collapse = SketchOperationSnapshot::capture(&source_session)
        .prepare(SketchOperationRequest::ProfileOffset {
            label: "collapsed circular hole".into(),
            distance: 1.5,
            operand: SketchProfileOffsetOperand::Face {
                key: face.clone(),
                direction: DocumentFaceOffsetDirection::Outward,
            },
            operand_index: Arc::clone(&source_index),
        })
        .execute(OperationControl::unlimited());
    assert!(
        collapse.is_err(),
        "zero-radius hole must fail before proposal"
    );
    assert_eq!(source_session.prepared_input(), before_input);
    assert_eq!(source_session.design_document(), &before_document);
    assert_eq!(
        source_session
            .accepted_state()
            .map(geosolve_sketch::SketchAcceptedDocumentState::identity),
        before_accepted
    );

    let contact_proposal = proposal(
        &source_session,
        SketchOperationRequest::ProfileOffset {
            label: "hole contact barrier".into(),
            distance: 3.5,
            operand: SketchProfileOffsetOperand::Face {
                key: face,
                direction: DocumentFaceOffsetDirection::Inward,
            },
            operand_index: source_index,
        },
    );
    let contact_edit = contact_proposal
        .profile_offset_document_edit()
        .expect("contact candidate retains an authenticated prepared edit");
    let OperationOutcome::Completed {
        value: rejected_patch,
        ..
    } = source_session
        .prepared_snapshot()
        .prepare(PreparedDocumentOperation::Apply(contact_edit))
        .execute(OperationControl::unlimited())
        .expect("contact candidate solve executes")
    else {
        panic!("unbounded contact candidate must complete");
    };
    assert!(
        rejected_patch.preview().accepted_state().is_none(),
        "hole/hole contact or overlap must not become accepted geometry"
    );
    let rejected_preview = rejected_patch.preview();
    let rejected_design = rejected_preview.design_document();
    let DocumentProfileOffsetOperand::Face { holes, .. } = profile_offset_operand(rejected_design)
    else {
        panic!("rejected candidate must retain the complete face operand");
    };
    let circle_pair = holes
        .iter()
        .flat_map(|hole| &hole.edges)
        .find(|edge| edge.source.curve == circle_hole)
        .expect("rejected circular-hole pair");
    let CurveDefinition::Circle { center, radius } = &rejected_design
        .curve(circle_pair.target.curve.curve)
        .unwrap()
        .definition
    else {
        panic!("rejected circular target remains native");
    };
    let center = rejected_design.point(*center).unwrap().position;
    let radius = rejected_design.scalar(*radius).unwrap().value;
    let polygon_target = holes
        .iter()
        .find(|hole| hole.edges.len() == polygon_hole_spans.len())
        .expect("rejected polygon-hole target");
    assert!(
        polygon_target.edges.iter().any(|edge| line_crosses_circle(
            rejected_design,
            edge.target.curve,
            center,
            radius
        )),
        "independent analytic check must reproduce the forbidden target-contour crossing"
    );
    assert_eq!(source_session.prepared_input(), before_input);
    assert_eq!(source_session.design_document(), &before_document);
    assert_eq!(
        source_session
            .accepted_state()
            .map(geosolve_sketch::SketchAcceptedDocumentState::identity),
        before_accepted
    );

    let mut direct_session = source_session.clone();
    let direct_result = contact_proposal.apply(&mut direct_session);
    assert!(
        matches!(
            direct_result,
            Err(SketchOperationApplyError::ProfileOffsetRejected)
        ),
        "the direct public operation path must reject rather than retain a topology-invalid Profile Offset design"
    );
    assert_eq!(direct_session.prepared_input(), before_input);
    assert_eq!(direct_session.design_document(), &before_document);
    assert_eq!(
        direct_session
            .accepted_state()
            .map(geosolve_sketch::SketchAcceptedDocumentState::identity),
        before_accepted
    );

    let mut controlled_session = source_session.clone();
    let controlled_result =
        contact_proposal.apply_controlled(&mut controlled_session, OperationControl::unlimited());
    assert!(matches!(
        controlled_result,
        Err(SketchOperationApplyError::ProfileOffsetRejected)
    ));
    assert_eq!(controlled_session.prepared_input(), before_input);
    assert_eq!(controlled_session.design_document(), &before_document);
    assert_eq!(
        controlled_session
            .accepted_state()
            .map(geosolve_sketch::SketchAcceptedDocumentState::identity),
        before_accepted
    );
}

#[test]
fn profile_offset_cancellation_and_exhaustion_leave_all_live_authority_unchanged() {
    let mut document = SketchDocument::new(10.0).expect("document");
    document
        .add_rectangle("source", [0.0, 0.0], 4.0, 3.0)
        .expect("rectangle");
    let mut session = session(document);
    let index = operand_index(&session);
    let face = index.faces().first().expect("rectangle face").key.clone();
    let request = || SketchOperationRequest::ProfileOffset {
        label: "controlled profile offset".into(),
        distance: 0.5,
        operand: SketchProfileOffsetOperand::Face {
            key: face.clone(),
            direction: DocumentFaceOffsetDirection::Outward,
        },
        operand_index: Arc::clone(&index),
    };
    let before_input = session.prepared_input();
    let before_document = session.design_document().clone();
    let before_accepted = session
        .accepted_state()
        .map(geosolve_sketch::SketchAcceptedDocumentState::identity);

    let (cancel_handle, cancel_token) = cancellation_pair();
    cancel_handle.cancel();
    let cancelled = SketchOperationSnapshot::capture(&session)
        .prepare(request())
        .execute(OperationControl::new(
            cancel_token,
            OperationLimits::unlimited(),
        ))
        .expect("controlled preparation");
    assert!(matches!(cancelled, OperationOutcome::Cancelled { .. }));

    let mut preparation_limits = OperationLimits::unlimited();
    preparation_limits.document_dependency_items = 0;
    let exhausted = SketchOperationSnapshot::capture(&session)
        .prepare(request())
        .execute(OperationControl::new(
            CancellationToken::default(),
            preparation_limits,
        ))
        .expect("controlled preparation");
    assert!(matches!(exhausted, OperationOutcome::WorkExhausted { .. }));
    assert_eq!(session.prepared_input(), before_input);
    assert_eq!(session.design_document(), &before_document);
    assert_eq!(
        session
            .accepted_state()
            .map(geosolve_sketch::SketchAcceptedDocumentState::identity),
        before_accepted
    );

    let proposal = proposal(&session, request());
    let (apply_cancel_handle, apply_cancel_token) = cancellation_pair();
    apply_cancel_handle.cancel();
    let cancelled_apply = proposal
        .apply_controlled(
            &mut session,
            OperationControl::new(apply_cancel_token, OperationLimits::unlimited()),
        )
        .expect("controlled application");
    assert!(matches!(
        cancelled_apply,
        OperationOutcome::Cancelled { .. }
    ));

    let mut application_limits = OperationLimits::unlimited();
    application_limits.document_validation_items = 0;
    let exhausted_apply = proposal
        .apply_controlled(
            &mut session,
            OperationControl::new(CancellationToken::default(), application_limits),
        )
        .expect("controlled application");
    assert!(matches!(
        exhausted_apply,
        OperationOutcome::WorkExhausted { .. }
    ));
    assert_eq!(session.prepared_input(), before_input);
    assert_eq!(session.design_document(), &before_document);
    assert_eq!(
        session
            .accepted_state()
            .map(geosolve_sketch::SketchAcceptedDocumentState::identity),
        before_accepted
    );

    let accepted = proposal
        .apply_controlled(&mut session, OperationControl::unlimited())
        .expect("accepted controlled application");
    let OperationOutcome::Completed { value, .. } = accepted else {
        panic!("unlimited valid application must complete");
    };
    assert!(value.published_accepted_identity().is_some());
    assert_current_hard_valid(&session);
}
