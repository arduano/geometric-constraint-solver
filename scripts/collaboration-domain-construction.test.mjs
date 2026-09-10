// SPDX-License-Identifier: GPL-3.0-or-later
import assert from "node:assert/strict";
import { test } from "node:test";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { createEngine } from "../packages/geosolve-engine/dist/index.js";
import { runCollaborationDomainJob as job } from "./collaboration-domain.mjs";
const viewport = { screen_size: [800, 600], model_center: [0, 0], pixels_per_model_unit: 5 };
test("domain replays genuine local construction and preserves native source receipts and monotonic names", async (t) => {
  const folder = await mkdtemp(join(tmpdir(), "geosolve-domain-construction-")); t.after(() => rm(folder, { recursive: true, force: true }));
  const files = { "geosolve.json": JSON.stringify({ format: "geosolve-folder-v2", entry: "sketch.ts", mode: "editable" }),
    "sketch.ts": '"use geosolve sketch";\nimport {sketch,mm} from "@geosolve/sketch-code";\n// Retain source comment.\nexport default sketch(($)=>{const bore=$.geometry.centerRadiusCircle("bore",{center:[0,0],radius:mm(2)});return {bore};});\n' };
  const initial = await job({ kind: "initialize", folder, files });
  const engine = await createEngine(); t.after(() => engine.dispose());
  const local = engine.openEditableSession(initial.model.project, { design: initial.model.design }); t.after(() => local.dispose());
  const prediction = local.beginConstruction("segment", { expected: local.token, gestureId: 19, viewport });
  let sequence = 0;
  for (const position of [[0, 0], [20, 0.1]]) for (const event of ["move", "click"]) {
    const frame = prediction.advance({ sequence: ++sequence, input: { event, position, suppressed: false, regularized: false } }); assert.equal(frame.diagnostic, null);
  }
  const command = prediction.finish();
  const added = await job({ kind: "construction", folder, files, model: initial.model, command });
  assert.ok(added.createdDeclarations.length >= 2); assert.match(added.candidateFiles["sketch.ts"], /start: bore\.center/u);
  assert.match(added.candidateFiles["sketch.ts"], /constraint\.horizontal/u); assert.match(added.candidateFiles["sketch.ts"], /Retain source comment/u);
  assert.equal(added.result.validation.hard_residuals_validated, true);
  const points = added.result.geometry.points.map(({ position }) => position);
  assert.equal(points.length, 2); assert.ok(points.some(([x, y]) => Math.abs(x - 20) < 1e-9 && Math.abs(y) < 1e-9));
  const rebuilt = await job({ kind: "rebuild", folder, files: added.candidateFiles, model: added.model });
  assert.equal(rebuilt.acceptedInput, added.acceptedInput); assert.ok(JSON.parse(added.model.project).managed.declaration_name_high_water > 0);
  await assert.rejects(job({ kind: "construction", folder, files: added.candidateFiles, model: added.model, command }), /basis/u);
});
