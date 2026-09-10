// SPDX-License-Identifier: GPL-3.0-or-later
import { randomBytes } from "node:crypto";
import { Worker } from "node:worker_threads";
import { previewFault, validatePreviewRequest } from "./collaboration-preview-route.mjs";

const defaults = Object.freeze({ maxPreviews: 4, maxPerConnection: 1, maxBasisBytes: 64 * 1024 * 1024, maxTotalBasisBytes: 128 * 1024 * 1024,
  maxRequestBytes: 1024 * 1024, maxResponseBytes: 64 * 1024 * 1024, timeoutMs: 60_000, idleMs: 30_000 });
const maximums = { maxPreviews: 32, maxPerConnection: 4, maxBasisBytes: 64 * 1024 * 1024, maxTotalBasisBytes: 512 * 1024 * 1024,
  maxRequestBytes: 1024 * 1024, maxResponseBytes: 64 * 1024 * 1024, timeoutMs: 300_000, idleMs: 300_000 };
const owner = connection => JSON.stringify([connection.documentEpoch, connection.serverEpoch, connection.sessionId, connection.userId, connection.clientId]);
function finiteJson(value, limit, description) {
  let encoded;
  try {
    encoded = JSON.stringify(value, (_key, child) => {
      if (typeof child === "number" && !Number.isFinite(child) || ["function", "symbol", "bigint"].includes(typeof child)) throw Error("Expected finite JSON data");
      return child;
    });
  } catch { throw previewFault("invalid_preview", `${description} must be finite JSON data`); }
  if (!encoded || Buffer.byteLength(encoded) > limit) throw previewFault("preview_limit", `${description} exceeds configured byte limit`, 413);
  return encoded;
}
/** Opt-in server authoring compute. Trusted callbacks own admission; immutable
 * accepted model snapshots are never taken from network request bodies.
 * Every native operation runs in a bounded, terminable worker distinct from the
 * ordered commit worker. No browser camera, selection or picking runs here.
 */
export function createCollaborationPreviewService({ enabled = false, authenticate, captureBasis, limits: overrides = {} } = {}) {
  if (typeof enabled !== "boolean" || typeof authenticate !== "function" || typeof captureBasis !== "function") throw TypeError("Preview service requires trusted authentication and basis capture callbacks");
  const limits = { ...defaults, ...overrides };
  if (Object.keys(limits).some(key => !Object.hasOwn(defaults, key)) || Object.entries(limits).some(([key, value]) => !Number.isSafeInteger(value) || value < 1 || value > maximums[key])) throw TypeError("Invalid preview resource limits");
  const entries = new Map();
  let stopped = false, totalBytes = 0, closing;
  function authorize(connection) {
    if (!enabled) throw previewFault("preview_disabled", "Server authoring preview is disabled", 503);
    if (stopped) throw previewFault("preview_closed", "Server authoring preview is stopping", 503);
    const checked = authenticate(connection);
    if (checked?.then) { void Promise.resolve(checked).catch(() => {}); throw TypeError("Preview authentication must be synchronous"); }
    if (connection?.role !== "editor" || ![connection.sessionId, connection.documentEpoch, connection.serverEpoch, connection.userId, connection.clientId].every(value => typeof value === "string" && value.length > 0)) throw previewFault("preview_forbidden", "An authenticated editor connection is required", 403);
  }
  function destroy(entry, error = previewFault("preview_cancelled", "Preview ended")) {
    if (entry.ending) return entry.ending;
    clearTimeout(entry.idle);
    const active = entry.active; entry.active = undefined;
    if (active) { clearTimeout(active.timer); active.signal?.removeEventListener("abort", active.abort); }
    entry.ending = entry.worker.terminate().then(() => {
      entries.delete(entry.ticket); totalBytes -= entry.bytes;
      if (active) active.reject(error);
    });
    return entry.ending;
  }
  function create(connection, request) {
    const connectionOwner = owner(connection);
    if (entries.size >= limits.maxPreviews || [...entries.values()].filter(entry => entry.owner === connectionOwner).length >= limits.maxPerConnection) throw previewFault("preview_backpressure", "Finish or cancel an existing preview before opening another", 429);
    const model = captureBasis(connection, request.basis);
    if (model?.then) { void Promise.resolve(model).catch(() => {}); throw TypeError("Preview accepted basis capture must be synchronous"); }
    if (!model || model.documentEpoch !== request.basis.documentEpoch || model.revision !== request.basis.revision
      || model.sourceDesignDigest !== request.basis.sourceDesignDigest) throw previewFault("preview_stale_basis", "Authoring preview basis is no longer accepted", 409);
    const encoded = finiteJson({ basis: request.basis, model: { project: model.project, design: model.design, sourceDesignDigest: model.sourceDesignDigest } }, limits.maxBasisBytes, "Accepted preview basis");
    const bytes = Buffer.byteLength(encoded);
    if (totalBytes + bytes > limits.maxTotalBasisBytes) throw previewFault("preview_backpressure", "Retained preview memory budget is full", 429);
    const ticket = randomBytes(32).toString("hex");
    const worker = new Worker(new URL("./collaboration-preview-worker.mjs", import.meta.url), { workerData: { encoded, maxResponseBytes: limits.maxResponseBytes }, execArgv: [], stdout: true, stderr: true, resourceLimits: { maxOldGenerationSizeMb: 512 } });
    const entry = { ticket, owner: connectionOwner, basis: request.basis, kind: request.kind, worker, bytes, next: 0, active: undefined, idle: undefined, ending: undefined };
    entries.set(ticket, entry); totalBytes += bytes;
    let diagnosticBytes = 0;
    for (const pipe of [worker.stdout, worker.stderr]) pipe.on("data", chunk => {
      diagnosticBytes += chunk.length;
      if (diagnosticBytes > 64 * 1024) void destroy(entry, previewFault("preview_limit", "Native preview diagnostic limit exceeded", 413));
    });
    worker.on("message", message => {
      if (entry.ending) return;
      const active = entry.active;
      if (!active || message?.id !== active.id || typeof message.ok !== "boolean") { void destroy(entry, previewFault("preview_protocol", "Unexpected native preview response", 500)); return; }
      if (!message.ok) { void destroy(entry, previewFault("preview_rejected", String(message.error).slice(0, 8192), 409)); return; }
      let result;
      try {
        if (typeof message.encoded !== "string" || Buffer.byteLength(message.encoded) > limits.maxResponseBytes) throw Error("Native preview response exceeds bounds");
        result = JSON.parse(message.encoded);
        const expectedKind = active.action === "finish" ? entry.kind : "preview";
        if (!result || result.kind !== expectedKind || JSON.stringify(result.basis) !== JSON.stringify(entry.basis)
          || result.kind === "preview" && typeof result.presentation !== "string"
          || result.kind === "point" && !result.terminal?.command || result.kind === "construction" && !result.command) throw Error("Native preview response has foreign basis or operation");
        authorize(active.connection);
      } catch (error) { void destroy(entry, error); return; }
      clearTimeout(active.timer); active.signal?.removeEventListener("abort", active.abort); entry.active = undefined;
      if (active.action === "finish" || active.action === "cancel") void destroy(entry).then(() => active.resolve(result), active.reject);
      else {
        entry.idle = setTimeout(() => { void destroy(entry, previewFault("preview_expired", "Idle authoring preview expired", 410)); }, limits.idleMs); entry.idle.unref();
        active.resolve({ ...result, ticket });
      }
    });
    worker.on("error", error => { void destroy(entry, previewFault("preview_worker_failed", error.message, 500)); });
    worker.on("exit", code => { if (!entry.ending) void destroy(entry, previewFault("preview_worker_failed", `Native preview worker exited (${code})`, 500)); });
    return entry;
  }
  async function request(connection, body, { signal } = {}) {
    authorize(connection);
    const request = validatePreviewRequest(JSON.parse(finiteJson(body, limits.maxRequestBytes, "Preview request")));
    if (signal?.aborted) throw previewFault("preview_cancelled", "Preview request cancelled", 499);
    const entry = request.action === "begin" ? create(connection, request) : entries.get(request.ticket);
    if (!entry || entry.owner !== owner(connection) || entry.ending) throw previewFault("preview_missing", "Preview ticket is absent, foreign, expired or consumed", 410);
    if (request.action === "cancel") { await destroy(entry); return { kind: "cancelled", basis: entry.basis }; }
    if (entry.active) throw previewFault("preview_backpressure", "An authoring preview operation is already running", 429);
    clearTimeout(entry.idle);
    return new Promise((resolve, reject) => {
      const id = ++entry.next;
      const abort = () => { void destroy(entry, previewFault("preview_cancelled", "Preview request cancelled", 499)); };
      const timer = setTimeout(() => { void destroy(entry, previewFault("preview_timeout", `Native preview exceeded ${limits.timeoutMs} ms`, 504)); }, limits.timeoutMs);
      entry.active = { id, action: request.action, resolve, reject, timer, signal, abort, connection };
      signal?.addEventListener("abort", abort, { once: true });
      if (signal?.aborted) { abort(); return; }
      try { entry.worker.postMessage({ id, request }); } catch (error) { void destroy(entry, error); }
    });
  }
  return {
    enabled, request,
    stats: () => ({ enabled, previews: entries.size, busy: [...entries.values()].filter(entry => entry.active).length, retainedBasisBytes: totalBytes }),
    dropConnection: connection => Promise.all([...entries.values()].filter(entry => entry.owner === owner(connection)).map(entry => destroy(entry))).then(() => undefined),
    close() { if (!closing) { stopped = true; closing = Promise.all([...entries.values()].map(entry => destroy(entry))).then(() => undefined); } return closing; },
  };
}
