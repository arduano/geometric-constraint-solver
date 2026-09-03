// SPDX-License-Identifier: GPL-3.0-or-later

import { definePatch, t } from "@geosolve/sketch-code";

/** Preserve caller-owned center keys while adding shared-radius corner reliefs. */
export const cornerReliefs = definePatch(
  { centers: t.record(t.point()), radius: t.length() },
  (p, { centers, radius }) => ({
    reliefs: p.mapRecord(centers, (center) => {
      const circle = p.geometry.centerRadiusCircle("relief", { center, radius });
      p.dimension.radius("radius", { curve: circle.curve, value: radius });
      return circle.curve;
    }),
  }),
);
