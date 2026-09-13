// SPDX-License-Identifier: GPL-3.0-or-later
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import { setTimeout as delay } from "node:timers/promises";
import { Worker } from "node:worker_threads";
import { createWorkspaceWorkbench } from "../packages/geosolve-cli/runtime/workspace-workbench.mjs";

const timeoutMs = 1500;
const source = readFileSync(new URL("../examples/file-workspace/sketch.ts", import.meta.url), "utf8");

async function fixture(t) {
  const workers = new Set();
  const drop = new Set();
  const dropped = [];
  const postMessage = Worker.prototype.postMessage;
  let loseResponses = 0;
  let faultOwner;
  t.mock.method(Worker.prototype, "postMessage", function (message, ...rest) {
    if (!workers.has(this)) {
      workers.add(this);
      // Inject loss after the real worker executes the real native request. No
      // substitute adapter, worker program or checkpoint logic is involved.
      const emit = this.emit;
      this.emit = function (event, ...args) {
        if (event === "message" && drop.delete(args[0].id)) {
          dropped.push(args[0]);
          return true;
        }
        return emit.call(this, event, ...args);
      };
    }
    if (this === faultOwner && loseResponses > 0) {
      loseResponses -= 1;
      drop.add(message.id);
    }
    return postMessage.call(this, message, ...rest);
  });
  const actor = await createWorkspaceWorkbench({ timeoutMs });
  t.after(() => actor.dispose());
  await actor.construct({ version: 2 });
  await actor.dispatch({ version: 2, command: "project.new-code" });
  const accepted = await actor.dispatch({ version: 2, command: "source.prepare", payload: {
    path: "sketch.ts", contents: source,
  } });
  assert.equal(accepted.format, "geosolve-folder-model-v1");
  assert.equal(accepted.status, "accepted");
  const checkpoint = (await actor.persistProject()).contents;
  // Native profile envelopes identify their evaluation, including history basis.
  // Establish the saved checkpoint's evaluation before comparing full exports
  // across worker replacement; source and native geometry must also stay exact.
  const saved = await actor.construct({ version: 2, persistedProject: checkpoint });
  assert.equal(saved.status, "accepted");
  assert.deepEqual(saved.source, accepted.source);
  assert.deepEqual(saved.result.geometry, accepted.result.geometry);
  assert.equal((await actor.persistProject()).contents, checkpoint);
  const profiles = await actor.bakeProfile(0.02);
  assert.deepEqual(profiles.evaluation, {
    result_id: saved.result.result_id, input_digest: saved.result.input_digest,
  });
  assert.equal(profiles.regions.length, 1);
  assert.ok(profiles.regions[0].outer.length > 8);
  assert.ok(profiles.regions[0].outer.every(([x, y]) => Number.isFinite(x) && Number.isFinite(y) && Math.abs(Math.hypot(x, y) - 10) < 1e-7));
  const assertRestored = async () => {
    const restored = await actor.snapshot();
    assert.equal(restored.status, "accepted");
    assert.deepEqual(restored.source, accepted.source);
    assert.equal((await actor.persistProject()).contents, checkpoint);
    assert.deepEqual(await actor.bakeProfile(0.02), profiles);
  };
  return { actor, workers, dropped, checkpoint, assertRestored, lose: (count) => {
    faultOwner = [...workers].at(-1);
    loseResponses = count;
  } };
}

test("actor restores its exact saved native project after a lost mutation response", { timeout: 15000 }, async (t) => {
  const f = await fixture(t);
  const changed = source.replace("value: mm(10)", "value: mm(12)");
  assert.notEqual(changed, source);
  f.lose(1);
  await assert.rejects(f.actor.dispatch({ version: 2, command: "source.prepare", payload: {
    path: "sketch.ts", contents: changed,
  } }), /Workbench operation exceeded/);
  assert.equal(f.dropped.length, 1);
  assert.equal(f.dropped[0].ok, true, "the native mutation actually completed before its response was lost");
  assert.equal(f.dropped[0].result.status, "accepted");
  assert.match(f.dropped[0].result.source.files[0].contents, /value: mm\(12\)/);
  await f.assertRestored();
  assert.equal(f.workers.size, 2, "one replacement worker restores the saved project");
});

test("actor replaces a crashed worker before the next request and retains its saved project", { timeout: 15000 }, async (t) => {
  const f = await fixture(t);
  const original = [...f.workers][0];
  assert.equal(await original.terminate(), 1, "inject a real unexpected worker exit");
  await f.assertRestored();
  assert.equal(f.workers.size, 2);
});

test("simultaneous actor timeouts share one recovery and cannot strand its checkpoint restore", { timeout: 15000 }, async (t) => {
  const f = await fixture(t);
  f.lose(2);
  const results = await Promise.allSettled([f.actor.snapshot(), f.actor.snapshot()]);
  assert.equal(results[0].status, "rejected");
  // The owner may serialize calls and run the second request after recovery, or
  // reject both outstanding requests together when retiring their worker.
  assert.ok(f.dropped.length >= 1 && f.dropped.length <= 2);
  assert.ok(f.dropped.every((message) => message.ok));
  // A timer orphaned by the second retirement must not destroy the replacement
  // after it has already restored the design.
  await delay(timeoutMs + 100);
  await f.assertRestored();
  assert.equal(f.workers.size, 2, "all pending calls belong to the same retired worker generation");
});

test("a timed out construct permits an explicit retry with the exact saved checkpoint", { timeout: 15000 }, async (t) => {
  const f = await fixture(t);
  f.lose(1);
  await assert.rejects(f.actor.construct({ version: 2, persistedProject: f.checkpoint }), /Workbench operation exceeded/);
  assert.equal(f.dropped.length, 1);
  assert.equal(f.dropped[0].ok, true);
  await f.actor.construct({ version: 2, persistedProject: f.checkpoint });
  await f.assertRestored();
});
