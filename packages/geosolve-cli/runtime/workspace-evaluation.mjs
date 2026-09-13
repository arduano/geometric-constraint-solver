// SPDX-License-Identifier: GPL-3.0-or-later
import { runWorkerTask } from "./worker-task.mjs";
import { evaluateWorkspaceSnapshot, WorkspaceLoadError } from "./workspace-loader.mjs";

/** Bound both trusted TypeScript execution and native solving/export in terminable workers. */
export async function evaluateProjectSnapshot(snapshot, { profiles, signal, timeoutMs = 120000 } = {}) {
  if (!Number.isSafeInteger(timeoutMs) || timeoutMs < 1 || timeoutMs > 300000) throw Error("Evaluation timeout must be 1..300000 ms");
  const compiled = await evaluateWorkspaceSnapshot(snapshot, { signal, timeoutMs: Math.min(timeoutMs, 15000) });
  if (signal?.aborted) throw new WorkspaceLoadError("cancelled", "Project evaluation cancelled");
  return runWorkerTask({
    url: new URL("./workspace-evaluation-worker.mjs", import.meta.url),
    workerData: { compiled, design: snapshot.files.find((file) => file.path === ".geosolve/design.json")?.contents, profiles },
    memoryMb: 512, signal, timeoutMs,
    decode(message) {
      if (!message.ok) throw new WorkspaceLoadError("native_rejected", message.error);
      return message.result;
    },
    failure(kind, detail) {
      if (kind === "cancelled") return new WorkspaceLoadError("cancelled", "Project evaluation cancelled");
      if (kind === "timeout") return new WorkspaceLoadError("timeout", `Native evaluation exceeded ${timeoutMs} ms`);
      return kind === "error" ? detail : new WorkspaceLoadError("native_rejected", `Native worker exited before completion (${detail})`);
    },
  });
}
