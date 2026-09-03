// SPDX-License-Identifier: GPL-3.0-or-later
import { readFile } from "node:fs/promises";

const samples = JSON.parse(await readFile(new URL("../src/data/samples.json", import.meta.url), "utf8"));
const commands = JSON.parse(await readFile(new URL("../src/data/commands.json", import.meta.url), "utf8"));
const nativeSource = await readFile(new URL("../../src/workbench/samples.rs", import.meta.url), "utf8");
const codeSource = await readFile(new URL("../../../geosolve-sketch-code/src/demos.rs", import.meta.url), "utf8");
const geometrySource = await readFile(new URL("../../../geosolve-constraint-editor/src/geometry_tools.rs", import.meta.url), "utf8");
const paletteSource = await readFile(new URL("../../src/workbench/geometry_palette.rs", import.meta.url), "utf8");
const actionSource = await readFile(new URL("../../src/workbench/action_surface.rs", import.meta.url), "utf8");
const manifestSource = await readFile(new URL("../../src/workbench/command_manifest.rs", import.meta.url), "utf8");

const native = samples.filter(({ kind }) => kind === "native");
const code = samples.filter(({ kind }) => kind === "code");
if (samples.length !== 37 || native.length !== 25 || code.length !== 12) {
  throw new Error(`sample inventory must remain 25 native + 12 code; received ${native.length} + ${code.length}`);
}
const identities = new Set(samples.map(({ stableId }) => stableId));
if (identities.size !== samples.length) throw new Error("sample stable IDs must be unique");
const sampleKeys = new Set(samples.map(({ key }) => key));
if (sampleKeys.size !== samples.length) throw new Error("sample keys must be unique");
for (const sample of samples) {
  if (sample.stableId !== `sample.${sample.kind}.${sample.key}`) {
    throw new Error(`sample identity is not derived from kind and key: ${sample.stableId}`);
  }
  if (![sample.key, sample.title, sample.group].every((value) => typeof value === "string" && value.trim())) {
    throw new Error(`sample metadata is incomplete: ${sample.stableId}`);
  }
}

for (const sample of native) {
  if (!nativeSource.includes(`"${sample.key}"`) || !nativeSource.includes(`"${sample.title}"`)) {
    throw new Error(`native sample drift: ${sample.stableId}`);
  }
}
for (const sample of code) {
  if (!codeSource.includes(`"${sample.key}"`) || !codeSource.includes(`"${sample.title}"`)) {
    throw new Error(`code sample drift: ${sample.stableId}`);
  }
}
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
console.log("frontend manifests match Rust authority: 37 samples + complete primary command inventory");
