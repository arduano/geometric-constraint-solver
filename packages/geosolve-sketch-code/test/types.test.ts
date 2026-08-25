// SPDX-License-Identifier: GPL-3.0-or-later

import {
  createProject,
  fillets,
  line,
  point,
  polyline,
  rectangle,
} from "../src/index.js";
import type {
  CurveSpanRef,
  FeatureRecord,
  KeyedFeatureCollection,
  OutputRef,
  RectangleFeature,
} from "../src/index.js";

const alpha = createProject("alpha");
const beta = createProject("beta");
const alphaStart = point(alpha, "start");
const alphaEnd = point(alpha, "end");
const betaEnd = point(beta, "end");
const segment = line(alpha, "segment", alphaStart, alphaEnd);

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
