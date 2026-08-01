// SPDX-License-Identifier: GPL-3.0-or-later

#![allow(clippy::float_cmp)]

use geosolve_sketch::{
    CurveDefinition, CurveId, CurveSpan, DesignPointId, DesignScalarId,
    DocumentConstraintDefinition, DocumentDimensionDefinition, DocumentDimensionId,
    DocumentDimensionMode, DocumentLineOffsetOrientation, DocumentLineSide, DocumentSolveRequest,
    MAX_LABEL_BYTES, OperationControl, OperationLimits, OperationOutcome, PersistentId,
    RetainedSketchDocumentSession, ScalarDomain, ScalarUnit, SketchDocument, SolverConfig,
    cancellation_pair,
};
use geosolve_sketch_ops::{
    AssociativeLineOffsetMode, LineOffsetChainSpan, SketchOperationApplication,
    SketchOperationApplyError, SketchOperationIdentityChange, SketchOperationIncompleteReason,
    SketchOperationKind, SketchOperationProposal, SketchOperationRequest, SketchOperationResult,
    SketchOperationSnapshot, SketchOperationUnsupportedReason,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct OffsetIds {
    source: CurveSpan,
    target_start: DesignPointId,
    target_end: DesignPointId,
    target_segment: geosolve_sketch::CurveId,
    distance: DesignScalarId,
    dimension: DocumentDimensionId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct JoinedOffsetIds {
    sources: Vec<LineOffsetChainSpan>,
    target_points: Vec<DesignPointId>,
    target_curve: CurveId,
}

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
) -> (geosolve_sketch::CurveId, [DesignPointId; 2]) {
    let points = [
        document.add_point(format!("{label}.start"), start).unwrap(),
        document.add_point(format!("{label}.end"), end).unwrap(),
    ];
    let delta = subtract(end, start);
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

fn line_between(
    document: &mut SketchDocument,
    label: &str,
    start: DesignPointId,
    end: DesignPointId,
) -> CurveId {
    let start_position = document.point(start).unwrap().position;
    let end_position = document.point(end).unwrap().position;
    let delta = subtract(end_position, start_position);
    let length = delta[0].hypot(delta[1]);
    let branch_direction = if length > 0.0 && length.is_finite() {
        [delta[0] / length, delta[1] / length]
    } else {
        [1.0, 0.0]
    };
    document
        .add_curve(
            label,
            CurveDefinition::Line {
                start,
                end,
                branch_direction,
            },
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

fn offset_ids(application: &SketchOperationApplication) -> OffsetIds {
    application
        .identity_changes
        .iter()
        .find_map(|change| match change {
            SketchOperationIdentityChange::AssociativeLineOffset {
                source,
                target_start,
                target_end,
                target_segment,
                distance,
                dimension,
            } => Some(OffsetIds {
                source: *source,
                target_start: *target_start,
                target_end: *target_end,
                target_segment: *target_segment,
                distance: *distance,
                dimension: *dimension,
            }),
            _ => None,
        })
        .expect("line-offset proposal must publish its typed identity mapping")
}

fn joined_offset_ids(application: &SketchOperationApplication) -> JoinedOffsetIds {
    application
        .identity_changes
        .iter()
        .find_map(|change| match change {
            SketchOperationIdentityChange::JoinedLineOffset {
                sources,
                target_points,
                target_curve,
            } => Some(JoinedOffsetIds {
                sources: sources.clone(),
                target_points: target_points.clone(),
                target_curve: *target_curve,
            }),
            _ => None,
        })
        .expect("joined line-offset proposal must publish its typed identity mapping")
}

fn joined_source(
    source: CurveId,
    orientation: DocumentLineOffsetOrientation,
) -> LineOffsetChainSpan {
    LineOffsetChainSpan {
        source: CurveSpan::line(source),
        orientation,
    }
}

fn joined_request(
    label: &str,
    sources: Vec<LineOffsetChainSpan>,
    distance: f64,
    side: DocumentLineSide,
) -> SketchOperationRequest {
    SketchOperationRequest::JoinedLineOffset {
        label: label.into(),
        sources,
        distance,
        side,
    }
}

fn incomplete_reason(
    session: &RetainedSketchDocumentSession,
    request: SketchOperationRequest,
) -> SketchOperationIncompleteReason {
    let outcome = SketchOperationSnapshot::capture(session)
        .prepare(request)
        .execute(OperationControl::default())
        .unwrap();
    let OperationOutcome::Completed { value, .. } = outcome else {
        panic!("uncontrolled operation must complete");
    };
    let SketchOperationResult::Incomplete(incomplete) = value else {
        panic!("typed incomplete result expected");
    };
    incomplete.reason
}

fn transformed(point: [f64; 2], scale: f64, angle: f64, translation: [f64; 2]) -> [f64; 2] {
    let (sine, cosine) = angle.sin_cos();
    [
        scale * cosine.mul_add(point[0], -sine * point[1]) + translation[0],
        scale * sine.mul_add(point[0], cosine * point[1]) + translation[1],
    ]
}

fn expected_offset(
    start: [f64; 2],
    end: [f64; 2],
    distance: f64,
    side: DocumentLineSide,
) -> ([f64; 2], [f64; 2], [f64; 2]) {
    let delta = subtract(end, start);
    let length = delta[0].hypot(delta[1]);
    let direction = [delta[0] / length, delta[1] / length];
    let sign = match side {
        DocumentLineSide::Left => 1.0,
        DocumentLineSide::Right => -1.0,
    };
    let offset = [
        -direction[1] * distance * sign,
        direction[0] * distance * sign,
    ];
    (add(start, offset), add(end, offset), direction)
}

fn assert_point(actual: [f64; 2], expected: [f64; 2], scale: f64) {
    let tolerance = scale.abs().max(1.0e-12) * 2.0e-8;
    assert!(
        (actual[0] - expected[0]).abs() <= tolerance
            && (actual[1] - expected[1]).abs() <= tolerance,
        "actual={actual:?}, expected={expected:?}, tolerance={tolerance}"
    );
}

fn mixed_corner_fixture() -> (
    RetainedSketchDocumentSession,
    Vec<LineOffsetChainSpan>,
    CurveId,
) {
    let mut document = SketchDocument::new(10.0).unwrap();
    let points = [
        document.add_point("a", [0.0, 0.0]).unwrap(),
        document.add_point("b", [4.0, 0.0]).unwrap(),
        document.add_point("c", [4.0, 3.0]).unwrap(),
        document.add_point("branch", [7.0, -2.0]).unwrap(),
    ];
    let first = line_between(&mut document, "first", points[0], points[1]);
    let second = line_between(&mut document, "second reversed", points[2], points[1]);
    let branch = line_between(&mut document, "unrequested branch", points[1], points[3]);
    fix_points(
        &mut document,
        &[
            (points[0], [0.0, 0.0]),
            (points[1], [4.0, 0.0]),
            (points[2], [4.0, 3.0]),
            (points[3], [7.0, -2.0]),
        ],
    );
    (
        session(document),
        vec![
            joined_source(first, DocumentLineOffsetOrientation::Same),
            joined_source(second, DocumentLineOffsetOrientation::Reversed),
        ],
        branch,
    )
}

#[test]
fn joined_offset_uses_explicit_mixed_traversal_and_creates_one_editable_polyline() {
    let (mut session, sources, unrequested_branch) = mixed_corner_fixture();
    let before = session.design_document();
    let before_counts = (
        before.points().len(),
        before.curves().len(),
        before.scalars().len(),
        before.constraints().len(),
        before.dimensions().len(),
    );
    let request = joined_request("joined", sources.clone(), 1.0, DocumentLineSide::Left);
    assert_eq!(request.kind(), SketchOperationKind::JoinedLineOffset);
    let first = proposed(&session, request.clone());
    let second = proposed(&session, request);
    assert_eq!(first.expected_application(), second.expected_application());
    assert_eq!(
        first.expected_application().kind,
        SketchOperationKind::JoinedLineOffset
    );
    assert_eq!(first.expected_application().identity_changes.len(), 1);
    let ids = joined_offset_ids(first.expected_application());
    assert_eq!(ids.sources, sources);
    assert_eq!(ids.target_points.len(), sources.len() + 1);
    assert!(
        ids.sources
            .iter()
            .all(|source| source.source.curve != unrequested_branch)
    );

    let outcome = first.apply(&mut session).unwrap();
    assert!(outcome.published_accepted_identity().is_some());
    let accepted = session.accepted_state().unwrap().document();
    assert_eq!(accepted.points().len(), before_counts.0 + 3);
    assert_eq!(accepted.curves().len(), before_counts.1 + 1);
    assert_eq!(accepted.scalars().len(), before_counts.2);
    assert_eq!(accepted.constraints().len(), before_counts.3);
    assert_eq!(accepted.dimensions().len(), before_counts.4);
    for (point, expected) in
        ids.target_points
            .iter()
            .copied()
            .zip([[0.0, 1.0], [3.0, 1.0], [3.0, 3.0]])
    {
        assert_point(accepted.point(point).unwrap().position, expected, 1.0);
    }
    let CurveDefinition::Polyline {
        points,
        closed,
        branch_directions,
    } = &accepted.curve(ids.target_curve).unwrap().definition
    else {
        panic!("joined offset must be one ordinary polyline");
    };
    assert_eq!(points, &ids.target_points);
    assert!(!closed);
    assert_eq!(branch_directions, &vec![[1.0, 0.0], [0.0, 1.0]]);

    let edited = session
        .transact(session.design_identity(), |document| {
            document.set_point_position(ids.target_points[1], [2.5, 1.5])?;
            Ok(())
        })
        .unwrap();
    assert!(edited.published_accepted_identity().is_some());
    assert_point(
        session
            .accepted_state()
            .unwrap()
            .document()
            .point(ids.target_points[1])
            .unwrap()
            .position,
        [2.5, 1.5],
        1.0,
    );
}

#[test]
fn joined_offset_global_side_controls_exact_miter_coordinates() {
    for (side, expected) in [
        (DocumentLineSide::Left, [[0.0, 1.0], [3.0, 1.0], [3.0, 3.0]]),
        (
            DocumentLineSide::Right,
            [[0.0, -1.0], [5.0, -1.0], [5.0, 3.0]],
        ),
    ] {
        let (mut session, sources, _) = mixed_corner_fixture();
        let proposal = proposed(&session, joined_request("sided joined", sources, 1.0, side));
        let ids = joined_offset_ids(proposal.expected_application());
        proposal.apply(&mut session).unwrap();
        let accepted = session.accepted_state().unwrap().document();
        for (point, expected) in ids.target_points.into_iter().zip(expected) {
            assert_point(accepted.point(point).unwrap().position, expected, 1.0);
        }
    }
}

#[test]
fn joined_offset_keeps_same_direction_collinear_vertices_shared() {
    let mut document = SketchDocument::new(10.0).unwrap();
    let points = [
        document.add_point("a", [0.0, 0.0]).unwrap(),
        document.add_point("b", [4.0, 0.0]).unwrap(),
        document.add_point("c", [9.0, 0.0]).unwrap(),
    ];
    let polyline = document
        .add_curve(
            "collinear source polyline",
            CurveDefinition::Polyline {
                points: points.to_vec(),
                closed: false,
                branch_directions: vec![[1.0, 0.0], [1.0, 0.0]],
            },
        )
        .unwrap();
    let mut session = session(document);
    let proposal = proposed(
        &session,
        joined_request(
            "collinear",
            vec![
                LineOffsetChainSpan {
                    source: CurveSpan {
                        curve: polyline,
                        segment: 0,
                    },
                    orientation: DocumentLineOffsetOrientation::Same,
                },
                LineOffsetChainSpan {
                    source: CurveSpan {
                        curve: polyline,
                        segment: 1,
                    },
                    orientation: DocumentLineOffsetOrientation::Same,
                },
            ],
            2.0,
            DocumentLineSide::Left,
        ),
    );
    let ids = joined_offset_ids(proposal.expected_application());
    proposal.apply(&mut session).unwrap();
    let accepted = session.accepted_state().unwrap().document();
    for (point, expected) in ids
        .target_points
        .into_iter()
        .zip([[0.0, 2.0], [4.0, 2.0], [9.0, 2.0]])
    {
        assert_point(accepted.point(point).unwrap().position, expected, 1.0);
    }
}

#[test]
fn joined_offset_accepts_the_inclusive_thirty_two_span_limit() {
    let mut document = SketchDocument::new(40.0).unwrap();
    let points = (0_u32..=32)
        .map(|index| {
            document.add_point(
                format!("maximum source point {index}"),
                [f64::from(index), 0.0],
            )
        })
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    let source_curve = document
        .add_curve(
            "maximum source polyline",
            CurveDefinition::Polyline {
                points,
                closed: false,
                branch_directions: vec![[1.0, 0.0]; 32],
            },
        )
        .unwrap();
    let sources = (0_u32..32)
        .map(|segment| LineOffsetChainSpan {
            source: CurveSpan {
                curve: source_curve,
                segment,
            },
            orientation: DocumentLineOffsetOrientation::Same,
        })
        .collect::<Vec<_>>();
    let mut session = session(document);
    let proposal = proposed(
        &session,
        joined_request(
            "maximum joined",
            sources.clone(),
            1.0,
            DocumentLineSide::Left,
        ),
    );
    let ids = joined_offset_ids(proposal.expected_application());
    assert_eq!(ids.sources, sources);
    assert_eq!(ids.target_points.len(), 33);
    proposal.apply(&mut session).unwrap();
    let CurveDefinition::Polyline { points, closed, .. } = &session
        .accepted_state()
        .unwrap()
        .document()
        .curve(ids.target_curve)
        .unwrap()
        .definition
    else {
        panic!("maximum joined offset must remain one polyline");
    };
    assert_eq!(points.len(), 33);
    assert!(!closed);
}

#[test]
#[allow(clippy::too_many_lines)]
fn associative_offsets_cover_modes_sides_reversal_scales_and_rigid_transforms() {
    for scale in [1.0e-6, 1.0, 1.0e6] {
        let angle = 0.61;
        let translation = [7.0 * scale, -4.0 * scale];
        for reversed in [false, true] {
            let mut source_positions = [
                transformed([0.0, 0.0], scale, angle, translation),
                transformed([4.0, 0.0], scale, angle, translation),
            ];
            if reversed {
                source_positions.reverse();
            }
            for side in [DocumentLineSide::Left, DocumentLineSide::Right] {
                for mode in [
                    AssociativeLineOffsetMode::ExactTranslatedSegment,
                    AssociativeLineOffsetMode::SupportingLine,
                ] {
                    let mut document = SketchDocument::new(10.0 * scale).unwrap();
                    let (source_curve, source_points) = line(
                        &mut document,
                        "source",
                        source_positions[0],
                        source_positions[1],
                    );
                    fix_points(
                        &mut document,
                        &[
                            (source_points[0], source_positions[0]),
                            (source_points[1], source_positions[1]),
                        ],
                    );
                    let mut session = session(document);
                    let distance = 2.0 * scale;
                    let source = CurveSpan::line(source_curve);
                    let proposal = proposed(
                        &session,
                        SketchOperationRequest::AssociativeLineOffset {
                            label: "offset".into(),
                            source,
                            distance,
                            side,
                            mode,
                        },
                    );
                    assert_eq!(
                        proposal.accepted_state_identity(),
                        session
                            .accepted_state()
                            .map(geosolve_sketch::SketchAcceptedDocumentState::identity)
                    );
                    assert_eq!(
                        proposal.expected_application().kind,
                        SketchOperationKind::AssociativeLineOffset
                    );
                    let ids = offset_ids(proposal.expected_application());
                    assert_eq!(ids.source, source);

                    let outcome = proposal.apply(&mut session).unwrap();
                    assert!(outcome.published_accepted_identity().is_some());
                    let accepted = session.accepted_state().unwrap().document();
                    let (expected_start, expected_end, expected_direction) =
                        expected_offset(source_positions[0], source_positions[1], distance, side);
                    assert_point(
                        accepted.point(ids.target_start).unwrap().position,
                        expected_start,
                        scale,
                    );
                    assert_point(
                        accepted.point(ids.target_end).unwrap().position,
                        expected_end,
                        scale,
                    );
                    let CurveDefinition::Line {
                        start,
                        end,
                        branch_direction,
                    } = accepted.curve(ids.target_segment).unwrap().definition
                    else {
                        panic!("offset target must be one ordinary line");
                    };
                    assert_eq!([start, end], [ids.target_start, ids.target_end]);
                    assert_point(branch_direction, expected_direction, 1.0);
                    let scalar = accepted.scalar(ids.distance).unwrap();
                    assert_eq!(scalar.value.to_bits(), distance.to_bits());
                    assert_eq!(scalar.unit, ScalarUnit::Length);
                    assert_eq!(scalar.domain, ScalarDomain::Positive);

                    let dimension = accepted.dimension(ids.dimension).unwrap();
                    assert_eq!(dimension.mode, DocumentDimensionMode::Driving);
                    let common = match &dimension.definition {
                        DocumentDimensionDefinition::ExactTranslatedSegmentOffset {
                            source,
                            target_segment,
                            target,
                            side,
                            orientation,
                        } if mode == AssociativeLineOffsetMode::ExactTranslatedSegment => {
                            (*source, *target_segment, *target, *side, *orientation)
                        }
                        DocumentDimensionDefinition::SupportingLineOffset {
                            source,
                            target_segment,
                            target,
                            side,
                            orientation,
                        } if mode == AssociativeLineOffsetMode::SupportingLine => {
                            (*source, *target_segment, *target, *side, *orientation)
                        }
                        other => panic!("wrong offset definition: {other:?}"),
                    };
                    assert_eq!(common.0, source);
                    assert_eq!(common.1, CurveSpan::line(ids.target_segment));
                    assert_eq!(common.2, ids.distance);
                    assert_eq!(common.3, side);
                    assert_eq!(common.4, DocumentLineOffsetOrientation::Same);
                }
            }
        }
    }
}

#[test]
fn polyline_span_mapping_is_deterministic_and_offset_target_is_editable() {
    let mut document = SketchDocument::new(10.0).unwrap();
    let points = [
        document.add_point("polyline p0", [0.0, 0.0]).unwrap(),
        document.add_point("polyline p1", [4.0, 0.0]).unwrap(),
        document.add_point("polyline p2", [4.0, 3.0]).unwrap(),
    ];
    fix_points(
        &mut document,
        &[
            (points[0], [0.0, 0.0]),
            (points[1], [4.0, 0.0]),
            (points[2], [4.0, 3.0]),
        ],
    );
    let polyline = document
        .add_curve(
            "polyline",
            CurveDefinition::Polyline {
                points: points.to_vec(),
                closed: false,
                branch_directions: vec![[1.0, 0.0], [0.0, 1.0]],
            },
        )
        .unwrap();
    let source = CurveSpan {
        curve: polyline,
        segment: 1,
    };
    let mut session = session(document);
    let request = SketchOperationRequest::AssociativeLineOffset {
        label: "polyline offset".into(),
        source,
        distance: 2.0,
        side: DocumentLineSide::Left,
        mode: AssociativeLineOffsetMode::SupportingLine,
    };
    let first = proposed(&session, request.clone());
    let second = proposed(&session, request);
    assert_eq!(first.expected_application(), second.expected_application());
    let ids = offset_ids(first.expected_application());
    assert_eq!(ids.source, source);
    first.apply(&mut session).unwrap();

    let updated = session
        .transact(session.design_identity(), |document| {
            document.set_scalar_value(ids.distance, 3.0)?;
            Ok(())
        })
        .unwrap();
    assert!(updated.published_accepted_identity().is_some());
    let accepted = session.accepted_state().unwrap().document();
    let target_start = accepted.point(ids.target_start).unwrap().position;
    let target_end = accepted.point(ids.target_end).unwrap().position;
    assert!((target_start[0] - 1.0).abs() <= 1.0e-8);
    assert!((target_end[0] - 1.0).abs() <= 1.0e-8);
    assert!((target_end[0] - target_start[0]).abs() <= 1.0e-8);
    assert!(target_end[1] > target_start[1]);
    assert_eq!(accepted.scalar(ids.distance).unwrap().value, 3.0);
}

#[test]
fn exact_offset_remains_associated_when_the_source_length_changes() {
    let mut document = SketchDocument::new(10.0).unwrap();
    let (source_curve, source_points) = line(&mut document, "source", [0.0, 0.0], [4.0, 0.0]);
    let source = CurveSpan::line(source_curve);
    document
        .add_constraint(
            "fixed source start",
            DocumentConstraintDefinition::FixedPoint {
                point: source_points[0],
                target: [0.0, 0.0],
            },
        )
        .unwrap();
    document
        .add_constraint(
            "horizontal source",
            DocumentConstraintDefinition::Horizontal { line: source },
        )
        .unwrap();
    let source_length = document
        .add_scalar(
            "source length",
            4.0,
            ScalarUnit::Length,
            ScalarDomain::Positive,
        )
        .unwrap();
    document
        .add_dimension(
            "source length dimension",
            DocumentDimensionDefinition::CurveLength {
                curve: source,
                target: source_length,
            },
            DocumentDimensionMode::Driving,
        )
        .unwrap();
    let mut session = session(document);
    let proposal = proposed(
        &session,
        SketchOperationRequest::AssociativeLineOffset {
            label: "associated exact offset".into(),
            source,
            distance: 2.0,
            side: DocumentLineSide::Left,
            mode: AssociativeLineOffsetMode::ExactTranslatedSegment,
        },
    );
    let ids = offset_ids(proposal.expected_application());
    proposal.apply(&mut session).unwrap();

    let changed = session
        .transact(session.design_identity(), |document| {
            document.set_scalar_value(source_length, 6.0)?;
            Ok(())
        })
        .unwrap();
    assert!(changed.published_accepted_identity().is_some());
    let accepted = session.accepted_state().unwrap().document();
    assert_point(
        accepted.point(source_points[1]).unwrap().position,
        [6.0, 0.0],
        1.0,
    );
    assert_point(
        accepted.point(ids.target_start).unwrap().position,
        [0.0, 2.0],
        1.0,
    );
    assert_point(
        accepted.point(ids.target_end).unwrap().position,
        [6.0, 2.0],
        1.0,
    );
}

#[test]
fn invalid_and_unsupported_offsets_never_mutate_the_live_session() {
    let mut document = SketchDocument::new(10.0).unwrap();
    let (source_curve, _) = line(&mut document, "source", [0.0, 0.0], [4.0, 0.0]);
    let center = document.add_point("circle center", [0.0, 4.0]).unwrap();
    let radius = document
        .add_scalar(
            "circle radius",
            1.0,
            ScalarUnit::Length,
            ScalarDomain::Positive,
        )
        .unwrap();
    let circle = document
        .add_curve("circle", CurveDefinition::Circle { center, radius })
        .unwrap();
    let session = session(document);
    let before_input = session.prepared_input();
    let before_document = session.design_document().clone();
    let before_accepted = session
        .accepted_state()
        .map(geosolve_sketch::SketchAcceptedDocumentState::identity);

    for distance in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        let result = SketchOperationSnapshot::capture(&session)
            .prepare(SketchOperationRequest::AssociativeLineOffset {
                label: "invalid offset".into(),
                source: CurveSpan::line(source_curve),
                distance,
                side: DocumentLineSide::Left,
                mode: AssociativeLineOffsetMode::ExactTranslatedSegment,
            })
            .execute(OperationControl::default());
        assert!(result.is_err());
    }

    let unsupported = SketchOperationSnapshot::capture(&session)
        .prepare(SketchOperationRequest::AssociativeLineOffset {
            label: "unsupported curve offset".into(),
            source: CurveSpan::line(circle),
            distance: 1.0,
            side: DocumentLineSide::Right,
            mode: AssociativeLineOffsetMode::SupportingLine,
        })
        .execute(OperationControl::default())
        .unwrap();
    let OperationOutcome::Completed { value, .. } = unsupported else {
        panic!("unsupported request must complete with typed evidence");
    };
    let SketchOperationResult::Unsupported(unsupported) = value else {
        panic!("circle offset must not be approximated");
    };
    assert_eq!(
        unsupported.reason,
        SketchOperationUnsupportedReason::CurveFamily {
            curve: circle,
            operation: "line offset",
        }
    );

    let oversized_label = "x".repeat(MAX_LABEL_BYTES + 1);
    assert!(
        SketchOperationSnapshot::capture(&session)
            .prepare(SketchOperationRequest::AssociativeLineOffset {
                label: oversized_label,
                source: CurveSpan::line(source_curve),
                distance: 1.0,
                side: DocumentLineSide::Left,
                mode: AssociativeLineOffsetMode::ExactTranslatedSegment,
            })
            .execute(OperationControl::default())
            .is_err()
    );

    assert_eq!(session.prepared_input(), before_input);
    assert_eq!(session.design_document(), &before_document);
    assert_eq!(
        session
            .accepted_state()
            .map(geosolve_sketch::SketchAcceptedDocumentState::identity),
        before_accepted
    );
}

#[test]
fn controlled_proposal_apply_is_mutation_free_when_cancelled_or_exhausted() {
    let mut document = SketchDocument::new(10.0).unwrap();
    let (source_curve, _) = line(&mut document, "source", [0.0, 0.0], [4.0, 0.0]);
    let mut session = session(document);
    let proposal = proposed(
        &session,
        SketchOperationRequest::AssociativeLineOffset {
            label: "controlled offset".into(),
            source: CurveSpan::line(source_curve),
            distance: 1.0,
            side: DocumentLineSide::Left,
            mode: AssociativeLineOffsetMode::ExactTranslatedSegment,
        },
    );
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

#[test]
fn geometry_operations_reject_accepted_geometry_from_a_different_current_input() {
    let mut document = SketchDocument::new(10.0).unwrap();
    let (source, source_points) = line(&mut document, "source", [-3.0, 0.0], [-1.0, 0.0]);
    let (axis, axis_points) = line(&mut document, "axis", [0.0, -2.0], [0.0, 2.0]);
    fix_points(
        &mut document,
        &[
            (source_points[0], [-3.0, 0.0]),
            (source_points[1], [-1.0, 0.0]),
            (axis_points[0], [0.0, -2.0]),
            (axis_points[1], [0.0, 2.0]),
        ],
    );
    let mut session = session(document);
    let accepted = session.accepted_state().unwrap().identity();
    let attempt = session
        .reattempt(
            session.design_identity(),
            DocumentSolveRequest::default().with_drag(
                DesignPointId(PersistentId::from_u128(u128::MAX)),
                [20.0, 20.0],
            ),
        )
        .unwrap();
    assert!(attempt.accepted_state_identity().is_none());
    assert_eq!(session.accepted_state().unwrap().identity(), accepted);
    assert_ne!(
        session.accepted_state().unwrap().input(),
        session.prepared_input().attempt_input()
    );

    let before = session.prepared_input();
    let outcome = SketchOperationSnapshot::capture(&session)
        .prepare(SketchOperationRequest::Mirror {
            label: "stale-input mirror".into(),
            source,
            axis: CurveSpan::line(axis),
        })
        .execute(OperationControl::default())
        .unwrap();
    let OperationOutcome::Completed { value, .. } = outcome else {
        panic!("typed incomplete operation must finish preparation");
    };
    let SketchOperationResult::Incomplete(incomplete) = value else {
        panic!("accepted geometry from another current input must be incomplete");
    };
    assert_eq!(
        incomplete.reason,
        geosolve_sketch_ops::SketchOperationIncompleteReason::AcceptedStateForDifferentInput
    );
    assert_eq!(session.prepared_input(), before);
}

#[test]
#[allow(clippy::too_many_lines)]
fn joined_offset_rejects_every_invalid_path_class_without_mutation() {
    let (base_session, base_sources, _) = mixed_corner_fixture();
    let base_input = base_session.prepared_input();
    let base_document = base_session.design_document().clone();
    assert_eq!(
        incomplete_reason(
            &base_session,
            joined_request(
                "too short",
                vec![base_sources[0]],
                1.0,
                DocumentLineSide::Left,
            ),
        ),
        SketchOperationIncompleteReason::JoinedLineOffsetSourceCount { count: 1 }
    );
    assert_eq!(
        incomplete_reason(
            &base_session,
            joined_request(
                "too long",
                vec![base_sources[0]; 33],
                1.0,
                DocumentLineSide::Left,
            ),
        ),
        SketchOperationIncompleteReason::JoinedLineOffsetSourceCount { count: 33 }
    );
    assert_eq!(
        incomplete_reason(
            &base_session,
            joined_request(
                "duplicate",
                vec![
                    base_sources[0],
                    LineOffsetChainSpan {
                        source: base_sources[0].source,
                        orientation: DocumentLineOffsetOrientation::Reversed,
                    },
                ],
                1.0,
                DocumentLineSide::Left,
            ),
        ),
        SketchOperationIncompleteReason::DuplicateJoinedLineOffsetSource {
            source: base_sources[0].source,
        }
    );
    for distance in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        assert!(
            SketchOperationSnapshot::capture(&base_session)
                .prepare(joined_request(
                    "non-finite",
                    base_sources.clone(),
                    distance,
                    DocumentLineSide::Left,
                ))
                .execute(OperationControl::default())
                .is_err()
        );
    }
    assert_eq!(base_session.prepared_input(), base_input);
    assert_eq!(base_session.design_document(), &base_document);

    let mut disconnected_document = SketchDocument::new(10.0).unwrap();
    let (first, _) = line(
        &mut disconnected_document,
        "disconnected first",
        [0.0, 0.0],
        [1.0, 0.0],
    );
    let (second, _) = line(
        &mut disconnected_document,
        "disconnected second",
        [2.0, 0.0],
        [3.0, 0.0],
    );
    let disconnected_session = session(disconnected_document);
    assert_eq!(
        incomplete_reason(
            &disconnected_session,
            joined_request(
                "disconnected",
                vec![
                    joined_source(first, DocumentLineOffsetOrientation::Same),
                    joined_source(second, DocumentLineOffsetOrientation::Same),
                ],
                1.0,
                DocumentLineSide::Left,
            ),
        ),
        SketchOperationIncompleteReason::DisconnectedJoinedLineOffsetPath {
            previous: CurveSpan::line(first),
            next: CurveSpan::line(second),
        }
    );

    let mut cycle_document = SketchDocument::new(10.0).unwrap();
    let cycle_points = [
        cycle_document.add_point("cycle a", [0.0, 0.0]).unwrap(),
        cycle_document.add_point("cycle b", [3.0, 0.0]).unwrap(),
        cycle_document.add_point("cycle c", [2.0, 2.0]).unwrap(),
    ];
    let cycle_lines = [
        line_between(
            &mut cycle_document,
            "cycle first",
            cycle_points[0],
            cycle_points[1],
        ),
        line_between(
            &mut cycle_document,
            "cycle second",
            cycle_points[1],
            cycle_points[2],
        ),
        line_between(
            &mut cycle_document,
            "cycle third",
            cycle_points[2],
            cycle_points[0],
        ),
    ];
    let cycle_session = session(cycle_document);
    assert_eq!(
        incomplete_reason(
            &cycle_session,
            joined_request(
                "cycle",
                cycle_lines
                    .into_iter()
                    .map(|curve| joined_source(curve, DocumentLineOffsetOrientation::Same))
                    .collect(),
                0.5,
                DocumentLineSide::Left,
            ),
        ),
        SketchOperationIncompleteReason::CyclicJoinedLineOffsetPath {
            point: cycle_points[0],
        }
    );

    let mut degenerate_document = SketchDocument::new(10.0).unwrap();
    let degenerate_start = degenerate_document
        .add_point("degenerate start", [0.0, 0.0])
        .unwrap();
    let degenerate_end = degenerate_document
        .add_point("degenerate end", [1.0e-15, 0.0])
        .unwrap();
    let following_point = degenerate_document
        .add_point("following", [2.0, 0.0])
        .unwrap();
    let degenerate = line_between(
        &mut degenerate_document,
        "degenerate span",
        degenerate_start,
        degenerate_end,
    );
    let following = line_between(
        &mut degenerate_document,
        "following span",
        degenerate_end,
        following_point,
    );
    let degenerate_session = session(degenerate_document);
    assert_eq!(
        incomplete_reason(
            &degenerate_session,
            joined_request(
                "degenerate",
                vec![
                    joined_source(degenerate, DocumentLineOffsetOrientation::Same),
                    joined_source(following, DocumentLineOffsetOrientation::Same),
                ],
                0.5,
                DocumentLineSide::Left,
            ),
        ),
        SketchOperationIncompleteReason::DegenerateLineSpan {
            support: CurveSpan::line(degenerate),
        }
    );

    let mut uturn_document = SketchDocument::new(10.0).unwrap();
    let uturn_points = [
        uturn_document.add_point("uturn a", [0.0, 0.0]).unwrap(),
        uturn_document.add_point("uturn b", [2.0, 0.0]).unwrap(),
        uturn_document
            .add_point("uturn distinct a", [0.0, 0.0])
            .unwrap(),
    ];
    let uturn_first = line_between(
        &mut uturn_document,
        "uturn first",
        uturn_points[0],
        uturn_points[1],
    );
    let uturn_second = line_between(
        &mut uturn_document,
        "uturn second",
        uturn_points[1],
        uturn_points[2],
    );
    let uturn_session = session(uturn_document);
    assert_eq!(
        incomplete_reason(
            &uturn_session,
            joined_request(
                "uturn",
                vec![
                    joined_source(uturn_first, DocumentLineOffsetOrientation::Same),
                    joined_source(uturn_second, DocumentLineOffsetOrientation::Same),
                ],
                0.5,
                DocumentLineSide::Left,
            ),
        ),
        SketchOperationIncompleteReason::JoinedLineOffsetUTurn {
            point: uturn_points[1],
        }
    );

    let mut remote_document = SketchDocument::new(100.0).unwrap();
    let remote_points = [
        remote_document.add_point("remote a", [0.0, 0.0]).unwrap(),
        remote_document.add_point("remote b", [10.0, 0.0]).unwrap(),
        remote_document
            .add_point(
                "remote c",
                [
                    10.0 + 10.0 * 175.0_f64.to_radians().cos(),
                    10.0 * 175.0_f64.to_radians().sin(),
                ],
            )
            .unwrap(),
    ];
    let remote_first = line_between(
        &mut remote_document,
        "remote first",
        remote_points[0],
        remote_points[1],
    );
    let remote_second = line_between(
        &mut remote_document,
        "remote second",
        remote_points[1],
        remote_points[2],
    );
    let remote_session = session(remote_document);
    assert_eq!(
        incomplete_reason(
            &remote_session,
            joined_request(
                "remote",
                vec![
                    joined_source(remote_first, DocumentLineOffsetOrientation::Same),
                    joined_source(remote_second, DocumentLineOffsetOrientation::Same),
                ],
                1.0,
                DocumentLineSide::Left,
            ),
        ),
        SketchOperationIncompleteReason::JoinedLineOffsetRemoteMiter {
            point: remote_points[1],
        }
    );

    let mut collapsed_document = SketchDocument::new(10.0).unwrap();
    let collapsed_points = [
        collapsed_document
            .add_point("collapsed a", [0.0, 0.0])
            .unwrap(),
        collapsed_document
            .add_point("collapsed b", [1.0, 0.0])
            .unwrap(),
        collapsed_document
            .add_point("collapsed c", [1.0, 1.0])
            .unwrap(),
    ];
    let collapsed_first = line_between(
        &mut collapsed_document,
        "collapsed first",
        collapsed_points[0],
        collapsed_points[1],
    );
    let collapsed_second = line_between(
        &mut collapsed_document,
        "collapsed second",
        collapsed_points[1],
        collapsed_points[2],
    );
    let collapsed_session = session(collapsed_document);
    assert_eq!(
        incomplete_reason(
            &collapsed_session,
            joined_request(
                "collapsed",
                vec![
                    joined_source(collapsed_first, DocumentLineOffsetOrientation::Same),
                    joined_source(collapsed_second, DocumentLineOffsetOrientation::Same),
                ],
                1.0,
                DocumentLineSide::Left,
            ),
        ),
        SketchOperationIncompleteReason::JoinedLineOffsetCollapsedTargetSpan { index: 0 }
    );
}

#[test]
fn joined_offset_charges_each_explicit_operand_and_preserves_stale_or_cancelled_state() {
    let (mut session, sources, _) = mixed_corner_fixture();
    let request = joined_request(
        "controlled joined",
        sources.clone(),
        1.0,
        DocumentLineSide::Left,
    );
    let before_input = session.prepared_input();
    let before_document = session.design_document().clone();

    let mut insufficient = OperationLimits::unlimited();
    insufficient.document_dependency_items = sources.len() - 1;
    let exhausted = SketchOperationSnapshot::capture(&session)
        .prepare(request.clone())
        .execute(OperationControl::new(
            geosolve_sketch::CancellationToken::default(),
            insufficient,
        ))
        .unwrap();
    assert!(matches!(exhausted, OperationOutcome::WorkExhausted { .. }));

    let mut exact = OperationLimits::unlimited();
    exact.document_dependency_items = sources.len();
    let completed = SketchOperationSnapshot::capture(&session)
        .prepare(request.clone())
        .execute(OperationControl::new(
            geosolve_sketch::CancellationToken::default(),
            exact,
        ))
        .unwrap();
    let OperationOutcome::Completed { value, report } = completed else {
        panic!("exact operand budget must complete");
    };
    assert_eq!(report.consumed.document_dependency_items, sources.len());
    let SketchOperationResult::Proposed(proposal) = value else {
        panic!("joined proposal expected");
    };

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

    let stale = proposed(&session, request);
    session
        .transact(session.design_identity(), |document| {
            document.add_point("newer winner", [20.0, 20.0])?;
            Ok(())
        })
        .unwrap();
    let winner_input = session.prepared_input();
    let winner_document = session.design_document().clone();
    assert!(matches!(
        stale.apply(&mut session),
        Err(SketchOperationApplyError::StaleInput { .. })
    ));
    assert_eq!(session.prepared_input(), winner_input);
    assert_eq!(session.design_document(), &winner_document);
}

fn add(first: [f64; 2], second: [f64; 2]) -> [f64; 2] {
    [first[0] + second[0], first[1] + second[1]]
}

fn subtract(first: [f64; 2], second: [f64; 2]) -> [f64; 2] {
    [first[0] - second[0], first[1] - second[1]]
}
