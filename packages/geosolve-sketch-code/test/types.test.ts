// SPDX-License-Identifier: GPL-3.0-or-later

import {
  createProject,
  definePatch,
  fillets,
  line,
  point,
  polyline,
  rectangle,
  sketch,
  t,
} from "../src/index.js";
import type {
  CurveSpanRef,
  FeatureRecord,
  KeyedFeatureCollection,
  LineFeature,
  ManagedSketchProject,
  NativeCurveSpanRef,
  OutputRef,
  RectangleFeature,
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

  return $.outputs({ first, second, round });
});
managedFilletSet;
