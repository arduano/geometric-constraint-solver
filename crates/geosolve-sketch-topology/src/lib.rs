// SPDX-License-Identifier: GPL-3.0-or-later

//! Read-only, revision-stamped production wire extraction over accepted sketches.
//!
//! This crate treats sketch visual-profile analysis only as bounded arrangement
//! candidate evidence. It publishes production wires only after independently
//! checking the exact accepted input stamp, declared scope and policies, complete
//! source coverage, source-parameter provenance, evaluated edge endpoints, contour
//! closure, orientation and output limits.

use std::collections::{BTreeMap, BTreeSet};

use geosolve_sketch::{
    CurveDefinition, CurveId, CurveSpan, DocumentError, DocumentExternalBindingId,
    DocumentVisibleCurveInterval, ExternalSnapshotDigest, ExternalSnapshotFeatureV1,
    ExternalSnapshotSet, GeometryRole, OperationControl, OperationOutcome, PreparedSketchInput,
    RetainedSketchDocumentSession, SketchAcceptedStateIdentity, SketchDocument,
    VisualProfileAnalysis, VisualProfileIssueKind, VisualProfileOptions, VisualProfileOrientation,
    VisualProfileStatus,
};
use thiserror::Error;

mod offset_operands;

pub use offset_operands::{
    OffsetContourKey, OffsetDirectedSpan, OffsetEndpointAdjacency, OffsetEndpointCandidate,
    OffsetEndpointEligibility, OffsetEndpointRef, OffsetEndpointRole, OffsetFaceCandidate,
    OffsetFaceKey, OffsetFaceLookup, OffsetJoinOwner, OffsetOperandConsumptionError,
    OffsetOperandCurveFamily, OffsetOperandEligibility, OffsetOperandIndex,
    OffsetOperandIneligibility, OffsetOperandRequest, OffsetOperandResult, OffsetSpanCandidate,
    OffsetTraversal, PreparedOffsetOperandQuery,
};

/// Native curve-role scope declared by one production query.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TopologyNativeGeometryScope {
    ProfileOnly,
    ProfileAndConstruction,
}

/// External snapshot geometry included by one production query.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TopologyExternalGeometryScope {
    Exclude,
    IncludeLineSegments,
}

/// Interior tangencies are rejected; only persistent/owned endpoint joins may
/// participate in a complete production result.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TopologyTangencyPolicy {
    ExplicitOwnedEndpointJoinsOnly,
}

/// Coincident carrier intervals are never guessed into one production edge.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TopologyOverlapPolicy {
    Reject,
}

/// Contours that merely touch do not establish nesting.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TopologyTouchingContourPolicy {
    Reject,
}

/// A one-sided interior junction is not silently promoted to a wire branch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TopologyTJunctionPolicy {
    Reject,
}

/// Explicit treatment of transverse self-intersections.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TopologySelfIntersectionPolicy {
    Reject,
    ResolveTransverse,
}

/// Complete production ambiguity policy evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TopologyPolicy {
    pub tangency: TopologyTangencyPolicy,
    pub overlap: TopologyOverlapPolicy,
    pub touching_contours: TopologyTouchingContourPolicy,
    pub t_junctions: TopologyTJunctionPolicy,
    pub self_intersections: TopologySelfIntersectionPolicy,
}

impl Default for TopologyPolicy {
    fn default() -> Self {
        Self {
            tangency: TopologyTangencyPolicy::ExplicitOwnedEndpointJoinsOnly,
            overlap: TopologyOverlapPolicy::Reject,
            touching_contours: TopologyTouchingContourPolicy::Reject,
            t_junctions: TopologyTJunctionPolicy::Reject,
            self_intersections: TopologySelfIntersectionPolicy::ResolveTransverse,
        }
    }
}

/// Deterministic topology evidence and output limits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TopologyLimits {
    pub max_candidate_pairs: usize,
    pub max_intersection_subdivisions: usize,
    pub max_intersection_depth: usize,
    pub max_intersection_roots: usize,
    pub max_fragments: usize,
    pub max_integration_subdivisions: usize,
    pub max_containment_tests: usize,
    pub max_wires: usize,
    pub max_regions: usize,
}

impl Default for TopologyLimits {
    fn default() -> Self {
        Self {
            max_candidate_pairs: 100_000,
            max_intersection_subdivisions: 500_000,
            max_intersection_depth: 64,
            max_intersection_roots: 100_000,
            max_fragments: 100_000,
            max_integration_subdivisions: 500_000,
            max_containment_tests: 100_000,
            max_wires: 10_000,
            max_regions: 10_000,
        }
    }
}

impl TopologyLimits {
    fn visual_options(self) -> VisualProfileOptions {
        VisualProfileOptions {
            max_candidate_pairs: self.max_candidate_pairs,
            max_intersection_subdivisions: self.max_intersection_subdivisions,
            max_intersection_depth: self.max_intersection_depth,
            max_intersection_roots: self.max_intersection_roots,
            max_fragments: self.max_fragments,
            max_integration_subdivisions: self.max_integration_subdivisions,
            max_containment_tests: self.max_containment_tests,
            max_faces: self.max_regions,
        }
    }

    fn validate(self) -> Result<(), TopologyError> {
        let values = [
            ("candidate pairs", self.max_candidate_pairs),
            (
                "intersection subdivisions",
                self.max_intersection_subdivisions,
            ),
            ("intersection depth", self.max_intersection_depth),
            ("intersection roots", self.max_intersection_roots),
            ("fragments", self.max_fragments),
            (
                "integration subdivisions",
                self.max_integration_subdivisions,
            ),
            ("containment tests", self.max_containment_tests),
            ("wires", self.max_wires),
            ("regions", self.max_regions),
        ];
        if let Some((field, _)) = values.into_iter().find(|(_, value)| *value == 0) {
            return Err(TopologyError::InvalidRequest {
                field,
                message: "must be positive",
            });
        }
        Ok(())
    }
}

/// Immutable complete production-query request evidence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TopologyRequest {
    pub native_geometry: TopologyNativeGeometryScope,
    pub external_geometry: TopologyExternalGeometryScope,
    pub policy: TopologyPolicy,
    pub limits: TopologyLimits,
}

impl Default for TopologyRequest {
    fn default() -> Self {
        Self {
            native_geometry: TopologyNativeGeometryScope::ProfileOnly,
            external_geometry: TopologyExternalGeometryScope::Exclude,
            policy: TopologyPolicy::default(),
            limits: TopologyLimits::default(),
        }
    }
}

/// Immutable accepted topology input. It owns no live session or solver state.
#[derive(Clone, Debug)]
pub struct TopologySnapshot {
    input: PreparedSketchInput,
    accepted: SketchAcceptedStateIdentity,
    document: SketchDocument,
    external: ExternalSnapshotSet,
}

impl TopologySnapshot {
    /// Captures only a current independently accepted state for the current design.
    ///
    /// # Errors
    ///
    /// Rejects absent or retained older accepted geometry rather than mixing it
    /// with the current retained input stamp.
    pub fn capture(session: &RetainedSketchDocumentSession) -> Result<Self, TopologySnapshotError> {
        let accepted = session
            .accepted_state()
            .ok_or(TopologySnapshotError::AcceptedStateRequired)?;
        if accepted.design_identity() != session.design_identity() {
            return Err(TopologySnapshotError::AcceptedStateForDifferentDesign);
        }
        let input = session.prepared_input();
        if input.accepted_state_identity() != Some(accepted.identity())
            || accepted.input() != input.attempt_input()
        {
            return Err(TopologySnapshotError::AcceptedInputMismatch);
        }
        Ok(Self {
            input,
            accepted: accepted.identity(),
            document: accepted.document().clone(),
            external: session.external_snapshot_set().clone(),
        })
    }

    #[must_use]
    pub const fn input(&self) -> PreparedSketchInput {
        self.input
    }

    #[must_use]
    pub const fn accepted_state_identity(&self) -> SketchAcceptedStateIdentity {
        self.accepted
    }

    /// Prepares a worker-movable, read-only query.
    #[must_use]
    pub fn prepare(self, request: TopologyRequest) -> PreparedTopologyQuery {
        PreparedTopologyQuery {
            snapshot: self,
            request,
        }
    }
}

/// Worker-movable immutable topology query.
#[derive(Debug)]
pub struct PreparedTopologyQuery {
    snapshot: TopologySnapshot,
    request: TopologyRequest,
}

impl PreparedTopologyQuery {
    #[must_use]
    pub const fn input(&self) -> PreparedSketchInput {
        self.snapshot.input
    }

    #[must_use]
    pub const fn request(&self) -> &TopologyRequest {
        &self.request
    }

    /// Executes read-only bounded arrangement and strict production validation.
    ///
    /// Cancellation/work exhaustion are outer operation outcomes. A completed
    /// operation may still carry `Truncated` or `Skipped` topology evidence.
    ///
    /// # Errors
    ///
    /// Returns a typed request or immutable-snapshot construction error.
    pub fn execute(
        self,
        control: OperationControl,
    ) -> Result<OperationOutcome<TopologyResult>, TopologyError> {
        self.request.limits.validate()?;
        if control.token.is_cancelled() {
            let stopped = self
                .snapshot
                .document
                .analyze_visual_profiles_controlled(self.request.limits.visual_options(), control);
            return Ok(stopped.map(|_| unreachable!("pre-cancelled analysis cannot complete")));
        }
        let scope = PreparedScope::new(
            &self.snapshot.document,
            &self.snapshot.external,
            &self.request,
        )?;
        let analysis = scope
            .document
            .analyze_visual_profiles_controlled(self.request.limits.visual_options(), control);
        let input = self.snapshot.input;
        let accepted = self.snapshot.accepted;
        let request = self.request;
        Ok(analysis.map(move |analysis| build_result(&input, accepted, request, scope, &analysis)))
    }
}

/// Typed inability to capture a coherent accepted query input.
#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum TopologySnapshotError {
    #[error("production topology requires an independently accepted state")]
    AcceptedStateRequired,
    #[error("retained accepted topology belongs to an older design")]
    AcceptedStateForDifferentDesign,
    #[error("accepted topology input does not match the current complete input stamp")]
    AcceptedInputMismatch,
}

/// Query setup failure before bounded topology evidence exists.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum TopologyError {
    #[error("invalid {field}: {message}")]
    InvalidRequest {
        field: &'static str,
        message: &'static str,
    },
    #[error(transparent)]
    Document(#[from] DocumentError),
}

/// Production completeness is independent of outer cancellation/work exhaustion.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TopologyCompleteness {
    Complete,
    Truncated,
    Skipped,
}

/// Configured and consumed bounded production work.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TopologyBudgetCounter {
    pub limit: usize,
    pub consumed: usize,
}

/// Complete bounded topology-work evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TopologyBudgetReport {
    pub candidate_pairs: TopologyBudgetCounter,
    pub intersection_subdivisions: TopologyBudgetCounter,
    pub intersection_roots: TopologyBudgetCounter,
    pub fragments: TopologyBudgetCounter,
    pub integration_subdivisions: TopologyBudgetCounter,
    pub containment_tests: TopologyBudgetCounter,
    pub regions: TopologyBudgetCounter,
}

/// Stable source identity independent of query-local temporary curves.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum TopologySourceRef {
    Native(CurveSpan),
    External(DocumentExternalBindingId),
}

/// Exact source and interval provenance for one directed production fragment.
#[derive(Clone, Debug, PartialEq)]
pub enum TopologySourceProvenance {
    Native {
        support: CurveSpan,
        visible_interval: DocumentVisibleCurveInterval,
    },
    ExternalLine {
        binding: DocumentExternalBindingId,
        source_revision: u64,
        source_digest: ExternalSnapshotDigest,
        domain: [f64; 2],
    },
}

impl TopologySourceProvenance {
    #[must_use]
    pub const fn source_ref(&self) -> TopologySourceRef {
        match self {
            Self::Native { support, .. } => TopologySourceRef::Native(*support),
            Self::ExternalLine { binding, .. } => TopologySourceRef::External(*binding),
        }
    }
}

/// One eligible interval in the declared and accepted query scope.
#[derive(Clone, Debug, PartialEq)]
pub struct TopologyEligibleSource {
    pub source: TopologySourceProvenance,
    pub parameters: [f64; 2],
}

/// Exact geometry-scope evidence for complete or incomplete output.
#[derive(Clone, Debug, PartialEq)]
pub struct TopologyScopeEvidence {
    pub native_curves: Vec<CurveId>,
    pub external_lines: Vec<DocumentExternalBindingId>,
    pub ignored_external_points: Vec<DocumentExternalBindingId>,
    pub eligible_sources: Vec<TopologyEligibleSource>,
}

/// Typed reason production wires were withheld.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum TopologyIssueKind {
    CandidateBudgetExceeded,
    CandidateAnalysisSkipped,
    TangencyRejected,
    OverlapRejected,
    TouchingContoursRejected,
    TJunctionRejected,
    SelfIntersectionRejected,
    IntersectionAmbiguous,
    SourceEvaluationFailed,
    SourceProvenanceUnavailable,
    UncoveredEligibleSource,
    InvalidWireClosure,
    InvalidWireOrientation,
    OutputWireLimitExceeded { required: usize, limit: usize },
    OutputRegionLimitExceeded { required: usize, limit: usize },
}

/// One issue with persistent/query source scope.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TopologyIssue {
    pub kind: TopologyIssueKind,
    pub affected_sources: Vec<TopologySourceRef>,
}

/// Query-local wire identity. It is not persistent topological naming.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct TopologyWireId(pub u32);

/// Query-local bounded-region identity.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct TopologyRegionId(pub u32);

/// Traversal orientation of one complete wire.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TopologyOrientation {
    CounterClockwise,
    Clockwise,
}

/// One validated directed source fragment.
#[derive(Clone, Debug, PartialEq)]
pub struct TopologyFragment {
    pub start: [f64; 2],
    pub end: [f64; 2],
    pub source: TopologySourceProvenance,
    pub source_parameters: [f64; 2],
    pub source_parameter_enclosures: [[f64; 2]; 2],
}

/// One complete oriented production wire.
#[derive(Clone, Debug, PartialEq)]
pub struct TopologyWire {
    pub id: TopologyWireId,
    pub orientation: TopologyOrientation,
    pub signed_area: f64,
    pub area_uncertainty: f64,
    pub fragments: Vec<TopologyFragment>,
}

/// One complete bounded production region with outer/hole nesting.
#[derive(Clone, Debug, PartialEq)]
pub struct TopologyRegion {
    pub id: TopologyRegionId,
    pub outer: TopologyWireId,
    pub holes: Vec<TopologyWireId>,
    pub area: f64,
    pub area_uncertainty: f64,
}

/// The only consumable production topology form.
#[derive(Clone, Debug, PartialEq)]
pub struct TopologyProductionProfile {
    input: PreparedSketchInput,
    accepted: SketchAcceptedStateIdentity,
    request: TopologyRequest,
    scope: TopologyScopeEvidence,
    wires: Vec<TopologyWire>,
    regions: Vec<TopologyRegion>,
}

impl TopologyProductionProfile {
    #[must_use]
    pub const fn input(&self) -> PreparedSketchInput {
        self.input
    }

    #[must_use]
    pub const fn accepted_state_identity(&self) -> SketchAcceptedStateIdentity {
        self.accepted
    }

    #[must_use]
    pub const fn request(&self) -> &TopologyRequest {
        &self.request
    }

    #[must_use]
    pub const fn scope(&self) -> &TopologyScopeEvidence {
        &self.scope
    }

    #[must_use]
    pub fn wires(&self) -> &[TopologyWire] {
        &self.wires
    }

    #[must_use]
    pub fn regions(&self) -> &[TopologyRegion] {
        &self.regions
    }

    /// Revalidates exact live-session provenance before host consumption.
    ///
    /// # Errors
    ///
    /// Rejects any newer design, attempt, accepted state, parameter, activation,
    /// external snapshot or policy-bearing profile mismatch.
    pub fn validate_current(
        &self,
        session: &RetainedSketchDocumentSession,
    ) -> Result<(), TopologyConsumptionError> {
        let accepted = session
            .accepted_state()
            .ok_or(TopologyConsumptionError::Stale)?;
        if session.prepared_input() != self.input
            || accepted.identity() != self.accepted
            || accepted.design_identity() != session.design_identity()
        {
            return Err(TopologyConsumptionError::Stale);
        }
        Ok(())
    }
}

/// A complete production profile is stale after any input transition.
#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum TopologyConsumptionError {
    #[error("production topology is stale for the current sketch input")]
    Stale,
}

/// Completed topology evidence. Incomplete results never carry production wires.
#[derive(Clone, Debug, PartialEq)]
pub struct TopologyResult {
    pub input: PreparedSketchInput,
    pub accepted: SketchAcceptedStateIdentity,
    pub request: TopologyRequest,
    pub completeness: TopologyCompleteness,
    pub scope: TopologyScopeEvidence,
    pub budgets: TopologyBudgetReport,
    pub issues: Vec<TopologyIssue>,
    pub production_profile: Option<TopologyProductionProfile>,
}

#[derive(Clone, Debug)]
struct PreparedSource {
    span: CurveSpan,
    internal_parameters: [f64; 2],
    provenance: TopologySourceProvenance,
}

#[derive(Debug)]
struct PreparedScope {
    document: SketchDocument,
    sources: Vec<PreparedSource>,
    evidence: TopologyScopeEvidence,
}

impl PreparedScope {
    #[allow(clippy::too_many_lines)]
    fn new(
        accepted: &SketchDocument,
        external: &ExternalSnapshotSet,
        request: &TopologyRequest,
    ) -> Result<Self, TopologyError> {
        let mut document = accepted.clone();
        let activity = accepted.effective_activity();
        let mut native_curves = Vec::new();
        let mut sources = Vec::new();
        for curve in accepted.curves() {
            if !activity.is_active(curve.id) {
                continue;
            }
            let role = accepted
                .geometry_role(curve.id)
                .expect("accepted curve has a geometry role");
            let included = role == GeometryRole::Profile
                || request.native_geometry == TopologyNativeGeometryScope::ProfileAndConstruction;
            if !included {
                continue;
            }
            native_curves.push(curve.id);
            if role == GeometryRole::Construction {
                document.set_geometry_role(curve.id, GeometryRole::Profile)?;
            }
            for interval in accepted.visible_curve_intervals(curve.id)? {
                sources.push(PreparedSource {
                    span: interval.support,
                    internal_parameters: [interval.start, interval.end],
                    provenance: TopologySourceProvenance::Native {
                        support: interval.support,
                        visible_interval: interval,
                    },
                });
            }
        }

        let mut external_lines = Vec::new();
        let mut ignored_external_points = Vec::new();
        if request.external_geometry == TopologyExternalGeometryScope::IncludeLineSegments {
            for entry in external.entries() {
                match &entry.feature {
                    ExternalSnapshotFeatureV1::Point { .. } => {
                        ignored_external_points.push(entry.binding);
                    }
                    ExternalSnapshotFeatureV1::LineSegment {
                        start, end, domain, ..
                    } => {
                        let points = [
                            document.add_point(
                                format!("topology.external.{}.start", entry.binding),
                                *start,
                            )?,
                            document.add_point(
                                format!("topology.external.{}.end", entry.binding),
                                *end,
                            )?,
                        ];
                        let direction = normalized_direction(*start, *end)?;
                        let curve = document.add_curve(
                            format!("topology.external.{}.line", entry.binding),
                            CurveDefinition::Line {
                                start: points[0],
                                end: points[1],
                                branch_direction: direction,
                            },
                        )?;
                        external_lines.push(entry.binding);
                        sources.push(PreparedSource {
                            span: CurveSpan::line(curve),
                            internal_parameters: [0.0, 1.0],
                            provenance: TopologySourceProvenance::ExternalLine {
                                binding: entry.binding,
                                source_revision: entry.source_revision,
                                source_digest: entry.source_digest,
                                domain: *domain,
                            },
                        });
                    }
                }
            }
        }
        document.validate()?;

        sources.sort_by(|first, second| {
            first
                .provenance
                .source_ref()
                .cmp(&second.provenance.source_ref())
                .then_with(|| {
                    first.internal_parameters[0].total_cmp(&second.internal_parameters[0])
                })
                .then_with(|| {
                    first.internal_parameters[1].total_cmp(&second.internal_parameters[1])
                })
        });
        native_curves.sort_unstable();
        external_lines.sort_unstable();
        ignored_external_points.sort_unstable();
        let eligible_sources = sources
            .iter()
            .map(|source| TopologyEligibleSource {
                source: source.provenance.clone(),
                parameters: map_parameters(&source.provenance, source.internal_parameters),
            })
            .collect();
        let evidence = TopologyScopeEvidence {
            native_curves,
            external_lines,
            ignored_external_points,
            eligible_sources,
        };
        Ok(Self {
            document,
            sources,
            evidence,
        })
    }

    fn source_for_edge(&self, span: CurveSpan, parameters: [f64; 2]) -> Option<&PreparedSource> {
        let lower = parameters[0].min(parameters[1]);
        let upper = parameters[0].max(parameters[1]);
        self.sources.iter().find(|source| {
            source.span == span
                && lower >= source.internal_parameters[0] - parameter_tolerance(source)
                && upper <= source.internal_parameters[1] + parameter_tolerance(source)
        })
    }

    fn source_ref(&self, span: CurveSpan) -> Option<TopologySourceRef> {
        self.sources
            .iter()
            .find(|source| source.span == span)
            .map(|source| source.provenance.source_ref())
    }
}

fn normalized_direction(first: [f64; 2], second: [f64; 2]) -> Result<[f64; 2], DocumentError> {
    let delta = [second[0] - first[0], second[1] - first[1]];
    let norm = delta[0].hypot(delta[1]);
    if !norm.is_finite() || norm <= f64::MIN_POSITIVE {
        return Err(DocumentError::InvalidField {
            field: "external topology line",
            message: "accepted external line must be finite and nondegenerate".into(),
        });
    }
    Ok([delta[0] / norm, delta[1] / norm])
}

fn map_parameters(provenance: &TopologySourceProvenance, parameters: [f64; 2]) -> [f64; 2] {
    match provenance {
        TopologySourceProvenance::Native { .. } => parameters,
        TopologySourceProvenance::ExternalLine { domain, .. } => [
            domain[0] + parameters[0] * (domain[1] - domain[0]),
            domain[0] + parameters[1] * (domain[1] - domain[0]),
        ],
    }
}

fn map_enclosures(
    provenance: &TopologySourceProvenance,
    enclosures: [[f64; 2]; 2],
) -> [[f64; 2]; 2] {
    [
        map_parameters(provenance, enclosures[0]),
        map_parameters(provenance, enclosures[1]),
    ]
}

fn parameter_tolerance(source: &PreparedSource) -> f64 {
    let width = source.internal_parameters[1] - source.internal_parameters[0];
    width.abs().max(1.0) * 256.0 * f64::EPSILON
}

#[allow(clippy::too_many_lines)]
fn build_result(
    input: &PreparedSketchInput,
    accepted: SketchAcceptedStateIdentity,
    request: TopologyRequest,
    scope: PreparedScope,
    analysis: &VisualProfileAnalysis,
) -> TopologyResult {
    let budgets = topology_budgets(analysis);
    let mut issues = map_analysis_issues(&scope, analysis);
    let mut completeness = match analysis.status {
        VisualProfileStatus::Complete => TopologyCompleteness::Complete,
        VisualProfileStatus::Truncated => TopologyCompleteness::Truncated,
        VisualProfileStatus::Skipped => TopologyCompleteness::Skipped,
    };
    if analysis.status == VisualProfileStatus::Truncated && issues.is_empty() {
        issues.push(TopologyIssue {
            kind: TopologyIssueKind::CandidateBudgetExceeded,
            affected_sources: Vec::new(),
        });
    } else if analysis.status == VisualProfileStatus::Skipped && issues.is_empty() {
        issues.push(TopologyIssue {
            kind: TopologyIssueKind::CandidateAnalysisSkipped,
            affected_sources: Vec::new(),
        });
    }

    let self_intersections = analysis
        .intersections
        .iter()
        .filter(|intersection| intersection.first_span.curve == intersection.second_span.curve)
        .collect::<Vec<_>>();
    if request.policy.self_intersections == TopologySelfIntersectionPolicy::Reject {
        for intersection in self_intersections {
            issues.push(TopologyIssue {
                kind: TopologyIssueKind::SelfIntersectionRejected,
                affected_sources: source_refs(
                    &scope,
                    [intersection.first_span, intersection.second_span],
                ),
            });
        }
    }
    for intersection in &analysis.intersections {
        if is_t_junction(&scope, intersection) {
            issues.push(TopologyIssue {
                kind: TopologyIssueKind::TJunctionRejected,
                affected_sources: source_refs(
                    &scope,
                    [intersection.first_span, intersection.second_span],
                ),
            });
        }
    }

    let (wires, regions, validation_issues) = build_and_validate_wires(&scope, analysis);
    issues.extend(validation_issues);
    if wires.len() > request.limits.max_wires {
        issues.push(TopologyIssue {
            kind: TopologyIssueKind::OutputWireLimitExceeded {
                required: wires.len(),
                limit: request.limits.max_wires,
            },
            affected_sources: Vec::new(),
        });
        completeness = TopologyCompleteness::Truncated;
    }
    if regions.len() > request.limits.max_regions {
        issues.push(TopologyIssue {
            kind: TopologyIssueKind::OutputRegionLimitExceeded {
                required: regions.len(),
                limit: request.limits.max_regions,
            },
            affected_sources: Vec::new(),
        });
        completeness = TopologyCompleteness::Truncated;
    }
    canonicalize_issues(&mut issues);
    if !issues.is_empty() && completeness == TopologyCompleteness::Complete {
        completeness = TopologyCompleteness::Skipped;
    }

    let production_profile =
        (completeness == TopologyCompleteness::Complete).then(|| TopologyProductionProfile {
            input: *input,
            accepted,
            request: request.clone(),
            scope: scope.evidence.clone(),
            wires,
            regions,
        });
    TopologyResult {
        input: *input,
        accepted,
        request,
        completeness,
        scope: scope.evidence,
        budgets,
        issues,
        production_profile,
    }
}

fn topology_budgets(analysis: &VisualProfileAnalysis) -> TopologyBudgetReport {
    let map = |counter: geosolve_sketch::VisualProfileBudgetCounter| TopologyBudgetCounter {
        limit: counter.limit,
        consumed: counter.consumed,
    };
    TopologyBudgetReport {
        candidate_pairs: map(analysis.budgets.candidate_pairs),
        intersection_subdivisions: map(analysis.budgets.intersection_subdivisions),
        intersection_roots: map(analysis.budgets.intersection_roots),
        fragments: map(analysis.budgets.fragments),
        integration_subdivisions: map(analysis.budgets.integration_subdivisions),
        containment_tests: map(analysis.budgets.containment_tests),
        regions: map(analysis.budgets.faces),
    }
}

fn map_analysis_issues(
    scope: &PreparedScope,
    analysis: &VisualProfileAnalysis,
) -> Vec<TopologyIssue> {
    analysis
        .issues
        .iter()
        .map(|issue| {
            let kind = match issue.kind {
                VisualProfileIssueKind::CandidateBudgetExceeded { .. }
                | VisualProfileIssueKind::IntersectionSubdivisionBudgetExceeded { .. }
                | VisualProfileIssueKind::IntersectionRootBudgetExceeded { .. }
                | VisualProfileIssueKind::FragmentBudgetExceeded { .. }
                | VisualProfileIssueKind::IntegrationBudgetExceeded { .. }
                | VisualProfileIssueKind::ContainmentBudgetExceeded { .. }
                | VisualProfileIssueKind::FaceBudgetExceeded { .. } => {
                    TopologyIssueKind::CandidateBudgetExceeded
                }
                VisualProfileIssueKind::CollinearOverlap { .. }
                | VisualProfileIssueKind::CurveOverlap { .. } => TopologyIssueKind::OverlapRejected,
                VisualProfileIssueKind::TangentIntersection { .. } => {
                    TopologyIssueKind::TangencyRejected
                }
                VisualProfileIssueKind::ContainmentAmbiguity { .. } => {
                    TopologyIssueKind::TouchingContoursRejected
                }
                VisualProfileIssueKind::UnresolvedIntersection { .. }
                | VisualProfileIssueKind::NumericalAmbiguity { .. }
                | VisualProfileIssueKind::UnresolvedTangentOrder { .. } => {
                    TopologyIssueKind::IntersectionAmbiguous
                }
                VisualProfileIssueKind::RationalPole { .. }
                | VisualProfileIssueKind::ZeroSpeed { .. }
                | VisualProfileIssueKind::AreaUncertainty { .. } => {
                    TopologyIssueKind::SourceEvaluationFailed
                }
                VisualProfileIssueKind::InconsistentCoincidence { .. }
                | VisualProfileIssueKind::ExplicitJoinMismatch { .. }
                | VisualProfileIssueKind::VisibleIntervalUnavailable { .. } => {
                    TopologyIssueKind::SourceProvenanceUnavailable
                }
            };
            TopologyIssue {
                kind,
                affected_sources: issue
                    .affected_spans
                    .iter()
                    .filter_map(|span| scope.source_ref(*span))
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect(),
            }
        })
        .collect()
}

fn source_refs(
    scope: &PreparedScope,
    spans: impl IntoIterator<Item = CurveSpan>,
) -> Vec<TopologySourceRef> {
    spans
        .into_iter()
        .filter_map(|span| scope.source_ref(span))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn is_t_junction(
    scope: &PreparedScope,
    intersection: &geosolve_sketch::VisualProfileIntersection,
) -> bool {
    let first_boundary = enclosure_is_boundary(
        scope,
        intersection.first_span,
        intersection.first_parameter_enclosure,
    );
    let second_boundary = enclosure_is_boundary(
        scope,
        intersection.second_span,
        intersection.second_parameter_enclosure,
    );
    first_boundary != second_boundary
}

fn enclosure_is_boundary(scope: &PreparedScope, span: CurveSpan, enclosure: [f64; 2]) -> bool {
    scope
        .sources
        .iter()
        .filter(|source| source.span == span)
        .any(|source| {
            let tolerance = parameter_tolerance(source);
            source.internal_parameters.into_iter().any(|boundary| {
                enclosure[0] - tolerance <= boundary && boundary <= enclosure[1] + tolerance
            })
        })
}

#[allow(clippy::too_many_lines)]
fn build_and_validate_wires(
    scope: &PreparedScope,
    analysis: &VisualProfileAnalysis,
) -> (Vec<TopologyWire>, Vec<TopologyRegion>, Vec<TopologyIssue>) {
    let mut wires = Vec::new();
    let mut regions = Vec::new();
    let mut issues = Vec::new();
    let mut coverage = BTreeMap::<TopologySourceRef, Vec<[f64; 2]>>::new();
    for (region_index, face) in analysis.faces.iter().enumerate() {
        let Some(region_ordinal) = u32::try_from(region_index).ok() else {
            issues.push(TopologyIssue {
                kind: TopologyIssueKind::OutputRegionLimitExceeded {
                    required: analysis.faces.len(),
                    limit: u32::MAX as usize,
                },
                affected_sources: Vec::new(),
            });
            continue;
        };
        let mut contour_ids = Vec::new();
        for contour in &face.contours {
            let Some(wire_ordinal) = u32::try_from(wires.len()).ok() else {
                issues.push(TopologyIssue {
                    kind: TopologyIssueKind::OutputWireLimitExceeded {
                        required: wires.len().saturating_add(1),
                        limit: u32::MAX as usize,
                    },
                    affected_sources: Vec::new(),
                });
                continue;
            };
            let mut fragments = Vec::new();
            for edge in &contour.edges {
                let Some(source) = scope.source_for_edge(edge.source_span, edge.source_parameters)
                else {
                    issues.push(TopologyIssue {
                        kind: TopologyIssueKind::SourceProvenanceUnavailable,
                        affected_sources: source_refs(scope, [edge.source_span]),
                    });
                    continue;
                };
                if !enclosures_contain_parameters(
                    edge.source_parameters,
                    edge.source_parameter_enclosures,
                ) || !edge_endpoints_match_source(scope, edge)
                {
                    issues.push(TopologyIssue {
                        kind: TopologyIssueKind::SourceEvaluationFailed,
                        affected_sources: vec![source.provenance.source_ref()],
                    });
                    continue;
                }
                coverage
                    .entry(source.provenance.source_ref())
                    .or_default()
                    .push([
                        edge.source_parameters[0].min(edge.source_parameters[1]),
                        edge.source_parameters[0].max(edge.source_parameters[1]),
                    ]);
                fragments.push(TopologyFragment {
                    start: edge.start,
                    end: edge.end,
                    source: source.provenance.clone(),
                    source_parameters: map_parameters(&source.provenance, edge.source_parameters),
                    source_parameter_enclosures: map_enclosures(
                        &source.provenance,
                        edge.source_parameter_enclosures,
                    ),
                });
            }
            let source_scope = fragments
                .iter()
                .map(|fragment| fragment.source.source_ref())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>();
            if !wire_is_closed(scope.document.model_scale(), &fragments) {
                issues.push(TopologyIssue {
                    kind: TopologyIssueKind::InvalidWireClosure,
                    affected_sources: source_scope.clone(),
                });
            }
            let orientation = match contour.orientation {
                VisualProfileOrientation::CounterClockwise => {
                    if !positive_area_is_certified(contour.signed_area, contour.area_uncertainty) {
                        issues.push(TopologyIssue {
                            kind: TopologyIssueKind::InvalidWireOrientation,
                            affected_sources: source_scope,
                        });
                    }
                    TopologyOrientation::CounterClockwise
                }
                VisualProfileOrientation::Clockwise => {
                    if !negative_area_is_certified(contour.signed_area, contour.area_uncertainty) {
                        issues.push(TopologyIssue {
                            kind: TopologyIssueKind::InvalidWireOrientation,
                            affected_sources: source_scope,
                        });
                    }
                    TopologyOrientation::Clockwise
                }
            };
            let id = TopologyWireId(wire_ordinal);
            contour_ids.push(id);
            wires.push(TopologyWire {
                id,
                orientation,
                signed_area: contour.signed_area,
                area_uncertainty: contour.area_uncertainty,
                fragments,
            });
        }
        let Some(outer) = contour_ids.first().copied() else {
            continue;
        };
        if !positive_area_is_certified(face.visual_area, face.area_uncertainty) {
            issues.push(TopologyIssue {
                kind: TopologyIssueKind::InvalidWireOrientation,
                affected_sources: Vec::new(),
            });
        }
        regions.push(TopologyRegion {
            id: TopologyRegionId(region_ordinal),
            outer,
            holes: contour_ids.into_iter().skip(1).collect(),
            area: face.visual_area,
            area_uncertainty: face.area_uncertainty,
        });
    }

    for source in &scope.sources {
        let ranges = coverage
            .get(&source.provenance.source_ref())
            .map_or(&[][..], Vec::as_slice);
        if !interval_is_covered(source, ranges) {
            issues.push(TopologyIssue {
                kind: TopologyIssueKind::UncoveredEligibleSource,
                affected_sources: vec![source.provenance.source_ref()],
            });
        }
    }
    (wires, regions, issues)
}

fn enclosures_contain_parameters(parameters: [f64; 2], enclosures: [[f64; 2]; 2]) -> bool {
    parameters
        .into_iter()
        .zip(enclosures)
        .all(|(value, bounds)| {
            value.is_finite()
                && bounds[0].is_finite()
                && bounds[1].is_finite()
                && bounds[0] <= value
                && value <= bounds[1]
        })
}

fn edge_endpoints_match_source(
    scope: &PreparedScope,
    edge: &geosolve_sketch::VisualProfileEdge,
) -> bool {
    let Ok(start) = scope
        .document
        .evaluate_curve_jet(edge.source_span, edge.source_parameters[0])
    else {
        return false;
    };
    let Ok(end) = scope
        .document
        .evaluate_curve_jet(edge.source_span, edge.source_parameters[1])
    else {
        return false;
    };
    position_matches(
        scope.document.model_scale(),
        edge.start,
        [start.position.x, start.position.y],
    ) && position_matches(
        scope.document.model_scale(),
        edge.end,
        [end.position.x, end.position.y],
    )
}

fn wire_is_closed(model_scale: f64, fragments: &[TopologyFragment]) -> bool {
    if fragments.is_empty() {
        return false;
    }
    fragments
        .iter()
        .zip(fragments.iter().cycle().skip(1))
        .take(fragments.len())
        .all(|(first, second)| position_matches(model_scale, first.end, second.start))
}

fn position_matches(model_scale: f64, first: [f64; 2], second: [f64; 2]) -> bool {
    first.into_iter().zip(second).all(|(first, second)| {
        let scale = first
            .abs()
            .max(second.abs())
            .max(model_scale.abs())
            .max(1.0);
        (first - second).abs() <= model_scale.abs() * 1.0e-9 + scale * 256.0 * f64::EPSILON
    })
}

fn positive_area_is_certified(area: f64, uncertainty: f64) -> bool {
    area.is_finite() && uncertainty.is_finite() && uncertainty >= 0.0 && area - uncertainty > 0.0
}

fn negative_area_is_certified(area: f64, uncertainty: f64) -> bool {
    area.is_finite() && uncertainty.is_finite() && uncertainty >= 0.0 && area + uncertainty < 0.0
}

fn interval_is_covered(source: &PreparedSource, ranges: &[[f64; 2]]) -> bool {
    let tolerance = parameter_tolerance(source);
    let mut ranges = ranges.to_vec();
    ranges.sort_by(|first, second| {
        first[0]
            .total_cmp(&second[0])
            .then(first[1].total_cmp(&second[1]))
    });
    let mut covered = source.internal_parameters[0];
    for range in ranges {
        if range[1] < covered - tolerance {
            continue;
        }
        if range[0] > covered + tolerance {
            return false;
        }
        covered = covered.max(range[1]);
        if covered >= source.internal_parameters[1] - tolerance {
            return true;
        }
    }
    covered >= source.internal_parameters[1] - tolerance
}

fn canonicalize_issues(issues: &mut Vec<TopologyIssue>) {
    for issue in issues.iter_mut() {
        issue.affected_sources.sort_unstable();
        issue.affected_sources.dedup();
    }
    issues.sort_by(|first, second| {
        issue_kind_key(&first.kind)
            .cmp(&issue_kind_key(&second.kind))
            .then(first.affected_sources.cmp(&second.affected_sources))
    });
    issues.dedup();
}

fn issue_kind_key(kind: &TopologyIssueKind) -> (u8, usize, usize) {
    match kind {
        TopologyIssueKind::CandidateBudgetExceeded => (0, 0, 0),
        TopologyIssueKind::CandidateAnalysisSkipped => (1, 0, 0),
        TopologyIssueKind::TangencyRejected => (2, 0, 0),
        TopologyIssueKind::OverlapRejected => (3, 0, 0),
        TopologyIssueKind::TouchingContoursRejected => (4, 0, 0),
        TopologyIssueKind::TJunctionRejected => (5, 0, 0),
        TopologyIssueKind::SelfIntersectionRejected => (6, 0, 0),
        TopologyIssueKind::IntersectionAmbiguous => (7, 0, 0),
        TopologyIssueKind::SourceEvaluationFailed => (8, 0, 0),
        TopologyIssueKind::SourceProvenanceUnavailable => (9, 0, 0),
        TopologyIssueKind::UncoveredEligibleSource => (10, 0, 0),
        TopologyIssueKind::InvalidWireClosure => (11, 0, 0),
        TopologyIssueKind::InvalidWireOrientation => (12, 0, 0),
        TopologyIssueKind::OutputWireLimitExceeded { required, limit } => (13, *required, *limit),
        TopologyIssueKind::OutputRegionLimitExceeded { required, limit } => (14, *required, *limit),
    }
}
