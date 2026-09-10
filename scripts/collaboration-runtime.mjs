// SPDX-License-Identifier: GPL-3.0-or-later
import { createHash, randomUUID } from "node:crypto";
import { readdir, lstat, readFile, realpath } from "node:fs/promises";
import { join } from "node:path";
import { acquireWorkspaceLock } from "./workspace-storage.mjs";
import { openDurableCollaborationHost } from "./collaboration-host.mjs";
import { createCollaborationHttpServer } from "./collaboration-http.mjs";
import { collaborationHostModuleUrl } from "./workspace-runtime-paths.mjs";
const { createTrustedSourceHost, createTrustedSemanticHost } = await import(collaborationHostModuleUrl);
import { runCollaborationDomainJob } from "./collaboration-domain.mjs";
import { createCollaborationPreviewService } from "./collaboration-preview.mjs";
import { createCollaborationPreviewRoute } from "./collaboration-preview-route.mjs";

const encode = (value) => Buffer.from(JSON.stringify(value));
const same = (left, right) => JSON.stringify(left) === JSON.stringify(right);
const sourceActor = (documentEpoch, userId, clientId) => createHash("sha256").update(JSON.stringify([documentEpoch, userId, clientId])).digest();
const objectName = (entry, declaration) => `${entry}#${declaration}`;
const documentObject = (model) => objectName(model.entry, "@document");
// This unreleased runtime schema pins one canonical property namespace. Mixing
// older parent/child or generation-bearing history coordinates is unsafe.
const modelCheckpoint = (model) => encode({ format: "geosolve-collaboration-model-v2", propertySchema: "canonical-v1", model });
function restoreModelCheckpoint(bytes) {
  const checkpoint = JSON.parse(bytes);
  if (checkpoint?.format !== "geosolve-collaboration-model-v2" || checkpoint.propertySchema !== "canonical-v1" || !checkpoint.model) {
    throw Error("Unsupported collaborative history schema; preserve this folder and explicitly migrate its checkpoint before reopening");
  }
  return checkpoint.model;
}
function exact(value, keys) { if (!value || typeof value !== "object" || Array.isArray(value) || Object.keys(value).some((key) => !keys.includes(key))) throw Error("Unexpected collaborative command fields"); }

/** Explicit initialization capture. Invalid raw text is subsequently preserved by
 * native shared source, independent of compiler dependency discovery. Metadata
 * and unrelated package/build directories are never imported into the draft.
 */
async function authoredFiles(folder) {
  const files = {}; let total = 0;
  async function visit(relative) {
    const entries = await readdir(join(folder, relative), { withFileTypes: true });
    for (const entry of entries.sort((a, b) => a.name.localeCompare(b.name, "en"))) {
      const path = relative ? `${relative}/${entry.name}` : entry.name;
      if ([".geosolve", ".git", "node_modules", "target", "dist"].includes(entry.name)) continue;
      if (entry.isSymbolicLink()) throw Error(`Authored source cannot contain symlinks: ${path}`);
      if (entry.isDirectory()) { await visit(path); continue; }
      if (path !== "geosolve.json" && !/\.[cm]?[jt]s$/u.test(path)) continue;
      const stat = await lstat(join(folder, path));
      if (!stat.isFile() || stat.size > 4 * 1024 * 1024) throw Error(`Invalid authored file: ${path}`);
      const bytes = await readFile(join(folder, path)); total += bytes.length;
      if (bytes.length > 4 * 1024 * 1024 || total > 16 * 1024 * 1024 || Object.keys(files).length >= 512) throw Error("Authored source tree exceeds resource bounds");
      files[path] = new TextDecoder("utf-8", { fatal: true }).decode(bytes);
    }
  }
  await visit(""); return files;
}

/** Actual editable-folder reference composition. The caller explicitly enables
 * collaboration; the existing single-editor preview is unaffected. Compilation,
 * solve and source reconciliation use terminable workers. Native Rust owns text,
 * target lifetimes, contribution ownership and prepared validation.
 */
export async function openCollaborationRuntime(folder, {
  initialize = false, invitations, limits, storageOptions, domainOptions, allowedOrigins = [], staticRoutes, workbenchScenes = false, documentId = randomUUID(), mirror: mirrorEnabled = false, authoringPreview = {},
} = {}) {
  exact(authoringPreview, ["enabled", "preferred", "limits"]);
  const previewMode = authoringPreview.preferred ?? "client";
  if (!["client", "server"].includes(previewMode) || previewMode === "server" && authoringPreview.enabled !== true) throw Error("Server preview preference requires an enabled server preview service");
  if (!(invitations instanceof Map) || !invitations.size || [...invitations.values()].some((principal) => typeof principal?.userId !== "string" || principal.userId.startsWith("geosolve.server."))) throw Error("Provide invited principals outside the reserved server identity namespace");
  folder = await realpath(folder);
  const lock = acquireWorkspaceLock(folder), serverEpoch = randomUUID();
  let source, semantic, host, transport, accepted, initial, mirror, mirrorConnection, mirrorStatus, mirrorTimer, mirrorUnsubscribe, mirrorWork, previews;
  let stoppingMirror = false;
  const jobs = new Set();
  const domain = async (input) => {
    const controller = new AbortController(); jobs.add(controller);
    try { await domainOptions?.beforeJob?.(input); return await runCollaborationDomainJob(input, { ...domainOptions, signal: controller.signal }); }
    finally { jobs.delete(controller); }
  };
  const sourceOptions = (documentEpoch, input, files) => ({ configuration: { documentEpoch, serverEpoch, initialInput: input, files }, actor: sourceActor(documentEpoch, "server", serverEpoch) });
  try {
    if (initialize) {
      let exists = false;
      try { await lstat(join(folder, ".geosolve/collaboration/operations.journal")); exists = true; } catch (error) { if (error.code !== "ENOENT") throw error; }
      if (exists) throw Error("Collaboration is already initialized; reopen without initialization");
      const files = await authoredFiles(folder);
      const documentEpoch = randomUUID();
      accepted = await domain({ kind: "initialize", folder, files });
      source = await createTrustedSourceHost(sourceOptions(documentEpoch, accepted.acceptedInput, files));
      semantic = await createTrustedSemanticHost({ configuration: { documentEpoch, serverEpoch, objects: [
        { object: documentObject(accepted.model), dependencies: [] }, ...accepted.inventory.objects.map(({ object, dependencies }) => ({ object, dependencies })),
      ] } });
      const checkpoint = semantic.checkpoint();
      initial = { configuration: { documentId, documentEpoch, initialInput: accepted.acceptedInput }, checkpoints: {
        source: Buffer.from(source.checkpoint()), model: modelCheckpoint(accepted.model), targets: Buffer.from(checkpoint.targetsJson), history: Buffer.from(checkpoint.historyJson),
      } };
    }
    host = await openDurableCollaborationHost(folder, {
      initial, limits: limits?.authority, storageOptions, serverEpoch,
      rebuild: async ({ configuration, checkpoints, acceptedRevision, acceptedInput }) => {
        const restoredSource = await createTrustedSourceHost({ ...sourceOptions(configuration.documentEpoch, configuration.initialInput, {}), checkpointJson: checkpoints.source.toString() });
        let restoredSemantic;
        try {
          const snapshot = restoredSource.snapshot();
          if (snapshot.accepted.modelRevision !== acceptedRevision || snapshot.accepted.acceptedInput !== acceptedInput) throw Error("Source/authority accepted checkpoints disagree");
          const rebuilt = await domain({ kind: "rebuild", folder, files: snapshot.accepted.files, model: restoreModelCheckpoint(checkpoints.model) });
          if (rebuilt.acceptedInput !== acceptedInput) throw Error("Independent model rebuild does not match durable accepted source/design input");
          restoredSemantic = await createTrustedSemanticHost({ configuration: { documentEpoch: configuration.documentEpoch, serverEpoch }, checkpoint: {
            documentEpoch: configuration.documentEpoch, revision: acceptedRevision, targetsJson: checkpoints.targets.toString(), historyJson: checkpoints.history.toString(),
          } });
          for (const item of [{ object: documentObject(rebuilt.model) }, ...rebuilt.inventory.objects]) if (!restoredSemantic.current(item.object)) throw Error("Accepted model has a missing semantic target lifetime");
          source?.dispose(); semantic?.dispose(); source = restoredSource; semantic = restoredSemantic; accepted = rebuilt;
          return rebuilt.acceptedInput;
        } catch (error) { restoredSource.dispose(); restoredSemantic?.dispose(); throw error; }
      },
    });
    const reconcileMirror = () => {
      if (!mirror || stoppingMirror) return Promise.resolve(mirrorStatus);
      if (mirrorWork) return mirrorWork;
      mirrorWork = mirror.reconcile().then(status => { mirrorStatus = status; return status; }, error => {
        mirrorStatus = { status: "reconciliation_pending", notices: [{ path: "", reason: String(error.message ?? error) }] };
        return mirrorStatus;
      }).finally(() => { mirrorWork = undefined; });
      return mirrorWork;
    };
    const scheduleMirror = (delay = 1000) => {
      if (!mirror || stoppingMirror || mirrorTimer) return;
      mirrorTimer = setTimeout(() => {
        mirrorTimer = undefined; void reconcileMirror().finally(() => scheduleMirror());
      }, delay);
      mirrorTimer.unref();
    };
    if (mirrorEnabled) {
      const { createMirrorWorker } = await import("./collaboration-mirror-worker-bridge.mjs");
      const userId = "geosolve.server.external-files", clientId = "filesystem-mirror";
      mirrorConnection = await host.connect({ userId, role: "editor" }, clientId);
      mirror = await createMirrorWorker({ folder, ...host.configuration, userId, clientId,
        readCommitted: () => host.withCommittedState(() => ({ checkpoint: source.textCheckpoint(), snapshot: source.snapshot() })),
        admitWorkingEdits: async ({ operation, expectedRevision, edits }) => {
          if (operation?.userId !== userId || operation.clientId !== clientId) throw Error("External mirror principal does not match its configured identity");
          const body = { requestId: operation.requestId, action: "working", revision: expectedRevision, edits };
          try {
            await host.writeText(mirrorConnection, () => receiveText(mirrorConnection, body), { requestId: body.requestId, payload: body });
            return { status: "committed" };
          } catch (error) { if (error.code === "text_rejected") return { status: "rejected", reason: error.message }; throw error; }
        },
      });
      await reconcileMirror();
      mirrorUnsubscribe = host.subscribe(() => scheduleMirror(50)); scheduleMirror();
    }
    const generationSnapshot = () => Object.fromEntries([documentObject(accepted.model), ...accepted.inventory.objects.map(({ object }) => object)].map(object => [object, semantic.current(object)]));
    const buildScene = async (candidate) => {
      if (!workbenchScenes) return undefined;
      const view = await domain({ kind: "scene", folder, files: candidate.candidateFiles, model: candidate.model, expectedInput: candidate.acceptedInput });
      if (view.acceptedInput !== candidate.acceptedInput) throw Error("Workbench scene does not match the accepted input");
      return view.scene;
    };
    let acceptedScene = await buildScene(accepted);
    function captureApply() {
      const capture = source.captureApply();
      try { return { applyCapture: Buffer.from(capture.captureJson), applyBasis: encode(capture.acceptedBasis), targetBasis: encode(generationSnapshot()) }; }
      finally { source.release(capture); }
    }
    async function captureSemantic(_connection, command) {
      if (!["point_gesture", "construction"].includes(command.payload?.action)) return {};
      const captured = await host.acceptedCheckpoint(command.basisRevision);
      const model = restoreModelCheckpoint(captured.checkpoints.model);
      if (command.payload.gesture?.basis !== model.sourceDesignDigest) throw Error("Gesture basis differs from the historical accepted model revision");
      return { replayBasis: encode({ revision: captured.revision, acceptedInput: captured.acceptedInput }), replayModel: captured.checkpoints.model,
        replaySource: captured.checkpoints.source, replayTargets: captured.checkpoints.targets, replayHistory: captured.checkpoints.history };
    }
    function replayCheckpoint(command, attachments) {
      if (!["replayBasis", "replayModel", "replaySource", "replayTargets", "replayHistory"].every(name => attachments[name])) throw Error("Gesture lacks its durable original accepted checkpoint");
      const basis = JSON.parse(attachments.replayBasis), model = restoreModelCheckpoint(attachments.replayModel);
      if (basis.revision !== command.basisRevision || model.sourceDesignDigest !== command.payload.gesture?.basis) throw Error("Gesture historical checkpoint identity changed");
      return { ...basis, model, documentEpoch: host.configuration.documentEpoch, initialInput: host.configuration.initialInput, serverEpoch,
        source: attachments.replaySource.toString(), targets: attachments.replayTargets.toString(), history: attachments.replayHistory.toString() };
    }
    function receiveText(connection, body) {
      const operation = { userId: connection.userId, clientId: connection.clientId, requestId: body.requestId };
      let stage;
      if (body.action === "undo" || body.action === "redo") {
        exact(body, ["requestId", "action"]);
        stage = body.action === "undo" ? source.stageUserUndo(operation) : source.stageUserRedo(operation);
      } else if (body.action === "working") {
        exact(body, ["requestId", "action", "revision", "edits"]);
        if (!body.revision || !Array.isArray(body.edits) || !body.edits.length || body.edits.length > 256) throw Error("Expected bounded working source edits with an exact revision");
        stage = source.stageUserWorkingEdits(body.edits, operation, body.revision);
      } else if (body.action === "files") {
        exact(body, ["requestId", "action", "revision", "edits"]);
        if (!body.revision || !Array.isArray(body.edits) || !body.edits.length || body.edits.length > 256
          || body.edits.some((edit) => !["create_file", "remove_file", "rename_file"].includes(edit?.kind))) throw Error("Expected bounded file lifecycle edits with an exact source revision");
        stage = source.stageUserFileEdits(body.edits, operation, body.revision);
      } else {
        exact(body, ["requestId", "changes"]);
        if (!Array.isArray(body.changes) || body.changes.length > 256 || body.changes.some((change) => !Array.isArray(change) || change.some((byte) => !Number.isInteger(byte) || byte < 0 || byte > 255))) throw Error("Expected bounded native text changes");
        const actor = sourceActor(connection.documentEpoch, connection.userId, connection.clientId);
        stage = source.stageUserTextChanges(body.changes.map((value) => Uint8Array.from(value)), actor, operation);
      }
      return { checkpoint: Buffer.from(stage.checkpointJson), ack: { sourceSequence: stage.sequence, workingRevision: stage.workingRevision },
        commit: () => source.commitStage(stage), fail: () => source.failStage(stage) };
    }
    const checkedWrites = (payload) => {
      exact(payload, ["action", "writes"]);
      if (payload.action !== "values" || !Array.isArray(payload.writes) || !payload.writes.length || payload.writes.length > 1024) throw Error("Expected semantic values with explicit target generations");
      return payload.writes.map((write) => {
        exact(write, ["target", "declaration", "path", "value"]);
        semantic.authenticate(write.target);
        if (write.target.object !== objectName(accepted.model.entry, write.declaration)) throw Error("Semantic declaration does not match its authenticated target");
        return { declaration: write.declaration, path: write.path, value: write.value };
      });
    };
    async function execute(prepared, attachments) {
      lock.assertHeld();
      const basis = source.snapshot().accepted;
      if (prepared.acceptedInput !== basis.acceptedInput || prepared.acceptedRevision !== basis.modelRevision) throw Error("Domain job does not match current accepted source authority");
      let inverse, capture, sourcePrepared, result;
      const releaseHandles = () => {
        if (sourcePrepared) { source.release(sourcePrepared); sourcePrepared = undefined; }
        if (inverse) { semantic.release(inverse); inverse = undefined; }
        if (capture) { source.release(capture); capture = undefined; }
      };
      try {
        if (prepared.command.kind === "semantic") {
          if (prepared.command.payload?.action === "point_gesture") {
            const payload = prepared.command.payload;
            exact(payload, ["action", "targets", "gesture"]);
            const target = payload.gesture?.target;
            const addresses = target?.target === "point" ? [target.address] : target?.target === "rectangle_corner" ? [target.lower_left, target.upper_right] : [];
            const owners = [...new Set(addresses.map((address) => {
              const owner = address?.owner?.address;
              const declaration = owner?.owner === "direct_declaration" ? owner.declaration : owner?.owner === "generated_member" ? owner.address?.invocation : undefined;
              if (typeof declaration !== "string") throw Error("Gesture requires explicit native target owners");
              return objectName(accepted.model.entry, declaration);
            }))];
            if (!owners.length || !Array.isArray(payload.targets) || payload.targets.length !== owners.length) throw Error("Gesture requires exact semantic target lifetimes");
            for (const [index, object] of owners.entries()) {
              if (payload.targets[index]?.object !== object) throw Error("Gesture semantic target does not match its native owner");
              semantic.authenticate(payload.targets[index]);
            }
            result = await domain({ kind: "point_gesture", folder, files: basis.files, model: accepted.model, command: payload.gesture,
              replayCheckpoint: replayCheckpoint(prepared.command, attachments), originalTargets: payload.targets });
          } else if (prepared.command.payload?.action === "construction") {
            exact(prepared.command.payload, ["action", "gesture"]);
            result = await domain({ kind: "construction", folder, files: basis.files, model: accepted.model, command: prepared.command.payload.gesture,
              replayCheckpoint: replayCheckpoint(prepared.command, attachments) });
          } else if (prepared.command.payload?.action === "mutation") {
            const payload = prepared.command.payload;
            exact(payload, ["action", "mutation", "targets", "deletion"]);
            const mutation = payload.mutation;
            if (!mutation || !["set_metadata", "reorder_declaration", "delete", "extract_parameter", "set_suppressed"].includes(mutation.mutation)) throw Error("Unsupported collaborative structured mutation");
            const names = mutation.mutation === "extract_parameter" ? [mutation.declaration] : mutation.mutation === "reorder_declaration" ? [mutation.declaration, ...(mutation.before === null ? [] : [mutation.before])]
              : mutation.target?.target === "document" && mutation.mutation === "set_metadata" ? ["@document"]
              : mutation.target?.target === "declaration" || mutation.target?.target === "parameter" ? [mutation.target.declaration]
              : mutation.mutation === "set_suppressed" && mutation.target?.target === "generated" ? [mutation.target.address?.invocation] : [];
            const required = [...new Set(names.map(name => objectName(accepted.model.entry, name)))].sort();
            if (!required.length || !Array.isArray(payload.targets) || !same(payload.targets.map(target => target.object).sort(), required)) throw Error("Structured mutation requires exact explicit target lifetimes");
            for (const target of payload.targets) semantic.authenticate(target);
            if (mutation.mutation === "delete") {
              if (!payload.deletion || !same(payload.deletion.roots.map(target => target.object).sort(), required)) throw Error("Deletion requires its reviewed native dependency closure");
              semantic.authenticateDelete(payload.deletion);
            } else if (payload.deletion !== undefined) throw Error("Unexpected deletion closure");
            result = await domain({ kind: "mutation", folder, files: basis.files, model: accepted.model, mutation });
            const removed = accepted.inventory.objects.filter(item => !result.inventory.objects.some(next => next.object === item.object)).map(item => item.object).sort();
            if (!same(removed, (payload.deletion?.closure ?? []).map(target => target.object).sort())) throw Error("Compiler deletion differs from the reviewed dependency closure");
          } else {
            const writes = checkedWrites(prepared.command.payload);
            result = await domain({ kind: "values", folder, files: basis.files, model: accepted.model, writes });
          }
        } else if (prepared.command.kind === "undo" || prepared.command.kind === "redo") {
          exact(prepared.command.payload, []);
          inverse = prepared.command.kind === "undo" ? semantic.prepareUndo(prepared.operation.userId) : semantic.prepareRedo(prepared.operation.userId);
          for (const change of inverse.changes) semantic.authenticate(change.address.target);
          result = await domain({ kind: "structural_inverse", folder, files: basis.files, model: accepted.model, inverse });
        } else if (prepared.command.kind === "apply") {
          exact(prepared.command.payload, []);
          if (!attachments.applyCapture || !attachments.applyBasis || !attachments.targetBasis) throw Error("Apply lacks a durable native capture");
          capture = await host.withCommittedState(() => source.restoreApplyCapture(attachments.applyCapture.toString(), JSON.parse(attachments.applyBasis)));
          const generations = JSON.parse(attachments.targetBasis);
          const invalidatedDeclarations = Object.entries(generations).filter(([name, target]) => !same(semantic.current(name), target)).map(([name]) => name.slice(accepted.model.entry.length + 1));
          result = await domain({ kind: "apply", folder, files: basis.files, model: accepted.model, capture, invalidatedDeclarations });
          for (const declaration of result.requiredStableDeclarations ?? []) {
            const name = objectName(accepted.model.entry, declaration);
            if (!generations[name]) throw Error("Captured declaration has no admitted lifetime");
            semantic.authenticate(generations[name]);
          }
        } else throw Error("Unsupported collaborative semantic operation");
        // These targets come from the native historical registry, never from a
        // replacement token supplied by the client. A delete/restore cycle is a
        // different lifetime even when its source and geometry look identical.
        for (const target of result.requiredStableTargets ?? []) semantic.authenticate(target);
        const candidateScene = await buildScene(result);
        sourcePrepared = await host.withCommittedState(() => capture ? source.prepareApplyUpdate(capture, result.candidateFiles) : source.prepareCanvasUpdate(result.patches));
        const working = source.snapshot().working;
        const reconciled = result.preparedContribution
          ? await domain({ kind: "reconcile", preparedContribution: result.preparedContribution, workingFiles: working.files })
          : { reconciliations: sourcePrepared.changedPaths.map((path) => capture && capture.working.files[path] === result.candidateFiles[path]
            ? { kind: "captured", path } : { kind: "pending", path, reason: "Captured Apply includes intervening accepted source changes" }) };
        return () => {
          lock.assertHeld();
          let sourceStage, semanticStage;
          try {
            for (const target of result.requiredStableTargets ?? []) semantic.authenticate(target);
            const latest = source.snapshot().working;
            const updates = same(latest.revision, working.revision) ? reconciled.reconciliations : sourcePrepared.changedPaths.map((path) => {
              const prior = reconciled.reconciliations.find((entry) => entry.path === path);
              return prior?.kind === "captured" ? prior : { kind: "pending", path, reason: "Working source changed during reconciliation; accepted canvas edit is retained" };
            });
            sourceStage = source.stageValidatedPublication(sourcePrepared, result.acceptedInput, latest.revision, updates);
            const revision = basis.modelRevision + 1;
            if (inverse) semanticStage = semantic.stageValidatedInverse(inverse, prepared.operation, revision);
            else semanticStage = stageSemanticResult(result, prepared, basis.modelRevision, revision);
            return { result: { allocationMapping: result.allocationMapping ?? [], createdDeclarations: result.createdDeclarations ?? [] }, completion: { status: "accepted", acceptedInput: result.acceptedInput, summary: prepared.command.kind === "apply" ? "Applied captured shared draft" : `${prepared.command.kind} accepted` },
              checkpoints: { source: Buffer.from(sourceStage.checkpointJson), model: modelCheckpoint(result.model), targets: Buffer.from(semanticStage.targetsJson), history: Buffer.from(semanticStage.historyJson) },
              install: () => { source.commitStage(sourceStage); sourcePrepared = undefined; semantic.commitStage(semanticStage); inverse = undefined; accepted = result; acceptedScene = candidateScene; releaseHandles(); },
              abort: () => { if (source.snapshot().hasPendingStage) source.failStage(sourceStage); if (semantic.snapshot().hasPendingStage) semantic.failStage(semanticStage); sourcePrepared = undefined; capture = undefined; inverse = undefined; },
            };
          } catch (error) {
            // An unexpected failure after one native stage cannot silently install
            // the other. Recovery reconstructs all components from durable bytes.
            // No append has been attempted yet. Drop the known unpersisted
            // source candidate; uncertain filesystem failures use abort above.
            if (sourceStage) source.discardUnpersistedStage(sourceStage);
            if (semanticStage) semantic.discardUnpersistedStage(semanticStage);
            releaseHandles();
            throw error;
          }
        };
      } catch (error) {
        if (!host.snapshot().needsRecovery) await host.withCommittedState(releaseHandles);
        throw error;
      }
    }
    function stageSemanticResult(result, prepared, basisRevision, revision) {
      const oldObjects = new Map(accepted.inventory.objects.map((item) => [item.object, item]));
      const newObjects = new Map(result.inventory.objects.map((item) => [item.object, item]));
      const removed = [...oldObjects.keys()].filter((name) => !newObjects.has(name));
      const deletions = removed.length ? [semantic.planDelete(removed.map((name) => semantic.current(name)))] : [];
      if (deletions.some((plan) => plan.closure.some((target) => newObjects.has(target.object)))) throw Error("Candidate deletion would remove a retained dependency");
      const createdNames = new Set([...newObjects.keys()].filter((name) => !oldObjects.has(name)));
      const reference = (object) => createdNames.has(object) ? { kind: "created", object } : { kind: "existing", target: semantic.current(object) };
      if (!Array.isArray(result.propertyChanges)) throw Error("Domain result lacks canonical property ownership");
      const changes = result.propertyChanges.filter(change => !createdNames.has(change.object)).map(change => {
        const target = semantic.current(change.object);
        if (!target) throw Error("Canonical property owner has no current target lifetime");
        return { address: { target, property: change.property }, before: change.before, after: change.after };
      });
      const existingPosition = position => ({ previous: position.previous ? semantic.current(position.previous) : null, next: position.next ? semantic.current(position.next) : null });
      const creationPosition = position => ({ previous: position.previous ? reference(position.previous) : null, next: position.next ? reference(position.next) : null });
      const structural = {
        created: [...createdNames].map(object => ({ object, payload: newObjects.get(object).payload, position: creationPosition(newObjects.get(object).position) })),
        deleted: removed.map(object => ({ target: semantic.current(object), payload: oldObjects.get(object).payload, position: existingPosition(oldObjects.get(object).position) })),
        reorders: (result.structuralChanges?.reorders ?? []).filter(item => !createdNames.has(item.object)).map(item => ({ target: semantic.current(item.object), before: existingPosition(item.before), after: existingPosition(item.after) })),
      };
      const changedDependencies = new Set((result.structuralChanges?.dependencies ?? []).map(item => item.object));
      // Apply publishes shared text; its barrier prevents canvas Undo from
      // overwriting that source without assigning all authors' text to the clicker.
      // Text contributions use their own Undo then explicit Apply.
      const operation = prepared.command.kind === "apply" ? { ...prepared.operation, userId: `geosolve.server.shared-draft.${host.configuration.documentEpoch}` } : prepared.operation;
      return semantic.stageValidatedTransaction({ basisRevision, revision,
        create: [...createdNames].map((object) => ({ object, dependencies: newObjects.get(object).dependencies.map(reference) })), deletions,
        dependencies: [...newObjects.values()].filter((item) => !createdNames.has(item.object) && (changedDependencies.has(item.object) || !same(item.dependencies, oldObjects.get(item.object).dependencies)))
          .map((item) => ({ target: reference(item.object), dependencies: item.dependencies.map(reference) })),
        ...(changes.length || structural.created.length || structural.deleted.length || structural.reorders.length || changedDependencies.size ? { record: { operation, changes, structural } } : {}),
      });
    }
    previews = createCollaborationPreviewService({ enabled: authoringPreview.enabled ?? false, limits: authoringPreview.limits,
      authenticate(connection) { host.resume(connection, host.snapshot().latestSequence); },
      captureBasis() { return { documentEpoch: host.configuration.documentEpoch, revision: host.snapshot().acceptedRevision,
        sourceDesignDigest: accepted.model.sourceDesignDigest, project: accepted.model.project, design: accepted.model.design }; },
    });
    const documentSnapshot = (connection) => ({ ...source.snapshot(), targets: generationSnapshot(), documentTarget: semantic.current(documentObject(accepted.model)), inventory: accepted.inventory, sourceProjection: accepted.sourceProjection,
      authoringPreview: { server: previews.enabled, preferred: previewMode },
      mirror: mirrorStatus, textHistory: source.userHistory(connection.userId), semanticHistory: semantic.userHistory(connection.userId),
      pointTargets: accepted.pointTargets, model: { project: accepted.model.project, design: accepted.model.design, sourceDesignDigest: accepted.model.sourceDesignDigest },
      textActor: Array.from(sourceActor(connection.documentEpoch, connection.userId, connection.clientId)), textCheckpoint: Array.from(source.textCheckpoint()),
    });
    transport = createCollaborationHttpServer({ host, invitations, execute, captureApply, captureSemantic, receiveText, documentSnapshot,
      textDelta: (connection, revision) => ({ ...source.textChangesSince(revision), history: source.userHistory(connection.userId) }),
      scene: () => encode(acceptedScene ?? accepted.result), limits: limits?.http, allowedOrigins, staticRoutes,
      authoringPreview: { request: createCollaborationPreviewRoute(previews), dropConnection: previews.dropConnection, close: previews.close },
    });
    let closing;
    return { host, source: () => source, semantic: () => semantic, transport, previews, mirror: () => mirrorStatus, reconcileMirror,
      async listen(port = 0, hostname = "127.0.0.1") {
        await new Promise((resolve, reject) => { transport.server.once("error", reject); transport.server.listen(port, hostname, resolve); });
        const address = transport.server.address();
        transport.allowOrigin(`http://${hostname.includes(":") ? `[${hostname}]` : hostname}:${address.port}`);
        transport.kick(); return address;
      },
      close() {
        if (closing) return closing;
        transport.stop(); stoppingMirror = true; clearTimeout(mirrorTimer); mirrorUnsubscribe?.();
        closing = (async () => {
          for (const controller of jobs) controller.abort();
          while (transport.stats().workerBusy) await new Promise((resolve) => setTimeout(resolve, 5));
          await mirror?.close(); await mirrorWork;
          try { await transport.close(); if (mirrorConnection && !host.snapshot().needsRecovery) await host.disconnect(mirrorConnection); }
          finally { try { await host.close(); } finally { source.dispose(); semantic.dispose(); lock.release(); } }
        })();
        return closing;
      },
    };
  } catch (error) { stoppingMirror = true; clearTimeout(mirrorTimer); mirrorUnsubscribe?.(); await mirror?.close(); await mirrorWork; for (const controller of jobs) controller.abort(); if (transport) await transport.close(); await previews?.close(); await host?.close(); source?.dispose(); semantic?.dispose(); lock.release(); throw error; }
}
