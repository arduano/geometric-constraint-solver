// SPDX-License-Identifier: GPL-3.0-or-later
import { Worker } from "node:worker_threads";
import { evaluateWorkspaceSnapshot, WorkspaceLoadError } from "./workspace-loader.mjs";

/** Bound both trusted TypeScript execution and native solving/export in terminable workers. */
export async function evaluateProjectSnapshot(snapshot, { profiles, signal, timeoutMs = 120000 } = {}) {
  if (!Number.isSafeInteger(timeoutMs) || timeoutMs < 1 || timeoutMs > 300000) throw Error("Evaluation timeout must be 1..300000 ms");
  const compiled = await evaluateWorkspaceSnapshot(snapshot, { signal, timeoutMs: Math.min(timeoutMs, 15000) });
  if (signal?.aborted) throw new WorkspaceLoadError("cancelled", "Project evaluation cancelled");
  return new Promise((resolveResult, reject) => {
    const worker = new Worker(new URL("./workspace-evaluation-worker.mjs", import.meta.url), {
      workerData: { compiled, design: snapshot.files.find((file) => file.path === ".geosolve/design.json")?.contents, profiles },
      execArgv: [], stdout: true, stderr: true, resourceLimits: { maxOldGenerationSizeMb: 512 },
    });
    worker.stdout.resume(); worker.stderr.resume();
    let finished = false;
    const done = (error, result) => {
      if (finished) return;
      finished = true;
      clearTimeout(timer); signal?.removeEventListener("abort", abort);
      void worker.terminate();
      if (error) reject(error); else resolveResult(result);
    };
    const abort = () => done(new WorkspaceLoadError("cancelled", "Project evaluation cancelled"));
    const timer = setTimeout(() => done(new WorkspaceLoadError("timeout", `Native evaluation exceeded ${timeoutMs} ms`)), timeoutMs);
    signal?.addEventListener("abort", abort, { once: true });
    worker.once("message", (message) => message.ok ? done(null, message.result) : done(new WorkspaceLoadError("native_rejected", message.error)));
    worker.once("error", (error) => done(error));
    worker.once("exit", (code) => { if (!finished) done(new WorkspaceLoadError("native_rejected", `Native worker exited before completion (${code})`)); });
  });
}
