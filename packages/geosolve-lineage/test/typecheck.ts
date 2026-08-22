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
void client.call("evaluate", {
  expected: {
    document: identity.document,
    revision: identity.revision,
    digest: identity.digest,
  },
  host_inputs: hostInputs,
});
// @ts-expect-error caller-certified acceptance is not part of the closed RPC surface
void client.call("accept", {});
// @ts-expect-error caller-certified rejection is not part of the closed RPC surface
void client.call("reject", {});
