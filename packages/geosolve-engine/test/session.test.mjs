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
