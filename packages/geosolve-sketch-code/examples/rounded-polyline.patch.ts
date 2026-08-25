// SPDX-License-Identifier: GPL-3.0-or-later

import { definePatch, t } from "@geosolve/sketch-code";

/** Structurally Fillet every keyed, currently filletable Polyline corner. */
export const roundEveryCorner = definePatch(
  { corners: t.keyed(t.corner()), radius: t.length() },
  (p, { corners, radius }) => {
    p.editLens({
      output: ["fillets"],
      invocationArgument: ["radius"],
      expectedKind: "scalar",
    });
    return {
      fillets: p.each(
        corners,
        (corner) => p.fillet({ corner, radius }),
        { key: (corner) => corner.key },
      ),
    };
  },
);
