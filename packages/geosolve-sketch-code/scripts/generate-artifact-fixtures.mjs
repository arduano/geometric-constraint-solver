// SPDX-License-Identifier: GPL-3.0-or-later

import assert from "node:assert/strict";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { crossBrace } from "../dist/examples/braced-frame.patch.js";
import { adaptiveLanterns } from "../dist/examples/adaptive-lanterns.patch.js";
import { bridgeCables } from "../dist/examples/bridge-cables.patch.js";
import { compassCore } from "../dist/examples/compass-core.patch.js";
import { cornerReliefs } from "../dist/examples/corner-reliefs.patch.js";
import { harnessRoute } from "../dist/examples/harness-route.patch.js";
import { mountingPlate } from "../dist/examples/mounting-plate.patch.js";
import { roundEveryCorner } from "../dist/examples/rounded-polyline.patch.js";
import { fillets } from "../dist/examples/typed-panel.patch.js";
import { waterChannel } from "../dist/examples/water-channel.patch.js";
import { compilePatchArtifact } from "../dist/src/compiler.js";

const packageRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const check = process.argv.includes("--check");
const fixtures = [
  {
    source: "examples/adaptive-lanterns.patch.ts",
    fixture: "test/fixtures/adaptive-lanterns.artifact.json",
    moduleSpecifier: "./patches/adaptive-lanterns.patch.ts",
    exportName: "adaptiveLanterns",
    patch: adaptiveLanterns,
  },
  {
    source: "examples/bridge-cables.patch.ts",
    fixture: "test/fixtures/bridge-cables.artifact.json",
    moduleSpecifier: "./patches/bridge-cables.patch.ts",
    exportName: "bridgeCables",
    patch: bridgeCables,
  },
  {
    source: "examples/compass-core.patch.ts",
    fixture: "test/fixtures/compass-core.artifact.json",
    moduleSpecifier: "./patches/compass-core.patch.ts",
    exportName: "compassCore",
    patch: compassCore,
  },
  {
    source: "examples/corner-reliefs.patch.ts",
    fixture: "test/fixtures/corner-reliefs.artifact.json",
    rustFixture: "../../crates/geosolve-sketch-code/assets/artifacts/corner-reliefs.artifact.json",
    moduleSpecifier: "./patches/corner-reliefs.patch.ts",
    exportName: "cornerReliefs",
    patch: cornerReliefs,
  },
  {
    source: "examples/harness-route.patch.ts",
    fixture: "test/fixtures/harness-route.artifact.json",
    rustFixture: "../../crates/geosolve-sketch-code/assets/artifacts/harness-route.artifact.json",
    moduleSpecifier: "./patches/harness-route.patch.ts",
    exportName: "harnessRoute",
    patch: harnessRoute,
  },
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
  {
    source: "examples/water-channel.patch.ts",
    fixture: "test/fixtures/water-channel.artifact.json",
    rustFixture: "../../crates/geosolve-sketch-code/assets/artifacts/water-channel.artifact.json",
    moduleSpecifier: "./patches/water-channel.patch.ts",
    exportName: "waterChannel",
    patch: waterChannel,
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
    if (fixture.rustFixture !== undefined) {
      assert.equal(
        await readFile(resolve(packageRoot, fixture.rustFixture), "utf8"),
        compiled.canonicalJson,
        `${fixture.rustFixture} is stale; run npm run generate:fixtures`,
      );
    }
  } else {
    await mkdir(dirname(destination), { recursive: true });
    await writeFile(destination, compiled.canonicalJson);
    if (fixture.rustFixture !== undefined) {
      const rustDestination = resolve(packageRoot, fixture.rustFixture);
      await mkdir(dirname(rustDestination), { recursive: true });
      await writeFile(rustDestination, compiled.canonicalJson);
    }
  }
}
