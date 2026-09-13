// SPDX-License-Identifier: GPL-3.0-or-later
//! Source-workspace transport for hosts that persist native authoring history.
use geosolve_sketch_code::authoring_persistence::{
    SourceWorkspacePresentation, decode_source_workspace, encode_source_workspace,
    validate_source_workspace_presentation,
};

use crate::{EditableSession, EngineError, WorkspaceViewPresentation};
use geosolve_constraint_editor::presentation_persistence::{
    WORKBENCH_PERSISTENCE_FORMAT, WorkbenchPersistenceEnvelope,
};

fn error(value: impl std::fmt::Display) -> EngineError {
    EngineError::Admission(value.to_string())
}

impl EditableSession {
    /// Admits the existing outer browser/folder envelope or an inner source workspace.
    /// # Errors
    /// Rejects malformed transport, personal metadata or any retained source/native checkpoint.
    pub fn restore_host_workspace(
        json: &str,
    ) -> Result<
        (
            Self,
            SourceWorkspacePresentation,
            Option<WorkspaceViewPresentation>,
        ),
        EngineError,
    > {
        #[derive(serde::Deserialize)]
        struct Probe {
            format: Option<String>,
        }
        if json.len() > 96 * 1024 * 1024 {
            return Err(error("workspace presentation exceeds its byte limit"));
        }
        // Discriminator only: every path then uses its complete strict owner decoder.
        let probe: Probe = serde_json::from_str(json).map_err(error)?;
        if probe.format.is_some() {
            let envelope = WorkbenchPersistenceEnvelope::decode(json).map_err(error)?;
            let (session, presentation) = Self::restore_source_workspace(&envelope.project)?;
            Ok((session, presentation, Some(envelope.presentation)))
        } else {
            let (session, presentation) = Self::restore_source_workspace(json)?;
            Ok((session, presentation, None))
        }
    }

    /// Retains the existing outer presentation format when requested by the host.
    /// # Errors
    /// Rejects invalid personal state, ephemeral history or oversized source history.
    pub fn export_host_workspace(
        &self,
        source: &SourceWorkspacePresentation,
        view: Option<WorkspaceViewPresentation>,
    ) -> Result<String, EngineError> {
        if let Some(presentation) = view {
            presentation.validate().map_err(error)?;
            serde_json::to_string(&WorkbenchPersistenceEnvelope {
                format: WORKBENCH_PERSISTENCE_FORMAT.into(),
                project: self.export_source_workspace(source)?,
                presentation,
            })
            .map_err(error)
        } else {
            self.export_source_workspace(source)
        }
    }

    /// Opens the existing saved source workspace, including unfinished text and all history.
    ///
    /// # Errors
    /// Rejects corrupt transport, source/native history, origin, file selection or diagnostics.
    pub fn restore_source_workspace(
        json: &str,
    ) -> Result<(Self, SourceWorkspacePresentation), EngineError> {
        let workspace = decode_source_workspace(json).map_err(error)?;
        let presentation = SourceWorkspacePresentation {
            origin: workspace.origin,
            selected_file: workspace.selected_file,
            managed_draft: workspace.managed_draft,
            draft_diagnostic: workspace.draft_diagnostic,
        };
        let session = Self::from_validated_source_history(workspace.session)?;
        Ok((session, presentation))
    }

    /// Saves exact native source history with host-owned draft and file selection.
    ///
    /// # Errors
    /// Rejects ephemeral history, invalid personal source state or oversized transport.
    pub fn export_source_workspace(
        &self,
        presentation: &SourceWorkspacePresentation,
    ) -> Result<String, EngineError> {
        if !self.persistable_history {
            return Err(error("session has no persistable native checkpoints"));
        }
        let project = self
            .source_session()
            .snapshot()
            .code_project
            .as_ref()
            .ok_or_else(|| error("missing managed project"))?;
        validate_source_workspace_presentation(project, presentation).map_err(error)?;
        encode_source_workspace(
            &presentation.origin,
            project,
            self.source_session(),
            &presentation.selected_file,
            &presentation.managed_draft,
            presentation.draft_diagnostic.as_ref(),
        )
        .map_err(error)
    }
}

impl EditableSession {
    /// Exact pinned data-only patch definitions for a genuine host compiler invocation.
    /// # Errors
    /// Rejects unavailable source authority, invalid project pins or ambiguous bindings.
    pub fn managed_compiler_patches(
        &self,
    ) -> Result<std::collections::BTreeMap<String, serde_json::Value>, EngineError> {
        let project = self
            .source_session()
            .snapshot()
            .code_project
            .as_ref()
            .ok_or_else(|| error("missing source project"))?;
        geosolve_sketch_code::managed_compiler_patches(project).map_err(error)
    }
}
impl crate::AcceptedEvaluation {
    /// Exports the existing native v8 workspace for a retained accepted result.
    /// # Errors
    /// Rejects missing accepted native authority or an oversized checkpoint.
    pub fn export_native_workspace(&self) -> Result<String, EngineError> {
        geosolve_constraint_editor::workspace_persistence::WorkspaceSnapshot::from_projectional_editor(&self.0.materialized.editor)
            .and_then(|snapshot| snapshot.encode()).map_err(error)
    }
}
