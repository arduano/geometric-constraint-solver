// SPDX-License-Identifier: GPL-3.0-or-later

import { definePatch, t } from "@geosolve/sketch-code";

/** Fixed named rectangle outputs make both brace diagonals type-safe. */
export const crossBrace = definePatch(
  { frame: t.feature("rectangle") },
  (p, { frame }) => ({
    diagonals: {
      rising: p.geometry.segment("rising", {
        start: frame.corners[0],
        end: frame.corners[2],
      }),
      falling: p.geometry.segment("falling", {
        start: frame.corners[3],
        end: frame.corners[1],
      }),
    },
  }),
);
