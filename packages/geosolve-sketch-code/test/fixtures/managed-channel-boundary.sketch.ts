// SPDX-License-Identifier: GPL-3.0-or-later
"use geosolve sketch";
import { sketch, mm } from "@geosolve/sketch-code";
import { waterChannel } from "./patches/water-channel.patch.ts";

export default sketch(($) => {
  const route = $.geometry.polyline("route", {
    vertices: [
      { key: "start", position: [0, 0] },
      { key: "east", position: [30, 0] },
      { key: "north", position: [30, 30] },
      { key: "west", position: [5, 30] },
      { key: "end", position: [5, 10] },
    ],
    closed: false,
  });
  const channel = $.use("channel", waterChannel, {
    polyline: route,
    width: mm(6),
    bendRadius: mm(5),
  });
  return { route, channel };
});
