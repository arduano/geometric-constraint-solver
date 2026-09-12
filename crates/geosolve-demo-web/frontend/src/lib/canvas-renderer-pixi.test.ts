// SPDX-License-Identifier: GPL-3.0-or-later
import { describe, expect, it, vi } from "vitest";
import type { DrawFrame, DrawItem, DrawStyle } from "./canvas-scene";

const fake = vi.hoisted(() => ({ shaderBind: vi.fn(), filters: [] as { blurXFilter: {destroy: ReturnType<typeof vi.fn>}; blurYFilter: {destroy: ReturnType<typeof vi.fn>}; destroy: ReturnType<typeof vi.fn> }[], uniformSystems: [] as Record<string, unknown>[], graphics: [] as { destroyed: boolean; destroyOptions?: unknown; clears: number; filters: unknown[]; parent: { position: { set: ReturnType<typeof vi.fn> } } | null; stroke: ReturnType<typeof vi.fn> }[], texts: [] as { style: unknown; resolution: number }[], shapeFill: vi.fn(), textureCreate: vi.fn(), textures: [] as { destroy: ReturnType<typeof vi.fn> }[], resize: vi.fn(), render: vi.fn(), rendererDestroy: vi.fn(), reorder: vi.fn() }));
vi.mock("pixi.js/filters", () => ({}));
vi.mock("pixi.js", () => {
  class Container {
    children: Container[] = []; destroyed = false; alpha = 1; rotation = 0;
    destroyOptions?: { children?: boolean; context?: boolean };
    parent: Container | null = null; position = { set: vi.fn() }; anchor = { set: vi.fn() }; pivot = { y: 0, set: vi.fn() };
    addChild(child: Container) { this.children.push(child); child.parent = this; return child; }
    addChildAt(child: Container, index: number) { this.children.splice(index, 0, child); child.parent = this; return child; }
    setChildIndex(child: Container, index: number) { fake.reorder(); const old = this.children.indexOf(child); this.children.splice(old, 1); this.children.splice(index, 0, child); }
    destroy(options?: { children?: boolean; context?: boolean }) { this.destroyed = true; this.destroyOptions = options; if (options?.children) [...this.children].forEach((child) => child.destroy(options)); if (this.parent) this.parent.children.splice(this.parent.children.indexOf(this), 1); }
  }
  class Graphics extends Container {
    clears = 0; filters = []; context = { translate: () => this.context, rotate: () => this.context, ellipse: () => this.context, resetTransform: () => this.context };
    constructor() { super(); fake.graphics.push(this); }
    clear() { this.clears++; return this; }
    moveTo() { return this; } lineTo() { return this; } closePath() { return this; }
    circle() { return this; } roundRect() { return this; } fill() { fake.shapeFill(); return this; } stroke = vi.fn(() => this);
  }
  class Text extends Container { text = ""; style = {}; resolution = 1; constructor() { super(); fake.texts.push(this); } }
  class RenderTexture { destroy = vi.fn(); static create(options: unknown) { fake.textureCreate(options); const texture = new RenderTexture(); fake.textures.push(texture); return texture; } }
  return { VERSION: "8.20.1", Container, Graphics, Text, RenderTexture, TextStyle: class {}, CanvasTextMetrics: { measureText: () => ({ height: 12, lineHeight: 12, maxLineWidth: 40, fontProperties: { ascent: 10, descent: 2, fontSize: 12 } }) }, Color: class { alpha = 1; toNumber() { return 0xffffff; } },
    BlurFilter: class { strength = 0; blurXFilter = {destroy:vi.fn()}; blurYFilter = {destroy:vi.fn()}; destroy=vi.fn(); constructor(){fake.filters.push(this);} }, WebGLRenderer: class {
      uniformGroup: Record<string, unknown> = { _cache: { stale: () => undefined }, _uniformGroupSyncHash: { stale: () => undefined } }; constructor() { fake.uniformSystems.push(this.uniformGroup); }
      shader = {bind:fake.shaderBind}; context = { webGLVersion: 2 }; events = undefined; scheduler = { destroy: vi.fn() }; gc = { run: vi.fn() }; background = { color: "" }; init = async () => undefined; resize = fake.resize; render = fake.render; destroy = fake.rendererDestroy;
    } };
});
import { createPixiBackend } from "./canvas-renderer-pixi";

const style: DrawStyle = { fill: null, stroke: "#ffffff", strokeWidth: 2, dash: [], opacity: 1, lineCap: "round", lineJoin: "round", nonScalingStroke: true, fontFamily: "sans-serif", fontSize: 12, fontWeight: 400, textAnchor: "start", textBaseline: "alphabetic", letterSpacing: 0, shadow: null };
const item = (id: string, radius = 4): DrawItem => ({ id, layer: "geometry", semanticKey: id, accessibleLabel: null, className: "wb-point", interactive: true, metadata: {}, style, kind: "circle", center: [20, 30], radius });
const frame = (items: DrawItem[]): DrawFrame => ({ format: "geosolve-draw-frame-v1", viewBox: [0, 0, 1000, 700], background: "#151617", provenance: {}, items });
const surface = { width: 1000, height: 700, pixelRatio: 1 };
function readyGl() {
  return { getExtension: () => null, getParameter: () => "test-double", isContextLost: () => false, flush() {}, getError: () => 0, NO_ERROR: 0,
    fenceSync: () => ({}), clientWaitSync: () => 0x911a, deleteSync() {}, SYNC_GPU_COMMANDS_COMPLETE:0x9117,TIMEOUT_EXPIRED:0x911b,ALREADY_SIGNALED:0x911a,CONDITION_SATISFIED:0x911c,WAIT_FAILED:0x911d };
}
async function fencedBackend(){
  const canvas=document.createElement("canvas"),sync={};
  const gl={...readyGl(),fenceSync:vi.fn<()=>object|null>(()=>sync),clientWaitSync:vi.fn(()=>0x911b),deleteSync:vi.fn(),getError:vi.fn(()=>0),flush:vi.fn(),isContextLost:vi.fn(()=>false)};
  vi.spyOn(canvas,"getContext").mockReturnValue(gl as never);
  return {canvas,gl,sync,backend:await createPixiBackend(canvas)};
}

describe("Pixi resource ownership (GPU calls replaced, no claim of WebGL qualification)", () => {
  it("prepares blur composition for the actual canvas target before clearing it with the native scene", async () => {
    const canvas = document.createElement("canvas");
    vi.spyOn(canvas, "getContext").mockReturnValue(readyGl() as never);
    const backend = await createPixiBackend(canvas), before = fake.render.mock.calls.length;
    try {
      await backend.render(frame([item("a")]), surface);
      const draws = fake.render.mock.calls.slice(before).map(([options]) => options);
      expect(draws).toHaveLength(3);
      expect(draws[0].target).toBeDefined();
      expect(draws[1]).toMatchObject({ container: draws[0].container, clear: true });
      expect(draws[1].target).toBeUndefined();
      expect(draws[2].container).not.toBe(draws[1].container);
      expect(draws[2].target).toBeUndefined();
    } finally { backend.destroy(); }
  });
  it("draws blur preparation offscreen once per context and validates it together with the native frame", async () => {
    vi.useFakeTimers(); const f = await fencedBackend(), before = fake.render.mock.calls.length;
    const graphicsBefore = fake.graphics.length, texturesBefore = fake.textures.length;
    try {
      expect(fake.textures).toHaveLength(texturesBefore);
      const outcome = Promise.resolve(f.backend.render(frame([item("a")]), surface));
      void outcome.catch(() => undefined); // Cleanup may reject after an earlier assertion failure.
      const draws = fake.render.mock.calls.slice(before);
      expect(draws).toHaveLength(3);
      const [warm, canvasWarm, native] = draws.map(([options]) => options);
      expect(warm.target).toBe(fake.textures.at(-1)); expect(warm.clear).toBe(true);
      expect(canvasWarm.target).toBeUndefined(); expect(canvasWarm.container).toBe(warm.container);
      expect(native.target).toBeUndefined(); expect(native.container).not.toBe(warm.container);
      expect(warm.container.destroyed).toBe(true); expect(native.container.destroyed).toBe(false);
      const preparatoryShape = fake.graphics[graphicsBefore];
      expect(preparatoryShape.filters).toEqual([fake.filters.at(-1)]); expect(preparatoryShape.destroyed).toBe(true);
      expect(preparatoryShape.destroyOptions).toMatchObject({ context: true });
      expect(fake.textures.at(-1)!.destroy).toHaveBeenCalledExactlyOnceWith(true);
      expect(f.gl.fenceSync).toHaveBeenCalledOnce(); expect(f.gl.getError).not.toHaveBeenCalled();
      let resolved = false; void outcome.then(() => { resolved = true; });
      await vi.advanceTimersByTimeAsync(20); expect(resolved).toBe(false);
      f.gl.clientWaitSync.mockReturnValue(f.gl.ALREADY_SIGNALED); await vi.advanceTimersByTimeAsync(4);
      await expect(outcome).resolves.toMatchObject({ resources: 1, created: 1, destroyed: 0 });
      const ordinary = Promise.resolve(f.backend.render(frame([item("b")]), surface));
      await vi.advanceTimersByTimeAsync(0); await ordinary;
      expect(fake.render.mock.calls.length - before).toBe(4); expect(fake.textures.length - texturesBefore).toBe(1);
      f.canvas.dispatchEvent(new Event("webglcontextrestored"));
      const restored = Promise.resolve(f.backend.render(frame([item("c")]), surface));
      await vi.advanceTimersByTimeAsync(0); await restored;
      expect(fake.render.mock.calls.length - before).toBe(7); expect(fake.textures.length - texturesBefore).toBe(2);
      expect(fake.textures.at(-1)!.destroy).toHaveBeenCalledExactlyOnceWith(true);
    } finally { f.backend.destroy(); vi.useRealTimers(); }
  });
  for (const reason of ["geometry", "allocation", "draw", "context loss", "canvas draw", "canvas context loss"] as const) it(`cleans offscreen preparation after ${reason} failure and retries only after restoration`, async () => {
    vi.useFakeTimers(); const f = await fencedBackend();
    const graphicsBefore = fake.graphics.length, texturesBefore = fake.textures.length;
    try {
      if (reason === "geometry") fake.shapeFill.mockImplementationOnce(() => { throw Error("geometry preparation failed"); });
      else if (reason === "allocation") fake.textureCreate.mockImplementationOnce(() => { throw Error("target allocation failed"); });
      else {
        if (reason.startsWith("canvas")) fake.render.mockImplementationOnce(() => undefined);
        fake.render.mockImplementationOnce(() => {
          if (reason.endsWith("draw")) throw Error("preparation draw failed");
          f.gl.isContextLost.mockReturnValue(true);
        });
      }
      expect(() => {
        const result = f.backend.render(frame([item("a")]), surface);
        if (result instanceof Promise) void result.catch(() => undefined);
      }).toThrow();
      expect(fake.graphics[graphicsBefore].destroyed).toBe(true);
      expect(fake.graphics[graphicsBefore].destroyOptions).toMatchObject({ context: true });
      if (reason !== "geometry" && reason !== "allocation") expect(fake.textures.at(-1)!.destroy).toHaveBeenCalledExactlyOnceWith(true);
      expect(f.gl.fenceSync).not.toHaveBeenCalled(); expect(f.gl.getError).not.toHaveBeenCalled();
      const draws = fake.render.mock.calls.length; f.gl.isContextLost.mockReturnValue(false);
      expect(() => f.backend.render(frame([item("a")]), surface)).toThrow(); expect(fake.render).toHaveBeenCalledTimes(draws);
      f.canvas.dispatchEvent(new Event("webglcontextrestored")); f.gl.clientWaitSync.mockReturnValue(f.gl.ALREADY_SIGNALED);
      const restored = Promise.resolve(f.backend.render(frame([item("a")]), surface));
      await vi.advanceTimersByTimeAsync(0); await expect(restored).resolves.toMatchObject({ resources: 1 });
      expect(fake.render.mock.calls.length - draws).toBe(3);
      expect(fake.textures.length - texturesBefore).toBe(reason === "allocation" || reason === "geometry" ? 1 : 2);
      expect(fake.textures.at(-1)!.destroy).toHaveBeenCalledExactlyOnceWith(true);
    } finally { f.backend.destroy(); vi.useRealTimers(); }
  });
  it("changes only the raster surface during interaction and retains static text textures and CSS strokes", async () => {
    const canvas = document.createElement("canvas");
    vi.spyOn(canvas, "getContext").mockReturnValue(readyGl() as never);
    const backend = await createPixiBackend(canvas);
    try {
      const text: DrawItem = { ...item("label"), kind: "text", position: [10, 10], text: "12 mm", rotation: 0 };
      const drawing = frame([item("a"), text]);
      expect(await backend.render(drawing, surface)).toMatchObject({ rasterResolution: 2, repainted: 2 });
      const graphic = fake.graphics.at(-1)!, label = fake.texts.at(-1)!, rasterStyle = label.style;
      expect(fake.resize).toHaveBeenLastCalledWith(1000, 700, 2); expect(label.resolution).toBe(2);
      expect(await backend.render(drawing, surface, "interactive")).toMatchObject({ rasterResolution: 1, repainted: 2, updated: 2 });
      expect(fake.resize).toHaveBeenLastCalledWith(1000, 700, 1);
      expect(label.style).toBe(rasterStyle); expect(label.resolution).toBe(2); expect(graphic.clears).toBe(1);
      expect(graphic.stroke).toHaveBeenLastCalledWith(expect.objectContaining({ width: 2 }));
      expect(await backend.render(drawing, surface, "settled")).toMatchObject({ rasterResolution: 2, repainted: 2, updated: 2 });
      expect(fake.resize).toHaveBeenLastCalledWith(1000, 700, 2);
      expect(label.style).toBe(rasterStyle); expect(label.resolution).toBe(2); expect(graphic.clears).toBe(1);
      const highDpr = { ...surface, pixelRatio: 3 };
      expect(await backend.render(drawing, highDpr, "interactive")).toMatchObject({ rasterResolution: 3, repainted: 3 });
      expect(label.resolution).toBe(3); const highStyle = label.style, resizes = fake.resize.mock.calls.length;
      expect(await backend.render(drawing, highDpr, "settled")).toMatchObject({ rasterResolution: 3, repainted: 3 });
      expect(fake.resize).toHaveBeenCalledTimes(resizes); expect(label.style).toBe(highStyle);
      canvas.dispatchEvent(new Event("webglcontextrestored"));
      expect(await backend.render(drawing, surface, "interactive")).toMatchObject({ rasterResolution: 1, resources: 2, created: 4, destroyed: 2 });
      expect(fake.resize).toHaveBeenLastCalledWith(1000, 700, 1); expect(fake.texts.at(-1)!.resolution).toBe(2);
    } finally { backend.destroy(); }
  });
  it("prepares interaction shader programs at first presentation and again after context restoration",async()=>{
    vi.useFakeTimers();const f=await fencedBackend(),before=fake.shaderBind.mock.calls.length;
    try{
      expect(fake.shaderBind.mock.calls.length).toBe(before); // Backend must be installed first.
      const first=Promise.resolve(f.backend.render(frame([item("a")]),surface));
      const filter=fake.filters.at(-1)!;
      expect(fake.shaderBind.mock.calls.slice(before)).toEqual([[filter.blurXFilter,true],[filter.blurYFilter,true]]);
      expect(f.gl.getError).not.toHaveBeenCalled();
      f.gl.clientWaitSync.mockReturnValue(f.gl.ALREADY_SIGNALED);await vi.advanceTimersByTimeAsync(0);await first;
      const second=Promise.resolve(f.backend.render(frame([item("b")]),surface));await vi.advanceTimersByTimeAsync(0);await second;
      expect(fake.shaderBind.mock.calls.length).toBe(before+2);
      f.canvas.dispatchEvent(new Event("webglcontextrestored"));
      const restored=Promise.resolve(f.backend.render(frame([item("c")]),surface));await vi.advanceTimersByTimeAsync(0);await restored;
      expect(fake.shaderBind.mock.calls.slice(before+2)).toEqual([[filter.blurXFilter,true],[filter.blurYFilter,true]]);
      f.backend.destroy();
      expect(filter.blurXFilter.destroy).toHaveBeenCalledExactlyOnceWith(false);
      expect(filter.blurYFilter.destroy).toHaveBeenCalledExactlyOnceWith(false);
      expect(filter.destroy).toHaveBeenCalledExactlyOnceWith(false);
    }finally{f.backend.destroy();vi.useRealTimers();}
  });
  for(const reason of ["exception","context loss"])it(`rejects ${reason} during initial shader preparation and retries only after restoration`,async()=>{
    vi.useFakeTimers();const f=await fencedBackend(),renders=fake.render.mock.calls.length;
    try{
      fake.shaderBind.mockImplementationOnce(()=>{if(reason==="exception")throw Error("shader setup failed");f.gl.isContextLost.mockReturnValue(true);});
      expect(()=>f.backend.render(frame([item("a")]),surface)).toThrow();
      expect(fake.render).toHaveBeenCalledTimes(renders);expect(f.gl.fenceSync).not.toHaveBeenCalled();expect(f.gl.getError).not.toHaveBeenCalled();
      f.gl.isContextLost.mockReturnValue(false);
      expect(()=>f.backend.render(frame([item("b")]),surface)).toThrow();
      f.canvas.dispatchEvent(new Event("webglcontextrestored"));f.gl.clientWaitSync.mockReturnValue(f.gl.ALREADY_SIGNALED);
      const restored=Promise.resolve(f.backend.render(frame([item("b")]),surface));await vi.advanceTimersByTimeAsync(0);await expect(restored).resolves.toMatchObject({resources:1});
    }finally{f.backend.destroy();vi.useRealTimers();}
  });
  it("validates a completed fence after a background tab delays its first poll past the deadline",async()=>{
    vi.useFakeTimers();const clock=vi.spyOn(performance,"now").mockReturnValue(0),f=await fencedBackend();
    try{
      const outcome=Promise.resolve(f.backend.render(frame([item("a")]),surface));
      clock.mockReturnValue(6_000);f.gl.clientWaitSync.mockReturnValue(f.gl.CONDITION_SATISFIED);
      await vi.advanceTimersByTimeAsync(0);
      await expect(outcome).resolves.toMatchObject({resources:1});
      expect(f.gl.getError).toHaveBeenCalledOnce();expect(f.gl.deleteSync).toHaveBeenCalledOnce();expect(vi.getTimerCount()).toBe(0);
    }finally{f.backend.destroy();clock.mockRestore();vi.useRealTimers();}
  });
  for(const reason of ["exception","context loss"])it(`retains failure after ${reason} during Pixi submission before a fence exists`,async()=>{
    vi.useFakeTimers();const f=await fencedBackend();
    try{
      fake.render.mockImplementationOnce(() => undefined).mockImplementationOnce(() => undefined); // Both preparation targets succeed first.
      fake.render.mockImplementationOnce(()=>{if(reason==="exception")throw Error("submission failed");f.gl.isContextLost.mockReturnValue(true);});
      expect(()=>f.backend.render(frame([item("a")]),surface)).toThrow();
      expect(f.gl.fenceSync).not.toHaveBeenCalled();expect(f.gl.getError).not.toHaveBeenCalled();
      const submissions=fake.render.mock.calls.length;f.gl.isContextLost.mockReturnValue(false);
      expect(()=>f.backend.render(frame([item("b")]),surface)).toThrow();expect(fake.render).toHaveBeenCalledTimes(submissions);
      f.canvas.dispatchEvent(new Event("webglcontextrestored"));f.gl.clientWaitSync.mockReturnValue(f.gl.ALREADY_SIGNALED);
      const restored=Promise.resolve(f.backend.render(frame([item("b")]),surface));await vi.advanceTimersByTimeAsync(0);
      await expect(restored).resolves.toMatchObject({resources:1});expect(f.gl.getError).toHaveBeenCalledOnce();expect(vi.getTimerCount()).toBe(0);
    }finally{f.backend.destroy();vi.useRealTimers();}
  });
  it("yields while the GPU fence is pending and reads errors only after completion", async () => {
    vi.useFakeTimers();
    const canvas = document.createElement("canvas"), sync = {};
    let status = 0x911b;
    const gl = { getExtension: () => null, getParameter: () => "test-double", isContextLost: () => false,
      flush:vi.fn(),getError:vi.fn(()=>0),fenceSync:vi.fn(()=>sync),clientWaitSync:vi.fn(()=>status),deleteSync:vi.fn(),
      NO_ERROR:0,SYNC_GPU_COMMANDS_COMPLETE:0x9117,TIMEOUT_EXPIRED:0x911b,ALREADY_SIGNALED:0x911a,CONDITION_SATISFIED:0x911c,WAIT_FAILED:0x911d };
    vi.spyOn(canvas,"getContext").mockReturnValue(gl as never);
    const backend = await createPixiBackend(canvas);
    try {
      const rendered = Promise.resolve(backend.render(frame([item("a")]),surface));
      expect(gl.getError).not.toHaveBeenCalled();
      await vi.advanceTimersByTimeAsync(20);
      expect(gl.getError).not.toHaveBeenCalled();
      expect(gl.clientWaitSync).toHaveBeenCalledWith(sync,0,0);
      status=gl.CONDITION_SATISFIED;
      await vi.advanceTimersByTimeAsync(20);
      await expect(rendered).resolves.toMatchObject({resources:1});
      expect(gl.getError).toHaveBeenCalledOnce();expect(gl.deleteSync).toHaveBeenCalledOnce();
      expect(vi.getTimerCount()).toBe(0);
    } finally { backend.destroy();vi.useRealTimers(); }
  });
  for(const reason of ["loss","destroy"])it(`cancels a pending GPU check on ${reason} without reading errors or leaving timers`,async()=>{
    vi.useFakeTimers();const f=await fencedBackend();
    try{
      const outcome=Promise.resolve(f.backend.render(frame([item("a")]),surface)).catch(error=>error);
      await vi.advanceTimersByTimeAsync(16);expect(f.gl.getError).not.toHaveBeenCalled();
      if(reason==="loss")f.canvas.dispatchEvent(new Event("webglcontextlost"));else f.backend.destroy();
      expect(await outcome).toBeInstanceOf(Error);const calls=f.gl.clientWaitSync.mock.calls.length;
      await vi.advanceTimersByTimeAsync(10_000);
      expect(f.gl.clientWaitSync).toHaveBeenCalledTimes(calls);expect(f.gl.getError).not.toHaveBeenCalled();expect(f.gl.deleteSync).toHaveBeenCalledOnce();expect(vi.getTimerCount()).toBe(0);
    }finally{f.backend.destroy();vi.useRealTimers();}
  });
  for(const failure of ["null fence","wait failure","unknown status","flush throw","poll throw","GPU error","deadline"])it(`fails closed on ${failure} until the context is restored`,async()=>{
    vi.useFakeTimers();const f=await fencedBackend();
    try{
      if(failure==="null fence")f.gl.fenceSync.mockReturnValueOnce(null);
      if(failure==="wait failure")f.gl.clientWaitSync.mockReturnValue(f.gl.WAIT_FAILED);
      if(failure==="unknown status")f.gl.clientWaitSync.mockReturnValue(-1);
      if(failure==="flush throw")f.gl.flush.mockImplementationOnce(()=>{throw Error("flush failed");});
      if(failure==="poll throw")f.gl.clientWaitSync.mockImplementation(()=>{throw Error("poll failed");});
      if(failure==="GPU error"){f.gl.clientWaitSync.mockReturnValue(f.gl.ALREADY_SIGNALED);f.gl.getError.mockReturnValue(0x0502);}
      const outcome=Promise.resolve().then(()=>f.backend.render(frame([item("a")]),surface)).catch(error=>error);
      await vi.advanceTimersByTimeAsync(failure==="deadline"?5_016:16);
      expect(await outcome).toBeInstanceOf(Error);expect(vi.getTimerCount()).toBe(0);
      const renders=fake.render.mock.calls.length,fences=f.gl.fenceSync.mock.calls.length;
      expect(()=>f.backend.render(frame([item("b")]),surface)).toThrow();
      expect(fake.render).toHaveBeenCalledTimes(renders);expect(f.gl.fenceSync).toHaveBeenCalledTimes(fences);
      if(failure!=="GPU error")expect(f.gl.getError).not.toHaveBeenCalled();
      expect(f.gl.deleteSync).toHaveBeenCalledTimes(failure==="null fence"?0:1);
      f.gl.clientWaitSync.mockImplementation(()=>f.gl.ALREADY_SIGNALED);f.gl.getError.mockReturnValue(0);
      f.canvas.dispatchEvent(new Event("webglcontextrestored"));
      const restored=Promise.resolve(f.backend.render(frame([item("b")]),surface));await vi.advanceTimersByTimeAsync(16);
      await expect(restored).resolves.toMatchObject({resources:1});expect(vi.getTimerCount()).toBe(0);
    }finally{f.backend.destroy();vi.useRealTimers();}
  });
  it("rejects overlapping submissions and context loss during the final error read without false success",async()=>{
    vi.useFakeTimers();const f=await fencedBackend();
    try{
      const outcome=Promise.resolve(f.backend.render(frame([item("a")]),surface)).catch(error=>error);
      expect(()=>f.backend.render(frame([item("b")]),surface)).toThrow("already in flight");expect(f.gl.fenceSync).toHaveBeenCalledOnce();
      f.gl.clientWaitSync.mockReturnValue(f.gl.CONDITION_SATISFIED);
      f.gl.getError.mockImplementation(()=>{f.gl.isContextLost.mockReturnValue(true);return 0;});
      await vi.advanceTimersByTimeAsync(16);expect(await outcome).toBeInstanceOf(Error);expect(f.gl.deleteSync).toHaveBeenCalledOnce();expect(vi.getTimerCount()).toBe(0);
    }finally{f.backend.destroy();vi.useRealTimers();}
  });
  it("reuses keyed primitives across reorder/update and releases removed and kind-replaced resources", async () => {
    const destroyedBefore = fake.rendererDestroy.mock.calls.length;
    const canvas = document.createElement("canvas");
    const gl = readyGl();
    vi.spyOn(canvas, "getContext").mockReturnValue(gl as never);
    const backend = await createPixiBackend(canvas);
    const before = fake.graphics.length;
    expect(await backend.render(frame([item("a"), item("b")]), surface)).toMatchObject({ resources: 2, created: 2, destroyed: 0, updated: 2 });
    const [preparation, a, b] = fake.graphics.slice(before);
    expect(preparation.destroyed).toBe(true);
    expect(await backend.render(frame([item("b"), item("a", 9)]), surface)).toMatchObject({ resources: 2, created: 2, destroyed: 0, updated: 3 });
    expect(a.clears).toBe(2); expect(b.clears).toBe(1);
    expect(await backend.render(frame([item("b")]), surface)).toMatchObject({ resources: 1, created: 2, destroyed: 1 });
    expect(a.destroyed).toBe(true); expect(b.destroyed).toBe(false);
    const text: DrawItem = { ...item("b"), kind: "text", position: [10, 10], text: "Radius", rotation: 0 };
    expect(await backend.render(frame([text]), surface)).toMatchObject({ resources: 1, created: 3, destroyed: 2 });
    expect(b.destroyed).toBe(true);
    const rasterStyle = fake.texts.at(-1)!.style;
    await backend.render(frame([{ ...text, position: [50, 60] }]), surface);
    expect(fake.texts.at(-1)!.style).toBe(rasterStyle);
    await backend.render(frame([{ ...text, text: "Diameter" }]), surface);
    expect(fake.texts.at(-1)!.style).not.toBe(rasterStyle);
    backend.destroy(); backend.destroy(); expect(fake.rendererDestroy).toHaveBeenCalledTimes(destroyedBefore + 1);
  });
  it("rebuilds lost-context resources and invalidates first-compile uniform caches exactly once", async () => {
    const canvas = document.createElement("canvas");
    const gl = readyGl();
    vi.spyOn(canvas, "getContext").mockReturnValue(gl as never);
    const backend = await createPixiBackend(canvas);
    const text: DrawItem = { ...item("label"), kind: "text", position: [10, 10], text: "X", rotation: 0 };
    const drawing = frame([item("a"), text]);
    await backend.render(drawing, surface);
    const oldGraphic = fake.graphics.at(-1)!; const oldText = fake.texts.at(-1)!;
    const uniforms = fake.uniformSystems.at(-1)!;
    canvas.dispatchEvent(new Event("webglcontextrestored"));
    expect(await backend.render(drawing, surface)).toMatchObject({ resources: 2, created: 4, destroyed: 2 });
    expect(oldGraphic.destroyed).toBe(true);
    expect(fake.texts.at(-1)).not.toBe(oldText);
    expect(Object.keys(uniforms._cache as object)).toEqual([]);
    expect(Object.keys(uniforms._uniformGroupSyncHash as object)).toEqual([]);
    expect(await backend.render(drawing, surface)).toMatchObject({ resources: 2, created: 4, destroyed: 2 });
    uniforms._cache = [];
    canvas.dispatchEvent(new Event("webglcontextrestored"));
    expect(() => backend.render(drawing, surface)).toThrow("Unsupported Pixi uniform-cache restoration contract");
    backend.destroy();
  });
  it("does not choose another rendering API when WebGL2 creation fails", async () => {
    const canvas = document.createElement("canvas"); const get = vi.spyOn(canvas, "getContext").mockReturnValue(null);
    await expect(createPixiBackend(canvas)).rejects.toThrow("WebGL2 is unavailable");
    expect(get).toHaveBeenCalledTimes(1); expect(get.mock.calls[0][0]).toBe("webgl2");
  });
  it("retains translated graphics and native paint order without rebuilding paths or scanning child order", async () => {
    const canvas = document.createElement("canvas");
    const gl = readyGl();
    vi.spyOn(canvas, "getContext").mockReturnValue(gl as never);
    const backend = await createPixiBackend(canvas);
    const before = fake.graphics.length; const reorders = fake.reorder.mock.calls.length;
    const shapes: DrawItem[] = [item("circle"),
      { ...item("line"), kind: "polyline", points: [[20, 30], [80, 60]], closed: false },
      { ...item("ellipse"), kind: "ellipse", center: [20, 30], radii: [4, 8], rotation: 0.2 },
      { ...item("rect"), kind: "rect", x: 20, y: 30, width: 40, height: 20, radius: 3 }];
    await backend.render(frame(shapes), surface);
    const [preparation, ...graphics] = fake.graphics.slice(before);
    expect(preparation.destroyed).toBe(true);
    const translated = shapes.map((shape): DrawItem => {
      if (shape.kind === "circle" || shape.kind === "ellipse") return { ...shape, center: [67, 9] };
      if (shape.kind === "rect") return { ...shape, x: 67, y: 9 };
      if (shape.kind === "polyline") return { ...shape, points: [[67, 9], [127, 39]] };
      return shape;
    });
    expect(await backend.render(frame(translated), surface)).toMatchObject({ repainted: 4, updated: 8 });
    expect(graphics.map((graphic) => graphic.clears)).toEqual([1, 1, 1, 1]);
    for (const graphic of graphics) expect(graphic.parent?.position.set).toHaveBeenLastCalledWith(67, 9);
    expect(fake.reorder.mock.calls.length).toBe(reorders);
    await backend.render(frame([...translated].reverse()), surface);
    expect(fake.reorder.mock.calls.length - reorders).toBe(3);
    expect(graphics.map((graphic) => graphic.clears)).toEqual([1, 1, 1, 1]);
    // Native geometry changes and different CSS mapping still rebuild the affected raster path.
    await backend.render(frame([{ ...translated[0], radius: 8 } as DrawItem, ...translated.slice(1)]), surface);
    expect(graphics.map((graphic) => graphic.clears)).toEqual([2, 1, 1, 1]);
    await backend.render(frame(translated), { ...surface, width: 500, height: 350 });
    expect(graphics.map((graphic) => graphic.clears)).toEqual([3, 2, 2, 2]);
    for (const graphic of graphics) expect(graphic.stroke).toHaveBeenLastCalledWith(expect.objectContaining({ width: 2 }));
    backend.destroy();
  });
  it("keeps shadow offsets local during translation and rebuilds dashed paint when style changes", async () => {
    const canvas = document.createElement("canvas");
    const gl = readyGl();
    vi.spyOn(canvas, "getContext").mockReturnValue(gl as never);
    const backend = await createPixiBackend(canvas);
    const before = fake.graphics.length;
    const glowing: DrawItem = { ...item("glow"), style: { ...style, dash: [3, 2], shadow: { color: "#ffffff", blur: 3, offset: [2, -4] } } };
    await backend.render(frame([glowing]), surface);
    const [preparation, content, shadow] = fake.graphics.slice(before);
    expect(preparation.destroyed).toBe(true);
    expect(await backend.render(frame([{ ...glowing, center: [30.125, 40.25] } as DrawItem]), surface)).toMatchObject({ repainted: 1 });
    expect(content.clears).toBe(1); expect(shadow.clears).toBe(1);
    expect(content.parent?.position.set).toHaveBeenLastCalledWith(30.125, 40.25);
    expect(shadow.parent).toBe(content.parent);
    const next: DrawItem = { ...glowing, style: { ...glowing.style, dash: [5, 4], shadow: null } };
    await backend.render(frame([next]), surface);
    expect(content.clears).toBe(2); expect(shadow.destroyed).toBe(true);
    backend.destroy();
  });
});
