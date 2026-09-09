// SPDX-License-Identifier: GPL-3.0-or-later
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { spawn, spawnSync } from "node:child_process";
import { cpSync, mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { createServer } from "node:http";
import { tmpdir } from "node:os";
import { extname, resolve, sep } from "node:path";
import test from "node:test";
import { packageM98, repository } from "./package-m98.mjs";

const digest = (bytes) => createHash("sha256").update(bytes).digest("hex");
const fixture = `import { defineGenerator, sketch, mm } from "@geosolve/sketch-code";
export default defineGenerator({ radius: { type: "number", default: 5, min: 1, max: 10, unit: "mm", label: "Radius" } }, ({radius}) => sketch(($) => ({ ring: $.geometry.centerRadiusCircle("ring", { center: [0, 0], radius: mm(radius) }) })));
`;

function run(command, args, options) {
  const result = spawnSync(command, args, { encoding: "utf8", timeout: 90000, maxBuffer: 8 * 1024 * 1024, ...options });
  assert.equal(result.status, 0, `${command} ${args.join(" ")} failed: ${result.error ?? ""}\n${result.stderr ?? ""}\n${result.stdout ?? ""}`);
  return result.stdout;
}

test("three offline archives install into an empty cache and run the actual SDK, engine and folder CLI", { timeout: 180000 }, async (t) => {
  const temporary = mkdtempSync(resolve(tmpdir(), "geosolve-package-"));
  t.after(() => rmSync(temporary, { recursive: true, force: true }));
  const archiveDirectory = process.env.GEOSOLVE_M98_PACKAGES
    ? resolve(process.env.GEOSOLVE_M98_PACKAGES)
    : packageM98({ out: process.env.GEOSOLVE_M98_PACKAGE_OUT ?? resolve(temporary, "archives"), ...(process.env.GEOSOLVE_M98_DIST ? { dist: process.env.GEOSOLVE_M98_DIST } : {}) }).output;
  const manifest = JSON.parse(readFileSync(resolve(archiveDirectory, "packages.json"), "utf8"));
  assert.equal(manifest.format, "geosolve-offline-packages-v1");
  assert.equal(manifest.archives.length, 3);
  const archives = manifest.archives.map((archive) => {
    const path = resolve(archiveDirectory, archive.file);
    assert.equal(digest(readFileSync(path)), archive.sha256);
    return path;
  });
  const installed = resolve(temporary, "consumer"); mkdirSync(installed);
  writeFileSync(resolve(installed, "package.json"), JSON.stringify({ private: true, type: "module" }));
  const environment = { ...process.env, NODE_PATH: "", NODE_OPTIONS: "", npm_config_cache: resolve(temporary, "empty-cache"), npm_config_audit: "false", npm_config_fund: "false", npm_config_update_notifier: "false" };
  delete environment.GEOSOLVE_DIST;
  run("npm", ["install", "--offline", "--ignore-scripts", "--no-audit", "--no-fund", ...archives], { cwd: installed, env: environment });
  const nativeEsbuild = resolve(installed, `node_modules/@geosolve/cli/runtime/vendor/esbuild/lib/downloaded-@esbuild-linux-${process.arch}-esbuild`);
  assert.equal(readFileSync(nativeEsbuild).subarray(0, 4).toString("hex"), "7f454c46", "vendored esbuild must be the native ELF executable, not npm's forwarding launcher");
  const esbuildPackage = JSON.parse(readFileSync(resolve(installed, "node_modules/@geosolve/cli/runtime/vendor/esbuild/package.json"), "utf8"));
  assert.equal(run(nativeEsbuild, ["--version"], { cwd: installed, env: environment, timeout: 5000 }).trim(), esbuildPackage.version);
  const sdkPackage = JSON.parse(readFileSync(resolve(installed, "node_modules/@geosolve/sketch-code/package.json"), "utf8"));
  assert.equal(sdkPackage.dependencies.typescript, "5.9.2");
  assert.equal(sdkPackage.dependencies["@geosolve/intent"], "0.2.0");
  for (const packageName of ["sketch-code", "engine", "cli"]) {
    const metadata = JSON.parse(readFileSync(resolve(installed, `node_modules/@geosolve/${packageName}/package.json`), "utf8"));
    assert.ok(Object.values(metadata.dependencies ?? {}).every((version) => !version.startsWith("file:")), "archive dependencies never reference sibling checkout paths");
  }
  const bin = resolve(installed, "node_modules/.bin/geosolve");
  const cli = (...args) => JSON.parse(run(bin, args, { cwd: installed, env: environment }));
  const starter = resolve(installed, "starter");
  assert.equal(cli("init", starter).ok, true);
  assert.equal(cli("inspect", starter).mode, "editable");
  const starterCheck = cli("check", starter);
  assert.equal(starterCheck.ok, true, JSON.stringify(starterCheck));
  const project = resolve(installed, "generator"); mkdirSync(project);
  writeFileSync(resolve(project, "geosolve.json"), JSON.stringify({ format: "geosolve-folder-v2", mode: "generator", entry: "generator.ts" }));
  writeFileSync(resolve(project, "generator.ts"), fixture);
  assert.equal(cli("inspect", project).mode, "generator");
  const generated = cli("check", project);
  assert.equal(generated.ok, true, JSON.stringify(generated));
  const output = resolve(installed, "ring.json");
  assert.equal(cli("bake", project, "--out", output, "--chord-error-mm", "0.08", "--output", "/ring").ok, true);
  const profile = JSON.parse(readFileSync(output, "utf8"));
  assert.equal(profile.regions.length, 1); assert.ok(profile.regions[0].outer.length >= 16);
  const smoke = resolve(installed, "engine-smoke.mjs");
  writeFileSync(smoke, `import assert from "node:assert/strict";
import { createEngine } from "@geosolve/engine";
import { sketch, mm } from "@geosolve/sketch-code";
const engine = await createEngine();
try {
  const accepted = await engine.evaluate({ definition: ({ count }) => sketch(($) => ({ rings: Array.from({length: count}, (_, index) => $.geometry.centerRadiusCircle("ring" + index, { center: [index * 20, 0], radius: mm(3) })) })), parameters: {count: 2} });
  assert.equal(accepted.status, "accepted");
  assert.equal(accepted.validation.hard_residuals_validated, true);
  const profiles = await engine.exportProfiles(accepted, {chordErrorMm: 0.08});
  assert.equal(profiles.regions.length, 2);
  console.log(JSON.stringify({ok:true, regions:profiles.regions.length}));
} finally {engine.dispose();}
`);
  assert.deepEqual(JSON.parse(run(process.execPath, [smoke], { cwd: installed, env: environment })), { ok: true, regions: 2 });

  // A clean-installed server must use its own frozen assets and native workers.
  const server = spawn(bin, ["serve", project], { cwd: installed, env: environment, stdio: ["ignore", "pipe", "pipe"] });
  t.after(async () => { if (server.exitCode === null) { server.kill("SIGTERM"); await new Promise((done) => server.once("exit", done)); } });
  const session = await new Promise((accept, reject) => {
    let output = "", errors = "";
    const timer = setTimeout(() => reject(Error(`Installed server did not start: ${errors}`)), 30000);
    server.stderr.on("data", (data) => { errors += data; });
    server.stdout.on("data", (data) => {
      output += data;
      try { const parsed = JSON.parse(output); clearTimeout(timer); accept(parsed); } catch { /* JSON may span chunks. */ }
    });
    server.once("error", (error) => { clearTimeout(timer); reject(error); });
    server.once("exit", (code) => { clearTimeout(timer); reject(Error(`Installed server exited ${code}: ${errors}`)); });
  });
  assert.equal(session.ok, true);
  const response = await fetch(session.url);
  assert.equal(response.status, 200);
  const bytes = Buffer.from(await response.arrayBuffer());
  const packagedIndex = manifest.archives.find((archive) => archive.name === "@geosolve/cli").files.find((file) => file.path === "runtime/assets/workbench/index.html");
  assert.equal(digest(bytes), packagedIndex.sha256, "server serves the exact packaged production entry");
  const live = cli("status", project, "--client", "offline-package-smoke");
  assert.equal(live.ok, true); assert.equal(live.state.mode, "generator");
  assert.equal(live.state.inputDefinitions.radius.label, "Radius");
  assert.equal(live.state.inputs.radius, 5);
  assert.equal(live.state.status, "saved");
  assert.equal(live.state.currentHash, live.state.acceptedHash);

  // Build the actual custom website against the clean-installed packages. The
  // repository contributes test/build tools only, never product SDK/engine bytes.
  const website = resolve(installed, "website"); mkdirSync(website);
  const example = resolve(repository, "examples/generator-website");
  cpSync(resolve(example, "src"), resolve(website, "src"), { recursive: true });
  const assets = resolve(website, "dist"); mkdirSync(assets);
  for (const name of ["index.html", "style.css"]) cpSync(resolve(example, name), resolve(assets, name));
  cpSync(resolve(installed, "node_modules/@geosolve/engine/dist/wasm"), resolve(assets, "wasm"), { recursive: true });
  const { build } = await import(resolve(repository, "crates/geosolve-demo-web/frontend/node_modules/esbuild/lib/main.js"));
  await build({ entryPoints: [resolve(website, "src/main.ts"), resolve(website, "src/worker.ts")], outdir: assets,
    bundle: true, format: "esm", target: "es2022", platform: "browser", logLevel: "silent",
    alias: { "@geosolve/sketch-code": resolve(installed, "node_modules/@geosolve/sketch-code/dist/src/index.js"), "@geosolve/engine": resolve(installed, "node_modules/@geosolve/engine/dist/index.js") } });
  const staticServer = createServer((request, response) => {
    try {
      const pathname = new URL(request.url, "http://localhost").pathname;
      const path = resolve(assets, `.${pathname === "/" ? "/index.html" : pathname}`);
      if (!path.startsWith(assets + sep)) throw Error("Unknown asset");
      response.setHeader("Content-Type", { ".html": "text/html", ".js": "text/javascript", ".css": "text/css", ".wasm": "application/wasm" }[extname(path)] ?? "application/octet-stream");
      response.end(readFileSync(path));
    } catch { response.writeHead(404); response.end(); }
  });
  await new Promise((done) => staticServer.listen(0, "127.0.0.1", done));
  const { chromium } = await import(resolve(repository, "crates/geosolve-demo-web/frontend/node_modules/playwright/index.mjs"));
  const browser = await chromium.launch({ executablePath: process.env.GEOSOLVE_CHROMIUM_PATH ?? "/home/arduano/.nix-profile/bin/google-chrome", args: ["--disable-dev-shm-usage"] });
  try {
    const page = await browser.newPage();
    const errors = []; page.on("pageerror", (error) => errors.push(error.message));
    await page.goto(`http://127.0.0.1:${staticServer.address().port}`);
    await page.locator('#status[data-state="ready"]').waitFor({ timeout: 30000 });
    assert.equal(await page.locator("#width").textContent(), "125.5 mm");
    assert.equal(await page.locator("#bores").textContent(), "24");
    await page.getByLabel("Columns", { exact: true }).fill("1");
    await page.locator('#status[data-state="ready"]').waitFor({ timeout: 30000 });
    assert.equal(await page.locator("#width").textContent(), "41.5 mm");
    assert.equal(await page.locator("#bores").textContent(), "8");
    assert.deepEqual(errors, []);
  } finally {
    await browser.close();
    await new Promise((done) => staticServer.close(done));
  }
});

test("packaging refuses an existing destination without replacing bytes", () => {
  const temporary = mkdtempSync(resolve(tmpdir(), "geosolve-package-existing-"));
  try {
    writeFileSync(resolve(temporary, "keep.txt"), "retained");
    assert.throws(() => packageM98({ out: temporary, dist: resolve(repository, "crates/geosolve-demo-web/dist") }), /Refusing to overwrite/);
    assert.equal(readFileSync(resolve(temporary, "keep.txt"), "utf8"), "retained");
  } finally { rmSync(temporary, { recursive: true }); }
});
