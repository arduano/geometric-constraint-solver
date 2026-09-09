// SPDX-License-Identifier: GPL-3.0-or-later
import { assertWorkbenchSnapshot, getCanvasSnapshotSequence, stampCanvasSnapshot, markCanvasOnlySnapshot, type WorkbenchAdapter, type WorkbenchSnapshot, type PointerSample, type WheelSample, type SourceFileSnapshot, type DeclarationRow, type AuthoringMetadataSnapshot } from "./adapter";
import type { GeneratorInputDefinition } from "../components/generator-inputs";
import type { ToolCatalog } from "./tool-catalog";
import { WorkbenchActivity } from "./workbench-activity";
import { LocalInteractionWorker, type LocalInteractionClient, type InteractionSeed, type InteractionState, type LocalInteractionUpdate } from "./local-interaction-adapter";

export interface FolderAuthority { epoch: string; lease: number; revision: number; }
interface FolderBasis { hash: string | null; authority?: FolderAuthority; }
interface SyncReceipt { before: FolderBasis; installed: Promise<FolderBasis | undefined>; resolve: (basis?: FolderBasis) => void; }

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

/** Server-owned edits with a detached Rust interaction worker for accepted canvas navigation. */
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
  private installedRemoteOrder = -1;
  private remoteOrder = 0;
  private readonly snapshotRemoteOrders = new WeakMap<object, number>();
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
  private local?: LocalInteractionClient;
  private localSnapshot?: WorkbenchSnapshot;
  private localTail: Promise<unknown> = Promise.resolve();
  private localSelectionRevision = 0;
  private readonly snapshotSeeds = new WeakMap<object, { seed: InteractionSeed; selectionRevision: number; preserveSelection: boolean }>();
  private syncPending = false;
  private syncAgain = false;
  private syncNeedsRefresh = false;
  private syncRefreshPending = false;
  private syncReceipt?: SyncReceipt;
  private readonly snapshotSyncReceipts = new WeakMap<object, SyncReceipt>();
  private gesture?: { down: PointerSample; state: Promise<InteractionState>; samples: PointerSample[]; remote: boolean; editable: boolean };
  private remoteGesture = false;
  private remoteDraft = false;
  private localPan?: number;
  private semanticPending = 0;
  get responsiveCanvas() { return this.localSnapshot !== undefined; }

  constructor(private readonly token: string, private readonly createLocal: () => LocalInteractionClient = () => new LocalInteractionWorker()) {
    try { this.pending = sessionStorage.getItem("geosolve.folder.pending") ?? ""; } catch { /* In-memory intent remains available. */ }
  }
  private rpc<T>(method: string, input?: unknown, options: { interaction?: Promise<InteractionState> | null; preserveSelection?: boolean } = {}): Promise<T> {
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
    let selectionRevision = this.localSelectionRevision;
    const precedingSync = method !== "interaction.sync" ? this.syncReceipt : undefined;
    const interaction = options.interaction !== undefined ? options.interaction
      : this.localSnapshot && !readOnly && !["session.join", "session.takeover"].includes(method)
        ? this.queueLocal(() => { selectionRevision = this.localSelectionRevision; return this.local!.state(); }) : undefined;
    const semantic = !readOnly && method !== "interaction.sync" && !["session.join", "session.takeover"].includes(method)
      && !(method === "dispatch" && isPresentationCommand((input as { command?: string })?.command ?? ""));
    if (semantic) this.semanticPending += 1;
    const finishActivity = method === "interaction.sync" ? () => {} : this.activity.begin();
    const run = this.tail.then(async () => {
      const synchronized = await precedingSync?.installed;
      // Only an installed, same-source presentation receipt may advance the
      // interaction revision of an intent enqueued behind its own selection sync.
      const authority = synchronized && this.sameBasis(expected, synchronized)
        && expected.authority?.revision === precedingSync?.before.authority?.revision
        ? synchronized.authority : expected.authority;
      const request = { method, input, baseHash: expected.hash, authority, clientId: this.clientId, operationId, localInteraction: true, ...(interaction ? { interaction: await interaction } : {}) };
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
      if (!response.ok && method === "interaction.sync" && response.status === 409) {
        // Automatic presentation can race a source/lease notification. Retain
        // local navigation and refresh its basis; this is not an authored save.
        this.syncAgain = true;
        this.syncNeedsRefresh = true;
        this.refreshSelectionBasis();
        return null as T;
      }
      if (!response.ok) {
        this.notice = body.error ?? `Folder request failed (${response.status})`;
        this.retainedError = this.notice;
        if (method !== "interaction.sync" && (response.status === 409 || body.pendingSource)) {
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
        const result = body.result as unknown as WorkbenchSnapshot & { localInteraction?: InteractionSeed };
        if (result.localInteraction) this.snapshotSeeds.set(result, { seed: result.localInteraction, selectionRevision, preserveSelection: options.preserveSelection ?? !changesSelection(method, input) });
        if (field && this.fields.get(field) === fieldIntent && !result.source.dirty && result.project.status === "accepted") this.fields.delete(field);
      }
      this.notice = this.retainedError || (this.state?.editor && !this.state.editor.canEdit ? "Read only — another tab owns editing. Take over editing to work here." : undefined) || (this.fieldBase !== undefined && !this.sameBasis(this.fieldBase, { hash: this.state?.currentHash ?? null, authority: this.state?.authority })
        ? "Disk changed while an Inspector edit is pending. Your input is retained; applying it will report a conflict."
        : this.state?.ok ? "Saved to disk" : `Last accepted geometry retained — ${this.state?.status}: ${this.state?.diagnostics.map((item) => item.detail).join("; ")}`);
      this.listener?.();
      return body.result;
    }).finally(() => { if (semantic) this.semanticPending -= 1; finishActivity(); });
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
      const seed = this.snapshotSeeds.get(snapshot);
      if (seed) this.snapshotSeeds.set(result, seed);
      if (state) this.snapshotStates.set(result, state);
    }
    this.snapshotRemoteOrders.set(result, ++this.remoteOrder);
    return stampCanvasSnapshot(assertWorkbenchSnapshot(result), ++this.sequence);
  }
  private async update(method: string, input?: unknown, options?: { interaction?: Promise<InteractionState> | null; preserveSelection?: boolean }) {
    if (this.state?.editor && !this.state.editor.canEdit) return null;
    const result = await this.rpc<WorkbenchSnapshot | null>(method, input, options);
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
    if (this.localSnapshot && localCommands.has(input.command)) {
      if (input.command === "view.fit" || input.command === "view.origin") this.cancelRemoteGesture();
      const snapshot = await this.localUpdate("dispatch", input);
      if (!transientLocalCommands.has(input.command)) this.requestSelectionSync();
      return snapshot ?? this.localSnapshot!;
    }

    if (input.command === "source.select" && this.isGenerator && this.installedSnapshot) {
      const path = (input.payload as { path?: string })?.path;
      if (!path || !this.installedSnapshot.source.files.some((file) => file.path === path)) throw Error("Source file is not in this accepted generator");
      const result = structuredClone(this.localSnapshot ?? this.installedSnapshot);
      result.source.selectedPath = path;
      this.selectedSourcePath = path;
      this.snapshotHashes.set(result, this.snapshotHashes.get(this.installedSnapshot)!);
      this.snapshotStates.set(result, this.snapshotStates.get(this.installedSnapshot)!);
      return this.checked(result);
    }
    const options = this.remoteDraft ? { interaction: null } : undefined;
    const result = this.checked(await this.rpc<WorkbenchSnapshot>("dispatch", input, options));
    if (["tool.select", "tool.finish", "feature.apply"].includes(input.command)) this.remoteDraft = result.presentation.activeTool !== "select";
    if (["source.prepare", "source.revert"].includes(input.command) && !result.source.dirty) this.draftBase = undefined;
    return result;
  }
  async managedCompilerContext(): Promise<{ version: 2; patches: Record<string, unknown> }> { throw Error("Folder compiler transactions settle in the local Rust bridge."); }
  async pointer(input: PointerSample) {
    if (!this.localSnapshot) {
      if (this.isGenerator && input.phase === "move" && (input.buttons & 1)) return null;
      return this.update("pointer", input);
    }
    if (input.phase === "down" && input.buttons === 4) {
      this.cancelRemoteGesture();
      this.localPan = input.pointerId;
    }
    if (this.localPan === input.pointerId) {
      if (input.phase === "up") this.localPan = undefined;
      return this.localUpdate("pointer", input);
    }
    const select = Boolean(this.editingBlockedReason) || this.localSnapshot.presentation.activeTool === "select";
    if (!select) {
      const continuation = this.remoteDraft || this.remoteGesture;
      if (input.phase === "down") { this.remoteGesture = true; this.remoteDraft = true; }
      else if (input.phase === "up") this.remoteGesture = false;
      this.deliverRemote("pointer", input, continuation ? { interaction: null } : {});
      return null;
    }
    if (input.phase === "down" && (input.buttons & 1)) {
      this.gesture = { down: input, state: this.queueLocal(() => this.local!.state()), samples: [], remote: false,
        editable: !this.editingBlockedReason && this.semanticPending === 0 && !this.activity.getPendingSnapshot() };
      return this.localUpdate("pointer", input);
    }
    const gesture = this.gesture;
    if (gesture && gesture.down.pointerId === input.pointerId && (input.phase === "move" || input.phase === "up")) {
      gesture.samples.push(input);
      if (gesture.editable && !gesture.remote && Math.hypot(input.x - gesture.down.x, input.y - gesture.down.y) >= 3) {
        gesture.remote = true;
        this.deliverRemote("pointer", gesture.down, { interaction: gesture.state });
        for (const sample of gesture.samples) this.deliverRemote("pointer", sample, { interaction: null });
        gesture.samples = [];
        await this.localUpdate("cancel", { version: 2, reason: "lost-capture" });
      } else if (gesture.remote) {
        this.deliverRemote("pointer", input, { interaction: null });
        gesture.samples = [];
      }
      if (input.phase === "up") {
        this.gesture = undefined;
        if (!gesture.remote) {
          const snapshot = await this.localUpdate("pointer", input);
          this.requestSelectionSync();
          return snapshot;
        }
      }
      return gesture.remote ? null : this.localUpdate("pointer", input);
    }
    return this.localUpdate("pointer", input);
  }
  async setGeneratorInputs(values: Record<string, unknown>) {
    if (!this.isGenerator || this.installedState?.capabilities?.generatorInputs === false) throw Error("This project does not expose generator inputs");
    if (this.state?.editor && !this.state.editor.canEdit) throw Error("Another tab owns editing. Take over editing to change generator inputs.");
    return this.checked(await this.rpc<WorkbenchSnapshot>("inputs.set", { values: structuredClone(values) }));
  }
  wheel(input: WheelSample) { if (this.localSnapshot) this.cancelRemoteGesture(); return this.localSnapshot ? this.localUpdate("wheel", input) : this.update("wheel", input); }
  wheelBatch(input: WheelSample[]) { if (this.localSnapshot) this.cancelRemoteGesture(); return this.localSnapshot ? (input.length ? this.localUpdate("wheel", { version: 2, samples: input }) : Promise.resolve(null)) : this.update("wheelBatch", input); }
  resize(input: { version: 2; width: number; height: number; pixelRatio: number }) { if (this.localSnapshot) this.cancelRemoteGesture(); return this.localSnapshot ? this.localUpdate("resize", input) : this.update("resize", input); }
  cancel(input: { version: 2; reason: "escape" | "lost-capture" | "blur" }) {
    if (!this.localSnapshot) return this.update("cancel", input);
    if (this.gesture?.remote || this.remoteGesture || this.localSnapshot.presentation.activeTool !== "select") this.deliverRemote("cancel", input, { interaction: null });
    this.gesture = undefined;
    this.remoteGesture = false;
    this.remoteDraft = false;
    return this.localUpdate("cancel", input);
  }
  exportProject() { return this.rpc<{ version: 2; filename: string; contents: string }>("exportProject"); }
  exportReproduction() { return this.rpc<{ version: 2; filename: string; contents: string }>("exportReproduction"); }
  exportInteractionTrace() { return this.rpc<{ version: 2; filename: string; contents: string }>("exportInteractionTrace"); }
  async persistProject(): Promise<{ version: 2; contents: string }> { throw Error("Folder source is saved automatically by accepted transactions."); }

  /** Called only when the host intends to install this authenticated remote snapshot. */
  async prepareSnapshot(snapshot: WorkbenchSnapshot): Promise<WorkbenchSnapshot> {
    const seed = this.snapshotSeeds.get(snapshot);
    if (!seed || !this.canInstall(snapshot)) return snapshot;
    return this.queueLocal(async () => {
      if (!this.canInstall(snapshot)) return snapshot;
      const existed = this.local !== undefined;
      this.local ??= this.createLocal();
      const update = existed ? await this.local.replace(seed.seed, seed.preserveSelection || seed.selectionRevision !== this.localSelectionRevision)
        : await this.local.construct(seed.seed);
      const result = this.withLocalFrame(snapshot, update, false);
      this.localSnapshot = result;
      return result;
    }).catch((error: unknown) => { this.snapshotSyncReceipts.get(snapshot)?.resolve(); throw error; });
  }
  private queueLocal<T>(operation: () => Promise<T>): Promise<T> {
    const run = this.localTail.then(operation, operation);
    this.localTail = run.catch(() => undefined);
    return run;
  }
  private withLocalFrame(snapshot: WorkbenchSnapshot, update: LocalInteractionUpdate, canvasOnly: boolean) {
    // Authoritative drawing contains construction/inference overlays absent from
    // the detached accepted scene. Rust permits it only at the exact local view.
    const frame = !canvasOnly && update.serverFrameCompatible ? snapshot.frame : update.frame;
    const result = stampCanvasSnapshot(assertWorkbenchSnapshot({ ...snapshot, frame }), ++this.sequence);
    const basis = this.snapshotHashes.get(snapshot);
    if (basis) this.snapshotHashes.set(result, basis);
    const state = this.snapshotStates.get(snapshot);
    if (state) this.snapshotStates.set(result, state);
    this.snapshotRemoteOrders.set(result, this.snapshotRemoteOrders.get(snapshot)!);
    const sync = this.snapshotSyncReceipts.get(snapshot);
    if (sync) this.snapshotSyncReceipts.set(result, sync);
    return canvasOnly ? markCanvasOnlySnapshot(result) : result;
  }
  private localUpdate(method: "dispatch" | "pointer" | "wheel" | "resize" | "cancel", input: unknown) {
    return this.queueLocal(async () => {
      const update = await this.local!.update(method, input);
      if (!update) return null;
      if (update.selectionChanged) this.localSelectionRevision += 1;
      const result = this.withLocalFrame(this.localSnapshot!, update, true);
      this.localSnapshot = result;
      return result;
    });
  }
  private cancelRemoteGesture() {
    if (this.gesture?.remote || this.remoteGesture || this.remoteDraft) this.deliverRemote("cancel", { version: 2, reason: "lost-capture" }, { interaction: null });
    this.gesture = undefined;
    this.remoteGesture = false;
    this.remoteDraft = false;
    this.localPan = undefined;
  }
  private deliverRemote(method: string, input: unknown, options: { interaction?: Promise<InteractionState> | null } = {}) {
    void this.update(method, input, options).then((snapshot) => { if (snapshot) this.listener?.(snapshot); }).catch((error: unknown) => {
      if (!this.retainedError) this.notice = this.retainedError = `Canvas edit failed: ${String(error)}`;
      this.listener?.();
    });
  }
  private requestSelectionSync() {
    if (this.state?.editor?.canEdit === false || this.gesture?.remote || this.remoteGesture || this.remoteDraft) return;
    if (this.syncPending || this.semanticPending > 0 || this.activity.getPendingSnapshot() || this.syncNeedsRefresh) { this.syncAgain = true; return; }
    this.syncAgain = false;
    this.syncPending = true;
    let resolve!: (basis?: FolderBasis) => void;
    const receipt: SyncReceipt = { before: this.basis, installed: new Promise((done) => { resolve = done; }), resolve: (basis) => resolve(basis) };
    const response = this.update("interaction.sync", undefined, { preserveSelection: true });
    this.syncReceipt = receipt;
    void response.then(async (snapshot) => {
      if (!snapshot || !this.listener) { receipt.resolve(); return; }
      // Even an old selection response carries a new server interaction revision.
      // The host acknowledges it while prepareSnapshot preserves the latest view.
      this.snapshotSyncReceipts.set(snapshot, receipt);
      this.listener(snapshot);
      await receipt.installed;
    }).catch(() => { receipt.resolve(); }).finally(() => {
      if (this.syncReceipt === receipt) this.syncReceipt = undefined;
      this.syncPending = false;
      if (this.syncAgain && this.semanticPending === 0 && !this.syncNeedsRefresh) this.requestSelectionSync();
    });
  }
  private refreshSelectionBasis() {
    if (this.syncRefreshPending || !this.syncAgain || !this.listener) return;
    this.syncRefreshPending = true;
    void this.refresh(false).catch(() => {}).finally(() => { this.syncRefreshPending = false; });
  }
  private canInstall(snapshot: WorkbenchSnapshot) {
    const basis = this.snapshotHashes.get(snapshot);
    if (!basis) throw Error("Folder snapshot has no transport authority");
    const sequence = getCanvasSnapshotSequence(snapshot)!;
    const remoteOrder = this.snapshotRemoteOrders.get(snapshot)!;
    return (remoteOrder > this.installedRemoteOrder || remoteOrder === this.installedRemoteOrder && sequence >= this.installedSequence)
      && !(this.fieldBase !== undefined && !this.sameBasis(this.fieldBase, basis))
      && !(this.draftBase !== undefined && !this.sameBasis(this.draftBase, basis));
  }
  dispose() { this.syncReceipt?.resolve(); this.local?.dispose(); this.local = undefined; this.localSnapshot = undefined; }

  draftChanged(dirty: boolean) {
    if (dirty && this.draftBase === undefined) this.draftBase = this.basis;
    if (!dirty) this.draftBase = undefined;
  }
  /** Called after host validation and immediately before replacing displayed state. */
  installSnapshot(snapshot: WorkbenchSnapshot): boolean {
    const sync = this.snapshotSyncReceipts.get(snapshot);
    if (!this.canInstall(snapshot)) { sync?.resolve(); return false; }
    const sequence = getCanvasSnapshotSequence(snapshot)!;
    const basis = this.snapshotHashes.get(snapshot)!;
    if (sync && this.sameBasis(sync.before, basis)) {
      for (const intent of this.fields.values()) if (this.sameBasis(intent.base, basis)
        && intent.base.authority?.revision === sync.before.authority?.revision) intent.base = basis;
      if (this.draftBase && this.sameBasis(this.draftBase, basis)
        && this.draftBase.authority?.revision === sync.before.authority?.revision) this.draftBase = basis;
    }
    if (!sync && this.snapshotRemoteOrders.get(snapshot)! > this.installedRemoteOrder) this.syncNeedsRefresh = false;
    this.basis = basis;
    this.installedSequence = sequence;
    this.installedRemoteOrder = this.snapshotRemoteOrders.get(snapshot)!;
    this.installedState = this.snapshotStates.get(snapshot) ?? null;
    this.installedSnapshot = snapshot;
    if (this.localSnapshot && getCanvasSnapshotSequence(this.localSnapshot)! <= sequence) this.localSnapshot = snapshot;
    sync?.resolve(basis);
    // Wait until an edited scene is actually installed before rebasing a pending
    // selection sync. Its old scene/source basis cannot authorize a new scene.
    if (this.syncAgain && this.semanticPending === 0 && !this.syncPending) this.requestSelectionSync();
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
        else if (busy === false) {
          clearRemoteActivity();
          // A selection made during external work must observe the completed
          // accepted scene before it can synchronize with the server.
          if (this.syncAgain && !this.syncPending && this.semanticPending === 0) this.refreshSelectionBasis();
        }
      } catch { /* An invalid presentation hint grants no state or authority. */ }
    });
    events.onmessage = (event) => {
      const sequence = Number(event.data);
      if (sequence === this.eventSequence) return;
      this.eventSequence = sequence;
      void this.refresh(false).catch(() => {});
    };
    events.onerror = () => { clearRemoteActivity(); this.notice = "Disconnected — edits are not saved. Reconnecting to disk…"; this.eventSequence = -1; listener(); };
    return () => { events.close(); clearRemoteActivity(); if (this.events === events) { this.events = undefined; this.listener = undefined; this.syncReceipt?.resolve(); } };
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

const transientLocalCommands = new Set(["dimensions.hover", "dimensions.hover.clear", "dimensions.navigation.begin", "dimensions.navigation.end"]);
const localCommands = new Set([...transientLocalCommands, "view.fit", "view.origin", "view.grid.toggle", "dimensions.mode", "dimensions.pin", "dimensions.clearPins", "dimensions.focus", "selection.clear"]);
function changesSelection(method: string, input: unknown) {
  // All pointer requests on the local-capable path are semantic edits; their
  // authoritative created/dragged selection may intentionally change.
  return method === "pointer" || method === "cancel" || method === "dispatch" && ["selection.select", "declaration.select", "navigation.rows.select", "navigation.source.select", "sample.open", "project.new", "project.new-code", "project.import", "history.undo", "history.redo", "declaration.delete", "declaration.suppression.set", "tool.select", "tool.finish", "feature.apply"].includes((input as { command?: string })?.command ?? "");
}

function isPresentationCommand(command: string) {
  return command.startsWith("view.") || command.startsWith("navigation.") || command.startsWith("explorer.") || ["selection.select", "declaration.select", "source.select", "declaration.source.open"].includes(command) || localCommands.has(command);
}
