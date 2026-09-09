// SPDX-License-Identifier: GPL-3.0-or-later
import { afterEach, describe, expect, it, vi } from "vitest";
import { getCanvasSnapshotSequence, isCanvasOnlySnapshot, type WorkbenchAdapter, type WorkbenchSnapshot } from "./adapter";
import { MockWorkbenchAdapter } from "./mock-adapter";
import { WasmWorkbenchAdapter, type JsonWorkbenchHandle } from "./wasm-adapter";
import { createWorkbenchWorkerMessageHandler, type WorkbenchWorkerRequest, type WorkbenchWorkerResponse } from "./workbench-worker";
import { WorkerWorkbenchAdapter } from "./worker-workbench-adapter";
import compiledFixture from "../../../../../packages/geosolve-sketch-code/test/fixtures/managed-empty-circle.json";
import type { CompiledManagedSource } from "./managed-compiler";

class TestWorker extends EventTarget {
  readonly requests: WorkbenchWorkerRequest[] = [];
  readonly responses: WorkbenchWorkerResponse[] = [];
  readonly terminate = vi.fn();
  handler?: (event: MessageEvent<WorkbenchWorkerRequest>) => void;
  postMessage(request: WorkbenchWorkerRequest) {
    const clone = structuredClone(request);
    this.requests.push(clone);
    this.handler?.(new MessageEvent("message", { data: clone }));
  }
  reply = (response: WorkbenchWorkerResponse) => {
    const clone = structuredClone(response);
    this.responses.push(clone);
    this.dispatchEvent(new MessageEvent("message", { data: clone }));
  };
}

async function fixture() {
  const mock = new MockWorkbenchAdapter();
  const snapshot = await mock.construct({ version: 2 });
  const catalog = await mock.toolCatalog();
  const calls: { method: string; input?: unknown }[] = [];
  const record = (method: string, input?: string) => { calls.push({ method, input: input === undefined ? undefined : JSON.parse(input) }); };
  let wheelResponse = "null";
  class Handle implements JsonWorkbenchHandle {
    constructor(input: string) { record("construct", input); }
    snapshot() { return JSON.stringify(snapshot); }
    toolCatalog() { return JSON.stringify(catalog); }
    managedCompilerContext() { return JSON.stringify({ version: 2, patches: { custom: {} } }); }
    dispatch(input: string) {
      record("dispatch", input);
      if (JSON.parse(input).command === "reject") throw new Error("candidate rejected");
      return this.snapshot();
    }
    pointer(input: string) { record("pointer", input); return "null"; }
    wheel(input: string) { record("wheel", input); return wheelResponse; }
    resize(input: string) { record("resize", input); return "null"; }
    cancel(input: string) { record("cancel", input); return this.snapshot(); }
    exportProject() { return JSON.stringify({ version: 2, filename: "project.json", contents: "project bytes" }); }
    persistProject() { return JSON.stringify({ version: 2, contents: "persisted bytes" }); }
    exportReproduction() { return JSON.stringify({ version: 2, filename: "repro.txt", contents: "reproduction bytes" }); }
    exportInteractionTrace() { return JSON.stringify({ version: 2, filename: "trace.txt", contents: "trace bytes" }); }
    intentRpc(input: string) { return input; }
    codeControlRpc(input: string) { return input; }
  }
  const owner = new WasmWorkbenchAdapter(Handle);
  const worker = new TestWorker();
  worker.handler = createWorkbenchWorkerMessageHandler(Promise.resolve(owner), worker.reply);
  const adapter = new WorkerWorkbenchAdapter(worker);
  return { owner, adapter, worker, calls, snapshot, setWheelResponse: (value: unknown) => { wheelResponse = JSON.stringify(value); } };
}

const wheel = { version: 2 as const, x: 10, y: 20, deltaX: 0, deltaY: -30, ctrl: false };
afterEach(() => { vi.useRealTimers(); });

describe("WorkerWorkbenchAdapter", () => {
  it("forwards the complete public interface through one existing adapter without rewriting inputs", async () => {
    const { adapter, worker, calls, snapshot } = await fixture();
    const persistedProject = '{"format":"first","format":"duplicate"}';
    const base = await adapter.construct({ version: 2, persistedProject });
    expect(base).toEqual(snapshot);
    expect(getCanvasSnapshotSequence(base)).toBe(1);
    expect(Object.isFrozen(base.frame.scene.items)).toBe(true);
    expect((await adapter.toolCatalog()).version).toBe(1);
    expect(await adapter.managedCompilerContext()).toEqual({ version: 2, patches: { custom: {} } });
    expect(getCanvasSnapshotSequence(await adapter.snapshot())).toBe(2);
    await adapter.dispatch({ version: 2, command: "selection.clear", payload: { original: "bytes" } });
    const pointer = { version: 2 as const, phase: "up" as const, pointerId: 7, x: 4, y: 5, buttons: 0, modifiers: { alt: false, ctrl: false, meta: false, shift: true } };
    expect(await adapter.pointer(pointer)).toBeNull();
    expect(await adapter.wheel(wheel)).toBeNull();
    expect(await adapter.wheelBatch([wheel, { ...wheel, x: 50 }])).toBeNull();
    expect(await adapter.resize({ version: 2, width: 800, height: 600, pixelRatio: 2 })).toBeNull();
    await adapter.cancel({ version: 2, reason: "lost-capture" });
    expect(await adapter.exportProject()).toEqual({ version: 2, filename: "project.json", contents: "project bytes" });
    expect(await adapter.persistProject()).toEqual({ version: 2, contents: "persisted bytes" });
    expect(await adapter.exportReproduction()).toEqual({ version: 2, filename: "repro.txt", contents: "reproduction bytes" });
    expect(await adapter.exportInteractionTrace()).toEqual({ version: 2, filename: "trace.txt", contents: "trace bytes" });
    expect(calls).toEqual([
      { method: "construct", input: { version: 2, persistedProject } },
      { method: "dispatch", input: { version: 2, command: "selection.clear", payload: { original: "bytes" } } },
      { method: "pointer", input: pointer },
      { method: "wheel", input: wheel },
      { method: "wheel", input: { version: 2, samples: [wheel, { ...wheel, x: 50 }] } },
      { method: "resize", input: { version: 2, width: 800, height: 600, pixelRatio: 2 } },
      { method: "cancel", input: { version: 2, reason: "lost-capture" } },
    ]);
    expect(worker.requests.map((request) => request.id)).toEqual(Array.from({ length: 14 }, (_, index) => index + 1));
    adapter.dispose();
  });

  it("preserves canvas-only markers, order and immutable geometry across structured clone without copying UI state", async () => {
    const { adapter, worker, snapshot, setWheelResponse } = await fixture();
    const base = await adapter.construct({ version: 2 });
    const frame = { ...snapshot.frame, scene: { ...snapshot.frame.scene, viewBox: [10, 20, 900, 700] } };
    setWheelResponse({ version: 2, kind: "frame", revision: snapshot.revision, frame });
    for (let sequence = 2; sequence <= 3; sequence += 1) {
      const next = (await adapter.wheel(wheel))!;
      expect(next.frame).toEqual(frame);
      expect(Object.isFrozen(next.frame.scene.items)).toBe(true);
      expect(Object.isFrozen(next.frame.scene.viewBox)).toBe(true);
      expect(isCanvasOnlySnapshot(next)).toBe(true);
      expect(getCanvasSnapshotSequence(next)).toBe(sequence);
      for (const key of ["source", "explorer", "parameters", "presentation", "project", "problems"] as const) expect(next[key]).toBe(base[key]);
      expect(worker.responses.at(-1)).toEqual({ id: sequence, kind: "canvas", frame, sequence, baseSequence: sequence - 1 });
    }
    const full = await adapter.snapshot();
    expect(isCanvasOnlySnapshot(full)).toBe(false);
    expect(getCanvasSnapshotSequence(full)).toBe(4);
    adapter.dispose();
  });

  it("serializes operations during initialization and slow work and keeps activity until every queued request settles", async () => {
    vi.useFakeTimers();
    const { owner } = await fixture();
    const worker = new TestWorker();
    let initialize!: (adapter: WorkbenchAdapter) => void;
    worker.handler = createWorkbenchWorkerMessageHandler(new Promise((resolve) => { initialize = resolve; }), worker.reply);
    const adapter = new WorkerWorkbenchAdapter(worker);
    const constructed = adapter.construct({ version: 2 });
    const originalDispatch = owner.dispatch.bind(owner);
    let finishSolve!: () => void;
    const solving = new Promise<void>((resolve) => { finishSolve = resolve; });
    const order: string[] = [];
    owner.dispatch = async (input) => { order.push("dispatch"); await solving; return originalDispatch(input); };
    const originalSnapshot = owner.snapshot.bind(owner);
    owner.snapshot = async () => { order.push("snapshot"); return originalSnapshot(); };
    const dispatched = adapter.dispatch({ version: 2, command: "slow" });
    const read = adapter.snapshot();
    await vi.advanceTimersByTimeAsync(499);
    expect(adapter.activity.getSnapshot()).toBe(false);
    initialize(owner);
    await constructed;
    await vi.advanceTimersByTimeAsync(1);
    expect(order).toEqual(["dispatch"]);
    expect(adapter.activity.getSnapshot()).toBe(true);
    finishSolve();
    const [changed, reread] = await Promise.all([dispatched, read]);
    expect(order).toEqual(["dispatch", "snapshot"]);
    expect(getCanvasSnapshotSequence(reread)).toBeGreaterThan(getCanvasSnapshotSequence(changed)!);
    expect(adapter.activity.getSnapshot()).toBe(false);
    adapter.dispose();
  });

  it("retains the accepted owner after an operation rejects and does not poison the queue", async () => {
    const { adapter } = await fixture();
    const accepted = await adapter.construct({ version: 2 });
    const rejected = adapter.dispatch({ version: 2, command: "reject" });
    const following = adapter.snapshot();
    await expect(rejected).rejects.toThrow("candidate rejected");
    expect(await following).toEqual(accepted);
    expect(adapter.activity.getSnapshot()).toBe(false);
    adapter.dispose();
  });

  it.each([false, true])("keeps one delayed activity across prepared compilation and resolve (reject=%s)", async (reject) => {
    vi.useFakeTimers();
    const { adapter, worker, snapshot } = await fixture();
    await adapter.construct({ version: 2 });
    worker.handler = undefined;
    const current = compiledFixture as unknown as CompiledManagedSource;
    const digest = "a".repeat(64);
    const pending = { ...snapshot, pendingManagedMutation: { kind: "source" as const, request: {
      current, candidateSource: current.normalizedSource,
      ticket: { format: "geosolve-prepared-managed-source-v1" as const, ticketDigest: digest, project: "test",
        session: { session: 1, revision: 0, digest }, acceptedSourceDigest: digest, acceptedIrDigest: digest,
        acceptedArtifactDigest: digest, acceptedExpansionDigest: digest, declarationNameHighWater: 0,
        candidateInputSourceDigest: digest },
    } } };
    const prepared = adapter.dispatch({ version: 2, command: "source.prepare" });
    await vi.advanceTimersByTimeAsync(500);
    expect(adapter.activity.getSnapshot()).toBe(true);
    worker.reply({ id: 2, kind: "snapshot", sequence: 2, value: pending });
    await prepared;
    await vi.advanceTimersByTimeAsync(1000);
    expect(adapter.activity.getSnapshot()).toBe(true);
    const result = adapter.dispatch({ version: 2, command: "managed.mutation.resolve" });
    const settled = Promise.allSettled([result]);
    if (reject) worker.reply({ id: 3, kind: "error", message: "resolution rejected" });
    else worker.reply({ id: 3, kind: "snapshot", sequence: 3, value: snapshot });
    expect((await settled)[0].status).toBe(reject ? "rejected" : "fulfilled");
    expect(adapter.activity.getSnapshot()).toBe(false);
    expect(adapter.activity.getPendingSnapshot()).toBe(false);
    adapter.dispose();
  });

  it("rejects every queued request when WASM initialization fails", async () => {
    const worker = new TestWorker();
    worker.handler = createWorkbenchWorkerMessageHandler(Promise.reject(new Error("WASM unavailable")), worker.reply);
    const adapter = new WorkerWorkbenchAdapter(worker);
    const promises = [adapter.construct({ version: 2 }), adapter.toolCatalog()];
    const results = await Promise.allSettled(promises);
    expect(results.map((result) => result.status === "rejected" ? result.reason.message : "fulfilled")).toEqual(["WASM unavailable", "WASM unavailable"]);
    expect(adapter.activity.getSnapshot()).toBe(false);
    adapter.dispose();
  });

  it.each(["error", "messageerror", "dispose"] as const)("rejects all outstanding and future requests on %s and clears activity", async (failure) => {
    vi.useFakeTimers();
    const worker = new TestWorker();
    const adapter = new WorkerWorkbenchAdapter(worker);
    const settled = Promise.allSettled([adapter.construct({ version: 2 }), adapter.snapshot()]);
    await vi.advanceTimersByTimeAsync(500);
    expect(adapter.activity.getSnapshot()).toBe(true);
    if (failure === "dispose") adapter.dispose();
    else if (failure === "error") worker.dispatchEvent(new ErrorEvent("error", { message: "worker crashed" }));
    else worker.dispatchEvent(new MessageEvent("messageerror"));
    expect((await settled).every((result) => result.status === "rejected")).toBe(true);
    expect(adapter.activity.getSnapshot()).toBe(false);
    expect(worker.terminate).toHaveBeenCalledTimes(1);
    await expect(adapter.snapshot()).rejects.toThrow();
    adapter.dispose();
    expect(worker.terminate).toHaveBeenCalledTimes(1);
  });

  it("rejects a mismatched canvas basis and the rest of the queue", async () => {
    const { adapter, worker, snapshot } = await fixture();
    await adapter.construct({ version: 2 });
    worker.handler = undefined;
    const results = Promise.allSettled([adapter.wheel(wheel), adapter.snapshot()]);
    worker.reply({ id: 2, kind: "canvas", frame: snapshot.frame, sequence: 2, baseSequence: 99 });
    expect((await results).every((result) => result.status === "rejected")).toBe(true);
    expect(worker.terminate).toHaveBeenCalledTimes(1);
    expect(adapter.activity.getSnapshot()).toBe(false);
  });

  it("clears failed structured-clone activity and still permits later valid requests", async () => {
    const { adapter } = await fixture();
    await adapter.construct({ version: 2 });
    await expect(adapter.dispatch({ version: 2, command: "clone-failure", payload: () => undefined })).rejects.toThrow();
    expect(adapter.activity.getSnapshot()).toBe(false);
    expect((await adapter.snapshot()).version).toBe(2);
    adapter.dispose();
  });
});
