// SPDX-License-Identifier: GPL-3.0-or-later
import assert from "node:assert/strict";
import { test } from "node:test";
import { readFile } from "node:fs/promises";
import { createEngine } from "../dist/index.js";
import { applyManagedSketchMutation, compileManagedSource } from "../../geosolve-sketch-code/dist/src/managed.js";

const viewport = { screen_size: [800, 600], model_center: [0, 0], pixels_per_model_unit: 5 };
async function setup(t, name = "authoring-radius-2") {
  const compiled = JSON.parse(await readFile(new URL(`../../../crates/geosolve-sketch-engine/tests/fixtures/${name}.json`, import.meta.url), "utf8"));
  const engine = await createEngine(); t.after(() => engine.dispose());
  const project = engine.compileProject({ project: "replay", compiled, customFiles: {}, artifacts: {}, lock: { format: "geosolve-lock-v1", modules: {} } });
  const basis = engine.openEditableSession(project), latest = engine.openEditableSession(project);
  t.after(() => { basis.dispose(); latest.dispose(); });
  return { engine, basis, latest, compiled, project };
}
function construct(session, points, { inferred = false, tool = "segment" } = {}) {
  const draft = session.beginConstruction(tool, { expected: session.token, gestureId: 101, viewport });
  let sequence = 0;
  for (const position of points) draft.advance({ sequence: ++sequence, input: { event: "click", position, suppressed: !inferred, regularized: false } });
  if (tool === "polyline") draft.advance({ sequence: ++sequence, input: { event: "complete" } });
  return draft.finish();
}
function move(session, position, owner) {
  const target = session.pointGestureTargets().find(({ target }) => !owner || target.address?.owner.address.declaration === owner).target;
  const gesture = session.beginPointGesture(target, { expected: session.token, gestureId: 102, viewport });
  gesture.advance({ sequence: 1, position }); return gesture.finish().command;
}
function receipt(prepared) {
  const response = prepared.kind === "source" ? (() => {
    const compiled = compileManagedSource(prepared.request.candidateSource);
    return { baseSourceDigest: prepared.request.current.ir.source_digest, candidateSourceDigest: compiled.ir.source_digest, compiled };
  })() : applyManagedSketchMutation(prepared.request.current, prepared.request.ticket.mutation);
  return { ticketDigest: prepared.request.ticket.ticketDigest, ...response };
}
function insert(session, command) {
  const prepared = session.prepareConstruction(command, { expected: session.token });
  const candidate = session.resolveConstruction(prepared, receipt(prepared));
  assert.equal(session.applyConstructionCommit(candidate).status, "accepted");
}
function source(session, value) {
  const prepared = session.prepareAuthoring({ kind: "source", source: value }, { expected: session.token });
  assert.equal(session.applyAuthoring(prepared, receipt(prepared)).status, "accepted");
}
function valid(result) {
  assert.ok(result.validation.hard_residuals_validated && result.validation.all_active_features_current);
  assert.ok(result.geometry.points.every(({ position }) => position.every(Number.isFinite)));
}

test("latest construction authenticates original intent and commits distinct concurrent creations with current names", async (t) => {
  const { engine, basis, latest } = await setup(t);
  const first = construct(basis, [[40, 40], [60, 50]]);
  const second = construct(basis, [[-40, -40], [-30, -40]], { tool: "center_radius_circle" });
  insert(latest, first);
  assert.throws(() => latest.prepareConstruction(second, { expected: latest.token }), /basis/u);
  const before = latest.state;
  const prepared = latest.prepareConstructionReplay(basis, second, { expected: latest.token });
  assert.equal(latest.state, before);
  assert.equal(prepared.replay.requiredStableDeclarations.length, 0);
  assert.ok(prepared.replay.allocationMapping.every(({ provisional, persistent }) => provisional !== persistent));
  const candidate = latest.resolveConstruction(prepared, receipt(prepared)); valid(candidate.result);
  assert.equal(latest.applyConstructionCommit(candidate).status, "accepted");
  for (const point of [[40, 40], [60, 50], [-40, -40]]) assert.ok(latest.accepted.geometry.points.some(({ position }) => position.every((v, i) => Math.abs(v - point[i]) < 1e-8)));
  const reopened = engine.openEditableSession(latest.exportProject(), { design: latest.exportDesign() }); t.after(() => reopened.dispose());
  assert.equal(reopened.sourceDesignDigest(), latest.sourceDesignDigest());
  const accepted = latest.state;
  const forged = structuredClone(second); forged.expected_declarations[0].comments = ["forged"];
  assert.throws(() => latest.prepareConstructionReplay(basis, forged, { expected: latest.token }), /semantic|intent/u);
  assert.equal(latest.state, accepted);
});

test("latest construction preserves inferred external operands and refuses new snap targets", async (t) => {
  const { basis, latest } = await setup(t);
  const intent = construct(basis, [[0, 0], [20, 0.1]], { inferred: true });
  insert(latest, construct(basis, [[-40, -40], [-20, -30]]));
  const prepared = latest.prepareConstructionReplay(basis, intent, { expected: latest.token });
  assert.deepEqual(prepared.replay.requiredStableDeclarations, ["bore"]);
  const compiled = receipt(prepared);
  assert.match(compiled.compiled.normalizedSource, /start: bore\.center/u);
  const candidate = latest.resolveConstruction(prepared, compiled); valid(candidate.result);
  latest.releaseConstruction(candidate);
  const moveCommand = move(latest, [30, 30], "bore");
  const moved = latest.preparePointGestureCommit(moveCommand, { expected: latest.token });
  assert.equal(latest.applyPointGestureCommit(moved).status, "accepted");
  insert(latest, construct(latest, [[0, 0], [2, 0]], { tool: "center_radius_circle" }));
  const before = latest.state;
  assert.throws(() => latest.prepareConstructionReplay(basis, intent, { expected: latest.token }), /semantic|operand|count/u);
  assert.equal(latest.state, before);
});

test("latest point replay keeps concurrent radius and point writes with original identity authentication", async (t) => {
  const { basis, latest } = await setup(t);
  const command = move(basis, [12, 7]);
  const prepared = latest.prepareAuthoring({ kind: "values", writes: [{ declaration: "bore", path: ["radius"], value: { kind: "unit", value: { unit: "mm", value: 5 } } }] }, { expected: latest.token });
  assert.equal(latest.applyAuthoring(prepared, receipt(prepared)).status, "accepted");
  const first = latest.preparePointGestureCommit(move(latest, [3, 4]), { expected: latest.token });
  assert.equal(latest.applyPointGestureCommit(first).status, "accepted");
  const candidate = latest.preparePointGestureReplay(basis, command, { expected: latest.token });
  assert.deepEqual(candidate.replay.requiredStableDeclarations, ["bore"]); valid(candidate.result);
  assert.equal(latest.applyPointGestureCommit(candidate).status, "accepted");
  assert.ok(latest.pointGestureTargets()[0].position.every((v, i) => Math.abs(v - [12, 7][i]) < 1e-8));
  assert.ok(latest.accepted.geometry.scalars.some(({ value }) => Math.abs(value - 5) < 1e-8));
  const before = latest.state, forged = structuredClone(command); forged.target.address.owner.generation++;
  assert.throws(() => latest.preparePointGestureReplay(basis, forged, { expected: latest.token }), /stale|absent/u);
  assert.equal(latest.state, before);
});

test("latest point replay rejects producer-consumer codec changes and connected explicit branch changes", async (t) => {
  const shared = await setup(t, "point-gesture-shared");
  const command = move(shared.basis, [12, 7], "consumer");
  source(shared.latest, shared.compiled.normalizedSource.replace("center: producer.center", "center: [0, 0]"));
  const before = shared.latest.state;
  assert.throws(() => shared.latest.preparePointGestureReplay(shared.basis, command, { expected: shared.latest.token }), /codec|detachment/u);
  assert.equal(shared.latest.state, before);
  const bar = await setup(t, "point-gesture-constrained");
  const explicit = bar.compiled.normalizedSource.replace("end: [20, 0],", "end: [20, 0], branchDirection: [1, 0],");
  source(bar.basis, explicit); source(bar.latest, explicit);
  const barCommand = move(bar.basis, [4, 4]);
  const reversed = explicit.replace("branchDirection: [1, 0]", "branchDirection: [-1, 0]");
  assert.notEqual(reversed, explicit);
  source(bar.latest, reversed);
  const accepted = bar.latest.state;
  assert.throws(() => bar.latest.preparePointGestureReplay(bar.basis, barCommand, { expected: bar.latest.token }), /branch|codec/u);
  assert.equal(bar.latest.state, accepted);
});

test("latest point replay carries named scalar dependencies across value edits and rejects changing their meaning", async t => {
  const {basis,latest,compiled}=await setup(t,"point-gesture-parameter");
  const command=move(basis,[8,4],"bore");
  const edit=latest.prepareAuthoring({kind:"values",writes:[{declaration:"boreRadius",path:[],value:{kind:"unit",value:{unit:"mm",value:5}}}]},{expected:latest.token});
  assert.equal(latest.applyAuthoring(edit,receipt(edit)).status,"accepted");
  const candidate=latest.preparePointGestureReplay(basis,command,{expected:latest.token});
  assert.deepEqual(candidate.replay.requiredStableDeclarations,["bore","boreRadius"]);valid(candidate.result);
  assert.equal(latest.applyPointGestureCommit(candidate).status,"accepted");
  assert.deepEqual(latest.pointGestureTargets()[0].position,[8,4]);
  assert.ok(latest.accepted.geometry.scalars.some(({value})=>Math.abs(value-5)<1e-8));
  const materialized=JSON.parse(latest.exportProject()).managed.compiled.normalizedSource;
  assert.match(materialized,/const width = \$\.parameter\("boreRadius"/u);
  assert.match(materialized,/const port = \$\.geometry.centerRadiusCircle\("bore"/u);
  // Keeping the same numerical radius does not authorize replacing a shared
  // parameter reference with a local literal or changing the parameter's unit.
  source(latest,compiled.normalizedSource.replace("radius: width","radius: mm(2)"));
  const before=latest.state;
  assert.throws(()=>latest.preparePointGestureReplay(basis,command,{expected:latest.token}),/codec|operand/u);
  assert.equal(latest.state,before);
});
