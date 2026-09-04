"use geosolve sketch";
import { sketch, mm, rad } from "@geosolve/sketch-code";

export default sketch(($) => {
  const point1A3LineStart = $.geometry.sketchPoint("point1A3LineStart", {
    point: [-5, 0],
    label: "A3 line start",
  });
  const point2A3LineEnd = $.geometry.sketchPoint("point2A3LineEnd", {
    point: [5, 0],
    label: "A3 line end",
  });
  const point3A3GuideG = $.geometry.sketchPoint("point3A3GuideG", {
    point: [1, 0],
    label: "A3 guide G",
  });
  const point4A3CircleCenterO = $.geometry.sketchPoint("point4A3CircleCenterO", {
    point: [1, 3],
    label: "A3 circle center O",
  });
  const curve1A3FixedLine = $.geometry.segment("curve1A3FixedLine", {
    start: point1A3LineStart.point,
    end: point2A3LineEnd.point,
    branchDirection: [1, 0],
    label: "A3 fixed line",
    role: "profile",
  });
  const curve2A3VerticalGuide = $.geometry.segment("curve2A3VerticalGuide", {
    start: point3A3GuideG.point,
    end: point4A3CircleCenterO.point,
    branchDirection: [0, 1],
    label: "A3 vertical guide",
    role: "profile",
  });
  const curve3A3Circle = $.geometry.centerRadiusCircle("curve3A3Circle", {
    center: point4A3CircleCenterO.point,
    radius: mm(2),
    label: "A3 circle",
    role: "profile",
  });
  const constraint1A3LineStartFixed = $.constraint.fixedPoint("constraint1A3LineStartFixed", {
    point: point1A3LineStart.point,
    target: [-5, 0],
    label: "A3 line start fixed",
  });
  const constraint2A3LineEndFixed = $.constraint.fixedPoint("constraint2A3LineEndFixed", {
    point: point2A3LineEnd.point,
    target: [5, 0],
    label: "A3 line end fixed",
  });
  const constraint3A3GuideFixed = $.constraint.fixedPoint("constraint3A3GuideFixed", {
    point: point3A3GuideG.point,
    target: [1, 0],
    label: "A3 guide fixed",
  });
  const constraint4A3CenterGuideVertical = $.constraint.vertical("constraint4A3CenterGuideVertical", {
    span: curve2A3VerticalGuide.span,
    label: "A3 center guide vertical",
  });
  const dimension1A3Radius2 = $.dimension.radius("dimension1A3Radius2", {
    curve: curve3A3Circle.curve,
    value: mm(2),
    label: "A3 radius 2",
    mode: "driving",
  });
  const constraint5A3LineCircleTangency = $.constraint.lineCircleTangency("constraint5A3LineCircleTangency", {
    line: curve1A3FixedLine.span,
    circle: curve3A3Circle.curve,
    contacts: {
      first: {
        parameter: 0.6,
        winding: 0,
        neighborhood: {
          kind: "interior",
        },
        orientation: "aligned",
      },
      second: {
        parameter: 4.71238898038469,
        winding: 0,
        neighborhood: {
          kind: "interior",
        },
        orientation: "aligned",
      },
    },
    side: "left",
    label: "A3 line-circle tangency",
  });
  $.group("Points", [point1A3LineStart, point2A3LineEnd, point3A3GuideG, point4A3CircleCenterO]);
  $.group("Geometry", [curve1A3FixedLine, curve2A3VerticalGuide, curve3A3Circle]);
  $.group("Constraints", [constraint1A3LineStartFixed, constraint2A3LineEndFixed, constraint3A3GuideFixed, constraint4A3CenterGuideVertical, constraint5A3LineCircleTangency]);
  $.group("Dimensions", [dimension1A3Radius2]);
  return {};
});
