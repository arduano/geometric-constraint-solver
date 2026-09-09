// SPDX-License-Identifier: GPL-3.0-or-later
import type { AcceptedResult, EvaluationFailure } from "./index.js";

export interface EditableSessionToken {
  readonly session: number;
  readonly revision: number;
  readonly digest: string;
}
export interface EditableSessionState {
  readonly token: EditableSessionToken;
  readonly can_undo: boolean;
  readonly can_redo: boolean;
  readonly result: AcceptedResult;
}
export interface EditableDesign {
  readonly format: "geosolve-design-v1";
  readonly project: string;
  readonly generated: unknown;
  readonly overrides: unknown;
}
export interface EditableNativeHandle {
  openEditableSession(json: string): string;
  editableSessionState(id: string): string;
  applyEditableProject(json: string): string;
  applyEditableOverlay(json: string): string;
  undoEditable(json: string): string;
  redoEditable(json: string): string;
  exportEditableDesign(id: string): string;
  closeEditableSession(id: string): boolean;
}
/** Internal engine integration: results join the same immutable export/lifetime table. */
export interface EditableSessionHost {
  readonly native: Partial<EditableNativeHandle>;
  isLive(): boolean;
  accept(result: AcceptedResult): AcceptedResult;
}
export type EditableUpdate = { readonly status: "accepted"; readonly state: EditableSessionState } | EvaluationFailure;

/** Explicit source revision ownership; long native work belongs in a host worker. */
export class EditableSession {
  private disposed = false;
  private current: EditableSessionState;
  private readonly native: EditableNativeHandle;
  constructor(private readonly host: EditableSessionHost, project: unknown, options: { design?: EditableDesign | string } = {}) {
    if (!host.isLive()) throw Error("Engine has been disposed");
    const native = host.native;
    for (const name of ["openEditableSession", "editableSessionState", "applyEditableProject", "applyEditableOverlay", "undoEditable", "redoEditable", "exportEditableDesign", "closeEditableSession"] as const) {
      if (typeof native[name] !== "function") throw Error("This engine build does not support editable sessions");
    }
    this.native = native as EditableNativeHandle;
    this.current = this.admit(this.native.openEditableSession(JSON.stringify({
      project: encode(project), design: options.design === undefined ? null : encode(options.design),
    })));
  }
  get state(): EditableSessionState { return this.current; }
  get token(): EditableSessionToken { return this.current.token; }
  get accepted(): AcceptedResult { return this.current.result; }

  async applyProject(project: unknown, options: { expected: EditableSessionToken }): Promise<EditableUpdate> {
    return this.update(() => this.native.applyEditableProject(JSON.stringify({ session: this.token.session, expected: options.expected, project: encode(project) })));
  }
  async applyOverlay(overrides: unknown, options: { expected: EditableSessionToken }): Promise<EditableUpdate> {
    return this.update(() => this.native.applyEditableOverlay(JSON.stringify({ session: this.token.session, expected: options.expected, overlay: overrides })));
  }
  async undo(options: { expected: EditableSessionToken }): Promise<EditableUpdate> {
    return this.update(() => this.native.undoEditable(JSON.stringify({ session: this.token.session, expected: options.expected })));
  }
  async redo(options: { expected: EditableSessionToken }): Promise<EditableUpdate> {
    return this.update(() => this.native.redoEditable(JSON.stringify({ session: this.token.session, expected: options.expected })));
  }
  exportDesign(): EditableDesign {
    this.assertLive();
    return freeze(JSON.parse(this.native.exportEditableDesign(String(this.token.session)))) as EditableDesign;
  }
  dispose(): void {
    if (this.disposed) return;
    this.disposed = true;
    if (this.host.isLive()) this.native.closeEditableSession(String(this.token.session));
  }
  private update(action: () => string): EditableUpdate {
    this.assertLive();
    try {
      const next = this.admit(action());
      this.current = next;
      return freeze({ status: "accepted", state: next }) as EditableUpdate;
    } catch (error) {
      return freeze({ status: "rejected", diagnostics: [{ code: "editable-session", detail: String(error) }] }) as EvaluationFailure;
    }
  }
  private admit(json: string): EditableSessionState {
    const state = JSON.parse(json) as EditableSessionState;
    if (!Number.isSafeInteger(state.token?.session) || !Number.isSafeInteger(state.token?.revision)
      || state.token.session < 1 || state.token.revision < 0 || typeof state.token.digest !== "string") {
      throw Error("Native editable state has invalid revision authority");
    }
    const result = this.host.accept(state.result);
    return freeze({ ...state, result }) as EditableSessionState;
  }
  private assertLive(): void {
    if (this.disposed) throw Error("Editable session has been disposed");
    if (!this.host.isLive()) throw Error("Engine has been disposed");
  }
}
function encode(value: unknown): string { return typeof value === "string" ? value : JSON.stringify(value); }
function freeze(value: unknown): unknown {
  if (value !== null && typeof value === "object" && !Object.isFrozen(value)) {
    Object.values(value).forEach(freeze); Object.freeze(value);
  }
  return value;
}
