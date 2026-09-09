// SPDX-License-Identifier: GPL-3.0-or-later

//! Exact computed geometry is projected into an ephemeral, independently accepted native
//! document solely for the existing production topology query. It owns its own input stamp;
//! it is neither an edit to the design nor a claim of original native-source authority.

use std::collections::{BTreeMap, BTreeSet};

use geosolve_constraint_editor::ColdIntentMaterialization;
use geosolve_sketch::{
    ContactNeighborhood, CurveDefinition, CurveId, CurveSpan, DocumentConstraintDefinition,
    DocumentSolveRequest, GeometryRole, OperationControl, OperationOutcome,
    RetainedSketchDocumentSession, ScalarDomain, ScalarUnit, SketchDocument, SolverConfig,
};
use geosolve_sketch_features::{
    ComputedEdgeGeometry, ComputedEdgeProvenance, ComputedFeatureDefinition,
    ComputedFeatureEvaluationState,
};
use geosolve_sketch_topology::{
    EndpointTopologyRequest, OffsetEndpointRole, PreparedEndpointTopologyQuery,
    SampledTopologyRegion, TopologyProductionProfile, TopologyRequest,
    TopologySelfIntersectionPolicy, TopologySnapshot, TopologySourceProvenance,
};

use crate::{EngineError, EngineOutputGeometry};

const MAX_PROFILE_SOURCES: usize = 4096;

fn error(value: impl std::fmt::Display) -> EngineError {
    EngineError::Export(value.to_string())
}

pub(crate) fn export(
    accepted: &ColdIntentMaterialization,
    max_chord_error: f64,
    selection: Option<&EngineOutputGeometry>,
) -> Result<Vec<SampledTopologyRegion>, EngineError> {
    if !max_chord_error.is_finite() || max_chord_error <= 0.0 {
        return Err(error("max_chord_error must be finite and positive"));
    }
    validate_authority(accepted)?;
    let (projected, computed_curves, native_curves, projection_roundoff) =
        if accepted.computed.edges().is_empty() {
            (None, BTreeMap::new(), BTreeMap::new(), 0.0)
        } else {
            let (session, mapping, native, roundoff) = projection(accepted, max_chord_error * 0.5)?;
            (Some(session), mapping, native, roundoff)
        };
    let session = projected.as_ref().unwrap_or(&accepted.session);
    let profile = production_profile(session)?;
    // Charge the bounded analytic reparameterization error to the same requested
    // chord budget without needlessly multiplying polygon size for exact primitives.
    let sampling_error = max_chord_error - projection_roundoff;
    let regions = profile
        .sample_polygons(session, sampling_error)
        .map_err(error)?;
    let Some(selection) = selection else {
        return Ok(regions);
    };
    let spans = selection.spans.iter().copied().collect::<BTreeSet<_>>();
    let computed = selection
        .computed_edges
        .iter()
        .filter_map(|id| computed_curves.get(id).copied())
        .collect::<BTreeSet<_>>();
    let selected = profile
        .regions()
        .iter()
        .zip(regions)
        .filter_map(|(region, polygon)| {
            let outer = profile
                .wires()
                .iter()
                .find(|wire| wire.id == region.outer)?;
            outer
                .fragments
                .iter()
                .any(|fragment| match fragment.source {
                    TopologySourceProvenance::Native { support, .. } => {
                        spans.contains(
                            &native_curves
                                .get(&support.curve)
                                .copied()
                                .unwrap_or(support),
                        ) || computed.contains(&support.curve)
                    }
                    TopologySourceProvenance::ExternalLine { .. } => false,
                })
                .then_some(polygon)
        })
        .collect::<Vec<_>>();
    if selected.is_empty() {
        return Err(error(
            "selected output has no complete bounded profile region",
        ));
    }
    Ok(selected)
}

fn validate_authority(accepted: &ColdIntentMaterialization) -> Result<(), EngineError> {
    let native = accepted
        .session
        .accepted_state_for_current_input()
        .ok_or_else(|| error("current accepted native geometry required"))?;
    let input = accepted.computed.input();
    if !accepted.validation.hard_residuals_validated
        || !accepted.validation.all_active_features_current
        || input.sketch != accepted.session.prepared_input()
        || input.accepted != native.identity()
        || input.features != accepted.features.identity()
    {
        return Err(error(
            "native and computed geometry do not share current accepted authority",
        ));
    }
    for feature in accepted.features.features() {
        let evaluation = accepted
            .computed
            .feature_evaluations()
            .iter()
            .find(|evaluation| evaluation.feature == feature.id)
            .ok_or_else(|| error("computed feature evaluation is missing"))?;
        if !matches!(
            (&evaluation.state, feature.suppressed),
            (ComputedFeatureEvaluationState::Current { .. }, false)
                | (ComputedFeatureEvaluationState::Suppressed, true)
        ) {
            return Err(error("all active computed features must be Current"));
        }
    }
    Ok(())
}

fn production_profile(
    session: &RetainedSketchDocumentSession,
) -> Result<TopologyProductionProfile, EngineError> {
    let mut request = TopologyRequest::default();
    request.policy.self_intersections = TopologySelfIntersectionPolicy::Reject;
    let result = TopologySnapshot::capture(session)
        .map_err(error)?
        .prepare(request)
        .execute(OperationControl::default())
        .map_err(error)?;
    let OperationOutcome::Completed { value, .. } = result else {
        return Err(error("production query did not complete"));
    };
    value.production_profile.ok_or_else(|| {
        error(format!(
            "incomplete production topology: {:?}: {:?}",
            value.completeness, value.issues
        ))
    })
}

#[derive(Clone, Copy)]
struct SourceFragment {
    original: CurveSpan,
    interval: [f64; 2],
    projected: CurveId,
}

type Projection = (
    RetainedSketchDocumentSession,
    BTreeMap<u32, CurveId>,
    BTreeMap<CurveId, CurveSpan>,
    f64,
);

#[allow(
    clippy::too_many_lines,
    reason = "one projection carries accepted fragments, explicit joins and unchanged-geometry validation together"
)]
fn projection(
    accepted: &ColdIntentMaterialization,
    projection_error: f64,
) -> Result<Projection, EngineError> {
    let state = accepted
        .session
        .accepted_state_for_current_input()
        .ok_or_else(|| error("current accepted native geometry required"))?;
    let native = state.document();
    let mut document = SketchDocument::new(native.model_scale()).map_err(error)?;
    let mut sources = Vec::new();
    let mut roundoff: f64 = 0.0;
    let mut mapping = BTreeMap::new();
    for curve in native.curves() {
        if !state.effective_activity().is_active(curve.id)
            || native.geometry_role(curve.id) != Some(GeometryRole::Profile)
        {
            continue;
        }
        for interval in native.visible_curve_intervals(curve.id).map_err(error)? {
            if accepted
                .computed
                .replaced_sources()
                .iter()
                .any(|source| source.span == interval.support)
            {
                continue;
            }
            if sources.len() >= MAX_PROFILE_SOURCES {
                return Err(error("computed profile source limit exceeded"));
            }
            let (projected, projection_roundoff) = add_native(
                &mut document,
                native,
                interval.support,
                [interval.start, interval.end],
                projection_error,
            )?;
            roundoff = roundoff.max(projection_roundoff);
            sources.push(SourceFragment {
                original: interval.support,
                interval: [interval.start, interval.end],
                projected,
            });
        }
    }
    for edge in accepted.computed.edges() {
        if edge.role != GeometryRole::Profile {
            continue;
        }
        if let ComputedEdgeGeometry::NativeSourceFragment { source, interval } = edge.geometry {
            if sources.len() >= MAX_PROFILE_SOURCES {
                return Err(error("computed profile source limit exceeded"));
            }
            let (projected, projection_roundoff) = add_native(
                &mut document,
                native,
                source.span,
                [interval.start, interval.end],
                projection_error,
            )?;
            roundoff = roundoff.max(projection_roundoff);
            sources.push(SourceFragment {
                original: source.span,
                interval: [interval.start, interval.end],
                projected,
            });
            mapping.insert(edge.id.ordinal, projected);
        }
    }
    let topology = PreparedEndpointTopologyQuery::capture(
        &accepted.session,
        EndpointTopologyRequest {
            max_spans: MAX_PROFILE_SOURCES * 4,
            max_adjacencies: MAX_PROFILE_SOURCES * 4,
            max_connection_attempts: MAX_PROFILE_SOURCES * 16,
        },
    )
    .map_err(error)?
    .execute()
    .map_err(error)?;
    for adjacency in topology.adjacencies() {
        let find = |index: usize| {
            let endpoint = adjacency.endpoints[index];
            let parameter: f64 = match endpoint.endpoint {
                OffsetEndpointRole::Start => 0.0,
                OffsetEndpointRole::End => 1.0,
            };
            sources.iter().find_map(|source| {
                (source.original == endpoint.span)
                    .then(|| {
                        source
                            .interval
                            .iter()
                            .position(|candidate| candidate.to_bits() == parameter.to_bits())
                            .map(|end| (source.projected, end))
                    })
                    .flatten()
            })
        };
        if let (Some(first), Some(second)) = (find(0), find(1)) {
            add_join(&mut document, first, second, None)?;
        }
    }
    for edge in accepted.computed.edges() {
        if edge.role != GeometryRole::Profile {
            continue;
        }
        let ComputedEdgeGeometry::CircularArc(arc) = &edge.geometry else {
            if matches!(
                edge.geometry,
                ComputedEdgeGeometry::NativeSourceFragment { .. }
            ) {
                continue;
            }
            return Err(error("unsupported computed edge family"));
        };
        let ComputedEdgeProvenance::FilletArc { owner, .. } = edge.provenance else {
            return Err(error("computed arc has no authenticated Fillet owner"));
        };
        let feature = accepted
            .features
            .feature(owner.feature)
            .ok_or_else(|| error("missing computed owner"))?;
        let ComputedFeatureDefinition::FilletSet(set) = &feature.definition;
        let corner = set
            .corners
            .iter()
            .find(|corner| corner.id == owner.corner)
            .ok_or_else(|| error("missing computed corner"))?;
        if document.curves().len() >= MAX_PROFILE_SOURCES {
            return Err(error("computed profile source limit exceeded"));
        }
        let curve = add_arc(
            &mut document,
            arc.center,
            arc.radius,
            arc.start_angle,
            arc.end_angle,
            arc.sweep,
        )?;
        let ordered = match corner.endpoint_order {
            geosolve_sketch::DocumentFilletEndpointOrder::FirstThenSecond => [0, 1],
            geosolve_sketch::DocumentFilletEndpointOrder::SecondThenFirst => [1, 0],
        };
        for (index, contact) in arc.contacts.iter().enumerate() {
            let endpoint = sources
                .iter()
                .find_map(|source| {
                    if source.original != contact.source.span {
                        return None;
                    }
                    source
                        .interval
                        .iter()
                        .position(|parameter| {
                            parameter.to_bits() == contact.total_parameter.to_bits()
                        })
                        .map(|end| (source.projected, end))
                })
                .ok_or_else(|| error("computed contact has no exact retained source boundary"))?;
            add_join(
                &mut document,
                endpoint,
                (curve, ordered[index]),
                Some(arc.tangent_orientations[index]),
            )?;
        }
        mapping.insert(edge.id.ordinal, curve);
    }
    let points = document.points().to_vec();
    let scalars = document.scalars().to_vec();
    let session = RetainedSketchDocumentSession::new(
        document,
        DocumentSolveRequest::default().without_previous_state_preferences(),
        SolverConfig::default(),
    )
    .map_err(error)?;
    let projected = session
        .accepted_state_for_current_input()
        .ok_or_else(|| error("projection did not independently validate"))?
        .document();
    if projected.points() != points || projected.scalars() != scalars {
        return Err(error(
            "computed profile projection changed accepted geometry",
        ));
    }
    let native_mapping = sources
        .into_iter()
        .map(|source| (source.projected, source.original))
        .collect();
    Ok((session, mapping, native_mapping, roundoff))
}

fn add_join(
    document: &mut SketchDocument,
    first: (CurveId, usize),
    second: (CurveId, usize),
    orientation: Option<geosolve_sketch::TangentOrientation>,
) -> Result<(), EngineError> {
    let label = format!("export.join.{}", document.constraints().len());
    let mut contact = |end: (CurveId, usize), index: usize, orientation| {
        document
            .add_curve_contact(
                format!("{label}.{index}"),
                CurveSpan::line(end.0),
                if end.1 == 0 { 0.0 } else { 1.0 },
                0,
                if end.1 == 0 {
                    ContactNeighborhood::Start
                } else {
                    ContactNeighborhood::End
                },
                orientation,
            )
            .map_err(error)
    };
    let first_contact = contact(first, 0, orientation)?;
    let second_contact = contact(second, 1, orientation)?;
    let definition = if orientation.is_some() {
        DocumentConstraintDefinition::CurveCurveTangency {
            first_contact,
            second_contact,
        }
    } else {
        DocumentConstraintDefinition::CurveCurveContact {
            first_contact,
            second_contact,
        }
    };
    document.add_constraint(label, definition).map_err(error)?;
    Ok(())
}

fn add_native(
    document: &mut SketchDocument,
    native: &SketchDocument,
    span: CurveSpan,
    interval: [f64; 2],
    projection_error: f64,
) -> Result<(CurveId, f64), EngineError> {
    let source = native
        .curve(span.curve)
        .ok_or_else(|| error("missing native source"))?;
    let label = format!("export.source.{}", document.curves().len());
    let projected = match source.definition {
        CurveDefinition::Line { .. } | CurveDefinition::Polyline { .. } => {
            let start = native
                .evaluate_curve_jet(span, interval[0])
                .map_err(error)?
                .position;
            let end = native
                .evaluate_curve_jet(span, interval[1])
                .map_err(error)?
                .position;
            let direction = [end.x - start.x, end.y - start.y];
            let length = direction[0].hypot(direction[1]);
            if !length.is_finite() || length <= 0.0 {
                return Err(error("invalid projected line length"));
            }
            let direction = [direction[0] / length, direction[1] / length];
            let start = document
                .add_point(format!("{label}.start"), [start.x, start.y])
                .map_err(error)?;
            let end = document
                .add_point(format!("{label}.end"), [end.x, end.y])
                .map_err(error)?;
            document
                .add_curve(
                    label,
                    CurveDefinition::Line {
                        start,
                        end,
                        branch_direction: direction,
                    },
                )
                .map_err(error)
        }
        CurveDefinition::Circle { center, radius }
        | CurveDefinition::CircularArc { center, radius, .. } => {
            let center = native
                .point(center)
                .ok_or_else(|| error("missing circle center"))?
                .position;
            let radius = native
                .scalar(radius)
                .ok_or_else(|| error("missing circle radius"))?
                .value;
            if matches!(source.definition, CurveDefinition::Circle { .. })
                && (interval[1] - interval[0] - std::f64::consts::TAU).abs()
                    <= std::f64::consts::TAU * 32.0 * f64::EPSILON
            {
                let center = document
                    .add_point(format!("{label}.center"), center)
                    .map_err(error)?;
                let radius = document
                    .add_scalar(
                        format!("{label}.radius"),
                        radius,
                        ScalarUnit::Length,
                        ScalarDomain::Positive,
                    )
                    .map_err(error)?;
                return document
                    .add_curve(label, CurveDefinition::Circle { center, radius })
                    .map(|curve| (curve, 0.0))
                    .map_err(error);
            }
            let (start_angle, end_angle, sweep) =
                native_arc_interval(native, span, interval, radius)?;
            add_arc(document, center, radius, start_angle, end_angle, sweep)
        }
        _ => Err(error(format!(
            "unsupported profile curve family on {}",
            source.label
        ))),
    }?;
    let roundoff = validate_native_projection(
        native,
        span,
        interval,
        document,
        projected,
        projection_error,
    )?;
    Ok((projected, roundoff))
}

fn native_arc_interval(
    native: &SketchDocument,
    span: CurveSpan,
    interval: [f64; 2],
    radius: f64,
) -> Result<(f64, f64, geosolve_sketch::DocumentArcSweep), EngineError> {
    let source = native
        .curve(span.curve)
        .ok_or_else(|| error("missing native source"))?;
    Ok(match source.definition {
        CurveDefinition::CircularArc {
            start_angle,
            end_angle,
            sweep,
            ..
        } => {
            let start = native
                .scalar(start_angle)
                .ok_or_else(|| error("missing arc start angle"))?
                .value;
            let end = native
                .scalar(end_angle)
                .ok_or_else(|| error("missing arc end angle"))?
                .value;
            if interval.map(f64::to_bits) == [0.0_f64.to_bits(), 1.0_f64.to_bits()] {
                (start, end, sweep)
            } else {
                let jet = native.evaluate_curve_jet(span, 0.0).map_err(error)?;
                let sign = if sweep == geosolve_sketch::DocumentArcSweep::CounterClockwise {
                    1.0
                } else {
                    -1.0
                };
                let rate = sign * jet.first_derivative.norm() / radius;
                (
                    start + rate * interval[0],
                    start + rate * interval[1],
                    sweep,
                )
            }
        }
        _ => (
            interval[0],
            interval[1],
            geosolve_sketch::DocumentArcSweep::CounterClockwise,
        ),
    })
}

fn validate_native_projection(
    native: &SketchDocument,
    source: CurveSpan,
    interval: [f64; 2],
    projected: &SketchDocument,
    curve: CurveId,
    budget: f64,
) -> Result<f64, EngineError> {
    // Both representations are the same analytic line or circular interval, with an affine
    // parameter map. Bound floating-point evaluation/reparameterization roundoff and check
    // the independent source and projected jets before the owning polygon sampler runs.
    let source_curve = native
        .curve(source.curve)
        .ok_or_else(|| error("missing native source"))?;
    let magnitude = match source_curve.definition {
        CurveDefinition::Circle { center, radius }
        | CurveDefinition::CircularArc { center, radius, .. } => {
            let center = native
                .point(center)
                .ok_or_else(|| error("missing native center"))?
                .position;
            let radius = native
                .scalar(radius)
                .ok_or_else(|| error("missing native radius"))?
                .value;
            let angles = match source_curve.definition {
                CurveDefinition::CircularArc {
                    start_angle,
                    end_angle,
                    ..
                } => {
                    native
                        .scalar(start_angle)
                        .ok_or_else(|| error("missing native start"))?
                        .value
                        .abs()
                        + native
                            .scalar(end_angle)
                            .ok_or_else(|| error("missing native end"))?
                            .value
                            .abs()
                }
                _ => interval[0].abs() + interval[1].abs(),
            };
            center[0].abs() + center[1].abs() + radius * (1.0 + angles)
        }
        _ => {
            let first = native
                .evaluate_curve_jet(source, interval[0])
                .map_err(error)?
                .position;
            let last = native
                .evaluate_curve_jet(source, interval[1])
                .map_err(error)?
                .position;
            first.x.abs() + first.y.abs() + last.x.abs() + last.y.abs()
        }
    };
    let roundoff = 256.0 * f64::EPSILON * magnitude.max(f64::MIN_POSITIVE);
    if !roundoff.is_finite() || roundoff > budget {
        return Err(error(
            "requested chord error cannot bound analytic projection roundoff",
        ));
    }
    for parameter in [0.0, 0.25, 0.5, 0.75, 1.0] {
        let original = native
            .evaluate_curve_jet(
                source,
                interval[0] + (interval[1] - interval[0]) * parameter,
            )
            .map_err(error)?;
        let candidate = projected
            .evaluate_curve_jet(CurveSpan::line(curve), parameter)
            .map_err(error)?;
        if (original.position.x - candidate.position.x)
            .hypot(original.position.y - candidate.position.y)
            > roundoff
        {
            return Err(error(
                "analytic projection does not preserve accepted source geometry",
            ));
        }
    }
    Ok(roundoff)
}

fn add_arc(
    document: &mut SketchDocument,
    center: [f64; 2],
    radius: f64,
    start: f64,
    end: f64,
    sweep: geosolve_sketch::DocumentArcSweep,
) -> Result<CurveId, EngineError> {
    let label = format!("export.arc.{}", document.curves().len());
    let center = document
        .add_point(format!("{label}.center"), center)
        .map_err(error)?;
    let radius = document
        .add_scalar(
            format!("{label}.radius"),
            radius,
            ScalarUnit::Length,
            ScalarDomain::Positive,
        )
        .map_err(error)?;
    let start_angle = document
        .add_scalar(
            format!("{label}.start"),
            start,
            ScalarUnit::Angle,
            ScalarDomain::Finite,
        )
        .map_err(error)?;
    let end_angle = document
        .add_scalar(
            format!("{label}.end"),
            end,
            ScalarUnit::Angle,
            ScalarDomain::Finite,
        )
        .map_err(error)?;
    document
        .add_curve(
            label,
            CurveDefinition::CircularArc {
                center,
                radius,
                start_angle,
                end_angle,
                sweep,
            },
        )
        .map_err(error)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn circular_projection_preserves_partial_periodic_and_clockwise_winding() {
        let mut native = SketchDocument::new(1.0).unwrap();
        let center = native.add_point("center", [10.0, -8.0]).unwrap();
        let radius = native
            .add_scalar("radius", 3.0, ScalarUnit::Length, ScalarDomain::Positive)
            .unwrap();
        let circle = native
            .add_curve("circle", CurveDefinition::Circle { center, radius })
            .unwrap();
        let start_angle = native
            .add_scalar(
                "start",
                -6.0 * std::f64::consts::PI,
                ScalarUnit::Angle,
                ScalarDomain::Finite,
            )
            .unwrap();
        let end_angle = native
            .add_scalar(
                "end",
                -7.0 * std::f64::consts::PI,
                ScalarUnit::Angle,
                ScalarDomain::Finite,
            )
            .unwrap();
        let radius = native
            .add_scalar(
                "arc.radius",
                3.0,
                ScalarUnit::Length,
                ScalarDomain::Positive,
            )
            .unwrap();
        let arc = native
            .add_curve(
                "clockwise",
                CurveDefinition::CircularArc {
                    center,
                    radius,
                    start_angle,
                    end_angle,
                    sweep: geosolve_sketch::DocumentArcSweep::Clockwise,
                },
            )
            .unwrap();
        let mut projected = SketchDocument::new(1.0).unwrap();
        for (source, interval, angles) in [
            (
                circle,
                [-1.5 * std::f64::consts::PI, -0.5 * std::f64::consts::PI],
                [
                    std::f64::consts::FRAC_PI_2,
                    std::f64::consts::PI,
                    1.5 * std::f64::consts::PI,
                ],
            ),
            (
                arc,
                [0.25, 0.75],
                [
                    -std::f64::consts::FRAC_PI_4,
                    -std::f64::consts::FRAC_PI_2,
                    -3.0 * std::f64::consts::FRAC_PI_4,
                ],
            ),
        ] {
            let (curve, _) = add_native(
                &mut projected,
                &native,
                CurveSpan::line(source),
                interval,
                1e-8,
            )
            .unwrap();
            for (parameter, angle) in [0.0, 0.5, 1.0].into_iter().zip(angles) {
                let point = projected
                    .evaluate_curve_jet(CurveSpan::line(curve), parameter)
                    .unwrap()
                    .position;
                assert!(
                    (point.x - (10.0 + 3.0 * angle.cos()))
                        .hypot(point.y - (-8.0 + 3.0 * angle.sin()))
                        < 1e-12
                );
            }
        }
        assert!(
            add_native(
                &mut projected,
                &native,
                CurveSpan::line(arc),
                [0.25, 0.75],
                1e-30
            )
            .is_err()
        );
    }

    #[test]
    fn changed_feature_identity_cannot_export_cached_computed_geometry() {
        let artifact = geosolve_sketch_code::GeneratedSketchArtifact::from_json(include_str!(
            "../tests/fixtures/channel.json"
        ))
        .unwrap();
        let project = geosolve_sketch_code::materialize_generated_sketch_cold(
            &artifact,
            geosolve_sketch_intent::IntentSessionId::from_raw(980_200),
            geosolve_sketch::DocumentId(geosolve_sketch::PersistentId::from_u128(980_201)),
            1.0,
        )
        .unwrap();
        let mut accepted = project
            .editor
            .coordinator()
            .accepted_materialization()
            .unwrap()
            .clone();
        validate_authority(&accepted).unwrap();
        let feature = accepted.features.features()[0].id;
        accepted.features.set_suppressed(feature, true).unwrap();
        let failure = export(&accepted, 0.02, None).unwrap_err().to_string();
        assert!(
            failure.contains("do not share current accepted authority"),
            "{failure}"
        );
    }
}
