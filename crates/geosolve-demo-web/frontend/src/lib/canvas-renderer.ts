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

/** Exact equality with an identity fast path and early exit, without serializing a whole frame. */
function sameFrameValue(a: unknown, b: unknown): boolean {
  if (a === b) return true;
  if (a === null || b === null || typeof a !== "object" || typeof b !== "object") return false;
  if (Array.isArray(a)) return Array.isArray(b) && a.length === b.length && a.every((value, index) => sameFrameValue(value, b[index]));
  if (Array.isArray(b)) return false;
  const left = a as Record<string, unknown>; const right = b as Record<string, unknown>;
  const keys = Object.keys(left);
  return keys.length === Object.keys(right).length && keys.every((key) => Object.hasOwn(right, key) && sameFrameValue(left[key], right[key]));
}

/** Owns only presentation. The readonly observers report completed draws, never pending snapshots. */
export function createCanvasRenderer(canvas: HTMLCanvasElement, options: RendererOptions = {}): CanvasRenderer {
  const requestFrame = options.requestFrame ?? requestAnimationFrame;
  const cancelFrame = options.cancelFrame ?? cancelAnimationFrame;
  const now = options.now ?? (() => performance.now());
  let backend: CanvasBackend | null = null;
  let accepted: DrawFrame | null = null;
  let acceptedId = 0;
  let presented: DrawFrame | null = null;
  let pending: number | null = null;
  let disposed = false;
  let lost = false;
  let failed = false;
  let dirty = false;
  let surface: CanvasSurface = { width: 0, height: 0, pixelRatio: 1 };
  let surfaceVersion = 0;
  let contextEpoch = 0;
  type Submission = { frame: DrawFrame; frameId: number; surface: CanvasSurface; surfaceVersion: number; contextEpoch: number; began: number };
  let inFlight: Submission | null = null;
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
  function canSubmit() {
    return !(disposed || lost || failed || !backend || !accepted || !dirty || pending !== null || inFlight || surface.width <= 0 || surface.height <= 0);
  }
  function schedule() {
    if (!canSubmit()) return;
    pending = requestFrame(() => { pending = null; submit(); });
  }
  function submit() {
    if (!canSubmit() || !backend || !accepted) return;
    const submission: Submission = { frame: accepted, frameId: acceptedId, surface: { ...surface }, surfaceVersion, contextEpoch, began: now() };
    inFlight = submission;
    const current = () => !disposed && !lost && inFlight === submission && contextEpoch === submission.contextEpoch;
    const complete = (stats: BackendStats, asynchronous = false) => {
      if (!current()) return;
      inFlight = null;
      // Publish the exact validated draw, including its submitted camera and
      // surface. Newer input stays queued; it cannot relabel this older draw.
      presented = submission.frame;
      dirty = acceptedId !== submission.frameId || surfaceVersion !== submission.surfaceVersion;
      const elapsed = now() - submission.began;
      diagnostics = Object.freeze({ ...diagnostics, ...submission.surface, ...stats, frameCount: diagnostics.frameCount + 1, lastFrameId: submission.frameId,
        hardware: backend!.hardware, lastRenderMilliseconds: elapsed, totalRenderMilliseconds: diagnostics.totalRenderMilliseconds + elapsed });
      canvas.dataset.presentedFrame = String(submission.frameId);
      state("ready");
      // An asynchronous validation already yields to the browser. Submit the
      // newest queued frame now, without adding another animation-frame delay.
      // Initial and synchronous draws retain RAF coalescing.
      if (asynchronous) submit(); else schedule();
    };
    const fail = (error: unknown) => {
      if (!current()) return;
      inFlight = null; failed = true;
      state("unavailable", error instanceof Error ? error.message : String(error));
    };
    try {
      const result = backend.render(submission.frame, submission.surface);
      if (result instanceof Promise) void result.then((stats) => complete(stats, true), fail); else complete(result);
    } catch (error) { fail(error); }
  }
  function contextLost(event: Event) {
    event.preventDefault();
    if (disposed) return;
    lost = true; dirty = true; contextEpoch++; inFlight = null;
    if (pending !== null) { cancelFrame(pending); pending = null; }
    state("lost");
  }
  function contextRestored() {
    if (disposed) return;
    // Pixi's context-restored listener rebuilds GPU systems in the same event dispatch.
    // The next RAF runs after every restoration listener and presents the newest retained input.
    lost = false; failed = false; dirty = true; state("initializing"); schedule();
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
      if (sameFrameValue(copy, accepted)) return;
      accepted = copy; acceptedId++; dirty = true; schedule();
    },
    resize(next) {
      if (disposed) return;
      if (![next.width, next.height].every((value) => Number.isFinite(value) && value >= 0)
        || !Number.isFinite(next.pixelRatio) || next.pixelRatio <= 0) return;
      if (next.width === surface.width && next.height === surface.height && next.pixelRatio === surface.pixelRatio) return;
      surface = { ...next }; surfaceVersion++; dirty = true;
      if ((next.width === 0 || next.height === 0) && pending !== null) { cancelFrame(pending); pending = null; }
      schedule();
    },
    destroy() {
      if (disposed) return;
      disposed = true; contextEpoch++; inFlight = null;
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
