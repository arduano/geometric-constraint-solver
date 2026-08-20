// SPDX-License-Identifier: GPL-3.0-or-later

use super::{
    BTreeSet, CurveSpan, DocumentConstraintDefinition, DocumentCurveContinuity,
    DocumentDirectedProfileOffsetCurve, DocumentOffsetTraversal,
    DocumentProfileOffsetCreationJunction, DocumentProfileOffsetCreationOperand,
    DocumentProfileOffsetCreationPath, DocumentProfileOffsetJunctionBranch,
    DocumentProfileOffsetJunctionOwner, DocumentProfileOffsetTurn, MAX_PROFILE_OFFSET_SPANS,
    OffsetDirectedSpan, OffsetEndpointRef, OffsetEndpointRole, OffsetJoinOwner, OffsetOperandIndex,
    OffsetOperandIneligibility, OffsetOperandEligibility, OffsetTraversal,
    PROFILE_OFFSET_TANGENT_CROSS_TOLERANCE, SketchDocument, SketchOperationIncompleteReason,
    SketchOperationUnsupportedReason, SketchProfileOffsetOperand, cross,
};

pub(super) enum ProfileOffsetPlanFailure {
    Unsupported(SketchOperationUnsupportedReason),
    Incomplete(SketchOperationIncompleteReason),
}

pub(super) fn plan_profile_offset_operand(
    document: &SketchDocument,
    index: &OffsetOperandIndex,
    operand: &SketchProfileOffsetOperand,
) -> Result<(DocumentProfileOffsetCreationOperand, Vec<CurveSpan>), ProfileOffsetPlanFailure> {
    match operand {
        SketchProfileOffsetOperand::Face { key, direction } => {
            let candidate = index.face(key).ok_or_else(|| {
                ProfileOffsetPlanFailure::Incomplete(
                    SketchOperationIncompleteReason::ProfileOffsetFaceMissing { key: key.clone() },
                )
            })?;
            if let Some(reasons) = disabled_offset_reasons(&candidate.eligibility) {
                return Err(ProfileOffsetPlanFailure::Unsupported(
                    SketchOperationUnsupportedReason::ProfileOffsetFace {
                        key: key.clone(),
                        reasons,
                    },
                ));
            }
            let span_count = key.outer.spans.len()
                + key.holes.iter().map(|hole| hole.spans.len()).sum::<usize>();
            if span_count == 0 || span_count > MAX_PROFILE_OFFSET_SPANS {
                return Err(ProfileOffsetPlanFailure::Incomplete(
                    SketchOperationIncompleteReason::ProfileOffsetSpanLimitExceeded,
                ));
            }
            let mut seen = BTreeSet::new();
            let outer =
                plan_profile_offset_path(document, index, &key.outer.spans, true, &mut seen)?;
            let holes = key
                .holes
                .iter()
                .map(|hole| plan_profile_offset_path(document, index, &hole.spans, true, &mut seen))
                .collect::<Result<Vec<_>, _>>()?;
            let sources = seen.into_iter().collect();
            Ok((
                DocumentProfileOffsetCreationOperand::Face {
                    direction: *direction,
                    outer,
                    holes,
                },
                sources,
            ))
        }
        SketchProfileOffsetOperand::OpenChain { spans, side } => {
            if spans.is_empty() {
                return Err(ProfileOffsetPlanFailure::Incomplete(
                    SketchOperationIncompleteReason::ProfileOffsetEmptyChain,
                ));
            }
            if spans.len() > MAX_PROFILE_OFFSET_SPANS {
                return Err(ProfileOffsetPlanFailure::Incomplete(
                    SketchOperationIncompleteReason::ProfileOffsetSpanLimitExceeded,
                ));
            }
            for directed in spans {
                let candidate = index.span(directed.span).ok_or({
                    ProfileOffsetPlanFailure::Incomplete(
                        SketchOperationIncompleteReason::ProfileOffsetSpanMissing {
                            span: directed.span,
                        },
                    )
                })?;
                if let Some(reasons) = disabled_offset_reasons(&candidate.eligibility) {
                    return Err(ProfileOffsetPlanFailure::Unsupported(
                        SketchOperationUnsupportedReason::ProfileOffsetSpan {
                            span: directed.span,
                            reasons,
                        },
                    ));
                }
                if candidate.periodic {
                    return Err(ProfileOffsetPlanFailure::Unsupported(
                        SketchOperationUnsupportedReason::ProfileOffsetPeriodicChain {
                            span: directed.span,
                        },
                    ));
                }
            }
            let mut seen = BTreeSet::new();
            let chain = plan_profile_offset_path(document, index, spans, false, &mut seen)?;
            if index
                .adjacent_endpoints(directed_offset_endpoint(spans[0], true))
                .any(|endpoint| endpoint == directed_offset_endpoint(spans[spans.len() - 1], false))
            {
                return Err(ProfileOffsetPlanFailure::Incomplete(
                    SketchOperationIncompleteReason::ProfileOffsetClosedChain,
                ));
            }
            let sources = seen.into_iter().collect();
            Ok((
                DocumentProfileOffsetCreationOperand::OpenChain { side: *side, chain },
                sources,
            ))
        }
    }
}

fn disabled_offset_reasons(
    eligibility: &OffsetOperandEligibility,
) -> Option<Vec<OffsetOperandIneligibility>> {
    match eligibility {
        OffsetOperandEligibility::Eligible => None,
        OffsetOperandEligibility::Disabled { reasons } => Some(reasons.clone()),
    }
}

fn plan_profile_offset_path(
    document: &SketchDocument,
    index: &OffsetOperandIndex,
    spans: &[OffsetDirectedSpan],
    closed: bool,
    seen: &mut BTreeSet<CurveSpan>,
) -> Result<DocumentProfileOffsetCreationPath, ProfileOffsetPlanFailure> {
    let mut edges = Vec::with_capacity(spans.len());
    for directed in spans {
        if !seen.insert(directed.span) {
            return Err(ProfileOffsetPlanFailure::Incomplete(
                SketchOperationIncompleteReason::ProfileOffsetDuplicateSpan {
                    span: directed.span,
                },
            ));
        }
        edges.push(DocumentDirectedProfileOffsetCurve {
            curve: directed.span,
            traversal: document_offset_traversal(directed.traversal),
        });
    }
    if !closed {
        let selected_spans = spans
            .iter()
            .map(|directed| directed.span)
            .collect::<BTreeSet<_>>();
        if let Some(endpoint) = selected_branch_endpoint(index, &selected_spans) {
            return Err(ProfileOffsetPlanFailure::Incomplete(
                SketchOperationIncompleteReason::ProfileOffsetBranchedJoin { endpoint },
            ));
        }
    }
    let junction_count = if closed && !is_periodic_offset_path(index, spans) {
        spans.len()
    } else {
        spans.len().saturating_sub(1)
    };
    let mut junctions = Vec::with_capacity(junction_count);
    for current in 0..junction_count {
        let next = (current + 1) % spans.len();
        let incoming_endpoint = directed_offset_endpoint(spans[current], false);
        let outgoing_endpoint = directed_offset_endpoint(spans[next], true);
        let adjacency = find_offset_adjacency(index, incoming_endpoint, outgoing_endpoint)
            .ok_or_else(|| {
                ProfileOffsetPlanFailure::Incomplete(
                    SketchOperationIncompleteReason::ProfileOffsetDisconnectedJoin {
                        incoming: spans[current].span,
                        outgoing: spans[next].span,
                    },
                )
            })?;
        let owner = adjacency
            .owners
            .first()
            .copied()
            .map(document_profile_offset_owner)
            .ok_or_else(|| {
                ProfileOffsetPlanFailure::Incomplete(
                    SketchOperationIncompleteReason::ProfileOffsetDisconnectedJoin {
                        incoming: spans[current].span,
                        outgoing: spans[next].span,
                    },
                )
            })?;
        let branch = profile_offset_junction_branch(
            document,
            spans[current],
            spans[next],
            adjacency
                .owners
                .iter()
                .copied()
                .any(|owner| offset_join_is_explicitly_tangent(document, owner)),
        )
        .ok_or_else(|| {
            ProfileOffsetPlanFailure::Incomplete(
                SketchOperationIncompleteReason::ProfileOffsetDegenerateJunction {
                    incoming: spans[current].span,
                    outgoing: spans[next].span,
                },
            )
        })?;
        junctions.push(DocumentProfileOffsetCreationJunction {
            source_owner: owner,
            branch,
        });
    }
    Ok(DocumentProfileOffsetCreationPath { edges, junctions })
}

fn is_periodic_offset_path(index: &OffsetOperandIndex, spans: &[OffsetDirectedSpan]) -> bool {
    spans.len() == 1
        && index
            .span(spans[0].span)
            .is_some_and(|candidate| candidate.periodic)
}

fn find_offset_adjacency(
    index: &OffsetOperandIndex,
    first: OffsetEndpointRef,
    second: OffsetEndpointRef,
) -> Option<&geosolve_sketch_topology::OffsetEndpointAdjacency> {
    let endpoints = if first < second {
        [first, second]
    } else {
        [second, first]
    };
    index
        .adjacencies()
        .iter()
        .find(|adjacency| adjacency.endpoints == endpoints)
}

/// Finds a branch inside the proposed operand; incident unselected geometry does not contribute.
fn selected_branch_endpoint(
    index: &OffsetOperandIndex,
    selected_spans: &BTreeSet<CurveSpan>,
) -> Option<OffsetEndpointRef> {
    selected_spans.iter().find_map(|span| {
        index.span(*span)?.endpoints.iter().find_map(|candidate| {
            (index
                .adjacent_endpoints(candidate.endpoint)
                .filter(|adjacent| selected_spans.contains(&adjacent.span))
                .take(2)
                .count()
                > 1)
            .then_some(candidate.endpoint)
        })
    })
}

const fn document_profile_offset_owner(
    owner: OffsetJoinOwner,
) -> DocumentProfileOffsetJunctionOwner {
    match owner {
        OffsetJoinOwner::SharedPoint(point) => {
            DocumentProfileOffsetJunctionOwner::SharedPoint(point)
        }
        OffsetJoinOwner::Constraint(constraint) => {
            DocumentProfileOffsetJunctionOwner::Constraint(constraint)
        }
    }
}

fn offset_join_is_explicitly_tangent(document: &SketchDocument, owner: OffsetJoinOwner) -> bool {
    let OffsetJoinOwner::Constraint(owner) = owner else {
        return false;
    };
    document.constraint(owner).is_some_and(|constraint| {
        matches!(
            &constraint.definition,
            DocumentConstraintDefinition::LineCircleTangency { .. }
                | DocumentConstraintDefinition::CircleArcTangency { .. }
                | DocumentConstraintDefinition::LineCurveTangency { .. }
                | DocumentConstraintDefinition::CurveCurveTangency { .. }
                | DocumentConstraintDefinition::EndpointContinuity {
                    continuity: DocumentCurveContinuity::G1
                        | DocumentCurveContinuity::G2
                        | DocumentCurveContinuity::ParametricC2 { .. },
                    ..
                }
        )
    })
}

fn profile_offset_junction_branch(
    document: &SketchDocument,
    incoming: OffsetDirectedSpan,
    outgoing: OffsetDirectedSpan,
    explicitly_tangent: bool,
) -> Option<DocumentProfileOffsetJunctionBranch> {
    let incoming_tangent = directed_offset_tangent(document, incoming, false)?;
    let outgoing_tangent = directed_offset_tangent(document, outgoing, true)?;
    let cross_value = cross(incoming_tangent, outgoing_tangent);
    let alignment = incoming_tangent[0].mul_add(
        outgoing_tangent[0],
        incoming_tangent[1] * outgoing_tangent[1],
    );
    if explicitly_tangent || cross_value.abs() <= PROFILE_OFFSET_TANGENT_CROSS_TOLERANCE {
        (alignment > 0.0).then_some(DocumentProfileOffsetJunctionBranch::Tangent)
    } else {
        Some(DocumentProfileOffsetJunctionBranch::Miter {
            turn: if cross_value.is_sign_positive() {
                DocumentProfileOffsetTurn::Left
            } else {
                DocumentProfileOffsetTurn::Right
            },
        })
    }
}

fn directed_offset_tangent(
    document: &SketchDocument,
    directed: OffsetDirectedSpan,
    at_start: bool,
) -> Option<[f64; 2]> {
    let parameter = match (directed.traversal, at_start) {
        (OffsetTraversal::Forward, true) | (OffsetTraversal::Reverse, false) => 0.0,
        (OffsetTraversal::Forward, false) | (OffsetTraversal::Reverse, true) => 1.0,
    };
    let differential = document
        .evaluate_curve_jet(directed.span, parameter)
        .ok()?
        .differential()
        .ok()?;
    let sign = match directed.traversal {
        OffsetTraversal::Forward => 1.0,
        OffsetTraversal::Reverse => -1.0,
    };
    Some([
        differential.unit_tangent.x * sign,
        differential.unit_tangent.y * sign,
    ])
}

const fn document_offset_traversal(traversal: OffsetTraversal) -> DocumentOffsetTraversal {
    match traversal {
        OffsetTraversal::Forward => DocumentOffsetTraversal::Forward,
        OffsetTraversal::Reverse => DocumentOffsetTraversal::Reverse,
    }
}

const fn directed_offset_endpoint(
    directed: OffsetDirectedSpan,
    at_start: bool,
) -> OffsetEndpointRef {
    let endpoint = match (directed.traversal, at_start) {
        (OffsetTraversal::Forward, true) | (OffsetTraversal::Reverse, false) => {
            OffsetEndpointRole::Start
        }
        (OffsetTraversal::Forward, false) | (OffsetTraversal::Reverse, true) => {
            OffsetEndpointRole::End
        }
    };
    OffsetEndpointRef {
        span: directed.span,
        endpoint,
    }
}
