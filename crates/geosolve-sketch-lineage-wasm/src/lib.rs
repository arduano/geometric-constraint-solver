// SPDX-License-Identifier: GPL-3.0-or-later

//! DOM-free, stateful JSON RPC adapter for sketch lineage.
//!
//! The boundary deliberately accepts and returns strings. JavaScript never
//! participates in lineage validation, dependency planning, geometry, solving,
//! or accepted-state publication.

mod semantic;

use std::str::FromStr;

use geosolve_constraint_editor::{
    LINEAGE_DOMAIN_EVALUATOR, RetainedEditorCoordinator, evaluate_lineage_session_cold_with_inputs,
    lineage_external_input_stamp,
};
use geosolve_sketch::{
    DocumentId, DocumentSolveRequest, ExternalSnapshotSet, ParameterBatch, PersistentId,
    RetainedSketchDocumentSession, SketchDocument, SolverConfig,
};
use geosolve_sketch_lineage::{
    LineageDependencyPlanEntry, LineageDocument, LineageDocumentError, LineageDocumentId,
    LineageDocumentIdentity, LineageEvaluationPolicy, LineageMutation, LineageOpaqueId,
    LineagePatch, LineageSemanticKey, LineageSession, LineageStepEvaluationState,
};
use serde::Deserialize;
use serde_json::{Value, json};

/// Stable protocol discriminator for every request and response.
pub const LINEAGE_RPC_PROTOCOL: &str = "geosolve.lineage.rpc.v0";

const MAX_RPC_REQUEST_BYTES: usize = 16 * 1024 * 1024;
const RPC_HOST_INPUT_VERSION: u32 = 1;

/// Stateful, presentation-independent lineage RPC engine.
#[cfg_attr(target_arch = "wasm32", wasm_bindgen::prelude::wasm_bindgen)]
#[derive(Debug, Default)]
pub struct LineageRpcEngine {
    session: Option<LineageSession>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RpcEnvelope {
    protocol: String,
    method: String,
    request_id: String,
    #[serde(default)]
    session_id: Option<String>,
    #[serde(default)]
    params: Value,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateParams {
    #[serde(default)]
    document_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LoadParams {
    session_json: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ImportParams {
    lineage_json: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct MutateParams {
    patch: LineagePatch,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReconcileParams {
    expected: LineageDocumentIdentity,
    lineage_json: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PolicyParams {
    expected: LineageDocumentIdentity,
    policy: LineageEvaluationPolicy,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExpectedParams {
    expected: LineageDocumentIdentity,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct EvaluateParams {
    expected: LineageDocumentIdentity,
    host_inputs: RpcHostInputs,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RpcHostInputs {
    version: u32,
    parameter_batch_json: String,
    external_snapshot_set_json: String,
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen::prelude::wasm_bindgen)]
impl LineageRpcEngine {
    /// Creates an uninitialized RPC engine. Call `create` or `load` first.
    #[cfg_attr(
        target_arch = "wasm32",
        wasm_bindgen::prelude::wasm_bindgen(constructor)
    )]
    #[must_use]
    pub fn new() -> Self {
        Self { session: None }
    }

    /// Handles one bounded versioned JSON request and returns one JSON response.
    #[must_use]
    pub fn request(&mut self, request: &str) -> String {
        let parsed = if request.len() > MAX_RPC_REQUEST_BYTES {
            Err(RpcFailure::new(
                "request_too_large",
                "lineage RPC request exceeds the bounded input size",
            ))
        } else {
            serde_json::from_str::<RpcEnvelope>(request)
                .map_err(|error| RpcFailure::new("invalid_request", error.to_string()))
        };
        let envelope = match parsed {
            Ok(envelope) => envelope,
            Err(error) => return encode_response(None, None, None, Err(error)),
        };
        let request_id = envelope.request_id.clone();
        let method = envelope.method.clone();
        if envelope.protocol != LINEAGE_RPC_PROTOCOL {
            return encode_response(
                Some(&request_id),
                Some(&method),
                envelope.session_id.as_deref(),
                Err(RpcFailure::new(
                    "unsupported_protocol",
                    format!(
                        "unsupported lineage RPC protocol `{}`; expected `{LINEAGE_RPC_PROTOCOL}`",
                        envelope.protocol
                    ),
                )),
            );
        }
        let supplied_session_id = envelope.session_id.clone();
        let result = self.dispatch(
            &envelope.method,
            envelope.session_id.as_deref(),
            envelope.params,
        );
        let response_session_id = if result.is_ok() {
            self.rpc_session_id()
        } else {
            supplied_session_id
        };
        encode_response(
            Some(&request_id),
            Some(&method),
            response_session_id.as_deref(),
            result,
        )
    }
}

impl LineageRpcEngine {
    fn dispatch(
        &mut self,
        method: &str,
        session_id: Option<&str>,
        params: Value,
    ) -> Result<Value, RpcFailure> {
        if matches!(method, "create" | "load" | "import") {
            if session_id.is_some() {
                return Err(RpcFailure::new(
                    "unexpected_session",
                    "create, load, and import start a session and must not target an existing one",
                ));
            }
        } else {
            self.ensure_session_id(session_id)?;
        }
        match method {
            "create" => self.create(parse_params(params)?),
            "load" => self.load(&parse_params(params)?),
            "import" => self.import(&parse_params(params)?),
            "inspect" => self.snapshot(),
            "reconcile" => self.reconcile(&parse_params(params)?),
            "mutate" => self.mutate(parse_params(params)?),
            "rewrite_owners" => self.rewrite_owners(parse_params(params)?),
            "set_policy" => self.set_policy(&parse_params(params)?),
            "undo" => self.undo(&parse_params(params)?),
            "redo" => self.redo(&parse_params(params)?),
            "evaluate" => self.evaluate(&parse_params(params)?),
            "export" => self.export(),
            "export_lineage" => self.export_lineage(),
            _ => Err(RpcFailure::new(
                "unknown_method",
                format!("unknown lineage RPC method `{method}`"),
            )),
        }
    }

    fn create(&mut self, params: CreateParams) -> Result<Value, RpcFailure> {
        let document_id = params.document_id.map_or_else(
            || Ok(LineageDocument::new().id()),
            |value| {
                LineageDocumentId::from_str(&value)
                    .map_err(|error| RpcFailure::new("invalid_document_id", error.to_string()))
            },
        )?;
        let sketch =
            SketchDocument::with_id(10.0, DocumentId(PersistentId::from_u128(document_id.raw())))
                .map_err(|error| RpcFailure::new("create_failed", error.to_string()))?;
        let retained = RetainedSketchDocumentSession::new(
            sketch,
            DocumentSolveRequest::default(),
            SolverConfig::default(),
        )
        .map_err(|error| RpcFailure::new("create_failed", error.to_string()))?;
        let coordinator = RetainedEditorCoordinator::new(retained)
            .map_err(|error| RpcFailure::new("create_failed", error.to_string()))?;
        let candidate = LineageSession::from_session_json(
            &coordinator
                .lineage_session_json()
                .map_err(|error| RpcFailure::new("create_failed", error.to_string()))?,
        )
        .map_err(RpcFailure::lineage)?;
        validate_session_semantics(&candidate)?;
        self.session = Some(candidate);
        self.snapshot()
    }

    fn load(&mut self, params: &LoadParams) -> Result<Value, RpcFailure> {
        let mut candidate =
            LineageSession::from_session_json(&params.session_json).map_err(RpcFailure::lineage)?;
        validate_session_semantics(&candidate)?;
        // The session wire digest authenticates structure, not the truth of a
        // caller-provided accepted materialization digest. Preserve the full
        // declarative program/history, but require this engine's ordinary cold
        // evaluator to establish fresh accepted authority before export.
        candidate.discard_unverified_evaluation_authority();
        let result = snapshot_value(&candidate)?;
        self.session = Some(candidate);
        Ok(result)
    }

    fn import(&mut self, params: &ImportParams) -> Result<Value, RpcFailure> {
        let candidate =
            LineageSession::from_json(&params.lineage_json).map_err(RpcFailure::lineage)?;
        validate_session_semantics(&candidate)?;
        let result = snapshot_value(&candidate)?;
        self.session = Some(candidate);
        Ok(result)
    }

    fn snapshot(&self) -> Result<Value, RpcFailure> {
        snapshot_value(self.session()?)
    }

    fn mutate(&mut self, params: MutateParams) -> Result<Value, RpcFailure> {
        let mut schema_candidate = self.session()?.document().clone();
        schema_candidate
            .apply_patch(params.patch.clone())
            .map_err(RpcFailure::lineage)?;
        semantic::validate_document(&schema_candidate)
            .map_err(|error| RpcFailure::new(error.code, error.message))?;

        let mut candidate = self.session()?.clone();
        let outcome = candidate
            .apply_patch(params.patch)
            .map_err(RpcFailure::lineage)?;
        validate_session_semantics(&candidate)?;
        self.session = Some(candidate);
        Ok(json!({
            "identity": identity_value(outcome.identity),
            "changed": outcome.changed,
            "inserted_steps": outcome.inserted_steps.iter().map(ToString::to_string).collect::<Vec<_>>(),
            "tombstoned_steps": outcome.tombstoned_steps.iter().map(ToString::to_string).collect::<Vec<_>>(),
        }))
    }

    fn rewrite_owners(&mut self, params: MutateParams) -> Result<Value, RpcFailure> {
        if !params
            .patch
            .mutations
            .iter()
            .all(|mutation| matches!(mutation, LineageMutation::Rewrite { .. }))
        {
            return Err(RpcFailure::new(
                "invalid_owner_rewrite",
                "rewrite_owners accepts only one atomic batch of existing-step rewrites",
            ));
        }
        self.mutate(params)
    }

    fn reconcile(&mut self, params: &ReconcileParams) -> Result<Value, RpcFailure> {
        let replacement =
            LineageDocument::from_json(&params.lineage_json).map_err(RpcFailure::lineage)?;
        validate_document_schema(&replacement)?;
        let mut candidate = self.session()?.clone();
        let identity = candidate
            .reconcile(params.expected, replacement)
            .map_err(RpcFailure::lineage)?;
        validate_session_semantics(&candidate)?;
        self.session = Some(candidate);
        Ok(json!({ "identity": identity_value(identity) }))
    }

    fn set_policy(&mut self, params: &PolicyParams) -> Result<Value, RpcFailure> {
        self.mutate(MutateParams {
            patch: LineagePatch::new(
                params.expected,
                vec![LineageMutation::SetEvaluationPolicy {
                    policy: params.policy,
                }],
            ),
        })
    }

    fn undo(&mut self, params: &ExpectedParams) -> Result<Value, RpcFailure> {
        self.ensure_expected(params.expected)?;
        let mut candidate = self.session()?.clone();
        candidate
            .undo()
            .map_err(RpcFailure::lineage)?
            .ok_or_else(|| {
                RpcFailure::new("nothing_to_undo", "lineage history has no prior entry")
            })?;
        validate_session_semantics(&candidate)?;
        let result = snapshot_value(&candidate)?;
        self.session = Some(candidate);
        Ok(result)
    }

    fn redo(&mut self, params: &ExpectedParams) -> Result<Value, RpcFailure> {
        self.ensure_expected(params.expected)?;
        let mut candidate = self.session()?.clone();
        candidate
            .redo()
            .map_err(RpcFailure::lineage)?
            .ok_or_else(|| {
                RpcFailure::new("nothing_to_redo", "lineage history has no later entry")
            })?;
        validate_session_semantics(&candidate)?;
        let result = snapshot_value(&candidate)?;
        self.session = Some(candidate);
        Ok(result)
    }

    #[allow(
        clippy::too_many_lines,
        reason = "the stateful RPC method keeps exact host-input decoding, dependency planning, owning-domain evaluation, and atomic accepted-or-failed authority visibly ordered"
    )]
    fn evaluate(&mut self, params: &EvaluateParams) -> Result<Value, RpcFailure> {
        self.ensure_expected(params.expected)?;
        validate_document_schema(self.session()?.document())?;
        let (parameters, snapshots, external_inputs) = parse_host_inputs(&params.host_inputs)?;
        let plan = self
            .session()?
            .document()
            .dependency_plan([])
            .map_err(RpcFailure::lineage)?;
        let mut failed_steps = plan
            .iter()
            .filter_map(|entry| {
                matches!(entry.state, LineageStepEvaluationState::Blocked { .. })
                    .then_some(entry.step)
            })
            .collect::<Vec<_>>();
        let step_values = plan.iter().map(plan_entry_value).collect::<Vec<_>>();
        let (disposition, evidence) = if failed_steps.is_empty() {
            match evaluate_lineage_session_cold_with_inputs(
                self.session()?,
                &parameters,
                &snapshots,
            ) {
                Ok(evaluation) => {
                    if evaluation.external_inputs() != &external_inputs {
                        return Err(RpcFailure::new(
                            "host_input_identity_mismatch",
                            "cold evaluation returned a different host-input identity",
                        ));
                    }
                    self.session_mut()?
                        .accept_current(
                            params.expected,
                            Some(external_inputs.clone()),
                            evaluation.materialization_digest(),
                        )
                        .map_err(RpcFailure::lineage)?;
                    (
                        "accepted",
                        json!({
                            "evaluator": LINEAGE_DOMAIN_EVALUATOR,
                            "external_inputs": evaluation.external_inputs().as_str(),
                            "materialization_digest": evaluation.materialization_digest().to_string(),
                            "sketch_digest": evaluation.sketch_digest().to_string(),
                            "feature_digest": evaluation.feature_digest().to_string(),
                            "computed_feature_count": evaluation.computed_feature_count(),
                            "validated_prefix_count": evaluation.validated_prefix_count(),
                            "strict_prefix_digest": evaluation.strict_prefix_digest().to_string(),
                        }),
                    )
                }
                Err(failure) => {
                    failed_steps = failure
                        .failed_step()
                        .or_else(|| {
                            plan.iter().rev().find_map(|entry| {
                                matches!(entry.state, LineageStepEvaluationState::Ready)
                                    .then_some(entry.step)
                            })
                        })
                        .into_iter()
                        .collect();
                    if failed_steps.is_empty() {
                        return Err(RpcFailure::new(failure.code(), failure.message()));
                    }
                    let diagnostic = LineageSemanticKey::new(failure.diagnostic())
                        .expect("domain evaluator diagnostics are static valid semantic keys");
                    self.session_mut()?
                        .reject_current(
                            params.expected,
                            Some(external_inputs.clone()),
                            diagnostic,
                            failed_steps.clone(),
                        )
                        .map_err(RpcFailure::lineage)?;
                    (
                        "failed",
                        json!({
                            "evaluator": LINEAGE_DOMAIN_EVALUATOR,
                            "code": failure.code(),
                            "message": failure.message(),
                            "failed_step": failure.failed_step().map(|step| step.to_string()),
                            "failed_features": failure.failed_features(),
                        }),
                    )
                }
            }
        } else {
            self.session_mut()?
                .reject_current(
                    params.expected,
                    Some(external_inputs),
                    LineageSemanticKey::new("semantic-evaluation-blocked")
                        .expect("static semantic diagnostic is valid"),
                    failed_steps,
                )
                .map_err(RpcFailure::lineage)?;
            (
                "failed",
                json!({
                    "evaluator": LINEAGE_DOMAIN_EVALUATOR,
                    "code": "dependency_blocked",
                    "message": "lineage dependency planning blocked one or more live steps",
                    "failed_features": [],
                }),
            )
        };
        Ok(json!({
            "identity": identity_value(self.session()?.identity()),
            "disposition": disposition,
            "evidence": evidence,
            "steps": step_values,
            "attempt": self.session()?.latest_attempt(),
            "last_accepted": self.session()?.last_accepted(),
        }))
    }

    fn export(&self) -> Result<Value, RpcFailure> {
        let session = self.session()?;
        Ok(json!({
            "identity": identity_value(session.identity()),
            "session_json": session.to_canonical_session_json().map_err(RpcFailure::lineage)?,
        }))
    }

    fn export_lineage(&self) -> Result<Value, RpcFailure> {
        let session = self.session()?;
        Ok(json!({
            "identity": identity_value(session.identity()),
            "lineage_json": session.to_canonical_json().map_err(RpcFailure::lineage)?,
        }))
    }

    fn ensure_session_id(&self, supplied: Option<&str>) -> Result<(), RpcFailure> {
        let actual = self.rpc_session_id().ok_or_else(|| {
            RpcFailure::new("not_initialized", "call create, load, or import first")
        })?;
        match supplied {
            Some(value) if value == actual => Ok(()),
            Some(_) => Err(RpcFailure::new(
                "wrong_session",
                "request targets a different lineage RPC session",
            )),
            None => Err(RpcFailure::new(
                "missing_session",
                "initialized lineage RPC requests require the retained session ID",
            )),
        }
    }

    fn rpc_session_id(&self) -> Option<String> {
        self.session
            .as_ref()
            .map(|session| format!("lineage:{}", session.identity().document))
    }

    fn ensure_expected(&self, expected: LineageDocumentIdentity) -> Result<(), RpcFailure> {
        let actual = self.session()?.identity();
        if expected.document != actual.document {
            return Err(RpcFailure::new(
                "wrong_document",
                format!(
                    "expected lineage document {}, current is {}",
                    expected.document, actual.document
                ),
            ));
        }
        if expected.revision != actual.revision || expected.digest != actual.digest {
            return Err(RpcFailure::new(
                "stale_revision",
                format!(
                    "expected lineage revision {}/{}, current is {}/{}",
                    expected.revision, expected.digest, actual.revision, actual.digest
                ),
            ));
        }
        Ok(())
    }

    fn session(&self) -> Result<&LineageSession, RpcFailure> {
        self.session
            .as_ref()
            .ok_or_else(|| RpcFailure::new("not_initialized", "call create or load first"))
    }

    fn session_mut(&mut self) -> Result<&mut LineageSession, RpcFailure> {
        self.session
            .as_mut()
            .ok_or_else(|| RpcFailure::new("not_initialized", "call create or load first"))
    }
}

fn parse_params<T: for<'de> Deserialize<'de>>(params: Value) -> Result<T, RpcFailure> {
    serde_json::from_value(params)
        .map_err(|error| RpcFailure::new("invalid_params", error.to_string()))
}

fn parse_host_inputs(
    value: &RpcHostInputs,
) -> Result<(ParameterBatch, ExternalSnapshotSet, LineageOpaqueId), RpcFailure> {
    if value.version != RPC_HOST_INPUT_VERSION {
        return Err(RpcFailure::new(
            "unsupported_host_input_version",
            format!(
                "unsupported RPC host-input version {}; expected {RPC_HOST_INPUT_VERSION}",
                value.version
            ),
        ));
    }
    let parameters = ParameterBatch::from_json(&value.parameter_batch_json)
        .map_err(|error| RpcFailure::new("invalid_parameter_batch", error.to_string()))?;
    let snapshots = ExternalSnapshotSet::from_json(&value.external_snapshot_set_json)
        .map_err(|error| RpcFailure::new("invalid_external_snapshot_set", error.to_string()))?;
    let identity = lineage_external_input_stamp(&parameters, &snapshots)
        .map_err(|error| RpcFailure::new(error.code(), error.message()))?;
    Ok((parameters, snapshots, identity))
}

fn plan_entry_value(entry: &LineageDependencyPlanEntry) -> Value {
    let (state, dependencies) = match &entry.state {
        LineageStepEvaluationState::Ready => ("ready", Vec::new()),
        LineageStepEvaluationState::Suppressed => ("suppressed", Vec::new()),
        LineageStepEvaluationState::Tombstoned => ("tombstoned", Vec::new()),
        LineageStepEvaluationState::Failed => ("failed", Vec::new()),
        LineageStepEvaluationState::Blocked { dependencies } => (
            "blocked",
            dependencies.iter().map(ToString::to_string).collect(),
        ),
        LineageStepEvaluationState::Unevaluated => ("unevaluated", Vec::new()),
    };
    json!({
        "step": entry.step.to_string(),
        "state": state,
        "dependencies": dependencies,
    })
}

fn validate_document_schema(document: &LineageDocument) -> Result<(), RpcFailure> {
    semantic::validate_document(document)
        .map_err(|error| RpcFailure::new(error.code, error.message))?;
    RetainedEditorCoordinator::validate_lineage_document_semantics(document)
        .map_err(|error| RpcFailure::new("invalid_editor_authority", error.to_string()))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SemanticSessionCheckpointWire {
    document: String,
    #[serde(rename = "latest_attempt")]
    _latest_attempt: Value,
    last_accepted_document: Option<String>,
    #[serde(rename = "last_accepted")]
    _last_accepted: Value,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SemanticSessionWire {
    #[serde(rename = "version")]
    _version: u32,
    document: String,
    #[serde(rename = "latest_attempt")]
    _latest_attempt: Value,
    last_accepted_document: Option<String>,
    #[serde(rename = "last_accepted")]
    _last_accepted: Value,
    undo: Vec<SemanticSessionCheckpointWire>,
    redo: Vec<SemanticSessionCheckpointWire>,
    #[serde(rename = "lifecycle")]
    _lifecycle: Value,
    #[serde(rename = "auxiliary_high_waters")]
    _auxiliary_high_waters: Value,
    #[serde(rename = "digest")]
    _digest: Value,
}

fn validate_session_semantics(session: &LineageSession) -> Result<(), RpcFailure> {
    let json = session
        .to_canonical_session_json()
        .map_err(RpcFailure::lineage)?;
    let wire = serde_json::from_str::<SemanticSessionWire>(&json)
        .map_err(|error| RpcFailure::new("internal_serialization", error.to_string()))?;
    validate_document_json(&wire.document)?;
    if let Some(document) = wire.last_accepted_document.as_deref() {
        validate_document_json(document)?;
    }
    for checkpoint in wire.undo.iter().chain(&wire.redo) {
        validate_document_json(&checkpoint.document)?;
        if let Some(document) = checkpoint.last_accepted_document.as_deref() {
            validate_document_json(document)?;
        }
    }
    RetainedEditorCoordinator::validate_lineage_session_semantics(session)
        .map_err(|error| RpcFailure::new("invalid_editor_authority", error.to_string()))
}

fn validate_document_json(json: &str) -> Result<(), RpcFailure> {
    let document = LineageDocument::from_json(json).map_err(RpcFailure::lineage)?;
    validate_document_schema(&document)
}

fn snapshot_value(session: &LineageSession) -> Result<Value, RpcFailure> {
    let lineage_json = session.to_canonical_json().map_err(RpcFailure::lineage)?;
    let document = serde_json::from_str::<Value>(&lineage_json)
        .map_err(|error| RpcFailure::new("internal_serialization", error.to_string()))?;
    let last_accepted_document = session
        .last_accepted_document()
        .map(LineageDocument::to_canonical_json)
        .transpose()
        .map_err(RpcFailure::lineage)?
        .map(|json| serde_json::from_str::<Value>(&json))
        .transpose()
        .map_err(|error| RpcFailure::new("internal_serialization", error.to_string()))?;
    let lifecycle = session.lifecycle_high_water();
    Ok(json!({
        "session_id": format!("lineage:{}", session.identity().document),
        "identity": identity_value(session.identity()),
        "evaluation_policy": match session.document().evaluation_policy() {
            LineageEvaluationPolicy::StrictChronological => "strict_chronological",
            LineageEvaluationPolicy::DependencyLocal => "dependency_local",
        },
        "can_undo": session.can_undo(),
        "can_redo": session.can_redo(),
        "undo_length": session.undo_len().to_string(),
        "redo_length": session.redo_len().to_string(),
        "lifecycle": {
            "revision": lifecycle.revision.to_string(),
            "next_step_id": lifecycle.allocator.next_step_id.to_string(),
            "next_output_id": lifecycle.allocator.next_output_id.to_string(),
            "next_reservation_id": lifecycle.allocator.next_reservation_id.to_string(),
        },
        "latest_attempt": session.latest_attempt(),
        "last_accepted": session.last_accepted(),
        "last_accepted_document": last_accepted_document,
        "document": document,
    }))
}

fn identity_value(identity: LineageDocumentIdentity) -> Value {
    json!({
        "document": identity.document.to_string(),
        "revision": identity.revision.to_string(),
        "digest": identity.digest.to_string(),
    })
}

#[derive(Debug)]
struct RpcFailure {
    code: &'static str,
    message: String,
}

impl RpcFailure {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    #[allow(
        clippy::needless_pass_by_value,
        reason = "the map_err adapter converts and releases the complete owned domain error at the RPC boundary"
    )]
    fn lineage(error: LineageDocumentError) -> Self {
        let code = match &error {
            LineageDocumentError::StalePatch { .. } => "stale_revision",
            LineageDocumentError::WrongPatchDocument { .. } => "wrong_document",
            LineageDocumentError::WrongOutputKind { .. }
            | LineageDocumentError::ReservationKindMismatch { .. }
            | LineageDocumentError::RebindKindMismatch { .. } => "wrong_port_kind",
            LineageDocumentError::RetiredOutputReference { .. }
            | LineageDocumentError::ConsumedOutputReference { .. }
            | LineageDocumentError::LiveDependent { .. }
            | LineageDocumentError::InvalidBaselinePosition { .. }
            | LineageDocumentError::NonSequentialReplacementRevision { .. }
            | LineageDocumentError::AllocatorRegression { .. }
            | LineageDocumentError::InvalidEvaluationAttempt { .. }
            | LineageDocumentError::EmptyPatch
            | LineageDocumentError::TombstonedStep { .. } => "invalid_state_transition",
            LineageDocumentError::IdExhausted => "id_exhausted",
            LineageDocumentError::RevisionExhausted => "revision_exhausted",
            LineageDocumentError::InvalidFailedStep { .. } => "invalid_failed_step",
            LineageDocumentError::JsonResourceLimit { .. }
            | LineageDocumentError::SessionJsonResourceLimit { .. }
            | LineageDocumentError::ResourceLimit { .. } => "resource_limit",
            LineageDocumentError::CrossHistoryIdentityRebinding { .. }
            | LineageDocumentError::InvalidSessionAuthority { .. } => "invalid_editor_authority",
            _ => "lineage_rejected",
        };
        Self::new(code, error.to_string())
    }
}

fn encode_response(
    request_id: Option<&str>,
    method: Option<&str>,
    session_id: Option<&str>,
    result: Result<Value, RpcFailure>,
) -> String {
    let value = match result {
        Ok(result) => json!({
            "protocol": LINEAGE_RPC_PROTOCOL,
            "request_id": request_id,
            "method": method,
            "session_id": session_id,
            "ok": true,
            "result": result,
        }),
        Err(error) => json!({
            "protocol": LINEAGE_RPC_PROTOCOL,
            "request_id": request_id,
            "method": method,
            "session_id": session_id,
            "ok": false,
            "error": {
                "code": error.code,
                "message": error.message,
            },
        }),
    };
    serde_json::to_string(&value).unwrap_or_else(|_| {
        "{\"protocol\":\"geosolve.lineage.rpc.v0\",\"ok\":false,\"error\":{\"code\":\"internal_serialization\",\"message\":\"response serialization failed\"}}".into()
    })
}

#[cfg(test)]
mod tests {
    use geosolve_constraint_editor::RetainedEditorCoordinator;
    use geosolve_sketch::{
        DocumentId, DocumentSolveRequest, ExternalSnapshotSet, ParameterBatch, PersistentId,
        RetainedSketchDocumentSession, SketchDocument, SolverConfig,
    };
    use geosolve_sketch_lineage::{
        LineageActionDefinition, LineageDeveloperKey, LineageDocument, LineageDocumentError,
        LineageDocumentId, LineageEvaluationPolicy, LineageInputBinding, LineageMutation,
        LineageOpaqueId, LineageOutput, LineageOutputId, LineageOutputKind, LineageOutputRef,
        LineagePatch, LineageReservation, LineageReservationId, LineageReservationKind,
        LineageRevision, LineageSemanticKey, LineageSession, LineageStep, LineageStepId,
        LineageStepRewrite, LineageWritableLeaf, VersionedActionPayload,
    };
    use serde_json::{Value, json};

    use super::{LINEAGE_RPC_PROTOCOL, LineageRpcEngine, lineage_external_input_stamp};

    fn host_inputs() -> Value {
        json!({
            "version": 1,
            "parameter_batch_json": ParameterBatch::default()
                .to_canonical_json()
                .expect("canonical empty parameter batch"),
            "external_snapshot_set_json": ExternalSnapshotSet::default()
                .to_canonical_json()
                .expect("canonical empty external snapshot set"),
        })
    }

    fn point_action(schema: &str, version: u32) -> LineageActionDefinition {
        LineageActionDefinition::GeometryRecipe {
            action: VersionedActionPayload::empty(
                LineageSemanticKey::new(schema).expect("point schema"),
                version,
            ),
        }
    }

    fn point_step(action: LineageActionDefinition) -> LineageStep {
        numbered_point_step(1, "point", "sketch:point:1", action)
    }

    fn numbered_point_step(
        id: u64,
        key: &str,
        persistent_id: &str,
        action: LineageActionDefinition,
    ) -> LineageStep {
        LineageStep::new(
            LineageStepId::from_raw(id),
            LineageDeveloperKey::new(key).expect("developer key"),
            "Point",
            action,
            vec![LineageOutput {
                id: LineageOutputId::from_raw(id),
                key: LineageSemanticKey::new("point").expect("output key"),
                kind: LineageOutputKind::Point,
                reservation: Some(LineageReservationId::from_raw(id)),
            }],
            vec![LineageReservation {
                id: LineageReservationId::from_raw(id),
                key: LineageSemanticKey::new("point").expect("reservation key"),
                kind: LineageReservationKind::Point,
                persistent_id: LineageOpaqueId::new(persistent_id).expect("persistent point ID"),
            }],
        )
    }

    fn accepted_workbench_session(document_id: LineageDocumentId) -> LineageSession {
        let sketch =
            SketchDocument::with_id(10.0, DocumentId(PersistentId::from_u128(document_id.raw())))
                .expect("empty workbench sketch");
        let retained = RetainedSketchDocumentSession::new(
            sketch,
            DocumentSolveRequest::default(),
            SolverConfig::default(),
        )
        .expect("retained workbench sketch");
        let coordinator =
            RetainedEditorCoordinator::new(retained).expect("retained workbench coordinator");
        LineageSession::from_session_json(
            &coordinator
                .lineage_session_json()
                .expect("canonical workbench lineage session"),
        )
        .expect("strict workbench lineage session")
    }

    fn forged_workbench_session(document_id: LineageDocumentId) -> LineageSession {
        let accepted = accepted_workbench_session(document_id);
        let mut forged_document = LineageDocument::with_id(accepted.document().id());
        for source_step in accepted.document().steps() {
            let mut step = source_step.clone();
            if matches!(
                step.action,
                LineageActionDefinition::ImportedBaseline { .. }
            ) {
                let result = step
                    .outputs
                    .iter()
                    .find(|output| output.kind == LineageOutputKind::Collection)
                    .expect("baseline result output");
                step.writable_leaves.push(LineageWritableLeaf {
                    output: result.id,
                    key: LineageSemanticKey::new("forged-authority").expect("forged field"),
                });
            }
            forged_document
                .apply_patch(LineagePatch::new(
                    forged_document.identity(),
                    vec![LineageMutation::Insert {
                        before: None,
                        step: Box::new(step),
                    }],
                ))
                .expect("forged document remains structurally valid");
        }
        LineageSession::new(forged_document).expect("structurally valid forged session")
    }

    fn nonmaterializable_point_step() -> LineageStep {
        LineageStep::new(
            LineageStepId::from_raw(2),
            LineageDeveloperKey::new("rpc-point").expect("developer key"),
            "RPC point",
            point_action("geosolve.geometry.v1.sketch-point", 1),
            vec![LineageOutput {
                id: LineageOutputId::from_raw(2),
                key: LineageSemanticKey::new("point").expect("output key"),
                kind: LineageOutputKind::Point,
                reservation: Some(LineageReservationId::from_raw(1)),
            }],
            vec![LineageReservation {
                id: LineageReservationId::from_raw(1),
                key: LineageSemanticKey::new("point").expect("reservation key"),
                kind: LineageReservationKind::Point,
                persistent_id: LineageOpaqueId::new("sketch:point:1").expect("persistent point ID"),
            }],
        )
    }

    fn dependent_step(document_id: LineageDocumentId) -> LineageStep {
        LineageStep::new(
            LineageStepId::from_raw(2),
            LineageDeveloperKey::new("fixed-point").expect("developer key"),
            "Fixed point",
            LineageActionDefinition::Constraint {
                action: VersionedActionPayload {
                    schema: LineageSemanticKey::new("geosolve.constraint.v1.fixed-point")
                        .expect("constraint schema"),
                    version: 1,
                    inputs: vec![LineageInputBinding {
                        key: LineageSemanticKey::new("point").expect("input key"),
                        kind: LineageOutputKind::Point,
                        source: LineageOutputRef {
                            document: document_id,
                            step: LineageStepId::from_raw(1),
                            output: LineageOutputId::from_raw(1),
                            kind: LineageOutputKind::Point,
                        },
                    }],
                    parameters: std::collections::BTreeMap::new(),
                },
            },
            vec![LineageOutput {
                id: LineageOutputId::from_raw(2),
                key: LineageSemanticKey::new("constraint").expect("output key"),
                kind: LineageOutputKind::Constraint,
                reservation: Some(LineageReservationId::from_raw(2)),
            }],
            vec![LineageReservation {
                id: LineageReservationId::from_raw(2),
                key: LineageSemanticKey::new("constraint").expect("reservation key"),
                kind: LineageReservationKind::Constraint,
                persistent_id: LineageOpaqueId::new("sketch:constraint:1")
                    .expect("persistent constraint ID"),
            }],
        )
    }

    fn two_step_document(document_id: LineageDocumentId) -> LineageDocument {
        let mut document = LineageDocument::with_id(document_id);
        let expected = document.identity();
        document
            .apply_patch(LineagePatch::new(
                expected,
                vec![
                    LineageMutation::Insert {
                        before: None,
                        step: Box::new(point_step(point_action(
                            "geosolve.geometry.v1.sketch-point",
                            1,
                        ))),
                    },
                    LineageMutation::Insert {
                        before: None,
                        step: Box::new(dependent_step(document_id)),
                    },
                ],
            ))
            .expect("two-step replacement");
        document
    }

    #[allow(
        clippy::needless_pass_by_value,
        reason = "the unit helper takes ownership of each one-shot JSON payload to mirror an RPC request envelope"
    )]
    fn request(
        engine: &mut LineageRpcEngine,
        session_id: Option<&str>,
        method: &str,
        params: Value,
    ) -> Value {
        serde_json::from_str(
            &engine.request(
                &json!({
                    "protocol": LINEAGE_RPC_PROTOCOL,
                    "request_id": "test",
                    "session_id": session_id,
                    "method": method,
                    "params": params,
                })
                .to_string(),
            ),
        )
        .expect("response JSON")
    }

    #[test]
    fn create_snapshot_export_and_load_are_dom_free_and_canonical() {
        let mut engine = LineageRpcEngine::new();
        let created = request(&mut engine, None, "create", json!({}));
        assert_eq!(created["ok"], true);
        assert_eq!(created["request_id"], "test");
        let session_id = created["session_id"].as_str().expect("session ID");
        let evaluated = request(
            &mut engine,
            Some(session_id),
            "evaluate",
            json!({
                "expected": created["result"]["identity"],
                "host_inputs": host_inputs(),
            }),
        );
        assert_eq!(evaluated["ok"], true, "{evaluated:#}");
        assert_eq!(evaluated["result"]["disposition"], "accepted");
        assert_eq!(evaluated["result"]["evidence"]["validated_prefix_count"], 1);
        let exported = request(&mut engine, Some(session_id), "export", json!({}));
        assert_eq!(exported["ok"], true);
        let session_json = exported["result"]["session_json"]
            .as_str()
            .expect("canonical session");
        let mut restored = LineageRpcEngine::new();
        let loaded = request(
            &mut restored,
            None,
            "load",
            json!({ "session_json": session_json }),
        );
        assert_eq!(loaded["ok"], true);
        assert_eq!(loaded["result"]["identity"], created["result"]["identity"]);
        assert_eq!(loaded["result"]["latest_attempt"]["disposition"], "pending");
        assert!(loaded["result"]["last_accepted"].is_null());
    }

    #[test]
    fn protocol_and_initialization_fail_closed() {
        let mut engine = LineageRpcEngine::new();
        let absent = request(&mut engine, None, "inspect", json!({}));
        assert_eq!(absent["error"]["code"], "not_initialized");
        let wrong: Value = serde_json::from_str(
            &engine.request(
                &json!({
                    "protocol": "other",
                    "request_id": "wrong",
                    "method": "create",
                    "params": {},
                })
                .to_string(),
            ),
        )
        .expect("response JSON");
        assert_eq!(wrong["error"]["code"], "unsupported_protocol");
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "one closed table keeps every public lineage failure class and stable RPC code directly comparable"
    )]
    fn lineage_failures_have_stable_discriminated_rpc_error_classes() {
        let document = LineageDocumentId::from_raw(0x83_0010);
        let step = LineageStepId::from_raw(1);
        let provider = LineageStepId::from_raw(2);
        let output = LineageOutputId::from_raw(1);
        let reservation = LineageReservationId::from_raw(1);
        let successor = LineageOutputRef {
            document,
            step: provider,
            output: LineageOutputId::from_raw(2),
            kind: LineageOutputKind::Point,
        };
        let cases = [
            (
                LineageDocumentError::WrongOutputKind {
                    step,
                    expected: LineageOutputKind::Point,
                    actual: LineageOutputKind::Scalar,
                },
                "wrong_port_kind",
            ),
            (
                LineageDocumentError::ReservationKindMismatch {
                    step,
                    reservation,
                    output: LineageOutputKind::Scalar,
                },
                "wrong_port_kind",
            ),
            (
                LineageDocumentError::RebindKindMismatch {
                    step,
                    input: "point".into(),
                    expected: LineageOutputKind::Point,
                    actual: LineageOutputKind::Scalar,
                },
                "wrong_port_kind",
            ),
            (
                LineageDocumentError::RetiredOutputReference {
                    step,
                    provider,
                    output,
                },
                "invalid_state_transition",
            ),
            (
                LineageDocumentError::ConsumedOutputReference {
                    step,
                    provider,
                    output,
                    successor,
                },
                "invalid_state_transition",
            ),
            (
                LineageDocumentError::LiveDependent {
                    deleted: provider,
                    dependent: step,
                },
                "invalid_state_transition",
            ),
            (
                LineageDocumentError::InvalidBaselinePosition { step },
                "invalid_state_transition",
            ),
            (
                LineageDocumentError::NonSequentialReplacementRevision {
                    expected: LineageRevision::from_raw(2),
                    actual: LineageRevision::from_raw(3),
                },
                "invalid_state_transition",
            ),
            (
                LineageDocumentError::AllocatorRegression {
                    allocator: "step",
                    minimum: "2".into(),
                    actual: "1".into(),
                },
                "invalid_state_transition",
            ),
            (
                LineageDocumentError::InvalidEvaluationAttempt {
                    message: "invalid test transition",
                },
                "invalid_state_transition",
            ),
            (LineageDocumentError::EmptyPatch, "invalid_state_transition"),
            (
                LineageDocumentError::TombstonedStep { step },
                "invalid_state_transition",
            ),
            (LineageDocumentError::IdExhausted, "id_exhausted"),
            (
                LineageDocumentError::RevisionExhausted,
                "revision_exhausted",
            ),
            (
                LineageDocumentError::InvalidSessionAuthority {
                    message: "invalid test authority",
                },
                "invalid_editor_authority",
            ),
        ];

        for (error, expected_code) in cases {
            let response: Value = serde_json::from_str(&super::encode_response(
                Some("classification"),
                Some("mutate"),
                Some("lineage:test"),
                Err(super::RpcFailure::lineage(error)),
            ))
            .expect("classified response JSON");
            assert_eq!(response["ok"], false);
            assert_eq!(response["error"]["code"], expected_code);
            assert!(response.get("result").is_none());
        }
    }

    #[test]
    fn wrong_port_and_invalid_transition_rpc_requests_reject_atomically() {
        let document_id = LineageDocumentId::from_raw(0x83_0011);
        let mut engine = LineageRpcEngine::new();
        let imported = request(
            &mut engine,
            None,
            "import",
            json!({
                "lineage_json": two_step_document(document_id)
                    .to_canonical_json()
                    .expect("two-step lineage JSON"),
            }),
        );
        assert_eq!(imported["ok"], true, "{imported:#}");
        let session_id = imported["session_id"].as_str().expect("session ID");
        let expected = serde_json::from_value(imported["result"]["identity"].clone())
            .expect("imported identity");
        let before = request(&mut engine, Some(session_id), "export", json!({}));

        let wrong_kind = request(
            &mut engine,
            Some(session_id),
            "mutate",
            json!({
                "patch": LineagePatch::new(
                    expected,
                    vec![LineageMutation::Rebind {
                        step: LineageStepId::from_raw(2),
                        input: LineageSemanticKey::new("point").expect("input key"),
                        target: LineageOutputRef {
                            document: document_id,
                            step: LineageStepId::from_raw(1),
                            output: LineageOutputId::from_raw(1),
                            kind: LineageOutputKind::Scalar,
                        },
                    }],
                ),
            }),
        );
        assert_eq!(wrong_kind["ok"], false, "{wrong_kind:#}");
        assert_eq!(wrong_kind["error"]["code"], "wrong_port_kind");
        assert_eq!(
            request(&mut engine, Some(session_id), "export", json!({})),
            before,
            "wrong-port rejection must preserve the exact session",
        );

        let invalid_transition = request(
            &mut engine,
            Some(session_id),
            "mutate",
            json!({
                "patch": LineagePatch::new(
                    expected,
                    vec![LineageMutation::Tombstone {
                        step: LineageStepId::from_raw(1),
                    }],
                ),
            }),
        );
        assert_eq!(invalid_transition["ok"], false, "{invalid_transition:#}");
        assert_eq!(
            invalid_transition["error"]["code"],
            "invalid_state_transition"
        );
        assert_eq!(
            request(&mut engine, Some(session_id), "export", json!({})),
            before,
            "invalid transition must preserve the exact session",
        );
    }

    #[test]
    fn exact_expected_identity_distinguishes_wrong_document_from_stale_revision() {
        let mut engine = LineageRpcEngine::new();
        let created = request(&mut engine, None, "create", json!({}));
        assert_eq!(created["ok"], true, "{created:#}");
        let session_id = created["session_id"].as_str().expect("session ID");
        let before = request(&mut engine, Some(session_id), "export", json!({}));
        let mut wrong_document = created["result"]["identity"].clone();
        wrong_document["document"] = json!(LineageDocumentId::from_raw(0x83_dead).to_string());
        let rejected = request(
            &mut engine,
            Some(session_id),
            "evaluate",
            json!({
                "expected": wrong_document,
                "host_inputs": host_inputs(),
            }),
        );
        assert_eq!(rejected["ok"], false, "{rejected:#}");
        assert_eq!(rejected["error"]["code"], "wrong_document");
        let after = request(&mut engine, Some(session_id), "export", json!({}));
        assert_eq!(after, before, "wrong-document rejection must be atomic");
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "one table-driven RPC lifecycle test keeps every malformed exact-input case beside the byte-for-byte no-mutation assertion"
    )]
    fn exact_host_input_payloads_fail_closed_without_mutating_the_session() {
        let document_id = LineageDocumentId::from_raw(0x83_0100);
        let mut engine = LineageRpcEngine::new();
        let seed = accepted_workbench_session(document_id);
        let loaded = request(
            &mut engine,
            None,
            "load",
            json!({
                "session_json": seed.to_canonical_session_json().expect("seed session JSON"),
            }),
        );
        let session_id = loaded["session_id"].as_str().expect("session ID");
        let expected = loaded["result"]["identity"].clone();
        let before = request(&mut engine, Some(session_id), "export", json!({}));
        let before_json = before["result"]["session_json"].clone();
        let parameter_json = ParameterBatch::default()
            .to_canonical_json()
            .expect("canonical empty parameter batch");
        let snapshot_json = ExternalSnapshotSet::default()
            .to_canonical_json()
            .expect("canonical empty external snapshot set");

        let mut mismatched_parameter: Value =
            serde_json::from_str(&parameter_json).expect("parameter batch value");
        mismatched_parameter["digest"][0] = json!(
            mismatched_parameter["digest"][0]
                .as_u64()
                .expect("first parameter digest byte")
                ^ 1
        );
        let malformed_cases = [
            (json!({ "expected": expected }), "invalid_params"),
            (
                json!({
                    "expected": expected,
                    "host_inputs": {
                        "version": 2,
                        "parameter_batch_json": parameter_json,
                        "external_snapshot_set_json": snapshot_json,
                    },
                }),
                "unsupported_host_input_version",
            ),
            (
                json!({
                    "expected": expected,
                    "host_inputs": {
                        "version": 1,
                        "parameter_batch_json": "{",
                        "external_snapshot_set_json": snapshot_json,
                    },
                }),
                "invalid_parameter_batch",
            ),
            (
                json!({
                    "expected": expected,
                    "host_inputs": {
                        "version": 1,
                        "parameter_batch_json": mismatched_parameter.to_string(),
                        "external_snapshot_set_json": snapshot_json,
                    },
                }),
                "invalid_parameter_batch",
            ),
            (
                json!({
                    "expected": expected,
                    "host_inputs": {
                        "version": 1,
                        "parameter_batch_json": parameter_json,
                        "external_snapshot_set_json": "{",
                    },
                }),
                "invalid_external_snapshot_set",
            ),
        ];

        for (params, code) in malformed_cases {
            let rejected = request(&mut engine, Some(session_id), "evaluate", params);
            assert_eq!(rejected["error"]["code"], code, "{rejected:#}");
            let after = request(&mut engine, Some(session_id), "export", json!({}));
            assert_eq!(after["result"]["session_json"], before_json);
            assert_eq!(after["result"]["identity"], expected);
        }

        let rejected = request(
            &mut engine,
            Some(session_id),
            "record_nonpublishing",
            json!({
                "target": expected,
                "host_inputs": {
                    "version": 1,
                    "parameter_batch_json": parameter_json,
                    "external_snapshot_set_json": "{",
                },
                "disposition": "cancelled",
                "diagnostic": "host-cancelled",
            }),
        );
        assert_eq!(rejected["error"]["code"], "unknown_method");
        let after = request(&mut engine, Some(session_id), "export", json!({}));
        assert_eq!(after["result"]["session_json"], before_json);
        assert_eq!(after["result"]["identity"], expected);
    }

    #[test]
    fn exact_reconcile_and_history_are_stateful() {
        let document_id = LineageDocumentId::from_raw(0x8300);
        let mut engine = LineageRpcEngine::new();
        let initial = LineageDocument::with_id(document_id);
        let imported = request(
            &mut engine,
            None,
            "import",
            json!({
                "lineage_json": initial.to_canonical_json().expect("initial lineage JSON"),
            }),
        );
        let session_id = imported["session_id"].as_str().expect("session ID");
        let expected = imported["result"]["identity"].clone();

        let mut replacement = LineageDocument::with_id(document_id);
        replacement
            .apply_patch(LineagePatch::new(
                replacement.identity(),
                vec![LineageMutation::SetEvaluationPolicy {
                    policy: LineageEvaluationPolicy::DependencyLocal,
                }],
            ))
            .expect("replacement policy");
        let reconciled = request(
            &mut engine,
            Some(session_id),
            "reconcile",
            json!({
                "expected": expected,
                "lineage_json": replacement.to_canonical_json().expect("replacement JSON"),
            }),
        );
        assert_eq!(reconciled["ok"], true);
        let snapshot = request(&mut engine, Some(session_id), "inspect", json!({}));
        assert_eq!(snapshot["result"]["evaluation_policy"], "dependency_local");
        assert_eq!(snapshot["result"]["can_undo"], true);

        let undone = request(
            &mut engine,
            Some(session_id),
            "undo",
            json!({ "expected": snapshot["result"]["identity"] }),
        );
        assert_eq!(undone["ok"], true);
        assert_eq!(
            undone["result"]["evaluation_policy"],
            "strict_chronological"
        );
        assert_eq!(undone["result"]["latest_attempt"]["disposition"], "pending");
        assert_eq!(undone["result"]["can_redo"], true);
        let restored = request(&mut engine, Some(session_id), "inspect", json!({}));
        assert_eq!(
            restored["result"]["evaluation_policy"],
            "strict_chronological"
        );
        assert_eq!(restored["result"]["can_redo"], true);
        let redone = request(
            &mut engine,
            Some(session_id),
            "redo",
            json!({ "expected": restored["result"]["identity"] }),
        );
        assert_eq!(redone["ok"], true);
        assert_eq!(redone["result"]["evaluation_policy"], "dependency_local");
        assert_eq!(redone["result"]["latest_attempt"]["disposition"], "pending");
        assert_eq!(redone["result"]["can_undo"], true);
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "the full-session round trip intentionally exercises accepted authority, a later failed attempt, and exact export/load preservation in one lifecycle"
    )]
    fn full_session_load_preserves_program_history_but_requires_fresh_evaluation_authority() {
        let document_id = LineageDocumentId::from_raw(0x83_1000);
        let mut engine = LineageRpcEngine::new();
        let seed = accepted_workbench_session(document_id);
        let loaded = request(
            &mut engine,
            None,
            "load",
            json!({
                "session_json": seed.to_canonical_session_json().expect("seed session JSON"),
            }),
        );
        let session_id = loaded["session_id"].as_str().expect("session ID");

        let accepted = request(
            &mut engine,
            Some(session_id),
            "evaluate",
            json!({
                "expected": loaded["result"]["identity"],
                "host_inputs": host_inputs(),
            }),
        );
        assert_eq!(accepted["ok"], true);
        assert_eq!(accepted["result"]["disposition"], "accepted");
        assert_eq!(
            accepted["result"]["evidence"]["evaluator"],
            "geosolve.constraint-editor.lineage-cold.v1"
        );
        let expected_host_inputs = lineage_external_input_stamp(
            &ParameterBatch::default(),
            &ExternalSnapshotSet::default(),
        )
        .expect("default host-input identity");
        assert_eq!(
            accepted["result"]["evidence"]["external_inputs"],
            expected_host_inputs.as_str()
        );
        assert_eq!(
            accepted["result"]["attempt"]["external_inputs"],
            expected_host_inputs.as_str()
        );
        assert_eq!(
            accepted["result"]["last_accepted"]["external_inputs"],
            expected_host_inputs.as_str()
        );
        let policy = request(
            &mut engine,
            Some(session_id),
            "set_policy",
            json!({
                "expected": accepted["result"]["identity"],
                "policy": "dependency_local",
            }),
        );
        assert_eq!(policy["ok"], true);
        let policy_accepted = request(
            &mut engine,
            Some(session_id),
            "evaluate",
            json!({
                "expected": policy["result"]["identity"],
                "host_inputs": host_inputs(),
            }),
        );
        assert_eq!(policy_accepted["result"]["disposition"], "accepted");
        let inserted = request(
            &mut engine,
            Some(session_id),
            "mutate",
            json!({
                "patch": LineagePatch::new(
                    serde_json::from_value(policy_accepted["result"]["identity"].clone())
                        .expect("policy identity"),
                    vec![LineageMutation::Insert {
                        before: None,
                        step: Box::new(nonmaterializable_point_step()),
                    }],
                ),
            }),
        );
        assert_eq!(inserted["ok"], true, "{inserted:#}");
        let rejected = request(
            &mut engine,
            Some(session_id),
            "evaluate",
            json!({
                "expected": inserted["result"]["identity"],
                "host_inputs": host_inputs(),
            }),
        );
        assert_eq!(rejected["ok"], true, "{rejected:#}");
        assert_eq!(rejected["result"]["disposition"], "failed");
        assert_eq!(rejected["result"]["attempt"]["disposition"], "failed");
        assert_eq!(
            rejected["result"]["evidence"]["code"],
            "workbench_materialization_unsupported"
        );
        assert_eq!(
            rejected["result"]["evidence"]["failed_step"],
            LineageStepId::from_raw(2).to_string()
        );
        assert_eq!(
            rejected["result"]["attempt"]["failed_steps"],
            json!([LineageStepId::from_raw(2).to_string()])
        );
        assert_eq!(
            rejected["result"]["last_accepted"]["lineage"]["revision"],
            "0000000000000002"
        );

        let exported = request(&mut engine, Some(session_id), "export", json!({}));
        let session_json = exported["result"]["session_json"]
            .as_str()
            .expect("full session JSON");
        let mut restored = LineageRpcEngine::new();
        let loaded = request(
            &mut restored,
            None,
            "load",
            json!({ "session_json": session_json }),
        );
        assert_eq!(loaded["ok"], true);
        assert_eq!(loaded["result"]["latest_attempt"]["disposition"], "pending");
        assert!(loaded["result"]["last_accepted"].is_null());
        assert_eq!(loaded["result"]["can_undo"], true);
        let restored_session_id = loaded["session_id"].as_str().expect("restored session ID");
        let reexported = request(
            &mut restored,
            Some(restored_session_id),
            "export",
            json!({}),
        );
        assert_ne!(reexported["result"]["session_json"], session_json);
        let sanitized = LineageSession::from_session_json(
            reexported["result"]["session_json"]
                .as_str()
                .expect("sanitized session JSON"),
        )
        .expect("sanitized session");
        assert_eq!(
            sanitized.document(),
            engine.session().expect("source session").document()
        );
        assert_eq!(
            sanitized.undo_len(),
            engine.session().expect("source session").undo_len()
        );
        assert!(sanitized.last_accepted().is_none());

        let undone = request(
            &mut restored,
            Some(restored_session_id),
            "undo",
            json!({ "expected": loaded["result"]["identity"] }),
        );
        assert_eq!(undone["ok"], true, "{undone:#}");
        assert_eq!(undone["result"]["latest_attempt"]["disposition"], "pending");
        assert!(undone["result"]["last_accepted"].is_null());
        let inspected = request(
            &mut restored,
            Some(restored_session_id),
            "inspect",
            json!({}),
        );
        assert_eq!(
            inspected["result"]["latest_attempt"]["disposition"],
            "pending"
        );
        assert!(inspected["result"]["last_accepted"].is_null());
    }

    #[test]
    fn unsupported_schemas_versions_and_caller_certification_fail_atomically() {
        let document_id = LineageDocumentId::from_raw(0x83_2000);
        let mut engine = LineageRpcEngine::new();
        let replacement = two_step_document(document_id);
        let imported = request(
            &mut engine,
            None,
            "import",
            json!({
                "lineage_json": replacement.to_canonical_json().expect("replacement JSON"),
            }),
        );
        let session_id = imported["session_id"].as_str().expect("session ID");
        let retained_identity = imported["result"]["identity"].clone();

        for (schema, version, code) in [
            (
                "geosolve.geometry.v1.future-shape",
                1,
                "unsupported_action_schema",
            ),
            (
                "geosolve.geometry.v1.sketch-point",
                2,
                "unsupported_action_version",
            ),
        ] {
            let rejected = request(
                &mut engine,
                Some(session_id),
                "rewrite_owners",
                json!({
                    "patch": LineagePatch::new(
                        serde_json::from_value(retained_identity.clone()).expect("identity"),
                        vec![LineageMutation::Rewrite {
                            step: LineageStepId::from_raw(1),
                            replacement: Box::new(LineageStepRewrite {
                                label: "unsupported".into(),
                                action: point_action(schema, version),
                            }),
                        }],
                    ),
                }),
            );
            assert_eq!(rejected["error"]["code"], code);
            let inspected = request(&mut engine, Some(session_id), "inspect", json!({}));
            assert_eq!(inspected["result"]["identity"], retained_identity);
        }

        for method in ["accept", "reject"] {
            let rejected = request(&mut engine, Some(session_id), method, json!({}));
            assert_eq!(rejected["error"]["code"], "unknown_method");
        }

        let mut unknown_document = LineageDocument::with_id(document_id);
        unknown_document
            .apply_patch(LineagePatch::new(
                unknown_document.identity(),
                vec![LineageMutation::Insert {
                    before: None,
                    step: Box::new(point_step(point_action(
                        "geosolve.geometry.v1.future-shape",
                        1,
                    ))),
                }],
            ))
            .expect("structurally valid unknown schema");
        let hidden = LineageSession::new(unknown_document).expect("unknown-schema session");
        let mut loader = LineageRpcEngine::new();
        let rejected = request(
            &mut loader,
            None,
            "load",
            json!({
                "session_json": hidden.to_canonical_session_json().expect("session JSON"),
            }),
        );
        assert_eq!(rejected["error"]["code"], "unsupported_action_schema");
        let absent = request(&mut loader, None, "inspect", json!({}));
        assert_eq!(absent["error"]["code"], "not_initialized");
    }

    #[test]
    fn reconcile_validates_complete_candidate_session_and_rejects_atomically() {
        let document_id = LineageDocumentId::from_raw(0x83_2800);
        let mut engine = LineageRpcEngine::new();
        let initial = two_step_document(document_id);
        let imported = request(
            &mut engine,
            None,
            "import",
            json!({
                "lineage_json": initial.to_canonical_json().expect("initial JSON"),
            }),
        );
        assert_eq!(imported["ok"], true, "{imported:#}");
        let session_id = imported["session_id"].as_str().expect("session ID");
        let expected = imported["result"]["identity"].clone();
        let before = request(&mut engine, Some(session_id), "export", json!({}));

        let mut replacement = initial.clone();
        replacement
            .apply_patch(LineagePatch::new(
                replacement.identity(),
                vec![LineageMutation::Rewrite {
                    step: LineageStepId::from_raw(1),
                    replacement: Box::new(LineageStepRewrite {
                        label: "Same output, forged action identity".into(),
                        action: point_action("geosolve.geometry.v1.legacy-construction", 1),
                    }),
                }],
            ))
            .expect("valid sequential replacement");
        super::validate_document_schema(&replacement)
            .expect("replacement is independently valid in isolation");

        let rejected = request(
            &mut engine,
            Some(session_id),
            "reconcile",
            json!({
                "expected": expected,
                "lineage_json": replacement.to_canonical_json().expect("replacement JSON"),
            }),
        );
        assert_eq!(rejected["ok"], false, "{rejected:#}");
        assert_eq!(rejected["error"]["code"], "invalid_editor_authority");
        let after = request(&mut engine, Some(session_id), "export", json!({}));
        assert_eq!(
            after, before,
            "failed session-wide identity validation must retain the exact prior session"
        );
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "one RPC lifecycle regression keeps branch abandonment, hostile reconciliation, and byte-exact atomicity in the same externally visible session"
    )]
    fn reconcile_rejects_abandoned_identity_reuse_after_divergence_atomically() {
        let document_id = LineageDocumentId::from_raw(0x83_2850);
        let mut engine = LineageRpcEngine::new();
        let imported = request(
            &mut engine,
            None,
            "import",
            json!({
                "lineage_json": LineageDocument::with_id(document_id)
                    .to_canonical_json()
                    .expect("empty lineage JSON"),
            }),
        );
        assert_eq!(imported["ok"], true, "{imported:#}");
        let session_id = imported["session_id"].as_str().expect("session ID");
        let first = request(
            &mut engine,
            Some(session_id),
            "mutate",
            json!({
                "patch": LineagePatch::new(
                    serde_json::from_value(imported["result"]["identity"].clone())
                        .expect("initial identity"),
                    vec![LineageMutation::Insert {
                        before: None,
                        step: Box::new(numbered_point_step(
                            1,
                            "abandoned-first",
                            "sketch:point:1",
                            point_action("geosolve.geometry.v1.sketch-point", 1),
                        )),
                    }],
                ),
            }),
        );
        assert_eq!(first["ok"], true, "{first:#}");
        let undone = request(
            &mut engine,
            Some(session_id),
            "undo",
            json!({ "expected": first["result"]["identity"] }),
        );
        assert_eq!(undone["ok"], true, "{undone:#}");
        let divergent = request(
            &mut engine,
            Some(session_id),
            "mutate",
            json!({
                "patch": LineagePatch::new(
                    serde_json::from_value(undone["result"]["identity"].clone())
                        .expect("Undo identity"),
                    vec![LineageMutation::Insert {
                        before: None,
                        step: Box::new(numbered_point_step(
                            2,
                            "divergent-second",
                            "sketch:point:2",
                            point_action("geosolve.geometry.v1.sketch-point", 1),
                        )),
                    }],
                ),
            }),
        );
        assert_eq!(divergent["ok"], true, "{divergent:#}");
        let before = request(&mut engine, Some(session_id), "export", json!({}));

        let mut replacement = LineageDocument::with_id(document_id);
        replacement
            .apply_patch(LineagePatch::new(
                replacement.identity(),
                vec![
                    LineageMutation::Insert {
                        before: None,
                        step: Box::new(numbered_point_step(
                            1,
                            "reused-first-with-new-meaning",
                            "sketch:point:rebound",
                            point_action("geosolve.geometry.v1.sketch-point", 1),
                        )),
                    },
                    LineageMutation::Insert {
                        before: None,
                        step: Box::new(numbered_point_step(
                            2,
                            "divergent-second",
                            "sketch:point:2",
                            point_action("geosolve.geometry.v1.sketch-point", 1),
                        )),
                    },
                ],
            ))
            .expect("standalone replacement");
        let expected_revision = engine
            .session()
            .expect("live session")
            .identity()
            .revision
            .raw()
            + 1;
        while replacement.revision().raw() < expected_revision {
            let policy = if replacement.evaluation_policy()
                == LineageEvaluationPolicy::StrictChronological
            {
                LineageEvaluationPolicy::DependencyLocal
            } else {
                LineageEvaluationPolicy::StrictChronological
            };
            replacement
                .apply_patch(LineagePatch::new(
                    replacement.identity(),
                    vec![LineageMutation::SetEvaluationPolicy { policy }],
                ))
                .expect("advance replacement revision");
        }
        assert_eq!(replacement.revision().raw(), expected_revision);

        let rejected = request(
            &mut engine,
            Some(session_id),
            "reconcile",
            json!({
                "expected": divergent["result"]["identity"],
                "lineage_json": replacement.to_canonical_json().expect("replacement JSON"),
            }),
        );
        assert_eq!(rejected["ok"], false, "{rejected:#}");
        assert_eq!(rejected["error"]["code"], "invalid_editor_authority");
        let after = request(&mut engine, Some(session_id), "export", json!({}));
        assert_eq!(
            after, before,
            "rejected abandoned identity reuse must preserve the byte-exact RPC session"
        );
    }

    #[test]
    fn rewrite_owners_cannot_reassign_immutable_action_identity_across_history() {
        let document_id = LineageDocumentId::from_raw(0x83_2900);
        let mut engine = LineageRpcEngine::new();
        let initial = two_step_document(document_id);
        let imported = request(
            &mut engine,
            None,
            "import",
            json!({
                "lineage_json": initial.to_canonical_json().expect("initial JSON"),
            }),
        );
        assert_eq!(imported["ok"], true, "{imported:#}");
        let session_id = imported["session_id"].as_str().expect("session ID");
        let before = request(&mut engine, Some(session_id), "export", json!({}));
        let rejected = request(
            &mut engine,
            Some(session_id),
            "rewrite_owners",
            json!({
                "patch": LineagePatch::new(
                    serde_json::from_value(imported["result"]["identity"].clone())
                        .expect("imported identity"),
                    vec![LineageMutation::Rewrite {
                        step: LineageStepId::from_raw(1),
                        replacement: Box::new(LineageStepRewrite {
                            label: "Same output, forged action identity".into(),
                            action: point_action("geosolve.geometry.v1.legacy-construction", 1),
                        }),
                    }],
                ),
            }),
        );
        assert_eq!(rejected["ok"], false, "{rejected:#}");
        assert_eq!(rejected["error"]["code"], "invalid_editor_authority");
        let after = request(&mut engine, Some(session_id), "export", json!({}));
        assert_eq!(
            after, before,
            "failed rewrite must preserve identity, history, and allocator authority"
        );
    }

    #[test]
    fn forged_load_rejects_atomically_and_preserves_previous_session() {
        let document_id = LineageDocumentId::from_raw(0x83_3000);
        let mut engine = LineageRpcEngine::new();
        let created = request(
            &mut engine,
            None,
            "create",
            json!({ "document_id": document_id.to_string() }),
        );
        assert_eq!(created["ok"], true, "{created:#}");
        let session_id = created["session_id"].as_str().expect("session ID");
        let before = request(&mut engine, Some(session_id), "export", json!({}));
        assert_eq!(before["ok"], true, "{before:#}");

        let forged_json = forged_workbench_session(document_id)
            .to_canonical_session_json()
            .expect("canonical forged session");
        LineageSession::from_session_json(&forged_json)
            .expect("core session validation alone accepts the forged declaration");

        let rejected = request(
            &mut engine,
            None,
            "load",
            json!({ "session_json": forged_json }),
        );
        assert_eq!(rejected["ok"], false, "{rejected:#}");
        assert_eq!(rejected["error"]["code"], "invalid_editor_authority");

        let after = request(&mut engine, Some(session_id), "export", json!({}));
        assert_eq!(after, before, "failed load must preserve the prior session");
    }
}
