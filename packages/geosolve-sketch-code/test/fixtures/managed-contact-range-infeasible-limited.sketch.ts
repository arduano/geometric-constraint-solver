// SPDX-License-Identifier: GPL-3.0-or-later

"use geosolve sketch";

import { sketch } from "@geosolve/sketch-code";

export default sketch(($) => {
  const line = $.geometry.segment("line", {
    start: [0, 0],
    end: [10, 0],
  });
  const contactPoint = $.geometry.sketchPoint("contactPoint", {
    point: [8, 0],
  });
  const unrelated = $.geometry.sketchPoint("unrelated", {
    point: [17, -9],
  });
  const fixedStart = $.constraint.fixedPoint("fixedStart", {
    point: line.start,
    target: [0, 0],
  });
  const fixedEnd = $.constraint.fixedPoint("fixedEnd", {
    point: line.end,
    target: [10, 0],
  });
  const fixedContactPoint = $.constraint.fixedPoint("fixedContactPoint", {
    point: contactPoint.point,
    target: [8, 0],
  });
  const contact = $.constraint.pointOnCurve("contact", {
    point: contactPoint.point,
    curve: line.span,
    contact: {
      parameter: 0.8,
      winding: 0,
      range: { lower: 0, upper: 0.5 },
      neighborhood: { kind: "interior" },
      orientation: "none",
    },
  });
  $.group("Infeasible contact range", [
    line,
    contactPoint,
    unrelated,
    fixedStart,
    fixedEnd,
    fixedContactPoint,
    contact,
  ]);
  return { line, contactPoint, unrelated, contact };
});
