// SPDX-License-Identifier: GPL-3.0-or-later

import { definePatch, t } from "@geosolve/sketch-code";

/** Decorate every current harness vertex with a clip and round every current bend. */
export const harnessRoute = definePatch(
  {
    vertices: t.keyed(t.point()),
    corners: t.keyed(t.corner()),
    clipRadius: t.length(),
    bendRadius: t.length(),
  },
  (p, { vertices, corners, clipRadius, bendRadius }) => {
    p.editLens({
      output: ["clips"],
      invocationArgument: ["clipRadius"],
      expectedKind: "scalar",
    });
    p.editLens({
      output: ["fillets"],
      invocationArgument: ["bendRadius"],
      expectedKind: "scalar",
    });
    return {
      clips: p.each(
        vertices,
        (vertex) => p.circle(vertex, clipRadius),
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
