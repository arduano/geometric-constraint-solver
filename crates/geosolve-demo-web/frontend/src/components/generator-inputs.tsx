// SPDX-License-Identifier: GPL-3.0-or-later
import { useId, useRef, useState } from "react";
import { useAuthoringField } from "../lib/authoring-edit";

/** Discoverable source inputs; this mirrors the SDK's serializable GeneratorInput. */
export type GeneratorInputDefinition = {
  readonly label?: string;
  readonly description?: string;
} & (
  | { readonly type: "number" | "integer"; readonly default: number; readonly min?: number; readonly max?: number; readonly unit?: "mm" | "cm" | "m" | "inch" | "deg" | "rad" }
  | { readonly type: "boolean"; readonly default: boolean }
  | { readonly type: "string"; readonly default: string }
  | { readonly type: "choice"; readonly default: string; readonly choices: readonly string[] }
);

const controlClass = "min-w-0 w-full rounded border border-border bg-canvas px-2 py-1.5 text-xs text-foreground outline-none focus:border-accent disabled:opacity-40";
const buttonClass = "rounded border border-border px-2 py-1.5 text-[11px] text-foreground outline-none hover:bg-raised focus-visible:ring-1 focus-visible:ring-accent disabled:opacity-40";

export function GeneratorInputs({ definitions, values, disabledReason, onApply }: {
  definitions: Record<string, GeneratorInputDefinition>;
  values: Record<string, unknown>;
  disabledReason?: string;
  onApply: (values: Record<string, unknown>) => Promise<void>;
}) {
  const accepted = { ...Object.fromEntries(Object.entries(definitions).map(([name, definition]) => [name, definition.default])), ...values };
  return <section aria-label="Generator inputs" className="grid min-w-0 gap-3">
    <div>
      <h3 className="text-xs font-semibold text-foreground">Generator inputs</h3>
      <p className="mt-1 text-[11px] leading-relaxed text-muted">Apply an input to regenerate the design. The last accepted design stays visible while it runs.</p>
    </div>
    {Object.entries(definitions).map(([name, definition]) => <GeneratorInputField key={name} name={name} definition={definition} accepted={accepted[name]} disabledReason={disabledReason} onApply={(value) => onApply({ ...accepted, [name]: value })} />)}
    {disabledReason && <p role="status" className="text-[11px] leading-relaxed text-muted">{disabledReason}</p>}
  </section>;
}

function GeneratorInputField({ name, definition, accepted, disabledReason, onApply }: {
  name: string;
  definition: GeneratorInputDefinition;
  accepted: unknown;
  disabledReason?: string;
  onApply: (value: string | boolean | number) => Promise<void>;
}) {
  const id = useId();
  const label = definition.label ?? name;
  const field = useAuthoringField(label, String(accepted));
  const latestField = useRef(field);
  latestField.current = field;
  const version = useRef(0);
  const [error, setError] = useState<string>();
  const [pending, setPending] = useState(false);
  const numeric = definition.type === "number" || definition.type === "integer";
  const changed = field.value !== String(accepted);
  const change = (value: string) => { ++version.current; field.change(value); setError(undefined); };
  const cancel = () => { ++version.current; field.cancel(); setError(undefined); };
  const apply = () => {
    if (disabledReason || pending) return;
    let parsed: string | boolean | number;
    try { parsed = parseInput(definition, field.value); }
    catch (cause) { setError(errorText(cause)); return; }
    if (Object.is(parsed, accepted)) { cancel(); return; }
    const submittedVersion = version.current;
    const rejected = (cause: unknown) => {
      if (version.current === submittedVersion) {
        // Start another draft version so the same text can be retried after a
        // rejected request without releasing its original folder authority.
        latestField.current.change(latestField.current.value);
        setError(errorText(cause));
      }
    };
    field.submit(String(parsed), () => {
      setPending(true); setError(undefined);
      // Invoke inside commitFieldEdit: the folder captures its CAS basis here.
      try { void onApply(parsed).catch(rejected).finally(() => setPending(false)); }
      catch (cause) { rejected(cause); setPending(false); }
    });
  };
  const properties = {
    id,
    "aria-label": label,
    "aria-describedby": `${id}-help${error ? ` ${id}-error` : ""}`,
    "aria-invalid": Boolean(error),
    disabled: Boolean(disabledReason),
    title: disabledReason,
    onFocus: () => field.focus(),
    onBlur: () => { if (!changed && !pending) field.cancel(); },
    onKeyDown: (event: React.KeyboardEvent<HTMLInputElement | HTMLSelectElement>) => {
      if (event.key === "Enter") { event.preventDefault(); event.stopPropagation(); apply(); }
      if (event.key === "Escape") { event.preventDefault(); event.stopPropagation(); cancel(); }
    },
  };
  return <div className="grid min-w-0 gap-1.5 rounded border border-border p-2.5">
    <label htmlFor={id} className="text-[11px] font-medium text-foreground">{label}</label>
    <div className="flex min-w-0 items-center gap-2">
      {definition.type === "boolean" ? <input {...properties} type="checkbox" checked={field.value === "true"} onChange={(event) => change(String(event.currentTarget.checked))} className="accent-accent" />
        : definition.type === "choice" ? <select {...properties} value={field.value} onChange={(event) => change(event.currentTarget.value)} className={controlClass}>{definition.choices.map((choice) => <option key={choice} value={choice}>{choice}</option>)}</select>
          : <input {...properties} type="text" inputMode={numeric ? "decimal" : undefined} value={field.value} onChange={(event) => change(event.currentTarget.value)} className={controlClass} />}
      {numeric && definition.unit && <span className="text-[11px] text-muted">{definition.unit}</span>}
      <button type="button" aria-label={`Apply ${label}`} disabled={Boolean(disabledReason) || pending || !changed} onClick={apply} className={buttonClass}>{pending ? "Applying…" : "Apply"}</button>
      {changed && <button type="button" aria-label={`Revert ${label}`} onClick={cancel} className={buttonClass}>Revert</button>}
    </div>
    <p id={`${id}-help`} className="whitespace-pre-wrap text-[10px] leading-relaxed text-muted">{definition.description}{numeric && <>{definition.description ? " " : ""}{definition.type === "integer" ? "Whole number" : "Number"}{definition.min !== undefined ? `, minimum ${definition.min}` : ""}{definition.max !== undefined ? `, maximum ${definition.max}` : ""}.</>}</p>
    {error && <p id={`${id}-error`} role="alert" className="text-[11px] leading-relaxed text-danger">{error}</p>}
  </div>;
}

function parseInput(definition: GeneratorInputDefinition, text: string): string | number | boolean {
  switch (definition.type) {
    case "number":
    case "integer": {
      const value = Number(text);
      if (!text.trim() || !Number.isFinite(value)) throw new Error("Enter a finite number.");
      if (definition.type === "integer" && !Number.isSafeInteger(value)) throw new Error("Enter a whole number within the safe integer range.");
      if (definition.min !== undefined && value < definition.min) throw new Error(`Enter a value of at least ${definition.min}.`);
      if (definition.max !== undefined && value > definition.max) throw new Error(`Enter a value of at most ${definition.max}.`);
      return value;
    }
    case "boolean": return text === "true";
    case "choice":
      if (!definition.choices.includes(text)) throw new Error("Choose one of the available options.");
      return text;
    case "string": return text;
  }
}

function errorText(cause: unknown): string { return cause instanceof Error ? cause.message : String(cause); }
