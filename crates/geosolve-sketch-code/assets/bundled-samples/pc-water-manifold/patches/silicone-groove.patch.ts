// SPDX-License-Identifier: GPL-3.0-or-later

import { definePatch, t } from "@geosolve/sketch-code";

/**
 * A finite-width silicone seal groove around a closed editable keyed polyline.
 * Two native offsets and tangent corner fillets form its inner and outer walls.
 */
export const siliconeGroove = definePatch(
  {
    polyline: t.feature("polyline"),
    width: t.length(),
    bendRadius: t.length(),
  },
  (p, { polyline, width, bendRadius }) => ({
    profile: p.computed.polylineChannel("groove", {
      polyline, width, bendRadius, caps: "none",
    }),
  }),
);
