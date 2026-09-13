// SPDX-License-Identifier: GPL-3.0-or-later
import assert from "node:assert/strict";
import test from "node:test";
import { mkdtempSync, readFileSync, writeFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { resolve } from "node:path";
import { initProject, serveProject, hash } from "../packages/geosolve-cli/runtime/file-workspace.mjs";

async function setup(t) {
  const folder = mkdtempSync(resolve(tmpdir(), "geosolve-m98-bridge-"));
  initProject(folder);
  // Retain the original single-file compatibility contract explicitly.
  writeFileSync(resolve(folder, "geosolve.json"), JSON.stringify({ format: "geosolve-folder-v1", entry: "sketch.ts" }));
  const bridge = await serveProject(folder);
  t.after(async () => { await bridge.close(); rmSync(folder, { recursive: true, force: true }); });
  const rpc = async (clientId, method, { input, state, operationId, localInteraction, interaction } = {}) => {
    const response = await fetch(`${bridge.origin}/api/rpc`, {
      method: "POST", headers: { Authorization: `Bearer ${bridge.token}`, "Content-Type": "application/json" },
      body: JSON.stringify({ method, input, clientId, baseHash: state?.currentHash, authority: state?.authority, operationId, localInteraction, interaction }),
    });
    return { status: response.status, ...await response.json() };
  };
  return { folder, bridge, rpc };
}

test("canonical folder has one bridge, two tabs require explicit handoff", async (t) => {
  const { folder, rpc } = await setup(t);
  await assert.rejects(() => serveProject(folder), /already|lock|owner/i);
  const initial = await rpc("first-editor", "session.join");
  assert.equal(initial.status, 200);
  assert.equal(initial.state.editor.canEdit, true);
  const viewer = await rpc("second-editor", "session.join");
  assert.equal(viewer.state.editor.canEdit, false);
  const resized = await rpc("second-editor", "resize", { input: { version: 2, width: 600, height: 400, pixelRatio: 1 }, state: viewer.state });
  assert.equal(resized.status, 400, resized.error);
  assert.match(resized.error, /Unsupported workspace method/);
  const denied = await rpc("second-editor", "dispatch", { input: { version: 2, command: "history.undo" }, state: viewer.state });
  assert.equal(denied.status, 409);
  const takeover = await rpc("second-editor", "session.takeover", { state: viewer.state });
  assert.equal(takeover.status, 200);
  assert.equal(takeover.state.editor.canEdit, true);
  const old = await rpc("first-editor", "dispatch", { input: { version: 2, command: "history.undo" }, state: initial.state });
  assert.equal(old.status, 409);
});

// Legacy personal payloads are journal identity only. The server has no route
// that applies them to its semantic authority or executes canvas navigation.
function interactionProbe(t, bridge) {
  const calls = [];
  bridge.project.adapter.interactionApply = async state => { calls.push(state); throw Error("Server applied personal presentation"); };
  t.after(() => { delete bridge.project.adapter.interactionApply; });
  return { calls };
}

test("every observer receives an engine seed and personal input never changes semantic authority", async (t) => {
  const { bridge, rpc } = await setup(t), probe = interactionProbe(t, bridge);
  const first = await rpc("first-editor", "session.join");
  assert.equal(first.status, 200, first.error);
  assert.equal(first.result.format, "geosolve-folder-model-v1");
  assert.ok(first.result.seed.scene); assert.equal(first.result.frame, undefined);
  const viewer = await rpc("second-editor", "session.join", { localInteraction: true, interaction: { reject: true } });
  assert.equal(viewer.status, 200, viewer.error); assert.equal(viewer.state.editor.canEdit, false);
  const observed = await rpc("second-editor", "snapshot", { localInteraction: true, interaction: { reject: true } });
  assert.deepEqual(observed.result, viewer.result);
  assert.deepEqual(observed.result.model, first.result.model);
  assert.deepEqual(probe.calls, []);
});

test("semantic publication validates editor, epoch, lease, revision and disk before native work", async (t) => {
  const { bridge, folder, rpc } = await setup(t), probe = interactionProbe(t, bridge);
  const initial = await rpc("first-editor", "session.join");
  const source = readFileSync(resolve(folder, "sketch.ts"), "utf8");
  const input = { mutation: { mutation: "invalid" } };
  const before = await bridge.project.adapter.persistProject();
  const calls = [], original = bridge.project.adapter.mutation;
  bridge.project.adapter.mutation = async value => { calls.push(value); throw Error("must not enter native mutation"); };
  t.after(() => { bridge.project.adapter.mutation = original; });
  for (const [client, basis] of [
    ["second-editor", initial.state],
    ["first-editor", { ...initial.state, authority: { ...initial.state.authority, epoch: "old-epoch" } }],
    ["first-editor", { ...initial.state, authority: { ...initial.state.authority, lease: initial.state.authority.lease + 1 } }],
    ["first-editor", { ...initial.state, authority: { ...initial.state.authority, revision: initial.state.authority.revision + 1 } }],
    ["first-editor", { ...initial.state, currentHash: "uninstalled-disk" }],
  ]) {
    const denied = await rpc(client, "authoring.mutation", { state: basis, input });
    assert.equal(denied.status, 409, denied.error); assert.deepEqual(calls, []);
  }
  for (const method of ["interaction.sync", "pointer", "wheelBatch", "resize"]) {
    const denied = await rpc("first-editor", method, { state: initial.state, localInteraction: true, interaction: { reject: true } });
    assert.equal(denied.status, 400, denied.error); assert.match(denied.error, /Unsupported workspace method/);
  }
  assert.equal(readFileSync(resolve(folder, "sketch.ts"), "utf8"), source);
  assert.deepEqual(await bridge.project.adapter.persistProject(), before);
  assert.deepEqual(probe.calls, []);
});

test("legacy personal bytes retain durable request identity while receipt retries never execute them", async (t) => {
  const { folder, bridge, rpc } = await setup(t);
  const probe = interactionProbe(t, bridge);
  const initial = await rpc("first-editor", "session.join");
  const source = readFileSync(resolve(folder, "sketch.ts"), "utf8");
  const input = { version: 2, command: "source.prepare", payload: { path: "sketch.ts", contents: source.replace("value: mm(10)", "value: mm(12)") } };
  const interaction = { width: 723 };
  const intent = { input, state: initial.state, operationId: "local-interaction-edit", localInteraction: true, interaction };
  const edited = await rpc("first-editor", "dispatch", intent);
  assert.equal(edited.status, 200, edited.error);
  assert.equal(edited.state.writes, initial.state.writes + 1);
  assert.match(readFileSync(resolve(folder, "sketch.ts"), "utf8"), /value: mm\(12\)/);
  const expectedDigest = hash(JSON.stringify({ method: "dispatch", input, baseHash: initial.state.currentHash,
    clientId: "first-editor", interaction }));
  assert.equal(bridge.storage.outcome(intent.operationId).requestDigest, expectedDigest, "existing M98 journal digest remains exact");
  assert.deepEqual(probe.calls, []);
  const repeated = await rpc("first-editor", "dispatch", intent);
  assert.equal(repeated.status, 200, repeated.error);
  assert.equal(repeated.state.writes, edited.state.writes);
  assert.deepEqual(probe.calls, []);
  const changed = await rpc("first-editor", "dispatch", { ...intent, interaction: { width: 724 } });
  assert.equal(changed.status, 400, changed.error);
  assert.match(changed.error, /different intent/);
  assert.deepEqual(probe.calls, []);
});

test("invalid native authoring terminal retains complete source and history without publication", async (t) => {
  const { folder, bridge, rpc } = await setup(t), probe = interactionProbe(t, bridge);
  const initial = await rpc("first-editor", "session.join");
  const source = readFileSync(resolve(folder, "sketch.ts"), "utf8");
  const before = await bridge.project.adapter.persistProject();
  const rejected = await rpc("first-editor", "authoring.commit", { state: initial.state, operationId: "invalid-native-prediction",
    input: { kind: "construction", command: {} }, localInteraction: true, interaction: { reject: true } });
  assert.equal(rejected.status, 400, rejected.error);
  assert.deepEqual(probe.calls, []);
  assert.equal(readFileSync(resolve(folder, "sketch.ts"), "utf8"), source);
  assert.deepEqual(await bridge.project.adapter.persistProject(), before);
  assert.equal(rejected.state.writes, initial.state.writes);
  assert.equal(bridge.storage.outcome("invalid-native-prediction"), null);
});

test("accepted source edit is journaled and read-only observation never serializes rollback history", async (t) => {
  const { folder, bridge, rpc } = await setup(t);
  const initial = await rpc("first-editor", "session.join");
  const source = readFileSync(resolve(folder, "sketch.ts"), "utf8");
  const candidate = source.replace("value: mm(10)", "value: mm(12)");
  assert.notEqual(candidate, source);
  const input = { version: 2, command: "source.prepare", payload: { path: "sketch.ts", contents: candidate } };
  const edited = await rpc("first-editor", "dispatch", { input, state: initial.state, operationId: "source-edit-1" });
  assert.equal(edited.status, 200, edited.error);
  const written = readFileSync(resolve(folder, "sketch.ts"), "utf8");
  assert.equal(written, "// SPDX-License-Identifier: GPL-3.0-or-later\n" + edited.result.source.files.find((file) => file.path === "sketch.ts").contents);
  assert.match(written, /value: mm\(12\)/);
  assert.equal(edited.state.acceptedHash, hash(written));
  const profiles = await bridge.project.adapter.bakeProfile(0.01);
  assert.ok(profiles.regions.length > 0);
  const vertices = profiles.regions[0].outer;
  assert.ok(vertices.every(([x, y]) => Math.abs(Math.hypot(x, y) - 12) < 1e-7));
  assert.equal(bridge.storage.outcome("source-edit-1").state, "acknowledged");
  const repeated = await rpc("first-editor", "dispatch", { input, state: initial.state, operationId: "source-edit-1" });
  assert.equal(repeated.status, 200, repeated.error);
  assert.equal(repeated.state.writes, edited.state.writes);
  assert.equal(readFileSync(resolve(folder, "sketch.ts"), "utf8"), written);
  const forged = await rpc("second-editor", "dispatch", { input, state: initial.state, operationId: "source-edit-1" });
  assert.equal(forged.status, 400);
  const writes = edited.state.writes;
  const applies = edited.state.externalApplies;
  const persist = bridge.project.adapter.persistProject;
  bridge.project.adapter.persistProject = () => { throw Error("navigation serialized rollback history"); };
  const moved = await rpc("first-editor", "snapshot", { state: edited.state });
  bridge.project.adapter.persistProject = persist;
  assert.equal(moved.status, 200, moved.error);
  assert.equal(moved.state.writes, writes);
  assert.equal(moved.state.externalApplies, applies);
});

test("hidden refresh cannot authorize stale Undo on real source", async (t) => {
  const { folder, bridge, rpc } = await setup(t);
  const initial = await rpc("first-editor", "session.join");
  const changed = readFileSync(resolve(folder, "sketch.ts"), "utf8").replace("value: mm(10)", "value: mm(15)");
  writeFileSync(resolve(folder, "sketch.ts"), changed);
  await bridge.project.scan(true);
  const observed = await rpc("first-editor", "snapshot");
  assert.equal(observed.state.acceptedHash, hash(changed));
  const stale = await rpc("first-editor", "dispatch", { input: { version: 2, command: "history.undo" }, state: initial.state });
  assert.equal(stale.status, 409);
  assert.equal(readFileSync(resolve(folder, "sketch.ts"), "utf8"), changed);
});

test("rejected external source cannot grant stale scene writeback authority", async (t) => {
  const { folder, bridge, rpc } = await setup(t);
  const initial = await rpc("first-editor", "session.join");
  const source = readFileSync(resolve(folder, "sketch.ts"), "utf8");
  const candidate = source.replace("value: mm(10)", "value: mm(12)");
  const edit = await rpc("first-editor", "dispatch", { state: initial.state,
    input: { version: 2, command: "source.prepare", payload: { path: "sketch.ts", contents: candidate } } });
  assert.equal(edit.status, 200, edit.error);
  writeFileSync(resolve(folder, "sketch.ts"), "invalid external source");
  await bridge.project.scan(true);
  const rejected = await rpc("first-editor", "snapshot");
  assert.equal(rejected.state.ok, false);
  const undo = await rpc("first-editor", "dispatch", { state: rejected.state,
    input: { version: 2, command: "history.undo" } });
  assert.equal(undo.status, 409, undo.error);
  assert.equal(readFileSync(resolve(folder, "sketch.ts"), "utf8"), "invalid external source");
});
