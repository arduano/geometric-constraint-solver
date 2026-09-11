// SPDX-License-Identifier: GPL-3.0-or-later
use crate::{EngineAdapter, decode_session_request};
use geosolve_sketch_code::CodeSessionIdentity;
use serde::Deserialize;
use serde_json::Value;

impl EngineAdapter {
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
