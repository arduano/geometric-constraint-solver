// SPDX-License-Identifier: GPL-3.0-or-later
import { createHash, randomUUID } from "node:crypto";
import { createDocumentAuthorityHost } from "../packages/geosolve-collaboration/dist/host.js";
import { openCollaborationStorage } from "./collaboration-storage.mjs";

const format = "geosolve-collaboration-host-v1";
const checkpointNames = ["model", "source", "targets", "history"];
const frozen = (value) => { if (value && typeof value === "object") { Object.values(value).forEach(frozen); Object.freeze(value); } return value; };
const copy = (value) => frozen(JSON.parse(JSON.stringify(value)));
function failure(code, message) { return Object.assign(Error(message), { code }); }
function checkpointBytes(attachments) {
  if (!attachments || Object.keys(attachments).sort().join(",") !== checkpointNames.slice().sort().join(",")) throw Error("Accepted publication requires exact model/source/targets/history checkpoints");
  return Object.fromEntries(checkpointNames.map((name) => {
    const bytes = attachments[name];
    if (!(bytes instanceof Uint8Array) || bytes.length === 0) throw Error(`Missing exact ${name} checkpoint bytes`);
    return [name, Buffer.from(bytes)];
  }));
}
function validateBoot(payload) {
  if (!payload || payload.format !== format || payload.kind !== "bootstrap" || !payload.configuration
    || Object.keys(payload.configuration).sort().join(",") !== "documentEpoch,documentId,initialInput") throw Error("Invalid collaboration bootstrap checkpoint");
}

/** One trusted document process, behind the canonical folder's exclusive lock.
 * Rust owns sessions/order/dedup/CAS. This adapter joins its durable journal with
 * exact source/model/target/history bytes and runs domain compute outside the
 * short persistence queue. It never accepts a network completion/solver proof.
 *
 * `rebuild` independently reconstructs accepted geometry from checkpoints before
 * authoring is enabled. It returns the resulting acceptedInput, not a boolean.
 */
export async function openDurableCollaborationHost(folder, {
  initial, rebuild, limits, storageOptions, serverEpoch = randomUUID(), maxIngress = 128,
} = {}) {
  if (typeof rebuild !== "function") throw Error("Independent accepted-model rebuild is required");
  if (!Number.isSafeInteger(maxIngress) || maxIngress < 1 || maxIngress > 4096) throw Error("Invalid collaboration ingress bound");
  const storage = await openCollaborationStorage(folder, { ...storageOptions, create: initial !== undefined });
  let authority;
  try {
    if (!storage.records().length) {
      if (!initial) throw Error("Collaboration bootstrap is missing; explicit initialization required");
      const configuration = copy(initial.configuration);
      const attachments = checkpointBytes(initial.checkpoints);
      const payload = { format, kind: "bootstrap", configuration };
      validateBoot(payload);
      if (await rebuild({ configuration, acceptedRevision: 0, acceptedInput: configuration.initialInput, checkpoints: attachments }) !== configuration.initialInput) throw Error("Rebuilt bootstrap input does not match its accepted identity");
      await storage.append(payload, attachments);
    }
    const envelopes = storage.records();
    const boot = envelopes[0]; validateBoot(boot.payload);
    checkpointBytes(await loadCheckpoints(storage, boot.attachments));
    const configuration = boot.payload.configuration;
    if (initial && JSON.stringify(copy(initial.configuration)) !== JSON.stringify(configuration)) throw Error("Existing collaboration identity differs from initialization");
    const records = [];
    let current = { acceptedRevision: 0, acceptedInput: configuration.initialInput, references: boot.attachments };
    let textReferences = { source: boot.attachments.source };
    const admissionAttachments = new Map();
    const textReceipts = new Map();
    for (const envelope of envelopes.slice(1)) {
      const payload = envelope.payload;
      if (payload?.format !== format) throw Error("Unknown collaboration host envelope format");
      if (payload.kind === "authority") {
        const record = payload.record;
        if (!record || record.sequence !== records.length + 1) throw Error("Incomplete collaboration authority record sequence");
        // Rust journal records bind the exact pre-event accepted input. A
        // terminal's outcome carries the newly accepted revision and input.
        if (record.acceptedRevision !== current.acceptedRevision || record.acceptedInput !== current.acceptedInput) throw Error("Authority record has the wrong accepted checkpoint basis");
        records.push(record);
        if (record.event.event === "admitted") admissionAttachments.set(operationKey(record.event.operation), envelope.attachments);
        else if (record.event.event !== "finished") throw Error("Invalid collaboration authority event");
        if (record.event.event === "finished" && record.event.outcome.status === "accepted") {
          checkpointBytes(await loadCheckpoints(storage, envelope.attachments));
          const outcome = record.event.outcome;
          if (outcome.revision !== current.acceptedRevision + 1) throw Error("Accepted checkpoint revision gap");
          current = { acceptedRevision: outcome.revision, acceptedInput: outcome.accepted_input, references: envelope.attachments };
          textReferences = { source: envelope.attachments.source };
        }
      } else if (payload.kind === "text") {
        if (payload.acceptedRevision !== current.acceptedRevision || payload.acceptedInput !== current.acceptedInput
          || Object.keys(envelope.attachments).join(",") !== "source") throw Error("Text checkpoint changed accepted model identity");
        textReferences = envelope.attachments;
        if (payload.operation !== undefined) {
          const key = operationKey(payload.operation);
          if (textReceipts.has(key) || typeof payload.requestDigest !== "string" || !/^[a-f0-9]{64}$/u.test(payload.requestDigest) || payload.ack === undefined) throw Error("Invalid durable text request receipt");
          if (payload.rejection !== undefined && (payload.rejection?.code !== "text_rejected" || typeof payload.rejection.message !== "string" || payload.rejection.message.length > 1024)) throw Error("Invalid durable text refusal");
          textReceipts.set(key, { digest: payload.requestDigest, ack: copy(payload.ack), rejection: payload.rejection });
        }
      } else throw Error("Unknown collaboration host envelope kind");
    }
    authority = await createDocumentAuthorityHost({ configuration: { ...configuration, serverEpoch, limits }, records });
    if (records.some((record) => record.event.event === "admitted" && textReceipts.has(operationKey(record.event.operation)))) throw Error("Operation ID reused across text and model history");
    if (authority.snapshot().acceptedRevision !== current.acceptedRevision || authority.snapshot().acceptedInput !== current.acceptedInput) throw Error("Authority ledger and accepted checkpoint disagree");
    const restoredCheckpoints = await loadCheckpoints(storage, { ...current.references, ...textReferences });
    if (envelopes.length > 1 || !initial) {
      if (await rebuild({ configuration, acceptedRevision: current.acceptedRevision, acceptedInput: current.acceptedInput, checkpoints: restoredCheckpoints }) !== current.acceptedInput) throw Error("Reconstructed geometry does not match accepted collaboration checkpoint");
    }
    const listeners = new Set();
    let tail = Promise.resolve(); let queued = 0; let closed = false; let closing = false; let poisoned = false; let worker;
    const live = () => { if (closed || closing || poisoned || storage.needsRecovery || authority.snapshot().needsRecovery) throw failure("recovery_required", "Collaboration host requires reopen/recovery"); };
    function enqueue(action, internal = false) {
      try { live(); if (!internal && queued >= maxIngress) throw failure("backpressure", "Collaboration ingress is full; retain the request and retry"); }
      catch (error) { return Promise.reject(error); }
      queued++;
      const run = async () => { if (poisoned || storage.needsRecovery) throw failure("recovery_required", "Collaboration host requires reopen/recovery"); return action(); };
      const promise = tail.then(run).finally(() => { queued--; });
      tail = promise.catch(() => {});
      return promise;
    }
    function publish(envelope) {
      // Subscriptions are disposable notifications. A slow/broken transport must
      // never roll back a committed operation or prevent another subscriber.
      for (const listener of listeners) { try { listener(envelope); } catch { listeners.delete(listener); } }
    }
    async function persistStage(stage, attachments = {}) {
      return storage.append({ format, kind: "authority", record: JSON.parse(stage.recordJson) }, attachments);
    }
    const host = {
      configuration: copy(configuration),
      snapshot() { return { ...authority.snapshot(), latestEnvelope: storage.latestSequence, ingressCount: queued, workerBusy: !!worker, needsRecovery: poisoned || storage.needsRecovery || authority.snapshot().needsRecovery }; },
      restoredCheckpoints: () => Object.fromEntries(Object.entries(restoredCheckpoints).map(([key, bytes]) => [key, Buffer.from(bytes)])),
      connect(principal, clientId, sessionId = randomUUID()) { return enqueue(() => authority.connect(principal, clientId, sessionId)); },
      disconnect(connection) { return enqueue(() => { authenticate(connection); authority.disconnect(connection.sessionId); }); },
      receipt(connection, requestId) { live(); return authority.receipt(connection, requestId); },
      resume(connection, after) { live(); return authority.resume(connection, after); },
      subscribe(listener) { live(); if (listeners.size >= 256 || typeof listener !== "function") throw failure("backpressure", "Collaboration subscription limit"); listeners.add(listener); return () => listeners.delete(listener); },
      /** Trusted short native preparation between fsynced source stages. Do
       * compiler/solver work outside this callback; never expose it on the wire.
       */
      withCommittedState(action) {
        if (typeof action !== "function") throw Error("Trusted synchronous preparation is required");
        return enqueue(() => {
          const result = action();
          if (result && typeof result.then === "function") throw Error("Preparation callback must be synchronous");
          return result;
        }, true);
      },
      /** Trusted host capture factory runs in the short transaction queue, fixing
       * Apply's immutable capture at admission. Clients cannot submit attachments.
       */
      admit(request, capture) {
        const immutableRequest = copy(request);
        return enqueue(async () => {
          authenticate(immutableRequest.connection, true);
          if (textReceipts.has(operationKey({ userId: immutableRequest.connection.userId, clientId: immutableRequest.connection.clientId, requestId: immutableRequest.requestId }))) throw failure("operation_reused", "Operation ID already belongs to a text change");
          const known = authority.receipt(immutableRequest.connection, immutableRequest.requestId);
          // Capture before creating a pending authority stage: a compiler/text
          // capture refusal has no uncertain durable write to recover from.
          const attachments = !known && capture ? await capture() : {};
          const stage = authority.stageAdmission(immutableRequest);
          if (stage.status === "duplicate") return stage.receipt;
          let envelope;
          try {
            envelope = await persistStage(stage, attachments);
            const receipt = authority.commitStage(stage);
            admissionAttachments.set(operationKey(receipt.operation), envelope.attachments);
            publish(envelope); return receipt;
          } catch (error) {
            // A capture failure is also a discarded pending authority stage. Its
            // uncertain lifecycle is rebuilt from storage, never guessed at.
            if (authority.snapshot().hasPendingStage) authority.failStage(stage);
            poisoned = true; throw error;
          }
        });
      },
      /** Domain prepare/solve runs outside enqueue. It returns a host-only finish
       * function which reconciles latest draft and stages checkpoints inside the
       * short queue. No client request can provide these publication callbacks.
       */
      async runNext(execute) {
        live(); if (worker) throw failure("worker_busy", "One ordered model worker is already running");
        if (typeof execute !== "function") throw Error("Trusted domain worker is required");
        const marker = {}; worker = marker;
        try {
          const prepared = await enqueue(() => authority.beginNext(), true);
          if (!prepared) return null;
          const references = admissionAttachments.get(operationKey(prepared.operation)) ?? {};
          const attachments = Object.fromEntries(await Promise.all(Object.entries(references).map(async ([key, digest]) => [key, await storage.readBlob(digest)])));
          let finish;
          try { finish = await execute(prepared, attachments); }
          catch (error) { finish = () => ({ completion: { status: "rejected", code: "domain_rejected", message: String(error).slice(0, 1024) } }); }
          if (typeof finish !== "function") throw Error("Trusted domain worker must return a publication preparation function");
          return await enqueue(async () => {
            let publication;
            try { publication = await finish(); }
            catch (error) {
              if (error.recoveryRequired) { poisoned = true; throw error; }
              publication = { completion: { status: "rejected", code: "reconciliation_rejected", message: String(error).slice(0, 1024) } };
            }
            const accepted = publication?.completion?.status === "accepted";
            let checkpoints;
            try { checkpoints = accepted ? checkpointBytes(publication.checkpoints) : {}; }
            catch (error) { publication.abort?.(); throw error; }
            const stage = authority.stageValidatedCompletion(prepared, publication.completion);
            try {
              const envelope = await persistStage(stage, checkpoints);
              // Both installs are synchronous after the one fsynced envelope.
              // Any unexpected install error poisons host reads/authoring until
              // replay from the now-durable accepted checkpoint.
              const receipt = authority.commitStage(stage);
              publication.install?.();
              if (accepted) {
                current = { acceptedRevision: receipt.outcome.revision, acceptedInput: receipt.outcome.accepted_input, references: envelope.attachments };
                textReferences = { source: envelope.attachments.source };
              }
              publish(envelope); return receipt;
            } catch (error) {
              if (authority.snapshot().hasPendingStage) authority.failStage(stage);
              publication.abort?.(); poisoned = true; throw error;
            }
          }, true);
        } catch (error) { if (authority.snapshot().pendingCount && !authority.snapshot().hasPendingStage && !storage.needsRecovery) poisoned = true; throw error; }
        finally { if (worker === marker) worker = undefined; }
      },
      /** Stage an authenticated raw-text change independently of model compute.
       * Source adapter owns actor/schema/lifecycle validation and its own staging.
       */
      writeText(connection, prepare, request) {
        const requested = request === undefined ? undefined : textRequest(connection, request);
        return enqueue(async () => {
          authenticate(connection, true);
          if (requested) {
            if (authority.receipt(connection, requested.operation.requestId)) throw failure("operation_reused", "Operation ID already belongs to a model operation");
            const previous = textReceipts.get(operationKey(requested.operation));
            if (previous) {
              if (previous.digest !== requested.requestDigest) throw failure("operation_reused", "Text request ID was reused with different content");
              if (previous.rejection) throw failure(previous.rejection.code, previous.rejection.message);
              return copy(previous.ack);
            }
            if (textReceipts.size >= 100_000) throw failure("backpressure", "Durable text receipt limit reached");
          }
          let stage;
          try { stage = await prepare(); }
          catch (error) {
            if (!requested) throw error;
            // A transactional native refusal is also a final operation result.
            // Keep exact source bytes and journal the refusal before returning;
            // retry may never reinterpret this ID against a newer document.
            const rejection = { code: "text_rejected", message: String(error.message ?? error).slice(0, 1024) };
            try {
              const snapshot = authority.snapshot();
              const checkpoint = await storage.readBlob(textReferences.source);
              const envelope = await storage.append({ format, kind: "text", acceptedRevision: snapshot.acceptedRevision, acceptedInput: snapshot.acceptedInput, ...requested, ack: null, rejection }, { source: checkpoint });
              textReceipts.set(operationKey(requested.operation), { digest: requested.requestDigest, ack: null, rejection });
              textReferences = envelope.attachments;
              publish(envelope);
            } catch (persistenceError) { poisoned = true; throw persistenceError; }
            throw failure(rejection.code, rejection.message);
          }
          if (!(stage?.checkpoint instanceof Uint8Array) || typeof stage.commit !== "function" || typeof stage.fail !== "function") throw Error("Invalid trusted text persistence stage");
          try {
            const snapshot = authority.snapshot();
            if (requested && stage.ack === undefined) throw Error("Durable text request requires the exact staged acknowledgement");
            const ack = requested ? copy(stage.ack) : undefined;
            const envelope = await storage.append({ format, kind: "text", acceptedRevision: snapshot.acceptedRevision, acceptedInput: snapshot.acceptedInput, ...(requested ? { ...requested, ack } : {}) }, { source: stage.checkpoint });
            const result = stage.commit(); textReferences = envelope.attachments;
            if (requested) textReceipts.set(operationKey(requested.operation), { digest: requested.requestDigest, ack });
            publish(envelope); return requested ? ack : result;
          } catch (error) { stage.fail(); poisoned = true; throw error; }
        });
      },
      async close() {
        if (worker) throw Error("Finish or cancel the domain worker before closing collaboration");
        if (closed || closing) return;
        closing = true; await tail; closed = true; listeners.clear(); authority.dispose(); await storage.close();
      },
    };
    function authenticate(connection, edit = false) {
      // Native resume verifies the complete server-issued connection, including
      // role, identity and epochs; the role is not taken from untrusted JSON alone.
      authority.resume(connection, authority.snapshot().latestSequence);
      if (edit && connection.role !== "editor") throw failure("read_only", "This session cannot edit shared text");
    }
    return host;
  } catch (error) { authority?.dispose(); await storage.close(); throw error; }
}
function operationKey(operation) { return JSON.stringify([operation.userId, operation.clientId, operation.requestId]); }
function textRequest(connection, request) {
  if (typeof request.requestId !== "string" || !/^[A-Za-z0-9_.:-]{1,128}$/u.test(request.requestId)) throw Error("Invalid durable text request ID");
  const ordered = (value) => {
    if (value === null || typeof value === "boolean" || typeof value === "string" || typeof value === "number" && Number.isFinite(value)) return value;
    if (Array.isArray(value)) return value.map(ordered);
    if (!value || typeof value !== "object" || ![Object.prototype, null].includes(Object.getPrototypeOf(value))) throw Error("Text payload must be finite JSON data");
    return Object.fromEntries(Object.keys(value).sort().map((key) => [key, ordered(value[key])]));
  };
  const bytes = JSON.stringify(ordered(request.payload));
  if (Buffer.byteLength(bytes) > 1024 * 1024) throw Error("Durable text request exceeds payload bound");
  return { operation: { userId: connection.userId, clientId: connection.clientId, requestId: request.requestId }, requestDigest: createHash("sha256").update(bytes).digest("hex") };
}
async function loadCheckpoints(storage, references) {
  return Object.fromEntries(await Promise.all(Object.entries(references).map(async ([name, key]) => [name, await storage.readBlob(key)])));
}
