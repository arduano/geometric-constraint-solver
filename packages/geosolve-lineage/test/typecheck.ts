// SPDX-License-Identifier: GPL-3.0-or-later

import {
  LineagePatchBuilder,
  LineageRpcClient,
  annotations,
  bindings,
  computedFeatures,
  constraints,
  developerKey,
  dimensions,
  externals,
  geometry,
  nativePublications,
  operations,
  parameters,
  persistentId,
  semanticKey,
  trims,
  type AllocatorHighWater,
  type LineageIdentity,
  type LineageReservationId,
} from "../src/index.js";

const identity = {
  document: "00000000000000000000000000000083",
  revision: "0000000000000000",
  digest: "0000000000000000000000000000000000000000000000000000000000000000",
} as LineageIdentity;
const hostInputs = {
  version: 1,
  parameter_batch_json:
    '{"version":1,"revision":0,"digest":[178,48,67,32,232,86,240,224,17,22,252,246,136,181,160,168,150,4,247,242,2,49,6,164,189,140,2,203,116,23,238,239],"entries":[]}',
  external_snapshot_set_json:
    '{"version":1,"revision":0,"digest":[242,68,150,135,86,171,49,151,133,16,230,224,165,214,84,191,34,4,18,89,26,83,128,95,16,92,28,50,48,252,92,180],"entries":[]}',
} as const;
const highWater = {
  next_step_id: "0000000000000001",
  next_output_id: "0000000000000001",
  next_reservation_id: "0000000000000001",
} as AllocatorHighWater;

const patch = new LineagePatchBuilder(identity, highWater);
patch.insert({
  key: developerKey("profile"),
  label: "Profile",
  action: {
    kind: "geometry_recipe",
    action: geometry["two-point-aligned-rectangle"]({
      parameters: { first: [0, 0], second: [20, 10], regularized: false },
    }),
  },
  outputs: [
    {
      key: semanticKey("edge:right"),
      kind: "curve",
      reservation: "0000000000000001" as LineageReservationId,
      writable: [semanticKey("definition.radius")],
    },
  ],
  reservations: [
    {
      key: semanticKey("edge:right"),
      kind: "curve",
      persistent_id: persistentId("sketch:curve:31"),
    },
  ],
});

void constraints.horizontal({});
void dimensions["oriented-angle"]({ parameters: { orientation: "counter_clockwise" } });
void operations["profile-offset"]({ parameters: { distance: 2, side: "outward" } });
void nativePublications["native-line-fillet"]({ parameters: { radius: 1 } });
void computedFeatures["fillet-set"]({ parameters: { radius: 1 } });
void trims["curve-trim-view"]({});
void parameters["host-parameter"]({ parameters: { kind: "length" } });
void bindings["parameter-binding"]({});
void externals["external-binding"]({});
void annotations["annotation-layout"]({});
void patch.patch();

declare const client: LineageRpcClient;
const response = client.call("evaluate", {
  expected: {
    document: identity.document,
    revision: identity.revision,
    digest: identity.digest,
  },
  host_inputs: hostInputs,
});
if (response.ok) {
  void response.result.disposition;
  if (response.result.disposition === "accepted") {
    void response.result.evidence.materialization_digest;
  } else {
    void response.result.evidence.code;
  }
  // @ts-expect-error a successful envelope cannot contain an error branch
  void response.error;
} else {
  void response.error.code;
  // @ts-expect-error a failed envelope cannot contain a result branch
  void response.result;
}
const snapshot = client.call("inspect", {});
if (snapshot.ok) {
  void snapshot.result.can_undo;
  void snapshot.result.document;
  // @ts-expect-error inspect returns a snapshot, not an export payload
  void snapshot.result.session_json;
}
const mutation = client.call("mutate", { patch: patch.patch() });
if (mutation.ok) {
  void mutation.result.inserted_steps;
  // @ts-expect-error mutate returns mutation evidence, not a snapshot
  void mutation.result.can_undo;
}
const undone = client.call("undo", { expected: identity });
if (undone.ok) {
  void undone.result.evaluation_policy;
  void undone.result.last_accepted;
  // @ts-expect-error Undo returns restored snapshot authority, not mutation evidence
  void undone.result.tombstoned_steps;
}
const exported = client.call("export", {});
if (exported.ok) {
  void exported.result.session_json;
  // @ts-expect-error export does not return lineage-only JSON
  void exported.result.lineage_json;
}
// @ts-expect-error inspect accepts no fields
void client.call("inspect", { unexpected: true });
// @ts-expect-error load requires session_json
void client.call("load", {});
// @ts-expect-error evaluate requires host_inputs
void client.call("evaluate", { expected: identity });
// @ts-expect-error set_policy accepts only registered policy keys
void client.call("set_policy", { expected: identity, policy: "future" });
// @ts-expect-error mutate requires the closed Rust LineagePatch shape
void client.call("mutate", { patch: null });
void client.call("rewrite_owners", {
  // @ts-expect-error mutation kinds and fields are closed
  patch: { expected: identity, mutations: [{ kind: "future_mutation" }] },
});
void client.call("rewrite_owners", {
  patch: {
    expected: identity,
    // @ts-expect-error rewrite_owners accepts only existing-step rewrite mutations
    mutations: [{ kind: "tombstone", step: "0000000000000001" }],
  },
});
// @ts-expect-error callers cannot select an arbitrary asserted result type
void client.call<{ readonly forged: true }>("inspect", {});
// @ts-expect-error caller-certified acceptance is not part of the closed RPC surface
void client.call("accept", {});
// @ts-expect-error caller-certified rejection is not part of the closed RPC surface
void client.call("reject", {});
