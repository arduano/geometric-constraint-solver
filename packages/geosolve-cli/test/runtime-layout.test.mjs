// SPDX-License-Identifier: GPL-3.0-or-later
import assert from "node:assert/strict";
import { existsSync, readFileSync, readdirSync } from "node:fs";
import { fileURLToPath } from "node:url";
import test from "node:test";
import { workbenchRuntimeUrl, esbuildRoot, releaseArtifactModuleUrl } from "../runtime/workspace-runtime-paths.mjs";

test("checkout hosting resolves package-owned runtime and build resources", () => {
  assert.equal(workbenchRuntimeUrl, new URL("../dist/workspace-runtime.mjs", import.meta.url).href);
  assert.equal(esbuildRoot, fileURLToPath(new URL("../node_modules/esbuild", import.meta.url)));
  assert.equal(releaseArtifactModuleUrl, new URL("../runtime/release-artifact.mjs", import.meta.url).href);
  const packageRoot = new URL("../", import.meta.url);
  const repository = new URL("../../", packageRoot);
  for (const file of readdirSync(new URL("runtime/", packageRoot))) {
    assert.ok(file.endsWith(".mjs"), `Unexpected production runtime resource: ${file}`);
    assert.equal(existsSync(new URL(`scripts/${file}`, repository)), false, `Duplicate runtime owner: ${file}`);
    const source = readFileSync(new URL(`runtime/${file}`, packageRoot), "utf8");
    assert.doesNotMatch(source, /geosolve_demo_web|demoWasm|demoBindings|frontend\/src/u, `Server executes browser code: ${file}`);
    assert.doesNotMatch(source, /target\/m\d+\//u, `Milestone-owned runtime input: ${file}`);
  }
});
