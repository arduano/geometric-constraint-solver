// SPDX-License-Identifier: GPL-3.0-or-later

//! Grouped, presentation-independent authoring for computed sketch features.

use geosolve_sketch::{
    CurveDefinition, CurveSpan, DesignPointId, DocumentFilletTrimEndpoint, OperationControl,
    OperationOutcome, PreparedSketchInput, SketchAcceptedStateIdentity, SketchDocument,
};
use geosolve_sketch_features::{
    ComputedCircularArc, ComputedFeatureAuthoringError, ComputedFeatureAuthoringSnapshot,
    ComputedFeatureEvaluationPolicy, ComputedFilletAuthoringOptions,
    ComputedFilletCornerAuthoringRequest, ComputedFilletCurvePick, ContinuedComputedFilletCorner,
    NativeCurveSpanSource, NewComputedFilletCorner,
};

use crate::coordinator::computed_feature_authoring_control;
use crate::{EditorScene, PickTolerance, ScreenPoint, SelectionItem};

const MAX_GROUPED_FILLET_CORNERS: usize = 16_384;
const MAX_FEATURE_AUTHORING_HIT_CANDIDATES: usize = 256;
const MAX_FEATURE_AUTHORING_SEMANTIC_TARGETS: usize = 16_384;

/// Closed computed-feature palette owned by this authoring state machine.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FeatureAuthoringTool {
    Fillet,
}

/// One exact accepted native-curve pick.
#[derive(Clone, Debug, PartialEq)]
pub struct FeatureAuthoringPick {
    pub curve: ComputedFilletCurvePick,
    sketch_input: PreparedSketchInput,
    accepted: SketchAcceptedStateIdentity,
    span_endpoints: Option<(DesignPointId, DesignPointId)>,
}

impl FeatureAuthoringPick {
    #[must_use]
    pub const fn sketch_input(&self) -> PreparedSketchInput {
        self.sketch_input
    }

    #[must_use]
    pub const fn accepted_state_identity(&self) -> SketchAcceptedStateIdentity {
        self.accepted
    }
}

/// One semantic click/preselection target. A corner stays atomic instead of
/// being flattened into two unrelated clicks that can cross-pair with an
/// already pending curve.
#[derive(Clone, Debug, PartialEq)]
enum FeatureAuthoringTarget {
    Curve(Box<FeatureAuthoringPick>),
    Corner(Box<[FeatureAuthoringPick; 2]>),
}

type FeatureCornerOccurrence = (CurveSpan, f64, DocumentFilletTrimEndpoint);
type FeatureCornerIncidenceIndex =
    std::collections::BTreeMap<DesignPointId, Vec<FeatureCornerOccurrence>>;
const MAX_RETAINED_FEATURE_CORNER_OCCURRENCES: usize = 3;

/// Process-local shared-radius and next-corner branch choices.
///
/// `fillet_radius: None` is an absent host override. Once authoring is active,
/// the state retains its remembered or model-scale default radius rather than
/// allowing an optional presentation field to erase that required value.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FeatureAuthoringOptions {
    pub fillet_radius: Option<f64>,
    pub flip_first_side: bool,
    pub flip_second_side: bool,
    pub alternate_arc: bool,
}

impl FeatureAuthoringOptions {
    const fn branch_options(self) -> ComputedFilletAuthoringOptions {
        ComputedFilletAuthoringOptions {
            flip_first_side: self.flip_first_side,
            flip_second_side: self.flip_second_side,
            alternate_arc: self.alternate_arc,
        }
    }
}

/// Stable grouped-authoring progression.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FeatureAuthoringStage {
    PickFirstFilletCurve,
    PickSecondFilletCurve,
    PreviewReady,
}

/// Current headless guidance.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FeatureAuthoringGuidance {
    pub tool: FeatureAuthoringTool,
    pub stage: FeatureAuthoringStage,
    pub completed_corners: usize,
    pub message: &'static str,
}

/// Typed non-mutating authoring warning.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum FeatureAuthoringWarningKind {
    MissingObject,
    WrongOperandKind,
    NonFinitePick,
    StalePick,
    DuplicateSupport,
    IncompleteCorner,
    UnsupportedCurveFamily,
    UnsupportedFilletPair,
    SingularFillet,
    AmbiguousFilletRoot,
    AmbiguousTrimSide,
    InvalidRadius,
    WorkStopped,
}

/// One warning with stable stage context.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FeatureAuthoringWarning {
    pub tool: FeatureAuthoringTool,
    pub stage: FeatureAuthoringStage,
    pub kind: FeatureAuthoringWarningKind,
    pub message: String,
}

/// One resolved corner retained by a grouped candidate.
#[derive(Clone, Debug, PartialEq)]
pub struct FeatureAuthoringCornerPreview {
    pub corner: NewComputedFilletCorner,
    pub arc: ComputedCircularArc,
    pub options: ComputedFilletAuthoringOptions,
}

#[derive(Clone, Debug, PartialEq)]
struct FeatureAuthoringCornerDraft {
    picks: [FeatureAuthoringPick; 2],
    preview: FeatureAuthoringCornerPreview,
}

/// Complete immutable grouped feature request.
#[derive(Clone, Debug, PartialEq)]
pub struct FeatureAuthoringCandidate {
    tool: FeatureAuthoringTool,
    radius: f64,
    corners: Vec<FeatureAuthoringCornerPreview>,
    sketch_input: PreparedSketchInput,
    accepted: SketchAcceptedStateIdentity,
}

impl FeatureAuthoringCandidate {
    #[must_use]
    pub const fn tool(&self) -> FeatureAuthoringTool {
        self.tool
    }

    #[must_use]
    pub const fn radius(&self) -> f64 {
        self.radius
    }

    #[must_use]
    pub fn corners(&self) -> &[FeatureAuthoringCornerPreview] {
        &self.corners
    }

    #[must_use]
    pub const fn sketch_input(&self) -> PreparedSketchInput {
        self.sketch_input
    }

    #[must_use]
    pub const fn accepted_state_identity(&self) -> SketchAcceptedStateIdentity {
        self.accepted
    }

    #[must_use]
    pub fn persistent_corners(&self) -> Vec<NewComputedFilletCorner> {
        self.corners.iter().map(|value| value.corner).collect()
    }
}

/// Result of one grouped feature-authoring transition.
#[derive(Clone, Debug, PartialEq)]
pub enum FeatureAuthoringOutcome {
    ModeEntered(FeatureAuthoringGuidance),
    /// A screen pick found no native sketch item. Presentation adapters may use
    /// this state-neutral result to try a computed preview radius grip without
    /// allowing that generated geometry to mask any native candidate.
    NoNativeHit(FeatureAuthoringGuidance),
    Collecting {
        pending: Vec<FeatureAuthoringPick>,
        guidance: FeatureAuthoringGuidance,
    },
    PreviewRequested {
        candidate: FeatureAuthoringCandidate,
        guidance: FeatureAuthoringGuidance,
    },
    Apply(FeatureAuthoringCandidate),
    Warning(FeatureAuthoringWarning),
    CandidateCleared(FeatureAuthoringGuidance),
    ModeExited,
    Inactive,
}

/// Reusable grouped computed-feature collector.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FeatureAuthoringState {
    active: Option<FeatureAuthoringTool>,
    pending: Vec<FeatureAuthoringPick>,
    corners: Vec<FeatureAuthoringCornerDraft>,
    options: FeatureAuthoringOptions,
}

enum FeatureAuthoringPickResolution {
    Accepted {
        state: FeatureAuthoringState,
        outcome: FeatureAuthoringOutcome,
        item: SelectionItem,
    },
    Rejected(FeatureAuthoringOutcome),
}

impl FeatureAuthoringState {
    #[must_use]
    pub const fn active_tool(&self) -> Option<FeatureAuthoringTool> {
        self.active
    }

    #[must_use]
    pub const fn options(&self) -> FeatureAuthoringOptions {
        self.options
    }

    #[must_use]
    pub fn completed_corner_count(&self) -> usize {
        self.corners.len()
    }

    /// Activates grouped Fillet authoring and consumes every complete preselected
    /// semantic target. Point corners remain atomic when mixed with curve picks.
    #[must_use]
    pub fn activate(
        &mut self,
        snapshot: &ComputedFeatureAuthoringSnapshot,
        document: &SketchDocument,
        tool: FeatureAuthoringTool,
        selection: &[(SelectionItem, Option<f64>)],
    ) -> FeatureAuthoringOutcome {
        self.active = Some(tool);
        self.pending.clear();
        self.corners.clear();
        self.ensure_radius(document);
        if selection.is_empty() {
            return FeatureAuthoringOutcome::ModeEntered(self.guidance());
        }
        self.pick_items(snapshot, document, selection)
    }

    /// Activates directly from coordinator-stamped exact picks.
    #[must_use]
    pub fn activate_picks(
        &mut self,
        snapshot: &ComputedFeatureAuthoringSnapshot,
        document: &SketchDocument,
        tool: FeatureAuthoringTool,
        picks: impl IntoIterator<Item = FeatureAuthoringPick>,
    ) -> FeatureAuthoringOutcome {
        self.active = Some(tool);
        self.pending.clear();
        self.corners.clear();
        self.ensure_radius(document);
        let mut picks = picks.into_iter();
        let Some(first) = picks.next() else {
            return FeatureAuthoringOutcome::ModeEntered(self.guidance());
        };
        self.pick_many(snapshot, std::iter::once(first).chain(picks))
    }

    /// Adds exact accepted picks, resolving each completed pair immediately.
    #[must_use]
    pub fn pick_many(
        &mut self,
        snapshot: &ComputedFeatureAuthoringSnapshot,
        picks: impl IntoIterator<Item = FeatureAuthoringPick>,
    ) -> FeatureAuthoringOutcome {
        self.pick_many_controlled(snapshot, picks, computed_feature_authoring_control())
    }

    /// Adds semantic native items as one atomic transition. A point that owns
    /// exactly two incident spans is one complete corner, not an untyped stream
    /// of two curve picks.
    #[must_use]
    pub fn pick_items(
        &mut self,
        snapshot: &ComputedFeatureAuthoringSnapshot,
        document: &SketchDocument,
        items: &[(SelectionItem, Option<f64>)],
    ) -> FeatureAuthoringOutcome {
        if self.active.is_none() {
            return FeatureAuthoringOutcome::Inactive;
        }
        if items.len() > MAX_FEATURE_AUTHORING_SEMANTIC_TARGETS {
            return self.warning(
                FeatureAuthoringWarningKind::WorkStopped,
                "Fillet semantic target limit was exhausted",
            );
        }
        let selected_points = items
            .iter()
            .filter_map(|(item, _)| match item {
                SelectionItem::Point(point) => Some(*point),
                SelectionItem::Curve(_)
                | SelectionItem::Constraint(_)
                | SelectionItem::Dimension(_)
                | SelectionItem::Datum(_)
                | SelectionItem::Feature(_)
                | SelectionItem::FeatureCorner(_) => None,
            })
            .collect::<std::collections::BTreeSet<_>>();
        let incidences = match feature_corner_incidence_index(document, &selected_points) {
            Ok(incidences) => incidences,
            Err(kind) => {
                return self.warning(kind, "the selected item is not a Fillet corner operand");
            }
        };
        let mut targets = Vec::new();
        for (item, parameter) in items {
            if matches!(
                item,
                SelectionItem::Feature(_) | SelectionItem::FeatureCorner(_)
            ) {
                // Computed output is deliberately not a sketch operand. Ignore a
                // prior result selection so it cannot poison entry into the next
                // Fillet batch.
                continue;
            }
            let target = match resolve_feature_item_target_with_incidence(
                snapshot,
                document,
                *item,
                *parameter,
                &incidences,
            ) {
                Ok(target) => target,
                Err(kind) => {
                    return self.warning(kind, "the selected item is not a Fillet corner operand");
                }
            };
            targets.push(target);
        }
        if targets.is_empty() && self.pending.is_empty() && self.corners.is_empty() {
            return FeatureAuthoringOutcome::ModeEntered(self.guidance());
        }
        self.pick_targets_controlled(snapshot, targets, computed_feature_authoring_control())
    }

    /// Resolves one screen click using domain-aware fallback over every native
    /// hit candidate. An inapplicable point cannot mask a curve beneath it, and
    /// an already-pending span cannot mask a distinct overlapping second span.
    #[must_use]
    pub fn pick_at(
        &mut self,
        snapshot: &ComputedFeatureAuthoringSnapshot,
        document: &SketchDocument,
        scene: &EditorScene,
        position: ScreenPoint,
        tolerance: PickTolerance,
    ) -> FeatureAuthoringOutcome {
        self.pick_at_with_policy(
            snapshot,
            document,
            scene,
            position,
            tolerance,
            crate::GeometryInteractionPolicy::default(),
        )
    }

    /// Resolves one screen click using the current headless geometry filtering
    /// policy and domain-aware fallback over every remaining native candidate.
    #[must_use]
    pub fn pick_at_with_policy(
        &mut self,
        snapshot: &ComputedFeatureAuthoringSnapshot,
        document: &SketchDocument,
        scene: &EditorScene,
        position: ScreenPoint,
        tolerance: PickTolerance,
        policy: crate::GeometryInteractionPolicy,
    ) -> FeatureAuthoringOutcome {
        match self
            .resolve_pick_at_with_policy(snapshot, document, scene, position, tolerance, policy)
        {
            FeatureAuthoringPickResolution::Accepted { state, outcome, .. } => {
                *self = state;
                outcome
            }
            FeatureAuthoringPickResolution::Rejected(outcome) => outcome,
        }
    }

    /// Resolves the exact native semantic item that an unchanged Fillet press
    /// would accept next, without mutating the grouped candidate.
    pub(crate) fn hover_item_at_with_policy(
        &self,
        snapshot: &ComputedFeatureAuthoringSnapshot,
        document: &SketchDocument,
        scene: &EditorScene,
        position: ScreenPoint,
        tolerance: PickTolerance,
        policy: crate::GeometryInteractionPolicy,
    ) -> Option<SelectionItem> {
        match self
            .resolve_pick_at_with_policy(snapshot, document, scene, position, tolerance, policy)
        {
            FeatureAuthoringPickResolution::Accepted { item, .. } => Some(item),
            FeatureAuthoringPickResolution::Rejected(_) => None,
        }
    }

    #[allow(
        clippy::too_many_lines,
        reason = "one shared hover/click resolver keeps Fillet incidence fallback and warning precedence atomic"
    )]
    fn resolve_pick_at_with_policy(
        &self,
        snapshot: &ComputedFeatureAuthoringSnapshot,
        document: &SketchDocument,
        scene: &EditorScene,
        position: ScreenPoint,
        tolerance: PickTolerance,
        policy: crate::GeometryInteractionPolicy,
    ) -> FeatureAuthoringPickResolution {
        if self.active.is_none() {
            return FeatureAuthoringPickResolution::Rejected(FeatureAuthoringOutcome::Inactive);
        }
        if scene.accepted_revision != snapshot.accepted_state_identity().revision().get()
            || scene.design_identity != snapshot.sketch_input().design_identity()
            || document.id() != snapshot.accepted_state_identity().document()
        {
            return FeatureAuthoringPickResolution::Rejected(self.warning(
                FeatureAuthoringWarningKind::StalePick,
                "the visible Fillet hit scene belongs to an older accepted sketch input",
            ));
        }
        let hits = match scene.native_authoring_hit_candidates_with_policy(
            position,
            tolerance,
            MAX_FEATURE_AUTHORING_HIT_CANDIDATES,
            policy,
        ) {
            Ok(hits) => hits,
            Err(crate::NativeAuthoringHitError::CandidateLimitExceeded { .. }) => {
                return FeatureAuthoringPickResolution::Rejected(self.warning(
                    FeatureAuthoringWarningKind::WorkStopped,
                    "too many overlapping native Fillet hit candidates",
                ));
            }
        };
        if hits.is_empty() {
            return FeatureAuthoringPickResolution::Rejected(FeatureAuthoringOutcome::NoNativeHit(
                self.guidance(),
            ));
        }
        let incidences = match feature_hit_incidence_index(document, &hits) {
            Ok(incidences) => incidences,
            Err(kind) => {
                return FeatureAuthoringPickResolution::Rejected(self.warning(
                    kind,
                    "the native point under this click is not a current Fillet operand",
                ));
            }
        };
        let mut first_resolution_warning = None;
        let mut duplicate_support_warning = None;
        for hit in hits {
            let item = hit.item;
            let target = match resolve_feature_item_target_with_incidence(
                snapshot,
                document,
                hit.item,
                hit.curve_parameter,
                &incidences,
            ) {
                Ok(target) => target,
                Err(kind) => {
                    if matches!(hit.item, SelectionItem::Point(_))
                        && kind != FeatureAuthoringWarningKind::WrongOperandKind
                    {
                        return FeatureAuthoringPickResolution::Rejected(self.warning(
                            kind,
                            "the native point under this click is not an unambiguous Fillet corner",
                        ));
                    }
                    first_resolution_warning.get_or_insert(kind);
                    continue;
                }
            };
            let is_corner = matches!(target, FeatureAuthoringTarget::Corner(_));
            let mut trial = self.clone();
            let outcome = trial.pick_targets_controlled(
                snapshot,
                std::iter::once(target),
                computed_feature_authoring_control(),
            );
            match outcome {
                FeatureAuthoringOutcome::Warning(warning) => {
                    if is_corner {
                        return FeatureAuthoringPickResolution::Rejected(
                            FeatureAuthoringOutcome::Warning(warning),
                        );
                    }
                    if warning.kind == FeatureAuthoringWarningKind::DuplicateSupport {
                        duplicate_support_warning.get_or_insert(warning);
                    } else {
                        return FeatureAuthoringPickResolution::Rejected(
                            FeatureAuthoringOutcome::Warning(warning),
                        );
                    }
                }
                accepted => {
                    return FeatureAuthoringPickResolution::Accepted {
                        state: trial,
                        outcome: accepted,
                        item,
                    };
                }
            }
        }
        if let Some(warning) = duplicate_support_warning {
            FeatureAuthoringPickResolution::Rejected(FeatureAuthoringOutcome::Warning(warning))
        } else {
            FeatureAuthoringPickResolution::Rejected(self.warning(
                first_resolution_warning.unwrap_or(FeatureAuthoringWarningKind::WrongOperandKind),
                "no applicable native Fillet operand is under this click",
            ))
        }
    }

    /// Controlled counterpart used to qualify cancellation and exhaustion.
    #[must_use]
    pub fn pick_many_controlled(
        &mut self,
        snapshot: &ComputedFeatureAuthoringSnapshot,
        picks: impl IntoIterator<Item = FeatureAuthoringPick>,
        control: OperationControl,
    ) -> FeatureAuthoringOutcome {
        self.pick_targets_controlled(
            snapshot,
            picks
                .into_iter()
                .map(|pick| FeatureAuthoringTarget::Curve(Box::new(pick))),
            control,
        )
    }

    fn pick_targets_controlled(
        &mut self,
        snapshot: &ComputedFeatureAuthoringSnapshot,
        targets: impl IntoIterator<Item = FeatureAuthoringTarget>,
        control: OperationControl,
    ) -> FeatureAuthoringOutcome {
        if self.active.is_none() {
            return FeatureAuthoringOutcome::Inactive;
        }
        if !self.matches_snapshot(snapshot) {
            return self.warning(
                FeatureAuthoringWarningKind::StalePick,
                "the active Fillet batch belongs to an older accepted sketch input",
            );
        }
        let mut next = self.clone();
        let mut requests = Vec::new();
        for target in targets {
            if !target_matches_snapshot(&target, snapshot) {
                return self.warning(
                    FeatureAuthoringWarningKind::StalePick,
                    "the pick belongs to an older accepted sketch input",
                );
            }
            let pair = match target {
                FeatureAuthoringTarget::Curve(pick) => {
                    next.pending.push(*pick);
                    if next.pending.len() < 2 {
                        continue;
                    }
                    if next.pending.len() > 2 {
                        return self.warning(
                            FeatureAuthoringWarningKind::WorkStopped,
                            "Fillet authoring accumulated an invalid pending operand count",
                        );
                    }
                    [next.pending.remove(0), next.pending.remove(0)]
                }
                FeatureAuthoringTarget::Corner(pair) => match next.pending.as_slice() {
                    [] => *pair,
                    [pending] => {
                        let matches_first = same_support(pending, &pair[0]);
                        let matches_second = same_support(pending, &pair[1]);
                        let other = match (matches_first, matches_second) {
                            (true, false) => pair[1].clone(),
                            (false, true) => pair[0].clone(),
                            _ => {
                                return self.warning(
                                    FeatureAuthoringWarningKind::AmbiguousTrimSide,
                                    "the corner does not unambiguously complete the pending Fillet support",
                                );
                            }
                        };
                        [next.pending.remove(0), other]
                    }
                    _ => {
                        return self.warning(
                            FeatureAuthoringWarningKind::WorkStopped,
                            "Fillet authoring accumulated an invalid pending operand count",
                        );
                    }
                },
            };
            if next.corners.len() + requests.len() >= MAX_GROUPED_FILLET_CORNERS {
                return self.warning(
                    FeatureAuthoringWarningKind::WorkStopped,
                    "grouped Fillet corner limit was exhausted",
                );
            }
            requests.push((pair, next.options));
        }
        match resolve_corners(snapshot, requests, control) {
            Ok(mut corners) => next.corners.append(&mut corners),
            Err((kind, message)) => return self.warning(kind, message),
        }
        *self = next;
        self.current_outcome()
    }

    /// Updates the shared radius and next-corner branch choices. Every completed
    /// corner is re-resolved atomically from its exact original picks.
    #[must_use]
    pub fn set_options(
        &mut self,
        snapshot: &ComputedFeatureAuthoringSnapshot,
        options: FeatureAuthoringOptions,
    ) -> FeatureAuthoringOutcome {
        self.update_options_once(snapshot, options, None)
    }

    /// Atomically updates the shared radius and next-corner defaults while also
    /// applying those branch controls to one selected completed corner. Every
    /// completed corner is re-resolved once under one aggregate work envelope.
    #[must_use]
    pub fn set_options_with_corner(
        &mut self,
        snapshot: &ComputedFeatureAuthoringSnapshot,
        options: FeatureAuthoringOptions,
        selected_corner: Option<usize>,
    ) -> FeatureAuthoringOutcome {
        self.update_options_once(
            snapshot,
            options,
            selected_corner.map(|index| (index, options.branch_options())),
        )
    }

    fn update_options_once(
        &mut self,
        snapshot: &ComputedFeatureAuthoringSnapshot,
        mut options: FeatureAuthoringOptions,
        selected_corner: Option<(usize, ComputedFilletAuthoringOptions)>,
    ) -> FeatureAuthoringOutcome {
        if self.active.is_none() {
            self.options = options;
            return FeatureAuthoringOutcome::Inactive;
        }
        if !self.matches_snapshot(snapshot) {
            return self.warning(
                FeatureAuthoringWarningKind::StalePick,
                "the active Fillet batch belongs to an older accepted sketch input",
            );
        }
        if options.fillet_radius.is_none() {
            options.fillet_radius = self.options.fillet_radius;
        }
        if !valid_radius(options.fillet_radius) {
            return self.warning(
                FeatureAuthoringWarningKind::InvalidRadius,
                "Fillet radius must be finite and positive",
            );
        }
        if selected_corner.is_some_and(|(index, _)| index >= self.corners.len()) {
            return self.warning(
                FeatureAuthoringWarningKind::MissingObject,
                "the Fillet corner no longer exists",
            );
        }
        let mut next = self.clone();
        next.options = options;
        let requests = next
            .corners
            .iter()
            .enumerate()
            .map(|(index, draft)| {
                let branches = selected_corner
                    .filter(|(selected, _)| *selected == index)
                    .map_or(draft.preview.options, |(_, branches)| branches);
                (
                    draft.picks.clone(),
                    FeatureAuthoringOptions {
                        fillet_radius: options.fillet_radius,
                        flip_first_side: branches.flip_first_side,
                        flip_second_side: branches.flip_second_side,
                        alternate_arc: branches.alternate_arc,
                    },
                )
            })
            .collect();
        next.corners =
            match resolve_corners(snapshot, requests, computed_feature_authoring_control()) {
                Ok(corners) => corners,
                Err((kind, message)) => return self.warning(kind, message),
            };
        *self = next;
        self.current_outcome()
    }

    /// Changes one completed corner's explicit branch controls without affecting
    /// other corners in the set.
    #[must_use]
    pub fn set_corner_options(
        &mut self,
        snapshot: &ComputedFeatureAuthoringSnapshot,
        index: usize,
        options: ComputedFilletAuthoringOptions,
    ) -> FeatureAuthoringOutcome {
        self.update_options_once(snapshot, self.options, Some((index, options)))
    }

    /// Continues every completed corner from its exact absolute branch intent
    /// to one new shared radius.
    ///
    /// Unlike [`Self::set_options`], this path never reconstructs a completed
    /// corner from its original screen picks or relative flip booleans. The
    /// feature-domain continuation must complete for the whole batch before
    /// this state changes. An explicit numeric edit may depart a rail-less fold
    /// only through its persisted local branch cell; invalid radius, ambiguous
    /// branch, cancellation or exhausted work retains the exact prior candidate.
    #[must_use]
    pub fn continue_radius_absolute(
        &mut self,
        snapshot: &ComputedFeatureAuthoringSnapshot,
        radius: f64,
    ) -> FeatureAuthoringOutcome {
        if self.active.is_none() {
            return FeatureAuthoringOutcome::Inactive;
        }
        if !self.matches_snapshot(snapshot) {
            return self.warning(
                FeatureAuthoringWarningKind::StalePick,
                "the active Fillet batch belongs to an older accepted sketch input",
            );
        }
        if !radius.is_finite() || radius <= 0.0 {
            return self.warning(
                FeatureAuthoringWarningKind::InvalidRadius,
                "Fillet radius must be finite and positive",
            );
        }
        if self.corners.is_empty() {
            let mut next = self.clone();
            next.options.fillet_radius = Some(radius);
            *self = next;
            return self.current_outcome();
        }
        let Some(from_radius) = self
            .options
            .fillet_radius
            .filter(|value| value.is_finite() && *value > 0.0)
        else {
            return self.warning(
                FeatureAuthoringWarningKind::InvalidRadius,
                "the current Fillet radius is not valid continuation state",
            );
        };
        let priors = self
            .corners
            .iter()
            .map(|draft| draft.preview.corner)
            .collect::<Vec<_>>();
        let outcome = match snapshot.continue_fillet_corners_numeric(
            &priors,
            from_radius,
            radius,
            ComputedFeatureEvaluationPolicy::default(),
            computed_feature_authoring_control(),
        ) {
            Ok(outcome) => outcome,
            Err(error) => {
                return self.warning(
                    map_authoring_error(&error),
                    format!("Fillet radius continuation was rejected: {error}"),
                );
            }
        };
        let OperationOutcome::Completed {
            value: continued, ..
        } = outcome
        else {
            return self.warning(
                FeatureAuthoringWarningKind::WorkStopped,
                "Fillet radius continuation exhausted its bounded work envelope",
            );
        };
        if continued.len() != self.corners.len()
            || continued.iter().any(|value| {
                value.sketch_input != snapshot.sketch_input()
                    || value.accepted != snapshot.accepted_state_identity()
                    || value.arc.radius.to_bits() != radius.to_bits()
            })
        {
            return self.warning(
                FeatureAuthoringWarningKind::StalePick,
                "Fillet radius continuation returned mismatched accepted input",
            );
        }
        let mut next = self.clone();
        next.options.fillet_radius = Some(radius);
        for (draft, value) in next.corners.iter_mut().zip(continued) {
            draft.preview.corner = value.corner;
            draft.preview.arc = value.arc;
        }
        *self = next;
        self.current_outcome()
    }

    /// Replaces one completed corner with an independently validated absolute
    /// continuation while retaining the original semantic picks needed to add
    /// further corners to the same authoring batch.
    pub(crate) fn replace_corner_absolute(
        &mut self,
        snapshot: &ComputedFeatureAuthoringSnapshot,
        index: usize,
        continued: ContinuedComputedFilletCorner,
    ) -> FeatureAuthoringOutcome {
        if self.active.is_none() {
            return FeatureAuthoringOutcome::Inactive;
        }
        if !self.matches_snapshot(snapshot)
            || continued.sketch_input != snapshot.sketch_input()
            || continued.accepted != snapshot.accepted_state_identity()
        {
            return self.warning(
                FeatureAuthoringWarningKind::StalePick,
                "the replacement Fillet corner belongs to an older accepted sketch input",
            );
        }
        let Some(_) = self.corners.get(index) else {
            return self.warning(
                FeatureAuthoringWarningKind::MissingObject,
                "the replacement Fillet corner no longer exists",
            );
        };
        let radius = continued.arc.radius;
        if !radius.is_finite() || radius <= 0.0 {
            return self.warning(
                FeatureAuthoringWarningKind::InvalidRadius,
                "the replacement Fillet corner has an invalid radius",
            );
        }
        let mut next = self.clone();
        next.options.fillet_radius = Some(radius);
        next.corners[index].preview.corner = continued.corner;
        next.corners[index].preview.arc = continued.arc;
        *self = next;
        self.current_outcome()
    }

    /// Applies the complete batch immediately; no final canvas radius click exists.
    #[must_use]
    pub fn apply(&self) -> FeatureAuthoringOutcome {
        if self.active.is_none() {
            return FeatureAuthoringOutcome::Inactive;
        }
        if !self.pending.is_empty() || self.corners.is_empty() {
            return FeatureAuthoringOutcome::Warning(FeatureAuthoringWarning {
                tool: FeatureAuthoringTool::Fillet,
                stage: self.guidance().stage,
                kind: FeatureAuthoringWarningKind::IncompleteCorner,
                message: "complete at least one two-parent Fillet corner before applying".into(),
            });
        }
        FeatureAuthoringOutcome::Apply(self.candidate())
    }

    #[must_use]
    pub fn enter(&self) -> FeatureAuthoringOutcome {
        self.apply()
    }

    pub fn deactivate(&mut self) {
        self.active = None;
        self.pending.clear();
        self.corners.clear();
    }

    #[must_use]
    pub fn publication_succeeded(&mut self) -> FeatureAuthoringOutcome {
        if self.active.is_none() {
            return FeatureAuthoringOutcome::Inactive;
        }
        self.deactivate();
        FeatureAuthoringOutcome::ModeExited
    }

    /// First Escape clears the batch, and a second Escape exits the mode.
    #[must_use]
    pub fn cancel(&mut self) -> FeatureAuthoringOutcome {
        if self.active.is_none() {
            return FeatureAuthoringOutcome::Inactive;
        }
        if self.pending.is_empty() && self.corners.is_empty() {
            self.active = None;
            FeatureAuthoringOutcome::ModeExited
        } else {
            self.pending.clear();
            self.corners.clear();
            FeatureAuthoringOutcome::CandidateCleared(self.guidance())
        }
    }

    #[must_use]
    pub fn guidance(&self) -> FeatureAuthoringGuidance {
        let stage = if self.pending.is_empty() {
            if self.corners.is_empty() {
                FeatureAuthoringStage::PickFirstFilletCurve
            } else {
                FeatureAuthoringStage::PreviewReady
            }
        } else {
            FeatureAuthoringStage::PickSecondFilletCurve
        };
        let message = match stage {
            FeatureAuthoringStage::PickFirstFilletCurve => {
                "Pick two native curves, or one unambiguous polyline corner"
            }
            FeatureAuthoringStage::PickSecondFilletCurve => {
                "Pick the distinct second native curve for this corner"
            }
            FeatureAuthoringStage::PreviewReady => {
                "Pick more corners, or Apply/Enter to create one shared-radius Fillet set"
            }
        };
        FeatureAuthoringGuidance {
            tool: self.active.unwrap_or(FeatureAuthoringTool::Fillet),
            stage,
            completed_corners: self.corners.len(),
            message,
        }
    }

    fn ensure_radius(&mut self, document: &SketchDocument) {
        if !valid_radius(self.options.fillet_radius) {
            self.options.fillet_radius = Some(0.1 * document.model_scale());
        }
    }

    fn matches_snapshot(&self, snapshot: &ComputedFeatureAuthoringSnapshot) -> bool {
        self.pending
            .iter()
            .chain(self.corners.iter().flat_map(|corner| corner.picks.iter()))
            .all(|pick| pick_matches_snapshot(pick, snapshot))
    }

    fn candidate(&self) -> FeatureAuthoringCandidate {
        let first = self
            .corners
            .first()
            .expect("a candidate is built only for a non-empty corner batch");
        FeatureAuthoringCandidate {
            tool: FeatureAuthoringTool::Fillet,
            radius: self
                .options
                .fillet_radius
                .expect("active feature authoring owns a positive radius"),
            corners: self
                .corners
                .iter()
                .map(|value| value.preview.clone())
                .collect(),
            sketch_input: first.picks[0].sketch_input,
            accepted: first.picks[0].accepted,
        }
    }

    fn current_outcome(&self) -> FeatureAuthoringOutcome {
        if !self.corners.is_empty() && self.pending.is_empty() {
            FeatureAuthoringOutcome::PreviewRequested {
                candidate: self.candidate(),
                guidance: self.guidance(),
            }
        } else {
            FeatureAuthoringOutcome::Collecting {
                pending: self.pending.clone(),
                guidance: self.guidance(),
            }
        }
    }

    fn warning(
        &self,
        kind: FeatureAuthoringWarningKind,
        message: impl Into<String>,
    ) -> FeatureAuthoringOutcome {
        FeatureAuthoringOutcome::Warning(FeatureAuthoringWarning {
            tool: self.active.unwrap_or(FeatureAuthoringTool::Fillet),
            stage: self.guidance().stage,
            kind,
            message: message.into(),
        })
    }
}

pub(crate) fn resolve_feature_item_picks(
    snapshot: &ComputedFeatureAuthoringSnapshot,
    document: &SketchDocument,
    item: SelectionItem,
    parameter: Option<f64>,
) -> Result<Vec<FeatureAuthoringPick>, FeatureAuthoringWarningKind> {
    resolve_feature_item_target(snapshot, document, item, parameter).map(|target| match target {
        FeatureAuthoringTarget::Curve(pick) => vec![*pick],
        FeatureAuthoringTarget::Corner(picks) => Vec::from(*picks),
    })
}

fn resolve_feature_item_target(
    snapshot: &ComputedFeatureAuthoringSnapshot,
    document: &SketchDocument,
    item: SelectionItem,
    parameter: Option<f64>,
) -> Result<FeatureAuthoringTarget, FeatureAuthoringWarningKind> {
    let selected_points = match item {
        SelectionItem::Point(point) => std::collections::BTreeSet::from([point]),
        SelectionItem::Curve(_)
        | SelectionItem::Constraint(_)
        | SelectionItem::Dimension(_)
        | SelectionItem::Datum(_)
        | SelectionItem::Feature(_)
        | SelectionItem::FeatureCorner(_) => std::collections::BTreeSet::new(),
    };
    let incidences = feature_corner_incidence_index(document, &selected_points)?;
    resolve_feature_item_target_with_incidence(snapshot, document, item, parameter, &incidences)
}

fn resolve_feature_item_target_with_incidence(
    snapshot: &ComputedFeatureAuthoringSnapshot,
    document: &SketchDocument,
    item: SelectionItem,
    parameter: Option<f64>,
    incidences: &FeatureCornerIncidenceIndex,
) -> Result<FeatureAuthoringTarget, FeatureAuthoringWarningKind> {
    match item {
        SelectionItem::Curve(span) => Ok(FeatureAuthoringTarget::Curve(Box::new(
            feature_curve_pick(snapshot, document, span, parameter, None)?,
        ))),
        SelectionItem::Point(point) => {
            let picks =
                resolve_feature_corner_point_from_incidence(snapshot, document, point, incidences)?;
            let picks = <[FeatureAuthoringPick; 2]>::try_from(picks)
                .map_err(|_| FeatureAuthoringWarningKind::AmbiguousTrimSide)?;
            Ok(FeatureAuthoringTarget::Corner(Box::new(picks)))
        }
        SelectionItem::Constraint(_)
        | SelectionItem::Dimension(_)
        | SelectionItem::Datum(_)
        | SelectionItem::Feature(_)
        | SelectionItem::FeatureCorner(_) => Err(FeatureAuthoringWarningKind::WrongOperandKind),
    }
}

fn pick_matches_snapshot(
    pick: &FeatureAuthoringPick,
    snapshot: &ComputedFeatureAuthoringSnapshot,
) -> bool {
    pick.sketch_input == snapshot.sketch_input()
        && pick.accepted == snapshot.accepted_state_identity()
}

fn target_matches_snapshot(
    target: &FeatureAuthoringTarget,
    snapshot: &ComputedFeatureAuthoringSnapshot,
) -> bool {
    match target {
        FeatureAuthoringTarget::Curve(pick) => pick_matches_snapshot(pick, snapshot),
        FeatureAuthoringTarget::Corner(picks) => picks
            .iter()
            .all(|pick| pick_matches_snapshot(pick, snapshot)),
    }
}

fn same_support(first: &FeatureAuthoringPick, second: &FeatureAuthoringPick) -> bool {
    first.curve.source == second.curve.source
}

fn feature_curve_pick(
    snapshot: &ComputedFeatureAuthoringSnapshot,
    document: &SketchDocument,
    span: CurveSpan,
    parameter: Option<f64>,
    retained_endpoint_hint: Option<DocumentFilletTrimEndpoint>,
) -> Result<FeatureAuthoringPick, FeatureAuthoringWarningKind> {
    if !document
        .curve_spans(span.curve)
        .is_ok_and(|spans| spans.contains(&span))
    {
        return Err(FeatureAuthoringWarningKind::MissingObject);
    }
    let parameter = match parameter {
        Some(parameter) if parameter.is_finite() => parameter,
        Some(_) => return Err(FeatureAuthoringWarningKind::NonFinitePick),
        None => {
            let intervals = document
                .visible_intervals(span)
                .map_err(|_| FeatureAuthoringWarningKind::MissingObject)?;
            let interval = intervals
                .first()
                .ok_or(FeatureAuthoringWarningKind::MissingObject)?;
            0.5 * (interval.start + interval.end)
        }
    };
    let jet = document
        .evaluate_curve_jet(span, parameter)
        .map_err(|_| FeatureAuthoringWarningKind::StalePick)?;
    Ok(FeatureAuthoringPick {
        curve: ComputedFilletCurvePick {
            source: NativeCurveSpanSource { span },
            parameter,
            model_position: [jet.position.x, jet.position.y],
            retained_endpoint_hint,
        },
        sketch_input: snapshot.sketch_input(),
        accepted: snapshot.accepted_state_identity(),
        span_endpoints: span_endpoint_ids(document, span),
    })
}

#[cfg(test)]
fn resolve_feature_corner_point(
    snapshot: &ComputedFeatureAuthoringSnapshot,
    document: &SketchDocument,
    point: DesignPointId,
) -> Result<Vec<FeatureAuthoringPick>, FeatureAuthoringWarningKind> {
    let points = std::collections::BTreeSet::from([point]);
    let incidences = feature_corner_incidence_index(document, &points)?;
    resolve_feature_corner_point_from_incidence(snapshot, document, point, &incidences)
}

fn feature_corner_incidence_index(
    document: &SketchDocument,
    selected_points: &std::collections::BTreeSet<DesignPointId>,
) -> Result<FeatureCornerIncidenceIndex, FeatureAuthoringWarningKind> {
    let mut incidences = FeatureCornerIncidenceIndex::new();
    let representatives = document.point_coincidence_representatives();
    let mut selected_by_representative =
        std::collections::BTreeMap::<DesignPointId, Vec<DesignPointId>>::new();
    for point in selected_points {
        if document.point(*point).is_none() {
            return Err(FeatureAuthoringWarningKind::MissingObject);
        }
        incidences.insert(*point, Vec::new());
        let representative = representatives
            .get(point)
            .copied()
            .ok_or(FeatureAuthoringWarningKind::MissingObject)?;
        selected_by_representative
            .entry(representative)
            .or_default()
            .push(*point);
    }
    if selected_points.is_empty() {
        return Ok(incidences);
    }
    for curve in document.curves() {
        match &curve.definition {
            CurveDefinition::Line { start, end, .. } => {
                let span = CurveSpan {
                    curve: curve.id,
                    segment: 0,
                };
                retain_equivalent_feature_corner_occurrence(
                    &mut incidences,
                    &selected_by_representative,
                    &representatives,
                    *start,
                    (span, 0.25, DocumentFilletTrimEndpoint::Start),
                );
                retain_equivalent_feature_corner_occurrence(
                    &mut incidences,
                    &selected_by_representative,
                    &representatives,
                    *end,
                    (span, 0.75, DocumentFilletTrimEndpoint::End),
                );
            }
            CurveDefinition::Polyline { points, closed, .. } => {
                let span_count = if *closed {
                    points.len()
                } else {
                    points.len().saturating_sub(1)
                };
                for index in 0..span_count {
                    let span = CurveSpan {
                        curve: curve.id,
                        segment: u32::try_from(index)
                            .map_err(|_| FeatureAuthoringWarningKind::MissingObject)?,
                    };
                    retain_equivalent_feature_corner_occurrence(
                        &mut incidences,
                        &selected_by_representative,
                        &representatives,
                        points[index],
                        (span, 0.25, DocumentFilletTrimEndpoint::Start),
                    );
                    retain_equivalent_feature_corner_occurrence(
                        &mut incidences,
                        &selected_by_representative,
                        &representatives,
                        points[(index + 1) % points.len()],
                        (span, 0.75, DocumentFilletTrimEndpoint::End),
                    );
                }
            }
            _ => {}
        }
    }
    Ok(incidences)
}

fn retain_equivalent_feature_corner_occurrence(
    incidences: &mut FeatureCornerIncidenceIndex,
    selected_by_representative: &std::collections::BTreeMap<DesignPointId, Vec<DesignPointId>>,
    representatives: &std::collections::BTreeMap<DesignPointId, DesignPointId>,
    endpoint: DesignPointId,
    occurrence: FeatureCornerOccurrence,
) {
    let Some(representative) = representatives.get(&endpoint) else {
        return;
    };
    let Some(selected) = selected_by_representative.get(representative) else {
        return;
    };
    for point in selected {
        if let Some(occurrences) = incidences.get_mut(point) {
            retain_feature_corner_occurrence(occurrences, occurrence);
        }
    }
}

fn feature_hit_incidence_index(
    document: &SketchDocument,
    hits: &[crate::Hit],
) -> Result<FeatureCornerIncidenceIndex, FeatureAuthoringWarningKind> {
    let points = hits
        .iter()
        .filter_map(|hit| match hit.item {
            SelectionItem::Point(point) => Some(point),
            SelectionItem::Curve(_)
            | SelectionItem::Constraint(_)
            | SelectionItem::Dimension(_)
            | SelectionItem::Datum(_)
            | SelectionItem::Feature(_)
            | SelectionItem::FeatureCorner(_) => None,
        })
        .collect::<std::collections::BTreeSet<_>>();
    feature_corner_incidence_index(document, &points)
}

fn retain_feature_corner_occurrence(
    occurrences: &mut Vec<FeatureCornerOccurrence>,
    occurrence: FeatureCornerOccurrence,
) {
    // Resolution distinguishes only zero, exactly two distinct supports, and
    // every ambiguous cardinality. Three retained entries therefore preserve
    // the complete decision while bounding a pathological high-valence point.
    if occurrences.len() < MAX_RETAINED_FEATURE_CORNER_OCCURRENCES {
        occurrences.push(occurrence);
    }
}

fn resolve_feature_corner_point_from_incidence(
    snapshot: &ComputedFeatureAuthoringSnapshot,
    document: &SketchDocument,
    point: DesignPointId,
    incidences: &FeatureCornerIncidenceIndex,
) -> Result<Vec<FeatureAuthoringPick>, FeatureAuthoringWarningKind> {
    let occurrences = incidences
        .get(&point)
        .ok_or(FeatureAuthoringWarningKind::MissingObject)?;
    match occurrences.as_slice() {
        [] | [_] => Err(FeatureAuthoringWarningKind::WrongOperandKind),
        [
            (first_span, first_parameter, first_endpoint),
            (second_span, second_parameter, second_endpoint),
        ] if first_span != second_span => Ok(vec![
            feature_curve_pick(
                snapshot,
                document,
                *first_span,
                Some(*first_parameter),
                Some(*first_endpoint),
            )?,
            feature_curve_pick(
                snapshot,
                document,
                *second_span,
                Some(*second_parameter),
                Some(*second_endpoint),
            )?,
        ]),
        _ => Err(FeatureAuthoringWarningKind::AmbiguousTrimSide),
    }
}

fn resolve_corners(
    snapshot: &ComputedFeatureAuthoringSnapshot,
    requests: Vec<([FeatureAuthoringPick; 2], FeatureAuthoringOptions)>,
    control: OperationControl,
) -> Result<Vec<FeatureAuthoringCornerDraft>, (FeatureAuthoringWarningKind, String)> {
    if requests.is_empty() {
        return Ok(Vec::new());
    }
    let radius = requests[0].1.fillet_radius.ok_or_else(|| {
        (
            FeatureAuthoringWarningKind::InvalidRadius,
            "Fillet radius is not initialized".into(),
        )
    })?;
    let mut prepared = Vec::with_capacity(requests.len());
    let mut core_requests = Vec::with_capacity(requests.len());
    let coincidence_representatives = snapshot
        .sketch_document()
        .point_coincidence_representatives();
    for (mut picks, options) in requests {
        if options.fillet_radius != Some(radius) {
            return Err((
                FeatureAuthoringWarningKind::InvalidRadius,
                "one grouped Fillet transition must use one shared radius".into(),
            ));
        }
        if picks[0].curve.source == picks[1].curve.source {
            return Err((
                FeatureAuthoringWarningKind::DuplicateSupport,
                "Fillet parents must be distinct native spans".into(),
            ));
        }
        if let Some((first, second)) =
            shared_endpoint_hints(&picks[0], &picks[1], &coincidence_representatives)
        {
            picks[0].curve.retained_endpoint_hint.get_or_insert(first);
            picks[1].curve.retained_endpoint_hint.get_or_insert(second);
        }
        core_requests.push(ComputedFilletCornerAuthoringRequest {
            first: picks[0].curve,
            second: picks[1].curve,
            options: options.branch_options(),
        });
        prepared.push((picks, options));
    }
    let outcome = snapshot
        .resolve_fillet_corners(
            &core_requests,
            radius,
            ComputedFeatureEvaluationPolicy::default(),
            control,
        )
        .map_err(|error| {
            let kind = map_authoring_error(&error);
            (kind, error.to_string())
        })?;
    let resolved = match outcome {
        OperationOutcome::Completed { value, .. } => value,
        stopped => {
            return Err((
                FeatureAuthoringWarningKind::WorkStopped,
                format!(
                    "Fillet batch resolution stopped: {:?}",
                    stopped.report().stopping_reason
                ),
            ));
        }
    };
    if resolved.len() != prepared.len() {
        return Err((
            FeatureAuthoringWarningKind::WorkStopped,
            "Fillet batch resolution returned incomplete output".into(),
        ));
    }
    prepared
        .into_iter()
        .zip(resolved)
        .map(|((picks, options), resolved)| {
            if resolved.sketch_input != picks[0].sketch_input
                || resolved.accepted != picks[0].accepted
                || picks[0].sketch_input != picks[1].sketch_input
                || picks[0].accepted != picks[1].accepted
            {
                return Err((
                    FeatureAuthoringWarningKind::StalePick,
                    "Fillet resolution did not match the exact accepted pick input".into(),
                ));
            }
            Ok(FeatureAuthoringCornerDraft {
                picks,
                preview: FeatureAuthoringCornerPreview {
                    corner: resolved.corner,
                    arc: resolved.arc,
                    options: options.branch_options(),
                },
            })
        })
        .collect()
}

fn shared_endpoint_hints(
    first: &FeatureAuthoringPick,
    second: &FeatureAuthoringPick,
    coincidence_representatives: &std::collections::BTreeMap<DesignPointId, DesignPointId>,
) -> Option<(DocumentFilletTrimEndpoint, DocumentFilletTrimEndpoint)> {
    let (first_start, first_end) = first.span_endpoints?;
    let (second_start, second_end) = second.span_endpoints?;
    let mut matches = Vec::new();
    let equivalent = |left: DesignPointId, right: DesignPointId| {
        coincidence_representatives.get(&left) == coincidence_representatives.get(&right)
    };
    if equivalent(first_start, second_start) {
        matches.push((
            DocumentFilletTrimEndpoint::Start,
            DocumentFilletTrimEndpoint::Start,
        ));
    }
    if equivalent(first_start, second_end) {
        matches.push((
            DocumentFilletTrimEndpoint::Start,
            DocumentFilletTrimEndpoint::End,
        ));
    }
    if equivalent(first_end, second_start) {
        matches.push((
            DocumentFilletTrimEndpoint::End,
            DocumentFilletTrimEndpoint::Start,
        ));
    }
    if equivalent(first_end, second_end) {
        matches.push((
            DocumentFilletTrimEndpoint::End,
            DocumentFilletTrimEndpoint::End,
        ));
    }
    let [value] = matches.as_slice() else {
        return None;
    };
    Some(*value)
}

fn span_endpoint_ids(
    document: &SketchDocument,
    span: CurveSpan,
) -> Option<(DesignPointId, DesignPointId)> {
    let curve = document.curve(span.curve)?;
    match &curve.definition {
        CurveDefinition::Line { start, end, .. } if span.segment == 0 => Some((*start, *end)),
        CurveDefinition::Polyline { points, closed, .. } => {
            let index = usize::try_from(span.segment).ok()?;
            let end = index.checked_add(1)?;
            if index >= points.len() || (!closed && end >= points.len()) {
                return None;
            }
            Some((points[index], points[end % points.len()]))
        }
        _ => None,
    }
}

const fn valid_radius(value: Option<f64>) -> bool {
    match value {
        None => false,
        Some(value) => value.is_finite() && value > 0.0,
    }
}

const fn map_authoring_error(error: &ComputedFeatureAuthoringError) -> FeatureAuthoringWarningKind {
    match error {
        ComputedFeatureAuthoringError::InvalidRadius => FeatureAuthoringWarningKind::InvalidRadius,
        ComputedFeatureAuthoringError::NonFinitePick => FeatureAuthoringWarningKind::NonFinitePick,
        ComputedFeatureAuthoringError::StalePick => FeatureAuthoringWarningKind::StalePick,
        ComputedFeatureAuthoringError::DuplicateSource
        | ComputedFeatureAuthoringError::UnsupportedSameCurvePair => {
            FeatureAuthoringWarningKind::DuplicateSupport
        }
        ComputedFeatureAuthoringError::UnsupportedCurvedPair
        | ComputedFeatureAuthoringError::UnsupportedSourceTopology => {
            FeatureAuthoringWarningKind::UnsupportedFilletPair
        }
        ComputedFeatureAuthoringError::SingularParents
        | ComputedFeatureAuthoringError::NoLocalRoot
        | ComputedFeatureAuthoringError::OffsetSingularity
        | ComputedFeatureAuthoringError::InvalidResolvedGeometry => {
            FeatureAuthoringWarningKind::SingularFillet
        }
        ComputedFeatureAuthoringError::AmbiguousLocalRoot => {
            FeatureAuthoringWarningKind::AmbiguousFilletRoot
        }
        ComputedFeatureAuthoringError::SideCorrectionUnavailable
        | ComputedFeatureAuthoringError::AmbiguousRetainedEndpoint => {
            FeatureAuthoringWarningKind::AmbiguousTrimSide
        }
        ComputedFeatureAuthoringError::UncertifiedCurvedBranch => {
            FeatureAuthoringWarningKind::UnsupportedCurveFamily
        }
        ComputedFeatureAuthoringError::Evaluation(_) => FeatureAuthoringWarningKind::WorkStopped,
        _ => FeatureAuthoringWarningKind::UnsupportedFilletPair,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use geosolve_sketch::{
        DocumentSolveRequest, RetainedSketchDocumentSession, SolverConfig, cancellation_pair,
    };

    struct AdjacentCornerFixture {
        session: RetainedSketchDocumentSession,
        points: [DesignPointId; 4],
        spans: [CurveSpan; 3],
    }

    fn adjacent_corner_fixture() -> AdjacentCornerFixture {
        let mut document = SketchDocument::new(10.0).expect("document");
        let points = [
            document.add_point("p0", [0.0, 0.0]).expect("p0"),
            document.add_point("p1", [4.0, 0.0]).expect("p1"),
            document.add_point("p2", [4.0, 4.0]).expect("p2"),
            document.add_point("p3", [8.0, 4.0]).expect("p3"),
        ];
        let curve = document
            .add_curve(
                "three-span polyline",
                CurveDefinition::Polyline {
                    points: points.to_vec(),
                    closed: false,
                    branch_directions: vec![[1.0, 0.0], [0.0, 1.0], [1.0, 0.0]],
                },
            )
            .expect("polyline");
        let session = RetainedSketchDocumentSession::new(
            document,
            DocumentSolveRequest::default(),
            SolverConfig::default(),
        )
        .expect("accepted session");
        AdjacentCornerFixture {
            session,
            points,
            spans: [0, 1, 2].map(|segment| CurveSpan { curve, segment }),
        }
    }

    fn candidate(outcome: FeatureAuthoringOutcome) -> FeatureAuthoringCandidate {
        match outcome {
            FeatureAuthoringOutcome::PreviewRequested { candidate, .. }
            | FeatureAuthoringOutcome::Apply(candidate) => candidate,
            other => panic!("expected complete feature candidate, got {other:?}"),
        }
    }

    fn completed_continuation(
        snapshot: &ComputedFeatureAuthoringSnapshot,
        prior: NewComputedFilletCorner,
        from_radius: f64,
        radius: f64,
    ) -> ContinuedComputedFilletCorner {
        match snapshot
            .continue_fillet_corner(
                prior,
                from_radius,
                radius,
                ComputedFeatureEvaluationPolicy::default(),
                computed_feature_authoring_control(),
            )
            .expect("continuation request")
        {
            OperationOutcome::Completed { value, .. } => value,
            stopped => panic!(
                "expected completed continuation, got {:?}",
                stopped.report().stopping_reason
            ),
        }
    }

    fn assert_absolute_branch_preserved(
        expected: NewComputedFilletCorner,
        actual: NewComputedFilletCorner,
    ) {
        assert_eq!(actual.first.source, expected.first.source);
        assert_eq!(actual.second.source, expected.second.source);
        assert_eq!(actual.first.normal_side, expected.first.normal_side);
        assert_eq!(actual.second.normal_side, expected.second.normal_side);
        assert_eq!(
            actual.first.retained_endpoint,
            expected.first.retained_endpoint
        );
        assert_eq!(
            actual.second.retained_endpoint,
            expected.second.retained_endpoint
        );
        assert_eq!(actual.endpoint_order, expected.endpoint_order);
        assert_eq!(actual.sweep, expected.sweep);
    }

    #[test]
    fn grouped_adjacent_authoring_is_canonical_and_keeps_corner_branches_on_radius_edit() {
        let fixture = adjacent_corner_fixture();
        let snapshot = ComputedFeatureAuthoringSnapshot::capture(&fixture.session)
            .expect("authoring snapshot");
        let document = fixture.session.design_document();
        let selection =
            [fixture.points[1], fixture.points[2]].map(|point| (SelectionItem::Point(point), None));
        let mut grouped = FeatureAuthoringState::default();
        let grouped_candidate = candidate(grouped.activate(
            &snapshot,
            document,
            FeatureAuthoringTool::Fillet,
            &selection,
        ));
        assert_eq!(grouped_candidate.corners().len(), 2);
        assert_eq!(grouped.completed_corner_count(), 2);
        assert_eq!(
            grouped.guidance().stage,
            FeatureAuthoringStage::PreviewReady
        );

        let original_branches = grouped_candidate
            .corners()
            .iter()
            .map(|corner| corner.options)
            .collect::<Vec<_>>();
        let resized = candidate(grouped.set_options(
            &snapshot,
            FeatureAuthoringOptions {
                fillet_radius: Some(0.75),
                flip_first_side: true,
                flip_second_side: true,
                alternate_arc: true,
            },
        ));
        assert_eq!(resized.radius().to_bits(), 0.75_f64.to_bits());
        assert_eq!(
            resized
                .corners()
                .iter()
                .map(|corner| corner.options)
                .collect::<Vec<_>>(),
            original_branches,
            "shared-radius edits must not overwrite explicit per-corner branches"
        );

        let forward_picks = resolve_feature_corner_point(&snapshot, document, fixture.points[1])
            .expect("corner picks");
        let mut reverse_picks = forward_picks.clone();
        reverse_picks.reverse();
        let mut forward = FeatureAuthoringState::default();
        let forward_candidate = candidate(forward.activate_picks(
            &snapshot,
            document,
            FeatureAuthoringTool::Fillet,
            forward_picks,
        ));
        let mut reverse = FeatureAuthoringState::default();
        let reverse_candidate = candidate(reverse.activate_picks(
            &snapshot,
            document,
            FeatureAuthoringTool::Fillet,
            reverse_picks,
        ));
        assert_eq!(
            forward_candidate.persistent_corners(),
            reverse_candidate.persistent_corners(),
            "parent order must canonicalize without changing endpoint semantics"
        );
        assert_eq!(
            fixture.spans[1],
            resized.corners()[1].corner.first.source.span
        );
    }

    #[test]
    fn absolute_radius_travel_preserves_completed_branches_and_batch_order() {
        let fixture = adjacent_corner_fixture();
        let snapshot = ComputedFeatureAuthoringSnapshot::capture(&fixture.session)
            .expect("authoring snapshot");
        let document = fixture.session.design_document();
        let selection =
            [fixture.points[1], fixture.points[2]].map(|point| (SelectionItem::Point(point), None));
        let mut state = FeatureAuthoringState::default();
        let initial = candidate(state.activate(
            &snapshot,
            document,
            FeatureAuthoringTool::Fillet,
            &selection,
        ));
        let initial_radius = initial.radius();
        let initial_corners = initial.persistent_corners();

        let relative_history = ComputedFilletAuthoringOptions {
            flip_first_side: true,
            flip_second_side: true,
            alternate_arc: true,
        };
        for draft in &mut state.corners {
            draft.preview.options = relative_history;
        }

        let forward = candidate(state.continue_radius_absolute(&snapshot, 0.75));
        assert_eq!(forward.radius().to_bits(), 0.75_f64.to_bits());
        assert_eq!(forward.corners().len(), initial_corners.len());
        for (index, (expected, actual)) in initial_corners
            .iter()
            .copied()
            .zip(forward.corners())
            .enumerate()
        {
            assert_absolute_branch_preserved(expected, actual.corner);
            assert_eq!(actual.options, relative_history);
            assert_eq!(
                actual.corner.first.source, initial_corners[index].first.source,
                "aggregate continuation must retain input order"
            );
        }

        let reversed = candidate(state.continue_radius_absolute(&snapshot, initial_radius));
        assert_eq!(reversed.radius().to_bits(), initial_radius.to_bits());
        for (expected, actual) in initial_corners.iter().copied().zip(reversed.corners()) {
            assert_absolute_branch_preserved(expected, actual.corner);
            assert_eq!(actual.options, relative_history);
        }
        assert!(matches!(
            state.current_outcome(),
            FeatureAuthoringOutcome::PreviewRequested {
                ref candidate,
                ref guidance,
            } if candidate.corners().len() == 2
                && guidance.completed_corners == 2
                && guidance.stage == FeatureAuthoringStage::PreviewReady
        ));
    }

    #[test]
    fn stopped_and_invalid_absolute_radius_continuation_are_state_neutral() {
        let fixture = adjacent_corner_fixture();
        let snapshot = ComputedFeatureAuthoringSnapshot::capture(&fixture.session)
            .expect("authoring snapshot");
        let document = fixture.session.design_document();
        let mut state = FeatureAuthoringState::default();
        candidate(state.activate(
            &snapshot,
            document,
            FeatureAuthoringTool::Fillet,
            &[(SelectionItem::Point(fixture.points[1]), None)],
        ));

        let before_invalid_radius = state.clone();
        assert!(matches!(
            state.continue_radius_absolute(&snapshot, f64::NAN),
            FeatureAuthoringOutcome::Warning(FeatureAuthoringWarning {
                kind: FeatureAuthoringWarningKind::InvalidRadius,
                ..
            })
        ));
        assert_eq!(state, before_invalid_radius);

        let mut invalid_branch = state.clone();
        invalid_branch.corners[0].preview.corner.second.source =
            invalid_branch.corners[0].preview.corner.first.source;
        let before_invalid_branch = invalid_branch.clone();
        assert!(matches!(
            invalid_branch.continue_radius_absolute(&snapshot, 0.75),
            FeatureAuthoringOutcome::Warning(_)
        ));
        assert_eq!(invalid_branch, before_invalid_branch);

        let mut stopped = state;
        stopped.corners = std::iter::repeat_n(
            stopped.corners[0].clone(),
            MAX_GROUPED_FILLET_CORNERS / 2 + 1,
        )
        .collect();
        let before_stopped = stopped.clone();
        assert!(matches!(
            stopped.continue_radius_absolute(&snapshot, 0.75),
            FeatureAuthoringOutcome::Warning(FeatureAuthoringWarning {
                kind: FeatureAuthoringWarningKind::WorkStopped,
                ..
            })
        ));
        assert_eq!(stopped, before_stopped);
    }

    #[test]
    fn absolute_corner_replacement_keeps_pending_semantic_pick_capability() {
        let fixture = adjacent_corner_fixture();
        let snapshot = ComputedFeatureAuthoringSnapshot::capture(&fixture.session)
            .expect("authoring snapshot");
        let document = fixture.session.design_document();
        let first_corner = resolve_feature_corner_point(&snapshot, document, fixture.points[1])
            .expect("first corner picks");
        let second_corner = resolve_feature_corner_point(&snapshot, document, fixture.points[2])
            .expect("second corner picks");
        let mut state = FeatureAuthoringState::default();
        candidate(state.activate_picks(
            &snapshot,
            document,
            FeatureAuthoringTool::Fillet,
            first_corner,
        ));
        let original_absolute = state.corners[0].preview.corner;
        let original_radius = state.options().fillet_radius.expect("active Fillet radius");
        let mut pending = second_corner.into_iter();
        assert!(matches!(
            state.pick_many(&snapshot, std::iter::once(pending.next().expect("first pick"))),
            FeatureAuthoringOutcome::Collecting { ref pending, .. } if pending.len() == 1
        ));
        let held_pending = state.pending.clone();
        let held_corner_picks = state.corners[0].picks.clone();

        let replacement =
            completed_continuation(&snapshot, original_absolute, original_radius, 0.75);
        assert!(matches!(
            state.replace_corner_absolute(&snapshot, 0, replacement),
            FeatureAuthoringOutcome::Collecting {
                ref pending,
                ref guidance,
            } if pending == &held_pending
                && guidance.completed_corners == 1
                && guidance.stage == FeatureAuthoringStage::PickSecondFilletCurve
        ));
        assert_eq!(state.options().fillet_radius, Some(0.75));
        assert_eq!(state.corners[0].picks, held_corner_picks);

        let completed = candidate(state.pick_many(
            &snapshot,
            std::iter::once(pending.next().expect("second pick")),
        ));
        assert_eq!(completed.radius().to_bits(), 0.75_f64.to_bits());
        assert_eq!(completed.corners().len(), 2);
        assert_absolute_branch_preserved(original_absolute, completed.corners()[0].corner);
    }

    #[test]
    fn repeated_curve_pairs_accumulate_one_pick_at_a_time_and_apply_or_enter_directly() {
        let fixture = adjacent_corner_fixture();
        let snapshot = ComputedFeatureAuthoringSnapshot::capture(&fixture.session)
            .expect("authoring snapshot");
        let document = fixture.session.design_document();
        let curve_picks = [
            (fixture.spans[0], 0.75),
            (fixture.spans[1], 0.25),
            (fixture.spans[1], 0.75),
            (fixture.spans[2], 0.25),
        ]
        .map(|(span, parameter)| {
            let mut picks = resolve_feature_item_picks(
                &snapshot,
                document,
                SelectionItem::Curve(span),
                Some(parameter),
            )
            .expect("native curve pick");
            assert_eq!(picks.len(), 1);
            picks.pop().expect("one native curve pick")
        });
        let [first_curve, second_curve, third_curve, fourth_curve] = curve_picks;
        let mut state = FeatureAuthoringState::default();
        assert!(matches!(
            state.activate_picks(
                &snapshot,
                document,
                FeatureAuthoringTool::Fillet,
                std::iter::empty(),
            ),
            FeatureAuthoringOutcome::ModeEntered(_)
        ));

        let first_pick = state.pick_many(&snapshot, std::iter::once(first_curve));
        assert!(matches!(
            first_pick,
            FeatureAuthoringOutcome::Collecting {
                ref pending,
                ref guidance,
            } if pending.len() == 1
                && guidance.stage == FeatureAuthoringStage::PickSecondFilletCurve
                && guidance.completed_corners == 0
        ));
        let first_pair = state.pick_many(&snapshot, std::iter::once(second_curve));
        assert!(matches!(
            first_pair,
            FeatureAuthoringOutcome::PreviewRequested {
                ref candidate,
                ref guidance,
            } if candidate.corners().len() == 1
                && guidance.stage == FeatureAuthoringStage::PreviewReady
                && guidance.completed_corners == 1
        ));

        let third_pick = state.pick_many(&snapshot, std::iter::once(third_curve));
        assert!(matches!(
            third_pick,
            FeatureAuthoringOutcome::Collecting {
                ref pending,
                ref guidance,
            } if pending.len() == 1
                && guidance.stage == FeatureAuthoringStage::PickSecondFilletCurve
                && guidance.completed_corners == 1
        ));
        let grouped = candidate(state.pick_many(&snapshot, std::iter::once(fourth_curve)));
        assert_eq!(grouped.corners().len(), 2);

        let applied = candidate(state.apply());
        let entered = candidate(state.enter());
        assert_eq!(applied, grouped);
        assert_eq!(entered, grouped);
        assert_eq!(state.completed_corner_count(), 2);
        assert_eq!(state.active_tool(), Some(FeatureAuthoringTool::Fillet));
    }

    #[test]
    fn radius_defaults_from_model_scale_then_remembers_the_last_valid_value() {
        let fixture = adjacent_corner_fixture();
        let snapshot = ComputedFeatureAuthoringSnapshot::capture(&fixture.session)
            .expect("authoring snapshot");
        let document = fixture.session.design_document();
        let selection = [(SelectionItem::Point(fixture.points[1]), None)];
        let mut state = FeatureAuthoringState::default();

        let initial = candidate(state.activate(
            &snapshot,
            document,
            FeatureAuthoringTool::Fillet,
            &selection,
        ));
        let expected_default = 0.1 * document.model_scale();
        assert_eq!(initial.radius().to_bits(), expected_default.to_bits());
        assert_eq!(
            state.options().fillet_radius.map(f64::to_bits),
            Some(expected_default.to_bits())
        );

        let remembered_radius = 0.625;
        let remembered_options = FeatureAuthoringOptions {
            fillet_radius: Some(remembered_radius),
            ..state.options()
        };
        let remembered = candidate(state.set_options(&snapshot, remembered_options));
        assert_eq!(remembered.radius().to_bits(), remembered_radius.to_bits());
        assert_eq!(candidate(state.enter()), remembered);
        assert_eq!(
            state.publication_succeeded(),
            FeatureAuthoringOutcome::ModeExited
        );
        assert!(matches!(
            state.activate_picks(
                &snapshot,
                document,
                FeatureAuthoringTool::Fillet,
                std::iter::empty(),
            ),
            FeatureAuthoringOutcome::ModeEntered(_)
        ));
        assert_eq!(
            state.options().fillet_radius.map(f64::to_bits),
            Some(remembered_radius.to_bits())
        );

        let mut invalid_memory = FeatureAuthoringState {
            options: FeatureAuthoringOptions {
                fillet_radius: Some(f64::NAN),
                ..FeatureAuthoringOptions::default()
            },
            ..FeatureAuthoringState::default()
        };
        assert!(matches!(
            invalid_memory.activate_picks(
                &snapshot,
                document,
                FeatureAuthoringTool::Fillet,
                std::iter::empty(),
            ),
            FeatureAuthoringOutcome::ModeEntered(_)
        ));
        assert_eq!(
            invalid_memory.options().fillet_radius.map(f64::to_bits),
            Some(expected_default.to_bits())
        );
    }

    #[test]
    fn stopped_corner_resolution_is_atomic_and_escape_is_two_stage() {
        let fixture = adjacent_corner_fixture();
        let snapshot = ComputedFeatureAuthoringSnapshot::capture(&fixture.session)
            .expect("authoring snapshot");
        let document = fixture.session.design_document();
        let picks = resolve_feature_corner_point(&snapshot, document, fixture.points[1])
            .expect("corner picks");
        let mut state = FeatureAuthoringState::default();
        assert!(matches!(
            state.activate_picks(
                &snapshot,
                document,
                FeatureAuthoringTool::Fillet,
                std::iter::empty(),
            ),
            FeatureAuthoringOutcome::ModeEntered(_)
        ));
        let (controller, token) = cancellation_pair();
        controller.cancel();
        let outcome = state.pick_many_controlled(
            &snapshot,
            picks,
            OperationControl::new(token, computed_feature_authoring_control().limits),
        );
        assert!(matches!(
            outcome,
            FeatureAuthoringOutcome::Warning(FeatureAuthoringWarning {
                kind: FeatureAuthoringWarningKind::WorkStopped,
                ..
            })
        ));
        assert_eq!(state.completed_corner_count(), 0);
        assert_eq!(
            state.guidance().stage,
            FeatureAuthoringStage::PickFirstFilletCurve
        );

        let grouped_picks = [fixture.points[1], fixture.points[2]]
            .into_iter()
            .flat_map(|point| {
                resolve_feature_corner_point(&snapshot, document, point).expect("grouped picks")
            })
            .collect::<Vec<_>>();
        let mut aggregate_control = computed_feature_authoring_control();
        aggregate_control.limits.document_validation_items = 3;
        assert!(matches!(
            state.pick_many_controlled(&snapshot, grouped_picks, aggregate_control),
            FeatureAuthoringOutcome::Warning(FeatureAuthoringWarning {
                kind: FeatureAuthoringWarningKind::WorkStopped,
                ..
            })
        ));
        assert_eq!(state.completed_corner_count(), 0);
        assert_eq!(
            state.guidance().stage,
            FeatureAuthoringStage::PickFirstFilletCurve
        );

        let ready = state.pick_many(
            &snapshot,
            resolve_feature_corner_point(&snapshot, document, fixture.points[1])
                .expect("fresh picks"),
        );
        assert!(matches!(
            ready,
            FeatureAuthoringOutcome::PreviewRequested { .. }
        ));
        assert!(matches!(
            state.cancel(),
            FeatureAuthoringOutcome::CandidateCleared(_)
        ));
        assert_eq!(state.active_tool(), Some(FeatureAuthoringTool::Fillet));
        assert_eq!(state.cancel(), FeatureAuthoringOutcome::ModeExited);
        assert_eq!(state.active_tool(), None);
    }

    #[test]
    fn direct_activation_bounds_an_unbounded_pick_iterator_without_partial_state() {
        let fixture = adjacent_corner_fixture();
        let snapshot = ComputedFeatureAuthoringSnapshot::capture(&fixture.session)
            .expect("authoring snapshot");
        let document = fixture.session.design_document();
        let pick = resolve_feature_corner_point(&snapshot, document, fixture.points[1])
            .expect("corner picks")
            .remove(0);
        let mut state = FeatureAuthoringState::default();

        let outcome = state.activate_picks(
            &snapshot,
            document,
            FeatureAuthoringTool::Fillet,
            std::iter::repeat(pick),
        );

        assert!(matches!(
            outcome,
            FeatureAuthoringOutcome::Warning(FeatureAuthoringWarning {
                kind: FeatureAuthoringWarningKind::WorkStopped,
                ..
            })
        ));
        assert_eq!(state.completed_corner_count(), 0);
        assert_eq!(
            state.guidance().stage,
            FeatureAuthoringStage::PickFirstFilletCurve
        );
    }

    fn retained_session(document: SketchDocument) -> RetainedSketchDocumentSession {
        RetainedSketchDocumentSession::new(
            document,
            DocumentSolveRequest::default(),
            SolverConfig::default(),
        )
        .expect("accepted session")
    }

    fn add_test_line(
        document: &mut SketchDocument,
        label: &str,
        start: DesignPointId,
        end: DesignPointId,
        branch_direction: [f64; 2],
    ) -> CurveSpan {
        let curve = document
            .add_curve(
                label,
                CurveDefinition::Line {
                    start,
                    end,
                    branch_direction,
                },
            )
            .expect("line");
        CurveSpan { curve, segment: 0 }
    }

    fn assert_resolved_pick(
        pick: &FeatureAuthoringPick,
        source: CurveSpan,
        parameter: f64,
        endpoint: DocumentFilletTrimEndpoint,
    ) {
        assert_eq!(pick.curve.source.span, source);
        assert_eq!(pick.curve.parameter.to_bits(), parameter.to_bits());
        assert_eq!(pick.curve.retained_endpoint_hint, Some(endpoint));
    }

    #[test]
    fn shared_endpoint_of_two_lines_resolves_exact_end_and_start_picks() {
        let mut document = SketchDocument::new(10.0).expect("document");
        let first = document.add_point("first", [0.0, 0.0]).expect("first");
        let corner = document.add_point("corner", [4.0, 0.0]).expect("corner");
        let last = document.add_point("last", [4.0, 4.0]).expect("last");
        let incoming = add_test_line(&mut document, "incoming", first, corner, [1.0, 0.0]);
        let outgoing = add_test_line(&mut document, "outgoing", corner, last, [0.0, 1.0]);
        let session = retained_session(document);
        let snapshot =
            ComputedFeatureAuthoringSnapshot::capture(&session).expect("authoring snapshot");

        let picks = resolve_feature_corner_point(&snapshot, session.design_document(), corner)
            .expect("picks");

        assert_eq!(picks.len(), 2);
        assert_resolved_pick(&picks[0], incoming, 0.75, DocumentFilletTrimEndpoint::End);
        assert_resolved_pick(&picks[1], outgoing, 0.25, DocumentFilletTrimEndpoint::Start);

        let mut state = FeatureAuthoringState::default();
        let outcome = state.activate(
            &snapshot,
            session.design_document(),
            FeatureAuthoringTool::Fillet,
            &[(SelectionItem::Point(corner), None)],
        );
        assert!(matches!(
            outcome,
            FeatureAuthoringOutcome::PreviewRequested {
                ref candidate,
                ref guidance,
            } if candidate.corners().len() == 1
                && guidance.completed_corners == 1
                && guidance.stage == FeatureAuthoringStage::PreviewReady
        ));
    }

    #[test]
    fn shared_endpoint_of_line_and_open_polyline_resolves() {
        let mut document = SketchDocument::new(10.0).expect("document");
        let first = document.add_point("first", [0.0, 0.0]).expect("first");
        let corner = document.add_point("corner", [4.0, 0.0]).expect("corner");
        let next = document.add_point("next", [4.0, 4.0]).expect("next");
        let last = document.add_point("last", [8.0, 4.0]).expect("last");
        let incoming = add_test_line(&mut document, "incoming", first, corner, [1.0, 0.0]);
        let polyline = document
            .add_curve(
                "outgoing polyline",
                CurveDefinition::Polyline {
                    points: vec![corner, next, last],
                    closed: false,
                    branch_directions: vec![[0.0, 1.0], [1.0, 0.0]],
                },
            )
            .expect("polyline");
        let session = retained_session(document);
        let snapshot =
            ComputedFeatureAuthoringSnapshot::capture(&session).expect("authoring snapshot");

        let picks = resolve_feature_corner_point(&snapshot, session.design_document(), corner)
            .expect("picks");

        assert_eq!(picks.len(), 2);
        assert_resolved_pick(&picks[0], incoming, 0.75, DocumentFilletTrimEndpoint::End);
        assert_resolved_pick(
            &picks[1],
            CurveSpan {
                curve: polyline,
                segment: 0,
            },
            0.25,
            DocumentFilletTrimEndpoint::Start,
        );
    }

    #[test]
    fn open_polyline_interior_vertex_continues_to_resolve() {
        let fixture = adjacent_corner_fixture();
        let snapshot = ComputedFeatureAuthoringSnapshot::capture(&fixture.session)
            .expect("authoring snapshot");

        let picks = resolve_feature_corner_point(
            &snapshot,
            fixture.session.design_document(),
            fixture.points[1],
        )
        .expect("picks");

        assert_eq!(picks.len(), 2);
        assert_resolved_pick(
            &picks[0],
            fixture.spans[0],
            0.75,
            DocumentFilletTrimEndpoint::End,
        );
        assert_resolved_pick(
            &picks[1],
            fixture.spans[1],
            0.25,
            DocumentFilletTrimEndpoint::Start,
        );
    }

    #[test]
    fn closed_polyline_vertex_zero_resolves_across_wrap_span() {
        let mut document = SketchDocument::new(10.0).expect("document");
        let points = [
            document.add_point("p0", [0.0, 0.0]).expect("p0"),
            document.add_point("p1", [4.0, 0.0]).expect("p1"),
            document.add_point("p2", [0.0, 4.0]).expect("p2"),
        ];
        let curve = document
            .add_curve(
                "closed polyline",
                CurveDefinition::Polyline {
                    points: points.to_vec(),
                    closed: true,
                    branch_directions: vec![
                        [1.0, 0.0],
                        [
                            -std::f64::consts::FRAC_1_SQRT_2,
                            std::f64::consts::FRAC_1_SQRT_2,
                        ],
                        [0.0, -1.0],
                    ],
                },
            )
            .expect("closed polyline");
        let session = retained_session(document);
        let snapshot =
            ComputedFeatureAuthoringSnapshot::capture(&session).expect("authoring snapshot");

        let picks = resolve_feature_corner_point(&snapshot, session.design_document(), points[0])
            .expect("picks");

        assert_eq!(picks.len(), 2);
        assert_resolved_pick(
            &picks[0],
            CurveSpan { curve, segment: 0 },
            0.25,
            DocumentFilletTrimEndpoint::Start,
        );
        assert_resolved_pick(
            &picks[1],
            CurveSpan { curve, segment: 2 },
            0.75,
            DocumentFilletTrimEndpoint::End,
        );
    }

    #[test]
    fn lone_line_endpoint_is_not_a_complete_fillet_corner() {
        let mut document = SketchDocument::new(10.0).expect("document");
        let start = document.add_point("start", [0.0, 0.0]).expect("start");
        let end = document.add_point("end", [4.0, 0.0]).expect("end");
        add_test_line(&mut document, "line", start, end, [1.0, 0.0]);
        let session = retained_session(document);
        let snapshot =
            ComputedFeatureAuthoringSnapshot::capture(&session).expect("authoring snapshot");

        assert_eq!(
            resolve_feature_corner_point(&snapshot, session.design_document(), start),
            Err(FeatureAuthoringWarningKind::WrongOperandKind)
        );
    }

    #[test]
    fn three_line_junction_is_an_ambiguous_fillet_corner() {
        let mut document = SketchDocument::new(10.0).expect("document");
        let corner = document.add_point("corner", [0.0, 0.0]).expect("corner");
        let right = document.add_point("right", [4.0, 0.0]).expect("right");
        let up = document.add_point("up", [0.0, 4.0]).expect("up");
        let left = document.add_point("left", [-4.0, 0.0]).expect("left");
        add_test_line(&mut document, "right", corner, right, [1.0, 0.0]);
        add_test_line(&mut document, "up", corner, up, [0.0, 1.0]);
        add_test_line(&mut document, "left", left, corner, [1.0, 0.0]);
        let session = retained_session(document);
        let snapshot =
            ComputedFeatureAuthoringSnapshot::capture(&session).expect("authoring snapshot");

        assert_eq!(
            resolve_feature_corner_point(&snapshot, session.design_document(), corner),
            Err(FeatureAuthoringWarningKind::AmbiguousTrimSide)
        );
    }

    #[test]
    fn isolated_point_is_not_a_fillet_corner_operand() {
        let mut document = SketchDocument::new(10.0).expect("document");
        let isolated = document
            .add_point("isolated", [0.0, 0.0])
            .expect("isolated");
        let session = retained_session(document);
        let snapshot =
            ComputedFeatureAuthoringSnapshot::capture(&session).expect("authoring snapshot");

        assert_eq!(
            resolve_feature_corner_point(&snapshot, session.design_document(), isolated),
            Err(FeatureAuthoringWarningKind::WrongOperandKind)
        );
    }
}
