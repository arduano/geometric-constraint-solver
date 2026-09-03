// SPDX-License-Identifier: GPL-3.0-or-later

import { expect, test, type Page } from "@playwright/test";

function auditRuntime(page: Page) {
  const errors: string[] = [];
  page.on("pageerror", (error) => errors.push(`page: ${error.message}`));
  page.on("console", (message) => {
    if (message.type() === "error") errors.push(`console: ${message.text()}`);
  });
  page.on("response", (response) => {
    if (response.status() >= 400) errors.push(`http: ${response.status()} ${response.url()}`);
  });
  return () => expect(errors).toEqual([]);
}

test("TypeScript worker provides diagnostics, completions, hover and signature help", async ({ page }) => {
  const assertCleanRuntime = auditRuntime(page);
  await page.goto("./");
  await expect(page.getByRole("status")).toHaveCount(0, { timeout: 30_000 });
  await page.getByRole("button", { name: "File menu" }).click();
  await page.getByRole("menuitem", { name: /Open/ }).click();
  await page.getByRole("button", { name: "Start from code" }).click();
  await page.getByRole("button", { name: "code", exact: true }).click();

  const content = page.locator(".cm-content");
  await expect(content).toBeVisible();
  await expect(page.getByText("No TypeScript issues")).toBeVisible({ timeout: 30_000 });

  const returnLine = page.locator(".cm-line").filter({ hasText: "return {};" });
  await returnLine.click();
  await page.keyboard.press("Home");
  await page.keyboard.insertText("$.geometry.\n  ");
  await page.keyboard.press("ArrowUp");
  await page.keyboard.press("End");
  await page.keyboard.press("Control+Space");
  await expect(page.locator(".cm-tooltip-autocomplete")).toContainText("segment", { timeout: 30_000 });
  await page.keyboard.press("Escape");
  await page.keyboard.press("Control+z");

  await returnLine.click();
  await page.keyboard.press("Home");
  await page.keyboard.insertText("$.group(\n  ");
  await page.keyboard.press("ArrowUp");
  await page.keyboard.press("End");
  await page.keyboard.press("Control+Shift+Space");
  await expect(page.locator(".cm-tooltip-signature")).toContainText("label: string", { timeout: 30_000 });
  await page.keyboard.press("Control+z");

  await page.keyboard.press("Escape");
  await page.locator(".cm-content").getByText("sketch", { exact: true }).first().hover();
  await expect(page.locator(".cm-tooltip-hover")).toContainText("function sketch", { timeout: 30_000 });

  await returnLine.click();
  await page.keyboard.press("Home");
  await page.keyboard.insertText("const badLength: number = \"wrong\";\n  ");
  await expect(page.getByText("1 error")).toBeVisible({ timeout: 30_000 });
  await page.getByText("1 error").click();
  await expect(page.getByText(/not assignable to type 'number'/u)).toBeVisible();
  assertCleanRuntime();
});
