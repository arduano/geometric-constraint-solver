// SPDX-License-Identifier: GPL-3.0-or-later

import assert from "node:assert/strict";
import test from "node:test";
import { readFileSync } from "node:fs";

import * as publicManagedIr from "@geosolve/sketch-code/ir";
// @ts-expect-error Generic one-phase mutation DTOs are not public IR API.
import type { ManagedSketchMutation as PublicManagedSketchMutation } from "@geosolve/sketch-code/ir";
import { definePatch } from "../src/authoring.js";
import { recordPatchArtifact } from "../src/compiler.js";
import {
  applyManagedSketchMutation,
  applyManagedSketchSourceMutation,
  applyManagedSourcePatch,
  compileManagedSource,
  executeManagedSketch,
  canonicalExecutedSketchArtifact,
  managedContentDigest,
  type ManagedDeclarationDraft,
} from "../src/managed.js";

const emptySketchSource = `"use geosolve sketch";
import { sketch } from "@geosolve/sketch-code";

export default sketch(($) => {
  return {};
});
`;

const localizedSource = `"use geosolve sketch";
import { sketch, mm } from '@geosolve/sketch-code'; // keep import trivia
export default sketch(($) => {
  // 🧭 日本語: keep coordinates readable
  const a=$.geometry.segment('a',{ start : [0, 0], end:[12, 0] /* endpoint */ }); const b = $.geometry.segment("b", {start:[0,1],end:[4,1]}); // b annotation
  return { a, b }; // keep output shorthand
});
`;

test("localized value patches retain Unicode, comments, quote style and same-line neighbors", () => {
  const current = compileManagedSource(localizedSource);
  const receipt = applyManagedSketchSourceMutation(current, {
    mutation: "set_value", declaration: "a", path: ["end", 0],
    expected: { kind: "number", value: 12 }, value: { kind: "number", value: 14 },
  }, { source: localizedSource });
  const start = localizedSource.indexOf("12, 0");
  assert.deepEqual(receipt.patch.edits, [{ start, end: start + 2, expected: "12", replacement: "14" }]);
  assert.equal(receipt.source, localizedSource.replace("12, 0", "14, 0"));
  assert.equal(applyManagedSourcePatch(localizedSource, receipt.patch), receipt.source);
  assert.equal(receipt.patch.baseSourceDigest, managedContentDigest(localizedSource));
  assert.equal(receipt.baseSourceDigest, current.ir.source_digest);
  assert.equal(receipt.compiled.inputSourceDigest, receipt.patch.candidateSourceDigest);
  assert.equal(compileManagedSource(receipt.source).canonicalArtifactJson, receipt.compiled.canonicalArtifactJson);
  assert.equal(current.normalizedSource, compileManagedSource(localizedSource).normalizedSource);
});

test("localized patch authentication rejects stale text, forged slices, offset units and candidate bytes", () => {
  const receipt = applyManagedSketchSourceMutation(compileManagedSource(localizedSource), {
    mutation: "set_value", declaration: "a", path: ["end", 0],
    expected: { kind: "number", value: 12 }, value: { kind: "number", value: 14 },
  }, { source: localizedSource });
  assert.throws(() => applyManagedSourcePatch(localizedSource + " ", receipt.patch), /exact source digest/u);
  const edit = receipt.patch.edits[0]!;
  for (const forged of [
    { ...edit, expected: "13" }, { ...edit, start: -1 }, { ...edit, start: edit.start + 0.5 },
    { ...edit, end: localizedSource.length + 1 }, { ...edit, replacement: "15" },
  ]) assert.throws(() => applyManagedSourcePatch(localizedSource, { ...receipt.patch, edits: [forged] }));
  assert.throws(() => applyManagedSourcePatch(localizedSource, { ...receipt.patch, edits: [edit, edit] }), /offsets, order/u);
  const split = localizedSource.indexOf("🧭") + 1;
  const replacement = localizedSource.slice(0, split) + "x" + localizedSource.slice(split);
  assert.throws(() => applyManagedSourcePatch(localizedSource, {
    baseSourceDigest: managedContentDigest(localizedSource), candidateSourceDigest: managedContentDigest(replacement),
    edits: [{ start: split, end: split, expected: "", replacement: "x" }],
  }), /UTF-16/u);
  assert.throws(() => applyManagedSketchSourceMutation(compileManagedSource(localizedSource), {
    mutation: "set_suppressed", target: { target: "declaration", declaration: "a" }, suppressed: true,
  }, { source: localizedSource.replace("12, 0", "13, 0") }), /does not match its compiled authority/u);
});

test("localized metadata insertion, update and reset preserve neighboring properties and comments", () => {
  const current = compileManagedSource(localizedSource);
  const inserted = applyManagedSketchSourceMutation(current, {
    mutation: "set_metadata", target: { target: "declaration", declaration: "a" },
    property: "label", value: { kind: "string", value: "Passage 🧭" },
  }, { source: localizedSource });
  assert.equal(inserted.patch.edits.length, 1);
  assert.equal(inserted.patch.edits[0]!.expected, "");
  assert.ok(inserted.source.includes("end:[12, 0] /* endpoint */ "));
  assert.ok(inserted.source.includes('}); const b = $.geometry.segment("b", {start:[0,1],end:[4,1]}); // b annotation'));
  const updated = applyManagedSketchSourceMutation(inserted.compiled, {
    mutation: "set_metadata", target: { target: "declaration", declaration: "a" },
    property: "label", value: { kind: "string", value: "Water" },
  }, { source: inserted.source });
  assert.equal(updated.patch.edits.length, 1);
  assert.equal(updated.patch.edits[0]!.expected, '"Passage 🧭"');
  const reset = applyManagedSketchSourceMutation(updated.compiled, {
    mutation: "set_metadata", target: { target: "declaration", declaration: "a" }, property: "label", value: null,
  }, { source: updated.source });
  assert.equal(reset.compiled.artifact.declarations[0]?.family, "geometry.segment");
  assert.ok(!reset.source.includes("label:"));
  assert.ok(reset.source.includes("/* endpoint */"));
  assert.ok(reset.source.includes("return { a, b }; // keep output shorthand"));
});

test("localized lifecycle mutations preserve same-line statements and exact moved comments", () => {
  const current = compileManagedSource(localizedSource);
  const reordered = applyManagedSketchSourceMutation(current, {
    mutation: "reorder_declaration", declaration: "b", before: "a",
  }, { source: localizedSource });
  assert.ok(reordered.source.indexOf('const b =') < reordered.source.indexOf("const a="));
  assert.ok(reordered.source.includes('const b = $.geometry.segment("b", {start:[0,1],end:[4,1]}); // b annotation'));
  assert.ok(reordered.source.includes("// 🧭 日本語: keep coordinates readable"));
  assert.ok(reordered.source.includes("const a=$.geometry.segment('a',{ start : [0, 0], end:[12, 0] /* endpoint */ });"));
  const suppressed = applyManagedSketchSourceMutation(current, {
    mutation: "set_suppressed", target: { target: "declaration", declaration: "a" }, suppressed: true,
  }, { source: localizedSource });
  assert.ok(suppressed.source.includes("$.suppress(a);"));
  assert.equal(suppressed.patch.edits.length, 1);
  assert.equal(suppressed.patch.edits[0]!.expected, "");
  const restored = applyManagedSketchSourceMutation(suppressed.compiled, {
    mutation: "set_suppressed", target: { target: "declaration", declaration: "a" }, suppressed: false,
  }, { source: suppressed.source });
  assert.deepEqual(restored.compiled.artifact.suppressions, []);
  assert.ok(restored.source.includes("// b annotation"));
  const deleted = applyManagedSketchSourceMutation(current, {
    mutation: "delete", target: { target: "declaration", declaration: "a" },
  }, { source: localizedSource });
  assert.ok(!deleted.source.includes("const a="));
  assert.ok(deleted.source.includes('const b = $.geometry.segment("b", {start:[0,1],end:[4,1]}); // b annotation'));
  assert.equal(deleted.compiled.artifact.declarations.length, 1);
  assert.deepEqual(deleted.compiled.artifact.output, { kind: "object", value: { b: { kind: "reference", value: { declaration: "b", path: [] } } } });
});

test("localized insertion closes helper imports and preserves existing CRLF source bytes", () => {
  const source = localizedSource.replace("sketch, mm", "sketch").replaceAll("\n", "\r\n");
  const inserted = applyManagedSketchSourceMutation(compileManagedSource(source), {
    mutation: "insert_declarations", declarations: [circleDraft("circle", "circle")],
  }, { source });
  assert.ok(inserted.source.includes("'@geosolve/sketch-code'; // keep import trivia\r\n"));
  assert.ok(inserted.source.includes("const a=$.geometry.segment('a',{ start : [0, 0], end:[12, 0] /* endpoint */ }); const b ="));
  assert.equal(inserted.compiled.artifact.declarations.length, 3);
  assert.ok(inserted.patch.edits.every((edit) => edit.expected === ""));
  assert.ok(!inserted.source.replaceAll("\r\n", "").includes("\n"));
});

test("localized document and parameter metadata preserve original callback and scalar spelling", () => {
  const source = localizedSource.replace("  // 🧭", "  const width=mm(/* dimension */ 1.2e1);\n  // 🧭");
  const extracted = applyManagedSketchSourceMutation(compileManagedSource(source), {
    mutation: "extract_parameter", declaration: "width", path: [], symbol: "channelWidth", variable: "width",
  }, { source });
  assert.ok(extracted.source.includes('$.parameter("channelWidth", mm(/* dimension */ 1.2e1))'));
  assert.ok(extracted.source.includes("const a=$.geometry.segment('a',{ start : [0, 0], end:[12, 0] /* endpoint */ }); const b ="));
  assert.equal(extracted.compiled.artifact.parameters?.[0]?.declaration, "channelWidth");
  const titled = applyManagedSketchSourceMutation(extracted.compiled, {
    mutation: "set_metadata", target: { target: "document" }, property: "title", value: { kind: "string", value: "Plate 🧭" },
  }, { source: extracted.source });
  assert.equal(titled.patch.edits.length, 1);
  assert.equal(titled.patch.edits[0]!.expected, "");
  const marked = applyManagedSketchSourceMutation(titled.compiled, {
    mutation: "set_metadata", target: { target: "parameter", declaration: "channelWidth" }, property: "isKeyParameter", value: { kind: "bool", value: true },
  }, { source: titled.source });
  assert.equal(marked.compiled.artifact.parameters?.[0]?.presentation.isKeyParameter, true);
  const reset = applyManagedSketchSourceMutation(marked.compiled, {
    mutation: "set_metadata", target: { target: "document" }, property: "title", value: null,
  }, { source: marked.source });
  assert.equal(reset.compiled.artifact.document, undefined);
  assert.ok(reset.source.includes("return { a, b }; // keep output shorthand"));
});

test("localized batch values authenticate all expected owners before returning any patch", () => {
  const current = compileManagedSource(localizedSource);
  const values = [
    { declaration: "a", path: ["end", 0], expected: { kind: "number", value: 12 }, value: { kind: "number", value: 14 } },
    { declaration: "b", path: ["end", 0], expected: { kind: "number", value: 4 }, value: { kind: "number", value: 6 } },
  ] as const;
  const receipt = applyManagedSketchSourceMutation(current, { mutation: "set_values", values }, { source: localizedSource });
  assert.equal(receipt.patch.edits.length, 2);
  assert.equal(receipt.source, localizedSource.replace("12, 0", "14, 0").replace("end:[4,1]", "end:[6,1]"));
  assert.throws(() => applyManagedSketchSourceMutation(current, {
    mutation: "set_values", values: [values[0], { ...values[1], expected: { kind: "number", value: 5 } }],
  }, { source: localizedSource }), /expected/u);
  assert.equal(current.canonicalArtifactJson, compileManagedSource(localizedSource).canonicalArtifactJson);
  const noop = applyManagedSketchSourceMutation(current, {
    mutation: "reorder_declaration", declaration: "a", before: "a",
  }, { source: localizedSource });
  assert.deepEqual(noop.patch.edits, []);
  assert.equal(noop.source, localizedSource);
});

test("localized patches reject ill-formed Unicode and bounded transport amplification", () => {
  const source = "valid 🧭 text";
  const patch = { baseSourceDigest: managedContentDigest(source), candidateSourceDigest: managedContentDigest(source), edits: [] };
  assert.equal(applyManagedSourcePatch(source, patch), source);
  for (const malformed of ["\ud800", "\udc00", "\ud800x"]) {
    assert.throws(() => applyManagedSourcePatch(malformed, {
      baseSourceDigest: managedContentDigest(malformed), candidateSourceDigest: managedContentDigest(malformed), edits: [],
    }), /unpaired UTF-16/u);
    assert.throws(() => applyManagedSourcePatch(source, {
      ...patch, edits: [{ start: 0, end: 0, expected: "", replacement: malformed }],
    }), /UTF-16/u);
  }
  assert.throws(() => applyManagedSourcePatch(source, {
    ...patch, edits: [{ start: 0, end: 0, expected: "", replacement: "x".repeat(4 * 1024 * 1024 + 1) }],
  }), /replacement text exceeds/u);
});

test("legacy managed receipt retains its exact Rust wire shape while localized transport is explicit", () => {
  const current = compileManagedSource(localizedSource);
  const mutation = { mutation: "set_value", declaration: "a", path: ["end", 0], expected: { kind: "number", value: 12 }, value: { kind: "number", value: 14 } } as const;
  const legacy = applyManagedSketchMutation(current, mutation);
  assert.deepEqual(Object.keys(legacy), ["baseSourceDigest", "candidateSourceDigest", "compiled"]);
  const localized = applyManagedSketchSourceMutation(current, mutation, { source: localizedSource });
  assert.equal(localized.compiled.canonicalArtifactJson, legacy.compiled.canonicalArtifactJson);
  assert.equal(localized.compiled.canonicalIrJson, legacy.compiled.canonicalIrJson);
  assert.equal("applyManagedSketchSourceMutation" in publicManagedIr, false);
  assert.equal(typeof publicManagedIr.applyManagedSourcePatch, "function");
});

function circleDraft(
  variable: string,
  symbol: string,
  unit = "mm",
): ManagedDeclarationDraft {
  return {
    variable,
    symbol,
    builder_path: ["geometry", "centerRadiusCircle"],
    arguments: {
      kind: "object",
      value: {
        center: {
          kind: "array",
          value: [
            { kind: "number", value: -3.633667933314297 },
            { kind: "number", value: 5.39811540404595 },
          ],
        },
        label: {
          kind: "string",
          value: "center-radius-circle-0000000000000001",
        },
        radius: {
          kind: "unit",
          value: { unit, value: 1.2599398842396183 },
        },
        role: { kind: "string", value: "profile" },
      },
    },
  };
}

const cleanBezierSource = `"use geosolve sketch";
import { sketch } from "@geosolve/sketch-code";

export default sketch(($) => {
  const bezier = $.geometry.quadraticBezier("bezier", {
    start: [0, 0],
    control: [1, 2],
    end: [3, 0],
  });
  return { bezier };
});
`;

test("named geometry executes and cold-replays its ordinary output", () => {
  const compiled = compileManagedSource(cleanBezierSource);
  assert.equal(compiled.artifact.declarations.length, 1);
  assert.equal(compiled.artifact.declarations[0]?.family, "geometry.quadraticBezier");
  assert.deepEqual(compiled.artifact.output, {
    kind: "object",
    value: {
      bezier: {
        kind: "reference",
        value: {
          declaration: "bezier",
          path: [],
        },
      },
    },
  });
  const replayed = compileManagedSource(compiled.normalizedSource);
  assert.equal(replayed.canonicalIrJson, compiled.canonicalIrJson);
  assert.equal(replayed.canonicalArtifactJson, compiled.canonicalArtifactJson);
});

test("explicit Segment and Polyline branches survive compiler normalization and replay", () => {
  const source = `"use geosolve sketch";
import { sketch } from "@geosolve/sketch-code";

export default sketch(($) => {
  const segment = $.geometry.segment("segment", {
    start: [0, 0],
    end: [3, 4],
    branchDirection: [0.8, 0.6],
  });
  const open = $.geometry.polyline("open", {
    vertices: [
      { key: "a", position: [0, 0] },
      { key: "b", position: [2, 0] },
      { key: "c", position: [2, 2] },
    ],
    branchDirections: [[0.8, 0.6], [0, 1]],
  });
  const closed = $.geometry.polyline("closed", {
    vertices: [
      { key: "a", position: [0, 0] },
      { key: "b", position: [2, 0] },
      { key: "c", position: [2, 2] },
    ],
    closed: true,
    branchDirections: [[1, 0], [0, 1], [-1, 0]],
  });
  return { segment, open, closed };
});
`;
  const compiled = compileManagedSource(source);
  const declarations = compiled.ir.statements.filter((statement) =>
    statement.statement === "declaration"
  );
  assert.equal(declarations.length, 3);
  assert.match(compiled.normalizedSource, /branchDirection: \[0\.8, 0\.6\]/u);
  assert.match(
    compiled.normalizedSource,
    /branchDirections: \[\[0\.8, 0\.6\], \[0, 1\]\]/u,
  );
  assert.match(
    compiled.normalizedSource,
    /branchDirections: \[\[1, 0\], \[0, 1\], \[-1, 0\]\]/u,
  );
  const replayed = compileManagedSource(compiled.normalizedSource);
  assert.equal(replayed.canonicalIrJson, compiled.canonicalIrJson);
  assert.equal(replayed.canonicalArtifactJson, compiled.canonicalArtifactJson);
});

test("canvas circle insertion closes its generated mm import exactly once", () => {
  const initial = compileManagedSource(emptySketchSource);
  const inserted = applyManagedSketchMutation(initial, {
    mutation: "insert_declarations",
    declarations: [circleDraft("geometry1", "geometry1")],
  });

  assert.match(
    inserted.compiled.normalizedSource,
    /^import \{ sketch, mm \} from "@geosolve\/sketch-code";$/mu,
  );
  assert.match(
    inserted.compiled.normalizedSource,
    /radius: mm\(1\.2599398842396183\)/u,
  );
  assert.equal(
    inserted.compiled.artifact.declarations[0]?.family,
    "geometry.centerRadiusCircle",
  );
  const radiusConsumer = inserted.compiled.artifact.value_consumers.find((
    consumer,
  ) =>
    consumer.target.target === "declaration" &&
    consumer.target.declaration === "geometry1" &&
    consumer.target.family === "geometry.centerRadiusCircle" &&
    consumer.property.length === 1 &&
    consumer.property[0] === "radius"
  );
  assert.ok(
    radiusConsumer,
    "the generated radius must remain a runtime value consumer",
  );

  const cold = compileManagedSource(inserted.compiled.normalizedSource);
  assert.equal(cold.canonicalIrJson, inserted.compiled.canonicalIrJson);
  assert.equal(
    cold.canonicalArtifactJson,
    inserted.compiled.canonicalArtifactJson,
  );

  const insertedAgain = applyManagedSketchMutation(inserted.compiled, {
    mutation: "insert_declarations",
    declarations: [circleDraft("geometry2", "geometry2")],
  });
  const sdkImport = insertedAgain.compiled.ir.imports.find((entry) =>
    entry.module === "@geosolve/sketch-code" &&
    entry.bindings.includes("sketch")
  );
  assert.deepEqual(sdkImport?.bindings, ["sketch", "mm"]);
  assert.equal(
    insertedAgain.compiled.ir.imports.flatMap((entry) => entry.bindings)
      .filter((binding) => binding === "mm").length,
    1,
  );

  const deleted = applyManagedSketchMutation(insertedAgain.compiled, {
    mutation: "delete",
    target: { target: "declaration", declaration: "geometry1" },
  });
  assert.deepEqual(deleted.compiled.ir.imports, [{
    module: "@geosolve/sketch-code",
    bindings: ["sketch", "mm"],
  }]);

  const splitImports = compileManagedSource(emptySketchSource.replace(
    'import { sketch } from "@geosolve/sketch-code";',
    'import { sketch } from "@geosolve/sketch-code";\n' +
      'import { mm } from "@geosolve/sketch-code";',
  ));
  const insertedWithSplitImports = applyManagedSketchMutation(splitImports, {
    mutation: "insert_declarations",
    declarations: [circleDraft("geometry1", "geometry1")],
  });
  assert.deepEqual(insertedWithSplitImports.compiled.ir.imports, [
    { module: "@geosolve/sketch-code", bindings: ["sketch"] },
    { module: "@geosolve/sketch-code", bindings: ["mm"] },
  ]);
});

test("generated mm and rad imports preserve existing binding and declaration order", () => {
  const helperPatch = definePatch({}, (p) => ({
    edge: p.geometry.segment("edge", {
      start: [0, 0],
      end: [1, 0],
    }),
  }));
  const options = {
    patches: { helperPatch: recordPatchArtifact(helperPatch) },
  };
  const source = `"use geosolve sketch";
import { cm, sketch } from "@geosolve/sketch-code";
import { helperPatch } from "./helper.patch.ts";

export default sketch(($) => {
  const first = $.geometry.segment("first", {
    start: [0, 0],
    end: [4, 0],
  });
  const second = $.geometry.segment("second", {
    start: [0, 0],
    end: [0, 4],
  });
  return {};
});
`;
  const initial = compileManagedSource(source, options);
  const inserted = applyManagedSketchMutation(initial, {
    mutation: "insert_declarations",
    declarations: [
      circleDraft("circle", "circle"),
      {
        variable: "angle",
        symbol: "angle",
        builder_path: ["dimension", "orientedAngle"],
        arguments: {
          kind: "object",
          value: {
            first: {
              kind: "reference",
              value: { declaration: "first", path: ["span"] },
            },
            second: {
              kind: "reference",
              value: { declaration: "second", path: ["span"] },
            },
            value: {
              kind: "unit",
              value: { unit: "rad", value: Math.PI / 2 },
            },
            orientation: {
              kind: "string",
              value: "counterClockwise",
            },
          },
        },
      },
    ],
  }, options);

  assert.deepEqual(inserted.compiled.ir.imports, [
    {
      module: "@geosolve/sketch-code",
      bindings: ["cm", "sketch", "mm", "rad"],
    },
    { module: "./helper.patch.ts", bindings: ["helperPatch"] },
  ]);
  assert.match(
    inserted.compiled.normalizedSource,
    /value: rad\(1\.5707963267948966\)/u,
  );
  const cold = compileManagedSource(
    inserted.compiled.normalizedSource,
    options,
  );
  assert.equal(cold.canonicalIrJson, inserted.compiled.canonicalIrJson);
  assert.equal(
    cold.canonicalArtifactJson,
    inserted.compiled.canonicalArtifactJson,
  );
});

test("unsupported generated units reject without changing accepted input", () => {
  const initial = compileManagedSource(emptySketchSource);
  const acceptedSource = initial.normalizedSource;
  const acceptedIr = initial.canonicalIrJson;
  const acceptedArtifact = initial.canonicalArtifactJson;

  assert.throws(
    () =>
      applyManagedSketchMutation(initial, {
        mutation: "insert_declarations",
        declarations: [circleDraft("geometry1", "geometry1", "cm")],
      }),
    /unsupported managed sketch unit cm/u,
  );
  assert.equal(initial.normalizedSource, acceptedSource);
  assert.equal(initial.canonicalIrJson, acceptedIr);
  assert.equal(initial.canonicalArtifactJson, acceptedArtifact);
  assert.deepEqual(initial.ir.imports, [{
    module: "@geosolve/sketch-code",
    bindings: ["sketch"],
  }]);
});

test("whole patch declaration results retain one runtime reference", () => {
  const wholeSegment = definePatch({}, (p) => ({
    segment: p.geometry.segment("segment", {
      start: [0, 0],
      end: [2, 0],
    }),
  }));
  const source = `"use geosolve sketch";
import { sketch } from "@geosolve/sketch-code";
import { wholeSegment } from "./whole-segment.patch.ts";

export default sketch(($) => {
  const generated = $.use("generated", wholeSegment, {});
  return { segment: generated.segment };
});
`;
  const compiled = compileManagedSource(source, {
    patches: { wholeSegment: recordPatchArtifact(wholeSegment) },
  });

  assert.deepEqual(compiled.artifact.output, {
    kind: "object",
    value: {
      segment: {
        kind: "reference",
        value: {
          declaration: "generated",
          path: ["segment"],
        },
      },
    },
  });
});

test("closed source requires the canonical directive and named authoring methods", () => {
  assert.throws(
    () => compileManagedSource(cleanBezierSource.replace(
      "use geosolve sketch",
      "use geosolve",
    )),
    /first statement must be exactly "use geosolve sketch"/u,
  );
  assert.throws(
    () => compileManagedSource(cleanBezierSource.replace(
      "$.geometry.quadraticBezier",
      "$.geometry.unknown",
    )),
    /unsupported managed sketch declaration family/u,
  );
  assert.throws(
    () => compileManagedSource(cleanBezierSource.replace(
      "$.geometry.quadraticBezier",
      "$.constraint.externalPointCoincident",
    )),
    /requires immutable host-snapshot authority/u,
  );
});

test("named declaration arguments reject retired transport properties at any depth", () => {
  for (const property of [
    "recipe",
    "inputs",
    "fields",
    "values",
    "results",
    "operationOutputs",
    "outputs",
    "editLens",
  ]) {
    const source = cleanBezierSource.replace(
      "    start: [0, 0],",
      `    start: [0, 0],\n    nested: [{ ${property}: [] }],`,
    );
    assert.throws(
      () => compileManagedSource(source),
      /retired transport property/u,
      `direct ${property}`,
    );
    const referencedSource = cleanBezierSource
      .replace(
        "export default sketch(($) => {",
        `export default sketch(($) => {\n  const legacyPayload = { nested: [{ ${property}: [] }] };`,
      )
      .replace(
        "    start: [0, 0],",
        "    start: [0, 0],\n    metadata: legacyPayload,",
      );
    assert.throws(
      () => compileManagedSource(referencedSource),
      /retired transport property/u,
      `referenced ${property}`,
    );
  }
});

test("public IR handoff excludes ticketless one-phase rewrite helpers", () => {
  assert.equal(typeof publicManagedIr.compileManagedSource, "function");
  for (const retired of [
    "applyManagedSketchMutation",
    "recompileManagedSketchIr",
    "parseManagedSource",
    "printManagedSource",
    "executeManagedSketch",
    "executeManagedSource",
  ]) {
    assert.equal(retired in publicManagedIr, false, retired);
  }
});

test("source-native metadata preserves parameter provenance and explicit overview intent", () => {
  const current = compileManagedSource(`"use geosolve sketch";
import { sketch, mm } from "@geosolve/sketch-code";
export default sketch({ title: "Channel plate", dimensions: { areKeyConstraintsByDefault: true } }, ($) => {
  const width = $.parameter("channelWidth", mm(12), { label: "Channel width", isKeyParameter: true });
  const equalWidth = $.parameter("otherWidth", mm(12));
  const line = $.geometry.segment("line", { start: [0, 0], end: [12, 0], description: "Full passage" });
  const size = $.dimension.curveLength("size", { curve: line, value: width, isKeyConstraint: false });
  const another = $.dimension.curveLength("another", { curve: line, value: width });
  return { line, size, another, equalWidth };
});`);
  assert.equal(current.ir.format, "geosolve-managed-sketch-ir-v4");
  assert.equal(canonicalExecutedSketchArtifact(executeManagedSketch(current.ir)), current.canonicalArtifactJson);
  assert.deepEqual(current.artifact.document, { title: "Channel plate", dimensions: { areKeyConstraintsByDefault: true } });
  const [shared, equal] = current.artifact.parameters!;
  assert.notEqual(shared!.value_site, equal!.value_site);
  assert.equal(current.artifact.value_consumers.filter((consumer) => consumer.value_site === shared!.value_site).length, 2);
  assert.equal(current.artifact.value_consumers.filter((consumer) => consumer.value_site === equal!.value_site).length, 0);
  assert.deepEqual(current.artifact.presentations?.find((entry) => entry.declaration === "size")?.presentation, { isKeyConstraint: false });
  const changed = applyManagedSketchMutation(current, { mutation: "set_value", declaration: "channelWidth", path: [], expected: { kind: "unit", value: { unit: "mm", value: 12 } }, value: { kind: "unit", value: { unit: "mm", value: 14 } } }).compiled;
  assert.equal(changed.artifact.parameters?.[0]?.value.kind, "unit");
  assert.deepEqual(changed.artifact.parameters?.[1]?.value, equal!.value);
});

test("metadata edits reset overrides and extract values without changing ownership", () => {
  const current = compileManagedSource(`"use geosolve sketch";
import { sketch, mm } from "@geosolve/sketch-code";
export default sketch(($) => {
  const width = mm(12);
  const line = $.geometry.segment("line", { start: [0, 0], end: [12, 0] });
  const size = $.dimension.curveLength("size", { curve: line, value: width });
  return { line, size };
});`);
  const extracted = applyManagedSketchMutation(current, { mutation: "extract_parameter", declaration: "width", path: [], symbol: "channelWidth", variable: "width", presentation: { label: "通路幅", isKeyParameter: true } }).compiled;
  assert.match(extracted.normalizedSource, /\$\.parameter\("channelWidth", mm\(12\)/u);
  assert.equal(extracted.artifact.parameters?.[0]?.presentation.label, "通路幅");
  const titled = applyManagedSketchMutation(extracted, { mutation: "set_metadata", target: { target: "document" }, property: "title", value: { kind: "string", value: "Demo plate" } }).compiled;
  assert.equal(titled.artifact.document?.title, "Demo plate");
  const marked = applyManagedSketchMutation(titled, { mutation: "set_metadata", target: { target: "declaration", declaration: "size" }, property: "isKeyConstraint", value: { kind: "bool", value: false } }).compiled;
  assert.deepEqual(marked.artifact.presentations, [{ declaration: "size", presentation: { isKeyConstraint: false } }]);
  const reset = applyManagedSketchMutation(marked, { mutation: "set_metadata", target: { target: "declaration", declaration: "size" }, property: "isKeyConstraint", value: null }).compiled;
  assert.equal(reset.artifact.presentations, undefined);
  assert.deepEqual(reset.artifact.output, current.artifact.output);
});

test("metadata rejects ambiguous names, invalid label bytes, duplicate parameter identity and forged current", () => {
  const make = (body: string) => `"use geosolve sketch"; import { sketch, mm } from "@geosolve/sketch-code"; export default sketch(($) => { ${body} return {}; });`;
  assert.throws(() => compileManagedSource(make('const value = $.parameter("width", mm(12), { key: true });')), /unknown presentation field/u);
  assert.throws(() => compileManagedSource(make(`const value = $.parameter("width", mm(12), { label: "${"幅".repeat(86)}" });`)), /256 UTF-8/u);
  assert.throws(() => compileManagedSource(make('const first = $.parameter("width", 12); const second = $.parameter("width", 12);')), /duplicate parameter ID/u);
  const current = compileManagedSource(make('const value = $.parameter("width", mm(12));'));
  assert.throws(() => applyManagedSketchMutation({ ...current, canonicalArtifactJson: current.canonicalArtifactJson.replace('"presentation":{}', '"presentation":{"isKeyParameter":true}') }, { mutation: "set_metadata", target: { target: "parameter", declaration: "width" }, property: "label", value: { kind: "string", value: "Width" } }), /does not match/u);
});

test("inline parameter extraction rewrites only its consumer and unused named values survive consumer deletion", () => {
  const current = compileManagedSource(`"use geosolve sketch"; import { sketch, mm } from "@geosolve/sketch-code"; export default sketch(($) => {
    const line = $.geometry.segment("line", { start: [0, 0], end: [12, 0] });
    const size = $.dimension.curveLength("size", { curve: line, value: mm(12) });
    return { line, size };
  });`);
  const extracted = applyManagedSketchMutation(current, { mutation: "extract_parameter", declaration: "size", path: ["value"], symbol: "width", variable: "width", presentation: { isKeyParameter: true } }).compiled;
  assert.deepEqual(extracted.artifact.parameters?.[0]?.value, { kind: "unit", value: { unit: "mm", value: 12 } });
  assert.match(extracted.normalizedSource, /value: width/u);
  const deleted = applyManagedSketchMutation(extracted, { mutation: "delete", target: { target: "declaration", declaration: "size" } }).compiled;
  assert.equal(deleted.artifact.parameters?.length, 1);
  assert.equal(deleted.artifact.value_consumers.filter((consumer) => consumer.value_site === deleted.artifact.parameters?.[0]?.value_site).length, 0);
});


test("archived V3 authority survives exact restoration and upgrades on its first metadata edit", () => {
  const bytes = readFileSync(new URL("../../test/fixtures/legacy-v3/managed-compiler-envelope.json", import.meta.url), "utf8");
  const current = JSON.parse(bytes) as import("../src/managed.js").CompiledManagedSource;
  const source = current.normalizedSource;
  assert.equal(current.ir.format, "geosolve-managed-sketch-ir-v3");
  assert.equal(canonicalExecutedSketchArtifact(executeManagedSketch(current.ir)), current.canonicalArtifactJson);
  const upgraded = applyManagedSketchMutation(current, { mutation: "set_metadata", target: { target: "document" }, property: "title", value: { kind: "string", value: "Restored project" } }).compiled;
  assert.equal(current.normalizedSource, source);
  assert.equal(JSON.stringify(current), bytes);
  assert.equal(upgraded.ir.format, "geosolve-managed-sketch-ir-v4");
  assert.equal(upgraded.artifact.document?.title, "Restored project");
  assert.deepEqual(upgraded.artifact.output, current.artifact.output);
  assert.deepEqual(upgraded.artifact.declarations, current.artifact.declarations);
  assert.deepEqual(upgraded.artifact.value_consumers, current.artifact.value_consumers);
});


test("named parameter IDs cannot collide with implicit scalar identities in either source order", () => {
  const source = (body: string) => `"use geosolve sketch"; import { sketch, mm } from "@geosolve/sketch-code"; export default sketch(($) => { ${body} return {}; });`;
  assert.throws(() => compileManagedSource(source('const ordinary = mm(12); const other = $.parameter("ordinary", mm(14));')), /duplicate parameter ID/u);
  assert.throws(() => compileManagedSource(source('const other = $.parameter("ordinary", mm(14)); const ordinary = mm(12);')), /duplicate source scalar identity/u);
});
