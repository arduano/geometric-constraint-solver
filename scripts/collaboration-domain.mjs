// SPDX-License-Identifier: GPL-3.0-or-later
import { Worker } from "node:worker_threads";

export class CollaborationDomainError extends Error {
  constructor(code, message, location = {}) { super(message); this.name = "CollaborationDomainError"; this.code = code; Object.assign(this, location); }
}

/** Trusted host jobs only. Compilation and native evaluation never run on this
 * caller thread. Worker termination bounds CPU work, not untrusted-code access.
 */
export function runCollaborationDomainJob(input, { timeoutMs = 60_000, signal } = {}) {
  if (!Number.isSafeInteger(timeoutMs) || timeoutMs < 1 || timeoutMs > 300_000) throw new TypeError("Domain timeout must be 1..300000 ms");
  if (signal?.aborted) return Promise.reject(new CollaborationDomainError("cancelled", "Domain job cancelled"));
  let encoded;
  try {
    encoded = JSON.stringify(input, (_key, value) => {
      if (typeof value === "number" && !Number.isFinite(value) || ["function", "symbol", "bigint"].includes(typeof value)) throw Error("Domain input must be finite JSON data");
      return value;
    });
    if (!encoded || Buffer.byteLength(encoded) > 128 * 1024 * 1024) throw Error("Domain job input exceeds 128 MiB");
  } catch (error) { return Promise.reject(new CollaborationDomainError("invalid_input", error.message)); }
  return new Promise((resolve, reject) => {
    const worker = new Worker(new URL("./collaboration-domain-worker.mjs", import.meta.url), { workerData: { encoded, timeoutMs }, execArgv: [], stdout: true, stderr: true, resourceLimits: { maxOldGenerationSizeMb: 512 } });
    let settled = false, outputBytes = 0;
    const finish = (error, result) => {
      if (settled) return; settled = true; clearTimeout(timer); signal?.removeEventListener("abort", abort);
      // Wait for termination so an aborted job cannot retain nested compiler CPU.
      void worker.terminate().then(() => error ? reject(error) : resolve(result), reject);
    };
    const abort = () => finish(new CollaborationDomainError("cancelled", "Domain job cancelled"));
    const timer = setTimeout(() => finish(new CollaborationDomainError("timeout", `Domain job exceeded ${timeoutMs} ms`)), timeoutMs);
    for (const pipe of [worker.stdout, worker.stderr]) pipe.on("data", (chunk) => {
      outputBytes += chunk.length;
      if (outputBytes > 1024 * 1024) finish(new CollaborationDomainError("resource_limit", "Domain diagnostic output exceeds 1 MiB"));
    });
    signal?.addEventListener("abort", abort, { once: true });
    if (signal?.aborted) abort();
    worker.once("message", (message) => message.ok ? finish(null, message.result) : finish(new CollaborationDomainError(message.error.code, message.error.message, message.error.location)));
    worker.once("error", (error) => finish(new CollaborationDomainError("worker_failed", error.message)));
    worker.once("exit", (code) => { if (!settled) finish(new CollaborationDomainError("worker_failed", `Domain worker exited before completion (${code})`)); });
  });
}
