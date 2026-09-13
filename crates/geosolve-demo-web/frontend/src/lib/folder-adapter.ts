// SPDX-License-Identifier: GPL-3.0-or-later
import { assertWorkbenchSnapshot, getCanvasSnapshotSequence, stampCanvasSnapshot, markCanvasOnlySnapshot, type WorkbenchAdapter, type WorkbenchSnapshot, type PointerSample, type WheelSample, type SourceFileSnapshot } from "./adapter";
import { readOnlyWorkbenchPresentation } from "./workbench-session";
import type { GeneratorInputDefinition } from "../components/generator-inputs";
import { assertToolCatalog, type ToolCatalog } from "./tool-catalog";
import type { EditableDesign, WorkspaceViewPresentation } from "../../../../../packages/geosolve-engine/src/index";
import { auxiliaryTools } from "../../../../../packages/geosolve-engine/src/tool-catalog";
import { LocalBrowsingWorker, type LocalBrowsingClient, type BrowsingModel, type BrowsingInitialization, type BrowsingChrome } from "./collaboration-browsing-adapter";
import type { BrowsingEditCommand, BrowsingNavigationCommand } from "./collaboration-browsing-worker";
import { LocalAuthoringWorker, type LocalAuthoringClient, type AuthoringModel, type AuthoringPreview, type AuthoringView } from "./collaboration-authoring-adapter";
import { CollaborationAuthoringController, type AuthoringCommand } from "./collaboration-authoring-controller";
import { WorkbenchActivity } from "./workbench-activity";
import { LocalInteractionWorker, type LocalInteractionClient, type InteractionSeed, type LocalInteractionUpdate } from "./local-interaction-adapter";

export interface FolderAuthority { epoch: string; lease: number; revision: number; }
interface FolderBasis { hash: string | null; authority?: FolderAuthority; }
interface FieldIntent { base: FolderBasis; label: string; value: string }
interface RequestIntent { expected: FolderBasis; field?: string; fieldIntent?: FieldIntent }
export interface FolderModelSnapshot {
  format: "geosolve-folder-model-v1";
  revision: number;
  status: "accepted" | "retained";
  mode: "editable" | "generator";
  model: { project: string; design: EditableDesign; sourceDesignDigest: string } | null;
  generated?: unknown;
  source: WorkbenchSnapshot["source"];
  history: { canUndo: boolean; canRedo: boolean };
  problems: { id: string; detail: string; path?: string }[];
  seed: InteractionSeed;
  restoredViewPresentation?: WorkspaceViewPresentation;
}
interface BrowsingCandidate {
  key: string;
  model: BrowsingModel;
  rawSeed: InteractionSeed;
  presentation?: WorkspaceViewPresentation;
  savedPresentation?: WorkspaceViewPresentation;
  client?: LocalBrowsingClient;
  ready?: Promise<BrowsingInitialization>;
  initialized?: boolean;
  preparing?: number;
}
export interface FolderBrowserOptions {
  createBrowsing?: () => LocalBrowsingClient;
  authoring?: LocalAuthoringClient | null;
}

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
  private readonly fields = new Map<string, FieldIntent>();
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
  private view?: AuthoringView;
  private activeBrowsing?: BrowsingCandidate;
  private readonly snapshotBrowsing = new WeakMap<object, BrowsingCandidate>();
  private readonly candidates = new Map<string, BrowsingCandidate>();
  private catalog?: ToolCatalog;
  private initializationTail: Promise<unknown> = Promise.resolve();
  private readonly snapshotInstallReceipts = new WeakMap<object, (installed: boolean) => void>();
  private readonly pendingInstallReceipts = new Set<(installed: boolean) => void>();
  private authoring?: CollaborationAuthoringController;
  private readonly authoringBases = new WeakMap<object, FolderBasis>();
  private prediction?: AuthoringPreview;
  private acceptedFrame?: WorkbenchSnapshot["frame"];
  private browsingScheduled = false;
  private browsingAgain = false;
  private panning?: number;
  private blockedPointer?: number;
  private disposed = false;
  private semanticPending = 0;
  private externalBusy = false;
  get responsiveCanvas() { return this.localSnapshot !== undefined; }

  constructor(private readonly token: string, private readonly createLocal: () => LocalInteractionClient = () => new LocalInteractionWorker(), private readonly options: FolderBrowserOptions = {}) {
    try { this.pending = sessionStorage.getItem("geosolve.folder.pending") ?? ""; } catch { /* In-memory intent remains available. */ }
  }
  private captureIntent(method: string, input?: unknown): RequestIntent {
    const field = this.committingField;
    return { field, fieldIntent: field ? this.fields.get(field) : undefined,
      expected: method === "session.takeover" ? { hash: this.basis.hash, authority: this.state?.authority }
        : method === "dispatch" && (input as { command?: string })?.command === "source.prepare" && this.draftBase
          ? this.draftBase : this.fieldBase ?? this.basis };
  }
  private rpc<T>(method: string, input?: unknown, intent = this.captureIntent(method, input)): Promise<T> {
    // Capture the exact authority and field witness before any queue or native read.
    const { expected, field, fieldIntent } = intent;
    const operationId = crypto.randomUUID();
    const savedMutation = ["files.apply", "inputs.set", "authoring.commit", "authoring.mutation"].includes(method)
      || method === "dispatch" && ["source.prepare", "history.undo", "history.redo"].includes((input as { command?: string })?.command ?? "");
    const readOnly = ["snapshot", "operation.outcome", "recovery.inspect", "exportProject", "exportReproduction"].includes(method);
    const semantic = !readOnly && !["session.join", "session.takeover"].includes(method);
    if (semantic) this.semanticPending += 1;
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
        // Only authored transactions have durable receipts. Lease handoff is not replayed.
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
        if (savedMutation && (response.status === 409 || body.pendingSource)) {
          this.pending = JSON.stringify({ ...request, pendingSource: body.pendingSource }, null, 2);
          try { sessionStorage.setItem("geosolve.folder.pending", this.pending); } catch { this.notice += " Download pending intent before closing this tab."; }
        }
        this.listener?.();
        throw Error(this.notice);
      }
      // Fetching is observation. Only the host can acknowledge installation.
      if (body.result && typeof body.result === "object" && "format" in body.result && body.result.format === "geosolve-folder-model-v1") {
        this.snapshotHashes.set(body.result, { hash: body.state?.currentHash ?? null, authority: body.state?.authority });
        this.snapshotRemoteOrders.set(body.result, ++this.remoteOrder);
        if (body.state) this.snapshotStates.set(body.result, body.state);
        const result = body.result as unknown as FolderModelSnapshot;
        if (field && this.fields.get(field) === fieldIntent && !result.source.dirty && result.status === "accepted") this.fields.delete(field);
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
  private copyAuthority(from: object, to: WorkbenchSnapshot) {
    const basis = this.snapshotHashes.get(from);
    if (!basis) throw Error("Folder snapshot has no transport authority");
    this.snapshotHashes.set(to, basis);
    this.snapshotRemoteOrders.set(to, this.snapshotRemoteOrders.get(from)!);
    const state = this.snapshotStates.get(from), browsing = this.snapshotBrowsing.get(from);
    if (state) this.snapshotStates.set(to, state);
    if (browsing) this.snapshotBrowsing.set(to, browsing);
    const receipt = this.snapshotInstallReceipts.get(from);
    if (receipt) this.snapshotInstallReceipts.set(to, receipt);
    return stampCanvasSnapshot(assertWorkbenchSnapshot(to), ++this.sequence);
  }
  private readonlyPresentation(snapshot: WorkbenchSnapshot, state = this.snapshotStates.get(snapshot)) {
    if (!state || state.mode !== "generator" && state.editor?.canEdit !== false && state.capabilities?.geometryEditing !== false) return snapshot;
    const reason = state.editor?.canEdit === false ? "Another tab owns editing."
      : "Change generator inputs or edit the TypeScript files to regenerate this design.";
    return readOnlyWorkbenchPresentation({ ...snapshot, source: { ...snapshot.source, files: snapshot.source.files.map(file => ({ ...file, readOnly: true })) } }, reason);
  }
  private ensureBrowsing(candidate: BrowsingCandidate): Promise<BrowsingInitialization> {
    if (candidate.ready) return candidate.ready;
    this.candidates.set(candidate.key, candidate);
    const ready = this.initializationTail.then(async () => {
      if (this.disposed) throw Error("Folder workbench closed");
      const client = (this.options.createBrowsing ?? (() => new LocalBrowsingWorker()))();
      if (!client.initialize) { client.dispose(); throw Error("Native folder browsing initialization is unavailable"); }
      candidate.client = client;
      try {
        let initial: BrowsingInitialization;
        try { initial = await client.initialize(candidate.model, candidate.rawSeed, candidate.savedPresentation ?? candidate.presentation); }
        catch (error) {
          if (!candidate.savedPresentation) throw error;
          // Only the native codec interprets stored preferences. A corrupt or
          // obsolete personal envelope must not prevent the accepted model opening.
          candidate.savedPresentation = undefined;
          initial = await client.initialize(candidate.model, candidate.rawSeed, candidate.presentation);
        }
        if (this.disposed) throw Error("Folder workbench closed");
        candidate.initialized = true;
        this.pruneBrowsing(candidate);
        return initial;
      } catch (error) { candidate.client = undefined; client.dispose(); throw error; }
    });
    candidate.ready = ready;
    this.initializationTail = ready.catch(() => undefined);
    void ready.catch(() => { if (candidate.ready === ready) candidate.ready = undefined; });
    return ready;
  }
  private pruneBrowsing(keep: BrowsingCandidate) {
    for (const [key, candidate] of this.candidates) {
      if (candidate === keep || candidate === this.activeBrowsing || candidate.preparing || !candidate.initialized) continue;
      candidate.client?.dispose(); candidate.client = undefined; candidate.ready = undefined; candidate.initialized = false;
      this.candidates.delete(key);
    }
  }
  private async checked(remote: FolderModelSnapshot): Promise<WorkbenchSnapshot> {
    if (remote?.format !== "geosolve-folder-model-v1" || !Number.isSafeInteger(remote.revision) || !remote.seed) throw Error("Invalid folder model snapshot");
    const basis = this.snapshotHashes.get(remote), state = this.snapshotStates.get(remote);
    if (!basis) throw Error("Folder snapshot has no transport authority");
    const identity = { documentEpoch: `${basis.authority?.epoch ?? "folder"}:${basis.authority?.lease ?? 0}`, revision: basis.authority?.revision ?? remote.revision };
    const model: BrowsingModel = remote.mode === "generator"
      ? { ...identity, generated: JSON.stringify(remote.generated), sourceDesignDigest: `generated:${String(remote.seed.sceneKey)}` }
      : { ...remote.model!, ...identity };
    const key = JSON.stringify([identity, model.sourceDesignDigest, remote.seed.sceneKey]);
    let candidate = this.candidates.get(key);
    if (!candidate) { candidate = { key, model, rawSeed: remote.seed, presentation: remote.restoredViewPresentation, savedPresentation: this.readPersonalPresentation(state) }; this.candidates.set(key, candidate); }
    // Reconstruction is independent of both HTTP ordering and local canvas input.
    const initial = await this.ensureBrowsing(candidate);
    if (this.disposed) throw Error("Folder workbench closed");
    this.catalog ??= assertToolCatalog(initial.toolCatalog);
    let source = remote.source;
    if (remote.mode === "generator" && state?.sourceFiles?.length) source = { files: state.sourceFiles.map(file => ({ ...file, readOnly: true })), selectedPath: state.entry ?? state.sourceFiles[0].path, dirty: false };
    source = { ...source, selectedPath: source.files.find(file => file.path === this.selectedSourcePath)?.path ?? source.selectedPath };
    const snapshot = this.copyAuthority(remote, this.readonlyPresentation({ ...initial.snapshot,
      project: { ...initial.snapshot.project, status: remote.status === "accepted" ? "accepted" : "failed" }, source,
      problems: [...initial.snapshot.problems, ...remote.problems.map(problem => ({ id: problem.id, detail: problem.detail, file: problem.path, severity: "error" as const, title: "Folder source" }))],
      presentation: { ...initial.snapshot.presentation, ...remote.history },
    }, state));
    this.snapshotBrowsing.set(snapshot, candidate);
    return snapshot;
  }
  async construct() { return this.checked(await this.rpc<FolderModelSnapshot>("session.join")); }
  async takeOver() {
    if (!this.pendingOperationId && this.fieldBase !== undefined) this.pending = this.fieldDraft;
    this.retainedError = "";
    this.listener?.(await this.checked(await this.rpc<FolderModelSnapshot>("session.takeover")));
  }
  async snapshot() { return this.checked(await this.rpc<FolderModelSnapshot>("snapshot")); }
  async toolCatalog() {
    if (!this.catalog) throw Error("Folder tool catalog has not loaded");
    const unavailableReason = this.editingBlockedReason ?? (this.options.authoring === null ? "Authoring is unavailable in this session" : undefined);
    return { ...this.catalog, geometryRole: { ...this.catalog.geometryRole, unavailableReason }, sections: this.catalog.sections.map(section => ({ ...section, commands: section.commands.map(command => ({ ...command, unavailableReason: unavailableReason ?? command.unavailableReason })) })) };
  }
  async dispatch(input: { version: 2; command: string; payload?: unknown }): Promise<WorkbenchSnapshot> {
    if (localCommands.has(input.command)) return await this.localUpdate("dispatch", input) ?? this.currentSnapshot();
    if (input.command === "navigation.rows.select" || input.command === "navigation.source.select") return this.navigate(input.command, input.payload);
    if (structuredCommands.has(input.command)) return this.editStructured(input.command as BrowsingEditCommand, input.payload, this.captureIntent("authoring.mutation"));
    if (input.command === "source.select") {
      const snapshot = this.currentSnapshot(), path = (input.payload as { path?: string })?.path;
      if (!path || !snapshot.source.files.some(file => file.path === path)) throw Error("Source file is not in this accepted project");
      this.selectedSourcePath = path;
      this.localSnapshot = this.copyAuthority(snapshot, { ...snapshot, source: { ...snapshot.source, selectedPath: path } });
      return this.localSnapshot;
    }
    if (["source.prepare", "source.revert", "history.undo", "history.redo"].includes(input.command)) {
      if (this.state?.editor?.canEdit === false) throw Error(this.editingBlockedReason);
      const result = await this.checked(await this.rpc<FolderModelSnapshot>("dispatch", input));
      if (["source.prepare", "source.revert"].includes(input.command) && !result.source.dirty) this.draftBase = undefined;
      return result;
    }
    if (!this.authoring) throw Error("Local authoring is unavailable");
    if (this.editingBlockedReason && !(input.command === "tool.select" && (input.payload as { id?: string })?.id === "select")) throw Error(this.editingBlockedReason);
    if (input.command === "tool.select") {
      const id = (input.payload as { id: string }).id;
      if (id === "select") this.authoring.select(id);
      else await this.queueLocal(async () => { const start = await this.operationStart(); this.authoring!.select(id, start); });
      this.restoreAcceptedFrame();
    } else if (input.command === "tool.finish" || input.command === "feature.apply") this.authoring.finish();
    else if (input.command === "tool.step_back") this.authoring.stepBack();
    else if (input.command === "tool.construction.input") this.authoring.constructionEvent(input.payload as Parameters<CollaborationAuthoringController["constructionEvent"]>[0]);
    else if (input.command === "tool.operation.input") this.authoring.operationEvent(input.payload as Parameters<CollaborationAuthoringController["operationEvent"]>[0]);
    else if (input.command === "geometry.authoring-role.toggle") this.authoring.toggleRole();
    else if (input.command === "geometry.role.toggle") await this.queueLocal(async () => { this.authoring!.applyOperation(auxiliaryTools["geometry-role"], await this.operationStart()); });
    else throw Error(`Folder command is unavailable: ${input.command}`);
    return this.projectAuthoring();
  }
  private async operationStart() {
    if (!this.local?.authoringPointer || !this.view) throw Error("Accepted tool selection is still loading");
    const { viewport } = await this.local.authoringPointer({ x: 0, y: 0 });
    return { viewport, view: this.view };
  }
  private async navigate(command: BrowsingNavigationCommand, payload: unknown) {
    const snapshot = this.currentSnapshot(), candidate = this.activeBrowsing, view = this.view;
    const requested = payload as { authority?: unknown } | null;
    if (!candidate?.client || !view) throw Error("Local browsing is unavailable");
    if (!requested || typeof requested.authority !== "string" || requested.authority !== snapshot.navigation?.authority) throw Error("This navigation belongs to an older source revision");
    if (command === "navigation.source.select" && (snapshot.source.dirty || this.draftBase)) throw Error("Apply the source draft before navigating from its source");
    const current = await candidate.client.present(view);
    this.assertCurrentView(candidate, view);
    const authority = current.chrome.navigation?.authority;
    if (!authority) throw Error("Accepted navigation details are still loading");
    const result = await candidate.client.navigate(view, command, { ...(payload as Record<string, unknown>), authority });
    return this.queueLocal(async () => {
      this.assertCurrentView(candidate, view);
      const update = await this.local!.update("restoreSelection", { expected: view.state, state: result.state });
      if (update) {
        await this.installLocal(update); this.installChrome(result.chrome);
        if (this.view && this.authoring) { const projection = await this.local!.authoringPointer?.({ x: 0, y: 0 }); this.authoring.pickSelection(this.view, projection?.viewport); }
      }
      return this.projectAuthoring();
    });
  }
  private async editStructured(command: BrowsingEditCommand, payload: unknown, intent: RequestIntent) {
    if (this.editingBlockedReason) throw Error(this.editingBlockedReason);
    const candidate = this.activeBrowsing, view = this.view;
    if (!candidate?.client || !view) throw Error("Accepted Inspector details are still loading");
    const current = await candidate.client.present(view);
    this.assertCurrentView(candidate, view);
    const authority = current.chrome.authoringDocument?.authority;
    if (!authority) throw Error("Accepted Inspector details are still loading");
    const described = await candidate.client.describe(view, authority, command, payload);
    this.assertCurrentView(candidate, view);
    if (!described.mutation) return this.currentSnapshot();
    return this.checked(await this.rpc<FolderModelSnapshot>("authoring.mutation", { mutation: described.mutation }, intent));
  }
  private assertCurrentView(candidate: BrowsingCandidate, view: AuthoringView) {
    if (candidate !== this.activeBrowsing || JSON.stringify(view.state) !== JSON.stringify(this.view?.state)) throw Error("The accepted sketch or selection changed; review the current Inspector");
  }
  async managedCompilerContext(): Promise<{ version: 2; patches: Record<string, unknown> }> { throw Error("Folder compiler transactions settle in the engine host."); }
  pointer(input: PointerSample) {
    return this.queueLocal(async () => {
      if (!this.localSnapshot || !this.local) return null;
      if (input.phase === "down" && input.buttons === 4) this.panning = input.pointerId;
      const navigating = this.panning === input.pointerId;
      if (input.phase === "down") this.blockedPointer = this.editingBlockedReason || this.semanticPending || this.externalBusy ? input.pointerId : undefined;
      const blocked = Boolean(this.editingBlockedReason) || this.blockedPointer === input.pointerId;
      const drawing = !blocked && this.authoring && this.authoring.tool !== "select" && !navigating;
      const update = drawing ? null : await this.local.update("pointer", input);
      const painted = update ? await this.installLocal(update) : null;
      if (!navigating && !blocked && this.local.authoringPointer && this.view) {
        const projection = await this.local.authoringPointer({ x: input.x, y: input.y, captured: input.phase !== "down" });
        this.authoring?.pointer(input, projection, this.view);
      }
      if (input.phase === "up") { if (navigating) this.panning = undefined; if (this.blockedPointer === input.pointerId) this.blockedPointer = undefined; }
      return painted;
    });
  }
  async setGeneratorInputs(values: Record<string, unknown>) {
    if (!this.isGenerator || this.installedState?.capabilities?.generatorInputs === false) throw Error("This project does not expose generator inputs");
    if (this.state?.editor && !this.state.editor.canEdit) throw Error("Another tab owns editing. Take over editing to change generator inputs.");
    return this.checked(await this.rpc<FolderModelSnapshot>("inputs.set", { values: structuredClone(values) }));
  }
  wheel(input: WheelSample) { return this.localUpdate("wheel", input); }
  wheelBatch(input: WheelSample[]) { return input.length ? this.localUpdate("wheel", { version: 2, samples: input }) : Promise.resolve(null); }
  resize(input: { version: 2; width: number; height: number; pixelRatio: number }) { return this.localUpdate("resize", input); }
  cancel(input: { version: 2; reason: "escape" | "lost-capture" | "blur" }) {
    this.authoring?.cancel(); this.restoreAcceptedFrame(); this.panning = this.blockedPointer = undefined;
    return this.localUpdate("cancel", input);
  }
  exportProject() { return this.rpc<{ version: 2; filename: string; contents: string }>("exportProject"); }
  exportReproduction() { return this.rpc<{ version: 2; filename: string; contents: string }>("exportReproduction"); }
  async exportInteractionTrace() { return { version: 2 as const, filename: "folder-local-view.json", contents: JSON.stringify(await this.local?.state()) }; }
  async persistProject(): Promise<{ version: 2; contents: string }> { throw Error("Folder source is saved automatically by accepted transactions."); }

  /** Only an installable response may replace the accepted local browsing authority. */
  async prepareSnapshot(snapshot: WorkbenchSnapshot): Promise<WorkbenchSnapshot> {
    const candidate = this.snapshotBrowsing.get(snapshot);
    if (!candidate || !this.canInstall(snapshot)) return snapshot;
    if (candidate === this.activeBrowsing) return this.queueLocal(async () => {
      if (!this.canInstall(snapshot)) return snapshot;
      const current = this.currentSnapshot();
      return this.copyAuthority(snapshot, this.readonlyPresentation({ ...current,
        project: snapshot.project, source: snapshot.source, problems: snapshot.problems,
        presentation: { ...current.presentation, canUndo: snapshot.presentation.canUndo, canRedo: snapshot.presentation.canRedo },
      }, this.snapshotStates.get(snapshot)));
    });
    candidate.preparing = (candidate.preparing ?? 0) + 1;
    try {
      const initial = await this.ensureBrowsing(candidate);
      return await this.queueLocal(async () => {
        if (!this.canInstall(snapshot)) return snapshot;
        const previous = this.view;
        const existed = this.local !== undefined;
        this.local ??= this.createLocal();
        const update = existed ? await this.local.replace(initial.seed, true) : await this.local.construct(initial.seed);
        // A field can begin while native scene replacement is in flight. Restore
        // the old authority before local input resumes if that field vetoes install.
        if (!this.canInstall(snapshot)) {
          if (previous) {
            const restored = await this.local.replace(previous.seed, true);
            await this.local.update("restoreSelection", { expected: restored.state, state: previous.state });
          }
          return snapshot;
        }
        this.prediction = undefined; this.acceptedFrame = update.frame;
        this.view = { seed: initial.seed, state: update.state };
        this.activeBrowsing = candidate;
        this.localSnapshot = this.copyAuthority(snapshot, { ...snapshot, frame: update.frame });
        this.catalog = assertToolCatalog(initial.toolCatalog);
        if ("project" in candidate.model && this.options.authoring !== null) {
          this.authoring ??= new CollaborationAuthoringController(this.options.authoring ?? new LocalAuthoringWorker(), {
            paint: preview => this.presentPrediction(preview), changed: () => { if (this.localSnapshot) this.listener?.(this.projectAuthoring()); },
            cleared: () => this.restoreAcceptedFrame(), activity: () => this.activity.begin(),
            error: error => this.reportLocalError(error), commit: (model, command, kind) => this.commitGesture(model, command, kind),
          });
          this.authoringBases.set(candidate.model, this.snapshotHashes.get(snapshot)!);
          this.authoring.replace(candidate.model);
        } else { this.authoring?.dispose(); this.authoring = undefined; }
        this.pruneBrowsing(candidate);
        this.scheduleBrowsing();
        return this.projectAuthoring();
      });
    } finally { candidate.preparing!--; }
  }
  private currentSnapshot() {
    const result = this.localSnapshot ?? this.installedSnapshot;
    if (!result) throw Error("Folder workbench has not opened");
    return result;
  }
  private projectAuthoring() {
    const snapshot = this.currentSnapshot();
    const result = this.readonlyPresentation({ ...snapshot,
      authoringContext: this.authoring ? { construction: this.authoring.construction, operation: this.authoring.operation } : undefined,
      presentation: { ...snapshot.presentation, activeTool: this.authoring?.tool ?? "select", canFinish: this.authoring?.canFinish ?? false, geometryRole: this.authoring?.role ?? "profile" },
    }, this.snapshotStates.get(snapshot));
    this.localSnapshot = this.copyAuthority(snapshot, result);
    return this.localSnapshot;
  }
  private queueLocal<T>(operation: () => Promise<T>): Promise<T> {
    const run = this.localTail.then(operation, operation);
    this.localTail = run.catch(() => undefined); return run;
  }
  private localUpdate(method: "dispatch" | "pointer" | "wheel" | "resize" | "cancel", input: unknown) {
    return this.queueLocal(async () => {
      if (!this.local || !this.localSnapshot) return null;
      const update = await this.local.update(method, input);
      if (!update) return null;
      const snapshot = await this.installLocal(update);
      if (method === "dispatch" && personalCommands.has((input as { command: string }).command)) await this.savePersonalPresentation();
      return snapshot;
    });
  }
  private readPersonalPresentation(state?: FolderState): WorkspaceViewPresentation | undefined {
    if (!state?.paths.folder) return;
    try {
      const saved = localStorage.getItem(`geosolve.folder.view:${state.paths.folder}`);
      return saved ? JSON.parse(saved) as WorkspaceViewPresentation : undefined;
    } catch { return undefined; }
  }
  private async savePersonalPresentation() {
    const folder = this.snapshotStates.get(this.currentSnapshot())?.paths.folder;
    if (!folder || !this.local?.exportPresentation) return;
    try {
      const presentation = await this.local.exportPresentation();
      localStorage.setItem(`geosolve.folder.view:${folder}`, JSON.stringify(presentation));
    } catch (error) { this.reportLocalError(`View preferences could not be saved: ${String(error)}`); }
  }
  private async installLocal(update: LocalInteractionUpdate) {
    const changed = JSON.stringify(update.state) !== JSON.stringify(this.view?.state), prediction = this.prediction;
    this.acceptedFrame = update.frame;
    if (this.view) this.view = { seed: this.view.seed, state: update.state };
    if (changed && prediction?.presentation && this.local?.projectPrediction && this.view) {
      const construction = prediction.construction ? { preview: prediction.construction.preview, inference_guides: prediction.construction.inference_guides } : undefined;
      const projected = await this.local.projectPrediction({ presentation: prediction.presentation, view: this.view, construction });
      if (this.prediction === prediction) this.prediction = { ...prediction, view: this.view, frame: projected.frame };
    }
    const predicted = this.prediction && JSON.stringify(this.prediction.view.state) === JSON.stringify(this.view?.state) ? this.prediction.frame : undefined;
    const snapshot = markCanvasOnlySnapshot(this.copyAuthority(this.currentSnapshot(), { ...this.currentSnapshot(), frame: predicted ?? update.frame }));
    this.localSnapshot = snapshot;
    if (changed) { this.scheduleBrowsing(); if (this.view && (!prediction?.presentation || !this.local?.projectPrediction)) this.authoring?.render(this.view); }
    return snapshot;
  }
  private scheduleBrowsing() {
    if (!this.activeBrowsing?.client || !this.view) return;
    if (this.browsingScheduled) { this.browsingAgain = true; return; }
    this.browsingScheduled = true;
    void (async () => {
      do {
        this.browsingAgain = false;
        const candidate = this.activeBrowsing!, view = this.view!;
        try {
          const result = await candidate.client!.present(view);
          if (this.disposed || candidate !== this.activeBrowsing || JSON.stringify(result.view.state) !== JSON.stringify(this.view?.state)) continue;
          this.installChrome(result.chrome); this.listener?.(this.projectAuthoring());
        } catch (error) {
          if (!this.disposed && candidate === this.activeBrowsing && JSON.stringify(view.state) === JSON.stringify(this.view?.state)) this.reportLocalError(error);
        }
      } while (this.browsingAgain && !this.disposed);
    })().finally(() => { this.browsingScheduled = false; if (this.browsingAgain && !this.disposed) this.scheduleBrowsing(); });
  }
  private installChrome(value: BrowsingChrome) {
    const snapshot = this.currentSnapshot();
    const { authoringDocument, selection, selectedGeometryRole, constructionVisible, visibilityRestoreAvailable, ...chrome } = value;
    this.localSnapshot = this.copyAuthority(snapshot, this.readonlyPresentation({ ...snapshot, ...chrome,
      // Source failures belong to the host and survive read-only browser refresh.
      problems: [...chrome.problems, ...snapshot.problems.filter(problem => problem.title === "Folder source")],
      authoringDocument: authoringDocument ?? undefined, selection: selection ?? undefined,
      presentation: { ...snapshot.presentation, selectedGeometryRole: selectedGeometryRole ?? undefined,
        constructionVisible: constructionVisible ?? snapshot.presentation.constructionVisible,
        visibilityRestoreAvailable: visibilityRestoreAvailable ?? snapshot.presentation.visibilityRestoreAvailable },
    }, this.snapshotStates.get(snapshot)));
  }
  private restoreAcceptedFrame() {
    this.prediction = undefined;
    if (this.localSnapshot && this.acceptedFrame) this.localSnapshot = markCanvasOnlySnapshot(this.copyAuthority(this.localSnapshot, { ...this.localSnapshot, frame: this.acceptedFrame }));
  }
  private presentPrediction(preview: AuthoringPreview) {
    const model = this.activeBrowsing?.model;
    if (!this.localSnapshot || !model || preview.model.documentEpoch !== model.documentEpoch || preview.model.revision !== model.revision || preview.model.sourceDesignDigest !== model.sourceDesignDigest || JSON.stringify(preview.view.state) !== JSON.stringify(this.view?.state)) return;
    this.prediction = preview;
    this.localSnapshot = markCanvasOnlySnapshot(this.copyAuthority(this.localSnapshot, { ...this.localSnapshot, frame: preview.frame }));
    this.listener?.(this.localSnapshot);
  }
  private async commitGesture(model: AuthoringModel, command: AuthoringCommand, kind: "point" | "construction" | "operation") {
    const expected = this.authoringBases.get(model);
    if (model !== this.activeBrowsing?.model || !expected) throw Error("The accepted sketch changed during drawing; draw again on the current sketch");
    if (this.editingBlockedReason) throw Error(this.editingBlockedReason);
    const snapshot = await this.checked(await this.rpc<FolderModelSnapshot>("authoring.commit", { kind, command }, { expected }));
    if (!this.listener) return;
    const installed = new Promise<boolean>(resolve => {
      const receipt = (accepted: boolean) => { this.pendingInstallReceipts.delete(receipt); resolve(accepted); };
      this.pendingInstallReceipts.add(receipt); this.snapshotInstallReceipts.set(snapshot, receipt);
    });
    this.listener(snapshot);
    if (!await installed && !this.disposed) throw Error("The edit was saved; revert the pending draft and refresh to display it.");
  }
  private reportLocalError(error: unknown) {
    if (this.retainedError || this.pendingOperationId) return;
    this.notice = String(error); this.listener?.();
  }
  private canInstall(snapshot: WorkbenchSnapshot) {
    const basis = this.snapshotHashes.get(snapshot);
    if (!basis) throw Error("Folder snapshot has no transport authority");
    const sequence = getCanvasSnapshotSequence(snapshot)!, remoteOrder = this.snapshotRemoteOrders.get(snapshot)!;
    return (remoteOrder > this.installedRemoteOrder || remoteOrder === this.installedRemoteOrder && sequence >= this.installedSequence)
      && !(this.fieldBase !== undefined && !this.sameBasis(this.fieldBase, basis))
      && !(this.draftBase !== undefined && !this.sameBasis(this.draftBase, basis));
  }
  dispose() {
    if (this.disposed) return;
    this.disposed = true; this.authoring?.dispose(); this.local?.dispose();
    for (const receipt of this.pendingInstallReceipts) receipt(false);
    const browsers = new Set([...this.candidates.values(), this.activeBrowsing].flatMap(candidate => candidate?.client ? [candidate.client] : []));
    for (const browser of browsers) browser.dispose();
    this.candidates.clear(); this.local = undefined; this.localSnapshot = undefined;
    this.events?.close(); this.events = undefined; this.listener = undefined; this.activity.reset();
  }

  draftChanged(dirty: boolean) {
    if (dirty && this.draftBase === undefined) this.draftBase = this.basis;
    if (!dirty) this.draftBase = undefined;
  }
  /** Called after host validation and immediately before replacing displayed state. */
  installSnapshot(snapshot: WorkbenchSnapshot): boolean {
    if (!this.canInstall(snapshot)) { this.snapshotInstallReceipts.get(snapshot)?.(false); return false; }
    this.basis = this.snapshotHashes.get(snapshot)!;
    this.installedSequence = getCanvasSnapshotSequence(snapshot)!;
    this.installedRemoteOrder = this.snapshotRemoteOrders.get(snapshot)!;
    this.installedState = this.snapshotStates.get(snapshot) ?? null;
    this.installedSnapshot = snapshot;
    if (this.localSnapshot && getCanvasSnapshotSequence(this.localSnapshot)! <= this.installedSequence) this.localSnapshot = snapshot;
    this.snapshotInstallReceipts.get(snapshot)?.(true);
    return true;
  }
  // Display compatibility follows source/lease identity. A rejected source draft
  // can advance server revision without changing disk. Each captured edit still
  // sends its original exact revision; installing diagnostics never rebases it.
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
    const clearRemoteActivity = () => { this.externalBusy = false; finishRemoteActivity?.(); finishRemoteActivity = undefined; };
    events.addEventListener("activity", (event) => {
      if (this.events !== events) return;
      try {
        const { busy } = JSON.parse((event as MessageEvent<string>).data) as { busy?: unknown };
        if (busy === true) { this.externalBusy = true; finishRemoteActivity ??= this.activity.begin(); }
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

const localCommands = new Set(["explorer.visibility.set", "explorer.visibility.isolate", "explorer.visibility.restore", "view.construction.toggle", "view.fit", "view.origin", "view.grid.toggle", "dimensions.hover", "dimensions.hover.clear", "dimensions.navigation.begin", "dimensions.navigation.end", "dimensions.mode", "dimensions.pin", "dimensions.clearPins", "dimensions.focus", "selection.clear"]);
const structuredCommands = new Set(["parameter.edit", "dimensions.edit", "authoring.metadata.set", "authoring.parameter.extract", "declaration.move", "declaration.delete", "declaration.suppression.set"]);

const personalCommands = new Set(["explorer.visibility.set", "explorer.visibility.isolate", "explorer.visibility.restore", "view.construction.toggle", "dimensions.mode", "dimensions.pin", "dimensions.clearPins"]);
