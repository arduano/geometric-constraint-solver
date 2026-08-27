// SPDX-License-Identifier: GPL-3.0-or-later

"use geosolve managed-v1";
import { sketch, mm } from "@geosolve/sketch-code";

export default sketch(($) => {
  const feed = $.geometry.line("feed", {
    start: [-54, -20],
    end: [-20, -20],
  });
  const rise = $.geometry.line("rise", {
    start: feed.end,
    end: [-20, 12],
  });
  const bridge = $.geometry.line("bridge", {
    start: rise.end,
    end: [20, 12],
  });
  const stack = $.geometry.line("stack", {
    start: bridge.end,
    end: [20, 44],
  });
  const feedAxis = $.constraint.horizontal("feedAxis", { curve: feed.span });
  const riseAxis = $.constraint.vertical("riseAxis", { curve: rise.span });
  const bridgeAxis = $.constraint.horizontal("bridgeAxis", { curve: bridge.span });
  const stackAxis = $.constraint.vertical("stackAxis", { curve: stack.span });
  const bends = $.computed.filletSet("bends", {
    radius: mm(5),
    corners: [{
      parents: [{
        span: feed.span,
        parameter: 0.75,
        winding: 0,
        neighborhood: { kind: "interior" },
        normalSide: "left",
        retainedEndpoint: "end",
        periodicAnchor: null,
      }, {
        span: rise.span,
        parameter: 0.25,
        winding: 0,
        neighborhood: { kind: "interior" },
        normalSide: "left",
        retainedEndpoint: "start",
        periodicAnchor: null,
      }],
      endpointOrder: "firstThenSecond",
      sweep: "counterClockwise",
    }, {
      parents: [{
        span: bridge.span,
        parameter: 0.75,
        winding: 0,
        neighborhood: { kind: "interior" },
        normalSide: "left",
        retainedEndpoint: "end",
        periodicAnchor: null,
      }, {
        span: stack.span,
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
  $.organize("Neon manifold", [feed, rise, bridge, stack, bends, feedAxis, riseAxis, bridgeAxis, stackAxis]);
  return $.outputs({ feed, rise, bridge, stack, bends, feedAxis, riseAxis, bridgeAxis, stackAxis });
});
