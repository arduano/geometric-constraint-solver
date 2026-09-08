// SPDX-License-Identifier: GPL-3.0-or-later
import { assertWorkbenchSnapshot, stampCanvasSnapshot, type WorkbenchAdapter, type WorkbenchSnapshot, type PointerSample, type WheelSample } from "./adapter";
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
  private draftBase: string | null | undefined;
  private fieldBase: string | null | undefined;
  private fieldDraft = "";
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
    const expected = method === "dispatch" && (input as { command?: string })?.command === "source.prepare"
      ? this.draftBase ?? this.hash : method === "dispatch" && /(?:edit|set)$/.test((input as { command?: string })?.command ?? "")
        ? this.fieldBase ?? this.hash : this.hash;
    const run = this.tail.then(async () => {
      const response = await fetch("/api/rpc", {
        method: "POST", headers: { "Content-Type": "application/json", Authorization: `Bearer ${this.token}` },
        body: JSON.stringify({ method, input, baseHash: expected }),
      });
      const body = await response.json() as { result: T; state?: FolderState; error?: string; pendingSource?: string };
      if (body.state) this.state = body.state;
      if (response.ok && method === "dispatch" && /(?:edit|set)$/.test((input as { command?: string })?.command ?? "")
        && body.result && typeof body.result === "object" && "source" in body.result
        && !(body.result as unknown as WorkbenchSnapshot).source.dirty && (body.result as unknown as WorkbenchSnapshot).project.status === "accepted") this.fieldBase = undefined;
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
      // Only snapshots advance the client's displayed-source CAS token.
      if (body.result && typeof body.result === "object" && "frame" in body.result) this.hash = this.state?.currentHash ?? null;
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
    if (/(?:edit|set)$/.test(input.command) && !result.source.dirty && result.project.status === "accepted") this.fieldBase = undefined;
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
  fieldChanged(label: string, value: string) {
    if (this.fieldBase === undefined) this.fieldBase = this.hash;
    this.fieldDraft = JSON.stringify({ label, value, baseHash: this.fieldBase }, null, 2);
  }
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
      this.fieldBase = undefined;
      this.retainedError = "";
    }
    try { const snapshot = await this.snapshot(); if (this.fieldBase === undefined) this.listener?.(snapshot); return snapshot; }
    catch (error) { this.notice = `Refresh failed; pending intent retained: ${String(error)}`; this.listener?.(); throw error; }
  }
}
