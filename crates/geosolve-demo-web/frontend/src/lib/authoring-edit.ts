// SPDX-License-Identifier: GPL-3.0-or-later
import { createContext, useContext, useEffect, useId } from "react";

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
    change: (value: string) => lifecycle?.changeFieldEdit(id, label, value),
    commit: (action: () => void) => lifecycle ? lifecycle.commitFieldEdit(id, action) : action(),
    cancel: () => lifecycle?.cancelFieldEdit(id),
  };
}
