// SPDX-License-Identifier: GPL-3.0-or-later
import { Worker } from "node:worker_threads";

/** A single ordered Rust workbench in a terminable worker, independent of HTTP liveness. */
export async function createWorkspaceWorkbench({ timeoutMs = 120000, workerUrl = new URL("./workspace-workbench-worker.mjs", import.meta.url) } = {}) {
  if (!Number.isSafeInteger(timeoutMs) || timeoutMs < 1 || timeoutMs > 300000) throw Error("Workbench timeout must be 1..300000 ms");
  let worker;
  let sequence = 0;
  let disposed = false;
  let lastCheckpoint;
  let recovery;
  const pending = new Map();
  function fail(owner, error) {
    if (disposed || worker !== owner) return;
    const reconstructing = [...pending.values()].some((request) => request.method === "construct");
    for (const request of pending.values()) { clearTimeout(request.timer); request.reject(error); }
    pending.clear();
    // Detach before termination: the old exit event cannot reject new requests.
    worker = undefined;
    void owner.terminate();
    start();
    recovery = reconstructing ? Promise.reject(error)
      : send("construct", { version: 2, ...(lastCheckpoint ? { persistedProject: lastCheckpoint } : {}) });
    void recovery.catch(() => {});
  }
  function start() {
    worker = new Worker(workerUrl, {
      execArgv: [], stdout: true, stderr: true, resourceLimits: { maxOldGenerationSizeMb: 512 },
    });
    const owner = worker;
    worker.stdout.resume(); worker.stderr.resume();
    worker.on("message", (message) => {
      if (worker !== owner) return;
      const request = pending.get(message.id);
      if (!request) return;
      pending.delete(message.id); clearTimeout(request.timer);
      if (message.ok) request.resolve(message.result); else request.reject(Error(message.error));
    });
    worker.once("error", (error) => fail(owner, error));
    worker.once("exit", (code) => fail(owner, Error(`Workbench worker exited (${code}); last saved design retained`)));
  }
  function send(method, input) {
    if (disposed) return Promise.reject(Error("Folder workbench is disposed"));
    const id = ++sequence;
    const owner = worker;
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => fail(owner, Error(`Workbench operation exceeded ${timeoutMs} ms; last saved design retained`)), timeoutMs);
      pending.set(id, { resolve, reject, timer, method });
      try { owner.postMessage({ id, method, input }); }
      catch (error) { fail(owner, error); }
    });
  }
  start();
  const call = async (method, input) => {
      if (disposed) throw Error("Folder workbench is disposed");
      if (recovery) {
        const restoring = recovery;
        // An explicit reconstruction can recover even if the old checkpoint failed.
        try { await restoring; } catch (error) { if (method !== "construct") throw error; }
        if (recovery === restoring) recovery = undefined;
      }
      const result = await send(method, input);
      if (method === "persistProject") lastCheckpoint = result.contents;
      if (method === "construct") lastCheckpoint = input.persistedProject;
      if (method === "dispatch" && input.command === "workspace.checkpoint.restore") lastCheckpoint = input.payload.contents;
      return result;
  };
  const adapter = Object.fromEntries(["construct", "snapshot", "toolCatalog", "dispatch", "pointer", "wheel", "wheelBatch", "resize", "cancel", "exportProject", "exportWorkspaceDesign", "persistProject", "exportReproduction", "exportInteractionTrace", "bakeProfile"].map((method) => [method, (input) => call(method, input)]));
  adapter.dispose = async () => {
    if (disposed) return;
    disposed = true;
    for (const request of pending.values()) { clearTimeout(request.timer); request.reject(Error("Folder workbench disposed")); }
    pending.clear();
    await worker.terminate();
  };
  return adapter;
}
