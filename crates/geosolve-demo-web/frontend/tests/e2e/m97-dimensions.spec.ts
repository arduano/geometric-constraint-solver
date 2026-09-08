// SPDX-License-Identifier: GPL-3.0-or-later
import { expect, test, type Page } from "@playwright/test";
import { acceptedSource, savedWorkspace } from "./release-sample-prefix";
import { canvasFrame, itemGeometry, presentedFrame, settlePresentation } from "./presented-canvas";

async function openManifold(page: Page) {
  await page.setViewportSize({ width: 1600, height: 1000 });
  await page.goto("./", { waitUntil: "networkidle" });
  await page.getByRole("button", { name: "File menu" }).click();
  await page.getByRole("menuitem", { name: /Open/ }).click();
  await page.getByPlaceholder(/Search \d+ samples/).fill("water manifold");
  await page.getByRole("button", { name: "PC liquid-cooling manifold", exact: false }).click();
  await page.getByRole("button", { name: "design", exact: true }).click();
  await page.getByRole("button", { name: "Fit sketch" }).click();
  await settlePresentation(page);
}

const inspector = (page: Page) => page.getByRole("region", { name: "Dimensions", exact: true });
const explorer = (page: Page) => page.getByRole("complementary", { name: "Explorer", exact: true });
const dimensionTexts = async (page: Page) => (await presentedFrame(canvasFrame(page))).items.filter(
  (item) => item.kind === "text" && item.id.startsWith("dimension:"),
);
const dimensionIds = async (page: Page) => (await dimensionTexts(page)).map((item) => item.id).sort();
const geometry = async (page: Page) => (await presentedFrame(canvasFrame(page))).items.filter(
  (item) => ["geometry", "computed"].includes(item.layer),
).map((item) => ({ id: item.id, geometry: itemGeometry(item) }));

async function emptyClick(page: Page) {
  const box = await canvasFrame(page).boundingBox();
  if (!box) throw Error("Missing canvas");
  await page.mouse.click(box.x + 8, box.y + 8);
  await page.mouse.move(box.x - 5, box.y - 5);
  await settlePresentation(page);
}

test("M97 manifold focus limits canvas dimensions and preserves pinned measurements on reload", async ({ page }, info) => {
  await openManifold(page);
  await expect(page.getByRole("combobox", { name: "Dimension display" })).toHaveValue("focused");
  await expect.poll(() => dimensionTexts(page).then((items) => items.length)).toBeGreaterThan(0);
  expect((await dimensionTexts(page)).length).toBeLessThanOrEqual(6);
  await expect(inspector(page).getByRole("listitem")).toHaveCount(6);
  await expect(inspector(page).getByRole("listitem").filter({ hasText: "Key dimension" })).toHaveCount(6);
  await expect(inspector(page).getByRole("textbox", { name: "plateWidth value", exact: true })).toHaveValue("240");
  await expect(inspector(page).getByRole("textbox", { name: "reservoirWidth value", exact: true })).toHaveValue("60");
  await expect(inspector(page).getByRole("textbox", { name: "channelWidth value", exact: true })).toHaveValue("12");
  await expect(inspector(page).getByRole("textbox", { name: "commonSealGroove · width value", exact: true })).toHaveValue("2.4");
  await page.screenshot({ path: info.outputPath("focused-overview.png") });
  const source = await acceptedSource(page);
  const baseline = await geometry(page);
  await explorer(page).getByRole("button", { name: "Upper channel circuit", exact: true }).click();
  await expect.poll(() => dimensionTexts(page).then((items) => items.length)).toBeGreaterThan(0);
  expect((await dimensionTexts(page)).length).toBeLessThanOrEqual(12);
  await expect(inspector(page).getByRole("group", { name: "Dimensional parameters" })).toBeVisible();
  await expect(inspector(page).getByText(/Generated dimensions/)).toBeVisible();
  await page.screenshot({ path: info.outputPath("focused-channel.png") });
  await inspector(page).getByRole("button", { name: "Inspect upperCenterLength1", exact: true }).click();
  const pin = inspector(page).getByRole("listitem").filter({ hasText: "On canvas", hasNotText: "Key dimension" }).getByRole("button", { name: /^Pin / }).first();
  await expect(pin).toBeEnabled();
  const label = (await pin.getAttribute("aria-label"))!.slice(4);
  await pin.click();
  await expect(inspector(page).getByText("1/4 pinned")).toBeVisible();
  await emptyClick(page);
  await expect(inspector(page).getByRole("button", { name: `Unpin ${label}`, exact: true })).toBeVisible();
  expect(await geometry(page)).toEqual(baseline);
  expect(await acceptedSource(page)).toBe(source);
  await expect(page.getByRole("button", { name: "Undo", exact: true })).toBeDisabled();
  await expect.poll(() => savedWorkspace(page)).not.toBeNull();
  await page.reload({ waitUntil: "networkidle" });
  await expect(inspector(page).getByText("1/4 pinned")).toBeVisible();
  await expect.poll(() => dimensionTexts(page).then((items) => items.length)).toBeGreaterThan(0);
  await page.screenshot({ path: info.outputPath("focused-pin.png") });
  await inspector(page).getByRole("button", { name: "Clear pins", exact: true }).click();
  await expect(inspector(page).getByText("0/4 pinned")).toBeVisible();
  await expect(inspector(page).getByRole("listitem")).toHaveCount(6);
  await expect.poll(() => dimensionTexts(page).then((items) => items.length)).toBeGreaterThan(0);
  expect((await dimensionTexts(page)).length).toBeLessThanOrEqual(6);
  // Resolve a known nonpriority bore through its ordinary Explorer selection.
  // The selected native circle identifies the exact geometry for the hover probe.
  await explorer(page).getByRole("button", { name: "middleOutlet", exact: true }).click();
  const selected = await presentedFrame(canvasFrame(page));
  const bores = selected.items.filter((item) => item.layer === "geometry" && item.kind === "polyline"
    && item.style.stroke === "#efb856" && item.style.shadow !== null
    && item.points.length > 12 && Math.hypot(item.points[0][0] - item.points.at(-1)![0], item.points[0][1] - item.points.at(-1)![1]) < 0.01);
  expect(bores).toHaveLength(1);
  await emptyClick(page);
  const overview = await dimensionIds(page);
  const bore = (await presentedFrame(canvasFrame(page))).items.find((item) => item.id === bores[0].id);
  if (!bore || bore.kind !== "polyline") throw Error("Expected the accepted middle outlet bore");
  const box = (await canvasFrame(page).boundingBox())!;
  const point = bore.points[Math.floor(bore.points.length / 8)];
  await page.mouse.move(box.x + point[0], box.y + point[1]);
  await expect.poll(() => dimensionTexts(page).then((items) => items.length)).toBe(overview.length + 1);
  expect(await dimensionIds(page)).toEqual(expect.arrayContaining(overview));
  await page.mouse.down({ button: "middle" });
  await expect.poll(() => dimensionIds(page)).toEqual(overview);
  await page.mouse.up({ button: "middle" });
  await page.mouse.move(box.x + point[0] + 0.2, box.y + point[1]);
  await expect.poll(() => dimensionTexts(page).then((items) => items.length)).toBe(overview.length + 1);
  const preview = (await dimensionTexts(page)).find((item) => !overview.includes(item.id));
  if (!preview || preview.kind !== "text") throw Error("Expected the nonpriority hovered dimension label");
  await page.mouse.move(box.x + preview.position[0], box.y + preview.position[1], { steps: 6 });
  await page.waitForTimeout(300);
  expect((await dimensionTexts(page)).map((item) => item.id)).toContain(preview.id);
  await page.mouse.click(box.x + preview.position[0], box.y + preview.position[1]);
  await expect.poll(() => inspector(page).getByRole("button", { name: /^Inspect / }).count()).toBeGreaterThan(0);
  await emptyClick(page);
  await expect.poll(() => dimensionIds(page)).toEqual(overview);
  expect(await acceptedSource(page)).toBe(source);
  await expect(page.getByRole("button", { name: "Undo", exact: true })).toBeDisabled();
});

test("M97 dimension positions survive zoom followed by a selection-only scene rebuild", async ({ page }, info) => {
  await openManifold(page);
  await page.getByRole("combobox", { name: "Dimension display" }).selectOption("all");
  await expect.poll(() => dimensionTexts(page).then((items) => items.length)).toBe(82);
  const source = await acceptedSource(page);
  const box = (await canvasFrame(page).boundingBox())!;
  await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
  for (let i = 0; i < 4; i++) { await page.mouse.wheel(0, -90); await page.waitForTimeout(150); }
  await page.mouse.move(box.x - 5, box.y - 5);
  await settlePresentation(page);
  const before = await dimensionTexts(page);
  await emptyClick(page);
  const after = await dimensionTexts(page);
  expect(after.map((item) => item.id)).toEqual(before.map((item) => item.id));
  for (let i = 0; i < before.length; i++) {
    const a = before[i]; const b = after[i];
    if (a.kind !== "text" || b.kind !== "text") throw Error("Expected dimension text");
    expect(Math.hypot(a.position[0] - b.position[0], a.position[1] - b.position[1])).toBeLessThan(0.01);
  }
  expect(await acceptedSource(page)).toBe(source);
  await page.getByRole("combobox", { name: "Dimension display" }).selectOption("hidden");
  await expect.poll(() => dimensionTexts(page).then((items) => items.length)).toBe(0);
  await page.screenshot({ path: info.outputPath("hidden-overview.png") });

  await page.getByRole("button", { name: "File menu" }).click();
  await page.getByRole("menuitem", { name: /Open/ }).click();
  await page.getByPlaceholder(/Search \d+ samples/).fill("gridfinity");
  await page.getByRole("button", { name: "Gridfinity plan and 3U section", exact: false }).click();
  await page.getByRole("button", { name: "design", exact: true }).click();
  await page.getByRole("button", { name: "Fit sketch" }).click();
  await page.getByRole("combobox", { name: "Dimension display" }).selectOption("focused");
  await expect(inspector(page).getByRole("listitem")).toHaveCount(20);
  await expect(inspector(page).getByRole("listitem").filter({ hasText: "Key dimension" })).toHaveCount(20);
  await expect.poll(() => acceptedSource(page)).toContain("const baseBottomWidth");
  const gridfinitySource = await acceptedSource(page);
  await expect(page.getByRole("button", { name: "Undo", exact: true })).toBeDisabled();
  // The complete standard-dimension set is eligible; it is never reduced to
  // the ordinary six related callouts. Zoom may omit labels outside the view.
  await expect.poll(() => dimensionTexts(page).then((items) => items.length)).toBeGreaterThan(6);
  await page.screenshot({ path: info.outputPath("gridfinity-key-overview.png") });
  const gridBox = (await canvasFrame(page).boundingBox())!;
  await page.mouse.move(gridBox.x + gridBox.width / 2, gridBox.y + gridBox.height / 2);
  for (let i = 0; i < 4; i++) { await page.mouse.wheel(0, -90); await page.waitForTimeout(150); }
  await page.mouse.move(gridBox.x - 5, gridBox.y - 5);
  await expect.poll(() => dimensionTexts(page).then((items) => items.length)).toBeGreaterThan(0);
  expect((await dimensionTexts(page)).length).toBeLessThanOrEqual(20);
  const gridfinityOverview = await dimensionTexts(page);
  await page.screenshot({ path: info.outputPath("gridfinity-key-dimensions.png") });
  await page.getByRole("combobox", { name: "Dimension display" }).selectOption("hidden");
  await expect.poll(() => dimensionTexts(page).then((items) => items.length)).toBe(0);
  await expect(inspector(page).getByRole("listitem")).toHaveCount(20);
  await page.getByRole("combobox", { name: "Dimension display" }).selectOption("focused");
  await expect.poll(() => dimensionIds(page)).toEqual(expect.arrayContaining(gridfinityOverview.map((item) => item.id)));
  const restored = await dimensionTexts(page);
  expect(restored.length).toBeLessThanOrEqual(20);
  for (const before of gridfinityOverview) {
    const after = restored.find((item) => item.id === before.id);
    if (before.kind !== "text" || after?.kind !== "text") throw Error("Expected retained priority text");
    expect(Math.hypot(after.position[0] - before.position[0], after.position[1] - before.position[1])).toBeLessThan(0.01);
  }
  expect(await acceptedSource(page)).toBe(gridfinitySource);
  await expect(page.getByRole("button", { name: "Undo", exact: true })).toBeDisabled();
});

test("M97 contextual dimension edits retain source authority and Undo Redo reload", async ({ page }, info) => {
  test.setTimeout(180_000);
  await openManifold(page);
  const source = await acceptedSource(page);
  await explorer(page).getByRole("button", { name: "reservoirWidth", exact: true }).click();
  const value = inspector(page).getByRole("textbox", { name: "reservoirWidth value", exact: true });
  await expect(value).toHaveValue("60");
  await value.fill("62"); await value.press("Enter");
  await expect.poll(() => acceptedSource(page), { timeout: 60_000 }).not.toBe(source);
  await expect(value).toHaveValue("62");
  await expect(value).toBeFocused();
  const edited = await acceptedSource(page);
  await page.getByRole("button", { name: "Undo", exact: true }).click();
  await expect.poll(() => acceptedSource(page), { timeout: 30_000 }).toBe(source);
  await page.getByRole("button", { name: "Redo", exact: true }).click();
  await expect.poll(() => acceptedSource(page), { timeout: 30_000 }).toBe(edited);
  await page.reload({ waitUntil: "networkidle" });
  await expect.poll(() => acceptedSource(page)).toBe(edited);
  await explorer(page).getByRole("button", { name: "reservoirWidth", exact: true }).click();
  await expect(value).toHaveValue("62");
  await page.screenshot({ path: info.outputPath("dimension-edit.png") });
});
