// SPDX-License-Identifier: GPL-3.0-or-later

import assert from "node:assert/strict";
import test from "node:test";

import { definePatch, mm, t } from "../src/authoring.js";
import {
  PATCH_ARTIFACT_FORMAT,
  SKETCH_CODE_SDK_ABI,
  compilePatchArtifact,
} from "../src/compiler.js";

test("patch artifacts retain local IDs and ordinary result paths", () => {
  const roundEveryCorner = definePatch(
    { corners: t.keyed(t.corner()), radius: t.length() },
    (p, { corners, radius }) => ({
      fillets: p.each(corners, (corner) =>
        p.computed.fillet("fillet", { corner, radius })),
    }),
  );
  const compiled = compilePatchArtifact({
    source: "source",
    moduleSpecifier: "./patches/rounded-polyline.patch.ts",
    exportName: "roundEveryCorner",
    patch: roundEveryCorner,
  });

  assert.equal(compiled.artifact.format, PATCH_ARTIFACT_FORMAT);
  assert.equal(compiled.artifact.sdk_abi, SKETCH_CODE_SDK_ABI);
  assert.deepEqual(compiled.artifact.inputs, { corners: "collection", radius: "scalar" });
  assert.deepEqual(compiled.artifact.outputs, { fillets: "collection" });
  assert.deepEqual(compiled.artifact.collections, [{
    rule: "each",
    path: ["fillets"],
    input: "corners",
    member_key_field: "key",
    templates: [["fillet"]],
  }]);
  assert.deepEqual(compiled.artifact.templates[0]?.path, ["fillet"]);
  assert.equal(compiled.artifact.templates[0]?.result_path, null);
  assert.equal(compiled.artifact.templates[0]?.declaration_family, "computed.fillet");
  assert.equal(compiled.artifact.templates[0]?.result_output, null);
  assert.deepEqual(compiled.artifact.templates[0]?.outputs, [{
    path: ["arc"],
    kind: "curve_span",
  }]);
  assert.doesNotMatch(compiled.canonicalJson, /node_id|port_id|equation|residual|function/u);
  assert.deepEqual(JSON.parse(compiled.canonicalJson), compiled.artifact);
});

test("renaming an ordinary return does not erase mandatory patch-local identity", () => {
  const edge = definePatch(
    { start: t.point(), end: t.point() },
    (p, { start, end }) => ({
      nested: {
        visibleEdge: p.geometry.segment("semanticEdge", { start, end }).span,
      },
    }),
  );
  const compiled = compilePatchArtifact({
    source: "edge",
    moduleSpecifier: "./patches/edge.patch.ts",
    exportName: "edge",
    patch: edge,
  });

  assert.deepEqual(compiled.artifact.outputs, { nested: "collection" });
  assert.deepEqual(compiled.artifact.templates, [{
    path: ["semanticEdge"],
    result_path: ["nested", "visibleEdge"],
    result_output: ["span"],
    declaration_family: "geometry.segment",
    arguments: {
      argument: "object",
      value: {
        end: {
          argument: "binding",
          value: {
            source: "input",
            name: "end",
            path: [],
            expected_kind: "point",
          },
        },
        start: {
          argument: "binding",
          value: {
            source: "input",
            name: "start",
            path: [],
            expected_kind: "point",
          },
        },
      },
    },
    outputs: [
      { path: ["curve"], kind: "curve" },
      { path: ["end"], kind: "point" },
      { path: ["span"], kind: "curve_span" },
      { path: ["start"], kind: "point" },
    ],
  }]);
});

test("the clean recorder rejects duplicate local IDs and non-finite literals", () => {
  const duplicate = definePatch(
    { start: t.point(), end: t.point() },
    (p, { start, end }) => ({
      first: p.geometry.segment("edge", { start, end }).span,
      second: p.geometry.segment("edge", { start: end, end: start }).span,
    }),
  );
  assert.throws(() => compilePatchArtifact({
    source: "duplicate",
    moduleSpecifier: "./patches/duplicate.patch.ts",
    exportName: "duplicate",
    patch: duplicate,
  }), /duplicate patch-local declaration ID/u);

  const nonFinite = definePatch(
    { start: t.point(), end: t.point() },
    (p, { start, end }) => ({
      edge: p.geometry.segment("edge", { start, end, branchDirection: [Number.NaN, 0] }).span,
    }),
  );
  assert.throws(() => compilePatchArtifact({
    source: "nonfinite",
    moduleSpecifier: "./patches/nonfinite.patch.ts",
    exportName: "nonfinite",
    patch: nonFinite,
  }), /must be finite/u);

  const unsafe = definePatch({}, (p) => ({
    legacy: (p.geometry as unknown as Readonly<Record<string, (
      id: string,
      values: Readonly<Record<string, unknown>>,
    ) => unknown>>).line!("legacy", { start: [0, 0], end: [1, 0] }),
  }));
  assert.throws(() => compilePatchArtifact({
    source: "unsafe",
    moduleSpecifier: "./patches/unsafe.patch.ts",
    exportName: "unsafe",
    patch: unsafe,
  }), /unsupported patch authoring method geometry\.line/u);
});

test("nested declaration arguments retain every binding and branch literal", () => {
  const nested = definePatch(
    {
      first: t.curveSpan(),
      second: t.curveSpan(),
      radius: t.length(),
    },
    (p, { first, second, radius }) => ({
      rounded: p.computed.filletSet("rounded", {
        radius,
        corners: [{
          key: "joint",
          parents: [{
            span: first,
            parameter: 0.25,
            winding: 0,
            neighborhood: { kind: "local", lower: 0.1, upper: 0.4 },
            normalSide: "left",
            trimEndpoint: "end",
            periodicAnchor: { kind: "none" },
          }, {
            span: second,
            parameter: 0.75,
            winding: 1,
            neighborhood: { kind: "interior" },
            normalSide: "right",
            trimEndpoint: "start",
            periodicAnchor: { kind: "anchor", parameter: 0.5, winding: 1 },
          }],
          endpointOrder: "firstThenSecond",
          sweep: "counterClockwise",
        }],
      }),
    }),
  );
  const template = compilePatchArtifact({
    source: "nested",
    moduleSpecifier: "./patches/nested.patch.ts",
    exportName: "nested",
    patch: nested,
  }).artifact.templates[0];

  assert.equal(template?.arguments.argument, "object");
  if (template?.arguments.argument !== "object") return;
  const corners = template.arguments.value.corners;
  assert.equal(corners?.argument, "array");
  if (corners?.argument !== "array") return;
  const corner = corners.value[0];
  assert.equal(corner?.argument, "object");
  if (corner?.argument !== "object") return;
  const parents = corner.value.parents;
  assert.equal(parents?.argument, "array");
  if (parents?.argument !== "array") return;
  const firstParent = parents.value[0];
  const secondParent = parents.value[1];
  assert.equal(firstParent?.argument, "object");
  assert.equal(secondParent?.argument, "object");
  if (firstParent?.argument !== "object" || secondParent?.argument !== "object") return;
  assert.deepEqual(firstParent.value.span, {
    argument: "binding",
    value: {
      source: "input",
      name: "first",
      path: [],
      expected_kind: "curve_span",
    },
  });
  assert.deepEqual(secondParent.value.span, {
    argument: "binding",
    value: {
      source: "input",
      name: "second",
      path: [],
      expected_kind: "curve_span",
    },
  });
  assert.deepEqual(firstParent.value.neighborhood, {
    argument: "object",
    value: {
      kind: { argument: "literal", value: { kind: "string", value: "local" } },
      lower: { argument: "literal", value: { kind: "number", value: 0.1 } },
      upper: { argument: "literal", value: { kind: "number", value: 0.4 } },
    },
  });
});

test("catalog output paths compose nested tuple results without flattened aliases", () => {
  const composed = definePatch(
    { first: t.point(), opposite: t.point() },
    (p, { first, opposite }) => {
      const panel = p.geometry.twoPointAlignedRectangle("panel", {
        firstCorner: first,
        oppositeCorner: opposite,
      });
      const diagonal = p.geometry.segment("diagonal", {
        start: panel.corners[0],
        end: panel.corners[2],
      });
      return { panel, diagonal: diagonal.span };
    },
  );
  const artifact = compilePatchArtifact({
    source: "composed",
    moduleSpecifier: "./patches/composed.patch.ts",
    exportName: "composed",
    patch: composed,
  }).artifact;

  const panel = artifact.templates.find((template) => template.path[0] === "panel");
  const diagonal = artifact.templates.find((template) => template.path[0] === "diagonal");
  assert.deepEqual(panel?.result_path, ["panel"]);
  assert.equal(panel?.result_output, null);
  assert.deepEqual(panel?.outputs.filter((output) => output.path[0] === "corners"), [
    { path: ["corners", 0], kind: "point" },
    { path: ["corners", 1], kind: "point" },
    { path: ["corners", 2], kind: "point" },
    { path: ["corners", 3], kind: "point" },
  ]);
  assert.deepEqual(diagonal?.result_output, ["span"]);
  assert.equal(diagonal?.arguments.argument, "object");
  if (diagonal?.arguments.argument !== "object") return;
  assert.deepEqual(diagonal.arguments.value.start, {
    argument: "binding",
    value: {
      source: "template_output",
      template: ["panel"],
      path: ["corners", 0],
      expected_kind: "point",
    },
  });
  assert.deepEqual(diagonal.arguments.value.end, {
    argument: "binding",
    value: {
      source: "template_output",
      template: ["panel"],
      path: ["corners", 2],
      expected_kind: "point",
    },
  });
});

test("dynamic catalog results retain collection and member semantic paths", () => {
  const keyed = definePatch({}, (p) => {
    const path = p.geometry.polyline("path", {
      vertices: [
        { key: "start", position: [0, 0] },
        { key: "bend", position: [4, 0] },
        { key: "end", position: [4, 3] },
      ],
    });
    const fillets = p.computed.filletSet("fillets", {
      radius: mm(1),
      corners: [{
        key: "bend",
        parents: [{
          span: path.segments.byKey.start,
          parameter: 1,
          winding: 0,
          neighborhood: { kind: "end" },
          normalSide: "left",
          trimEndpoint: "end",
          periodicAnchor: { kind: "none" },
        }, {
          span: path.segments.byKey.bend,
          parameter: 0,
          winding: 0,
          neighborhood: { kind: "start" },
          normalSide: "left",
          trimEndpoint: "start",
          periodicAnchor: { kind: "none" },
        }],
        endpointOrder: "firstThenSecond",
        sweep: "counterClockwise",
      }],
    });
    return { path, fillets };
  });
  const artifact = compilePatchArtifact({
    source: "keyed",
    moduleSpecifier: "./patches/keyed.patch.ts",
    exportName: "keyed",
    patch: keyed,
  }).artifact;
  const path = artifact.templates.find((template) => template.path[0] === "path");
  const fillets = artifact.templates.find((template) => template.path[0] === "fillets");

  assert.ok(path?.outputs.some((output) =>
    output.kind === "collection" && JSON.stringify(output.path) === JSON.stringify(["segments"])));
  assert.ok(path?.outputs.some((output) =>
    output.kind === "curve_span" && JSON.stringify(output.path) ===
      JSON.stringify(["segments", { member: "bend" }])));
  assert.ok(fillets?.outputs.some((output) =>
    output.kind === "feature_corner" && JSON.stringify(output.path) ===
      JSON.stringify(["fillets", { member: "bend" }, "corner"])));
  assert.ok(fillets?.outputs.some((output) =>
    output.kind === "curve_span" && JSON.stringify(output.path) ===
      JSON.stringify(["fillets", { member: "bend" }, "arc"])));
});

test("plan-dependent operation result paths fail closed", () => {
  const planned = definePatch({}, (p) => ({
    rectangle: p.operation.rectangle("rectangle", {
      origin: [0, 0],
      width: mm(10),
      height: mm(5),
      role: "profile",
    }),
  }));
  assert.throws(() => compilePatchArtifact({
    source: "planned",
    moduleSpecifier: "./patches/planned.patch.ts",
    exportName: "planned",
    patch: planned,
  }), /plan-dependent outputs/u);
});

test("typed feature inputs retain deep semantic member paths", () => {
  const featureInput = definePatch(
    { source: t.feature("cubicBezier") },
    (p, { source }) => ({
      handle: p.geometry.segment("handle", {
        start: source.start,
        end: source.controls[1],
      }).span,
    }),
  );
  const template = compilePatchArtifact({
    source: "featureInput",
    moduleSpecifier: "./patches/feature-input.patch.ts",
    exportName: "featureInput",
    patch: featureInput,
  }).artifact.templates[0];
  assert.equal(template?.arguments.argument, "object");
  if (template?.arguments.argument !== "object") return;
  assert.deepEqual(template.arguments.value.start, {
    argument: "binding",
    value: {
      source: "input",
      name: "source",
      path: ["start"],
      expected_kind: "point",
    },
  });
  assert.deepEqual(template.arguments.value.end, {
    argument: "binding",
    value: {
      source: "input",
      name: "source",
      path: ["controls", 1],
      expected_kind: "point",
    },
  });
});

test("collection callback records nested result namespaces independently of local IDs", () => {
  const nested = definePatch(
    { centers: t.record(t.point()), radius: t.length() },
    (p, { centers, radius }) => ({
      reliefs: p.mapRecord(centers, (center) => {
        const circle = p.geometry.centerRadiusCircle("circle", { center, radius });
        const dimension = p.dimension.radius("radius", { curve: circle.curve, value: radius });
        return { geometry: circle.curve, annotation: dimension };
      }),
    }),
  );
  const templates = compilePatchArtifact({
    source: "nested",
    moduleSpecifier: "./patches/nested-record.patch.ts",
    exportName: "nested",
    patch: nested,
  }).artifact.templates;
  assert.deepEqual(templates.find((template) => template.path[0] === "circle")?.result_path, ["geometry"]);
  assert.deepEqual(templates.find((template) => template.path[0] === "circle")?.result_output, ["curve"]);
  assert.deepEqual(templates.find((template) => template.path[0] === "radius")?.result_path, ["annotation"]);
  assert.equal(templates.find((template) => template.path[0] === "radius")?.result_output, null);
});

test("patch-only rounded rectangle preserves profile and scalar-derived mount centres", () => {
  const mountingPlate = definePatch(
    {
      width: t.length(),
      height: t.length(),
      cornerRadius: t.length(),
      holeRadius: t.length(),
    },
    (p, input) => {
      const rounded = p.computed.roundedRectangleProfile("profile", {
        width: input.width,
        height: input.height,
        cornerRadius: input.cornerRadius,
      });
      return {
        profile: rounded.profile,
        holes: {
          nw: p.geometry.centerRadiusCircle("hole-nw", {
            center: rounded.mounts.nw,
            radius: input.holeRadius,
          }).curve,
        },
      };
    },
  );
  const artifact = compilePatchArtifact({
    source: "mountingPlate",
    moduleSpecifier: "./patches/mounting-plate.patch.ts",
    exportName: "mountingPlate",
    patch: mountingPlate,
  }).artifact;
  assert.deepEqual(artifact.outputs, { holes: "collection", profile: "profile" });
  const profile = artifact.templates.find((template) => template.path[0] === "profile");
  assert.equal(profile?.declaration_family, "computed.roundedRectangleProfile");
  assert.deepEqual(profile?.result_path, ["profile"]);
  assert.deepEqual(profile?.result_output, ["profile"]);
  assert.deepEqual(profile?.outputs, [
    { path: ["mounts", "ne"], kind: "point" },
    { path: ["mounts", "nw"], kind: "point" },
    { path: ["mounts", "se"], kind: "point" },
    { path: ["mounts", "sw"], kind: "point" },
    { path: ["profile"], kind: "profile" },
  ]);
  const northwest = artifact.templates.find((template) => template.path[0] === "hole-nw");
  assert.equal(northwest?.arguments.argument, "object");
  if (northwest?.arguments.argument !== "object") return;
  assert.deepEqual(northwest.arguments.value.center, {
    argument: "binding",
    value: {
      source: "template_output",
      template: ["profile"],
      path: ["mounts", "nw"],
      expected_kind: "point",
    },
  });
  assert.deepEqual(northwest.result_path, ["holes", "nw"]);
  assert.deepEqual(northwest.result_output, ["curve"]);
});

test("native-defining tangent centre and conic weighted control stay explicit literals", () => {
  const nativeFields = definePatch({ source: t.curveSpan() }, (p, { source }) => ({
    tangent: p.geometry.tangentArc("tangent", {
      center: [2, 2],
      start: [0, 0],
      end: [4, 0],
      source: { span: source },
    }),
    conic: p.geometry.rationalQuadraticConic("conic", {
      start: [0, 0],
      weightedMiddle: [2, 3],
      end: [4, 0],
      middleWeight: 0.75,
    }),
  }));
  const templates = compilePatchArtifact({
    source: "nativeFields",
    moduleSpecifier: "./patches/native-fields.patch.ts",
    exportName: "nativeFields",
    patch: nativeFields,
  }).artifact.templates;
  const tangent = templates.find((template) => template.path[0] === "tangent");
  const conic = templates.find((template) => template.path[0] === "conic");
  assert.equal(tangent?.arguments.argument, "object");
  assert.equal(conic?.arguments.argument, "object");
  if (tangent?.arguments.argument !== "object" || conic?.arguments.argument !== "object") return;
  assert.deepEqual(tangent.arguments.value.center, {
    argument: "array",
    value: [
      { argument: "literal", value: { kind: "number", value: 2 } },
      { argument: "literal", value: { kind: "number", value: 2 } },
    ],
  });
  assert.deepEqual(conic.arguments.value.weightedMiddle, {
    argument: "array",
    value: [
      { argument: "literal", value: { kind: "number", value: 2 } },
      { argument: "literal", value: { kind: "number", value: 3 } },
    ],
  });
});
