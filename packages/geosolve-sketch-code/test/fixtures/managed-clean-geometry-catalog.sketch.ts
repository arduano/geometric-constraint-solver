// SPDX-License-Identifier: GPL-3.0-or-later

"use geosolve sketch";

import { mm, sketch } from "@geosolve/sketch-code";

export default sketch(($) => {
  const sketchPoint = $.geometry.sketchPoint("sketchPoint", {
    point: [0, 0],
  });
  const segment = $.geometry.segment("segment", {
    start: [0, 4],
    end: [4, 4],
  });
  const polyline = $.geometry.polyline("polyline", {
    vertices: [
      { key: "start", position: [0, 8] },
      { key: "corner", position: [4, 8] },
      { key: "end", position: [4, 11] },
    ],
    closed: false,
  });
  const midpointLine = $.geometry.midpointLine("midpointLine", {
    midpoint: [12, 4],
    end: [15, 5],
  });
  const twoPointAlignedRectangle = $.geometry.twoPointAlignedRectangle(
    "twoPointAlignedRectangle",
    { firstCorner: [20, 0], oppositeCorner: [25, 3] },
  );
  const threePointCornerRectangle = $.geometry.threePointCornerRectangle(
    "threePointCornerRectangle",
    {
      firstCorner: [30, 0],
      secondCorner: [35, 1],
      thirdCorner: [34.4, 4],
    },
  );
  const centerRectangle = $.geometry.centerRectangle("centerRectangle", {
    center: [42, 2],
    corner: [45, 4],
  });
  const threePointCenterRectangle = $.geometry.threePointCenterRectangle(
    "threePointCenterRectangle",
    {
      center: [54, 2],
      corner: [57, 4],
      sideMidpoint: [51, 4],
    },
  );
  const centerRadiusCircle = $.geometry.centerRadiusCircle("centerRadiusCircle", {
    center: [2, 18],
    radius: mm(2),
  });
  const twoPointDiameterCircle = $.geometry.twoPointDiameterCircle(
    "twoPointDiameterCircle",
    { start: [10, 16], end: [14, 20] },
  );
  const threePointCircle = $.geometry.threePointCircle("threePointCircle", {
    first: [20, 18],
    second: [22, 16],
    third: [24, 18],
  });
  const centerArc = $.geometry.centerArc("centerArc", {
    center: [32, 18],
    start: [35, 18],
    end: [32, 21],
    sweep: "counterClockwise",
  });
  const threePointArc = $.geometry.threePointArc("threePointArc", {
    first: [40, 18],
    second: [42, 20],
    third: [44, 18],
    sweep: "clockwise",
  });
  const tangentArc = $.geometry.tangentArc("tangentArc", {
    center: [4, 6],
    start: segment.end,
    end: [6, 6],
    source: {
      span: segment.span,
      contact: {
        parameter: 1,
        winding: 0,
        domain: { kind: "bounded", lower: 0, upper: 1 },
        neighborhood: { kind: "end" },
        orientation: "aligned",
      },
    },
    sweep: "counterClockwise",
    orientation: "aligned",
  });
  const centerAxesEllipse = $.geometry.centerAxesEllipse("centerAxesEllipse", {
    center: [2, 30],
    majorAxisPoint: [6, 30],
    minorAxisPoint: [2, 32],
  });
  const axisEndpointsEllipse = $.geometry.axisEndpointsEllipse(
    "axisEndpointsEllipse",
    {
      majorAxisStart: [10, 30],
      majorAxisEnd: [18, 30],
      minorAxisPoint: [14, 32],
    },
  );
  const centerAxesEllipticalArc = $.geometry.centerAxesEllipticalArc(
    "centerAxesEllipticalArc",
    {
      center: [24, 30],
      majorAxisPoint: [28, 30],
      minorAxisPoint: [24, 32],
      start: [28, 30],
      end: [24, 32],
      sweep: "counterClockwise",
    },
  );
  const axisEndpointsEllipticalArc = $.geometry.axisEndpointsEllipticalArc(
    "axisEndpointsEllipticalArc",
    {
      majorAxisStart: [34, 30],
      majorAxisEnd: [42, 30],
      minorAxisPoint: [38, 32],
      start: [42, 30],
      end: [38, 32],
      sweep: "counterClockwise",
    },
  );
  const quadraticBezier = $.geometry.quadraticBezier("quadraticBezier", {
    start: [0, 42],
    control: [3, 46],
    end: [7, 42],
  });
  const cubicBezier = $.geometry.cubicBezier("cubicBezier", {
    start: [12, 42],
    firstControl: [14, 46],
    secondControl: [18, 46],
    end: [20, 42],
  });
  const rationalQuadraticConic = $.geometry.rationalQuadraticConic(
    "rationalQuadraticConic",
    {
      start: [25, 42],
      weightedMiddle: [28, 46],
      end: [32, 42],
      middleWeight: 0.75,
    },
  );
  const parabola = $.geometry.parabola("parabola", {
    vertex: [38, 42],
    focus: [38, 44],
    trimStart: -2,
    trimEnd: 2,
  });
  const hyperbola = $.geometry.hyperbola("hyperbola", {
    center: [48, 42],
    transverseAxisPoint: [51, 42],
    semiConjugate: mm(2),
    trimStart: -1,
    trimEnd: 1,
    branch: "positive",
  });
  const openControlNurbs = $.geometry.openControlNurbs("openControlNurbs", {
    controls: [
      { key: "a", position: [0, 56], weight: 1 },
      { key: "b", position: [3, 60], weight: 0.8 },
      { key: "c", position: [7, 60], weight: 1.2 },
      { key: "d", position: [10, 56], weight: 1 },
    ],
    degree: 3,
    gauge: "b",
  });
  const periodicControlNurbs = $.geometry.periodicControlNurbs(
    "periodicControlNurbs",
    {
      controls: [
        { key: "north", position: [20, 60], weight: 1 },
        { key: "east", position: [24, 56], weight: 1 },
        { key: "south", position: [20, 52], weight: 1 },
        { key: "west", position: [16, 56], weight: 1 },
      ],
      degree: 2,
      gauge: "north",
    },
  );

  return {
    sketchPoint,
    segment,
    polyline,
    midpointLine,
    twoPointAlignedRectangle,
    threePointCornerRectangle,
    centerRectangle,
    threePointCenterRectangle,
    centerRadiusCircle,
    twoPointDiameterCircle,
    threePointCircle,
    centerArc,
    threePointArc,
    tangentArc,
    centerAxesEllipse,
    axisEndpointsEllipse,
    centerAxesEllipticalArc,
    axisEndpointsEllipticalArc,
    quadraticBezier,
    cubicBezier,
    rationalQuadraticConic,
    parabola,
    hyperbola,
    openControlNurbs,
    periodicControlNurbs,
  };
});
