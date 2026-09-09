// SPDX-License-Identifier: GPL-3.0-or-later
import type { PointerSample, WheelSample, WorkbenchAdapter, WorkbenchSnapshot } from "./adapter";
import { assertWorkbenchSnapshot, getCanvasSnapshotSequence, markCanvasOnlySnapshot, stampCanvasSnapshot } from "./adapter";
import { freezeDrawFrame } from "./canvas-scene";
import { WorkbenchActivity } from "./workbench-activity";
import type { WorkbenchWorkerMethod, WorkbenchWorkerRequest, WorkbenchWorkerResponse } from "./workbench-worker";

type PendingRequest = { resolve: (value: unknown) => void; reject: (error: Error) => void };
type WorkerTransport = Pick<Worker, "postMessage" | "addEventListener" | "removeEventListener" | "terminate">;

/** Keep synchronous WASM work off the UI thread so accepted geometry and busy feedback can paint. */
export class WorkerWorkbenchAdapter implements WorkbenchAdapter {
  readonly activity = new WorkbenchActivity();
  private readonly pending = new Map<number, PendingRequest>();
  private nextId = 0;
  private current: WorkbenchSnapshot | undefined;
  private failure: Error | undefined;
  private finishManagedActivity: (() => void) | undefined;

  constructor(private readonly worker: WorkerTransport = new Worker(new URL("./workbench-worker.ts", import.meta.url), { type: "module" })) {
    worker.addEventListener("message", this.receive);
    worker.addEventListener("error", this.workerError);
    worker.addEventListener("messageerror", this.messageError);
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

  dispose() { this.fail(new Error("Workbench worker has been disposed")); }

  private request<T>(method: WorkbenchWorkerMethod, args: unknown[] = []): Promise<T> {
    if (this.failure) return Promise.reject(this.failure);
    const finish = this.activity.begin();
    const id = ++this.nextId;
    return new Promise<unknown>((resolve, reject) => {
      this.pending.set(id, { resolve, reject });
      try {
        this.worker.postMessage({ id, method, args } satisfies WorkbenchWorkerRequest);
      } catch (error) {
        this.pending.delete(id);
        reject(error);
      }
    }).finally(finish) as Promise<T>;
  }

  private readonly receive = (event: MessageEvent<WorkbenchWorkerResponse>) => {
    const response = event.data;
    const pending = this.pending.get(response?.id);
    if (!pending) return;
    this.pending.delete(response.id);
    if (response.kind === "error") {
      this.finishManagedActivity?.();
      this.finishManagedActivity = undefined;
      pending.reject(new Error(response.message));
      return;
    }
    try {
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
      pending.resolve(value);
    } catch (error) {
      const failure = error instanceof Error ? error : new Error(String(error));
      pending.reject(failure);
      // A broken transport cannot safely interpret any later canvas deltas.
      this.fail(failure);
    }
  };

  private readonly workerError = (event: ErrorEvent) => {
    event.preventDefault();
    this.fail(new Error(event.message || "Workbench worker stopped unexpectedly"));
  };
  private readonly messageError = () => { this.fail(new Error("Workbench worker response could not be read")); };

  private fail(error: Error) {
    if (this.failure) return;
    this.failure = error;
    this.worker.removeEventListener("message", this.receive);
    this.worker.removeEventListener("error", this.workerError);
    this.worker.removeEventListener("messageerror", this.messageError);
    this.worker.terminate();
    for (const pending of this.pending.values()) pending.reject(error);
    this.pending.clear();
    this.finishManagedActivity = undefined;
    this.activity.reset();
  }
}
