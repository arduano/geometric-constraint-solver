// SPDX-License-Identifier: GPL-3.0-or-later
import { useEffect } from "react";
import { Button } from "./ui/button";
import { TransientPopover } from "./transient-popover";
import { ToolIcon } from "./tool-icon";
import type { TransientSurface } from "../hooks/use-transient-surface";
import type { ToolCatalog, ToolCommandDefinition, ToolSectionDefinition } from "../lib/tool-catalog";
import { toolSection } from "../lib/tool-catalog";

type ToolSurface = Extract<Exclude<TransientSurface, null>, `tools-${string}`>;
const REPRESENTATIVE: Record<ToolSectionDefinition["id"], string> = {
  sketch: "segment",
  constraint: "coincident",
  dimension: "point-distance",
  modify: "fillet",
};

interface ToolRailProps {
  catalog: ToolCatalog;
  activeTool: string;
  activeSurface: TransientSurface;
  surfaceRef: React.RefObject<HTMLElement | null>;
  selectTool: (id: string, origin?: HTMLElement) => void;
  toggleSurface: (id: ToolSurface, invoker: HTMLElement) => void;
}

export function ToolRail({ catalog, activeTool, activeSurface, surfaceRef, selectTool, toggleSurface }: ToolRailProps) {
  const activeSection = toolSection(catalog, activeTool);
  const openSection = catalog.sections.find((section) => activeSurface === surfaceFor(section));

  useEffect(() => {
    if (!openSection) return;
    queueMicrotask(() => {
      const preferred = Array.from(surfaceRef.current?.querySelectorAll<HTMLElement>("[data-tool-id]") ?? []).find((item) => item.dataset.toolId === activeTool);
      (preferred ?? surfaceRef.current?.querySelector<HTMLElement>('[role="menuitem"]'))?.focus();
    });
  }, [activeTool, openSection, surfaceRef]);

  return (
    <nav aria-label="Primary tools" className="relative z-30 flex w-12 shrink-0 flex-col items-center border-r border-border bg-surface py-2">
      <RailButton
        active={activeTool === catalog.select.toolId}
        command={catalog.select}
        label="Select"
        onClick={(button) => selectTool(catalog.select.toolId, button)}
      />
      <div aria-hidden="true" className="my-2 h-px w-7 bg-border" />
      <div role="group" aria-label="Authoring tool groups" className="grid gap-1">
        {catalog.sections.map((section) => {
          const surface = surfaceFor(section);
          const expanded = activeSurface === surface;
          const active = activeSection?.id === section.id;
          const command = active
            ? section.commands.find((candidate) => candidate.toolId === activeTool)
            : section.commands.find((candidate) => candidate.toolId === REPRESENTATIVE[section.id]);
          return (
            <RailButton
              key={section.id}
              active={active}
              command={command ?? section.commands[0]!}
              controls={expanded ? `tool-menu-${section.id}` : undefined}
              expanded={expanded}
              label={section.label}
              onClick={(button) => toggleSurface(surface, button)}
            />
          );
        })}
      </div>
      {openSection && (
        <ToolMenu
          activeTool={activeTool}
          section={openSection}
          selectTool={selectTool}
          surfaceRef={surfaceRef}
        />
      )}
    </nav>
  );
}

function RailButton({ active, command, controls, expanded, label, onClick }: { active: boolean; command: ToolCommandDefinition; controls?: string; expanded?: boolean; label: string; onClick: (button: HTMLButtonElement) => void }) {
  const category = expanded !== undefined;
  return (
    <Button
      aria-controls={controls}
      aria-expanded={category ? expanded : undefined}
      aria-haspopup={category ? "menu" : undefined}
      aria-label={label}
      aria-pressed={active}
      className={`group relative size-9 ${active ? "bg-amber-400/15 text-accent hover:bg-amber-400/20" : "text-muted"}`}
      onClick={(event) => onClick(event.currentTarget)}
      size="icon"
      title={label}
      variant="ghost"
    >
      {active && <span aria-hidden="true" className="absolute -left-1.5 h-5 w-0.5 rounded-r bg-accent" />}
      <ToolIcon className="size-[19px] transition-colors group-hover:text-foreground" icon={command.icon} />
    </Button>
  );
}

function ToolMenu({ activeTool, section, selectTool, surfaceRef }: { activeTool: string; section: ToolSectionDefinition; selectTool: (id: string, origin?: HTMLElement) => void; surfaceRef: React.RefObject<HTMLElement | null> }) {
  const groups = grouped(section.commands);
  const columns = section.id === "sketch" || section.id === "constraint" ? "grid-cols-2" : "grid-cols-1";
  const width = section.id === "sketch" ? "w-[35rem]" : section.id === "constraint" ? "w-[29rem]" : "w-[22rem]";
  return (
    <TransientPopover surfaceRef={surfaceRef} label={`${section.label} tools`} className={`left-11 top-2 max-h-[calc(100vh-64px)] ${width} overflow-y-auto p-0`}>
      <header className="border-b border-border px-3 py-2.5">
        <div className="flex items-baseline gap-2"><h2 className="text-sm font-semibold text-foreground">{section.label}</h2><span className="text-[10px] text-muted">{section.commands.length} tools</span></div>
        <p className="mt-0.5 text-[11px] text-muted">{section.description}</p>
      </header>
      <div id={`tool-menu-${section.id}`} role="menu" aria-label={`${section.label} tools`} className={`grid ${columns} items-start gap-x-2 gap-y-3 p-2.5`}>
        {groups.map(([group, commands], groupIndex) => {
          const heading = `tool-group-${section.id}-${groupIndex}`;
          return (
            <section key={group} role="group" aria-labelledby={heading} className="min-w-0">
              <h3 id={heading} className="mb-1 px-1 text-[10px] font-semibold uppercase tracking-wider text-muted">{group}</h3>
              <div className="grid gap-0.5">
                {commands.map((command) => (
                  <button
                    key={command.stableId}
                    aria-current={activeTool === command.toolId ? "true" : undefined}
                    className="group flex min-h-9 w-full items-center gap-2 rounded px-2 py-1.5 text-left text-xs leading-tight text-foreground outline-none hover:bg-neutral-700 focus-visible:bg-neutral-700 focus-visible:ring-1 focus-visible:ring-accent aria-current:bg-amber-400/10 aria-current:text-accent"
                    data-tool-id={command.toolId}
                    onClick={(event) => selectTool(command.toolId, event.currentTarget)}
                    role="menuitem"
                  >
                    <ToolIcon className="size-[18px] text-accent/90 group-hover:text-accent" icon={command.icon} />
                    <span>{command.label}</span>
                  </button>
                ))}
              </div>
            </section>
          );
        })}
      </div>
    </TransientPopover>
  );
}

function grouped(commands: ToolCommandDefinition[]) {
  const groups = new Map<string, ToolCommandDefinition[]>();
  for (const command of commands) groups.set(command.group, [...(groups.get(command.group) ?? []), command]);
  return [...groups.entries()];
}

function surfaceFor(section: ToolSectionDefinition): ToolSurface {
  return `tools-${section.id}` as ToolSurface;
}
