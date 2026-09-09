// SPDX-License-Identifier: GPL-3.0-or-later
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import { createEngine } from "../dist/index.js";

function area(points) {
  return points.reduce((total, a, index) => {
    const b = points[(index + 1) % points.length];
    return total + a[0] * b[1] - a[1] * b[0];
  }, 0) * 0.5;
}

test("dedicated release WASM exports all computed manifold faces with exact plate area partition", async () => {
  const artifact = JSON.parse(await readFile(new URL("../../../crates/geosolve-sketch-engine/tests/fixtures/manifold.json", import.meta.url), "utf8"));
  const engine = await createEngine();
  try {
    const accepted = await engine.evaluateGenerated(artifact);
    assert.equal(accepted.status, "accepted", JSON.stringify(accepted));
    assert.equal(accepted.capabilities.profile_export, true);
    assert.equal(accepted.validation.all_active_features_current, true);
    const exported = await engine.exportProfiles(accepted, { chordErrorMm: 0.02 });
    assert.equal(exported.regions.length, 18);
    assert.ok(exported.regions.every((region) => area(region.outer) > 0 && region.holes.every((hole) => area(hole) < 0)));
    const netArea = exported.regions.reduce((sum, region) => sum + area(region.outer) + region.holes.reduce((total, hole) => total + area(hole), 0), 0);
    assert.ok(Math.abs(netArea - 240 * 120) < 1e-6, `${netArea} mm2`);
    assert.equal(exported.regions.filter((region) => region.holes.length === 0).length, 13);
    assert.ok(exported.regions.every((region) => [region.outer, ...region.holes].every((loop) => loop.flat().every(Number.isFinite))));
    await assert.rejects(() => engine.exportProfiles(accepted, { chordErrorMm: 0.02, output: "/missing" }), /unknown output/);
    assert.equal(engine.lastAccepted, accepted);
  } finally { engine.dispose(); }
});
