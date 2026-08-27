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
      left: p.line(anchors.leftAbutment, anchors.leftPeak),
      crown: p.line(anchors.leftPeak, anchors.rightPeak),
      right: p.line(anchors.rightPeak, anchors.rightAbutment),
    },
    stays: {
      falling: p.line(anchors.leftPeak, anchors.rightBase),
      rising: p.line(anchors.leftBase, anchors.rightPeak),
    },
  }),
);
