// SPDX-License-Identifier: GPL-3.0-or-later

//! Presentation-independent operand collection for native profile offsets.
//!
//! The state owns no offset equations or browser event policy. It consumes one complete,
//! accepted-input-stamped topology index and resolves hover and pointer-down through the same
//! bounded target resolver.

use std::collections::BTreeSet;
use std::sync::Arc;

use geosolve_sketch::{DocumentFaceOffsetDirection, DocumentLineSide, PreparedSketchInput};
use geosolve_sketch_topology::{
    OffsetDirectedSpan, OffsetEndpointRef, OffsetEndpointRole, OffsetFaceKey, OffsetFaceLookup,
    OffsetOperandIndex, OffsetTraversal,
};

use crate::{
    EditorScene, GeometryInteractionPolicy, PickTolerance, SceneCurveOrigin, SceneGeometryHit,
    ScreenPoint, SelectionItem,
};

const MAX_OFFSET_CHAIN_SPANS: usize = 256;
const MAX_OFFSET_HIT_CANDIDATES: usize = 256;

/// One exact semantic operand under the pointer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OffsetAuthoringTarget {
    Face(OffsetFaceKey),
    Span(geosolve_sketch::CurveSpan),
}

/// Whether the current Offset target can be consumed by an unchanged pick.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OffsetAuthoringTargetAvailability {
    Available,
    Unavailable {
        kind: OffsetAuthoringWarningKind,
        message: String,
    },
}

impl OffsetAuthoringTargetAvailability {
    #[must_use]
    pub const fn is_available(&self) -> bool {
        matches!(self, Self::Available)
    }

    #[must_use]
    pub fn message(&self) -> Option<&str> {
        match self {
            Self::Available => None,
            Self::Unavailable { message, .. } => Some(message.as_str()),
        }
    }
}

/// Exact hover identity plus the semantic result an unchanged pick will produce.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OffsetAuthoringHover {
    pub target: OffsetAuthoringTarget,
    pub availability: OffsetAuthoringTargetAvailability,
}

/// One collected-chain terminal in traversal order.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OffsetAuthoringChainTerminal {
    pub endpoint: OffsetEndpointRef,
    pub model_position: [f64; 2],
}

/// Ordered chain presentation retained independently of browser selection sets.
#[derive(Clone, Debug, PartialEq)]
pub struct OffsetAuthoringChainPresentation {
    pub spans: Vec<OffsetDirectedSpan>,
    pub start: OffsetAuthoringChainTerminal,
    pub end: OffsetAuthoringChainTerminal,
}

/// One complete operand collected by the Offset tool.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OffsetAuthoringOperand {
    Face {
        key: OffsetFaceKey,
        direction: DocumentFaceOffsetDirection,
    },
    OpenChain {
        spans: Vec<OffsetDirectedSpan>,
        side: DocumentLineSide,
    },
}

impl OffsetAuthoringOperand {
    #[must_use]
    pub const fn kind_label(&self) -> &'static str {
        match self {
            Self::Face { .. } => "Face",
            Self::OpenChain { .. } => "Open chain",
        }
    }

    #[must_use]
    pub fn span_count(&self) -> usize {
        match self {
            Self::Face { key, .. } => {
                key.outer.spans.len() + key.holes.iter().map(|hole| hole.spans.len()).sum::<usize>()
            }
            Self::OpenChain { spans, .. } => spans.len(),
        }
    }
}

/// Stable stage used by panel guidance and accessibility text.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OffsetAuthoringStage {
    PickOperand,
    CollectChain,
    PreviewReady,
}

/// Current presentation-neutral Offset guidance.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OffsetAuthoringGuidance {
    pub stage: OffsetAuthoringStage,
    pub message: &'static str,
}

/// Typed state-neutral warning from operand collection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum OffsetAuthoringWarningKind {
    NoTarget,
    UnsupportedOperand,
    PeriodicChain,
    DuplicateSpan,
    DisconnectedSpan,
    AmbiguousJoin,
    BranchingJoin,
    WouldCloseChain,
    CandidateLimitExceeded,
    ChainLimitExceeded,
    InvalidDistance,
    StaleInput,
}

/// One warning with stable stage context.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OffsetAuthoringWarning {
    pub kind: OffsetAuthoringWarningKind,
    pub stage: OffsetAuthoringStage,
    pub message: String,
}

/// Result of one Offset authoring transition.
#[derive(Clone, Debug, PartialEq)]
pub enum OffsetAuthoringOutcome {
    ModeEntered(OffsetAuthoringGuidance),
    HoverChanged(Option<OffsetAuthoringHover>),
    OperandChanged {
        operand: Option<OffsetAuthoringOperand>,
        guidance: OffsetAuthoringGuidance,
    },
    DistanceChanged {
        distance: f64,
        operand: Option<OffsetAuthoringOperand>,
        guidance: OffsetAuthoringGuidance,
    },
    ApplyRequested(Box<OffsetAuthoringCandidate>),
    Warning(OffsetAuthoringWarning),
    ModeExited,
    Inactive,
}

/// Complete immutable request ready for deterministic construction.
#[derive(Clone, Debug, PartialEq)]
pub struct OffsetAuthoringCandidate {
    pub input: PreparedSketchInput,
    pub operand: OffsetAuthoringOperand,
    pub distance: f64,
}

/// Separate reusable collector for native topology-preserving Offset.
#[derive(Clone, Debug, Default)]
pub struct OffsetAuthoringState {
    index: Option<Arc<OffsetOperandIndex>>,
    operand: Option<OffsetAuthoringOperand>,
    distance: Option<f64>,
    remembered_distance: Option<f64>,
    pending_direction_flip: bool,
    hover: Option<OffsetAuthoringHover>,
    addition_history: Vec<Vec<OffsetDirectedSpan>>,
}

impl PartialEq for OffsetAuthoringState {
    fn eq(&self, other: &Self) -> bool {
        self.index.as_deref() == other.index.as_deref()
            && self.operand == other.operand
            && self.distance == other.distance
            && self.remembered_distance == other.remembered_distance
            && self.pending_direction_flip == other.pending_direction_flip
            && self.hover == other.hover
            && self.addition_history == other.addition_history
    }
}

impl OffsetAuthoringState {
    /// Enters Offset with one complete index. Prior operand identity is never retained.
    #[must_use]
    pub fn activate(
        &mut self,
        index: Arc<OffsetOperandIndex>,
        model_scale: f64,
    ) -> OffsetAuthoringOutcome {
        self.index = Some(index);
        self.operand = None;
        self.pending_direction_flip = false;
        self.hover = None;
        self.addition_history.clear();
        let fallback = 0.1 * model_scale.abs();
        self.distance = self
            .remembered_distance
            .filter(|value| finite_positive(*value))
            .or_else(|| finite_positive(fallback).then_some(fallback));
        OffsetAuthoringOutcome::ModeEntered(self.guidance())
    }

    /// Exits authoring while preserving only the process-local last valid distance.
    #[must_use]
    pub fn cancel(&mut self) -> OffsetAuthoringOutcome {
        if self.index.take().is_none() {
            return OffsetAuthoringOutcome::Inactive;
        }
        self.operand = None;
        self.pending_direction_flip = false;
        self.hover = None;
        self.addition_history.clear();
        self.distance = None;
        OffsetAuthoringOutcome::ModeExited
    }

    #[must_use]
    pub const fn is_active(&self) -> bool {
        self.index.is_some()
    }

    #[must_use]
    pub fn index(&self) -> Option<&Arc<OffsetOperandIndex>> {
        self.index.as_ref()
    }

    #[must_use]
    pub const fn operand(&self) -> Option<&OffsetAuthoringOperand> {
        self.operand.as_ref()
    }

    #[must_use]
    pub const fn distance(&self) -> Option<f64> {
        self.distance
    }

    #[must_use]
    pub fn hover_target(&self) -> Option<&OffsetAuthoringTarget> {
        self.hover.as_ref().map(|hover| &hover.target)
    }

    #[must_use]
    pub const fn hover(&self) -> Option<&OffsetAuthoringHover> {
        self.hover.as_ref()
    }

    /// Clears presentation-only operand hover when a higher-priority provisional surface wins.
    pub(crate) fn clear_hover(&mut self) {
        self.hover = None;
    }

    #[must_use]
    pub const fn remembered_distance(&self) -> Option<f64> {
        self.remembered_distance
    }

    /// Returns ordered traversal and exact model-space terminal positions for an open chain.
    #[must_use]
    pub fn chain_presentation(&self) -> Option<OffsetAuthoringChainPresentation> {
        let index = self.index.as_ref()?;
        let Some(OffsetAuthoringOperand::OpenChain { spans, .. }) = self.operand.as_ref() else {
            return None;
        };
        let first = *spans.first()?;
        let last = *spans.last()?;
        let start_endpoint = directed_endpoint(first, true);
        let end_endpoint = directed_endpoint(last, false);
        Some(OffsetAuthoringChainPresentation {
            spans: spans.clone(),
            start: OffsetAuthoringChainTerminal {
                endpoint: start_endpoint,
                model_position: endpoint_position(index, start_endpoint)?,
            },
            end: OffsetAuthoringChainTerminal {
                endpoint: end_endpoint,
                model_position: endpoint_position(index, end_endpoint)?,
            },
        })
    }

    #[must_use]
    pub fn guidance(&self) -> OffsetAuthoringGuidance {
        match self.operand.as_ref() {
            None => OffsetAuthoringGuidance {
                stage: OffsetAuthoringStage::PickOperand,
                message: "Select a face, or select a curve to start an open chain",
            },
            Some(OffsetAuthoringOperand::OpenChain { .. }) => OffsetAuthoringGuidance {
                stage: OffsetAuthoringStage::CollectChain,
                message: "Select an adjacent curve, or Apply the collected open chain",
            },
            Some(OffsetAuthoringOperand::Face { .. }) => OffsetAuthoringGuidance {
                stage: OffsetAuthoringStage::PreviewReady,
                message: "Adjust Distance or direction, then Apply",
            },
        }
    }

    /// Resolves and publishes the exact target an unchanged click would consume.
    #[must_use]
    pub fn hover_at(
        &mut self,
        scene: &EditorScene,
        position: ScreenPoint,
        tolerance: PickTolerance,
        policy: GeometryInteractionPolicy,
    ) -> OffsetAuthoringOutcome {
        if self.index.is_none() {
            return OffsetAuthoringOutcome::Inactive;
        }
        let resolved = self.resolve_target(scene, position, tolerance, policy);
        let hover = resolved.ok().flatten();
        self.hover.clone_from(&hover);
        OffsetAuthoringOutcome::HoverChanged(hover)
    }

    /// Resolves one pointer-down through the same target resolver used by hover.
    #[must_use]
    pub fn pick_at(
        &mut self,
        scene: &EditorScene,
        position: ScreenPoint,
        tolerance: PickTolerance,
        policy: GeometryInteractionPolicy,
    ) -> OffsetAuthoringOutcome {
        if self.index.is_none() {
            return OffsetAuthoringOutcome::Inactive;
        }
        let hover = match self.resolve_target(scene, position, tolerance, policy) {
            Ok(Some(hover)) => hover,
            Ok(None) => {
                self.hover = None;
                return self.warning(
                    OffsetAuthoringWarningKind::NoTarget,
                    "No eligible face or curve is under the pointer",
                );
            }
            Err(kind) => {
                self.hover = None;
                return self.warning(
                    kind,
                    "Offset hit resolution exceeded its bounded candidate limit",
                );
            }
        };
        self.pick_hover(hover)
    }

    /// Consumes one persistent semantic target from a tree or keyboard activation.
    #[must_use]
    pub fn pick_target(&mut self, target: OffsetAuthoringTarget) -> OffsetAuthoringOutcome {
        if self.index.is_none() {
            return OffsetAuthoringOutcome::Inactive;
        }
        let Some(hover) = self.describe_target(target) else {
            self.hover = None;
            return self.warning(
                OffsetAuthoringWarningKind::StaleInput,
                "The selected geometry no longer belongs to this Offset snapshot",
            );
        };
        self.pick_hover(hover)
    }

    fn pick_hover(&mut self, hover: OffsetAuthoringHover) -> OffsetAuthoringOutcome {
        self.hover = Some(hover.clone());
        if let OffsetAuthoringTargetAvailability::Unavailable { kind, message } = hover.availability
        {
            return self.warning(kind, message);
        }
        match hover.target {
            OffsetAuthoringTarget::Face(key) => self.pick_face(key),
            OffsetAuthoringTarget::Span(span) => self.pick_span(span),
        }
    }

    /// Replaces the numeric input. Negative input flips direction and stores a positive value.
    #[must_use]
    pub fn set_distance(&mut self, value: f64) -> OffsetAuthoringOutcome {
        if self.index.is_none() {
            return OffsetAuthoringOutcome::Inactive;
        }
        if !value.is_finite() || value == 0.0 {
            self.distance = None;
            return self.warning(
                OffsetAuthoringWarningKind::InvalidDistance,
                "Offset distance must be finite and nonzero",
            );
        }
        if self.operand.is_none() {
            self.pending_direction_flip = value.is_sign_negative();
        } else if value.is_sign_negative() {
            self.flip_operand_direction();
        }
        let value = value.abs();
        self.distance = Some(value);
        self.remembered_distance = Some(value);
        OffsetAuthoringOutcome::DistanceChanged {
            distance: value,
            operand: self.operand.clone(),
            guidance: self.guidance(),
        }
    }

    /// Flips Outward/Inward or Left/Right without changing the positive distance.
    #[must_use]
    pub fn flip(&mut self) -> OffsetAuthoringOutcome {
        if self.index.is_none() {
            return OffsetAuthoringOutcome::Inactive;
        }
        self.flip_operand_direction();
        OffsetAuthoringOutcome::OperandChanged {
            operand: self.operand.clone(),
            guidance: self.guidance(),
        }
    }

    /// Clears the operand and transient direction/hover, retaining the last valid numeric input.
    #[must_use]
    pub fn reset(&mut self) -> OffsetAuthoringOutcome {
        if self.index.is_none() {
            return OffsetAuthoringOutcome::Inactive;
        }
        self.operand = None;
        self.pending_direction_flip = false;
        self.hover = None;
        self.addition_history.clear();
        OffsetAuthoringOutcome::OperandChanged {
            operand: None,
            guidance: self.guidance(),
        }
    }

    /// Removes the most recently added chain span, or clears a one-shot face selection.
    #[must_use]
    pub fn backspace(&mut self) -> OffsetAuthoringOutcome {
        if self.index.is_none() {
            return OffsetAuthoringOutcome::Inactive;
        }
        match &mut self.operand {
            Some(OffsetAuthoringOperand::OpenChain { spans, .. }) => {
                if let Some(previous) = self.addition_history.pop() {
                    *spans = previous;
                    if spans.is_empty() {
                        self.operand = None;
                        self.pending_direction_flip = false;
                    }
                } else {
                    self.operand = None;
                }
            }
            Some(OffsetAuthoringOperand::Face { .. }) => self.operand = None,
            None => {}
        }
        self.hover = None;
        OffsetAuthoringOutcome::OperandChanged {
            operand: self.operand.clone(),
            guidance: self.guidance(),
        }
    }

    /// Returns the complete current request without mutating authoring state.
    #[must_use]
    pub fn candidate(&self) -> Option<OffsetAuthoringCandidate> {
        let index = self.index.as_ref()?;
        let operand = self.operand.clone()?;
        let distance = self.distance.filter(|value| finite_positive(*value))?;
        Some(OffsetAuthoringCandidate {
            input: index.input(),
            operand,
            distance,
        })
    }

    /// Requests Apply while keeping Offset active. A successful coordinator commit should call
    /// [`Self::clear_after_apply`] so only the distance memory survives.
    #[must_use]
    pub fn apply(&self) -> OffsetAuthoringOutcome {
        self.candidate().map_or_else(
            || {
                if self.index.is_some() {
                    self.warning_value(
                        OffsetAuthoringWarningKind::InvalidDistance,
                        "Select one complete operand and enter a valid distance before Apply",
                    )
                } else {
                    OffsetAuthoringOutcome::Inactive
                }
            },
            |candidate| OffsetAuthoringOutcome::ApplyRequested(Box::new(candidate)),
        )
    }

    /// Clears committed operand identity but leaves this tool and its valid distance active.
    pub fn clear_after_apply(&mut self) {
        self.operand = None;
        self.pending_direction_flip = false;
        self.hover = None;
        self.addition_history.clear();
    }

    fn resolve_target(
        &self,
        scene: &EditorScene,
        position: ScreenPoint,
        tolerance: PickTolerance,
        policy: GeometryInteractionPolicy,
    ) -> Result<Option<OffsetAuthoringHover>, OffsetAuthoringWarningKind> {
        let Some(index) = self.index.as_ref() else {
            return Ok(None);
        };
        let hits = scene
            .native_authoring_hit_candidates_with_policy(
                position,
                tolerance,
                MAX_OFFSET_HIT_CANDIDATES,
                policy,
            )
            .map_err(|_| OffsetAuthoringWarningKind::CandidateLimitExceeded)?;
        if let Some(span) = hits
            .into_iter()
            .find_map(|hit| match (hit.item, hit.geometry) {
                (
                    SelectionItem::Curve(span),
                    Some(SceneGeometryHit::NativeCurve {
                        origin: SceneCurveOrigin::Native,
                        ..
                    }),
                ) if index.span(span).is_some() => Some(span),
                _ => None,
            })
        {
            return Ok(self.describe_target(OffsetAuthoringTarget::Span(span)));
        }
        let model = scene.viewport.screen_to_model(position);
        Ok(match index.face_at_point(model) {
            OffsetFaceLookup::Hit(key) => self.describe_target(OffsetAuthoringTarget::Face(key)),
            OffsetFaceLookup::None | OffsetFaceLookup::BoundaryAmbiguous { .. } => None,
        })
    }

    fn describe_target(&self, target: OffsetAuthoringTarget) -> Option<OffsetAuthoringHover> {
        let index = self.index.as_ref()?;
        match &target {
            OffsetAuthoringTarget::Face(key) => {
                index.face(key)?;
            }
            OffsetAuthoringTarget::Span(span) => {
                index.span(*span)?;
            }
        }
        let mut probe = self.clone();
        probe.hover = None;
        let preflight = match target.clone() {
            OffsetAuthoringTarget::Face(key) => probe.pick_face(key),
            OffsetAuthoringTarget::Span(span) => probe.pick_span(span),
        };
        let availability = match preflight {
            OffsetAuthoringOutcome::OperandChanged { .. } => {
                OffsetAuthoringTargetAvailability::Available
            }
            OffsetAuthoringOutcome::Warning(warning) => {
                OffsetAuthoringTargetAvailability::Unavailable {
                    kind: warning.kind,
                    message: warning.message,
                }
            }
            OffsetAuthoringOutcome::Inactive
            | OffsetAuthoringOutcome::ModeEntered(_)
            | OffsetAuthoringOutcome::HoverChanged(_)
            | OffsetAuthoringOutcome::DistanceChanged { .. }
            | OffsetAuthoringOutcome::ApplyRequested(_)
            | OffsetAuthoringOutcome::ModeExited => {
                return None;
            }
        };
        Some(OffsetAuthoringHover {
            target,
            availability,
        })
    }

    fn pick_face(&mut self, key: OffsetFaceKey) -> OffsetAuthoringOutcome {
        let Some(index) = self.index.as_ref() else {
            return OffsetAuthoringOutcome::Inactive;
        };
        let Some(face) = index.face(&key) else {
            return self.warning(
                OffsetAuthoringWarningKind::StaleInput,
                "The selected face no longer belongs to this Offset snapshot",
            );
        };
        if !face.eligibility.is_eligible() {
            return self.warning(
                OffsetAuthoringWarningKind::UnsupportedOperand,
                "This face contains geometry that cannot be offset exactly",
            );
        }
        let direction = if std::mem::take(&mut self.pending_direction_flip) {
            DocumentFaceOffsetDirection::Inward
        } else {
            DocumentFaceOffsetDirection::Outward
        };
        self.operand = Some(OffsetAuthoringOperand::Face { key, direction });
        self.addition_history.clear();
        OffsetAuthoringOutcome::OperandChanged {
            operand: self.operand.clone(),
            guidance: self.guidance(),
        }
    }

    #[allow(clippy::too_many_lines)]
    fn pick_span(&mut self, span: geosolve_sketch::CurveSpan) -> OffsetAuthoringOutcome {
        let Some(index) = self.index.as_ref() else {
            return OffsetAuthoringOutcome::Inactive;
        };
        let Some(candidate) = index.span(span) else {
            return self.warning(
                OffsetAuthoringWarningKind::StaleInput,
                "The selected span no longer belongs to this Offset snapshot",
            );
        };
        if !candidate.eligibility.is_eligible() {
            return self.warning(
                OffsetAuthoringWarningKind::UnsupportedOperand,
                "This curve cannot participate in an exact native offset",
            );
        }
        if candidate.periodic {
            return self.warning(
                OffsetAuthoringWarningKind::PeriodicChain,
                "A full circle must be selected as a face, not collected as an open chain",
            );
        }
        let current = match self.operand.as_ref() {
            None => {
                let side = if std::mem::take(&mut self.pending_direction_flip) {
                    DocumentLineSide::Right
                } else {
                    DocumentLineSide::Left
                };
                self.operand = Some(OffsetAuthoringOperand::OpenChain {
                    spans: vec![OffsetDirectedSpan {
                        span,
                        traversal: OffsetTraversal::Forward,
                    }],
                    side,
                });
                self.addition_history.push(Vec::new());
                return OffsetAuthoringOutcome::OperandChanged {
                    operand: self.operand.clone(),
                    guidance: self.guidance(),
                };
            }
            Some(OffsetAuthoringOperand::Face { .. }) => {
                return self.warning(
                    OffsetAuthoringWarningKind::UnsupportedOperand,
                    "Reset the selected face before collecting an open chain",
                );
            }
            Some(OffsetAuthoringOperand::OpenChain { spans, .. }) => spans.clone(),
        };
        if let Some(selected_index) = current.iter().position(|selected| selected.span == span) {
            if selected_index != 0 && selected_index + 1 != current.len() {
                return self.warning(
                    OffsetAuthoringWarningKind::DuplicateSpan,
                    "An interior chain span cannot be removed out of order",
                );
            }
            let mut next = current;
            next.remove(selected_index);
            if let Some(OffsetAuthoringOperand::OpenChain { spans, .. }) = &mut self.operand {
                *spans = next;
                if spans.is_empty() {
                    self.operand = None;
                }
            }
            self.addition_history.clear();
            return OffsetAuthoringOutcome::OperandChanged {
                operand: self.operand.clone(),
                guidance: self.guidance(),
            };
        }
        if current.len() >= MAX_OFFSET_CHAIN_SPANS {
            return self.warning(
                OffsetAuthoringWarningKind::ChainLimitExceeded,
                "An Offset chain may contain at most 256 spans",
            );
        }
        let prospective_spans = current
            .iter()
            .map(|directed| directed.span)
            .chain(std::iter::once(span))
            .collect::<BTreeSet<_>>();
        if selected_branch_endpoint(index, &prospective_spans).is_some() {
            return self.warning(
                OffsetAuthoringWarningKind::BranchingJoin,
                "The selected curves would branch instead of forming one continuous chain",
            );
        }

        let front = directed_endpoint(current[0], true);
        let back = directed_endpoint(current[current.len() - 1], false);
        let native_start = OffsetEndpointRef {
            span,
            endpoint: OffsetEndpointRole::Start,
        };
        let native_end = OffsetEndpointRef {
            span,
            endpoint: OffsetEndpointRole::End,
        };
        let mut attachments = Vec::new();
        for (end, selected_terminal, at_front, traversal) in [
            (native_end, front, true, OffsetTraversal::Forward),
            (native_start, front, true, OffsetTraversal::Reverse),
            (native_start, back, false, OffsetTraversal::Forward),
            (native_end, back, false, OffsetTraversal::Reverse),
        ] {
            if index
                .adjacent_endpoints(end)
                .any(|adjacent| adjacent == selected_terminal)
            {
                attachments.push((at_front, traversal, end, selected_terminal));
            }
        }
        attachments.sort_unstable_by_key(|(front, traversal, end, terminal)| {
            (*front, traversal_order(*traversal), *end, *terminal)
        });
        attachments.dedup();
        if attachments.is_empty() {
            return self.warning(
                OffsetAuthoringWarningKind::DisconnectedSpan,
                "The selected curve is not connected to either open-chain terminal",
            );
        }
        if attachments.len() > 1 {
            let closes =
                attachments.iter().any(|value| value.0) && attachments.iter().any(|value| !value.0);
            return self.warning(
                if closes {
                    OffsetAuthoringWarningKind::WouldCloseChain
                } else {
                    OffsetAuthoringWarningKind::AmbiguousJoin
                },
                if closes {
                    "This curve would close the operand; select the bounded face instead"
                } else {
                    "The selected curve has more than one possible terminal attachment"
                },
            );
        }
        let (at_front, traversal, _, _) = attachments[0];
        self.addition_history.push(current.clone());
        if let Some(OffsetAuthoringOperand::OpenChain { spans, .. }) = &mut self.operand {
            let directed = OffsetDirectedSpan { span, traversal };
            if at_front {
                spans.insert(0, directed);
            } else {
                spans.push(directed);
            }
        }
        OffsetAuthoringOutcome::OperandChanged {
            operand: self.operand.clone(),
            guidance: self.guidance(),
        }
    }

    fn flip_operand_direction(&mut self) {
        match self.operand.as_mut() {
            Some(OffsetAuthoringOperand::Face { direction, .. }) => {
                *direction = match direction {
                    DocumentFaceOffsetDirection::Outward => DocumentFaceOffsetDirection::Inward,
                    DocumentFaceOffsetDirection::Inward => DocumentFaceOffsetDirection::Outward,
                };
            }
            Some(OffsetAuthoringOperand::OpenChain { side, .. }) => {
                *side = match side {
                    DocumentLineSide::Left => DocumentLineSide::Right,
                    DocumentLineSide::Right => DocumentLineSide::Left,
                };
            }
            None => self.pending_direction_flip = !self.pending_direction_flip,
        }
    }

    fn warning(
        &self,
        kind: OffsetAuthoringWarningKind,
        message: impl Into<String>,
    ) -> OffsetAuthoringOutcome {
        self.warning_value(kind, message)
    }

    fn warning_value(
        &self,
        kind: OffsetAuthoringWarningKind,
        message: impl Into<String>,
    ) -> OffsetAuthoringOutcome {
        OffsetAuthoringOutcome::Warning(OffsetAuthoringWarning {
            kind,
            stage: self.guidance().stage,
            message: message.into(),
        })
    }
}

fn finite_positive(value: f64) -> bool {
    value.is_finite() && value > 0.0
}

const fn traversal_order(traversal: OffsetTraversal) -> u8 {
    match traversal {
        OffsetTraversal::Forward => 0,
        OffsetTraversal::Reverse => 1,
    }
}

const fn directed_endpoint(span: OffsetDirectedSpan, start: bool) -> OffsetEndpointRef {
    let endpoint = match (span.traversal, start) {
        (OffsetTraversal::Forward, true) | (OffsetTraversal::Reverse, false) => {
            OffsetEndpointRole::Start
        }
        (OffsetTraversal::Forward, false) | (OffsetTraversal::Reverse, true) => {
            OffsetEndpointRole::End
        }
    };
    OffsetEndpointRef {
        span: span.span,
        endpoint,
    }
}

fn endpoint_position(index: &OffsetOperandIndex, endpoint: OffsetEndpointRef) -> Option<[f64; 2]> {
    index
        .span(endpoint.span)?
        .endpoints
        .iter()
        .find(|candidate| candidate.endpoint == endpoint)
        .map(|candidate| candidate.position)
}

/// Finds a branch inside the proposed operand; incident unselected geometry does not contribute.
fn selected_branch_endpoint(
    index: &OffsetOperandIndex,
    selected_spans: &BTreeSet<geosolve_sketch::CurveSpan>,
) -> Option<OffsetEndpointRef> {
    selected_spans.iter().find_map(|span| {
        index.span(*span)?.endpoints.iter().find_map(|candidate| {
            (index
                .adjacent_endpoints(candidate.endpoint)
                .filter(|adjacent| selected_spans.contains(&adjacent.span))
                .take(2)
                .count()
                > 1)
            .then_some(candidate.endpoint)
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use geosolve_sketch::{
        CurveDefinition, CurveSpan, DocumentFilletTrimEndpoint, DocumentSolveRequest,
        OperationControl, OperationOutcome, RetainedSketchDocumentSession, SketchDocument,
    };
    use geosolve_sketch_topology::{
        OffsetEndpointEligibility, OffsetOperandRequest, PreparedOffsetOperandQuery,
    };

    use crate::{
        ComputedConstructionFragmentId, ComputedConstructionFragmentProvenance, ComputedCornerRef,
        ComputedEvaluationRevision, ComputedFeatureCornerId, ComputedFeatureId,
        ComputedSourceInterval, NativeCurveSpanSource, SceneCurveOrigin, Viewport,
    };

    fn add_line(
        document: &mut SketchDocument,
        label: &str,
        start: geosolve_sketch::DesignPointId,
        end: geosolve_sketch::DesignPointId,
    ) -> CurveSpan {
        let start_position = document.point(start).expect("start").position;
        let end_position = document.point(end).expect("end").position;
        let delta = [
            end_position[0] - start_position[0],
            end_position[1] - start_position[1],
        ];
        let length = delta[0].hypot(delta[1]);
        CurveSpan::line(
            document
                .add_curve(
                    label,
                    CurveDefinition::Line {
                        start,
                        end,
                        branch_direction: [delta[0] / length, delta[1] / length],
                    },
                )
                .expect("line"),
        )
    }

    fn fixture(
        document: SketchDocument,
    ) -> (
        RetainedSketchDocumentSession,
        Arc<OffsetOperandIndex>,
        EditorScene,
    ) {
        let session = RetainedSketchDocumentSession::new(
            document,
            DocumentSolveRequest::default(),
            geosolve_sketch::SolverConfig::default(),
        )
        .expect("session");
        let query = PreparedOffsetOperandQuery::capture(&session, OffsetOperandRequest::default())
            .expect("query");
        let OperationOutcome::Completed { value, .. } = query
            .execute(OperationControl::unlimited())
            .expect("query execution")
        else {
            panic!("query unexpectedly stopped");
        };
        let index = Arc::new(value.operand_index.expect("complete index"));
        let accepted = session
            .accepted_state_for_current_input()
            .expect("accepted state");
        let viewport = Viewport::new([800.0, 600.0], [0.0, 0.0], 50.0).expect("viewport");
        let scene = EditorScene::from_accepted_for_design(
            accepted.identity().revision().get(),
            session.design_identity(),
            accepted.document(),
            session.design_document(),
            viewport,
            0.5,
        )
        .expect("scene")
        .with_retained_session(&session)
        .expect("bound scene");
        (session, index, scene)
    }

    #[test]
    fn face_interior_and_boundary_curve_use_one_target_resolver_with_curve_precedence() {
        let mut document = SketchDocument::new(1.0).expect("document");
        let points = [
            document.add_point("a", [-2.0, -2.0]).unwrap(),
            document.add_point("b", [2.0, -2.0]).unwrap(),
            document.add_point("c", [2.0, 2.0]).unwrap(),
            document.add_point("d", [-2.0, 2.0]).unwrap(),
        ];
        let bottom = add_line(&mut document, "bottom", points[0], points[1]);
        add_line(&mut document, "right", points[1], points[2]);
        add_line(&mut document, "top", points[2], points[3]);
        add_line(&mut document, "left", points[3], points[0]);
        let (_, index, scene) = fixture(document);
        let mut state = OffsetAuthoringState::default();
        assert!(matches!(
            state.activate(index, 1.0),
            OffsetAuthoringOutcome::ModeEntered(_)
        ));

        let interior = scene.viewport.model_to_screen([0.0, 0.0]);
        let hover = state.hover_at(
            &scene,
            interior,
            PickTolerance::default(),
            GeometryInteractionPolicy::default(),
        );
        let OffsetAuthoringOutcome::HoverChanged(Some(OffsetAuthoringHover {
            target: OffsetAuthoringTarget::Face(key),
            availability: OffsetAuthoringTargetAvailability::Available,
        })) = hover
        else {
            panic!("face hover expected");
        };
        assert!(matches!(
            state.pick_at(
                &scene,
                interior,
                PickTolerance::default(),
                GeometryInteractionPolicy::default(),
            ),
            OffsetAuthoringOutcome::OperandChanged {
                operand: Some(OffsetAuthoringOperand::Face { key: selected, .. }),
                ..
            } if selected == key
        ));

        let _ = state.reset();
        let boundary = scene.viewport.model_to_screen([0.0, -2.0]);
        assert!(matches!(
            state.hover_at(
                &scene,
                boundary,
                PickTolerance::default(),
                GeometryInteractionPolicy::default(),
            ),
            OffsetAuthoringOutcome::HoverChanged(Some(OffsetAuthoringHover {
                target: OffsetAuthoringTarget::Span(span),
                availability: OffsetAuthoringTargetAvailability::Available,
            }))
                if span == bottom
        ));
        assert!(matches!(
            state.pick_at(
                &scene,
                boundary,
                PickTolerance::default(),
                GeometryInteractionPolicy::default(),
            ),
            OffsetAuthoringOutcome::OperandChanged {
                operand: Some(OffsetAuthoringOperand::OpenChain { spans, .. }),
                ..
            } if spans == vec![OffsetDirectedSpan { span: bottom, traversal: OffsetTraversal::Forward }]
        ));
    }

    #[test]
    fn m80_f001_computed_fillet_fragment_cannot_masquerade_as_a_native_offset_operand() {
        let mut document = SketchDocument::new(1.0).expect("document");
        let start = document.add_point("start", [-2.0, 0.0]).unwrap();
        let end = document.add_point("end", [2.0, 0.0]).unwrap();
        let span = add_line(&mut document, "source", start, end);
        let (_, index, mut scene) = fixture(document);
        let curve = scene
            .curves
            .iter_mut()
            .find(|curve| curve.span == span)
            .expect("native source occurrence");
        curve.origin = SceneCurveOrigin::FilletDiscarded {
            fragment: ComputedConstructionFragmentId {
                evaluation: ComputedEvaluationRevision::from_raw(1),
                ordinal: 0,
            },
            source: NativeCurveSpanSource { span },
            interval: ComputedSourceInterval {
                start: 0.25,
                end: 0.75,
            },
            provenance: ComputedConstructionFragmentProvenance {
                owner: ComputedCornerRef {
                    feature: ComputedFeatureId::from_raw(1),
                    corner: ComputedFeatureCornerId::from_raw(1),
                },
                endpoint: DocumentFilletTrimEndpoint::End,
                base_interval: ComputedSourceInterval {
                    start: 0.0,
                    end: 1.0,
                },
            },
        };

        let mut state = OffsetAuthoringState::default();
        assert!(matches!(
            state.activate(index, 1.0),
            OffsetAuthoringOutcome::ModeEntered(_)
        ));
        let midpoint = scene.viewport.model_to_screen([0.0, 0.0]);
        assert_eq!(
            state.hover_at(
                &scene,
                midpoint,
                PickTolerance::default(),
                GeometryInteractionPolicy::default(),
            ),
            OffsetAuthoringOutcome::HoverChanged(None)
        );
        assert!(matches!(
            state.pick_at(
                &scene,
                midpoint,
                PickTolerance::default(),
                GeometryInteractionPolicy::default(),
            ),
            OffsetAuthoringOutcome::Warning(OffsetAuthoringWarning {
                kind: OffsetAuthoringWarningKind::NoTarget,
                ..
            })
        ));
        assert!(state.operand().is_none());
    }

    #[test]
    fn chain_collection_is_ordered_bounded_and_terminal_removal_is_explicit() {
        let mut document = SketchDocument::new(1.0).expect("document");
        let points = [
            document.add_point("a", [-3.0, 0.0]).unwrap(),
            document.add_point("b", [-1.0, 0.0]).unwrap(),
            document.add_point("c", [1.0, 0.0]).unwrap(),
            document.add_point("d", [3.0, 0.0]).unwrap(),
        ];
        let first = add_line(&mut document, "first", points[0], points[1]);
        let middle = add_line(&mut document, "middle", points[1], points[2]);
        let last = add_line(&mut document, "last", points[2], points[3]);
        let (_, index, scene) = fixture(document);
        let mut state = OffsetAuthoringState::default();
        let _ = state.activate(index, 1.0);
        for (span, model) in [
            (middle, [0.0, 0.0]),
            (first, [-2.0, 0.0]),
            (last, [2.0, 0.0]),
        ] {
            assert!(matches!(
                state.pick_at(
                    &scene,
                    scene.viewport.model_to_screen(model),
                    PickTolerance::default(),
                    GeometryInteractionPolicy::default(),
                ),
                OffsetAuthoringOutcome::OperandChanged { .. }
            ));
            assert!(
                state
                    .operand()
                    .is_some_and(|operand| operand.span_count() <= 3)
            );
            let _ = span;
        }
        let Some(OffsetAuthoringOperand::OpenChain { spans, .. }) = state.operand() else {
            panic!("chain");
        };
        assert_eq!(
            spans.iter().map(|span| span.span).collect::<Vec<_>>(),
            vec![first, middle, last]
        );

        assert!(matches!(
            state.pick_at(
                &scene,
                scene.viewport.model_to_screen([0.0, 0.0]),
                PickTolerance::default(),
                GeometryInteractionPolicy::default(),
            ),
            OffsetAuthoringOutcome::Warning(OffsetAuthoringWarning {
                kind: OffsetAuthoringWarningKind::DuplicateSpan,
                ..
            })
        ));
        assert!(matches!(
            state.pick_at(
                &scene,
                scene.viewport.model_to_screen([2.0, 0.0]),
                PickTolerance::default(),
                GeometryInteractionPolicy::default(),
            ),
            OffsetAuthoringOutcome::OperandChanged { .. }
        ));
        assert_eq!(
            state.operand().map(OffsetAuthoringOperand::span_count),
            Some(2)
        );
    }

    #[test]
    fn m80_f006_unselected_incident_branch_does_not_block_a_continuous_chain() {
        let mut document = SketchDocument::new(10.0).expect("document");
        let start = document.add_point("start", [0.0, 0.0]).unwrap();
        let junction = document.add_point("junction", [2.0, 0.0]).unwrap();
        let end = document.add_point("end", [2.0, 2.0]).unwrap();
        let branch_end = document.add_point("branch end", [4.0, 0.0]).unwrap();
        let isolated_start = document.add_point("isolated start", [6.0, 0.0]).unwrap();
        let isolated_end = document.add_point("isolated end", [8.0, 0.0]).unwrap();
        let first = add_line(&mut document, "first", start, junction);
        let second = add_line(&mut document, "second", junction, end);
        let closing = add_line(&mut document, "closing", end, start);
        let branch = add_line(&mut document, "unselected branch", junction, branch_end);
        let isolated = add_line(&mut document, "isolated", isolated_start, isolated_end);
        let (_, index, _) = fixture(document);

        let first_end = index
            .span(first)
            .expect("first span")
            .endpoints
            .iter()
            .find(|candidate| candidate.endpoint.endpoint == OffsetEndpointRole::End)
            .expect("first end");
        assert_eq!(
            first_end.eligibility,
            OffsetEndpointEligibility::Branched { adjacent: 2 },
            "the topology index must retain the truthful global junction degree"
        );

        let mut state = OffsetAuthoringState::default();
        let _ = state.activate(index, 10.0);
        assert!(matches!(
            state.pick_target(OffsetAuthoringTarget::Span(first)),
            OffsetAuthoringOutcome::OperandChanged { .. }
        ));
        assert!(matches!(
            state.pick_target(OffsetAuthoringTarget::Span(second)),
            OffsetAuthoringOutcome::OperandChanged { .. }
        ));
        assert!(matches!(
            state.operand(),
            Some(OffsetAuthoringOperand::OpenChain { spans, .. })
                if spans == &vec![
                    OffsetDirectedSpan {
                        span: first,
                        traversal: OffsetTraversal::Forward,
                    },
                    OffsetDirectedSpan {
                        span: second,
                        traversal: OffsetTraversal::Forward,
                    },
                ]
        ));

        assert!(matches!(
            state.pick_target(OffsetAuthoringTarget::Span(branch)),
            OffsetAuthoringOutcome::Warning(OffsetAuthoringWarning {
                kind: OffsetAuthoringWarningKind::BranchingJoin,
                ..
            })
        ));
        assert!(matches!(
            state.pick_target(OffsetAuthoringTarget::Span(closing)),
            OffsetAuthoringOutcome::Warning(OffsetAuthoringWarning {
                kind: OffsetAuthoringWarningKind::WouldCloseChain,
                ..
            })
        ));
        assert!(matches!(
            state.pick_target(OffsetAuthoringTarget::Span(isolated)),
            OffsetAuthoringOutcome::Warning(OffsetAuthoringWarning {
                kind: OffsetAuthoringWarningKind::DisconnectedSpan,
                ..
            })
        ));
        assert!(matches!(
            state.operand(),
            Some(OffsetAuthoringOperand::OpenChain { spans, .. })
                if spans.len() == 2
                    && spans[0].span == first
                    && spans[1].span == second
        ));
    }

    #[test]
    fn negative_distance_flips_direction_and_only_positive_value_is_candidate_state() {
        let mut document = SketchDocument::new(10.0).expect("document");
        let first = document.add_point("a", [-1.0, 0.0]).unwrap();
        let second = document.add_point("b", [1.0, 0.0]).unwrap();
        add_line(&mut document, "line", first, second);
        let (_, index, scene) = fixture(document);
        let mut state = OffsetAuthoringState::default();
        let _ = state.activate(index, 10.0);
        let _ = state.pick_at(
            &scene,
            scene.viewport.model_to_screen([0.0, 0.0]),
            PickTolerance::default(),
            GeometryInteractionPolicy::default(),
        );
        assert!(matches!(
            state.operand(),
            Some(OffsetAuthoringOperand::OpenChain {
                side: DocumentLineSide::Left,
                ..
            })
        ));
        assert!(matches!(
            state.set_distance(-2.5),
            OffsetAuthoringOutcome::DistanceChanged { distance, .. }
                if distance.to_bits() == 2.5_f64.to_bits()
        ));
        assert!(matches!(
            state.candidate(),
            Some(OffsetAuthoringCandidate {
                distance: 2.5,
                operand: OffsetAuthoringOperand::OpenChain {
                    side: DocumentLineSide::Right,
                    ..
                },
                ..
            })
        ));
        assert!(matches!(
            state.set_distance(0.0),
            OffsetAuthoringOutcome::Warning(OffsetAuthoringWarning {
                kind: OffsetAuthoringWarningKind::InvalidDistance,
                ..
            })
        ));
        assert!(state.candidate().is_none());
        assert_eq!(state.remembered_distance(), Some(2.5));
    }

    #[test]
    fn signed_distance_before_operand_deterministically_sets_the_next_face_or_chain_direction() {
        let mut document = SketchDocument::new(10.0).expect("document");
        let square_points = [
            document.add_point("a", [-2.0, -2.0]).unwrap(),
            document.add_point("b", [2.0, -2.0]).unwrap(),
            document.add_point("c", [2.0, 2.0]).unwrap(),
            document.add_point("d", [-2.0, 2.0]).unwrap(),
        ];
        add_line(&mut document, "bottom", square_points[0], square_points[1]);
        add_line(&mut document, "right", square_points[1], square_points[2]);
        add_line(&mut document, "top", square_points[2], square_points[3]);
        add_line(&mut document, "left", square_points[3], square_points[0]);
        let open_start = document.add_point("open start", [5.0, 0.0]).unwrap();
        let open_end = document.add_point("open end", [8.0, 0.0]).unwrap();
        let open = add_line(&mut document, "open", open_start, open_end);
        let (_, index, _) = fixture(document);
        let face = index
            .faces()
            .iter()
            .find(|face| face.eligibility.is_eligible())
            .expect("eligible square face")
            .key
            .clone();

        let mut state = OffsetAuthoringState::default();
        let _ = state.activate(Arc::clone(&index), 10.0);
        let _ = state.set_distance(-2.5);
        assert!(matches!(
            state.pick_target(OffsetAuthoringTarget::Face(face.clone())),
            OffsetAuthoringOutcome::OperandChanged {
                operand: Some(OffsetAuthoringOperand::Face {
                    direction: DocumentFaceOffsetDirection::Inward,
                    ..
                }),
                ..
            }
        ));

        let _ = state.reset();
        let _ = state.set_distance(-3.0);
        let _ = state.set_distance(3.0);
        assert!(matches!(
            state.pick_target(OffsetAuthoringTarget::Face(face)),
            OffsetAuthoringOutcome::OperandChanged {
                operand: Some(OffsetAuthoringOperand::Face {
                    direction: DocumentFaceOffsetDirection::Outward,
                    ..
                }),
                ..
            }
        ));

        let _ = state.reset();
        let _ = state.set_distance(-4.0);
        assert!(matches!(
            state.pick_target(OffsetAuthoringTarget::Span(open)),
            OffsetAuthoringOutcome::OperandChanged {
                operand: Some(OffsetAuthoringOperand::OpenChain {
                    side: DocumentLineSide::Right,
                    ..
                }),
                ..
            }
        ));
    }

    #[test]
    fn pending_signed_direction_is_transient_across_cancel_and_reactivation() {
        let mut document = SketchDocument::new(10.0).expect("document");
        let first = document.add_point("a", [-1.0, 0.0]).unwrap();
        let second = document.add_point("b", [1.0, 0.0]).unwrap();
        let span = add_line(&mut document, "line", first, second);
        let (_, index, _) = fixture(document);

        let mut state = OffsetAuthoringState::default();
        let _ = state.activate(Arc::clone(&index), 10.0);
        let _ = state.set_distance(-2.5);
        let _ = state.reset();
        assert!(matches!(
            state.pick_target(OffsetAuthoringTarget::Span(span)),
            OffsetAuthoringOutcome::OperandChanged {
                operand: Some(OffsetAuthoringOperand::OpenChain {
                    side: DocumentLineSide::Left,
                    ..
                }),
                ..
            }
        ));
        let _ = state.reset();
        let _ = state.set_distance(-2.5);
        let _ = state.cancel();
        let _ = state.activate(index, 10.0);
        assert_eq!(state.distance(), Some(2.5));
        assert!(matches!(
            state.pick_target(OffsetAuthoringTarget::Span(span)),
            OffsetAuthoringOutcome::OperandChanged {
                operand: Some(OffsetAuthoringOperand::OpenChain {
                    side: DocumentLineSide::Left,
                    ..
                }),
                ..
            }
        ));
    }

    #[test]
    fn unsupported_native_hover_is_typed_unavailable_and_pick_uses_the_same_target() {
        let mut document = SketchDocument::new(10.0).expect("document");
        let controls = [
            document.add_point("q0", [-2.0, 0.0]).unwrap(),
            document.add_point("q1", [0.0, 2.0]).unwrap(),
            document.add_point("q2", [2.0, 0.0]).unwrap(),
        ];
        let span = CurveSpan::line(
            document
                .add_curve("quadratic", CurveDefinition::QuadraticBezier { controls })
                .unwrap(),
        );
        let (_, index, scene) = fixture(document);
        let mut state = OffsetAuthoringState::default();
        let _ = state.activate(index, 10.0);
        let midpoint = scene.viewport.model_to_screen([0.0, 1.0]);

        assert!(matches!(
            state.hover_at(
                &scene,
                midpoint,
                PickTolerance::default(),
                GeometryInteractionPolicy::default(),
            ),
            OffsetAuthoringOutcome::HoverChanged(Some(OffsetAuthoringHover {
                target: OffsetAuthoringTarget::Span(target),
                availability: OffsetAuthoringTargetAvailability::Unavailable {
                    kind: OffsetAuthoringWarningKind::UnsupportedOperand,
                    ..
                },
            })) if target == span
        ));
        assert!(matches!(
            state.pick_at(
                &scene,
                midpoint,
                PickTolerance::default(),
                GeometryInteractionPolicy::default(),
            ),
            OffsetAuthoringOutcome::Warning(OffsetAuthoringWarning {
                kind: OffsetAuthoringWarningKind::UnsupportedOperand,
                ..
            })
        ));
        assert_eq!(
            state.hover_target(),
            Some(&OffsetAuthoringTarget::Span(span))
        );
        assert!(state.operand().is_none());
    }

    #[test]
    fn dynamically_disconnected_chain_hover_preflights_as_unavailable_before_pick() {
        let mut document = SketchDocument::new(10.0).expect("document");
        let first_start = document.add_point("first start", [-4.0, 0.0]).unwrap();
        let first_end = document.add_point("first end", [-2.0, 0.0]).unwrap();
        let second_start = document.add_point("second start", [2.0, 0.0]).unwrap();
        let second_end = document.add_point("second end", [4.0, 0.0]).unwrap();
        let first = add_line(&mut document, "first", first_start, first_end);
        let second = add_line(&mut document, "second", second_start, second_end);
        let (_, index, scene) = fixture(document);
        let mut state = OffsetAuthoringState::default();
        let _ = state.activate(index, 10.0);
        assert!(matches!(
            state.pick_target(OffsetAuthoringTarget::Span(first)),
            OffsetAuthoringOutcome::OperandChanged { .. }
        ));
        let second_midpoint = scene.viewport.model_to_screen([3.0, 0.0]);

        assert!(matches!(
            state.hover_at(
                &scene,
                second_midpoint,
                PickTolerance::default(),
                GeometryInteractionPolicy::default(),
            ),
            OffsetAuthoringOutcome::HoverChanged(Some(OffsetAuthoringHover {
                target: OffsetAuthoringTarget::Span(target),
                availability: OffsetAuthoringTargetAvailability::Unavailable {
                    kind: OffsetAuthoringWarningKind::DisconnectedSpan,
                    ..
                },
            })) if target == second
        ));
        assert!(matches!(
            state.pick_at(
                &scene,
                second_midpoint,
                PickTolerance::default(),
                GeometryInteractionPolicy::default(),
            ),
            OffsetAuthoringOutcome::Warning(OffsetAuthoringWarning {
                kind: OffsetAuthoringWarningKind::DisconnectedSpan,
                ..
            })
        ));
        assert!(matches!(
            state.operand(),
            Some(OffsetAuthoringOperand::OpenChain { spans, .. })
                if spans.len() == 1 && spans[0].span == first
        ));
    }

    #[test]
    fn semantic_chain_presentation_preserves_order_reversal_and_terminal_positions() {
        let mut document = SketchDocument::new(10.0).expect("document");
        let points = [
            document.add_point("a", [-3.0, 0.0]).unwrap(),
            document.add_point("b", [-1.0, 0.0]).unwrap(),
            document.add_point("c", [1.0, 0.0]).unwrap(),
            document.add_point("d", [3.0, 0.0]).unwrap(),
        ];
        let first = add_line(&mut document, "first", points[1], points[0]);
        let middle = add_line(&mut document, "middle", points[1], points[2]);
        let last = add_line(&mut document, "last", points[3], points[2]);
        let (_, index, _) = fixture(document);
        let mut state = OffsetAuthoringState::default();
        let _ = state.activate(index, 10.0);
        for span in [middle, first, last] {
            assert!(matches!(
                state.pick_target(OffsetAuthoringTarget::Span(span)),
                OffsetAuthoringOutcome::OperandChanged { .. }
            ));
        }

        let presentation = state.chain_presentation().expect("ordered chain DTO");
        assert_eq!(
            presentation.spans,
            vec![
                OffsetDirectedSpan {
                    span: first,
                    traversal: OffsetTraversal::Reverse,
                },
                OffsetDirectedSpan {
                    span: middle,
                    traversal: OffsetTraversal::Forward,
                },
                OffsetDirectedSpan {
                    span: last,
                    traversal: OffsetTraversal::Reverse,
                },
            ]
        );
        assert_eq!(presentation.start.endpoint.span, first);
        assert_eq!(
            presentation.start.endpoint.endpoint,
            OffsetEndpointRole::End
        );
        assert_eq!(
            presentation.start.model_position.map(f64::to_bits),
            [(-3.0_f64).to_bits(), 0.0_f64.to_bits()]
        );
        assert_eq!(presentation.end.endpoint.span, last);
        assert_eq!(
            presentation.end.endpoint.endpoint,
            OffsetEndpointRole::Start
        );
        assert_eq!(
            presentation.end.model_position.map(f64::to_bits),
            [3.0_f64.to_bits(), 0.0_f64.to_bits()]
        );
    }

    #[test]
    fn cancel_and_reactivate_reuse_only_the_last_valid_distance_or_model_scale_fallback() {
        let mut document = SketchDocument::new(10.0).expect("document");
        let first = document.add_point("a", [-1.0, 0.0]).unwrap();
        let second = document.add_point("b", [1.0, 0.0]).unwrap();
        add_line(&mut document, "line", first, second);
        let (_, index, _) = fixture(document);

        let mut fresh = OffsetAuthoringState::default();
        let _ = fresh.activate(Arc::clone(&index), 20.0);
        assert_eq!(fresh.distance(), Some(2.0));
        assert_eq!(fresh.remembered_distance(), None);
        assert!(matches!(fresh.cancel(), OffsetAuthoringOutcome::ModeExited));

        let mut remembered = OffsetAuthoringState::default();
        let _ = remembered.activate(Arc::clone(&index), 10.0);
        let _ = remembered.set_distance(2.5);
        assert!(matches!(
            remembered.cancel(),
            OffsetAuthoringOutcome::ModeExited
        ));
        assert!(!remembered.is_active());
        assert_eq!(remembered.distance(), None);
        assert_eq!(remembered.remembered_distance(), Some(2.5));

        let _ = remembered.activate(index, 100.0);
        assert!(remembered.is_active());
        assert_eq!(remembered.distance(), Some(2.5));
        assert_eq!(remembered.operand(), None);
        assert_eq!(remembered.hover_target(), None);
    }
}
