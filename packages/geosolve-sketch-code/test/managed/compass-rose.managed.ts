// SPDX-License-Identifier: GPL-3.0-or-later

"use geosolve managed-v1";
import { sketch, mm } from "@geosolve/sketch-code";
import { compassCore } from "./patches/compass-core.patch.ts";

export default sketch(($) => {
  const north = $.geometry.line("north", { start: [0, 0], end: [0, 34] });
  const east = $.geometry.line("east", { start: north.start, end: [34, 0] });
  const south = $.geometry.line("south", { start: north.start, end: [0, -34] });
  const west = $.geometry.line("west", { start: north.start, end: [-34, 0] });
  const core = $.use("core", compassCore, {
    north: north.end,
    east: east.end,
    south: south.end,
    west: west.end,
    markerRadius: mm(3),
  });
  const northAxis = $.constraint.vertical("northAxis", { curve: north.span });
  const eastAxis = $.constraint.horizontal("eastAxis", { curve: east.span });
  const southAxis = $.constraint.vertical("southAxis", { curve: south.span });
  const westAxis = $.constraint.horizontal("westAxis", { curve: west.span });
  $.organize("Composable compass rose", [north, east, south, west, core, northAxis, eastAxis, southAxis, westAxis]);
  return $.outputs({
    north,
    east,
    south,
    west,
    core,
    northMarker: core.markers.north.circle,
    eastMarker: core.markers.east.circle,
    southMarker: core.markers.south.circle,
    westMarker: core.markers.west.circle,
    northEast: core.ring.northEast.span,
    northAxis,
    eastAxis,
    southAxis,
    westAxis,
  });
});
