// SPDX-License-Identifier: GPL-3.0-or-later

import { lstat, mkdir, mkdtemp, readFile, readdir, rename, rm, writeFile } from "node:fs/promises";
import { dirname, relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";
import { parseArgs } from "node:util";
import { frontendDirectory, hash, publicBase, run, writeManifest } from "./release-artifact-lib.mjs";

async function readWasmPackage(directory, module = "geosolve_demo_web") {
  const root = await lstat(directory);
  if (!root.isDirectory() || root.isSymbolicLink()) throw new Error("WASM package must be a real directory");
  const files = [];
  async function walk(path) {
    for (const name of (await readdir(path)).sort()) {
      const absolute = resolve(path, name);
      const info = await lstat(absolute);
      if (info.isSymbolicLink()) throw new Error(`WASM package contains a symlink: ${absolute}`);
      if (info.isDirectory()) await walk(absolute);
      else if (info.isFile()) files.push({ path: relative(directory, absolute).split(sep).join("/"), contents: await readFile(absolute) });
      else throw new Error(`WASM package contains a non-file entry: ${absolute}`);
    }
  }
  await walk(directory);
  files.sort((a, b) => a.path < b.path ? -1 : a.path > b.path ? 1 : 0);
  const wasmName = `${module}_bg.wasm`;
  for (const name of [wasmName, `${module}.js`, `${module}.d.ts`, `${module}_bg.wasm.d.ts`]) {
    if (!files.some((file) => file.path === name && file.contents.length > 0)) throw new Error(`WASM package missing required nonempty file: ${name}`);
  }
  const wasm = files.find((file) => file.path === wasmName).contents;
  if (!wasm.subarray(0, 8).equals(Buffer.from([0, 97, 115, 109, 1, 0, 0, 0]))) throw new Error("WASM package contains an invalid WASM header");
  // Hash the same captured bytes that are installed. The gate separately authenticates
  // this package's input identity and build receipt before supplying --wasm-package.
  const inventory = files.map(({ path, contents }) => ({ path, bytes: contents.length, sha256: hash(contents) }));
  return { files, inventory, filesSha256: hash(JSON.stringify(inventory)), optimizedWasmSha256: hash(wasm) };
}

export async function prepareWasm({ packageDirectory, frontendRoot = frontendDirectory, commandRunner = run } = {}) {
  const generated = resolve(frontendRoot, "src/generated");
  if (!packageDirectory) await commandRunner("npm", ["run", "wasm:release"], { cwd: frontendRoot });
  const source = packageDirectory ? resolve(packageDirectory) : generated;
  const snapshot = await readWasmPackage(source);
  if (packageDirectory) {
    // Capture before replacing mutable generated output; an invalid package cannot
    // erase the previously usable bindings, and no compiler runs on this path.
    const staging = await mkdtemp(resolve(dirname(generated), ".generated-prepared-"));
    try {
      for (const file of snapshot.files) {
        const target = resolve(staging, file.path);
        await mkdir(dirname(target), { recursive: true });
        await writeFile(target, file.contents, { flag: "wx" });
      }
      await rm(generated, { recursive: true, force: true });
      await rename(staging, generated);
    } finally {
      await rm(staging, { recursive: true, force: true });
    }
  }
  return {
    source: packageDirectory ? "prepared-package" : "wasm:release",
    directory: source, files: snapshot.inventory, filesSha256: snapshot.filesSha256,
    optimizedWasmSha256: snapshot.optimizedWasmSha256,
  };
}

export async function buildArtifacts({ out, base: requestedBase = "./", wasmPackage }, { frontendRoot = frontendDirectory, commandRunner = run, moduleDirectories } = {}) {
  if (!out) throw new Error("usage: node scripts/build-release-artifacts.mjs --out <new-directory> [--base ./] [--wasm-package <directory>]");
  const output = resolve(out);
  const base = publicBase(requestedBase);
  // A stage owns this new directory. Failed/incomplete output is preserved for its
  // receipt; callers resume into another new path instead of overwriting evidence.
  await mkdir(output);
  const artifacts = [
    { kind: "harness", base: "./", directory: resolve(output, "geosolve-harness"), manifest: resolve(output, "harness.json") },
    { kind: "production", base, directory: resolve(output, "geosolve-production"), manifest: resolve(output, "production.json") },
  ];

  // The scheduler serializes this preparation with every writer of src/generated
  // and node_modules. Both Vite bundles consume the same optimized WASM package.
  const wasmPackageProvenance = await prepareWasm({ packageDirectory: wasmPackage, frontendRoot, commandRunner });
  const { optimizedWasmSha256 } = wasmPackageProvenance;
  const repository = resolve(frontendRoot, "../../..");
  if (!wasmPackage) {
    for (const packageName of ["geosolve-engine", "geosolve-collaboration"]) {
      for (const script of ["build-wasm.mjs", "build.mjs"]) await commandRunner("node", [`packages/${packageName}/scripts/${script}`], { cwd: repository });
    }
  }
  const directories = moduleDirectories ?? {
    geosolve_sketch_engine_wasm: resolve(repository, "packages/geosolve-engine/dist/wasm"),
    geosolve_collaboration_wasm: resolve(repository, "packages/geosolve-collaboration/dist/wasm"),
  };
  if (Object.keys(directories).sort().join(",") !== "geosolve_collaboration_wasm,geosolve_sketch_engine_wasm") throw Error("Expected exact engine and collaboration native packages");
  const wasmModules = { geosolve_demo_web: wasmPackageProvenance };
  for (const [module, directory] of Object.entries(directories)) {
    const snapshot = await readWasmPackage(directory, module);
    wasmModules[module] = { source: "prepared-runtime", directory, files: snapshot.inventory, filesSha256: snapshot.filesSha256, optimizedWasmSha256: snapshot.optimizedWasmSha256 };
  }
  await commandRunner("npm", ["run", "check:types"], { cwd: frontendRoot });
  for (const artifact of artifacts) {
    await commandRunner(resolve(frontendRoot, "node_modules/.bin/vite"), ["build"], {
      cwd: frontendRoot,
      env: { ...process.env, GEOSOLVE_PUBLIC_BASE: artifact.base, GEOSOLVE_DIST: artifact.directory,
        GEOSOLVE_BROWSER_COMPILER_HARNESS: artifact.kind === "harness" ? "1" : "0", VITE_GEOSOLVE_MOCK: "0" },
    });
    const manifest = await writeManifest(artifact.directory, artifact.kind, artifact.base, artifact.manifest);
    const wasm = manifest.files.filter(file => file.path.endsWith(".wasm"));
    if (wasm.length !== Object.keys(wasmModules).length) throw new Error(`${artifact.kind} must contain exactly three prepared native WASM modules`);
    for (const [module, provenance] of Object.entries(wasmModules)) {
      const matches = wasm.filter(file => new RegExp(`(?:^|/)${module}_bg-[A-Za-z0-9_-]+\\.wasm$`, "u").test(file.path));
      if (matches.length !== 1 || matches[0].sha256 !== provenance.optimizedWasmSha256) throw new Error(`${artifact.kind} did not preserve exact prepared ${module} WASM bytes`);
    }
    // Bindings are code inputs too; a concurrent writer must not alter any
    // package while harness and production artifacts are consuming it.
    for (const [module, directory] of Object.entries({ geosolve_demo_web: resolve(frontendRoot, "src/generated"), ...directories })) {
      if ((await readWasmPackage(directory, module)).filesSha256 !== wasmModules[module].filesSha256) throw Error(`Prepared ${module} package changed during browser build`);
    }
  }
  await writeFile(resolve(output, "build.json"), `${JSON.stringify({
    format: "geosolve-release-browser-build-v1", optimizedWasmSha256, wasmPackageProvenance, wasmModules,
    harnessManifest: artifacts[0].manifest, productionManifest: artifacts[1].manifest,
  }, null, 2)}\n`, { flag: "wx" });
  console.log(`prepared one harness and one production distribution in ${output}`);
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const { values } = parseArgs({ options: { out: { type: "string" }, base: { type: "string", default: "./" }, "wasm-package": { type: "string" } } });
  await buildArtifacts({ out: values.out, base: values.base, wasmPackage: values["wasm-package"] });
}
