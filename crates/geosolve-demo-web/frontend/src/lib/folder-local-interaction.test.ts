// SPDX-License-Identifier: GPL-3.0-or-later
import { afterEach, describe, expect, it, vi } from "vitest";
import { FolderWorkbenchAdapter } from "./folder-adapter";
import { MockWorkbenchAdapter } from "./mock-adapter";
import { isCanvasOnlySnapshot, type PointerSample, type WorkbenchSnapshot } from "./adapter";
import type { LocalInteractionClient, LocalInteractionUpdate } from "./local-interaction-adapter";

const point = (phase: PointerSample["phase"], x = 20, buttons = phase === "up" ? 0 : 1): PointerSample => ({ version: 2, phase, pointerId: 7, x, y: 30, buttons, modifiers: { alt: false, ctrl: false, meta: false, shift: false } });
const wheel = { version: 2 as const, x: 30, y: 20, deltaX: 0, deltaY: -90, ctrl: false };
async function harness(strictRevision = false, nativeDraftGuard = false) {
  const fixture = await new MockWorkbenchAdapter().snapshot();
  let camera = 0, selection = "none", scene = "accepted-A";
  const update = (selectionChanged = false): LocalInteractionUpdate => ({ frame: { ...fixture.frame, scene: { ...fixture.frame.scene, provenance: { camera: String(camera), selection, scene } } }, state: { sceneKey: scene, camera, selection }, selectionChanged, serverFrameCompatible: false });
  const local: LocalInteractionClient = {
    construct: vi.fn(async () => update()),
    replace: vi.fn(async (seed, preserve) => { scene = String(seed.sceneKey); if (!preserve) selection = String(seed.selection); return update(); }),
    state: vi.fn(async () => update().state), dispose: vi.fn(),
    update: vi.fn(async (method, input) => {
      if (method === "wheel" || method === "resize" || method === "dispatch") camera += 1;
      const changed = method === "pointer" && (input as PointerSample).phase === "down" && (input as PointerSample).buttons === 1;
      if (changed) selection = `point-${(input as PointerSample).x}`;
      return update(changed);
    }),
  };
  const requests: Array<{ method: string; input?: unknown; interaction?: unknown; baseHash: string; localInteraction?: boolean }> = [];
  let wait: Promise<void> | undefined, release = () => {}, disk = "disk-A", revision = 1, nativeDraft = false;
  vi.stubGlobal("fetch", vi.fn(async (_url, init) => {
    const request = JSON.parse(init.body); requests.push(request); if (wait) await wait;
    if (strictRevision && !["session.join", "snapshot"].includes(request.method) && request.authority?.revision !== revision) throw Error(`stale interaction revision ${request.authority?.revision}; expected ${revision}`);
    if (strictRevision && request.method === "interaction.sync") revision += 1;
    if (nativeDraftGuard && request.method === "dispatch" && request.input?.command === "tool.select") { fixture.presentation.activeTool = request.input.payload.id; nativeDraft = fixture.presentation.activeTool !== "select"; }
    if (nativeDraftGuard && request.method === "pointer" && request.interaction && nativeDraft) throw Error("finish the current interaction before replacing selection");
    if (nativeDraftGuard && request.method === "pointer" && request.input.phase === "down") nativeDraft = fixture.presentation.activeTool !== "select";
    if (nativeDraftGuard && request.method === "cancel") nativeDraft = false;
    return { ok: true, json: async () => ({ result: { ...structuredClone(fixture), localInteraction: { sceneKey: disk === "disk-A" ? "accepted-A" : "accepted-B", selection: "server-selection" } }, state: { ok: true, authority: { epoch: "epoch", lease: 1, revision }, editor: { canEdit: true }, sequence: 1, currentHash: disk, acceptedHash: disk, status: "saved", diagnostics: [], paths: { folder: "/project", source: "/project/sketch.ts" } } }) };
  }));
  let events: EventTarget;
  vi.stubGlobal("EventSource", class extends EventTarget { constructor() { super(); events = this; } close() {} });
  const adapter = new FolderWorkbenchAdapter("test-token", () => local);
  const install = async (snapshot: WorkbenchSnapshot) => { const prepared = await adapter.prepareSnapshot(snapshot); adapter.installSnapshot(prepared); return prepared; };
  await install(await adapter.construct());
  return { adapter, local, requests, install, activity: (busy: boolean) => events.dispatchEvent(new MessageEvent("activity", { data: JSON.stringify({ busy }) })), changeDisk: () => { disk = "disk-B"; }, pause: () => { wait = new Promise((resolve) => { release = resolve; }); }, resume: () => { wait = undefined; release(); } };
}
afterEach(() => { vi.unstubAllGlobals(); vi.useRealTimers(); sessionStorage.clear(); });

describe("folder detached local canvas", () => {
  it("navigates and highlights with no RPC while a server edit is stalled", async () => {
    const h = await harness(); h.pause();
    const edit = h.adapter.dispatch({ version: 2, command: "parameter.edit", payload: { id: "r", value: "12" } });
    await vi.waitFor(() => expect(h.requests).toHaveLength(2));
    const zoomed = (await h.adapter.wheel(wheel))!, hovered = (await h.adapter.pointer(point("move", 40, 0)))!;
    expect(h.adapter.responsiveCanvas).toBe(true); expect(isCanvasOnlySnapshot(zoomed)).toBe(true); expect(isCanvasOnlySnapshot(hovered)).toBe(true);
    expect(zoomed.frame.scene.provenance.camera).toBe("1"); expect(h.requests).toHaveLength(2);
    h.resume(); const installed = await h.install(await edit);
    expect(installed.frame.scene.provenance.camera).toBe("1");
    expect(h.requests[1].interaction).toMatchObject({ sceneKey: "accepted-A", camera: 0 });
    expect(h.requests.every((request) => request.localInteraction)).toBe(true);
  });
  it("selects immediately, coalesces Inspector sync, and does not rewind the latest selection or camera", async () => {
    const h = await harness(true); let latest: WorkbenchSnapshot | undefined; const delivered: Promise<unknown>[] = [];
    const stop = h.adapter.subscribe((snapshot) => { if (snapshot) delivered.push(h.install(snapshot).then((value) => { latest = value; })); });
    h.pause(); const selected = (await h.adapter.pointer(point("down", 25)))!; await h.adapter.pointer(point("up", 25));
    expect(selected.frame.scene.provenance.selection).toBe("point-25"); await vi.waitFor(() => expect(h.requests).toHaveLength(2));
    await h.adapter.pointer(point("down", 35)); await h.adapter.pointer(point("up", 35)); await h.adapter.wheel(wheel);
    expect(h.requests).toHaveLength(2); h.resume(); await vi.waitFor(() => expect(h.requests).toHaveLength(3)); await Promise.all(delivered);
    expect(latest!.frame.scene.provenance).toMatchObject({ camera: "1", selection: "point-35" });
    expect(h.local.replace).toHaveBeenLastCalledWith(expect.anything(), true); expect(h.requests.filter((request) => request.method === "pointer")).toHaveLength(0); stop();
  });
  it("waits for installed sync authority before an explicit edit enqueued during sync", async () => {
    const h = await harness(true);
    let acknowledge!: () => Promise<void>;
    const stop = h.adapter.subscribe((snapshot) => { if (snapshot) acknowledge = async () => { await h.install(snapshot); }; });
    h.pause();
    await h.adapter.pointer(point("down", 25)); await h.adapter.pointer(point("up", 25));
    await vi.waitFor(() => expect(h.requests).toHaveLength(2));
    h.adapter.changeFieldEdit("r", "Radius", "12");
    const edit = h.adapter.dispatch({ version: 2, command: "parameter.edit" });
    h.resume(); await vi.waitFor(() => expect(acknowledge).toBeDefined());
    expect(h.requests).toHaveLength(2);
    await acknowledge(); await edit;
    expect(h.requests).toHaveLength(3);
    expect((h.requests[2] as unknown as { authority: { revision: number } }).authority.revision).toBe(2);
    stop();
  });
  it("replays exact drag samples with the original pre-down interaction once while navigation remains unblocked", async () => {
    const h = await harness(); h.pause();
    await h.adapter.pointer(point("down", 20)); await h.adapter.pointer(point("move", 21)); await h.adapter.pointer(point("move", 24)); await h.adapter.pointer(point("move", 29)); await h.adapter.pointer(point("up", 32));
    expect((await h.adapter.wheel(wheel))!.frame.scene.provenance.camera).toBe("1"); h.resume(); await vi.waitFor(() => expect(h.requests).toHaveLength(6));
    const pointers = h.requests.filter((request) => request.method === "pointer");
    expect(pointers.map((request) => request.input)).toEqual([point("down", 20), point("move", 21), point("move", 24), point("move", 29), point("up", 32)]);
    expect(pointers[0].interaction).toMatchObject({ camera: 0, selection: "none" }); expect(pointers.slice(1).every((request) => request.interaction === undefined)).toBe(true);
  });
  it("keeps the installed accepted scene when disk refresh conflicts with a pending field", async () => {
    const h = await harness(); h.adapter.changeFieldEdit("r", "Radius", "12"); h.changeDisk(); const observed = await h.adapter.snapshot();
    expect(h.adapter.installSnapshot(await h.adapter.prepareSnapshot(observed))).toBe(false); expect(h.local.replace).not.toHaveBeenCalled();
    expect((await h.adapter.wheel(wheel))!.frame.scene.provenance.scene).toBe("accepted-A");
    await h.adapter.dispatch({ version: 2, command: "history.undo" }); expect(h.requests.at(-1)!.baseHash).toBe("disk-A"); expect(h.requests.at(-1)!.interaction).toMatchObject({ sceneKey: "accepted-A" });
  });
  it("installs a completed edit after a newer local toolbar frame without losing that camera", async () => {
    const h = await harness(); h.changeDisk();
    const completed = await h.adapter.dispatch({ version: 2, command: "parameter.edit" });
    // An edit result can wait for the host while a later local command paints.
    const view = await h.adapter.dispatch({ version: 2, command: "dimensions.navigation.end" });
    expect(h.adapter.installSnapshot(view)).toBe(true);
    const installed = await h.install(completed);
    expect(installed.frame.scene.provenance).toMatchObject({ scene: "accepted-B", camera: "1" });
    expect(h.adapter.installedState?.currentHash).toBe("disk-B");
  });
  it("preserves authenticated server previews only when Rust verifies the exact local viewport", async () => {
    const h = await harness();
    const compatible = await h.adapter.snapshot();
    const localFrame = structuredClone(compatible.frame);
    localFrame.scene.provenance = { local: "accepted-only" };
    vi.mocked(h.local.replace).mockResolvedValueOnce({ frame: localFrame, state: {}, selectionChanged: false, serverFrameCompatible: true });
    expect((await h.install(compatible)).frame).toBe(compatible.frame);
    const obsolete = await h.adapter.snapshot();
    vi.mocked(h.local.replace).mockResolvedValueOnce({ frame: localFrame, state: {}, selectionChanged: false, serverFrameCompatible: false });
    expect((await h.install(obsolete)).frame).toBe(localFrame);
    // The compatibility flag never authorizes an old server overlay on a local operation.
    vi.mocked(h.local.update).mockResolvedValueOnce({ frame: localFrame, state: {}, selectionChanged: false, serverFrameCompatible: true });
    expect((await h.adapter.wheel(wheel))!.frame).toBe(localFrame);
  });
  it("transfers interaction before arming a tool and never restores its already active native draft", async () => {
    const h = await harness(false, true);
    const armed = await h.adapter.dispatch({ version: 2, command: "tool.select", payload: { id: "polyline" } });
    await h.install(armed);
    await h.adapter.pointer(point("move", 20, 0));
    await h.adapter.pointer(point("down", 20)); await h.adapter.pointer(point("up", 20));
    await vi.waitFor(() => expect(h.requests.filter(({ method }) => method === "pointer")).toHaveLength(3));
    expect(h.requests[1].interaction).toBeDefined();
    expect(h.requests.filter(({ method }) => method === "pointer").every(({ interaction }) => interaction === undefined)).toBe(true);
    expect(h.adapter.notice).not.toContain("finish the current interaction");
  });
  it("does not promote a primary gesture to editing in the first half-second of external work", async () => {
    vi.useFakeTimers();
    const h = await harness();
    const stop = h.adapter.subscribe(() => {});
    h.activity(true);
    await vi.advanceTimersByTimeAsync(100);
    expect(h.adapter.activity.getSnapshot()).toBe(false);
    expect(h.adapter.activity.getPendingSnapshot()).toBe(true);
    await h.adapter.pointer(point("down", 20));
    await h.adapter.pointer(point("move", 28));
    await h.adapter.pointer(point("up", 28));
    await vi.advanceTimersByTimeAsync(0);
    expect(h.requests).toHaveLength(1);
    expect(h.local.update).toHaveBeenCalledWith("pointer", point("move", 28));
    stop();
  });
  it("defers selection synchronization during externally announced work until the new accepted seed is installed", async () => {
    vi.useFakeTimers();
    const h = await harness();
    const stop = h.adapter.subscribe((snapshot) => { if (snapshot) void h.install(snapshot); });
    h.activity(true); await vi.advanceTimersByTimeAsync(500);
    await h.adapter.pointer(point("down", 25)); await h.adapter.pointer(point("up", 25));
    await vi.advanceTimersByTimeAsync(0);
    expect(h.requests).toHaveLength(1);
    h.changeDisk();
    h.activity(false);
    await h.install(await h.adapter.snapshot());
    await vi.advanceTimersByTimeAsync(0);
    expect(h.requests.at(-1)).toMatchObject({ method: "interaction.sync", baseHash: "disk-B", interaction: { sceneKey: "accepted-B", selection: "point-25" } });
    stop(); vi.useRealTimers();
  });
  it("refreshes an automatic stale selection sync without replacing pending authored intent or masking later diagnostics", async () => {
    const h = await harness();
    const connected = vi.mocked(fetch).getMockImplementation()!;
    let rejected = false;
    vi.stubGlobal("fetch", vi.fn(async (...args: Parameters<typeof fetch>) => {
      const request = JSON.parse(String(args[1]?.body));
      if (!rejected && request.method === "interaction.sync") {
        rejected = true;
        return { ok: false, status: 409, json: async () => ({ error: "Conflict: the installed interaction state changed" }) };
      }
      return connected(...args);
    }));
    const stop = h.adapter.subscribe((snapshot) => { if (snapshot) void h.install(snapshot); });
    h.adapter.pending = '{"operationId":"saved-authored-intent"}';
    await h.adapter.pointer(point("down", 25)); await h.adapter.pointer(point("up", 25));
    await vi.waitFor(() => expect(h.requests.map(({ method }) => method)).toEqual(["session.join", "snapshot", "interaction.sync"]));
    expect(h.adapter.notice).not.toContain("Conflict:");
    expect(h.adapter.pending).toBe('{"operationId":"saved-authored-intent"}');
    stop();
  });
  it("retains the server geometry draft across multiple authoring clicks and cancels it before local navigation", async () => {
    const h = await harness(false, true);
    const installations: Promise<unknown>[] = [];
    const stop = h.adapter.subscribe((snapshot) => { if (snapshot) {
      snapshot.presentation.activeTool = "polyline";
      installations.push(h.install(snapshot));
    } });
    const armed = await h.adapter.dispatch({ version: 2, command: "tool.select", payload: { id: "polyline" } });
    armed.presentation.activeTool = "polyline";
    await h.install(armed);
    await h.adapter.pointer(point("down", 20)); await h.adapter.pointer(point("up", 20));
    await h.adapter.pointer(point("move", 35, 0));
    await h.adapter.pointer(point("down", 35)); await h.adapter.pointer(point("up", 35));
    await vi.waitFor(() => expect(h.requests.filter(({ method }) => method === "pointer")).toHaveLength(5));
    const pointers = h.requests.filter(({ method }) => method === "pointer");
    expect(pointers.every(({ interaction }) => interaction === undefined)).toBe(true);
    await Promise.all(installations);
    expect(h.local.replace).toHaveBeenLastCalledWith(expect.anything(), false);
    const finished = await h.adapter.dispatch({ version: 2, command: "tool.finish" });
    expect(h.requests.at(-1)!.interaction).toBeUndefined();
    finished.presentation.activeTool = "polyline";
    await h.install(finished);
    await h.adapter.pointer(point("down", 45)); await h.adapter.pointer(point("up", 45));
    h.pause();
    expect((await h.adapter.wheel(wheel))!.frame.scene.provenance.camera).toBe("1");
    await h.adapter.pointer(point("down", 50)); await h.adapter.pointer(point("up", 50));
    h.resume();
    await vi.waitFor(() => expect(h.requests.filter(({ method }) => method === "pointer")).toHaveLength(9));
    const cancellation = h.requests.findIndex(({ method }) => method === "cancel");
    expect(cancellation).toBeGreaterThan(0);
    expect(h.requests[cancellation].interaction).toBeUndefined();
    expect(h.requests[cancellation + 1]).toMatchObject({ method: "pointer", interaction: { camera: 1 } });
    await Promise.all(installations);
    stop();
  });
  it("does not clone, stamp or publish an unchanged local hover result", async () => {
    const h = await harness();
    vi.mocked(h.local.update).mockResolvedValueOnce(null);
    expect(await h.adapter.pointer(point("move", 40, 0))).toBeNull();
    expect(h.requests).toHaveLength(1);
  });
  it("navigation survives network loss without granting server edit authority", async () => {
    const h = await harness(); vi.mocked(fetch).mockRejectedValue(Error("offline"));
    expect((await h.adapter.wheel(wheel))!.frame.scene.provenance.camera).toBe("1");
    await expect(h.adapter.dispatch({ version: 2, command: "parameter.edit" })).rejects.toThrow("offline"); expect(h.adapter.pendingOperationId).toBeTruthy();
    h.adapter.dispose(); expect(h.local.dispose).toHaveBeenCalledOnce();
  });
});
