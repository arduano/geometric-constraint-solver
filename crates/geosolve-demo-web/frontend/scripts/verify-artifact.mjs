// SPDX-License-Identifier: GPL-3.0-or-later

import { randomUUID } from "node:crypto";
import { writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { parseArgs } from "node:util";
import { pathToFileURL } from "node:url";
import { hash, mediaType, readManifest } from "./release-artifact-lib.mjs";

export async function verifyTransport(artifact, baseUrl) {
  const base = new URL(baseUrl);
  if (!["http:", "https:"].includes(base.protocol) || !base.pathname.endsWith("/") || base.search || base.hash
    || base.username || base.password) throw new Error("endpoint must be an HTTP(S) base URL with a trailing slash");
  if (artifact.manifest.publicBase !== "./" && base.pathname !== artifact.manifest.publicBase) {
    throw new Error("endpoint path does not match the artifact public base");
  }
  const index = artifact.manifest.files.find((file) => file.path === "index.html");
  const routes = [{ ...index, route: "" }, ...artifact.manifest.files.map((file) => ({ ...file, route: file.path }))];
  const verified = [];
  for (const file of routes) {
    const url = new URL(file.route, base);
    url.searchParams.set("geosolve_verify", randomUUID());
    const response = await fetch(url, { redirect: "manual", signal: AbortSignal.timeout(10_000),
      headers: { "Accept-Encoding": "identity", "Cache-Control": "no-cache", Pragma: "no-cache" } });
    const type = response.headers.get("content-type")?.split(";", 1)[0].trim().toLowerCase();
    const expectedType = mediaType(file.path);
    if (response.status !== 200 || response.headers.has("location")) throw new Error(`${file.route || "/"}: HTTP ${response.status} or redirect`);
    if (type !== expectedType && !(expectedType === "text/javascript" && type === "application/javascript")) {
      throw new Error(`${file.route || "/"}: expected ${expectedType}, received ${type}`);
    }
    const encoding = response.headers.get("content-encoding");
    if (encoding && encoding !== "identity") throw new Error(`${file.route || "/"}: compressed transport instead of exact bytes`);
    const bytes = Buffer.from(await response.arrayBuffer());
    if (bytes.length !== file.bytes || hash(bytes) !== file.sha256) throw new Error(`${file.route || "/"}: HTTP bytes differ from artifact`);
    verified.push({ route: file.route || "/", status: response.status, contentType: type, bytes: bytes.length, sha256: hash(bytes) });
  }
  return verified;
}

export function verifyWorkbenchWasmLoaded(manifest, baseUrl, wasmResponses) {
  const modules = manifest.files.filter((file) => /^assets\/geosolve_demo_web_bg(?:-[A-Za-z0-9_-]+)?\.wasm$/.test(file.path));
  if (modules.length !== 1) throw new Error("readiness requires exactly one nominated workbench WASM module");
  const expectedUrl = new URL(modules[0].path, baseUrl).href;
  if (!wasmResponses.has(expectedUrl)) throw new Error("readiness did not load the nominated WASM module");
  return expectedUrl;
}

async function readiness(baseUrl, manifest) {
  const { chromium, expect: playwrightExpect } = await import("@playwright/test");
  // Worker evaluation leaves the page responsive before accepted geometry is ready.
  // Every readiness wait remains bounded by the overall 60-second deadline below.
  const expect = playwrightExpect.configure({ timeout: 60_000 });
  const browser = await chromium.launch(process.env.GEOSOLVE_CHROMIUM_PATH ? { executablePath: process.env.GEOSOLVE_CHROMIUM_PATH } : {});
  const errors = [];
  const wasmResponses = new Set();
  let deadline;
  try {
    return await Promise.race([
      (async () => {
        const context = await browser.newContext({ viewport: { width: 1024, height: 720 }, serviceWorkers: "block" });
        const page = await context.newPage();
        page.setDefaultTimeout(30_000);
        page.on("pageerror", (error) => errors.push(`page: ${error.message}`));
        page.on("console", (message) => { if (message.type() === "error") errors.push(`console: ${message.text()}`); });
        page.on("requestfailed", (request) => errors.push(`request: ${request.url()} ${request.failure()?.errorText}`));
        page.on("response", (response) => {
          if (response.status() >= 400) errors.push(`HTTP ${response.status()}: ${response.url()}`);
          if (new URL(response.url()).pathname.endsWith(".wasm") && response.status() === 200) wasmResponses.add(response.url());
        });
        await page.goto(baseUrl, { waitUntil: "networkidle" });
        await expect(page.locator("header").getByText("Untitled sketch", { exact: true })).toBeVisible();
        await page.getByRole("button", { name: "File menu" }).click();
        await page.getByRole("menuitem", { name: /Open/ }).click();
        await page.getByPlaceholder(/Search \d+ samples/).fill("water manifold");
        await page.getByRole("button", { name: "PC liquid-cooling manifold", exact: false }).click();
        await expect(page.locator("header").getByText("PC liquid-cooling manifold", { exact: true })).toBeVisible();
        const application = page.getByRole("application");
        await expect(application).toHaveAttribute("aria-busy", "false");
        await page.evaluate(() => new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve))));
        await expect(application).toHaveAttribute("aria-busy", "false");
        const frame = page.locator('[role="application"] canvas[data-renderer="webgl2"]');
        await expect(frame).toHaveCount(1);
        await expect(frame).toHaveAttribute("data-render-state", "ready");
        await expect(page.locator('[role="application"] svg')).toHaveCount(0);
        const drawing = await frame.evaluate((canvas) => canvas.__geosolvePresentedFrame);
        expect(drawing?.format).toBe("geosolve-draw-frame-v1");
        expect(drawing.provenance.scene).toBe("accepted");
        const geometryCount = drawing.items.filter((item) => ["geometry", "computed"].includes(item.layer)).length;
        expect(geometryCount).toBeGreaterThan(10);
        const renderer = await frame.evaluate((canvas) => canvas.__geosolveRendererDiagnostics);
        expect(renderer.backend).toBe("webgl2");
        expect(renderer.state).toBe("ready");
        const screenshotSha256 = hash(await frame.screenshot());
        await expect(page.locator("header").getByText(/accepted · r/i)).toBeVisible();
        const expectedUrl = verifyWorkbenchWasmLoaded(manifest, baseUrl, wasmResponses);
        if (errors.length) throw new Error(errors.join("\n"));
        return { status: "passed", browserVersion: browser.version(), sample: "pc-water-manifold",
          wasmUrl: expectedUrl, geometryCount, screenshotSha256, renderer, errors };
      })(),
      new Promise((_, reject) => { deadline = setTimeout(() => reject(new Error("runtime readiness exceeded 60 seconds")), 60_000); }),
    ]);
  } finally {
    clearTimeout(deadline);
    await browser.close();
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  const { values } = parseArgs({ options: { manifest: { type: "string" }, directory: { type: "string" },
    url: { type: "string" }, receipt: { type: "string" }, "transport-only": { type: "boolean", default: false } } });
  if (!values.manifest || !values.url || !values.receipt) {
    throw new Error("usage: node scripts/verify-artifact.mjs --manifest <manifest> --url <base-url> --receipt <new-path> [--directory <moved-copy>] [--transport-only]");
  }
  const receipt = { format: "geosolve-artifact-verification-v1", startedAt: new Date().toISOString(),
    endpoint: values.url, command: process.argv.slice(1), status: "failed", transport: [], runtime: { status: "not_run" } };
  try {
    const artifact = await readManifest(values.manifest, values.directory);
    Object.assign(receipt, { manifestSha256: artifact.manifestSha256, filesSha256: artifact.manifest.filesSha256, kind: artifact.manifest.kind });
    receipt.transport = await verifyTransport(artifact, values.url);
    if (!values["transport-only"]) receipt.runtime = await readiness(values.url, artifact.manifest);
    receipt.status = values["transport-only"] ? "transport_only_passed" : "passed";
  } catch (error) {
    receipt.error = error.message;
    process.exitCode = 1;
  } finally {
    receipt.finishedAt = new Date().toISOString();
    await writeFile(values.receipt, `${JSON.stringify(receipt, null, 2)}\n`, { flag: "wx" });
  }
  console.log(`${receipt.status}: ${values.receipt}`);
}
