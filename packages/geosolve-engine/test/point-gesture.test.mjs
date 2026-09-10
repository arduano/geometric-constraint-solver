// SPDX-License-Identifier: GPL-3.0-or-later
import assert from "node:assert/strict";
import { test } from "node:test";
import { readFile } from "node:fs/promises";
import { createEngine } from "../dist/index.js";

const viewport = { screen_size: [800, 600], model_center: [0, 0], pixels_per_model_unit: 10 };
async function fixture(t) {
  const compiled = JSON.parse(await readFile(new URL("../../../crates/geosolve-sketch-engine/tests/fixtures/point-gesture-constrained.json", import.meta.url), "utf8"));
  const engine = await createEngine(); t.after(() => engine.dispose());
  const project = engine.compileProject({ project: "wasm-point", compiled, customFiles: {}, artifacts: {}, lock: { format: "geosolve-lock-v1", modules: {} } });
  const session = engine.openEditableSession(project); t.after(() => session.dispose());
  return { engine, project, session };
}
function begin(session, gestureId = 7) {
  const target = session.pointGestureTargets().find((handle) => handle.position[0] === 0 && handle.position[1] === 0).target;
  return session.beginPointGesture(target, { expected: session.token, gestureId, viewport });
}
function terminal(session) {
  const gesture = begin(session);
  for (let sequence = 1; sequence <= 4; ++sequence) gesture.advance({ sequence, position: [sequence, sequence] });
  return gesture.finish();
}
function independentBar(result) {
  const points = result.geometry.points.map(({ position }) => position);
  assert.equal(points.length, 2);
  const dx = points[1][0] - points[0][0], dy = points[1][1] - points[0][1];
  assert.ok(Number.isFinite(dx) && Number.isFinite(dy));
  assert.ok(Math.abs(Math.hypot(dx, dy) - 20) < 1e-9);
  assert.ok(Math.abs(dy) < 1e-9 && dx > 0);
  assert.equal(result.validation.hard_residuals_validated, true);
  assert.equal(result.validation.all_active_features_current, true);
}

test("actual WASM retained frames avoid materialization/history and server independently stages durable bar commit", async (t) => {
  const { engine, project, session: prediction } = await fixture(t);
  const serverEngine = await createEngine(); t.after(() => serverEngine.dispose());
  const server = serverEngine.openEditableSession(project); t.after(() => server.dispose());
  const beforePrediction = prediction.state, beforeServer = server.state;
  const gesture = begin(prediction);
  for (let sequence = 1; sequence <= 8; ++sequence) {
    const frame = gesture.advance({ sequence, position: [sequence, sequence] });
    assert.equal(frame.accepted, true);
    assert.equal(frame.work.native_preview_attempts, 1);
    assert.equal(frame.work.intent_materialization_attempts, 0);
    assert.equal(frame.work.history_publications, 0);
    assert.equal(prediction.state, beforePrediction); assert.equal(server.state, beforeServer);
  }
  assert.equal(typeof JSON.parse(gesture.sceneJSON()), "object");
  const { command } = gesture.finish();
  assert.equal(command.samples.length, 8);
  assert.equal(command.basis, server.sourceDesignDigest());
  assert.ok(Object.isFrozen(command.samples));
  assert.throws(() => gesture.advance({ sequence: 9, position: [9, 9] }), /consumed/u);
  const prepared = server.preparePointGestureCommit(command, { expected: server.token });
  assert.equal(server.state, beforeServer); independentBar(prepared.result);
  // Candidate JSON is reviewable and cold-restorable, but has no engine export authority.
  await assert.rejects(() => serverEngine.exportProfiles(prepared.result, { chordErrorMm: 0.1 }), /belongs/u);
  const cold = engine.openEditableSession(prepared.project, { design: prepared.design }); t.after(() => cold.dispose());
  assert.equal(cold.sourceDesignDigest(), prepared.source_design_digest);
  independentBar(cold.accepted);
  const accepted = server.applyPointGestureCommit(prepared);
  assert.equal(accepted.status, "accepted"); independentBar(server.accepted);
  assert.equal(server.token.revision, beforeServer.token.revision + 1);
  assert.deepEqual(server.accepted.geometry, prepared.result.geometry);
  assert.deepEqual(cold.pointGestureTargets(), server.pointGestureTargets());
  assert.deepEqual(cold.accepted.geometry.curves[0].curve.definition.branch_direction, server.accepted.geometry.curves[0].curve.definition.branch_direction);
  assert.equal(server.sourceDesignDigest(), prepared.source_design_digest);
  assert.throws(() => server.applyPointGestureCommit(prepared), /consumed/u);
  assert.throws(() => server.preparePointGestureCommit(command, { expected: server.token }), /basis/u);
});

test("actual WASM gesture sequence, cancellation, disposal and count bounds preserve accepted source", async (t) => {
  const { engine, project, session } = await fixture(t);
  const before = session.state;
  const gesture = begin(session);
  assert.throws(() => gesture.advance({ sequence: 2, position: [4, 4] }), /order/u);
  assert.throws(() => gesture.advance({ sequence: 1, position: [NaN, 4] }));
  gesture.advance({ sequence: 1, position: [4, 4] });
  assert.throws(() => gesture.advance({ sequence: 1, position: [8, 8] }), /order/u);
  gesture.cancel(); gesture.cancel();
  assert.throws(() => gesture.sceneJSON(), /cancelled/u);
  assert.equal(session.state, before);
  const gestures = Array.from({ length: 8 }, (_, i) => begin(session, i + 20));
  assert.throws(() => begin(session, 40), /eight/u);
  gestures[0].cancel(); begin(session, 41);
  session.dispose();
  assert.throws(() => gestures[1].advance({ sequence: 1, position: [4, 4] }), /disposed/u);
  const replacement = engine.openEditableSession(project); t.after(() => replacement.dispose());
  begin(replacement).cancel();
  const pending = begin(replacement);
  engine.dispose();
  assert.throws(() => pending.sceneJSON(), /disposed/u);
});

test("actual WASM prepared point commits reject copied, foreign, stale and released handles", async (t) => {
  const { engine, project, session } = await fixture(t);
  const other = engine.openEditableSession(project); t.after(() => other.dispose());
  const { command } = terminal(session);
  const prepared = session.preparePointGestureCommit(command, { expected: session.token });
  const concurrent = session.preparePointGestureCommit(command, { expected: session.token });
  assert.throws(() => session.applyPointGestureCommit(structuredClone(prepared)), /foreign/u);
  assert.throws(() => other.applyPointGestureCommit(prepared), /foreign/u);
  assert.equal(session.applyPointGestureCommit(concurrent).status, "accepted");
  const accepted = session.state;
  assert.equal(session.applyPointGestureCommit(prepared).status, "rejected");
  assert.equal(session.state, accepted);
  session.releasePointGestureCommit(prepared);
  assert.throws(() => session.applyPointGestureCommit(prepared), /released/u);
  const otherPrepared = other.preparePointGestureCommit(command, { expected: other.token });
  other.dispose();
  assert.throws(() => other.applyPointGestureCommit(otherPrepared), /disposed/u);
});
