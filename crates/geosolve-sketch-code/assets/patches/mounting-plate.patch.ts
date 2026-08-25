// SPDX-License-Identifier: GPL-3.0-or-later

import { definePatch, t } from "@geosolve/sketch-code";

const compass = ["nw", "ne", "se", "sw"] as const;

/** Rounded profile plus keyed holes; native Rust still owns all geometry. */
export const mountingPlate = definePatch(
  {
    width: t.length(),
    height: t.length(),
    cornerRadius: t.length(),
    holeRadius: t.length(),
  },
  (p, input) => {
    const rounded = p.roundedRectangle(input.width, input.height, input.cornerRadius);
    const holes = p.record(compass, (key) =>
      p.circle(rounded.mounts[key], input.holeRadius));
    return { profile: rounded.profile, holes };
  },
);
