// SPDX-License-Identifier: GPL-3.0-or-later

import assert from "node:assert/strict";
import test from "node:test";

import {
  CodeControlClient,
  CodeControlRpcProtocolError,
  MAX_CODE_CONTROL_RPC_MUTATION_RECEIPT_BYTES,
  MAX_CODE_CONTROL_RPC_REQUEST_BYTES,
  encodeCodeControlRpcRequest,
} from "../src/control.js";
import type {
  CodeControlRpcTransport,
  CodeSessionIdentity,
  ManagedControlManifest,
  ManagedValue,
} from "../src/control.js";

const asynchronousTransportTypeCheck: CodeControlRpcTransport = {
  apply: async () => "{}",
};
void asynchronousTransportTypeCheck;

const synchronousTransportTypeCheck: CodeControlRpcTransport = {
  // @ts-expect-error The clean control boundary is asynchronous-only.
  apply: () => "{}",
};
void synchronousTransportTypeCheck;

function codeControlFixture() {
  const identity: CodeSessionIdentity = {
    session: 7,
    revision: 3,
    digest: "a".repeat(64),
  };
  const expected: ManagedValue = {
    kind: "unit",
    value: { unit: "mm", value: 4 },
  };
  const token = {
    id: "b".repeat(64),
    project: "typed-panel",
    project_digest: "c".repeat(64),
    source_digest: "d".repeat(64),
    declaration: "cornerFillets",
    path: ["radius"],
    expected,
    generation_digest: "e".repeat(64),
    authentication: "f".repeat(64),
  } as const;
  const manifest: ManagedControlManifest = {
    project: "typed-panel",
    project_digest: token.project_digest,
    source_digest: token.source_digest,
    expansion_digest: "1".repeat(64),
    controls: [{
      id: token.id,
      source: {
        declaration: token.declaration,
        path: token.path,
        kind: "literal",
        span: { start: 120, end: 125 },
        source_text: "mm(4)",
      },
      value: expected,
      schema: {
        kind: "unit",
        unit: "mm",
        number: "real",
        minimum: { value: 0, inclusive: false },
        maximum: null,
      },
      consumers: [{
        target: {
          target: "generated",
          address: {
            invocation: "cornerFillets",
            template: ["fillet"],
            member_key: ["upperLeft"],
            output: ["field:arc"],
          },
          identity: { allocation: 11, generation: 0 },
          artifact_digest: "2".repeat(64),
          family: "computed.fillet",
        },
        property: ["radius"],
      }],
      access: { access: "editable", token },
    }],
  };
  return { identity, manifest, token };
}
test("code-control client inspects, edits, and moves authenticated outer history", async () => {
  const fixture = codeControlFixture();
  const after: CodeSessionIdentity = {
    session: fixture.identity.session,
    revision: fixture.identity.revision + 1,
    digest: "3".repeat(64),
  };
  const requests: unknown[] = [];
  const client = new CodeControlClient({
    apply: async (encoded) => {
      const request = JSON.parse(encoded) as { method: string };
      requests.push(request);
      switch (request.method) {
        case "inspect_managed_controls":
          return JSON.stringify({
            outcome: "success",
            value: {
              result: "managed_controls",
              snapshot: {
                identity: fixture.identity,
                manifest: fixture.manifest,
                can_undo: false,
                can_redo: false,
              },
            },
          });
        case "edit_managed_controls":
          return JSON.stringify({
            outcome: "success",
            value: {
              result: "managed_control_edit",
              receipt: {
                receipt: {
                  before: fixture.identity,
                  after,
                  label: "Apply managed source",
                  retained_failure: false,
                },
                diagnostic: null,
              },
            },
          });
        case "undo":
          return JSON.stringify({
            outcome: "success",
            value: {
              result: "history",
              receipt: {
                identity: fixture.identity,
                moved: true,
                receipt: {
                  before: after,
                  after: fixture.identity,
                  label: "Undo Apply managed source",
                  retained_failure: false,
                },
              },
            },
          });
        case "redo":
          return JSON.stringify({
            outcome: "success",
            value: {
              result: "history",
              receipt: {
                identity: after,
                moved: true,
                receipt: {
                  before: fixture.identity,
                  after,
                  label: "Redo Apply managed source",
                  retained_failure: false,
                },
              },
            },
          });
        default:
          throw new Error(`unexpected method ${request.method}`);
      }
    },
  });

  const inspected = await client.inspect();
  assert.equal(inspected.outcome, "success");
  if (inspected.outcome !== "success") return;
  const edited = await client.edit(inspected.value.snapshot, {
    edits: [{
      token: fixture.token,
      value: { kind: "unit", value: { unit: "mm", value: 2 } },
    }],
  });
  assert.equal(edited.outcome, "success");
  assert.equal(
    edited.outcome === "success" && edited.value.receipt.receipt.after.digest,
    after.digest,
  );
  assert.equal((await client.undo(after)).outcome, "success");
  assert.equal((await client.redo(fixture.identity)).outcome, "success");
  assert.deepEqual(
    requests.map((request) => (request as { method: string }).method),
    ["inspect_managed_controls", "edit_managed_controls", "undo", "redo"],
  );
  const editRequest = requests[1] as Record<string, unknown>;
  assert.deepEqual(Object.keys(editRequest), ["method", "expected", "batch"]);
});

test("code-control encoder preserves signed zero and rejects loose or unsafe authority", () => {
  const fixture = codeControlFixture();
  const encoded = encodeCodeControlRpcRequest({
    method: "edit_managed_controls",
    expected: fixture.identity,
    batch: {
      edits: [{ token: fixture.token, value: { kind: "number", value: -0 } }],
    },
  });
  const parsed = JSON.parse(encoded) as {
    batch: { edits: [{ value: { value: number } }] };
  };
  assert.ok(Object.is(parsed.batch.edits[0].value.value, -0));

  assert.throws(
    () => encodeCodeControlRpcRequest({
      method: "undo",
      expected: {
        ...fixture.identity,
        revision: Number.MAX_SAFE_INTEGER + 1,
      },
    }),
    /exactly represented unsigned integer/u,
  );
  assert.throws(
    () => encodeCodeControlRpcRequest({
      method: "inspect_managed_controls",
      browser_only: true,
    } as never),
    /unknown or missing fields/u,
  );
  assert.throws(
    () => encodeCodeControlRpcRequest({
      method: "edit_managed_controls",
      expected: fixture.identity,
      batch: {
        edits: [
          { token: fixture.token, value: { kind: "number", value: 1 } },
          { token: fixture.token, value: { kind: "number", value: 2 } },
        ],
      },
    }),
    /repeats control/u,
  );
  assert.throws(
    () => encodeCodeControlRpcRequest({
      method: "edit_managed_controls",
      expected: fixture.identity,
      batch: {
        edits: [{
          token: fixture.token,
          value: { kind: "string", value: "x".repeat(MAX_CODE_CONTROL_RPC_REQUEST_BYTES) },
        }],
      },
    }),
    /request exceeds/u,
  );
});

test("code-control token authentication is independent of DTO property order", async () => {
  const fixture = codeControlFixture();
  const reorderedToken = Object.fromEntries(
    Object.entries(fixture.token).reverse(),
  ) as unknown as typeof fixture.token;
  let called = false;
  const response = await new CodeControlClient({
    apply: async () => {
      called = true;
      return JSON.stringify({
        outcome: "failure",
        failure: {
          code: "control_edit_rejected",
          message: "server-side test stop",
          identity: fixture.identity,
        },
      });
    },
  }).edit(
    { identity: fixture.identity, manifest: fixture.manifest },
    {
      edits: [{
        token: reorderedToken,
        value: { kind: "unit", value: { unit: "mm", value: 2 } },
      }],
    },
  );
  assert.ok(called);
  assert.equal(response.outcome, "failure");
});

test("managed object decoding preserves __proto__ as an ordinary own field", async () => {
  const fixture = codeControlFixture();
  const manifest = JSON.parse(JSON.stringify(fixture.manifest)) as {
    controls: [{ value: ManagedValue; access: { token: { expected: ManagedValue } } }];
  };
  const protoValue = JSON.parse(
    '{"kind":"object","value":{"__proto__":{"kind":"number","value":7}}}',
  ) as ManagedValue;
  manifest.controls[0].value = protoValue;
  manifest.controls[0].access.token.expected = protoValue;
  const response = await new CodeControlClient({
    apply: async () => JSON.stringify({
      outcome: "success",
      value: {
        result: "managed_controls",
        snapshot: {
          identity: fixture.identity,
          manifest,
          can_undo: false,
          can_redo: false,
        },
      },
    }),
  }).inspect();
  assert.equal(response.outcome, "success");
  if (response.outcome !== "success") return;
  const value = response.value.snapshot.manifest.controls[0]?.value;
  assert.equal(value?.kind, "object");
  if (value?.kind !== "object") return;
  assert.ok(Object.hasOwn(value.value, "__proto__"));
  assert.deepEqual(Object.keys(value.value), ["__proto__"]);
  assert.equal(Object.getPrototypeOf(value.value), Object.prototype);
});

test("code-control mutation receipts are bound to the requested outer identity", async () => {
  const fixture = codeControlFixture();
  const foreignBefore = {
    ...fixture.identity,
    revision: fixture.identity.revision + 6,
    digest: "6".repeat(64),
  };
  const foreignAfter = {
    ...foreignBefore,
    revision: foreignBefore.revision + 1,
    digest: "7".repeat(64),
  };
  const batch = {
    edits: [{
      token: fixture.token,
      value: { kind: "unit", value: { unit: "mm", value: 2 } },
    }],
  } as const;
  await assert.rejects(
    () => new CodeControlClient({
      apply: async () => JSON.stringify({
        outcome: "success",
        value: {
          result: "managed_control_edit",
          receipt: {
            receipt: {
              before: foreignBefore,
              after: foreignAfter,
              label: "Foreign edit",
              retained_failure: false,
            },
            diagnostic: null,
          },
        },
      }),
    }).edit({ identity: fixture.identity, manifest: fixture.manifest }, batch),
    /does not start at the requested code-session identity/u,
  );
  await assert.rejects(
    () => new CodeControlClient({
      apply: async () => JSON.stringify({
        outcome: "success",
        value: {
          result: "history",
          receipt: {
            identity: foreignAfter,
            moved: true,
            receipt: {
              before: foreignBefore,
              after: foreignAfter,
              label: "Foreign Undo",
              retained_failure: false,
            },
          },
        },
      }),
    }).undo(fixture.identity),
    /does not start at the requested code-session identity/u,
  );
  const noOp = await new CodeControlClient({
    apply: async () => JSON.stringify({
      outcome: "success",
      value: {
        result: "history",
        receipt: { identity: fixture.identity, moved: false, receipt: null },
      },
    }),
  }).redo(fixture.identity);
  assert.equal(noOp.outcome, "success");
});

test("code-control client rejects malformed, method-confused, and unsafe responses", async () => {
  const fixture = codeControlFixture();
  const invoke = async (response: string) => {
    const client = new CodeControlClient({ apply: async () => response });
    await client.inspect();
  };

  await assert.rejects(() => invoke("not json"), CodeControlRpcProtocolError);
  await assert.rejects(
    () => invoke(JSON.stringify({
      outcome: "failure",
      failure: { code: "browser_guess", message: "unknown", identity: null },
    })),
    /failure code has unknown value/u,
  );
  await assert.rejects(
    () => invoke(JSON.stringify({
      outcome: "success",
      value: {
        result: "history",
        receipt: { identity: fixture.identity, moved: false, receipt: null },
      },
    })),
    /expected result managed_controls/u,
  );
  await assert.rejects(
    () => invoke(JSON.stringify({
      outcome: "success",
      value: {
        result: "managed_controls",
        snapshot: {
          identity: { ...fixture.identity, session: Number.MAX_SAFE_INTEGER + 1 },
          manifest: fixture.manifest,
          can_undo: false,
          can_redo: false,
        },
      },
    })),
    /exactly represented unsigned integer/u,
  );
  await assert.rejects(
    () => new CodeControlClient({
      apply: async () => " ".repeat(MAX_CODE_CONTROL_RPC_MUTATION_RECEIPT_BYTES + 1),
    }).undo(fixture.identity),
    /response exceeds/u,
  );

  const failure = await new CodeControlClient({
    apply: async () => JSON.stringify({
      outcome: "failure",
      failure: {
        code: "control_edit_rejected",
        message: "stale token",
        identity: fixture.identity,
      },
    }),
  }).inspect();
  assert.equal(failure.outcome, "failure");
  assert.equal(
    failure.outcome === "failure" && failure.failure.code,
    "control_edit_rejected",
  );
});
