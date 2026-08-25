// SPDX-License-Identifier: GPL-3.0-or-later

import {
  createProject,
  fillets,
  line,
  point,
  polyline,
  rectangle,
  sketch,
} from "../src/index.js";
import type {
  CurveSpanRef,
  FeatureRecord,
  KeyedFeatureCollection,
  LineFeature,
  ManagedSketchProject,
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

const path = polyline(alpha, "path", ["lowerLeft", "upperRight", "tail"] as const);
const selected = {
  lowerLeft: path.filletableCorners.byKey.lowerLeft,
  upperRight: path.filletableCorners.byKey.upperRight,
};
const mapped = fillets(alpha, selected);
mapped.lowerLeft.arc;
mapped.upperRight.arc;

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
