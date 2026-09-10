// SPDX-License-Identifier: GPL-3.0-or-later
// Protocol tests use actual Rust/WASM authority and fsynced temporary storage.
// The domain checkpoint codec below is a fixture, not geometry qualification.
import assert from "node:assert/strict";
import { test } from "node:test";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { EventEmitter } from "node:events";
import { request as httpRequest } from "node:http";
import { setImmediate as immediate } from "node:timers/promises";
import { openDurableCollaborationHost } from "./collaboration-host.mjs";
import { createCollaborationHttpServer, subscriber } from "./collaboration-http.mjs";

const configuration = { documentId: "http-drawing", documentEpoch: "http-drawing-life", initialInput: "independently-rebuilt-fixture" };
const bytes = (value) => Buffer.from(JSON.stringify(value));
const checkpoints = (input, draft) => ({ model: bytes({ input }), source: bytes({ input, draft }), targets: bytes({ generation: 1 }), history: bytes({ contributions: [] }) });
const invitations = () => new Map([["alice-invite", { userId: "alice", role: "editor" }], ["bob-invite", { userId: "bob", role: "editor" }], ["view-invite", { userId: "visitor", role: "viewer" }]]);
function deferred() { let resolve; const promise = new Promise((done) => { resolve = done; }); return { promise, resolve }; }
async function until(predicate, message = "condition was not reached") {
  const deadline = Date.now() + 3_000;
  while (!predicate()) { if (Date.now() > deadline) throw Error(message); await immediate(); }
}
function command(requestId, value = 14, kind = "semantic") { return { requestId, command: { kind, basisRevision: 0, payload: { property: "width", value } } }; }
function partialRequest(t, base, route, token) {
  let request;
  const result = new Promise((resolve, reject) => {
    request = httpRequest(base + route, { method: "POST", headers: { "Content-Type": "application/json", ...(token ? { Authorization: `Bearer ${token}` } : {}) } }, (response) => {
      const chunks = []; response.on("data", (chunk) => chunks.push(chunk)); response.on("error", reject);
      response.on("end", () => resolve({ status: response.statusCode, body: JSON.parse(Buffer.concat(chunks)) }));
    });
    request.on("error", reject);
  });
  // Cleanup may deliberately abort a request; attach a rejection handler before
  // it can settle, while preserving the result for the test's assertions.
  void result.catch(() => {}); t.after(() => request.destroy());
  request.flushHeaders(); return { request, result };
}

async function fixture(t, options = {}) {
  const folder = await mkdtemp(join(tmpdir(), "geosolve-collaboration-http-"));
  const handles = [], barriers = options.barriers ?? [], eventStreams = [];
  const state = { draft: "const width = 12;", sceneCalls: 0, captures: 0, textCalls: 0, executions: [] };
  async function open(extra = {}) {
    const host = await openDurableCollaborationHost(folder, {
      initial: { configuration, checkpoints: checkpoints(configuration.initialInput, state.draft) },
      rebuild: async ({ acceptedInput, checkpoints: snapshot }) => {
        assert.equal(JSON.parse(snapshot.source).input, acceptedInput);
        return JSON.parse(snapshot.model).input;
      }, ...options.hostOptions, ...extra.hostOptions,
    });
    state.draft = JSON.parse(host.restoredCheckpoints().source).draft;
    const execute = options.execute ?? (async (prepared) => {
      state.executions.push(prepared);
      const input = `validated-fixture-${prepared.operation.requestId}`;
      return () => ({ completion: { status: "accepted", acceptedInput: input, summary: "Fixture commit" }, checkpoints: checkpoints(input, state.draft), install() {} });
    });
    const transport = createCollaborationHttpServer({ host, invitations: invitations(), execute,
      captureApply: () => { state.captures++; return { applyCapture: bytes({ draft: state.draft }) }; },
      receiveText: (_connection, body) => {
        state.textCalls++;
        if (!body || Object.keys(body).sort().join(",") !== "requestId,text" || typeof body.text !== "string") throw Error("Invalid fixture text payload");
        return { checkpoint: checkpoints(host.snapshot().acceptedInput, body.text).source, ack: { draft: body.text },
          commit() { state.draft = body.text; return { draft: state.draft }; }, fail() {} };
      },
      documentSnapshot: () => ({ draft: state.draft }),
      scene: () => { state.sceneCalls++; state.sceneBytes = bytes({ input: host.snapshot().acceptedInput }); return state.sceneBytes; },
      ...options.transportOptions, ...extra.transportOptions,
    });
    await new Promise((resolve, reject) => { transport.server.once("error", reject); transport.server.listen(0, "127.0.0.1", resolve); });
    const base = `http://127.0.0.1:${transport.server.address().port}/api/collaboration/`;
    const handle = { host, transport, base };
    handles.push(handle);
    async function request(route, { token, body, method = body === undefined ? "GET" : "POST", headers = {} } = {}) {
      const response = await fetch(base + route, { method,
        headers: { ...(body === undefined ? {} : { "Content-Type": "application/json" }), ...(token ? { Authorization: `Bearer ${token}` } : {}), ...headers },
        ...(body === undefined ? {} : { body: typeof body === "string" ? body : JSON.stringify(body) }), signal: AbortSignal.timeout(5_000) });
      const text = await response.text();
      return { status: response.status, headers: response.headers, body: text ? JSON.parse(text) : null };
    }
    async function connect(inviteToken = "alice-invite", clientId = "alice-tab") {
      const response = await request("join", { body: { protocol: 1, inviteToken, clientId } });
      assert.equal(response.status, 200, JSON.stringify(response.body)); return response.body;
    }
    async function events(token, after = 0, headers = {}) {
      const controller = new AbortController();
      const response = await fetch(base + "events" + (after === null ? "" : `?after=${after}`), { headers: { Authorization: `Bearer ${token}`, ...headers }, signal: controller.signal });
      assert.equal(response.status, 200);
      const reader = response.body.getReader();
      const observed = [], decoder = new TextDecoder(); let buffer = "", readError;
      const reading = (async () => {
        try {
          while (true) {
            const { value, done } = await reader.read(); if (done) break;
            buffer += decoder.decode(value, { stream: true });
            let end;
            while ((end = buffer.indexOf("\n\n")) >= 0) {
              const packet = buffer.slice(0, end); buffer = buffer.slice(end + 2);
              const fields = Object.fromEntries(packet.split("\n").filter((line) => line.includes(":")).map((line) => [line.slice(0, line.indexOf(":")), line.slice(line.indexOf(":") + 1).trimStart()]));
              if (fields.event) observed.push({ event: fields.event, ...(fields.id === undefined ? {} : { id: Number(fields.id) }), data: JSON.parse(fields.data) });
            }
          }
        } catch (error) { if (!controller.signal.aborted) readError = error; }
      })();
      const stream = { observed, async wait(predicate) { await until(() => readError || observed.some(predicate), "SSE event was not received"); if (readError) throw readError; return observed.find(predicate); }, async close() { controller.abort(); await reading; } };
      eventStreams.push(stream); return stream;
    }
    return Object.assign(handle, { request, connect, events });
  }
  t.after(async () => {
    for (const barrier of barriers) barrier.resolve();
    for (const stream of eventStreams) await stream.close();
    for (const handle of handles.reverse()) {
      await until(() => !handle.transport.stats().workerBusy, "domain fixture worker did not settle");
      await handle.transport.close(); await handle.host.close();
    }
    await rm(folder, { recursive: true, force: true });
  });
  const first = await open();
  return { ...first, state, open, barrier() { const value = deferred(); barriers.push(value); return value; } };
}

test("HTTP invitation identity rejects forged role/completion and viewer writes", async (t) => {
  const { request, connect, host, state } = await fixture(t);
  assert.equal((await request("state")).status, 401);
  assert.equal((await request("join", { body: { protocol: 1, inviteToken: "bad", clientId: "bad" } })).status, 401);
  assert.equal((await request("join", { body: { protocol: 2, inviteToken: "alice-invite", clientId: "bad" } })).status, 409);
  assert.equal((await request("join", { body: { protocol: 1, inviteToken: "view-invite", clientId: "bad", role: "editor" } })).status, 400);
  const viewer = await connect("view-invite", "viewer-tab"), alice = await connect();
  assert.equal(viewer.connection.role, "viewer"); assert.equal(alice.connection.userId, "alice");
  assert.equal((await request("commands", { token: viewer.token, body: command("forbidden") })).status, 400);
  assert.equal((await request("text", { token: viewer.token, body: { requestId: "text-1", text: "invalid =" } })).status, 400);
  assert.equal(state.textCalls, 0);
  assert.equal((await request("commands", { token: alice.token, body: { ...command("forged"), completion: { status: "accepted" } } })).status, 400);
  assert.equal((await request("complete", { token: alice.token, body: { status: "accepted", acceptedInput: "forged" } })).status, 404);
  assert.equal((await request("navigation", { token: alice.token, body: { zoom: 2 } })).status, 404);
  assert.equal(host.snapshot().acceptedRevision, 0); assert.equal(host.snapshot().latestSequence, 0);
});

test("HTTP/SSE presence is independent, disposable and cannot mutate the model", async (t) => {
  const { request, connect, events, host, transport } = await fixture(t);
  const alice = await connect(), bob = await connect("bob-invite", "bob-tab");
  const stream = await events(alice.token); await stream.wait((entry) => entry.event === "ready");
  const before = host.snapshot();
  assert.equal((await request("presence", { token: alice.token, body: { sequence: 2, cursor: [1, 2], selection: ["a"] } })).status, 200);
  await request("presence", { token: bob.token, body: { sequence: 1, cursor: [8, 9], selection: ["b"] } });
  await request("presence", { token: alice.token, body: { sequence: 1, cursor: [0, 0], selection: [] } });
  await stream.wait((entry) => entry.event === "presence" && entry.data.clientId === "bob-tab");
  const state = (await request("state", { token: bob.token })).body;
  assert.deepEqual(state.presence.map((entry) => [entry.clientId, entry.cursor, entry.selection]), [["alice-tab", [1, 2], ["a"]], ["bob-tab", [8, 9], ["b"]]]);
  assert.equal(host.snapshot().acceptedRevision, before.acceptedRevision); assert.equal(host.snapshot().latestSequence, before.latestSequence);
  await request("leave", { token: alice.token, body: {} });
  await until(() => transport.stats().streams === 0);
  assert.equal(transport.stats().presence, 1);
  assert.equal((await request("state", { token: alice.token })).status, 401);
});

test("HTTP reconnect replaces streams and presence even at the native and transport session bound", async (t) => {
  const f = await fixture(t, { transportOptions: { limits: { maxConnections: 1 } }, hostOptions: { limits: { maxSessions: 1 } } });
  const alice = await f.connect(); const stream = await f.events(alice.token);
  await stream.wait((entry) => entry.event === "ready");
  await f.request("presence", { token: alice.token, body: { sequence: 1, selection: ["old"] } });
  const replacement = await f.connect();
  assert.notEqual(replacement.connection.sessionId, alice.connection.sessionId);
  assert.equal((await f.request("state", { token: alice.token })).status, 401);
  const state = (await f.request("state", { token: replacement.token })).body;
  assert.equal(state.participants.length, 1); assert.deepEqual(state.presence, []);
  assert.equal(f.transport.stats().streams, 0); assert.equal(f.transport.stats().sessions, 1);
});

test("HTTP partial presence upload cannot resurrect a session after leave", async (t) => {
  const f = await fixture(t); const alice = await f.connect();
  const upload = partialRequest(t, f.base, "presence", alice.token); upload.request.write('{"sequence":1,');
  await until(() => f.transport.stats().activeRequests === 1);
  assert.equal((await f.request("leave", { token: alice.token, body: {} })).status, 200);
  upload.request.end('"selection":["ghost"]}');
  assert.equal((await upload.result).status, 401);
  assert.equal(f.transport.stats().sessions, 0); assert.equal(f.transport.stats().presence, 0);
  assert.equal(f.host.snapshot().latestSequence, 0);
});

test("HTTP/SSE durable replay and duplicate requests return the original terminal outcome", async (t) => {
  const fixtureState = await fixture(t); const { request, connect, events, host, transport } = fixtureState;
  const alice = await connect(); const stream = await events(alice.token);
  await stream.wait((entry) => entry.event === "ready");
  const admitted = await request("commands", { token: alice.token, body: command("one") });
  assert.equal(admitted.status, 200); assert.equal(admitted.body.receipt.admission, 1);
  const terminal = await stream.wait((entry) => entry.event === "operation" && entry.data.event.event === "finished");
  assert.deepEqual(stream.observed.filter((entry) => entry.event === "operation").map((entry) => entry.id), [1, 2]);
  const receipt = (await request("receipt?requestId=one", { token: alice.token })).body.receipt;
  assert.deepEqual(receipt.outcome, terminal.data.event.outcome);
  assert.deepEqual((await request("commands", { token: alice.token, body: command("one") })).body.receipt, receipt);
  assert.equal((await request("commands", { token: alice.token, body: command("one", 99) })).status, 400);
  assert.equal(host.snapshot().latestSequence, 2);
  const replay = await events(alice.token, 1); await replay.wait((entry) => entry.event === "ready");
  assert.deepEqual(replay.observed.filter((entry) => entry.event === "operation").map((entry) => entry.id), [2]);
  await replay.close(); await until(() => transport.stats().streams === 1);
  const headerReplay = await events(alice.token, null, { "Last-Event-ID": "1" });
  await headerReplay.wait((entry) => entry.event === "ready");
  assert.deepEqual(headerReplay.observed.filter((entry) => entry.event === "operation").map((entry) => entry.id), [2]);
  await stream.close(); await headerReplay.close(); await until(() => !transport.stats().workerBusy);
  await transport.close(); await host.close();
  const restored = await fixtureState.open(); const reconnect = await restored.connect();
  assert.deepEqual((await restored.request("commands", { token: reconnect.token, body: command("one") })).body.receipt, receipt);
  assert.equal(restored.host.snapshot().acceptedRevision, 1);
});

test("held HTTP model work leaves text ACK and admission available and Apply keeps its original capture", async (t) => {
  const hold = deferred(), entered = deferred(); const observed = [];
  const f = await fixture(t, { barriers: [hold], execute: async (prepared, attachments) => {
    observed.push({ requestId: prepared.operation.requestId, attachments });
    if (prepared.operation.requestId === "slow") { entered.resolve(); await hold.promise; }
    return () => ({ completion: { status: "rejected", code: "fixture", message: "No geometry claim" } });
  } });
  const alice = await f.connect(), bob = await f.connect("bob-invite", "bob-tab"), stream = await f.events(alice.token);
  await f.request("commands", { token: alice.token, body: command("slow") }); await entered.promise;
  assert.deepEqual((await f.request("text", { token: bob.token, body: { requestId: "text-2", text: "const width = (" } })).body, { draft: "const width = (" });
  const apply = await f.request("commands", { token: bob.token, body: command("apply", 0, "apply") });
  assert.equal(apply.body.receipt.admission, 2); assert.equal(f.host.snapshot().acceptedRevision, 0);
  await f.request("text", { token: bob.token, body: { requestId: "text-3", text: "const width = 99; // newer typing" } });
  assert.equal(f.state.captures, 1);
  assert.deepEqual((await f.request("commands", { token: bob.token, body: command("apply", 0, "apply") })).body.receipt, apply.body.receipt);
  assert.equal(f.state.captures, 1); assert.equal(f.transport.stats().workerBusy, true);
  hold.resolve(); await stream.wait((entry) => entry.event === "operation" && entry.data.event.event === "finished" && entry.data.event.operation.requestId === "apply");
  assert.equal(JSON.parse(observed[1].attachments.applyCapture).draft, "const width = (");
  assert.equal(f.state.draft, "const width = 99; // newer typing");
  assert.deepEqual(stream.observed.filter((entry) => entry.event === "operation").map((entry) => entry.id), [1, 2, 3, 4]);
});

test("HTTP scene cache copies immutable bytes and invalidates only accepted commits", async (t) => {
  const f = await fixture(t); const alice = await f.connect();
  const first = await f.request("scene", { token: alice.token });
  assert.equal(first.headers.get("x-geosolve-revision"), "0");
  f.state.sceneBytes.fill(0);
  assert.deepEqual((await f.request("scene", { token: alice.token })).body, first.body); assert.equal(f.state.sceneCalls, 1);
  await f.request("presence", { token: alice.token, body: { sequence: 1, selection: ["a"] } });
  await f.request("text", { token: alice.token, body: { requestId: "text-4", text: "const invalid =" } });
  await f.request("scene", { token: alice.token }); assert.equal(f.state.sceneCalls, 1);
  const stream = await f.events(alice.token);
  await f.request("commands", { token: alice.token, body: command("new-scene") });
  await stream.wait((entry) => entry.event === "operation" && entry.data.event.event === "finished");
  const next = await f.request("scene", { token: alice.token });
  assert.equal(next.headers.get("x-geosolve-revision"), "1"); assert.equal(f.state.sceneCalls, 2);
  assert.notDeepEqual(next.body, first.body);
});

test("HTTP concurrent joins cannot bypass the connection bound while durable storage is held", async (t) => {
  const hold = deferred(), entered = deferred(); let enabled = false;
  const f = await fixture(t, { barriers: [hold], transportOptions: { limits: { maxConnections: 2 } }, hostOptions: { storageOptions: { fault: async (point) => {
    if (enabled && point === "journal-synced") { entered.resolve(); await hold.promise; }
  } } } });
  const alice = await f.connect();
  enabled = true;
  const text = f.request("text", { token: alice.token, body: { requestId: "text-5", text: "const invalid =" } }); await entered.promise;
  const joins = ["bob-one", "bob-two"].map((clientId) => f.request("join", { body: { protocol: 1, inviteToken: "bob-invite", clientId } }));
  await until(() => f.transport.stats().activeRequests >= 2);
  await immediate(); await immediate();
  hold.resolve(); await text;
  assert.deepEqual((await Promise.all(joins)).map((entry) => entry.status).sort(), [200, 429]);
  assert.equal(f.transport.stats().sessions, 2);
});

test("HTTP rejects invalid replay cursors before streaming and close disconnects native sessions", async (t) => {
  const f = await fixture(t, { hostOptions: { limits: { maxSessions: 1 } } }); const alice = await f.connect();
  const future = await f.request("events?after=1", { token: alice.token });
  assert.equal(future.status, 400); assert.ok(future.body.error);
  assert.equal(f.transport.stats().streams, 0);
  const stream = await f.events(alice.token); await stream.wait((entry) => entry.event === "ready");
  await f.transport.close();
  assert.equal(f.transport.stats().sessions, 0); assert.equal(f.transport.stats().streams, 0);
  assert.throws(() => f.host.resume(alice.connection, 0));
  const replacement = await f.host.connect({ userId: "bob", role: "editor" }, "new-transport-tab");
  await f.host.disconnect(replacement);
});

test("HTTP abandoned join releases its reservation and native session after the host queue resumes", async (t) => {
  const hold = deferred(), entered = deferred(); let enabled = false;
  const f = await fixture(t, { barriers: [hold], hostOptions: { limits: { maxSessions: 2 }, storageOptions: { fault: async (point) => {
    if (enabled && point === "journal-synced") { entered.resolve(); await hold.promise; }
  } } } });
  const alice = await f.connect(); enabled = true;
  const text = f.request("text", { token: alice.token, body: { requestId: "text-6", text: "const waiting =" } }); await entered.promise;
  let serverSawClose = false;
  f.transport.server.once("request", (_request, response) => response.once("close", () => { serverSawClose = true; }));
  const abandoned = partialRequest(t, f.base, "join");
  abandoned.request.end(JSON.stringify({ protocol: 1, inviteToken: "bob-invite", clientId: "abandoned-tab" }));
  await until(() => f.transport.stats().pendingJoins === 1);
  const closed = new Promise((resolve) => abandoned.request.once("close", resolve));
  abandoned.request.destroy(); await closed; await assert.rejects(abandoned.result);
  await until(() => serverSawClose);
  hold.resolve(); assert.equal((await text).status, 200);
  await until(() => f.transport.stats().pendingJoins === 0 && f.transport.stats().activeRequests === 0);
  assert.equal(f.transport.stats().sessions, 1);
  const next = await f.host.connect({ userId: "bob", role: "editor" }, "replacement-tab");
  await f.host.disconnect(next);
});

test("HTTP bounds active request admission while fsync is held", async (t) => {
  const hold = deferred(), entered = deferred(); let enabled = false;
  const f = await fixture(t, { barriers: [hold], transportOptions: { limits: { maxConcurrentRequests: 1 } }, hostOptions: { storageOptions: { fault: async (point) => {
    if (enabled && point === "journal-synced") { entered.resolve(); await hold.promise; }
  } } } });
  const alice = await f.connect(); enabled = true;
  const text = f.request("text", { token: alice.token, body: { requestId: "text-7", text: "const pending =" } }); await entered.promise;
  assert.equal((await f.request("state", { token: alice.token })).status, 429);
  assert.equal(f.host.snapshot().acceptedRevision, 0);
  hold.resolve(); assert.equal((await text).status, 200);
  assert.equal((await f.request("state", { token: alice.token })).status, 200);
});

test("HTTP rejects malformed UTF-8 and chunked body overflow without registering sessions", async (t) => {
  const f = await fixture(t, { transportOptions: { limits: { maxBodyBytes: 64 } } });
  const invalid = partialRequest(t, f.base, "join");
  invalid.request.end(Buffer.from([0x7b, 0x22, 0x78, 0x22, 0x3a, 0x22, 0xc3, 0x28, 0x22, 0x7d]));
  assert.equal((await invalid.result).status, 400);
  const oversized = partialRequest(t, f.base, "join");
  oversized.request.write('{"inviteToken":"'); await until(() => f.transport.stats().activeRequests === 1);
  oversized.request.end('x'.repeat(100) + '"}');
  assert.equal((await oversized.result).status, 413);
  assert.equal(f.transport.stats().sessions, 0); assert.equal(f.host.snapshot().latestSequence, 0);
});

test("HTTP body, origin, rate and event stream limits reject without leaking sessions", async (t) => {
  const f = await fixture(t, { transportOptions: { limits: { maxBodyBytes: 512, maxPresenceBytes: 64, maxRequestsPerSecond: 4, maxStreamsPerConnection: 1 }, allowedOrigins: ["https://invited.example"] } });
  assert.equal((await f.request("join", { body: "x".repeat(513) })).status, 413);
  assert.equal((await f.request("join", { body: "{}", headers: { "Content-Type": "text/plain" } })).status, 415);
  assert.equal((await f.request("join", { body: {}, headers: { Origin: "https://uninvited.example" } })).status, 403);
  assert.equal((await f.request("join", { body: "{" })).status, 400);
  assert.equal(f.transport.stats().sessions, 0);
  const alice = await f.connect(); const stream = await f.events(alice.token);
  assert.equal((await f.request("events", { token: alice.token })).status, 429);
  assert.equal((await f.request("presence", { token: alice.token, body: { sequence: 1, selection: ["x".repeat(80)] } })).status, 413);
  assert.equal((await f.request("state", { token: alice.token })).status, 200);
  assert.equal((await f.request("state", { token: alice.token })).status, 429);
  await stream.close(); await until(() => f.transport.stats().streams === 0);
});

class SlowResponse extends EventEmitter {
  chunks = []; writableLength = 0; destroyed = false; blocked = true;
  write(chunk) { const copy = Buffer.from(chunk); this.chunks.push(copy); this.writableLength += copy.length; return !this.blocked; }
  destroy() { if (!this.destroyed) { this.destroyed = true; this.writableLength = 0; this.emit("close"); } }
  drain() { this.writableLength = 0; this.blocked = false; this.emit("drain"); }
}

test("SSE subscriber preserves durable order and coalesces disposable presence within its byte bound", () => {
  const response = new SlowResponse(); const stream = subscriber(response, 1_024);
  stream.send("operation", { order: 1 }, 1);
  stream.send("presence", { cursor: 1 }, undefined, "alice");
  stream.send("operation", { order: 2 }, 2);
  stream.send("presence", { cursor: 2 }, undefined, "alice");
  stream.send("operation", { order: 3 }, 3);
  assert.ok(stream.bufferedBytes() <= 1_024);
  response.drain();
  const text = Buffer.concat(response.chunks).toString();
  assert.deepEqual([...text.matchAll(/id: (\d+)/gu)].map((match) => Number(match[1])), [1, 2, 3]);
  assert.ok(!text.includes('"cursor":1')); assert.ok(text.includes('"cursor":2'));
  stream.close(); assert.equal(stream.bufferedBytes(), 0);
});

test("SSE subscriber bounds slow buffers and survives synchronous close and write failures", () => {
  const response = new SlowResponse(); let closed = 0; const stream = subscriber(response, 100, () => { closed++; });
  stream.send("operation", { value: "x".repeat(20) }, 1); stream.send("operation", { value: "x".repeat(80) }, 2);
  assert.equal(response.destroyed, true); assert.equal(closed, 1); assert.equal(stream.bufferedBytes(), 0);
  class ClosedAtRegistration extends SlowResponse {
    on(event, listener) { super.on(event, listener); if (event === "close") listener(); return this; }
  }
  assert.doesNotThrow(() => subscriber(new ClosedAtRegistration(), 100));
  class ThrowsOnWrite extends SlowResponse { write() { throw Error("socket closed while writing"); } }
  const broken = new ThrowsOnWrite(); const failed = subscriber(broken, 100);
  assert.doesNotThrow(() => failed.send("operation", { order: 1 }, 1));
  assert.equal(broken.destroyed, true);
});

test("SSE never coalesces durable IDs or overflows the bound with its recovery hint", () => {
  const response = new SlowResponse(); const stream = subscriber(response, 1_024);
  stream.send("operation", { order: 1 }, 1, "operation");
  stream.send("operation", { order: 2 }, 2, "operation");
  stream.send("operation", { order: 3 }, 3, "operation");
  response.drain();
  assert.deepEqual([...Buffer.concat(response.chunks).toString().matchAll(/id: (\d+)/gu)].map((match) => Number(match[1])), [1, 2, 3]);
  stream.close();
  const tiny = new SlowResponse(); tiny.blocked = false;
  const bounded = subscriber(tiny, 8); bounded.send("operation", { order: 1 }, 1);
  assert.equal(tiny.destroyed, true); assert.ok(Buffer.concat(tiny.chunks).length <= 8);
});

test("expired connections release participants, presence and native slots while live heartbeats retain viewers", async (t) => {
  const f = await fixture(t, { transportOptions: { limits: { sessionIdleMs: 200, maxConnections: 2 } } });
  const alice = await f.request("join", { body: { protocol: 1, inviteToken: "alice-invite", clientId: "abandoned" } });
  const bob = await f.request("join", { body: { protocol: 1, inviteToken: "view-invite", clientId: "live-viewer" } });
  assert.equal(alice.status, 200); assert.equal(bob.status, 200);
  const oldToken = alice.body.token, token = bob.body.token;
  assert.equal((await f.request("presence", { token: oldToken, body: { sequence: 1, cursor: [3, 4], selection: ["ring"] } })).status, 200);
  const deadline = Date.now() + 5000;
  while (f.transport.stats().sessions > 1 && Date.now() < deadline) {
    await new Promise(resolve => setTimeout(resolve, 40));
    assert.equal((await f.request("heartbeat", { token, body: {} })).status, 200);
  }
  assert.equal(f.transport.stats().sessions, 1); assert.equal(f.transport.stats().presence, 0);
  const state = await f.request("state", { token }); assert.equal(state.status, 200);
  assert.deepEqual(state.body.participants.map(person => person.clientId), ["live-viewer"]);
  assert.equal((await f.request("presence", { token: oldToken, body: { sequence: 2, cursor: [5, 6] } })).status, 401);
  assert.equal((await f.request("join", { body: { protocol: 1, inviteToken: "bob-invite", clientId: "replacement" } })).status, 200);
});
