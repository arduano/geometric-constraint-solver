// SPDX-License-Identifier: GPL-3.0-or-later

"use geosolve sketch";
import { sketch, mm } from "@geosolve/sketch-code";
import { mountingPlate } from "./patches/mounting-plate.patch.ts";

export default sketch(($) => {
  const plate = $.use("plate", mountingPlate, {
    width: mm(90),
    height: mm(55),
    cornerRadius: mm(7),
    holeRadius: mm(2.5),
  });
  $.group("Mounting plate", [
    plate.profile,
    plate.holes.nw,
    plate.holes.ne,
    plate.holes.se,
    plate.holes.sw,
  ]);
  return {
    plate,
    profile: plate.profile,
    nw: plate.holes.nw,
    ne: plate.holes.ne,
    se: plate.holes.se,
    sw: plate.holes.sw,
  };
});
