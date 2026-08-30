// SPDX-License-Identifier: GPL-3.0-or-later

import { definePatch, t } from "@geosolve/sketch-code";

/** Preserve caller-owned center keys while adding shared-radius corner reliefs. */
export const cornerReliefs = definePatch(
  { centers: t.record(t.point()), radius: t.length() },
  (p, { centers, radius }) => {
    p.editLens({
      output: ["reliefs"],
      invocationArgument: ["radius"],
      expectedKind: "scalar",
    });
    return {
      reliefs: p.mapRecord(centers, (center) => {
        const circle = p.circle(center, radius).circle;
        p.radius(circle, radius);
        return circle;
      }),
    };
  },
);
