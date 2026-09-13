// SPDX-License-Identifier: GPL-3.0-or-later
import assert from "node:assert/strict";
import test from "node:test";
import { createEngine } from "../dist/index.js";
import { compileManagedSource } from "../../geosolve-sketch-code/dist/src/managed.js";

function project(engine, radius) {
  const compiled = compileManagedSource(`"use geosolve sketch";
import { sketch, mm } from "@geosolve/sketch-code";
export default sketch(($) => {
  const bore = $.geometry.centerRadiusCircle("bore", { center: [0, 0], radius: mm(${radius}) });
  return { bore };
});`);
  return engine.compileProject({ project: "native-session", compiled, customFiles: {}, artifacts: {}, lock: { format: "geosolve-lock-v1", modules: {} } });
}
async function assertRadius(engine, result, radius) {
  const profile = await engine.exportProfiles(result, { chordErrorMm: 0.01 });
  assert.equal(profile.regions.length, 1);
  assert.ok(profile.regions[0].outer.every(([x,y]) => Math.abs(Math.hypot(x,y)-radius) < 1e-9));
}

test("real WASM editable sessions retain immutable results across history, stale requests and disposal", async () => {
  const engine = await createEngine();
  try {
    const two = project(engine, 2), five = project(engine, 5);
    const session = await engine.openEditableSession(two);
    const old = session.accepted;
    const initialToken = session.token;
    const seed = session.interactionSeed({ width: 900, height: 450, pixelRatio: 2 });
    assert.ok(Object.isFrozen(seed));
    assert.deepEqual(JSON.parse(seed.scene).viewport.screen_size, [900, 450]);
    assert.equal(seed.sceneKey, old.result_id);
    assert.deepEqual(engine.interactionSeed(old, { width: 900, height: 450, pixelRatio: 2 }), seed);
    assert.throws(() => session.interactionSeed({ width: -1, height: 450 }), /viewport/);
    assert.equal(session.accepted, old);
    assert.deepEqual(session.managedCompilerPatches(), {});
    assert.equal(JSON.parse(engine.exportWorkspace(old)).version, 8);
    assert.ok(engine.encodeReproduction(engine.exportWorkspace(old)).startsWith("GEOSOLVE_REPRO_V1"));
    assert.ok(Object.isFrozen(initialToken));
    assert.ok(Object.isFrozen(old.geometry));
    assert.equal(old.capabilities.managed_source_edits, true);
    assert.equal(session.state.can_undo, false);
    assert.equal((await session.applyProject(five, { expected: initialToken })).status, "accepted");
    await assertRadius(engine, session.accepted, 5);
    await assertRadius(engine, old, 2);
    const before = session.state;
    assert.equal((await session.applyProject(two, { expected: initialToken })).status, "rejected");
    assert.equal((await session.applyProject("{}", { expected: session.token })).status, "rejected");
    assert.equal(session.state, before);
    const other = await engine.openEditableSession(two);
    const otherBefore = other.state;
    assert.equal((await session.undo({ expected: other.token })).status, "rejected");
    assert.equal(other.state, otherBefore);
    assert.equal(session.state, before);
    assert.equal((await session.undo({ expected: session.token })).status, "accepted");
    await assertRadius(engine, session.accepted, 2);
    assert.equal((await session.redo({ expected: session.token })).status, "accepted");
    await assertRadius(engine, session.accepted, 5);
    const design = session.exportDesign();
    assert.equal(design.format, "geosolve-design-v1");
    assert.ok(!JSON.stringify(design).includes("editor_checkpoint"));
    const restored = await engine.openEditableSession(five, { design });
    assert.deepEqual(restored.exportDesign(), design);
    assert.equal(restored.state.can_undo, false);
    session.dispose(); session.dispose();
    await assert.rejects(() => session.undo({ expected: session.token }), /disposed/);
    await assertRadius(engine, old, 2);
    other.dispose(); restored.dispose();
    engine.dispose();
    assert.throws(() => restored.exportDesign(), /disposed/);
  } finally { engine.dispose(); }
});

test("real WASM restores complete source history without losing either history direction", async () => {
  const engine = await createEngine();
  try {
    const two = project(engine, 2), five = project(engine, 5);
    const session = await engine.openEditableSession(two, { persistableHistory: true });
    assert.equal((await session.applyProject(five, { expected: session.token })).status, "accepted");
    assert.equal((await session.applyProject(two, { expected: session.token })).status, "accepted");
    assert.equal((await session.undo({ expected: session.token })).status, "accepted");
    const history = session.exportHistory(), token = session.token, geometry = session.accepted.geometry;
    assert.equal(session.state.can_undo, true);
    assert.equal(session.state.can_redo, true);
    assert.throws(() => engine.restoreEditableSession(history), /already open/);
    assert.equal(session.exportHistory(), history);
    session.dispose();
    assert.throws(() => session.exportHistory(), /disposed/);
    const restored = await engine.restoreEditableSession(history);
    assert.deepEqual(restored.token, token);
    assert.deepEqual(restored.accepted.geometry, geometry);
    assert.equal(restored.exportHistory(), history);
    assert.equal((await restored.applyProject("{}", { expected: restored.token })).status, "rejected");
    assert.equal(restored.exportHistory(), history);
    assert.equal((await restored.undo({ expected: restored.token })).status, "accepted");
    await assertRadius(engine, restored.accepted, 2);
    assert.equal((await restored.redo({ expected: restored.token })).status, "accepted");
    await assertRadius(engine, restored.accepted, 5);
    assert.equal((await restored.redo({ expected: restored.token })).status, "accepted");
    await assertRadius(engine, restored.accepted, 2);
    assert.equal(restored.state.can_redo, false);
    const presentation = { origin: { kind: "authored" }, selectedFile: "sketch.ts",
      managedDraft: "// unfinished source 😀", draftDiagnostic: null };
    const saved = restored.exportWorkspace(presentation);
    assert.throws(() => engine.restoreEditableWorkspace(saved), /already open/);
    restored.dispose();
    const workspace = engine.restoreEditableWorkspace(saved);
    assert.deepEqual(workspace.restoredWorkspacePresentation, presentation);
    assert.ok(Object.isFrozen(workspace.restoredWorkspacePresentation));
    assert.equal(workspace.exportWorkspace(workspace.restoredWorkspacePresentation), saved);
    await assertRadius(engine, workspace.accepted, 2);
    const view = { hiddenRows: ["managed:stale"], constructionVisible: false,
      dimensions: { mode: "hidden", pins: [] } };
    const outer = workspace.exportWorkspace(presentation, view), completeHistory = workspace.exportHistory();
    assert.equal(JSON.parse(outer).format, "geosolve-workbench-presentation-v1");
    workspace.dispose();
    const folder = engine.restoreEditableWorkspace(outer);
    assert.deepEqual(folder.restoredViewPresentation, view);
    assert.ok(Object.isFrozen(folder.restoredViewPresentation.dimensions.pins));
    assert.equal(folder.exportHistory(), completeHistory);
    assert.equal(folder.exportWorkspace(folder.restoredWorkspacePresentation, folder.restoredViewPresentation), outer);
    folder.dispose();
  } finally { engine.dispose(); }
});
