// SPDX-License-Identifier: GPL-3.0-or-later
import initializeWasm, { BrowsingHandle } from "../generated/geosolve_demo_web.js";
import type { WorkbenchSnapshot } from "./adapter";
import type { AuthoringModel, AuthoringModelIdentity, AuthoringView } from "./collaboration-authoring-worker";
import type { InteractionSeed, InteractionState } from "./local-interaction-worker";
import type { ManagedSketchMutation } from "./managed-compiler";

export type BrowsingNavigationCommand = "navigation.rows.select" | "navigation.source.select";
export type BrowsingEditCommand = "parameter.edit" | "dimensions.edit" | "authoring.metadata.set" | "declaration.move" | "declaration.delete";

export type BrowsingChrome = Pick<WorkbenchSnapshot, "explorer" | "navigation" | "dimensions" | "parameters" | "problems"> & {
  readonly authoringDocument: WorkbenchSnapshot["authoringDocument"] | null;
  readonly selection: WorkbenchSnapshot["selection"] | null;
  readonly selectedGeometryRole: WorkbenchSnapshot["presentation"]["selectedGeometryRole"] | null;
};
export type BrowsingAction = { method: "replace"; model: AuthoringModel; seed: InteractionSeed }
  | { method: "present"; view: AuthoringView }
  | { method: "navigate"; view: AuthoringView; command: BrowsingNavigationCommand; payload: unknown }
  | { method: "describe"; view: AuthoringView; authority: string; command: BrowsingEditCommand; payload: unknown };
export type BrowsingWorkerRequest = BrowsingAction & { id: number; generation: number };
export type BrowsingResult = { readonly kind: "ready"; readonly model: AuthoringModelIdentity }
  | { readonly kind: "chrome"; readonly model: AuthoringModelIdentity; readonly view: AuthoringView; readonly chrome: BrowsingChrome }
  | { readonly kind: "navigation"; readonly model: AuthoringModelIdentity; readonly view: AuthoringView; readonly chrome: BrowsingChrome; readonly state: InteractionState }
  | { readonly kind: "mutation"; readonly model: AuthoringModelIdentity; readonly view: AuthoringView; readonly mutation: ManagedSketchMutation | null };
export type BrowsingWorkerResponse = { id: number; generation: number; result: BrowsingResult }
  | { id: number; generation: number; error: string };
export interface BrowsingNativeHandle { update(state: string): string; navigate(request: string): string; describe(request: string): string; free(): void }
export type BrowsingConstructor = new (request: string) => BrowsingNativeHandle;

/** The native constructor independently reconstructs accepted project/design once.
 * Updates use only retained native presentation. No compiler/edit dispatch is
 * reachable from this worker, and no browser navigation frame waits for it.
 */
export function createBrowsingWorkerHandler(constructor: Promise<BrowsingConstructor>, reply: (response: BrowsingWorkerResponse) => void) {
  let handle: BrowsingNativeHandle | undefined, model: AuthoringModelIdentity | undefined;
  let generation = 0, sceneKey: unknown, tail = Promise.resolve();
  void constructor.catch(() => undefined);
  return ({ data }: MessageEvent<BrowsingWorkerRequest>) => {
    const run = async () => {
      try {
        if (!Number.isSafeInteger(data?.id) || data.id < 1 || !Number.isSafeInteger(data.generation) || data.generation < 1) throw Error("Invalid browsing worker request");
        let result: BrowsingResult;
        if (data.method === "replace") {
          if (data.generation <= generation || !data.model.documentEpoch || !Number.isSafeInteger(data.model.revision) || data.model.revision < 0 || !data.model.sourceDesignDigest) throw Error("Invalid or obsolete browsing model");
          const Constructor = await constructor;
          const next = new Constructor(JSON.stringify({ project: data.model.project, design: data.model.design, seed: data.seed }));
          handle?.free(); handle = next; generation = data.generation; sceneKey = data.seed.sceneKey;
          model = { documentEpoch: data.model.documentEpoch, revision: data.model.revision, sourceDesignDigest: data.model.sourceDesignDigest };
          result = { kind: "ready", model };
        } else {
          if (!handle || !model || generation !== data.generation) throw Error("Browsing request belongs to an obsolete accepted model");
          if (data.view.seed.sceneKey !== sceneKey || data.view.state.sceneKey !== sceneKey) throw Error("Browsing view belongs to another scene");
          if (data.method === "present") result = { kind: "chrome", model, view: data.view, chrome: JSON.parse(handle.update(JSON.stringify(data.view.state))) };
          else if (data.method === "navigate") {
            const navigation = JSON.parse(handle.navigate(JSON.stringify({ state: data.view.state, command: data.command, payload: data.payload })));
            result = { kind: "navigation", model, view: data.view, chrome: navigation.chrome, state: navigation.state };
          } else if (data.method === "describe") result = { kind: "mutation", model, view: data.view, mutation: JSON.parse(handle.describe(JSON.stringify({ state: data.view.state, authority: data.authority, command: data.command, payload: data.payload }))) };
          else throw Error("Unsupported browsing operation");
        }
        reply({ id: data.id, generation: data.generation, result });
      } catch (error) { reply({ id: data?.id, generation: data?.generation, error: error instanceof Error ? error.message : String(error) }); }
    };
    tail = tail.then(run, run);
  };
}
if (typeof document === "undefined" && typeof self !== "undefined") {
  self.onmessage = createBrowsingWorkerHandler(initializeWasm().then(() => BrowsingHandle), response => self.postMessage(response));
}
