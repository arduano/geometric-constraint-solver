// SPDX-License-Identifier: GPL-3.0-or-later

import { writeFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";
import { artifactFormat, hash, publicBase, inventory, validateManifest } from "../../../../packages/geosolve-cli/runtime/release-artifact.mjs";
export { artifactFormat, hash, mediaType, publicBase, inventory, validateManifest, readManifest } from "../../../../packages/geosolve-cli/runtime/release-artifact.mjs";

export const frontendDirectory = resolve(dirname(fileURLToPath(import.meta.url)), "..");

export function run(command, args, options = {}) {
  const result = spawnSync(command, args, { cwd: frontendDirectory, stdio: "inherit", ...options });
  if (result.error) throw result.error;
  if (result.status !== 0) throw new Error(`${command} exited ${result.status ?? result.signal}`);
  return result;
}

export async function writeManifest(directory, kind, base, output) {
  run(process.execPath, [resolve(frontendDirectory, "scripts/validate-dist.mjs"), directory, base,
    ...(kind === "harness" ? ["--harness"] : [])]);
  const files = await inventory(directory);
  const manifest = validateManifest({ format: artifactFormat, kind, publicBase: publicBase(base),
    directory: resolve(directory), files, totalBytes: files.reduce((sum, file) => sum + file.bytes, 0),
    filesSha256: hash(JSON.stringify(files)) });
  await writeFile(output, `${JSON.stringify(manifest, null, 2)}\n`, { flag: "wx" });
  return manifest;
}
