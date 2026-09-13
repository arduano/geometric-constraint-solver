// SPDX-License-Identifier: GPL-3.0-or-later
import { createEngine, type Engine, type EditableSession, type AcceptedResult, type SourceWorkspacePresentation, type WorkspaceViewPresentation, type AuthoringReceipt, type PreparedAuthoring, type ConstructionCommand, type ToolOperationCommand, type PointGestureCommand, type AuthoringAction } from "@geosolve/engine";
import { applyManagedSketchMutation, compileManagedSource, type ManagedSketchMutation } from "@geosolve/sketch-code/managed";

const emptySource = '"use geosolve sketch";\nimport { sketch } from "@geosolve/sketch-code";\n\nexport default sketch(($) => {\n  return {};\n});\n';
const generatorCheckpoint = "GEOSOLVE_FOLDER_GENERATOR_V1\n";
const defaultView = (): WorkspaceViewPresentation => ({ hiddenRows: [], constructionVisible: true, dimensions: { mode: "focused", pins: [] } });
const file = (contents: string, filename = "geosolve-project.json") => ({ version: 2, filename, contents });
const accepted = (result: { status: string; diagnostics?: readonly { detail: string }[] }) => {
  if (result.status !== "accepted") throw Error(result.diagnostics?.map(item => item.detail).join("; ") || "Native candidate was rejected");
};

/** A folder's semantic authoring owner. No browser adapter, pointer loop or renderer lives here.
 * The surrounding storage actor captures a full checkpoint before applying a candidate,
 * publishes its files with disk CAS, and restores that checkpoint if publication fails.
 */
export class WorkspaceEngineRuntime {
  private engine!: Engine;
  private session?: EditableSession;
  private generated?: { artifact: Parameters<Engine["evaluateGenerated"]>[0]; result: AcceptedResult };
  private source!: SourceWorkspacePresentation;
  private view: WorkspaceViewPresentation = defaultView();
  private revision = 0;
  private problems: { id: string; detail: string; path?: string }[] = [];

  async construct(input: { persistedProject?: string } = {}) {
    const engine = await createEngine();
    let session: EditableSession | undefined;
    let generated: typeof this.generated;
    try {
      if (input.persistedProject?.startsWith(generatorCheckpoint)) {
        const artifact = JSON.parse(input.persistedProject.slice(generatorCheckpoint.length));
        const result = await engine.evaluateGenerated(artifact); accepted(result);
        generated = { artifact, result: result as AcceptedResult };
      } else if (input.persistedProject) {
        try { session = engine.restoreEditableWorkspace(input.persistedProject); }
        catch (workspaceError) {
          try { session = engine.openEditableSession(input.persistedProject, { persistableHistory: true }); }
          catch (projectError) { throw Error(`Cannot restore folder authority: ${workspaceError}; ${projectError}`); }
        }
      } else {
        const compiled = compileManagedSource(emptySource);
        const project = engine.compileProject({ project: "code-authored-sketch", compiled, customFiles: {}, artifacts: {}, lock: { format: "geosolve-lock-v1", modules: {} } });
        session = engine.openEditableSession(project, { persistableHistory: true });
      }
    } catch (error) { engine.dispose(); throw error; }
    this.session?.dispose(); this.engine?.dispose();
    this.engine = engine; this.session = session; this.generated = generated;
    this.source = session?.restoredWorkspacePresentation ?? this.cleanSource();
    this.view = session?.restoredViewPresentation ?? defaultView();
    this.problems = this.source.draftDiagnostic ? [{ id: "folder-source", path: "sketch.ts", detail: String(this.source.draftDiagnostic.message) }] : [];
    ++this.revision;
    return this.snapshot();
  }

  snapshot() {
    if (!this.engine) throw Error("Folder engine has not opened");
    const project = this.session?.exportProject();
    const document = project ? JSON.parse(project) : null;
    return {
      format: "geosolve-folder-model-v1", revision: this.revision,
      status: this.problems.length ? "retained" : "accepted",
      mode: this.generated ? "generator" : "editable",
      model: this.session ? { project, design: this.session.exportDesign(), sourceDesignDigest: this.session.sourceDesignDigest() } : null,
      generated: this.generated?.artifact,
      result: this.session?.accepted ?? this.generated!.result,
      source: { files: document ? [
        { path: "sketch.ts", contents: this.source.managedDraft, language: "typescript", readOnly: false },
        ...Object.values(document.custom_files).map((value: unknown) => { const item = value as { path: string; contents: string }; return { ...item, language: "typescript", readOnly: true }; }),
      ] : [], selectedPath: this.source.selectedFile, dirty: Boolean(document && this.source.managedDraft !== document.managed.source) },
      history: { canUndo: this.session?.state.can_undo ?? false, canRedo: this.session?.state.can_redo ?? false },
      problems: this.problems,
      seed: this.session ? this.session.interactionSeed() : this.engine.interactionSeed(this.generated!.result),
      restoredViewPresentation: this.view,
    };
  }

  async dispatch(input: { command: string; payload?: unknown }) {
    const payload = (input.payload ?? {}) as Record<string, unknown>;
    if (input.command === "project.new-code") return this.snapshot();
    if (input.command === "workspace.checkpoint.restore") return this.construct({ persistedProject: String(payload.contents) });
    if (input.command === "workspace.generator.apply") {
      const artifact = JSON.parse(String(payload.artifact));
      const result = await this.engine.evaluateGenerated(artifact); accepted(result);
      const previous = this.generated?.result ?? this.session?.accepted;
      this.session?.dispose(); this.session = undefined;
      this.generated = { artifact, result: result as AcceptedResult };
      if (previous) this.engine.release(previous);
      this.source = this.cleanSource(); this.problems = []; ++this.revision;
      return this.snapshot();
    }
    const session = this.editable();
    if (input.command === "workspace.project.apply") {
      if (payload.design !== undefined) {
        const candidate = this.engine.openEditableSession(payload.project, { design: payload.design as never, persistableHistory: true });
        this.session = candidate; session.dispose(); this.engine.release(session.accepted);
      } else await this.change(() => session.applyProject(payload.project, { expected: session.token }));
      this.source = this.cleanSource(); this.problems = []; ++this.revision;
      return this.snapshot();
    }
    if (input.command === "source.revert") {
      this.source = this.cleanSource(); this.problems = []; ++this.revision; return this.snapshot();
    }
    if (input.command === "source.prepare") {
      if (payload.path !== "sketch.ts" || typeof payload.contents !== "string") throw Error("Only the managed source entry can be applied");
      if (new TextEncoder().encode(payload.contents).length > 4 * 1024 * 1024) throw Error("Managed source exceeds 4 MiB");
      try {
        await this.author({ kind: "source", source: payload.contents });
        this.source = this.cleanSource(); this.problems = [];
      } catch (error) {
        const detail = boundedText(String(error), 64 * 1024);
        this.source = { ...this.source, managedDraft: payload.contents, draftDiagnostic: draftDiagnostic(error, payload.contents, detail) };
        this.problems = [{ id: "folder-source", path: "sketch.ts", detail }];
      }
      ++this.revision; return this.snapshot();
    }
    if (["history.undo", "history.redo"].includes(input.command)) {
      await this.change(() => input.command === "history.undo" ? session.undo({ expected: session.token }) : session.redo({ expected: session.token }));
      this.source = this.cleanSource(); this.problems = []; ++this.revision; return this.snapshot();
    }
    throw Error(`Unsupported folder authoring command: ${input.command}`);
  }

  async mutation(input: { mutation: ManagedSketchMutation }) {
    const mutation = input.mutation;
    if (mutation.mutation === "extract_parameter") {
      await this.author({ kind: "extract_parameter", declaration: mutation.declaration, path: mutation.path, presentation: mutation.presentation ?? {} });
    } else {
      // The native session owns allocation. Browser descriptions do not supply counters.
      const project = JSON.parse(this.editable().exportProject());
      await this.author({ kind: "mutation", mutation, candidate_name_high_water: project.managed.declaration_name_high_water ?? 0 });
    }
    this.source = this.cleanSource(); this.problems = []; ++this.revision;
    return this.snapshot();
  }

  async commit(input: { kind: "point"; command: PointGestureCommand } | { kind: "construction"; command: ConstructionCommand } | { kind: "operation"; command: ToolOperationCommand }) {
    const session = this.editable(); this.assertClean();
    if (input.kind === "point") {
      const prepared = session.preparePointGestureCommit(input.command, { expected: session.token });
      try { await this.change(() => session.applyPointGestureCommit(prepared)); }
      finally { session.releasePointGestureCommit(prepared); }
    } else if (input.kind === "construction") {
      const prepared = session.prepareConstruction(input.command, { expected: session.token });
      let candidate;
      try {
        candidate = session.resolveConstruction(prepared, this.receipt(prepared));
        await this.change(() => session.applyConstructionCommit(candidate!));
      } finally { session.releaseConstruction(candidate ?? prepared); }
    } else if (input.kind === "operation") {
      const prepared = session.prepareToolOperation(input.command, { expected: session.token });
      let candidate;
      try {
        candidate = session.resolveToolOperation(prepared, this.receipt(prepared));
        await this.change(() => session.applyToolOperationCommit(candidate!));
      } finally { session.releaseToolOperation(candidate ?? prepared); }
    } else throw Error("Unsupported folder authoring terminal");
    this.source = this.cleanSource(); this.problems = []; ++this.revision;
    return this.snapshot();
  }

  exportProject() {
    if (this.generated) return file(this.engine.exportWorkspace(this.generated.result));
    if (this.problems.length || this.source.managedDraft !== JSON.parse(this.editable().exportProject()).managed.source) throw Error("Canonical export is unavailable while source is invalid or unfinished");
    return file(this.editable().exportProject());
  }
  exportReproduction() {
    const saved = this.generated ? this.engine.exportWorkspace(this.generated.result) : this.persistProject().contents;
    return file(this.engine.encodeReproduction(saved), "geosolve-reproduction.txt");
  }
  exportWorkspaceDesign() { return this.editable().exportDesign(); }
  persistProject() {
    return file(this.generated ? generatorCheckpoint + JSON.stringify(this.generated.artifact) : this.editable().exportWorkspace(this.source, this.view));
  }
  async bakeProfile(maxChordErrorMm: number) { return this.engine.exportProfiles(this.session?.accepted ?? this.generated!.result, { chordErrorMm: maxChordErrorMm }); }
  dispose() { this.session?.dispose(); this.engine?.dispose(); }

  private editable() { if (!this.session) throw Error("Generator mode is read only. Change its inputs or TypeScript source to regenerate."); return this.session; }
  private cleanSource(): SourceWorkspacePresentation {
    const project = this.session && JSON.parse(this.session.exportProject());
    return { origin: { kind: "authored" }, selectedFile: "sketch.ts", managedDraft: project?.managed.source ?? "", draftDiagnostic: null };
  }
  private assertClean() { if (this.problems.length || this.source.managedDraft !== JSON.parse(this.editable().exportProject()).managed.source) throw Error("Apply or revert the source draft before editing geometry"); }
  private async change(action: () => unknown) {
    const previous = this.editable().accepted;
    const result = await action() as { status: string; diagnostics?: readonly { detail: string }[] };
    accepted(result);
    if (previous !== this.editable().accepted) this.engine.release(previous);
  }
  private async author(action: AuthoringAction) {
    if (action.kind !== "source") this.assertClean();
    const session = this.editable(), prepared = session.prepareAuthoring(action, { expected: session.token });
    try { await this.change(() => session.applyAuthoring(prepared, this.receipt(prepared))); }
    finally { session.releaseAuthoring(prepared); }
  }
  private receipt(prepared: Pick<PreparedAuthoring, "request">): AuthoringReceipt {
    const session = this.editable();
    const options = { patches: session.managedCompilerPatches() as never };
    const { request } = prepared;
    const receipt = request.candidateSource !== undefined ? (() => {
      const compiled = compileManagedSource(request.candidateSource, options);
      return { baseSourceDigest: request.current.ir.source_digest, candidateSourceDigest: compiled.ir.source_digest, compiled };
    })() : applyManagedSketchMutation(request.current, request.ticket.mutation as ManagedSketchMutation, options);
    return { ...receipt, ticketDigest: request.ticket.ticketDigest };
  }
}
function boundedText(value: string, limit: number) {
  const encoder = new TextEncoder(), decoder = new TextDecoder("utf-8", { fatal: true }), bytes = encoder.encode(value);
  if (bytes.length <= limit) return value;
  let end = limit - 3;
  while (end > 0) { try { return decoder.decode(bytes.slice(0, end)) + "…"; } catch { --end; } }
  return "…";
}
function draftDiagnostic(error: unknown, source: string, message: string) {
  const bytes = new TextEncoder().encode(source), decoder = new TextDecoder("utf-8", { fatal: true });
  const candidate = error as { span?: { start?: number; end?: number } };
  let span = { start: 0, end: 0 }, prefix = "";
  const { start, end } = candidate?.span ?? {};
  if (Number.isSafeInteger(start) && Number.isSafeInteger(end) && start! >= 0 && end! >= start! && end! <= bytes.length) {
    try {
      prefix = decoder.decode(bytes.slice(0, start));
      decoder.decode(bytes.slice(0, end));
      span = { start: start!, end: end! };
    } catch { prefix = ""; }
  }
  return { code: "unsupported_syntax", message, span, line: prefix.split("\n").length, column: Array.from(prefix.slice(prefix.lastIndexOf("\n") + 1)).length + 1 };
}
