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
  await expect(page.locator("header").getByText("PC liquid-cooling manifold", { exact: true })).toBeVisible({ timeout: 60_000 });
  await expect(page.getByRole("textbox", { name: "Channel width value", exact: true })).toHaveValue("12", { timeout: 60_000 });
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
  await expect(inspector(page).getByRole("textbox", { name: "Channel width value", exact: true })).toHaveValue("12");
  await expect(inspector(page).getByRole("textbox", { name: "Seal groove width value", exact: true })).toHaveValue("2.4");
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
  await settlePresentation(page);
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
  await explorer(page).getByRole("button", { name: "Middle outlet bore", exact: true }).click();
  await expect(explorer(page).getByRole("button", { name: "Middle outlet bore", exact: true })).toHaveAttribute("aria-pressed", "true");
  await settlePresentation(page);
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
  await settlePresentation(page);
  await expect.poll(() => acceptedSource(page)).toBe(edited);
  await explorer(page).getByRole("button", { name: "reservoirWidth", exact: true }).click();
  await expect(value).toHaveValue("62");
  await page.screenshot({ path: info.outputPath("dimension-edit.png") });
});

test("M98 standalone manifold solving paints delayed busy feedback and retains accepted geometry", async ({ page }, info) => {
  test.setTimeout(180_000);
  const errors: string[] = [];
  const workerUrls: string[] = [];
  page.on("pageerror", (error) => errors.push(error.message));
  page.on("worker", (worker) => workerUrls.push(worker.url()));
  await openManifold(page);
  await settlePresentation(page);
  expect(workerUrls.some((url) => /workbench-worker-[^/]+\.js$/.test(url))).toBe(true);
  const baseline = await geometry(page);
  expect(baseline.length).toBeGreaterThan(100);
  const source = await acceptedSource(page);
  const value = inspector(page).getByRole("textbox", { name: "Channel width value", exact: true });
  const application = page.getByRole("application");
  const overlay = page.getByRole("status").filter({ hasText: "Solving…" });
  await expect(overlay).not.toBeVisible();
  await page.evaluate(() => {
    const document = Reflect.get(globalThis, "document");
    const MutationObserver = Reflect.get(globalThis, "MutationObserver");
    const application = document.querySelector('[role="application"]')!;
    const samples: Array<{ busy: boolean; visible: boolean; at: number }> = [];
    const record = () => samples.push({ busy: application.getAttribute("aria-busy") === "true", visible: document.querySelector(".geosolve-solving-overlay") !== null, at: performance.now() });
    const observer = new MutationObserver(record);
    observer.observe(application.parentElement!, { attributes: true, attributeFilter: ["aria-busy"], childList: true, subtree: true });
    Reflect.set(globalThis, "__loadingWitness", { samples, observer });
  });
  await value.fill("13");
  await value.press("Enter");
  await expect(application).toHaveAttribute("aria-busy", "true");
  await expect(overlay).toBeVisible({ timeout: 10_000 });
  expect(await geometry(page)).toEqual(baseline);
  expect(await acceptedSource(page)).toBe(source);
  const animationFrames = await page.evaluate(() => new Promise<number>((resolve) => {
    const requestAnimationFrame = Reflect.get(globalThis, "requestAnimationFrame") as (callback: () => void) => void;
    const started = performance.now();
    let count = 0;
    function frame() { count += 1; if (performance.now() - started >= 100) resolve(count); else requestAnimationFrame(frame); }
    requestAnimationFrame(frame);
  }));
  expect(animationFrames).toBeGreaterThan(1);
  await expect(overlay).toBeVisible();
  await page.screenshot({ path: info.outputPath("manifold-solving.png") });
  await settlePresentation(page);
  await expect(overlay).not.toBeVisible();
  await expect(value).toHaveValue("13");
  await expect.poll(() => acceptedSource(page), { timeout: 60_000 }).not.toBe(source);
  await expect(page.getByRole("button", { name: "Undo", exact: true })).toBeEnabled();
  expect((await geometry(page)).length).toBe(baseline.length);
  const samples = await page.evaluate(() => {
    const witness = Reflect.get(globalThis, "__loadingWitness") as { samples: Array<{ busy: boolean; visible: boolean; at: number }>; observer: { disconnect(): void } };
    witness.observer.disconnect();
    return witness.samples;
  });
  const busy = samples.find((sample) => sample.busy);
  const visible = samples.find((sample) => sample.visible);
  expect(busy).toBeDefined();
  expect(visible).toBeDefined();
  expect(visible!.at - busy!.at).toBeGreaterThanOrEqual(450);
  expect(errors).toEqual([]);
});

const METADATA_SOURCE = `"use geosolve sketch";
import { sketch, mm } from "@geosolve/sketch-code";

export default sketch({
  title: "Metadata workbench",
  description: "Shared dimensions and a contextual radius.",
}, ($) => {
  // naïve 東京 🧭 — source-owned names never replace declaration identity.
  const sharedSpan = $.parameter("sharedSpan", mm(20), { label: "Shared span", isKeyParameter: true });
  const spareSpan = $.parameter("spareSpan", mm(20), { label: "Spare span" });
  const cornerRadius = mm(3);
  const first = $.geometry.segment("first", { start: [-30, -10], end: [-10, -10], label: "First edge" });
  const firstAnchor = $.constraint.fixedPoint("firstAnchor", { point: first.start, target: [-30, -10] });
  const firstAxis = $.constraint.horizontal("firstAxis", { span: first.span });
  const firstLength = $.dimension.curveLength("firstLength", { curve: first.span, value: sharedSpan, label: "First span", isKeyConstraint: true });
  const second = $.geometry.segment("second", { start: [10, 10], end: [30, 10], label: "Second edge" });
  const secondAnchor = $.constraint.fixedPoint("secondAnchor", { point: second.start, target: [10, 10] });
  const secondAxis = $.constraint.horizontal("secondAxis", { span: second.span });
  const secondLength = $.dimension.curveLength("secondLength", { curve: second.span, value: sharedSpan, label: "Second span" });
  const corner = $.geometry.centerRadiusCircle("corner", { center: [0, 30], radius: cornerRadius, label: "Corner bore" });
  const cornerAnchor = $.constraint.fixedPoint("cornerAnchor", { point: corner.center, target: [0, 30] });
  const cornerSize = $.dimension.radius("cornerSize", { curve: corner.curve, value: cornerRadius, label: "Corner radius" });
  $.group("Span dimensions", [first, firstAnchor, firstAxis, firstLength, second, secondAnchor, secondAxis, secondLength]);
  $.group("Corner", [corner, cornerAnchor, cornerSize]);
  return {};
});
`;

function declarationSource(source: string, id: string) {
  const declaration = source.match(new RegExp(`const ${id} = [\\s\\S]*?\\n  \\}\\);`));
  if (!declaration) throw Error(`Missing accepted source declaration ${id}`);
  return declaration[0];
}

test("M97 contextual dimension edits author source-owned overview metadata and public parameters", async ({ page }, info) => {
  test.setTimeout(300_000);
  await openManifold(page);
  const manifoldParameters = inspector(page).getByRole("group", { name: "Dimensional parameters" });
  await expect(manifoldParameters.getByRole("textbox", { name: "Channel width value", exact: true })).toHaveCount(1);
  await expect(manifoldParameters.getByRole("textbox", { name: "Channel width value", exact: true })).toHaveValue("12");
  await expect(manifoldParameters.getByRole("textbox", { name: "Seal groove width value", exact: true })).toHaveValue("2.4");
  await expect(manifoldParameters.getByText("Used by upperChannel, middleChannel, lowerChannel, stairChannel", { exact: true })).toBeVisible();

  // Run the mutation matrix on a small ordinary authored document. The full
  // manifold above independently proves the shared 12 mm public input projects once.
  await page.getByRole("button", { name: "File menu" }).click();
  await page.getByRole("menuitem", { name: /Open/ }).click();
  await page.getByRole("button", { name: "Start from code" }).click();
  const content = page.locator(".cm-content");
  await expect(page.locator("header").getByText("Untitled code sketch", { exact: true })).toBeVisible({ timeout: 60_000 });
  await expect(content).toBeEditable();
  await content.click(); await page.keyboard.press("Control+A"); await page.keyboard.insertText(METADATA_SOURCE);
  await page.getByRole("button", { name: "Apply", exact: true }).click();
  await expect.poll(() => acceptedSource(page), { timeout: 60_000 }).toContain('title: "Metadata workbench"');
  const revert = page.getByRole("button", { name: "Revert", exact: true });
  if (await revert.isEnabled()) await revert.click();
  await expect(revert).toBeDisabled();
  await page.getByRole("button", { name: "design", exact: true }).click();
  await page.getByRole("button", { name: "Fit sketch" }).click();
  await settlePresentation(page);
  const initial = (await acceptedSource(page))!;
  const baselineGeometry = await geometry(page);
  await inspector(page).getByText(/^All measurements/).click();
  await inspector(page).getByRole("button", { name: "Show details for First span", exact: true }).click();
  const overview = inspector(page).getByRole("checkbox", { name: "Show First span in overview", exact: true });
  await expect(overview).toBeChecked();
  await overview.click();
  await expect.poll(async () => declarationSource((await acceptedSource(page))!, "firstLength"), { timeout: 30_000 }).toContain("isKeyConstraint: false");
  await expect(overview).not.toBeChecked();
  await expect(overview).toBeFocused();
  const explicitlyHidden = (await acceptedSource(page))!;
  expect(declarationSource(explicitlyHidden, "secondLength")).not.toContain("isKeyConstraint");
  expect(await geometry(page)).toEqual(baselineGeometry);
  await expect(page.getByRole("button", { name: "design", exact: true })).toHaveAttribute("aria-pressed", "true");
  await emptyClick(page);
  await expect(inspector(page).getByRole("button", { name: /^Inspect / })).toHaveCount(0);
  await inspector(page).getByRole("button", { name: "Show details for Second span", exact: true }).click();
  await expect(inspector(page).getByRole("checkbox", { name: "Show Second span in overview", exact: true })).not.toBeChecked();
  await inspector(page).getByRole("button", { name: "Show details for First span", exact: true }).click();
  expect(await acceptedSource(page)).toBe(explicitlyHidden);

  await inspector(page).getByLabel("Edit First span name and description", { exact: true }).click();
  const name = inspector(page).getByRole("textbox", { name: "First span name", exact: true });
  await name.fill("Primary span"); await name.press("Enter");
  await expect.poll(async () => declarationSource((await acceptedSource(page))!, "firstLength")).toContain('label: "Primary span"');
  const renamed = inspector(page).getByRole("textbox", { name: "Primary span name", exact: true });
  await expect(renamed).toBeFocused();
  await expect(explorer(page).getByRole("button", { name: "Primary span", exact: true })).toBeVisible();
  const help = inspector(page).getByRole("textbox", { name: "Primary span description", exact: true });
  await help.fill("Overall span · naïve 東京 🧭"); await help.press("Control+Enter");
  await expect.poll(async () => declarationSource((await acceptedSource(page))!, "firstLength")).toContain('description: "Overall span · naïve 東京 🧭"');

  await page.getByText("Document properties", { exact: true }).click();
  const title = page.getByRole("textbox", { name: "Document title", exact: true });
  await title.fill("Metadata fixture"); await title.press("Enter");
  await expect.poll(() => acceptedSource(page)).toContain('title: "Metadata fixture"');
  await expect(page.locator("header").getByText("Metadata fixture", { exact: true })).toBeVisible();
  const documentHelp = page.getByRole("textbox", { name: "Document description", exact: true });
  await documentHelp.fill("Source-owned names, descriptions and dimensional intent."); await documentHelp.press("Control+Enter");
  await expect.poll(() => acceptedSource(page)).toContain('description: "Source-owned names, descriptions and dimensional intent."');
  const defaults = page.getByRole("checkbox", { name: "Show authored dimensions in overview by default", exact: true });
  await defaults.click();
  await expect.poll(() => acceptedSource(page)).toContain("areKeyConstraintsByDefault: true");
  await inspector(page).getByRole("button", { name: "Show details for Second span", exact: true }).click();
  await expect(inspector(page).getByRole("checkbox", { name: "Show Second span in overview", exact: true })).toBeChecked();
  await inspector(page).getByRole("button", { name: "Show details for Primary span", exact: true }).click();
  await expect(inspector(page).getByRole("checkbox", { name: "Show Primary span in overview", exact: true })).not.toBeChecked();
  const beforeReset = (await acceptedSource(page))!;
  await inspector(page).getByRole("button", { name: "Reset Primary span to default", exact: true }).click();
  await expect.poll(async () => declarationSource((await acceptedSource(page))!, "firstLength")).not.toContain("isKeyConstraint");
  await expect(inspector(page).getByRole("checkbox", { name: "Show Primary span in overview", exact: true })).toBeChecked();
  const afterReset = (await acceptedSource(page))!;
  expect(afterReset).toContain("// naïve 東京 🧭 — source-owned names never replace declaration identity.");
  expect(await geometry(page)).toEqual(baselineGeometry);

  await page.getByRole("button", { name: "Undo", exact: true }).click();
  await expect.poll(() => acceptedSource(page)).toBe(beforeReset);
  await page.getByRole("button", { name: "Redo", exact: true }).click();
  await expect.poll(() => acceptedSource(page)).toBe(afterReset);
  await page.reload({ waitUntil: "networkidle" });
  await settlePresentation(page);
  await expect.poll(() => acceptedSource(page)).toBe(afterReset);
  await inspector(page).getByText(/^All measurements/).click();
  await inspector(page).getByRole("button", { name: "Show details for Primary span", exact: true }).click();
  await expect(inspector(page).getByRole("checkbox", { name: "Show Primary span in overview", exact: true })).toBeChecked();
  await expect(inspector(page).locator("p").filter({ hasText: /^Overall span · naïve 東京 🧭$/ })).toBeVisible();

  await page.getByRole("tab", { name: "Parameters", exact: true }).click();
  const parameters = page.getByRole("region", { name: "Parameters", exact: true });
  await expect(parameters.getByRole("textbox", { name: "Shared span", exact: true })).toHaveCount(1);
  await expect(parameters.getByRole("textbox", { name: "Shared span", exact: true })).toHaveValue("20");
  await expect(parameters.getByRole("textbox", { name: "Spare span", exact: true })).toHaveValue("20");
  await expect(parameters.getByText("Used by Primary span, Second span", { exact: true })).toBeVisible();
  await expect(parameters.getByRole("checkbox", { name: "Show Spare span in overview", exact: true })).not.toBeChecked();
  const beforeExtractionGeometry = await geometry(page);
  await parameters.getByRole("button", { name: "Make cornerRadius a named parameter", exact: true }).click();
  await expect.poll(() => acceptedSource(page)).toContain('const cornerRadius = $.parameter("cornerRadius", mm(3), {');
  const extracted = (await acceptedSource(page))!;
  expect(extracted).toContain("radius: cornerRadius");
  expect(extracted).toContain("value: cornerRadius");
  expect(declarationSource(extracted, "cornerSize")).toContain('label: "Corner radius"');
  await expect(parameters.getByRole("textbox", { name: "cornerRadius", exact: true })).toHaveValue("3");
  await expect(parameters.getByRole("checkbox", { name: "Show cornerRadius in overview", exact: true })).toBeEnabled();
  expect(await geometry(page)).toEqual(beforeExtractionGeometry);
  await page.getByRole("button", { name: "Undo", exact: true }).click();
  await expect.poll(() => acceptedSource(page)).toBe(afterReset);
  await page.getByRole("button", { name: "Redo", exact: true }).click();
  await expect.poll(() => acceptedSource(page)).toBe(extracted);
  await page.reload({ waitUntil: "networkidle" });
  await settlePresentation(page);
  await expect.poll(() => acceptedSource(page)).toBe(extracted);
  await page.getByRole("tab", { name: "Parameters", exact: true }).click();
  await expect(parameters.getByRole("checkbox", { name: "Show cornerRadius in overview", exact: true })).toBeEnabled();
  await page.screenshot({ path: info.outputPath("source-owned-parameters.png") });
  expect(extracted).not.toBe(initial);
});
