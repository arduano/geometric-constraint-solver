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
export function utf8ByteSpanToUtf16Range(
  source: string,
  from: number,
  to: number,
): SourceNavigationConversion {
  if (!Number.isSafeInteger(from) || !Number.isSafeInteger(to)) {
    return failure("source offsets must be safe integers");
  }
  if (from < 0 || to < from) {
    return failure("source span must satisfy 0 <= from <= to");
  }
  if (to > MAX_SOURCE_NAVIGATION_UTF8_BYTES) {
    return failure(
      `source span exceeds the ${MAX_SOURCE_NAVIGATION_UTF8_BYTES}-byte navigation limit`,
    );
  }

  // Every valid Unicode scalar consumes at least as many UTF-8 bytes as
  // UTF-16 code units, so this rejects oversized input before scanning it.
  if (source.length > MAX_SOURCE_NAVIGATION_UTF8_BYTES) {
    return failure(
      `source text exceeds the ${MAX_SOURCE_NAVIGATION_UTF8_BYTES}-byte navigation limit`,
    );
  }

  let byteOffset = 0;
  let codeUnitOffset = 0;
  let utf16From = from === 0 ? 0 : undefined;
  let utf16To = to === 0 ? 0 : undefined;

  while (codeUnitOffset < source.length) {
    const first = source.charCodeAt(codeUnitOffset);
    let codePoint = first;
    let codeUnits = 1;

    if (first >= 0xd800 && first <= 0xdbff) {
      const second = source.charCodeAt(codeUnitOffset + 1);
      if (!(second >= 0xdc00 && second <= 0xdfff)) {
        return failure("source text contains an unpaired UTF-16 surrogate");
      }
      codePoint = 0x10000 + ((first - 0xd800) << 10) + (second - 0xdc00);
      codeUnits = 2;
    } else if (first >= 0xdc00 && first <= 0xdfff) {
      return failure("source text contains an unpaired UTF-16 surrogate");
    }

    const codePointBytes = codePoint <= 0x7f
      ? 1
      : codePoint <= 0x7ff
        ? 2
        : codePoint <= 0xffff
          ? 3
          : 4;
    const nextByteOffset = byteOffset + codePointBytes;
    if (nextByteOffset > MAX_SOURCE_NAVIGATION_UTF8_BYTES) {
      return failure(
        `source text exceeds the ${MAX_SOURCE_NAVIGATION_UTF8_BYTES}-byte navigation limit`,
      );
    }

    if (
      (from > byteOffset && from < nextByteOffset)
      || (to > byteOffset && to < nextByteOffset)
    ) {
      return failure("source offset splits a UTF-8 code point");
    }

    byteOffset = nextByteOffset;
    codeUnitOffset += codeUnits;
    if (byteOffset === from) utf16From = codeUnitOffset;
    if (byteOffset === to) utf16To = codeUnitOffset;
  }

  if (utf16From === undefined || utf16To === undefined) {
    return failure(`source span is outside the ${byteOffset}-byte source text`);
  }
  return { ok: true, range: { from: utf16From, to: utf16To } };
}

function failure(reason: string): SourceNavigationConversion {
  return { ok: false, reason };
}
