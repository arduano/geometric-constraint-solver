// SPDX-License-Identifier: GPL-3.0-or-later
import { createHash } from "node:crypto";
import { mkdir, readFile, readdir, writeFile } from "node:fs/promises";
import { isAbsolute, join, resolve } from "node:path";
import { expect, type Page, type TestInfo } from "@playwright/test";
import { canvasFrame, canvasVisualWitness, canonicalSceneJson, drawItems, itemBounds, itemGeometry, presentedFrame, settlePresentation } from "./presented-canvas";

export interface Edit {
  declaration: string;
  path: Array<string | number>;
  replacement: number | { unit: string; value: number };
}

export const sampleRoot = resolve(import.meta.dirname, "../../../../geosolve-sketch-code/assets/bundled-samples");
export const catalogContract = JSON.parse(await readFile(resolve(sampleRoot, "../bundled-sample-catalog.json"), "utf8")) as {
  schema: number;
  samples: Array<{ key: string; title: string; category: string }>;
  retired_keys: string[];
};
expect(catalogContract.schema).toBe(1);
const directories = (await readdir(sampleRoot, { withFileTypes: true }))
  .filter((entry) => entry.isDirectory()).map((entry) => entry.name);
export const samples = await Promise.all(directories.map(async (key) => ({
  key,
  manifest: JSON.parse(await readFile(join(sampleRoot, key, "manifest.json"), "utf8")) as { ordinal: number; title: string; category: string },
  source: await readFile(join(sampleRoot, key, "sketch.ts"), "utf8"),
  witnesses: JSON.parse(await readFile(join(sampleRoot, key, "witnesses.json"), "utf8")) as {
    representative_edit: Edit; secondary_edit: Edit;
  },
})));
samples.sort((a, b) => a.manifest.ordinal - b.manifest.ordinal);
expect(samples.map(({ key, manifest }) => ({ key, title: manifest.title, category: manifest.category }))).toEqual(catalogContract.samples);

export const savedWorkspaceExpression = `(async () => {
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

export async function savedWorkspace(page: Page): Promise<string | null> {
  return page.evaluate<string | null>(savedWorkspaceExpression);
}

export async function acceptedSource(page: Page): Promise<string | null> {
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

// This geometry projection preserves the existing fitted-frame assertions. It is
// additional evidence; it never substitutes for the complete persisted-state hash.
export async function fittedGeometry(page: Page): Promise<string> {
  await page.getByRole("button", { name: "Fit sketch" }).click();
  await settlePresentation(page);
  const scene = await presentedFrame(canvasFrame(page));
  const elements = scene.items.filter((item) => ["geometry", "computed", "points"].includes(item.layer));
  if (elements.length === 0) throw new Error("accepted geometry is empty");
  const [x, y, width, height] = scene.viewBox;
  const tokens = elements.map((item) => {
    const box = itemBounds(item);
    if (box.x < x - 2 || box.y < y - 2 || box.x + box.width > x + width + 2 || box.y + box.height > y + height + 2) {
      throw new Error(`fitted geometry is clipped: ${JSON.stringify(item).slice(0, 240)}`);
    }
    return itemGeometry(item);
  });
  return JSON.stringify({ viewBox: scene.viewBox, tokens });
}

export async function openSamplePrefix(page: Page, sample: typeof samples[number], info?: TestInfo) {
  const { key, manifest, source } = sample;
  await page.getByRole("button", { name: "File menu" }).click();
  await page.getByRole("menuitem", { name: /Open/ }).click();
  await page.getByPlaceholder(`Search ${catalogContract.samples.length} samples…`).fill(key);
  await page.getByRole("dialog", { name: "Open project" }).getByRole("button")
    .filter({ has: page.getByText(manifest.title, { exact: true }) }).click();
  await expect(page.locator("header").getByText(manifest.title, { exact: true })).toBeVisible();
  await expect.poll(() => acceptedSource(page), { timeout: 60_000 }).toBe(source);
  await page.getByRole("button", { name: "design", exact: true }).click();
  const geometry = await fittedGeometry(page);
  const frame = canvasFrame(page);
  const acceptedScene = await presentedFrame(frame);
  expect(acceptedScene.provenance.scene).toBe("accepted");
  expect(acceptedScene.items.some((item) => item.layer === "geometry")).toBe(true);
  const points = drawItems(frame, { layer: "points", kind: "circle", interactive: true });
  const canvasVisual = await canvasVisualWitness(frame);
  expect(await points.count()).toBeGreaterThan(0);
  // One complete wire read at this boundary retains compressed session/history,
  // allocators, branches, identities and revisions byte for byte. No fields are
  // removed to manufacture a reuse match. Incidental drift requires fresh work.
  const wire = await savedWorkspace(page);
  expect(wire).not.toBeNull();
  const workbench = JSON.parse(JSON.parse(wire!).project);
  const project = JSON.parse(workbench.project);
  expect(workbench.origin).toEqual({ kind: "bundled", sample: key });
  expect(project.project).toBe(`geosolve-sample-${key}`);
  expect(project.managed.source).toBe(source);
  const sha256 = (value: string) => createHash("sha256").update(value).digest("hex");
  const witness = {
    format: "geosolve-browser-sample-prefix-v2",
    key,
    title: manifest.title,
    project: project.project as string,
    workspaceSha256: sha256(wire!),
    workspaceBytes: Buffer.byteLength(wire!),
    acceptedSourceSha256: sha256(source),
    fittedGeometrySha256: sha256(geometry),
    canonicalAcceptedSceneSha256: sha256(canonicalSceneJson(acceptedScene)),
    canvasVisual,
  };
  const output = process.env.GEOSOLVE_BROWSER_WITNESS_OUTPUT;
  if (output) {
    if (!isAbsolute(output)) throw new Error("GEOSOLVE_BROWSER_WITNESS_OUTPUT must be absolute");
    await mkdir(output, { recursive: true });
    await writeFile(join(output, `${key}.json`), `${JSON.stringify(witness, null, 2)}\n`, { flag: "wx" });
  }
  if (info) await info.attach("geosolve-sample-prefix", { body: Buffer.from(JSON.stringify(witness)), contentType: "application/json" });
  return { original: source, frame, points, witness };
}
