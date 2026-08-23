// SPDX-License-Identifier: GPL-3.0-or-later

//! Headless interaction adapter for projectional design intent.
//!
//! This adapter deliberately owns no [`RetainedEditorCoordinator`]. Durable
//! geometry, accepted-scene publication and Undo/Redo remain exclusively in
//! [`ProjectionalIntentCoordinator`]; [`ConstraintEditor`] contributes only
//! disposable selection, hover and pointer-gesture state.

use geosolve_sketch::{DesignPointId, OperationControl};
use geosolve_sketch_intent::{IntentPatch, IntentPlanDisposition, IntentSessionIdentity};
use thiserror::Error;

use crate::{
    ConstraintEditor, EditorEffect, EditorError, EditorScene, Modifiers, PointerInput,
    ProjectionalCoordinatorError, ProjectionalIntentCoordinator, ProjectionalPatchOutcome,
    SelectionItem, Viewport,
};

#[derive(Clone, Copy, Debug, PartialEq)]
struct ActivePointDrag {
    pointer_id: u64,
    point: DesignPointId,
    latest_request_id: Option<u64>,
    latest_position: Option<[f64; 2]>,
}

/// Result of one terminal pointer sample.
#[derive(Clone, Debug, PartialEq)]
pub struct ProjectionalEditorPointerOutcome {
    /// Presentation-only effects already reflected in the embedded editor.
    pub effects: Vec<EditorEffect>,
    /// The sole durable transaction, present only for an accepted moved drag.
    pub transaction: Option<ProjectionalPatchOutcome>,
}

/// Projectional intent plus disposable headless interaction state.
///
/// Pointer motion solves against retained native continuation owned by the
/// projectional coordinator. It does not serialize or replay intent and cannot
/// append history. A matching pointer-up converts the newest accepted sample
/// into exactly one instance patch.
#[derive(Debug)]
pub struct ProjectionalEditorSession {
    coordinator: ProjectionalIntentCoordinator,
    editor: ConstraintEditor,
    point_drag: Option<ActivePointDrag>,
    preview_control: OperationControl,
}

impl ProjectionalEditorSession {
    /// Creates a headless projectional interaction session.
    #[must_use]
    pub fn new(coordinator: ProjectionalIntentCoordinator) -> Self {
        Self::with_editor_and_control(
            coordinator,
            ConstraintEditor::default(),
            crate::coordinator::bounded_geometry_control(),
        )
    }

    /// Creates a session with explicit transient editor and solve-work policy.
    #[must_use]
    pub const fn with_editor_and_control(
        coordinator: ProjectionalIntentCoordinator,
        editor: ConstraintEditor,
        preview_control: OperationControl,
    ) -> Self {
        Self {
            coordinator,
            editor,
            point_drag: None,
            preview_control,
        }
    }

    /// Canonical durable authority.
    #[must_use]
    pub const fn coordinator(&self) -> &ProjectionalIntentCoordinator {
        &self.coordinator
    }

    /// Disposable selection, hover and gesture state.
    #[must_use]
    pub const fn editor(&self) -> &ConstraintEditor {
        &self.editor
    }

    /// Builds the currently presentable native scene.
    ///
    /// A valid point preview temporarily outranks the durable accepted scene.
    /// Retained invalid intent continues to display the prior accepted scene.
    ///
    /// # Errors
    ///
    /// Returns a typed no-authority or scene-composition failure.
    pub fn scene(
        &self,
        viewport: Viewport,
        chord_tolerance_pixels: f64,
    ) -> Result<EditorScene, ProjectionalEditorError> {
        let session = self
            .coordinator
            .presentation_session()
            .ok_or(ProjectionalEditorError::NoAcceptedAuthority)?;
        let accepted = session
            .accepted_state_for_current_input()
            .ok_or(ProjectionalEditorError::NoAcceptedAuthority)?;
        let mut scene = EditorScene::from_accepted_for_design(
            accepted.identity().revision().get(),
            session.design_identity(),
            accepted.document(),
            session.design_document(),
            viewport,
            chord_tolerance_pixels,
        )?;
        if !scene.update_annotation_values(accepted) {
            return Err(ProjectionalEditorError::SceneAuthorityMismatch);
        }
        scene.apply_annotation_layout(&self.editor.annotation_layout_for_scene());
        let mut scene = scene.with_retained_session(session)?;
        self.editor.populate_curve_controls(&mut scene)?;
        Ok(scene)
    }

    /// Applies a typed durable patch through the sole intent history.
    ///
    /// # Errors
    ///
    /// Returns the ordinary projectional planning/materialization error.
    pub fn apply_patch(
        &mut self,
        patch: IntentPatch,
    ) -> Result<ProjectionalPatchOutcome, ProjectionalEditorError> {
        self.cancel_interaction();
        let outcome = self.coordinator.apply_patch(patch)?;
        if outcome.disposition == IntentPlanDisposition::Accepted {
            self.clear_transient_selection();
        }
        Ok(outcome)
    }

    /// Steps the sole intent history backward.
    ///
    /// # Errors
    ///
    /// Returns an intent or cold-reconstruction error without changing state.
    pub fn undo(&mut self) -> Result<Option<IntentSessionIdentity>, ProjectionalEditorError> {
        self.cancel_interaction();
        let moved = self.coordinator.undo()?;
        if moved.is_some() {
            self.clear_transient_selection();
        }
        Ok(moved)
    }

    /// Steps the sole intent history forward.
    ///
    /// # Errors
    ///
    /// Returns an intent or cold-reconstruction error without changing state.
    pub fn redo(&mut self) -> Result<Option<IntentSessionIdentity>, ProjectionalEditorError> {
        self.cancel_interaction();
        let moved = self.coordinator.redo()?;
        if moved.is_some() {
            self.clear_transient_selection();
        }
        Ok(moved)
    }

    /// Replaces transient selection without touching intent or history.
    pub fn set_selection(&mut self, selection: impl IntoIterator<Item = SelectionItem>) {
        self.editor.set_selection(selection);
    }

    /// Applies one transient selection click without touching intent or history.
    pub fn select_item(&mut self, item: SelectionItem, modifiers: Modifiers) {
        self.editor.select_item(item, modifiers);
    }

    /// Starts one headless pointer gesture. Selection changes are transient.
    ///
    /// # Errors
    ///
    /// Returns a typed ownership/continuation error when the selected point is
    /// not a writable intent output. The rejected route is cancelled before a
    /// move frame can request native work.
    pub fn pointer_down(
        &mut self,
        scene: &EditorScene,
        input: PointerInput,
    ) -> Result<Vec<EditorEffect>, ProjectionalEditorError> {
        let effects = self.editor.pointer_down(scene, input);
        let route = self.editor.prepared_point_drag_route();
        match (self.point_drag, route) {
            (Some(active), Some(route))
                if active.pointer_id == route.pointer_id && active.point == route.point => {}
            (Some(_), Some(_)) => {
                self.cancel_point_drag();
                let _ = self.editor.cancel();
                return Err(ProjectionalEditorError::PointDragRouteMismatch);
            }
            (None, Some(route)) => {
                if let Err(error) = self
                    .coordinator
                    .begin_point_drag(route.pointer_id, route.point)
                {
                    let _ = self.editor.cancel();
                    return Err(error.into());
                }
                self.point_drag = Some(ActivePointDrag {
                    pointer_id: route.pointer_id,
                    point: route.point,
                    latest_request_id: None,
                    latest_position: None,
                });
            }
            (Some(_), None) => self.cancel_point_drag(),
            (None, None) => {}
        }
        Ok(effects)
    }

    /// Resolves one pointer frame through native retained continuation.
    ///
    /// The returned effects are presentation signals only. The intent identity
    /// and history are unchanged for every successful or rejected frame.
    ///
    /// # Errors
    ///
    /// Returns a typed route or native preview failure.
    pub fn pointer_move(
        &mut self,
        scene: &EditorScene,
        input: PointerInput,
    ) -> Result<Vec<EditorEffect>, ProjectionalEditorError> {
        let effects = self.editor.pointer_move(scene, input);
        self.resolve_pointer_frame(effects)
    }

    /// Finishes a pointer gesture and publishes at most one intent transaction.
    ///
    /// # Errors
    ///
    /// Returns a typed stale-route, preview, or exact cold-reconstruction error.
    pub fn pointer_up(
        &mut self,
        scene: &EditorScene,
        input: PointerInput,
    ) -> Result<ProjectionalEditorPointerOutcome, ProjectionalEditorError> {
        let effects = self.editor.pointer_up(scene, scene.design_identity, input);
        let mut presentation = Vec::new();
        let mut transaction = None;
        for effect in effects {
            match effect {
                EditorEffect::CommitPointMove {
                    point,
                    model_position,
                    ..
                } => {
                    let Some(drag) = self.point_drag.take() else {
                        self.coordinator.cancel_point_drag();
                        return Err(ProjectionalEditorError::MissingPointDragRoute);
                    };
                    if drag.pointer_id != input.pointer_id || drag.point != point {
                        self.coordinator.cancel_point_drag();
                        return Err(ProjectionalEditorError::PointDragRouteMismatch);
                    }
                    let Some(request_id) = drag.latest_request_id else {
                        self.coordinator.cancel_point_drag();
                        return Err(ProjectionalEditorError::MissingAcceptedPointSample);
                    };
                    let Some(accepted_position) = drag.latest_position else {
                        self.coordinator.cancel_point_drag();
                        return Err(ProjectionalEditorError::MissingAcceptedPointSample);
                    };
                    if accepted_position.map(f64::to_bits) != model_position.map(f64::to_bits) {
                        self.coordinator.cancel_point_drag();
                        return Err(ProjectionalEditorError::PointPreviewMismatch);
                    }
                    transaction = Some(
                        self.coordinator
                            .finish_point_drag(input.pointer_id, request_id)?,
                    );
                }
                EditorEffect::ClearPointPreview => {
                    self.cancel_point_drag();
                    presentation.push(EditorEffect::ClearPointPreview);
                }
                effect => presentation.push(effect),
            }
        }
        if self.point_drag.is_some() {
            self.cancel_point_drag();
        }
        Ok(ProjectionalEditorPointerOutcome {
            effects: presentation,
            transaction,
        })
    }

    /// Cancels every active pointer/draft interaction without changing intent.
    pub fn cancel_interaction(&mut self) -> Vec<EditorEffect> {
        self.cancel_point_drag();
        self.editor.cancel()
    }

    fn resolve_pointer_frame(
        &mut self,
        effects: Vec<EditorEffect>,
    ) -> Result<Vec<EditorEffect>, ProjectionalEditorError> {
        let mut presentation = Vec::new();
        for effect in effects {
            match effect {
                EditorEffect::RequestProjectedPointMove {
                    pointer_id,
                    request_id,
                    point,
                    model_position,
                } => {
                    self.authenticate_point_drag(pointer_id, point)?;
                    let preview = self.coordinator.preview_point_drag(
                        pointer_id,
                        request_id,
                        model_position,
                        self.preview_control.clone(),
                    )?;
                    let accepted_position = preview.map(|preview| preview.accepted_position);
                    if let Some(drag) = self.point_drag.as_mut() {
                        drag.latest_request_id = Some(request_id);
                        if accepted_position.is_some() {
                            drag.latest_position = accepted_position;
                        }
                    }
                    presentation.extend(self.editor.projected_drag_result(
                        pointer_id,
                        request_id,
                        point,
                        accepted_position,
                    ));
                }
                EditorEffect::ClearPointPreview => {
                    self.cancel_point_drag();
                    presentation.push(EditorEffect::ClearPointPreview);
                }
                effect => presentation.push(effect),
            }
        }
        Ok(presentation)
    }

    fn authenticate_point_drag(
        &mut self,
        pointer_id: u64,
        point: DesignPointId,
    ) -> Result<(), ProjectionalEditorError> {
        if let Some(drag) = self.point_drag {
            if drag.pointer_id == pointer_id && drag.point == point {
                return Ok(());
            }
            self.cancel_point_drag();
            return Err(ProjectionalEditorError::PointDragRouteMismatch);
        }
        Err(ProjectionalEditorError::MissingPointDragRoute)
    }

    fn cancel_point_drag(&mut self) {
        self.point_drag = None;
        self.coordinator.cancel_point_drag();
    }

    fn clear_transient_selection(&mut self) {
        let _ = self.editor.cancel();
        self.editor.set_selection([]);
    }
}

/// Projectional headless interaction failure.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ProjectionalEditorError {
    #[error(transparent)]
    Coordinator(#[from] ProjectionalCoordinatorError),
    #[error(transparent)]
    Scene(#[from] EditorError),
    #[error("there is no independently accepted projectional scene")]
    NoAcceptedAuthority,
    #[error("the accepted scene does not match its retained native authority")]
    SceneAuthorityMismatch,
    #[error("the terminal point sample has no prepared projectional drag route")]
    MissingPointDragRoute,
    #[error("the terminal point sample does not match its prepared drag route")]
    PointDragRouteMismatch,
    #[error("the terminal point sample has no independently accepted preview")]
    MissingAcceptedPointSample,
    #[error("the terminal editor point differs from the accepted native preview")]
    PointPreviewMismatch,
}
