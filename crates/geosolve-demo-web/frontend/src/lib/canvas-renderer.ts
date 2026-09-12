// SPDX-License-Identifier: GPL-3.0-or-later
import { immutableDrawFrame, type DrawFrame } from "./canvas-scene";
import type { BackendStats, CanvasBackend, CanvasRenderQuality, CanvasSurface } from "./canvas-renderer-pixi";

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
  /** Includes queued input/surface changes, idle refinement and submitted GPU work. */
  readonly hasPendingDraw: boolean;
  /** No active input, pending draw or idle refinement remains. */
  readonly isSettled: boolean;
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
  accept(frame: DrawFrame, priority?: "immediate" | "hover"): void;
  resize(surface: CanvasSurface): void;
  /** Hold native raster quality while input is queued or a pointer is captured. */
  setInteractionActive(active: boolean, options?: { supersedeHover?: boolean }): void;
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
  let acceptedPriority: "immediate" | "hover" = "immediate";
  let hoverSuppressed = false;
  let presented: DrawFrame | null = null;
  let pending: number | null = null;
  let disposed = false;
  let lost = false;
  let failed = false;
  let dirty = false;
  let refinementPending = false;
  let surface: CanvasSurface = { width: 0, height: 0, pixelRatio: 1 };
  let surfaceVersion = 0;
  let contextEpoch = 0;
  let interactionActive = false;
  let quality: CanvasRenderQuality = "settled";
  let presentedResolution: number | null = null;
  let settleTimer: ReturnType<typeof setTimeout> | null = null;
  let hoverTimer: ReturnType<typeof setTimeout> | null = null;
  type Submission = { frame: DrawFrame; frameId: number; surface: CanvasSurface; surfaceVersion: number; quality: CanvasRenderQuality; contextEpoch: number; began: number };
  let inFlight: Submission | null = null;
  let diagnostics: Readonly<Omit<RendererDiagnostics, "hasPendingDraw" | "isSettled">> = Object.freeze({ ...surface, backend: "webgl2", state: "initializing", frameCount: 0, lastFrameId: null,
    lastRenderMilliseconds: 0, totalRenderMilliseconds: 0, hardware: null, error: null, resources: 0, created: 0, destroyed: 0, updated: 0 });
  Object.defineProperties(canvas, {
    __geosolvePresentedFrame: { configurable: true, get: () => presented },
    __geosolveRendererDiagnostics: { configurable: true, get: () => {
      // Completion fields remain from the validated submission. Quiescence is
      // sampled now so a screenshot cannot mistake an older ready frame for
      // an idle GPU while its successor is queued or already drawing.
      const hasPendingDraw = dirty || refinementPending || pending !== null || hoverTimer !== null || inFlight !== null;
      return Object.freeze({ ...diagnostics, hasPendingDraw,
        isSettled: diagnostics.state === "ready" && !hasPendingDraw && !interactionActive && settleTimer === null && quality === "settled" });
    } },
  });
  canvas.dataset.renderer = "webgl2";
  function state(next: RendererState, error: string | null = null) {
    diagnostics = Object.freeze({ ...diagnostics, state: next, error });
    canvas.dataset.renderState = next;
    options.onState?.(next);
  }
  function canSubmit() {
    return !(disposed || lost || failed || !backend || !accepted || !(dirty || refinementPending) || pending !== null || hoverTimer !== null || hoverSuppressed && acceptedPriority === "hover" || inFlight || surface.width <= 0 || surface.height <= 0);
  }
  function schedule() {
    if (!canSubmit()) return;
    pending = requestFrame(() => { pending = null; submit(); });
  }
  function cancelSettle() {
    if (settleTimer !== null) clearTimeout(settleTimer);
    settleTimer = null;
  }
  function cancelHover() {
    if (hoverTimer !== null) clearTimeout(hoverTimer);
    hoverTimer = null;
  }
  function useInteractiveQuality() {
    cancelSettle(); quality = "interactive"; refinementPending = false;
    // A refinement RAF may have been queued just before new input arrived.
    // Cancel only that quality-only work; retain all semantic/surface work and
    // never invalidate an already submitted GPU draw.
    if (!dirty && pending !== null) { cancelFrame(pending); pending = null; }
  }
  function scheduleSettle() {
    cancelSettle();
    if (disposed || interactionActive || quality === "settled") return;
    // Short pauses between editor updates are still interaction. Refining all
    // canvases after only half a second can saturate a shared software GPU just
    // as the next editor receives input. Text textures remain sharp throughout.
    settleTimer = setTimeout(() => {
      settleTimer = null;
      // A slow submitted draw still belongs to this interaction. Its completed
      // GPU validation starts a fresh quiet interval before static restoration.
      if (inFlight) return;
      quality = "settled";
      if (presentedResolution !== null && presentedResolution !== Math.max(2, surface.pixelRatio)) {
        refinementPending = true; schedule();
      }
    }, 1_500);
  }
  function submit() {
    if (!canSubmit() || !backend || !accepted) return;
    const submission: Submission = { frame: accepted, frameId: acceptedId, surface: { ...surface }, surfaceVersion, quality, contextEpoch, began: now() };
    inFlight = submission; refinementPending = false;
    const current = () => !disposed && !lost && inFlight === submission && contextEpoch === submission.contextEpoch;
    const complete = (stats: BackendStats, asynchronous = false) => {
      if (!current()) return;
      inFlight = null;
      // Publish the exact validated draw, including its submitted camera and
      // surface. Newer input stays queued; it cannot relabel this older draw.
      presented = submission.frame;
      presentedResolution = submission.quality === "interactive" ? submission.surface.pixelRatio : Math.max(2, submission.surface.pixelRatio);
      dirty = acceptedId !== submission.frameId || surfaceVersion !== submission.surfaceVersion;
      const elapsed = now() - submission.began;
      diagnostics = Object.freeze({ ...diagnostics, ...submission.surface, ...stats, frameCount: diagnostics.frameCount + 1, lastFrameId: submission.frameId,
        hardware: backend!.hardware, lastRenderMilliseconds: elapsed, totalRenderMilliseconds: diagnostics.totalRenderMilliseconds + elapsed });
      canvas.dataset.presentedFrame = String(submission.frameId);
      state("ready");
      scheduleSettle();
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
      const result = backend.render(submission.frame, submission.surface, submission.quality);
      if (result instanceof Promise) void result.then((stats) => complete(stats, true), fail); else complete(result);
    } catch (error) { fail(error); }
  }
  function contextLost(event: Event) {
    event.preventDefault();
    if (disposed) return;
    lost = true; dirty = true; refinementPending = false; contextEpoch++; inFlight = null;
    cancelHover();
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
    accept(frame, priority = "immediate") {
      if (disposed) return;
      const copy = immutableDrawFrame(frame);
      if (sameFrameValue(copy, accepted)) {
        // A clamped/no-op wheel can return the same scene as deferred hover.
        // Its immediate response still releases that retained frame for paint.
        if (hoverSuppressed && priority === "immediate" && acceptedPriority === "hover") {
          acceptedPriority = "immediate"; cancelHover(); schedule();
        }
        return;
      }
      // Shared cursors, peer previews and delayed authoring responses animate
      // without local DOM input. Every changed frame after the first receives
      // the same native-DPR presentation, then refines when updates stop.
      if (accepted !== null) useInteractiveQuality();
      accepted = copy; acceptedId++; acceptedPriority = priority; dirty = true; scheduleSettle();
      if (priority === "hover" && !hoverSuppressed && !inFlight && pending === null) {
        // A following wheel/click often supersedes idle hover. Coalesce paint
        // for at most two display intervals, without delaying native input or
        // extending the window on every move. Actual editing stays immediate.
        hoverTimer ??= setTimeout(() => { hoverTimer = null; schedule(); }, 32);
      } else {
        cancelHover(); schedule();
      }
    },
    resize(next) {
      if (disposed) return;
      if (![next.width, next.height].every((value) => Number.isFinite(value) && value >= 0)
        || !Number.isFinite(next.pixelRatio) || next.pixelRatio <= 0) return;
      if (next.width === surface.width && next.height === surface.height && next.pixelRatio === surface.pixelRatio) return;
      surface = { ...next }; surfaceVersion++; dirty = true; acceptedPriority = "immediate";
      cancelHover();
      if ((next.width === 0 || next.height === 0) && pending !== null) { cancelFrame(pending); pending = null; }
      scheduleSettle(); schedule();
    },
    setInteractionActive(active, options) {
      if (disposed) return;
      interactionActive = active;
      if (active) {
        useInteractiveQuality();
        if (options?.supersedeHover) {
          // Wheel input is already queued, but its native reply may arrive
          // after a hover timer/RAF. Hold only unsubmitted hover until that
          // reply (or queue completion), keeping old-camera work out of its way.
          hoverSuppressed = true;
          if (acceptedPriority === "hover") {
            cancelHover();
            if (pending !== null) { cancelFrame(pending); pending = null; }
          }
        }
        // Wait for the input's accepted frame; a quality-only redraw here
        // would put old-camera GPU work ahead of the response being awaited.
      } else { hoverSuppressed = false; scheduleSettle(); schedule(); }
    },
    destroy() {
      if (disposed) return;
      disposed = true; contextEpoch++; inFlight = null;
      cancelSettle(); cancelHover();
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
