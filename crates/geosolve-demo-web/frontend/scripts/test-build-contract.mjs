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
const maximumDistributionBytes = 60 * 1024 * 1024;
const maximumNonWasmBytes = 14 * 1024 * 1024;
const modules = ["geosolve_demo_web_bg", "geosolve_sketch_engine_wasm_bg", "geosolve_collaboration_wasm_bg"];
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
      resolve(distribution, "assets/geosolve_demo_web_bg-12345678.wasm"),
      new Uint8Array([0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00]),
    ),
    writeFile(
      resolve(distribution, "assets/index-12345678.js"),
      'new URL("geosolve_demo_web_bg-12345678.wasm", import.meta.url);\n',
    ),
    writeFile(
      resolve(distribution, "index.html"),
      `<!doctype html><link rel="stylesheet" href="${publicBase}assets/index-12345678.css"><script type="module" src="${publicBase}assets/index-12345678.js"></script>\n`,
    ),
  ]);
  for (const module of modules.slice(1)) await writeFile(resolve(distribution, `assets/${module}-12345678.wasm`), wasmMagic);
  await writeFile(resolve(distribution, "assets/index-12345678.js"), modules.map(module => `new URL("${module}-12345678.wasm", import.meta.url);`).join("\n"));
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
  await writeFile(resolve(invalidWasm, "assets/geosolve_demo_web_bg-12345678.wasm"), "not wasm\n");
  validate(invalidWasm, "./", false);
  const wasmAtCeiling = await fixture("geosolve-wasm-at-ceiling", "./");
  const ceilingWasm = new Uint8Array(maximumReleaseWasmBytes);
  ceilingWasm.set(wasmMagic);
  await writeFile(resolve(wasmAtCeiling, "assets/geosolve_demo_web_bg-12345678.wasm"), ceilingWasm);
  validate(wasmAtCeiling, "./", false);
  const wasmBelowCeiling = await fixture("geosolve-wasm-below-ceiling", "./");
  const belowCeilingWasm = new Uint8Array(maximumReleaseWasmBytes - 1);
  belowCeilingWasm.set(wasmMagic);
  await writeFile(resolve(wasmBelowCeiling, "assets/geosolve_demo_web_bg-12345678.wasm"), belowCeilingWasm);
  validate(wasmBelowCeiling, "./", true);
  for (const module of modules.slice(1)) {
    const invalid = await fixture(`invalid-${module}`, "./");
    await writeFile(resolve(invalid, `assets/${module}-12345678.wasm`), "invalid"); validate(invalid, "./", false);
    const unreferenced = await fixture(`unreferenced-${module}`, "./");
    await writeFile(resolve(unreferenced, "assets/index-12345678.js"), modules.filter(item => item !== module).join("-12345678.wasm ") + "-12345678.wasm"); validate(unreferenced, "./", false);
    const oversized = await fixture(`oversized-${module}`, "./");
    const bytes = new Uint8Array((module.includes("collaboration") ? 6 : 20) * 1024 * 1024); bytes.set(wasmMagic);
    await writeFile(resolve(oversized, `assets/${module}-12345678.wasm`), bytes); validate(oversized, "./", false);
  }
  const extra = await fixture("extra-module", "./");
  await writeFile(resolve(extra, "assets/unknown-12345678.wasm"), wasmMagic); validate(extra, "./", false);
  const duplicate = await fixture("duplicate-module", "./");
  await rm(resolve(duplicate, "assets/geosolve_collaboration_wasm_bg-12345678.wasm"));
  await writeFile(resolve(duplicate, "assets/geosolve_demo_web_bg-abcdefgh.wasm"), wasmMagic); validate(duplicate, "./", false);
  const missing = await fixture("missing-module", "./");
  await rm(resolve(missing, "assets/geosolve_collaboration_wasm_bg-12345678.wasm")); validate(missing, "./", false);
  const distributionAtCeiling = await fixture("geosolve-distribution-at-ceiling", "./");
  const fixtureBytes = await distributionBytes(distributionAtCeiling);
  await writeFile(resolve(distributionAtCeiling, "assets/padding-12345678.js"), new Uint8Array(maximumDistributionBytes - fixtureBytes));
  validate(distributionAtCeiling, "./", false);
  const nonWasmAtCeiling = await fixture("non-wasm-at-ceiling", "./");
  const nonWasmBytes = await distributionBytes(nonWasmAtCeiling) - modules.length * wasmMagic.length;
  await writeFile(resolve(nonWasmAtCeiling, "assets/padding-12345678.js"), new Uint8Array(maximumNonWasmBytes - nonWasmBytes));
  validate(nonWasmAtCeiling, "./", false);
  const distributionBelowCeiling = await fixture("geosolve-distribution-below-ceiling", "./");
  for (const module of modules) {
    const bytes = new Uint8Array((module.includes("collaboration") ? 6 : 20) * 1024 * 1024 - 1); bytes.set(wasmMagic);
    await writeFile(resolve(distributionBelowCeiling, `assets/${module}-12345678.wasm`), bytes);
  }
  const belowCeilingFixtureBytes = await distributionBytes(distributionBelowCeiling);
  await writeFile(resolve(distributionBelowCeiling, "assets/padding-12345678.js"), new Uint8Array(maximumDistributionBytes - belowCeilingFixtureBytes - 4));
  validate(distributionBelowCeiling, "./", true);
  const tampered = await fixture("geosolve-tampered", "./");
  await writeFile(resolve(tampered, "LICENSE"), "tampered\n");
  validate(tampered, "./", false);
  console.log("validated relative and repository-prefixed build contracts");
} finally {
  await rm(temporaryRoot, { recursive: true, force: true });
}
