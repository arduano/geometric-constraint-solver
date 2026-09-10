// SPDX-License-Identifier: GPL-3.0-or-later
import test from "node:test";
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { createSharedText } from "../dist/index.js";

const actor = (name) => new TextEncoder().encode(name);
async function seeded(text) {
  const alice = await createSharedText({ actor: actor("alice") });
  alice.edit([{ kind: "create_file", path: "main.ts", text }]);
  return [alice, alice.fork(actor("bob"))];
}
const splice = (doc, start, count, insert) => doc.edit([{ kind: "splice", path: "main.ts", start_utf16: start, delete_utf16: count, insert }]);
function sync(a, b) {
  for (let i = 0; i < 20; i++) {
    const am = a.generateSyncMessage("bob"), bm = b.generateSyncMessage("alice");
    if (!am && !bm) { assert.deepEqual(a.capture(), b.capture()); return; }
    if (am) b.receiveSyncMessage("alice", am);
    if (bm) a.receiveSyncMessage("bob", bm);
  }
  throw Error("Sync did not settle within its bound");
}

test("actual WASM shares raw invalid Unicode source and immutable Apply capture", async () => {
  const [alice, bob] = await seeded("a😀bc");
  try {
    const capture = alice.capture();
    const cursor = alice.cursor("main.ts", 3);
    splice(alice, 1, 2, "🐟");
    splice(bob, 4, 0, "import { incomplete\n");
    sync(alice, bob);
    assert.equal(alice.capture().files["main.ts"], "a🐟bimport { incomplete\nc");
    assert.deepEqual(alice.resolveCursor(cursor), { path: "main.ts", utf16: 3 });
    assert.equal(capture.files["main.ts"], "a😀bc");
    assert(Object.isFrozen(capture.files));
    assert.throws(() => alice.edit([], capture.revision), /another revision/);
    const restarted = await createSharedText({ actor: actor("carol"), checkpoint: alice.save() });
    try { assert.deepEqual(restarted.capture(), alice.capture()); } finally { restarted.dispose(); }
  } finally { alice.dispose(); bob.dispose(); }
});

test("actual WASM preserves file identity and checked per-writer Undo", async () => {
  const [alice, bob] = await seeded("width=12; height=8");
  try {
    const id = alice.fileId("main.ts");
    const cursor = alice.cursor("main.ts", 0);
    splice(alice, 6, 2, "16");
    splice(bob, 17, 1, "9");
    sync(alice, bob);
    assert.equal(alice.undo().files["main.ts"], "width=12; height=9");
    assert.equal(alice.redo().files["main.ts"], "width=16; height=9");
    alice.edit([{ kind: "rename_file", path: "main.ts", new_path: "src/design.ts" }]);
    sync(alice, bob);
    assert.equal(alice.fileId("src/design.ts"), id);
    assert.deepEqual(bob.resolveCursor(cursor), { path: "src/design.ts", utf16: 0 });
    bob.edit([{ kind: "splice", path: "src/design.ts", start_utf16: 6, delete_utf16: 2, insert: "20" }]);
    sync(alice, bob);
    const before = alice.capture();
    assert.throws(() => alice.undo());
    assert.deepEqual(alice.capture(), before);
  } finally { alice.dispose(); bob.dispose(); }
});

test("JS rejects surrogate loss, numeric truncation and stale edits before mutation", async () => {
  const [alice, bob] = await seeded("a😀b");
  try {
    const before = alice.capture();
    for (const bad of ["\ud800", "\udfff", "x\ud800z"]) {
      assert.throws(() => splice(alice, 0, 0, bad), /unpaired UTF-16/);
      assert.throws(() => alice.replaceRange({}, bad), /unpaired UTF-16/);
    }
    assert.throws(() => splice(alice, 2, 0, "x"), /Unicode scalar/);
    for (const bad of [NaN, Infinity, -1, 0.5, 2 ** 32]) assert.throws(() => alice.cursor("main.ts", bad));
    assert.deepEqual(alice.capture(), before);
    const message = bob.generateSyncMessage("alice");
    const trailing = Uint8Array.from([...message, 0]);
    assert.throws(() => alice.receiveSyncMessage("bob", trailing));
    assert.deepEqual(alice.capture(), before);
    alice.dispose();
    assert.throws(() => alice.capture(), /disposed/);
  } finally { alice.dispose(); bob.dispose(); }
});

test("server ingress authenticates native incremental contributions", async () => {
  const [alice, bob] = await seeded("abc");
  try {
    const basis = alice.capture().revision;
    splice(bob, 1, 0, "B");
    const changes = bob.changesSince(basis);
    const before = alice.capture();
    assert.throws(() => alice.applyChangesFrom(changes, actor("forged")), /unauthenticated writer/);
    assert.deepEqual(alice.capture(), before);
    alice.applyChangesFrom(changes, actor("bob"));
    assert.deepEqual(alice.capture(), bob.capture());
    alice.applyChangesFrom(changes, actor("relay"));
    assert.deepEqual(alice.capture(), bob.capture());
  } finally { alice.dispose(); bob.dispose(); }
});

test("native and WASM checkpoints agree on exact heads, files, Unicode and identities", async () => {
  const executable = process.env.GEOSOLVE_COLLABORATION_NATIVE_FIXTURE ?? fileURLToPath(new URL("../../../target/debug/examples/text_fixture", import.meta.url));
  const fixture = JSON.parse(execFileSync(executable, [], { encoding: "utf8" }));
  const wasm = await createSharedText({ actor: actor("wasm-writer"), checkpoint: Uint8Array.from(fixture.checkpoint) });
  try {
    assert.deepEqual(wasm.capture(), fixture.snapshot);
    assert.deepEqual(wasm.resolveCursor(fixture.cursor), fixture.location);
    assert.equal(wasm.fileId("src/design.ts"), fixture.file_id);
    wasm.edit([{ kind: "splice", path: "src/design.ts", start_utf16: 0, delete_utf16: 0, insert: "// 😀\n" }]);
    const native = JSON.parse(execFileSync(executable, ["load"], { input: JSON.stringify(Array.from(wasm.save())), encoding: "utf8" }));
    assert.deepEqual(native.snapshot, wasm.capture());
    assert.equal(native.file_id, fixture.file_id);
  } finally { wasm.dispose(); }
});

test("binary sync authenticates session writer and typing rejects server-deleted files", async () => {
  const [server, client] = await seeded("abc");
  try {
    splice(client, 1, 0, "B");
    for (let i = 0; i < 20; i++) {
      const sm = server.generateSyncMessage("bob"), cm = client.generateSyncMessage("alice");
      if (!sm && !cm) break;
      if (sm) client.receiveSyncMessage("alice", sm);
      if (cm) server.receiveSyncMessageFrom("bob", cm, actor("bob"));
    }
    assert.deepEqual(server.capture(), client.capture());
    const basis = server.capture().revision;
    server.edit([{ kind: "remove_file", path: "main.ts" }]);
    splice(client, 0, 0, "pending");
    const before = server.capture();
    assert.throws(() => server.applyChangesFrom(client.changesSince(basis), actor("bob")), /absent\/replaced file/);
    assert.deepEqual(server.capture(), before);
  } finally { server.dispose(); client.dispose(); }
});

test("large paste remains valid raw text when bounded local Undo cannot retain it", async () => {
  const [alice, bob] = await seeded("x");
  try {
    const pasted = "😀".repeat(17000);
    assert.equal(splice(alice, 1, 0, pasted).files["main.ts"], `x${pasted}`);
    assert.equal(alice.history.undo, 0);
  } finally { alice.dispose(); bob.dispose(); }
});
