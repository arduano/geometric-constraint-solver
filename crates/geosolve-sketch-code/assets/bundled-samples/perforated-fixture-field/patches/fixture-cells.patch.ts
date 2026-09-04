// SPDX-License-Identifier: GPL-3.0-or-later

import { definePatch, t } from "@geosolve/sketch-code";

/** Generate one exact pilot and counterbore Circle at every keyed fixture centre. */
export const fixtureCells = definePatch(
  {
    centers: t.keyed(t.point()),
    pilotRadius: t.length(),
    counterboreRadius: t.length(),
  },
  (p, { centers, pilotRadius, counterboreRadius }) => ({
    pilots: p.each(centers, (center) =>
      p.geometry.centerRadiusCircle("pilot", { center, radius: pilotRadius })),
    counterbores: p.each(centers, (center) =>
      p.geometry.centerRadiusCircle("counterbore", { center, radius: counterboreRadius })),
  }),
);
