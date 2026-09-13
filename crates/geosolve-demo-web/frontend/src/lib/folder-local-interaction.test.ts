// SPDX-License-Identifier: GPL-3.0-or-later
import { afterEach, describe, expect, it, vi } from "vitest";
import { FolderWorkbenchAdapter } from "./folder-adapter";
import { MockWorkbenchAdapter } from "./mock-adapter";
import { isCanvasOnlySnapshot, type PointerSample, type WorkbenchSnapshot } from "./adapter";
import { folderBrowser, folderModel } from "./folder-test-support";
import type { AuthoringModel, AuthoringPreview, AuthoringView, LocalAuthoringClient } from "./collaboration-authoring-adapter";

const point = (phase: PointerSample["phase"], x = 20, buttons = phase === "up" ? 0 : 1): PointerSample => ({ version: 2, phase, pointerId: 7, x, y: 30, buttons, modifiers: { alt: false, ctrl: false, meta: false, shift: false } });
const wheel = { version: 2 as const, x: 30, y: 20, deltaX: 0, deltaY: -90, ctrl: false };
function deferred<T = void>() { let resolve!: (value: T) => void; const promise = new Promise<T>(done => { resolve = done; }); return { promise, resolve }; }
const handles: FolderWorkbenchAdapter[] = [];
async function harness() {
  let initializationGate: Promise<void> | undefined;
  const fixture = await new MockWorkbenchAdapter().snapshot(), browser = await folderBrowser(fixture, async () => { await initializationGate; });
  fixture.navigation = { authority: "navigation", selectionKey: "none", rows: [], sources: [], itemCount: 0, canNavigateSource: true };
  const viewport = { screen_size: [1000, 700] as const, model_center: [0, 0] as const, pixels_per_model_unit: 10 };
  const target = { target: "point" as const, address: { project: "doc", owner: { address: { owner: "direct_declaration" as const, declaration: "circle" }, allocation: 1, generation: 1 }, output: [], field: "point" as const } };
  browser.local.authoringPointer = vi.fn(async input => ({ target, viewport, position: [input.x, input.y] as const }));
  browser.local.projectPrediction = vi.fn(async input => ({ ...browser.update(), frame: { ...fixture.frame, ariaLabel: input.presentation, scene: { ...fixture.frame.scene, provenance: { scene: "provisional", camera: String(input.view.state.camera) } } } }));
  let model!: AuthoringModel, view!: AuthoringView, x = 0;
  const samples: { sequence: number; position: readonly [number, number] }[] = [];
  const preview = (): AuthoringPreview => ({ kind: "preview", model, view, presentation: `native:${x}`, frame: { ...fixture.frame, ariaLabel: `prediction-${x}`, scene: { ...fixture.frame.scene, provenance: { scene: "provisional", x: String(x), camera: String(view.state.camera) } } } });
  const authoring: LocalAuthoringClient = {
    replace: vi.fn(async next => { model = next; return { kind: "ready" as const, model }; }),
    beginPoint: vi.fn(async input => { view = input.view; return preview(); }),
    advancePoint: vi.fn(async (sample, next) => { samples.push(sample); view = next; x = sample.position[0]; return preview(); }),
    finishPoint: vi.fn(async () => ({ kind: "point" as const, model, terminal: { command: { basis: model.sourceDesignDigest, gesture_id: 1, target, viewport, samples }, accepted_position: [x, 30] as const } })),
    beginConstruction: vi.fn(async input => { view = input.view; return preview(); }),
    advanceConstruction: vi.fn(async (_sample, next) => { view = next; return preview(); }),
    finishConstruction: vi.fn(), beginOperation: vi.fn(), advanceOperation: vi.fn(), pickOperationSelection: vi.fn(), finishOperation: vi.fn(),
    render: vi.fn(async next => { view = next; return preview(); }), cancel: vi.fn(async () => ({ kind: "cancelled" as const, model })), dispose: vi.fn(),
  };
  const requests: Array<{ method: string; input?: { kind?: string; command?: unknown }; baseHash: string; authority: { revision: number }; interaction?: unknown; localInteraction?: unknown }> = [];
  let wait: Promise<void> | undefined, release = () => {}, disk = "disk-A", revision = 1, failedDraft = false;
  vi.stubGlobal("fetch", vi.fn(async (_url, init) => {
    const request = JSON.parse(init.body); requests.push(request); if (wait) await wait;
    if (!["session.join", "snapshot"].includes(request.method) && request.authority?.revision !== revision) throw Error(`stale revision ${request.authority?.revision}; expected ${revision}`);
    return { ok: true, json: async () => ({ result: { ...folderModel(fixture, revision, disk === "disk-A" ? "accepted-A" : "accepted-B"), ...(failedDraft ? { status: "retained", source: { ...fixture.source, dirty: true }, problems: [{ id: "source", detail: "Invalid source", path: "sketch.ts" }] } : {}) }, state: { ok: true, authority: { epoch: "epoch", lease: 1, revision }, editor: { canEdit: true }, sequence: revision, currentHash: disk, acceptedHash: disk, status: "saved", diagnostics: [], paths: { folder: "/project", source: "/project/sketch.ts" } } }) };
  }));
  let events!: EventTarget;
  vi.stubGlobal("EventSource", class extends EventTarget { constructor() { super(); events = this; } close() {} });
  const adapter = new FolderWorkbenchAdapter("test-token", () => browser.local, { createBrowsing: browser.createBrowsing, authoring }); handles.push(adapter);
  const install = async (snapshot: WorkbenchSnapshot) => { const prepared = await adapter.prepareSnapshot(snapshot); adapter.installSnapshot(prepared); return prepared; };
  await install(await adapter.construct());
  return { adapter, ...browser, authoring, samples, requests, install, fixture,
    activity: (busy: boolean) => events.dispatchEvent(new MessageEvent("activity", { data: JSON.stringify({ busy }) })),
    failSource: () => { failedDraft = true; revision++; },
    pauseInitialization: (gate: Promise<void>) => { initializationGate = gate; },
    changeDisk: () => { disk = "disk-B"; }, advanceRevision: () => { revision++; },
    pause: () => { wait = new Promise(resolve => { release = resolve; }); }, resume: () => { wait = undefined; release(); },
  };
}
afterEach(() => { for (const adapter of handles.splice(0)) adapter.dispose(); vi.unstubAllGlobals(); vi.useRealTimers(); sessionStorage.clear(); localStorage.clear(); });

describe("folder engine host and local browser", () => {
  it("navigates and highlights with no RPC while a server edit is stalled", async () => {
    const h = await harness(); h.pause();
    const edit = h.adapter.dispatch({ version: 2, command: "parameter.edit", payload: { id: "r", value: "12" } });
    await vi.waitFor(() => expect(h.requests).toHaveLength(2));
    const zoomed = (await h.adapter.wheel(wheel))!, hovered = (await h.adapter.pointer(point("move", 40, 0)))!;
    expect(h.adapter.responsiveCanvas).toBe(true); expect(isCanvasOnlySnapshot(zoomed)).toBe(true); expect(isCanvasOnlySnapshot(hovered)).toBe(true);
    expect(zoomed.frame.scene.provenance.camera).toBe("1"); expect(h.requests).toHaveLength(2);
    h.resume(); const installed = await h.install(await edit);
    expect(installed.frame.scene.provenance.camera).toBe("1");
    expect(h.requests[1].method).toBe("authoring.mutation");
    expect(h.requests.every(request => request.interaction === undefined && request.localInteraction === undefined)).toBe(true);
  });
  it("keeps the old canvas responsive during native initialization of another accepted model", async () => {
    const h = await harness(), gate = deferred();
    h.pauseInitialization(gate.promise); h.changeDisk(); h.advanceRevision();
    const loading = h.adapter.snapshot();
    await vi.waitFor(() => expect(h.browsers).toHaveLength(2));
    expect((await h.adapter.wheel(wheel))!.frame.scene.provenance.camera).toBe("1");
    expect(h.browsers[0].dispose).not.toHaveBeenCalled();
    gate.resolve(); await h.install(await loading);
    expect((await h.adapter.wheel(wheel))!.frame.scene.provenance.scene).toBe("accepted-B");
  });
  it("coalesces local Inspector refresh and rejects stale view replies without rewinding camera or selection", async () => {
    const h = await harness(), browser = h.browsers[0], gate = deferred();
    const present = vi.mocked(browser.present).getMockImplementation()!;
    vi.mocked(browser.present).mockImplementation(async view => { await gate.promise; return present(view); });
    let latest: WorkbenchSnapshot | undefined;
    const stop = h.adapter.subscribe(snapshot => { if (snapshot) latest = snapshot; });
    await h.adapter.pointer(point("down", 25)); await h.adapter.pointer(point("up", 25));
    await h.adapter.pointer(point("down", 35)); await h.adapter.pointer(point("up", 35)); await h.adapter.wheel(wheel);
    expect(h.requests).toHaveLength(1); gate.resolve();
    await vi.waitFor(() => expect(latest?.frame.scene.provenance).toMatchObject({ camera: "1", selection: "point-35" }));
    expect(h.local.replace).not.toHaveBeenCalled(); stop();
  });
  it("keeps an explicit edit on its installed basis while local browsing is held", async () => {
    const h = await harness(), gate = deferred(), browser = h.browsers[0], present = vi.mocked(browser.present).getMockImplementation()!;
    vi.mocked(browser.present).mockImplementation(async view => { await gate.promise; return present(view); });
    h.adapter.changeFieldEdit("r", "Radius", "12");
    let edit!: Promise<WorkbenchSnapshot>;
    h.adapter.commitFieldEdit("r", () => { edit = h.adapter.dispatch({ version: 2, command: "parameter.edit" }); });
    h.adapter.changeFieldEdit("r", "Radius", "14");
    gate.resolve(); await edit;
    expect(h.requests.at(-1)).toMatchObject({ method: "authoring.mutation", authority: { revision: 1 }, baseHash: "disk-A" });
    h.changeDisk(); expect(h.adapter.installSnapshot(await h.adapter.snapshot())).toBe(false);
  });
  it("records ordered admitted drag samples and sends one terminal commit while navigation remains unblocked", async () => {
    const h = await harness(); h.pause();
    await h.adapter.pointer(point("down", 20)); await h.adapter.pointer(point("move", 21)); await h.adapter.pointer(point("move", 24)); await h.adapter.pointer(point("move", 29)); await h.adapter.pointer(point("up", 32));
    expect((await h.adapter.wheel(wheel))!.frame.scene.provenance.camera).toBe("1");
    await vi.waitFor(() => expect(h.requests).toHaveLength(2));
    expect(h.samples.map(sample => sample.position[0])).toEqual([24, 29, 32]);
    expect(h.samples.map(sample => sample.sequence)).toEqual([1, 2, 3]);
    expect(h.requests[1]).toMatchObject({ method: "authoring.commit", input: { kind: "point" }, baseHash: "disk-A" });
    expect(h.requests[1].input?.command).toMatchObject({ samples: h.samples });
    h.resume();
  });
  it("keeps installed browsing authority when an observed disk refresh conflicts with a field", async () => {
    const h = await harness(); h.adapter.changeFieldEdit("r", "Radius", "12"); h.changeDisk(); const observed = await h.adapter.snapshot();
    expect(h.adapter.installSnapshot(await h.adapter.prepareSnapshot(observed))).toBe(false); expect(h.local.replace).not.toHaveBeenCalled();
    expect(h.browsers[0].dispose).not.toHaveBeenCalled();
    expect((await h.adapter.wheel(wheel))!.frame.scene.provenance.scene).toBe("accepted-A");
    await h.adapter.dispatch({ version: 2, command: "history.undo" }); expect(h.requests.at(-1)!.baseHash).toBe("disk-A");
  });
  it("installs a completed edit after a newer local toolbar frame without losing its camera", async () => {
    const h = await harness(); h.changeDisk();
    const completed = await h.adapter.dispatch({ version: 2, command: "parameter.edit" });
    const local = await h.adapter.dispatch({ version: 2, command: "dimensions.navigation.end" });
    expect(h.adapter.installSnapshot(local)).toBe(true);
    const installed = await h.install(completed);
    expect(installed.frame.scene.provenance).toMatchObject({ scene: "accepted-B", camera: "1" });
    expect(h.adapter.installedState?.currentHash).toBe("disk-B");
  });
  it("reprojects native prediction at the latest local viewport while its terminal save is held", async () => {
    const h = await harness(); h.pause();
    h.local.projectPrediction = vi.fn(async input => ({ ...h.update(), frame: { ...h.fixture.frame, ariaLabel: input.presentation, scene: { ...h.fixture.frame.scene, provenance: { scene: "provisional", camera: String(input.view.state.camera) } } } }));
    let latest: WorkbenchSnapshot | undefined; h.adapter.subscribe(snapshot => { if (snapshot) latest = snapshot; });
    await h.adapter.pointer(point("down", 20)); await h.adapter.pointer(point("move", 28)); await h.adapter.pointer(point("up", 32));
    await vi.waitFor(() => expect(h.requests).toHaveLength(2));
    expect(latest?.frame.ariaLabel).toBe("prediction-32");
    const zoomed = await h.adapter.wheel(wheel);
    expect(zoomed?.frame.ariaLabel).toBe("native:32"); expect(zoomed?.frame.scene.provenance).toMatchObject({ scene: "provisional", camera: "1" });
    h.resume();
  });
  it("retains one native construction draft over multiple clicks and local camera changes", async () => {
    const h = await harness();
    await h.adapter.dispatch({ version: 2, command: "tool.select", payload: { id: "polyline" } });
    await h.adapter.pointer(point("down", 20)); await h.adapter.pointer(point("up", 20));
    await h.adapter.pointer(point("move", 35, 0)); await h.adapter.pointer(point("down", 35)); await h.adapter.pointer(point("up", 35));
    await h.adapter.wheel(wheel);
    await h.adapter.pointer(point("down", 45)); await h.adapter.pointer(point("up", 45));
    await vi.waitFor(() => expect(h.authoring.advanceConstruction).toHaveBeenCalledTimes(4));
    expect(h.authoring.beginConstruction).toHaveBeenCalledOnce(); expect(h.local.replace).not.toHaveBeenCalled(); expect(h.requests).toHaveLength(1);
    const before = vi.mocked(h.authoring.cancel).mock.calls.length;
    await h.adapter.cancel({ version: 2, reason: "escape" });
    await vi.waitFor(() => expect(vi.mocked(h.authoring.cancel).mock.calls.length).toBeGreaterThan(before));
  });
  it("does not start semantic dragging in the first half-second of external work", async () => {
    vi.useFakeTimers(); const h = await harness(); const stop = h.adapter.subscribe(() => {});
    h.activity(true); await vi.advanceTimersByTimeAsync(100);
    expect(h.adapter.activity.getSnapshot()).toBe(false); expect(h.adapter.activity.getPendingSnapshot()).toBe(true);
    await h.adapter.pointer(point("down", 20)); h.activity(false);
    await h.adapter.pointer(point("move", 28)); await h.adapter.pointer(point("up", 28)); await vi.advanceTimersByTimeAsync(0);
    expect(h.requests).toHaveLength(1); expect(h.authoring.beginPoint).not.toHaveBeenCalled();
    expect(h.local.update).toHaveBeenCalledWith("pointer", point("move", 28)); stop();
  });
  it("preserves local selection made during external work into the newly installed seed", async () => {
    const h = await harness(); const stop = h.adapter.subscribe(() => {});
    h.activity(true); await h.adapter.pointer(point("down", 25)); await h.adapter.pointer(point("up", 25));
    expect(h.requests).toHaveLength(1); h.changeDisk(); h.activity(false);
    const installed = await h.install(await h.adapter.snapshot());
    expect(installed.frame.scene.provenance).toMatchObject({ scene: "accepted-B", selection: "point-25" });
    expect(h.requests.map(request => request.method)).toEqual(["session.join", "snapshot"]); stop();
  });
  it("retains saved operation identity and later diagnostics through failed read-only browsing", async () => {
    const h = await harness(), gate = deferred(), browser = h.browsers[0];
    vi.mocked(browser.present).mockImplementation(async () => { await gate.promise; throw Error("old browsing failed"); });
    h.adapter.pending = '{"operationId":"saved-authored-intent"}'; h.adapter.notice = "Saved operation needs recovery";
    await h.adapter.pointer(point("down", 25)); await h.adapter.pointer(point("up", 25)); gate.resolve();
    await vi.waitFor(() => expect(browser.present).toHaveBeenCalled());
    expect(h.adapter.notice).toBe("Saved operation needs recovery"); expect(h.adapter.pendingOperationId).toBe("saved-authored-intent");
  });
  it("holds provisional geometry until the saved terminal snapshot is actually installed", async () => {
    const h = await harness(); let terminal: WorkbenchSnapshot | undefined;
    h.changeDisk(); h.fixture.project.title = "Saved terminal";
    h.adapter.subscribe(snapshot => { if (snapshot?.project.title === "Saved terminal") terminal = snapshot; });
    await h.adapter.pointer(point("down", 20)); await h.adapter.pointer(point("move", 28)); await h.adapter.pointer(point("up", 32));
    await vi.waitFor(() => expect(terminal).toBeDefined());
    expect((await h.adapter.wheel(wheel))!.frame.scene.provenance.scene).toBe("provisional");
    const accepted = await h.install(terminal!);
    expect(accepted.frame.scene.provenance.scene).toBe("accepted-B");
    expect((await h.adapter.wheel(wheel))!.frame.scene.provenance.scene).toBe("accepted-B");
  });
  it("restores the prior scene and exact selection if a draft starts during native replacement", async () => {
    const h = await harness(), entered = deferred(), gate = deferred();
    await h.adapter.pointer(point("down", 25)); await h.adapter.pointer(point("up", 25));
    h.changeDisk(); const observed = await h.adapter.snapshot(), replace = vi.mocked(h.local.replace).getMockImplementation()!;
    vi.mocked(h.local.replace).mockImplementationOnce(async (...args) => { entered.resolve(); await gate.promise; return replace(...args); });
    const preparing = h.adapter.prepareSnapshot(observed); await entered.promise;
    h.adapter.changeFieldEdit("r", "Radius", "14"); gate.resolve();
    expect(h.adapter.installSnapshot(await preparing)).toBe(false);
    expect((await h.adapter.wheel(wheel))!.frame.scene.provenance).toMatchObject({ scene: "accepted-A", selection: "point-25" });
    expect(h.browsers[0].dispose).not.toHaveBeenCalled();
  });
  it("validates navigation against the displayed authority and keeps selection RPC-free", async () => {
    const h = await harness();
    await expect(h.adapter.dispatch({ version: 2, command: "navigation.rows.select", payload: { authority: "old", ids: ["radius"] } })).rejects.toThrow(/older source revision/);
    const selected = await h.adapter.dispatch({ version: 2, command: "navigation.rows.select", payload: { authority: "navigation", ids: ["radius"] } });
    expect(selected.frame.scene.provenance.selection).toBe("radius"); expect(h.requests).toHaveLength(1);
    expect(h.local.update).toHaveBeenCalledWith("restoreSelection", expect.objectContaining({ expected: expect.objectContaining({ sceneKey: "accepted-A" }) }));
  });
  it("installs rejected-source diagnostics on unchanged disk while the original source draft remains stale", async () => {
    const h = await harness(); h.adapter.draftChanged(true); h.failSource();
    const observed = await h.adapter.snapshot(), prepared = await h.adapter.prepareSnapshot(observed);
    expect(h.adapter.installSnapshot(prepared)).toBe(true);
    expect(prepared.project.status).toBe("failed"); expect(prepared.source.dirty).toBe(true);
    expect(prepared.problems).toContainEqual(expect.objectContaining({ detail: "Invalid source" }));
    await expect(h.adapter.dispatch({ version: 2, command: "source.prepare", payload: { path: "sketch.ts", contents: "draft" } })).rejects.toThrow("stale revision 1; expected 2");
    expect(h.requests.at(-1)).toMatchObject({ authority: { revision: 1 }, baseHash: "disk-A" });
  });
  it("persists native personal preferences per folder without authoring RPC and restores them on reopen", async () => {
    const h = await harness();
    const presentation = { hiddenRows: ["managed:edge"], constructionVisible: false, dimensions: { mode: "all" as const, pins: ["native-persistent-key"] } };
    h.local.exportPresentation = vi.fn(async () => presentation);
    const count = h.requests.length;
    await h.adapter.dispatch({ version: 2, command: "dimensions.mode", payload: { mode: "all" } });
    expect(h.requests).toHaveLength(count);
    expect(JSON.parse(localStorage.getItem("geosolve.folder.view:/project")!)).toEqual(presentation);
    h.adapter.dispose();
    const reopened = await harness();
    expect(reopened.browsers[0].initialize).toHaveBeenCalledWith(expect.anything(), expect.anything(), presentation);
  });
  it("falls back to the server saved view when native validation rejects a stored personal envelope", async () => {
    const h = await harness(); h.adapter.dispose();
    const corrupted = { dimensions: { mode: "invalid", pins: ["untrusted"] } };
    localStorage.setItem("geosolve.folder.view:/project", JSON.stringify(corrupted));
    const created = folderBrowser(h.fixture); const browser = await created;
    const factory = () => {
      const client = browser.createBrowsing(), initialize = vi.mocked(client.initialize!).getMockImplementation()!;
      vi.mocked(client.initialize!).mockImplementation(async (model, seed, presentation) => {
        if (presentation) throw Error("Native codec rejected personal presentation");
        return initialize(model, seed);
      });
      return client;
    };
    const adapter = new FolderWorkbenchAdapter("test-token", () => browser.local, { createBrowsing: factory, authoring: null }); handles.push(adapter);
    const opened = await adapter.prepareSnapshot(await adapter.construct());
    expect(adapter.installSnapshot(opened)).toBe(true);
    expect(browser.browsers[0].initialize).toHaveBeenNthCalledWith(1, expect.anything(), expect.anything(), corrupted);
    expect(browser.browsers[0].initialize).toHaveBeenNthCalledWith(2, expect.anything(), expect.anything(), undefined);
    expect(opened.project.status).toBe("accepted");
  });
  it("bounds retained browser workers while draft-vetoed observations keep arriving", async () => {
    const h = await harness(); h.adapter.changeFieldEdit("r", "Radius", "14");
    for (let index = 0; index < 4; index++) { h.advanceRevision(); await h.adapter.snapshot(); }
    expect(h.browsers).toHaveLength(5);
    expect(h.browsers[0].dispose).not.toHaveBeenCalled();
    expect(h.browsers.slice(1, -1).every(browser => vi.mocked(browser.dispose).mock.calls.length === 1)).toBe(true);
    expect(h.browsers.at(-1)!.dispose).not.toHaveBeenCalled();
    expect((await h.adapter.wheel(wheel))!.frame.scene.provenance.scene).toBe("accepted-A");
  });
  it("does not clone, stamp or publish an unchanged local hover", async () => {
    const h = await harness(); vi.mocked(h.local.update).mockResolvedValueOnce(null);
    expect(await h.adapter.pointer(point("move", 40, 0))).toBeNull(); expect(h.requests).toHaveLength(1);
  });
  it("survives network loss during navigation and retains exact unsaved semantic intent", async () => {
    const h = await harness(); vi.mocked(fetch).mockRejectedValue(Error("offline"));
    expect((await h.adapter.wheel(wheel))!.frame.scene.provenance.camera).toBe("1");
    await expect(h.adapter.dispatch({ version: 2, command: "parameter.edit" })).rejects.toThrow("offline");
    expect(h.adapter.pendingOperationId).toBeTruthy(); expect(JSON.parse(h.adapter.pending).method).toBe("authoring.mutation");
  });
});
