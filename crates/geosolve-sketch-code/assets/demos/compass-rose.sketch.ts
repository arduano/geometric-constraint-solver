"use geosolve sketch";
import { sketch, mm } from "@geosolve/sketch-code";
import { compassCore } from "./patches/compass-core.patch.ts";

export default sketch(($) => {
  const north = $.geometry.segment("north", {
    start: [0, 0],
    end: [0, 34],
  });
  const east = $.geometry.segment("east", {
    start: north.start,
    end: [34, 0],
  });
  const south = $.geometry.segment("south", {
    start: north.start,
    end: [0, -34],
  });
  const west = $.geometry.segment("west", {
    start: north.start,
    end: [-34, 0],
  });
  const core = $.use("core", compassCore, {
    north: north.end,
    east: east.end,
    south: south.end,
    west: west.end,
    markerRadius: mm(3),
  });
  const northAxis = $.constraint.vertical("northAxis", {
    span: north.span,
  });
  const eastAxis = $.constraint.horizontal("eastAxis", {
    span: east.span,
  });
  const southAxis = $.constraint.vertical("southAxis", {
    span: south.span,
  });
  const westAxis = $.constraint.horizontal("westAxis", {
    span: west.span,
  });
  $.group("Composable compass rose", [north, east, south, west, northAxis, eastAxis, southAxis, westAxis]);
  return {
    north: north,
    east: east,
    south: south,
    west: west,
    core: core,
    northMarker: core.markers.north.curve,
    eastMarker: core.markers.east.curve,
    southMarker: core.markers.south.curve,
    westMarker: core.markers.west.curve,
    northEast: core.ring.northEast.span,
    northAxis: northAxis,
    eastAxis: eastAxis,
    southAxis: southAxis,
    westAxis: westAxis,
  };
});
