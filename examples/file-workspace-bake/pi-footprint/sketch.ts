// SPDX-License-Identifier: GPL-3.0-or-later
"use geosolve sketch";
import { mm, sketch } from "@geosolve/sketch-code";

// Nominal reference footprint only. No enclosure or hardware-fit claim.
export default sketch(($) => {
  const board = $.operation.rectangle("board", {
    origin: [0, 0], width: mm(85), height: mm(56), role: "profile",
  });
  const bottomLeft = $.geometry.centerRadiusCircle("bottomLeft", {
    center: [3.5, 3.5], radius: mm(1.35),
  });
  const bottomRight = $.geometry.centerRadiusCircle("bottomRight", {
    center: [61.5, 3.5], radius: mm(1.35),
  });
  const topLeft = $.geometry.centerRadiusCircle("topLeft", {
    center: [3.5, 52.5], radius: mm(1.35),
  });
  const topRight = $.geometry.centerRadiusCircle("topRight", {
    center: [61.5, 52.5], radius: mm(1.35),
  });
  return { board, bottomLeft, bottomRight, topLeft, topRight };
});
