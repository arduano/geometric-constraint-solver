// SPDX-License-Identifier: GPL-3.0-or-later
import { createDomainSessionOwner } from "./collaboration-domain-sessions.mjs";
import { createHash } from "node:crypto";
import { resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { parentPort, workerData } from "node:worker_threads";
import { readWorkspaceSnapshot, evaluateWorkspaceSnapshot } from "./workspace-loader.mjs";
import { engineModuleUrl, sdkDirectory, collaborationHostModuleUrl } from "./workspace-runtime-paths.mjs";
import { capturedManagedPropertyTouches, capturedManagedMetadataTouches, declarationSourceProjection } from "./collaboration-domain-syntax.mjs";
import { enrichInventory, prepareStructuralSource, metadataChanges, compilerMetadata } from "./collaboration-domain-structure.mjs";
import { canonicalPropertyChanges, canonicalSourceWrites, deepestTouches, semanticPointLens, pointPropertyKey, pointCodec } from "./collaboration-domain-properties.mjs";

const managed = await import(pathToFileURL(resolve(sdkDirectory, "managed.js")).href);
const { capturedManagedDeclarationChanges } = await import(pathToFileURL(resolve(sdkDirectory, "apply-rebase.js")).href);
const { createEngine } = await import(engineModuleUrl);
const format = "geosolve-collaboration-model-v1";
// Private to this document worker. Inputs and compiler receipts are immutable;
// editable sessions leave the cache before any job can mutate them.
const compiledCache = new Map(), sessionCache = new Map();
let compilerBytes = 0, sessionBytes = 0, enginePromise, retainedScene;
let jobTimeoutMs = workerData.timeoutMs, documentFolder, toolchain, leases, outputs, sessions;
const retainedLimit = 32 * 1024 * 1024;
function compilerRemember(key, value) {
  const bytes = Buffer.byteLength(JSON.stringify(value));
  if (bytes > retainedLimit) return;
  compiledCache.set(key, { value, bytes }); compilerBytes += bytes;
  while (compiledCache.size > 8 || compilerBytes > retainedLimit) {
    const first = compiledCache.keys().next().value;
    compilerBytes -= compiledCache.get(first).bytes; compiledCache.delete(first);
  }
}
function sessionKey(files, model) { return hash(JSON.stringify(sorted({ files, model }))); }
function sessionRemember(record) {
  const key = sessionKey(record.files, record.model), bytes = Buffer.byteLength(JSON.stringify({ files: record.files, model: record.model }));
  if (bytes > retainedLimit) { sessions.close(record.session); return; }
  const old = sessionCache.get(key);
  if (old) { sessions.close(old.session); sessionBytes -= old.bytes; }
  sessionCache.set(key, { ...record, bytes }); sessionBytes += bytes;
  while (sessionCache.size > 4 || sessionBytes > retainedLimit) {
    const first = sessionCache.keys().next().value, old = sessionCache.get(first);
    sessionBytes -= old.bytes; sessions.close(old.session); sessionCache.delete(first);
  }
}
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
  if (toolchain !== undefined && toolchain !== snapshot.toolchain) fail("conflict", "SDK/compiler bytes changed while the document worker was open; restart required");
  toolchain = snapshot.toolchain;
  const key = snapshot.revision, cached = compiledCache.get(key);
  if (cached) { compiledCache.delete(key); compiledCache.set(key, cached); return cached.value; }
  const compiled = await evaluateWorkspaceSnapshot(snapshot, { timeoutMs: jobTimeoutMs });
  compilerRemember(key, compiled);
  return compiled;
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
function inventory(compiled, project, source = compiled.compiled.normalizedSource) {
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
  return enrichInventory(compiled, source, { objects, properties });
}
async function reopen(engine, folder, files, model, { writable = false } = {}) {
  if (!model || model.format !== format || Object.keys(model).sort().join(",") !== "design,entry,format,patches,project,sourceDesignDigest") fail("invalid_model", "Expected complete collaboration model checkpoint");
  const compiled = await compile(folder, files), saved = projectValue(model.project);
  if (model.entry !== compiled.entry || !equal(model.patches, compiled.patches)) fail("invalid_model", "Saved compiler context differs from exact source tree");
  const key = sessionKey(files, model), cached = sessionCache.get(key);
  if (cached && !writable) {
    sessionCache.delete(key); sessionBytes -= cached.bytes;
    leases.set(cached.session, cached);
    return { session: cached.session, compiled };
  }
  const fresh = cached ? saved : preserveAllocator(compileProject(engine, compiled, saved.project), saved);
  // Compare all compiler/artifact/source fields. Only the history-owned allocator
  // differs from a fresh compiler invocation and is retained explicitly above.
  if (!equal(fresh, saved)) fail("invalid_model", "Saved project differs from independently compiled source/artifacts");
  // Fresh writable sessions bound their native Undo/transcript history to this
  // one server transaction. Cached historical sessions are never mutated.
  const session = sessions.open(model.project, { design: model.design });
  try {
    accepted(session);
    if (!equal(projectValue(session.exportProject()), saved) || !equal(session.exportDesign(), model.design)) fail("invalid_model", "Saved model required lifecycle reconciliation during rebuild");
    if (session.sourceDesignDigest() !== model.sourceDesignDigest) fail("invalid_model", "Rebuilt source/design identity differs from checkpoint");
    leases.set(session, { session, compiled, files, model });
    return { session, compiled };
  } catch (error) { sessions.close(session); throw error; }
}
async function openReplayBasis(engine, input) {
  if (input.replayBasis && input.replayCheckpoint) fail("invalid_input", "Expected one trusted replay basis");
  if (!input.replayBasis && !input.replayCheckpoint) return undefined;
  if (!["point_gesture", "construction", "tool_operation"].includes(input.kind)) fail("invalid_input", "Historical replay is only available to semantic gestures");
  let source, semantic, historical;
  try {
    let basis = input.replayBasis;
    if (input.replayCheckpoint) {
      const saved = input.replayCheckpoint;
      const { createTrustedSourceHost, createTrustedSemanticHost } = await import(collaborationHostModuleUrl);
      source = await createTrustedSourceHost({ configuration: { documentEpoch: saved.documentEpoch, serverEpoch: saved.serverEpoch, initialInput: saved.initialInput, files: {} },
        actor: createHash("sha256").update(`replay:${saved.documentEpoch}`).digest(), checkpointJson: saved.source });
      const snapshot = source.snapshot().accepted;
      if (snapshot.modelRevision !== saved.revision || snapshot.acceptedInput !== saved.acceptedInput) fail("invalid_model", "Historical source and authority revision disagree");
      semantic = await createTrustedSemanticHost({ configuration: { documentEpoch: saved.documentEpoch, serverEpoch: saved.serverEpoch }, checkpoint: {
        documentEpoch: saved.documentEpoch, revision: saved.revision, targetsJson: saved.targets, historyJson: saved.history,
      } });
      for (const target of input.originalTargets ?? []) semantic.authenticate(target);
      basis = { files: snapshot.files, model: saved.model };
      if (inputIdentity(basis.files, basis.model.sourceDesignDigest) !== saved.acceptedInput) fail("invalid_model", "Historical source/design differs from its accepted identity");
    }
    if (basis.model.entry !== input.model.entry) fail("stale_input", "Gesture source entry changed since its original basis");
    historical = await reopen(engine, input.folder, basis.files, basis.model);
    // Native checkpoint restoration and genuine compiler reconstruction run in
    // this worker; neither can stall the HTTP host's text/navigation handlers.
    const generations = new Map();
    if (semantic) for (const item of inventory(historical.compiled, basis.model.project).objects) {
      const target = semantic.current(item.object);
      if (!target) fail("invalid_model", "Historical declaration has no native target lifetime");
      generations.set(item.declaration, target);
    }
    return { session: historical.session,
      stableTargets(declarations) {
        if (!input.replayCheckpoint) return [];
        return [...new Set(declarations)].sort().map(name => {
          const target = generations.get(name); if (!target) fail("stale_target", "Gesture dependency has no original accepted lifetime");
          return target;
        });
      },
    };
  } finally { source?.dispose(); semantic?.dispose(); }
}
function output(session, compiled, files, extra = {}) {
  const result = accepted(session), model = modelFor(session, compiled);
  const observed = inventory(compiled, model.project, files[compiled.entry]);
  for (const item of observed.objects) {
    const owns = (address) => pointOwnerDeclaration(address) === item.declaration;
    item.payload.overrides = { drafts: model.design.overrides.drafts.filter(([address]) => owns(address)),
      suppressed_children: model.design.overrides.suppressed_children.filter(([address]) => owns(address)) };
    if (model.design.generated.overrides.some(([, override]) => override.address.invocation === item.declaration)) item.payload.unsupported = "Legacy generated value override requires a native restoration API";
  }
  const value = { acceptedInput: inputIdentity(files, model.sourceDesignDigest), model, result, candidateFiles: files, inventory: observed,
    sourceProjection: declarationSourceProjection(compiled.compiled.normalizedSource, files[compiled.entry], compiled.entry), pointTargets: session.pointGestureTargets(), ...extra };
  outputs.set(value, { session, compiled, files, model });
  return value;
}
const pointOwnerDeclaration = (address) => address.owner.address.owner === "direct_declaration" ? address.owner.address.declaration : address.owner.address.address.invocation;
const addressKey = (address) => JSON.stringify(sorted(address));
const pointTargetAddresses = (target) => target.target === "point" ? [target.address] : [target.lower_left, target.upper_right];
function currentPointAddress(session, compiled, lens) {
  // A semantic generated-member lens also has owner.address, but that is its
  // member address rather than the allocation-stamped native owner wrapper.
  if (lens?.owner?.address?.owner === "direct_declaration" || lens?.owner?.address?.owner === "generated_member") return lens; // Legacy exact-generation address.
  const observed = inventory(compiled, session.exportProject()), candidates = session.pointGestureTargets().flatMap(({ target }) => pointTargetAddresses(target));
  const matching = candidates.filter((address) => equal(semanticPointLens(address, pointCodec(observed, address)), lens));
  const distinct = new Map(matching.map((address) => [addressKey(address), address]));
  if (distinct.size !== 1) fail("stale_property", "Semantic point lens is absent, replaced or ambiguous");
  return [...distinct.values()][0];
}
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
function observeStructuralChanges(before, after, explicitMutation) {
  const previous = new Map(before.objects.map((item) => [item.object, item])), current = new Map(after.objects.map((item) => [item.object, item]));
  const commonBefore = before.objects.filter((item) => current.has(item.object)).map((item) => item.object);
  const commonAfter = after.objects.filter((item) => previous.has(item.object)).map((item) => item.object);
  const orderChanged = !equal(commonBefore, commonAfter);
  const dependencies = [], reorders = [];
  for (const item of after.objects) {
    const old = previous.get(item.object); if (!old) continue;
    const values = explicitMutation?.mutation === "set_values" ? explicitMutation.values : explicitMutation?.mutation === "set_value" ? [explicitMutation] : [];
    const explicitDependency = values.some((write) => write.declaration === item.declaration && (write.expected.kind === "reference" || write.value.kind === "reference"));
    if (!equal(old.dependencies, item.dependencies) || explicitDependency) dependencies.push({ object: item.object, before: old.dependencies, after: item.dependencies });
    if (orderChanged && !equal(old.position, item.position) || explicitMutation?.mutation === "reorder_declaration" && explicitMutation.declaration === item.declaration) reorders.push({ object: item.object, before: old.position, after: item.position });
  }
  return { dependencies, reorders };
}
function observedValueChanges(before, after, explicit = []) {
  const key = ({ declaration, path }) => JSON.stringify([declaration, path]);
  const prior = new Map(before.properties.map((property) => [key(property), property])), touched = new Set(explicit.map(key));
  const changes = after.properties.flatMap((property) => {
    const old = prior.get(key(property));
    return old && (!equal(old.value, property.value) || touched.has(key(property))) ? [{ write: { declaration: property.declaration, path: property.path, value: property.value }, before: old.value }] : [];
  });
  // A native mutation inverse must never carry overlapping ancestor/child
  // writes in one set_values request. Explicit writes keep their declared
  // granularity; derived changes use the most specific available coordinate.
  const prefix = (a, b) => a.length < b.length && a.every((segment, index) => equal(segment, b[index]));
  return changes.filter(({ write }) => !changes.some(({ write: other }) => other.declaration === write.declaration
    && (touched.has(key(other)) && prefix(other.path, write.path) || !touched.has(key(write)) && prefix(write.path, other.path))));
}
function checkedUpdate(update) {
  if (update.status !== "accepted") fail("candidate_rejected", update.diagnostics.map((item) => item.detail).join("; "));
  return update;
}
async function evaluate(input) {
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
    } else if (contribution.kind === "structural") {
      const { entry, acceptedSource, source } = contribution;
      if (workingFiles[entry] === acceptedSource || workingFiles[entry] === source) {
        const patch = exactPatch(workingFiles[entry], source); workingFiles[entry] = source;
        reconciliations.push({ kind: "reconciled", path: entry, patch });
      } else reconciliations.push({ kind: "pending", path: entry, reason: "Structural inverse overlaps a newer working draft; accepted geometry is retained" });
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
  const engine = await (enginePromise ??= createEngine()); sessions ??= createDomainSessionOwner(engine); let session, replay;
  if (input.kind === "initialize") {
    const compiled = await compile(input.folder, input.files);
    const brand = input.design === undefined ? undefined : projectValue(input.design).project;
    session = sessions.open(compileProject(engine, compiled, brand), { design: input.design });
    leases.set(session, undefined);
    return output(session, compiled, input.files);
  }
  const reopened = await reopen(engine, input.folder, input.files, input.model, { writable: !["scene", "rebuild"].includes(input.kind) }); session = reopened.session;
  replay = await openReplayBasis(engine, input);
  if (input.expectedInput !== undefined && input.expectedInput !== inputIdentity(input.files, session.sourceDesignDigest())) fail("stale_input", "Domain job does not match its expected accepted input");
  if (input.kind === "rebuild") return output(session, reopened.compiled, input.files);
  if (input.kind === "scene") {
    const key = hash(JSON.stringify(sorted({ files: input.files, model: input.model, viewport: input.viewport ?? null })));
    if (retainedScene?.key === key) return retainedScene.result;
    // Native accepted geometry and exact bindings are the only server scene payload.
    // Each browser builds its frame/chrome through the shared read-only adapter.
    const result = { acceptedInput: inputIdentity(input.files, session.sourceDesignDigest()),
      scene: { seed: session.interactionSeed(input.viewport) }, pointTargets: session.pointGestureTargets() };
    retainedScene = Buffer.byteLength(JSON.stringify(result)) <= retainedLimit ? { key, result } : undefined;
    return result;
  }
  if (input.kind === "point_gesture" || input.kind === "point_properties") {
    const before = session.exportDesign(), project = session.exportProject();
    let ownedAddresses, replayWitness;
    if (input.kind === "point_gesture") {
      const prepared = replay ? session.preparePointGestureReplay(replay.session, input.command, { expected: session.token })
        : session.preparePointGestureCommit(input.command, { expected: session.token });
      replayWitness = prepared.replay;
      checkedUpdate(await sessions.change(session, () => session.applyPointGestureCommit(prepared)));
      if (session.sourceDesignDigest() !== prepared.source_design_digest) fail("invalid_result", "Installed gesture differs from its native prepared input");
      ownedAddresses = pointTargetAddresses(replayWitness?.resolvedTarget ?? input.command.target);
    } else {
      if (!Array.isArray(input.writes) || input.writes.length < 1 || input.writes.length > 4096) fail("invalid_input", "Expected bounded point property writes");
      const writes = input.writes.map((write) => ({ ...write, address: currentPointAddress(session, reopened.compiled, write.address) }));
      const drafts = new Map(before.overrides.drafts.map(([address, draft]) => [addressKey(address), [address, draft]])), seen = new Set();
      let currentPointKeys;
      for (const write of writes) {
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
      checkedUpdate(await sessions.change(session, () => session.applyOverlay({ ...before.overrides, drafts: [...drafts.values()] }, { expected: session.token })));
      ownedAddresses = writes.map(({ address }) => address);
    }
    if (session.exportProject() !== project) fail("invalid_result", "Point publication changed its source project");
    const entry = reopened.compiled.entry;
    // SourceDocument advances its accepted model identity through an exact
    // identity patch. There is no authored source edit or draft recompilation.
    const result = output(session, reopened.compiled, input.files, { patches: [{ path: entry, patch: exactPatch(input.files[entry], input.files[entry]) }],
      preparedContribution: { kind: "point", entry }, pointChanges: pointChanges(entry, before, session.exportDesign(), ownedAddresses) });
    result.propertyChanges = result.pointChanges.map(({ object, address, before, after }) => ({ object, property: pointPropertyKey(address, pointCodec(result.inventory, address)), before, after }));
    if (replay) {
      if (!replayWitness) fail("invalid_result", "Native point replay omitted its original dependency witness");
      result.requiredStableDeclarations = replayWitness.requiredStableDeclarations;
      result.requiredStableTargets = replay.stableTargets(replayWitness.requiredStableDeclarations);
    }
    return result;
  }
  if (input.kind === "structural_inverse") {
    const observed = inventory(reopened.compiled, input.model.project, input.files[reopened.compiled.entry]);
    const removed = new Set(input.inverse.structural.delete?.closure.map((target) => target.object) ?? []);
    const activeChanges = input.inverse.changes.filter((change) => !removed.has(change.address.target.object));
    const canonicalWrites = canonicalSourceWrites(reopened.compiled, observed, activeChanges);
    const inverse = { ...input.inverse, changes: activeChanges.filter((change) => JSON.parse(change.address.property)[0] !== "source") };
    for (const write of canonicalWrites) {
      const prior = observed.properties.find((item) => item.declaration === write.declaration && item.path.length === 0);
      const target = activeChanges.find((change) => change.address.target.object === `${reopened.compiled.entry}#${write.declaration}`).address.target;
      inverse.changes.push({ address: { target, property: "[]" }, before: prior.value, after: write.value });
    }
    const pointInverse = inverse.changes.filter((change) => JSON.parse(change.address.property)[0] === "point" && typeof JSON.parse(change.address.property)[1] === "object");
    inverse.changes = inverse.changes.filter((change) => !pointInverse.includes(change));
    const next = prepareStructuralSource(reopened.compiled, input.files[reopened.compiled.entry], inverse);
    if (next.compiled.normalizedSource !== reopened.compiled.compiled.normalizedSource) {
      const prepared = session.prepareAuthoring({ kind: "source", source: next.compiled.normalizedSource }, { expected: session.token });
      const receipt = managed.compileManagedSource(prepared.request.candidateSource, { patches: reopened.compiled.patches });
      checkedUpdate(await sessions.change(session, () => session.applyAuthoring(prepared, { ticketDigest: prepared.request.ticket.ticketDigest, baseSourceDigest: prepared.request.current.ir.source_digest, candidateSourceDigest: receipt.ir.source_digest, compiled: receipt })));
    }
    const restored = input.inverse.structural.create ?? [];
    if (restored.some((item) => item.payload.overrides?.drafts.length || item.payload.overrides?.suppressed_children.length)) {
      const design = session.exportDesign(), overlays = structuredClone(design.overrides);
      const targets = session.pointGestureTargets().flatMap(({ target }) => pointTargetAddresses(target));
      const remap = (address) => {
        const semanticAddress = ({ owner, ...rest }) => ({ ...rest, owner: owner.address });
        if (address.field === "point") {
          const candidates = targets.filter((target) => equal(semanticAddress(target), semanticAddress(address)));
          if (candidates.length !== 1) fail("reconciliation_conflict", "Restored point has no unique fresh native target");
          return candidates[0];
        }
        const owner = address.owner.address;
        const active = design.generated.active.filter(([member]) => owner.owner === "generated_member" ? equal(member, owner.address) : member.invocation === owner.declaration && member.template[0] === "direct");
        if (active.length !== 1) fail("reconciliation_conflict", "Restored suppression has no unique fresh native owner");
        return { ...address, owner: { address: owner, ...active[0][1] } };
      };
      for (const item of restored) for (const kind of ["drafts", "suppressed_children"]) for (const [address, value] of item.payload.overrides?.[kind] ?? []) {
        if (pointOwnerDeclaration(address) !== item.target.object.slice(reopened.compiled.entry.length + 1)) fail("reconciliation_conflict", "Restored overlay belongs to another object");
        const fresh = remap(address);
        if (overlays[kind].some(([current]) => equal(current, fresh))) fail("reconciliation_conflict", "Restored overlay conflicts with a retained property");
        overlays[kind].push([fresh, value]);
      }
      checkedUpdate(await sessions.change(session, () => session.applyOverlay(overlays, { expected: session.token })));
    }
    if (pointInverse.length) {
      const overlay = structuredClone(session.exportDesign().overrides);
      for (const change of pointInverse) {
        const lens = JSON.parse(change.address.property)[1], address = currentPointAddress(session, { ...reopened.compiled, compiled: next.compiled }, lens);
        if (`${reopened.compiled.entry}#${pointOwnerDeclaration(address)}` !== change.address.target.object) fail("stale_property", "Point inverse semantic target differs from native owner");
        const index = overlay.drafts.findIndex(([current]) => equal(current, address));
        if (!equal(index < 0 ? null : overlay.drafts[index][1], change.before)) fail("stale_property", "Point inverse draft no longer matches expected value");
        if (index >= 0) overlay.drafts.splice(index, 1);
        if (change.after !== null) overlay.drafts.push([address, change.after]);
      }
      checkedUpdate(await sessions.change(session, () => session.applyOverlay(overlay, { expected: session.token })));
    }
    const entry = reopened.compiled.entry, files = { ...input.files, [entry]: next.source }, compiled = { ...reopened.compiled, compiled: next.compiled };
    const result = output(session, compiled, files, { valueChanges: [], patches: [{ path: entry, patch: exactPatch(input.files[entry], next.source) }],
      preparedContribution: { kind: "structural", entry, acceptedSource: input.files[entry], source: next.source } });
    for (const item of [...(input.inverse.structural.create ?? []).map((item) => ({ target: item.target, after: item.dependencies })), ...(input.inverse.structural.dependencies ?? [])]) {
      if (input.inverse.structural.delete?.closure.some((target) => target.object === item.target.object)) continue;
      const actual = result.inventory.objects.find((object) => object.object === item.target.object);
      if (!actual || !equal(actual.dependencies, item.after.map((target) => target.object).sort())) fail("reconciliation_conflict", "Replayed compiler dependencies differ from native inverse");
    }
    return result;
  }
  if (["values", "mutation", "construction", "tool_operation"].includes(input.kind)) {
    const highWater = projectValue(input.model.project).managed.declaration_name_high_water ?? 0;
    const extraction = input.kind === "mutation" && input.mutation?.mutation === "extract_parameter";
    const prepared = input.kind === "construction" ? replay ? session.prepareConstructionReplay(replay.session, input.command, { expected: session.token })
      : session.prepareConstruction(input.command, { expected: session.token })
      : input.kind === "tool_operation" ? replay ? session.prepareToolOperationReplay(replay.session, input.command, { expected: session.token })
      : session.prepareToolOperation(input.command, { expected: session.token })
      : session.prepareAuthoring(extraction ? { kind: "extract_parameter", declaration: input.mutation.declaration, path: input.mutation.path, presentation: input.mutation.presentation }
        : input.kind === "values" ? { kind: "values", writes: input.writes }
        : { kind: "mutation", mutation: input.mutation, candidate_name_high_water: input.candidateNameHighWater ?? highWater }, { expected: session.token });
    const current = prepared.request.current, mutation = prepared.request.ticket.mutation, entry = reopened.compiled.entry;
    const receipt = managed.applyManagedSketchSourceMutation(current, mutation, { source: input.files[entry], patches: input.model.patches });
    const canonical = managed.applyManagedSketchMutation(current, mutation, { patches: input.model.patches });
    if (canonical.compiled.canonicalIrJson !== receipt.compiled.canonicalIrJson || canonical.compiled.canonicalArtifactJson !== receipt.compiled.canonicalArtifactJson) fail("invalid_receipt", "Localized source differs from canonical native mutation");
    let update;
    if (["construction", "tool_operation"].includes(input.kind)) {
      const receipt = { ticketDigest: prepared.request.ticket.ticketDigest, ...canonical };
      const candidate = input.kind === "construction" ? session.resolveConstruction(prepared, receipt) : session.resolveToolOperation(prepared, receipt);
      update = checkedUpdate(await sessions.change(session, () => input.kind === "construction" ? session.applyConstructionCommit(candidate) : session.applyToolOperationCommit(candidate)));
      if (session.sourceDesignDigest() !== candidate.source_design_digest) fail("invalid_result", "Tool installation differs from its native candidate");
    } else update = checkedUpdate(await sessions.change(session, () => session.applyAuthoring(prepared, { ticketDigest: prepared.request.ticket.ticketDigest, ...canonical })));
    const files = { ...input.files, [entry]: receipt.source }, compiled = { ...reopened.compiled, compiled: receipt.compiled };
    const result = output(session, compiled, files, { valueChanges: update.valueChanges ?? [], ...(["construction", "tool_operation"].includes(input.kind) ? { createdDeclarations: prepared.declarations } : {}),
      patches: [{ path: entry, patch: receipt.patch }], preparedContribution: { kind: "values", entry, current, mutation, acceptedSource: input.files[entry], patch: receipt.patch, patches: input.model.patches } });
    if (replay) {
      if (!prepared.replay) fail("invalid_result", "Native tool replay omitted its original dependency witness");
      result.requiredStableDeclarations = prepared.replay.requiredStableDeclarations;
      result.requiredStableTargets = replay.stableTargets(prepared.replay.requiredStableDeclarations);
      result.allocationMapping = prepared.replay.allocationMapping;
    }
    const prior = inventory(reopened.compiled, input.model.project, input.files[entry]);
    result.structuralChanges = observeStructuralChanges(prior, result.inventory, mutation);
    if (extraction) {
      result.createdDeclarations = result.inventory.objects.filter(item => !prior.objects.some(old => old.object === item.object)).map(item => item.declaration);
      result.allocationMapping = [{ provisional: input.mutation.symbol, persistent: mutation.symbol,
        provisionalVariable: input.mutation.variable, persistentVariable: mutation.variable }];
    }
    result.metadataChanges = metadataChanges(reopened.compiled, compiled, mutation);
    result.propertyChanges = canonicalPropertyChanges(reopened.compiled, prior, compiled, result.inventory,
      mutation.mutation === "set_values" ? mutation.values : mutation.mutation === "set_value" ? [mutation] : [], mutation);
    if (input.kind === "mutation") result.valueChanges = observedValueChanges(prior, result.inventory,
      mutation.mutation === "set_values" ? mutation.values : mutation.mutation === "set_value" ? [mutation] : []);
    return result;
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
    const basisInventory = inventory(basis, compileProject(engine, basis), basisFiles[basis.entry]);
    const capturedTouches = capturedManagedPropertyTouches(basisFiles[basis.entry], capturedFiles[captured.entry], basisInventory.properties);
    requiredStableDeclarations.push(...capturedTouches.map(({ declaration }) => declaration));
    const metadataTouches = capturedManagedMetadataTouches(basisFiles[basis.entry], capturedFiles[captured.entry], [...compilerMetadata(basis), ...compilerMetadata(captured)]);
    requiredStableDeclarations.push(...metadataTouches.map(({ target }) => target.declaration ?? "@document"));
    // A dependency change can alter every invocation without changing entry
    // text. Require the captured declaration lifetimes as a conservative guard.
    if (!equal(basis.patches, captured.patches)) requiredStableDeclarations.push(...basisInventory.objects.map(({ declaration }) => declaration));
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
        checkedUpdate(await sessions.change(session, () => session.applyAuthoring(prepared, { ticketDigest: prepared.request.ticket.ticketDigest, baseSourceDigest: prepared.request.current.ir.source_digest, candidateSourceDigest: receipt.ir.source_digest, compiled: receipt })));
      }
    } else {
      const project = preserveAllocator(compileProject(engine, compiled, projectValue(input.model.project).project), projectValue(input.model.project));
      checkedUpdate(await sessions.change(session, () => session.applyProject(project, { expected: session.token })));
    }
    const changedPaths = [...paths].filter((path) => input.files[path] !== files[path]).sort();
    const result = output(session, compiled, files, { requiredStableDeclarations: [...new Set(requiredStableDeclarations)].sort(), patches: changedPaths.filter((path) => typeof input.files[path] === "string" && typeof files[path] === "string").map((path) => ({ path, patch: exactPatch(input.files[path], files[path]) })), preparedContribution: { kind: "apply", changedPaths, candidateFiles: files, capturedFiles } });
    result.structuralChanges = observeStructuralChanges(inventory(latest, input.model.project, input.files[latest.entry]), result.inventory);
    result.metadataChanges = metadataChanges(latest, compiled);
    const canonicalTouches = capturedManagedPropertyTouches(basisFiles[basis.entry], capturedFiles[captured.entry], [...basisInventory.properties, ...result.inventory.properties]);
    result.propertyChanges = canonicalPropertyChanges(latest, inventory(latest, input.model.project, input.files[latest.entry]), compiled, result.inventory, deepestTouches(canonicalTouches), metadataTouches);
    const propertyKey = ({ object, path }) => JSON.stringify([object, path]);
    const previous = new Map(inventory(latest, input.model.project).properties.map((property) => [propertyKey(property), property]));
    const touched = new Set(capturedManagedPropertyTouches(basisFiles[basis.entry], capturedFiles[captured.entry], result.inventory.properties)
      .map(({ declaration, path }) => propertyKey({ object: `${compiled.entry}#${declaration}`, path })));
    result.valueChanges = result.inventory.properties.flatMap((property) => {
      const before = previous.get(propertyKey(property));
      return before && (!equal(before.value, property.value) || touched.has(propertyKey(property))) ? [{ write: { declaration: property.declaration, path: property.path, value: property.value }, before: before.value }] : [];
    });
    return result;
  }
  fail("invalid_input", "Unknown domain job kind");
}
async function run(input) {
  if (input.kind !== "reconcile") {
    const folder = resolve(input.folder);
    if (documentFolder !== undefined && folder !== documentFolder) fail("invalid_input", "A retained domain service belongs to exactly one document folder");
    documentFolder = folder;
  }
  leases = new Map(); outputs = new WeakMap();
  let result, success = false;
  try { result = await evaluate(input); success = true; return result; }
  finally {
    const outputRecord = success ? outputs.get(result) : undefined;
    if (outputRecord) leases.set(outputRecord.session, outputRecord);
    for (const [session, record] of leases) {
      // This catches mutated current sessions and permits the independently
      // authenticated, read-only historical session to survive a successful job.
      if (success && record && equal(session.exportDesign(), record.model.design)
        && session.exportProject() === record.model.project && session.sourceDesignDigest() === record.model.sourceDesignDigest) sessionRemember(record);
      else sessions.close(session);
    }
    leases = undefined; outputs = undefined;
  }
}
async function request({ id, encoded, timeoutMs }) {
  jobTimeoutMs = timeoutMs;
  try {
    const result = await run(JSON.parse(encoded));
    if (Buffer.byteLength(JSON.stringify(result)) > 128 * 1024 * 1024) fail("resource_limit", "Domain result exceeds 128 MiB");
    parentPort.postMessage({ id, ok: true, result });
  } catch (error) { parentPort.postMessage({ id, ok: false, error: { code: error.code ?? "domain_rejected", message: String(error.message ?? error).slice(0, 8192), location: { ...(error.path ? { path: error.path } : {}) } } }); }
}
// The host sends one request at a time; this second boundary also prevents
// accidental direct worker callers from overlapping mutable native sessions.
let tail = Promise.resolve();
parentPort.on("message", (message) => { tail = tail.then(() => request(message), () => request(message)); });
