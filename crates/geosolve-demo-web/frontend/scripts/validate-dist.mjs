// SPDX-License-Identifier: GPL-3.0-or-later

import { lstat, readFile, readdir } from "node:fs/promises";
import { dirname, resolve, relative, sep } from "node:path";
import { fileURLToPath } from "node:url";

const [distributionArgument, baseArgument = "./"] = process.argv.slice(2);
if (!distributionArgument) {
  throw new Error("usage: node scripts/validate-dist.mjs <distribution> [public-base]");
}
if (baseArgument !== "./" && !(baseArgument.startsWith("/") && baseArgument.endsWith("/"))) {
  throw new Error(`public base must be ./ or an absolute trailing-slash path: ${baseArgument}`);
}

const distribution = resolve(distributionArgument);
const scriptDirectory = dirname(fileURLToPath(import.meta.url));
const repositoryRoot = resolve(scriptDirectory, "../../../..");
const required = [
  "index.html",
  "LICENSE",
  "THIRD_PARTY_LICENSES.md",
  "API_COMPATIBILITY.md",
];
const files = [];
const directories = [];
const fileSizes = new Map();
let totalBytes = 0;

async function walk(directory) {
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    const path = resolve(directory, entry.name);
    const stat = await lstat(path);
    if (stat.isSymbolicLink()) throw new Error(`distribution contains a symlink: ${path}`);
    if (entry.isDirectory()) {
      directories.push(relative(distribution, path).split(sep).join("/"));
      await walk(path);
    }
    else if (entry.isFile()) {
      const relativePath = relative(distribution, path).split(sep).join("/");
      files.push(relativePath);
      fileSizes.set(relativePath, stat.size);
      totalBytes += stat.size;
    }
  }
}

const rootStat = await lstat(distribution);
if (!rootStat.isDirectory() || rootStat.isSymbolicLink()) {
  throw new Error("distribution must be a real directory");
}
await walk(distribution);

for (const file of required) {
  if (!files.includes(file)) throw new Error(`distribution is missing ${file}`);
}
for (const file of files) {
  if (!required.includes(file) && !file.startsWith("assets/")) {
    throw new Error(`distribution contains an unexpected root file: ${file}`);
  }
}
for (const directory of directories) {
  if (directory !== "assets") {
    throw new Error(`distribution contains an unexpected directory: ${directory}`);
  }
}

const documentSources = new Map([
  ["LICENSE", resolve(repositoryRoot, "LICENSE")],
  ["THIRD_PARTY_LICENSES.md", resolve(repositoryRoot, "THIRD_PARTY_LICENSES.md")],
  ["API_COMPATIBILITY.md", resolve(repositoryRoot, "docs/API_COMPATIBILITY.md")],
]);
for (const [name, source] of documentSources) {
  const [published, authoritative] = await Promise.all([
    readFile(resolve(distribution, name)),
    readFile(source),
  ]);
  if (!published.equals(authoritative)) {
    throw new Error(`${name} does not byte-match repository authority`);
  }
}

const byExtension = (extension) => files.filter((file) => file.endsWith(extension));
if (byExtension(".js").length === 0) throw new Error("distribution has no JavaScript bundle");
if (byExtension(".css").length === 0) throw new Error("distribution has no CSS bundle");
if (byExtension(".wasm").length !== 1) {
  throw new Error(`distribution must have one WASM module; found ${byExtension(".wasm").length}`);
}
const applicationAssets = files.filter((file) => file.startsWith("assets/"));
for (const file of applicationAssets) {
  if (![".js", ".css", ".wasm"].some((extension) => file.endsWith(extension))) {
    throw new Error(`distribution contains an unexpected application asset: ${file}`);
  }
}
for (const file of [...byExtension(".js"), ...byExtension(".css"), ...byExtension(".wasm")]) {
  if (!/^assets\/.+-[A-Za-z0-9_-]{8,}\.[^.]+$/.test(file)) {
    throw new Error(`application asset is not content-hashed under assets/: ${file}`);
  }
}

const html = await readFile(resolve(distribution, "index.html"), "utf8");
const applicationUrls = [...html.matchAll(/(?:src|href)="([^"]+\.(?:js|css)(?:\?[^\"]*)?)"/g)].map(
  (match) => match[1],
);
if (applicationUrls.length < 2) throw new Error("index.html does not load JavaScript and CSS");
for (const url of applicationUrls) {
  if (!url.startsWith(baseArgument)) {
    throw new Error(`application URL does not use public base ${baseArgument}: ${url}`);
  }
  const file = url.slice(baseArgument.length).split("?", 1)[0];
  if (!files.includes(file)) throw new Error(`index.html references missing asset: ${file}`);
}

const wasmFile = byExtension(".wasm")[0];
const wasmBytes = await readFile(resolve(distribution, wasmFile));
const wasmMagic = [0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00];
if (wasmBytes.length < wasmMagic.length || wasmMagic.some((byte, index) => wasmBytes[index] !== byte)) {
  throw new Error(`${wasmFile} is not a version-1 WebAssembly module`);
}
const maximumReleaseWasmBytes = 20 * 1024 * 1024;
if ((fileSizes.get(wasmFile) ?? 0) > maximumReleaseWasmBytes) {
  throw new Error(`${wasmFile} exceeds the 20 MiB optimized-release ceiling`);
}
// M91's TypeScript language service is a separate, lazy Web Worker so its
// compiler payload never blocks the primary design shell. Keep a hard overall
// ceiling while admitting the pinned compiler and SDK declarations once.
const maximumDistributionBytes = 30 * 1024 * 1024;
if (totalBytes > maximumDistributionBytes) {
  throw new Error("distribution exceeds the 30 MiB optimized-release ceiling");
}
const scripts = await Promise.all(
  byExtension(".js").map((file) => readFile(resolve(distribution, file), "utf8")),
);
if (!scripts.some((script) => script.includes(wasmFile.split("/").at(-1)))) {
  throw new Error(`JavaScript bundles do not reference ${wasmFile}`);
}
const styles = await Promise.all(
  byExtension(".css").map((file) => readFile(resolve(distribution, file), "utf8")),
);
for (const bundledText of [...scripts, ...styles]) {
  if (baseArgument !== "/" && /['"`(]\/assets\//.test(bundledText)) {
    throw new Error("bundle contains an unprefixed root application asset URL");
  }
}

console.log(
  `validated ${files.length}-file Vite distribution (${byExtension(".js").length} JS, ${byExtension(".css").length} CSS, 1 WASM)`,
);
