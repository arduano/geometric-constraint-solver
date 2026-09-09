// SPDX-License-Identifier: GPL-3.0-or-later
import { parentPort } from "node:worker_threads";
import { readFileSync } from "node:fs";
import { demoBindingsUrl, demoWasmPath, workbenchRuntimeUrl } from "./workspace-runtime-paths.mjs";

const { default: wasm, WorkbenchHandle } = await import(demoBindingsUrl);
const { WasmWorkbenchAdapter, resolvePendingManagedMutationSnapshot } = await import(workbenchRuntimeUrl);

await wasm({ module_or_path: readFileSync(demoWasmPath) });
const adapter = new WasmWorkbenchAdapter(WorkbenchHandle);
let tail = Promise.resolve();
parentPort.on("message", (message) => {
  const run = async () => {
    try {
      let result = await adapter[message.method](message.input);
      if (result?.frame) result = await resolvePendingManagedMutationSnapshot(adapter, result);
      // interactionSnapshot returns its matching chrome/seed pair from one
      // native call. Keep it opaque and inside this ordered worker turn.
      parentPort.postMessage({ id: message.id, ok: true, result });
    } catch (error) { parentPort.postMessage({ id: message.id, ok: false, error: String(error) }); }
  };
  tail = tail.then(run, run);
});
