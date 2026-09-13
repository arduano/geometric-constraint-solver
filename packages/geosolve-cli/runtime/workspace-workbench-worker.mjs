// SPDX-License-Identifier: GPL-3.0-or-later
import { parentPort } from "node:worker_threads";
import { workbenchRuntimeUrl } from "./workspace-runtime-paths.mjs";

const { WorkspaceEngineRuntime } = await import(workbenchRuntimeUrl);
const adapter = new WorkspaceEngineRuntime();
let tail = Promise.resolve();
parentPort.on("message", (message) => {
  const run = async () => {
    try {
      const result = await adapter[message.method](message.input);
      parentPort.postMessage({ id: message.id, ok: true, result });
    } catch (error) { parentPort.postMessage({ id: message.id, ok: false, error: String(error) }); }
  };
  tail = tail.then(run, run);
});
