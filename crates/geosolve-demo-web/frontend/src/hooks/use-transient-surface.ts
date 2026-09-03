// SPDX-License-Identifier: GPL-3.0-or-later
import { useCallback, useEffect, useRef, useState } from "react";

export type TransientSurface = "file" | "tools-sketch" | "tools-constraint" | "tools-dimension" | "tools-modify" | "diagnostics" | "open" | null;

/** One light-dismiss surface. Pointer dismissal happens during capture and never cancels the destination click. */
export function useTransientSurface() {
  const [active, setActive] = useState<TransientSurface>(null);
  const contentRef = useRef<HTMLElement | null>(null);
  const invokerRef = useRef<HTMLElement | null>(null);

  const close = useCallback((restoreFocus = false) => {
    setActive(null);
    if (restoreFocus) queueMicrotask(() => invokerRef.current?.focus());
  }, []);

  const toggle = useCallback((surface: Exclude<TransientSurface, null>, invoker: HTMLElement) => {
    invokerRef.current = invoker;
    setActive((current) => current === surface ? null : surface);
  }, []);

  useEffect(() => {
    if (!active) return;
    const outside = (event: PointerEvent) => {
      const target = event.target as Node;
      if (!contentRef.current?.contains(target) && !invokerRef.current?.contains(target)) close(false);
    };
    document.addEventListener("pointerdown", outside, true);
    return () => document.removeEventListener("pointerdown", outside, true);
  }, [active, close]);

  return { active, contentRef, close, toggle };
}
