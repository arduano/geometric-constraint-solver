// SPDX-License-Identifier: GPL-3.0-or-later
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { spawn, spawnSync } from "node:child_process";
import { cpSync, mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { createServer } from "node:http";
import { tmpdir } from "node:os";
import { extname, resolve, sep } from "node:path";
import test from "node:test";
import { packageArchives, repository } from "../packages/geosolve-cli/scripts/package.mjs";

const digest = (bytes) => createHash("sha256").update(bytes).digest("hex");
const fixture = `import { defineGenerator, sketch, mm } from "@geosolve/sketch-code";
export default defineGenerator({ radius: { type: "number", default: 5, min: 1, max: 10, unit: "mm", label: "Radius" } }, ({radius}) => sketch(($) => ({ ring: $.geometry.centerRadiusCircle("ring", { center: [0, 0], radius: mm(radius) }) })));
`;

function run(command, args, options) {
  const result = spawnSync(command, args, { encoding: "utf8", timeout: 90000, maxBuffer: 8 * 1024 * 1024, ...options });
  assert.equal(result.status, 0, `${command} ${args.join(" ")} failed: ${result.error ?? ""}\n${result.stderr ?? ""}\n${result.stdout ?? ""}`);
  return result.stdout;
}

async function installedServer(t, bin, args, installed, environment) {
  const server = spawn(bin, args, { cwd: installed, env: environment, stdio: ["ignore", "pipe", "pipe"] });
  t.after(async () => { if (server.exitCode === null) { server.kill("SIGTERM"); await new Promise(done => server.once("exit", done)); } });
  return new Promise((accept, reject) => {
    let output = "", errors = "";
    const timer = setTimeout(() => reject(Error(`Installed collaborative server did not start: ${errors}`)), 30000);
    server.stderr.on("data", data => { errors += data; });
    server.stdout.on("data", data => {
      output += data;
      try { const parsed = JSON.parse(output); clearTimeout(timer); accept(parsed); } catch { /* Wait for complete JSON. */ }
    });
    server.once("error", error => { clearTimeout(timer); reject(error); });
    server.once("exit", code => { clearTimeout(timer); reject(Error(`Installed collaborative server exited ${code}: ${errors}`)); });
  });
}

test("four offline archives install into an empty cache and run the actual SDK, engine and folder CLI", { timeout: 180000 }, async (t) => {
  const temporary = mkdtempSync(resolve(tmpdir(), "geosolve-package-"));
  t.after(() => rmSync(temporary, { recursive: true, force: true }));
  const archiveDirectory = process.env.GEOSOLVE_M98_PACKAGES
    ? resolve(process.env.GEOSOLVE_M98_PACKAGES)
    : packageArchives({ out: process.env.GEOSOLVE_M98_PACKAGE_OUT ?? resolve(temporary, "archives"), ...(process.env.GEOSOLVE_M98_DIST ? { dist: process.env.GEOSOLVE_M98_DIST } : {}) }).output;
  const manifest = JSON.parse(readFileSync(resolve(archiveDirectory, "packages.json"), "utf8"));
  assert.equal(manifest.format, "geosolve-offline-packages-v1");
  assert.equal(manifest.archives.length, 4);
  assert.deepEqual(manifest.archives.map(archive => archive.name).sort(), ["@geosolve/cli", "@geosolve/collaboration", "@geosolve/engine", "@geosolve/sketch-code"]);
  const cliFiles = manifest.archives.find(archive => archive.name === "@geosolve/cli").files;
  assert.ok(cliFiles.some(file => file.path === "runtime/geosolve-cli.mjs"));
  assert.ok(cliFiles.some(file => file.path === "runtime/release-artifact.mjs"));
  assert.ok(cliFiles.some(file => file.path === "dist/workspace-runtime.mjs"));
  assert.ok(cliFiles.every(file => !file.path.startsWith("runtime/assets/demo-wasm/")), "Node hosts ship no separate demo execution package");
  assert.ok(cliFiles.every(file => !file.path.startsWith("runtime/scripts/") && !file.path.startsWith("target/")), "packaging owns ordinary runtime/build resources");
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
  for (const packageName of ["sketch-code", "engine", "collaboration", "cli"]) {
    const metadata = JSON.parse(readFileSync(resolve(installed, `node_modules/@geosolve/${packageName}/package.json`), "utf8"));
    assert.ok(Object.values(metadata.dependencies ?? {}).every((version) => !version.startsWith("file:")), "archive dependencies never reference sibling checkout paths");
  }
  for (const packageName of ["collaboration", "cli"]) {
    const notices = readFileSync(resolve(installed, `node_modules/@geosolve/${packageName}/THIRD_PARTY_LICENSES.md`), "utf8");
    assert.match(notices, /automerge 0\.11\.0/);
    assert.match(notices, /Copyright \(c\) 2019-2021 the Automerge contributors/);
    assert.match(notices, /\(C\) 2024 Trifecta Tech Foundation/);
    assert.match(notices, /Permission is hereby granted, free of charge/);
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
  const collaborationSmoke = resolve(installed, "collaboration-smoke.mjs");
  writeFileSync(collaborationSmoke, `import assert from "node:assert/strict";
import {createSharedText} from "@geosolve/collaboration";
import {createTrustedSourceHost} from "@geosolve/collaboration/host";
import {CollaborationClient} from "@geosolve/collaboration/client";
import {createMirrorWorker} from "./node_modules/@geosolve/cli/runtime/collaboration-mirror-worker-bridge.mjs";
import {mkdir,writeFile} from "node:fs/promises";
import {resolve} from "node:path";
const actor=new TextEncoder().encode("installed-server");
const host=await createTrustedSourceHost({configuration:{documentEpoch:"installed",serverEpoch:"run",initialInput:"initial",files:{"main.ts":"a0"}},actor});
const replica=await createSharedText({actor:new TextEncoder().encode("installed-client"),checkpoint:host.textCheckpoint()});
try{
  assert.equal(typeof CollaborationClient,"function");assert.deepEqual(replica.resolveRange(replica.anchorRange("main.ts",1,2)),{path:"main.ts",start_utf16:1,end_utf16:2});
  const stage=host.stageUserWorkingEdits([{kind:"splice",path:"main.ts",start_utf16:1,delete_utf16:1,insert:"😀("},{kind:"create_file",path:"invalid.ts",text:"const = ("}],{userId:"alice",clientId:"installed",requestId:"mixed"});
  assert.equal(host.snapshot().working.files["main.ts"],"a0");host.commitStage(stage);assert.equal(host.snapshot().working.files["main.ts"],"a😀(");
  const folder=resolve("installed-mirror");await mkdir(folder);
  for(const [path,text] of Object.entries(host.snapshot().working.files))await writeFile(resolve(folder,path),text);
  const mirror=await createMirrorWorker({folder,documentId:"installed",documentEpoch:"installed",userId:"external",clientId:"mirror",
    readCommitted:async()=>({checkpoint:host.textCheckpoint(),snapshot:host.snapshot()}),
    admitWorkingEdits:async({edits,operation,expectedRevision})=>{host.commitStage(host.stageUserWorkingEdits(edits,operation,expectedRevision));return{status:"committed"};}});
  try{
    assert.equal((await mirror.reconcile()).status,"synchronized");await writeFile(resolve(folder,"main.ts"),"a2");
    assert.equal((await mirror.reconcile()).status,"synchronized");assert.equal(host.snapshot().working.files["main.ts"],"a2");
  }finally{await mirror.close();}
  console.log(JSON.stringify({ok:true,contributions:host.userHistory("alice").undoCount,mirror:true}));
}finally{replica.dispose();host.dispose();}
`);
  assert.deepEqual(JSON.parse(run(process.execPath, [collaborationSmoke], { cwd: installed, env: environment })), { ok: true, contributions: 1, mirror: true });

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

  // Installed shared authority must resolve all native packages and scripts from
  // archives, serve the exact bundled UI, and use the durable CLI text gateway.
  const invitations = resolve(installed, "invitations.json");
  writeFileSync(invitations, JSON.stringify([{ token: "installed-collaboration-invitation-token", userId: "alice", role: "editor" }]));
  const sharedSession = await installedServer(t, bin, ["serve", starter, "--collaboration", "true", "--initialize", "true", "--invitations", invitations], installed, environment);
  assert.equal(sharedSession.ok, true); assert.equal(sharedSession.collaboration, true);
  const sharedResponse = await fetch(sharedSession.urls[0].url);
  assert.equal(sharedResponse.status, 200); assert.equal(digest(Buffer.from(await sharedResponse.arrayBuffer())), packagedIndex.sha256);
  const flags = ["--collaboration", "true", "--user", "alice", "--client", "installed-shared-cli"], expected = resolve(installed, "shared-expected.json");
  const sharedBefore = cli("status", starter, ...flags, "--out", expected);
  const acceptedInput = sharedBefore.state.authority.acceptedInput;
  const edits = resolve(installed, "shared-edits.json");
  writeFileSync(edits, JSON.stringify([{ kind: "splice", path: "sketch.ts", start_utf16: 0, delete_utf16: 0, insert: "// installed shared edit 😀\n" }, { kind: "create_file", path: "notes.ts", text: "// shared file\n" }]));
  const draftArguments = ["draft", starter, ...flags, "--expected", expected, "--operation", "installed-draft", "--edits", edits];
  const draft = cli(...draftArguments); assert.equal(draft.applied, false);
  assert.deepEqual(cli(...draftArguments).ack, draft.ack, "installed gateway deduplicates exact retries");
  const sharedAfter = cli("status", starter, ...flags, "--out", expected);
  assert.equal(sharedAfter.state.authority.acceptedInput, acceptedInput);
  assert.match(sharedAfter.state.document.working.files["sketch.ts"], /installed shared edit 😀/u);
  assert.equal(cli("apply", starter, ...flags, "--expected", expected, "--operation", "installed-apply").ok, true);
  const editableFolder = resolve(installed, "folder-ui");
  assert.equal(cli("init", editableFolder).ok, true);
  const editableSession = await installedServer(t, bin, ["serve", editableFolder], installed, environment);

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
  const { chromium, expect } = await import(resolve(repository, "crates/geosolve-demo-web/frontend/node_modules/@playwright/test/index.mjs"));
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
    // Exercise the shipped workbench and native browsing/authoring workers from
    // installed URLs too; matching index bytes alone cannot witness worker loads.
    for (const [url, sourceFolder, editable] of [[session.url, project, false], [editableSession.url, editableFolder, true], [sharedSession.urls[0].url, starter, true]]) {
      const sourcePath = resolve(sourceFolder, editable ? "sketch.ts" : "generator.ts");
      const before = readFileSync(sourcePath, "utf8");
      const workbench = await browser.newPage({ viewport: { width: 1400, height: 900 } });
      const failures = []; workbench.on("pageerror", error => failures.push(error.message));
      try {
        await workbench.goto(url);
        const canvas = workbench.locator('canvas[data-renderer="webgl2"]');
        await expect(canvas).toHaveAttribute("data-render-state", "ready", { timeout: 30000 });
        await expect.poll(() => canvas.evaluate(element => Reflect.get(element, "__geosolvePresentedFrame")?.items.filter(item => item.layer === "geometry").length ?? 0)).toBeGreaterThan(0);
        if (editable) {
          await workbench.getByRole("navigation", { name: "Primary tools" }).getByRole("button", { name: "Sketch", exact: true }).click();
          await workbench.getByRole("menuitem", { name: "Polyline", exact: true }).click();
          const box = await workbench.getByRole("application").boundingBox(); assert.ok(box);
          await workbench.keyboard.down("Alt");
          await workbench.mouse.click(box.x + box.width * 0.7, box.y + box.height * 0.7);
          await workbench.mouse.click(box.x + box.width * 0.8, box.y + box.height * 0.8);
          await workbench.keyboard.up("Alt");
          await expect(workbench.getByRole("button", { name: "Finish", exact: true })).toBeEnabled({ timeout: 15000 });
          await workbench.keyboard.press("Escape");
          assert.equal(readFileSync(sourcePath, "utf8"), before, "native local drafting cannot publish source");
        }
        assert.deepEqual(failures, []);
      } finally { await workbench.close(); }
    }
  } finally {
    await browser.close();
    await new Promise((done) => staticServer.close(done));
  }
});

test("packaging refuses an existing destination without replacing bytes", () => {
  const temporary = mkdtempSync(resolve(tmpdir(), "geosolve-package-existing-"));
  try {
    writeFileSync(resolve(temporary, "keep.txt"), "retained");
    assert.throws(() => packageArchives({ out: temporary, dist: resolve(repository, "crates/geosolve-demo-web/dist") }), /Refusing to overwrite/);
    assert.equal(readFileSync(resolve(temporary, "keep.txt"), "utf8"), "retained");
  } finally { rmSync(temporary, { recursive: true }); }
});
