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
import { verifyTransport } from "./verify-artifact.mjs";

async function fixture(t, { kind = "production", base = "./" } = {}) {
  const root = await mkdtemp(resolve(tmpdir(), "geosolve-release-artifact-test-"));
  t.after(() => rm(root, { recursive: true, force: true }));
  const directory = resolve(root, "geosolve-fixture");
  await mkdir(resolve(directory, "assets"), { recursive: true });
  const repository = resolve(frontendDirectory, "../../..");
  for (const [source, target] of [["LICENSE", "LICENSE"], ["THIRD_PARTY_LICENSES.md", "THIRD_PARTY_LICENSES.md"], ["docs/API_COMPATIBILITY.md", "API_COMPATIBILITY.md"]]) {
    await copyFile(resolve(repository, source), resolve(directory, target));
  }
  await writeFile(resolve(directory, "assets/index-12345678.js"), 'new URL("module-12345678.wasm", import.meta.url);\n');
  await writeFile(resolve(directory, "assets/index-12345678.css"), "body{}\n");
  await writeFile(resolve(directory, "assets/module-12345678.wasm"), Buffer.from([0, 97, 115, 109, 1, 0, 0, 0]));
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

test("an authenticated production copy serves identical bytes at its declared base without the harness route", async (t) => {
  const f = await fixture(t, { base: "/geometric-constraint-solver/" });
  const copy = resolve(f.root, "geosolve-moved");
  await cp(f.directory, copy, { recursive: true });
  const { server, baseUrl } = await serveArtifact(f.manifest, { directory: copy, port: 0 });
  t.after(() => new Promise((accept) => server.close(accept)));
  const rows = await verifyTransport(await readManifest(f.manifest, copy), baseUrl);
  assert.equal(rows.length, f.artifact.manifest.files.length + 1);
  assert.equal(rows.filter((row) => row.contentType === "application/wasm").length, 1);
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


test("prepared Playwright discovery retains all 37 rows and refuses a production manifest", async (t) => {
  const f = await fixture(t, { kind: "harness" });
  const env = { ...process.env, GEOSOLVE_E2E_ARTIFACT_MANIFEST: f.manifest };
  delete env.GEOSOLVE_E2E_BASE_URL;
  const command = resolve(frontendDirectory, "node_modules/.bin/playwright");
  const listed = spawnSync(command, ["test", "--list"], { cwd: frontendDirectory, env, encoding: "utf8" });
  assert.equal(listed.status, 0, listed.stderr);
  assert.match(listed.stdout, /Total: 37 tests in 3 files/);
  assert.equal((listed.stdout.match(/M92 visual workflow/g) ?? []).length, 16);
  assert.equal((listed.stdout.match(/workbench\.spec\.ts:/g) ?? []).length, 20);
  assert.equal((listed.stdout.match(/language-service\.spec\.ts:/g) ?? []).length, 1);
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
    "geosolve_demo_web_bg.wasm": Buffer.from([0, 97, 115, 109, 1, 0, 0, 0]),
    "geosolve_demo_web.js": "export class WorkbenchHandle {}\n",
    "geosolve_demo_web.d.ts": "export class WorkbenchHandle {}\n",
    "geosolve_demo_web_bg.wasm.d.ts": "export const memory: WebAssembly.Memory;\n",
  };
  for (const [name, bytes] of Object.entries(files)) await writeFile(resolve(packageDirectory, name), bytes);
  await writeFile(resolve(frontendRoot, "src/generated/stale.js"), "old binding");
  return { root, frontendRoot, packageDirectory, files };
}

test("prepared WASM supplies both browser bundles without invoking the Rust or WASM build", async (t) => {
  const f = await wasmPackageFixture(t);
  const harness = await fixture(t, { kind: "harness" });
  const production = await fixture(t);
  const commands = [];
  const output = resolve(f.root, "browser");
  await buildArtifacts({ out: output, wasmPackage: f.packageDirectory }, {
    frontendRoot: f.frontendRoot,
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
    assert.equal((await readManifest(manifest)).manifest.files.find((file) => file.path.endsWith(".wasm")).sha256, build.optimizedWasmSha256);
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
