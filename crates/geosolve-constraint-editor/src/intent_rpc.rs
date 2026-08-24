// SPDX-License-Identifier: GPL-3.0-or-later

//! DOM-free JSON RPC over the projectional intent coordinator.
//!
//! Native and WASM hosts call this exact implementation. The protocol carries
//! typed Rust patches and projections; it does not evaluate TypeScript or own
//! any geometry/constraint equation.

use geosolve_sketch_intent::{
    IntentAliasMap, IntentPatch, IntentPatchOperation, IntentPlanDisposition,
    IntentSessionIdentity, NodeId,
};
use serde::{Deserialize, Serialize};
use std::io::Write;

use crate::{
    IntentGraphSnapshot, IntentInspectorProjection, IntentSourceEditError, IntentSourceTokenId,
    IntentValidationEvidence, IntentWorkbenchProjection, ProjectionalEditorError,
    ProjectionalEditorSession, ProjectionalIntentCoordinator, ProjectionalPatchOutcome,
};

/// Maximum accepted byte length for one DOM-free intent RPC request.
///
/// The session and graph codecs have their own larger persistence bounds. RPC
/// requests are deliberately narrower so an untrusted host message cannot
/// force an unbounded JSON parse before those inner limits are reached.
pub const MAX_INTENT_RPC_REQUEST_BYTES: usize = 16 * 1024 * 1024;

/// Maximum encoded size reserved for one successful mutation receipt.
///
/// Unlike the explicit read-only Snapshot and Inspector queries, `ApplyPatch`,
/// `EditSourceToken`, Undo, and Redo never return a graph or workbench
/// projection. `ApplyPatch` is rejected before planning or publication when its
/// schema-generated alias map could exceed this conservative bound.
pub const MAX_INTENT_RPC_MUTATION_RECEIPT_BYTES: usize = 16 * 1024 * 1024;

/// Maximum compact JSON response emitted by the DOM-free RPC producer.
///
/// Structured Rust and JSON callers share this ceiling. Direct snapshot and
/// projection APIs remain available when a host needs another transport.
pub const MAX_INTENT_RPC_RESPONSE_BYTES: usize = 64 * 1024 * 1024;

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
        expected: Box<IntentSessionIdentity>,
        token: IntentSourceTokenId,
        replacement: String,
    },
}

/// Current equation-free projection and independently validated native status.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IntentRpcSnapshot {
    pub identity: IntentSessionIdentity,
    pub graph: IntentGraphSnapshot,
    pub projection: IntentWorkbenchProjection,
    pub accepted_validation: Option<IntentValidationEvidence>,
}

/// Bounded proof of one committed patch or Structured Source edit.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IntentRpcPatchReceipt {
    pub identity: IntentSessionIdentity,
    pub disposition: IntentPlanDisposition,
    pub aliases: IntentAliasMap,
}

/// Fixed-size proof of one Undo or Redo request.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IntentRpcHistoryReceipt {
    pub identity: IntentSessionIdentity,
    pub moved: bool,
}

/// Successful RPC result.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "result", rename_all = "snake_case", deny_unknown_fields)]
pub enum IntentRpcSuccess {
    Snapshot {
        snapshot: Box<IntentRpcSnapshot>,
    },
    Patch {
        receipt: Box<IntentRpcPatchReceipt>,
    },
    History {
        receipt: Box<IntentRpcHistoryReceipt>,
    },
    Inspector {
        identity: Box<IntentSessionIdentity>,
        inspector: Option<Box<IntentInspectorProjection>>,
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
    Failure { failure: Box<IntentRpcFailure> },
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

    /// Applies one typed request and returns a stable response-bounded outcome.
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
        expected: IntentSessionIdentity,
        token: IntentSourceTokenId,
        replacement: &str,
        maximum_response_bytes: usize,
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
        expected: IntentSessionIdentity,
        token: IntentSourceTokenId,
        replacement: &str,
        maximum_response_bytes: usize,
    ) -> Result<ProjectionalPatchOutcome, RpcError> {
        if self.coordinator.intent().identity() != expected {
            return Err(RpcError::Source(IntentSourceEditError::StaleProjection));
        }
        let projection = IntentWorkbenchProjection::from_session(self.coordinator.intent());
        projection
            .structured_source
            .patch_for_edit(self.coordinator.intent(), token, replacement)
            .map_err(RpcError::Source)
            .and_then(|patch| {
                ensure_patch_response_bound(&patch, maximum_response_bytes)?;
                Ok(patch)
            })
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
        expected: IntentSessionIdentity,
        token: IntentSourceTokenId,
        replacement: &str,
        maximum_response_bytes: usize,
    ) -> Result<ProjectionalPatchOutcome, RpcError> {
        if self.coordinator().intent().identity() != expected {
            return Err(RpcError::Source(IntentSourceEditError::StaleProjection));
        }
        let projection = self.workbench_projection();
        let patch = projection
            .structured_source
            .patch_for_edit(self.coordinator().intent(), token, replacement)
            .map_err(RpcError::Source)?;
        ensure_patch_response_bound(&patch, maximum_response_bytes)?;
        ProjectionalEditorSession::apply_patch(self, patch).map_err(RpcError::from)
    }
}

fn snapshot(coordinator: &ProjectionalIntentCoordinator) -> IntentRpcSnapshot {
    IntentRpcSnapshot {
        identity: coordinator.intent().identity(),
        graph: IntentGraphSnapshot::from_session(coordinator.intent()),
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
    apply_to_backend_with_response_limit(backend, request, MAX_INTENT_RPC_RESPONSE_BYTES)
}

fn apply_to_backend_with_response_limit(
    backend: &mut impl IntentRpcBackend,
    request: IntentRpcRequest,
    maximum_response_bytes: usize,
) -> IntentRpcOutcome {
    let result: Result<IntentRpcSuccess, RpcError> = match request {
        IntentRpcRequest::Snapshot => Ok(IntentRpcSuccess::Snapshot {
            snapshot: Box::new(snapshot(backend.coordinator())),
        }),
        IntentRpcRequest::ApplyPatch { patch } => {
            ensure_patch_response_bound(&patch, maximum_response_bytes)
                .and_then(|()| backend.apply_patch(*patch))
                .map(patch_success)
        }
        IntentRpcRequest::Undo => ensure_fixed_mutation_response_bound(maximum_response_bytes)
            .and_then(|()| backend.undo())
            .map(|moved| IntentRpcSuccess::History {
                receipt: Box::new(IntentRpcHistoryReceipt {
                    identity: backend.coordinator().intent().identity(),
                    moved,
                }),
            }),
        IntentRpcRequest::Redo => ensure_fixed_mutation_response_bound(maximum_response_bytes)
            .and_then(|()| backend.redo())
            .map(|moved| IntentRpcSuccess::History {
                receipt: Box::new(IntentRpcHistoryReceipt {
                    identity: backend.coordinator().intent().identity(),
                    moved,
                }),
            }),
        IntentRpcRequest::Inspector { node } => Ok(IntentRpcSuccess::Inspector {
            identity: Box::new(backend.coordinator().intent().identity()),
            inspector: IntentInspectorProjection::from_session(
                backend.coordinator().intent(),
                node,
            )
            .map(Box::new),
        }),
        IntentRpcRequest::EditSourceToken {
            expected,
            token,
            replacement,
        } => backend
            .edit_source_token(*expected, token, &replacement, maximum_response_bytes)
            .map(patch_success),
    };
    let outcome = match result {
        Ok(value) => IntentRpcOutcome::Success { value },
        Err(error) => IntentRpcOutcome::Failure {
            failure: Box::new(IntentRpcFailure {
                code: error.code().to_owned(),
                message: error.to_string(),
                identity: Some(backend.coordinator().intent().identity()),
            }),
        },
    };
    enforce_structured_response_bound(
        outcome,
        backend.coordinator().intent().identity(),
        maximum_response_bytes,
    )
}

fn patch_success(outcome: ProjectionalPatchOutcome) -> IntentRpcSuccess {
    IntentRpcSuccess::Patch {
        receipt: Box::new(IntentRpcPatchReceipt {
            identity: outcome.identity,
            disposition: outcome.disposition,
            aliases: outcome.aliases,
        }),
    }
}

// This estimate deliberately overstates Serde's compact JSON encoding:
// - one key byte is charged as six escaped bytes in both alias maps;
// - every generated selector plus typed port reference is charged 128 bytes;
//   the longest closed selector and port-kind entry encodes to 117 bytes;
// - the shared Rust schema query counts every fixed, child, result, paired
//   source and explicitly declared curve-span port exactly.
// Keeping the estimate below 16 MiB therefore keeps the actual receipt below
// both this producer contract and the TypeScript transport's 64 MiB query
// ceiling without discovering an oversized response after publication.
const RECEIPT_FIXED_UPPER_BYTES: usize = 8 * 1024;
const RECEIPT_ALIAS_FIXED_UPPER_BYTES: usize = 512;
const RECEIPT_PORT_UPPER_BYTES: usize = 128;

#[cfg(test)]
fn ensure_patch_receipt_bound(patch: &IntentPatch) -> Result<(), RpcError> {
    ensure_patch_response_bound(patch, usize::MAX)
}

fn ensure_patch_response_bound(
    patch: &IntentPatch,
    maximum_response_bytes: usize,
) -> Result<(), RpcError> {
    let mut bytes = RECEIPT_FIXED_UPPER_BYTES;
    for operation in patch.operations() {
        match operation {
            IntentPatchOperation::CreateNode { alias, draft, .. } => {
                let key_bytes = alias.as_str().len();
                let Some(port_count) = draft.schema_generated_port_count() else {
                    // The graph validator rejects this malformed/resource-
                    // excessive shape before expanding its generated ports.
                    // Preserve that owning-layer error instead of guessing a
                    // receipt for a transaction which cannot commit.
                    continue;
                };
                let node_bytes = key_bytes
                    .checked_mul(12)
                    .and_then(|value| value.checked_add(RECEIPT_ALIAS_FIXED_UPPER_BYTES))
                    .and_then(|value| {
                        port_count
                            .checked_mul(RECEIPT_PORT_UPPER_BYTES)
                            .and_then(|ports| value.checked_add(ports))
                    })
                    .ok_or(RpcError::ReceiptTooLarge)?;
                bytes = bytes
                    .checked_add(node_bytes)
                    .ok_or(RpcError::ReceiptTooLarge)?;
            }
            IntentPatchOperation::CreateCell { alias, .. } => {
                let cell_bytes = alias
                    .as_str()
                    .len()
                    .checked_mul(6)
                    .and_then(|value| value.checked_add(RECEIPT_ALIAS_FIXED_UPPER_BYTES))
                    .ok_or(RpcError::ReceiptTooLarge)?;
                bytes = bytes
                    .checked_add(cell_bytes)
                    .ok_or(RpcError::ReceiptTooLarge)?;
            }
            IntentPatchOperation::DeleteNode { .. }
            | IntentPatchOperation::SetSuppressed { .. }
            | IntentPatchOperation::SetDefinitionField { .. }
            | IntentPatchOperation::SetInstanceLeaf { .. }
            | IntentPatchOperation::RebindInput { .. }
            | IntentPatchOperation::EjectBootstrapPoint { .. }
            | IntentPatchOperation::RenameNode { .. }
            | IntentPatchOperation::MoveDeclaration { .. }
            | IntentPatchOperation::DeleteCell { .. }
            | IntentPatchOperation::ReorderCells { .. }
            | IntentPatchOperation::ReplaceExternalInputs { .. } => {}
        }
        if bytes > MAX_INTENT_RPC_MUTATION_RECEIPT_BYTES {
            return Err(RpcError::ReceiptTooLarge);
        }
    }
    if bytes > maximum_response_bytes {
        return Err(RpcError::ResponseTooLarge {
            maximum_bytes: maximum_response_bytes,
        });
    }
    Ok(())
}

fn ensure_fixed_mutation_response_bound(maximum_response_bytes: usize) -> Result<(), RpcError> {
    if RECEIPT_FIXED_UPPER_BYTES > maximum_response_bytes {
        return Err(RpcError::ResponseTooLarge {
            maximum_bytes: maximum_response_bytes,
        });
    }
    Ok(())
}

fn apply_json_to_backend(backend: &mut impl IntentRpcBackend, request: &str) -> String {
    apply_json_to_backend_with_response_limit(backend, request, MAX_INTENT_RPC_RESPONSE_BYTES)
}

fn apply_json_to_backend_with_response_limit(
    backend: &mut impl IntentRpcBackend,
    request: &str,
    maximum_response_bytes: usize,
) -> String {
    let outcome = if request.len() > MAX_INTENT_RPC_REQUEST_BYTES {
        IntentRpcOutcome::Failure {
            failure: Box::new(IntentRpcFailure {
                code: "request_too_large".to_owned(),
                message: format!("intent RPC request exceeds {MAX_INTENT_RPC_REQUEST_BYTES} bytes"),
                identity: Some(backend.coordinator().intent().identity()),
            }),
        }
    } else {
        match serde_json::from_str(request) {
            Ok(request) => {
                apply_to_backend_with_response_limit(backend, request, maximum_response_bytes)
            }
            Err(error) => IntentRpcOutcome::Failure {
                failure: Box::new(IntentRpcFailure {
                    code: "invalid_request".to_owned(),
                    message: error.to_string(),
                    identity: Some(backend.coordinator().intent().identity()),
                }),
            },
        }
    };
    encode_bounded_outcome(
        &outcome,
        backend.coordinator().intent().identity(),
        maximum_response_bytes,
    )
}

struct BoundedJsonWriter<W> {
    inner: W,
    written: usize,
    maximum_bytes: usize,
    exceeded: bool,
}

impl<W> BoundedJsonWriter<W> {
    const fn new(inner: W, maximum_bytes: usize) -> Self {
        Self {
            inner,
            written: 0,
            maximum_bytes,
            exceeded: false,
        }
    }

    fn into_inner(self) -> W {
        self.inner
    }
}

impl<W: Write> Write for BoundedJsonWriter<W> {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        if buffer.len() > self.maximum_bytes.saturating_sub(self.written) {
            self.exceeded = true;
            return Err(std::io::Error::other(
                "intent RPC response budget exhausted",
            ));
        }
        let written = self.inner.write(buffer)?;
        self.written = self
            .written
            .checked_add(written)
            .expect("bounded RPC byte count cannot overflow");
        Ok(written)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

fn outcome_fits_response_bound(outcome: &IntentRpcOutcome, maximum_bytes: usize) -> bool {
    let mut writer = BoundedJsonWriter::new(std::io::sink(), maximum_bytes);
    match serde_json::to_writer(&mut writer, outcome) {
        Ok(()) => true,
        Err(_) if writer.exceeded => false,
        Err(error) => panic!("closed RPC response is infallibly serializable: {error}"),
    }
}

fn encode_outcome_with_limit(outcome: &IntentRpcOutcome, maximum_bytes: usize) -> Option<String> {
    let mut writer = BoundedJsonWriter::new(Vec::new(), maximum_bytes);
    match serde_json::to_writer(&mut writer, outcome) {
        Ok(()) => Some(
            String::from_utf8(writer.into_inner())
                .expect("serde_json always emits valid UTF-8 response bytes"),
        ),
        Err(_) if writer.exceeded => None,
        Err(error) => panic!("closed RPC response is infallibly serializable: {error}"),
    }
}

fn response_too_large_outcome(
    identity: IntentSessionIdentity,
    maximum_bytes: usize,
) -> IntentRpcOutcome {
    IntentRpcOutcome::Failure {
        failure: Box::new(IntentRpcFailure {
            code: "response_too_large".to_owned(),
            message: format!("intent RPC response exceeds {maximum_bytes} bytes"),
            identity: Some(identity),
        }),
    }
}

fn enforce_structured_response_bound(
    outcome: IntentRpcOutcome,
    identity: IntentSessionIdentity,
    maximum_bytes: usize,
) -> IntentRpcOutcome {
    if outcome_fits_response_bound(&outcome, maximum_bytes) {
        return outcome;
    }
    let failure = response_too_large_outcome(identity, maximum_bytes);
    assert!(
        outcome_fits_response_bound(&failure, maximum_bytes),
        "intent RPC response budget must fit its closed failure envelope"
    );
    failure
}

fn encode_bounded_outcome(
    outcome: &IntentRpcOutcome,
    identity: IntentSessionIdentity,
    maximum_bytes: usize,
) -> String {
    if let Some(encoded) = encode_outcome_with_limit(outcome, maximum_bytes) {
        return encoded;
    }
    encode_outcome_with_limit(
        &response_too_large_outcome(identity, maximum_bytes),
        maximum_bytes,
    )
    .expect("intent RPC response budget must fit its closed failure envelope")
}

#[derive(Debug)]
enum RpcError {
    Coordinator(crate::ProjectionalCoordinatorError),
    Editor(ProjectionalEditorError),
    Source(IntentSourceEditError),
    ReceiptTooLarge,
    ResponseTooLarge { maximum_bytes: usize },
}

impl RpcError {
    const fn code(&self) -> &'static str {
        match self {
            Self::Coordinator(error)
            | Self::Editor(ProjectionalEditorError::Coordinator(error)) => {
                coordinator_error_code(error)
            }
            Self::Editor(ProjectionalEditorError::SourceEdit(_)) | Self::Source(_) => {
                "source_edit_rejected"
            }
            Self::Editor(_) => "editor_rejected",
            Self::ReceiptTooLarge => "receipt_too_large",
            Self::ResponseTooLarge { .. } => "response_too_large",
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
            Self::Source(error) => error.fmt(formatter),
            Self::ReceiptTooLarge => write!(
                formatter,
                "intent RPC mutation receipt would exceed {MAX_INTENT_RPC_MUTATION_RECEIPT_BYTES} bytes"
            ),
            Self::ResponseTooLarge { maximum_bytes } => {
                write!(
                    formatter,
                    "intent RPC response exceeds {maximum_bytes} bytes"
                )
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use geosolve_sketch::{DocumentId, PersistentId};
    use geosolve_sketch_intent::{
        GeometryRecipeKind, IntentKey, IntentNodeDraft, IntentNodeKind, IntentPatch,
        IntentPatchOperation, IntentPatchPolicy, IntentSession, IntentSessionId,
    };

    use super::{
        IntentRpcFailure, IntentRpcOutcome, IntentRpcRequest, IntentRpcSession, IntentRpcSuccess,
        apply_json_to_backend_with_response_limit, apply_to_backend_with_response_limit,
        encode_bounded_outcome, ensure_patch_receipt_bound,
    };
    use crate::{ColdIntentMaterializer, ProjectionalIntentCoordinator};

    fn rpc() -> IntentRpcSession {
        IntentRpcSession::new(
            ProjectionalIntentCoordinator::empty(
                IntentSessionId::from_raw(0x8300_40fd),
                ColdIntentMaterializer::with_default_policy(
                    DocumentId(PersistentId::from_u128(0x8300_40fd_u128 << 32)),
                    1.0,
                )
                .unwrap(),
            )
            .unwrap(),
        )
    }

    fn create_cell_request(rpc: &IntentRpcSession) -> IntentRpcRequest {
        IntentRpcRequest::ApplyPatch {
            patch: Box::new(IntentPatch::new(
                rpc.coordinator().intent().identity(),
                IntentPatchPolicy::RequireAccepted,
                vec![IntentPatchOperation::CreateCell {
                    alias: IntentKey::new("cell").unwrap(),
                    name: IntentKey::new("Response budget cell").unwrap(),
                    before: None,
                }],
            )),
        }
    }

    #[test]
    fn bulk_trivial_code_patch_fits_schema_exact_receipt_preflight() {
        const POINTS: usize = 512;
        let session = IntentSession::with_id(IntentSessionId::from_raw(0x8300_40ff)).unwrap();
        let operations = (0..POINTS)
            .map(|index| {
                let alias = IntentKey::new(format!("point-{index:04}")).unwrap();
                IntentPatchOperation::CreateNode {
                    alias,
                    draft: Box::new(IntentNodeDraft::new(
                        IntentNodeKind::Geometry {
                            recipe: GeometryRecipeKind::SketchPoint,
                        },
                        IntentKey::new(format!("rpc.bulk.point.{index:04}")).unwrap(),
                    )),
                    cell: None,
                }
            })
            .collect();
        let patch = IntentPatch::new(
            session.identity(),
            IntentPatchPolicy::RequireAccepted,
            operations,
        );

        ensure_patch_receipt_bound(&patch)
            .expect("512 one-port declarations fit the bounded mutation receipt");
    }

    #[test]
    fn malformed_excessive_operation_spans_reject_before_schema_expansion() {
        let draft =
            IntentNodeDraft::new(
                IntentNodeKind::Operation {
                    operation: geosolve_sketch_intent::OperationKind::ProfileOffset,
                },
                IntentKey::new("rpc.hostile.operation").unwrap(),
            )
            .with_operation_outputs(vec![
            geosolve_sketch_intent::IntentOperationOutput::curve(u16::MAX);
            4_096
        ]);

        assert_eq!(draft.schema_generated_port_count(), None);
    }

    #[test]
    fn json_producer_replaces_an_excessive_query_with_a_bounded_failure() {
        let session = IntentSession::with_id(IntentSessionId::from_raw(0x8300_40fe)).unwrap();
        let oversized = IntentRpcOutcome::Failure {
            failure: Box::new(IntentRpcFailure {
                code: "synthetic".to_owned(),
                message: "x".repeat(4_096),
                identity: Some(session.identity()),
            }),
        };

        let encoded = encode_bounded_outcome(&oversized, session.identity(), 2_048);
        let decoded: IntentRpcOutcome = serde_json::from_str(&encoded).unwrap();
        assert!(matches!(
            decoded,
            IntentRpcOutcome::Failure { ref failure }
                if failure.code == "response_too_large"
                    && failure.identity == Some(session.identity())
        ));
        assert!(encoded.len() < 2_048);
    }

    #[test]
    fn structured_and_json_mutation_responses_share_an_injectable_budget() {
        const UNDER_BUDGET: usize = 16 * 1024;
        const OVER_BUDGET: usize = 4 * 1024;

        let mut structured = rpc();
        let mut json = rpc();
        let structured_request = create_cell_request(&structured);
        let structured_success =
            apply_to_backend_with_response_limit(&mut structured, structured_request, UNDER_BUDGET);
        let request = serde_json::to_string(&create_cell_request(&json)).unwrap();
        let json_success: IntentRpcOutcome = serde_json::from_str(
            &apply_json_to_backend_with_response_limit(&mut json, &request, UNDER_BUDGET),
        )
        .unwrap();
        assert_eq!(structured_success, json_success);
        assert!(matches!(
            &structured_success,
            IntentRpcOutcome::Success {
                value: IntentRpcSuccess::Patch { .. }
            }
        ));
        assert_eq!(
            structured.coordinator().intent().identity(),
            json.coordinator().intent().identity()
        );
        assert!(serde_json::to_vec(&structured_success).unwrap().len() <= UNDER_BUDGET);

        let mut structured = rpc();
        let mut json = rpc();
        let structured_before = structured.snapshot();
        let json_before = json.snapshot();
        assert_eq!(structured_before, json_before);
        let structured_request = create_cell_request(&structured);
        let structured_failure =
            apply_to_backend_with_response_limit(&mut structured, structured_request, OVER_BUDGET);
        let request = serde_json::to_string(&create_cell_request(&json)).unwrap();
        let json_failure: IntentRpcOutcome = serde_json::from_str(
            &apply_json_to_backend_with_response_limit(&mut json, &request, OVER_BUDGET),
        )
        .unwrap();
        assert_eq!(structured_failure, json_failure);
        assert!(matches!(
            &structured_failure,
            IntentRpcOutcome::Failure { failure }
                if failure.code == "response_too_large"
                    && failure.identity == Some(structured_before.identity)
        ));
        assert!(serde_json::to_vec(&structured_failure).unwrap().len() <= OVER_BUDGET);
        assert_eq!(structured.snapshot(), structured_before);
        assert_eq!(json.snapshot(), json_before);
    }

    #[test]
    fn structured_snapshot_response_honors_the_exact_json_boundary() {
        let mut baseline = rpc();
        let expected = apply_to_backend_with_response_limit(
            &mut baseline,
            IntentRpcRequest::Snapshot,
            usize::MAX,
        );
        let exact_bytes = serde_json::to_vec(&expected).unwrap().len();

        let mut exact = rpc();
        assert_eq!(
            apply_to_backend_with_response_limit(
                &mut exact,
                IntentRpcRequest::Snapshot,
                exact_bytes,
            ),
            expected
        );
        let mut exact_json = rpc();
        let encoded = apply_json_to_backend_with_response_limit(
            &mut exact_json,
            r#"{"method":"snapshot"}"#,
            exact_bytes,
        );
        assert_eq!(encoded.len(), exact_bytes);
        assert_eq!(
            serde_json::from_str::<IntentRpcOutcome>(&encoded).unwrap(),
            expected
        );

        let mut short = rpc();
        let before = short.snapshot();
        let rejected = apply_to_backend_with_response_limit(
            &mut short,
            IntentRpcRequest::Snapshot,
            exact_bytes - 1,
        );
        assert!(matches!(
            rejected,
            IntentRpcOutcome::Failure { ref failure }
                if failure.code == "response_too_large"
                    && failure.identity == Some(before.identity)
        ));
        assert_eq!(short.snapshot(), before);

        let mut short_json = rpc();
        let before = short_json.snapshot();
        let encoded = apply_json_to_backend_with_response_limit(
            &mut short_json,
            r#"{"method":"snapshot"}"#,
            exact_bytes - 1,
        );
        let rejected_json = serde_json::from_str::<IntentRpcOutcome>(&encoded).unwrap();
        assert_eq!(rejected_json, rejected);
        assert_eq!(short_json.snapshot(), before);
    }
}
