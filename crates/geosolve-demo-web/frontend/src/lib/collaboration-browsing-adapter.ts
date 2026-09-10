// SPDX-License-Identifier: GPL-3.0-or-later
import type { AuthoringModel, AuthoringView } from "./collaboration-authoring-worker";
import type { InteractionSeed } from "./local-interaction-worker";
import type { BrowsingAction, BrowsingChrome, BrowsingResult, BrowsingWorkerRequest, BrowsingWorkerResponse, BrowsingNavigationCommand, BrowsingEditCommand } from "./collaboration-browsing-worker";
export type { BrowsingChrome, BrowsingResult } from "./collaboration-browsing-worker";
export interface LocalBrowsingClient {
  replace(model: AuthoringModel, seed: InteractionSeed): Promise<Extract<BrowsingResult, { kind: "ready" }>>;
  present(view: AuthoringView): Promise<Extract<BrowsingResult, { kind: "chrome" }>>;
  navigate(view: AuthoringView, command: BrowsingNavigationCommand, payload: unknown): Promise<Extract<BrowsingResult, { kind: "navigation" }>>;
  describe(view: AuthoringView, authority: string, command: BrowsingEditCommand, payload: unknown): Promise<Extract<BrowsingResult, { kind: "mutation" }>>;
  dispose(): void;
}
type WorkerTransport = Pick<Worker, "postMessage" | "addEventListener" | "removeEventListener" | "terminate">;
type Waiter = { resolve(result: BrowsingResult): void; reject(error: Error): void };
type Queued = { action: BrowsingAction; generation: number; waiters: Waiter[] };

/** One running request, a bounded command queue and coalesced adjacent pending views. Superseded presents
 * share the newest response, stamped with its actual view. Callers publish only
 * while that model/view still matches their local interaction state.
 */
export class LocalBrowsingWorker implements LocalBrowsingClient {
  private nextId = 0; private generation = 0;
  private running?: Queued & { id: number }; private queued: Queued[] = [];
  private failure?: Error; private scheduled = false;
  constructor(private readonly worker: WorkerTransport = new Worker(new URL("./collaboration-browsing-worker.ts", import.meta.url), { type: "module" })) {
    worker.addEventListener("message", this.receive);
    worker.addEventListener("error", this.error);
    worker.addEventListener("messageerror", this.messageError);
  }
  replace(model: AuthoringModel, seed: InteractionSeed) {
    this.invalidate(Error("Browsing model was replaced")); ++this.generation;
    return this.request<Extract<BrowsingResult, { kind: "ready" }>>({ method: "replace", model, seed });
  }
  present(view: AuthoringView) { return this.request<Extract<BrowsingResult, { kind: "chrome" }>>({ method: "present", view }); }
  navigate(view: AuthoringView, command: BrowsingNavigationCommand, payload: unknown) { return this.request<Extract<BrowsingResult, { kind: "navigation" }>>({ method: "navigate", view, command, payload }); }
  describe(view: AuthoringView, authority: string, command: BrowsingEditCommand, payload: unknown) { return this.request<Extract<BrowsingResult, { kind: "mutation" }>>({ method: "describe", view, authority, command, payload }); }
  dispose() { this.fail(Error("Browsing worker was disposed")); }
  private request<T extends BrowsingResult>(action: BrowsingAction): Promise<T> {
    if (this.failure) return Promise.reject(this.failure);
    if (!this.generation) return Promise.reject(Error("Browsing model has not opened"));
    const count = (this.running?.waiters.length ?? 0) + this.queued.reduce((sum, request) => sum + request.waiters.length, 0);
    if (count >= 256) return Promise.reject(Error("Browsing request limit reached"));
    return new Promise<T>((resolve, reject) => {
      const waiter: Waiter = { resolve: resolve as (result: BrowsingResult) => void, reject };
      const last = this.queued.at(-1);
      if (last?.action.method === "present" && action.method === "present") { last.action = action; last.waiters.push(waiter); }
      else this.queued.push({ action, generation: this.generation, waiters: [waiter] });
      this.schedule();
    });
  }
  private schedule() {
    if (this.scheduled || this.running || this.failure) return;
    this.scheduled = true;
    queueMicrotask(() => { this.scheduled = false; this.pump(); });
  }
  private pump() {
    if (this.running || this.failure || !this.queued.length) return;
    const next = this.queued.shift()!;
    this.running = { ...next, id: ++this.nextId };
    try { this.worker.postMessage({ ...next.action, generation: next.generation, id: this.running.id } satisfies BrowsingWorkerRequest); }
    catch (error) { this.fail(error instanceof Error ? error : Error(String(error))); }
  }
  private receive = ({ data }: MessageEvent<BrowsingWorkerResponse>) => {
    const request = this.running;
    if (!request || data?.id !== request.id || data.generation !== request.generation) return;
    this.running = undefined;
    if ("error" in data) request.waiters.forEach(waiter => waiter.reject(Error(data.error)));
    else {
      try {
        const expected = { replace: "ready", present: "chrome", navigate: "navigation", describe: "mutation" }[request.action.method];
        if (!data.result || data.result.kind !== expected || !data.result.model?.documentEpoch || !Number.isSafeInteger(data.result.model.revision) || !data.result.model.sourceDesignDigest) throw Error("Invalid native browsing response");
        if (data.result.kind === "chrome" || data.result.kind === "navigation") validateChrome(data.result.chrome);
        request.waiters.forEach(waiter => waiter.resolve(data.result));
      } catch (error) {
        const failure = error instanceof Error ? error : Error(String(error));
        request.waiters.forEach(waiter => waiter.reject(failure)); this.fail(failure);
      }
    }
    this.schedule();
  };
  private error = (event: ErrorEvent) => { event.preventDefault(); this.fail(Error(event.message || "Browsing worker stopped")); };
  private messageError = () => this.fail(Error("Browsing response could not be read"));
  private invalidate(error: Error) {
    for (const request of [this.running, ...this.queued]) request?.waiters.forEach(waiter => waiter.reject(error));
    this.running = undefined; this.queued = [];
  }
  private fail(error: Error) {
    if (this.failure) return; this.failure = error;
    this.worker.removeEventListener("message", this.receive); this.worker.removeEventListener("error", this.error); this.worker.removeEventListener("messageerror", this.messageError);
    this.worker.terminate(); this.invalidate(error);
  }
}
function validateChrome(chrome: BrowsingChrome) {
  if (!chrome || !Array.isArray(chrome.explorer) || !Array.isArray(chrome.parameters) || !Array.isArray(chrome.problems) || !chrome.navigation || !Array.isArray(chrome.navigation.rows) || !chrome.dimensions || !Array.isArray(chrome.dimensions.entries)) throw Error("Invalid native browsing chrome");
  const freeze = (value: unknown) => { if (value && typeof value === "object") { Object.values(value).forEach(freeze); Object.freeze(value); } };
  freeze(chrome);
}
