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
  (p, { vertices, corners, bulbRadius, bendRadius }) => ({
    bulbs: p.each(vertices, (vertex) =>
      p.geometry.centerRadiusCircle("bulb", { center: vertex, radius: bulbRadius })),
    fillets: p.each(corners, (corner) =>
      p.computed.fillet("fillet", { corner, radius: bendRadius })),
  }),
);
