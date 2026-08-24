// SPDX-License-Identifier: GPL-3.0-or-later

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

import {
  IntentClient,
  aliasCellTarget,
  aliasPort,
  canonicalStringify,
  cascade,
  cascadeRoots,
  createCell,
  createNode,
  deleteCell,
  deleteNode,
  draft,
  encodeIntentLiteral,
  encodeIntentPatch,
  encodeIntentRpcRequest,
  ejectBootstrapPoint,
  externalInputs,
  input,
  leaf,
  moveDeclaration,
  nodePort,
  patch,
  rebindInput,
  renameNode,
  reorderCells,
  replaceExternalInputs,
  session,
  sessionIdentity,
  setDefinitionField,
  setInstanceLeaf,
  setSuppressed,
  stableCell,
  stableCellTarget,
  stableNode,
  stablePort,
} from "../src/index.js";

const fixtureText = readFileSync(
  new URL("../../test/fixtures/rust-intent-patch-v1.json", import.meta.url),
  "utf8",
).trim();
const literalFixtureText = readFileSync(
  new URL("../../test/fixtures/rust-intent-literals-v1.json", import.meta.url),
  "utf8",
).trim();
const cascadeRootsFixtureText = readFileSync(
  new URL("../../test/fixtures/rust-intent-cascade-roots-v1.json", import.meta.url),
  "utf8",
).trim();
const fixture = JSON.parse(fixtureText) as {
  expected: Parameters<typeof sessionIdentity<string>>[1];
};

function representativePatch() {
  const owner = session(fixture.expected.session);
  const expected = sessionIdentity(owner, fixture.expected);
  const n = (value: string) => stableNode(owner, value);
  const c = (value: string) => stableCell(owner, value);

  const start = stablePort(owner, n("0000000000000010"), "0000000000000020", "point");
  const end = aliasPort(owner, "created-point", nodePort("primary"), "point");
  const segment = draft(owner, "segment.main", {
    family: "geometry",
    recipe: "segment",
  }, {
    name: "Main segment",
    inputs: [input(owner, "point", 0, start), input(owner, "point", 1, end)],
    fields: {
      flag: { kind: "boolean", value: true },
      integer: { kind: "integer", value: -7 },
      natural: { kind: "natural", value: 12 },
      text: { kind: "text", value: "fixture text" },
      enum: { kind: "enum", value: "clockwise" },
      point: { kind: "point", value: [1.25, -2.5] },
      quantity: { kind: "quantity", value: { value: 3.75, unit: "length" } },
    },
    initialInstance: {
      [nodePort("start")]: {
        x: { kind: "quantity", value: { value: 4.5, unit: "length" } },
        y: { kind: "quantity", value: { value: -6.25, unit: "length" } },
      },
    } as const,
    dynamicChildren: 2,
  });

  const scalar = stablePort(owner, n("0000000000000042"), "0000000000000052", "scalar");
  const curve = stablePort(owner, n("0000000000000044"), "0000000000000054", "curve");
  return patch(owner, expected, "retain_failed_intent", [
    createNode(owner, "segment", segment, aliasCellTarget(owner, "geometry-cell")),
    deleteNode(
      owner,
      n("0000000000000030"),
      cascade(owner, [n("0000000000000031"), n("0000000000000030")]),
    ),
    setSuppressed(owner, n("0000000000000040"), true),
    setDefinitionField(
      owner,
      n("0000000000000041"),
      "sweep",
      { kind: "enum", value: "counterclockwise" },
    ),
    setInstanceLeaf(
      owner,
      leaf(scalar, "angle"),
      { kind: "quantity", value: { value: 0.75, unit: "angle" } },
    ),
    rebindInput(owner, n("0000000000000043"), "curve:0002", curve),
    renameNode(owner, n("0000000000000045"), "Renamed declaration"),
    moveDeclaration(
      owner,
      n("0000000000000046"),
      stableCellTarget(owner, c("0000000000000002")),
      n("0000000000000047"),
    ),
    createCell(owner, "geometry-cell", "Geometry", c("0000000000000003")),
    deleteCell(owner, c("0000000000000004")),
    reorderCells(owner, [
      c("0000000000000001"),
      c("0000000000000003"),
      c("0000000000000002"),
    ]),
    replaceExternalInputs(owner, externalInputs("0000000000000005", [1, 2, 3], [8, 13, 21])),
  ]);
}

test("TypeScript builders reproduce the checked canonical Rust patch byte-for-byte", () => {
  assert.equal(encodeIntentPatch(representativePatch()), fixtureText);
});

test("multi-root cascade policy reproduces the checked Rust operation byte-for-byte", () => {
  const owner = session(fixture.expected.session);
  const n = (value: string) => stableNode(owner, value);
  const operation = deleteNode(
    owner,
    n("0000000000000030"),
    cascadeRoots(
      owner,
      [n("0000000000000032"), n("0000000000000030")],
      [n("0000000000000032"), n("0000000000000031"), n("0000000000000030")],
    ),
  );
  const encoded = encodeIntentPatch(patch(
    owner,
    sessionIdentity(owner, fixture.expected),
    "require_accepted",
    [operation],
  ));
  const decoded = JSON.parse(encoded) as { operations: unknown[] };

  assert.equal(JSON.stringify(decoded.operations[0]), cascadeRootsFixtureText);
});

test("bootstrap Point ejection uses the closed Rust operation shape", () => {
  const owner = session(fixture.expected.session);
  const operation = ejectBootstrapPoint(
    owner,
    stableNode(owner, "0000000000000048"),
  );
  const encoded = encodeIntentPatch(patch(
    owner,
    sessionIdentity(owner, fixture.expected),
    "require_accepted",
    [operation],
  ));
  const decoded = JSON.parse(encoded) as { operations: unknown[] };

  assert.deepEqual(decoded.operations, [{
    operation: "eject_bootstrap_point",
    node: "0000000000000048",
  }]);
});

test("finite float edge spellings reproduce checked Rust serde bytes", () => {
  const values = [
    0,
    -0,
    1e-7,
    1e-6,
    1e-5,
    1e15,
    1e16,
    1.234567890123456e17,
    1e20,
  ].map((value) => encodeIntentLiteral({
    kind: "quantity",
    value: { value, unit: "length" },
  }));
  values.push(encodeIntentLiteral({ kind: "point", value: [1e-6, -0] }));
  values.push(encodeIntentLiteral({ kind: "integer", value: -(1n << 63n) }));
  values.push(encodeIntentLiteral({ kind: "integer", value: (1n << 63n) - 1n }));
  values.push(encodeIntentLiteral({ kind: "natural", value: (1n << 64n) - 1n }));
  assert.equal(`[${values.join(",")}]`, literalFixtureText);
  assert.throws(
    () => encodeIntentLiteral({ kind: "integer", value: 1n << 63n }),
    /fit Rust i64/u,
  );
  assert.throws(
    () => encodeIntentLiteral({ kind: "natural", value: 1n << 64n }),
    /fit Rust u64/u,
  );
});

test("builders reject cross-session, wrong-kind, malformed-ID, and invalid-leaf references", () => {
  const first = session("11111111111111111111111111111111");
  const second = session("22222222222222222222222222222222");
  const foreign = stablePort(second, stableNode(second, "0000000000000001"), "0000000000000002", "point");
  const curve = stablePort(first, stableNode(first, "0000000000000003"), "0000000000000004", "curve");

  assert.throws(
    () => input(first, "point", 0, foreign as never),
    /cross-session intent reference/u,
  );
  assert.throws(
    () => input(first, "point", 0, curve as never),
    /requires point, received curve/u,
  );
  assert.throws(() => stableNode(first, "node-1"), /lowercase hexadecimal/u);
  assert.throws(
    () => ejectBootstrapPoint(first, stableNode(second, "0000000000000007") as never),
    /cross-session intent reference/u,
  );
  assert.throws(() => aliasPort(first, "alias", "node:primary:0" as never, "point"), /selector/u);
  assert.throws(
    () => aliasPort(first, "unpaired\ud800", nodePort("primary"), "point"),
    /invalid Unicode/u,
  );
  assert.throws(
    () => leaf(stablePort(first, stableNode(first, "0000000000000005"), "0000000000000006", "scalar"), "x" as never),
    /not valid for scalar/u,
  );
});

test("full session identity and Rust quantity nesting are mandatory on transport", () => {
  const encoded = JSON.parse(encodeIntentPatch(representativePatch())) as Record<string, unknown>;
  assert.deepEqual(Object.keys(encoded.expected as object), [
    "session",
    "revision",
    "digest",
    "graph",
    "instance",
    "reservations",
    "organization",
    "external_inputs",
  ]);
  const operation = (encoded.operations as Array<Record<string, unknown>>)
    .find((value) => value.operation === "set_instance_leaf");
  assert.deepEqual(operation?.value, {
    kind: "quantity",
    value: { value: 0.75, unit: "angle" },
  });
  assert.equal("session" in encoded, false);
  assert.equal("op" in (operation ?? {}), false);
});

test("client sends only the closed Rust apply_patch RPC payload", async () => {
  const calls: string[] = [];
  const value = representativePatch();
  const client = new IntentClient(fixture.expected.session, {
    async apply(json) {
      calls.push(json);
      return "accepted-identity";
    },
  });

  assert.equal(await client.apply(value), "accepted-identity");
  assert.deepEqual(calls, [`{"method":"apply_patch","patch":${fixtureText}}`]);
  assert.doesNotMatch(calls[0] ?? "", /residual|jacobian|solve/u);
});

test("all DOM-free RPC requests use the closed Rust method tags", async () => {
  assert.equal(encodeIntentRpcRequest({ method: "snapshot" }), '{"method":"snapshot"}');
  assert.equal(encodeIntentRpcRequest({ method: "undo" }), '{"method":"undo"}');
  assert.equal(encodeIntentRpcRequest({ method: "redo" }), '{"method":"redo"}');
  assert.equal(
    encodeIntentRpcRequest({ method: "inspector", node: "0000000000000042" }),
    '{"method":"inspector","node":"0000000000000042"}',
  );
  assert.equal(
    encodeIntentRpcRequest({ method: "edit_source_token", token: 7, replacement: "3.5" }),
    '{"method":"edit_source_token","token":7,"replacement":"3.5"}',
  );
});

test("generic canonical JSON remains insertion-order independent and finite-only", () => {
  const left = canonicalStringify({ z: 1, nested: { b: true, a: false }, a: 2 });
  const right = canonicalStringify({ a: 2, nested: { a: false, b: true }, z: 1 });
  assert.equal(left, right);
  assert.throws(() => canonicalStringify({ value: Number.NaN }), /must be finite/u);
});
