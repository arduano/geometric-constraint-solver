// SPDX-License-Identifier: GPL-3.0-or-later

"use geosolve managed-v1";
import { sketch } from "@geosolve/sketch-code";

export default sketch(($) => {
  const line = $.geometry.line("line", {
    start: [0, 0],
    end: [4, 0],
  });
  const line2 = $.geometry.line("line2", {
    start: line.end,
    end: [4, 4],
  });
  const fillet = $.computed.filletSet("fillet", {
    radius: 1,
    corners: [{
      parents: [{
        span: line.span,
        parameter: 0.75,
        winding: 0,
        neighborhood: { kind: "interior" },
        normalSide: "left",
        retainedEndpoint: "end",
        periodicAnchor: null,
      }, {
        span: line2.span,
        parameter: 0.25,
        winding: 0,
        neighborhood: { kind: "interior" },
        normalSide: "left",
        retainedEndpoint: "start",
        periodicAnchor: null,
      }],
      endpointOrder: "firstThenSecond",
      sweep: "counterClockwise",
    }],
    suppressed: false,
  });
  return $.outputs({ line, line2, fillet });
});
