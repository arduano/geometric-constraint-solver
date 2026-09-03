// SPDX-License-Identifier: GPL-3.0-or-later

"use geosolve sketch";

import { sketch, mm } from "@geosolve/sketch-code";

export default sketch(($) => {
  const base = $.geometry.segment("base", {
    start: [0, 0],
    end: [20, 0],
  });
  const guide = $.geometry.centerRadiusCircle("guide", {
    center: [30, 10],
    radius: mm(2),
  });
  const offsetChain11 = $.aggregate.openChain("offsetChain11", {
    label: "canvas.offset.helper",
    spans: [base.span],
  });
  const profileOffset12 = $.operation.profileOffset("profileOffset12", {
    label: "canvas.offset",
    sources: [offsetChain11.chain],
    distance: mm(1),
    side: "left",
    firstTraversal: "forward",
  });
  $.group("Canvas additions", [offsetChain11, profileOffset12]);
  return { base, guide, offsetChain11, profileOffset12 };
});
