// SPDX-License-Identifier: GPL-3.0-or-later
import { createWorkerLifetime, createWorkerRequest } from "./worker-lifetime.mjs";

export class CollaborationDomainError extends Error {
  constructor(code, message, location = {}) { super(message); this.name = "CollaborationDomainError"; this.code = code; Object.assign(this, location); }
}
const error = (code, message) => new CollaborationDomainError(code, message);
function encode(input) {
  const encoded = JSON.stringify(input, (_key, value) => {
    if (typeof value === "number" && !Number.isFinite(value) || ["function", "symbol", "bigint"].includes(typeof value)) throw Error("Domain input must be finite JSON data");
    return value;
  });
  if (!encoded || Buffer.byteLength(encoded) > 128 * 1024 * 1024) throw Error("Domain job input exceeds 128 MiB");
  return encoded;
}
function checkedTimeout(timeoutMs) {
  if (!Number.isSafeInteger(timeoutMs) || timeoutMs < 1 || timeoutMs > 300_000) throw new TypeError("Domain timeout must be 1..300000 ms");
  return timeoutMs;
}

/** One document's trusted compiler/native jobs. The worker is serial and bounded;
 * authority remains entirely with the caller's durable publication transaction.
 * Terminating an active job also discards every retained candidate in its worker.
 */
export function createCollaborationDomainService({ timeoutMs = 60_000 } = {}) {
  checkedTimeout(timeoutMs);
  let worker, active, disposed = false, sequence = 0, stopping, queuedBytes = 0;
  const queue = [];
  function finish(request, failure, result) {
    failure ? request.reject(failure) : request.resolve(result);
  }
  async function terminate(owner, failure) {
    if (worker !== owner) return;
    worker = undefined;
    const request = active; active = undefined;
    request?.clear();
    const stopped = owner.stop(); stopping = stopped;
    try { await stopped; } finally {
      if (request) finish(request, failure);
      if (stopping === stopped) stopping = undefined;
      pump();
    }
  }
  function start() {
    worker = createWorkerLifetime({
      url: new URL("./collaboration-domain-worker.mjs", import.meta.url),
      workerData: { retained: true, timeoutMs },
      onOutput(chunk) {
        if (!active) return;
        active.outputBytes += chunk.length;
        if (active.outputBytes > 1024 * 1024) void terminate(worker, error("resource_limit", "Domain diagnostic output exceeds 1 MiB"));
      },
      onMessage(message) {
        if (!active || message.id !== active.id) return;
        const request = active; active = undefined;
        finish(request, message.ok ? null : new CollaborationDomainError(message.error.code, message.error.message, message.error.location), message.result);
        pump();
      },
      onError: failure => { void terminate(worker, error("worker_failed", failure.message)); },
      onExit: code => { void terminate(worker, error("worker_failed", `Domain worker exited before completion (${code})`)); },
    });
  }
  function pump() {
    if (disposed || active || stopping || !queue.length) return;
    if (!worker) start();
    const request = queue.shift(); queuedBytes -= request.bytes; active = request;
    const owner = worker;
    request.watch({ timeoutMs: request.timeoutMs, onTimeout: () => { void terminate(owner, error("timeout", `Domain job exceeded ${request.timeoutMs} ms`)); },
      signal: request.signal, onAbort: request.abort });
    if (active !== request) return;
    try { owner.post({ id: request.id, encoded: request.encoded, timeoutMs: request.timeoutMs }); }
    catch (failure) { void terminate(owner, error("worker_failed", failure.message)); }
  }
  return {
    run(input, { signal, timeoutMs: deadline = timeoutMs } = {}) {
      checkedTimeout(deadline);
      if (disposed || signal?.aborted) return Promise.reject(error("cancelled", disposed ? "Domain service disposed" : "Domain job cancelled"));
      let encoded;
      try { encoded = encode(input); } catch (failure) { return Promise.reject(error("invalid_input", failure.message)); }
      const bytes = Buffer.byteLength(encoded);
      if (queue.length >= 16 || queuedBytes + bytes > 128 * 1024 * 1024) return Promise.reject(error("resource_limit", "Domain job queue exceeds 16 waiting requests / 128 MiB"));
      const request = Object.assign(createWorkerRequest(), { id: ++sequence, encoded, bytes, signal, timeoutMs: deadline, outputBytes: 0 });
      request.abort = () => {
        if (active === request) { void terminate(worker, error("cancelled", "Domain job cancelled")); return; }
        const index = queue.indexOf(request);
        if (index >= 0) { queue.splice(index, 1); queuedBytes -= request.bytes; finish(request, error("cancelled", "Domain job cancelled")); }
      };
      queue.push(request); queuedBytes += bytes;
      request.watch({ signal, onAbort: request.abort });
      pump();
      return request.promise;
    },
    async dispose() {
      if (disposed) { await stopping; return; }
      disposed = true;
      for (const request of queue.splice(0)) finish(request, error("cancelled", "Domain service disposed"));
      queuedBytes = 0;
      if (worker) await terminate(worker, error("cancelled", "Domain service disposed"));
      await stopping;
    },
  };
}

/** Ephemeral cold execution remains available for independent rebuild/parity. */
export async function runCollaborationDomainJob(input, options = {}) {
  const service = createCollaborationDomainService(options);
  try { return await service.run(input, options); }
  finally { await service.dispose(); }
}
