// SPDX-License-Identifier: GPL-3.0-or-later
import assert from "node:assert/strict";
import { test } from "node:test";
import { mkdtemp, rm } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { runCollaborationDomainJob as job } from "./collaboration-domain.mjs";
import { createTrustedSemanticHost } from "../packages/geosolve-collaboration/dist/host.js";
const source = '"use geosolve sketch";import{sketch,mm}from"@geosolve/sketch-code";export default sketch(($)=>{const bore=$.geometry.centerRadiusCircle("bore",{center:[0,0],radius:mm(2),label:"Base"});const other=$.geometry.centerRadiusCircle("other",{center:[30,0],radius:mm(3)});return{bore,other};});';
const operation = (userId, requestId) => ({ userId, clientId: `tab-${userId}`, requestId });
const number = (value) => ({ kind: "number", value }), pair = (x, y) => ({ kind: "array", value: [number(x), number(y)] });
async function fixture(t) {
  const folder = await mkdtemp(join(tmpdir(), "geosolve-domain-properties-")); t.after(() => rm(folder, { recursive: true, force: true }));
  const files = { "geosolve.json": JSON.stringify({ format: "geosolve-folder-v2", entry: "sketch.ts", mode: "editable" }), "sketch.ts": source };
  let accepted = await job({ kind: "initialize", folder, files });
  const host = await createTrustedSemanticHost({ configuration: { documentEpoch: "doc", serverEpoch: "one", objects: accepted.inventory.objects.map(({ object, dependencies }) => ({ object, dependencies })) } }); t.after(() => host.dispose());
  async function domain(kind, extra) { accepted = await job({ kind, folder, files: accepted.candidateFiles, model: accepted.model, ...extra }); return accepted; }
  async function record(user, id, result) { await host.record({ basisRevision: host.snapshot().revision, revision: host.snapshot().revision + 1, operation: operation(user, id),
    changes: result.propertyChanges.map(({ object, property, before, after }) => ({ address: { target: host.current(object), property }, before, after })) }, async () => {}); }
  async function values(user, id, path, value) { const result = await domain("values", { writes: [{ declaration: "bore", path, value }] }); await record(user, id, result); return result; }
  async function undo(user, id) { const inverse = host.prepareUndo(user), result = await domain("structural_inverse", { inverse }); host.commitStage(host.stageValidatedInverse(inverse, operation(user, id), host.snapshot().revision + 1)); return result; }
  return { folder, files, host, domain, record, values, undo, get accepted() { return accepted; } };
}
test("canonical ownership bars parent/child overwrite in either direction but preserves disjoint siblings", async (t) => {
  const parentFirst = await fixture(t);
  await parentFirst.values("alice", "parent", ["center"], pair(4, 6));
  await parentFirst.values("bob", "child", ["center", 0], number(7));
  assert.throws(() => parentFirst.host.prepareUndo("alice"), /owns|overwrit/u);
  const childFirst = await fixture(t);
  await childFirst.values("alice", "child", ["center", 0], number(4));
  const sameParent = await childFirst.values("bob", "parent", ["center"], pair(4, 0));
  assert.ok(sameParent.propertyChanges.some((item) => item.property === '["source",["center",0]]' && JSON.stringify(item.before) === JSON.stringify(item.after)));
  assert.throws(() => childFirst.host.prepareUndo("alice"), /owns|overwrit/u);
  const siblings = await fixture(t);
  await siblings.values("alice", "x", ["center", 0], number(4));
  await siblings.values("bob", "y", ["center", 1], number(6));
  const undone = await siblings.undo("alice", "undo-x");
  assert.deepEqual(undone.pointTargets.find((item) => item.target.address?.owner.address.declaration === "bore").position, [0, 6]);
  const cold = await job({ kind: "rebuild", folder: siblings.folder, files: undone.candidateFiles, model: undone.model }); assert.equal(cold.acceptedInput, undone.acceptedInput);
});
test("same-value metadata inverse preserves native model and metadata aliases share one ownership key", async (t) => {
  const f = await fixture(t), mutation = { mutation: "set_metadata", target: { target: "declaration", declaration: "bore" }, property: "label", value: { kind: "string", value: "Cooling port" } };
  const first = await f.domain("mutation", { mutation }); await f.record("alice", "label", first);
  const same = await f.domain("mutation", { mutation }); await f.record("bob", "same", same);
  assert.equal(same.propertyChanges.length, 1); assert.equal(same.propertyChanges[0].property, '["metadata","declaration","label"]');
  assert.throws(() => f.host.prepareUndo("alice"), /owns|overwrit/u);
  const undone = await f.undo("bob", "undo-same"); assert.equal(undone.acceptedInput, same.acceptedInput); assert.deepEqual(undone.patches[0].patch.edits, []);
  const viaValue = await f.values("bob", "direct-label", ["label"], mutation.value);
  assert.equal(viaValue.propertyChanges.length, 1); assert.equal(viaValue.propertyChanges[0].property, '["metadata","declaration","label"]');
  assert.throws(() => f.host.prepareUndo("alice"), /owns|overwrit/u);
});
test("older point history resolves exact semantic lens after deletion Undo allocates fresh native identities", async (t) => {
  const f = await fixture(t), old = f.accepted.pointTargets.find((item) => item.target.address?.owner.address.declaration === "bore").target;
  const moved = await f.domain("point_gesture", { command: { basis: f.accepted.model.sourceDesignDigest, gesture_id: 29, target: old,
    viewport: { screen_size: [800, 600], model_center: [0, 0], pixels_per_model_unit: 10 }, samples: [{ sequence: 1, position: [4, 6] }] } });
  await f.record("alice", "move", moved);
  const item = moved.inventory.objects.find((item) => item.declaration === "bore"), target = f.host.current(item.object), deletion = f.host.planDelete([target]);
  await f.domain("mutation", { mutation: { mutation: "delete", target: { target: "declaration", declaration: "bore" } } });
  f.host.commitStage(f.host.stageValidatedTransaction({ basisRevision: 1, revision: 2, deletions: [deletion], record: { operation: operation("bob", "delete"), changes: [], structural: {
    deleted: [{ target, payload: item.payload, position: { previous: null, next: f.host.current("sketch.ts#other") } }] } } }));
  const restored = await f.undo("bob", "undo-delete"), fresh = restored.pointTargets.find((item) => item.target.address?.owner.address.declaration === "bore").target;
  assert.ok(fresh.address.owner.allocation > old.address.owner.allocation); assert.notEqual(f.host.current(item.object).generation, target.generation);
  const undone = await f.undo("alice", "undo-old-point");
  assert.deepEqual(undone.pointTargets.find((item) => item.target.address?.owner.address.declaration === "bore").position, [0, 0]);
  assert.deepEqual(undone.pointTargets.find((item) => item.target.address?.owner.address.declaration === "other").position, [30, 0]);
  const lens = JSON.parse(moved.propertyChanges[0].property)[1]; lens.codec.builder = ["geometry", "sketchPoint"];
  await assert.rejects(job({ kind: "point_properties", folder: f.folder, files: undone.candidateFiles, model: undone.model,
    writes: [{ address: lens, expected: null, value: null }] }), /replaced|absent|ambiguous/u);
});

test("canonical source shape changes undo only their owned keys and preserve a newer sibling", async (t) => {
  const f = await fixture(t);
  // A source Apply changes array membership; this records only changed nodes,
  // unlike an explicit whole-array canvas replacement that owns every member.
  const before = f.accepted;
  const withBranch = before.candidateFiles["sketch.ts"].replace('radius:mm(2),', 'radius:mm(2),role:"construction",');
  const added = await f.domain("apply", { capture: { acceptedBasis: { files: before.candidateFiles }, working: { files: { ...before.candidateFiles, "sketch.ts": withBranch } } } });
  assert.ok(added.propertyChanges.some((entry) => entry.property === '["source",[]]' && entry.before.state === "object" && entry.after.state === "object"));
  assert.ok(added.propertyChanges.some((entry) => entry.property === '["source",["role"]]' && entry.before.state === "absent"));
  await f.record("alice", "add-role", added);
  await f.values("bob", "radius", ["radius"], { kind: "unit", value: { unit: "mm", value: 8 } });
  const restored = await f.undo("alice", "undo-role");
  assert.doesNotMatch(restored.candidateFiles["sketch.ts"], /role:/u); assert.match(restored.candidateFiles["sketch.ts"], /mm\(8\)/u);
  const object = restored.inventory.objects.find((item) => item.declaration === "bore");
  assert.equal(object.payload.statement.arguments.fields.some((entry) => entry.name === "role"), false);
});

test("canonical whole source value and metadata edits share ownership without duplicate source fields", async (t) => {
  const f = await fixture(t);
  const label = { kind: "string", value: "Cooling port" };
  const mutation = { mutation: "set_metadata", target: { target: "declaration", declaration: "bore" }, property: "label", value: label };
  const first = await f.domain("mutation", { mutation }); await f.record("alice", "metadata", first);
  const value = first.inventory.properties.find((entry) => entry.declaration === "bore" && entry.path.length === 0).value;
  const same = await f.values("bob", "whole", [], value);
  assert.equal(same.propertyChanges.filter((entry) => entry.property === '["metadata","declaration","label"]').length, 1);
  assert.equal(same.propertyChanges.some((entry) => entry.property === '["source",["label"]]'), false);
  assert.throws(() => f.host.prepareUndo("alice"), /owns|overwrit/u);
  const restored = await f.undo("bob", "undo-whole"); assert.equal(restored.acceptedInput, same.acceptedInput);
});

test("same-value parameter presentation Apply owns metadata without claiming consumers", async (t) => {
  const folder = await mkdtemp(join(tmpdir(), "geosolve-domain-parameter-")); t.after(() => rm(folder, { recursive: true, force: true }));
  const text = source.replace('const bore=', 'const r=$.parameter("boreRadius",mm(2),{label:"Radius"});const bore=').replace('radius:mm(2)', 'radius:r');
  const files = { "geosolve.json": JSON.stringify({ format: "geosolve-folder-v2", entry: "sketch.ts", mode: "editable" }), "sketch.ts": text };
  const initial = await job({ kind: "initialize", folder, files });
  const captured = { ...files, "sketch.ts": text.replace('label:"Radius"', "label:'Radius'") };
  const applied = await job({ kind: "apply", folder, files, model: initial.model, capture: { acceptedBasis: { files }, working: { files: captured } } });
  assert.deepEqual(applied.requiredStableDeclarations, ["boreRadius"]);
  assert.equal(applied.propertyChanges.length, 1); assert.equal(applied.propertyChanges[0].object, "sketch.ts#boreRadius");
  assert.equal(applied.propertyChanges[0].property, '["metadata","parameter","label"]');
  assert.deepEqual(applied.propertyChanges[0].before, applied.propertyChanges[0].after);
});
