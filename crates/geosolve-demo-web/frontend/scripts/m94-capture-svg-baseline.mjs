// SPDX-License-Identifier: GPL-3.0-or-later
// Read-only capture of the accepted immutable SVG artifact. This helper never builds or serves it.
import { chromium, expect } from '@playwright/test';
import { createHash } from 'node:crypto';
import { readFile, mkdir, writeFile } from 'node:fs/promises';
import { resolve, join } from 'node:path';
import { execFileSync } from 'node:child_process';

const repository = resolve(import.meta.dirname, '../../../..');
const output = resolve(repository, 'target/m94/baseline');
const artifact = '/tmp/geosolve-m92-uat._8s63qiy';
const baseURL = 'http://100.94.63.83:18092/';
const sha256 = (bytes) => createHash('sha256').update(bytes).digest('hex');
const manifest = await readFile(`${artifact}.sha256`);
if (sha256(manifest) !== '2dd2173040aebd99227aba488e2992ef864dd44ed2e9c95f709d5fc7884b20a9') throw Error('Unexpected accepted artifact manifest');
const files = [];
for (const line of manifest.toString().trim().split('\n')) {
  const [, hash, path] = /^(\w+)\s+\.\/(.+)$/.exec(line);
  const bytes = await readFile(join(artifact, path));
  if (sha256(bytes) !== hash) throw Error(`Accepted artifact mismatch: ${path}`);
  const response = await fetch(new URL(path, baseURL));
  if (!response.ok || sha256(Buffer.from(await response.arrayBuffer())) !== hash) throw Error(`Served artifact mismatch: ${path}`);
  files.push({ path, sha256: hash, bytes: bytes.length });
}
const catalog = JSON.parse(await readFile(join(repository, 'crates/geosolve-sketch-code/assets/bundled-sample-catalog.json'), 'utf8'));
await mkdir(output, { recursive: true });
const executablePath = process.env.GEOSOLVE_CHROMIUM_PATH ?? '/home/arduano/.nix-profile/bin/google-chrome';
const browser = await chromium.launch({ executablePath });
const browserSession = await browser.newBrowserCDPSession();
const system = await browserSession.send('SystemInfo.getInfo');
const provenance = {
  format: 'geosolve-m94-svg-baseline-v1', capturedAt: new Date().toISOString(),
  planningSource: execFileSync('git', ['rev-parse', '37e3915'], { cwd: repository, encoding: 'utf8' }).trim(),
  acceptedProductSource: 'b153a28d9e44dde934b325835913986b0e316ee7',
  artifact, baseURL, manifestSha256: sha256(manifest), files,
  browser: browser.version(), executablePath, system, viewport: { width: 1440, height: 900 }, deviceScaleFactor: 1,
  captures: [],
};
await writeFile(join(output, 'provenance.json'), JSON.stringify(provenance, null, 2));
const representative = new Set(['theo-jansen-leg', 'pc-water-manifold', 'curves-contact-continuity-atlas', 'fabrication-operations-atlas', 'perforated-fixture-field', 'robotic-harness-backplane']);
const settle = (page) => page.evaluate(() => new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve))));
try {
  for (const sample of catalog.samples) {
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
    const frame = page.locator('[role="application"] svg.geosolve-authoritative-frame');
    const directory = join(output, sample.key);
    await mkdir(directory, { recursive: true });
    const capture = async (stage) => {
      await settle(page);
      const geometry = await frame.evaluate((svg) => ({
        attributes: Object.fromEntries([...svg.attributes].map(({ name, value }) => [name, value])),
        elements: [...svg.querySelectorAll('.wb-geometry path, .wb-computed-geometry path, .wb-points > circle.wb-point')].map((element) => {
          const box = element.getBBox();
          return { tag: element.tagName, attributes: Object.fromEntries([...element.attributes].map(({ name, value }) => [name, value])), bounds: { x: box.x, y: box.y, width: box.width, height: box.height } };
        }),
        viewport: svg.getBoundingClientRect().toJSON(),
      }));
      if (!geometry.elements.length || /NaN|Infinity/.test(JSON.stringify(geometry))) throw Error(`Invalid baseline ${sample.key}`);
      const screenshot = await page.screenshot({ path: join(directory, `${stage}.png`), fullPage: true });
      const svg = await frame.evaluate((svg) => svg.outerHTML);
      await writeFile(join(directory, `${stage}.svg`), svg);
      await writeFile(join(directory, `${stage}.geometry.json`), JSON.stringify(geometry, null, 2));
      provenance.captures.push({ key: sample.key, stage, screenshotSha256: sha256(screenshot), svgSha256: sha256(svg), geometrySha256: sha256(JSON.stringify(geometry)), elements: geometry.elements.length });
      await writeFile(join(output, 'provenance.json'), JSON.stringify(provenance, null, 2));
    };
    await page.mouse.move(0, 0);
    await capture('fitted');
    if (representative.has(sample.key)) {
      const point = frame.locator('.wb-points > circle.wb-point[data-interactive="true"]').first();
      const box = await point.boundingBox();
      if (!box) throw Error('Missing painted interactive point');
      await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
      await capture('hover');
      await page.mouse.click(box.x + box.width / 2, box.y + box.height / 2);
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
      await expect(frame.locator('.wb-draft')).toHaveCount(1);
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
