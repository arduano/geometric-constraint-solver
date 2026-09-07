// SPDX-License-Identifier: GPL-3.0-or-later

/**
 * Keep presentation-side source navigation within the managed-source bound.
 * This mirrors `geosolve_sketch_code::MANAGED_SOURCE_LIMIT`; Rust remains the
 * authority that admits source, while this limit bounds browser-side scans.
 */
export const MAX_SOURCE_NAVIGATION_UTF8_BYTES = 4 * 1024 * 1024;

export interface Utf16SourceRange {
  from: number;
  to: number;
}

export type SourceNavigationConversion =
  | { ok: true; range: Utf16SourceRange }
  | { ok: false; reason: string };

/**
 * Converts an authenticated Rust UTF-8 byte span into JavaScript/CodeMirror
 * UTF-16 code-unit positions. Invalid coordinates and non-Unicode JavaScript
 * strings fail closed; an offset is never rounded onto a nearby character.
 */
export function utf8ByteSpanToUtf16Range(source: string, from: number, to: number): SourceNavigationConversion {
  const result = utf8ByteSpansToUtf16Ranges(source, [{ from, to }]);
  return result.ok ? { ok: true, range: result.ranges[0]! } : result;
}

/** One bounded source scan for a group or multi-selection, rather than one per owner. */
export function utf8ByteSpansToUtf16Ranges(source: string, spans: readonly Utf16SourceRange[]): { ok: true; ranges: Utf16SourceRange[] } | { ok: false; reason: string } {
  const offsets = new Map<number, number | undefined>();
  for (const { from, to } of spans) {
    if (!Number.isSafeInteger(from) || !Number.isSafeInteger(to)) return { ok: false, reason: "source offsets must be safe integers" };
    if (from < 0 || to < from) return { ok: false, reason: "source span must satisfy 0 <= from <= to" };
    if (to > MAX_SOURCE_NAVIGATION_UTF8_BYTES) return { ok: false, reason: `source span exceeds the ${MAX_SOURCE_NAVIGATION_UTF8_BYTES}-byte navigation limit` };
    offsets.set(from, undefined);
    offsets.set(to, undefined);
  }
  if (source.length > MAX_SOURCE_NAVIGATION_UTF8_BYTES) return { ok: false, reason: `source text exceeds the ${MAX_SOURCE_NAVIGATION_UTF8_BYTES}-byte navigation limit` };
  let bytes = 0;
  let units = 0;
  if (offsets.has(0)) offsets.set(0, 0);
  while (units < source.length) {
    const first = source.charCodeAt(units);
    let point = first;
    let width = 1;
    if (first >= 0xd800 && first <= 0xdbff) {
      const second = source.charCodeAt(units + 1);
      if (!(second >= 0xdc00 && second <= 0xdfff)) return { ok: false, reason: "source text contains an unpaired UTF-16 surrogate" };
      point = 0x10000 + ((first - 0xd800) << 10) + second - 0xdc00;
      width = 2;
    } else if (first >= 0xdc00 && first <= 0xdfff) return { ok: false, reason: "source text contains an unpaired UTF-16 surrogate" };
    const byteWidth = point <= 0x7f ? 1 : point <= 0x7ff ? 2 : point <= 0xffff ? 3 : 4;
    for (let inside = 1; inside < byteWidth; inside += 1) {
      if (offsets.has(bytes + inside)) return { ok: false, reason: "source offset splits a UTF-8 code point" };
    }
    bytes += byteWidth;
    units += width;
    if (bytes > MAX_SOURCE_NAVIGATION_UTF8_BYTES) return { ok: false, reason: `source text exceeds the ${MAX_SOURCE_NAVIGATION_UTF8_BYTES}-byte navigation limit` };
    if (offsets.has(bytes)) offsets.set(bytes, units);
  }
  if ([...offsets.values()].some((offset) => offset === undefined)) return { ok: false, reason: `source span is outside the ${bytes}-byte source text` };
  return { ok: true, ranges: spans.map(({ from, to }) => ({ from: offsets.get(from)!, to: offsets.get(to)! })) };
}

/** Exact editor coordinates to native byte coordinates; never rounds a surrogate split. */
export function utf16RangeToUtf8ByteSpan(source: string, from: number, to: number): SourceNavigationConversion {
  if (!Number.isSafeInteger(from) || !Number.isSafeInteger(to)) return failure("source offsets must be safe integers");
  if (from < 0 || to < from || to > source.length) return failure("source span is outside the UTF-16 source text");
  const validSource = utf8ByteSpanToUtf16Range(source, 0, 0);
  if (!validSource.ok) return validSource;
  for (const offset of [from, to]) {
    const unit = source.charCodeAt(offset);
    if (unit >= 0xdc00 && unit <= 0xdfff) return failure("source offset splits a UTF-16 surrogate pair");
  }
  const encoder = new TextEncoder();
  return { ok: true, range: { from: encoder.encode(source.slice(0, from)).length, to: encoder.encode(source.slice(0, to)).length } };
}

function failure(reason: string): SourceNavigationConversion {
  return { ok: false, reason };
}
