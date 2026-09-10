// SPDX-License-Identifier: GPL-3.0-or-later
import assert from "node:assert/strict";
import { test } from "node:test";
import { readFile } from "node:fs/promises";
import { createEngine } from "../dist/index.js";
import { applyManagedSketchMutation, compileManagedSource } from "../../geosolve-sketch-code/dist/src/managed.js";

async function fixture(t) {
  const compiled = JSON.parse(await readFile(new URL("../../../crates/geosolve-sketch-engine/tests/fixtures/authoring-radius-2.json", import.meta.url), "utf8"));
  const engine = await createEngine(); t.after(() => engine.dispose());
  const project = engine.compileProject({ project: "wasm-authoring", compiled, customFiles: {}, artifacts: {}, lock: { format: "geosolve-lock-v1", modules: {} } });
  const session = engine.openEditableSession(project); t.after(() => session.dispose());
  return { engine, session };
}
function write(value) { return { kind: "values", writes: [{ declaration: "bore", path: ["radius"], value: { kind: "unit", value: { unit: "mm", value } } }] }; }
function compile(prepared) {
  const receipt = prepared.kind === "source"
    ? (() => { const compiled = compileManagedSource(prepared.request.candidateSource); return { baseSourceDigest: prepared.request.current.ir.source_digest, candidateSourceDigest: compiled.ir.source_digest, compiled }; })()
    : applyManagedSketchMutation(prepared.request.current, prepared.request.ticket.mutation);
  return { ticketDigest: prepared.request.ticket.ticketDigest, ...receipt };
}

test("actual WASM prepares latest-value edits, authenticates receipts and returns checked contribution values", async (t) => {
  const { session } = await fixture(t);
  const before = session.state;
  const prepared = session.prepareAuthoring(write(5), { expected: session.token });
  assert.equal(session.state, before);
  assert.throws(() => session.applyAuthoring(structuredClone(prepared), compile(prepared)), /foreign/u);
  const forged = compile(prepared); forged.ticketDigest = "forged";
  assert.equal(session.applyAuthoring(prepared, forged).status, "rejected"); assert.equal(session.state, before);
  const result = session.applyAuthoring(prepared, compile(prepared));
  assert.equal(result.status, "accepted"); assert.equal(session.accepted.geometry.scalars[0].value, 5);
  assert.equal(result.valueChanges[0].before.value.value, 2);
  assert.equal(result.valueChanges[0].write.value.value.value, 5);
  assert.ok(session.accepted.validation.hard_residuals_validated);
  assert.throws(() => session.applyAuthoring(prepared, compile(prepared)), /consumed/u);
  assert.throws(() => session.prepareAuthoring(write(7), { expected: before.token }), /stale/u);
  const next = session.prepareAuthoring(write(7), { expected: session.token });
  const accepted = session.applyAuthoring(next, compile(next));
  assert.equal(accepted.valueChanges[0].before.value.value, 5);
});

test("actual WASM explicit source capture and accepted export cold-rebuild exact durable input", async (t) => {
  const { engine, session } = await fixture(t);
  const compiled = JSON.parse(await readFile(new URL("../../../crates/geosolve-sketch-engine/tests/fixtures/authoring-radius-5.json", import.meta.url), "utf8"));
  const capture = session.prepareAuthoring({ kind: "source", source: compiled.normalizedSource }, { expected: session.token });
  const result = session.applyAuthoring(capture, compile(capture)); assert.equal(result.status, "accepted");
  const digest = session.sourceDesignDigest();
  const project = session.exportProject(), design = session.exportDesign();
  const reopened = engine.openEditableSession(project, { design }); t.after(() => reopened.dispose());
  assert.equal(reopened.sourceDesignDigest(), digest);
  assert.equal(reopened.accepted.geometry.scalars[0].value, 5);
});

test("actual WASM foreign session and released preparation cannot publish", async (t) => {
  const { engine, session } = await fixture(t);
  const other = engine.openEditableSession(session.exportProject()); t.after(() => other.dispose());
  const prepared = session.prepareAuthoring(write(5), { expected: session.token });
  assert.throws(() => other.applyAuthoring(prepared, compile(prepared)), /foreign/u);
  session.releaseAuthoring(prepared);
  assert.throws(() => session.applyAuthoring(prepared, compile(prepared)), /released/u);
  assert.equal(session.accepted.geometry.scalars[0].value, 2);
});
