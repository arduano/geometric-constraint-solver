// SPDX-License-Identifier: GPL-3.0-or-later
import { immutableDrawFrame, type DrawFrame } from "./canvas-scene";
import type { BackendStats, CanvasBackend, CanvasSurface } from "./canvas-renderer-pixi";

export type RendererState = "initializing" | "ready" | "lost" | "unavailable";
export interface RendererDiagnostics extends BackendStats, CanvasSurface {
  readonly backend: "webgl2";
  readonly state: RendererState;
  readonly frameCount: number;
  readonly lastFrameId: number | null;
  readonly lastRenderMilliseconds: number;
  readonly totalRenderMilliseconds: number;
  readonly hardware: Readonly<Record<string, string>> | null;
  readonly error: string | null;
}
declare global {
  interface HTMLCanvasElement {
    readonly __geosolvePresentedFrame?: DrawFrame | null;
    readonly __geosolveRendererDiagnostics?: Readonly<RendererDiagnostics>;
  }
}
export interface RendererOptions {
  createBackend?: (canvas: HTMLCanvasElement) => Promise<CanvasBackend>;
  requestFrame?: (callback: FrameRequestCallback) => number;
  cancelFrame?: (handle: number) => void;
  now?: () => number;
  onState?: (state: RendererState) => void;
}
export interface CanvasRenderer {
  accept(frame: DrawFrame): void;
  resize(surface: CanvasSurface): void;
  destroy(): void;
}

/** Owns only presentation. The readonly observers report completed draws, never pending snapshots. */
export function createCanvasRenderer(canvas: HTMLCanvasElement, options: RendererOptions = {}): CanvasRenderer {
  const requestFrame = options.requestFrame ?? requestAnimationFrame;
  const cancelFrame = options.cancelFrame ?? cancelAnimationFrame;
  const now = options.now ?? (() => performance.now());
  let backend: CanvasBackend | null = null;
  let accepted: DrawFrame | null = null;
  let acceptedIdentity = "";
  let acceptedId = 0;
  let presented: DrawFrame | null = null;
  let pending: number | null = null;
  let disposed = false;
  let lost = false;
  let dirty = false;
  let surface: CanvasSurface = { width: 0, height: 0, pixelRatio: 1 };
  let diagnostics: Readonly<RendererDiagnostics> = Object.freeze({ ...surface, backend: "webgl2", state: "initializing", frameCount: 0, lastFrameId: null,
    lastRenderMilliseconds: 0, totalRenderMilliseconds: 0, hardware: null, error: null, resources: 0, created: 0, destroyed: 0, updated: 0 });
  Object.defineProperties(canvas, {
    __geosolvePresentedFrame: { configurable: true, get: () => presented },
    __geosolveRendererDiagnostics: { configurable: true, get: () => diagnostics },
  });
  canvas.dataset.renderer = "webgl2";
  function state(next: RendererState, error: string | null = null) {
    diagnostics = Object.freeze({ ...diagnostics, state: next, error });
    canvas.dataset.renderState = next;
    options.onState?.(next);
  }
  function schedule() {
    if (disposed || lost || !backend || !accepted || !dirty || pending !== null || surface.width <= 0 || surface.height <= 0) return;
    pending = requestFrame(() => {
      pending = null;
      if (disposed || lost || !backend || !accepted) return;
      const input = accepted; const frameId = acceptedId;
      const began = now();
      try {
        const stats = backend.render(input, surface);
        if (disposed || lost) return;
        const elapsed = now() - began;
        // Commit evidence only after the GPU renderer completed its submission successfully.
        presented = input;
        dirty = false;
        diagnostics = Object.freeze({ ...diagnostics, ...surface, ...stats, frameCount: diagnostics.frameCount + 1, lastFrameId: frameId,
          hardware: backend.hardware, lastRenderMilliseconds: elapsed, totalRenderMilliseconds: diagnostics.totalRenderMilliseconds + elapsed });
        canvas.dataset.presentedFrame = String(frameId);
        state("ready");
      } catch (error) {
        state(lost ? "lost" : "unavailable", error instanceof Error ? error.message : String(error));
      }
    });
  }
  function contextLost(event: Event) {
    event.preventDefault();
    if (disposed) return;
    lost = true; dirty = true;
    if (pending !== null) { cancelFrame(pending); pending = null; }
    state("lost");
  }
  function contextRestored() {
    if (disposed) return;
    // Pixi's context-restored listener rebuilds GPU systems in the same event dispatch.
    // The next RAF runs after every restoration listener and presents the newest retained input.
    lost = false; dirty = true; state("initializing"); schedule();
  }
  canvas.addEventListener("webglcontextlost", contextLost);
  canvas.addEventListener("webglcontextrestored", contextRestored);
  state("initializing");
  const factory = options.createBackend ?? (async (element) => {
    if (document.fonts) await document.fonts.ready;
    const { createPixiBackend } = await import("./canvas-renderer-pixi");
    // React StrictMode can retire an effect while this import is pending. Do not
    // initialize then destroy an obsolete renderer on the replacement effect's canvas.
    if (disposed) return null;
    return createPixiBackend(element);
  });
  void factory(canvas).then((created) => {
    if (!created) return;
    if (disposed) { created.destroy(); return; }
    backend = created;
    diagnostics = Object.freeze({ ...diagnostics, hardware: created.hardware });
    schedule();
  }).catch((error: unknown) => { if (!disposed) state(lost ? "lost" : "unavailable", error instanceof Error ? error.message : String(error)); });
  return {
    accept(frame) {
      if (disposed) return;
      const copy = immutableDrawFrame(frame);
      const identity = JSON.stringify(copy);
      if (identity === acceptedIdentity) return;
      accepted = copy; acceptedIdentity = identity; acceptedId++; dirty = true; schedule();
    },
    resize(next) {
      if (disposed) return;
      if (![next.width, next.height].every((value) => Number.isFinite(value) && value >= 0)
        || !Number.isFinite(next.pixelRatio) || next.pixelRatio <= 0) return;
      if (next.width === surface.width && next.height === surface.height && next.pixelRatio === surface.pixelRatio) return;
      surface = { ...next }; dirty = true;
      if ((next.width === 0 || next.height === 0) && pending !== null) { cancelFrame(pending); pending = null; }
      schedule();
    },
    destroy() {
      if (disposed) return;
      disposed = true;
      if (pending !== null) cancelFrame(pending);
      canvas.removeEventListener("webglcontextlost", contextLost);
      canvas.removeEventListener("webglcontextrestored", contextRestored);
      backend?.destroy(); backend = null; accepted = null; presented = null;
      delete canvas.dataset.presentedFrame;
      Reflect.deleteProperty(canvas, "__geosolvePresentedFrame");
      Reflect.deleteProperty(canvas, "__geosolveRendererDiagnostics");
    },
  };
}
