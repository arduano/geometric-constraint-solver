// SPDX-License-Identifier: GPL-3.0-or-later

//! Exact selected occurrence context shared by local and remote canvas hosts.

use std::collections::BTreeSet;

use geosolve_sketch::CurveSpan;
use serde::{Deserialize, Serialize};

use crate::{ConstraintEditor, EditorScene, SceneCurveOrigin, SelectionItem};

/// Explicit location and native occurrence chosen by one canvas curve click.
/// This is presentation intent, never a prepared edit or geometry certificate.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CurvePickContext {
    pub span: CurveSpan,
    pub parameter: f64,
    #[serde(with = "crate::detached_scene::origin_codec")]
    pub origin: SceneCurveOrigin,
}

/// Ordered selection and exact picked locations needed by later authoring tools.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SelectionPresentationState {
    pub items: Vec<SelectionItem>,
    pub curve_picks: Vec<CurvePickContext>,
}

/// Presentation selection could not be admitted against the current scene.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum SelectionPresentationError {
    #[error("selection belongs to an unavailable or stale scene")]
    StaleScene,
    #[error("finish the current interaction before replacing selection")]
    ActiveInteraction,
    #[error("selection contains excessive, duplicate or unavailable identities")]
    InvalidSelection,
    #[error("curve pick does not identify a finite visible selected occurrence")]
    InvalidCurvePick,
}

impl SelectionPresentationState {
    /// Validates complete selection without changing any editor state.
    ///
    /// # Errors
    /// Rejects unsealed scenes, duplicate/foreign identities, hidden picked
    /// occurrences and parameters outside the exact selected visible interval.
    pub fn validate(&self, scene: &EditorScene) -> Result<(), SelectionPresentationError> {
        if !scene.retained_reprojection_semantics_are_sealed() {
            return Err(SelectionPresentationError::StaleScene);
        }
        let selected = self.items.iter().copied().collect::<BTreeSet<_>>();
        if self.items.len() > 100_000
            || selected.len() != self.items.len()
            || self
                .items
                .iter()
                .any(|item| !contains_selection(scene, *item))
        {
            return Err(SelectionPresentationError::InvalidSelection);
        }
        let mut spans = BTreeSet::new();
        for pick in &self.curve_picks {
            if !pick.parameter.is_finite()
                || !selected.contains(&SelectionItem::Curve(pick.span))
                || !spans.insert(pick.span)
                || !scene.curves.iter().any(|curve| {
                    curve.span == pick.span
                        && curve.origin == pick.origin
                        && curve
                            .screen_parameters
                            .first()
                            .is_some_and(|start| pick.parameter >= *start)
                        && curve
                            .screen_parameters
                            .last()
                            .is_some_and(|end| pick.parameter <= *end)
                })
                || !scene
                    .accepted_document
                    .evaluate_curve_jet(pick.span, pick.parameter)
                    .is_ok_and(|jet| jet.position.x.is_finite() && jet.position.y.is_finite())
            {
                return Err(SelectionPresentationError::InvalidCurvePick);
            }
        }
        Ok(())
    }
}

fn contains_selection(scene: &EditorScene, item: SelectionItem) -> bool {
    let document = &scene.accepted_document;
    match item {
        SelectionItem::Point(id) => document.point(id).is_some(),
        SelectionItem::Curve(span) => document.curve_spans(span.curve).is_ok_and(|spans| spans.contains(&span)),
        SelectionItem::Constraint(id) => document.constraint(id).is_some()
            || scene.constraint_entries.iter().any(|entry| entry.id == id),
        SelectionItem::Dimension(id) => document.dimension(id).is_some(),
        SelectionItem::Datum(_) => true,
        SelectionItem::Feature(feature) => scene.computed_curves.iter().any(|curve| curve.owner.feature == feature)
            || scene.hidden_presentation_items.contains(&item)
            || scene.hidden_presentation_items.iter().any(|hidden| matches!(hidden, SelectionItem::FeatureCorner(owner) if owner.feature == feature)),
        SelectionItem::FeatureCorner(owner) => scene.computed_curves.iter().any(|curve| curve.owner == owner)
            || scene.hidden_presentation_items.contains(&item),
    }
}

impl ConstraintEditor {
    /// Captures exact canvas picks separately from tree/programmatic selection.
    #[must_use]
    pub fn selection_presentation_state(&self) -> SelectionPresentationState {
        SelectionPresentationState {
            items: self.selection.clone(),
            curve_picks: self
                .curve_pick_parameters
                .iter()
                .filter_map(|(span, parameter)| {
                    self.curve_pick_origin(*span)
                        .map(|origin| CurvePickContext {
                            span: *span,
                            parameter: *parameter,
                            origin,
                        })
                })
                .collect(),
        }
    }

    /// Restores selection and explicit curve locations in one non-editing step.
    ///
    /// # Errors
    /// Rejects active gestures or any invalid selection/pick without clearing
    /// the previous selection, hover context, parameter or native occurrence.
    pub fn restore_selection_presentation(
        &mut self,
        scene: &EditorScene,
        state: SelectionPresentationState,
    ) -> Result<(), SelectionPresentationError> {
        if self.active_pointer_gesture().is_some() || self.geometry_draft_status().is_some() {
            return Err(SelectionPresentationError::ActiveInteraction);
        }
        state.validate(scene)?;
        self.set_selection(state.items);
        self.curve_pick_parameters = state
            .curve_picks
            .iter()
            .map(|pick| (pick.span, pick.parameter))
            .collect();
        self.curve_pick_origins = state
            .curve_picks
            .iter()
            .map(|pick| (pick.span, pick.origin))
            .collect();
        Ok(())
    }
}
