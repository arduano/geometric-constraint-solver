// SPDX-License-Identifier: GPL-3.0-or-later

import { definePatch, t } from "@geosolve/sketch-code";

/** Decorate every current path vertex and round every current interior corner. */
export const adaptiveLanterns = definePatch(
  {
    vertices: t.keyed(t.point()),
    corners: t.keyed(t.corner()),
    bulbRadius: t.length(),
    bendRadius: t.length(),
  },
  (p, { vertices, corners, bulbRadius, bendRadius }) => {
    p.editLens({
      output: ["bulbs"],
      invocationArgument: ["bulbRadius"],
      expectedKind: "scalar",
    });
    p.editLens({
      output: ["fillets"],
      invocationArgument: ["bendRadius"],
      expectedKind: "scalar",
    });
    return {
      bulbs: p.each(
        vertices,
        (vertex) => p.circle(vertex, bulbRadius),
        { key: (vertex) => vertex.key },
      ),
      fillets: p.each(
        corners,
        (corner) => p.fillet({ corner, radius: bendRadius }),
        { key: (corner) => corner.key },
      ),
    };
  },
);
