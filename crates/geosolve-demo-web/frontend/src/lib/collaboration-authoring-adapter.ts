// SPDX-License-Identifier: GPL-3.0-or-later
import type { ToolOperationTool, ToolOperationOperand, ToolOperationSample } from "../../../../../packages/geosolve-engine/src/tool-operations";
import { freezeDrawFrame } from "./canvas-scene";
import type { ConstructionSample, ConstructionTool, PointGestureSample, PointGestureTarget, PointGestureViewport } from "../../../../../packages/geosolve-engine/src/index";
import type { ToolOperationOptions, AuthoringAction, AuthoringModel, AuthoringPreview, AuthoringResult, AuthoringView, AuthoringWorkerResponse } from "./collaboration-authoring-worker";
export type { AuthoringModel, AuthoringModelIdentity, AuthoringPreview, AuthoringResult, AuthoringView } from "./collaboration-authoring-worker";

import { WorkerMailbox, type WorkerTransport } from "./worker-channel";
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
  private readonly mailbox: WorkerMailbox<AuthoringAction, AuthoringWorkerResponse, AuthoringResult>;
  constructor(worker: WorkerTransport = new Worker(new URL("./collaboration-authoring-worker.ts", import.meta.url), { type: "module" })) {
    this.mailbox = new WorkerMailbox(worker, {
      stoppedMessage: "Authoring worker stopped", unreadableMessage: "Authoring response could not be read",
      unopenedMessage: "Authoring model has not opened", replacedMessage: "Authoring model was replaced",
      fullMessage: "Authoring mailbox is full; finish or cancel the gesture", limit: 4096,
      combine: combineAuthoringActions,
      responseError: (response) => "error" in response ? response.error : undefined,
      decode: (response, action) => {
        if ("error" in response) throw Error(response.error);
        validateAuthoringResult(response.result, action);
        return response.result;
      },
    });
  }
  replace(model: AuthoringModel) { return this.mailbox.replace<Extract<AuthoringResult, { kind: "ready" }>>({ method: "replace", model }); }
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
  dispose() { this.mailbox.dispose(Error("Authoring worker was disposed")); }
  private request<T extends AuthoringResult>(action: AuthoringAction): Promise<T> { return this.mailbox.request<T>(action); }
}

/** Preserve all gesture samples, including intermediate clicks/corrections. Only paint is replaced. */
function combineAuthoringActions(previous: AuthoringAction, next: AuthoringAction): AuthoringAction | undefined {
  if (previous.method === "advancePoint" && next.method === "advancePoint" && previous.samples.length < 256)
    return { ...next, samples: [...previous.samples, ...next.samples] };
  if (previous.method === "advanceConstruction" && next.method === "advanceConstruction" && previous.samples.length < 256)
    return { ...next, samples: [...previous.samples, ...next.samples] };
  if (previous.method === "advanceOperation" && next.method === "advanceOperation" && previous.samples.length < 256)
    return { ...next, samples: [...previous.samples, ...next.samples] };
  if (previous.method === "render" && next.method === "render") return next;
  return undefined;
}
function validateAuthoringResult(result: AuthoringResult, action: AuthoringAction) {
  const expected = action.method === "replace" ? "ready"
    : action.method === "finishPoint" ? "point"
    : action.method === "finishConstruction" ? "construction"
    : action.method === "finishOperation" ? "operation"
    : action.method === "cancel" ? "cancelled" : "preview";
  if (!result || result.kind !== expected || !result.model || !result.model.documentEpoch || !Number.isSafeInteger(result.model.revision) || !result.model.sourceDesignDigest) throw Error("Invalid authoring worker response");
  if (result.kind === "preview") {
    if (result.presentation !== undefined && typeof result.presentation !== "string") throw Error("Invalid native authoring presentation");
    if (!result.view || typeof result.frame?.ariaLabel !== "string" || result.frame.scene.provenance.scene !== "provisional") throw Error("Invalid provisional authoring frame");
    freezeDrawFrame(result.frame.scene);
    if (result.frame.scene.items.some(item => item.interactive)) throw Error("Provisional authoring paint cannot own picking");
  }
}
