// SPDX-License-Identifier: GPL-3.0-or-later
import { createHash } from "node:crypto";
import { resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { parentPort, workerData } from "node:worker_threads";
import { readWorkspaceSnapshot, evaluateWorkspaceSnapshot } from "./workspace-loader.mjs";
import { engineModuleUrl, sdkDirectory } from "./workspace-runtime-paths.mjs";

const managed = await import(pathToFileURL(resolve(sdkDirectory, "managed.js")).href);
const { capturedManagedDeclarationChanges } = await import(pathToFileURL(resolve(sdkDirectory, "apply-rebase.js")).href);
const { createEngine } = await import(engineModuleUrl);
const format = "geosolve-collaboration-model-v1";
const fail = (code, message) => { throw Object.assign(Error(message), { code }); };
const hash = (value) => createHash("sha256").update(value).digest("hex");
const sorted = (value) => Array.isArray(value) ? value.map(sorted) : value && typeof value === "object" ? Object.fromEntries(Object.keys(value).sort().map((key) => [key, sorted(value[key])])) : value;
const equal = (a, b) => JSON.stringify(sorted(a)) === JSON.stringify(sorted(b));
const projectValue = (value) => typeof value === "string" ? JSON.parse(value) : structuredClone(value);
const inputIdentity = (files, sourceDesignDigest) => hash(JSON.stringify({ format: "geosolve-collaboration-input-v1", files: Object.keys(files).sort().map((path) => [path, files[path]]), sourceDesignDigest }));
const exactPatch = (before, after) => ({ baseSourceDigest: hash(before), candidateSourceDigest: hash(after), edits: before === after ? [] : [{ start: 0, end: before.length, expected: before, replacement: after }] });

function validateFiles(files) {
  if (!files || typeof files !== "object" || Array.isArray(files)) fail("invalid_input", "Expected exact source files");
  let bytes = 0;
  for (const [path, source] of Object.entries(files)) {
    if (!path || path.startsWith("/") || path.includes("\\") || path.includes("\0") || path.split("/").some((part) => !part || part === "." || part === "..") || path === ".geosolve" || path.startsWith(".geosolve/")) fail("invalid_input", "Expected normalized authored file paths outside .geosolve");
    if (typeof source !== "string" || !source.isWellFormed() || Buffer.byteLength(source) > 4 * 1024 * 1024) fail("invalid_input", "Expected bounded well-formed UTF-8 source");
    bytes += Buffer.byteLength(source);
  }
  if (Object.keys(files).length > 512 || bytes > 16 * 1024 * 1024) fail("resource_limit", "Source tree exceeds 512 files / 16 MiB");
}
async function compile(folder, files) {
  validateFiles(files);
  const snapshot = readWorkspaceSnapshot(folder, { capturedFiles: files, sidecarPaths: [] });
  if (snapshot.mode !== "editable") fail("unsupported_mode", "Collaborative authoring requires editable mode");
  return evaluateWorkspaceSnapshot(snapshot, { timeoutMs: workerData.timeoutMs });
}
function compileProject(engine, compiled, brand = "collaborative-sketch") {
  // Native managed mutation receipts deliberately authenticate canonical source.
  // The exact authored tree belongs to SourceDocument and inputIdentity. Obtain
  // the native envelope through a real second compiler invocation, never by
  // replacing its input digest or dropping it from checkpoint comparison.
  const canonical = managed.compileManagedSource(compiled.compiled.normalizedSource, { patches: compiled.patches });
  return engine.compileProject({ project: brand, compiled: canonical, customFiles: compiled.customFiles, artifacts: compiled.artifacts, lock: compiled.lock });
}
function accepted(session) {
  const result = session.accepted, residual = result.validation.maximum_normalized_hard_residual;
  if (!result.validation.hard_residuals_validated || !result.validation.all_active_features_current || !Number.isFinite(residual) || residual > 1e-9) fail("invalid_result", "Native candidate lacks independent finite hard-residual validation");
  const finite = (value) => { if (typeof value === "number" && !Number.isFinite(value)) fail("invalid_result", "Native candidate contains nonfinite geometry"); if (value && typeof value === "object") Object.values(value).forEach(finite); };
  finite(result.geometry); return result;
}
function preserveAllocator(project, old) {
  const next = projectValue(project), high = old.managed.declaration_name_high_water ?? 0;
  if (high > (next.managed.declaration_name_high_water ?? 0)) next.managed.declaration_name_high_water = high;
  return next;
}
function modelFor(session, compiled) {
  return { format, entry: compiled.entry, project: session.exportProject(), design: session.exportDesign(), sourceDesignDigest: session.sourceDesignDigest(), patches: compiled.patches };
}
function inventory(compiled, project) {
  const entry = compiled.entry, object = (declaration) => `${entry}#${declaration}`, objects = [], properties = [];
  // Use Rust's validated equation-free projection for both property values and
  // dependency addresses. No alternate expression evaluator or label inference.
  const program = projectValue(project).managed.program;
  const declarations = [...(program.scalar_bindings ?? []), ...program.declarations];
  const variables = new Map(declarations.map((declaration) => [declaration.variable, declaration.symbol]));
  const refs = (value, into) => {
    if (!value || typeof value !== "object") return;
    if (value.kind === "reference") into.add(variables.get(value.value.declaration) ?? value.value.declaration);
    Object.values(value).forEach((child) => refs(child, into));
  };
  const visit = (value, declaration, path) => {
    properties.push({ object: object(declaration), declaration, path, value });
    if (value.kind === "object") for (const [name, child] of Object.entries(value.value)) visit(child, declaration, [...path, name]);
    else if (value.kind === "array") value.value.forEach((child, index) => {
      const key = child.kind === "object" && child.value.key;
      visit(child, declaration, [...path, key?.kind === "string" ? { member: key.value } : index]);
    });
  };
  for (const statement of declarations) {
    const declaration = statement.symbol;
    const value = statement.arguments ?? statement.value, dependencies = new Set(); refs(value, dependencies); dependencies.delete(declaration);
    objects.push({ object: object(declaration), declaration, dependencies: [...dependencies].sort().map(object) });
    visit(value, declaration, []);
  }
  return { objects, properties };
}
async function reopen(engine, folder, files, model) {
  if (!model || model.format !== format || Object.keys(model).sort().join(",") !== "design,entry,format,patches,project,sourceDesignDigest") fail("invalid_model", "Expected complete collaboration model checkpoint");
  const compiled = await compile(folder, files), saved = projectValue(model.project);
  if (model.entry !== compiled.entry || !equal(model.patches, compiled.patches)) fail("invalid_model", "Saved compiler context differs from exact source tree");
  const fresh = preserveAllocator(compileProject(engine, compiled, saved.project), saved);
  // Compare all compiler/artifact/source fields. Only the history-owned allocator
  // differs from a fresh compiler invocation and is retained explicitly above.
  if (!equal(fresh, saved)) fail("invalid_model", "Saved project differs from independently compiled source/artifacts");
  const session = engine.openEditableSession(model.project, { design: model.design });
  try {
    accepted(session);
    if (!equal(projectValue(session.exportProject()), saved) || !equal(session.exportDesign(), model.design)) fail("invalid_model", "Saved model required lifecycle reconciliation during rebuild");
    if (session.sourceDesignDigest() !== model.sourceDesignDigest) fail("invalid_model", "Rebuilt source/design identity differs from checkpoint");
    return { session, compiled };
  } catch (error) { session.dispose(); throw error; }
}
function output(session, compiled, files, extra = {}) {
  const result = accepted(session), model = modelFor(session, compiled);
  return { acceptedInput: inputIdentity(files, model.sourceDesignDigest), model, result, candidateFiles: files, inventory: inventory(compiled, model.project), pointTargets: session.pointGestureTargets(), ...extra };
}
const addressKey = (address) => JSON.stringify(sorted(address));
const pointTargetAddresses = (target) => target.target === "point" ? [target.address] : [target.lower_left, target.upper_right];
function pointChanges(entry, before, after, ownedAddresses) {
  if (!equal({ ...before, overrides: { ...before.overrides, drafts: [] } }, { ...after, overrides: { ...after.overrides, drafts: [] } })) fail("unsupported_point_change", "Point operation changed non-property design state");
  const previous = new Map(before.overrides.drafts.map(([address, draft]) => [addressKey(address), { address, draft }]));
  const next = new Map(after.overrides.drafts.map(([address, draft]) => [addressKey(address), { address, draft }]));
  const owned = new Map(ownedAddresses.map((address) => [addressKey(address), address]));
  return [...new Set([...previous.keys(), ...next.keys(), ...owned.keys()])].sort().flatMap((key) => {
    const old = previous.get(key), value = next.get(key);
    // An explicit same-value write remains an ownership barrier for another
    // user's earlier inverse; unchanged incidental companions claim nothing.
    if (equal(old?.draft ?? null, value?.draft ?? null) && !owned.has(key)) return [];
    const address = (value ?? old)?.address ?? owned.get(key), owner = address.owner.address;
    if (address.field !== "point") fail("unsupported_point_change", "Point replay changed a non-point semantic property");
    const declaration = owner.owner === "direct_declaration" ? owner.declaration : owner.owner === "generated_member" ? owner.address.invocation : undefined;
    if (!declaration) fail("unsupported_point_change", "Point replay has an unknown semantic owner");
    return [{ object: `${entry}#${declaration}`, declaration, address: sorted(address), before: old?.draft ?? null, after: value?.draft ?? null }];
  });
}
function checkedUpdate(update) {
  if (update.status !== "accepted") fail("candidate_rejected", update.diagnostics.map((item) => item.detail).join("; "));
  return update;
}
async function run(input) {
  if (input.kind === "reconcile") {
    validateFiles(input.workingFiles);
    const contribution = input.preparedContribution, workingFiles = { ...input.workingFiles }, reconciliations = [];
    if (contribution.kind === "values") {
      const { entry, current, mutation, acceptedSource, patch, patches } = contribution;
      if (!Object.hasOwn(workingFiles, entry)) reconciliations.push({ kind: "pending", path: entry, reason: "Working file was removed" });
      else {
        const outcome = managed.reconcileManagedSketchSourceMutation(current, mutation, { acceptedSource, workingSource: workingFiles[entry], acceptedPatch: patch }, { patches });
        if (outcome.status === "reconciled") { workingFiles[entry] = outcome.source; reconciliations.push({ kind: "reconciled", path: entry, patch: outcome.patch }); }
        else reconciliations.push({ kind: "pending", path: entry, reason: outcome.reason });
      }
    } else if (contribution.kind === "point") {
      const path = contribution.entry;
      if (typeof workingFiles[path] !== "string") reconciliations.push({ kind: "pending", path, reason: "Working file was removed" });
      else reconciliations.push({ kind: "reconciled", path, patch: exactPatch(workingFiles[path], workingFiles[path]) });
    } else if (contribution.kind === "apply") {
      for (const path of contribution.changedPaths) {
        if (contribution.candidateFiles[path] === contribution.capturedFiles[path]) reconciliations.push({ kind: "captured", path });
        else reconciliations.push({ kind: "pending", path, reason: "Rebased Apply differs from its immutable captured contribution" });
      }
    } else fail("invalid_input", "Unknown prepared contribution");
    return { reconciliations, workingFiles };
  }
  const engine = await createEngine(); let session;
  try {
    if (input.kind === "initialize") {
      const compiled = await compile(input.folder, input.files);
      const brand = input.design === undefined ? undefined : projectValue(input.design).project;
      session = engine.openEditableSession(compileProject(engine, compiled, brand), { design: input.design });
      return output(session, compiled, input.files);
    }
    const reopened = await reopen(engine, input.folder, input.files, input.model); session = reopened.session;
    if (input.expectedInput !== undefined && input.expectedInput !== inputIdentity(input.files, session.sourceDesignDigest())) fail("stale_input", "Domain job does not match its expected accepted input");
    if (input.kind === "rebuild") return output(session, reopened.compiled, input.files);
    if (input.kind === "scene") {
      const { createWorkspaceWorkbench } = await import("./workspace-workbench.mjs");
      const workbench = await createWorkspaceWorkbench({ timeoutMs: workerData.timeoutMs });
      try {
        await workbench.construct({ version: 2, persistedProject: session.exportProject() });
        await workbench.dispatch({ version: 2, command: "workspace.project.apply", payload: { project: session.exportProject(), design: session.exportDesign() } });
        if (input.viewport !== undefined) {
          const { width, height, pixelRatio = 1 } = input.viewport;
          if (![width, height, pixelRatio].every((value) => Number.isFinite(value) && value > 0)) fail("invalid_input", "Expected finite positive scene viewport");
          await workbench.resize({ version: 2, width, height, pixelRatio });
        }
        const scene = await workbench.interactionSnapshot();
        scene.toolCatalog = await workbench.toolCatalog();
        if (scene.snapshot.project.status !== "accepted" || scene.snapshot.source.dirty || scene.snapshot.pendingManagedMutation) fail("invalid_scene", "Workbench did not accept the independently rebuilt source/design");
        const exported = await workbench.exportProject(), design = await workbench.exportWorkspaceDesign();
        if (!equal(projectValue(exported.contents), projectValue(session.exportProject())) || !equal(design, session.exportDesign())) fail("invalid_scene", "Workbench scene changed accepted source/design authority");
        return { acceptedInput: inputIdentity(input.files, session.sourceDesignDigest()), scene, pointTargets: session.pointGestureTargets() };
      } finally { await workbench.dispose(); }
    }
    if (input.kind === "point_gesture" || input.kind === "point_properties") {
      const before = session.exportDesign(), project = session.exportProject();
      let ownedAddresses;
      if (input.kind === "point_gesture") {
        const prepared = session.preparePointGestureCommit(input.command, { expected: session.token });
        checkedUpdate(session.applyPointGestureCommit(prepared));
        if (session.sourceDesignDigest() !== prepared.source_design_digest) fail("invalid_result", "Installed gesture differs from its native prepared input");
        ownedAddresses = pointTargetAddresses(input.command.target);
      } else {
        if (!Array.isArray(input.writes) || input.writes.length < 1 || input.writes.length > 4096) fail("invalid_input", "Expected bounded point property writes");
        const drafts = new Map(before.overrides.drafts.map(([address, draft]) => [addressKey(address), [address, draft]])), seen = new Set();
        let currentPointKeys;
        for (const write of input.writes) {
          if (!write || Object.keys(write).sort().join(",") !== "address,expected,value" || write.address?.field !== "point") fail("invalid_input", "Expected exact native point address, expected draft and next draft");
          const key = addressKey(write.address);
          if (seen.has(key)) fail("invalid_input", "Point property occurs twice in one contribution"); seen.add(key);
          if (!equal(drafts.get(key)?.[1] ?? null, write.expected)) fail("stale_property", "Point property no longer matches its expected native draft");
          if (write.value === null && !drafts.has(key)) {
            // No draft remains for applyOverlay to authenticate. Check the
            // exact native writable inventory before accepting null -> null.
            currentPointKeys ??= new Set(session.pointGestureTargets().flatMap(({ target }) => pointTargetAddresses(target)).map(addressKey));
            if (!currentPointKeys.has(key)) fail("stale_property", "Absent point property has no current native writable target");
          }
          if (write.value === null) drafts.delete(key); else drafts.set(key, [write.address, write.value]);
        }
        checkedUpdate(await session.applyOverlay({ ...before.overrides, drafts: [...drafts.values()] }, { expected: session.token }));
        ownedAddresses = input.writes.map(({ address }) => address);
      }
      if (session.exportProject() !== project) fail("invalid_result", "Point publication changed its source project");
      const entry = reopened.compiled.entry;
      // SourceDocument advances its accepted model identity through an exact
      // identity patch. There is no authored source edit or draft recompilation.
      return output(session, reopened.compiled, input.files, { patches: [{ path: entry, patch: exactPatch(input.files[entry], input.files[entry]) }],
        preparedContribution: { kind: "point", entry }, pointChanges: pointChanges(entry, before, session.exportDesign(), ownedAddresses) });
    }
    if (input.kind === "values") {
      const prepared = session.prepareAuthoring({ kind: "values", writes: input.writes }, { expected: session.token });
      const current = prepared.request.current, mutation = prepared.request.ticket.mutation, entry = reopened.compiled.entry;
      const receipt = managed.applyManagedSketchSourceMutation(current, mutation, { source: input.files[entry], patches: input.model.patches });
      const canonical = managed.applyManagedSketchMutation(current, mutation, { patches: input.model.patches });
      if (canonical.compiled.canonicalIrJson !== receipt.compiled.canonicalIrJson || canonical.compiled.canonicalArtifactJson !== receipt.compiled.canonicalArtifactJson) fail("invalid_receipt", "Localized source differs from canonical native mutation");
      const update = checkedUpdate(session.applyAuthoring(prepared, { ticketDigest: prepared.request.ticket.ticketDigest, ...canonical }));
      const files = { ...input.files, [entry]: receipt.source }, compiled = { ...reopened.compiled, compiled: receipt.compiled };
      return output(session, compiled, files, { valueChanges: update.valueChanges, patches: [{ path: entry, patch: receipt.patch }], preparedContribution: { kind: "values", entry, current, mutation, acceptedSource: input.files[entry], patch: receipt.patch, patches: input.model.patches } });
    }
    if (input.kind === "apply") {
      const basisFiles = input.capture.acceptedBasis.files, capturedFiles = input.capture.working.files;
      validateFiles(basisFiles); validateFiles(capturedFiles);
      // Compile the exact capture first. A later accepted edit cannot disguise
      // malformed captured source or supply omitted dependencies from disk.
      const captured = await compile(input.folder, capturedFiles);
      const basis = await compile(input.folder, basisFiles);
      const latest = reopened.compiled;
      if (basis.entry !== latest.entry || captured.entry !== latest.entry) fail("reconciliation_conflict", "Concurrent entry lifecycle changes require explicit reconciliation");
      const files = Object.assign(Object.create(null), input.files), requiredStableDeclarations = [];
      requiredStableDeclarations.push(...capturedManagedDeclarationChanges(basisFiles[basis.entry], capturedFiles[captured.entry]));
      // A dependency change can alter every invocation without changing entry
      // text. Require the captured declaration lifetimes as a conservative guard.
      if (!equal(basis.patches, captured.patches)) requiredStableDeclarations.push(...inventory(basis, compileProject(engine, basis)).objects.map(({ declaration }) => declaration));
      if (requiredStableDeclarations.some((declaration) => input.invalidatedDeclarations?.includes(declaration))) fail("reconciliation_conflict", "Captured declaration lifetime changed");
      const paths = new Set([...Object.keys(basisFiles), ...Object.keys(capturedFiles), ...Object.keys(input.files)]);
      for (const path of paths) {
        const before = basisFiles[path], capture = capturedFiles[path], current = input.files[path];
        if (capture === before) continue;
        if (path === latest.entry && !equal(basis.patches, latest.patches)) fail("reconciliation_conflict", "Captured entry edit refers to a concurrently changed patch definition");
        if (path === latest.entry && equal(basis.patches, latest.patches) && equal(captured.patches, latest.patches)) {
          const rebased = managed.rebaseCapturedManagedSource({ basis: { source: before, compiled: basis.compiled }, capturedSource: capture, latest: { source: current, compiled: latest.compiled }, invalidatedDeclarations: input.invalidatedDeclarations }, { patches: latest.patches });
          files[path] = rebased.source; requiredStableDeclarations.push(...rebased.requiredStableDeclarations);
        } else {
          if (current !== before && current !== capture) fail("reconciliation_conflict", `Concurrent dependency or file lifecycle changes at ${path}`);
          if (path === latest.entry && current !== before) fail("reconciliation_conflict", "Concurrent entry edits across changed compiler context");
          if (capture === undefined) delete files[path]; else files[path] = capture;
        }
      }
      const compiled = await compile(input.folder, files);
      if (equal(latest.patches, compiled.patches) && equal(latest.lock, compiled.lock) && equal(latest.customFiles, compiled.customFiles)) {
        if (compiled.compiled.normalizedSource !== latest.compiled.normalizedSource) {
          const prepared = session.prepareAuthoring({ kind: "source", source: compiled.compiled.normalizedSource }, { expected: session.token });
          const receipt = managed.compileManagedSource(prepared.request.candidateSource, { patches: compiled.patches });
          checkedUpdate(session.applyAuthoring(prepared, { ticketDigest: prepared.request.ticket.ticketDigest, baseSourceDigest: prepared.request.current.ir.source_digest, candidateSourceDigest: receipt.ir.source_digest, compiled: receipt }));
        }
      } else {
        const project = preserveAllocator(compileProject(engine, compiled, projectValue(input.model.project).project), projectValue(input.model.project));
        checkedUpdate(await session.applyProject(project, { expected: session.token }));
      }
      const changedPaths = [...paths].filter((path) => input.files[path] !== files[path]).sort();
      const result = output(session, compiled, files, { requiredStableDeclarations: [...new Set(requiredStableDeclarations)].sort(), patches: changedPaths.filter((path) => typeof input.files[path] === "string" && typeof files[path] === "string").map((path) => ({ path, patch: exactPatch(input.files[path], files[path]) })), preparedContribution: { kind: "apply", changedPaths, candidateFiles: files, capturedFiles } });
      const propertyKey = ({ object, path }) => JSON.stringify([object, path]);
      const previous = new Map(inventory(latest, input.model.project).properties.map((property) => [propertyKey(property), property]));
      result.valueChanges = result.inventory.properties.flatMap((property) => {
        const before = previous.get(propertyKey(property));
        return before && !equal(before.value, property.value) ? [{ write: { declaration: property.declaration, path: property.path, value: property.value }, before: before.value }] : [];
      });
      return result;
    }
    fail("invalid_input", "Unknown domain job kind");
  } finally { session?.dispose(); engine.dispose(); }
}
try {
  const result = await run(JSON.parse(workerData.encoded));
  if (Buffer.byteLength(JSON.stringify(result)) > 128 * 1024 * 1024) fail("resource_limit", "Domain result exceeds 128 MiB");
  parentPort.postMessage({ ok: true, result });
} catch (error) { parentPort.postMessage({ ok: false, error: { code: error.code ?? "domain_rejected", message: String(error.message ?? error).slice(0, 8192), location: { ...(error.path ? { path: error.path } : {}) } } }); }
