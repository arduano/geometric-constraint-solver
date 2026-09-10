// SPDX-License-Identifier: GPL-3.0-or-later
import type { AuthoringPreview, AuthoringView } from "./collaboration-authoring-worker";
import { freezeDrawFrame } from "./canvas-scene";
import type { InteractionSeed, InteractionState, LocalInteractionMethod, LocalInteractionRequest, LocalInteractionResponse, LocalInteractionUpdate } from "./local-interaction-worker";
export type { InteractionSeed, InteractionState, LocalInteractionUpdate } from "./local-interaction-worker";

export interface AuthoringPointer { viewport: import("../../../../../packages/geosolve-engine/src/index").PointGestureViewport; position: readonly [number,number]; target: import("../../../../../packages/geosolve-engine/src/index").PointGestureTarget | null }
export interface LocalInteractionClient {
  projectPrediction?(input:{presentation:string;view:AuthoringView;construction?:Pick<NonNullable<AuthoringPreview["construction"]>,"preview"|"inference_guides">}):Promise<LocalInteractionUpdate>;
  authoringPointer?(input: {x:number;y:number;captured?:boolean}): Promise<AuthoringPointer>;
  construct(seed: InteractionSeed): Promise<LocalInteractionUpdate>;
  replace(seed: InteractionSeed, preserveSelection: boolean): Promise<LocalInteractionUpdate>;
  update(method: Exclude<LocalInteractionMethod, "construct" | "replace" | "state">, input: unknown): Promise<LocalInteractionUpdate | null>;
  state(): Promise<InteractionState>;
  dispose(): void;
}
type WorkerTransport = Pick<Worker, "postMessage" | "addEventListener" | "removeEventListener" | "terminate">;

/** The dedicated worker owns only detached accepted-scene interaction, never editing or solving. */
export class LocalInteractionWorker implements LocalInteractionClient {
  private nextId = 0;
  private readonly pending = new Map<number, { resolve: (value: unknown) => void; reject: (error: Error) => void }>();
  private failure?: Error;
  constructor(private readonly worker: WorkerTransport = new Worker(new URL("./local-interaction-worker.ts", import.meta.url), { type: "module" })) {
    worker.addEventListener("message", this.receive);
    worker.addEventListener("error", this.error);
    worker.addEventListener("messageerror", this.messageError);
  }
  projectPrediction(input:Parameters<NonNullable<LocalInteractionClient["projectPrediction"]>>[0]) { return this.request<LocalInteractionUpdate>("projectPrediction",input); }
  authoringPointer(input: {x:number;y:number;captured?:boolean}) { return this.request<AuthoringPointer>("authoringPointer",input); }
  construct(seed: InteractionSeed) { return this.request<LocalInteractionUpdate>("construct", seed); }
  replace(seed: InteractionSeed, preserveSelection: boolean) { return this.request<LocalInteractionUpdate>("replace", { seed, preserveSelection }); }
  update(method: Exclude<LocalInteractionMethod, "construct" | "replace" | "state">, input: unknown) { return this.request<LocalInteractionUpdate | null>(method, input); }
  state() { return this.request<InteractionState>("state"); }
  dispose() { this.fail(Error("Local interaction worker was disposed")); }
  private request<T>(method: LocalInteractionMethod, input?: unknown): Promise<T> {
    if (this.failure) return Promise.reject(this.failure);
    const id = ++this.nextId;
    return new Promise((resolve, reject) => {
      this.pending.set(id, { resolve: resolve as (value: unknown) => void, reject });
      try { this.worker.postMessage({ id, method, input } satisfies LocalInteractionRequest); }
      catch (error) { this.pending.delete(id); reject(error); }
    });
  }
  private receive = ({ data }: MessageEvent<LocalInteractionResponse>) => {
    const request = this.pending.get(data?.id);
    if (!request) return;
    this.pending.delete(data.id);
    if ("error" in data) { request.reject(Error(data.error)); return; }
    try {
      if (data.result !== null && typeof data.result !== "object") throw Error("Invalid local interaction response");
      if (data.result && "frame" in data.result) {
        const update = data.result as LocalInteractionUpdate;
        if (!update.state || typeof update.state !== "object" || typeof update.selectionChanged !== "boolean" || typeof update.serverFrameCompatible !== "boolean" || typeof update.frame?.ariaLabel !== "string") throw Error("Invalid local interaction update");
        freezeDrawFrame(update.frame.scene);
      }
      request.resolve(data.result);
    } catch (error) {
      const failure = error instanceof Error ? error : Error(String(error));
      request.reject(failure);
      this.fail(failure);
    }
  };
  private error = (event: ErrorEvent) => { event.preventDefault(); this.fail(Error(event.message || "Local interaction worker stopped")); };
  private messageError = () => this.fail(Error("Local interaction response could not be read"));
  private fail(error: Error) {
    if (this.failure) return;
    this.failure = error;
    this.worker.removeEventListener("message", this.receive);
    this.worker.removeEventListener("error", this.error);
    this.worker.removeEventListener("messageerror", this.messageError);
    this.worker.terminate();
    for (const request of this.pending.values()) request.reject(error);
    this.pending.clear();
  }
}
