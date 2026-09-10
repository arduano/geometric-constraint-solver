// SPDX-License-Identifier: GPL-3.0-or-later
import assert from "node:assert/strict";
import { test } from "node:test";
import { mkdtemp, rm, appendFile, readFile, writeFile, symlink } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { openCollaborationStorage } from "./collaboration-storage.mjs";
async function fixture(t) { const folder = await mkdtemp(join(tmpdir(), "geosolve-collab-storage-")); t.after(() => rm(folder, { recursive: true, force: true })); return folder; }

test("durable envelope retains exact accepted and invalid working source across reopen", async (t) => {
  const folder = await fixture(t); const store = await openCollaborationStorage(folder, { create: true });
  const accepted = Buffer.from("const width = 12;\n"); const draft = Buffer.from("const width =\n// 🚰");
  const first = await store.append({ kind: "text", operation: "alice-one" }, { acceptedSource: accepted, workingText: draft });
  assert.equal(store.records().length, 1);
  await store.close();
  const reopened = await openCollaborationStorage(folder); t.after(() => reopened.close());
  assert.deepEqual(reopened.records(), [first]);
  assert.deepEqual(await reopened.readBlob(first.attachments.acceptedSource), accepted);
  assert.deepEqual(await reopened.readBlob(first.attachments.workingText), draft);
  const second = await reopened.append({ kind: "semantic", operation: "bob-one" }, { acceptedSource: accepted });
  assert.equal(second.previousDigest, first.digest);
  assert.equal(second.attachments.acceptedSource, first.attachments.acceptedSource);
});

test("durability acknowledgement is withheld while fsync completion is held", async (t) => {
  const folder = await fixture(t); let release; const held = new Promise((resolve) => { release = resolve; }); let reached; const entered = new Promise((resolve) => { reached = resolve; });
  const store = await openCollaborationStorage(folder, { create: true, fault: async (point) => { if (point === "journal-synced") { reached(); await held; } } }); t.after(() => store.close());
  let acknowledged = false;
  const append = store.append({ operation: "one" }).then((record) => { acknowledged = true; return record; });
  await entered; assert.equal(acknowledged, false); assert.equal(store.records().length, 0);
  await assert.rejects(store.append({ operation: "two" }), /one collaboration journal append/u);
  release(); await append; assert.equal(acknowledged, true); assert.equal(store.records().length, 1);
});

test("lost post-sync acknowledgement forces recovery and exposes the one durable record", async (t) => {
  const folder = await fixture(t); const store = await openCollaborationStorage(folder, { create: true, fault: async (point) => { if (point === "journal-synced") throw Error("lost acknowledgement"); } });
  await assert.rejects(store.append({ operation: "one" }, { source: Buffer.from("valid source") }), /lost acknowledgement/u);
  assert.equal(store.needsRecovery, true); assert.equal(store.records().length, 0);
  await assert.rejects(store.append({ operation: "one" }), /reopen\/recovery/u); await store.close();
  const recovered = await openCollaborationStorage(folder); t.after(() => recovered.close());
  assert.equal(recovered.records().length, 1); assert.equal(recovered.records()[0].payload.operation, "one");
});

test("partial journal and corrupt checkpoints are refused without changing bytes", async (t) => {
  const folder = await fixture(t); const store = await openCollaborationStorage(folder, { create: true });
  const record = await store.append({ operation: "one" }, { source: Buffer.from("accepted") }); const directory = store.directory; await store.close();
  const journal = join(directory, "operations.journal"); await appendFile(journal, Buffer.from([3, 0])); const corrupt = await readFile(journal);
  await assert.rejects(openCollaborationStorage(folder), /Partial collaboration journal/u); assert.deepEqual(await readFile(journal), corrupt);
  await writeFile(journal, corrupt.subarray(0, corrupt.length-2));
  const blob = join(directory, "blobs", `${record.attachments.source}.blob`); await writeFile(blob, "replaced");
  await assert.rejects(openCollaborationStorage(folder), /Corrupt collaboration checkpoint/u); assert.equal(await readFile(blob, "utf8"), "replaced");
});

test("mutable caller input is captured before persistence awaits and limits reject cleanly", async (t) => {
  const folder = await fixture(t); const store = await openCollaborationStorage(folder, { create: true, limits: { maxBlobBytes: 64, maxRecordBytes: 1024 } }); t.after(() => store.close());
  const bytes = Buffer.from("before"); const payload = { version: 1 }; const append = store.append(payload, { source: bytes }); bytes.fill(0); payload.version = 99;
  const record = await append; assert.equal(record.payload.version, 1); assert.equal((await store.readBlob(record.attachments.source)).toString(), "before");
  await assert.rejects(store.append({}, { source: Buffer.alloc(65) }), /Invalid collaboration attachment/u);
  assert.equal(store.needsRecovery, false); assert.equal(store.records().length, 1);
});

test("symlinked storage directories cannot redirect collaboration persistence", async (t) => {
  const folder = await fixture(t); const destination = await fixture(t);
  await symlink(destination, join(folder, ".geosolve"));
  await assert.rejects(openCollaborationStorage(folder, { create: true }), /regular local directories/u);
});
