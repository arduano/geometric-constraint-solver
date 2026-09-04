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
  (p, { vertices, corners, clipRadius, bendRadius }) => ({
    clips: p.each(vertices, (vertex) =>
      p.geometry.centerRadiusCircle("clip", { center: vertex, radius: clipRadius })),
    fillets: p.each(corners, (corner) =>
      p.computed.fillet("fillet", { corner, radius: bendRadius })),
  }),
);
