// SPDX-License-Identifier: GPL-3.0-or-later
export function counter(value: number): void {
  if (!Number.isSafeInteger(value) || value < 0) throw Error("Expected a nonnegative safe integer counter");
}
export function unicode(value: string): void {
  if (typeof value !== "string") throw Error("Expected text");
  for (let index = 0; index < value.length; index++) {
    const code = value.charCodeAt(index);
    if (code >= 0xd800 && code <= 0xdbff) {
      const next = value.charCodeAt(++index);
      if (!(next >= 0xdc00 && next <= 0xdfff)) throw Error("Text contains an unpaired UTF-16 surrogate");
    } else if (code >= 0xdc00 && code <= 0xdfff) throw Error("Text contains an unpaired UTF-16 surrogate");
  }
}
export function encode(value: unknown): string {
  const ancestors = new Set<object>();
  const inspect = (item: unknown): void => {
    if (typeof item === "string") unicode(item);
    else if (typeof item === "number" && !Number.isFinite(item)) throw Error("Nonfinite JSON number");
    else if (typeof item === "function" || typeof item === "symbol" || typeof item === "bigint") throw Error("Expected ordinary JSON data");
    else if (item !== null && typeof item === "object") {
      if (ancestors.has(item)) throw Error("Cyclic JSON data");
      if (!Array.isArray(item) && Object.getPrototypeOf(item) !== Object.prototype && Object.getPrototypeOf(item) !== null) throw Error("Expected ordinary JSON data");
      ancestors.add(item);
      for (const [key, child] of Object.entries(item)) { unicode(key); inspect(child); }
      ancestors.delete(item);
    }
  };
  inspect(value);
  return JSON.stringify(value);
}
export function decode<T>(text: string): T {
  const freeze = (value: unknown): unknown => {
    if (value !== null && typeof value === "object") { Object.values(value).forEach(freeze); Object.freeze(value); }
    return value;
  };
  return freeze(JSON.parse(text)) as T;
}
