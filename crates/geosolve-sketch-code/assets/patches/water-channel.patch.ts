// SPDX-License-Identifier: GPL-3.0-or-later

import { definePatch, t } from "@geosolve/sketch-code";

/**
 * AI-authored manifold primitive: round every current corner of one editable
 * channel centreline or independently constrained O-ring groove.
 *
 * The managed file owns coordinates and mechanical constraints. This custom
 * patch owns the adaptive structural rule, so inserting or deleting keyed
 * Polyline vertices automatically changes the corresponding bend set.
 */
export const waterChannel = definePatch(
  {
    corners: t.keyed(t.corner()),
    bendRadius: t.length(),
  },
  (p, { corners, bendRadius }) => {
    p.editLens({
      output: ["bends"],
      invocationArgument: ["bendRadius"],
      expectedKind: "scalar",
    });
    return {
      bends: p.each(
        corners,
        (corner) => p.fillet({ corner, radius: bendRadius }),
        { key: (corner) => corner.key },
      ),
    };
  },
);
