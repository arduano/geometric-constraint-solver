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

const recipeCases = [
  ["sketch_point", [[10, 10]], 1],
  ["segment", [[10, 10], [14, 12]], 2],
  ["polyline", [[10, 10], [14, 10], [12, 13]], 3],
  ["midpoint_line", [[10, 10], [14, 12]], 3],
  ["two_point_aligned_rectangle", [[10, 10], [14, 12]], 4],
  ["three_point_corner_rectangle", [[10, 10], [14, 10], [12, 13]], 4],
  ["center_rectangle", [[10, 10], [14, 12]], 5],
  ["three_point_center_rectangle", [[10, 10], [14, 10], [12, 13]], 5],
  ["center_radius_circle", [[10, 10], [14, 12]], 1],
  ["two_point_diameter_circle", [[10, 10], [14, 12]], 1],
  ["three_point_circle", [[10, 10], [14, 10], [12, 13]], 1],
  ["center_arc", [[10, 10], [14, 10], [12, 13]], 1],
  ["three_point_arc", [[14, 10], [10, 10], [12, 12]], 1],
  ["tangent_arc", [[2, 0], [3, 1]], 1],
  ["center_axes_ellipse", [[10, 10], [14, 10], [10, 12]], 2],
  ["axis_endpoints_ellipse", [[14, 10], [6, 10], [10, 12]], 2],
  ["center_axes_elliptical_arc", [[10, 10], [14, 10], [10, 12], [14, 10], [10, 12]], 2],
  ["axis_endpoints_elliptical_arc", [[14, 10], [6, 10], [10, 12], [14, 10], [10, 12]], 2],
  ["quadratic_bezier", [[10, 10], [14, 10], [12, 13]], 3],
  ["cubic_bezier", [[10, 10], [14, 10], [14, 14], [10, 14]], 4],
  ["rational_quadratic_conic", [[10, 10], [14, 10], [12, 13]], 2],
  ["parabola", [[10, 10], [14, 12]], 2],
  ["hyperbola", [[10, 10], [14, 12]], 2],
  ["open_control_nurbs", [[10, 10], [14, 10], [14, 14], [10, 14]], 4],
  ["periodic_control_nurbs", [[10, 10], [14, 10], [14, 14], [10, 14]], 4],
];
async function recipeFixture(t) {
  const compiled = JSON.parse(await readFile(new URL("../../geosolve-sketch-code/test/fixtures/managed-clean-segment.json", import.meta.url), "utf8"));
  const engine = await createEngine(); t.after(() => engine.dispose());
  const project = engine.compileProject({ project: "construction-parity", compiled, customFiles: {}, artifacts: {}, lock: { format: "geosolve-lock-v1", modules: {} } });
  return { engine, project };
}
function recipeCommand(session, tool, points, initial = []) {
  const prediction = session.beginConstruction(tool, { expected: session.token, gestureId: 71,
    viewport: { ...viewport, pixels_per_model_unit: 20 }, role: "profile" });
  assert.equal(prediction.initialFrame.sequence, 0);
  assert.equal(prediction.initialFrame.completed, false);
  assert.equal(prediction.initialFrame.nurbs_options.degree, 3);
  assert.equal(prediction.initialFrame.conic_options.arc_sweep, "counter_clockwise");
  let sequence = 0, completed = false;
  for (const input of initial) prediction.advance({ sequence: ++sequence, input });
  for (const position of points) {
    for (const event of ["move", "click"]) {
      const frame = prediction.advance({ sequence: ++sequence, input: { event, position, suppressed: true, regularized: false } });
      assert.equal(frame.diagnostic, null, `${tool}: ${frame.diagnostic}`);
      completed = frame.completed;
    }
  }
  if (tool === "polyline" || tool.endsWith("_nurbs")) completed = prediction.advance({ sequence: ++sequence, input: { event: "complete" } }).completed;
  assert.equal(completed, true, `${tool} has no terminal`);
  return prediction.finish();
}

test("actual WASM all 25 native recipes compile, publish, cold restore and undo as single source transactions", async (t) => {
  const { engine, project } = await recipeFixture(t);
  for (const [tool, points, addedPoints] of recipeCases) {
    await t.test(tool, async () => {
      const client = engine.openEditableSession(project), server = engine.openEditableSession(project);
      try {
        const before = server.state, intent = recipeCommand(client, tool, points);
        const prepared = server.prepareConstruction(intent, { expected: server.token });
        const receipt = compile(prepared), candidate = server.resolveConstruction(prepared, receipt);
        assert.equal(server.state, before);
        assert.equal(candidate.result.validation.hard_residuals_validated, true);
        assert.equal(candidate.result.validation.all_active_features_current, true);
        assert.equal(candidate.result.geometry.points.length, before.result.geometry.points.length + addedPoints, tool);
        assert.ok(candidate.result.geometry.points.every(({ position }) => position.every(Number.isFinite)));
        assert.ok(candidate.result.geometry.scalars.every(({ value }) => Number.isFinite(value)));
        for (const position of [[0, 0], [2, 0]]) assert.ok(candidate.result.geometry.points.some((point) => point.position.every((value, i) => Math.abs(value - position[i]) < 1e-9)));
        const cold = engine.openEditableSession(candidate.project, { design: candidate.design });
        try { assert.equal(cold.sourceDesignDigest(), candidate.source_design_digest); assert.deepEqual(sourceOwnedGeometry(cold.accepted.geometry), sourceOwnedGeometry(candidate.result.geometry)); }
        finally { cold.dispose(); }
        assert.equal(server.applyConstructionCommit(candidate).status, "accepted");
        assert.equal(server.token.revision, before.token.revision + 1);
        const created = server.accepted.geometry;
        assert.equal((await server.undo({ expected: server.token })).status, "accepted");
        assert.deepEqual(server.accepted.geometry, before.result.geometry);
        assert.equal((await server.redo({ expected: server.token })).status, "accepted");
        assert.deepEqual(server.accepted.geometry, created);
      } finally { client.dispose(); server.dispose(); }
    });
  }
});

test("actual WASM construction options are replayed and compiler validated rather than reset on publication", async (t) => {
  const { engine, project } = await recipeFixture(t);
  const conic = { minor_axis_ratio: 0.5, arc_start: 0, arc_end: Math.PI / 2, arc_sweep: "clockwise", middle_weight: 0.5, trim_start: -2, trim_end: 3, semi_conjugate: 2, hyperbola_branch: "negative" };
  for (const [tool, event, sourceFragment] of [
    ["rational_quadratic_conic", { event: "conic_options", options: conic }, /middleWeight: 0\.5/u],
    ["open_control_nurbs", { event: "nurbs_options", options: { form: "clamped", degree: 2, weights: [1, 2, 1, 0.5], gauge_index: 0 } }, /degree: 2/u],
    ["center_arc", { event: "conic_options", options: conic }, /clockwise/u],
  ]) {
    const session = engine.openEditableSession(project);
    try {
      const [, points] = recipeCases.find(([key]) => key === tool);
      const intent = recipeCommand(session, tool, points, [event]);
      const prepared = session.prepareConstruction(intent, { expected: session.token });
      const receipt = compile(prepared); assert.match(receipt.compiled.normalizedSource, sourceFragment);
      const candidate = session.resolveConstruction(prepared, receipt);
      assert.equal(candidate.result.validation.hard_residuals_validated, true);
      assert.equal(session.applyConstructionCommit(candidate).status, "accepted");
    } finally { session.dispose(); }
  }
});

function sourceOwnedGeometry(geometry) {
  const ids = new Map([...geometry.points.map(({id,label}) => [id,label]), ...geometry.scalars.map(({id,label}) => [id,label]), ...geometry.curves.map(({curve}) => [curve.id,curve.label])]);
  const normalize = (value) => typeof value === "string" ? ids.get(value) ?? value : Array.isArray(value) ? value.map(normalize)
    : value && typeof value === "object" ? Object.fromEntries(Object.entries(value).map(([key,entry]) => [key,normalize(entry)])) : value;
  const result = normalize(geometry);
  for (const key of ["points", "scalars", "curves"]) result[key].sort((a,b) => (key === "curves" ? a.curve.label : a.label).localeCompare(key === "curves" ? b.curve.label : b.label));
  return result;
}

test("actual WASM Tangent Arc from source start preserves explicit opposed contact through compiler replay", async (t) => {
  const { engine, project } = await recipeFixture(t);
  const session = engine.openEditableSession(project);
  try {
    const intent = recipeCommand(session, "tangent_arc", [[0, 0], [-1, 1]]);
    const prepared = session.prepareConstruction(intent, { expected: session.token });
    const receipt = compile(prepared);
    assert.match(receipt.compiled.normalizedSource, /orientation: "opposed"/u);
    assert.match(receipt.compiled.normalizedSource, /center:/u);
    const candidate = session.resolveConstruction(prepared, receipt);
    assert.equal(candidate.result.validation.hard_residuals_validated, true);
    assert.equal(session.applyConstructionCommit(candidate).status, "accepted");
  } finally { session.dispose(); }
});

test("actual WASM construction camera and Reset preserve native snap tolerance, options and source replay", async (t) => {
  const { engine, project } = await recipeFixture(t);
  const session = engine.openEditableSession(project); t.after(() => session.dispose());
  const origin = { ...viewport, pixels_per_model_unit: 20 };
  const prediction = session.beginConstruction("segment", { expected: session.token, gestureId: 71, viewport: origin, role: "profile" });
  assert.equal(prediction.initialFrame.has_pending, false);
  assert.equal(prediction.initialFrame.can_reset, false);
  const pointer = (event, position) => ({ event, position, suppressed: false, regularized: false });
  assert.deepEqual(prediction.advance({ sequence: 1, input: pointer("move", [0.1, 0.1]) }).adjusted_position, [0, 0]);
  const zoomed = { screen_size: [1024, 768], model_center: [1, 0.5], pixels_per_model_unit: 200 };
  const zoom = prediction.advance({ sequence: 2, input: { event: "viewport", viewport: zoomed } });
  assert.equal(zoom.adjusted_position, null);
  assert.deepEqual(zoom.inference_guides, []);
  const pending = prediction.advance({ sequence: 3, input: pointer("click", [0.1, 0.1]) });
  assert.equal(pending.has_pending, true); assert.equal(pending.can_step_back, true);
  const options = { ...pending.conic_options, middle_weight: 1.75 };
  prediction.advance({ sequence: 4, input: { event: "conic_options", options } });
  const reset = prediction.advance({ sequence: 5, input: { event: "reset" } });
  assert.equal(reset.has_pending, false); assert.equal(reset.can_reset, false);
  assert.equal(reset.preview, null); assert.deepEqual(reset.conic_options, options);
  const empty = prediction.advance({ sequence: 6, input: { event: "reset" } });
  assert.equal(empty.has_pending, false);
  assert.equal(prediction.advance({ sequence: 7, input: { event: "viewport", viewport: zoomed } }).preview, null);
  prediction.advance({ sequence: 8, input: pointer("click", [0.1, 0.1]) });
  assert.equal(prediction.advance({ sequence: 9, input: pointer("click", [10, 11]) }).completed, true);
  const command = prediction.finish(); assert.deepEqual(command.viewport, origin);
  const prepared = session.prepareConstruction(command, { expected: session.token });
  const candidate = session.resolveConstruction(prepared, compile(prepared));
  assert.ok(candidate.result.geometry.points.some(({ position }) => position.every(value => Math.abs(value - 0.1) < 1e-12)));
  assert.equal(session.applyConstructionCommit(candidate).status, "accepted");
});
