// SPDX-License-Identifier: GPL-3.0-or-later
//! Accepted native input for independent browsing hosts; no renderer or chrome.
use crate::{AcceptedEvaluation, EditableSession, EngineError};
use geosolve_constraint_editor::{
    CanvasCamera, DetachedDimensionPresentation, DimensionDisplayMode,
    DimensionPresentationContext, DimensionPresentationState, VisibilitySeed, VisibilityState,
    detached_interaction::DetachedInteractionSeed,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Host CSS extent used for the initial fitted accepted view.
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InteractionViewport {
    pub width: f64,
    pub height: f64,
    #[serde(default = "pixel_ratio")]
    pub pixel_ratio: f64,
}
const fn pixel_ratio() -> f64 {
    1.0
}

fn error(value: impl std::fmt::Display) -> EngineError {
    EngineError::Admission(value.to_string())
}

impl AcceptedEvaluation {
    /// Exports exact accepted geometry and semantic bindings without a solve.
    /// Hosts enrich dimensions and source-row visibility through their read adapter.
    ///
    /// # Errors
    /// Rejects invalid host extents, unavailable native ownership or invalid scene projection.
    pub fn interaction_seed(
        &self,
        viewport: Option<InteractionViewport>,
    ) -> Result<DetachedInteractionSeed, EngineError> {
        let mut camera = CanvasCamera::default();
        if let Some(size) = viewport {
            if [size.width, size.height]
                .iter()
                .any(|v| !v.is_finite() || *v <= 0.0 || *v > 32_768.0)
                || !size.pixel_ratio.is_finite()
                || size.pixel_ratio <= 0.0
                || size.pixel_ratio > 16.0
            {
                return Err(error("expected finite bounded positive scene viewport"));
            }
            camera.resize([size.width, size.height]);
        }
        let materialized = &self.0.materialized;
        let editor = &materialized.editor;
        let mut scene = editor.scene(camera.viewport(), 0.25).map_err(error)?;
        camera.fit_scene_or_reset(Some(&scene));
        scene.reproject_viewport(camera.viewport()).map_err(error)?;
        let mut state = DimensionPresentationState::default();
        state.mode = DimensionDisplayMode::Focused;
        Ok(DetachedInteractionSeed {
            format: "geosolve-local-interaction-v1".into(),
            scene_key: self.result().result_id.clone(),
            scene: scene.to_detached_json().map_err(error)?,
            title: self
                .result()
                .document
                .title
                .clone()
                .unwrap_or_else(|| "Untitled code sketch".into()),
            host_size_received: viewport.is_some(),
            semantic_preview: false,
            selection: Vec::new(),
            curve_picks: Vec::new(),
            grid_visible: true,
            policy: editor.editor().geometry_interaction_policy(),
            dimensions: DetachedDimensionPresentation {
                state,
                context: DimensionPresentationContext::default(),
                layout: editor.editor().annotation_layout_for_scene(),
                ids: BTreeMap::new(),
            },
            visibility_seed: VisibilitySeed::default(),
            visibility: VisibilityState::default(),
            bindings: editor.presentation_bindings(),
            point_targets: geosolve_sketch_code::interaction::point_targets(
                &materialized.expansion,
                editor,
            ),
            presence_bindings: geosolve_sketch_code::interaction::presence_bindings(
                &materialized.expansion,
                editor,
            ),
        })
    }
}

impl EditableSession {
    /// Exports the accepted basis independently of working text and personal history.
    ///
    /// # Errors
    /// Rejects invalid viewport or unavailable accepted presentation.
    pub fn interaction_seed(
        &self,
        viewport: Option<InteractionViewport>,
    ) -> Result<DetachedInteractionSeed, EngineError> {
        self.accepted().interaction_seed(viewport)
    }
}
