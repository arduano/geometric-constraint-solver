// SPDX-License-Identifier: GPL-3.0-or-later

import assert from "node:assert/strict";
import test from "node:test";

import * as publicManagedIr from "@geosolve/sketch-code/ir";
// @ts-expect-error Generic one-phase mutation DTOs are not public IR API.
import type { ManagedSketchMutation as PublicManagedSketchMutation } from "@geosolve/sketch-code/ir";
import { definePatch } from "../src/authoring.js";
import { recordPatchArtifact } from "../src/compiler.js";
import {
  applyManagedSketchMutation,
  compileManagedSource,
  type ManagedDeclarationDraft,
} from "../src/managed.js";

const emptySketchSource = `"use geosolve sketch";
import { sketch } from "@geosolve/sketch-code";

export default sketch(($) => {
  return {};
});
`;

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
