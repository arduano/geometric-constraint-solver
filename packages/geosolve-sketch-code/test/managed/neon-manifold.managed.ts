// SPDX-License-Identifier: GPL-3.0-or-later

"use geosolve sketch";
import { sketch, mm } from "@geosolve/sketch-code";

export default sketch(($) => {
  const feed = $.geometry.segment("feed", {
    start: [-54, -20],
    end: [-20, -20],
  });
  const rise = $.geometry.segment("rise", {
    start: feed.end,
    end: [-20, 12],
  });
  const bridge = $.geometry.segment("bridge", {
    start: rise.end,
    end: [20, 12],
  });
  const stack = $.geometry.segment("stack", {
    start: bridge.end,
    end: [20, 44],
  });
  const feedAxis = $.constraint.horizontal("feedAxis", { span: feed.span });
  const riseAxis = $.constraint.vertical("riseAxis", { span: rise.span });
  const bridgeAxis = $.constraint.horizontal("bridgeAxis", { span: bridge.span });
  const stackAxis = $.constraint.vertical("stackAxis", { span: stack.span });
  const bends = $.computed.filletSet("bends", {
    radius: mm(5),
    corners: [{
      key: "feedRise",
      parents: [{
        span: feed.span,
        parameter: 0.75,
        winding: 0,
        neighborhood: { kind: "interior" },
        normalSide: "left",
        trimEndpoint: "end",
        periodicAnchor: { kind: "none" },
      }, {
        span: rise.span,
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
      key: "bridgeStack",
      parents: [{
        span: bridge.span,
        parameter: 0.75,
        winding: 0,
        neighborhood: { kind: "interior" },
        normalSide: "left",
        trimEndpoint: "end",
        periodicAnchor: { kind: "none" },
      }, {
        span: stack.span,
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
  $.group("Neon manifold", [feed, rise, bridge, stack, bends, feedAxis, riseAxis, bridgeAxis, stackAxis]);
  return { feed, rise, bridge, stack, bends, feedAxis, riseAxis, bridgeAxis, stackAxis };
});
