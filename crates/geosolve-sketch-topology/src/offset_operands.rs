// SPDX-License-Identifier: GPL-3.0-or-later

//! Exact accepted-input-stamped operands for constraint-friendly profile offsets.
//!
//! This is deliberately separate from [`crate::TopologyProductionProfile`]. A complete
//! operand index may retain one independently valid face beside unrelated open or
//! offset-unsupported Profile geometry, but an interrupted or incomplete visual analysis never
//! publishes a usable prefix. Query-local wire and region ordinals are not part of any key.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

use geosolve_sketch::{
    ContactDomain, ContactNeighborhood, CurveDefinition, CurveSpan, DesignPointId,
    DocumentArcSweep, DocumentConstraintDefinition, DocumentConstraintId, EffectiveActivity,
    FeatureEndpoint, GeometryRole, OperationControl, OperationOutcome, PreparedSketchInput,
    RetainedSketchDocumentSession, SketchAcceptedStateIdentity, SketchDocument,
    VisualProfileAnalysis, VisualProfileContour, VisualProfileEdge, VisualProfileFace,
    VisualProfileIssue, VisualProfileStatus,
};
use thiserror::Error;

use crate::{
    TopologyBudgetReport, TopologyCompleteness, TopologyError, TopologyLimits, TopologySnapshot,
    TopologySnapshotError, topology_budgets,
};

/// Deterministic bounded request for one offset-operand index.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct OffsetOperandRequest {
    pub limits: TopologyLimits,
}

/// Worker-movable immutable offset-operand query.
#[derive(Debug)]
pub struct PreparedOffsetOperandQuery {
    snapshot: TopologySnapshot,
    request: OffsetOperandRequest,
}

impl PreparedOffsetOperandQuery {
    /// Captures and prepares one current independently accepted operand query.
    ///
    /// # Errors
    ///
    /// Rejects absent, historical or input-mismatched accepted state.
    pub fn capture(
        session: &RetainedSketchDocumentSession,
        request: OffsetOperandRequest,
    ) -> Result<Self, TopologySnapshotError> {
        Ok(TopologySnapshot::capture(session)?.prepare_offset_operands(request))
    }

    #[must_use]
    pub const fn input(&self) -> PreparedSketchInput {
        self.snapshot.input
    }

    #[must_use]
    pub const fn request(&self) -> OffsetOperandRequest {
        self.request
    }

    /// Executes one bounded read-only operand query.
    ///
    /// Cancellation and work exhaustion are outer outcomes and therefore cannot carry an index.
    /// A completed but truncated/skipped analysis reports its evidence with `operand_index ==
    /// None`; no clean-component prefix becomes authoring authority.
    ///
    /// # Errors
    ///
    /// Returns a typed invalid-limit error before executing profile analysis.
    pub fn execute(
        self,
        control: OperationControl,
    ) -> Result<OperationOutcome<OffsetOperandResult>, TopologyError> {
        self.request.limits.validate()?;
        let input = self.snapshot.input;
        let accepted = self.snapshot.accepted;
        let document = self.snapshot.document;
        let request = self.request;
        let analysis =
            document.analyze_visual_profiles_controlled(request.limits.visual_options(), control);
        Ok(analysis.map(move |analysis| {
            build_operand_result(&input, accepted, request, document, &analysis)
        }))
    }
}

impl TopologySnapshot {
    /// Prepares offset-specific face and manual-chain operands from this exact accepted snapshot.
    #[must_use]
    pub fn prepare_offset_operands(
        self,
        request: OffsetOperandRequest,
    ) -> PreparedOffsetOperandQuery {
        PreparedOffsetOperandQuery {
            snapshot: self,
            request,
        }
    }
}

/// Native family reported for one curve-span operand.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum OffsetOperandCurveFamily {
    Line,
    CircularArc,
    Circle,
    Ellipse,
    EllipticalArc,
    RationalQuadraticConic,
    Parabola,
    Hyperbola,
    QuadraticBezier,
    CubicBezier,
    BSpline,
    Nurbs,
}

impl OffsetOperandCurveFamily {
    #[must_use]
    pub const fn is_exact_offset_supported(self) -> bool {
        matches!(self, Self::Line | Self::CircularArc | Self::Circle)
    }
}

/// Why a face or curve span cannot be used by the exact associative offset operation.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum OffsetOperandIneligibility {
    NonProfileGeometry,
    UnsupportedCurveFamily,
    TrimmedOrPartialSpan,
    ArrangementDerivedFragment,
    UnownedEndpointJoin,
}

/// Complete typed eligibility for one operand.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OffsetOperandEligibility {
    Eligible,
    Disabled {
        reasons: Vec<OffsetOperandIneligibility>,
    },
}

impl OffsetOperandEligibility {
    #[must_use]
    pub const fn is_eligible(&self) -> bool {
        matches!(self, Self::Eligible)
    }

    fn from_reasons(mut reasons: Vec<OffsetOperandIneligibility>) -> Self {
        reasons.sort_unstable();
        reasons.dedup();
        if reasons.is_empty() {
            Self::Eligible
        } else {
            Self::Disabled { reasons }
        }
    }

    fn reasons(&self) -> impl Iterator<Item = OffsetOperandIneligibility> + '_ {
        match self {
            Self::Eligible => [].iter().copied(),
            Self::Disabled { reasons } => reasons.iter().copied(),
        }
    }
}

/// Explicit traversal of one persistent native span.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum OffsetTraversal {
    Forward,
    Reverse,
}

/// One directed complete native span in a canonical face contour.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct OffsetDirectedSpan {
    pub span: CurveSpan,
    pub traversal: OffsetTraversal,
}

/// Rotation-canonical ordered contour key. Direction is preserved; reversal is not canonicalized.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct OffsetContourKey {
    pub spans: Vec<OffsetDirectedSpan>,
}

/// Stable semantic face key. Hole order is canonical and query-local region IDs are absent.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct OffsetFaceKey {
    pub outer: OffsetContourKey,
    pub holes: Vec<OffsetContourKey>,
}

/// Native terminal of a bounded supported span.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum OffsetEndpointRole {
    Start,
    End,
}

/// Stable semantic reference to one bounded native span endpoint.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct OffsetEndpointRef {
    pub span: CurveSpan,
    pub endpoint: OffsetEndpointRole,
}

/// Persistent ownership establishing endpoint adjacency without coordinate welding.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum OffsetJoinOwner {
    SharedPoint(DesignPointId),
    Constraint(DocumentConstraintId),
}

/// One undirected exact endpoint adjacency. Endpoints are stored in canonical order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OffsetEndpointAdjacency {
    pub endpoints: [OffsetEndpointRef; 2],
    pub owners: Vec<OffsetJoinOwner>,
}

/// Whether one endpoint is free, has exactly one continuation, or is a branch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OffsetEndpointEligibility {
    Terminal,
    Joined,
    Branched { adjacent: usize },
}

/// One finite supported endpoint with adjacency classification.
#[derive(Clone, Debug, PartialEq)]
pub struct OffsetEndpointCandidate {
    pub endpoint: OffsetEndpointRef,
    pub position: [f64; 2],
    pub eligibility: OffsetEndpointEligibility,
}

/// One native curve-span candidate for manual chain collection.
#[derive(Clone, Debug, PartialEq)]
pub struct OffsetSpanCandidate {
    pub span: CurveSpan,
    pub family: OffsetOperandCurveFamily,
    pub eligibility: OffsetOperandEligibility,
    pub periodic: bool,
    pub endpoints: Vec<OffsetEndpointCandidate>,
}

/// One visual face projected to a stable semantic key and typed exact-offset eligibility.
#[derive(Clone, Debug, PartialEq)]
pub struct OffsetFaceCandidate {
    pub key: OffsetFaceKey,
    pub visual_area: f64,
    pub eligibility: OffsetOperandEligibility,
    hit_contours: Vec<VisualProfileContour>,
}

/// Deterministic result of model-space face lookup.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OffsetFaceLookup {
    None,
    Hit(OffsetFaceKey),
    BoundaryAmbiguous { candidates: Vec<OffsetFaceKey> },
}

/// Exact complete offset operands for one accepted prepared input.
#[derive(Clone, Debug, PartialEq)]
pub struct OffsetOperandIndex {
    input: PreparedSketchInput,
    accepted: SketchAcceptedStateIdentity,
    document: SketchDocument,
    spans: Vec<OffsetSpanCandidate>,
    faces: Vec<OffsetFaceCandidate>,
    adjacencies: Vec<OffsetEndpointAdjacency>,
}

impl OffsetOperandIndex {
    #[must_use]
    pub const fn input(&self) -> PreparedSketchInput {
        self.input
    }

    #[must_use]
    pub const fn accepted_state_identity(&self) -> SketchAcceptedStateIdentity {
        self.accepted
    }

    #[must_use]
    pub fn spans(&self) -> &[OffsetSpanCandidate] {
        &self.spans
    }

    #[must_use]
    pub fn faces(&self) -> &[OffsetFaceCandidate] {
        &self.faces
    }

    #[must_use]
    pub fn adjacencies(&self) -> &[OffsetEndpointAdjacency] {
        &self.adjacencies
    }

    #[must_use]
    pub fn span(&self, span: CurveSpan) -> Option<&OffsetSpanCandidate> {
        self.spans.iter().find(|candidate| candidate.span == span)
    }

    #[must_use]
    pub fn face(&self, key: &OffsetFaceKey) -> Option<&OffsetFaceCandidate> {
        self.faces.iter().find(|candidate| &candidate.key == key)
    }

    /// Returns exact semantic neighbours of one endpoint in canonical order.
    pub fn adjacent_endpoints(
        &self,
        endpoint: OffsetEndpointRef,
    ) -> impl Iterator<Item = OffsetEndpointRef> + '_ {
        self.adjacencies.iter().filter_map(move |adjacency| {
            if adjacency.endpoints[0] == endpoint {
                Some(adjacency.endpoints[1])
            } else if adjacency.endpoints[1] == endpoint {
                Some(adjacency.endpoints[0])
            } else {
                None
            }
        })
    }

    /// Finds the smallest face containing a finite model-space point.
    ///
    /// Boundary-near input is never guessed to one side. Faces containing unsupported curve
    /// families remain enumerable and disabled, but are skipped by this exact supported-family
    /// lookup; hovering their boundary spans still exposes the typed family reason.
    #[must_use]
    pub fn face_at_point(&self, point: [f64; 2]) -> OffsetFaceLookup {
        if !point.into_iter().all(f64::is_finite) {
            return OffsetFaceLookup::None;
        }
        let mut inside = Vec::new();
        let mut boundary = Vec::new();
        for candidate in &self.faces {
            match face_containment(&self.document, candidate, point) {
                FaceContainment::Inside => {
                    inside.push((candidate.visual_area, candidate.key.clone()));
                }
                FaceContainment::Boundary => boundary.push(candidate.key.clone()),
                FaceContainment::Outside | FaceContainment::Unavailable => {}
            }
        }
        if !boundary.is_empty() {
            boundary.sort();
            boundary.dedup();
            return OffsetFaceLookup::BoundaryAmbiguous {
                candidates: boundary,
            };
        }
        inside.sort_by(|first, second| {
            first
                .0
                .total_cmp(&second.0)
                .then_with(|| first.1.cmp(&second.1))
        });
        inside
            .into_iter()
            .next()
            .map_or(OffsetFaceLookup::None, |(_, key)| {
                OffsetFaceLookup::Hit(key)
            })
    }

    /// Revalidates exact live-session provenance before authoring consumption.
    ///
    /// # Errors
    ///
    /// Rejects any newer design, attempt, accepted state, parameter, activation or external
    /// snapshot input.
    pub fn validate_current(
        &self,
        session: &RetainedSketchDocumentSession,
    ) -> Result<(), OffsetOperandConsumptionError> {
        let accepted = session
            .accepted_state_for_current_input()
            .ok_or(OffsetOperandConsumptionError::Stale)?;
        if session.prepared_input() != self.input
            || accepted.identity() != self.accepted
            || accepted.design_identity() != session.design_identity()
        {
            return Err(OffsetOperandConsumptionError::Stale);
        }
        Ok(())
    }
}

/// An operand index is stale after any accepted-input transition.
#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum OffsetOperandConsumptionError {
    #[error("offset operands are stale for the current sketch input")]
    Stale,
}

/// Completed operand-query evidence. Incomplete results never carry an index.
#[derive(Clone, Debug, PartialEq)]
pub struct OffsetOperandResult {
    pub input: PreparedSketchInput,
    pub accepted: SketchAcceptedStateIdentity,
    pub request: OffsetOperandRequest,
    pub completeness: TopologyCompleteness,
    pub budgets: TopologyBudgetReport,
    pub issues: Vec<VisualProfileIssue>,
    pub operand_index: Option<OffsetOperandIndex>,
}

fn build_operand_result(
    input: &PreparedSketchInput,
    accepted: SketchAcceptedStateIdentity,
    request: OffsetOperandRequest,
    document: SketchDocument,
    analysis: &VisualProfileAnalysis,
) -> OffsetOperandResult {
    let completeness = match analysis.status {
        VisualProfileStatus::Complete => TopologyCompleteness::Complete,
        VisualProfileStatus::Truncated => TopologyCompleteness::Truncated,
        VisualProfileStatus::Skipped => TopologyCompleteness::Skipped,
    };
    let budgets = topology_budgets(analysis);
    let issues = analysis.issues.clone();
    let operand_index = (analysis.status == VisualProfileStatus::Complete)
        .then(|| build_operand_index(input, accepted, document, analysis));
    OffsetOperandResult {
        input: *input,
        accepted,
        request,
        completeness,
        budgets,
        issues,
        operand_index,
    }
}

#[derive(Clone, Copy, Debug)]
struct EndpointSeed {
    endpoint: OffsetEndpointRef,
    position: [f64; 2],
    point: Option<DesignPointId>,
}

#[allow(clippy::too_many_lines)]
fn build_operand_index(
    input: &PreparedSketchInput,
    accepted: SketchAcceptedStateIdentity,
    document: SketchDocument,
    analysis: &VisualProfileAnalysis,
) -> OffsetOperandIndex {
    let activity = document.effective_activity();
    let arrangement_spans = analysis
        .intersections
        .iter()
        .flat_map(|intersection| [intersection.first_span, intersection.second_span])
        .collect::<BTreeSet<_>>();
    let mut spans = Vec::new();
    let mut endpoint_seeds = Vec::new();
    for curve in document.curves() {
        if !activity.is_active(curve.id) {
            continue;
        }
        let family = curve_family(&curve.definition);
        let role = document
            .geometry_role(curve.id)
            .expect("accepted curve has a geometry role");
        let Ok(curve_spans) = document.curve_spans(curve.id) else {
            continue;
        };
        for span in curve_spans {
            let mut reasons = Vec::new();
            if role != GeometryRole::Profile {
                reasons.push(OffsetOperandIneligibility::NonProfileGeometry);
            }
            if !family.is_exact_offset_supported() {
                reasons.push(OffsetOperandIneligibility::UnsupportedCurveFamily);
            }
            if document.trim_views_for_span(span).next().is_some() {
                reasons.push(OffsetOperandIneligibility::TrimmedOrPartialSpan);
            }
            if arrangement_spans.contains(&span) {
                reasons.push(OffsetOperandIneligibility::ArrangementDerivedFragment);
            }
            let periodic = family == OffsetOperandCurveFamily::Circle;
            let seeds = supported_endpoint_seeds(&document, span, &curve.definition);
            endpoint_seeds.extend(seeds.iter().copied());
            spans.push(OffsetSpanCandidate {
                span,
                family,
                eligibility: OffsetOperandEligibility::from_reasons(reasons),
                periodic,
                endpoints: seeds
                    .iter()
                    .map(|seed| OffsetEndpointCandidate {
                        endpoint: seed.endpoint,
                        position: seed.position,
                        eligibility: OffsetEndpointEligibility::Terminal,
                    })
                    .collect(),
            });
        }
    }
    spans.sort_by_key(|candidate| candidate.span);
    endpoint_seeds.sort_by_key(|candidate| candidate.endpoint);

    let adjacencies = build_endpoint_adjacencies(&document, &activity, &endpoint_seeds);
    let mut endpoint_degree = BTreeMap::<OffsetEndpointRef, usize>::new();
    for adjacency in &adjacencies {
        for endpoint in adjacency.endpoints {
            *endpoint_degree.entry(endpoint).or_default() += 1;
        }
    }
    for span in &mut spans {
        for endpoint in &mut span.endpoints {
            endpoint.eligibility = match endpoint_degree
                .get(&endpoint.endpoint)
                .copied()
                .unwrap_or_default()
            {
                0 => OffsetEndpointEligibility::Terminal,
                1 => OffsetEndpointEligibility::Joined,
                adjacent => OffsetEndpointEligibility::Branched { adjacent },
            };
        }
    }
    let span_by_id = spans
        .iter()
        .map(|candidate| (candidate.span, candidate))
        .collect::<BTreeMap<_, _>>();
    let adjacency_pairs = adjacencies
        .iter()
        .map(|adjacency| adjacency.endpoints)
        .collect::<BTreeSet<_>>();
    let mut faces = analysis
        .faces
        .iter()
        .filter_map(|face| build_face_candidate(face, &span_by_id, &adjacency_pairs))
        .collect::<Vec<_>>();
    faces.sort_by(|first, second| {
        first
            .key
            .cmp(&second.key)
            .then_with(|| first.visual_area.total_cmp(&second.visual_area))
    });
    faces.dedup_by(|first, second| first.key == second.key);

    OffsetOperandIndex {
        input: *input,
        accepted,
        document,
        spans,
        faces,
        adjacencies,
    }
}

const fn curve_family(definition: &CurveDefinition) -> OffsetOperandCurveFamily {
    match definition {
        CurveDefinition::Line { .. } | CurveDefinition::Polyline { .. } => {
            OffsetOperandCurveFamily::Line
        }
        CurveDefinition::Circle { .. } => OffsetOperandCurveFamily::Circle,
        CurveDefinition::CircularArc { .. } => OffsetOperandCurveFamily::CircularArc,
        CurveDefinition::Ellipse { .. } => OffsetOperandCurveFamily::Ellipse,
        CurveDefinition::EllipticalArc { .. } => OffsetOperandCurveFamily::EllipticalArc,
        CurveDefinition::RationalQuadraticConic { .. } => {
            OffsetOperandCurveFamily::RationalQuadraticConic
        }
        CurveDefinition::ParabolaSegment { .. } => OffsetOperandCurveFamily::Parabola,
        CurveDefinition::HyperbolaSegment { .. } => OffsetOperandCurveFamily::Hyperbola,
        CurveDefinition::QuadraticBezier { .. } => OffsetOperandCurveFamily::QuadraticBezier,
        CurveDefinition::CubicBezier { .. } => OffsetOperandCurveFamily::CubicBezier,
        CurveDefinition::BSpline { .. } => OffsetOperandCurveFamily::BSpline,
        CurveDefinition::Nurbs { .. } => OffsetOperandCurveFamily::Nurbs,
    }
}

fn supported_endpoint_seeds(
    document: &SketchDocument,
    span: CurveSpan,
    definition: &CurveDefinition,
) -> Vec<EndpointSeed> {
    let point_for = |point: DesignPointId, endpoint| {
        document.point(point).map(|value| EndpointSeed {
            endpoint: OffsetEndpointRef { span, endpoint },
            position: value.position,
            point: Some(point),
        })
    };
    match definition {
        CurveDefinition::Line { start, end, .. } => [
            point_for(*start, OffsetEndpointRole::Start),
            point_for(*end, OffsetEndpointRole::End),
        ]
        .into_iter()
        .flatten()
        .collect(),
        CurveDefinition::Polyline { points, closed, .. } => {
            let index = span.segment as usize;
            let Some(start) = points.get(index).copied() else {
                return Vec::new();
            };
            let end = if index + 1 < points.len() {
                points[index + 1]
            } else if *closed {
                points[0]
            } else {
                return Vec::new();
            };
            [
                point_for(start, OffsetEndpointRole::Start),
                point_for(end, OffsetEndpointRole::End),
            ]
            .into_iter()
            .flatten()
            .collect()
        }
        CurveDefinition::CircularArc { .. } => [
            (OffsetEndpointRole::Start, 0.0),
            (OffsetEndpointRole::End, 1.0),
        ]
        .into_iter()
        .filter_map(|(endpoint, parameter)| {
            document
                .evaluate_curve_jet(span, parameter)
                .ok()
                .map(|jet| EndpointSeed {
                    endpoint: OffsetEndpointRef { span, endpoint },
                    position: [jet.position.x, jet.position.y],
                    point: None,
                })
        })
        .collect(),
        _ => Vec::new(),
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "one closed connectivity-owner catalog keeps every supported endpoint relation auditable"
)]
fn build_endpoint_adjacencies(
    document: &SketchDocument,
    activity: &EffectiveActivity,
    seeds: &[EndpointSeed],
) -> Vec<OffsetEndpointAdjacency> {
    let mut links = BTreeMap::<[OffsetEndpointRef; 2], BTreeSet<OffsetJoinOwner>>::new();
    let mut by_point = BTreeMap::<DesignPointId, Vec<OffsetEndpointRef>>::new();
    for seed in seeds {
        if let Some(point) = seed.point {
            by_point.entry(point).or_default().push(seed.endpoint);
        }
    }
    for (point, endpoints) in &mut by_point {
        endpoints.sort_unstable();
        endpoints.dedup();
        for first in 0..endpoints.len() {
            for second in first + 1..endpoints.len() {
                add_endpoint_link(
                    &mut links,
                    endpoints[first],
                    endpoints[second],
                    OffsetJoinOwner::SharedPoint(*point),
                );
            }
        }
    }

    let point_endpoints = |point: DesignPointId| by_point.get(&point).cloned().unwrap_or_default();
    for constraint in document.constraints() {
        if !activity.is_active(constraint.id) {
            continue;
        }
        let owner = OffsetJoinOwner::Constraint(constraint.id);
        match constraint.definition {
            DocumentConstraintDefinition::Coincident { first, second } => {
                for first_endpoint in point_endpoints(first) {
                    for second_endpoint in point_endpoints(second) {
                        add_endpoint_link(&mut links, first_endpoint, second_endpoint, owner);
                    }
                }
            }
            DocumentConstraintDefinition::PointOnCurve { point, contact } => {
                if let Some(curve_endpoint) = endpoint_for_contact(document, seeds, contact) {
                    for point_endpoint in point_endpoints(point) {
                        add_endpoint_link(&mut links, point_endpoint, curve_endpoint, owner);
                    }
                }
            }
            DocumentConstraintDefinition::LineCurveTangency {
                line,
                endpoint,
                curve_contact,
            } => {
                let line_endpoint = OffsetEndpointRef {
                    span: line,
                    endpoint: match endpoint {
                        FeatureEndpoint::Start => OffsetEndpointRole::Start,
                        FeatureEndpoint::End => OffsetEndpointRole::End,
                    },
                };
                if seeds.iter().any(|seed| seed.endpoint == line_endpoint)
                    && let Some(curve_endpoint) =
                        endpoint_for_contact(document, seeds, curve_contact)
                {
                    add_endpoint_link(&mut links, line_endpoint, curve_endpoint, owner);
                }
            }
            DocumentConstraintDefinition::LineCircleTangency {
                line_contact,
                circle_contact,
                ..
            }
            | DocumentConstraintDefinition::CircleArcTangency {
                circle_contact: line_contact,
                arc_contact: circle_contact,
                ..
            }
            | DocumentConstraintDefinition::CurveCurveContact {
                first_contact: line_contact,
                second_contact: circle_contact,
            }
            | DocumentConstraintDefinition::CurveCurveTangency {
                first_contact: line_contact,
                second_contact: circle_contact,
            }
            | DocumentConstraintDefinition::EndpointContinuity {
                first_contact: line_contact,
                second_contact: circle_contact,
                ..
            } => {
                if let (Some(first), Some(second)) = (
                    endpoint_for_contact(document, seeds, line_contact),
                    endpoint_for_contact(document, seeds, circle_contact),
                ) {
                    add_endpoint_link(&mut links, first, second, owner);
                }
            }
            _ => {}
        }
    }
    links
        .into_iter()
        .map(|(endpoints, owners)| OffsetEndpointAdjacency {
            endpoints,
            owners: owners.into_iter().collect(),
        })
        .collect()
}

fn add_endpoint_link(
    links: &mut BTreeMap<[OffsetEndpointRef; 2], BTreeSet<OffsetJoinOwner>>,
    first: OffsetEndpointRef,
    second: OffsetEndpointRef,
    owner: OffsetJoinOwner,
) {
    if first == second {
        return;
    }
    let endpoints = if first < second {
        [first, second]
    } else {
        [second, first]
    };
    links.entry(endpoints).or_default().insert(owner);
}

fn endpoint_for_contact(
    document: &SketchDocument,
    seeds: &[EndpointSeed],
    contact_id: geosolve_sketch::ContactId,
) -> Option<OffsetEndpointRef> {
    let contact = document.contact(contact_id)?;
    if contact.domain
        != (ContactDomain::Bounded {
            lower: 0.0,
            upper: 1.0,
        })
        || contact.winding != 0
    {
        return None;
    }
    let principal = document.scalar(contact.parameter)?.value;
    let endpoint = match (principal.to_bits(), contact.neighborhood) {
        (bits, ContactNeighborhood::Start) if bits == 0.0_f64.to_bits() => {
            OffsetEndpointRole::Start
        }
        (bits, ContactNeighborhood::End) if bits == 1.0_f64.to_bits() => OffsetEndpointRole::End,
        _ => return None,
    };
    let reference = OffsetEndpointRef {
        span: contact.curve,
        endpoint,
    };
    seeds
        .iter()
        .any(|seed| seed.endpoint == reference)
        .then_some(reference)
}

fn build_face_candidate(
    face: &VisualProfileFace,
    spans: &BTreeMap<CurveSpan, &OffsetSpanCandidate>,
    adjacencies: &BTreeSet<[OffsetEndpointRef; 2]>,
) -> Option<OffsetFaceCandidate> {
    let mut contours = Vec::new();
    let mut reasons = Vec::new();
    for contour in &face.contours {
        let (key, mut contour_reasons) = build_contour_key(contour, spans, adjacencies)?;
        contours.push(key);
        reasons.append(&mut contour_reasons);
    }
    let outer = contours.first()?.clone();
    let mut holes = contours.into_iter().skip(1).collect::<Vec<_>>();
    holes.sort();
    let key = OffsetFaceKey { outer, holes };
    Some(OffsetFaceCandidate {
        key,
        visual_area: face.visual_area,
        eligibility: OffsetOperandEligibility::from_reasons(reasons),
        hit_contours: face.contours.clone(),
    })
}

#[allow(clippy::too_many_lines)]
fn build_contour_key(
    contour: &VisualProfileContour,
    spans: &BTreeMap<CurveSpan, &OffsetSpanCandidate>,
    adjacencies: &BTreeSet<[OffsetEndpointRef; 2]>,
) -> Option<(OffsetContourKey, Vec<OffsetOperandIneligibility>)> {
    let first = contour.edges.first()?;
    let mut reasons = Vec::new();

    if contour
        .edges
        .iter()
        .all(|edge| edge.source_span == first.source_span)
        && spans
            .get(&first.source_span)
            .is_some_and(|candidate| candidate.family == OffsetOperandCurveFamily::Circle)
    {
        let candidate = spans[&first.source_span];
        reasons.extend(candidate.eligibility.reasons());
        let directions = contour
            .edges
            .iter()
            .filter_map(|edge| traversal(edge.source_parameters))
            .collect::<Vec<_>>();
        let covered_parameter_length = contour
            .edges
            .iter()
            .map(|edge| (edge.source_parameters[1] - edge.source_parameters[0]).abs())
            .sum::<f64>();
        let period_tolerance = std::f64::consts::TAU * 256.0 * f64::EPSILON;
        let complete = directions.len() == contour.edges.len()
            && directions.windows(2).all(|pair| pair[0] == pair[1])
            && contour.edges.len() == 2
            && (covered_parameter_length - std::f64::consts::TAU).abs() <= period_tolerance;
        if !complete {
            reasons.push(OffsetOperandIneligibility::ArrangementDerivedFragment);
        }
        let traversal = directions
            .first()
            .copied()
            .unwrap_or(OffsetTraversal::Forward);
        return Some((
            OffsetContourKey {
                spans: vec![OffsetDirectedSpan {
                    span: first.source_span,
                    traversal,
                }],
            },
            reasons,
        ));
    }

    let mut directed = Vec::new();
    let mut seen = BTreeSet::new();
    for edge in &contour.edges {
        let Some(candidate) = spans.get(&edge.source_span).copied() else {
            reasons.push(OffsetOperandIneligibility::ArrangementDerivedFragment);
            continue;
        };
        reasons.extend(candidate.eligibility.reasons());
        if !seen.insert(edge.source_span) {
            reasons.push(OffsetOperandIneligibility::ArrangementDerivedFragment);
        }
        let Some(edge_traversal) = traversal(edge.source_parameters) else {
            reasons.push(OffsetOperandIneligibility::ArrangementDerivedFragment);
            continue;
        };
        if candidate.family == OffsetOperandCurveFamily::Circle
            || !is_complete_bounded_interval(edge.source_parameters)
        {
            reasons.push(OffsetOperandIneligibility::ArrangementDerivedFragment);
        }
        directed.push(OffsetDirectedSpan {
            span: edge.source_span,
            traversal: edge_traversal,
        });
    }
    if directed.is_empty() {
        return None;
    }
    for (current, next) in directed
        .iter()
        .zip(directed.iter().cycle().skip(1))
        .take(directed.len())
    {
        let Some(current_candidate) = spans.get(&current.span).copied() else {
            continue;
        };
        let Some(next_candidate) = spans.get(&next.span).copied() else {
            continue;
        };
        if !matches!(
            current_candidate.family,
            OffsetOperandCurveFamily::Line | OffsetOperandCurveFamily::CircularArc
        ) || !matches!(
            next_candidate.family,
            OffsetOperandCurveFamily::Line | OffsetOperandCurveFamily::CircularArc
        ) {
            continue;
        }
        let current_end = OffsetEndpointRef {
            span: current.span,
            endpoint: match current.traversal {
                OffsetTraversal::Forward => OffsetEndpointRole::End,
                OffsetTraversal::Reverse => OffsetEndpointRole::Start,
            },
        };
        let next_start = OffsetEndpointRef {
            span: next.span,
            endpoint: match next.traversal {
                OffsetTraversal::Forward => OffsetEndpointRole::Start,
                OffsetTraversal::Reverse => OffsetEndpointRole::End,
            },
        };
        if !adjacencies.contains(&canonical_endpoint_pair(current_end, next_start)) {
            reasons.push(OffsetOperandIneligibility::UnownedEndpointJoin);
        }
    }
    canonicalize_contour_rotation(&mut directed);
    Some((OffsetContourKey { spans: directed }, reasons))
}

fn traversal(parameters: [f64; 2]) -> Option<OffsetTraversal> {
    match parameters[0].total_cmp(&parameters[1]) {
        Ordering::Less => Some(OffsetTraversal::Forward),
        Ordering::Greater => Some(OffsetTraversal::Reverse),
        Ordering::Equal => None,
    }
}

fn is_complete_bounded_interval(parameters: [f64; 2]) -> bool {
    (parameters[0].to_bits() == 0.0_f64.to_bits() && parameters[1].to_bits() == 1.0_f64.to_bits())
        || (parameters[0].to_bits() == 1.0_f64.to_bits()
            && parameters[1].to_bits() == 0.0_f64.to_bits())
}

fn canonical_endpoint_pair(
    first: OffsetEndpointRef,
    second: OffsetEndpointRef,
) -> [OffsetEndpointRef; 2] {
    if first < second {
        [first, second]
    } else {
        [second, first]
    }
}

fn canonicalize_contour_rotation(spans: &mut [OffsetDirectedSpan]) {
    if spans.len() < 2 {
        return;
    }
    let mut best = 0;
    for candidate in 1..spans.len() {
        let comparison = (0..spans.len())
            .map(|offset| {
                spans[(candidate + offset) % spans.len()].cmp(&spans[(best + offset) % spans.len()])
            })
            .find(|comparison| *comparison != Ordering::Equal)
            .unwrap_or(Ordering::Equal);
        if comparison == Ordering::Less {
            best = candidate;
        }
    }
    spans.rotate_left(best);
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FaceContainment {
    Inside,
    Outside,
    Boundary,
    Unavailable,
}

fn face_containment(
    document: &SketchDocument,
    candidate: &OffsetFaceCandidate,
    point: [f64; 2],
) -> FaceContainment {
    let tolerance = model_tolerance(document, point);
    for contour in &candidate.hit_contours {
        for edge in &contour.edges {
            let Some(on_boundary) = point_on_edge(document, edge, point, tolerance) else {
                return FaceContainment::Unavailable;
            };
            if on_boundary {
                return FaceContainment::Boundary;
            }
        }
    }
    let Some(outer) = candidate.hit_contours.first() else {
        return FaceContainment::Unavailable;
    };
    let Some(inside_outer) = point_in_contour(document, outer, point, tolerance) else {
        return FaceContainment::Unavailable;
    };
    if !inside_outer {
        return FaceContainment::Outside;
    }
    for hole in candidate.hit_contours.iter().skip(1) {
        let Some(inside_hole) = point_in_contour(document, hole, point, tolerance) else {
            return FaceContainment::Unavailable;
        };
        if inside_hole {
            return FaceContainment::Outside;
        }
    }
    FaceContainment::Inside
}

fn model_tolerance(document: &SketchDocument, point: [f64; 2]) -> f64 {
    let scale = document
        .model_scale()
        .abs()
        .max(point[0].abs())
        .max(point[1].abs())
        .max(1.0);
    document.model_scale().abs() * 1.0e-9 + scale * 256.0 * f64::EPSILON
}

fn point_on_edge(
    document: &SketchDocument,
    edge: &VisualProfileEdge,
    point: [f64; 2],
    tolerance: f64,
) -> Option<bool> {
    let curve = document.curve(edge.source_span.curve)?;
    match &curve.definition {
        CurveDefinition::Line { .. } | CurveDefinition::Polyline { .. } => {
            Some(point_segment_distance(point, edge.start, edge.end) <= tolerance)
        }
        CurveDefinition::Circle { center, radius } => {
            let center = document.point(*center)?.position;
            let radius = document.scalar(*radius)?.value;
            radial_boundary(
                point,
                center,
                radius,
                edge.source_parameters,
                RadialParameterization::Circle,
                tolerance,
            )
        }
        CurveDefinition::CircularArc {
            center,
            radius,
            start_angle,
            end_angle,
            sweep,
        } => {
            let center = document.point(*center)?.position;
            let radius = document.scalar(*radius)?.value;
            let start = document.scalar(*start_angle)?.value;
            let end = document.scalar(*end_angle)?.value;
            let signed_sweep = signed_arc_sweep(start, end, *sweep)?;
            radial_boundary(
                point,
                center,
                radius,
                edge.source_parameters,
                RadialParameterization::Arc {
                    start,
                    sweep: signed_sweep,
                },
                tolerance,
            )
        }
        _ => None,
    }
}

fn point_segment_distance(point: [f64; 2], start: [f64; 2], end: [f64; 2]) -> f64 {
    let direction = [end[0] - start[0], end[1] - start[1]];
    let length_squared = direction[0].mul_add(direction[0], direction[1] * direction[1]);
    if !length_squared.is_finite() || length_squared <= 0.0 {
        return (point[0] - start[0]).hypot(point[1] - start[1]);
    }
    let displacement = [point[0] - start[0], point[1] - start[1]];
    let parameter = ((displacement[0] * direction[0] + displacement[1] * direction[1])
        / length_squared)
        .clamp(0.0, 1.0);
    let projection = [
        direction[0].mul_add(parameter, start[0]),
        direction[1].mul_add(parameter, start[1]),
    ];
    (point[0] - projection[0]).hypot(point[1] - projection[1])
}

#[derive(Clone, Copy, Debug)]
enum RadialParameterization {
    Circle,
    Arc { start: f64, sweep: f64 },
}

fn radial_boundary(
    point: [f64; 2],
    center: [f64; 2],
    radius: f64,
    interval: [f64; 2],
    parameterization: RadialParameterization,
    tolerance: f64,
) -> Option<bool> {
    if !radius.is_finite() || radius <= 0.0 {
        return None;
    }
    let displacement = [point[0] - center[0], point[1] - center[1]];
    let distance = displacement[0].hypot(displacement[1]);
    if (distance - radius).abs() > tolerance {
        return Some(false);
    }
    let angle = displacement[1]
        .atan2(displacement[0])
        .rem_euclid(std::f64::consts::TAU);
    Some(
        parameter_for_angle(parameterization, angle)
            .into_iter()
            .any(|parameter| parameter_in_closed_interval(parameter, interval)),
    )
}

fn point_in_contour(
    document: &SketchDocument,
    contour: &VisualProfileContour,
    point: [f64; 2],
    tolerance: f64,
) -> Option<bool> {
    let mut ray_y = point[1];
    for _ in 0..8 {
        if contour.edges.iter().any(|edge| {
            (edge.start[1] - ray_y).abs() <= tolerance || (edge.end[1] - ray_y).abs() <= tolerance
        }) {
            ray_y += 4.0 * tolerance;
        } else {
            break;
        }
    }
    let mut crossings = 0_usize;
    for edge in &contour.edges {
        crossings = crossings.checked_add(edge_ray_crossings(
            document, edge, point[0], ray_y, tolerance,
        )?)?;
    }
    Some(!crossings.is_multiple_of(2))
}

fn edge_ray_crossings(
    document: &SketchDocument,
    edge: &VisualProfileEdge,
    point_x: f64,
    ray_y: f64,
    tolerance: f64,
) -> Option<usize> {
    let curve = document.curve(edge.source_span.curve)?;
    match &curve.definition {
        CurveDefinition::Line { .. } | CurveDefinition::Polyline { .. } => {
            let crosses = (edge.start[1] > ray_y) != (edge.end[1] > ray_y);
            if !crosses {
                return Some(0);
            }
            let parameter = (ray_y - edge.start[1]) / (edge.end[1] - edge.start[1]);
            let intersection_x = (edge.end[0] - edge.start[0]).mul_add(parameter, edge.start[0]);
            Some(usize::from(intersection_x > point_x + tolerance))
        }
        CurveDefinition::Circle { center, radius } => radial_ray_crossings(
            document.point(*center)?.position,
            document.scalar(*radius)?.value,
            edge.source_parameters,
            RadialParameterization::Circle,
            point_x,
            ray_y,
            tolerance,
        ),
        CurveDefinition::CircularArc {
            center,
            radius,
            start_angle,
            end_angle,
            sweep,
        } => {
            let start = document.scalar(*start_angle)?.value;
            let end = document.scalar(*end_angle)?.value;
            radial_ray_crossings(
                document.point(*center)?.position,
                document.scalar(*radius)?.value,
                edge.source_parameters,
                RadialParameterization::Arc {
                    start,
                    sweep: signed_arc_sweep(start, end, *sweep)?,
                },
                point_x,
                ray_y,
                tolerance,
            )
        }
        _ => None,
    }
}

fn radial_ray_crossings(
    center: [f64; 2],
    radius: f64,
    interval: [f64; 2],
    parameterization: RadialParameterization,
    point_x: f64,
    ray_y: f64,
    tolerance: f64,
) -> Option<usize> {
    if !radius.is_finite() || radius <= 0.0 {
        return None;
    }
    let sine = (ray_y - center[1]) / radius;
    if !sine.is_finite() || sine.abs() >= 1.0 {
        return Some(0);
    }
    let cosine = (1.0 - sine * sine).sqrt();
    let mut crossings = 0;
    for signed_cosine in [-cosine, cosine] {
        let intersection_x = signed_cosine.mul_add(radius, center[0]);
        if intersection_x <= point_x + tolerance {
            continue;
        }
        let angle = sine.atan2(signed_cosine).rem_euclid(std::f64::consts::TAU);
        if parameter_for_angle(parameterization, angle)
            .into_iter()
            .any(|parameter| parameter_in_open_interval(parameter, interval))
        {
            crossings += 1;
        }
    }
    Some(crossings)
}

fn parameter_for_angle(parameterization: RadialParameterization, angle: f64) -> Vec<f64> {
    match parameterization {
        RadialParameterization::Circle => (-2..=2)
            .map(|winding| angle + f64::from(winding) * std::f64::consts::TAU)
            .collect(),
        RadialParameterization::Arc { start, sweep } => (-2..=2)
            .map(|winding| {
                let unwrapped = angle + f64::from(winding) * std::f64::consts::TAU;
                (unwrapped - start) / sweep
            })
            .collect(),
    }
}

fn parameter_in_closed_interval(parameter: f64, interval: [f64; 2]) -> bool {
    let lower = interval[0].min(interval[1]);
    let upper = interval[0].max(interval[1]);
    let tolerance = (upper - lower).abs().max(1.0) * 256.0 * f64::EPSILON;
    parameter >= lower - tolerance && parameter <= upper + tolerance
}

fn parameter_in_open_interval(parameter: f64, interval: [f64; 2]) -> bool {
    let lower = interval[0].min(interval[1]);
    let upper = interval[0].max(interval[1]);
    let tolerance = (upper - lower).abs().max(1.0) * 256.0 * f64::EPSILON;
    parameter > lower + tolerance && parameter < upper - tolerance
}

fn signed_arc_sweep(start: f64, end: f64, sweep: DocumentArcSweep) -> Option<f64> {
    let magnitude = match sweep {
        DocumentArcSweep::CounterClockwise => (end - start).rem_euclid(std::f64::consts::TAU),
        DocumentArcSweep::Clockwise => (start - end).rem_euclid(std::f64::consts::TAU),
    };
    if !magnitude.is_finite() || magnitude == 0.0 {
        return None;
    }
    Some(match sweep {
        DocumentArcSweep::CounterClockwise => magnitude,
        DocumentArcSweep::Clockwise => -magnitude,
    })
}
