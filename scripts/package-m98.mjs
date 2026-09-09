#!/usr/bin/env node
// SPDX-License-Identifier: GPL-3.0-or-later
import { createHash } from "node:crypto";
import { spawnSync } from "node:child_process";
import { chmodSync, copyFileSync, cpSync, existsSync, lstatSync, mkdirSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { dirname, relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";
import { parseArgs } from "node:util";

export const repository = fileURLToPath(new URL("../", import.meta.url));
const hash = (bytes) => createHash("sha256").update(bytes).digest("hex");
const readJson = (path) => JSON.parse(readFileSync(path, "utf8"));
const json = (path, value) => writeFileSync(path, JSON.stringify(value, null, 2) + "\n", { flag: "wx" });
const copy = (source, destination) => { mkdirSync(dirname(destination), { recursive: true }); cpSync(source, destination, { recursive: true, dereference: true, errorOnExist: true, force: false }); };

function filesUnder(folder) {
  const result = [];
  function visit(path) {
    for (const name of readdirSync(path).sort()) {
      const absolute = resolve(path, name), info = lstatSync(absolute);
      if (info.isSymbolicLink()) throw Error(`Packaged resources cannot contain symlinks: ${absolute}`);
      if (info.isDirectory()) visit(absolute);
      else if (info.isFile()) result.push({ path: relative(folder, absolute).split(sep).join("/"), bytes: info.size, sha256: hash(readFileSync(absolute)) });
      else throw Error(`Packaged resources require ordinary files: ${absolute}`);
    }
  }
  visit(folder);
  return result;
}

function packageMetadata(source) {
  const original = readJson(resolve(source, "package.json"));
  return Object.fromEntries(Object.entries(original).filter(([key]) => ["name", "version", "description", "type", "exports", "license"].includes(key)));
}

function stageIntent(output) {
  const source = resolve(repository, "packages/geosolve-intent");
  copy(resolve(source, "dist/src"), resolve(output, "dist/src"));
  copy(resolve(source, "src"), resolve(output, "src"));
  copy(resolve(repository, "LICENSE"), resolve(output, "LICENSE"));
  const metadata = packageMetadata(source);
  json(resolve(output, "package.json"), { ...metadata, files: ["dist", "src", "LICENSE"] });
  return metadata;
}

function stageSdk(output) {
  const source = resolve(repository, "packages/geosolve-sketch-code");
  for (const path of ["dist/src", "src", "README.md"]) copy(resolve(source, path), resolve(output, path));
  copy(resolve(repository, "LICENSE"), resolve(output, "LICENSE"));
  const intent = stageIntent(resolve(output, "node_modules/@geosolve/intent"));
  const compiler = resolve(source, "node_modules/typescript");
  copy(compiler, resolve(output, "node_modules/typescript"));
  const typescript = readJson(resolve(compiler, "package.json"));
  const metadata = packageMetadata(source);
  json(resolve(output, "package.json"), { ...metadata, engines: { node: ">=22" },
    files: ["dist", "src", "README.md", "LICENSE"],
    dependencies: { "@geosolve/intent": intent.version, typescript: typescript.version },
    bundledDependencies: ["@geosolve/intent", "typescript"],
  });
  return metadata;
}

function stageEngine(output, sdk) {
  const source = resolve(repository, "packages/geosolve-engine");
  for (const path of ["dist", "README.md"]) copy(resolve(source, path), resolve(output, path));
  copy(resolve(repository, "LICENSE"), resolve(output, "LICENSE"));
  const metadata = packageMetadata(source);
  json(resolve(output, "package.json"), { ...metadata, engines: { node: ">=22" }, files: ["dist", "README.md", "LICENSE"], dependencies: { "@geosolve/sketch-code": sdk.version } });
  return metadata;
}

function stageCli(output, sdk, engine, dist) {
  const source = resolve(repository, "packages/geosolve-cli");
  for (const path of ["bin", "README.md"]) copy(resolve(source, path), resolve(output, path));
  chmodSync(resolve(output, "bin/geosolve.mjs"), 0o755);
  copy(resolve(repository, "LICENSE"), resolve(output, "LICENSE"));
  const metadata = readJson(resolve(source, "package.json"));
  json(resolve(output, "package.json"), { ...metadata, cpu: [process.arch], dependencies: { "@geosolve/sketch-code": sdk.version, "@geosolve/engine": engine.version } });
  const runtime = resolve(output, "runtime");
  const scripts = ["geosolve-cli.mjs", "file-workspace.mjs", "workspace-loader.mjs", "workspace-loader-worker.mjs", "workspace-runtime-paths.mjs",
    "workspace-evaluation.mjs", "workspace-evaluation-worker.mjs", "workspace-workbench.mjs", "workspace-workbench-worker.mjs", "workspace-storage.mjs", "workspace-session.mjs"];
  for (const name of scripts) copy(resolve(repository, "scripts", name), resolve(runtime, "scripts", name));
  copy(resolve(repository, "target/m98/workspace-runtime.mjs"), resolve(runtime, "assets/workspace-runtime.mjs"));
  copy(resolve(repository, "crates/geosolve-demo-web/frontend/src/generated"), resolve(runtime, "assets/demo-wasm"));
  copy(dist, resolve(runtime, "assets/workbench"));
  for (const name of ["geosolve.json", "sketch.ts"]) copy(resolve(repository, "examples/file-workspace", name), resolve(runtime, "assets/starter", name));
  const esbuild = resolve(repository, "crates/geosolve-demo-web/frontend/node_modules/esbuild");
  for (const path of ["lib/main.js", "lib/main.d.ts", "LICENSE.md"]) copy(resolve(esbuild, path), resolve(runtime, "vendor/esbuild", path));
  // Enable ordinary Node package self-reference from a vendor directory. This
  // keeps esbuild's JS unmodified while its own binary fallback resolves itself.
  json(resolve(runtime, "vendor/esbuild/package.json"), { ...readJson(resolve(esbuild, "package.json")), exports: { ".": "./lib/main.js" } });
  const binary = resolve(esbuild, "bin/esbuild");
  const binaryVersion = spawnSync(binary, ["--version"], { encoding: "utf8" });
  if (binaryVersion.status !== 0 || binaryVersion.stdout.trim() !== readJson(resolve(esbuild, "package.json")).version) throw Error("Prepared esbuild binary does not match its JavaScript package");
  // esbuild's documented package fallback works without installation scripts or
  // fetching optional dependencies. Keep the binary with its original JS package.
  copyFileSync(binary, resolve(runtime, `vendor/esbuild/lib/downloaded-@esbuild-linux-${process.arch}-esbuild`));
  chmodSync(resolve(runtime, `vendor/esbuild/lib/downloaded-@esbuild-linux-${process.arch}-esbuild`), 0o755);
  json(resolve(runtime, "runtime-layout.json"), { format: "geosolve-cli-runtime-v1", platform: process.platform, arch: process.arch });
  const notices = ["# Third-party runtime notices\n", "The CLI's GeoSolve code is GPL-3.0-or-later.\n",
    "## esbuild (MIT)\n", readFileSync(resolve(esbuild, "LICENSE.md"), "utf8"),
    "## Frozen workbench and compiler notices\n", readFileSync(resolve(dist, "THIRD_PARTY_LICENSES.md"), "utf8")];
  writeFileSync(resolve(output, "THIRD_PARTY_LICENSES.md"), notices.join("\n") + "\n", { flag: "wx" });
  return metadata;
}

/** Package only prepared product bytes; qualification owns all builds and gates. */
export function packageM98({ out, dist = resolve(repository, "crates/geosolve-demo-web/dist") }) {
  if (!out) throw Error("Expected --out <new directory> and optionally --dist <frozen production directory>");
  if (process.platform !== "linux" || !["x64", "arm64"].includes(process.arch)) throw Error("CLI packaging currently supports Linux x64 and arm64 hosts");
  const output = resolve(out), distribution = resolve(dist);
  if (existsSync(output)) throw Error(`Refusing to overwrite package output: ${output}`);
  for (const path of ["index.html", "LICENSE", "THIRD_PARTY_LICENSES.md"]) if (!existsSync(resolve(distribution, path))) throw Error(`Prepared workbench is missing ${path}`);
  mkdirSync(output, { recursive: true });
  const staging = resolve(output, "staging"); mkdirSync(staging);
  const sdkRoot = resolve(staging, "sketch-code"), engineRoot = resolve(staging, "engine"), cliRoot = resolve(staging, "cli");
  const sdk = stageSdk(sdkRoot), engine = stageEngine(engineRoot, sdk), cli = stageCli(cliRoot, sdk, engine, distribution);
  const archives = [];
  for (const [folder, metadata] of [[sdkRoot, sdk], [engineRoot, engine], [cliRoot, cli]]) {
    const stagedFiles = new Map(filesUnder(folder).map((file) => [file.path, file]));
    const result = spawnSync("npm", ["pack", "--offline", "--ignore-scripts", "--json", "--pack-destination", output], {
      cwd: folder, encoding: "utf8", env: { ...process.env, npm_config_update_notifier: "false", npm_config_audit: "false", npm_config_fund: "false" }, maxBuffer: 16 * 1024 * 1024,
    });
    if (result.status !== 0) throw Error(`npm pack ${metadata.name} failed: ${result.stderr || result.error}`);
    const [packed] = JSON.parse(result.stdout);
    const archive = resolve(output, packed.filename);
    const files = packed.files.map(({ path }) => {
      const file = stagedFiles.get(path);
      if (!file) throw Error(`Archive contains an unrecorded file: ${path}`);
      return file;
    }).sort((left, right) => left.path.localeCompare(right.path, "en"));
    archives.push({ name: metadata.name, version: metadata.version, file: packed.filename, bytes: lstatSync(archive).size, sha256: hash(readFileSync(archive)), files });
  }
  const manifest = { format: "geosolve-offline-packages-v1", platform: process.platform, arch: process.arch, node: process.version,
    workbench: filesUnder(distribution), archives };
  json(resolve(output, "packages.json"), manifest);
  rmSync(staging, { recursive: true });
  return { output, manifest };
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const { values } = parseArgs({ options: { out: { type: "string" }, dist: { type: "string" } } });
  const result = packageM98(values);
  console.log(JSON.stringify({ ok: true, output: result.output, archives: result.manifest.archives.map(({ name, file, bytes, sha256 }) => ({ name, file, bytes, sha256 })) }, null, 2));
}
