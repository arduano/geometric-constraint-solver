"use geosolve sketch";
import { sketch, mm, rad } from "@geosolve/sketch-code";

export default sketch(($) => {
  const point1CircleCenter = $.geometry.sketchPoint("point1CircleCenter", {
    point: [0, 0],
    label: "Circle center",
  });
  const point2TangentStart = $.geometry.sketchPoint("point2TangentStart", {
    point: [-4, 2],
    label: "Tangent start",
  });
  const point3TangentEnd = $.geometry.sketchPoint("point3TangentEnd", {
    point: [4, 2],
    label: "Tangent end",
  });
  const point4NormalStart = $.geometry.sketchPoint("point4NormalStart", {
    point: [-4, 0],
    label: "Normal start",
  });
  const point5NormalEnd = $.geometry.sketchPoint("point5NormalEnd", {
    point: [4, 0],
    label: "Normal end",
  });
  const curve1ReferenceCircle = $.geometry.centerRadiusCircle("curve1ReferenceCircle", {
    center: point1CircleCenter.point,
    radius: mm(2),
    label: "Reference circle",
    role: "profile",
  });
  const curve2TrueTangentLine = $.geometry.segment("curve2TrueTangentLine", {
    start: point2TangentStart.point,
    end: point3TangentEnd.point,
    branchDirection: [1, 0],
    label: "True tangent line",
    role: "profile",
  });
  const curve3RadialNormalLine = $.geometry.segment("curve3RadialNormalLine", {
    start: point4NormalStart.point,
    end: point5NormalEnd.point,
    branchDirection: [1, 0],
    label: "Radial normal line",
    role: "profile",
  });
  const constraint1FixedCircleCenter = $.constraint.fixedPoint("constraint1FixedCircleCenter", {
    point: point1CircleCenter.point,
    target: [0, 0],
    label: "Fixed circle center",
  });
  const dimension1CircleRadius2 = $.dimension.radius("dimension1CircleRadius2", {
    curve: curve1ReferenceCircle.curve,
    value: mm(2),
    label: "Circle radius 2",
    mode: "driving",
  });
  const constraint2LineTangentToCircle = $.constraint.curveCurveTangency("constraint2LineTangentToCircle", {
    first: curve2TrueTangentLine.span,
    second: curve1ReferenceCircle.span,
    contacts: {
      first: {
        parameter: 0.5,
        winding: 0,
        neighborhood: {
          kind: "interior",
        },
        orientation: "opposed",
      },
      second: {
        parameter: 1.5707963267948966,
        winding: 0,
        neighborhood: {
          kind: "interior",
        },
        orientation: "opposed",
      },
    },
    label: "Line tangent to circle",
  });
  const constraint3CircleCenterOnNormalLine = $.constraint.pointOnCurve("constraint3CircleCenterOnNormalLine", {
    point: point1CircleCenter.point,
    curve: curve3RadialNormalLine.span,
    contact: {
      parameter: 0.5,
      winding: 0,
      neighborhood: {
        kind: "interior",
      },
      orientation: "none",
    },
    label: "Circle center on normal line",
  });
  $.group("Points", [point1CircleCenter, point2TangentStart, point3TangentEnd, point4NormalStart, point5NormalEnd]);
  $.group("Geometry", [curve1ReferenceCircle, curve2TrueTangentLine, curve3RadialNormalLine]);
  $.group("Constraints", [constraint1FixedCircleCenter, constraint2LineTangentToCircle, constraint3CircleCenterOnNormalLine]);
  $.group("Dimensions", [dimension1CircleRadius2]);
  return {};
});
