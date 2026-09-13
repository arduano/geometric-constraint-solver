// SPDX-License-Identifier: GPL-3.0-or-later
import { existsSync, readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

/** Host code and its build outputs belong to the CLI in both supported layouts. */
const runtimeDirectory = fileURLToPath(new URL("./", import.meta.url));
const packageRoot = fileURLToPath(new URL("../", import.meta.url));
const descriptor = resolve(runtimeDirectory, "runtime-layout.json");
const packaged = existsSync(descriptor);
if (packaged && JSON.parse(readFileSync(descriptor, "utf8")).format !== "geosolve-cli-runtime-v1") throw Error("Unsupported GeoSolve CLI runtime layout");
export const runtimeRoot = packaged ? runtimeDirectory : resolve(packageRoot, "../..");
export const sdkRoot = resolve(dirname(fileURLToPath(import.meta.resolve("@geosolve/sketch-code"))), "../..");
export const sdkDirectory = resolve(sdkRoot, "dist/src");
export const typescriptModuleUrl = pathToFileURL(resolve(sdkRoot, "node_modules/typescript/lib/typescript.js")).href;
export const esbuildRoot = packaged ? resolve(runtimeRoot, "vendor/esbuild") : resolve(packageRoot, "node_modules/esbuild");
export const esbuildModuleUrl = pathToFileURL(resolve(esbuildRoot, "lib/main.js")).href;
export const engineModuleUrl = import.meta.resolve("@geosolve/engine");
export const workbenchRuntimeUrl = pathToFileURL(resolve(packageRoot, "dist/workspace-runtime.mjs")).href;
export const workbenchDist = packaged ? resolve(runtimeRoot, "assets/workbench") : resolve(runtimeRoot, "crates/geosolve-demo-web/dist");
export const starterDirectory = packaged ? resolve(runtimeRoot, "assets/starter") : resolve(runtimeRoot, "examples/file-workspace");

export const collaborationModuleUrl = import.meta.resolve("@geosolve/collaboration");
export const collaborationHostModuleUrl = import.meta.resolve("@geosolve/collaboration/host");
export const collaborationClientModuleUrl = import.meta.resolve("@geosolve/collaboration/client");
export const releaseArtifactModuleUrl = new URL("./release-artifact.mjs", import.meta.url).href;
export const packagedWorkbenchManifest = packaged ? resolve(runtimeRoot, "assets/workbench-artifact.json") : undefined;
