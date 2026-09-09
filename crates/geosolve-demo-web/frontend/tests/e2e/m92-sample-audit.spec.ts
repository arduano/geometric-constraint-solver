// SPDX-License-Identifier: GPL-3.0-or-later
import { mkdir, writeFile } from "node:fs/promises";
import { authoredLabel } from "./source-presentation";
import { join } from "node:path";
import { expect, test, type Page, type TestInfo } from "@playwright/test";

import { compareFittedGeometry } from "../fitted-geometry";
import { canvasFrame, drawItems, expectItemCount, logicalToClient, presentedFrame, settlePresentation } from "./presented-canvas";
import { acceptedSource, fittedGeometry, openSamplePrefix, samples, savedWorkspace, type Edit } from "./release-sample-prefix";

function expectGeometry(actual: string, expected: string, shouldMatch = true) {
  const comparison = compareFittedGeometry(actual, expected);
  expect(comparison.equal, comparison.difference ?? "geometry is unchanged within 1e-9 logical px").toBe(shouldMatch);
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
  const frame = canvasFrame(page);
  await writeFile(join(target, `${stage}.scene.json`), JSON.stringify(await presentedFrame(frame)));
  await writeFile(join(target, `${stage}.ts`), (await acceptedSource(page)) ?? "");
}

function controlLabel(edit: Edit, source: string): string {
  return edit.path.length ? `${edit.declaration} · ${edit.path.join(".")}` : authoredLabel(source, edit.declaration);
}

async function requireJansenDrag(page: Page, info: TestInfo, originalSource: string, baseline: string) {
  const frame = canvasFrame(page);
  const points = drawItems(frame, { layer: "points", kind: "circle" });
  await expectItemCount(points, 8);
  const geometry = (await points.all()).map((item) => {
    if (item.kind !== "circle") throw Error("Expected presented point circle");
    return { id: item.metadata.persistentId, x: item.center[0], y: item.center[1] };
  });
  // The authored crank starts at [15, 0], to the right of O=[0, 0]
  // and every leg joint. Use its two actual painted endpoints to calibrate
  // the model-to-screen transform, without a private bridge test hook.
  const [crank, ground] = [...geometry].sort((a, b) => b.x - a.x);
  const foot = [...geometry].sort((a, b) => b.y - a.y)[0];
  expect(Math.abs(crank.y - ground.y)).toBeLessThan(0.002);
  const scale = (crank.x - ground.x) / 15;
  expect(scale).toBeGreaterThan(0);
  const input = drawItems(frame, { layer: "points", kind: "circle", persistentId: crank.id });
  const bounds = await input.boundingBox();
  expect(bounds).not.toBeNull();
  const start = [bounds!.x + bounds!.width / 2, bounds!.y + bounds!.height / 2];
  const target = { x: ground.x + 14.265847744427303 * scale, y: ground.y - 4.635254915624211 * scale };
  const pointerTarget = await logicalToClient(frame, target);
  const undo = page.getByRole("button", { name: "Undo", exact: true });
  await expect(undo).toBeDisabled();
  const initialSave = await savedWorkspace(page);
  await page.mouse.move(start[0], start[1]);
  await page.mouse.down();
  await page.mouse.move(pointerTarget.x, pointerTarget.y, { steps: 12 });
  await page.mouse.up();
  // Unlimited native sweeps previously missed a bounded preview exhaustion:
  // selecting the input succeeded, but no accepted frame moved or committed.
  await expect(undo).toBeEnabled();
  const position = async (id: string | null) => {
    const [x, y] = await drawItems(frame, { layer: "points", kind: "circle", persistentId: id }).position();
    return { x, y };
  };
  const moved = await position(crank.id);
  expect(Math.hypot(moved.x - target.x, moved.y - target.y) / scale).toBeLessThan(0.05);
  expect(await position(ground.id)).toEqual({ x: ground.x, y: ground.y });
  const movedFoot = await position(foot.id);
  expect(Math.hypot(movedFoot.x - foot.x, movedFoot.y - foot.y) / scale).toBeGreaterThan(0.1);
  await expect.poll(() => acceptedSource(page)).toBe(originalSource);
  await expect.poll(() => savedWorkspace(page)).not.toBe(initialSave);
  const terminal = await fittedGeometry(page);
  expectGeometry(terminal, baseline, false);
  await capture(page, info, "theo-jansen-leg", "drag-terminal");
  const terminalSave = await savedWorkspace(page);
  await undo.click();
  expectGeometry(await fittedGeometry(page), baseline);
  await expect(undo).toBeDisabled();
  await expect.poll(() => savedWorkspace(page)).not.toBe(terminalSave);
  const undoSave = await savedWorkspace(page);
  await page.getByRole("button", { name: "Redo", exact: true }).click();
  expectGeometry(await fittedGeometry(page), terminal);
  await expect.poll(() => savedWorkspace(page)).not.toBe(undoSave);
  await page.reload({ waitUntil: "networkidle" });
  await settlePresentation(page);
  await expect.poll(() => acceptedSource(page), { timeout: 60_000 }).toBe(originalSource);
  expectGeometry(await fittedGeometry(page), terminal);
  await capture(page, info, "theo-jansen-leg", "drag-reload");
  await undo.click();
  expectGeometry(await fittedGeometry(page), baseline);
  await expect(undo).toBeDisabled();
}

for (const sample of samples) {
  const { key, manifest, witnesses } = sample;
  test(`M92 visual workflow: ${key}`, async ({ page }, info) => {
    test.setTimeout(360_000);
    await page.setViewportSize({ width: 1440, height: 900 });
    const errors: string[] = [];
    page.on("pageerror", (error) => errors.push(error.message));
    page.on("console", (message) => { if (message.type() === "error") errors.push(message.text()); });
    await page.goto("/", { waitUntil: "networkidle" });
    const { original, frame, points } = await openSamplePrefix(page, sample, info);
    if (process.env.GEOSOLVE_BROWSER_PREFIX_ONLY === "1") {
      expect(errors).toEqual([]);
      return;
    }
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
    expectGeometry(await fittedGeometry(page), baselineGeometry, false);
    expect(await acceptedSource(page)).toBe(original);
    await page.getByRole("button", { name: "Restore visibility before isolate" }).click();
    expectGeometry(await fittedGeometry(page), baselineGeometry);
    const explorer = page.getByRole("complementary", { name: "Explorer", exact: true });
    const horizontalScroll = await explorer.evaluate((root) => [...root.querySelectorAll("*")]
      .map((element) => ({ left: element.scrollLeft, overflow: element.scrollWidth - element.clientWidth }))
      .filter(({ left }) => left !== 0));
    expect(horizontalScroll, "group actions must not scroll declaration labels out of view").toEqual([]);
    await capture(page, info, key, "baseline");
    if (key === "theo-jansen-leg") {
      expect(original).not.toBeNull();
      await requireJansenDrag(page, info, original!, baselineGeometry);
    }
    expect(witnesses.secondary_edit, `${key} requires a distinct measured second parameter`).toBeDefined();
    expect(controlLabel(witnesses.secondary_edit, sample.source)).not.toBe(controlLabel(witnesses.representative_edit, sample.source));
    for (const [index, edit] of [witnesses.representative_edit, witnesses.secondary_edit].entries()) {
      if (index > 0) {
        await page.getByRole("button", { name: "Undo", exact: true }).click();
        await expect.poll(() => acceptedSource(page), { timeout: 60_000 }).toBe(original);
      }
      await page.getByRole("tab", { name: "Parameters", exact: true }).click();
      const control = page.getByRole("textbox", { name: controlLabel(edit, sample.source), exact: true });
      const value = String(typeof edit.replacement === "number" ? edit.replacement : edit.replacement.value);
      const beforeValue = await control.inputValue();
      expect(beforeValue).not.toBe(value);
      await control.fill(value);
      await control.press("Enter");
      await expect.poll(() => acceptedSource(page), { timeout: 60_000 }).not.toBe(original);
      await expect(control).toHaveValue(value);
      const edited = await acceptedSource(page);
      const editedGeometry = await fittedGeometry(page);
      expectGeometry(editedGeometry, baselineGeometry, false);
      await capture(page, info, key, `edit-${index + 1}`);
      await page.getByRole("button", { name: "Undo", exact: true }).click();
      await expect.poll(() => acceptedSource(page), { timeout: 60_000 }).toBe(original);
      await expect(control).toHaveValue(beforeValue);
      expectGeometry(await fittedGeometry(page), baselineGeometry);
      await capture(page, info, key, `undo-${index + 1}`);
      await page.getByRole("button", { name: "Redo", exact: true }).click();
      await expect.poll(() => acceptedSource(page), { timeout: 60_000 }).toBe(edited);
      await expect(control).toHaveValue(value);
      expectGeometry(await fittedGeometry(page), editedGeometry);
      await capture(page, info, key, `redo-${index + 1}`);
      await page.reload({ waitUntil: "networkidle" });
      await settlePresentation(page);
      await expect(page.locator("header").getByText(manifest.title, { exact: true })).toBeVisible();
      await expect.poll(() => acceptedSource(page), { timeout: 60_000 }).toBe(edited);
      await page.getByRole("tab", { name: "Parameters", exact: true }).click();
      await expect(page.getByRole("textbox", { name: controlLabel(edit, sample.source), exact: true })).toHaveValue(value);
      expect((await presentedFrame(frame)).items.some((item) => item.layer === "geometry")).toBe(true);
      expectGeometry(await fittedGeometry(page), editedGeometry);
      await capture(page, info, key, `reload-${index + 1}`);
    }
    expect(errors).toEqual([]);
  });
}
