// SPDX-License-Identifier: GPL-3.0-or-later
import { describe, expect, it, vi } from "vitest";
import type { DrawFrame, DrawItem, DrawStyle } from "./canvas-scene";

const fake = vi.hoisted(() => ({ graphics: [] as { destroyed: boolean; clears: number }[], texts: [] as { style: unknown }[], render: vi.fn(), rendererDestroy: vi.fn() }));
vi.mock("pixi.js/filters", () => ({}));
vi.mock("pixi.js", () => {
  class Container {
    children: Container[] = []; destroyed = false; alpha = 1; rotation = 0;
    parent: Container | null = null; position = { set: vi.fn() }; anchor = { set: vi.fn() }; pivot = { y: 0, set: vi.fn() };
    addChild(child: Container) { this.children.push(child); child.parent = this; return child; }
    addChildAt(child: Container, index: number) { this.children.splice(index, 0, child); child.parent = this; return child; }
    setChildIndex(child: Container, index: number) { const old = this.children.indexOf(child); this.children.splice(old, 1); this.children.splice(index, 0, child); }
    destroy(options?: { children?: boolean }) { this.destroyed = true; if (options?.children) [...this.children].forEach((child) => child.destroy()); if (this.parent) this.parent.children.splice(this.parent.children.indexOf(this), 1); }
  }
  class Graphics extends Container {
    clears = 0; filters = []; context = { translate: () => this.context, rotate: () => this.context, ellipse: () => this.context, resetTransform: () => this.context };
    constructor() { super(); fake.graphics.push(this); }
    clear() { this.clears++; return this; }
    moveTo() { return this; } lineTo() { return this; } closePath() { return this; }
    circle() { return this; } roundRect() { return this; } fill() { return this; } stroke() { return this; }
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
});
