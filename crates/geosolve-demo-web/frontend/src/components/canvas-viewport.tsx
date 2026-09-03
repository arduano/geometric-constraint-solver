// SPDX-License-Identifier: GPL-3.0-or-later
import { useEffect, useRef } from "react";
import type { WorkbenchAdapter, WorkbenchSnapshot } from "../lib/adapter";

interface CanvasViewportProps {
  adapter: WorkbenchAdapter;
  snapshot: WorkbenchSnapshot;
  onSnapshot: (snapshot: WorkbenchSnapshot) => void;
  onCaptureChange: (captured: boolean) => void;
  onError: (error: unknown) => void;
}

export function CanvasViewport({ adapter, snapshot, onSnapshot, onCaptureChange, onError }: CanvasViewportProps) {
  const host = useRef<HTMLDivElement>(null);
  const capturedPointer = useRef<number | null>(null);

  const retireCapture = (pointerId: number) => {
    if (capturedPointer.current !== pointerId) return false;
    capturedPointer.current = null;
    onCaptureChange(false);
    return true;
  };

  const cancelCapturedPointer = (pointerId: number) => {
    if (!retireCapture(pointerId)) return;
    void adapter.cancel({ version: 1, reason: "lost-capture" }).then((next) => next && onSnapshot(next)).catch(onError);
  };

  const sendPointer = async (event: React.PointerEvent, phase: "down" | "move" | "up") => {
    // Primary authoring and middle-button camera pan are the only canvas
    // pointer routes. Keep secondary clicks available to the browser instead
    // of turning a context-menu gesture into semantic sketch input.
    if ((phase === "down" || phase === "up") && typeof event.button === "number" && event.button !== 0 && event.button !== 1) return;
    if (phase === "move" && ((event.buttons ?? 0) & 2) !== 0) return;
    const bounds = event.currentTarget.getBoundingClientRect();
    if (phase === "down" && capturedPointer.current === null) {
      capturedPointer.current = event.pointerId;
      try {
        event.currentTarget.setPointerCapture(event.pointerId);
        onCaptureChange(true);
      } catch (error) {
        capturedPointer.current = null;
        onError(error);
      }
    } else if (phase === "up") {
      // Browsers release pointer capture after pointerup and then emit
      // lostpointercapture. Retire ownership before awaiting the adapter so
      // that expected follow-up event is a stale no-op, not a cancellation.
      retireCapture(event.pointerId);
    }
    try {
      const next = await adapter.pointer({ version: 1, phase, pointerId: event.pointerId, x: event.clientX - bounds.left, y: event.clientY - bounds.top, buttons: event.buttons, modifiers: { alt: event.altKey, ctrl: event.ctrlKey, meta: event.metaKey, shift: event.shiftKey } });
      if (next) onSnapshot(next);
    } catch (error) {
      onError(error);
    }
  };

  useEffect(() => {
    const element = host.current;
    if (!element) return;
    const observer = new ResizeObserver(([entry]) => {
      if (entry.contentRect.width <= 0 || entry.contentRect.height <= 0) return;
      void adapter.resize({ version: 1, width: entry.contentRect.width, height: entry.contentRect.height, pixelRatio: window.devicePixelRatio }).then((next) => next && onSnapshot(next)).catch(onError);
    });
    observer.observe(element);
    return () => observer.disconnect();
  }, [adapter, onError, onSnapshot]);

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
      onPointerCancel={(event) => cancelCapturedPointer(event.pointerId)}
      onLostPointerCapture={(event) => cancelCapturedPointer(event.pointerId)}
      onWheel={(event) => { const bounds = event.currentTarget.getBoundingClientRect(); void adapter.wheel({ version: 1, x: event.clientX - bounds.left, y: event.clientY - bounds.top, deltaX: event.deltaX, deltaY: event.deltaY, ctrl: event.ctrlKey }).then((next) => next && onSnapshot(next)).catch(onError); }}
      dangerouslySetInnerHTML={{ __html: snapshot.frame.svg }}
    />
  );
}
