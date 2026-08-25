// SPDX-License-Identifier: GPL-3.0-or-later

import { definePatch, t } from "@geosolve/sketch-code";

/** Fixed named rectangle outputs make both brace diagonals type-safe. */
export const crossBrace = definePatch(
  { frame: t.feature("rectangle") },
  (p, { frame }) => ({
    diagonals: {
      rising: p.line(frame.corners.lowerLeft, frame.corners.upperRight),
      falling: p.line(frame.corners.upperLeft, frame.corners.lowerRight),
    },
  }),
);
