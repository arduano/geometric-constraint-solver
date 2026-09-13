// SPDX-License-Identifier: GPL-3.0-or-later
import type { WorkspaceViewPresentation } from "../../../../../packages/geosolve-engine/src/index";
import type { AuthoringPreview, AuthoringView } from "./collaboration-authoring-worker";
import { freezeDrawFrame } from "./canvas-scene";
import type { InteractionSeed, InteractionState, LocalInteractionMethod, LocalInteractionRequest, LocalInteractionResponse, LocalInteractionUpdate } from "./local-interaction-worker";
export type { InteractionSeed, InteractionState, LocalInteractionUpdate } from "./local-interaction-worker";

export interface AuthoringPointer { viewport: import("../../../../../packages/geosolve-engine/src/index").PointGestureViewport; position: readonly [number,number]; target: import("../../../../../packages/geosolve-engine/src/index").PointGestureTarget | null }
export interface LocalInteractionClient {
  exportPresentation?(): Promise<WorkspaceViewPresentation>;
  projectPrediction?(input:{presentation:string;view:AuthoringView;construction?:Pick<NonNullable<AuthoringPreview["construction"]>,"preview"|"inference_guides">}):Promise<LocalInteractionUpdate>;
  authoringPointer?(input: {x:number;y:number;captured?:boolean}): Promise<AuthoringPointer>;
  construct(seed: InteractionSeed): Promise<LocalInteractionUpdate>;
  replace(seed: InteractionSeed, preserveSelection: boolean): Promise<LocalInteractionUpdate>;
  update(method: Exclude<LocalInteractionMethod, "construct" | "replace" | "state">, input: unknown): Promise<LocalInteractionUpdate | null>;
  state(): Promise<InteractionState>;
  dispose(): void;
}
import { WorkerRequestChannel, type WorkerTransport, type WithoutWorkerId } from "./worker-channel";

/** The dedicated worker owns only detached accepted-scene interaction, never editing or solving.
 * Navigation coalescing stays with the canvas/adapter owner; this transport drops no admitted input. */
export class LocalInteractionWorker implements LocalInteractionClient {
  private readonly channel: WorkerRequestChannel<WithoutWorkerId<LocalInteractionRequest>, LocalInteractionResponse, unknown>;
  constructor(worker: WorkerTransport = new Worker(new URL("./local-interaction-worker.ts", import.meta.url), { type: "module" })) {
    this.channel = new WorkerRequestChannel(worker, {
      stoppedMessage: "Local interaction worker stopped", unreadableMessage: "Local interaction response could not be read",
      responseError: (response) => "error" in response ? response.error : undefined,
      decode: (response) => {
        if ("error" in response) throw Error(response.error);
        if (response.result !== null && typeof response.result !== "object") throw Error("Invalid local interaction response");
        if (response.result && "frame" in response.result) {
          const update = response.result as LocalInteractionUpdate;
          if (!update.state || typeof update.state !== "object" || typeof update.selectionChanged !== "boolean" || typeof update.serverFrameCompatible !== "boolean" || typeof update.frame?.ariaLabel !== "string") throw Error("Invalid local interaction update");
          freezeDrawFrame(update.frame.scene);
        }
        return response.result;
      },
    });
  }
  projectPrediction(input:Parameters<NonNullable<LocalInteractionClient["projectPrediction"]>>[0]) { return this.request<LocalInteractionUpdate>("projectPrediction",input); }
  authoringPointer(input: {x:number;y:number;captured?:boolean}) { return this.request<AuthoringPointer>("authoringPointer",input); }
  construct(seed: InteractionSeed) { return this.request<LocalInteractionUpdate>("construct", seed); }
  replace(seed: InteractionSeed, preserveSelection: boolean) { return this.request<LocalInteractionUpdate>("replace", { seed, preserveSelection }); }
  update(method: Exclude<LocalInteractionMethod, "construct" | "replace" | "state">, input: unknown) { return this.request<LocalInteractionUpdate | null>(method, input); }
  exportPresentation() { return this.request<WorkspaceViewPresentation>("exportPresentation"); }
  state() { return this.request<InteractionState>("state"); }
  dispose() { this.channel.fail(Error("Local interaction worker was disposed")); }
  private request<T>(method: LocalInteractionMethod, input?: unknown): Promise<T> { return this.channel.request<T>({ method, input }); }
}
