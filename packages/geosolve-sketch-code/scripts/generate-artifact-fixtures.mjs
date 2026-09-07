// SPDX-License-Identifier: GPL-3.0-or-later

import assert from "node:assert/strict";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { crossBrace } from "../dist/examples/braced-frame.patch.js";
import { adaptiveLanterns } from "../dist/examples/adaptive-lanterns.patch.js";
import { bridgeCables } from "../dist/examples/bridge-cables.patch.js";
import { compassCore } from "../dist/examples/compass-core.patch.js";
import { cornerReliefs } from "../dist/examples/corner-reliefs.patch.js";
import { harnessRoute } from "../dist/examples/harness-route.patch.js";
import { mountingPlate } from "../dist/examples/mounting-plate.patch.js";
import { roundEveryCorner } from "../dist/examples/rounded-polyline.patch.js";
import { fillets } from "../dist/examples/typed-panel.patch.js";
import { waterChannel } from "../dist/examples/water-channel.patch.js";
import {
  compilePatchArtifact,
  recordPatchArtifact,
} from "../dist/src/compiler.js";
import {
  applyManagedSketchMutation,
  compileManagedSource,
} from "../dist/src/managed.js";

const packageRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const check = process.argv.includes("--check");
const manifoldWaterArtifact = JSON.parse(await readFile(resolve(packageRoot,
  "../../crates/geosolve-sketch-code/assets/bundled-samples/pc-water-manifold/patches/water-channel.artifact.json"), "utf8"));

const fixtures = [
  {
    source: "examples/adaptive-lanterns.patch.ts",
    fixture: "test/fixtures/adaptive-lanterns.artifact.json",
    moduleSpecifier: "./patches/adaptive-lanterns.patch.ts",
    exportName: "adaptiveLanterns",
    patch: adaptiveLanterns,
  },
  {
    source: "examples/bridge-cables.patch.ts",
    fixture: "test/fixtures/bridge-cables.artifact.json",
    moduleSpecifier: "./patches/bridge-cables.patch.ts",
    exportName: "bridgeCables",
    patch: bridgeCables,
  },
  {
    source: "examples/compass-core.patch.ts",
    fixture: "test/fixtures/compass-core.artifact.json",
    moduleSpecifier: "./patches/compass-core.patch.ts",
    exportName: "compassCore",
    patch: compassCore,
  },
  {
    source: "examples/corner-reliefs.patch.ts",
    fixture: "test/fixtures/corner-reliefs.artifact.json",
    moduleSpecifier: "./patches/corner-reliefs.patch.ts",
    exportName: "cornerReliefs",
    patch: cornerReliefs,
  },
  {
    source: "examples/harness-route.patch.ts",
    fixture: "test/fixtures/harness-route.artifact.json",
    moduleSpecifier: "./patches/harness-route.patch.ts",
    exportName: "harnessRoute",
    patch: harnessRoute,
  },
  {
    source: "examples/rounded-polyline.patch.ts",
    fixture: "test/fixtures/round-every-corner.artifact.json",
    moduleSpecifier: "./patches/round-every-corner.patch.ts",
    exportName: "roundEveryCorner",
    patch: roundEveryCorner,
  },
  {
    source: "examples/typed-panel.patch.ts",
    fixture: "test/fixtures/fillet-record.artifact.json",
    moduleSpecifier: "./patches/fillet-record.patch.ts",
    exportName: "fillets",
    patch: fillets,
  },
  {
    source: "examples/braced-frame.patch.ts",
    fixture: "test/fixtures/cross-brace.artifact.json",
    moduleSpecifier: "./patches/cross-brace.patch.ts",
    exportName: "crossBrace",
    patch: crossBrace,
  },
  {
    source: "examples/mounting-plate.patch.ts",
    fixture: "test/fixtures/mounting-plate.artifact.json",
    moduleSpecifier: "./patches/mounting-plate.patch.ts",
    exportName: "mountingPlate",
    patch: mountingPlate,
  },
  {
    source: "examples/water-channel.patch.ts",
    fixture: "test/fixtures/water-channel.artifact.json",
    moduleSpecifier: "./patches/water-channel.patch.ts",
    exportName: "waterChannel",
    patch: waterChannel,
  },
];

const authoredCircleOne = {
  mutation: "insert_declarations",
  declarations: [{
    variable: "geometry1",
    symbol: "geometry1",
    builder_path: ["geometry", "centerRadiusCircle"],
    arguments: {
      kind: "object",
      value: {
        center: {
          kind: "array",
          value: [
            { kind: "number", value: -0.6 },
            { kind: "number", value: 2.6 },
          ],
        },
        label: {
          kind: "string",
          value: "center-radius-circle-0000000000000001",
        },
        radius: {
          kind: "unit",
          value: { unit: "mm", value: 1.7999999999999998 },
        },
        role: { kind: "string", value: "profile" },
      },
    },
    group: "Canvas additions",
    suppressed: false,
  }],
};

const managedNumber = (value) => ({ kind: "number", value });
const managedString = (value) => ({ kind: "string", value });
const managedArray = (value) => ({ kind: "array", value });
const managedObject = (value) => ({ kind: "object", value });
const managedReference = (declaration, path) => ({
  kind: "reference",
  value: { declaration, path },
});
const canvasDeclaration = (variable, builder_path, value) => ({
  variable,
  symbol: variable,
  builder_path,
  arguments: managedObject(value),
  group: "Canvas additions",
  suppressed: false,
});
const periodicContact = (parameter) => managedObject({
  neighborhood: managedObject({ kind: managedString("interior") }),
  orientation: managedString("none"),
  parameter: managedNumber(parameter),
  winding: managedNumber(0),
});
const twoCircleSnappedSegment = {
  mutation: "insert_declarations",
  declarations: [
    canvasDeclaration("segment3", ["geometry", "segment"], {
      branchDirection: managedArray([
        managedNumber(1),
        managedNumber(-1.4802973661655742e-16),
      ]),
      end: managedArray([
        managedNumber(1.1999991280705136),
        managedNumber(1.75714111328125),
      ]),
      label: managedString("segment-0000000000000002"),
      role: managedString("profile"),
      start: managedArray([
        managedNumber(-1.8000008719321237),
        managedNumber(1.7571411132812504),
      ]),
    }),
    canvasDeclaration("constraint5", ["constraint", "pointOnCurve"], {
      contact: periodicContact(6.283184096163701),
      curve: managedReference("geometry1", ["span"]),
      label: managedString("segment-0000000000000002-relation-0000"),
      point: managedReference("segment3", ["start"]),
    }),
    canvasDeclaration("constraint6", ["constraint", "pointOnCurve"], {
      contact: periodicContact(3.141593864604212),
      curve: managedReference("geometry2", ["span"]),
      label: managedString("segment-0000000000000002-relation-0001"),
      point: managedReference("segment3", ["end"]),
    }),
    canvasDeclaration("constraint4", ["constraint", "horizontal"], {
      label: managedString("segment-0000000000000002-relation-0002"),
      span: managedReference("segment3", ["span"]),
    }),
  ],
};

const managedSketchFixtures = [
  {
    source: "../../crates/geosolve-sketch-code/assets/bootstrap/authored-empty.sketch.ts",
    fixture: "../../crates/geosolve-sketch-code/assets/bootstrap/authored-empty.compiled.json",
  },
  {
    source: "test/fixtures/managed-contact-range-base.sketch.ts",
    fixture: "test/fixtures/managed-contact-range-base.json",
  },
  {
    source: "test/fixtures/managed-contact-range-limited.sketch.ts",
    fixture: "test/fixtures/managed-contact-range-limited.json",
  },
  {
    source: "test/fixtures/managed-contact-range-infeasible-base.sketch.ts",
    fixture: "test/fixtures/managed-contact-range-infeasible-base.json",
  },
  {
    source: "test/fixtures/managed-contact-range-infeasible-limited.sketch.ts",
    fixture: "test/fixtures/managed-contact-range-infeasible-limited.json",
  },
  {
    source: "test/fixtures/managed-contact-supporting-line.sketch.ts",
    fixture: "test/fixtures/managed-contact-supporting-line.json",
  },
  {
    source: "test/fixtures/managed-clean-operation-catalog.sketch.ts",
    fixture: "test/fixtures/managed-clean-operation-catalog.json",
  },
  {
    source: "test/fixtures/managed-clean-geometry-catalog.sketch.ts",
    fixture: "test/fixtures/managed-clean-geometry-catalog.json",
  },
  {
    source: "test/fixtures/managed-clean-relation-catalog.sketch.ts",
    fixture: "test/fixtures/managed-clean-relation-catalog.json",
  },
  {
    source: "test/fixtures/managed-clean-dimension-catalog.sketch.ts",
    fixture: "test/fixtures/managed-clean-dimension-catalog.json",
  },
  {
    source: "test/fixtures/managed-curve-tangency.sketch.ts",
    fixture: "test/fixtures/managed-curve-tangency.json",
  },
  {
    source: "test/fixtures/managed-geometry-controls.sketch.ts",
    fixture: "test/fixtures/managed-geometry-controls.json",
  },
  {
    source: "test/fixtures/managed-geometry-controls.sketch.ts",
    fixture: "test/fixtures/managed-geometry-controls-edited.json",
    mutations: [{
      mutation: "set_value",
      declaration: "cubic",
      path: ["firstControl", 0],
      expected: { kind: "number", value: 2 },
      value: { kind: "number", value: 2.5 },
    }, {
      mutation: "set_value",
      declaration: "cubic",
      path: ["firstControl", 1],
      expected: { kind: "number", value: 4 },
      value: { kind: "number", value: 4.5 },
    }, {
      mutation: "set_value",
      declaration: "periodic",
      path: ["controls", 3, "weight"],
      expected: { kind: "number", value: 1 },
      value: { kind: "number", value: 1.5 },
    }],
  },
  {
    source: "test/fixtures/managed-polyline.sketch.ts",
    fixture: "test/fixtures/managed-polyline.json",
  },
  {
    source: "../../crates/geosolve-sketch-code/assets/bootstrap/authored-empty.sketch.ts",
    fixture: "test/fixtures/managed-empty-circle.json",
    mutation: authoredCircleOne,
  },
  {
    source: "test/fixtures/managed-two-circles.sketch.ts",
    fixture: "test/fixtures/managed-two-circles.json",
  },
  {
    source: "test/fixtures/managed-two-circles.sketch.ts",
    fixture: "test/fixtures/managed-two-circles-snapped-segment.json",
    mutation: twoCircleSnappedSegment,
  },
  {
    source: "test/fixtures/managed-compiler-envelope.sketch.ts",
    fixture: "test/fixtures/managed-compiler-envelope.json",
  },
  {
    source: "test/fixtures/managed-compiler-envelope.sketch.ts",
    fixture: "test/fixtures/managed-compiler-envelope-restored.json",
    mutation: {
      mutation: "set_suppressed",
      target: { target: "declaration", declaration: "radius" },
      suppressed: false,
    },
  },
  {
    source: "test/fixtures/managed-compiler-envelope.sketch.ts",
    fixture: "test/fixtures/managed-reference-detached.json",
    mutation: {
      mutation: "set_value",
      declaration: "segment",
      path: ["start"],
      expected: {
        kind: "reference",
        value: { declaration: "point", path: ["point"] },
      },
      value: {
        kind: "array",
        value: [
          { kind: "number", value: -1.9999999999999993 },
          { kind: "number", value: 4 },
        ],
      },
    },
  },
  {
    source: "test/fixtures/managed-fillet.sketch.ts",
    fixture: "test/fixtures/managed-fillet.json",
  },
  {
    source: "test/fixtures/managed-fillet.sketch.ts",
    fixture: "test/fixtures/managed-fillet-radius.json",
    mutation: {
      mutation: "set_value",
      declaration: "round",
      path: ["radius"],
      expected: { kind: "unit", value: { unit: "mm", value: 1 } },
      value: { kind: "unit", value: { unit: "mm", value: 1.25 } },
    },
  },
  {
    source: "test/managed/managed-lifecycle.sketch.ts",
    fixture: "test/fixtures/managed-lifecycle-base.json",
    options: { patches: { fillets: recordPatchArtifact(fillets) } },
  },
  {
    source: "test/managed/managed-lifecycle.sketch.ts",
    fixture: "test/fixtures/managed-lifecycle-reordered.json",
    options: { patches: { fillets: recordPatchArtifact(fillets) } },
    mutations: [{
      mutation: "reorder_declaration",
      declaration: "guide",
      before: null,
    }],
  },
  {
    source: "test/managed/managed-lifecycle.sketch.ts",
    fixture: "test/fixtures/managed-lifecycle-direct-suppressed.json",
    options: { patches: { fillets: recordPatchArtifact(fillets) } },
    mutations: [{
      mutation: "reorder_declaration",
      declaration: "guide",
      before: null,
    }, {
      mutation: "set_suppressed",
      target: { target: "declaration", declaration: "guide" },
      suppressed: true,
    }],
  },
  {
    source: "test/managed/managed-lifecycle.sketch.ts",
    fixture: "test/fixtures/managed-lifecycle-both-suppressed.json",
    options: { patches: { fillets: recordPatchArtifact(fillets) } },
    mutations: [{
      mutation: "reorder_declaration",
      declaration: "guide",
      before: null,
    }, {
      mutation: "set_suppressed",
      target: { target: "declaration", declaration: "guide" },
      suppressed: true,
    }, {
      mutation: "set_suppressed",
      target: {
        target: "generated",
        address: {
          invocation: "cornerFillets",
          template: ["fillet"],
          member_key: ["lowerLeft"],
          output: ["field:arc"],
        },
      },
      suppressed: true,
    }],
  },
  {
    source: "test/managed/managed-lifecycle-renamed.sketch.ts",
    fixture: "test/fixtures/managed-lifecycle-renamed.json",
    options: { patches: { fillets: recordPatchArtifact(fillets) } },
  },
  {
    source: "test/managed/managed-lifecycle-group-removed.sketch.ts",
    fixture: "test/fixtures/managed-lifecycle-group-removed.json",
    options: { patches: { fillets: recordPatchArtifact(fillets) } },
  },
  {
    source: "test/managed/managed-profile-offset-closure.sketch.ts",
    fixture: "test/fixtures/managed-profile-offset-closure-base.json",
  },
  {
    source: "test/managed/managed-polyline-profile-offset.sketch.ts",
    fixture: "test/fixtures/managed-polyline-profile-offset.json",
  },
  {
    source: "test/fixtures/managed-channel-boundary-driven.sketch.ts",
    fixture: "test/fixtures/managed-channel-boundary-driven.json",
    options: { patches: { waterChannel: manifoldWaterArtifact } },
  },
  {
    source: "test/fixtures/managed-channel-boundary-driven.sketch.ts",
    fixture: "test/fixtures/managed-channel-boundary-driven-edited.json",
    options: { patches: { waterChannel: manifoldWaterArtifact } },
    mutation: {
      mutation: "set_value", declaration: "firstLength", path: ["value"],
      expected: { kind: "unit", value: { unit: "mm", value: 30 } },
      value: { kind: "unit", value: { unit: "mm", value: 32 } },
    },
  },
  {
    source: "test/fixtures/managed-channel-boundary.sketch.ts",
    fixture: "test/fixtures/managed-channel-boundary.json",
    options: { patches: { waterChannel: manifoldWaterArtifact } },
  },
  {
    source: "test/fixtures/managed-channel-boundary.sketch.ts",
    fixture: "test/fixtures/managed-channel-boundary-self-intersection.json",
    options: { patches: { waterChannel: manifoldWaterArtifact } },
    mutation: {
      mutation: "set_value", declaration: "route", path: ["vertices", 4, "position", 1],
      expected: { kind: "number", value: 10 }, value: { kind: "number", value: 5 },
    },
  },
  {
    source: "test/managed/managed-profile-offset-closure.sketch.ts",
    fixture: "test/fixtures/managed-profile-offset-closure-reordered.json",
    mutations: [{
      mutation: "reorder_declaration",
      declaration: "profileOffset12",
      before: "guide",
    }],
  },
  {
    source: "test/managed/managed-profile-offset-closure.sketch.ts",
    fixture: "test/fixtures/managed-profile-offset-closure-restored-order.json",
    mutations: [{
      mutation: "reorder_declaration",
      declaration: "profileOffset12",
      before: "guide",
    }, {
      mutation: "reorder_declaration",
      declaration: "guide",
      before: "profileOffset12",
    }],
  },
  {
    source: "test/managed/managed-profile-offset-closure.sketch.ts",
    fixture: "test/fixtures/managed-profile-offset-closure-deleted.json",
    mutations: [{
      mutation: "delete",
      target: { target: "declaration", declaration: "profileOffset12" },
    }],
  },
];

for (const fixture of fixtures) {
  const source = await readFile(resolve(packageRoot, fixture.source));
  const compiled = compilePatchArtifact({
    source,
    moduleSpecifier: fixture.moduleSpecifier,
    exportName: fixture.exportName,
    patch: fixture.patch,
  });
  const destination = resolve(packageRoot, fixture.fixture);
  if (check) {
    assert.equal(
      await readFile(destination, "utf8"),
      compiled.canonicalJson,
      `${fixture.fixture} is stale; run npm run generate:fixtures`,
    );
  } else {
    await mkdir(dirname(destination), { recursive: true });
    await writeFile(destination, compiled.canonicalJson);
  }
}

for (const fixture of managedSketchFixtures) {
  const source = await readFile(resolve(packageRoot, fixture.source), "utf8");
  let compiled;
  try {
    compiled = compileManagedSource(source, fixture.options);
    const mutations = fixture.mutations ?? (
      fixture.mutation === undefined ? [] : [fixture.mutation]
    );
    for (const mutation of mutations) {
      compiled = applyManagedSketchMutation(
        compiled,
        mutation,
        fixture.options,
      ).compiled;
    }
  } catch (error) {
    throw new Error(`failed to compile managed fixture ${fixture.source}`, {
      cause: error,
    });
  }
  const canonicalEnvelope = JSON.stringify(compiled);
  const destination = resolve(packageRoot, fixture.fixture);
  if (check) {
    assert.equal(
      await readFile(destination, "utf8"),
      canonicalEnvelope,
      `${fixture.fixture} is stale; run npm run generate:fixtures`,
    );
  } else {
    await mkdir(dirname(destination), { recursive: true });
    await writeFile(destination, canonicalEnvelope);
  }
}
