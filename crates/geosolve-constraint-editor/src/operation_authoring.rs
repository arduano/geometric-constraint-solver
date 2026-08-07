// SPDX-License-Identifier: GPL-3.0-or-later

//! Presentation-independent authoring policy for equation-free sketch operations.

use std::sync::Arc;

use geosolve_sketch::{
    ContactDomain, ContactNeighborhood, CurveCurveFilletRequest, CurveDefinition,
    CurveFilletParentRequest, CurveSpan, DesignPointId, DocumentArcSweep, DocumentCurveNormalSide,
    DocumentDimensionMode, DocumentFilletEndpointOrder, DocumentFilletTrimEndpoint,
    DocumentTrimParameter, PreparedSketchInput, SketchDocument,
};
use geosolve_sketch_ops::SketchOperationRequest;

use crate::SelectionItem;

const LOCAL_FILLET_ITERATIONS: usize = 16;
const LOCAL_FILLET_WINDOW_FRACTION: f64 = 0.35;

/// Closed operation palette owned by the reusable Fillet authoring state machine.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OperationAuthoringTool {
    Fillet,
}

/// One finite model-space geometry pick.
///
/// The parameter and position must describe the same accepted curve sample. Keeping
/// both values makes local root selection reproducible without asking a renderer to
/// reconstruct a curve point.
#[derive(Clone, Debug, PartialEq)]
pub struct OperationAuthoringPick {
    pub item: SelectionItem,
    pub curve_parameter: f64,
    pub model_position: [f64; 2],
    source_input: Option<Arc<PreparedSketchInput>>,
    fillet_trim_endpoint_hint: Option<DocumentFilletTrimEndpoint>,
}

impl OperationAuthoringPick {
    /// Resolves an ordinary selection/tree item into an exact accepted curve pick.
    /// An explicit canvas parameter is retained; otherwise the midpoint of the
    /// first visible interval is selected deterministically.
    ///
    /// # Errors
    ///
    /// Returns a typed warning when the item is not a current curve span or its
    /// requested/default sample cannot be evaluated from accepted geometry.
    pub fn for_item(
        document: &SketchDocument,
        item: SelectionItem,
        curve_parameter: Option<f64>,
    ) -> Result<Self, OperationAuthoringWarningKind> {
        let SelectionItem::Curve(span) = item else {
            return Err(OperationAuthoringWarningKind::WrongOperandKind);
        };
        let parameter = if let Some(parameter) = curve_parameter {
            parameter
        } else {
            let intervals = document
                .visible_intervals(span)
                .map_err(|_| OperationAuthoringWarningKind::MissingObject)?;
            let interval = intervals
                .first()
                .ok_or(OperationAuthoringWarningKind::MissingObject)?;
            0.5 * (interval.start + interval.end)
        };
        Self::at_curve_parameter(document, span, parameter)
    }

    /// Resolves both the exact parameter and model position through accepted
    /// public curve evaluation. This is the ordinary tree/preselection path.
    ///
    /// # Errors
    ///
    /// Returns a typed warning when the parameter is non-finite or the accepted
    /// curve span cannot be evaluated at that parameter.
    pub fn at_curve_parameter(
        document: &SketchDocument,
        span: CurveSpan,
        curve_parameter: f64,
    ) -> Result<Self, OperationAuthoringWarningKind> {
        if !curve_parameter.is_finite() {
            return Err(OperationAuthoringWarningKind::NonFinitePick);
        }
        let jet = document
            .evaluate_curve_jet(span, curve_parameter)
            .map_err(|_| OperationAuthoringWarningKind::MissingObject)?;
        Self::curve(
            document,
            span,
            curve_parameter,
            [jet.position.x, jet.position.y],
        )
    }

    /// Constructs and validates one accepted curve pick.
    ///
    /// # Errors
    ///
    /// Returns a typed warning when the item is not a current curve span, the
    /// parameter/position is non-finite, or the two do not identify the same
    /// accepted sample.
    pub fn curve(
        document: &SketchDocument,
        span: CurveSpan,
        curve_parameter: f64,
        model_position: [f64; 2],
    ) -> Result<Self, OperationAuthoringWarningKind> {
        let pick = Self {
            item: SelectionItem::Curve(span),
            curve_parameter,
            model_position,
            source_input: None,
            fillet_trim_endpoint_hint: None,
        };
        validate_pick(document, &pick)?;
        Ok(pick)
    }

    #[must_use]
    pub const fn curve_span(&self) -> Option<CurveSpan> {
        match self.item {
            SelectionItem::Curve(span) => Some(span),
            SelectionItem::Point(_)
            | SelectionItem::Constraint(_)
            | SelectionItem::Dimension(_)
            | SelectionItem::Feature(_)
            | SelectionItem::FeatureCorner(_) => None,
        }
    }

    pub(crate) fn bind_input(mut self, input: &PreparedSketchInput) -> Self {
        self.source_input = Some(Arc::new(*input));
        self
    }

    fn with_fillet_trim_endpoint_hint(mut self, endpoint: DocumentFilletTrimEndpoint) -> Self {
        self.fillet_trim_endpoint_hint = Some(endpoint);
        self
    }

    pub(crate) fn source_input(&self) -> Option<&PreparedSketchInput> {
        self.source_input.as_deref()
    }

    pub(crate) fn validate(
        &self,
        document: &SketchDocument,
    ) -> Result<(), OperationAuthoringWarningKind> {
        validate_pick(document, self)
    }
}

/// Explicit process-local authoring choices. A `None` radius acquires the
/// documented `0.1 * model_scale` default on first tool activation and is then
/// remembered by this state value.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OperationAuthoringOptions {
    pub fillet_radius: Option<f64>,
    pub fillet_radius_mode: DocumentDimensionMode,
    pub fillet_flip_first_side: bool,
    pub fillet_flip_second_side: bool,
    pub fillet_alternate_arc: bool,
}

impl Default for OperationAuthoringOptions {
    fn default() -> Self {
        Self {
            fillet_radius: None,
            fillet_radius_mode: DocumentDimensionMode::Reference,
            fillet_flip_first_side: false,
            fillet_flip_second_side: false,
            fillet_alternate_arc: false,
        }
    }
}

/// Stable authoring progression. Presentation may label these stages but must not
/// derive a different operand or branch policy from them.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OperationAuthoringStage {
    PickFirstFilletCurve,
    PickSecondFilletCurve,
    PlaceFilletRadius,
    PreviewReady,
}

/// Presentation-neutral expected input at the current stage.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OperationAuthoringOperandKind {
    RegularCurveSpan,
    DistinctRegularCurveSpan,
    FilletCornerPoint,
    FilletRadius,
}

/// Current headless guidance for one active tool.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OperationAuthoringGuidance {
    pub tool: OperationAuthoringTool,
    pub stage: OperationAuthoringStage,
    pub expected: Vec<OperationAuthoringOperandKind>,
    pub message: &'static str,
}

/// Typed non-mutating authoring or preparation warning.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum OperationAuthoringWarningKind {
    MissingObject,
    WrongOperandKind,
    WrongArity,
    NonFinitePick,
    StalePick,
    DuplicateSupport,
    AmbiguousFilletCorner,
    FilletCornerNotInterior,
    AlreadyTrimmed,
    UnsupportedCurveFamily,
    UnsupportedFilletPair,
    SingularFillet,
    AmbiguousFilletRoot,
    AmbiguousTrimSide,
    UnresolvedLocalFilletRoot,
    NoPreview,
    OperationUnsupported(geosolve_sketch_ops::SketchOperationUnsupportedReason),
    OperationIncomplete(geosolve_sketch_ops::SketchOperationIncompleteReason),
    PreviewRejected,
    WorkStopped,
}

/// One typed warning with stable operation/stage context.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OperationAuthoringWarning {
    pub tool: OperationAuthoringTool,
    pub stage: OperationAuthoringStage,
    pub kind: OperationAuthoringWarningKind,
    pub message: String,
}

/// Complete immutable operation request synthesized by the headless state machine.
#[derive(Clone, Debug, PartialEq)]
pub struct OperationAuthoringCandidate {
    tool: OperationAuthoringTool,
    picks: Vec<OperationAuthoringPick>,
    request: SketchOperationRequest,
    confirmed: bool,
    source_input: Option<Arc<PreparedSketchInput>>,
}

impl OperationAuthoringCandidate {
    pub(crate) fn explicit_replay(
        tool: OperationAuthoringTool,
        request: SketchOperationRequest,
        source_input: &PreparedSketchInput,
    ) -> Self {
        Self {
            tool,
            picks: Vec::new(),
            request,
            confirmed: true,
            source_input: Some(Arc::new(*source_input)),
        }
    }

    #[must_use]
    pub const fn tool(&self) -> OperationAuthoringTool {
        self.tool
    }

    #[must_use]
    pub fn picks(&self) -> &[OperationAuthoringPick] {
        &self.picks
    }

    #[must_use]
    pub const fn request(&self) -> &SketchOperationRequest {
        &self.request
    }

    /// Whether all semantic pointer stages, including Fillet radius confirmation,
    /// are complete. This does not claim solver acceptance; only a
    /// coordinator-held [`crate::OperationAuthoringPreview`] is apply-ready.
    #[must_use]
    pub const fn is_confirmed(&self) -> bool {
        self.confirmed
    }

    pub(crate) fn source_input(&self) -> Option<&PreparedSketchInput> {
        self.source_input.as_deref()
    }
}

/// Result of one operation-authoring transition.
#[derive(Clone, Debug, PartialEq)]
pub enum OperationAuthoringOutcome {
    ModeEntered(OperationAuthoringGuidance),
    Collecting {
        picks: Vec<OperationAuthoringPick>,
        guidance: OperationAuthoringGuidance,
    },
    PreviewRequested {
        candidate: OperationAuthoringCandidate,
        guidance: OperationAuthoringGuidance,
    },
    Apply(OperationAuthoringCandidate),
    Warning(OperationAuthoringWarning),
    CandidateCleared(OperationAuthoringGuidance),
    ModeExited,
    Inactive,
}

/// Separate Fillet-operation collector. It deliberately does not overload the
/// M62 constraint [`crate::AuthoringState`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct OperationAuthoringState {
    active: Option<OperationAuthoringTool>,
    picks: Vec<OperationAuthoringPick>,
    candidate: Option<OperationAuthoringCandidate>,
    candidate_confirmed: bool,
    options: OperationAuthoringOptions,
}

impl OperationAuthoringState {
    #[must_use]
    pub const fn active_tool(&self) -> Option<OperationAuthoringTool> {
        self.active
    }

    #[must_use]
    pub fn picks(&self) -> &[OperationAuthoringPick] {
        &self.picks
    }

    #[must_use]
    pub const fn options(&self) -> OperationAuthoringOptions {
        self.options
    }

    #[must_use]
    pub fn candidate(&self) -> Option<&OperationAuthoringCandidate> {
        self.candidate.as_ref()
    }

    #[must_use]
    pub const fn candidate_confirmed(&self) -> bool {
        self.candidate_confirmed
    }

    /// Leaves operation authoring immediately while preserving process-local
    /// option memory for the next activation.
    pub fn deactivate(&mut self) {
        self.active = None;
        self.transaction_finished();
    }

    /// Updates remembered explicit options and refreshes an existing candidate.
    #[must_use]
    pub fn set_options(
        &mut self,
        document: &SketchDocument,
        options: OperationAuthoringOptions,
    ) -> OperationAuthoringOutcome {
        if !valid_optional_positive(options.fillet_radius) {
            self.candidate = None;
            self.candidate_confirmed = false;
            return self.warning(
                OperationAuthoringWarningKind::NonFinitePick,
                "fillet radius must be finite and positive",
            );
        }
        self.options = options;
        self.ensure_defaults(document);
        let Some(OperationAuthoringTool::Fillet) = self.active else {
            return OperationAuthoringOutcome::Inactive;
        };
        if self.picks.len() == 2 {
            self.stage_complete_candidate(document, self.candidate_confirmed)
        } else {
            OperationAuthoringOutcome::Collecting {
                picks: self.picks.clone(),
                guidance: self.guidance(),
            }
        }
    }

    /// Activates a tool from an immutable accepted-selection snapshot. Empty
    /// selection enters operand-collection mode; compatible preselection is fed
    /// through the same pick transitions and seeds a preview.
    #[must_use]
    pub fn activate(
        &mut self,
        document: &SketchDocument,
        tool: OperationAuthoringTool,
        selection: &[OperationAuthoringPick],
    ) -> OperationAuthoringOutcome {
        self.begin_activation(document, tool);
        if selection.len() > 2 {
            return self.warning(
                OperationAuthoringWarningKind::WrongArity,
                "the preselection contains too many operands for this operation",
            );
        }
        if selection.is_empty() {
            return OperationAuthoringOutcome::ModeEntered(self.guidance());
        }
        let mut outcome = OperationAuthoringOutcome::ModeEntered(self.guidance());
        for pick in selection {
            outcome = self.pick(document, pick.clone());
            if matches!(outcome, OperationAuthoringOutcome::Warning(_)) {
                break;
            }
        }
        outcome
    }

    /// Activates directly from ordinary selection identities and optional canvas
    /// parameters. This preserves an incompatible non-empty selection as a typed
    /// warning instead of accidentally treating it as empty collection mode.
    #[must_use]
    pub fn activate_items(
        &mut self,
        document: &SketchDocument,
        tool: OperationAuthoringTool,
        selection: &[(SelectionItem, Option<f64>)],
    ) -> OperationAuthoringOutcome {
        self.begin_activation(document, tool);
        if selection.is_empty() {
            return OperationAuthoringOutcome::ModeEntered(self.guidance());
        }
        let picks = selection
            .iter()
            .try_fold(Vec::new(), |mut picks, (item, parameter)| {
                picks.extend(resolve_operation_item_picks(
                    document, tool, *item, *parameter,
                )?);
                Ok::<_, OperationAuthoringWarningKind>(picks)
            });
        match picks {
            Ok(picks) if picks.len() <= 2 => self.activate(document, tool, &picks),
            Ok(_) => self.warning(
                OperationAuthoringWarningKind::WrongArity,
                "the preselection contains too many operands for this operation",
            ),
            Err(kind) => self.warning(
                kind,
                "the current non-empty selection is incompatible with this operation",
            ),
        }
    }

    /// Resolves and adds one ordinary tree/canvas item through headless accepted
    /// geometry. A tree item may omit its parameter.
    #[must_use]
    pub fn pick_item(
        &mut self,
        document: &SketchDocument,
        item: SelectionItem,
        curve_parameter: Option<f64>,
    ) -> OperationAuthoringOutcome {
        let Some(tool) = self.active else {
            return OperationAuthoringOutcome::Inactive;
        };
        match resolve_operation_item_picks(document, tool, item, curve_parameter) {
            Ok(picks) => self.pick_many(document, picks),
            Err(kind) => self.warning(kind, "the item is not a compatible accepted curve pick"),
        }
    }

    /// Applies one already resolved item event. A Fillet corner shortcut may
    /// contribute its two explicitly ordered adjacent spans atomically; every
    /// other item contributes exactly one pick.
    #[must_use]
    pub fn pick_many(
        &mut self,
        document: &SketchDocument,
        picks: impl IntoIterator<Item = OperationAuthoringPick>,
    ) -> OperationAuthoringOutcome {
        let picks = picks.into_iter().collect::<Vec<_>>();
        if picks.is_empty() {
            return self.warning(
                OperationAuthoringWarningKind::WrongOperandKind,
                "the current operation stage requires a compatible geometry pick",
            );
        }
        if self.active == Some(OperationAuthoringTool::Fillet)
            && picks.len() > 1
            && (!self.picks.is_empty() || picks.len() != 2)
        {
            return self.warning(
                OperationAuthoringWarningKind::WrongArity,
                "one fillet corner item supplies both parents and requires an empty collector",
            );
        }
        let mut outcome = OperationAuthoringOutcome::Collecting {
            picks: self.picks.clone(),
            guidance: self.guidance(),
        };
        for pick in picks {
            outcome = self.pick(document, pick);
            if matches!(outcome, OperationAuthoringOutcome::Warning(_)) {
                break;
            }
        }
        outcome
    }

    /// One universal pointer-down transition. Fillet radius confirmation is owned
    /// here, so a thin host never branches on pending stages.
    #[must_use]
    pub fn pointer_down(
        &mut self,
        document: &SketchDocument,
        pick: Option<OperationAuthoringPick>,
        model_position: [f64; 2],
    ) -> OperationAuthoringOutcome {
        let picks = pick.into_iter().collect::<Vec<_>>();
        self.pointer_down_picks(document, &picks, model_position)
    }

    /// One universal pointer-down transition that may carry the two expanded
    /// spans of one unambiguous Fillet corner point. Radius confirmation remains
    /// headless stage policy.
    #[must_use]
    pub fn pointer_down_picks(
        &mut self,
        document: &SketchDocument,
        picks: &[OperationAuthoringPick],
        model_position: [f64; 2],
    ) -> OperationAuthoringOutcome {
        if !model_position.into_iter().all(f64::is_finite) {
            return self.warning(
                OperationAuthoringWarningKind::NonFinitePick,
                "pointer position must be finite",
            );
        }
        if self.candidate_confirmed {
            return self.confirm(document, model_position);
        }
        if self.active == Some(OperationAuthoringTool::Fillet) && self.picks.len() == 2 {
            return self.confirm(document, model_position);
        }
        if picks.is_empty() {
            self.warning(
                OperationAuthoringWarningKind::WrongOperandKind,
                "the current operation stage requires a curve pick",
            )
        } else {
            self.pick_many(document, picks.iter().cloned())
        }
    }

    /// Adds one exact accepted-geometry pick.
    #[must_use]
    pub fn pick(
        &mut self,
        document: &SketchDocument,
        pick: OperationAuthoringPick,
    ) -> OperationAuthoringOutcome {
        let Some(tool) = self.active else {
            return OperationAuthoringOutcome::Inactive;
        };
        if let Err(kind) = validate_pick(document, &pick) {
            return self.warning(kind, "the pick does not identify current accepted geometry");
        }
        let Some(span) = pick.curve_span() else {
            return self.warning(
                OperationAuthoringWarningKind::WrongOperandKind,
                "helper operations accept curve-span operands",
            );
        };
        let OperationAuthoringTool::Fillet = tool;
        self.pick_fillet(document, pick, span)
    }

    fn pick_fillet(
        &mut self,
        document: &SketchDocument,
        mut pick: OperationAuthoringPick,
        span: CurveSpan,
    ) -> OperationAuthoringOutcome {
        if self.picks.len() >= 2 {
            return self.warning(
                OperationAuthoringWarningKind::WrongArity,
                "fillet already has two parent picks",
            );
        }
        if let Some(first) = self
            .picks
            .first()
            .and_then(OperationAuthoringPick::curve_span)
        {
            if first == span {
                return self.warning(
                    OperationAuthoringWarningKind::DuplicateSupport,
                    "fillet parents must be distinct support spans",
                );
            }
            if first.curve == span.curve
                && !same_open_polyline_adjacent_spans(document, first, span)
            {
                return self.warning(
                    OperationAuthoringWarningKind::DuplicateSupport,
                    "same-curve fillet spans must be adjacent at one shared vertex",
                );
            }
            if let Some((first_endpoint, second_endpoint)) =
                line_span_shared_endpoint_hints(document, first, span)
            {
                if let Some(first_pick) = self.picks.first_mut() {
                    first_pick
                        .fillet_trim_endpoint_hint
                        .get_or_insert(first_endpoint);
                }
                pick.fillet_trim_endpoint_hint
                    .get_or_insert(second_endpoint);
            }
        }
        self.picks.push(pick);
        if self.picks.len() == 2 {
            self.stage_complete_candidate(document, false)
        } else {
            OperationAuthoringOutcome::Collecting {
                picks: self.picks.clone(),
                guidance: self.guidance(),
            }
        }
    }

    /// Updates the non-committable Fillet radius preview from a finite pointer
    /// location. Pan/zoom remain presentation concerns and do not mutate this state.
    #[must_use]
    pub fn hover(
        &mut self,
        document: &SketchDocument,
        model_position: [f64; 2],
    ) -> OperationAuthoringOutcome {
        if self.candidate_confirmed {
            let Some(candidate) = self.candidate.clone() else {
                self.transaction_finished();
                return self.warning(
                    OperationAuthoringWarningKind::NoPreview,
                    "the confirmed operation no longer owns an exact candidate",
                );
            };
            return OperationAuthoringOutcome::PreviewRequested {
                candidate,
                guidance: self.guidance(),
            };
        }
        if self.active == Some(OperationAuthoringTool::Fillet) && self.picks.len() == 2 {
            return self.stage_fillet_radius_candidate(document, model_position, false);
        }
        OperationAuthoringOutcome::Collecting {
            picks: self.picks.clone(),
            guidance: self.guidance(),
        }
    }

    /// Confirms the currently placed Fillet radius and makes its preview committable.
    #[must_use]
    pub fn confirm(
        &mut self,
        document: &SketchDocument,
        model_position: [f64; 2],
    ) -> OperationAuthoringOutcome {
        if self.active == Some(OperationAuthoringTool::Fillet) && self.picks.len() == 2 {
            return self.stage_fillet_radius_candidate(document, model_position, true);
        }
        match self.candidate.clone() {
            Some(candidate) => OperationAuthoringOutcome::PreviewRequested {
                candidate,
                guidance: self.guidance(),
            },
            None => self.warning(
                OperationAuthoringWarningKind::NoPreview,
                "there is no complete operation preview",
            ),
        }
    }

    /// Requests commit of the exact currently staged preview.
    #[must_use]
    pub fn apply(&self) -> OperationAuthoringOutcome {
        let Some(OperationAuthoringTool::Fillet) = self.active else {
            return OperationAuthoringOutcome::Inactive;
        };
        match (&self.candidate, self.candidate_confirmed) {
            (Some(candidate), true) => OperationAuthoringOutcome::Apply(candidate.clone()),
            _ => OperationAuthoringOutcome::Warning(OperationAuthoringWarning {
                tool: OperationAuthoringTool::Fillet,
                stage: self.guidance().stage,
                kind: OperationAuthoringWarningKind::NoPreview,
                message: "complete the operation picks before applying".into(),
            }),
        }
    }

    /// Enter has exactly the same meaning as Apply.
    #[must_use]
    pub fn enter(&self) -> OperationAuthoringOutcome {
        self.apply()
    }

    /// Clears a rejected terminal application attempt and re-arms the same tool.
    /// Successful publication must instead use [`Self::publication_succeeded`]
    /// so the newly created geometry immediately returns to ordinary interaction.
    pub fn transaction_finished(&mut self) {
        self.picks.clear();
        self.candidate = None;
        self.candidate_confirmed = false;
    }

    /// Records successful publication and leaves operation authoring so a host
    /// can expose the published geometry to its ordinary interaction tool.
    ///
    /// This is a trusted completion notification: call it only after the exact
    /// candidate returned by [`Self::apply`] has published successfully.
    #[must_use]
    pub fn publication_succeeded(&mut self) -> OperationAuthoringOutcome {
        if self.active.is_none() {
            self.transaction_finished();
            return OperationAuthoringOutcome::Inactive;
        }
        self.deactivate();
        OperationAuthoringOutcome::ModeExited
    }

    /// Records a failed scratch-preview preparation without discarding reusable
    /// Fillet operands. An unconfirmed radius preview returns to radius placement;
    /// failure of a confirmed, apply-ready candidate remains terminal.
    pub fn preview_failed(&mut self) {
        if self.candidate_confirmed {
            self.transaction_finished();
        } else {
            self.candidate = None;
            self.candidate_confirmed = false;
        }
    }

    /// First Escape clears picks/candidate; a subsequent Escape exits the tool.
    #[must_use]
    pub fn cancel(&mut self) -> OperationAuthoringOutcome {
        let Some(_) = self.active else {
            return OperationAuthoringOutcome::Inactive;
        };
        if self.picks.is_empty() && self.candidate.is_none() {
            self.active = None;
            OperationAuthoringOutcome::ModeExited
        } else {
            self.transaction_finished();
            OperationAuthoringOutcome::CandidateCleared(self.guidance())
        }
    }

    /// Removes stale identities after an external topology change.
    #[must_use]
    pub fn reconcile(&mut self, document: &SketchDocument) -> OperationAuthoringOutcome {
        let Some(tool) = self.active else {
            self.transaction_finished();
            return OperationAuthoringOutcome::Inactive;
        };
        if self
            .picks
            .iter()
            .any(|pick| validate_pick(document, pick).is_err())
        {
            let stage = self.guidance().stage;
            self.transaction_finished();
            return OperationAuthoringOutcome::Warning(OperationAuthoringWarning {
                tool,
                stage,
                kind: OperationAuthoringWarningKind::StalePick,
                message: "operation operands became stale and were cleared".into(),
            });
        }
        OperationAuthoringOutcome::Collecting {
            picks: self.picks.clone(),
            guidance: self.guidance(),
        }
    }

    /// Reconciles both accepted geometry and the complete retained-session input
    /// stamp. Hosts using [`crate::RetainedEditorCoordinator`] should call this
    /// form after Undo, Redo, reattempt or any external mutation.
    #[must_use]
    pub fn reconcile_exact_input(
        &mut self,
        document: &SketchDocument,
        current_input: PreparedSketchInput,
    ) -> OperationAuthoringOutcome {
        let Some(tool) = self.active else {
            return self.reconcile(document);
        };
        if self
            .picks
            .iter()
            .any(|pick| pick.source_input() != Some(&current_input))
            || self
                .candidate
                .as_ref()
                .is_some_and(|candidate| candidate.source_input() != Some(&current_input))
        {
            let stage = self.guidance().stage;
            self.transaction_finished();
            return OperationAuthoringOutcome::Warning(OperationAuthoringWarning {
                tool,
                stage,
                kind: OperationAuthoringWarningKind::StalePick,
                message: "operation operands belong to an older retained-session input".into(),
            });
        }
        self.reconcile(document)
    }

    fn ensure_defaults(&mut self, document: &SketchDocument) {
        let default = 0.1 * document.model_scale();
        if self.options.fillet_radius.is_none() {
            self.options.fillet_radius = Some(default);
        }
    }

    fn begin_activation(&mut self, document: &SketchDocument, tool: OperationAuthoringTool) {
        self.active = Some(tool);
        self.transaction_finished();
        self.ensure_defaults(document);
    }

    fn stage_complete_candidate(
        &mut self,
        document: &SketchDocument,
        ready: bool,
    ) -> OperationAuthoringOutcome {
        let Some(OperationAuthoringTool::Fillet) = self.active else {
            return OperationAuthoringOutcome::Inactive;
        };
        let candidate = synthesize_fillet(document, &self.picks, self.options);
        match candidate {
            Ok(mut candidate) => {
                candidate.confirmed = ready;
                self.candidate = Some(candidate.clone());
                self.candidate_confirmed = ready;
                OperationAuthoringOutcome::PreviewRequested {
                    candidate,
                    guidance: self.guidance(),
                }
            }
            Err((kind, message)) => {
                let tool = self
                    .active
                    .expect("candidate synthesis requires an active tool");
                let stage = self.guidance().stage;
                self.candidate = None;
                self.candidate_confirmed = false;
                OperationAuthoringOutcome::Warning(OperationAuthoringWarning {
                    tool,
                    stage,
                    kind,
                    message: message.into(),
                })
            }
        }
    }

    fn stage_fillet_radius_candidate(
        &mut self,
        document: &SketchDocument,
        model_position: [f64; 2],
        ready: bool,
    ) -> OperationAuthoringOutcome {
        let fallback = self
            .options
            .fillet_radius
            .expect("operation defaults initialized");
        match pointer_fillet_radius(document, &self.picks, model_position, fallback) {
            Ok(radius) => {
                self.options.fillet_radius = Some(radius);
                self.stage_complete_candidate(document, ready)
            }
            Err(kind) => {
                if self.candidate_confirmed {
                    return self
                        .warning(kind, "fillet radius could not be resolved from the pointer");
                }
                // An exploratory pointer sample is disposable. Do not leave the
                // previous unconfirmed geometry looking current after this sample
                // proved invalid, but retain both parent picks so the next valid
                // sample can immediately rebuild the preview.
                self.candidate = None;
                self.candidate_confirmed = false;
                self.warning(kind, "fillet radius could not be resolved from the pointer")
            }
        }
    }

    fn warning(
        &mut self,
        kind: OperationAuthoringWarningKind,
        message: impl Into<String>,
    ) -> OperationAuthoringOutcome {
        let Some(tool) = self.active else {
            return OperationAuthoringOutcome::Inactive;
        };
        let stage = self.guidance().stage;
        if self.candidate_confirmed {
            self.transaction_finished();
        }
        OperationAuthoringOutcome::Warning(OperationAuthoringWarning {
            tool,
            stage,
            kind,
            message: message.into(),
        })
    }

    #[must_use]
    /// Returns guidance for the currently active operation tool.
    ///
    /// # Panics
    ///
    /// Panics when called while no operation tool is active. Hosts should inspect
    /// [`Self::active_tool`] before requesting active-mode guidance.
    pub fn guidance(&self) -> OperationAuthoringGuidance {
        let tool = self
            .active
            .expect("guidance is requested only for an active operation tool");
        let (stage, expected, message) = if self.candidate_confirmed {
            (
                OperationAuthoringStage::PreviewReady,
                Vec::new(),
                "Review the accepted preview, then Apply or press Enter",
            )
        } else {
            match self.picks.len() {
                0 => (
                    OperationAuthoringStage::PickFirstFilletCurve,
                    vec![
                        OperationAuthoringOperandKind::RegularCurveSpan,
                        OperationAuthoringOperandKind::FilletCornerPoint,
                    ],
                    "Pick two curves, or pick one unambiguous polyline corner",
                ),
                1 => (
                    OperationAuthoringStage::PickSecondFilletCurve,
                    vec![OperationAuthoringOperandKind::DistinctRegularCurveSpan],
                    "Pick a distinct second curve near the portion to retain",
                ),
                _ => (
                    OperationAuthoringStage::PlaceFilletRadius,
                    vec![OperationAuthoringOperandKind::FilletRadius],
                    "Move the pointer to place the flexible radius, then click",
                ),
            }
        };
        OperationAuthoringGuidance {
            tool,
            stage,
            expected,
            message,
        }
    }
}

fn valid_optional_positive(value: Option<f64>) -> bool {
    value.is_none_or(|value| value.is_finite() && value > 0.0)
}

fn common_source_input(
    picks: &[OperationAuthoringPick],
) -> Result<Option<Arc<PreparedSketchInput>>, OperationAuthoringWarningKind> {
    let source = picks.first().and_then(|pick| pick.source_input.as_ref());
    if picks
        .iter()
        .all(|pick| pick.source_input.as_ref() == source)
    {
        Ok(source.cloned())
    } else {
        Err(OperationAuthoringWarningKind::StalePick)
    }
}

fn validate_pick(
    document: &SketchDocument,
    pick: &OperationAuthoringPick,
) -> Result<(), OperationAuthoringWarningKind> {
    if !pick.curve_parameter.is_finite() || !pick.model_position.into_iter().all(f64::is_finite) {
        return Err(OperationAuthoringWarningKind::NonFinitePick);
    }
    let Some(span) = pick.curve_span() else {
        return Err(OperationAuthoringWarningKind::WrongOperandKind);
    };
    if !document
        .curve_spans(span.curve)
        .is_ok_and(|spans| spans.contains(&span))
    {
        return Err(OperationAuthoringWarningKind::MissingObject);
    }
    let jet = document
        .evaluate_curve_jet(span, pick.curve_parameter)
        .map_err(|_| OperationAuthoringWarningKind::StalePick)?;
    let error =
        (jet.position.x - pick.model_position[0]).hypot(jet.position.y - pick.model_position[1]);
    let tolerance = (document.model_scale() * 1.0e-7).max(1.0e-10);
    if !error.is_finite() || error > tolerance {
        return Err(OperationAuthoringWarningKind::StalePick);
    }
    Ok(())
}

pub(crate) fn resolve_operation_item_picks(
    document: &SketchDocument,
    tool: OperationAuthoringTool,
    item: SelectionItem,
    curve_parameter: Option<f64>,
) -> Result<Vec<OperationAuthoringPick>, OperationAuthoringWarningKind> {
    match item {
        SelectionItem::Curve(_) => {
            OperationAuthoringPick::for_item(document, item, curve_parameter).map(|pick| vec![pick])
        }
        SelectionItem::Point(point) if tool == OperationAuthoringTool::Fillet => {
            resolve_fillet_corner_picks(document, point)
        }
        SelectionItem::Point(_)
        | SelectionItem::Constraint(_)
        | SelectionItem::Dimension(_)
        | SelectionItem::Feature(_)
        | SelectionItem::FeatureCorner(_) => Err(OperationAuthoringWarningKind::WrongOperandKind),
    }
}

fn resolve_fillet_corner_picks(
    document: &SketchDocument,
    point: DesignPointId,
) -> Result<Vec<OperationAuthoringPick>, OperationAuthoringWarningKind> {
    if document.point(point).is_none() {
        return Err(OperationAuthoringWarningKind::MissingObject);
    }
    let mut endpoint_seen = false;
    let mut candidates = Vec::new();
    for curve in document.curves() {
        let CurveDefinition::Polyline { points, closed, .. } = &curve.definition else {
            continue;
        };
        if *closed {
            if points.contains(&point) {
                endpoint_seen = true;
            }
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
                    .map_err(|_| OperationAuthoringWarningKind::MissingObject)?,
            };
            let outgoing = CurveSpan {
                curve: curve.id,
                segment: u32::try_from(index)
                    .map_err(|_| OperationAuthoringWarningKind::MissingObject)?,
            };
            candidates.push([
                OperationAuthoringPick::at_curve_parameter(document, incoming, 0.75)?
                    .with_fillet_trim_endpoint_hint(DocumentFilletTrimEndpoint::End),
                OperationAuthoringPick::at_curve_parameter(document, outgoing, 0.25)?
                    .with_fillet_trim_endpoint_hint(DocumentFilletTrimEndpoint::Start),
            ]);
        }
    }
    match candidates.as_slice() {
        [picks] => Ok(picks.to_vec()),
        [] if endpoint_seen => Err(OperationAuthoringWarningKind::FilletCornerNotInterior),
        [] => Err(OperationAuthoringWarningKind::WrongOperandKind),
        _ => Err(OperationAuthoringWarningKind::AmbiguousFilletCorner),
    }
}

fn line_span_endpoint_ids(
    document: &SketchDocument,
    span: CurveSpan,
) -> Option<(DesignPointId, DesignPointId)> {
    let curve = document.curve(span.curve)?;
    match &curve.definition {
        CurveDefinition::Line { start, end, .. } if span.segment == 0 => Some((*start, *end)),
        CurveDefinition::Polyline { points, closed, .. } => {
            let index = usize::try_from(span.segment).ok()?;
            let end_index = index.checked_add(1)?;
            if index >= points.len() || (!closed && end_index >= points.len()) {
                return None;
            }
            Some((points[index], points[end_index % points.len()]))
        }
        _ => None,
    }
}

fn same_open_polyline_adjacent_spans(
    document: &SketchDocument,
    first: CurveSpan,
    second: CurveSpan,
) -> bool {
    first.curve == second.curve
        && first.segment.abs_diff(second.segment) == 1
        && document.curve(first.curve).is_some_and(|curve| {
            matches!(
                curve.definition,
                CurveDefinition::Polyline { closed: false, .. }
            )
        })
}

fn line_span_shared_endpoint_hints(
    document: &SketchDocument,
    first: CurveSpan,
    second: CurveSpan,
) -> Option<(DocumentFilletTrimEndpoint, DocumentFilletTrimEndpoint)> {
    let (first_start, first_end) = line_span_endpoint_ids(document, first)?;
    let (second_start, second_end) = line_span_endpoint_ids(document, second)?;
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
    let [hints] = matches.as_slice() else {
        return None;
    };
    Some(*hints)
}

fn pointer_fillet_radius(
    document: &SketchDocument,
    picks: &[OperationAuthoringPick],
    model_position: [f64; 2],
    fallback: f64,
) -> Result<f64, OperationAuthoringWarningKind> {
    if !model_position.into_iter().all(f64::is_finite) {
        return Err(OperationAuthoringWarningKind::NonFinitePick);
    }
    let [first, second] = picks else {
        return Err(OperationAuthoringWarningKind::WrongArity);
    };
    let first_span = first
        .curve_span()
        .ok_or(OperationAuthoringWarningKind::WrongOperandKind)?;
    let second_span = second
        .curve_span()
        .ok_or(OperationAuthoringWarningKind::WrongOperandKind)?;
    let first_jet = document
        .evaluate_curve_jet(first_span, first.curve_parameter)
        .map_err(|_| OperationAuthoringWarningKind::StalePick)?;
    let second_jet = document
        .evaluate_curve_jet(second_span, second.curve_parameter)
        .map_err(|_| OperationAuthoringWarningKind::StalePick)?;
    let first_length = first_jet.first_derivative.norm();
    let second_length = second_jet.first_derivative.norm();
    if !first_length.is_finite()
        || !second_length.is_finite()
        || first_length <= 0.0
        || second_length <= 0.0
    {
        return Err(OperationAuthoringWarningKind::SingularFillet);
    }
    let first_direction = [
        first_jet.first_derivative.x / first_length,
        first_jet.first_derivative.y / first_length,
    ];
    let second_direction = [
        second_jet.first_derivative.x / second_length,
        second_jet.first_derivative.y / second_length,
    ];
    let cross = first_direction[0] * second_direction[1] - first_direction[1] * second_direction[0];
    if !cross.is_finite() || cross.abs() <= 1.0e-8 {
        return Err(OperationAuthoringWarningKind::SingularFillet);
    }
    let between = [
        second_jet.position.x - first_jet.position.x,
        second_jet.position.y - first_jet.position.y,
    ];
    let first_parameter =
        (between[0] * second_direction[1] - between[1] * second_direction[0]) / cross;
    let intersection = [
        first_jet.position.x + first_parameter * first_direction[0],
        first_jet.position.y + first_parameter * first_direction[1],
    ];
    let pointer = [
        model_position[0] - intersection[0],
        model_position[1] - intersection[1],
    ];
    let distance = pointer[0].hypot(pointer[1]);
    if !distance.is_finite() {
        return Err(OperationAuthoringWarningKind::NonFinitePick);
    }
    if distance <= document.model_scale() * 1.0e-10 {
        return Ok(fallback);
    }
    let oriented_ray = |direction: [f64; 2]| {
        let dot = direction[0] * pointer[0] + direction[1] * pointer[1];
        if dot < 0.0 {
            [-direction[0], -direction[1]]
        } else {
            direction
        }
    };
    let first_ray = oriented_ray(first_direction);
    let second_ray = oriented_ray(second_direction);
    let wedge_cosine =
        (first_ray[0] * second_ray[0] + first_ray[1] * second_ray[1]).clamp(-1.0, 1.0);
    let sine_half = (0.5 * (1.0 - wedge_cosine)).sqrt();
    let denominator = 1.0 - sine_half;
    if !sine_half.is_finite()
        || !denominator.is_finite()
        || sine_half <= 1.0e-8
        || denominator <= 1.0e-8
    {
        return Err(OperationAuthoringWarningKind::SingularFillet);
    }
    let radius = distance * sine_half / denominator;
    if !radius.is_finite() || radius <= document.model_scale() * 1.0e-12 {
        return Err(OperationAuthoringWarningKind::NonFinitePick);
    }
    Ok(radius)
}

#[derive(Clone, Copy, Debug)]
struct LocalFilletSolution {
    parameters: [f64; 2],
    sides: [DocumentCurveNormalSide; 2],
    center: [f64; 2],
    score: f64,
}

#[allow(
    clippy::too_many_lines,
    reason = "branch-explicit local root selection and request synthesis remain one auditable path"
)]
fn synthesize_fillet(
    document: &SketchDocument,
    picks: &[OperationAuthoringPick],
    options: OperationAuthoringOptions,
) -> Result<OperationAuthoringCandidate, (OperationAuthoringWarningKind, &'static str)> {
    let [first, second] = picks else {
        return Err((
            OperationAuthoringWarningKind::WrongArity,
            "fillet requires two curve picks",
        ));
    };
    let first_span = first.curve_span().expect("validated fillet first pick");
    let second_span = second.curve_span().expect("validated fillet second pick");
    if first_span == second_span
        || (first_span.curve == second_span.curve
            && !same_open_polyline_adjacent_spans(document, first_span, second_span))
    {
        return Err((
            OperationAuthoringWarningKind::DuplicateSupport,
            "same-curve fillet parents must be distinct adjacent line spans",
        ));
    }
    if !is_affine_line_span(document, first_span) && !is_affine_line_span(document, second_span) {
        return Err((
            OperationAuthoringWarningKind::UnsupportedFilletPair,
            "Fillet authoring between two curved parents requires certified pairwise continuation",
        ));
    }
    let source_input = common_source_input(picks).map_err(|kind| {
        (
            kind,
            "fillet operands were picked from different accepted inputs",
        )
    })?;
    if document.trim_views_for_span(first_span).next().is_some()
        || document.trim_views_for_span(second_span).next().is_some()
    {
        return Err((
            OperationAuthoringWarningKind::AlreadyTrimmed,
            "a selected support already has persistent trim topology",
        ));
    }
    let radius = options
        .fillet_radius
        .expect("operation defaults initialized");
    let first_jet = document
        .evaluate_curve_jet(first_span, first.curve_parameter)
        .map_err(|_| {
            (
                OperationAuthoringWarningKind::UnresolvedLocalFilletRoot,
                "the first picked curve is not regular",
            )
        })?;
    let second_jet = document
        .evaluate_curve_jet(second_span, second.curve_parameter)
        .map_err(|_| {
            (
                OperationAuthoringWarningKind::UnresolvedLocalFilletRoot,
                "the second picked curve is not regular",
            )
        })?;
    let tangent_cross = first_jet.first_derivative.x * second_jet.first_derivative.y
        - first_jet.first_derivative.y * second_jet.first_derivative.x;
    let tangent_scale = first_jet.first_derivative.norm() * second_jet.first_derivative.norm();
    if !tangent_cross.is_finite() || !tangent_scale.is_finite() || tangent_scale <= 0.0 {
        return Err((
            OperationAuthoringWarningKind::SingularFillet,
            "a picked parent tangent is zero-speed or numerically unresolved",
        ));
    }
    let picked_tangents_parallel = tangent_cross.abs() <= 1.0e-8 * tangent_scale;

    let mut solutions = Vec::new();
    for first_side in [
        DocumentCurveNormalSide::Left,
        DocumentCurveNormalSide::Right,
    ] {
        for second_side in [
            DocumentCurveNormalSide::Left,
            DocumentCurveNormalSide::Right,
        ] {
            solutions.extend(local_fillet_roots(
                document,
                [first, second],
                [first_side, second_side],
                radius,
            ));
        }
    }
    solutions.sort_by(|left, right| {
        left.score
            .total_cmp(&right.score)
            .then_with(|| side_rank(left.sides).cmp(&side_rank(right.sides)))
    });
    if solutions.is_empty() && picked_tangents_parallel {
        return Err((
            OperationAuthoringWarningKind::SingularFillet,
            "the bounded local parent offsets are parallel or numerically unresolved",
        ));
    }
    let mut solution = select_local_fillet_solution(document, &solutions)?;
    if options.fillet_flip_first_side || options.fillet_flip_second_side {
        let target_sides = [
            if options.fillet_flip_first_side {
                flip_side(solution.sides[0])
            } else {
                solution.sides[0]
            },
            if options.fillet_flip_second_side {
                flip_side(solution.sides[1])
            } else {
                solution.sides[1]
            },
        ];
        let mut corrected = solutions
            .iter()
            .copied()
            .filter(|candidate| candidate.sides == target_sides)
            .collect::<Vec<_>>();
        corrected.sort_by(|left, right| left.score.total_cmp(&right.score));
        solution = select_local_fillet_solution(document, &corrected).map_err(|_| {
            (
                OperationAuthoringWarningKind::UnresolvedLocalFilletRoot,
                "the selected side combination has no bounded local fillet root",
            )
        })?;
    }

    let first_contact = document
        .evaluate_curve_jet(first_span, solution.parameters[0])
        .map_err(|_| {
            (
                OperationAuthoringWarningKind::UnresolvedLocalFilletRoot,
                "the inferred first fillet contact is invalid",
            )
        })?;
    let second_contact = document
        .evaluate_curve_jet(second_span, solution.parameters[1])
        .map_err(|_| {
            (
                OperationAuthoringWarningKind::UnresolvedLocalFilletRoot,
                "the inferred second fillet contact is invalid",
            )
        })?;
    for (jet, side) in [
        (first_contact, solution.sides[0]),
        (second_contact, solution.sides[1]),
    ] {
        let signed_curvature = jet
            .differential()
            .map_err(|_| {
                (
                    OperationAuthoringWarningKind::SingularFillet,
                    "an inferred fillet contact is not regular",
                )
            })?
            .signed_curvature;
        validate_fillet_offset_regular(signed_curvature, side, radius)?;
    }
    let first_angle = (first_contact.position.y - solution.center[1])
        .atan2(first_contact.position.x - solution.center[0]);
    let second_angle = (second_contact.position.y - solution.center[1])
        .atan2(second_contact.position.x - solution.center[0]);
    let ccw = (second_angle - first_angle).rem_euclid(std::f64::consts::TAU);
    let mut endpoint_order = if ccw <= std::f64::consts::PI {
        DocumentFilletEndpointOrder::FirstThenSecond
    } else {
        DocumentFilletEndpointOrder::SecondThenFirst
    };
    if options.fillet_alternate_arc {
        endpoint_order = match endpoint_order {
            DocumentFilletEndpointOrder::FirstThenSecond => {
                DocumentFilletEndpointOrder::SecondThenFirst
            }
            DocumentFilletEndpointOrder::SecondThenFirst => {
                DocumentFilletEndpointOrder::FirstThenSecond
            }
        };
    }
    let [first_parent, second_parent] = fillet_parents(
        document,
        [first, second],
        solution.parameters,
        solution.sides,
    )?;
    Ok(OperationAuthoringCandidate {
        tool: OperationAuthoringTool::Fillet,
        picks: picks.to_vec(),
        request: SketchOperationRequest::AssociativeFillet {
            label: "Fillet".into(),
            request: CurveCurveFilletRequest {
                first: first_parent,
                second: second_parent,
                endpoint_order,
                sweep: DocumentArcSweep::CounterClockwise,
                radius,
                radius_mode: options.fillet_radius_mode,
            },
        },
        confirmed: true,
        source_input,
    })
}

fn select_local_fillet_solution(
    document: &SketchDocument,
    solutions: &[LocalFilletSolution],
) -> Result<LocalFilletSolution, (OperationAuthoringWarningKind, &'static str)> {
    let Some(solution) = solutions.first().copied() else {
        return Err((
            OperationAuthoringWarningKind::UnresolvedLocalFilletRoot,
            "no fillet root was found inside the picked local neighborhoods",
        ));
    };
    if solutions.iter().skip(1).any(|other| {
        fillet_scores_nearly_tied(solution.score, other.score)
            && fillet_solutions_materially_distinct(document, solution, *other)
    }) {
        return Err((
            OperationAuthoringWarningKind::AmbiguousFilletRoot,
            "multiple materially distinct local fillet roots are equally close to the picks",
        ));
    }
    Ok(solution)
}

fn fillet_scores_nearly_tied(first: f64, second: f64) -> bool {
    let scale = first.abs().max(second.abs()).max(1.0);
    (first - second).abs() <= 1.0e-7 * scale
}

fn fillet_solutions_materially_distinct(
    document: &SketchDocument,
    first: LocalFilletSolution,
    second: LocalFilletSolution,
) -> bool {
    let position_tolerance = (document.model_scale() * 1.0e-7).max(1.0e-10);
    (first.center[0] - second.center[0]).hypot(first.center[1] - second.center[1])
        > position_tolerance
        || first
            .parameters
            .into_iter()
            .zip(second.parameters)
            .any(|(left, right)| (left - right).abs() > 1.0e-8)
}

fn side_rank(sides: [DocumentCurveNormalSide; 2]) -> u8 {
    match sides {
        [DocumentCurveNormalSide::Left, DocumentCurveNormalSide::Left] => 0,
        [
            DocumentCurveNormalSide::Left,
            DocumentCurveNormalSide::Right,
        ] => 1,
        [
            DocumentCurveNormalSide::Right,
            DocumentCurveNormalSide::Left,
        ] => 2,
        [
            DocumentCurveNormalSide::Right,
            DocumentCurveNormalSide::Right,
        ] => 3,
    }
}

const fn flip_side(side: DocumentCurveNormalSide) -> DocumentCurveNormalSide {
    match side {
        DocumentCurveNormalSide::Left => DocumentCurveNormalSide::Right,
        DocumentCurveNormalSide::Right => DocumentCurveNormalSide::Left,
    }
}

fn fillet_parent(
    document: &SketchDocument,
    pick: &OperationAuthoringPick,
    total_parameter: f64,
    side: DocumentCurveNormalSide,
    neighborhood: ContactNeighborhood,
) -> Result<CurveFilletParentRequest, (OperationAuthoringWarningKind, &'static str)> {
    let span = pick.curve_span().expect("validated fillet pick");
    let domain = primary_domain(document, span).ok_or((
        OperationAuthoringWarningKind::UnsupportedCurveFamily,
        "the selected curve does not expose a fillet contact domain",
    ))?;
    match domain {
        FilletContactDomain::Periodic { period } => {
            let ContactNeighborhood::Local {
                lower: neighborhood_lower,
                upper: neighborhood_upper,
            } = neighborhood
            else {
                return Err((
                    OperationAuthoringWarningKind::UnresolvedLocalFilletRoot,
                    "a periodic Fillet parent requires a certified local branch cell",
                ));
            };
            if !(neighborhood_lower < total_parameter && total_parameter < neighborhood_upper) {
                return Err((
                    OperationAuthoringWarningKind::UnresolvedLocalFilletRoot,
                    "the periodic Fillet root escaped its certified branch cell",
                ));
            }
            let (parameter, winding) = normalize_periodic(total_parameter, period).ok_or((
                OperationAuthoringWarningKind::UnresolvedLocalFilletRoot,
                "periodic fillet state cannot be represented",
            ))?;
            let trim_endpoint = fillet_trim_endpoint(document, pick, total_parameter, period)?;
            let anchor_total = match trim_endpoint {
                DocumentFilletTrimEndpoint::End => total_parameter - 0.5 * period,
                DocumentFilletTrimEndpoint::Start => total_parameter + 0.5 * period,
            };
            let (anchor_parameter, anchor_winding) = normalize_periodic(anchor_total, period)
                .ok_or((
                    OperationAuthoringWarningKind::UnresolvedLocalFilletRoot,
                    "periodic trim anchor cannot be represented",
                ))?;
            Ok(CurveFilletParentRequest {
                curve: span,
                parameter,
                winding,
                neighborhood,
                side,
                trim_endpoint,
                periodic_anchor: Some(DocumentTrimParameter {
                    parameter: anchor_parameter,
                    winding: anchor_winding,
                }),
            })
        }
        FilletContactDomain::Bounded { lower, upper } => {
            if !(lower < total_parameter && total_parameter < upper) {
                return Err((
                    OperationAuthoringWarningKind::UnresolvedLocalFilletRoot,
                    "a bounded fillet contact reached a support endpoint",
                ));
            }
            let trim_endpoint =
                fillet_trim_endpoint(document, pick, total_parameter, upper - lower)?;
            let valid_neighborhood = match neighborhood {
                ContactNeighborhood::Interior => is_affine_line_span(document, span),
                ContactNeighborhood::Local {
                    lower: neighborhood_lower,
                    upper: neighborhood_upper,
                } => {
                    lower <= neighborhood_lower
                        && neighborhood_lower < total_parameter
                        && total_parameter < neighborhood_upper
                        && neighborhood_upper <= upper
                }
                ContactNeighborhood::Start | ContactNeighborhood::End => false,
            };
            if !valid_neighborhood {
                return Err((
                    OperationAuthoringWarningKind::UnresolvedLocalFilletRoot,
                    "the bounded Fillet root escaped its semantic support or certified branch cell",
                ));
            }
            Ok(CurveFilletParentRequest {
                curve: span,
                parameter: total_parameter,
                winding: 0,
                neighborhood,
                side,
                trim_endpoint,
                periodic_anchor: None,
            })
        }
    }
}

fn fillet_parents(
    document: &SketchDocument,
    picks: [&OperationAuthoringPick; 2],
    parameters: [f64; 2],
    sides: [DocumentCurveNormalSide; 2],
) -> Result<[CurveFilletParentRequest; 2], (OperationAuthoringWarningKind, &'static str)> {
    let spans = picks.map(|pick| pick.curve_span().expect("validated Fillet pick"));
    let affine = spans.map(|span| is_affine_line_span(document, span));
    let neighborhoods = match affine {
        [true, true] => [ContactNeighborhood::Interior; 2],
        [true, false] => [
            ContactNeighborhood::Interior,
            certified_curved_fillet_neighborhood(document, spans[0], spans[1], parameters[1])?,
        ],
        [false, true] => [
            certified_curved_fillet_neighborhood(document, spans[1], spans[0], parameters[0])?,
            ContactNeighborhood::Interior,
        ],
        [false, false] => {
            return Err((
                OperationAuthoringWarningKind::UnsupportedFilletPair,
                "Fillet authoring between two curved parents requires certified pairwise continuation",
            ));
        }
    };
    Ok([
        fillet_parent(
            document,
            picks[0],
            parameters[0],
            sides[0],
            neighborhoods[0],
        )?,
        fillet_parent(
            document,
            picks[1],
            parameters[1],
            sides[1],
            neighborhoods[1],
        )?,
    ])
}

fn certified_curved_fillet_neighborhood(
    document: &SketchDocument,
    line: CurveSpan,
    curve: CurveSpan,
    parameter: f64,
) -> Result<ContactNeighborhood, (OperationAuthoringWarningKind, &'static str)> {
    let domain = primary_domain(document, curve).ok_or((
        OperationAuthoringWarningKind::UnsupportedCurveFamily,
        "the curved Fillet parent does not expose a contact domain",
    ))?;
    let (lower, upper) = match domain {
        FilletContactDomain::Bounded { lower, upper } => (lower, upper),
        FilletContactDomain::Periodic { period } => {
            (parameter - 0.5 * period, parameter + 0.5 * period)
        }
    };
    document
        .certify_line_curve_fillet_branch_cell(line, curve, parameter, lower, upper)
        .map_err(|_| {
            (
                OperationAuthoringWarningKind::UnresolvedLocalFilletRoot,
                "the selected line/curve Fillet root has no certified tangent branch cell",
            )
        })
}

fn fillet_trim_endpoint(
    document: &SketchDocument,
    pick: &OperationAuthoringPick,
    contact_parameter: f64,
    parameter_scale: f64,
) -> Result<DocumentFilletTrimEndpoint, (OperationAuthoringWarningKind, &'static str)> {
    if let Some(endpoint) = pick.fillet_trim_endpoint_hint {
        return Ok(endpoint);
    }
    let span = pick.curve_span().expect("validated fillet pick");
    let contact = document
        .evaluate_curve_jet(span, contact_parameter)
        .map_err(|_| {
            (
                OperationAuthoringWarningKind::UnresolvedLocalFilletRoot,
                "the inferred trim contact is not evaluable",
            )
        })?;
    let parameter_tolerance = (parameter_scale.abs() * 1.0e-9).max(1.0e-12);
    let position_tolerance = (document.model_scale() * 1.0e-8).max(1.0e-10);
    let position_delta = (contact.position.x - pick.model_position[0])
        .hypot(contact.position.y - pick.model_position[1]);
    if (pick.curve_parameter - contact_parameter).abs() <= parameter_tolerance
        || position_delta <= position_tolerance
    {
        return Err((
            OperationAuthoringWarningKind::AmbiguousTrimSide,
            "the pick lies on the inferred contact and does not identify a retained trim side",
        ));
    }
    Ok(if pick.curve_parameter < contact_parameter {
        DocumentFilletTrimEndpoint::End
    } else {
        DocumentFilletTrimEndpoint::Start
    })
}

fn normalize_periodic(total: f64, period: f64) -> Option<(f64, i32)> {
    if !total.is_finite() || !period.is_finite() || period <= 0.0 {
        return None;
    }
    let parameter = total.rem_euclid(period);
    let winding_value = ((total - parameter) / period).round();
    if winding_value < f64::from(i32::MIN) || winding_value > f64::from(i32::MAX) {
        return None;
    }
    #[allow(clippy::cast_possible_truncation)]
    let winding = winding_value as i32;
    Some((parameter, winding))
}

#[derive(Clone, Copy, Debug)]
enum FilletContactDomain {
    Bounded { lower: f64, upper: f64 },
    Periodic { period: f64 },
}

fn is_affine_line_span(document: &SketchDocument, span: CurveSpan) -> bool {
    document
        .curve(span.curve)
        .is_some_and(|curve| match &curve.definition {
            CurveDefinition::Line { .. } => span.segment == 0,
            CurveDefinition::Polyline { points, closed, .. } => {
                let segment_count = if *closed {
                    points.len()
                } else {
                    points.len().saturating_sub(1)
                };
                usize::try_from(span.segment).is_ok_and(|segment| segment < segment_count)
            }
            CurveDefinition::Circle { .. }
            | CurveDefinition::CircularArc { .. }
            | CurveDefinition::Ellipse { .. }
            | CurveDefinition::EllipticalArc { .. }
            | CurveDefinition::RationalQuadraticConic { .. }
            | CurveDefinition::ParabolaSegment { .. }
            | CurveDefinition::HyperbolaSegment { .. }
            | CurveDefinition::QuadraticBezier { .. }
            | CurveDefinition::CubicBezier { .. }
            | CurveDefinition::BSpline { .. }
            | CurveDefinition::Nurbs { .. } => false,
        })
}

fn primary_domain(document: &SketchDocument, span: CurveSpan) -> Option<FilletContactDomain> {
    document
        .curve_contact_domains(span)
        .ok()?
        .into_iter()
        .find_map(|domain| match domain {
            ContactDomain::SupportingLine => None,
            ContactDomain::Bounded { lower, upper } => {
                Some(FilletContactDomain::Bounded { lower, upper })
            }
            ContactDomain::Periodic { period } => Some(FilletContactDomain::Periodic { period }),
        })
}

fn local_fillet_roots(
    document: &SketchDocument,
    picks: [&OperationAuthoringPick; 2],
    sides: [DocumentCurveNormalSide; 2],
    radius: f64,
) -> Vec<LocalFilletSolution> {
    let Some(spans) = picks[0]
        .curve_span()
        .zip(picks[1].curve_span())
        .map(|(first, second)| [first, second])
    else {
        return Vec::new();
    };
    let Some(bounds) = local_parameter_bounds(document, picks[0])
        .zip(local_parameter_bounds(document, picks[1]))
        .map(|(first, second)| [first, second])
    else {
        return Vec::new();
    };
    let seeds = bounds.map(|(lower, upper)| {
        let width = upper - lower;
        [
            lower + 0.15 * width,
            lower + 0.5 * width,
            lower + 0.85 * width,
        ]
    });
    let mut solutions = Vec::new();
    for first in seeds[0] {
        for second in seeds[1] {
            let Some(solution) = local_fillet_root_from_seed(
                document,
                picks,
                spans,
                bounds,
                sides,
                radius,
                [first, second],
            ) else {
                continue;
            };
            if solutions
                .iter()
                .all(|existing| fillet_solutions_materially_distinct(document, *existing, solution))
            {
                solutions.push(solution);
            }
        }
    }
    solutions
}

fn local_fillet_root_from_seed(
    document: &SketchDocument,
    picks: [&OperationAuthoringPick; 2],
    spans: [CurveSpan; 2],
    bounds: [(f64, f64); 2],
    sides: [DocumentCurveNormalSide; 2],
    radius: f64,
    mut parameters: [f64; 2],
) -> Option<LocalFilletSolution> {
    let tolerance = (document.model_scale() * 1.0e-8).max(1.0e-11);
    for _ in 0..LOCAL_FILLET_ITERATIONS {
        let offsets = [
            offset_curve_point(document, spans[0], parameters[0], sides[0], radius)?,
            offset_curve_point(document, spans[1], parameters[1], sides[1], radius)?,
        ];
        let residual = [offsets[0][0] - offsets[1][0], offsets[0][1] - offsets[1][1]];
        let norm = residual[0].hypot(residual[1]);
        if norm <= tolerance {
            let parameter_score = (parameters[0] - picks[0].curve_parameter).abs()
                / (bounds[0].1 - bounds[0].0)
                + (parameters[1] - picks[1].curve_parameter).abs() / (bounds[1].1 - bounds[1].0);
            let center = [
                0.5 * (offsets[0][0] + offsets[1][0]),
                0.5 * (offsets[0][1] + offsets[1][1]),
            ];
            return Some(LocalFilletSolution {
                parameters,
                sides,
                center,
                score: parameter_score,
            });
        }
        let first_derivative = offset_curve_derivative(
            document,
            spans[0],
            parameters[0],
            bounds[0],
            sides[0],
            radius,
        )?;
        let second_derivative = offset_curve_derivative(
            document,
            spans[1],
            parameters[1],
            bounds[1],
            sides[1],
            radius,
        )?;
        let matrix = [
            [first_derivative[0], -second_derivative[0]],
            [first_derivative[1], -second_derivative[1]],
        ];
        let determinant = matrix[0][0] * matrix[1][1] - matrix[0][1] * matrix[1][0];
        let scale = first_derivative[0].hypot(first_derivative[1])
            * second_derivative[0].hypot(second_derivative[1]);
        if !determinant.is_finite() || !scale.is_finite() || determinant.abs() <= 1.0e-10 * scale {
            return None;
        }
        let step = [
            (-residual[0] * matrix[1][1] + matrix[0][1] * residual[1]) / determinant,
            (-matrix[0][0] * residual[1] + residual[0] * matrix[1][0]) / determinant,
        ];
        if !step.into_iter().all(f64::is_finite) {
            return None;
        }
        let mut accepted = false;
        let mut factor = 1.0;
        for _ in 0..8 {
            let candidate = [
                (parameters[0] + factor * step[0]).clamp(bounds[0].0, bounds[0].1),
                (parameters[1] + factor * step[1]).clamp(bounds[1].0, bounds[1].1),
            ];
            let next = [
                offset_curve_point(document, spans[0], candidate[0], sides[0], radius)?,
                offset_curve_point(document, spans[1], candidate[1], sides[1], radius)?,
            ];
            let next_norm = (next[0][0] - next[1][0]).hypot(next[0][1] - next[1][1]);
            if next_norm < norm {
                parameters = candidate;
                accepted = true;
                break;
            }
            factor *= 0.5;
        }
        if !accepted {
            return None;
        }
    }
    None
}

fn local_parameter_bounds(
    document: &SketchDocument,
    pick: &OperationAuthoringPick,
) -> Option<(f64, f64)> {
    let span = pick.curve_span()?;
    let parameter = pick.curve_parameter;
    match primary_domain(document, span)? {
        FilletContactDomain::Bounded { lower, upper } => {
            let width = upper - lower;
            let epsilon = (width * 1.0e-9).max(f64::EPSILON);
            let ordinary_start =
                (parameter - LOCAL_FILLET_WINDOW_FRACTION * width).max(lower + epsilon);
            let ordinary_end =
                (parameter + LOCAL_FILLET_WINDOW_FRACTION * width).min(upper - epsilon);
            let (start, end) = match pick.fillet_trim_endpoint_hint {
                Some(DocumentFilletTrimEndpoint::Start) => (lower + epsilon, ordinary_end),
                Some(DocumentFilletTrimEndpoint::End) => (ordinary_start, upper - epsilon),
                None => (ordinary_start, ordinary_end),
            };
            (start < parameter && parameter < end).then_some((start, end))
        }
        FilletContactDomain::Periodic { period } => Some((
            parameter - LOCAL_FILLET_WINDOW_FRACTION * period,
            parameter + LOCAL_FILLET_WINDOW_FRACTION * period,
        )),
    }
}

fn offset_curve_point(
    document: &SketchDocument,
    span: CurveSpan,
    parameter: f64,
    side: DocumentCurveNormalSide,
    radius: f64,
) -> Option<[f64; 2]> {
    let jet = document.evaluate_curve_jet(span, parameter).ok()?;
    let differential = jet.differential().ok()?;
    let sign = match side {
        DocumentCurveNormalSide::Left => 1.0,
        DocumentCurveNormalSide::Right => -1.0,
    };
    let result = [
        jet.position.x + sign * radius * differential.left_normal.x,
        jet.position.y + sign * radius * differential.left_normal.y,
    ];
    result.into_iter().all(f64::is_finite).then_some(result)
}

fn validate_fillet_offset_regular(
    signed_curvature: f64,
    side: DocumentCurveNormalSide,
    radius: f64,
) -> Result<(), (OperationAuthoringWarningKind, &'static str)> {
    let side_sign = match side {
        DocumentCurveNormalSide::Left => 1.0,
        DocumentCurveNormalSide::Right => -1.0,
    };
    let factor = 1.0 - side_sign * radius * signed_curvature;
    if !factor.is_finite() || factor.abs() <= 1.0e-8 {
        return Err((
            OperationAuthoringWarningKind::SingularFillet,
            "the selected radius reaches a singular parent offset",
        ));
    }
    Ok(())
}

fn offset_curve_derivative(
    document: &SketchDocument,
    span: CurveSpan,
    parameter: f64,
    bounds: (f64, f64),
    side: DocumentCurveNormalSide,
    radius: f64,
) -> Option<[f64; 2]> {
    let width = bounds.1 - bounds.0;
    let h = (width * 1.0e-5).max(1.0e-7);
    let lower = (parameter - h).max(bounds.0);
    let upper = (parameter + h).min(bounds.1);
    if lower >= parameter || upper <= parameter {
        return None;
    }
    let first = offset_curve_point(document, span, lower, side, radius)?;
    let second = offset_curve_point(document, span, upper, side, radius)?;
    let inverse = 1.0 / (upper - lower);
    let derivative = [
        (second[0] - first[0]) * inverse,
        (second[1] - first[1]) * inverse,
    ];
    derivative
        .into_iter()
        .all(f64::is_finite)
        .then_some(derivative)
}

#[cfg(test)]
mod tests {
    use geosolve_sketch::{
        CurveDefinition, DocumentSolveRequest, RetainedSketchDocumentSession, SketchDocument,
        SolverConfig,
    };

    use super::*;

    fn perpendicular_lines() -> (SketchDocument, [CurveSpan; 2]) {
        let mut document = SketchDocument::new(1.0).unwrap();
        let a = document.add_point("a", [-4.0, 0.0]).unwrap();
        let b = document.add_point("b", [0.0, 0.0]).unwrap();
        let c = document.add_point("c", [0.0, 0.0]).unwrap();
        let d = document.add_point("d", [0.0, 4.0]).unwrap();
        let first = document
            .add_curve(
                "first",
                CurveDefinition::Line {
                    start: a,
                    end: b,
                    branch_direction: [1.0, 0.0],
                },
            )
            .unwrap();
        let second = document
            .add_curve(
                "second",
                CurveDefinition::Line {
                    start: c,
                    end: d,
                    branch_direction: [0.0, 1.0],
                },
            )
            .unwrap();
        (document, [CurveSpan::line(first), CurveSpan::line(second)])
    }

    fn pick(document: &SketchDocument, span: CurveSpan, parameter: f64) -> OperationAuthoringPick {
        let jet = document.evaluate_curve_jet(span, parameter).unwrap();
        OperationAuthoringPick::curve(document, span, parameter, [jet.position.x, jet.position.y])
            .unwrap()
    }

    fn crossing_lines() -> (SketchDocument, [CurveSpan; 2]) {
        let mut document = SketchDocument::new(1.0).unwrap();
        let points = [
            document.add_point("left", [-4.0, 0.0]).unwrap(),
            document.add_point("right", [4.0, 0.0]).unwrap(),
            document.add_point("bottom", [0.0, -4.0]).unwrap(),
            document.add_point("top", [0.0, 4.0]).unwrap(),
        ];
        let horizontal = document
            .add_curve(
                "horizontal",
                CurveDefinition::Line {
                    start: points[0],
                    end: points[1],
                    branch_direction: [1.0, 0.0],
                },
            )
            .unwrap();
        let vertical = document
            .add_curve(
                "vertical",
                CurveDefinition::Line {
                    start: points[2],
                    end: points[3],
                    branch_direction: [0.0, 1.0],
                },
            )
            .unwrap();
        (
            document,
            [CurveSpan::line(horizontal), CurveSpan::line(vertical)],
        )
    }

    fn open_polyline() -> (SketchDocument, [DesignPointId; 4], [CurveSpan; 3]) {
        let mut document = SketchDocument::new(1.0).unwrap();
        let points = [[0.0, 0.0], [2.0, 0.0], [2.0, 2.0], [4.0, 2.0]]
            .map(|position| document.add_point("polyline point", position).unwrap());
        let curve = document
            .add_curve(
                "polyline",
                CurveDefinition::Polyline {
                    points: points.to_vec(),
                    closed: false,
                    branch_directions: vec![[1.0, 0.0], [0.0, 1.0], [1.0, 0.0]],
                },
            )
            .unwrap();
        let spans = [0, 1, 2].map(|segment| CurveSpan { curve, segment });
        (document, points, spans)
    }

    fn preview_candidate(outcome: OperationAuthoringOutcome) -> OperationAuthoringCandidate {
        let OperationAuthoringOutcome::PreviewRequested { candidate, .. } = outcome else {
            panic!("expected preview candidate, got {outcome:?}");
        };
        candidate
    }

    fn assert_close(actual: f64, expected: f64) {
        assert!(
            (actual - expected).abs() <= 1.0e-8,
            "{actual} != {expected}"
        );
    }

    #[test]
    fn fillet_collects_local_picks_and_rearms_after_rejected_terminal_attempt() {
        let (document, spans) = perpendicular_lines();
        let mut state = OperationAuthoringState::default();
        assert!(matches!(
            state.activate(&document, OperationAuthoringTool::Fillet, &[]),
            OperationAuthoringOutcome::ModeEntered(_)
        ));
        assert!(matches!(
            state.pick(&document, pick(&document, spans[0], 0.8)),
            OperationAuthoringOutcome::Collecting { .. }
        ));
        let preview = state.pick(&document, pick(&document, spans[1], 0.2));
        let OperationAuthoringOutcome::PreviewRequested { candidate, .. } = preview else {
            panic!("expected a fillet preview request");
        };
        assert!(!candidate.is_confirmed());
        assert!(matches!(
            state.enter(),
            OperationAuthoringOutcome::Warning(OperationAuthoringWarning {
                kind: OperationAuthoringWarningKind::NoPreview,
                ..
            })
        ));
        let placed = preview_candidate(state.hover(&document, [0.03, 0.03]));
        assert!(!placed.is_confirmed());
        let confirmed = preview_candidate(state.pointer_down(&document, None, [0.03, 0.03]));
        assert!(confirmed.is_confirmed());
        assert!(matches!(state.enter(), OperationAuthoringOutcome::Apply(_)));
        state.transaction_finished();
        assert_eq!(state.active_tool(), Some(OperationAuthoringTool::Fillet));
        assert!(state.picks().is_empty());
        assert!(state.candidate().is_none());
    }

    #[test]
    fn successful_fillet_publication_exits_to_ordinary_interaction() {
        let (document, spans) = perpendicular_lines();
        let mut state = OperationAuthoringState::default();
        let remembered_options = OperationAuthoringOptions {
            fillet_radius: Some(0.1),
            fillet_radius_mode: DocumentDimensionMode::Driving,
            ..OperationAuthoringOptions::default()
        };
        assert_eq!(
            state.set_options(&document, remembered_options),
            OperationAuthoringOutcome::Inactive
        );
        let _ = state.activate(
            &document,
            OperationAuthoringTool::Fillet,
            &[
                pick(&document, spans[0], 0.8),
                pick(&document, spans[1], 0.2),
            ],
        );
        let _ = state.confirm(&document, [0.03, 0.03]);
        let remembered_after_confirmation = state.options();
        assert_eq!(
            remembered_after_confirmation.fillet_radius_mode,
            DocumentDimensionMode::Driving
        );
        assert!(matches!(state.apply(), OperationAuthoringOutcome::Apply(_)));

        assert_eq!(
            state.publication_succeeded(),
            OperationAuthoringOutcome::ModeExited
        );
        assert_eq!(state.active_tool(), None);
        assert!(state.picks().is_empty());
        assert!(state.candidate().is_none());
        assert!(!state.candidate_confirmed());
        assert_eq!(state.options(), remembered_after_confirmation);
        assert_eq!(
            state.publication_succeeded(),
            OperationAuthoringOutcome::Inactive
        );
        assert!(matches!(
            state.activate(&document, OperationAuthoringTool::Fillet, &[]),
            OperationAuthoringOutcome::ModeEntered(_)
        ));
        assert_eq!(state.options(), remembered_after_confirmation);
        assert!(state.picks().is_empty());
        assert!(state.candidate().is_none());
    }

    #[test]
    fn fillet_request_records_complete_explicit_branch_and_support_state() {
        let (document, spans) = perpendicular_lines();
        let mut state = OperationAuthoringState::default();
        let candidate = preview_candidate(state.activate(
            &document,
            OperationAuthoringTool::Fillet,
            &[
                pick(&document, spans[0], 0.8),
                pick(&document, spans[1], 0.2),
            ],
        ));
        let SketchOperationRequest::AssociativeFillet { request, .. } = candidate.request() else {
            panic!("expected associative fillet request");
        };
        assert_eq!(request.first.curve, spans[0]);
        assert_eq!(request.second.curve, spans[1]);
        assert_close(request.first.parameter, 0.975);
        assert_close(request.second.parameter, 0.025);
        assert_eq!(request.first.winding, 0);
        assert_eq!(request.second.winding, 0);
        assert_eq!(request.first.side, DocumentCurveNormalSide::Left);
        assert_eq!(request.second.side, DocumentCurveNormalSide::Left);
        assert_eq!(request.first.trim_endpoint, DocumentFilletTrimEndpoint::End);
        assert_eq!(
            request.second.trim_endpoint,
            DocumentFilletTrimEndpoint::Start
        );
        assert_eq!(request.first.periodic_anchor, None);
        assert_eq!(request.second.periodic_anchor, None);
        assert_eq!(request.first.neighborhood, ContactNeighborhood::Interior);
        assert_eq!(request.second.neighborhood, ContactNeighborhood::Interior);
        assert_eq!(
            request.endpoint_order,
            DocumentFilletEndpointOrder::FirstThenSecond
        );
        assert_eq!(request.sweep, DocumentArcSweep::CounterClockwise);
        assert_close(request.radius, 0.1);
        assert_eq!(request.radius_mode, DocumentDimensionMode::Reference);
    }

    #[test]
    fn fillet_corrections_change_the_requested_side_or_arc_choice() {
        let (document, spans) = crossing_lines();
        let picks = [
            pick(&document, spans[0], 0.25),
            pick(&document, spans[1], 0.75),
        ];
        let request = |options: OperationAuthoringOptions| {
            let mut state = OperationAuthoringState::default();
            let _ = state.set_options(&document, options);
            let candidate = preview_candidate(state.activate(
                &document,
                OperationAuthoringTool::Fillet,
                &picks,
            ));
            let SketchOperationRequest::AssociativeFillet { request, .. } = candidate.request()
            else {
                panic!("fillet request");
            };
            *request
        };
        let base = request(OperationAuthoringOptions::default());
        let first = request(OperationAuthoringOptions {
            fillet_flip_first_side: true,
            ..OperationAuthoringOptions::default()
        });
        let second = request(OperationAuthoringOptions {
            fillet_flip_second_side: true,
            ..OperationAuthoringOptions::default()
        });
        let alternate = request(OperationAuthoringOptions {
            fillet_alternate_arc: true,
            ..OperationAuthoringOptions::default()
        });
        assert_eq!(first.first.side, flip_side(base.first.side));
        assert_eq!(first.second.side, base.second.side);
        assert_eq!(second.first.side, base.first.side);
        assert_eq!(second.second.side, flip_side(base.second.side));
        assert_eq!(alternate.first.side, base.first.side);
        assert_eq!(alternate.second.side, base.second.side);
        assert_close(alternate.first.parameter, base.first.parameter);
        assert_close(alternate.second.parameter, base.second.parameter);
        assert_ne!(alternate.endpoint_order, base.endpoint_order);
        assert_eq!(alternate.sweep, base.sweep);
    }

    #[test]
    fn adjacent_polyline_spans_are_valid_fillet_parents() {
        let mut document = SketchDocument::new(1.0).unwrap();
        let points = [
            document.add_point("a", [0.0, 0.0]).unwrap(),
            document.add_point("b", [2.0, 0.0]).unwrap(),
            document.add_point("c", [2.0, 2.0]).unwrap(),
        ];
        let curve = document
            .add_curve(
                "polyline",
                CurveDefinition::Polyline {
                    points: points.to_vec(),
                    closed: false,
                    branch_directions: vec![[1.0, 0.0], [0.0, 1.0]],
                },
            )
            .unwrap();
        let spans = [
            CurveSpan { curve, segment: 0 },
            CurveSpan { curve, segment: 1 },
        ];
        let mut state = OperationAuthoringState::default();
        let _ = state.activate(&document, OperationAuthoringTool::Fillet, &[]);
        let _ = state.pick(&document, pick(&document, spans[0], 0.5));
        let candidate = preview_candidate(state.pick(&document, pick(&document, spans[1], 0.5)));
        assert!(!candidate.is_confirmed());
        let SketchOperationRequest::AssociativeFillet { request, .. } = candidate.request() else {
            panic!("fillet request");
        };
        assert_eq!(request.first.curve, spans[0]);
        assert_eq!(request.second.curve, spans[1]);
        assert_eq!(request.first.trim_endpoint, DocumentFilletTrimEndpoint::End);
        assert_eq!(
            request.second.trim_endpoint,
            DocumentFilletTrimEndpoint::Start
        );
    }

    #[test]
    fn closed_polyline_spans_do_not_use_the_open_corner_exception() {
        let mut document = SketchDocument::new(1.0).unwrap();
        let points = [[0.0, 0.0], [2.0, 0.0], [2.0, 2.0]]
            .map(|position| document.add_point("closed point", position).unwrap());
        let curve = document
            .add_curve(
                "closed polyline",
                CurveDefinition::Polyline {
                    points: points.to_vec(),
                    closed: true,
                    branch_directions: vec![
                        [1.0, 0.0],
                        [0.0, 1.0],
                        [-std::f64::consts::FRAC_1_SQRT_2; 2],
                    ],
                },
            )
            .unwrap();
        let spans = [
            CurveSpan { curve, segment: 0 },
            CurveSpan { curve, segment: 1 },
        ];
        let mut state = OperationAuthoringState::default();
        let _ = state.activate(&document, OperationAuthoringTool::Fillet, &[]);
        let _ = state.pick(&document, pick(&document, spans[0], 0.5));
        assert!(matches!(
            state.pick(&document, pick(&document, spans[1], 0.5)),
            OperationAuthoringOutcome::Warning(OperationAuthoringWarning {
                kind: OperationAuthoringWarningKind::DuplicateSupport,
                ..
            })
        ));
        assert_eq!(state.picks().len(), 1);
    }

    #[test]
    fn one_interior_polyline_point_expands_to_the_ordered_corner_but_endpoints_do_not() {
        let (document, points, spans) = open_polyline();
        let picks = resolve_operation_item_picks(
            &document,
            OperationAuthoringTool::Fillet,
            SelectionItem::Point(points[1]),
            None,
        )
        .unwrap();
        assert_eq!(picks.len(), 2);
        assert_eq!(picks[0].curve_span(), Some(spans[0]));
        assert_eq!(picks[1].curve_span(), Some(spans[1]));
        assert_eq!(
            picks[0].fillet_trim_endpoint_hint,
            Some(DocumentFilletTrimEndpoint::End)
        );
        assert_eq!(
            picks[1].fillet_trim_endpoint_hint,
            Some(DocumentFilletTrimEndpoint::Start)
        );

        let mut state = OperationAuthoringState::default();
        let candidate = preview_candidate(state.activate_items(
            &document,
            OperationAuthoringTool::Fillet,
            &[(SelectionItem::Point(points[1]), None)],
        ));
        assert!(!candidate.is_confirmed());
        let SketchOperationRequest::AssociativeFillet { request, .. } = candidate.request() else {
            panic!("corner fillet request");
        };
        assert_eq!(request.first.curve, spans[0]);
        assert_eq!(request.second.curve, spans[1]);

        assert_eq!(
            resolve_operation_item_picks(
                &document,
                OperationAuthoringTool::Fillet,
                SelectionItem::Point(points[0]),
                None,
            ),
            Err(OperationAuthoringWarningKind::FilletCornerNotInterior)
        );
    }

    #[test]
    fn one_corner_item_cannot_partially_overwrite_an_existing_fillet_prefix() {
        let (mut document, points, _) = open_polyline();
        let other_start = document.add_point("other start", [-3.0, 4.0]).unwrap();
        let other_end = document.add_point("other end", [3.0, 4.0]).unwrap();
        let other = CurveSpan::line(
            document
                .add_curve(
                    "other support",
                    CurveDefinition::Line {
                        start: other_start,
                        end: other_end,
                        branch_direction: [1.0, 0.0],
                    },
                )
                .unwrap(),
        );
        let corner_picks = resolve_operation_item_picks(
            &document,
            OperationAuthoringTool::Fillet,
            SelectionItem::Point(points[1]),
            None,
        )
        .unwrap();
        let first = pick(&document, other, 0.5);
        let mut state = OperationAuthoringState::default();
        let _ = state.activate(&document, OperationAuthoringTool::Fillet, &[]);
        let _ = state.pick(&document, first.clone());
        assert!(matches!(
            state.pick_many(&document, corner_picks),
            OperationAuthoringOutcome::Warning(OperationAuthoringWarning {
                kind: OperationAuthoringWarningKind::WrongArity,
                ..
            })
        ));
        assert_eq!(state.picks(), &[first]);
        assert!(state.candidate().is_none());
    }

    #[test]
    fn shared_corner_ambiguity_is_typed_and_does_not_guess_an_owner() {
        let (mut document, points, _) = open_polyline();
        let extra_start = document.add_point("extra start", [-2.0, 0.0]).unwrap();
        let extra_end = document.add_point("extra end", [2.0, -2.0]).unwrap();
        document
            .add_curve(
                "second polyline owner",
                CurveDefinition::Polyline {
                    points: vec![extra_start, points[1], extra_end],
                    closed: false,
                    branch_directions: vec![[1.0, 0.0], [0.0, -1.0]],
                },
            )
            .unwrap();
        assert_eq!(
            resolve_operation_item_picks(
                &document,
                OperationAuthoringTool::Fillet,
                SelectionItem::Point(points[1]),
                None,
            ),
            Err(OperationAuthoringWarningKind::AmbiguousFilletCorner)
        );
    }

    #[test]
    fn fillet_pointer_placement_changes_radius_and_reference_is_default_unless_locked() {
        let (document, spans) = perpendicular_lines();
        let selected = [
            pick(&document, spans[0], 0.8),
            pick(&document, spans[1], 0.2),
        ];
        let mut state = OperationAuthoringState::default();
        let initial =
            preview_candidate(state.activate(&document, OperationAuthoringTool::Fillet, &selected));
        assert!(!initial.is_confirmed());
        let SketchOperationRequest::AssociativeFillet {
            request: initial_request,
            ..
        } = initial.request()
        else {
            panic!("initial fillet request");
        };
        assert_close(initial_request.radius, 0.1);
        assert_eq!(
            initial_request.radius_mode,
            DocumentDimensionMode::Reference
        );

        let placed = preview_candidate(state.hover(&document, [0.06, 0.06]));
        let SketchOperationRequest::AssociativeFillet {
            request: placed_request,
            ..
        } = placed.request()
        else {
            panic!("placed fillet request");
        };
        assert!(placed_request.radius > initial_request.radius);
        assert_eq!(placed_request.radius_mode, DocumentDimensionMode::Reference);
        let confirmed = preview_candidate(state.confirm(&document, [0.06, 0.06]));
        assert!(confirmed.is_confirmed());
        assert_eq!(confirmed.request(), placed.request());

        let mut locked = OperationAuthoringState::default();
        let _ = locked.set_options(
            &document,
            OperationAuthoringOptions {
                fillet_radius_mode: DocumentDimensionMode::Driving,
                ..OperationAuthoringOptions::default()
            },
        );
        let _ = locked.activate(&document, OperationAuthoringTool::Fillet, &selected);
        let locked = preview_candidate(locked.confirm(&document, [0.03, 0.03]));
        let SketchOperationRequest::AssociativeFillet { request, .. } = locked.request() else {
            panic!("locked fillet request");
        };
        assert_eq!(request.radius_mode, DocumentDimensionMode::Driving);
    }

    #[test]
    fn stale_reconcile_clears_every_operand_instead_of_shifting_roles() {
        let (mut document, spans) = crossing_lines();
        let first_pick = pick(&document, spans[0], 0.25);
        let second_pick = pick(&document, spans[1], 0.75);
        let mut state = OperationAuthoringState::default();
        let _ = state.activate(
            &document,
            OperationAuthoringTool::Fillet,
            &[first_pick, second_pick],
        );
        let CurveDefinition::Line { start, .. } =
            document.curve(spans[0].curve).unwrap().definition
        else {
            unreachable!();
        };
        document.set_point_position(start, [-5.0, 1.0]).unwrap();
        assert!(matches!(
            state.reconcile(&document),
            OperationAuthoringOutcome::Warning(OperationAuthoringWarning {
                kind: OperationAuthoringWarningKind::StalePick,
                ..
            })
        ));
        assert!(state.picks().is_empty());
        assert!(state.candidate().is_none());
        assert_eq!(state.active_tool(), Some(OperationAuthoringTool::Fillet));
    }

    #[test]
    fn synthesis_failure_retains_operands_for_radius_or_branch_correction() {
        let mut document = SketchDocument::new(1.0).unwrap();
        let points = [
            document.add_point("a", [-2.0, 0.0]).unwrap(),
            document.add_point("b", [2.0, 0.0]).unwrap(),
            document.add_point("c", [-2.0, 1.0]).unwrap(),
            document.add_point("d", [2.0, 1.0]).unwrap(),
        ];
        let spans = [
            CurveSpan::line(
                document
                    .add_curve(
                        "first",
                        CurveDefinition::Line {
                            start: points[0],
                            end: points[1],
                            branch_direction: [1.0, 0.0],
                        },
                    )
                    .unwrap(),
            ),
            CurveSpan::line(
                document
                    .add_curve(
                        "second",
                        CurveDefinition::Line {
                            start: points[2],
                            end: points[3],
                            branch_direction: [1.0, 0.0],
                        },
                    )
                    .unwrap(),
            ),
        ];
        let mut state = OperationAuthoringState::default();
        let _ = state.activate(&document, OperationAuthoringTool::Fillet, &[]);
        let first = pick(&document, spans[0], 0.5);
        let _ = state.pick(&document, first.clone());
        assert!(matches!(
            state.pick(&document, first.clone()),
            OperationAuthoringOutcome::Warning(OperationAuthoringWarning {
                kind: OperationAuthoringWarningKind::DuplicateSupport,
                ..
            })
        ));
        assert_eq!(state.picks(), std::slice::from_ref(&first));
        assert!(matches!(
            state.pick(&document, pick(&document, spans[1], 0.5)),
            OperationAuthoringOutcome::Warning(OperationAuthoringWarning {
                kind: OperationAuthoringWarningKind::SingularFillet,
                ..
            })
        ));
        assert_eq!(state.picks(), &[first, pick(&document, spans[1], 0.5),]);
        assert!(state.candidate().is_none());
    }

    #[test]
    fn periodic_parent_state_and_trim_ambiguity_are_explicit() {
        let mut document = SketchDocument::new(2.0).unwrap();
        let center = document.add_point("center", [0.0, 0.0]).unwrap();
        let radius = document
            .add_scalar(
                "radius",
                1.0,
                geosolve_sketch::ScalarUnit::Length,
                geosolve_sketch::ScalarDomain::Positive,
            )
            .unwrap();
        let span = CurveSpan::line(
            document
                .add_curve("circle", CurveDefinition::Circle { center, radius })
                .unwrap(),
        );
        let contact = std::f64::consts::TAU + 0.5;
        let root_cell = ContactNeighborhood::Local {
            lower: contact - 1.0,
            upper: contact + 1.0,
        };
        let retained_pick = pick(&document, span, std::f64::consts::TAU + 2.0);
        let parent = fillet_parent(
            &document,
            &retained_pick,
            contact,
            DocumentCurveNormalSide::Left,
            root_cell,
        )
        .unwrap();
        assert_close(parent.parameter, 0.5);
        assert_eq!(parent.winding, 1);
        assert_eq!(parent.trim_endpoint, DocumentFilletTrimEndpoint::Start);
        let anchor = parent.periodic_anchor.expect("periodic trim anchor");
        assert_close(anchor.parameter, std::f64::consts::PI + 0.5);
        assert_eq!(anchor.winding, 1);
        let ContactNeighborhood::Local { lower, upper } = parent.neighborhood else {
            panic!("periodic parent retains its explicit local root interval");
        };
        assert_close(lower, contact - 1.0);
        assert_close(upper, contact + 1.0);

        let discarded_start_pick = pick(&document, span, contact - 1.0);
        let end_parent = fillet_parent(
            &document,
            &discarded_start_pick,
            contact,
            DocumentCurveNormalSide::Left,
            root_cell,
        )
        .unwrap();
        assert_eq!(end_parent.trim_endpoint, DocumentFilletTrimEndpoint::End);
        let end_anchor = end_parent.periodic_anchor.expect("periodic trim anchor");
        assert_close(end_anchor.parameter, 0.5 + std::f64::consts::PI);
        assert_eq!(end_anchor.winding, 0);
        let ContactNeighborhood::Local {
            lower: end_lower,
            upper: end_upper,
        } = end_parent.neighborhood
        else {
            panic!("periodic end parent retains its explicit local root interval");
        };
        assert_close(end_lower, contact - 1.0);
        assert_close(end_upper, contact + 1.0);
        let ambiguous_pick = pick(&document, span, contact);
        assert!(matches!(
            fillet_parent(
                &document,
                &ambiguous_pick,
                contact,
                DocumentCurveNormalSide::Left,
                root_cell,
            ),
            Err((OperationAuthoringWarningKind::AmbiguousTrimSide, _))
        ));
    }

    #[test]
    fn curved_parent_root_cells_exclude_a_remote_same_branch_bezier_root() {
        let mut document = SketchDocument::new(8.0).unwrap();
        let line_points = [[-10.0, 0.0], [10.0, 0.0]]
            .map(|position| document.add_point("line point", position).unwrap());
        let line = CurveSpan::line(
            document
                .add_curve(
                    "line parent",
                    CurveDefinition::Line {
                        start: line_points[0],
                        end: line_points[1],
                        branch_direction: [1.0, 0.0],
                    },
                )
                .unwrap(),
        );
        let controls = [[-6.0, 1.0], [-1.0, 8.0], [1.0, 8.0], [6.0, 1.0]]
            .map(|position| document.add_point("Bezier control", position).unwrap());
        let bezier = CurveSpan::line(
            document
                .add_curve(
                    "symmetric multi-root Bezier",
                    CurveDefinition::CubicBezier { controls },
                )
                .unwrap(),
        );

        let roots = [0.361_804_407_5, 0.638_195_592_5];
        let radius = 3.108_415_523_5;
        let expected_center = [0.0, radius];
        let line_center =
            offset_curve_point(&document, line, 0.5, DocumentCurveNormalSide::Left, radius)
                .unwrap();
        assert!((line_center[0] - expected_center[0]).abs() <= 1.0e-9);
        assert!((line_center[1] - expected_center[1]).abs() <= 1.0e-9);
        for root in roots {
            let center = offset_curve_point(
                &document,
                bezier,
                root,
                DocumentCurveNormalSide::Right,
                radius,
            )
            .unwrap();
            assert!((center[0] - expected_center[0]).abs() <= 1.0e-7);
            assert!((center[1] - expected_center[1]).abs() <= 1.0e-7);
        }

        let first_neighborhood =
            certified_curved_fillet_neighborhood(&document, line, bezier, roots[0]).unwrap();
        let first = fillet_parent(
            &document,
            &pick(&document, bezier, 0.15),
            roots[0],
            DocumentCurveNormalSide::Right,
            first_neighborhood,
        )
        .unwrap();
        let ContactNeighborhood::Local {
            lower: first_lower,
            upper: first_upper,
        } = first.neighborhood
        else {
            panic!("a bounded nonlinear parent must retain a local root cell");
        };
        assert!(first_lower < roots[0] && roots[0] < first_upper);
        assert!(roots[1] >= first_upper);

        let second_neighborhood =
            certified_curved_fillet_neighborhood(&document, line, bezier, roots[1]).unwrap();
        let second = fillet_parent(
            &document,
            &pick(&document, bezier, 0.85),
            roots[1],
            DocumentCurveNormalSide::Right,
            second_neighborhood,
        )
        .unwrap();
        let ContactNeighborhood::Local {
            lower: second_lower,
            upper: second_upper,
        } = second.neighborhood
        else {
            panic!("the alternate curved root must have its own local cell");
        };
        assert!(second_lower < roots[1] && roots[1] < second_upper);
        assert!(roots[0] <= second_lower);

        let line_parent = fillet_parent(
            &document,
            &pick(&document, line, 0.25),
            0.5,
            DocumentCurveNormalSide::Left,
            ContactNeighborhood::Interior,
        )
        .unwrap();
        assert_eq!(line_parent.neighborhood, ContactNeighborhood::Interior);
    }

    #[test]
    fn near_tied_materially_distinct_roots_are_typed_ambiguous() {
        let document = SketchDocument::new(1.0).unwrap();
        let solutions = [
            LocalFilletSolution {
                parameters: [0.2, 0.3],
                sides: [DocumentCurveNormalSide::Left, DocumentCurveNormalSide::Left],
                center: [0.0, 0.0],
                score: 0.5,
            },
            LocalFilletSolution {
                parameters: [0.8, 0.7],
                sides: [
                    DocumentCurveNormalSide::Right,
                    DocumentCurveNormalSide::Right,
                ],
                center: [1.0, 1.0],
                score: 0.5 + 1.0e-10,
            },
        ];
        assert!(matches!(
            select_local_fillet_solution(&document, &solutions),
            Err((OperationAuthoringWarningKind::AmbiguousFilletRoot, _))
        ));
    }

    #[test]
    fn warnings_cannot_leave_an_unbacked_apply_ready_candidate() {
        let (document, spans) = crossing_lines();
        let mut state = OperationAuthoringState::default();
        let _ = state.activate(
            &document,
            OperationAuthoringTool::Fillet,
            &[
                pick(&document, spans[0], 0.25),
                pick(&document, spans[1], 0.75),
            ],
        );
        let _ = state.confirm(&document, [-0.1, 0.1]);
        assert!(state.candidate_confirmed());
        assert!(matches!(
            state.pick(&document, pick(&document, spans[0], 0.75)),
            OperationAuthoringOutcome::Warning(_)
        ));
        assert!(state.candidate().is_none());
        assert!(matches!(
            state.apply(),
            OperationAuthoringOutcome::Warning(OperationAuthoringWarning {
                kind: OperationAuthoringWarningKind::NoPreview,
                ..
            })
        ));

        let _ = state.activate(
            &document,
            OperationAuthoringTool::Fillet,
            &[
                pick(&document, spans[0], 0.25),
                pick(&document, spans[1], 0.75),
            ],
        );
        let _ = state.confirm(&document, [-0.1, 0.1]);
        let invalid = OperationAuthoringOptions {
            fillet_radius: Some(-1.0),
            ..state.options()
        };
        assert!(matches!(
            state.set_options(&document, invalid),
            OperationAuthoringOutcome::Warning(OperationAuthoringWarning {
                kind: OperationAuthoringWarningKind::NonFinitePick,
                ..
            })
        ));
        assert!(state.candidate().is_none());
        assert_eq!(state.picks().len(), 2);
        assert_eq!(
            state.guidance().stage,
            OperationAuthoringStage::PlaceFilletRadius
        );
    }

    #[test]
    fn failed_preview_retains_unconfirmed_fillet_parents_but_confirmed_failure_is_terminal() {
        let (document, spans) = crossing_lines();
        let picks = [
            pick(&document, spans[0], 0.25),
            pick(&document, spans[1], 0.75),
        ];
        let mut state = OperationAuthoringState::default();
        let options = OperationAuthoringOptions {
            fillet_radius: Some(0.25),
            ..OperationAuthoringOptions::default()
        };
        let _ = state.set_options(&document, options);
        let initial =
            preview_candidate(state.activate(&document, OperationAuthoringTool::Fillet, &picks));
        assert!(!initial.is_confirmed());
        assert!(matches!(
            state.hover(&document, [f64::NAN, 0.0]),
            OperationAuthoringOutcome::Warning(OperationAuthoringWarning {
                kind: OperationAuthoringWarningKind::NonFinitePick,
                ..
            })
        ));
        assert_eq!(state.picks(), &picks);
        assert!(state.candidate().is_none());
        assert!(!state.candidate_confirmed());
        assert_eq!(
            state.guidance().stage,
            OperationAuthoringStage::PlaceFilletRadius
        );

        // Coordinator-level scratch cleanup is idempotent after the authoring
        // state has already discarded the invalid pointer-derived candidate.
        state.preview_failed();
        assert_eq!(state.picks(), &picks);
        assert!(state.candidate().is_none());
        assert!(!state.candidate_confirmed());
        assert_eq!(
            state.guidance().stage,
            OperationAuthoringStage::PlaceFilletRadius
        );

        let recovered = preview_candidate(state.hover(&document, [-0.1, 0.1]));
        assert!(!recovered.is_confirmed());
        let confirmed = preview_candidate(state.confirm(&document, [-0.1, 0.1]));
        assert!(confirmed.is_confirmed());
        let placed_radius = state.options().fillet_radius;
        state.preview_failed();
        assert!(state.picks().is_empty());
        assert!(state.candidate().is_none());
        assert!(!state.candidate_confirmed());
        assert_eq!(
            state.guidance().stage,
            OperationAuthoringStage::PickFirstFilletCurve
        );
        assert_eq!(state.options().fillet_radius, placed_radius);
    }

    #[test]
    fn confirmed_fillet_hover_preserves_preview_and_exact_input_reconcile_rearms_stale_state() {
        let (document, spans) = crossing_lines();
        let mut session = RetainedSketchDocumentSession::new(
            document,
            DocumentSolveRequest::default(),
            SolverConfig::default(),
        )
        .unwrap();
        let accepted = session.accepted_state().unwrap().document().clone();
        let input = session.prepared_input();
        let picks = [
            pick(&accepted, spans[0], 0.25).bind_input(&input),
            pick(&accepted, spans[1], 0.75).bind_input(&input),
        ];
        let mut state = OperationAuthoringState::default();
        let _ = state.activate(&accepted, OperationAuthoringTool::Fillet, &picks);
        let confirmed = preview_candidate(state.confirm(&accepted, [-0.1, 0.1]));
        let hovered = preview_candidate(state.hover(&accepted, [100.0, -100.0]));
        assert_eq!(hovered, confirmed);

        session
            .reattempt(
                session.design_identity(),
                session.last_attempt().input().candidate_request(),
            )
            .unwrap();
        assert!(matches!(
            state.reconcile_exact_input(
                session.accepted_state().unwrap().document(),
                session.prepared_input(),
            ),
            OperationAuthoringOutcome::Warning(OperationAuthoringWarning {
                kind: OperationAuthoringWarningKind::StalePick,
                ..
            })
        ));
        assert!(state.picks().is_empty());
        assert!(state.candidate().is_none());
        assert_eq!(state.active_tool(), Some(OperationAuthoringTool::Fillet));
    }

    #[test]
    fn two_curved_parents_fail_typed_until_pairwise_continuation_is_certified() {
        let mut document = SketchDocument::new(1.0).unwrap();
        let centers = [
            document.add_point("left center", [-1.0, 0.0]).unwrap(),
            document.add_point("right center", [1.0, 0.0]).unwrap(),
        ];
        let radii = ["left radius", "right radius"].map(|label| {
            document
                .add_scalar(
                    label,
                    1.0,
                    geosolve_sketch::ScalarUnit::Length,
                    geosolve_sketch::ScalarDomain::Positive,
                )
                .unwrap()
        });
        let spans = [0, 1].map(|index| {
            CurveSpan::line(
                document
                    .add_curve(
                        "circle",
                        CurveDefinition::Circle {
                            center: centers[index],
                            radius: radii[index],
                        },
                    )
                    .unwrap(),
            )
        });
        let picks = spans.map(|span| pick(&document, span, std::f64::consts::FRAC_PI_2));
        let mut state = OperationAuthoringState::default();
        assert!(matches!(
            state.activate(&document, OperationAuthoringTool::Fillet, &picks),
            OperationAuthoringOutcome::Warning(OperationAuthoringWarning {
                kind: OperationAuthoringWarningKind::UnsupportedFilletPair,
                ..
            })
        ));
        assert!(state.candidate().is_none());
        assert_eq!(state.picks(), &picks);
    }

    #[test]
    fn osculating_radius_offset_singularity_is_typed_before_candidate_publication() {
        assert!(matches!(
            validate_fillet_offset_regular(1.0, DocumentCurveNormalSide::Left, 1.0),
            Err((OperationAuthoringWarningKind::SingularFillet, _))
        ));
        assert!(validate_fillet_offset_regular(1.0, DocumentCurveNormalSide::Right, 1.0).is_ok());
    }

    #[test]
    fn existing_trim_ownership_rejects_without_losing_the_explicit_operands() {
        let (mut document, spans) = perpendicular_lines();
        document
            .replace_trim_views(
                spans[0],
                vec![geosolve_sketch::DocumentCurveTrimView {
                    support: spans[0],
                    start: geosolve_sketch::DocumentTrimBoundary::Fixed(DocumentTrimParameter {
                        parameter: 0.0,
                        winding: 0,
                    }),
                    end: geosolve_sketch::DocumentTrimBoundary::Fixed(DocumentTrimParameter {
                        parameter: 0.9,
                        winding: 0,
                    }),
                }],
            )
            .unwrap();
        let mut state = OperationAuthoringState::default();
        assert!(matches!(
            state.activate(
                &document,
                OperationAuthoringTool::Fillet,
                &[
                    pick(&document, spans[0], 0.8),
                    pick(&document, spans[1], 0.2),
                ],
            ),
            OperationAuthoringOutcome::Warning(OperationAuthoringWarning {
                kind: OperationAuthoringWarningKind::AlreadyTrimmed,
                ..
            })
        ));
        assert_eq!(state.picks().len(), 2);
        assert!(state.candidate().is_none());
    }
}
