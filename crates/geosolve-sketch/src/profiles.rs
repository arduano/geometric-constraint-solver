// SPDX-License-Identifier: GPL-3.0-or-later

mod fillet_branch;
mod interval;
mod offset;
mod pieces;

pub use fillet_branch::LineCurveFilletBranchCellError;
pub use offset::{
    CurveOffsetCertificate, CurveOffsetCubicPatch, CurveOffsetError, CurveOffsetGeometry,
    CurveOffsetOptions, CurveOffsetResult, CurveOffsetTraversal, compute_curve_offset,
    compute_curve_offset_with_controller,
};

use std::cell::RefCell;
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::rc::Rc;

use interval::{
    Interval, TAU_INTERVAL, atan2_box, atan2_point, cross_interval, next_down, next_up,
};
use pieces::{Box2, CurvePiece, PieceEvaluationError, PieceKind, piece_for_span};

use crate::{
    ContactDomain, ContactId, CurveDefinition, CurveId, CurveSpan, DesignPointId,
    DocumentBSplineForm, DocumentConstraintDefinition, DocumentConstraintId,
    DocumentFilletEndpointOrder, DocumentTrimBoundary, DocumentVisibleCurveInterval,
    EffectiveActivity, FeatureEndpoint, GeometryRole, InactivityReason, SketchDocument,
};
use geosolve_core::{
    OperationCheckpoint, OperationControl, OperationController, OperationOutcome,
    OperationWorkCounter,
};

/// Maximum half-width accepted for displayed area, relative to model-scale squared.
const AREA_DISPLAY_RELATIVE_TARGET: f64 = 1.0e-9;
const MAX_EXACT_F64_INTEGER: f64 = 9_007_199_254_740_992.0;

/// Deterministic resource limits for read-only all-family visual-profile analysis.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VisualProfileOptions {
    pub max_candidate_pairs: usize,
    pub max_intersection_subdivisions: usize,
    pub max_intersection_depth: usize,
    pub max_intersection_roots: usize,
    pub max_fragments: usize,
    pub max_integration_subdivisions: usize,
    pub max_containment_tests: usize,
    pub max_faces: usize,
}

impl Default for VisualProfileOptions {
    fn default() -> Self {
        Self {
            max_candidate_pairs: 100_000,
            max_intersection_subdivisions: 500_000,
            max_intersection_depth: 64,
            max_intersection_roots: 100_000,
            max_fragments: 100_000,
            max_integration_subdivisions: 500_000,
            max_containment_tests: 100_000,
            max_faces: 10_000,
        }
    }
}

/// The geometry set explicitly covered by one analysis report.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VisualProfileGeometryScope {
    AllBuiltInPlanarCurves,
}

/// Family role retained by the private bounded-piece kernel.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum VisualProfileCurveFamily {
    Line,
    Polyline,
    Circle,
    CircularArc,
    Ellipse,
    EllipticalArc,
    RationalQuadraticConic,
    Parabola,
    Hyperbola,
    QuadraticBezier,
    CubicBezier,
    ClampedBSpline,
    PeriodicBSpline,
    ClampedNurbs,
    PeriodicNurbs,
}

/// Configured limit and deterministic work consumed by one bounded operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VisualProfileBudgetCounter {
    pub limit: usize,
    pub consumed: usize,
}

/// Complete caller-visible resource evidence for an analysis.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VisualProfileBudgetReport {
    pub candidate_pairs: VisualProfileBudgetCounter,
    pub intersection_subdivisions: VisualProfileBudgetCounter,
    pub intersection_roots: VisualProfileBudgetCounter,
    pub fragments: VisualProfileBudgetCounter,
    pub integration_subdivisions: VisualProfileBudgetCounter,
    pub containment_tests: VisualProfileBudgetCounter,
    pub faces: VisualProfileBudgetCounter,
}

/// Completeness of one visual-only profile analysis.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VisualProfileStatus {
    Complete,
    Truncated,
    Skipped,
}

/// Why an analysis could not publish every affected component as complete.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VisualProfileIssueKind {
    CandidateBudgetExceeded {
        required: usize,
        limit: usize,
    },
    IntersectionSubdivisionBudgetExceeded {
        first: CurveSpan,
        second: CurveSpan,
        limit: usize,
    },
    IntersectionRootBudgetExceeded {
        required: usize,
        limit: usize,
    },
    FragmentBudgetExceeded {
        required: usize,
        limit: usize,
    },
    IntegrationBudgetExceeded {
        support: CurveSpan,
        limit: usize,
    },
    ContainmentBudgetExceeded {
        required: usize,
        limit: usize,
    },
    FaceBudgetExceeded {
        required: usize,
        limit: usize,
    },
    InconsistentCoincidence {
        first: DesignPointId,
        second: DesignPointId,
    },
    ExplicitJoinMismatch {
        first: CurveSpan,
        second: CurveSpan,
    },
    CollinearOverlap {
        first: CurveSpan,
        second: CurveSpan,
    },
    CurveOverlap {
        first: CurveSpan,
        second: CurveSpan,
    },
    TangentIntersection {
        first: CurveSpan,
        second: CurveSpan,
    },
    RationalPole {
        support: CurveSpan,
    },
    ZeroSpeed {
        support: CurveSpan,
    },
    UnresolvedIntersection {
        first: CurveSpan,
        second: CurveSpan,
    },
    UnresolvedTangentOrder {
        support: CurveSpan,
    },
    AreaUncertainty {
        support: CurveSpan,
    },
    ContainmentAmbiguity {
        support: CurveSpan,
    },
    NumericalAmbiguity {
        first: CurveSpan,
        second: CurveSpan,
    },
    VisibleIntervalUnavailable {
        support: CurveSpan,
    },
}

/// One typed analysis issue and all source spans in its affected component.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VisualProfileIssue {
    pub kind: VisualProfileIssueKind,
    pub affected_spans: Vec<CurveSpan>,
}

/// Orientation of a published visual contour.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VisualProfileOrientation {
    CounterClockwise,
    Clockwise,
}

/// One certified transverse arrangement root.
#[derive(Clone, Debug, PartialEq)]
pub struct VisualProfileIntersection {
    pub first_span: CurveSpan,
    pub second_span: CurveSpan,
    pub first_parameter_enclosure: [f64; 2],
    pub second_parameter_enclosure: [f64; 2],
    pub position_enclosure: [[f64; 2]; 2],
}

/// One directed contour edge with source-span and parameter-enclosure provenance.
#[derive(Clone, Debug, PartialEq)]
pub struct VisualProfileEdge {
    pub start: [f64; 2],
    pub end: [f64; 2],
    pub source_span: CurveSpan,
    pub source_parameters: [f64; 2],
    pub source_parameter_enclosures: [[f64; 2]; 2],
}

/// One ordered outer or hole contour.
#[derive(Clone, Debug, PartialEq)]
pub struct VisualProfileContour {
    pub orientation: VisualProfileOrientation,
    pub signed_area: f64,
    pub area_uncertainty: f64,
    pub edges: Vec<VisualProfileEdge>,
}

/// Certified relation of one finite point to a visual-profile face.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VisualProfilePointContainment {
    Inside,
    Outside,
    Boundary,
}

/// Why a visual-profile point query could not return a certified classification.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VisualProfilePointContainmentError {
    NonFinitePoint,
    InvalidFace,
    Uncertified { kind: VisualProfileIssueKind },
}

#[derive(Clone, Debug)]
struct CertifiedContainmentEdge {
    support: CurveSpan,
    curve: CurvePiece,
    source_parameter_enclosures: [Interval; 2],
}

/// One visual bounded face. The first contour is counterclockwise; later contours are holes.
#[derive(Clone)]
pub struct VisualProfileFace {
    pub contours: Vec<VisualProfileContour>,
    pub visual_area: f64,
    pub area_uncertainty: f64,
    containment_contours: Vec<Vec<CertifiedContainmentEdge>>,
}

impl fmt::Debug for VisualProfileFace {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VisualProfileFace")
            .field("contours", &self.contours)
            .field("visual_area", &self.visual_area)
            .field("area_uncertainty", &self.area_uncertainty)
            .finish_non_exhaustive()
    }
}

impl PartialEq for VisualProfileFace {
    fn eq(&self, other: &Self) -> bool {
        self.contours == other.contours
            && self.visual_area == other.visual_area
            && self.area_uncertainty == other.area_uncertainty
    }
}

impl VisualProfileFace {
    /// Classifies one exact finite model-space point against this certified face.
    ///
    /// The query reuses the interval ray/root certificates retained from profile analysis. It
    /// never tessellates or trusts the display endpoints as a geometric oracle.
    ///
    /// # Errors
    ///
    /// Returns a typed error for non-finite input, malformed face evidence, containment
    /// ambiguity, invalid curve evaluation, or deterministic work-budget exhaustion.
    pub fn classify_point(
        &self,
        point: [f64; 2],
        options: VisualProfileOptions,
    ) -> Result<VisualProfilePointContainment, VisualProfilePointContainmentError> {
        if !point.into_iter().all(f64::is_finite) {
            return Err(VisualProfilePointContainmentError::NonFinitePoint);
        }
        let Some(outer) = self.containment_contours.first() else {
            return Err(VisualProfilePointContainmentError::InvalidFace);
        };
        if outer.is_empty() || self.containment_contours.iter().any(Vec::is_empty) {
            return Err(VisualProfilePointContainmentError::InvalidFace);
        }
        let witness = Box2 {
            x: Interval::point(point[0]),
            y: Interval::point(point[1]),
        };
        let mut work = Work::new(options);
        let outer = point_in_certified_contour(witness, outer, &mut work)
            .map_err(|kind| VisualProfilePointContainmentError::Uncertified { kind })?;
        match outer {
            VisualProfilePointContainment::Outside => {
                return Ok(VisualProfilePointContainment::Outside);
            }
            VisualProfilePointContainment::Boundary => {
                return Ok(VisualProfilePointContainment::Boundary);
            }
            VisualProfilePointContainment::Inside => {}
        }
        for hole in self.containment_contours.iter().skip(1) {
            match point_in_certified_contour(witness, hole, &mut work)
                .map_err(|kind| VisualProfilePointContainmentError::Uncertified { kind })?
            {
                VisualProfilePointContainment::Inside => {
                    return Ok(VisualProfilePointContainment::Outside);
                }
                VisualProfilePointContainment::Boundary => {
                    return Ok(VisualProfilePointContainment::Boundary);
                }
                VisualProfilePointContainment::Outside => {}
            }
        }
        Ok(VisualProfilePointContainment::Inside)
    }
}

/// Read-only all-family visual profile result.
#[derive(Clone, Debug, PartialEq)]
pub struct VisualProfileAnalysis {
    pub scope: VisualProfileGeometryScope,
    pub status: VisualProfileStatus,
    pub families: Vec<VisualProfileCurveFamily>,
    pub faces: Vec<VisualProfileFace>,
    pub intersections: Vec<VisualProfileIntersection>,
    pub issues: Vec<VisualProfileIssue>,
    pub budgets: VisualProfileBudgetReport,
    /// Compatibility count retained from the M26 line-profile report.
    pub candidate_pairs: usize,
    /// Compatibility count retained from the M26 line-profile report.
    pub fragment_count: usize,
}

#[derive(Clone, Debug)]
struct SourcePiece {
    span: CurveSpan,
    piece_ordinal: u32,
    curve: CurvePiece,
    parameters: Interval,
    start: VertexKey,
    end: VertexKey,
    start_position: [f64; 2],
    end_position: [f64; 2],
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum VertexKey {
    Persistent(DesignPointId),
    CurveBoundary {
        curve: CurveId,
        boundary: u32,
    },
    PeriodicAnchor {
        span: CurveSpan,
        anchor: u8,
    },
    LoopAnchor {
        span: CurveSpan,
        ordinal: u32,
    },
    TrimFixed {
        span: CurveSpan,
        parameter_bits: u64,
        winding: i32,
    },
    TrimContact {
        owner: DocumentConstraintId,
        contact: ContactId,
    },
    Intersection(usize),
}

#[derive(Clone, Copy, Debug)]
struct Split {
    parameter: f64,
    enclosure: Interval,
    vertex: VertexKey,
}

#[derive(Clone, Debug)]
struct Fragment {
    start: VertexKey,
    end: VertexKey,
    start_position: [f64; 2],
    end_position: [f64; 2],
    source: usize,
    source_span: CurveSpan,
    source_parameters: [f64; 2],
    source_parameter_enclosures: [Interval; 2],
    component: usize,
}

#[derive(Clone, Copy, Debug)]
struct DirectedFragment {
    fragment: usize,
    forward: bool,
}

#[derive(Clone, Debug)]
struct Cycle {
    component: usize,
    nesting_component: usize,
    area: Interval,
    representative_area: f64,
    bounds: Box2,
    edges: Vec<DirectedFragment>,
}

enum FaceBuildError {
    Global(VisualProfileIssueKind),
    Local {
        kind: VisualProfileIssueKind,
        components: Vec<usize>,
    },
}

type IntersectionDomains = (Interval, Interval, Vec<(f64, f64, VertexKey)>);

#[derive(Clone, Copy, Debug)]
struct CertifiedRoot {
    first_source: usize,
    second_source: usize,
    first: Interval,
    second: Interval,
    position: Box2,
    vertex: Option<VertexKey>,
}

#[derive(Clone, Copy, Debug)]
struct KrawczykResult {
    image: [Interval; 2],
    contraction_bound: f64,
}

#[derive(Clone, Debug)]
struct PairIssue {
    kind: VisualProfileIssueKind,
    first_source: usize,
    second_source: usize,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct ExplicitFilletJoin {
    first: CurveSpan,
    second: CurveSpan,
    vertex: VertexKey,
}

#[derive(Clone, Debug)]
struct Work {
    options: VisualProfileOptions,
    operation: Option<Rc<RefCell<OperationController>>>,
    candidate_pairs: usize,
    intersection_subdivisions: usize,
    intersection_roots: usize,
    fragments: usize,
    integration_subdivisions: usize,
    containment_tests: usize,
    faces: usize,
}

impl Work {
    fn new(options: VisualProfileOptions) -> Self {
        Self {
            options,
            operation: None,
            candidate_pairs: 0,
            intersection_subdivisions: 0,
            intersection_roots: 0,
            fragments: 0,
            integration_subdivisions: 0,
            containment_tests: 0,
            faces: 0,
        }
    }

    fn controlled(
        options: VisualProfileOptions,
        operation: Rc<RefCell<OperationController>>,
    ) -> Self {
        let mut work = Self::new(options);
        work.operation = Some(operation);
        work
    }

    fn charge_operation(
        &self,
        counter: OperationWorkCounter,
        amount: usize,
        checkpoint: OperationCheckpoint,
    ) -> bool {
        self.operation.as_ref().is_none_or(|operation| {
            operation
                .borrow_mut()
                .charge(counter, amount, checkpoint)
                .is_ok()
        })
    }

    fn checkpoint(&self, checkpoint: OperationCheckpoint) -> bool {
        self.operation
            .as_ref()
            .is_none_or(|operation| operation.borrow_mut().checkpoint(checkpoint).is_ok())
    }

    fn report(&self) -> VisualProfileBudgetReport {
        let counter = |limit, consumed| VisualProfileBudgetCounter { limit, consumed };
        VisualProfileBudgetReport {
            candidate_pairs: counter(self.options.max_candidate_pairs, self.candidate_pairs),
            intersection_subdivisions: counter(
                self.options.max_intersection_subdivisions,
                self.intersection_subdivisions,
            ),
            intersection_roots: counter(
                self.options.max_intersection_roots,
                self.intersection_roots,
            ),
            fragments: counter(self.options.max_fragments, self.fragments),
            integration_subdivisions: counter(
                self.options.max_integration_subdivisions,
                self.integration_subdivisions,
            ),
            containment_tests: counter(self.options.max_containment_tests, self.containment_tests),
            faces: counter(self.options.max_faces, self.faces),
        }
    }

    fn charge_root(&mut self) -> Result<(), VisualProfileIssueKind> {
        if !self.charge_operation(
            OperationWorkCounter::ProfileRoots,
            1,
            OperationCheckpoint::ProfileSubdivision,
        ) {
            return Err(VisualProfileIssueKind::IntersectionRootBudgetExceeded {
                required: self.intersection_roots.saturating_add(1),
                limit: self.options.max_intersection_roots,
            });
        }
        let required = self.intersection_roots.checked_add(1).ok_or(
            VisualProfileIssueKind::IntersectionRootBudgetExceeded {
                required: usize::MAX,
                limit: self.options.max_intersection_roots,
            },
        )?;
        if required > self.options.max_intersection_roots {
            return Err(VisualProfileIssueKind::IntersectionRootBudgetExceeded {
                required,
                limit: self.options.max_intersection_roots,
            });
        }
        self.intersection_roots = required;
        Ok(())
    }
}

enum ProfileSetupError<T> {
    Interrupted,
    Analysis(T),
}

impl<T> From<T> for ProfileSetupError<T> {
    fn from(value: T) -> Self {
        Self::Analysis(value)
    }
}

#[derive(Clone, Debug)]
struct DisjointSet {
    parent: Vec<usize>,
}

impl DisjointSet {
    fn new(len: usize) -> Self {
        Self {
            parent: (0..len).collect(),
        }
    }

    fn root(&mut self, value: usize) -> usize {
        let mut root = value;
        while self.parent[root] != root {
            root = self.parent[root];
        }
        let mut current = value;
        while self.parent[current] != current {
            let parent = self.parent[current];
            self.parent[current] = root;
            current = parent;
        }
        root
    }

    fn union(&mut self, first: usize, second: usize) {
        let first = self.root(first);
        let second = self.root(second);
        if first != second {
            let (root, child) = if first < second {
                (first, second)
            } else {
                (second, first)
            };
            self.parent[child] = root;
        }
    }
}

impl SketchDocument {
    /// Extracts deterministic visual-only bounded faces from every accepted built-in curve family.
    ///
    /// The operation is equation-free and persistence-neutral. It uses explicit point,
    /// coincidence/contact, intrinsic spline, and fillet-owner topology only. Rendering samples
    /// and coordinate proximity never establish arrangement topology.
    #[must_use]
    pub fn analyze_visual_profiles(&self, options: VisualProfileOptions) -> VisualProfileAnalysis {
        analyze_visual_profiles(self, options)
    }

    /// Runs visual profile analysis under cooperative operation control.
    ///
    /// Cancellation and operation-limit exhaustion are outer outcomes and can
    /// therefore never be confused with `Complete`, `Truncated`, or `Skipped`.
    pub fn analyze_visual_profiles_controlled(
        &self,
        options: VisualProfileOptions,
        control: OperationControl,
    ) -> OperationOutcome<VisualProfileAnalysis> {
        let operation = Rc::new(RefCell::new(OperationController::new(control)));
        if operation
            .borrow_mut()
            .checkpoint(OperationCheckpoint::ProfileCandidate)
            .is_err()
        {
            return operation.borrow().outcome_unchecked();
        }
        let analysis =
            analyze_visual_profiles_with_work(self, Work::controlled(options, operation.clone()));
        operation.borrow().outcome(analysis)
    }
}

#[allow(clippy::too_many_lines)]
fn analyze_visual_profiles(
    document: &SketchDocument,
    options: VisualProfileOptions,
) -> VisualProfileAnalysis {
    analyze_visual_profiles_with_work(document, Work::new(options))
}

#[allow(clippy::too_many_lines)]
fn analyze_visual_profiles_with_work(
    document: &SketchDocument,
    mut work: Work,
) -> VisualProfileAnalysis {
    let options = work.options;
    let activity = document.effective_activity();
    let welded = match welded_points(document, &activity, &work) {
        Ok(value) => value,
        Err(ProfileSetupError::Analysis((first, second))) => {
            return skipped_analysis(
                &work,
                VisualProfileIssueKind::InconsistentCoincidence { first, second },
                Vec::new(),
            );
        }
        Err(ProfileSetupError::Interrupted) => return interrupted_profile_analysis(&work),
    };
    let mut sources = match source_pieces(document, &activity, &welded, &work) {
        Ok(value) => value,
        Err(ProfileSetupError::Analysis((support, kind))) => {
            return skipped_analysis(&work, kind, vec![support]);
        }
        Err(ProfileSetupError::Interrupted) => return interrupted_profile_analysis(&work),
    };
    let families = sources
        .iter()
        .map(|source| source.curve.family)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();

    let explicit_fillet_joins =
        match apply_explicit_joins(document, &activity, &welded, &mut sources, &work) {
            Ok(value) => value,
            Err(ProfileSetupError::Analysis((kind, affected))) => {
                return skipped_analysis(&work, kind, affected);
            }
            Err(ProfileSetupError::Interrupted) => return interrupted_profile_analysis(&work),
        };

    for source in &sources {
        if !work.checkpoint(OperationCheckpoint::ProfileSubdivision) {
            return interrupted_profile_analysis(&work);
        }
        if let Err(kind) = certify_piece_domain(source, &mut work) {
            return skipped_analysis(&work, kind, vec![source.span]);
        }
    }
    // Every pair test starts with the same complete-domain enclosure. Cache those certified
    // boxes once: adaptive Offset output can contain hundreds of cubic patches, and recomputing
    // identical interval hulls for every O(n^2) candidate otherwise dominates an interaction
    // even though nearly all ordered-neighbourhood boxes are immediately disjoint.
    let source_bounds = match sources
        .iter()
        .map(|source| {
            source.curve.position(source.parameters).map_err(|error| {
                (
                    source.span,
                    evaluation_issue(error, source.span, source.span),
                )
            })
        })
        .collect::<Result<Vec<_>, _>>()
    {
        Ok(bounds) => bounds,
        Err((span, kind)) => return skipped_analysis(&work, kind, vec![span]),
    };

    let self_candidates = sources
        .iter()
        .filter(|source| source.curve.may_self_intersect())
        .count();
    let candidate_pairs =
        candidate_pair_count(sources.len()).and_then(|value| value.checked_add(self_candidates));
    let Some(candidate_pairs) = candidate_pairs else {
        return skipped_analysis(
            &work,
            VisualProfileIssueKind::CandidateBudgetExceeded {
                required: usize::MAX,
                limit: options.max_candidate_pairs,
            },
            Vec::new(),
        );
    };
    if candidate_pairs > options.max_candidate_pairs {
        return skipped_analysis(
            &work,
            VisualProfileIssueKind::CandidateBudgetExceeded {
                required: candidate_pairs,
                limit: options.max_candidate_pairs,
            },
            Vec::new(),
        );
    }
    if !work.charge_operation(
        OperationWorkCounter::ProfileCandidatePairs,
        candidate_pairs,
        OperationCheckpoint::ProfileCandidate,
    ) {
        return interrupted_profile_analysis(&work);
    }
    work.candidate_pairs = candidate_pairs;
    let Some(overlapping_pairs) = overlapping_source_pairs(&source_bounds, &work) else {
        return interrupted_profile_analysis(&work);
    };

    let mut source_components = DisjointSet::new(sources.len());
    let mut endpoint_owner = BTreeMap::<VertexKey, usize>::new();
    for (index, source) in sources.iter().enumerate() {
        for endpoint in [source.start, source.end] {
            if let Some(owner) = endpoint_owner.insert(endpoint, index) {
                source_components.union(owner, index);
            }
        }
    }

    let mut roots = Vec::new();
    let mut pair_issues = Vec::new();
    for (first, second) in overlapping_pairs {
        if same_periodic_partition(&sources[first], &sources[second]) {
            continue;
        }
        match isolate_pair(
            &sources,
            first,
            second,
            source_bounds[first],
            source_bounds[second],
            &explicit_fillet_joins,
            &mut work,
        ) {
            Ok(pair_roots) => {
                if !pair_roots.is_empty() {
                    source_components.union(first, second);
                    roots.extend(pair_roots);
                }
            }
            Err(kind) => {
                if matches!(
                    kind,
                    VisualProfileIssueKind::IntersectionRootBudgetExceeded { .. }
                ) {
                    return skipped_analysis_with_families(
                        &work,
                        kind,
                        sources.iter().map(|source| source.span).collect(),
                        families,
                    );
                }
                source_components.union(first, second);
                pair_issues.push(PairIssue {
                    kind,
                    first_source: first,
                    second_source: second,
                });
            }
        }
    }
    for (source, piece) in sources.iter().enumerate() {
        if !piece.curve.may_self_intersect() {
            continue;
        }
        match isolate_self(piece, source, &mut work) {
            Ok(self_roots) => roots.extend(self_roots),
            Err(kind) => {
                if matches!(
                    kind,
                    VisualProfileIssueKind::IntersectionRootBudgetExceeded { .. }
                ) {
                    return skipped_analysis_with_families(
                        &work,
                        kind,
                        sources.iter().map(|source| source.span).collect(),
                        families,
                    );
                }
                pair_issues.push(PairIssue {
                    kind,
                    first_source: source,
                    second_source: source,
                });
            }
        }
    }

    let component_roots = (0..sources.len())
        .map(|index| source_components.root(index))
        .collect::<Vec<_>>();
    let mut component_spans = BTreeMap::<usize, Vec<CurveSpan>>::new();
    for (index, source) in sources.iter().enumerate() {
        component_spans
            .entry(component_roots[index])
            .or_default()
            .push(source.span);
    }
    for spans in component_spans.values_mut() {
        spans.sort_unstable();
        spans.dedup();
    }
    let mut component_bounds = BTreeMap::<usize, Box2>::new();
    for (index, source) in sources.iter().enumerate() {
        let bounds = match source.curve.position(source.parameters) {
            Ok(value) => value,
            Err(error) => {
                return skipped_analysis_with_families(
                    &work,
                    evaluation_issue(error, source.span, source.span),
                    vec![source.span],
                    families,
                );
            }
        };
        component_bounds
            .entry(component_roots[index])
            .and_modify(|value| *value = value.include(bounds))
            .or_insert(bounds);
    }
    let mut bad_components = BTreeSet::new();
    let mut issues = pair_issues
        .into_iter()
        .map(|issue| {
            let component = component_roots[issue.first_source];
            debug_assert_eq!(component, component_roots[issue.second_source]);
            bad_components.insert(component);
            VisualProfileIssue {
                kind: issue.kind,
                affected_spans: component_spans.get(&component).cloned().unwrap_or_default(),
            }
        })
        .collect::<Vec<_>>();

    let (root_vertices, intersections) = merge_roots(&sources, &roots);
    let mut splits = sources
        .iter()
        .map(|source| {
            vec![
                Split {
                    parameter: source.parameters.lower,
                    enclosure: Interval::point(source.parameters.lower),
                    vertex: source.start,
                },
                Split {
                    parameter: source.parameters.upper,
                    enclosure: Interval::point(source.parameters.upper),
                    vertex: source.end,
                },
            ]
        })
        .collect::<Vec<_>>();
    let mut vertex_positions = BTreeMap::new();
    for source in &sources {
        vertex_positions
            .entry(source.start)
            .or_insert(source.start_position);
        vertex_positions
            .entry(source.end)
            .or_insert(source.end_position);
    }
    for (index, root) in roots.iter().enumerate() {
        let vertex = root_vertices[index];
        let (first_parameter, second_parameter) = root_representative_parameters(root, &sources);
        splits[root.first_source].push(Split {
            parameter: first_parameter,
            enclosure: root.first,
            vertex,
        });
        splits[root.second_source].push(Split {
            parameter: second_parameter,
            enclosure: root.second,
            vertex,
        });
        vertex_positions
            .entry(vertex)
            .or_insert([root.position.x.midpoint(), root.position.y.midpoint()]);
    }

    let mut normalized_splits = vec![Vec::<Split>::new(); sources.len()];
    for (source_index, source) in sources.iter().enumerate() {
        let component = component_roots[source_index];
        if bad_components.contains(&component) {
            continue;
        }
        splits[source_index].sort_by(|first, second| {
            first
                .parameter
                .total_cmp(&second.parameter)
                .then_with(|| first.vertex.cmp(&second.vertex))
        });
        let mut normalized = Vec::<Split>::new();
        let mut ambiguous = false;
        for split in &splits[source_index] {
            if let Some(previous) = normalized.last_mut()
                && previous.enclosure.overlaps(split.enclosure)
            {
                if previous.vertex == split.vertex {
                    previous.enclosure = Interval {
                        lower: previous.enclosure.lower.min(split.enclosure.lower),
                        upper: previous.enclosure.upper.max(split.enclosure.upper),
                    };
                    previous.parameter = previous.enclosure.midpoint();
                    continue;
                }
                ambiguous = true;
                break;
            }
            normalized.push(*split);
        }
        if ambiguous {
            bad_components.insert(component);
            issues.push(component_issue(
                VisualProfileIssueKind::NumericalAmbiguity {
                    first: source.span,
                    second: source.span,
                },
                component,
                &component_spans,
            ));
            continue;
        }
        if normalized
            .windows(2)
            .any(|pair| pair[0].parameter >= pair[1].parameter)
        {
            bad_components.insert(component);
            issues.push(component_issue(
                VisualProfileIssueKind::NumericalAmbiguity {
                    first: source.span,
                    second: source.span,
                },
                component,
                &component_spans,
            ));
            continue;
        }
        normalized_splits[source_index] = normalized;
    }
    let required_fragments = normalized_splits
        .iter()
        .enumerate()
        .filter(|(source, _)| !bad_components.contains(&component_roots[*source]))
        .try_fold(0_usize, |required, (_, normalized)| {
            normalized.windows(2).try_fold(required, |required, pair| {
                required.checked_add(if pair[0].vertex == pair[1].vertex {
                    2
                } else {
                    1
                })
            })
        });
    let Some(required_fragments) = required_fragments else {
        return skipped_analysis_with_families(
            &work,
            VisualProfileIssueKind::FragmentBudgetExceeded {
                required: usize::MAX,
                limit: options.max_fragments,
            },
            sources.iter().map(|source| source.span).collect(),
            families,
        );
    };
    if required_fragments > options.max_fragments {
        return skipped_analysis_with_families(
            &work,
            VisualProfileIssueKind::FragmentBudgetExceeded {
                required: required_fragments,
                limit: options.max_fragments,
            },
            sources.iter().map(|source| source.span).collect(),
            families,
        );
    }
    if !work.charge_operation(
        OperationWorkCounter::ProfileFragments,
        required_fragments,
        OperationCheckpoint::ProfileSubdivision,
    ) {
        return skipped_analysis_with_families(
            &work,
            VisualProfileIssueKind::FragmentBudgetExceeded {
                required: required_fragments,
                limit: options.max_fragments,
            },
            sources.iter().map(|source| source.span).collect(),
            families,
        );
    }
    let mut fragments = Vec::with_capacity(required_fragments);
    for (source_index, normalized) in normalized_splits.iter().enumerate() {
        let source = &sources[source_index];
        let component = component_roots[source_index];
        if bad_components.contains(&component) {
            continue;
        }
        for (pair_index, pair) in normalized.windows(2).enumerate() {
            let first = pair[0];
            let second = pair[1];
            if first.vertex == second.vertex {
                let middle = first.parameter + 0.5 * (second.parameter - first.parameter);
                let anchor = VertexKey::LoopAnchor {
                    span: source.span,
                    ordinal: u32::try_from(pair_index).unwrap_or(u32::MAX),
                };
                let position = source.curve.point(middle).map_err(|_| ()).unwrap_or([
                    0.5 * (vertex_positions[&first.vertex][0]
                        + vertex_positions[&second.vertex][0]),
                    0.5 * (vertex_positions[&first.vertex][1]
                        + vertex_positions[&second.vertex][1]),
                ]);
                vertex_positions.insert(anchor, position);
                fragments.push(Fragment {
                    start: first.vertex,
                    end: anchor,
                    start_position: vertex_positions[&first.vertex],
                    end_position: position,
                    source: source_index,
                    source_span: source.span,
                    source_parameters: [first.parameter, middle],
                    source_parameter_enclosures: [first.enclosure, Interval::point(middle)],
                    component,
                });
                fragments.push(Fragment {
                    start: anchor,
                    end: second.vertex,
                    start_position: position,
                    end_position: vertex_positions[&second.vertex],
                    source: source_index,
                    source_span: source.span,
                    source_parameters: [middle, second.parameter],
                    source_parameter_enclosures: [Interval::point(middle), second.enclosure],
                    component,
                });
                continue;
            }
            fragments.push(Fragment {
                start: first.vertex,
                end: second.vertex,
                start_position: vertex_positions[&first.vertex],
                end_position: vertex_positions[&second.vertex],
                source: source_index,
                source_span: source.span,
                source_parameters: [first.parameter, second.parameter],
                source_parameter_enclosures: [first.enclosure, second.enclosure],
                component,
            });
        }
    }
    debug_assert_eq!(fragments.len(), required_fragments);
    work.fragments = fragments.len();
    fragments.sort_by(|first, second| {
        first
            .source_span
            .cmp(&second.source_span)
            .then_with(|| first.source_parameters[0].total_cmp(&second.source_parameters[0]))
            .then_with(|| first.source.cmp(&second.source))
    });

    let (mut cycles, cycle_issues) =
        extract_cycles(&fragments, &sources, document.model_scale(), &mut work);
    for (component, kind) in cycle_issues {
        bad_components.insert(component);
        issues.push(component_issue(kind, component, &component_spans));
    }
    cycles.retain(|cycle| !bad_components.contains(&cycle.component));

    let incomplete_bounds = bad_components
        .iter()
        .filter_map(|component| component_bounds.get(component).copied())
        .collect::<Vec<_>>();
    let mut containment_tainted = BTreeMap::<usize, CurveSpan>::new();
    for cycle in &cycles {
        if incomplete_bounds
            .iter()
            .any(|bounds| !bounds_clearly_disjoint(cycle.bounds, *bounds))
        {
            containment_tainted
                .entry(cycle.component)
                .or_insert_with(|| fragments[cycle.edges[0].fragment].source_span);
        }
    }
    for (component, support) in containment_tainted {
        bad_components.insert(component);
        issues.push(component_issue(
            VisualProfileIssueKind::ContainmentAmbiguity { support },
            component,
            &component_spans,
        ));
    }
    cycles.retain(|cycle| !bad_components.contains(&cycle.component));

    let mut faces = loop {
        work.faces = 0;
        match build_faces(
            &cycles,
            &fragments,
            &sources,
            &mut work,
            document.model_scale(),
            options.max_faces,
        ) {
            Ok(value) => break value,
            Err(FaceBuildError::Global(kind)) => {
                return skipped_analysis_with_families(
                    &work,
                    kind,
                    sources.iter().map(|source| source.span).collect(),
                    families,
                );
            }
            Err(FaceBuildError::Local { kind, components }) => {
                let mut changed = false;
                for component in components {
                    if bad_components.insert(component) {
                        changed = true;
                        issues.push(component_issue(kind.clone(), component, &component_spans));
                    }
                }
                loop {
                    let incomplete_bounds = bad_components
                        .iter()
                        .filter_map(|component| component_bounds.get(component).copied())
                        .collect::<Vec<_>>();
                    let newly_tainted = cycles
                        .iter()
                        .filter(|cycle| !bad_components.contains(&cycle.component))
                        .filter(|cycle| {
                            incomplete_bounds
                                .iter()
                                .any(|bounds| !bounds_clearly_disjoint(cycle.bounds, *bounds))
                        })
                        .map(|cycle| {
                            (
                                cycle.component,
                                fragments[cycle.edges[0].fragment].source_span,
                            )
                        })
                        .collect::<BTreeMap<_, _>>();
                    if newly_tainted.is_empty() {
                        break;
                    }
                    for (component, support) in newly_tainted {
                        if bad_components.insert(component) {
                            changed = true;
                            issues.push(component_issue(
                                VisualProfileIssueKind::ContainmentAmbiguity { support },
                                component,
                                &component_spans,
                            ));
                        }
                    }
                }
                if !changed {
                    return skipped_analysis_with_families(
                        &work,
                        kind,
                        sources.iter().map(|source| source.span).collect(),
                        families,
                    );
                }
                cycles.retain(|cycle| !bad_components.contains(&cycle.component));
            }
        }
    };
    let required_faces = cycles.len();
    faces.sort_by(compare_faces);
    if required_faces > options.max_faces {
        issues.push(VisualProfileIssue {
            kind: VisualProfileIssueKind::FaceBudgetExceeded {
                required: required_faces,
                limit: options.max_faces,
            },
            affected_spans: sources.iter().map(|source| source.span).collect(),
        });
    }

    issues.sort_by(|first, second| compare_issue_kinds(&first.kind, &second.kind));
    issues.dedup();
    let clean_components = component_spans
        .keys()
        .filter(|component| !bad_components.contains(component))
        .count();
    let status = if issues.is_empty() {
        VisualProfileStatus::Complete
    } else if clean_components == 0 && faces.is_empty() {
        VisualProfileStatus::Skipped
    } else {
        VisualProfileStatus::Truncated
    };
    VisualProfileAnalysis {
        scope: VisualProfileGeometryScope::AllBuiltInPlanarCurves,
        status,
        families,
        faces,
        intersections,
        issues,
        budgets: work.report(),
        candidate_pairs: work.candidate_pairs,
        fragment_count: work.fragments,
    }
}

fn root_representative_parameters(root: &CertifiedRoot, sources: &[SourcePiece]) -> (f64, f64) {
    let first = &sources[root.first_source];
    let second = &sources[root.second_source];
    if let (Some((first_start, first_direction)), Some((second_start, second_direction))) =
        (linear_geometry(first), linear_geometry(second))
    {
        let first_start = first_start.map(Interval::midpoint);
        let second_start = second_start.map(Interval::midpoint);
        let first_direction = first_direction.map(Interval::midpoint);
        let second_direction = second_direction.map(Interval::midpoint);
        let denominator = cross(first_direction, second_direction);
        if denominator != 0.0 && denominator.is_finite() {
            let displacement = subtract(second_start, first_start);
            let first_parameter = cross(displacement, second_direction) / denominator;
            let second_parameter = cross(displacement, first_direction) / denominator;
            if root.first.contains(first_parameter) && root.second.contains(second_parameter) {
                return (first_parameter, second_parameter);
            }
        }
    }
    (root.first.midpoint(), root.second.midpoint())
}

struct WeldedPoints {
    roots: BTreeMap<DesignPointId, DesignPointId>,
}

fn welded_points(
    document: &SketchDocument,
    activity: &EffectiveActivity,
    work: &Work,
) -> Result<WeldedPoints, ProfileSetupError<(DesignPointId, DesignPointId)>> {
    let mut points = document
        .points()
        .iter()
        .map(|point| point.id)
        .collect::<Vec<_>>();
    points.sort_unstable();
    let indices = points
        .iter()
        .enumerate()
        .map(|(index, point)| (*point, index))
        .collect::<BTreeMap<_, _>>();
    let mut sets = DisjointSet::new(points.len());
    for constraint in document
        .constraints()
        .iter()
        .filter(|constraint| activity.is_active(constraint.id))
    {
        if !work.checkpoint(OperationCheckpoint::ProfileCandidate) {
            return Err(ProfileSetupError::Interrupted);
        }
        if let DocumentConstraintDefinition::Coincident { first, second } = constraint.definition
            && let (Some(first), Some(second)) = (indices.get(&first), indices.get(&second))
        {
            sets.union(*first, *second);
        }
    }
    let profile_points = profile_point_ids(document, activity);
    let profile_roots = points
        .iter()
        .enumerate()
        .filter(|(_, point)| profile_points.contains(point))
        .map(|(index, _)| points[sets.root(index)])
        .collect::<BTreeSet<_>>();
    let mut roots = BTreeMap::new();
    let mut positions = BTreeMap::<DesignPointId, [f64; 2]>::new();
    let tolerance = document.model_scale() * 1.0e-9;
    for (index, point) in points.iter().copied().enumerate() {
        if !work.checkpoint(OperationCheckpoint::ProfileCandidate) {
            return Err(ProfileSetupError::Interrupted);
        }
        let root = points[sets.root(index)];
        roots.insert(point, root);
        if !profile_roots.contains(&root) {
            continue;
        }
        let position = document
            .point(point)
            .expect("point came from document")
            .position;
        let root_position = *positions.entry(root).or_insert_with(|| {
            document
                .point(root)
                .expect("point came from document")
                .position
        });
        if (position[0] - root_position[0]).abs() > tolerance
            || (position[1] - root_position[1]).abs() > tolerance
        {
            return Err((root, point).into());
        }
    }
    Ok(WeldedPoints { roots })
}

fn profile_point_ids(
    document: &SketchDocument,
    activity: &EffectiveActivity,
) -> BTreeSet<DesignPointId> {
    document
        .curves()
        .iter()
        .filter(|curve| {
            activity.is_active(curve.id)
                && document.geometry_role(curve.id) == Some(GeometryRole::Profile)
        })
        .flat_map(|curve| match &curve.definition {
            CurveDefinition::Line { start, end, .. }
            | CurveDefinition::RationalQuadraticConic { start, end, .. } => {
                vec![*start, *end]
            }
            CurveDefinition::Polyline { points, .. }
            | CurveDefinition::BSpline {
                controls: points, ..
            }
            | CurveDefinition::Nurbs {
                controls: points, ..
            } => points.clone(),
            CurveDefinition::QuadraticBezier { controls } => controls.to_vec(),
            CurveDefinition::CubicBezier { controls } => controls.to_vec(),
            CurveDefinition::Circle { .. }
            | CurveDefinition::CircularArc { .. }
            | CurveDefinition::Ellipse { .. }
            | CurveDefinition::EllipticalArc { .. }
            | CurveDefinition::ParabolaSegment { .. }
            | CurveDefinition::HyperbolaSegment { .. } => Vec::new(),
        })
        .collect()
}

#[allow(clippy::too_many_lines)]
fn source_pieces(
    document: &SketchDocument,
    activity: &EffectiveActivity,
    welded: &WeldedPoints,
    work: &Work,
) -> Result<Vec<SourcePiece>, ProfileSetupError<(CurveSpan, VisualProfileIssueKind)>> {
    let mut sources = Vec::new();
    for curve in document.curves() {
        if !activity.is_active(curve.id)
            || document.geometry_role(curve.id) != Some(GeometryRole::Profile)
        {
            continue;
        }
        if !work.checkpoint(OperationCheckpoint::ProfileCandidate) {
            return Err(ProfileSetupError::Interrupted);
        }
        let spans = document.curve_spans(curve.id).map_err(|_| {
            let support = CurveSpan::line(curve.id);
            (
                support,
                VisualProfileIssueKind::VisibleIntervalUnavailable { support },
            )
        })?;
        for (span_ordinal, span) in spans.iter().copied().enumerate() {
            if !work.checkpoint(OperationCheckpoint::ProfileCandidate) {
                return Err(ProfileSetupError::Interrupted);
            }
            let intervals = document.visible_intervals(span).map_err(|_| {
                (
                    span,
                    VisualProfileIssueKind::VisibleIntervalUnavailable { support: span },
                )
            })?;
            let curve_piece = piece_for_span(document, span).map_err(|error| {
                let kind = match error {
                    PieceEvaluationError::Pole => {
                        VisualProfileIssueKind::RationalPole { support: span }
                    }
                    PieceEvaluationError::NonFinite => {
                        VisualProfileIssueKind::VisibleIntervalUnavailable { support: span }
                    }
                };
                (span, kind)
            })?;
            for (interval_ordinal, interval) in intervals.into_iter().enumerate() {
                let interval_ordinal = u32::try_from(interval_ordinal).map_err(|_| {
                    (
                        span,
                        VisualProfileIssueKind::VisibleIntervalUnavailable { support: span },
                    )
                })?;
                let full_periodic = is_explicit_full_period(&curve.definition, interval);
                if full_periodic {
                    let middle = interval.start + 0.5 * (interval.end - interval.start);
                    let seam = VertexKey::PeriodicAnchor { span, anchor: 0 };
                    let antipodal = VertexKey::PeriodicAnchor { span, anchor: 1 };
                    for (half, (parameters, start, end)) in [
                        (Interval::hull(interval.start, middle), seam, antipodal),
                        (Interval::hull(middle, interval.end), antipodal, seam),
                    ]
                    .into_iter()
                    .enumerate()
                    {
                        let half = u32::try_from(half).map_err(|_| {
                            (
                                span,
                                VisualProfileIssueKind::VisibleIntervalUnavailable {
                                    support: span,
                                },
                            )
                        })?;
                        let piece_ordinal = interval_ordinal
                            .checked_mul(2)
                            .and_then(|value| value.checked_add(half))
                            .ok_or((
                                span,
                                VisualProfileIssueKind::VisibleIntervalUnavailable {
                                    support: span,
                                },
                            ))?;
                        sources.push(build_source_piece(
                            document,
                            span,
                            piece_ordinal,
                            curve_piece.clone(),
                            parameters,
                            start,
                            end,
                        )?);
                    }
                    continue;
                }
                let start = endpoint_key(
                    welded,
                    &curve.definition,
                    &spans,
                    span_ordinal,
                    span,
                    interval.start,
                    interval.start_boundary,
                    true,
                );
                let end = endpoint_key(
                    welded,
                    &curve.definition,
                    &spans,
                    span_ordinal,
                    span,
                    interval.end,
                    interval.end_boundary,
                    false,
                );
                sources.push(build_source_piece(
                    document,
                    span,
                    interval_ordinal,
                    curve_piece.clone(),
                    Interval::hull(interval.start, interval.end),
                    start,
                    end,
                )?);
            }
        }
    }
    sources.sort_by_key(|source| (source.span, source.piece_ordinal));
    Ok(sources)
}

fn is_explicit_full_period(
    definition: &CurveDefinition,
    interval: DocumentVisibleCurveInterval,
) -> bool {
    if !matches!(
        definition,
        CurveDefinition::Circle { .. } | CurveDefinition::Ellipse { .. }
    ) {
        return false;
    }
    match (interval.start_boundary, interval.end_boundary) {
        (DocumentTrimBoundary::Fixed(start), DocumentTrimBoundary::Fixed(end)) => {
            parameter_equal(start.parameter, end.parameter)
                && end.winding.checked_sub(start.winding) == Some(1)
        }
        _ => false,
    }
}

#[allow(clippy::too_many_arguments)]
fn endpoint_key(
    welded: &WeldedPoints,
    definition: &CurveDefinition,
    spans: &[CurveSpan],
    span_ordinal: usize,
    span: CurveSpan,
    parameter: f64,
    boundary: DocumentTrimBoundary,
    start: bool,
) -> VertexKey {
    let native: f64 = if start { 0.0 } else { 1.0 };
    if matches!(
        definition,
        CurveDefinition::Circle { .. } | CurveDefinition::Ellipse { .. }
    ) || !matches!(boundary, DocumentTrimBoundary::Fixed(_))
        || !parameter_equal(parameter, native)
    {
        return match boundary {
            DocumentTrimBoundary::Fixed(fixed) => VertexKey::TrimFixed {
                span,
                parameter_bits: fixed.parameter.to_bits(),
                winding: fixed.winding,
            },
            DocumentTrimBoundary::FilletContact { owner, contact }
            | DocumentTrimBoundary::ConstraintContact { owner, contact } => {
                VertexKey::TrimContact { owner, contact }
            }
        };
    }
    let persistent = |point| VertexKey::Persistent(welded.roots[&point]);
    match definition {
        CurveDefinition::Line {
            start: first,
            end: second,
            ..
        }
        | CurveDefinition::RationalQuadraticConic {
            start: first,
            end: second,
            ..
        } => persistent(if start { *first } else { *second }),
        CurveDefinition::Polyline { points, closed, .. } => {
            let index = span.segment as usize;
            let point = if start {
                points[index]
            } else if index + 1 == points.len() && *closed {
                points[0]
            } else {
                points[index + 1]
            };
            persistent(point)
        }
        CurveDefinition::QuadraticBezier { controls } => {
            persistent(if start { controls[0] } else { controls[2] })
        }
        CurveDefinition::CubicBezier { controls } => {
            persistent(if start { controls[0] } else { controls[3] })
        }
        CurveDefinition::BSpline { form, controls, .. }
        | CurveDefinition::Nurbs { form, controls, .. } => match form {
            DocumentBSplineForm::Clamped if start && span_ordinal == 0 => persistent(controls[0]),
            DocumentBSplineForm::Clamped if !start && span_ordinal + 1 == spans.len() => {
                persistent(*controls.last().expect("validated spline controls"))
            }
            DocumentBSplineForm::Periodic => {
                let boundary = if start {
                    span_ordinal
                } else {
                    (span_ordinal + 1) % spans.len()
                };
                VertexKey::CurveBoundary {
                    curve: span.curve,
                    boundary: u32::try_from(boundary).expect("validated spline span count"),
                }
            }
            DocumentBSplineForm::Clamped => VertexKey::CurveBoundary {
                curve: span.curve,
                boundary: u32::try_from(if start {
                    span_ordinal
                } else {
                    span_ordinal + 1
                })
                .expect("validated spline span count"),
            },
        },
        CurveDefinition::Circle { .. } | CurveDefinition::Ellipse { .. } => {
            VertexKey::PeriodicAnchor {
                span,
                anchor: u8::from(!start),
            }
        }
        CurveDefinition::CircularArc { .. }
        | CurveDefinition::EllipticalArc { .. }
        | CurveDefinition::ParabolaSegment { .. }
        | CurveDefinition::HyperbolaSegment { .. } => VertexKey::CurveBoundary {
            curve: span.curve,
            boundary: u32::from(!start),
        },
    }
}

fn build_source_piece(
    document: &SketchDocument,
    span: CurveSpan,
    piece_ordinal: u32,
    curve: CurvePiece,
    parameters: Interval,
    start: VertexKey,
    end: VertexKey,
) -> Result<SourcePiece, (CurveSpan, VisualProfileIssueKind)> {
    let evaluate = |parameter| {
        document
            .evaluate_curve_jet(span, parameter)
            .map(|jet| [jet.position.x, jet.position.y])
            .map_err(|_| {
                (
                    span,
                    VisualProfileIssueKind::VisibleIntervalUnavailable { support: span },
                )
            })
    };
    Ok(SourcePiece {
        span,
        piece_ordinal,
        curve,
        parameters,
        start,
        end,
        start_position: evaluate(parameters.lower)?,
        end_position: evaluate(parameters.upper)?,
    })
}

#[allow(clippy::too_many_lines)]
fn apply_explicit_joins(
    document: &SketchDocument,
    activity: &EffectiveActivity,
    welded: &WeldedPoints,
    sources: &mut Vec<SourcePiece>,
    work: &Work,
) -> Result<BTreeSet<ExplicitFilletJoin>, ProfileSetupError<(VisualProfileIssueKind, Vec<CurveSpan>)>>
{
    apply_explicit_contact_splits(document, activity, welded, sources, work)?;
    let join_tolerance = document.model_scale() * 1.0e-9;
    let keys = sources
        .iter()
        .flat_map(|source| [source.start, source.end])
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let key_index = keys
        .iter()
        .enumerate()
        .map(|(index, key)| (*key, index))
        .collect::<BTreeMap<_, _>>();
    let mut sets = DisjointSet::new(keys.len());
    let mut raw_fillet_joins = Vec::new();
    let mut union = |first: VertexKey, second: VertexKey| {
        if let (Some(first), Some(second)) = (key_index.get(&first), key_index.get(&second)) {
            sets.union(*first, *second);
        }
    };

    for constraint in document.constraints() {
        if !work.checkpoint(OperationCheckpoint::ProfileCandidate) {
            return Err(ProfileSetupError::Interrupted);
        }
        let frozen_suppressed_fillet = activity.reason(constraint.id)
            == Some(InactivityReason::UserSuppressed)
            && matches!(
                constraint.definition,
                DocumentConstraintDefinition::CurveCurveFillet { .. }
            );
        if !activity.is_active(constraint.id) && !frozen_suppressed_fillet {
            continue;
        }
        match constraint.definition {
            DocumentConstraintDefinition::PointOnCurve { point, contact } => {
                if let Some(curve_endpoint) = contact_endpoint(document, sources, contact) {
                    let point_endpoint = VertexKey::Persistent(welded.roots[&point]);
                    validate_join_positions(
                        sources,
                        point_endpoint,
                        curve_endpoint,
                        join_tolerance,
                    )?;
                    union(point_endpoint, curve_endpoint);
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
                    contact_endpoint(document, sources, line_contact),
                    contact_endpoint(document, sources, circle_contact),
                ) {
                    validate_join_positions(sources, first, second, join_tolerance)?;
                    union(first, second);
                }
            }
            DocumentConstraintDefinition::LineCurveTangency {
                line,
                endpoint,
                curve_contact,
            } => {
                let line_parameter = match endpoint {
                    FeatureEndpoint::Start => 0.0,
                    FeatureEndpoint::End => 1.0,
                };
                if let (Some(first), Some(second)) = (
                    source_endpoint(sources, line, line_parameter),
                    contact_endpoint(document, sources, curve_contact),
                ) {
                    validate_join_positions(sources, first, second, join_tolerance)?;
                    union(first, second);
                }
            }
            DocumentConstraintDefinition::CurveCurveFillet {
                arc,
                first_contact,
                second_contact,
                endpoint_order,
                ..
            } => {
                let ordered = match endpoint_order {
                    DocumentFilletEndpointOrder::FirstThenSecond => {
                        [(first_contact, 0.0, 0_u8), (second_contact, 1.0, 1_u8)]
                    }
                    DocumentFilletEndpointOrder::SecondThenFirst => {
                        [(first_contact, 1.0, 0_u8), (second_contact, 0.0, 1_u8)]
                    }
                };
                for (contact, arc_parameter, parent) in ordered {
                    let Some(parent_endpoint) = contact_endpoint(document, sources, contact) else {
                        let support = document.contact(contact).expect("validated contact").curve;
                        return Err((
                            VisualProfileIssueKind::ExplicitJoinMismatch {
                                first: support,
                                second: CurveSpan::line(arc),
                            },
                            vec![support, CurveSpan::line(arc)],
                        )
                            .into());
                    };
                    let arc_span = CurveSpan::line(arc);
                    let Some(arc_endpoint) = source_endpoint(sources, arc_span, arc_parameter)
                    else {
                        return Err((
                            VisualProfileIssueKind::ExplicitJoinMismatch {
                                first: document.contact(contact).expect("validated contact").curve,
                                second: arc_span,
                            },
                            vec![
                                document.contact(contact).expect("validated contact").curve,
                                arc_span,
                            ],
                        )
                            .into());
                    };
                    validate_join_positions(
                        sources,
                        parent_endpoint,
                        arc_endpoint,
                        join_tolerance,
                    )?;
                    raw_fillet_joins.push((
                        document.contact(contact).expect("validated contact").curve,
                        arc_span,
                        parent_endpoint,
                        arc_endpoint,
                    ));
                    let _ = parent;
                    union(parent_endpoint, arc_endpoint);
                }
            }
            _ => {}
        }
    }
    let canonical = keys
        .iter()
        .take(key_index.len())
        .enumerate()
        .map(|(index, _)| keys[sets.root(index)])
        .collect::<Vec<_>>();
    for source in sources {
        if !work.checkpoint(OperationCheckpoint::ProfileCandidate) {
            return Err(ProfileSetupError::Interrupted);
        }
        source.start = canonical[key_index[&source.start]];
        source.end = canonical[key_index[&source.end]];
    }
    Ok(raw_fillet_joins
        .into_iter()
        .map(|(first, second, first_vertex, second_vertex)| {
            let vertex = canonical[key_index[&first_vertex]];
            debug_assert_eq!(vertex, canonical[key_index[&second_vertex]]);
            let (first, second) = if first <= second {
                (first, second)
            } else {
                (second, first)
            };
            ExplicitFilletJoin {
                first,
                second,
                vertex,
            }
        })
        .collect())
}

fn apply_explicit_contact_splits(
    document: &SketchDocument,
    activity: &EffectiveActivity,
    welded: &WeldedPoints,
    sources: &mut Vec<SourcePiece>,
    work: &Work,
) -> Result<(), ProfileSetupError<(VisualProfileIssueKind, Vec<CurveSpan>)>> {
    let contacts = document
        .constraints()
        .iter()
        .filter(|constraint| activity.is_active(constraint.id))
        .filter_map(|constraint| match constraint.definition {
            DocumentConstraintDefinition::PointOnCurve { point, contact } => Some((point, contact)),
            _ => None,
        })
        .collect::<Vec<_>>();
    let tolerance = document.model_scale() * 1.0e-9;
    for (point, contact_id) in contacts {
        if !work.checkpoint(OperationCheckpoint::ProfileCandidate) {
            return Err(ProfileSetupError::Interrupted);
        }
        let point_vertex = VertexKey::Persistent(welded.roots[&point]);
        let Some(point_source) = sources
            .iter()
            .find(|source| source.start == point_vertex || source.end == point_vertex)
            .map(|source| source.span)
        else {
            continue;
        };
        let contact = document.contact(contact_id).expect("validated contact");
        let principal = document
            .scalar(contact.parameter)
            .expect("validated contact parameter")
            .value;
        let parameter = match contact.domain {
            ContactDomain::Periodic { period } => principal + f64::from(contact.winding) * period,
            ContactDomain::SupportingLine | ContactDomain::Bounded { .. } => principal,
        };
        let Some(source_index) = sources.iter().position(|source| {
            source.span == contact.curve
                && source.parameters.lower < parameter
                && parameter < source.parameters.upper
        }) else {
            continue;
        };
        let contact_position = document
            .evaluate_curve_jet(contact.curve, parameter)
            .map(|jet| [jet.position.x, jet.position.y])
            .map_err(|_| {
                (
                    VisualProfileIssueKind::ExplicitJoinMismatch {
                        first: point_source,
                        second: contact.curve,
                    },
                    vec![point_source, contact.curve],
                )
            })?;
        let point_position = document.point(point).expect("validated point").position;
        if (point_position[0] - contact_position[0]).abs() > tolerance
            || (point_position[1] - contact_position[1]).abs() > tolerance
        {
            return Err((
                VisualProfileIssueKind::ExplicitJoinMismatch {
                    first: point_source,
                    second: contact.curve,
                },
                vec![point_source, contact.curve],
            )
                .into());
        }

        let mut right = sources[source_index].clone();
        let next_ordinal = sources
            .iter()
            .filter(|source| source.span == contact.curve)
            .map(|source| source.piece_ordinal)
            .max()
            .and_then(|ordinal| ordinal.checked_add(1))
            .ok_or_else(|| {
                (
                    VisualProfileIssueKind::VisibleIntervalUnavailable {
                        support: contact.curve,
                    },
                    vec![contact.curve],
                )
            })?;
        sources[source_index].parameters.upper = parameter;
        sources[source_index].end = point_vertex;
        sources[source_index].end_position = contact_position;
        right.piece_ordinal = next_ordinal;
        right.parameters.lower = parameter;
        right.start = point_vertex;
        right.start_position = contact_position;
        sources.push(right);
    }
    sources.sort_by(|first, second| {
        first
            .span
            .cmp(&second.span)
            .then_with(|| first.parameters.lower.total_cmp(&second.parameters.lower))
            .then_with(|| first.piece_ordinal.cmp(&second.piece_ordinal))
    });
    Ok(())
}

fn contact_endpoint(
    document: &SketchDocument,
    sources: &[SourcePiece],
    contact: ContactId,
) -> Option<VertexKey> {
    let contact = document.contact(contact)?;
    let principal = document.scalar(contact.parameter)?.value;
    let parameter = match contact.domain {
        ContactDomain::Periodic { period } => principal + f64::from(contact.winding) * period,
        ContactDomain::SupportingLine | ContactDomain::Bounded { .. } => principal,
    };
    source_endpoint(sources, contact.curve, parameter)
}

fn source_endpoint(sources: &[SourcePiece], span: CurveSpan, parameter: f64) -> Option<VertexKey> {
    sources.iter().find_map(|source| {
        if source.span != span {
            return None;
        }
        if parameter_equal(source.parameters.lower, parameter) {
            Some(source.start)
        } else if parameter_equal(source.parameters.upper, parameter) {
            Some(source.end)
        } else {
            None
        }
    })
}

fn validate_join_positions(
    sources: &[SourcePiece],
    first: VertexKey,
    second: VertexKey,
    tolerance: f64,
) -> Result<(), (VisualProfileIssueKind, Vec<CurveSpan>)> {
    let endpoint = |key| {
        sources.iter().find_map(|source| {
            if source.start == key {
                Some((
                    source.span,
                    source
                        .curve
                        .position(Interval::point(source.parameters.lower)),
                    source.start_position,
                ))
            } else if source.end == key {
                Some((
                    source.span,
                    source
                        .curve
                        .position(Interval::point(source.parameters.upper)),
                    source.end_position,
                ))
            } else {
                None
            }
        })
    };
    let Some((first_span, first_position, first_representative)) = endpoint(first) else {
        return Ok(());
    };
    let Some((second_span, second_position, second_representative)) = endpoint(second) else {
        return Ok(());
    };
    let (Ok(first_position), Ok(second_position)) = (first_position, second_position) else {
        return Err((
            VisualProfileIssueKind::ExplicitJoinMismatch {
                first: first_span,
                second: second_span,
            },
            vec![first_span, second_span],
        ));
    };
    let interval_overlap = first_position.x.overlaps(second_position.x)
        && first_position.y.overlaps(second_position.y);
    let accepted_relation = (first_representative[0] - second_representative[0]).abs() <= tolerance
        && (first_representative[1] - second_representative[1]).abs() <= tolerance;
    if !interval_overlap && !accepted_relation {
        Err((
            VisualProfileIssueKind::ExplicitJoinMismatch {
                first: first_span,
                second: second_span,
            },
            vec![first_span, second_span],
        ))
    } else {
        Ok(())
    }
}

fn certify_piece_domain(
    source: &SourcePiece,
    work: &mut Work,
) -> Result<(), VisualProfileIssueKind> {
    let mut stack = vec![(source.parameters, 0_usize)];
    while let Some((parameter, depth)) = stack.pop() {
        match source.curve.denominator_excludes_zero(parameter) {
            Ok(true) => {}
            Ok(false) | Err(PieceEvaluationError::Pole) => {
                if !subdivide_preflight(source, parameter, depth, &mut stack, work)? {
                    return Err(VisualProfileIssueKind::RationalPole {
                        support: source.span,
                    });
                }
                continue;
            }
            Err(PieceEvaluationError::NonFinite) => {
                return Err(VisualProfileIssueKind::VisibleIntervalUnavailable {
                    support: source.span,
                });
            }
        }
        let derivative = source
            .curve
            .derivative(parameter)
            .map_err(|error| match error {
                PieceEvaluationError::Pole => VisualProfileIssueKind::RationalPole {
                    support: source.span,
                },
                PieceEvaluationError::NonFinite => VisualProfileIssueKind::ZeroSpeed {
                    support: source.span,
                },
            })?;
        if derivative[0].excludes_zero() || derivative[1].excludes_zero() {
            continue;
        }
        if !subdivide_preflight(source, parameter, depth, &mut stack, work)? {
            return Err(VisualProfileIssueKind::ZeroSpeed {
                support: source.span,
            });
        }
    }
    Ok(())
}

fn subdivide_preflight(
    source: &SourcePiece,
    parameter: Interval,
    depth: usize,
    stack: &mut Vec<(Interval, usize)>,
    work: &mut Work,
) -> Result<bool, VisualProfileIssueKind> {
    if depth >= work.options.max_intersection_depth {
        return Ok(false);
    }
    if work.intersection_subdivisions >= work.options.max_intersection_subdivisions {
        return Err(
            VisualProfileIssueKind::IntersectionSubdivisionBudgetExceeded {
                first: source.span,
                second: source.span,
                limit: work.options.max_intersection_subdivisions,
            },
        );
    }
    let middle = parameter.midpoint();
    if middle.to_bits() == parameter.lower.to_bits()
        || middle.to_bits() == parameter.upper.to_bits()
    {
        return Ok(false);
    }
    if !work.charge_operation(
        OperationWorkCounter::ProfileSubdivisions,
        1,
        OperationCheckpoint::ProfileSubdivision,
    ) {
        return Err(
            VisualProfileIssueKind::IntersectionSubdivisionBudgetExceeded {
                first: source.span,
                second: source.span,
                limit: work.options.max_intersection_subdivisions,
            },
        );
    }
    work.intersection_subdivisions += 1;
    stack.push((Interval::hull(middle, parameter.upper), depth + 1));
    stack.push((Interval::hull(parameter.lower, middle), depth + 1));
    Ok(true)
}

fn same_periodic_partition(first: &SourcePiece, second: &SourcePiece) -> bool {
    first.span == second.span
        && first.piece_ordinal != second.piece_ordinal
        && matches!(
            (&first.curve.kind, &second.curve.kind),
            (PieceKind::Circular { .. }, PieceKind::Circular { .. })
                | (PieceKind::Elliptic { .. }, PieceKind::Elliptic { .. })
        )
}

/// Returns a conservative, deterministic broad phase for source-pair intersection work.
///
/// The narrow phase deliberately treats boxes separated by no more than its round-off margin as
/// potentially touching. Expand each sweep interval by its own outward-rounded contribution to
/// that margin so the broad phase can only add candidates, never discard one the narrow phase
/// would inspect. The final sort restores the historical `(first, second)` iteration order even
/// when geometric x-order differs from persistent source order.
fn overlapping_source_pairs(bounds: &[Box2], work: &Work) -> Option<Vec<(usize, usize)>> {
    let mut sweep = bounds
        .iter()
        .copied()
        .enumerate()
        .map(|(index, bounds)| {
            debug_assert!(bounds.is_finite());
            let scale = [
                bounds.x.lower,
                bounds.x.upper,
                bounds.y.lower,
                bounds.y.upper,
            ]
            .into_iter()
            .map(f64::abs)
            .fold(1.0, f64::max);
            let margin = 256.0 * f64::EPSILON * scale;
            (
                next_down(bounds.x.lower - margin),
                next_up(bounds.x.upper + margin),
                index,
            )
        })
        .collect::<Vec<_>>();
    sweep.sort_by(|first, second| {
        first
            .0
            .total_cmp(&second.0)
            .then_with(|| first.2.cmp(&second.2))
    });

    let mut active = Vec::<(f64, usize)>::new();
    let mut pairs = Vec::new();
    for (lower, upper, current) in sweep {
        if !work.checkpoint(OperationCheckpoint::ProfileCandidate) {
            return None;
        }
        active.retain(|(active_upper, _)| *active_upper >= lower);
        for &(_, other) in &active {
            if !bounds_clearly_disjoint(bounds[current], bounds[other]) {
                pairs.push((current.min(other), current.max(other)));
            }
        }
        active.push((upper, current));
    }
    pairs.sort_unstable();
    Some(pairs)
}

#[allow(clippy::too_many_lines)]
fn isolate_pair(
    sources: &[SourcePiece],
    first_index: usize,
    second_index: usize,
    first_bounds: Box2,
    second_bounds: Box2,
    explicit_fillet_joins: &BTreeSet<ExplicitFilletJoin>,
    work: &mut Work,
) -> Result<Vec<CertifiedRoot>, VisualProfileIssueKind> {
    let first = &sources[first_index];
    let second = &sources[second_index];
    if bounds_clearly_disjoint(first_bounds, second_bounds) {
        return Ok(Vec::new());
    }
    if let Some(relation) = same_circular_carrier_relation(first, second)? {
        return match relation {
            CircularCarrierRelation::Disjoint | CircularCarrierRelation::EndpointTouch => {
                Ok(Vec::new())
            }
            CircularCarrierRelation::PositiveOverlap => Err(VisualProfileIssueKind::CurveOverlap {
                first: first.span,
                second: second.span,
            }),
            CircularCarrierRelation::Ambiguous => Err(VisualProfileIssueKind::NumericalAmbiguity {
                first: first.span,
                second: second.span,
            }),
        };
    }
    if exact_overlap(first, second) {
        return Err(if first.curve.is_linear() && second.curve.is_linear() {
            VisualProfileIssueKind::CollinearOverlap {
                first: first.span,
                second: second.span,
            }
        } else {
            VisualProfileIssueKind::CurveOverlap {
                first: first.span,
                second: second.span,
            }
        });
    }
    if first.curve.is_linear()
        && second.curve.is_linear()
        && linear_endpoint_contact_is_local(first, second)
    {
        return Ok(Vec::new());
    }
    if matches!(first.curve.kind, PieceKind::Circular { .. })
        && matches!(second.curve.kind, PieceKind::Circular { .. })
    {
        return circular_intersections(
            first,
            first_index,
            second,
            second_index,
            explicit_fillet_joins,
            work,
        );
    }
    if first.curve.is_linear() && matches!(second.curve.kind, PieceKind::Circular { .. }) {
        return line_circular_intersections(
            first,
            first_index,
            second,
            second_index,
            explicit_fillet_joins,
            work,
        );
    }
    if second.curve.is_linear() && matches!(first.curve.kind, PieceKind::Circular { .. }) {
        return line_circular_intersections(
            second,
            second_index,
            first,
            first_index,
            explicit_fillet_joins,
            work,
        )
        .map(|roots| {
            roots
                .into_iter()
                .map(|root| CertifiedRoot {
                    first_source: root.second_source,
                    second_source: root.first_source,
                    first: root.second,
                    second: root.first,
                    position: root.position,
                    vertex: root.vertex,
                })
                .collect()
        });
    }
    if first.curve.is_linear() {
        return line_curve_intersections(first, first_index, second, second_index, work);
    }
    if second.curve.is_linear() {
        return line_curve_intersections(second, second_index, first, first_index, work).map(
            |roots| {
                roots
                    .into_iter()
                    .map(|root| CertifiedRoot {
                        first_source: root.second_source,
                        second_source: root.first_source,
                        first: root.second,
                        second: root.first,
                        position: root.position,
                        vertex: root.vertex,
                    })
                    .collect()
            },
        );
    }
    isolate_rectangles_with_owned_joins(
        first,
        first_index,
        second,
        second_index,
        explicit_fillet_joins,
        work,
    )
}

fn isolate_rectangles_with_owned_joins(
    first: &SourcePiece,
    first_index: usize,
    second: &SourcePiece,
    second_index: usize,
    explicit_fillet_joins: &BTreeSet<ExplicitFilletJoin>,
    work: &mut Work,
) -> Result<Vec<CertifiedRoot>, VisualProfileIssueKind> {
    let Some((first_parameters, second_parameters, excluded_corners)) =
        intersection_domains_excluding_owned_joins(first, second, explicit_fillet_joins, work)?
    else {
        return Ok(Vec::new());
    };
    isolate_rectangles(
        first,
        first_index,
        second,
        second_index,
        first_parameters,
        second_parameters,
        &excluded_corners,
        work,
    )
}

#[allow(clippy::too_many_lines)]
fn circular_intersections(
    first: &SourcePiece,
    first_index: usize,
    second: &SourcePiece,
    second_index: usize,
    explicit_fillet_joins: &BTreeSet<ExplicitFilletJoin>,
    work: &mut Work,
) -> Result<Vec<CertifiedRoot>, VisualProfileIssueKind> {
    let PieceKind::Circular {
        center: first_center,
        radius: first_radius,
        ..
    } = &first.curve.kind
    else {
        unreachable!("circular kernel requires a circular first piece")
    };
    let PieceKind::Circular {
        center: second_center,
        radius: second_radius,
        ..
    } = &second.curve.kind
    else {
        unreachable!("circular kernel requires a circular second piece")
    };
    let center_delta = subtract(*second_center, *first_center);
    let distance = norm(center_delta);
    let geometry_scale = first_radius.abs().max(second_radius.abs()).max(distance);
    if !distance.is_finite() {
        return Err(VisualProfileIssueKind::UnresolvedIntersection {
            first: first.span,
            second: second.span,
        });
    }
    let center_delta_interval = [
        Interval::point(second_center[0]).sub(Interval::point(first_center[0])),
        Interval::point(second_center[1]).sub(Interval::point(first_center[1])),
    ];
    let distance_squared = center_delta_interval[0]
        .square()
        .add(center_delta_interval[1].square());
    let radius_sum_squared = Interval::point(*first_radius)
        .add(Interval::point(*second_radius))
        .square();
    let radius_difference_squared = Interval::point(*first_radius)
        .sub(Interval::point(*second_radius))
        .square();
    if distance_squared.lower > radius_sum_squared.upper
        || distance_squared.upper < radius_difference_squared.lower
    {
        return Ok(Vec::new());
    }
    let two_roots = distance_squared.upper < radius_sum_squared.lower
        && distance_squared.lower > radius_difference_squared.upper;
    if !two_roots {
        return isolate_rectangles_with_owned_joins(
            first,
            first_index,
            second,
            second_index,
            explicit_fillet_joins,
            work,
        );
    }
    let along = (first_radius.mul_add(*first_radius, distance * distance)
        - second_radius * second_radius)
        / (2.0 * distance);
    let height_squared = first_radius.mul_add(*first_radius, -(along * along));
    let height_tolerance = 512.0 * f64::EPSILON * geometry_scale * geometry_scale;
    if height_squared <= height_tolerance {
        return isolate_rectangles_with_owned_joins(
            first,
            first_index,
            second,
            second_index,
            explicit_fillet_joins,
            work,
        );
    }
    let unit = scale(center_delta, 1.0 / distance);
    let base = add(*first_center, scale(unit, along));
    let height = height_squared.max(0.0).sqrt();
    let perpendicular = [-unit[1], unit[0]];
    let candidates = vec![
        add(base, scale(perpendicular, height)),
        add(base, scale(perpendicular, -height)),
    ];
    let mut roots = Vec::new();
    for position in candidates {
        let first_angle = atan2_point(position[1] - first_center[1], position[0] - first_center[0])
            .map_err(|_| VisualProfileIssueKind::UnresolvedIntersection {
                first: first.span,
                second: second.span,
            })?;
        let second_angle = atan2_point(
            position[1] - second_center[1],
            position[0] - second_center[0],
        )
        .map_err(|_| VisualProfileIssueKind::UnresolvedIntersection {
            first: first.span,
            second: second.span,
        })?;
        let Some(first_parameter) = circular_source_parameter(first, first_angle)? else {
            return isolate_rectangles_with_owned_joins(
                first,
                first_index,
                second,
                second_index,
                explicit_fillet_joins,
                work,
            );
        };
        let Some(second_parameter) = circular_source_parameter(second, second_angle)? else {
            return isolate_rectangles_with_owned_joins(
                first,
                first_index,
                second,
                second_index,
                explicit_fillet_joins,
                work,
            );
        };
        if shared_corners(first, second)
            .iter()
            .any(|(first_corner, second_corner, _)| {
                first_parameter.contains(*first_corner) && second_parameter.contains(*second_corner)
            })
        {
            return isolate_rectangles_with_owned_joins(
                first,
                first_index,
                second,
                second_index,
                explicit_fillet_joins,
                work,
            );
        }
        let first_seed = root_seed(first.parameters, first_parameter);
        let second_seed = root_seed(second.parameters, second_parameter);
        let Some(krawczyk) = krawczyk_box(first, second, first_seed, second_seed)? else {
            return Err(VisualProfileIssueKind::UnresolvedIntersection {
                first: first.span,
                second: second.span,
            });
        };
        let [first_k, second_k] = krawczyk.image;
        if !first_seed.interior_contains(first_k) || !second_seed.interior_contains(second_k) {
            return Err(VisualProfileIssueKind::UnresolvedIntersection {
                first: first.span,
                second: second.span,
            });
        }
        let first_root = first_seed
            .intersection(first_k)
            .expect("strictly contained circular Krawczyk interval");
        let second_root = second_seed
            .intersection(second_k)
            .expect("strictly contained circular Krawczyk interval");
        let [first_root, second_root] =
            contract_krawczyk_root(first, second, first_root, second_root)?;
        let first_derivative = first
            .curve
            .derivative(first_root)
            .map_err(|error| evaluation_issue(error, first.span, second.span))?;
        let second_derivative = second
            .curve
            .derivative(second_root)
            .map_err(|error| evaluation_issue(error, first.span, second.span))?;
        if !cross_interval(first_derivative, second_derivative).excludes_zero() {
            return Err(VisualProfileIssueKind::TangentIntersection {
                first: first.span,
                second: second.span,
            });
        }
        let first_position = first
            .curve
            .position(first_root)
            .map_err(|error| evaluation_issue(error, first.span, second.span))?;
        let second_position = second
            .curve
            .position(second_root)
            .map_err(|error| evaluation_issue(error, first.span, second.span))?;
        let position = Box2 {
            x: first_position.x.intersection(second_position.x).ok_or(
                VisualProfileIssueKind::UnresolvedIntersection {
                    first: first.span,
                    second: second.span,
                },
            )?,
            y: first_position.y.intersection(second_position.y).ok_or(
                VisualProfileIssueKind::UnresolvedIntersection {
                    first: first.span,
                    second: second.span,
                },
            )?,
        };
        work.charge_root()?;
        roots.push(CertifiedRoot {
            first_source: first_index,
            second_source: second_index,
            first: first_root,
            second: second_root,
            position,
            vertex: None,
        });
    }
    roots.sort_by(|first, second| {
        first
            .first
            .lower
            .total_cmp(&second.first.lower)
            .then_with(|| first.second.lower.total_cmp(&second.second.lower))
    });
    certify_distinct_roots(roots, first, second)
}

#[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
fn circular_source_parameter(
    source: &SourcePiece,
    angle: Interval,
) -> Result<Option<Interval>, VisualProfileIssueKind> {
    let PieceKind::Circular {
        angle_offset,
        angle_rate,
        ..
    } = &source.curve.kind
    else {
        return Ok(None);
    };
    let unresolved = || VisualProfileIssueKind::UnresolvedIntersection {
        first: source.span,
        second: source.span,
    };
    if !angle.is_finite()
        || !angle_offset.is_finite()
        || *angle_rate == 0.0
        || !angle_rate.is_finite()
    {
        return Err(unresolved());
    }
    let source_angles = angle_offset.add(source.parameters.mul(Interval::point(*angle_rate)));
    let winding_range = source_angles
        .sub(angle)
        .div(TAU_INTERVAL)
        .ok_or_else(unresolved)?;
    let first_winding = winding_range.lower.ceil();
    let last_winding = winding_range.upper.floor();
    if first_winding > last_winding {
        return Ok(None);
    }
    if !first_winding.is_finite()
        || !last_winding.is_finite()
        || first_winding.abs() > MAX_EXACT_F64_INTEGER
        || last_winding.abs() > MAX_EXACT_F64_INTEGER
    {
        return Err(unresolved());
    }
    let first_winding = first_winding as i64;
    let last_winding = last_winding as i64;
    let winding_count = last_winding
        .checked_sub(first_winding)
        .and_then(|width| width.checked_add(1))
        .and_then(|count| usize::try_from(count).ok())
        .ok_or_else(unresolved)?;
    // A bounded circular source piece spans at most one turn. A larger algebraic range is
    // malformed or no longer representable precisely enough to seed strict certification.
    if winding_count > 4 {
        return Err(unresolved());
    }
    let mut result: Option<Interval> = None;
    for winding in first_winding..=last_winding {
        let numerator = angle
            .add(TAU_INTERVAL.scale(winding as f64))
            .sub(*angle_offset);
        let parameter = numerator
            .div(Interval::point(*angle_rate))
            .ok_or_else(unresolved)?;
        if !parameter.is_finite() {
            return Err(unresolved());
        }
        let Some(parameter) = parameter.intersection(source.parameters) else {
            continue;
        };
        result = Some(result.map_or(parameter, |current| current.include(parameter)));
    }
    Ok(result)
}

fn root_seed(domain: Interval, parameter: Interval) -> Interval {
    let half_width = domain.width().max(1.0) * 1.0e-8;
    parameter
        .add(Interval {
            lower: -half_width,
            upper: half_width,
        })
        .intersection(domain)
        .expect("finite circular root seed lies in its source domain")
}

fn line_circular_intersections(
    line: &SourcePiece,
    line_index: usize,
    circular: &SourcePiece,
    circular_index: usize,
    explicit_fillet_joins: &BTreeSet<ExplicitFilletJoin>,
    work: &mut Work,
) -> Result<Vec<CertifiedRoot>, VisualProfileIssueKind> {
    let PieceKind::Circular { center, radius, .. } = circular.curve.kind else {
        unreachable!("line/circular kernel requires circular geometry")
    };
    let Some((line_start, line_direction)) = linear_geometry(line) else {
        unreachable!("line/circular kernel requires linear geometry")
    };
    if has_owned_join_corner(line, circular, explicit_fillet_joins) {
        let Some((line_parameters, circular_parameters, excluded_corners)) =
            intersection_domains_excluding_owned_joins(
                line,
                circular,
                explicit_fillet_joins,
                work,
            )?
        else {
            return Ok(Vec::new());
        };
        return isolate_rectangles(
            line,
            line_index,
            circular,
            circular_index,
            line_parameters,
            circular_parameters,
            &excluded_corners,
            work,
        );
    }
    let length_squared = line_direction[0].square().add(line_direction[1].square());
    if !length_squared.excludes_zero() {
        return Err(VisualProfileIssueKind::UnresolvedIntersection {
            first: line.span,
            second: circular.span,
        });
    }
    let center_offset = [
        Interval::point(center[0]).sub(line_start[0]),
        Interval::point(center[1]).sub(line_start[1]),
    ];
    let signed_distance_numerator = cross_interval(center_offset, line_direction);
    let discriminant = Interval::point(radius)
        .square()
        .mul(length_squared)
        .sub(signed_distance_numerator.square());
    if discriminant.upper < 0.0 {
        return Ok(Vec::new());
    }
    if discriminant.lower <= 0.0 {
        return isolate_rectangles(
            line,
            line_index,
            circular,
            circular_index,
            line.parameters,
            circular.parameters,
            &shared_corners(line, circular),
            work,
        );
    }
    line_curve_intersections(line, line_index, circular, circular_index, work)
}

#[allow(clippy::too_many_lines)]
fn line_curve_intersections(
    line: &SourcePiece,
    line_index: usize,
    curve: &SourcePiece,
    curve_index: usize,
    work: &mut Work,
) -> Result<Vec<CertifiedRoot>, VisualProfileIssueKind> {
    let Some((start, direction)) = linear_geometry(line) else {
        unreachable!("line/curve kernel requires linear geometry")
    };
    let length_squared = direction[0].square().add(direction[1].square());
    if !length_squared.excludes_zero() {
        return Err(VisualProfileIssueKind::UnresolvedIntersection {
            first: line.span,
            second: curve.span,
        });
    }
    let mut stack = vec![(curve.parameters, 0_usize)];
    let mut roots = Vec::new();
    while let Some((parameter, depth)) = stack.pop() {
        let position = curve
            .curve
            .position(parameter)
            .map_err(|error| evaluation_issue(error, line.span, curve.span))?;
        let projected = position
            .x
            .sub(start[0])
            .mul(direction[0])
            .add(position.y.sub(start[1]).mul(direction[1]))
            .div(length_squared)
            .ok_or(VisualProfileIssueKind::UnresolvedIntersection {
                first: line.span,
                second: curve.span,
            })?;
        if projected.upper < line.parameters.lower || projected.lower > line.parameters.upper {
            continue;
        }
        let offset = [position.x.sub(start[0]), position.y.sub(start[1])];
        let value = cross_interval(offset, direction);
        if !value.contains_zero() {
            continue;
        }
        let derivative = curve
            .curve
            .derivative(parameter)
            .map_err(|error| evaluation_issue(error, line.span, curve.span))?;
        let derivative_value = cross_interval(derivative, direction);
        if curve.curve.is_linear() && derivative_value.excludes_zero() {
            let endpoint_root = [
                (curve.parameters.lower, curve.start_position),
                (curve.parameters.upper, curve.end_position),
            ]
            .into_iter()
            .find(|(candidate, point)| {
                parameter.contains(*candidate) && linear_support_contains(line, *point)
            });
            if let Some((endpoint, _)) = endpoint_root {
                if let Some(root) = make_line_curve_root(
                    line,
                    line_index,
                    curve,
                    curve_index,
                    Interval::point(endpoint),
                    work,
                )? {
                    roots.push(root);
                }
                continue;
            }
        }
        if derivative_value.excludes_zero() {
            let middle = parameter.midpoint();
            let middle_position = curve
                .curve
                .position(Interval::point(middle))
                .map_err(|error| evaluation_issue(error, line.span, curve.span))?;
            let middle_value = middle_position
                .x
                .sub(start[0])
                .mul(direction[1])
                .sub(middle_position.y.sub(start[1]).mul(direction[0]));
            let newton = Interval::point(middle).sub(middle_value.div(derivative_value).ok_or(
                VisualProfileIssueKind::UnresolvedIntersection {
                    first: line.span,
                    second: curve.span,
                },
            )?);
            if parameter.interior_contains(newton) {
                let curve_root = parameter.intersection(newton).expect("contained interval");
                if let Some(root) =
                    make_line_curve_root(line, line_index, curve, curve_index, curve_root, work)?
                {
                    roots.push(root);
                }
                continue;
            }
        }
        let middle = parameter.midpoint();
        if let Some(root_interval) =
            bracket_midpoint_root(line, curve, parameter, middle, direction)?
        {
            if let Some(root) =
                make_line_curve_root(line, line_index, curve, curve_index, root_interval, work)?
            {
                roots.push(root);
            }
            if parameter.lower < root_interval.lower {
                stack.push((
                    Interval::hull(parameter.lower, root_interval.lower),
                    depth + 1,
                ));
            }
            if root_interval.upper < parameter.upper {
                stack.push((
                    Interval::hull(root_interval.upper, parameter.upper),
                    depth + 1,
                ));
            }
            continue;
        }
        if excluded_line_curve_corner_is_local(line, curve, parameter, derivative_value)? {
            continue;
        }
        if depth >= work.options.max_intersection_depth {
            return Err(if derivative_value.contains_zero() {
                VisualProfileIssueKind::TangentIntersection {
                    first: line.span,
                    second: curve.span,
                }
            } else {
                VisualProfileIssueKind::UnresolvedIntersection {
                    first: line.span,
                    second: curve.span,
                }
            });
        }
        if work.intersection_subdivisions >= work.options.max_intersection_subdivisions {
            return Err(
                VisualProfileIssueKind::IntersectionSubdivisionBudgetExceeded {
                    first: line.span,
                    second: curve.span,
                    limit: work.options.max_intersection_subdivisions,
                },
            );
        }
        let middle = parameter.midpoint();
        if middle.to_bits() == parameter.lower.to_bits()
            || middle.to_bits() == parameter.upper.to_bits()
        {
            return Err(VisualProfileIssueKind::UnresolvedIntersection {
                first: line.span,
                second: curve.span,
            });
        }
        if !work.charge_operation(
            OperationWorkCounter::ProfileSubdivisions,
            1,
            OperationCheckpoint::ProfileSubdivision,
        ) {
            return Err(
                VisualProfileIssueKind::IntersectionSubdivisionBudgetExceeded {
                    first: line.span,
                    second: curve.span,
                    limit: work.options.max_intersection_subdivisions,
                },
            );
        }
        work.intersection_subdivisions += 1;
        stack.push((Interval::hull(middle, parameter.upper), depth + 1));
        stack.push((Interval::hull(parameter.lower, middle), depth + 1));
    }
    roots.sort_by(|first, second| first.second.lower.total_cmp(&second.second.lower));
    certify_distinct_roots(roots, line, curve)
}

fn bracket_midpoint_root(
    line: &SourcePiece,
    curve: &SourcePiece,
    domain: Interval,
    middle: f64,
    line_direction: [Interval; 2],
) -> Result<Option<Interval>, VisualProfileIssueKind> {
    let Some((line_start, _)) = linear_geometry(line) else {
        unreachable!("line root bracketing requires linear geometry")
    };
    let width = domain.width() * 1.0e-8;
    if width == 0.0 {
        return Ok(None);
    }
    let interval = Interval::hull(
        (middle - width).max(domain.lower),
        (middle + width).min(domain.upper),
    );
    if interval.lower.to_bits() == interval.upper.to_bits() {
        return Ok(None);
    }
    let derivative = curve
        .curve
        .derivative(interval)
        .map_err(|error| evaluation_issue(error, line.span, curve.span))?;
    let derivative = cross_interval(derivative, line_direction);
    if !derivative.excludes_zero() {
        return Ok(None);
    }
    let signed_value = |parameter| -> Result<Interval, VisualProfileIssueKind> {
        let position = curve
            .curve
            .position(Interval::point(parameter))
            .map_err(|error| evaluation_issue(error, line.span, curve.span))?;
        Ok(position
            .x
            .sub(line_start[0])
            .mul(line_direction[1])
            .sub(position.y.sub(line_start[1]).mul(line_direction[0])))
    };
    let lower = signed_value(interval.lower)?;
    let upper = signed_value(interval.upper)?;
    let bracketed =
        lower.upper < 0.0 && upper.lower > 0.0 || upper.upper < 0.0 && lower.lower > 0.0;
    Ok(bracketed.then_some(interval))
}

fn make_line_curve_root(
    line: &SourcePiece,
    line_index: usize,
    curve: &SourcePiece,
    curve_index: usize,
    curve_root: Interval,
    work: &mut Work,
) -> Result<Option<CertifiedRoot>, VisualProfileIssueKind> {
    let curve_root = contract_line_curve_root(line, curve, curve_root)?;
    let Some((start, direction)) = linear_geometry(line) else {
        unreachable!("line root construction requires linear geometry")
    };
    let length_squared = direction[0].square().add(direction[1].square());
    let curve_position = curve
        .curve
        .position(curve_root)
        .map_err(|error| evaluation_issue(error, line.span, curve.span))?;
    let line_parameter = curve_position
        .x
        .sub(start[0])
        .mul(direction[0])
        .add(curve_position.y.sub(start[1]).mul(direction[1]))
        .div(length_squared)
        .ok_or(VisualProfileIssueKind::UnresolvedIntersection {
            first: line.span,
            second: curve.span,
        })?;
    let Some(line_parameter) = line_parameter.intersection(line.parameters) else {
        return Ok(None);
    };
    let line_vertex = if line_parameter.contains(line.parameters.lower) {
        Some((line.start, line.parameters.lower, line.start_position))
    } else if line_parameter.contains(line.parameters.upper) {
        Some((line.end, line.parameters.upper, line.end_position))
    } else {
        None
    };
    let curve_vertex = if curve_root.contains(curve.parameters.lower) {
        Some((curve.start, curve.parameters.lower, curve.start_position))
    } else if curve_root.contains(curve.parameters.upper) {
        Some((curve.end, curve.parameters.upper, curve.end_position))
    } else {
        None
    };
    if line_vertex.is_some() && curve_vertex.is_some() {
        return Err(VisualProfileIssueKind::UnresolvedIntersection {
            first: line.span,
            second: curve.span,
        });
    }
    let (line_root, curve_root, vertex) = match (line_vertex, curve_vertex) {
        (Some((vertex, parameter, _)), None) => {
            (Interval::point(parameter), curve_root, Some(vertex))
        }
        (None, Some((vertex, parameter, point))) if linear_support_contains(line, point) => {
            (line_parameter, Interval::point(parameter), Some(vertex))
        }
        (None, None)
            if line_parameter.lower > line.parameters.lower
                && line_parameter.upper < line.parameters.upper =>
        {
            (line_parameter, curve_root, None)
        }
        _ => {
            return Err(VisualProfileIssueKind::UnresolvedIntersection {
                first: line.span,
                second: curve.span,
            });
        }
    };
    let line_position = line
        .curve
        .position(line_root)
        .map_err(|error| evaluation_issue(error, line.span, curve.span))?;
    let curve_position = curve
        .curve
        .position(curve_root)
        .map_err(|error| evaluation_issue(error, line.span, curve.span))?;
    let root_position = Box2 {
        x: line_position.x.intersection(curve_position.x).ok_or(
            VisualProfileIssueKind::UnresolvedIntersection {
                first: line.span,
                second: curve.span,
            },
        )?,
        y: line_position.y.intersection(curve_position.y).ok_or(
            VisualProfileIssueKind::UnresolvedIntersection {
                first: line.span,
                second: curve.span,
            },
        )?,
    };
    work.charge_root()?;
    Ok(Some(CertifiedRoot {
        first_source: line_index,
        second_source: curve_index,
        first: line_root,
        second: curve_root,
        position: root_position,
        vertex,
    }))
}

fn linear_support_contains(source: &SourcePiece, point: [f64; 2]) -> bool {
    let Some((start, direction)) = linear_geometry(source) else {
        return false;
    };
    let length_squared = direction[0].square().add(direction[1].square());
    if !length_squared.excludes_zero() {
        return false;
    }
    let offset = [
        Interval::point(point[0]).sub(start[0]),
        Interval::point(point[1]).sub(start[1]),
    ];
    let Some(projected) = offset[0]
        .mul(direction[0])
        .add(offset[1].mul(direction[1]))
        .div(length_squared)
    else {
        return false;
    };
    cross_interval(offset, direction).contains_zero()
        && projected.upper >= source.parameters.lower
        && projected.lower <= source.parameters.upper
}

fn contract_line_curve_root(
    line: &SourcePiece,
    curve: &SourcePiece,
    mut root: Interval,
) -> Result<Interval, VisualProfileIssueKind> {
    let Some((start, direction)) = linear_geometry(line) else {
        unreachable!("line root contraction requires linear geometry")
    };
    for _ in 0..16 {
        let derivative = curve
            .curve
            .derivative(root)
            .map_err(|error| evaluation_issue(error, line.span, curve.span))?;
        let derivative = cross_interval(derivative, direction);
        if !derivative.excludes_zero() {
            break;
        }
        let middle = root.midpoint();
        let position = curve
            .curve
            .position(Interval::point(middle))
            .map_err(|error| evaluation_issue(error, line.span, curve.span))?;
        let value = cross_interval(
            [position.x.sub(start[0]), position.y.sub(start[1])],
            direction,
        );
        let newton = Interval::point(middle).sub(value.div(derivative).ok_or(
            VisualProfileIssueKind::UnresolvedIntersection {
                first: line.span,
                second: curve.span,
            },
        )?);
        let Some(contracted) = root.intersection(newton) else {
            return Err(VisualProfileIssueKind::UnresolvedIntersection {
                first: line.span,
                second: curve.span,
            });
        };
        if contracted.lower.to_bits() == root.lower.to_bits()
            && contracted.upper.to_bits() == root.upper.to_bits()
        {
            break;
        }
        root = contracted;
    }
    Ok(root)
}

fn linear_geometry(source: &SourcePiece) -> Option<([Interval; 2], [Interval; 2])> {
    let PieceKind::Linear { start, delta } = &source.curve.kind else {
        return None;
    };
    Some((start.map(Interval::point), *delta))
}

fn bounds_clearly_disjoint(first: Box2, second: Box2) -> bool {
    let scale = [
        first.x.lower,
        first.x.upper,
        first.y.lower,
        first.y.upper,
        second.x.lower,
        second.x.upper,
        second.y.lower,
        second.y.upper,
    ]
    .into_iter()
    .map(f64::abs)
    .fold(1.0, f64::max);
    let margin = 256.0 * f64::EPSILON * scale;
    first.x.upper + margin < second.x.lower
        || second.x.upper + margin < first.x.lower
        || first.y.upper + margin < second.y.lower
        || second.y.upper + margin < first.y.lower
}

fn isolate_self(
    source: &SourcePiece,
    source_index: usize,
    work: &mut Work,
) -> Result<Vec<CertifiedRoot>, VisualProfileIssueKind> {
    fn visit(
        source: &SourcePiece,
        source_index: usize,
        parameter: Interval,
        depth: usize,
        roots: &mut Vec<CertifiedRoot>,
        work: &mut Work,
    ) -> Result<(), VisualProfileIssueKind> {
        let derivative = source
            .curve
            .derivative(parameter)
            .map_err(|error| match error {
                PieceEvaluationError::Pole => VisualProfileIssueKind::RationalPole {
                    support: source.span,
                },
                PieceEvaluationError::NonFinite => VisualProfileIssueKind::ZeroSpeed {
                    support: source.span,
                },
            })?;
        if derivative[0].excludes_zero() || derivative[1].excludes_zero() {
            return Ok(());
        }
        if depth >= work.options.max_intersection_depth
            || work.intersection_subdivisions >= work.options.max_intersection_subdivisions
        {
            return Err(
                if work.intersection_subdivisions >= work.options.max_intersection_subdivisions {
                    VisualProfileIssueKind::IntersectionSubdivisionBudgetExceeded {
                        first: source.span,
                        second: source.span,
                        limit: work.options.max_intersection_subdivisions,
                    }
                } else {
                    VisualProfileIssueKind::UnresolvedIntersection {
                        first: source.span,
                        second: source.span,
                    }
                },
            );
        }
        let middle = parameter.midpoint();
        if middle.to_bits() == parameter.lower.to_bits()
            || middle.to_bits() == parameter.upper.to_bits()
        {
            return Err(VisualProfileIssueKind::UnresolvedIntersection {
                first: source.span,
                second: source.span,
            });
        }
        if !work.charge_operation(
            OperationWorkCounter::ProfileSubdivisions,
            1,
            OperationCheckpoint::ProfileSubdivision,
        ) {
            return Err(
                VisualProfileIssueKind::IntersectionSubdivisionBudgetExceeded {
                    first: source.span,
                    second: source.span,
                    limit: work.options.max_intersection_subdivisions,
                },
            );
        }
        work.intersection_subdivisions += 1;
        let left = Interval::hull(parameter.lower, middle);
        let right = Interval::hull(middle, parameter.upper);
        visit(source, source_index, left, depth + 1, roots, work)?;
        visit(source, source_index, right, depth + 1, roots, work)?;
        roots.extend(isolate_rectangles(
            source,
            source_index,
            source,
            source_index,
            left,
            right,
            &[(
                left.upper,
                right.lower,
                VertexKey::CurveBoundary {
                    curve: source.span.curve,
                    boundary: u32::MAX,
                },
            )],
            work,
        )?);
        Ok(())
    }

    let mut roots = Vec::new();
    visit(source, source_index, source.parameters, 0, &mut roots, work)?;
    roots.sort_by(|first, second| {
        first
            .first
            .lower
            .total_cmp(&second.first.lower)
            .then_with(|| first.second.lower.total_cmp(&second.second.lower))
    });
    certify_distinct_roots(roots, source, source)
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn isolate_rectangles(
    first: &SourcePiece,
    first_index: usize,
    second: &SourcePiece,
    second_index: usize,
    first_parameter: Interval,
    second_parameter: Interval,
    excluded_corners: &[(f64, f64, VertexKey)],
    work: &mut Work,
) -> Result<Vec<CertifiedRoot>, VisualProfileIssueKind> {
    let mut stack = vec![(first_parameter, second_parameter, 0_usize)];
    let mut roots = Vec::new();
    while let Some((first_box, second_box, depth)) = stack.pop() {
        let first_position = first
            .curve
            .position(first_box)
            .map_err(|error| evaluation_issue(error, first.span, second.span))?;
        let second_position = second
            .curve
            .position(second_box)
            .map_err(|error| evaluation_issue(error, first.span, second.span))?;
        if first_position.disjoint(second_position) {
            continue;
        }
        let first_derivative = first
            .curve
            .derivative(first_box)
            .map_err(|error| evaluation_issue(error, first.span, second.span))?;
        let second_derivative = second
            .curve
            .derivative(second_box)
            .map_err(|error| evaluation_issue(error, first.span, second.span))?;
        let determinant = cross_interval(first_derivative, second_derivative);
        let krawczyk = krawczyk_box(first, second, first_box, second_box)?;
        if let Some(krawczyk) = krawczyk {
            let [first_k, second_k] = krawczyk.image;
            if !first_box.overlaps(first_k) || !second_box.overlaps(second_k) {
                continue;
            }
            let certified = if determinant.excludes_zero()
                && first_box.interior_contains(first_k)
                && second_box.interior_contains(second_k)
            {
                let first_root = first_box.intersection(first_k).expect("overlap checked");
                let second_root = second_box.intersection(second_k).expect("overlap checked");
                Some(contract_krawczyk_root(
                    first,
                    second,
                    first_root,
                    second_root,
                )?)
            } else if determinant.excludes_zero()
                && krawczyk.contraction_bound < 1.0
                && interval_contains(first_box, first_k)
                && interval_contains(second_box, second_k)
            {
                let first_root = first_box.intersection(first_k).expect("closed containment");
                let second_root = second_box
                    .intersection(second_k)
                    .expect("closed containment");
                Some(contract_krawczyk_root(
                    first,
                    second,
                    first_root,
                    second_root,
                )?)
            } else if determinant.excludes_zero() {
                certify_boundary_krawczyk_root(
                    first,
                    first_index,
                    second,
                    second_index,
                    first_box,
                    second_box,
                    first_k,
                    second_k,
                )?
            } else {
                None
            };
            if let Some([first_root, second_root]) = certified {
                let contains_excluded_corner =
                    excluded_corners
                        .iter()
                        .any(|(first_corner, second_corner, _)| {
                            first_root.contains(*first_corner)
                                && second_root.contains(*second_corner)
                        });
                if contains_excluded_corner
                    && excluded_corner_is_local(
                        excluded_corners,
                        first,
                        second,
                        first_root,
                        second_root,
                    )?
                {
                    continue;
                }
                if !contains_excluded_corner {
                    let first_root_position = first
                        .curve
                        .position(first_root)
                        .map_err(|error| evaluation_issue(error, first.span, second.span))?;
                    let second_root_position = second
                        .curve
                        .position(second_root)
                        .map_err(|error| evaluation_issue(error, first.span, second.span))?;
                    let position = Box2 {
                        x: first_root_position
                            .x
                            .intersection(second_root_position.x)
                            .ok_or(VisualProfileIssueKind::UnresolvedIntersection {
                                first: first.span,
                                second: second.span,
                            })?,
                        y: first_root_position
                            .y
                            .intersection(second_root_position.y)
                            .ok_or(VisualProfileIssueKind::UnresolvedIntersection {
                                first: first.span,
                                second: second.span,
                            })?,
                    };
                    work.charge_root()?;
                    roots.push(CertifiedRoot {
                        first_source: first_index,
                        second_source: second_index,
                        first: first_root,
                        second: second_root,
                        position,
                        vertex: None,
                    });
                    continue;
                }
            }
        }

        if excluded_corner_is_local(excluded_corners, first, second, first_box, second_box)? {
            continue;
        }
        if depth >= work.options.max_intersection_depth {
            return Err(if determinant.contains_zero() {
                VisualProfileIssueKind::TangentIntersection {
                    first: first.span,
                    second: second.span,
                }
            } else {
                VisualProfileIssueKind::UnresolvedIntersection {
                    first: first.span,
                    second: second.span,
                }
            });
        }
        if work.intersection_subdivisions >= work.options.max_intersection_subdivisions {
            return Err(
                VisualProfileIssueKind::IntersectionSubdivisionBudgetExceeded {
                    first: first.span,
                    second: second.span,
                    limit: work.options.max_intersection_subdivisions,
                },
            );
        }
        if !work.charge_operation(
            OperationWorkCounter::ProfileSubdivisions,
            1,
            OperationCheckpoint::ProfileSubdivision,
        ) {
            return Err(
                VisualProfileIssueKind::IntersectionSubdivisionBudgetExceeded {
                    first: first.span,
                    second: second.span,
                    limit: work.options.max_intersection_subdivisions,
                },
            );
        }
        work.intersection_subdivisions += 1;
        if normalized_width(first_box, first.parameters)
            >= normalized_width(second_box, second.parameters)
        {
            let middle = intersection_subdivision_point(first_box);
            if middle.to_bits() == first_box.lower.to_bits()
                || middle.to_bits() == first_box.upper.to_bits()
            {
                return Err(VisualProfileIssueKind::UnresolvedIntersection {
                    first: first.span,
                    second: second.span,
                });
            }
            stack.push((
                Interval::hull(middle, first_box.upper),
                second_box,
                depth + 1,
            ));
            stack.push((
                Interval::hull(first_box.lower, middle),
                second_box,
                depth + 1,
            ));
        } else {
            let middle = intersection_subdivision_point(second_box);
            if middle.to_bits() == second_box.lower.to_bits()
                || middle.to_bits() == second_box.upper.to_bits()
            {
                return Err(VisualProfileIssueKind::UnresolvedIntersection {
                    first: first.span,
                    second: second.span,
                });
            }
            stack.push((
                first_box,
                Interval::hull(middle, second_box.upper),
                depth + 1,
            ));
            stack.push((
                first_box,
                Interval::hull(second_box.lower, middle),
                depth + 1,
            ));
        }
    }
    roots.sort_by(|first, second| {
        first
            .first
            .lower
            .total_cmp(&second.first.lower)
            .then_with(|| first.second.lower.total_cmp(&second.second.lower))
    });
    certify_distinct_roots(roots, first, second)
}

#[allow(clippy::too_many_arguments)]
fn certify_boundary_krawczyk_root(
    first: &SourcePiece,
    first_index: usize,
    second: &SourcePiece,
    second_index: usize,
    first_box: Interval,
    second_box: Interval,
    first_k: Interval,
    second_k: Interval,
) -> Result<Option<[Interval; 2]>, VisualProfileIssueKind> {
    let Some(first_candidate) = first_k.intersection(first_box) else {
        return Ok(None);
    };
    let Some(second_candidate) = second_k.intersection(second_box) else {
        return Ok(None);
    };
    let first_seed = root_seed(first.parameters, first_candidate);
    let second_seed = root_seed(second.parameters, second_candidate);
    if first_index == second_index && first_seed.overlaps(second_seed) {
        return Ok(None);
    }
    let first_derivative = first
        .curve
        .derivative(first_seed)
        .map_err(|error| evaluation_issue(error, first.span, second.span))?;
    let second_derivative = second
        .curve
        .derivative(second_seed)
        .map_err(|error| evaluation_issue(error, first.span, second.span))?;
    if !cross_interval(first_derivative, second_derivative).excludes_zero() {
        return Ok(None);
    }
    let Some(krawczyk) = krawczyk_box(first, second, first_seed, second_seed)? else {
        return Ok(None);
    };
    let [first_retry, second_retry] = krawczyk.image;
    if !first_seed.interior_contains(first_retry) || !second_seed.interior_contains(second_retry) {
        return Ok(None);
    }
    let first_root = first_seed
        .intersection(first_retry)
        .expect("strictly contained boundary retry");
    let second_root = second_seed
        .intersection(second_retry)
        .expect("strictly contained boundary retry");
    let roots = contract_krawczyk_root(first, second, first_root, second_root)?;
    Ok((first_box.overlaps(roots[0]) && second_box.overlaps(roots[1])).then_some(roots))
}

fn contract_krawczyk_root(
    first: &SourcePiece,
    second: &SourcePiece,
    mut first_root: Interval,
    mut second_root: Interval,
) -> Result<[Interval; 2], VisualProfileIssueKind> {
    for _ in 0..16 {
        let Some(krawczyk) = krawczyk_box(first, second, first_root, second_root)? else {
            break;
        };
        let [first_k, second_k] = krawczyk.image;
        let Some(first_contracted) = first_root.intersection(first_k) else {
            return Err(VisualProfileIssueKind::UnresolvedIntersection {
                first: first.span,
                second: second.span,
            });
        };
        let Some(second_contracted) = second_root.intersection(second_k) else {
            return Err(VisualProfileIssueKind::UnresolvedIntersection {
                first: first.span,
                second: second.span,
            });
        };
        let unchanged = first_contracted.lower.to_bits() == first_root.lower.to_bits()
            && first_contracted.upper.to_bits() == first_root.upper.to_bits()
            && second_contracted.lower.to_bits() == second_root.lower.to_bits()
            && second_contracted.upper.to_bits() == second_root.upper.to_bits();
        first_root = first_contracted;
        second_root = second_contracted;
        if unchanged {
            break;
        }
    }
    Ok([first_root, second_root])
}

fn krawczyk_box(
    first: &SourcePiece,
    second: &SourcePiece,
    first_parameter: Interval,
    second_parameter: Interval,
) -> Result<Option<KrawczykResult>, VisualProfileIssueKind> {
    let first_mid = first_parameter.midpoint();
    let second_mid = second_parameter.midpoint();
    let first_point = first
        .curve
        .position(Interval::point(first_mid))
        .map_err(|error| evaluation_issue(error, first.span, second.span))?;
    let second_point = second
        .curve
        .position(Interval::point(second_mid))
        .map_err(|error| evaluation_issue(error, first.span, second.span))?;
    let value = [
        first_point.x.sub(second_point.x),
        first_point.y.sub(second_point.y),
    ];
    let first_mid_derivative = first
        .curve
        .tangent(first_mid)
        .map_err(|error| evaluation_issue(error, first.span, second.span))?;
    let second_mid_derivative = second
        .curve
        .tangent(second_mid)
        .map_err(|error| evaluation_issue(error, first.span, second.span))?;
    let determinant = cross(first_mid_derivative, second_mid_derivative);
    let scale = norm(first_mid_derivative) * norm(second_mid_derivative);
    if !determinant.is_finite() || determinant.abs() <= 64.0 * f64::EPSILON * scale {
        return Ok(None);
    }
    // Inverse of J = [C1', -C2'].
    let inverse = [
        [
            second_mid_derivative[1] / determinant,
            -second_mid_derivative[0] / determinant,
        ],
        [
            first_mid_derivative[1] / determinant,
            -first_mid_derivative[0] / determinant,
        ],
    ];
    let first_derivative = first
        .curve
        .derivative(first_parameter)
        .map_err(|error| evaluation_issue(error, first.span, second.span))?;
    let second_derivative = second
        .curve
        .derivative(second_parameter)
        .map_err(|error| evaluation_issue(error, first.span, second.span))?;
    let jacobian = [
        [first_derivative[0], second_derivative[0].neg()],
        [first_derivative[1], second_derivative[1].neg()],
    ];
    let mut remainder = [[Interval::ZERO; 2]; 2];
    for row in 0..2 {
        for column in 0..2 {
            let product = Interval::point(inverse[row][0])
                .mul(jacobian[0][column])
                .add(Interval::point(inverse[row][1]).mul(jacobian[1][column]));
            remainder[row][column] = Interval::point(f64::from(row == column)).sub(product);
        }
    }
    let displacement = [
        Interval::point(first_parameter.lower)
            .sub(Interval::point(first_mid))
            .include(Interval::point(first_parameter.upper).sub(Interval::point(first_mid))),
        Interval::point(second_parameter.lower)
            .sub(Interval::point(second_mid))
            .include(Interval::point(second_parameter.upper).sub(Interval::point(second_mid))),
    ];
    let mid = [first_mid, second_mid];
    let mut result = [Interval::ZERO; 2];
    let mut contraction_bound = 0.0_f64;
    for row in 0..2 {
        let row_bound = remainder[row].iter().fold(Interval::ZERO, |sum, value| {
            sum.add(Interval::point(value.lower.abs().max(value.upper.abs())))
        });
        contraction_bound = contraction_bound.max(row_bound.upper);
        let correction = Interval::point(inverse[row][0])
            .mul(value[0])
            .add(Interval::point(inverse[row][1]).mul(value[1]));
        result[row] = Interval::point(mid[row])
            .sub(correction)
            .add(remainder[row][0].mul(displacement[0]))
            .add(remainder[row][1].mul(displacement[1]));
    }
    Ok(Some(KrawczykResult {
        image: result,
        contraction_bound,
    }))
}

fn interval_contains(outer: Interval, inner: Interval) -> bool {
    outer.lower <= inner.lower && inner.upper <= outer.upper
}

fn merge_roots(
    sources: &[SourcePiece],
    roots: &[CertifiedRoot],
) -> (Vec<VertexKey>, Vec<VisualProfileIntersection>) {
    let vertices = roots
        .iter()
        .enumerate()
        .map(|(index, root)| root.vertex.unwrap_or(VertexKey::Intersection(index)))
        .collect::<Vec<_>>();
    let mut intersections = roots
        .iter()
        .map(|root| VisualProfileIntersection {
            first_span: sources[root.first_source].span,
            second_span: sources[root.second_source].span,
            first_parameter_enclosure: [root.first.lower, root.first.upper],
            second_parameter_enclosure: [root.second.lower, root.second.upper],
            position_enclosure: [
                [root.position.x.lower, root.position.y.lower],
                [root.position.x.upper, root.position.y.upper],
            ],
        })
        .collect::<Vec<_>>();
    intersections.sort_by(|first, second| {
        first
            .first_span
            .cmp(&second.first_span)
            .then_with(|| first.second_span.cmp(&second.second_span))
            .then_with(|| {
                first.first_parameter_enclosure[0].total_cmp(&second.first_parameter_enclosure[0])
            })
            .then_with(|| {
                first.second_parameter_enclosure[0].total_cmp(&second.second_parameter_enclosure[0])
            })
    });
    (vertices, intersections)
}

fn certify_distinct_roots(
    mut roots: Vec<CertifiedRoot>,
    first_source: &SourcePiece,
    second_source: &SourcePiece,
) -> Result<Vec<CertifiedRoot>, VisualProfileIssueKind> {
    let first_span = first_source.span;
    let second_span = second_source.span;
    loop {
        let mut merged = false;
        'pairs: for first in 0..roots.len() {
            for second in first + 1..roots.len() {
                if !roots[first].first.overlaps(roots[second].first)
                    || !roots[first].second.overlaps(roots[second].second)
                {
                    continue;
                }
                roots[first] = merge_certified_roots(
                    roots[first],
                    roots[second],
                    first_source,
                    second_source,
                )?;
                roots.remove(second);
                merged = true;
                break 'pairs;
            }
        }
        if !merged {
            break;
        }
    }
    for first in 0..roots.len() {
        for second in first + 1..roots.len() {
            let overlaps = roots[first].first.overlaps(roots[second].first)
                || roots[first].second.overlaps(roots[second].second);
            if overlaps
                && !matches!(
                    (roots[first].vertex, roots[second].vertex),
                    (Some(first), Some(second)) if first == second
                )
            {
                return Err(VisualProfileIssueKind::NumericalAmbiguity {
                    first: first_span,
                    second: second_span,
                });
            }
        }
    }
    roots.dedup_by(|first, second| {
        matches!((first.vertex, second.vertex), (Some(first), Some(second)) if first == second)
    });
    Ok(roots)
}

fn merge_certified_roots(
    first: CertifiedRoot,
    second: CertifiedRoot,
    first_source: &SourcePiece,
    second_source: &SourcePiece,
) -> Result<CertifiedRoot, VisualProfileIssueKind> {
    let first_span = first_source.span;
    let second_span = second_source.span;
    let ambiguity = || VisualProfileIssueKind::NumericalAmbiguity {
        first: first_span,
        second: second_span,
    };
    if matches!(
        (first.vertex, second.vertex),
        (Some(first), Some(second)) if first != second
    ) {
        return Err(ambiguity());
    }
    let first_seed = root_seed(first_source.parameters, first.first.include(second.first));
    let second_seed = root_seed(
        second_source.parameters,
        first.second.include(second.second),
    );
    if first_source.span == second_source.span && first_seed.overlaps(second_seed) {
        return Err(ambiguity());
    }
    let first_derivative = first_source
        .curve
        .derivative(first_seed)
        .map_err(|error| evaluation_issue(error, first_span, second_span))?;
    let second_derivative = second_source
        .curve
        .derivative(second_seed)
        .map_err(|error| evaluation_issue(error, first_span, second_span))?;
    let krawczyk = krawczyk_box(first_source, second_source, first_seed, second_seed)?
        .ok_or_else(ambiguity)?;
    let [first_k, second_k] = krawczyk.image;
    if !cross_interval(first_derivative, second_derivative).excludes_zero()
        || !first_seed.interior_contains(first_k)
        || !second_seed.interior_contains(second_k)
    {
        return Err(ambiguity());
    }
    let first_root = first_seed
        .intersection(first_k)
        .expect("strictly contained merged root");
    let second_root = second_seed
        .intersection(second_k)
        .expect("strictly contained merged root");
    let [first_root, second_root] =
        contract_krawczyk_root(first_source, second_source, first_root, second_root)?;
    let first_position = first_source
        .curve
        .position(first_root)
        .map_err(|error| evaluation_issue(error, first_span, second_span))?;
    let second_position = second_source
        .curve
        .position(second_root)
        .map_err(|error| evaluation_issue(error, first_span, second_span))?;
    Ok(CertifiedRoot {
        first_source: first.first_source,
        second_source: first.second_source,
        first: first_root,
        second: second_root,
        position: Box2 {
            x: first_position
                .x
                .intersection(second_position.x)
                .ok_or_else(ambiguity)?,
            y: first_position
                .y
                .intersection(second_position.y)
                .ok_or_else(ambiguity)?,
        },
        vertex: first.vertex.or(second.vertex),
    })
}

fn exact_overlap(first: &SourcePiece, second: &SourcePiece) -> bool {
    match (&first.curve.kind, &second.curve.kind) {
        (PieceKind::Linear { .. }, PieceKind::Linear { .. }) => {
            let (first_start, first_delta) =
                linear_geometry(first).expect("linear source has native geometry");
            let (second_start, second_delta) =
                linear_geometry(second).expect("linear source has native geometry");
            cross_interval(first_delta, second_delta).contains_zero()
                && cross_interval(
                    [
                        second_start[0].sub(first_start[0]),
                        second_start[1].sub(first_start[1]),
                    ],
                    first_delta,
                )
                .contains_zero()
                && linear_overlap(first, second)
        }
        _ => false,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CircularCarrierRelation {
    Disjoint,
    EndpointTouch,
    PositiveOverlap,
    Ambiguous,
}

#[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
fn same_circular_carrier_relation(
    first: &SourcePiece,
    second: &SourcePiece,
) -> Result<Option<CircularCarrierRelation>, VisualProfileIssueKind> {
    let (
        PieceKind::Circular {
            center: first_center,
            radius: first_radius,
            angle_rate: first_rate,
            ..
        },
        PieceKind::Circular {
            center: second_center,
            radius: second_radius,
            angle_rate: second_rate,
            ..
        },
    ) = (&first.curve.kind, &second.curve.kind)
    else {
        return Ok(None);
    };
    if first_center.map(f64::to_bits) != second_center.map(f64::to_bits)
        || first_radius.to_bits() != second_radius.to_bits()
    {
        return Ok(None);
    }
    let unresolved = || VisualProfileIssueKind::NumericalAmbiguity {
        first: first.span,
        second: second.span,
    };
    let (first_outer, first_inner) = circular_angle_extent(first).ok_or_else(unresolved)?;
    let (second_outer, second_inner) = circular_angle_extent(second).ok_or_else(unresolved)?;
    let windings = first_outer
        .sub(second_outer)
        .div(TAU_INTERVAL)
        .ok_or_else(unresolved)?;
    let first_winding = windings.lower.ceil();
    let last_winding = windings.upper.floor();
    if first_winding > last_winding {
        return Ok(Some(CircularCarrierRelation::Disjoint));
    }
    if !first_winding.is_finite()
        || !last_winding.is_finite()
        || first_winding.abs() > MAX_EXACT_F64_INTEGER
        || last_winding.abs() > MAX_EXACT_F64_INTEGER
        || last_winding - first_winding > 4.0
    {
        return Ok(Some(CircularCarrierRelation::Ambiguous));
    }
    let mut possible_contact = false;
    for winding in first_winding as i64..=last_winding as i64 {
        let shift = TAU_INTERVAL.scale(winding as f64);
        let shifted_outer = second_outer.add(shift);
        if !first_outer.overlaps(shifted_outer) {
            continue;
        }
        possible_contact = true;
        let guaranteed_shifted = Interval::checked(
            next_up(second_inner.lower + shift.upper),
            next_down(second_inner.upper + shift.lower),
        );
        if let Some(guaranteed_shifted) = guaranteed_shifted
            && first_inner.lower.max(guaranteed_shifted.lower)
                < first_inner.upper.min(guaranteed_shifted.upper)
        {
            return Ok(Some(CircularCarrierRelation::PositiveOverlap));
        }
    }
    if !possible_contact {
        return Ok(Some(CircularCarrierRelation::Disjoint));
    }

    let corners = shared_corners(first, second);
    if corners.is_empty() {
        return Ok(Some(CircularCarrierRelation::Ambiguous));
    }
    for (first_corner, second_corner, _) in corners {
        let Some(first_direction) = inward_parameter_direction(first.parameters, first_corner)
        else {
            return Ok(Some(CircularCarrierRelation::Ambiguous));
        };
        let Some(second_direction) = inward_parameter_direction(second.parameters, second_corner)
        else {
            return Ok(Some(CircularCarrierRelation::Ambiguous));
        };
        if (first_direction * first_rate).is_sign_positive()
            == (second_direction * second_rate).is_sign_positive()
        {
            return Ok(Some(CircularCarrierRelation::PositiveOverlap));
        }
    }
    let combined_span = circular_angle_span(first)
        .and_then(|first| circular_angle_span(second).map(|second| first.add(second)));
    Ok(Some(
        if combined_span.is_some_and(|span| span.upper < TAU_INTERVAL.lower) {
            CircularCarrierRelation::EndpointTouch
        } else {
            CircularCarrierRelation::Ambiguous
        },
    ))
}

fn circular_angle_extent(source: &SourcePiece) -> Option<(Interval, Interval)> {
    let PieceKind::Circular {
        angle_offset,
        angle_rate,
        ..
    } = source.curve.kind
    else {
        return None;
    };
    if !angle_offset.is_finite() || !angle_rate.is_finite() || angle_rate == 0.0 {
        return None;
    }
    let lower = angle_offset.add(Interval::scalar_product(
        source.parameters.lower,
        angle_rate,
    ));
    let upper = angle_offset.add(Interval::scalar_product(
        source.parameters.upper,
        angle_rate,
    ));
    let outer = lower.include(upper);
    let inner = if angle_rate > 0.0 {
        Interval::checked(lower.upper, upper.lower)
    } else {
        Interval::checked(upper.upper, lower.lower)
    }?;
    Some((outer, inner))
}

fn circular_angle_span(source: &SourcePiece) -> Option<Interval> {
    let PieceKind::Circular { angle_rate, .. } = source.curve.kind else {
        return None;
    };
    Some(
        Interval::point(source.parameters.upper)
            .sub(Interval::point(source.parameters.lower))
            .mul(Interval::point(angle_rate.abs())),
    )
}

fn linear_overlap(first: &SourcePiece, second: &SourcePiece) -> bool {
    let Some((first_start, first_direction)) = linear_geometry(first) else {
        return false;
    };
    let length_squared = dot_interval(first_direction, first_direction);
    if !length_squared.excludes_zero() {
        return false;
    }
    let projected = [second.parameters.lower, second.parameters.upper].map(|parameter| {
        let position = second
            .curve
            .position(Interval::point(parameter))
            .expect("certified source domain was preflighted");
        dot_interval(
            [
                position.x.sub(first_start[0]),
                position.y.sub(first_start[1]),
            ],
            first_direction,
        )
        .div(length_squared)
        .expect("nonzero line direction has finite squared length")
    });
    let second_inner = if projected[0].midpoint() <= projected[1].midpoint() {
        Interval::checked(projected[0].upper, projected[1].lower)
    } else {
        Interval::checked(projected[1].upper, projected[0].lower)
    };
    second_inner.is_some_and(|second_inner| {
        first.parameters.lower.max(second_inner.lower)
            < first.parameters.upper.min(second_inner.upper)
    })
}

fn linear_endpoint_contact_is_local(first: &SourcePiece, second: &SourcePiece) -> bool {
    let Some((_, first_direction)) = linear_geometry(first) else {
        return false;
    };
    let Some((_, second_direction)) = linear_geometry(second) else {
        return false;
    };
    let directions_are_local = |first_parameter: f64, second_parameter: f64| {
        let first_inward = if parameter_equal(first_parameter, first.parameters.lower) {
            1.0
        } else {
            -1.0
        };
        let second_inward = if parameter_equal(second_parameter, second.parameters.lower) {
            1.0
        } else {
            -1.0
        };
        let first_inward = first_direction.map(|value| value.scale(first_inward));
        let second_inward = second_direction.map(|value| value.scale(second_inward));
        cross_interval(first_inward, second_inward).excludes_zero()
            || dot_interval(first_inward, second_inward).upper < 0.0
    };
    if shared_corners(first, second)
        .iter()
        .any(|(first_parameter, second_parameter, _)| {
            directions_are_local(*first_parameter, *second_parameter)
        })
    {
        return true;
    }
    let first_endpoints = [
        (first.parameters.lower, first.start_position),
        (first.parameters.upper, first.end_position),
    ];
    let second_endpoints = [
        (second.parameters.lower, second.start_position),
        (second.parameters.upper, second.end_position),
    ];
    first_endpoints
        .into_iter()
        .any(|(first_parameter, first_position)| {
            second_endpoints
                .iter()
                .copied()
                .any(|(second_parameter, second_position)| {
                    if !parameter_equal(first_position[0], second_position[0])
                        || !parameter_equal(first_position[1], second_position[1])
                    {
                        return false;
                    }
                    directions_are_local(first_parameter, second_parameter)
                })
        })
}

fn shared_corners(first: &SourcePiece, second: &SourcePiece) -> Vec<(f64, f64, VertexKey)> {
    let first_endpoints = [
        (first.parameters.lower, first.start),
        (first.parameters.upper, first.end),
    ];
    let second_endpoints = [
        (second.parameters.lower, second.start),
        (second.parameters.upper, second.end),
    ];
    first_endpoints
        .into_iter()
        .flat_map(|(first_parameter, first_vertex)| {
            second_endpoints
                .into_iter()
                .filter_map(move |(second_parameter, second_vertex)| {
                    (first_vertex == second_vertex).then_some((
                        first_parameter,
                        second_parameter,
                        first_vertex,
                    ))
                })
        })
        .collect()
}

fn has_owned_join_corner(
    first: &SourcePiece,
    second: &SourcePiece,
    joins: &BTreeSet<ExplicitFilletJoin>,
) -> bool {
    shared_corners(first, second)
        .iter()
        .any(|(_, _, vertex)| owned_join(first.span, second.span, *vertex, joins))
}

fn intersection_domains_excluding_owned_joins(
    first: &SourcePiece,
    second: &SourcePiece,
    joins: &BTreeSet<ExplicitFilletJoin>,
    work: &mut Work,
) -> Result<Option<IntersectionDomains>, VisualProfileIssueKind> {
    let mut first_parameters = first.parameters;
    let mut second_parameters = second.parameters;
    let mut remaining = Vec::new();
    for (first_corner, second_corner, vertex) in shared_corners(first, second) {
        if !owned_join(first.span, second.span, vertex, joins) {
            remaining.push((first_corner, second_corner, vertex));
            continue;
        }
        match certified_owned_tangent_collar(
            first,
            second,
            first_parameters,
            second_parameters,
            first_corner,
            second_corner,
            work,
        )? {
            Some(OwnedDomainTrim::First(parameters)) => first_parameters = parameters,
            Some(OwnedDomainTrim::Second(parameters)) => second_parameters = parameters,
            None => remaining.push((first_corner, second_corner, vertex)),
        }
    }
    Ok(Some((first_parameters, second_parameters, remaining)))
}

fn owned_join(
    first: CurveSpan,
    second: CurveSpan,
    vertex: VertexKey,
    joins: &BTreeSet<ExplicitFilletJoin>,
) -> bool {
    let (first, second) = if first <= second {
        (first, second)
    } else {
        (second, first)
    };
    joins.contains(&ExplicitFilletJoin {
        first,
        second,
        vertex,
    })
}

enum OwnedDomainTrim {
    First(Interval),
    Second(Interval),
}

#[allow(clippy::too_many_arguments)]
fn certified_owned_tangent_collar(
    first: &SourcePiece,
    second: &SourcePiece,
    first_domain: Interval,
    second_domain: Interval,
    first_endpoint: f64,
    second_endpoint: f64,
    work: &mut Work,
) -> Result<Option<OwnedDomainTrim>, VisualProfileIssueKind> {
    if first.curve.is_linear() && matches!(second.curve.kind, PieceKind::Circular { .. }) {
        return certify_line_circular_collar(
            first,
            second,
            first_endpoint,
            second_endpoint,
            second_domain,
            work,
        )
        .map(|value| value.map(OwnedDomainTrim::Second));
    }
    if second.curve.is_linear() && matches!(first.curve.kind, PieceKind::Circular { .. }) {
        return certify_line_circular_collar(
            second,
            first,
            second_endpoint,
            first_endpoint,
            first_domain,
            work,
        )
        .map(|value| value.map(OwnedDomainTrim::First));
    }
    if matches!(first.curve.kind, PieceKind::Circular { .. })
        && matches!(second.curve.kind, PieceKind::Circular { .. })
    {
        if let Some(parameters) = certify_circular_circular_collar(
            first,
            second,
            first_endpoint,
            second_endpoint,
            first_domain,
            work,
        )? {
            return Ok(Some(OwnedDomainTrim::First(parameters)));
        }
        return certify_circular_circular_collar(
            second,
            first,
            second_endpoint,
            first_endpoint,
            second_domain,
            work,
        )
        .map(|value| value.map(OwnedDomainTrim::Second));
    }
    Ok(None)
}

fn certify_line_circular_collar(
    line: &SourcePiece,
    circular: &SourcePiece,
    line_endpoint: f64,
    circular_endpoint: f64,
    circular_domain: Interval,
    work: &mut Work,
) -> Result<Option<Interval>, VisualProfileIssueKind> {
    let Some((line_start, line_direction)) = linear_geometry(line) else {
        return Ok(None);
    };
    let line_position = line
        .curve
        .position(Interval::point(line_endpoint))
        .map_err(|error| evaluation_issue(error, line.span, circular.span))?;
    let circular_position = circular
        .curve
        .position(Interval::point(circular_endpoint))
        .map_err(|error| evaluation_issue(error, line.span, circular.span))?;
    if !line_position.x.overlaps(circular_position.x)
        || !line_position.y.overlaps(circular_position.y)
    {
        return Ok(None);
    }
    let value = cross_interval(
        [
            circular_position.x.sub(line_start[0]),
            circular_position.y.sub(line_start[1]),
        ],
        line_direction,
    );
    let first_derivative = circular
        .curve
        .derivative(Interval::point(circular_endpoint))
        .map_err(|error| evaluation_issue(error, line.span, circular.span))?;
    if !value.contains_zero() || !cross_interval(first_derivative, line_direction).contains_zero() {
        return Ok(None);
    }
    certify_tangent_collar(
        circular,
        circular_domain,
        circular_endpoint,
        line.span,
        work,
        |interval| {
            let second_derivative = circular_second_derivative(circular, interval)?;
            Ok(cross_interval(second_derivative, line_direction))
        },
    )
}

fn certify_circular_circular_collar(
    source: &SourcePiece,
    carrier: &SourcePiece,
    source_endpoint: f64,
    carrier_endpoint: f64,
    source_domain: Interval,
    work: &mut Work,
) -> Result<Option<Interval>, VisualProfileIssueKind> {
    let PieceKind::Circular { center, radius, .. } = &carrier.curve.kind else {
        return Ok(None);
    };
    let source_position = source
        .curve
        .position(Interval::point(source_endpoint))
        .map_err(|error| evaluation_issue(error, source.span, carrier.span))?;
    let carrier_position = carrier
        .curve
        .position(Interval::point(carrier_endpoint))
        .map_err(|error| evaluation_issue(error, source.span, carrier.span))?;
    if !source_position.x.overlaps(carrier_position.x)
        || !source_position.y.overlaps(carrier_position.y)
    {
        return Ok(None);
    }
    let offset = [
        source_position.x.sub(Interval::point(center[0])),
        source_position.y.sub(Interval::point(center[1])),
    ];
    let derivative = source
        .curve
        .derivative(Interval::point(source_endpoint))
        .map_err(|error| evaluation_issue(error, source.span, carrier.span))?;
    let value = dot_interval(offset, offset).sub(Interval::point(*radius).square());
    let first_derivative = dot_interval(offset, derivative).scale(2.0);
    if !value.contains_zero() || !first_derivative.contains_zero() {
        return Ok(None);
    }
    certify_tangent_collar(
        source,
        source_domain,
        source_endpoint,
        carrier.span,
        work,
        |interval| {
            let position = source
                .curve
                .position(interval)
                .map_err(|error| evaluation_issue(error, source.span, carrier.span))?;
            let offset = [
                position.x.sub(Interval::point(center[0])),
                position.y.sub(Interval::point(center[1])),
            ];
            let derivative = source
                .curve
                .derivative(interval)
                .map_err(|error| evaluation_issue(error, source.span, carrier.span))?;
            let second_derivative = circular_second_derivative(source, interval)?;
            Ok(dot_interval(derivative, derivative)
                .add(dot_interval(offset, second_derivative))
                .scale(2.0))
        },
    )
}

fn certify_tangent_collar(
    source: &SourcePiece,
    domain: Interval,
    endpoint: f64,
    other_span: CurveSpan,
    work: &mut Work,
    mut second_derivative: impl FnMut(Interval) -> Result<Interval, VisualProfileIssueKind>,
) -> Result<Option<Interval>, VisualProfileIssueKind> {
    let direction = inward_parameter_direction(domain, endpoint);
    let Some(direction) = direction else {
        return Ok(None);
    };
    let mut width = domain.width() / 16.0;
    for _ in 0..32 {
        if work.intersection_subdivisions >= work.options.max_intersection_subdivisions {
            return Err(
                VisualProfileIssueKind::IntersectionSubdivisionBudgetExceeded {
                    first: source.span,
                    second: other_span,
                    limit: work.options.max_intersection_subdivisions,
                },
            );
        }
        if !work.charge_operation(
            OperationWorkCounter::ProfileSubdivisions,
            1,
            OperationCheckpoint::ProfileSubdivision,
        ) {
            return Err(
                VisualProfileIssueKind::IntersectionSubdivisionBudgetExceeded {
                    first: source.span,
                    second: other_span,
                    limit: work.options.max_intersection_subdivisions,
                },
            );
        }
        work.intersection_subdivisions += 1;
        let interior = endpoint + direction * width;
        if !interior.is_finite() || interior.to_bits() == endpoint.to_bits() {
            width *= 0.5;
            continue;
        }
        let collar = Interval::hull(endpoint, interior);
        if second_derivative(collar)?.excludes_zero() {
            return Ok(if direction > 0.0 {
                Interval::checked(collar.upper, domain.upper)
            } else {
                Interval::checked(domain.lower, collar.lower)
            });
        }
        width *= 0.5;
    }
    Ok(None)
}

fn circular_second_derivative(
    source: &SourcePiece,
    parameter: Interval,
) -> Result<[Interval; 2], VisualProfileIssueKind> {
    let PieceKind::Circular {
        radius,
        angle_offset,
        angle_rate,
        ..
    } = &source.curve.kind
    else {
        return Err(VisualProfileIssueKind::UnresolvedIntersection {
            first: source.span,
            second: source.span,
        });
    };
    let angle = angle_offset.add(parameter.mul(Interval::point(*angle_rate)));
    let scale = Interval::point(*radius)
        .mul(Interval::point(*angle_rate).square())
        .neg();
    Ok([
        angle
            .cos()
            .map_err(|_| VisualProfileIssueKind::UnresolvedIntersection {
                first: source.span,
                second: source.span,
            })?
            .mul(scale),
        angle
            .sin()
            .map_err(|_| VisualProfileIssueKind::UnresolvedIntersection {
                first: source.span,
                second: source.span,
            })?
            .mul(scale),
    ])
}

fn dot_interval(first: [Interval; 2], second: [Interval; 2]) -> Interval {
    first[0].mul(second[0]).add(first[1].mul(second[1]))
}

fn excluded_line_curve_corner_is_local(
    line: &SourcePiece,
    curve: &SourcePiece,
    parameter: Interval,
    signed_derivative: Interval,
) -> Result<bool, VisualProfileIssueKind> {
    let Some((_, line_derivative)) = linear_geometry(line) else {
        return Ok(false);
    };
    for (line_parameter, curve_parameter, _) in shared_corners(line, curve) {
        let Some(curve_direction) = inward_parameter_direction(parameter, curve_parameter) else {
            continue;
        };
        if signed_derivative.excludes_zero() {
            return Ok(true);
        }
        let line_direction = if parameter_equal(line_parameter, line.parameters.lower) {
            1.0
        } else {
            -1.0
        };
        let line_inward = line_derivative.map(|value| value.scale(line_direction));
        let curve_inward = curve
            .curve
            .derivative(parameter)
            .map_err(|error| evaluation_issue(error, line.span, curve.span))?
            .map(|value| value.scale(curve_direction));
        if dot_interval(line_inward, curve_inward).upper < 0.0 {
            return Ok(true);
        }
    }
    Ok(false)
}

fn excluded_corner_is_local(
    corners: &[(f64, f64, VertexKey)],
    first_source: &SourcePiece,
    second_source: &SourcePiece,
    first_parameter: Interval,
    second_parameter: Interval,
) -> Result<bool, VisualProfileIssueKind> {
    for (first_corner, second_corner, _) in corners {
        let Some(first_direction) = inward_parameter_direction(first_parameter, *first_corner)
        else {
            continue;
        };
        let Some(second_direction) = inward_parameter_direction(second_parameter, *second_corner)
        else {
            continue;
        };
        let first_derivative = first_source
            .curve
            .derivative(first_parameter)
            .map_err(|error| evaluation_issue(error, first_source.span, second_source.span))?
            .map(|value| value.scale(first_direction));
        let second_derivative = second_source
            .curve
            .derivative(second_parameter)
            .map_err(|error| evaluation_issue(error, first_source.span, second_source.span))?
            .map(|value| value.scale(second_direction));
        let first_tangent = first_derivative.map(Interval::midpoint);
        let second_tangent = second_derivative.map(Interval::midpoint);
        let first_norm = norm(first_tangent);
        let second_norm = norm(second_tangent);
        if first_norm == 0.0 || second_norm == 0.0 {
            continue;
        }
        let separator = subtract(
            scale(first_tangent, 1.0 / first_norm),
            scale(second_tangent, 1.0 / second_norm),
        );
        if separator == [0.0, 0.0] {
            continue;
        }
        let first_projection = first_derivative[0]
            .scale(separator[0])
            .add(first_derivative[1].scale(separator[1]));
        let second_projection = second_derivative[0]
            .scale(separator[0])
            .add(second_derivative[1].scale(separator[1]));
        if first_projection.lower > 0.0 && second_projection.upper < 0.0
            || first_projection.upper < 0.0 && second_projection.lower > 0.0
        {
            return Ok(true);
        }
    }
    Ok(false)
}

fn inward_parameter_direction(parameter: Interval, corner: f64) -> Option<f64> {
    if parameter_equal(parameter.lower, corner) && !parameter_equal(parameter.upper, corner) {
        Some(1.0)
    } else if parameter_equal(parameter.upper, corner) && !parameter_equal(parameter.lower, corner)
    {
        Some(-1.0)
    } else {
        None
    }
}

fn evaluation_issue(
    error: PieceEvaluationError,
    first: CurveSpan,
    second: CurveSpan,
) -> VisualProfileIssueKind {
    match error {
        PieceEvaluationError::Pole => VisualProfileIssueKind::RationalPole { support: first },
        PieceEvaluationError::NonFinite => {
            VisualProfileIssueKind::UnresolvedIntersection { first, second }
        }
    }
}

fn normalized_width(value: Interval, domain: Interval) -> f64 {
    value.width() / domain.width()
}

fn intersection_subdivision_point(value: Interval) -> f64 {
    // A symmetric split can place a transverse root on every descendant boundary,
    // preventing the strict-interior Krawczyk proof from ever succeeding.
    0.503_906_25_f64.mul_add(value.width(), value.lower)
}

#[allow(clippy::too_many_lines)]
fn extract_cycles(
    fragments: &[Fragment],
    sources: &[SourcePiece],
    model_scale: f64,
    work: &mut Work,
) -> (Vec<Cycle>, Vec<(usize, VisualProfileIssueKind)>) {
    if fragments.is_empty() {
        return (Vec::new(), Vec::new());
    }
    let mut vertex_ids = BTreeMap::new();
    for fragment in fragments {
        for vertex in [fragment.start, fragment.end] {
            let next = vertex_ids.len();
            vertex_ids.entry(vertex).or_insert(next);
        }
    }
    let bridges = bridge_fragments(vertex_ids.len(), fragments, &vertex_ids);
    let mut nesting_components = DisjointSet::new(vertex_ids.len());
    for (fragment_index, fragment) in fragments.iter().enumerate() {
        if !bridges.contains(&fragment_index) {
            nesting_components.union(vertex_ids[&fragment.start], vertex_ids[&fragment.end]);
        }
    }
    let nesting_roots = (0..vertex_ids.len())
        .map(|vertex| nesting_components.root(vertex))
        .collect::<Vec<_>>();
    let mut outgoing = vec![Vec::<usize>::new(); vertex_ids.len()];
    let mut half_from = Vec::new();
    let mut half_to = Vec::new();
    let mut half_fragment = Vec::new();
    for (fragment_index, fragment) in fragments.iter().enumerate() {
        if bridges.contains(&fragment_index) {
            continue;
        }
        let first = half_from.len();
        let start = vertex_ids[&fragment.start];
        let end = vertex_ids[&fragment.end];
        half_from.extend([start, end]);
        half_to.extend([end, start]);
        half_fragment.extend([fragment_index, fragment_index]);
        outgoing[start].push(first);
        outgoing[end].push(first + 1);
    }
    let mut issues = Vec::new();
    for edges in &mut outgoing {
        let mut angles = Vec::with_capacity(edges.len());
        for edge in edges.iter().copied() {
            let fragment = &fragments[half_fragment[edge]];
            match outgoing_angle(fragment, edge & 1 == 0, sources) {
                Ok(angle) => angles.push((edge, angle)),
                Err(kind) => issues.push((fragment.component, kind)),
            }
        }
        angles.sort_by(|first, second| {
            first
                .1
                .midpoint()
                .total_cmp(&second.1.midpoint())
                .then_with(|| half_fragment[first.0].cmp(&half_fragment[second.0]))
                .then_with(|| first.0.cmp(&second.0))
        });
        if angles.len() > 2 {
            for pair in angles.windows(2) {
                if pair[0].1.overlaps(pair[1].1) {
                    let fragment = &fragments[half_fragment[pair[0].0]];
                    issues.push((
                        fragment.component,
                        VisualProfileIssueKind::UnresolvedTangentOrder {
                            support: fragment.source_span,
                        },
                    ));
                }
            }
            if let (Some(first), Some(last)) = (angles.first(), angles.last()) {
                let wrapped_first = first.1.add(TAU_INTERVAL);
                if last.1.overlaps(wrapped_first) {
                    let fragment = &fragments[half_fragment[first.0]];
                    issues.push((
                        fragment.component,
                        VisualProfileIssueKind::UnresolvedTangentOrder {
                            support: fragment.source_span,
                        },
                    ));
                }
            }
        }
        *edges = angles.into_iter().map(|(edge, _)| edge).collect();
    }
    issues.sort_by_key(|issue| issue.0);
    issues.dedup();
    let bad = issues
        .iter()
        .map(|(component, _)| *component)
        .collect::<BTreeSet<_>>();

    let mut next = vec![0; half_from.len()];
    for half in 0..half_from.len() {
        let destination = half_to[half];
        let twin = half ^ 1;
        let edges = &outgoing[destination];
        let Some(reverse) = edges.iter().position(|candidate| *candidate == twin) else {
            continue;
        };
        next[half] = edges[(reverse + edges.len() - 1) % edges.len()];
    }
    let mut visited = vec![false; half_from.len()];
    let mut cycles = Vec::new();
    for start in 0..half_from.len() {
        if visited[start] {
            continue;
        }
        let component = fragments[half_fragment[start]].component;
        if bad.contains(&component) {
            continue;
        }
        let mut half = start;
        let mut directed = Vec::new();
        for _ in 0..=half_from.len() {
            if visited[half] {
                break;
            }
            visited[half] = true;
            let fragment = half_fragment[half];
            directed.push(DirectedFragment {
                fragment,
                forward: half & 1 == 0,
            });
            half = next[half];
            if half == start {
                break;
            }
        }
        if half != start || directed.len() < 2 {
            continue;
        }
        let origin = if directed[0].forward {
            fragments[directed[0].fragment].start_position
        } else {
            fragments[directed[0].fragment].end_position
        };
        // Edge-local integration budgets must sum to the cycle-level display bound.
        let display_target = model_scale * model_scale * AREA_DISPLAY_RELATIVE_TARGET;
        let edge_count = u32::try_from(directed.len()).unwrap_or(u32::MAX);
        let fragment_width_target = display_target / f64::from(edge_count);
        let mut area = Interval::ZERO;
        let mut bounds: Option<Box2> = None;
        let mut failed = None;
        for edge in &directed {
            let fragment = &fragments[edge.fragment];
            let source = &sources[fragment.source];
            let parameter = Interval::hull(
                fragment.source_parameter_enclosures[0].lower,
                fragment.source_parameter_enclosures[1].upper,
            );
            match source.curve.position(parameter) {
                Ok(edge_bounds) => {
                    bounds = Some(bounds.map_or(edge_bounds, |value| value.include(edge_bounds)));
                }
                Err(error) => {
                    failed = Some(evaluation_issue(error, source.span, source.span));
                    break;
                }
            }
            match integrate_fragment(
                source,
                fragment.source_parameters,
                fragment.source_parameter_enclosures,
                origin,
                fragment_width_target,
                work,
            ) {
                Ok(value) => {
                    area = area.add(if edge.forward { value } else { value.neg() });
                }
                Err(kind) => {
                    failed = Some(kind);
                    break;
                }
            }
        }
        if let Some(kind) = failed {
            issues.push((component, kind));
            continue;
        }
        let area_tolerance = 512.0 * f64::EPSILON * model_scale * model_scale;
        if !area.is_finite()
            || !display_target.is_finite()
            || display_target <= 0.0
            || interval_uncertainty(area) > display_target
            || area.lower <= area_tolerance && area.upper >= -area_tolerance
        {
            issues.push((
                component,
                VisualProfileIssueKind::AreaUncertainty {
                    support: fragments[directed[0].fragment].source_span,
                },
            ));
            continue;
        }
        if area.lower > 0.0 {
            let representative_area = if directed
                .iter()
                .all(|edge| sources[fragments[edge.fragment].source].curve.is_linear())
            {
                polygon_area(
                    &directed
                        .iter()
                        .map(|edge| {
                            let fragment = &fragments[edge.fragment];
                            if edge.forward {
                                fragment.start_position
                            } else {
                                fragment.end_position
                            }
                        })
                        .collect::<Vec<_>>(),
                )
            } else {
                area.midpoint()
            };
            if !representative_area.is_finite() || !area.contains(representative_area) {
                issues.push((
                    component,
                    VisualProfileIssueKind::AreaUncertainty {
                        support: fragments[directed[0].fragment].source_span,
                    },
                ));
                continue;
            }
            cycles.push(Cycle {
                component,
                nesting_component: nesting_roots[half_from[start]],
                area,
                representative_area,
                bounds: bounds.expect("cycle has edges"),
                edges: directed,
            });
        }
    }
    issues.sort_by_key(|issue| issue.0);
    issues.dedup();
    (cycles, issues)
}

fn outgoing_angle(
    fragment: &Fragment,
    forward: bool,
    sources: &[SourcePiece],
) -> Result<Interval, VisualProfileIssueKind> {
    let parameter = if forward {
        fragment.source_parameter_enclosures[0]
    } else {
        fragment.source_parameter_enclosures[1]
    };
    let source = &sources[fragment.source];
    let mut derivative =
        source
            .curve
            .derivative(parameter)
            .map_err(|_| VisualProfileIssueKind::ZeroSpeed {
                support: source.span,
            })?;
    if !forward {
        derivative = derivative.map(Interval::neg);
    }
    if derivative[0].contains_zero() && derivative[1].contains_zero() {
        return Err(VisualProfileIssueKind::UnresolvedTangentOrder {
            support: source.span,
        });
    }
    atan2_box(derivative[1], derivative[0]).map_err(|_| {
        VisualProfileIssueKind::UnresolvedTangentOrder {
            support: source.span,
        }
    })
}

fn integrate_fragment(
    source: &SourcePiece,
    parameters: [f64; 2],
    parameter_enclosures: [Interval; 2],
    origin: [f64; 2],
    target: f64,
    work: &mut Work,
) -> Result<Interval, VisualProfileIssueKind> {
    if !target.is_finite() || target <= 0.0 {
        return Err(VisualProfileIssueKind::AreaUncertainty {
            support: source.span,
        });
    }
    let nominal_parameter = Interval::hull(parameters[0], parameters[1]);
    let mut total = if let Some(value) = source
        .curve
        .exact_area(nominal_parameter, origin)
        .map_err(|error| area_evaluation_issue(error, source.span))?
    {
        value
    } else {
        let mut stack = vec![(nominal_parameter, 0_usize)];
        let mut nominal = Interval::ZERO;
        while let Some((interval, depth)) = stack.pop() {
            let contribution = simpson_area_enclosure(source, interval, origin)?;
            let local_target = target * interval.width() / nominal_parameter.width();
            if contribution.width() <= local_target {
                nominal = nominal.add(contribution);
                continue;
            }
            if depth >= work.options.max_intersection_depth {
                return Err(VisualProfileIssueKind::AreaUncertainty {
                    support: source.span,
                });
            }
            if work.integration_subdivisions >= work.options.max_integration_subdivisions {
                return Err(VisualProfileIssueKind::IntegrationBudgetExceeded {
                    support: source.span,
                    limit: work.options.max_integration_subdivisions,
                });
            }
            let middle = interval.midpoint();
            if middle.to_bits() == interval.lower.to_bits()
                || middle.to_bits() == interval.upper.to_bits()
            {
                return Err(VisualProfileIssueKind::AreaUncertainty {
                    support: source.span,
                });
            }
            if !work.charge_operation(
                OperationWorkCounter::ProfileIntegrations,
                1,
                OperationCheckpoint::ProfileIntegration,
            ) {
                return Err(VisualProfileIssueKind::IntegrationBudgetExceeded {
                    support: source.span,
                    limit: work.options.max_integration_subdivisions,
                });
            }
            work.integration_subdivisions += 1;
            stack.push((Interval::hull(middle, interval.upper), depth + 1));
            stack.push((Interval::hull(interval.lower, middle), depth + 1));
        }
        nominal
    };

    for (index, (representative, enclosure)) in
        parameters.into_iter().zip(parameter_enclosures).enumerate()
    {
        if !enclosure.contains(representative) {
            return Err(VisualProfileIssueKind::AreaUncertainty {
                support: source.span,
            });
        }
        if enclosure.lower.to_bits() == representative.to_bits()
            && enclosure.upper.to_bits() == representative.to_bits()
        {
            continue;
        }
        let endpoint_domain = enclosure.include(Interval::point(representative));
        let integrand = source
            .curve
            .area_integrand(endpoint_domain, origin)
            .map_err(|error| area_evaluation_issue(error, source.span))?;
        let displacement = enclosure.sub(Interval::point(representative));
        let correction = integrand.mul(displacement);
        total = if index == 0 {
            total.sub(correction)
        } else {
            total.add(correction)
        };
    }
    total
        .is_finite()
        .then_some(total)
        .ok_or(VisualProfileIssueKind::AreaUncertainty {
            support: source.span,
        })
}

fn simpson_area_enclosure(
    source: &SourcePiece,
    parameter: Interval,
    origin: [f64; 2],
) -> Result<Interval, VisualProfileIssueKind> {
    let middle = parameter.midpoint();
    let lower = source
        .curve
        .area_integrand(Interval::point(parameter.lower), origin)
        .map_err(|error| area_evaluation_issue(error, source.span))?;
    let center = source
        .curve
        .area_integrand(Interval::point(middle), origin)
        .map_err(|error| area_evaluation_issue(error, source.span))?;
    let upper = source
        .curve
        .area_integrand(Interval::point(parameter.upper), origin)
        .map_err(|error| area_evaluation_issue(error, source.span))?;
    let width = Interval::point(parameter.upper).sub(Interval::point(parameter.lower));
    let estimate = lower
        .add(center.scale(4.0))
        .add(upper)
        .mul(width)
        .div(Interval::point(6.0))
        .ok_or(VisualProfileIssueKind::AreaUncertainty {
            support: source.span,
        })?;
    let fourth = source
        .curve
        .area_integrand_fourth_derivative(parameter, origin)
        .map_err(|error| area_evaluation_issue(error, source.span))?
        .ok_or(VisualProfileIssueKind::AreaUncertainty {
            support: source.span,
        })?;
    let derivative_bound = fourth.lower.abs().max(fourth.upper.abs());
    let error = width
        .powi(5)
        .mul(Interval::point(derivative_bound))
        .div(Interval::point(2880.0))
        .ok_or(VisualProfileIssueKind::AreaUncertainty {
            support: source.span,
        })?;
    Ok(estimate.add(Interval {
        lower: -error.upper,
        upper: error.upper,
    }))
}

fn area_evaluation_issue(
    error: PieceEvaluationError,
    support: CurveSpan,
) -> VisualProfileIssueKind {
    match error {
        PieceEvaluationError::Pole => VisualProfileIssueKind::RationalPole { support },
        PieceEvaluationError::NonFinite => VisualProfileIssueKind::AreaUncertainty { support },
    }
}

fn bridge_fragments(
    vertex_count: usize,
    fragments: &[Fragment],
    vertex_ids: &BTreeMap<VertexKey, usize>,
) -> BTreeSet<usize> {
    let mut adjacency = vec![Vec::<(usize, usize)>::new(); vertex_count];
    for (edge, fragment) in fragments.iter().enumerate() {
        let start = vertex_ids[&fragment.start];
        let end = vertex_ids[&fragment.end];
        adjacency[start].push((end, edge));
        adjacency[end].push((start, edge));
    }
    let mut discovery = vec![usize::MAX; vertex_count];
    let mut low = vec![usize::MAX; vertex_count];
    let mut parent_vertex = vec![usize::MAX; vertex_count];
    let mut parent_edge = vec![usize::MAX; vertex_count];
    let mut clock = 0;
    let mut bridges = BTreeSet::new();
    for root in 0..vertex_count {
        if discovery[root] != usize::MAX {
            continue;
        }
        discovery[root] = clock;
        low[root] = clock;
        clock += 1;
        let mut stack = vec![(root, 0_usize)];
        while let Some((vertex, next_index)) = stack.last_mut() {
            if let Some(&(next, edge)) = adjacency[*vertex].get(*next_index) {
                *next_index += 1;
                if edge == parent_edge[*vertex] {
                    continue;
                }
                if discovery[next] == usize::MAX {
                    parent_vertex[next] = *vertex;
                    parent_edge[next] = edge;
                    discovery[next] = clock;
                    low[next] = clock;
                    clock += 1;
                    stack.push((next, 0));
                } else {
                    low[*vertex] = low[*vertex].min(discovery[next]);
                }
            } else {
                let completed = *vertex;
                stack.pop();
                let parent = parent_vertex[completed];
                if parent != usize::MAX {
                    low[parent] = low[parent].min(low[completed]);
                    if low[completed] > discovery[parent] {
                        bridges.insert(parent_edge[completed]);
                    }
                }
            }
        }
    }
    bridges
}

#[allow(clippy::too_many_lines)]
fn build_faces(
    cycles: &[Cycle],
    fragments: &[Fragment],
    sources: &[SourcePiece],
    work: &mut Work,
    model_scale: f64,
    face_limit: usize,
) -> Result<Vec<VisualProfileFace>, FaceBuildError> {
    let mut component_cycles = BTreeMap::<usize, Vec<usize>>::new();
    let mut component_bounds = BTreeMap::<usize, Box2>::new();
    for (index, cycle) in cycles.iter().enumerate() {
        component_cycles
            .entry(cycle.nesting_component)
            .or_default()
            .push(index);
        component_bounds
            .entry(cycle.nesting_component)
            .and_modify(|bounds| *bounds = bounds.include(cycle.bounds))
            .or_insert(cycle.bounds);
    }
    let mut parents = vec![None; cycles.len()];
    for (index, cycle) in cycles.iter().enumerate() {
        let mut parent: Option<usize> = None;
        for (component, candidates) in &component_cycles {
            if *component == cycle.nesting_component {
                continue;
            }
            charge_containment(work).map_err(FaceBuildError::Global)?;
            if !component_bounds[component].contains_box(cycle.bounds) {
                continue;
            }
            for candidate in candidates {
                charge_containment(work).map_err(FaceBuildError::Global)?;
                if !cycles[*candidate].bounds.contains_box(cycle.bounds) {
                    continue;
                }
                let contains =
                    strictly_contains_cycle(&cycles[*candidate], cycle, fragments, sources, work)
                        .map_err(|kind| {
                        face_build_error(kind, vec![cycles[*candidate].component, cycle.component])
                    })?;
                if !contains {
                    continue;
                }
                if cycles[*candidate].area.lower <= cycle.area.upper {
                    return Err(FaceBuildError::Local {
                        kind: VisualProfileIssueKind::AreaUncertainty {
                            support: fragments[cycle.edges[0].fragment].source_span,
                        },
                        components: vec![cycles[*candidate].component, cycle.component],
                    });
                }
                if let Some(current) = parent {
                    if cycles[*candidate].area.upper < cycles[current].area.lower {
                        parent = Some(*candidate);
                    } else if cycles[current].area.upper >= cycles[*candidate].area.lower {
                        return Err(FaceBuildError::Local {
                            kind: VisualProfileIssueKind::AreaUncertainty {
                                support: fragments[cycle.edges[0].fragment].source_span,
                            },
                            components: vec![
                                cycles[*candidate].component,
                                cycles[current].component,
                                cycle.component,
                            ],
                        });
                    }
                } else {
                    parent = Some(*candidate);
                }
            }
        }
        parents[index] = parent;
    }
    let mut children = vec![Vec::new(); cycles.len()];
    for (child, parent) in parents.iter().enumerate() {
        if let Some(parent) = parent {
            children[*parent].push(child);
        }
    }
    for values in &mut children {
        values.sort_unstable();
    }
    let mut faces = Vec::new();
    for (index, cycle) in cycles.iter().enumerate().take(face_limit) {
        if !work.charge_operation(
            OperationWorkCounter::ProfileFaces,
            1,
            OperationCheckpoint::ProfileFace,
        ) {
            return Err(FaceBuildError::Global(
                VisualProfileIssueKind::FaceBudgetExceeded {
                    required: work.faces.saturating_add(1),
                    limit: work.options.max_faces,
                },
            ));
        }
        let visual_area = children[index]
            .iter()
            .fold(cycle.area, |area, child| area.sub(cycles[*child].area));
        let display_target = model_scale * model_scale * AREA_DISPLAY_RELATIVE_TARGET;
        if visual_area.lower <= 0.0
            || !visual_area.is_finite()
            || !display_target.is_finite()
            || display_target <= 0.0
            || interval_uncertainty(visual_area) > display_target
        {
            let mut components = vec![cycle.component];
            components.extend(children[index].iter().map(|child| cycles[*child].component));
            return Err(FaceBuildError::Local {
                kind: VisualProfileIssueKind::AreaUncertainty {
                    support: fragments[cycle.edges[0].fragment].source_span,
                },
                components,
            });
        }
        let mut contours = vec![cycle_contour(cycle, fragments, false)];
        contours.extend(
            children[index]
                .iter()
                .map(|child| cycle_contour(&cycles[*child], fragments, true)),
        );
        let mut containment_contours = vec![cycle_containment_edges(cycle, fragments, sources)];
        containment_contours.extend(
            children[index]
                .iter()
                .map(|child| cycle_containment_edges(&cycles[*child], fragments, sources)),
        );
        faces.push(VisualProfileFace {
            contours,
            visual_area: children[index]
                .iter()
                .fold(cycle.representative_area, |area, child| {
                    area - cycles[*child].representative_area
                }),
            area_uncertainty: interval_uncertainty(visual_area),
            containment_contours,
        });
        work.faces += 1;
    }
    Ok(faces)
}

fn face_build_error(kind: VisualProfileIssueKind, components: Vec<usize>) -> FaceBuildError {
    if matches!(
        kind,
        VisualProfileIssueKind::ContainmentBudgetExceeded { .. }
    ) {
        FaceBuildError::Global(kind)
    } else {
        FaceBuildError::Local { kind, components }
    }
}

fn strictly_contains_cycle(
    parent: &Cycle,
    child: &Cycle,
    fragments: &[Fragment],
    sources: &[SourcePiece],
    work: &mut Work,
) -> Result<bool, VisualProfileIssueKind> {
    for edge in &child.edges {
        let fragment = &fragments[edge.fragment];
        let lower = fragment.source_parameter_enclosures[0].upper;
        let upper = fragment.source_parameter_enclosures[1].lower;
        if lower >= upper {
            continue;
        }
        let parameter = lower + 0.5 * (upper - lower);
        if parameter.to_bits() == lower.to_bits() || parameter.to_bits() == upper.to_bits() {
            continue;
        }
        let witness = sources[fragment.source]
            .curve
            .position(Interval::point(parameter))
            .map_err(|_| VisualProfileIssueKind::ContainmentAmbiguity {
                support: fragment.source_span,
            })?;
        return match point_in_cycle(witness, parent, fragments, sources, work)? {
            VisualProfilePointContainment::Inside => Ok(true),
            VisualProfilePointContainment::Outside => Ok(false),
            VisualProfilePointContainment::Boundary => {
                Err(VisualProfileIssueKind::ContainmentAmbiguity {
                    support: fragment.source_span,
                })
            }
        };
    }
    Err(VisualProfileIssueKind::ContainmentAmbiguity {
        support: fragments[child.edges[0].fragment].source_span,
    })
}

#[derive(Clone, Copy)]
struct ContainmentEdge<'a> {
    support: CurveSpan,
    curve: &'a CurvePiece,
    source_parameter_enclosures: [Interval; 2],
}

#[derive(Clone, Copy, Debug)]
enum ContainmentRayAxis {
    PositiveX,
    PositiveY,
    PositiveDiagonal,
    PositiveSkewX,
    PositiveSkewY,
}

impl ContainmentRayAxis {
    fn along(self, value: Box2) -> Interval {
        match self {
            Self::PositiveX => value.x,
            Self::PositiveY => value.y,
            Self::PositiveDiagonal => value.x.add(value.y),
            Self::PositiveSkewX => value.x.mul(Interval::point(2.0)).add(value.y),
            Self::PositiveSkewY => value.x.add(value.y.mul(Interval::point(2.0))),
        }
    }

    fn normal(self, value: Box2) -> Interval {
        match self {
            Self::PositiveX => value.y,
            Self::PositiveY => value.x,
            Self::PositiveDiagonal => value.y.sub(value.x),
            Self::PositiveSkewX => value.y.mul(Interval::point(2.0)).sub(value.x),
            Self::PositiveSkewY => value.y.sub(value.x.mul(Interval::point(2.0))),
        }
    }

    fn normal_derivative(self, value: [Interval; 2]) -> Interval {
        match self {
            Self::PositiveX => value[1],
            Self::PositiveY => value[0],
            Self::PositiveDiagonal => value[1].sub(value[0]),
            Self::PositiveSkewX => value[1].mul(Interval::point(2.0)).sub(value[0]),
            Self::PositiveSkewY => value[1].sub(value[0].mul(Interval::point(2.0))),
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum RayPointContainment {
    Inside,
    Outside,
    Boundary,
    Degenerate(CurveSpan),
}

fn point_in_cycle(
    witness: Box2,
    cycle: &Cycle,
    fragments: &[Fragment],
    sources: &[SourcePiece],
    work: &mut Work,
) -> Result<VisualProfilePointContainment, VisualProfileIssueKind> {
    let edges = cycle
        .edges
        .iter()
        .map(|edge| {
            let fragment = &fragments[edge.fragment];
            let source = &sources[fragment.source];
            ContainmentEdge {
                support: source.span,
                curve: &source.curve,
                source_parameter_enclosures: fragment.source_parameter_enclosures,
            }
        })
        .collect::<Vec<_>>();
    point_in_containment_edges(witness, &edges, work)
}

fn point_in_certified_contour(
    witness: Box2,
    contour: &[CertifiedContainmentEdge],
    work: &mut Work,
) -> Result<VisualProfilePointContainment, VisualProfileIssueKind> {
    let edges = contour
        .iter()
        .map(|edge| ContainmentEdge {
            support: edge.support,
            curve: &edge.curve,
            source_parameter_enclosures: edge.source_parameter_enclosures,
        })
        .collect::<Vec<_>>();
    point_in_containment_edges(witness, &edges, work)
}

fn point_in_containment_edges(
    witness: Box2,
    edges: &[ContainmentEdge<'_>],
    work: &mut Work,
) -> Result<VisualProfilePointContainment, VisualProfileIssueKind> {
    const RAY_AXES: [ContainmentRayAxis; 5] = [
        ContainmentRayAxis::PositiveSkewX,
        ContainmentRayAxis::PositiveSkewY,
        ContainmentRayAxis::PositiveDiagonal,
        ContainmentRayAxis::PositiveX,
        ContainmentRayAxis::PositiveY,
    ];
    let mut ambiguous_support = None;
    for (axis_index, axis) in RAY_AXES.into_iter().enumerate() {
        let remaining = work
            .options
            .max_containment_tests
            .saturating_sub(work.containment_tests);
        if remaining == 0 {
            return Err(VisualProfileIssueKind::ContainmentBudgetExceeded {
                required: work.options.max_containment_tests.saturating_add(1),
                limit: work.options.max_containment_tests,
            });
        }
        let remaining_axes = RAY_AXES.len() - axis_index;
        let axis_limit = work
            .containment_tests
            .saturating_add(remaining.div_ceil(remaining_axes));
        let result = point_in_positive_ray(witness, edges, axis, axis_limit, work);
        match result {
            Ok(RayPointContainment::Inside) => {
                return Ok(VisualProfilePointContainment::Inside);
            }
            Ok(RayPointContainment::Outside) => {
                return Ok(VisualProfilePointContainment::Outside);
            }
            Ok(RayPointContainment::Boundary) => {
                return Ok(VisualProfilePointContainment::Boundary);
            }
            Ok(RayPointContainment::Degenerate(support))
            | Err(VisualProfileIssueKind::ContainmentAmbiguity { support }) => {
                ambiguous_support = Some(
                    ambiguous_support.map_or(support, |current: CurveSpan| current.min(support)),
                );
            }
            Err(kind) => return Err(kind),
        }
    }
    Err(VisualProfileIssueKind::ContainmentAmbiguity {
        support: ambiguous_support.unwrap_or(edges[0].support),
    })
}

fn point_in_positive_ray(
    witness: Box2,
    edges: &[ContainmentEdge<'_>],
    axis: ContainmentRayAxis,
    axis_containment_limit: usize,
    work: &mut Work,
) -> Result<RayPointContainment, VisualProfileIssueKind> {
    let mut crossings = 0_usize;
    let witness_along = axis.along(witness);
    let witness_normal = axis.normal(witness);
    for edge in edges {
        let complete_parameters = Interval::hull(
            edge.source_parameter_enclosures[0]
                .lower
                .min(edge.source_parameter_enclosures[1].lower),
            edge.source_parameter_enclosures[0]
                .upper
                .max(edge.source_parameter_enclosures[1].upper),
        );
        let complete_position = edge.curve.position(complete_parameters).map_err(|_| {
            VisualProfileIssueKind::ContainmentAmbiguity {
                support: edge.support,
            }
        })?;
        if !axis.normal(complete_position).overlaps(witness_normal)
            || axis.along(complete_position).upper < witness_along.lower
        {
            continue;
        }
        for endpoint in edge.source_parameter_enclosures {
            let position = edge.curve.position(endpoint).map_err(|_| {
                VisualProfileIssueKind::ContainmentAmbiguity {
                    support: edge.support,
                }
            })?;
            if position.x.overlaps(witness.x) && position.y.overlaps(witness.y) {
                return Ok(RayPointContainment::Boundary);
            }
            if axis.normal(position).overlaps(witness_normal)
                && axis.along(position).upper >= witness_along.lower
            {
                return Ok(RayPointContainment::Degenerate(edge.support));
            }
        }
        let Some(parameters) = Interval::checked(
            next_up(edge.source_parameter_enclosures[0].upper),
            next_down(edge.source_parameter_enclosures[1].lower),
        ) else {
            let unresolved = Interval::hull(
                edge.source_parameter_enclosures[0].upper,
                edge.source_parameter_enclosures[1].lower,
            );
            let position = edge.curve.position(unresolved).map_err(|_| {
                VisualProfileIssueKind::ContainmentAmbiguity {
                    support: edge.support,
                }
            })?;
            if position.x.overlaps(witness.x) && position.y.overlaps(witness.y) {
                return Ok(RayPointContainment::Boundary);
            }
            if !axis.normal(position).overlaps(witness_normal)
                || axis.along(position).upper < witness_along.lower
            {
                continue;
            }
            return Ok(RayPointContainment::Degenerate(edge.support));
        };
        let roots = isolate_ray_roots(
            edge,
            parameters,
            witness_normal,
            axis,
            axis_containment_limit,
            work,
        )?;
        for root in roots {
            let position = edge.curve.position(root).map_err(|_| {
                VisualProfileIssueKind::ContainmentAmbiguity {
                    support: edge.support,
                }
            })?;
            if position.x.overlaps(witness.x) && position.y.overlaps(witness.y) {
                return Ok(RayPointContainment::Boundary);
            }
            let derivative = edge.curve.derivative(root).map_err(|_| {
                VisualProfileIssueKind::ContainmentAmbiguity {
                    support: edge.support,
                }
            })?;
            if axis.normal_derivative(derivative).contains_zero() {
                return Ok(RayPointContainment::Degenerate(edge.support));
            }
            let position_along = axis.along(position);
            if position_along.upper < witness_along.lower {
                continue;
            }
            if position_along.lower <= witness_along.upper {
                return Ok(RayPointContainment::Boundary);
            }
            crossings += 1;
        }
    }
    Ok(if crossings.is_multiple_of(2) {
        RayPointContainment::Outside
    } else {
        RayPointContainment::Inside
    })
}

#[allow(
    clippy::too_many_lines,
    reason = "the bounded interval root isolator keeps subdivision, budget and distinct-root certification in one auditable routine"
)]
fn isolate_ray_roots(
    source: &ContainmentEdge<'_>,
    parameter: Interval,
    ray_normal: Interval,
    axis: ContainmentRayAxis,
    axis_containment_limit: usize,
    work: &mut Work,
) -> Result<Vec<Interval>, VisualProfileIssueKind> {
    let mut stack = vec![(parameter, 0_usize)];
    let mut roots = Vec::new();
    while let Some((interval, depth)) = stack.pop() {
        if work.containment_tests >= axis_containment_limit {
            return Err(VisualProfileIssueKind::ContainmentAmbiguity {
                support: source.support,
            });
        }
        charge_containment(work)?;
        let position = source.curve.position(interval).map_err(|_| {
            VisualProfileIssueKind::ContainmentAmbiguity {
                support: source.support,
            }
        })?;
        if !axis.normal(position).overlaps(ray_normal) {
            continue;
        }
        let derivative = source.curve.derivative(interval).map_err(|_| {
            VisualProfileIssueKind::ContainmentAmbiguity {
                support: source.support,
            }
        })?;
        let normal_derivative = axis.normal_derivative(derivative);
        if normal_derivative.excludes_zero() {
            let middle = interval.midpoint();
            let value = source
                .curve
                .position(Interval::point(middle))
                .map_err(|_| VisualProfileIssueKind::ContainmentAmbiguity {
                    support: source.support,
                })?;
            let value = axis.normal(value).sub(ray_normal);
            let newton = Interval::point(middle).sub(value.div(normal_derivative).ok_or(
                VisualProfileIssueKind::ContainmentAmbiguity {
                    support: source.support,
                },
            )?);
            let Some(root) = interval.intersection(newton) else {
                continue;
            };
            if interval.interior_contains(newton) {
                roots.push(root);
                continue;
            }
        }
        let middle = interval.midpoint();
        if let Some(root) = bracket_ray_midpoint_root(source, interval, middle, ray_normal, axis)? {
            roots.push(root);
            if interval.lower < root.lower {
                stack.push((Interval::hull(interval.lower, root.lower), depth + 1));
            }
            if root.upper < interval.upper {
                stack.push((Interval::hull(root.upper, interval.upper), depth + 1));
            }
            continue;
        }
        if depth >= work.options.max_intersection_depth {
            return Err(VisualProfileIssueKind::ContainmentAmbiguity {
                support: source.support,
            });
        }
        if middle.to_bits() == interval.lower.to_bits()
            || middle.to_bits() == interval.upper.to_bits()
        {
            return Err(VisualProfileIssueKind::ContainmentAmbiguity {
                support: source.support,
            });
        }
        if work.intersection_subdivisions >= work.options.max_intersection_subdivisions {
            return Err(
                VisualProfileIssueKind::IntersectionSubdivisionBudgetExceeded {
                    first: source.support,
                    second: source.support,
                    limit: work.options.max_intersection_subdivisions,
                },
            );
        }
        if !work.charge_operation(
            OperationWorkCounter::ProfileSubdivisions,
            1,
            OperationCheckpoint::ProfileSubdivision,
        ) {
            return Err(
                VisualProfileIssueKind::IntersectionSubdivisionBudgetExceeded {
                    first: source.support,
                    second: source.support,
                    limit: work.options.max_intersection_subdivisions,
                },
            );
        }
        work.intersection_subdivisions += 1;
        stack.push((Interval::hull(middle, interval.upper), depth + 1));
        stack.push((Interval::hull(interval.lower, middle), depth + 1));
    }
    roots.sort_by(|first, second| first.lower.total_cmp(&second.lower));
    let mut distinct_roots = Vec::<Interval>::with_capacity(roots.len());
    for root in roots {
        if let Some(previous) = distinct_roots.last_mut()
            && previous.overlaps(root)
        {
            let joined = previous.include(root);
            let derivative = source.curve.derivative(joined).map_err(|_| {
                VisualProfileIssueKind::ContainmentAmbiguity {
                    support: source.support,
                }
            })?;
            if axis.normal_derivative(derivative).contains_zero() {
                return Err(VisualProfileIssueKind::ContainmentAmbiguity {
                    support: source.support,
                });
            }
            *previous = joined;
        } else {
            distinct_roots.push(root);
        }
    }
    Ok(distinct_roots)
}

fn bracket_ray_midpoint_root(
    source: &ContainmentEdge<'_>,
    domain: Interval,
    middle: f64,
    ray_normal: Interval,
    axis: ContainmentRayAxis,
) -> Result<Option<Interval>, VisualProfileIssueKind> {
    let width = domain.width() * 1.0e-8;
    if width == 0.0 {
        return Ok(None);
    }
    let interval = Interval::hull(
        (middle - width).max(domain.lower),
        (middle + width).min(domain.upper),
    );
    if interval.lower.to_bits() == interval.upper.to_bits() {
        return Ok(None);
    }
    let derivative = source.curve.derivative(interval).map_err(|_| {
        VisualProfileIssueKind::ContainmentAmbiguity {
            support: source.support,
        }
    })?;
    if !axis.normal_derivative(derivative).excludes_zero() {
        return Ok(None);
    }
    let signed_value = |parameter| -> Result<Interval, VisualProfileIssueKind> {
        source
            .curve
            .position(Interval::point(parameter))
            .map(|position| axis.normal(position).sub(ray_normal))
            .map_err(|_| VisualProfileIssueKind::ContainmentAmbiguity {
                support: source.support,
            })
    };
    let lower = signed_value(interval.lower)?;
    let upper = signed_value(interval.upper)?;
    let bracketed =
        lower.upper < 0.0 && upper.lower > 0.0 || upper.upper < 0.0 && lower.lower > 0.0;
    Ok(bracketed.then_some(interval))
}

fn charge_containment(work: &mut Work) -> Result<(), VisualProfileIssueKind> {
    if work.containment_tests >= work.options.max_containment_tests {
        return Err(VisualProfileIssueKind::ContainmentBudgetExceeded {
            required: work.options.max_containment_tests.saturating_add(1),
            limit: work.options.max_containment_tests,
        });
    }
    if !work.charge_operation(
        OperationWorkCounter::ProfileContainmentTests,
        1,
        OperationCheckpoint::ProfileContainment,
    ) {
        return Err(VisualProfileIssueKind::ContainmentBudgetExceeded {
            required: work.containment_tests.saturating_add(1),
            limit: work.options.max_containment_tests,
        });
    }
    work.containment_tests += 1;
    Ok(())
}

fn cycle_contour(cycle: &Cycle, fragments: &[Fragment], reverse: bool) -> VisualProfileContour {
    let directed = if reverse {
        cycle
            .edges
            .iter()
            .rev()
            .map(|edge| DirectedFragment {
                fragment: edge.fragment,
                forward: !edge.forward,
            })
            .collect::<Vec<_>>()
    } else {
        cycle.edges.clone()
    };
    let edges = directed
        .into_iter()
        .map(|directed| {
            let fragment = &fragments[directed.fragment];
            if directed.forward {
                VisualProfileEdge {
                    start: fragment.start_position,
                    end: fragment.end_position,
                    source_span: fragment.source_span,
                    source_parameters: fragment.source_parameters,
                    source_parameter_enclosures: fragment
                        .source_parameter_enclosures
                        .map(|value| [value.lower, value.upper]),
                }
            } else {
                VisualProfileEdge {
                    start: fragment.end_position,
                    end: fragment.start_position,
                    source_span: fragment.source_span,
                    source_parameters: [
                        fragment.source_parameters[1],
                        fragment.source_parameters[0],
                    ],
                    source_parameter_enclosures: [
                        [
                            fragment.source_parameter_enclosures[1].lower,
                            fragment.source_parameter_enclosures[1].upper,
                        ],
                        [
                            fragment.source_parameter_enclosures[0].lower,
                            fragment.source_parameter_enclosures[0].upper,
                        ],
                    ],
                }
            }
        })
        .collect();
    let area = if reverse {
        cycle.area.neg()
    } else {
        cycle.area
    };
    VisualProfileContour {
        orientation: if reverse {
            VisualProfileOrientation::Clockwise
        } else {
            VisualProfileOrientation::CounterClockwise
        },
        signed_area: if reverse {
            -cycle.representative_area
        } else {
            cycle.representative_area
        },
        area_uncertainty: interval_uncertainty(area),
        edges,
    }
}

fn cycle_containment_edges(
    cycle: &Cycle,
    fragments: &[Fragment],
    sources: &[SourcePiece],
) -> Vec<CertifiedContainmentEdge> {
    cycle
        .edges
        .iter()
        .map(|edge| {
            let fragment = &fragments[edge.fragment];
            let source = &sources[fragment.source];
            CertifiedContainmentEdge {
                support: source.span,
                curve: source.curve.clone(),
                source_parameter_enclosures: fragment.source_parameter_enclosures,
            }
        })
        .collect()
}

fn skipped_analysis(
    work: &Work,
    kind: VisualProfileIssueKind,
    affected_spans: Vec<CurveSpan>,
) -> VisualProfileAnalysis {
    skipped_analysis_with_families(work, kind, affected_spans, Vec::new())
}

fn interrupted_profile_analysis(work: &Work) -> VisualProfileAnalysis {
    VisualProfileAnalysis {
        scope: VisualProfileGeometryScope::AllBuiltInPlanarCurves,
        status: VisualProfileStatus::Skipped,
        families: Vec::new(),
        faces: Vec::new(),
        intersections: Vec::new(),
        issues: Vec::new(),
        budgets: work.report(),
        candidate_pairs: work.candidate_pairs,
        fragment_count: work.fragments,
    }
}

fn skipped_analysis_with_families(
    work: &Work,
    kind: VisualProfileIssueKind,
    affected_spans: Vec<CurveSpan>,
    families: Vec<VisualProfileCurveFamily>,
) -> VisualProfileAnalysis {
    VisualProfileAnalysis {
        scope: VisualProfileGeometryScope::AllBuiltInPlanarCurves,
        status: VisualProfileStatus::Skipped,
        families,
        faces: Vec::new(),
        intersections: Vec::new(),
        issues: vec![VisualProfileIssue {
            kind,
            affected_spans,
        }],
        budgets: work.report(),
        candidate_pairs: work.candidate_pairs,
        fragment_count: work.fragments,
    }
}

fn component_issue(
    kind: VisualProfileIssueKind,
    component: usize,
    component_spans: &BTreeMap<usize, Vec<CurveSpan>>,
) -> VisualProfileIssue {
    VisualProfileIssue {
        kind,
        affected_spans: component_spans.get(&component).cloned().unwrap_or_default(),
    }
}

fn compare_faces(first: &VisualProfileFace, second: &VisualProfileFace) -> Ordering {
    let first_edge = &first.contours[0].edges[0];
    let second_edge = &second.contours[0].edges[0];
    first_edge
        .source_span
        .cmp(&second_edge.source_span)
        .then_with(|| first_edge.source_parameters[0].total_cmp(&second_edge.source_parameters[0]))
        .then_with(|| first.visual_area.total_cmp(&second.visual_area))
}

fn compare_issue_kinds(
    first: &VisualProfileIssueKind,
    second: &VisualProfileIssueKind,
) -> Ordering {
    issue_key(first).cmp(&issue_key(second))
}

fn issue_key(issue: &VisualProfileIssueKind) -> (u8, Option<CurveSpan>, Option<CurveSpan>) {
    match issue {
        VisualProfileIssueKind::CandidateBudgetExceeded { .. } => (0, None, None),
        VisualProfileIssueKind::IntersectionSubdivisionBudgetExceeded { first, second, .. } => {
            (1, Some(*first), Some(*second))
        }
        VisualProfileIssueKind::IntersectionRootBudgetExceeded { .. } => (2, None, None),
        VisualProfileIssueKind::FragmentBudgetExceeded { .. } => (3, None, None),
        VisualProfileIssueKind::IntegrationBudgetExceeded { support, .. } => {
            (4, Some(*support), None)
        }
        VisualProfileIssueKind::ContainmentBudgetExceeded { .. } => (5, None, None),
        VisualProfileIssueKind::FaceBudgetExceeded { .. } => (6, None, None),
        VisualProfileIssueKind::InconsistentCoincidence { .. } => (7, None, None),
        VisualProfileIssueKind::ExplicitJoinMismatch { first, second } => {
            (8, Some(*first), Some(*second))
        }
        VisualProfileIssueKind::CollinearOverlap { first, second } => {
            (9, Some(*first), Some(*second))
        }
        VisualProfileIssueKind::CurveOverlap { first, second } => (10, Some(*first), Some(*second)),
        VisualProfileIssueKind::TangentIntersection { first, second } => {
            (11, Some(*first), Some(*second))
        }
        VisualProfileIssueKind::RationalPole { support } => (12, Some(*support), None),
        VisualProfileIssueKind::ZeroSpeed { support } => (13, Some(*support), None),
        VisualProfileIssueKind::UnresolvedIntersection { first, second } => {
            (14, Some(*first), Some(*second))
        }
        VisualProfileIssueKind::UnresolvedTangentOrder { support } => (15, Some(*support), None),
        VisualProfileIssueKind::AreaUncertainty { support } => (16, Some(*support), None),
        VisualProfileIssueKind::ContainmentAmbiguity { support } => (17, Some(*support), None),
        VisualProfileIssueKind::NumericalAmbiguity { first, second } => {
            (18, Some(*first), Some(*second))
        }
        VisualProfileIssueKind::VisibleIntervalUnavailable { support } => {
            (19, Some(*support), None)
        }
    }
}

fn candidate_pair_count(segment_count: usize) -> Option<usize> {
    if segment_count < 2 {
        return Some(0);
    }
    let predecessor = segment_count - 1;
    if segment_count.is_multiple_of(2) {
        (segment_count / 2).checked_mul(predecessor)
    } else {
        segment_count.checked_mul(predecessor / 2)
    }
}

#[allow(clippy::float_cmp)]
fn parameter_equal(first: f64, second: f64) -> bool {
    first == second
}

fn subtract(first: [f64; 2], second: [f64; 2]) -> [f64; 2] {
    [first[0] - second[0], first[1] - second[1]]
}

fn add(first: [f64; 2], second: [f64; 2]) -> [f64; 2] {
    [first[0] + second[0], first[1] + second[1]]
}

fn scale(value: [f64; 2], factor: f64) -> [f64; 2] {
    [value[0] * factor, value[1] * factor]
}

fn cross(first: [f64; 2], second: [f64; 2]) -> f64 {
    first[0] * second[1] - first[1] * second[0]
}

fn norm(value: [f64; 2]) -> f64 {
    value[0].hypot(value[1])
}

fn polygon_area(points: &[[f64; 2]]) -> f64 {
    let origin = points.first().copied().unwrap_or([0.0, 0.0]);
    0.5 * points
        .iter()
        .zip(points.iter().cycle().skip(1))
        .take(points.len())
        .map(|(first, second)| cross(subtract(*first, origin), subtract(*second, origin)))
        .sum::<f64>()
}

fn interval_uncertainty(value: Interval) -> f64 {
    Interval::point(value.upper)
        .sub(Interval::point(value.lower))
        .scale(0.5)
        .upper
}

#[cfg(test)]
mod tests {
    use super::{
        Box2, DisjointSet, Interval, VisualProfileOptions, Work, bounds_clearly_disjoint,
        candidate_pair_count, is_explicit_full_period, next_down, overlapping_source_pairs,
    };
    use crate::{
        CurveDefinition, CurveSpan, DocumentTrimBoundary, DocumentTrimParameter, ScalarDomain,
        ScalarUnit, SketchDocument,
    };

    #[test]
    fn descending_union_chain_uses_iterative_path_compression() {
        let mut sets = DisjointSet::new(50_000);
        for value in (1..50_000).rev() {
            sets.union(value, value - 1);
        }
        assert_eq!(sets.root(49_999), 0);
        assert_eq!(sets.parent[49_999], 0);
    }

    #[test]
    fn candidate_pair_count_divides_before_multiplying() {
        assert_eq!(candidate_pair_count(0), Some(0));
        assert_eq!(candidate_pair_count(70_000), Some(2_449_965_000));
        assert_eq!(candidate_pair_count(usize::MAX), None);
    }

    #[test]
    fn source_pair_sweep_is_conservative_and_restores_lexicographic_order() {
        let box_2d = |x: [f64; 2], y: [f64; 2]| Box2 {
            x: Interval::hull(x[0], x[1]),
            y: Interval::hull(y[0], y[1]),
        };
        let bounds = vec![
            box_2d([10.0, 11.0], [0.0, 1.0]),
            box_2d([0.0, 2.0], [0.0, 2.0]),
            box_2d([1.0, 3.0], [1.0, 3.0]),
            box_2d([10.5, 12.0], [0.5, 1.5]),
            box_2d([5.0, 6.0], [5.0, 6.0]),
            box_2d([20.0, 21.0], [0.0, 1.0]),
            box_2d([21.0 + 128.0 * f64::EPSILON, 22.0], [0.0, 1.0]),
        ];
        let expected = (0..bounds.len())
            .flat_map(|first| ((first + 1)..bounds.len()).map(move |second| (first, second)))
            .filter(|(first, second)| !bounds_clearly_disjoint(bounds[*first], bounds[*second]))
            .collect::<Vec<_>>();

        let actual =
            overlapping_source_pairs(&bounds, &Work::new(VisualProfileOptions::default())).unwrap();

        assert_eq!(actual, expected);
        assert_eq!(actual, vec![(0, 3), (1, 2), (5, 6)]);
    }

    #[test]
    fn periodic_closure_uses_explicit_boundary_winding() {
        let mut document = SketchDocument::new(1.0).unwrap();
        let center = document.add_point("center", [0.0, 0.0]).unwrap();
        let radius = document
            .add_scalar("radius", 1.0, ScalarUnit::Length, ScalarDomain::Positive)
            .unwrap();
        let curve = document
            .add_curve("circle", CurveDefinition::Circle { center, radius })
            .unwrap();
        let definition = &document.curve(curve).unwrap().definition;
        let mut interval = document.visible_interval(CurveSpan::line(curve)).unwrap();
        assert!(is_explicit_full_period(definition, interval));

        interval.end = next_down(std::f64::consts::TAU);
        interval.end_boundary = DocumentTrimBoundary::Fixed(DocumentTrimParameter {
            parameter: interval.end,
            winding: 0,
        });
        assert!(!is_explicit_full_period(definition, interval));

        interval.start_boundary = DocumentTrimBoundary::Fixed(DocumentTrimParameter {
            parameter: 1.0,
            winding: 100,
        });
        interval.end_boundary = DocumentTrimBoundary::Fixed(DocumentTrimParameter {
            parameter: 1.0,
            winding: 101,
        });
        assert!(is_explicit_full_period(definition, interval));
    }
}
