// SPDX-License-Identifier: GPL-3.0-or-later
// Actual compiler + Rust/WASM engine worker tests; no mocked native acceptance.
import assert from "node:assert/strict";
import { test } from "node:test";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { runCollaborationDomainJob as job } from "../packages/geosolve-cli/runtime/collaboration-domain.mjs";
import { createEngine } from "../packages/geosolve-engine/dist/index.js";

const manifest = JSON.stringify({ format: "geosolve-folder-v2", entry: "sketch.ts", mode: "editable" });
const source = '"use geosolve sketch";\nimport { sketch, mm } from "@geosolve/sketch-code";\n// Keep this authored layout.\nexport default sketch(($) => {\n  const bore = $.geometry.centerRadiusCircle("bore", { center: [0, 0], radius: mm(2) });\n  const marker = $.geometry.sketchPoint("marker", { point: [10, 20] });\n  return { bore, marker };\n});\n';
const files = () => ({ "geosolve.json": manifest, "sketch.ts": source, "notes.txt": "Unconsumed bytes also belong to the accepted source tree.\n" });
const radius = (value) => ({ declaration: "bore", path: ["radius"], value: { kind: "unit", value: { unit: "mm", value } } });
const point = (x, y) => ({ declaration: "marker", path: ["point"], value: { kind: "array", value: [{ kind: "number", value: x }, { kind: "number", value: y }] } });
async function fixture(t) {
  const folder = await mkdtemp(join(tmpdir(), "geosolve-collaboration-domain-"));
  t.after(() => rm(folder, { force: true, recursive: true }));
  return folder;
}
function checkAccepted(result) {
  assert.equal(result.status, "accepted"); assert.equal(result.validation.hard_residuals_validated, true); assert.equal(result.validation.all_active_features_current, true);
  const residual = result.validation.maximum_normalized_hard_residual;
  assert.ok(Number.isFinite(residual) && residual <= 1e-9);
  assert.ok(result.geometry.points.every((entry) => entry.position.every(Number.isFinite)));
  assert.ok(result.geometry.scalars.every((entry) => Number.isFinite(entry.value)));
}
function circleRadius(result) {
  const definition = result.geometry.curves.find((entry) => entry.curve.definition.kind === "circle")?.curve.definition;
  assert.ok(definition, JSON.stringify(result.geometry.curves));
  return result.geometry.scalars.find((entry) => JSON.stringify(entry.id) === JSON.stringify(definition.radius))?.value;
}

test("domain initialize and independent cold rebuild retain exact raw source, native identity and geometry", async (t) => {
  const folder = await fixture(t), raw = files();
  const initial = await job({ kind: "initialize", folder, files: raw });
  checkAccepted(initial.result); assert.equal(circleRadius(initial.result), 2);
  assert.deepEqual(initial.candidateFiles, raw);
  const rebuilt = await job({ kind: "rebuild", folder, files: raw, model: initial.model, expectedInput: initial.acceptedInput });
  assert.equal(rebuilt.acceptedInput, initial.acceptedInput); assert.deepEqual(rebuilt.model, initial.model); checkAccepted(rebuilt.result);
  assert.equal(initial.inventory.objects.find((entry) => entry.declaration === "bore").object, "sketch.ts#bore");
  const unrelated = await job({ kind: "initialize", folder, files: { ...raw, "notes.txt": "Changed unused text" } });
  assert.notEqual(unrelated.acceptedInput, initial.acceptedInput); assert.equal(unrelated.model.sourceDesignDigest, initial.model.sourceDesignDigest);
});

test("domain values preserve authored text and exact native previous/new semantic values", async (t) => {
  const folder = await fixture(t), raw = files(); const initial = await job({ kind: "initialize", folder, files: raw });
  const result = await job({ kind: "values", folder, files: raw, model: initial.model, expectedInput: initial.acceptedInput, writes: [radius(5)] });
  checkAccepted(result.result); assert.equal(circleRadius(result.result), 5);
  assert.equal(result.candidateFiles["sketch.ts"], source.replace("radius: mm(2)", "radius: mm(5)"));
  assert.equal(result.patches[0].patch.edits.length, 1); assert.equal(result.patches[0].patch.edits[0].expected, "2");
  assert.equal(result.valueChanges[0].before.value.value, 2); assert.deepEqual(result.valueChanges[0].write, radius(5));
  const rebuilt = await job({ kind: "rebuild", folder, files: result.candidateFiles, model: result.model });
  assert.equal(rebuilt.acceptedInput, result.acceptedInput); assert.equal(circleRadius(rebuilt.result), 5);
  assert.deepEqual(raw, files()); assert.equal(circleRadius(initial.result), 2);
});

test("domain rebuild refuses raw source, dependency, model and stale input corruption", async (t) => {
  const folder = await fixture(t), raw = files(); const initial = await job({ kind: "initialize", folder, files: raw });
  await assert.rejects(job({ kind: "rebuild", folder, files: { ...raw, "sketch.ts": source + "// corrupt\n" }, model: initial.model, expectedInput: initial.acceptedInput }), (error) => error.code === "stale_input");
  await assert.rejects(job({ kind: "values", folder, files: raw, model: initial.model, expectedInput: "stale", writes: [radius(5)] }), (error) => error.code === "stale_input");
  const wrong = structuredClone(initial.model); wrong.sourceDesignDigest = "f".repeat(64);
  await assert.rejects(job({ kind: "rebuild", folder, files: raw, model: wrong }), /identity differs/u);
  const corrupt = structuredClone(initial.model), project = JSON.parse(corrupt.project); project.managed.source += "//changed"; corrupt.project = JSON.stringify(project);
  await assert.rejects(job({ kind: "rebuild", folder, files: raw, model: corrupt }), /differs from independently/u);
  await writeFile(join(folder, "sketch.ts"), source);
  await assert.rejects(job({ kind: "initialize", folder, files: { "geosolve.json": manifest } }), /absent from the captured source tree/u);
});

test("immutable Apply rebases disjoint accepted values and draft reconciliation retains later invalid typing", async (t) => {
  const folder = await fixture(t), raw = files(), capture = { acceptedBasis: { files: raw }, working: { files: { ...raw, "sketch.ts": source.replace("radius: mm(2)", "radius: mm(7)") } } };
  const initial = await job({ kind: "initialize", folder, files: raw });
  const latest = await job({ kind: "values", folder, files: raw, model: initial.model, writes: [point(30, 40)] });
  const applied = await job({ kind: "apply", folder, files: latest.candidateFiles, model: latest.model, capture });
  assert.equal(circleRadius(applied.result), 7); checkAccepted(applied.result);
  assert.match(applied.candidateFiles["sketch.ts"], /point: \[30, 40\]/u);
  assert.deepEqual(applied.requiredStableDeclarations, ["bore"]);
  const live = { ...latest.candidateFiles, "sketch.ts": latest.candidateFiles["sketch.ts"] + "const incomplete = (\n" };
  const reconciled = await job({ kind: "reconcile", preparedContribution: latest.preparedContribution, workingFiles: { ...live, "sketch.ts": source + "const incomplete = (\n" } });
  assert.equal(reconciled.reconciliations[0].kind, "reconciled");
  assert.equal(reconciled.workingFiles["sketch.ts"], live["sketch.ts"]);
  const applyDraft = await job({ kind: "reconcile", preparedContribution: applied.preparedContribution, workingFiles: live });
  assert.equal(applyDraft.reconciliations[0].kind, "pending"); assert.deepEqual(applyDraft.workingFiles, live);
  const rebuilt = await job({ kind: "rebuild", folder, files: applied.candidateFiles, model: applied.model });
  assert.equal(rebuilt.acceptedInput, applied.acceptedInput);
  const invalid = structuredClone(capture); invalid.working.files["sketch.ts"] += "const invalid = (";
  await assert.rejects(job({ kind: "apply", folder, files: latest.candidateFiles, model: latest.model, capture: invalid }));
});

test("domain cancellation and timeout keep the main event loop responsive during trusted compilation", async (t) => {
  const folder = await fixture(t);
  const raw = { "geosolve.json": manifest,
    "sketch.ts": '"use geosolve sketch";import {sketch,mm} from "@geosolve/sketch-code";import {hole} from "./hole.ts";export default sketch(($)=>{const bore=$.use("bore",hole,{radius:mm(2)});return {bore};});',
    "hole.ts": 'import {definePatch,t} from "@geosolve/sketch-code";for(;;){};export const hole=definePatch({radius:t.length()},(p,{radius})=>({circle:p.geometry.centerRadiusCircle("circle",{center:[0,0],radius})}));' };
  let ticks = 0; const timer = setInterval(() => { ticks++; }, 10); t.after(() => clearInterval(timer));
  await assert.rejects(job({ kind: "initialize", folder, files: raw }, { timeoutMs: 1_000 }), (error) => error.code === "timeout");
  assert.ok(ticks >= 5, `Main-thread timer advanced ${ticks} times`);
  const controller = new AbortController(), pending = job({ kind: "initialize", folder, files: raw }, { signal: controller.signal });
  setTimeout(() => controller.abort(), 100);
  await assert.rejects(pending, (error) => error.code === "cancelled");
  const accepted = await job({ kind: "initialize", folder, files: files() }); checkAccepted(accepted.result);
});

test("domain captured source Apply preserves raw receipt identity and unused file changes", async (t) => {
  const folder = await fixture(t), raw = files(); const initial = await job({ kind: "initialize", folder, files: raw });
  const capturedFiles = { ...raw, "sketch.ts": source.replace("radius: mm(2)", "radius: mm(8)") };
  const applied = await job({ kind: "apply", folder, files: raw, model: initial.model, capture: { acceptedBasis: { files: raw }, working: { files: capturedFiles } } });
  assert.equal(circleRadius(applied.result), 8); assert.deepEqual(applied.candidateFiles, capturedFiles);
  const changed = applied.valueChanges.find((entry) => JSON.stringify(entry.write.path) === '["radius"]');
  assert.deepEqual(changed, { write: radius(8), before: radius(2).value });
  assert.deepEqual(applied.inventory.properties.find((entry) => entry.declaration === "bore" && JSON.stringify(entry.path) === '["radius"]').value, radius(8).value);
  const rebuilt = await job({ kind: "rebuild", folder, files: applied.candidateFiles, model: applied.model });
  assert.equal(rebuilt.acceptedInput, applied.acceptedInput);
  await assert.rejects(job({ kind: "apply", folder, files: raw, model: initial.model, capture: { acceptedBasis: { files: raw }, working: { files: capturedFiles } }, invalidatedDeclarations: ["bore"] }), /lifetime changed/u);
  const notes = { ...raw, "notes.txt": "Only notes changed" };
  const noGeometry = await job({ kind: "apply", folder, files: raw, model: initial.model, capture: { acceptedBasis: { files: raw }, working: { files: notes } } });
  assert.equal(noGeometry.model.sourceDesignDigest, initial.model.sourceDesignDigest); assert.notEqual(noGeometry.acceptedInput, initial.acceptedInput);
  assert.equal(noGeometry.preparedContribution.changedPaths[0], "notes.txt");
});

test("domain recompiles custom patch dependencies and rejects mismatched persisted artifacts", async (t) => {
  const folder = await fixture(t);
  const raw = { "geosolve.json": manifest,
    "sketch.ts": '"use geosolve sketch";import {sketch,mm} from "@geosolve/sketch-code";import {hole} from "./hole.ts";export default sketch(($)=>{const bore=$.use("bore",hole,{radius:mm(2)});return {bore};});',
    "hole.ts": 'import {definePatch,t} from "@geosolve/sketch-code";import {center} from "./helper.ts";export const hole=definePatch({radius:t.length()},(p,{radius})=>({circle:p.geometry.centerRadiusCircle("circle",{center,radius})}));',
    "helper.ts": 'export const center=[0,0] as const;' };
  const initial = await job({ kind: "initialize", folder, files: raw }); checkAccepted(initial.result);
  const changed = { ...raw, "helper.ts": 'export const center=[30,40] as const;' };
  await assert.rejects(job({ kind: "rebuild", folder, files: changed, model: initial.model }), /compiler context differs/u);
  const applied = await job({ kind: "apply", folder, files: raw, model: initial.model, capture: { acceptedBasis: { files: raw }, working: { files: changed } } });
  checkAccepted(applied.result); assert.equal(circleRadius(applied.result), 2);
  assert.ok(applied.result.geometry.points.some((point) => point.position[0] === 30 && point.position[1] === 40));
  assert.deepEqual(applied.requiredStableDeclarations, ["bore"]);
  assert.equal((await job({ kind: "rebuild", folder, files: changed, model: applied.model })).acceptedInput, applied.acceptedInput);
  const concurrent = { ...raw, "helper.ts": 'export const center=[50,60] as const;' };
  const latest = await job({ kind: "initialize", folder, files: concurrent });
  await assert.rejects(job({ kind: "apply", folder, files: concurrent, model: latest.model, capture: { acceptedBasis: { files: raw }, working: { files: changed } } }), /Concurrent dependency/u);
  const oldDefinitionEdit = { ...raw, "sketch.ts": raw["sketch.ts"].replace("mm(2)", "mm(5)") };
  await assert.rejects(job({ kind: "apply", folder, files: concurrent, model: latest.model, capture: { acceptedBasis: { files: raw }, working: { files: oldDefinitionEdit } } }), /concurrently changed patch definition/u);
});

test("domain preserves authenticated declaration high water and maps compiler identities and keyed paths", async (t) => {
  const folder = await fixture(t), raw = files();
  raw["sketch.ts"] = '"use geosolve sketch";import {sketch,mm} from "@geosolve/sketch-code";export default sketch(($)=>{const offset=0;const radius=$.parameter("boreRadius",mm(2));const circle=$.geometry.centerRadiusCircle("bore",{center:[0,0],radius});const line=$.geometry.polyline("path",{vertices:[{key:"a",position:[0,0]},{key:"b",position:[10,0]}],closed:false});return {circle,line};});';
  const initial = await job({ kind: "initialize", folder, files: raw });
  assert.deepEqual(initial.inventory.objects.find((entry) => entry.declaration === "bore").dependencies, ["sketch.ts#boreRadius"]);
  assert.deepEqual(initial.inventory.properties.find((entry) => entry.declaration === "offset").value, { kind: "number", value: 0 });
  assert.ok(initial.inventory.properties.some((entry) => entry.declaration === "path" && JSON.stringify(entry.path) === JSON.stringify(["vertices", { member: "b" }, "position", 0])));
  const project = JSON.parse(initial.model.project); project.managed.declaration_name_high_water = 57;
  const engine = await createEngine(); t.after(() => engine.dispose());
  const session = engine.openEditableSession(project, { design: initial.model.design }); t.after(() => session.dispose());
  const model = { ...initial.model, project: session.exportProject(), design: session.exportDesign(), sourceDesignDigest: session.sourceDesignDigest() };
  const rebuilt = await job({ kind: "rebuild", folder, files: raw, model });
  assert.equal(JSON.parse(rebuilt.model.project).managed.declaration_name_high_water, 57);
  assert.equal(rebuilt.model.sourceDesignDigest, model.sourceDesignDigest);
  const nextFiles = { ...raw, "sketch.ts": raw["sketch.ts"].replace('mm(2)', 'mm(3)') };
  const applied = await job({ kind: "apply", folder, files: raw, model, capture: { acceptedBasis: { files: raw }, working: { files: nextFiles } } });
  assert.equal(JSON.parse(applied.model.project).managed.declaration_name_high_water, 57);
  assert.equal(circleRadius(applied.result), 3);
});
