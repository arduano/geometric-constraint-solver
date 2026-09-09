// SPDX-License-Identifier: GPL-3.0-or-later
import { copyFileSync, cpSync, existsSync, mkdirSync, readFileSync, rmSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

export const folder = fileURLToPath(new URL("../", import.meta.url));
export const repository = resolve(folder, "../..");
const fromPackage = (name, fallback) => {
  try { return fileURLToPath(import.meta.resolve(name)); }
  catch { return resolve(repository, fallback); }
};
export const engineEntry = fromPackage("@geosolve/engine", "packages/geosolve-engine/dist/index.js");
export const sdkEntry = fromPackage("@geosolve/sketch-code", "packages/geosolve-sketch-code/dist/src/index.js");
const esbuildEntry = fromPackage("esbuild", "crates/geosolve-demo-web/frontend/node_modules/esbuild/lib/main.js");

export async function buildExample() {
  const { build } = await import(esbuildEntry);
  const dist = resolve(folder, "dist");
  const wasm = resolve(dirname(engineEntry), "wasm");
  if (!existsSync(resolve(wasm, "geosolve_sketch_engine_wasm_bg.wasm"))) throw Error("Build @geosolve/engine with its WASM before building this example");
  rmSync(dist, { recursive: true, force: true }); mkdirSync(dist, { recursive: true });
  const common = { bundle: true, format: "esm", target: "es2022", logLevel: "silent",
    alias: { "@geosolve/engine": engineEntry, "@geosolve/sketch-code": sdkEntry },
  };
  await build({ ...common, entryPoints: [resolve(folder, "src/main.ts"), resolve(folder, "src/worker.ts")], outdir: dist, platform: "browser" });
  cpSync(wasm, resolve(dist, "wasm"), { recursive: true });
  for (const name of ["index.html", "style.css"]) copyFileSync(resolve(folder, name), resolve(dist, name));
  const license = resolve(repository, "LICENSE");
  if (existsSync(license)) copyFileSync(license, resolve(dist, "LICENSE"));
  // Shipping own UI keeps the example independent of React and the demo editor.
  for (const name of ["main.js", "worker.js"]) {
    const text = readFileSync(resolve(dist, name), "utf8");
    if (text.includes("react-dom") || text.includes("WasmWorkbenchAdapter")) throw Error("Example unexpectedly bundled workbench UI");
  }
  return dist;
}

/** Test-only Node entry shares the installed SDK instance with the engine. */
export async function buildNodeGenerator() {
  const { build } = await import(esbuildEntry);
  const output = resolve(folder, "test-output/generator.mjs");
  await build({ entryPoints: [resolve(folder, "src/generator.ts")], outfile: output,
    bundle: true, format: "esm", target: "es2022", platform: "node", logLevel: "silent",
    alias: { "@geosolve/sketch-code": sdkEntry }, external: [sdkEntry] });
  return output;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  console.log(await buildExample());
}
