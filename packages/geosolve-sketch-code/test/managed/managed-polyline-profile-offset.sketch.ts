// SPDX-License-Identifier: GPL-3.0-or-later
"use geosolve sketch";
import { sketch, mm } from "@geosolve/sketch-code";

export default sketch(($) => {
  const route = $.geometry.polyline("route", {
    vertices: [
      { key: "start", position: [0, 0] },
      { key: "bend", position: [30, 0] },
      { key: "end", position: [30, 30] },
    ],
    closed: false,
    branchDirections: [[1, 0], [0, 1]],
  });
  const spine = $.aggregate.openChain("spine", {
    spans: [route.segments.byKey.start, route.segments.byKey.bend],
  });
  const left = $.operation.profileOffset("left", {
    sources: [spine.chain],
    distance: mm(3),
    side: "left",
    firstTraversal: "forward",
  });
  const right = $.operation.profileOffset("right", {
    sources: [spine.chain],
    distance: mm(3),
    side: "right",
    firstTraversal: "forward",
  });
  return { route, spine, left, right };
});
