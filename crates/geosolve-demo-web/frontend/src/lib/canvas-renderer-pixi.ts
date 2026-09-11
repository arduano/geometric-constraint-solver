// SPDX-License-Identifier: GPL-3.0-or-later
import "pixi.js/filters";
import { BlurFilter, CanvasTextMetrics, Color, Container, Graphics, Text, TextStyle, VERSION, WebGLRenderer } from "pixi.js";
import type { DrawFrame, DrawItem, DrawPoint, DrawStyle } from "./canvas-scene";
import { dashedSegments, fitDrawing, textRasterOrigin } from "./canvas-renderer-geometry";

export interface CanvasSurface { width: number; height: number; pixelRatio: number }
export interface BackendStats { resources: number; created: number; destroyed: number; updated: number; repainted?: number; rasterResolution?: number }
export interface CanvasBackend {
  readonly hardware: Readonly<Record<string, string>>;
  render(frame: DrawFrame, surface: CanvasSurface): BackendStats | Promise<BackendStats>;
  destroy(): void;
}
interface Entry { container: Container; content: Graphics | Text; shadow: Graphics | null; blur: BlurFilter | null; item: DrawItem | null; kind: DrawItem["kind"]; scale: number; resolution: number; position: DrawPoint | null }

/** Pixi 8.20.1 retains generated uniform uploaders across context restoration.
 * Loss during shader compilation can cache empty uploaders from absent uniform
 * metadata. Recompile them for the restored context, preserving normal batching.
 * Keep this version-bound compatibility seam explicit until upstream fixes it.
 */
function invalidateRestoredUniformUploads(renderer: WebGLRenderer) {
  const system = renderer.uniformGroup as unknown as Record<string, unknown>;
  const caches = ["_cache", "_uniformGroupSyncHash"] as const;
  if (String(VERSION) !== "8.20.1" || !system || caches.some((key) => {
    const value = system[key];
    return !value || typeof value !== "object" || Array.isArray(value)
      || ![Object.prototype, null].includes(Object.getPrototypeOf(value));
  })) throw new Error("Unsupported Pixi uniform-cache restoration contract");
  for (const key of caches) system[key] = Object.create(null) as Record<string, unknown>;
}

function sameStyle(a: DrawStyle, b: DrawStyle): boolean {
  return a === b || a.fill === b.fill && a.stroke === b.stroke && a.strokeWidth === b.strokeWidth
    && a.opacity === b.opacity && a.lineCap === b.lineCap && a.lineJoin === b.lineJoin && a.nonScalingStroke === b.nonScalingStroke
    && a.fontFamily === b.fontFamily && a.fontSize === b.fontSize && a.fontWeight === b.fontWeight
    && a.textAnchor === b.textAnchor && a.textBaseline === b.textBaseline && a.letterSpacing === b.letterSpacing
    && a.dash.length === b.dash.length && a.dash.every((value, index) => value === b.dash[index])
    && (a.shadow === b.shadow || !!a.shadow && !!b.shadow && a.shadow.color === b.shadow.color
      && a.shadow.blur === b.shadow.blur && a.shadow.offset[0] === b.shadow.offset[0] && a.shadow.offset[1] === b.shadow.offset[1]);
}
function origin(item: DrawItem): DrawPoint {
  if (item.kind === "text") return item.position;
  if (item.kind === "rect") return [item.x, item.y];
  if (item.kind === "polyline") return item.points[0] ?? [0, 0];
  return item.center;
}
/** Compare the exact native-authored shape in local coordinates, excluding its position.
 * A camera translation can move retained GPU geometry without rebuilding stroke tessellation.
 * No tolerance or camera inference is used: an actual change still rebuilds its raster path.
 */
function sameLocalGeometry(a: DrawItem, b: DrawItem): boolean {
  if (a === b) return true;
  switch (a.kind) {
    case "circle": return b.kind === "circle" && a.radius === b.radius;
    case "ellipse": return b.kind === "ellipse" && a.radii[0] === b.radii[0] && a.radii[1] === b.radii[1] && a.rotation === b.rotation;
    case "rect": return b.kind === "rect" && a.width === b.width && a.height === b.height && a.radius === b.radius;
    case "text": return b.kind === "text" && a.text === b.text;
    case "polyline": {
      if (b.kind !== "polyline" || a.closed !== b.closed || a.points.length !== b.points.length) return false;
      const from = origin(a); const to = origin(b);
      return a.points.every((point, index) => point[0] - from[0] === b.points[index][0] - to[0]
        && point[1] - from[1] === b.points[index][1] - to[1]);
    }
  }
}

function colorValue(value: string) { const color = new Color(value); return { color: color.toNumber(), alpha: color.alpha }; }
function contour(item: Exclude<DrawItem, { kind: "text" }>, map: (p: DrawPoint) => DrawPoint, scale: number): { points: DrawPoint[]; closed: boolean } {
  if (item.kind === "polyline") return { points: item.points.map(map), closed: item.closed };
  if (item.kind === "rect") {
    const [x, y] = map([item.x, item.y]); const [w, h] = [item.width * scale, item.height * scale];
    const radius = Math.min(item.radius * scale, w / 2, h / 2);
    if (!radius) return { points: [[x, y], [x + w, y], [x + w, y + h], [x, y + h]], closed: true };
    const points: DrawPoint[] = [];
    for (const [cx, cy, start] of [[x + w - radius, y + radius, -Math.PI / 2], [x + w - radius, y + h - radius, 0], [x + radius, y + h - radius, Math.PI / 2], [x + radius, y + radius, Math.PI]]) {
      const steps = Math.max(4, Math.ceil(Math.PI / 2 / Math.acos(Math.max(-1, 1 - 0.1 / radius))));
      for (let i = 0; i <= steps; i++) { const t = start + i / steps * Math.PI / 2; points.push([cx + radius * Math.cos(t), cy + radius * Math.sin(t)]); }
    }
    return { points, closed: true };
  }
  const center = map(item.center);
  const radii = item.kind === "circle" ? [item.radius * scale, item.radius * scale] : item.radii.map((v) => v * scale);
  const rotation = item.kind === "ellipse" ? item.rotation : 0;
  const maximum = Math.max(...radii);
  const count = Math.max(16, Math.ceil(Math.PI * 2 / Math.acos(Math.max(-1, 1 - 0.1 / Math.max(maximum, 0.1)))));
  const points: DrawPoint[] = [];
  for (let i = 0; i < count; i++) {
    const t = i / count * Math.PI * 2; const x = radii[0] * Math.cos(t); const y = radii[1] * Math.sin(t);
    points.push([center[0] + x * Math.cos(rotation) - y * Math.sin(rotation), center[1] + x * Math.sin(rotation) + y * Math.cos(rotation)]);
  }
  return { points, closed: true };
}
function path(graphics: Graphics, points: DrawPoint[], closed: boolean) {
  if (!points.length) return;
  graphics.moveTo(...points[0]);
  for (const point of points.slice(1)) graphics.lineTo(...point);
  if (closed) graphics.closePath();
}
function paint(graphics: Graphics, item: Exclude<DrawItem, { kind: "text" }>, map: (p: DrawPoint) => DrawPoint, scale: number, shadow?: string) {
  graphics.clear();
  const { style } = item;
  const strokeScale = style.nonScalingStroke ? 1 : scale;
  const paintColor = (value: string) => {
    const original = colorValue(value);
    if (!shadow) return original;
    const halo = colorValue(shadow);
    return { color: halo.color, alpha: halo.alpha * original.alpha };
  };
  const drawShape = () => {
    if (item.kind === "polyline") path(graphics, item.points.map(map), item.closed);
    else if (item.kind === "circle") graphics.circle(...map(item.center), item.radius * scale);
    else if (item.kind === "ellipse") {
      const center = map(item.center);
      graphics.context.translate(...center).rotate(item.rotation).ellipse(0, 0, item.radii[0] * scale, item.radii[1] * scale).resetTransform();
    } else graphics.roundRect(...map([item.x, item.y]), item.width * scale, item.height * scale, item.radius * scale);
  };
  if (style.fill) { drawShape(); graphics.fill(paintColor(style.fill)); }
  if (style.stroke && style.strokeWidth > 0) {
    const stroke = { ...paintColor(style.stroke), width: style.strokeWidth * strokeScale, cap: style.lineCap, join: style.lineJoin };
    if (style.dash.length) {
      const outline = contour(item, map, scale);
      for (const segment of dashedSegments(outline.points, outline.closed, style.dash.map((length) => length * strokeScale))) { path(graphics, segment, false); graphics.stroke(stroke); }
    } else { drawShape(); graphics.stroke(stroke); }
  }
}

export async function createPixiBackend(canvas: HTMLCanvasElement): Promise<CanvasBackend> {
  // Request precisely WebGL2. Never silently fall back to software Canvas2D or WebGL1.
  const context = canvas.getContext("webgl2", { alpha: false, antialias: true, stencil: true, preserveDrawingBuffer: true, premultipliedAlpha: true });
  if (!context) throw new Error("WebGL2 is unavailable");
  const gl = context;
  const renderer = new WebGLRenderer();
  try {
    await renderer.init({ canvas, context: gl, width: 1, height: 1, resolution: 1, antialias: true, backgroundAlpha: 1, clearBeforeRender: true, manageImports: false, gcActive: false, eventMode: "none", eventFeatures: { move: false, globalMove: false, click: false, wheel: false } });
  } catch (error) { try { renderer.destroy({ removeView: false }); } catch { /* Partial initialization may not own all systems yet. */ } throw error; }
  if (renderer.context.webGLVersion !== 2) { renderer.destroy({ removeView: false }); throw new Error("The renderer did not acquire WebGL2"); }
  // A bare Pixi renderer otherwise starts a system ticker for events/GC. The host
  // owns pointer routing and presentation is strictly on demand; collect only after draws.
  // Pixi 8.20.1 documents and implements null detachment but omits it in the declaration.
  if (renderer.events) {
    const detachEvents = renderer.events.setTargetElement.bind(renderer.events) as (element: HTMLElement | null) => void;
    detachEvents(null);
  }
  renderer.scheduler.destroy();
  const debug = gl.getExtension("WEBGL_debug_renderer_info");
  const hardware = Object.freeze({ api: "WebGL2", vendor: String(gl.getParameter(debug?.UNMASKED_VENDOR_WEBGL ?? gl.VENDOR)), renderer: String(gl.getParameter(debug?.UNMASKED_RENDERER_WEBGL ?? gl.RENDERER)), version: String(gl.getParameter(gl.VERSION)) });
  const stage = new Container({ eventMode: "none", interactiveChildren: false });
  const entries = new Map<string, Entry>();
  let surfaceKey = "";
  let created = 0; let destroyed = 0; let updated = 0; let repainted = 0; let disposed = false; let restored = false;
  let cancelCompletion: ((error: Error) => void) | null = null;
  let failure: Error | null = null;
  const contextLost = () => cancelCompletion?.(new Error("WebGL2 context was lost during presentation"));
  // Registered after Pixi initialization: rebuild only on the next render, after
  // all of Pixi's own context-restoration listeners have completed.
  const contextRestored = () => { restored = true; failure = null; };
  canvas.addEventListener("webglcontextlost", contextLost);
  canvas.addEventListener("webglcontextrestored", contextRestored);
  function completeSubmission(stats: BackendStats): Promise<BackendStats> {
    if (gl.isContextLost()) throw new Error("WebGL2 context was lost during presentation");
    const sync = gl.fenceSync(gl.SYNC_GPU_COMMANDS_COMPLETE, 0);
    if (!sync) throw new Error("WebGL2 could not create a presentation fence");
    return new Promise((resolve, reject) => {
      let timer: ReturnType<typeof setTimeout> | undefined;
      let finished = false;
      const deadline = performance.now() + 5_000;
      const finish = (error?: Error) => {
        if (finished) return;
        finished = true;
        if (timer !== undefined) clearTimeout(timer);
        if (cancelCompletion === cancel) cancelCompletion = null;
        try { gl.deleteSync(sync); } catch (cleanupError) { error ??= cleanupError instanceof Error ? cleanupError : new Error(String(cleanupError)); }
        // A deleted fence does not cancel submitted GPU work. A failed wait
        // retires this context until restoration, preventing an unbounded queue.
        if (error) failure = error;
        if (error) reject(error); else resolve(stats);
      };
      const cancel = (error: Error) => finish(error);
      cancelCompletion = cancel;
      const poll = () => {
        if (finished) return;
        timer = undefined;
        try {
          if (disposed || gl.isContextLost()) throw new Error("WebGL2 context is unavailable for presentation");
          const status = gl.clientWaitSync(sync, 0, 0);
          if (status === gl.TIMEOUT_EXPIRED) {
            if (performance.now() >= deadline) throw new Error("WebGL2 presentation did not complete within 5 seconds");
            // Match browsers' nested-timer floor without an extra half-frame
            // delay before releasing the newest queued interaction frame.
            timer = setTimeout(poll, 4);
            return;
          }
          if (status !== gl.ALREADY_SIGNALED && status !== gl.CONDITION_SATISFIED) throw new Error(`WebGL2 presentation fence failed with status ${status}`);
          // Error flags remain authoritative. Reading them only after the fence
          // completes avoids synchronously waiting for queued GPU work here.
          const error = gl.getError();
          if (error !== gl.NO_ERROR) throw new Error(`WebGL2 presentation failed with error ${error}`);
          if (gl.isContextLost()) throw new Error("WebGL2 context was lost during presentation");
          finish();
        } catch (error) { finish(error instanceof Error ? error : new Error(String(error))); }
      };
      try {
        gl.flush(); // Submit the fence too; never busy-wait or use a nonzero wait timeout.
        timer = setTimeout(poll, 0);
      } catch (error) { finish(error instanceof Error ? error : new Error(String(error))); }
    });
  }
  function remove(entry: Entry) {
    entry.blur?.destroy();
    entry.container.destroy({ children: true, texture: true, textureSource: true });
    destroyed++;
  }
  return {
    hardware,
    render(frame, surface) {
      if (disposed || gl.isContextLost()) throw new Error("WebGL2 context is unavailable for presentation");
      if (failure) throw failure;
      if (cancelCompletion) throw new Error("WebGL2 presentation is already in flight");
      try {
        if (restored) {
          invalidateRestoredUniformUploads(renderer);
          // Textures rasterized during a failed first draw can also be incomplete.
          // Retire retained presentation resources once; native frame authority and
          // ordinary translation/resource reuse remain unchanged.
          for (const entry of entries.values()) remove(entry);
          entries.clear(); surfaceKey = ""; restored = false;
        }
        // Supersample low-DPR displays so thin CAD strokes and small dimension
        // text retain subpixel coverage comparable to the SVG baseline.
        const rasterResolution = Math.max(2, surface.pixelRatio);
        const nextSurface = `${surface.width}:${surface.height}:${surface.pixelRatio}`;
        if (nextSurface !== surfaceKey) { renderer.resize(surface.width, surface.height, rasterResolution); surfaceKey = nextSurface; }
        renderer.background.color = frame.background;
        const fit = fitDrawing(frame.viewBox, surface.width, surface.height);
        const map = (p: DrawPoint): DrawPoint => [fit.x + p[0] * fit.scale, fit.y + p[1] * fit.scale];
        const present = new Set(frame.items.map((item) => item.id));
        for (const [id, entry] of entries) if (!present.has(id)) { remove(entry); entries.delete(id); }
        frame.items.forEach((item, index) => {
          let entry = entries.get(item.id);
          if (entry && entry.kind !== item.kind) { remove(entry); entries.delete(item.id); entry = undefined; }
          if (!entry) {
            const container = new Container({ eventMode: "none", interactiveChildren: false });
            const content = item.kind === "text" ? new Text({ text: "", resolution: rasterResolution }) : new Graphics();
            container.addChild(content); stage.addChild(container);
            entry = { container, content, shadow: null, blur: null, item: null, kind: item.kind, scale: 0, resolution: 0, position: null };
            entries.set(item.id, entry); created++;
          }
          // Paint order is explicitly native-authored; semantic keys do not reorder anything.
          // Pixi's setChildIndex searches twice even when the child is already in place.
          if (stage.children[index] !== entry.container) stage.setChildIndex(entry.container, index);
          const { style } = item;
          const anchor = origin(item); const position = map(anchor);
          const previous = entry.item;
          const paintChanged = !previous || entry.scale !== fit.scale || !sameStyle(previous.style, style)
            || !sameLocalGeometry(previous, item) || item.kind === "text" && entry.resolution !== rasterResolution;
          const positionChanged = !entry.position || entry.position[0] !== position[0] || entry.position[1] !== position[1];
          const rotationChanged = item.kind === "text" && (!previous || previous.kind !== "text" || previous.rotation !== item.rotation);
          entry.item = item;
          if (!paintChanged && !positionChanged && !rotationChanged) return;
          // Keep CSS stroke widths, glyph sizes and shadow offsets in their original units;
          // only translation is delegated to the container transform.
          if (positionChanged) entry.container.position.set(...position);
          entry.container.alpha = style.opacity;
          if (item.kind === "text" && entry.content instanceof Text) {
            const text = entry.content;
            if (paintChanged) {
              const textStyle = new TextStyle({ fontFamily: style.fontFamily.split(",").map((font) => font.trim().replace(/^['"]|['"]$/g, "")), fontSize: style.fontSize * fit.scale,
                fontWeight: String(style.fontWeight) as "400", fill: style.fill ? colorValue(style.fill) : { color: 0, alpha: 0 },
                stroke: style.stroke ? { ...colorValue(style.stroke), width: style.strokeWidth * (style.nonScalingStroke ? 1 : fit.scale), join: style.lineJoin } : undefined,
                textBaseline: "alphabetic", letterSpacing: style.letterSpacing * fit.scale, padding: Math.max(style.strokeWidth, style.shadow?.blur ?? 0) * 2,
                dropShadow: style.shadow ? { ...colorValue(style.shadow.color), blur: style.shadow.blur, distance: Math.hypot(...style.shadow.offset), angle: Math.atan2(style.shadow.offset[1], style.shadow.offset[0]) } : false });
              text.text = item.text; text.style = textStyle; text.resolution = rasterResolution;
              text.anchor.set(0, 0);
              const metrics = CanvasTextMetrics.measureText(item.text, textStyle);
              const origin = textRasterOrigin(metrics, style.textAnchor, style.textBaseline, style.stroke ? style.strokeWidth * (style.nonScalingStroke ? 1 : fit.scale) : 0);
              text.pivot.set(origin.x, origin.y);
            }
            text.rotation = item.rotation;
          } else if (paintChanged && item.kind !== "text" && entry.content instanceof Graphics) {
            const local = (point: DrawPoint): DrawPoint => [(point[0] - anchor[0]) * fit.scale, (point[1] - anchor[1]) * fit.scale];
            paint(entry.content, item, local, fit.scale);
            if (style.shadow) {
              if (!entry.shadow) { entry.shadow = new Graphics(); entry.container.addChildAt(entry.shadow, 0); }
              paint(entry.shadow, item, local, fit.scale, style.shadow.color);
              entry.shadow.position.set(...style.shadow.offset);
              if (!entry.blur) entry.blur = new BlurFilter({ strength: style.shadow.blur, quality: 4 });
              entry.blur.strength = style.shadow.blur;
              entry.shadow.filters = style.shadow.blur ? [entry.blur] : [];
            } else if (entry.shadow) { entry.shadow.destroy(); entry.shadow = null; entry.blur?.destroy(); entry.blur = null; }
          }
          entry.scale = fit.scale; entry.resolution = rasterResolution; entry.position = position; updated++;
          if (paintChanged) repainted++;
        });
        renderer.render({ container: stage });
        renderer.gc.run();
        return completeSubmission({ resources: entries.size, created, destroyed, updated, repainted, rasterResolution });
      } catch (error) {
        failure = error instanceof Error ? error : new Error(String(error));
        throw failure;
      }
    },
    destroy() {
      if (disposed) return;
      disposed = true;
      cancelCompletion?.(new Error("WebGL2 renderer was disposed during presentation"));
      canvas.removeEventListener("webglcontextlost", contextLost);
      canvas.removeEventListener("webglcontextrestored", contextRestored);
      for (const entry of entries.values()) remove(entry);
      entries.clear(); stage.destroy(); renderer.destroy({ removeView: false });
    },
  };
}
