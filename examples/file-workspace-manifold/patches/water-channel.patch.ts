// SPDX-License-Identifier: GPL-3.0-or-later

import { definePatch, t } from "@geosolve/sketch-code";

/**
 * A finite-width water passage associated with an editable keyed polyline.
 * Two native offsets form the walls, tangent fillets round the corners, and a
 * semicircular outlet cap closes the end. The inlet stays open to a reservoir.
 */
export const waterChannel = definePatch(
  {
    polyline: t.feature("polyline"),
    width: t.length({ label: "Channel width", description: "Full width across the water passage.", isKeyParameter: true }),
    bendRadius: t.length({ label: "Bend radius", description: "Centreline radius; must exceed half the width." }),
  },
  (p, { polyline, width, bendRadius }) => ({
    profile: p.computed.polylineChannel("channel", {
      polyline, width, bendRadius, caps: "end",
    }),
  }),
);
