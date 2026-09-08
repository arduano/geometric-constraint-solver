// SPDX-License-Identifier: GPL-3.0-or-later
"use geosolve sketch";
import { mm, sketch } from "@geosolve/sketch-code";

export default sketch(($) => {
  // Edit the driving radius below, or select it in the existing Inspector.
  const ring = $.geometry.centerRadiusCircle("ring", {
    center: [0, 0],
    radius: mm(10),
  });
  const ringRadius = $.dimension.radius("ringRadius", {
    curve: ring.curve,
    value: mm(10),
  });
  return { ring, ringRadius };
});
