// SPDX-License-Identifier: GPL-3.0-or-later
import { expect, test, type Locator, type Page } from "@playwright/test";
import { compareFittedGeometry } from "../fitted-geometry";
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

async function expectFullCanvas(canvas: Locator, pixelRatio: number) {
  await expect.poll(async () => {
    const frame = await presentedFrame(canvas);
    const box = await canvas.boundingBox();
    if (!box) throw Error("Expected visible canvas");
    return Math.max(Math.abs(frame.viewBox[2] - box.width), Math.abs(frame.viewBox[3] - box.height));
  }, { message: "logical drawing area must fill the actual canvas without fixed-aspect letterboxing" }).toBeLessThan(0.01);
  const backing = await canvas.evaluate((element) => ({
    width: Reflect.get(element, "width"), height: Reflect.get(element, "height"),
    css: element.getBoundingClientRect().toJSON(), ratio: Reflect.get(globalThis, "devicePixelRatio"),
  }));
  expect(backing.ratio).toBe(pixelRatio);
  expect(Math.abs(backing.width - backing.css.width * pixelRatio)).toBeLessThanOrEqual(2);
  expect(Math.abs(backing.height - backing.css.height * pixelRatio)).toBeLessThanOrEqual(2);
}

async function exerciseFormerLetterbox(page: Page, canvas: Locator, size: { width: number; height: number }) {
  await page.setViewportSize(size);
  await page.getByRole("button", { name: "File menu" }).click();
  await page.getByRole("menuitem", { name: /Open/ }).click();
  await page.getByRole("button", { name: "New sketch" }).click();
  await expect(page.locator("header").getByText("Untitled sketch", { exact: true })).toBeVisible();
  await page.getByRole("button", { name: "design", exact: true }).click();
  await expectFullCanvas(canvas, 2);
  const box = (await canvas.boundingBox())!;
  const wide = box.width / box.height > 10 / 7;
  const band = wide ? (box.width - box.height * 10 / 7) / 2 : (box.height - box.width * 7 / 10) / 2;
  expect(band, "exercise a substantial area outside the old 1000 × 700 drawing rectangle").toBeGreaterThan(100);
  const start = wide ? { x: band / 2, y: box.height * 0.32 } : { x: box.width * 0.32, y: band / 2 };
  const end = { x: start.x + 45, y: start.y + 65 };
  await page.getByRole("button", { name: "Sketch", exact: true }).click();
  await page.getByRole("menuitem", { name: "Segment", exact: true }).click();
  await page.mouse.click(box.x + start.x, box.y + start.y);
  await page.mouse.click(box.x + end.x, box.y + end.y);
  const points = drawItems(canvas, { layer: "points", kind: "circle", interactive: true });
  await expect.poll(() => points.count()).toBe(2);
  await page.keyboard.press("Escape");
  const first = points.first(); const second = points.nth(1);
  const assertPosition = async (point: typeof first, position: { x: number; y: number }) => {
    await expect.poll(async () => {
      const [x, y] = await point.position();
      return Math.hypot(x - position.x, y - position.y);
    }).toBeLessThan(0.75);
  };
  // Expected coordinates come directly from real CSS mouse positions, independent
  // of the presented-frame coordinate helper used by the other canvas tests.
  await assertPosition(first, start);
  await assertPosition(second, end);
  await page.mouse.click(box.x + start.x, box.y + start.y);
  await expect(page.getByRole("tabpanel").getByText("Ownership", { exact: true })).toBeVisible();
  const saved = await savedWorkspace(page);
  const moved = { x: start.x + 23, y: start.y - 17 };
  await page.mouse.move(box.x + start.x, box.y + start.y);
  await page.mouse.down();
  await page.mouse.move(box.x + moved.x, box.y + moved.y, { steps: 4 });
  await page.mouse.up();
  await assertPosition(first, moved);
  await assertPosition(second, end);
  await expect.poll(() => savedWorkspace(page)).not.toBe(saved);
  const dragged = await savedWorkspace(page);
  await page.getByRole("button", { name: "Undo", exact: true }).click();
  await assertPosition(first, start);
  await assertPosition(second, end);
  await expect.poll(() => savedWorkspace(page)).not.toBe(dragged);
  const beforeNavigation = await savedWorkspace(page);

  // Zoom preserves the world point under the mouse; panning translates every
  // point by exactly the CSS displacement, including at DPR 2 in the old bands.
  const distance = async () => {
    const [a, b] = await Promise.all([first.position(), second.position()]);
    return Math.hypot(a[0] - b[0], a[1] - b[1]);
  };
  const beforeZoom = await distance();
  await page.mouse.move(box.x + start.x, box.y + start.y);
  await page.mouse.wheel(0, -120);
  await expect.poll(distance).toBeGreaterThan(beforeZoom * 1.04);
  await assertPosition(first, start);
  const beforePan = await second.position();
  await page.mouse.down({ button: "middle" });
  await page.mouse.move(box.x + start.x + 37, box.y + start.y + 29, { steps: 3 });
  await page.mouse.up({ button: "middle" });
  await assertPosition(first, { x: start.x + 37, y: start.y + 29 });
  await assertPosition(second, { x: beforePan[0] + 37, y: beforePan[1] + 29 });
  expect(await savedWorkspace(page)).toBe(beforeNavigation);
  return canvasVisualWitness(canvas);
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
    expect(compareFittedGeometry(await fittedGeometry(highDpr), geometry).equal).toBe(true);
    await expectFullCanvas(highCanvas, 2);
    const highSaved = await savedWorkspace(highDpr);
    for (const size of [{ width: 2200, height: 800 }, { width: 1280, height: 1200 }]) {
      const before = await presentedFrame(highCanvas);
      const beforePoints = await drawItems(highCanvas, { layer: "points", kind: "circle", interactive: true }).all();
      await highDpr.setViewportSize(size);
      await expectFullCanvas(highCanvas, 2);
      const after = await presentedFrame(highCanvas);
      const afterPoints = await drawItems(highCanvas, { layer: "points", kind: "circle", interactive: true }).all();
      expect(afterPoints.length).toBe(beforePoints.length);
      for (let i = 0; i < beforePoints.length; i++) {
        const a = beforePoints[i]; const b = afterPoints[i];
        if (a.kind !== "circle" || b.kind !== "circle") throw Error("Expected circular point markers");
        expect(b.metadata.persistentId).toBe(a.metadata.persistentId);
        expect(b.center[0] - after.viewBox[2] / 2).toBeCloseTo(a.center[0] - before.viewBox[2] / 2, 4);
        expect(b.center[1] - after.viewBox[3] / 2).toBeCloseTo(a.center[1] - before.viewBox[3] / 2, 4);
        expect(b.radius).toBe(a.radius);
      }
      expect(await savedWorkspace(highDpr)).toBe(highSaved);
      const visible = afterPoints.find((point) => point.kind === "circle"
        && point.center[0] > 10 && point.center[0] < after.viewBox[2] - 10
        && point.center[1] > 10 && point.center[1] < after.viewBox[3] - 10)!;
      const point = drawItems(highCanvas, { layer: "points", kind: "circle", persistentId: visible.metadata.persistentId });
      const bounds = await point.boundingBox();
      await highDpr.mouse.click(bounds.x + bounds.width / 2, bounds.y + bounds.height / 2);
      await expect(highDpr.getByRole("tabpanel").getByText("Ownership", { exact: true })).toBeVisible();
    }
    await highDpr.getByRole("button", { name: "code", exact: true }).click();
    await expect(highDpr.locator(".cm-editor")).toBeVisible();
    await highDpr.getByRole("button", { name: "design", exact: true }).click();
    await expectFullCanvas(highCanvas, 2);
    expect(await savedWorkspace(highDpr)).toBe(highSaved);
    await fittedGeometry(highDpr);
    await info.attach("canvas-dpr2-pixels", { body: JSON.stringify(await canvasVisualWitness(highCanvas)), contentType: "application/json" });
    // Pane minimums can change their retained proportions after a narrow window.
    // Free the Explorer width to make the wide/portrait margin witnesses explicit.
    await highDpr.getByRole("button", { name: "Hide Explorer", exact: true }).click();
    for (const size of [{ width: 2200, height: 800 }, { width: 1280, height: 1200 }]) {
      await info.attach(`canvas-former-letterbox-${size.width}x${size.height}`, {
        body: JSON.stringify(await exerciseFormerLetterbox(highDpr, highCanvas, size)), contentType: "application/json",
      });
    }
  } finally { await second.close(); }
  expect(await savedWorkspace(page)).toBe(saved);
  expect(compareFittedGeometry(await fittedGeometry(page), geometry).equal).toBe(true);
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
  // Zoom out so the complete fitted point markers remain available to the
  // pixel witness after restoration, including at a tall canvas aspect ratio.
  await page.mouse.wheel(0, 120);
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
