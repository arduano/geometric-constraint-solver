// SPDX-License-Identifier: GPL-3.0-or-later

"use geosolve sketch";
import { sketch } from "@geosolve/sketch-code";

export default sketch(($) => {
  const frame = $.geometry.twoPointAlignedRectangle("frame", {
    firstCorner: [0, 0],
    oppositeCorner: [60, 35],
  });
  const diagonal = $.geometry.segment("diagonal", {
    start: frame.corners[0],
    end: frame.corners[2],
  });
  $.group("Frame", [frame, diagonal]);
  return { frame, diagonal };
});
