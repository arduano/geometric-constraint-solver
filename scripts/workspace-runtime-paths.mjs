// SPDX-License-Identifier: GPL-3.0-or-later
import { existsSync, readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

/** The same bridge runs from a checkout or the CLI's explicit packaged resources. */
export const runtimeRoot = fileURLToPath(new URL("../", import.meta.url));
const descriptor = resolve(runtimeRoot, "runtime-layout.json");
const packaged = existsSync(descriptor);
if (packaged && JSON.parse(readFileSync(descriptor, "utf8")).format !== "geosolve-cli-runtime-v1") throw Error("Unsupported GeoSolve CLI runtime layout");
const frontend = resolve(runtimeRoot, "crates/geosolve-demo-web/frontend");
export const sdkRoot = packaged ? resolve(dirname(fileURLToPath(import.meta.resolve("@geosolve/sketch-code"))), "../..") : resolve(runtimeRoot, "packages/geosolve-sketch-code");
export const sdkDirectory = resolve(sdkRoot, "dist/src");
export const typescriptModuleUrl = pathToFileURL(resolve(sdkRoot, "node_modules/typescript/lib/typescript.js")).href;
export const esbuildRoot = packaged ? resolve(runtimeRoot, "vendor/esbuild") : resolve(frontend, "node_modules/esbuild");
export const esbuildModuleUrl = pathToFileURL(resolve(esbuildRoot, "lib/main.js")).href;
export const engineModuleUrl = packaged ? import.meta.resolve("@geosolve/engine") : pathToFileURL(resolve(runtimeRoot, "packages/geosolve-engine/dist/index.js")).href;
export const demoBindingsUrl = pathToFileURL(packaged ? resolve(runtimeRoot, "assets/demo-wasm/geosolve_demo_web.js") : resolve(frontend, "src/generated/geosolve_demo_web.js")).href;
export const demoWasmPath = packaged ? resolve(runtimeRoot, "assets/demo-wasm/geosolve_demo_web_bg.wasm") : resolve(frontend, "src/generated/geosolve_demo_web_bg.wasm");
export const workbenchRuntimeUrl = pathToFileURL(packaged ? resolve(runtimeRoot, "assets/workspace-runtime.mjs") : resolve(runtimeRoot, "target/m98/workspace-runtime.mjs")).href;
export const workbenchDist = packaged ? resolve(runtimeRoot, "assets/workbench") : resolve(runtimeRoot, "crates/geosolve-demo-web/dist");
export const starterDirectory = packaged ? resolve(runtimeRoot, "assets/starter") : resolve(runtimeRoot, "examples/file-workspace");
