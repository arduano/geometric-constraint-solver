// SPDX-License-Identifier: GPL-3.0-or-later

import {
  CodeControlClient,
  createProject,
  definePatch,
  fillets,
  line,
  mm,
  point,
  polyline,
  rectangle,
  sketch,
  t,
} from "../src/index.js";
import type {
  CodeControlSnapshot,
  CurveSpanRef,
  FeatureRecord,
  KeyedFeatureCollection,
  LineFeature,
  ManagedSketchProject,
  NativeCurveSpanRef,
  OutputRef,
  RectangleFeature,
  ManagedControlToken,
} from "../src/index.js";

const alpha = createProject("alpha");
const beta = createProject("beta");
const alphaStart = point(alpha, "start");
const alphaEnd = point(alpha, "end");
const betaEnd = point(beta, "end");
const segment = line(alpha, "segment", alphaStart, alphaEnd);
const reservedProject = createProject("__geosolve_managed_v1__");
const reservedPoint = point(reservedProject, "outside");

const panel = rectangle(alpha, "panel");
const panelType: RectangleFeature<typeof alpha> = panel;
const bottom: CurveSpanRef<typeof alpha> = panelType.edges.bottom;
bottom;
const nativeBottom: NativeCurveSpanRef<typeof alpha> = panelType.edges.bottom;
nativeBottom;

const path = polyline(alpha, "path", ["lowerLeft", "upperRight", "tail"] as const);
const selected = {
  lowerLeft: path.filletableCorners.byKey.lowerLeft,
  upperRight: path.filletableCorners.byKey.upperRight,
};
const mapped = fillets(alpha, selected);
mapped.lowerLeft.arc;
mapped.upperRight.arc;
// @ts-expect-error A computed host output is not an already materialized native span.
const computedArcIsNotNative: NativeCurveSpanRef<typeof alpha> = mapped.lowerLeft.arc;
computedArcIsNotNative;

type ExactKeys = keyof typeof mapped;
const exactKey: ExactKeys = "lowerLeft";
exactKey;
// @ts-expect-error Mapped Fillet records preserve exact input keys.
mapped.lowerleft;

const collection: KeyedFeatureCollection<"lowerLeft" | "upperRight" | "tail", OutputRef<typeof alpha, "feature_corner">> =
  path.filletableCorners;
collection.byKey.tail;

// @ts-expect-error Cross-project references cannot enter an alpha declaration.
line(alpha, "foreign", alphaStart, betaEnd);
// @ts-expect-error A curve span cannot occupy a point input.
line(alpha, "wrong-kind", alphaStart, segment.span);
// @ts-expect-error Raw wire IDs/strings are not code-facing semantic inputs.
line(alpha, "raw-id", alphaStart, "0000000000000042");
// @ts-expect-error Rectangle named outputs reject misspellings.
panel.corners.lowerleft;

type MappedRecord = FeatureRecord<typeof mapped>;
const mappedRecord: MappedRecord = mapped;
mappedRecord;

const nativeSpanPatch = definePatch(
  { frame: t.feature("rectangle") },
  (p, { frame }) => ({
    rising: p.line(frame.corners.lowerLeft, frame.corners.upperRight).span,
  }),
);

const managedNativeSpanPatch = sketch(($) => {
  const frame = $.geometry.rectangle("nativeSpanFrame", {
    lowerLeft: [0, 0],
    upperRight: [20, 10],
  });
  const generated = $.use("nativeSpan", nativeSpanPatch, { frame });
  const native: NativeCurveSpanRef<ManagedSketchProject> = generated.rising;
  native;
  return $.outputs({ frame, generated });
});
managedNativeSpanPatch;

const managedRectangleDiagonal = sketch(($) => {
  const frame = $.geometry.rectangle("frame", {
    lowerLeft: [0, 0],
    upperRight: [60, 35],
  });
  const diagonal = $.geometry.line("diagonal", {
    start: frame.corners.lowerLeft,
    end: frame.corners.upperRight,
  });
  const mixedEndpointLine = $.geometry.line("mixedEndpointLine", {
    start: [10, 12],
    end: frame.corners.lowerRight,
  });
  const literalLine = $.geometry.line("literalLine", {
    start: [0, 0],
    end: [10, 12],
  });

  const managedLine: LineFeature<ManagedSketchProject> = diagonal;
  managedLine;

  // @ts-expect-error Managed line endpoints reject curve outputs.
  $.geometry.line("wrongKind", { start: frame.edges.bottom, end: frame.corners.upperRight });
  // @ts-expect-error Managed line endpoints reject references from another project.
  $.geometry.line("foreign", { start: alphaStart, end: frame.corners.upperRight });
  // @ts-expect-error The reserved public project name cannot forge the managed-only brand.
  $.geometry.line("forged", { start: reservedPoint, end: frame.corners.upperRight });
  // @ts-expect-error Managed line endpoints reject raw semantic ID strings.
  $.geometry.line("rawId", { start: "frame.corners.lowerLeft", end: frame.corners.upperRight });
  const rawDto = {
    declaration: "frame",
    output: ["corners", "lowerLeft"],
    kind: "point",
  } as const;
  // @ts-expect-error Managed line endpoints reject transport DTOs in place of lexical references.
  $.geometry.line("rawDto", { start: rawDto, end: frame.corners.upperRight });
  // @ts-expect-error Rectangle named outputs reject misspellings in managed source.
  $.geometry.line("misspelled", { start: frame.corners.lowerleft, end: frame.corners.upperRight });

  return $.outputs({ frame, diagonal, mixedEndpointLine, literalLine });
});
managedRectangleDiagonal;

const managedManifoldVocabulary = sketch(($) => {
  const route = $.geometry.polyline("route", {
    vertices: [
      { key: "inlet", position: [0, 0] },
      { key: "elbow", position: [20, 0] },
      { key: "outlet", position: [20, 10] },
    ],
    closed: false,
  });
  const screw = $.geometry.circle("screw", {
    center: [30, 10],
    radius: mm(2.5),
  });
  const constructionDatum = $.geometry.line("constructionDatum", {
    start: route.vertices.byKey.outlet,
    end: screw.center,
    role: "construction",
  });
  const join = $.constraint.coincident("join", {
    first: route.vertices.byKey.outlet,
    second: screw.center,
  });
  const anchor = $.constraint.fixedPoint("anchor", {
    point: route.vertices.byKey.inlet,
    target: [0, 0],
  });
  const screwX = $.constraint.fixedCoordinate("screwX", {
    point: screw.center,
    axis: "x",
    target: mm(30),
  });
  const routeLength = $.dimension.curveLength("routeLength", {
    curve: route.segments.byKey.inlet,
    target: mm(20),
  });
  const screwDiameter = $.dimension.diameter("screwDiameter", {
    curve: screw,
    target: mm(5),
    mode: "driving",
  });
  const typedAnchor: OutputRef<ManagedSketchProject, "constraint"> = anchor;
  const typedJoin: OutputRef<ManagedSketchProject, "constraint"> = join;
  const typedLength: OutputRef<ManagedSketchProject, "dimension"> = routeLength;
  const typedDiameter: OutputRef<ManagedSketchProject, "dimension"> = screwDiameter;
  typedAnchor;
  typedJoin;
  typedLength;
  typedDiameter;
  screwX;

  // @ts-expect-error Exact keyed Polyline members reject unknown keys.
  route.segments.byKey.missing;
  // @ts-expect-error A curve span cannot be fixed as a point.
  $.constraint.fixedPoint("spanFix", { point: route.segments.byKey.inlet, target: [0, 0] });
  // @ts-expect-error Fixed-coordinate axes are the closed Cartesian x/y set.
  $.constraint.fixedCoordinate("badAxis", { point: screw.center, axis: "z", target: 30 });
  // @ts-expect-error Curve-length dimensions require a native span or owning line.
  $.dimension.curveLength("pointLength", { curve: screw.center, target: 20 });
  // @ts-expect-error Diameter dimensions require a curve or owning circle.
  $.dimension.diameter("spanDiameter", { curve: route.segments.byKey.inlet, target: 5 });
  // @ts-expect-error Managed circles reject point references from another project.
  $.geometry.circle("foreignCircle", { center: alphaStart, radius: 2.5 });
  // @ts-expect-error Managed line roles are the closed profile/construction set.
  $.geometry.line("badRole", { start: route.vertices.byKey.inlet, end: screw.center, role: "datum" });
  // @ts-expect-error Coincident operands must both be point-like references.
  $.constraint.coincident("curveJoin", { first: route.segments.byKey.inlet, second: screw.center });
  // @ts-expect-error Coincident rejects a point reference from another project.
  $.constraint.coincident("foreignJoin", { first: alphaStart, second: screw.center });

  return $.outputs({ route, screw, constructionDatum, join, anchor, routeLength, screwDiameter });
});
managedManifoldVocabulary;

const managedFilletSet = sketch(($) => {
  const first = $.geometry.line("first", { start: [0, 0], end: [30, 0] });
  const second = $.geometry.line("second", { start: [30, 0], end: [30, 20] });
  const round = $.computed.filletSet("round", {
    radius: 4,
    corners: [{
      parents: [{
        span: first.span,
        parameter: 0.9,
        winding: 0,
        neighborhood: { kind: "local", lower: 0.75, upper: 1 },
        normalSide: "left",
        retainedEndpoint: "start",
        periodicAnchor: null,
      }, {
        span: second.span,
        parameter: 0.1,
        winding: 0,
        neighborhood: { kind: "interior" },
        normalSide: "right",
        retainedEndpoint: "end",
        periodicAnchor: null,
      }],
      endpointOrder: "firstThenSecond",
      sweep: "counterClockwise",
    }],
    suppressed: false,
  });
  const horizontal = $.constraint.horizontal("horizontal", {
    curve: first,
  });
  const vertical = $.constraint.vertical("vertical", {
    curve: second.span,
    suppressed: false,
  });
  const symmetric = $.constraint.symmetricAboutDatumAxis("symmetric", {
    first: first.start,
    second: first.end,
    axis: "y",
  });
  const hole = $.geometry.circle("hole", { center: first.start, radius: mm(2) });
  const holeRadius = $.dimension.radius("holeRadius", {
    curve: hole.circle,
    target: mm(2),
  });
  const typedVertical: OutputRef<ManagedSketchProject, "constraint"> = vertical;
  typedVertical;
  const typedSymmetric: OutputRef<ManagedSketchProject, "constraint"> = symmetric;
  typedSymmetric;
  const typedRadius: OutputRef<ManagedSketchProject, "dimension"> = holeRadius;
  typedRadius;

  const parent = {
    span: first.span,
    parameter: 0.9,
    winding: 0,
    neighborhood: { kind: "interior" as const },
    normalSide: "left" as const,
    retainedEndpoint: "start" as const,
    periodicAnchor: null,
  };

  // @ts-expect-error Direct Fillet parents reject raw semantic path strings.
  $.computed.filletSet("raw", { radius: 4, corners: [{ parents: [{ ...parent, span: "first.span" }, parent], endpointOrder: "firstThenSecond", sweep: "counterClockwise" }], suppressed: false });
  // @ts-expect-error Direct Fillet parents require curve spans, not point outputs.
  $.computed.filletSet("point", { radius: 4, corners: [{ parents: [{ ...parent, span: first.start }, parent], endpointOrder: "firstThenSecond", sweep: "counterClockwise" }], suppressed: false });
  // @ts-expect-error Direct Fillet parents require the lexical span output, not its owning line.
  $.computed.filletSet("line", { radius: 4, corners: [{ parents: [{ ...parent, span: first }, parent], endpointOrder: "firstThenSecond", sweep: "counterClockwise" }], suppressed: false });
  // @ts-expect-error Direct Fillet parents reject references from another project.
  $.computed.filletSet("foreign", { radius: 4, corners: [{ parents: [{ ...parent, span: segment.span }, parent], endpointOrder: "firstThenSecond", sweep: "counterClockwise" }], suppressed: false });
  // @ts-expect-error Direct Fillet parents reject transport DTOs.
  $.computed.filletSet("dto", { radius: 4, corners: [{ parents: [{ ...parent, span: { declaration: "first", output: ["span"] } }, parent], endpointOrder: "firstThenSecond", sweep: "counterClockwise" }], suppressed: false });
  // @ts-expect-error Direct Fillet parent branch state is required in full.
  $.computed.filletSet("incomplete", { radius: 4, corners: [{ parents: [{ span: first.span }, parent], endpointOrder: "firstThenSecond", sweep: "counterClockwise" }], suppressed: false });
  // @ts-expect-error Direct Fillet branch enums are closed.
  $.computed.filletSet("enum", { radius: 4, corners: [{ parents: [parent, parent], endpointOrder: "nearest", sweep: "counterClockwise" }], suppressed: false });
  // @ts-expect-error Direct Fillet radius rejects angular units.
  $.computed.filletSet("angle", { radius: { unit: "deg", value: 4 }, corners: [{ parents: [parent, parent], endpointOrder: "firstThenSecond", sweep: "counterClockwise" }], suppressed: false });
  // @ts-expect-error Managed units must come from the branded supported constructor.
  $.computed.filletSet("forgedUnit", { radius: { unit: "mm", value: 4 }, corners: [{ parents: [parent, parent], endpointOrder: "firstThenSecond", sweep: "counterClockwise" }], suppressed: false });
  // @ts-expect-error Managed Vertical requires a native curve span or its owning line.
  $.constraint.vertical("pointVertical", { curve: second.start });
  // @ts-expect-error Managed Vertical rejects raw semantic path strings.
  $.constraint.vertical("rawVertical", { curve: "second.span" });
  // @ts-expect-error Managed Vertical rejects native spans from another project.
  $.constraint.vertical("foreignVertical", { curve: segment.span });
  // @ts-expect-error Datum symmetry has a closed axis choice.
  $.constraint.symmetricAboutDatumAxis("badAxis", { first: first.start, second: first.end, axis: "z" });
  // @ts-expect-error Radius dimensions require a curve rather than a curve span.
  $.dimension.radius("spanRadius", { curve: first.span, target: mm(2) });
  // @ts-expect-error Radius dimensions reject angular units.
  $.dimension.radius("angleRadius", { curve: hole.circle, target: { unit: "deg", value: 2 } });

  return $.outputs({ first, second, round, horizontal, vertical, symmetric, hole, holeRadius });
});
managedFilletSet;

async function managedCodeControlTypes(
  client: CodeControlClient,
  snapshot: CodeControlSnapshot,
  token: ManagedControlToken,
): Promise<void> {
  const inspected = await client.inspect();
  if (inspected.outcome === "success") {
    inspected.value.snapshot.manifest.controls[0]?.consumers;
  }
  await client.edit(snapshot, {
    edits: [{
      token,
      value: { kind: "unit", value: { unit: "mm", value: 2 } },
    }],
  });
  await client.undo(snapshot.identity);
  await client.redo(snapshot.identity);

  await client.edit(snapshot, {
    edits: [{
      token,
      // @ts-expect-error Managed-control replacements use the closed Rust tagged-value DTO.
      value: { kind: "quantity", value: 2 },
    }],
  });
}
managedCodeControlTypes;
