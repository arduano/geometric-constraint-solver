// SPDX-License-Identifier: GPL-3.0-or-later
import { createHash } from "node:crypto";
import { inflateSync } from "node:zlib";
import { expect, type Locator, type Page } from "@playwright/test";
import type { DrawFrame, DrawItem } from "../../src/lib/canvas-scene";

export const canvasFrame = (page: Page) => page.locator('[role="application"] canvas[data-renderer="webgl2"]');
export async function presentedFrame(canvas: Locator): Promise<DrawFrame> {
  await expect(canvas).toHaveAttribute("data-render-state", "ready");
  return canvas.evaluate((element) => {
    const scene = Reflect.get(element, "__geosolvePresentedFrame");
    if (!scene || scene.format !== "geosolve-draw-frame-v1") throw Error("No successfully presented canvas frame");
    return scene;
  });
}
export async function presentedIdentity(canvas: Locator) { return JSON.stringify(await presentedFrame(canvas)); }
export function canonicalSceneJson(scene: DrawFrame): string {
  const canonical = (value: unknown): unknown => Array.isArray(value) ? value.map(canonical)
    : value !== null && typeof value === "object"
      ? Object.fromEntries(Object.entries(value).sort(([a], [b]) => a < b ? -1 : a > b ? 1 : 0).map(([key, entry]) => [key, canonical(entry)]))
      : value;
  return JSON.stringify(canonical(scene));
}
export async function rendererDiagnostics(canvas: Locator): Promise<Record<string, unknown>> {
  return canvas.evaluate((element) => Reflect.get(element, "__geosolveRendererDiagnostics"));
}
export async function settlePresentation(page: Page) {
  const application = page.getByRole("application");
  await expect(application).toHaveAttribute("aria-busy", "false", { timeout: 60_000 });
  await page.evaluate(() => new Promise<void>((resolve) => {
    const raf = Reflect.get(globalThis, "requestAnimationFrame") as (callback: () => void) => void;
    raf(() => raf(resolve));
  }));
  await expect(application).toHaveAttribute("aria-busy", "false", { timeout: 60_000 });
  await presentedFrame(canvasFrame(page));
}

export interface ItemQuery { layer?: string; kind?: DrawItem["kind"]; className?: string; interactive?: boolean; persistentId?: string | null }
export function itemGeometry(item: DrawItem) {
  switch (item.kind) {
    case "polyline": return { kind: item.kind, points: item.points, closed: item.closed };
    case "circle": return { kind: item.kind, center: item.center, radius: item.radius };
    case "ellipse": return { kind: item.kind, center: item.center, radii: item.radii, rotation: item.rotation };
    case "rect": return { kind: item.kind, x: item.x, y: item.y, width: item.width, height: item.height, radius: item.radius };
    case "text": return { kind: item.kind, position: item.position, text: item.text, rotation: item.rotation };
  }
}
export function itemBounds(item: DrawItem) {
  let points: number[][];
  switch (item.kind) {
    case "polyline": points = item.points; break;
    case "circle": points = [[item.center[0] - item.radius, item.center[1] - item.radius], [item.center[0] + item.radius, item.center[1] + item.radius]]; break;
    case "ellipse": {
      const [a, b] = item.radii; const c = Math.cos(item.rotation); const s = Math.sin(item.rotation);
      const dx = Math.hypot(a * c, b * s); const dy = Math.hypot(a * s, b * c);
      points = [[item.center[0] - dx, item.center[1] - dy], [item.center[0] + dx, item.center[1] + dy]]; break;
    }
    case "rect": points = [[item.x, item.y], [item.x + item.width, item.y + item.height]]; break;
    case "text": throw Error("Text bounds require glyph metrics, not a geometry approximation");
  }
  if (!points.length || !points.flat().every(Number.isFinite)) throw Error("Empty/nonfinite presented geometry");
  const xs = points.map(([x]) => x); const ys = points.map(([, y]) => y);
  const x = Math.min(...xs); const y = Math.min(...ys);
  return { x, y, width: Math.max(...xs) - x, height: Math.max(...ys) - y };
}
export async function logicalToClient(canvas: Locator, position: { x: number; y: number }) {
  const scene = await presentedFrame(canvas); const box = await canvas.boundingBox();
  if (!box) throw Error("Canvas is not visible");
  const [x, y, width, height] = scene.viewBox;
  const scale = Math.min(box.width / width, box.height / height);
  return { x: box.x + (box.width - width * scale) / 2 + (position.x - x) * scale,
    y: box.y + (box.height - height * scale) / 2 + (position.y - y) * scale };
}
export async function fractionToClient(canvas: Locator, fraction: { x: number; y: number }) {
  const [x, y, width, height] = (await presentedFrame(canvas)).viewBox;
  return logicalToClient(canvas, { x: x + width * fraction.x, y: y + height * fraction.y });
}

// Semantic queries read only the renderer's last presented frame. They are not DOM
// locators, do not construct SVG elements, and never dispatch application actions.
export class PresentedItems {
  constructor(readonly canvas: Locator, readonly query: ItemQuery, readonly index?: number) {}
  async all(): Promise<DrawItem[]> {
    const q = this.query;
    const items = (await presentedFrame(this.canvas)).items.filter((item) =>
      (q.layer === undefined || item.layer === q.layer) && (q.kind === undefined || item.kind === q.kind)
      && (q.className === undefined || item.className.split(/\s+/).includes(q.className))
      && (q.interactive === undefined || item.interactive === q.interactive)
      && (q.persistentId === undefined || item.metadata.persistentId === q.persistentId));
    return this.index === undefined ? items : items.slice(this.index, this.index + 1);
  }
  async count() { return (await this.all()).length; }
  first() { return this.nth(0); }
  nth(index: number) { return new PresentedItems(this.canvas, this.query, index); }
  async read() { const items = await this.all(); if (items.length !== 1) throw Error(`Expected one presented item, found ${items.length}`); return items[0]; }
  async position(): Promise<[number, number]> { const item = await this.read(); if (item.kind !== "circle") throw Error("Expected circle point"); return item.center; }
  async geometryKey() { return JSON.stringify(itemGeometry(await this.read())); }
  async boundingBox() {
    const item = await this.read(); const bounds = itemBounds(item);
    const start = await logicalToClient(this.canvas, bounds);
    const end = await logicalToClient(this.canvas, { x: bounds.x + bounds.width, y: bounds.y + bounds.height });
    return { ...start, width: end.x - start.x, height: end.y - start.y };
  }
}
export const drawItems = (canvas: Locator, query: ItemQuery) => new PresentedItems(canvas, query);
export async function expectItemCount(items: PresentedItems, count: number) { await expect.poll(() => items.count()).toBe(count); }
export async function expectDraft(canvas: Locator, active: boolean) {
  await expect.poll(async () => (await presentedFrame(canvas)).items.some((item) => item.layer === "draft")).toBe(active);
}

// Decode the actual browser screenshot, including PNG row filters. No renderer
// snapshots or generated geometry are accepted as pixel evidence.
export function decodeScreenshot(png: Buffer) {
  expect(png.subarray(0, 8)).toEqual(Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]));
  let width = 0; let height = 0; let channels = 0; const chunks: Buffer[] = [];
  for (let offset = 8; offset < png.length;) {
    const size = png.readUInt32BE(offset); const type = png.toString("ascii", offset + 4, offset + 8);
    const data = png.subarray(offset + 8, offset + 8 + size);
    if (type === "IHDR") { width = data.readUInt32BE(0); height = data.readUInt32BE(4); expect(data[8]).toBe(8); channels = data[9] === 6 ? 4 : data[9] === 2 ? 3 : 0; expect(channels).toBeGreaterThan(0); expect(data[12]).toBe(0); }
    if (type === "IDAT") chunks.push(data);
    offset += size + 12;
  }
  const raw = inflateSync(Buffer.concat(chunks)); const stride = width * channels;
  expect(raw.length).toBe((stride + 1) * height);
  const pixels = Buffer.alloc(stride * height);
  const paeth = (a: number, b: number, c: number) => { const p = a + b - c; const pa = Math.abs(p - a); const pb = Math.abs(p - b); const pc = Math.abs(p - c); return pa <= pb && pa <= pc ? a : pb <= pc ? b : c; };
  for (let y = 0; y < height; y++) {
    const filter = raw[y * (stride + 1)]; expect(filter).toBeLessThanOrEqual(4);
    for (let x = 0; x < stride; x++) {
      const i = y * stride + x; const a = x >= channels ? pixels[i - channels] : 0; const b = y ? pixels[i - stride] : 0; const c = y && x >= channels ? pixels[i - stride - channels] : 0;
      const correction = [0, a, b, Math.floor((a + b) / 2), paeth(a, b, c)][filter];
      pixels[i] = (raw[y * (stride + 1) + 1 + x] + correction) & 255;
    }
  }
  return { width, height, channels, pixels };
}
export async function canvasVisualWitness(canvas: Locator) {
  const scene = await presentedFrame(canvas);
  const screenshot = await canvas.screenshot();
  const decoded = decodeScreenshot(screenshot); const box = await canvas.boundingBox();
  if (!box) throw Error("Canvas must be visible for pixel qualification");
  const points = scene.items.filter((item): item is DrawItem & { kind: "circle" } => item.layer === "points" && item.kind === "circle");
  expect(points.length).toBeGreaterThan(0);
  // Native paint order permits later filled markers and annotation masks to
  // cover earlier point rings. Sample exposed discs from the presented frame;
  // retain the same real-pixel and seven-sector assertions for selected rings.
  const exposed = points.filter((point) => !scene.items.slice(scene.items.indexOf(point) + 1).some((later) => {
    if (!later.style.fill || later.style.opacity <= 0) return false;
    if (later.kind === "circle") return Math.hypot(point.center[0] - later.center[0], point.center[1] - later.center[1])
      < point.radius + later.radius;
    if (later.kind === "rect") return point.center[0] + point.radius > later.x
      && point.center[0] - point.radius < later.x + later.width
      && point.center[1] + point.radius > later.y
      && point.center[1] - point.radius < later.y + later.height;
    return false;
  }));
  expect(exposed.length, "presented scene must contain exposed point markers").toBeGreaterThan(0);
  const sampled = exposed.filter((_, index) => index % Math.max(1, Math.floor(exposed.length / 8)) === 0).slice(0, 8);
  const samples = [];
  for (const point of sampled) {
    const client = await logicalToClient(canvas, { x: point.center[0], y: point.center[1] });
    const x = Math.round((client.x - box.x) * decoded.width / box.width); const y = Math.round((client.y - box.y) * decoded.height / box.height);
    const radius = Math.ceil(8 * decoded.width / box.width); let brightPixels = 0;
    for (let dy = -radius; dy <= radius; dy++) for (let dx = -radius; dx <= radius; dx++) {
      const px = x + dx; const py = y + dy;
      if (px < 0 || py < 0 || px >= decoded.width || py >= decoded.height) continue;
      const i = (py * decoded.width + px) * decoded.channels;
      if (Math.max(decoded.pixels[i], decoded.pixels[i + 1], decoded.pixels[i + 2]) > 100) brightPixels++;
    }
    expect(brightPixels, `presented point ${point.id} must have real visible canvas pixels`).toBeGreaterThan(2);
    const logicalScale = Math.min(box.width / scene.viewBox[2], box.height / scene.viewBox[3]);
    const ringRadius = point.radius * logicalScale * decoded.width / box.width;
    let ringSectors = 0;
    for (let sector = 0; sector < 12; sector++) {
      const angle = sector * Math.PI / 6; let visible = false;
      for (let radial = -2; radial <= 2; radial++) {
        const px = Math.round(x + Math.cos(angle) * (ringRadius + radial));
        const py = Math.round(y + Math.sin(angle) * (ringRadius + radial));
        if (px < 0 || py < 0 || px >= decoded.width || py >= decoded.height) continue;
        const i = (py * decoded.width + px) * decoded.channels;
        visible ||= Math.max(decoded.pixels[i], decoded.pixels[i + 1], decoded.pixels[i + 2]) > 100;
      }
      if (visible) ringSectors++;
    }
    expect(ringSectors, `presented point ${point.id} must paint its circular marker`).toBeGreaterThanOrEqual(7);
    samples.push({ itemId: point.id, x, y, brightPixels, ringSectors });
  }
  const { backend, state, width, height, pixelRatio, rasterResolution, hardware } = await rendererDiagnostics(canvas);
  // Timings/resource counters are observed separately; the initial-state witness
  // retains every stable drawing, pixel, surface and execution-environment input.
  return { format: "geosolve-canvas-visual-v1", screenshotSha256: createHash("sha256").update(screenshot).digest("hex"), width: decoded.width, height: decoded.height, samples,
    diagnostics: { backend, state, width, height, pixelRatio, rasterResolution, hardware } };
}
