// SPDX-License-Identifier: GPL-3.0-or-later

//! Selection-independent CAD relation and dimension authoring.

use geosolve_sketch::{
    CurveDefinition, DocumentAngleOrientation, DocumentCurveContinuity,
    DocumentCurveCurvatureRelation, DocumentDimensionMode, SketchDocument, TangentOrientation,
};

use crate::coordinator::{
    line_endpoints, resolve_constraint, selection_exists, validate_dimension_selection,
};
use crate::{
    ConstraintIntent, DimensionKind, DisabledReason, EditorScene, GeometryInteractionPolicy,
    PickTolerance, ResolvedConstraintKind, ScreenPoint, SelectionItem,
};

const MAX_CONSTRAINT_AUTHORING_HIT_CANDIDATES: usize = 64;

/// One palette tool owned by the reusable authoring state machine.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuthoringTool {
    Constraint(ConstraintIntent),
    Dimension(DimensionKind),
}

impl AuthoringTool {
    /// Maximum number of operands collected before the tool applies.
    ///
    /// Horizontal and Vertical are variable-arity: one affine span is already
    /// complete, while one stored point is a compatible prefix for a second.
    #[must_use]
    pub const fn arity(self) -> usize {
        match self {
            Self::Constraint(ConstraintIntent::Lock)
            | Self::Dimension(
                DimensionKind::SegmentLength | DimensionKind::Radius | DimensionKind::Diameter,
            ) => 1,
            Self::Constraint(ConstraintIntent::Symmetric) => 3,
            Self::Constraint(
                ConstraintIntent::Coincident
                | ConstraintIntent::Parallel
                | ConstraintIntent::Perpendicular
                | ConstraintIntent::Equal
                | ConstraintIntent::Midpoint
                | ConstraintIntent::Tangent
                | ConstraintIntent::Continuity
                | ConstraintIntent::Horizontal
                | ConstraintIntent::Vertical
                | ConstraintIntent::Concentric
                | ConstraintIntent::Collinear,
            )
            | Self::Dimension(DimensionKind::PointDistance | DimensionKind::OrientedAngle) => 2,
        }
    }
}

/// One explicit host-supplied operand. Curve parameters come from the actual pick,
/// not from application selection or a coordinate heuristic.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AuthoringOperand {
    pub item: SelectionItem,
    pub curve_parameter: Option<f64>,
}

impl AuthoringOperand {
    #[must_use]
    pub const fn selected(item: SelectionItem) -> Self {
        Self {
            item,
            curve_parameter: None,
        }
    }

    #[must_use]
    pub const fn picked(item: SelectionItem, curve_parameter: Option<f64>) -> Self {
        Self {
            item,
            curve_parameter,
        }
    }
}

/// Presentation-neutral kind accepted for the next authoring operand.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuthoringOperandKind {
    Point,
    Curve,
    Line,
    CircleOrArc,
    Datum,
    DatumAxis,
}

impl AuthoringOperandKind {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Point => "point",
            Self::Curve => "curve",
            Self::Line => "line",
            Self::CircleOrArc => "circle or arc",
            Self::Datum => "reference datum",
            Self::DatumAxis => "reference axis",
        }
    }
}

/// Explicit non-persisted choices used for the next authoring transaction.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AuthoringOptions {
    pub tangent_orientation: TangentOrientation,
    pub curvature_relation: DocumentCurveCurvatureRelation,
    pub continuity: DocumentCurveContinuity,
    pub dimension_mode: DocumentDimensionMode,
    pub angle_orientation: DocumentAngleOrientation,
}

impl Default for AuthoringOptions {
    fn default() -> Self {
        Self {
            tangent_orientation: TangentOrientation::Aligned,
            curvature_relation: DocumentCurveCurvatureRelation::Signed,
            continuity: DocumentCurveContinuity::G1,
            dimension_mode: DocumentDimensionMode::Driving,
            angle_orientation: DocumentAngleOrientation::CounterClockwise,
        }
    }
}

/// Typed, stable incompatibility information. No mutation accompanies a warning.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthoringWarning {
    pub reason: DisabledReason,
    pub expected: Vec<AuthoringOperandKind>,
    pub message: String,
}

/// A complete immutable application request produced by the authoring state.
#[derive(Clone, Debug, PartialEq)]
pub struct AuthoringApplication {
    pub tool: AuthoringTool,
    pub operands: Vec<AuthoringOperand>,
    pub options: AuthoringOptions,
    pub resolved_constraint: Option<ResolvedConstraintKind>,
}

/// Result of activating, picking, cancelling or reconciling authoring state.
#[derive(Clone, Debug, PartialEq)]
pub enum AuthoringOutcome {
    ModeEntered {
        tool: AuthoringTool,
        expected: Vec<AuthoringOperandKind>,
    },
    Collecting {
        tool: AuthoringTool,
        operands: Vec<AuthoringOperand>,
        expected: Vec<AuthoringOperandKind>,
    },
    Apply(AuthoringApplication),
    Warning(AuthoringWarning),
    PendingCleared {
        tool: AuthoringTool,
        expected: Vec<AuthoringOperandKind>,
    },
    ModeExited,
    Inactive,
}

/// Reusable operand collector. It deliberately contains no application selection.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AuthoringState {
    active: Option<AuthoringTool>,
    pending: Vec<AuthoringOperand>,
    options: AuthoringOptions,
}

enum AuthoringPickResolution {
    Accepted {
        state: AuthoringState,
        outcome: AuthoringOutcome,
        item: SelectionItem,
    },
    Rejected(AuthoringOutcome),
}

impl AuthoringState {
    #[must_use]
    pub const fn active_tool(&self) -> Option<AuthoringTool> {
        self.active
    }

    #[must_use]
    pub fn pending(&self) -> &[AuthoringOperand] {
        &self.pending
    }

    #[must_use]
    pub const fn options(&self) -> AuthoringOptions {
        self.options
    }

    pub fn set_options(&mut self, options: AuthoringOptions) {
        self.options = options;
    }

    /// Leaves authoring immediately while preserving remembered options.
    pub fn deactivate(&mut self) {
        self.active = None;
        self.pending.clear();
    }

    /// Activates a tool from an immutable host selection snapshot.
    ///
    /// Empty selection enters repeated mode. A complete compatible selection emits
    /// one application without changing mode. Incompatible selection emits only a
    /// typed warning.
    #[must_use]
    pub fn activate(
        &mut self,
        document: &SketchDocument,
        tool: AuthoringTool,
        selection: &[AuthoringOperand],
    ) -> AuthoringOutcome {
        if selection.is_empty() {
            self.active = Some(tool);
            self.pending.clear();
            return AuthoringOutcome::ModeEntered {
                tool,
                expected: expected_operands(document, tool, &[]),
            };
        }
        if !selection_is_complete(document, tool, selection)
            && selection.len() < tool.arity()
            && prefix_is_compatible(document, tool, selection)
        {
            self.active = Some(tool);
            self.pending = selection.to_vec();
            return AuthoringOutcome::Collecting {
                tool,
                operands: self.pending.clone(),
                expected: expected_operands(document, tool, &self.pending),
            };
        }
        match application(document, tool, selection, self.options) {
            Ok(application) => AuthoringOutcome::Apply(application),
            Err(warning) => AuthoringOutcome::Warning(warning),
        }
    }

    /// Adds one picked operand to the active repeated tool.
    #[must_use]
    pub fn pick(
        &mut self,
        document: &SketchDocument,
        operand: AuthoringOperand,
    ) -> AuthoringOutcome {
        let Some(tool) = self.active else {
            return AuthoringOutcome::Inactive;
        };
        if !selection_exists(document, operand.item) {
            return AuthoringOutcome::Warning(warning(
                document,
                tool,
                &self.pending,
                DisabledReason::MissingObject,
            ));
        }
        let mut candidate = self.pending.clone();
        candidate.push(operand);
        if selection_is_complete(document, tool, &candidate) {
            return match application(document, tool, &candidate, self.options) {
                Ok(application) => {
                    self.pending.clone_from(&application.operands);
                    AuthoringOutcome::Apply(application)
                }
                Err(warning) => AuthoringOutcome::Warning(warning),
            };
        }
        if let AuthoringTool::Constraint(intent) = tool {
            let selection = candidate
                .iter()
                .map(|operand| operand.item)
                .collect::<Vec<_>>();
            if let Err(reason) = resolve_constraint(document, &selection, intent)
                && (candidate.len() >= tool.arity()
                    || reason == DisabledReason::SameSemanticOperand)
            {
                return AuthoringOutcome::Warning(warning(document, tool, &self.pending, reason));
            }
        }
        if candidate.len() < tool.arity() {
            if !prefix_is_compatible(document, tool, &candidate) {
                return AuthoringOutcome::Warning(warning(
                    document,
                    tool,
                    &self.pending,
                    DisabledReason::WrongOperandKind,
                ));
            }
            self.pending = candidate;
            return AuthoringOutcome::Collecting {
                tool,
                operands: self.pending.clone(),
                expected: expected_operands(document, tool, &self.pending),
            };
        }
        if candidate.len() > tool.arity() {
            return AuthoringOutcome::Warning(warning(
                document,
                tool,
                &self.pending,
                DisabledReason::WrongArity,
            ));
        }
        AuthoringOutcome::Warning(warning(
            document,
            tool,
            &self.pending,
            DisabledReason::WrongOperandKind,
        ))
    }

    /// Resolves one screen click through bounded compatibility-aware native
    /// candidates under the supplied geometry policy. An incompatible point or
    /// nearer support cannot mask a valid operand underneath the same click.
    #[must_use]
    pub fn pick_at_with_policy(
        &mut self,
        document: &SketchDocument,
        scene: &EditorScene,
        position: ScreenPoint,
        tolerance: PickTolerance,
        policy: GeometryInteractionPolicy,
    ) -> AuthoringOutcome {
        match self.resolve_pick_at_with_policy(document, scene, position, tolerance, policy) {
            AuthoringPickResolution::Accepted { state, outcome, .. } => {
                *self = state;
                outcome
            }
            AuthoringPickResolution::Rejected(outcome) => outcome,
        }
    }

    /// Resolves the exact semantic item that an unchanged canvas press would
    /// accept next, without mutating authoring state.
    pub(crate) fn hover_item_at_with_policy(
        &self,
        document: &SketchDocument,
        scene: &EditorScene,
        position: ScreenPoint,
        tolerance: PickTolerance,
        policy: GeometryInteractionPolicy,
    ) -> Option<SelectionItem> {
        match self.resolve_pick_at_with_policy(document, scene, position, tolerance, policy) {
            AuthoringPickResolution::Accepted { item, .. } => Some(item),
            AuthoringPickResolution::Rejected(_) => None,
        }
    }

    fn resolve_pick_at_with_policy(
        &self,
        document: &SketchDocument,
        scene: &EditorScene,
        position: ScreenPoint,
        tolerance: PickTolerance,
        policy: GeometryInteractionPolicy,
    ) -> AuthoringPickResolution {
        let Some(tool) = self.active else {
            return AuthoringPickResolution::Rejected(AuthoringOutcome::Inactive);
        };
        let hits = match scene.native_authoring_hit_candidates_with_policy(
            position,
            tolerance,
            MAX_CONSTRAINT_AUTHORING_HIT_CANDIDATES,
            policy,
        ) {
            Ok(hits) => hits,
            Err(crate::NativeAuthoringHitError::CandidateLimitExceeded { .. }) => {
                return AuthoringPickResolution::Rejected(AuthoringOutcome::Warning(
                    AuthoringWarning {
                        reason: DisabledReason::WrongOperandKind,
                        expected: expected_operands(document, tool, &self.pending),
                        message: "too many overlapping authoring candidates under this click"
                            .into(),
                    },
                ));
            }
        };
        let mut first_warning = None;
        let mut first_collecting = None;
        for hit in hits {
            let item = hit.item;
            let mut trial = self.clone();
            let outcome = trial.pick(
                document,
                AuthoringOperand::picked(hit.item, hit.curve_parameter),
            );
            match outcome {
                AuthoringOutcome::Apply(_) => {
                    return AuthoringPickResolution::Accepted {
                        state: trial,
                        outcome,
                        item,
                    };
                }
                AuthoringOutcome::Collecting { .. } => {
                    // A newly admissible variable-arity prefix (for example a
                    // stored point under Horizontal/Vertical) must not mask a
                    // complete compatible operand painted under the same
                    // click. Retain the first valid prefix only if no later
                    // hit can apply immediately.
                    if first_collecting.is_none() {
                        first_collecting = Some((trial, outcome, item));
                    }
                }
                AuthoringOutcome::Warning(value) => {
                    first_warning.get_or_insert(value);
                }
                AuthoringOutcome::ModeEntered { .. }
                | AuthoringOutcome::PendingCleared { .. }
                | AuthoringOutcome::ModeExited
                | AuthoringOutcome::Inactive => {}
            }
        }
        if let Some((state, outcome, item)) = first_collecting {
            return AuthoringPickResolution::Accepted {
                state,
                outcome,
                item,
            };
        }
        AuthoringPickResolution::Rejected(AuthoringOutcome::Warning(first_warning.unwrap_or_else(
            || {
                warning(
                    document,
                    tool,
                    &self.pending,
                    DisabledReason::WrongOperandKind,
                )
            },
        )))
    }

    /// Clears a terminal application attempt while retaining the repeated tool.
    ///
    /// Hosts call this after processing [`AuthoringOutcome::Apply`], whether the
    /// coordinator accepted, retained-rejected or refused that application. A
    /// terminal candidate must never leave repeated authoring wedged at full arity.
    pub fn transaction_finished(&mut self) {
        self.pending.clear();
    }

    /// First Escape clears operands; a subsequent Escape exits mode.
    #[must_use]
    pub fn cancel(&mut self, document: &SketchDocument) -> AuthoringOutcome {
        let Some(tool) = self.active else {
            return AuthoringOutcome::Inactive;
        };
        if self.pending.is_empty() {
            self.active = None;
            AuthoringOutcome::ModeExited
        } else {
            self.pending.clear();
            AuthoringOutcome::PendingCleared {
                tool,
                expected: expected_operands(document, tool, &[]),
            }
        }
    }

    /// Removes operands whose persistent identities no longer exist.
    #[must_use]
    pub fn reconcile(&mut self, document: &SketchDocument) -> AuthoringOutcome {
        let Some(tool) = self.active else {
            self.pending.clear();
            return AuthoringOutcome::Inactive;
        };
        self.pending
            .retain(|operand| selection_exists(document, operand.item));
        AuthoringOutcome::Collecting {
            tool,
            operands: self.pending.clone(),
            expected: expected_operands(document, tool, &self.pending),
        }
    }
}

fn application(
    document: &SketchDocument,
    tool: AuthoringTool,
    operands: &[AuthoringOperand],
    options: AuthoringOptions,
) -> Result<AuthoringApplication, AuthoringWarning> {
    if !selection_is_complete(document, tool, operands) {
        if let AuthoringTool::Constraint(intent) = tool {
            let selection = operands
                .iter()
                .map(|operand| operand.item)
                .collect::<Vec<_>>();
            if let Err(reason) = resolve_constraint(document, &selection, intent) {
                return Err(warning(document, tool, operands, reason));
            }
        }
        return Err(warning(
            document,
            tool,
            operands,
            DisabledReason::WrongArity,
        ));
    }
    if operands
        .iter()
        .any(|operand| !selection_exists(document, operand.item))
    {
        return Err(warning(
            document,
            tool,
            operands,
            DisabledReason::MissingObject,
        ));
    }
    let operands = normalize_operands(tool, operands);
    let selection = operands
        .iter()
        .map(|operand| operand.item)
        .collect::<Vec<_>>();
    let resolved_constraint = match tool {
        AuthoringTool::Constraint(intent) => Some(
            resolve_constraint(document, &selection, intent)
                .map_err(|reason| warning(document, tool, &operands, reason))?,
        ),
        AuthoringTool::Dimension(kind) => {
            validate_dimension_selection(document, &selection, kind)
                .map_err(|reason| warning(document, tool, &operands, reason))?;
            None
        }
    };
    Ok(AuthoringApplication {
        tool,
        operands,
        options,
        resolved_constraint,
    })
}

fn normalize_operands(tool: AuthoringTool, operands: &[AuthoringOperand]) -> Vec<AuthoringOperand> {
    let mut normalized = operands.to_vec();
    match tool {
        AuthoringTool::Constraint(ConstraintIntent::Coincident | ConstraintIntent::Midpoint)
            if matches!(
                normalized.first().map(|value| value.item),
                Some(SelectionItem::Curve(_))
            ) && matches!(
                normalized.get(1).map(|value| value.item),
                Some(SelectionItem::Point(_))
            ) =>
        {
            normalized.swap(0, 1);
        }
        AuthoringTool::Constraint(ConstraintIntent::Symmetric) => {
            normalized.sort_by_key(|operand| match operand.item {
                SelectionItem::Point(_) => 0,
                SelectionItem::Curve(_) => 1,
                SelectionItem::Constraint(_)
                | SelectionItem::Dimension(_)
                | SelectionItem::Datum(_)
                | SelectionItem::Feature(_)
                | SelectionItem::FeatureCorner(_) => 2,
            });
        }
        _ => {}
    }
    normalized
}

fn selection_is_complete(
    document: &SketchDocument,
    tool: AuthoringTool,
    operands: &[AuthoringOperand],
) -> bool {
    let has_complete_arity = match tool {
        AuthoringTool::Constraint(ConstraintIntent::Horizontal | ConstraintIntent::Vertical) => {
            matches!(
                operands,
                [AuthoringOperand {
                    item: SelectionItem::Curve(_),
                    ..
                }] | [
                    AuthoringOperand {
                        item: SelectionItem::Point(_),
                        ..
                    },
                    AuthoringOperand {
                        item: SelectionItem::Point(_),
                        ..
                    }
                ]
            )
        }
        _ => operands.len() == tool.arity(),
    };
    if !has_complete_arity {
        return false;
    }
    match tool {
        AuthoringTool::Constraint(intent) => {
            let selection = operands
                .iter()
                .map(|operand| operand.item)
                .collect::<Vec<_>>();
            resolve_constraint(document, &selection, intent).is_ok()
        }
        AuthoringTool::Dimension(_) => true,
    }
}

fn prefix_is_compatible(
    document: &SketchDocument,
    tool: AuthoringTool,
    operands: &[AuthoringOperand],
) -> bool {
    let kinds = expected_operands(
        document,
        tool,
        &operands[..operands.len().saturating_sub(1)],
    );
    operands.last().is_some_and(|operand| {
        kinds
            .iter()
            .any(|kind| operand_matches(document, operand.item, *kind))
    })
}

fn operand_matches(
    document: &SketchDocument,
    item: SelectionItem,
    kind: AuthoringOperandKind,
) -> bool {
    match (item, kind) {
        (SelectionItem::Point(_), AuthoringOperandKind::Point)
        | (SelectionItem::Curve(_), AuthoringOperandKind::Curve)
        | (SelectionItem::Datum(_), AuthoringOperandKind::Datum) => true,
        (SelectionItem::Datum(datum), AuthoringOperandKind::DatumAxis) => {
            datum.coordinate_axis().is_some()
        }
        (SelectionItem::Curve(span), AuthoringOperandKind::Line) => {
            line_endpoints(document, span).is_ok()
        }
        (SelectionItem::Curve(span), AuthoringOperandKind::CircleOrArc) => {
            document.curve(span.curve).is_some_and(|curve| {
                matches!(
                    curve.definition,
                    CurveDefinition::Circle { .. } | CurveDefinition::CircularArc { .. }
                )
            })
        }
        _ => false,
    }
}

fn expected_operands(
    _document: &SketchDocument,
    tool: AuthoringTool,
    operands: &[AuthoringOperand],
) -> Vec<AuthoringOperandKind> {
    use AuthoringOperandKind::{CircleOrArc, Curve, Datum, DatumAxis, Line, Point};
    match tool {
        AuthoringTool::Constraint(ConstraintIntent::Lock)
        | AuthoringTool::Dimension(DimensionKind::PointDistance) => vec![Point],
        AuthoringTool::Constraint(ConstraintIntent::Horizontal | ConstraintIntent::Vertical) => {
            if matches!(
                operands,
                [AuthoringOperand {
                    item: SelectionItem::Point(_),
                    ..
                }]
            ) {
                vec![Point]
            } else {
                vec![Point, Line]
            }
        }
        AuthoringTool::Constraint(ConstraintIntent::Coincident) => match operands {
            [
                AuthoringOperand {
                    item: SelectionItem::Datum(_),
                    ..
                },
            ] => vec![Point],
            [
                AuthoringOperand {
                    item: SelectionItem::Curve(_),
                    ..
                },
            ] => vec![Point, Curve],
            _ => vec![Point, Curve, Datum],
        },
        AuthoringTool::Dimension(DimensionKind::SegmentLength | DimensionKind::OrientedAngle) => {
            vec![Line]
        }
        AuthoringTool::Constraint(ConstraintIntent::Parallel | ConstraintIntent::Collinear) => {
            if matches!(
                operands,
                [AuthoringOperand {
                    item: SelectionItem::Datum(_),
                    ..
                }]
            ) {
                vec![Line]
            } else {
                vec![Line, DatumAxis]
            }
        }
        AuthoringTool::Constraint(ConstraintIntent::Perpendicular) => {
            if matches!(
                operands,
                [AuthoringOperand {
                    item: SelectionItem::Datum(_),
                    ..
                }]
            ) {
                vec![Line]
            } else {
                vec![Line, CircleOrArc, DatumAxis]
            }
        }
        AuthoringTool::Constraint(
            ConstraintIntent::Concentric
            | ConstraintIntent::Continuity
            | ConstraintIntent::Equal
            | ConstraintIntent::Tangent,
        ) => {
            vec![Curve]
        }
        AuthoringTool::Constraint(ConstraintIntent::Midpoint) => {
            if operands
                .iter()
                .any(|operand| matches!(operand.item, SelectionItem::Point(_)))
            {
                vec![Line]
            } else if operands
                .iter()
                .any(|operand| matches!(operand.item, SelectionItem::Curve(_)))
            {
                vec![Point]
            } else {
                vec![Point, Line]
            }
        }
        AuthoringTool::Constraint(ConstraintIntent::Symmetric) => {
            if operands.len() < 2 {
                vec![Point]
            } else {
                vec![Line, DatumAxis]
            }
        }
        AuthoringTool::Dimension(DimensionKind::Radius | DimensionKind::Diameter) => {
            vec![CircleOrArc]
        }
    }
}

fn warning(
    document: &SketchDocument,
    tool: AuthoringTool,
    operands: &[AuthoringOperand],
    reason: DisabledReason,
) -> AuthoringWarning {
    let expected = expected_operands(document, tool, operands);
    let expected_label = expected
        .iter()
        .map(|kind| kind.label())
        .collect::<Vec<_>>()
        .join(" or ");
    let message = match reason {
        DisabledReason::EmptySelection => {
            "select operands or click the tool to enter authoring mode"
        }
        DisabledReason::WrongArity => "the tool needs a different number of operands",
        DisabledReason::WrongOperandKind => "that item is not compatible with the active tool",
        DisabledReason::ProtectedDatum => {
            "reference datums are immutable and cannot be edited by this action"
        }
        DisabledReason::MissingObject => "that operand no longer exists in the current design",
        DisabledReason::InvalidSpan => "that curve span is not valid for the active tool",
        DisabledReason::SameSemanticOperand => {
            "those selections resolve to the same semantic operand"
        }
        DisabledReason::AlreadyInRequestedState
        | DisabledReason::NothingToUndo
        | DisabledReason::NothingToRedo => "the requested authoring action is unavailable",
    };
    AuthoringWarning {
        reason,
        expected,
        message: if expected_label.is_empty() {
            message.to_owned()
        } else {
            format!("{message}; expected {expected_label}")
        },
    }
}

#[cfg(test)]
mod tests {
    use geosolve_sketch::{
        CurveDefinition, CurveSpan, DocumentConstraintDefinition, ScalarDomain, ScalarUnit,
        SketchDatum, SketchDocument,
    };

    use super::*;

    fn document() -> (SketchDocument, [SelectionItem; 4]) {
        let mut document = SketchDocument::new(1.0).unwrap();
        let first = document.add_point("first", [0.0, 0.0]).unwrap();
        let second = document.add_point("second", [2.0, 0.0]).unwrap();
        let line = document
            .add_curve(
                "line",
                CurveDefinition::Line {
                    start: first,
                    end: second,
                    branch_direction: [1.0, 0.0],
                },
            )
            .unwrap();
        let center = document.add_point("center", [0.0, 2.0]).unwrap();
        let radius = document
            .add_scalar("radius", 1.0, ScalarUnit::Length, ScalarDomain::Positive)
            .unwrap();
        let circle = document
            .add_curve("circle", CurveDefinition::Circle { center, radius })
            .unwrap();
        (
            document,
            [
                SelectionItem::Point(first),
                SelectionItem::Point(second),
                SelectionItem::Curve(CurveSpan {
                    curve: line,
                    segment: 0,
                }),
                SelectionItem::Curve(CurveSpan {
                    curve: circle,
                    segment: 0,
                }),
            ],
        )
    }

    #[test]
    fn preselection_applies_once_without_entering_mode() {
        let (document, items) = document();
        let mut state = AuthoringState::default();
        let outcome = state.activate(
            &document,
            AuthoringTool::Constraint(ConstraintIntent::Coincident),
            &items[..2]
                .iter()
                .copied()
                .map(AuthoringOperand::selected)
                .collect::<Vec<_>>(),
        );
        assert!(matches!(outcome, AuthoringOutcome::Apply(_)));
        assert_eq!(state.active_tool(), None);
        assert!(state.pending().is_empty());
    }

    #[test]
    fn repeated_pair_mode_clears_only_after_transaction() {
        let (document, items) = document();
        let mut state = AuthoringState::default();
        assert!(matches!(
            state.activate(
                &document,
                AuthoringTool::Constraint(ConstraintIntent::Coincident),
                &[],
            ),
            AuthoringOutcome::ModeEntered { .. }
        ));
        assert!(matches!(
            state.pick(&document, AuthoringOperand::selected(items[0])),
            AuthoringOutcome::Collecting { .. }
        ));
        assert!(matches!(
            state.pick(&document, AuthoringOperand::selected(items[1])),
            AuthoringOutcome::Apply(_)
        ));
        assert_eq!(state.pending().len(), 2);
        state.transaction_finished();
        assert!(state.pending().is_empty());
        assert_eq!(
            state.active_tool(),
            Some(AuthoringTool::Constraint(ConstraintIntent::Coincident))
        );
    }

    #[test]
    fn escape_is_two_stage_and_stale_operands_reconcile() {
        let (mut document, items) = document();
        let mut state = AuthoringState::default();
        let _ = state.activate(
            &document,
            AuthoringTool::Constraint(ConstraintIntent::Coincident),
            &[],
        );
        let _ = state.pick(&document, AuthoringOperand::selected(items[3]));
        document
            .remove_with_owned_state(items[3].object().expect("native fixture item"))
            .unwrap();
        assert!(matches!(
            state.reconcile(&document),
            AuthoringOutcome::Collecting { .. }
        ));
        assert!(state.pending().is_empty());
        let _ = state.pick(&document, AuthoringOperand::selected(items[1]));
        assert!(matches!(
            state.cancel(&document),
            AuthoringOutcome::PendingCleared { .. }
        ));
        assert!(matches!(
            state.cancel(&document),
            AuthoringOutcome::ModeExited
        ));
    }

    #[test]
    fn role_distinct_operands_normalize_but_ordered_angle_does_not() {
        let (document, items) = document();
        let mut state = AuthoringState::default();
        let midpoint = state.activate(
            &document,
            AuthoringTool::Constraint(ConstraintIntent::Midpoint),
            &[
                AuthoringOperand::selected(items[2]),
                AuthoringOperand::selected(items[0]),
            ],
        );
        let AuthoringOutcome::Apply(midpoint) = midpoint else {
            panic!("midpoint application");
        };
        assert!(matches!(midpoint.operands[0].item, SelectionItem::Point(_)));

        let angle = state.activate(
            &document,
            AuthoringTool::Dimension(DimensionKind::OrientedAngle),
            &[
                AuthoringOperand::selected(items[2]),
                AuthoringOperand::selected(items[2]),
            ],
        );
        assert!(matches!(angle, AuthoringOutcome::Warning(_)));
    }

    #[test]
    fn options_are_process_local_and_survive_tool_reentry() {
        let (document, _) = document();
        let options = AuthoringOptions {
            tangent_orientation: TangentOrientation::Opposed,
            curvature_relation: DocumentCurveCurvatureRelation::MagnitudeOppositeSign,
            continuity: DocumentCurveContinuity::G2,
            dimension_mode: DocumentDimensionMode::Reference,
            angle_orientation: DocumentAngleOrientation::Clockwise,
        };
        let mut state = AuthoringState::default();
        state.set_options(options);
        let _ = state.activate(
            &document,
            AuthoringTool::Constraint(ConstraintIntent::Tangent),
            &[],
        );
        state.deactivate();
        let _ = state.activate(
            &document,
            AuthoringTool::Dimension(DimensionKind::OrientedAngle),
            &[],
        );
        assert_eq!(state.options(), options);
        assert_eq!(
            AuthoringState::default().options(),
            AuthoringOptions::default()
        );
    }

    #[test]
    fn horizontal_and_vertical_apply_one_line_or_collect_two_points_in_either_order() {
        let (document, items) = document();
        for intent in [ConstraintIntent::Horizontal, ConstraintIntent::Vertical] {
            let mut state = AuthoringState::default();
            let line = state.activate(
                &document,
                AuthoringTool::Constraint(intent),
                &[AuthoringOperand::selected(items[2])],
            );
            assert!(matches!(line, AuthoringOutcome::Apply(_)));

            for points in [[items[0], items[1]], [items[1], items[0]]] {
                let mut state = AuthoringState::default();
                let first = state.activate(
                    &document,
                    AuthoringTool::Constraint(intent),
                    &[AuthoringOperand::selected(points[0])],
                );
                assert!(matches!(first, AuthoringOutcome::Collecting { .. }));
                assert_eq!(state.pending().len(), 1);
                let second = state.pick(&document, AuthoringOperand::selected(points[1]));
                assert!(matches!(second, AuthoringOutcome::Apply(_)));
            }
        }
    }

    #[test]
    fn horizontal_and_vertical_report_operand_kind_separately_from_arity() {
        let (document, items) = document();
        for intent in [ConstraintIntent::Horizontal, ConstraintIntent::Vertical] {
            assert_eq!(
                resolve_constraint(&document, &[items[0], items[3]], intent),
                Err(DisabledReason::WrongOperandKind)
            );
            assert_eq!(
                resolve_constraint(&document, &items[..3], intent),
                Err(DisabledReason::WrongArity)
            );

            let mixed_pair = AuthoringState::default().activate(
                &document,
                AuthoringTool::Constraint(intent),
                &[
                    AuthoringOperand::selected(items[0]),
                    AuthoringOperand::selected(items[3]),
                ],
            );
            assert!(matches!(
                mixed_pair,
                AuthoringOutcome::Warning(AuthoringWarning {
                    reason: DisabledReason::WrongOperandKind,
                    ..
                })
            ));

            let too_many = AuthoringState::default().activate(
                &document,
                AuthoringTool::Constraint(intent),
                &items[..3]
                    .iter()
                    .copied()
                    .map(AuthoringOperand::selected)
                    .collect::<Vec<_>>(),
            );
            assert!(matches!(
                too_many,
                AuthoringOutcome::Warning(AuthoringWarning {
                    reason: DisabledReason::WrongArity,
                    ..
                })
            ));
        }
    }

    #[test]
    fn incompatible_pick_preserves_pending_point_and_same_point_is_precise() {
        let (document, items) = document();
        let mut state = AuthoringState::default();
        let _ = state.activate(
            &document,
            AuthoringTool::Constraint(ConstraintIntent::Horizontal),
            &[],
        );
        assert!(matches!(
            state.pick(&document, AuthoringOperand::selected(items[0])),
            AuthoringOutcome::Collecting { .. }
        ));
        assert!(matches!(
            state.pick(&document, AuthoringOperand::selected(items[3])),
            AuthoringOutcome::Warning(AuthoringWarning {
                reason: DisabledReason::WrongOperandKind,
                ..
            })
        ));
        assert_eq!(state.pending().len(), 1);
        assert!(matches!(
            state.pick(&document, AuthoringOperand::selected(items[0])),
            AuthoringOutcome::Warning(AuthoringWarning {
                reason: DisabledReason::SameSemanticOperand,
                ..
            })
        ));
        assert_eq!(state.pending().len(), 1);
    }

    #[test]
    fn axis_relation_authoring_rejects_origin_as_a_prefix_but_accepts_axis_datums() {
        let (document, items) = document();
        for (intent, resolved) in [
            (
                ConstraintIntent::Collinear,
                ResolvedConstraintKind::CollinearWithDatumAxis,
            ),
            (
                ConstraintIntent::Parallel,
                ResolvedConstraintKind::HorizontalLine,
            ),
            (
                ConstraintIntent::Perpendicular,
                ResolvedConstraintKind::VerticalLine,
            ),
        ] {
            let tool = AuthoringTool::Constraint(intent);
            let mut state = AuthoringState::default();
            let _ = state.activate(&document, tool, &[]);
            assert!(matches!(
                state.pick(
                    &document,
                    AuthoringOperand::selected(SelectionItem::Datum(SketchDatum::Origin)),
                ),
                AuthoringOutcome::Warning(AuthoringWarning {
                    reason: DisabledReason::WrongOperandKind,
                    ..
                })
            ));
            assert!(state.pending().is_empty());

            assert!(matches!(
                state.pick(
                    &document,
                    AuthoringOperand::selected(SelectionItem::Datum(SketchDatum::XAxis)),
                ),
                AuthoringOutcome::Collecting { .. }
            ));
            assert!(matches!(
                state.pick(&document, AuthoringOperand::selected(items[2])),
                AuthoringOutcome::Apply(AuthoringApplication {
                    resolved_constraint: Some(actual),
                    ..
                }) if actual == resolved
            ));
        }
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "one fixture proves commutative acceptance and exact semantic-operand rejection for both new curve relations"
    )]
    fn concentric_and_collinear_are_commutative_and_polyline_supports_are_lines() {
        let (mut document, items) = document();
        let other_center = document.add_point("other center", [4.0, 2.0]).unwrap();
        let other_radius = document
            .add_scalar(
                "other radius",
                1.5,
                ScalarUnit::Length,
                ScalarDomain::Positive,
            )
            .unwrap();
        let other_circle = document
            .add_curve(
                "other circle",
                CurveDefinition::Circle {
                    center: other_center,
                    radius: other_radius,
                },
            )
            .unwrap();
        let third = document.add_point("third", [4.0, 0.0]).unwrap();
        let polyline = document
            .add_curve(
                "polyline",
                CurveDefinition::Polyline {
                    points: vec![
                        match items[1] {
                            SelectionItem::Point(point) => point,
                            _ => unreachable!(),
                        },
                        third,
                    ],
                    closed: false,
                    branch_directions: vec![[1.0, 0.0]],
                },
            )
            .unwrap();
        let other_circle = SelectionItem::Curve(CurveSpan::line(other_circle));
        let polyline = SelectionItem::Curve(CurveSpan::line(polyline));

        for selection in [[items[3], other_circle], [other_circle, items[3]]] {
            let outcome = AuthoringState::default().activate(
                &document,
                AuthoringTool::Constraint(ConstraintIntent::Concentric),
                &selection.map(AuthoringOperand::selected),
            );
            assert!(matches!(
                outcome,
                AuthoringOutcome::Apply(AuthoringApplication {
                    resolved_constraint: Some(ResolvedConstraintKind::ConcentricCurves),
                    ..
                })
            ));
        }
        for selection in [[items[2], polyline], [polyline, items[2]]] {
            let outcome = AuthoringState::default().activate(
                &document,
                AuthoringTool::Constraint(ConstraintIntent::Collinear),
                &selection.map(AuthoringOperand::selected),
            );
            assert!(matches!(
                outcome,
                AuthoringOutcome::Apply(AuthoringApplication {
                    resolved_constraint: Some(ResolvedConstraintKind::CollinearSupports),
                    ..
                })
            ));
        }

        let same_center_radius = document
            .add_scalar(
                "alias radius",
                0.5,
                ScalarUnit::Length,
                ScalarDomain::Positive,
            )
            .unwrap();
        let CurveDefinition::Circle {
            center: original_center,
            ..
        } = document
            .curve(match items[3] {
                SelectionItem::Curve(span) => span.curve,
                _ => unreachable!(),
            })
            .unwrap()
            .definition
        else {
            unreachable!()
        };
        let alias = document
            .add_curve(
                "center alias",
                CurveDefinition::Circle {
                    center: original_center,
                    radius: same_center_radius,
                },
            )
            .unwrap();
        let warning = AuthoringState::default().activate(
            &document,
            AuthoringTool::Constraint(ConstraintIntent::Concentric),
            &[
                AuthoringOperand::selected(items[3]),
                AuthoringOperand::selected(SelectionItem::Curve(CurveSpan::line(alias))),
            ],
        );
        assert!(matches!(
            warning,
            AuthoringOutcome::Warning(AuthoringWarning {
                reason: DisabledReason::SameSemanticOperand,
                ..
            })
        ));

        let repeated = AuthoringState::default().activate(
            &document,
            AuthoringTool::Constraint(ConstraintIntent::Collinear),
            &[
                AuthoringOperand::selected(items[2]),
                AuthoringOperand::selected(items[2]),
            ],
        );
        assert!(matches!(
            repeated,
            AuthoringOutcome::Warning(AuthoringWarning {
                reason: DisabledReason::SameSemanticOperand,
                ..
            })
        ));

        assert!(document.constraints().iter().all(|constraint| !matches!(
            constraint.definition,
            DocumentConstraintDefinition::Concentric { .. }
                | DocumentConstraintDefinition::Collinear { .. }
        )));
    }
}
