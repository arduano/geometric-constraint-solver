// SPDX-License-Identifier: GPL-3.0-or-later

import assert from "node:assert/strict";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { crossBrace } from "../dist/examples/braced-frame.patch.js";
import { mountingPlate } from "../dist/examples/mounting-plate.patch.js";
import { roundEveryCorner } from "../dist/examples/rounded-polyline.patch.js";
import { fillets } from "../dist/examples/typed-panel.patch.js";
import { compilePatchArtifact } from "../dist/src/compiler.js";

const packageRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const check = process.argv.includes("--check");
const fixtures = [
  {
    source: "examples/rounded-polyline.patch.ts",
    fixture: "test/fixtures/round-every-corner.artifact.json",
    moduleSpecifier: "./patches/round-every-corner.patch.ts",
    exportName: "roundEveryCorner",
    patch: roundEveryCorner,
  },
  {
    source: "examples/typed-panel.patch.ts",
    fixture: "test/fixtures/fillet-record.artifact.json",
    moduleSpecifier: "./patches/fillet-record.patch.ts",
    exportName: "fillets",
    patch: fillets,
  },
  {
    source: "examples/braced-frame.patch.ts",
    fixture: "test/fixtures/cross-brace.artifact.json",
    moduleSpecifier: "./patches/cross-brace.patch.ts",
    exportName: "crossBrace",
    patch: crossBrace,
  },
  {
    source: "examples/mounting-plate.patch.ts",
    fixture: "test/fixtures/mounting-plate.artifact.json",
    moduleSpecifier: "./patches/mounting-plate.patch.ts",
    exportName: "mountingPlate",
    patch: mountingPlate,
  },
];

for (const fixture of fixtures) {
  const source = await readFile(resolve(packageRoot, fixture.source));
  const compiled = compilePatchArtifact({
    source,
    moduleSpecifier: fixture.moduleSpecifier,
    exportName: fixture.exportName,
    patch: fixture.patch,
  });
  const destination = resolve(packageRoot, fixture.fixture);
  if (check) {
    assert.equal(
      await readFile(destination, "utf8"),
      compiled.canonicalJson,
      `${fixture.fixture} is stale; run npm run generate:fixtures`,
    );
  } else {
    await mkdir(dirname(destination), { recursive: true });
    await writeFile(destination, compiled.canonicalJson);
  }
}
