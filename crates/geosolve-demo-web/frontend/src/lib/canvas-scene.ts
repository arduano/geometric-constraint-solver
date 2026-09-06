// SPDX-License-Identifier: GPL-3.0-or-later
/** Presentation-only primitives composed by Rust. No equations or picking authority live here. */
export type DrawPoint = [number, number];
export interface DrawStyle {
  fill: string | null;
  stroke: string | null;
  strokeWidth: number;
  dash: number[];
  opacity: number;
  lineCap: "round" | "butt" | "square";
  lineJoin: "round" | "miter" | "bevel";
  nonScalingStroke: boolean;
  textBaseline: "alphabetic" | "central";
  letterSpacing: number;
  fontFamily: string;
  fontSize: number;
  fontWeight: number;
  textAnchor: "start" | "middle" | "end";
  shadow: { color: string; blur: number; offset: DrawPoint } | null;
}
interface DrawIdentity {
  id: string;
  layer: string;
  semanticKey: string | null;
  className: string;
  accessibleLabel: string | null;
  interactive: boolean;
  metadata: Record<string, string>;
  style: DrawStyle;
}
export type DrawItem = DrawIdentity & (
  | { kind: "polyline"; points: DrawPoint[]; closed: boolean }
  | { kind: "circle"; center: DrawPoint; radius: number }
  | { kind: "ellipse"; center: DrawPoint; radii: DrawPoint; rotation: number }
  | { kind: "rect"; x: number; y: number; width: number; height: number; radius: number }
  | { kind: "text"; position: DrawPoint; text: string; rotation: number }
);
export interface DrawFrame {
  format: "geosolve-draw-frame-v1";
  viewBox: [number, number, number, number];
  background: string;
  provenance: Record<string, string>;
  items: DrawItem[];
}

const record = (value: unknown): value is Record<string, unknown> => !!value && typeof value === "object" && !Array.isArray(value);
const finite = (value: unknown): value is number => typeof value === "number" && Number.isFinite(value);
const nonnegative = (value: unknown): value is number => finite(value) && value >= 0;
const point = (value: unknown): value is DrawPoint => Array.isArray(value) && value.length === 2 && value.every(finite);
const strings = (value: unknown): value is Record<string, string> => record(value) && Object.values(value).every((entry) => typeof entry === "string");
const nullableString = (value: unknown) => value === null || typeof value === "string";
// Native palette is serialized as concrete CSS colours, never CSS variables or markup.
const color = (value: unknown): value is string => typeof value === "string" && /^(?:#[0-9a-f]{3,4}|#[0-9a-f]{6}|#[0-9a-f]{8}|(?:rgb|rgba|hsl|hsla)\([\d\s.,%/+\-]+\)|[a-z]+)$/i.test(value);
function style(value: unknown): value is DrawStyle {
  if (!record(value)) return false;
  return (value.fill === null || color(value.fill)) && (value.stroke === null || color(value.stroke))
    && nonnegative(value.strokeWidth) && Array.isArray(value.dash) && value.dash.every(nonnegative)
    && (value.dash.length === 0 || value.dash.some((length) => length > 0))
    && finite(value.opacity) && value.opacity >= 0 && value.opacity <= 1
    && ["round", "butt", "square"].includes(String(value.lineCap))
    && ["round", "miter", "bevel"].includes(String(value.lineJoin))
    && typeof value.nonScalingStroke === "boolean" && ["alphabetic", "central"].includes(String(value.textBaseline)) && finite(value.letterSpacing)
    && typeof value.fontFamily === "string" && value.fontFamily.length > 0
    && finite(value.fontSize) && value.fontSize > 0
    && finite(value.fontWeight) && value.fontWeight >= 1 && value.fontWeight <= 1000
    && ["start", "middle", "end"].includes(String(value.textAnchor))
    && (value.shadow === null || (record(value.shadow) && color(value.shadow.color)
      && nonnegative(value.shadow.blur) && point(value.shadow.offset)));
}
function item(value: unknown): value is DrawItem {
  if (!record(value) || typeof value.id !== "string" || value.id.length === 0 || typeof value.layer !== "string"
    || !nullableString(value.semanticKey) || typeof value.className !== "string" || !nullableString(value.accessibleLabel) || typeof value.interactive !== "boolean"
    || !strings(value.metadata) || !style(value.style)) return false;
  switch (value.kind) {
    case "polyline": return Array.isArray(value.points) && value.points.every(point) && typeof value.closed === "boolean";
    case "circle": return point(value.center) && nonnegative(value.radius);
    case "ellipse": return point(value.center) && point(value.radii) && value.radii.every(nonnegative) && finite(value.rotation);
    case "rect": return finite(value.x) && finite(value.y) && nonnegative(value.width) && nonnegative(value.height) && nonnegative(value.radius);
    case "text": return point(value.position) && typeof value.text === "string" && finite(value.rotation);
    default: return false;
  }
}
export function assertDrawFrame(value: unknown): asserts value is DrawFrame {
  if (!record(value) || value.format !== "geosolve-draw-frame-v1" || !Array.isArray(value.viewBox)
    || value.viewBox.length !== 4 || !value.viewBox.every(finite) || value.viewBox[2] <= 0 || value.viewBox[3] <= 0
    || !color(value.background) || !strings(value.provenance)
    || !Array.isArray(value.items) || !value.items.every(item)
    || new Set(value.items.map((entry) => entry.id)).size !== value.items.length) {
    throw new Error("Invalid geosolve drawing frame: expected finite typed primitives and unique presentation identities");
  }
}

/** Clone once at the input boundary: observers and caller mutation cannot alter accepted renderer input. */
export function immutableDrawFrame(frame: DrawFrame): DrawFrame {
  assertDrawFrame(frame);
  const copy = structuredClone(frame);
  function freeze(value: unknown): void {
    if (value && typeof value === "object") { Object.values(value).forEach(freeze); Object.freeze(value); }
  }
  freeze(copy);
  return copy;
}
