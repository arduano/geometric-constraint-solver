// SPDX-License-Identifier: GPL-3.0-or-later
import { useEffect, useMemo, useRef, useState } from "react";
import { createCanvasRenderer, type CanvasRenderer, type RendererState } from "../lib/canvas-renderer";
import { getCanvasSnapshotSequence, isCanvasOnlySnapshot, type PointerSample, type WheelSample, type WorkbenchAdapter, type WorkbenchSnapshot } from "../lib/adapter";

type CanvasOperation = () => Promise<WorkbenchSnapshot | null>;
type PendingInput = { kind: "move"; sample: PointerSample } | { kind: "wheel"; samples: WheelSample[] };

// Retain every semantic sample and every wheel anchor. Only idle Select hover
// and fixed-origin middle pan may replace an earlier movement in the same RAF.
class CanvasInputQueue {
  private pending: PendingInput | null = null;
  private frame: number | null = null;
  private operations: Array<{ run: CanvasOperation; transient: boolean }> = [];
  private running = false;
  private disposed = false;
  private generation = 0;

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
    if (this.pending?.kind !== "move" || this.pending.sample.pointerId !== sample.pointerId) this.flush();
    this.pending = { kind: "move", sample };
    this.schedule();
  }

  wheel(sample: WheelSample) {
    if (this.pending?.kind !== "wheel") this.flush();
    // Match the bounded native batch. Never add deltas: clamp and changing
    // anchors make the individual ordered zoom operations significant.
    if (this.pending?.kind === "wheel" && this.pending.samples.length === 256) this.flush();
    if (this.pending?.kind === "wheel") this.pending.samples.push(sample);
    else this.pending = { kind: "wheel", samples: [sample] };
    this.schedule();
  }

  flush() {
    if (this.frame !== null) cancelAnimationFrame(this.frame);
    this.frame = null;
    const pending = this.pending;
    this.pending = null;
    if (!pending) return;
    if (pending.kind === "move") this.dispatch(() => this.adapter.pointer(pending.sample), true);
    else if (this.adapter.wheelBatch) this.dispatch(() => this.adapter.wheelBatch!(pending.samples), true);
    else for (const sample of pending.samples) this.dispatch(() => this.adapter.wheel(sample), true);
  }

  dispatch(run: CanvasOperation, transient = false) {
    if (this.disposed) return;
    this.operations.push({ run, transient });
    this.drain();
  }

  discardHover() {
    if (this.pending?.kind !== "move" || this.pending.sample.buttons !== 0) return;
    if (this.frame !== null) cancelAnimationFrame(this.frame);
    this.frame = null;
    this.pending = null;
  }

  discardTransient() {
    if (this.frame !== null) cancelAnimationFrame(this.frame);
    this.frame = null;
    this.pending = null;
    this.operations = this.operations.filter((operation) => !operation.transient);
  }

  activate() { this.disposed = false; }

  dispose() {
    this.disposed = true;
    this.generation += 1;
    this.running = false;
    this.discardTransient();
    this.operations = [];
  }

  private schedule() {
    if (this.frame === null && !this.disposed) this.frame = requestAnimationFrame(() => this.flush());
  }

  private drain() {
    if (this.running || this.disposed) return;
    const operation = this.operations.shift();
    if (!operation) return;
    this.running = true;
    const generation = this.generation;
    let result: Promise<WorkbenchSnapshot | null>;
    try { result = operation.run(); }
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
  const host = useRef<HTMLDivElement>(null);
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
    input.flush();
    input.dispatch(() => adapter.cancel({ version: 2, reason: "lost-capture" }));
  };

  const sendPointer = (event: React.PointerEvent, phase: "down" | "move" | "up") => {
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
    input.pointer({ version: 2, phase, pointerId: event.pointerId, x: event.clientX - bounds.left, y: event.clientY - bounds.top, buttons: event.buttons, modifiers: { alt: event.altKey, ctrl: event.ctrlKey, meta: event.metaKey, shift: event.shiftKey } }, coalesce);
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
    const discardHidden = () => { if (document.hidden) input.discardTransient(); };
    const discardBlurred = () => input.discardTransient();
    const flushBeforeCommand = () => input.flush();
    // Menu, project and keyboard actions may change the Rust workbench without
    // passing through this component. Drain navigation before those handlers.
    window.addEventListener("pointerdown", flushBeforeCommand, true);
    window.addEventListener("click", flushBeforeCommand, true);
    window.addEventListener("keydown", flushBeforeCommand, true);
    document.addEventListener("visibilitychange", discardHidden);
    window.addEventListener("blur", discardBlurred);
    return () => {
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
    const element = host.current;
    if (!element) return;
    let media: MediaQueryList | null = null;
    let size = { width: 0, height: 0 };
    const resize = () => {
      const pixelRatio = window.devicePixelRatio || 1;
      renderer.current?.resize({ ...size, pixelRatio });
      if (size.width <= 0 || size.height <= 0) { input.discardTransient(); return; }
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
      onPointerLeave={() => { if (capturedPointer.current === null) input.discardHover(); }}
      onPointerCancel={(event) => cancelCapturedPointer(event.pointerId)}
      onLostPointerCapture={(event) => cancelCapturedPointer(event.pointerId)}
      onWheel={(event) => { const bounds = event.currentTarget.getBoundingClientRect(); input.wheel({ version: 2, x: event.clientX - bounds.left, y: event.clientY - bounds.top, deltaX: event.deltaX, deltaY: event.deltaY, ctrl: event.ctrlKey }); }}
    >
      <canvas ref={canvas} className="geosolve-canvas" aria-hidden="true" />
      {renderState !== "ready" && <div className="geosolve-render-status" role="status">
        {renderState === "lost" ? "Graphics connection lost. Waiting for recovery…" : renderState === "unavailable" ? "WebGL2 is unavailable. Enable graphics acceleration to show this sketch." : "Preparing sketch…"}
      </div>}
    </div>
  );
}
