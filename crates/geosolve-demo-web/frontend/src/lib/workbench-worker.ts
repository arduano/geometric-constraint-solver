// SPDX-License-Identifier: GPL-3.0-or-later
import initializeWasm, { WorkbenchHandle } from "../generated/geosolve_demo_web.js";
import type { WorkbenchAdapter, WorkbenchSnapshot } from "./adapter";
import { getCanvasSnapshotSequence, isCanvasOnlySnapshot } from "./adapter";
import { WasmWorkbenchAdapter } from "./wasm-adapter";

const methods = ["construct", "toolCatalog", "snapshot", "dispatch", "managedCompilerContext", "pointer", "wheel", "wheelBatch", "resize", "cancel", "exportProject", "persistProject", "exportReproduction", "exportInteractionTrace"] as const;
export type WorkbenchWorkerMethod = typeof methods[number];
export interface WorkbenchWorkerRequest {
  id: number;
  method: WorkbenchWorkerMethod;
  args: unknown[];
}
export type WorkbenchWorkerResponse =
  | { id: number; kind: "value"; value: unknown }
  | { id: number; kind: "snapshot"; value: WorkbenchSnapshot; sequence: number }
  | { id: number; kind: "canvas"; frame: WorkbenchSnapshot["frame"]; sequence: number; baseSequence: number }
  | { id: number; kind: "error"; message: string };

const snapshotMethods = new Set<WorkbenchWorkerMethod>(["construct", "snapshot", "dispatch", "pointer", "wheel", "wheelBatch", "resize", "cancel"]);

/** One existing Rust-backed adapter, with ordered requests and no new scene authority. */
export function createWorkbenchWorkerMessageHandler(
  adapter: Promise<WorkbenchAdapter>,
  reply: (response: WorkbenchWorkerResponse) => void,
): (event: MessageEvent<WorkbenchWorkerRequest>) => void {
  let tail = Promise.resolve();
  let previousSequence: number | undefined;
  // Keep an early initialization rejection handled until the first request arrives.
  void adapter.catch(() => undefined);
  return ({ data: request }) => {
    const run = async () => {
      try {
        if (!request || !Number.isSafeInteger(request.id) || request.id < 1
          || !methods.includes(request.method) || !Array.isArray(request.args)) {
          throw new Error("Invalid workbench worker request");
        }
        const owner = await adapter;
        const operation = owner[request.method];
        if (typeof operation !== "function") throw new Error(`Workbench operation is unavailable: ${request.method}`);
        const value = await (operation as (...args: unknown[]) => Promise<unknown>).apply(owner, request.args);
        if (value !== null && snapshotMethods.has(request.method)) {
          const snapshot = value as WorkbenchSnapshot;
          const sequence = getCanvasSnapshotSequence(snapshot);
          if (sequence === undefined) throw new Error("Workbench snapshot is missing its decode sequence");
          if (isCanvasOnlySnapshot(snapshot) && previousSequence !== undefined) {
            // Canvas-only updates retain the exact non-canvas state on both sides.
            // Do not clone source/history into every camera and hover response.
            reply({ id: request.id, kind: "canvas", frame: snapshot.frame, sequence, baseSequence: previousSequence });
          } else {
            reply({ id: request.id, kind: "snapshot", value: snapshot, sequence });
          }
          previousSequence = sequence;
        } else {
          reply({ id: request.id, kind: "value", value });
        }
      } catch (error) {
        reply({ id: request?.id, kind: "error", message: error instanceof Error ? error.message : String(error) });
      }
    };
    tail = tail.then(run, run);
  };
}

// The document guard allows focused transport tests to import the message owner
// without starting WASM in their browser-like main-thread test environment.
if (typeof document === "undefined" && typeof self !== "undefined") {
  const adapter = initializeWasm().then(() => new WasmWorkbenchAdapter(WorkbenchHandle));
  self.onmessage = createWorkbenchWorkerMessageHandler(adapter, (response) => self.postMessage(response));
}
