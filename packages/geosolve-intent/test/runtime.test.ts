// SPDX-License-Identifier: GPL-3.0-or-later

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

import {
  IntentClient,
  IntentRpcProtocolError,
  MAX_INTENT_RPC_MUTATION_RECEIPT_BYTES,
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

const emptyDescriptor = {
  schema: {
    inputs: [],
    input_choices: [],
    fields: [],
    minimum_children: 0,
    maximum_children: 0,
  },
  inputs: [],
  fields: [],
  outputs: [],
  suppression_edit: "definition",
  input_edit: "input_binding",
  name_edit: "organization",
};

function rpcSnapshot(identity = fixture.expected) {
  const sourceText = [
    'import type { IntentSourceSnapshot } from "@geosolve/intent";',
    "",
    "export const sketch = {",
    "  cells: [],",
    "} satisfies IntentSourceSnapshot;",
    "",
  ].join("\n");
  return {
    identity,
    graph: {
      identity,
      cells: [] as unknown[],
    },
    projection: {
      identity,
      outline: [],
      structured_source: {
        identity,
        text: sourceText,
        tokens: [],
      },
      history: {
        applied: [],
        redoable: [],
      },
      latest_disposition: null,
      latest_diagnostic: null,
    },
    accepted_validation: null,
  };
}

function rpcSuccess(value: unknown): string {
  return JSON.stringify({ outcome: "success", value });
}

function patchSuccess(identity = fixture.expected): string {
  return rpcSuccess({
    result: "patch",
    receipt: {
      identity,
      disposition: "accepted",
      aliases: { nodes: {}, ports: {}, cells: {} },
    },
  });
}

function rpcInspectorProjection(options: {
  readonly node?: string;
  readonly identity?: typeof fixture.expected;
  readonly inputs?: readonly unknown[];
  readonly descriptor?: unknown;
} = {}) {
  const descriptor = options.descriptor ?? {
    ...emptyDescriptor,
    inputs: [{ slot: "point:0000", path: ["start"] }],
  };
  return {
    identity: options.identity ?? fixture.expected,
    node: options.node ?? "0000000000000042",
    symbol: "segment.main",
    name: "Main segment",
    kind: { family: "geometry", recipe: "segment" },
    suppressed: false,
    retained_failure: false,
    inputs: options.inputs ?? [{
      path: ["start"],
      source: {
        declaration: "source.point",
        output: ["point"],
        kind: "point",
      },
    }],
    descriptor,
    fields: [],
  };
}

function inspectorClient(inspector: unknown, identity = fixture.expected) {
  return new IntentClient(fixture.expected.session, {
    apply: () => rpcSuccess({ result: "inspector", identity, inspector }),
  });
}

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

test("client sends only the closed Rust apply_patch RPC payload and parses its typed result", async () => {
  const calls: string[] = [];
  const value = representativePatch();
  const client = new IntentClient(fixture.expected.session, {
    async apply(json) {
      calls.push(json);
      return patchSuccess();
    },
  });

  const response = await client.apply(value);
  assert.equal(response.outcome, "success");
  if (response.outcome === "success") {
    assert.equal(response.value.result, "patch");
    assert.equal(response.value.receipt.disposition, "accepted");
    assert.equal(response.value.receipt.identity.session, fixture.expected.session);
  }
  assert.deepEqual(calls, [`{"method":"apply_patch","patch":${fixtureText}}`]);
  assert.doesNotMatch(calls[0] ?? "", /residual|jacobian|solve/u);
});

test("mutation methods accept only bounded receipts and never require a full snapshot", async () => {
  const encoded = patchSuccess();
  assert.doesNotMatch(encoded, /"snapshot"/u);
  assert.ok(Buffer.byteLength(encoded, "utf8") < MAX_INTENT_RPC_MUTATION_RECEIPT_BYTES);

  const client = new IntentClient(fixture.expected.session, {
    apply: () => " ".repeat(MAX_INTENT_RPC_MUTATION_RECEIPT_BYTES + 1),
  });
  await assert.rejects(
    () => client.apply(representativePatch()),
    new RegExp(`response exceeds ${MAX_INTENT_RPC_MUTATION_RECEIPT_BYTES} bytes`, "u"),
  );
});

test("mutation receipts accept wide Rust-valid per-declaration port alias maps", async () => {
  const selectorCount = 16_385;
  const encoded = JSON.parse(patchSuccess()) as {
    value: { receipt: { aliases: { ports: Record<string, unknown> } } };
  };
  encoded.value.receipt.aliases.ports.wide = Object.fromEntries(
    Array.from({ length: selectorCount }, (_, index) => [
      `node:span:${index.toString(16).padStart(4, "0")}`,
      {
        node: "0000000000000001",
        port: (index + 2).toString(16).padStart(16, "0"),
        kind: "curve_span",
      },
    ]),
  );
  const client = new IntentClient(fixture.expected.session, {
    apply: () => JSON.stringify(encoded),
  });

  const response = await client.apply(representativePatch());
  assert.equal(response.outcome, "success");
  if (response.outcome === "success") {
    assert.equal(
      Object.keys(response.value.receipt.aliases.ports.wide ?? {}).length,
      selectorCount,
    );
  }
});

test("client stamps source-token edits with the originating exact session identity", async () => {
  const calls: string[] = [];
  const client = new IntentClient(fixture.expected.session, {
    async apply(json) {
      calls.push(json);
      return patchSuccess();
    },
  });
  const expected = sessionIdentity(client.owner, fixture.expected);

  const response = await client.editSourceToken(expected, 7, "3.5");
  assert.equal(response.outcome, "success");
  if (response.outcome === "success") {
    assert.equal(response.value.result, "patch");
  }
  assert.deepEqual(calls, [
    `{"method":"edit_source_token","expected":${JSON.stringify(fixture.expected)},"token":7,"replacement":"3.5"}`,
  ]);
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
    encodeIntentRpcRequest({
      method: "edit_source_token",
      expected: sessionIdentity(session(fixture.expected.session), fixture.expected),
      token: 7,
      replacement: "3.5",
    }),
    `{"method":"edit_source_token","expected":${JSON.stringify(fixture.expected)},"token":7,"replacement":"3.5"}`,
  );
});

test("snapshot parses the compact stable graph projection without bootstrap payload bytes", async () => {
  const snapshot = rpcSnapshot();
  snapshot.graph.cells.push({
    cell: "0000000000000001",
    name: "Bootstrap",
    declarations: [{
      node: "0000000000000002",
      symbol: "bootstrap.point",
      name: "Bootstrap point",
      kind: {
        family: "bootstrap",
        object: {
          kind: "point",
          codec: "sketch-point-v1",
          payload_bytes: 524288,
          payload_sha256: "a".repeat(64),
        },
      },
      bootstrap_origin: null,
      suppressed: false,
      inputs: [],
      definition_fields: [],
      instance_leaves: [],
      operation_outputs: [],
      children: [],
      descriptor: {
        ...emptyDescriptor,
        schema: {
          ...emptyDescriptor.schema,
          fields: [{
            field: "topology_digest",
            literal: { kind: "text" },
            required: false,
          }],
        },
        fields: [{
          schema: {
            field: "topology_digest",
            literal: { kind: "text" },
            required: false,
          },
          path: ["topologyDigest"],
          default: { default: "conditional" },
          choices: { choices: "not_applicable" },
          edit: "definition",
        }],
      },
      dependencies: [],
    }],
  });
  snapshot.graph.cells.push({
    cell: "0000000000000003",
    name: "Stable topology",
    declarations: [{
      node: "0000000000000004",
      symbol: "polyline.main",
      name: "Polyline",
      kind: { family: "geometry", recipe: "polyline" },
      bootstrap_origin: null,
      suppressed: false,
      inputs: [],
      definition_fields: [],
      instance_leaves: [],
      operation_outputs: [],
      children: [{
        child: "0000000000000010",
        schema: "polyline_vertex",
        ports: [{
          node: "0000000000000004",
          port: "0000000000000020",
          kind: "point",
        }],
      }, {
        child: "0000000000000011",
        schema: "polyline_vertex",
        ports: [{
          node: "0000000000000004",
          port: "0000000000000021",
          kind: "point",
        }],
      }],
      descriptor: emptyDescriptor,
      dependencies: [],
    }, {
      node: "0000000000000005",
      symbol: "operation.rectangle",
      name: "Rectangle operation",
      kind: { family: "operation", operation: "rectangle" },
      bootstrap_origin: null,
      suppressed: false,
      inputs: [],
      definition_fields: [],
      instance_leaves: [],
      operation_outputs: [{ kind: "curve", curve_span_count: 2 }],
      children: [],
      descriptor: emptyDescriptor,
      dependencies: [],
    }],
  });
  const client = new IntentClient(fixture.expected.session, {
    apply: () => rpcSuccess({ result: "snapshot", snapshot }),
  });

  const response = await client.snapshot();
  assert.equal(response.outcome, "success");
  if (response.outcome === "success") {
    const declaration = response.value.snapshot.graph.cells[0]?.declarations[0];
    assert.equal(declaration?.kind.family, "bootstrap");
    if (declaration?.kind.family === "bootstrap") {
      assert.equal(declaration.kind.object.payload_bytes, 524288);
      assert.equal("payload" in declaration.kind.object, false);
    }
    assert.equal(
      declaration?.descriptor.fields[0]?.default.default,
      "conditional",
    );
    assert.match(
      response.value.snapshot.projection.structured_source.text,
      /satisfies IntentSourceSnapshot/u,
    );
    const topology = response.value.snapshot.graph.cells[1]?.declarations;
    assert.deepEqual(
      topology?.[0]?.children.map((child) => ({
        child: child.child,
        ports: child.ports.map((port) => port.port),
      })),
      [{ child: "0000000000000010", ports: ["0000000000000020"] },
        { child: "0000000000000011", ports: ["0000000000000021"] }],
    );
    assert.deepEqual(
      topology?.[1]?.operation_outputs,
      [{ kind: "curve", curve_span_count: 2 }],
    );
  }
});

test("snapshot accepts every declaration count admitted by the Rust graph contract", async () => {
  const snapshot = rpcSnapshot();
  const declaration = (index: number) => ({
    node: (index + 2).toString(16).padStart(16, "0"),
    symbol: `point.${index}`,
    name: `Point ${index}`,
    kind: { family: "geometry", recipe: "sketch_point" },
    bootstrap_origin: null,
    suppressed: false,
    inputs: [],
    definition_fields: [],
    instance_leaves: [],
    operation_outputs: [],
    children: [],
    descriptor: emptyDescriptor,
    dependencies: [],
  });
  // This deliberately crosses the former presentation-only limit of 4,096.
  // Rust's closed graph contract admits 65,536 declarations in total.
  snapshot.graph.cells.push({
    cell: "0000000000000001",
    name: "Large graph",
    declarations: Array.from({ length: 4_097 }, (_, index) => declaration(index)),
  });
  const client = new IntentClient(fixture.expected.session, {
    apply: () => rpcSuccess({ result: "snapshot", snapshot }),
  });

  const response = await client.snapshot();
  assert.equal(response.outcome, "success");
  if (response.outcome === "success") {
    assert.equal(response.value.snapshot.graph.cells[0]?.declarations.length, 4_097);
  }
});

test("snapshot accepts operation port catalogs beyond the former UI convenience limit", async () => {
  const snapshot = rpcSnapshot();
  const node = "0000000000000002";
  const outputCount = 16_385;
  snapshot.graph.cells.push({
    cell: "0000000000000001",
    name: "Wide operation",
    declarations: [{
      node,
      symbol: "operation.wide",
      name: "Wide operation",
      kind: { family: "operation", operation: "profile_offset" },
      bootstrap_origin: null,
      suppressed: false,
      inputs: [],
      definition_fields: [],
      instance_leaves: [],
      operation_outputs: [{ kind: "curve", curve_span_count: outputCount }],
      children: [],
      descriptor: {
        ...emptyDescriptor,
        outputs: Array.from({ length: outputCount }, (_, index) => ({
          port: {
            node,
            port: (index + 3).toString(16).padStart(16, "0"),
            kind: "curve_span",
          },
          selector: `node:span:${index.toString(16).padStart(4, "0")}`,
          path: ["spans", index],
          kind: "curve_span",
          writable: [],
          flow: { state: "owned_logical" },
          native: null,
          edit: "read_only",
        })),
      },
      dependencies: [],
    }],
  });
  const client = new IntentClient(fixture.expected.session, {
    apply: () => rpcSuccess({ result: "snapshot", snapshot }),
  });

  const response = await client.snapshot();
  assert.equal(response.outcome, "success");
  if (response.outcome === "success") {
    assert.equal(
      response.value.snapshot.graph.cells[0]?.declarations[0]?.descriptor.outputs.length,
      outputCount,
    );
  }
});

test("snapshot, Undo, Redo, and Inspector enforce method-specific success DTOs", async () => {
  const requests: string[] = [];
  const client = new IntentClient(fixture.expected.session, {
    apply(request) {
      requests.push(request);
      const method = (JSON.parse(request) as { method: string }).method;
      if (method === "snapshot") {
        return rpcSuccess({ result: "snapshot", snapshot: rpcSnapshot() });
      }
      if (method === "undo") {
        return rpcSuccess({
          result: "history",
          receipt: { identity: fixture.expected, moved: true },
        });
      }
      if (method === "redo") {
        return rpcSuccess({
          result: "history",
          receipt: { identity: fixture.expected, moved: false },
        });
      }
      return rpcSuccess({
        result: "inspector",
        identity: fixture.expected,
        inspector: {
          identity: fixture.expected,
          node: "0000000000000042",
          symbol: "point.main",
          name: "Main point",
          kind: { family: "geometry", recipe: "sketch_point" },
          suppressed: false,
          retained_failure: false,
          inputs: [{
            path: ["corners", 0, "parents", 1],
            source: {
              declaration: "segment.main",
              output: ["spans", 0],
              kind: "curve_span",
            },
          }],
          descriptor: {
            ...emptyDescriptor,
            inputs: [{
              slot: "span:0003",
              path: ["corners", 0, "parents", 1],
            }],
          },
          fields: [],
        },
      });
    },
  });

  const snapshot = await client.snapshot();
  const undo = await client.undo();
  const redo = await client.redo();
  const inspector = await client.inspector(
    stableNode(client.owner, "0000000000000042"),
  );

  assert.equal(snapshot.outcome === "success" && snapshot.value.result, "snapshot");
  assert.equal(undo.outcome === "success" && undo.value.receipt.moved, true);
  assert.equal(redo.outcome === "success" && redo.value.receipt.moved, false);
  assert.equal(
    inspector.outcome === "success" && inspector.value.inspector?.symbol,
    "point.main",
  );
  assert.deepEqual(
    inspector.outcome === "success" && inspector.value.inspector?.inputs[0],
    {
      path: ["corners", 0, "parents", 1],
      source: {
        declaration: "segment.main",
        output: ["spans", 0],
        kind: "curve_span",
      },
    },
  );
  assert.deepEqual(requests, [
    '{"method":"snapshot"}',
    '{"method":"undo"}',
    '{"method":"redo"}',
    '{"method":"inspector","node":"0000000000000042"}',
  ]);
});

test("valid Rust failure envelopes remain typed and state-neutral to the caller", async () => {
  const client = new IntentClient(fixture.expected.session, {
    apply: () => JSON.stringify({
      outcome: "failure",
      failure: {
        code: "source_edit_rejected",
        message: "the structured-source projection is stale",
        identity: fixture.expected,
      },
    }),
  });

  const response = await client.editSourceToken(
    sessionIdentity(client.owner, fixture.expected),
    1,
    "2.0",
  );
  assert.equal(response.outcome, "failure");
  if (response.outcome === "failure") {
    assert.equal(response.failure.code, "source_edit_rejected");
    assert.equal(response.failure.identity?.session, fixture.expected.session);
  }
});

test("Inspector rejects malformed semantic projection paths", async (context) => {
  const cases: readonly (readonly [string, unknown, RegExp])[] = [
    ["empty", [], /must not be empty/u],
    ["index first", [0, "point"], /must begin with an object field/u],
    ["negative index", ["points", -1], /exactly represented unsigned integer/u],
    ["fractional index", ["points", 0.5], /exactly represented unsigned integer/u],
    ["out-of-range index", ["points", 0x1_0000], /exactly represented unsigned integer/u],
    ["excessive depth", Array.from({ length: 33 }, () => "nested"), /exceeds 32 entries/u],
    ["invalid field", [" padded"], /not a valid bounded intent key/u],
  ];
  for (const [name, path, expected] of cases) {
    await context.test(name, async () => {
      const projection = rpcInspectorProjection({
        inputs: [{
          path,
          source: {
            declaration: "source.point",
            output: ["point"],
            kind: "point",
          },
        }],
      });
      const client = inspectorClient(projection);
      await assert.rejects(
        () => client.inspector(stableNode(client.owner, "0000000000000042")),
        expected,
      );
    });
  }

  await context.test("malformed projected output", async () => {
    const projection = rpcInspectorProjection({
      inputs: [{
        path: ["start"],
        source: {
          declaration: "source.point",
          output: [],
          kind: "point",
        },
      }],
    });
    const client = inspectorClient(projection);
    await assert.rejects(
      () => client.inspector(stableNode(client.owner, "0000000000000042")),
      /projected output path must not be empty/u,
    );
  });
});

test("Inspector binds the response to the exact requested node and inner identity", async () => {
  const wrongNode = inspectorClient(rpcInspectorProjection({ node: "0000000000000043" }));
  await assert.rejects(
    () => wrongNode.inspector(stableNode(wrongNode.owner, "0000000000000042")),
    /does not match the requested node/u,
  );

  const staleIdentity = {
    ...fixture.expected,
    revision: "ffffffffffffffff",
  };
  const wrongIdentity = inspectorClient(rpcInspectorProjection({ identity: staleIdentity }));
  await assert.rejects(
    () => wrongIdentity.inspector(stableNode(wrongIdentity.owner, "0000000000000042")),
    /projection identity does not match receipt/u,
  );
});

test("descriptor mappings reject duplicate and prefix-colliding semantic coordinates", async (context) => {
  const cases = [
    {
      name: "duplicate canonical input slot",
      inputs: [
        { slot: "point:0000", path: ["start"] },
        { slot: "point:0000", path: ["end"] },
      ],
      expected: /input descriptor slots contain duplicate coordinate/u,
    },
    {
      name: "duplicate input path",
      inputs: [
        { slot: "point:0000", path: ["points", 0] },
        { slot: "point:0001", path: ["points", 0] },
      ],
      expected: /input descriptor paths contain a duplicate path/u,
    },
    {
      name: "prefix-colliding input path",
      inputs: [
        { slot: "point:0000", path: ["corners", 0] },
        { slot: "point:0001", path: ["corners", 0, "position"] },
      ],
      expected: /input descriptor paths contain a prefix collision/u,
    },
  ] as const;
  for (const candidate of cases) {
    await context.test(candidate.name, async () => {
      const inspectorInputs = candidate.inputs.map((input, index) => ({
        path: index === 0 ? ["first"] : ["second"],
        source: {
          declaration: `source.${index}`,
          output: ["point"],
          kind: "point",
        },
      }));
      const projection = rpcInspectorProjection({
        inputs: inspectorInputs,
        descriptor: { ...emptyDescriptor, inputs: candidate.inputs },
      });
      const client = inspectorClient(projection);
      await assert.rejects(
        () => client.inspector(stableNode(client.owner, "0000000000000042")),
        candidate.expected,
      );
    });
  }

  await context.test("prefix-colliding definition paths", async () => {
    const schemas = [
      { field: "corner", literal: { kind: "point" }, required: false },
      { field: "corner_x", literal: { kind: "quantity", unit: "length" }, required: false },
    ];
    const projection = rpcInspectorProjection({
      descriptor: {
        ...emptyDescriptor,
        schema: { ...emptyDescriptor.schema, fields: schemas },
        fields: schemas.map((schema, index) => ({
          schema,
          path: index === 0 ? ["corners", 0] : ["corners", 0, "x"],
          default: { default: "contextual" },
          choices: { choices: "not_applicable" },
          edit: "definition",
        })),
      },
    });
    const client = inspectorClient(projection);
    await assert.rejects(
      () => client.inspector(stableNode(client.owner, "0000000000000042")),
      /definition descriptor paths contain a prefix collision/u,
    );
  });
});

test("output descriptors may name a semantic object and one of its members", async () => {
  const node = "0000000000000042";
  const descriptor = {
    ...emptyDescriptor,
    outputs: [{
      port: { node, port: "0000000000000051", kind: "contact" },
      selector: "node:contact:0000",
      path: ["contact"],
      kind: "contact",
      writable: [],
      flow: { state: "owned_logical" },
      native: null,
      edit: "read_only",
    }, {
      port: { node, port: "0000000000000052", kind: "scalar" },
      selector: "node:parameter:0000",
      path: ["contact", "parameter"],
      kind: "scalar",
      writable: ["parameter"],
      flow: { state: "owned_logical" },
      native: null,
      edit: "instance",
    }],
  };
  const client = inspectorClient(rpcInspectorProjection({ inputs: [], descriptor }));
  const response = await client.inspector(stableNode(client.owner, node));
  assert.equal(response.outcome, "success");
  assert.deepEqual(
    response.outcome === "success"
      && response.value.inspector?.descriptor.outputs.map((output) => output.path),
    [["contact"], ["contact", "parameter"]],
  );
});

test("Inspector inputs correspond one-to-one with descriptor semantic paths", async (context) => {
  const descriptor = {
    ...emptyDescriptor,
    inputs: [
      { slot: "point:0000", path: ["start"] },
      { slot: "point:0001", path: ["end"] },
    ],
  };
  const source = (path: readonly (string | number)[], index: number) => ({
    path,
    source: {
      declaration: `source.${index}`,
      output: ["point"],
      kind: "point",
    },
  });
  const cases = [
    ["missing", [source(["start"], 0)]],
    ["extra", [source(["start"], 0), source(["end"], 1), source(["third"], 2)]],
    ["path mismatch", [source(["start"], 0), source(["finish"], 1)]],
    ["duplicate", [source(["start"], 0), source(["start"], 1)]],
  ] as const;
  for (const [name, inputs] of cases) {
    await context.test(name, async () => {
      const client = inspectorClient(rpcInspectorProjection({ inputs, descriptor }));
      await assert.rejects(
        () => client.inspector(stableNode(client.owner, "0000000000000042")),
        /Inspector input paths contain a duplicate path|Inspector inputs do not match descriptor inputs/u,
      );
    });
  }
});

test("client rejects malformed, unknown-field, method-confused, and cross-session responses", async () => {
  const owner = fixture.expected.session;
  const invoke = async (response: string) => {
    const client = new IntentClient(owner, { apply: () => response });
    await client.snapshot();
  };

  const codeOwned = new IntentClient(owner, {
    apply: () => JSON.stringify({
      outcome: "failure",
      failure: {
        code: "code_authority_required",
        message: "use code controls",
        identity: null,
      },
    }),
  });
  const codeOwnedResponse = await codeOwned.snapshot();
  assert.equal(codeOwnedResponse.outcome, "failure");
  assert.equal(
    codeOwnedResponse.outcome === "failure" && codeOwnedResponse.failure.code,
    "code_authority_required",
  );

  await assert.rejects(() => invoke("not json"), IntentRpcProtocolError);
  await assert.rejects(
    () => invoke(JSON.stringify({
      outcome: "failure",
      failure: { code: "browser_guess", message: "unknown", identity: null },
    })),
    /failure code has unknown value/u,
  );
  await assert.rejects(
    () => invoke(rpcSuccess({
      result: "history",
      receipt: { identity: fixture.expected, moved: false },
    })),
    /expected result snapshot/u,
  );

  const unknownFieldSnapshot = rpcSnapshot();
  const invalidProjection = {
    ...unknownFieldSnapshot.projection,
    browser_only: true,
  };
  await assert.rejects(
    () => invoke(rpcSuccess({
      result: "snapshot",
      snapshot: { ...unknownFieldSnapshot, projection: invalidProjection },
    })),
    /unknown or missing fields/u,
  );

  const foreignIdentity = {
    ...fixture.expected,
    session: "22222222222222222222222222222222",
  };
  await assert.rejects(
    () => invoke(rpcSuccess({ result: "snapshot", snapshot: rpcSnapshot(foreignIdentity) })),
    /cross-session intent identity/u,
  );

  const patchClient = new IntentClient(owner, { apply: () => patchSuccess(foreignIdentity) });
  await assert.rejects(
    () => patchClient.apply(representativePatch()),
    /cross-session intent identity/u,
  );

  const obsoletePatchClient = new IntentClient(owner, {
    apply: () => rpcSuccess({
      result: "patch",
      disposition: "accepted",
      aliases: { nodes: {}, ports: {}, cells: {} },
      snapshot: rpcSnapshot(),
    }),
  });
  await assert.rejects(
    () => obsoletePatchClient.apply(representativePatch()),
    /patch RPC value has unknown or missing fields/u,
  );

  const mismatchedGraph = rpcSnapshot();
  mismatchedGraph.graph.identity = {
    ...fixture.expected,
    revision: "ffffffffffffffff",
  };
  await assert.rejects(
    () => invoke(rpcSuccess({ result: "snapshot", snapshot: mismatchedGraph })),
    /graph identity does not match snapshot identity/u,
  );

  const staleSource = rpcSnapshot();
  staleSource.projection.structured_source.identity = {
    ...fixture.expected,
    revision: "eeeeeeeeeeeeeeee",
  };
  await assert.rejects(
    () => invoke(rpcSuccess({ result: "snapshot", snapshot: staleSource })),
    /Structured Source identity does not match snapshot identity/u,
  );
});

test("client rejects non-finite and unsafe-integer DTO coordinates", async () => {
  const unsafe = rpcSnapshot();
  unsafe.graph.cells.push({
    cell: "0000000000000001",
    name: "Point",
    declarations: [{
      node: "0000000000000002",
      symbol: "point.main",
      name: "Point",
      kind: { family: "geometry", recipe: "sketch_point" },
      bootstrap_origin: null,
      suppressed: false,
      inputs: [],
      definition_fields: [{
        field: "count",
        value: { kind: "natural", value: Number.MAX_SAFE_INTEGER + 1 },
      }],
      instance_leaves: [],
      operation_outputs: [],
      children: [],
      descriptor: emptyDescriptor,
      dependencies: [],
    }],
  });
  const client = new IntentClient(fixture.expected.session, {
    apply: () => rpcSuccess({ result: "snapshot", snapshot: unsafe }),
  });
  await assert.rejects(
    () => client.snapshot(),
    /cannot be represented exactly/u,
  );
});

test("generic canonical JSON remains insertion-order independent and finite-only", () => {
  const left = canonicalStringify({ z: 1, nested: { b: true, a: false }, a: 2 });
  const right = canonicalStringify({ a: 2, nested: { a: false, b: true }, z: 1 });
  assert.equal(left, right);
  assert.throws(() => canonicalStringify({ value: Number.NaN }), /must be finite/u);
});
