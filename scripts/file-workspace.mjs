#!/usr/bin/env node
// SPDX-License-Identifier: GPL-3.0-or-later
// M98 single-folder prototype. All geometry/compiler authority is existing Rust/WASM.
import { createHash, randomBytes } from "node:crypto";
import { createServer } from "node:http";
import { existsSync, readFileSync, writeFileSync, mkdirSync, lstatSync, realpathSync,
  renameSync, linkSync, unlinkSync, openSync, fsyncSync, closeSync } from "node:fs";
import { resolve, dirname, basename, extname, sep } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const root = fileURLToPath(new URL("../", import.meta.url));
const frontend = resolve(root, "crates/geosolve-demo-web/frontend");
const sourceLimit = 4 * 1024 * 1024;
export const hash = (text) => createHash("sha256").update(text).digest("hex");

function regular(path) {
  const stat = lstatSync(path);
  if (!stat.isFile() || stat.isSymbolicLink() || stat.size > sourceLimit) {
    throw Error(`Expected a regular file of at most 4 MiB: ${path}`);
  }
}
function readSource(path) {
  regular(path);
  return new TextDecoder("utf-8", { fatal: true }).decode(readFileSync(path));
}
function atomicWrite(path, text, expectedHash, recoveryDirectory) {
  const temporary = `${path}.m98-${randomBytes(8).toString("hex")}.tmp`;
  try {
    const descriptor = openSync(temporary, "wx", 0o600);
    try { writeFileSync(descriptor, text); fsyncSync(descriptor); }
    finally { closeSync(descriptor); }
    // Recheck after staging, immediately before publication; watch timing is irrelevant.
    if (expectedHash !== undefined && hash(readSource(path)) !== expectedHash) {
      throw Error("Conflict: disk changed before writeback. Disk and pending intent were retained; refresh and retry.");
    }
    if (expectedHash === undefined) renameSync(temporary, path);
    else {
      // A check followed by overwrite-rename has a race against an independent editor.
      // Claim and retain the displaced inode, then publish with an exclusive hard link.
      // Even a rename in that tiny interval can never be overwritten. Recovery files
      // also retain a writer which still holds an open descriptor to the displaced inode.
      const recovery = resolve(recoveryDirectory, `before-${Date.now()}-${randomBytes(6).toString("hex")}.ts`);
      renameSync(path, recovery);
      try {
        if (hash(readSource(recovery)) !== expectedHash) throw Error("Conflict: external text arrived during writeback.");
        linkSync(temporary, path); // Fails with EEXIST if another writer published.
      } catch (error) {
        try { linkSync(recovery, path); } catch { /* Newer disk text wins; displaced text stays in recovery. */ }
        throw Error(`Conflict or failed write: disk was preserved; refresh/retry. Recovery: ${recovery}. ${error}`);
      }
    }
  } finally { if (existsSync(temporary)) unlinkSync(temporary); }
}

export function initProject(folder) {
  // An existing empty directory is allowed; any existing project file is refused.
  mkdirSync(folder, { recursive: true });
  const files = ["geosolve.json", "sketch.ts"];
  for (const name of files) if (existsSync(resolve(folder, name))) throw Error(`Refusing to overwrite ${resolve(folder, name)}`);
  for (const name of files) writeFileSync(resolve(folder, name), readFileSync(resolve(root, "examples/file-workspace", name)), { flag: "wx" });
  return { ok: true, folder: resolve(folder), source: resolve(folder, "sketch.ts") };
}

export async function openProject(folder, { cache = true } = {}) {
  folder = realpathSync(folder);
  const manifestPath = resolve(folder, "geosolve.json");
  const manifest = JSON.parse(readSource(manifestPath));
  if (manifest.format !== "geosolve-folder-v1" || manifest.entry !== "sketch.ts" || Object.keys(manifest).some((key) => !["format", "entry"].includes(key))) {
    throw Error("M98 supports geosolve-folder-v1 with the single fixed sketch.ts entry only.");
  }
  const sourcePath = resolve(folder, "sketch.ts");
  const cachePath = resolve(folder, ".geosolve");
  if (cache) {
    mkdirSync(cachePath, { recursive: true });
    if (lstatSync(cachePath).isSymbolicLink() || realpathSync(cachePath) !== cachePath) throw Error("Project cache must be a local directory, not a symlink.");
  }
  const wasm = await import(pathToFileURL(resolve(frontend, "src/generated/geosolve_demo_web.js")));
  await wasm.default({ module_or_path: readFileSync(resolve(frontend, "src/generated/geosolve_demo_web_bg.wasm")) });
  const { WasmWorkbenchAdapter, resolvePendingManagedMutationSnapshot: settle } = await import(pathToFileURL(resolve(root, "target/m98/workspace-runtime.mjs")));
  const adapter = new WasmWorkbenchAdapter(wasm.WorkbenchHandle);
  await adapter.construct({ version: 2 });
  let snapshot = await adapter.dispatch({ version: 2, command: "project.new-code" });
  const apply = async (contents) => {
    const next = await adapter.dispatch({ version: 2, command: "source.prepare", payload: { path: "sketch.ts", contents } });
    snapshot = await settle(adapter, next);
    return snapshot.project.status === "accepted" && !snapshot.source.dirty && !snapshot.pendingManagedMutation;
  };
  let acceptedHash = null;
  let currentHash = null;
  let acceptedRevision = null;
  let sourceRevision = 0;
  let sequence = 0;
  let externalApplies = 0;
  let writes = 0;
  let ioError = null;
  let candidate = null;
  let candidateSince = 0;
  let notify = () => {};
  const lastGoodPath = resolve(cachePath, "last-good.ts");
  const warnings = [];
  const sourceOf = (value) => value.source.files.find((file) => file.path === "sketch.ts")?.contents;
  const state = () => ({
    format: "geosolve-folder-status-v1", ok: !ioError && currentHash === acceptedHash && !snapshot.source.dirty && snapshot.project.status === "accepted",
    warnings, sequence, revision: sourceRevision, acceptedRevision, workbenchRevision: snapshot.revision, currentHash, acceptedHash,
    status: ioError ? "error" : currentHash !== acceptedHash ? "stale" : snapshot.source.dirty ? "unsaved" : "saved",
    diagnostics: ioError ? [{ path: "sketch.ts", detail: ioError }] : snapshot.problems,
    paths: { folder, source: sourcePath, manifest: manifestPath }, externalApplies, writes,
  });
  const publish = () => { sequence++; notify(sequence); };
  async function scan(force = false) {
    try {
      const text = readSource(sourcePath);
      const digest = hash(text);
      if (digest === currentHash) { if (ioError) { ioError = null; publish(); } return; }
      if (!force) {
        if (candidate !== digest) { candidate = digest; candidateSince = Date.now(); return; }
        if (Date.now() - candidateSince < 120) return;
      }
      ioError = null;
      currentHash = digest;
      sourceRevision++;
      candidate = null;
      externalApplies++;
      await adapter.cancel({ version: 2, reason: "blur" });
      // The existing prepare API deliberately rejects an unchanged accepted source.
      // Restoring exactly those bytes clears its retained draft through Revert.
      const accepted = digest === acceptedHash
        ? ((snapshot = await adapter.dispatch({ version: 2, command: "source.revert" })), true)
        : await apply(text);
      if (accepted) {
        acceptedHash = digest;
        acceptedRevision = sourceRevision;
        if (cache) {
          try { atomicWrite(lastGoodPath, text); }
          catch (error) { warnings.push(`Derived cache update failed: ${error}`); }
        }
      }
      publish();
    } catch (error) {
      const detail = String(error);
      if (ioError !== detail) { ioError = detail; publish(); }
    }
  }
  // Current authored files are primary. Read optional cache only after current
  // reconstruction fails, and never let malformed derived bytes block opening.
  await scan(true);
  if (acceptedHash === null && cache && existsSync(lastGoodPath)) {
    const rejected = (await adapter.persistProject()).contents;
    try {
      const previous = readSource(lastGoodPath);
      if (await apply(previous)) {
        acceptedHash = hash(previous);
        acceptedRevision = 0;
        if (currentHash !== null) await apply(readSource(sourcePath));
      } else {
        snapshot = await adapter.construct({ version: 2, persistedProject: rejected });
        warnings.push("Derived last-good cache is invalid; current source diagnostics retained.");
      }
    } catch (error) {
      snapshot = await adapter.construct({ version: 2, persistedProject: rejected });
      warnings.push(`Derived last-good cache ignored: ${error}`);
    }
  }
  await adapter.dispatch({ version: 2, command: "view.fit" });
  snapshot = await adapter.snapshot();

  const allowed = new Set(["snapshot", "toolCatalog", "dispatch", "pointer", "wheel", "wheelBatch", "resize", "cancel", "exportProject", "exportReproduction", "exportInteractionTrace"]);
  async function request(method, input, baseHash) {
    if (!allowed.has(method)) throw Error("Unsupported workspace method");
    if (method === "toolCatalog" || method.startsWith("export")) return adapter[method]();
    if (method === "snapshot") return adapter.snapshot();
    if (method === "dispatch" && ["project.new", "project.new-code", "project.import", "sample.open"].includes(input?.command)) {
      throw Error("Folder mode keeps this project's sketch.ts open. Use the ordinary demo URL for New, samples or project import.");
    }
    const navigation = ["resize", "wheel", "wheelBatch", "cancel"].includes(method);
    if (!navigation && (baseHash !== currentHash || hash(readSource(sourcePath)) !== baseHash)) {
      await scan(true);
      const error = Error("Conflict: disk changed since this edit began. Disk and pending intent were retained. Refresh from disk, then retry explicitly.");
      error.conflict = true;
      throw error;
    }
    if (method === "pointer" && snapshot.presentation.activeTool === "select" && input.phase === "move" && (input.buttons & 1)) {
      snapshot = await adapter.cancel({ version: 2, reason: "escape" });
      throw Error("Point/grip dragging is deferred in folder mode because it stores native instance overlays. Edit a source value or an Inspector dimension instead.");
    }
    const before = sourceOf(snapshot);
    const rollback = !navigation ? (await adapter.persistProject()).contents : null;
    const next = await adapter[method](input);
    if (!next) return null;
    snapshot = await settle(adapter, next);
    const after = sourceOf(snapshot);
    if (!navigation && snapshot.project.status === "accepted" && !snapshot.source.dirty && after !== before) {
      try {
        if (typeof after !== "string") throw Error("This edit has no supported plaintext source.");
        atomicWrite(sourcePath, after, baseHash, cachePath);
      } catch (error) {
        snapshot = await adapter.construct({ version: 2, persistedProject: rollback });
        // Preserve the compiled candidate as well as the original command for manual recovery.
        error.pendingSource = after;
        error.conflict = String(error).includes("Conflict:");
        throw error;
      }
      writes++;
      sourceRevision++;
      currentHash = acceptedHash = hash(after);
      acceptedRevision = sourceRevision;
      ioError = null;
      candidate = null;
      if (cache) {
        try { atomicWrite(lastGoodPath, after); }
        catch (error) { warnings.push(`Source saved, but derived last-good cache failed: ${error}`); }
      }
      publish();
    }
    return snapshot;
  }
  return { adapter, scan, request, state, cachePath, setNotify: (callback) => { notify = callback; } };
}

export async function serveProject(folder, { port = 0 } = {}) {
  const project = await openProject(folder);
  const token = randomBytes(24).toString("hex");
  const clients = new Set();
  let tail = Promise.resolve();
  const serial = (action) => { const next = tail.then(action); tail = next.catch(() => {}); return next; };
  project.setNotify((sequence) => { for (const client of clients) client.write(`data: ${sequence}\n\n`); });
  let origin;
  const send = (response, status, value) => { response.writeHead(status, { "Content-Type": "application/json", "Cache-Control": "no-store" }); response.end(JSON.stringify(value)); };
  const server = createServer(async (request, response) => {
    try {
      if (request.headers.host !== new URL(origin).host || (request.headers.origin && request.headers.origin !== origin)) {
        send(response, 403, { error: "Loopback origin required" }); return;
      }
      const url = new URL(request.url, origin);
      if (url.pathname.startsWith("/api/")) {
        if ((request.headers.authorization ?? `Bearer ${url.searchParams.get("token")}`) !== `Bearer ${token}`) {
          send(response, 403, { error: "Session token required" }); return;
        }
        if (url.pathname === "/api/events" && request.method === "GET") {
          response.writeHead(200, { "Content-Type": "text/event-stream", "Cache-Control": "no-store", Connection: "keep-alive" });
          clients.add(response); response.write(`data: ${project.state().sequence}\n\n`);
          request.on("close", () => clients.delete(response)); return;
        }
        if (url.pathname === "/api/status" && request.method === "GET") {
          send(response, 200, await serial(async () => { await project.scan(true); return project.state(); })); return;
        }
        if (url.pathname !== "/api/rpc" || request.method !== "POST" || request.headers["content-type"] !== "application/json") {
          send(response, 400, { error: "Expected JSON workspace RPC" }); return;
        }
        let body = "";
        for await (const chunk of request) { body += chunk; if (Buffer.byteLength(body) > sourceLimit + 65536) throw Error("Request too large"); }
        const rpc = JSON.parse(body);
        await serial(async () => {
          try { const result = await project.request(rpc.method, rpc.input, rpc.baseHash); send(response, 200, { result, state: project.state() }); }
          catch (error) { send(response, error.conflict ? 409 : 400, { error: String(error), pendingSource: error.pendingSource, state: project.state() }); }
        }); return;
      }
      if (request.method !== "GET") { send(response, 405, { error: "GET required" }); return; }
      const dist = resolve(root, process.env.GEOSOLVE_DIST ?? "crates/geosolve-demo-web/dist");
      const path = resolve(dist, `.${decodeURIComponent(url.pathname === "/" ? "/index.html" : url.pathname)}`);
      if (!path.startsWith(dist + sep) || !realpathSync(path).startsWith(dist + sep)) throw Error("Unknown asset");
      const mime = { ".html": "text/html", ".js": "text/javascript", ".css": "text/css", ".wasm": "application/wasm", ".json": "application/json" }[extname(path)] ?? "text/plain";
      response.writeHead(200, { "Content-Type": mime, "Cache-Control": "no-store", "Referrer-Policy": "no-referrer" }); response.end(readFileSync(path));
    } catch (error) { if (!response.headersSent) send(response, 400, { error: String(error) }); else response.end(); }
  });
  await new Promise((resolveListen, reject) => { server.once("error", reject); server.listen(port, "127.0.0.1", resolveListen); });
  origin = `http://127.0.0.1:${server.address().port}`;
  const url = `${origin}/?folder=1#token=${token}`;
  const session = { url, origin, token, pid: process.pid, ...project.state().paths };
  atomicWrite(resolve(project.cachePath, "session.json"), JSON.stringify(session, null, 2));
  const timer = setInterval(() => { void serial(() => project.scan()); }, 100);
  const heartbeat = setInterval(() => { for (const client of clients) client.write(": connected\n\n"); }, 15000);
  const close = async () => {
    clearInterval(timer); clearInterval(heartbeat);
    for (const client of clients) client.end();
    await new Promise((done) => server.close(done));
  };
  return { ...session, project, close };
}

export async function bakeProject(folder, output, chordErrorMm) {
  if (!Number.isFinite(chordErrorMm) || chordErrorMm <= 0) throw Error("--chord-error-mm must be finite and positive");
  folder = realpathSync(folder);
  // Generated interchange is deliberately outside the source project, never source/cache authority.
  output = resolve(realpathSync(dirname(resolve(output))), basename(resolve(output)));
  if (output === folder || output.startsWith(folder + sep)) throw Error("Bake output must be outside the source project");
  if (existsSync(output) && (!lstatSync(output).isFile() || lstatSync(output).isSymbolicLink())) throw Error("Bake output must be a regular file, not a symlink");
  const sourcePath = resolve(folder, "sketch.ts");
  const manifestPath = resolve(folder, "geosolve.json");
  regular(sourcePath);
  regular(manifestPath);
  const sourceBytes = readFileSync(sourcePath);
  const manifestBytes = readFileSync(manifestPath);
  const sourceText = new TextDecoder("utf-8", { fatal: true }).decode(sourceBytes);
  const sourceHash = hash(sourceBytes);
  const project = await openProject(folder, { cache: false });
  const state = project.state();
  if (!state.ok || state.acceptedHash !== hash(sourceText)) {
    throw Error(`Bake requires current accepted disk source: ${JSON.stringify(state)}`);
  }
  const geometry = await project.adapter.bakeProfile(chordErrorMm);
  geometry.source = { sha256: sourceHash };
  const temporary = `${output}.m98-${randomBytes(8).toString("hex")}.tmp`;
  try {
    const descriptor = openSync(temporary, "wx", 0o600);
    try { writeFileSync(descriptor, JSON.stringify(geometry, null, 2) + "\n"); fsyncSync(descriptor); }
    finally { closeSync(descriptor); }
    regular(sourcePath);
    regular(manifestPath);
    if (hash(readFileSync(sourcePath)) !== sourceHash || !readFileSync(manifestPath).equals(manifestBytes)) {
      throw Error("Bake conflict: disk changed during export; retry with the current source");
    }
    renameSync(temporary, output);
  } finally { if (existsSync(temporary)) unlinkSync(temporary); }
  return { ok: true, output, source: { path: sourcePath, sha256: sourceHash },
    acceptedRevision: state.acceptedRevision, workbenchRevision: state.workbenchRevision,
    regions: geometry.regions.map(({ id, outer, holes }) => ({ id, outerVertices: outer.length, holes: holes.map((loop) => loop.length) })) };
}

async function main() {
  const [command, folder, ...options] = process.argv.slice(2);
  if (!folder || !["init", "serve", "open", "check", "status", "bake"].includes(command)) throw Error("Usage: node scripts/file-workspace.mjs init|serve|open|check|status <folder> [--port N]; bake <folder> --out <file> --chord-error-mm <positive number>");
  if (command === "bake") {
    const flags = new Map();
    for (let i = 0; i < options.length; i += 2) {
      if (!["--out", "--chord-error-mm"].includes(options[i]) || !options[i + 1] || flags.has(options[i])) throw Error("Expected --out <file> --chord-error-mm <positive number>");
      flags.set(options[i], options[i + 1]);
    }
    if (flags.size !== 2) throw Error("Bake requires --out <file> and --chord-error-mm <positive number>");
    console.log(JSON.stringify(await bakeProject(folder, flags.get("--out"), Number(flags.get("--chord-error-mm"))), null, 2));
    return;
  }
  if (command === "init") { console.log(JSON.stringify(initProject(folder), null, 2)); return; }
  if (command === "status") {
    const session = JSON.parse(readSource(resolve(folder, ".geosolve/session.json")));
    const response = await fetch(`${session.origin}/api/status`, { headers: { Authorization: `Bearer ${session.token}` } });
    if (!response.ok) throw Error(`Status failed: ${response.status}`);
    console.log(JSON.stringify(await response.json(), null, 2)); return;
  }
  if (command === "check") {
    const project = await openProject(folder, { cache: false });
    const result = project.state(); console.log(JSON.stringify(result, null, 2)); process.exitCode = result.ok ? 0 : 1; return;
  }
  if (options.length && (options.length !== 2 || options[0] !== "--port" || !/^\d+$/.test(options[1]))) throw Error("Expected --port N (0 chooses a fresh private port)");
  const session = await serveProject(folder, { port: Number(options[1] ?? 0) });
  console.log(JSON.stringify({ status: "PROTOTYPE_READY_FOR_UAT", ...session, project: undefined, close: undefined }, null, 2));
  for (const signal of ["SIGINT", "SIGTERM"]) process.once(signal, () => { void session.close().then(() => process.exit(0)); });
}
if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  main().catch((error) => { console.error(JSON.stringify({ ok: false, error: String(error) })); process.exitCode = 1; });
}
