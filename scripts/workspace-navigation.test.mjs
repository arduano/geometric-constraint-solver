// SPDX-License-Identifier: GPL-3.0-or-later
import assert from "node:assert/strict";
import fs, { cpSync, lstatSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { syncBuiltinESMExports } from "node:module";
import { tmpdir } from "node:os";
import { resolve } from "node:path";
import test from "node:test";
import { Worker } from "node:worker_threads";
import { hash, serveProject } from "./file-workspace.mjs";

const root = new URL("../", import.meta.url);
const samples = [
  { name: "small", source: "examples/file-workspace" },
  { name: "gridfinity", source: "crates/geosolve-sketch-code/assets/bundled-samples/gridfinity-bin-section" },
  { name: "manifold", source: "examples/file-workspace-manifold" },
  { name: "dense", source: "crates/geosolve-sketch-code/assets/bundled-samples/robotic-harness-backplane" },
];

function diskState(folder) {
  const visit = (path = "") => readdirSync(resolve(folder, path), { withFileTypes: true }).sort((a, b) => a.name.localeCompare(b.name)).flatMap((entry) => {
    const relative = path ? `${path}/${entry.name}` : entry.name;
    const absolute = resolve(folder, relative);
    const stat = lstatSync(absolute, { bigint: true });
    assert.ok(!stat.isSymbolicLink());
    const item = { path: relative, inode: String(stat.ino), mode: String(stat.mode), mtime: String(stat.mtimeNs), ctime: String(stat.ctimeNs) };
    if (entry.isDirectory()) return [item, ...visit(relative)];
    assert.ok(entry.isFile());
    return [{ ...item, bytes: Number(stat.size), hash: hash(readFileSync(absolute)) }];
  });
  return visit();
}

async function fixture(t, sample) {
  const folder = mkdtempSync(resolve(tmpdir(), "geosolve-m98-navigation-"));
  cpSync(new URL(sample.source, root), folder, { recursive: true, filter: (path) => !path.endsWith("/.geosolve") });
  if (sample.contents) writeFileSync(resolve(folder, "sketch.ts"), sample.contents);
  writeFileSync(resolve(folder, "geosolve.json"), JSON.stringify({ format: "geosolve-folder-v2", entry: "sketch.ts", mode: sample.mode ?? "editable" }));
  let bridge;
  t.after(async () => { if (bridge) await bridge.close(); rmSync(folder, { recursive: true, force: true }); });
  const start = performance.now();
  bridge = await serveProject(folder);
  const openingMs = performance.now() - start;
  const rpc = async (method, input, state) => {
    const response = await fetch(`${bridge.origin}/api/rpc`, { method: "POST", headers: {
      Authorization: `Bearer ${bridge.token}`, "Content-Type": "application/json",
    }, body: JSON.stringify({ method, input, clientId: "navigation-review", authority: state?.authority, baseHash: state?.currentHash }) });
    return { status: response.status, ...await response.json() };
  };
  const current = await rpc("session.join");
  assert.equal(current.status, 200, current.error);
  assert.equal(current.state.ok, true, JSON.stringify(current.state.diagnostics));
  assert.equal(current.result.project.status, "accepted");
  return { folder, bridge, rpc, current, openingMs };
}

function navigationGuard(t, adapter) {
  const writes = [];
  const workers = [];
  const methods = [];
  const restores = [];
  for (const name of ["writeFileSync", "appendFileSync", "renameSync", "linkSync", "unlinkSync", "mkdirSync", "rmSync", "truncateSync", "chmodSync", "fsyncSync"]) {
    const original = fs[name];
    fs[name] = (...args) => { writes.push({ name, path: String(args[0]) }); throw Error(`Navigation attempted ${name}`); };
    restores.push(() => { fs[name] = original; });
  }
  const originalOpen = fs.openSync;
  fs.openSync = (path, flags, ...args) => {
    if (flags !== "r" && flags !== fs.constants.O_RDONLY) { writes.push({ name: "openSync", path: String(path), flags }); throw Error("Navigation opened a file for writing"); }
    return originalOpen(path, flags, ...args);
  };
  restores.push(() => { fs.openSync = originalOpen; });
  syncBuiltinESMExports();
  const emit = Worker.prototype.emit;
  const emitMock = t.mock.method(Worker.prototype, "emit", function (event, ...args) {
    if (event === "online") workers.push(this.threadId);
    return emit.call(this, event, ...args);
  });
  const postMessage = Worker.prototype.postMessage;
  const postMock = t.mock.method(Worker.prototype, "postMessage", function (message, ...args) {
    methods.push({ method: message.method, command: message.input?.command });
    return postMessage.call(this, message, ...args);
  });
  const persist = adapter.persistProject;
  adapter.persistProject = () => { throw Error("Navigation serialized full rollback/history checkpoint"); };
  return { writes, workers, methods, restore() {
    adapter.persistProject = persist;
    emitMock.mock.restore(); postMock.mock.restore();
    restores.reverse().forEach((restore) => restore());
    syncBuiltinESMExports();
  } };
}

for (const sample of samples) test(`${sample.name} folder navigation preserves every authored/cache file without compilation or history serialization`, { timeout: 180000 }, async (t) => {
  const f = await fixture(t, sample);
  let current = f.current;
  current = await f.rpc("resize", { version: 2, width: 960, height: 640, pixelRatio: 1 }, current.state);
  assert.equal(current.status, 200, current.error);
  current = await f.rpc("dispatch", { version: 2, command: "view.fit" }, current.state);
  assert.equal(current.status, 200, current.error);
  await f.bridge.project.saveDerived();
  const beforeDisk = diskState(f.folder);
  const beforeProject = await f.bridge.project.adapter.exportProject();
  const beforeDesign = await f.bridge.project.adapter.exportWorkspaceDesign();
  const before = current;
  const elapsed = [];
  const guard = navigationGuard(t, f.bridge.project.adapter);
  try {
    for (let i = 0; i < 8; i++) {
      const operations = [
        ["wheelBatch", [{ version: 2, x: 480, y: 320, deltaX: 0, deltaY: i % 2 ? 12 : -12, ctrl: false }]],
        ["pointer", { version: 2, phase: "move", pointerId: 1, x: 160 + i * 20, y: 240, buttons: 0, modifiers: { shift: false, ctrl: false, alt: false, meta: false } }],
      ];
      for (const [method, input] of operations) {
        const start = performance.now();
        const next = await f.rpc(method, input, current.state);
        current = { ...next, result: next.result ?? current.result };
        elapsed.push({ method, ms: performance.now() - start });
        assert.equal(current.status, 200, current.error);
        assert.equal(current.state.ok, true, JSON.stringify(current.state.diagnostics));
        assert.equal(current.result.project.status, "accepted");
      }
    }
    for (const command of ["view.origin", "view.fit", "view.grid.toggle", "view.grid.toggle", "view.fit"]) {
      const start = performance.now();
      current = await f.rpc("dispatch", { version: 2, command }, current.state);
      elapsed.push({ method: command, ms: performance.now() - start });
      assert.equal(current.status, 200, current.error);
    }
    assert.deepEqual(current.result.source, before.result.source);
    for (const key of ["writes", "externalApplies", "currentHash", "acceptedHash", "sourceHash", "revision", "acceptedRevision"]) assert.deepEqual(current.state[key], before.state[key], key);
    assert.deepEqual(await f.bridge.project.adapter.exportProject(), beforeProject);
    assert.deepEqual(await f.bridge.project.adapter.exportWorkspaceDesign(), beforeDesign);
    assert.deepEqual(diskState(f.folder), beforeDisk);
    assert.deepEqual(guard.writes, [], "navigation attempted no filesystem writes, including temporary/cache files");
    assert.deepEqual(guard.workers, [], "navigation started no compilation or replacement workers");
    assert.ok(guard.methods.every(({ method, command }) => ["wheelBatch", "pointer", "dispatch", "exportProject", "exportWorkspaceDesign"].includes(method) && (!command || command.startsWith("view."))), JSON.stringify(guard.methods));
  } finally { guard.restore(); }
  const durations = elapsed.map((row) => row.ms).sort((a, b) => a - b);
  const evidence = { sample: sample.name, openingMs: f.openingMs, sceneItems: current.result.frame.scene.items.length,
    sourceFiles: before.result.source.files.length, diskFiles: beforeDisk.filter((file) => file.hash).length,
    requests: elapsed.length, medianMs: durations[Math.floor(durations.length / 2)], p95Ms: durations[Math.ceil(durations.length * .95) - 1], maxMs: durations.at(-1),
    writes: guard.writes, compilationWorkers: guard.workers, elapsed };
  t.diagnostic(JSON.stringify(evidence));
  mkdirSync(new URL("target/m98/navigation", root), { recursive: true });
  writeFileSync(new URL(`target/m98/navigation/${sample.name}.json`, root), JSON.stringify(evidence, null, 2) + "\n");
});

test("generator Center on origin remains available as read-only navigation", { timeout: 15000 }, async (t) => {
  const f = await fixture(t, { ...samples[0], mode: "generator", contents:
    'import {sketch,mm} from "@geosolve/sketch-code"; export default () => sketch(($) => ({ring: $.geometry.centerRadiusCircle("ring", {center:[10,20],radius:mm(10)})}));\n' });
  const before = f.current;
  const beforeDisk = diskState(f.folder);
  const guard = navigationGuard(t, f.bridge.project.adapter);
  try {
    const current = await f.rpc("dispatch", { version: 2, command: "view.origin" }, before.state);
    assert.equal(current.status, 200, current.error);
    assert.equal(current.state.ok, true);
    assert.equal(current.result.project.status, "accepted");
    for (const key of ["writes", "externalApplies", "currentHash", "acceptedHash", "sourceFiles", "inputs"]) assert.deepEqual(current.state[key], before.state[key], key);
    assert.deepEqual(diskState(f.folder), beforeDisk);
    assert.deepEqual(guard.writes, []);
    assert.deepEqual(guard.workers, []);
  } finally { guard.restore(); }
});

test("middle-button pan terminals preserve authority without serializing authored history", { timeout: 15000 }, async (t) => {
  const f = await fixture(t, samples[0]);
  let current = f.current;
  const beforeProject = await f.bridge.project.adapter.exportProject();
  const beforeDesign = await f.bridge.project.adapter.exportWorkspaceDesign();
  await f.bridge.project.saveDerived();
  const beforeDisk = diskState(f.folder);
  const guard = navigationGuard(t, f.bridge.project.adapter);
  try {
    for (const [phase, x, y, buttons] of [["down", 480, 320, 4], ["move", 520, 340, 4], ["up", 520, 340, 0]]) {
      const next = await f.rpc("pointer", { version: 2, phase, pointerId: 17, x, y, buttons,
        modifiers: { alt: false, ctrl: false, meta: false, shift: false } }, current.state);
      assert.equal(next.status, 200, `${phase}: ${next.error}`);
      current = { ...next, result: next.result ?? current.result };
    }
    assert.equal(current.state.ok, true);
    assert.deepEqual(await f.bridge.project.adapter.exportProject(), beforeProject);
    assert.deepEqual(await f.bridge.project.adapter.exportWorkspaceDesign(), beforeDesign);
    for (const key of ["writes", "externalApplies", "currentHash", "acceptedHash", "revision", "acceptedRevision"]) assert.deepEqual(current.state[key], f.current.state[key], key);
    assert.deepEqual(diskState(f.folder), beforeDisk);
    assert.deepEqual(guard.writes, []);
    assert.deepEqual(guard.workers, []);
  } finally { guard.restore(); }
});

test("Show in canvas selects source-owned geometry without serializing authored history", { timeout: 15000 }, async (t) => {
  const f = await fixture(t, samples[0]);
  const source = f.current.result.source.files.find((file) => file.path === "sketch.ts").contents;
  const offset = source.indexOf('$.geometry.centerRadiusCircle(');
  assert.ok(offset >= 0);
  const from = Buffer.byteLength(source.slice(0, offset));
  const beforeProject = await f.bridge.project.adapter.exportProject();
  const beforeDesign = await f.bridge.project.adapter.exportWorkspaceDesign();
  const beforeDisk = diskState(f.folder);
  const guard = navigationGuard(t, f.bridge.project.adapter);
  try {
    const current = await f.rpc("dispatch", { version: 2, command: "navigation.source.select", payload: {
      authority: f.current.result.navigation.authority, path: "sketch.ts", from, to: from + 10,
    } }, f.current.state);
    assert.equal(current.status, 200, current.error);
    assert.equal(current.state.ok, true);
    assert.ok(current.result.navigation.itemCount > 0);
    assert.ok(current.result.navigation.rows.some((row) => row.id === "managed:ring" && row.state === "selected"));
    assert.notEqual(current.result.navigation.selectionKey, f.current.result.navigation.selectionKey);
    assert.deepEqual(await f.bridge.project.adapter.exportProject(), beforeProject);
    assert.deepEqual(await f.bridge.project.adapter.exportWorkspaceDesign(), beforeDesign);
    assert.deepEqual(diskState(f.folder), beforeDisk);
    assert.deepEqual(guard.writes, []);
    assert.deepEqual(guard.workers, []);
  } finally { guard.restore(); }
});

test("HTTP admission is bounded and repeated watcher ticks coalesce while the host is busy", { timeout: 30000 }, async (t) => {
  const intervals = new Map();
  const interval = globalThis.setInterval;
  // Drive the actual registered watcher callback deterministically, while real
  // HTTP, worker execution, request timers and the bridge queue remain live.
  t.mock.method(globalThis, "setInterval", (callback, ms, ...args) => {
    intervals.set(ms, () => callback(...args));
    return interval(callback, 2147483647, ...args);
  });
  const f = await fixture(t, samples[0]);
  await f.bridge.project.saveDerived();
  const beforeDisk = diskState(f.folder);
  const watcher = intervals.get(100);
  assert.equal(typeof watcher, "function");
  const entered = Promise.withResolvers();
  const release = Promise.withResolvers();
  const scan = f.bridge.project.scan;
  let scans = 0;
  f.bridge.project.scan = async (...args) => {
    scans++;
    if (scans === 1) { entered.resolve(); await release.promise; }
    return scan(...args);
  };
  const guard = navigationGuard(t, f.bridge.project.adapter);
  let responses;
  try {
    watcher();
    await entered.promise;
    for (let i = 0; i < 1000; i++) watcher();
    assert.equal(scans, 1);
    const rejected = Promise.withResolvers();
    let refusals = 0;
    const requests = Array.from({ length: 140 }, () => f.rpc("snapshot").then((result) => {
      if (result.status !== 200 && ++refusals === 13) rejected.resolve();
      return result;
    }));
    const timeout = setTimeout(() => rejected.reject(Error("Queue overflow did not respond while the host was busy")), 10000);
    try { await rejected.promise; } finally { clearTimeout(timeout); }
    assert.equal(scans, 1, "queued watcher work is coalesced independently of request saturation");
    // The native actor and unauthenticated HTTP refusal both remain responsive
    // while a host operation is deliberately pending.
    assert.equal((await f.bridge.project.adapter.snapshot()).project.status, "accepted");
    const denied = await fetch(`${f.bridge.origin}/api/status`);
    assert.equal(denied.status, 403);
    release.resolve();
    responses = await Promise.all(requests);
    assert.equal(responses.filter((response) => response.status === 200).length, 127);
    const overflow = responses.filter((response) => response.status !== 200);
    assert.equal(overflow.length, 13);
    assert.ok(overflow.every((response) => response.status === 400 && /queue is full/.test(response.error)));
    assert.equal(scans, 1, "one thousand pending ticks did not enqueue additional scans");
    assert.equal((await f.rpc("snapshot")).status, 200, "admission recovers after pending requests finish");
    assert.deepEqual(diskState(f.folder), beforeDisk);
    assert.deepEqual(guard.writes, []);
    assert.deepEqual(guard.workers, []);
  } finally {
    release.resolve();
    f.bridge.project.scan = scan;
    guard.restore();
  }
  t.diagnostic(JSON.stringify({ watcherTicks: 1001, actualScans: scans, requests: responses.length, accepted: 127, overflow: 13, queueLimitIncludingRunning: 128, writes: 0 }));
});
