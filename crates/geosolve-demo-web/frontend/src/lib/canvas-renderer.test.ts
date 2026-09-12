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

describe("interaction raster quality", () => {
  it("coalesces idle hover within a bounded window and lets navigation replace it immediately", async () => {
    vi.useFakeTimers(); const h = harness();
    try {
      h.renderer.accept(frame()); await h.ready(); h.flush();
      h.renderer.accept(frame(12), "hover");
      expect(h.callbacks.size).toBe(0);
      expect(h.canvas.__geosolveRendererDiagnostics?.hasPendingDraw).toBe(true);
      await vi.advanceTimersByTimeAsync(16);
      h.renderer.accept(frame(14), "hover");
      await vi.advanceTimersByTimeAsync(15); h.flush();
      expect(h.backend.render).toHaveBeenCalledTimes(1);
      await vi.advanceTimersByTimeAsync(1); h.flush();
      expect(h.canvas.__geosolvePresentedFrame).toEqual(frame(14));
      h.renderer.accept(frame(16), "hover");
      await vi.advanceTimersByTimeAsync(8);
      h.renderer.accept(frame(18)); h.flush();
      expect(h.canvas.__geosolvePresentedFrame).toEqual(frame(18));
      expect(h.backend.render).toHaveBeenCalledTimes(3);
      await vi.advanceTimersByTimeAsync(32); h.flush();
      expect(h.backend.render).toHaveBeenCalledTimes(3);
    } finally { h.renderer.destroy(); vi.useRealTimers(); }
  });
  for (const timing of ["timer", "queued RAF", "late reply"] as const) it(`lets wheel input supersede hover before its ${timing} draws`, async () => {
    vi.useFakeTimers(); const h = harness();
    try {
      h.renderer.accept(frame()); await h.ready(); h.flush();
      if (timing !== "late reply") h.renderer.accept(frame(12), "hover");
      if (timing === "queued RAF") await vi.advanceTimersByTimeAsync(32);
      h.renderer.setInteractionActive(true, { supersedeHover: true });
      if (timing === "late reply") h.renderer.accept(frame(12), "hover");
      await vi.advanceTimersByTimeAsync(64); h.flush();
      expect(h.backend.render).toHaveBeenCalledTimes(1);
      expect(h.canvas.__geosolveRendererDiagnostics).toMatchObject({ hasPendingDraw: true, isSettled: false });
      h.renderer.accept(frame(14)); h.flush();
      expect(h.canvas.__geosolvePresentedFrame).toEqual(frame(14));
      expect(h.backend.render).toHaveBeenCalledTimes(2);
    } finally { h.renderer.destroy(); vi.useRealTimers(); }
  });
  it("releases retained hover when navigation ends without a changed frame and preserves immediate surface work", async () => {
    vi.useFakeTimers(); const h = harness();
    try {
      h.renderer.accept(frame()); await h.ready(); h.flush();
      h.renderer.setInteractionActive(true, { supersedeHover: true });
      h.renderer.accept(frame(12), "hover");
      await vi.advanceTimersByTimeAsync(64); h.flush();
      expect(h.backend.render).toHaveBeenCalledTimes(1);
      h.renderer.setInteractionActive(false); h.flush();
      expect(h.canvas.__geosolvePresentedFrame).toEqual(frame(12));
      h.renderer.accept(frame(14), "hover");
      h.renderer.resize({ width: 900, height: 600, pixelRatio: 1 });
      h.renderer.setInteractionActive(true, { supersedeHover: true }); h.flush();
      expect(h.backend.render).toHaveBeenLastCalledWith(frame(14), { width: 900, height: 600, pixelRatio: 1 }, "interactive");
      h.renderer.accept(frame(16), "hover");
      h.renderer.accept(frame(16)); h.flush();
      expect(h.canvas.__geosolvePresentedFrame).toEqual(frame(16));
    } finally { h.renderer.destroy(); vi.useRealTimers(); }
  });
  it("retires deferred hover on resize, context loss and disposal", async () => {
    vi.useFakeTimers(); const h = harness();
    try {
      h.renderer.accept(frame()); await h.ready(); h.flush();
      h.renderer.accept(frame(12), "hover");
      h.renderer.resize({ width: 900, height: 600, pixelRatio: 1 }); h.flush();
      expect(h.backend.render).toHaveBeenCalledTimes(2);
      h.renderer.accept(frame(14), "hover");
      h.canvas.dispatchEvent(new Event("webglcontextlost", { cancelable: true }));
      await vi.advanceTimersByTimeAsync(32); h.flush();
      expect(h.backend.render).toHaveBeenCalledTimes(2);
      h.canvas.dispatchEvent(new Event("webglcontextrestored")); h.flush();
      expect(h.canvas.__geosolvePresentedFrame).toEqual(frame(14));
      h.renderer.accept(frame(16), "hover"); h.renderer.destroy();
      await vi.advanceTimersByTimeAsync(2_000);
      expect(h.callbacks.size).toBe(0); expect(vi.getTimerCount()).toBe(0);
    } finally { h.renderer.destroy(); vi.useRealTimers(); }
  });
  it("keeps short pauses between four changing editors free of competing refinements", async () => {
    vi.useFakeTimers(); const editors = Array.from({ length: 4 }, harness);
    try {
      for (const h of editors) {
        h.renderer.accept(frame()); await h.ready(); h.flush();
        h.renderer.accept(frame(12)); h.flush();
      }
      for (let step = 0; step < 12; step++) {
        await vi.advanceTimersByTimeAsync(300);
        for (const h of editors) h.flush();
        const active = editors[step % editors.length];
        active.renderer.accept(frame(14 + step)); active.flush();
        for (const h of editors) {
          expect(vi.mocked(h.backend.render).mock.calls.slice(1).every((call) => call[2] === "interactive")).toBe(true);
        }
      }
      await vi.advanceTimersByTimeAsync(1_500);
      for (const h of editors) {
        h.flush();
        expect(vi.mocked(h.backend.render).mock.lastCall?.[2]).toBe("settled");
        expect(h.canvas.__geosolveRendererDiagnostics?.isSettled).toBe(true);
      }
    } finally { for (const h of editors) h.renderer.destroy(); vi.useRealTimers(); }
  });
  for (const pendingWork of ["none", "frame", "surface"] as const) it(`cancels a queued idle refinement while preserving ${pendingWork} work when input restarts`, async () => {
    vi.useFakeTimers(); const h = harness();
    try {
      h.renderer.setInteractionActive(true); h.renderer.accept(frame()); await h.ready(); h.flush();
      h.renderer.setInteractionActive(false); await vi.advanceTimersByTimeAsync(1_500);
      expect(h.callbacks.size).toBe(1); expect(h.canvas.__geosolveRendererDiagnostics?.hasPendingDraw).toBe(true);
      if (pendingWork === "frame") h.renderer.accept(frame(12));
      if (pendingWork === "surface") h.renderer.resize({ width: 900, height: 600, pixelRatio: 1 });
      h.renderer.setInteractionActive(true); h.flush();
      if (pendingWork === "none") {
        expect(h.backend.render).toHaveBeenCalledTimes(1);
        expect(h.canvas.__geosolveRendererDiagnostics).toMatchObject({ hasPendingDraw: false, isSettled: false, frameCount: 1 });
        h.renderer.accept(frame(14)); h.flush();
        expect(h.backend.render).toHaveBeenLastCalledWith(frame(14), { width: 1000, height: 700, pixelRatio: 1 }, "interactive");
      } else {
        expect(h.backend.render).toHaveBeenCalledTimes(2);
        expect(h.backend.render).toHaveBeenLastCalledWith(pendingWork === "frame" ? frame(12) : frame(),
          pendingWork === "surface" ? { width: 900, height: 600, pixelRatio: 1 } : { width: 1000, height: 700, pixelRatio: 1 }, "interactive");
      }
    } finally { h.renderer.destroy(); vi.useRealTimers(); }
  });
  it("keeps the first frame static and presents later unsolicited changes at native DPR before refining", async () => {
    vi.useFakeTimers(); const h = harness();
    try {
      h.renderer.accept(frame()); await h.ready(); h.flush();
      expect(h.backend.render).toHaveBeenLastCalledWith(frame(), { width: 1000, height: 700, pixelRatio: 1 }, "settled");
      expect(h.canvas.__geosolveRendererDiagnostics).toMatchObject({ isSettled: true, hasPendingDraw: false });
      await vi.advanceTimersByTimeAsync(2_000); h.renderer.accept(frame()); h.flush();
      expect(h.backend.render).toHaveBeenCalledTimes(1); expect(vi.getTimerCount()).toBe(0);
      h.renderer.accept(frame(12)); h.flush();
      expect(h.backend.render).toHaveBeenLastCalledWith(frame(12), { width: 1000, height: 700, pixelRatio: 1 }, "interactive");
      expect(h.canvas.__geosolveRendererDiagnostics).toMatchObject({ isSettled: false, hasPendingDraw: false });
      await vi.advanceTimersByTimeAsync(400); h.renderer.accept(frame(12));
      await vi.advanceTimersByTimeAsync(1_100); h.flush(); // Identical publications do not defer refinement.
      expect(h.backend.render).toHaveBeenLastCalledWith(frame(12), { width: 1000, height: 700, pixelRatio: 1 }, "settled");
      expect(h.canvas.__geosolveRendererDiagnostics).toMatchObject({ isSettled: true, lastFrameId: 2, frameCount: 3 });
      await vi.advanceTimersByTimeAsync(2_000); h.renderer.accept(frame(14)); h.flush();
      expect(h.backend.render).toHaveBeenLastCalledWith(frame(14), { width: 1000, height: 700, pixelRatio: 1 }, "interactive");
      await vi.advanceTimersByTimeAsync(1_500); h.flush();
      expect(h.canvas.__geosolveRendererDiagnostics).toMatchObject({ isSettled: true, frameCount: 5, lastFrameId: 3 });
    } finally { h.renderer.destroy(); vi.useRealTimers(); }
  });
  it("samples readonly quiescence separately from completed frame authority through async refinement and loss", async () => {
    vi.useFakeTimers(); const h = harness();
    try {
      expect(h.canvas.__geosolveRendererDiagnostics).toMatchObject({ isSettled: false, hasPendingDraw: true, frameCount: 0 });
      h.renderer.accept(frame()); await h.ready(); h.flush();
      const staticStatus = h.canvas.__geosolveRendererDiagnostics!;
      expect(staticStatus).toMatchObject({ isSettled: true, hasPendingDraw: false, frameCount: 1 });
      expect(Object.isFrozen(staticStatus)).toBe(true);
      expect(Reflect.set(staticStatus, "isSettled", false)).toBe(false);
      h.renderer.accept(frame(12));
      expect(h.canvas.__geosolveRendererDiagnostics).toMatchObject({ isSettled: false, hasPendingDraw: true, frameCount: 1 });
      let complete!: (stats: { resources: number; created: number; destroyed: number; updated: number }) => void;
      vi.mocked(h.backend.render).mockImplementationOnce(() => new Promise(resolve => { complete = resolve; })); h.flush();
      await vi.advanceTimersByTimeAsync(1_600);
      expect(h.canvas.__geosolveRendererDiagnostics).toMatchObject({ isSettled: false, hasPendingDraw: true, frameCount: 1 });
      complete({ resources: 1, created: 0, destroyed: 0, updated: 1 }); await Promise.resolve();
      expect(h.canvas.__geosolveRendererDiagnostics).toMatchObject({ isSettled: false, hasPendingDraw: false, frameCount: 2 });
      await vi.advanceTimersByTimeAsync(1_500);
      expect(h.canvas.__geosolveRendererDiagnostics).toMatchObject({ isSettled: false, hasPendingDraw: true, frameCount: 2 });
      vi.mocked(h.backend.render).mockImplementationOnce(() => new Promise(resolve => { complete = resolve; })); h.flush();
      expect(h.canvas.__geosolveRendererDiagnostics).toMatchObject({ isSettled: false, hasPendingDraw: true, frameCount: 2 });
      complete({ resources: 1, created: 0, destroyed: 0, updated: 0 }); await Promise.resolve();
      expect(h.canvas.__geosolveRendererDiagnostics).toMatchObject({ isSettled: true, hasPendingDraw: false, frameCount: 3 });
      h.canvas.dispatchEvent(new Event("webglcontextlost", { cancelable: true }));
      expect(h.canvas.__geosolveRendererDiagnostics).toMatchObject({ state: "lost", isSettled: false, hasPendingDraw: true, frameCount: 3 });
      h.canvas.dispatchEvent(new Event("webglcontextrestored")); h.flush();
      expect(h.canvas.__geosolveRendererDiagnostics).toMatchObject({ state: "ready", isSettled: true, hasPendingDraw: false, frameCount: 4 });
      expect(staticStatus).toMatchObject({ isSettled: true, hasPendingDraw: false, frameCount: 1 });
      h.renderer.destroy(); expect(h.canvas.__geosolveRendererDiagnostics).toBeUndefined();
    } finally { h.renderer.destroy(); vi.useRealTimers(); }
  });
  it("uses native resolution for input, holds it through queued draws, and validates the settled redraw", async () => {
    vi.useFakeTimers(); const h = harness();
    try {
      h.renderer.accept(frame()); await h.ready(); h.flush();
      h.renderer.setInteractionActive(true);
      // Activity alone must not insert an old-camera redraw ahead of input.
      expect(h.callbacks.size).toBe(0);
      h.renderer.accept(frame(12)); h.flush();
      expect(h.backend.render).toHaveBeenLastCalledWith(frame(12), { width: 1000, height: 700, pixelRatio: 1 }, "interactive");
      await vi.advanceTimersByTimeAsync(2_000); h.flush();
      expect(h.backend.render).toHaveBeenCalledTimes(2);
      h.renderer.setInteractionActive(false);
      await vi.advanceTimersByTimeAsync(400);
      h.renderer.accept(frame(14)); h.flush();
      await vi.advanceTimersByTimeAsync(1_499); h.flush();
      expect(h.backend.render).toHaveBeenCalledTimes(3);
      let complete!: (stats: { resources: number; created: number; destroyed: number; updated: number }) => void;
      vi.mocked(h.backend.render).mockImplementationOnce(() => new Promise(resolve => { complete = resolve; }));
      await vi.advanceTimersByTimeAsync(1); h.flush();
      expect(h.backend.render).toHaveBeenLastCalledWith(frame(14), { width: 1000, height: 700, pixelRatio: 1 }, "settled");
      expect(h.canvas.__geosolveRendererDiagnostics).toMatchObject({ frameCount: 3, lastFrameId: 3 });
      complete({ resources: 1, created: 0, destroyed: 0, updated: 0 }); await Promise.resolve();
      expect(h.canvas.__geosolveRendererDiagnostics).toMatchObject({ frameCount: 4, lastFrameId: 3 });
      expect(h.canvas.__geosolvePresentedFrame).toEqual(frame(14));
      expect(vi.getTimerCount()).toBe(0);
    } finally { h.renderer.destroy(); vi.useRealTimers(); }
  });
  it("waits for the final interactive GPU draw before starting the idle interval", async () => {
    vi.useFakeTimers(); const h = harness();
    try {
      await h.ready(); h.renderer.setInteractionActive(true); h.renderer.accept(frame());
      let complete!: (stats: { resources: number; created: number; destroyed: number; updated: number }) => void;
      vi.mocked(h.backend.render).mockImplementationOnce(() => new Promise(resolve => { complete = resolve; })); h.flush();
      h.renderer.setInteractionActive(false); await vi.advanceTimersByTimeAsync(3_000); h.flush();
      expect(h.backend.render).toHaveBeenCalledTimes(1);
      complete({ resources: 1, created: 1, destroyed: 0, updated: 1 }); await Promise.resolve();
      expect(h.canvas.__geosolvePresentedFrame).toEqual(frame());
      await vi.advanceTimersByTimeAsync(1_499); h.flush(); expect(h.backend.render).toHaveBeenCalledTimes(1);
      await vi.advanceTimersByTimeAsync(1); h.flush();
      expect(h.backend.render).toHaveBeenLastCalledWith(frame(), { width: 1000, height: 700, pixelRatio: 1 }, "settled");
    } finally { h.renderer.destroy(); vi.useRealTimers(); }
  });
  it("keeps submitted surface and quality exact across DPR changes and context loss", async () => {
    vi.useFakeTimers(); const h = harness();
    try {
      await h.ready(); h.renderer.setInteractionActive(true); h.renderer.accept(frame());
      let complete!: (stats: { resources: number; created: number; destroyed: number; updated: number; rasterResolution: number }) => void;
      vi.mocked(h.backend.render).mockImplementationOnce(() => new Promise(resolve => { complete = resolve; })); h.flush();
      h.renderer.resize({ width: 900, height: 600, pixelRatio: 3 });
      let completeResize!: typeof complete;
      vi.mocked(h.backend.render).mockImplementationOnce(() => new Promise(resolve => { completeResize = resolve; }));
      complete({ resources: 1, created: 1, destroyed: 0, updated: 1, rasterResolution: 1 }); await Promise.resolve();
      expect(h.backend.render).toHaveBeenLastCalledWith(frame(), { width: 900, height: 600, pixelRatio: 3 }, "interactive");
      expect(h.canvas.__geosolveRendererDiagnostics).toMatchObject({ width: 1000, height: 700, pixelRatio: 1, rasterResolution: 1, frameCount: 1 });
      completeResize({ resources: 1, created: 0, destroyed: 0, updated: 1, rasterResolution: 3 }); await Promise.resolve();
      expect(h.canvas.__geosolveRendererDiagnostics).toMatchObject({ pixelRatio: 3, rasterResolution: 3, lastFrameId: 1 });
      h.renderer.setInteractionActive(false); await vi.advanceTimersByTimeAsync(1_500); h.flush();
      expect(h.backend.render).toHaveBeenCalledTimes(2); // Native DPR already meets static quality.
      h.canvas.dispatchEvent(new Event("webglcontextlost", { cancelable: true }));
      h.renderer.resize({ width: 900, height: 600, pixelRatio: 1 }); h.renderer.setInteractionActive(true);
      h.canvas.dispatchEvent(new Event("webglcontextrestored")); h.flush();
      expect(h.backend.render).toHaveBeenLastCalledWith(frame(), { width: 900, height: 600, pixelRatio: 1 }, "interactive");
      h.renderer.setInteractionActive(false); h.renderer.destroy();
      await vi.advanceTimersByTimeAsync(3_000); expect(h.callbacks.size).toBe(0); expect(vi.getTimerCount()).toBe(0);
    } finally { h.renderer.destroy(); vi.useRealTimers(); }
  });
  it("publishes the submitted settled draw without relabeling input queued behind it", async () => {
    vi.useFakeTimers(); const h = harness();
    try {
      await h.ready(); h.renderer.setInteractionActive(true); h.renderer.accept(frame()); h.flush();
      h.renderer.setInteractionActive(false);
      let completeStatic!: (stats: { resources: number; created: number; destroyed: number; updated: number; rasterResolution: number }) => void;
      vi.mocked(h.backend.render).mockImplementationOnce(() => new Promise(resolve => { completeStatic = resolve; }));
      await vi.advanceTimersByTimeAsync(1_500); h.flush();
      h.renderer.setInteractionActive(true); h.renderer.accept(frame(14));
      let completeInput!: typeof completeStatic;
      vi.mocked(h.backend.render).mockImplementationOnce(() => new Promise(resolve => { completeInput = resolve; }));
      completeStatic({ resources: 1, created: 0, destroyed: 0, updated: 0, rasterResolution: 2 }); await Promise.resolve();
      expect(h.canvas.__geosolvePresentedFrame).toEqual(frame());
      expect(h.canvas.__geosolveRendererDiagnostics).toMatchObject({ lastFrameId: 1, rasterResolution: 2 });
      expect(h.backend.render).toHaveBeenLastCalledWith(frame(14), { width: 1000, height: 700, pixelRatio: 1 }, "interactive");
      completeInput({ resources: 1, created: 0, destroyed: 0, updated: 1, rasterResolution: 1 }); await Promise.resolve();
      expect(h.canvas.__geosolvePresentedFrame).toEqual(frame(14));
      expect(h.canvas.__geosolveRendererDiagnostics).toMatchObject({ lastFrameId: 2, rasterResolution: 1 });
      expect(vi.getTimerCount()).toBe(0);
    } finally { h.renderer.destroy(); vi.useRealTimers(); }
  });
});

describe("on-demand accepted-frame presentation", () => {
  it("keeps synchronous reentrant input coalesced behind an animation frame",async()=>{
    const h=harness();h.renderer.accept(frame());await h.ready();
    vi.mocked(h.backend.render).mockImplementationOnce(()=>{h.renderer.accept(frame(14));h.renderer.accept(frame(18));return {resources:1,created:1,destroyed:0,updated:1};});
    h.flush();expect(h.backend.render).toHaveBeenCalledTimes(1);expect(h.canvas.__geosolvePresentedFrame).toEqual(frame());expect(h.callbacks.size).toBe(1);
    h.flush();expect(h.backend.render).toHaveBeenCalledTimes(2);expect(h.canvas.__geosolvePresentedFrame).toEqual(frame(18));h.renderer.destroy();
  });
  it("retains the preceding validated frame when its immediate queued successor fails",async()=>{
    const h=harness();h.renderer.accept(frame());await h.ready();let finish!:(stats:{resources:number;created:number;destroyed:number;updated:number})=>void;
    vi.mocked(h.backend.render).mockImplementationOnce(()=>new Promise(resolve=>{finish=resolve;}));h.flush();h.renderer.accept(frame(18));
    vi.mocked(h.backend.render).mockImplementationOnce(()=>{throw Error("successor failed");});
    finish({resources:1,created:1,destroyed:0,updated:1});await Promise.resolve();
    expect(h.canvas.__geosolvePresentedFrame).toEqual(frame());expect(h.canvas.__geosolveRendererDiagnostics).toMatchObject({state:"unavailable",frameCount:1,error:"successor failed"});
    h.renderer.accept(frame(20));h.flush();expect(h.backend.render).toHaveBeenCalledTimes(2);expect(h.callbacks.size).toBe(0);h.renderer.destroy();
  });
  it("waits for GPU validation before publishing a frame and coalesces input received while it is pending", async () => {
    const h = harness(); h.renderer.accept(frame()); await h.ready(); h.flush();
    let complete!: (stats: {resources:number;created:number;destroyed:number;updated:number}) => void;
    const pending = new Promise<{resources:number;created:number;destroyed:number;updated:number}>(resolve => { complete = resolve; });
    vi.mocked(h.backend.render).mockImplementationOnce(() => pending);
    h.renderer.accept(frame(14)); h.flush();
    expect(h.canvas.__geosolvePresentedFrame).toEqual(frame());
    h.renderer.accept(frame(16)); h.renderer.accept(frame(18)); h.flush();
    expect(h.backend.render).toHaveBeenCalledTimes(2);
    let completeNewest!: (stats: {resources:number;created:number;destroyed:number;updated:number}) => void;
    vi.mocked(h.backend.render).mockImplementationOnce(() => new Promise(resolve => { completeNewest = resolve; }));
    complete({resources:1,created:1,destroyed:0,updated:2}); await Promise.resolve();
    // The next GPU submission starts immediately after validation, with no RAF
    // between them; its public witness must still await its own completion.
    expect(h.callbacks.size).toBe(0);
    expect(h.backend.render).toHaveBeenLastCalledWith(frame(18), {width:1000,height:700,pixelRatio:1}, "interactive");
    expect(h.canvas.__geosolvePresentedFrame).toEqual(frame(14));
    completeNewest({resources:1,created:1,destroyed:0,updated:3}); await Promise.resolve();
    expect(h.canvas.__geosolvePresentedFrame).toEqual(frame(18));
    expect(h.backend.render).toHaveBeenCalledTimes(3); h.renderer.destroy();
  });
  it("retains the submitted surface while newer resize and hidden input wait for completion", async () => {
    const h=harness();h.renderer.accept(frame());await h.ready();h.flush();
    let resolve!:(value:{resources:number;created:number;destroyed:number;updated:number})=>void;
    vi.mocked(h.backend.render).mockImplementationOnce(()=>new Promise(done=>{resolve=done;}));
    h.renderer.accept(frame(14));h.flush();
    h.renderer.resize({width:1200,height:800,pixelRatio:2});h.renderer.accept(frame(18));
    h.renderer.resize({width:0,height:0,pixelRatio:2});h.flush();
    expect(h.backend.render).toHaveBeenCalledTimes(2);
    resolve({resources:1,created:1,destroyed:0,updated:2});await Promise.resolve();
    expect(h.canvas.__geosolvePresentedFrame).toEqual(frame(14));
    expect(h.canvas.__geosolveRendererDiagnostics).toMatchObject({width:1000,height:700,pixelRatio:1});
    expect(h.callbacks.size).toBe(0);
    h.renderer.resize({width:1200,height:800,pixelRatio:2});h.flush();
    expect(h.backend.render).toHaveBeenLastCalledWith(frame(18),{width:1200,height:800,pixelRatio:2}, "interactive");
    expect(h.canvas.__geosolvePresentedFrame).toEqual(frame(18));h.renderer.destroy();
  });
  for(const rejected of [false,true])it(`ignores a ${rejected?"rejected":"successful"} old-context completion after restored drawing starts`,async()=>{
    const h=harness();h.renderer.accept(frame());await h.ready();h.flush();
    const stats={resources:1,created:1,destroyed:0,updated:2};
    let oldResolve!:(value:typeof stats)=>void,oldReject!:(error:Error)=>void,newResolve!:(value:typeof stats)=>void;
    vi.mocked(h.backend.render).mockImplementationOnce(()=>new Promise((yes,no)=>{oldResolve=yes;oldReject=no;}));
    h.renderer.accept(frame(14));h.flush();h.canvas.dispatchEvent(new Event("webglcontextlost",{cancelable:true}));
    h.renderer.accept(frame(18));h.canvas.dispatchEvent(new Event("webglcontextrestored"));
    vi.mocked(h.backend.render).mockImplementationOnce(()=>new Promise(yes=>{newResolve=yes;}));h.flush();
    if(rejected)oldReject(new Error("stale GPU failure"));else oldResolve(stats);await Promise.resolve();
    h.renderer.accept(frame(20));h.flush();
    expect(h.backend.render).toHaveBeenCalledTimes(3);expect(h.canvas.__geosolvePresentedFrame).toEqual(frame());
    expect(h.canvas.__geosolveRendererDiagnostics?.state).toBe("initializing");
    newResolve(stats);await Promise.resolve();expect(h.canvas.__geosolvePresentedFrame).toEqual(frame(20));
    expect(h.callbacks.size).toBe(0);h.flush();expect(h.canvas.__geosolvePresentedFrame).toEqual(frame(20));h.renderer.destroy();
  });
  it("latches GPU validation failure until restoration and never publishes a rejected draw",async()=>{
    const h=harness();h.renderer.accept(frame());await h.ready();h.flush();
    let reject!:(error:Error)=>void;
    vi.mocked(h.backend.render).mockImplementationOnce(()=>new Promise((_,no)=>{reject=no;}));
    h.renderer.accept(frame(14));h.flush();reject(new Error("GPU timeout"));await Promise.resolve();
    h.renderer.accept(frame(18));h.renderer.resize({width:1200,height:800,pixelRatio:2});h.flush();
    expect(h.backend.render).toHaveBeenCalledTimes(2);expect(h.callbacks.size).toBe(0);
    expect(h.canvas.__geosolvePresentedFrame).toEqual(frame());expect(h.canvas.__geosolveRendererDiagnostics).toMatchObject({state:"unavailable",frameCount:1,error:"GPU timeout"});
    h.canvas.dispatchEvent(new Event("webglcontextlost",{cancelable:true}));h.canvas.dispatchEvent(new Event("webglcontextrestored"));h.flush();
    expect(h.canvas.__geosolvePresentedFrame).toEqual(frame(18));h.renderer.destroy();
  });
  for(const rejected of [false,true])it(`does not revive destroyed presentation after ${rejected?"rejection":"completion"}`,async()=>{
    const h=harness();await h.ready();let resolve!:(value:{resources:number;created:number;destroyed:number;updated:number})=>void,reject!:(error:Error)=>void;
    vi.mocked(h.backend.render).mockImplementationOnce(()=>new Promise((yes,no)=>{resolve=yes;reject=no;}));h.renderer.accept(frame());h.flush();h.renderer.destroy();
    if(rejected)reject(new Error("disposed"));else resolve({resources:1,created:1,destroyed:0,updated:1});await Promise.resolve();
    expect(h.canvas.__geosolvePresentedFrame).toBeUndefined();expect(h.canvas.__geosolveRendererDiagnostics).toBeUndefined();expect(h.callbacks.size).toBe(0);expect(h.canvas.dataset.presentedFrame).toBeUndefined();
  });
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
    expect(h.backend.render).toHaveBeenLastCalledWith(frame(), { width: 1000, height: 700, pixelRatio: 2 }, "settled");
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
