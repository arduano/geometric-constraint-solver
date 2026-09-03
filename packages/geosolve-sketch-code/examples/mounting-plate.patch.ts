// SPDX-License-Identifier: GPL-3.0-or-later

import { definePatch, t } from "@geosolve/sketch-code";

/** Rounded profile plus keyed holes; native Rust still owns all geometry. */
export const mountingPlate = definePatch(
  {
    width: t.length(),
    height: t.length(),
    cornerRadius: t.length(),
    holeRadius: t.length(),
  },
  (p, input) => {
    const rounded = p.computed.roundedRectangleProfile("profile", {
      width: input.width,
      height: input.height,
      cornerRadius: input.cornerRadius,
    });
    const holes = {
      nw: p.geometry.centerRadiusCircle("hole-nw", {
        center: rounded.mounts.nw,
        radius: input.holeRadius,
      }).curve,
      ne: p.geometry.centerRadiusCircle("hole-ne", {
        center: rounded.mounts.ne,
        radius: input.holeRadius,
      }).curve,
      se: p.geometry.centerRadiusCircle("hole-se", {
        center: rounded.mounts.se,
        radius: input.holeRadius,
      }).curve,
      sw: p.geometry.centerRadiusCircle("hole-sw", {
        center: rounded.mounts.sw,
        radius: input.holeRadius,
      }).curve,
    };
    return { profile: rounded.profile, holes };
  },
);
