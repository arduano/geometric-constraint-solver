// SPDX-License-Identifier: GPL-3.0-or-later

import {
  cm,
  definePatch,
  deg,
  inch,
  m,
  mm,
  rad,
  sketch,
  t,
} from "../src/authoring.js";
import type {
  AggregateBuilder,
  Angle,
  ComputedBuilder,
  ConstraintBuilder,
  DimensionBuilder,
  GeometryBuilder,
  Length,
  NativeCurveSpanRef,
  OperationBuilder,
  OutputRef,
  PatchBuilder,
  PointRef,
  SketchExecutionProject,
  SketchProject,
} from "../src/authoring.js";

type SameUnion<Left, Right> = [Left] extends [Right]
  ? [Right] extends [Left] ? true : false
  : false;

const geometryMethods = [
  "sketchPoint",
  "segment",
  "polyline",
  "midpointLine",
  "twoPointAlignedRectangle",
  "threePointCornerRectangle",
  "centerRectangle",
  "threePointCenterRectangle",
  "centerRadiusCircle",
  "twoPointDiameterCircle",
  "threePointCircle",
  "centerArc",
  "threePointArc",
  "tangentArc",
  "centerAxesEllipse",
  "axisEndpointsEllipse",
  "centerAxesEllipticalArc",
  "axisEndpointsEllipticalArc",
  "quadraticBezier",
  "cubicBezier",
  "rationalQuadraticConic",
  "parabola",
  "hyperbola",
  "openControlNurbs",
  "periodicControlNurbs",
] as const;
const geometryInventoryIsExact: SameUnion<
  (typeof geometryMethods)[number],
  keyof GeometryBuilder<unknown>
> = true;
geometryInventoryIsExact;

const constraintMethods = [
  "fixedPoint",
  "fixedCoordinate",
  "coincidentWithOrigin",
  "pointOnDatumAxis",
  "coincident",
  "horizontal",
  "vertical",
  "horizontalPoints",
  "verticalPoints",
  "horizontalPointToMidpoint",
  "verticalPointToMidpoint",
  "pointOnCurve",
  "parallel",
  "perpendicular",
  "collinearWithDatumAxis",
  "concentric",
  "collinear",
  "equalLength",
  "equalRadius",
  "midpoint",
  "symmetricAboutLine",
  "symmetricAboutDatumAxis",
  "lineCircleTangency",
  "circleCircleTangency",
  "circleArcTangency",
  "lineCurveTangency",
  "curveCurveContact",
  "curveCurveTangency",
  "curveDirection",
  "equalCurvature",
  "endpointContinuity",
  "lineLineFillet",
  "curveCurveFillet",
] as const;
const constraintInventoryIsExact: SameUnion<
  (typeof constraintMethods)[number],
  keyof ConstraintBuilder<unknown>
> = true;
constraintInventoryIsExact;

const dimensionMethods = [
  "pointDistance",
  "curveLength",
  "radius",
  "diameter",
  "orientedAngle",
  "supportingLineOffset",
  "exactTranslatedSegmentOffset",
  "profileOffset",
] as const;
const dimensionInventoryIsExact: SameUnion<
  (typeof dimensionMethods)[number],
  keyof DimensionBuilder<unknown>
> = true;
dimensionInventoryIsExact;

const operationMethods = [
  "split",
  "break",
  "trim",
  "extend",
  "mirror",
  "chamfer",
  "associativeFillet",
  "rectangle",
  "regularPolygon",
  "slot",
  "linearPattern",
  "profileOffset",
] as const;
const operationInventoryIsExact: SameUnion<
  (typeof operationMethods)[number],
  keyof OperationBuilder<unknown>
> = true;
operationInventoryIsExact;

const aggregateInventoryIsExact: SameUnion<
  "openChain" | "closedProfile",
  keyof AggregateBuilder<unknown>
> = true;
aggregateInventoryIsExact;

const computedInventoryIsExact: SameUnion<
  "filletSet",
  keyof ComputedBuilder<unknown>
> = true;
computedInventoryIsExact;

const lengths: readonly Length[] = [mm(1), cm(1), m(1), inch(1)];
const angles: readonly Angle[] = [deg(1), rad(1)];
lengths;
angles;

const ordinaryOutput = sketch((s) => {
  const west = s.geometry.segment("west", { start: [0, 0], end: [2, 0] });
  const south = s.geometry.segment("south", { start: [2, 0], end: [2, -2] });
  const curve = s.geometry.quadraticBezier("curve", {
    start: west.end,
    control: south.end,
    end: [4, 0],
    role: "profile",
  });
  const keyed = s.geometry.polyline("keyed", {
    vertices: [
      { key: "start", position: curve.end },
      { key: "corner", position: [5, 0] },
      { key: "end", position: [5, 1] },
    ],
  });
  const nurbs = s.geometry.openControlNurbs("spline", {
    controls: [
      { key: "a", position: keyed.vertices.byKey.start, weight: 1 },
      { key: "b", position: keyed.vertices.byKey.corner, weight: 1 },
      { key: "c", position: keyed.vertices.byKey.end, weight: 1 },
    ],
    degree: 2,
    gauge: "b",
  });
  return { curve, keyed, nurbs };
});
ordinaryOutput.output.curve.end;
ordinaryOutput.output.keyed.segments.byKey.start;
ordinaryOutput.output.nurbs.controls.byKey.b.weight;
// @ts-expect-error Exact keyed children reject unknown keys.
ordinaryOutput.output.nurbs.controls.byKey.missing;

const generated = definePatch(
  { start: t.point(), end: t.point() },
  (p, { start, end }) => ({
    edge: p.geometry.segment("edge", { start, end }),
  }),
);

sketch((s) => {
  const start = s.geometry.sketchPoint("start", { point: [0, 0] });
  const end = s.geometry.sketchPoint("end", { point: [1, 0] });
  const result = s.use("generated", generated, { start: start.point, end: end.point });
  const span: NativeCurveSpanRef<SketchExecutionProject> = result.edge.span;
  return { start, end, result, span };
});

declare interface Alpha extends SketchProject<"alpha"> {}
declare interface Beta extends SketchProject<"beta"> {}
declare const alphaBuilder: GeometryBuilder<Alpha>;
declare const alphaPoint: PointRef<Alpha>;
declare const betaPoint: PointRef<Beta>;
alphaBuilder.segment("valid", { start: alphaPoint, end: [1, 0] });
// @ts-expect-error Cross-project references cannot enter an Alpha declaration.
alphaBuilder.segment("foreign", { start: alphaPoint, end: betaPoint });
alphaBuilder.quadraticBezier("legacyBezier", {
  // @ts-expect-error The clean break removed tuple-key geometry inputs.
  inputs: [
    ["start", "point", alphaPoint],
    ["control", "point", [1, 1]],
    ["end", "point", [2, 0]],
  ],
});

declare const span: NativeCurveSpanRef<Alpha>;
declare const dimensions: DimensionBuilder<Alpha>;
dimensions.curveLength("length", { curve: span, value: mm(20) });
// @ts-expect-error Length-bearing fields require an explicit unit.
dimensions.curveLength("rawLength", { curve: span, value: 20 });

alphaBuilder.tangentArc("tangentArc", {
  center: [0, 1],
  start: alphaPoint,
  end: [2, 1],
  source: { span },
});
// @ts-expect-error Tangent Arc requires its explicit native centre.
alphaBuilder.tangentArc("missingCenter", {
  start: alphaPoint,
  end: [2, 1],
  source: { span },
});
alphaBuilder.rationalQuadraticConic("conic", {
  start: alphaPoint,
  weightedMiddle: [1, 1],
  end: [2, 0],
  middleWeight: 0.75,
});
alphaBuilder.rationalQuadraticConic("boundMiddle", {
  start: alphaPoint,
  // @ts-expect-error The stored homogeneous control is a literal Point2, not an alias.
  weightedMiddle: alphaPoint,
  end: [2, 0],
  middleWeight: 0.75,
});

declare const operations: OperationBuilder<Alpha>;
const polygonOperation = operations.regularPolygon("polygon", {
  center: [0, 0],
  radius: mm(20),
  sides: 6,
  rotation: deg(30),
  role: "profile",
});
const polygonSpan: NativeCurveSpanRef<Alpha> = polygonOperation.spans[0]!;
polygonSpan;
// @ts-expect-error Native operation results expose semantic fields, not a generic recipe.
polygonOperation.recipe;
operations.regularPolygon("rawAngle", {
  center: [0, 0],
  radius: mm(20),
  sides: 6,
  // @ts-expect-error Angle-bearing fields require an explicit unit.
  rotation: 30,
  role: "profile",
});

operations.mirror("legacyMirror", {
  // @ts-expect-error The clean break removed tuple-key operation inputs.
  inputs: [
    ["source", "curve", {}],
    ["axis", "span", {}],
  ],
});

declare const patchBuilder: PatchBuilder<Alpha>;
patchBuilder.geometry.segment("localEdge", { start: alphaPoint, end: [1, 0] });
// @ts-expect-error Patch declarations require a local ID.
patchBuilder.geometry.segment({ start: alphaPoint, end: [1, 0] });
patchBuilder.computed.fillet;
patchBuilder.computed.roundedRectangleProfile;
// @ts-expect-error Host-authored rounded profiles are custom-patch-only.
({} as ComputedBuilder<Alpha>).roundedRectangleProfile;
// @ts-expect-error Host-snapshot external constraints are not public standalone methods.
({} as ConstraintBuilder<Alpha>).externalPointCoincident;

declare const output: OutputRef<Alpha, "point">;
output;
