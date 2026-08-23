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
    IntentWorkbenchProjection, ProjectionalIntentCoordinator,
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
        IntentRpcSnapshot {
            identity: self.coordinator.intent().identity(),
            projection: IntentWorkbenchProjection::from_session(self.coordinator.intent()),
            accepted_validation: self
                .coordinator
                .accepted_materialization()
                .map(|accepted| accepted.validation.clone()),
        }
    }

    /// Applies one typed request and returns a stable structured outcome.
    #[must_use]
    pub fn apply(&mut self, request: IntentRpcRequest) -> IntentRpcOutcome {
        let result: Result<IntentRpcSuccess, RpcError> = match request {
            IntentRpcRequest::Snapshot => Ok(IntentRpcSuccess::Snapshot {
                snapshot: Box::new(self.snapshot()),
            }),
            IntentRpcRequest::ApplyPatch { patch } => self
                .coordinator
                .apply_patch(*patch)
                .map_err(RpcError::from)
                .map(|outcome| IntentRpcSuccess::Patch {
                    disposition: outcome.disposition,
                    aliases: outcome.aliases,
                    snapshot: Box::new(self.snapshot()),
                }),
            IntentRpcRequest::Undo => {
                self.coordinator
                    .undo()
                    .map_err(RpcError::from)
                    .map(|moved| IntentRpcSuccess::History {
                        moved: moved.is_some(),
                        snapshot: Box::new(self.snapshot()),
                    })
            }
            IntentRpcRequest::Redo => {
                self.coordinator
                    .redo()
                    .map_err(RpcError::from)
                    .map(|moved| IntentRpcSuccess::History {
                        moved: moved.is_some(),
                        snapshot: Box::new(self.snapshot()),
                    })
            }
            IntentRpcRequest::Inspector { node } => Ok(IntentRpcSuccess::Inspector {
                inspector: IntentWorkbenchProjection::from_session(self.coordinator.intent())
                    .inspector(self.coordinator.intent(), node),
                snapshot: Box::new(self.snapshot()),
            }),
            IntentRpcRequest::EditSourceToken { token, replacement } => {
                let projection = IntentWorkbenchProjection::from_session(self.coordinator.intent());
                projection
                    .structured_source
                    .patch_for_edit(self.coordinator.intent(), token, &replacement)
                    .map_err(|error| error.to_string())
                    .and_then(|patch| {
                        self.coordinator
                            .apply_patch(patch)
                            .map_err(|error| error.to_string())
                    })
                    .map(|outcome| IntentRpcSuccess::Patch {
                        disposition: outcome.disposition,
                        aliases: outcome.aliases,
                        snapshot: Box::new(self.snapshot()),
                    })
                    .map_err(RpcError::Text)
            }
        };
        match result {
            Ok(value) => IntentRpcOutcome::Success { value },
            Err(error) => IntentRpcOutcome::Failure {
                failure: IntentRpcFailure {
                    code: error.code().to_owned(),
                    message: error.to_string(),
                    identity: Some(self.coordinator.intent().identity()),
                },
            },
        }
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
        let outcome = if request.len() > MAX_INTENT_RPC_REQUEST_BYTES {
            IntentRpcOutcome::Failure {
                failure: IntentRpcFailure {
                    code: "request_too_large".to_owned(),
                    message: format!(
                        "intent RPC request exceeds {MAX_INTENT_RPC_REQUEST_BYTES} bytes"
                    ),
                    identity: Some(self.coordinator.intent().identity()),
                },
            }
        } else {
            match serde_json::from_str(request) {
                Ok(request) => self.apply(request),
                Err(error) => IntentRpcOutcome::Failure {
                    failure: IntentRpcFailure {
                        code: "invalid_request".to_owned(),
                        message: error.to_string(),
                        identity: Some(self.coordinator.intent().identity()),
                    },
                },
            }
        };
        serde_json::to_string(&outcome).expect("closed RPC response is infallibly serializable")
    }
}

#[derive(Debug)]
enum RpcError {
    Coordinator(crate::ProjectionalCoordinatorError),
    Text(String),
}

impl RpcError {
    const fn code(&self) -> &'static str {
        match self {
            Self::Coordinator(crate::ProjectionalCoordinatorError::Plan(_)) => "patch_rejected",
            Self::Coordinator(crate::ProjectionalCoordinatorError::Intent(_)) => "session_rejected",
            Self::Coordinator(crate::ProjectionalCoordinatorError::Materialization(_)) => {
                "materialization_rejected"
            }
            Self::Coordinator(crate::ProjectionalCoordinatorError::Native(_)) => "native_rejected",
            Self::Coordinator(crate::ProjectionalCoordinatorError::Document(_)) => {
                "document_rejected"
            }
            Self::Coordinator(_) => "coordinator_rejected",
            Self::Text(_) => "source_edit_rejected",
        }
    }
}

impl From<crate::ProjectionalCoordinatorError> for RpcError {
    fn from(error: crate::ProjectionalCoordinatorError) -> Self {
        Self::Coordinator(error)
    }
}

impl std::fmt::Display for RpcError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Coordinator(error) => error.fmt(formatter),
            Self::Text(error) => error.fmt(formatter),
        }
    }
}
