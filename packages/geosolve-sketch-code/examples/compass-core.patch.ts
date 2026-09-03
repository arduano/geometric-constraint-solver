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
  (p, input) => ({
    ring: {
      northEast: p.geometry.segment("northEast", { start: input.north, end: input.east }),
      southEast: p.geometry.segment("southEast", { start: input.east, end: input.south }),
      southWest: p.geometry.segment("southWest", { start: input.south, end: input.west }),
      northWest: p.geometry.segment("northWest", { start: input.west, end: input.north }),
    },
    markers: {
      north: p.geometry.centerRadiusCircle("northMarker", {
        center: input.north, radius: input.markerRadius,
      }),
      east: p.geometry.centerRadiusCircle("eastMarker", {
        center: input.east, radius: input.markerRadius,
      }),
      south: p.geometry.centerRadiusCircle("southMarker", {
        center: input.south, radius: input.markerRadius,
      }),
      west: p.geometry.centerRadiusCircle("westMarker", {
        center: input.west, radius: input.markerRadius,
      }),
    },
  }),
);
