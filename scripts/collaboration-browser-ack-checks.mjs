// SPDX-License-Identifier: GPL-3.0-or-later
import assert from "node:assert/strict";
import { createServer } from "node:http";
import { readFile } from "node:fs/promises";
import { chromium } from "../crates/geosolve-demo-web/frontend/node_modules/playwright-core/index.mjs";
import { trackCommittedTextOutbox, waitForCommittedTextAck } from "./collaboration-browser-ack.mjs";

export async function checkTextAckWitness(t) {
  const client = await readFile("packages/geosolve-collaboration/dist/client.js");
  let held;
  const send = (response, value) => {
    const body = Buffer.from(JSON.stringify(value));
    response.writeHead(200, { "Content-Type": "application/json", "Content-Length": body.length }); response.end(body);
  };
  const server = createServer(async (request, response) => {
    const path = new URL(request.url, "http://test").pathname;
    const chunks = []; for await (const chunk of request) chunks.push(chunk);
    if (path === "/client.js") { response.writeHead(200, { "Content-Type": "text/javascript" }); response.end(client); return; }
    if (path.endsWith("/join")) { send(response, { token: "a".repeat(64), connection: {
      protocol: 1, documentId: "ack-document", documentEpoch: "epoch", serverEpoch: "server", sessionId: "session",
      clientId: "ack-editor", userId: "alice", role: "editor",
    } }); return; }
    if (path.endsWith("/state")) { send(response, { authority: { acceptedRevision: 0, latestSequence: 0 }, document: {}, participants: [], presence: [] }); return; }
    if (path.endsWith("/events")) { response.writeHead(200, { "Content-Type": "text/event-stream" }); response.flushHeaders(); return; }
    if (path.endsWith("/heartbeat")) { send(response, { ok: true }); return; }
    if (path.endsWith("/text")) {
      const { requestId } = JSON.parse(Buffer.concat(chunks));
      if (requestId === "held") { held = response; response.writeHead(200, { "Content-Type": "application/json" }); response.flushHeaders(); return; }
      if (requestId === "truncated") {
        response.writeHead(200, { "Content-Type": "application/json", "Content-Length": 100 }); response.write('{"sourceSequence":');
        setTimeout(() => response.destroy(), 20); return;
      }
      if (requestId === "malformed") { response.writeHead(200, { "Content-Type": "application/json" }); response.end("not JSON"); return; }
      send(response, { sourceSequence: 1, workingRevision: { heads: ["a".repeat(64)] } }); return;
    }
    response.writeHead(200, { "Content-Type": "text/html" }); response.end("<!doctype html><title>Text ACK witness</title>");
  });
  await new Promise(resolve => server.listen(0, "127.0.0.1", resolve));
  const browser = await chromium.launch({ headless: true, executablePath: process.env.GEOSOLVE_CHROMIUM_PATH ?? "/home/arduano/.nix-profile/bin/google-chrome", args: ["--disable-dev-shm-usage"] });
  t.after(async () => { await browser.close(); server.closeAllConnections(); await new Promise(resolve => server.close(resolve)); });
  const origin = `http://127.0.0.1:${server.address().port}`;
  const controls = ["complete", "held", "truncated", "malformed", "storage-abort"];
  for (const control of controls) {
    const context = await browser.newContext(); await trackCommittedTextOutbox(context);
    const page = await context.newPage(); await page.goto(origin);
    await page.evaluate(async control => {
      const { CollaborationClient } = await import("/client.js");
      const scope = `geosolve.collaboration.${location.origin}/api/collaboration/`, clientId = "ack-editor", key = `${scope}.${clientId}`;
      sessionStorage.setItem(`${scope}.client`, clientId);
      const database = await new Promise((resolve, reject) => {
        const request = indexedDB.open("geosolve.collaboration.v1", 2);
        request.onupgradeneeded = () => request.result.createObjectStore("outboxes");
        request.onsuccess = () => resolve(request.result); request.onerror = () => reject(request.error);
      });
      window.ackClient = new CollaborationClient({ baseUrl: `${location.origin}/api/collaboration/`, inviteToken: "invite", clientId,
        savePending: pending => new Promise((resolve, reject) => {
          const transaction = database.transaction("outboxes", "readwrite"), store = transaction.objectStore("outboxes");
          // An attempted removal later overwritten in the same transaction is
          // not an ACK. The observer must retain only the final committed value.
          if (pending.requests.length) store.put({ ...pending, requests: [] }, key);
          store.put(pending, key);
          transaction.oncomplete = resolve; transaction.onabort = () => reject(Error("forced storage abort"));
          if (control === "storage-abort" && window.ackStarted && !pending.requests.length) transaction.abort();
        }),
      });
      await window.ackClient.connect();
      window.startAck = () => {
        window.ackStarted = true;
        void window.ackClient.writeText([Uint8Array.of(7)], control).then(value => { window.ackResult = { ok: true, value }; }, error => { window.ackResult = { ok: false, error: String(error) }; });
      };
    }, control);
    const headers = page.waitForResponse(response => response.url().endsWith("/text") && response.request().method() === "POST");
    await page.evaluate(() => window.startAck()); const response = await headers;
    assert.equal(response.status(), 200);
    if (control === "complete") {
      const witness = await waitForCommittedTextAck(page, response);
      assert.equal(witness.requestId, control); assert.equal(witness.clientId, "ack-editor"); assert.equal(witness.documentId, "ack-document");
      assert.equal(await page.evaluate(() => window.ackResult?.ok), true);
    } else {
      await assert.rejects(waitForCommittedTextAck(page, response, 250), /Timeout/u, control);
      const snapshots = await page.evaluate(() => window.geosolveCommittedOutboxes);
      assert.ok(snapshots.at(-1).requests.some(request => request.requestId === control), `${control} must retain its committed outbox`);
      if (control === "held") {
        assert.equal(await page.evaluate(() => window.ackResult), undefined);
        held.end(JSON.stringify({ sourceSequence: 1, workingRevision: { heads: [] } }));
        assert.equal((await waitForCommittedTextAck(page, response)).requestId, control);
      } else {
        await page.waitForFunction(() => window.ackResult?.ok === false);
      }
    }
    await context.close();
  }
  t.diagnostic(JSON.stringify({ controls, completedBodyAndCommittedRemovalRequired: true }));
}
