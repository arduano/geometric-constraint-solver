// SPDX-License-Identifier: GPL-3.0-or-later
//! Explicit semantic point gestures over the shared retained native continuation.
//! Working text and accepted source/history are never changed by this module.

use geosolve_constraint_editor::{
    DelegatedPointDragProposal, EditorEffect, IntentNativeBinding, InteractionWorkReceipt,
    Modifiers, PointerInput, ProjectionalEditorSession, Viewport,
};
use geosolve_sketch::{DesignPointId, SketchHardValidity};
use geosolve_sketch_code::{
    CodePointEdit, CodeRectangleCorner, CodeWritableAddress, ExpandedPort, ExpandedWritablePoint,
    materialize_code_project_incremental_with_overlay,
};
use geosolve_sketch_intent::IntentPortKind;
use serde::{Deserialize, Serialize};

use crate::{EditableSession, EngineError};

/// Hard bound on the ordered continuation trace retained for one gesture.
pub const MAX_POINT_GESTURE_SAMPLES: usize = 4_096;
const CHORD_TOLERANCE_PIXELS: f64 = 0.25;

fn error(value: impl std::fmt::Display) -> EngineError {
    EngineError::Admission(value.to_string())
}

/// Explicit generation-authenticated semantic lens, independent of any user's selection.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "target", rename_all = "snake_case", deny_unknown_fields)]
pub enum PointGestureTarget {
    Point {
        address: CodeWritableAddress,
    },
    RectangleCorner {
        lower_left: CodeWritableAddress,
        upper_right: CodeWritableAddress,
        corner: CodeRectangleCorner,
    },
}

impl PointGestureTarget {
    fn from_lens(lens: &ExpandedWritablePoint) -> Self {
        match &lens.edit {
            CodePointEdit::Point { address } => Self::Point {
                address: address.clone(),
            },
            CodePointEdit::RectangleCorner {
                lower_left,
                upper_right,
                corner,
                ..
            } => Self::RectangleCorner {
                lower_left: lower_left.clone(),
                upper_right: upper_right.clone(),
                corner: *corner,
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PointGestureHandle {
    pub target: PointGestureTarget,
    pub position: [f64; 2],
}

/// Model-space pointer target, in contiguous order starting with sequence 1.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PointGestureSample {
    pub sequence: u32,
    pub position: [f64; 2],
}

/// Replayable semantic intent. This wire value is never accepted native authority.
/// The host authenticates its document/user and lifecycle before invoking replay.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PointGestureCommand {
    pub basis: String,
    pub gesture_id: u64,
    pub target: PointGestureTarget,
    pub viewport: Viewport,
    pub samples: Vec<PointGestureSample>,
}

/// One provisional frame; native acceptance applies only to this isolated gesture.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PointGestureFrame {
    pub sequence: u32,
    pub accepted: bool,
    pub accepted_position: [f64; 2],
    pub work: InteractionWorkReceipt,
}

/// A linear checked native terminal plus its semantic command, never server publication.
#[derive(Debug)]
pub struct PointGestureTerminal {
    command: PointGestureCommand,
    pub(super) proposal: DelegatedPointDragProposal,
    pub(super) editor: Box<ProjectionalEditorSession>,
    pub(super) lens: ExpandedWritablePoint,
}

impl PointGestureTerminal {
    pub fn command(&self) -> &PointGestureCommand {
        &self.command
    }
    pub fn accepted_position(&self) -> [f64; 2] {
        self.proposal.accepted_position
    }
}

/// One warmed, isolated authoring coordinator. Drop or cancel restores its origin by
/// discarding the fork; no accepted engine session or nested history was mutated.
#[derive(Debug)]
pub struct RetainedPointGesture {
    editor: Box<ProjectionalEditorSession>,
    point: DesignPointId,
    command: PointGestureCommand,
    origin: [f64; 2],
    accepted_position: [f64; 2],
    lens: ExpandedWritablePoint,
}

impl EditableSession {
    /// Returns exact current semantic point lenses, including explicit producer/consumer
    /// identities when several lenses share the same visible native point.
    ///
    /// # Errors
    /// Rejects missing accepted native point ownership.
    pub fn point_gesture_targets(&self) -> Result<Vec<PointGestureHandle>, EngineError> {
        let materialized = &self.accepted().0.materialized;
        materialized
            .expansion
            .writable_points
            .iter()
            .map(|lens| {
                let point = expanded_port_point(&materialized.editor, &lens.handle)
                    .ok_or_else(|| error("semantic point has no accepted native owner"))?;
                Ok(PointGestureHandle {
                    target: PointGestureTarget::from_lens(lens),
                    position: accepted_point(&materialized.editor, point)?,
                })
            })
            .collect()
    }

    /// Forks accepted native authority once and starts an explicit semantic point gesture.
    /// Referenced consumers detach once at the origin through the shared point codec.
    ///
    /// # Errors
    /// Rejects invalid camera/gesture IDs, stale generations, absent/ambiguous lenses,
    /// non-writable native points or rejected reference detachment.
    pub fn begin_point_gesture(
        &self,
        target: PointGestureTarget,
        gesture_id: u64,
        viewport: Viewport,
    ) -> Result<RetainedPointGesture, EngineError> {
        Viewport::new(
            viewport.screen_size,
            viewport.model_center,
            viewport.pixels_per_model_unit,
        )
        .map_err(error)?;
        if gesture_id == 0 || gesture_id > geosolve_sketch_code::MAX_CODE_SESSION_WIRE_INTEGER {
            return Err(error("invalid point gesture identity"));
        }
        let materialized = &self.accepted().0.materialized;
        let candidates = materialized
            .expansion
            .writable_points
            .iter()
            .filter(|lens| PointGestureTarget::from_lens(lens) == target)
            .collect::<Vec<_>>();
        let [lens] = candidates.as_slice() else {
            return Err(error("point gesture target is absent, stale or ambiguous"));
        };
        let point = expanded_port_point(&materialized.editor, &lens.handle)
            .ok_or_else(|| error("semantic point has no native owner"))?;
        let origin = accepted_point(&materialized.editor, point)?;
        let mut editor = if lens.source.is_reference() {
            let snapshot = self.code_snapshot();
            let overlay = lens
                .stage_drag(&snapshot.interaction_overlay, origin)
                .map_err(error)?;
            let project = snapshot
                .code_project
                .as_ref()
                .ok_or_else(|| error("missing accepted managed project"))?;
            let detached = materialize_code_project_incremental_with_overlay(
                materialized,
                project,
                &snapshot.generated,
                &overlay,
            )
            .map_err(error)?;
            Box::new(detached.editor)
        } else {
            Box::new(
                materialized
                    .editor
                    .fork_accepted_authority()
                    .map_err(error)?,
            )
        };
        let point = expanded_port_point(&editor, &lens.handle)
            .ok_or_else(|| error("detached semantic point has no native owner"))?;
        let scene = editor
            .scene(viewport, CHORD_TOLERANCE_PIXELS)
            .map_err(error)?;
        editor
            .pointer_down_exact_point(&scene, pointer(gesture_id, viewport, origin)?, point)
            .map_err(error)?;
        Ok(RetainedPointGesture {
            editor,
            point,
            origin,
            accepted_position: origin,
            lens: (*lens).clone(),
            command: PointGestureCommand {
                basis: self.point_gesture_basis()?,
                gesture_id,
                target,
                viewport,
                samples: Vec::new(),
            },
        })
    }

    /// Recomputes a complete explicit gesture against an exact accepted source/design basis.
    /// Client positions are targets only; every frame and terminal cross native validation.
    ///
    /// # Errors
    /// Rejects stale basis, forged generations, malformed traces and unaccepted terminals.
    /// This never publishes source, semantic overlays or accepted session history.
    pub fn replay_point_gesture(
        &self,
        command: &PointGestureCommand,
    ) -> Result<PointGestureTerminal, EngineError> {
        if command.basis != self.point_gesture_basis()? {
            return Err(error("point gesture accepted source/design basis changed"));
        }
        if command.samples.len() > MAX_POINT_GESTURE_SAMPLES {
            return Err(error("point gesture sample limit exceeded"));
        }
        let mut gesture =
            self.begin_point_gesture(command.target.clone(), command.gesture_id, command.viewport)?;
        for sample in &command.samples {
            gesture.advance(command.gesture_id, *sample)?;
        }
        gesture.finish(command.gesture_id)
    }

    fn point_gesture_basis(&self) -> Result<String, EngineError> {
        self.source_design_digest()
    }
}

impl RetainedPointGesture {
    /// Processes exactly the next pointer sample through retained continuation.
    /// A rejected native solve preserves the preceding valid preview and remains in
    /// the ordered trace; malformed routing/sequence data does not consume a sample.
    ///
    /// # Errors
    /// Rejects wrong gestures, missing/repeated/out-of-order samples, nonfinite positions
    /// and exhausted trace bounds. Native errors preserve engine accepted authority.
    pub fn advance(
        &mut self,
        gesture_id: u64,
        sample: PointGestureSample,
    ) -> Result<PointGestureFrame, EngineError> {
        self.authenticate(gesture_id)?;
        if self.command.samples.len() >= MAX_POINT_GESTURE_SAMPLES {
            return Err(error("point gesture sample limit exceeded"));
        }
        let next = u32::try_from(self.command.samples.len() + 1).map_err(error)?;
        if sample.sequence != next {
            return Err(error(
                "point gesture sample is missing, repeated or out of order",
            ));
        }
        let input = pointer(gesture_id, self.command.viewport, sample.position)?;
        let scene = self
            .editor
            .scene_audited(self.command.viewport, CHORD_TOLERANCE_PIXELS);
        let mut work = scene.work;
        let scene = scene.outcome.map_err(error)?;
        let frame = self.editor.pointer_move_audited(&scene, input);
        work.merge(frame.work);
        // The coordinator may consume a request even if native preview fails. Keep the
        // exact ordered input so retry/replay cannot silently choose another branch path.
        self.command.samples.push(sample);
        let effects = frame.outcome.map_err(error)?;
        let accepted = effects.iter().any(|effect| matches!(effect, EditorEffect::PreviewPointMove { point, .. } if *point == self.point));
        self.accepted_position = accepted_point(&self.editor, self.point)?;
        Ok(PointGestureFrame {
            sequence: sample.sequence,
            accepted,
            accepted_position: self.accepted_position,
            work,
        })
    }

    /// A detached presentation payload cannot authorize editing in another coordinator.
    ///
    /// # Errors
    /// Rejects unavailable or unrepresentable provisional scenes.
    pub fn scene_json(&self) -> Result<String, EngineError> {
        self.editor
            .scene(self.command.viewport, CHORD_TOLERANCE_PIXELS)
            .map_err(error)?
            .to_detached_json()
            .map_err(error)
    }

    pub fn accepted_position(&self) -> [f64; 2] {
        self.accepted_position
    }
    pub fn sample_count(&self) -> usize {
        self.command.samples.len()
    }

    /// Consumes one exact delegated terminal without publishing native or source history.
    /// Whole-model source/overlay publication remains a separate checked host transaction.
    ///
    /// # Errors
    /// Rejects wrong gestures, no accepted movement or stale native terminal authority.
    pub fn finish(mut self, gesture_id: u64) -> Result<PointGestureTerminal, EngineError> {
        self.authenticate(gesture_id)?;
        let position = self
            .command
            .samples
            .last()
            .map_or(self.origin, |sample| sample.position);
        let scene = self
            .editor
            .scene(self.command.viewport, CHORD_TOLERANCE_PIXELS)
            .map_err(error)?;
        let outcome = self
            .editor
            .pointer_up_delegated_point(
                &scene,
                pointer(gesture_id, self.command.viewport, position)?,
            )
            .map_err(error)?;
        let proposal = outcome
            .proposal
            .ok_or_else(|| error("point gesture has no accepted terminal movement"))?;
        if proposal.point != self.point
            || proposal.pointer_id != gesture_id
            || proposal.intent != self.editor.coordinator().intent().identity()
            || proposal.accepted_position.map(f64::to_bits)
                != self.accepted_position.map(f64::to_bits)
        {
            return Err(error(
                "point gesture terminal differs from its authenticated native route",
            ));
        }
        let accepted = proposal
            .terminal_session()
            .accepted_state_for_current_input()
            .ok_or_else(|| error("point terminal has no current native acceptance"))?;
        let solve = accepted
            .diagnostics()
            .solve
            .ok_or_else(|| error("point terminal has no independent solve evidence"))?;
        if !solve.accepted
            || solve.hard_validity != SketchHardValidity::Valid
            || !solve.hard_residuals_validated
            || solve
                .maximum_normalized_hard_residual
                .is_some_and(|residual| !residual.is_finite() || residual > 1.0e-9)
        {
            return Err(error(
                "point terminal failed independent hard-residual validation",
            ));
        }
        Ok(PointGestureTerminal {
            command: self.command,
            proposal,
            editor: self.editor,
            lens: self.lens,
        })
    }

    /// Cancels by consuming the isolated gesture; the accepted engine is untouched.
    pub fn cancel(mut self) {
        self.editor.cancel_interaction();
    }

    fn authenticate(&self, gesture_id: u64) -> Result<(), EngineError> {
        if gesture_id != self.command.gesture_id {
            return Err(error("point gesture belongs to a different pointer"));
        }
        Ok(())
    }
}

fn pointer(
    gesture_id: u64,
    viewport: Viewport,
    position: [f64; 2],
) -> Result<PointerInput, EngineError> {
    if !position.into_iter().all(f64::is_finite) {
        return Err(error("nonfinite point gesture target"));
    }
    let position = viewport.model_to_screen(position);
    if !position.x.is_finite() || !position.y.is_finite() {
        return Err(error("point gesture screen coordinate overflow"));
    }
    Ok(PointerInput {
        pointer_id: gesture_id,
        position,
        modifiers: Modifiers::default(),
    })
}

fn accepted_point(
    editor: &ProjectionalEditorSession,
    point: DesignPointId,
) -> Result<[f64; 2], EngineError> {
    editor
        .presentation_session()
        .and_then(|session| session.accepted_state_for_current_input())
        .and_then(|accepted| accepted.document().point(point))
        .map(|point| point.position)
        .ok_or_else(|| error("point gesture has no independently accepted native position"))
}

pub(super) fn expanded_port_point(
    editor: &ProjectionalEditorSession,
    handle: &ExpandedPort,
) -> Option<DesignPointId> {
    if handle.kind != IntentPortKind::Point {
        return None;
    }
    let node = editor
        .coordinator()
        .intent()
        .graph()
        .node_by_symbol(&handle.alias)?;
    let port = node.port_by_selector(handle.selector)?;
    if port.kind != handle.kind {
        return None;
    }
    match editor
        .coordinator()
        .accepted_materialization()?
        .ownership
        .port(port.as_ref(node.id))?
    {
        IntentNativeBinding::Point(point) => Some(point),
        _ => None,
    }
}
