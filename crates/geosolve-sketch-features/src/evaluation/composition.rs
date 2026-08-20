// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::{BTreeMap, BTreeSet};

use geosolve_sketch::{DocumentFilletTrimEndpoint, GeometryRole, SketchDocument};

use crate::document::{ComputedFeatureId, ComputedFilletParent, NativeCurveSpanSource};

use super::{
    ComputedClaimEndpoint, ComputedConstructionFragmentId, ComputedCornerRef, ComputedEdgeId,
    ComputedEvaluationRevision, ComputedFeatureEvaluationError, ComputedFeatureFailure,
    ComputedSourceInterval, EvaluatedFeatureCandidate, SourceTopology, parameter_strictly_inside,
    parameter_tolerance, strict_finite_interval,
};

#[derive(Clone, Copy, Debug)]
pub(super) struct EndpointClaim {
    pub(super) owner: ComputedCornerRef,
    pub(super) source: NativeCurveSpanSource,
    pub(super) endpoint: DocumentFilletTrimEndpoint,
    pub(super) parameter: f64,
    pub(super) base_interval: ComputedSourceInterval,
    pub(super) participates_in_trimming: bool,
}

#[derive(Clone, Debug)]
pub(super) struct SourceComposition {
    pub(super) source: NativeCurveSpanSource,
    pub(super) base_interval: ComputedSourceInterval,
    pub(super) start: Option<EndpointClaim>,
    pub(super) end: Option<EndpointClaim>,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct DiscardedSourceComplement {
    pub(super) interval: ComputedSourceInterval,
    pub(super) claim: EndpointClaim,
}

#[derive(Clone, Debug)]
pub(super) struct ComposedSourceOutput {
    pub(super) effective_interval: ComputedSourceInterval,
    pub(super) discarded: Vec<DiscardedSourceComplement>,
}

pub(super) fn endpoint_claim(
    owner: ComputedCornerRef,
    parent: ComputedFilletParent,
    parameter: f64,
    topology: SourceTopology,
) -> EndpointClaim {
    EndpointClaim {
        owner,
        source: parent.source,
        endpoint: parent.retained_endpoint,
        parameter,
        base_interval: topology.base_interval,
        participates_in_trimming: topology.participates_in_trimming(),
    }
}

pub(super) fn combined_source_role(
    sketch: &SketchDocument,
    parents: [ComputedFilletParent; 2],
) -> GeometryRole {
    if parents.into_iter().any(|parent| {
        sketch.geometry_role(parent.source.span.curve) == Some(GeometryRole::Construction)
    }) {
        GeometryRole::Construction
    } else {
        GeometryRole::Profile
    }
}

pub(super) fn composition_failures(
    candidates: &[EvaluatedFeatureCandidate],
) -> BTreeMap<ComputedFeatureId, ComputedFeatureFailure> {
    let mut claims = BTreeMap::<NativeCurveSpanSource, Vec<EndpointClaim>>::new();
    for corner in candidates
        .iter()
        .flat_map(|candidate| candidate.corners.iter())
    {
        for claim in corner.claims {
            if claim.participates_in_trimming {
                claims.entry(claim.source).or_default().push(claim);
            }
        }
    }
    let mut failures = BTreeMap::new();
    for (source, mut source_claims) in claims {
        source_claims.sort_by_key(|claim| claim.owner);
        source_claims.retain(|claim| {
            let valid = strict_finite_interval(claim.base_interval)
                && parameter_strictly_inside(claim.parameter, claim.base_interval);
            if !valid {
                failures.entry(claim.owner.feature).or_insert(
                    ComputedFeatureFailure::InvalidParentState {
                        corner: claim.owner.corner,
                    },
                );
            }
            valid
        });
        if source_claims.is_empty() {
            continue;
        }
        let starts = source_claims
            .iter()
            .copied()
            .filter(|claim| claim.endpoint == DocumentFilletTrimEndpoint::Start)
            .collect::<Vec<_>>();
        let ends = source_claims
            .iter()
            .copied()
            .filter(|claim| claim.endpoint == DocumentFilletTrimEndpoint::End)
            .collect::<Vec<_>>();
        if starts.len() > 1 {
            insert_conflict_failures(
                &mut failures,
                &source_claims,
                &ComputedFeatureFailure::EndpointClaimConflict {
                    span_source: source,
                    endpoint: ComputedClaimEndpoint::Start,
                    participants: starts.iter().map(|claim| claim.owner).collect(),
                },
            );
        }
        if ends.len() > 1 {
            insert_conflict_failures(
                &mut failures,
                &source_claims,
                &ComputedFeatureFailure::EndpointClaimConflict {
                    span_source: source,
                    endpoint: ComputedClaimEndpoint::End,
                    participants: ends.iter().map(|claim| claim.owner).collect(),
                },
            );
        }
        if let ([start], [end]) = (starts.as_slice(), ends.as_slice()) {
            let tolerance = parameter_tolerance(ComputedSourceInterval {
                start: start.base_interval.start.min(end.base_interval.start),
                end: start.base_interval.end.max(end.base_interval.end),
            });
            if start.parameter + tolerance >= end.parameter {
                let participants = vec![start.owner, end.owner];
                insert_conflict_failures(
                    &mut failures,
                    &source_claims,
                    &ComputedFeatureFailure::ConsumedSourceInterval {
                        span_source: source,
                        participants,
                    },
                );
            }
        }
        let base_mismatch = source_claims.iter().skip(1).any(|claim| {
            claim.base_interval.start.to_bits() != source_claims[0].base_interval.start.to_bits()
                || claim.base_interval.end.to_bits() != source_claims[0].base_interval.end.to_bits()
        });
        if base_mismatch {
            insert_conflict_failures(
                &mut failures,
                &source_claims,
                &ComputedFeatureFailure::EndpointClaimConflict {
                    span_source: source,
                    endpoint: ComputedClaimEndpoint::Both,
                    participants: source_claims.iter().map(|claim| claim.owner).collect(),
                },
            );
        }
    }
    failures
}

fn insert_conflict_failures(
    failures: &mut BTreeMap<ComputedFeatureId, ComputedFeatureFailure>,
    source_claims: &[EndpointClaim],
    failure: &ComputedFeatureFailure,
) {
    let (ComputedFeatureFailure::EndpointClaimConflict { participants, .. }
    | ComputedFeatureFailure::ConsumedSourceInterval { participants, .. }) = failure
    else {
        return;
    };
    for feature in participants.iter().map(|owner| owner.feature) {
        let mut attributed = (*failure).clone();
        if let ComputedFeatureFailure::EndpointClaimConflict {
            participants: owners,
            ..
        }
        | ComputedFeatureFailure::ConsumedSourceInterval {
            participants: owners,
            ..
        } = &mut attributed
        {
            owners.sort();
            owners.dedup();
        }
        failures.entry(feature).or_insert(attributed);
    }
    // A whole set fails atomically. If another claim from that same set is in
    // this source group, retain deterministic attribution to the same conflict.
    let failed_set_ids = participants
        .iter()
        .map(|owner| owner.feature)
        .collect::<BTreeSet<_>>();
    for feature in source_claims
        .iter()
        .map(|claim| claim.owner.feature)
        .filter(|feature| failed_set_ids.contains(feature))
    {
        failures
            .entry(feature)
            .or_insert_with(|| (*failure).clone());
    }
}

pub(super) fn compose_sources(
    candidates: &[EvaluatedFeatureCandidate],
) -> BTreeMap<NativeCurveSpanSource, SourceComposition> {
    let mut compositions = BTreeMap::new();
    for claim in candidates
        .iter()
        .flat_map(|candidate| candidate.corners.iter())
        .flat_map(|corner| corner.claims)
        .filter(|claim| claim.participates_in_trimming)
    {
        let composition = compositions
            .entry(claim.source)
            .or_insert(SourceComposition {
                source: claim.source,
                base_interval: claim.base_interval,
                start: None,
                end: None,
            });
        match claim.endpoint {
            DocumentFilletTrimEndpoint::Start => composition.start = Some(claim),
            DocumentFilletTrimEndpoint::End => composition.end = Some(claim),
        }
    }
    compositions
}

pub(super) fn compose_source_output(
    composition: &SourceComposition,
) -> Result<ComposedSourceOutput, ComputedFeatureEvaluationError> {
    let effective_interval = ComputedSourceInterval {
        start: composition
            .start
            .map_or(composition.base_interval.start, |claim| claim.parameter),
        end: composition
            .end
            .map_or(composition.base_interval.end, |claim| claim.parameter),
    };
    let mut discarded = Vec::with_capacity(2);
    if let Some(claim) = composition.start {
        let interval = ComputedSourceInterval {
            start: composition.base_interval.start,
            end: claim.parameter,
        };
        if material_interval(interval, composition.base_interval) {
            discarded.push(DiscardedSourceComplement { interval, claim });
        }
    }
    if let Some(claim) = composition.end {
        let interval = ComputedSourceInterval {
            start: claim.parameter,
            end: composition.base_interval.end,
        };
        if material_interval(interval, composition.base_interval) {
            discarded.push(DiscardedSourceComplement { interval, claim });
        }
    }
    let output = ComposedSourceOutput {
        effective_interval,
        discarded,
    };
    validate_composed_source_output(composition, &output)?;
    Ok(output)
}

fn validate_composed_source_output(
    composition: &SourceComposition,
    output: &ComposedSourceOutput,
) -> Result<(), ComputedFeatureEvaluationError> {
    let invalid = || ComputedFeatureEvaluationError::InvalidGeneratedTopology {
        resource: "source composition",
    };
    if !strict_finite_interval(composition.base_interval)
        || !strict_finite_interval(output.effective_interval)
    {
        return Err(invalid());
    }
    let start_discarded = composition.start.map(|claim| ComputedSourceInterval {
        start: composition.base_interval.start,
        end: claim.parameter,
    });
    let end_discarded = composition.end.map(|claim| ComputedSourceInterval {
        start: claim.parameter,
        end: composition.base_interval.end,
    });
    let expected_count = usize::from(
        start_discarded
            .is_some_and(|interval| material_interval(interval, composition.base_interval)),
    ) + usize::from(
        end_discarded
            .is_some_and(|interval| material_interval(interval, composition.base_interval)),
    );
    if output.discarded.len() != expected_count {
        return Err(invalid());
    }

    let mut discarded_index = 0;
    if let Some(claim) = composition.start {
        if claim.source != composition.source
            || claim.endpoint != DocumentFilletTrimEndpoint::Start
            || !same_interval(claim.base_interval, composition.base_interval)
            || !same_parameter(output.effective_interval.start, claim.parameter)
        {
            return Err(invalid());
        }
        if let Some(interval) = start_discarded
            && material_interval(interval, composition.base_interval)
        {
            let complement = output.discarded.get(discarded_index).ok_or_else(invalid)?;
            discarded_index += 1;
            if !same_parameter(complement.interval.start, interval.start)
                || !same_parameter(complement.interval.end, interval.end)
                || !same_claim(complement.claim, claim)
            {
                return Err(invalid());
            }
        }
    } else if !same_parameter(
        output.effective_interval.start,
        composition.base_interval.start,
    ) {
        return Err(invalid());
    }

    if let Some(claim) = composition.end {
        if claim.source != composition.source
            || claim.endpoint != DocumentFilletTrimEndpoint::End
            || !same_interval(claim.base_interval, composition.base_interval)
            || !same_parameter(output.effective_interval.end, claim.parameter)
        {
            return Err(invalid());
        }
        if let Some(interval) = end_discarded
            && material_interval(interval, composition.base_interval)
        {
            let complement = output.discarded.get(discarded_index).ok_or_else(invalid)?;
            discarded_index += 1;
            if !same_parameter(complement.interval.start, interval.start)
                || !same_parameter(complement.interval.end, interval.end)
                || !same_claim(complement.claim, claim)
            {
                return Err(invalid());
            }
        }
    } else if !same_parameter(output.effective_interval.end, composition.base_interval.end) {
        return Err(invalid());
    }
    if discarded_index != output.discarded.len() {
        return Err(invalid());
    }

    // The theoretical complements share exact boundaries with the retained
    // interval. Every material complement that is actually published must
    // remain strictly disjoint from that interval and from its sibling.
    if output.discarded.iter().any(|complement| {
        complement.interval.start < output.effective_interval.end
            && output.effective_interval.start < complement.interval.end
    }) {
        return Err(invalid());
    }
    Ok(())
}

fn same_claim(first: EndpointClaim, second: EndpointClaim) -> bool {
    first.owner == second.owner
        && first.source == second.source
        && first.endpoint == second.endpoint
        && same_parameter(first.parameter, second.parameter)
        && same_interval(first.base_interval, second.base_interval)
        && first.participates_in_trimming == second.participates_in_trimming
}

fn same_interval(first: ComputedSourceInterval, second: ComputedSourceInterval) -> bool {
    same_parameter(first.start, second.start) && same_parameter(first.end, second.end)
}

fn same_parameter(first: f64, second: f64) -> bool {
    first.to_bits() == second.to_bits()
}

fn material_interval(
    interval: ComputedSourceInterval,
    base_interval: ComputedSourceInterval,
) -> bool {
    strict_finite_interval(interval)
        && interval.end - interval.start > parameter_tolerance(base_interval)
}

pub(super) fn edge_id(
    evaluation: ComputedEvaluationRevision,
    index: usize,
) -> Result<ComputedEdgeId, ComputedFeatureEvaluationError> {
    Ok(ComputedEdgeId {
        evaluation,
        ordinal: u32::try_from(index).map_err(|_| {
            ComputedFeatureEvaluationError::PolicyLimitExceeded {
                resource: "edge ordinals",
                actual: index,
                limit: u32::MAX as usize,
            }
        })?,
    })
}

pub(super) fn construction_fragment_id(
    evaluation: ComputedEvaluationRevision,
    index: usize,
) -> Result<ComputedConstructionFragmentId, ComputedFeatureEvaluationError> {
    Ok(ComputedConstructionFragmentId {
        evaluation,
        ordinal: u32::try_from(index).map_err(|_| {
            ComputedFeatureEvaluationError::PolicyLimitExceeded {
                resource: "construction fragment ordinals",
                actual: index,
                limit: u32::MAX as usize,
            }
        })?,
    })
}
