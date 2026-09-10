// SPDX-License-Identifier: GPL-3.0-or-later
import assert from "node:assert/strict";
import { test } from "node:test";
import { mkdtemp, writeFile, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { setTimeout as delay } from "node:timers/promises";
import { chromium } from "../crates/geosolve-demo-web/frontend/node_modules/playwright-core/index.mjs";
import { inventory, mediaType, hash } from "../crates/geosolve-demo-web/frontend/scripts/release-artifact-lib.mjs";
import { openCollaborationRuntime } from "./collaboration-runtime.mjs";

const source = `"use geosolve sketch";
import {sketch,mm} from "@geosolve/sketch-code";
// Browser recovery 😀
export default sketch(($)=>{
 const bore=$.geometry.centerRadiusCircle("bore",{center:[0,0],radius:mm(2)});
 const other=$.geometry.centerRadiusCircle("other",{center:[20,0],radius:mm(3)});
 return {bore,other};
});
`;
const deferred = () => {
  let resolve;
  const promise = new Promise(done => { resolve = done; });
  return { promise, resolve };
};
async function until(predicate, message, timeout = 30_000) {
  const deadline = Date.now() + timeout;
  while (Date.now() < deadline) {
    if (await predicate()) return;
    await delay(25);
  }
  throw Error(message);
}

async function fixture(t, domainOptions) {
  const folder = await mkdtemp(join(tmpdir(), "geosolve-browser-recovery-"));
  const handles = [], cleanups = [], errors = [], observationErrors = [], joins = [];
  let browser, runtime, port = 0;
  t.after(async () => {
    for (const cleanup of cleanups) await cleanup();
    await browser?.close();
    for (const handle of handles.reverse()) await handle.close();
    await rm(folder, { recursive: true, force: true });
  });
  await writeFile(join(folder, "geosolve.json"), JSON.stringify({ format: "geosolve-folder-v2", entry: "sketch.ts", mode: "editable" }));
  await writeFile(join(folder, "sketch.ts"), source);
  const dist = resolve(process.env.GEOSOLVE_DIST ?? "crates/geosolve-demo-web/dist");
  const routes = new Map();
  for (const file of await inventory(dist)) {
    const body = await readFile(join(dist, file.path));
    assert.equal(hash(body), file.sha256);
    routes.set(`/${file.path}`, { body, type: mediaType(file.path) });
  }
  routes.set("/", routes.get("/index.html"));
  const invitations = new Map(["alice", "bob"].map(userId => [userId, { userId, role: "editor" }]));
  async function open(initialize) {
    runtime = await openCollaborationRuntime(folder, { initialize, invitations, workbenchScenes: true, staticRoutes: routes, domainOptions });
    handles.push(runtime);
    port = (await runtime.listen(port)).port;
  }
  await open(true);
  const origin = `http://127.0.0.1:${port}`, base = `${origin}/api/collaboration/`;
  browser = await chromium.launch({ headless: true,
    executablePath: process.env.GEOSOLVE_CHROMIUM_PATH ?? "/home/arduano/.nix-profile/bin/google-chrome",
    args: ["--disable-dev-shm-usage", "--enable-webgl", "--use-gl=angle", "--use-angle=swiftshader", "--enable-unsafe-swiftshader"] });
  return {
    get runtime() { return runtime; }, origin, base, cleanups, errors, observationErrors, joins,
    async restart() { await runtime.close(); await open(false); },
    async page(user) {
      const context = await browser.newContext({ viewport: { width: 1400, height: 950 } });
      const page = await context.newPage();
      await page.route("**/api/collaboration/join", async route => {
        try {
          // Observe actual HTTP bytes before delivering the unchanged response.
          // CDP can retire an aborted startup response before a later body read.
          const response = await route.fetch();
          if (response.status() === 200) joins.push({ page, ...await response.json() });
          await route.fulfill({ response });
        } catch (error) { observationErrors.push(String(error)); await route.abort().catch(() => {}); }
      });
      page.on("pageerror", error => errors.push(String(error)));
      page.on("dialog", dialog => { void dialog.accept(); });
      await page.goto(`${origin}/?collaboration=1#invite=${user}`);
      await ready(page);
      await until(() => joins.some(join => join.page === page), "Browser did not consume an authenticated join");
      return page;
    },
    async request(path, token) {
      const response = await fetch(base + path, { headers: { Authorization: `Bearer ${token}` }, signal: AbortSignal.timeout(15_000) });
      const result = await response.json();
      assert.equal(response.status, 200, JSON.stringify(result));
      return result;
    },
  };
}
async function ready(page) {
  await page.getByRole("region", { name: "Shared document", exact: true }).waitFor({ timeout: 90_000 });
  await page.locator('canvas[data-render-state="ready"]').waitFor({ timeout: 90_000 });
}
async function select(page, name) {
  const row = page.locator("[data-navigation-row]").filter({ hasText: name }).first();
  await row.click();
  await until(async () => await row.getAttribute("aria-pressed") === "true", `Explorer ${name} did not select`);
}
async function editor(page) {
  await page.getByRole("group", { name: "Workspace layout", exact: true }).getByRole("button", { name: "split", exact: true }).click();
  const content = page.locator(".cm-content[contenteditable=true]");
  await content.waitFor();
  return content;
}
async function replaceText(page, content, text) {
  await content.click();
  await page.keyboard.press("Control+Home");
  await page.keyboard.press("Control+a");
  await page.keyboard.insertText(text);
}
function paintedGeometry(page) {
  return page.locator("canvas").evaluate(canvas => {
    const frame = canvas.__geosolvePresentedFrame;
    if (!frame) throw Error("Expected a native accepted canvas frame");
    return frame.items.filter(item => item.interactive && item.layer === "geometry")
      .map(({ kind, id, center, points, radius, radii, rotation }) => ({ kind, id, center, points, radius, radii, rotation }));
  });
}
function outbox(page) {
  // Read the actual persisted browser record; never seed or modify recovery data.
  return page.evaluate(async () => {
    const clientId = Object.entries(sessionStorage).find(([key]) => key.endsWith(".client"))?.[1];
    return new Promise((resolve, reject) => {
      const opening = indexedDB.open("geosolve.collaboration.v1");
      opening.onerror = () => reject(opening.error);
      opening.onsuccess = () => {
        const database = opening.result, transaction = database.transaction("outboxes", "readonly");
        const request = transaction.objectStore("outboxes").getAll();
        let value;
        request.onsuccess = () => { value = request.result.find(item => item.clientId === clientId); };
        transaction.oncomplete = () => { database.close(); resolve(value); };
        transaction.onerror = transaction.onabort = () => { database.close(); reject(transaction.error); };
      };
    });
  });
}

test("browser retries its exact persisted lost-ACK command after server restart and page reload", { timeout: 180_000 }, async t => {
  const release = deferred();
  let held = false;
  const f = await fixture(t, { beforeJob: async input => {
    if (input.kind === "values" && !held) { held = true; await release.promise; }
  } });
  f.cleanups.push(() => release.resolve());
  const page = await f.page("alice"), context = page.context();
  const commands = [], resumedReceipts = [];
  let blocked = false, dropped = false, faultError;
  await page.route("**/api/collaboration/**", async route => {
    try {
      if (blocked) { await route.abort("internetdisconnected"); return; }
      if (!new URL(route.request().url()).pathname.endsWith("/commands")) { await route.fallback(); return; }
      const body = route.request().postDataJSON(); commands.push(body);
      const response = await route.fetch(), result = await response.json();
      assert.equal(response.status(), 200, JSON.stringify(result));
      if (!dropped) {
        // The admission ACK already crossed the real fsync boundary. Keep the
        // solve held while cutting both HTTP and SSE delivery to this browser.
        assert.equal(result.receipt.outcome, null);
        blocked = true;
        await context.setOffline(true);
        await route.abort("internetdisconnected");
        dropped = true;
      } else {
        resumedReceipts.push(result.receipt);
        await route.fulfill({ response });
      }
    } catch (error) { faultError = error; await route.abort().catch(() => {}); }
  });
  await select(page, "bore");
  const radius = page.getByRole("textbox", { name: "bore · radius value", exact: true });
  await radius.fill("5"); await radius.press("Enter");
  await until(() => faultError || (dropped && held), "The admitted command did not lose its browser ACK");
  if (faultError) throw faultError;
  assert.equal(commands.length, 1);
  const saved = await outbox(page);
  assert.equal(saved.requests.length, 1);
  assert.deepEqual(saved.requests[0], { route: "commands", requestId: commands[0].requestId, body: commands[0] });
  await page.getByRole("button", { name: "Download pending work", exact: true }).waitFor();
  assert.equal(f.runtime.host.snapshot().acceptedRevision, 0);
  release.resolve();
  await until(() => f.runtime.host.snapshot().acceptedRevision === 1, "Server did not durably complete the offline browser's command");
  await until(() => f.joins.some(item => item.page === page), "Missing original authenticated browser join");
  const originalJoin = f.joins.find(item => item.page === page);
  const originalReceipt = (await f.request(`receipt?requestId=${commands[0].requestId}`, originalJoin.token)).receipt;
  assert.equal(originalReceipt.outcome.status, "accepted");
  const accepted = await f.request("state", originalJoin.token);
  assert.match(accepted.document.accepted.files["sketch.ts"], /radius:\s*mm\(5\)/u);
  assert.deepEqual(await outbox(page), saved, "An unseen terminal must not clear the persisted request");

  await f.restart();
  assert.equal(f.runtime.host.snapshot().acceptedInput, accepted.authority.acceptedInput);
  assert.equal(f.runtime.host.snapshot().acceptedRevision, 1);
  assert.deepEqual(f.runtime.source().snapshot().accepted, accepted.document.accepted);
  // Permit API requests only after the new page replaces the disconnected one,
  // so an old page's automatic reconnect cannot consume the saved request first.
  page.once("framenavigated", () => { blocked = false; });
  await context.setOffline(false);
  await page.reload(); await ready(page);
  await until(async () => (await outbox(page))?.requests.length === 0, "Reload did not resolve the persisted operation");
  if (faultError) throw faultError;
  assert.ok(commands.length >= 2, "The reloaded workbench must retry its saved command");
  for (const command of commands) assert.deepEqual(command, saved.requests[0].body);
  assert.ok(resumedReceipts.length > 0);
  for (const receipt of resumedReceipts) assert.deepEqual(receipt, originalReceipt);
  assert.equal((await outbox(page)).clientId, saved.clientId);
  assert.equal(f.runtime.host.snapshot().acceptedRevision, 1, "Deduplication must not publish a second edit");
  assert.equal(f.runtime.host.snapshot().acceptedInput, accepted.authority.acceptedInput);
  await select(page, "bore");
  await until(async () => await page.getByRole("textbox", { name: "bore · radius value", exact: true }).inputValue() === "5", "Recovered Inspector did not show the accepted radius");
  assert.equal(await page.getByRole("button", { name: "Download pending work", exact: true }).count(), 0);
  assert.deepEqual(f.errors, []);
  assert.deepEqual(f.observationErrors, []);
  t.diagnostic(JSON.stringify({ lostAdmissionAck: true, serverRestart: true, pageReload: true, attempts: commands.length, acceptedRevision: 1, recoveredRequestId: saved.requests[0].requestId }));
});

test("browser preserves invalid draft and accepted canvas, then Apply captures source before later typing", { timeout: 180_000 }, async t => {
  const release = deferred();
  let holdApply = false, held = false;
  const f = await fixture(t, { beforeJob: async input => {
    if (input.kind === "apply" && holdApply) { held = true; await release.promise; }
  } });
  f.cleanups.push(() => release.resolve());
  const alice = await f.page("alice"), bob = await f.page("bob");
  const aliceEditor = await editor(alice), bobEditor = await editor(bob);
  const events = [];
  f.cleanups.push(f.runtime.host.subscribe(envelope => { if (envelope.payload.kind === "authority") events.push(envelope.payload.record); }));
  const before = f.runtime.source().snapshot(), input = f.runtime.host.snapshot().acceptedInput;
  const initialPaint = await paintedGeometry(alice);
  assert.ok(initialPaint.length > 0);
  const invalid = source.replace("radius:mm(2)", "radius:mm(");
  await replaceText(alice, aliceEditor, invalid);
  await until(() => f.runtime.source().snapshot().working.files["sketch.ts"] === invalid, "Invalid source did not synchronize");
  await until(async () => (await bobEditor.innerText()).includes("radius:mm(}"), "Peer CodeMirror did not display the invalid draft");
  await alice.getByRole("button", { name: "Apply", exact: true }).click();
  await until(() => events.some(record => record.event.event === "finished" && record.event.outcome.status === "rejected"), "Invalid Apply did not produce a durable refusal");
  assert.equal(f.runtime.host.snapshot().acceptedInput, input);
  assert.deepEqual(f.runtime.source().snapshot().accepted, before.accepted);
  assert.equal(f.runtime.source().snapshot().working.files["sketch.ts"], invalid);
  assert.deepEqual(await paintedGeometry(alice), initialPaint, "Invalid Apply must retain the complete accepted drawing");

  await select(bob, "bore");
  const radius = bob.getByRole("textbox", { name: "bore · radius value", exact: true });
  await radius.fill("5"); await radius.press("Enter");
  await until(() => f.runtime.host.snapshot().acceptedRevision === 1, "Canvas value edit did not publish under invalid source");
  const canvasAccepted = f.runtime.source().snapshot();
  assert.match(canvasAccepted.accepted.files["sketch.ts"], /radius:\s*mm\(5\)/u);
  assert.equal(canvasAccepted.working.files["sketch.ts"], invalid);
  await until(async () => await radius.inputValue() === "5", "Canvas Inspector did not show its accepted value");

  const captured = source.replace("radius:mm(2)", "radius:mm(6)");
  await replaceText(alice, aliceEditor, captured);
  await until(() => f.runtime.source().snapshot().working.files["sketch.ts"] === captured, "Valid captured draft did not synchronize");
  holdApply = true;
  await alice.getByRole("button", { name: "Apply", exact: true }).click();
  await until(() => held, "Apply did not reach the held native compilation job");
  const later = captured.replace("radius:mm(6)", "radius:mm(8)");
  await replaceText(alice, aliceEditor, later);
  await until(() => f.runtime.source().snapshot().working.files["sketch.ts"] === later, "Typing did not synchronize while Apply was held");
  await until(async () => (await bobEditor.innerText()).includes("radius:mm(8)"), "Peer CodeMirror did not receive later typing before Apply release");
  assert.equal(f.runtime.host.snapshot().acceptedRevision, 1);
  assert.deepEqual(f.runtime.source().snapshot().accepted, canvasAccepted.accepted);
  release.resolve();
  await until(() => f.runtime.host.snapshot().acceptedRevision === 2, "Captured Apply did not publish");
  const final = f.runtime.source().snapshot();
  assert.equal(final.accepted.files["sketch.ts"], captured);
  assert.equal(final.working.files["sketch.ts"], later);
  await until(async () => await radius.inputValue() === "6", "Peer Inspector did not receive the captured accepted value");
  await until(async () => (await aliceEditor.innerText()).includes("radius:mm(8)"), "Apply replaced the author's later draft");
  assert.deepEqual(f.errors, []);
  assert.deepEqual(f.observationErrors, []);
  t.diagnostic(JSON.stringify({ invalidApplyRejected: true, canvasUnderInvalidSource: true, peerTextBeforeApplyRelease: true, acceptedRadius: 6, workingRadius: 8, acceptedRevision: 2 }));
});
