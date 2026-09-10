// SPDX-License-Identifier: GPL-3.0-or-later
import assert from "node:assert/strict";
import { test } from "node:test";
import { readFile } from "node:fs/promises";
import { createEngine } from "../dist/index.js";
import { applyManagedSketchMutation } from "../../geosolve-sketch-code/dist/src/managed.js";
const viewport = { screen_size: [800, 600], model_center: [0, 0], pixels_per_model_unit: 5 };
async function fixture(t, name = "authoring-radius-2") {
  const compiled = JSON.parse(await readFile(new URL(`../../../crates/geosolve-sketch-engine/tests/fixtures/${name}.json`, import.meta.url), "utf8"));
  const engine = await createEngine(); t.after(() => engine.dispose());
  const project = engine.compileProject({ project: "construction", compiled, customFiles: {}, artifacts: {}, lock: { format: "geosolve-lock-v1", modules: {} } });
  const session = engine.openEditableSession(project); t.after(() => session.dispose());
  return { engine, project, session };
}
function begin(session, tool = "segment", role = "profile") {
  return session.beginConstruction(tool, { expected: session.token, gestureId: 71, viewport, role });
}
function command(session, tool = "segment", inferred = false, role = "profile", regularized = false) {
  const before = session.state, prediction = begin(session, tool, role);
  const points = inferred ? [[0, 0], [20, 0.1]] : tool === "polyline" ? [[40, 40], [60, 40], [60, 60]]
    : tool === "center_radius_circle" ? [[40, 40], [50, 40]] : [[40, 40], [60, 50]];
  let sequence = 0, completed = false;
  for (const position of points) {
    for (const event of ["move", "click"]) {
      const frame = prediction.advance({ sequence: ++sequence, input: { event, position, suppressed: !inferred, regularized } });
      assert.equal(session.state, before);
      assert.equal(frame.diagnostic, null);
      completed = frame.completed;
    }
  }
  if (tool === "polyline") completed = prediction.advance({ sequence: ++sequence, input: { event: "complete" } }).completed;
  assert.ok(completed);
  assert.equal(typeof JSON.parse(prediction.sceneJSON()), "object");
  return prediction.finish();
}
function compile(prepared) {
  return { ticketDigest: prepared.request.ticket.ticketDigest,
    ...applyManagedSketchMutation(prepared.request.current, prepared.request.ticket.mutation) };
}
function independentPoints(result, expected) {
  const points = result.geometry.points.map(({ position }) => position);
  assert.equal(points.length, expected.length);
  for (const pair of expected) assert.ok(points.some((point) => point.every((value, i) => Number.isFinite(value) && Math.abs(value - pair[i]) < 1e-9)), `missing ${pair}`);
  assert.equal(result.validation.hard_residuals_validated, true);
  assert.equal(result.validation.all_active_features_current, true);
}

test("actual WASM predicts four native tools and server validates exact compiler/durable publication", async (t) => {
  const { engine, project, session: prediction } = await fixture(t);
  const serverEngine = await createEngine(); t.after(() => serverEngine.dispose());
  for (const [tool, inferred, expected] of [
    ["segment", false, [[0, 0], [40, 40], [60, 50]]],
    ["polyline", false, [[0, 0], [40, 40], [60, 40], [60, 60]]],
    ["center_radius_circle", false, [[0, 0], [40, 40]]],
    ["two_point_aligned_rectangle", false, [[0, 0], [40, 40], [60, 40], [60, 50], [40, 50]]],
    ["segment", true, [[0, 0], [20, 0]]],
  ]) {
    const server = serverEngine.openEditableSession(project);
    const before = server.state;
    const intent = command(prediction, tool, inferred);
    const prepared = server.prepareConstruction(intent, { expected: server.token });
    const receipt = compile(prepared);
    if (inferred) {
      assert.match(receipt.compiled.normalizedSource, /start: bore\.center/u);
      assert.match(receipt.compiled.normalizedSource, /constraint\.horizontal/u);
      assert.match(receipt.compiled.normalizedSource, /branchDirection: \[1, 0\]/u);
    }
    const candidate = server.resolveConstruction(prepared, receipt);
    assert.equal(server.state, before); independentPoints(candidate.result, expected);
    await assert.rejects(() => serverEngine.exportProfiles(candidate.result, { chordErrorMm: 0.1 }), /belongs/u);
    const cold = engine.openEditableSession(candidate.project, { design: candidate.design });
    assert.equal(cold.sourceDesignDigest(), candidate.source_design_digest);
    independentPoints(cold.accepted, expected);
    assert.equal(server.applyConstructionCommit(candidate).status, "accepted");
    assert.deepEqual(server.accepted.geometry, candidate.result.geometry);
    assert.deepEqual(server.pointGestureTargets(), cold.pointGestureTargets());
    assert.equal(server.token.revision, before.token.revision + 1);
    assert.throws(() => server.applyConstructionCommit(candidate), /consumed/u);
    assert.throws(() => server.prepareConstruction(intent, { expected: server.token }), /basis/u);
    cold.dispose(); server.dispose();
  }
});

test("actual WASM construction correction, regularization and role preserve ordinary drafting semantics", async (t) => {
  const { session } = await fixture(t);
  const before = session.state;
  const prediction = begin(session, "center_radius_circle");
  prediction.advance({ sequence: 1, input: { event: "click", position: [40, 40], suppressed: true, regularized: false } });
  const rejected = prediction.advance({ sequence: 2, input: { event: "click", position: [40, 40], suppressed: true, regularized: false } });
  assert.equal(rejected.completed, false); assert.match(rejected.diagnostic, /invalid/u); assert.equal(session.state, before);
  assert.equal(prediction.advance({ sequence: 3, input: { event: "click", position: [50, 40], suppressed: true, regularized: false } }).completed, true);
  prediction.cancel(); assert.equal(session.state, before);
  const path = begin(session, "polyline");
  path.advance({ sequence: 1, input: { event: "click", position: [40, 40], suppressed: true, regularized: false } });
  assert.ok(path.advance({ sequence: 2, input: { event: "click", position: [60, 40], suppressed: true, regularized: false } }).can_finish);
  assert.equal(path.advance({ sequence: 3, input: { event: "step_back" } }).can_finish, false);
  path.advance({ sequence: 4, input: { event: "click", position: [50, 40], suppressed: true, regularized: false } });
  assert.ok(path.advance({ sequence: 5, input: { event: "complete" } }).completed);
  const corrected = path.finish();
  const correctedReceipt = session.prepareConstruction(corrected, { expected: session.token });
  const correctedSource = compile(correctedReceipt).compiled.normalizedSource;
  assert.match(correctedSource, /position: \[50, 40\]/u); assert.doesNotMatch(correctedSource, /position: \[60, 40\]/u);
  session.releaseConstruction(correctedReceipt); assert.equal(session.state, before);
  const intent = command(session, "two_point_aligned_rectangle", false, "construction", true);
  const prepared = session.prepareConstruction(intent, { expected: session.token });
  const receipt = compile(prepared);
  assert.match(receipt.compiled.normalizedSource, /role: "construction"/u);
  assert.match(receipt.compiled.normalizedSource, /regularized: true/u);
  const candidate = session.resolveConstruction(prepared, receipt);
  const corners = candidate.result.geometry.points.map(({ position }) => position).filter(([x, y]) => x > 1 && y > 1);
  assert.equal(corners.length, 4);
  const width = Math.max(...corners.map(([x]) => x)) - Math.min(...corners.map(([x]) => x));
  const height = Math.max(...corners.map(([, y]) => y)) - Math.min(...corners.map(([, y]) => y));
  assert.ok(width > 0 && Math.abs(width - height) < 1e-9);
  assert.equal(session.applyConstructionCommit(candidate).status, "accepted");
});

test("actual WASM construction compiler/commit handles reject copied, foreign, stale and released ownership", async (t) => {
  const { engine, project, session } = await fixture(t);
  const other = engine.openEditableSession(project); t.after(() => other.dispose());
  const intent = command(session), before = session.state;
  const prepared = session.prepareConstruction(intent, { expected: session.token });
  const receipt = compile(prepared);
  assert.throws(() => session.resolveConstruction(structuredClone(prepared), receipt), /foreign/u);
  assert.throws(() => other.resolveConstruction(prepared, receipt), /foreign/u);
  assert.throws(() => session.resolveConstruction(prepared, { ...receipt, ticketDigest: "forged" }));
  assert.equal(session.state, before);
  const candidate = session.resolveConstruction(prepared, receipt);
  assert.throws(() => session.resolveConstruction(prepared, receipt), /consumed/u);
  const competing = session.prepareConstruction(intent, { expected: session.token });
  const next = session.resolveConstruction(competing, compile(competing));
  assert.equal(session.applyConstructionCommit(next).status, "accepted");
  const accepted = session.state;
  assert.equal(session.applyConstructionCommit(candidate).status, "rejected"); assert.equal(session.state, accepted);
  session.releaseConstruction(candidate);
  assert.throws(() => session.applyConstructionCommit(candidate), /released/u);
  const pending = begin(other); other.dispose();
  assert.throws(() => pending.sceneJSON(), /disposed/u);
});

test("actual WASM construction preserves existing computed fillets and advances source allocation monotonically", async (t) => {
  const { session } = await fixture(t, "point-gesture-computed");
  const before = session.accepted;
  assert.ok(before.geometry.computed_edges.length > 0);
  let priorHighWater = JSON.parse(session.exportProject()).managed.declaration_name_high_water ?? 0;
  const created = [];
  for (let i = 0; i < 2; ++i) {
    const prepared = session.prepareConstruction(command(session), { expected: session.token });
    const candidate = session.resolveConstruction(prepared, compile(prepared));
    assert.deepEqual(candidate.result.geometry.computed_edges, before.geometry.computed_edges);
    const project = JSON.parse(candidate.project), highWater = project.managed.declaration_name_high_water;
    assert.ok(highWater > priorHighWater); priorHighWater = highWater;
    for (const declaration of candidate.declarations) assert.ok(!created.includes(declaration));
    created.push(...candidate.declarations);
    assert.equal(session.applyConstructionCommit(candidate).status, "accepted");
    assert.ok(session.accepted.validation.all_active_features_current);
  }
});
