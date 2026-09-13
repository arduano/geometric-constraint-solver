// SPDX-License-Identifier: GPL-3.0-or-later
use crate::{EngineAdapter, decode_session_request};
use geosolve_sketch_code::CodeSessionIdentity;
use serde::Deserialize;
use serde_json::Value;

impl EngineAdapter {
    /// Exports a trusted detached accepted basis for an editable session.
    /// # Errors
    /// Rejects stale tokens, invalid extents or missing native authority.
    pub fn editable_interaction_seed(&self, encoded: &str) -> Result<String, String> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Request {
            session: u64,
            expected: CodeSessionIdentity,
            viewport: Option<geosolve_sketch_engine::InteractionViewport>,
        }
        let input: Request = decode_session_request(encoded)?;
        let session = self
            .sessions
            .get(&input.session)
            .ok_or("unknown editable session")?;
        if session.token() != &input.expected {
            return Err("interaction seed input is stale or foreign".into());
        }
        serde_json::to_string(
            &session
                .interaction_seed(input.viewport)
                .map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())
    }

    /// Exports a retained immutable result, including generator results, for browsing.
    /// # Errors
    /// Rejects released results or invalid extents.
    pub fn result_interaction_seed(&self, encoded: &str) -> Result<String, String> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase", deny_unknown_fields)]
        struct Request {
            result_id: String,
            viewport: Option<geosolve_sketch_engine::InteractionViewport>,
        }
        let input: Request = decode_session_request(encoded)?;
        let result = self
            .retained
            .get(&input.result_id)
            .ok_or("unknown or released result ID")?;
        serde_json::to_string(
            &result
                .interaction_seed(input.viewport)
                .map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())
    }

    /// Read-only accepted source/native inspection, with no demo workbench reconstruction.
    /// # Errors
    /// Rejects stale tokens, invalid viewports and missing native authority.
    pub fn inspect_editable_session(&self, encoded: &str) -> Result<String, String> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Request {
            session: u64,
            expected: CodeSessionIdentity,
            viewport: Value,
        }
        let input: Request = decode_session_request(encoded)?;
        let session = self
            .sessions
            .get(&input.session)
            .ok_or("unknown editable session")?;
        if session.token() != &input.expected {
            return Err("inspection input is stale or foreign".into());
        }
        serde_json::to_string(
            &session
                .inspect(serde_json::from_value(input.viewport).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())
    }

    /// Stateless accepted presentation for exact namespace translation.
    /// # Errors
    /// Rejects stale session tokens, invalid viewports and absent native authority.
    pub fn editable_tool_operation_context(&self, encoded: &str) -> Result<String, String> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Request {
            session: u64,
            expected: CodeSessionIdentity,
            viewport: Value,
        }
        let input: Request = decode_session_request(encoded)?;
        let session = self
            .sessions
            .get(&input.session)
            .ok_or("unknown editable session")?;
        if session.token() != &input.expected {
            return Err("tool selection input is stale or foreign".into());
        }
        session
            .tool_operation_presentation_json(
                serde_json::from_value(input.viewport).map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())
    }
    /// Maps accepted personal selection and exposes semantic operands without a demo host.
    ///
    /// # Errors
    /// Rejects stale tokens, foreign scenes and invalid native occurrences.
    pub fn editable_tool_operation_view_operands(&self, encoded: &str) -> Result<String, String> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Request {
            session: u64,
            expected: CodeSessionIdentity,
            viewport: Value,
            view: Value,
        }
        let input: Request = decode_session_request(encoded)?;
        let session = self
            .sessions
            .get(&input.session)
            .ok_or("unknown editable session")?;
        if session.token() != &input.expected {
            return Err("tool selection input is stale or foreign".into());
        }
        serde_json::to_string(
            &session
                .tool_operation_view_operands(
                    serde_json::from_value(input.viewport).map_err(|e| e.to_string())?,
                    serde_json::from_value(input.view).map_err(|e| e.to_string())?,
                )
                .map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())
    }

    /// Validates native presentation selection before exposing semantic operands.
    /// # Errors
    /// Rejects stale tokens, foreign selection and invalid native occurrences.
    pub fn editable_tool_operation_operands(&self, encoded: &str) -> Result<String, String> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Request {
            session: u64,
            expected: CodeSessionIdentity,
            selection: Value,
        }
        let input: Request = decode_session_request(encoded)?;
        let session = self
            .sessions
            .get(&input.session)
            .ok_or("unknown editable session")?;
        if session.token() != &input.expected {
            return Err("tool selection input is stale or foreign".into());
        }
        let selection = serde_json::from_value(input.selection).map_err(|e| e.to_string())?;
        serde_json::to_string(
            &session
                .tool_operation_operands(&selection)
                .map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())
    }
}
