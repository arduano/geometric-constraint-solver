// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_constraint_editor::RetainedEditorCoordinator;
use geosolve_sketch::{
    DocumentId, DocumentSolveRequest, ExternalSnapshotSet, ParameterBatch, PersistentId,
    RetainedSketchDocumentSession, SketchDocument, SolverConfig,
};
use geosolve_sketch_lineage::{
    LineageActionDefinition, LineageDeveloperKey, LineageDigest, LineageDocument,
    LineageDocumentId, LineageEvaluationPolicy, LineageInputBinding, LineageMutation,
    LineageOpaqueId, LineageOutput, LineageOutputId, LineageOutputKind, LineageOutputRef,
    LineagePatch, LineageReservation, LineageReservationId, LineageReservationKind,
    LineageRevision, LineageSemanticKey, LineageSession, LineageStep, LineageStepId,
    LineageStepRewrite, VersionedActionPayload, lineage_content_digest,
};
use geosolve_sketch_lineage_wasm::{LINEAGE_RPC_PROTOCOL, LineageRpcEngine};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test;

const GOLDEN: &str = include_str!("fixtures/m83_rpc_transcript.golden.txt");

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct TestDocumentWire {
    version: u32,
    document_id: LineageDocumentId,
    revision: LineageRevision,
    next_step_id: LineageStepId,
    next_output_id: LineageOutputId,
    next_reservation_id: LineageReservationId,
    evaluation_policy: LineageEvaluationPolicy,
    steps: Vec<LineageStep>,
    digest: LineageDigest,
}

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
struct TestDocumentCanonicalPayload<'a> {
    version: u32,
    document_id: LineageDocumentId,
    revision: LineageRevision,
    next_step_id: LineageStepId,
    next_output_id: LineageOutputId,
    next_reservation_id: LineageReservationId,
    evaluation_policy: LineageEvaluationPolicy,
    steps: &'a [LineageStep],
}

impl TestDocumentWire {
    fn refresh_digest(&mut self) {
        let payload = TestDocumentCanonicalPayload {
            version: self.version,
            document_id: self.document_id,
            revision: self.revision,
            next_step_id: self.next_step_id,
            next_output_id: self.next_output_id,
            next_reservation_id: self.next_reservation_id,
            evaluation_policy: self.evaluation_policy,
            steps: &self.steps,
        };
        self.digest = lineage_content_digest(
            &serde_json::to_vec(&payload).expect("canonical test document payload"),
        );
    }
}

fn boundary_document_json(document_id: LineageDocumentId, revision: u64, next_id: u64) -> String {
    let mut wire: TestDocumentWire = serde_json::from_str(
        &LineageDocument::with_id(document_id)
            .to_canonical_json()
            .expect("empty lineage JSON"),
    )
    .expect("empty lineage wire");
    wire.revision = LineageRevision::from_raw(revision);
    wire.next_step_id = LineageStepId::from_raw(next_id);
    wire.next_output_id = LineageOutputId::from_raw(next_id);
    wire.next_reservation_id = LineageReservationId::from_raw(next_id);
    wire.refresh_digest();
    serde_json::to_string(&wire).expect("boundary lineage JSON")
}

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

fn point_action(position: [i32; 2]) -> LineageActionDefinition {
    LineageActionDefinition::GeometryRecipe {
        action: VersionedActionPayload {
            schema: LineageSemanticKey::new("geosolve.geometry.v1.sketch-point")
                .expect("point schema"),
            version: 1,
            inputs: Vec::new(),
            parameters: [("position".into(), json!(position))].into_iter().collect(),
        },
    }
}

fn point_step() -> LineageStep {
    LineageStep::new(
        LineageStepId::from_raw(2),
        LineageDeveloperKey::new("point").expect("developer key"),
        "Point",
        point_action([1, 2]),
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

fn boundary_point_step() -> LineageStep {
    LineageStep::new(
        LineageStepId::from_raw(u64::MAX),
        LineageDeveloperKey::new("boundary-point").expect("developer key"),
        "Boundary point",
        point_action([1, 2]),
        vec![LineageOutput {
            id: LineageOutputId::from_raw(u64::MAX),
            key: LineageSemanticKey::new("point").expect("output key"),
            kind: LineageOutputKind::Point,
            reservation: Some(LineageReservationId::from_raw(u64::MAX)),
        }],
        vec![LineageReservation {
            id: LineageReservationId::from_raw(u64::MAX),
            key: LineageSemanticKey::new("point").expect("reservation key"),
            kind: LineageReservationKind::Point,
            persistent_id: LineageOpaqueId::new("sketch:point:boundary")
                .expect("persistent point ID"),
        }],
    )
}

fn dependent_step(document: LineageDocumentId) -> LineageStep {
    LineageStep::new(
        LineageStepId::from_raw(3),
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
                        document,
                        step: LineageStepId::from_raw(2),
                        output: LineageOutputId::from_raw(2),
                        kind: LineageOutputKind::Point,
                    },
                }],
                parameters: std::collections::BTreeMap::new(),
            },
        },
        vec![LineageOutput {
            id: LineageOutputId::from_raw(3),
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

#[allow(
    clippy::needless_pass_by_value,
    reason = "the transcript helper takes ownership of each one-shot JSON payload to mirror an RPC request envelope"
)]
fn request(
    engine: &mut LineageRpcEngine,
    transcript: &mut Vec<String>,
    request_id: &str,
    session_id: Option<&str>,
    method: &str,
    params: Value,
) -> Value {
    let response = engine.request(
        &json!({
            "protocol": LINEAGE_RPC_PROTOCOL,
            "request_id": request_id,
            "session_id": session_id,
            "method": method,
            "params": params,
        })
        .to_string(),
    );
    let value: Value = serde_json::from_str(&response).expect("canonical RPC response");
    assert_eq!(
        serde_json::to_string(&value).expect("response re-encode"),
        response
    );
    assert_eq!(value["protocol"], LINEAGE_RPC_PROTOCOL);
    assert_eq!(value["request_id"], request_id);
    assert_eq!(value["method"], method);
    transcript.push(response);
    value
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

#[allow(
    clippy::too_many_lines,
    reason = "one contiguous transcript proves correlated state, retained failure, history, import, and full-session restoration across native and WASM"
)]
fn rpc_transcript() -> String {
    let document_id = LineageDocumentId::from_raw(0x8300_0000_0000_0001);
    let mut engine = LineageRpcEngine::new();
    let mut transcript = Vec::new();

    let uninitialized = request(
        &mut engine,
        &mut transcript,
        "01",
        None,
        "inspect",
        json!({}),
    );
    assert_eq!(uninitialized["error"]["code"], "not_initialized");

    let created = request(
        &mut engine,
        &mut transcript,
        "02",
        None,
        "create",
        json!({ "document_id": document_id.to_string() }),
    );
    assert_eq!(created["ok"], true);
    let created_session_id = created["session_id"].as_str().expect("created session ID");
    let created_evaluation = request(
        &mut engine,
        &mut transcript,
        "02a",
        Some(created_session_id),
        "evaluate",
        json!({
            "expected": created["result"]["identity"],
            "host_inputs": host_inputs(),
        }),
    );
    assert_eq!(created_evaluation["ok"], true);
    assert_eq!(created_evaluation["result"]["disposition"], "accepted");
    assert_eq!(
        created_evaluation["result"]["evidence"]["validated_prefix_count"],
        1
    );
    let seeded = accepted_workbench_session(document_id);
    let loaded_seed = request(
        &mut engine,
        &mut transcript,
        "03",
        None,
        "load",
        json!({
            "session_json": seeded.to_canonical_session_json().expect("seed session JSON"),
        }),
    );
    let session_id = loaded_seed["session_id"]
        .as_str()
        .expect("loaded session ID")
        .to_owned();
    let mut identity =
        serde_json::from_value(loaded_seed["result"]["identity"].clone()).expect("seed identity");

    let accepted = request(
        &mut engine,
        &mut transcript,
        "04",
        Some(&session_id),
        "evaluate",
        json!({
            "expected": identity,
            "host_inputs": host_inputs(),
        }),
    );
    assert_eq!(accepted["result"]["disposition"], "accepted");
    assert_eq!(
        accepted["result"]["evidence"]["evaluator"],
        "geosolve.constraint-editor.lineage-cold.v1"
    );

    let inserted = request(
        &mut engine,
        &mut transcript,
        "05",
        Some(&session_id),
        "mutate",
        json!({
            "patch": LineagePatch::new(
                identity,
                vec![
                    LineageMutation::Insert { before: None, step: Box::new(point_step()) },
                    LineageMutation::Insert {
                        before: None,
                        step: Box::new(dependent_step(document_id)),
                    },
                ],
            ),
        }),
    );
    assert_eq!(
        inserted["result"]["inserted_steps"],
        json!(["0000000000000002", "0000000000000003"])
    );
    identity =
        serde_json::from_value(inserted["result"]["identity"].clone()).expect("inserted identity");

    let wrong_port = request(
        &mut engine,
        &mut transcript,
        "05a",
        Some(&session_id),
        "mutate",
        json!({
            "patch": LineagePatch::new(
                identity,
                vec![LineageMutation::Rebind {
                    step: LineageStepId::from_raw(3),
                    input: LineageSemanticKey::new("point").expect("input key"),
                    target: LineageOutputRef {
                        document: document_id,
                        step: LineageStepId::from_raw(2),
                        output: LineageOutputId::from_raw(2),
                        kind: LineageOutputKind::Scalar,
                    },
                }],
            ),
        }),
    );
    assert_eq!(wrong_port["error"]["code"], "wrong_port_kind");

    let invalid_transition = request(
        &mut engine,
        &mut transcript,
        "05b",
        Some(&session_id),
        "mutate",
        json!({
            "patch": LineagePatch::new(
                identity,
                vec![LineageMutation::Tombstone {
                    step: LineageStepId::from_raw(2),
                }],
            ),
        }),
    );
    assert_eq!(
        invalid_transition["error"]["code"],
        "invalid_state_transition"
    );

    let rejected_materialization = request(
        &mut engine,
        &mut transcript,
        "06",
        Some(&session_id),
        "evaluate",
        json!({
            "expected": identity,
            "host_inputs": host_inputs(),
        }),
    );
    assert_eq!(rejected_materialization["result"]["disposition"], "failed");
    assert_eq!(
        rejected_materialization["result"]["evidence"]["code"],
        "workbench_materialization_unsupported"
    );

    let caller_certification = request(
        &mut engine,
        &mut transcript,
        "07",
        Some(&session_id),
        "accept",
        json!({}),
    );
    assert_eq!(caller_certification["error"]["code"], "unknown_method");

    let unknown_schema = request(
        &mut engine,
        &mut transcript,
        "08",
        Some(&session_id),
        "rewrite_owners",
        json!({
            "patch": LineagePatch::new(
                identity,
                vec![LineageMutation::Rewrite {
                    step: LineageStepId::from_raw(2),
                    replacement: Box::new(LineageStepRewrite {
                        label: "Unknown point".into(),
                        action: LineageActionDefinition::GeometryRecipe {
                            action: VersionedActionPayload::empty(
                                LineageSemanticKey::new("geosolve.geometry.v1.future-point")
                                    .expect("unknown schema key"),
                                1,
                            ),
                        },
                    }),
                }],
            ),
        }),
    );
    assert_eq!(unknown_schema["error"]["code"], "unsupported_action_schema");

    let rewritten = request(
        &mut engine,
        &mut transcript,
        "09",
        Some(&session_id),
        "rewrite_owners",
        json!({
            "patch": LineagePatch::new(
                identity,
                vec![LineageMutation::Rewrite {
                    step: LineageStepId::from_raw(2),
                    replacement: Box::new(LineageStepRewrite {
                        label: "Moved point".into(),
                        action: point_action([3, 4]),
                    }),
                }],
            ),
        }),
    );
    identity = serde_json::from_value(rewritten["result"]["identity"].clone())
        .expect("rewritten identity");

    let rewrite_rejected = request(
        &mut engine,
        &mut transcript,
        "10",
        Some(&session_id),
        "evaluate",
        json!({
            "expected": identity,
            "host_inputs": host_inputs(),
        }),
    );
    assert_eq!(rewrite_rejected["result"]["disposition"], "failed");

    let suppressed = request(
        &mut engine,
        &mut transcript,
        "11",
        Some(&session_id),
        "mutate",
        json!({
            "patch": LineagePatch::new(
                identity,
                vec![LineageMutation::SetSuppressed {
                    step: LineageStepId::from_raw(2),
                    suppressed: true,
                }],
            ),
        }),
    );
    identity = serde_json::from_value(suppressed["result"]["identity"].clone())
        .expect("suppressed identity");

    let rejected = request(
        &mut engine,
        &mut transcript,
        "12",
        Some(&session_id),
        "evaluate",
        json!({
            "expected": identity,
            "host_inputs": host_inputs(),
        }),
    );
    assert_eq!(rejected["result"]["disposition"], "failed");
    assert_eq!(rejected["result"]["attempt"]["disposition"], "failed");
    assert_eq!(
        rejected["result"]["attempt"]["failed_steps"],
        json!(["0000000000000003"])
    );
    assert_eq!(
        rejected["result"]["last_accepted"]["lineage"]["revision"],
        "0000000000000001"
    );

    let inspected = request(
        &mut engine,
        &mut transcript,
        "13",
        Some(&session_id),
        "inspect",
        json!({}),
    );
    assert_eq!(
        inspected["result"]["latest_attempt"]["disposition"],
        "failed"
    );
    assert_eq!(inspected["result"]["can_undo"], true);

    let undone = request(
        &mut engine,
        &mut transcript,
        "14",
        Some(&session_id),
        "undo",
        json!({ "expected": identity }),
    );
    identity =
        serde_json::from_value(undone["result"]["identity"].clone()).expect("undone identity");
    let redone = request(
        &mut engine,
        &mut transcript,
        "15",
        Some(&session_id),
        "redo",
        json!({ "expected": identity }),
    );
    identity =
        serde_json::from_value(redone["result"]["identity"].clone()).expect("redone identity");

    let caller_certified_attempt = request(
        &mut engine,
        &mut transcript,
        "16",
        Some(&session_id),
        "record_nonpublishing",
        json!({
            "target": identity,
            "host_inputs": host_inputs(),
            "disposition": "cancelled",
            "diagnostic": "evaluation.cancelled",
        }),
    );
    assert_eq!(caller_certified_attempt["error"]["code"], "unknown_method");

    let lineage_export = request(
        &mut engine,
        &mut transcript,
        "17",
        Some(&session_id),
        "export_lineage",
        json!({}),
    );
    let lineage_json = lineage_export["result"]["lineage_json"]
        .as_str()
        .expect("lineage export");
    let mut imported_engine = LineageRpcEngine::new();
    let imported = request(
        &mut imported_engine,
        &mut transcript,
        "18",
        None,
        "import",
        json!({ "lineage_json": lineage_json }),
    );
    assert_eq!(imported["result"]["identity"], json!(identity));
    assert_eq!(imported["result"]["can_undo"], false);

    let exported = request(
        &mut engine,
        &mut transcript,
        "19",
        Some(&session_id),
        "export",
        json!({}),
    );
    let session_json = exported["result"]["session_json"]
        .as_str()
        .expect("session export");
    let mut loaded_engine = LineageRpcEngine::new();
    let loaded = request(
        &mut loaded_engine,
        &mut transcript,
        "20",
        None,
        "load",
        json!({ "session_json": session_json }),
    );
    assert_eq!(loaded["result"]["latest_attempt"]["disposition"], "pending");
    assert!(loaded["result"]["last_accepted"].is_null());
    assert_eq!(loaded["result"]["can_undo"], true);

    let wrong_session = request(
        &mut loaded_engine,
        &mut transcript,
        "21",
        Some("lineage:00000000000000000000000000000001"),
        "inspect",
        json!({}),
    );
    assert_eq!(wrong_session["error"]["code"], "wrong_session");

    let mut id_boundary_engine = LineageRpcEngine::new();
    let id_boundary = request(
        &mut id_boundary_engine,
        &mut transcript,
        "22",
        None,
        "import",
        json!({
            "lineage_json": boundary_document_json(
                LineageDocumentId::from_raw(0x8300_0000_0000_0022),
                0,
                u64::MAX,
            ),
        }),
    );
    let id_boundary_session = id_boundary["session_id"]
        .as_str()
        .expect("ID-boundary session ID");
    let id_exhausted = request(
        &mut id_boundary_engine,
        &mut transcript,
        "23",
        Some(id_boundary_session),
        "mutate",
        json!({
            "patch": LineagePatch::new(
                serde_json::from_value(id_boundary["result"]["identity"].clone())
                    .expect("ID-boundary identity"),
                vec![LineageMutation::Insert {
                    before: None,
                    step: Box::new(boundary_point_step()),
                }],
            ),
        }),
    );
    assert_eq!(id_exhausted["error"]["code"], "id_exhausted");

    let mut revision_boundary_engine = LineageRpcEngine::new();
    let revision_boundary = request(
        &mut revision_boundary_engine,
        &mut transcript,
        "24",
        None,
        "import",
        json!({
            "lineage_json": boundary_document_json(
                LineageDocumentId::from_raw(0x8300_0000_0000_0024),
                u64::MAX,
                1,
            ),
        }),
    );
    let revision_boundary_session = revision_boundary["session_id"]
        .as_str()
        .expect("revision-boundary session ID");
    let revision_exhausted = request(
        &mut revision_boundary_engine,
        &mut transcript,
        "25",
        Some(revision_boundary_session),
        "set_policy",
        json!({
            "expected": revision_boundary["result"]["identity"],
            "policy": "dependency_local",
        }),
    );
    assert_eq!(revision_exhausted["error"]["code"], "revision_exhausted");

    transcript.join("\n") + "\n"
}

#[test]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
fn native_and_wasm_stateful_rpc_transcript_matches_exact_golden() {
    let transcript = rpc_transcript();
    let summary = format!(
        "protocol\t{LINEAGE_RPC_PROTOCOL}\nresponses\t{}\nbytes\t{}\nfnv1a64\t{:016x}\n",
        transcript.lines().count(),
        transcript.len(),
        fnv1a64(transcript.as_bytes()),
    );
    #[cfg(not(target_arch = "wasm32"))]
    if std::env::var_os("GEOSOLVE_SURVEY_M83_RPC_TRANSCRIPT").is_some() {
        print!("{summary}");
        return;
    }
    assert_eq!(summary, GOLDEN);
}
