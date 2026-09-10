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

test("configured HTTP authoring preview remains provisional and its terminal uses durable latest-model replay", async t => {
  const f = await fixture(t, { authoringPreview: { enabled: true, preferred: "server" } });
  const app = f.first, alice = await app.connect("alice"), bob = await app.connect("bob"), initial = await app.state(alice);
  assert.deepEqual(initial.document.authoringPreview, { server: true, preferred: "server" });
  const basis = { documentEpoch: alice.connection.documentEpoch, revision: initial.authority.acceptedRevision, sourceDesignDigest: initial.document.model.sourceDesignDigest };
  const target = initial.document.pointTargets.find(item => item.target.address?.owner.address.declaration === "bore").target;
  const opened = await app.request("authoring-preview", alice.token, { action: "begin", basis, kind: "point", target, gestureId: 73,
    viewport: { screen_size: [800, 600], model_center: [0, 0], pixels_per_model_unit: 10 } });
  assert.equal(opened.kind, "preview"); assert.equal(typeof opened.presentation, "string");
  await app.request("authoring-preview", alice.token, { action: "advance", ticket: opened.ticket, samples: [{ sequence: 1, position: [8, 4] }] });
  assert.equal((await app.state(alice)).authority.acceptedInput, initial.authority.acceptedInput);
  assert.equal((await app.state(alice)).authority.acceptedRevision, 0);
  assert.equal((await app.send(bob, "intervening-radius", value(initial.document.targets["sketch.ts#other"], "other", 7))).outcome.status, "accepted");
  const terminal = await app.request("authoring-preview", alice.token, { action: "finish", ticket: opened.ticket });
  assert.equal(terminal.kind, "point"); assert.equal(app.runtime.previews.stats().previews, 0);
  const command = { kind: "semantic", basisRevision: 0, payload: { action: "point_gesture", targets: [initial.document.targets["sketch.ts#bore"]], gesture: terminal.terminal.command } };
  const committed = await app.send(alice, "remote-point", command);
  assert.equal(committed.outcome.status, "accepted", JSON.stringify(committed));
  const final = await app.state(alice);
  assert.deepEqual(final.document.pointTargets.find(item => item.target.address?.owner.address.declaration === "bore").position, [8, 4]);
  assert.match(final.document.accepted.files["sketch.ts"], /radius:\s*mm\(7\)/u);
  assert.equal(final.authority.acceptedRevision, 2);
  const stale = await fetch(app.base + "authoring-preview", { method: "POST", headers: { "Content-Type": "application/json", Authorization: `Bearer ${alice.token}` },
    body: JSON.stringify({ action: "begin", basis, kind: "point", target, gestureId: 74, viewport: { screen_size: [800, 600], model_center: [0, 0], pixels_per_model_unit: 10 } }) });
  assert.equal(stale.status, 409); assert.equal((await stale.json()).error.code, "preview_stale_basis");
  assert.equal((await app.state(alice)).authority.acceptedInput, final.authority.acceptedInput);
});

async function predictedCommand(t, initial, kind, positions, suppressed = true) {
  const engine = await createEngine(); t.after(() => engine.dispose());
  const local = engine.openEditableSession(initial.model.project, { design: initial.model.design }); t.after(() => local.dispose());
  const options = { expected: local.token, gestureId: 99, viewport: { screen_size: [800, 600], model_center: [0, 0], pixels_per_model_unit: 10 } };
  if (kind === "point") {
    const target = local.pointGestureTargets().find(entry => entry.target.address?.owner.address.declaration === "bore").target;
    const prediction = local.beginPointGesture(target, options);
    assert.equal(prediction.advance({ sequence: 1, position: positions[0] }).accepted, true);
    return { kind: "semantic", basisRevision: 0, payload: { action: "point_gesture", targets: [initial.targets["sketch.ts#bore"]], gesture: prediction.finish().command } };
  }
  const prediction = local.beginConstruction(kind, options); let sequence = 0;
  for (const position of positions) for (const event of ["move", "click"]) {
    const frame = prediction.advance({ sequence: ++sequence, input: { event, position, suppressed, regularized: false } });
    assert.equal(frame.diagnostic, null);
  }
  return { kind: "semantic", basisRevision: 0, payload: { action: "construction", gesture: prediction.finish() } };
}

test("concurrent constructions replay their immutable original basis and retain distinct durable allocation results", async t => {
  const f = await fixture(t), app = f.first, alice = await app.connect("alice"), bob = await app.connect("bob");
  const initial = (await app.state(alice)).document;
  const a = await predictedCommand(t, initial, "segment", [[-40, -20], [-20, -13]]);
  const b = await predictedCommand(t, initial, "segment", [[40, 30], [50, 40]]);
  const outcomes = await Promise.all([app.send(alice, "create-a", a), app.send(bob, "create-b", b)]);
  for (const receipt of outcomes) assert.equal(receipt.outcome.status, "accepted", JSON.stringify(receipt));
  const state = await app.state(alice), mappingA = (await app.request("result?requestId=create-a", alice.token)).result;
  const mappingB = (await app.request("result?requestId=create-b", bob.token)).result;
  assert.equal(state.authority.acceptedRevision, 2);
  assert.equal(state.document.inventory.objects.length, initial.inventory.objects.length + 2);
  assert.equal(mappingA.allocationMapping.length, 1); assert.equal(mappingB.allocationMapping.length, 1);
  assert.notEqual(mappingA.allocationMapping[0].persistent, mappingB.allocationMapping[0].persistent);
  assert.equal((await app.request("result?requestId=create-a", bob.token)).result, null);
  const before = state.authority.acceptedInput;
  await app.runtime.close(); const restored = await f.open(false), rejoined = await restored.connect("bob");
  assert.deepEqual((await restored.request("commands", rejoined.token, { requestId: "create-b", command: b })).receipt, outcomes[1]);
  assert.deepEqual((await restored.request("result?requestId=create-b", rejoined.token)).result, mappingB);
  assert.equal((await restored.state(rejoined)).authority.acceptedInput, before);
});

test("stale point gestures retain disjoint source edits and later same-point writes win without borrowing a replacement lifetime", async t => {
  const f = await fixture(t), app = f.first, alice = await app.connect("alice"), bob = await app.connect("bob");
  const initial = (await app.state(alice)).document, bore = initial.targets["sketch.ts#bore"];
  const a = await predictedCommand(t, initial, "point", [[4, 5]]), b = await predictedCommand(t, initial, "point", [[8, 9]]);
  assert.equal((await app.send(bob, "resize-other", value(initial.targets["sketch.ts#other"], "other", 7))).outcome.status, "accepted");
  for (const [who, id, command] of [[alice, "move-a", a], [bob, "move-b", b]]) {
    const receipt = await app.send(who, id, command); assert.equal(receipt.outcome.status, "accepted", JSON.stringify(receipt));
  }
  let state = await app.state(alice);
  assert.deepEqual(state.document.pointTargets.find(entry => entry.target.address?.owner.address.declaration === "bore").position, [8, 9]);
  assert.match(state.document.accepted.files["sketch.ts"], /mm\(7\)/u);
  assert.equal((await app.send(alice, "undo-foreign-point", { kind: "undo", basisRevision: 0, payload: {} })).outcome.status, "rejected");
  const forged = structuredClone(a); forged.payload.gesture.target.address.owner.generation++;
  assert.equal((await app.send(alice, "forged-native-owner", forged)).outcome.status, "rejected");
  assert.equal((await app.state(alice)).authority.acceptedInput, state.authority.acceptedInput);
  const deletion = { kind: "semantic", basisRevision: 0, payload: { action: "mutation", targets: [bore], deletion: { roots: [bore], closure: [bore] }, mutation: { mutation: "delete", target: { target: "declaration", declaration: "bore" } } } };
  assert.equal((await app.send(bob, "delete-bore", deletion)).outcome.status, "accepted");
  assert.equal((await app.send(bob, "restore-bore", { kind: "undo", basisRevision: 0, payload: {} })).outcome.status, "accepted");
  state = await app.state(alice); assert.ok(state.document.targets["sketch.ts#bore"].generation > bore.generation);
  // Even substituting the fresh outer token cannot authenticate an old native
  // gesture as an edit of the restored object's new lifetime.
  const borrowed = structuredClone(a); borrowed.payload.targets = [state.document.targets["sketch.ts#bore"]];
  assert.equal((await app.send(alice, "borrow-new-generation", borrowed)).outcome.status, "rejected");
  assert.equal((await app.state(alice)).authority.acceptedInput, state.authority.acceptedInput);
  assert.equal(app.runtime.host.snapshot().needsRecovery, false);
});

test("queued gesture resumes its admitted historical checkpoint after restart and external inference cannot acquire a restored lifetime", async t => {
  const f = await fixture(t), app = f.first, alice = await app.connect("alice"), bob = await app.connect("bob");
  const initial = (await app.state(alice)).document, bore = initial.targets["sketch.ts#bore"];
  const command = await predictedCommand(t, initial, "segment", [[0, 0], [35, 15]], false);
  assert.ok(command.payload.gesture.expected_declarations.some(item => JSON.stringify(item).includes("bore")));
  assert.equal((await app.send(bob, "resize-other", value(initial.targets["sketch.ts#other"], "other", 7))).outcome.status, "accepted");
  // Stop only disposable transport before admitting through the trusted host.
  // This freezes a genuinely pending journal operation without substituting a
  // client-supplied model or mocking native checkpoint restoration.
  app.runtime.transport.stop();
  const historical = await app.runtime.host.acceptedCheckpoint(0);
  const admitted = await app.runtime.host.admit({ connection: alice.connection, requestId: "queued-construction", command }, () => ({
    replayBasis: Buffer.from(JSON.stringify({ revision: 0, acceptedInput: historical.acceptedInput })), replayModel: historical.checkpoints.model,
    replaySource: historical.checkpoints.source, replayTargets: historical.checkpoints.targets, replayHistory: historical.checkpoints.history,
  }));
  assert.equal(admitted.outcome, null); await app.runtime.close();
  const restored = await f.open(false), rejoined = await restored.connect("alice"), b = await restored.connect("bob");
  const outcome = await restored.send(rejoined, "queued-construction", command); assert.equal(outcome.outcome.status, "accepted", JSON.stringify(outcome));
  assert.match((await restored.state(rejoined)).document.accepted.files["sketch.ts"], /bore\.center/u);
  assert.equal((await restored.send(rejoined, "undo-create", { kind: "undo", basisRevision: 0, payload: {} })).outcome.status, "accepted");
  const deletion = { kind: "semantic", basisRevision: 0, payload: { action: "mutation", targets: [bore], deletion: { roots: [bore], closure: [bore] }, mutation: { mutation: "delete", target: { target: "declaration", declaration: "bore" } } } };
  assert.equal((await restored.send(b, "delete-old-operand", deletion)).outcome.status, "accepted");
  assert.equal((await restored.send(b, "restore-operand", { kind: "undo", basisRevision: 0, payload: {} })).outcome.status, "accepted");
  const before = await restored.state(rejoined);
  assert.ok(before.document.targets["sketch.ts#bore"].generation > bore.generation);
  const rejected = await restored.send(rejoined, "old-inference-new-lifetime", command);
  assert.equal(rejected.outcome.status, "rejected", JSON.stringify(rejected));
  assert.equal((await restored.state(rejoined)).authority.acceptedInput, before.authority.acceptedInput);
  assert.equal(restored.runtime.host.snapshot().needsRecovery, false);
});

test("concurrent parameter extraction allocates on the server and personal Undo protects foreign parameter writes", async t => {
  const f=await fixture(t),app=f.first,alice=await app.connect("alice"),bob=await app.connect("bob"),initial=(await app.state(alice)).document;
  const extraction=declaration=>({kind:"semantic",basisRevision:0,payload:{action:"mutation",targets:[initial.targets[`sketch.ts#${declaration}`]],mutation:{mutation:"extract_parameter",declaration,path:["radius"],symbol:"parameter1",variable:"parameter1",presentation:{label:`${declaration} radius`,isKeyParameter:true}}}});
  const receipts=await Promise.all([app.send(alice,"extract-bore",extraction("bore")),app.send(bob,"extract-other",extraction("other"))]);
  for(const receipt of receipts)assert.equal(receipt.outcome.status,"accepted",JSON.stringify(receipt));
  const a=(await app.request("result?requestId=extract-bore",alice.token)).result.allocationMapping[0].persistent;
  const b=(await app.request("result?requestId=extract-other",bob.token)).result.allocationMapping[0].persistent;
  assert.notEqual(a,b);
  let state=(await app.state(alice)).document;
  assert.equal(state.inventory.objects.length,4);assert.ok(state.targets[`sketch.ts#${a}`]);assert.ok(state.targets[`sketch.ts#${b}`]);
  const undone = await app.send(alice,"undo-extract",{kind:"undo",basisRevision:0,payload:{}});
  assert.equal(undone.outcome.status,"accepted",JSON.stringify(undone));
  state=(await app.state(alice)).document;
  assert.equal(state.targets[`sketch.ts#${a}`],undefined);assert.ok(state.targets[`sketch.ts#${b}`]);
  const redone = await app.send(alice,"redo-extract",{kind:"redo",basisRevision:0,payload:{}});
  assert.equal(redone.outcome.status,"accepted",JSON.stringify(redone));
  state=(await app.state(alice)).document;
  const write={kind:"semantic",basisRevision:0,payload:{action:"values",writes:[{target:state.targets[`sketch.ts#${a}`],declaration:a,path:[],value:{kind:"unit",value:{unit:"mm",value:2}}}]}};
  assert.equal((await app.send(bob,"same-value-foreign-parameter",write)).outcome.status,"accepted");
  const before=(await app.state(alice)).authority.acceptedInput;
  assert.equal((await app.send(alice,"cannot-remove-foreign-parameter",{kind:"undo",basisRevision:0,payload:{}})).outcome.status,"rejected");
  assert.equal((await app.state(alice)).authority.acceptedInput,before);assert.equal(app.runtime.host.snapshot().needsRecovery,false);
});


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

test("server personal typing history survives restart and duplicate Undo without changing accepted geometry", async (t) => {
  const f = await fixture(t), app = f.first, alice = await app.connect("alice"), bob = await app.connect("bob");
  const before = (await app.state(alice)).authority;
  await app.text(alice, await app.replica(alice), "alice-typing", "radius:mm(2)", "radius:mm(5)");
  await app.text(bob, await app.replica(bob), "bob-typing", "radius:mm(3)", "radius:mm(9)");
  const undoBody = { requestId: "undo-my-typing", action: "undo" };
  const undo = await app.request("text", alice.token, undoBody);
  let state = await app.state(alice);
  assert.match(state.document.working.files["sketch.ts"], /radius:mm\(2\)/u);
  assert.match(state.document.working.files["sketch.ts"], /radius:mm\(9\)/u);
  assert.equal(state.authority.acceptedInput, before.acceptedInput);
  assert.equal(state.authority.acceptedRevision, before.acceptedRevision);
  assert.equal(state.document.textHistory.canRedo, true);
  await app.runtime.close();
  const reopened = await f.open(false), rejoined = await reopened.connect("alice");
  assert.deepEqual(await reopened.request("text", rejoined.token, undoBody), undo);
  const revision = (await reopened.state(rejoined)).document.working.revision;
  await reopened.request("text", rejoined.token, { requestId: "redo-my-typing", action: "redo" });
  state = await reopened.state(rejoined);
  assert.match(state.document.working.files["sketch.ts"], /radius:mm\(5\)/u);
  assert.match(state.document.working.files["sketch.ts"], /radius:mm\(9\)/u);
  assert.equal(state.authority.acceptedInput, before.acceptedInput);
  const delta = await reopened.request("text-state", rejoined.token, { revision });
  assert.equal(delta.history.canUndo, true); assert.equal(delta.history.canRedo, false);
  assert.ok(delta.changes.length > 0);
});

test("file lifecycle shares authenticated personal draft history and preserves another author's text through rename Undo", async (t) => {
  const f = await fixture(t), app = f.first, client = new CollaborationClient({ baseUrl: app.base, inviteToken: "alice", clientId: "file-tab" });
  t.after(() => client.dispose());
  const joined = await client.connect(), acceptedInput = joined.authority.acceptedInput;
  await client.editFiles([{ kind: "create_file", path: "notes.ts", text: "// notes 😀\n" }], joined.document.working.revision, "create-notes");
  let state = await client.refresh();
  const fileId = state.document.fileIds["notes.ts"];
  await client.editFiles([{ kind: "rename_file", path: "notes.ts", new_path: "renamed.ts" }], state.document.working.revision, "rename-notes");
  const bob = await app.connect("bob"), replica = await app.replica(bob), basis = replica.capture().revision;
  replica.edit([{ kind: "splice", path: "renamed.ts", start_utf16: 0, delete_utf16: 0, insert: "// Bob\n" }]);
  await app.request("text", bob.token, { requestId: "bob-notes", changes: replica.changesSince(basis).map(bytes => Array.from(bytes)) });
  await client.undoText(false, "undo-rename");
  state = await client.refresh();
  assert.equal(state.document.fileIds["notes.ts"], fileId);
  assert.equal(state.document.working.files["notes.ts"], "// Bob\n// notes 😀\n");
  assert.equal(state.document.working.files["renamed.ts"], undefined);
  assert.equal(state.authority.acceptedInput, acceptedInput);
  // Removing Alice's original file would also remove Bob's live contribution.
  await assert.rejects(client.undoText(false, "undo-create-protected"), /overwrit|contribution|foreign|changed|ownership/u);
  assert.equal((await app.state(bob)).document.working.files["notes.ts"], "// Bob\n// notes 😀\n");
  assert.equal(app.runtime.host.snapshot().needsRecovery, false);
});

test("actual folder structural deletion Undo restores fresh identity and preserves another editor through restart", async (t) => {
  const f = await fixture(t), app = f.first, alice = await app.connect("alice"), bob = await app.connect("bob");
  const initial = (await app.state(alice)).document, bore = initial.targets["sketch.ts#bore"], other = initial.targets["sketch.ts#other"];
  const command = { kind: "semantic", basisRevision: 0, payload: { action: "mutation", targets: [bore], deletion: { roots: [bore], closure: [bore] },
    mutation: { mutation: "delete", target: { target: "declaration", declaration: "bore" } } } };
  const deleted = await app.send(alice, "delete-bore", command); assert.equal(deleted.outcome.status, "accepted", JSON.stringify(deleted));
  const resized = await app.send(bob, "bob-radius", value(other, "other", 7)); assert.equal(resized.outcome.status, "accepted", JSON.stringify(resized));
  assert.equal((await app.state(alice)).document.semanticHistory.canUndo, true);
  await app.runtime.close(); const restored = await f.open(false), reconnect = await restored.connect("alice");
  const undone = await restored.send(reconnect, "undo-delete", { kind: "undo", basisRevision: 0, payload: {} });
  assert.equal(undone.outcome.status, "accepted", JSON.stringify(undone));
  const state = (await restored.state(reconnect)).document;
  assert.ok(state.targets["sketch.ts#bore"].generation > bore.generation);
  assert.match(state.accepted.files["sketch.ts"], /radius:\s*mm\(7\)/u);
  assert.match(state.accepted.files["sketch.ts"], /radius:\s*mm\(2\)/u);
  const stale = await restored.send(reconnect, "stale-radius", value(bore, "bore", 8)); assert.equal(stale.outcome.status, "rejected");
  const redone = await restored.send(reconnect, "redo-delete", { kind: "redo", basisRevision: 0, payload: {} });
  assert.equal(redone.outcome.status, "accepted", JSON.stringify(redone));
  assert.equal((await restored.state(reconnect)).document.targets["sketch.ts#bore"], undefined);
  assert.equal(restored.runtime.host.snapshot().needsRecovery, false);
});

test("shared metadata Undo has explicit targets and same-value ownership barriers", async (t) => {
  const f = await fixture(t), app = f.first, alice = await app.connect("alice"), bob = await app.connect("bob");
  const target = (await app.state(alice)).document.targets["sketch.ts#bore"];
  const command = { kind: "semantic", basisRevision: 0, payload: { action: "mutation", targets: [target], mutation: {
    mutation: "set_metadata", target: { target: "declaration", declaration: "bore" }, property: "label", value: { kind: "string", value: "Cooling port" },
  } } };
  const edited = await app.send(alice, "label", command); assert.equal(edited.outcome.status, "accepted", JSON.stringify(edited));
  const bobSame = await app.send(bob, "same-label", command); assert.equal(bobSame.outcome.status, "accepted", JSON.stringify(bobSame));
  const blocked = await app.send(alice, "undo-label", { kind: "undo", basisRevision: 0, payload: {} }); assert.equal(blocked.outcome.status, "rejected");
  assert.equal((await app.state(alice)).document.semanticHistory.canUndo, false);
  const bobUndo = await app.send(bob, "undo-same-label", { kind: "undo", basisRevision: 0, payload: {} }); assert.equal(bobUndo.outcome.status, "accepted", JSON.stringify(bobUndo));
  assert.match((await app.state(alice)).document.accepted.files["sketch.ts"], /Cooling port/u);
});

test("captured same-value lexical Apply protects the shared source contribution", async (t) => {
  const f = await fixture(t), app = f.first, alice = await app.connect("alice"), bob = await app.connect("bob");
  const target = (await app.state(alice)).document.targets["sketch.ts#bore"];
  assert.equal((await app.send(alice, "radius5", value(target, "bore", 5))).outcome.status, "accepted");
  await app.text(bob, await app.replica(bob), "literal5", "mm(5)", "mm(5.0)");
  const apply = await app.send(bob, "apply-literal", { kind: "apply", basisRevision: 1, payload: {} }); assert.equal(apply.outcome.status, "accepted", JSON.stringify(apply));
  const undo = await app.send(alice, "undo-before-shared-write", { kind: "undo", basisRevision: 1, payload: {} }); assert.equal(undo.outcome.status, "rejected");
  assert.match((await app.state(alice)).document.accepted.files["sketch.ts"], /mm\(5\.0\)/u);
});

test("canonical parent ownership blocks child Undo while disjoint siblings remain personal", async (t) => {
  const f = await fixture(t), app = f.first, alice = await app.connect("alice"), bob = await app.connect("bob");
  const target = (await app.state(alice)).document.targets["sketch.ts#bore"];
  const write = (path, next) => ({ kind: "semantic", basisRevision: 0, payload: { action: "values", writes: [{ target, declaration: "bore", path, value: next }] } });
  const number = value => ({ kind: "number", value });
  assert.equal((await app.send(alice, "center-x", write(["center", 0], number(4)))).outcome.status, "accepted");
  assert.equal((await app.send(bob, "center-y", write(["center", 1], number(5)))).outcome.status, "accepted");
  assert.equal((await app.send(alice, "undo-x", { kind: "undo", basisRevision: 0, payload: {} })).outcome.status, "accepted");
  let state = (await app.state(alice)).document;
  assert.deepEqual(state.pointTargets.find(entry => entry.target.address?.owner.address.declaration === "bore").position, [0, 5]);
  assert.equal((await app.send(alice, "radius5", value(target, "bore", 5))).outcome.status, "accepted");
  state = (await app.state(alice)).document;
  const root = state.inventory.properties.find(item => item.declaration === "bore" && item.path.length === 0).value;
  assert.equal((await app.send(bob, "same-whole-object", write([], root))).outcome.status, "accepted");
  await app.runtime.close(); const reopened = await f.open(false), rejoined = await reopened.connect("alice");
  const blocked = await reopened.send(rejoined, "undo-radius", { kind: "undo", basisRevision: 0, payload: {} });
  assert.equal(blocked.outcome.status, "rejected", JSON.stringify(blocked));
  assert.match(blocked.outcome.message, /owns|overwrit/u);
  assert.equal(reopened.runtime.host.snapshot().needsRecovery, false);
});

test("point contribution resolves restored target identity after another editor deletes and undoes", async (t) => {
  const f = await fixture(t), app = f.first, alice = await app.connect("alice"), bob = await app.connect("bob");
  const initial = (await app.state(alice)).document, bore = initial.targets["sketch.ts#bore"];
  const engine = await createEngine(); t.after(() => engine.dispose());
  const local = engine.openEditableSession(initial.model.project, { design: initial.model.design }); t.after(() => local.dispose());
  const target = local.pointGestureTargets().find(entry => entry.target.address?.owner.address.declaration === "bore").target;
  const prediction = local.beginPointGesture(target, { expected: local.token, gestureId: 8, viewport: { screen_size: [800, 600], model_center: [0, 0], pixels_per_model_unit: 10 } });
  assert.equal(prediction.advance({ sequence: 1, position: [4, 5] }).accepted, true);
  const moved = await app.send(alice, "move", { kind: "semantic", basisRevision: 0, payload: { action: "point_gesture", targets: [bore], gesture: prediction.finish().command } });
  assert.equal(moved.outcome.status, "accepted", JSON.stringify(moved));
  const deleted = await app.send(bob, "delete", { kind: "semantic", basisRevision: 0, payload: { action: "mutation", targets: [bore], deletion: { roots: [bore], closure: [bore] }, mutation: { mutation: "delete", target: { target: "declaration", declaration: "bore" } } } });
  assert.equal(deleted.outcome.status, "accepted", JSON.stringify(deleted));
  assert.equal((await app.send(bob, "undo-delete", { kind: "undo", basisRevision: 0, payload: {} })).outcome.status, "accepted");
  assert.ok((await app.state(alice)).document.targets["sketch.ts#bore"].generation > bore.generation);
  await app.runtime.close(); const reopened = await f.open(false), rejoined = await reopened.connect("alice");
  const undone = await reopened.send(rejoined, "undo-move", { kind: "undo", basisRevision: 0, payload: {} });
  assert.equal(undone.outcome.status, "accepted", JSON.stringify(undone));
  const state = (await reopened.state(rejoined)).document;
  assert.deepEqual(state.pointTargets.find(entry => entry.target.address?.owner.address.declaration === "bore").position, [0, 0]);
  assert.deepEqual(state.pointTargets.find(entry => entry.target.address?.owner.address.declaration === "other").position, [20, 0]);
  assert.equal(reopened.runtime.host.snapshot().needsRecovery, false);
});

test("external mirror feeds invalid raw saves through durable text gateway and exports other editors without applying", async (t) => {
  const f = await fixture(t, { mirror: true }), app = f.first, alice = await app.connect("alice");
  const initial = (await app.state(alice)), input = initial.authority.acceptedInput;
  assert.equal(app.runtime.mirror().status, "synchronized");
  await app.text(alice, await app.replica(alice), "comment", 'return {bore,other};', '// Alice 😀\n return {bore,other};');
  // The on-disk save still has the old comment-free export, so merging must
  // preserve Alice's independent insertion and the incomplete external literal.
  await writeFile(join(f.folder, "sketch.ts"), source.replace("radius:mm(2)", "radius:mm("));
  const status = await app.runtime.reconcileMirror(); assert.equal(status.status, "synchronized", JSON.stringify(status));
  let state = await app.state(alice);
  assert.match(state.document.working.files["sketch.ts"], /Alice 😀/u);
  assert.match(state.document.working.files["sketch.ts"], /radius:mm\(\}/u);
  assert.equal(state.authority.acceptedInput, input);
  assert.equal(state.authority.acceptedRevision, 0);
  await app.runtime.close(); const reopened = await f.open(false), rejoined = await reopened.connect("alice");
  state = await reopened.state(rejoined);
  assert.match(state.document.working.files["sketch.ts"], /Alice 😀/u);
  assert.equal(state.authority.acceptedInput, input);
  assert.equal(reopened.runtime.mirror().status, "synchronized");
});

test("source suppression has personal Undo and Redo without losing another editor's shape edit", async t => {
  const f = await fixture(t), app = f.first, alice = await app.connect("alice"), bob = await app.connect("bob");
  const targets = (await app.state(alice)).document.targets;
  const mutation = { mutation: "set_suppressed", target: { target: "declaration", declaration: "bore" }, suppressed: true };
  const command = { kind: "semantic", basisRevision: 0, payload: { action: "mutation", mutation, targets: [targets["sketch.ts#bore"]] } };
  const suppressed = await app.send(alice, "suppress-bore", command);
  assert.equal(suppressed.outcome.status, "accepted", JSON.stringify(suppressed));
  assert.match((await app.state(alice)).document.accepted.files["sketch.ts"], /\$\.suppress\(bore\)/u);
  const edit = await app.send(bob, "other-radius", value(targets["sketch.ts#other"], "other", 7));
  assert.equal(edit.outcome.status, "accepted", JSON.stringify(edit));
  const undo = await app.send(alice, "undo-suppression", { kind: "undo", basisRevision: 0, payload: {} });
  assert.equal(undo.outcome.status, "accepted", JSON.stringify(undo));
  const restored = (await app.state(alice)).document.accepted.files["sketch.ts"];
  assert.doesNotMatch(restored, /\$\.suppress/u); assert.match(restored, /mm\(7\)/u);
  const redo = await app.send(alice, "redo-suppression", { kind: "redo", basisRevision: 0, payload: {} });
  assert.equal(redo.outcome.status, "accepted", JSON.stringify(redo));
  const final = await app.state(alice); assert.match(final.document.accepted.files["sketch.ts"], /\$\.suppress\(bore\)/u); assert.match(final.document.accepted.files["sketch.ts"], /mm\(7\)/u);
  await app.runtime.close(); const reopened = await f.open(false), reconnect = await reopened.connect("alice");
  assert.equal((await reopened.state(reconnect)).authority.acceptedInput, final.authority.acceptedInput);
});

test("same-value source suppression owns activation and rejects stale targets transactionally", async t => {
  const f = await fixture(t), app = f.first, alice = await app.connect("alice"), bob = await app.connect("bob");
  const target = (await app.state(alice)).document.targets["sketch.ts#bore"];
  const command = { kind: "semantic", basisRevision: 0, payload: { action: "mutation", targets: [target], mutation: { mutation: "set_suppressed", target: { target: "declaration", declaration: "bore" }, suppressed: true } } };
  for (const [who, id] of [[alice, "first-suppression"], [bob, "same-suppression"]]) {
    const outcome = await app.send(who, id, command); assert.equal(outcome.outcome.status, "accepted", JSON.stringify(outcome));
  }
  const before = (await app.state(alice)).authority.acceptedInput;
  const undo = await app.send(alice, "blocked-suppression-undo", { kind: "undo", basisRevision: 0, payload: {} });
  assert.equal(undo.outcome.status, "rejected"); assert.match(undo.outcome.message, /owns|overwrit/u);
  const stale = structuredClone(command); stale.payload.targets[0].generation += 100;
  assert.equal((await app.send(alice, "stale-suppression", stale)).outcome.status, "rejected");
  assert.equal((await app.state(alice)).authority.acceptedInput, before); assert.equal(app.runtime.host.snapshot().needsRecovery, false);
});
