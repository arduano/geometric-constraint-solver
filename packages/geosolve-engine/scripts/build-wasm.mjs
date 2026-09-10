// SPDX-License-Identifier: GPL-3.0-or-later
import { spawnSync } from "node:child_process";
import { readFileSync, mkdirSync, mkdtempSync, renameSync, rmSync, existsSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { resolve } from "node:path";
const root = fileURLToPath(new URL("../../../", import.meta.url));
const packageRoot = resolve(root, "packages/geosolve-engine");
const run = (command, args, capture = false) => {
  const result = spawnSync(command, args, { cwd: root, encoding: "utf8", stdio: capture ? "pipe" : "inherit" });
  if (result.error) throw result.error;
  if (result.status !== 0) throw Error(`${command} failed (${result.status}): ${result.stderr ?? ""}`);
  return result.stdout?.trim();
};
const release = !process.argv.includes("--debug");
const locked = readFileSync(resolve(root, "Cargo.lock"), "utf8").match(/name = "wasm-bindgen"\nversion = "([^"]+)"/)?.[1];
if (!locked || !run("wasm-bindgen", ["--version"], true).endsWith(` ${locked}`)) throw Error("Use the pinned wasm-bindgen CLI from shell.nix");
run("cargo", ["build", "--locked", "-p", "geosolve-sketch-engine-wasm", "--target", "wasm32-unknown-unknown", ...(release ? ["--release"] : [])]);
const metadata = JSON.parse(run("cargo", ["metadata", "--locked", "--format-version", "1", "--no-deps"], true));
mkdirSync(resolve(packageRoot, "dist"), { recursive: true });
const stage = mkdtempSync(resolve(packageRoot, ".wasm-"));
try {
  run("wasm-bindgen", ["--target", "web", "--out-dir", stage, "--out-name", "geosolve_sketch_engine_wasm", resolve(metadata.target_directory, "wasm32-unknown-unknown", release ? "release" : "debug", "geosolve_sketch_engine_wasm.wasm")]);
  const declarations = readFileSync(resolve(stage, "geosolve_sketch_engine_wasm.d.ts"), "utf8");
  for (const method of ["export class SketchEngine", "evaluateGenerated(", "evaluateManaged(", "exportProfiles(", "releaseResult(", "prepareEditableAuthoring(", "applyEditableAuthoring(", "releaseEditableAuthoring(", "exportEditableProject(", "editableSourceDesignDigest(", "editablePointGestureTargets(", "beginEditablePointGesture(", "advanceEditablePointGesture(", "editablePointGestureScene(", "finishEditablePointGesture(", "cancelEditablePointGesture(", "prepareEditablePointCommit(", "applyEditablePointCommit(", "releaseEditablePointCommit(", "beginEditableConstruction(", "advanceEditableConstruction(", "editableConstructionScene(", "finishEditableConstruction(", "cancelEditableConstruction(", "prepareEditableConstruction(", "resolveEditableConstruction(", "applyEditableConstructionCommit(", "releaseEditableConstruction("]) if (!declarations.includes(method)) throw Error(`Missing native contract: ${method}`);
  const output = resolve(packageRoot, "dist/wasm");
  if (existsSync(output)) rmSync(output, { recursive: true });
  renameSync(stage, output);
} finally { if (existsSync(stage)) rmSync(stage, { recursive: true }); }
