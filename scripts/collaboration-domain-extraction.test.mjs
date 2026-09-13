// SPDX-License-Identifier: GPL-3.0-or-later
// Actual native allocation and pinned compiler source edits; no mocked geometry.
import assert from "node:assert/strict";
import { test } from "node:test";
import { createEngine } from "../packages/geosolve-engine/dist/index.js";
import { compileManagedSource, applyManagedSketchMutation, applyManagedSketchSourceMutation } from "../packages/geosolve-sketch-code/dist/src/managed.js";
import { prepareStructuralSource } from "../packages/geosolve-cli/runtime/collaboration-domain-structure.mjs";

const source = `"use geosolve sketch";
import { sketch, mm } from "@geosolve/sketch-code";
// Keep the author's spacing and note.
export default sketch(($) => {
  const parameter1 = mm(99);
  const occupied = $.parameter("parameter2", mm(88));
  const bore = $.geometry.centerRadiusCircle("bore", { center: [0, 0], radius: mm(2) });
  const second = $.geometry.centerRadiusCircle("second", { center: [10, 0], radius: mm(5) });
  return { bore, second };
});
`;
test("structural source inverse detaches a parameter consumer before removing its binding", () => {
  const current = applyManagedSketchMutation(compileManagedSource(source), { mutation: "extract_parameter", declaration: "bore", path: ["radius"], symbol: "radius", variable: "width" }).compiled;
  const target = name => ({ object: `sketch.ts#${name}`, generation: 1 });
  const inverse = { structural: { delete: { roots: [target("radius")], closure: [target("radius")] } }, changes: [{
    address: { target: target("bore"), property: '["radius"]' },
    before: { kind: "reference", value: { declaration: "width", path: [] } },
    after: { kind: "unit", value: { unit: "mm", value: 2 } },
  }] };
  const input = { entry: "sketch.ts", compiled: current, patches: {} }, before = JSON.stringify(input);
  const restored = prepareStructuralSource(input, current.normalizedSource, inverse);
  assert.equal(restored.compiled.canonicalArtifactJson, compileManagedSource(source).canonicalArtifactJson);
  assert.equal(restored.compiled.canonicalIrJson, compileManagedSource(source).canonicalIrJson);
  const forged = structuredClone(inverse); forged.changes[0].before.value.declaration = "occupied";
  assert.throws(() => prepareStructuralSource(input, current.normalizedSource, forged), /expected|stale/u);
  assert.equal(JSON.stringify(input), before);
});
const action = (declaration, path = ["radius"]) => ({ kind: "extract_parameter", declaration, path,
  presentation: { label: "Radius", description: "Shared design intent", isKeyParameter: true } });
const projectValue = (session) => JSON.parse(session.exportProject());
function accepted(session) {
  assert.equal(session.accepted.validation.hard_residuals_validated, true);
  assert.equal(session.accepted.validation.all_active_features_current, true);
  const residual = session.accepted.validation.maximum_normalized_hard_residual;
  assert.ok(Number.isFinite(residual) && residual <= 1e-9);
  assert.ok(session.accepted.geometry.points.every(point => point.position.every(Number.isFinite)));
  assert.ok(session.accepted.geometry.scalars.every(scalar => Number.isFinite(scalar.value)));
}
function geometryByOwner(geometry) {
  // A cold native session allocates fresh local IDs. Preserve the complete
  // geometry and incidence graph, resolving those IDs through unique owners.
  const entities = [...geometry.points, ...geometry.scalars, ...geometry.curves.map(entry => entry.curve)];
  const owners = new Map(entities.map(entity => [entity.id, entity.label]));
  assert.equal(owners.size, entities.length);
  assert.equal(new Set(owners.values()).size, entities.length);
  return JSON.parse(JSON.stringify(geometry, (_key, value) => typeof value === "string" && owners.has(value) ? owners.get(value) : value));
}
function publish(session, prepared, raw) {
  const exact = applyManagedSketchMutation(prepared.request.current, prepared.request.ticket.mutation);
  const localized = applyManagedSketchSourceMutation(prepared.request.current, prepared.request.ticket.mutation, { source: raw });
  assert.equal(localized.compiled.canonicalIrJson, exact.compiled.canonicalIrJson);
  assert.equal(localized.compiled.canonicalArtifactJson, exact.compiled.canonicalArtifactJson);
  const update = session.applyAuthoring(prepared, { ticketDigest: prepared.request.ticket.ticketDigest, ...exact });
  assert.equal(update.status, "accepted", JSON.stringify(update));
  accepted(session);
  return localized.source;
}

test("native extraction allocates on latest source, preserves geometry and lexical bindings, and survives reopen", async t => {
  const engine = await createEngine(); t.after(() => engine.dispose());
  const project = engine.compileProject({ project: "collaboration-extraction", compiled: compileManagedSource(source), customFiles: {}, artifacts: {}, lock: { format: "geosolve-lock-v1", modules: {} } });
  const session = engine.openEditableSession(project); t.after(() => session.dispose());
  const other = engine.openEditableSession(project); t.after(() => other.dispose());
  const baseline = structuredClone(session.accepted.geometry), before = session.state;
  const first = session.prepareAuthoring(action("bore"), { expected: session.token });
  const staleOther = other.prepareAuthoring(action("second"), { expected: other.token });
  assert.equal(first.request.ticket.mutation.symbol, "parameter3");
  assert.equal(staleOther.request.ticket.mutation.symbol, "parameter3");
  assert.equal(session.state, before, "describing and allocating cannot publish");
  let raw = publish(session, first, source);
  assert.deepEqual(session.accepted.geometry, baseline);
  assert.equal(projectValue(session).managed.declaration_name_high_water, 3);
  assert.ok(raw.includes("// Keep the author's spacing and note."));
  assert.match(raw, /radius: parameter3/u);
  assert.match(raw, /isKeyParameter: true/u);
  assert.throws(() => session.prepareAuthoring({ ...action("second"), symbol: "parameter3", variable: "parameter3" }, { expected: session.token }), /unknown field/u);
  const second = session.prepareAuthoring(action("second"), { expected: session.token });
  assert.equal(second.request.ticket.mutation.symbol, "parameter4", "server reallocates after the concurrent winner");
  raw = publish(session, second, raw);
  assert.equal(projectValue(session).managed.declaration_name_high_water, 4);
  assert.deepEqual(session.accepted.geometry, baseline);
  const binding = session.prepareAuthoring(action("parameter1", []), { expected: session.token });
  assert.equal(binding.request.ticket.mutation.variable, "parameter1");
  assert.equal(binding.request.ticket.mutation.symbol, "parameter1");
  raw = publish(session, binding, raw);
  assert.equal(projectValue(session).managed.declaration_name_high_water, 4);
  assert.match(raw, /const parameter1 = \$\.parameter\("parameter1", mm\(99\)/u);
  const after = session.state;
  assert.throws(() => session.prepareAuthoring(action("bore"), { expected: session.token }), /literal|reference/u);
  assert.throws(() => session.prepareAuthoring(action("missing"), { expected: session.token }), /absent/u);
  assert.equal(session.state, after);
  const reopened = engine.openEditableSession(session.exportProject(), { design: session.exportDesign() }); t.after(() => reopened.dispose());
  assert.equal(reopened.sourceDesignDigest(), session.sourceDesignDigest());
  assert.equal(projectValue(reopened).managed.declaration_name_high_water, 4);
  assert.deepEqual(geometryByOwner(reopened.accepted.geometry), geometryByOwner(baseline));
  accepted(reopened);
});
