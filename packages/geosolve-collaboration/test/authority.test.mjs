// SPDX-License-Identifier: GPL-3.0-or-later
import test from "node:test";
import assert from "node:assert/strict";
import { createDocumentAuthorityHost } from "../dist/host.js";
import { createSharedText } from "../dist/index.js";

const configuration = (serverEpoch = "process-1") => ({ documentId: "doc", documentEpoch: "generation-1", serverEpoch, initialInput: "initial-model" });
const request = (connection, requestId = "one") => ({ connection, requestId, command: { kind: "semantic", basisRevision: 0, payload: { target: "width", value: -12.5 } } });
async function opened(options = {}) {
  const host = await createDocumentAuthorityHost({ configuration: configuration(), ...options });
  return [host, host.connect({ userId: "alice", role: "editor" }, "client-a", "session-a")];
}
function deferred() {
  let resolve, reject;
  const promise = new Promise((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
}

test("actual WASM admission waits for asynchronous durable ACK while draft replica remains usable", async () => {
  const [host, connection] = await opened();
  const text = await createSharedText({ actor: new TextEncoder().encode("text-worker") });
  try {
    const persistence = deferred();
    let returned = false, captured;
    const pending = host.admit(request(connection), async (stage) => {
      captured = stage;
      await persistence.promise;
    }).then((receipt) => { returned = true; return receipt; });
    assert.equal(captured.status, "staged");
    assert(!("receipt" in captured));
    assert(Object.isFrozen(captured));
    assert.equal(host.snapshot().latestSequence, 0);
    assert.equal(host.snapshot().hasPendingStage, true);
    assert.equal(host.receipt(connection, "one"), null);
    assert.deepEqual(host.checkpoint(), []);
    assert.deepEqual(host.resume(connection, 0).records, []);
    assert.throws(() => host.beginNext(), /pending/);
    assert.throws(() => host.disconnect(connection.sessionId), /pending/);
    assert.throws(() => host.connect({ userId: "bob", role: "editor" }, "b", "b"), /pending/);
    assert.throws(() => host.stageAdmission(request(connection, "two")), /pending/);
    assert.throws(() => host.commitStage({ ...captured }), /stale/);
    assert.throws(() => host.dispose(), /pending/);
    text.edit([{ kind: "create_file", path: "main.ts", text: "import { incomplete\n" }]);
    await new Promise((resolve) => setImmediate(resolve));
    assert.equal(returned, false);
    assert.equal(text.capture().files["main.ts"], "import { incomplete\n");
    persistence.resolve();
    const receipt = await pending;
    assert.equal(receipt.admission, 1);
    assert.equal(receipt.outcome, null);
    assert.equal(host.snapshot().latestSequence, 1);
    assert.equal(host.snapshot().hasPendingStage, false);
    assert.deepEqual(host.checkpoint(), [JSON.parse(captured.recordJson)]);
    const duplicate = await host.admit(request(connection), async () => { throw Error("duplicate appended again"); });
    assert.deepEqual(duplicate, receipt);
  } finally { host.dispose(); text.dispose(); }
});

test("actual WASM validated completion publishes only after recoverable snapshot persistence", async () => {
  const [host, connection] = await opened();
  try {
    const records = [];
    await host.admit(request(connection), async (stage) => records.push(JSON.parse(stage.recordJson)));
    const prepared = host.beginNext();
    assert.equal(prepared.acceptedInput, "initial-model");
    assert.equal(prepared.command.payload.value, -12.5);
    assert.throws(() => host.stageValidatedCompletion({ ...prepared }, { status: "accepted", acceptedInput: "fake", summary: "fake" }), /ticket/);
    const persistence = deferred();
    let snapshotDurable = false;
    const terminal = host.complete(prepared, { status: "accepted", acceptedInput: "model-1", summary: "independent validation passed" }, async (stage) => {
      await persistence.promise;
      // Reference host owns the actual recoverable source/design/model transaction.
      snapshotDurable = true;
      records.push(JSON.parse(stage.recordJson));
    });
    assert.equal(host.snapshot().acceptedRevision, 0);
    assert.equal(host.receipt(connection, "one").outcome, null);
    assert.equal(host.checkpoint().length, 1);
    persistence.resolve();
    const receipt = await terminal;
    assert(snapshotDurable);
    assert.equal(receipt.outcome.accepted_input, "model-1");
    assert.equal(host.snapshot().acceptedRevision, 1);
    assert.deepEqual(host.checkpoint(), records);
    assert.throws(() => host.stageValidatedCompletion(prepared, { status: "rejected", code: "old", message: "old" }), /ticket/);
    const restarted = await createDocumentAuthorityHost({ configuration: configuration("process-2"), records });
    try {
      assert.throws(() => restarted.receipt(connection, "one"), /stale/);
      const renewed = restarted.connect({ userId: "alice", role: "editor" }, "client-a", "new-session");
      const duplicate = await restarted.admit(request(renewed), async () => { throw Error("duplicate write"); });
      assert.deepEqual(duplicate, receipt);
      assert.equal(restarted.beginNext(), undefined);
    } finally { restarted.dispose(); }
  } finally { host.dispose(); }
});

test("uncertain async append poisons handle and recovery resolves lost ACK from actual journal", async () => {
  const [host, connection] = await opened();
  const records = [];
  try {
    await assert.rejects(host.admit(request(connection), async (stage) => {
      records.push(JSON.parse(stage.recordJson));
      throw Error("fsync outcome uncertain");
    }), /fsync outcome uncertain/);
    assert.equal(host.snapshot().needsRecovery, true);
    assert.equal(host.snapshot().latestSequence, 0);
    assert.deepEqual(host.checkpoint(), []);
    assert.throws(() => host.stageAdmission(request(connection)), /recovery/);
    assert.throws(() => host.beginNext(), /recovery/);
    const recovered = await createDocumentAuthorityHost({ configuration: configuration("process-2"), records });
    try {
      const renewed = recovered.connect({ userId: "alice", role: "editor" }, "client-a", "recovered");
      const duplicate = recovered.stageAdmission(request(renewed));
      assert.equal(duplicate.status, "duplicate");
      assert.equal(duplicate.receipt.admission, 1);
      assert.equal(recovered.beginNext().operation.requestId, "one");
    } finally { recovered.dispose(); }
  } finally { host.dispose(); }
});

test("host adapter rejects forged roles, malformed numbers/Unicode, stale intent and corruption", async () => {
  const [host, connection] = await opened({ configuration: { ...configuration(), limits: { maxPending: 1 } } });
  try {
    const viewer = host.connect({ userId: "viewer", role: "viewer" }, "v", "v");
    assert.throws(() => host.stageAdmission(request(viewer)), /cannot edit/);
    assert.throws(() => host.stageAdmission(request({ ...connection, userId: "forged" })), /authenticated/);
    assert.throws(() => host.connect({ userId: "bad\ud800", role: "editor" }, "b", "b"), /unpaired/);
    for (const bad of [-1, 0.5, NaN, Infinity, 2 ** 53]) assert.throws(() => host.resume(connection, bad));
    assert.throws(() => host.stageAdmission({ ...request(connection), command: { kind: "semantic", basisRevision: 0, payload: { value: NaN } } }), /Nonfinite/);
    await host.admit(request(connection), async () => {});
    assert.throws(() => host.stageAdmission(request(connection, "two")), /pending operations/);
    const changed = request(connection); changed.command.payload.value = 4;
    assert.throws(() => host.stageAdmission(changed), /different immutable intent/);
    const corrupt = structuredClone(host.checkpoint()); corrupt[0].digest = "invalid";
    await assert.rejects(createDocumentAuthorityHost({ configuration: configuration("new"), records: corrupt }), /corrupt/);
  } finally { host.dispose(); }
});

test("queued admission retains active worker ticket and terminal lost ACK recovers original outcome", async () => {
  const [host, connection] = await opened();
  const records = [];
  try {
    const persist = async (stage) => { records.push(JSON.parse(stage.recordJson)); };
    await host.admit(request(connection, "one"), persist);
    const prepared = host.beginNext();
    await host.admit(request(connection, "two"), persist);
    assert.equal(host.beginNext(), undefined);
    await assert.rejects(host.complete(prepared, { status: "accepted", acceptedInput: "model-1", summary: "validated" }, async (stage) => {
      records.push(JSON.parse(stage.recordJson));
      throw Error("terminal fsync response lost");
    }), /response lost/);
    assert.equal(host.snapshot().acceptedRevision, 0);
    assert.equal(host.snapshot().needsRecovery, true);
    assert.equal(host.checkpoint().length, 2);
    const recovered = await createDocumentAuthorityHost({ configuration: configuration("process-2"), records });
    try {
      const renewed = recovered.connect({ userId: "alice", role: "editor" }, "client-a", "new-session");
      const first = await recovered.admit(request(renewed, "one"), async () => { throw Error("reappended"); });
      assert.equal(first.outcome.accepted_input, "model-1");
      assert.equal(first.outcome.revision, 1);
      const second = recovered.beginNext();
      assert.equal(second.operation.requestId, "two");
      assert.equal(second.acceptedRevision, 1);
      assert.equal(second.acceptedInput, "model-1");
      assert.notEqual(second.ticket, prepared.ticket);
      assert.throws(() => recovered.stageValidatedCompletion(prepared, { status: "rejected", code: "old", message: "old process" }), /ticket/);
    } finally { recovered.dispose(); }
  } finally { host.dispose(); }
});
