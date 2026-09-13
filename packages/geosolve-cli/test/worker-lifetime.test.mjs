// SPDX-License-Identifier: GPL-3.0-or-later
import assert from "node:assert/strict";
import { getEventListeners } from "node:events";
import { setTimeout as delay } from "node:timers/promises";
import test from "node:test";
import { createWorkerLifetime, createWorkerRequest } from "../runtime/worker-lifetime.mjs";

const workerUrl = source => new URL(`data:text/javascript,${encodeURIComponent(source)}`);

test("retirement fences already queued messages before a replacement reuses a request identity", { timeout: 10000 }, async t => {
  const received = [], stopped = [], ready = createWorkerRequest();
  let replacement;
  const first = createWorkerLifetime({
    url: workerUrl(`import { parentPort } from "node:worker_threads";
      parentPort.on("message", message => {
        parentPort.postMessage({ ...message, value: "first" });
        parentPort.postMessage({ ...message, value: "stale" });
        while (true) {}
      });`),
    onMessage(message) {
      received.push(message);
      stopped.push(first.stop());
      assert.equal(first.stop(), stopped[0], "repeated stop shares actual termination");
      assert.throws(() => first.post({ id: 1 }), /retired/u);
      replacement = createWorkerLifetime({
        url: workerUrl(`import { parentPort } from "node:worker_threads";
          parentPort.on("message", message => parentPort.postMessage({ ...message, value: "replacement" }));`),
        onMessage(value) { received.push(value); ready.resolve(); },
        onError: ready.reject,
        onExit: () => ready.reject(Error("Unexpected replacement exit")),
      });
      replacement.post({ id: 1 });
    },
    onError: ready.reject,
    onExit: () => ready.reject(Error("Retired worker forwarded its exit")),
  });
  t.after(async () => { await first.stop(); await replacement?.stop(); });
  first.post({ id: 1 });
  await ready.promise;
  await stopped[0];
  assert.deepEqual(received, [{ id: 1, value: "first" }, { id: 1, value: "replacement" }]);
});

test("natural exit remains observable and closes the generation mailbox", { timeout: 10000 }, async () => {
  const exitCodes = [], failures = [];
  const worker = createWorkerLifetime({ url: workerUrl("process.exit(7);"),
    onMessage: assert.fail, onError: error => failures.push(error), onExit: code => exitCodes.push(code) });
  assert.equal(await worker.exited, 7);
  assert.deepEqual(exitCodes, [7]);
  assert.deepEqual(failures, []);
  assert.throws(() => worker.post({}), /retired/u);
  await worker.stop();
});

test("settlement removes guards and ignores competing terminal events", async () => {
  const controller = new AbortController(), request = createWorkerRequest();
  let guards = 0;
  request.watch({ signal: controller.signal, onAbort: () => guards++, timeoutMs: 5, onTimeout: () => guards++ });
  assert.equal(getEventListeners(controller.signal, "abort").length, 1);
  request.resolve("accepted");
  request.reject(Error("late failure"));
  request.watch({ signal: controller.signal, onAbort: () => guards++, timeoutMs: 5, onTimeout: () => guards++ });
  controller.abort();
  await delay(15);
  assert.equal(await request.promise, "accepted");
  assert.equal(getEventListeners(controller.signal, "abort").length, 0);
  assert.equal(guards, 0);
});

test("clearing cancellation guards leaves settlement with the supervisor's termination boundary", async () => {
  const controller = new AbortController(), request = createWorkerRequest();
  let completed = false, guards = 0;
  const outcome = request.promise.catch(error => { completed = true; return error.message; });
  request.watch({ signal: controller.signal, onAbort: () => guards++, timeoutMs: 5, onTimeout: () => guards++ });
  request.clear();
  controller.abort();
  await delay(15);
  assert.equal(guards, 0);
  assert.equal(completed, false, "worker and durable callbacks may still be draining");
  request.reject(Error("worker has stopped"));
  assert.equal(await outcome, "worker has stopped");
});

test("an abort before guard installation is handled after request identity is installed", async () => {
  const controller = new AbortController(), request = createWorkerRequest();
  controller.abort();
  let active = request;
  request.watch({ signal: controller.signal, onAbort() {
    assert.equal(active, request);
    active = undefined;
    request.reject(Error("cancelled"));
  } });
  await assert.rejects(request.promise, /cancelled/u);
  assert.equal(active, undefined);
  assert.equal(getEventListeners(controller.signal, "abort").length, 0);
});
