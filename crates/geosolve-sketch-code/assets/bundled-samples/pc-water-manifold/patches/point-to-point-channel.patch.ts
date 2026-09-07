// SPDX-License-Identifier: GPL-3.0-or-later

import { definePatch, t } from "@geosolve/sketch-code";

/** A finite-width passage between two ports, with tangent bends and two rounded ends. */
export const pointToPointChannel = definePatch(
  { polyline: t.feature("polyline"), width: t.length(), bendRadius: t.length() },
  (p, { polyline, width, bendRadius }) => ({
    profile: p.computed.polylineChannel("channel", {
      polyline, width, bendRadius, caps: "both",
    }),
  }),
);
