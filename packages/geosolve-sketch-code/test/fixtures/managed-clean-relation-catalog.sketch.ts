// SPDX-License-Identifier: GPL-3.0-or-later

"use geosolve sketch";

import { mm, sketch } from "@geosolve/sketch-code";

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
  const fixedPoint = $.constraint.fixedPoint("fixedPoint", {
    point: horizontalLine.start,
    target: [0, 0],
    suppressed: true,
  });
  const fixedCoordinate = $.constraint.fixedCoordinate("fixedCoordinate", {
    point: horizontalLine.start,
    axis: "x",
    target: mm(0),
    suppressed: true,
  });
  const coincidentWithOrigin = $.constraint.coincidentWithOrigin(
    "coincidentWithOrigin",
    { point: horizontalLine.start, suppressed: true },
  );
  const pointOnDatumAxis = $.constraint.pointOnDatumAxis("pointOnDatumAxis", {
    point: horizontalLine.end,
    axis: "x",
    suppressed: true,
  });
  const coincident = $.constraint.coincident("coincident", {
    first: horizontalLine.start,
    second: verticalLine.start,
    suppressed: true,
  });
  const horizontal = $.constraint.horizontal("horizontal", {
    span: horizontalLine.span,
    suppressed: true,
  });
  const vertical = $.constraint.vertical("vertical", {
    span: verticalLine.span,
    suppressed: true,
  });
  const horizontalPoints = $.constraint.horizontalPoints("horizontalPoints", {
    first: horizontalLine.start,
    second: horizontalLine.end,
    suppressed: true,
  });
  const verticalPoints = $.constraint.verticalPoints("verticalPoints", {
    first: verticalLine.start,
    second: verticalLine.end,
    suppressed: true,
  });
  const horizontalPointToMidpoint = $.constraint.horizontalPointToMidpoint(
    "horizontalPointToMidpoint",
    {
      point: offsetLine.start,
      line: horizontalLine.span,
      suppressed: true,
    },
  );
  const verticalPointToMidpoint = $.constraint.verticalPointToMidpoint(
    "verticalPointToMidpoint",
    {
      point: offsetLine.start,
      line: verticalLine.span,
      suppressed: true,
    },
  );
  const pointOnCurve = $.constraint.pointOnCurve("pointOnCurve", {
    point: horizontalLine.start,
    curve: verticalLine.span,
    suppressed: true,
  });
  const parallel = $.constraint.parallel("parallel", {
    first: horizontalLine.span,
    second: offsetLine.span,
    suppressed: true,
  });
  const perpendicular = $.constraint.perpendicular("perpendicular", {
    first: horizontalLine.span,
    second: verticalLine.span,
    suppressed: true,
  });
  const collinearWithDatumAxis = $.constraint.collinearWithDatumAxis(
    "collinearWithDatumAxis",
    { span: horizontalLine.span, axis: "x", suppressed: true },
  );
  const concentric = $.constraint.concentric("concentric", {
    first: horizontalLine.curve,
    second: offsetLine.curve,
    suppressed: true,
  });
  const collinear = $.constraint.collinear("collinear", {
    first: horizontalLine.span,
    second: offsetLine.span,
    firstDirection: "forward",
    secondDirection: "forward",
    suppressed: true,
  });
  const equalLength = $.constraint.equalLength("equalLength", {
    first: horizontalLine.span,
    second: offsetLine.span,
    suppressed: true,
  });
  const equalRadius = $.constraint.equalRadius("equalRadius", {
    first: horizontalLine.curve,
    second: offsetLine.curve,
    suppressed: true,
  });
  const midpoint = $.constraint.midpoint("midpoint", {
    point: offsetLine.start,
    line: horizontalLine.span,
    suppressed: true,
  });
  const symmetricAboutLine = $.constraint.symmetricAboutLine(
    "symmetricAboutLine",
    {
      first: horizontalLine.start,
      second: horizontalLine.end,
      axis: verticalLine.span,
      suppressed: true,
    },
  );
  const symmetricAboutDatumAxis = $.constraint.symmetricAboutDatumAxis(
    "symmetricAboutDatumAxis",
    {
      first: horizontalLine.start,
      second: horizontalLine.end,
      axis: "y",
      suppressed: true,
    },
  );
  const lineCircleTangency = $.constraint.lineCircleTangency(
    "lineCircleTangency",
    {
      line: horizontalLine.span,
      circle: horizontalLine.curve,
      side: "left",
      suppressed: true,
    },
  );
  const circleCircleTangency = $.constraint.circleCircleTangency(
    "circleCircleTangency",
    {
      first: horizontalLine.curve,
      second: offsetLine.curve,
      mode: "external",
      centerDirection: [1, 0],
      suppressed: true,
    },
  );
  const circleArcTangency = $.constraint.circleArcTangency(
    "circleArcTangency",
    {
      circle: horizontalLine.curve,
      arc: offsetLine.curve,
      side: "outsideArc",
      suppressed: true,
    },
  );
  const lineCurveTangency = $.constraint.lineCurveTangency(
    "lineCurveTangency",
    {
      line: horizontalLine.span,
      curve: offsetLine.span,
      endpoint: "start",
      suppressed: true,
    },
  );
  const curveCurveContact = $.constraint.curveCurveContact(
    "curveCurveContact",
    {
      first: horizontalLine.span,
      second: offsetLine.span,
      suppressed: true,
    },
  );
  const curveCurveTangency = $.constraint.curveCurveTangency(
    "curveCurveTangency",
    {
      first: horizontalLine.span,
      second: offsetLine.span,
      suppressed: true,
    },
  );
  const curveDirection = $.constraint.curveDirection("curveDirection", {
    first: horizontalLine.span,
    second: offsetLine.span,
    relation: "tangent",
    orientation: "aligned",
    side: "left",
    suppressed: true,
  });
  const equalCurvature = $.constraint.equalCurvature("equalCurvature", {
    first: horizontalLine.span,
    second: offsetLine.span,
    relation: "signed",
    suppressed: true,
  });
  const endpointContinuity = $.constraint.endpointContinuity(
    "endpointContinuity",
    {
      first: horizontalLine.span,
      second: offsetLine.span,
      continuity: "g1",
      suppressed: true,
    },
  );
  const lineLineFillet = $.constraint.lineLineFillet("lineLineFillet", {
    fillet: horizontalLine.curve,
    first: horizontalLine.span,
    second: verticalLine.span,
    firstSide: "left",
    secondSide: "right",
    endpointOrder: "firstThenSecond",
    suppressed: true,
  });
  const curveCurveFillet = $.constraint.curveCurveFillet(
    "curveCurveFillet",
    {
      fillet: horizontalLine.curve,
      first: horizontalLine.span,
      second: offsetLine.span,
      firstSide: "left",
      secondSide: "right",
      endpointOrder: "firstThenSecond",
      firstTrimEndpoint: "end",
      secondTrimEndpoint: "start",
      suppressed: true,
    },
  );

  return {
    horizontalLine,
    verticalLine,
    offsetLine,
    fixedPoint,
    fixedCoordinate,
    coincidentWithOrigin,
    pointOnDatumAxis,
    coincident,
    horizontal,
    vertical,
    horizontalPoints,
    verticalPoints,
    horizontalPointToMidpoint,
    verticalPointToMidpoint,
    pointOnCurve,
    parallel,
    perpendicular,
    collinearWithDatumAxis,
    concentric,
    collinear,
    equalLength,
    equalRadius,
    midpoint,
    symmetricAboutLine,
    symmetricAboutDatumAxis,
    lineCircleTangency,
    circleCircleTangency,
    circleArcTangency,
    lineCurveTangency,
    curveCurveContact,
    curveCurveTangency,
    curveDirection,
    equalCurvature,
    endpointContinuity,
    lineLineFillet,
    curveCurveFillet,
  };
});
