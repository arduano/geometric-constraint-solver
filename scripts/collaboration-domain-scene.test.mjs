// SPDX-License-Identifier: GPL-3.0-or-later
import assert from "node:assert/strict";
import { test } from "node:test";
import { mkdtemp, rm, readFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { runCollaborationDomainJob as job } from "./collaboration-domain.mjs";
import { demoBindingsUrl, demoWasmPath } from "./workspace-runtime-paths.mjs";

test("domain scene returns matching actual workbench chrome and immutable interaction seed", async (t) => {
  const folder = await mkdtemp(join(tmpdir(), "geosolve-domain-scene-")); t.after(() => rm(folder, { recursive: true, force: true }));
  const compiled = JSON.parse(await readFile(new URL("../crates/geosolve-sketch-engine/tests/fixtures/point-gesture-constrained.json", import.meta.url)));
  const files = { "geosolve.json": JSON.stringify({ format: "geosolve-folder-v2", entry: "sketch.ts", mode: "editable" }), "sketch.ts": compiled.normalizedSource };
  const initial = await job({ kind: "initialize", folder, files });
  const result = await job({ kind: "scene", folder, files, model: initial.model, expectedInput: initial.acceptedInput, viewport: { width: 900, height: 700, pixelRatio: 1 } });
  assert.equal(result.acceptedInput, initial.acceptedInput); assert.equal(result.scene.snapshot.project.status, "accepted");
  assert.equal(result.scene.snapshot.source.dirty, false); assert.ok(result.scene.seed);
  assert.equal(result.scene.toolCatalog.version, 1);
  assert.ok(result.scene.toolCatalog.sections.some((section) => section.id === "sketch" && section.commands.length > 0));
  assert.deepEqual(result.pointTargets, initial.pointTargets);
  assert.ok(result.scene.snapshot.frame.scene);
  const wasm = await import(demoBindingsUrl); await wasm.default({ module_or_path: await readFile(demoWasmPath) });
  const first = new wasm.InteractionHandle(JSON.stringify(result.scene.seed)), second = new wasm.InteractionHandle(JSON.stringify(result.scene.seed));
  t.after(() => { first.free(); second.free(); });
  const installed = JSON.parse(first.replace(JSON.stringify({ seed: result.scene.seed, preserveSelection: false })));
  assert.equal(result.scene.seed.semanticPreview, false);
  // Local recomposition intentionally carries presentation-only provenance; its
  // drawing and document/revision still match the accepted workbench export.
  assert.deepEqual(installed.frame.scene.items, result.scene.snapshot.frame.scene.items);
  assert.deepEqual(installed.frame.scene.viewBox, result.scene.snapshot.frame.scene.viewBox);
  assert.equal(installed.frame.scene.provenance.scene, "accepted-presentation");
  assert.equal(installed.frame.scene.provenance.document, result.scene.snapshot.frame.scene.provenance.document);
  assert.equal(installed.frame.scene.provenance.revision, result.scene.snapshot.frame.scene.provenance.revision);
  const secondBefore = second.state();
  const zoomed = JSON.parse(first.wheel(JSON.stringify({ version: 2, x: 400, y: 300, deltaX: 0, deltaY: -80, ctrl: false })));
  assert.notDeepEqual(zoomed.state.viewport, JSON.parse(secondBefore).viewport);
  assert.equal(second.state(), secondBefore);
  // The real local Rust handle picks from detached geometry after reprojection.
  const center = zoomed.state.viewport.model_center, scale = zoomed.state.viewport.pixels_per_model_unit;
  const size = zoomed.state.viewport.screen_size;
  const target = initial.pointTargets[0].position;
  const x = size[0] / 2 + (target[0] - center[0]) * scale, y = size[1] / 2 - (target[1] - center[1]) * scale;
  for (const [phase, buttons] of [["down", 1], ["up", 0]]) first.pointer(JSON.stringify({ version: 2, phase, pointerId: 1, x, y, buttons, modifiers: { alt: false, ctrl: false, meta: false, shift: false } }));
  assert.ok(JSON.parse(first.state()).selection.length > 0); assert.equal(second.state(), secondBefore);
  assert.equal(result.acceptedInput, initial.acceptedInput);
  const moved = await job({ kind: "point_gesture", folder, files, model: initial.model, command: { basis: initial.model.sourceDesignDigest,
    gesture_id: 17, target: initial.pointTargets[0].target, viewport: { screen_size: [800, 600], model_center: [0, 0], pixels_per_model_unit: 10 }, samples: [{ sequence: 1, position: [6, 4] }] } });
  const refreshed = await job({ kind: "scene", folder, files, model: moved.model, expectedInput: moved.acceptedInput });
  assert.equal(refreshed.acceptedInput, moved.acceptedInput); assert.notEqual(refreshed.acceptedInput, result.acceptedInput);
  const presented = JSON.parse(refreshed.scene.seed.scene).points.map((point) => point.model_position);
  assert.ok(presented.some((point) => Math.hypot(point[0] - 6, point[1] - 4) < 1e-9));
  assert.ok(presented.some((point) => Math.hypot(point[0] - 26, point[1] - 4) < 1e-9));
  assert.deepEqual(JSON.parse(result.scene.seed.scene).points.map((point) => point.model_position), [[0, 0], [20, 0]]);
});
