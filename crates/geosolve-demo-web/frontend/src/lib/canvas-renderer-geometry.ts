// SPDX-License-Identifier: GPL-3.0-or-later
import type { DrawFrame, DrawPoint } from "./canvas-scene";

export function fitDrawing(viewBox: DrawFrame["viewBox"], width: number, height: number) {
  const scale = Math.min(width / viewBox[2], height / viewBox[3]);
  return { scale, x: (width - viewBox[2] * scale) / 2 - viewBox[0] * scale, y: (height - viewBox[3] * scale) / 2 - viewBox[1] * scale };
}

/** Raster stroke segmentation, not semantic geometry. Phase continues over vertices and the closing edge. */
export function dashedSegments(points: DrawPoint[], closed: boolean, dash: number[]): DrawPoint[][] {
  if (points.length < 2) return [];
  if (!dash.length) return [closed ? [...points, points[0]] : points];
  const pattern = dash.length % 2 ? [...dash, ...dash] : dash;
  if (!pattern.some((length) => length > 0)) throw new Error("Dash pattern has zero length");
  const result: DrawPoint[][] = [];
  let patternIndex = 0;
  let remaining = pattern[0];
  let active: DrawPoint[] | null = null;
  let pieces = 0;
  const vertices = closed ? [...points, points[0]] : points;
  for (let edge = 1; edge < vertices.length; edge++) {
    const a = vertices[edge - 1]; const b = vertices[edge];
    const length = Math.hypot(b[0] - a[0], b[1] - a[1]);
    if (!Number.isFinite(length)) throw new Error("Non-finite raster stroke extent");
    if (!length) continue;
    let offset = 0;
    while (offset < length) {
      while (remaining <= 0) {
        patternIndex = (patternIndex + 1) % pattern.length;
        remaining = pattern[patternIndex];
        if (patternIndex % 2) active = null;
      }
      const step = Math.min(remaining, length - offset);
      if (++pieces > 1_000_000 || offset + step === offset) throw new Error("Raster dash detail exceeds representable presentation extent");
      const from: DrawPoint = [a[0] + (b[0] - a[0]) * offset / length, a[1] + (b[1] - a[1]) * offset / length];
      offset += step;
      const to: DrawPoint = [a[0] + (b[0] - a[0]) * offset / length, a[1] + (b[1] - a[1]) * offset / length];
      if (patternIndex % 2 === 0) {
        if (!active) { active = [from]; result.push(active); }
        active.push(to);
      }
      remaining -= step;
    }
  }
  return result;
}

/** Align the glyph advance/baseline, excluding texture padding and drop-shadow extents. */
export function textRasterOrigin(metrics: { maxLineWidth: number; lineHeight: number; fontProperties: { ascent: number; descent: number; fontSize: number } }, anchor: "start" | "middle" | "end", baseline: "alphabetic" | "central", strokeWidth: number) {
  const font = metrics.fontProperties;
  const alphabetic = strokeWidth / 2 + Math.max(0, (metrics.lineHeight - font.fontSize) / 2) + font.ascent;
  return { x: strokeWidth / 2 + metrics.maxLineWidth * (anchor === "middle" ? 0.5 : anchor === "end" ? 1 : 0),
    y: alphabetic - (baseline === "central" ? (font.ascent - font.descent) / 2 : 0) };
}
