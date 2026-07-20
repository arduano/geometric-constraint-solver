// SPDX-License-Identifier: GPL-3.0-or-later

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

use crate::{
    CurveDefinition, CurveSpan, DesignPointId, DocumentConstraintDefinition, SketchDocument,
};

/// Deterministic resource limits for read-only visual line-profile analysis.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VisualProfileOptions {
    pub max_candidate_pairs: usize,
    pub max_fragments: usize,
    pub max_containment_tests: usize,
    pub max_faces: usize,
}

impl Default for VisualProfileOptions {
    fn default() -> Self {
        Self {
            max_candidate_pairs: 100_000,
            max_fragments: 100_000,
            max_containment_tests: 100_000,
            max_faces: 10_000,
        }
    }
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
    FragmentBudgetExceeded {
        required: usize,
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
    CollinearOverlap {
        first: CurveSpan,
        second: CurveSpan,
    },
    NumericalAmbiguity {
        first: CurveSpan,
        second: CurveSpan,
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

/// One directed contour edge with exact source-span provenance.
#[derive(Clone, Debug, PartialEq)]
pub struct VisualProfileEdge {
    pub start: [f64; 2],
    pub end: [f64; 2],
    pub source_span: CurveSpan,
    pub source_parameters: [f64; 2],
}

/// One ordered outer or hole contour.
#[derive(Clone, Debug, PartialEq)]
pub struct VisualProfileContour {
    pub orientation: VisualProfileOrientation,
    pub signed_area: f64,
    pub edges: Vec<VisualProfileEdge>,
}

/// One visual bounded face. The first contour is counterclockwise; later contours are holes.
#[derive(Clone, Debug, PartialEq)]
pub struct VisualProfileFace {
    pub contours: Vec<VisualProfileContour>,
    pub visual_area: f64,
}

/// Read-only visual profile result. It never owns persistent IDs or solver equations.
#[derive(Clone, Debug, PartialEq)]
pub struct VisualProfileAnalysis {
    pub status: VisualProfileStatus,
    pub faces: Vec<VisualProfileFace>,
    pub issues: Vec<VisualProfileIssue>,
    pub candidate_pairs: usize,
    pub fragment_count: usize,
}

#[derive(Clone, Debug)]
struct SourceSegment {
    span: CurveSpan,
    start: DesignPointId,
    end: DesignPointId,
    start_position: [f64; 2],
    end_position: [f64; 2],
}

struct WeldedPoints {
    roots: BTreeMap<DesignPointId, DesignPointId>,
    positions: BTreeMap<DesignPointId, [f64; 2]>,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum VertexKey {
    Persistent(DesignPointId),
    Crossing(usize),
}

#[derive(Clone, Copy, Debug)]
struct Split {
    parameter: f64,
    vertex: VertexKey,
}

#[derive(Clone, Copy, Debug)]
struct PairHit {
    first_parameter: f64,
    second_parameter: f64,
    point: [f64; 2],
    vertex: VertexKey,
    first_interior: bool,
    second_interior: bool,
}

#[derive(Clone, Copy, Debug)]
enum PairResult {
    None,
    Hit(PairHit),
    CollinearOverlap,
    Ambiguous,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ParameterLocation {
    Start,
    End,
    Interior,
    Outside,
    Ambiguous,
}

#[derive(Clone, Debug)]
struct Fragment {
    start: VertexKey,
    end: VertexKey,
    start_position: [f64; 2],
    end_position: [f64; 2],
    source_span: CurveSpan,
    source_parameters: [f64; 2],
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
    area: f64,
    vertices: Vec<[f64; 2]>,
    edges: Vec<DirectedFragment>,
}

#[derive(Clone, Debug)]
struct PairIssue {
    kind: VisualProfileIssueKind,
    first_segment: usize,
    second_segment: usize,
}

#[derive(Clone, Copy, Debug)]
struct Bounds {
    min: [f64; 2],
    max: [f64; 2],
}

impl Bounds {
    fn from_points(points: &[[f64; 2]]) -> Self {
        let mut bounds = Self {
            min: points[0],
            max: points[0],
        };
        for point in &points[1..] {
            bounds.include(*point);
        }
        bounds
    }

    fn include(&mut self, point: [f64; 2]) {
        self.min[0] = self.min[0].min(point[0]);
        self.min[1] = self.min[1].min(point[1]);
        self.max[0] = self.max[0].max(point[0]);
        self.max[1] = self.max[1].max(point[1]);
    }

    fn include_bounds(&mut self, other: Self) {
        self.include(other.min);
        self.include(other.max);
    }

    fn contains(self, other: Self) -> bool {
        self.min[0] <= other.min[0]
            && self.min[1] <= other.min[1]
            && self.max[0] >= other.max[0]
            && self.max[1] >= other.max[1]
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
    /// Extracts deterministic visual-only bounded faces from consistent line/polyline geometry.
    ///
    /// Shared point identities and active coincidence constraints weld topology. Proper crossings
    /// and T-junctions split ephemeral fragments. No proximity snapping, persistent region state,
    /// history entry, selection identity, or solver equation is created. A profile-relevant
    /// coincidence class outside the default normalized hard-residual tolerance is skipped rather
    /// than moving unsolved geometry into an apparently complete face.
    #[must_use]
    pub fn analyze_visual_profiles(&self, options: VisualProfileOptions) -> VisualProfileAnalysis {
        analyze_visual_profiles(self, options)
    }
}

#[allow(clippy::too_many_lines)]
fn analyze_visual_profiles(
    document: &SketchDocument,
    options: VisualProfileOptions,
) -> VisualProfileAnalysis {
    let WeldedPoints {
        roots: point_roots,
        positions: root_positions,
    } = match welded_points(document) {
        Ok(points) => points,
        Err((first, second)) => {
            return budget_analysis(
                VisualProfileIssueKind::InconsistentCoincidence { first, second },
                0,
                0,
            );
        }
    };
    let segments = source_segments(document, &point_roots, &root_positions);
    let Some(candidate_pairs) = candidate_pair_count(segments.len()) else {
        return budget_analysis(
            VisualProfileIssueKind::CandidateBudgetExceeded {
                required: usize::MAX,
                limit: options.max_candidate_pairs,
            },
            usize::MAX,
            0,
        );
    };
    if candidate_pairs > options.max_candidate_pairs {
        return budget_analysis(
            VisualProfileIssueKind::CandidateBudgetExceeded {
                required: candidate_pairs,
                limit: options.max_candidate_pairs,
            },
            candidate_pairs,
            0,
        );
    }
    if segments.len() > options.max_fragments {
        return budget_analysis(
            VisualProfileIssueKind::FragmentBudgetExceeded {
                required: segments.len(),
                limit: options.max_fragments,
            },
            candidate_pairs,
            segments.len(),
        );
    }

    let mut segment_components = DisjointSet::new(segments.len());
    let mut endpoint_owners: BTreeMap<DesignPointId, usize> = BTreeMap::new();
    for (index, segment) in segments.iter().enumerate() {
        for endpoint in [segment.start, segment.end] {
            if let Some(owner) = endpoint_owners.insert(endpoint, index) {
                segment_components.union(owner, index);
            }
        }
    }

    let mut splits = segments
        .iter()
        .map(|segment| {
            vec![
                Split {
                    parameter: 0.0,
                    vertex: VertexKey::Persistent(segment.start),
                },
                Split {
                    parameter: 1.0,
                    vertex: VertexKey::Persistent(segment.end),
                },
            ]
        })
        .collect::<Vec<_>>();
    let mut vertex_positions = root_positions
        .iter()
        .map(|(point, position)| (VertexKey::Persistent(*point), *position))
        .collect::<BTreeMap<_, _>>();
    let mut pair_issues = Vec::new();
    let mut pair_ordinal = 0;
    let mut predicted_fragments = segments.len();
    let mut budget_splits = segments
        .iter()
        .map(|segment| {
            BTreeSet::from([
                (0.0_f64.to_bits(), VertexKey::Persistent(segment.start)),
                (1.0_f64.to_bits(), VertexKey::Persistent(segment.end)),
            ])
        })
        .collect::<Vec<_>>();
    for first in 0..segments.len() {
        for second in first + 1..segments.len() {
            let result = classify_pair(
                &segments[first],
                &segments[second],
                pair_ordinal,
                document.model_scale(),
            );
            match result {
                PairResult::None => {}
                PairResult::Hit(hit) => {
                    if hit.first_interior
                        && budget_splits[first].insert((hit.first_parameter.to_bits(), hit.vertex))
                    {
                        let Some(required) = predicted_fragments.checked_add(1) else {
                            return budget_analysis(
                                VisualProfileIssueKind::FragmentBudgetExceeded {
                                    required: usize::MAX,
                                    limit: options.max_fragments,
                                },
                                candidate_pairs,
                                usize::MAX,
                            );
                        };
                        predicted_fragments = required;
                    }
                    if hit.second_interior
                        && budget_splits[second]
                            .insert((hit.second_parameter.to_bits(), hit.vertex))
                    {
                        let Some(required) = predicted_fragments.checked_add(1) else {
                            return budget_analysis(
                                VisualProfileIssueKind::FragmentBudgetExceeded {
                                    required: usize::MAX,
                                    limit: options.max_fragments,
                                },
                                candidate_pairs,
                                usize::MAX,
                            );
                        };
                        predicted_fragments = required;
                    }
                    if predicted_fragments > options.max_fragments {
                        return budget_analysis(
                            VisualProfileIssueKind::FragmentBudgetExceeded {
                                required: predicted_fragments,
                                limit: options.max_fragments,
                            },
                            candidate_pairs,
                            predicted_fragments,
                        );
                    }
                    segment_components.union(first, second);
                    vertex_positions.entry(hit.vertex).or_insert(hit.point);
                    if hit.first_interior {
                        splits[first].push(Split {
                            parameter: hit.first_parameter,
                            vertex: hit.vertex,
                        });
                    }
                    if hit.second_interior {
                        splits[second].push(Split {
                            parameter: hit.second_parameter,
                            vertex: hit.vertex,
                        });
                    }
                }
                PairResult::CollinearOverlap => {
                    segment_components.union(first, second);
                    pair_issues.push(PairIssue {
                        kind: VisualProfileIssueKind::CollinearOverlap {
                            first: segments[first].span,
                            second: segments[second].span,
                        },
                        first_segment: first,
                        second_segment: second,
                    });
                }
                PairResult::Ambiguous => {
                    segment_components.union(first, second);
                    pair_issues.push(PairIssue {
                        kind: VisualProfileIssueKind::NumericalAmbiguity {
                            first: segments[first].span,
                            second: segments[second].span,
                        },
                        first_segment: first,
                        second_segment: second,
                    });
                }
            }
            pair_ordinal = pair_ordinal
                .checked_add(1)
                .expect("candidate pair count was validated");
        }
    }

    let component_roots = (0..segments.len())
        .map(|index| segment_components.root(index))
        .collect::<Vec<_>>();
    let mut component_spans: BTreeMap<usize, Vec<CurveSpan>> = BTreeMap::new();
    for (index, segment) in segments.iter().enumerate() {
        component_spans
            .entry(component_roots[index])
            .or_default()
            .push(segment.span);
    }
    for spans in component_spans.values_mut() {
        spans.sort_unstable();
        spans.dedup();
    }
    let mut bad_components = BTreeSet::new();
    let mut issues = pair_issues
        .into_iter()
        .map(|issue| {
            let root = component_roots[issue.first_segment];
            debug_assert_eq!(root, component_roots[issue.second_segment]);
            bad_components.insert(root);
            VisualProfileIssue {
                kind: issue.kind,
                affected_spans: component_spans.get(&root).cloned().unwrap_or_default(),
            }
        })
        .collect::<Vec<_>>();

    let parameter_tolerance = 128.0 * f64::EPSILON;
    let mut fragments = Vec::new();
    for (index, segment) in segments.iter().enumerate() {
        let component = component_roots[index];
        if bad_components.contains(&component) {
            continue;
        }
        splits[index].sort_by(|first, second| {
            first
                .parameter
                .total_cmp(&second.parameter)
                .then_with(|| first.vertex.cmp(&second.vertex))
        });
        let mut normalized = Vec::<Split>::new();
        let mut ambiguous = false;
        for split in &splits[index] {
            if let Some(previous) = normalized.last() {
                if split.parameter.to_bits() == previous.parameter.to_bits()
                    && split.vertex == previous.vertex
                {
                    continue;
                }
                if (split.parameter - previous.parameter).abs() <= parameter_tolerance {
                    ambiguous = true;
                    break;
                }
            }
            normalized.push(*split);
        }
        if ambiguous {
            bad_components.insert(component);
            issues.push(VisualProfileIssue {
                kind: VisualProfileIssueKind::NumericalAmbiguity {
                    first: segment.span,
                    second: segment.span,
                },
                affected_spans: component_spans.get(&component).cloned().unwrap_or_default(),
            });
            continue;
        }
        for pair in normalized.windows(2) {
            let [first, second] = pair else {
                unreachable!()
            };
            if first.vertex == second.vertex {
                bad_components.insert(component);
                issues.push(VisualProfileIssue {
                    kind: VisualProfileIssueKind::NumericalAmbiguity {
                        first: segment.span,
                        second: segment.span,
                    },
                    affected_spans: component_spans.get(&component).cloned().unwrap_or_default(),
                });
                break;
            }
            fragments.push(Fragment {
                start: first.vertex,
                end: second.vertex,
                start_position: vertex_positions[&first.vertex],
                end_position: vertex_positions[&second.vertex],
                source_span: segment.span,
                source_parameters: [first.parameter, second.parameter],
                component,
            });
        }
    }
    fragments.retain(|fragment| !bad_components.contains(&fragment.component));
    if fragments.len() > options.max_fragments {
        return budget_analysis(
            VisualProfileIssueKind::FragmentBudgetExceeded {
                required: fragments.len(),
                limit: options.max_fragments,
            },
            candidate_pairs,
            fragments.len(),
        );
    }

    fragments.sort_by(|first, second| {
        first
            .source_span
            .cmp(&second.source_span)
            .then_with(|| first.source_parameters[0].total_cmp(&second.source_parameters[0]))
    });
    let (mut cycles, ambiguous_cycles) = extract_cycles(&fragments, document.model_scale());
    for component in ambiguous_cycles {
        bad_components.insert(component);
        let spans = component_spans.get(&component).cloned().unwrap_or_default();
        let span = spans.first().copied().unwrap_or(segments[0].span);
        issues.push(VisualProfileIssue {
            kind: VisualProfileIssueKind::NumericalAmbiguity {
                first: span,
                second: span,
            },
            affected_spans: spans,
        });
    }
    cycles.retain(|cycle| !bad_components.contains(&cycle.component));
    let mut faces = match build_faces(&cycles, &fragments, options.max_containment_tests) {
        Ok(faces) => faces,
        Err(required) => {
            return budget_analysis(
                VisualProfileIssueKind::ContainmentBudgetExceeded {
                    required,
                    limit: options.max_containment_tests,
                },
                candidate_pairs,
                fragments.len(),
            );
        }
    };
    faces.sort_by(compare_faces);
    let required_faces = faces.len();
    if faces.len() > options.max_faces {
        faces.truncate(options.max_faces);
        issues.push(VisualProfileIssue {
            kind: VisualProfileIssueKind::FaceBudgetExceeded {
                required: required_faces,
                limit: options.max_faces,
            },
            affected_spans: segments.iter().map(|segment| segment.span).collect(),
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
        status,
        faces,
        issues,
        candidate_pairs,
        fragment_count: fragments.len(),
    }
}

fn budget_analysis(
    kind: VisualProfileIssueKind,
    candidate_pairs: usize,
    fragment_count: usize,
) -> VisualProfileAnalysis {
    VisualProfileAnalysis {
        status: VisualProfileStatus::Skipped,
        faces: Vec::new(),
        issues: vec![VisualProfileIssue {
            kind,
            affected_spans: Vec::new(),
        }],
        candidate_pairs,
        fragment_count,
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

fn welded_points(
    document: &SketchDocument,
) -> Result<WeldedPoints, (DesignPointId, DesignPointId)> {
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
        .filter(|value| !value.suppressed)
    {
        if let DocumentConstraintDefinition::Coincident { first, second } = constraint.definition
            && let (Some(first), Some(second)) = (indices.get(&first), indices.get(&second))
        {
            sets.union(*first, *second);
        }
    }
    let profile_points = profile_point_ids(document);
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
            return Err((root, point));
        }
    }
    Ok(WeldedPoints { roots, positions })
}

fn profile_point_ids(document: &SketchDocument) -> BTreeSet<DesignPointId> {
    document
        .curves()
        .iter()
        .flat_map(|curve| match &curve.definition {
            CurveDefinition::Line { start, end, .. } => vec![*start, *end],
            CurveDefinition::Polyline { points, .. } => points.clone(),
            _ => Vec::new(),
        })
        .collect()
}

fn source_segments(
    document: &SketchDocument,
    point_roots: &BTreeMap<DesignPointId, DesignPointId>,
    root_positions: &BTreeMap<DesignPointId, [f64; 2]>,
) -> Vec<SourceSegment> {
    let mut segments = Vec::new();
    for curve in document.curves() {
        let pairs = match &curve.definition {
            CurveDefinition::Line { start, end, .. } => vec![(*start, *end)],
            CurveDefinition::Polyline { points, closed, .. } => {
                let mut pairs = points
                    .windows(2)
                    .map(|pair| (pair[0], pair[1]))
                    .collect::<Vec<_>>();
                if *closed {
                    pairs.push((*points.last().expect("validated polyline"), points[0]));
                }
                pairs
            }
            _ => continue,
        };
        for (segment, (start, end)) in pairs.into_iter().enumerate() {
            let start = point_roots[&start];
            let end = point_roots[&end];
            segments.push(SourceSegment {
                span: CurveSpan {
                    curve: curve.id,
                    segment: u32::try_from(segment).expect("validated polyline span count"),
                },
                start,
                end,
                start_position: root_positions[&start],
                end_position: root_positions[&end],
            });
        }
    }
    segments.sort_by_key(|segment| segment.span);
    segments
}

fn classify_pair(
    first: &SourceSegment,
    second: &SourceSegment,
    pair_ordinal: usize,
    model_scale: f64,
) -> PairResult {
    let p = first.start_position;
    let q = second.start_position;
    let r = subtract(first.end_position, p);
    let s = subtract(second.end_position, q);
    let denominator = cross(r, s);
    let coordinate_scale = p
        .into_iter()
        .chain(q)
        .chain(first.end_position)
        .chain(second.end_position)
        .map(f64::abs)
        .fold(model_scale, f64::max);
    let first_length = norm(r);
    let second_length = norm(s);
    let determinant_scale = first_length.mul_add(second_length, model_scale * model_scale);
    let subtraction_scale = coordinate_scale * (first_length + second_length);
    let determinant_tolerance =
        128.0 * f64::EPSILON * determinant_scale + 8.0 * f64::EPSILON * subtraction_scale;
    let displacement = subtract(q, p);
    if denominator.abs() <= determinant_tolerance {
        let collinearity = cross(displacement, r);
        if denominator == 0.0 && collinearity == 0.0 {
            return if collinear_overlap(first, second) {
                PairResult::CollinearOverlap
            } else {
                PairResult::None
            };
        }
        return if collinearity.abs() > determinant_tolerance {
            PairResult::None
        } else {
            PairResult::Ambiguous
        };
    }
    if [first.start, first.end]
        .into_iter()
        .any(|point| point == second.start || point == second.end)
    {
        return PairResult::None;
    }
    let first_parameter = cross(displacement, s) / denominator;
    let second_parameter = cross(displacement, r) / denominator;
    let parameter_tolerance = 128.0 * f64::EPSILON;
    let first_location = parameter_location(first_parameter, parameter_tolerance);
    let second_location = parameter_location(second_parameter, parameter_tolerance);
    if matches!(first_location, ParameterLocation::Ambiguous)
        || matches!(second_location, ParameterLocation::Ambiguous)
    {
        return PairResult::Ambiguous;
    }
    if matches!(first_location, ParameterLocation::Outside)
        || matches!(second_location, ParameterLocation::Outside)
    {
        return PairResult::None;
    }
    let first_interior = first_location == ParameterLocation::Interior;
    let second_interior = second_location == ParameterLocation::Interior;
    if !first_interior && !second_interior {
        return PairResult::None;
    }
    let endpoint_vertex = |segment: &SourceSegment, location| match location {
        ParameterLocation::Start => Some(VertexKey::Persistent(segment.start)),
        ParameterLocation::End => Some(VertexKey::Persistent(segment.end)),
        _ => None,
    };
    let vertex = endpoint_vertex(first, first_location)
        .or_else(|| endpoint_vertex(second, second_location))
        .unwrap_or(VertexKey::Crossing(pair_ordinal));
    let point = match vertex {
        VertexKey::Persistent(point) if point == first.start => first.start_position,
        VertexKey::Persistent(point) if point == first.end => first.end_position,
        VertexKey::Persistent(point) if point == second.start => second.start_position,
        VertexKey::Persistent(_) => second.end_position,
        VertexKey::Crossing(_) => add(p, scale(r, first_parameter)),
    };
    if !point.into_iter().all(f64::is_finite) {
        return PairResult::Ambiguous;
    }
    PairResult::Hit(PairHit {
        first_parameter,
        second_parameter,
        point,
        vertex,
        first_interior,
        second_interior,
    })
}

fn parameter_location(parameter: f64, tolerance: f64) -> ParameterLocation {
    if !parameter.is_finite() {
        return ParameterLocation::Ambiguous;
    }
    if parameter.abs().to_bits() == 0.0_f64.to_bits() {
        ParameterLocation::Start
    } else if parameter.to_bits() == 1.0_f64.to_bits() {
        ParameterLocation::End
    } else if parameter > tolerance && parameter < 1.0 - tolerance {
        ParameterLocation::Interior
    } else if parameter < -tolerance || parameter > 1.0 + tolerance {
        ParameterLocation::Outside
    } else {
        ParameterLocation::Ambiguous
    }
}

fn collinear_overlap(first: &SourceSegment, second: &SourceSegment) -> bool {
    let first_direction = subtract(first.end_position, first.start_position);
    let use_x = first_direction[0].abs() >= first_direction[1].abs();
    let coordinate = |point: [f64; 2]| if use_x { point[0] } else { point[1] };
    let first_min = coordinate(first.start_position).min(coordinate(first.end_position));
    let first_max = coordinate(first.start_position).max(coordinate(first.end_position));
    let second_min = coordinate(second.start_position).min(coordinate(second.end_position));
    let second_max = coordinate(second.start_position).max(coordinate(second.end_position));
    first_min.max(second_min) < first_max.min(second_max)
}

#[allow(clippy::too_many_lines)]
fn extract_cycles(fragments: &[Fragment], model_scale: f64) -> (Vec<Cycle>, BTreeSet<usize>) {
    if fragments.is_empty() {
        return (Vec::new(), BTreeSet::new());
    }
    let mut vertex_ids = BTreeMap::new();
    for fragment in fragments {
        for vertex in [fragment.start, fragment.end] {
            let next = vertex_ids.len();
            vertex_ids.entry(vertex).or_insert(next);
        }
    }
    let mut positions = vec![[0.0; 2]; vertex_ids.len()];
    for fragment in fragments {
        positions[vertex_ids[&fragment.start]] = fragment.start_position;
        positions[vertex_ids[&fragment.end]] = fragment.end_position;
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
    for edges in &mut outgoing {
        edges.sort_by(|first, second| {
            let first_direction =
                subtract(positions[half_to[*first]], positions[half_from[*first]]);
            let second_direction =
                subtract(positions[half_to[*second]], positions[half_from[*second]]);
            first_direction[1]
                .atan2(first_direction[0])
                .total_cmp(&second_direction[1].atan2(second_direction[0]))
                .then_with(|| half_fragment[*first].cmp(&half_fragment[*second]))
        });
    }
    let mut next = vec![0; half_from.len()];
    for half in 0..half_from.len() {
        let destination = half_to[half];
        let twin = half ^ 1;
        let edges = &outgoing[destination];
        let reverse = edges
            .iter()
            .position(|candidate| *candidate == twin)
            .expect("twin is outgoing from destination");
        next[half] = edges[(reverse + edges.len() - 1) % edges.len()];
    }
    let area_tolerance = 256.0 * f64::EPSILON * model_scale * model_scale;
    let mut visited = vec![false; half_from.len()];
    let mut cycles = Vec::new();
    let mut ambiguous_components = BTreeSet::new();
    for start in 0..half_from.len() {
        if visited[start] {
            continue;
        }
        let mut half = start;
        let mut directed = Vec::new();
        let mut vertices = Vec::new();
        for _ in 0..=half_from.len() {
            if visited[half] {
                break;
            }
            visited[half] = true;
            vertices.push(positions[half_from[half]]);
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
        if half != start || vertices.len() < 3 {
            continue;
        }
        let area = polygon_area(&vertices);
        if !area.is_finite() || area.abs() <= area_tolerance {
            ambiguous_components.insert(fragments[directed[0].fragment].component);
        } else if area > 0.0 {
            cycles.push(Cycle {
                component: fragments[directed[0].fragment].component,
                nesting_component: nesting_roots[half_from[start]],
                area,
                vertices,
                edges: directed,
            });
        }
    }
    (cycles, ambiguous_components)
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

fn build_faces(
    cycles: &[Cycle],
    fragments: &[Fragment],
    max_containment_tests: usize,
) -> Result<Vec<VisualProfileFace>, usize> {
    let mut component_cycles: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    let cycle_bounds = cycles
        .iter()
        .map(|cycle| Bounds::from_points(&cycle.vertices))
        .collect::<Vec<_>>();
    let mut component_bounds = BTreeMap::<usize, Bounds>::new();
    for (index, cycle) in cycles.iter().enumerate() {
        component_cycles
            .entry(cycle.nesting_component)
            .or_default()
            .push(index);
        component_bounds
            .entry(cycle.nesting_component)
            .and_modify(|bounds| bounds.include_bounds(cycle_bounds[index]))
            .or_insert(cycle_bounds[index]);
    }
    let mut parents = vec![None; cycles.len()];
    let mut containment_tests = 0_usize;
    for (index, cycle) in cycles.iter().enumerate() {
        let mut parent: Option<usize> = None;
        for (component, candidates) in &component_cycles {
            if *component == cycle.nesting_component {
                continue;
            }
            charge_containment_test(&mut containment_tests, max_containment_tests)?;
            if !component_bounds[component].contains(cycle_bounds[index]) {
                continue;
            }
            for candidate in candidates {
                charge_containment_test(&mut containment_tests, max_containment_tests)?;
                if cycles[*candidate].area <= cycle.area
                    || !cycle_bounds[*candidate].contains(cycle_bounds[index])
                    || !strictly_contains(&cycles[*candidate].vertices, &cycle.vertices)
                {
                    continue;
                }
                if parent.is_none_or(|current| cycles[*candidate].area < cycles[current].area) {
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
    Ok(cycles
        .iter()
        .enumerate()
        .filter_map(|(index, cycle)| {
            let visual_area = children[index]
                .iter()
                .fold(cycle.area, |area, child| area - cycles[*child].area);
            (visual_area > 0.0).then(|| {
                let mut contours = vec![cycle_contour(cycle, fragments, false)];
                contours.extend(
                    children[index]
                        .iter()
                        .map(|child| cycle_contour(&cycles[*child], fragments, true)),
                );
                VisualProfileFace {
                    contours,
                    visual_area,
                }
            })
        })
        .collect())
}

fn charge_containment_test(count: &mut usize, limit: usize) -> Result<(), usize> {
    if *count >= limit {
        return Err(limit.saturating_add(1));
    }
    *count += 1;
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
                }
            }
        })
        .collect();
    VisualProfileContour {
        orientation: if reverse {
            VisualProfileOrientation::Clockwise
        } else {
            VisualProfileOrientation::CounterClockwise
        },
        signed_area: if reverse { -cycle.area } else { cycle.area },
        edges,
    }
}

fn compare_faces(first: &VisualProfileFace, second: &VisualProfileFace) -> Ordering {
    let first_edge = &first.contours[0].edges[0];
    let second_edge = &second.contours[0].edges[0];
    first_edge
        .source_span
        .cmp(&second_edge.source_span)
        .then_with(|| first_edge.start[0].total_cmp(&second_edge.start[0]))
        .then_with(|| first_edge.start[1].total_cmp(&second_edge.start[1]))
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
        VisualProfileIssueKind::FragmentBudgetExceeded { .. } => (1, None, None),
        VisualProfileIssueKind::ContainmentBudgetExceeded { .. } => (2, None, None),
        VisualProfileIssueKind::FaceBudgetExceeded { .. } => (3, None, None),
        VisualProfileIssueKind::InconsistentCoincidence { .. } => (4, None, None),
        VisualProfileIssueKind::CollinearOverlap { first, second } => {
            (5, Some(*first), Some(*second))
        }
        VisualProfileIssueKind::NumericalAmbiguity { first, second } => {
            (6, Some(*first), Some(*second))
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PointContainment {
    Inside,
    Outside,
    Boundary,
}

fn strictly_contains(parent: &[[f64; 2]], child: &[[f64; 2]]) -> bool {
    child
        .iter()
        .all(|point| point_in_polygon(*point, parent) == PointContainment::Inside)
}

fn point_in_polygon(point: [f64; 2], polygon: &[[f64; 2]]) -> PointContainment {
    let mut inside = false;
    for index in 0..polygon.len() {
        let first = polygon[index];
        let second = polygon[(index + 1) % polygon.len()];
        let offset = subtract(point, first);
        let edge = subtract(second, first);
        if cross(edge, offset) == 0.0
            && point[0] >= first[0].min(second[0])
            && point[0] <= first[0].max(second[0])
            && point[1] >= first[1].min(second[1])
            && point[1] <= first[1].max(second[1])
        {
            return PointContainment::Boundary;
        }
        if (first[1] > point[1]) != (second[1] > point[1]) {
            let crossing =
                (second[0] - first[0]) * (point[1] - first[1]) / (second[1] - first[1]) + first[0];
            if point[0] < crossing {
                inside = !inside;
            }
        }
    }
    if inside {
        PointContainment::Inside
    } else {
        PointContainment::Outside
    }
}

fn polygon_area(vertices: &[[f64; 2]]) -> f64 {
    let origin = vertices[0];
    let mut sum = 0.0;
    let mut correction = 0.0;
    for index in 0..vertices.len() {
        let first = subtract(vertices[index], origin);
        let second = subtract(vertices[(index + 1) % vertices.len()], origin);
        let term = cross(first, second);
        let updated = sum + term;
        if sum.abs() >= term.abs() {
            correction += (sum - updated) + term;
        } else {
            correction += (term - updated) + sum;
        }
        sum = updated;
    }
    0.5 * (sum + correction)
}

fn add(first: [f64; 2], second: [f64; 2]) -> [f64; 2] {
    [first[0] + second[0], first[1] + second[1]]
}

fn subtract(first: [f64; 2], second: [f64; 2]) -> [f64; 2] {
    [first[0] - second[0], first[1] - second[1]]
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

#[cfg(test)]
mod tests {
    use super::{
        Cycle, DirectedFragment, DisjointSet, Fragment, VertexKey, build_faces,
        candidate_pair_count,
    };
    use crate::{CurveId, CurveSpan, DesignPointId, PersistentId};

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
    fn dense_same_component_faces_do_not_enter_quadratic_nesting() {
        let point = DesignPointId(PersistentId::from_u128(1));
        let fragment = Fragment {
            start: VertexKey::Persistent(point),
            end: VertexKey::Crossing(0),
            start_position: [0.0, 0.0],
            end_position: [1.0, 0.0],
            source_span: CurveSpan {
                curve: CurveId(PersistentId::from_u128(2)),
                segment: 0,
            },
            source_parameters: [0.0, 1.0],
            component: 0,
        };
        let cycles = (0..20_000)
            .map(|index| Cycle {
                component: 0,
                nesting_component: 0,
                area: 1.0,
                vertices: vec![[0.0, 0.0], [1.0, 0.0], [0.0, 2.0]],
                edges: vec![DirectedFragment {
                    fragment: 0,
                    forward: index % 2 == 0,
                }],
            })
            .collect::<Vec<_>>();
        assert_eq!(
            build_faces(&cycles, std::slice::from_ref(&fragment), 0)
                .unwrap()
                .len(),
            cycles.len()
        );

        let mut separated = cycles;
        separated.extend((0..20_000).map(|index| Cycle {
            component: 1,
            nesting_component: 1,
            area: 2.0,
            vertices: vec![[100.0, 0.0], [101.0, 0.0], [100.0, 4.0]],
            edges: vec![DirectedFragment {
                fragment: 0,
                forward: index % 2 == 0,
            }],
        }));
        assert_eq!(
            build_faces(&separated, &[fragment], 40_000).unwrap().len(),
            separated.len()
        );
    }
}
