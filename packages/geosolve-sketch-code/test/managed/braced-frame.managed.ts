// SPDX-License-Identifier: GPL-3.0-or-later

"use geosolve managed-v1";
import { sketch } from "@geosolve/sketch-code";
import { crossBrace } from "./patches/cross-brace.patch.ts";

export default sketch(($) => {
  const frame = $.geometry.rectangle("frame", {
    lowerLeft: [0, 0],
    upperRight: [60, 35],
  });
  const brace = $.use("brace", crossBrace, { frame: frame });
  // A diagonal of an axis-aligned frame cannot itself be horizontal. Keep the
  // downstream ordinary relation as an explicit, editable suppressed example
  // rather than publishing an invalid demonstration scene.
  const datum = $.constraint.horizontal("datum", {
    curve: brace.diagonals.rising,
    suppressed: true,
  });
  $.organize("Frame", [frame, brace, datum]);
  return $.outputs({ frame, brace, rising: brace.diagonals.rising, datum });
});
