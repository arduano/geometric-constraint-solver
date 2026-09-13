// SPDX-License-Identifier: GPL-3.0-or-later
//! Read-only projections from an already independently accepted engine result.
use crate::{AcceptedEvaluation, EditableSession, EngineError};
use geosolve_constraint_editor::{
    IntentWorkbenchProjection, ProjectionalPresentationBindings, Viewport,
};
use geosolve_sketch_code::{CodeSessionIdentity, ManagedNavigationIndex};
use serde::Serialize;

/// Detached native presentation and durable outline from one exact accepted result.
/// No field is a publication or mutation capability.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AcceptedInspection {
    pub result_id: String,
    pub scene: String,
    pub bindings: ProjectionalPresentationBindings,
    pub intent: IntentWorkbenchProjection,
}

/// Accepted source navigation paired with its exact session and native presentation.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditableInspection {
    pub token: CodeSessionIdentity,
    pub accepted: AcceptedInspection,
    pub navigation: ManagedNavigationIndex,
}

impl AcceptedEvaluation {
    /// Inspects retained native authority without restoring a project or running a solve.
    ///
    /// # Errors
    /// Rejects invalid viewport or missing accepted native ownership.
    pub fn inspect(&self, viewport: Viewport) -> Result<AcceptedInspection, EngineError> {
        let editor = &self.0.materialized.editor;
        Ok(AcceptedInspection {
            result_id: self.result().result_id.clone(),
            scene: editor
                .scene(viewport, 0.25)
                .map_err(admission)?
                .to_detached_json()
                .map_err(admission)?,
            bindings: editor
                .presentation_bindings()
                .ok_or_else(|| admission("missing accepted presentation bindings"))?,
            intent: editor.workbench_projection(),
        })
    }
}

impl EditableSession {
    /// Inspects accepted source and native authority; working text remains host-owned.
    ///
    /// # Errors
    /// Rejects invalid viewport or unavailable accepted source/native authority.
    pub fn inspect(&self, viewport: Viewport) -> Result<EditableInspection, EngineError> {
        let snapshot = self.code_snapshot();
        let managed = snapshot
            .accepted_code_project
            .as_ref()
            .map_or(&snapshot.managed, |project| &project.managed);
        let materialized = &self.accepted().0.materialized;
        let generated = snapshot
            .accepted_generated
            .as_ref()
            .unwrap_or(&snapshot.generated);
        Ok(EditableInspection {
            token: self.token().clone(),
            accepted: self.accepted().inspect(viewport)?,
            navigation: geosolve_sketch_code::managed_navigation_index(
                managed,
                &materialized.expansion,
                generated,
                &materialized.editor,
            ),
        })
    }
}

fn admission(error: impl std::fmt::Display) -> EngineError {
    EngineError::Admission(error.to_string())
}
