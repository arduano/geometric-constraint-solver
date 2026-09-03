// SPDX-License-Identifier: GPL-3.0-or-later

import { definePatch, t } from "@geosolve/sketch-code";

/** A reusable cable-and-stay layer over typed bridge anchor points. */
export const bridgeCables = definePatch(
  {
    leftAbutment: t.point(),
    leftBase: t.point(),
    leftPeak: t.point(),
    rightBase: t.point(),
    rightPeak: t.point(),
    rightAbutment: t.point(),
  },
  (p, anchors) => ({
    mainCable: {
      left: p.geometry.segment("left", {
        start: anchors.leftAbutment,
        end: anchors.leftPeak,
      }),
      crown: p.geometry.segment("crown", {
        start: anchors.leftPeak,
        end: anchors.rightPeak,
      }),
      right: p.geometry.segment("right", {
        start: anchors.rightPeak,
        end: anchors.rightAbutment,
      }),
    },
    stays: {
      falling: p.geometry.segment("falling", {
        start: anchors.leftPeak,
        end: anchors.rightBase,
      }),
      rising: p.geometry.segment("rising", {
        start: anchors.leftBase,
        end: anchors.rightPeak,
      }),
    },
  }),
);
