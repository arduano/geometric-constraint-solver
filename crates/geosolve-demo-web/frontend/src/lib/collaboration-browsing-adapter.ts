// SPDX-License-Identifier: GPL-3.0-or-later
import type { WorkspaceViewPresentation } from "../../../../../packages/geosolve-engine/src/index";
import { assertWorkbenchSnapshot } from "./adapter";
import { assertToolCatalog } from "./tool-catalog";
import type { AuthoringView } from "./collaboration-authoring-worker";
import type { InteractionSeed } from "./local-interaction-worker";
import type { BrowsingAction, BrowsingModel, BrowsingChrome, BrowsingResult, BrowsingWorkerResponse, BrowsingNavigationCommand, BrowsingEditCommand } from "./collaboration-browsing-worker";
export type { BrowsingModel, BrowsingChrome, BrowsingInitialization, BrowsingResult } from "./collaboration-browsing-worker";
export interface LocalBrowsingClient {
  initialize?(model: BrowsingModel, seed: InteractionSeed, presentation?: WorkspaceViewPresentation): Promise<Extract<BrowsingResult, { kind: "initialized" }>>;
  replace(model: BrowsingModel, seed: InteractionSeed): Promise<Extract<BrowsingResult, { kind: "ready" }>>;
  present(view: AuthoringView): Promise<Extract<BrowsingResult, { kind: "chrome" }>>;
  navigate(view: AuthoringView, command: BrowsingNavigationCommand, payload: unknown): Promise<Extract<BrowsingResult, { kind: "navigation" }>>;
  describe(view: AuthoringView, authority: string, command: BrowsingEditCommand, payload: unknown): Promise<Extract<BrowsingResult, { kind: "mutation" }>>;
  dispose(): void;
}
import { WorkerMailbox, type WorkerTransport } from "./worker-channel";

/** One running request, a bounded command queue and coalesced adjacent pending views. Superseded presents
 * share the newest response, stamped with its actual view. Callers publish only
 * while that model/view still matches their local interaction state.
 */
export class LocalBrowsingWorker implements LocalBrowsingClient {
  private readonly mailbox: WorkerMailbox<BrowsingAction, BrowsingWorkerResponse, BrowsingResult>;
  constructor(worker: WorkerTransport = new Worker(new URL("./collaboration-browsing-worker.ts", import.meta.url), { type: "module" })) {
    this.mailbox = new WorkerMailbox(worker, {
      stoppedMessage: "Browsing worker stopped", unreadableMessage: "Browsing response could not be read",
      unopenedMessage: "Browsing model has not opened", replacedMessage: "Browsing model was replaced",
      fullMessage: "Browsing request limit reached", limit: 256,
      // Navigation and edit descriptions are ordering barriers between disposable views.
      combine: (previous, next) => previous.method === "present" && next.method === "present" ? next : undefined,
      responseError: (response) => "error" in response ? response.error : undefined,
      decode: (response, action) => {
        if ("error" in response) throw Error(response.error);
        const result = response.result;
        const expected = { initialize: "initialized", replace: "ready", present: "chrome", navigate: "navigation", describe: "mutation" }[action.method];
        if (!result || result.kind !== expected || !result.model?.documentEpoch || !Number.isSafeInteger(result.model.revision) || !result.model.sourceDesignDigest) throw Error("Invalid native browsing response");
        if (result.kind === "chrome" || result.kind === "navigation") validateChrome(result.chrome);
        if (result.kind === "initialized") {
          if (action.method !== "initialize" || result.model.documentEpoch !== action.model.documentEpoch || result.model.revision !== action.model.revision || result.model.sourceDesignDigest !== action.model.sourceDesignDigest || !result.seed || result.seed.sceneKey !== action.seed.sceneKey) throw Error("Invalid native browsing initialization");
          assertWorkbenchSnapshot(result.snapshot);
          assertToolCatalog(result.toolCatalog);
        }
        return result;
      },
    });
  }
  initialize(model: BrowsingModel, seed: InteractionSeed, presentation?: WorkspaceViewPresentation) { return this.mailbox.replace<Extract<BrowsingResult, { kind: "initialized" }>>({ method: "initialize", model, seed, presentation }); }
  replace(model: BrowsingModel, seed: InteractionSeed) { return this.mailbox.replace<Extract<BrowsingResult, { kind: "ready" }>>({ method: "replace", model, seed }); }
  present(view: AuthoringView) { return this.request<Extract<BrowsingResult, { kind: "chrome" }>>({ method: "present", view }); }
  navigate(view: AuthoringView, command: BrowsingNavigationCommand, payload: unknown) { return this.request<Extract<BrowsingResult, { kind: "navigation" }>>({ method: "navigate", view, command, payload }); }
  describe(view: AuthoringView, authority: string, command: BrowsingEditCommand, payload: unknown) { return this.request<Extract<BrowsingResult, { kind: "mutation" }>>({ method: "describe", view, authority, command, payload }); }
  dispose() { this.mailbox.dispose(Error("Browsing worker was disposed")); }
  private request<T extends BrowsingResult>(action: BrowsingAction): Promise<T> { return this.mailbox.request<T>(action); }
}
function validateChrome(chrome: BrowsingChrome) {
  if (!chrome || !Array.isArray(chrome.explorer) || !Array.isArray(chrome.parameters) || !Array.isArray(chrome.problems) || !chrome.navigation || !Array.isArray(chrome.navigation.rows) || !chrome.dimensions || !Array.isArray(chrome.dimensions.entries)) throw Error("Invalid native browsing chrome");
  const freeze = (value: unknown) => { if (value && typeof value === "object") { Object.values(value).forEach(freeze); Object.freeze(value); } };
  freeze(chrome);
}
