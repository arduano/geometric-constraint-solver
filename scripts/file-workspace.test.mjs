// SPDX-License-Identifier: GPL-3.0-or-later
import assert from "node:assert/strict";
import test from "node:test";
import { mkdtempSync, readFileSync, writeFileSync, renameSync, mkdirSync, chmodSync } from "node:fs";
import { resolve } from "node:path";
import { chromium, expect } from "../crates/geosolve-demo-web/frontend/node_modules/@playwright/test/index.mjs";
import { initProject, serveProject, hash } from "./file-workspace.mjs";

const evidence = resolve("target/m98/tests");
mkdirSync(evidence, { recursive: true });
const pause = (ms) => new Promise((done) => setTimeout(done, ms));

test("M98 real folder, Rust workbench and existing UI vertical slice", { timeout: 180000 }, async (t) => {
  const folder = mkdtempSync(resolve(evidence, "project-"));
  const sourcePath = resolve(folder, "sketch.ts");
  const read = () => readFileSync(sourcePath, "utf8");
  const replace = (source) => { writeFileSync(`${sourcePath}.external`, source); renameSync(`${sourcePath}.external`, sourcePath); };
  initProject(folder);
  assert.throws(() => initProject(folder), /Refusing to overwrite/);
  let session = await serveProject(folder);
  const browser = await chromium.launch({ executablePath: process.env.GEOSOLVE_CHROMIUM_PATH ?? "/home/arduano/.nix-profile/bin/google-chrome", args: ["--disable-dev-shm-usage"] });
  const context = await browser.newContext({ viewport: { width: 1600, height: 1000 }, acceptDownloads: true });
  const page = await context.newPage();
  const browserErrors = [];
  page.on("pageerror", (error) => browserErrors.push(String(error)));
  const status = () => fetch(`${session.origin}/api/status`, { headers: { Authorization: `Bearer ${session.token}` } }).then((response) => response.json());
  const rpc = (method, input, baseHash) => fetch(`${session.origin}/api/rpc`, {
    method: "POST", headers: { Authorization: `Bearer ${session.token}`, "Content-Type": "application/json" },
    body: JSON.stringify({ method, input, baseHash }),
  });
  const dimension = () => page.getByRole("textbox", { name: "ringRadius value", exact: true });
  const canvas = () => page.locator('canvas[data-renderer="webgl2"]');
  const geometry = () => canvas().evaluate((element) => Reflect.get(element, "__geosolvePresentedFrame").items.filter((item) => item.layer === "geometry").map(({ id, kind, points, center, radius }) => ({ id, kind, points, center, radius })));
  try {
    await t.test("init and folder startup ignore browser design/draft storage", async () => {
      await context.addInitScript(() => {
        localStorage.setItem("geosolve.project.v1", "unrelated browser project");
        localStorage.setItem("geosolve.source-draft.v1", "unrelated browser draft");
      });
      await page.goto(session.url);
      await expect(page.getByRole("region", { name: "Local folder" })).toContainText("Saved to disk");
      await expect(page.getByRole("button", { name: "Download pending intent" })).toHaveCount(0);
      await expect(canvas()).toHaveAttribute("data-render-state", "ready");
      await expect(dimension()).toHaveValue("10");
      assert.equal((await status()).ok, true);
      assert.equal(await page.evaluate(() => localStorage.getItem("geosolve.project.v1")), "unrelated browser project");
      assert.equal(await page.evaluate(() => localStorage.getItem("geosolve.source-draft.v1")), "unrelated browser draft");
      await page.screenshot({ path: resolve(evidence, "01-initial.png") });
    });
    await t.test("atomic external rename updates real geometry without reload or duplicate solve", async () => {
      await page.evaluate(() => { window.m98PageIdentity = "still-open"; });
      const before = await geometry();
      const width = (items) => {
        const curve = items.find((item) => item.kind === "polyline" && item.points.length > 12);
        const xs = curve.points.map((point) => point[0]);
        return Math.max(...xs) - Math.min(...xs);
      };
      const initial = await status();
      const started = Date.now();
      replace(read().replace("value: mm(10)", "value: mm(12)"));
      await expect(dimension()).toHaveValue("12");
      const latencyMs = Date.now() - started;
      assert.notDeepEqual(await geometry(), before);
      assert.ok(Math.abs(width(await geometry()) / width(before) - 1.2) < 0.01, "external source edit preserves camera scale and changes the circle radius");
      assert.equal(await page.evaluate(() => window.m98PageIdentity), "still-open");
      await pause(500);
      const after = await status();
      assert.equal(after.externalApplies, initial.externalApplies + 1);
      assert.equal(after.writes, initial.writes);
      writeFileSync(resolve(evidence, "latency.json"), JSON.stringify({ smallSketchSaveToInspectorMs: latencyMs }, null, 2));
    });
    await t.test("existing Inspector edit atomically writes plaintext with no watch loop", async () => {
      const before = await status();
      await dimension().fill("14");
      await dimension().press("Enter");
      await expect.poll(read).toContain("value: mm(14)");
      await expect(page.getByRole("region", { name: "Local folder" })).toContainText("Saved to disk");
      await expect(page.getByRole("button", { name: "Download pending intent" })).toHaveCount(0);
      await pause(600);
      const after = await status();
      assert.equal(after.writes, before.writes + 1);
      assert.equal(after.externalApplies, before.externalApplies);
      assert.equal(after.currentHash, hash(read()));
      assert.equal(after.currentHash, after.acceptedHash);
      assert.match(read(), /Edit the driving radius below/);
      await page.getByRole("button", { name: "split", exact: true }).click();
      await expect(page.locator(".cm-content")).toContainText("value: mm(14)");
      await page.getByRole("button", { name: "Fit sketch" }).click();
      await page.screenshot({ path: resolve(evidence, "02-two-way-source.png") });
    });
    await t.test("invalid source retains exact disk text and last-good canvas with located error", async () => {
      const good = read();
      const before = await geometry();
      const invalid = good.replace("value: mm(14)", "value: mm(");
      replace(invalid);
      await expect(page.getByRole("region", { name: "Local folder" })).toContainText("Last accepted geometry retained");
      const failed = await status();
      assert.equal(failed.ok, false);
      assert.notEqual(failed.currentHash, failed.acceptedHash);
      assert.ok(failed.diagnostics.some((item) => item.file === "sketch.ts" && item.line >= 1));
      assert.equal(read(), invalid);
      assert.deepEqual(await geometry(), before);
      await page.screenshot({ path: resolve(evidence, "03-invalid-retained.png") });
      replace(good);
      await expect(page.getByRole("region", { name: "Local folder" })).toContainText("Saved to disk");
    });
    await t.test("in-progress Inspector edit retains input and refuses a changed disk base", async () => {
      const good = read();
      await dimension().fill("77");
      replace(good.replace("value: mm(14)", "value: mm(15)"));
      await expect(page.getByRole("region", { name: "Local folder" })).toContainText("Disk changed while an Inspector edit is pending");
      await expect(dimension()).toHaveValue("77");
      await dimension().press("Enter");
      await expect(page.getByRole("region", { name: "Local folder" })).toContainText("Conflict");
      assert.match(read(), /value: mm\(15\)/);
      await expect(page.getByRole("button", { name: "Download pending intent" })).toBeVisible();
      await page.getByRole("button", { name: "Refresh from disk" }).click();
      await expect(dimension()).toHaveValue("15");
      replace(good);
      await expect(dimension()).toHaveValue("14");
    });
    await t.test("stale RPC before watcher observes rename refuses overwrite", async () => {
      const previous = await status();
      const external = read().replace("value: mm(14)", "value: mm(16)");
      replace(external);
      const response = await rpc("dispatch", { version: 2, command: "source.prepare", payload: { path: "sketch.ts", contents: external.replace("value: mm(16)", "value: mm(99)") } }, previous.currentHash);
      assert.equal(response.status, 409);
      assert.match((await response.json()).error, /Conflict/);
      assert.equal(read(), external);
      await expect(dimension()).toHaveValue("16");
    });
    await t.test("stale browser draft preserves pending intent while external geometry updates", async () => {
      const external = read().replace("value: mm(16)", "value: mm(18)");
      await page.locator(".cm-content").fill(read().replace("value: mm(16)", "value: mm(99)"));
      replace(external);
      await expect(page.getByRole("region", { name: "Local folder" })).toContainText("Saved to disk");
      await expect.poll(async () => (await status()).currentHash).toBe(hash(external));
      await page.getByRole("button", { name: "Apply", exact: true }).click();
      await expect(page.getByRole("region", { name: "Local folder" })).toContainText("Conflict");
      await expect(page.getByRole("button", { name: "Download pending intent" })).toBeVisible();
      await expect(page.locator(".cm-content")).toContainText("value: mm(99)");
      assert.equal(read(), external);
      const downloadEvent = page.waitForEvent("download");
      await page.getByRole("button", { name: "Download pending intent" }).click();
      await (await downloadEvent).saveAs(resolve(evidence, "pending-intent.json"));
      await page.getByRole("button", { name: "Revert", exact: true }).click();
      await page.getByRole("button", { name: "Refresh from disk" }).click();
      await expect(dimension()).toHaveValue("18");
    });
    await t.test("existing manual canonical sketch export remains accessible", async () => {
      await page.getByRole("button", { name: "File menu" }).click();
      const downloaded = page.waitForEvent("download");
      await page.getByRole("menuitem", { name: "Export canonical project…" }).click();
      const exportPath = resolve(evidence, "manual-project-export.json");
      await (await downloaded).saveAs(exportPath);
      const exported = JSON.parse(readFileSync(exportPath, "utf8"));
      assert.ok(JSON.stringify(exported).includes("ringRadius"));
    });
    await t.test("bridge restart and browser reopen reconstruct accepted plaintext", async () => {
      const saved = read();
      await session.close();
      session = await serveProject(folder);
      await page.goto(session.url);
      await expect(dimension()).toHaveValue("18");
      await expect(page.getByRole("region", { name: "Local folder" })).toContainText("Saved to disk");
      assert.equal(read(), saved);
      await page.screenshot({ path: resolve(evidence, "04-reopened.png") });
    });
    await t.test("ordinary WASM demo still opens and authors independently", async () => {
      const ordinary = await browser.newPage({ viewport: { width: 1400, height: 900 } });
      await ordinary.goto(session.origin);
      await expect(ordinary.getByRole("region", { name: "Local folder" })).toHaveCount(0);
      await expect(ordinary.locator('canvas[data-renderer="webgl2"]')).toHaveAttribute("data-render-state", "ready");
      await ordinary.getByRole("button", { name: "File menu" }).click();
      await ordinary.getByRole("menuitem", { name: /Open/ }).click();
      await expect(ordinary.getByPlaceholder(/Search \d+ samples/)).toBeVisible();
      await ordinary.getByRole("button", { name: "Start from code", exact: true }).click();
      await expect(ordinary.locator(".cm-content")).toContainText("use geosolve sketch");
      await ordinary.screenshot({ path: resolve(evidence, "05-ordinary-demo.png") });
      await ordinary.close();
    });
    await t.test("restart on incomplete disk source rebuilds last-good view and keeps the invalid file", async () => {
      const good = read();
      const invalid = good.replace("value: mm(18)", "value: mm(");
      await session.close();
      replace(invalid);
      session = await serveProject(folder);
      await page.goto(session.url);
      await expect(page.getByRole("region", { name: "Local folder" })).toContainText("Last accepted geometry retained");
      await expect(canvas()).toHaveAttribute("data-render-state", "ready");
      assert.ok((await geometry()).some((item) => item.kind === "polyline" && item.points.length > 12));
      await expect(page.getByRole("region", { name: "Dimensions", exact: true })).toContainText("18 mm");
      assert.equal(read(), invalid);
      assert.equal((await status()).ok, false);
      replace(good);
      await expect(dimension()).toHaveValue("18");
      await expect(page.getByRole("region", { name: "Local folder" })).toContainText("Saved to disk");
    });
    await t.test("write endpoint requires token and matching origin", async () => {
      assert.equal((await fetch(`${session.origin}/api/rpc`, { method: "POST", body: "{}" })).status, 403);
      assert.equal((await fetch(`${session.origin}/api/status`, { headers: { Authorization: `Bearer ${session.token}`, Origin: "https://example.com" } })).status, 403);
      const before = read();
      assert.equal((await rpc("writeFile", { path: "../outside.ts", contents: "" }, (await status()).currentHash)).status, 400);
      assert.equal(read(), before);
      assert.equal((await status()).writes, 0);
    });
    await t.test("failed file write returns error, retains disk and compiled pending source", async () => {
      const before = read();
      const base = await status();
      chmodSync(folder, 0o500);
      try {
        const response = await rpc("dispatch", { version: 2, command: "source.prepare", payload: { path: "sketch.ts", contents: before.replace("value: mm(18)", "value: mm(20)") } }, base.currentHash);
        assert.equal(response.status, 400);
        const result = await response.json();
        assert.match(result.error, /EACCES/);
        assert.match(result.pendingSource, /value: mm\(20\)/);
        assert.equal(read(), before);
        assert.equal(result.state.writes, base.writes);
      } finally { chmodSync(folder, 0o700); }
    });
    assert.deepEqual(browserErrors, []);
    writeFileSync(resolve(evidence, "final-status.json"), JSON.stringify(await status(), null, 2));
  } finally { await browser.close(); await session.close(); }
});
