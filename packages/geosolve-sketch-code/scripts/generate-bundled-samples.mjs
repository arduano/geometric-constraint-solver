// SPDX-License-Identifier: GPL-3.0-or-later

import assert from "node:assert/strict";
import { readdir, readFile, writeFile } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { deflateSync, inflateSync } from "node:zlib";
import { manifoldPatches } from "./manifold-patches.mjs";

import {
  applyManagedSketchMutation,
  compileManagedSource,
} from "../dist/src/managed.js";

const packageRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const sampleRoot = resolve(
  packageRoot,
  "../../crates/geosolve-sketch-code/assets/bundled-samples",
);
const demoFixtureRoot = resolve(
  packageRoot,
  "../../crates/geosolve-demo-web/tests/fixtures",
);
const check = process.argv.includes("--check");
const write = process.argv.includes("--write");

const releaseWasmScaleEdits = new Map([
  [
    "perforated-fixture-field",
    {
      declaration: "northernCells",
      path: ["pilotRadius"],
      expected: { unit: "mm", value: 2.5 },
    },
  ],
  [
    "robotic-harness-backplane",
    {
      declaration: "bendRadius",
      path: [],
      expected: { unit: "mm", value: 5 },
    },
  ],
]);

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
const catalog = JSON.parse(await readFile(join(sampleRoot, "../bundled-sample-catalog.json"), "utf8"));
assert.equal(catalog.schema, 1);
assert.deepEqual(directories.map(({ key }) => key), catalog.samples.map(({ key }) => key));
assert.deepEqual(directories.map(({ ordinal }) => ordinal), catalog.samples.map((_, index) => index + 1));
assert.equal(new Set(catalog.samples.map(({ key }) => key)).size, catalog.samples.length);
assert.equal(catalog.retired_keys.some((key) => directories.some((entry) => entry.key === key)), false);

for (const { directory, key } of directories) {
  if (key === "pc-water-manifold") await manifoldPatches(directory, check);
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

  const scaleEdit = releaseWasmScaleEdits.get(key);
  let editedEnvelope;
  if (scaleEdit !== undefined) {
    const witnesses = JSON.parse(
      await readFile(join(directory, "witnesses.json"), "utf8"),
    );
    const edit = witnesses.representative_edit;
    assert.equal(edit.declaration, scaleEdit.declaration);
    assert.deepEqual(edit.path, scaleEdit.path);
    assert.equal(edit.replacement?.unit, scaleEdit.expected.unit);
    assert.equal(typeof edit.replacement?.value, "number");
    const receipt = applyManagedSketchMutation(
      compiled,
      {
        mutation: "set_values",
        values: [{
          declaration: scaleEdit.declaration,
          path: scaleEdit.path,
          expected: { kind: "unit", value: scaleEdit.expected },
          value: { kind: "unit", value: edit.replacement },
        }],
      },
      options,
    );
    assert.equal(receipt.baseSourceDigest, compiled.ir.source_digest);
    assert.equal(
      receipt.candidateSourceDigest,
      receipt.compiled.ir.source_digest,
    );
    assert.notEqual(receipt.compiled.normalizedSource, compiled.normalizedSource);
    editedEnvelope = JSON.stringify(receipt.compiled);
  }

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

  if (editedEnvelope !== undefined) {
    const editedPath = join(
      demoFixtureRoot,
      `m92-${key}-edit.compiled.json.zlib`,
    );
    if (check) {
      assert.equal(
        inflateSync(await readFile(editedPath)).toString("utf8"),
        editedEnvelope,
        `${key} release-WASM edit fixture is stale; run the bundled-sample generator`,
      );
    } else {
      await writeFile(
        editedPath,
        deflateSync(Buffer.from(editedEnvelope), { level: 9 }),
      );
    }
  }
}
