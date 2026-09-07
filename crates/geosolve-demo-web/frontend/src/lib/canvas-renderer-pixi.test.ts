// SPDX-License-Identifier: GPL-3.0-or-later
import { describe, expect, it, vi } from "vitest";
import type { DrawFrame, DrawItem, DrawStyle } from "./canvas-scene";

const fake = vi.hoisted(() => ({ graphics: [] as { destroyed: boolean; clears: number; parent: { position: { set: ReturnType<typeof vi.fn> } } | null; stroke: ReturnType<typeof vi.fn> }[], texts: [] as { style: unknown }[], render: vi.fn(), rendererDestroy: vi.fn(), reorder: vi.fn() }));
vi.mock("pixi.js/filters", () => ({}));
vi.mock("pixi.js", () => {
  class Container {
    children: Container[] = []; destroyed = false; alpha = 1; rotation = 0;
    parent: Container | null = null; position = { set: vi.fn() }; anchor = { set: vi.fn() }; pivot = { y: 0, set: vi.fn() };
    addChild(child: Container) { this.children.push(child); child.parent = this; return child; }
    addChildAt(child: Container, index: number) { this.children.splice(index, 0, child); child.parent = this; return child; }
    setChildIndex(child: Container, index: number) { fake.reorder(); const old = this.children.indexOf(child); this.children.splice(old, 1); this.children.splice(index, 0, child); }
    destroy(options?: { children?: boolean }) { this.destroyed = true; if (options?.children) [...this.children].forEach((child) => child.destroy()); if (this.parent) this.parent.children.splice(this.parent.children.indexOf(this), 1); }
  }
  class Graphics extends Container {
    clears = 0; filters = []; context = { translate: () => this.context, rotate: () => this.context, ellipse: () => this.context, resetTransform: () => this.context };
    constructor() { super(); fake.graphics.push(this); }
    clear() { this.clears++; return this; }
    moveTo() { return this; } lineTo() { return this; } closePath() { return this; }
    circle() { return this; } roundRect() { return this; } fill() { return this; } stroke = vi.fn(() => this);
  }
  class Text extends Container { text = ""; style = {}; resolution = 1; constructor() { super(); fake.texts.push(this); } }
  return { Container, Graphics, Text, TextStyle: class {}, CanvasTextMetrics: { measureText: () => ({ height: 12, lineHeight: 12, maxLineWidth: 40, fontProperties: { ascent: 10, descent: 2, fontSize: 12 } }) }, Color: class { alpha = 1; toNumber() { return 0xffffff; } },
    BlurFilter: class { strength = 0; destroy() {} }, WebGLRenderer: class {
      context = { webGLVersion: 2 }; events = undefined; scheduler = { destroy: vi.fn() }; gc = { run: vi.fn() }; background = { color: "" }; init = async () => undefined; resize = vi.fn(); render = fake.render; destroy = fake.rendererDestroy;
    } };
});
import { createPixiBackend } from "./canvas-renderer-pixi";

const style: DrawStyle = { fill: null, stroke: "#ffffff", strokeWidth: 2, dash: [], opacity: 1, lineCap: "round", lineJoin: "round", nonScalingStroke: true, fontFamily: "sans-serif", fontSize: 12, fontWeight: 400, textAnchor: "start", textBaseline: "alphabetic", letterSpacing: 0, shadow: null };
const item = (id: string, radius = 4): DrawItem => ({ id, layer: "geometry", semanticKey: id, accessibleLabel: null, className: "wb-point", interactive: true, metadata: {}, style, kind: "circle", center: [20, 30], radius });
const frame = (items: DrawItem[]): DrawFrame => ({ format: "geosolve-draw-frame-v1", viewBox: [0, 0, 1000, 700], background: "#151617", provenance: {}, items });
const surface = { width: 1000, height: 700, pixelRatio: 1 };

describe("Pixi resource ownership (GPU calls replaced, no claim of WebGL qualification)", () => {
  it("reuses keyed primitives across reorder/update and releases removed and kind-replaced resources", async () => {
    const canvas = document.createElement("canvas");
    const gl = { getExtension: () => null, getParameter: () => "test-double", isContextLost: () => false, flush() {}, getError: () => 0, NO_ERROR: 0 };
    vi.spyOn(canvas, "getContext").mockReturnValue(gl as never);
    const backend = await createPixiBackend(canvas);
    const before = fake.graphics.length;
    expect(backend.render(frame([item("a"), item("b")]), surface)).toMatchObject({ resources: 2, created: 2, destroyed: 0, updated: 2 });
    const [a, b] = fake.graphics.slice(before);
    expect(backend.render(frame([item("b"), item("a", 9)]), surface)).toMatchObject({ resources: 2, created: 2, destroyed: 0, updated: 3 });
    expect(a.clears).toBe(2); expect(b.clears).toBe(1);
    expect(backend.render(frame([item("b")]), surface)).toMatchObject({ resources: 1, created: 2, destroyed: 1 });
    expect(a.destroyed).toBe(true); expect(b.destroyed).toBe(false);
    const text: DrawItem = { ...item("b"), kind: "text", position: [10, 10], text: "Radius", rotation: 0 };
    expect(backend.render(frame([text]), surface)).toMatchObject({ resources: 1, created: 3, destroyed: 2 });
    expect(b.destroyed).toBe(true);
    const rasterStyle = fake.texts.at(-1)!.style;
    backend.render(frame([{ ...text, position: [50, 60] }]), surface);
    expect(fake.texts.at(-1)!.style).toBe(rasterStyle);
    backend.render(frame([{ ...text, text: "Diameter" }]), surface);
    expect(fake.texts.at(-1)!.style).not.toBe(rasterStyle);
    backend.destroy(); backend.destroy(); expect(fake.rendererDestroy).toHaveBeenCalledTimes(1);
  });
  it("does not choose another rendering API when WebGL2 creation fails", async () => {
    const canvas = document.createElement("canvas"); const get = vi.spyOn(canvas, "getContext").mockReturnValue(null);
    await expect(createPixiBackend(canvas)).rejects.toThrow("WebGL2 is unavailable");
    expect(get).toHaveBeenCalledTimes(1); expect(get.mock.calls[0][0]).toBe("webgl2");
  });
  it("retains translated graphics and native paint order without rebuilding paths or scanning child order", async () => {
    const canvas = document.createElement("canvas");
    const gl = { getExtension: () => null, getParameter: () => "test-double", isContextLost: () => false, flush() {}, getError: () => 0, NO_ERROR: 0 };
    vi.spyOn(canvas, "getContext").mockReturnValue(gl as never);
    const backend = await createPixiBackend(canvas);
    const before = fake.graphics.length; const reorders = fake.reorder.mock.calls.length;
    const shapes: DrawItem[] = [item("circle"),
      { ...item("line"), kind: "polyline", points: [[20, 30], [80, 60]], closed: false },
      { ...item("ellipse"), kind: "ellipse", center: [20, 30], radii: [4, 8], rotation: 0.2 },
      { ...item("rect"), kind: "rect", x: 20, y: 30, width: 40, height: 20, radius: 3 }];
    backend.render(frame(shapes), surface);
    const graphics = fake.graphics.slice(before);
    const translated = shapes.map((shape): DrawItem => {
      if (shape.kind === "circle" || shape.kind === "ellipse") return { ...shape, center: [67, 9] };
      if (shape.kind === "rect") return { ...shape, x: 67, y: 9 };
      if (shape.kind === "polyline") return { ...shape, points: [[67, 9], [127, 39]] };
      return shape;
    });
    expect(backend.render(frame(translated), surface)).toMatchObject({ repainted: 4, updated: 8 });
    expect(graphics.map((graphic) => graphic.clears)).toEqual([1, 1, 1, 1]);
    for (const graphic of graphics) expect(graphic.parent?.position.set).toHaveBeenLastCalledWith(67, 9);
    expect(fake.reorder.mock.calls.length).toBe(reorders);
    backend.render(frame([...translated].reverse()), surface);
    expect(fake.reorder.mock.calls.length - reorders).toBe(3);
    expect(graphics.map((graphic) => graphic.clears)).toEqual([1, 1, 1, 1]);
    // Native geometry changes and different CSS mapping still rebuild the affected raster path.
    backend.render(frame([{ ...translated[0], radius: 8 } as DrawItem, ...translated.slice(1)]), surface);
    expect(graphics.map((graphic) => graphic.clears)).toEqual([2, 1, 1, 1]);
    backend.render(frame(translated), { ...surface, width: 500, height: 350 });
    expect(graphics.map((graphic) => graphic.clears)).toEqual([3, 2, 2, 2]);
    for (const graphic of graphics) expect(graphic.stroke).toHaveBeenLastCalledWith(expect.objectContaining({ width: 2 }));
    backend.destroy();
  });
  it("keeps shadow offsets local during translation and rebuilds dashed paint when style changes", async () => {
    const canvas = document.createElement("canvas");
    const gl = { getExtension: () => null, getParameter: () => "test-double", isContextLost: () => false, flush() {}, getError: () => 0, NO_ERROR: 0 };
    vi.spyOn(canvas, "getContext").mockReturnValue(gl as never);
    const backend = await createPixiBackend(canvas);
    const before = fake.graphics.length;
    const glowing: DrawItem = { ...item("glow"), style: { ...style, dash: [3, 2], shadow: { color: "#ffffff", blur: 3, offset: [2, -4] } } };
    backend.render(frame([glowing]), surface);
    const [content, shadow] = fake.graphics.slice(before);
    expect(backend.render(frame([{ ...glowing, center: [30.125, 40.25] } as DrawItem]), surface)).toMatchObject({ repainted: 1 });
    expect(content.clears).toBe(1); expect(shadow.clears).toBe(1);
    expect(content.parent?.position.set).toHaveBeenLastCalledWith(30.125, 40.25);
    expect(shadow.parent).toBe(content.parent);
    const next: DrawItem = { ...glowing, style: { ...glowing.style, dash: [5, 4], shadow: null } };
    backend.render(frame([next]), surface);
    expect(content.clears).toBe(2); expect(shadow.destroyed).toBe(true);
    backend.destroy();
  });
});
