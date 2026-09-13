// SPDX-License-Identifier: GPL-3.0-or-later
import test from "node:test";
import assert from "node:assert/strict";
import {
  chmodSync, closeSync, existsSync, mkdtempSync, mkdirSync, openSync, readFileSync,
  renameSync, rmSync, statSync, symlinkSync, writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { spawn, spawnSync } from "node:child_process";
import { once } from "node:events";
import { createHash } from "node:crypto";
import { acquireWorkspaceLock, createWorkspaceStorage } from "../packages/geosolve-cli/runtime/workspace-storage.mjs";

const hash = (bytes) => createHash("sha256").update(bytes).digest("hex");
const storageUrl = new URL("../packages/geosolve-cli/runtime/workspace-storage.mjs", import.meta.url).href;

function fixture(t, options = {}) {
  const folder = mkdtempSync(join(tmpdir(), "geosolve-storage-"));
  writeFileSync(join(folder, "sketch.ts"), "before");
  chmodSync(join(folder, "sketch.ts"), 0o640);
  let lock = acquireWorkspaceLock(folder);
  let storage = createWorkspaceStorage(folder, { lock, ...options });
  t.after(() => { lock.release(); rmSync(folder, { recursive: true, force: true }); });
  return {
    folder, get storage() { return storage; }, get lock() { return lock; },
    restart(nextOptions = {}) {
      lock.release(); lock = acquireWorkspaceLock(folder);
      storage = createWorkspaceStorage(folder, { lock, ...nextOptions });
      return storage.reconcile();
    },
    input(operationId = "edit-1", text = "after", expectedHash = hash("before")) {
      return { operationId, files: [{ path: "sketch.ts", expectedHash, bytes: text }] };
    },
    read(path = "sketch.ts") { return readFileSync(join(folder, path), "utf8"); },
  };
}

const crashAt = (target) => (point) => { if (point === target) throw Object.assign(Error(`crash:${target}`), { simulateCrash: true }); };

test("publication preserves permissions, all byte alternatives and operation receipts", (t) => {
  const f = fixture(t);
  const originalInode = statSync(join(f.folder, "sketch.ts")).ino;
  const published = f.storage.publish(f.input());
  assert.equal(published.state, "published");
  assert.equal(f.read(), "after");
  assert.equal(statSync(join(f.folder, "sketch.ts")).mode & 0o777, 0o640);
  assert.equal(statSync(published.files[0].locations.before).ino, originalInode);
  assert.equal(readFileSync(published.files[0].locations.original, "utf8"), "before");
  assert.equal(readFileSync(published.files[0].locations.candidate, "utf8"), "after");
  assert.notEqual(statSync(published.files[0].locations.candidate).ino, statSync(join(f.folder, "sketch.ts")).ino);
  assert.equal(f.storage.acknowledge("edit-1").state, "acknowledged");
  assert.equal(f.storage.publish(f.input()).state, "acknowledged");
  assert.throws(() => f.storage.publish(f.input("edit-1", "different")), /already used/);
  f.restart();
  assert.equal(f.storage.outcome("edit-1").state, "acknowledged");
});

for (const point of ["staging", "staged", "before-displace", "displaced", "published-file", "published", "before-acknowledge", "acknowledged"]) {
  test(`restart reconciles crash at ${point} without losing candidate or original`, (t) => {
    const f = fixture(t, { fault: crashAt(point) });
    assert.throws(() => {
      f.storage.publish(f.input());
      f.storage.acknowledge("edit-1");
    }, /crash:/);
    const records = f.restart();
    assert.equal(records.length, 1);
    const record = records[0];
    const afterPublication = ["published-file", "published", "before-acknowledge", "acknowledged"].includes(point);
    assert.equal(f.read(), afterPublication ? "after" : "before");
    assert.equal(record.state, point === "acknowledged" ? "acknowledged" : afterPublication ? "published" : "conflict");
    assert.equal(readFileSync(record.files[0].locations.candidate, "utf8"), "after");
    assert.equal(readFileSync(record.files[0].locations.original, "utf8"), "before");
    if (afterPublication) assert.equal(f.storage.acknowledge("edit-1").state, "acknowledged");
  });
}

test("stale expected hash rejects before any displacement", (t) => {
  const f = fixture(t);
  writeFileSync(join(f.folder, "sketch.ts"), "external");
  assert.throws(() => f.storage.publish(f.input()), (error) => error.conflict && error.operationId === "edit-1");
  assert.equal(f.read(), "external");
  assert.equal(f.storage.outcome("edit-1").state, "conflict");
});

test("external rename during displacement preserves the replaced and staged bytes", (t) => {
  let once = true;
  const f = fixture(t, { fault(point, paths) {
    if (point === "before-displace" && once) {
      once = false;
      writeFileSync(join(f.folder, "external.tmp"), "external replacement");
      renameSync(join(f.folder, "external.tmp"), paths.current);
    }
  } });
  assert.throws(() => f.storage.publish(f.input()), /External write during displacement/);
  assert.equal(f.read(), "external replacement");
  const retained = f.storage.inspect("edit-1").files[0];
  assert.equal(readFileSync(retained.locations.before, "utf8"), "external replacement");
  assert.equal(readFileSync(retained.locations.original, "utf8"), "before");
  assert.equal(readFileSync(retained.locations.candidate, "utf8"), "after");
});

test("external creation in missing-path interval is never overwritten", (t) => {
  const f = fixture(t, { fault(point, paths) { if (point === "displaced") writeFileSync(paths.current, "newer external", { flag: "wx" }); } });
  assert.throws(() => f.storage.publish(f.input()), (error) => error.code === "EEXIST");
  assert.equal(f.read(), "newer external");
  const record = f.storage.inspect("edit-1");
  assert.equal(readFileSync(record.files[0].locations.before, "utf8"), "before");
  assert.equal(readFileSync(record.files[0].locations.candidate, "utf8"), "after");
  f.restart(); assert.equal(f.read(), "newer external");
});

test("external old-descriptor in-place write prevents false acknowledgment", (t) => {
  const f = fixture(t, { fault(point) { if (point === "before-acknowledge") writeFileSync(fd, "late external", { flag: "w" }); } });
  const fd = openSync(join(f.folder, "sketch.ts"), "r+");
  t.after(() => closeSync(fd));
  const publication = f.storage.publish(f.input());
  assert.throws(() => f.storage.acknowledge("edit-1"), /changed before acknowledgment/);
  assert.equal(f.read(), "after");
  assert.equal(readFileSync(publication.files[0].locations.before, "utf8"), "late external");
  assert.equal(readFileSync(publication.files[0].locations.original, "utf8"), "before");
  assert.equal(f.storage.prune().removed.length, 0);
});

test("live in-place writes preserve the independent staged candidate", (t) => {
  const f = fixture(t);
  const publication = f.storage.publish(f.input());
  writeFileSync(join(f.folder, "sketch.ts"), "new live text");
  assert.throws(() => f.storage.acknowledge("edit-1"), /changed or is incomplete/);
  assert.equal(readFileSync(publication.files[0].locations.candidate, "utf8"), "after");
  f.restart(); assert.equal(f.read(), "new live text");
});

test("partial multi-file publication restores only missing files, retaining newer bytes", (t) => {
  const f = fixture(t, { fault(point, context) {
    if (point === "displaced" && context.index === 1) {
      writeFileSync(context.current, "new external second file");
      throw Object.assign(Error("crash:second-file"), { simulateCrash: true });
    }
  } });
  writeFileSync(join(f.folder, "patch.ts"), "old patch");
  assert.throws(() => f.storage.publish({ operationId: "multi", files: [
    { path: "patch.ts", expectedHash: hash("old patch"), bytes: "new patch" },
    { path: "sketch.ts", expectedHash: hash("before"), bytes: "new sketch" },
  ] }), /crash:second-file/);
  f.restart();
  assert.equal(f.read("patch.ts"), "new patch");
  assert.equal(f.read(), "new external second file");
  const record = f.storage.inspect("multi");
  assert.equal(record.state, "conflict");
  assert.equal(readFileSync(record.files[1].locations.before, "utf8"), "before");
  assert.throws(() => f.storage.resolve({ operationId: "multi", expectedHashes: { "patch.ts": hash("new patch"), "sketch.ts": hash("before") } }), /exact current/);
  const resolution = f.storage.resolve({ operationId: "multi", expectedHashes: { "patch.ts": hash("new patch"), "sketch.ts": hash("new external second file") } });
  assert.equal(resolution.state, "resolved");
  assert.equal(f.read(), "new external second file");
});

test("source and design sidecar publish together; explicit deletion also recovers", (t) => {
  const f = fixture(t);
  const record = f.storage.publish({ operationId: "sidecar", files: [
    { path: "sketch.ts", expectedHash: hash("before"), bytes: "after" },
    { path: ".geosolve/design.json", expectedHash: null, bytes: '{"format":"test"}' },
  ] });
  assert.equal(record.state, "published");
  assert.equal(f.read(".geosolve/design.json"), '{"format":"test"}');
  f.storage.acknowledge("sidecar");
  f.storage.publish({ operationId: "delete", files: [{ path: ".geosolve/design.json", expectedHash: hash('{"format":"test"}'), bytes: null }],
    expectedInputs: [{ path: ".geosolve/design.json", expectedHash: hash('{"format":"test"}') }] });
  f.storage.acknowledge("delete");
  f.restart(); assert.equal(existsSync(join(f.folder, ".geosolve/design.json")), false);
});

test("imported dependency changes invalidate source publication and acknowledgment", (t) => {
  const f = fixture(t);
  writeFileSync(join(f.folder, "patch.ts"), "patch before");
  const input = { ...f.input(), expectedInputs: [{ path: "patch.ts", expectedHash: hash("patch before") }] };
  f.storage.publish(input);
  writeFileSync(join(f.folder, "patch.ts"), "patch after");
  assert.throws(() => f.storage.acknowledge("edit-1"), /changed or is incomplete/);
  f.restart();
  assert.equal(f.storage.outcome("edit-1").state, "conflict");
  assert.equal(f.read("patch.ts"), "patch after");
  assert.throws(() => f.storage.publish({ ...input, operationId: "edit-2", files: [{ path: "sketch.ts", expectedHash: hash("after"), bytes: "next" }] }), /Stale project input/);
  assert.equal(f.read(), "after");
});

test("missing source requires explicit absence authority; failed write retains candidate", (t) => {
  const f = fixture(t, { fault(point) {
    if (point === "staged") throw Object.assign(Error("simulated fsync EIO"), { code: "EIO" });
  } });
  assert.throws(() => f.storage.publish(f.input()), (error) => error.code === "EIO");
  assert.equal(f.read(), "before");
  assert.equal(readFileSync(f.storage.outcome("edit-1").files[0].locations.candidate, "utf8"), "after");
  f.restart(); rmSync(join(f.folder, "sketch.ts"));
  assert.throws(() => f.storage.publish(f.input("missing")), /Stale input/);
  f.storage.publish(f.input("create", "recreated", null));
  f.storage.acknowledge("create");
  assert.equal(f.read(), "recreated");
});

test("acknowledged live in-place edits remain recoverable after a later external replacement", (t) => {
  const f = fixture(t, { historyLimit: 0 });
  const record = f.storage.publish(f.input()); f.storage.acknowledge("edit-1");
  writeFileSync(join(f.folder, "sketch.ts"), "intermediate external text");
  writeFileSync(join(f.folder, "replace.tmp"), "newest external text");
  renameSync(join(f.folder, "replace.tmp"), join(f.folder, "sketch.ts"));
  const pruning = f.storage.prune({ allowUnverifiedDescriptors: true });
  assert.deepEqual(pruning.removed, []);
  assert.equal(pruning.deferred[0].reason, "retained-bytes-changed");
  assert.equal(readFileSync(record.files[0].locations.publication, "utf8"), "intermediate external text");
  assert.equal(readFileSync(record.files[0].locations.candidate, "utf8"), "after");
  assert.equal(f.read(), "newest external text");
});

test("acknowledged history prunes payloads but operation IDs remain idempotent", (t) => {
  const f = fixture(t, { historyLimit: 1 });
  f.storage.publish(f.input()); f.storage.acknowledge("edit-1");
  f.storage.publish(f.input("edit-2", "latest", hash("after"))); f.storage.acknowledge("edit-2");
  // Explicit cleanup opt-in on hosts where protected GUI processes hide their FDs.
  const pruning = f.storage.prune({ allowUnverifiedDescriptors: true });
  assert.deepEqual(pruning.removed, ["edit-1"]);
  assert.equal(f.storage.outcome("edit-1").pruned, true);
  assert.equal(f.storage.publish(f.input()).state, "acknowledged");
  assert.equal(f.read(), "latest");
});

test("open writable displaced descriptors are retained even before a late write", (t) => {
  const f = fixture(t, { historyLimit: 0 });
  const fd = openSync(join(f.folder, "sketch.ts"), "r+");
  t.after(() => closeSync(fd));
  const record = f.storage.publish(f.input()); f.storage.acknowledge("edit-1");
  const pruning = f.storage.prune();
  assert.equal(pruning.removed.length, 0);
  assert.equal(pruning.deferred.length, 1);
  assert.equal(f.storage.prune({ allowUnverifiedDescriptors: true }).deferred[0].reason, "writable-descriptor-open");
  writeFileSync(fd, "late external");
  assert.equal(readFileSync(record.files[0].locations.before, "utf8"), "late external");
  assert.equal(f.storage.prune().deferred[0].reason, "retained-bytes-changed");
});

test("restart finishes interrupted payload pruning without forgetting operation identity", (t) => {
  const f = fixture(t, { historyLimit: 0, fault: crashAt("pruned-receipt") });
  const record = f.storage.publish(f.input()); f.storage.acknowledge("edit-1");
  assert.throws(() => f.storage.prune({ allowUnverifiedDescriptors: true }), /crash:pruned-receipt/);
  assert.equal(existsSync(record.files[0].locations.candidate), true);
  f.restart();
  assert.equal(existsSync(record.files[0].locations.candidate), false);
  assert.equal(f.storage.outcome("edit-1").state, "acknowledged");
  assert.equal(f.storage.publish(f.input()).state, "acknowledged");
  assert.equal(f.read(), "after");
});

test("unresolved competing bytes and malformed journals are never pruned", (t) => {
  const f = fixture(t, { historyLimit: 0 });
  f.storage.publish(f.input());
  writeFileSync(join(f.folder, "sketch.ts"), "external");
  f.restart({ historyLimit: 0 });
  const broken = join(f.storage.operations, "malformed"); mkdirSync(broken);
  writeFileSync(join(broken, "manifest.json"), Buffer.from([0xff]));
  assert.equal(f.storage.reconcile().find((item) => item.operationId === "malformed").state, "unreadable");
  assert.deepEqual(f.storage.prune().removed, []);
  assert.equal(existsSync(broken), true);
});

test("publication refuses escaping, symlink and reserved storage paths", (t) => {
  const f = fixture(t);
  symlinkSync(f.folder, join(f.folder, "alias"));
  symlinkSync(join(f.folder, "sketch.ts"), join(f.folder, "linked.ts"));
  for (const path of ["../outside.ts", "/outside.ts", "alias/sketch.ts", "linked.ts", ".geosolve/bridge.lock"]) {
    assert.throws(() => f.storage.publish({ operationId: `bad-${Math.random().toString(16).slice(2)}`, files: [{ path, expectedHash: hash("before"), bytes: "bad" }] }));
  }
  assert.equal(f.read(), "before");
});

test("canonical folder lock refuses same-process aliases and live other processes", (t) => {
  const f = fixture(t);
  const alias = `${f.folder}-alias`; symlinkSync(f.folder, alias);
  t.after(() => rmSync(alias));
  assert.throws(() => acquireWorkspaceLock(alias), (error) => error.code === "WORKSPACE_LOCKED");
  const child = spawnSync(process.execPath, ["--input-type=module", "-e", `import { acquireWorkspaceLock } from ${JSON.stringify(storageUrl)}; try { acquireWorkspaceLock(process.argv[1]); process.exit(2); } catch(error) { if(error.code !== 'WORKSPACE_LOCKED') throw error; }`, f.folder]);
  assert.equal(child.status, 0, child.stderr.toString());
  f.lock.release();
  assert.throws(() => f.storage.read("sketch.ts"), /released/);
  f.restart(); assert.equal(f.read(), "before");
});

test("actual process death releases kernel lock and recovers stale process identity", async (t) => {
  const f = fixture(t); f.lock.release();
  const child = spawn(process.execPath, ["--input-type=module", "-e", `import { acquireWorkspaceLock } from ${JSON.stringify(storageUrl)}; acquireWorkspaceLock(process.argv[1]); console.log('locked'); setInterval(() => {}, 1000);`, f.folder], { stdio: ["ignore", "pipe", "pipe"] });
  t.after(() => { if (child.exitCode === null) child.kill("SIGKILL"); });
  await once(child.stdout, "data");
  assert.throws(() => acquireWorkspaceLock(f.folder), (error) => error.code === "WORKSPACE_LOCKED");
  const exited = once(child, "exit"); child.kill("SIGKILL"); await exited;
  f.restart();
  assert.equal(f.lock.staleOwnerRecovered, true);
  assert.equal(f.lock.previousOwner.pid, child.pid);
});

test("actual process termination after displacement restores durable original on startup", (t) => {
  const f = fixture(t); f.lock.release();
  const child = spawnSync(process.execPath, ["--input-type=module", "-e", `
    import { acquireWorkspaceLock, createWorkspaceStorage } from ${JSON.stringify(storageUrl)};
    const folder=process.argv[1]; const lock=acquireWorkspaceLock(folder);
    const storage=createWorkspaceStorage(folder,{lock,fault(point){if(point==='displaced')process.kill(process.pid,'SIGKILL');}});
    storage.publish(JSON.parse(process.argv[2]));
  `, f.folder, JSON.stringify(f.input())]);
  assert.equal(child.signal, "SIGKILL", child.stderr.toString());
  assert.equal(existsSync(join(f.folder, "sketch.ts")), false);
  f.restart();
  assert.equal(f.lock.staleOwnerRecovered, true);
  assert.equal(f.read(), "before");
  const record = f.storage.outcome("edit-1");
  assert.equal(record.state, "conflict");
  assert.equal(readFileSync(record.files[0].locations.candidate, "utf8"), "after");
});

test("PID reuse metadata and corrupt owner JSON cannot block a released kernel lock", (t) => {
  const f = fixture(t); f.lock.release();
  writeFileSync(join(f.folder, ".geosolve/bridge-owner.json"), JSON.stringify({ pid: process.pid, startTicks: "wrong", bootId: "wrong" }));
  f.restart(); assert.equal(f.lock.staleOwnerRecovered, true);
  f.lock.release(); writeFileSync(join(f.folder, ".geosolve/bridge-owner.json"), Buffer.from([0xff]));
  f.restart(); assert.equal(f.read(), "before");
});
