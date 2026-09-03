"use geosolve sketch";
import { sketch, mm, rad } from "@geosolve/sketch-code";

export default sketch(($) => {
  const point1FourBarGroundO2 = $.geometry.sketchPoint("point1FourBarGroundO2", {
    point: [0, 0],
    label: "Four-bar ground O2",
  });
  const point2FourBarGroundO4 = $.geometry.sketchPoint("point2FourBarGroundO4", {
    point: [8, 0],
    label: "Four-bar ground O4",
  });
  const point3FourBarInputJointA = $.geometry.sketchPoint("point3FourBarInputJointA", {
    point: [3, 4],
    label: "Four-bar input joint A",
  });
  const point4FourBarOutputJointB = $.geometry.sketchPoint("point4FourBarOutputJointB", {
    point: [7, 4],
    label: "Four-bar output joint B",
  });
  const point5FourBarCouplerTracerC = $.geometry.sketchPoint("point5FourBarCouplerTracerC", {
    point: [5, 4],
    label: "Four-bar coupler tracer C",
  });
  const curve1FourBarInputCrankO2A = $.geometry.segment("curve1FourBarInputCrankO2A", {
    start: point1FourBarGroundO2.point,
    end: point3FourBarInputJointA.point,
    branchDirection: [0.6, 0.8],
    label: "Four-bar input crank O2-A",
    role: "profile",
  });
  const curve2FourBarCouplerAB = $.geometry.segment("curve2FourBarCouplerAB", {
    start: point3FourBarInputJointA.point,
    end: point4FourBarOutputJointB.point,
    branchDirection: [1, 0],
    label: "Four-bar coupler A-B",
    role: "profile",
  });
  const curve3FourBarOutputRockerBO4 = $.geometry.segment("curve3FourBarOutputRockerBO4", {
    start: point4FourBarOutputJointB.point,
    end: point2FourBarGroundO4.point,
    branchDirection: [0.24253562503633297, -0.9701425001453319],
    label: "Four-bar output rocker B-O4",
    role: "profile",
  });
  const constraint1FourBarGroundO2Fixed = $.constraint.fixedPoint("constraint1FourBarGroundO2Fixed", {
    point: point1FourBarGroundO2.point,
    target: [0, 0],
    label: "Four-bar ground O2 fixed",
  });
  const constraint2FourBarGroundO4Fixed = $.constraint.fixedPoint("constraint2FourBarGroundO4Fixed", {
    point: point2FourBarGroundO4.point,
    target: [8, 0],
    label: "Four-bar ground O4 fixed",
  });
  const dimension1FourBarInputCrankLength5 = $.dimension.curveLength("dimension1FourBarInputCrankLength5", {
    curve: curve1FourBarInputCrankO2A.span,
    value: mm(5),
    label: "Four-bar input crank length 5",
    mode: "driving",
  });
  const dimension2FourBarCouplerLength4 = $.dimension.curveLength("dimension2FourBarCouplerLength4", {
    curve: curve2FourBarCouplerAB.span,
    value: mm(4),
    label: "Four-bar coupler length 4",
    mode: "driving",
  });
  const dimension3FourBarOutputRockerLengthSqrt17 = $.dimension.curveLength("dimension3FourBarOutputRockerLengthSqrt17", {
    curve: curve3FourBarOutputRockerBO4.span,
    value: mm(4.123105625617661),
    label: "Four-bar output rocker length sqrt 17",
    mode: "driving",
  });
  const constraint3FourBarTracerBisectsCoupler = $.constraint.midpoint("constraint3FourBarTracerBisectsCoupler", {
    point: point5FourBarCouplerTracerC.point,
    line: curve2FourBarCouplerAB.span,
    label: "Four-bar tracer bisects coupler",
  });
  $.group("Points", [point1FourBarGroundO2, point2FourBarGroundO4, point3FourBarInputJointA, point4FourBarOutputJointB, point5FourBarCouplerTracerC]);
  $.group("Geometry", [curve1FourBarInputCrankO2A, curve2FourBarCouplerAB, curve3FourBarOutputRockerBO4]);
  $.group("Constraints", [constraint1FourBarGroundO2Fixed, constraint2FourBarGroundO4Fixed, constraint3FourBarTracerBisectsCoupler]);
  $.group("Dimensions", [dimension1FourBarInputCrankLength5, dimension2FourBarCouplerLength4, dimension3FourBarOutputRockerLengthSqrt17]);
  return {};
});
