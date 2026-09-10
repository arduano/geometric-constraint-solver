// SPDX-License-Identifier: GPL-3.0-or-later

import { lstat, readFile, readdir } from "node:fs/promises";
import { dirname, resolve, relative, sep } from "node:path";
import { fileURLToPath } from "node:url";

const [distributionArgument, baseArgument = "./", harnessArgument] = process.argv.slice(2);
if (harnessArgument !== undefined && harnessArgument !== "--harness") throw new Error("unknown distribution validation option");
const harness = harnessArgument === "--harness";
if (!distributionArgument) {
  throw new Error("usage: node scripts/validate-dist.mjs <distribution> [public-base] [--harness]");
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
  ...(harness ? ["compiler-parity.html"] : []),
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
// M98 keeps navigation, provisional authoring and shared text in independent
// workers. Pin the complete module inventory rather than accepting arbitrary
// additions. Optional collaboration assets are fetched only when that mode opens.
const wasmPolicy = new Map([
  ["geosolve_demo_web_bg", 20 * 1024 * 1024],
  ["geosolve_sketch_engine_wasm_bg", 20 * 1024 * 1024],
  ["geosolve_collaboration_wasm_bg", 6 * 1024 * 1024],
]);
const wasmFiles = byExtension(".wasm");
if (wasmFiles.length !== wasmPolicy.size) throw new Error(`distribution must have ${wasmPolicy.size} declared WASM modules; found ${wasmFiles.length}`);
const seenModules = new Set();
for (const file of wasmFiles) {
  const name = [...wasmPolicy.keys()].find(module => new RegExp(`^assets/${module}-[A-Za-z0-9_-]{8,}\\.wasm$`).test(file));
  if (!wasmPolicy.has(name) || seenModules.has(name)) throw new Error(`unknown or duplicate WASM module: ${file}`);
  seenModules.add(name);
  if (fileSizes.get(file) >= wasmPolicy.get(name)) throw new Error(`${file} must remain below its ${wasmPolicy.get(name) / 1024 / 1024} MiB optimized-release ceiling`);
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

for (const entrypoint of harness ? ["index.html", "compiler-parity.html"] : ["index.html"]) {
  const html = await readFile(resolve(distribution, entrypoint), "utf8");
  const applicationUrls = [...html.matchAll(/(?:src|href)="([^"]+\.(?:js|css)(?:\?[^\"]*)?)"/g)].map(
    (match) => match[1],
  );
  if (applicationUrls.length < (entrypoint === "index.html" ? 2 : 1)) throw new Error(`${entrypoint} does not load its application assets`);
  for (const url of applicationUrls) {
    if (!url.startsWith(baseArgument)) {
      throw new Error(`application URL does not use public base ${baseArgument}: ${url}`);
    }
    const file = url.slice(baseArgument.length).split("?", 1)[0];
    if (!files.includes(file)) throw new Error(`index.html references missing asset: ${file}`);
  }
}

const wasmMagic = [0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00];
for (const wasmFile of wasmFiles) {
  const wasmBytes = await readFile(resolve(distribution, wasmFile));
  if (wasmBytes.length < wasmMagic.length || wasmMagic.some((byte, index) => wasmBytes[index] !== byte)) {
    throw new Error(`${wasmFile} is not a version-1 WebAssembly module`);
  }
}
// M91 compiler/SDK payload plus the three explicitly budgeted M98 modules.
// Keep the non-WASM code/assets bounded independently, as well as total output.
const wasmTotal = wasmFiles.reduce((sum, file) => sum + fileSizes.get(file), 0);
if (totalBytes - wasmTotal >= 14 * 1024 * 1024) throw new Error("non-WASM assets must remain below the 14 MiB optimized-release ceiling");
const maximumDistributionBytes = 60 * 1024 * 1024;
if (totalBytes >= maximumDistributionBytes) throw new Error("distribution must remain below the 60 MiB optimized-release ceiling");
const scripts = await Promise.all(byExtension(".js").map((file) => readFile(resolve(distribution, file), "utf8")));
for (const wasmFile of wasmFiles) {
  if (!scripts.some((script) => script.includes(wasmFile.split("/").at(-1)))) throw new Error(`JavaScript bundles do not reference ${wasmFile}`);
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
  `validated ${files.length}-file Vite distribution (${byExtension(".js").length} JS, ${byExtension(".css").length} CSS, ${wasmFiles.length} WASM)`,
);
