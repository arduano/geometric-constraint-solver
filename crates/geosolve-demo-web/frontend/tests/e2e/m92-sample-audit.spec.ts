// SPDX-License-Identifier: GPL-3.0-or-later
import { mkdir, readFile, readdir, writeFile } from "node:fs/promises";
import { join, resolve } from "node:path";
import { expect, test, type Page, type TestInfo } from "@playwright/test";

interface Edit {
  declaration: string;
  path: Array<string | number>;
  replacement: number | { unit: string; value: number };
}

const sampleRoot = resolve(import.meta.dirname, "../../../../geosolve-sketch-code/assets/bundled-samples");
const directories = (await readdir(sampleRoot, { withFileTypes: true }))
  .filter((entry) => entry.isDirectory()).map((entry) => entry.name);
const samples = await Promise.all(directories.map(async (key) => ({
  key,
  manifest: JSON.parse(await readFile(join(sampleRoot, key, "manifest.json"), "utf8")) as { ordinal: number; title: string },
  witnesses: JSON.parse(await readFile(join(sampleRoot, key, "witnesses.json"), "utf8")) as {
    representative_edit: Edit; secondary_edit: Edit;
  },
})));
samples.sort((a, b) => a.manifest.ordinal - b.manifest.ordinal);
expect(samples).toHaveLength(20);

const savedWorkspaceExpression = `(async () => {
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
  } finally { database.close(); }
})()`;

async function savedWorkspace(page: Page): Promise<string | null> {
  return page.evaluate<string | null>(savedWorkspaceExpression);
}

async function acceptedSource(page: Page): Promise<string | null> {
  // Read the same persisted authority in the browser, returning only source.
  // Repeatedly copying multi-megabyte session strings through the debugging
  // transport measures the test runner rather than the application workflow.
  return page.evaluate<string | null>(`(async () => {
    const wire = await ${savedWorkspaceExpression};
    if (wire === null) return null;
    const presentation = JSON.parse(wire);
    const workbench = JSON.parse(presentation.project);
    const project = JSON.parse(workbench.project);
    return project.managed?.source ?? null;
  })()`);
}

// Compare the actual painted geometry at a deterministic fitted camera. Ignore
// selection styling and allocator IDs, while retaining every native/computed path
// and point. Source equality alone cannot detect stale accepted presentation.
async function fittedGeometry(page: Page): Promise<string> {
  await page.getByRole("button", { name: "Fit sketch" }).click();
  await page.evaluate(() => new Promise<void>((resolve) => {
    const schedule = Reflect.get(globalThis, "requestAnimationFrame") as (callback: () => void) => number;
    schedule(() => schedule(resolve));
  }));
  return page.locator('[role="application"] svg.geosolve-authoritative-frame').evaluate((root) => {
    type Bounds = { x: number; y: number; width: number; height: number };
    type Graphic = { getBBox(): Bounds; getAttribute(name: string): string | null; outerHTML: string; tagName: string };
    const svg = root as unknown as {
      viewBox: { baseVal: Bounds };
      querySelectorAll(selector: string): Iterable<Graphic>;
      getAttribute(name: string): string | null;
    };
    const elements = [...svg.querySelectorAll(
      '.wb-geometry path, .wb-computed-geometry path, .wb-points > circle.wb-point',
    )];
    if (elements.length === 0) throw new Error("accepted geometry is empty");
    const bounds = svg.viewBox.baseVal;
    const tokens = elements.map((element) => {
      const box = element.getBBox();
      if (![box.x, box.y, box.width, box.height].every(Number.isFinite)) {
        throw new Error("accepted geometry has nonfinite bounds");
      }
      // Fit must keep all accepted profile/construction geometry inside the frame.
      if (box.x < bounds.x - 2 || box.y < bounds.y - 2
        || box.x + box.width > bounds.x + bounds.width + 2
        || box.y + box.height > bounds.y + bounds.height + 2) {
        throw new Error(`fitted geometry is clipped: ${element.outerHTML.slice(0, 240)}`);
      }
      const attributes = ["d", "cx", "cy", "r", "transform"].map((name) => element.getAttribute(name));
      if (attributes.some((value) => value !== null && /NaN|Infinity/.test(value))) {
        throw new Error("accepted geometry contains nonfinite coordinates");
      }
      return [element.tagName, ...attributes];
    });
    return JSON.stringify({ viewBox: svg.getAttribute("viewBox"), tokens });
  });
}

async function capture(page: Page, info: TestInfo, key: string, stage: string) {
  const target = process.env.M92_BROWSER_AUDIT_OUTPUT
    ? join(process.env.M92_BROWSER_AUDIT_OUTPUT, key) : info.outputPath("audit");
  await mkdir(target, { recursive: true });
  await writeFile(join(target, `${stage}.geometry.json`), await fittedGeometry(page));
  await page.screenshot({ path: join(target, `${stage}.png`), fullPage: true });
  if (process.env.M92_BROWSER_PRESERVE_WORKSPACE === "1") {
    await writeFile(join(target, `${stage}.workspace.json`), (await savedWorkspace(page)) ?? "");
  }
  const frame = page.locator('[role="application"] svg.geosolve-authoritative-frame');
  await writeFile(join(target, `${stage}.svg`), await frame.evaluate((svg) => svg.outerHTML));
  await writeFile(join(target, `${stage}.ts`), (await acceptedSource(page)) ?? "");
}

function controlLabel(edit: Edit): string {
  return edit.path.length ? `${edit.declaration} · ${edit.path.join(".")}` : edit.declaration;
}

for (const { key, manifest, witnesses } of samples) {
  test(`M92 visual workflow ${manifest.ordinal}: ${key}`, async ({ page }, info) => {
    test.setTimeout(360_000);
    await page.setViewportSize({ width: 1440, height: 900 });
    const errors: string[] = [];
    page.on("pageerror", (error) => errors.push(error.message));
    page.on("console", (message) => { if (message.type() === "error") errors.push(message.text()); });
    await page.goto("/", { waitUntil: "networkidle" });
    await page.getByRole("button", { name: "File menu" }).click();
    await page.getByRole("menuitem", { name: /Open/ }).click();
    await page.getByPlaceholder("Search 20 samples…").fill(manifest.title);
    await page.getByRole("button").filter({ has: page.getByText(manifest.title, { exact: true }) }).click();
    await expect(page.locator("header").getByText(manifest.title, { exact: true })).toBeVisible();
    await expect.poll(() => acceptedSource(page), { timeout: 60_000 }).not.toBeNull();
    const original = await acceptedSource(page);
    await page.getByRole("button", { name: "design", exact: true }).click();
    await page.getByRole("button", { name: "Fit sketch" }).click();
    const frame = page.locator('[role="application"] svg.geosolve-authoritative-frame');
    await expect(frame.locator(".wb-accepted-scene .wb-geometry")).toHaveCount(1);
    const points = frame.locator('.wb-accepted-scene .wb-points > circle.wb-point[data-interactive="true"]');
    expect(await points.count()).toBeGreaterThan(0);
    // Ordinary point selection, with no drag, must expose accepted ownership in the Inspector.
    const box = await points.first().boundingBox();
    expect(box).not.toBeNull();
    await page.mouse.click(box!.x + box!.width / 2, box!.y + box!.height / 2);
    await expect(page.getByRole("tabpanel").getByText("Ownership", { exact: true })).toBeVisible();
    const baselineGeometry = await fittedGeometry(page);
    // Group isolation is presentation-only, and restoring it must restore every
    // accepted path without creating a design-history entry.
    await page.getByRole("button", { name: /^Isolate / }).first().click();
    await expect(page.getByRole("button", { name: "Restore visibility before isolate" })).toBeEnabled();
    expect(await fittedGeometry(page)).not.toBe(baselineGeometry);
    expect(await acceptedSource(page)).toBe(original);
    await page.getByRole("button", { name: "Restore visibility before isolate" }).click();
    expect(await fittedGeometry(page)).toBe(baselineGeometry);
    const explorer = page.getByRole("complementary", { name: "Explorer", exact: true });
    const horizontalScroll = await explorer.evaluate((root) => [...root.querySelectorAll("*")]
      .map((element) => ({ left: element.scrollLeft, overflow: element.scrollWidth - element.clientWidth }))
      .filter(({ left }) => left !== 0));
    expect(horizontalScroll, "group actions must not scroll declaration labels out of view").toEqual([]);
    await capture(page, info, key, "baseline");
    expect(witnesses.secondary_edit, `${key} requires a distinct measured second parameter`).toBeDefined();
    expect(controlLabel(witnesses.secondary_edit)).not.toBe(controlLabel(witnesses.representative_edit));
    for (const [index, edit] of [witnesses.representative_edit, witnesses.secondary_edit].entries()) {
      if (index > 0) {
        await page.getByRole("button", { name: "Undo", exact: true }).click();
        await expect.poll(() => acceptedSource(page), { timeout: 60_000 }).toBe(original);
      }
      await page.getByRole("tab", { name: "Parameters", exact: true }).click();
      const control = page.getByRole("textbox", { name: controlLabel(edit), exact: true });
      const value = String(typeof edit.replacement === "number" ? edit.replacement : edit.replacement.value);
      const beforeValue = await control.inputValue();
      expect(beforeValue).not.toBe(value);
      await control.fill(value);
      await control.press("Enter");
      await expect.poll(() => acceptedSource(page), { timeout: 60_000 }).not.toBe(original);
      await expect(control).toHaveValue(value);
      const edited = await acceptedSource(page);
      const editedGeometry = await fittedGeometry(page);
      expect(editedGeometry).not.toBe(baselineGeometry);
      await capture(page, info, key, `edit-${index + 1}`);
      await page.getByRole("button", { name: "Undo", exact: true }).click();
      await expect.poll(() => acceptedSource(page), { timeout: 60_000 }).toBe(original);
      await expect(control).toHaveValue(beforeValue);
      expect(await fittedGeometry(page)).toBe(baselineGeometry);
      await capture(page, info, key, `undo-${index + 1}`);
      await page.getByRole("button", { name: "Redo", exact: true }).click();
      await expect.poll(() => acceptedSource(page), { timeout: 60_000 }).toBe(edited);
      await expect(control).toHaveValue(value);
      expect(await fittedGeometry(page)).toBe(editedGeometry);
      await capture(page, info, key, `redo-${index + 1}`);
      await page.reload({ waitUntil: "networkidle" });
      await expect(page.locator("header").getByText(manifest.title, { exact: true })).toBeVisible();
      await expect.poll(() => acceptedSource(page), { timeout: 60_000 }).toBe(edited);
      await page.getByRole("tab", { name: "Parameters", exact: true }).click();
      await expect(page.getByRole("textbox", { name: controlLabel(edit), exact: true })).toHaveValue(value);
      await expect(frame.locator(".wb-accepted-scene .wb-geometry")).toHaveCount(1);
      expect(await fittedGeometry(page)).toBe(editedGeometry);
      await capture(page, info, key, `reload-${index + 1}`);
    }
    expect(errors).toEqual([]);
  });
}
