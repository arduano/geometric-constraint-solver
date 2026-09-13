// SPDX-License-Identifier: GPL-3.0-or-later
import test from "node:test";
import assert from "node:assert/strict";
import { mkdtemp, mkdir, open, readFile, writeFile, rename, rm, symlink, unlink } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, dirname } from "node:path";
import { createTrustedSourceHost } from "../packages/geosolve-collaboration/dist/host.js";
import { createCollaborationMirror, externalTextSplices } from "../packages/geosolve-cli/runtime/collaboration-mirror.mjs";

const actor = value => new TextEncoder().encode(value);
async function durable(path, value) {
  const handle = await open(`${path}.tmp`, "w", 0o600);
  try { await handle.writeFile(JSON.stringify(value)); await handle.sync(); } finally { await handle.close(); }
  await rename(`${path}.tmp`, path);
  const dir = await open(dirname(path), "r"); try { await dir.sync(); } finally { await dir.close(); }
}
async function fixture(files = { "main.ts": "a😀0 middle b0\n", "other.ts": "const = (" }) {
  const folder = await mkdtemp(join(tmpdir(), "geosolve-mirror-"));
  for (const [path, value] of Object.entries(files)) { await mkdir(dirname(join(folder, path)), { recursive: true }); await writeFile(join(folder, path), value); }
  await mkdir(join(folder, ".geosolve"));
  const configuration = { documentEpoch: "document-epoch", serverEpoch: "server-epoch", initialInput: "initial", files };
  let source = await createTrustedSourceHost({ configuration, actor: actor("server") });
  let journal = {}, sequence = 0, loseAck = false;
  const requests = [], journalPath = join(folder, ".geosolve", "test-journal.json");
  async function persist(checkpoint) { await durable(journalPath, { checkpoint, journal }); }
  await persist(source.checkpoint());
  const baseOptions = {
    folder, documentId: "document", documentEpoch: configuration.documentEpoch, clientId: "external-mirror", userId: "disk-editor",
    readCommitted: async () => ({ checkpoint: source.textCheckpoint(), snapshot: source.snapshot() }),
    admitWorkingEdits: async request => {
      requests.push(structuredClone(request));
      const key = JSON.stringify(request.operation), previous = journal[key];
      if (previous) { assert.deepEqual(previous.request, request); return previous.result; }
      let stage, result;
      try { stage = source.stageUserWorkingEdits(request.edits, request.operation, request.expectedRevision); result = { status: "committed" }; }
      catch (error) { result = { status: "rejected", reason: error.message }; }
      journal[key] = { request, result };
      await persist(stage?.checkpointJson ?? source.checkpoint());
      if (stage) source.commitStage(stage);
      if (loseAck) { loseAck = false; throw Error("lost ACK after durable commit"); }
      return result;
    },
  };
  return {
    folder, requests, options: baseOptions,
    source: () => source,
    mirror: options => createCollaborationMirror({ ...baseOptions, ...options }),
    write: (path, value) => writeFile(join(folder, path), value),
    read: path => readFile(join(folder, path), "utf8"),
    loseAck() { loseAck = true; },
    async shared(edits) {
      const stage = source.stageUserWorkingEdits(edits, { userId: "bob", clientId: "bob-tab", requestId: `shared-${++sequence}` });
      await persist(stage.checkpointJson); source.commitStage(stage);
    },
    async restart() {
      const saved = JSON.parse(await readFile(journalPath, "utf8")); journal = saved.journal;
      source.dispose(); source = await createTrustedSourceHost({ configuration: { ...configuration, serverEpoch: `server-${++sequence}` }, actor: actor(`server-${sequence}`), checkpointJson: saved.checkpoint });
    },
    async close() { source.dispose(); await rm(folder, { recursive: true, force: true }); },
  };
}
const splice = (path, start, deleted, insert) => ({ kind: "splice", path, start_utf16: start, delete_utf16: deleted, insert });

test("scalar multi-hunk external diff roundtrips distinct edits and wide changes", () => {
  for (const [before, after] of [["a😀0 middle b0", "a🦀1 middle b2"], ["abc", ""], ["", "abc"], ["abc", "axyzc"], ["a".repeat(800), "b".repeat(800)]]) {
    let reconstructed = before;
    for (const edit of externalTextSplices(before, after).reverse()) reconstructed = reconstructed.slice(0, edit.start) + edit.insert + reconstructed.slice(edit.end);
    assert.equal(reconstructed, after);
  }
  assert.equal(externalTextSplices("a0 middle b0", "a1 middle b2").length, 2);
});

test("actual native three-way mirror preserves shared text between separate external Unicode edits and invalid syntax", async () => {
  const fx = await fixture();
  try {
    const mirror = await fx.mirror(); assert.equal((await mirror.reconcile()).status, "synchronized");
    await fx.write("main.ts", "a🦀1 middle b2\n");
    await fx.shared([splice("main.ts", 5, 6, "SHARED")]);
    const accepted = fx.source().snapshot().accepted;
    assert.equal((await mirror.reconcile()).status, "synchronized");
    assert.equal(await fx.read("main.ts"), "a🦀1 SHARED b2\n");
    assert.equal(fx.source().snapshot().working.files["main.ts"], await fx.read("main.ts"));
    assert.deepEqual(fx.source().snapshot().accepted, accepted);
    assert.equal(fx.source().userHistory("disk-editor").undoCount, 1);
    assert.equal(await fx.read("other.ts"), "const = (");
  } finally { await fx.close(); }
});

test("overlapping external bytes stay on disk and in protected storage without overwriting shared source", async () => {
  const fx = await fixture({ "main.ts": "width = 12;\n" });
  try {
    const mirror = await fx.mirror(); await mirror.reconcile();
    await fx.write("main.ts", "width = 14;\n");
    await fx.shared([splice("main.ts", 8, 2, "16")]);
    const result = await mirror.reconcile(); assert.equal(result.status, "reconciliation_pending"); assert.equal(fx.requests.length, 0);
    assert.equal(await fx.read("main.ts"), "width = 14;\n");
    assert.equal(fx.source().snapshot().working.files["main.ts"], "width = 16;\n");
    assert.equal(await readFile(join(fx.folder, ".geosolve/collaboration-mirror/blobs", result.notices[0].externalBlob), "utf8"), "width = 14;\n");
    await fx.restart(); const restarted = await fx.mirror(); assert.equal((await restarted.reconcile()).status, "reconciliation_pending");
    // An explicit choice of current shared bytes resolves the pending mirror.
    await fx.write("main.ts", "width = 16;\n"); assert.equal((await restarted.reconcile()).status, "synchronized");
  } finally { await fx.close(); }
});

test("external rename plus typing retains file identity and atomically includes creation and deletion", async () => {
  const fx = await fixture({ "main.ts": "a0b0", "remove.ts": "delete me" });
  try {
    const mirror = await fx.mirror(); await mirror.reconcile(); const original = fx.source().snapshot().fileIds["main.ts"];
    await rename(join(fx.folder, "main.ts"), join(fx.folder, "renamed.ts"));
    await fx.write("renamed.ts", "a1b0"); await fx.write("new.ts", "export const = ("); await unlink(join(fx.folder, "remove.ts"));
    await fx.shared([splice("main.ts", 3, 1, "2")]);
    assert.equal((await mirror.reconcile()).status, "synchronized");
    assert.equal(fx.source().snapshot().fileIds["renamed.ts"], original);
    assert.deepEqual(fx.source().snapshot().working.files, { "new.ts": "export const = (", "renamed.ts": "a1b2" });
    assert.equal(fx.requests.length, 1);
    await fx.restart(); assert.equal((await (await fx.mirror()).reconcile()).status, "synchronized");
  } finally { await fx.close(); }
});

test("shared rename uses stable native identity while external edits still refer to exported path", async () => {
  const fx = await fixture({ "main.ts": "a0b0" });
  try {
    const mirror = await fx.mirror(); await mirror.reconcile(); const original = fx.source().snapshot().fileIds["main.ts"];
    await fx.shared([{ kind: "rename_file", path: "main.ts", new_path: "shared.ts" }, splice("shared.ts", 3, 1, "2")]);
    await fx.write("main.ts", "a1b0");
    assert.equal((await mirror.reconcile()).status, "synchronized");
    assert.equal(await fx.read("shared.ts"), "a1b2"); await assert.rejects(fx.read("main.ts"), { code: "ENOENT" });
    assert.equal(fx.source().snapshot().fileIds["shared.ts"], original);
  } finally { await fx.close(); }
});

test("external deletion competes with shared edits and does not delete the shared file", async () => {
  const fx = await fixture({ "main.ts": "a0" });
  try {
    const mirror = await fx.mirror(); await mirror.reconcile(); await unlink(join(fx.folder, "main.ts"));
    await fx.shared([splice("main.ts", 1, 1, "1")]);
    assert.equal((await mirror.reconcile()).status, "reconciliation_pending");
    assert.equal(fx.requests.length, 0); assert.equal(fx.source().snapshot().working.files["main.ts"], "a1");
    await assert.rejects(fx.read("main.ts"), { code: "ENOENT" });
  } finally { await fx.close(); }
});

test("lost admission ACK retries the exact durable operation and payload after process reconstruction", async () => {
  const fx = await fixture({ "main.ts": "a0" });
  try {
    const mirror = await fx.mirror(); await mirror.reconcile(); await fx.write("main.ts", "a1"); fx.loseAck();
    await assert.rejects(mirror.reconcile(), /lost ACK/u);
    assert.equal((await mirror.status()).pendingPhase, "admission"); assert.equal(fx.source().snapshot().working.files["main.ts"], "a1");
    await fx.restart(); const recovered = await fx.mirror(); assert.equal((await recovered.reconcile()).status, "synchronized");
    assert.equal(fx.requests.length, 2); assert.deepEqual(fx.requests[0], fx.requests[1]);
    assert.equal(fx.source().userHistory("disk-editor").undoCount, 1);
  } finally { await fx.close(); }
});

for (const crashPhase of ["after-admission", "after-preserve", "after-install", "before-manifest"]) {
  test(`durable mirror recovery resumes ${crashPhase} without reimporting partial exports`, async () => {
    const fx = await fixture({ "main.ts": "a0b0" });
    let crash = true;
    try {
      const mirror = await fx.mirror({ onPhase: phase => { if (phase === crashPhase && crash) { crash = false; throw Error(`crash ${phase}`); } } });
      await mirror.reconcile(); await fx.write("main.ts", "a1b0"); await fx.shared([splice("main.ts", 3, 1, "2")]);
      await assert.rejects(mirror.reconcile(), /crash/u);
      await fx.restart(); const recovered = await fx.mirror(); assert.equal((await recovered.reconcile()).status, "synchronized");
      assert.equal(await fx.read("main.ts"), "a1b2"); assert.equal(fx.source().userHistory("disk-editor").undoCount, 1);
    } finally { await fx.close(); }
  });
}

test("new external bytes racing publication are preserved with a pending export across restart", async () => {
  const fx = await fixture({ "main.ts": "a0b0" }); let changed = false;
  try {
    const mirror = await fx.mirror({ onPhase: async phase => { if (phase === "after-admission" && !changed) { changed = true; await fx.write("main.ts", "COMPETING EXTERNAL"); } } });
    await mirror.reconcile(); await fx.write("main.ts", "a1b0"); await fx.shared([splice("main.ts", 3, 1, "2")]);
    assert.equal((await mirror.reconcile()).status, "reconciliation_pending"); assert.equal(await fx.read("main.ts"), "COMPETING EXTERNAL");
    assert.equal(fx.source().snapshot().working.files["main.ts"], "a1b2");
    await fx.restart(); const recovered = await fx.mirror(); assert.equal((await recovered.reconcile()).status, "reconciliation_pending");
    assert.equal(await fx.read("main.ts"), "COMPETING EXTERNAL");
    await fx.write("main.ts", "a1b2"); assert.equal((await recovered.reconcile()).status, "synchronized");
    assert.equal(fx.source().userHistory("disk-editor").undoCount, 1);
  } finally { await fx.close(); }
});

test("scanner excludes build metadata and rejects symlinks, malformed UTF-8, oversized files and forged manifest paths", async () => {
  const fx = await fixture({ "main.ts": "a0" });
  try {
    const mirror = await fx.mirror(); await mirror.reconcile();
    await mkdir(join(fx.folder, "node_modules")); await writeFile(join(fx.folder, "node_modules", "ignored.ts"), "ignored");
    await writeFile(join(fx.folder, "notes.txt"), "not authored"); assert.equal((await mirror.reconcile()).status, "synchronized");
    await symlink(join(fx.folder, "main.ts"), join(fx.folder, "linked.ts")); await assert.rejects(mirror.reconcile(), /symlink/u); await unlink(join(fx.folder, "linked.ts"));
    await writeFile(join(fx.folder, "bad.ts"), Buffer.from([0xff])); await assert.rejects(mirror.reconcile(), /encoded data/u); await unlink(join(fx.folder, "bad.ts"));
    await writeFile(join(fx.folder, "huge.ts"), Buffer.alloc(4 * 1024 * 1024 + 1)); await assert.rejects(mirror.reconcile(), /bounded regular/u); await unlink(join(fx.folder, "huge.ts"));
    const manifestPath = join(fx.folder, ".geosolve", "collaboration-mirror", "manifest.json"), manifest = JSON.parse(await readFile(manifestPath, "utf8"));
    manifest.basis.disk["../escape.ts"] = manifest.basis.disk["main.ts"]; await writeFile(manifestPath, JSON.stringify(manifest));
    await assert.rejects(mirror.reconcile(), /Corrupt mirror manifest checksum/u); assert.equal(fx.requests.length, 0);
  } finally { await fx.close(); }
});

test("first attachment never infers authority over different existing external bytes", async () => {
  const fx = await fixture({ "main.ts": "a0" });
  try {
    await fx.write("main.ts", "external-original"); const mirror = await fx.mirror();
    assert.equal((await mirror.reconcile()).status, "reconciliation_pending"); assert.equal((await mirror.reconcile()).status, "reconciliation_pending");
    assert.equal(fx.requests.length, 0); assert.equal(await fx.read("main.ts"), "external-original");
  } finally { await fx.close(); }
});

test("same-value shared text replacement still blocks an external deletion",async()=>{
  const fx=await fixture({"main.ts":"a0"});
  try{
    const mirror=await fx.mirror();await mirror.reconcile();
    await fx.shared([splice("main.ts",1,1,"0")]);await unlink(join(fx.folder,"main.ts"));
    assert.equal((await mirror.reconcile()).status,"reconciliation_pending");assert.equal(fx.requests.length,0);
    assert.equal(fx.source().snapshot().working.files["main.ts"],"a0");
  }finally{await fx.close();}
});

test("ambiguous byte-identical deleted files cannot assign a new external path an arbitrary identity",async()=>{
  const fx=await fixture({"a.ts":"same","b.ts":"same"});
  try{
    const mirror=await fx.mirror();await mirror.reconcile();
    await fx.write("new.ts","same");await unlink(join(fx.folder,"a.ts"));await unlink(join(fx.folder,"b.ts"));
    assert.equal((await mirror.reconcile()).status,"reconciliation_pending");assert.equal(fx.requests.length,0);
    assert.deepEqual(fx.source().snapshot().working.files,{"a.ts":"same","b.ts":"same"});
  }finally{await fx.close();}
});

test("a competing replacement immediately before filesystem preservation is restored instead of overwritten",async()=>{
  const fx=await fixture({"main.ts":"a0"});let raced=false;
  try{
    const mirror=await fx.mirror({onPhase:async phase=>{if(phase==="before-preserve"&&!raced){raced=true;await fx.write("main.ts","late external bytes");}}});
    await mirror.reconcile();await fx.shared([splice("main.ts",1,1,"1")]);
    assert.equal((await mirror.reconcile()).status,"reconciliation_pending");
    assert.equal(await fx.read("main.ts"),"late external bytes");assert.equal(fx.source().snapshot().working.files["main.ts"],"a1");
    await fx.restart();assert.equal((await(await fx.mirror()).reconcile()).status,"reconciliation_pending");
    assert.equal(await fx.read("main.ts"),"late external bytes");
  }finally{await fx.close();}
});

test("a durable rejected request is not resubmitted until its disk or shared basis changes",async()=>{
  const fx=await fixture({"main.ts":"a0"});let calls=0;
  try{
    const mirror=await fx.mirror({admitWorkingEdits:async()=>{calls++;return{status:"rejected",reason:"quota"};}});
    await mirror.reconcile();await fx.write("main.ts","a1");
    assert.equal((await mirror.reconcile()).status,"reconciliation_pending");
    assert.equal((await mirror.reconcile()).status,"reconciliation_pending");assert.equal(calls,1);
    await fx.write("main.ts","a2");await mirror.reconcile();assert.equal(calls,2);
  }finally{await fx.close();}
});

test("an actual disk rename retains identity even when a deleted sibling has identical bytes",async()=>{
  const fx=await fixture({"a.ts":"same","b.ts":"same"});
  try{
    const mirror=await fx.mirror();await mirror.reconcile();const id=fx.source().snapshot().fileIds["a.ts"];
    await rename(join(fx.folder,"a.ts"),join(fx.folder,"moved.ts"));await unlink(join(fx.folder,"b.ts"));
    assert.equal((await mirror.reconcile()).status,"synchronized");assert.equal(fx.source().snapshot().fileIds["moved.ts"],id);
  }finally{await fx.close();}
});

test("a stale admission races safely and retries against a new shared basis without losing disk bytes",async()=>{
  const fx=await fixture({"main.ts":"a0b0"});let raced=false;
  try{
    const mirror=await fx.mirror({admitWorkingEdits:async request=>{if(!raced){raced=true;await fx.shared([splice("main.ts",3,1,"2")]);}return fx.options.admitWorkingEdits(request);}});
    await mirror.reconcile();await fx.write("main.ts","a1b0");
    assert.equal((await mirror.reconcile()).status,"reconciliation_pending");assert.equal(await fx.read("main.ts"),"a1b0");
    assert.equal((await mirror.reconcile()).status,"synchronized");assert.equal(await fx.read("main.ts"),"a1b2");
    assert.equal(fx.requests.length,2);assert.notEqual(fx.requests[0].operation.requestId,fx.requests[1].operation.requestId);
  }finally{await fx.close();}
});

test("recovery restores competing bytes captured immediately before a crash after rename",async()=>{
  const fx=await fixture({"main.ts":"a0"});let raced=false;
  try{
    const mirror=await fx.mirror({onPhase:async phase=>{
      if(phase==="before-preserve"&&!raced){raced=true;await fx.write("main.ts","competing original");}
      if(phase==="after-move")throw Error("crash after moving competing bytes");
    }});
    await mirror.reconcile();await fx.shared([splice("main.ts",1,1,"1")]);
    await assert.rejects(mirror.reconcile(),/crash/u);await fx.restart();
    assert.equal((await(await fx.mirror()).reconcile()).status,"reconciliation_pending");
    assert.equal(await fx.read("main.ts"),"competing original");assert.equal(fx.source().snapshot().working.files["main.ts"],"a1");
  }finally{await fx.close();}
});

test("external replacement beyond one native anchor span keeps bounded chunk ownership and valid raw text",async()=>{
  const fx=await fixture({"main.ts":"a".repeat(65_536)+"🦀end"});
  try{
    const mirror=await fx.mirror();await mirror.reconcile();await fx.write("main.ts","const = (");
    assert.equal((await mirror.reconcile()).status,"synchronized");
    assert.equal(fx.source().snapshot().working.files["main.ts"],"const = (");assert.equal(await fx.read("main.ts"),"const = (");
  }finally{await fx.close();}
});

test("external occupied-path swaps retain both native identities",async()=>{
  const fx=await fixture({"a.ts":"first","b.ts":"second"});
  try{
    const mirror=await fx.mirror();await mirror.reconcile();const ids=fx.source().snapshot().fileIds;
    await rename(join(fx.folder,"a.ts"),join(fx.folder,"swap.tmp"));await rename(join(fx.folder,"b.ts"),join(fx.folder,"a.ts"));await rename(join(fx.folder,"swap.tmp"),join(fx.folder,"b.ts"));
    assert.equal((await mirror.reconcile()).status,"synchronized");
    assert.equal(fx.source().snapshot().fileIds["a.ts"],ids["b.ts"]);assert.equal(fx.source().snapshot().fileIds["b.ts"],ids["a.ts"]);
    assert.deepEqual(fx.source().snapshot().working.files,{"a.ts":"second","b.ts":"first"});
  }finally{await fx.close();}
});

test("external rename plus replacement at the old path preserves moved identity and creates a fresh file",async()=>{
  const fx=await fixture({"main.ts":"original"});
  try{
    const mirror=await fx.mirror();await mirror.reconcile();const id=fx.source().snapshot().fileIds["main.ts"];
    await rename(join(fx.folder,"main.ts"),join(fx.folder,"moved.ts"));await fx.write("main.ts","new file");
    assert.equal((await mirror.reconcile()).status,"synchronized");
    assert.equal(fx.source().snapshot().fileIds["moved.ts"],id);assert.notEqual(fx.source().snapshot().fileIds["main.ts"],id);
    assert.deepEqual(fx.source().snapshot().working.files,{"main.ts":"new file","moved.ts":"original"});
  }finally{await fx.close();}
});
