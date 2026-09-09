// SPDX-License-Identifier: GPL-3.0-or-later
import { useEffect, useMemo, useRef, useState } from "react";
import { createCanvasRenderer, type CanvasRenderer, type RendererState } from "../lib/canvas-renderer";
import { getCanvasSnapshotSequence, isCanvasOnlySnapshot, type PointerSample, type WheelSample, type WorkbenchAdapter, type WorkbenchSnapshot } from "../lib/adapter";
import { useWorkbenchBusy } from "../hooks/use-workbench-busy";

type CanvasOperation = () => Promise<WorkbenchSnapshot | null>;
type PendingInput = { kind: "move"; sample: PointerSample } | { kind: "wheel"; samples: WheelSample[] };
type QueuedOperation = { kind: "operation"; run: CanvasOperation; transient: boolean } | (PendingInput & { transient: true });

// Retain every semantic sample and every wheel anchor. Only idle Select hover
// and fixed-origin middle pan may replace adjacent movement waiting for the owner.
function sameMovement(left: PointerSample, right: PointerSample) {
  return left.pointerId === right.pointerId && left.buttons === right.buttons
    && left.modifiers.alt === right.modifiers.alt && left.modifiers.ctrl === right.modifiers.ctrl
    && left.modifiers.meta === right.modifiers.meta && left.modifiers.shift === right.modifiers.shift;
}

class CanvasInputQueue {
  private pending: PendingInput | null = null;
  private frame: number | null = null;
  private operations: QueuedOperation[] = [];
  private running = false;
  private disposed = false;
  private generation = 0;
  private dimensionTimer: ReturnType<typeof setTimeout> | null = null;
  private dimensionGeneration = 0;
  private dimensionPreview = false;
  private navigationTimer: ReturnType<typeof setTimeout> | null = null;
  private navigationGeneration = 0;

  constructor(
    private readonly adapter: WorkbenchAdapter,
    private readonly accept: (snapshot: WorkbenchSnapshot) => void,
    private readonly fail: (error: unknown) => void,
  ) {}

  pointer(sample: PointerSample, coalesce: boolean) {
    if (!coalesce) {
      this.flush();
      this.dispatch(() => this.adapter.pointer(sample));
      return;
    }
    if (this.pending?.kind !== "move" || !sameMovement(this.pending.sample, sample)) this.flush();
    this.pending = { kind: "move", sample };
    this.schedule();
  }

  wheel(sample: WheelSample) {
    this.clearDimensionHover();
    this.cancelNavigationTimer();
    if (this.pending?.kind !== "wheel") this.flush();
    // Match the bounded native batch. Never add deltas: clamp and changing
    // anchors make the individual ordered zoom operations significant.
    if (this.pending?.kind === "wheel" && this.pending.samples.length === 256) this.flush();
    if (this.pending?.kind === "wheel") this.pending.samples.push(sample);
    else this.pending = { kind: "wheel", samples: [sample] };
    this.schedule();
    const generation = this.navigationGeneration;
    this.navigationTimer = setTimeout(() => {
      this.navigationTimer = null;
      if (this.disposed || generation !== this.navigationGeneration) return;
      // Queue after every retained wheel anchor. A new wheel also invalidates
      // an old settle callback that is waiting behind asynchronous native work.
      this.flush();
      this.dispatch(() => generation === this.navigationGeneration
        ? this.adapter.dispatch({ version: 2, command: "dimensions.navigation.end" })
        : Promise.resolve(null));
    }, 180);
  }

  flush() {
    if (this.frame !== null) cancelAnimationFrame(this.frame);
    this.frame = null;
    const pending = this.pending;
    this.pending = null;
    if (!pending) return;
    if (pending.kind === "move" || this.adapter.wheelBatch) this.enqueueInput(pending);
    else for (const sample of pending.samples) this.dispatch(() => this.adapter.wheel(sample), true);
  }

  dispatch(run: CanvasOperation, transient = false) {
    if (this.disposed) return;
    this.operations.push({ kind: "operation", run, transient });
    this.drain();
  }

  discardHover() {
    this.operations = this.operations.filter((operation) => operation.kind !== "move" || operation.sample.buttons !== 0);
    if (this.pending?.kind !== "move" || this.pending.sample.buttons !== 0) return;
    if (this.frame !== null) cancelAnimationFrame(this.frame);
    this.frame = null;
    this.pending = null;
  }

  discardTransient() {
    this.cancelDimensionTimer();
    if (this.frame !== null) cancelAnimationFrame(this.frame);
    this.frame = null;
    this.pending = null;
    this.operations = this.operations.filter((operation) => !operation.transient);
  }

  activate() { this.disposed = false; }

  dimensionHover(x: number, y: number) {
    this.cancelDimensionTimer();
    const generation = this.dimensionGeneration;
    this.dimensionTimer = setTimeout(() => {
      this.dimensionTimer = null;
      if (this.disposed || generation !== this.dimensionGeneration) return;
      this.flush();
      this.dispatch(() => {
        if (generation !== this.dimensionGeneration) return Promise.resolve(null);
        this.dimensionPreview = true;
        return this.adapter.dispatch({ version: 2, command: "dimensions.hover", payload: { x, y } });
      }, true);
    }, 250);
  }

  cancelDimensionTimer() {
    if (this.dimensionTimer !== null) clearTimeout(this.dimensionTimer);
    this.dimensionTimer = null;
    this.dimensionGeneration += 1;
  }

  clearDimensionHover() {
    this.cancelDimensionTimer();
    if (!this.dimensionPreview || this.disposed) return;
    this.dimensionPreview = false;
    // A clear must survive transient-input pruning: otherwise the last preview
    // could remain painted after the pointer leaves or the window loses focus.
    this.dispatch(() => this.adapter.dispatch({ version: 2, command: "dimensions.hover.clear" }));
  }

  dispose() {
    this.cancelNavigationTimer();
    this.disposed = true;
    this.generation += 1;
    this.running = false;
    this.discardTransient();
    this.operations = [];
  }

  private schedule() {
    if (this.frame === null && !this.disposed) this.frame = requestAnimationFrame(() => this.flush());
  }

  private cancelNavigationTimer() {
    if (this.navigationTimer !== null) clearTimeout(this.navigationTimer);
    this.navigationTimer = null;
    this.navigationGeneration += 1;
  }

  private enqueueInput(input: PendingInput) {
    if (this.disposed) return;
    const last = this.operations.at(-1);
    if (input.kind === "move") {
      if (last?.kind === "move" && sameMovement(last.sample, input.sample)) last.sample = input.sample;
      else this.operations.push({ ...input, transient: true });
    } else {
      // Adjacent batches may span several animation frames while a worker or
      // HTTP request is pending. Preserve each anchor/delta and the native bound.
      const available = last?.kind === "wheel" ? 256 - last.samples.length : 0;
      if (available > 0 && last?.kind === "wheel") last.samples.push(...input.samples.slice(0, available));
      if (input.samples.length > available) this.operations.push({ kind: "wheel", samples: input.samples.slice(available), transient: true });
    }
    this.drain();
  }

  private drain() {
    if (this.running || this.disposed) return;
    const operation = this.operations.shift();
    if (!operation) return;
    this.running = true;
    const generation = this.generation;
    let result: Promise<WorkbenchSnapshot | null>;
    try {
      result = operation.kind === "move" ? this.adapter.pointer(operation.sample)
        : operation.kind === "wheel" ? this.adapter.wheelBatch!(operation.samples) : operation.run();
    }
    catch (error) { result = Promise.reject(error); }
    void result.then((next) => {
      if (next && !this.disposed && generation === this.generation) this.accept(next);
    }).catch((error: unknown) => {
      if (!this.disposed && generation === this.generation) this.fail(error);
    }).finally(() => {
      if (generation !== this.generation) return;
      this.running = false;
      this.drain();
    });
  }
}

interface CanvasViewportProps {
  adapter: WorkbenchAdapter;
  snapshot: WorkbenchSnapshot;
  onSnapshot: (snapshot: WorkbenchSnapshot) => void;
  onCaptureChange: (captured: boolean) => void;
  onError: (error: unknown) => void;
}

export function CanvasViewport({ adapter, snapshot, onSnapshot, onCaptureChange, onError }: CanvasViewportProps) {
  const busy = useWorkbenchBusy(adapter);
  const host = useRef<HTMLDivElement>(null);
  useEffect(() => {
    // Accessibility reflects pending work immediately; visual feedback is delayed.
    // Updating this attribute directly keeps fast hover/camera traffic off React.
    const update = () => host.current?.setAttribute("aria-busy", String(adapter.activity?.getPendingSnapshot() ?? false));
    update();
    return adapter.activity?.subscribePending(update);
  }, [adapter]);
  const canvas = useRef<HTMLCanvasElement>(null);
  const renderer = useRef<CanvasRenderer | null>(null);
  const [renderState, setRenderState] = useState<RendererState>("initializing");
  const capturedPointer = useRef<number | null>(null);
  const capturedButton = useRef<number | null>(null);
  const callbacks = useRef({ adapter, onSnapshot, onCaptureChange, onError });
  callbacks.current = { adapter, onSnapshot, onCaptureChange, onError };
  const acceptedFrames = useMemo(() => ({
    latest: null as WorkbenchSnapshot["frame"]["scene"] | null,
    sequence: undefined as number | undefined,
    forwarded: new WeakSet<WorkbenchSnapshot["frame"]["scene"]>(),
  }), [adapter]);
  const input = useMemo(() => new CanvasInputQueue(adapter, (next) => {
    if (callbacks.current.adapter !== adapter) return;
    const sequence = getCanvasSnapshotSequence(next);
    if (sequence !== undefined && acceptedFrames.sequence !== undefined && sequence < acceptedFrames.sequence) return;
    if (sequence !== undefined) acceptedFrames.sequence = sequence;
    acceptedFrames.latest = next.frame.scene;
    if (isCanvasOnlySnapshot(next)) renderer.current?.accept(next.frame.scene);
    else {
      acceptedFrames.forwarded.add(next.frame.scene);
      callbacks.current.onSnapshot(next);
    }
  }, (error) => callbacks.current.onError(error)), [adapter, acceptedFrames]);

  const retireCapture = (pointerId: number) => {
    if (capturedPointer.current !== pointerId) return false;
    capturedPointer.current = null;
    capturedButton.current = null;
    onCaptureChange(false);
    return true;
  };

  const cancelCapturedPointer = (pointerId: number) => {
    if (!retireCapture(pointerId)) return;
    input.clearDimensionHover();
    input.flush();
    input.dispatch(() => adapter.cancel({ version: 2, reason: "lost-capture" }));
  };

  const sendPointer = (event: React.PointerEvent, phase: "down" | "move" | "up") => {
    // A busy presentation blocks new gestures, never a captured gesture's
    // movement, release or cancellation. Do not detach the canvas or capture.
    if (busy && (phase === "down" || capturedPointer.current === null)) return;
    input.cancelDimensionTimer();
    // Primary authoring and middle-button camera pan are the only canvas
    // pointer routes. Keep secondary clicks available to the browser instead
    // of turning a context-menu gesture into semantic sketch input.
    if ((phase === "down" || phase === "up") && typeof event.button === "number" && event.button !== 0 && event.button !== 1) return;
    if (phase === "move" && ((event.buttons ?? 0) & 2) !== 0) return;
    const bounds = event.currentTarget.getBoundingClientRect();
    const coalesce = phase === "move" && (
      (event.buttons === 0 && capturedPointer.current === null && snapshot.presentation.activeTool === "select")
      || (event.buttons === 4 && capturedPointer.current === event.pointerId && capturedButton.current === 1)
    );
    if (phase === "down" && capturedPointer.current === null) {
      capturedPointer.current = event.pointerId;
      capturedButton.current = event.button;
      try {
        event.currentTarget.setPointerCapture(event.pointerId);
        onCaptureChange(true);
      } catch (error) {
        capturedPointer.current = null;
        capturedButton.current = null;
        onError(error);
      }
    } else if (phase === "up") {
      // Browsers release pointer capture after pointerup and then emit
      // lostpointercapture. Retire ownership before awaiting the adapter so
      // that expected follow-up event is a stale no-op, not a cancellation.
      retireCapture(event.pointerId);
    }
    const x = event.clientX - bounds.left;
    const y = event.clientY - bounds.top;
    input.pointer({ version: 2, phase, pointerId: event.pointerId, x, y, buttons: event.buttons, modifiers: { alt: event.altKey, ctrl: event.ctrlKey, meta: event.metaKey, shift: event.shiftKey } }, coalesce);
    // Keep the current preview during geometry-to-label transit. Native picking
    // decides whether this idle position retains it or reveals another measure.
    if (phase === "move" && event.buttons === 0 && capturedPointer.current === null && snapshot.presentation.activeTool === "select" && (snapshot.dimensions?.mode ?? "focused") === "focused") input.dimensionHover(x, y);
    // Hit-test a preview label before retiring hover ownership on pointer-down.
    // The native selected/editing dimension then owns its continuing visibility.
    else if (phase === "down") input.clearDimensionHover();
  };

  useEffect(() => {
    if (!canvas.current) return;
    const active = createCanvasRenderer(canvas.current, { onState: setRenderState });
    renderer.current = active;
    return () => { active.destroy(); renderer.current = null; };
  }, []);

  useEffect(() => {
    // React can commit a full response after a later canvas-only response was
    // already painted. Keep that commit from repainting an older camera.
    const sequence = getCanvasSnapshotSequence(snapshot);
    if (sequence !== undefined && acceptedFrames.sequence !== undefined && sequence < acceptedFrames.sequence) return;
    if (sequence !== undefined) acceptedFrames.sequence = sequence;
    const forwarded = acceptedFrames.forwarded.has(snapshot.frame.scene);
    if (!forwarded) input.discardTransient();
    const frame = forwarded && acceptedFrames.latest ? acceptedFrames.latest : snapshot.frame.scene;
    acceptedFrames.latest = frame;
    acceptedFrames.forwarded.delete(snapshot.frame.scene);
    renderer.current?.accept(frame);
  }, [acceptedFrames, input, snapshot]);

  useEffect(() => {
    input.activate();
    const discardBlurred = () => { input.discardTransient(); input.clearDimensionHover(); };
    const discardHidden = () => { if (document.hidden) discardBlurred(); };
    const flushBeforeCommand = () => { input.cancelDimensionTimer(); input.flush(); };
    // Menu, project and keyboard actions may change the Rust workbench without
    // passing through this component. Drain navigation before those handlers.
    window.addEventListener("pointerdown", flushBeforeCommand, true);
    window.addEventListener("click", flushBeforeCommand, true);
    window.addEventListener("keydown", flushBeforeCommand, true);
    document.addEventListener("visibilitychange", discardHidden);
    window.addEventListener("blur", discardBlurred);
    return () => {
      input.clearDimensionHover();
      input.dispose();
      window.removeEventListener("pointerdown", flushBeforeCommand, true);
      window.removeEventListener("click", flushBeforeCommand, true);
      window.removeEventListener("keydown", flushBeforeCommand, true);
      document.removeEventListener("visibilitychange", discardHidden);
      window.removeEventListener("blur", discardBlurred);
      if (capturedPointer.current !== null) callbacks.current.onCaptureChange(false);
      capturedPointer.current = null;
      capturedButton.current = null;
    };
  }, [input]);

  useEffect(() => {
    if (snapshot.presentation.activeTool !== "select" || (snapshot.dimensions?.mode ?? "focused") !== "focused") input.clearDimensionHover();
  }, [input, snapshot.presentation.activeTool, snapshot.dimensions?.mode]);

  useEffect(() => {
    const element = host.current;
    if (!element) return;
    let media: MediaQueryList | null = null;
    let size = { width: 0, height: 0 };
    const resize = () => {
      const pixelRatio = window.devicePixelRatio || 1;
      renderer.current?.resize({ ...size, pixelRatio });
      if (size.width <= 0 || size.height <= 0) { input.discardTransient(); input.clearDimensionHover(); return; }
      input.clearDimensionHover();
      input.flush();
      const resized = { version: 2 as const, ...size, pixelRatio };
      input.dispatch(() => adapter.resize(resized), true);
    };
    const watchPixelRatio = () => {
      media?.removeEventListener("change", changedPixelRatio);
      media = matchMedia(`(resolution: ${window.devicePixelRatio || 1}dppx)`);
      media.addEventListener("change", changedPixelRatio);
    };
    const changedPixelRatio = () => { resize(); watchPixelRatio(); };
    const observer = new ResizeObserver(([entry]) => {
      size = { width: entry.contentRect.width, height: entry.contentRect.height };
      resize();
    });
    observer.observe(element);
    watchPixelRatio();
    window.addEventListener("resize", resize);
    return () => { observer.disconnect(); media?.removeEventListener("change", changedPixelRatio); window.removeEventListener("resize", resize); };
  }, [adapter, input]);

  return (
    <div
      ref={host}
      className="relative h-full min-h-0 w-full touch-none overflow-hidden bg-canvas outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-accent"
      role="application"
      aria-label={snapshot.frame.ariaLabel}
      tabIndex={0}
      onPointerDown={(event) => void sendPointer(event, "down")}
      onPointerMove={(event) => void sendPointer(event, "move")}
      onPointerUp={(event) => void sendPointer(event, "up")}
      onPointerLeave={() => { input.clearDimensionHover(); if (capturedPointer.current === null) input.discardHover(); }}
      onPointerCancel={(event) => cancelCapturedPointer(event.pointerId)}
      onLostPointerCapture={(event) => cancelCapturedPointer(event.pointerId)}
      onWheel={(event) => { if (busy) return; const bounds = event.currentTarget.getBoundingClientRect(); input.wheel({ version: 2, x: event.clientX - bounds.left, y: event.clientY - bounds.top, deltaX: event.deltaX, deltaY: event.deltaY, ctrl: event.ctrlKey }); }}
    >
      <canvas ref={canvas} className="geosolve-canvas" aria-hidden="true" />
      {renderState !== "ready" && <div className="geosolve-render-status" role="status">
        {renderState === "lost" ? "Graphics connection lost. Waiting for recovery…" : renderState === "unavailable" ? "WebGL2 is unavailable. Enable graphics acceleration to show this sketch." : "Preparing sketch…"}
      </div>}
    </div>
  );
}
