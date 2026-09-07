// SPDX-License-Identifier: GPL-3.0-or-later
import { describe, expect, it } from "vitest";
import {
  MAX_SOURCE_NAVIGATION_UTF8_BYTES,
  utf8ByteSpanToUtf16Range,
  utf8ByteSpansToUtf16Ranges,
  utf16RangeToUtf8ByteSpan,
} from "./source-navigation";

const utf8Bytes = (value: string) => new TextEncoder().encode(value).byteLength;

describe("UTF-8 source-span navigation", () => {
  it("maps multibyte text before and inside a target to exact UTF-16 positions", () => {
    const prefix = "// naïve 東京 🧭\nconst radius = ";
    const target = 'mm("半径😀")';
    const source = `${prefix}${target};\n`;

    expect(utf8ByteSpanToUtf16Range(
      source,
      utf8Bytes(prefix),
      utf8Bytes(prefix + target),
    )).toEqual({
      ok: true,
      range: {
        from: prefix.length,
        to: prefix.length + target.length,
      },
    });
  });

  it("accepts exact boundaries around BMP and astral code points", () => {
    const source = "aé😀z";

    expect(utf8ByteSpanToUtf16Range(source, 1, 3)).toEqual({
      ok: true,
      range: { from: 1, to: 2 },
    });
    expect(utf8ByteSpanToUtf16Range(source, 3, 7)).toEqual({
      ok: true,
      range: { from: 2, to: 4 },
    });
    expect(utf8ByteSpanToUtf16Range(source, 0, 8)).toEqual({
      ok: true,
      range: { from: 0, to: source.length },
    });
  });

  it.each([
    ["inside a two-byte code point", 2, 3],
    ["at the first byte inside an astral code point", 4, 7],
    ["at the second byte inside an astral code point", 5, 7],
    ["at the third byte inside an astral code point", 6, 7],
    ["with an end inside an astral code point", 3, 6],
  ])("rejects an offset %s", (_label, from, to) => {
    expect(utf8ByteSpanToUtf16Range("aé😀z", from, to)).toEqual({
      ok: false,
      reason: "source offset splits a UTF-8 code point",
    });
  });

  it.each([
    ["negative", -1, 0],
    ["reversed", 2, 1],
  ])("rejects a %s span", (_label, from, to) => {
    expect(utf8ByteSpanToUtf16Range("abc", from, to)).toMatchObject({ ok: false });
  });

  it.each([
    [Number.NaN, 0],
    [0.5, 1],
    [0, Number.MAX_SAFE_INTEGER + 1],
  ])("rejects non-integral or unsafe offsets (%s, %s)", (from, to) => {
    expect(utf8ByteSpanToUtf16Range("abc", from, to)).toEqual({
      ok: false,
      reason: "source offsets must be safe integers",
    });
  });

  it("rejects out-of-range byte offsets instead of clamping them", () => {
    expect(utf8ByteSpanToUtf16Range("aé😀z", 8, 9)).toEqual({
      ok: false,
      reason: "source span is outside the 8-byte source text",
    });
  });

  it("bounds its scan and rejects strings that cannot represent Rust UTF-8", () => {
    expect(utf8ByteSpanToUtf16Range(
      "a".repeat(MAX_SOURCE_NAVIGATION_UTF8_BYTES + 1),
      0,
      0,
    )).toMatchObject({ ok: false, reason: expect.stringContaining("navigation limit") });
    expect(utf8ByteSpanToUtf16Range("before\ud800after", 0, 0)).toEqual({
      ok: false,
      reason: "source text contains an unpaired UTF-16 surrogate",
    });
  });
});


describe("UTF-16 editor selection navigation", () => {
  it("round trips exact Unicode boundaries including empty cursor ranges", () => {
    const source = "aé😀z";
    for (const from of [0, 1, 2, 4, 5]) {
      for (const to of [0, 1, 2, 4, 5].filter((end) => end >= from)) {
        const converted = utf16RangeToUtf8ByteSpan(source, from, to);
        expect(converted).toEqual({ ok: true, range: { from: utf8Bytes(source.slice(0, from)), to: utf8Bytes(source.slice(0, to)) } });
        if (converted.ok) expect(utf8ByteSpanToUtf16Range(source, converted.range.from, converted.range.to)).toEqual({ ok: true, range: { from, to } });
      }
    }
  });
  it.each([[3, 4], [2, 3], [-1, 0], [2, 1], [0, 6], [0.5, 1], [0, Number.NaN]])("rejects invalid UTF-16 boundaries %s, %s", (from, to) => {
    expect(utf16RangeToUtf8ByteSpan("aé😀z", from, to)).toMatchObject({ ok: false });
  });
  it("validates the whole source and limits its encoded size", () => {
    expect(utf16RangeToUtf8ByteSpan("a\ud800", 0, 1)).toMatchObject({ ok: false });
    expect(utf16RangeToUtf8ByteSpan("é".repeat(MAX_SOURCE_NAVIGATION_UTF8_BYTES / 2 + 1), 0, 1)).toMatchObject({ ok: false });
  });
});


it("converts a large multi-owner selection in source order without rounding or coalescing", () => {
  const statement = "// é🧭\nconst edge = 1;\n";
  const source = statement.repeat(2000);
  const bytes = utf8Bytes(statement);
  const spans = Array.from({ length: 2000 }, (_, index) => ({ from: index * bytes, to: (index + 1) * bytes }));
  expect(utf8ByteSpansToUtf16Ranges(source, spans)).toEqual({ ok: true, ranges: spans.map((_, index) => ({ from: index * statement.length, to: (index + 1) * statement.length })) });
  expect(utf8ByteSpansToUtf16Ranges(source, [{ from: 0, to: 3 }, { from: bytes + 4, to: bytes + 5 }])).toMatchObject({ ok: false });
});
