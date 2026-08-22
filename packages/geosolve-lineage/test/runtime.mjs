// SPDX-License-Identifier: GPL-3.0-or-later

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

import {
  LineagePatchBuilder,
  LineageRpcClient,
  LineageRpcRequestError,
  LineageRpcResponseError,
  RPC_MAX_REQUEST_BYTES,
  RPC_MAX_RESULT_ITEMS,
  actionKindKeys,
  annotationKeys,
  bindingKeys,
  constraintKeys,
  curveControlKeys,
  curvePropertyKeys,
  dimensionKeys,
  computedFeatureKeys,
  explicitBranchFamilyKeys,
  evaluationFailureCodes,
  externalKeys,
  geometryRecipeKeys,
  geometryRoleKeys,
  nativePublicationKeys,
  operationKeys,
  outputKindKeys,
  parameterKeys,
  reservationKindKeys,
  trimKeys,
  developerKey,
  geometry,
  persistentId,
  rpcErrorCodes,
  semanticKey,
} from "../src/index.ts";

assert.equal(geometryRecipeKeys.length, 25);
assert.equal(constraintKeys.length, 35);
assert.equal(dimensionKeys.length, 8);
assert.equal(operationKeys.length, 12);
assert.equal(curveControlKeys.length, 14);
assert.equal(curvePropertyKeys.length, 7);
assert.equal(geometryRoleKeys.length, 2);
assert.equal(explicitBranchFamilyKeys.length, 27);
assert.deepEqual(trimKeys, ["curve-trim-view"]);
assert.deepEqual(parameterKeys, ["host-parameter"]);
assert.deepEqual(bindingKeys, [
  "parameter-binding",
  "parameter-output",
  "host-activation",
  "geometry-role",
]);
assert.deepEqual(externalKeys, ["external-binding", "external-snapshot"]);
assert.deepEqual(annotationKeys, ["annotation-layout"]);

const catalogRows = [["category", "key"]];
const appendRows = (category, keys) => {
  for (const key of keys) catalogRows.push([category, key]);
};
appendRows("action-kind", actionKindKeys);
appendRows("output-kind", outputKindKeys);
appendRows("reservation-kind", reservationKindKeys);
appendRows("geometry", geometryRecipeKeys);
appendRows("constraint", constraintKeys);
appendRows("dimension", dimensionKeys);
appendRows("curve-control", curveControlKeys);
appendRows("curve-property", curvePropertyKeys);
appendRows("geometry-role", geometryRoleKeys);
appendRows("operation", operationKeys);
appendRows("native-publication", nativePublicationKeys);
appendRows("computed-feature", computedFeatureKeys);
appendRows("trim", trimKeys);
appendRows("parameter", parameterKeys);
appendRows("binding", bindingKeys);
appendRows("external", externalKeys);
appendRows("annotation", annotationKeys);
appendRows("branch", explicitBranchFamilyKeys);
const catalog = `${catalogRows.map((row) => row.join("\t")).join("\n")}\n`;
const golden = readFileSync(
  new URL(
    "../../../crates/geosolve-constraint-editor/tests/fixtures/m83_lineage_action_catalog.golden.tsv",
    import.meta.url,
  ),
  "utf8",
);
assert.equal(catalog, golden);

const productionSource = (relativePath) =>
  readFileSync(new URL(relativePath, import.meta.url), "utf8").split("\n#[cfg(test)]", 1)[0];
const rpcSource = productionSource(
  "../../../crates/geosolve-sketch-lineage-wasm/src/lib.rs",
);
const semanticSource = productionSource(
  "../../../crates/geosolve-sketch-lineage-wasm/src/semantic.rs",
);
const evaluationSource = productionSource(
  "../../../crates/geosolve-constraint-editor/src/coordinator/lineage_evaluation.rs",
);
const captureCodes = (source, expression) =>
  [...source.matchAll(expression)].map((match) => match[1]);
const lineageErrorMapping = rpcSource.slice(
  rpcSource.indexOf("fn lineage(error:"),
  rpcSource.indexOf("\nfn encode_response("),
);
const domainEvaluationCodes = captureCodes(
  evaluationSource,
  /LineageDomainEvaluationFailure::new\(\s*"([a-z][a-z0-9_]*)"/gu,
);
const rustRpcErrorCodes = new Set([
  ...captureCodes(rpcSource, /RpcFailure::new\(\s*"([a-z][a-z0-9_]*)"/gu),
  ...captureCodes(lineageErrorMapping, /=>\s*"([a-z][a-z0-9_]*)"/gu),
  ...captureCodes(semanticSource, /code:\s*"([a-z][a-z0-9_]*)"/gu),
  ...domainEvaluationCodes,
]);
assert.deepEqual(
  [...rpcErrorCodes].sort(),
  [...rustRpcErrorCodes].sort(),
  "the TypeScript RPC error union must cover every Rust envelope error source exactly",
);
assert.deepEqual(
  [...evaluationFailureCodes].sort(),
  [...new Set(["dependency_blocked", ...domainEvaluationCodes])].sort(),
  "the TypeScript evaluation failure union must cover every Rust result source exactly",
);

const identity = {
  document: "00000000000000000000000000000083",
  revision: "0000000000000000",
  digest: "00".repeat(32),
};
const hostInputs = {
  version: 1,
  parameter_batch_json:
    '{"version":1,"revision":0,"digest":[178,48,67,32,232,86,240,224,17,22,252,246,136,181,160,168,150,4,247,242,2,49,6,164,189,140,2,203,116,23,238,239],"entries":[]}',
  external_snapshot_set_json:
    '{"version":1,"revision":0,"digest":[242,68,150,135,86,171,49,151,133,16,230,224,165,214,84,191,34,4,18,89,26,83,128,95,16,92,28,50,48,252,92,180],"entries":[]}',
};
const highWater = {
  next_step_id: "0000000000000001",
  next_output_id: "0000000000000001",
  next_reservation_id: "0000000000000001",
};
const builder = new LineagePatchBuilder(identity, highWater);
builder.insert({
  key: developerKey("point"),
  label: "Point",
  action: {
    kind: "geometry_recipe",
    action: geometry["sketch-point"]({ parameters: { position: [1, 2] } }),
  },
  outputs: [
    {
      key: semanticKey("point"),
      kind: "point",
      reservation: "0000000000000001",
      writable: [semanticKey("position.x"), semanticKey("position.y")],
    },
  ],
  reservations: [
    {
      key: semanticKey("point"),
      kind: "point",
      persistent_id: persistentId("sketch:point:1"),
    },
  ],
});

const patch = builder.patch();
assert.deepEqual(patch.mutations[0].step.output_identities, [
  {
    output: "0000000000000001",
    flow: { kind: "created", reservation: "0000000000000001" },
  },
]);
assert.deepEqual(patch.mutations[0].step.writable_leaves, [
  { output: "0000000000000001", key: "position.x" },
  { output: "0000000000000001", key: "position.y" },
]);

const sessionId = "lineage:00000000000000000000000000000083";
const validAttempt = (
  disposition,
  materializationDigest,
  diagnostic = null,
  failedSteps = [],
) => ({
  target: identity,
  policy: "strict_chronological",
  disposition,
  external_inputs: disposition === "pending" ? null : "host-inputs:empty",
  materialization_digest: materializationDigest,
  failed_steps: failedSteps,
  diagnostic,
});
const validAuthority = {
  lineage: identity,
  external_inputs: "host-inputs:empty",
  materialization_digest: identity.digest,
};
const validDocument = {
  version: 1,
  document_id: identity.document,
  revision: identity.revision,
  next_step_id: "0000000000000001",
  next_output_id: "0000000000000001",
  next_reservation_id: "0000000000000001",
  evaluation_policy: "strict_chronological",
  steps: [],
  digest: identity.digest,
};
const validDocumentJson = JSON.stringify(validDocument);
const validSessionJson = JSON.stringify({
  version: 1,
  document: validDocumentJson,
  latest_attempt: null,
  last_accepted_document: null,
  last_accepted: null,
  undo: [],
  redo: [],
  lifecycle: {
    revision: identity.revision,
    allocator: {
      next_step_id: "0000000000000001",
      next_output_id: "0000000000000001",
      next_reservation_id: "0000000000000001",
    },
  },
  auxiliary_high_waters: {},
  digest: identity.digest,
});
const validSnapshot = {
  session_id: sessionId,
  identity,
  evaluation_policy: "strict_chronological",
  can_undo: false,
  can_redo: false,
  undo_length: "0",
  redo_length: "0",
  lifecycle: {
    revision: identity.revision,
    next_step_id: "0000000000000001",
    next_output_id: "0000000000000001",
    next_reservation_id: "0000000000000001",
  },
  latest_attempt: null,
  last_accepted: null,
  last_accepted_document: null,
  document: validDocument,
};
const validLoadedSnapshot = {
  ...validSnapshot,
  latest_attempt: validAttempt("pending", null),
};
const validMutation = {
  identity,
  changed: true,
  inserted_steps: [],
  tombstoned_steps: [],
};
const validAcceptedEvaluation = {
  identity,
  disposition: "accepted",
  evidence: {
    evaluator: "geosolve.constraint-editor.lineage-cold.v1",
    external_inputs: "host-inputs:empty",
    materialization_digest: identity.digest,
    sketch_digest: identity.digest,
    feature_digest: identity.digest,
    computed_feature_count: 0,
    validated_prefix_count: 1,
    strict_prefix_digest: identity.digest,
  },
  steps: [
    {
      step: "0000000000000001",
      state: "ready",
      dependencies: [],
    },
  ],
  attempt: validAttempt("accepted", identity.digest),
  last_accepted: validAuthority,
};
const validFailedEvaluation = {
  identity,
  disposition: "failed",
  evidence: {
    evaluator: "geosolve.constraint-editor.lineage-cold.v1",
    code: "dependency_blocked",
    message: "a dependency is blocked",
    failed_features: [],
  },
  steps: [
    {
      step: "0000000000000001",
      state: "suppressed",
      dependencies: [],
    },
    {
      step: "0000000000000002",
      state: "blocked",
      dependencies: ["0000000000000001"],
    },
  ],
  attempt: validAttempt(
    "failed",
    null,
    "semantic-evaluation-blocked",
    ["0000000000000002"],
  ),
  last_accepted: null,
};
const validDomainFailedEvaluation = {
  identity,
  disposition: "failed",
  evidence: {
    evaluator: "geosolve.constraint-editor.lineage-cold.v1",
    code: "sketch_evaluation_rejected",
    message: "the owning sketch rejected the ready step",
    failed_step: "0000000000000001",
    failed_features: [],
  },
  steps: [
    {
      step: "0000000000000001",
      state: "ready",
      dependencies: [],
    },
  ],
  attempt: validAttempt(
    "failed",
    null,
    "semantic-evaluation-failed",
    ["0000000000000001"],
  ),
  last_accepted: null,
};

const validResult = (method) => {
  switch (method) {
    case "create":
    case "import":
    case "inspect":
      return validSnapshot;
    case "load":
      return validLoadedSnapshot;
    case "mutate":
    case "rewrite_owners":
    case "set_policy":
      return validMutation;
    case "reconcile":
      return { identity };
    case "undo":
    case "redo":
      return validSnapshot;
    case "evaluate":
      return validAcceptedEvaluation;
    case "export":
      return { identity, session_json: validSessionJson };
    case "export_lineage":
      return { identity, lineage_json: validDocumentJson };
    default:
      throw new Error(`missing runtime result fixture for ${method}`);
  }
};

const validResponse = (request, result = validResult(request.method), responseSession = sessionId) => ({
  protocol: "geosolve.lineage.rpc.v0",
  request_id: request.request_id,
  method: request.method,
  session_id: responseSession,
  ok: true,
  result,
});

const requests = [];
const client = new LineageRpcClient({
  request(request) {
    const envelope = JSON.parse(request);
    requests.push(envelope);
    return JSON.stringify(validResponse(envelope));
  },
});
client.call("create", {});
client.call("inspect", {});
client.call("evaluate", { expected: identity, host_inputs: hostInputs });
client.call("export", {});
client.call("export_lineage", {});
client.call("load", { session_json: validSessionJson });
client.call("import", { lineage_json: validDocumentJson });
assert.equal(requests[0].session_id, null);
assert.equal(requests[1].session_id, sessionId);
assert.equal(requests[2].method, "evaluate");
assert.equal(requests[2].session_id, sessionId);
assert.equal(requests[5].session_id, null);
assert.equal(requests[6].session_id, null);

const nextIdentity = {
  ...identity,
  revision: "0000000000000001",
  digest: "22".repeat(32),
};
const secondNextIdentity = {
  ...identity,
  revision: "0000000000000002",
  digest: "33".repeat(32),
};
const pendingSnapshot = (
  next,
  policy = "strict_chronological",
  lastAccepted = null,
) => ({
  ...validSnapshot,
  identity: next,
  evaluation_policy: policy,
  can_redo: true,
  redo_length: "1",
  lifecycle: { ...validSnapshot.lifecycle, revision: next.revision },
  latest_attempt: {
    target: next,
    policy,
    disposition: "pending",
    external_inputs: null,
    materialization_digest: null,
    failed_steps: [],
    diagnostic: null,
  },
  last_accepted: lastAccepted,
  last_accepted_document: lastAccepted === null ? null : validDocument,
  document: {
    ...validDocument,
    revision: next.revision,
    digest: next.digest,
    evaluation_policy: policy,
  },
});

const dependencyLocalDocument = {
  ...validDocument,
  evaluation_policy: "dependency_local",
};
const historySessionJson = JSON.stringify({
  ...JSON.parse(validSessionJson),
  undo: [
    {
      document: JSON.stringify(dependencyLocalDocument),
      latest_attempt: null,
      last_accepted_document: validDocumentJson,
      last_accepted: validAuthority,
    },
  ],
});
const loadedHistorySnapshot = {
  ...validLoadedSnapshot,
  can_undo: true,
  undo_length: "1",
};
const nextDocumentJson = JSON.stringify({
  ...validDocument,
  revision: nextIdentity.revision,
  digest: nextIdentity.digest,
});
const callOneValidMethod = (method, params, result) => {
  let call = 0;
  const methodClient = new LineageRpcClient({
    request(request) {
      const envelope = JSON.parse(request);
      call += 1;
      return JSON.stringify(validResponse(envelope, call === 1 ? validSnapshot : result));
    },
  });
  methodClient.call("create", {});
  return methodClient.call(method, params);
};
const insertedStep = patch.mutations[0].step;
callOneValidMethod("mutate", { patch }, {
  identity: nextIdentity,
  changed: true,
  inserted_steps: [insertedStep.id],
  tombstoned_steps: [],
});
const rewritePatch = {
  expected: identity,
  mutations: [
    {
      kind: "rewrite",
      step: insertedStep.id,
      replacement: { label: insertedStep.label, action: insertedStep.action },
    },
  ],
};
callOneValidMethod("rewrite_owners", { patch: rewritePatch }, {
  identity,
  changed: false,
  inserted_steps: [],
  tombstoned_steps: [],
});
callOneValidMethod(
  "set_policy",
  { expected: identity, policy: "strict_chronological" },
  { identity, changed: false, inserted_steps: [], tombstoned_steps: [] },
);
callOneValidMethod(
  "set_policy",
  { expected: identity, policy: "dependency_local" },
  { identity: nextIdentity, changed: true, inserted_steps: [], tombstoned_steps: [] },
);
callOneValidMethod(
  "reconcile",
  { expected: identity, lineage_json: nextDocumentJson },
  { identity: nextIdentity },
);
let historyPolicyCall = 0;
const historyPolicyClient = new LineageRpcClient({
  request(request) {
    const envelope = JSON.parse(request);
    historyPolicyCall += 1;
    const result = historyPolicyCall === 1
      ? validSnapshot
      : historyPolicyCall === 2
        ? {
            identity: nextIdentity,
            changed: true,
            inserted_steps: [],
            tombstoned_steps: [],
          }
        : historyPolicyCall === 3
          ? pendingSnapshot(secondNextIdentity, "dependency_local")
          : historyPolicyCall === 4
            ? pendingSnapshot(secondNextIdentity, "strict_chronological")
            : historyPolicyCall === 5
              ? {
                  identity: secondNextIdentity,
                  changed: false,
                  inserted_steps: [],
                  tombstoned_steps: [],
                }
              : historyPolicyCall === 6
                ? {
                    ...validFailedEvaluation,
                    identity: secondNextIdentity,
                    attempt: {
                      ...validFailedEvaluation.attempt,
                      target: secondNextIdentity,
                      policy: "dependency_local",
                    },
                  }
                : {
                    ...validFailedEvaluation,
                    identity: secondNextIdentity,
                    attempt: {
                      ...validFailedEvaluation.attempt,
                      target: secondNextIdentity,
                    },
                    last_accepted: validAuthority,
                  };
    return JSON.stringify(validResponse(envelope, result));
  },
});
historyPolicyClient.call("create", {});
historyPolicyClient.call("set_policy", {
  expected: identity,
  policy: "dependency_local",
});
assert.throws(
  () => historyPolicyClient.call("undo", { expected: nextIdentity }),
  (error) => {
    assert.ok(error instanceof LineageRpcResponseError);
    assert.equal(error.violation, "invalid_result");
    return true;
  },
  "Undo cannot invent a historical policy that disagrees with the retained checkpoint",
);
historyPolicyClient.call("undo", { expected: nextIdentity });
assert.throws(
  () => historyPolicyClient.call("set_policy", {
    expected: secondNextIdentity,
    policy: "dependency_local",
  }),
  (error) => {
    assert.ok(error instanceof LineageRpcResponseError);
    assert.equal(error.violation, "invalid_result");
    return true;
  },
  "Undo must retain the restored policy for later set_policy validation",
);
assert.throws(
  () => historyPolicyClient.call("evaluate", {
    expected: secondNextIdentity,
    host_inputs: hostInputs,
  }),
  (error) => {
    assert.ok(error instanceof LineageRpcResponseError);
    assert.equal(error.violation, "invalid_result");
    return true;
  },
  "evaluation after Undo must use the restored policy",
);
assert.throws(
  () => historyPolicyClient.call("evaluate", {
    expected: secondNextIdentity,
    host_inputs: hostInputs,
  }),
  (error) => {
    assert.ok(error instanceof LineageRpcResponseError);
    assert.equal(error.violation, "invalid_result");
    return true;
  },
  "failed evaluation after Undo cannot invent prior accepted authority from null",
);

const failedAtSecondIdentity = (lastAccepted) => ({
  ...validFailedEvaluation,
  identity: secondNextIdentity,
  attempt: {
    ...validFailedEvaluation.attempt,
    target: secondNextIdentity,
  },
  last_accepted: lastAccepted,
});
let acceptedHistoryCall = 0;
const acceptedHistoryClient = new LineageRpcClient({
  request(request) {
    const envelope = JSON.parse(request);
    acceptedHistoryCall += 1;
    const result = acceptedHistoryCall === 1
      ? validSnapshot
      : acceptedHistoryCall === 2
        ? validAcceptedEvaluation
        : acceptedHistoryCall === 3
          ? {
              identity: nextIdentity,
              changed: true,
              inserted_steps: [insertedStep.id],
              tombstoned_steps: [],
            }
          : acceptedHistoryCall === 4
            ? pendingSnapshot(secondNextIdentity, "strict_chronological", validAuthority)
            : acceptedHistoryCall === 5
              ? failedAtSecondIdentity(validAuthority)
              : failedAtSecondIdentity(null);
    return JSON.stringify(validResponse(envelope, result));
  },
});
acceptedHistoryClient.call("create", {});
acceptedHistoryClient.call("evaluate", { expected: identity, host_inputs: hostInputs });
acceptedHistoryClient.call("mutate", { patch });
acceptedHistoryClient.call("undo", { expected: nextIdentity });
assert.equal(
  acceptedHistoryClient.call("evaluate", {
    expected: secondNextIdentity,
    host_inputs: hostInputs,
  }).result.last_accepted.materialization_digest,
  validAuthority.materialization_digest,
);
assert.throws(
  () => acceptedHistoryClient.call("evaluate", {
    expected: secondNextIdentity,
    host_inputs: hostInputs,
  }),
  (error) => {
    assert.ok(error instanceof LineageRpcResponseError);
    assert.equal(error.violation, "invalid_result");
    return true;
  },
  "failed evaluation after Undo must preserve the exact restored accepted authority",
);

let loadedHistoryCall = 0;
const loadedHistoryClient = new LineageRpcClient({
  request(request) {
    const envelope = JSON.parse(request);
    loadedHistoryCall += 1;
    const result = loadedHistoryCall === 1
      ? loadedHistorySnapshot
      : loadedHistoryCall === 2
        ? pendingSnapshot(nextIdentity, "dependency_local")
        : loadedHistoryCall === 3
          ? pendingSnapshot(secondNextIdentity, "strict_chronological", validAuthority)
          : pendingSnapshot(secondNextIdentity, "dependency_local");
    return JSON.stringify(validResponse(envelope, result));
  },
});
loadedHistoryClient.call("load", { session_json: historySessionJson });
loadedHistoryClient.call("undo", { expected: identity });
assert.throws(
  () => loadedHistoryClient.call("redo", { expected: nextIdentity }),
  (error) => {
    assert.ok(error instanceof LineageRpcResponseError);
    assert.equal(error.violation, "invalid_result");
    return true;
  },
  "history loaded across the untrusted boundary retains policy but strips accepted authority",
);
assert.throws(
  () => loadedHistoryClient.call("redo", { expected: nextIdentity }),
  (error) => {
    assert.ok(error instanceof LineageRpcResponseError);
    assert.equal(error.violation, "invalid_result");
    return true;
  },
  "Redo cannot invent a historical policy that disagrees with the loaded checkpoint",
);

const failedClient = new LineageRpcClient({
  request(request) {
    const envelope = JSON.parse(request);
    return JSON.stringify(
      validResponse(
        envelope,
        envelope.method === "evaluate" ? validFailedEvaluation : validResult(envelope.method),
      ),
    );
  },
});
failedClient.call("create", {});
const failedEvaluation = failedClient.call("evaluate", {
  expected: identity,
  host_inputs: hostInputs,
});
assert.equal(failedEvaluation.ok, true);
assert.equal(failedEvaluation.result.disposition, "failed");
assert.equal(failedEvaluation.result.evidence.code, "dependency_blocked");
const domainFailedClient = new LineageRpcClient({
  request(request) {
    const envelope = JSON.parse(request);
    return JSON.stringify(
      validResponse(
        envelope,
        envelope.method === "evaluate" ? validDomainFailedEvaluation : validResult(envelope.method),
      ),
    );
  },
});
domainFailedClient.call("create", {});
assert.equal(
  domainFailedClient.call("evaluate", { expected: identity, host_inputs: hostInputs }).result
    .disposition,
  "failed",
);

const invalidEvaluationCases = [
  [
    "accepted evidence digest disagrees with accepted authority",
    {
      ...validAcceptedEvaluation,
      evidence: {
        ...validAcceptedEvaluation.evidence,
        materialization_digest: "11".repeat(32),
      },
    },
  ],
  [
    "accepted attempt carries failed-step evidence",
    {
      ...validAcceptedEvaluation,
      attempt: {
        ...validAcceptedEvaluation.attempt,
        failed_steps: ["0000000000000001"],
      },
    },
  ],
  [
    "failed attempt omits failed-step evidence",
    {
      ...validFailedEvaluation,
      attempt: { ...validFailedEvaluation.attempt, failed_steps: [] },
    },
  ],
  [
    "failed evaluation invents accepted authority when the retained client has none",
    {
      ...validFailedEvaluation,
      last_accepted: validAuthority,
    },
  ],
  [
    "dependency-blocked evidence claims a domain failed_step",
    {
      ...validFailedEvaluation,
      evidence: {
        ...validFailedEvaluation.evidence,
        failed_step: "0000000000000001",
      },
    },
  ],
  [
    "accepted evaluation contains a blocked dependency plan",
    {
      ...validAcceptedEvaluation,
      steps: [
        {
          step: "0000000000000001",
          state: "blocked",
          dependencies: ["0000000000000002"],
        },
        {
          step: "0000000000000002",
          state: "ready",
          dependencies: [],
        },
      ],
    },
  ],
  [
    "accepted evidence overstates the validated ready-prefix count",
    {
      ...validAcceptedEvaluation,
      evidence: {
        ...validAcceptedEvaluation.evidence,
        validated_prefix_count: 2,
      },
    },
  ],
  [
    "blocked dependency appears after its consumer",
    {
      ...validFailedEvaluation,
      steps: [
        {
          step: "0000000000000001",
          state: "blocked",
          dependencies: ["0000000000000002"],
        },
        {
          step: "0000000000000002",
          state: "suppressed",
          dependencies: [],
        },
      ],
      attempt: validAttempt(
        "failed",
        null,
        "semantic-evaluation-blocked",
        ["0000000000000001"],
      ),
    },
  ],
  [
    "blocked dependency incorrectly names a ready step",
    {
      ...validFailedEvaluation,
      steps: [
        { step: "0000000000000001", state: "ready", dependencies: [] },
        {
          step: "0000000000000002",
          state: "blocked",
          dependencies: ["0000000000000001"],
        },
      ],
    },
  ],
  [
    "blocked evaluation row has no blocking dependency",
    {
      ...validFailedEvaluation,
      steps: [
        { step: "0000000000000001", state: "suppressed", dependencies: [] },
        { step: "0000000000000002", state: "blocked", dependencies: [] },
      ],
    },
  ],
  [
    "strict plan resumes readiness after its first block",
    {
      ...validFailedEvaluation,
      steps: [
        { step: "0000000000000001", state: "suppressed", dependencies: [] },
        {
          step: "0000000000000002",
          state: "blocked",
          dependencies: ["0000000000000001"],
        },
        { step: "0000000000000003", state: "ready", dependencies: [] },
      ],
    },
  ],
  [
    "dependency_plan with an empty failed set reports a failed state",
    {
      ...validDomainFailedEvaluation,
      steps: [
        { step: "0000000000000001", state: "failed", dependencies: [] },
      ],
    },
  ],
  [
    "accepted evaluation contains no ready prefix",
    {
      ...validAcceptedEvaluation,
      evidence: { ...validAcceptedEvaluation.evidence, validated_prefix_count: 0 },
      steps: [
        { step: "0000000000000001", state: "suppressed", dependencies: [] },
      ],
    },
  ],
  [
    "domain failure names a ready step absent from the returned plan",
    {
      ...validDomainFailedEvaluation,
      evidence: {
        ...validDomainFailedEvaluation.evidence,
        failed_step: "0000000000000002",
      },
    },
  ],
];

for (const [label, result] of invalidEvaluationCases) {
  let call = 0;
  const invalidClient = new LineageRpcClient({
    request(request) {
      const envelope = JSON.parse(request);
      call += 1;
      return JSON.stringify(validResponse(envelope, call === 1 ? validSnapshot : result));
    },
  });
  invalidClient.call("create", {});
  assert.throws(
    () => invalidClient.call("evaluate", { expected: identity, host_inputs: hostInputs }),
    (error) => {
      assert.ok(error instanceof LineageRpcResponseError, label);
      assert.equal(error.violation, "invalid_result", label);
      return true;
    },
  );
}

const dependencyLocalSnapshot = {
  ...validSnapshot,
  evaluation_policy: "dependency_local",
  document: { ...validDocument, evaluation_policy: "dependency_local" },
};
const dependencyLocalImpossiblePlan = {
  ...validFailedEvaluation,
  steps: [
    { step: "0000000000000001", state: "suppressed", dependencies: [] },
    {
      step: "0000000000000002",
      state: "blocked",
      dependencies: ["0000000000000001"],
    },
    { step: "0000000000000003", state: "unevaluated", dependencies: [] },
  ],
  attempt: {
    ...validFailedEvaluation.attempt,
    policy: "dependency_local",
  },
};
let dependencyLocalCall = 0;
const dependencyLocalClient = new LineageRpcClient({
  request(request) {
    const envelope = JSON.parse(request);
    dependencyLocalCall += 1;
    return JSON.stringify(
      validResponse(
        envelope,
        dependencyLocalCall === 1 ? dependencyLocalSnapshot : dependencyLocalImpossiblePlan,
      ),
    );
  },
});
dependencyLocalClient.call("create", {});
assert.throws(
  () => dependencyLocalClient.call("evaluate", { expected: identity, host_inputs: hostInputs }),
  (error) => {
    assert.ok(error instanceof LineageRpcResponseError);
    assert.equal(error.violation, "invalid_result");
    return true;
  },
  "dependency-local evaluation cannot contain strict-only unevaluated rows",
);

const sameDocumentFutureIdentity = {
  ...identity,
  revision: "0000000000000001",
  digest: "11".repeat(32),
};
const internallyConsistentWrongEvaluation = {
  ...validAcceptedEvaluation,
  identity: sameDocumentFutureIdentity,
  evidence: {
    ...validAcceptedEvaluation.evidence,
    materialization_digest: sameDocumentFutureIdentity.digest,
  },
  attempt: {
    ...validAcceptedEvaluation.attempt,
    target: sameDocumentFutureIdentity,
    materialization_digest: sameDocumentFutureIdentity.digest,
  },
  last_accepted: {
    ...validAcceptedEvaluation.last_accepted,
    lineage: sameDocumentFutureIdentity,
    materialization_digest: sameDocumentFutureIdentity.digest,
  },
};
let wrongEvaluationCall = 0;
const wrongEvaluationClient = new LineageRpcClient({
  request(request) {
    const envelope = JSON.parse(request);
    wrongEvaluationCall += 1;
    return JSON.stringify(
      validResponse(
        envelope,
        wrongEvaluationCall === 1 ? validSnapshot : internallyConsistentWrongEvaluation,
      ),
    );
  },
});
wrongEvaluationClient.call("create", {});
assert.throws(
  () => wrongEvaluationClient.call("evaluate", { expected: identity, host_inputs: hostInputs }),
  (error) => {
    assert.ok(error instanceof LineageRpcResponseError);
    assert.equal(error.violation, "invalid_result");
    return true;
  },
  "evaluate must correlate the exact request revision/digest, not only its session document",
);

const foreignIdentity = {
  ...identity,
  document: "0000000000000000000000000000bad0",
};
for (const method of ["mutate", "reconcile", "evaluate", "export", "export_lineage"]) {
  let call = 0;
  const invalidClient = new LineageRpcClient({
    request(request) {
      const envelope = JSON.parse(request);
      call += 1;
      const result = call === 1
        ? validSnapshot
        : { ...validResult(method), identity: foreignIdentity };
      return JSON.stringify(validResponse(envelope, result));
    },
  });
  invalidClient.call("create", {});
  const params = method === "mutate"
    ? { patch }
    : method === "reconcile"
      ? { expected: identity, lineage_json: nextDocumentJson }
      : method === "evaluate"
        ? { expected: identity, host_inputs: hostInputs }
        : {};
  assert.throws(
    () => invalidClient.call(method, params),
    (error) => {
      assert.ok(error instanceof LineageRpcResponseError, method);
      assert.equal(error.violation, "invalid_result", method);
      return true;
    },
  );
}

let oversizedTransportCalled = false;
const oversizedClient = new LineageRpcClient({
  request() {
    oversizedTransportCalled = true;
    throw new Error("oversized request reached the transport");
  },
});
assert.throws(
  () => oversizedClient.call("load", { session_json: "x".repeat(RPC_MAX_REQUEST_BYTES) }),
  (error) => {
    assert.ok(error instanceof LineageRpcRequestError);
    assert.equal(error.code, "request_too_large");
    assert.equal(error.byte_limit, RPC_MAX_REQUEST_BYTES);
    assert.ok(error.byte_length > error.byte_limit);
    return true;
  },
);
assert.equal(oversizedTransportCalled, false);

const malformedResponseCases = [
  ["invalid JSON", "invalid_json", () => "{"],
  ["non-object envelope", "invalid_envelope", () => "[]"],
  [
    "uncorrelated Rust pre-envelope rejection",
    "invalid_envelope",
    () => JSON.stringify({
      protocol: "geosolve.lineage.rpc.v0",
      request_id: null,
      method: null,
      session_id: null,
      ok: false,
      error: {
        code: "invalid_request",
        message: "request envelope could not be decoded",
      },
    }),
  ],
  [
    "wrong protocol",
    "protocol_mismatch",
    (response) => JSON.stringify({ ...response, protocol: "geosolve.lineage.rpc.v1" }),
  ],
  [
    "wrong request correlation",
    "request_id_mismatch",
    (response) => JSON.stringify({ ...response, request_id: "other" }),
  ],
  [
    "wrong method correlation",
    "method_mismatch",
    (response) => JSON.stringify({ ...response, method: "inspect" }),
  ],
  [
    "successful start without a session",
    "session_id_mismatch",
    (response) => JSON.stringify({ ...response, session_id: null }),
  ],
  [
    "missing result",
    "invalid_envelope",
    (response) => {
      const { result: _result, ...withoutResult } = response;
      return JSON.stringify(withoutResult);
    },
  ],
  [
    "simultaneous result and error",
    "invalid_envelope",
    (response) =>
      JSON.stringify({ ...response, error: { code: "impossible", message: "both branches" } }),
  ],
  [
    "failed response with a result instead of an error",
    "invalid_envelope",
    (response) => JSON.stringify({ ...response, session_id: null, ok: false }),
  ],
  [
    "unknown envelope member",
    "invalid_envelope",
    (response) => JSON.stringify({ ...response, extension: true }),
  ],
  [
    "non-finite JSON number",
    "invalid_result",
    (response) =>
      JSON.stringify(response).replace('"version":1,"document_id"', '"version":1e400,"document_id"'),
  ],
  [
    "method-incompatible result",
    "invalid_result",
    (response) => JSON.stringify({ ...response, result: { identity } }),
  ],
  [
    "unknown snapshot result member",
    "invalid_result",
    (response) =>
      JSON.stringify({ ...response, result: { ...response.result, unregistered: true } }),
  ],
  [
    "malformed structured error",
    "invalid_envelope",
    (response) => {
      const { result: _result, ...failure } = response;
      return JSON.stringify({
        ...failure,
        session_id: null,
        ok: false,
        error: { code: "Not-Stable", message: "" },
      });
    },
  ],
  [
    "unknown structured error code",
    "unknown_error_code",
    (response) => {
      const { result: _result, ...failure } = response;
      return JSON.stringify({
        ...failure,
        session_id: null,
        ok: false,
        error: { code: "future_unregistered_error", message: "not in the closed protocol" },
      });
    },
  ],
];

for (const [label, violation, encode] of malformedResponseCases) {
  const invalidClient = new LineageRpcClient({
    request(request) {
      const envelope = JSON.parse(request);
      return encode(validResponse(envelope));
    },
  });
  assert.throws(
    () => invalidClient.call("create", {}),
    (error) => {
      assert.ok(error instanceof LineageRpcResponseError, label);
      assert.equal(error.violation, violation, label);
      return true;
    },
  );
}

const correlatedRequests = [];
let correlatedCall = 0;
const correlatedClient = new LineageRpcClient({
  request(request) {
    const envelope = JSON.parse(request);
    correlatedRequests.push(envelope);
    correlatedCall += 1;
    if (correlatedCall === 1) return JSON.stringify(validResponse(envelope));
    if (correlatedCall === 2) {
      return JSON.stringify({
        ...validResponse(envelope),
        session_id: "lineage:0000000000000000000000000000bad0",
      });
    }
    return JSON.stringify({
      protocol: "geosolve.lineage.rpc.v0",
      request_id: envelope.request_id,
      method: envelope.method,
      session_id: envelope.session_id,
      ok: false,
      error: { code: "lineage_rejected", message: "test rejection" },
    });
  },
});
correlatedClient.call("create", {});
assert.throws(
  () => correlatedClient.call("inspect", {}),
  (error) => {
    assert.ok(error instanceof LineageRpcResponseError);
    assert.equal(error.violation, "session_id_mismatch");
    return true;
  },
);
const ordinaryFailure = correlatedClient.call("inspect", {});
assert.equal(ordinaryFailure.ok, false);
assert.equal(ordinaryFailure.error.code, "lineage_rejected");
assert.equal(
  correlatedRequests[2].session_id,
  sessionId,
  "a malformed response must not replace the retained client session",
);

const retainedSessionRequests = [];
let retainedSessionCall = 0;
const retainedSessionClient = new LineageRpcClient({
  request(request) {
    const envelope = JSON.parse(request);
    retainedSessionRequests.push(envelope);
    retainedSessionCall += 1;
    if (retainedSessionCall === 1) return JSON.stringify(validResponse(envelope));
    if (retainedSessionCall === 2) {
      return JSON.stringify(
        validResponse(envelope, {}, "lineage:0000000000000000000000000000bad0"),
      );
    }
    return JSON.stringify(validResponse(envelope));
  },
});
retainedSessionClient.call("create", {});
assert.throws(
  () => retainedSessionClient.call("load", { session_json: "{}" }),
  (error) => {
    assert.ok(error instanceof LineageRpcResponseError);
    assert.equal(error.violation, "invalid_result");
    return true;
  },
);
retainedSessionClient.call("inspect", {});
assert.equal(
  retainedSessionRequests[2].session_id,
  sessionId,
  "a malformed session-start result must not replace the retained client session",
);

const assertInvalidAfterCreate = (label, method, params, forgedResult) => {
  let call = 0;
  const observed = [];
  const invalidClient = new LineageRpcClient({
    request(request) {
      const envelope = JSON.parse(request);
      observed.push(envelope);
      call += 1;
      const result = call === 1
        ? validSnapshot
        : call === 2
          ? forgedResult
          : validSnapshot;
      return JSON.stringify(validResponse(envelope, result));
    },
  });
  invalidClient.call("create", {});
  assert.throws(
    () => invalidClient.call(method, params),
    (error) => {
      assert.ok(error instanceof LineageRpcResponseError, label);
      assert.equal(error.violation, "invalid_result", label);
      return true;
    },
    label,
  );
  const retained = invalidClient.call("inspect", {});
  assert.equal(
    retained.result.evaluation_policy,
    "strict_chronological",
    `${label}: malformed reply changed the retained policy`,
  );
  assert.equal(observed[2].session_id, sessionId, `${label}: retained session changed`);
};

const explicitCreateClient = new LineageRpcClient({
  request(request) {
    return JSON.stringify(validResponse(JSON.parse(request), validSnapshot));
  },
});
assert.throws(
  () => explicitCreateClient.call("create", { document_id: foreignIdentity.document }),
  (error) => {
    assert.ok(error instanceof LineageRpcResponseError);
    assert.equal(error.violation, "invalid_result");
    return true;
  },
  "create must return the explicitly requested document identity",
);

assertInvalidAfterCreate(
  "load cannot restore caller-asserted accepted authority",
  "load",
  { session_json: validSessionJson },
  {
    ...validLoadedSnapshot,
    latest_attempt: validAttempt("accepted", identity.digest),
    last_accepted: validAuthority,
    last_accepted_document: validDocument,
  },
);
assertInvalidAfterCreate(
  "load lifecycle must match the serialized session",
  "load",
  { session_json: validSessionJson },
  {
    ...validLoadedSnapshot,
    lifecycle: { ...validLoadedSnapshot.lifecycle, revision: nextIdentity.revision },
  },
);
assertInvalidAfterCreate(
  "import cannot invent history",
  "import",
  { lineage_json: validDocumentJson },
  { ...validSnapshot, can_undo: true, undo_length: "1" },
);
assertInvalidAfterCreate(
  "import cannot invent accepted authority",
  "import",
  { lineage_json: validDocumentJson },
  {
    ...validSnapshot,
    latest_attempt: validAttempt("accepted", identity.digest),
    last_accepted: validAuthority,
    last_accepted_document: validDocument,
  },
);

const inconsistentHistoryClient = new LineageRpcClient({
  request(request) {
    const envelope = JSON.parse(request);
    return JSON.stringify(
      validResponse(envelope, { ...validSnapshot, can_undo: true, undo_length: "0" }),
    );
  },
});
assert.throws(
  () => inconsistentHistoryClient.call("create", {}),
  (error) => {
    assert.ok(error instanceof LineageRpcResponseError);
    assert.equal(error.violation, "invalid_result");
    return true;
  },
  "snapshot history booleans and lengths must agree",
);

const stalePatch = { ...patch, expected: nextIdentity };
assertInvalidAfterCreate(
  "successful CAS cannot target a stale retained identity",
  "mutate",
  { patch: stalePatch },
  {
    identity: secondNextIdentity,
    changed: true,
    inserted_steps: [insertedStep.id],
    tombstoned_steps: [],
  },
);
assertInvalidAfterCreate(
  "changed mutation advances exactly one revision",
  "mutate",
  { patch },
  {
    identity: secondNextIdentity,
    changed: true,
    inserted_steps: [insertedStep.id],
    tombstoned_steps: [],
  },
);
assertInvalidAfterCreate(
  "mutation insertion evidence matches its patch",
  "mutate",
  { patch },
  {
    identity: nextIdentity,
    changed: true,
    inserted_steps: [],
    tombstoned_steps: [],
  },
);
assertInvalidAfterCreate(
  "insert-only mutation cannot invent tombstoned-step evidence",
  "mutate",
  { patch },
  {
    identity: nextIdentity,
    changed: true,
    inserted_steps: [insertedStep.id],
    tombstoned_steps: [insertedStep.id],
  },
);
const rewriteOnlyMutationPatch = {
  expected: identity,
  mutations: rewritePatch.mutations,
};
assertInvalidAfterCreate(
  "non-tombstoning mutation cannot invent tombstoned-step evidence",
  "mutate",
  { patch: rewriteOnlyMutationPatch },
  {
    identity: nextIdentity,
    changed: true,
    inserted_steps: [],
    tombstoned_steps: [insertedStep.id],
  },
);
const directTombstonePatch = {
  expected: identity,
  mutations: [{ kind: "tombstone", step: insertedStep.id }],
};
assertInvalidAfterCreate(
  "changed direct-tombstone-only mutation cannot omit all tombstone evidence",
  "mutate",
  { patch: directTombstonePatch },
  {
    identity: nextIdentity,
    changed: true,
    inserted_steps: [],
    tombstoned_steps: [],
  },
);
const deleteSubtreePatch = {
  expected: identity,
  mutations: [{ kind: "delete_subtree", root: insertedStep.id }],
};
assertInvalidAfterCreate(
  "changed delete-subtree-only mutation cannot omit all tombstone evidence",
  "mutate",
  { patch: deleteSubtreePatch },
  {
    identity: nextIdentity,
    changed: true,
    inserted_steps: [],
    tombstoned_steps: [],
  },
);
callOneValidMethod("mutate", { patch: directTombstonePatch }, {
  identity: nextIdentity,
  changed: true,
  inserted_steps: [],
  tombstoned_steps: [insertedStep.id],
});
callOneValidMethod("mutate", { patch: directTombstonePatch }, {
  identity,
  changed: false,
  inserted_steps: [],
  tombstoned_steps: [],
});
const insertedThenTombstonedPatch = {
  expected: identity,
  mutations: [
    patch.mutations[0],
    { kind: "tombstone", step: insertedStep.id },
  ],
};
assertInvalidAfterCreate(
  "newly inserted direct tombstone target cannot be omitted from evidence",
  "mutate",
  { patch: insertedThenTombstonedPatch },
  {
    identity: nextIdentity,
    changed: true,
    inserted_steps: [insertedStep.id],
    tombstoned_steps: [],
  },
);
const tombstoneAndRewritePatch = {
  expected: identity,
  mutations: [
    directTombstonePatch.mutations[0],
    { ...rewritePatch.mutations[0], step: "0000000000000002" },
  ],
};
// The data-only client cannot know whether this direct tombstone was a Rust
// no-op while the rewrite caused the revision change, so omission remains the
// strongest truthful mixed-batch contract.
callOneValidMethod("mutate", { patch: tombstoneAndRewritePatch }, {
  identity: nextIdentity,
  changed: true,
  inserted_steps: [],
  tombstoned_steps: [],
});
assertInvalidAfterCreate(
  "rewrite_owners cannot report inserted lifecycle state",
  "rewrite_owners",
  { patch: rewritePatch },
  {
    identity,
    changed: false,
    inserted_steps: [insertedStep.id],
    tombstoned_steps: [],
  },
);
assertInvalidAfterCreate(
  "set_policy change evidence matches the retained policy",
  "set_policy",
  { expected: identity, policy: "dependency_local" },
  { identity, changed: false, inserted_steps: [], tombstoned_steps: [] },
);
const genericPolicyMutationPatch = {
  expected: identity,
  mutations: [
    { kind: "set_evaluation_policy", policy: "dependency_local" },
  ],
};
assertInvalidAfterCreate(
  "unchanged generic mutation cannot forge a retained policy transition",
  "mutate",
  { patch: genericPolicyMutationPatch },
  { identity, changed: false, inserted_steps: [], tombstoned_steps: [] },
);
callOneValidMethod(
  "mutate",
  {
    patch: {
      expected: identity,
      mutations: [
        { kind: "set_evaluation_policy", policy: "strict_chronological" },
      ],
    },
  },
  { identity, changed: false, inserted_steps: [], tombstoned_steps: [] },
);
callOneValidMethod(
  "mutate",
  { patch: genericPolicyMutationPatch },
  {
    identity: nextIdentity,
    changed: true,
    inserted_steps: [],
    tombstoned_steps: [],
  },
);

const secondNextDocumentJson = JSON.stringify({
  ...validDocument,
  revision: secondNextIdentity.revision,
  digest: secondNextIdentity.digest,
});
assertInvalidAfterCreate(
  "reconcile advances exactly one revision",
  "reconcile",
  { expected: identity, lineage_json: secondNextDocumentJson },
  { identity: secondNextIdentity },
);
assertInvalidAfterCreate(
  "Undo advances exactly one revision",
  "undo",
  { expected: identity },
  { identity: secondNextIdentity },
);

const foreignDocument = {
  ...validDocument,
  document_id: foreignIdentity.document,
};
const foreignSessionJson = JSON.stringify({
  ...JSON.parse(validSessionJson),
  document: JSON.stringify(foreignDocument),
});
assertInvalidAfterCreate(
  "session export embeds the returned current identity",
  "export",
  {},
  { identity, session_json: foreignSessionJson },
);
assertInvalidAfterCreate(
  "lineage export embeds the returned current identity",
  "export_lineage",
  {},
  { identity, lineage_json: JSON.stringify(foreignDocument) },
);

const oversizedResultClient = new LineageRpcClient({
  request(request) {
    const envelope = JSON.parse(request);
    return JSON.stringify(
      validResponse(envelope, {
        ...validSnapshot,
        document: {
          ...validDocument,
          steps: new Array(RPC_MAX_RESULT_ITEMS + 1).fill(null),
        },
      }),
    );
  },
});
assert.throws(
  () => oversizedResultClient.call("create", {}),
  (error) => {
    assert.ok(error instanceof LineageRpcResponseError);
    assert.equal(error.violation, "invalid_result");
    return true;
  },
  "result arrays above Rust's retained-step bound must reject",
);

const mutableExpected = { expected: identity, host_inputs: hostInputs };
let mutableExpectedCall = 0;
const mutableExpectedClient = new LineageRpcClient({
  request(request) {
    const envelope = JSON.parse(request);
    mutableExpectedCall += 1;
    if (mutableExpectedCall === 1) return JSON.stringify(validResponse(envelope));
    mutableExpected.expected = sameDocumentFutureIdentity;
    return JSON.stringify(validResponse(envelope, internallyConsistentWrongEvaluation));
  },
});
mutableExpectedClient.call("create", {});
assert.throws(
  () => mutableExpectedClient.call("evaluate", mutableExpected),
  (error) => {
    assert.ok(error instanceof LineageRpcResponseError);
    assert.equal(error.violation, "invalid_result");
    return true;
  },
  "response decoding must use the immutable parameters that actually entered transport",
);
