// SPDX-License-Identifier: GPL-3.0-or-later

"use geosolve managed-v1";
import { sketch, mm } from "@geosolve/sketch-code";
import { adaptiveLanterns } from "./patches/adaptive-lanterns.patch.ts";

export default sketch(($) => {
  const wire = $.geometry.polyline("wire", {
    vertices: [
      { key: "plug", position: [-55, 4] },
      { key: "amber", position: [-38, 16] },
      { key: "coral", position: [-20, 7] },
      { key: "gold", position: [0, 18] },
      { key: "mint", position: [21, 8] },
      { key: "violet", position: [39, 16] },
      { key: "tail", position: [56, 4] },
    ],
    closed: false,
  });
  const decorations = $.use("decorations", adaptiveLanterns, {
    vertices: wire.vertices,
    corners: wire.filletableCorners,
    bulbRadius: mm(2.6),
    bendRadius: mm(3.2),
  });
  $.organize("Adaptive lantern garland", [wire, decorations]);
  return $.outputs({ wire, bulbs: decorations.bulbs, fillets: decorations.fillets });
});
