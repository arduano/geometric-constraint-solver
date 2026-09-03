// SPDX-License-Identifier: GPL-3.0-or-later
import { Crosshair, Grid3X3, Scan } from "lucide-react";
import { Button } from "./ui/button";

export function CanvasControls({ gridVisible, onCommand }: { gridVisible: boolean; onCommand: (command: "view.grid.toggle" | "view.fit" | "view.origin") => void }) {
  return (
    <div role="toolbar" aria-label="Canvas view" className="absolute right-3 top-3 z-20 flex items-center gap-0.5 rounded-md border border-border bg-surface/95 p-1 shadow-panel backdrop-blur-sm">
      <Button
        aria-label={gridVisible ? "Hide grid" : "Show grid"}
        aria-pressed={gridVisible}
        className="size-8 aria-pressed:bg-amber-400/15 aria-pressed:text-accent"
        onClick={() => onCommand("view.grid.toggle")}
        size="icon"
        title={gridVisible ? "Hide grid" : "Show grid"}
        variant="ghost"
      ><Grid3X3 className="size-4" /></Button>
      <span aria-hidden="true" className="mx-0.5 h-5 w-px bg-border" />
      <Button aria-label="Fit sketch" className="size-8" onClick={() => onCommand("view.fit")} size="icon" title="Fit sketch" variant="ghost"><Scan className="size-4" /></Button>
      <Button aria-label="Center on origin" className="size-8" onClick={() => onCommand("view.origin")} size="icon" title="Center on origin" variant="ghost"><Crosshair className="size-4" /></Button>
    </div>
  );
}
