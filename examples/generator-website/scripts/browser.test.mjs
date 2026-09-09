// SPDX-License-Identifier: GPL-3.0-or-later
import assert from "node:assert/strict";
import test from "node:test";
import { mkdirSync, readFileSync } from "node:fs";
import { resolve } from "node:path";
import { buildExample, folder, repository } from "./build.mjs";
import { serveExample } from "./serve.mjs";

const { chromium } = await import(resolve(repository, "crates/geosolve-demo-web/frontend/node_modules/playwright/index.mjs"));

test("custom website uses actual Rust WASM, owns its UI and retains accepted export through cancellation", { timeout: 120000 }, async (t) => {
  await buildExample();
  const server = await serveExample();
  const browser = await chromium.launch({ executablePath: process.env.GEOSOLVE_CHROMIUM_PATH ?? "/home/arduano/.nix-profile/bin/google-chrome", args: ["--disable-dev-shm-usage"] });
  t.after(async () => { await browser.close(); await server.close(); });
  const page = await browser.newPage({ viewport: { width: 1200, height: 1000 }, acceptDownloads: true });
  const errors = [], wasm = [];
  page.on("pageerror", (error) => errors.push(error.message));
  page.on("response", (response) => { if (response.url().endsWith(".wasm")) wasm.push(response.status()); });
  await page.goto(server.url);
  await page.locator('#status[data-state="ready"]').waitFor({ timeout: 30000 });
  assert.equal(await page.locator("#width").textContent(), "125.5 mm");
  assert.equal(await page.locator("#height").textContent(), "83.5 mm");
  assert.equal(await page.locator("#bores").textContent(), "24");
  assert.equal(await page.locator("#artwork .footprint").count(), 1);
  const initial = await page.locator("#preview").getAttribute("data-input-digest");
  await page.getByLabel("Columns", { exact: true }).fill("1");
  await page.locator('#status[data-state="ready"]').waitFor({ timeout: 30000 });
  assert.equal(await page.locator("#width").textContent(), "41.5 mm");
  assert.equal(await page.locator("#bores").textContent(), "8");
  assert.notEqual(await page.locator("#preview").getAttribute("data-input-digest"), initial);
  await page.getByLabel("Columns", { exact: true }).fill("3");
  await page.getByLabel("Columns", { exact: true }).fill("2");
  await page.getByLabel("Columns", { exact: true }).fill("1");
  await page.locator('#status[data-state="ready"]').waitFor({ timeout: 30000 });
  assert.equal(await page.locator("#width").textContent(), "41.5 mm", "superseded work never replaces the latest input");
  const accepted = await page.locator("#preview").getAttribute("data-input-digest");
  await page.getByLabel("Rows", { exact: true }).fill("0");
  await page.locator('#status[data-state="error"]').waitFor();
  assert.equal(await page.locator("#preview").getAttribute("data-input-digest"), accepted);
  assert.match(await page.locator("#status").textContent(), /Previous footprint retained/);
  const downloadEvent = page.waitForEvent("download");
  await page.getByRole("button", { name: "Download profile" }).click();
  const download = await downloadEvent;
  const exportData = JSON.parse(readFileSync(await download.path(), "utf8"));
  assert.equal(exportData.format, "geosolve-baked-profile-v1");
  assert.equal(exportData.generator.inputs.rows, 2, "download uses accepted input, not invalid controls");
  assert.equal(exportData.regions[0].holes.length, 8);

  // A real spinning worker demonstrates host preemption; the last actual accepted
  // engine result remains visible. No mocked geometry or native acceptance response.
  await page.route("**/worker.js", (route) => route.fulfill({ contentType: "text/javascript", body: "self.onmessage=()=>{while(true){}};" }));
  await page.getByLabel("Rows", { exact: true }).fill("3");
  await page.getByRole("button", { name: "Cancel update" }).waitFor();
  await page.getByRole("button", { name: "Cancel update" }).click();
  assert.match(await page.locator("#status").textContent(), /cancelled/);
  assert.equal(await page.locator("#preview").getAttribute("data-input-digest"), accepted);
  await page.clock.install();
  await page.getByRole("button", { name: "Update footprint" }).click();
  await page.clock.fastForward(21000);
  await page.locator('#status[data-state="error"]').waitFor();
  assert.match(await page.locator("#status").textContent(), /too long/);
  assert.equal(await page.locator("#preview").getAttribute("data-input-digest"), accepted);
  await page.unroute("**/worker.js");
  await page.clock.resume();
  await page.getByRole("button", { name: "Reset", exact: true }).click();
  await page.locator('#status[data-state="ready"]').waitFor({ timeout: 30000 });
  assert.equal(await page.locator("#preview").getAttribute("data-input-digest"), initial);
  const evidence = resolve(folder, "test-output"); mkdirSync(evidence, { recursive: true });
  await page.screenshot({ path: resolve(evidence, "desktop.png"), fullPage: true });
  await page.setViewportSize({ width: 390, height: 844 });
  assert.equal(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), true);
  await page.screenshot({ path: resolve(evidence, "mobile.png"), fullPage: true });
  assert.deepEqual(errors, []);
  assert.ok(wasm.every((status) => status === 200));
});
