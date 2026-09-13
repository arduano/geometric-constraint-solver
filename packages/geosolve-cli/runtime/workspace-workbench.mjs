// SPDX-License-Identifier: GPL-3.0-or-later
import { createWorkerLifetime, createWorkerRequest } from "./worker-lifetime.mjs";

/** A single ordered Rust engine session in a terminable worker, independent of HTTP liveness. */
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
    for (const request of pending.values()) request.reject(error);
    pending.clear();
    // Detach before termination: the old exit event cannot reject new requests.
    worker = undefined;
    void owner.stop();
    start();
    recovery = reconstructing ? Promise.reject(error)
      : send("construct", { version: 2, ...(lastCheckpoint ? { persistedProject: lastCheckpoint } : {}) });
    void recovery.catch(() => {});
  }
  function start() {
    worker = createWorkerLifetime({
      url: workerUrl,
      onMessage(message) {
        const request = pending.get(message.id);
        if (!request) return;
        pending.delete(message.id);
        if (message.ok) request.resolve(message.result); else request.reject(Error(message.error));
      },
      onError: error => fail(worker, error),
      onExit: code => fail(worker, Error(`Workbench worker exited (${code}); last saved design retained`)),
    });
  }
  function send(method, input) {
    if (disposed) return Promise.reject(Error("Folder workbench is disposed"));
    const id = ++sequence;
    const owner = worker;
    const request = Object.assign(createWorkerRequest(), { method });
    pending.set(id, request);
    request.watch({ timeoutMs, onTimeout: () => fail(owner, Error(`Workbench operation exceeded ${timeoutMs} ms; last saved design retained`)) });
    try { owner.post({ id, method, input }); }
    catch (error) { fail(owner, error); }
    return request.promise;
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
  const adapter = Object.fromEntries(["construct", "snapshot", "dispatch", "commit", "mutation", "exportProject", "exportWorkspaceDesign", "persistProject", "exportReproduction", "bakeProfile"].map((method) => [method, (input) => call(method, input)]));
  adapter.dispose = async () => {
    if (disposed) return;
    disposed = true;
    for (const request of pending.values()) request.reject(Error("Folder workbench disposed"));
    pending.clear();
    await worker.stop();
  };
  return adapter;
}
