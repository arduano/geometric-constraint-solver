// SPDX-License-Identifier: GPL-3.0-or-later

//! Selection-independent CAD relation and dimension authoring.

use geosolve_sketch::{
    CurveDefinition, DocumentAngleOrientation, DocumentCurveContinuity,
    DocumentCurveCurvatureRelation, DocumentDimensionMode, SketchDocument, TangentOrientation,
};

use crate::coordinator::{resolve_constraint, selection_exists, validate_dimension_selection};
use crate::{
    ConstraintIntent, DimensionKind, DisabledReason, ResolvedConstraintKind, SelectionItem,
};

/// One palette tool owned by the reusable authoring state machine.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuthoringTool {
    Constraint(ConstraintIntent),
    Dimension(DimensionKind),
}

impl AuthoringTool {
    /// Number of operands required before the tool can apply.
    #[must_use]
    pub const fn arity(self) -> usize {
        match self {
            Self::Constraint(
                ConstraintIntent::Lock | ConstraintIntent::Horizontal | ConstraintIntent::Vertical,
            )
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
                | ConstraintIntent::Continuity,
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
}

impl AuthoringOperandKind {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Point => "point",
            Self::Curve => "curve",
            Self::Line => "line",
            Self::CircleOrArc => "circle or arc",
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
        match application(document, tool, &candidate, self.options) {
            Ok(application) => {
                self.pending.clone_from(&application.operands);
                AuthoringOutcome::Apply(application)
            }
            Err(warning) => AuthoringOutcome::Warning(warning),
        }
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
    if operands.len() != tool.arity() {
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
                SelectionItem::Constraint(_) | SelectionItem::Dimension(_) => 2,
            });
        }
        _ => {}
    }
    normalized
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
        | (SelectionItem::Curve(_), AuthoringOperandKind::Curve) => true,
        (SelectionItem::Curve(span), AuthoringOperandKind::Line) => document
            .curve(span.curve)
            .is_some_and(|curve| matches!(curve.definition, CurveDefinition::Line { .. })),
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
    use AuthoringOperandKind::{CircleOrArc, Curve, Line, Point};
    match tool {
        AuthoringTool::Constraint(ConstraintIntent::Lock)
        | AuthoringTool::Dimension(DimensionKind::PointDistance) => vec![Point],
        AuthoringTool::Constraint(ConstraintIntent::Horizontal | ConstraintIntent::Vertical)
        | AuthoringTool::Dimension(DimensionKind::SegmentLength | DimensionKind::OrientedAngle) => {
            vec![Line]
        }
        AuthoringTool::Constraint(ConstraintIntent::Coincident) => vec![Point, Curve],
        AuthoringTool::Constraint(ConstraintIntent::Parallel) => vec![Line],
        AuthoringTool::Constraint(ConstraintIntent::Continuity) => vec![Curve],
        AuthoringTool::Constraint(ConstraintIntent::Perpendicular) => vec![Line, CircleOrArc],
        AuthoringTool::Constraint(ConstraintIntent::Equal | ConstraintIntent::Tangent) => {
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
                vec![Line]
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
        DisabledReason::MissingObject => "that operand no longer exists in the current design",
        DisabledReason::InvalidSpan => "that curve span is not valid for the active tool",
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
    use geosolve_sketch::{CurveSpan, ScalarDomain, ScalarUnit, SketchDocument};

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
        document.remove_with_owned_state(items[3].object()).unwrap();
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
}
