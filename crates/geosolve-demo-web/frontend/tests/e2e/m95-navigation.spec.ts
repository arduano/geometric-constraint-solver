// SPDX-License-Identifier: GPL-3.0-or-later
import { writeFile } from "node:fs/promises";
import { expect, test, type Locator, type Page } from "@playwright/test";
import { acceptedSource, samples, savedWorkspace } from "./release-sample-prefix";
import { canvasFrame, decodeScreenshot, itemGeometry, logicalToClient, presentedFrame, settlePresentation } from "./presented-canvas";

const SOURCE = `"use geosolve sketch";
import { sketch } from "@geosolve/sketch-code";

export default sketch(($) => {
  // naïve 東京 🧭 — source navigation must preserve exact Unicode offsets.
  const first = $.geometry.segment("first", { start: [-45, -20], end: [-15, -20], label: "First edge" });
  const second = $.geometry.segment("second", { start: [15, 20], end: [45, 20], label: "Second edge" });
  $.group("Edges", [first, second]);
  return { first, second };
});
`;

async function startControlled(page: Page) {
  await page.setViewportSize({ width: 1600, height: 1000 });
  await page.goto("./", { waitUntil: "networkidle" });
  await page.getByRole("button", { name: "File menu" }).click();
  await page.getByRole("menuitem", { name: /Open/ }).click();
  await page.getByRole("button", { name: "Start from code" }).click();
  const content = page.locator(".cm-content");
  await expect(page.locator("header").getByText("Untitled code sketch", { exact: true })).toBeVisible({ timeout: 60_000 });
  await expect(content).toBeEditable();
  await content.click();
  await page.keyboard.press("Control+A");
  await page.keyboard.insertText(SOURCE);
  await page.getByRole("button", { name: "Apply", exact: true }).click();
  await expect.poll(() => acceptedSource(page), { timeout: 30_000 }).toContain('$.geometry.segment("second"');
  // Managed Apply normalizes source. Adopt those accepted bytes before browsing.
  const revert = page.getByRole("button", { name: "Revert", exact: true });
  if (await revert.isEnabled()) await revert.click();
  await expect(revert).toBeDisabled();
  await expect(page.locator(".cm-content")).toContainText("naïve 東京 🧭");
  await page.getByRole("button", { name: "split", exact: true }).click();
  await page.getByRole("button", { name: "Fit sketch" }).click();
  await settlePresentation(page);
  return canvasFrame(page);
}

const explorer = (page: Page) => page.getByRole("complementary", { name: "Explorer" });
const row = (page: Page, label: string) => explorer(page).getByRole("button", { name: label, exact: true });
const highlightedSource = async (page: Page) => (await page.locator(".cm-sketch-source-owner").allTextContents()).join("\n").replace(/\s+/g, " ");

/** Read the same retained view used by CodeMirror's findFromDOM; never dispatch test actions. */
async function editorSelection(page: Page) {
  return page.locator(".cm-content").evaluate((element) => {
    const tile = Reflect.get(element, "cmTile");
    const view = tile?.root?.view;
    if (!view?.state?.selection?.main) throw Error("Retained CodeMirror selection is unavailable");
    const { from, to } = view.state.selection.main;
    return { from, to, text: view.state.sliceDoc(from, to), focused: view.hasFocus };
  });
}

async function selectedCurveIds(canvas: Locator, layer = "geometry") {
  return (await presentedFrame(canvas)).items.filter((item) => item.layer === layer && item.style.shadow !== null && item.style.stroke === "#efb856").map((item) => item.id).sort();
}

async function expectSelectedPixels(canvas: Locator, itemId: string) {
  const scene = await presentedFrame(canvas);
  const item = scene.items.find((candidate) => candidate.id === itemId);
  if (!item || item.kind !== "polyline" || item.points.length < 2) throw Error("Expected one selected rendered span");
  const middle = Math.floor((item.points.length - 1) / 2);
  const a = item.points[middle]; const b = item.points[middle + 1];
  const point = await logicalToClient(canvas, { x: (a[0] + b[0]) / 2, y: (a[1] + b[1]) / 2 });
  const image = decodeScreenshot(await canvas.screenshot());
  const box = (await canvas.boundingBox())!;
  const x = Math.round((point.x - box.x) * image.width / box.width);
  const y = Math.round((point.y - box.y) * image.height / box.height);
  let amber = 0;
  for (let dy = -5; dy <= 5; dy++) for (let dx = -5; dx <= 5; dx++) {
    const px = x + dx; const py = y + dy;
    if (px < 0 || py < 0 || px >= image.width || py >= image.height) continue;
    const index = (py * image.width + px) * image.channels;
    if (Math.abs(image.pixels[index] - 239) < 35 && Math.abs(image.pixels[index + 1] - 184) < 35 && Math.abs(image.pixels[index + 2] - 86) < 35) amber++;
  }
  expect(amber, "selected span must paint actual amber pixels away from point markers").toBeGreaterThan(2);
  return { itemId, amber };
}

test("M95 canvas and Explorer selection reveal exact source without stealing cursor or saving", async ({ page }, info) => {
  const canvas = await startControlled(page);
  const beforeSaved = await savedWorkspace(page);
  const beforeGeometry = (await presentedFrame(canvas)).items.filter((item) => item.layer === "geometry").map(itemGeometry);
  const content = page.locator(".cm-content");
  await content.click();
  await page.keyboard.press("Control+Home");
  await page.keyboard.press("ArrowRight");
  const cursor = await editorSelection(page);

  await row(page, "First edge").click();
  await expect(row(page, "First edge")).toHaveAttribute("aria-current", "true");
  await expect(row(page, "First edge")).toHaveCSS("background-color", "rgba(251, 191, 36, 0.1)");
  await expect(row(page, "Edges")).toHaveAttribute("aria-pressed", "mixed");
  await expect.poll(() => highlightedSource(page)).toContain('$.geometry.segment("first"');
  const after = await editorSelection(page);
  expect({ from: after.from, to: after.to }).toEqual({ from: cursor.from, to: cursor.to });
  await expect(row(page, "First edge")).toBeFocused();
  await expect.poll(() => selectedCurveIds(canvas)).toHaveLength(1);
  const firstId = (await selectedCurveIds(canvas))[0];
  const pixel = await expectSelectedPixels(canvas, firstId);

  await row(page, "Second edge").click({ modifiers: ["Shift"] });
  await expect(row(page, "Edges")).toHaveAttribute("aria-pressed", "true");
  await expect.poll(() => selectedCurveIds(canvas)).toHaveLength(2);
  expect(await highlightedSource(page)).toContain('$.geometry.segment("second"');
  const first = (await presentedFrame(canvas)).items.find((item) => item.id === firstId)!;
  if (first.kind !== "polyline") throw Error("Expected first segment");
  const point = await logicalToClient(canvas, { x: (first.points[0][0] + first.points.at(-1)![0]) / 2, y: (first.points[0][1] + first.points.at(-1)![1]) / 2 });
  await page.mouse.click(point.x, point.y);
  await page.mouse.move(1, 1);
  // Picking a span selects that exact native member; the declaration also owns endpoints.
  await expect(row(page, "First edge")).toHaveAttribute("aria-pressed", "mixed");
  await expect(row(page, "Second edge")).not.toHaveAttribute("aria-current", "true");
  await expect.poll(() => selectedCurveIds(canvas)).toEqual([firstId]);
  await expect.poll(() => highlightedSource(page)).not.toContain('$.geometry.segment("second"');

  await row(page, "Edges").click();
  await expect.poll(() => selectedCurveIds(canvas)).toHaveLength(2);
  await expect(row(page, "Edges")).toHaveAttribute("aria-pressed", "true");
  expect((await presentedFrame(canvas)).items.filter((item) => item.layer === "geometry").map(itemGeometry)).toEqual(beforeGeometry);
  expect(await savedWorkspace(page)).toBe(beforeSaved);
  await expect(page.getByRole("button", { name: "split", exact: true })).toHaveAttribute("aria-pressed", "true");
  const evidence = info.outputPath("connected-selection.json");
  await writeFile(evidence, JSON.stringify(pixel));
  const screenshot = info.outputPath("connected-selection.png");
  await page.screenshot({ path: screenshot });
  await info.attach("connected-selection-pixels", { path: evidence, contentType: "application/json" });
  await info.attach("connected-selection", { path: screenshot, contentType: "image/png" });
});

test("M95 explicit code navigation preserves layout intent and blocks unapplied source", async ({ page }) => {
  const canvas = await startControlled(page);
  const saved = await savedWorkspace(page);
  await row(page, "First edge").click();
  await page.getByRole("button", { name: "design", exact: true }).click();
  await row(page, "Second edge").click();
  await expect(page.getByRole("button", { name: "design", exact: true })).toHaveAttribute("aria-pressed", "true");
  await page.getByRole("button", { name: "Show in code", exact: true }).click();
  await expect(page.getByRole("button", { name: "split", exact: true })).toHaveAttribute("aria-pressed", "true");
  await expect(page.locator(".cm-content")).toBeFocused();
  expect((await editorSelection(page)).text).toMatch(/^\$\.geometry\.segment\("second"/);

  await page.getByRole("button", { name: "code", exact: true }).click();
  await page.locator(".cm-line").filter({ hasText: "const first =" }).click();
  await page.keyboard.press("Home");
  await page.keyboard.press("Shift+End");
  const selection = await editorSelection(page);
  expect(selection.text).toContain('$.geometry.segment("first"');
  await page.getByRole("button", { name: "Show in canvas", exact: true }).click();
  await expect(page.getByRole("button", { name: "split", exact: true })).toHaveAttribute("aria-pressed", "true");
  await expect(row(page, "First edge")).toHaveAttribute("aria-current", "true");
  await expect(row(page, "Second edge")).not.toHaveAttribute("aria-current", "true");
  await expect.poll(() => selectedCurveIds(canvas)).toHaveLength(1);

  await page.locator(".cm-content").click();
  await page.keyboard.press("Control+Home");
  await page.keyboard.press("Control+Shift+Enter");
  await expect(page.getByRole("status").filter({ hasText: "No sketch object here" })).toBeVisible();
  await expect(row(page, "First edge")).toHaveAttribute("aria-current", "true");
  await page.keyboard.insertText("// unapplied\n");
  await expect(page.getByRole("button", { name: "Show in canvas", exact: true })).toBeDisabled();
  await expect(page.getByRole("button", { name: "Show in code", exact: true })).toBeDisabled();
  await expect(page.locator(".cm-sketch-source-owner")).toHaveCount(0);
  await row(page, "Second edge").click();
  await expect(row(page, "Second edge")).toHaveAttribute("aria-current", "true");
  await expect(page.locator(".cm-sketch-source-owner")).toHaveCount(0);
  await page.getByRole("button", { name: "Revert", exact: true }).click();
  await expect(page.getByRole("button", { name: "Show in canvas", exact: true })).toBeEnabled();
  await expect.poll(() => highlightedSource(page)).toContain('$.geometry.segment("second"');
  expect(await savedWorkspace(page)).toBe(saved);
});

test("M95 scale navigation selects generated harness members and their invocation without saving", async ({ page }, info) => {
  await page.setViewportSize({ width: 1600, height: 1000 });
  await page.goto("./", { waitUntil: "networkidle" });
  const sample = samples.find((entry) => entry.key === "robotic-harness-backplane")!;
  await page.getByRole("button", { name: "File menu" }).click();
  await page.getByRole("menuitem", { name: /Open/ }).click();
  await page.getByPlaceholder(`Search ${samples.length} samples…`).fill(sample.key);
  await page.getByRole("dialog", { name: "Open project" }).getByRole("button").filter({ has: page.getByText(sample.manifest.title, { exact: true }) }).click();
  await expect.poll(() => acceptedSource(page), { timeout: 60_000 }).toBe(sample.source);
  await page.getByRole("button", { name: "Fit sketch" }).click();
  const canvas = canvasFrame(page);
  await settlePresentation(page);
  const saved = await savedWorkspace(page);
  const invocation = explorer(page).getByRole("button", { name: "powerHarness", exact: true });
  await expect(invocation).toBeVisible();
  const members = invocation.locator("xpath=ancestor::li[1]").getByRole("list", { name: "powerHarness generated outputs" });
  const memberRows = members.locator('button[data-navigation-row]');
  const memberIds = await memberRows.evaluateAll((buttons) => buttons.map((button) => button.getAttribute("data-navigation-row")!));
  const memberIndex = memberIds.findIndex((id) => {
    if (!id.startsWith("generated:")) return false;
    const address = JSON.parse(id.slice("generated:".length));
    return address.member_key.join("/") === "entry" && address.output.join("/") === "field:curve";
  });
  expect(memberIndex, "use the clip's exact curve output rather than its separate centre or radius port").toBeGreaterThanOrEqual(0);
  const member = memberRows.nth(memberIndex);
  const memberId = memberIds[memberIndex];
  await member.click();
  await expect(member).toHaveAttribute("aria-current", "true");
  await expect(invocation).toHaveAttribute("aria-pressed", "mixed");
  await expect.poll(() => highlightedSource(page)).toContain('$.use("powerHarness"');
  await expect.poll(async () => (await selectedCurveIds(canvas)).length).toBeGreaterThan(0);
  const selectedMember = await selectedCurveIds(canvas);
  await expect(page.getByRole("tabpanel", { name: "Inspector", exact: true })).not.toContainText("code.generated.");
  const timings = [];
  for (let index = 0; index < 3; index++) {
    const begin = Date.now();
    await invocation.click();
    await expect(invocation).toHaveAttribute("aria-pressed", "true");
    await expect.poll(async () => (await selectedCurveIds(canvas)).length).toBeGreaterThan(selectedMember.length);
    timings.push(Date.now() - begin);
    await member.click();
    await expect(member).toHaveAttribute("aria-current", "true");
    await expect.poll(() => selectedCurveIds(canvas)).toEqual(selectedMember);
  }
  expect(await savedWorkspace(page)).toBe(saved);
  await expect(page.getByRole("button", { name: "split", exact: true })).toHaveAttribute("aria-pressed", "true");
  const evidence = info.outputPath("dense-navigation.json");
  await writeFile(evidence, JSON.stringify({ memberId, selectedMember, invocationWallMs: timings, savedBytes: saved?.length }));
  const screenshot = info.outputPath("dense-navigation.png");
  await page.screenshot({ path: screenshot });
  await info.attach("dense-navigation-observation", { path: evidence, contentType: "application/json" });
  await info.attach("dense-navigation", { path: screenshot, contentType: "image/png" });
});
