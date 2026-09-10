// SPDX-License-Identifier: GPL-3.0-or-later
// Actual Rust source + authority + filesystem transaction boundary. Domain bytes
// are an explicit codec fixture here; domain-worker tests own geometry proof.
import assert from "node:assert/strict";
import { test } from "node:test";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { createHash } from "node:crypto";
import { createTrustedSourceHost } from "../packages/geosolve-collaboration/dist/host.js";
import { createSharedText } from "../packages/geosolve-collaboration/dist/index.js";
import { openDurableCollaborationHost } from "./collaboration-host.mjs";
const bytes = (value) => Buffer.from(JSON.stringify(value));
const actor = (value) => Buffer.from(value);
const files = { "main.ts": "const width = 12;\n", "helper.ts": "export const height = 8;\n" };
const configuration = { documentId: "source-integration", documentEpoch: "source-life", initialInput: "input-12" };
function patch(source, before, after) { const start = source.indexOf(before); return { baseSourceDigest: createHash("sha256").update(source).digest("hex"), candidateSourceDigest: createHash("sha256").update(source.replace(before, after)).digest("hex"), edits: [{ start, end: start + before.length, expected: before, replacement: after }] }; }
function deferred() { let resolve; const promise = new Promise((done) => { resolve = done; }); return { promise, resolve }; }
async function setup(t) {
  const folder = await mkdtemp(join(tmpdir(), "geosolve-collab-source-")), resources = [];
  const sourceConfig = (epoch) => ({ documentEpoch: configuration.documentEpoch, serverEpoch: epoch, initialInput: configuration.initialInput, files });
  let source = await createTrustedSourceHost({ configuration: sourceConfig("first"), actor: actor("server") }); resources.push(source);
  const host = await openDurableCollaborationHost(folder, {
    initial: { configuration, checkpoints: { source: Buffer.from(source.checkpoint()), model: bytes({ input: configuration.initialInput }), targets: bytes({ generation: 1 }), history: bytes({ records: [] }) } },
    rebuild: async ({ checkpoints }) => JSON.parse(checkpoints.model).input,
  }); resources.push(host);
  t.after(async () => { for (const resource of resources.reverse()) { if (resource.close) await resource.close(); else resource.dispose(); } await rm(folder, { force: true, recursive: true }); });
  const connection = await host.connect({ userId: "alice", role: "editor" }, "alice-tab");
  const client = await createSharedText({ actor: actor("alice"), checkpoint: source.textCheckpoint() }); resources.push(client);
  const send = async (requestId, edits) => {
    const basis = client.capture().revision; client.edit(edits);
    const changes = client.changesSince(basis);
    const request = { requestId, payload: { changes: changes.map((change) => Array.from(change)) } };
    return host.writeText(connection, () => {
      const stage = source.stageTextChanges(changes, actor("alice"));
      return { checkpoint: Buffer.from(stage.checkpointJson), ack: { sourceSequence: stage.sequence, workingRevision: stage.workingRevision }, commit: () => source.commitStage(stage), fail: () => source.failStage(stage) };
    }, request);
  };
  const reopen = async () => {
    await host.close();
    const restored = await openDurableCollaborationHost(folder, { serverEpoch: "restored", rebuild: async ({ checkpoints, acceptedInput, acceptedRevision }) => {
      source = await createTrustedSourceHost({ configuration: sourceConfig("restored"), actor: actor("new-server"), checkpointJson: checkpoints.source.toString() }); resources.push(source);
      assert.equal(source.snapshot().accepted.acceptedInput, acceptedInput); assert.equal(source.snapshot().accepted.modelRevision, acceptedRevision);
      return JSON.parse(checkpoints.model).input;
    } }); resources.push(restored); return { host: restored, source };
  };
  return { host, source, connection, client, send, reopen };
}

test("real source text ACK persists during held model work and invalid draft plus canvas checkpoint recover together", async (t) => {
  const f = await setup(t);
  await f.host.admit({ connection: f.connection, requestId: "canvas", command: { kind: "semantic", basisRevision: 0, payload: { width: 14 } } });
  const held = deferred(), entered = deferred();
  const job = f.host.runNext(async () => {
    const prepared = f.source.prepareCanvasUpdate([{ path: "main.ts", patch: patch(files["main.ts"], "12", "14") }]); entered.resolve(); await held.promise;
    return () => {
      const stage = f.source.stageValidatedPublication(prepared, "input-14", f.source.snapshot().working.revision, [{ kind: "pending", path: "main.ts", reason: "Unfinished expression owns this source range" }]);
      return { completion: { status: "accepted", acceptedInput: "input-14", summary: "Accepted canvas width" },
        checkpoints: { source: Buffer.from(stage.checkpointJson), model: bytes({ input: "input-14" }), targets: bytes({ generation: 1 }), history: bytes({ records: ["canvas"] }) },
        install: () => f.source.commitStage(stage), abort: () => f.source.failStage(stage) };
    };
  });
  await entered.promise;
  const ack = await f.send("unfinished", [{ kind: "splice", path: "main.ts", start_utf16: 0, delete_utf16: files["main.ts"].length, insert: "const width = (" }]);
  assert.deepEqual(ack.workingRevision, f.source.snapshot().working.revision); assert.equal(f.host.snapshot().acceptedRevision, 0);
  assert.equal(f.source.snapshot().working.files["main.ts"], "const width = (");
  held.resolve(); await job;
  assert.equal(f.source.snapshot().accepted.files["main.ts"], "const width = 14;\n");
  const restored = await f.reopen();
  assert.equal(restored.source.snapshot().working.files["main.ts"], "const width = (");
  assert.equal(restored.source.snapshot().accepted.files["main.ts"], "const width = 14;\n");
  assert.equal(restored.source.snapshot().pendingNotices.length, 1);
  assert.equal(restored.host.snapshot().acceptedInput, "input-14");
});

test("queued Apply durably restores exact native capture before later typing and publishes only captured source", async (t) => {
  const f = await setup(t);
  await f.send("type14", [{ kind: "splice", path: "main.ts", start_utf16: 14, delete_utf16: 2, insert: "14" }]);
  await f.host.admit({ connection: f.connection, requestId: "apply14", command: { kind: "apply", basisRevision: 0, payload: {} } }, () => {
    const capture = f.source.captureApply();
    const result = { applyCapture: Buffer.from(capture.captureJson), applyBasis: bytes(capture.acceptedBasis) }; f.source.release(capture); return result;
  });
  await f.send("type16", [{ kind: "splice", path: "main.ts", start_utf16: 14, delete_utf16: 2, insert: "16" }]);
  const restored = await f.reopen();
  await restored.host.runNext(async (_prepared, attachments) => {
    const capture = restored.source.restoreApplyCapture(attachments.applyCapture.toString(), JSON.parse(attachments.applyBasis));
    assert.equal(capture.working.files["main.ts"], "const width = 14;\n");
    const prepared = restored.source.prepareApplyUpdate(capture, capture.working.files);
    return () => {
      const stage = restored.source.stageValidatedPublication(prepared, "input-captured14", restored.source.snapshot().working.revision, [{ kind: "captured", path: "main.ts" }]);
      return { completion: { status: "accepted", acceptedInput: "input-captured14", summary: "Apply captured text" },
        checkpoints: { source: Buffer.from(stage.checkpointJson), model: bytes({ input: "input-captured14" }), targets: bytes({ generation: 1 }), history: bytes({ records: ["apply14"] }) },
        install: () => { restored.source.commitStage(stage); restored.source.release(capture); }, abort: () => restored.source.failStage(stage) };
    };
  });
  assert.equal(restored.source.snapshot().accepted.files["main.ts"], "const width = 14;\n");
  assert.equal(restored.source.snapshot().working.files["main.ts"], "const width = 16;\n");
  assert.equal(restored.host.snapshot().acceptedRevision, 1);
});
