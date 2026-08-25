// SPDX-License-Identifier: GPL-3.0-or-later

"use geosolve managed-v1";
import { sketch, mm } from "@geosolve/sketch-code";
import { mountingPlate } from "./patches/mounting-plate.patch.ts";

export default sketch(($) => {
  const plate = $.use("plate", mountingPlate, {
    width: mm(90),
    height: mm(55),
    cornerRadius: mm(7),
    holeRadius: mm(2.5),
  });
  $.organize("Mounting plate", [plate]);
  return $.outputs({
    plate,
    profile: plate.profile,
    nw: plate.holes.nw,
    ne: plate.holes.ne,
    se: plate.holes.se,
    sw: plate.holes.sw,
  });
});
