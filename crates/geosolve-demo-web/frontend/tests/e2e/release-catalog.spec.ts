// SPDX-License-Identifier: GPL-3.0-or-later
import { readFile } from "node:fs/promises";
import { expect, test } from "@playwright/test";
import { catalogContract, samples, savedWorkspace } from "./release-sample-prefix";

const frontendCatalog = JSON.parse(await readFile(new URL("../../src/data/samples.json", import.meta.url), "utf8")) as Array<{
  ordinal: number; stableId: string; key: string; title: string; category: string; group: string;
}>;
const groups: Record<string, string> = {
  mechanism: "Mechanisms", product_fabrication: "Products & fabrication",
  reference_lab: "Reference labs", scale_study: "Scale studies",
};

test("release catalog preserves reviewed order, categories and surviving Recent routes and rejects retired keys", async ({ page }) => {
  test.setTimeout(180_000);
  expect(frontendCatalog.map(({ key, title, category }) => ({ key, title, category }))).toEqual(catalogContract.samples);
  expect(frontendCatalog.map(({ ordinal }) => ordinal)).toEqual(samples.map((_, index) => index + 1));
  expect(frontendCatalog.map(({ stableId, key }) => stableId === `sample.${key}`)).toEqual(samples.map(() => true));
  const survivor = samples[0];
  expect(survivor).toBeDefined();
  // Unknown keys must remain covered even before the first post-contract pruning.
  const retired = [...new Set([...catalogContract.retired_keys, "geosolve-nonexistent-release-sample"])];
  expect(retired.every((key) => !samples.some((sample) => sample.key === key))).toBe(true);
  await page.addInitScript(({ retired, key }) => {
    localStorage.setItem("geosolve-workbench-recents-v2", JSON.stringify({
      version: 2, entries: [...retired.map((key) => ({ key })), { key }],
    }));
  }, { retired, key: survivor.key });
  const runtimeErrors: string[] = [];
  page.on("pageerror", (error) => runtimeErrors.push(error.message));
  page.on("console", (message) => { if (message.type() === "error") runtimeErrors.push(message.text()); });
  await page.setViewportSize({ width: 1440, height: 900 });
  await page.goto("/", { waitUntil: "networkidle" });
  const open = async () => {
    await page.getByRole("button", { name: "File menu" }).click();
    await page.getByRole("menuitem", { name: /Open/ }).click();
    return page.getByRole("dialog", { name: "Open project" });
  };
  const dialog = await open();
  await expect(dialog.getByPlaceholder(`Search ${samples.length} samples…`)).toBeVisible();
  const recent = dialog.getByRole("region", { name: "Recent", exact: true });
  await expect(recent.getByRole("button")).toHaveCount(1);
  await expect(recent.getByRole("button").locator("span").first()).toHaveText(survivor.manifest.title);
  const categoryOrder = [...new Set(catalogContract.samples.map(({ category }) => category))];
  await expect(dialog.locator('section[aria-labelledby^="sample-group-"] > h3')).toHaveText(categoryOrder.map((category) => groups[category]));
  const renderedOrder: string[] = [];
  for (const category of categoryOrder) {
    const entries = samples.filter((sample) => sample.manifest.category === category);
    const section = dialog.locator('section[aria-labelledby^="sample-group-"]')
      .filter({ has: page.getByRole("heading", { name: groups[category], exact: true }) });
    const titles = section.getByRole("button").locator("span:first-child");
    await expect(titles).toHaveText(entries.map((sample) => sample.manifest.title));
    renderedOrder.push(...await titles.allTextContents());
  }
  expect(renderedOrder).toEqual(samples.map((sample) => sample.manifest.title));
  for (const key of retired) {
    await dialog.getByPlaceholder(`Search ${samples.length} samples…`).fill(key);
    await expect(dialog.getByRole("status")).toHaveText(`No samples match “${key}”.`);
    await expect(dialog.getByRole("button")).toHaveCount(3); // Only the three non-catalog Quick Start actions.
  }
  await dialog.getByPlaceholder(`Search ${samples.length} samples…`).fill("");
  await recent.getByRole("button").click();
  await expect(page.locator("header").getByText(survivor.manifest.title, { exact: true })).toBeVisible();
  await expect.poll(async () => {
    const wire = await savedWorkspace(page);
    return wire ? JSON.parse(JSON.parse(wire).project).origin : null;
  }, { timeout: 60_000 }).toEqual({ kind: "bundled", sample: survivor.key });
  // The ordinary persisted/import path reaches the real Rust bundled-key lookup.
  // Changing only origin ensures a removed alias cannot restore or replace the
  // current accepted project while retaining all other valid session bytes.
  const baseline = (await savedWorkspace(page))!;
  // Import's public decoder deliberately tries several supported envelopes and
  // reports its final decoder error. Prove the unchanged control imports before
  // requiring each otherwise-identical retired origin to reject; its message is
  // not a stable bundled-lookup API.
  const importFile = page.locator('input[type="file"]');
  await importFile.setInputFiles({ name: "surviving-sample.json", mimeType: "application/json", buffer: Buffer.from(baseline) });
  await expect(importFile).toHaveValue("");
  await expect(page.getByRole("alert")).toHaveCount(0);
  await expect.poll(() => savedWorkspace(page), { timeout: 60_000 }).toBe(baseline);
  for (const key of retired) {
    const persisted = JSON.parse(baseline);
    const workbench = JSON.parse(persisted.project);
    workbench.origin = { kind: "bundled", sample: key };
    persisted.project = JSON.stringify(workbench);
    await importFile.setInputFiles({
      name: "retired-sample.json", mimeType: "application/json", buffer: Buffer.from(JSON.stringify(persisted)),
    });
    await expect(importFile).toHaveValue("");
    await expect(page.getByRole("alert"), `retired Rust origin ${key} must reject`).toBeVisible();
    await expect(page.locator("header").getByText(survivor.manifest.title, { exact: true })).toBeVisible();
    expect(await savedWorkspace(page)).toBe(baseline);
    await page.getByRole("button", { name: "Dismiss action error", exact: true }).click();
  }
  expect(runtimeErrors).toEqual([]);
  // Every key's actual UI→Rust project/source routing is independently checked by
  // the fresh sample-prefix batch. This catalog row owns the shared menu/Recent
  // boundary, and does not claim those per-key prefixes as its own execution.
});
