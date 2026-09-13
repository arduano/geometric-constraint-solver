// SPDX-License-Identifier: GPL-3.0-or-later
use crate::{EngineAdapter, decode_session_request};
use geosolve_sketch_engine::WorkspaceViewPresentation;
use geosolve_sketch_engine::{EditableSession, SourceWorkspacePresentation};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ExportRequest {
    session: u64,
    expected: geosolve_sketch_code::CodeSessionIdentity,
    presentation: SourceWorkspacePresentation,
    #[serde(default, rename = "viewPresentation")]
    view_presentation: Option<WorkspaceViewPresentation>,
}

#[derive(Serialize)]
struct RestoredWorkspace {
    state: geosolve_sketch_engine::EditableSessionState,
    presentation: SourceWorkspacePresentation,
    #[serde(rename = "viewPresentation")]
    view_presentation: Option<WorkspaceViewPresentation>,
}

impl EngineAdapter {
    /// # Errors
    /// Rejects unknown sessions, invalid project pins or ambiguous compiler bindings.
    pub fn editable_compiler_patches(&self, id: &str) -> Result<String, String> {
        let id = id.parse::<u64>().map_err(|error| error.to_string())?;
        let session = self.sessions.get(&id).ok_or("unknown editable session")?;
        serde_json::to_string(
            &session
                .managed_compiler_patches()
                .map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())
    }

    /// # Errors
    /// Rejects unknown/released native results or oversized workspace transport.
    pub fn export_result_workspace(&self, id: &str) -> Result<String, String> {
        self.retained
            .get(id)
            .ok_or("unknown or released result ID")?
            .export_native_workspace()
            .map_err(|error| error.to_string())
    }

    /// # Errors
    /// Rejects workspace text beyond the existing bounded reproduction transport.
    pub fn encode_reproduction(&self, workspace: &str) -> Result<String, String> {
        geosolve_sketch_engine::encode_reproduction_workspace(workspace)
            .map_err(|error| error.to_string())
    }

    /// # Errors
    /// Rejects corrupt persisted source/native history, duplicate live IDs or exhausted capacity.
    pub fn restore_editable_workspace(&mut self, json: &str) -> Result<String, String> {
        self.reserve()?;
        if self.sessions.len() >= 8 {
            return Err("close an editable session before opening more than eight".into());
        }
        let (session, presentation, view_presentation) =
            EditableSession::restore_host_workspace(json).map_err(|error| error.to_string())?;
        let reply = serde_json::to_string(&RestoredWorkspace {
            state: session.state(),
            presentation,
            view_presentation,
        })
        .map_err(|error| error.to_string())?;
        self.install_opened_session_reply(session, reply)
    }

    /// # Errors
    /// Rejects foreign/stale sessions, ephemeral history or invalid draft/presentation metadata.
    pub fn export_editable_workspace(&self, json: &str) -> Result<String, String> {
        let request: ExportRequest = decode_session_request(json)?;
        let session = self
            .sessions
            .get(&request.session)
            .ok_or("unknown or closed session ID")?;
        if session.token() != &request.expected {
            return Err("stale or foreign editable session token".into());
        }
        session
            .export_host_workspace(&request.presentation, request.view_presentation)
            .map_err(|error| error.to_string())
    }
}
