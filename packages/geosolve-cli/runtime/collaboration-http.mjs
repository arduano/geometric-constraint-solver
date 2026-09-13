// SPDX-License-Identifier: GPL-3.0-or-later
import { createServer } from "node:http";
import { randomBytes } from "node:crypto";

const prefix = "/api/collaboration/";
const jsonBytes = (value) => Buffer.from(JSON.stringify(value));
function problem(code, message, status = 400) { return Object.assign(Error(message), { code, httpStatus: status }); }
function counter(value) { if (!Number.isSafeInteger(value) || value < 0) throw problem("invalid_request", "Expected nonnegative safe integer counter"); return value; }
function object(value) { if (!value || typeof value !== "object" || Array.isArray(value)) throw problem("invalid_request", "Expected JSON object"); return value; }
function exact(value, keys) { object(value); if (Object.keys(value).some((key) => !keys.includes(key))) throw problem("invalid_request", "Unknown request field"); }
function limitsOf(overrides) {
  const defaults = { maxBodyBytes: 1024 * 1024, maxConnections: 128, maxStreamsPerConnection: 2, maxSubscriberBytes: 256 * 1024, maxPresenceBytes: 16 * 1024, maxRequestsPerSecond: 120, maxConcurrentRequests: 128, sessionIdleMs: 120_000 };
  const limits = { ...defaults, ...overrides };
  if (Object.keys(limits).some((key) => !Object.hasOwn(defaults, key)) || Object.values(limits).some((value) => !Number.isSafeInteger(value) || value < 1)) throw Error("Invalid collaboration HTTP limits");
  return limits;
}
async function readJson(request, maximum) {
  if (!/^application\/json(?:;|$)/iu.test(request.headers["content-type"] ?? "")) throw problem("content_type", "Use application/json", 415);
  const length = request.headers["content-length"];
  if (length !== undefined && (!/^\d+$/u.test(length) || Number(length) > maximum)) throw problem("body_limit", "Request body exceeds the byte limit", 413);
  let size = 0; const chunks = [];
  for await (const chunk of request) { size += chunk.length; if (size > maximum) throw problem("body_limit", "Request body exceeds the byte limit", 413); chunks.push(chunk); }
  try { return JSON.parse(new TextDecoder("utf-8", { fatal: true }).decode(Buffer.concat(chunks))); }
  catch { throw problem("invalid_json", "Expected valid UTF-8 JSON"); }
}
function sendJson(response, status, value) {
  const body = jsonBytes(value);
  response.writeHead(status, { "Content-Type": "application/json; charset=utf-8", "Content-Length": body.length, "Cache-Control": "no-store", "X-Content-Type-Options": "nosniff" }); response.end(body);
}

/** Reference HTTP commands + SSE. Caller supplies invited principals and trusted
 * domain adapters. A network body can never call completion/install/rebuild.
 * Canvas camera/picking/selection stay local; no navigation route exists here.
 */
export function createCollaborationHttpServer({ host, invitations, execute, captureApply, captureSemantic, receiveText, textDelta, documentSnapshot, scene, staticRoutes, authoringPreview, limits: overrides, allowedOrigins = [] }) {
  if (!host || !(invitations instanceof Map) || !invitations.size || typeof execute !== "function") throw Error("A durable host, trusted invitations and domain worker are required");
  const limits = limitsOf(overrides), origins = new Set(allowedOrigins);
  const sessions = new Map(), streams = new Set(), presence = new Map(), pendingJoins = new Set();
  let processing = false, stopped = false, activeRequests = 0, workerError, closing, previewClosing;
  const retiringPreviews = new Set();
  let cachedScene;
  const identity = (userId, clientId) => JSON.stringify([userId, clientId]);
  function dropSession(session) {
    sessions.delete(session.token); presence.delete(session.token);
    for (const stream of session.streams) stream.close();
    if (authoringPreview?.dropConnection) {
      const retired = Promise.resolve().then(() => authoringPreview.dropConnection(session.connection));
      retiringPreviews.add(retired);
      void retired.catch(() => {}).finally(() => retiringPreviews.delete(retired));
    }
  }
  function kick() {
    if (processing || stopped) return;
    processing = true;
    void (async () => {
      try { while (!stopped && host.snapshot().pendingCount && !host.snapshot().needsRecovery) await host.runNext(execute); }
      catch (error) { workerError = String(error); notify("status", { state: "recovery_required" }); }
      finally { processing = false; }
    })();
  }
  function participants() { return [...sessions.values()].map(({ connection }) => ({ clientId: connection.clientId, userId: connection.userId, role: connection.role })); }
  function notify(event, data, key = event) { for (const stream of streams) stream.send(event, data, undefined, key); }
  const unsubscribe = host.subscribe((envelope) => {
    if (envelope.payload.kind === "authority") {
      const record = envelope.payload.record;
      if (record.event.event === "finished" && record.event.outcome.status === "accepted") cachedScene = undefined;
      for (const stream of streams) stream.send("operation", record, record.sequence);
    } else if (envelope.payload.kind === "text") notify("text", { sequence: envelope.sequence }, "text");
  });
  function authenticate(request) {
    const match = /^Bearer ([a-f0-9]{64})$/u.exec(request.headers.authorization ?? "");
    const session = match && sessions.get(match[1]);
    if (!session) throw problem("unauthorized", "Join this document before sending requests", 401);
    // Validate epochs and native host session on every request; transport tokens
    // cannot outlive Rust authority restoration or manufacture an editor role.
    host.resume(session.connection, host.snapshot().latestSequence);
    const now = Date.now(); session.lastSeen = now;
    if (now - session.window >= 1000) { session.window = now; session.requests = 0; }
    if (++session.requests > limits.maxRequestsPerSecond) throw problem("backpressure", "Request rate exceeded; retain work and retry", 429);
    return session;
  }
  const server = createServer(async (request, response) => {
    activeRequests++;
    try {
      if (stopped) throw problem("unavailable", "Collaboration transport is stopping", 503);
      if (activeRequests > limits.maxConcurrentRequests) throw problem("backpressure", "Too many concurrent requests", 429);
      if (request.headers.origin && !origins.has(request.headers.origin)) throw problem("origin", "Origin is not admitted by this host", 403);
      const url = new URL(request.url, "http://collaboration.local");
      if (!url.pathname.startsWith(prefix) && ["GET", "HEAD"].includes(request.method)) {
        const asset = staticRoutes?.get(url.pathname);
        if (asset) {
          response.writeHead(200, { "Content-Type": asset.type, "Content-Length": asset.body.length, "Cache-Control": "no-store", "Referrer-Policy": "no-referrer", "X-Content-Type-Options": "nosniff" });
          response.end(request.method === "HEAD" ? undefined : asset.body); return;
        }
      }
      if (!url.pathname.startsWith(prefix)) throw problem("not_found", "Unknown route", 404);
      const route = url.pathname.slice(prefix.length);
      if (request.method === "POST" && route === "join") {
        const body = await readJson(request, limits.maxBodyBytes); exact(body, ["protocol", "inviteToken", "clientId"]);
        if (body.protocol !== 1) throw problem("protocol_mismatch", "Update this client to collaboration protocol 1", 409);
        const principal = invitations.get(body.inviteToken);
        if (!principal) throw problem("unauthorized", "Invitation is not valid for this document", 401);
        if (stopped) throw problem("unavailable", "Collaboration transport is stopping", 503);
        const joining = identity(principal.userId, body.clientId);
        const occupied = new Set([...sessions.values()].map(({ connection }) => identity(connection.userId, connection.clientId)));
        for (const pending of pendingJoins) occupied.add(pending);
        if (pendingJoins.has(joining) || (!occupied.has(joining) && occupied.size >= limits.maxConnections)) throw problem("backpressure", "Document connection limit reached", 429);
        // Reserve before awaiting the host's persistence queue. Parallel joins
        // cannot all observe the same remaining slot and over-admit sessions.
        pendingJoins.add(joining);
        try {
          const connection = await host.connect(principal, body.clientId);
          // Native reconnect replaces this user's same client identity. Retire
          // its old token, streams and presence in the transport as well.
          for (const old of sessions.values()) if (identity(old.connection.userId, old.connection.clientId) === joining) dropSession(old);
          if (stopped || response.destroyed) {
            await host.disconnect(connection);
            if (stopped) throw problem("unavailable", "Collaboration transport is stopping", 503);
            notify("participants", participants());
            return;
          }
          const token = randomBytes(32).toString("hex");
          sessions.set(token, { token, connection, streams: new Set(), window: Date.now(), lastSeen: Date.now(), requests: 0 });
          sendJson(response, 200, { connection, token, authority: host.snapshot(), participants: participants() }); notify("participants", participants()); kick(); return;
        } finally { pendingJoins.delete(joining); }
      }
      const session = authenticate(request);
      if (request.method === "GET" && route === "state") {
        sendJson(response, 200, { authority: host.snapshot(), document: documentSnapshot?.(session.connection), participants: participants(), presence: [...presence.values()], workerError }); return;
      }
      if (request.method === "GET" && route === "events") {
        const after = counter(Number(url.searchParams.get("after") ?? request.headers["last-event-id"] ?? 0));
        if (session.streams.size >= limits.maxStreamsPerConnection) throw problem("backpressure", "Close an existing event stream before opening another", 429);
        // Validate before committing streaming headers so invalid cursors receive
        // a structured error. Replay and registration are synchronous, with no
        // asynchronous gap in which a committed notification could be missed.
        const replay = host.resume(session.connection, after);
        response.writeHead(200, { "Content-Type": "text/event-stream; charset=utf-8", "Cache-Control": "no-store", Connection: "keep-alive", "X-Accel-Buffering": "no" }); response.flushHeaders();
        let stream;
        stream = subscriber(response, limits.maxSubscriberBytes, () => { streams.delete(stream); session.streams.delete(stream); });
        if (stream.closed) return;
        streams.add(stream); session.streams.add(stream);
        if (replay.status === "events") for (const record of replay.records) stream.send("operation", record, record.sequence);
        else stream.send("checkpoint_required", replay, undefined, "checkpoint");
        stream.send("ready", { authority: host.snapshot() }, undefined, "ready"); return;
      }
      if (request.method === "GET" && route === "receipt") {
        const requestId = url.searchParams.get("requestId"); sendJson(response, 200, { receipt: host.receipt(session.connection, requestId) }); return;
      }
      if (request.method === "GET" && route === "result") {
        sendJson(response, 200, { result: await host.operationResult(session.connection, url.searchParams.get("requestId")) }); return;
      }
      if (request.method === "GET" && route === "scene") {
        if (!scene) throw problem("unavailable", "Accepted scene is not ready", 503);
        const snapshot = host.snapshot();
        if (snapshot.needsRecovery) throw problem("recovery_required", "Accepted scene requires recovery", 503);
        const key = `${snapshot.acceptedRevision}:${snapshot.acceptedInput}`;
        if (!cachedScene || cachedScene.key !== key) {
          const value = scene();
          if (!(value instanceof Uint8Array) || value.length > 64 * 1024 * 1024) throw Error("Expected one bounded encoded immutable accepted scene");
          cachedScene = { key, bytes: Buffer.from(value) };
        }
        response.writeHead(200, { "Content-Type": "application/json", "Content-Length": cachedScene.bytes.length, "Cache-Control": "no-cache", ETag: `"${snapshot.acceptedRevision}"`, "X-Geosolve-Revision": String(snapshot.acceptedRevision) }); response.end(cachedScene.bytes); return;
      }
      if (request.method !== "POST") throw problem("not_found", "Unknown route", 404);
      const body = await readJson(request, route === "presence" ? limits.maxPresenceBytes : limits.maxBodyBytes);
      // A slow body can finish after leave or shutdown. In particular, disposable
      // presence must never resurrect a removed session after this await.
      if (stopped) throw problem("unavailable", "Collaboration transport is stopping", 503);
      if (sessions.get(session.token) !== session) throw problem("unauthorized", "This document connection has ended", 401);
      host.resume(session.connection, host.snapshot().latestSequence);
      if (route === "heartbeat") {
        exact(body, []); sendJson(response, 200, { ok: true }); return;
      }
      if (route === "text-state") {
        if (!textDelta) throw problem("unavailable", "Incremental shared text is not ready", 503);
        exact(body, ["revision"]);
        sendJson(response, 200, textDelta(session.connection, body.revision)); return;
      }
      if (route === "authoring-preview") {
        if (!authoringPreview?.request) throw problem("preview_disabled", "Server authoring preview is disabled", 503);
        if (session.connection.role !== "editor") throw problem("preview_forbidden", "Authoring preview requires an editor connection", 403);
        const controller = new AbortController();
        const disconnected = () => { if (!response.writableEnded) controller.abort(); };
        response.once("close", disconnected);
        if (response.destroyed) controller.abort();
        try { sendJson(response, 200, await authoringPreview.request(session.connection, body, { signal: controller.signal })); }
        finally { response.off("close", disconnected); }
        return;
      }
      if (route === "commands") {
        exact(body, ["requestId", "command"]);
        const request = { connection: session.connection, requestId: body.requestId, command: body.command };
        const receipt = await host.admit(request, body.command?.kind === "apply" && captureApply ? () => captureApply(session.connection) : body.command?.kind === "semantic" && captureSemantic ? () => captureSemantic(session.connection, body.command) : undefined);
        sendJson(response, 200, { receipt }); kick(); return;
      }
      if (route === "text") {
        if (!receiveText) throw problem("unavailable", "Shared text is not ready", 503);
        object(body);
        if (typeof body.requestId !== "string") throw problem("invalid_request", "Shared text requests require a durable requestId");
        // writeText authenticates editor role before invoking the staged source
        // adapter. Its exact checkpoint is flushed before ACK or notification.
        const result = await host.writeText(session.connection, () => receiveText(session.connection, body), { requestId: body.requestId, payload: body });
        sendJson(response, 200, result); return;
      }
      if (route === "presence") {
        exact(body, ["sequence", "cursor", "selection"]); counter(body.sequence);
        if (body.cursor !== null && body.cursor !== undefined && (!Array.isArray(body.cursor) || body.cursor.length !== 2 || !body.cursor.every(Number.isFinite))) throw problem("invalid_presence", "Expected finite model-space cursor");
        if (body.selection !== undefined && (!Array.isArray(body.selection) || body.selection.length > 256 || body.selection.some((id) => typeof id !== "string" || id.length > 512))) throw problem("invalid_presence", "Selection presence exceeds bounds");
        const old = presence.get(session.token);
        if (!old || body.sequence > old.sequence) {
          const value = { userId: session.connection.userId, clientId: session.connection.clientId, sequence: body.sequence, cursor: body.cursor ?? null, selection: body.selection ?? [] };
          presence.set(session.token, value); notify("presence", value, `presence:${session.token}`);
        }
        sendJson(response, 200, { ok: true }); return;
      }
      if (route === "leave") {
        exact(body, []); await host.disconnect(session.connection);
        dropSession(session); notify("participants", participants()); sendJson(response, 200, { ok: true }); return;
      }
      throw problem("not_found", "Unknown route", 404);
    } catch (error) {
      if (!response.headersSent) sendJson(response, error.httpStatus ?? (error.code === "backpressure" ? 429 : error.code === "recovery_required" ? 503 : 400), { error: { code: error.code ?? "rejected", message: String(error.message ?? error) } });
      else response.destroy();
    } finally { activeRequests--; }
  });
  server.requestTimeout = 30_000; server.headersTimeout = 10_000; server.keepAliveTimeout = 5_000;
  const retiring = new Set();
  const expiry = setInterval(() => {
    if (stopped) return;
    for (const session of sessions.values()) {
      if (Date.now() - session.lastSeen < limits.sessionIdleMs) continue;
      // Remove disposable transport identity immediately. A request body that
      // completes later cannot revive presence or use this expired token.
      dropSession(session); notify("participants", participants());
      const task = host.disconnect(session.connection).catch(() => {}).finally(() => retiring.delete(task)); retiring.add(task);
    }
  }, Math.min(30_000, Math.max(1, Math.floor(limits.sessionIdleMs / 2))));
  expiry.unref();
  const stop = () => {
    if (stopped) return; stopped = true; clearInterval(expiry); unsubscribe(); for (const stream of streams) stream.close();
    previewClosing = Promise.resolve().then(() => authoringPreview?.close?.());
    void previewClosing.catch(() => {});
  };
  return { server, kick, stats: () => ({ sessions: sessions.size, pendingJoins: pendingJoins.size, streams: streams.size, presence: presence.size, activeRequests, workerBusy: processing, subscriberBytes: [...streams].reduce((sum, stream) => sum + stream.bufferedBytes(), 0) }),
    allowOrigin(origin) { const parsed = new URL(origin); if (parsed.origin !== origin || !["http:", "https:"].includes(parsed.protocol)) throw Error("Expected an explicit host origin"); origins.add(origin); },
    stop,
    close() {
      if (closing) return closing;
      stop();
      closing = (async () => {
        await previewClosing;
        await Promise.all(retiringPreviews);
        await new Promise((resolve, reject) => { server.close((error) => error && error.code !== "ERR_SERVER_NOT_RUNNING" ? reject(error) : resolve()); server.closeIdleConnections(); });
        // The caller may keep the host alive or attach a new transport. Release
        // native sessions after in-flight joins/leaves finish, preserving its cap.
        await Promise.all(retiring);
        try { for (const session of sessions.values()) if (!host.snapshot().needsRecovery) await host.disconnect(session.connection); }
        finally { sessions.clear(); presence.clear(); cachedScene = undefined; }
      })();
      return closing;
    },
  };
}

/** Durable records remain ordered; disposable presence/refresh entries coalesce.
 * Slow/oversized streams close with a resumable hint instead of growing forever.
 */
export function subscriber(response, maxBytes, onClose = () => {}) {
  const queue = []; let bytes = 0, blocked = false, closed = false, heartbeat;
  const close = () => { if (closed) return; closed = true; clearInterval(heartbeat); queue.length = 0; bytes = 0; response.destroy(); onClose(); };
  const flush = () => {
    if (closed || blocked) return;
    try { while (queue.length && !blocked && !closed) { const next = queue.shift(); bytes -= next.bytes.length; blocked = !response.write(next.bytes); } }
    catch { close(); }
  };
  const stream = { close, get closed() { return closed; }, bufferedBytes: () => closed ? 0 : bytes + (response.writableLength ?? 0),
    send(event, data, id, key) {
      if (closed) return;
      const encoded = Buffer.from(`${id === undefined ? "" : `id: ${id}\n`}event: ${event}\ndata: ${JSON.stringify(data)}\n\n`);
      if (id !== undefined) key = undefined;
      if (key) { const index = queue.findIndex((entry) => entry.key === key); if (index >= 0) { bytes -= queue[index].bytes.length; queue.splice(index, 1); } }
      if (bytes + encoded.length + (response.writableLength ?? 0) > maxBytes) {
        // If writable, provide an explicit resumable disposition; once the OS
        // socket is stalled, close immediately and resume from last observed ID.
        const hint = Buffer.from('event: checkpoint_required\ndata: {"reason":"subscriber_backpressure"}\n\n');
        if (!blocked && bytes + hint.length + (response.writableLength ?? 0) <= maxBytes) {
          try { response.write(hint); } catch { /* A failed socket only closes this subscriber. */ }
        }
        close(); return;
      }
      queue.push({ key, bytes: encoded }); bytes += encoded.length; flush();
    },
  };
  response.on("drain", () => { blocked = false; flush(); }); response.on("close", close); response.on("error", close);
  if (!closed) { heartbeat = setInterval(() => { stream.send("heartbeat", {}, undefined, "heartbeat"); }, 15_000); heartbeat.unref(); }
  return stream;
}
