// SPDX-License-Identifier: GPL-3.0-or-later
import type { ToolOperationFrame, ToolOperationCommand } from "../../../../../packages/geosolve-engine/src/tool-operations";
import { LocalAuthoringWorker } from "./collaboration-authoring-adapter";
import type { AuthoringModelIdentity, AuthoringPreview, AuthoringWorkerRequest, AuthoringWorkerResponse } from "./collaboration-authoring-worker";
import type { RemotePaintRequest } from "./collaboration-remote-authoring-renderer";
import type { ConstructionCommand, ConstructionFrame, PointGestureFrame, PointGestureTerminal } from "../../../../../packages/geosolve-engine/src/index";

type Transport = Pick<Worker, "postMessage" | "addEventListener" | "removeEventListener" | "terminate">;
type PreviewReply =
  | { kind: "preview"; basis: AuthoringModelIdentity; ticket: string; presentation: string; point?: PointGestureFrame; construction?: ConstructionFrame; operation?: ToolOperationFrame }
  | { kind: "point"; basis: AuthoringModelIdentity; terminal: PointGestureTerminal }
  | { kind: "construction"; basis: AuthoringModelIdentity; command: ConstructionCommand }
  | { kind: "operation"; basis: AuthoringModelIdentity; command: ToolOperationCommand }
  | { kind: "cancelled"; basis: AuthoringModelIdentity };
export type PreviewRpc = (request: unknown, signal?: AbortSignal) => Promise<PreviewReply>;
const sameModel = (a: AuthoringModelIdentity, b: AuthoringModelIdentity) => a.documentEpoch === b.documentEpoch && a.revision === b.revision && a.sourceDesignDigest === b.sourceDesignDigest;

/** Uses the ordinary bounded authoring mailbox. Network carries gesture samples;
 * cached native presentation is repainted locally for every camera/selection view.
 */
export function createRemoteAuthoringClient(rpc: PreviewRpc, renderer: Transport = new Worker(new URL("./collaboration-remote-authoring-renderer.ts", import.meta.url), { type: "module" })) {
  return new LocalAuthoringWorker(new RemoteAuthoringTransport(rpc, renderer));
}

export class RemoteAuthoringTransport extends EventTarget implements Transport {
  private model?: AuthoringModelIdentity;
  private generation = 0;
  private ticket?: string;
  private kind?: "point" | "construction" | "operation";
  private cached?: Extract<PreviewReply, { kind: "preview" }>;
  private readonly requests = new Set<AbortController>();
  private stopped = false;
  constructor(private readonly rpc: PreviewRpc, private readonly renderer: Transport) {
    super();
    renderer.addEventListener("message", this.rendered);
    renderer.addEventListener("error", this.renderError);
    renderer.addEventListener("messageerror", this.renderError);
  }
  postMessage(request: AuthoringWorkerRequest) {
    void this.handle(request).catch(error => {
      if (request.generation === this.generation) this.retire();
      this.reply({ id: request.id, generation: request.generation, error: String(error) });
    });
  }
  terminate() {
    if (this.stopped) return;
    this.stopped = true; this.retire();
    this.renderer.removeEventListener("message", this.rendered);
    this.renderer.removeEventListener("error", this.renderError);
    this.renderer.removeEventListener("messageerror", this.renderError);
    this.renderer.terminate();
  }
  private reply(response: AuthoringWorkerResponse) {
    if (!this.stopped) this.dispatchEvent(new MessageEvent("message", { data: response }));
  }
  private rendered = (event: MessageEvent<AuthoringWorkerResponse>) => {
    if (event.data.generation === this.generation) {
      if ("error" in event.data) this.retire();
      this.reply(event.data);
    }
  };
  private renderError = () => { this.dispatchEvent(new ErrorEvent("error", { message: "Authoring preview rendering stopped" })); };
  private retire() {
    for (const controller of this.requests) controller.abort();
    this.requests.clear();
    if (this.ticket) void this.rpc({ action: "cancel", ticket: this.ticket }).catch(() => {});
    this.ticket = undefined; this.kind = undefined; this.cached = undefined;
  }
  private async send(body: unknown, generation: number): Promise<PreviewReply> {
    const controller = new AbortController(); this.requests.add(controller);
    try {
      const result = await this.rpc(body, controller.signal);
      if (this.stopped || generation !== this.generation) {
        if (result.kind === "preview") void this.rpc({ action: "cancel", ticket: result.ticket }).catch(() => {});
        throw Error("Authoring preview belongs to an obsolete model");
      }
      if (!this.model || !result.basis || !sameModel(this.model, result.basis)) {
        if (result.kind === "preview") void this.rpc({ action: "cancel", ticket: result.ticket }).catch(() => {});
        throw Error("Server authoring preview has a foreign basis");
      }
      return result;
    } finally { this.requests.delete(controller); }
  }
  private async handle(request: AuthoringWorkerRequest) {
    if (this.stopped) throw Error("Remote authoring client was disposed");
    if (request.method === "replace") {
      if (request.generation <= this.generation) throw Error("Obsolete authoring model replacement");
      this.retire(); this.generation = request.generation;
      const { documentEpoch, revision, sourceDesignDigest } = request.model;
      this.model = { documentEpoch, revision, sourceDesignDigest };
      this.reply({ id: request.id, generation: request.generation, result: { kind: "ready", model: this.model } });
      return;
    }
    if (!this.model || request.generation !== this.generation) throw Error("Authoring model is unavailable or obsolete");
    const generation = this.generation, model = this.model;
    if (request.method === "cancel") {
      this.retire(); this.reply({ id: request.id, generation, result: { kind: "cancelled", model } }); return;
    }
    let result: PreviewReply;
    if (request.method === "beginPoint" || request.method === "beginConstruction" || request.method === "beginOperation") {
      if (this.ticket) throw Error("Finish or cancel the active gesture");
      this.kind = request.method === "beginPoint" ? "point" : request.method === "beginConstruction" ? "construction" : "operation";
      result = await this.send({ action: "begin", basis: model, kind: this.kind, gestureId: request.gestureId, viewport: request.viewport,
        ...(request.method === "beginPoint" ? { target: request.target } : request.method === "beginConstruction" ? { tool: request.tool, role: request.role } : { tool: request.tool, selection: request.selection, options:request.options, view: {state:request.view.state} }) }, generation);
    } else if(request.method === "pickOperationSelection") {
      if(!this.ticket||this.kind!=="operation")throw Error("No active remote tool operation");
      result=await this.send({action:"pick_selection",ticket:this.ticket,sequence:request.sequence,view:{state:request.view.state}},generation);
    } else if (request.method === "render") {
      if (!this.cached) throw Error("No authoring preview is available");
      result = this.cached;
    } else {
      if (!this.ticket) throw Error("No active remote authoring gesture");
      const kind = request.method === "advancePoint" || request.method === "finishPoint" ? "point" : request.method === "advanceConstruction" || request.method === "finishConstruction" ? "construction" : "operation";
      if (this.kind !== kind) throw Error("Authoring gesture kind changed");
      if (request.method === "advancePoint" || request.method === "advanceConstruction" || request.method === "advanceOperation") result = await this.send({ action: "advance", ticket: this.ticket, samples: request.samples }, generation);
      else {
        try { result = await this.send({ action: "finish", ticket: this.ticket }, generation); }
        finally { if (generation === this.generation) { this.ticket = undefined; this.kind = undefined; this.cached = undefined; } }
      }
    }
    if (request.method === "finishPoint" && result.kind === "point") this.reply({ id: request.id, generation, result: { kind: "point", model, terminal: result.terminal } });
    else if (request.method === "finishConstruction" && result.kind === "construction") this.reply({ id: request.id, generation, result: { kind: "construction", model, command: result.command } });
    else if (request.method === "finishOperation" && result.kind === "operation") this.reply({ id: request.id, generation, result: { kind: "operation", model, command: result.command } });
    else {
      if (result.kind !== "preview" || !("view" in request) || typeof result.presentation !== "string" || !/^[a-f0-9]{64}$/u.test(result.ticket)) throw Error("Unexpected authoring preview response");
      this.ticket = result.ticket; this.cached = result;
      const preview: Omit<AuthoringPreview, "frame"> = { kind: "preview", model, view: request.view, presentation: result.presentation, point: result.point, construction: result.construction, operation: result.operation };
      this.renderer.postMessage({ id: request.id, generation, presentation: result.presentation, result: preview } satisfies RemotePaintRequest);
    }
  }
}
