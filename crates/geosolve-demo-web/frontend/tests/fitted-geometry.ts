// SPDX-License-Identifier: GPL-3.0-or-later
/** Absolute logical-pixel tolerance for browser presentation comparisons only. */
export const FITTED_GEOMETRY_TOLERANCE_PIXELS = 1e-9;

type Json = number | string | boolean | Json[] | { [key: string]: Json };
const record = (value: unknown): value is Record<string, unknown> => value !== null && typeof value === "object" && !Array.isArray(value);
const finite = (value: unknown): value is number => typeof value === "number" && Number.isFinite(value);
const nonnegative = (value: unknown) => finite(value) && value >= 0;
const point = (value: unknown) => Array.isArray(value) && value.length === 2 && value.every(finite);
const keys = (value: Record<string, unknown>, expected: string[]) => Object.keys(value).length === expected.length && expected.every((key) => Object.hasOwn(value, key));

function primitive(value: unknown): boolean {
  if (!record(value)) return false;
  switch (value.kind) {
    case "polyline": return keys(value, ["kind", "points", "closed"])
      && Array.isArray(value.points) && value.points.length > 0 && value.points.every(point) && typeof value.closed === "boolean";
    case "circle": return keys(value, ["kind", "center", "radius"]) && point(value.center) && nonnegative(value.radius);
    case "ellipse": return keys(value, ["kind", "center", "radii", "rotation"])
      && point(value.center) && point(value.radii) && (value.radii as number[]).every(nonnegative) && finite(value.rotation);
    case "rect": return keys(value, ["kind", "x", "y", "width", "height", "radius"])
      && finite(value.x) && finite(value.y) && nonnegative(value.width) && nonnegative(value.height) && nonnegative(value.radius);
    case "text": return keys(value, ["kind", "position", "text", "rotation"])
      && point(value.position) && typeof value.text === "string" && finite(value.rotation);
    default: return false;
  }
}

function parseGeometry(encoded: string, label: string): Json {
  let value: unknown;
  try { value = JSON.parse(encoded); } catch { throw Error(`${label} fitted geometry is not JSON`); }
  if (!record(value) || !keys(value, ["viewBox", "tokens"])
    || !Array.isArray(value.viewBox) || value.viewBox.length !== 4 || !value.viewBox.every(finite)
    || value.viewBox[2] <= 0 || value.viewBox[3] <= 0
    || !Array.isArray(value.tokens) || value.tokens.length === 0 || !value.tokens.every(primitive)) {
    throw Error(`${label} fitted geometry has malformed or nonfinite primitives`);
  }
  return value as Json;
}

export interface GeometryComparison {
  equal: boolean;
  difference: string | null;
  maximumCoordinateDelta: number;
}

/**
 * Compare history/edit presentation without rounding the captured JSON or release hashes.
 * Arrays retain exact order/counts, keys and nonnumeric values remain exact, and rotations
 * remain exact because their unit is radians rather than logical pixels. Invalid input throws
 * even for an inequality assertion, so corrupt geometry cannot qualify an edit as meaningful.
 */
export function compareFittedGeometry(actual: string, expected: string): GeometryComparison {
  const left = parseGeometry(actual, "Actual");
  const right = parseGeometry(expected, "Expected");
  let difference: string | null = null;
  let maximumCoordinateDelta = 0;
  const mismatch = (path: string, reason: string) => { difference ??= `${path}: ${reason}`; };
  function compare(a: Json, b: Json, path: string): void {
    if (typeof a === "number" && typeof b === "number") {
      if (path.endsWith(".rotation")) {
        if (a !== b) mismatch(path, "rotation changed");
      } else {
        const delta = Math.abs(a - b);
        maximumCoordinateDelta = Math.max(maximumCoordinateDelta, delta);
        if (delta > FITTED_GEOMETRY_TOLERANCE_PIXELS) mismatch(path, `coordinate differs by ${delta} logical px`);
      }
    } else if (Array.isArray(a) && Array.isArray(b)) {
      if (a.length !== b.length) mismatch(path, `array length ${a.length} differs from ${b.length}`);
      for (let index = 0; index < Math.min(a.length, b.length); index++) compare(a[index], b[index], `${path}[${index}]`);
    } else if (record(a) && record(b)) {
      if (!keys(a, Object.keys(b))) mismatch(path, "object keys changed");
      for (const key of Object.keys(a)) if (Object.hasOwn(b, key)) compare(a[key], b[key], `${path}.${key}`);
    } else if (a !== b) mismatch(path, `value/type changed from ${JSON.stringify(b)} to ${JSON.stringify(a)}`);
  }
  compare(left, right, "geometry");
  return { equal: difference === null, difference, maximumCoordinateDelta };
}
