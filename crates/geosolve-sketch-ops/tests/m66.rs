// SPDX-License-Identifier: GPL-3.0-or-later

#![allow(clippy::float_cmp)]

use geosolve_sketch::{
    CurveDefinition, CurveSpan, DesignPointId, DesignScalarId, DocumentConstraintDefinition,
    DocumentDimensionDefinition, DocumentDimensionId, DocumentDimensionMode,
    DocumentLineOffsetOrientation, DocumentLineSide, DocumentSolveRequest, MAX_LABEL_BYTES,
    OperationControl, OperationOutcome, RetainedSketchDocumentSession, ScalarDomain, ScalarUnit,
    SketchDocument, SolverConfig,
};
use geosolve_sketch_ops::{
    AssociativeLineOffsetMode, SketchOperationApplication, SketchOperationIdentityChange,
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

fn add(first: [f64; 2], second: [f64; 2]) -> [f64; 2] {
    [first[0] + second[0], first[1] + second[1]]
}

fn subtract(first: [f64; 2], second: [f64; 2]) -> [f64; 2] {
    [first[0] - second[0], first[1] - second[1]]
}
