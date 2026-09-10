// SPDX-License-Identifier: GPL-3.0-or-later
import { describe, expect, it, vi } from "vitest";
import { readFile } from "node:fs/promises";
import { resolve } from "node:path";
import initializePresentation, { InteractionHandle, WorkbenchHandle, renderAuthoringPreview } from "../generated/geosolve_demo_web.js";
import { createEngine, type EditableSession, type PointGestureTarget } from "../../../../../packages/geosolve-engine/src/index";
import initializeEngine, * as engineWasm from "../../../../../packages/geosolve-engine/dist/wasm/geosolve_sketch_engine_wasm.js";
import { createAuthoringWorkerHandler, type AuthoringAction, type AuthoringModel, type AuthoringResult, type AuthoringRuntime, type AuthoringView, type AuthoringWorkerRequest, type AuthoringWorkerResponse } from "./collaboration-authoring-worker";
import { LocalAuthoringWorker } from "./collaboration-authoring-adapter";
import { LocalInteractionWorker } from "./local-interaction-adapter";
import type { LocalInteractionResponse } from "./local-interaction-worker";
import { MockWorkbenchAdapter } from "./mock-adapter";

const model: AuthoringModel = { documentEpoch: "doc-one", revision: 1, sourceDesignDigest: "source-design", project: "{}", design: { format: "geosolve-design-v1", project: "project", generated: {}, overrides: {} } };
const view: AuthoringView = { seed: { opaque: "seed" }, state: { opaque: "current-local-view" } };
const viewport = { screen_size: [1000, 700] as const, model_center: [0, 0] as const, pixels_per_model_unit: 10 };
class FakeWorker extends EventTarget {
  postMessage = vi.fn(); terminate = vi.fn();
  reply(response: AuthoringWorkerResponse | LocalInteractionResponse) { this.dispatchEvent(new MessageEvent("message", { data: response })); }
}
async function provisional() {
  const frame = structuredClone((await new MockWorkbenchAdapter().snapshot()).frame);
  frame.scene.provenance.scene = "provisional";
  frame.scene.items.forEach(item => { item.interactive = false; });
  return { kind: "preview" as const, model, view, frame };
}
function harness(runtime: Promise<AuthoringRuntime>) {
  let id = 0;
  const pending = new Map<number, { resolve(value: AuthoringResult): void; reject(error: Error): void }>();
  const handler = createAuthoringWorkerHandler(runtime, response => {
    const waiter = pending.get(response.id)!; pending.delete(response.id);
    if ("error" in response) waiter.reject(Error(response.error)); else waiter.resolve(response.result);
  });
  return (action: AuthoringAction, generation = 1) => new Promise<AuthoringResult>((resolve, reject) => {
    const request = { ...action, id: ++id, generation } satisfies AuthoringWorkerRequest;
    pending.set(id, { resolve, reject }); handler(new MessageEvent("message", { data: request }));
  });
}

describe("separate authoring worker", () => {
  it("coalesces painting without losing ordered pointer samples or the terminal", async () => {
    const worker = new FakeWorker(), client = new LocalAuthoringWorker(worker as unknown as Worker), preview = await provisional();
    const opened = client.replace(model); await Promise.resolve();
    worker.reply({ id: 1, generation: 1, result: { kind: "ready", model } }); await opened;
    const first = client.advancePoint({ sequence: 1, position: [1, 2] }, view);
    const second = client.advancePoint({ sequence: 2, position: [2, 3] }, view);
    const terminal = client.finishPoint();
    await Promise.resolve();
    expect(worker.postMessage.mock.lastCall?.[0]).toMatchObject({ method: "advancePoint", samples: [{ sequence: 1 }, { sequence: 2 }] });
    worker.reply({ id: 2, generation: 1, result: preview });
    expect(await first).toBe(await second);
    expect(Object.isFrozen(preview.frame.scene.items)).toBe(true);
    await Promise.resolve();
    expect(worker.postMessage.mock.lastCall?.[0].method).toBe("finishPoint");
    worker.reply({ id: 3, generation: 1, error: "native terminal rejection" });
    await expect(terminal).rejects.toThrow("native terminal rejection");
    client.dispose();
  });

  it("invalidates old generations immediately and isolates held authoring from navigation", async () => {
    vi.useFakeTimers();
    try {
      const authoringWorker = new FakeWorker(), navWorker = new FakeWorker();
      const client = new LocalAuthoringWorker(authoringWorker as unknown as Worker), navigation = new LocalInteractionWorker(navWorker as unknown as Worker);
      const opened = client.replace(model); await Promise.resolve();
      authoringWorker.reply({ id: 1, generation: 1, result: { kind: "ready", model } }); await opened;
      const held = client.advanceConstruction({ sequence: 1, input: { event: "click", position: [2, 3], suppressed: false, regularized: false } }, view);
      const rejection = expect(held).rejects.toThrow("Authoring model was replaced");
      await Promise.resolve();
      const nav = navigation.update("wheel", { deltaY: -90 });
      const frame = (await new MockWorkbenchAdapter().snapshot()).frame;
      navWorker.reply({ id: 1, result: { frame, state: {}, selectionChanged: false, serverFrameCompatible: false } });
      expect((await nav)?.frame).toBe(frame);
      await vi.advanceTimersByTimeAsync(10_000);
      expect(authoringWorker.postMessage).toHaveBeenCalledTimes(2);
      const newer = { ...model, revision: 2 };
      const replaced = client.replace(newer); await Promise.resolve(); await rejection;
      authoringWorker.reply({ id: 2, generation: 1, result: await provisional() });
      authoringWorker.reply({ id: 3, generation: 2, result: { kind: "ready", model: newer } });
      expect((await replaced).model.revision).toBe(2);
      client.dispose(); navigation.dispose();
    } finally { vi.useRealTimers(); }
  });

  it("uses actual WASM point and construction predictions without accepted model mutation", async () => {
    const bytes = await readFile(resolve(process.cwd(), "../../../packages/geosolve-engine/dist/wasm/geosolve_sketch_engine_wasm_bg.wasm"));
    const engine = await createEngine({ wasmModule: { ...engineWasm, default: initializeEngine }, wasm: bytes });
    const fixture = JSON.parse(await readFile(resolve(process.cwd(), "../../geosolve-sketch-engine/tests/fixtures/point-gesture-constrained.json"), "utf8"));
    await initializePresentation({ module_or_path: await readFile(resolve(process.cwd(), "src/generated/geosolve_demo_web_bg.wasm")) });
    let session: EditableSession | undefined, workbench: WorkbenchHandle | undefined, interaction: InteractionHandle | undefined;
    let genuineView = view;
    const runtime: AuthoringRuntime = {
      open: input => { session = engine.openEditableSession(input.project, { design: input.design }); return session; },
      release: held => { const result = held.accepted; held.dispose(); engine.release(result); },
      render: (scene, currentView, construction) => {
        expect(JSON.parse(scene)).toBeTypeOf("object"); expect(currentView).toEqual(genuineView);
        if (construction?.preview) expect(construction.preview).toHaveProperty("kind");
        const frame = JSON.parse(renderAuthoringPreview(JSON.stringify({ ...JSON.parse(scene), view: currentView,
          construction: construction ? { preview: construction.preview, inference_guides: construction.inference_guides } : null })));
        expect(frame.scene.provenance.scene).toBe("provisional");
        expect(frame.scene.items.every((item: { interactive: boolean }) => !item.interactive)).toBe(true);
        return frame;
      },
    };
    try {
      const project = engine.compileProject({ project: "browser-prediction", compiled: fixture, customFiles: {}, artifacts: {}, lock: { format: "geosolve-lock-v1", modules: {} } });
      const prepared = engine.openEditableSession(project);
      const accepted: AuthoringModel = { ...model, project: prepared.exportProject(), design: prepared.exportDesign(), sourceDesignDigest: prepared.sourceDesignDigest() };
      runtime.release(prepared);
      workbench = new WorkbenchHandle(JSON.stringify({ version: 2, persistedProject: accepted.project }));
      workbench.dispatch(JSON.stringify({ version: 2, command: "workspace.project.apply", payload: { project: accepted.project, design: accepted.design } }));
      const pair = JSON.parse(workbench.interactionSnapshot());
      interaction = new InteractionHandle(JSON.stringify(pair.seed));
      const screen = JSON.parse(pair.seed.scene).points[0].screen_position;
      const navigationBefore = interaction.state();
      const hit = JSON.parse(interaction.authoringPointer(JSON.stringify({x:screen.x,y:screen.y})));
      expect(hit.target).not.toBeNull();
      expect(hit.position).toEqual(JSON.parse(pair.seed.scene).points[0].model_position);
      expect(interaction.state()).toBe(navigationBefore);
      interaction.pointer(JSON.stringify({version:2,phase:"down",pointerId:1,x:screen.x,y:screen.y,buttons:1,modifiers:{alt:false,ctrl:false,meta:false,shift:false}}));
      genuineView = { seed: pair.seed, state: JSON.parse(interaction.state()) };
      const send = harness(Promise.resolve(runtime));
      expect((await send({ method: "replace", model: accepted })).kind).toBe("ready");
      const original = { project: session!.exportProject(), design: session!.exportDesign(), token: session!.token };
      const target: PointGestureTarget = hit.target;
      expect(session!.pointGestureTargets().map(handle=>handle.target)).toContainEqual(target);
      await send({ method: "beginPoint", target, gestureId: 71, viewport, view: genuineView });
      await send({ method: "advancePoint", samples: [{ sequence: 1, position: [1, 2] }, { sequence: 2, position: [2, 3] }], view: genuineView });
      const point = await send({ method: "finishPoint" });
      expect(point.kind).toBe("point");
      if (point.kind === "point") { expect(point.terminal.command.samples).toHaveLength(2); expect(point.terminal.command.basis).toBe(accepted.sourceDesignDigest); }
      await send({ method: "beginConstruction", tool: "segment", role: "profile", gestureId: 72, viewport, view: genuineView });
      const update = await send({ method: "advanceConstruction", samples: [
        { sequence: 1, input: { event: "click", position: [45, 45], suppressed: true, regularized: false } },
        { sequence: 2, input: { event: "move", position: [65, 55], suppressed: true, regularized: false } },
        { sequence: 3, input: { event: "click", position: [65, 55], suppressed: true, regularized: false } },
      ], view: genuineView });
      expect(update.kind === "preview" && update.construction?.completed).toBe(true);
      const construction = await send({ method: "finishConstruction" });
      expect(construction.kind).toBe("construction");
      if (construction.kind === "construction") { expect(construction.command.samples).toHaveLength(3); expect(construction.command.expected_declarations.length).toBeGreaterThan(0); }
      expect({ project: session!.exportProject(), design: session!.exportDesign(), token: session!.token }).toEqual(original);
      await expect(send({ method: "replace", model: { ...accepted, sourceDesignDigest: "forged" } }, 2)).rejects.toThrow("disagrees");
    } finally { interaction?.free(); workbench?.free(); engine.dispose(); }
  });
});
