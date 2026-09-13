// SPDX-License-Identifier: GPL-3.0-or-later
import { describe, expect, it, vi } from "vitest";
import { readFile } from "node:fs/promises";
import { resolve } from "node:path";
import initializePresentation, { BrowsingHandle, WorkbenchHandle, InteractionHandle } from "../generated/geosolve_demo_web.js";
import initializeEngine, * as engineWasm from "../../../../../packages/geosolve-engine/dist/wasm/geosolve_sketch_engine_wasm.js";
import { createEngine } from "../../../../../packages/geosolve-engine/src/index";
import { createBrowsingWorkerHandler, type BrowsingAction, type BrowsingModel, type BrowsingConstructor, type BrowsingNativeHandle, type BrowsingResult, type BrowsingWorkerRequest, type BrowsingWorkerResponse } from "./collaboration-browsing-worker";
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
  it("initializes a generated artifact without inventing source or design", async () => {
    const mock = new MockWorkbenchAdapter();
    const initial = { snapshot: await mock.snapshot(), seed: view.seed, toolCatalog: await mock.toolCatalog() };
    const generated: BrowsingModel = { documentEpoch: "generator", revision: 3, sourceDesignDigest: "generated-digest", generated: "{\"format\":\"generated-fixture\"}" };
    const inputs: unknown[] = [];
    class Native implements BrowsingNativeHandle {
      constructor(request: string) { inputs.push(JSON.parse(request)); }
      initialize() { return JSON.stringify(initial); }
      update() { return "{}"; } navigate() { return "{}"; } describe() { return "null"; } free() {}
    }
    const send = harness(Promise.resolve(Native));
    const expected = { kind: "initialized" as const, model: { documentEpoch: generated.documentEpoch, revision: generated.revision, sourceDesignDigest: generated.sourceDesignDigest }, ...initial };
    expect(await send({ method: "initialize", model: generated, seed: view.seed })).toEqual(expected);
    expect(inputs).toEqual([{ generated: generated.generated, seed: view.seed }]);
    expect(inputs[0]).not.toHaveProperty("project"); expect(inputs[0]).not.toHaveProperty("design");
    expect((await send({ method: "present", view })).model).toEqual(expected.model);
    await expect(send({ method: "initialize", model: { ...generated, ...model }, seed: view.seed }, 2)).rejects.toThrow("Invalid generated browsing model");
    expect(inputs).toHaveLength(1);
    expect(await send({ method: "replace", model: { ...generated, revision: 4 }, seed: view.seed }, 2)).toMatchObject({ kind: "ready", model: { revision: 4 } });
    expect(inputs[1]).toEqual({ generated: generated.generated, seed: view.seed });
    const transport = new FakeWorker(), browsing = new LocalBrowsingWorker(transport as unknown as Worker);
    const opened = browsing.initialize(generated, view.seed); await Promise.resolve();
    expect(transport.postMessage.mock.lastCall?.[0]).toMatchObject({ method: "initialize", model: generated, seed: view.seed });
    transport.reply({ id: 1, generation: 1, result: expected });
    expect(await opened).toEqual(expected); browsing.dispose();
  });

  it("initializes accepted chrome once and orders later views behind native startup", async () => {
    const mock = new MockWorkbenchAdapter();
    const initial = { snapshot: await mock.snapshot(), seed: { ...view.seed, enriched: true }, toolCatalog: await mock.toolCatalog() };
    const events: string[] = [];
    let resolveConstructor!: (constructor: BrowsingConstructor) => void;
    class Native implements BrowsingNativeHandle {
      constructor(request: string) { events.push("construct"); expect(JSON.parse(request)).toEqual({ project: model.project, design: model.design, seed: view.seed }); }
      initialize() { events.push("initialize"); return JSON.stringify(initial); }
      update() { events.push("update"); return "{}"; }
      navigate(): string { throw Error("unused"); }
      describe(): string { throw Error("unused"); }
      free() { events.push("free"); }
    }
    const send = harness(new Promise<BrowsingConstructor>(resolve => { resolveConstructor = resolve; }));
    const opening = send({ method: "initialize", model, seed: view.seed });
    const presenting = send({ method: "present", view });
    await Promise.resolve(); expect(events).toEqual([]);
    resolveConstructor(Native);
    expect(await opening).toEqual({ kind: "initialized", model: { documentEpoch: model.documentEpoch, revision: model.revision, sourceDesignDigest: model.sourceDesignDigest }, ...initial });
    expect((await presenting).kind).toBe("chrome");
    expect(events).toEqual(["construct", "initialize", "update"]);
    await send({ method: "replace", model: { ...model, revision: 2 }, seed: view.seed }, 2);
    expect(events).toEqual(["construct", "initialize", "update", "construct", "free"]);
  });

  it("forwards saved personal presentation through browser initialization", async () => {
    const mock = new MockWorkbenchAdapter();
    const initial = { snapshot: await mock.snapshot(), seed: view.seed, toolCatalog: await mock.toolCatalog() };
    const presentation = { hiddenRows: ["managed:edge"], constructionVisible: false, dimensions: { mode: "all" as const, pins: ["dimension:key"] } };
    const inputs: unknown[] = [];
    class Native implements BrowsingNativeHandle {
      constructor(request: string) { inputs.push(JSON.parse(request)); }
      initialize() { return JSON.stringify(initial); }
      update() { return "{}"; } navigate() { return "{}"; } describe() { return "null"; } free() {}
    }
    const send = harness(Promise.resolve(Native));
    expect(await send({ method: "initialize", model, seed: view.seed, presentation })).toMatchObject({ kind: "initialized", snapshot: initial.snapshot });
    expect(inputs).toEqual([{ project: model.project, design: model.design, seed: view.seed, presentation }]);
    const transport = new FakeWorker(), browsing = new LocalBrowsingWorker(transport as unknown as Worker);
    const opened = browsing.initialize(model, view.seed, presentation); await Promise.resolve();
    expect(transport.postMessage.mock.lastCall?.[0]).toMatchObject({ method: "initialize", presentation });
    transport.reply({ id: 1, generation: 1, result: { ...initial, kind: "initialized", model } });
    await opened; browsing.dispose();
  });

  it("retains the prior accepted handle and frees a refused initialization candidate", async () => {
    const events: string[] = [];
    let fail = true;
    class Native implements BrowsingNativeHandle {
      initialize() { if (fail) throw Error("initial chrome rejected"); return JSON.stringify({ snapshot: {}, seed: { sceneKey: "foreign" }, toolCatalog: {} }); }
      update() { events.push("update"); return "{}"; }
      navigate(): string { throw Error("unused"); }
      describe(): string { throw Error("unused"); }
      free() { events.push("free"); }
    }
    const send = harness(Promise.resolve(Native));
    await send({ method: "replace", model, seed: view.seed });
    await expect(send({ method: "initialize", model: { ...model, revision: 2 }, seed: view.seed }, 2)).rejects.toThrow("initial chrome rejected");
    expect(events).toEqual(["free"]);
    expect((await send({ method: "present", view })).kind).toBe("chrome");
    fail = false;
    await expect(send({ method: "initialize", model: { ...model, revision: 2 }, seed: view.seed }, 2)).rejects.toThrow("Invalid native browsing initialization");
    expect(events).toEqual(["free", "update", "free"]);
    await expect(send({ method: "initialize", model, seed: view.seed })).rejects.toThrow("obsolete");
    expect((await send({ method: "present", view })).kind).toBe("chrome");
    class LegacyNative implements BrowsingNativeHandle {
      update() { return "{}"; } navigate() { return "{}"; } describe() { return "null"; } free() {}
    }
    const legacy = harness(Promise.resolve(LegacyNative));
    await legacy({ method: "replace", model, seed: view.seed });
    await expect(legacy({ method: "initialize", model, seed: view.seed }, 2)).rejects.toThrow("initialization is unavailable");
    expect((await legacy({ method: "present", view })).kind).toBe("chrome");
  });

  it("replaces initialization generations and ignores late initialized replies", async () => {
    const mock = new MockWorkbenchAdapter();
    const initial = { snapshot: await mock.snapshot(), seed: view.seed, toolCatalog: await mock.toolCatalog() };
    const transport = new FakeWorker(), browsing = new LocalBrowsingWorker(transport as unknown as Worker);
    const first = expect(browsing.initialize(model, view.seed)).rejects.toThrow("Browsing model was replaced");
    await Promise.resolve();
    expect(transport.postMessage.mock.lastCall?.[0]).toMatchObject({ method: "initialize", generation: 1, model, seed: view.seed });
    const queued = expect(browsing.present(view)).rejects.toThrow("Browsing model was replaced");
    const replacement = { ...model, revision: 2 };
    const next = browsing.initialize(replacement, view.seed);
    await Promise.resolve(); await first; await queued;
    const current = transport.postMessage.mock.lastCall![0] as BrowsingWorkerRequest;
    expect(current).toMatchObject({ method: "initialize", generation: 2 });
    transport.reply({ id: 1, generation: 1, result: { kind: "initialized", model, ...initial } });
    transport.reply({ id: current.id, generation: 2, result: { kind: "initialized", model: replacement, ...initial } });
    expect(await next).toEqual({ kind: "initialized", model: replacement, ...initial });
    const pending = expect(browsing.initialize(replacement, view.seed)).rejects.toThrow("Browsing worker was disposed");
    await Promise.resolve(); browsing.dispose(); await pending;
    expect(transport.terminate).toHaveBeenCalledTimes(1);
  });

  it("rejects malformed initialized payloads at the native response boundary", async () => {
    const mock = new MockWorkbenchAdapter();
    const initial = { snapshot: await mock.snapshot(), seed: view.seed, toolCatalog: await mock.toolCatalog() };
    const invalid = [
      { ...initial, seed: { sceneKey: "foreign" } },
      { ...initial, snapshot: { ...initial.snapshot, revision: Number.NaN } },
      { ...initial, toolCatalog: { ...initial.toolCatalog, sections: [] } },
    ];
    for (const payload of invalid) {
      const transport = new FakeWorker(), browsing = new LocalBrowsingWorker(transport as unknown as Worker);
      const opened = expect(browsing.initialize(model, view.seed)).rejects.toThrow();
      await Promise.resolve();
      transport.reply({ id: 1, generation: 1, result: { kind: "initialized", model, ...payload } });
      await opened;
      expect(transport.terminate).toHaveBeenCalledTimes(1);
      await expect(browsing.present(view)).rejects.toThrow();
    }
  });

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


it("retains the selected circle Inspector across genuine reconstructed accepted namespaces",async()=>{
  await initializePresentation({module_or_path:await readFile(resolve(process.cwd(),"src/generated/geosolve_demo_web_bg.wasm"))});
  const engine=await createEngine({wasmModule:{...engineWasm,default:initializeEngine},wasm:await readFile(resolve(process.cwd(),"../../../packages/geosolve-engine/dist/wasm/geosolve_sketch_engine_wasm_bg.wasm"))});
  const workbenches:WorkbenchHandle[]=[];let local:InteractionHandle|undefined,browsing:BrowsingHandle|undefined;
  try{
    const compiled=JSON.parse(await readFile(resolve(process.cwd(),"../../geosolve-sketch-engine/tests/fixtures/authoring-radius-5.json"),"utf8"));
    const project=engine.compileProject({project:"replacement-inspector",compiled,customFiles:{},artifacts:{},lock:{format:"geosolve-lock-v1",modules:{}}});
    const session=engine.openEditableSession(project),accepted={project:session.exportProject(),design:session.exportDesign()};
    const seed=()=>{
      const workbench=new WorkbenchHandle(JSON.stringify({version:2,persistedProject:accepted.project}));workbenches.push(workbench);
      workbench.dispatch(JSON.stringify({version:2,command:"workspace.project.apply",payload:accepted}));
      return JSON.parse(workbench.interactionSnapshot()).seed;
    };
    const first=seed(),second=seed(),firstPoint=JSON.parse(first.scene).points[0],secondPoint=JSON.parse(second.scene).points[0];
    expect(firstPoint.id).not.toBe(secondPoint.id);
    local=new InteractionHandle(JSON.stringify(first));
    local.pointer(JSON.stringify({version:2,phase:"down",pointerId:1,x:firstPoint.screen_position.x,y:firstPoint.screen_position.y,buttons:1,modifiers:{alt:false,ctrl:false,meta:false,shift:false}}));
    const before=JSON.parse(local.state());
    const replacement=JSON.parse(local.replace(JSON.stringify({seed:second,preserveSelection:true})));
    expect(replacement.state.selection).toEqual([{Point:secondPoint.id}]);
    expect(replacement.state.viewport).toEqual(before.viewport);
    browsing=new BrowsingHandle(JSON.stringify({...accepted,seed:second}));
    const chrome=JSON.parse(browsing.update(JSON.stringify(replacement.state))) as WorkbenchSnapshot;
    // Selecting its center is a partial selection of the circle declaration.
    expect(chrome.navigation?.rows).toContainEqual({id:"managed:bore",state:"partial"});
    expect(chrome.parameters.some(parameter=>parameter.label.includes("radius")&&parameter.value==="5")).toBe(true);
    expect(session.exportProject()).toBe(accepted.project);expect(session.exportDesign()).toEqual(accepted.design);
    session.dispose();
  }finally{browsing?.free();local?.free();for(const workbench of workbenches)workbench.free();engine.dispose();}
});
