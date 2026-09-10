// SPDX-License-Identifier: GPL-3.0-or-later
import type { AcceptedResult, EvaluationFailure } from "./index.js";
import type { CompiledManagedSource, ManagedValue, SemanticPathSegment } from "@geosolve/sketch-code/ir";
import { RetainedPointGesture, decodePointValue, type PointGestureNativeHandle, type PointGestureHandle, type PointGestureTarget, type PointGestureViewport, type PointGestureCommand, type PreparedPointGestureCommit, type PreparedPointGestureReplay } from "./point-gesture.js";
import { ConstructionPrediction, type ConstructionNativeHandle, type ConstructionTool, type ConstructionCommand, type PreparedConstruction, type PreparedConstructionCommit, type PreparedConstructionReplay } from "./construction.js";

export interface AuthoringValueWrite {
  readonly declaration: string;
  readonly path: readonly SemanticPathSegment[];
  readonly value: ManagedValue;
}
export type AuthoringAction =
  | { readonly kind: "values"; readonly writes: readonly AuthoringValueWrite[] }
  | { readonly kind: "mutation"; readonly mutation: unknown; readonly candidate_name_high_water: number }
  | { readonly kind: "extract_parameter"; readonly declaration: string; readonly path: readonly SemanticPathSegment[]; readonly presentation: { readonly label?: string; readonly description?: string; readonly isKeyParameter?: boolean } }
  | { readonly kind: "source"; readonly source: string };
export interface PreparedAuthoring {
  readonly ticket: string;
  readonly kind: "values" | "mutation" | "source";
  readonly request: {
    readonly ticket: Readonly<Record<string, unknown>> & { readonly ticketDigest: string };
    readonly current: CompiledManagedSource;
    readonly candidateSource?: string;
  };
}
export interface AuthoringReceipt {
  readonly ticketDigest: string;
  readonly baseSourceDigest: string;
  readonly candidateSourceDigest: string;
  readonly compiled: CompiledManagedSource;
}
export interface AuthoringValueChange { readonly write: AuthoringValueWrite; readonly before: ManagedValue }
export type AuthoringUpdate = { readonly status: "accepted"; readonly state: EditableSessionState; readonly valueChanges: readonly AuthoringValueChange[] } | EvaluationFailure;

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
export interface EditableNativeHandle extends Partial<PointGestureNativeHandle>, Partial<ConstructionNativeHandle> {
  openEditableSession(json: string): string;
  editableSessionState(id: string): string;
  applyEditableProject(json: string): string;
  applyEditableOverlay(json: string): string;
  undoEditable(json: string): string;
  redoEditable(json: string): string;
  exportEditableDesign(id: string): string;
  closeEditableSession(id: string): boolean;
  prepareEditableAuthoring?(json: string): string;
  applyEditableAuthoring?(json: string): string;
  releaseEditableAuthoring?(ticket: string): boolean;
  exportEditableProject?(id: string): string;
  editableSourceDesignDigest?(id: string): string;
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
  private readonly preparations = new Set<PreparedAuthoring>();
  private readonly pointPreparations = new Set<PreparedPointGestureCommit>();
  private readonly constructionPreparations = new Set<PreparedConstruction>();
  private readonly constructionCommits = new Set<PreparedConstructionCommit>();
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

  beginConstruction(tool: ConstructionTool, options: { expected: EditableSessionToken; gestureId: number; viewport: PointGestureViewport; role?: "profile" | "construction" }): ConstructionPrediction {
    const native = this.constructionNative();
    const held = decodePointValue<{ ticket: string }>(native.beginEditableConstruction(JSON.stringify({
      session: this.token.session, expected: options.expected, gesture_id: options.gestureId, tool,
      viewport: options.viewport, role: options.role ?? "profile",
    })));
    return new ConstructionPrediction(native, () => this.assertLive(), { session: this.token.session, ticket: held.ticket, gesture_id: options.gestureId });
  }
  /** Trusted server replays the gesture and prepares its own exact compiler request. */
  prepareConstruction(command: ConstructionCommand, options: { expected: EditableSessionToken }): PreparedConstruction {
    const prepared = decodePointValue<PreparedConstruction>(this.constructionNative().prepareEditableConstruction(JSON.stringify({ session: this.token.session, expected: options.expected, command })));
    this.constructionPreparations.add(prepared); return prepared;
  }
  /** Trusted host: basis must be an independently authenticated accepted historical session.
   * Host checks returned external declaration lifetimes before publishing the latest candidate.
   */
  prepareConstructionReplay(basis: EditableSession, command: ConstructionCommand, options: { expected: EditableSessionToken }): PreparedConstructionReplay {
    this.assertReplayBasis(basis);
    const native = this.constructionNative();
    if (!native.prepareEditableConstructionReplay) throw Error("This engine build does not support latest construction replay");
    const prepared = decodePointValue<PreparedConstructionReplay>(native.prepareEditableConstructionReplay(JSON.stringify({
      session: this.token.session, expected: options.expected, basis_session: basis.token.session, basis_expected: basis.token, command,
    })));
    this.constructionPreparations.add(prepared); return prepared;
  }
  /** Returns an unpublished candidate after receipt authentication and whole native parity. */
  resolveConstruction(prepared: PreparedConstruction, receipt: AuthoringReceipt): PreparedConstructionCommit {
    this.assertLive();
    if (!this.constructionPreparations.has(prepared)) throw Error("Construction preparation is foreign, released or already consumed");
    const candidate = decodePointValue<PreparedConstructionCommit>(this.constructionNative().resolveEditableConstruction(JSON.stringify({ session: this.token.session, ticket: prepared.ticket, receipt })));
    this.constructionPreparations.delete(prepared); this.constructionCommits.add(candidate); return candidate;
  }
  /** Trusted synchronous publication after the host's durable transaction completes. */
  applyConstructionCommit(prepared: PreparedConstructionCommit): EditableUpdate {
    this.assertLive();
    if (!this.constructionCommits.has(prepared)) throw Error("Construction commit is foreign, released or already consumed");
    const native = this.constructionNative();
    const result = this.update(() => native.applyEditableConstructionCommit(JSON.stringify({ session: this.token.session, ticket: prepared.ticket })));
    if (result.status === "accepted") this.constructionCommits.delete(prepared); return result;
  }
  releaseConstruction(prepared: PreparedConstruction | PreparedConstructionCommit): void {
    this.assertLive();
    if (!this.constructionPreparations.has(prepared as PreparedConstruction) && !this.constructionCommits.has(prepared as PreparedConstructionCommit)) return;
    this.constructionNative().releaseEditableConstruction(JSON.stringify({ session: this.token.session, ticket: prepared.ticket }));
    this.constructionPreparations.delete(prepared as PreparedConstruction); this.constructionCommits.delete(prepared as PreparedConstructionCommit);
  }

  pointGestureTargets(): readonly PointGestureHandle[] {
    return decodePointValue(this.pointNative().editablePointGestureTargets(String(this.token.session)));
  }
  beginPointGesture(target: PointGestureTarget, options: { expected: EditableSessionToken; gestureId: number; viewport: PointGestureViewport }): RetainedPointGesture {
    const native = this.pointNative();
    const response = decodePointValue<{ ticket: string }>(native.beginEditablePointGesture(JSON.stringify({
      session: this.token.session, expected: options.expected, target, gesture_id: options.gestureId, viewport: options.viewport,
    })));
    return new RetainedPointGesture(native, () => this.assertLive(), { session: this.token.session, ticket: response.ticket, gesture_id: options.gestureId });
  }
  /** Trusted server use: independently replay semantic input and stage complete publication.
   * The host must durably persist project/design/digest before applying this candidate.
   */
  preparePointGestureCommit(command: PointGestureCommand, options: { expected: EditableSessionToken }): PreparedPointGestureCommit {
    const prepared = decodePointValue<PreparedPointGestureCommit>(this.pointNative().prepareEditablePointCommit(JSON.stringify({
      session: this.token.session, expected: options.expected, command,
    })));
    this.pointPreparations.add(prepared); return prepared;
  }
  /** Trusted historical authentication and latest semantic lens replay; lifetimes remain host-owned. */
  preparePointGestureReplay(basis: EditableSession, command: PointGestureCommand, options: { expected: EditableSessionToken }): PreparedPointGestureReplay {
    this.assertReplayBasis(basis);
    const native = this.pointNative();
    if (!native.prepareEditablePointReplay) throw Error("This engine build does not support latest point replay");
    const prepared = decodePointValue<PreparedPointGestureReplay>(native.prepareEditablePointReplay(JSON.stringify({
      session: this.token.session, expected: options.expected, basis_session: basis.token.session, basis_expected: basis.token, command,
    })));
    this.pointPreparations.add(prepared); return prepared;
  }
  /** Synchronous install after the trusted host's durable transaction completes. */
  applyPointGestureCommit(prepared: PreparedPointGestureCommit): EditableUpdate {
    this.assertLive();
    if (!this.pointPreparations.has(prepared)) throw Error("Point commit preparation is foreign, released or already consumed");
    const native = this.pointNative();
    const result = this.update(() => native.applyEditablePointCommit(JSON.stringify({ session: this.token.session, ticket: prepared.ticket })));
    if (result.status === "accepted") this.pointPreparations.delete(prepared);
    return result;
  }
  releasePointGestureCommit(prepared: PreparedPointGestureCommit): void {
    this.assertLive();
    if (!this.pointPreparations.has(prepared)) return;
    this.pointNative().releaseEditablePointCommit(JSON.stringify({ session: this.token.session, ticket: prepared.ticket }));
    this.pointPreparations.delete(prepared);
  }

  /** Pure preparation on this session; compiler work belongs in a host worker.
   * Collaboration hosts authenticate target lifetime/ownership before calling.
   */
  prepareAuthoring(action: AuthoringAction, options: { expected: EditableSessionToken }): PreparedAuthoring {
    this.assertLive();
    if (!this.native.prepareEditableAuthoring) throw Error("This engine build does not support prepared authoring");
    const prepared = freeze(JSON.parse(this.native.prepareEditableAuthoring(JSON.stringify({ session: this.token.session, expected: options.expected, action })))) as PreparedAuthoring;
    this.preparations.add(prepared); return prepared;
  }
  /** Native owner checks exact receipts and hard residuals. In a client worker,
   * this remains provisional; the server recomputes from its own accepted state.
   */
  applyAuthoring(prepared: PreparedAuthoring, receipt: AuthoringReceipt): AuthoringUpdate {
    this.assertLive();
    if (!this.preparations.has(prepared)) throw Error("Authoring preparation is foreign, released or already consumed");
    if (!this.native.applyEditableAuthoring) throw Error("This engine build does not support prepared authoring");
    try {
      const result = JSON.parse(this.native.applyEditableAuthoring(JSON.stringify({ session: this.token.session, ticket: prepared.ticket, receipt })));
      const state = this.admit(JSON.stringify(result.state));
      this.current = state; this.preparations.delete(prepared);
      return freeze({ status: "accepted", state, valueChanges: result.valueChanges }) as AuthoringUpdate;
    } catch (error) { return freeze({ status: "rejected", diagnostics: [{ code: "authoring", detail: String(error) }] }) as EvaluationFailure; }
  }
  releaseAuthoring(prepared: PreparedAuthoring): void {
    this.assertLive(); if (this.preparations.delete(prepared)) this.native.releaseEditableAuthoring?.(prepared.ticket);
  }
  exportProject(): string {
    this.assertLive(); if (!this.native.exportEditableProject) throw Error("This engine build does not support accepted project export");
    return this.native.exportEditableProject(String(this.token.session));
  }
  sourceDesignDigest(): string {
    this.assertLive(); if (!this.native.editableSourceDesignDigest) throw Error("This engine build does not support source/design identity");
    return this.native.editableSourceDesignDigest(String(this.token.session));
  }

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
    this.preparations.clear();
    this.pointPreparations.clear();
    this.constructionPreparations.clear(); this.constructionCommits.clear();
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
  private assertReplayBasis(basis: EditableSession): void {
    this.assertLive(); basis.assertLive();
    if (this.native !== basis.native) throw Error("Replay basis belongs to a different engine");
  }
  private pointNative(): PointGestureNativeHandle {
    this.assertLive();
    for (const name of ["editablePointGestureTargets", "beginEditablePointGesture", "advanceEditablePointGesture", "editablePointGestureScene", "editablePointGesturePresentation", "finishEditablePointGesture", "cancelEditablePointGesture", "prepareEditablePointCommit", "applyEditablePointCommit", "releaseEditablePointCommit"] as const) {
      if (typeof this.native[name] !== "function") throw Error("This engine build does not support retained point gestures");
    }
    return this.native as PointGestureNativeHandle;
  }
  private constructionNative(): ConstructionNativeHandle {
    this.assertLive();
    for (const name of ["beginEditableConstruction", "advanceEditableConstruction", "editableConstructionScene", "editableConstructionPresentation", "finishEditableConstruction", "cancelEditableConstruction", "prepareEditableConstruction", "resolveEditableConstruction", "applyEditableConstructionCommit", "releaseEditableConstruction"] as const) {
      if (typeof this.native[name] !== "function") throw Error("This engine build does not support construction prediction");
    }
    return this.native as ConstructionNativeHandle;
  }
}
function encode(value: unknown): string { return typeof value === "string" ? value : JSON.stringify(value); }
function freeze(value: unknown): unknown {
  if (value !== null && typeof value === "object" && !Object.isFrozen(value)) {
    Object.values(value).forEach(freeze); Object.freeze(value);
  }
  return value;
}
