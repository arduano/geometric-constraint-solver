// SPDX-License-Identifier: GPL-3.0-or-later
import type { ReactNode, RefObject } from "react";
import { cn } from "../lib/cn";

interface TransientPopoverProps {
  surfaceRef: RefObject<HTMLElement | null>;
  label: string;
  children: ReactNode;
  className?: string;
}

export function TransientPopover({ surfaceRef, label, children, className }: TransientPopoverProps) {
  const navigate = (event: React.KeyboardEvent<HTMLElement>) => {
    if (!["ArrowDown", "ArrowUp", "Home", "End"].includes(event.key)) return;
    const items = Array.from(event.currentTarget.querySelectorAll<HTMLElement>('[role="menuitem"]:not([disabled])'));
    if (!items.length) return;
    event.preventDefault();
    const current = items.indexOf(document.activeElement as HTMLElement);
    const next = event.key === "Home" ? 0 : event.key === "End" ? items.length - 1 : event.key === "ArrowDown" ? (current + 1 + items.length) % items.length : (current - 1 + items.length) % items.length;
    items[next]?.focus();
  };
  return (
    <section ref={surfaceRef} aria-label={label} onKeyDown={navigate} className={cn("absolute z-40 rounded-lg border border-border bg-raised p-1.5 shadow-panel", className)}>
      {children}
    </section>
  );
}
