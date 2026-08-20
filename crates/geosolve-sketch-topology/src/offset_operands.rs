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
    DocumentBSplineForm, DocumentConstraintDefinition, DocumentConstraintId, EffectiveActivity,
    FeatureEndpoint, GeometryRole, OperationControl, OperationOutcome, PreparedSketchInput,
    RetainedSketchDocumentSession, SketchAcceptedStateIdentity, SketchDocument,
    VisualProfileAnalysis, VisualProfileContour, VisualProfileFace, VisualProfileIssue,
    VisualProfileOptions, VisualProfilePointContainment, VisualProfileStatus,
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
#[derive(Clone, Debug)]
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
            build_operand_result(&input, accepted, request, &document, &analysis)
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

/// Why a face or curve span cannot be used by one offset route.
///
/// `UnsupportedCurveFamily` is specific to M80's exact native route. The other reasons apply to
/// both native and computed routing.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum OffsetOperandIneligibility {
    NonProfileGeometry,
    UnsupportedCurveFamily,
    TrimmedOrPartialSpan,
    ArrangementDerivedFragment,
    UnownedEndpointJoin,
}

/// Complete typed eligibility for one operand under one route.
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
    /// Exact boundary between adjacent semantic spans of one native B-spline or NURBS curve.
    IntrinsicSpanBoundary,
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

/// One native curve-span candidate for manual chain collection under either offset route.
#[derive(Clone, Debug, PartialEq)]
pub struct OffsetSpanCandidate {
    pub span: CurveSpan,
    pub family: OffsetOperandCurveFamily,
    /// Eligibility for M80's exact native `ProfileOffset` route.
    pub eligibility: OffsetOperandEligibility,
    /// Eligibility for M82's source-only computed `CurveOffset` route.
    pub computed_eligibility: OffsetOperandEligibility,
    pub periodic: bool,
    pub endpoints: Vec<OffsetEndpointCandidate>,
}

/// One visual face projected to a stable semantic key and typed native/computed eligibility.
#[derive(Clone, Debug, PartialEq)]
pub struct OffsetFaceCandidate {
    pub key: OffsetFaceKey,
    pub visual_area: f64,
    /// Eligibility for M80's exact native `ProfileOffset` route.
    pub eligibility: OffsetOperandEligibility,
    /// Eligibility for M82's source-only computed `CurveOffset` route.
    pub computed_eligibility: OffsetOperandEligibility,
    hit_face: VisualProfileFace,
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
    containment_options: VisualProfileOptions,
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

    /// Returns the persistent owners proving exact adjacency between two endpoints.
    #[must_use]
    pub fn adjacency_owners(
        &self,
        first: OffsetEndpointRef,
        second: OffsetEndpointRef,
    ) -> Option<&[OffsetJoinOwner]> {
        let endpoints = canonical_endpoint_pair(first, second);
        self.adjacencies
            .binary_search_by_key(&endpoints, |adjacency| adjacency.endpoints)
            .ok()
            .map(|index| self.adjacencies[index].owners.as_slice())
    }

    /// Finds the smallest face containing a finite model-space point.
    ///
    /// Boundary input is never guessed to one side. Every built-in family uses the sketch-owned
    /// interval certificate retained by visual-profile analysis; an ambiguous, invalid or
    /// work-exhausted predicate fails closed for that candidate.
    #[must_use]
    pub fn face_at_point(&self, point: [f64; 2]) -> OffsetFaceLookup {
        if !point.into_iter().all(f64::is_finite) {
            return OffsetFaceLookup::None;
        }
        let mut inside = Vec::new();
        let mut boundary = Vec::new();
        for candidate in &self.faces {
            match face_containment(candidate, point, self.containment_options) {
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
    document: &SketchDocument,
    analysis: &VisualProfileAnalysis,
) -> OffsetOperandResult {
    let completeness = match analysis.status {
        VisualProfileStatus::Complete => TopologyCompleteness::Complete,
        VisualProfileStatus::Truncated => TopologyCompleteness::Truncated,
        VisualProfileStatus::Skipped => TopologyCompleteness::Skipped,
    };
    let budgets = topology_budgets(analysis);
    let issues = analysis.issues.clone();
    let operand_index = (analysis.status == VisualProfileStatus::Complete).then(|| {
        build_operand_index(
            input,
            accepted,
            document,
            analysis,
            request.limits.visual_options(),
        )
    });
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
    document: &SketchDocument,
    analysis: &VisualProfileAnalysis,
    containment_options: VisualProfileOptions,
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
            let mut common_reasons = Vec::new();
            if role != GeometryRole::Profile {
                common_reasons.push(OffsetOperandIneligibility::NonProfileGeometry);
            }
            if document.trim_views_for_span(span).next().is_some() {
                common_reasons.push(OffsetOperandIneligibility::TrimmedOrPartialSpan);
            }
            if arrangement_spans.contains(&span) {
                common_reasons.push(OffsetOperandIneligibility::ArrangementDerivedFragment);
            }
            let mut native_reasons = common_reasons.clone();
            if !family.is_exact_offset_supported() {
                native_reasons.push(OffsetOperandIneligibility::UnsupportedCurveFamily);
            }
            let periodic = matches!(
                family,
                OffsetOperandCurveFamily::Circle | OffsetOperandCurveFamily::Ellipse
            );
            let seeds = supported_endpoint_seeds(document, span, &curve.definition);
            endpoint_seeds.extend(seeds.iter().copied());
            spans.push(OffsetSpanCandidate {
                span,
                family,
                eligibility: OffsetOperandEligibility::from_reasons(native_reasons),
                computed_eligibility: OffsetOperandEligibility::from_reasons(common_reasons),
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

    let adjacencies = build_endpoint_adjacencies(document, &activity, &endpoint_seeds);
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
        containment_options,
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
    match definition {
        CurveDefinition::Line { start, end, .. }
        | CurveDefinition::RationalQuadraticConic { start, end, .. } => {
            point_endpoint_seeds(document, span, *start, *end)
        }
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
            point_endpoint_seeds(document, span, start, end)
        }
        CurveDefinition::Circle { .. } | CurveDefinition::Ellipse { .. } => Vec::new(),
        CurveDefinition::CircularArc { .. }
        | CurveDefinition::EllipticalArc { .. }
        | CurveDefinition::ParabolaSegment { .. }
        | CurveDefinition::HyperbolaSegment { .. } => {
            evaluated_endpoint_seeds(document, span, None, None)
        }
        CurveDefinition::QuadraticBezier { controls } => {
            point_endpoint_seeds(document, span, controls[0], controls[2])
        }
        CurveDefinition::CubicBezier { controls } => {
            point_endpoint_seeds(document, span, controls[0], controls[3])
        }
        CurveDefinition::BSpline {
            form,
            controls,
            span_ids,
            ..
        }
        | CurveDefinition::Nurbs {
            form,
            controls,
            span_ids,
            ..
        } => spline_endpoint_seeds(document, span, *form, controls, span_ids),
    }
}

fn point_endpoint_seeds(
    document: &SketchDocument,
    span: CurveSpan,
    start: DesignPointId,
    end: DesignPointId,
) -> Vec<EndpointSeed> {
    [
        (OffsetEndpointRole::Start, start),
        (OffsetEndpointRole::End, end),
    ]
    .into_iter()
    .filter_map(|(endpoint, point)| {
        document.point(point).map(|value| EndpointSeed {
            endpoint: OffsetEndpointRef { span, endpoint },
            position: value.position,
            point: Some(point),
        })
    })
    .collect()
}

fn evaluated_endpoint_seeds(
    document: &SketchDocument,
    span: CurveSpan,
    start_point: Option<DesignPointId>,
    end_point: Option<DesignPointId>,
) -> Vec<EndpointSeed> {
    [
        (OffsetEndpointRole::Start, 0.0, start_point),
        (OffsetEndpointRole::End, 1.0, end_point),
    ]
    .into_iter()
    .filter_map(|(endpoint, parameter, point)| {
        document
            .evaluate_curve_jet(span, parameter)
            .ok()
            .map(|jet| EndpointSeed {
                endpoint: OffsetEndpointRef { span, endpoint },
                position: [jet.position.x, jet.position.y],
                point,
            })
    })
    .collect()
}

fn spline_endpoint_seeds(
    document: &SketchDocument,
    span: CurveSpan,
    form: DocumentBSplineForm,
    controls: &[DesignPointId],
    span_ids: &[u32],
) -> Vec<EndpointSeed> {
    let Some(ordinal) = span_ids
        .iter()
        .position(|candidate| *candidate == span.segment)
    else {
        return Vec::new();
    };
    let start_point = (form == DocumentBSplineForm::Clamped && ordinal == 0).then(|| controls[0]);
    let end_point = (form == DocumentBSplineForm::Clamped && ordinal + 1 == span_ids.len())
        .then(|| *controls.last().expect("validated spline controls"));
    evaluated_endpoint_seeds(document, span, start_point, end_point)
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
    let seeded_endpoints = seeds
        .iter()
        .map(|seed| seed.endpoint)
        .collect::<BTreeSet<_>>();
    for curve in document.curves() {
        if !activity.is_active(curve.id) {
            continue;
        }
        let (form, span_ids) = match &curve.definition {
            CurveDefinition::BSpline { form, span_ids, .. }
            | CurveDefinition::Nurbs { form, span_ids, .. } => (*form, span_ids.as_slice()),
            _ => continue,
        };
        for pair in span_ids.windows(2) {
            add_intrinsic_span_link(
                &mut links,
                &seeded_endpoints,
                CurveSpan {
                    curve: curve.id,
                    segment: pair[0],
                },
                CurveSpan {
                    curve: curve.id,
                    segment: pair[1],
                },
            );
        }
        if form == DocumentBSplineForm::Periodic
            && let (Some(first), Some(last)) = (span_ids.first(), span_ids.last())
        {
            add_intrinsic_span_link(
                &mut links,
                &seeded_endpoints,
                CurveSpan {
                    curve: curve.id,
                    segment: *last,
                },
                CurveSpan {
                    curve: curve.id,
                    segment: *first,
                },
            );
        }
    }
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

fn add_intrinsic_span_link(
    links: &mut BTreeMap<[OffsetEndpointRef; 2], BTreeSet<OffsetJoinOwner>>,
    seeded_endpoints: &BTreeSet<OffsetEndpointRef>,
    incoming: CurveSpan,
    outgoing: CurveSpan,
) {
    let incoming = OffsetEndpointRef {
        span: incoming,
        endpoint: OffsetEndpointRole::End,
    };
    let outgoing = OffsetEndpointRef {
        span: outgoing,
        endpoint: OffsetEndpointRole::Start,
    };
    if seeded_endpoints.contains(&incoming) && seeded_endpoints.contains(&outgoing) {
        add_endpoint_link(
            links,
            incoming,
            outgoing,
            OffsetJoinOwner::IntrinsicSpanBoundary,
        );
    }
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
    let mut native_reasons = Vec::new();
    let mut computed_reasons = Vec::new();
    for contour in &face.contours {
        let (key, mut contour_native_reasons, mut contour_computed_reasons) =
            build_contour_key(contour, spans, adjacencies)?;
        contours.push(key);
        native_reasons.append(&mut contour_native_reasons);
        computed_reasons.append(&mut contour_computed_reasons);
    }
    let outer = contours.first()?.clone();
    let mut holes = contours.into_iter().skip(1).collect::<Vec<_>>();
    holes.sort();
    let key = OffsetFaceKey { outer, holes };
    Some(OffsetFaceCandidate {
        key,
        visual_area: face.visual_area,
        eligibility: OffsetOperandEligibility::from_reasons(native_reasons),
        computed_eligibility: OffsetOperandEligibility::from_reasons(computed_reasons),
        hit_face: face.clone(),
    })
}

#[allow(clippy::too_many_lines)]
fn build_contour_key(
    contour: &VisualProfileContour,
    spans: &BTreeMap<CurveSpan, &OffsetSpanCandidate>,
    adjacencies: &BTreeSet<[OffsetEndpointRef; 2]>,
) -> Option<(
    OffsetContourKey,
    Vec<OffsetOperandIneligibility>,
    Vec<OffsetOperandIneligibility>,
)> {
    let first = contour.edges.first()?;
    let mut native_reasons = Vec::new();
    let mut computed_reasons = Vec::new();

    if contour
        .edges
        .iter()
        .all(|edge| edge.source_span == first.source_span)
        && spans
            .get(&first.source_span)
            .is_some_and(|candidate| candidate.periodic && candidate.endpoints.is_empty())
    {
        let candidate = spans[&first.source_span];
        native_reasons.extend(candidate.eligibility.reasons());
        computed_reasons.extend(candidate.computed_eligibility.reasons());
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
            native_reasons.push(OffsetOperandIneligibility::ArrangementDerivedFragment);
            computed_reasons.push(OffsetOperandIneligibility::ArrangementDerivedFragment);
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
            native_reasons,
            computed_reasons,
        ));
    }

    let mut directed = Vec::new();
    let mut seen = BTreeSet::new();
    for edge in &contour.edges {
        let Some(candidate) = spans.get(&edge.source_span).copied() else {
            native_reasons.push(OffsetOperandIneligibility::ArrangementDerivedFragment);
            computed_reasons.push(OffsetOperandIneligibility::ArrangementDerivedFragment);
            continue;
        };
        native_reasons.extend(candidate.eligibility.reasons());
        computed_reasons.extend(candidate.computed_eligibility.reasons());
        if !seen.insert(edge.source_span) {
            native_reasons.push(OffsetOperandIneligibility::ArrangementDerivedFragment);
            computed_reasons.push(OffsetOperandIneligibility::ArrangementDerivedFragment);
        }
        let Some(edge_traversal) = traversal(edge.source_parameters) else {
            native_reasons.push(OffsetOperandIneligibility::ArrangementDerivedFragment);
            computed_reasons.push(OffsetOperandIneligibility::ArrangementDerivedFragment);
            continue;
        };
        if candidate.family == OffsetOperandCurveFamily::Circle
            || !is_complete_bounded_interval(edge.source_parameters)
        {
            native_reasons.push(OffsetOperandIneligibility::ArrangementDerivedFragment);
            computed_reasons.push(OffsetOperandIneligibility::ArrangementDerivedFragment);
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
        let native_join_requires_owner = matches!(
            current_candidate.family,
            OffsetOperandCurveFamily::Line | OffsetOperandCurveFamily::CircularArc
        ) && matches!(
            next_candidate.family,
            OffsetOperandCurveFamily::Line | OffsetOperandCurveFamily::CircularArc
        );
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
            if native_join_requires_owner {
                native_reasons.push(OffsetOperandIneligibility::UnownedEndpointJoin);
            }
            computed_reasons.push(OffsetOperandIneligibility::UnownedEndpointJoin);
        }
    }
    canonicalize_contour_rotation(&mut directed);
    Some((
        OffsetContourKey { spans: directed },
        native_reasons,
        computed_reasons,
    ))
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
    candidate: &OffsetFaceCandidate,
    point: [f64; 2],
    options: VisualProfileOptions,
) -> FaceContainment {
    let result = candidate.hit_face.classify_point(point, options);
    match result {
        Ok(VisualProfilePointContainment::Inside) => FaceContainment::Inside,
        Ok(VisualProfilePointContainment::Outside) => FaceContainment::Outside,
        Ok(VisualProfilePointContainment::Boundary) => FaceContainment::Boundary,
        Err(_) => FaceContainment::Unavailable,
    }
}
