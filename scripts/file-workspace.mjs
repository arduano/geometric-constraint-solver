#!/usr/bin/env node
// SPDX-License-Identifier: GPL-3.0-or-later
// M98 single-folder prototype. All geometry/compiler authority is existing Rust/WASM.
import { createWorkspaceWorkbench } from "./workspace-workbench.mjs";
import { evaluateProjectSnapshot } from "./workspace-evaluation.mjs";
import { readWorkspaceSnapshot, evaluateWorkspaceSnapshot } from "./workspace-loader.mjs";
import { acquireWorkspaceLock, createWorkspaceStorage } from "./workspace-storage.mjs";
import { createWorkspaceSession, isWorkspaceNavigation, isWorkspacePresentation, workspaceInteractionKey } from "./workspace-session.mjs";
import { runtimeRoot as root, demoWasmPath, engineModuleUrl, workbenchDist, starterDirectory } from "./workspace-runtime-paths.mjs";
import { createHash, randomBytes } from "node:crypto";
import { createServer } from "node:http";
import { existsSync, readFileSync, writeFileSync, mkdirSync, lstatSync, realpathSync,
  renameSync, linkSync, unlinkSync, openSync, fsyncSync, closeSync } from "node:fs";
import { resolve, dirname, basename, extname, sep } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const sourceLimit = 4 * 1024 * 1024;
export const hash = (text) => createHash("sha256").update(text).digest("hex");

function regular(path) {
  const stat = lstatSync(path);
  if (!stat.isFile() || stat.isSymbolicLink() || stat.size > sourceLimit) {
    throw Error(`Expected a regular file of at most 4 MiB: ${path}`);
  }
}
function readSource(path) {
  regular(path);
  return new TextDecoder("utf-8", { fatal: true }).decode(readFileSync(path));
}
function atomicWrite(path, text, expectedHash, recoveryDirectory) {
  const temporary = `${path}.m98-${randomBytes(8).toString("hex")}.tmp`;
  try {
    const descriptor = openSync(temporary, "wx", 0o600);
    try { writeFileSync(descriptor, text); fsyncSync(descriptor); }
    finally { closeSync(descriptor); }
    // Recheck after staging, immediately before publication; watch timing is irrelevant.
    if (expectedHash !== undefined && hash(readSource(path)) !== expectedHash) {
      throw Error("Conflict: disk changed before writeback. Disk and pending intent were retained; refresh and retry.");
    }
    if (expectedHash === undefined) renameSync(temporary, path);
    else {
      // A check followed by overwrite-rename has a race against an independent editor.
      // Claim and retain the displaced inode, then publish with an exclusive hard link.
      // Even a rename in that tiny interval can never be overwritten. Recovery files
      // also retain a writer which still holds an open descriptor to the displaced inode.
      const recovery = resolve(recoveryDirectory, `before-${Date.now()}-${randomBytes(6).toString("hex")}.ts`);
      renameSync(path, recovery);
      try {
        if (hash(readSource(recovery)) !== expectedHash) throw Error("Conflict: external text arrived during writeback.");
        linkSync(temporary, path); // Fails with EEXIST if another writer published.
      } catch (error) {
        try { linkSync(recovery, path); } catch { /* Newer disk text wins; displaced text stays in recovery. */ }
        throw Error(`Conflict or failed write: disk was preserved; refresh/retry. Recovery: ${recovery}. ${error}`);
      }
    }
  } finally { if (existsSync(temporary)) unlinkSync(temporary); }
}

/** Preserve authored licence/header comments outside the managed compiler's directive. */
export function preserveSourcePreamble(original, normalized) {
  const prefix = original.match(/^(?:(?:\s+)|(?:\/\/[^\n]*(?:\n|$))|(?:\/\*[\s\S]*?\*\/))*(?=["']use geosolve sketch["'])/)?.[0] ?? "";
  return prefix && !normalized.startsWith(prefix) ? prefix + normalized : normalized;
}

/** Both wires were independently admitted by Rust. Raw pre-normalization input
 * hashes can differ when the managed compiler retains a normalized receipt while
 * the folder preserves its original licence preamble. All source/IR/patch bytes
 * and authority-bearing fields must still agree. No receipt is modified or reused. */
function sameCompiledProject(left, right) {
  const a = JSON.parse(left), b = JSON.parse(right);
  delete a.managed.compiled.inputSourceDigest;
  delete b.managed.compiled.inputSourceDigest;
  return JSON.stringify(a) === JSON.stringify(b);
}

export function initProject(folder) {
  // An existing empty directory is allowed; any existing project file is refused.
  mkdirSync(folder, { recursive: true });
  const files = ["geosolve.json", "sketch.ts"];
  for (const name of files) if (existsSync(resolve(folder, name))) throw Error(`Refusing to overwrite ${resolve(folder, name)}`);
  const manifest = JSON.stringify({ format: "geosolve-folder-v2", entry: "sketch.ts", mode: "editable" }, null, 2) + "\n";
  for (const name of files) writeFileSync(resolve(folder, name), name === "geosolve.json" ? manifest : readFileSync(resolve(starterDirectory, name)), { flag: "wx" });
  return { ok: true, folder: resolve(folder), source: resolve(folder, "sketch.ts") };
}

export async function openProject(folder, { cache = true, storage } = {}) {
  folder = realpathSync(folder);
  const manifestPath = resolve(folder, "geosolve.json");
  const manifest = JSON.parse(readSource(manifestPath));
  const multi = manifest.format === "geosolve-folder-v2";
  if (!multi && (manifest.format !== "geosolve-folder-v1" || manifest.entry !== "sketch.ts" || Object.keys(manifest).some((key) => !["format", "entry"].includes(key)))) {
    throw Error("Expected geosolve-folder-v1 or geosolve-folder-v2 project manifest");
  }
  let inputsSnapshot = null;
  if (multi) {
    if (typeof manifest.entry !== "string" || !manifest.entry || manifest.entry.includes("\\")
      || manifest.entry.split("/").some((part) => !part || part === "." || part === "..")
      || !["editable", "generator"].includes(manifest.mode)) throw Error("Invalid folder entry or mode");
    try { inputsSnapshot = readWorkspaceSnapshot(folder); }
    catch { /* scan reports the located dependency error without preventing recovery. */ }
  }
  const entry = inputsSnapshot?.entry ?? (multi ? manifest.entry : "sketch.ts");
  const mode = inputsSnapshot?.mode ?? (multi ? manifest.mode : "editable");
  const sourcePath = resolve(folder, entry);
  const nativeEngine = multi ? await (await import(engineModuleUrl)).createEngine() : null;
  let acceptedCompilation = null;
  let sourceHash = null;
  let acceptedDesign = null;
  let acceptedProject = null;
  const sourceHistory = new Map();
  const designPath = ".geosolve/design.json";
  const rememberSources = (projectJson, captured) => {
    if (!projectJson || !captured) return;
    sourceHistory.set(hash(projectJson), captured);
    while (sourceHistory.size > 64) sourceHistory.delete(sourceHistory.keys().next().value);
  };
  const cachePath = resolve(folder, ".geosolve");
  if (cache) {
    mkdirSync(cachePath, { recursive: true });
    if (lstatSync(cachePath).isSymbolicLink() || realpathSync(cachePath) !== cachePath) throw Error("Project cache must be a local directory, not a symlink.");
  }
  const runtimeIdentity = hash(readFileSync(demoWasmPath));
  const adapter = await createWorkspaceWorkbench();
  try {
    await adapter.construct({ version: 2 });
    let snapshot = await adapter.dispatch({ version: 2, command: "project.new-code" });
    const apply = async (contents, captured, expectedDiskRevision = captured?.revision) => {
      if (multi) {
        if (!captured) throw Error("Complete project snapshot required");
        const compiled = await evaluateWorkspaceSnapshot(captured);
        if (readWorkspaceSnapshot(folder).revision !== expectedDiskRevision) throw Error("Conflict: project files changed during compilation");
        let next;
        if (compiled.mode === "generator") {
          next = await adapter.dispatch({ version: 2, command: "workspace.generator.apply", payload: { artifact: JSON.stringify(compiled.generated) } });
        } else {
          const project = nativeEngine.compileProject({ project: "code-authored-sketch", compiled: compiled.compiled,
            customFiles: compiled.customFiles, artifacts: compiled.artifacts, lock: compiled.lock });
          const diskDesign = captured.files.find((file) => file.path === designPath);
          const restoreDesign = diskDesign && (!acceptedCompilation || diskDesign.sha256 !== inputsSnapshot?.files.find((file) => file.path === designPath)?.sha256);
          next = !acceptedCompilation && !restoreDesign
            ? await adapter.construct({ version: 2, persistedProject: project })
            : await adapter.dispatch({ version: 2, command: "workspace.project.apply", payload: { project,
              ...(restoreDesign ? { design: JSON.parse(diskDesign.contents) } : {}),
            } });
        }
        if (readWorkspaceSnapshot(folder).revision !== expectedDiskRevision) throw Error("Conflict: project files changed during native evaluation");
        if (next.project.status === "accepted" && !next.source.dirty && !next.pendingManagedMutation) {
          snapshot = next;
          acceptedCompilation = compiled;
          if (mode === "editable") {
            acceptedDesign = JSON.stringify(await adapter.exportWorkspaceDesign());
            acceptedProject = (await adapter.exportProject()).contents;
            rememberSources(acceptedProject, captured);
          }
          return true;
        }
        throw Error(`Project evaluation rejected: ${next.problems.map((item) => item.detail).join("; ")}`);
      }
      const next = await adapter.dispatch({ version: 2, command: "source.prepare", payload: { path: "sketch.ts", contents } });
      snapshot = next;
      return snapshot.project.status === "accepted" && !snapshot.source.dirty && !snapshot.pendingManagedMutation;
    };
    let acceptedHash = null;
    let currentHash = null;
    let acceptedRevision = null;
    let sourceRevision = 0;
    let sequence = 0;
    let externalApplies = 0;
    let writes = 0;
    let ioError = null;
    let loadDiagnostic = null;
    let candidate = null;
    let candidateSince = 0;
    let notify = () => {};
    let gestureRollback = null;
    const session = createWorkspaceSession();
    const lastGoodPath = resolve(cachePath, "last-good.ts");
    const derivedPath = resolve(cachePath, "derived-session.json");
    const warnings = [];
    const sourceOf = (value) => value.source.files.find((file) => file.path === "sketch.ts")?.contents;
    const state = (clientId) => ({
      ...session.state(clientId),
      format: "geosolve-folder-status-v1", ok: !ioError && currentHash === acceptedHash && !snapshot.source.dirty && snapshot.project.status === "accepted",
      sourceFiles: mode === "generator" && acceptedCompilation ? (inputsSnapshot?.files ?? []).filter((file) => /\.[mc]?ts$/.test(file.path)).map((file) => ({ path: file.path, contents: file.contents, language: "typescript", readOnly: true })) : undefined,
      mode, entry, sourceHash, capabilities: { sourceEditing: mode === "editable", geometryEditing: mode === "editable", generatorInputs: mode === "generator" },
      inputDefinitions: acceptedCompilation?.inputDefinitions ?? null, inputs: acceptedCompilation?.inputs ?? null,
      warnings, sequence, revision: sourceRevision, acceptedRevision, workbenchRevision: snapshot.revision, currentHash, acceptedHash,
      status: ioError ? "error" : currentHash !== acceptedHash ? "stale" : snapshot.source.dirty ? "unsaved" : "saved",
      diagnostics: ioError ? [loadDiagnostic ?? { path: entry, detail: ioError }] : snapshot.problems,
      paths: { folder, source: sourcePath, manifest: manifestPath }, externalApplies, writes,
    });
    const publish = () => { sequence++; notify(sequence); };
    async function scan(force = false) {
      let rollback;
      let previous;
      try {
        let captured = multi ? readWorkspaceSnapshot(folder) : null;
        const text = captured ? captured.files.find((file) => file.path === entry).contents : readSource(sourcePath);
        let digest = captured?.revision ?? hash(text);
        if (digest === currentHash && (!force || !ioError)) return;
        if (captured && (captured.entry !== entry || captured.mode !== mode)) {
          throw Error("Project entry or mode changed; restart this folder bridge to install the new project mode");
        }
        if (!force) {
          if (candidate !== digest) { candidate = digest; candidateSince = Date.now(); return; }
          if (Date.now() - candidateSince < 120) return;
        }
        ioError = null;
        loadDiagnostic = null;
        currentHash = digest;
        sourceHash = captured?.sourceHash ?? digest;
        sourceRevision++;
        candidate = null;
        externalApplies++;
        session.endGesture();
        await adapter.cancel({ version: 2, reason: "blur" });
        gestureRollback = null;
        rollback = (await adapter.persistProject()).contents;
        previous = { acceptedCompilation, acceptedDesign, acceptedProject, acceptedHash, acceptedRevision, inputsSnapshot };
        // Repairing exactly the last accepted bytes is not a new authoring step.
        // Rejected complete projects already restored their accepted checkpoint.
        const accepted = digest === acceptedHash
          ? ((snapshot = multi ? await adapter.snapshot() : await adapter.dispatch({ version: 2, command: "source.revert" })), true)
          : await apply(text, captured);
        if (accepted && !multi && hash(readSource(sourcePath)) !== digest) throw Error("Conflict: disk changed during native evaluation");
        if (accepted) {
          if (multi && mode === "editable" && storage) {
            const bytes = acceptedDesign + "\n";
            const previous = captured.files.find((file) => file.path === designPath);
            if (previous?.sha256 !== hash(bytes)) {
              const id = `design-${randomBytes(16).toString("hex")}`;
              const publishedRevision = readWorkspaceSnapshot(folder, { fileOverrides: { [designPath]: bytes } }).revision;
              const outcome = storage.publish({ operationId: id,
                files: [{ path: designPath, expectedHash: previous?.sha256 ?? null, bytes }],
                expectedInputs: captured.files.map((file) => ({ path: file.path, expectedHash: file.sha256 }))
                  .concat(captured.absent.map((path) => ({ path, expectedHash: null }))),
              });
              if (!["published", "acknowledged"].includes(outcome.state)) throw Error(`Design publication requires recovery: ${outcome.state}`);
              storage.acknowledge(id);
              captured = readWorkspaceSnapshot(folder);
              if (captured.revision !== publishedRevision) throw Error("Conflict: project changed after design publication");
              currentHash = digest = captured.revision;
              rememberSources(acceptedProject, captured);
              writes++;
            }
          }
          acceptedHash = digest;
          if (captured) inputsSnapshot = captured;
          acceptedRevision = sourceRevision;
          if (cache && !multi) {
            try { atomicWrite(lastGoodPath, text); }
            catch (error) { warnings.push(`Derived cache update failed: ${error}`); }
          }
        }
        if (accepted) {
          // Opening must inspect the existing derived history before replacing it.
          if (previous.acceptedHash === null) await adapter.persistProject();
          else await saveDerived();
        }
        session.advance();
        publish();
      } catch (error) {
        if (rollback) {
          snapshot = await adapter.dispatch({ version: 2, command: "workspace.checkpoint.restore", payload: { contents: rollback } });
          acceptedCompilation = previous.acceptedCompilation;
          acceptedDesign = previous.acceptedDesign;
          acceptedProject = previous.acceptedProject;
          acceptedHash = previous.acceptedHash;
          acceptedRevision = previous.acceptedRevision;
          inputsSnapshot = previous.inputsSnapshot;
        }
        const detail = String(error);
        const diagnostic = { path: error.path ?? entry, detail,
          ...(error.code ? { code: error.code } : {}),
          ...(error.line ? { line: error.line, column: error.column ?? 1 } : {}) };
        const changed = ioError !== detail || JSON.stringify(loadDiagnostic) !== JSON.stringify(diagnostic);
        loadDiagnostic = diagnostic;
        // A dependency graph that cannot be read has no complete revision identity.
        // Retain acceptedHash separately; never claim the old hash describes invalid disk.
        if (!rollback) currentHash = null;
        ioError = detail;
        if (changed) { session.advance(); publish(); }
      }
    }
    async function saveDerived() {
      if (currentHash !== acceptedHash || ioError || snapshot.source.dirty || snapshot.pendingManagedMutation) return;
      // Recovery must retain the newest accepted design even when optional disk
      // caches are disabled. Navigation never calls this publication checkpoint.
      const contents = (await adapter.persistProject()).contents;
      if (!cache || !multi || mode !== "editable") return;
      try {
        const sources = [...sourceHistory];
        const record = { format: "geosolve-derived-session-v1", revision: acceptedHash,
          runtime: runtimeIdentity, project: acceptedProject, design: acceptedDesign, contents, sources };
        let json = JSON.stringify(record);
        while (Buffer.byteLength(json) > sourceLimit && sources.length) {
          sources.shift(); json = JSON.stringify(record);
        }
        if (Buffer.byteLength(json) > sourceLimit) throw Error("Derived history exceeds 4 MiB; source and semantic design remain saved");
        atomicWrite(derivedPath, json);
      } catch (error) { warnings.push(`Derived history/view cache was not saved: ${error}`); }
    }
    async function restoreDerived() {
      if (!cache || !multi || mode !== "editable" || currentHash !== acceptedHash || !existsSync(derivedPath)) return;
      const current = (await adapter.persistProject()).contents;
      try {
        const derived = JSON.parse(readSource(derivedPath));
        if (derived.format !== "geosolve-derived-session-v1" || derived.revision !== acceptedHash || derived.runtime !== runtimeIdentity
          || derived.project !== acceptedProject || derived.design !== acceptedDesign) return;
        const restored = await adapter.construct({ version: 2, persistedProject: derived.contents });
        if (restored.project.status !== "accepted" || restored.source.dirty || restored.pendingManagedMutation
          || (await adapter.exportProject()).contents !== acceptedProject
          || JSON.stringify(await adapter.exportWorkspaceDesign()) !== acceptedDesign) throw Error("Derived session does not reconstruct the current source and design");
        if (!Array.isArray(derived.sources) || derived.sources.length > 64) throw Error("Derived dependency history exceeds its bound");
        // Historical bytes are only candidates. Undo/Redo recompiles them against the
        // independently restored canonical project before any disk publication.
        for (const [key, captured] of derived.sources) if (typeof key === "string" && /^[a-f0-9]{64}$/.test(key)) sourceHistory.set(key, captured);
        snapshot = restored;
      } catch (error) {
        snapshot = await adapter.construct({ version: 2, persistedProject: current });
        warnings.push(`Derived history/view cache ignored: ${error}`);
      }
    }
    async function restorePreviousSources() {
      if (!cache || !multi || mode !== "editable" || acceptedHash !== null || !existsSync(derivedPath)) return;
      const empty = (await adapter.persistProject()).contents;
      try {
        const derived = JSON.parse(readSource(derivedPath));
        if (derived.format !== "geosolve-derived-session-v1" || derived.runtime !== runtimeIdentity
          || typeof derived.project !== "string" || !Array.isArray(derived.sources) || derived.sources.length > 64) throw Error("Invalid previous-source cache");
        const recorded = derived.sources.find(([key]) => key === hash(derived.project))?.[1];
        if (!recorded || !Array.isArray(recorded.files)) throw Error("Previous source graph is unavailable");
        const captured = readWorkspaceSnapshot(folder, { fileOverrides: Object.fromEntries(recorded.files.map((file) => [file.path, file.contents])) });
        if (captured.revision !== derived.revision || captured.entry !== entry || captured.mode !== mode) throw Error("Previous source identities do not match their recorded revision");
        const compiled = await evaluateWorkspaceSnapshot(captured);
        const project = nativeEngine.compileProject({ project: "code-authored-sketch", compiled: compiled.compiled,
          customFiles: compiled.customFiles, artifacts: compiled.artifacts, lock: compiled.lock });
        if (!sameCompiledProject(project, derived.project)) throw Error("Previous source does not compile to its recorded project");
        const design = captured.files.find((file) => file.path === designPath)?.contents;
        if (!design || JSON.stringify(JSON.parse(design)) !== derived.design) throw Error("Previous semantic design differs from its source snapshot");
        await adapter.construct({ version: 2, persistedProject: project });
        const restored = await adapter.dispatch({ version: 2, command: "workspace.project.apply", payload: { project, design: JSON.parse(design) } });
        if (restored.project.status !== "accepted" || restored.source.dirty || restored.pendingManagedMutation) throw Error("Previous source reconstruction was rejected");
        const restoredDesign = JSON.stringify(await adapter.exportWorkspaceDesign());
        const restoredProject = (await adapter.exportProject()).contents;
        await adapter.persistProject();
        snapshot = restored;
        acceptedCompilation = compiled;
        acceptedProject = restoredProject;
        acceptedDesign = restoredDesign;
        inputsSnapshot = captured;
        acceptedHash = captured.revision;
        acceptedRevision = 0;
        rememberSources(acceptedProject, captured);
        warnings.push("Current files are invalid. Previous source and semantic design were independently reconstructed; repair disk files before editing.");
      } catch (error) {
        snapshot = await adapter.construct({ version: 2, persistedProject: empty });
        warnings.push(`Previous source cache ignored: ${error}`);
      }
    }
    // Current authored files are primary. Read optional cache only after current
    // reconstruction fails, and never let malformed derived bytes block opening.
    await scan(true);
    await restorePreviousSources();
    if (!multi && acceptedHash === null && cache && existsSync(lastGoodPath)) {
      const rejected = (await adapter.persistProject()).contents;
      try {
        const previous = readSource(lastGoodPath);
        if (await apply(previous)) {
          acceptedHash = hash(previous);
          acceptedRevision = 0;
          if (currentHash !== null) await apply(readSource(sourcePath));
        } else {
          snapshot = await adapter.construct({ version: 2, persistedProject: rejected });
          warnings.push("Derived last-good cache is invalid; current source diagnostics retained.");
        }
      } catch (error) {
        snapshot = await adapter.construct({ version: 2, persistedProject: rejected });
        warnings.push(`Derived last-good cache ignored: ${error}`);
      }
    }
    await adapter.dispatch({ version: 2, command: "view.fit" });
    snapshot = await adapter.snapshot();
    await restoreDerived();

    const allowed = new Set(["snapshot", "toolCatalog", "dispatch", "pointer", "wheel", "wheelBatch", "resize", "cancel", "exportProject", "exportReproduction", "exportInteractionTrace"]);
    async function request(method, input, baseHash, context) {
      if (method === "operation.outcome") return storage?.outcome(input?.operationId) ?? null;
      if (method === "recovery.inspect") return storage?.list() ?? [];
      if (method === "recovery.resolve") {
        session.verify(context?.clientId, context?.authority);
        if (!storage) throw Error("Recovery requires the active folder bridge");
        const result = storage.resolve(input);
        await scan(true);
        return result;
      }
      if (method === "session.join") {
        session.join(context?.clientId);
        return adapter.snapshot();
      }
      if (method === "session.takeover") {
        session.takeover(context?.clientId, context?.authority);
        snapshot = await adapter.cancel({ version: 2, reason: "blur" });
        publish();
        return snapshot ?? adapter.snapshot();
      }
      if (!["files.apply", "inputs.set"].includes(method) && !allowed.has(method)) throw Error("Unsupported workspace method");
      if (method === "toolCatalog" || method.startsWith("export")) return adapter[method]();
      if (method === "snapshot") return adapter.snapshot();
      if (method === "dispatch" && ["workspace.project.apply", "workspace.generator.apply", "workspace.checkpoint.restore", "project.new", "project.new-code", "project.import", "sample.open"].includes(input?.command)) {
        throw Error("Folder mode keeps this project's sketch.ts open. Use the ordinary demo URL for New, samples or project import.");
      }
      const operationId = context?.operationId ?? randomBytes(16).toString("hex");
      const requestDigest = hash(JSON.stringify({ method, input, baseHash, clientId: context?.clientId }));
      const previousOperation = storage?.outcome(operationId);
      if (previousOperation) {
        if (previousOperation.requestDigest !== requestDigest) throw Error("Operation ID already belongs to different intent");
        if (!["published", "acknowledged"].includes(previousOperation.state)) throw Error(`Operation ${operationId} requires explicit recovery`);
        // A published write can outlive a lost acknowledgment and native rollback.
        // Install current disk authority before answering an operation retry.
        await scan(true);
        return adapter.snapshot();
      }
      if (["files.apply", "inputs.set"].includes(method)) {
        if (!multi || !storage) throw Error("Complete file transactions require a v2 folder bridge");
        session.verify(context?.clientId, context?.authority);
        const basis = readWorkspaceSnapshot(folder);
        if (baseHash !== basis.revision) {
          const error = Error("Conflict: project files changed since this transaction was prepared");
          error.conflict = true;
          throw error;
        }
        if (method === "inputs.set" && mode !== "generator") throw Error("Input values belong to generator projects");
        if (method === "inputs.set" && (!input?.values || typeof input.values !== "object" || Array.isArray(input.values))) throw Error("Generator input values must be an object");
        const fileOverrides = method === "inputs.set" ? {
          ".geosolve/inputs.json": JSON.stringify({ ...basis.inputs, ...input.values }, null, 2) + "\n",
        } : input?.files;
        if (!fileOverrides || typeof fileOverrides !== "object" || Array.isArray(fileOverrides) || !Object.keys(fileOverrides).length) throw Error("Expected candidate file contents");
        const candidate = readWorkspaceSnapshot(folder, { fileOverrides });
        if (candidate.entry !== entry || candidate.mode !== mode) throw Error("Changing entry or project mode requires restarting the folder bridge");
        const files = Object.entries(fileOverrides).map(([path, bytes]) => {
          if (!candidate.files.some((file) => file.path === path)) throw Error(`Candidate file ${path} is not part of the project dependency graph`);
          return { path, bytes, expectedHash: storage.read(path)?.hash ?? null };
        });
        const rollback = (await adapter.persistProject()).contents;
        const previous = { snapshot, acceptedCompilation, acceptedDesign, acceptedProject };
        let publishedSnapshot;
        try {
          await apply(candidate.files.find((file) => file.path === entry).contents, candidate, basis.revision);
          if (mode === "editable") {
            const bytes = acceptedDesign + "\n";
            const index = files.findIndex((file) => file.path === designPath);
            const design = { path: designPath, expectedHash: basis.files.find((file) => file.path === designPath)?.sha256 ?? null, bytes };
            if (index >= 0) files[index] = design;
            else if (hash(bytes) !== design.expectedHash) files.push(design);
          }
          const publishedRevision = readWorkspaceSnapshot(folder, { fileOverrides: Object.fromEntries(files.map((file) => [file.path, file.bytes])) }).revision;
          const outcome = storage.publish({ operationId, requestDigest, files,
            expectedInputs: basis.files.map((file) => ({ path: file.path, expectedHash: file.sha256 }))
              .concat(basis.absent.map((path) => ({ path, expectedHash: null }))),
          });
          if (!["published", "acknowledged"].includes(outcome.state)) throw Error(`Operation ${operationId} requires recovery: ${outcome.state}`);
          storage.acknowledge(operationId);
          publishedSnapshot = readWorkspaceSnapshot(folder);
          if (publishedSnapshot.revision !== publishedRevision) throw Error("Conflict: project changed after file publication");
        } catch (error) {
          snapshot = await adapter.dispatch({ version: 2, command: "workspace.checkpoint.restore", payload: { contents: rollback } });
          acceptedCompilation = previous.acceptedCompilation;
          acceptedDesign = previous.acceptedDesign;
          acceptedProject = previous.acceptedProject;
          try { if (readWorkspaceSnapshot(folder).revision !== acceptedHash) ioError = `Publication interrupted: ${error}`; }
          catch (readError) { ioError = `Publication interrupted: ${error}; ${readError}`; }
          session.advance();
          publish();
          error.conflict ||= error.code === "conflict" || error.code === "WORKSPACE_CONFLICT";
          throw error;
        }
        inputsSnapshot = publishedSnapshot;
        sourceHash = inputsSnapshot.sourceHash;
        currentHash = acceptedHash = inputsSnapshot.revision;
        sourceRevision++;
        acceptedRevision = sourceRevision;
        ioError = null;
        loadDiagnostic = null;
        writes++;
        rememberSources(acceptedProject, inputsSnapshot);
        await saveDerived();
        session.advance();
        publish();
        return snapshot;
      }
      const navigation = isWorkspaceNavigation(method, input, snapshot)
        || method === "pointer" && ["move", "up"].includes(input?.phase)
          && session.isNavigationGesture(context?.clientId, input?.pointerId);
      const authoredMutation = !navigation && !isWorkspacePresentation(method, input);
      if (context && navigation && !session.state(context.clientId).editor.canEdit) return adapter.snapshot();
      if (context) {
        if (method === "pointer" && input.phase === "down") session.beginGesture(context.clientId, context.authority, input.pointerId, { navigation });
        else session.verify(context.clientId, context.authority, { navigation, pointerId: method === "pointer" ? input.pointerId : undefined });
      }
      if (mode === "generator" && !navigation && !(
        method === "pointer" && !(input.buttons & 1 && input.phase === "move")
        || method === "dispatch" && ["selection.select", "declaration.select", "navigation.rows.select", "dimensions.focus"].includes(input?.command)
        || method === "dispatch" && input?.command === "tool.select" && input.payload?.id === "select"
      )) throw Error("Generator mode is read only. Change its inputs or TypeScript source to regenerate.");
      const diskIdentity = () => multi ? readWorkspaceSnapshot(folder).revision : hash(readSource(sourcePath));
      const beforeInteraction = workspaceInteractionKey(snapshot);
      if (authoredMutation && (ioError || acceptedHash !== currentHash)) {
        const error = Error("Conflict: current disk files are not accepted. Repair the files before editing retained geometry; pending intent was preserved.");
        error.conflict = true;
        throw error;
      }
      if (!navigation && (baseHash !== currentHash || diskIdentity() !== baseHash)) {
        await scan(true);
        const error = Error("Conflict: disk changed since this edit began. Disk and pending intent were retained. Refresh from disk, then retry explicitly.");
        error.conflict = true;
        throw error;
      }
      if (!multi && method === "pointer" && snapshot.presentation.activeTool === "select" && input.phase === "move" && (input.buttons & 1)) {
        snapshot = await adapter.cancel({ version: 2, reason: "escape" });
        throw Error("Point/grip dragging is deferred in folder mode because it stores native instance overlays. Edit a source value or an Inspector dimension instead.");
      }
      const before = sourceOf(snapshot);
      const diskSource = authoredMutation ? readSource(sourcePath) : null;
      const rollback = authoredMutation ? gestureRollback ?? (await adapter.persistProject()).contents : null;
      if (authoredMutation && method === "pointer" && input.phase === "down") gestureRollback = rollback;
      const previousAccepted = { acceptedCompilation, acceptedDesign, acceptedProject, inputsSnapshot,
        acceptedHash, currentHash, sourceHash, sourceRevision, acceptedRevision };
      try {
        const next = await adapter[method](input);
        if (!next) {
          if (method === "cancel" || (method === "pointer" && input.phase === "up")) { session.endGesture(); gestureRollback = null; }
          return null;
        }
        snapshot = next;
        let after = sourceOf(snapshot);
        if (!navigation && typeof after === "string" && diskSource) after = preserveSourcePreamble(diskSource, after);
        const settledDesign = multi && mode === "editable" && authoredMutation ? JSON.stringify(await adapter.exportWorkspaceDesign()) : acceptedDesign;
        const settledProject = multi && mode === "editable" && authoredMutation ? (await adapter.exportProject()).contents : acceptedProject;
        const changedSource = sourceOf(snapshot) !== before;
        const changedDesign = settledDesign !== acceptedDesign;
        const changedProject = settledProject !== acceptedProject;
        if (authoredMutation && snapshot.project.status === "accepted" && !snapshot.source.dirty && (changedSource || changedDesign || changedProject)) {
          let publishedSnapshot;
          try {
            if (typeof after !== "string") throw Error("This edit has no supported plaintext source.");
            if (!storage) throw Error("Source publication requires the active folder bridge");
            const files = [];
            if (!multi) files.push({ path: entry, expectedHash: baseHash, bytes: after });
            else {
              const history = method === "dispatch" && ["history.undo", "history.redo"].includes(input.command) ? sourceHistory.get(hash(settledProject)) : null;
              if (changedSource) files.push({ path: entry, expectedHash: sourceHash, bytes: history?.files.find((file) => file.path === entry)?.contents ?? after });
              if (history) {
                const historical = readWorkspaceSnapshot(folder, { fileOverrides: Object.fromEntries(history.files.map((file) => [file.path, file.contents])) });
                const compilation = await evaluateWorkspaceSnapshot(historical);
                const canonical = nativeEngine.compileProject({ project: "code-authored-sketch", compiled: compilation.compiled,
                  customFiles: compilation.customFiles, artifacts: compilation.artifacts, lock: compilation.lock });
                if (!sameCompiledProject(canonical, settledProject)) throw Error("Historical dependency bytes do not match the accepted Undo/Redo project");
                // The derived cache supplies candidate bytes, never trusted digests.
                // The loader has independently recomputed the historical graph here.
                for (const file of historical.files) {
                  if ([entry, "geosolve.json", designPath, ".geosolve/inputs.json"].includes(file.path)) continue;
                  if (inputsSnapshot.files.find((current) => current.path === file.path)?.sha256 !== file.sha256) {
                    files.push({ path: file.path, expectedHash: storage.read(file.path)?.hash ?? null, bytes: file.contents });
                  }
                }
              } else if (changedProject) {
                const project = JSON.parse(settledProject);
                for (const file of Object.values(project.custom_files)) {
                  const path = resolve(dirname(sourcePath), file.path).slice(folder.length + 1);
                  const current = storage.read(path);
                  if (current?.hash !== hash(file.contents)) files.push({ path, expectedHash: current?.hash ?? null, bytes: file.contents });
                }
              }
              if (changedDesign || !inputsSnapshot.files.some((file) => file.path === designPath)) {
                files.push({ path: designPath, expectedHash: inputsSnapshot.files.find((file) => file.path === designPath)?.sha256 ?? null, bytes: settledDesign + "\n" });
              }
            }
            if (!files.length) throw Error("No authored files represent the accepted design change");
            const publishedRevision = multi ? readWorkspaceSnapshot(folder, { fileOverrides: Object.fromEntries(files.map((file) => [file.path, file.bytes])) }).revision : hash(after);
            const outcome = storage.publish({ operationId,
              files,
              expectedInputs: multi ? inputsSnapshot.files.map((file) => ({ path: file.path, expectedHash: file.sha256 }))
                .concat(inputsSnapshot.absent.map((path) => ({ path, expectedHash: null }))) : [],
              requestDigest,
            });
            if (!["published", "acknowledged"].includes(outcome.state)) throw Error(`Operation ${operationId} requires recovery: ${outcome.state}`);
            storage.acknowledge(operationId);
            publishedSnapshot = multi ? readWorkspaceSnapshot(folder) : null;
            if ((publishedSnapshot?.revision ?? hash(readSource(sourcePath))) !== publishedRevision) throw Error("Conflict: project changed after accepted publication");
          } catch (error) {
            // Preserve the compiled candidate as well as the original command for manual recovery.
            error.pendingSource = after;
            error.conflict = String(error).includes("Conflict:");
            throw error;
          }
          writes++;
          sourceRevision++;
          if (multi) inputsSnapshot = publishedSnapshot;
          sourceHash = multi ? inputsSnapshot.sourceHash : hash(after);
          acceptedDesign = settledDesign;
          acceptedProject = settledProject;
          rememberSources(acceptedProject, inputsSnapshot);
          currentHash = acceptedHash = inputsSnapshot?.revision ?? sourceHash;
          acceptedRevision = sourceRevision;
          ioError = null;
          loadDiagnostic = null;
          candidate = null;
          if (cache && !multi) {
            try { atomicWrite(lastGoodPath, after); }
            catch (error) { warnings.push(`Source saved, but derived last-good cache failed: ${error}`); }
          }
          await saveDerived();
          publish();
        }
        if (workspaceInteractionKey(snapshot) !== beforeInteraction) session.advance();
        if (method === "cancel" || (method === "pointer" && input.phase === "up")) { session.endGesture(); gestureRollback = null; }
        return snapshot;
      } catch (error) {
        if (rollback) {
          // Native dispatch and semantic export are part of the same transaction as
          // publication. Any failure restores their complete accepted checkpoint.
          snapshot = await adapter.dispatch({ version: 2, command: "workspace.checkpoint.restore", payload: { contents: rollback } });
          ({ acceptedCompilation, acceptedDesign, acceptedProject, inputsSnapshot,
            acceptedHash, currentHash, sourceHash, sourceRevision, acceptedRevision } = previousAccepted);
          try { if (diskIdentity() !== acceptedHash) ioError = `Publication interrupted: ${error}`; }
          catch (readError) { ioError = `Publication interrupted: ${error}; ${readError}`; }
          session.endGesture();
          gestureRollback = null;
          session.advance();
          publish();
        }
        throw error;
      }
    }
    return { adapter, scan, request, state, cachePath, saveDerived, dispose: async () => { nativeEngine?.dispose(); await adapter.dispose(); }, setNotify: (callback) => { notify = callback; } };
  } catch (error) { nativeEngine?.dispose(); await adapter.dispose(); throw error; }
}

export async function serveProject(folder, { port = 0 } = {}) {
  const lock = acquireWorkspaceLock(folder);
  let storage;
  let project;
  try {
    storage = createWorkspaceStorage(lock.folder, { lock });
    storage.reconcile();
    project = await openProject(lock.folder, { storage });
  } catch (error) { lock.release(); throw error; }
  const token = randomBytes(24).toString("hex");
  const clients = new Set();
  let tail = Promise.resolve();
  let queued = 0;
  let closing = false;
  let busy = false;
  let activityAnnounced = false;
  const announceActivity = () => {
    for (const client of clients) client.write(`event: activity\ndata: ${JSON.stringify({ busy })}\n\n`);
    if (busy && clients.size) activityAnnounced = true;
  };
  const serial = (action) => {
    if (closing) return Promise.reject(Error("Folder bridge is closing"));
    if (queued >= 128) return Promise.reject(Error("Folder bridge request queue is full; retry after the current work completes"));
    queued++;
    const next = tail.then(async () => {
      busy = true;
      activityAnnounced = false;
      // Ordinary unchanged scans settle in microtasks. Announce only work that
      // yields to the event loop, so idle polling does not flood the browser.
      const activityTimer = setTimeout(announceActivity, 0);
      try { return await action(); }
      finally {
        clearTimeout(activityTimer);
        busy = false;
        if (activityAnnounced) announceActivity();
      }
    }).finally(() => { queued--; });
    tail = next.catch(() => {});
    return next;
  };
  project.setNotify((sequence) => { for (const client of clients) client.write(`data: ${sequence}\n\n`); });
  let origin;
  const send = (response, status, value) => { response.writeHead(status, { "Content-Type": "application/json", "Cache-Control": "no-store" }); response.end(JSON.stringify(value)); };
  const server = createServer(async (request, response) => {
    try {
      if (request.headers.host !== new URL(origin).host || (request.headers.origin && request.headers.origin !== origin)) {
        send(response, 403, { error: "Loopback origin required" }); return;
      }
      const url = new URL(request.url, origin);
      if (url.pathname.startsWith("/api/")) {
        if ((request.headers.authorization ?? `Bearer ${url.searchParams.get("token")}`) !== `Bearer ${token}`) {
          send(response, 403, { error: "Session token required" }); return;
        }
        if (url.pathname === "/api/events" && request.method === "GET") {
          response.writeHead(200, { "Content-Type": "text/event-stream", "Cache-Control": "no-store", Connection: "keep-alive" });
          clients.add(response);
          response.write(`event: activity\ndata: ${JSON.stringify({ busy })}\n\ndata: ${project.state().sequence}\n\n`);
          if (busy) activityAnnounced = true;
          request.on("close", () => clients.delete(response)); return;
        }
        if (url.pathname === "/api/status" && request.method === "GET") {
          send(response, 200, await serial(async () => { await project.scan(true); return project.state(); })); return;
        }
        if (url.pathname !== "/api/rpc" || request.method !== "POST" || request.headers["content-type"] !== "application/json") {
          send(response, 400, { error: "Expected JSON workspace RPC" }); return;
        }
        let body = "";
        for await (const chunk of request) { body += chunk; if (Buffer.byteLength(body) > sourceLimit + 65536) throw Error("Request too large"); }
        const rpc = JSON.parse(body);
        await serial(async () => {
          try { const result = await project.request(rpc.method, rpc.input, rpc.baseHash, { clientId: rpc.clientId, authority: rpc.authority, operationId: rpc.operationId }); send(response, 200, { result, state: project.state(rpc.clientId) }); }
          catch (error) { send(response, error.conflict ? 409 : 400, { error: String(error), pendingSource: error.pendingSource, state: project.state(rpc.clientId) }); }
        }); return;
      }
      if (request.method !== "GET") { send(response, 405, { error: "GET required" }); return; }
      const dist = process.env.GEOSOLVE_DIST ? resolve(root, process.env.GEOSOLVE_DIST) : workbenchDist;
      const path = resolve(dist, `.${decodeURIComponent(url.pathname === "/" ? "/index.html" : url.pathname)}`);
      if (!path.startsWith(dist + sep) || !realpathSync(path).startsWith(dist + sep)) throw Error("Unknown asset");
      const mime = { ".html": "text/html", ".js": "text/javascript", ".css": "text/css", ".wasm": "application/wasm", ".json": "application/json" }[extname(path)] ?? "text/plain";
      response.writeHead(200, { "Content-Type": mime, "Cache-Control": "no-store", "Referrer-Policy": "no-referrer" }); response.end(readFileSync(path));
    } catch (error) { if (!response.headersSent) send(response, 400, { error: String(error) }); else response.end(); }
  });
  try { await new Promise((resolveListen, reject) => { server.once("error", reject); server.listen(port, "127.0.0.1", resolveListen); }); }
  catch (error) { await project.dispose(); lock.release(); throw error; }
  origin = `http://127.0.0.1:${server.address().port}`;
  const url = `${origin}/?folder=1#token=${token}`;
  const session = { url, origin, token, pid: process.pid, ...project.state().paths };
  atomicWrite(resolve(project.cachePath, "session.json"), JSON.stringify(session, null, 2));
  let scanQueued = false;
  const timer = setInterval(() => {
    if (scanQueued || closing) return;
    scanQueued = true;
    void serial(() => project.scan()).catch(() => {}).finally(() => { scanQueued = false; });
  }, 100);
  const heartbeat = setInterval(() => { for (const client of clients) client.write(": connected\n\n"); }, 15000);
  const close = async () => {
    closing = true;
    clearInterval(timer); clearInterval(heartbeat);
    for (const client of clients) client.end();
    await tail;
    await project.saveDerived();
    await new Promise((done) => server.close(done));
    await project.dispose();
    lock.release();
  };
  return { ...session, project, storage, close };
}

export async function checkProject(folder) {
  const snapshot = readWorkspaceSnapshot(folder);
  const evaluated = await evaluateProjectSnapshot(snapshot);
  if (readWorkspaceSnapshot(folder).revision !== snapshot.revision) throw Error("Conflict: project files changed during validation");
  return { ok: true, mode: snapshot.mode, entry: snapshot.entry, revision: snapshot.revision,
    sourceHash: snapshot.sourceHash, validation: evaluated.result.validation,
    inputDefinitions: evaluated.inputDefinitions ?? null, inputs: evaluated.inputs ?? null,
    outputs: Object.keys(evaluated.result.named_outputs) };
}

export async function bakeProject(folder, output, chordErrorMm, namedOutput) {
  if (!Number.isFinite(chordErrorMm) || chordErrorMm <= 0) throw Error("--chord-error-mm must be finite and positive");
  if (typeof output !== "string" || !output) throw Error("Bake requires an output path");
  folder = realpathSync(folder);
  output = resolve(realpathSync(dirname(resolve(output))), basename(resolve(output)));
  if (output === folder || output.startsWith(folder + sep)) throw Error("Bake output must be outside the source project");
  if (existsSync(output) && (!lstatSync(output).isFile() || lstatSync(output).isSymbolicLink())) throw Error("Bake output must be a regular file, not a symlink");
  const captured = readWorkspaceSnapshot(folder);
  const evaluated = await evaluateProjectSnapshot(captured, { profiles: { chordErrorMm, ...(namedOutput === undefined ? {} : { output: namedOutput }) } });
  const geometry = { ...evaluated.profiles, source: { sha256: captured.sourceHash }, project: {
    revision: captured.revision, entry: captured.entry, mode: captured.mode, toolchain: captured.toolchain,
    files: captured.files.map(({ path, sha256 }) => ({ path, sha256 })), inputs: evaluated.inputs ?? captured.inputs,
    ...(namedOutput === undefined ? {} : { output: namedOutput }),
  } };
  const temporary = `${output}.m98-${randomBytes(8).toString("hex")}.tmp`;
  try {
    const descriptor = openSync(temporary, "wx", 0o600);
    try { writeFileSync(descriptor, JSON.stringify(geometry, null, 2) + "\n"); fsyncSync(descriptor); }
    finally { closeSync(descriptor); }
    if (readWorkspaceSnapshot(folder).revision !== captured.revision) throw Error("Bake conflict: disk changed during export; retry with the current complete project");
    renameSync(temporary, output);
  } finally { if (existsSync(temporary)) unlinkSync(temporary); }
  return { ok: true, output, source: { path: resolve(folder, captured.entry), sha256: captured.sourceHash },
    revision: captured.revision, validation: evaluated.result.validation,
    regions: geometry.regions.map(({ id, outer, holes }) => ({ id, outerVertices: outer.length, holes: holes.map((loop) => loop.length) })) };
}

async function main() {
  const [command, folder, ...options] = process.argv.slice(2);
  if (!folder || !["init", "serve", "open", "check", "status", "bake"].includes(command)) throw Error("Usage: node scripts/file-workspace.mjs init|serve|open|check|status <folder> [--port N]; bake <folder> --out <file> --chord-error-mm <positive number>");
  if (command === "bake") {
    const flags = new Map();
    for (let i = 0; i < options.length; i += 2) {
      if (!["--out", "--chord-error-mm", "--output"].includes(options[i]) || !options[i + 1] || flags.has(options[i])) throw Error("Expected --out <file> --chord-error-mm <positive number>");
      flags.set(options[i], options[i + 1]);
    }
    if (!flags.has("--out") || !flags.has("--chord-error-mm")) throw Error("Bake requires --out <file> and --chord-error-mm <positive number>");
    console.log(JSON.stringify(await bakeProject(folder, flags.get("--out"), Number(flags.get("--chord-error-mm")), flags.get("--output")), null, 2));
    return;
  }
  if (command === "init") { console.log(JSON.stringify(initProject(folder), null, 2)); return; }
  if (command === "status") {
    const session = JSON.parse(readSource(resolve(folder, ".geosolve/session.json")));
    const response = await fetch(`${session.origin}/api/status`, { headers: { Authorization: `Bearer ${session.token}` } });
    if (!response.ok) throw Error(`Status failed: ${response.status}`);
    console.log(JSON.stringify(await response.json(), null, 2)); return;
  }
  if (command === "check") {
    const result = await checkProject(folder);
    console.log(JSON.stringify(result, null, 2)); return;
  }
  if (options.length && (options.length !== 2 || options[0] !== "--port" || !/^\d+$/.test(options[1]))) throw Error("Expected --port N (0 chooses a fresh private port)");
  const session = await serveProject(folder, { port: Number(options[1] ?? 0) });
  console.log(JSON.stringify({ status: "PROTOTYPE_READY_FOR_UAT", ...session, project: undefined, storage: undefined, close: undefined }, null, 2));
  for (const signal of ["SIGINT", "SIGTERM"]) process.once(signal, () => { void session.close().then(() => process.exit(0)); });
}
if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  main().catch((error) => { console.error(JSON.stringify({ ok: false, error: String(error) })); process.exitCode = 1; });
}
