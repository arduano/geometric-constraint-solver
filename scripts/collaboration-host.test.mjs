// SPDX-License-Identifier: GPL-3.0-or-later
import assert from "node:assert/strict";
import { test } from "node:test";
import { mkdtemp, rm, readFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { openDurableCollaborationHost } from "./collaboration-host.mjs";
const configuration = { documentId: "drawing", documentEpoch: "drawing-life", initialInput: "initial-independent-input" };
const bytes = (value) => Buffer.from(JSON.stringify(value));
// Transport owner fixture: domain-independent strings deliberately do not claim
// geometry qualification. Actual engine/workbench tests own mathematical proof.
function checkpoints(input, draft = "const width = 12;") {
  return { model: bytes({ independentlyRebuildableInput: input }), source: bytes({ input, draft }), targets: bytes({ highWater: 1 }), history: bytes({ contributions: [] }) };
}
async function rebuild({ acceptedInput, checkpoints: state }) {
  const model = JSON.parse(state.model), source = JSON.parse(state.source);
  assert.equal(source.input, acceptedInput);
  return model.independentlyRebuildableInput;
}
async function fixture(t, options = {}) {
  const folder = await mkdtemp(join(tmpdir(), "geosolve-collab-host-"));
  const hosts = [];
  t.after(async () => { for (const host of hosts.reverse()) await host.close(); await rm(folder, { recursive: true, force: true }); });
  async function open(extra = {}) {
    const host = await openDurableCollaborationHost(folder, { rebuild, initial: { configuration, checkpoints: checkpoints(configuration.initialInput) }, ...options, ...extra });
    hosts.push(host); return host;
  }
  const host = await open(); return { folder, host, open };
}
function deferred() { let resolve; const promise = new Promise((r) => { resolve = r; }); return { promise, resolve }; }
function submit(connection, requestId, basisRevision = 0) { return { connection, requestId, command: { kind: "semantic", basisRevision, payload: { property: "width", value: 14 } } }; }
function accepted(input, install = () => {}) { return () => ({ completion: { status: "accepted", acceptedInput: input, summary: "Set width" }, checkpoints: checkpoints(input), install }); }

test("actual Rust admission and terminal ACK follow real fsync, restart retains exact original outcome", async (t) => {
  const hold = deferred(), reached = deferred(); let enabled = false;
  const { host, open } = await fixture(t, { storageOptions: { fault: async (point) => { if (enabled && point === "journal-synced") { reached.resolve(); await hold.promise; } } } });
  const alice = await host.connect({ userId: "alice", role: "editor" }, "alice-tab");
  const admitted = await host.admit(submit(alice, "one")); assert.equal(admitted.outcome, null);
  enabled = true; let installed = false, acknowledged = false;
  const pending = host.runNext(async () => accepted("validated-one", () => { installed = true; })).then((receipt) => { acknowledged = true; return receipt; });
  await reached.promise;
  assert.equal(host.snapshot().acceptedRevision, 0); assert.equal(installed, false); assert.equal(acknowledged, false);
  assert.equal(host.receipt(alice, "one").outcome, null);
  hold.resolve(); const terminal = await pending; assert.equal(installed, true); assert.equal(terminal.outcome.revision, 1);
  await host.close();
  const reopened = await open({ serverEpoch: "restart", storageOptions: {} });
  assert.equal(reopened.snapshot().acceptedInput, "validated-one");
  const reconnect = await reopened.connect({ userId: "alice", role: "editor" }, "alice-tab");
  const duplicate = await reopened.admit(submit(reconnect, "one"), () => { throw Error("duplicate must not recapture Apply"); });
  assert.deepEqual(duplicate, terminal); assert.equal(reopened.snapshot().latestSequence, 2);
  assert.throws(() => reopened.receipt(alice, "one"), /epoch|connection|session/u);
});

test("held domain solve leaves text persistence, new admissions and committed reads available", async (t) => {
  const { host } = await fixture(t);
  const alice = await host.connect({ userId: "alice", role: "editor" }, "alice-tab");
  const bob = await host.connect({ userId: "bob", role: "editor" }, "bob-tab");
  await host.admit(submit(alice, "one"));
  const hold = deferred(), entered = deferred();
  const pending = host.runNext(async (prepared) => { assert.equal(prepared.acceptedInput, configuration.initialInput); entered.resolve(); await hold.promise; return accepted("validated-first"); });
  await entered.promise;
  let textInstalled = false;
  const acknowledgement = await host.writeText(bob, () => ({ checkpoint: checkpoints(configuration.initialInput, "const unfinished =").source,
    commit: () => { textInstalled = true; return { heads: ["durable-text"] }; }, fail: () => { throw Error("unexpected text failure"); } }));
  assert.equal(textInstalled, true); assert.deepEqual(acknowledgement, { heads: ["durable-text"] });
  const second = await host.admit(submit(bob, "two")); assert.equal(second.admission, 2);
  assert.equal(host.snapshot().workerBusy, true); assert.equal(host.snapshot().acceptedRevision, 0);
  hold.resolve(); await pending;
  await host.runNext(async (prepared) => { assert.equal(prepared.acceptedInput, "validated-first"); assert.equal(prepared.acceptedRevision, 1); return accepted("validated-second"); });
  assert.equal(host.snapshot().acceptedRevision, 2);
});

test("lost terminal ACK poisons authoring and recovery reconstructs the durably committed model", async (t) => {
  let enabled = false;
  const { host, open } = await fixture(t, { storageOptions: { fault: async (point) => { if (enabled && point === "journal-synced") throw Error("ACK lost after sync"); } } });
  const alice = await host.connect({ userId: "alice", role: "editor" }, "alice-tab");
  await host.admit(submit(alice, "one")); enabled = true;
  await assert.rejects(host.runNext(async () => accepted("validated-before-crash")), /ACK lost/u);
  assert.equal(host.snapshot().needsRecovery, true);
  await assert.rejects(host.admit(submit(alice, "two")), /reopen\/recovery/u);
  await host.close();
  const restored = await open({ storageOptions: {}, serverEpoch: "second-process" });
  const reconnect = await restored.connect({ userId: "alice", role: "editor" }, "alice-tab");
  assert.equal(restored.receipt(reconnect, "one").outcome.accepted_input, "validated-before-crash");
  assert.equal(restored.snapshot().pendingCount, 0);
});

test("restart preserves raw invalid working text independently of accepted model checkpoint", async (t) => {
  const { host, open } = await fixture(t);
  const alice = await host.connect({ userId: "alice", role: "editor" }, "alice-tab");
  await host.writeText(alice, () => ({ checkpoint: checkpoints(configuration.initialInput, "const 🚰 = (").source, commit: () => true, fail: () => {} }));
  await host.close(); const restored = await open({ serverEpoch: "second-process" });
  assert.equal(JSON.parse(restored.restoredCheckpoints().source).draft, "const 🚰 = (");
  assert.equal(JSON.parse(restored.restoredCheckpoints().model).independentlyRebuildableInput, configuration.initialInput);
});

test("viewer/forged text ingress, mismatched rebuild and missing publication checkpoints fail closed", async (t) => {
  const { host, open } = await fixture(t);
  const viewer = await host.connect({ userId: "viewer", role: "viewer" }, "viewer-tab");
  let staged = false;
  await assert.rejects(host.writeText(viewer, () => { staged = true; }), /cannot edit/u);
  await assert.rejects(host.writeText({ ...viewer, role: "editor" }, () => { staged = true; }), /connection|session/u);
  assert.equal(staged, false); assert.equal(host.snapshot().needsRecovery, false);
  const alice = await host.connect({ userId: "alice", role: "editor" }, "alice-tab"); await host.admit(submit(alice, "one"));
  await assert.rejects(host.runNext(async () => () => ({ completion: { status: "accepted", acceptedInput: "forged", summary: "missing proof bytes" }, checkpoints: {} })), /exact model\/source/u);
  assert.equal(host.snapshot().acceptedRevision, 0);
  await host.close();
  await assert.rejects(open({ rebuild: async () => "different-input", serverEpoch: "different" }), /Reconstructed geometry/u);
});

test("immutable Apply attachment survives queued restart; external ingress is bounded while storage is held", async (t) => {
  const hold = deferred(), entered = deferred(); let enabled = false;
  const { host, open } = await fixture(t, { maxIngress: 1, storageOptions: { fault: async (point) => { if (enabled && point === "journal-synced") { entered.resolve(); await hold.promise; } } } });
  const alice = await host.connect({ userId: "alice", role: "editor" }, "alice-tab");
  enabled = true;
  const request = submit(alice, "apply"); request.command.kind = "apply";
  const pending = host.admit(request, () => ({ applyCapture: bytes({ captured: "width14", heads: ["old-head"] }) }));
  await entered.promise;
  await assert.rejects(host.admit(submit(alice, "later")), /ingress is full/u);
  hold.resolve(); await pending; await host.close();
  const restored = await open({ storageOptions: {}, serverEpoch: "restored" });
  await restored.runNext(async (prepared, attachments) => {
    assert.equal(prepared.command.kind, "apply"); assert.deepEqual(JSON.parse(attachments.applyCapture), { captured: "width14", heads: ["old-head"] });
    return () => ({ completion: { status: "rejected", code: "invalid_source", message: "Draft remains available" } });
  });
  assert.equal(restored.snapshot().acceptedRevision, 0); assert.equal(restored.snapshot().pendingCount, 0);
});

test("durable text IDs recover the original ACK and reject changed retries or model namespace reuse", async (t) => {
  const { host, open } = await fixture(t);
  const alice = await host.connect({ userId: "alice", role: "editor" }, "alice-tab");
  const request = { requestId: "raw-one", payload: { delta: [1, 2], heads: ["old"] } };
  const ack = { sourceSequence: 1, workingRevision: { heads: ["accepted-text"] } }; let applications = 0;
  await host.writeText(alice, () => ({ checkpoint: checkpoints(configuration.initialInput, "const incomplete =").source, ack,
    commit: () => { applications++; }, fail: () => {} }), request);
  const sequence = host.snapshot().latestEnvelope;
  assert.deepEqual(await host.writeText(alice, () => { throw Error("must dedup before native text staging"); }, { requestId: "raw-one", payload: { heads: ["old"], delta: [1, 2] } }), ack);
  assert.equal(applications, 1); assert.equal(host.snapshot().latestEnvelope, sequence);
  await assert.rejects(host.writeText(alice, () => {}, { ...request, payload: { delta: [3] } }), /different content/u);
  await assert.rejects(host.admit(submit(alice, "raw-one")), /belongs to a text change/u);
  assert.equal(host.snapshot().needsRecovery, false);
  await host.close(); const restored = await open({ serverEpoch: "text-restart" });
  const reconnect = await restored.connect({ userId: "alice", role: "editor" }, "alice-tab");
  assert.deepEqual(await restored.writeText(reconnect, () => { throw Error("must restore original text receipt"); }, request), ack);
  assert.equal(restored.snapshot().latestEnvelope, sequence);
  await restored.admit(submit(reconnect, "model-one"));
  await assert.rejects(restored.writeText(reconnect, () => {}, { requestId: "model-one", payload: {} }), /belongs to a model operation/u);
});

test("lost text ACK restores both invalid source and original dedup receipt after synced journal recovery", async (t) => {
  let enabled = false;
  const { host, open } = await fixture(t, { storageOptions: { fault: async (point) => { if (enabled && point === "journal-synced") throw Error("lost text ACK"); } } });
  const alice = await host.connect({ userId: "alice", role: "editor" }, "alice-tab"); enabled = true;
  const request = { requestId: "text-lost", payload: { nativeChange: [8, 9] } };
  let failed = false;
  const ack = { workingRevision: { heads: ["durable-invalid"] } };
  await assert.rejects(host.writeText(alice, () => ({ checkpoint: checkpoints(configuration.initialInput, "const incomplete =").source,
    ack, commit: () => { throw Error("install before sync ACK must not happen"); }, fail: () => { failed = true; } }), request), /lost text ACK/u);
  assert.equal(failed, true); assert.equal(host.snapshot().needsRecovery, true); await host.close();
  const restored = await open({ storageOptions: {}, serverEpoch: "text-recovered" });
  const connection = await restored.connect({ userId: "alice", role: "editor" }, "alice-tab");
  assert.deepEqual(await restored.writeText(connection, () => { throw Error("duplicate must not replay source splice"); }, request), ack);
  assert.equal(JSON.parse(restored.restoredCheckpoints().source).draft, "const incomplete =");
});
