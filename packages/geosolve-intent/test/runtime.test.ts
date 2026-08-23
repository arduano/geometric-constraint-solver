// SPDX-License-Identifier: GPL-3.0-or-later

import assert from "node:assert/strict";
import test from "node:test";

import {
  IntentClient,
  aliasPort,
  canonicalStringify,
  createNode,
  draft,
  input,
  patch,
  session,
  stablePort,
} from "../src/index.js";

test("builders reject cross-session and wrong-kind references at runtime", () => {
  const first = session("first");
  const second = session("second");
  const foreign = stablePort(second, "node-1", "port-1", "point");
  const curve = stablePort(first, "node-2", "port-2", "curve");

  assert.throws(
    () => input(first, "point", 0, foreign as never),
    /cross-session intent reference/u,
  );
  assert.throws(
    () => input(first, "point", 0, curve as never),
    /requires point, received curve/u,
  );
  assert.throws(() => patch(first, "identity", "require_accepted", []), /at least one/u);
});

test("equivalent object insertion order has canonical transport bytes", () => {
  const left = canonicalStringify({ z: 1, nested: { b: true, a: false }, a: 2 });
  const right = canonicalStringify({ a: 2, nested: { a: false, b: true }, z: 1 });
  assert.equal(left, right);
  assert.throws(() => canonicalStringify({ value: Number.NaN }), /must be finite/u);
});

test("client sends only a closed intent.apply patch payload", async () => {
  const calls: Array<readonly [string, string]> = [];
  const client = new IntentClient("rpc-session", {
    async request(method, json) {
      calls.push([method, json]);
      return "accepted-identity";
    },
  });
  const start = aliasPort(client.owner, "point", "node:primary:0000", "point");
  const segment = draft(client.owner, "segment.main", {
    family: "geometry",
    recipe: "segment",
  }, {
    inputs: [input(client.owner, "point", 0, start)],
  });
  const value = patch(client.owner, "identity-1", "retain_failed_intent", [
    createNode(client.owner, "segment", segment),
  ]);

  assert.equal(await client.apply(value), "accepted-identity");
  assert.equal(calls.length, 1);
  assert.equal(calls[0]?.[0], "intent.apply");
  assert.match(calls[0]?.[1] ?? "", /"recipe":"segment"/u);
  assert.doesNotMatch(calls[0]?.[1] ?? "", /residual|jacobian|solve/u);
});
