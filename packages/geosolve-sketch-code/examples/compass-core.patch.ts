// SPDX-License-Identifier: GPL-3.0-or-later

import { definePatch, t } from "@geosolve/sketch-code";

/** A generated compass ring and markers driven by four editable native spokes. */
export const compassCore = definePatch(
  {
    north: t.point(),
    east: t.point(),
    south: t.point(),
    west: t.point(),
    markerRadius: t.length(),
  },
  (p, input) => {
    p.editLens({
      output: ["markers"],
      invocationArgument: ["markerRadius"],
      expectedKind: "scalar",
    });
    return {
      ring: {
        northEast: p.line(input.north, input.east),
        southEast: p.line(input.east, input.south),
        southWest: p.line(input.south, input.west),
        northWest: p.line(input.west, input.north),
      },
      markers: {
        north: p.circle(input.north, input.markerRadius),
        east: p.circle(input.east, input.markerRadius),
        south: p.circle(input.south, input.markerRadius),
        west: p.circle(input.west, input.markerRadius),
      },
    };
  },
);
