// SPDX-License-Identifier: GPL-3.0-or-later
import assert from "node:assert/strict";
import test from "node:test";
import { WorkspaceEngineRuntime } from "../packages/geosolve-cli/dist/workspace-runtime.mjs";
import { pointCommand, polylineCommand, roleCommand } from "./workspace-native-test.mjs";

const source = radius => `"use geosolve sketch";
import { sketch, mm } from "@geosolve/sketch-code";
export default sketch(($) => {
  const bore = $.geometry.centerRadiusCircle("bore", { center: [0, 0], radius: mm(${radius}) });
  return { bore };
});`;
const apply = (runtime, contents) => runtime.dispatch({ command: "source.prepare", payload: { path: "sketch.ts", contents } });
async function fixture(t) { const runtime = new WorkspaceEngineRuntime(); t.after(() => runtime.dispose()); await runtime.construct(); return runtime; }
const positions = result => result.geometry.points.map(point => point.position);

test("folder engine retains complete Undo/Redo and unfinished source through the exact existing outer workspace", async t => {
  const runtime = await fixture(t);
  await apply(runtime, source(2));
  await apply(runtime, source(5));
  const five = runtime.snapshot();
  const fiveProject = runtime.exportProject().contents;
  await runtime.dispatch({ command: "history.undo" });
  const two = runtime.snapshot();
  assert.equal(two.history.canUndo, true);
  assert.equal(two.history.canRedo, true);
  const rejected = await apply(runtime, "// unfinished 😀\nnot valid managed source");
  assert.equal(rejected.status, "retained");
  assert.equal(rejected.source.dirty, true);
  assert.deepEqual(positions(rejected.result), positions(two.result));
  assert.throws(() => runtime.exportProject(), /Canonical export.*invalid/);
  const saved = runtime.persistProject().contents;
  assert.match(saved, /^\{"format":"geosolve-workbench-presentation-v1"/);
  const restored = await runtime.construct({ persistedProject: saved });
  assert.equal(restored.source.files[0].contents, rejected.source.files[0].contents);
  assert.equal(restored.history.canRedo, true);
  assert.equal(runtime.persistProject().contents, saved);
  await runtime.dispatch({ command: "source.revert" });
  await runtime.dispatch({ command: "history.redo" });
  assert.equal(runtime.exportProject().contents, fiveProject);
  assert.deepEqual(positions(runtime.snapshot().result), positions(five.result));
  assert.match(runtime.exportReproduction().contents, /GEOSOLVE_REPRO_V1/);
});

test("folder engine publishes native point, construction and operation terminals with isolated failed candidates", async t => {
  const runtime = await fixture(t);
  await apply(runtime, source(2));
  let model = runtime.snapshot();
  const point = await pointCommand(model, [3, -2]);
  await runtime.commit({ kind: "point", command: point });
  assert.notDeepEqual(positions(runtime.snapshot().result), positions(model.result));
  const moved = runtime.persistProject().contents;
  await assert.rejects(runtime.commit({ kind: "point", command: point }), /basis|stale|different|accepted/i);
  assert.equal(runtime.persistProject().contents, moved);
  const command = await polylineCommand(runtime.snapshot(), [[20, 0], [30, 10], [40, 0]]);
  await runtime.commit({ kind: "construction", command });
  assert.match(runtime.snapshot().source.files[0].contents, /geometry\.polyline/);
  const construction = runtime.persistProject().contents;
  await assert.rejects(runtime.commit({ kind: "construction", command: { ...command, expected_declarations: [] } }), /basis|stale|different|declaration/i);
  assert.equal(runtime.persistProject().contents, construction);
  const role = await roleCommand(runtime.snapshot());
  await runtime.commit({ kind: "operation", command: role });
  assert.match(runtime.snapshot().source.files[0].contents, /role: "construction"/);
  const authored = runtime.exportProject().contents;
  await runtime.dispatch({ command: "history.undo" });
  assert.doesNotMatch(runtime.snapshot().source.files[0].contents, /role: "construction"/);
  await runtime.dispatch({ command: "history.redo" });
  assert.equal(runtime.exportProject().contents, authored);
  assert.equal(Object.hasOwn(runtime.snapshot(), "frame"), false);
  assert.equal(runtime.snapshot().seed.sceneKey, runtime.snapshot().result.result_id);
});

test("folder engine refuses corrupt checkpoint installation without losing current source/history", async t => {
  const runtime = await fixture(t);
  await apply(runtime, source(2));
  await apply(runtime, source(4));
  const before = runtime.persistProject().contents;
  await assert.rejects(runtime.construct({ persistedProject: before.replace('"format":', '"format":"forged","format":') }), /restore/);
  assert.equal(runtime.persistProject().contents, before);
  await runtime.dispatch({ command: "history.undo" });
  const profile = await runtime.bakeProfile(0.02);
  assert.ok(profile.regions[0].outer.every(([x, y]) => Math.abs(Math.hypot(x, y) - 2) < 1e-9));
});

test("folder source metadata and parameter extraction use the shared compiler and one native history", async t => {
  const runtime = await fixture(t);
  await apply(runtime, source(2));
  const before = runtime.snapshot().result.geometry;
  await runtime.mutation({ mutation: { mutation: "set_metadata", target: { target: "document" }, property: "title", value: { kind: "string", value: "Folder part 😀" } } });
  assert.equal(runtime.snapshot().result.document.title, "Folder part 😀");
  assert.deepEqual(runtime.snapshot().result.geometry, before);
  await runtime.mutation({ mutation: { mutation: "extract_parameter", declaration: "bore", path: ["radius"], symbol: "radius", variable: "radius", presentation: { label: "Bore radius", isKeyParameter: true } } });
  const extracted = runtime.exportProject().contents;
  assert.match(runtime.snapshot().source.files[0].contents, /isKeyParameter: true/);
  assert.deepEqual(runtime.snapshot().result.geometry, before);
  await runtime.dispatch({ command: "history.undo" });
  assert.doesNotMatch(runtime.snapshot().source.files[0].contents, /isKeyParameter/);
  await runtime.dispatch({ command: "history.redo" });
  assert.equal(runtime.exportProject().contents, extracted);
});
