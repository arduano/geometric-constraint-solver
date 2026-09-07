// SPDX-License-Identifier: GPL-3.0-or-later
import { fireEvent, render, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { CanvasViewport } from "./canvas-viewport";
import { MockWorkbenchAdapter } from "../lib/mock-adapter";
import { createCanvasRenderer } from "../lib/canvas-renderer";

vi.mock("../lib/canvas-renderer", () => ({ createCanvasRenderer: vi.fn(() => ({ accept: vi.fn(), resize: vi.fn(), destroy: vi.fn() })) }));
afterEach(() => vi.restoreAllMocks());

async function setup() {
  const adapter = new MockWorkbenchAdapter(); const snapshot = await adapter.snapshot();
  const onCaptureChange = vi.fn(); const onSnapshot = vi.fn(); const onError = vi.fn();
  const view = render(<CanvasViewport adapter={adapter} snapshot={snapshot} onSnapshot={onSnapshot} onCaptureChange={onCaptureChange} onError={onError} />);
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
