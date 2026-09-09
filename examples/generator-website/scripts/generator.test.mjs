// SPDX-License-Identifier: GPL-3.0-or-later
import assert from "node:assert/strict";
import test from "node:test";
import { pathToFileURL } from "node:url";
import { resolve } from "node:path";
import { buildExample, buildNodeGenerator, engineEntry, sdkEntry, folder, repository } from "./build.mjs";

test("custom generator source declares controls and dynamic topology using the public SDK", async () => {
  await buildExample();
  const { footprint } = await import(pathToFileURL(await buildNodeGenerator()));
  // Use the exact recorder from the same bundled module as its branded builder.
  const { readWorkspaceSnapshot, evaluateWorkspaceSnapshot } = await import(pathToFileURL(resolve(repository, "scripts/workspace-loader.mjs")));
  const defaults = footprint.parseInputs();
  assert.deepEqual(defaults, { columns: 3, rows: 2, clearance: 0.5, mounting: "magnets", rounded: true });
  assert.equal(footprint.inputs.clearance.unit, "mm");
  assert.throws(() => footprint({ columns: 4 }), /at most 3/);
  assert.throws(() => footprint({ rows: 1.5 }), /integer/);
  const first = await evaluateWorkspaceSnapshot(readWorkspaceSnapshot(folder));
  const bores = first.generated.declarations.filter((item) => item.family === "geometry.centerRadiusCircle");
  assert.equal(bores.length, 24);
  assert.equal(first.generated.declarations.filter((item) => item.family === "computed.fillet").length, 4);
  const small = await evaluateWorkspaceSnapshot(readWorkspaceSnapshot(folder, { inputs: { columns: 1, rows: 1, mounting: "none", rounded: false } }));
  assert.equal(small.generated.declarations.length, 3);
  assert.equal(small.generated.declarations.filter((item) => item.family === "computed.fillet").length, 0);
  const { createEngine } = await import(pathToFileURL(engineEntry));
  const engine = await createEngine();
  try {
    const accepted = await engine.evaluate({ definition: footprint, parameters: { columns: 1, rows: 1, mounting: "none", rounded: false } });
    assert.equal(accepted.status, "accepted", JSON.stringify(accepted));
    assert.equal(accepted.capabilities.reverse_geometry_edits, false);
    assert.equal(accepted.validation.hard_residuals_validated, true);
    const profiles = await engine.exportProfiles(accepted, { chordErrorMm: 0.08, output: "/outline" });
    assert.equal(profiles.regions.length, 1); assert.equal(profiles.regions[0].holes.length, 0);
    assert.deepEqual(bounds(profiles.regions[0].outer), [-20.75, -20.75, 20.75, 20.75]);
    const defaultResult = await engine.evaluateGenerated(first.generated);
    assert.equal(defaultResult.status, "accepted", JSON.stringify(defaultResult));
    const defaultProfiles = await engine.exportProfiles(defaultResult, { chordErrorMm: 0.08, output: "/outline" });
    assert.equal(defaultProfiles.regions.length, 1); assert.equal(defaultProfiles.regions[0].holes.length, 24);
    const [x0, y0, x1, y1] = bounds(defaultProfiles.regions[0].outer);
    assert.ok(Math.abs(x1 - x0 - 125.5) < 1e-6); assert.ok(Math.abs(y1 - y0 - 83.5) < 1e-6);
    assert.ok(defaultProfiles.regions[0].outer.length > 4, "computed corner arcs are sampled");
    for (const hole of defaultProfiles.regions[0].holes) {
      const [left, bottom, right, top] = bounds(hole);
      assert.ok(Math.abs(right - left - 6) < 0.08 && Math.abs(top - bottom - 6) < 0.08);
    }
    const largest = await engine.evaluate({ definition: footprint, parameters: { rows: 3, columns: 3, clearance: 1.5, mounting: "magnets", rounded: true } });
    assert.equal(largest.status, "accepted", JSON.stringify(largest));
    const largestProfiles = await engine.exportProfiles(largest, { chordErrorMm: 0.08, output: "/outline" });
    assert.equal(largestProfiles.regions[0].holes.length, 36, "the complete supported control range fits the sampling budget");
    const screw = await engine.evaluate({ definition: footprint, parameters: { columns: 1, rows: 1, mounting: "screws" } });
    assert.equal(screw.status, "accepted", JSON.stringify(screw));
    const screws = await engine.exportProfiles(screw, { chordErrorMm: 0.08, output: "/outline" });
    assert.equal(screws.regions[0].holes.length, 4);
    assert.ok(screws.regions[0].holes.every((hole) => {
      const [left, bottom, right, top] = bounds(hole);
      return Math.abs(right - left - 3) < 0.08 && Math.abs(top - bottom - 3) < 0.08;
    }));
  } finally { engine.dispose(); }
  assert.ok(sdkEntry.endsWith("index.js"));
});

function bounds(points) {
  return [Math.min(...points.map(([x]) => x)), Math.min(...points.map(([, y]) => y)),
    Math.max(...points.map(([x]) => x)), Math.max(...points.map(([, y]) => y))];
}

test("editable manifold is a complete source-only folder with three freshly compiled patches", async () => {
  const { readWorkspaceSnapshot, evaluateWorkspaceSnapshot } = await import(pathToFileURL(resolve(repository, "scripts/workspace-loader.mjs")));
  const input = readWorkspaceSnapshot(resolve(repository, "examples/file-workspace-manifold"));
  assert.equal(input.mode, "editable");
  assert.deepEqual(input.files.map((file) => file.path), ["geosolve.json", "patches/point-to-point-channel.patch.ts", "patches/silicone-groove.patch.ts", "patches/water-channel.patch.ts", "sketch.ts"]);
  const loaded = await evaluateWorkspaceSnapshot(input);
  assert.equal(Object.keys(loaded.artifacts).length, 3);
  assert.equal(Object.keys(loaded.customFiles).length, 3);
  assert.equal(loaded.compiled.artifact.parameters.length, 2);
  assert.equal(loaded.compiled.artifact.groups.length, 7);
});
