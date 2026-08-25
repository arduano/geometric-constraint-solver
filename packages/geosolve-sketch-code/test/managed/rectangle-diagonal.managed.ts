// SPDX-License-Identifier: GPL-3.0-or-later

"use geosolve managed-v1";
import { sketch } from "@geosolve/sketch-code";

export default sketch(($) => {
  const frame = $.geometry.rectangle("frame", {
    lowerLeft: [0, 0],
    upperRight: [60, 35],
  });
  const diagonal = $.geometry.line("diagonal", {
    start: frame.corners.lowerLeft,
    end: frame.corners.upperRight,
  });
  $.organize("Frame", [frame, diagonal]);
  return $.outputs({ frame, diagonal });
});
