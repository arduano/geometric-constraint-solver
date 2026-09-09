// SPDX-License-Identifier: GPL-3.0-or-later
import { assertWorkbenchSnapshot, getCanvasSnapshotSequence, stampCanvasSnapshot, type WorkbenchAdapter, type WorkbenchSnapshot, type PointerSample, type WheelSample } from "./adapter";
import type { ToolCatalog } from "./tool-catalog";

export interface FolderState {
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
  state: FolderState | null = null;
  notice = "Connecting to local folder…";
  pending = "";
  private hash: string | null = null;
  private installedSequence = -1;
  private readonly snapshotHashes = new WeakMap<object, string | null>();
  private readonly fields = new Map<string, { base: string | null; label: string; value: string }>();
  private committingField?: string;
  private draftBase: string | null | undefined;
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
    const expected = method === "dispatch" && (input as { command?: string })?.command === "source.prepare" && this.draftBase !== undefined
      ? this.draftBase : this.fieldBase !== undefined ? this.fieldBase : this.hash;
    const run = this.tail.then(async () => {
      const response = await fetch("/api/rpc", {
        method: "POST", headers: { "Content-Type": "application/json", Authorization: `Bearer ${this.token}` },
        body: JSON.stringify({ method, input, baseHash: expected }),
      });
      const body = await response.json() as { result: T; state?: FolderState; error?: string; pendingSource?: string };
      if (body.state) this.state = body.state;
      if (this.fieldBase !== undefined && this.fieldBase !== this.state?.currentHash && this.fieldDraft) {
        this.pending = this.fieldDraft;
        try { sessionStorage.setItem("geosolve.folder.pending", this.pending); } catch { /* Download remains available. */ }
      }
      if (!response.ok) {
        this.notice = body.error ?? `Folder request failed (${response.status})`;
        this.retainedError = this.notice;
        if (response.status === 409 || body.pendingSource) {
          this.pending = JSON.stringify({ method, input, baseHash: expected, pendingSource: body.pendingSource }, null, 2);
          try { sessionStorage.setItem("geosolve.folder.pending", this.pending); } catch { this.notice += " Download pending intent before closing this tab."; }
        }
        this.listener?.();
        throw Error(this.notice);
      }
      // Fetching is observation. Only the host can acknowledge installation.
      if (body.result && typeof body.result === "object" && "frame" in body.result) {
        this.snapshotHashes.set(body.result, body.state?.currentHash ?? null);
        const result = body.result as unknown as WorkbenchSnapshot;
        if (field && this.fields.get(field) === fieldIntent && !result.source.dirty && result.project.status === "accepted") this.fields.delete(field);
      }
      this.notice = this.retainedError || (this.fieldBase !== undefined && this.fieldBase !== this.state?.currentHash
        ? "Disk changed while an Inspector edit is pending. Your input is retained; applying it will report a conflict."
        : this.state?.ok ? "Saved to disk" : `Last accepted geometry retained — ${this.state?.status}: ${this.state?.diagnostics.map((item) => item.detail).join("; ")}`);
      this.listener?.();
      return body.result;
    });
    this.tail = run.catch(() => {});
    return run;
  }
  private checked(snapshot: WorkbenchSnapshot) { return stampCanvasSnapshot(assertWorkbenchSnapshot(snapshot), ++this.sequence); }
  private async update(method: string, input?: unknown) {
    const result = await this.rpc<WorkbenchSnapshot | null>(method, input);
    return result ? this.checked(result) : null;
  }
  async construct() { return this.snapshot(); }
  async snapshot() { return this.checked(await this.rpc<WorkbenchSnapshot>("snapshot")); }
  async toolCatalog() { return this.rpc<ToolCatalog>("toolCatalog"); }
  async dispatch(input: { version: 2; command: string; payload?: unknown }) {
    const result = this.checked(await this.rpc<WorkbenchSnapshot>("dispatch", input));
    if (["source.prepare", "source.revert"].includes(input.command) && !result.source.dirty) this.draftBase = undefined;
    return result;
  }
  async managedCompilerContext(): Promise<{ version: 2; patches: Record<string, unknown> }> { throw Error("Folder compiler transactions settle in the local Rust bridge."); }
  pointer(input: PointerSample) { return this.update("pointer", input); }
  wheel(input: WheelSample) { return this.update("wheel", input); }
  wheelBatch(input: WheelSample[]) { return this.update("wheelBatch", input); }
  resize(input: { version: 2; width: number; height: number; pixelRatio: number }) { return this.update("resize", input); }
  cancel(input: { version: 2; reason: "escape" | "lost-capture" | "blur" }) { return this.update("cancel", input); }
  exportProject() { return this.rpc<{ version: 2; filename: string; contents: string }>("exportProject"); }
  exportReproduction() { return this.rpc<{ version: 2; filename: string; contents: string }>("exportReproduction"); }
  exportInteractionTrace() { return this.rpc<{ version: 2; filename: string; contents: string }>("exportInteractionTrace"); }
  async persistProject(): Promise<{ version: 2; contents: string }> { throw Error("Folder source is saved automatically by accepted transactions."); }

  draftChanged(dirty: boolean) {
    if (dirty && this.draftBase === undefined) this.draftBase = this.hash;
    if (!dirty) this.draftBase = undefined;
  }
  /** Called after host validation and immediately before replacing displayed state. */
  installSnapshot(snapshot: WorkbenchSnapshot): boolean {
    if (!this.snapshotHashes.has(snapshot)) throw Error("Folder snapshot has no transport authority");
    const sequence = getCanvasSnapshotSequence(snapshot)!;
    if (sequence < this.installedSequence) return false;
    const hash = this.snapshotHashes.get(snapshot)!;
    if ((this.fieldBase !== undefined && this.fieldBase !== hash) || (this.draftBase !== undefined && this.draftBase !== hash)) return false;
    this.hash = hash;
    this.installedSequence = sequence;
    return true;
  }
  beginFieldEdit(id: string, label: string) {
    if (!this.fields.has(id)) this.fields.set(id, { base: this.hash, label, value: "" });
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
    events.onmessage = (event) => {
      const sequence = Number(event.data);
      if (sequence === this.eventSequence) return;
      this.eventSequence = sequence;
      void this.refresh(false).catch(() => {});
    };
    events.onerror = () => { this.notice = "Disconnected — edits are not saved. Reconnecting to disk…"; this.eventSequence = -1; listener(); };
    return () => { events.close(); if (this.events === events) { this.events = undefined; this.listener = undefined; } };
  }
  async refresh(explicit = true) {
    if (explicit) {
      if (this.fieldBase !== undefined) this.pending = this.fieldDraft;
      this.fields.clear();
      this.retainedError = "";
    }
    try { const snapshot = await this.snapshot(); if (this.fieldBase === undefined && this.draftBase === undefined) this.listener?.(snapshot); return snapshot; }
    catch (error) { this.notice = `Refresh failed; pending intent retained: ${String(error)}`; this.listener?.(); throw error; }
  }
}
