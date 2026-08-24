// SPDX-License-Identifier: GPL-3.0-or-later

import {
  IntentClient,
  aliasPort,
  createNode,
  draft,
  ejectBootstrapPoint,
  input,
  leaf,
  nodePort,
  patch,
  session,
  sessionIdentity,
  setInstanceLeaf,
  stableNode,
  stablePort,
} from "../src/index.js";
import type {
  IntentGraphChild,
  IntentGraphNodeKind,
  IntentOperationOutput,
  IntentRpcPatchResponse,
  IntentRpcSnapshotResponse,
  IntentSourceSnapshot,
} from "../src/index.js";

const first = session("11111111111111111111111111111111");
const second = session("22222222222222222222222222222222");
const component = {
  revision: "0000000000000000",
  digest: "0000000000000000000000000000000000000000000000000000000000000000",
};
const firstIdentity = sessionIdentity(first, {
  session: first.id,
  revision: component.revision,
  digest: component.digest,
  graph: component,
  instance: component,
  reservations: component,
  organization: component,
  external_inputs: component,
});
const secondIdentity = sessionIdentity(second, {
  session: second.id,
  revision: component.revision,
  digest: component.digest,
  graph: component,
  instance: component,
  reservations: component,
  organization: component,
  external_inputs: component,
});

const point = stablePort(first, stableNode(first, "0000000000000001"), "0000000000000011", "point");
const scalar = stablePort(first, stableNode(first, "0000000000000002"), "0000000000000012", "scalar");
const curve = stablePort(first, stableNode(first, "0000000000000003"), "0000000000000013", "curve");
const foreignPoint = stablePort(second, stableNode(second, "0000000000000001"), "0000000000000011", "point");

input(first, "point", 0, point);
input(first, "curve", 0, curve);
leaf(point, "x");
leaf(scalar, "angle");

// @ts-expect-error A curve cannot occupy a point input.
input(first, "point", 0, curve);
// @ts-expect-error Branded references from another session cannot be mixed.
input(first, "point", 0, foreignPoint);
// @ts-expect-error A point has no scalar value leaf.
leaf(point, "value");
// @ts-expect-error A scalar has no Cartesian leaf.
leaf(scalar, "x");

const start = aliasPort(first, "start", nodePort("primary"), "point");
const segmentDraft = draft(first, "segment.main", {
  family: "geometry",
  recipe: "segment",
}, {
  inputs: [input(first, "point", 0, start)],
});
const curveOperationOutput: IntentOperationOutput = {
  kind: "curve",
  curve_span_count: 2,
};
draft(first, "operation.rectangle", {
  family: "operation",
  operation: "rectangle",
}, {
  operationOutputs: [curveOperationOutput],
});
const graphChild: IntentGraphChild = {
  child: "0000000000000020",
  schema: "polyline_vertex",
  ports: [{
    node: "0000000000000004",
    port: "0000000000000021",
    kind: "point",
  }],
};
graphChild.ports[0]?.kind;
const operation = createNode(first, "segment", segmentDraft);
patch(first, firstIdentity, "require_accepted", [operation]);
ejectBootstrapPoint(first, stableNode(first, "0000000000000004"));

const client = new IntentClient(first.id, { apply: () => "accepted" });
client.editSourceToken(firstIdentity, 0, "1");
// @ts-expect-error Source-token CAS identities are branded to the client's session namespace.
client.editSourceToken(secondIdentity, 0, "1");

// @ts-expect-error Bootstrap ejection cannot target another session namespace.
ejectBootstrapPoint(first, stableNode(second, "0000000000000004"));

const foreignDraft = draft(second, "point.foreign", {
  family: "geometry",
  recipe: "sketch_point",
});
const foreignOperation = createNode(second, "foreign", foreignDraft);

// @ts-expect-error Patch operations are branded to one exact session namespace.
patch(first, firstIdentity, "require_accepted", [foreignOperation]);
// @ts-expect-error A full identity from another session cannot stamp the patch.
patch(first, secondIdentity, "require_accepted", [operation]);

// @ts-expect-error Instance leaves are numerical quantity literals on the Rust wire.
setInstanceLeaf(first, leaf(point, "x"), { kind: "enum", value: "invalid" });

const generatedSource = {
  cells: [{
    cell: "0000000000000001",
    name: "Imported",
    declarations: [{
      node: "0000000000000002",
      symbol: "bootstrap.point",
      name: "Bootstrap point",
      kind: {
        family: "bootstrap",
        object: {
          kind: "point",
          codec: "sketch-point-v1",
          payload_bytes: 512,
          payload_sha256: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        },
      },
      suppressed: false,
      inputs: {},
      fields: {},
      instance: {},
    }],
  }],
} as const satisfies IntentSourceSnapshot;

generatedSource.cells[0].declarations[0].kind.object.payload_sha256;

const rawBootstrapKind: IntentGraphNodeKind = {
  family: "bootstrap",
  // @ts-expect-error Structured Source admits compact metadata, never raw bootstrap payload bytes.
  object: { kind: "point", codec: "sketch-point-v1", payload: [1, 2, 3] },
};

type Equal<Left, Right> =
  (<T>() => T extends Left ? 1 : 2) extends
  (<T>() => T extends Right ? 1 : 2) ? true : false;
type Assert<Value extends true> = Value;

type SnapshotReturn = Awaited<ReturnType<typeof client.snapshot>>;
type PatchReturn = Awaited<ReturnType<typeof client.apply>>;
type SnapshotIsTyped = Assert<Equal<SnapshotReturn, IntentRpcSnapshotResponse<typeof first.id>>>;
type PatchIsTyped = Assert<Equal<PatchReturn, IntentRpcPatchResponse<typeof first.id>>>;

declare const firstResponse: SnapshotReturn;
const sameSessionResponse: IntentRpcSnapshotResponse<typeof first.id> = firstResponse;
sameSessionResponse;

declare const patchResponse: PatchReturn;
if (patchResponse.outcome === "success") {
  patchResponse.value.receipt.identity.session;
  patchResponse.value.receipt.aliases.nodes;
}

// @ts-expect-error Parsed response branding cannot cross an intent-session namespace.
const foreignResponse: IntentRpcSnapshotResponse<typeof second.id> = firstResponse;
foreignResponse;

// Keep compile-time assertions live under noUnusedLocals-compatible configurations.
type TypeAssertions = SnapshotIsTyped | PatchIsTyped;
declare const typeAssertions: TypeAssertions;
typeAssertions;
