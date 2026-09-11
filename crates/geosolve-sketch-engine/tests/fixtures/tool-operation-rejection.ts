// SPDX-License-Identifier: GPL-3.0-or-later
"use geosolve sketch";
import { sketch } from "@geosolve/sketch-code";
export default sketch(($) => {
  const skew = $.geometry.segment("skew", { start: [0, 0], end: [20, 10] });
  const fixedStart = $.constraint.fixedPoint("fixedStart", { point: skew.start, target: [0, 0] });
  const fixedEnd = $.constraint.fixedPoint("fixedEnd", { point: skew.end, target: [20, 10] });
  const good = $.geometry.segment("good", { start: [0, 30], end: [20, 30] });
  return { skew, fixedStart, fixedEnd, good };
});
