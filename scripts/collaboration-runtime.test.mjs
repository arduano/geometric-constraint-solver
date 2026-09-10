// SPDX-License-Identifier: GPL-3.0-or-later
import assert from "node:assert/strict";
import { test } from "node:test";
import { mkdtemp, writeFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { setTimeout as delay } from "node:timers/promises";
import { openCollaborationRuntime } from "./collaboration-runtime.mjs";
import { createSharedText } from "../packages/geosolve-collaboration/dist/index.js";
import { createEngine } from "../packages/geosolve-engine/dist/index.js";
import { CollaborationClient } from "../packages/geosolve-collaboration/dist/client.js";
const source = `"use geosolve sketch";
import {sketch,mm} from "@geosolve/sketch-code";
export default sketch(($)=>{
 const bore=$.geometry.centerRadiusCircle("bore",{center:[0,0],radius:mm(2)});
 const other=$.geometry.centerRadiusCircle("other",{center:[20,0],radius:mm(3)});
 return {bore,other};
});\n`;
const invitations = new Map([["alice", { userId: "alice", role: "editor" }], ["bob", { userId: "bob", role: "editor" }]]);
async function fixture(t, options = {}) {
  const folder = await mkdtemp(join(tmpdir(), "geosolve-collab-runtime-")), handles = [], clients = [];
  await writeFile(join(folder, "geosolve.json"), JSON.stringify({ format: "geosolve-folder-v2", entry: "sketch.ts", mode: "editable" })); await writeFile(join(folder, "sketch.ts"), source);
  t.after(async () => { for (const client of clients) client.dispose(); for (const runtime of handles.reverse()) await runtime.close(); await rm(folder, { force: true, recursive: true }); });
  async function open(initialize) {
    const runtime = await openCollaborationRuntime(folder, { initialize, invitations, ...options }); handles.push(runtime);
    const address = await runtime.listen(), base = `http://127.0.0.1:${address.port}/api/collaboration/`;
    async function request(route, token, body) {
      const response = await fetch(base + route, { method: body === undefined ? "GET" : "POST", headers: { ...(body === undefined ? {} : { "Content-Type": "application/json" }), ...(token ? { Authorization: `Bearer ${token}` } : {}) }, ...(body === undefined ? {} : { body: JSON.stringify(body) }), signal: AbortSignal.timeout(15_000) });
      const result = await response.json(); assert.equal(response.status, 200, JSON.stringify(result)); return result;
    }
    const connect = async (user) => request("join", undefined, { protocol: 1, inviteToken: user, clientId: `tab-${user}` });
    const state = async (who) => request("state", who.token);
    const send = async (who, requestId, command) => {
      const { receipt } = await request("commands", who.token, { requestId, command }); assert.equal(receipt.operation.requestId, requestId);
      const deadline = Date.now() + 20_000;
      while (Date.now() < deadline) {
        const result = (await request(`receipt?requestId=${requestId}`, who.token)).receipt;
        if (result?.outcome) return result;
        await delay(20);
      }
      throw Error(`Operation ${requestId} did not finish: ${JSON.stringify(runtime.host.snapshot())}`);
    };
    async function replica(who) {
      const snapshot = (await state(who)).document;
      const client = await createSharedText({ actor: Uint8Array.from(snapshot.textActor), checkpoint: Uint8Array.from(snapshot.textCheckpoint) }); clients.push(client); return client;
    }
    async function text(who, client, requestId, before, after) {
      const snapshot = client.capture(), contents = snapshot.files["sketch.ts"], index = contents.indexOf(before); assert.ok(index >= 0);
      client.edit([{ kind: "splice", path: "sketch.ts", start_utf16: index, delete_utf16: before.length, insert: after }]);
      return request("text", who.token, { requestId, changes: client.changesSince(snapshot.revision).map((bytes) => Array.from(bytes)) });
    }
    return { runtime, base, request, connect, state, send, replica, text };
  }
  return { folder, open, first: await open(true) };
}
function value(target, declaration, radius) { return { kind: "semantic", basisRevision: 0, payload: { action: "values", writes: [{ target, declaration, path: ["radius"], value: { kind: "unit", value: { unit: "mm", value: radius } } }] } }; }


test("actual collaborative folder accepts independent edits, personal Undo and original outcomes after native rebuild", async (t) => {
  const f = await fixture(t), app = f.first, alice = await app.connect("alice"), bob = await app.connect("bob");
  const targets = (await app.state(alice)).document.targets;
  const first = await app.send(alice, "radius5", value(targets["sketch.ts#bore"], "bore", 5)); assert.equal(first.outcome.status, "accepted", JSON.stringify(first));
  const second = await app.send(bob, "radius7", value(targets["sketch.ts#other"], "other", 7)); assert.equal(second.outcome.status, "accepted", JSON.stringify(second));
  const undo = await app.send(alice, "undo-radius", { kind: "undo", basisRevision: 0, payload: {} }); assert.equal(undo.outcome.status, "accepted", JSON.stringify(undo));
  const state = await app.state(alice); assert.match(state.document.accepted.files["sketch.ts"], /radius:\s*mm\(2\)/u); assert.match(state.document.accepted.files["sketch.ts"], /radius:\s*mm\(7\)/u);
  const acceptedInput = state.authority.acceptedInput;
  await app.runtime.close(); const reopened = await f.open(false), reconnect = await reopened.connect("alice");
  assert.equal((await reopened.state(reconnect)).authority.acceptedInput, acceptedInput);
  assert.deepEqual((await reopened.request("commands", reconnect.token, { requestId: "radius5", command: value(targets["sketch.ts#bore"], "bore", 5) })).receipt, first);
  const redo = await reopened.send(reconnect, "redo-radius", { kind: "redo", basisRevision: 0, payload: {} }); assert.equal(redo.outcome.status, "accepted", JSON.stringify(redo));
});

test("canvas edits survive incomplete real shared text and Apply leaves later source outside its capture", async (t) => {
  const f = await fixture(t), app = f.first, alice = await app.connect("alice"), bob = await app.connect("bob");
  const client = await app.replica(alice), target = (await app.state(bob)).document.targets["sketch.ts#bore"];
  await app.text(alice, client, "unfinished", "radius:mm(2)", "radius:mm(");
  const edited = await app.send(bob, "canvas5", value(target, "bore", 5)); assert.equal(edited.outcome.status, "accepted", JSON.stringify(edited));
  const state = await app.state(alice);
  assert.ok(state.document.working.files["sketch.ts"].includes("radius:mm(")); assert.match(state.document.accepted.files["sketch.ts"], /mm\(5\)/u); assert.equal(state.document.pendingNotices.length, 1);
  await app.text(alice, client, "finish6", "radius:mm(", "radius:mm(6)");
  const applied = await app.send(alice, "apply6", { kind: "apply", basisRevision: state.authority.acceptedRevision, payload: {} }); assert.equal(applied.outcome.status, "accepted", JSON.stringify(applied));
  assert.match((await app.state(alice)).document.accepted.files["sketch.ts"], /mm\(6\)/u);
});

test("later same-value native edit owns property and blocks the earlier author's Undo", async (t) => {
  const f = await fixture(t), app = f.first, alice = await app.connect("alice"), bob = await app.connect("bob");
  const target = (await app.state(alice)).document.targets["sketch.ts#bore"];
  const a = await app.send(alice, "a5", value(target, "bore", 5)); assert.equal(a.outcome.status, "accepted", JSON.stringify(a));
  const b = await app.send(bob, "b5", value(target, "bore", 5)); assert.equal(b.outcome.status, "accepted", JSON.stringify(b));
  const undo = await app.send(alice, "undo", { kind: "undo", basisRevision: 0, payload: {} }); assert.equal(undo.outcome.status, "rejected");
  assert.match(undo.outcome.message, /owns|overwrit/u); assert.equal(app.runtime.host.snapshot().needsRecovery, false);
});

test("explicit Apply captures its admission text while later typing remains unapplied", async (t) => {
  const f = await fixture(t), app = f.first, alice = await app.connect("alice");
  const client = await app.replica(alice);
  await app.text(alice, client, "type6", "radius:mm(2)", "radius:mm(6)");
  const admission = await app.request("commands", alice.token, { requestId: "apply-capture", command: { kind: "apply", basisRevision: 0, payload: {} } });
  assert.equal(admission.receipt.outcome, null);
  await app.text(alice, client, "type8", "radius:mm(6)", "radius:mm(8)");
  const outcome = await app.send(alice, "apply-capture", { kind: "apply", basisRevision: 0, payload: {} });
  assert.equal(outcome.outcome.status, "accepted", JSON.stringify(outcome));
  const final = (await app.state(alice)).document;
  assert.ok(final.accepted.files["sketch.ts"].includes("radius:mm(6)"));
  assert.ok(final.working.files["sketch.ts"].includes("radius:mm(8)"));
});

test("invalid captured Apply preserves current geometry and a valid later draft can retry", async (t) => {
  const f = await fixture(t), app = f.first, alice = await app.connect("alice");
  const client = await app.replica(alice), before = (await app.state(alice)).authority;
  await app.text(alice, client, "broken", "radius:mm(2)", "radius:mm(");
  const failed = await app.send(alice, "apply-broken", { kind: "apply", basisRevision: 0, payload: {} });
  assert.equal(failed.outcome.status, "rejected");
  assert.equal((await app.state(alice)).authority.acceptedInput, before.acceptedInput);
  await app.text(alice, client, "fixed", "radius:mm(", "radius:mm(4)");
  const accepted = await app.send(alice, "apply-fixed", { kind: "apply", basisRevision: 0, payload: {} });
  assert.equal(accepted.outcome.status, "accepted", JSON.stringify(accepted));
  assert.equal(app.runtime.host.snapshot().needsRecovery, false);
});

test("Apply after a canvas value invalidates stale canvas Undo without claiming shared text for the clicker", async (t) => {
  const f = await fixture(t), app = f.first, alice = await app.connect("alice"), bob = await app.connect("bob");
  const target = (await app.state(alice)).document.targets["sketch.ts#bore"];
  const first = await app.send(alice, "canvas5", value(target, "bore", 5)); assert.equal(first.outcome.status, "accepted", JSON.stringify(first));
  const client = await app.replica(bob);
  const current = client.capture().files["sketch.ts"];
  const match = current.match(/radius:\s*mm\(5\)/u); assert.ok(match);
  await app.text(bob, client, "draft6", match[0], "radius:mm(6)");
  const applied = await app.send(bob, "apply6", { kind: "apply", basisRevision: 1, payload: {} }); assert.equal(applied.outcome.status, "accepted", JSON.stringify(applied));
  const undo = await app.send(alice, "undo-canvas", { kind: "undo", basisRevision: 0, payload: {} });
  assert.equal(undo.outcome.status, "rejected"); assert.equal(app.runtime.host.snapshot().needsRecovery, false);
  const bobUndo = await app.send(bob, "undo-apply", { kind: "undo", basisRevision: 0, payload: {} });
  assert.equal(bobUndo.outcome.status, "rejected");
  assert.ok((await app.state(alice)).document.accepted.files["sketch.ts"].includes("radius:mm(6)"));
});

test("incremental native text through the connected client preserves unsent typing and receives durable SSE outcomes", async (t) => {
  const f = await fixture(t), app = f.first, events = [], saved = [];
  const client = new CollaborationClient({ baseUrl: app.base, inviteToken: "alice", clientId: "alice-client", onEvent: (event) => events.push(event), savePending: (pending) => saved.push(pending) });
  t.after(() => client.dispose());
  const joined = await client.connect(), document = joined.document;
  const alice = await createSharedText({ actor: Uint8Array.from(document.textActor), checkpoint: Uint8Array.from(document.textCheckpoint) });
  t.after(() => alice.dispose());
  const basis = alice.capture().revision;
  alice.edit([{ kind: "splice", path: "sketch.ts", start_utf16: source.length, delete_utf16: 0, insert: "// Alice is still typing 😀\n" }]);
  const bob = await app.connect("bob"), replica = await app.replica(bob);
  await app.text(bob, replica, "bob-radius", "radius:mm(3)", "radius:mm(9)");
  const delta = await client.textDelta(basis);
  assert.equal(delta.changes.length, 1);
  alice.applyServerChanges(delta.changes.map((bytes) => Uint8Array.from(bytes)));
  assert.match(alice.capture().files["sketch.ts"], /mm\(9\)/u);
  assert.match(alice.capture().files["sketch.ts"], /Alice is still typing 😀/u);
  await client.writeText(alice.changesSince(delta.workingRevision), "alice-comment");
  const admission = await client.submit({ kind: "apply", basisRevision: 0, payload: {} }, "apply-both");
  assert.equal(admission.operation.requestId, "apply-both");
  const deadline = Date.now() + 15_000;
  while (!events.some(({ type, data }) => type === "receipt" && data.operation.requestId === "apply-both" && data.outcome) && Date.now() < deadline) await delay(20);
  const terminal = events.find(({ type, data }) => type === "receipt" && data.operation.requestId === "apply-both" && data.outcome);
  assert.equal(terminal?.data.outcome.status, "accepted", JSON.stringify(events));
  assert.equal(client.pendingRequests.length, 0); assert.ok(saved.some((state) => state.requests.length > 0));
  assert.match((await client.refresh()).document.accepted.files["sketch.ts"], /Alice is still typing 😀/u);
});

test("real point gesture contribution survives an independent edit and has property-only personal Undo", async (t) => {
  const f = await fixture(t), app = f.first, alice = await app.connect("alice"), bob = await app.connect("bob");
  const initial = (await app.state(alice)).document;
  const engine = await createEngine(); t.after(() => engine.dispose());
  const local = engine.openEditableSession(initial.model.project, { design: initial.model.design }); t.after(() => local.dispose());
  const target = local.pointGestureTargets().find((entry) => entry.target.address?.owner.address.declaration === "bore").target;
  const preview = local.beginPointGesture(target, { expected: local.token, gestureId: 1, viewport: { screen_size: [800, 600], model_center: [0, 0], pixels_per_model_unit: 10 } });
  assert.equal(preview.advance({ sequence: 1, position: [4, 5] }).accepted, true);
  const gesture = preview.finish().command;
  const moved = await app.send(alice, "move-bore", { kind: "semantic", basisRevision: 0, payload: { action: "point_gesture", targets: [initial.targets["sketch.ts#bore"]], gesture } });
  assert.equal(moved.outcome.status, "accepted", JSON.stringify(moved));
  const edited = await app.send(bob, "resize-other", value(initial.targets["sketch.ts#other"], "other", 7));
  assert.equal(edited.outcome.status, "accepted", JSON.stringify(edited));
  const undone = await app.send(alice, "undo-point", { kind: "undo", basisRevision: 0, payload: {} });
  assert.equal(undone.outcome.status, "accepted", JSON.stringify(undone));
  const final = (await app.state(alice)).document;
  assert.match(final.accepted.files["sketch.ts"], /mm\(7\)/u);
  const point = final.pointTargets.find((entry) => entry.target.address?.owner.address.declaration === "bore");
  assert.deepEqual(point.position, [0, 0]);
  assert.equal(app.runtime.host.snapshot().needsRecovery, false);
});
