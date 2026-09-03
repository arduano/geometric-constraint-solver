// SPDX-License-Identifier: GPL-3.0-or-later

"use geosolve sketch";
import { sketch, mm } from "@geosolve/sketch-code";

export default sketch(($) => {
  const first = $.geometry.segment("first", {
    start: [0, 0],
    end: [4, 0],
  });
  const second = $.geometry.segment("second", {
    start: first.end,
    end: [4, 4],
  });
  const third = $.geometry.segment("third", {
    start: second.end,
    end: [0, 4],
  });
  const round = $.computed.filletSet("round", {
    radius: mm(1),
    corners: [{
      key: "firstSecond",
      parents: [{
        span: first.span,
        parameter: 0.75,
        winding: 0,
        neighborhood: { kind: "interior" },
        normalSide: "left",
        trimEndpoint: "end",
        periodicAnchor: { kind: "none" },
      }, {
        span: second.span,
        parameter: 0.25,
        winding: 0,
        neighborhood: { kind: "interior" },
        normalSide: "left",
        trimEndpoint: "start",
        periodicAnchor: { kind: "none" },
      }],
      endpointOrder: "firstThenSecond",
      sweep: "counterClockwise",
    }, {
      key: "secondThird",
      parents: [{
        span: second.span,
        parameter: 0.75,
        winding: 0,
        neighborhood: { kind: "interior" },
        normalSide: "left",
        trimEndpoint: "end",
        periodicAnchor: { kind: "none" },
      }, {
        span: third.span,
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
  $.group("Direct Fillet", [first, second, third, round]);
  return { first, second, third, round };
});
