// SPDX-License-Identifier: GPL-3.0-or-later
import { expect, test, type Page } from "@playwright/test";
import { acceptedSource, fittedGeometry, samples, savedWorkspace } from "./release-sample-prefix";
import { canvasFrame, canvasVisualWitness, drawItems, fractionToClient, presentedFrame, presentedIdentity, rendererDiagnostics, settlePresentation } from "./presented-canvas";

async function openJansen(page: Page) {
  await page.setViewportSize({ width: 1440, height: 900 });
  await page.goto("/", { waitUntil: "networkidle" });
  const sample = samples.find((sample) => sample.key === "theo-jansen-leg")!;
  await page.getByRole("button", { name: "File menu" }).click();
  await page.getByRole("menuitem", { name: /Open/ }).click();
  await page.getByPlaceholder(`Search ${samples.length} samples…`).fill(sample.key);
  await page.getByRole("dialog", { name: "Open project" }).getByRole("button")
    .filter({ has: page.getByText(sample.manifest.title, { exact: true }) }).click();
  await expect.poll(() => acceptedSource(page), { timeout: 60_000 }).toBe(sample.source);
  await page.getByRole("button", { name: "design", exact: true }).click();
  await fittedGeometry(page);
  return canvasFrame(page);
}

test("M94 canvas presents actual WebGL2 pixels and remains idle without redrawing", async ({ page }, info) => {
  const canvas = await openJansen(page);
  await expect(page.getByRole("application").locator("svg")).toHaveCount(0);
  const observer = await canvas.evaluate((element) => {
    const descriptor = Object.getOwnPropertyDescriptor(element, "__geosolvePresentedFrame");
    const context = Reflect.apply(Reflect.get(element, "getContext"), element, ["webgl2"]) as { VERSION: number; getParameter(key: number): string; getExtension(key: string): { loseContext(): void; restoreContext(): void } | null } | null;
    return { getter: typeof descriptor?.get, setter: typeof descriptor?.set, webglVersion: context?.getParameter(context.VERSION) };
  });
  expect(observer).toMatchObject({ getter: "function", setter: "undefined" });
  expect(observer.webglVersion).toContain("WebGL 2.0");
  await page.mouse.move(0, 0);
  await settlePresentation(page);
  const visual = await canvasVisualWitness(canvas);
  await info.attach("canvas-gpu-pixels", { body: JSON.stringify(visual), contentType: "application/json" });
  const before = await rendererDiagnostics(canvas);
  await page.waitForTimeout(350);
  const after = await rendererDiagnostics(canvas);
  expect(after.frameCount).toBe(before.frameCount);
  expect(after.hardware).toBeTruthy();
  expect(after.state).toBe("ready");
});

test("M94 canvas aligns DPR resize and hidden layouts with presented geometry and picking", async ({ page, browser }, info) => {
  const canvas = await openJansen(page);
  const geometry = await fittedGeometry(page);
  const saved = await savedWorkspace(page);
  const second = await browser.newContext({ baseURL: new URL(".", page.url()).href, viewport: { width: 1440, height: 900 }, deviceScaleFactor: 2 });
  try {
    const highDpr = await second.newPage();
    const highCanvas = await openJansen(highDpr);
    expect(await fittedGeometry(highDpr)).toBe(geometry);
    const backing = await highCanvas.evaluate((element) => ({
      width: Reflect.get(element, "width"), height: Reflect.get(element, "height"),
      css: element.getBoundingClientRect().toJSON(), ratio: Reflect.get(globalThis, "devicePixelRatio"),
    }));
    expect(backing.ratio).toBe(2);
    expect(Math.abs(backing.width - backing.css.width * 2)).toBeLessThanOrEqual(2);
    expect(Math.abs(backing.height - backing.css.height * 2)).toBeLessThanOrEqual(2);
    for (const size of [{ width: 1440, height: 900 }, { width: 1600, height: 1000 }]) {
      await highDpr.setViewportSize(size);
      await settlePresentation(highDpr);
      const point = drawItems(highCanvas, { layer: "points", kind: "circle", interactive: true }).first();
      const bounds = await point.boundingBox();
      await highDpr.mouse.click(bounds.x + bounds.width / 2, bounds.y + bounds.height / 2);
      await expect(highDpr.getByRole("tabpanel").getByText("Ownership", { exact: true })).toBeVisible();
    }
    await highDpr.getByRole("button", { name: "code", exact: true }).click();
    await expect(highDpr.locator(".cm-editor")).toBeVisible();
    await highDpr.getByRole("button", { name: "design", exact: true }).click();
    await settlePresentation(highDpr);
    await info.attach("canvas-dpr2-pixels", { body: JSON.stringify(await canvasVisualWitness(highCanvas)), contentType: "application/json" });
  } finally { await second.close(); }
  expect(await savedWorkspace(page)).toBe(saved);
  expect(await fittedGeometry(page)).toBe(geometry);
  await canvasVisualWitness(canvas);
});

test("M94 canvas context loss retains presented evidence and restores the newest camera", async ({ page }, info) => {
  const canvas = await openJansen(page);
  const initial = await presentedIdentity(canvas);
  const workspace = await savedWorkspace(page);
  const before = await rendererDiagnostics(canvas);
  await canvas.evaluate((element) => {
    const context = Reflect.apply(Reflect.get(element, "getContext"), element, ["webgl2"]) as { VERSION: number; getParameter(key: number): string; getExtension(key: string): { loseContext(): void; restoreContext(): void } | null } | null;
    const extension = context?.getExtension("WEBGL_lose_context");
    if (!extension) throw Error("WebGL2 context loss extension unavailable");
    Reflect.set(globalThis, "__m94RestoreContext", () => extension.restoreContext());
    extension.loseContext();
  });
  await expect(canvas).toHaveAttribute("data-render-state", "lost");
  expect(await canvas.evaluate((element) => JSON.stringify(Reflect.get(element, "__geosolvePresentedFrame")))).toBe(initial);
  const position = await canvas.boundingBox();
  expect(position).not.toBeNull();
  await page.mouse.move(position!.x + position!.width / 2, position!.y + position!.height / 2);
  await page.mouse.wheel(0, -120);
  await page.waitForTimeout(150);
  expect((await rendererDiagnostics(canvas)).frameCount).toBe(before.frameCount);
  expect(await canvas.evaluate((element) => JSON.stringify(Reflect.get(element, "__geosolvePresentedFrame")))).toBe(initial);
  await page.evaluate(() => Reflect.apply(Reflect.get(globalThis, "__m94RestoreContext"), globalThis, []));
  await expect(canvas).toHaveAttribute("data-render-state", "ready");
  await expect.poll(() => presentedIdentity(canvas)).not.toBe(initial);
  expect(await savedWorkspace(page)).toBe(workspace);
  await info.attach("canvas-restored-pixels", { body: JSON.stringify(await canvasVisualWitness(canvas)), contentType: "application/json" });
});

test("M94 canvas lost pointer capture cancels a preview and permits the next real gesture", async ({ page }) => {
  const canvas = await openJansen(page);
  const host = page.getByRole("application");
  const points = await drawItems(canvas, { layer: "points", kind: "circle", interactive: true }).all();
  const crank = points.reduce((best, point) => point.kind === "circle" && best.kind === "circle" && point.center[0] > best.center[0] ? point : best);
  const point = drawItems(canvas, { layer: "points", kind: "circle", persistentId: crank.metadata.persistentId });
  const initial = await point.position();
  const saved = await savedWorkspace(page);
  await host.evaluate((element) => {
    element.addEventListener("pointerdown", (event) => Reflect.set(element, "__m94PointerId", Reflect.get(event, "pointerId")), { once: true });
  });
  const box = await point.boundingBox(); const start = { x: box.x + box.width / 2, y: box.y + box.height / 2 };
  await page.mouse.move(start.x, start.y);
  await page.mouse.down();
  await page.mouse.move(start.x - 2, start.y - 12, { steps: 4 });
  await expect.poll(() => point.position()).not.toEqual(initial);
  await host.evaluate((element) => {
    const id = Reflect.get(element, "__m94PointerId");
    if (!Reflect.apply(Reflect.get(element, "hasPointerCapture"), element, [id])) throw Error("Real pointer was not captured");
    Reflect.apply(Reflect.get(element, "releasePointerCapture"), element, [id]);
  });
  await page.mouse.up();
  await expect.poll(() => point.position()).toEqual(initial);
  expect(await savedWorkspace(page)).toBe(saved);
  await expect(page.getByRole("button", { name: "Undo", exact: true })).toBeDisabled();
  const click = await fractionToClient(canvas, { x: 0.4, y: 0.4 });
  await page.mouse.click(click.x, click.y);
  const refreshed = await point.boundingBox();
  await page.mouse.click(refreshed.x + refreshed.width / 2, refreshed.y + refreshed.height / 2);
  await expect(page.getByRole("tabpanel").getByText("Ownership", { exact: true })).toBeVisible();
  expect((await presentedFrame(canvas)).provenance.scene).toBe("accepted");
});
