// SPDX-License-Identifier: GPL-3.0-or-later
import { useAuthoringField } from "../lib/authoring-edit";
import { Pin, PinOff } from "lucide-react";
import { useEffect, useState } from "react";
import type { DimensionEntry, DimensionsSnapshot } from "../lib/adapter";
import { AuthoringMetadataEditor, type AuthoringMetadataActions } from "./authoring-metadata";

export interface DimensionPanelActions {
  onFocus: (id: string) => void;
  onPin: (id: string, pinned: boolean) => void;
  onClearPins: () => void;
  onEdit: (id: string, value: string) => void;
}

export function DimensionInspector({ dimensions, actions, onParameterEdit, editingBlocked, inspectionBlocked, metadataActions }: {
  dimensions?: DimensionsSnapshot;
  actions: DimensionPanelActions;
  onParameterEdit: (id: string, value: string) => void;
  editingBlocked?: string;
  inspectionBlocked?: string;
  metadataActions?: AuthoringMetadataActions;
}) {
  const entries = dimensions?.entries ?? [];
  const parameters = dimensions?.parameters ?? [];
  const mode = dimensions?.mode ?? "focused";
  const pinCount = dimensions?.pinCount ?? 0;
  const visible = entries.filter((entry) => entry.visible).length;
  const primary = entries.filter((entry) => !entry.generated || entry.defaultPriority);
  const generated = entries.filter((entry) => entry.generated && !entry.defaultPriority);
  const hasGeneratedFocus = generated.some((entry) => entry.pinned || entry.focused);
  const [generatedOpen, setGeneratedOpen] = useState(hasGeneratedFocus);
  // Keep pins reachable on restoration without moving their DOM rows, focus or
  // the user's scroll position. Unpinned generated detail starts collapsed.
  useEffect(() => { if (hasGeneratedFocus) setGeneratedOpen(true); }, [hasGeneratedFocus]);
  const renderEntries = (rows: DimensionEntry[]) => <ul className="grid min-w-0 gap-2">{rows.map((entry) => <DimensionRow key={entry.rowKey ?? entry.id} entry={entry} actions={actions} pinCount={pinCount} editingBlocked={editingBlocked ?? inspectionBlocked} inspectionBlocked={inspectionBlocked} metadataActions={metadataActions} />)}</ul>;
  return <section aria-label="Dimensions" className="mt-5 min-w-0 border-t border-border pt-3">
    <div className="flex items-center justify-between gap-2"><h3 className="text-xs font-semibold text-foreground">Dimensions</h3><span className="text-[10px] tabular-nums text-muted">{pinCount}/4 pinned</span></div>
    <p className="mt-1 text-[11px] leading-relaxed text-muted">{mode === "hidden" ? "Canvas dimensions are hidden. Choose a measurement to inspect it." : entries.length ? `${visible} of ${entries.length} measurements shown on canvas.` : "Select geometry or pause over it to see measurements. Pin a dimension to keep it in view."}</p>
    {pinCount > 0 && <button type="button" onClick={actions.onClearPins} disabled={Boolean(inspectionBlocked)} title={inspectionBlocked} className="mt-1 rounded text-[11px] text-accent outline-none hover:underline focus-visible:ring-1 focus-visible:ring-accent disabled:opacity-40">Clear pins</button>}
    {parameters.length > 0 && <div className="mt-3 grid gap-2" role="group" aria-label="Dimensional parameters"><h4 className="text-[11px] font-semibold text-muted">Parameters</h4>{parameters.map((parameter) => <div key={parameter.rowKey ?? parameter.id} className="rounded border border-border bg-raised/40 p-2"><p className="mb-1.5 truncate text-xs font-medium text-foreground" title={parameter.label}>{parameter.label}</p>{parameter.defaultPriority && !parameter.metadata && <p className="mb-1.5 text-[10px] text-muted">Key dimension</p>}<DimensionValue label={parameter.label} value={parameter.value} unit={parameter.unit} editable={parameter.editable} blockedReason={editingBlocked ?? inspectionBlocked} onEdit={(value) => onParameterEdit(parameter.id, value)} /><AuthoringMetadataEditor metadata={parameter.metadata} label={parameter.label} actions={metadataActions} blockedReason={editingBlocked ?? inspectionBlocked} extractionId={parameter.id} />{parameter.consumers && parameter.consumers.length > 0 && <p className="mt-2 break-words text-[10px] leading-relaxed text-muted">Used by {parameter.consumers.join(", ")}</p>}</div>)}</div>}
    {primary.length > 0 && <div className="mt-3">{renderEntries(primary)}</div>}
    {generated.length > 0 && <details className="mt-3" open={generatedOpen} onToggle={(event) => setGeneratedOpen(event.currentTarget.open)}><summary className="cursor-pointer rounded py-1 text-[11px] text-muted outline-none hover:text-foreground focus-visible:ring-1 focus-visible:ring-accent">Generated dimensions <span className="tabular-nums">({generated.length})</span></summary><div className="mt-2">{renderEntries(generated)}</div></details>}
    {dimensions?.allMeasurements && dimensions.allMeasurements.length > 0 && <details className="mt-3">
      <summary className="cursor-pointer rounded py-1 text-[11px] text-muted outline-none hover:text-foreground focus-visible:ring-1 focus-visible:ring-accent">All measurements <span className="tabular-nums">({dimensions.allMeasurements.length})</span></summary>
      {!entries.some((entry) => entry.defaultPriority) && <p className="my-2 text-[11px] leading-relaxed text-muted">Choose a measurement, then enable Show in overview to keep it available.</p>}
      <ul className="mt-2 grid min-w-0 gap-1">{dimensions.allMeasurements.map((entry) => <li key={entry.rowKey ?? entry.id}><button type="button" aria-label={`Show details for ${entry.label}`} aria-pressed={entry.focused} disabled={Boolean(inspectionBlocked)} title={inspectionBlocked} onClick={() => actions.onFocus(entry.id)} className="flex w-full min-w-0 items-start justify-between gap-2 rounded px-2 py-1.5 text-left text-[11px] outline-none hover:bg-raised focus-visible:ring-1 focus-visible:ring-accent disabled:opacity-40"><span className="min-w-0 break-words text-foreground">{entry.label}{entry.generated && <span className="block text-[10px] text-muted">Generated</span>}</span><span className="shrink-0 tabular-nums text-muted">{entry.value}{entry.unit ? ` ${entry.unit}` : ""}</span></button></li>)}</ul>
    </details>}
  </section>;
}

function DimensionRow({ entry, actions, pinCount, editingBlocked, inspectionBlocked, metadataActions }: { entry: DimensionEntry; actions: DimensionPanelActions; pinCount: number; editingBlocked?: string; inspectionBlocked?: string; metadataActions?: AuthoringMetadataActions }) {
  const pinBlocked = inspectionBlocked ?? (!entry.pinned && pinCount >= 4 ? "Unpin a dimension before adding another. Four pins keep the canvas readable." : undefined);
  return <li className={`min-w-0 rounded border p-2 ${entry.focused ? "border-amber-400/50 bg-amber-400/5" : "border-border"}`}>
    <div className="flex min-w-0 items-start gap-1">
      <button type="button" aria-label={`Inspect ${entry.label}`} aria-pressed={entry.focused} disabled={Boolean(inspectionBlocked)} title={inspectionBlocked ?? `${entry.label} · ${entry.kind}${entry.reference ? " · Reference" : ""}`} onClick={() => actions.onFocus(entry.id)} className="min-w-0 flex-1 rounded text-left outline-none focus-visible:ring-1 focus-visible:ring-accent disabled:opacity-40">
        <span className="block truncate text-xs font-medium text-foreground hover:text-accent">{entry.label}</span>
        <span className="mt-0.5 block text-[10px] text-muted">{entry.defaultPriority && "Key dimension · "}{entry.reference ? "Reference" : entry.editable ? "Editable" : "Read only"} · {entry.visible ? "On canvas" : "In Inspector"}</span>
      </button>
      <button type="button" aria-label={`${entry.pinned ? "Unpin" : "Pin"} ${entry.label}`} aria-pressed={entry.pinned} disabled={Boolean(pinBlocked)} title={pinBlocked ?? (entry.pinned ? "Remove this pin" : "Keep this dimension visible when selection changes")} onClick={() => actions.onPin(entry.id, !entry.pinned)} className={`grid size-6 shrink-0 place-items-center rounded outline-none hover:bg-raised focus-visible:ring-1 focus-visible:ring-accent disabled:opacity-30 ${entry.pinned ? "text-accent" : "text-muted"}`}>{entry.pinned ? <PinOff className="size-3.5" /> : <Pin className="size-3.5" />}</button>
    </div>
    <div className="mt-2"><DimensionValue label={entry.label} value={entry.value} unit={entry.unit} editable={entry.editable} reference={entry.reference} blockedReason={editingBlocked ?? entry.reason} onFocus={inspectionBlocked ? undefined : () => actions.onFocus(entry.id)} onEdit={(value) => actions.onEdit(entry.id, value)} /></div>
    {entry.reason && <p className="mt-1 text-[10px] leading-relaxed text-muted">{entry.reason}</p>}
    <AuthoringMetadataEditor metadata={entry.metadata} label={entry.label} actions={metadataActions} blockedReason={editingBlocked ?? inspectionBlocked} />
  </li>;
}

function DimensionValue({ label, value, unit, editable, reference, blockedReason, onFocus, onEdit }: { label: string; value: string; unit?: string; editable: boolean; reference?: boolean; blockedReason?: string; onFocus?: () => void; onEdit: (value: string) => void }) {
  const editing = useAuthoringField(`${label} value`, value);
  const submit = (next: string) => editing.submit(next, () => onEdit(next), !editable || Boolean(blockedReason));
  if (!editable) return <p aria-label={`${label} value`} className="text-right text-sm tabular-nums text-foreground">{reference ? `(${value}${unit ? ` ${unit}` : ""})` : `${value}${unit ? ` ${unit}` : ""}`}</p>;
  return <span className="flex min-w-0">
    <input aria-label={`${label} value`} value={editing.value} disabled={Boolean(blockedReason)} title={blockedReason} onFocus={() => editing.focus(onFocus)} onChange={(event) => editing.change(event.target.value)} onBlur={(event) => submit(event.currentTarget.value)} onKeyDown={(event) => {
      if (event.key === "Enter") { event.preventDefault(); event.stopPropagation(); submit(event.currentTarget.value); }
      if (event.key === "Escape") { event.preventDefault(); event.stopPropagation(); editing.cancel(); }
    }} className="h-7 min-w-0 flex-1 rounded-l border border-border bg-canvas px-2 text-right text-xs tabular-nums text-foreground outline-none focus:border-accent disabled:opacity-40" />
    <span className="flex h-7 items-center rounded-r border border-l-0 border-border bg-raised px-2 text-[10px] text-muted">{unit}</span>
  </span>;
}
