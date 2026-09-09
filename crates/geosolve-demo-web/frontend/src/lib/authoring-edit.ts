// SPDX-License-Identifier: GPL-3.0-or-later
import { createContext, useContext, useEffect, useId, useRef, useState } from "react";

/** Explicit authoring fields only: search/filter/navigation inputs never open an edit. */
export interface AuthoringEditLifecycle {
  beginFieldEdit(id: string, label: string): void;
  changeFieldEdit(id: string, label: string, value: string): void;
  commitFieldEdit(id: string, action: () => void): void;
  cancelFieldEdit(id: string): void;
}
export const AuthoringEditContext = createContext<AuthoringEditLifecycle | undefined>(undefined);
export function useAuthoringEdit(label: string) {
  const lifecycle = useContext(AuthoringEditContext);
  const id = useId();
  useEffect(() => () => lifecycle?.cancelFieldEdit(id), [id, lifecycle]);
  return {
    begin: () => lifecycle?.beginFieldEdit(id, label),
    focus: (inspect?: () => void) => lifecycle ? lifecycle.beginFieldEdit(id, label) : inspect?.(),
    change: (value: string) => lifecycle?.changeFieldEdit(id, label, value),
    commit: (action: () => void) => lifecycle ? lifecycle.commitFieldEdit(id, action) : action(),
    cancel: () => lifecycle?.cancelFieldEdit(id),
  };
}

/** Field text survives a delayed echo of an older submission. Reconciliation is
 * shared by dimensions, parameters and metadata, including multiline text. */
export function useAuthoringField(label: string, acceptedValue: string) {
  const editing = useAuthoringEdit(label);
  const [value, setValue] = useState(acceptedValue);
  const current = useRef({ value: acceptedValue, version: 0, dirty: false });
  const submitted = useRef<{ value: string; version: number } | undefined>(undefined);
  useEffect(() => {
    const pending = current.current;
    if (pending.dirty && (!submitted.current || pending.version > submitted.current.version)) return;
    editing.cancel();
    current.current = { value: acceptedValue, version: pending.version, dirty: false };
    submitted.current = undefined;
    setValue(acceptedValue);
  }, [acceptedValue]);
  return {
    value,
    focus: editing.focus,
    change(next: string) {
      current.current = { value: next, version: current.current.version + 1, dirty: next !== acceptedValue };
      editing.change(next); setValue(next);
    },
    cancel() {
      editing.cancel();
      current.current = { value: acceptedValue, version: current.current.version + 1, dirty: false };
      submitted.current = undefined; setValue(acceptedValue);
    },
    submit(next: string, action: () => void, blocked = false) {
      if (next === acceptedValue) { editing.cancel(); return; }
      if (blocked || (next === submitted.current?.value && current.current.version === submitted.current.version)) return;
      submitted.current = { value: next, version: current.current.version };
      editing.commit(action);
    },
  };
}
