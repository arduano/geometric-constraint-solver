// SPDX-License-Identifier: GPL-3.0-or-later
import assert from "node:assert/strict";
import { test } from "node:test";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { createCollaborationDomainService, runCollaborationDomainJob as cold } from "./collaboration-domain.mjs";
const files = {
  "geosolve.json": JSON.stringify({ format: "geosolve-folder-v2", entry: "sketch.ts", mode: "editable" }),
  "sketch.ts": '"use geosolve sketch";import {sketch,mm} from "@geosolve/sketch-code";export default sketch(($)=>{const first=$.geometry.centerRadiusCircle("first",{center:[-20,0],radius:mm(5)});const second=$.geometry.centerRadiusCircle("second",{center:[20,0],radius:mm(3)});return {first,second};});',
};
const viewport = { screen_size: [800, 600], model_center: [0, 0], pixels_per_model_unit: 10 };
async function setup(t) {
  const folder = await mkdtemp(join(tmpdir(), "geosolve-domain-retained-")), service = createCollaborationDomainService();
  t.after(async () => { await service.dispose(); await rm(folder, { recursive: true, force: true }); });
  return { folder, service, initial: await service.run({ kind: "initialize", folder, files }) };
}
function check(result) {
  assert.equal(result.validation.hard_residuals_validated, true);
  assert.equal(result.validation.all_active_features_current, true);
  assert.ok(Number.isFinite(result.validation.maximum_normalized_hard_residual) && result.validation.maximum_normalized_hard_residual <= 1e-9);
  assert.ok(result.geometry.points.every(point => point.position.every(Number.isFinite)));
  assert.deepEqual(result.geometry.scalars.map(scalar => scalar.value).sort(), [3, 5]);
}
function command(basis, position, sequence = 1) {
  return { basis: basis.model.sourceDesignDigest, gesture_id: sequence, target: basis.pointTargets[0].target,
    viewport, samples: [{ sequence: 1, position }] };
}
const scene = (run, folder, basis) => run({ kind: "scene", folder, files, model: basis.model, expectedInput: basis.acceptedInput });
async function terminal(run, folder, basis, position, replayBasis = basis) {
  const moved = await run({ kind: "point_gesture", folder, files, model: basis.model, expectedInput: basis.acceptedInput,
    replayBasis: { files, model: replayBasis.model }, command: command(replayBasis, position) });
  const presented = await scene(run, folder, moved);
  const workingFiles = { ...files, "sketch.ts": files["sketch.ts"] + "const invalid = (" };
  const reconciled = await run({ kind: "reconcile", workingFiles, preparedContribution: moved.preparedContribution });
  assert.deepEqual(reconciled.workingFiles, workingFiles); assert.deepEqual(reconciled.reconciliations[0].patch.edits, []);
  check(moved.result); assert.equal(presented.acceptedInput, moved.acceptedInput);
  assert.ok(JSON.parse(presented.scene.seed.scene).points.some(point => point.model_position.every((value, index) => Math.abs(value - position[index]) < 1e-9)));
  return moved;
}
test("retained domain removes cold point/scene reconstruction while matching independently rebuilt geometry", async (t) => {
  const { folder, service, initial } = await setup(t), run = service.run;
  await scene(run, folder, initial);
  let current = initial; const samples = [];
  for (let index = 0; index < 4; index++) {
    const start = performance.now(); current = await terminal(run, folder, current, [-18 + index, 4 + index]);
    samples.push(performance.now() - start);
  }
  const start = performance.now(), independent = await terminal(cold, folder, initial, [-15, 7]);
  const coldMs = performance.now() - start;
  assert.deepEqual(current.model, independent.model); assert.equal(current.acceptedInput, independent.acceptedInput);
  function semanticGeometry(geometry) {
    const ids = new Map();
    for (const kind of ["points", "scalars", "curves"]) for (const entry of geometry[kind]) {
      const item = entry.curve ?? entry; ids.set(item.id, `${kind}:${item.label}`);
    }
    const remap = value => typeof value === "string" ? (ids.get(value) ?? value) : Array.isArray(value) ? value.map(remap)
      : value && typeof value === "object" ? Object.fromEntries(Object.entries(value).map(([key, child]) => [ids.get(key) ?? key, remap(child)])) : value;
    return remap(geometry);
  }
  assert.deepEqual(semanticGeometry(current.result.geometry), semanticGeometry(independent.result.geometry));
  const median = [...samples].sort((a, b) => a - b)[Math.floor(samples.length / 2)];
  t.diagnostic(JSON.stringify({ warmTerminalMs: samples, median, coldTerminalMs: coldMs }));
  assert.ok(median < coldMs / 3, `Warm terminal median ${median} ms must eliminate repeated cold reconstruction (${coldMs} ms)`);
  assert.ok(median < 500, `Two-circle warm terminal median exceeded 500 ms: ${median}`);
});

test("retained domain authenticates full model, source, expected input and historical basis after cache hits", async (t) => {
  const { folder, service, initial } = await setup(t), run = service.run;
  const first = await terminal(run, folder, initial, [-14, 6]);
  const request = { kind: "rebuild", folder, files, model: first.model, expectedInput: first.acceptedInput };
  await assert.rejects(run({ ...request, expectedInput: initial.acceptedInput }), error => error.code === "stale_input");
  await assert.rejects(run({ ...request, files: { ...files, "unused.ts": "// changed accepted bytes" } }), error => error.code === "stale_input");
  const corrupt = structuredClone(first.model); corrupt.sourceDesignDigest = "f".repeat(64);
  await assert.rejects(run({ ...request, model: corrupt }), /identity differs/u);
  const forged = structuredClone(first.model), project = JSON.parse(forged.project);
  project.managed.source += "// forged canonical source"; forged.project = JSON.stringify(project);
  await assert.rejects(run({ ...request, model: forged }), /differs from independently/u);
  const invalid = command(first, [-10, 7]); invalid.target.address.owner.generation++;
  await assert.rejects(run({ kind: "point_gesture", folder, files, model: first.model, command: invalid }));
  const restored = await run(request); assert.deepEqual(restored.model, first.model); check(restored.result);
  // An old authenticated basis still replays onto the latest model; its session
  // has never been changed by the earlier candidate, rejection or scene job.
  const replayed = await terminal(run, folder, first, [-12, 8], initial);
  const independent = await terminal(cold, folder, first, [-12, 8], initial);
  assert.deepEqual(replayed.model, independent.model); check(replayed.result);
});

test("retained services isolate documents and recover queued work after cancellation and timeout", async (t) => {
  const { folder, service, initial } = await setup(t), other = await setup(t);
  await assert.rejects(service.run({ kind: "rebuild", folder: other.folder, files, model: initial.model }), /exactly one document/u);
  const moved = await terminal(service.run, folder, initial, [-10, 2]);
  assert.notEqual(moved.acceptedInput, other.initial.acceptedInput);
  const unchanged = await other.service.run({ kind: "rebuild", folder: other.folder, files, model: other.initial.model });
  assert.deepEqual(unchanged.model, other.initial.model);
  const looping = { ...files,
    "sketch.ts": '"use geosolve sketch";import {sketch,mm} from "@geosolve/sketch-code";import {hole} from "./hole.ts";export default sketch(($)=>{const bore=$.use("bore",hole,{radius:mm(2)});return {bore};});',
    "hole.ts": 'import {definePatch,t} from "@geosolve/sketch-code";for(;;){};export const hole=definePatch({radius:t.length()},(p,{radius})=>({circle:p.geometry.centerRadiusCircle("circle",{center:[0,0],radius})}));',
  };
  const controller = new AbortController(), cancelled = service.run({ kind: "initialize", folder, files: looping }, { signal: controller.signal });
  const cancellation = assert.rejects(cancelled, error => error.code === "cancelled");
  const queuedController = new AbortController();
  const queued = service.run({ kind: "rebuild", folder, files, model: initial.model }, { signal: queuedController.signal });
  const queuedCancellation = assert.rejects(queued, error => error.code === "cancelled");
  queuedController.abort(); await queuedCancellation;
  const surviving = service.run({ kind: "rebuild", folder, files, model: moved.model });
  setTimeout(() => controller.abort(), 300);
  await cancellation; assert.deepEqual((await surviving).model, moved.model);
  let ticks = 0; const timer = setInterval(() => ticks++, 10);
  try {
    await assert.rejects(service.run({ kind: "initialize", folder, files: looping }, { timeoutMs: 500 }), error => error.code === "timeout");
    assert.ok(ticks >= 10, `Main-thread timer advanced only ${ticks} times`);
  } finally { clearInterval(timer); }
  assert.deepEqual((await service.run({ kind: "rebuild", folder, files, model: moved.model })).model, moved.model);
});

test("retained scene requests do not inherit a previous request's custom viewport", async (t) => {
  const { folder, service, initial } = await setup(t);
  const custom = await service.run({ kind: "scene", folder, files, model: initial.model, viewport: { width: 900, height: 700, pixelRatio: 2 } });
  const restored = await scene(service.run, folder, initial), independent = await scene(cold, folder, initial);
  assert.notDeepEqual(custom.scene.snapshot.frame.scene.viewBox, independent.scene.snapshot.frame.scene.viewBox);
  assert.deepEqual(restored.scene.snapshot.frame.scene.viewBox, independent.scene.snapshot.frame.scene.viewBox);
  // Workbench restoration allocates a fresh native document namespace. Compare
  // every semantic and drawing field after mapping only those native IDs to
  // their exact stable compiler labels; do not omit geometry or authority.
  function semanticScene(seed) {
    const scene = JSON.parse(seed.scene), document = JSON.parse(scene.document);
    const ids = new Map([[document.document.id, "@document"], [document.document.next_id, "@next"]]);
    for (const kind of ["points", "scalars", "curves"]) for (const item of document.document[kind]) ids.set(item.id, `${kind}:${item.label}`);
    const remap = value => typeof value === "string" ? (ids.get(value) ?? value) : Array.isArray(value) ? value.map(remap)
      : value && typeof value === "object" ? Object.fromEntries(Object.entries(value).map(([key, child]) => [ids.get(key) ?? key, remap(child)])) : value;
    return remap({ ...scene, document });
  }
  assert.deepEqual(semanticScene(restored.scene.seed), semanticScene(independent.scene.seed));
});

test("domain session owner releases superseded results and evicted sessions across accepted and rejected updates", async (t) => {
  const { createEngine } = await import("../packages/geosolve-engine/dist/index.js");
  const { createDomainSessionOwner } = await import("./collaboration-domain-sessions.mjs");
  const { initial } = await setup(t), engine = await createEngine(), owner = createDomainSessionOwner(engine);
  t.after(() => engine.dispose());
  let model = initial.model;
  for (let index = 0; index < 16; index++) {
    const session = owner.open(model.project, { design: model.design });
    assert.equal(session.state.can_undo, false, "Each writable transaction starts without retained native Undo history");
    const prior = session.accepted;
    const intent = { basis: session.sourceDesignDigest(), gesture_id: index + 1, target: session.pointGestureTargets()[0].target,
      viewport, samples: [{ sequence: 1, position: [-18 + index, 2] }] };
    const prepared = session.preparePointGestureCommit(intent, { expected: session.token });
    const changed = await owner.change(session, () => session.applyPointGestureCommit(prepared));
    assert.equal(changed.status, "accepted"); check(session.accepted);
    await assert.rejects(engine.exportProfiles(prior, { chordErrorMm: 0.1 }), /released/u);
    const current = session.accepted;
    await assert.rejects(owner.change(session, () => { throw Error("Rejected before native mutation"); }), /Rejected/u);
    assert.equal(session.accepted, current);
    assert.equal((await engine.exportProfiles(current, { chordErrorMm: 0.1 })).regions.length, 2);
    model = { project: session.exportProject(), design: session.exportDesign() };
    owner.close(session); owner.close(session);
    await assert.rejects(engine.exportProfiles(current, { chordErrorMm: 0.1 }), /released/u);
  }
});
