// SPDX-License-Identifier: GPL-3.0-or-later
import assert from "node:assert/strict";
import test from "node:test";
import { mkdtempSync, readFileSync, writeFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { resolve } from "node:path";
import { initProject, serveProject, hash } from "./file-workspace.mjs";

async function setup(t) {
  const folder = mkdtempSync(resolve(tmpdir(), "geosolve-m98-bridge-"));
  initProject(folder);
  const bridge = await serveProject(folder);
  t.after(async () => { await bridge.close(); rmSync(folder, { recursive: true, force: true }); });
  const rpc = async (clientId, method, { input, state, operationId } = {}) => {
    const response = await fetch(`${bridge.origin}/api/rpc`, {
      method: "POST", headers: { Authorization: `Bearer ${bridge.token}`, "Content-Type": "application/json" },
      body: JSON.stringify({ method, input, clientId, baseHash: state?.currentHash, authority: state?.authority, operationId }),
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
  assert.equal(resized.status, 200, resized.error);
  assert.deepEqual(resized.result.frame.scene.viewBox, viewer.result.frame.scene.viewBox);
  const denied = await rpc("second-editor", "dispatch", { input: { version: 2, command: "history.undo" }, state: viewer.state });
  assert.equal(denied.status, 409);
  const takeover = await rpc("second-editor", "session.takeover", { state: viewer.state });
  assert.equal(takeover.status, 200);
  assert.equal(takeover.state.editor.canEdit, true);
  const old = await rpc("first-editor", "dispatch", { input: { version: 2, command: "history.undo" }, state: initial.state });
  assert.equal(old.status, 409);
});

test("accepted source edit is journaled and navigation neither writes nor serializes rollback history", async (t) => {
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
  const moved = await rpc("first-editor", "wheelBatch", { input: [{ version: 2, x: 100, y: 100, deltaX: 0, deltaY: -20, ctrl: false }], state: edited.state });
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
