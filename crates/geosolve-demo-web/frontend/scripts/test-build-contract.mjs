// SPDX-License-Identifier: GPL-3.0-or-later

import { copyFile, mkdir, mkdtemp, readdir, rm, stat, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";

const scriptDirectory = dirname(fileURLToPath(import.meta.url));
const repositoryRoot = resolve(scriptDirectory, "../../../..");
const validator = resolve(scriptDirectory, "validate-dist.mjs");
const temporaryRoot = await mkdtemp(resolve(tmpdir(), "geosolve-build-contract-"));
const maximumReleaseWasmBytes = 20 * 1024 * 1024;
const maximumDistributionBytes = 30 * 1024 * 1024;
const wasmMagic = new Uint8Array([0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00]);

async function fixture(name, publicBase) {
  const distribution = resolve(temporaryRoot, name);
  await mkdir(resolve(distribution, "assets"), { recursive: true });
  await Promise.all([
    copyFile(resolve(repositoryRoot, "LICENSE"), resolve(distribution, "LICENSE")),
    copyFile(
      resolve(repositoryRoot, "THIRD_PARTY_LICENSES.md"),
      resolve(distribution, "THIRD_PARTY_LICENSES.md"),
    ),
    copyFile(
      resolve(repositoryRoot, "docs/API_COMPATIBILITY.md"),
      resolve(distribution, "API_COMPATIBILITY.md"),
    ),
    writeFile(resolve(distribution, "assets/index-12345678.css"), "body{}\n"),
    writeFile(
      resolve(distribution, "assets/module-12345678.wasm"),
      new Uint8Array([0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00]),
    ),
    writeFile(
      resolve(distribution, "assets/index-12345678.js"),
      'new URL("module-12345678.wasm", import.meta.url);\n',
    ),
    writeFile(
      resolve(distribution, "index.html"),
      `<!doctype html><link rel="stylesheet" href="${publicBase}assets/index-12345678.css"><script type="module" src="${publicBase}assets/index-12345678.js"></script>\n`,
    ),
  ]);
  return distribution;
}

function validate(distribution, publicBase, success) {
  const result = spawnSync(process.execPath, [validator, distribution, publicBase], {
    encoding: "utf8",
  });
  if ((result.status === 0) !== success) {
    process.stderr.write(result.stdout);
    process.stderr.write(result.stderr);
    throw new Error(`distribution validation unexpectedly ${success ? "failed" : "passed"}`);
  }
}

async function distributionBytes(directory) {
  let bytes = 0;
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    const path = resolve(directory, entry.name);
    bytes += entry.isDirectory() ? await distributionBytes(path) : (await stat(path)).size;
  }
  return bytes;
}

try {
  validate(await fixture("geosolve-relative", "./"), "./", true);
  const pages = await fixture("geosolve-pages", "/geometric-constraint-solver/");
  validate(pages, "/geometric-constraint-solver/", true);
  validate(pages, "/wrong-repository/", false);
  const unexpected = await fixture("geosolve-unexpected", "./");
  await writeFile(resolve(unexpected, "source.ts"), "not a release asset\n");
  validate(unexpected, "./", false);
  const invalidWasm = await fixture("geosolve-invalid-wasm", "./");
  await writeFile(resolve(invalidWasm, "assets/module-12345678.wasm"), "not wasm\n");
  validate(invalidWasm, "./", false);
  const wasmAtCeiling = await fixture("geosolve-wasm-at-ceiling", "./");
  const ceilingWasm = new Uint8Array(maximumReleaseWasmBytes);
  ceilingWasm.set(wasmMagic);
  await writeFile(resolve(wasmAtCeiling, "assets/module-12345678.wasm"), ceilingWasm);
  validate(wasmAtCeiling, "./", false);
  const wasmBelowCeiling = await fixture("geosolve-wasm-below-ceiling", "./");
  const belowCeilingWasm = new Uint8Array(maximumReleaseWasmBytes - 1);
  belowCeilingWasm.set(wasmMagic);
  await writeFile(resolve(wasmBelowCeiling, "assets/module-12345678.wasm"), belowCeilingWasm);
  validate(wasmBelowCeiling, "./", true);
  const distributionAtCeiling = await fixture("geosolve-distribution-at-ceiling", "./");
  const fixtureBytes = await distributionBytes(distributionAtCeiling);
  await writeFile(
    resolve(distributionAtCeiling, "assets/padding-12345678.js"),
    new Uint8Array(maximumDistributionBytes - fixtureBytes),
  );
  validate(distributionAtCeiling, "./", false);
  const distributionBelowCeiling = await fixture("geosolve-distribution-below-ceiling", "./");
  const belowCeilingFixtureBytes = await distributionBytes(distributionBelowCeiling);
  await writeFile(
    resolve(distributionBelowCeiling, "assets/padding-12345678.js"),
    new Uint8Array(maximumDistributionBytes - belowCeilingFixtureBytes - 1),
  );
  validate(distributionBelowCeiling, "./", true);
  const tampered = await fixture("geosolve-tampered", "./");
  await writeFile(resolve(tampered, "LICENSE"), "tampered\n");
  validate(tampered, "./", false);
  console.log("validated relative and repository-prefixed build contracts");
} finally {
  await rm(temporaryRoot, { recursive: true, force: true });
}
