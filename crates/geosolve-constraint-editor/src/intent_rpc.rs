// SPDX-License-Identifier: GPL-3.0-or-later

//! DOM-free JSON RPC over the projectional intent coordinator.
//!
//! Native and WASM hosts call this exact implementation. The protocol carries
//! typed Rust patches and projections; it does not evaluate TypeScript or own
//! any geometry/constraint equation.

use geosolve_sketch_intent::{
    IntentAliasMap, IntentPatch, IntentPlanDisposition, IntentSessionIdentity, NodeId,
};
use serde::{Deserialize, Serialize};

use crate::{
    IntentInspectorProjection, IntentSourceTokenId, IntentValidationEvidence,
    IntentWorkbenchProjection, ProjectionalEditorError, ProjectionalEditorSession,
    ProjectionalIntentCoordinator, ProjectionalPatchOutcome,
};

/// Maximum accepted byte length for one DOM-free intent RPC request.
///
/// The session and graph codecs have their own larger persistence bounds. RPC
/// requests are deliberately narrower so an untrusted host message cannot
/// force an unbounded JSON parse before those inner limits are reached.
pub const MAX_INTENT_RPC_REQUEST_BYTES: usize = 16 * 1024 * 1024;

/// Closed version-one RPC request vocabulary.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "method", rename_all = "snake_case", deny_unknown_fields)]
pub enum IntentRpcRequest {
    Snapshot,
    ApplyPatch {
        patch: Box<IntentPatch>,
    },
    Undo,
    Redo,
    Inspector {
        node: NodeId,
    },
    EditSourceToken {
        token: IntentSourceTokenId,
        replacement: String,
    },
}

/// Current equation-free projection and independently validated native status.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IntentRpcSnapshot {
    pub identity: IntentSessionIdentity,
    pub projection: IntentWorkbenchProjection,
    pub accepted_validation: Option<IntentValidationEvidence>,
}

/// Successful RPC result.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "result", rename_all = "snake_case", deny_unknown_fields)]
pub enum IntentRpcSuccess {
    Snapshot {
        snapshot: Box<IntentRpcSnapshot>,
    },
    Patch {
        disposition: IntentPlanDisposition,
        aliases: IntentAliasMap,
        snapshot: Box<IntentRpcSnapshot>,
    },
    History {
        moved: bool,
        snapshot: Box<IntentRpcSnapshot>,
    },
    Inspector {
        inspector: Option<IntentInspectorProjection>,
        snapshot: Box<IntentRpcSnapshot>,
    },
}

/// Stable machine code plus human-readable failure detail.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IntentRpcFailure {
    pub code: String,
    pub message: String,
    pub identity: Option<IntentSessionIdentity>,
}

/// One canonical RPC response.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "outcome", rename_all = "snake_case", deny_unknown_fields)]
pub enum IntentRpcOutcome {
    Success { value: IntentRpcSuccess },
    Failure { failure: IntentRpcFailure },
}

/// Stateful DOM-free RPC adapter around the single-history coordinator.
#[derive(Debug)]
pub struct IntentRpcSession {
    coordinator: ProjectionalIntentCoordinator,
}

impl IntentRpcSession {
    #[must_use]
    pub const fn new(coordinator: ProjectionalIntentCoordinator) -> Self {
        Self { coordinator }
    }

    #[must_use]
    pub const fn coordinator(&self) -> &ProjectionalIntentCoordinator {
        &self.coordinator
    }

    #[must_use]
    pub fn snapshot(&self) -> IntentRpcSnapshot {
        snapshot(self.coordinator())
    }

    /// Applies one typed request and returns a stable structured outcome.
    #[must_use]
    pub fn apply(&mut self, request: IntentRpcRequest) -> IntentRpcOutcome {
        apply_to_backend(self, request)
    }

    /// Decodes one strict JSON request and encodes one deterministic compact
    /// response. Parse errors never mutate the session.
    ///
    /// # Panics
    ///
    /// Panics only if serialization of the closed Rust-owned response schema
    /// fails after all values have already been validated.
    #[must_use]
    pub fn apply_json(&mut self, request: &str) -> String {
        apply_json_to_backend(self, request)
    }
}

/// Applies the closed RPC vocabulary directly to the live projectional editor.
///
/// This is the code-to-GUI bridge: canvas, Inspector, source and external
/// TypeScript/RPC clients all mutate the editor's one canonical intent session
/// and one Undo/Redo history. It does not create or synchronize a second RPC
/// session.
#[must_use]
pub fn apply_intent_rpc_to_editor(
    editor: &mut ProjectionalEditorSession,
    request: IntentRpcRequest,
) -> IntentRpcOutcome {
    apply_to_backend(editor, request)
}

/// Strict JSON form of [`apply_intent_rpc_to_editor`]. Parse/resource failures
/// are state-neutral and carry the current live editor identity.
#[must_use]
pub fn apply_intent_rpc_json_to_editor(
    editor: &mut ProjectionalEditorSession,
    request: &str,
) -> String {
    apply_json_to_backend(editor, request)
}

trait IntentRpcBackend {
    fn coordinator(&self) -> &ProjectionalIntentCoordinator;

    fn apply_patch(&mut self, patch: IntentPatch) -> Result<ProjectionalPatchOutcome, RpcError>;

    fn undo(&mut self) -> Result<bool, RpcError>;

    fn redo(&mut self) -> Result<bool, RpcError>;

    fn edit_source_token(
        &mut self,
        token: IntentSourceTokenId,
        replacement: &str,
    ) -> Result<ProjectionalPatchOutcome, RpcError>;
}

impl IntentRpcBackend for IntentRpcSession {
    fn coordinator(&self) -> &ProjectionalIntentCoordinator {
        &self.coordinator
    }

    fn apply_patch(&mut self, patch: IntentPatch) -> Result<ProjectionalPatchOutcome, RpcError> {
        self.coordinator.apply_patch(patch).map_err(RpcError::from)
    }

    fn undo(&mut self) -> Result<bool, RpcError> {
        self.coordinator
            .undo()
            .map(|moved| moved.is_some())
            .map_err(RpcError::from)
    }

    fn redo(&mut self) -> Result<bool, RpcError> {
        self.coordinator
            .redo()
            .map(|moved| moved.is_some())
            .map_err(RpcError::from)
    }

    fn edit_source_token(
        &mut self,
        token: IntentSourceTokenId,
        replacement: &str,
    ) -> Result<ProjectionalPatchOutcome, RpcError> {
        let projection = IntentWorkbenchProjection::from_session(self.coordinator.intent());
        projection
            .structured_source
            .patch_for_edit(self.coordinator.intent(), token, replacement)
            .map_err(|error| RpcError::Text(error.to_string()))
            .and_then(|patch| self.coordinator.apply_patch(patch).map_err(RpcError::from))
    }
}

impl IntentRpcBackend for ProjectionalEditorSession {
    fn coordinator(&self) -> &ProjectionalIntentCoordinator {
        self.coordinator()
    }

    fn apply_patch(&mut self, patch: IntentPatch) -> Result<ProjectionalPatchOutcome, RpcError> {
        ProjectionalEditorSession::apply_patch(self, patch).map_err(RpcError::from)
    }

    fn undo(&mut self) -> Result<bool, RpcError> {
        ProjectionalEditorSession::undo(self)
            .map(|moved| moved.is_some())
            .map_err(RpcError::from)
    }

    fn redo(&mut self) -> Result<bool, RpcError> {
        ProjectionalEditorSession::redo(self)
            .map(|moved| moved.is_some())
            .map_err(RpcError::from)
    }

    fn edit_source_token(
        &mut self,
        token: IntentSourceTokenId,
        replacement: &str,
    ) -> Result<ProjectionalPatchOutcome, RpcError> {
        let projection = self.workbench_projection();
        ProjectionalEditorSession::edit_source_token(self, &projection, token, replacement)
            .map_err(RpcError::from)
    }
}

fn snapshot(coordinator: &ProjectionalIntentCoordinator) -> IntentRpcSnapshot {
    IntentRpcSnapshot {
        identity: coordinator.intent().identity(),
        projection: IntentWorkbenchProjection::from_session(coordinator.intent()),
        accepted_validation: coordinator
            .accepted_materialization()
            .map(|accepted| accepted.validation.clone()),
    }
}

fn apply_to_backend(
    backend: &mut impl IntentRpcBackend,
    request: IntentRpcRequest,
) -> IntentRpcOutcome {
    let result: Result<IntentRpcSuccess, RpcError> = match request {
        IntentRpcRequest::Snapshot => Ok(IntentRpcSuccess::Snapshot {
            snapshot: Box::new(snapshot(backend.coordinator())),
        }),
        IntentRpcRequest::ApplyPatch { patch } => {
            backend
                .apply_patch(*patch)
                .map(|outcome| IntentRpcSuccess::Patch {
                    disposition: outcome.disposition,
                    aliases: outcome.aliases,
                    snapshot: Box::new(snapshot(backend.coordinator())),
                })
        }
        IntentRpcRequest::Undo => backend.undo().map(|moved| IntentRpcSuccess::History {
            moved,
            snapshot: Box::new(snapshot(backend.coordinator())),
        }),
        IntentRpcRequest::Redo => backend.redo().map(|moved| IntentRpcSuccess::History {
            moved,
            snapshot: Box::new(snapshot(backend.coordinator())),
        }),
        IntentRpcRequest::Inspector { node } => Ok(IntentRpcSuccess::Inspector {
            inspector: IntentWorkbenchProjection::from_session(backend.coordinator().intent())
                .inspector(backend.coordinator().intent(), node),
            snapshot: Box::new(snapshot(backend.coordinator())),
        }),
        IntentRpcRequest::EditSourceToken { token, replacement } => backend
            .edit_source_token(token, &replacement)
            .map(|outcome| IntentRpcSuccess::Patch {
                disposition: outcome.disposition,
                aliases: outcome.aliases,
                snapshot: Box::new(snapshot(backend.coordinator())),
            }),
    };
    match result {
        Ok(value) => IntentRpcOutcome::Success { value },
        Err(error) => IntentRpcOutcome::Failure {
            failure: IntentRpcFailure {
                code: error.code().to_owned(),
                message: error.to_string(),
                identity: Some(backend.coordinator().intent().identity()),
            },
        },
    }
}

fn apply_json_to_backend(backend: &mut impl IntentRpcBackend, request: &str) -> String {
    let outcome = if request.len() > MAX_INTENT_RPC_REQUEST_BYTES {
        IntentRpcOutcome::Failure {
            failure: IntentRpcFailure {
                code: "request_too_large".to_owned(),
                message: format!("intent RPC request exceeds {MAX_INTENT_RPC_REQUEST_BYTES} bytes"),
                identity: Some(backend.coordinator().intent().identity()),
            },
        }
    } else {
        match serde_json::from_str(request) {
            Ok(request) => apply_to_backend(backend, request),
            Err(error) => IntentRpcOutcome::Failure {
                failure: IntentRpcFailure {
                    code: "invalid_request".to_owned(),
                    message: error.to_string(),
                    identity: Some(backend.coordinator().intent().identity()),
                },
            },
        }
    };
    serde_json::to_string(&outcome).expect("closed RPC response is infallibly serializable")
}

#[derive(Debug)]
enum RpcError {
    Coordinator(crate::ProjectionalCoordinatorError),
    Editor(ProjectionalEditorError),
    Text(String),
}

impl RpcError {
    const fn code(&self) -> &'static str {
        match self {
            Self::Coordinator(error)
            | Self::Editor(ProjectionalEditorError::Coordinator(error)) => {
                coordinator_error_code(error)
            }
            Self::Editor(ProjectionalEditorError::SourceEdit(_)) | Self::Text(_) => {
                "source_edit_rejected"
            }
            Self::Editor(_) => "editor_rejected",
        }
    }
}

const fn coordinator_error_code(error: &crate::ProjectionalCoordinatorError) -> &'static str {
    match error {
        crate::ProjectionalCoordinatorError::Plan(_) => "patch_rejected",
        crate::ProjectionalCoordinatorError::Intent(_) => "session_rejected",
        crate::ProjectionalCoordinatorError::Materialization(_) => "materialization_rejected",
        crate::ProjectionalCoordinatorError::Native(_) => "native_rejected",
        crate::ProjectionalCoordinatorError::Document(_) => "document_rejected",
        _ => "coordinator_rejected",
    }
}

impl From<crate::ProjectionalCoordinatorError> for RpcError {
    fn from(error: crate::ProjectionalCoordinatorError) -> Self {
        Self::Coordinator(error)
    }
}

impl From<ProjectionalEditorError> for RpcError {
    fn from(error: ProjectionalEditorError) -> Self {
        Self::Editor(error)
    }
}

impl std::fmt::Display for RpcError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Coordinator(error) => error.fmt(formatter),
            Self::Editor(error) => error.fmt(formatter),
            Self::Text(error) => error.fmt(formatter),
        }
    }
}
