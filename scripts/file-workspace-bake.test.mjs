// SPDX-License-Identifier: GPL-3.0-or-later
// Real disk -> managed compiler -> Rust/WASM accepted topology -> artifact tests.
import assert from "node:assert/strict";
import test from "node:test";
import { spawnSync } from "node:child_process";
import { cpSync, existsSync, mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { bakeProject, hash } from "./file-workspace.mjs";

const evidence = resolve("target/m98/bake");
mkdirSync(evidence, { recursive: true });
const working = mkdtempSync(resolve(evidence, "regression-"));
const fixtures = resolve("examples/file-workspace-bake");
const read = (path) => JSON.parse(readFileSync(path, "utf8"));
const area = (loop) => loop.reduce((sum, a, i) => {
  const b = loop[(i + 1) % loop.length];
  return sum + a[0] * b[1] - b[0] * a[1];
}, 0) / 2;
const width = (loop) => Math.max(...loop.map(([x]) => x)) - Math.min(...loop.map(([x]) => x));
const logs = [];
function bake(folder, output, error = "0.02", success = true) {
  const args = ["scripts/file-workspace.mjs", "bake", folder, "--out", output, "--chord-error-mm", error];
  const result = spawnSync(process.execPath, args, { encoding: "utf8", timeout: 60000 });
  logs.push({ args, status: result.status, stdout: result.stdout, stderr: result.stderr });
  writeFileSync(resolve(evidence, "regression-invocations.json"), JSON.stringify(logs, null, 2));
  assert.ifError(result.error);
  if (success) assert.equal(result.status, 0, result.stderr);
  else assert.notEqual(result.status, 0, result.stdout);
  return result;
}
function copy(name, from = "pi-footprint") {
  const folder = resolve(working, name);
  cpSync(resolve(fixtures, from), folder, { recursive: true });
  return folder;
}

test("real Pi production regions satisfy v1, hole pattern, winding, exact byte hash", () => {
  const folder = resolve(fixtures, "pi-footprint");
  const output = resolve(evidence, "pi-footprint.json");
  bake(folder, output);
  const baked = read(output);
  assert.equal(baked.format, "geosolve-baked-profile-v1");
  assert.equal(baked.units, "mm");
  assert.deepEqual(baked.plane, { origin: [0, 0, 0], x_axis: [1, 0, 0], y_axis: [0, 1, 0] });
  assert.equal(baked.sampling.max_chord_error_mm, 0.02);
  assert.equal(baked.source.sha256, hash(readFileSync(resolve(folder, "sketch.ts"))));
  assert.match(baked.source.sha256, /^[0-9a-f]{64}$/);
  assert.equal(baked.regions.length, 5);
  const board = baked.regions.find((region) => region.holes.length === 4);
  assert.ok(board, "production board face has four holes");
  assert.equal(board.outer.length, 4);
  assert.ok(Math.abs(width(board.outer) - 85) < 1e-8);
  assert.ok(Math.abs(area(board.outer) - 85 * 56) < 1e-8);
  const centers = board.holes.map((loop) => {
    const center = [0, 1].map((axis) => loop.reduce((sum, p) => sum + p[axis], 0) / loop.length);
    for (const p of loop) assert.ok(Math.abs(Math.hypot(p[0] - center[0], p[1] - center[1]) - 1.35) < 1e-8);
    return center.map((value) => Number(value.toFixed(6)));
  }).sort((a, b) => a[0] - b[0] || a[1] - b[1]);
  assert.deepEqual(centers, [[3.5, 3.5], [3.5, 52.5], [61.5, 3.5], [61.5, 52.5]]);
  for (const region of baked.regions) {
    assert.ok(area(region.outer) > 0);
    for (const hole of region.holes) assert.ok(area(hole) < 0);
    for (const loop of [region.outer, ...region.holes]) {
      assert.ok(loop.length >= 3);
      assert.notDeepEqual(loop[0], loop.at(-1));
      assert.ok(loop.flat().every(Number.isFinite));
    }
  }
});

test("accepted solved circle and circular arc are sampled with measured sagitta under target", () => {
  const output = resolve(evidence, "circle-arc.json");
  bake(resolve(fixtures, "circle-arc"), output);
  const baked = read(output);
  assert.equal(baked.regions.length, 2);
  const disk = baked.regions.find(({ outer }) => outer.every(([x]) => x < 20)).outer;
  const arc = baked.regions.find(({ outer }) => outer.some(([x]) => x > 20)).outer;
  assert.ok(disk.length > 16);
  let measuredSagitta = 0;
  for (const [loop, cx, radius] of [[disk, 0, 12], [arc, 30, 5]]) {
    for (let i = 0; i < loop.length; i++) {
      const a = loop[i], b = loop[(i + 1) % loop.length];
      assert.ok(Math.abs(Math.hypot(a[0] - cx, a[1]) - radius) < 1e-7, "accepted curve, not the radius-10 seed");
      if (cx === 30 && Math.hypot(b[0] - a[0], b[1] - a[1]) > 5) continue; // straight closing chord
      const sagitta = radius - Math.hypot((a[0] + b[0]) / 2 - cx, (a[1] + b[1]) / 2);
      measuredSagitta = Math.max(measuredSagitta, sagitta);
      assert.ok(sagitta <= 0.02 + 1e-10);
    }
  }
  assert.ok(arc.some(([x, y]) => x > 32 && x < 34 && y > 2 && y < 4), "interior arc samples exist");
  writeFileSync(resolve(evidence, "sampling-measurement.json"), JSON.stringify({ measuredMaxSagittaMm: measuredSagitta, requestedMaxChordErrorMm: 0.02, diskVertices: disk.length, arcVertices: arc.length }, null, 2));
});

test("repeat bake is deterministic and a private width edit changes accepted output and hash", () => {
  const folder = copy("pi-width-90");
  const first = resolve(working, "before.json");
  bake(folder, first);
  const previous = readFileSync(first);
  bake(folder, first);
  assert.deepEqual(readFileSync(first), previous);
  const source = resolve(folder, "sketch.ts");
  writeFileSync(source, readFileSync(source, "utf8").replace("width: mm(85)", "width: mm(90)"));
  const output = resolve(evidence, "pi-footprint-width-90.json");
  bake(folder, output);
  const before = JSON.parse(previous), after = read(output);
  assert.notEqual(before.source.sha256, after.source.sha256);
  assert.equal(after.source.sha256, hash(readFileSync(source)));
  assert.ok(Math.abs(width(after.regions.find((region) => region.holes.length === 4).outer) - 90) < 1e-8);
  writeFileSync(resolve(evidence, "dimension-change.json"), JSON.stringify({ source, beforeSha256: before.source.sha256, afterSha256: after.source.sha256, beforeWidthMm: 85, afterWidthMm: 90, output }, null, 2));
});

test("invalid current source ignores recovery and leaves existing output and source intact", () => {
  const folder = copy("invalid");
  const output = resolve(working, "last-valid.json");
  bake(folder, output);
  const previous = readFileSync(output);
  mkdirSync(resolve(folder, ".geosolve"));
  cpSync(resolve(folder, "sketch.ts"), resolve(folder, ".geosolve/last-good.ts"));
  const invalid = '"use geosolve sketch";\nexport default sketch(($) => { incomplete';
  writeFileSync(resolve(folder, "sketch.ts"), invalid);
  const result = bake(folder, output, "0.02", false);
  assert.match(result.stderr, /sketch.ts/);
  assert.deepEqual(readFileSync(output), previous);
  assert.equal(readFileSync(resolve(folder, "sketch.ts"), "utf8"), invalid);
});

test("disk change during cold compilation refuses publication of a mismatched snapshot", async () => {
  const folder = copy("concurrent");
  const output = resolve(working, "concurrent.json");
  writeFileSync(output, "previous artifact");
  const source = resolve(folder, "sketch.ts");
  // bake captures bytes synchronously, then the real runtime/compiler loads asynchronously.
  const pending = bakeProject(folder, output, 0.02);
  const newer = readFileSync(source, "utf8").replace("width: mm(85)", "width: mm(91)");
  writeFileSync(source, newer);
  await assert.rejects(pending, /current accepted disk source|disk changed during export/);
  assert.equal(readFileSync(output, "utf8"), "previous artifact");
  assert.equal(readFileSync(source, "utf8"), newer);
});

test("open and unsupported geometry refuse without creating an output", () => {
  const folder = copy("refusals");
  const output = resolve(working, "refused.json");
  for (const geometry of [
    '$.geometry.segment("open", { start: [0, 0], end: [10, 0] })',
    '$.geometry.centerAxesEllipse("ellipse", { center: [0, 0], majorAxisPoint: [10, 0], minorAxisPoint: [0, 5] })',
  ]) {
    writeFileSync(resolve(folder, "sketch.ts"), `"use geosolve sketch";\nimport { sketch } from "@geosolve/sketch-code";\nexport default sketch(($) => { const shape = ${geometry}; return {shape}; });\n`);
    const result = bake(folder, output, "0.02", false);
    assert.match(result.stderr, /production topology|unsupported curve family/);
    assert.equal(existsSync(output), false);
  }
});

test("invalid sampling targets and source output paths refuse; exact UTF-8 BOM hash is preserved", () => {
  const folder = copy("bom", "circle-arc");
  const output = resolve(working, "bom.json");
  for (const error of ["0", "-1", "NaN", "Infinity", "1e-30"]) {
    bake(folder, output, error, false);
    assert.equal(existsSync(output), false);
  }
  const source = resolve(folder, "sketch.ts");
  const before = readFileSync(source);
  bake(folder, source, "0.02", false);
  assert.deepEqual(readFileSync(source), before);
  writeFileSync(source, Buffer.concat([Buffer.from([0xef, 0xbb, 0xbf]), before]));
  bake(folder, output);
  assert.equal(read(output).source.sha256, hash(readFileSync(source)));
});
