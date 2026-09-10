// SPDX-License-Identifier: GPL-3.0-or-later
import { describe, expect, it, vi } from "vitest";
import { readFile } from "node:fs/promises";
import { resolve } from "node:path";
import initializePresentation, { BrowsingHandle, WorkbenchHandle, InteractionHandle } from "../generated/geosolve_demo_web.js";
import initializeEngine, * as engineWasm from "../../../../../packages/geosolve-engine/dist/wasm/geosolve_sketch_engine_wasm.js";
import { createEngine } from "../../../../../packages/geosolve-engine/src/index";
import { createBrowsingWorkerHandler, type BrowsingAction, type BrowsingConstructor, type BrowsingNativeHandle, type BrowsingResult, type BrowsingWorkerRequest, type BrowsingWorkerResponse } from "./collaboration-browsing-worker";
import { LocalBrowsingWorker } from "./collaboration-browsing-adapter";
import type { AuthoringModel, AuthoringView } from "./collaboration-authoring-worker";
import { LocalInteractionWorker } from "./local-interaction-adapter";
import type { LocalInteractionResponse } from "./local-interaction-worker";
import { MockWorkbenchAdapter } from "./mock-adapter";
import type { ParameterEntry, WorkbenchSnapshot } from "./adapter";

class FakeWorker extends EventTarget {
  postMessage = vi.fn(); terminate = vi.fn();
  reply(response: BrowsingWorkerResponse | LocalInteractionResponse) { this.dispatchEvent(new MessageEvent("message", { data: response })); }
}
const model: AuthoringModel = { documentEpoch: "document", revision: 1, sourceDesignDigest: "digest", project: "{}", design: { format: "geosolve-design-v1", project: "p", generated: {}, overrides: {} } };
const view: AuthoringView = { seed: { sceneKey: "scene" }, state: { sceneKey: "scene", selected: 1 } };
function harness(constructor: Promise<BrowsingConstructor>) {
  let id = 0;
  const requests = new Map<number, { resolve(value: BrowsingResult): void; reject(error: Error): void }>();
  const handler = createBrowsingWorkerHandler(constructor, response => {
    const waiter = requests.get(response.id)!; requests.delete(response.id);
    if ("error" in response) waiter.reject(Error(response.error)); else waiter.resolve(response.result);
  });
  return (action: BrowsingAction, generation = 1) => new Promise<BrowsingResult>((resolve, reject) => {
    const request = { ...action, id: ++id, generation } satisfies BrowsingWorkerRequest;
    requests.set(id, { resolve, reject }); handler(new MessageEvent("message", { data: request }));
  });
}
async function chrome() {
  const snapshot = await new MockWorkbenchAdapter().snapshot();
  return { explorer: snapshot.explorer, navigation: { authority: "local", selectionKey: "selection", rows: [], sources: [], itemCount: 0, canNavigateSource: false }, dimensions: { mode: "focused" as const, entries: [], parameters: [], pinCount: 0 }, parameters: snapshot.parameters, problems: snapshot.problems, selection: null, authoringDocument: null, selectedGeometryRole: null };
}
function parameterMeaning({ rowKey: _rowKey, metadata, ...parameter }: ParameterEntry) {
  if (!metadata) return parameter;
  const { authority: _authority, ...meaning } = metadata;
  return { ...parameter, metadata: meaning };
}

describe("independent native browsing", () => {
  it("preserves navigation and edit descriptions between coalesced views", async () => {
    const transport = new FakeWorker(), browsing = new LocalBrowsingWorker(transport as unknown as Worker);
    const opened = browsing.replace(model, view.seed); await Promise.resolve();
    transport.reply({ id: 1, generation: 1, result: { kind: "ready", model } }); await opened;
    const pending = [
      browsing.present(view),
      browsing.navigate(view, "navigation.rows.select", { authority: "navigation", ids: ["row"], mode: "replace" }),
      browsing.present(view),
      browsing.describe(view, "authority", "parameter.edit", { id: "parameter", value: "24" }),
      browsing.present(view),
    ];
    const methods = ["present", "navigate", "present", "describe", "present"];
    for (const [index, method] of methods.entries()) {
      await Promise.resolve();
      expect(transport.postMessage.mock.lastCall?.[0].method).toBe(method);
      const result: BrowsingResult = method === "describe" ? { kind: "mutation", model, view, mutation: null }
        : method === "navigate" ? { kind: "navigation", model, view, state: view.state, chrome: await chrome() }
        : { kind: "chrome", model, view, chrome: await chrome() };
      transport.reply({ id: index + 2, generation: 1, result });
    }
    expect((await Promise.all(pending)).map(result => result.kind)).toEqual(["chrome", "navigation", "chrome", "mutation", "chrome"]);
    browsing.dispose();
  });

  it("coalesces only pending views, invalidates old model replies, and never holds navigation", async () => {
    const transport = new FakeWorker(), navTransport = new FakeWorker();
    const browsing = new LocalBrowsingWorker(transport as unknown as Worker), navigation = new LocalInteractionWorker(navTransport as unknown as Worker);
    const opened = browsing.replace(model, view.seed); await Promise.resolve();
    transport.reply({ id: 1, generation: 1, result: { kind: "ready", model } }); await opened;
    const first = browsing.present(view), secondView = { ...view, state: { ...view.state, selected: 2 } }, second = browsing.present(secondView);
    await Promise.resolve();
    expect(transport.postMessage.mock.lastCall?.[0].view).toEqual(secondView);
    const nav = navigation.update("wheel", { deltaY: -90 });
    const frame = (await new MockWorkbenchAdapter().snapshot()).frame;
    navTransport.reply({ id: 1, result: { frame, state: {}, selectionChanged: false, serverFrameCompatible: false } });
    expect((await nav)?.frame).toBe(frame);
    transport.reply({ id: 2, generation: 1, result: { kind: "chrome", model, view: secondView, chrome: await chrome() } });
    const result = await first; expect(result).toBe(await second); expect(Object.isFrozen(result.chrome.parameters)).toBe(true);
    const stale = expect(browsing.present(view)).rejects.toThrow("Browsing model was replaced"); await Promise.resolve();
    const replacement = { ...model, revision: 2 };
    const next = browsing.replace(replacement, view.seed); await Promise.resolve(); await stale;
    transport.reply({ id: 3, generation: 1, result: { kind: "chrome", model, view, chrome: await chrome() } });
    transport.reply({ id: 4, generation: 2, result: { kind: "ready", model: replacement } });
    expect((await next).model.revision).toBe(2);
    browsing.dispose(); navigation.dispose();
  });

  it("reuses actual native chrome across source namespaces and rebuilds only on model replacement", async () => {
    await initializePresentation({ module_or_path: await readFile(resolve(process.cwd(), "src/generated/geosolve_demo_web_bg.wasm")) });
    const engine = await createEngine({ wasmModule: { ...engineWasm, default: initializeEngine }, wasm: await readFile(resolve(process.cwd(), "../../../packages/geosolve-engine/dist/wasm/geosolve_sketch_engine_wasm_bg.wasm")) });
    let source: WorkbenchHandle | undefined, local: InteractionHandle | undefined;
    const handles: BrowsingNativeHandle[] = []; let constructions = 0, updates = 0, frees = 0;
    class Counted implements BrowsingNativeHandle {
      private readonly native: BrowsingHandle;
      constructor(input: string) { ++constructions; this.native = new BrowsingHandle(input); handles.push(this); }
      update(input: string) { ++updates; return this.native.update(input); }
      navigate(input: string) { return this.native.navigate(input); }
      describe(input: string) { return this.native.describe(input); }
      free() { ++frees; this.native.free(); handles.splice(handles.indexOf(this), 1); }
    }
    try {
      const compiled = JSON.parse(await readFile(resolve(process.cwd(), "../../geosolve-sketch-engine/tests/fixtures/point-gesture-constrained.json"), "utf8"));
      const project = engine.compileProject({ project: "browsing-native", compiled, customFiles: {}, artifacts: {}, lock: { format: "geosolve-lock-v1", modules: {} } });
      const session = engine.openEditableSession(project);
      const accepted: AuthoringModel = { ...model, project: session.exportProject(), design: session.exportDesign(), sourceDesignDigest: session.sourceDesignDigest() };
      source = new WorkbenchHandle(JSON.stringify({ version: 2, persistedProject: accepted.project }));
      source.dispatch(JSON.stringify({ version: 2, command: "workspace.project.apply", payload: { project: accepted.project, design: accepted.design } }));
      const pair = JSON.parse(source.interactionSnapshot());
      local = new InteractionHandle(JSON.stringify(pair.seed));
      const send = harness(Promise.resolve(Counted));
      await send({ method: "replace", model: accepted, seed: pair.seed });
      const original = { project: source.exportProject(), design: source.exportWorkspaceDesign() };
      const point = JSON.parse(pair.seed.scene).points[0].screen_position;
      local.pointer(JSON.stringify({ version: 2, phase: "down", pointerId: 1, x: point.x, y: point.y, buttons: 1, modifiers: { alt: false, ctrl: false, meta: false, shift: false } }));
      for (let index = 0; index < 4; ++index) {
        local.wheel(JSON.stringify({ version: 2, x: 300, y: 250, deltaX: 0, deltaY: -30, ctrl: false }));
        const state = JSON.parse(local.state()), currentView = { seed: pair.seed, state };
        const expected = JSON.parse(source.interactionApply(JSON.stringify(state))) as WorkbenchSnapshot;
        const actual = await send({ method: "present", view: currentView });
        expect(actual.kind).toBe("chrome");
        if (actual.kind === "chrome") {
          expect(actual.view).toEqual(currentView);
          expect(actual.chrome.explorer).toEqual(expected.explorer);
          expect(actual.chrome.navigation?.rows).toEqual(expected.navigation?.rows);
          expect(actual.chrome.navigation?.sources).toEqual(expected.navigation?.sources);
          expect(actual.chrome.navigation?.authority).not.toBe(expected.navigation?.authority);
          expect(actual.chrome.selection?.source).toEqual(expected.selection?.source);
          expect(actual.chrome.selection?.label).toEqual(expected.selection?.label);
          expect(actual.chrome.parameters.map(parameterMeaning)).toEqual(expected.parameters.map(parameterMeaning));
          for (const [rowIndex, parameter] of actual.chrome.parameters.entries()) {
            expect(parameter.rowKey).toBeTruthy();
            expect(parameter.rowKey).not.toBe(expected.parameters[rowIndex].rowKey);
            if (parameter.metadata) {
              expect(parameter.metadata.authority).toBeTruthy();
              expect(parameter.metadata.authority).not.toBe(expected.parameters[rowIndex].metadata?.authority);
            }
          }
          expect(actual.chrome.selectedGeometryRole).toEqual(expected.presentation.selectedGeometryRole ?? null);
          expect(actual.chrome).not.toHaveProperty("frame"); expect(actual.chrome).not.toHaveProperty("source");
        }
        expect(constructions).toBe(1); expect(updates).toBe(index + 1);
      }
      expect({ project: source.exportProject(), design: source.exportWorkspaceDesign() }).toEqual(original);
      const currentView = { seed: pair.seed, state: JSON.parse(local.state()) };
      const presented = await send({ method: "present", view: currentView });
      if (presented.kind !== "chrome") throw Error("Expected current chrome");
      const authority = presented.chrome.authoringDocument!.authority;
      const parameter = presented.chrome.parameters.find(row => row.editable)!;
      const described = await send({ method: "describe", view: currentView, authority, command: "parameter.edit", payload: { id: parameter.id, value: "24" } });
      expect(described).toMatchObject({ kind: "mutation", model: { sourceDesignDigest: accepted.sourceDesignDigest }, view: currentView,
        mutation: { mutation: "set_values", values: [{ declaration: "length", path: ["value"], value: { kind: "unit", value: { value: 24 } } }] } });
      await expect(send({ method: "describe", view: currentView, authority: "stale", command: "parameter.edit", payload: { id: parameter.id, value: "24" } })).rejects.toThrow("older source revision");
      const navigated = await send({ method: "navigate", view: currentView, command: "navigation.rows.select", payload: { authority: presented.chrome.navigation!.authority, ids: ["managed:bar"], mode: "replace" } });
      if (navigated.kind !== "navigation") throw Error("Expected native navigation");
      expect(navigated.chrome.navigation?.rows).toContainEqual({ id: "managed:bar", state: "selected" });
      const restored = JSON.parse(local.restoreSelection(JSON.stringify({ expected: currentView.state, state: navigated.state })));
      expect(restored.state).toEqual(navigated.state);
      const sourceChrome = JSON.parse(source.interactionApply(JSON.stringify(restored.state))) as WorkbenchSnapshot;
      expect(sourceChrome.navigation?.rows).toEqual(navigated.chrome.navigation?.rows);
      local.wheel(JSON.stringify({ version: 2, x: 300, y: 250, deltaX: 0, deltaY: -30, ctrl: false }));
      const afterZoom = local.state();
      expect(() => local!.restoreSelection(JSON.stringify({ expected: currentView.state, state: navigated.state }))).toThrow("obsolete local view");
      expect(local.state()).toBe(afterZoom);
      const withPresence = JSON.parse(local.presence(JSON.stringify({ sceneKey: pair.seed.sceneKey, presence: [{ userId: "peer", clientId: "peer-tab", sequence: 1, cursor: [5, 0], selection: ["bar"] }] })));
      const overlay = withPresence.frame.scene.items.filter((item: { layer: string }) => item.layer === "presence");
      expect(overlay).toHaveLength(5);
      expect(overlay.every((item: { interactive: boolean; semanticKey: string | null }) => !item.interactive && item.semanticKey === null)).toBe(true);
      expect(local.state()).toBe(afterZoom);
      expect(constructions).toBe(1);
      expect({ project: source.exportProject(), design: source.exportWorkspaceDesign() }).toEqual(original);
      await expect(send({ method: "present", view: { seed: pair.seed, state: { ...JSON.parse(local.state()), sceneKey: "stale" } } })).rejects.toThrow("another scene");
      await send({ method: "replace", model: { ...accepted, revision: 2 }, seed: pair.seed }, 2);
      expect(constructions).toBe(2); expect(frees).toBe(1);
      await expect(send({ method: "present", view: { seed: pair.seed, state: JSON.parse(local.state()) } }, 1)).rejects.toThrow("obsolete");
      session.dispose();
    } finally { for (const handle of [...handles]) handle.free(); local?.free(); source?.free(); engine.dispose(); }
  });
});
