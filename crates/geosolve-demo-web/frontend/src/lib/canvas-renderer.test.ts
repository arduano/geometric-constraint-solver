// SPDX-License-Identifier: GPL-3.0-or-later
import { describe, expect, it, vi } from "vitest";
import { createCanvasRenderer } from "./canvas-renderer";
import { assertDrawFrame, freezeDrawFrame, immutableDrawFrame, type DrawFrame, type DrawStyle } from "./canvas-scene";
import { dashedSegments, fitDrawing, textRasterOrigin } from "./canvas-renderer-geometry";
import type { CanvasBackend } from "./canvas-renderer-pixi";

const style: DrawStyle = { fill: null, stroke: "#ffffff", strokeWidth: 2.2, dash: [], opacity: 1, lineCap: "round", lineJoin: "round", nonScalingStroke: true, textBaseline: "alphabetic", letterSpacing: 0, fontFamily: "sans-serif", fontSize: 12, fontWeight: 400, textAnchor: "start", shadow: null };
const frame = (x = 10): DrawFrame => ({ format: "geosolve-draw-frame-v1", viewBox: [0, 0, 1000, 700], background: "#151617", provenance: { revision: "same" }, items: [{ id: "circle-1", layer: "geometry", semanticKey: "circle:1", accessibleLabel: null, className: "wb-point", interactive: true, metadata: {}, style: { ...style }, kind: "circle", center: [x, 20], radius: 4 }] });
function harness() {
  const canvas = document.createElement("canvas");
  const callbacks = new Map<number, FrameRequestCallback>();
  let serial = 0;
  let resolve!: (backend: CanvasBackend) => void;
  let reject!: (reason: Error) => void;
  const backend: CanvasBackend = { hardware: Object.freeze({ api: "WebGL2", renderer: "unit-test-double" }), render: vi.fn(() => ({ resources: 1, created: 1, destroyed: 0, updated: 1 })), destroy: vi.fn() };
  const renderer = createCanvasRenderer(canvas, { createBackend: () => new Promise((yes, no) => { resolve = yes; reject = no; }), requestFrame: (callback) => { callbacks.set(++serial, callback); return serial; }, cancelFrame: (id) => { callbacks.delete(id); }, now: () => 0 });
  const flush = () => { const batch = [...callbacks.values()]; callbacks.clear(); batch.forEach((callback) => callback(0)); };
  const ready = async () => { resolve(backend); await Promise.resolve(); };
  renderer.resize({ width: 1000, height: 700, pixelRatio: 1 });
  return { canvas, callbacks, backend, renderer, flush, ready, reject };
}

describe("on-demand accepted-frame presentation", () => {
  it("coalesces camera/hover frame changes even when semantic revision is unchanged", async () => {
    const h = harness(); h.renderer.accept(frame()); h.renderer.accept(frame(12));
    expect(h.canvas.__geosolvePresentedFrame).toBeNull();
    await h.ready(); expect(h.callbacks.size).toBe(1); h.flush();
    expect(h.backend.render).toHaveBeenCalledTimes(1);
    expect(h.canvas.__geosolvePresentedFrame).toEqual(frame(12));
    expect(h.canvas.__geosolveRendererDiagnostics?.state).toBe("ready");
    expect(h.callbacks.size).toBe(0);
    h.renderer.accept(frame(12)); expect(h.callbacks.size).toBe(0);
    h.renderer.destroy();
  });
  it("records only successful presentation and keeps observers and input immutable", async () => {
    const h = harness(); const input = frame(); h.renderer.accept(input); input.items[0].style.stroke = "#ff0000";
    await h.ready(); h.flush();
    expect(h.canvas.__geosolvePresentedFrame?.items[0].style.stroke).toBe("#ffffff");
    expect(() => { h.canvas.__geosolvePresentedFrame!.items[0].style.opacity = 0; }).toThrow();
    expect(Reflect.set(h.canvas, "__geosolvePresentedFrame", frame(99))).toBe(false);
    vi.mocked(h.backend.render).mockImplementationOnce(() => { throw new Error("draw failed"); });
    h.renderer.accept(frame(14)); h.flush();
    expect(h.canvas.__geosolvePresentedFrame).toEqual(frame());
    expect(h.canvas.dataset.presentedFrame).toBe("1");
    expect(h.canvas.__geosolveRendererDiagnostics?.state).toBe("unavailable");
    h.renderer.destroy();
  });
  it("retains a boundary-owned frozen frame directly and skips unchanged publications", async () => {
    const h = harness(); const input = freezeDrawFrame(frame());
    h.renderer.accept(input); await h.ready(); h.flush();
    expect(h.canvas.__geosolvePresentedFrame).toBe(input);
    h.renderer.accept(input);
    h.renderer.accept(freezeDrawFrame(frame()));
    expect(h.callbacks.size).toBe(0);
    // Non-paint metadata still belongs to the exact presented-frame observer.
    h.renderer.accept(freezeDrawFrame({ ...frame(), provenance: { revision: "next" } })); h.flush();
    expect(h.canvas.__geosolvePresentedFrame?.provenance).toEqual({ revision: "next" });
    expect(h.backend.render).toHaveBeenCalledTimes(2);
    h.renderer.destroy();
  });
  it("cancels queued draws during context loss and restores the newest retained input", async () => {
    const h = harness(); await h.ready(); h.renderer.accept(frame()); h.flush();
    h.renderer.accept(frame(15)); expect(h.callbacks.size).toBe(1);
    const lost = new Event("webglcontextlost", { cancelable: true }); h.canvas.dispatchEvent(lost);
    expect(lost.defaultPrevented).toBe(true); expect(h.callbacks.size).toBe(0);
    h.renderer.accept(frame(19)); expect(h.callbacks.size).toBe(0);
    expect(h.canvas.__geosolveRendererDiagnostics?.state).toBe("lost");
    expect(h.canvas.__geosolvePresentedFrame).toEqual(frame());
    h.canvas.dispatchEvent(new Event("webglcontextrestored")); h.flush();
    expect(h.canvas.__geosolvePresentedFrame).toEqual(frame(19));
    expect(h.canvas.__geosolveRendererDiagnostics?.state).toBe("ready");
    h.renderer.destroy();
  });
  it("survives loss before initialization and a DPR-only resize without a new snapshot", async () => {
    const h = harness(); h.renderer.accept(frame()); h.canvas.dispatchEvent(new Event("webglcontextlost", { cancelable: true }));
    await h.ready(); expect(h.callbacks.size).toBe(0);
    h.canvas.dispatchEvent(new Event("webglcontextrestored")); h.flush();
    h.renderer.resize({ width: 1000, height: 700, pixelRatio: 2 }); h.flush();
    expect(h.backend.render).toHaveBeenLastCalledWith(frame(), { width: 1000, height: 700, pixelRatio: 2 });
    expect(h.canvas.__geosolveRendererDiagnostics?.frameCount).toBe(2);
    expect(h.canvas.__geosolveRendererDiagnostics?.lastFrameId).toBe(1);
    h.renderer.destroy();
  });
  it("waits for a nonzero viewport, fails explicitly without WebGL2, and does not leave a RAF loop", async () => {
    const h = harness(); h.renderer.accept(frame()); h.reject(new Error("WebGL2 is unavailable"));
    await Promise.resolve(); await Promise.resolve();
    expect(h.canvas.dataset.renderState).toBe("unavailable"); expect(h.callbacks.size).toBe(0);
    expect(h.canvas.__geosolvePresentedFrame).toBeNull(); h.renderer.destroy();
  });
  it("releases an asynchronously initialized backend after unmount and removes observers", async () => {
    const h = harness(); h.renderer.accept(frame()); h.renderer.destroy(); await h.ready();
    expect(h.backend.destroy).toHaveBeenCalledTimes(1);
    expect(h.canvas.__geosolvePresentedFrame).toBeUndefined();
    expect(h.canvas.__geosolveRendererDiagnostics).toBeUndefined();
    expect(h.callbacks.size).toBe(0);
    h.renderer.destroy(); expect(h.backend.destroy).toHaveBeenCalledTimes(1);
  });
  it("suspends queued drawing when hidden and presents the newest frame when visible again", async () => {
    const h = harness(); await h.ready(); h.renderer.accept(frame()); h.flush();
    h.renderer.accept(frame(12)); expect(h.callbacks.size).toBe(1);
    h.renderer.resize({ width: 0, height: 0, pixelRatio: 1 });
    expect(h.callbacks.size).toBe(0);
    h.renderer.accept(frame(20)); h.flush();
    expect(h.backend.render).toHaveBeenCalledTimes(1);
    expect(h.canvas.__geosolvePresentedFrame).toEqual(frame());
    h.renderer.resize({ width: 1000, height: 700, pixelRatio: 2 }); h.flush();
    expect(h.backend.render).toHaveBeenCalledTimes(2);
    expect(h.canvas.__geosolvePresentedFrame).toEqual(frame(20));
    h.renderer.destroy();
  });
});

describe("typed drawing boundary and raster stroke helpers", () => {
  it("freezes every boundary child and never trusts a merely shallow-frozen or mutable caller", () => {
    const input = frame(); const frozen = freezeDrawFrame(input);
    expect(frozen).toBe(input); expect(immutableDrawFrame(frozen)).toBe(frozen);
    expect(() => { frozen.items[0].style.dash.push(4); }).toThrow();
    expect(() => { frozen.viewBox[2] = 4; }).toThrow();
    const mutable = frame(); const isolated = immutableDrawFrame(mutable);
    mutable.items[0].style.opacity = 0;
    expect(isolated.items[0].style.opacity).toBe(1);
    const shallow = Object.freeze(frame()); shallow.items[0].style.strokeWidth = Infinity;
    expect(() => freezeDrawFrame(shallow)).toThrow("Invalid geosolve drawing frame");
    expect(() => immutableDrawFrame(shallow)).toThrow("Invalid geosolve drawing frame");
  });
  it("rejects invalid geometry, non-finite styles, duplicate keys and unsupported primitives", () => {
    for (const bad of [
      { ...frame(), viewBox: [0, 0, 0, 700] },
      { ...frame(), items: [frame().items[0], frame().items[0]] },
      { ...frame(), items: [{ ...frame().items[0], radius: NaN }] },
      { ...frame(), items: [{ ...frame().items[0], radius: -1 }] },
      { ...frame(), items: [{ ...frame().items[0], style: { ...style, strokeWidth: Infinity } }] },
      { ...frame(), items: [{ ...frame().items[0], style: { ...style, dash: [0, 0] } }] },
      { ...frame(), items: [{ ...frame().items[0], kind: "svg", markup: "<path />" }] },
    ]) expect(() => assertDrawFrame(bad)).toThrow();
    expect(() => assertDrawFrame(frame())).not.toThrow();
  });
  it("fits the same centered logical plane for horizontal and vertical letterboxing", () => {
    expect(fitDrawing([0, 0, 1000, 700], 2000, 700)).toEqual({ scale: 1, x: 500, y: 0 });
    expect(fitDrawing([0, 0, 1000, 700], 500, 700)).toEqual({ scale: 0.5, x: 0, y: 175 });
    expect(fitDrawing([10, 20, 1000, 700], 1000, 700)).toEqual({ scale: 1, x: -10, y: -20 });
  });
  it("anchors text to glyph advance and alphabetic/central baselines, excluding halo extents", () => {
    const metrics = { maxLineWidth: 60, lineHeight: 22, fontProperties: { ascent: 15, descent: 5, fontSize: 20 } };
    expect(textRasterOrigin(metrics, "start", "alphabetic", 4)).toEqual({ x: 2, y: 18 });
    expect(textRasterOrigin(metrics, "middle", "central", 4)).toEqual({ x: 32, y: 13 });
    expect(textRasterOrigin(metrics, "end", "alphabetic", 0)).toEqual({ x: 60, y: 16 });
  });
  it("keeps dash phase through corners and odd-length patterns", () => {
    expect(dashedSegments([[0, 0], [3, 0], [3, 7]], false, [5, 2])).toEqual([[[0, 0], [3, 0], [3, 2]], [[3, 4], [3, 7]]]);
    expect(dashedSegments([[0, 0], [12, 0]], false, [3])).toEqual([[[0, 0], [3, 0]], [[6, 0], [9, 0]]]);
    expect(dashedSegments([[0, 0], [2, 0]], true, [3, 1])).toEqual([[[0, 0], [2, 0], [1, 0]]]);
  });
});
