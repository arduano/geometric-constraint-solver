// SPDX-License-Identifier: GPL-3.0-or-later
import { useAuthoringEdit } from "../lib/authoring-edit";
import { useEffect, useId, useRef, useState } from "react";
import type { AuthoringDocumentSnapshot, AuthoringMetadataChanges, AuthoringMetadataCommand, AuthoringMetadataSnapshot, AuthoringParameterExtractionCommand } from "../lib/adapter";

export interface AuthoringMetadataActions {
  onEdit: (command: AuthoringMetadataCommand) => void;
  onExtract: (command: AuthoringParameterExtractionCommand) => void;
}

const fieldClass = "min-w-0 rounded border border-border bg-canvas px-2 py-1.5 text-xs text-foreground outline-none focus:border-accent disabled:opacity-40";
const linkClass = "rounded text-left text-[11px] text-accent outline-none hover:underline focus-visible:ring-1 focus-visible:ring-accent disabled:opacity-40";

/** Drafts stay local; only native authenticated metadata commands change source. */
export function AuthoringMetadataEditor({ metadata, label, actions, blockedReason, extractionId }: {
  metadata?: AuthoringMetadataSnapshot;
  label: string;
  actions?: AuthoringMetadataActions;
  blockedReason?: string;
  extractionId?: string;
}) {
  const helpId = useId();
  if (!metadata) return null;
  const reason = blockedReason ?? (!metadata.editable ? metadata.reason ?? "Edit this presentation in its source declaration." : !actions ? "Source editing is unavailable." : undefined);
  const keyProperty = metadata.target.kind === "dimension" ? "isKeyConstraint" : metadata.target.kind === "parameter" ? "isKeyParameter" : undefined;
  const edit = (changes: AuthoringMetadataChanges) => {
    if (!reason) actions?.onEdit({ authority: metadata.authority, target: metadata.target, changes });
  };
  return <div className="mt-2 grid min-w-0 gap-2">
    {metadata.description && <p className="whitespace-pre-wrap break-words text-[11px] leading-relaxed text-muted">{metadata.description}</p>}
    {keyProperty && <div>
      <label className="flex items-start gap-2 text-[11px] text-muted" title={reason}>
        <input type="checkbox" aria-label={`Show ${label} in overview`} aria-describedby={helpId} checked={Boolean(metadata[keyProperty])} disabled={Boolean(reason)} onChange={(event) => edit({ [keyProperty]: event.currentTarget.checked })} className="mt-0.5 accent-accent" />
        <span>Show in overview</span>
      </label>
      <span id={helpId} className="sr-only">Keep this measurement available without selecting its geometry.</span>
      {metadata.hasKeyOverride && <button type="button" aria-label={`Reset ${label} to default`} disabled={Boolean(reason)} title={reason ?? (keyProperty === "isKeyConstraint" ? "Use the document's overview default" : "Use this parameter's overview default")} onClick={() => edit({ [keyProperty]: null })} className={`ml-5 mt-1 ${linkClass}`}>Reset to default</button>}
    </div>}
    {metadata.canExtract && extractionId && <button type="button" aria-label={`Make ${label} a named parameter`} disabled={Boolean(blockedReason) || !actions} title={blockedReason ?? "Give this value its own name, description and overview setting."} onClick={() => {
      if (!blockedReason) actions?.onExtract({
        authority: metadata.authority,
        id: extractionId,
        label: metadata.label || label,
        ...(metadata.description === undefined ? {} : { description: metadata.description }),
        ...(metadata.isKeyParameter === undefined ? {} : { isKeyParameter: metadata.isKeyParameter }),
      });
    }} className={linkClass}>Make named parameter</button>}
    <details className="min-w-0">
      <summary aria-label={`Edit ${label} name and description`} className="cursor-pointer rounded text-[11px] text-muted outline-none hover:text-foreground focus-visible:ring-1 focus-visible:ring-accent">Name and description</summary>
      <div className="mt-2 grid min-w-0 gap-2">
        <MetadataTextField label="Name" accessibleLabel={`${label} name`} value={metadata.label ?? ""} placeholder={label} blockedReason={reason} onCommit={(value) => edit({ label: value || null })} />
        <MetadataTextField label="Description" accessibleLabel={`${label} description`} value={metadata.description ?? ""} multiline blockedReason={reason} onCommit={(value) => edit({ description: value || null })} />
      </div>
    </details>
    {!metadata.editable && metadata.reason && <p className="text-[10px] leading-relaxed text-muted">{metadata.reason}</p>}
  </div>;
}

export function AuthoringDocumentProperties({ document, actions, blockedReason }: { document?: AuthoringDocumentSnapshot; actions?: AuthoringMetadataActions; blockedReason?: string }) {
  if (!document) return null;
  const reason = blockedReason ?? (!document.editable ? document.reason ?? "Document properties are read only." : !actions ? "Source editing is unavailable." : undefined);
  const edit = (changes: AuthoringMetadataChanges) => {
    if (!reason) actions?.onEdit({ authority: document.authority, target: { kind: "document" }, changes });
  };
  return <details className="mt-5 min-w-0 border-t border-border pt-3">
    <summary className="cursor-pointer rounded text-xs font-semibold text-foreground outline-none focus-visible:ring-1 focus-visible:ring-accent">Document properties</summary>
    <div className="mt-3 grid min-w-0 gap-3">
      <MetadataTextField label="Title" accessibleLabel="Document title" value={document.title} placeholder="Untitled code sketch" blockedReason={reason} onCommit={(value) => edit({ title: value || null })} />
      <MetadataTextField label="Description" accessibleLabel="Document description" value={document.description} multiline blockedReason={reason} onCommit={(value) => edit({ description: value || null })} />
      <label className="flex items-start gap-2 text-[11px] leading-relaxed text-muted" title={reason}>
        <input type="checkbox" checked={document.areKeyConstraintsByDefault} disabled={Boolean(reason)} onChange={(event) => edit({ areKeyConstraintsByDefault: event.currentTarget.checked })} className="mt-0.5 accent-accent" />
        <span>Show authored dimensions in overview by default</span>
      </label>
      <p className="text-[10px] leading-relaxed text-muted">Individual dimensions can override this setting. Generated dimensions and public parameters keep their own settings.</p>
      {reason && <p role="status" className="text-[11px] text-muted">{reason}</p>}
    </div>
  </details>;
}

function MetadataTextField({ label, accessibleLabel, value, placeholder, multiline, blockedReason, onCommit }: { label: string; accessibleLabel: string; value: string; placeholder?: string; multiline?: boolean; blockedReason?: string; onCommit: (value: string) => void }) {
  const editing = useAuthoringEdit(accessibleLabel);
  const [draft, setDraft] = useState(value);
  const submitted = useRef(value);
  useEffect(() => { editing.cancel(); setDraft(value); submitted.current = value; }, [value]);
  const commit = (next: string) => {
    if (next === value) editing.cancel();
    if (blockedReason || next === submitted.current) return;
    submitted.current = next;
    editing.commit(() => onCommit(next));
  };
  const properties = {
    "aria-label": accessibleLabel,
    value: draft,
    placeholder,
    disabled: Boolean(blockedReason),
    title: blockedReason,
    className: fieldClass,
    onFocus: editing.begin,
    onChange: (event: React.ChangeEvent<HTMLInputElement | HTMLTextAreaElement>) => { editing.change(event.currentTarget.value); setDraft(event.currentTarget.value); },
    onBlur: (event: React.FocusEvent<HTMLInputElement | HTMLTextAreaElement>) => commit(event.currentTarget.value),
    onKeyDown: (event: React.KeyboardEvent<HTMLInputElement | HTMLTextAreaElement>) => {
      if (event.key === "Enter" && (!multiline || event.ctrlKey || event.metaKey)) { event.preventDefault(); event.stopPropagation(); commit(event.currentTarget.value); }
      if (event.key === "Escape") { event.preventDefault(); event.stopPropagation(); editing.cancel(); setDraft(value); submitted.current = value; }
    },
  };
  return <label className="grid min-w-0 gap-1 text-[11px] text-muted"><span>{label}</span>{multiline ? <textarea {...properties} rows={3} /> : <input {...properties} />}</label>;
}
