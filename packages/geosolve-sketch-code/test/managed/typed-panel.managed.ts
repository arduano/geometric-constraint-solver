// SPDX-License-Identifier: GPL-3.0-or-later

"use geosolve managed-v1";
import { sketch, mm } from "@geosolve/sketch-code";
import { fillets } from "./patches/fillet-record.patch.ts";

export default sketch(($) => {
  const panel = $.geometry.rectangle("panel", {
    lowerLeft: [0, 0],
    upperRight: [80, 40],
  });
  const corners = $.use("cornerFillets", fillets, {
    corners: {
      lowerLeft: panel.corners.lowerLeft,
      upperRight: panel.corners.upperRight,
    },
    radius: mm(4),
  });
  return $.outputs({
    panel,
    lowerLeft: corners.fillets.lowerLeft,
    upperRight: corners.fillets.upperRight,
  });
});
