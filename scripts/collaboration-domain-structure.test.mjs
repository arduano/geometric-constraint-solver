// SPDX-License-Identifier: GPL-3.0-or-later
import assert from "node:assert/strict";
import { test } from "node:test";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { createTrustedSemanticHost } from "../packages/geosolve-collaboration/dist/host.js";
import { runCollaborationDomainJob as job } from "../packages/geosolve-cli/runtime/collaboration-domain.mjs";
const source = '"use geosolve sketch";\nimport {sketch,mm} from "@geosolve/sketch-code";\nexport default sketch(($)=>{\n // Bore stays owned.\n const bore=$.geometry.centerRadiusCircle("bore",{center:[0,0],radius:mm(5)});\n const other=$.geometry.centerRadiusCircle("other",{center:[30,0],radius:mm(3)});\n return {bore,other};\n});\n';
const operation = (userId, requestId) => ({ userId, clientId: `tab-${userId}`, requestId });
async function fixture(t, text = source) {
  const folder = await mkdtemp(join(tmpdir(), "geosolve-domain-structure-")); t.after(() => rm(folder, { recursive: true, force: true }));
  const files = { "geosolve.json": JSON.stringify({ format: "geosolve-folder-v2", entry: "sketch.ts", mode: "editable" }), "sketch.ts": text };
  const initial = await job({ kind: "initialize", folder, files }); return { folder, files, initial };
}
const radius = (declaration, value) => ({ declaration, path: ["radius"], value: { kind: "unit", value: { unit: "mm", value } } });
test("domain Apply owns same-value literal syntax without claiming comments or unrelated properties", async (t) => {
  const f = await fixture(t);
  const apply = (text) => job({ kind: "apply", folder: f.folder, files: f.files, model: f.initial.model,
    capture: { acceptedBasis: { files: f.files }, working: { files: { ...f.files, "sketch.ts": text } } } });
  const changed = await apply(source.replace("mm(5)", "mm(5.0)"));
  const entry = changed.valueChanges.find(({ write }) => write.declaration === "bore" && JSON.stringify(write.path) === '["radius"]');
  assert.ok(entry); assert.deepEqual(entry.before, entry.write.value);
  assert.ok(changed.valueChanges.every(({ write }) => write.declaration === "bore" && (write.path.length === 0 || write.path[0] === "radius")));
  const commented = await apply(source.replace("mm(5)", "mm( /* keep */ 5)")); assert.deepEqual(commented.valueChanges, []);
  await assert.rejects(job({ kind: "apply", folder: f.folder, files: f.files, model: f.initial.model, invalidatedDeclarations: ["bore"],
    capture: { acceptedBasis: { files: f.files }, working: { files: { ...f.files, "sketch.ts": source.replace("mm(5)", "mm(5.0)") } } } }), /lifetime changed/u);
});
test("domain native structural Undo recreates only deleted compiler payload with fresh semantic identity", async (t) => {
  const f = await fixture(t), host = await createTrustedSemanticHost({ configuration: { documentEpoch: "doc", serverEpoch: "one", objects: f.initial.inventory.objects.map(({ object, dependencies }) => ({ object, dependencies })) } }); t.after(() => host.dispose());
  const bore = host.current("sketch.ts#bore"), original = f.initial.inventory.objects.find((item) => item.object === bore.object);
  assert.equal(original.payload.source.includes("Bore stays owned"), true);
  assert.deepEqual(original.position, { previous: null, next: "sketch.ts#other" });
  const deletion = host.planDelete([bore]);
  const removed = await job({ kind: "mutation", folder: f.folder, files: f.files, model: f.initial.model, mutation: { mutation: "delete", target: { target: "declaration", declaration: "bore" } } });
  assert.deepEqual(removed.inventory.objects.map((item) => item.declaration), ["other"]);
  const stage = host.stageValidatedTransaction({ basisRevision: 0, revision: 1, deletions: [deletion], record: { operation: operation("alice", "delete"), changes: [], structural: {
    deleted: [{ target: bore, payload: original.payload, position: { previous: null, next: host.current("sketch.ts#other") } }] } } }); host.commitStage(stage);
  const edited = await job({ kind: "values", folder: f.folder, files: removed.candidateFiles, model: removed.model, writes: [radius("other", 7)] });
  await host.record({ basisRevision: 1, revision: 2, operation: operation("bob", "radius"), changes: [{ address: { target: host.current("sketch.ts#other"), property: '["radius"]' }, before: radius("other", 3).value, after: radius("other", 7).value }] }, async () => {});
  const inverse = host.prepareUndo("alice"); assert.ok(inverse.structural.create[0].target.generation > bore.generation);
  const restored = await job({ kind: "structural_inverse", folder: f.folder, files: edited.candidateFiles, model: edited.model, inverse });
  assert.match(restored.candidateFiles["sketch.ts"], /Bore stays owned/u); assert.match(restored.candidateFiles["sketch.ts"], /radius:\s*mm\(7\)/u);
  assert.ok(restored.candidateFiles["sketch.ts"].includes(original.payload.source)); assert.equal(restored.result.geometry.curves.length, 2);
  assert.equal((await job({ kind: "rebuild", folder: f.folder, files: restored.candidateFiles, model: restored.model })).acceptedInput, restored.acceptedInput);
  host.commitStage(host.stageValidatedInverse(inverse, operation("alice", "undo"), 3)); assert.throws(() => host.authenticate(bore), /lifetime/u);
  const redo = host.prepareRedo("alice"), again = await job({ kind: "structural_inverse", folder: f.folder, files: restored.candidateFiles, model: restored.model, inverse: redo });
  assert.equal(again.inventory.objects.length, 1); assert.match(again.candidateFiles["sketch.ts"], /mm\(7\)/u);
});

test("domain deletion inverse restores only owned point overrides using fresh native generations", async (t) => {
  const f = await fixture(t);
  const target = f.initial.pointTargets.find((item) => item.target.address?.owner.address.declaration === "bore").target;
  const moved = await job({ kind: "point_gesture", folder: f.folder, files: f.files, model: f.initial.model, command: { basis: f.initial.model.sourceDesignDigest, gesture_id: 8,
    target, viewport: { screen_size: [800, 600], model_center: [0, 0], pixels_per_model_unit: 10 }, samples: [{ sequence: 1, position: [4, 6] }] } });
  const payload = moved.inventory.objects.find((item) => item.declaration === "bore").payload;
  assert.equal(payload.overrides.drafts.length, 1);
  const host = await createTrustedSemanticHost({ configuration: { documentEpoch: "doc", serverEpoch: "one", objects: moved.inventory.objects.map(({ object, dependencies }) => ({ object, dependencies })) } }); t.after(() => host.dispose());
  const bore = host.current("sketch.ts#bore"), deletion = host.planDelete([bore]);
  const deleted = await job({ kind: "mutation", folder: f.folder, files: f.files, model: moved.model, mutation: { mutation: "delete", target: { target: "declaration", declaration: "bore" } } });
  host.commitStage(host.stageValidatedTransaction({ basisRevision: 0, revision: 1, deletions: [deletion], record: { operation: operation("alice", "delete"), changes: [], structural: {
    deleted: [{ target: bore, payload, position: { previous: null, next: host.current("sketch.ts#other") } }] } } }));
  const restored = await job({ kind: "structural_inverse", folder: f.folder, files: deleted.candidateFiles, model: deleted.model, inverse: host.prepareUndo("alice") });
  const fresh = restored.pointTargets.find((item) => item.target.address?.owner.address.declaration === "bore");
  assert.deepEqual(fresh.position, [4, 6]); assert.ok(fresh.target.address.owner.allocation > target.address.owner.allocation);
  assert.deepEqual(restored.pointTargets.find((item) => item.target.address?.owner.address.declaration === "other").position, [30, 0]);
  assert.equal((await job({ kind: "rebuild", folder: f.folder, files: restored.candidateFiles, model: restored.model })).acceptedInput, restored.acceptedInput);
});

test("domain reports explicit same-position reorder and restores native neighbor intent", async (t) => {
  const f = await fixture(t);
  const same = await job({ kind: "mutation", folder: f.folder, files: f.files, model: f.initial.model,
    mutation: { mutation: "reorder_declaration", declaration: "bore", before: "other" } });
  assert.equal(same.structuralChanges.reorders.length, 1); assert.deepEqual(same.structuralChanges.reorders[0].before, same.structuralChanges.reorders[0].after);
  const moved = await job({ kind: "mutation", folder: f.folder, files: f.files, model: f.initial.model,
    mutation: { mutation: "reorder_declaration", declaration: "other", before: "bore" } });
  assert.deepEqual(moved.inventory.objects.map(({ declaration }) => declaration), ["other", "bore"]);
  const host = await createTrustedSemanticHost({ configuration: { documentEpoch: "doc", serverEpoch: "one", objects: f.initial.inventory.objects.map(({ object, dependencies }) => ({ object, dependencies })) } }); t.after(() => host.dispose());
  const position = (value) => ({ previous: value.previous && host.current(value.previous), next: value.next && host.current(value.next) });
  host.commitStage(host.stageValidatedTransaction({ basisRevision: 0, revision: 1, record: { operation: operation("alice", "move"), changes: [], structural: {
    reorders: moved.structuralChanges.reorders.map((item) => ({ target: host.current(item.object), before: position(item.before), after: position(item.after) })) } } }));
  const restored = await job({ kind: "structural_inverse", folder: f.folder, files: moved.candidateFiles, model: moved.model, inverse: host.prepareUndo("alice") });
  assert.deepEqual(restored.inventory.objects.map(({ declaration }) => declaration), ["bore", "other"]);
});

test("domain native deletion closure restores sibling reference dependencies and source positions", async (t) => {
  const text = source.replace(' const other=', ' const linked=$.geometry.sketchPoint("linked",{point:bore.center});\n const other=').replace('return {bore,other}', 'return {bore,linked,other}');
  const f = await fixture(t, text), host = await createTrustedSemanticHost({ configuration: { documentEpoch: "doc", serverEpoch: "one", objects: f.initial.inventory.objects.map(({ object, dependencies }) => ({ object, dependencies })) } }); t.after(() => host.dispose());
  const plan = host.planDelete([host.current("sketch.ts#bore")]); assert.equal(plan.closure.length, 2);
  const deleted = await job({ kind: "mutation", folder: f.folder, files: f.files, model: f.initial.model, mutation: { mutation: "delete", target: { target: "declaration", declaration: "bore" } } });
  const descriptions = plan.closure.map((target) => { const item = f.initial.inventory.objects.find((entry) => entry.object === target.object);
    return { target, payload: item.payload, position: { previous: item.position.previous && host.current(item.position.previous), next: item.position.next && host.current(item.position.next) } }; });
  host.commitStage(host.stageValidatedTransaction({ basisRevision: 0, revision: 1, deletions: [plan], record: { operation: operation("alice", "delete"), changes: [], structural: { deleted: descriptions } } }));
  const restored = await job({ kind: "structural_inverse", folder: f.folder, files: deleted.candidateFiles, model: deleted.model, inverse: host.prepareUndo("alice") });
  assert.deepEqual(restored.inventory.objects.map(({ declaration }) => declaration), ["bore", "linked", "other"]);
  assert.deepEqual(restored.inventory.objects.find((item) => item.declaration === "linked").dependencies, ["sketch.ts#bore"]);
  assert.match(restored.candidateFiles["sketch.ts"], /point:bore\.center/u);
});

test("domain metadata changes have exact absence and inverse retains unrelated labels and dimensions", async (t) => {
  const f = await fixture(t), mutation = { mutation: "set_metadata", target: { target: "declaration", declaration: "bore" }, property: "label", value: { kind: "string", value: "Cooling port" } };
  const changed = await job({ kind: "mutation", folder: f.folder, files: f.files, model: f.initial.model, mutation });
  assert.deepEqual(changed.metadataChanges, [{ object: "sketch.ts#bore", target: mutation.target, property: "label", before: null, after: mutation.value }]);
  const host = await createTrustedSemanticHost({ configuration: { documentEpoch: "doc", serverEpoch: "one", objects: f.initial.inventory.objects.map(({ object, dependencies }) => ({ object, dependencies })) } }); t.after(() => host.dispose());
  await host.record({ basisRevision: 0, revision: 1, operation: operation("alice", "label"), changes: [{ address: { target: host.current("sketch.ts#bore"), property: '["metadata","declaration","label"]' }, before: null, after: mutation.value }] }, async () => {});
  const restored = await job({ kind: "structural_inverse", folder: f.folder, files: changed.candidateFiles, model: changed.model, inverse: host.prepareUndo("alice") });
  assert.doesNotMatch(restored.candidateFiles["sketch.ts"], /Cooling port/u); assert.match(restored.candidateFiles["sketch.ts"], /mm\(5\)/u);
  assert.equal(restored.result.geometry.curves.length, 2);
});

test("domain reference replacement and its native structural inverse preserve explicit dependencies", async (t) => {
  const text = source.replace(' return {bore,other}', ' const linked=$.geometry.sketchPoint("linked",{point:bore.center});\n return {bore,other,linked}');
  const f = await fixture(t, text), reference = (declaration) => ({ kind: "reference", value: { declaration, path: ["center"] } });
  const changed = await job({ kind: "mutation", folder: f.folder, files: f.files, model: f.initial.model,
    mutation: { mutation: "set_value", declaration: "linked", path: ["point"], expected: reference("bore"), value: reference("other") } });
  const host = await createTrustedSemanticHost({ configuration: { documentEpoch: "doc", serverEpoch: "one", objects: f.initial.inventory.objects.map(({ object, dependencies }) => ({ object, dependencies })) } }); t.after(() => host.dispose());
  const target = host.current("sketch.ts#linked");
  assert.deepEqual(changed.structuralChanges.dependencies, [{ object: target.object, before: ["sketch.ts#bore"], after: ["sketch.ts#other"] }]);
  assert.deepEqual(changed.valueChanges, [{ write: { declaration: "linked", path: ["point"], value: reference("other") }, before: reference("bore") }]);
  host.commitStage(host.stageValidatedTransaction({ basisRevision: 0, revision: 1, dependencies: [{ target: { kind: "existing", target }, dependencies: [{ kind: "existing", target: host.current("sketch.ts#other") }] }],
    record: { operation: operation("alice", "reference"), changes: [{ address: { target, property: '["point"]' }, before: reference("bore"), after: reference("other") }], structural: {} } }));
  const restored = await job({ kind: "structural_inverse", folder: f.folder, files: changed.candidateFiles, model: changed.model, inverse: host.prepareUndo("alice") });
  assert.deepEqual(restored.inventory.objects.find((item) => item.declaration === "linked").dependencies, ["sketch.ts#bore"]);
  assert.match(restored.candidateFiles["sketch.ts"], /point:bore\.center/u);
  const same = await job({ kind: "mutation", folder: f.folder, files: changed.candidateFiles, model: changed.model,
    mutation: { mutation: "set_value", declaration: "linked", path: ["point"], expected: reference("other"), value: reference("other") } });
  assert.equal(same.structuralChanges.dependencies.length, 1); assert.deepEqual(same.structuralChanges.dependencies[0].before, same.structuralChanges.dependencies[0].after);
});

test("compiler suppression history preserves exact activation, explicit same-value ownership and independent values", async () => {
  const { compileManagedSource, applyManagedSketchMutation } = await import("../packages/geosolve-sketch-code/dist/src/managed.js");
  const { suppressionChanges, prepareStructuralSource } = await import("../packages/geosolve-cli/runtime/collaboration-domain-structure.mjs");
  const before = { entry: "sketch.ts", compiled: compileManagedSource(source), patches: {} };
  const mutation = { mutation: "set_suppressed", target: { target: "declaration", declaration: "bore" }, suppressed: true };
  const after = { ...before, compiled: applyManagedSketchMutation(before.compiled, mutation).compiled };
  const change = { object: "sketch.ts#bore", property: '["suppression",[]]', before: false, after: true };
  assert.deepEqual(suppressionChanges(before, after, mutation), [change]);
  assert.deepEqual(suppressionChanges(after, after, mutation), [{ ...change, before: true }]);
  assert.deepEqual(suppressionChanges(before, before, { ...mutation, suppressed: false }), [{ ...change, after: false }]);
  const changed = { ...after, compiled: applyManagedSketchMutation(after.compiled, { mutation: "set_value", declaration: "other", path: ["radius"], expected: { kind: "unit", value: { unit: "mm", value: 3 } }, value: { kind: "unit", value: { unit: "mm", value: 7 } } }).compiled };
  const inverse = { structural: {}, changes: [{ address: { target: { object: change.object, generation: 1 }, property: change.property }, before: true, after: false }] };
  const restored = prepareStructuralSource(changed, changed.compiled.normalizedSource, inverse);
  assert.doesNotMatch(restored.source, /\$\.suppress/u); assert.match(restored.source, /mm\(7\)/u);
  assert.throws(() => prepareStructuralSource(before, before.compiled.normalizedSource, inverse), /no longer matches/u);
  const redo = { ...inverse, changes: inverse.changes.map(item => ({ ...item, before: false, after: true })) };
  const suppressed = prepareStructuralSource({ ...changed, compiled: restored.compiled }, restored.source, redo);
  assert.equal(suppressed.compiled.canonicalArtifactJson, changed.compiled.canonicalArtifactJson);
});
