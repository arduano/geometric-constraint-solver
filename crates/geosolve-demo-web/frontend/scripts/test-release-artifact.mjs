// SPDX-License-Identifier: GPL-3.0-or-later

import assert from "node:assert/strict";
import { copyFile, cp, mkdir, mkdtemp, readFile, rm, symlink, writeFile } from "node:fs/promises";
import { createServer } from "node:http";
import { tmpdir } from "node:os";
import { resolve } from "node:path";
import { spawnSync } from "node:child_process";
import { test } from "node:test";
import { frontendDirectory, hash, publicBase, readManifest, validateManifest, writeManifest } from "./release-artifact-lib.mjs";
import { serveArtifact } from "./serve-artifact.mjs";
import { buildArtifacts, prepareWasm } from "./build-release-artifacts.mjs";
import { verifyTransport, verifyWorkbenchWasmLoaded } from "./verify-artifact.mjs";
const modules = ["geosolve_demo_web", "geosolve_sketch_engine_wasm", "geosolve_collaboration_wasm"];
const wasmBytes = module => Buffer.from([0, 97, 115, 109, 1, 0, 0, 0, 0, 2, 1, 97 + modules.indexOf(module)]);

async function fixture(t, { kind = "production", base = "./" } = {}) {
  const root = await mkdtemp(resolve(tmpdir(), "geosolve-release-artifact-test-"));
  t.after(() => rm(root, { recursive: true, force: true }));
  const directory = resolve(root, "geosolve-fixture");
  await mkdir(resolve(directory, "assets"), { recursive: true });
  const repository = resolve(frontendDirectory, "../../..");
  for (const [source, target] of [["LICENSE", "LICENSE"], ["THIRD_PARTY_LICENSES.md", "THIRD_PARTY_LICENSES.md"], ["docs/API_COMPATIBILITY.md", "API_COMPATIBILITY.md"]]) {
    await copyFile(resolve(repository, source), resolve(directory, target));
  }
  await writeFile(resolve(directory, "assets/index-12345678.js"), modules.map(module => `new URL("${module}_bg-12345678.wasm", import.meta.url);`).join("\n"));
  await writeFile(resolve(directory, "assets/index-12345678.css"), "body{}\n");
  for (const module of modules) await writeFile(resolve(directory, `assets/${module}_bg-12345678.wasm`), wasmBytes(module));
  await writeFile(resolve(directory, "index.html"), `<link rel="stylesheet" href="${base}assets/index-12345678.css"><script type="module" src="${base}assets/index-12345678.js"></script>`);
  if (kind === "harness") await writeFile(resolve(directory, "compiler-parity.html"), `<script type="module" src="${base}assets/index-12345678.js"></script>`);
  const manifest = resolve(root, "artifact.json");
  await writeManifest(directory, kind, base, manifest);
  return { root, directory, manifest, artifact: await readManifest(manifest) };
}

async function startServer(t, handler) {
  const server = createServer(handler);
  await new Promise((accept) => server.listen(0, "127.0.0.1", accept));
  t.after(() => new Promise((accept) => server.close(accept)));
  return `http://127.0.0.1:${server.address().port}/`;
}

test("ordinary readiness requires its exact workbench WASM among optional collaboration modules", () => {
  const baseUrl = "https://preview.example/geometric-constraint-solver/";
  const paths = [
    "assets/geosolve_collaboration_wasm_bg-CaFsKnrI.wasm",
    "assets/geosolve_demo_web_bg-BgawRWrn.wasm",
    "assets/geosolve_sketch_engine_wasm_bg-CdK2xYC8.wasm",
  ];
  const manifest = { files: paths.map((path) => ({ path })) };
  const demoUrl = new URL(paths[1], baseUrl).href;
  const observed = new Set([demoUrl]);
  assert.deepEqual(paths, [...paths].sort());
  assert.equal(verifyWorkbenchWasmLoaded(manifest, baseUrl, observed), demoUrl);
  assert.equal(verifyWorkbenchWasmLoaded({ files: [...manifest.files].reverse() }, baseUrl, observed), demoUrl);
  assert.equal(verifyWorkbenchWasmLoaded({ files: [manifest.files[1]] }, baseUrl, observed), demoUrl);
  const plain = "assets/geosolve_demo_web_bg.wasm";
  const plainUrl = new URL(plain, baseUrl).href;
  assert.equal(verifyWorkbenchWasmLoaded({ files: [{ path: plain }] }, baseUrl, new Set([plainUrl])), plainUrl);
  const optionalResponses = new Set([paths[0], paths[2]].map((path) => new URL(path, baseUrl).href));
  assert.throws(() => verifyWorkbenchWasmLoaded(manifest, baseUrl, optionalResponses), /did not load the nominated WASM/);
  assert.throws(() => verifyWorkbenchWasmLoaded(manifest, baseUrl, new Set([new URL(paths[1], "https://other.example/").href])), /did not load the nominated WASM/);
  assert.throws(() => verifyWorkbenchWasmLoaded({ files: [manifest.files[0], manifest.files[2]] }, baseUrl, optionalResponses), /exactly one nominated workbench WASM/);
  assert.throws(() => verifyWorkbenchWasmLoaded({ files: [...manifest.files, { path: plain }] }, baseUrl, new Set([...observed, plainUrl])), /exactly one nominated workbench WASM/);
});

test("an authenticated production copy serves identical bytes at its declared base without the harness route", async (t) => {
  const f = await fixture(t, { base: "/geometric-constraint-solver/" });
  const copy = resolve(f.root, "geosolve-moved");
  await cp(f.directory, copy, { recursive: true });
  const { server, baseUrl } = await serveArtifact(f.manifest, { directory: copy, port: 0 });
  t.after(() => new Promise((accept) => server.close(accept)));
  const rows = await verifyTransport(await readManifest(f.manifest, copy), baseUrl);
  assert.equal(rows.length, f.artifact.manifest.files.length + 1);
  assert.equal(rows.filter((row) => row.contentType === "application/wasm").length, 3);
  assert.equal((await fetch(new URL("compiler-parity.html", baseUrl))).status, 404);
  assert.equal((await fetch(new URL("missing.js", baseUrl))).status, 404);
  assert.equal((await fetch(new URL("/", baseUrl))).status, 404);
  await assert.rejects(verifyTransport(f.artifact, new URL("/", baseUrl).href), /public base/);
});

test("harness manifest preserves its separate compiler entrypoint and production validation rejects it", async (t) => {
  const f = await fixture(t, { kind: "harness" });
  const result = spawnSync(process.execPath, [resolve(frontendDirectory, "scripts/validate-dist.mjs"), f.directory, "./"], { encoding: "utf8" });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /unexpected root file: compiler-parity.html/);
  const changed = structuredClone(f.artifact.manifest);
  changed.kind = "production";
  assert.throws(() => validateManifest(changed), /identity mismatch/);
});

test("mutated, missing, additional, symlink and malformed artifact inputs fail closed", async (t) => {
  const f = await fixture(t);
  const file = resolve(f.directory, "assets/index-12345678.js");
  const original = await readFile(file);
  await writeFile(file, "changed");
  await assert.rejects(readManifest(f.manifest), /bytes do not match/);
  await rm(file);
  await assert.rejects(readManifest(f.manifest), /bytes do not match/);
  await writeFile(file, original);
  await writeFile(resolve(f.directory, "assets/extra-12345678.js"), "extra");
  await assert.rejects(readManifest(f.manifest), /bytes do not match/);
  await rm(resolve(f.directory, "assets/extra-12345678.js"));
  await mkdir(resolve(f.directory, "unexpected"));
  await assert.rejects(readManifest(f.manifest), /unexpected artifact directory/);
  await rm(resolve(f.directory, "unexpected"), { recursive: true });
  await symlink(file, resolve(f.directory, "assets/link-12345678.js"));
  await assert.rejects(readManifest(f.manifest), /symlink/);
  for (const mutation of [
    (m) => { m.format = "unknown"; },
    (m) => { m.files[0].path = "../escape"; },
    (m) => { m.files.push(m.files[0]); },
    (m) => { m.files[0].sha256 = "0".repeat(64); },
    (m) => { m.totalBytes += 1; },
    (m) => { m.extra = true; },
  ]) {
    const changed = structuredClone(f.artifact.manifest);
    mutation(changed);
    assert.throws(() => validateManifest(changed));
  }
});

test("HTTP redirects, MIME drift and changed bytes cannot borrow artifact evidence", async (t) => {
  const f = await fixture(t);
  for (const [name, handler, expected] of [
    ["redirect", (_request, response) => { response.writeHead(302, { Location: "/index.html" }); response.end(); }, /HTTP 302 or redirect/],
    ["type", (_request, response) => { response.writeHead(200, { "Content-Type": "text/plain" }); response.end("wrong"); }, /expected text\/html/],
    ["bytes", (_request, response) => { response.writeHead(200, { "Content-Type": "text/html" }); response.end("changed"); }, /HTTP bytes differ/],
  ]) {
    await t.test(name, async (subtest) => {
      await assert.rejects(verifyTransport(f.artifact, await startServer(subtest, handler)), expected);
    });
  }
});

test("artifact publication and build preparation never overwrite existing evidence", async (t) => {
  const f = await fixture(t);
  const previous = await readFile(f.manifest);
  await assert.rejects(writeManifest(f.directory, "production", "./", f.manifest), /EEXIST/);
  assert.deepEqual(await readFile(f.manifest), previous);
  const result = spawnSync(process.execPath, [resolve(frontendDirectory, "scripts/build-release-artifacts.mjs"), "--out", f.root], { encoding: "utf8" });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /EEXIST/);
  assert.equal(result.stdout, "");
});

test("manifest digest binds exact file metadata, not only aggregate byte counts", async (t) => {
  const f = await fixture(t);
  assert.equal(f.artifact.manifest.filesSha256, hash(JSON.stringify(f.artifact.manifest.files)));
  const changed = structuredClone(f.artifact.manifest);
  [changed.files[0].sha256, changed.files[1].sha256] = [changed.files[1].sha256, changed.files[0].sha256];
  assert.throws(() => validateManifest(changed), /digest mismatch/);
});


test("public-base validation rejects protocol-relative and traversal paths", () => {
  for (const value of [undefined, "//remote/", "/../", "/a/./", "/a?b/", "/a%2fb/", "relative/"]) {
    assert.throws(() => publicBase(value), /invalid public base/);
  }
  assert.equal(publicBase("./"), "./");
  assert.equal(publicBase("/repository/"), "/repository/");
});


test("prepared Playwright discovery retains every stable sample and shared workflow and refuses a production manifest", async (t) => {
  const f = await fixture(t, { kind: "harness" });
  const env = { ...process.env, GEOSOLVE_E2E_ARTIFACT_MANIFEST: f.manifest };
  delete env.GEOSOLVE_E2E_BASE_URL;
  const command = resolve(frontendDirectory, "node_modules/.bin/playwright");
  const listed = spawnSync(command, ["test", "--list"], { cwd: frontendDirectory, env, encoding: "utf8" });
  assert.equal(listed.status, 0, listed.stderr);
  const catalog = JSON.parse(await readFile(resolve(frontendDirectory, "../../geosolve-sketch-code/assets/bundled-sample-catalog.json"), "utf8"));
  const inventory = JSON.parse(await readFile(resolve(frontendDirectory, "../../../scripts/release_test_inventory.json"), "utf8"));
  const shared = inventory.browser_non_sample_cases;
  const sharedFiles = new Set(shared.map(([file]) => file));
  assert.match(listed.stdout, new RegExp(`Total: ${catalog.samples.length + shared.length} tests in ${sharedFiles.size + 1} files`));
  const sharedRows = [...listed.stdout.matchAll(/^  \[([^\]]+)\] › ([^:]+):\d+:\d+ › (.+)$/gm)]
    .filter(([, , file]) => file !== "m92-sample-audit.spec.ts")
    .map(([, project, file, title]) => [file, title, project]);
  const ordered = (rows) => rows.map((row) => JSON.stringify(row)).sort();
  assert.deepEqual(ordered(sharedRows), ordered(shared));
  const sampleRows = [...listed.stdout.matchAll(/\[(chromium(?:-memory)?)\].*M92 visual workflow: ([a-z0-9-]+)\n/g)];
  assert.deepEqual(sampleRows.map((row) => row[2]).sort(), catalog.samples.map(({ key }) => key).sort());
  const heavy = new Set(["perforated-fixture-field", "robotic-harness-backplane", "curves-contact-continuity-atlas", "fabrication-operations-atlas"]);
  for (const [, project, key] of sampleRows) assert.equal(project, heavy.has(key) ? "chromium-memory" : "chromium");
  const prefix = spawnSync(command, ["test", "tests/e2e/m92-sample-audit.spec.ts", "--list"], {
    cwd: frontendDirectory, env: { ...env, GEOSOLVE_BROWSER_PREFIX_ONLY: "1" }, encoding: "utf8",
  });
  assert.equal(prefix.status, 0, prefix.stderr);
  assert.deepEqual([...prefix.stdout.matchAll(/M92 visual workflow: ([a-z0-9-]+)\n/g)].map((row) => row[1]).sort(), catalog.samples.map(({ key }) => key).sort());
  const production = await fixture(t);
  env.GEOSOLVE_E2E_ARTIFACT_MANIFEST = production.manifest;
  const rejected = spawnSync(command, ["test", "--list"], { cwd: frontendDirectory, env, encoding: "utf8" });
  assert.notEqual(rejected.status, 0);
  assert.match(rejected.stderr, /requires a prepared compiler-harness artifact/);
});


async function wasmPackageFixture(t) {
  const root = await mkdtemp(resolve(tmpdir(), "geosolve-prepared-wasm-test-"));
  t.after(() => rm(root, { recursive: true, force: true }));
  const frontendRoot = resolve(root, "frontend");
  const packageDirectory = resolve(root, "package");
  await mkdir(resolve(frontendRoot, "src/generated"), { recursive: true });
  await mkdir(packageDirectory);
  const files = {
    "geosolve_demo_web_bg.wasm": wasmBytes("geosolve_demo_web"),
    "geosolve_demo_web.js": "export class WorkbenchHandle {}\n",
    "geosolve_demo_web.d.ts": "export class WorkbenchHandle {}\n",
    "geosolve_demo_web_bg.wasm.d.ts": "export const memory: WebAssembly.Memory;\n",
  };
  for (const [name, bytes] of Object.entries(files)) await writeFile(resolve(packageDirectory, name), bytes);
  const moduleDirectories = {};
  for (const module of modules.slice(1)) {
    const directory = resolve(root, module); await mkdir(directory); moduleDirectories[module] = directory;
    for (const [name, bytes] of Object.entries(files)) await writeFile(resolve(directory, name.replace("geosolve_demo_web", module)), name.endsWith(".wasm") ? wasmBytes(module) : bytes);
  }
  await writeFile(resolve(frontendRoot, "src/generated/stale.js"), "old binding");
  return { root, frontendRoot, packageDirectory, files, moduleDirectories };
}

test("prepared WASM supplies both browser bundles without invoking the Rust or WASM build", async (t) => {
  const f = await wasmPackageFixture(t);
  const harness = await fixture(t, { kind: "harness" });
  const production = await fixture(t);
  const commands = [];
  const output = resolve(f.root, "browser");
  await buildArtifacts({ out: output, wasmPackage: f.packageDirectory }, {
    frontendRoot: f.frontendRoot, moduleDirectories: f.moduleDirectories,
    commandRunner: async (command, args, options) => {
      commands.push([command, ...args]);
      for (const [name, bytes] of Object.entries(f.files)) assert.deepEqual(await readFile(resolve(f.frontendRoot, "src/generated", name)), Buffer.from(bytes));
      if (command.endsWith("/vite")) {
        await cp(options.env.GEOSOLVE_BROWSER_COMPILER_HARNESS === "1" ? harness.directory : production.directory, options.env.GEOSOLVE_DIST, { recursive: true });
      } else assert.deepEqual([command, ...args], ["npm", "run", "check:types"]);
    },
  });
  assert.equal(commands.length, 3);
  const build = JSON.parse(await readFile(resolve(output, "build.json"), "utf8"));
  assert.equal(build.wasmPackageProvenance.source, "prepared-package");
  assert.equal(build.wasmPackageProvenance.filesSha256, hash(JSON.stringify(build.wasmPackageProvenance.files)));
  assert.equal(build.wasmPackageProvenance.files.length, 4);
  assert.equal(build.optimizedWasmSha256, hash(f.files["geosolve_demo_web_bg.wasm"]));
  for (const manifest of [build.harnessManifest, build.productionManifest]) {
    const files = (await readManifest(manifest)).manifest.files;
    for (const module of modules) {
      assert.equal(files.find(file => file.path.includes(`${module}_bg-`)).sha256, build.wasmModules[module].optimizedWasmSha256);
      assert.equal(build.wasmModules[module].optimizedWasmSha256, hash(wasmBytes(module)));
      assert.equal(build.wasmModules[module].filesSha256, hash(JSON.stringify(build.wasmModules[module].files)));
    }
  }
  await assert.rejects(readFile(resolve(f.frontendRoot, "src/generated/stale.js")), /ENOENT/);
  for (const [name, bytes] of Object.entries(f.files)) assert.deepEqual(await readFile(resolve(f.packageDirectory, name)), Buffer.from(bytes));
});

test("standalone preparation still runs wasm:release and records all generated binding bytes", async (t) => {
  const f = await wasmPackageFixture(t);
  const commands = [];
  const result = await prepareWasm({
    frontendRoot: f.frontendRoot,
    commandRunner: async (command, args) => {
      commands.push([command, ...args]);
      await rm(resolve(f.frontendRoot, "src/generated"), { recursive: true });
      await cp(f.packageDirectory, resolve(f.frontendRoot, "src/generated"), { recursive: true });
    },
  });
  assert.deepEqual(commands, [["npm", "run", "wasm:release"]]);
  assert.equal(result.source, "wasm:release");
  const prepared = await prepareWasm({ packageDirectory: f.packageDirectory, frontendRoot: f.frontendRoot, commandRunner: () => assert.fail("unexpected compiler") });
  assert.equal(prepared.filesSha256, result.filesSha256);
  await writeFile(resolve(f.packageDirectory, "geosolve_demo_web.js"), "changed binding");
  const changed = await prepareWasm({ packageDirectory: f.packageDirectory, frontendRoot: f.frontendRoot });
  assert.equal(changed.optimizedWasmSha256, result.optimizedWasmSha256);
  assert.notEqual(changed.filesSha256, result.filesSha256);
});

test("invalid prepared WASM packages preserve prior generated output and never invoke a compiler", async (t) => {
  for (const kind of ["missing", "empty", "header", "symlink"]) {
    await t.test(kind, async (subtest) => {
      const f = await wasmPackageFixture(subtest);
      const binding = resolve(f.packageDirectory, "geosolve_demo_web.js");
      if (kind === "missing") await rm(binding);
      if (kind === "empty") await writeFile(binding, "");
      if (kind === "header") await writeFile(resolve(f.packageDirectory, "geosolve_demo_web_bg.wasm"), "invalid header");
      if (kind === "symlink") {
        await rm(binding);
        await symlink(resolve(f.packageDirectory, "geosolve_demo_web.d.ts"), binding);
      }
      await assert.rejects(prepareWasm({ packageDirectory: f.packageDirectory, frontendRoot: f.frontendRoot, commandRunner: () => assert.fail("unexpected compiler") }), /WASM package/);
      assert.equal(await readFile(resolve(f.frontendRoot, "src/generated/stale.js"), "utf8"), "old binding");
    });
  }
});

test("three-module provenance refuses substituted engine/collaboration bytes and changed binding packages",async t=>{
  for(const kind of ["engine-bytes","collaboration-bytes","binding-race","demo-binding-race"]){
    await t.test(kind,async subtest=>{
      const f=await wasmPackageFixture(subtest),harness=await fixture(subtest,{kind:"harness"});
      if(kind.endsWith("-bytes")){
        const module=kind==="engine-bytes"?"geosolve_sketch_engine_wasm":"geosolve_collaboration_wasm";
        await writeFile(resolve(harness.directory,`assets/${module}_bg-12345678.wasm`),wasmBytes("geosolve_demo_web"));
      }
      await assert.rejects(buildArtifacts({out:resolve(f.root,"refused"),wasmPackage:f.packageDirectory},{
        frontendRoot:f.frontendRoot,moduleDirectories:f.moduleDirectories,
        commandRunner:async(command,_args,options)=>{
          if(command.endsWith("/vite")){
            await cp(harness.directory,options.env.GEOSOLVE_DIST,{recursive:true});
            if(kind==="binding-race")await writeFile(resolve(f.moduleDirectories.geosolve_collaboration_wasm,"geosolve_collaboration_wasm.js"),"changed binding");
            if(kind==="demo-binding-race")await writeFile(resolve(f.frontendRoot,"src/generated/geosolve_demo_web.js"),"changed binding");
          }
        },
      }),/did not preserve exact prepared|package changed/u);
      await assert.rejects(readFile(resolve(f.root,"refused/build.json")),/ENOENT/u);
    });
  }
});

test("standalone artifact preparation builds engine and collaboration packages before bundling",async t=>{
  const f=await wasmPackageFixture(t),harness=await fixture(t,{kind:"harness"}),production=await fixture(t),commands=[];
  await buildArtifacts({out:resolve(f.root,"standalone")},{frontendRoot:f.frontendRoot,moduleDirectories:f.moduleDirectories,
    commandRunner:async(command,args,options)=>{
      commands.push([command,...args]);
      if(command==="npm"&&args.includes("wasm:release")){
        await rm(resolve(f.frontendRoot,"src/generated"),{recursive:true});await cp(f.packageDirectory,resolve(f.frontendRoot,"src/generated"),{recursive:true});
      }
      if(command.endsWith("/vite"))await cp(options.env.GEOSOLVE_BROWSER_COMPILER_HARNESS==="1"?harness.directory:production.directory,options.env.GEOSOLVE_DIST,{recursive:true});
    },
  });
  assert.deepEqual(commands.slice(0,6),[
    ["npm","run","wasm:release"],["node","packages/geosolve-engine/scripts/build-wasm.mjs"],["node","packages/geosolve-engine/scripts/build.mjs"],
    ["node","packages/geosolve-collaboration/scripts/build-wasm.mjs"],["node","packages/geosolve-collaboration/scripts/build.mjs"],["npm","run","check:types"],
  ]);
});
