// SPDX-License-Identifier: GPL-3.0-or-later
import { act, fireEvent, render, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { StrictMode } from "react";
import { markCanvasOnlySnapshot, stampCanvasSnapshot, type WheelSample, type WorkbenchAdapter, type WorkbenchSnapshot } from "../lib/adapter";
import { CanvasViewport } from "./canvas-viewport";
import { MockWorkbenchAdapter } from "../lib/mock-adapter";
import { createCanvasRenderer } from "../lib/canvas-renderer";

vi.mock("../lib/canvas-renderer", () => ({ createCanvasRenderer: vi.fn(() => ({ accept: vi.fn(), resize: vi.fn(), destroy: vi.fn() })) }));
afterEach(() => { vi.restoreAllMocks(); vi.unstubAllGlobals(); vi.useRealTimers(); });

async function setup(activeTool = "select", strict = false) {
  const adapter = new MockWorkbenchAdapter(); const snapshot = await adapter.snapshot();
  snapshot.presentation.activeTool = activeTool;
  const onCaptureChange = vi.fn(); const onSnapshot = vi.fn(); const onError = vi.fn();
  const element = <CanvasViewport adapter={adapter} snapshot={snapshot} onSnapshot={onSnapshot} onCaptureChange={onCaptureChange} onError={onError} />;
  const view = render(strict ? <StrictMode>{element}</StrictMode> : element);
  return { adapter, snapshot, onCaptureChange, onSnapshot, onError, view, host: view.getByRole("application") };
}
describe("canvas host lifecycle", () => {
  it("preserves CSS-local pointer input and normal pointer-up capture retirement", async () => {
    const h = await setup(); const pointer = vi.spyOn(h.adapter, "pointer"); const cancel = vi.spyOn(h.adapter, "cancel");
    vi.spyOn(h.host, "getBoundingClientRect").mockReturnValue(new DOMRect(100, 50, 2000, 700));
    // Native bridge owns the camera mapping: these remain CSS coordinates, independent of DPR.
    const send = (type: string, properties: Record<string, unknown>) => {
      const event = new Event(type, { bubbles: true }); Object.assign(event, { pointerId: 7, button: 0, buttons: 1, clientX: 620, clientY: 80, ...properties }); fireEvent(h.host, event);
    };
    send("pointerdown", {}); send("pointerup", { buttons: 0 }); send("lostpointercapture", {});
    await waitFor(() => expect(pointer).toHaveBeenCalledTimes(2));
    expect(pointer.mock.calls[0][0]).toMatchObject({ version: 2, phase: "down", x: 520, y: 30, pointerId: 7 });
    expect(h.onCaptureChange.mock.calls).toEqual([[true], [false]]); expect(cancel).not.toHaveBeenCalled();
  });
  it("forwards real lost capture once and ignores secondary mouse gestures", async () => {
    const h = await setup(); const pointer = vi.spyOn(h.adapter, "pointer"); const cancel = vi.spyOn(h.adapter, "cancel");
    const send = (type: string, properties: Record<string, unknown>) => { const event = new Event(type, { bubbles: true }); Object.assign(event, { pointerId: 7, button: 0, buttons: 1, clientX: 20, clientY: 30, ...properties }); fireEvent(h.host, event); };
    send("pointerdown", { button: 2, buttons: 2 }); send("pointermove", { buttons: 2 }); expect(pointer).not.toHaveBeenCalled();
    send("pointerdown", {}); send("lostpointercapture", {}); send("lostpointercapture", {});
    await waitFor(() => expect(cancel).toHaveBeenCalledTimes(1));
    expect(cancel).toHaveBeenCalledWith({ version: 2, reason: "lost-capture" });
  });
  it("owns one canvas renderer, forwards frames and disposes it without an SVG scene", async () => {
    const h = await setup(); const renderer = vi.mocked(createCanvasRenderer).mock.results.at(-1)!.value;
    expect(h.host.querySelectorAll("canvas")).toHaveLength(1); expect(h.host.querySelector("svg")).toBeNull();
    expect(renderer.accept).toHaveBeenCalledWith(h.snapshot.frame.scene);
    h.view.unmount(); expect(renderer.destroy).toHaveBeenCalledTimes(1);
  });
});


function pointerEvent(host: HTMLElement, type: string, properties: Record<string, unknown> = {}) {
  const event = new Event(type, { bubbles: true });
  Object.assign(event, { pointerId: 7, button: 0, buttons: 0, clientX: 20, clientY: 30, ...properties });
  fireEvent(host, event);
}
function animationFrames() {
  let identifier = 0;
  const pending = new Map<number, FrameRequestCallback>();
  vi.stubGlobal("requestAnimationFrame", vi.fn((callback: FrameRequestCallback) => { pending.set(++identifier, callback); return identifier; }));
  vi.stubGlobal("cancelAnimationFrame", vi.fn((id: number) => { pending.delete(id); }));
  return async () => {
    await act(async () => {
      const batch = [...pending.values()]; pending.clear();
      for (const callback of batch) callback(16);
    });
  };
}

describe("M97 idle dimension preview", () => {
  const advance = async (milliseconds: number) => { await act(async () => { await vi.advanceTimersByTimeAsync(milliseconds); }); };

  it("waits for 250 ms of idle Select input and sends only the latest CSS coordinates", async () => {
    vi.useFakeTimers(); const frame = animationFrames(); const h = await setup();
    const dispatch = vi.spyOn(h.adapter, "dispatch");
    vi.spyOn(h.host, "getBoundingClientRect").mockReturnValue(new DOMRect(100, 50, 900, 700));
    pointerEvent(h.host, "pointermove", { clientX: 150, clientY: 80 });
    await frame(); await advance(200);
    expect(dispatch).not.toHaveBeenCalled();
    pointerEvent(h.host, "pointermove", { clientX: 175, clientY: 95 });
    await frame(); await advance(249);
    expect(dispatch).not.toHaveBeenCalled();
    await advance(1);
    expect(dispatch).toHaveBeenCalledExactlyOnceWith({ version: 2, command: "dimensions.hover", payload: { x: 75, y: 45 } });
    await advance(2000);
    expect(dispatch).toHaveBeenCalledTimes(1);
  });

  it.each(["wheel", "leave", "down", "blur", "unmount", "tool", "hidden"])("cancels the pending preview on %s", async (action) => {
    vi.useFakeTimers(); const frame = animationFrames(); const h = await setup();
    const dispatch = vi.spyOn(h.adapter, "dispatch");
    pointerEvent(h.host, "pointermove"); await frame(); await advance(100);
    if (action === "wheel") fireEvent.wheel(h.host, { deltaY: 1 });
    if (action === "leave") fireEvent.pointerLeave(h.host);
    if (action === "down") pointerEvent(h.host, "pointerdown", { buttons: 1 });
    if (action === "blur") fireEvent(window, new Event("blur"));
    if (action === "unmount") h.view.unmount();
    if (action === "tool" || action === "hidden") h.view.rerender(<CanvasViewport adapter={h.adapter} snapshot={{ ...h.snapshot, presentation: { ...h.snapshot.presentation, activeTool: action === "tool" ? "line" : "select" }, dimensions: { mode: "hidden", entries: [], parameters: [], pinCount: 0 } }} onSnapshot={h.onSnapshot} onCaptureChange={h.onCaptureChange} onError={h.onError} />);
    await advance(1000);
    expect(dispatch).not.toHaveBeenCalledWith(expect.objectContaining({ command: "dimensions.hover" }));
  });

  it("preserves an existing preview during pointer transit and retires it on leave using frame-only transport", async () => {
    vi.useFakeTimers(); const frame = animationFrames(); const h = await setup();
    const dispatch = vi.spyOn(h.adapter, "dispatch").mockImplementation(async () => markCanvasOnlySnapshot({ ...h.snapshot, frame: { ...h.snapshot.frame } }));
    pointerEvent(h.host, "pointermove"); await frame(); h.onSnapshot.mockClear(); await advance(250);
    expect(dispatch).toHaveBeenCalledTimes(1);
    expect(h.onSnapshot).not.toHaveBeenCalled();
    pointerEvent(h.host, "pointermove", { clientX: 50 }); await frame();
    await advance(100);
    expect(dispatch).not.toHaveBeenCalledWith(expect.objectContaining({ command: "dimensions.hover.clear" }));
    fireEvent.pointerLeave(h.host); await advance(1);
    expect(dispatch).toHaveBeenLastCalledWith({ version: 2, command: "dimensions.hover.clear" });
    await advance(500);
    expect(dispatch).toHaveBeenCalledTimes(2);
  });

  it("serializes preview after pointer work and discards a stale preview queued behind it", async () => {
    vi.useFakeTimers(); const frame = animationFrames(); const h = await setup();
    let release!: () => void;
    vi.spyOn(h.adapter, "pointer").mockImplementationOnce(() => new Promise<null>((resolve) => { release = () => resolve(null); }));
    const dispatch = vi.spyOn(h.adapter, "dispatch");
    pointerEvent(h.host, "pointermove"); await frame(); await advance(250);
    expect(dispatch).not.toHaveBeenCalled();
    pointerEvent(h.host, "pointermove", { clientX: 70 });
    await act(async () => release()); await frame(); await advance(250);
    expect(dispatch).toHaveBeenCalledExactlyOnceWith({ version: 2, command: "dimensions.hover", payload: { x: 70, y: 30 } });
  });

  it("hit-tests a preview label before clearing hover ownership on pointer-down", async () => {
    vi.useFakeTimers(); const frame = animationFrames(); const h = await setup();
    const order: string[] = [];
    vi.spyOn(h.adapter, "pointer").mockImplementation(async (sample) => { order.push(sample.phase); return null; });
    vi.spyOn(h.adapter, "dispatch").mockImplementation(async (command) => { order.push(command.command); return markCanvasOnlySnapshot(h.snapshot); });
    pointerEvent(h.host, "pointermove"); await frame(); await advance(250);
    pointerEvent(h.host, "pointerdown", { buttons: 1 }); await advance(1);
    expect(order).toEqual(["move", "dimensions.hover", "down", "dimensions.hover.clear"]);
  });
});

describe("canvas input scheduling", () => {
  it("settles dimensions once after 180 ms without wheel input and paints the frame without a React update", async () => {
    vi.useFakeTimers(); const frame = animationFrames(); const h = await setup();
    const advance = async (ms: number) => { await act(async () => { await vi.advanceTimersByTimeAsync(ms); }); };
    const dispatch = vi.spyOn(h.adapter, "dispatch").mockResolvedValue(markCanvasOnlySnapshot(h.snapshot));
    const wheel = vi.spyOn(h.adapter, "wheel").mockResolvedValue(null);
    fireEvent.wheel(h.host, { deltaY: -30 }); await frame(); await advance(120);
    fireEvent.wheel(h.host, { deltaY: -40 }); await frame(); await advance(179);
    expect(wheel).toHaveBeenCalledTimes(2); expect(dispatch).not.toHaveBeenCalled();
    h.onSnapshot.mockClear(); await advance(1);
    expect(dispatch).toHaveBeenCalledExactlyOnceWith({ version: 2, command: "dimensions.navigation.end" });
    expect(h.onSnapshot).not.toHaveBeenCalled();
    await advance(1000); expect(dispatch).toHaveBeenCalledTimes(1);
  });

  it("orders navigation settle behind wheel work and invalidates a queued settle when a new wheel arrives", async () => {
    vi.useFakeTimers(); const frame = animationFrames(); const h = await setup();
    const advance = async (ms: number) => { await act(async () => { await vi.advanceTimersByTimeAsync(ms); }); };
    let release!: () => void; const order: string[] = [];
    vi.spyOn(h.adapter, "wheel").mockImplementationOnce(() => { order.push("wheel:first"); return new Promise<null>((resolve) => { release = () => resolve(null); }); }).mockImplementation(async () => { order.push("wheel:second"); return null; });
    vi.spyOn(h.adapter, "dispatch").mockImplementation(async ({ command }) => { order.push(command); return markCanvasOnlySnapshot(h.snapshot); });
    fireEvent.wheel(h.host, { deltaY: -30 }); await frame(); await advance(180);
    expect(order).toEqual(["wheel:first"]);
    fireEvent.wheel(h.host, { deltaY: -40 }); await frame();
    await act(async () => release());
    expect(order).toEqual(["wheel:first", "wheel:second"]);
    await advance(179); expect(order).toHaveLength(2);
    await advance(1);
    expect(order).toEqual(["wheel:first", "wheel:second", "dimensions.navigation.end"]);
  });

  it.each([false, true])("cancels navigation settle on disposal, including an already queued callback (%s)", async (queued) => {
    vi.useFakeTimers(); const frame = animationFrames(); const h = await setup();
    let release!: () => void;
    vi.spyOn(h.adapter, "wheel").mockImplementation(() => new Promise<null>((resolve) => { release = () => resolve(null); }));
    const dispatch = vi.spyOn(h.adapter, "dispatch");
    fireEvent.wheel(h.host, { deltaY: 20 }); await frame();
    await act(async () => { await vi.advanceTimersByTimeAsync(queued ? 180 : 100); });
    h.view.unmount();
    await act(async () => { release(); await vi.advanceTimersByTimeAsync(500); });
    expect(dispatch).not.toHaveBeenCalled();
  });

  it("coalesces Select hover to the newest CSS-local sample once per animation frame", async () => {
    const frame = animationFrames(); const h = await setup(); const pointer = vi.spyOn(h.adapter, "pointer");
    vi.spyOn(h.host, "getBoundingClientRect").mockReturnValue(new DOMRect(100, 50, 900, 700));
    for (let offset = 0; offset < 100; offset += 1) pointerEvent(h.host, "pointermove", { clientX: 300 + offset, clientY: 100 });
    expect(pointer).not.toHaveBeenCalled();
    await frame();
    expect(pointer).toHaveBeenCalledTimes(1);
    expect(pointer.mock.calls[0][0]).toMatchObject({ phase: "move", x: 299, y: 50, buttons: 0 });
    await frame(); expect(pointer).toHaveBeenCalledTimes(1);
  });

  it("drains newest middle pan before exact release while retiring capture immediately", async () => {
    const frame = animationFrames(); const h = await setup(); const pointer = vi.spyOn(h.adapter, "pointer");
    pointerEvent(h.host, "pointerdown", { button: 1, buttons: 4 });
    pointerEvent(h.host, "pointermove", { button: -1, buttons: 4, clientX: 50 });
    pointerEvent(h.host, "pointermove", { button: -1, buttons: 4, clientX: 70 });
    pointerEvent(h.host, "pointerup", { button: 1, clientX: 90 });
    pointerEvent(h.host, "lostpointercapture");
    expect(h.onCaptureChange.mock.calls).toEqual([[true], [false]]);
    await waitFor(() => expect(pointer).toHaveBeenCalledTimes(3));
    expect(pointer.mock.calls.map(([sample]) => [sample.phase, sample.x])).toEqual([["down", 20], ["move", 70], ["up", 90]]);
    await frame(); expect(pointer).toHaveBeenCalledTimes(3);
  });

  it("preserves every solver drag and authoring move in input order", async () => {
    animationFrames(); const h = await setup(); const pointer = vi.spyOn(h.adapter, "pointer");
    pointerEvent(h.host, "pointerdown", { buttons: 1 });
    for (const x of [30, 40, 50]) pointerEvent(h.host, "pointermove", { buttons: 1, clientX: x });
    pointerEvent(h.host, "pointerup", { clientX: 60 });
    await waitFor(() => expect(pointer).toHaveBeenCalledTimes(5));
    expect(pointer.mock.calls.map(([sample]) => sample.x)).toEqual([20, 30, 40, 50, 60]);
    const authoring = { ...h.snapshot, presentation: { ...h.snapshot.presentation, activeTool: "line" } };
    h.view.rerender(<CanvasViewport adapter={h.adapter} snapshot={authoring} onSnapshot={h.onSnapshot} onCaptureChange={h.onCaptureChange} onError={h.onError} />);
    for (const x of [70, 80, 90]) pointerEvent(h.host, "pointermove", { clientX: x });
    await waitFor(() => expect(pointer).toHaveBeenCalledTimes(8));
    expect(pointer.mock.calls.slice(5).map(([sample]) => sample.x)).toEqual([70, 80, 90]);
  });

  it("flushes pending input before loss cancellation and waits for asynchronous operations", async () => {
    animationFrames(); const h = await setup();
    let completeDown!: (snapshot: WorkbenchSnapshot | null) => void;
    const down = new Promise<WorkbenchSnapshot | null>((resolve) => { completeDown = resolve; });
    const order: string[] = [];
    vi.spyOn(h.adapter, "pointer").mockImplementation(async (sample) => { order.push(`${sample.phase}:${sample.x}`); return sample.phase === "down" ? down : null; });
    vi.spyOn(h.adapter, "cancel").mockImplementation(async () => { order.push("cancel"); return null; });
    pointerEvent(h.host, "pointerdown", { button: 1, buttons: 4 });
    pointerEvent(h.host, "pointermove", { buttons: 4, clientX: 80 });
    pointerEvent(h.host, "lostpointercapture");
    expect(order).toEqual(["down:20"]);
    await act(async () => completeDown(null));
    await waitFor(() => expect(order).toEqual(["down:20", "move:80", "cancel"]));
  });

  it("batches exact ordered wheel operations without merging changing anchors or opposite deltas", async () => {
    const frame = animationFrames(); const h = await setup();
    const wheelBatch = vi.fn(async (_samples: WheelSample[]) => null);
    (h.adapter as WorkbenchAdapter).wheelBatch = wheelBatch;
    const pointer = vi.spyOn(h.adapter, "pointer");
    pointerEvent(h.host, "pointermove", { clientX: 35 });
    fireEvent.wheel(h.host, { clientX: 60, clientY: 70, deltaX: 2, deltaY: 10000 });
    fireEvent.wheel(h.host, { clientX: 80, clientY: 90, deltaX: -3, deltaY: -10000, ctrlKey: true });
    expect(pointer).toHaveBeenCalledTimes(1); expect(wheelBatch).not.toHaveBeenCalled();
    await frame();
    expect(wheelBatch).toHaveBeenCalledExactlyOnceWith([
      { version: 2, x: 60, y: 70, deltaX: 2, deltaY: 10000, ctrl: false },
      { version: 2, x: 80, y: 90, deltaX: -3, deltaY: -10000, ctrl: true },
    ]);
  });

  it("flushes wheel input before external pointer and keyboard command handlers", async () => {
    const frame = animationFrames(); const h = await setup(); const order: string[] = [];
    (h.adapter as WorkbenchAdapter).wheelBatch = vi.fn(async () => { order.push("wheel"); return null; });
    const clicked = () => order.push("outside-click");
    const keyed = () => order.push("outside-key");
    document.addEventListener("pointerdown", clicked);
    document.addEventListener("keydown", keyed);
    try {
      fireEvent.wheel(h.host, { clientX: 11, deltaY: 40 });
      pointerEvent(document.body, "pointerdown");
      expect(order).toEqual(["wheel", "outside-click"]);
      await act(async () => {});
      fireEvent.wheel(h.host, { clientX: 22, deltaY: -10 });
      fireEvent.keyDown(document.body, { key: "Escape" });
      expect(order).toEqual(["wheel", "outside-click", "wheel", "outside-key"]);
      await frame(); expect(order).toHaveLength(4);
    } finally {
      document.removeEventListener("pointerdown", clicked);
      document.removeEventListener("keydown", keyed);
    }
  });

  it("bounds wheel batches and preserves fallback wheel order before a pointer terminal", async () => {
    const frame = animationFrames(); const h = await setup();
    const batches: WheelSample[][] = [];
    (h.adapter as WorkbenchAdapter).wheelBatch = vi.fn(async (samples) => { batches.push(samples); return null; });
    for (let x = 0; x < 260; x += 1) fireEvent.wheel(h.host, { clientX: x, clientY: 20, deltaY: x % 2 ? 300 : -300 });
    await frame();
    expect(batches.map((batch) => batch.length)).toEqual([256, 4]);
    expect(batches.flat().map((sample) => sample.x)).toEqual(Array.from({ length: 260 }, (_, x) => x));
    delete (h.adapter as WorkbenchAdapter).wheelBatch;
    const order: string[] = [];
    vi.spyOn(h.adapter as WorkbenchAdapter, "wheel").mockImplementation(async (sample: WheelSample) => { order.push(`wheel:${sample.x}`); return null; });
    vi.spyOn(h.adapter, "pointer").mockImplementation(async (sample) => { order.push(`pointer:${sample.phase}`); return null; });
    fireEvent.wheel(h.host, { clientX: 11, deltaY: 40 }); fireEvent.wheel(h.host, { clientX: 22, deltaY: -10 });
    pointerEvent(h.host, "pointerdown", { buttons: 1 });
    await waitFor(() => expect(order).toEqual(["wheel:11", "wheel:22", "pointer:down"]));
  });

  it("drops stale queued input and in-flight publications on adapter replacement and unmount", async () => {
    const frame = animationFrames(); const h = await setup(); const pointer = vi.spyOn(h.adapter, "pointer");
    pointerEvent(h.host, "pointermove");
    const replacement = new MockWorkbenchAdapter();
    h.view.rerender(<CanvasViewport adapter={replacement} snapshot={h.snapshot} onSnapshot={h.onSnapshot} onCaptureChange={h.onCaptureChange} onError={h.onError} />);
    await frame(); expect(pointer).not.toHaveBeenCalled();
    let complete!: (snapshot: WorkbenchSnapshot) => void;
    vi.spyOn(replacement, "pointer").mockImplementation(() => new Promise((resolve) => { complete = resolve; }));
    pointerEvent(h.host, "pointerdown", { buttons: 1 });
    h.view.unmount();
    await act(async () => complete(h.snapshot));
    expect(h.onSnapshot).not.toHaveBeenCalled(); expect(h.onError).not.toHaveBeenCalled();
  });

  it("drops deferred hover on pointer leave and hidden documents", async () => {
    const frame = animationFrames(); const h = await setup(); const pointer = vi.spyOn(h.adapter, "pointer");
    pointerEvent(h.host, "pointermove"); pointerEvent(h.host, "pointerout");
    await frame(); expect(pointer).not.toHaveBeenCalled();
    pointerEvent(h.host, "pointermove");
    vi.spyOn(document, "hidden", "get").mockReturnValue(true);
    fireEvent(document, new Event("visibilitychange"));
    await frame(); expect(pointer).not.toHaveBeenCalled();
  });

  it("paints validated canvas-only results directly and keeps newer camera frames across delayed full commits", async () => {
    const frame = animationFrames(); const h = await setup();
    const renderer = vi.mocked(createCanvasRenderer).mock.results.at(-1)!.value;
    const full = { ...h.snapshot, frame: { ...h.snapshot.frame, scene: structuredClone(h.snapshot.frame.scene) } };
    const delta = markCanvasOnlySnapshot({ ...full, frame: { ...full.frame, scene: structuredClone(full.frame.scene) } });
    vi.spyOn(h.adapter, "pointer").mockResolvedValueOnce(full).mockResolvedValueOnce(delta);
    pointerEvent(h.host, "pointermove", { clientX: 40 }); await frame();
    pointerEvent(h.host, "pointermove", { clientX: 60 }); await frame();
    expect(h.onSnapshot).toHaveBeenCalledExactlyOnceWith(full);
    expect(vi.mocked(renderer.accept).mock.calls.at(-1)?.[0]).toBe(delta.frame.scene);
    h.view.rerender(<CanvasViewport adapter={h.adapter} snapshot={full} onSnapshot={h.onSnapshot} onCaptureChange={h.onCaptureChange} onError={h.onError} />);
    expect(vi.mocked(renderer.accept).mock.calls.at(-1)?.[0]).toBe(delta.frame.scene);
    const external = { ...full, frame: { ...full.frame, scene: structuredClone(full.frame.scene) } };
    h.view.rerender(<CanvasViewport adapter={h.adapter} snapshot={external} onSnapshot={h.onSnapshot} onCaptureChange={h.onCaptureChange} onError={h.onError} />);
    expect(vi.mocked(renderer.accept).mock.calls.at(-1)?.[0]).toBe(external.frame.scene);
  });

  it("rejects stale external full frames and late input results using adapter decode order", async () => {
    const frame = animationFrames(); const h = await setup();
    const renderer = vi.mocked(createCanvasRenderer).mock.results.at(-1)!.value;
    const external = stampCanvasSnapshot({ ...h.snapshot, frame: { ...h.snapshot.frame, scene: structuredClone(h.snapshot.frame.scene) } }, 1);
    const delta = markCanvasOnlySnapshot(stampCanvasSnapshot({ ...h.snapshot, frame: { ...h.snapshot.frame, scene: structuredClone(h.snapshot.frame.scene) } }, 2));
    vi.spyOn(h.adapter, "pointer").mockResolvedValueOnce(delta).mockResolvedValueOnce(external);
    pointerEvent(h.host, "pointermove"); await frame();
    const presentations = vi.mocked(renderer.accept).mock.calls.length;
    h.view.rerender(<CanvasViewport adapter={h.adapter} snapshot={external} onSnapshot={h.onSnapshot} onCaptureChange={h.onCaptureChange} onError={h.onError} />);
    expect(renderer.accept).toHaveBeenCalledTimes(presentations);
    expect(vi.mocked(renderer.accept).mock.calls.at(-1)?.[0]).toBe(delta.frame.scene);
    pointerEvent(h.host, "pointermove"); await frame();
    expect(renderer.accept).toHaveBeenCalledTimes(presentations);
    expect(h.onSnapshot).not.toHaveBeenCalled();
    const replacement = stampCanvasSnapshot({ ...h.snapshot, frame: { ...h.snapshot.frame, scene: structuredClone(h.snapshot.frame.scene) } }, 3);
    h.view.rerender(<CanvasViewport adapter={h.adapter} snapshot={replacement} onSnapshot={h.onSnapshot} onCaptureChange={h.onCaptureChange} onError={h.onError} />);
    expect(vi.mocked(renderer.accept).mock.calls.at(-1)?.[0]).toBe(replacement.frame.scene);
  });

  it("flushes movement before positive resize and drops deferred input when the canvas becomes hidden", async () => {
    const frame = animationFrames();
    let resized!: ResizeObserverCallback;
    vi.stubGlobal("ResizeObserver", class {
      constructor(callback: ResizeObserverCallback) { resized = callback; }
      observe() {} disconnect() {} unobserve() {}
    });
    const h = await setup(); const order: string[] = [];
    vi.spyOn(h.adapter, "pointer").mockImplementation(async (sample) => { order.push(`pointer:${sample.x}`); return null; });
    vi.spyOn(h.adapter as WorkbenchAdapter, "resize").mockImplementation(async (sample) => { order.push(`resize:${sample.width}`); return null; });
    const resize = (width: number, height: number) => act(() => resized([{ target: h.host, contentRect: new DOMRect(0, 0, width, height), borderBoxSize: [], contentBoxSize: [], devicePixelContentBoxSize: [] }], {} as ResizeObserver));
    pointerEvent(h.host, "pointermove", { clientX: 55 });
    resize(500, 400);
    await waitFor(() => expect(order).toEqual(["pointer:55", "resize:500"]));
    pointerEvent(h.host, "pointermove", { clientX: 75 });
    resize(0, 0);
    await frame();
    expect(order).toEqual(["pointer:55", "resize:500"]);
  });

  it("continues processing after React StrictMode effect teardown and setup", async () => {
    const frame = animationFrames(); const h = await setup("select", true); const pointer = vi.spyOn(h.adapter, "pointer");
    pointerEvent(h.host, "pointermove"); await frame(); expect(pointer).toHaveBeenCalledTimes(1);
  });
});
