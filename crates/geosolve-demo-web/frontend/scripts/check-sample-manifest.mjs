// SPDX-License-Identifier: GPL-3.0-or-later
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";

const samples = JSON.parse(await readFile(new URL("../src/data/samples.json", import.meta.url), "utf8"));
const commands = JSON.parse(await readFile(new URL("../src/data/commands.json", import.meta.url), "utf8"));
const registrySource = await readFile(new URL("../../../geosolve-sketch-code/src/bundled_samples.rs", import.meta.url), "utf8");
const geometrySource = await readFile(new URL("../../../geosolve-constraint-editor/src/geometry_tools.rs", import.meta.url), "utf8");
const paletteSource = await readFile(new URL("../../src/workbench/geometry_palette.rs", import.meta.url), "utf8");
const actionSource = await readFile(new URL("../../../geosolve-constraint-editor/src/authoring_catalog.rs", import.meta.url), "utf8");
const manifestSource = await readFile(new URL("../../src/workbench/command_manifest.rs", import.meta.url), "utf8");

const reviewed = JSON.parse(await readFile(new URL("../../../geosolve-sketch-code/assets/bundled-sample-catalog.json", import.meta.url), "utf8"));
assert.equal(reviewed.schema, 1, "unknown reviewed sample catalog schema");
assert.ok(Array.isArray(reviewed.samples) && reviewed.samples.length > 0, "reviewed sample catalog is nonempty");
assert.deepEqual(samples.map(({ key, title, category }) => ({ key, title, category })), reviewed.samples,
  "frontend catalog must match independent reviewed keys/order/titles/categories");
assert.deepEqual(samples.map(({ ordinal }) => ordinal), reviewed.samples.map((_, index) => index + 1),
  "sample ordinals must be contiguous in reviewed order");
assert.equal(new Set(reviewed.retired_keys).size, reviewed.retired_keys.length, "reviewed retired keys must be unique");
for (const retired of reviewed.retired_keys) {
  assert.ok(!samples.some(({ key }) => key === retired), `retired sample remains in frontend catalog: ${retired}`);
}
const identities = new Set(samples.map(({ stableId }) => stableId));
if (identities.size !== samples.length) throw new Error("sample stable IDs must be unique");
const sampleKeys = new Set(samples.map(({ key }) => key));
if (sampleKeys.size !== samples.length) throw new Error("sample keys must be unique");
for (const sample of samples) {
  if (sample.stableId !== `sample.${sample.key}`) {
    throw new Error(`sample identity is not derived from its canonical key: ${sample.stableId}`);
  }
  if (![sample.key, sample.title, sample.group].every((value) => typeof value === "string" && value.trim())) {
    throw new Error(`sample metadata is incomplete: ${sample.stableId}`);
  }
}

if (!registrySource.includes("bundled_sample_catalog") || !registrySource.includes("BundledSampleSpec")) throw new Error("Rust canonical registry surface is unavailable");
if (commands.geometry.length !== 25 || new Set(commands.geometry.map(({ group }) => group)).size !== 9 || commands.constraints.length !== 13 || commands.dimensions.length !== 5 || commands.modify.length !== 2 || commands.context.length !== 1 || commands.canvas.length !== 3) {
  throw new Error("primary tool inventory must remain 25 geometry / 9 families / 13 relations / 5 dimensions");
}
for (const [category, entries] of Object.entries(commands)) {
  if (new Set(entries.map(({ id }) => id)).size !== entries.length) {
    throw new Error(`${category} command IDs must be unique`);
  }
  for (const entry of entries) {
    if (![entry.id, entry.label].every((value) => typeof value === "string" && value.trim())) {
      throw new Error(`${category} contains incomplete command metadata`);
    }
  }
}
for (const entry of commands.geometry) {
  if (!geometrySource.includes(`"${entry.id}"`) || !paletteSource.includes(`"${entry.label}"`)) throw new Error(`geometry command drift: ${entry.id}`);
}
for (const entry of [...commands.constraints, ...commands.dimensions]) {
  if (!actionSource.includes(`"${entry.id}"`) || !actionSource.includes(`"${entry.label}"`)) throw new Error(`authoring command drift: ${entry.id}`);
}
for (const entry of [...commands.modify, ...commands.context, ...commands.canvas]) {
  const keyFound = actionSource.includes(`"${entry.id}"`) || manifestSource.includes(`"${entry.id}"`);
  if (!keyFound || !(manifestSource.includes(`"${entry.label}"`) || actionSource.includes(`"${entry.label}"`))) throw new Error(`feature/display command drift: ${entry.id}`);
}
console.log(`frontend manifests match reviewed Rust authority: ${reviewed.samples.length} canonical source-authoritative samples + complete primary command inventory`);
