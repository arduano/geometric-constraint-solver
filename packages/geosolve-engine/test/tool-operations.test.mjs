// SPDX-License-Identifier: GPL-3.0-or-later
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import { createEngine } from "../dist/index.js";
import { applyManagedSketchMutation } from "../../geosolve-sketch-code/dist/src/managed.js";

const viewport = { screen_size: [800, 600], model_center: [0, 0], pixels_per_model_unit: 5 };
const ordinaryOptions = { tangent_orientation: "aligned", curvature_relation: "signed", continuity: { kind: "g1" }, dimension_mode: "driving", angle_orientation: "counter_clockwise" };
const tools = ["lock", "coincident", "horizontal", "vertical", "concentric", "collinear", "parallel", "perpendicular", "equal", "midpoint", "symmetric", "tangent", "continuity", "point_distance", "segment_length", "radius", "diameter", "oriented_angle", "fillet", "offset", "toggle_geometry_role"];
const dimensions = new Set(["point_distance", "segment_length", "radius", "diameter", "oriented_angle"]);

async function project(engine, name) {
  const compiled = JSON.parse(await readFile(new URL(`../../../crates/geosolve-sketch-engine/tests/fixtures/${name}.json`, import.meta.url), "utf8"));
  return engine.compileProject({ project: "tool-operations", compiled, customFiles: {}, artifacts: {}, lock: { format: "geosolve-lock-v1", modules: {} } });
}
function begin(session, tool, selection = [], options = {}) {
  return session.beginToolOperation(tool, { expected: session.token, gestureId: 71, viewport, selection, options });
}
function compile(prepared) {
  return { ticketDigest: prepared.request.ticket.ticketDigest,
    ...applyManagedSketchMutation(prepared.request.current, prepared.request.ticket.mutation) };
}
function geometry(session, label, curve = false) {
  const result = session.accepted;
  const output = Object.entries(result.named_outputs).find(([key, value]) =>
    value.reference.declaration === label && (!curve || result.named_geometry[key].spans.length > 0));
  assert.ok(output, `missing named ${curve ? "curve" : "point"} ${label}`);
  return result.named_geometry[output[0]];
}
function operand(session, item, parameter = null) {
  const curve_picks = item.Curve ? [{ span: item.Curve, parameter, origin: null }] : [];
  const result = session.toolOperationOperands({ items: [item], curve_picks });
  assert.equal(result.length, 1);
  return result[0];
}
function familyOperands(session, tool) {
  const curve = (name, parameter = 0.5) => operand(session, { Curve: geometry(session, name, true).spans[0] }, parameter);
  const point = (name) => operand(session, { Point: geometry(session, name).points[0] });
  switch (tool) {
    case "lock": return [point("left")];
    case "coincident": return [point("left"), point("coincident")];
    case "horizontal": return [curve("hline")];
    case "vertical": return [curve("vline")];
    case "concentric": return [curve("circle", 0), curve("concentric", 0)];
    case "collinear": return [curve("hline"), curve("collinear")];
    case "parallel": case "equal": return [curve("hline"), curve("parallel")];
    case "perpendicular": case "oriented_angle": return [curve("hline"), curve("vline")];
    case "midpoint": return [point("midpoint"), curve("hline")];
    case "symmetric": return [point("left"), point("right"), curve("axis")];
    case "tangent": return [curve("tangent"), curve("circle", Math.PI / 2)];
    case "continuity": return [curve("incoming", 1), curve("outgoing", 0)];
    case "point_distance": return [point("left"), point("right")];
    case "segment_length": return [curve("hline")];
    case "radius": case "diameter": return [curve("circle", 0)];
    case "offset": return [curve("corner")];
    case "fillet": {
      const span = geometry(session, "corner", true).spans[0];
      const definition = session.accepted.geometry.curves.find(({ curve }) => curve.id === span.curve).curve.definition;
      assert.equal(definition.kind, "polyline");
      return [operand(session, { Point: definition.points[1] })];
    }
    case "toggle_geometry_role": {
      const { curve } = session.accepted.geometry.curves.find(({ curve }) => curve.definition.kind === "circle");
      return [operand(session, { Curve: { curve: curve.id, segment: 0 } }, 0)];
    }
    default: throw Error(`uncovered native tool ${tool}`);
  }
}
function command(session, tool) {
  const before = session.state, operands = familyOperands(session, tool);
  const prediction = begin(session, tool, tool === "toggle_geometry_role" ? operands : []);
  try {
    let frame = prediction.initialFrame, sequence = 0;
    const event = (input) => {
      frame = prediction.advance({ sequence: ++sequence, input });
      assert.equal(frame.diagnostic, null, `${tool}: ${frame.diagnostic}`);
      assert.equal(session.state, before);
    };
    if (dimensions.has(tool)) event({ event: "authoring_options", options: { ...ordinaryOptions, dimension_mode: "reference" } });
    if (tool === "tangent") event({ event: "authoring_options", options: { ...ordinaryOptions, tangent_orientation: "opposed" } });
    if (tool !== "toggle_geometry_role") for (const operand of operands) event({ event: "pick", operand });
    if (tool === "fillet") event({ event: "fillet_radius", radius: 2 });
    if (tool === "offset") event({ event: "offset_distance", distance: 2 });
    if (!frame.completed) event({ event: "complete" });
    assert.equal(frame.completed, true, tool);
    const presentation = JSON.parse(prediction.presentationJSON());
    assert.ok(presentation.bindings);
    assert.equal(JSON.parse(presentation.scene).format, "geosolve-detached-scene-v1");
    return prediction.finish();
  } finally { prediction.cancel(); }
}
function validated(result) {
  assert.equal(result.validation.hard_residuals_validated, true);
  assert.equal(result.validation.all_active_features_current, true);
  const residual = result.validation.maximum_normalized_hard_residual;
  assert.ok(residual == null || Number.isFinite(residual) && residual <= 1e-9, `hard residual ${residual}`);
  assert.ok(result.geometry.points.every(({ position }) => position.every(Number.isFinite)));
  assert.ok(result.geometry.scalars.every(({ value }) => Number.isFinite(value)));
}
function sourceOwnedGeometry(geometry) {
  const ids = new Map([...geometry.points.map(({ id, label }) => [id, label]), ...geometry.scalars.map(({ id, label }) => [id, label]), ...geometry.curves.map(({ curve }) => [curve.id, curve.label])]);
  const normalize = (value) => typeof value === "string" ? ids.get(value) ?? value : Array.isArray(value) ? value.map(normalize)
    : value && typeof value === "object" ? Object.fromEntries(Object.entries(value).map(([key, entry]) => [key, normalize(entry)])) : value;
  const result = normalize(geometry);
  for (const key of ["points", "scalars", "curves"]) result[key].sort((a, b) => (key === "curves" ? a.curve.label : a.label).localeCompare(key === "curves" ? b.curve.label : b.label));
  return result;
}

test("actual WASM all 21 native operations compile, publish, cold restore and undo as single source transactions", async (t) => {
  const engine = await createEngine(); t.after(() => engine.dispose());
  const basis = await project(engine, "tool-operation-basis"), feature = await project(engine, "tool-operation-feature"), role = await project(engine, "authoring-radius-2");
  for (const tool of tools) await t.test(tool, async () => {
    const source = tool === "toggle_geometry_role" ? role : tool === "fillet" || tool === "offset" ? feature : basis;
    const client = engine.openEditableSession(source), server = engine.openEditableSession(source);
    try {
      const before = server.state, intent = command(client, tool);
      const prepared = server.prepareToolOperation(intent, { expected: server.token });
      const receipt = compile(prepared), candidate = server.resolveToolOperation(prepared, receipt);
      assert.equal(server.state, before);
      validated(candidate.result);
      if (dimensions.has(tool)) assert.match(receipt.compiled.normalizedSource, /isKeyConstraint: true/u);
      if (tool === "tangent") assert.match(receipt.compiled.normalizedSource, /opposed/u);
      if (tool === "toggle_geometry_role") assert.match(receipt.compiled.normalizedSource, /role: "construction"/u);
      const cold = engine.openEditableSession(candidate.project, { design: candidate.design });
      try {
        assert.equal(cold.sourceDesignDigest(), candidate.source_design_digest);
        validated(cold.accepted);
        assert.deepEqual(sourceOwnedGeometry(cold.accepted.geometry), sourceOwnedGeometry(candidate.result.geometry));
      } finally { cold.dispose(); }
      assert.equal(server.applyToolOperationCommit(candidate).status, "accepted");
      assert.equal(server.token.revision, before.token.revision + 1);
      const created = server.accepted.geometry;
      assert.deepEqual(created, candidate.result.geometry);
      assert.equal((await server.undo({ expected: server.token })).status, "accepted");
      assert.deepEqual(server.accepted.geometry, before.result.geometry);
      assert.equal((await server.redo({ expected: server.token })).status, "accepted");
      assert.deepEqual(server.accepted.geometry, created);
    } finally { client.dispose(); server.dispose(); }
  });
});

test("actual WASM empty Clear picks and Step back retain native tools and remembered options", async (t) => {
  const engine = await createEngine(); t.after(() => engine.dispose());
  const basis = await project(engine, "tool-operation-basis"), feature = await project(engine, "tool-operation-feature");
  for (const [tool, position] of [["radius", [105, 100]], ["point_distance", [10, 30]], ["fillet", [210, 0]], ["offset", [210, 0]]]) {
    const session = engine.openEditableSession(tool === "fillet" || tool === "offset" ? feature : basis);
    try {
      const before = session.state;
      for (const event of ["reset", "step_back"]) {
        const options = dimensions.has(tool) ? { authoring_options: { ...ordinaryOptions, dimension_mode: "reference" } } : {};
        const prediction = begin(session, tool, [], options);
        try {
          const initial = prediction.initialFrame;
          assert.equal(initial.has_pending, false);
          const cleared = prediction.advance({ sequence: 1, input: { event } });
          assert.deepEqual(cleared.authoring_options, initial.authoring_options);
          assert.deepEqual(cleared.fillet_options, initial.fillet_options);
          assert.equal(cleared.offset_distance, initial.offset_distance);
          const picked = prediction.advance({ sequence: 2, input: { event: "click", position } });
          assert.ok(picked.completed || picked.has_pending, `${tool} lost active mode after ${event}`);
          assert.equal(session.state, before);
        } finally { prediction.cancel(); }
      }
    } finally { session.dispose(); }
  }
});

test("actual WASM refused relation preserves the draft and the next valid pick compiles and publishes", async (t) => {
  const engine = await createEngine(); t.after(() => engine.dispose());
  const source = await project(engine, "tool-operation-rejection");
  const client = engine.openEditableSession(source), server = engine.openEditableSession(source);
  t.after(() => { client.dispose(); server.dispose(); });
  const before = client.state, prediction = begin(client, "horizontal"), scene = prediction.sceneJSON();
  try {
    const refused = prediction.advance({ sequence: 1, input: { event: "click", position: [10, 5] } });
    assert.equal(refused.completed, false); assert.ok(refused.diagnostic);
    assert.deepEqual(refused.pending, []); assert.equal(refused.has_pending, false);
    assert.equal(prediction.sceneJSON(), scene); assert.equal(client.state, before);
    const retried = prediction.advance({ sequence: 2, input: { event: "click", position: [10, 30] } });
    assert.equal(retried.completed, true); assert.equal(retried.diagnostic, null);
    const intent = prediction.finish(); assert.equal(intent.expected_declarations.length, 1);
    const prepared = server.prepareToolOperation(intent, { expected: server.token });
    const receipt = compile(prepared);
    assert.match(receipt.compiled.normalizedSource, /constraint\.horizontal/u);
    const candidate = server.resolveToolOperation(prepared, receipt);
    validated(candidate.result);
    assert.deepEqual(candidate.result.geometry, before.result.geometry);
    assert.equal(server.applyToolOperationCommit(candidate).status, "accepted");
    assert.equal(client.state, before);
    assert.equal((await server.undo({ expected: server.token })).status, "accepted");
    assert.deepEqual(server.accepted.geometry, before.result.geometry);
  } finally { prediction.cancel(); }
});

test("actual WASM repeated selection batches share the bounded trace and preserve retry after rejection", async (t) => {
  const engine = await createEngine(); t.after(() => engine.dispose());
  const session = engine.openEditableSession(await project(engine, "authoring-radius-2")); t.after(() => session.dispose());
  const before = session.state, prediction = begin(session, "parallel");
  try {
    const operands = Array.from({ length: 256 }, () => ({ target: "datum", datum: "x_axis" }));
    let retainedBytes = 0, rejected = false;
    for (let sequence = 1; sequence <= 140; ++sequence) {
      const sample = { sequence, input: { event: "pick_selection", operands } }, bytes = Buffer.byteLength(JSON.stringify(sample));
      const presentation = prediction.presentationJSON();
      try { prediction.advance(sample); retainedBytes += bytes; }
      catch (error) {
        assert.match(String(error), /trace byte limit/u);
        assert.ok(retainedBytes <= 1024 * 1024 && retainedBytes + bytes > 1024 * 1024);
        assert.equal(prediction.presentationJSON(), presentation);
        assert.equal(prediction.advance({ sequence, input: { event: "reset" } }).sequence, sequence);
        rejected = true; break;
      }
    }
    assert.equal(rejected, true); assert.equal(session.state, before);
  } finally { prediction.cancel(); }
});
