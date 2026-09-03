"use geosolve sketch";
import { sketch, mm, rad } from "@geosolve/sketch-code";

export default sketch(($) => {
  const point1BridgeLeftP0 = $.geometry.sketchPoint("point1BridgeLeftP0", {
    point: [-4, 0],
    label: "Bridge left P0",
  });
  const point2BridgeLeftP1 = $.geometry.sketchPoint("point2BridgeLeftP1", {
    point: [-3, 2],
    label: "Bridge left P1",
  });
  const point3BridgeLeftP2 = $.geometry.sketchPoint("point3BridgeLeftP2", {
    point: [-1, 2],
    label: "Bridge left P2",
  });
  const point4BridgeLeftSeam = $.geometry.sketchPoint("point4BridgeLeftSeam", {
    point: [0, 0],
    label: "Bridge left seam",
  });
  const point5BridgeRightSeam = $.geometry.sketchPoint("point5BridgeRightSeam", {
    point: [0, 0],
    label: "Bridge right seam",
  });
  const point6BridgeRightP1 = $.geometry.sketchPoint("point6BridgeRightP1", {
    point: [1, -2],
    label: "Bridge right P1",
  });
  const point7BridgeRightP2 = $.geometry.sketchPoint("point7BridgeRightP2", {
    point: [3, -2],
    label: "Bridge right P2",
  });
  const point8BridgeRightP3 = $.geometry.sketchPoint("point8BridgeRightP3", {
    point: [4, 0],
    label: "Bridge right P3",
  });
  const curve1BridgeLeftCubicBezier = $.geometry.cubicBezier("curve1BridgeLeftCubicBezier", {
    start: point1BridgeLeftP0.point,
    firstControl: point2BridgeLeftP1.point,
    secondControl: point3BridgeLeftP2.point,
    end: point4BridgeLeftSeam.point,
    label: "Bridge left cubic Bezier",
    role: "profile",
  });
  const curve2BridgeRightCubicBezier = $.geometry.cubicBezier("curve2BridgeRightCubicBezier", {
    start: point5BridgeRightSeam.point,
    firstControl: point6BridgeRightP1.point,
    secondControl: point7BridgeRightP2.point,
    end: point8BridgeRightP3.point,
    label: "Bridge right cubic Bezier",
    role: "profile",
  });
  const curve3BridgeLeftSeamHandle = $.geometry.segment("curve3BridgeLeftSeamHandle", {
    start: point3BridgeLeftP2.point,
    end: point4BridgeLeftSeam.point,
    branchDirection: [0.4472135954999579, -0.8944271909999159],
    label: "Bridge left seam handle",
    role: "profile",
  });
  const curve4BridgeRightSeamHandle = $.geometry.segment("curve4BridgeRightSeamHandle", {
    start: point5BridgeRightSeam.point,
    end: point6BridgeRightP1.point,
    branchDirection: [0.4472135954999579, -0.8944271909999159],
    label: "Bridge right seam handle",
    role: "profile",
  });
  const constraint1BridgeOuterControl1Fixed = $.constraint.fixedPoint("constraint1BridgeOuterControl1Fixed", {
    point: point1BridgeLeftP0.point,
    target: [-4, 0],
    label: "Bridge outer control 1 fixed",
  });
  const constraint2BridgeOuterControl2Fixed = $.constraint.fixedPoint("constraint2BridgeOuterControl2Fixed", {
    point: point2BridgeLeftP1.point,
    target: [-3, 2],
    label: "Bridge outer control 2 fixed",
  });
  const constraint3BridgeOuterControl3Fixed = $.constraint.fixedPoint("constraint3BridgeOuterControl3Fixed", {
    point: point3BridgeLeftP2.point,
    target: [-1, 2],
    label: "Bridge outer control 3 fixed",
  });
  const constraint4BridgeOuterControl4Fixed = $.constraint.fixedPoint("constraint4BridgeOuterControl4Fixed", {
    point: point6BridgeRightP1.point,
    target: [1, -2],
    label: "Bridge outer control 4 fixed",
  });
  const constraint5BridgeOuterControl5Fixed = $.constraint.fixedPoint("constraint5BridgeOuterControl5Fixed", {
    point: point7BridgeRightP2.point,
    target: [3, -2],
    label: "Bridge outer control 5 fixed",
  });
  const constraint6BridgeOuterControl6Fixed = $.constraint.fixedPoint("constraint6BridgeOuterControl6Fixed", {
    point: point8BridgeRightP3.point,
    target: [4, 0],
    label: "Bridge outer control 6 fixed",
  });
  const constraint7BridgeC1EndpointTangency = $.constraint.curveCurveTangency("constraint7BridgeC1EndpointTangency", {
    first: curve1BridgeLeftCubicBezier.span,
    second: curve2BridgeRightCubicBezier.span,
    contacts: {
      first: {
        parameter: 1,
        winding: 0,
        neighborhood: {
          kind: "end",
        },
        orientation: "aligned",
      },
      second: {
        parameter: 0,
        winding: 0,
        neighborhood: {
          kind: "start",
        },
        orientation: "aligned",
      },
    },
    label: "Bridge C1 endpoint tangency",
  });
  const constraint8BridgeEqualSeamHandles = $.constraint.equalLength("constraint8BridgeEqualSeamHandles", {
    first: curve3BridgeLeftSeamHandle.span,
    second: curve4BridgeRightSeamHandle.span,
    label: "Bridge equal seam handles",
    suppressed: true,
  });
  const dimension1BridgeLeftHandleReference = $.dimension.curveLength("dimension1BridgeLeftHandleReference", {
    curve: curve3BridgeLeftSeamHandle.span,
    value: mm(2.23606797749979),
    label: "Bridge left handle reference",
    mode: "reference",
  });
  const dimension2BridgeRightHandleReference = $.dimension.curveLength("dimension2BridgeRightHandleReference", {
    curve: curve4BridgeRightSeamHandle.span,
    value: mm(2.23606797749979),
    label: "Bridge right handle reference",
    mode: "reference",
  });
  $.group("Points", [point1BridgeLeftP0, point2BridgeLeftP1, point3BridgeLeftP2, point4BridgeLeftSeam, point5BridgeRightSeam, point6BridgeRightP1, point7BridgeRightP2, point8BridgeRightP3]);
  $.group("Geometry", [curve1BridgeLeftCubicBezier, curve2BridgeRightCubicBezier, curve3BridgeLeftSeamHandle, curve4BridgeRightSeamHandle]);
  $.group("Constraints", [constraint1BridgeOuterControl1Fixed, constraint2BridgeOuterControl2Fixed, constraint3BridgeOuterControl3Fixed, constraint4BridgeOuterControl4Fixed, constraint5BridgeOuterControl5Fixed, constraint6BridgeOuterControl6Fixed, constraint7BridgeC1EndpointTangency, constraint8BridgeEqualSeamHandles]);
  $.group("Dimensions", [dimension1BridgeLeftHandleReference, dimension2BridgeRightHandleReference]);
  return {};
});
