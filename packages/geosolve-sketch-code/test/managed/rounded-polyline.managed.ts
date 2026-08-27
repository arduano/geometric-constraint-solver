// SPDX-License-Identifier: GPL-3.0-or-later

"use geosolve managed-v1";
import { sketch, mm } from "@geosolve/sketch-code";
import { roundEveryCorner } from "./patches/round-every-corner.patch.ts";

export default sketch(($) => {
  const path = $.geometry.polyline("path", {
    vertices: [
      { key: "start", position: [0, 0] },
      { key: "rise", position: [20, 0] },
      { key: "shoulder", position: [24, 12] },
      { key: "ridge", position: [40, 18] },
      { key: "fall", position: [55, 10] },
      { key: "end", position: [65, 10] },
    ],
    closed: false,
  });
  const rounded = $.use("rounded", roundEveryCorner, {
    corners: path.filletableCorners,
    radius: mm(4),
  });
  $.organize("Adaptive profile", [path, rounded]);
  return $.outputs({ path, rounded });
});
