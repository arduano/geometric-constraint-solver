// SPDX-License-Identifier: GPL-3.0-or-later

"use geosolve sketch";
import { sketch, mm } from "@geosolve/sketch-code";
import { fillets } from "./patches/fillet-record.patch.ts";

export default sketch(($) => {
  const panel = $.geometry.polyline("panel", {
    vertices: [
      { key: "lowerLeft", position: [0, 0] },
      { key: "lowerRight", position: [80, 0] },
      { key: "upperRight", position: [80, 40] },
      { key: "upperLeft", position: [0, 40] },
    ],
    closed: true,
  });
  const corners = $.use("cornerFillets", fillets, {
    corners: {
      lowerLeft: panel.filletableCorners.byKey.lowerLeft,
      upperRight: panel.filletableCorners.byKey.upperRight,
    },
    radius: mm(4),
  });
  return {
    panel,
    lowerLeft: corners.fillets.lowerLeft,
    upperRight: corners.fillets.upperRight,
  };
});
