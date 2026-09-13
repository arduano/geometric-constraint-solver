// SPDX-License-Identifier: GPL-3.0-or-later
import type { PointerSample, WheelSample, WorkbenchAdapter, WorkbenchSnapshot } from "./adapter";
import { assertWorkbenchSnapshot, getCanvasSnapshotSequence, markCanvasOnlySnapshot, stampCanvasSnapshot } from "./adapter";
import { freezeDrawFrame } from "./canvas-scene";
import { WorkbenchActivity } from "./workbench-activity";
import type { WorkbenchWorkerMethod, WorkbenchWorkerRequest, WorkbenchWorkerResponse } from "./workbench-worker";

import { WorkerRequestChannel, type WorkerTransport, type WithoutWorkerId } from "./worker-channel";

/** Keep synchronous WASM work off the UI thread so accepted geometry and busy feedback can paint. */
export class WorkerWorkbenchAdapter implements WorkbenchAdapter {
  readonly activity = new WorkbenchActivity();
  private readonly channel: WorkerRequestChannel<WithoutWorkerId<WorkbenchWorkerRequest>, WorkbenchWorkerResponse, unknown>;
  private current: WorkbenchSnapshot | undefined;
  private finishManagedActivity: (() => void) | undefined;

  constructor(worker: WorkerTransport = new Worker(new URL("./workbench-worker.ts", import.meta.url), { type: "module" })) {
    this.channel = new WorkerRequestChannel(worker, {
      stoppedMessage: "Workbench worker stopped unexpectedly", unreadableMessage: "Workbench worker response could not be read",
      responseError: (response) => {
        if (response.kind !== "error") return undefined;
        this.finishManagedActivity?.(); this.finishManagedActivity = undefined;
        return response.message;
      },
      decode: (response) => this.decode(response),
      onFailure: () => { this.finishManagedActivity = undefined; this.activity.reset(); },
    });
  }

  construct(input: { version: 2; persistedProject?: string }) { return this.request<WorkbenchSnapshot>("construct", [input]); }
  toolCatalog() { return this.request<Awaited<ReturnType<WorkbenchAdapter["toolCatalog"]>>>("toolCatalog"); }
  snapshot() { return this.request<WorkbenchSnapshot>("snapshot"); }
  dispatch(input: { version: 2; command: string; payload?: unknown }) { return this.request<WorkbenchSnapshot>("dispatch", [input]); }
  managedCompilerContext() { return this.request<{ version: 2; patches: Record<string, unknown> }>("managedCompilerContext"); }
  pointer(input: PointerSample) { return this.request<WorkbenchSnapshot | null>("pointer", [input]); }
  wheel(input: WheelSample) { return this.request<WorkbenchSnapshot | null>("wheel", [input]); }
  wheelBatch(inputs: WheelSample[]) { return this.request<WorkbenchSnapshot | null>("wheelBatch", [inputs]); }
  resize(input: { version: 2; width: number; height: number; pixelRatio: number }) { return this.request<WorkbenchSnapshot | null>("resize", [input]); }
  cancel(input: { version: 2; reason: "escape" | "lost-capture" | "blur" }) { return this.request<WorkbenchSnapshot | null>("cancel", [input]); }
  exportProject() { return this.request<{ version: 2; filename: string; contents: string }>("exportProject"); }
  persistProject() { return this.request<{ version: 2; contents: string }>("persistProject"); }
  exportReproduction() { return this.request<{ version: 2; filename: string; contents: string }>("exportReproduction"); }
  exportInteractionTrace() { return this.request<{ version: 2; filename: string; contents: string }>("exportInteractionTrace"); }

  dispose() { this.channel.fail(new Error("Workbench worker has been disposed")); }

  private request<T>(method: WorkbenchWorkerMethod, args: unknown[] = []): Promise<T> {
    if (this.channel.failure) return Promise.reject(this.channel.failure);
    const finish = this.activity.begin();
    return this.channel.request<T>({ method, args }).finally(finish);
  }

  /** Canvas deltas and compiler activity are workbench protocol semantics. */
  private decode(response: WorkbenchWorkerResponse): unknown {
    let value: unknown;
    if (response.kind === "snapshot" || response.kind === "canvas") {
      const sequence = response.sequence;
      const previousSequence = this.current && getCanvasSnapshotSequence(this.current);
      if (!Number.isSafeInteger(sequence) || sequence < 1 || (previousSequence !== undefined && sequence <= previousSequence)) {
        throw new Error("Invalid workbench worker snapshot sequence");
      }
      if (response.kind === "canvas") {
        if (!this.current || response.baseSequence !== previousSequence) throw new Error("Canvas worker response has no matching snapshot");
        freezeDrawFrame(response.frame.scene);
        value = markCanvasOnlySnapshot(stampCanvasSnapshot(assertWorkbenchSnapshot({ ...this.current, frame: response.frame }), sequence));
      } else {
        freezeDrawFrame(response.value.frame.scene);
        value = stampCanvasSnapshot(assertWorkbenchSnapshot(response.value), sequence);
      }
      this.current = value as WorkbenchSnapshot;
      // A prepared edit is one operation across its compiler/resolve round trip.
      // Acquire before settling the request so the delayed overlay cannot blink.
      if (this.current.pendingManagedMutation) this.finishManagedActivity ??= this.activity.begin();
      else {
        this.finishManagedActivity?.();
        this.finishManagedActivity = undefined;
      }
    } else if (response.kind === "value") {
      value = response.value;
    } else {
      throw new Error("Invalid workbench worker response");
    }
    return value;
  }
}
