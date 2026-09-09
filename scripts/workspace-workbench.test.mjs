// SPDX-License-Identifier: GPL-3.0-or-later
import assert from "node:assert/strict";
import test from "node:test";
import { createWorkspaceWorkbench } from "./workspace-workbench.mjs";

// Real workers exercise process loss independently of native geometry. Native
// persistence validity is owned by the public Rust/session tests.
const workerUrl = new URL(`data:text/javascript,${encodeURIComponent(`
import { parentPort } from "node:worker_threads";
let value = "empty";
parentPort.on("message", ({ id, method, input }) => {
  if (method === "construct") value = input.persistedProject ?? "empty";
  if (method === "dispatch") {
    if (input.crash !== undefined) process.exit(input.crash);
    if (input.throw) throw Error("worker panic");
    if (input.hang) { while (true) {} }
    value = input.command === "workspace.checkpoint.restore" ? input.payload.contents : input.value;
  }
  parentPort.postMessage({ id, ok: true, result: method === "persistProject" ? { contents: value } : { value } });
});`)}`);

for (const [name, input, pattern] of [
  ["normal exit", { crash: 0 }, /exited/],
  ["crash", { crash: 7 }, /exited/],
  ["uncaught error", { throw: true }, /worker panic/],
  ["timeout", { hang: true }, /exceeded/],
]) test(`workbench ${name} rejects the interrupted operation and restores the last checkpoint`, { timeout: 10000 }, async (t) => {
  const adapter = await createWorkspaceWorkbench({ workerUrl, timeoutMs: 1000 });
  t.after(() => adapter.dispose());
  await adapter.construct({ version: 2, persistedProject: "saved" });
  await adapter.dispatch({ value: "uncommitted" });
  await assert.rejects(adapter.dispatch(input), pattern);
  assert.deepEqual(await adapter.snapshot(), { value: "saved" });
  await adapter.dispatch({ value: "next" });
  assert.deepEqual(await adapter.persistProject(), { contents: "next" });
  assert.deepEqual(await adapter.snapshot(), { value: "next" });
});

test("queued calls preserve order and disposal settles every caller", { timeout: 10000 }, async () => {
  const adapter = await createWorkspaceWorkbench({ workerUrl, timeoutMs: 1000 });
  await adapter.construct({ version: 2 });
  const calls = [adapter.dispatch({ value: "first" }), adapter.dispatch({ value: "second" }), adapter.snapshot()];
  assert.deepEqual(await Promise.all(calls), [{ value: "first" }, { value: "second" }, { value: "second" }]);
  const interrupted = adapter.dispatch({ hang: true });
  const queued = adapter.snapshot();
  const finished = Promise.allSettled([interrupted, queued]);
  await adapter.dispose();
  assert.ok((await finished).every((result) => result.status === "rejected"));
});

test("M98-F015 a restored rollback checkpoint remains authoritative after a later worker crash", { timeout: 10000 }, async (t) => {
  const adapter = await createWorkspaceWorkbench({ workerUrl, timeoutMs: 1000 });
  t.after(() => adapter.dispose());
  await adapter.construct({ version: 2, persistedProject: "accepted" });
  await adapter.dispatch({ value: "candidate" });
  await adapter.persistProject();
  await adapter.dispatch({ command: "workspace.checkpoint.restore", payload: { contents: "accepted" } });
  await assert.rejects(adapter.dispatch({ crash: 7 }), /exited/);
  assert.deepEqual(await adapter.snapshot(), { value: "accepted" });
});
