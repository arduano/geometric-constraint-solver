// SPDX-License-Identifier: GPL-3.0-or-later
import { AlertCircle, Box, Braces, ChevronDown, ChevronUp, DraftingCompass, ExternalLink, Eye, EyeOff, Focus, GripVertical, Minus, Pencil, RotateCcw, SlidersHorizontal, Trash2 } from "lucide-react";
import { useState } from "react";
import type { DeclarationCapability, DeclarationRow, WorkbenchSnapshot } from "../lib/adapter";
import { Button } from "./ui/button";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "./ui/tabs";

export type DeclarationMove = { direction: "up" | "down" } | { targetId: string; position: "before" | "after" };

export interface DeclarationPanelActions {
  onSelect: (id: string) => void;
  onNavigate: (row: DeclarationRow, edit: boolean) => void;
  onMove: (id: string, move: DeclarationMove) => void;
  onVisibility: (id: string, visible: boolean) => void;
  onIsolate: (id: string) => void;
  onRestoreVisibility: () => void;
  onConstructionVisibility: () => void;
  onSuppress: (id: string, suppressed: boolean) => void;
  onDelete: (id: string) => void;
}

export function Explorer({ snapshot, actions, blockedReason }: { snapshot: WorkbenchSnapshot; actions: DeclarationPanelActions; blockedReason?: string }) {
  return <aside aria-label="Explorer" className="flex h-full min-h-0 flex-col bg-surface">
    <header className="shrink-0 border-b border-border p-2">
      <div className="px-1 text-xs font-semibold uppercase tracking-wide text-muted">Explorer</div>
      <div role="group" aria-label="Explorer display filters" className="mt-2 flex min-w-0 items-center gap-1">
        <Button aria-label={`${snapshot.presentation.constructionVisible ? "Hide" : "Show"} construction geometry`} aria-pressed={snapshot.presentation.constructionVisible} className="h-7 min-w-0 flex-1 justify-start px-2 text-[10px]" onClick={actions.onConstructionVisibility} size="compact" title="Show or hide all explicit and Fillet-derived construction geometry without changing solver participation" variant="ghost"><DraftingCompass className="size-3.5 shrink-0" /><span className="truncate">Construction</span></Button>
        <Button aria-label="Restore visibility before isolate" className="size-7 shrink-0" disabled={!snapshot.presentation.visibilityRestoreAvailable} onClick={actions.onRestoreVisibility} size="icon" title={snapshot.presentation.visibilityRestoreAvailable ? "Restore visibility from before group isolation" : "No isolated visibility state to restore"} variant="ghost"><RotateCcw className="size-3.5" /></Button>
      </div>
    </header>
    <DeclarationPanel rows={snapshot.explorer} actions={actions} blockedReason={blockedReason} className="min-h-0 flex-1 overflow-auto p-2" />
  </aside>;
}

export function DeclarationPanel({ rows, actions, blockedReason, className = "" }: { rows: DeclarationRow[]; actions: DeclarationPanelActions; blockedReason?: string; className?: string }) {
  const [dragged, setDragged] = useState<string | null>(null);
  const [drop, setDrop] = useState<{ id: string; position: "before" | "after" } | null>(null);
  const finishDrag = () => { setDragged(null); setDrop(null); };
  if (!rows.length) return <div className={className}><Empty icon={<Braces />} title="No declarations" detail="Accepted declarations appear here in sketch.ts order." /></div>;
  return <div className={className}>{blockedReason && <p role="status" className="mb-2 rounded border border-amber-400/30 bg-amber-400/10 p-2 text-xs text-accent">{blockedReason}</p>}<ul aria-label="Ordered declarations" className="grid content-start gap-1">{rows.map((row) => <DeclarationTreeRow key={row.id} row={row} depth={0} actions={actions} blockedReason={blockedReason} dragged={dragged} drop={drop} onDrag={setDragged} onDropTarget={setDrop} onDragEnd={finishDrag} />)}</ul></div>;
}

function DeclarationTreeRow({ row, depth, actions, blockedReason, dragged, drop, onDrag, onDropTarget, onDragEnd }: { row: DeclarationRow; depth: number; actions: DeclarationPanelActions; blockedReason?: string; dragged: string | null; drop: { id: string; position: "before" | "after" } | null; onDrag: (id: string | null) => void; onDropTarget: (target: { id: string; position: "before" | "after" } | null) => void; onDragEnd: () => void }) {
  if (row.rowKind === "group") {
    return <li><div className="mt-2 flex h-7 items-center gap-1 border-b border-border px-1 text-[10px] font-semibold uppercase tracking-wider text-muted first:mt-0"><Box className="size-3 shrink-0" /><span className="min-w-0 flex-1 truncate">{row.label}</span><span className="shrink-0 tabular-nums">{row.children.length}</span><VisibilityButton row={row} onVisibility={actions.onVisibility} /><button type="button" aria-label={`Isolate ${row.label}`} onClick={() => actions.onIsolate(row.id)} title={`Show only ${row.label}; Restore returns the previous visibility`} className="grid size-6 shrink-0 place-items-center rounded outline-none hover:bg-raised hover:text-foreground focus-visible:ring-1 focus-visible:ring-accent"><Focus className="size-3" /></button></div>{row.children.length > 0 && <ul className="grid gap-1" aria-label={row.label}>{row.children.map((child) => <DeclarationTreeRow key={child.id} row={child} depth={depth} actions={actions} blockedReason={blockedReason} dragged={dragged} drop={drop} onDrag={onDrag} onDropTarget={onDropTarget} onDragEnd={onDragEnd} />)}</ul>}</li>;
  }
  const blocked = blockedReason ? { enabled: false, reason: blockedReason } : undefined;
  const mutationCapability = (capability: DeclarationCapability) => blocked ?? capability;
  const movable = mutationCapability(row.capabilities.move).enabled;
  const dropClass = drop?.id === row.id ? drop.position === "before" ? "border-t-accent" : "border-b-accent" : "border-y-transparent";
  const nested = row.rowKind === "generated" || depth > 0;
  return <li className={`${nested ? "ml-4 border-l border-border pl-1" : ""} border-y ${dropClass}`}>
    <div className="group rounded" draggable={movable} onDragStart={(event) => { if (!movable) return; event.stopPropagation(); event.dataTransfer.effectAllowed = "move"; event.dataTransfer.setData("application/x-geosolve-declaration", row.id); onDrag(row.id); }} onDragEnd={onDragEnd} onDragOver={(event) => { if (!dragged || dragged === row.id) return; event.preventDefault(); event.stopPropagation(); const bounds = event.currentTarget.getBoundingClientRect(); onDropTarget({ id: row.id, position: event.clientY < bounds.top + bounds.height / 2 ? "before" : "after" }); }} onDrop={(event) => { event.preventDefault(); event.stopPropagation(); if (dragged && dragged !== row.id) actions.onMove(dragged, { targetId: row.id, position: drop?.id === row.id ? drop.position : "before" }); onDragEnd(); }}>
      <div className="flex min-w-0 items-center"><button type="button" aria-current={row.selected ? "true" : undefined} disabled={!row.capabilities.select.enabled} title={row.capabilities.select.enabled ? `${row.kind} declaration` : row.capabilities.select.reason} onClick={() => actions.onSelect(row.id)} className="flex h-8 min-w-0 flex-1 items-center gap-1.5 rounded px-1.5 text-left text-sm text-foreground outline-none hover:bg-raised focus-visible:ring-1 focus-visible:ring-accent disabled:cursor-default disabled:opacity-60 aria-current:bg-amber-400/10 aria-current:text-accent">
        <GripVertical aria-hidden="true" className={`size-3 shrink-0 ${movable ? "cursor-grab text-muted group-active:cursor-grabbing" : "text-transparent"}`} />
        <Box className={`size-3.5 shrink-0 ${row.rowKind === "generated" ? "text-cyan-300" : "text-muted"}`} />
        <span className={`min-w-0 flex-1 truncate ${row.suppressed ? "line-through opacity-60" : ""}`}>{row.label}</span>
        {row.rowKind === "generated" && <span className="shrink-0 rounded bg-cyan-400/10 px-1 text-[8px] uppercase tracking-wide text-cyan-200">generated</span>}
      </button><VisibilityButton row={row} onVisibility={actions.onVisibility} /></div>
      {(row.selected || row.suppressed === true) && <div role="group" aria-label={`${row.label} actions`} className="mb-1 ml-8 flex flex-wrap items-center gap-0.5 px-1">
        <RowAction label="Move up" capability={mutationCapability(row.capabilities.moveUp)} onClick={() => actions.onMove(row.id, { direction: "up" })}><ChevronUp /></RowAction>
        <RowAction label="Move down" capability={mutationCapability(row.capabilities.moveDown)} onClick={() => actions.onMove(row.id, { direction: "down" })}><ChevronDown /></RowAction>
        <RowAction label="Open source" capability={mutationCapability(row.capabilities.navigate)} onClick={() => actions.onNavigate(row, false)}><ExternalLink /></RowAction>
        <RowAction label="Edit source" capability={mutationCapability(row.capabilities.edit)} onClick={() => actions.onNavigate(row, true)}><Pencil /></RowAction>
        <RowAction label={row.suppressed ? "Restore" : "Suppress"} capability={mutationCapability(row.capabilities.suppress)} pressed={row.suppressed} onClick={() => actions.onSuppress(row.id, !row.suppressed)}>{row.suppressed ? <Eye /> : <EyeOff />}</RowAction>
        <RowAction label="Delete" capability={mutationCapability(row.capabilities.delete)} danger onClick={() => actions.onDelete(row.id)}><Trash2 /></RowAction>
      </div>}
    </div>
    {row.children.length > 0 && <ul aria-label={`${row.label} generated outputs`} className="grid gap-1">{row.children.map((child) => <DeclarationTreeRow key={child.id} row={child} depth={depth + 1} actions={actions} blockedReason={blockedReason} dragged={dragged} drop={drop} onDrag={onDrag} onDropTarget={onDropTarget} onDragEnd={onDragEnd} />)}</ul>}
  </li>;
}

function VisibilityButton({ row, onVisibility }: { row: DeclarationRow; onVisibility: (id: string, visible: boolean) => void }) {
  const action = row.visible ? "Hide" : "Show";
  const inherited = row.visible && !row.effectiveVisible;
  const pressed = row.visibilityState === "mixed" ? "mixed" : row.visible;
  const title = inherited ? `${row.label} is individually shown but hidden by an ancestor group` : `${action} ${row.label} without suppressing or changing solver activity`;
  return <button type="button" aria-label={`${action} ${row.label}`} aria-pressed={pressed} data-visibility-state={row.visibilityState} onClick={() => onVisibility(row.id, !row.visible)} title={title} className={`relative grid size-6 shrink-0 place-items-center rounded outline-none hover:bg-raised hover:text-foreground focus-visible:ring-1 focus-visible:ring-accent ${inherited ? "text-muted/50" : "text-muted"}`}>
    {row.visible ? <Eye className="size-3.5" /> : <EyeOff className="size-3.5" />}
    {row.visibilityState === "mixed" && <Minus className="absolute size-2.5 stroke-[3]" />}
  </button>;
}

function RowAction({ label, capability, pressed, danger = false, onClick, children }: { label: string; capability: DeclarationCapability; pressed?: boolean; danger?: boolean; onClick: () => void; children: React.ReactNode }) {
  return <button type="button" aria-label={label} aria-pressed={pressed} disabled={!capability.enabled} title={capability.enabled ? label : capability.reason} onClick={onClick} className={`grid size-6 place-items-center rounded outline-none focus-visible:ring-1 focus-visible:ring-accent disabled:cursor-not-allowed disabled:opacity-30 [&>svg]:size-3 ${danger ? "text-red-300 hover:bg-danger/20" : "text-muted hover:bg-raised hover:text-foreground"}`}>{children}</button>;
}

export function DetailsPanel({ snapshot, onOpenCode, onParameterEdit, onProblemOpen, parametersBlocked }: { snapshot: WorkbenchSnapshot; onOpenCode: () => void; onParameterEdit: (id: string, value: string) => void; onProblemOpen: (problem: WorkbenchSnapshot["problems"][number]) => void; parametersBlocked: boolean }) {
  return <Tabs defaultValue="inspector" className="flex h-full min-h-0 flex-col bg-surface"><TabsList aria-label="Details"><TabsTrigger value="inspector">Inspector</TabsTrigger><TabsTrigger value="parameters">Parameters</TabsTrigger><TabsTrigger value="problems">Problems {snapshot.problems.length > 0 && <span className="ml-1 rounded bg-danger px-1 text-[9px] text-white">{snapshot.problems.length}</span>}</TabsTrigger></TabsList><TabsContent value="inspector" className="overflow-auto p-3"><Inspector snapshot={snapshot} onOpenCode={onOpenCode} /></TabsContent><TabsContent value="parameters" className="overflow-auto p-3"><ParametersView snapshot={snapshot} onEdit={onParameterEdit} blocked={parametersBlocked} /></TabsContent><TabsContent value="problems" className="overflow-auto p-3"><ProblemsView snapshot={snapshot} onOpen={onProblemOpen} /></TabsContent></Tabs>;
}

function Inspector({ snapshot, onOpenCode }: { snapshot: WorkbenchSnapshot; onOpenCode: () => void }) {
  if (!snapshot.selection) return <Empty icon={<SlidersHorizontal />} title="Nothing selected" detail="Select sketch geometry to inspect it." />;
  return <div><div className="mb-4"><p className="text-[10px] font-semibold uppercase tracking-wider text-muted">{snapshot.selection.kind}</p><h2 className="mt-1 truncate text-base font-medium text-foreground">{snapshot.selection.label}</h2></div><dl className="grid grid-cols-[auto_1fr] gap-x-4 gap-y-2 text-xs"><dt className="text-muted">Ownership</dt><dd className="text-right text-foreground">{snapshot.selection.ownership ?? "Editable native"}</dd><dt className="text-muted">Status</dt><dd className="text-right text-emerald-300">Accepted</dd></dl>{snapshot.selection.ownership?.includes("source") && <Button onClick={onOpenCode} className="mt-4 w-full"><ExternalLink className="size-3.5" />Open in code</Button>}</div>;
}
export function ParametersView({ snapshot, onEdit, blocked }: { snapshot: WorkbenchSnapshot; onEdit: (id: string, value: string) => void; blocked: boolean }) {
  if (!snapshot.parameters.length) return <Empty icon={<Braces />} title="No parameters" detail="Managed source parameters appear here." />;
  return <div className="grid gap-3">{blocked && <p role="status" className="rounded border border-amber-400/30 bg-amber-400/10 p-2 text-xs text-accent">Apply or Revert the source draft before editing parameters.</p>}{snapshot.parameters.map((parameter) => <label key={`${parameter.id}:${parameter.value}`} className="grid gap-1 text-xs text-muted"><span>{parameter.label}</span><span className="flex"><input aria-label={parameter.label} disabled={!parameter.editable || blocked} defaultValue={parameter.value} onKeyDown={(event) => { if (event.key === "Enter") { event.preventDefault(); onEdit(parameter.id, event.currentTarget.value); } }} onBlur={(event) => { if (event.currentTarget.value !== parameter.value) onEdit(parameter.id, event.currentTarget.value); }} className="h-8 min-w-0 flex-1 rounded-l border border-border bg-canvas px-2 text-right text-sm text-foreground outline-none focus:border-accent disabled:opacity-50" /><span className="flex h-8 items-center rounded-r border border-l-0 border-border bg-raised px-2 text-xs text-muted">{parameter.unit}</span></span></label>)}</div>;
}
export function ProblemsView({ snapshot, onOpen }: { snapshot: WorkbenchSnapshot; onOpen?: (problem: WorkbenchSnapshot["problems"][number]) => void }) {
  if (!snapshot.problems.length) return <Empty icon={<AlertCircle />} title="No problems" detail="The accepted project has no reported issues." />;
  return <ul className="grid gap-2">{snapshot.problems.map((problem) => <li key={problem.id}><button type="button" disabled={!problem.file || !onOpen} onClick={() => onOpen?.(problem)} className="w-full rounded border border-danger/40 bg-danger/10 p-2 text-left outline-none enabled:hover:border-danger enabled:focus-visible:ring-1 enabled:focus-visible:ring-accent disabled:cursor-default"><p className="text-sm font-medium text-foreground">{problem.title}</p><p className="mt-1 text-xs text-muted">{problem.detail}</p>{problem.file && <p className="mt-2 font-mono text-[10px] text-accent">{problem.file}{problem.line ? `:${problem.line}${problem.column ? `:${problem.column}` : ""}` : ""}</p>}</button></li>)}</ul>;
}
function Empty({ icon, title, detail }: { icon: React.ReactNode; title: string; detail: string }) { return <div className="grid justify-items-center px-4 py-12 text-center"><span className="mb-3 text-muted [&>svg]:size-6">{icon}</span><p className="text-sm text-foreground">{title}</p><p className="mt-1 text-xs leading-relaxed text-muted">{detail}</p></div>; }
