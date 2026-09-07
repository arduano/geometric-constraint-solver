// SPDX-License-Identifier: GPL-3.0-or-later
"use geosolve sketch";
import { sketch, mm } from "@geosolve/sketch-code";
import { waterChannel } from "./patches/water-channel.patch.ts";

export default sketch(($) => {
  const route = $.geometry.polyline("route", {
    vertices: [
      { key: "start", position: [0, 0] },
      { key: "bend", position: [30, 0] },
      { key: "end", position: [30, 30] },
    ],
    closed: false,
    branchDirections: [[1, 0], [0, 1]],
    role: "construction",
  });
  const origin = $.constraint.fixedPoint("origin", {
    point: route.vertices.byKey.start,
    target: [0, 0],
  });
  const firstAxis = $.constraint.horizontal("firstAxis", { span: route.segments.byKey.start });
  const secondAxis = $.constraint.vertical("secondAxis", { span: route.segments.byKey.bend });
  const firstLength = $.dimension.curveLength("firstLength", { curve: route.segments.byKey.start, value: mm(30) });
  const secondLength = $.dimension.curveLength("secondLength", { curve: route.segments.byKey.bend, value: mm(30) });
  const channel = $.use("channel", waterChannel, {
    polyline: route,
    width: mm(6),
    bendRadius: mm(5),
  });
  return { route, origin, firstAxis, secondAxis, firstLength, secondLength, channel };
});
