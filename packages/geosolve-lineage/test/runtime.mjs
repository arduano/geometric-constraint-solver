// SPDX-License-Identifier: GPL-3.0-or-later

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

import {
  LineagePatchBuilder,
  LineageRpcClient,
  LineageRpcResponseError,
  actionKindKeys,
  annotationKeys,
  bindingKeys,
  constraintKeys,
  curveControlKeys,
  curvePropertyKeys,
  dimensionKeys,
  computedFeatureKeys,
  explicitBranchFamilyKeys,
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

const requests = [];
const client = new LineageRpcClient({
  request(request) {
    const envelope = JSON.parse(request);
    requests.push(envelope);
    return JSON.stringify({
      protocol: "geosolve.lineage.rpc.v0",
      request_id: envelope.request_id,
      method: envelope.method,
      session_id: "lineage:00000000000000000000000000000083",
      ok: true,
      result: {},
    });
  },
});
client.call("create", {});
client.call("inspect", {});
client.call("evaluate", { expected: identity, host_inputs: hostInputs });
assert.equal(requests[0].session_id, null);
assert.equal(requests[1].session_id, "lineage:00000000000000000000000000000083");
assert.equal(requests[2].method, "evaluate");
assert.equal(requests[2].session_id, "lineage:00000000000000000000000000000083");

const validStartResponse = (request) => ({
  protocol: "geosolve.lineage.rpc.v0",
  request_id: request.request_id,
  method: request.method,
  session_id: "lineage:00000000000000000000000000000083",
  ok: true,
  result: {},
});

const malformedResponseCases = [
  ["invalid JSON", "invalid_json", () => "{"],
  ["non-object envelope", "invalid_envelope", () => "[]"],
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
    "invalid_envelope",
    (response) => JSON.stringify(response).replace('"result":{}', '"result":1e400'),
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
];

for (const [label, violation, encode] of malformedResponseCases) {
  const invalidClient = new LineageRpcClient({
    request(request) {
      const envelope = JSON.parse(request);
      return encode(validStartResponse(envelope));
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
    if (correlatedCall === 1) return JSON.stringify(validStartResponse(envelope));
    if (correlatedCall === 2) {
      return JSON.stringify({
        ...validStartResponse(envelope),
        session_id: "lineage:0000000000000000000000000000bad0",
      });
    }
    return JSON.stringify({
      protocol: "geosolve.lineage.rpc.v0",
      request_id: envelope.request_id,
      method: envelope.method,
      session_id: envelope.session_id,
      ok: false,
      error: { code: "domain_rejected", message: "test rejection" },
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
assert.equal(ordinaryFailure.error.code, "domain_rejected");
assert.equal(
  correlatedRequests[2].session_id,
  "lineage:00000000000000000000000000000083",
  "a malformed response must not replace the retained client session",
);
