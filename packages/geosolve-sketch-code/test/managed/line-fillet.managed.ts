// SPDX-License-Identifier: GPL-3.0-or-later

"use geosolve sketch";
import { sketch, mm } from "@geosolve/sketch-code";

export default sketch(($) => {
  const line = $.geometry.segment("line", {
    start: [0, 0],
    end: [4, 0],
  });
  const horizontal = $.constraint.horizontal("horizontal", {
    span: line.span,
    suppressed: false,
  });
  const line2 = $.geometry.segment("line2", {
    start: line.end,
    end: [4, 4],
  });
  const vertical = $.constraint.vertical("vertical", {
    span: line2.span,
    suppressed: false,
  });
  const fillet = $.computed.filletSet("fillet", {
    radius: mm(1),
    corners: [{
      key: "corner",
      parents: [{
        span: line.span,
        parameter: 0.75,
        winding: 0,
        neighborhood: { kind: "interior" },
        normalSide: "left",
        trimEndpoint: "end",
        periodicAnchor: { kind: "none" },
      }, {
        span: line2.span,
        parameter: 0.25,
        winding: 0,
        neighborhood: { kind: "interior" },
        normalSide: "left",
        trimEndpoint: "start",
        periodicAnchor: { kind: "none" },
      }],
      endpointOrder: "firstThenSecond",
      sweep: "counterClockwise",
    }],
  });
  return { line, line2, fillet, horizontal, vertical };
});
