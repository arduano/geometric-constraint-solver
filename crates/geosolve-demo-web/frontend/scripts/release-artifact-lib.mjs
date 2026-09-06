// SPDX-License-Identifier: GPL-3.0-or-later

import { createHash } from "node:crypto";
import { lstat, readFile, readdir, writeFile } from "node:fs/promises";
import { dirname, relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";

export const artifactFormat = "geosolve-release-artifact-v1";
export const frontendDirectory = resolve(dirname(fileURLToPath(import.meta.url)), "..");
export const hash = (bytes) => createHash("sha256").update(bytes).digest("hex");
export const mediaType = (path) => path.endsWith(".html") ? "text/html"
  : path.endsWith(".js") ? "text/javascript"
    : path.endsWith(".css") ? "text/css"
      : path.endsWith(".wasm") ? "application/wasm"
        : path.endsWith(".md") ? "text/markdown" : "application/octet-stream";

export function publicBase(value) {
  if (typeof value !== "string" || (value !== "./" && !(value.startsWith("/") && value.endsWith("/")
    && !/[?#%\\]/.test(value) && !value.includes("//")
    && !value.split("/").some((part) => part === "." || part === "..")))) {
    throw new Error(`invalid public base: ${value}`);
  }
  return value;
}

export function run(command, args, options = {}) {
  const result = spawnSync(command, args, { cwd: frontendDirectory, stdio: "inherit", ...options });
  if (result.error) throw result.error;
  if (result.status !== 0) throw new Error(`${command} exited ${result.status ?? result.signal}`);
  return result;
}

export async function inventory(directory) {
  const root = await lstat(directory);
  if (!root.isDirectory() || root.isSymbolicLink()) throw new Error("artifact must be a real directory");
  const files = [];
  async function walk(path) {
    for (const name of (await readdir(path)).sort()) {
      const absolute = resolve(path, name);
      const info = await lstat(absolute);
      if (info.isSymbolicLink()) throw new Error(`artifact contains a symlink: ${absolute}`);
      if (info.isDirectory()) {
        if (relative(directory, absolute) !== "assets") throw new Error(`unexpected artifact directory: ${absolute}`);
        await walk(absolute);
      }
      else if (info.isFile()) {
        const bytes = await readFile(absolute);
        files.push({ path: relative(directory, absolute).split(sep).join("/"), bytes: bytes.length, sha256: hash(bytes) });
      } else throw new Error(`artifact contains a non-file entry: ${absolute}`);
    }
  }
  await walk(directory);
  return files.sort((a, b) => a.path < b.path ? -1 : a.path > b.path ? 1 : 0);
}

export function validateManifest(manifest) {
  const keys = ["format", "kind", "publicBase", "directory", "files", "totalBytes", "filesSha256"];
  if (!manifest || typeof manifest !== "object" || Array.isArray(manifest)
    || Object.keys(manifest).sort().join() !== keys.sort().join()
    || manifest.format !== artifactFormat || !["production", "harness"].includes(manifest.kind)
    || typeof manifest.directory !== "string" || !manifest.directory.startsWith("/")
    || !Array.isArray(manifest.files) || manifest.files.length === 0) throw new Error("invalid release artifact manifest");
  publicBase(manifest.publicBase);
  const seen = new Set();
  for (const file of manifest.files) {
    if (!file || Object.keys(file).sort().join() !== "bytes,path,sha256"
      || typeof file.path !== "string" || !/^[A-Za-z0-9_.\/-]+$/.test(file.path)
      || file.path.startsWith("/") || file.path.split("/").some((part) => !part || part === "." || part === "..")
      || seen.has(file.path) || !Number.isSafeInteger(file.bytes) || file.bytes < 0
      || typeof file.sha256 !== "string" || !/^[a-f0-9]{64}$/.test(file.sha256)) throw new Error("invalid artifact file inventory");
    seen.add(file.path);
  }
  if (manifest.totalBytes !== manifest.files.reduce((sum, file) => sum + file.bytes, 0)
    || manifest.filesSha256 !== hash(JSON.stringify(manifest.files))) throw new Error("artifact inventory digest mismatch");
  if (!seen.has("index.html") || seen.has("compiler-parity.html") !== (manifest.kind === "harness")) {
    throw new Error("artifact production/harness identity mismatch");
  }
  return manifest;
}

export async function readManifest(path, directoryOverride) {
  const bytes = await readFile(path);
  const manifest = validateManifest(JSON.parse(bytes));
  const directory = resolve(directoryOverride ?? manifest.directory);
  const actual = await inventory(directory);
  if (JSON.stringify(actual) !== JSON.stringify(manifest.files)) throw new Error("artifact bytes do not match manifest");
  return { manifest, directory, manifestSha256: hash(bytes) };
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
