// SPDX-License-Identifier: GPL-3.0-or-later

"use geosolve sketch";
import { sketch } from "@geosolve/sketch-code";

export default sketch(($) => {
  const frame = $.geometry.segment("frame", {
    start: [0, 0],
    end: [20, 0],
  });
  const cubic = $.geometry.cubicBezier("cubic", {
    start: [0, 0],
    firstControl: [2, 4],
    secondControl: [5, 4],
    end: [7, 0],
  });
  const tangent = $.geometry.tangentArc("tangent", {
    center: [20, 1],
    start: [20, 0],
    end: [21, 1],
    source: {
      span: frame.span,
      contact: {
        parameter: 1,
        winding: 0,
        neighborhood: { kind: "end" },
        orientation: "aligned",
      },
    },
    orientation: "aligned",
    sweep: "counterClockwise",
  });
  const periodic = $.geometry.periodicControlNurbs("periodic", {
    controls: [
      { key: "c0", position: [0, 0], weight: 1 },
      { key: "c1", position: [2, 2], weight: 1 },
      { key: "c2", position: [4, 0], weight: 1 },
      { key: "c3", position: [2, -2], weight: 1 },
    ],
    degree: 2,
    gauge: "c0",
  });
  $.group("Compact geometry controls", [cubic, tangent, periodic]);
  return { frame, cubic, tangent, periodic };
});
