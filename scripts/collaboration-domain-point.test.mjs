// SPDX-License-Identifier: GPL-3.0-or-later
import assert from "node:assert/strict";
import { test } from "node:test";
import { mkdtemp, rm, readFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { runCollaborationDomainJob as job } from "./collaboration-domain.mjs";
import { createEngine } from "../packages/geosolve-engine/dist/index.js";
const viewport = { screen_size: [800, 600], model_center: [0, 0], pixels_per_model_unit: 10 };
async function fixture(t, name = "point-gesture-constrained.json") {
  const folder = await mkdtemp(join(tmpdir(), "geosolve-domain-point-")); t.after(() => rm(folder, { recursive: true, force: true }));
  const compiled = JSON.parse(await readFile(new URL(`../crates/geosolve-sketch-engine/tests/fixtures/${name}`, import.meta.url)));
  const files = { "geosolve.json": JSON.stringify({ format: "geosolve-folder-v2", entry: "sketch.ts", mode: "editable" }), "sketch.ts": compiled.normalizedSource };
  const initial = await job({ kind: "initialize", folder, files }); return { folder, files, initial };
}
async function command(t, model, origin, destination, select) {
  const engine = await createEngine(); t.after(() => engine.dispose());
  const session = engine.openEditableSession(model.project, { design: model.design }); t.after(() => session.dispose());
  const target = session.pointGestureTargets().find(select ?? ((handle) => Math.hypot(handle.position[0] - origin[0], handle.position[1] - origin[1]) < 1e-9))?.target;
  assert.ok(target);
  const prediction = session.beginPointGesture(target, { expected: session.token, gestureId: 1, viewport });
  const before = session.sourceDesignDigest();
  assert.equal(prediction.advance({ sequence: 1, position: destination }).accepted, true);
  const terminal = prediction.finish(); assert.equal(session.sourceDesignDigest(), before); return terminal.command;
}
function bar(result) {
  const [a, b] = result.geometry.points.map((entry) => entry.position);
  assert.ok(Math.abs(Math.hypot(b[0] - a[0], b[1] - a[1]) - 20) < 1e-9);
  assert.ok(Math.abs(b[1] - a[1]) < 1e-9 && b[0] > a[0]);
  assert.equal(result.validation.hard_residuals_validated, true);
  assert.ok(Number.isFinite(result.validation.maximum_normalized_hard_residual) && result.validation.maximum_normalized_hard_residual <= 1e-9);
}
test("domain independently replays point gestures and records every coupled point property for cold Undo", async (t) => {
  const f = await fixture(t), intent = await command(t, f.initial.model, [0, 0], [6, 4]);
  const moved = await job({ kind: "point_gesture", folder: f.folder, files: f.files, model: f.initial.model, command: intent });
  bar(moved.result); assert.deepEqual(moved.candidateFiles, f.files); assert.equal(moved.model.project, f.initial.model.project);
  assert.equal(moved.pointChanges.length, 2); assert.deepEqual(moved.pointChanges.map((change) => change.address.output[0]).sort(), ["end", "start"]);
  assert.ok(moved.pointChanges.every((change) => change.object === "sketch.ts#bar" && change.before === null && change.after.value.value === "point"));
  assert.deepEqual(moved.patches[0].patch.edits, []);
  const workingFiles = { ...f.files, "sketch.ts": f.files["sketch.ts"] + "const incomplete = (" };
  const draft = await job({ kind: "reconcile", preparedContribution: moved.preparedContribution, workingFiles });
  assert.deepEqual(draft.workingFiles, workingFiles); assert.deepEqual(draft.reconciliations[0].patch.edits, []);
  const rebuilt = await job({ kind: "rebuild", folder: f.folder, files: f.files, model: moved.model });
  assert.equal(rebuilt.acceptedInput, moved.acceptedInput); bar(rebuilt.result);
  const undone = await job({ kind: "point_properties", folder: f.folder, files: f.files, model: rebuilt.model,
    writes: moved.pointChanges.map(({ address, before, after }) => ({ address, expected: after, value: before })) });
  assert.equal(undone.acceptedInput, f.initial.acceptedInput); bar(undone.result);
  assert.deepEqual(undone.pointTargets, f.initial.pointTargets);
  await assert.rejects(job({ kind: "point_gesture", folder: f.folder, files: f.files, model: moved.model, command: intent }), /basis/u);
  await assert.rejects(job({ kind: "point_properties", folder: f.folder, files: f.files, model: moved.model,
    writes: moved.pointChanges.map(({ address }) => ({ address, expected: null, value: null })) }), (error) => error.code === "stale_property");
  const forged = structuredClone(moved.pointChanges[0]); forged.address.owner.generation++;
  await assert.rejects(job({ kind: "point_properties", folder: f.folder, files: f.files, model: moved.model,
    writes: [{ address: forged.address, expected: null, value: forged.after }] }));
});

test("domain point property inverse retains an unrelated later point contribution", async (t) => {
  const f = await fixture(t, "point-gesture-shared.json");
  const consumer = await command(t, f.initial.model, [0, 0], [6, 4], (handle) => handle.target.address?.owner.address.declaration === "consumer");
  const first = await job({ kind: "point_gesture", folder: f.folder, files: f.files, model: f.initial.model, command: consumer });
  const producer = await command(t, first.model, [0, 0], [1, 2], (handle) => handle.target.address?.owner.address.declaration === "producer");
  const second = await job({ kind: "point_gesture", folder: f.folder, files: f.files, model: first.model, command: producer });
  const undone = await job({ kind: "point_properties", folder: f.folder, files: f.files, model: second.model,
    writes: first.pointChanges.map(({ address, before, after }) => ({ address, expected: after, value: before })) });
  assert.ok(undone.model.design.overrides.drafts.some(([address, draft]) => address.owner.address.declaration === "producer" && draft.value.seed[0] === 1 && draft.value.seed[1] === 2));
  assert.ok(!undone.model.design.overrides.drafts.some(([address]) => address.owner.address.declaration === "consumer"));
  // Removing the consumer override restores its authored reference, following
  // the producer's later position rather than restoring an old global scene.
  assert.ok(undone.result.geometry.points.every((point) => Math.hypot(point.position[0] - 1, point.position[1] - 2) < 1e-9));
});

test("domain same-value point property owns its explicit target without claiming unchanged companions", async (t) => {
  const f = await fixture(t), firstIntent = await command(t, f.initial.model, [0, 0], [6, 4]);
  const absent = await job({ kind: "point_properties", folder: f.folder, files: f.files, model: f.initial.model,
    writes: [{ address: firstIntent.target.address, expected: null, value: null }] });
  assert.equal(absent.acceptedInput, f.initial.acceptedInput);
  assert.equal(absent.pointChanges.length, 1);
  assert.equal(absent.pointChanges[0].before, null); assert.equal(absent.pointChanges[0].after, null);
  const reordered = (value) => Array.isArray(value) ? value.map(reordered) : value && typeof value === "object"
    ? Object.fromEntries(Object.entries(value).reverse().map(([key, child]) => [key, reordered(child)])) : value;
  const equivalent = await job({ kind: "point_properties", folder: f.folder, files: f.files, model: f.initial.model,
    writes: [{ address: reordered(firstIntent.target.address), expected: null, value: null }] });
  // History property identity uses JSON serialization: equivalent native
  // addresses must not acquire different ownership through object key order.
  assert.equal(JSON.stringify(equivalent.pointChanges), JSON.stringify(absent.pointChanges));
  const forgedAbsent = structuredClone(firstIntent.target.address); forgedAbsent.owner.generation++;
  await assert.rejects(job({ kind: "point_properties", folder: f.folder, files: f.files, model: f.initial.model,
    writes: [{ address: forgedAbsent, expected: null, value: null }] }), (error) => error.code === "stale_property");
  const first = await job({ kind: "point_gesture", folder: f.folder, files: f.files, model: f.initial.model, command: firstIntent });
  const explicit = first.pointChanges.find((change) => change.address.output[0] === "start");
  const same = await job({ kind: "point_properties", folder: f.folder, files: f.files, model: first.model,
    writes: [{ address: explicit.address, expected: explicit.after, value: explicit.after }] });
  bar(same.result); assert.equal(same.acceptedInput, first.acceptedInput);
  assert.deepEqual(same.model, first.model);
  assert.equal(same.pointChanges.length, 1);
  assert.deepEqual(same.pointChanges[0].address, explicit.address);
  assert.deepEqual(same.pointChanges[0].before, same.pointChanges[0].after);
  const inverse = await job({ kind: "point_properties", folder: f.folder, files: f.files, model: same.model,
    writes: same.pointChanges.map(({ address, before, after }) => ({ address, expected: after, value: before })) });
  assert.equal(inverse.acceptedInput, first.acceptedInput);
  assert.deepEqual(inverse.pointChanges, same.pointChanges);
});

test("domain point replay retains rectangle intent and detaches only the explicit consumer", async (t) => {
  const rectangle = await fixture(t, "point-gesture-rectangle.json");
  const intent = await command(t, rectangle.initial.model, [20, 10], [25, 15]);
  const resized = await job({ kind: "point_gesture", folder: rectangle.folder, files: rectangle.files, model: rectangle.initial.model, command: intent });
  const positions = resized.result.geometry.points.map((entry) => entry.position);
  for (const expected of [[0, 0], [25, 0], [25, 15], [0, 15]]) assert.ok(positions.some((position) => Math.hypot(position[0] - expected[0], position[1] - expected[1]) < 1e-9));
  assert.deepEqual(resized.model.design.generated, rectangle.initial.model.design.generated);
  assert.equal(resized.pointChanges.length, 2);
  const restored = await job({ kind: "point_properties", folder: rectangle.folder, files: rectangle.files, model: resized.model,
    writes: resized.pointChanges.map(({ address, before, after }) => ({ address, expected: after, value: before })) });
  assert.equal(restored.acceptedInput, rectangle.initial.acceptedInput);
  const shared = await fixture(t, "point-gesture-shared.json");
  const consumer = await command(t, shared.initial.model, [0, 0], [6, 4], (handle) => handle.target.address?.owner.address.declaration === "consumer");
  const detached = await job({ kind: "point_gesture", folder: shared.folder, files: shared.files, model: shared.initial.model, command: consumer });
  assert.deepEqual(detached.result.geometry.points.map((entry) => entry.position).sort((a, b) => a[0] - b[0]), [[0, 0], [6, 4]]);
  assert.ok(detached.pointChanges.every((change) => change.declaration === "consumer"));
  assert.equal((await job({ kind: "rebuild", folder: shared.folder, files: shared.files, model: detached.model })).acceptedInput, detached.acceptedInput);
});

test("domain point replay preserves computed fillets and refuses a native-valid infeasible terminal", async (t) => {
  const f = await fixture(t, "point-gesture-computed.json");
  const intent = await command(t, f.initial.model, [20, 20], [22, 20]);
  const moved = await job({ kind: "point_gesture", folder: f.folder, files: f.files, model: f.initial.model, command: intent });
  assert.ok(moved.result.geometry.computed_edges.length > 0); assert.equal(moved.result.validation.all_active_features_current, true);
  assert.equal(moved.model.project, f.initial.model.project); assert.deepEqual(moved.model.design.generated, f.initial.model.design.generated);
  const bad = { ...intent, samples: [{ sequence: 1, position: [20, 1] }] };
  await assert.rejects(job({ kind: "point_gesture", folder: f.folder, files: f.files, model: f.initial.model, command: bad }), /candidate evaluation rejected/u);
  const rebuilt = await job({ kind: "rebuild", folder: f.folder, files: f.files, model: f.initial.model });
  assert.equal(rebuilt.acceptedInput, f.initial.acceptedInput); assert.ok(rebuilt.result.geometry.computed_edges.length > 0);
});
