// SPDX-License-Identifier: GPL-3.0-or-later
import type { ToolOperationTool, ToolOperationOperand, ToolOperationSample } from "../../../../../packages/geosolve-engine/src/tool-operations";
import { freezeDrawFrame } from "./canvas-scene";
import type { ConstructionSample, ConstructionTool, PointGestureSample, PointGestureTarget, PointGestureViewport } from "../../../../../packages/geosolve-engine/src/index";
import type { ToolOperationOptions, AuthoringAction, AuthoringModel, AuthoringPreview, AuthoringResult, AuthoringView, AuthoringWorkerRequest, AuthoringWorkerResponse } from "./collaboration-authoring-worker";
export type { AuthoringModel, AuthoringModelIdentity, AuthoringPreview, AuthoringResult, AuthoringView } from "./collaboration-authoring-worker";

type WorkerTransport = Pick<Worker, "postMessage" | "addEventListener" | "removeEventListener" | "terminate">;
type Waiter = { resolve: (value: AuthoringResult) => void; reject: (error: Error) => void };
type Queued = { action: AuthoringAction; generation: number; waiters: Waiter[] };
type Start = { gestureId: number; viewport: PointGestureViewport; view: AuthoringView };
export interface LocalAuthoringClient {
  replace(model: AuthoringModel): Promise<Extract<AuthoringResult, { kind: "ready" }>>;
  beginPoint(input: Start & { target: PointGestureTarget }): Promise<AuthoringPreview>;
  advancePoint(sample: PointGestureSample, view: AuthoringView): Promise<AuthoringPreview>;
  finishPoint(): Promise<Extract<AuthoringResult, { kind: "point" }>>;
  beginConstruction(input: Start & { tool: ConstructionTool; role?: "profile" | "construction" }): Promise<AuthoringPreview>;
  advanceConstruction(sample: ConstructionSample, view: AuthoringView): Promise<AuthoringPreview>;
  finishConstruction(): Promise<Extract<AuthoringResult, { kind: "construction" }>>;
  beginOperation(input: Start & { tool: ToolOperationTool; selection?: readonly ToolOperationOperand[]; options?:ToolOperationOptions }): Promise<AuthoringPreview>;
  advanceOperation(sample: ToolOperationSample, view: AuthoringView): Promise<AuthoringPreview>;
  pickOperationSelection(sequence:number,view:AuthoringView):Promise<AuthoringPreview>;
  finishOperation(): Promise<Extract<AuthoringResult, { kind: "operation" }>>;
  render(view: AuthoringView): Promise<AuthoringPreview>;
  cancel(): Promise<Extract<AuthoringResult, { kind: "cancelled" }>>;
  dispose(): void;
}

/** Bounded mailbox; coalesces paint while preserving every admitted path sample,
 * click, correction and terminal ordering. A separate navigation worker never
 * waits for this queue. Model replacement immediately invalidates old replies.
 */
export class LocalAuthoringWorker implements LocalAuthoringClient {
  private nextId = 0;
  private generation = 0;
  private queue: Queued[] = [];
  private running?: Queued & { id: number };
  private failure?: Error;
  private scheduled = false;
  private pendingCount = 0;
  constructor(private readonly worker: WorkerTransport = new Worker(new URL("./collaboration-authoring-worker.ts", import.meta.url), { type: "module" })) {
    worker.addEventListener("message", this.receive);
    worker.addEventListener("error", this.error);
    worker.addEventListener("messageerror", this.messageError);
  }
  replace(model: AuthoringModel) {
    this.invalidate(Error("Authoring model was replaced"));
    ++this.generation;
    return this.request<Extract<AuthoringResult, { kind: "ready" }>>({ method: "replace", model });
  }
  beginPoint(input: Start & { target: PointGestureTarget }) { return this.request<AuthoringPreview>({ method: "beginPoint", ...input }); }
  advancePoint(sample: PointGestureSample, view: AuthoringView) { return this.request<AuthoringPreview>({ method: "advancePoint", samples: [sample], view }); }
  finishPoint() { return this.request<Extract<AuthoringResult, { kind: "point" }>>({ method: "finishPoint" }); }
  beginConstruction(input: Start & { tool: ConstructionTool; role?: "profile" | "construction" }) { return this.request<AuthoringPreview>({ method: "beginConstruction", ...input, role: input.role ?? "profile" }); }
  advanceConstruction(sample: ConstructionSample, view: AuthoringView) { return this.request<AuthoringPreview>({ method: "advanceConstruction", samples: [sample], view }); }
  finishConstruction() { return this.request<Extract<AuthoringResult, { kind: "construction" }>>({ method: "finishConstruction" }); }
  beginOperation(input: Start & { tool: ToolOperationTool; selection?: readonly ToolOperationOperand[]; options?:ToolOperationOptions }) { return this.request<AuthoringPreview>({ method: "beginOperation", ...input }); }
  advanceOperation(sample: ToolOperationSample, view: AuthoringView) { return this.request<AuthoringPreview>({ method: "advanceOperation", samples: [sample], view }); }
  pickOperationSelection(sequence:number,view:AuthoringView) { return this.request<AuthoringPreview>({method:"pickOperationSelection",sequence,view}); }
  finishOperation() { return this.request<Extract<AuthoringResult, { kind: "operation" }>>({ method: "finishOperation" }); }
  render(view: AuthoringView) { return this.request<AuthoringPreview>({ method: "render", view }); }
  cancel() { return this.request<Extract<AuthoringResult, { kind: "cancelled" }>>({ method: "cancel" }); }
  dispose() { this.fail(Error("Authoring worker was disposed")); }
  private request<T extends AuthoringResult>(action: AuthoringAction): Promise<T> {
    if (this.failure) return Promise.reject(this.failure);
    if (!this.generation) return Promise.reject(Error("Authoring model has not opened"));
    if (this.pendingCount >= 4096) return Promise.reject(Error("Authoring mailbox is full; finish or cancel the gesture"));
    return new Promise<T>((resolve, reject) => {
      const waiter: Waiter = { resolve: resolve as (value: AuthoringResult) => void, reject };
      const last = this.queue.at(-1);
      let combined = false;
      if (last?.generation === this.generation) {
        if (last.action.method === "advancePoint" && action.method === "advancePoint" && last.action.samples.length < 256) {
          last.action = { ...action, samples: [...last.action.samples, ...action.samples] }; combined = true;
        } else if (last.action.method === "advanceConstruction" && action.method === "advanceConstruction" && last.action.samples.length < 256) {
          last.action = { ...action, samples: [...last.action.samples, ...action.samples] }; combined = true;
        } else if (last.action.method === "advanceOperation" && action.method === "advanceOperation" && last.action.samples.length < 256) {
          last.action = { ...action, samples: [...last.action.samples, ...action.samples] }; combined = true;
        } else if (last.action.method === "render" && action.method === "render") {
          last.action = action; combined = true;
        }
      }
      if (combined) last!.waiters.push(waiter);
      else this.queue.push({ action, generation: this.generation, waiters: [waiter] });
      ++this.pendingCount;
      this.schedule();
    });
  }
  private schedule() {
    if (this.scheduled || this.running || this.failure) return;
    this.scheduled = true;
    queueMicrotask(() => { this.scheduled = false; this.pump(); });
  }
  private pump() {
    if (this.running || this.failure) return;
    const next = this.queue.shift();
    if (!next) return;
    const id = ++this.nextId;
    this.running = { ...next, id };
    try { this.worker.postMessage({ ...next.action, id, generation: next.generation } satisfies AuthoringWorkerRequest); }
    catch (error) { this.fail(error instanceof Error ? error : Error(String(error))); }
  }
  private receive = ({ data }: MessageEvent<AuthoringWorkerResponse>) => {
    const request = this.running;
    if (!request || data?.id !== request.id || data.generation !== request.generation) return;
    this.running = undefined;
    this.pendingCount -= request.waiters.length;
    if ("error" in data) {
      for (const waiter of request.waiters) waiter.reject(Error(data.error));
    } else {
      try {
        const expected = request.action.method === "replace" ? "ready"
          : request.action.method === "finishPoint" ? "point"
          : request.action.method === "finishConstruction" ? "construction"
          : request.action.method === "finishOperation" ? "operation"
          : request.action.method === "cancel" ? "cancelled" : "preview";
        if (!data.result || data.result.kind !== expected || !data.result.model || !data.result.model.documentEpoch || !Number.isSafeInteger(data.result.model.revision) || !data.result.model.sourceDesignDigest) throw Error("Invalid authoring worker response");
        if (data.result.kind === "preview") {
          if (data.result.presentation!==undefined&&typeof data.result.presentation!=="string") throw Error("Invalid native authoring presentation");
          if (!data.result.view || typeof data.result.frame?.ariaLabel !== "string" || data.result.frame.scene.provenance.scene !== "provisional") throw Error("Invalid provisional authoring frame");
          freezeDrawFrame(data.result.frame.scene);
          if (data.result.frame.scene.items.some(item => item.interactive)) throw Error("Provisional authoring paint cannot own picking");
        }
        for (const waiter of request.waiters) waiter.resolve(data.result);
      } catch (error) {
        const failure = error instanceof Error ? error : Error(String(error));
        for (const waiter of request.waiters) waiter.reject(failure);
        this.fail(failure);
      }
    }
    this.schedule();
  };
  private error = (event: ErrorEvent) => { event.preventDefault(); this.fail(Error(event.message || "Authoring worker stopped")); };
  private messageError = () => this.fail(Error("Authoring response could not be read"));
  private invalidate(error: Error) {
    for (const request of [...this.queue, ...(this.running ? [this.running] : [])]) for (const waiter of request.waiters) waiter.reject(error);
    this.queue = []; this.running = undefined; this.pendingCount = 0;
  }
  private fail(error: Error) {
    if (this.failure) return;
    this.failure = error;
    this.worker.removeEventListener("message", this.receive);
    this.worker.removeEventListener("error", this.error);
    this.worker.removeEventListener("messageerror", this.messageError);
    this.worker.terminate();
    this.invalidate(error);
  }
}
