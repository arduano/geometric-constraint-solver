// SPDX-License-Identifier: GPL-3.0-or-later

//! Grouped, presentation-independent authoring for computed sketch features.

use geosolve_sketch::{
    CurveDefinition, CurveSpan, DesignPointId, DocumentFilletTrimEndpoint, OperationControl,
    OperationOutcome, PreparedSketchInput, SketchAcceptedStateIdentity, SketchDocument,
};
use geosolve_sketch_features::{
    ComputedCircularArc, ComputedFeatureAuthoringError, ComputedFeatureAuthoringSnapshot,
    ComputedFeatureEvaluationPolicy, ComputedFilletAuthoringOptions,
    ComputedFilletCornerAuthoringRequest, ComputedFilletCurvePick, NativeCurveSpanSource,
    NewComputedFilletCorner,
};

use crate::SelectionItem;
use crate::coordinator::computed_feature_authoring_control;

const MAX_GROUPED_FILLET_CORNERS: usize = 16_384;

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

/// Process-local shared-radius and next-corner branch choices.
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

    /// Activates grouped Fillet authoring and consumes every complete preselected pair.
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
        let mut picks = Vec::new();
        for (item, parameter) in selection {
            let resolved = match resolve_feature_item_picks(snapshot, document, *item, *parameter) {
                Ok(resolved) => resolved,
                Err(kind) => {
                    return self.warning(kind, "the non-empty selection is not a Fillet corner");
                }
            };
            picks.extend(resolved);
        }
        self.pick_many(snapshot, picks)
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

    /// Controlled counterpart used to qualify cancellation and exhaustion.
    #[must_use]
    pub fn pick_many_controlled(
        &mut self,
        snapshot: &ComputedFeatureAuthoringSnapshot,
        picks: impl IntoIterator<Item = FeatureAuthoringPick>,
        control: OperationControl,
    ) -> FeatureAuthoringOutcome {
        if self.active.is_none() {
            return FeatureAuthoringOutcome::Inactive;
        }
        let mut next = self.clone();
        let mut requests = Vec::new();
        for pick in picks {
            if pick.sketch_input != snapshot.sketch_input()
                || pick.accepted != snapshot.accepted_state_identity()
            {
                return self.warning(
                    FeatureAuthoringWarningKind::StalePick,
                    "the pick belongs to an older accepted sketch input",
                );
            }
            next.pending.push(pick);
            if next.pending.len() == 2 {
                if next.corners.len() + requests.len() >= MAX_GROUPED_FILLET_CORNERS {
                    return self.warning(
                        FeatureAuthoringWarningKind::WorkStopped,
                        "grouped Fillet corner limit was exhausted",
                    );
                }
                let pair = [next.pending.remove(0), next.pending.remove(0)];
                requests.push((pair, next.options));
            }
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
        options: FeatureAuthoringOptions,
        selected_corner: Option<(usize, ComputedFilletAuthoringOptions)>,
    ) -> FeatureAuthoringOutcome {
        if self.active.is_none() {
            self.options = options;
            return FeatureAuthoringOutcome::Inactive;
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
        if !valid_radius(self.options.fillet_radius) || self.options.fillet_radius.is_none() {
            self.options.fillet_radius = Some(0.1 * document.model_scale());
        }
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
    match item {
        SelectionItem::Curve(span) => Ok(vec![feature_curve_pick(
            snapshot, document, span, parameter, None,
        )?]),
        SelectionItem::Point(point) => resolve_feature_corner_point(snapshot, document, point),
        SelectionItem::Constraint(_)
        | SelectionItem::Dimension(_)
        | SelectionItem::Feature(_)
        | SelectionItem::FeatureCorner(_) => Err(FeatureAuthoringWarningKind::WrongOperandKind),
    }
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

fn resolve_feature_corner_point(
    snapshot: &ComputedFeatureAuthoringSnapshot,
    document: &SketchDocument,
    point: DesignPointId,
) -> Result<Vec<FeatureAuthoringPick>, FeatureAuthoringWarningKind> {
    if document.point(point).is_none() {
        return Err(FeatureAuthoringWarningKind::MissingObject);
    }
    let mut endpoint_seen = false;
    let mut candidates = Vec::new();
    for curve in document.curves() {
        let CurveDefinition::Polyline { points, closed, .. } = &curve.definition else {
            continue;
        };
        if *closed {
            endpoint_seen |= points.contains(&point);
            continue;
        }
        for (index, candidate) in points.iter().copied().enumerate() {
            if candidate != point {
                continue;
            }
            if index == 0 || index + 1 == points.len() {
                endpoint_seen = true;
                continue;
            }
            let incoming = CurveSpan {
                curve: curve.id,
                segment: u32::try_from(index - 1)
                    .map_err(|_| FeatureAuthoringWarningKind::MissingObject)?,
            };
            let outgoing = CurveSpan {
                curve: curve.id,
                segment: u32::try_from(index)
                    .map_err(|_| FeatureAuthoringWarningKind::MissingObject)?,
            };
            candidates.push(vec![
                feature_curve_pick(
                    snapshot,
                    document,
                    incoming,
                    Some(0.75),
                    Some(DocumentFilletTrimEndpoint::End),
                )?,
                feature_curve_pick(
                    snapshot,
                    document,
                    outgoing,
                    Some(0.25),
                    Some(DocumentFilletTrimEndpoint::Start),
                )?,
            ]);
        }
    }
    match candidates.len() {
        1 => Ok(candidates.pop().expect("single corner candidate")),
        0 if endpoint_seen => Err(FeatureAuthoringWarningKind::AmbiguousTrimSide),
        0 => Err(FeatureAuthoringWarningKind::WrongOperandKind),
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
        if let Some((first, second)) = shared_endpoint_hints(&picks[0], &picks[1]) {
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
) -> Option<(DocumentFilletTrimEndpoint, DocumentFilletTrimEndpoint)> {
    let (first_start, first_end) = first.span_endpoints?;
    let (second_start, second_end) = second.span_endpoints?;
    let mut matches = Vec::new();
    if first_start == second_start {
        matches.push((
            DocumentFilletTrimEndpoint::Start,
            DocumentFilletTrimEndpoint::Start,
        ));
    }
    if first_start == second_end {
        matches.push((
            DocumentFilletTrimEndpoint::Start,
            DocumentFilletTrimEndpoint::End,
        ));
    }
    if first_end == second_start {
        matches.push((
            DocumentFilletTrimEndpoint::End,
            DocumentFilletTrimEndpoint::Start,
        ));
    }
    if first_end == second_end {
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
        None => true,
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
}
