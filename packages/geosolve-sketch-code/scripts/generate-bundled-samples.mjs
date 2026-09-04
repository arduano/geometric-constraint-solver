// SPDX-License-Identifier: GPL-3.0-or-later

import assert from "node:assert/strict";
import { readdir, readFile, writeFile } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { compileManagedSource } from "../dist/src/managed.js";

const packageRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const sampleRoot = resolve(
  packageRoot,
  "../../crates/geosolve-sketch-code/assets/bundled-samples",
);
const check = process.argv.includes("--check");
const write = process.argv.includes("--write");

if (check === write) {
  throw new TypeError(
    "usage: generate-bundled-samples.mjs (--check|--write)",
  );
}

async function patchPlans(directory) {
  const patchDirectory = join(directory, "patches");
  let entries;
  try {
    entries = await readdir(patchDirectory, { withFileTypes: true });
  } catch (error) {
    if (error?.code === "ENOENT") return {};
    throw error;
  }
  const plans = {};
  for (const entry of entries) {
    if (!entry.isFile() || !entry.name.endsWith(".artifact.json")) continue;
    const artifact = JSON.parse(
      await readFile(join(patchDirectory, entry.name), "utf8"),
    );
    assert.equal(typeof artifact.export_name, "string");
    assert.equal(plans[artifact.export_name], undefined);
    plans[artifact.export_name] = artifact;
  }
  return plans;
}

const directories = [];
for (const entry of await readdir(sampleRoot, { withFileTypes: true })) {
  if (!entry.isDirectory()) continue;
  const directory = join(sampleRoot, entry.name);
  const manifest = JSON.parse(
    await readFile(join(directory, "manifest.json"), "utf8"),
  );
  directories.push({ directory, key: entry.name, ordinal: manifest.ordinal });
}
directories.sort((left, right) => left.ordinal - right.ordinal);
assert.equal(directories.length, 20);

for (const { directory, key } of directories) {
  const sourcePath = join(directory, "sketch.ts");
  const compiledPath = join(directory, "sketch.compiled.json");
  const source = await readFile(sourcePath, "utf8");
  const patches = await patchPlans(directory);
  const options = Object.keys(patches).length === 0 ? {} : { patches };
  const first = compileManagedSource(source, options);
  const compiled = first.normalizedSource === source
    ? first
    : compileManagedSource(first.normalizedSource, options);
  const envelope = JSON.stringify(compiled);

  if (check) {
    assert.equal(
      source,
      compiled.normalizedSource,
      `${key}/sketch.ts is not canonical; run the bundled-sample generator`,
    );
    assert.equal(
      await readFile(compiledPath, "utf8"),
      envelope,
      `${key}/sketch.compiled.json is stale; run the bundled-sample generator`,
    );
  } else {
    await writeFile(sourcePath, compiled.normalizedSource);
    await writeFile(compiledPath, envelope);
  }
}
