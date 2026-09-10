// SPDX-License-Identifier: GPL-3.0-or-later
import { Worker } from "node:worker_threads";

const PROTOCOL = "geosolve-mirror-worker-v1";
const MAX_CHECKPOINT = 64 * 1024 * 1024, MAX_TEXT = 16 * 1024 * 1024;
const errorMessage = error => String(error?.message ?? error).slice(0, 8192);
const fault = (code, message) => Object.assign(Error(message), { code });
const object = value => value && typeof value === "object" && !Array.isArray(value);
const validRevision = value => object(value) && Array.isArray(value.heads) && value.heads.length <= 4096
  && value.heads.every(head => typeof head === "string" && head.length <= 128);
function validCallback(method, value, identity) {
  if (method === "readCommitted") return value === null;
  if (method === "onPhase") return object(value) && typeof value.name === "string" && value.name.length <= 128 && object(value.detail);
  if (method !== "admitWorkingEdits" || !object(value) || !object(value.operation)
    || value.operation.userId !== identity.userId || value.operation.clientId !== identity.clientId
    || !/^mirror-[a-f0-9-]{36}$/u.test(value.operation.requestId)
    || !validRevision(value.expectedRevision)
    || !Array.isArray(value.edits) || !value.edits.length || value.edits.length > 256) return false;
  let bytes = 0;
  for (const edit of value.edits) {
    if (!object(edit) || !["splice", "create_file", "remove_file", "rename_file"].includes(edit.kind)
      || typeof edit.path !== "string" || edit.path.length > 512) return false;
    for (const field of ["insert", "text", "new_path"]) {
      if (edit[field] !== undefined) {
        if (typeof edit[field] !== "string") return false;
        bytes += Buffer.byteLength(edit[field]);
      }
    }
  }
  return bytes <= MAX_TEXT;
}
function validSnapshot(value) {
  if (!object(value) || !(value.checkpoint instanceof Uint8Array) || value.checkpoint.byteLength > MAX_CHECKPOINT
    || !object(value.snapshot) || !object(value.snapshot.working) || !object(value.snapshot.working.files)
    || !validRevision(value.snapshot.working.revision) || !object(value.snapshot.fileIds)
    || Object.keys(value.snapshot.fileIds).length > 512
    || Object.values(value.snapshot.fileIds).some(id => typeof id !== "string" || id.length > 256)) return false;
  let bytes = 0;
  for (const group of [value.snapshot.working]) {
    if (!object(group.files) || Object.keys(group.files).length > 512) return false;
    for (const [path, text] of Object.entries(group.files)) {
      if (path.length > 512 || typeof text !== "string" || Buffer.byteLength(text) > 4 * 1024 * 1024) return false;
      bytes += Buffer.byteLength(text);
    }
  }
  return bytes <= MAX_TEXT;
}

function validStatus(value) {
  return object(value) && ["synchronized", "reconciliation_pending", "pending", "uninitialized"].includes(value.status)
    && Number.isSafeInteger(value.generation) && value.generation >= 0
    && [null, "admission", "export"].includes(value.pendingPhase)
    && typeof value.manifestPath === "string" && value.manifestPath.length <= 8192
    && Array.isArray(value.notices) && value.notices.length <= 1024
    && value.notices.every(notice => object(notice) && (notice.path === null || typeof notice.path === "string" && notice.path.length <= 512)
      && typeof notice.reason === "string" && notice.reason.length <= 8192);
}

/** The caller keeps its workspace lock through close(). Bulk mirror/native text
 * processing and filesystem scans execute in a worker. Only committed checkpoint
 * bytes and exact authenticated admission JSON cross the callback boundary.
 * Already-started callbacks are drained even after worker timeout/termination. */
export async function createMirrorWorker(options) {
  const { folder, documentId, documentEpoch, userId, clientId, readCommitted, admitWorkingEdits, onPhase } = options;
  const timeoutMs = options.timeoutMs ?? 60_000;
  if (typeof folder !== "string" || ![documentId, documentEpoch, userId, clientId].every(value => typeof value === "string" && /^[A-Za-z0-9_.:-]{1,128}$/u.test(value))
    || typeof readCommitted !== "function" || typeof admitWorkingEdits !== "function" || (onPhase !== undefined && typeof onPhase !== "function")
    || !Number.isSafeInteger(timeoutMs) || timeoutMs < 1 || timeoutMs > 300_000) throw Error("Invalid mirror worker configuration");
  const identity = { folder, documentId, documentEpoch, userId, clientId };
  const worker = new Worker(new URL("./collaboration-mirror-worker.mjs", import.meta.url), {
    workerData: { ...identity, phases: !!onPhase }, execArgv: [], stdout: true, stderr: true,
    resourceLimits: { maxOldGenerationSizeMb: 512 },
  });
  let state = "open", nextId = 0, callbackId = 0, active, tail = Promise.resolve();
  let reconcilePromise, statusPromise, closing, terminating, output = 0, terminalError;
  const callbacks = new Set();
  let exitResolve;
  const exited = new Promise(resolve => { exitResolve = resolve; });
  function terminate(error) {
    terminalError ??= error;
    if (state !== "closed") state = "closing";
    if (!terminating) terminating = worker.terminate().then(() => exited);
    return terminating;
  }
  function rejectActive(error) {
    if (!active) return;
    const pending = active; active = undefined; clearTimeout(pending.timer);
    // Do not expose an operation failure while its worker can still write files.
    void terminate(error).then(() => pending.reject(error), pending.reject);
  }
  function protocolError(message) { rejectActive(fault("mirror_protocol", message)); void terminate(fault("mirror_protocol", message)); }
  function send(message) {
    try { worker.postMessage({ protocol: PROTOCOL, ...message }); }
    catch (error) { protocolError(`Mirror worker mailbox failed: ${errorMessage(error)}`); }
  }
  worker.on("message", message => {
    if (terminating || state === "closed") return;
    if (!object(message) || message.protocol !== PROTOCOL) { protocolError("Foreign mirror worker message"); return; }
    if (message.kind === "closed") {
      if (state !== "closing" || active) { protocolError("Unexpected mirror worker close acknowledgement"); return; }
      // Entry has drained all filesystem work before closing its message port.
      return;
    }
    if (message.kind === "fatal") { protocolError(`Mirror worker protocol failure: ${errorMessage(message.error)}`); return; }
    if (message.kind === "result") {
      if (!active || message.id !== active.id || typeof message.ok !== "boolean"
        || message.ok && (active.method === "initialize" ? message.value !== null : !validStatus(message.value))
        || !message.ok && (typeof message.error !== "string" || message.error.length > 8192)) { protocolError("Mismatched or malformed mirror worker result"); return; }
      const pending = active; active = undefined; clearTimeout(pending.timer);
      if (message.ok) pending.resolve(message.value);
      else pending.reject(fault("mirror_failed", errorMessage(message.error)));
      if (state === "closing") send({kind:"close"});
      return;
    }
    if (message.kind !== "callback" || state !== "open" || !active || message.requestId !== active.id
      || !Number.isSafeInteger(message.id) || message.id !== callbackId + 1 || callbacks.size
      || !validCallback(message.method, message.value, identity) || message.method === "onPhase" && !onPhase) {
      // Closing callbacks are answered with refusal so the worker can finish its
      // current fs-safe boundary without entering another authority callback.
      if (state === "closing" && message.kind === "callback") {
        send({kind:"callback_result",id:message.id,requestId:message.requestId,ok:false,error:"Mirror is closing"}); return;
      }
      protocolError("Unknown, duplicate or excessive mirror callback"); return;
    }
    callbackId = message.id;
    const invoke = message.method === "readCommitted" ? readCommitted
      : message.method === "admitWorkingEdits" ? () => admitWorkingEdits(message.value)
      : () => onPhase(message.value.name, message.value.detail);
    const task = Promise.resolve().then(invoke).then(value => {
      if (message.method === "readCommitted" && !validSnapshot(value)) throw Error("Committed mirror snapshot exceeds bounds or is malformed");
      if (message.method === "admitWorkingEdits" && (!object(value) || !["committed", "rejected"].includes(value.status)
        || value.reason !== undefined && (typeof value.reason !== "string" || value.reason.length > 8192))) throw Error("Invalid durable mirror admission result");
      // Only working text and file identity are needed. Do not duplicate the
      // accepted snapshot/history on every worker read or transfer native handles.
      const result = message.method === "readCommitted" ? {checkpoint:value.checkpoint,
        snapshot:{working:value.snapshot.working,fileIds:value.snapshot.fileIds}} : message.method === "onPhase" ? null : value;
      if (state === "open") send({kind:"callback_result",id:message.id,requestId:message.requestId,ok:true,value:result});
      else if (!terminating) send({kind:"callback_result",id:message.id,requestId:message.requestId,ok:false,error:"Mirror closed after callback; retry exact persisted admission"});
    }, error => {
      if (!terminating) send({kind:"callback_result",id:message.id,requestId:message.requestId,ok:false,error:errorMessage(error)});
    }).catch(error => {
      if (!terminating) send({kind:"callback_result",id:message.id,requestId:message.requestId,ok:false,error:errorMessage(error)});
    }).finally(() => callbacks.delete(task));
    callbacks.add(task);
  });
  worker.once("error", error => { rejectActive(fault("mirror_worker_failed", errorMessage(error))); void terminate(error); });
  worker.once("exit", code => {
    exitResolve();
    const error = terminalError ?? fault("mirror_worker_failed", `Mirror worker exited (${code})`);
    if (active) { const pending = active; active = undefined; clearTimeout(pending.timer); pending.reject(error); }
    state = "closed";
  });
  for (const pipe of [worker.stdout, worker.stderr]) pipe.on("data", chunk => {
    output += chunk.byteLength;
    if (output > 1024 * 1024) { rejectActive(fault("mirror_limit", "Mirror diagnostics exceed 1 MiB")); void terminate(fault("mirror_limit", "Mirror diagnostic limit")); }
  });
  function request(method) {
    if (state !== "open") return Promise.reject(terminalError ?? Error("Mirror worker is closed"));
    const promise = tail.then(() => {
      if (state !== "open") throw terminalError ?? Error("Mirror worker is closed");
      return new Promise((resolve, reject) => {
        const id = ++nextId;
        const timer = setTimeout(() => rejectActive(fault("mirror_timeout", `Mirror worker exceeded ${timeoutMs} ms`)), timeoutMs);
        active = { id, method, resolve, reject, timer }; send({kind:"request",id,method});
      });
    });
    tail = promise.catch(() => {});
    void promise.catch(() => {}); return promise;
  }
  function close() {
    if (closing) return closing;
    if (state === "open") {
      state = "closing";
      // Drain active native/filesystem work. Its next callback is refused, or
      // its complete result triggers close. Idle workers can close immediately.
      if (!active) send({kind:"close"});
    }
    closing = (async () => {
      await tail;
      await exited;
      await Promise.allSettled([...callbacks]);
      if (terminating) await terminating;
    })();
    return closing;
  }
  try { await request("initialize"); }
  catch (error) { await close(); throw error; }
  return {
    reconcile() {
      if (reconcilePromise) return reconcilePromise;
      reconcilePromise = request("reconcile").finally(() => { reconcilePromise = undefined; });
      void reconcilePromise.catch(() => {}); return reconcilePromise;
    },
    status() {
      if (statusPromise) return statusPromise;
      statusPromise = request("status").finally(() => { statusPromise = undefined; });
      void statusPromise.catch(() => {}); return statusPromise;
    },
    close,
  };
}
