// SPDX-License-Identifier: GPL-3.0-or-later
import { readFile } from "node:fs/promises";
import { readFileSync } from "node:fs";
import { inflateSync } from "node:zlib";
import { expect, test, type Page } from "@playwright/test";
import { canvasFrame, canvasVisualWitness, drawItems, expectDraft, expectItemCount, fractionToClient, presentedFrame, presentedIdentity, settlePresentation } from "./presented-canvas";

const MANIFOLD_TITLE = "PC liquid-cooling manifold";
const JANSEN_TITLE = "Theo Jansen-style walking leg · 1 DOF";
const SCISSOR_TITLE = "Generated five-stage scissor lift · 1 DOF";
const reviewedCatalog = JSON.parse(readFileSync(
  new URL("../../../../geosolve-sketch-code/assets/bundled-sample-catalog.json", import.meta.url), "utf8",
)) as { schema: number; samples: Array<{ key: string; title: string; category: string }>; retired_keys: string[] };
expect(reviewedCatalog.schema).toBe(1);
expect(reviewedCatalog.samples.length).toBeGreaterThan(0);


function auditRuntime(page: Page) {
  const errors: string[] = [];
  page.on("pageerror", (error) => errors.push(`page: ${error.message}`));
  page.on("console", (message) => {
    if (message.type() === "error") errors.push(`console: ${message.text()}`);
  });
  page.on("requestfailed", (request) => {
    errors.push(`network: ${request.method()} ${request.url()} (${request.failure()?.errorText ?? "unknown failure"})`);
  });
  page.on("response", (response) => {
    if (response.status() >= 400) errors.push(`http: ${response.status()} ${response.url()}`);
  });
  return () => expect(errors).toEqual([]);
}

async function boot(page: Page) {
  await page.goto("/", { waitUntil: "networkidle" });
  await expect(page.locator("header").getByText("Untitled sketch", { exact: true })).toBeVisible();
}

async function readSavedProject(page: Page) {
  return page.evaluate<string | null>(`(async () => {
    const database = await new Promise((resolve, reject) => {
      const request = indexedDB.open("geosolve.browser-projects.v1", 1);
      request.onsuccess = () => resolve(request.result);
      request.onerror = () => reject(request.error);
    });
    try {
      return await new Promise((resolve, reject) => {
        const request = database.transaction("projects", "readonly").objectStore("projects").get("current");
        request.onsuccess = () => resolve(typeof request.result === "string" ? request.result : null);
        request.onerror = () => reject(request.error);
      });
    } finally {
      database.close();
    }
  })()`);
}

function parseJsonRecord(value: string): Record<string, unknown> | null {
  try {
    const parsed: unknown = JSON.parse(value);
    return typeof parsed === "object" && parsed !== null && !Array.isArray(parsed)
      ? parsed as Record<string, unknown>
      : null;
  } catch {
    return null;
  }
}

function acceptedManagedSourceFromSavedProject(value: string | null) {
  if (value === null) return null;
  const presentation = parseJsonRecord(value);
  if (
    presentation?.format !== "geosolve-workbench-presentation-v1"
    || typeof presentation.project !== "string"
  ) return null;

  const codeWorkbench = parseJsonRecord(presentation.project);
  if (
    !["geosolve-code-workbench-v4", "geosolve-code-workbench-v5"].includes(String(codeWorkbench?.version))
    || typeof codeWorkbench.project !== "string"
  ) return null;

  const codeProject = parseJsonRecord(codeWorkbench.project);
  const managed = codeProject?.managed;
  if (typeof managed !== "object" || managed === null || Array.isArray(managed)) return null;
  const source = Reflect.get(managed, "source");
  return typeof source === "string" ? source : null;
}

async function readAcceptedManagedSource(page: Page) {
  return acceptedManagedSourceFromSavedProject(await readSavedProject(page));
}

async function waitForAcceptedManagedSource(
  page: Page,
  predicate: (source: string) => boolean,
) {
  let accepted: string | null = null;
  await expect.poll(async () => {
    accepted = await readAcceptedManagedSource(page);
    return accepted !== null && predicate(accepted);
  }, { timeout: 30_000 }).toBe(true);
  if (accepted === null) throw new Error("accepted managed source is unavailable");
  return accepted;
}

function savedProjectFingerprint(value: string | null) {
  if (value === null) return null;
  let checksum = 0x811c9dc5;
  for (let index = 0; index < value.length; index += 1) {
    checksum = Math.imul(checksum ^ value.charCodeAt(index), 0x01000193);
  }
  return `${value.length}:${checksum >>> 0}`;
}

async function openManifold(page: Page) {
  await page.getByRole("button", { name: "File menu" }).click();
  await page.getByRole("menuitem", { name: /Open/ }).click();
  await page.getByPlaceholder(`Search ${reviewedCatalog.samples.length} samples…`).fill("water manifold");
  await page.getByRole("button", { name: new RegExp(MANIFOLD_TITLE) }).click();
  await expect(page.locator("header").getByText(MANIFOLD_TITLE, { exact: true })).toBeVisible();
  await expect(page.locator(".cm-content")).toContainText('"use geosolve sketch"');
}

async function openJansen(page: Page) {
  await page.getByRole("button", { name: "File menu" }).click();
  await page.getByRole("menuitem", { name: /Open/ }).click();
  await page.getByPlaceholder(`Search ${reviewedCatalog.samples.length} samples…`).fill("Jansen");
  await page.getByRole("button", { name: new RegExp(JANSEN_TITLE) }).click();
  await expect(page.locator("header").getByText(JANSEN_TITLE, { exact: true })).toBeVisible();
  await expect(page.locator(".cm-content")).toContainText('"use geosolve sketch"');
}

async function openScissorLift(page: Page) {
  await page.getByRole("button", { name: "File menu" }).click();
  await page.getByRole("menuitem", { name: /Open/ }).click();
  await page.getByPlaceholder(`Search ${reviewedCatalog.samples.length} samples…`).fill("five-stage scissor");
  await page.getByRole("button", { name: new RegExp(SCISSOR_TITLE) }).click();
  await expect(page.locator("header").getByText(SCISSOR_TITLE, { exact: true })).toBeVisible();
  await expect(page.locator(".cm-content")).toContainText('"use geosolve sketch"');
}

async function openNewSketch(page: Page) {
  await page.getByRole("button", { name: "File menu" }).click();
  await page.getByRole("menuitem", { name: /Open/ }).click();
  await page.getByRole("button", { name: "New sketch" }).click();
  await expect(page.locator("header").getByText("Untitled sketch", { exact: true })).toBeVisible();
}

async function openControlledCodeProject(page: Page, source: string) {
  await page.getByRole("button", { name: "File menu" }).click();
  await page.getByRole("menuitem", { name: /Open/ }).click();
  await page.getByRole("button", { name: "Start from code" }).click();
  const content = page.locator(".cm-content");
  await expect(content).toContainText('"use geosolve sketch"');
  await content.click();
  await page.keyboard.press("Control+A");
  await page.keyboard.insertText(source);
  const apply = page.getByRole("button", { name: "Apply" });
  await expect(apply).toBeEnabled();
  await apply.click();
  const revert = page.getByRole("button", { name: "Revert" });
  if (await revert.isEnabled()) await revert.click();
  await expect(page.locator("header").first()).toContainText(/accepted/i);
  await expect(content).toContainText("const first = $.geometry.segment");
  await page.getByRole("button", { name: "split", exact: true }).click();
  await page.getByRole("button", { name: "Fit sketch" }).click();
}

test("real Chromium exactly reproduces the pinned Deno managed compiler envelope", async ({ page }) => {
  const assertCleanRuntime = auditRuntime(page);
  const fixtureRoot = new URL(
    "../../../../../packages/geosolve-sketch-code/test/fixtures/",
    import.meta.url,
  );
  const [source, expectedEnvelope] = await Promise.all([
    readFile(new URL("managed-compiler-envelope.sketch.ts", fixtureRoot), "utf8"),
    readFile(new URL("managed-compiler-envelope.json", fixtureRoot), "utf8"),
  ]);

  await page.goto("/compiler-parity.html", { waitUntil: "networkidle" });
  await expect(page.locator("body")).toHaveAttribute("data-compiler-ready", "true");
  const browserEnvelope = await page.evaluate((managedSource) => {
    const compile = Reflect.get(globalThis, "__geosolveCompileManagedSource");
    if (typeof compile !== "function") throw new Error("managed browser compiler is unavailable");
    return JSON.stringify(Reflect.apply(compile, undefined, [managedSource]));
  }, source);

  expect(browserEnvelope).toBe(expectedEnvelope);
  assertCleanRuntime();
});

test("real WASM opens an actual sample with a styled authoritative canvas and layout floors", async ({ page }) => {
  const assertCleanRuntime = auditRuntime(page);
  await page.addInitScript(() => {
    localStorage.setItem(
      "react-resizable-panels:geosolve-workbench-shell",
      JSON.stringify({ "details,explorer,workspace": { expandToSizes: {}, layout: [17, 62, 21] } }),
    );
  });
  await boot(page);
  const explorer = page.getByRole("complementary", { name: "Explorer" });
  const explorerBounds = await explorer.boundingBox();
  expect(explorerBounds?.width).toBeGreaterThanOrEqual(145);
  expect(explorerBounds?.width).toBeLessThanOrEqual(175);
  const imported = explorer.getByRole("button", { name: "legacy-document", exact: true });
  await expect(imported).toHaveText("legacy-document");
  const importedIcon = imported.locator("svg.lucide-box");
  await expect(importedIcon).toHaveClass(/\bshrink-0\b/);
  expect(await importedIcon.evaluate((element) => ({
    flexShrink: element.ownerDocument.defaultView?.getComputedStyle(element).flexShrink,
    width: element.getBoundingClientRect().width,
  }))).toEqual({ flexShrink: "0", width: 14 });
  await imported.click();
  await expect(page.getByRole("tabpanel").getByText("Imported", { exact: true })).toBeVisible();
  await expect(page.locator("body")).not.toContainText("IntentBootstrapMetadata");
  await expect(page.locator("body")).not.toContainText("Bootstrap {");
  await openManifold(page);

  const frame = canvasFrame(page);
  await expect(frame).toHaveCount(1);
  const scene = await presentedFrame(frame);
  expect(scene.provenance.scene).toBe("accepted");
  expect(scene.items.filter((item) => item.kind === "polyline").length).toBeGreaterThan(10);
  expect(scene.items.some((item) => item.layer === "points" && item.style.strokeWidth > 0)).toBe(true);
  await canvasVisualWitness(frame);

  await page.getByRole("button", { name: "File menu" }).click();
  await page.getByRole("menuitem", { name: /Open/ }).click();
  await expect(page.getByRole("region", { name: "Recent" }).getByRole("button", { name: new RegExp(MANIFOLD_TITLE) })).toBeVisible();
  await page.keyboard.press("Escape");

  await page.getByRole("button", { name: "code", exact: true }).click();
  const editor = page.locator(".cm-editor");
  await expect(editor).toBeVisible();
  const codeBounds = await editor.boundingBox();
  expect(codeBounds?.width).toBeGreaterThanOrEqual(720);
  expect(codeBounds?.height).toBeGreaterThanOrEqual(500);
  expect(Number.parseFloat(await page.locator(".cm-content").evaluate((element) => element.ownerDocument.defaultView?.getComputedStyle(element).fontSize ?? "0"))).toBeGreaterThanOrEqual(12);

  await page.setViewportSize({ width: 1440, height: 900 });
  await page.getByRole("button", { name: "split", exact: true }).click();
  const splitBounds = await editor.boundingBox();
  expect(splitBounds?.width).toBeGreaterThanOrEqual(520);
  expect(splitBounds?.height).toBeGreaterThanOrEqual(500);
  assertCleanRuntime();
});

test("CodeMirror DOM, selection, scroll, and browser-local draft survive layout changes", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  const assertCleanRuntime = auditRuntime(page);
  await boot(page);
  await openManifold(page);

  const editor = page.locator(".cm-editor");
  const content = page.locator(".cm-content");
  const scroller = page.locator(".cm-scroller");
  await editor.evaluate((element) => { Reflect.set(globalThis, "__geosolveEditor", element); });
  await content.click();
  await page.keyboard.press("Control+End");
  const filler = Array.from({ length: 90 }, (_, index) => `// continuity filler ${index}`).join("\n");
  const selectedText = "CONTINUITY_TARGET";
  await page.keyboard.insertText(`\n${filler}\n// ${selectedText}`);
  for (let index = 0; index < selectedText.length; index += 1) await page.keyboard.press("Shift+ArrowLeft");
  await scroller.evaluate((element) => { element.scrollTop = element.scrollHeight; });
  const scrollBefore = await scroller.evaluate((element) => element.scrollTop);
  expect(scrollBefore).toBeGreaterThan(0);

  const separator = page.getByRole("separator", { name: "Resize canvas and code" });
  const widthBefore = Number(await separator.getAttribute("aria-valuenow"));
  await separator.press("ArrowLeft");
  expect(Number(await separator.getAttribute("aria-valuenow"))).toBeGreaterThan(widthBefore);
  await page.getByRole("button", { name: "design", exact: true }).click();
  await page.getByRole("button", { name: "code", exact: true }).click();

  expect(await editor.evaluate((element) => Reflect.get(globalThis, "__geosolveEditor") === element)).toBe(true);
  expect(Math.abs((await scroller.evaluate((element) => element.scrollTop)) - scrollBefore)).toBeLessThanOrEqual(1);
  await content.evaluate((element) => element.focus());
  await page.keyboard.insertText("KEPT");
  await expect(content).toContainText("// KEPT");
  await expect(content).not.toContainText(selectedText);

  let downloads = 0;
  page.on("download", () => { downloads += 1; });
  await page.getByRole("button", { name: "File menu" }).click();
  await expect(page.getByRole("menuitem", { name: "Download raw draft…" })).toBeVisible();
  await page.getByRole("menuitem", { name: "Export canonical project…" }).click();
  await expect(page.getByRole("alert")).toContainText("Apply or Revert the current source draft");
  await page.waitForTimeout(100);
  expect(downloads).toBe(0);
  assertCleanRuntime();
});

test("invalid source retains the accepted frame and one positioned Problem until Revert", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  const assertCleanRuntime = auditRuntime(page);
  await boot(page);
  await openManifold(page);
  const frame = canvasFrame(page);
  const acceptedFrame = await presentedIdentity(frame);

  await page.getByRole("button", { name: "code", exact: true }).click();
  const content = page.locator(".cm-content");
  await content.click();
  await page.keyboard.press("Control+End");
  await page.keyboard.insertText("\nconst = ;\n");
  await page.getByRole("button", { name: "Apply" }).click();
  await expect(page.locator("header").first()).toContainText(/failed/i);
  const problems = page.getByRole("tab", { name: "Problems (1)" });
  await expect(problems).toBeVisible();
  await problems.click();
  await expect(page.getByRole("tabpanel").getByRole("button")).toHaveCount(1);
  await expect(page.getByRole("tabpanel")).toContainText(/sketch\.ts:\d+:/);
  expect(await presentedIdentity(frame)).toBe(acceptedFrame);

  await page.getByRole("button", { name: "File menu" }).click();
  await expect(page.getByRole("menuitem", { name: "Download raw draft…" })).toBeVisible();
  await page.getByRole("menuitem", { name: "Export canonical project…" }).click();
  await expect(page.getByRole("alert")).toBeVisible();
  await page.getByRole("button", { name: "Dismiss action error" }).click();
  await page.getByRole("button", { name: "Revert" }).click();
  await expect(page.locator("header").first()).toContainText(/accepted/i);
  await expect(page.getByRole("tab", { name: "Problems" })).toBeVisible();
  await expect(page.getByRole("tabpanel")).toContainText("No problems");
  expect(await presentedIdentity(frame)).toBe(acceptedFrame);
  assertCleanRuntime();
});

test("managed parameter edit persists through a real bridge reload", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  const assertCleanRuntime = auditRuntime(page);
  await boot(page);
  await openManifold(page);
  const originalSource = await waitForAcceptedManagedSource(
    page,
    (source) => /const reservoirWidth = \$\.dimension\.curveLength\([\s\S]*?value: mm\(60\),[\s\S]*?\n  \}\);/u.test(source),
  );
  await page.getByRole("tab", { name: "Parameters" }).click();
  const width = page.getByRole("textbox", { name: "reservoirWidth · value" });
  await width.fill("62");
  await width.press("Enter");
  await expect(page.getByRole("textbox", { name: "reservoirWidth · value" })).toHaveValue("62");
  await expect(page.locator("header").first()).toContainText(/accepted/i);
  const editedSource = await waitForAcceptedManagedSource(
    page,
    (source) => source !== originalSource
      && /const reservoirWidth = \$\.dimension\.curveLength\([\s\S]*?value: mm\(62\),[\s\S]*?\n  \}\);/u.test(source),
  );

  await page.reload({ waitUntil: "networkidle" });
  await expect(page.locator("header").getByText(MANIFOLD_TITLE, { exact: true })).toBeVisible();
  await page.getByRole("tab", { name: "Parameters" }).click();
  await expect(page.getByRole("textbox", { name: "reservoirWidth · value" })).toHaveValue("62");
  await waitForAcceptedManagedSource(page, (source) => source === editedSource);
  assertCleanRuntime();
});

test("real WASM source-backs click-authored geometry in a managed sample", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  const assertCleanRuntime = auditRuntime(page);
  await boot(page);
  await openManifold(page);
  const originalSource = await waitForAcceptedManagedSource(
    page,
    (source) => source.includes("const reservoirWidth = $.dimension.curveLength"),
  );

  const canvas = page.getByRole("application");
  const frame = canvasFrame(page);
  const bounds = await canvas.boundingBox();
  expect(bounds).not.toBeNull();
  const click = async (x: number, y: number) => {
    await page.mouse.click(bounds!.x + bounds!.width * x, bounds!.y + bounds!.height * y);
  };

  await page.getByRole("button", { name: "Sketch", exact: true }).click();
  await page.getByRole("menuitem", { name: "Segment" }).click();
  await click(0.35, 0.48);
  await expectDraft(canvasFrame(page), true);
  await click(0.65, 0.55);

  const upgradedSource = await waitForAcceptedManagedSource(
    page,
    (source) => source !== originalSource
      && source.includes("const segment1 = $.geometry.segment")
      && source.includes('$.group("Canvas additions", [segment1]);'),
  );
  await expect(
    page.getByRole("list", { name: "Canvas additions" })
      .getByRole("button", { name: "segment1", exact: true }),
  ).toBeVisible();
  await expect(page.getByText("Accepted source", { exact: true })).toBeVisible();
  await expectDraft(canvasFrame(page), false);
  await expect(page.getByRole("button", { name: "Undo" })).toBeEnabled();
  await expect(page.getByText(
    /pointer input is unavailable while a managed-source mutation is compiling/u,
  )).toHaveCount(0);

  await page.getByRole("button", { name: "Undo" }).click();
  await waitForAcceptedManagedSource(page, (source) => source === originalSource);
  await expect(page.getByRole("button", { name: "Redo" })).toBeEnabled();

  await page.getByRole("button", { name: "Redo" }).click();
  await waitForAcceptedManagedSource(page, (source) => source === upgradedSource);
  assertCleanRuntime();
});

test("real WASM source-backs a center-radius circle from an empty coded sketch", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  const assertCleanRuntime = auditRuntime(page);
  await boot(page);
  await page.getByRole("button", { name: "File menu" }).click();
  await page.getByRole("menuitem", { name: /Open/ }).click();
  await page.getByRole("button", { name: "Start from code" }).click();

  const source = page.locator(".cm-content");
  await expect(source).toContainText('import { sketch } from "@geosolve/sketch-code"');
  await page.getByRole("button", { name: "split", exact: true }).click();
  const canvas = page.getByRole("application");
  const frame = canvasFrame(page);
  const bounds = await canvas.boundingBox();
  expect(bounds).not.toBeNull();
  const click = async (x: number, y: number) => {
    await page.mouse.click(bounds!.x + bounds!.width * x, bounds!.y + bounds!.height * y);
  };

  await page.getByRole("button", { name: "Sketch", exact: true }).click();
  await page.getByRole("menuitem", { name: "Center–Radius" }).click();
  await click(0.42, 0.44);
  await expectDraft(canvasFrame(page), true);
  await click(0.58, 0.44);

  await expect(source).toContainText(
    'import { sketch, mm } from "@geosolve/sketch-code"',
  );
  await expect(source).toContainText(
    "const geometry1 = $.geometry.centerRadiusCircle",
  );
  await expect(source).toContainText("radius: mm(");
  await expectDraft(canvasFrame(page), false);
  await expectItemCount(drawItems(frame, { layer: "geometry", className: "wb-curve" }), 1);
  await expect(page.getByRole("button", { name: "Undo" })).toBeEnabled();

  await page.mouse.move(bounds!.x + bounds!.width * 0.70, bounds!.y + bounds!.height * 0.60);
  await expect(page.locator("header").first()).toContainText(/accepted/i);
  assertCleanRuntime();
});

test("two circle contacts publish their Segment and accept the next pointer gesture", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  const assertCleanRuntime = auditRuntime(page);
  await boot(page);
  await page.getByRole("button", { name: "File menu" }).click();
  await page.getByRole("menuitem", { name: /Open/ }).click();
  await page.getByRole("button", { name: "Start from code" }).click();

  const source = page.locator(".cm-content");
  await expect(source).toContainText(
    'import { sketch } from "@geosolve/sketch-code"',
  );
  await page.getByRole("button", { name: "split", exact: true }).click();
  const canvas = page.getByRole("application");
  const frame = canvasFrame(page);
  const bounds = await canvas.boundingBox();
  expect(bounds).not.toBeNull();
  const click = async (x: number, y: number) => {
    await page.mouse.click(bounds!.x + bounds!.width * x, bounds!.y + bounds!.height * y);
  };
  const readSource = async () =>
    `${(await source.locator(".cm-line").allTextContents()).join("\n")}\n`;
  const waitForPublishedSource = async (fragment: string) => {
    await expect.poll(readSource, { timeout: 30_000 }).toContain(fragment);
    await page.evaluate(() => new Promise<void>((resolve) => {
      const schedule = Reflect.get(globalThis, "requestAnimationFrame") as
        (callback: () => void) => number;
      schedule(resolve);
    }));
  };

  await page.getByRole("button", { name: "Sketch", exact: true }).click();
  await page.getByRole("menuitem", { name: "Center–Radius" }).click();
  await click(0.32, 0.47);
  await click(0.41, 0.47);
  await waitForPublishedSource("const geometry1 = $.geometry.centerRadiusCircle");

  await page.getByRole("button", { name: "Sketch", exact: true }).click();
  await page.getByRole("menuitem", { name: "Center–Radius" }).click();
  await click(0.65, 0.47);
  await click(0.74, 0.47);
  await waitForPublishedSource("const geometry2 = $.geometry.centerRadiusCircle");

  await page.getByRole("button", { name: "Sketch", exact: true }).click();
  await page.getByRole("menuitem", { name: "Segment" }).click();
  await click(0.41, 0.47);
  await expectDraft(canvasFrame(page), true);
  await click(0.56, 0.47);

  await waitForPublishedSource(
    "const segment3 = $.geometry.segment",
  );
  const publishedSource = await readSource();
  expect(publishedSource.match(/\$\.constraint\.pointOnCurve\(/gu)).toHaveLength(2);
  expect(publishedSource).toContain("curve: geometry1.span");
  expect(publishedSource).toContain("point: segment3.start");
  expect(publishedSource).toContain("curve: geometry2.span");
  expect(publishedSource).toContain("point: segment3.end");
  await expectDraft(canvasFrame(page), false);
  await expectItemCount(drawItems(frame, { layer: "geometry", className: "wb-curve" }), 3);
  await expect(page.locator("header").first()).toContainText(/accepted/i);

  await page.getByRole("button", { name: "Sketch", exact: true }).click();
  await page.getByRole("menuitem", { name: "Segment" }).click();
  await expect(page.getByText(
    /pointer input is unavailable while a managed-source mutation is compiling/u,
  )).toHaveCount(0);
  await click(0.35, 0.55);
  await expectDraft(canvasFrame(page), true);
  await expect(page.getByText(
    /pointer input is unavailable while a managed-source mutation is compiling/u,
  )).toHaveCount(0);
  await page.getByRole("button", { name: "Cancel" }).click();
  await expectDraft(canvasFrame(page), false);
  assertCleanRuntime();
});

test("canonical Jansen sample Polyline Finish publishes inferred constraints into sketch.ts", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  const assertCleanRuntime = auditRuntime(page);
  await boot(page);
  await openJansen(page);

  const source = page.locator(".cm-content");
  const originalSource = await waitForAcceptedManagedSource(
    page,
    (accepted) => accepted.includes("const groundPivot = $.geometry.sketchPoint"),
  );

  const canvas = page.getByRole("application");
  const frame = canvasFrame(page);
  const originalCurveCount = await drawItems(frame, { layer: "geometry", className: "wb-curve" }).count();
  const click = async (x: number, y: number) => {
    // Input uses the same logical view box and centred letterboxing as the canvas.
    const position = await fractionToClient(frame, { x, y });
    await page.mouse.click(position.x, position.y);
  };

  await page.getByRole("button", { name: "Sketch", exact: true }).click();
  await page.getByRole("menuitem", { name: "Polyline" }).click();
  const finish = page.getByRole("button", { name: "Finish" });
  await expect(finish).toBeDisabled();
  await click(0.35, 0.40);
  await click(0.60, 0.40);
  await expect(finish).toBeEnabled();
  await click(0.60, 0.65);
  await finish.click();

  const publishedSource = await waitForAcceptedManagedSource(
    page,
    (accepted) => accepted !== originalSource
      && accepted.includes("$.geometry.polyline")
      && accepted.includes("$.constraint.horizontal")
      && accepted.includes("$.constraint.vertical"),
  );
  expect(publishedSource).toContain('"use geosolve sketch"');
  const polyline = publishedSource.match(
    /const (geometry\d+) = \$\.geometry\.polyline\("\1", \{/u,
  );
  expect(polyline).not.toBeNull();
  const geometryName = polyline![1];
  const constraintDeclarations = Array.from(publishedSource.matchAll(
    /  const (constraint\d+) = \$\.constraint\.([A-Za-z][A-Za-z0-9]*)\("\1", \{([\s\S]*?)\n  \}\);/gu,
  ), ([, name, kind, body]) => ({ name, kind, body }));
  const horizontal = constraintDeclarations.find(({ kind, body }) =>
    kind === "horizontal" && body.includes(`span: ${geometryName}.segments.byKey.v0`)
  );
  const vertical = constraintDeclarations.find(({ kind, body }) =>
    kind === "vertical" && body.includes(`span: ${geometryName}.segments.byKey.v1`)
  );
  expect(horizontal).toBeDefined();
  expect(vertical).toBeDefined();
  expect(constraintDeclarations.length).toBeGreaterThanOrEqual(2);
  expect(constraintDeclarations.length).toBeLessThanOrEqual(3);
  const requiredConstraints = new Set([horizontal!.name, vertical!.name]);
  const inferredSnaps = constraintDeclarations.filter(({ name }) => !requiredConstraints.has(name));
  expect(inferredSnaps.every(({ kind }) => kind === "coincident" || kind === "pointOnCurve")).toBe(true);
  expect(inferredSnaps.every(({ body }) => body.includes(`${geometryName}.`))).toBe(true);
  expect(
    publishedSource.split("\n").length - originalSource.split("\n").length,
  ).toBeLessThanOrEqual(42);
  expect(
    new TextEncoder().encode(publishedSource).byteLength
      - new TextEncoder().encode(originalSource).byteLength,
  ).toBeLessThanOrEqual(2_300);

  const additions = page.getByRole("list", { name: "Canvas additions" });
  await expect(additions).toBeVisible();
  const geometry = additions.getByRole("button", { name: /^geometry\d+$/ });
  const constraints = additions.getByRole("button", { name: /^constraint\d+$/ });
  await expect(geometry).toHaveCount(1);
  await expect(constraints).toHaveCount(constraintDeclarations.length);

  const openAddedSource = async (declaration: typeof geometry, semanticName: RegExp) => {
    await declaration.click();
    const declarationName = await declaration.getAttribute("aria-label") ?? await declaration.textContent();
    expect(declarationName).not.toBeNull();
    const actions = additions.getByRole("group", { name: `${declarationName} actions`, exact: true });
    await expect(actions).toBeVisible();
    await actions.getByRole("button", { name: "Open source" }).click();
    await expect(page.getByRole("button", { name: "code", exact: true })).toHaveAttribute("aria-pressed", "true");
    await expect(source).toContainText(semanticName);
    const split = page.getByRole("button", { name: "split", exact: true });
    await split.click();
    await expect(split).toHaveAttribute("aria-pressed", "true");
    await expect(additions).toBeVisible();
  };
  await openAddedSource(
    additions.getByRole("button", { name: geometryName, exact: true }),
    new RegExp(`const ${geometryName} = \\$\\.geometry\\.polyline`, "u"),
  );
  for (const { name, kind } of constraintDeclarations) {
    await openAddedSource(
      additions.getByRole("button", { name, exact: true }),
      new RegExp(`const ${name} = \\$\\.constraint\\.${kind}`, "u"),
    );
  }

  await expect(page.locator("header").first()).toContainText(/accepted · r2/i);
  await expect(page.getByText("Accepted source", { exact: true })).toBeVisible();
  await expectDraft(canvasFrame(page), false);
  await expectItemCount(drawItems(frame, { layer: "geometry", className: "wb-curve" }), originalCurveCount + 2);
  await expect(page.getByRole("button", { name: "Undo" })).toBeEnabled();
  await expect(page.getByText(
    /pointer input is unavailable while a managed-source mutation is compiling/u,
  )).toHaveCount(0);
  assertCleanRuntime();
});

test("canonical scissor-lift point drags remain solver overlays and accept the next gesture", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  const assertCleanRuntime = auditRuntime(page);
  await boot(page);
  await openScissorLift(page);

  const originalSource = await waitForAcceptedManagedSource(
    page,
    (source) => source.includes("const point1TowerLevel0Left = $.geometry.sketchPoint"),
  );
  const frame = canvasFrame(page);
  const points = drawItems(frame, { layer: "points", kind: "circle", interactive: true });
  expect(await points.count()).toBeGreaterThan(0);
  const [minX, minY, width, height] = (await presentedFrame(frame)).viewBox;
  expect([minX, minY, width, height].every(Number.isFinite)).toBe(true);
  const center = [minX + width / 2, minY + height / 2];
  const centerIndex = (await points.all()).reduce((best, item, index) => {
    if (item.kind !== "circle") throw Error("Expected circle point");
    const distance = Math.hypot(item.center[0] - center[0], item.center[1] - center[1]);
    return distance < best.distance ? { index, distance } : best;
  }, { index: 0, distance: Number.POSITIVE_INFINITY }).index;
  const persistentId = (await points.nth(centerIndex).read()).metadata.persistentId;
  expect(persistentId).toBeTruthy();
  const point = drawItems(frame, { layer: "points", kind: "circle", persistentId });

  const dragBy = async (deltaX: number, deltaY: number) => {
    const before = await point.boundingBox();
    expect(before).not.toBeNull();
    const start = [before!.x + before!.width / 2, before!.y + before!.height / 2] as const;
    await page.mouse.move(...start);
    await page.mouse.down();
    await page.mouse.move(start[0] + deltaX, start[1] + deltaY, { steps: 5 });
    await page.mouse.up();
    await expect.poll(async () => {
      const after = await point.boundingBox();
      return after && [Math.round(after.x), Math.round(after.y)];
    }).not.toEqual([Math.round(before!.x), Math.round(before!.y)]);
  };

  await expect.poll(async () => savedProjectFingerprint(await readSavedProject(page))).not.toBeNull();
  let lastSaved = await readSavedProject(page);
  let lastSavedFingerprint = savedProjectFingerprint(lastSaved);
  for (const [deltaX, deltaY] of [[30, 20], [-18, 12], [16, -10], [-11, -14], [13, 9]] as const) {
    await dragBy(deltaX, deltaY);
    await expect(page.getByText(/pointer input is unavailable while a managed-source mutation is compiling/u)).toHaveCount(0);
    await expect(page.getByText(/QuotaExceededError/u)).toHaveCount(0);
    await expect.poll(async () => savedProjectFingerprint(await readSavedProject(page))).not.toBe(lastSavedFingerprint);
    lastSaved = await readSavedProject(page);
    lastSavedFingerprint = savedProjectFingerprint(lastSaved);
    await waitForAcceptedManagedSource(page, (source) => source === originalSource);
  }

  // Five retained scissor-lift drag snapshots exceed Chromium's former 5 MiB
  // localStorage path before v5 compression. Verify the actual decoded size;
  // successful compression must not make this history regression fail.
  const finalSaved = lastSaved;
  expect(finalSaved).not.toBeNull();
  const savedEnvelope = JSON.parse(finalSaved!) as { format?: unknown; project?: unknown };
  expect(savedEnvelope.format).toBe("geosolve-workbench-presentation-v1");
  expect(typeof savedEnvelope.project).toBe("string");
  const workbench = JSON.parse(savedEnvelope.project as string) as { version: string; session: string };
  expect(workbench.version).toBe("geosolve-code-workbench-v5");
  const capsule = workbench.session.split(":");
  expect(capsule).toHaveLength(5);
  expect(capsule.slice(0, 2)).toEqual(["GEOSOLVE_REPRO_V1", "zlib-base64url"]);
  const decoded = inflateSync(Buffer.from(capsule[4], "base64url"), { maxOutputLength: 64 * 1024 * 1024 });
  expect(decoded.byteLength).toBe(Number(capsule[2]));
  expect(decoded.byteLength).toBeGreaterThan(5 * 1024 * 1024);
  expect(await page.evaluate(() => localStorage.getItem("geosolve.project.v1"))).toBeNull();

  // Camera state is presentation-local. Normalize it exactly as the native
  // contact persistence test does before comparing presented coordinates across a
  // reload, then allow both retained camera frames to paint.
  await page.getByRole("button", { name: "Fit sketch" }).click();
  await page.evaluate(() => new Promise<void>((resolve) => {
    const schedule = Reflect.get(globalThis, "requestAnimationFrame") as
      (callback: () => void) => number;
    schedule(() => schedule(resolve));
  }));
  const finalPosition = await point.position();

  await page.reload({ waitUntil: "networkidle" });
  await expect(page.locator("header").getByText(SCISSOR_TITLE, { exact: true })).toBeVisible();
  const restoredPoint = drawItems(canvasFrame(page), { layer: "points", kind: "circle", persistentId });
  await expectItemCount(restoredPoint, 1);
  expect(await restoredPoint.position()).toEqual(finalPosition);
  expect(savedProjectFingerprint(await readSavedProject(page))).toBe(lastSavedFingerprint);
  await waitForAcceptedManagedSource(page, (source) => source === originalSource);
  const undo = page.getByRole("button", { name: "Undo" });
  await expect(undo).toBeEnabled();
  expect(await page.evaluate(() => localStorage.getItem("geosolve.project.v1"))).toBeNull();
  await expect(page.getByText(/QuotaExceededError/u)).toHaveCount(0);
  await undo.click();
  await expect.poll(async () => restoredPoint.position()).not.toEqual(finalPosition);
  assertCleanRuntime();
});

test("Cubic Bézier authoring publishes one named typed declaration", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  const assertCleanRuntime = auditRuntime(page);
  await boot(page);
  await openManifold(page);

  const originalSource = await waitForAcceptedManagedSource(
    page,
    (accepted) => accepted.includes("const reservoirWidth = $.dimension.curveLength"),
  );
  const canvas = page.getByRole("application");
  const frame = canvasFrame(page);
  const originalCurveCount = await drawItems(frame, { layer: "geometry", className: "wb-curve" }).count();
  const bounds = await canvas.boundingBox();
  expect(bounds).not.toBeNull();
  const click = async (x: number, y: number) => {
    await page.mouse.click(bounds!.x + bounds!.width * x, bounds!.y + bounds!.height * y);
  };

  await page.getByRole("button", { name: "Sketch", exact: true }).click();
  const cubic = page.getByRole("menuitem", { name: "Cubic", exact: true });
  await cubic.click();
  await expect(cubic).toBeHidden();
  await click(0.42, 0.55);
  await expectDraft(canvasFrame(page), true);
  await click(0.55, 0.40);
  await expectDraft(canvasFrame(page), true);
  await click(0.68, 0.40);
  await expectDraft(canvasFrame(page), true);
  await click(0.82, 0.55);

  const publishedSource = await waitForAcceptedManagedSource(
    page,
    (accepted) => accepted !== originalSource && accepted.includes("$.geometry.cubicBezier"),
  );
  const declarationMatch = publishedSource.match(
    /const (geometry\d+) = \$\.geometry\.cubicBezier\("\1", \{[\s\S]*?\n  \}\);/u,
  );
  const declaration = declarationMatch?.[0];
  expect(declaration).toBeDefined();
  expect(declaration).toContain("start: [");
  expect(declaration).toContain("firstControl: [");
  expect(declaration).toContain("secondControl: [");
  expect(declaration).toContain("end: [");
  expect(new TextEncoder().encode(declaration!).byteLength).toBeLessThanOrEqual(1_024);

  await expectDraft(canvasFrame(page), false);
  await expectItemCount(drawItems(frame, { layer: "geometry", className: "wb-curve" }), originalCurveCount + 1);
  await expect(page.getByRole("button", { name: "Undo" })).toBeEnabled();
  await expect(
    page.getByRole("list", { name: "Canvas additions" })
      .getByRole("button", { name: declarationMatch![1], exact: true }),
  ).toBeVisible();
  await expect(page.getByText("Accepted source", { exact: true })).toBeVisible();
  await expect(page.getByText(
    /pointer input is unavailable while a managed-source mutation is compiling/u,
  )).toHaveCount(0);
  assertCleanRuntime();
});

test("non-axis Parallel authoring publishes one named typed constraint", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  const assertCleanRuntime = auditRuntime(page);
  await boot(page);
  await openControlledCodeProject(page, `"use geosolve sketch";
import { sketch } from "@geosolve/sketch-code";

export default sketch(($) => {
  const first = $.geometry.segment("first", { start: [0, 0], end: [30, 15] });
  const second = $.geometry.segment("second", { start: [0, 35], end: [25, 48] });
  $.group("Lines", [first, second]);
  return { first, second };
});
`);

  const source = page.locator(".cm-content");
  const canvas = page.getByRole("application");
  const frame = canvasFrame(page);
  const curves = drawItems(frame, { layer: "geometry", className: "wb-curve" });
  await expectItemCount(curves, 2);
  const clickCurve = async (index: number) => {
    const bounds = await curves.nth(index).boundingBox();
    expect(bounds).not.toBeNull();
    await page.mouse.click(bounds!.x + bounds!.width / 2, bounds!.y + bounds!.height / 2);
  };
  const beforeConstraint = await source.textContent();

  await page.getByRole("button", { name: "Constraint" }).click();
  await page.getByRole("menuitem", { name: "Parallel" }).click();
  await clickCurve(0);
  await clickCurve(1);

  await expect.poll(() => source.textContent(), { timeout: 30_000 }).not.toBe(beforeConstraint);
  const publishedSource = `${(await source.locator(".cm-line").allTextContents()).join("\n")}\n`;
  expect(publishedSource).toContain("$.constraint.parallel");
  await expectDraft(canvasFrame(page), false);
  await expect(page.getByRole("list", { name: "Canvas additions" }).getByRole("button", { name: /^constraint\d+$/ })).toHaveCount(1);
  assertCleanRuntime();
});

test("computed Fillet authoring publishes one direct semantic FilletSet declaration", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  const assertCleanRuntime = auditRuntime(page);
  await boot(page);
  await openControlledCodeProject(page, `"use geosolve sketch";
import { sketch } from "@geosolve/sketch-code";

export default sketch(($) => {
  const first = $.geometry.segment("first", { start: [-30, 0], end: [0, 0] });
  const second = $.geometry.segment("second", { start: first.end, end: [0, 30] });
  $.group("Corner", [first, second]);
  return { first, second };
});
`);

  const source = page.locator(".cm-content");
  const originalSource = await source.textContent();
  const canvas = page.getByRole("application");
  const frame = canvasFrame(page);
  const curves = drawItems(frame, { layer: "geometry", className: "wb-curve" });
  await expectItemCount(curves, 2);
  const clickCurve = async (index: number) => {
    const bounds = await curves.nth(index).boundingBox();
    expect(bounds).not.toBeNull();
    await page.mouse.click(bounds!.x + bounds!.width / 2, bounds!.y + bounds!.height / 2);
  };

  await page.getByRole("button", { name: "Modify" }).click();
  await page.getByRole("menuitem", { name: "Fillet" }).click();
  await clickCurve(0);
  await clickCurve(1);
  const finishFillet = page.getByRole("button", { name: "Finish" });
  await expect(finishFillet).toBeEnabled();
  await finishFillet.click();

  await expect.poll(() => source.textContent(), { timeout: 30_000 }).not.toBe(originalSource);
  const publishedSource = `${(await source.locator(".cm-line").allTextContents()).join("\n")}\n`;
  expect(publishedSource).toContain("$.computed.filletSet");
  const declaration = publishedSource.match(
    /const ([A-Za-z_$][A-Za-z0-9_$]*) = \$\.computed\.filletSet\("([^"]+)", \{/u,
  );
  expect(declaration).not.toBeNull();
  expect(declaration![1]).toBe(declaration![2]);
  const declarationName = declaration![1];
  const filletLabel = publishedSource.match(/label: "(Fillet \d+)"/u)?.[1];
  expect(filletLabel).toBeDefined();
  await expectDraft(canvasFrame(page), false);
  const computedFillets = drawItems(frame, { layer: "computed", className: "wb-computed-fillet" });
  await expectItemCount(computedFillets, 1);
  const originalPath = await computedFillets.first().geometryKey();
  expect(originalPath).not.toBeNull();

  const additions = page.getByRole("list", { name: "Canvas additions" });
  const row = additions.getByRole("button", { name: declarationName, exact: true });
  await expect(row).toHaveCount(1);
  await row.click();
  await page.getByRole("tab", { name: "Inspector" }).click();
  const details = page.getByRole("tabpanel");
  await expect(details.getByRole("heading", { name: filletLabel!, exact: true })).toBeVisible();
  await expect(details.getByText("Modifiable in source", { exact: true })).toBeVisible();

  await page.getByRole("tab", { name: "Parameters" }).click();
  const editableParameters = details.locator(
    `input[aria-label^="${declarationName} · "]:not([disabled])`,
  );
  await expect(editableParameters).not.toHaveCount(0);
  let radiusInput = editableParameters.first();
  for (let index = 0; index < await editableParameters.count(); index += 1) {
    const candidate = editableParameters.nth(index);
    if ((await candidate.getAttribute("aria-label"))?.endsWith(" · radius")) {
      radiusInput = candidate;
      break;
    }
  }
  const radiusBefore = Number(await radiusInput.inputValue());
  expect(radiusBefore).toBeGreaterThan(0);
  const radiusAfter = radiusBefore / 2;
  const revisionBeforeRadiusEdit = await page.locator("header").first().textContent();
  const sourceBeforeRadiusEdit = await source.textContent();
  await radiusInput.fill(String(radiusAfter));
  await radiusInput.press("Enter");
  await expect.poll(() => page.locator("header").first().textContent()).not.toBe(revisionBeforeRadiusEdit);
  await expect.poll(() => source.textContent()).not.toBe(sourceBeforeRadiusEdit);
  await expectItemCount(computedFillets, 1);
  await expect.poll(() => computedFillets.first().geometryKey()).not.toBe(originalPath);
  const editedPath = await computedFillets.first().geometryKey();
  const sourceAfterRadiusEdit = `${(await source.locator(".cm-line").allTextContents()).join("\n")}\n`;
  expect(sourceAfterRadiusEdit).toContain(`label: "${filletLabel}"`);

  await page.getByRole("tab", { name: "Inspector" }).click();
  await expect(details.getByRole("heading", { name: filletLabel!, exact: true })).toBeVisible();
  await expect(details.getByText("Modifiable in source", { exact: true })).toBeVisible();
  const actions = additions.getByRole("group", {
    name: `${declarationName} actions`,
    exact: true,
  });
  await expect(actions).toBeVisible();
  await actions.getByRole("button", { name: "Suppress" }).click();
  await expect.poll(() => source.textContent()).toContain(`$.suppress(${declarationName})`);
  await expectItemCount(computedFillets, 0);
  await row.click();
  await expect(actions.getByRole("button", { name: "Restore" })).toBeVisible();

  await actions.getByRole("button", { name: "Restore" }).click();
  await expect.poll(() => source.textContent()).not.toContain(`$.suppress(${declarationName})`);
  await expectItemCount(computedFillets, 1);
  await expect.poll(() => computedFillets.first().geometryKey()).toBe(editedPath);
  await expect(row).toHaveCount(1);

  await row.click();
  const sourceBeforeDelete = `${(await source.locator(".cm-line").allTextContents()).join("\n")}\n`;
  await actions.getByRole("button", { name: "Delete" }).click();
  await expect.poll(() => source.textContent()).not.toContain(`const ${declarationName} =`);
  await expect(row).toHaveCount(0);
  await expectItemCount(computedFillets, 0);

  await page.getByRole("button", { name: "Undo" }).click();
  await expect.poll(async () => `${(await source.locator(".cm-line").allTextContents()).join("\n")}\n`)
    .toBe(sourceBeforeDelete);
  await expect(row).toHaveCount(1);
  await expectItemCount(computedFillets, 1);
  await expect.poll(() => computedFillets.first().geometryKey()).toBe(editedPath);
  assertCleanRuntime();
});

test("a downloaded reproduction imports atomically through the real bridge", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  const assertCleanRuntime = auditRuntime(page);
  await boot(page);
  await openManifold(page);

  await page.getByRole("button", { name: "Diagnostics" }).click();
  const downloadPromise = page.waitForEvent("download");
  await page.getByRole("menuitem", { name: "Download reproduction" }).click();
  const download = await downloadPromise;
  expect(download.suggestedFilename()).toBe("geosolve-reproduction.txt");
  const path = await download.path();
  expect(path).not.toBeNull();
  const reproduction = await readFile(path!, "utf8");
  expect(reproduction).toMatch(/^GEOSOLVE_REPRO_V1:/);

  await page.getByRole("button", { name: "File menu" }).click();
  await page.getByRole("menuitem", { name: /Open/ }).click();
  await page.getByRole("button", { name: "New sketch" }).click();
  await expect(page.locator("header").getByText("Untitled sketch", { exact: true })).toBeVisible();

  await page.getByRole("button", { name: "File menu" }).click();
  const chooserPromise = page.waitForEvent("filechooser");
  await page.getByRole("menuitem", { name: "Import project or repro…" }).click();
  const chooser = await chooserPromise;
  await chooser.setFiles({ name: "geosolve-reproduction.txt", mimeType: "text/plain", buffer: Buffer.from(reproduction) });
  await expect(page.locator("header").getByText(MANIFOLD_TITLE, { exact: true })).toBeVisible();
  await expect(page.locator(".cm-content")).toContainText("const reservoirWidth = $.dimension.curveLength");
  expect((await presentedFrame(canvasFrame(page))).items.some((item) => item.layer === "points" && item.style.strokeWidth > 0)).toBe(true);
  assertCleanRuntime();
});

test("the supplied native contact workspace retains a constrained point drag", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  const assertCleanRuntime = auditRuntime(page);
  const reproduction = await readFile(
    new URL("../../../tests/fixtures/m90_f005_native_drag_repro.txt", import.meta.url),
    "utf8",
  );
  expect(reproduction).toMatch(/^GEOSOLVE_REPRO_V1:/);

  await boot(page);
  await page.getByRole("button", { name: "File menu" }).click();
  const chooserPromise = page.waitForEvent("filechooser");
  await page.getByRole("menuitem", { name: "Import project or repro…" }).click();
  const chooser = await chooserPromise;
  await chooser.setFiles({
    name: "m90-f005-native-drag-repro.txt",
    mimeType: "text/plain",
    buffer: Buffer.from(reproduction),
  });
  await expect(page.locator("header").getByText("Restored sketch", { exact: true })).toBeVisible();

  const persistentId = "7b80e0003fe358f731b6e557427a0673";
  const point = drawItems(canvasFrame(page), { layer: "points", kind: "circle", persistentId });
  await expectItemCount(point, 1);
  const originalPosition = await point.position();
  const before = await point.boundingBox();
  expect(before).not.toBeNull();
  const start = [before!.x + before!.width / 2, before!.y + before!.height / 2] as const;
  const target = [start[0] + 20, start[1] - 15] as const;

  const initialStyle = (await point.read()).style;
  await page.mouse.move(...start);
  await expect.poll(async () => (await point.read()).style).not.toEqual(initialStyle);
  await page.mouse.down();
  await page.mouse.move(...target, { steps: 5 });
  await expect.poll(async () => point.position()).not.toEqual(originalPosition);
  const terminalPreview = await point.position();
  const savedBefore = savedProjectFingerprint(await readSavedProject(page));
  await page.mouse.up();

  await expect(page.locator("header").first()).toContainText(/accepted · r1/i);
  await expect(page.getByText(/candidate evaluation rejected and policy requires accepted publication/u)).toHaveCount(0);
  await expect.poll(async () => point.position()).toEqual(terminalPreview);
  await expect.poll(async () => savedProjectFingerprint(await readSavedProject(page))).not.toBe(savedBefore);
  const savedAfter = savedProjectFingerprint(await readSavedProject(page));

  // The camera is presentation-local rather than project authority. Fit once
  // before reload so both renderings use the same deterministic camera instead
  // of comparing the drag-time camera with the restored document's cold fit.
  await page.getByRole("button", { name: "Fit sketch" }).click();
  await page.evaluate(() => new Promise<void>((resolve) => {
    const schedule = Reflect.get(globalThis, "requestAnimationFrame") as
      (callback: () => void) => number;
    schedule(() => schedule(resolve));
  }));
  const fittedTerminal = await point.position();

  await page.reload({ waitUntil: "networkidle" });
  await expect(page.locator("header").getByText("Restored sketch", { exact: true })).toBeVisible();
  const restoredPoint = drawItems(canvasFrame(page), { layer: "points", kind: "circle", persistentId });
  await expectItemCount(restoredPoint, 1);
  expect(await restoredPoint.position()).toEqual(fittedTerminal);
  expect(savedProjectFingerprint(await readSavedProject(page))).toBe(savedAfter);

  const undo = page.getByRole("button", { name: "Undo" });
  await expect(undo).toBeEnabled();
  await undo.click();
  await expect.poll(async () => restoredPoint.position()).not.toEqual(fittedTerminal);
  await page.getByRole("button", { name: "Fit sketch" }).click();
  await expect.poll(async () => restoredPoint.position()).toEqual(originalPosition);
  assertCleanRuntime();
});

test("outside dismissal preserves the destination click", async ({ page }) => {
  const assertCleanRuntime = auditRuntime(page);
  await boot(page);
  await page.getByRole("button", { name: "Sketch", exact: true }).click();
  await expect(page.getByRole("menuitem", { name: "Segment" })).toBeVisible();
  await page.getByRole("button", { name: "code", exact: true }).click();
  await expect(page.getByRole("menuitem", { name: "Segment" })).toBeHidden();
  await expect(page.getByRole("navigation", { name: "Primary tools" })).toBeHidden();
  await expect(page.getByRole("button", { name: "code", exact: true })).toHaveAttribute("aria-pressed", "true");
  assertCleanRuntime();
});

test("canvas action feedback overlays the workspace without shifting it", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  const assertCleanRuntime = auditRuntime(page);
  await boot(page);
  await openManifold(page);

  const canvas = page.getByRole("application");
  const before = await canvas.boundingBox();
  expect(before).not.toBeNull();

  await page.locator(".cm-content").click();
  await page.keyboard.press("Control+End");
  await page.keyboard.insertText("\n// local draft");
  await page.getByRole("button", { name: "File menu" }).click();
  await page.getByRole("menuitem", { name: "Export canonical project…" }).click();
  await expect(page.getByRole("alert")).toContainText("Apply or Revert the current source draft");

  const after = await canvas.boundingBox();
  expect(after).toEqual(before);
  assertCleanRuntime();
});

test("Rust-owned CAD icons and semantic groups drive the toolbar while view controls stay on canvas", async ({ page }) => {
  const assertCleanRuntime = auditRuntime(page);
  await boot(page);
  await expect(page.getByRole("navigation", { name: "Primary tools" }).locator('svg[data-icon-key="geometry-select"]')).toBeVisible();

  await page.getByRole("button", { name: "Sketch", exact: true }).click();
  await expect(page.getByRole("menuitem")).toHaveCount(25);
  await expect(page.getByRole("heading", { name: "Rectangles" })).toBeVisible();
  await expect(page.getByRole("menuitem", { name: "Segment" }).locator('svg[data-icon-key="geometry-segment"]')).toBeVisible();

  await page.getByRole("button", { name: "Constraint" }).click();
  await expect(page.getByRole("menuitem")).toHaveCount(13);
  await expect(page.getByRole("heading", { name: "Curve join" })).toBeVisible();
  await expect(page.getByRole("menuitem", { name: "Coincident" }).locator('svg[data-icon-key="coincident"]')).toBeVisible();

  await page.getByRole("button", { name: "Dimension" }).click();
  await expect(page.getByRole("menuitem")).toHaveCount(5);
  await expect(page.getByRole("menuitem", { name: "Radius" }).locator('svg[data-icon-key="radius"]')).toBeVisible();

  await page.getByRole("button", { name: "Modify" }).click();
  await expect(page.getByRole("menuitem")).toHaveCount(2);
  await expect(page.getByRole("menuitem", { name: "Fillet" }).locator('svg[data-icon-key="feature-fillet"]')).toBeVisible();
  await expect(page.getByRole("menuitem", { name: "Offset" }).locator('svg[data-icon-key="modify-offset"]')).toBeVisible();
  await expect(page.getByRole("menuitem", { name: /grid|fit|origin/i })).toHaveCount(0);

  await page.getByRole("button", { name: "Sketch", exact: true }).click();
  await page.getByRole("menuitem", { name: "Segment" }).click();
  await expect(page.locator("span.font-medium", { hasText: "Segment" })).toBeVisible();
  await page.getByRole("button", { name: "Hide grid" }).click();
  await expect(page.getByRole("button", { name: "Show grid" })).toBeVisible();
  await expect(page.locator("span.font-medium", { hasText: "Segment" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Sketch", exact: true })).toHaveAttribute("aria-pressed", "true");
  await expect(page.getByRole("toolbar", { name: "Canvas view" }).getByRole("button")).toHaveCount(3);
  assertCleanRuntime();
});

test("normal pointer capture release commits Circle geometry and edits its compact radius", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  const assertCleanRuntime = auditRuntime(page);
  await boot(page);
  await openManifold(page);
  const originalSource = await waitForAcceptedManagedSource(
    page,
    (source) => source.includes("const reservoirWidth = $.dimension.curveLength"),
  );

  const canvas = page.getByRole("application");
  const frame = canvasFrame(page);
  const originalCurveCount = await drawItems(frame, { layer: "geometry", className: "wb-curve" }).count();
  const originalPointCount = await drawItems(frame, { layer: "points", kind: "circle" }).count();
  const click = async (x: number, y: number) => {
    const bounds = await canvas.boundingBox();
    expect(bounds).not.toBeNull();
    await page.mouse.click(bounds!.x + bounds!.width * x, bounds!.y + bounds!.height * y);
  };

  await page.getByRole("button", { name: "Sketch", exact: true }).click();
  await page.getByRole("menuitem", { name: "Center–Radius" }).click();
  await expect(page.locator("span.font-medium", { hasText: "Center–Radius" })).toBeVisible();
  await click(0.42, 0.55);
  await expectDraft(canvasFrame(page), true);
  await click(0.52, 0.55);
  const circleSource = await waitForAcceptedManagedSource(
    page,
    (source) => source !== originalSource && source.includes("$.geometry.centerRadiusCircle"),
  );
  await expect(page.locator("header").first()).toContainText("r2");
  await expectItemCount(drawItems(frame, { layer: "geometry", className: "wb-curve" }), originalCurveCount + 1);
  await expectItemCount(drawItems(frame, { layer: "points", kind: "circle" }), originalPointCount + 1);
  await expectDraft(canvasFrame(page), false);
  await expect(page.locator("span.font-medium", { hasText: "Center–Radius" })).toBeVisible();
  await expect(page.getByText(
    /pointer input is unavailable while a managed-source mutation is compiling/u,
  )).toHaveCount(0);

  const additions = page.getByRole("list", { name: "Canvas additions" });
  const compactCircle = additions.getByRole("button", { name: /^geometry\d+$/ });
  await expect(compactCircle).toHaveCount(1);
  await compactCircle.click();
  const circleName = (await compactCircle.textContent())?.trim() ?? null;
  expect(circleName).not.toBeNull();
  await page.getByRole("tab", { name: "Parameters" }).click();
  const circleParameters = page.getByRole("tabpanel").locator(`input[aria-label^="${circleName} · "]:not([disabled])`);
  await expect(circleParameters).not.toHaveCount(0);
  let radiusInput = circleParameters.first();
  for (let index = 0; index < await circleParameters.count(); index += 1) {
    const candidate = circleParameters.nth(index);
    if (Number(await candidate.inputValue()) > 0) {
      radiusInput = candidate;
      break;
    }
  }
  const radiusBefore = Number(await radiusInput.inputValue());
  expect(radiusBefore).toBeGreaterThan(0);
  const radiusAfter = radiusBefore + 1;
  await radiusInput.fill(String(radiusAfter));
  await radiusInput.press("Enter");
  await waitForAcceptedManagedSource(
    page,
    (source) => source !== circleSource
      && source.includes(`const ${circleName} = $.geometry.centerRadiusCircle`)
      && source.includes(`radius: mm(${radiusAfter})`),
  );
  await expect(page.locator("header").first()).toContainText("r3");
  const refreshedEditableParameters = page.getByRole("tabpanel").locator(`input[aria-label^="${circleName} · "]:not([disabled])`);
  await expect.poll(async () => {
    const values = await refreshedEditableParameters.evaluateAll((inputs) =>
      inputs.map((input) => Reflect.get(input, "value"))
    );
    return values.includes(String(radiusAfter));
  }).toBe(true);
  assertCleanRuntime();
});

test("canvas chrome exposes real role state and exact Polyline Finish readiness", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  const assertCleanRuntime = auditRuntime(page);
  await boot(page);
  await openNewSketch(page);

  await expect(page.getByRole("button", { name: /New curves:/ })).toHaveCount(0);
  await page.getByRole("button", { name: "Sketch", exact: true }).click();
  await page.getByRole("menuitem", { name: "Polyline" }).click();

  const role = page.getByRole("button", { name: "New curves: Profile. Change to Construction" });
  await expect(role).toHaveAttribute("aria-pressed", "false");
  await role.click();
  await expect(page.getByRole("button", { name: "New curves: Construction. Change to Profile" })).toHaveAttribute("aria-pressed", "true");

  const finish = page.getByRole("button", { name: "Finish" });
  await expect(finish).toBeDisabled();
  const canvas = page.getByRole("application");
  const bounds = await canvas.boundingBox();
  expect(bounds).not.toBeNull();
  const click = async (x: number, y: number) => {
    await page.mouse.click(bounds!.x + bounds!.width * x, bounds!.y + bounds!.height * y);
  };

  await page.mouse.click(bounds!.x + bounds!.width * 0.48, bounds!.y + bounds!.height * 0.44, { button: "right" });
  await expect(finish).toBeDisabled();
  await expectDraft(canvasFrame(page), false);
  await click(0.36, 0.44);
  await expect(finish).toBeDisabled();
  await click(0.62, 0.44);
  await expect(finish).toBeEnabled();
  await finish.click();
  await expect(finish).toBeDisabled();
  await expectDraft(canvasFrame(page), false);
  await expectItemCount(drawItems(canvasFrame(page), { layer: "geometry", className: "wb-curve" }), 1);
  assertCleanRuntime();
});
