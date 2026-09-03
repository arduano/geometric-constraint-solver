// SPDX-License-Identifier: GPL-3.0-or-later

"use geosolve sketch";
import { sketch } from "@geosolve/sketch-code";

export default sketch(($) => {
  const first = $.geometry.quadraticBezier("first", {
    start: [0, 0],
    control: [1, 0],
    end: [2, 1],
  });
  const second = $.geometry.quadraticBezier("second", {
    start: [0, 0],
    control: [1, 0],
    end: [2, -1],
  });
  const tangent = $.constraint.curveCurveTangency("tangent", {
    first: first.span,
    second: second.span,
    contacts: {
      first: {
        parameter: 0,
        winding: 0,
        neighborhood: { kind: "start" },
        orientation: "aligned",
      },
      second: {
        parameter: 0,
        winding: 0,
        neighborhood: { kind: "start" },
        orientation: "aligned",
      },
    },
  });
  $.group("Compact curve tangency", [first.span, second.span, tangent.constraint]);
  return { first, second, tangent };
});
