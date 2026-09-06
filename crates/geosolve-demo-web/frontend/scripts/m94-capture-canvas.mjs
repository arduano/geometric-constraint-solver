// SPDX-License-Identifier: GPL-3.0-or-later
// Capture the candidate canvas using the baseline camera and real pointer workflow.
import { chromium, expect } from '@playwright/test';
import { createHash } from 'node:crypto';
import { readFile, mkdir, writeFile } from 'node:fs/promises';
import { resolve, join } from 'node:path';
import { execFileSync } from 'node:child_process';

const repository = resolve(import.meta.dirname, '../../../..');
const output = resolve(process.env.M94_CAPTURE_OUTPUT ?? join(repository, 'target/m94/canvas'));
const baseURL = process.env.GEOSOLVE_E2E_BASE_URL ?? 'http://127.0.0.1:18094/';
const sha256 = (bytes) => createHash('sha256').update(bytes).digest('hex');
const catalog = JSON.parse(await readFile(join(repository, 'crates/geosolve-sketch-code/assets/bundled-sample-catalog.json'), 'utf8'));
await mkdir(output, { recursive: true });
const executablePath = process.env.GEOSOLVE_CHROMIUM_PATH ?? '/home/arduano/.nix-profile/bin/google-chrome';
const args = process.env.M94_HARDWARE === '1' ? ['--use-gl=angle', '--use-angle=vulkan', '--enable-features=Vulkan', '--disable-vulkan-surface', '--enable-gpu', '--ignore-gpu-blocklist'] : [];
const browser = await chromium.launch({ executablePath, args });
const browserSession = await browser.newBrowserCDPSession();
const system = await browserSession.send('SystemInfo.getInfo');
const provenance = {
  format: 'geosolve-m94-canvas-capture-v1', capturedAt: new Date().toISOString(),
  planningSource: execFileSync('git', ['rev-parse', '37e3915'], { cwd: repository, encoding: 'utf8' }).trim(),
  candidateSource: execFileSync('git', ['rev-parse', 'HEAD'], { cwd: repository, encoding: 'utf8' }).trim(),
  workingTree: execFileSync('git', ['status', '--porcelain'], { cwd: repository, encoding: 'utf8' }),
  baseURL,
  browser: browser.version(), executablePath, args, system, viewport: { width: 1440, height: 900 }, deviceScaleFactor: 1,
  captures: [],
};
await writeFile(join(output, 'provenance.json'), JSON.stringify(provenance, null, 2));
const representative = new Set(['theo-jansen-leg', 'pc-water-manifold', 'curves-contact-continuity-atlas', 'fabrication-operations-atlas', 'perforated-fixture-field', 'robotic-harness-backplane']);
const settle = (page) => page.evaluate(() => new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve))));
try {
  for (const sample of catalog.samples.filter((sample) => !process.env.M94_CAPTURE_SAMPLE || sample.key === process.env.M94_CAPTURE_SAMPLE)) {
    const context = await browser.newContext({ viewport: provenance.viewport, deviceScaleFactor: 1 });
    const page = await context.newPage();
    page.setDefaultTimeout(90000);
    const errors = [];
    page.on('pageerror', (error) => errors.push(error.message));
    await page.goto(baseURL, { waitUntil: 'networkidle' });
    await page.getByRole('button', { name: 'File menu' }).click();
    await page.getByRole('menuitem', { name: /Open/ }).click();
    await page.getByPlaceholder(`Search ${catalog.samples.length} samples…`).fill(sample.key);
    await page.getByRole('dialog', { name: 'Open project' }).getByRole('button').filter({ has: page.getByText(sample.title, { exact: true }) }).click();
    await expect(page.locator('header').getByText(sample.title, { exact: true })).toBeVisible();
    await page.getByRole('button', { name: 'design', exact: true }).click();
    await page.getByRole('button', { name: 'Fit sketch' }).click();
    await settle(page);
    const frame = page.locator('[role="application"] canvas[data-renderer="webgl2"]');
    const directory = join(output, sample.key);
    await mkdir(directory, { recursive: true });
    const capture = async (stage) => {
      await settle(page);
      await expect(frame).toHaveAttribute('data-render-state', 'ready').catch(async (error) => {
        await writeFile(join(directory, `${stage}.failure.json`), JSON.stringify(await frame.evaluate((canvas) => canvas.__geosolveRendererDiagnostics), null, 2));
        throw error;
      });
      const geometry = await frame.evaluate((canvas) => canvas.__geosolvePresentedFrame);
      const diagnostics = await frame.evaluate((canvas) => canvas.__geosolveRendererDiagnostics);
      if (!geometry.items.length || /NaN|Infinity/.test(JSON.stringify(geometry))) throw Error(`Invalid candidate ${sample.key}`);
      const screenshot = await page.screenshot({ path: join(directory, `${stage}.png`), fullPage: true });
      await frame.screenshot({ path: join(directory, `${stage}.canvas.png`) });
      await writeFile(join(directory, `${stage}.geometry.json`), JSON.stringify(geometry, null, 2));
      provenance.captures.push({ key: sample.key, stage, screenshotSha256: sha256(screenshot), geometrySha256: sha256(JSON.stringify(geometry)), elements: geometry.items.length, diagnostics });
      await writeFile(join(output, 'provenance.json'), JSON.stringify(provenance, null, 2));
    };
    await page.mouse.move(0, 0);
    await capture('fitted');
    if (representative.has(sample.key)) {
      const point = await frame.evaluate((canvas) => canvas.__geosolvePresentedFrame.items.find((item) => item.layer === 'points' && item.kind === 'circle' && item.interactive));
      const box = await frame.boundingBox();
      const viewBox = await frame.evaluate((canvas) => canvas.__geosolvePresentedFrame.viewBox);
      const scale = Math.min(box.width / viewBox[2], box.height / viewBox[3]);
      const cx = box.x + (box.width - viewBox[2] * scale) / 2 + (point.center[0] - viewBox[0]) * scale;
      const cy = box.y + (box.height - viewBox[3] * scale) / 2 + (point.center[1] - viewBox[1]) * scale;
      await page.mouse.move(cx, cy);
      await capture('hover');
      await page.mouse.click(cx, cy);
      await expect(page.getByRole('tabpanel').getByText('Ownership', { exact: true })).toBeVisible();
      await capture('selected');
      await page.mouse.wheel(0, -260);
      await page.waitForTimeout(200);
      await capture('zoom');
    }
    if (sample.key === 'pc-water-manifold') {
      await page.getByRole('button', { name: 'Fit sketch' }).click();
      await page.getByRole('button', { name: 'Sketch', exact: true }).click();
      await page.getByRole('menuitem', { name: 'Segment' }).click();
      const box = await page.getByRole('application').boundingBox();
      await page.mouse.click(box.x + box.width * .35, box.y + box.height * .48);
      await page.mouse.move(box.x + box.width * .65, box.y + box.height * .55);
      await expect.poll(() => frame.evaluate((canvas) => canvas.__geosolvePresentedFrame.items.some((item) => item.layer === 'draft'))).toBe(true);
      await capture('drafting');
      await page.keyboard.press('Escape');
    }
    if (errors.length) throw Error(JSON.stringify(errors));
    await context.close();
    process.stdout.write(`${sample.key}: captured\n`);
  }
  provenance.completedAt = new Date().toISOString();
  await writeFile(join(output, 'provenance.json'), JSON.stringify(provenance, null, 2));
} finally { await browser.close(); }
