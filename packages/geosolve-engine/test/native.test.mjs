// SPDX-License-Identifier: GPL-3.0-or-later
import assert from "node:assert/strict";
import test from "node:test";
import { createEngine } from "../dist/index.js";
import { defineGenerator, sketch, mm } from "@geosolve/sketch-code";
import { compileManagedSource } from "../../geosolve-sketch-code/dist/src/managed.js";

const definition = defineGenerator({ count: { type: "integer", default: 2, min: 1, max: 5 }, radius: { type: "number", default: 12, unit: "mm", min: 1, max: 15 } }, ({ count, radius }) => sketch(($) => {
  const holes = [];
  for (let i = 0; i < count; i++) holes.push($.geometry.centerRadiusCircle(`hole-${i}`, { center: [i * 40, 0], radius: mm(radius) }));
  return { holes };
}));

test("dedicated Node WASM evaluates ordinary loops, accepts independent geometry and retains last good output", async () => {
  const engine = await createEngine();
  try {
    const result = await engine.evaluate({ definition, parameters: {} });
    assert.equal(result.status, "accepted", JSON.stringify(result));
    assert.equal(result.mode, "generator");
    assert.equal(result.capabilities.reverse_geometry_edits, false);
    assert.equal(result.validation.hard_residuals_validated, true);
    const exported = await engine.exportProfiles(result, { chordErrorMm: 0.02 });
    assert.equal(exported.regions.length, 2);
    for (const region of exported.regions) {
      const xs = region.outer.map(([x]) => x);
      const center = (Math.min(...xs) + Math.max(...xs)) / 2;
      assert.ok(Math.min(Math.abs(center), Math.abs(center - 40)) < 1e-9);
      assert.ok(region.outer.every(([x, y]) => Math.abs(Math.hypot(x - center, y) - 12) < 1e-9));
    }
    assert.equal((await engine.evaluate({ definition, parameters: { count: 1.5 } })).status, "rejected");
    assert.equal(engine.lastAccepted, result);
    const reduced = await engine.evaluate({ definition, parameters: { count: 1, radius: 8 } });
    assert.equal(reduced.status, "accepted");
    const profile = await engine.exportProfiles(reduced, { chordErrorMm: 0.01 });
    assert.equal(profile.regions.length, 1);
    assert.ok(profile.regions[0].outer.every(([x, y]) => Math.abs(Math.hypot(x, y) - 8) < 1e-9));
    assert.equal((await engine.exportProfiles(result, { chordErrorMm: 0.02 })).regions.length, 2);
  } finally { engine.dispose(); }
});

test("managed compiler admission shares accepted circle geometry without acquiring generator source authority", async () => {
  const engine = await createEngine();
  try {
    const compiled = compileManagedSource(`"use geosolve sketch";
import { sketch, mm } from "@geosolve/sketch-code";
export default sketch(($) => {
  const ring = $.geometry.centerRadiusCircle("ring", { center: [0, 0], radius: mm(12) });
  return { ring };
});`);
    const project = engine.compileProject({ project: "engine-test", compiled, customFiles: {}, artifacts: {}, lock: { format: "geosolve-lock-v1", modules: {} } });
    const result = await engine.evaluateEditable(project);
    assert.equal(result.status, "accepted", JSON.stringify(result));
    assert.equal(result.mode, "editable");
    const profile = await engine.exportProfiles(result, { chordErrorMm: 0.01 });
    assert.equal(profile.regions.length, 1);
    assert.ok(profile.regions[0].outer.every(([x, y]) => Math.abs(Math.hypot(x, y) - 12) < 1e-9));
    const forged = JSON.parse(project); forged.managed.source += "forged";
    assert.equal((await engine.evaluateEditable(forged)).status, "rejected");
    assert.equal(engine.lastAccepted, result);
  } finally { engine.dispose(); }
});
