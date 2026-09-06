// SPDX-License-Identifier: GPL-3.0-or-later

import { mkdir, readFile, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { parseArgs } from "node:util";
import { frontendDirectory, hash, publicBase, run, writeManifest } from "./release-artifact-lib.mjs";

const { values } = parseArgs({ options: { out: { type: "string" }, base: { type: "string", default: "./" } } });
if (!values.out) throw new Error("usage: node scripts/build-release-artifacts.mjs --out <new-directory> [--base ./]");
const output = resolve(values.out);
const base = publicBase(values.base);
// A stage owns this new directory. Failed/incomplete output is preserved for its
// receipt; callers resume into another new path instead of overwriting evidence.
await mkdir(output);
const artifacts = [
  { kind: "harness", base: "./", directory: resolve(output, "geosolve-harness"), manifest: resolve(output, "harness.json") },
  { kind: "production", base, directory: resolve(output, "geosolve-production"), manifest: resolve(output, "production.json") },
];

// The scheduler serializes this preparation with every writer of src/generated
// and node_modules. Both Vite bundles consume the same optimized WASM package.
run("npm", ["run", "wasm:release"]);
run("npm", ["run", "check:types"]);
const generatedWasm = resolve(frontendDirectory, "src/generated/geosolve_demo_web_bg.wasm");
const optimizedWasmSha256 = hash(await readFile(generatedWasm));
for (const artifact of artifacts) {
  run(resolve(frontendDirectory, "node_modules/.bin/vite"), ["build"], {
    env: { ...process.env, GEOSOLVE_PUBLIC_BASE: artifact.base, GEOSOLVE_DIST: artifact.directory,
      GEOSOLVE_BROWSER_COMPILER_HARNESS: artifact.kind === "harness" ? "1" : "0", VITE_GEOSOLVE_MOCK: "0" },
  });
  const manifest = await writeManifest(artifact.directory, artifact.kind, artifact.base, artifact.manifest);
  const wasm = manifest.files.find((file) => file.path.endsWith(".wasm"));
  if (wasm.sha256 !== optimizedWasmSha256) throw new Error(`${artifact.kind} did not preserve the prepared optimized WASM bytes`);
}
await writeFile(resolve(output, "build.json"), `${JSON.stringify({
  format: "geosolve-release-browser-build-v1", optimizedWasmSha256,
  harnessManifest: artifacts[0].manifest, productionManifest: artifacts[1].manifest,
}, null, 2)}\n`, { flag: "wx" });
console.log(`prepared one harness and one production distribution in ${output}`);
