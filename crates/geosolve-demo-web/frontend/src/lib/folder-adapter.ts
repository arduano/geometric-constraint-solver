// SPDX-License-Identifier: GPL-3.0-or-later
import { assertWorkbenchSnapshot, getCanvasSnapshotSequence, stampCanvasSnapshot, type WorkbenchAdapter, type WorkbenchSnapshot, type PointerSample, type WheelSample, type SourceFileSnapshot, type DeclarationRow, type AuthoringMetadataSnapshot } from "./adapter";
import type { GeneratorInputDefinition } from "../components/generator-inputs";
import type { ToolCatalog } from "./tool-catalog";
import { WorkbenchActivity } from "./workbench-activity";

export interface FolderAuthority { epoch: string; lease: number; revision: number; }
interface FolderBasis { hash: string | null; authority?: FolderAuthority; }

export interface FolderState {
  authority?: FolderAuthority;
  mode?: "editable" | "generator";
  entry?: string;
  capabilities?: { sourceEditing: boolean; geometryEditing: boolean; generatorInputs: boolean };
  inputDefinitions?: Record<string, GeneratorInputDefinition> | null;
  inputs?: Record<string, unknown> | null;
  sourceFiles?: SourceFileSnapshot[];
  editor?: { clientId: string | null; canEdit: boolean };
  ok: boolean;
  sequence: number;
  currentHash: string | null;
  acceptedHash: string | null;
  status: string;
  diagnostics: Array<{ detail: string }>;
  paths: { folder: string; source: string };
}

/** Transport only. The server owns the same Rust workbench used by ordinary mode. */
export class FolderWorkbenchAdapter implements WorkbenchAdapter {
  readonly activity = new WorkbenchActivity();
  state: FolderState | null = null;
  /** Schema, values and source from the snapshot the host actually installed. */
  installedState: FolderState | null = null;
  private installedSnapshot?: WorkbenchSnapshot;
  private selectedSourcePath?: string;
  private readonly snapshotStates = new WeakMap<object, FolderState>();
  get isGenerator() { return this.installedState?.mode === "generator"; }
  get editingBlockedReason() {
    return this.state?.editor && !this.state.editor.canEdit
      ? "Another tab owns editing. Take over editing to make changes."
      : this.isGenerator ? "Change generator inputs or edit the TypeScript files to regenerate this design."
        : this.installedState?.capabilities?.geometryEditing === false ? "Geometry editing is unavailable for this project." : undefined;
  }
  notice = "Connecting to local folder…";
  pending = "";
  private basis: FolderBasis = { hash: null };
  private readonly clientId = crypto.randomUUID();
  private installedSequence = -1;
  private readonly snapshotHashes = new WeakMap<object, FolderBasis>();
  private readonly fields = new Map<string, { base: FolderBasis; label: string; value: string }>();
  private committingField?: string;
  private draftBase: FolderBasis | undefined;
  private get fieldBase() { return this.fields.values().next().value?.base; }
  private get fieldDraft() { return JSON.stringify([...this.fields.values()], null, 2); }
  private retainedError = "";
  private tail: Promise<unknown> = Promise.resolve();
  private sequence = 0;
  private listener?: (snapshot?: WorkbenchSnapshot) => void;
  private events?: EventSource;
  private eventSequence = -1;

  constructor(private readonly token: string) {
    try { this.pending = sessionStorage.getItem("geosolve.folder.pending") ?? ""; } catch { /* In-memory intent remains available. */ }
  }
  private rpc<T>(method: string, input?: unknown): Promise<T> {
    // Capture intent authority when enqueued, never from a later disk observation.
    const field = this.committingField;
    const fieldIntent = field ? this.fields.get(field) : undefined;
    const operationId = crypto.randomUUID();
    const savedMutation = ["files.apply", "inputs.set"].includes(method) || method === "dispatch" && [
      "source.prepare", "history.undo", "history.redo", "parameter.edit", "dimensions.edit",
      "authoring.metadata.set", "authoring.parameter.extract",
    ].includes((input as { command?: string })?.command ?? "");
    const readOnly = ["snapshot", "toolCatalog", "operation.outcome", "recovery.inspect",
      "exportProject", "exportReproduction", "exportInteractionTrace"].includes(method);
    const expected = method === "session.takeover"
      ? { hash: this.basis.hash, authority: this.state?.authority } : method === "dispatch" && (input as { command?: string })?.command === "source.prepare" && this.draftBase !== undefined
      ? this.draftBase : this.fieldBase !== undefined ? this.fieldBase : this.basis;
    const finishActivity = this.activity.begin();
    const run = this.tail.then(async () => {
      const request = { method, input, baseHash: expected.hash, authority: expected.authority, clientId: this.clientId, operationId };
      const send = async () => {
        const response = await fetch("/api/rpc", {
          method: "POST", headers: { "Content-Type": "application/json", Authorization: `Bearer ${this.token}` },
          body: JSON.stringify(request),
        });
        return { response, body: await response.json() as { result: T; state?: FolderState; error?: string; pendingSource?: string } };
      };
      let received;
      try { received = await send(); }
      catch (firstError) {
        // Only authored transactions have durable receipts. Replaying a wheel,
        // pointer or handoff could apply its relative action twice.
        try {
          if (!savedMutation && !readOnly) throw firstError;
          received = await send();
        } catch (error) {
          if (savedMutation) {
            this.pending = JSON.stringify(request, null, 2);
            try { sessionStorage.setItem("geosolve.folder.pending", this.pending); } catch { /* Download remains available. */ }
          }
          this.notice = this.retainedError = savedMutation
            ? "Save status is unknown after disconnect. Check the saved operation before retrying this edit."
            : "Connection lost. Refresh to reconnect to the current folder state.";
          this.listener?.();
          throw error;
        }
      }
      const { response, body } = received;
      if (body.state) this.state = body.state;
      if (!this.pendingOperationId && this.fieldBase !== undefined && !this.sameBasis(this.fieldBase, { hash: this.state?.currentHash ?? null, authority: this.state?.authority }) && this.fieldDraft) {
        this.pending = this.fieldDraft;
        try { sessionStorage.setItem("geosolve.folder.pending", this.pending); } catch { /* Download remains available. */ }
      }
      if (!response.ok) {
        this.notice = body.error ?? `Folder request failed (${response.status})`;
        this.retainedError = this.notice;
        if (response.status === 409 || body.pendingSource) {
          this.pending = JSON.stringify({ ...request, pendingSource: body.pendingSource }, null, 2);
          try { sessionStorage.setItem("geosolve.folder.pending", this.pending); } catch { this.notice += " Download pending intent before closing this tab."; }
        }
        this.listener?.();
        throw Error(this.notice);
      }
      // Fetching is observation. Only the host can acknowledge installation.
      if (body.result && typeof body.result === "object" && "frame" in body.result) {
        this.snapshotHashes.set(body.result, { hash: body.state?.currentHash ?? null, authority: body.state?.authority });
        if (body.state) this.snapshotStates.set(body.result, body.state);
        const result = body.result as unknown as WorkbenchSnapshot;
        if (field && this.fields.get(field) === fieldIntent && !result.source.dirty && result.project.status === "accepted") this.fields.delete(field);
      }
      this.notice = this.retainedError || (this.state?.editor && !this.state.editor.canEdit ? "Read only — another tab owns editing. Take over editing to work here." : undefined) || (this.fieldBase !== undefined && !this.sameBasis(this.fieldBase, { hash: this.state?.currentHash ?? null, authority: this.state?.authority })
        ? "Disk changed while an Inspector edit is pending. Your input is retained; applying it will report a conflict."
        : this.state?.ok ? "Saved to disk" : `Last accepted geometry retained — ${this.state?.status}: ${this.state?.diagnostics.map((item) => item.detail).join("; ")}`);
      this.listener?.();
      return body.result;
    }).finally(finishActivity);
    this.tail = run.catch(() => {});
    return run;
  }
  get pendingOperationId(): string | undefined {
    try { return (JSON.parse(this.pending) as { operationId?: string }).operationId; } catch { return undefined; }
  }
  async checkPendingOperation(): Promise<void> {
    const operationId = this.pendingOperationId;
    if (!operationId) throw Error("No saved operation ID is available");
    const outcome = await this.rpc<{ state: string } | null>("operation.outcome", { operationId });
    this.notice = outcome && ["published", "acknowledged"].includes(outcome.state)
      ? "This edit was saved. Revert any old draft and refresh to see the current disk state."
      : outcome ? `This edit requires recovery (${outcome.state}). Its files remain available through the CLI recovery command.`
        : "This edit has no saved receipt. Its original intent is retained for inspection and an explicit retry.";
    this.retainedError = this.notice;
    this.listener?.();
  }
  private checked(snapshot: WorkbenchSnapshot) {
    assertWorkbenchSnapshot(snapshot);
    const state = this.snapshotStates.get(snapshot);
    const basis = this.snapshotHashes.get(snapshot);
    if (!basis) throw Error("Folder snapshot has no transport authority");
    let result = snapshot;
    if (state?.mode === "generator" || state?.editor?.canEdit === false || state?.capabilities?.geometryEditing === false) {
      result = structuredClone(snapshot);
      const reason = state.editor?.canEdit === false ? "Another tab owns editing."
        : "Change generator inputs or edit the TypeScript files to regenerate this design.";
      if (state.mode === "generator" && state.sourceFiles?.length) {
        result.source.files = state.sourceFiles.map((file) => ({ ...file, readOnly: true }));
        result.source.selectedPath = result.source.files.find((file) => file.path === this.selectedSourcePath)?.path
          ?? result.source.files.find((file) => file.path === state.entry)?.path ?? result.source.files[0]!.path;
        result.source.dirty = false;
      } else result.source.files = result.source.files.map((file) => ({ ...file, readOnly: true }));
      result = readOnlyFolderPresentation(result, reason);
      this.snapshotHashes.set(result, basis);
      if (state) this.snapshotStates.set(result, state);
    }
    return stampCanvasSnapshot(assertWorkbenchSnapshot(result), ++this.sequence);
  }
  private async update(method: string, input?: unknown) {
    if (this.state?.editor && !this.state.editor.canEdit) return null;
    const result = await this.rpc<WorkbenchSnapshot | null>(method, input);
    return result ? this.checked(result) : null;
  }
  async construct() { return this.checked(await this.rpc<WorkbenchSnapshot>("session.join")); }
  async takeOver() {
    if (!this.pendingOperationId && this.fieldBase !== undefined) this.pending = this.fieldDraft;
    this.retainedError = "";
    const snapshot = this.checked(await this.rpc<WorkbenchSnapshot>("session.takeover"));
    this.listener?.(snapshot);
  }
  async snapshot() { return this.checked(await this.rpc<WorkbenchSnapshot>("snapshot")); }
  async toolCatalog() { return this.rpc<ToolCatalog>("toolCatalog"); }
  async dispatch(input: { version: 2; command: string; payload?: unknown }) {
    if (input.command === "source.select" && this.isGenerator && this.installedSnapshot) {
      const path = (input.payload as { path?: string })?.path;
      if (!path || !this.installedSnapshot.source.files.some((file) => file.path === path)) throw Error("Source file is not in this accepted generator");
      const result = structuredClone(this.installedSnapshot);
      result.source.selectedPath = path;
      this.selectedSourcePath = path;
      this.snapshotHashes.set(result, this.snapshotHashes.get(this.installedSnapshot)!);
      this.snapshotStates.set(result, this.snapshotStates.get(this.installedSnapshot)!);
      return this.checked(result);
    }
    const result = this.checked(await this.rpc<WorkbenchSnapshot>("dispatch", input));
    if (["source.prepare", "source.revert"].includes(input.command) && !result.source.dirty) this.draftBase = undefined;
    return result;
  }
  async managedCompilerContext(): Promise<{ version: 2; patches: Record<string, unknown> }> { throw Error("Folder compiler transactions settle in the local Rust bridge."); }
  pointer(input: PointerSample) {
    // Generator clicks select, but dragging cannot edit the generated native model.
    if (this.isGenerator && input.phase === "move" && (input.buttons & 1)) return Promise.resolve(null);
    return this.update("pointer", input);
  }
  async setGeneratorInputs(values: Record<string, unknown>) {
    if (!this.isGenerator || this.installedState?.capabilities?.generatorInputs === false) throw Error("This project does not expose generator inputs");
    if (this.state?.editor && !this.state.editor.canEdit) throw Error("Another tab owns editing. Take over editing to change generator inputs.");
    return this.checked(await this.rpc<WorkbenchSnapshot>("inputs.set", { values: structuredClone(values) }));
  }
  wheel(input: WheelSample) { return this.update("wheel", input); }
  wheelBatch(input: WheelSample[]) { return this.update("wheelBatch", input); }
  resize(input: { version: 2; width: number; height: number; pixelRatio: number }) { return this.update("resize", input); }
  cancel(input: { version: 2; reason: "escape" | "lost-capture" | "blur" }) { return this.update("cancel", input); }
  exportProject() { return this.rpc<{ version: 2; filename: string; contents: string }>("exportProject"); }
  exportReproduction() { return this.rpc<{ version: 2; filename: string; contents: string }>("exportReproduction"); }
  exportInteractionTrace() { return this.rpc<{ version: 2; filename: string; contents: string }>("exportInteractionTrace"); }
  async persistProject(): Promise<{ version: 2; contents: string }> { throw Error("Folder source is saved automatically by accepted transactions."); }

  draftChanged(dirty: boolean) {
    if (dirty && this.draftBase === undefined) this.draftBase = this.basis;
    if (!dirty) this.draftBase = undefined;
  }
  /** Called after host validation and immediately before replacing displayed state. */
  installSnapshot(snapshot: WorkbenchSnapshot): boolean {
    if (!this.snapshotHashes.has(snapshot)) throw Error("Folder snapshot has no transport authority");
    const sequence = getCanvasSnapshotSequence(snapshot)!;
    if (sequence < this.installedSequence) return false;
    const basis = this.snapshotHashes.get(snapshot)!;
    if ((this.fieldBase !== undefined && !this.sameBasis(this.fieldBase, basis)) || (this.draftBase !== undefined && !this.sameBasis(this.draftBase, basis))) return false;
    this.basis = basis;
    this.installedSequence = sequence;
    this.installedState = this.snapshotStates.get(snapshot) ?? null;
    this.installedSnapshot = snapshot;
    return true;
  }
  private sameBasis(left: FolderBasis, right: FolderBasis) {
    return left.hash === right.hash && left.authority?.epoch === right.authority?.epoch && left.authority?.lease === right.authority?.lease;
  }
  beginFieldEdit(id: string, label: string) {
    if (!this.fields.has(id)) this.fields.set(id, { base: this.basis, label, value: "" });
  }
  changeFieldEdit(id: string, label: string, value: string) {
    this.beginFieldEdit(id, label);
    this.fields.set(id, { ...this.fields.get(id)!, value });
  }
  commitFieldEdit(id: string, action: () => void) {
    this.committingField = id;
    try { action(); } finally { this.committingField = undefined; }
  }
  cancelFieldEdit(id: string) { this.fields.delete(id); }
  subscribe(listener: (snapshot?: WorkbenchSnapshot) => void) {
    this.listener = listener;
    const events = new EventSource(`/api/events?token=${encodeURIComponent(this.token)}`);
    this.events = events;
    let finishRemoteActivity: (() => void) | undefined;
    const clearRemoteActivity = () => { finishRemoteActivity?.(); finishRemoteActivity = undefined; };
    events.addEventListener("activity", (event) => {
      if (this.events !== events) return;
      try {
        const { busy } = JSON.parse((event as MessageEvent<string>).data) as { busy?: unknown };
        if (busy === true) finishRemoteActivity ??= this.activity.begin();
        else if (busy === false) clearRemoteActivity();
      } catch { /* An invalid presentation hint grants no state or authority. */ }
    });
    events.onmessage = (event) => {
      const sequence = Number(event.data);
      if (sequence === this.eventSequence) return;
      this.eventSequence = sequence;
      void this.refresh(false).catch(() => {});
    };
    events.onerror = () => { clearRemoteActivity(); this.notice = "Disconnected — edits are not saved. Reconnecting to disk…"; this.eventSequence = -1; listener(); };
    return () => { events.close(); clearRemoteActivity(); if (this.events === events) { this.events = undefined; this.listener = undefined; } };
  }
  async refresh(explicit = true) {
    if (explicit) {
      if (!this.pendingOperationId && this.fieldBase !== undefined) this.pending = this.fieldDraft;
      this.retainedError = "";
    }
    try { const snapshot = await this.snapshot(); if (this.fieldBase === undefined && this.draftBase === undefined) this.listener?.(snapshot); return snapshot; }
    catch (error) { this.notice = `Refresh failed; pending intent retained: ${String(error)}`; this.listener?.(); throw error; }
  }
}

/** Restrict host interactions without changing any accepted value or geometry. */
export function readOnlyFolderPresentation(snapshot: WorkbenchSnapshot, reason: string): WorkbenchSnapshot {
  const result = { ...snapshot, presentation: { ...snapshot.presentation },
    dimensions: snapshot.dimensions ? { ...snapshot.dimensions } : undefined };
  result.presentation.canUndo = false;
  result.presentation.canRedo = false;
  result.presentation.canFinish = false;
  delete result.presentation.selectedGeometryRole;
  const restrict = (rows: DeclarationRow[]): DeclarationRow[] => rows.map((row) => ({ ...row,
    capabilities: { ...row.capabilities, ...Object.fromEntries(["edit", "move", "moveUp", "moveDown", "suppress", "delete"].map((action) => [action, { enabled: false, reason }])) },
    children: restrict(row.children),
  }));
  result.explorer = restrict(result.explorer);
  const metadata = (value: AuthoringMetadataSnapshot | undefined) => value ? { ...value, editable: false, canExtract: false, reason } : undefined;
  if (result.authoringDocument) result.authoringDocument = { ...result.authoringDocument, editable: false, reason };
  if (result.selection) result.selection = { ...result.selection, metadata: metadata(result.selection.metadata) };
  result.parameters = result.parameters.map((parameter) => ({ ...parameter, editable: false, metadata: metadata(parameter.metadata) }));
  if (result.dimensions) {
    result.dimensions.entries = result.dimensions.entries.map((entry) => ({ ...entry, editable: false, reason, metadata: metadata(entry.metadata) }));
    result.dimensions.parameters = result.dimensions.parameters.map((parameter) => ({ ...parameter, editable: false, metadata: metadata(parameter.metadata) }));
    if (result.dimensions.allMeasurements) result.dimensions.allMeasurements = result.dimensions.allMeasurements.map((entry) => ({ ...entry, editable: false, reason, metadata: metadata(entry.metadata) }));
  }
  return result;
}
