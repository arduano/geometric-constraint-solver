// SPDX-License-Identifier: GPL-3.0-or-later

"use geosolve sketch";

import { deg, mm, sketch } from "@geosolve/sketch-code";

export default sketch(($) => {
  const horizontalLine = $.geometry.segment("horizontalLine", {
    start: [0, 0],
    end: [4, 0],
  });
  const verticalLine = $.geometry.segment("verticalLine", {
    start: [0, 0],
    end: [0, 4],
  });
  const offsetLine = $.geometry.segment("offsetLine", {
    start: [1, 1],
    end: [5, 1],
  });
  const chain = $.aggregate.openChain("chain", {
    spans: [horizontalLine.span],
  });

  const pointDistance = $.dimension.pointDistance("pointDistance", {
    first: horizontalLine.start,
    second: horizontalLine.end,
    value: mm(4),
    mode: "reference",
    suppressed: true,
  });
  const curveLength = $.dimension.curveLength("curveLength", {
    curve: horizontalLine.span,
    value: mm(4),
    mode: "reference",
    suppressed: true,
  });
  const radius = $.dimension.radius("radius", {
    curve: horizontalLine.curve,
    value: mm(2),
    mode: "reference",
    suppressed: true,
  });
  const diameter = $.dimension.diameter("diameter", {
    curve: horizontalLine.curve,
    value: mm(4),
    mode: "reference",
    suppressed: true,
  });
  const orientedAngle = $.dimension.orientedAngle("orientedAngle", {
    first: horizontalLine.span,
    second: verticalLine.span,
    value: deg(90),
    mode: "reference",
    orientation: "counterClockwise",
    suppressed: true,
  });
  const supportingLineOffset = $.dimension.supportingLineOffset(
    "supportingLineOffset",
    {
      first: horizontalLine.span,
      second: offsetLine.span,
      value: mm(1),
      mode: "reference",
      side: "left",
      orientation: "same",
      suppressed: true,
    },
  );
  const exactTranslatedSegmentOffset =
    $.dimension.exactTranslatedSegmentOffset(
      "exactTranslatedSegmentOffset",
      {
        first: horizontalLine.span,
        second: offsetLine.span,
        value: mm(1),
        mode: "reference",
        side: "left",
        orientation: "same",
        suppressed: true,
      },
    );
  const profileOffset = $.dimension.profileOffset("profileOffset", {
    source: chain,
    target: offsetLine.span,
    value: mm(1),
    direction: "outward",
    side: "left",
    sourceTraversal: "forward",
    targetTraversal: "forward",
    suppressed: true,
  });

  return {
    horizontalLine,
    verticalLine,
    offsetLine,
    chain,
    pointDistance,
    curveLength,
    radius,
    diameter,
    orientedAngle,
    supportingLineOffset,
    exactTranslatedSegmentOffset,
    profileOffset,
  };
});
